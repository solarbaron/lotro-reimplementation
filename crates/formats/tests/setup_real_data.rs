// SPDX-License-Identifier: GPL-3.0-or-later
//! `setup.md` decode rates against a user's own install (read-only):
//!
//! - every setup (0x04), visual template (0x1F), appearance record (0x20) and entity
//!   descriptor (0x47, with the install's property dictionary) decodes with zero trailing
//!   bytes and re-encodes byte for byte, and the observed invariants SI1–SI5 hold
//!   (`first_unusual_field`);
//! - setup collision volumes (spheres, capsules and boxes, rotated or not) agree with the
//!   stored physics bounds (§6.4, §6.5);
//! - every part DID of every setup and appearance block exists in the mesh archives (SI4),
//!   every collision-mesh DID exists;
//! - every 0x47 and 0x1F record resolves to a setup through [`resolve_setup`] (§3.1, SI6).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a sample unless `TESSERA_FULL=1`. Run with
//! `cargo test --release -p tessera-formats --test setup_real_data -- --nocapture`. Only
//! counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::setup::{Aabb, MAX_CHAIN_HOPS};
use tessera_formats::{
    AppearanceRecord, Did, EntityDescriptor, FormatError, PropertyDictionary, Record, ResolveError,
    Setup, VisualTemplate, resolve_setup,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
/// Largest share of records allowed to deviate from an observed invariant (0.1 %); they are
/// printed. Decode failures, re-encode differences and bounds disagreements must be 0, as
/// on the reference install (§10).
const MAX_UNUSUAL_RATE: f64 = 0.001;

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
    })
    .as_ref()
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Runs `f` over `items` on every core; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(64, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 64).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// Every (member index, entry) with type prefix `prefix`, over all archives; sampled in
/// debug builds.
fn entries_with_prefix(set: &ArchiveSet, prefix: u8) -> Vec<(usize, Entry)> {
    let lo = u32::from(prefix) << 24;
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        out.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    let full = exhaustive();
    out.into_iter()
        .enumerate()
        .filter(|(i, _)| full || i % 23 == 0)
        .map(|(_, e)| e)
        .collect()
}

/// All DIDs with type prefix `prefix`, over all archives (never sampled).
fn dids_with_prefix(set: &ArchiveSet, prefix: u8) -> HashSet<u32> {
    let lo = u32::from(prefix) << 24;
    let mut out = HashSet::new();
    for m in set.members() {
        out.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
                .map(|e| e.expect("directory walk").did),
        );
    }
    out
}

fn mesh_dids(set: &ArchiveSet) -> &'static HashSet<u32> {
    static MESHES: OnceLock<HashSet<u32>> = OnceLock::new();
    MESHES.get_or_init(|| dids_with_prefix(set, 0x06))
}

/// Short error category for a resolver error.
fn resolve_category(e: &ResolveError<tessera_dat::Error>) -> String {
    match e {
        ResolveError::Format(f) => category(f),
        ResolveError::Fetch { source, .. } => format!("read: {source}"),
    }
}

/// Short error category for the census.
fn category(e: &FormatError) -> String {
    match e.root_cause() {
        FormatError::Truncated { .. } => "truncated".into(),
        FormatError::TrailingBytes { .. } => "trailing bytes".into(),
        FormatError::Unsupported { what, .. } => format!("unsupported {what}"),
        FormatError::CountMismatch { what, .. } => format!("count mismatch {what}"),
        FormatError::DidMismatch { .. } => "DID mismatch".into(),
        FormatError::DanglingReference { .. } => "dangling reference".into(),
        FormatError::ReferenceChain { reason, .. } => format!("chain: {reason}"),
        other => format!("{other:?}").chars().take(60).collect(),
    }
}

