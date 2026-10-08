// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery generation and frills against a user's own install (read-only). Skipped unless
//! `TESSERA_GAME_DIR` points at an install.
//!
//! - `records_sc1_to_sc5_sc9_sc10`: the region tables read through the region descriptors'
//!   slots 1, 2 and 8 ([`SceneryTables::load_all_regions`]) and every 0x11, 0x12, 0x02 and
//!   0x32 record of the install: SC1 (decode to the last byte), SC2 (scene DIDs and
//!   references exist, acyclic), SC3 (objects are existing 0x47 records), SC4, SC5, SC9 and
//!   SC10 of `scenery.md` §13.
//! - `coarse_grid_sc6_and_census`: SC6 (bits 15–31 zero, edge words equal the neighbour's)
//!   over every landblock-data record, and the census of `scenery.md` §12 (blocks,
//!   vertices with a group scene, with a frill layer, without a scene), printed and not
//!   asserted: counts are observations of one install (`scenery.md` §0).
//! - `generation_is_deterministic`: ROADMAP M4.5 exit criterion 3 (SC8): the scenery of
//!   every landblock with a non-zero coarse grid is generated twice, with 1 worker in
//!   ascending order and with the default worker count in descending order; the object
//!   lists (DID, position, rotation, scale) must be identical bit for bit. No hostile-input
//!   bound may be hit on real data. Water comes from no landblock here (the world's water
//!   module wires the real surface in).
//! - `frill_sectors`: every sector of a sample of landblocks, spaced against the
//!   landblock's static list (§7.4 step 14a) and the footprint markers of its static pass
//!   run with that list (`scenery.md` §8.2 item 5: the frill pass runs on the generator
//!   that ran the static pass); frill instances lie in their landblock; density 1 is
//!   deterministic; no child fails (§7.4 step 15).
//! - `checklist_generated_lists`: the generated lists of the 11 landblocks of
//!   `docs/checklists/m4-5-visual.md` §4.2 (rows S1–S11), built through the bundle path
//!   (water and statics included), have the checklist's totals, and the objects of walks
//!   GN1–GN4 (§2.3) sit at the listed list indices with the listed DID, position and scale
//!   (`scenery.md` §7.4 step 14b as revised 2026-10-07); no child fails (step 15).
//!
//! Debug builds sample every 23rd landblock unless `TESSERA_FULL=1`; run
//! `cargo test --release -p tessera-world --test scenery_real_data -- --nocapture` for the
//! full criteria. Only counts and DIDs are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::environment::LodClassTable;
use tessera_formats::scenery::{
    SCALE_MODE_UNIFORM, Scene, SceneChild, SceneChoiceTable, ScenePair, SceneReferenceChecker,
    TerrainSceneTable,
};
use tessera_formats::{Did, LandblockData, LandblockInfo, Record};
use tessera_physics::geometry::GridMesh;
use tessera_world::frills::{DensityThinner, generate_sector};
use tessera_world::scenery::{
    ClearancePids, MAX_STATIC_VOLUMES, NoWater, ObjectPropertyCache, SceneLibrary, SceneSource,
    SceneryInput, SceneryLimits, SceneryStats, SceneryTables, StaticClearance, VertexLookup,
    generate_landblock, static_clearances, sum_stats,
};
use tessera_world::{
    BundleOptions, BundleSource, LandblockKey, WorldContext, WorldSource, build_bundle,
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn open() -> Option<(Arc<ArchiveSet>, WorldContext)> {
    let dir = install()?;
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    Some((set, ctx))
}

/// Every record with type prefix `prefix` (first archive wins).
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

/// The directory entry of every landblock-data record (kind 0) in the cell archives, by
/// DID.
fn data_entries(set: &ArchiveSet) -> Vec<Entry> {
    let mut out: BTreeMap<u32, Entry> = BTreeMap::new();
    for m in set.members() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8000_0000..=0x800F_FFFF) {
            let e = e.expect("directory walk");
            out.entry(e.did).or_insert(e);
        }
    }
    let all: Vec<Entry> = out.into_values().collect();
    if full() {
        all
    } else {
        all.into_iter().step_by(23).collect()
    }
}

