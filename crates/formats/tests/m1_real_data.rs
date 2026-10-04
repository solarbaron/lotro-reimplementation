// SPDX-License-Identifier: GPL-3.0-or-later
//! M1 exit criteria for `tessera-formats` against a user's own install (read-only):
//!
//! - **property sets** (`property-system.md` §5, §11, §14): every 0x78–0x7F record is
//!   decoded with the install's property dictionary; the clean rate (no error, zero
//!   trailing bytes) must be ≥ 99.5 %, and failures are reported with trailing-byte counts
//!   and the responsible property type. PS-O1, PS-O2, PS-U1, PS-C1 and PS-C2 are checked
//!   on every decoded record; the +0x09000000 partners are checked over the directory;
//! - **render surfaces** (`texture-list.md` §7, M1 criterion 1): every 0x41 record decodes
//!   to RGBA8 or uses a listed unsupported format;
//! - **texture lists** (`texture-list.md` §2–§5, §9): every 0x40 record decodes exactly,
//!   and TL1–TL9 hold when the levels are resolved against the surface headers.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build decodes pixels and property sets for a sample only, unless
//! `TESSERA_FULL=1`. Run with `cargo test --release -p tessera-formats --test m1_real_data
//! -- --nocapture`. Only counts, DIDs and property IDs/types are printed; no game data leaves
//! the install.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::{
    Did, EntryContext, FormatError, PropertyDictionary, PropertySetRecord, PropertyValue, Record,
    Surface, SurfaceFormat, TextureList, TextureListKind,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

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

/// Whether every record is visited (release builds, or `TESSERA_FULL=1`).
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

