// SPDX-License-Identifier: GPL-3.0-or-later
//! `landblock.md` / `entity-descriptor.md` decode rates against a user's own install
//! (read-only).
//!
//! Every landblock record (kinds 0, 1, 2, 4, 5) of every `client_cell_R` / `client_map_R`
//! archive is decoded with its directory stamp; failures are counted per
//! [`FailureCategory`] (spec §9) and records that decode but break an observed invariant
//! are counted as "unusual". Cross-record invariants are checked too:
//!
//! - LB-E1 (kinds 0 and 1 dense over 0–254), LB-E3 (info exists iff the data header names
//!   it), LB-E4 (kind 5 only at multiples of 3);
//! - HM7 (edge samples equal the neighbour's) over every pair of non-constant blocks;
//! - L9 (ceiling present iff the block map resolves `Landblock_UseCeiling` = 1);
//! - BM2 (identical key set per region), ED4 (object prefixes), ED6 (scale), ED7, LI4
//!   (cell geometry DIDs exist), minimap georeference (§6.2).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a 1-in-23 sample unless `TESSERA_FULL=1` (cross-record
//! checks then run on the sample only where meaningful). Run with
//! `cargo test --release -p tessera-formats --test landblock_real_data -- --nocapture`.
//! Only counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::landblock::{
    FailureCategory, GRID_SIDE, LandblockPids, LayoutRule, StaticEntityType, vertex_xy,
};
use tessera_formats::{
    BlockMap, BlockMapDescriptor, Did, LandblockData, LandblockGroupRecord, LandblockInfo,
    LandblockProperties, PropertyDictionary, PropertyValue, Record, RegionDescriptor,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
/// Largest share of records allowed to deviate from an observed invariant (0.1 %).
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

fn dictionary(set: &ArchiveSet) -> &'static PropertyDictionary {
    static DICT: OnceLock<PropertyDictionary> = OnceLock::new();
    DICT.get_or_init(|| {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    })
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

/// (member index, entry) of every landblock record of kind `k` in region archives of
/// `kind` (cell or map), per region.
fn landblock_entries(
    set: &ArchiveSet,
    k: u32,
    map_archive: bool,
) -> BTreeMap<u32, Vec<(usize, Entry)>> {
    let mut out: BTreeMap<u32, Vec<(usize, Entry)>> = BTreeMap::new();
    for (i, m) in set.members().iter().enumerate() {
        let region = match (&m.id.kind, map_archive) {
            (ArchiveKind::Cell(r), false) | (ArchiveKind::Map(r), true) => *r,
            _ => continue,
        };
        let lo = 0x8000_0000 | (k << 20);
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            let e = e.expect("directory walk");
            out.entry(region).or_default().push((i, e));
        }
    }
    out
}

fn sample<T: Clone>(v: &[T]) -> Vec<T> {
    let full = exhaustive();
    v.iter()
        .enumerate()
        .filter(|(i, _)| full || i % 23 == 0)
        .map(|(_, e)| e.clone())
        .collect()
}

/// Census of one record kind in one region.
#[derive(Default)]
struct Census {
    total: usize,
    clean: usize,
    failed: BTreeMap<String, (usize, u32)>,
    unusual: BTreeMap<&'static str, (usize, u32)>,
    stamp_rule: BTreeMap<String, usize>,
}

impl Census {
    fn fail(&mut self, did: u32, cat: FailureCategory, detail: String) {
        let key = format!(
            "{}: {}",
            cat.name(),
            detail.chars().take(80).collect::<String>()
        );
        self.failed.entry(key).or_insert((0, did)).0 += 1;
    }

    fn unusual(&mut self, did: u32, why: &'static str) {
        self.unusual.entry(why).or_insert((0, did)).0 += 1;
    }