/// Reads and decodes a landblock-data record with its directory stamp.
fn read_data(set: &ArchiveSet, e: &Entry) -> LandblockData {
    let (bytes, stamp) = WorldSource::read_stamped(set, Did(e.did))
        .expect("read")
        .expect("record exists");
    LandblockData::parse_expecting(&bytes, e.did, stamp).expect("data decodes")
}

/// Runs `f` over `items` on `workers` threads, results in item order.
fn par_map<T: Sync, R: Send>(items: &[T], workers: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(k) else { break };
                    let r = f(item);
                    out.lock().unwrap()[k] = Some(r);
                }
            });
        }
    });
    out.into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.expect("every item mapped"))
        .collect()
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

#[test]
fn records_sc1_to_sc5_sc9_sc10() {
    let Some((set, ctx)) = open() else { return };
    // The tables through the region descriptors (slots 1, 2, 8).
    let loaded = SceneryTables::load_all_regions(&ctx);
    assert!(
        loaded.retry.is_empty(),
        "pipeline failures reading tables: {:?}",
        loaded.retry
    );
    let by_region = loaded.tables;
    assert!(!by_region.is_empty(), "no region has scenery tables");
    for (r, t) in &by_region {
        // §4.1 observation, not an invariant: a region without a slot-8 table only loses
        // the terrain-type test (§7.4 step 7).
        if t.terrain_types.is_none() {
            eprintln!("region {r}: no terrain-type table (slot 8)");
        }
        assert_eq!(
            t.terrain_scenes.did.0 & 0x00FF_FFFF,
            t.scene_choices.did.0 & 0x00FF_FFFF,
            "region {r}: slots 1 and 2 share the serial"
        );
        eprintln!(
            "region {r}: 0x12 {:#010x} ({} entries), 0x11 {:#010x} ({} entries), 0x19 {:#010x}",
            t.terrain_scenes.did.0,
            t.terrain_scenes.entries().len(),
            t.scene_choices.did.0,
            t.scene_choices.entries().len(),
            t.terrain_types.as_ref().map_or(0, |t| t.did.0),
        );
    }

    // SC1 for 0x12 and 0x11; SC5.
    let ts = records(&set, 0x12);
    let mut n12 = 0;
    for (&did, bytes) in &ts {
        let t = TerrainSceneTable::parse_expecting(bytes, did).expect("0x12 decodes");
        assert_eq!(&t.to_bytes(), bytes, "{did:#010x} re-encodes");
        let d = t.default_entry().expect("default entry");
        let one = ScenePair {
            scene_key: 1,
            slot: 0,
        };
        assert_eq!(d.name.len(), 7, "{did:#010x}: 41-byte default entry");
        assert_eq!((d.kind, d.id, d.default_pair), (1, 0, Some(one)));
        assert_eq!(d.pairs, [one]);
        let mut ids = BTreeSet::new();
        for e in t.entries() {
            assert!(ids.insert(e.id), "{did:#010x}: id {} repeated", e.id);
            assert_eq!(
                e.pairs.first(),
                Some(&ScenePair {
                    scene_key: 2,
                    slot: 0
                })
            );
            let slots: BTreeSet<u32> = e.pairs.iter().map(|p| p.slot).collect();
            assert_eq!(slots.len(), e.pairs.len(), "{did:#010x}: slots distinct");
        }
        n12 += 1;
    }
    let cs = records(&set, 0x11);
    let scenes_raw = records(&set, 0x02);
    let mut scenes: HashMap<Did, Scene> = HashMap::new();
    for (&did, bytes) in &scenes_raw {
        let s = Scene::parse_expecting(bytes, did).expect("0x02 decodes");
        assert_eq!(&s.to_bytes(), bytes, "{did:#010x} re-encodes");
        scenes.insert(Did(did), s);
    }
    let mut n11 = 0;
    let mut named = BTreeSet::new();
    for (&did, bytes) in &cs {
        let t = SceneChoiceTable::parse_expecting(bytes, did).expect("0x11 decodes");
        assert_eq!(&t.to_bytes(), bytes, "{did:#010x} re-encodes");
        for e in t.entries() {
            for c in &e.choices {
                assert_eq!(c.flag, 1, "SC4: choice flag");
                assert!(scenes.contains_key(&c.scene), "SC2: {} missing", c.scene);
                named.insert(c.scene);
            }
        }
        n11 += 1;
    }
    eprintln!(
        "SC1: 0x12 {n12}, 0x11 {n11}, 0x02 {} decoded to the last byte; {} scenes named",
        scenes.len(),
        named.len()
    );
    // Counts are observations of one install (§0); only make sure the test saw records.
    assert!(
        n12 > 0 && n11 > 0 && !scenes.is_empty(),
        "no scenery records"
    );

    // SC2: references exist, the graph is acyclic.
    let mut checker = SceneReferenceChecker::new();
    let mut max_height = 0;
    for &did in scenes.keys() {
        let h = checker
            .height(did, |d| scenes.get(&d))
            .unwrap_or_else(|e| panic!("SC2: {did}: {e}"));
        max_height = max_height.max(h);
    }
    eprintln!("SC2: acyclic, longest reference path {max_height} scenes");

    // SC3, SC4, SC10.
    let mut objects = BTreeSet::new();
    let (mut nodes, mut clear0) = (0usize, 0usize);
    // cos 90° evaluated in f64 and stored as an f32 (SC10: 6.1 × 10⁻¹⁷).
    #[allow(clippy::cast_possible_truncation)]
    let cos90 = std::f64::consts::FRAC_PI_2.cos() as f32;
    for (did, s) in &scenes {
        for g in &s.groups {
            assert_eq!(g.group_id, 1, "SC4: {did} group id");
        }
        if let Some(f) = &s.frill_layer {
            assert!(
                f.children.iter().all(|c| matches!(c, SceneChild::Scene(_))),
                "SC4: {did} frill layer children"
            );
        }
        objects.extend(s.objects());
        for (_, _, n) in s.nodes() {
            nodes += 1;
            assert_eq!(n.scale_mode, SCALE_MODE_UNIFORM, "SC4: {did} scale mode");
            assert!(n.height_mode <= 1, "SC4: {did} height mode");
            assert_eq!(n.clear_aspect_tol, 360.0, "SC10: {did}");
            if n.clearance == 0.0 {
                clear0 += 1;
                assert_eq!(
                    (n.clear_max_cos, n.clear_min_cos, n.clear_min_dot),
                    (1.0, cos90, -1.0),
                    "SC10: {did}"
                );
            }
        }
    }
    for &o in &objects {
        assert_eq!(o.prefix(), 0x47, "SC3: {o}");
        assert!(
            WorldSource::contains(&*set, o).expect("lookup"),
            "SC3: {o} missing"
        );
    }
    eprintln!(
        "SC3: {} objects exist; SC4 and SC10 hold over {nodes} nodes ({clear0} with clearance 0)",
        objects.len()
    );

    // SC9.
    let lods = records(&set, 0x32);
    eprintln!("SC9: {} LOD-class records", lods.len());
    for (&did, bytes) in &lods {
        let t = LodClassTable::parse_expecting(bytes, did).expect("0x32 decodes");
        assert_eq!(bytes.len(), 8 + 36 * t.len(), "SC9: payload size");
        let keys: BTreeSet<u32> = t.entries.iter().map(|e| e.class).collect();
        assert_eq!(keys.len(), t.len(), "SC9: classes distinct");
        eprintln!(
            "SC9: {did:#010x}: {} classes, {} bytes",
            t.len(),
            bytes.len()
        );
    }
}

