// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain layer resolution against a user's own install (read-only):
//! `docs/specs/terrain-texturing.md` §3.5, §4, §5.1, §10 and `scenery.md` SC7.
//!
//! - **Tables (§3.5 items 1–6).** Every terrain type table (prefix 0x19), found both by
//!   scanning the archives and through slot 8 of every region descriptor (T1), decodes to
//!   its last byte (the decoder enforces items 1–6) and re-encodes byte for byte; the test
//!   checks the items again on the decoded values: keys exactly 0 … n − 1, curve modes
//!   1–10, projection 1–3, every label within the record.
//! - **SC7 and §4.1.** Every surface-layer key and every quad-grid value of every terrain
//!   and ceiling chunk of every landblock-data record resolves in its region's table:
//!   [`resolve_landblock`] reports no unresolved layer. Also §4.2 (draw order
//!   non-decreasing, no key twice in a chunk) and §4.3/§4.5 (no rejected layer triangle:
//!   none off the level-0 grid, wound against its surface or repeated;
//!   hole triangles counted in terrain chunks with layers).
//! - **§5.1 (T14).** Every mask named by a drawn layer exists and is an A8 surface, square,
//!   128, 256 or 512 texels, and passes [`AlphaMask::check`].
//! - **§10 (T37).** Every `Landblock_LightMap` and `Landblock_CeilingLightMap` value of
//!   every landblock-property record names a DXT1 surface.
//!
//! Counts are printed next to the spec's figures for this install (observations, so they
//! are not asserted). A release build visits every record; a debug build a 1-in-23 sample
//! unless `TESSERA_FULL=1`. Skipped unless `TESSERA_GAME_DIR` points at an install. Run
//! with `cargo test --release -p tessera-terrain --test layers_real_data -- --nocapture`.
//! Only counts and DIDs are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::landblock::{
    GridSurface, LandblockPids, LightMaps, QUAD_SIDE, REGION_DESCRIPTOR_PREFIX,
};
use tessera_formats::terrain_type::{TERRAIN_TYPE_TABLE_PREFIX, TerrainTypeTable};
use tessera_formats::{
    Did, LandblockData, LandblockProperties, PropertyDictionary, Record, RegionDescriptor, Surface,
    SurfaceFormat,
};
use tessera_terrain::{
    AlphaMask, ChunkLayers, EdgeLods, LayerVertices, Lod, Neighbours, ResolveOptions, TerrainBlock,
    VertexNormals, fixed_block_offset, resolve_landblock, triangulate,
};

/// The property dictionary (`property-system.md` §3), as the formats tests read it.
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

fn open_set() -> Option<ArchiveSet> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
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

/// Every record with type prefix `prefix` (first archive wins), with its stored bytes.
fn records(set: &ArchiveSet, prefix: u8) -> BTreeMap<u32, Vec<u8>> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if out.contains_key(&e.did) {
                continue;
            }
            out.insert(e.did, m.archive.read_entry(&e).expect("read record"));
        }
    }
    out
}