/// Outcome of decoding one record.
enum Outcome<T> {
    Clean(T),
    Unusual(u32, &'static str),
    Failed(u32, String),
}

struct Census<T> {
    total: usize,
    clean: Vec<T>,
    unusual: BTreeMap<&'static str, (usize, u32)>,
    failed: BTreeMap<String, (usize, u32)>,
}

impl<T> Census<T> {
    fn new(outcomes: Vec<Outcome<T>>) -> Self {
        let mut c = Self {
            total: outcomes.len(),
            clean: Vec::new(),
            unusual: BTreeMap::new(),
            failed: BTreeMap::new(),
        };
        for o in outcomes {
            match o {
                Outcome::Clean(t) => c.clean.push(t),
                Outcome::Unusual(did, why) => c.unusual.entry(why).or_insert((0, did)).0 += 1,
                Outcome::Failed(did, why) => c.failed.entry(why).or_insert((0, did)).0 += 1,
            }
        }
        c
    }

    fn report_and_check(&self, what: &str) {
        let bad: usize = self.failed.values().map(|v| v.0).sum();
        let odd: usize = self.unusual.values().map(|v| v.0).sum();
        eprintln!(
            "{what}: {} records, {} clean, {} failed, {} unusual",
            self.total,
            self.clean.len(),
            bad,
            odd
        );
        for (why, (n, did)) in &self.failed {
            eprintln!("  failed: {n} × {why} (e.g. {did:#010x})");
        }
        for (why, (n, did)) in &self.unusual {
            eprintln!("  unusual: {n} × {why} (e.g. {did:#010x})");
        }
        assert!(self.total > 0, "no {what} records found");
        assert_eq!(bad, 0, "{what}: {bad} of {} failed", self.total);
        assert!(
            odd as f64 <= MAX_UNUSUAL_RATE * self.total as f64,
            "{what}: {odd} unusual of {}",
            self.total
        );
    }
}

/// Decodes every record with `prefix`; a record whose re-encoding (`encode`) differs from
/// its payload counts as failed.
fn decode_all<T: Send>(
    set: &'static ArchiveSet,
    prefix: u8,
    decode: impl Fn(u32, &[u8]) -> Result<T, FormatError> + Sync,
    encode: impl Fn(&T, &[u8]) -> Result<Vec<u8>, FormatError> + Sync,
    unusual: impl Fn(&T) -> Option<&'static str> + Sync,
) -> Census<T> {
    let work = entries_with_prefix(set, prefix);
    Census::new(par_each(&work, |(i, e)| {
        let payload = match set.member(*i).archive.read_entry(e) {
            Ok(p) => p,
            Err(err) => return Outcome::Failed(e.did, format!("read: {err}")),
        };
        match decode(e.did, &payload) {
            Ok(t) if encode(&t, &payload).as_deref() != Ok(payload.as_slice()) => {
                Outcome::Failed(e.did, "re-encoding differs".into())
            }
            Ok(t) => match unusual(&t) {
                None => Outcome::Clean(t),
                Some(why) => Outcome::Unusual(e.did, why),
            },
            Err(err) => Outcome::Failed(e.did, category(&err)),
        }
    }))
}

#[test]
fn setups_decode_cleanly() {
    let Some(set) = open_set() else { return };
    let census = decode_all(
        set,
        0x04,
        |did, p| Setup::parse_expecting(p, did),
        |s, _| s.to_bytes(),
        Setup::first_unusual_field,
    );
    census.report_and_check("setups (0x04)");

    let meshes = mesh_dids(set);
    let collision = dids_with_prefix(set, 0x66);
    let mut parts_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let (mut parts, mut missing_parts, mut cmesh, mut missing_cmesh) = (0, 0, 0, 0);
    let (mut bad_height, mut bad_radius) = (0, 0);
    // [sphere/capsule-only, unrotated box-only, rotated box-only]: (checked, differ).
    let mut bounds = [(0usize, 0usize); 3];
    let mut lists = [0usize; 3];
    for s in &census.clean {
        *parts_hist.entry(s.parts.len()).or_default() += 1;
        parts += s.parts.len();
        missing_parts += s.parts.iter().filter(|d| !meshes.contains(&d.0)).count();
        if !s.collision_mesh.is_null() {
            cmesh += 1;
            missing_cmesh += usize::from(!collision.contains(&s.collision_mesh.0));
        }
        lists[0] += usize::from(!s.bone_frames.is_empty());
        lists[1] += usize::from(!s.placements.is_empty());
        lists[2] += usize::from(!s.named_frames.is_empty());
        let b = s.physics_bounds;
        if s.has_collision() {
            let h = b.aabb.max[2] - b.aabb.min[2];
            if (h - b.height).abs() > 1e-3 * h.abs().max(1.0) {
                bad_height += 1;
            }
        }
        if !s.collision_mesh.is_null() {
            continue;
        }
        // Volume-only setups: the bounds are the union of the volumes (§6.4).
        let class = match (
            s.boxes.is_empty(),
            s.spheres.is_empty() && s.capsules.is_empty(),
        ) {
            (true, false) => 0,
            (false, true) if s.boxes.iter().all(|x| x.frame.rotation.is_identity()) => 1,
            (false, true) => 2,
            _ => continue,
        };
        bounds[class].0 += 1;
        if !s.volume_bounds().is_some_and(|v| close(v, b.aabb)) {
            bounds[class].1 += 1;
        }
        // Horizontal radius of sphere-only setups: largest hypot(cx, cy) + r (§6.5).
        if s.boxes.is_empty() && s.capsules.is_empty() {
            let r = s
                .spheres
                .iter()
                .map(|x| x.centre[0].hypot(x.centre[1]) + x.radius)
                .fold(f32::MIN, f32::max);
            if (r - b.radius).abs() > 2e-3 * r.abs().max(1.0) {
                bad_radius += 1;
            }
        }
    }
    eprintln!("  part counts: {parts_hist:?}");
    eprintln!("  {parts} part DIDs, {missing_parts} missing from the mesh archives");
    eprintln!("  {cmesh} collision meshes, {missing_cmesh} missing");
    eprintln!("  frame lists A/B/C present in {lists:?} setups");
    eprintln!(
        "  volume bounds (checked, differ): sphere/capsule-only {:?}, unrotated box-only \
         {:?}, rotated box-only {:?}; {bad_height} heights and {bad_radius} sphere radii differ",
        bounds[0], bounds[1], bounds[2]
    );
    assert_eq!(missing_parts, 0, "SI4: part DIDs must exist");
    assert_eq!(missing_cmesh, 0, "SI4: collision meshes must exist");
    for (what, (checked, bad)) in ["sphere/capsule", "unrotated box", "rotated box"]
        .iter()
        .zip(bounds)
    {
        assert!(
            checked > 0 || !exhaustive(),
            "no {what}-only setups checked"
        );
        assert_eq!(
            bad, 0,
            "{what}-only volume bounds disagree in {bad} of {checked}"
        );
    }
    assert_eq!(bad_height, 0, "height disagrees in {bad_height} setups");
    assert_eq!(
        bad_radius, 0,
        "sphere radius disagrees in {bad_radius} setups"
    );
}

/// Whether two boxes agree within float noise (2e-3, relative for large coordinates).
fn close(a: Aabb, b: Aabb) -> bool {
    let scale = a
        .min
        .iter()
        .chain(&a.max)
        .fold(1.0f32, |m, v| m.max(v.abs()));
    a.approx_eq(b, 2e-3 * scale)
}

#[test]
fn visual_templates_decode_cleanly() {
    let Some(set) = open_set() else { return };
    let census = decode_all(
        set,
        0x1F,
        |did, p| VisualTemplate::parse_expecting(p, did),
        |t, _| Ok(t.to_bytes()),
        VisualTemplate::first_unusual_field,
    );
    census.report_and_check("visual templates (0x1F)");
    let mut flags: BTreeMap<u32, usize> = BTreeMap::new();
    for t in &census.clean {
        *flags.entry(t.flags).or_default() += 1;
    }
    let flags: Vec<String> = flags
        .iter()
        .map(|(f, n)| format!("{f:#05x}: {n}"))
        .collect();
    eprintln!("  flags: {}", flags.join(", "));
}

#[test]
fn appearances_decode_cleanly() {
    let Some(set) = open_set() else { return };
    let census = decode_all(
        set,
        0x20,
        |did, p| AppearanceRecord::parse_expecting(p, did),
        |a, _| Ok(a.to_bytes()),
        AppearanceRecord::first_unusual_field,
    );
    census.report_and_check("appearance records (0x20)");
    let meshes = mesh_dids(set);
    let (mut entries, mut blocks, mut groups, mut parts, mut extras, mut missing) =
        (0, 0, 0, 0, 0, 0);
    let mut no_base = 0;
    for a in &census.clean {
        entries += a.entries.len();
        for e in &a.entries {
            no_base += usize::from(e.base_block().is_none());
            blocks += e.blocks.len();
            for b in &e.blocks {
                groups += b.groups.len();
                parts += b.parts.len();
                extras += b.extras.len();
                missing += b.meshes().filter(|d| !meshes.contains(&d.0)).count();
            }
        }
    }
    eprintln!(
        "  {entries} entries ({no_base} without a base block), {blocks} blocks, {groups} \
         groups, {parts} parts ({missing} missing meshes), {extras} extras"
    );
    assert_eq!(missing, 0, "every appearance part names an existing mesh");
}

#[test]
fn entity_descriptors_decode_cleanly() {
    let Some(set) = open_set() else { return };
    let payload = set
        .read(PROPERTY_DICTIONARY)
        .expect("read the property dictionary");
    let dict = PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY)
        .expect("decode the property dictionary");
    let census = decode_all(
        set,
        0x47,
        |did, p| EntityDescriptor::parse_expecting(p, did, &dict),
        |e, p| e.to_bytes_with(e.property_set_bytes(p).unwrap_or_default()),
        EntityDescriptor::first_unusual_field,
    );
    census.report_and_check("entity descriptors (0x47)");
    let mut flags: BTreeMap<u32, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut extra = 0;
    for e in &census.clean {
        *flags.entry(e.header.flags).or_default() += 1;
        *kinds.entry(e.header.kind).or_default() += 1;
        extra += e.extra_sets.len();
    }
    eprintln!("  flags: {flags:x?}");
    eprintln!("  kinds: {kinds:?}");
    eprintln!("  {extra} 0x78 DIDs in extra-set lists");
}