/// Per-record result of the coarse-grid pass.
#[derive(Default)]
struct BlockCensus {
    region: u8,
    nonzero: bool,
    high_bits: u32,
    edges: Option<[[u32; 17]; 4]>,
    group: u64,
    frill: u64,
    frill_nonempty: u64,
    no_pair: u64,
}

#[test]
fn coarse_grid_sc6_and_census() {
    let Some((set, ctx)) = open() else { return };
    let tables = SceneryTables::for_all_regions(&ctx);
    let library = SceneLibrary::new(Arc::clone(ctx.source()));
    let entries = data_entries(&set);
    let start = Instant::now();
    // Per scene: (non-empty placement group, frill layer, frill layer with children).
    let traits = Mutex::new(HashMap::<Did, (bool, bool, bool)>::new());
    let scene_traits = |d: Did| {
        if let Some(t) = traits.lock().unwrap().get(&d) {
            return *t;
        }
        let t = library
            .scene(d)
            .expect("scene read")
            .map_or((false, false, false), |s| {
                (
                    s.placement_group().is_some_and(|g| !g.children.is_empty()),
                    s.frill_layer.is_some(),
                    s.frill_layer
                        .as_ref()
                        .is_some_and(|f| !f.children.is_empty()),
                )
            });
        traits.lock().unwrap().insert(d, t);
        t
    };
    let results = par_map(&entries, workers(), |e| {
        let data = read_data(&set, e);
        let key = LandblockKey::from_did(e.did).expect("landblock key");
        let mut c = BlockCensus {
            region: key.region(),
            ..BlockCensus::default()
        };
        c.high_bits = data.coarse.iter().filter(|w| **w >> 15 != 0).count() as u32;
        c.nonzero = data.coarse.iter().any(|w| *w != 0);
        if !c.nonzero {
            return (e.did, c);
        }
        let w = |i: usize, j: usize| data.coarse_word(i, j).unwrap_or(0);
        c.edges = Some([
            std::array::from_fn(|j| w(0, j)),
            std::array::from_fn(|j| w(16, j)),
            std::array::from_fn(|i| w(i, 0)),
            std::array::from_fn(|i| w(i, 16)),
        ]);
        let Some(t) = tables.get(&key.region()) else {
            return (e.did, c);
        };
        for &word in data.coarse.iter() {
            match t.lookup(word) {
                VertexLookup::NoPair => c.no_pair += 1,
                VertexLookup::Entry(entry) => {
                    let tr: Vec<_> = entry
                        .choices
                        .iter()
                        .map(|ch| scene_traits(ch.scene))
                        .collect();
                    c.group += u64::from(tr.iter().any(|t| t.0));
                    c.frill += u64::from(tr.iter().any(|t| t.1));
                    c.frill_nonempty += u64::from(tr.iter().any(|t| t.2));
                }
                _ => {}
            }
        }
        (e.did, c)
    });
    eprintln!(
        "{} landblock-data records read in {:.1} s",
        results.len(),
        start.elapsed().as_secs_f64()
    );
    // SC6: bits 15–31.
    let high: u32 = results.iter().map(|(_, c)| c.high_bits).sum();
    assert_eq!(high, 0, "SC6: coarse words with bits 15–31 set");
    // SC6: shared edges between neighbours that both have a non-zero grid.
    let edges: HashMap<u32, [[u32; 17]; 4]> = results
        .iter()
        .filter_map(|(d, c)| c.edges.map(|e| (*d, e)))
        .collect();
    let (mut ew, mut ew_bad, mut ns, mut ns_bad) = (0u64, 0u64, 0u64, 0u64);
    for (&did, e) in &edges {
        if let Some(east) = edges.get(&(did + 0x100)).filter(|_| did & 0xFF00 != 0xFF00) {
            ew += 1;
            ew_bad += u64::from(e[1] != east[0]);
        }
        if let Some(north) = edges.get(&(did + 1)).filter(|_| did & 0xFF != 0xFF) {
            ns += 1;
            ns_bad += u64::from(e[3] != north[2]);
        }
    }
    eprintln!(
        "SC6: east-west pairs {ew} ({ew_bad} differ), south-north pairs {ns} ({ns_bad} differ)"
    );
    assert_eq!(ew_bad, 0, "SC6: east-west edge words");
    assert_eq!(ns_bad, 0, "SC6: south-north edge words");

    // §12 census.
    let mut per: BTreeMap<u8, (u64, u64, u64, u64, u64)> = BTreeMap::new();
    for (_, c) in results.iter().filter(|(_, c)| c.nonzero) {
        let p = per.entry(c.region).or_default();
        p.0 += 1;
        p.1 += c.group;
        p.2 += c.frill;
        p.3 += c.frill_nonempty;
        p.4 += c.no_pair;
    }
    let mut tot = (0u64, 0u64, 0u64, 0u64, 0u64);
    for (r, p) in &per {
        eprintln!(
            "  region {r}: {} blocks, {} group-scene vertices, {} frill-layer vertices ({} with \
             children), {} without a scene",
            p.0, p.1, p.2, p.3, p.4
        );
        tot = (
            tot.0 + p.0,
            tot.1 + p.1,
            tot.2 + p.2,
            tot.3 + p.3,
            tot.4 + p.4,
        );
    }
    eprintln!(
        "§12 census: {} blocks, {} group-scene vertices, {} frill-layer vertices ({} with \
         children), {} without a scene",
        tot.0, tot.1, tot.2, tot.3, tot.4
    );
    // The census is an observation of one install (§0, §12): printed, not asserted.
    assert!(tot.0 > 0, "no landblock has a non-zero coarse grid");
}

