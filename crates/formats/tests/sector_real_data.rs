// SPDX-License-Identifier: GPL-3.0-or-later
//! `distant-landscape.md` census against a user's own install (read-only).
//!
//! Decodes **every** kind-5 record (`0x805RXXYY`) of every `client_cell_R` archive with
//! [`SectorRecord`] and checks the invariants of §7:
//!
//! - record-local DL-I1–I5, I7 (mode/mask), I8 (packing, zero chunks), I9, I10 (pair order
//!   and use), I11, I12, I13 (imposter table, DID types) through
//!   [`SectorRecord::deviations`];
//! - cross-record DL-I6 (terrain blocks equal the kind-0 height grids), DL-I7 (mask equals
//!   the landblocks' ceilings), DL-I8 (ceiling blocks equal the ceiling grids), DL-I10
//!   (`wet` and the named pair equal `water.md` §6.2 from the block maps and the region's
//!   water table) and DL-I13 (every named DID exists).
//!
//! DL-I14 (every terrain and ceiling normal within one step of the rule of §4.3, with the
//! 5 × 5 landblocks around each sector) and DL-I15 (no mask bit above 8) are checked too;
//! the imposter-key census (DL14) is not.
//!
//! DL-I6 and DL-I14 are asserted only in the regions of the spec's census (§2: 1–5 and 14,
//! [`CENSUS_REGIONS`]). The install patched on 2026-10-27 adds region 6, where a few
//! terrain blocks differ from their height grids and some normals lie more than one step
//! off; the spec does not yet say whether these two invariants hold outside its census, so
//! for other regions the counts are printed with an example DID and do not fail the test.
//! Every other invariant is asserted in every region.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a 1-in-23 sample unless `TESSERA_FULL=1`. Run with
//! `cargo test --release -p tessera-formats --test sector_real_data -- --nocapture`.
//! Only counts, DIDs and invariant names are printed; no game data leaves the install.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::landblock::sector_normals::SectorNormalCheck;
use tessera_formats::landblock::{
    FailureCategory, HeightGrid, REGION_DESCRIPTOR_PREFIX, SectorRecord, SectorSample,
    SectorWaterPair, block_sample_offset, height_from_raw, sprite_is_mirrored,
};
use tessera_formats::{
    BlockMap, LandblockData, PropertyDictionary, Record, RegionDescriptor, WaterTable,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

/// The regions whose sectors `distant-landscape.md` census covers (§2, §7: 33,159 records in
/// regions 1–5 and 14). DL-I6 and DL-I14 are asserted there and only reported elsewhere.
const CENSUS_REGIONS: [u32; 6] = [1, 2, 3, 4, 5, 14];

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

/// Runs `f` over `items` on up to eight threads; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(16, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 16).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// `water.md` §6.2 for one landblock: its (type, height) pair when it has water.
fn landblock_water(
    table: Option<&WaterTable>,
    map: &BlockMap,
    heights: &HeightGrid,
) -> Option<SectorWaterPair> {
    let table = table?;
    let t = map.get(table.type_key)?;
    let entry = table.select(u32::from(t))?;
    if entry.is_dry() {
        return None;
    }
    let level = map.get(table.level_key)?;
    let h = (f32::from(level) * 1000.0) * (1.0f32 / 255.0);
    if h <= height_from_raw(heights.min_max().0) {
        return None;
    }
    Some(SectorWaterPair {
        // Low 8 bits of the block-map value (distant-landscape §4.6).
        water_type: t as u8,
        height: h,
    })
}

fn same_pair(a: SectorWaterPair, b: SectorWaterPair) -> bool {
    a.water_type == b.water_type && a.height.to_bits() == b.height.to_bits()
}

/// Whether a 1,089-sample block (Y-major) equals a kind-0 grid (X-major) sample for sample.
fn block_equals(block: Option<&[SectorSample]>, g: &HeightGrid) -> bool {
    let Some(block) = block else { return false };
    (0..33).all(|i| {
        (0..33).all(|j| {
            block_sample_offset(i, j)
                .and_then(|o| block.get(o))
                .map(|s| s.height)
                == g.get(i, j)
        })
    })
}

#[derive(Default)]
struct Tally {
    records: usize,
    failed: BTreeMap<String, (usize, u32)>,
    deviations: BTreeMap<&'static str, (usize, u32)>,
    cross: BTreeMap<&'static str, (usize, u32)>,
    /// DL-I6 and DL-I14 findings outside [`CENSUS_REGIONS`]: reported, not asserted.
    reported: BTreeMap<&'static str, (usize, u32)>,
    stamps: BTreeMap<u16, usize>,
    modes: [usize; 3],
    ceiling_blocks: usize,
    pairs_per_record: BTreeMap<usize, usize>,
    wet_chunks: usize,
    wet_by_index: BTreeMap<u8, usize>,
    sprites: usize,
    sprite_records: usize,
    sprite_max: usize,
    distances: BTreeMap<u32, usize>,
    entities: usize,
    entity_records: usize,
    entity_max: usize,
    entity_types: BTreeMap<u32, usize>,
    position_flags: BTreeMap<u8, usize>,
    entity_properties: BTreeMap<u32, usize>,
    flow_maps: usize,
    landblocks_checked: usize,
    /// DL-I14: stored normals against the rule of §4.3, in [`CENSUS_REGIONS`].
    normals: SectorNormalCheck,
    /// DL-I14 outside [`CENSUS_REGIONS`] (reported, not asserted).
    other_normals: SectorNormalCheck,
    /// DL-I15: `mode` 2 masks with a bit above bit 8 (rejected by the decoder, so 0).
    high_mask_bits: usize,
    /// §5.6 step 4 census: sprites drawn mirrored for a viewer at the sector's centre
    /// landblock.
    sprites_mirrored: usize,
}

impl Tally {
    fn merge(&mut self, o: Tally) {
        fn add<K: Ord>(a: &mut BTreeMap<K, usize>, b: BTreeMap<K, usize>) {
            for (k, v) in b {
                *a.entry(k).or_default() += v;
            }
        }
        fn add_eg<K: Ord>(a: &mut BTreeMap<K, (usize, u32)>, b: BTreeMap<K, (usize, u32)>) {
            for (k, (n, d)) in b {
                a.entry(k).or_insert((0, d)).0 += n;
            }
        }
        self.records += o.records;
        add_eg(&mut self.failed, o.failed);
        add_eg(&mut self.deviations, o.deviations);
        add_eg(&mut self.cross, o.cross);
        add_eg(&mut self.reported, o.reported);
        add(&mut self.stamps, o.stamps);
        for (a, b) in self.modes.iter_mut().zip(o.modes) {
            *a += b;
        }
        self.ceiling_blocks += o.ceiling_blocks;
        add(&mut self.pairs_per_record, o.pairs_per_record);
        self.wet_chunks += o.wet_chunks;
        add(&mut self.wet_by_index, o.wet_by_index);
        self.sprites += o.sprites;
        self.sprite_records += o.sprite_records;
        self.sprite_max = self.sprite_max.max(o.sprite_max);
        add(&mut self.distances, o.distances);
        self.entities += o.entities;
        self.entity_records += o.entity_records;
        self.entity_max = self.entity_max.max(o.entity_max);
        add(&mut self.entity_types, o.entity_types);
        add(&mut self.position_flags, o.position_flags);
        add(&mut self.entity_properties, o.entity_properties);
        self.flow_maps += o.flow_maps;
        self.landblocks_checked += o.landblocks_checked;
        self.normals.merge(o.normals);
        self.other_normals.merge(o.other_normals);
        self.high_mask_bits += o.high_mask_bits;
        self.sprites_mirrored += o.sprites_mirrored;
    }

    fn cross(&mut self, did: u32, why: &'static str) {
        self.cross.entry(why).or_insert((0, did)).0 += 1;
    }

    fn report(&mut self, did: u32, why: &'static str) {
        self.reported.entry(why).or_insert((0, did)).0 += 1;
    }

    fn record(&mut self, did: u32, stamp: u16, s: &SectorRecord) {
        self.records += 1;
        *self.stamps.entry(stamp).or_default() += 1;
        for why in s.deviations() {
            self.deviations.entry(why).or_insert((0, did)).0 += 1;
        }
        if let Some(m) = self.modes.get_mut(usize::from(s.mode)) {
            *m += 1;
        }
        self.ceiling_blocks += s.ceiling_mask.count_ones() as usize;
        *self.pairs_per_record.entry(s.water.len()).or_default() += 1;
        for c in s.terrain.chunks.iter().filter(|c| c.wet) {
            self.wet_chunks += 1;
            *self.wet_by_index.entry(c.water_index).or_default() += 1;
        }
        self.sprites += s.sprites.len();
        self.sprite_records += usize::from(!s.sprites.is_empty());
        self.sprite_max = self.sprite_max.max(s.sprites.len());
        let centre = s.origin().map(|(x, y)| (x + 1, y + 1));
        for sp in &s.sprites {
            *self.distances.entry(sp.distance.to_bits()).or_default() += 1;
            if let (Some(o), Some(c)) = (s.origin(), centre) {
                self.sprites_mirrored += usize::from(sprite_is_mirrored(sp, sp.viewer_frame(o, c)));
            }
        }
        self.high_mask_bits += usize::from(s.mode == 2 && s.ceiling_mask & !0x1FF != 0);
        self.entities += s.entities.len();
        self.entity_records += usize::from(!s.entities.is_empty());
        self.entity_max = self.entity_max.max(s.entities.len());
        for e in &s.entities {
            *self.entity_types.entry(e.entity_type).or_default() += 1;
            *self.position_flags.entry(e.position.flags()).or_default() += 1;
            *self
                .entity_properties
                .entry(e.property.as_ref().map_or(0, |p| p.pid))
                .or_default() += 1;
        }
        self.flow_maps += usize::from(!s.flow_map.is_null());
    }
}

/// Region number → its water table (`environment.md` §3, slot 5).
fn water_tables(set: &ArchiveSet) -> HashMap<u32, WaterTable> {
    let mut out = HashMap::new();
    let lo = u32::from(REGION_DESCRIPTOR_PREFIX) << 24;
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            let payload = m.archive.read_entry(&e).expect("read region descriptor");
            let d = RegionDescriptor::parse_expecting(&payload, e.did).expect("region descriptor");
            let Some(w) = d.water_table() else { continue };
            let wt = WaterTable::parse_expecting(&set.read(w.0).expect("read water table"), w.0)
                .expect("water table");
            out.insert(d.region, wt);
        }
    }
    out
}