#[test]
fn chains_resolve_to_setups() {
    let Some(set) = open_set() else { return };
    // An absent record is `None`; any other read failure is passed through as a fetch error.
    let fetch = |d: Did| match set.read(d.0) {
        Ok(p) => Ok(Some(p)),
        Err(tessera_dat::Error::NotFound { .. }) => Ok(None),
        Err(e) => Err(e),
    };
    for prefix in [0x47u8, 0x1F] {
        let work = entries_with_prefix(set, prefix);
        let outcomes = par_each(&work, |(_, e)| {
            resolve_setup(Did(e.did), fetch).map_err(|err| (e.did, resolve_category(&err)))
        });
        let total = outcomes.len();
        let mut hops: BTreeMap<usize, usize> = BTreeMap::new();
        let (mut with_parts, mut without_parts) = (0, 0);
        let mut failed: BTreeMap<String, (usize, u32)> = BTreeMap::new();
        for o in outcomes {
            match o {
                Ok(chain) => {
                    assert!(chain.hops() <= MAX_CHAIN_HOPS);
                    *hops.entry(chain.hops()).or_default() += 1;
                    if chain.parts().is_empty() {
                        without_parts += 1;
                    } else {
                        with_parts += 1;
                    }
                }
                Err((did, why)) => failed.entry(why).or_insert((0, did)).0 += 1,
            }
        }
        let bad: usize = failed.values().map(|v| v.0).sum();
        eprintln!(
            "chains from {prefix:#04x}: {total} starts, {with_parts} reach parts, \
             {without_parts} reach a setup without parts, {bad} fail; hops {hops:?}"
        );
        for (why, (n, did)) in &failed {
            eprintln!("  failed: {n} × {why} (e.g. {did:#010x})");
        }
        assert!(total > 0);
        assert_eq!(bad, 0, "{bad} of {total} chains from {prefix:#04x} fail");
    }
}