/// FNV-1a over the bit patterns of a landblock's objects.
fn digest(objects: &[tessera_world::scenery::SceneryObject]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    let mut eat = |v: u32| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    for o in objects {
        eat(o.object.0);
        o.position.iter().for_each(|c| eat(c.to_bits()));
        o.rotation.to_wxyz().iter().for_each(|c| eat(c.to_bits()));
        o.scale.iter().for_each(|c| eat(c.to_bits()));
    }
    h
}

/// The static list of landblock `key` (§7.4 step 14a), empty without a landblock-info
/// record.
fn static_list(
    ctx: &WorldContext,
    key: LandblockKey,
    pids: &ClearancePids,
    objects: &ObjectPropertyCache,
) -> Vec<StaticClearance> {
    match ctx
        .source()
        .read_stamped(key.info_did())
        .expect("info read")
    {
        Some((b, stamp)) => {
            let info = LandblockInfo::parse_expecting(&b, key.info_did(), stamp, ctx.schema())
                .expect("info decodes");
            static_clearances(&info, &**ctx.source(), ctx.schema(), pids, objects)
                .expect("static list")
        }
        None => Vec::new(),
    }
}

struct Generator<'a> {
    set: &'a ArchiveSet,
    ctx: &'a WorldContext,
    tables: &'a BTreeMap<u8, Arc<SceneryTables>>,
    scenes: SceneLibrary,
    objects: ObjectPropertyCache,
    pids: ClearancePids,
}