/// Every (member index, entry) whose DID lies in `lo..=hi`, over all archives.
fn entries_in(set: &ArchiveSet, lo: u32, hi: u32) -> Vec<(usize, Entry)> {
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        out.extend(
            m.archive
                .entries_range(lo..=hi)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    out
}

// ------------------------------------------------------------------ property sets

#[derive(Default)]
struct SetStats {
    values: usize,
    depth: BTreeMap<usize, usize>,
    unordered_top: usize,
    unordered_structs: usize,
    unlisted_struct_fields: usize,
    unlisted_array_elements: usize,
    by_type: BTreeMap<u32, usize>,
}

enum SetOutcome {
    Clean(SetStats),
    /// Trailing bytes and the type code of the last top-level entry, if any.
    Trailing(u32, usize, Option<u32>),
    Failed(u32, String, Option<u32>),
}

fn check_set(rec: &PropertySetRecord, dict: &PropertyDictionary) -> SetStats {
    let mut st = SetStats::default();
    if rec.set.is_bucket_ordered() == Some(false) {
        st.unordered_top += 1;
    }
    let children = |pid: u32| dict.definition(pid).map(|d| d.children.as_slice());
    rec.set.walk(|v| {
        st.values += 1;
        *st.depth.entry(v.depth).or_default() += 1;
        *st.by_type
            .entry(v.entry.value.property_type().code())
            .or_default() += 1;
        if let PropertyValue::Struct(s) = &v.entry.value
            && s.is_bucket_ordered() == Some(false)
        {
            st.unordered_structs += 1;
        }
        match v.context {
            EntryContext::StructField { parent } => {
                if let Some(c) = children(parent)
                    && !c.is_empty()
                    && !c.contains(&v.entry.pid)
                {
                    st.unlisted_struct_fields += 1;
                }
            }
            EntryContext::ArrayElement { parent } => {
                if let Some(c) = children(parent)
                    && !c.is_empty()
                    && !c.contains(&v.entry.pid)
                {
                    st.unlisted_array_elements += 1;
                }
            }
            EntryContext::TopLevel => {}
        }
    });
    st
}

#[test]
fn property_sets_decode_cleanly() {
    let Some(set) = open_set() else { return };
    let t = Instant::now();
    let payload = set
        .read(PROPERTY_DICTIONARY)
        .expect("read the property dictionary through routing");
    let dict = PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY)
        .expect("decode the property dictionary");
    let all = entries_in(set, 0x7800_0000, 0x7FFF_FFFF);
    let mut per_prefix: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in &all {
        *per_prefix
            .entry(format!(
                "{} {:#04x}",
                set.member(*i).id,
                Did(e.did).prefix()
            ))
            .or_default() += 1;
    }
    let full = exhaustive();
    let work: Vec<_> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| full || i % 37 == 0)
        .map(|(_, w)| w)
        .collect();
    let outcomes = par_each(&work, |&&(i, ref e)| {
        let payload = match set.member(i).archive.read_entry(e) {
            Ok(p) => p,
            Err(err) => return SetOutcome::Failed(e.did, format!("read: {err}"), None),
        };
        match PropertySetRecord::parse_prefix(&payload, &dict) {
            Ok((rec, 0)) if rec.did.0 == e.did => SetOutcome::Clean(check_set(&rec, &dict)),
            Ok((rec, 0)) => {
                SetOutcome::Failed(e.did, format!("self DID {:#010x}", rec.did.0), None)
            }
            Ok((rec, n)) => SetOutcome::Trailing(
                e.did,
                n,
                rec.set
                    .entries
                    .last()
                    .map(|l| l.value.property_type().code()),
            ),
            Err(err) => {
                let ty = match &err {
                    FormatError::PropertyValue { type_code, .. } => Some(*type_code),
                    _ => None,
                };
                SetOutcome::Failed(e.did, err.to_string(), ty)
            }
        }
    });

    let mut total = SetStats::default();
    let mut clean = 0usize;
    let mut trailing: Vec<(u32, usize, Option<u32>)> = Vec::new();
    let mut failed: Vec<(u32, String, Option<u32>)> = Vec::new();
    for o in outcomes {
        match o {
            SetOutcome::Clean(s) => {
                clean += 1;
                total.values += s.values;
                total.unordered_top += s.unordered_top;
                total.unordered_structs += s.unordered_structs;
                total.unlisted_struct_fields += s.unlisted_struct_fields;
                total.unlisted_array_elements += s.unlisted_array_elements;
                for (k, v) in s.depth {
                    *total.depth.entry(k).or_default() += v;
                }
                for (k, v) in s.by_type {
                    *total.by_type.entry(k).or_default() += v;
                }
            }
            SetOutcome::Trailing(d, n, ty) => trailing.push((d, n, ty)),
            SetOutcome::Failed(d, m, ty) => failed.push((d, m, ty)),
        }
    }
    let attempted = work.len();
    let mut trailing_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut responsible: BTreeMap<String, usize> = BTreeMap::new();
    for (_, n, ty) in &trailing {
        *trailing_hist.entry(*n).or_default() += 1;
        *responsible
            .entry(format!("trailing after type {ty:?}"))
            .or_default() += 1;
    }
    for (_, _, ty) in &failed {
        *responsible
            .entry(format!("failed in type {ty:?}"))
            .or_default() += 1;
    }
    for (d, n, ty) in trailing.iter().take(10) {
        eprintln!("  {d:#010x}: {n} trailing bytes (last top-level type {ty:?})");
    }
    for (d, m, _) in failed.iter().take(10) {
        eprintln!("  {d:#010x}: {m}");
    }
    eprintln!("property-set records per archive/prefix: {per_prefix:?}");
    eprintln!("values by type code: {:?}", total.by_type);
    eprintln!("depth histogram: {:?}", total.depth);
    eprintln!("trailing-byte histogram (bytes -> records): {trailing_hist:?}");
    eprintln!("failures by responsible type: {responsible:?}");
    let rate = if attempted == 0 {
        0.0
    } else {
        clean as f64 * 100.0 / attempted as f64
    };
    eprintln!(
        "M1 criterion 3: {clean} of {attempted} property-set records clean ({rate:.3} %; {}), \
         {} with trailing bytes, {} failed; {} values; PS-O1 violations {}, PS-O2 {}, \
         PS-C1 {}, PS-C2 {}; {:?}",
        if full { "every record" } else { "debug sample" },
        trailing.len(),
        failed.len(),
        total.values,
        total.unordered_top,
        total.unordered_structs,
        total.unlisted_struct_fields,
        total.unlisted_array_elements,
        t.elapsed()
    );
    assert!(attempted > 0, "no property-set records found");
    assert!(rate >= 99.5, "clean rate {rate:.3} % is below 99.5 %");
    assert_eq!(total.unordered_top, 0, "PS-O1");
    assert_eq!(total.unordered_structs, 0, "PS-O2");
    assert_eq!(total.unlisted_struct_fields, 0, "PS-C1");
    assert_eq!(total.unlisted_array_elements, 0, "PS-C2");
}