    fn report(&self, what: &str) -> (usize, usize) {
        let bad: usize = self.failed.values().map(|v| v.0).sum();
        let odd: usize = self.unusual.values().map(|v| v.0).sum();
        #[allow(clippy::cast_precision_loss)]
        let rate = if self.total == 0 {
            100.0
        } else {
            100.0 * (self.total - bad) as f64 / self.total as f64
        };
        eprintln!(
            "{what}: {} records, {} decoded ({rate:.3} %), {} clean, {bad} failed, {odd} unusual",
            self.total,
            self.total - bad,
            self.clean,
        );
        for (why, (n, did)) in &self.failed {
            eprintln!("    failed: {n} × {why} (e.g. {did:#010x})");
        }
        for (why, (n, did)) in &self.unusual {
            eprintln!("    unusual: {n} × {why} (e.g. {did:#010x})");
        }
        for (rule, n) in &self.stamp_rule {
            eprintln!("    layout rule {rule}: {n}");
        }
        (bad, odd)
    }

    fn merge(&mut self, o: Census) {
        self.total += o.total;
        self.clean += o.clean;
        for (k, (n, d)) in o.failed {
            self.failed.entry(k).or_insert((0, d)).0 += n;
        }
        for (k, (n, d)) in o.unusual {
            self.unusual.entry(k).or_insert((0, d)).0 += n;
        }
        for (k, n) in o.stamp_rule {
            *self.stamp_rule.entry(k).or_default() += n;
        }
    }
}