impl Generator<'_> {
    /// Generates one landblock: `None` when its coarse grid is all zero.
    /// Returns (DID, digest, objects, stats, static entities, static volumes).
    fn run(&self, e: &Entry) -> Option<(u32, u64, usize, SceneryStats, usize, usize)> {
        let data = read_data(self.set, e);
        if data.coarse.iter().all(|w| *w == 0) {
            return None;
        }
        let key = LandblockKey::from_did(e.did).expect("key");
        let tables = self.tables.get(&key.region())?;
        let statics = static_list(self.ctx, key, &self.pids, &self.objects);
        let terrain = GridMesh::terrain(&data);
        let input = SceneryInput {
            key,
            data: &data,
            terrain: &terrain,
            tables,
            scenes: &self.scenes,
            water: &NoWater,
            statics: &statics,
            limits: SceneryLimits::default(),
        };
        let g = generate_landblock(&input);
        assert!(g.stats.is_complete());
        Some((
            e.did,
            digest(&g.objects),
            g.objects.len(),
            g.stats,
            statics.len(),
            statics.iter().map(|s| s.volume_count()).sum(),
        ))
    }
}

#[test]
fn generation_is_deterministic() {
    let Some((set, ctx)) = open() else { return };
    let tables = SceneryTables::for_all_regions(&ctx);
    let pids = ClearancePids::from_context(&ctx).expect("dictionary");
    eprintln!("clearance PIDs: {pids:?}");
    let entries = data_entries(&set);
    let make = || Generator {
        set: &set,
        ctx: &ctx,
        tables: &tables,
        scenes: SceneLibrary::new(Arc::clone(ctx.source())),
        objects: ObjectPropertyCache::default(),
        pids,
    };

    let start = Instant::now();
    let one = make();
    let a: Vec<_> = entries.iter().filter_map(|e| one.run(e)).collect();
    let t1 = start.elapsed();
    let start = Instant::now();
    let many = make();
    let reversed: Vec<Entry> = entries.iter().rev().cloned().collect();
    let b: Vec<_> = par_map(&reversed, workers(), |e| many.run(e))
        .into_iter()
        .flatten()
        .collect();
    let tn = start.elapsed();

    let ma: BTreeMap<u32, (u64, usize)> = a.iter().map(|r| (r.0, (r.1, r.2))).collect();
    let mb: BTreeMap<u32, (u64, usize)> = b.iter().map(|r| (r.0, (r.1, r.2))).collect();
    assert_eq!(ma.len(), mb.len());
    let differ = ma.iter().filter(|(k, v)| mb.get(k) != Some(v)).count();
    let stats = sum_stats(a.iter().map(|r| &r.3));
    let objects: usize = a.iter().map(|r| r.2).sum();
    let statics: usize = a.iter().map(|r| r.4).sum();
    let max_volumes = a.iter().map(|r| r.5).max().unwrap_or(0);
    eprintln!(
        "  static volumes: {} in all, at most {max_volumes} per landblock (cap {MAX_STATIC_VOLUMES})",
        a.iter().map(|r| r.5).sum::<usize>()
    );
    assert!(
        max_volumes < MAX_STATIC_VOLUMES,
        "the cap would cut real data"
    );
    eprintln!(
        "{} landblocks: {objects} objects; 1 worker {:.1} s, {} workers {:.1} s; {differ} differ",
        ma.len(),
        t1.as_secs_f64(),
        workers(),
        tn.as_secs_f64()
    );
    eprintln!(
        "  vertices with a scene {}, without a pair {}, missing keys {}, missing scenes {}",
        stats.vertices_with_scene,
        stats.vertices_without_pair,
        stats.missing_keys,
        stats.missing_scenes
    );
    eprintln!(
        "  nodes tested {}, placed {}, rejected {:?}; static clearance entities {statics}",
        stats.nodes_tested, stats.nodes_placed, stats.rejected
    );
    assert_eq!(differ, 0, "SC8: generation differs between runs");
    assert!(
        !stats.truncated(),
        "a hostile-input bound was hit: {stats:?}"
    );
    assert_eq!(stats.missing_scenes, 0);
    assert_eq!(
        stats.failed_vertices, 0,
        "no record of the install makes a child fail (§7.4 step 15)"
    );
    assert!(!ma.is_empty(), "no landblock generated");
}