#[test]
fn object_property_set_partners() {
    let Some(set) = open_set() else { return };
    let objects: Vec<u32> = entries_in(set, 0x7000_0000, 0x77FF_FFFF)
        .into_iter()
        .map(|(_, e)| e.did)
        .collect();
    let sets: HashSet<u32> = entries_in(set, 0x7800_0000, 0x7FFF_FFFF)
        .into_iter()
        .map(|(_, e)| e.did)
        .collect();
    let mut by_prefix: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    for &d in &objects {
        let target = tessera_formats::object_property_set_did(d).expect("game-object prefix");
        let slot = by_prefix.entry(Did(d).prefix()).or_default();
        slot.0 += 1;
        if sets.contains(&target.0) {
            slot.1 += 1;
        }
    }
    let object_set: HashSet<u32> = objects.iter().copied().collect();
    let orphans = sets
        .iter()
        .filter(|&&d| Did(d).prefix() == 0x79 && !object_set.contains(&(d - 0x0900_0000)))
        .count();
    eprintln!(
        "objects per prefix (count, with property set): {by_prefix:x?}; 0x79 sets without a \
         0x70 object: {orphans}"
    );
    if let Some(&(n, with)) = by_prefix.get(&0x70) {
        assert_eq!(n, with, "every 0x70 object has a 0x79 partner (§6.2)");
    }
    assert_eq!(orphans, 0, "every 0x79 set has a 0x70 partner (§6.2)");
}

// ------------------------------------------------------------------ surfaces

#[derive(Debug, Clone, Copy)]
struct SurfaceInfo {
    code: u32,
    category: u32,
    width: u32,
    height: u32,
    highres: bool,
}

struct SurfaceCensus {
    info: HashMap<u32, SurfaceInfo>,
    pixels_decoded: usize,
    unsupported: BTreeMap<u32, usize>,
    errors: Vec<String>,
    per_format: BTreeMap<String, usize>,
}

/// Reads every 0x41 header once (shared by the surface and texture-list tests) and decodes
/// the pixels of every surface (release) or a sample (debug).
fn surface_census(set: &'static ArchiveSet) -> &'static SurfaceCensus {
    static CENSUS: OnceLock<SurfaceCensus> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let t = Instant::now();
        let work = entries_in(set, 0x4100_0000, 0x41FF_FFFF);
        let full = exhaustive();
        let results = par_each(&work, |(i, e)| {
            let m = set.member(*i);
            let payload = m
                .archive
                .read_entry(e)
                .map_err(|err| (None, format!("{} {:#010x}: read: {err}", m.id, e.did)))?;
            let s = match Surface::parse_expecting(&payload, e.did) {
                Ok(s) => s,
                Err(FormatError::UnsupportedSurfaceFormat { code }) => {
                    return Err((Some(code), String::new()));
                }
                Err(err) => return Err((None, format!("{} {:#010x}: {err}", m.id, e.did))),
            };
            let decode = full || e.did % 97 == 0;
            if decode {
                let img = s
                    .to_rgba8()
                    .map_err(|err| (None, format!("{} {:#010x}: pixels: {err}", m.id, e.did)))?;
                if img.pixels.len() as u64 != u64::from(img.width) * u64::from(img.height) * 4 {
                    return Err((None, format!("{:#010x}: RGBA length mismatch", e.did)));
                }
            }
            Ok((
                e.did,
                SurfaceInfo {
                    code: s.format.code(),
                    category: s.category,
                    width: s.width,
                    height: s.height,
                    highres: m.id.kind == ArchiveKind::HighRes,
                },
                decode,
            ))
        });
        let mut c = SurfaceCensus {
            info: HashMap::with_capacity(results.len()),
            pixels_decoded: 0,
            unsupported: BTreeMap::new(),
            errors: Vec::new(),
            per_format: BTreeMap::new(),
        };
        for r in results {
            match r {
                Ok((did, info, decoded)) => {
                    c.pixels_decoded += usize::from(decoded);
                    let name = SurfaceFormat::from_code(info.code).map_or("?", |f| f.name());
                    *c.per_format.entry(name.to_owned()).or_default() += 1;
                    c.info.insert(did, info);
                }
                Err((Some(code), _)) => *c.unsupported.entry(code).or_default() += 1,
                Err((None, msg)) => c.errors.push(msg),
            }
        }
        eprintln!(
            "surface census: {} records in {:?}",
            work.len(),
            t.elapsed()
        );
        c
    })
}