enum Outcome<T> {
    Clean(u32, T),
    Unusual(u32, &'static str, T),
    Failed(u32, FailureCategory, String),
}

fn tally<T>(outcomes: Vec<Outcome<T>>, census: &mut Census) -> Vec<(u32, T)> {
    let mut kept = Vec::new();
    for o in outcomes {
        census.total += 1;
        match o {
            Outcome::Clean(did, t) => {
                census.clean += 1;
                kept.push((did, t));
            }
            Outcome::Unusual(did, why, t) => {
                census.unusual(did, why);
                kept.push((did, t));
            }
            Outcome::Failed(did, cat, detail) => census.fail(did, cat, detail),
        }
    }
    kept
}

fn check(census: &Census, what: &str) {
    let (bad, odd) = census.report(what);
    assert!(census.total > 0, "no {what} records found");
    assert_eq!(bad, 0, "{what}: {bad} of {} failed", census.total);
    #[allow(clippy::cast_precision_loss)]
    let limit = MAX_UNUSUAL_RATE * census.total as f64;
    #[allow(clippy::cast_precision_loss)]
    let odd_f = odd as f64;
    assert!(odd_f <= limit, "{what}: {odd} unusual of {}", census.total);
}

fn rule_name(r: LayoutRule) -> String {
    format!("{r:?}")
}

/// What the data pass keeps per landblock for cross-record checks.
struct DataSummary {
    x: u8,
    y: u8,
    constant: bool,
    /// Edges: i = 0 column, i = 32 column, j = 0 row, j = 32 row (33 samples each).
    edges: Box<[[u16; GRID_SIDE]; 4]>,
    rule: LayoutRule,
    /// Ceiling-layer triangles: (uses the flagged diagonal, uses the other, CCW from above).
    ceiling_tris: [usize; 3],
}

/// Census of ceiling triangles against the terrain's vertex-flag diagonals (§3.5 OPEN).
fn ceiling_triangle_census(d: &LandblockData) -> [usize; 3] {
    let mut out = [0usize; 3];
    let Some(c) = &d.ceiling else { return out };
    for l in &c.surface.layers {
        for t in c.surface.layer_indices(l).unwrap_or(&[]).chunks_exact(3) {
            let p: Vec<(usize, usize)> = t.iter().map(|&v| vertex_xy(v).unwrap()).collect();
            let i = p.iter().map(|q| q.0).min().unwrap();
            let j = p.iter().map(|q| q.1).min().unwrap();
            let sw_ne = p.contains(&(i, j)) && p.contains(&(i + 1, j + 1));
            if sw_ne == d.vertex_flags.split_sw_ne(i, j) {
                out[0] += 1;
            } else {
                out[1] += 1;
            }
            let s = |v: usize| v as i64;
            let cross = (s(p[1].0) - s(p[0].0)) * (s(p[2].1) - s(p[0].1))
                - (s(p[1].1) - s(p[0].1)) * (s(p[2].0) - s(p[0].0));
            out[2] += usize::from(cross > 0);
        }
    }
    out
}

fn summarise(d: &LandblockData) -> DataSummary {
    let lb = d.landblock().expect("landblock DID");
    let mut edges = Box::new([[0u16; GRID_SIDE]; 4]);
    for k in 0..GRID_SIDE {
        edges[0][k] = d.heights.get(0, k).unwrap();
        edges[1][k] = d.heights.get(32, k).unwrap();
        edges[2][k] = d.heights.get(k, 0).unwrap();
        edges[3][k] = d.heights.get(k, 32).unwrap();
    }
    DataSummary {
        x: lb.x,
        y: lb.y,
        constant: d.is_constant_height(),
        edges,
        rule: d.layout_rule,
        ceiling_tris: ceiling_triangle_census(d),
    }
}

#[test]
fn landblock_data_decodes() {
    let Some(set) = open_set() else { return };
    let info_dids: HashSet<u32> = landblock_entries(set, 2, false)
        .values()
        .flatten()
        .map(|(_, e)| e.did)
        .collect();
    let mut all = Census::default();
    let mut seam_pairs = 0usize;
    let mut seam_bad = 0usize;
    let mut ceiling_tris = [0usize; 3];
    for (region, entries) in landblock_entries(set, 0, false) {
        if exhaustive() {
            assert_eq!(entries.len(), 255 * 255, "LB-E1: region {region} not dense");
        }
        let work = sample(&entries);
        let outcomes = par_each(&work, |(i, e)| {
            let payload = match set.member(*i).archive.read_entry(e) {
                Ok(p) => p,
                Err(err) => {
                    return Outcome::Failed(
                        e.did,
                        FailureCategory::Malformed,
                        format!("read: {err}"),
                    );
                }
            };
            match LandblockData::parse_expecting(&payload, e.did, Some(e.stamp)) {
                Ok(d) => {
                    let s = summarise(&d);
                    // Fallback agrees with the stamp rule.
                    let fallback = LandblockData::parse(&payload, None);
                    if fallback.as_ref().ok()
                        != Some(&LandblockData {
                            layout_rule: LayoutRule::Fallback,
                            ..d.clone()
                        })
                        && fallback.as_ref().ok() != Some(&d)
                    {
                        return Outcome::Unusual(e.did, "stampless fallback disagrees", s);
                    }
                    if d.to_bytes() != payload {
                        return Outcome::Unusual(e.did, "re-encoding differs", s);
                    }
                    let info_ok = match d.info {
                        Some(i) => info_dids.contains(&i.0),
                        None => !info_dids.contains(&(e.did + 0x0020_0000)),
                    };
                    if !info_ok {
                        return Outcome::Unusual(
                            e.did,
                            "info DID / info record mismatch (LB-E3)",
                            s,
                        );
                    }
                    match d.first_unusual_field() {
                        None => Outcome::Clean(e.did, s),
                        Some(why) => Outcome::Unusual(e.did, why, s),
                    }
                }
                Err(err) => Outcome::Failed(e.did, FailureCategory::of(&err), err.to_string()),
            }
        });
        let mut census = Census::default();
        let kept = tally(outcomes, &mut census);
        for (_, s) in &kept {
            *census.stamp_rule.entry(rule_name(s.rule)).or_default() += 1;
        }
        for (_, s) in &kept {
            for (total, n) in ceiling_tris.iter_mut().zip(s.ceiling_tris) {
                *total += n;
            }
        }
        let by_xy: HashMap<(u8, u8), &DataSummary> =
            kept.iter().map(|(_, s)| ((s.x, s.y), s)).collect();
        for s in by_xy.values() {
            if s.constant {
                continue;
            }
            if let Some(n) = s.x.checked_add(1).and_then(|x| by_xy.get(&(x, s.y)))
                && !n.constant
            {
                seam_pairs += 1;
                seam_bad += usize::from(s.edges[1] != n.edges[0]);
            }
            if let Some(n) = s.y.checked_add(1).and_then(|y| by_xy.get(&(s.x, y)))
                && !n.constant
            {
                seam_pairs += 1;
                seam_bad += usize::from(s.edges[3] != n.edges[2]);
            }
        }
        census.report(&format!("landblock data, region {region}"));
        all.merge(census);
    }
    eprintln!("HM7 seams: {seam_bad} of {seam_pairs} neighbour pairs differ");
    eprintln!(
        "ceiling triangles: {} use the flagged diagonal, {} the other; {} counter-clockwise from above",
        ceiling_tris[0], ceiling_tris[1], ceiling_tris[2]
    );
    check(&all, "landblock data (all regions)");
    assert_eq!(seam_bad, 0, "HM7 violated");
    assert!(
        seam_pairs > 0 || !exhaustive(),
        "no neighbour pairs checked"
    );
}

/// Offset of `has_ceiling` in a landblock-data payload: header + height grid (§3.1).
const HAS_CEILING_OFFSET: usize = 16 + 2 * 1089;

/// Region number → block-map descriptor DID, from the region descriptors (§5.1).
fn region_descriptors(set: &ArchiveSet) -> BTreeMap<u32, RegionDescriptor> {
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x0F00_0000..=0x0FFF_FFFF) {
            let e = e.expect("directory walk");
            let payload = m.archive.read_entry(&e).expect("read region descriptor");
            let d = RegionDescriptor::parse_expecting(&payload, e.did).expect("region descriptor");
            assert_eq!(d.first_unusual_field(), None, "{:#010x}", e.did);
            out.insert(d.region, d);
        }
    }
    out
}