/// Cross-record checks of one sector against its nine landblocks (DL-I6, I7, I8, I10) and
/// the named DIDs (DL-I13).
fn cross_check(
    set: &ArchiveSet,
    water: &HashMap<u32, WaterTable>,
    s: &SectorRecord,
    t: &mut Tally,
) {
    let did = s.did.0;
    let Some(id) = s.id() else { return };
    let region = u32::from(id.region);
    let census = CENSUS_REGIONS.contains(&region);
    for d in [
        s.colour_list,
        s.ceiling_colour_list,
        s.tile,
        s.ceiling_tile,
        s.flow_map,
        s.imposter_table,
    ] {
        if !d.is_null() && !matches!(set.locate(d.0), Ok(Some(_))) {
            t.cross(did, "DL-I13: a named DID does not exist");
        }
    }
    let table = water.get(&region);
    // The 5 × 5 landblocks around the sector (its own 3 × 3 and the ring the normal rule
    // of §4.3 reads), parsed once.
    let mut near: HashMap<(i32, i32), LandblockData> = HashMap::new();
    for gx in -1..=3i32 {
        for gy in -1..=3i32 {
            let (lx, ly) = (i32::from(id.x) + gx, i32::from(id.y) + gy);
            let (Ok(ux), Ok(uy)) = (u8::try_from(lx), u8::try_from(ly)) else {
                continue;
            };
            let block = (region << 16) | (u32::from(ux) << 8) | u32::from(uy);
            let Ok((loc, payload)) = set.read_with_entry(0x8000_0000 | block) else {
                continue;
            };
            if let Ok(d) = LandblockData::parse(&payload, Some(loc.entry.stamp)) {
                near.insert((lx, ly), d);
            }
        }
    }
    let normals = s.check_normals(|x, y| near.get(&(x, y)));
    if census {
        t.normals.merge(normals);
    } else {
        if !normals.holds() {
            t.report(did, "DL-I14: a normal more than one step off");
        }
        t.other_normals.merge(normals);
    }
    let mut rank = 0usize;
    for x in 0..3usize {
        for y in 0..3usize {
            let (lx, ly) = (u32::from(id.x) + x as u32, u32::from(id.y) + y as u32);
            let block = (region << 16) | (lx << 8) | ly;
            let Some(data) = near.get(&(lx as i32, ly as i32)) else {
                t.cross(did, "landblock data missing or does not decode");
                continue;
            };
            t.landblocks_checked += 1;
            if !block_equals(s.terrain.block(x, y), &data.heights) {
                const WHY: &str = "DL-I6: terrain block differs from the height grid";
                if census {
                    t.cross(did, WHY);
                } else {
                    t.report(did, WHY);
                }
            }
            if s.has_ceiling(x, y) != data.ceiling.is_some() {
                t.cross(did, "DL-I7: mask bit differs from has_ceiling");
            }
            if let (Some(c), Some(mesh)) = (&data.ceiling, &s.ceiling)
                && s.has_ceiling(x, y)
            {
                if !block_equals(mesh.block(x, y), &c.heights) {
                    t.cross(did, "DL-I8: ceiling block differs from the ceiling grid");
                }
                let start = mesh.chunk(x, y).map(|c| c.vertex_start as usize);
                if start != Some(rank * 1089) {
                    t.cross(did, "DL-I8: ceiling block not at its rank");
                }
                rank += 1;
            }
            let predicted = set
                .read(0x8010_0000 | block)
                .ok()
                .and_then(|m| BlockMap::parse(&m).ok())
                .and_then(|m| landblock_water(table, &m, &data.heights));
            let stored = s.water_at(x, y);
            match (predicted, stored) {
                (None, None) => {}
                (Some(p), Some(q)) if same_pair(p, q) => {}
                (Some(_), Some(_)) => t.cross(did, "DL-I10: water pair differs from water.md §6.2"),
                (Some(_), None) => t.cross(did, "DL-I10: dry chunk where water.md §6.2 has water"),
                (None, Some(_)) => t.cross(did, "DL-I10: wet chunk where water.md §6.2 has none"),
            }
        }
    }
}