/// Landblock records of kind `k` (0x80KR….) per cell-archive region: (member, entry).
fn landblock_entries(set: &ArchiveSet, k: u32) -> BTreeMap<u32, Vec<(usize, Entry)>> {
    let mut out: BTreeMap<u32, Vec<(usize, Entry)>> = BTreeMap::new();
    for (i, m) in set.members().iter().enumerate() {
        let ArchiveKind::Cell(region) = m.id.kind else {
            continue;
        };
        let lo = 0x8000_0000 | (k << 20);
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            out.entry(region)
                .or_default()
                .push((i, e.expect("directory walk")));
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

/// Region number → its terrain type table, through slot 8 of the region descriptors (T1).
fn region_tables(set: &ArchiveSet) -> BTreeMap<u32, TerrainTypeTable> {
    let mut out = BTreeMap::new();
    for (did, payload) in records(set, REGION_DESCRIPTOR_PREFIX) {
        let d = RegionDescriptor::parse_expecting(&payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let tdid = d
            .terrain_type_table()
            .unwrap_or_else(|| panic!("{did:#010x}: no slot-8 table"));
        let bytes = set
            .read(tdid.0)
            .unwrap_or_else(|e| panic!("{did:#010x}: table {tdid}: {e}"));
        let t = TerrainTypeTable::parse_expecting(&bytes, tdid)
            .unwrap_or_else(|e| panic!("table {tdid}: {e}"));
        out.insert(d.region, t);
    }
    out
}

#[test]
fn terrain_type_tables_hold_the_decoder_invariants() {
    let Some(set) = open_set() else { return };
    let all = records(&set, TERRAIN_TYPE_TABLE_PREFIX);
    assert!(!all.is_empty(), "no terrain type table");
    let mut entries = 0usize;
    for (&did, payload) in &all {
        // Items 1–6 are enforced by the decoder: a violation is a decode error.
        let t = TerrainTypeTable::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        // Item 1: consumed exactly (and nothing invented): the stored form re-encodes.
        assert_eq!(&t.to_bytes(), payload, "{did:#010x}: re-encoding");
        // Item 2: keys exactly 0 … n − 1.
        let keys: Vec<u32> = t.iter_by_key().map(|e| e.key).collect();
        assert!(
            keys.iter().copied().eq(0..t.len() as u32),
            "{did:#010x}: keys"
        );
        for e in t.entries() {
            // Item 3: curve modes 1–10; item 5: projection 1–3; item 6: the label fits.
            for c in [&e.u_offset, &e.v_offset] {
                assert!((1..=10).contains(&c.mode()), "{did:#010x} key {}", e.key);
            }
            assert!((1..=3).contains(&e.projection.raw()));
            assert!(e.label.len() < payload.len());
        }
        entries += t.len();
    }
    let by_region = region_tables(&set);
    eprintln!(
        "terrain type tables: {} records, {entries} entries (spec §3.1: 5, 4,688); \
         regions with a slot-8 table: {:?}",
        all.len(),
        by_region.keys().collect::<Vec<_>>()
    );
    for t in by_region.values() {
        assert!(
            all.contains_key(&t.did.0),
            "slot-8 table {} not found by scan",
            t.did
        );
    }
}

#[derive(Default)]
struct Tally {
    blocks: usize,
    failures: Vec<String>,
    chunks: [usize; 2],
    layers: [usize; 2],
    drawn: [usize; 2],
    whole: usize,
    no_diffuse_drawn: [usize; 2],
    hole_blocks: usize,
    stream_checks: usize,
    all_empty: usize,
    hole_triangles: usize,
    off_grid: usize,
    order_violations: usize,
    repeated_keys: usize,
    masks: BTreeSet<u32>,
}

impl Tally {
    fn merge(&mut self, o: Self) {
        self.blocks += o.blocks;
        self.failures.extend(o.failures);
        for k in 0..2 {
            self.chunks[k] += o.chunks[k];
            self.layers[k] += o.layers[k];
            self.drawn[k] += o.drawn[k];
            self.no_diffuse_drawn[k] += o.no_diffuse_drawn[k];
        }
        self.whole += o.whole;
        self.hole_blocks += o.hole_blocks;
        self.stream_checks += o.stream_checks;
        self.all_empty += o.all_empty;
        self.hole_triangles += o.hole_triangles;
        self.off_grid += o.off_grid;
        self.order_violations += o.order_violations;
        self.repeated_keys += o.repeated_keys;
        self.masks.extend(o.masks);
    }
}

fn check_block(set: &ArchiveSet, member: usize, e: &Entry, table: &TerrainTypeTable) -> Tally {
    let mut t = Tally {
        blocks: 1,
        ..Tally::default()
    };
    let payload = match set.member(member).archive.read_entry(e) {
        Ok(p) => p,
        Err(err) => {
            t.failures.push(format!("{:#010x}: read: {err}", e.did));
            return t;
        }
    };
    let data = match LandblockData::parse_expecting(&payload, e.did, Some(e.stamp)) {
        Ok(d) => d,
        Err(err) => {
            t.failures.push(format!("{:#010x}: decode: {err}", e.did));
            return t;
        }
    };
    for (k, surface) in [GridSurface::Terrain, GridSurface::Ceiling]
        .into_iter()
        .enumerate()
    {
        let Some(chunk) = data.surface_chunk(surface) else {
            continue;
        };
        if chunk.layers.is_empty() {
            continue;
        }
        t.chunks[k] += 1;
        t.layers[k] += chunk.layers.len();
        // SC7: quad-grid values are keys of the table too.
        if let Some(q) = &chunk.quads {
            for &v in q.iter() {
                if table.get(u32::from(v)).is_none() {
                    t.failures.push(format!(
                        "{:#010x} {surface:?}: quad value {v} not in {}",
                        e.did, table.did
                    ));
                    break;
                }
            }
            debug_assert_eq!(q.len(), QUAD_SIDE * QUAD_SIDE);
        }
        // §4.2: non-decreasing draw order, no key twice.
        let mut seen = BTreeSet::new();
        let mut last = 0u32;
        for l in &chunk.layers {
            if !seen.insert(l.material) {
                t.repeated_keys += 1;
            }
            if let Some(entry) = table.get(l.material) {
                if entry.draw_order < last {
                    t.order_violations += 1;
                }
                last = entry.draw_order;
            }
        }
        let r = match resolve_landblock(
            &data,
            surface,
            table,
            &LightMaps::default(),
            &ResolveOptions::default(),
        ) {
            Ok(r) => r,
            Err(err) => {
                t.failures
                    .push(format!("{:#010x} {surface:?}: resolve: {err}", e.did));
                continue;
            }
        };
        // SC7 / §4.1: every layer key is in the region's table.
        for u in r.unresolved() {
            t.failures.push(format!(
                "{:#010x} {surface:?}: layer {} key {} not in {}",
                e.did, u.ordinal, u.key, table.did
            ));
        }
        t.off_grid += r.rejected_triangles();
        // On every 7th block: the level-0 index lists give the stored triangles back (both
        // surfaces, winding included), and the surface's vertex streams are finite with
        // downward ceiling normals.
        if e.did.is_multiple_of(7) {
            if let Err(f) = check_streams(&data, surface, &r) {
                t.failures.push(format!("{:#010x} {surface:?}: {f}", e.did));
            }
            t.stream_checks += 1;
        }
        t.drawn[k] += r.layers().len();
        for l in r.layers() {
            t.whole += usize::from(l.whole());
            if l.entry().diffuse.is_null() {
                t.no_diffuse_drawn[k] += 1;
            }
            if let Some(m) = l.mask() {
                t.masks.insert(m.0);
            }
        }
        // Counted over every terrain chunk with layers, drawn or not (§4.5).
        if surface == GridSurface::Terrain {
            t.all_empty += usize::from(r.is_empty());
            let holes = r.uncovered().len();
            if holes > 0 {
                t.hole_blocks += 1;
                t.hole_triangles += holes;
                if data.ceiling.is_none() {
                    t.failures.push(format!(
                        "{:#010x}: {holes} hole triangles without a ceiling (§4.5)",
                        e.did
                    ));
                }
            }
        }
    }
    t
}

/// Twice the signed area of a triangle of grid vertex numbers seen from above (positive:
/// counter-clockwise).
fn signed_area(t: [u16; 3]) -> i64 {
    let p = t.map(|v| (i64::from(v) % 33, i64::from(v) / 33));
    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0)
}

/// Rotation-normalised triangle (same winding).
fn canon(t: [u16; 3]) -> [u16; 3] {
    let k = (0..3).min_by_key(|&k| t[k]).unwrap_or(0);
    [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
}

fn check_streams(
    data: &LandblockData,
    surface: GridSurface,
    r: &ChunkLayers,
) -> Result<(), String> {
    let (block, normals) = match surface {
        GridSurface::Terrain => {
            let b = TerrainBlock::from_landblock(data);
            let n = VertexNormals::compute(&b, &Neighbours::NONE);
            (b, n)
        }
        GridSurface::Ceiling => {
            let b = TerrainBlock::from_ceiling(data).ok_or("no ceiling")?;
            let n = VertexNormals::ceiling(&b, &Neighbours::NONE);
            (b, n)
        }
    };
    let lb = data.landblock().ok_or("no landblock id")?;
    let v = LayerVertices::build(&block, &normals, fixed_block_offset(lb.x, lb.y));
    let finite = |s: &[[f32; 3]]| s.iter().all(|p| p.iter().all(|c| c.is_finite()));
    if !(finite(&v.positions) && finite(&v.normals) && finite(&v.tangents)) {
        return Err("non-finite stream".into());
    }
    let down = surface == GridSurface::Ceiling;
    if v.normals.iter().any(|n| (n[2] < 0.0) != down) {
        return Err("normal on the wrong side".into());
    }
    // Both surfaces: the level-0 lists of `layer_indices_for` give the stored triangles
    // back, winding included (clockwise for the ceiling), and every drawn triangle is wound
    // as its surface is.
    let level0 = triangulate(&block, Lod::FINEST, EdgeLods::NONE);
    for (k, l) in r.layers().iter().enumerate() {
        let got: BTreeSet<[u16; 3]> = r
            .layer_indices_for(k, &level0)
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&t| canon(t))
            .collect();
        let want: BTreeSet<[u16; 3]> = l.triangles().iter().map(|&t| canon(t)).collect();
        if got != want {
            return Err(format!("layer {}: level-0 list differs", l.ordinal()));
        }
        if l.triangles().iter().any(|&t| (signed_area(t) < 0) != down) {
            return Err(format!(
                "layer {}: triangle wound against the surface",
                l.ordinal()
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct MaskTally {
    failures: Vec<String>,
    sizes: BTreeMap<u32, usize>,
}

#[test]
fn layers_resolve_and_masks_are_a8() {
    let Some(set) = open_set() else { return };
    let tables = region_tables(&set);
    let mut all = Tally::default();
    for (region, entries) in landblock_entries(&set, 0) {
        let table = tables
            .get(&region)
            .unwrap_or_else(|| panic!("region {region}: no terrain type table"));
        let work = sample(&entries);
        let parts = par_each(&work, |(i, e)| check_block(&set, *i, e, table));
        let mut tally = Tally::default();
        for p in parts {
            tally.merge(p);
        }
        eprintln!(
            "region {region} (table {}): {} blocks; {} terrain + {} ceiling chunks with layers; \
             {} + {} layers, {} + {} drawn; {} hole triangles in {} blocks",
            table.did,
            tally.blocks,
            tally.chunks[0],
            tally.chunks[1],
            tally.layers[0],
            tally.layers[1],
            tally.drawn[0],
            tally.drawn[1],
            tally.hole_triangles,
            tally.hole_blocks,
        );
        all.merge(tally);
    }
    eprintln!(
        "all: {} blocks; chunks with layers {} terrain + {} ceiling (spec §4.2: 86,726 + 2,355); \
         layers {} (spec T5: 925,884); drawn {} + {}; whole {}; drawn layers without diffuse \
         {} terrain + {} ceiling (spec T39: 60 + 43); hole triangles {} in {} blocks \
         (spec §4.5: 2,750,134 in 2,368), of them {} terrain chunks with no drawn layer; rejected triangles {}; draw-order violations {}; repeated keys {}; stream checks {}",
        all.blocks,
        all.chunks[0],
        all.chunks[1],
        all.layers[0] + all.layers[1],
        all.drawn[0],
        all.drawn[1],
        all.whole,
        all.no_diffuse_drawn[0],
        all.no_diffuse_drawn[1],
        all.hole_triangles,
        all.hole_blocks,
        all.all_empty,
        all.off_grid,
        all.order_violations,
        all.repeated_keys,
        all.stream_checks,
    );
    for f in all.failures.iter().take(20) {
        eprintln!("FAIL {f}");
    }
    assert!(all.failures.is_empty(), "{} failures", all.failures.len());
    assert_eq!(
        all.off_grid, 0,
        "rejected layer triangles (off the grid, wrongly wound, repeated)"
    );
    assert_eq!(
        all.order_violations, 0,
        "§4.2: draw order decreases in a chunk"
    );
    assert_eq!(all.repeated_keys, 0, "§4.2: a chunk names a key twice");

    // §5.1 (T14): every mask of a drawn layer is an existing A8 surface.
    let masks: Vec<u32> = all.masks.iter().copied().collect();
    let parts = par_each(&masks, |&did| {
        let mut t = MaskTally::default();
        let bytes = match set.read(did) {
            Ok(b) => b,
            Err(err) => {
                t.failures.push(format!("{did:#010x}: read: {err}"));
                return t;
            }
        };
        match Surface::parse_expecting(&bytes, did) {
            Ok(s) => {
                if let Err(err) = AlphaMask::check(&s) {
                    t.failures.push(format!("{did:#010x}: {err}"));
                } else if s.width != s.height || ![128, 256, 512].contains(&s.width) {
                    t.failures
                        .push(format!("{did:#010x}: size {} × {}", s.width, s.height));
                }
                *t.sizes.entry(s.width).or_default() += 1;
            }
            Err(err) => t.failures.push(format!("{did:#010x}: decode: {err}")),
        }
        t
    });
    let mut sizes = BTreeMap::new();
    let mut failures = Vec::new();
    for p in parts {
        failures.extend(p.failures);
        for (k, n) in p.sizes {
            *sizes.entry(k).or_insert(0usize) += n;
        }
    }
    eprintln!(
        "masks of drawn layers: {} distinct, sizes {sizes:?} \
         (spec §5.1: 646,073 = 623,092 × 128 + 20,624 × 256 + 2,357 × 512), {} failures",
        masks.len(),
        failures.len()
    );
    for f in failures.iter().take(20) {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "{} mask failures", failures.len());
}

#[test]
fn light_maps_are_dxt1() {
    let Some(set) = open_set() else { return };
    let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
    let dict = PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY)
        .expect("property dictionary");
    let pids = LandblockPids::from_dictionary(&dict);
    assert!(
        pids.landblock_light_map.is_some() && pids.landblock_ceiling_light_map.is_some(),
        "light-map properties not in the dictionary"
    );
    let mut values = [0usize; 2];
    let mut surfaces = BTreeSet::new();
    let mut failures = Vec::new();
    let mut records = 0usize;
    for (_, entries) in landblock_entries(&set, 4) {
        let work = sample(&entries);
        records += work.len();
        let parts = par_each(&work, |(i, e)| {
            let payload = set
                .member(*i)
                .archive
                .read_entry(e)
                .map_err(|err| format!("{:#010x}: read: {err}", e.did))?;
            let p = LandblockProperties::parse_expecting(&payload, e.did, &dict)
                .map_err(|err| format!("{:#010x}: decode: {err}", e.did))?;
            Ok::<LightMaps, String>(p.light_maps(&pids))
        });
        for p in parts {
            match p {
                Ok(l) => {
                    for (k, d) in [l.terrain, l.ceiling].into_iter().enumerate() {
                        if let Some(d) = d {
                            values[k] += 1;
                            surfaces.insert(d.0);
                        }
                    }
                }
                Err(f) => failures.push(f),
            }
        }
    }
    let list: Vec<u32> = surfaces.iter().copied().collect();
    let checked = par_each(&list, |&did| {
        let bytes = set
            .read(did)
            .map_err(|err| format!("{did:#010x}: read: {err}"))?;
        let s = Surface::parse_expecting(&bytes, Did(did))
            .map_err(|err| format!("{did:#010x}: decode: {err}"))?;
        if s.format != SurfaceFormat::Dxt1 {
            return Err(format!("{did:#010x}: format {}", s.format.name()));
        }
        Ok((s.width, s.height))
    });
    let mut sizes: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for c in checked {
        match c {
            Ok(wh) => *sizes.entry(wh).or_default() += 1,
            Err(f) => failures.push(f),
        }
    }
    eprintln!(
        "landblock properties: {records} records; light-map values {} terrain + {} ceiling = {} \
         (spec §10: 4,692 + 749 = 5,441); {} distinct surfaces (spec: 5,422), sizes {sizes:?} \
         (spec: 5,421 × 128², 1 × 64²); {} failures",
        values[0],
        values[1],
        values[0] + values[1],
        surfaces.len(),
        failures.len()
    );
    for f in failures.iter().take(20) {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "{} failures", failures.len());
}