#[test]
fn block_maps_and_descriptors_decode() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let pids = LandblockPids::from_dictionary(dict);
    let regions = region_descriptors(set);
    eprintln!("region descriptors: {}", regions.len());
    assert!(!regions.is_empty());
    let mut descriptors: BTreeMap<u32, BlockMapDescriptor> = BTreeMap::new();
    for d in regions.values() {
        let did = d.block_map_descriptor().expect("slot with a 0x18 DID");
        if descriptors.contains_key(&did.0) {
            continue;
        }
        let Ok(payload) = set.read(did.0) else {
            eprintln!(
                "  block-map descriptor {did} of region {} missing",
                d.region
            );
            continue;
        };
        let desc = BlockMapDescriptor::parse_expecting(&payload, did, dict).expect("descriptor");
        assert_eq!(desc.first_unusual_field(), None, "{did}");
        eprintln!(
            "  descriptor {did}: {} entries, palette sizes {:?}",
            desc.entries.len(),
            desc.entries
                .iter()
                .map(|e| e.values.len())
                .collect::<Vec<_>>()
        );
        descriptors.insert(did.0, desc);
    }
    let mut all = Census::default();
    for (region, entries) in landblock_entries(set, 1, true) {
        if exhaustive() {
            assert_eq!(
                entries.len(),
                255 * 255,
                "LB-E1: region {region} block maps"
            );
        }
        let desc = regions
            .get(&region)
            .and_then(|r| r.block_map_descriptor())
            .and_then(|d| descriptors.get(&d.0));
        let work = sample(&entries);
        let outcomes = par_each(&work, |(i, e)| {
            let payload = match set.member(*i).archive.read_entry(e) {
                Ok(p) => p,
                Err(err) => {
                    return Outcome::Failed(
                        e.did,
                        FailureCategory::Malformed,
                        format!("read: {err}"),
                    );
                }
            };
            match BlockMap::parse_expecting(&payload, e.did) {
                Ok(m) => {
                    let keys: BTreeSet<u32> = m.entries.iter().map(|e| e.key).collect();
                    let out_of_range = desc.map_or(0, |d| {
                        d.resolve(&m)
                            .iter()
                            .filter(|v| v.pid.is_some() && v.value.is_none())
                            .count()
                    });
                    let ceiling = desc
                        .zip(pids.landblock_use_ceiling)
                        .and_then(|(d, p)| d.value_of(&m, p))
                        .map(|v| !matches!(v, PropertyValue::Int(0) | PropertyValue::Bool(false)));
                    // L9: compare with the landblock data's `has_ceiling` byte.
                    let has_ceiling = set
                        .read(e.did - 0x0010_0000)
                        .ok()
                        .and_then(|d| d.get(HAS_CEILING_OFFSET).copied())
                        .map(|b| b == 1);
                    let t = (keys, out_of_range, ceiling, has_ceiling);
                    if m.to_bytes() != payload {
                        return Outcome::Unusual(e.did, "re-encoding differs", t);
                    }
                    match m.first_unusual_field() {
                        None => Outcome::Clean(e.did, t),
                        Some(why) => Outcome::Unusual(e.did, why, t),
                    }
                }
                Err(err) => Outcome::Failed(e.did, FailureCategory::of(&err), err.to_string()),
            }
        });
        let mut census = Census::default();
        let kept = tally(outcomes, &mut census);
        let key_sets: BTreeSet<&BTreeSet<u32>> = kept.iter().map(|(_, t)| &t.0).collect();
        if key_sets.len() != 1 {
            census.unusual(0, "key set differs between records (BM2)");
        }
        let oor: usize = kept.iter().map(|(_, t)| t.1).sum();
        let mut ceiling_disagree = 0usize;
        let mut use_ceiling = 0usize;
        for (_, (_, _, c, has)) in &kept {
            use_ceiling += usize::from(*c == Some(true));
            if c.is_none() || has.is_none() || *c != *has {
                ceiling_disagree += 1;
            }
        }
        census.report(&format!("block map, region {region}"));
        eprintln!(
            "    keys per record {:?}; out-of-range indices {oor}; UseCeiling = 1 in {use_ceiling}; \
             ceiling/UseCeiling disagreements {ceiling_disagree}",
            key_sets.iter().map(|k| k.len()).collect::<Vec<_>>(),
        );
        assert_eq!(ceiling_disagree, 0, "L9 violated in region {region}");
        all.merge(census);
    }
    check(&all, "block maps (all regions)");
}