#[test]
fn frill_sectors() {
    let Some((set, ctx)) = open() else { return };
    let tables = SceneryTables::for_all_regions(&ctx);
    let scenes = SceneLibrary::new(Arc::clone(ctx.source()));
    let pids = ClearancePids::from_context(&ctx).expect("dictionary");
    let objects = ObjectPropertyCache::default();
    let entries = data_entries(&set);
    let step = if full() { 97 } else { 7 };
    let (mut blocks, mut instances, mut late, mut outside) = (0usize, 0usize, 0u64, 0u64);
    let mut kinds = BTreeSet::new();
    let start = Instant::now();
    for e in entries.iter().step_by(step) {
        let data = read_data(&set, e);
        if data.coarse.iter().all(|w| *w == 0) {
            continue;
        }
        let key = LandblockKey::from_did(e.did).expect("key");
        let Some(t) = tables.get(&key.region()) else {
            continue;
        };
        let terrain = GridMesh::terrain(&data);
        let statics = static_list(&ctx, key, &pids, &objects);
        let input = SceneryInput {
            key,
            data: &data,
            terrain: &terrain,
            tables: t,
            scenes: &scenes,
            water: &NoWater,
            statics: &statics,
            limits: SceneryLimits::default(),
        };
        blocks += 1;
        // §8.2 item 5: the frill pass is spaced against the static list and the markers of
        // the static pass run with the same input.
        let markers = generate_landblock(&input).spacing_markers();
        let mut thinner = DensityThinner::new();
        for sx in 0..16 {
            for sy in 0..16 {
                let s = generate_sector(&input, &markers, sx, sy, 1.0, &mut thinner);
                let again = generate_sector(&input, &markers, sx, sy, 1.0, &mut thinner);
                assert_eq!(s, again, "density 1 is deterministic");
                assert!(!s.stats.truncated());
                assert_eq!(
                    (s.stats.failed_vertices, s.stats.failed_frill_layers),
                    (0, 0),
                    "no record of the install makes a child fail (§7.4 step 15)"
                );
                for i in &s.instances {
                    assert!((0.0..160.0).contains(&i.position[0]));
                    assert!((0.0..160.0).contains(&i.position[1]));
                    kinds.insert(i.object);
                }
                instances += s.instances.len();
                late += u64::from(s.late_discarded);
                outside += u64::from(s.outside);
            }
        }
    }
    eprintln!(
        "{blocks} landblocks: {instances} frill instances of {} objects, {late} late records \
         discarded, {outside} outside their landblock ({:.1} s)",
        kinds.len(),
        start.elapsed().as_secs_f64()
    );
    assert!(blocks > 0 && instances > 0);
}