#[test]
fn every_surface_decodes_or_is_listed_unsupported() {
    let Some(set) = open_set() else { return };
    let c = surface_census(set);
    for msg in c.errors.iter().take(20) {
        eprintln!("  {msg}");
    }
    eprintln!("surfaces per format: {:?}", c.per_format);
    eprintln!(
        "M1 criterion 1: {} surfaces decoded (headers), {} decoded to RGBA8 ({}), unsupported \
         format codes {:x?}, {} errors",
        c.info.len(),
        c.pixels_decoded,
        if exhaustive() {
            "every record"
        } else {
            "debug sample"
        },
        c.unsupported,
        c.errors.len()
    );
    assert!(!c.info.is_empty());
    assert!(c.errors.is_empty(), "surfaces that fail to decode");
    // texture-list.md §7.1: no code outside records.md §5.2 occurs in this install family.
    // A new code is not corruption; it must be listed and specified before M1 closes.
    assert!(
        c.unsupported.is_empty(),
        "unlisted surface format codes {:x?}: add them to texture-list.md §7",
        c.unsupported
    );
}

// ------------------------------------------------------------------ texture lists

fn is_dxt(code: u32) -> bool {
    matches!(
        SurfaceFormat::from_code(code),
        Some(SurfaceFormat::Dxt1 | SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5)
    )
}

#[test]
fn texture_lists_decode_and_levels_resolve() {
    let Some(set) = open_set() else { return };
    let t = Instant::now();
    let work = entries_in(set, 0x4000_0000, 0x40FF_FFFF);
    let results = par_each(&work, |(i, e)| {
        let m = set.member(*i);
        let payload = m
            .archive
            .read_entry(e)
            .map_err(|err| format!("{} {:#010x}: read: {err}", m.id, e.did))?;
        TextureList::parse_expecting(&payload, e.did)
            .map_err(|err| format!("{} {:#010x}: {err}", m.id, e.did))
    });
    let mut lists = Vec::new();
    let mut errors = Vec::new();
    for r in results {
        match r {
            Ok(l) => lists.push(l),
            Err(m) => errors.push(m),
        }
    }
    let census = surface_census(set);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut unusual = 0usize;
    // Violation counters per invariant.
    let mut v: BTreeMap<&str, usize> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut flag = |v: &mut BTreeMap<&str, usize>, k: &'static str, did: Did| {
        *v.entry(k).or_default() += 1;
        if examples.len() < 20 {
            examples.push(format!("{k}: {did}"));
        }
    };
    for l in &lists {
        *kinds.entry(format!("{:?}", l.kind)).or_default() += 1;
        if l.has_unusual_fields() {
            unusual += 1;
        }
        // TL4 is enforced by the decoder (length check).
        let levels: Vec<Option<&SurfaceInfo>> = l
            .surfaces
            .iter()
            .map(|d| {
                if d.prefix() != 0x41 {
                    None
                } else {
                    census.info.get(&d.0)
                }
            })
            .collect();
        if levels.iter().any(Option::is_none) {
            flag(&mut v, "TL5 unresolved level", l.did);
            continue;
        }
        let levels: Vec<&SurfaceInfo> = levels.into_iter().flatten().collect();
        let first = levels[0];
        if levels
            .iter()
            .any(|s| s.code != first.code || s.category != first.category)
        {
            flag(&mut v, "TL6 mixed format/category", l.did);
        }
        if l.kind == TextureListKind::MipChain {
            if levels
                .windows(2)
                .any(|w| w[1].width > w[0].width || w[1].height > w[0].height)
            {
                flag(&mut v, "TL7 growing level", l.did);
            }
            if levels.iter().skip(1).any(|s| s.highres) {
                flag(&mut v, "TL8 highres beyond level 0", l.did);
            }
        } else if levels.iter().any(|s| s.highres) {
            flag(&mut v, "TL8 highres cube face", l.did);
        }
        if levels
            .iter()
            .any(|s| is_dxt(s.code) && (s.width < 4 || s.height < 4))
        {
            flag(&mut v, "TL9 DXT level below 4", l.did);
        }
    }
    for m in errors.iter().chain(&examples).take(20) {
        eprintln!("  {m}");
    }
    let with_highres = lists
        .iter()
        .filter(|l| {
            census
                .info
                .get(&l.surfaces.first().map_or(0, |d| d.0))
                .is_some_and(|s| s.highres)
        })
        .count();
    eprintln!(
        "texture lists: {} decoded, {} errors, kinds {kinds:?}, {unusual} with unobserved field \
         values (TL2/TL3), {with_highres} with a highres level 0; invariant violations {v:?}; \
         {:?}",
        lists.len(),
        errors.len(),
        t.elapsed()
    );
    assert!(!lists.is_empty());
    assert!(
        errors.is_empty(),
        "texture lists that fail to decode (TL1, TL4)"
    );
    assert!(v.is_empty(), "texture-list invariant violations: {v:?}");
}