#[derive(Default)]
struct InfoStats {
    cells: usize,
    entities: usize,
    links: usize,
    weenies: usize,
    by_type: BTreeMap<String, usize>,
    bad_object_prefix: usize,
    bad_scale: usize,
    outside_square: usize,
    outside_without_interior: usize,
    geometry: HashSet<u32>,
    /// (info DID, cell index, Position flag byte) of cells whose position has no offset.
    cells_without_offset: Vec<(u32, u16, u8)>,
    /// Cells with an orientation (Position flag 0x20).
    cells_with_rotation: usize,
    /// SEDs whose stored rotation is more than 0.001 from unit length (ED5).
    off_unit: usize,
}

#[test]
fn landblock_info_decodes() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let pids = LandblockPids::from_dictionary(dict);
    let (rs, ps) = (
        pids.render_adjustable_scale
            .expect("Render_AdjustableScale"),
        pids.physics_adjustable_scale
            .expect("Physics_AdjustableScale"),
    );
    let mut all = Census::default();
    let mut stats = InfoStats::default();
    for (region, entries) in landblock_entries(set, 2, false) {
        let work = sample(&entries);
        let outcomes = par_each(&work, |(i, e)| {
            let payload = match set.member(*i).archive.read_entry(e) {
                Ok(p) => p,
                Err(err) => {
                    return Outcome::Failed(
                        e.did,
                        FailureCategory::Malformed,
                        format!("read: {err}"),
                    );
                }
            };
            match LandblockInfo::parse_expecting(&payload, e.did, Some(e.stamp), dict) {
                Ok(info) => {
                    let mut s = InfoStats {
                        cells: info.cells().len(),
                        entities: info.entities.len(),
                        links: info.links.len(),
                        weenies: info.weenies.len(),
                        ..InfoStats::default()
                    };
                    for c in info.cells() {
                        s.geometry.insert(c.geometry.0);
                        if c.position.offset().is_none() {
                            s.cells_without_offset
                                .push((e.did, c.index, c.position.flags()));
                        }
                        s.cells_with_rotation += usize::from(c.rotation().is_some());
                    }
                    for sed in &info.entities {
                        let kind = sed.kind().map_or("none".into(), |k| format!("{k:?}"));
                        *s.by_type.entry(kind).or_default() += 1;
                        let want = if sed.kind() == Some(StaticEntityType::Weenie) {
                            0x70
                        } else {
                            0x47
                        };
                        if sed.object.is_some_and(|o| o.prefix() != want) {
                            s.bad_object_prefix += 1;
                        }
                        let r = sed.property(rs);
                        let p = sed.property(ps);
                        let ok = matches!(r, Some(PropertyValue::Vector3(v))
                            if v[0] == v[1] && v[1] == v[2] && (0.01..=100.0).contains(&v[0]))
                            && r == p;
                        s.bad_scale += usize::from(!ok);
                        s.off_unit += usize::from(sed.placement.is_some_and(|p| p.is_off_unit()));
                        if let Some(pl) = sed.placement
                            && !pl.is_inside_block()
                        {
                            s.outside_square += 1;
                            s.outside_without_interior += usize::from(!sed.is_interior());
                        }
                    }
                    let trailer_fallback = LandblockInfo::parse(&payload, None, dict)
                        .map(|f| f.trailer == info.trailer)
                        .unwrap_or(false);
                    let t = (s, info.layout_rule);
                    if !trailer_fallback {
                        return Outcome::Unusual(e.did, "stampless trailer fallback disagrees", t);
                    }
                    match info.first_unusual_field() {
                        None => Outcome::Clean(e.did, t),
                        Some(why) => Outcome::Unusual(e.did, why, t),
                    }
                }
                Err(err) => Outcome::Failed(e.did, FailureCategory::of(&err), err.to_string()),
            }
        });
        let mut census = Census::default();
        for (_, (s, rule)) in tally(outcomes, &mut census) {
            *census.stamp_rule.entry(rule_name(rule)).or_default() += 1;
            stats.cells += s.cells;
            stats.entities += s.entities;
            stats.links += s.links;
            stats.weenies += s.weenies;
            for (k, n) in s.by_type {
                *stats.by_type.entry(k).or_default() += n;
            }
            stats.bad_object_prefix += s.bad_object_prefix;
            stats.bad_scale += s.bad_scale;
            stats.outside_square += s.outside_square;
            stats.outside_without_interior += s.outside_without_interior;
            stats.geometry.extend(s.geometry);
            stats.cells_without_offset.extend(s.cells_without_offset);
            stats.cells_with_rotation += s.cells_with_rotation;
            stats.off_unit += s.off_unit;
        }
        census.report(&format!("landblock info, region {region}"));
        all.merge(census);
    }
    eprintln!(
        "totals: {} cells, {} static entities, {} links, {} weenies",
        stats.cells, stats.entities, stats.links, stats.weenies
    );
    eprintln!("static entity types: {:?}", stats.by_type);
    eprintln!(
        "ED4 bad object prefixes {}; ED6 bad scales {}; ED7 outside square {} ({} without interior flag)",
        stats.bad_object_prefix,
        stats.bad_scale,
        stats.outside_square,
        stats.outside_without_interior
    );
    let missing_geometry = stats
        .geometry
        .iter()
        .filter(|&&g| set.locate(g).ok().flatten().is_none())
        .count();
    eprintln!(
        "LI4: {} distinct cell geometry DIDs, {missing_geometry} missing",
        stats.geometry.len()
    );
    stats.cells_without_offset.sort_unstable();
    eprintln!(
        "cells with orientation {}; cells without x, y, z offset {}: {:x?}; ED5 off-unit SEDs {}",
        stats.cells_with_rotation,
        stats.cells_without_offset.len(),
        stats.cells_without_offset,
        stats.off_unit
    );
    check(&all, "landblock info (all regions)");
    assert_eq!(stats.off_unit, 0, "ED5");
    assert_eq!(stats.bad_object_prefix, 0, "ED4");
    assert_eq!(stats.bad_scale, 0, "ED6");
    assert_eq!(missing_geometry, 0, "LI4");
}