#[test]
fn every_sector_decodes_and_follows_the_invariants() {
    let Some(set) = open_set() else { return };
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };
    let water = water_tables(set);
    let mut entries: Vec<(usize, Entry)> = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8050_0000..=0x805F_FFFF) {
            entries.push((i, e.expect("directory walk")));
        }
    }
    let full = exhaustive();
    let work: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| full || i % 23 == 0)
        .map(|(_, e)| *e)
        .collect();
    eprintln!(
        "kind-5 records in the directory: {}; visiting {}",
        entries.len(),
        work.len()
    );
    let tallies = par_each(&work, |(i, e)| {
        let mut t = Tally::default();
        let payload = match set.member(*i).archive.read_entry(e) {
            Ok(p) => p,
            Err(err) => {
                t.failed.insert(format!("read: {err}"), (1, e.did));
                return t;
            }
        };
        match SectorRecord::parse_expecting(&payload, e.did, &dict) {
            Ok(s) => {
                t.record(e.did, e.stamp, &s);
                cross_check(set, &water, &s, &mut t);
            }
            Err(err) => {
                let key = format!(
                    "{}: {}",
                    FailureCategory::of(&err).name(),
                    err.to_string().chars().take(80).collect::<String>()
                );
                t.failed.insert(key, (1, e.did));
            }
        }
        t
    });
    let mut all = Tally::default();
    for t in tallies {
        all.merge(t);
    }

    let failed: usize = all.failed.values().map(|v| v.0).sum();
    eprintln!(
        "sectors: {} visited, {} decoded, {failed} failed; stamps {:?}",
        work.len(),
        all.records,
        all.stamps
    );
    for (why, (n, did)) in &all.failed {
        eprintln!("    failed: {n} × {why} (e.g. {did:#010x})");
    }
    for (why, (n, did)) in &all.deviations {
        eprintln!("    deviation: {n} × {why} (e.g. {did:#010x})");
    }
    for (why, (n, did)) in &all.cross {
        eprintln!("    cross-record: {n} × {why} (e.g. {did:#010x})");
    }
    for (why, (n, did)) in &all.reported {
        eprintln!(
            "    outside the census regions (reported, not asserted): {n} × {why} (e.g. \
             {did:#010x})"
        );
    }
    eprintln!(
        "mode 0/1/2: {:?}; ceiling blocks {}; landblocks cross-checked {}",
        all.modes, all.ceiling_blocks, all.landblocks_checked
    );
    eprintln!(
        "water pairs per record {:?}; wet terrain chunks {} by index {:?}",
        all.pairs_per_record, all.wet_chunks, all.wet_by_index
    );
    let distances: BTreeMap<String, usize> = all
        .distances
        .iter()
        .map(|(b, n)| (format!("{}", f32::from_bits(*b)), *n))
        .collect();
    eprintln!(
        "sprites {} in {} records (max {}); distance values {distances:?}",
        all.sprites, all.sprite_records, all.sprite_max
    );
    let flags: BTreeMap<String, usize> = all
        .position_flags
        .iter()
        .map(|(f, n)| (format!("{f:#04X}"), *n))
        .collect();
    let props: BTreeMap<String, usize> = all
        .entity_properties
        .iter()
        .map(|(p, n)| (format!("{p:#X}"), *n))
        .collect();
    eprintln!(
        "entities {} in {} records (max {}); types {:?}; Position flags {flags:?}; \
         properties (0 = null) {props:?}",
        all.entities, all.entity_records, all.entity_max, all.entity_types
    );
    eprintln!("flow maps present: {}", all.flow_maps);

    assert!(!work.is_empty(), "no kind-5 records found");
    assert_eq!(failed, 0, "sector records failed to decode (DL-I1)");
    assert!(all.deviations.is_empty(), "record-local invariants broken");
    assert!(all.cross.is_empty(), "cross-record invariants broken");
    assert_eq!(all.landblocks_checked, 9 * all.records);
    let n = all.normals;
    let o = all.other_normals;
    for (scope, n) in [("census regions", &n), ("other regions", &o)] {
        eprintln!(
            "DL-I14 normals ({scope}): terrain {} samples, {} exact, {} within one step, {} \
             beyond, {} uncomputable; ceiling {} samples, {} exact, {} within one step, {} \
             beyond, {} uncomputable, {} pointing up",
            n.terrain.samples,
            n.terrain.exact,
            n.terrain.within_one,
            n.terrain.beyond,
            n.terrain.uncomputable,
            n.ceiling.samples,
            n.ceiling.exact,
            n.ceiling.within_one,
            n.ceiling.beyond,
            n.ceiling.uncomputable,
            n.ceiling.ceiling_up
        );
    }
    eprintln!(
        "§5.6: {} of {} sprites mirrored for a viewer on the sector's centre landblock",
        all.sprites_mirrored, all.sprites
    );
    assert!(
        n.holds(),
        "DL-I14: stored normals follow the rule of §4.3 (census regions)"
    );
    assert_eq!(all.high_mask_bits, 0, "DL-I15");
    if full {
        assert_eq!(
            n.terrain.samples + o.terrain.samples,
            9 * 1089 * all.records as u64
        );
    }
}