/// Rows S1–S11 of `m4-5-visual.md` §4.2: (row, region, landblock, generated objects).
const S_TOTALS: [(&str, u8, u16, usize); 11] = [
    ("S1", 1, 0x669A, 25),
    ("S2", 1, 0xA197, 315),
    ("S3", 2, 0xAEBC, 165),
    ("S4", 2, 0x7744, 309),
    ("S5", 3, 0x79AD, 151),
    ("S6", 3, 0x7A69, 111),
    ("S7", 4, 0x1A23, 445),
    ("S8", 4, 0x7ADD, 166),
    ("S9", 5, 0x655A, 221),
    ("S10", 5, 0x6FD9, 80),
    ("S11", 14, 0x1FE2, 186),
];

/// The object of a walk of `m4-5-visual.md` §2.3.
struct WalkObject {
    walk: &'static str,
    region: u8,
    block: u16,
    /// 0-based index in the landblock's generated list.
    index: usize,
    did: u32,
    position: [f32; 3],
    /// Uniform scale.
    scale: f32,
}

/// Walks GN1–GN4 of `m4-5-visual.md` §2.3.
const GN_OBJECTS: [WalkObject; 4] = [
    WalkObject {
        walk: "GN1",
        region: 1,
        block: 0x669A,
        index: 16,
        did: 0x4700_35F4,
        position: [130.612, 83.519, 559.263],
        scale: 1.1701,
    },
    WalkObject {
        walk: "GN2",
        region: 4,
        block: 0x1A23,
        index: 33,
        did: 0x4700_988D,
        position: [40.970, 46.965, 192.134],
        scale: 1.0176,
    },
    WalkObject {
        walk: "GN3",
        region: 5,
        block: 0x655A,
        index: 176,
        did: 0x4700_D780,
        position: [123.961, 37.660, 440.486],
        scale: 1.0304,
    },
    WalkObject {
        walk: "GN4",
        region: 14,
        block: 0x1FE2,
        index: 138,
        did: 0x4700_3B84,
        position: [117.026, 130.965, 435.875],
        scale: 0.9556,
    },
];

fn row_key(region: u8, block: u16) -> LandblockKey {
    let [x, y] = block.to_be_bytes();
    LandblockKey::new(region, x, y).expect("row landblock")
}

#[test]
fn checklist_generated_lists() {
    let Some((_set, ctx)) = open() else { return };
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let bundle = |region: u8, block: u16| {
        let key = row_key(region, block);
        let frame = tessera_assets::RecordSource::read_record(&src, key.data_did().0)
            .expect("bundle frame");
        build_bundle(&ctx, key.data_did(), &frame, &|| false).expect("bundle")
    };
    let mut failures = Vec::new();
    let mut bundles = HashMap::new();
    for (row, region, block, total) in S_TOTALS {
        let b = bundle(region, block);
        let scenery = b.scenery.as_ref().expect("scenery generated");
        let stats = &scenery.stats;
        eprintln!(
            "{row}: {} generated objects (expected {total}), {} closed markers able to reject, \
             {} footprint rejections",
            b.generated.len(),
            scenery.markers.len(),
            stats.rejected[tessera_world::scenery::Rejection::Footprint.index()]
        );
        if b.generated.len() != total {
            failures.push(format!("{row}: {} objects, not {total}", b.generated.len()));
        }
        if stats.failed_vertices != 0 || !stats.is_complete() || stats.truncated() {
            failures.push(format!("{row}: generation incomplete or failed: {stats:?}"));
        }
        if !scenery.complete {
            failures.push(format!("{row}: the bundle's scenery is marked incomplete"));
        }
        bundles.insert((region, block), b);
    }
    for o in &GN_OBJECTS {
        let (walk, index, did, pos, scale) = (o.walk, o.index, o.did, o.position, o.scale);
        let b = &bundles[&(o.region, o.block)];
        match b.generated.get(index) {
            Some(e)
                if e.object == Some(Did(did))
                    && (0..3).all(|k| (e.placement.position[k] - pos[k]).abs() <= 0.01)
                    && (e.scale[0] - scale).abs() <= 0.0001 =>
            {
                eprintln!("{walk}: {did:#010x} at list index {index}");
            }
            other => failures.push(format!(
                "{walk}: index {index} holds {:?} at {:?}, not {did:#010x} at {pos:?}",
                other.and_then(|e| e.object),
                other.map(|e| e.placement.position)
            )),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