#[test]
fn landblock_properties_and_groups_decode() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let pids = LandblockPids::from_dictionary(dict);
    let mut all = Census::default();
    let mut minimaps = 0usize;
    let mut variants = 0usize;
    let mut light_maps = 0usize;
    for (region, entries) in landblock_entries(set, 4, false) {
        let work = sample(&entries);
        let outcomes = par_each(&work, |(i, e)| {
            let payload = match set.member(*i).archive.read_entry(e) {
                Ok(p) => p,
                Err(err) => {
                    return Outcome::Failed(
                        e.did,
                        FailureCategory::Malformed,
                        format!("read: {err}"),
                    );
                }
            };
            match LandblockProperties::parse_expecting(&payload, e.did, dict) {
                Ok(p) => {
                    let lb = p.landblock().unwrap();
                    let tile = p.minimap(&pids);
                    let lights = usize::from(p.light_map(&pids).is_some())
                        + usize::from(p.ceiling_light_map(&pids).is_some());
                    let t = (
                        tile.is_some(),
                        tile.as_ref().map_or(0, |t| t.variants.len()),
                        lights,
                    );
                    if tile.is_none() && lights == 0 {
                        return Outcome::Unusual(e.did, "neither minimap nor light map", t);
                    }
                    if let Some(tile) = &tile {
                        if tile.block_offset != [i32::from(lb.x), i32::from(lb.y)] {
                            return Outcome::Unusual(e.did, "minimap block offset is not X, Y", t);
                        }
                        if tile.pixel_offset != [0, 200] || (tile.scale - 1.25).abs() > 1e-5 {
                            return Outcome::Unusual(e.did, "minimap pixel offset/scale differ", t);
                        }
                    }
                    Outcome::Clean(e.did, t)
                }
                Err(err) => Outcome::Failed(e.did, FailureCategory::of(&err), err.to_string()),
            }
        });
        let mut census = Census::default();
        for (_, (tile, v, l)) in tally(outcomes, &mut census) {
            minimaps += usize::from(tile);
            variants += v;
            light_maps += l;
        }
        census.report(&format!("landblock properties, region {region}"));
        all.merge(census);
    }
    eprintln!("minimap tiles {minimaps} ({variants} variants), light maps {light_maps}");
    check(&all, "landblock properties (all regions)");

    let mut groups = Census::default();
    for (region, entries) in landblock_entries(set, 5, false) {
        let work = sample(&entries);
        let outcomes = par_each(&work, |(i, e)| {
            let payload = match set.member(*i).archive.read_entry(e) {
                Ok(p) => p,
                Err(err) => {
                    return Outcome::Failed(
                        e.did,
                        FailureCategory::Malformed,
                        format!("read: {err}"),
                    );
                }
            };
            match LandblockGroupRecord::parse_expecting(&payload, e.did) {
                Ok(g) => match g.first_unusual_field() {
                    None => Outcome::Clean(e.did, ()),
                    Some(why) => Outcome::Unusual(e.did, why, ()),
                },
                Err(err) => Outcome::Failed(e.did, FailureCategory::of(&err), err.to_string()),
            }
        });
        let mut census = Census::default();
        tally(outcomes, &mut census);
        census.report(&format!("kind-5 group records, region {region}"));
        groups.merge(census);
    }
    check(&groups, "kind-5 group records (all regions)");
}

/// Placement sanity on a sample: Physics entities in landblocks without cells sit near the
/// terrain more often than not (entity-descriptor.md §8), with the §3.11 triangulation.
#[test]
fn entities_rest_on_terrain() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let entries: Vec<_> = landblock_entries(set, 2, false)
        .into_values()
        .flatten()
        .collect();
    let work: Vec<_> = entries
        .iter()
        .step_by(if exhaustive() { 7 } else { 97 })
        .collect();
    let results = par_each(&work, |(i, e)| {
        let archive = &set.member(*i).archive;
        let info = LandblockInfo::parse(&archive.read_entry(e).ok()?, Some(e.stamp), dict).ok()?;
        if !info.cells().is_empty() {
            return None;
        }
        let data_did: Did = Did(e.did - 0x0020_0000);
        let data = LandblockData::parse(&archive.read(data_did.0).ok()?, None).ok()?;
        let (mut near, mut n) = (0usize, 0usize);
        for sed in &info.entities {
            let Some(p) = sed.placement else { continue };
            if sed.kind() != Some(StaticEntityType::Physics) || !p.is_inside_block() {
                continue;
            }
            let h = data.terrain_height(p.position[0], p.position[1])?;
            n += 1;
            near += usize::from((p.position[2] - h).abs() <= 0.25);
        }
        Some((near, n))
    });
    let (near, n) = results
        .into_iter()
        .flatten()
        .fold((0, 0), |(a, b), (c, d)| (a + c, b + d));
    #[allow(clippy::cast_precision_loss)]
    let share = near as f64 / n.max(1) as f64;
    eprintln!(
        "outdoor Physics entities within 25 cm of terrain: {near} of {n} ({:.1} %)",
        100.0 * share
    );
    assert!(n > 0);
    assert!(share > 0.3, "placements do not follow the terrain");
}
