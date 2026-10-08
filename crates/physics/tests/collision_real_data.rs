// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-physics` against a user's own install (read-only): the invariants CO1–CO11 of
//! `docs/specs/collision.md` §13 as far as they concern collision geometry, the §9.3
//! blocker census, and collision sets built from real landblocks.
//!
//! - **grids** (C12, C20, C21, CO7, CO10, CO11): every landblock's terrain mesh numbers
//!   its triangles as the stored terrain trees do (the point query at each triangle's
//!   centroid reports the triangle's key), likewise every ceiling mesh with its ceiling
//!   tree; every stored ceiling-chunk triangle is a triangle of the ceiling mesh (bit-0
//!   diagonal, clockwise from above: 5,310,848); the hole census (2,750,134 triangles
//!   covered by no layer, all in landblocks with a ceiling, never under an open cave);
//! - **meshes** (CO1, CO4, CO5, CO8): every 0x66 record decodes with its stamp, its tree
//!   walks with keys below its triangle count, origin and scale follow §3.2, and its shape
//!   template keeps every triangle; the setup census of §8 and the physics bounds of
//!   mesh-only and volume-only setups against the templates;
//! - **cells** (CO2, CO3, §5.4, §5.8.1): every 0x01 volume holds its hull vertices within
//!   5.2 mm, every landblock-info cell links its portals reciprocally, and the cell lookup
//!   finds a cell at the centre of every placed cell;
//! - **movement record** (CO9);
//! - **blockers** (§9.3, CO12): 82,139 undrawn mesh blockers among 97,688 undrawn blocking
//!   SEDs in the data census, and the run-time sets with the content-layer test and the
//!   creation-type 7/8 drop;
//! - **sets**: full collision sets of sampled landblocks, with sweeps.
//!
//! Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-physics --test collision_real_data -- --nocapture`. A
//! debug build visits a sample unless `TESSERA_FULL=1`. Only counts, DIDs and error kinds
//! are printed; no game data leaves the install.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::collision::{
    CELL_GEOMETRY_PREFIX, COLLISION_MESH_PREFIX, CellGeometry, CollisionMesh, TreeCode,
    quantisation_for_bounds,
};
use tessera_formats::landblock::{GridSurface, LayerKind, StaticEntityType};
use tessera_formats::setup::{ENTITY_PREFIX, SETUP_PREFIX};
use tessera_formats::{
    Did, EntityDescriptor, LandblockData, LandblockInfo, PropertyDictionary, PropertySet,
    PropertySetRecord, PropertyValue, Record, Setup,
};
use tessera_physics::geometry::{
    Capsule, CollisionFlags, CollisionLimits, CollisionSetBuilder, ETHEREAL_PLAYER, FetchedRecord,
    GRID_TRIANGLES, GeneratedObject, GridMesh, ObjectCollision, ObjectCollisionCache, PhysicsPids,
    QueryFilter, SetupCollision, ShapeTemplate, StaticOptions, Transform, content_layer_list,
    content_layers_pass, resolve_object_collision, static_blocks_walker,
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

fn dictionary(set: &ArchiveSet) -> &'static PropertyDictionary {
    static DICT: OnceLock<PropertyDictionary> = OnceLock::new();
    DICT.get_or_init(|| {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    })
}

fn full() -> bool {
    std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || full()
}

fn sample<T: Clone>(v: &[T], every: usize) -> Vec<T> {
    v.iter()
        .enumerate()
        .filter(|(i, _)| i.is_multiple_of(every))
        .map(|(_, e)| e.clone())
        .collect()
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

/// (member, entry) of every record with prefix `prefix` in archives of `kind` (first
/// archive wins).
fn entries_of(
    set: &ArchiveSet,
    prefix: u8,
    kind: impl Fn(&ArchiveKind) -> bool,
) -> Vec<(usize, Entry)> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for (i, m) in set.members().iter().enumerate() {
        if !kind(&m.id.kind) {
            continue;
        }
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            out.entry(e.did).or_insert((i, e));
        }
    }
    out.into_values().collect()
}

/// (member, entry) of every landblock record of kind `k` in the cell archives.
fn landblock_entries(set: &ArchiveSet, k: u32) -> Vec<(usize, Entry)> {
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        let lo = 0x8000_0000 | (k << 20);
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            out.push((i, e.expect("directory walk")));
        }
    }
    out
}

fn read(set: &ArchiveSet, member: usize, e: &Entry) -> Vec<u8> {
    set.member(member)
        .archive
        .read_entry(e)
        .unwrap_or_else(|err| panic!("{:#010x}: read: {err}", e.did))
}

/// A fetch function over the whole set for the resolver.
fn fetch(set: &ArchiveSet, did: Did) -> Result<Option<FetchedRecord>, String> {
    match set.read_with_entry(did.0) {
        Ok((l, bytes)) => Ok(Some(FetchedRecord {
            bytes,
            stamp: Some(l.entry.stamp),
        })),
        Err(tessera_dat::Error::NotFound { .. }) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------------------
// Grids: terrain and ceiling meshes, ceiling chunks, holes.

#[derive(Default, Debug, Clone, Copy)]
struct GridTally {
    records: usize,
    with_ceiling: usize,
    terrain_trees: usize,
    ceiling_trees: usize,
    tree_queries: usize,
    tree_misses: usize,
    terrain_key_sets_complete: usize,
    ceiling_chunks: usize,
    stored_ceiling_triangles: usize,
    ceiling_triangles_matching: usize,
    ceiling_triangles_clockwise: usize,
    hole_landblocks: usize,
    hole_triangles: usize,
    holes_without_ceiling: usize,
    holes_ceiling_above_terrain: usize,
    holes_ceiling_all_zero: usize,
    layered_landblocks: usize,
}

impl GridTally {
    fn add(&mut self, o: &Self) {
        macro_rules! sum {
            ($($f:ident),*) => { $(self.$f += o.$f;)* };
        }
        sum!(
            records,
            with_ceiling,
            terrain_trees,
            ceiling_trees,
            tree_queries,
            tree_misses,
            terrain_key_sets_complete,
            ceiling_chunks,
            stored_ceiling_triangles,
            ceiling_triangles_matching,
            ceiling_triangles_clockwise,
            hole_landblocks,
            hole_triangles,
            holes_without_ceiling,
            holes_ceiling_above_terrain,
            holes_ceiling_all_zero,
            layered_landblocks
        );
    }
}

/// The point query at the centroid of every triangle of `mesh` must report its key.
fn check_tree(
    did: u32,
    mesh: &GridMesh,
    code: &TreeCode<'_>,
    every: usize,
) -> (usize, usize, bool) {
    let walk = code
        .walk()
        .unwrap_or_else(|e| panic!("{did:#010x}: {:?} tree walk: {e}", mesh.surface()));
    let complete = walk.keys.len() == GRID_TRIANGLES
        && walk.keys.iter().enumerate().all(|(i, &k)| k as usize == i);
    let (mut queries, mut misses) = (0, 0);
    for (key, t) in mesh.triangles().step_by(every) {
        let c = std::array::from_fn(|a| (t[0][a] + t[1][a] + t[2][a]) / 3.0);
        let keys = code
            .point_keys(c)
            .unwrap_or_else(|e| panic!("{did:#010x}: point query: {e}"));
        queries += 1;
        if !keys.contains(&u32::from(key)) {
            misses += 1;
        }
    }
    (queries, misses, complete)
}

fn grid_tally(did: u32, data: &LandblockData, every: usize) -> GridTally {
    let mut t = GridTally {
        records: 1,
        ..GridTally::default()
    };
    let terrain = GridMesh::terrain(data);
    assert_eq!(terrain.triangles().count(), GRID_TRIANGLES);
    if let Some(tree) = &data.terrain_tree {
        t.terrain_trees += 1;
        let (q, m, complete) = check_tree(did, &terrain, &TreeCode::from(tree), every);
        t.tree_queries += q;
        t.tree_misses += m;
        t.terrain_key_sets_complete += usize::from(complete);
    }
    let ceiling = GridMesh::ceiling(data);
    if let (Some(c), Some(mesh)) = (&data.ceiling, &ceiling) {
        t.with_ceiling += 1;
        if let Some(tree) = &c.tree {
            t.ceiling_trees += 1;
            let (q, m, _) = check_tree(did, mesh, &TreeCode::from(tree), every);
            t.tree_queries += q;
            t.tree_misses += m;
        }
        // CO10: every stored ceiling-chunk triangle is a triangle of the ceiling mesh.
        let mut by_set: HashMap<[u16; 3], (u16, [u16; 3])> = HashMap::with_capacity(4096);
        for key in 0..GRID_TRIANGLES as u16 {
            let v = mesh.triangle_vertices(key).expect("key");
            let mut s = v;
            s.sort_unstable();
            by_set.insert(s, (key, v));
        }
        if !c.surface.indices.is_empty() {
            t.ceiling_chunks += 1;
        }
        for tri in c.surface.indices.as_chunks::<3>().0 {
            t.stored_ceiling_triangles += 1;
            let mut s = *tri;
            s.sort_unstable();
            if let Some((_, v)) = by_set.get(&s) {
                t.ceiling_triangles_matching += 1;
                let rot = |k: usize| [v[k], v[(k + 1) % 3], v[(k + 2) % 3]];
                if (0..3).any(|k| rot(k) == *tri) {
                    t.ceiling_triangles_clockwise += 1;
                }
            }
        }
    }
    // CO11: holes in landblocks whose terrain chunk has layers.
    if !data.terrain.layers.is_empty() {
        t.layered_landblocks += 1;
        let mut by_set: HashMap<[u16; 3], u16> = HashMap::with_capacity(4096);
        for key in 0..GRID_TRIANGLES as u16 {
            let mut s = terrain.triangle_vertices(key).expect("key");
            s.sort_unstable();
            by_set.insert(s, key);
        }
        let mut covered = vec![false; GRID_TRIANGLES];
        for layer in &data.terrain.layers {
            if layer.kind() == LayerKind::Whole {
                covered.iter_mut().for_each(|c| *c = true);
                continue;
            }
            for tri in data
                .layer_triangles(GridSurface::Terrain, layer)
                .unwrap_or_default()
            {
                let mut s = tri;
                s.sort_unstable();
                if let Some(&k) = by_set.get(&s) {
                    covered[usize::from(k)] = true;
                }
            }
        }
        let holes: Vec<u16> = (0..GRID_TRIANGLES as u16)
            .filter(|&k| !covered[usize::from(k)])
            .collect();
        if !holes.is_empty() {
            t.hole_landblocks += 1;
            t.hole_triangles += holes.len();
            match &data.ceiling {
                None => t.holes_without_ceiling += holes.len(),
                Some(c) => {
                    for k in holes {
                        let v = terrain.triangle_vertices(k).expect("key");
                        let raw = |g: &tessera_formats::landblock::HeightGrid, n: u16| {
                            let (i, j) = (usize::from(n % 33), usize::from(n / 33));
                            u32::from(g.get(i, j).unwrap_or(0))
                        };
                        let cs: u32 = v.iter().map(|&n| raw(&c.heights, n)).sum();
                        let ts: u32 = v.iter().map(|&n| raw(&data.heights, n)).sum();
                        if cs > ts {
                            t.holes_ceiling_above_terrain += 1;
                        }
                        if cs == 0 {
                            t.holes_ceiling_all_zero += 1;
                        }
                    }
                }
            }
        }
    }
    t
}

#[test]
fn grids_terrain_ceiling_and_holes() {
    let Some(set) = open_set() else { return };
    let started = Instant::now();
    let mut entries = landblock_entries(set, 0);
    let every = if exhaustive() { 1 } else { 16 };
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let results = par_each(&entries, |(member, e)| {
        let bytes = read(set, *member, e);
        let data = LandblockData::parse(&bytes, Some(e.stamp))
            .unwrap_or_else(|err| panic!("{:#010x}: landblock data: {err}", e.did));
        grid_tally(e.did, &data, every)
    });
    let mut t = GridTally::default();
    for r in &results {
        t.add(r);
    }
    eprintln!("grids: {t:#?} in {:.1} s", started.elapsed().as_secs_f64());
    // C12 / C21: the numbering of §6.2 (terrain bit 7, ceiling bit 0) is the trees'.
    assert_eq!(t.tree_misses, 0, "centroid queries that miss their key");
    // CO7.
    assert_eq!(t.terrain_key_sets_complete, t.terrain_trees, "CO7");
    // CO10.
    assert_eq!(
        t.ceiling_triangles_matching, t.stored_ceiling_triangles,
        "CO10 quads"
    );
    assert_eq!(
        t.ceiling_triangles_clockwise, t.stored_ceiling_triangles,
        "CO10 winding"
    );
    // CO11.
    assert_eq!(
        t.holes_without_ceiling, 0,
        "CO11: holes only under a ceiling"
    );
    assert_eq!(
        t.holes_ceiling_above_terrain, 0,
        "CO11: never under an open cave"
    );
    if exhaustive() {
        assert_eq!(t.records, 390_150, "landblock-data records");
        assert_eq!(t.terrain_trees, 58_546, "terrain trees");
        assert_eq!(t.ceiling_trees, 1_095, "ceiling trees");
        assert_eq!(
            t.stored_ceiling_triangles, 5_310_848,
            "stored ceiling triangles"
        );
        assert_eq!(t.ceiling_chunks, 2_068, "ceiling chunks with triangles");
        assert_eq!(t.hole_triangles, 2_750_134, "hole triangles");
        assert_eq!(t.hole_landblocks, 2_368, "landblocks with holes");
        assert_eq!(
            t.holes_ceiling_all_zero, 413_655,
            "holes with no ceiling at all"
        );
    }
}

// ---------------------------------------------------------------------------------------
// 0x66 meshes and setups.

#[test]
fn collision_meshes_and_setups() {
    let Some(set) = open_set() else { return };
    let started = Instant::now();
    let general = |k: &ArchiveKind| *k == ArchiveKind::General;
    let mut meshes = entries_of(set, COLLISION_MESH_PREFIX, general);
    if !exhaustive() {
        meshes = sample(&meshes, 23);
    }
    let bounds: HashMap<u32, ([f32; 3], [f32; 3])> = par_each(&meshes, |(member, e)| {
        let bytes = read(set, *member, e);
        // CO1.
        let m = CollisionMesh::parse(&bytes, Some(e.stamp))
            .unwrap_or_else(|err| panic!("{:#010x}: CO1: {err}", e.did));
        assert_eq!(
            m.tree.build_byte.is_some(),
            e.stamp >= 38,
            "{:#010x}: CO1 build_byte vs stamp {}",
            e.did,
            e.stamp
        );
        // CO4.
        let walk = m
            .tree_code()
            .walk()
            .unwrap_or_else(|err| panic!("{:#010x}: CO4: {err}", e.did));
        assert!(
            walk.keys_below(m.triangles.len() as u64),
            "{:#010x}: CO4 keys",
            e.did
        );
        // CO5 over the vertices the triangles use.
        let used = m
            .triangles
            .iter()
            .flatten()
            .map(|&i| m.vertices[usize::from(i)]);
        let b = tessera_physics::geometry::Aabb::from_points(used).expect("bounds");
        let (origin, scale) = quantisation_for_bounds(b.min, b.max);
        for (o, stored) in origin.iter().zip(m.tree.origin) {
            assert!(
                (o - f64::from(stored)).abs() < 1e-4,
                "{:#010x}: CO5 origin",
                e.did
            );
        }
        assert!(
            ((scale - f64::from(m.tree.scale)) / scale).abs() < 1e-5,
            "{:#010x}: CO5 scale",
            e.did
        );
        // The template keeps every triangle, in order.
        let t = ShapeTemplate::from_collision_mesh(&m).expect("template");
        assert_eq!(t.triangles.len(), m.triangles.len());
        assert_eq!(t.dropped, 0);
        let all = m.bounds().expect("vertex bounds");
        (e.did, all)
    })
    .into_iter()
    .collect();
    eprintln!("0x66: {} records decoded and templated", bounds.len());

    // Setups: §8 census, CO8, and templates against the physics bounds.
    let mut setups = entries_of(set, SETUP_PREFIX, |_| true);
    if !exhaustive() {
        setups = sample(&setups, 23);
    }
    #[derive(Default)]
    struct SetupTally {
        kinds: BTreeMap<&'static str, usize>,
        co8_checked: usize,
        co8_failed: usize,
        volume_checked: usize,
        volume_failed: usize,
    }
    let tallies = par_each(&setups, |(member, e)| {
        let bytes = read(set, *member, e);
        let s = Setup::parse_expecting(&bytes, Did(e.did))
            .unwrap_or_else(|err| panic!("{:#010x}: setup: {err}", e.did));
        let mesh = !s.collision_mesh.is_null();
        let vol = [
            !s.spheres.is_empty(),
            !s.boxes.is_empty(),
            !s.capsules.is_empty(),
        ];
        let kind = match (mesh, vol) {
            (true, [false, false, false]) => "mesh only",
            (false, [true, false, false]) => "sphere only",
            (false, [false, true, false]) => "box only",
            (false, [false, false, true]) => "capsule only",
            (false, [false, false, false]) => "none",
            _ => "mixture",
        };
        let mut t = SetupTally::default();
        *t.kinds.entry(kind).or_default() += 1;
        let close = |a: [f32; 3], b: [f32; 3], eps: f32| (0..3).all(|k| (a[k] - b[k]).abs() <= eps);
        let pb = s.physics_bounds.aabb;
        if kind == "mesh only"
            && let Some((lo, hi)) = bounds.get(&s.collision_mesh.0)
        {
            t.co8_checked += 1;
            if !(close(pb.min, *lo, 1e-3) && close(pb.max, *hi, 1e-3)) {
                t.co8_failed += 1;
            }
        }
        if !mesh && kind != "none" {
            let tpl = ShapeTemplate::from_setup(&s, None).expect("template");
            let b = tpl.bounds().expect("bounds");
            t.volume_checked += 1;
            if !(close(pb.min, b.min, 2e-3) && close(pb.max, b.max, 2e-3)) {
                t.volume_failed += 1;
            }
        }
        // CO4 for setups: SetupCollision agrees with has_collision.
        assert_eq!(SetupCollision::of(&s).has_collision(), s.has_collision());
        t
    });
    let mut total = SetupTally::default();
    for t in tallies {
        for (k, n) in t.kinds {
            *total.kinds.entry(k).or_default() += n;
        }
        total.co8_checked += t.co8_checked;
        total.co8_failed += t.co8_failed;
        total.volume_checked += t.volume_checked;
        total.volume_failed += t.volume_failed;
    }
    eprintln!(
        "setups: {:?}; CO8 {} checked, {} failed; volume templates {} checked, {} differ from \
         the physics bounds; {:.1} s",
        total.kinds,
        total.co8_checked,
        total.co8_failed,
        total.volume_checked,
        total.volume_failed,
        started.elapsed().as_secs_f64()
    );
    assert_eq!(total.co8_failed, 0, "CO8");
    assert_eq!(total.volume_failed, 0, "volume templates vs physics bounds");
    if exhaustive() {
        assert_eq!(bounds.len(), 16_819, "0x66 records");
        assert_eq!(total.co8_checked, 16_812, "CO8 setups");
        let k = |n: &str| total.kinds.get(n).copied().unwrap_or(0);
        assert_eq!(
            [
                k("mesh only"),
                k("sphere only"),
                k("box only"),
                k("capsule only"),
                k("none"),
                k("mixture")
            ],
            [16_812, 1_910, 990, 715, 8_117, 11],
            "§8 volume census"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Cells.

fn cell_geometries(set: &'static ArchiveSet) -> &'static HashMap<Did, Arc<CellGeometry>> {
    static GEOMETRY: OnceLock<HashMap<Did, Arc<CellGeometry>>> = OnceLock::new();
    GEOMETRY.get_or_init(|| {
        let dict = dictionary(set);
        let entries = entries_of(set, CELL_GEOMETRY_PREFIX, |k| *k == ArchiveKind::General);
        par_each(&entries, |(member, e)| {
            let bytes = read(set, *member, e);
            let g = CellGeometry::parse_expecting(&bytes, e.did, Some(e.stamp), dict)
                .unwrap_or_else(|err| panic!("{:#010x}: CO2: {err}", e.did));
            (Did(e.did), Arc::new(g))
        })
        .into_iter()
        .collect()
    })
}

#[test]
fn cells_volumes_links_and_lookup() {
    let Some(set) = open_set() else { return };
    let started = Instant::now();
    let dict = dictionary(set);
    let geometries = cell_geometries(set);
    // CO2 / §5.4: hull vertices inside their volume within 5.2 mm.
    let mut worst = 0.0f64;
    let mut physics_triangles = 0usize;
    for g in geometries.values() {
        let v = tessera_physics::geometry::CellVolume::from_planes(&g.planes).expect("volume");
        for h in &g.hull {
            let d = v.min_signed_distance(h.map(f64::from));
            worst = worst.max(-d);
        }
        if let Some(m) = &g.physics_mesh {
            // CO4 for cell physics meshes.
            let walk = m
                .tree_code()
                .walk()
                .unwrap_or_else(|e| panic!("{}: CO4 physics mesh walk: {e}", g.did));
            assert!(
                walk.keys_below(m.triangles.len() as u64),
                "{}: CO4 keys",
                g.did
            );
            let t = ShapeTemplate::from_collision_mesh(m).expect("template");
            assert_eq!(t.dropped, 0);
            physics_triangles += t.triangles.len();
        }
    }
    assert!(
        worst <= 0.0052,
        "a hull vertex lies {worst} outside its volume"
    );

    let mut entries = landblock_entries(set, 2);
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    #[derive(Default, Debug)]
    struct CellTally {
        infos_with_cells: usize,
        cells: usize,
        refused: usize,
        links: usize,
        centre_found_self: usize,
        centre_found_other: usize,
        centre_missed: usize,
        cell_triangles: usize,
    }
    let results = par_each(&entries, |(member, e)| {
        let mut t = CellTally::default();
        let bytes = read(set, *member, e);
        let info = LandblockInfo::parse_expecting(&bytes, e.did, Some(e.stamp), dict)
            .unwrap_or_else(|err| panic!("{:#010x}: landblock info: {err}", e.did));
        let cells = info.cells();
        if cells.is_empty() {
            return t;
        }
        t.infos_with_cells += 1;
        // CO3.
        let by_index: HashMap<u16, &tessera_formats::landblock::Cell> =
            cells.iter().map(|c| (c.index, c)).collect();
        for c in cells {
            let g = geometries.get(&c.geometry).expect("geometry exists");
            assert_eq!(c.neighbours.len(), g.portals.len(), "{:#010x}: CO3", e.did);
            for (k, n) in c.neighbours.iter().enumerate() {
                t.links += 1;
                let back = by_index
                    .get(&n.cell)
                    .and_then(|o| o.neighbours.get(usize::from(n.back_reference)))
                    .expect("back-reference");
                assert_eq!((back.cell, usize::from(back.back_reference)), (c.index, k));
            }
        }
        // The lookup of §5.8.1 at the centre of each placed cell.
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.add_cells(&info, &PhysicsPids::from_dictionary(dict), |did| {
            geometries.get(&did).cloned()
        });
        let s = b.build();
        t.cells = s.cells().len();
        t.refused = s.report().cells_refused as usize;
        t.cell_triangles = s.triangle_count();
        for c in s.cells().cells() {
            let g = &geometries[&c.geometry];
            let n = g.hull.len().max(1) as f64;
            let centre: [f64; 3] =
                std::array::from_fn(|a| g.hull.iter().map(|h| f64::from(h[a])).sum::<f64>() / n);
            #[allow(clippy::cast_possible_truncation)]
            let p = c.placement.to_landblock(centre).map(|v| v as f32);
            match s.cell_at(p) {
                Some(h) if h.cell == c.index => t.centre_found_self += 1,
                Some(_) => t.centre_found_other += 1,
                None => t.centre_missed += 1,
            }
        }
        t
    });
    let mut total = CellTally::default();
    for r in results {
        total.infos_with_cells += r.infos_with_cells;
        total.cells += r.cells;
        total.refused += r.refused;
        total.links += r.links;
        total.centre_found_self += r.centre_found_self;
        total.centre_found_other += r.centre_found_other;
        total.centre_missed += r.centre_missed;
        total.cell_triangles += r.cell_triangles;
    }
    eprintln!(
        "cells: {} geometries (worst hull vertex {worst:.5} m outside; {physics_triangles} \
         physics triangles); {total:?}; {:.1} s",
        geometries.len(),
        started.elapsed().as_secs_f64()
    );
    // Every cell centre lies in some cell: its own, or a lower-numbered cell overlapping it.
    assert!(
        total.centre_missed <= total.cells / 1000,
        "the lookup misses {} cell centres",
        total.centre_missed
    );
    if exhaustive() {
        assert_eq!(geometries.len(), 1_057, "0x01 records");
        assert_eq!(total.cells + total.refused, 104_271, "cells");
        assert_eq!(total.links, 393_842, "neighbour entries");
    }
}

// ---------------------------------------------------------------------------------------
// CO9: the movement record.

#[test]
fn co9_movement_record() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let player_pids: Vec<u32> = dict
        .names()
        .iter()
        .filter(|(_, n)| n.starts_with("PlayerPhysicsControl_"))
        .map(|(p, _)| *p)
        .collect();
    let movement_config = dict
        .pid("PlayerPhysicsControl_MovementConfiguration")
        .expect("movement configuration property");
    let allowed: Vec<u32> = [
        "ForwardMoveSpeed",
        "BackwardMoveSpeed",
        "StrafeLeftMoveSpeed",
        "StrafeRightMoveSpeed",
        "FlyUpMoveSpeed",
        "FlyDownMoveSpeed",
        "ControlAcceleration",
        "VelocityDamper",
        "RampTime",
        "Gravity",
        "PhysicsTerminalVelocity",
        "JumpAcceleration",
        "StepUpHeight",
        "FloatHeight",
        "ForceStrafe",
        "AllowContactLoss",
    ]
    .iter()
    .map(|n| {
        dict.pid(&format!("GenericPhysicsController_{n}"))
            .unwrap_or_else(|| panic!("§10.1 property {n} missing"))
    })
    .collect();
    let mut entries = entries_of(set, 0x79, |k| *k == ArchiveKind::Gamelogic);
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let found: Vec<(u32, usize, usize)> = par_each(&entries, |(member, e)| {
        let bytes = read(set, *member, e);
        let rec = PropertySetRecord::parse_expecting(&bytes, e.did, dict)
            .unwrap_or_else(|err| panic!("{:#010x}: property set: {err}", e.did));
        if !rec.set.iter().any(|en| player_pids.contains(&en.pid)) {
            return None;
        }
        // Every MovementConfiguration struct, wherever it sits, lists only §10.1 fields.
        let (mut configs, mut bad) = (0, 0);
        fn walk(
            v: &PropertyValue,
            target: u32,
            allowed: &[u32],
            configs: &mut usize,
            bad: &mut usize,
        ) {
            match v {
                PropertyValue::Struct(s) => {
                    for f in &s.fields {
                        if f.pid == target {
                            *configs += 1;
                            if let PropertyValue::Struct(c) = &f.value {
                                *bad += c
                                    .fields
                                    .iter()
                                    .filter(|x| !allowed.contains(&x.pid))
                                    .count();
                            }
                        }
                        walk(&f.value, target, allowed, configs, bad);
                    }
                }
                PropertyValue::Array(a) => {
                    for el in &a.elements {
                        walk(&el.value, target, allowed, configs, bad);
                    }
                }
                _ => {}
            }
        }
        for en in rec.set.iter() {
            walk(&en.value, movement_config, &allowed, &mut configs, &mut bad);
        }
        Some((e.did, configs, bad))
    })
    .into_iter()
    .flatten()
    .collect();
    eprintln!("CO9: sets with PlayerPhysicsControl_* properties: {found:x?}");
    if exhaustive() {
        assert_eq!(found.len(), 1, "CO9: exactly one 0x79 set");
    }
    for (did, configs, bad) in found {
        assert!(configs > 0, "{did:#010x}: no MovementConfiguration");
        assert_eq!(bad, 0, "{did:#010x}: CO9 fields outside §10.1");
    }
}

// ---------------------------------------------------------------------------------------
// §9.3: which placed objects block a walker.

/// Draw-suppression values (`entity-descriptor.md` §9.2 E22).
#[derive(Debug, Clone, Copy, Default)]
struct Draw {
    draw: Option<bool>,
    meshes: Option<bool>,
    never: Option<bool>,
    server: Option<bool>,
}

struct DrawPids {
    draw: Option<u32>,
    meshes: Option<u32>,
    never: Option<u32>,
    server: Option<u32>,
}

impl Draw {
    fn from_set(s: &PropertySet, p: &DrawPids) -> Self {
        let get = |pid: Option<u32>| match pid.and_then(|p| s.get(p)) {
            Some(PropertyValue::Bool(b)) => Some(*b),
            Some(PropertyValue::Int(i)) => Some(*i != 0),
            _ => None,
        };
        Self {
            draw: get(p.draw),
            meshes: get(p.meshes),
            never: get(p.never),
            server: get(p.server),
        }
    }
    fn or(self, o: Self) -> Self {
        Self {
            draw: self.draw.or(o.draw),
            meshes: self.meshes.or(o.meshes),
            never: self.never.or(o.never),
            server: self.server.or(o.server),
        }
    }
    fn suppressed(self) -> bool {
        let draw = self.server.or(self.draw).unwrap_or(true);
        !draw || self.meshes == Some(false) || self.never == Some(true)
    }
}

struct Resolved {
    collision: Arc<ObjectCollision>,
    draw: Draw,
}

fn resolve(
    set: &'static ArchiveSet,
    cache: &ObjectCollisionCache,
    draws: &Mutex<HashMap<Did, Draw>>,
    pids: &PhysicsPids,
    dpids: &DrawPids,
    object: Did,
) -> Resolved {
    let dict = dictionary(set);
    let collision = cache.get_or_resolve(object, |o| {
        resolve_object_collision(o, |d| fetch(set, d), dict, pids)
    });
    let cached = draws.lock().unwrap().get(&object).copied();
    let draw = cached.unwrap_or_else(|| {
        let mut d = Draw::default();
        for did in collision
            .chain
            .iter()
            .filter(|d| d.prefix() == ENTITY_PREFIX)
        {
            if let Ok(Some(rec)) = fetch(set, *did)
                && let Ok(e) = EntityDescriptor::parse_expecting(&rec.bytes, *did, dict)
            {
                d = d.or(Draw::from_set(&e.properties, dpids));
            }
        }
        draws.lock().unwrap().insert(object, d);
        d
    });
    Resolved { collision, draw }
}

#[test]
fn s9_3_blocker_classification() {
    let Some(set) = open_set() else { return };
    let started = Instant::now();
    let dict = dictionary(set);
    let pids = PhysicsPids::from_dictionary(dict);
    let dpids = DrawPids {
        draw: dict.pid("Render_Draw"),
        meshes: dict.pid("Render_DrawMeshes"),
        never: dict.pid("Render_NeverAddToScene"),
        server: dict.pid("Render_ServerDraw"),
    };
    let cache = ObjectCollisionCache::default();
    let draws = Mutex::new(HashMap::new());
    let mut entries = landblock_entries(set, 2);
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    type Row = (&'static str, bool, bool, bool);
    let results = par_each(&entries, |(member, e)| {
        let mut rows: HashMap<Row, usize> = HashMap::new();
        // Per set of §9.3 / CO12 (data census; content-layer test; and creation types 7
        // and 8 dropped): [blockers, undrawn blockers, undrawn mesh blockers].
        let mut blocking = [[0usize; 3]; 3];
        let mut unplaced = 0usize;
        // SEDs with an effective Physics_SurfacePropertiesToApply (extra sets included,
        // scenery.md §9.2 rule 1; collision.md §9.1 OPEN).
        let mut surfaced = 0usize;
        let bytes = read(set, *member, e);
        let info = LandblockInfo::parse_expecting(&bytes, e.did, Some(e.stamp), dict)
            .unwrap_or_else(|err| panic!("{:#010x}: landblock info: {err}", e.did));
        let instance: HashMap<u64, &PropertySet> = info
            .weenies
            .items
            .iter()
            .map(|w| (w.iid.0, &w.properties))
            .collect();
        for sed in &info.entities {
            if sed.kind() != Some(StaticEntityType::Physics) {
                continue;
            }
            if sed.placement.is_none() {
                unplaced += 1;
            }
            let Some(object) = sed.object else { continue };
            let r = resolve(set, &cache, &draws, &pids, &dpids, object);
            let own = sed
                .properties
                .as_ref()
                .map(|s| CollisionFlags::from_set(s, &pids))
                .unwrap_or_default();
            let flags = CollisionFlags::of_static(own, r.collision.flags);
            surfaced += usize::from(flags.surface_properties.is_some());
            let inst = sed.iid.and_then(|i| instance.get(&i.0).copied());
            let draw = inst
                .map(|s| Draw::from_set(s, &dpids))
                .unwrap_or_default()
                .or(sed
                    .properties
                    .as_ref()
                    .map(|s| Draw::from_set(s, &dpids))
                    .unwrap_or_default())
                .or(r.draw);
            let drawn = !draw.suppressed();
            let kind = match (
                r.collision.error.is_some() && r.collision.setup.is_none(),
                r.collision.collision,
            ) {
                (true, _) => "unresolved",
                (_, SetupCollision::Mesh) => "mesh",
                (_, SetupCollision::Volumes) => "volumes",
                (_, SetupCollision::None) => "none",
            };
            let ethereal = flags.passes(ETHEREAL_PLAYER);
            *rows
                .entry((kind, flags.has_no_physics(), drawn, ethereal))
                .or_default() += 1;
            let list = sed
                .properties
                .as_ref()
                .and_then(|s| content_layer_list(s, &pids));
            let layers = content_layers_pass(list.as_deref(), &[1], 0);
            let placed = sed.placement.is_some();
            let census = CollisionFlags {
                creation_type: None,
                ..flags
            };
            let mesh = r.collision.collision == SetupCollision::Mesh;
            for (k, (layers, flags)) in [(true, &census), (layers, &census), (layers, &flags)]
                .into_iter()
                .enumerate()
            {
                if static_blocks_walker(placed, layers, flags, r.collision.collision) {
                    blocking[k][0] += 1;
                    blocking[k][1] += usize::from(!drawn);
                    blocking[k][2] += usize::from(!drawn && mesh);
                }
            }
        }
        (rows, blocking, unplaced, surfaced)
    });
    let mut rows: BTreeMap<Row, usize> = BTreeMap::new();
    let mut blocking = [[0usize; 3]; 3];
    let mut unplaced = 0;
    let mut surfaced = 0;
    for (r, u, n, s) in results {
        surfaced += s;
        for (k, v) in r {
            *rows.entry(k).or_default() += v;
        }
        for (total, part) in blocking.iter_mut().zip(u) {
            for (t, p) in total.iter_mut().zip(part) {
                *t += p;
            }
        }
        unplaced += n;
    }
    let total: usize = rows.values().sum();
    eprintln!(
        "§9.3: {total} Physics SEDs ({unplaced} without a position), {} objects resolved; \
         rows (collision, NoPhysics, drawn, ethereal to Player):",
        cache.len()
    );
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in &sorted {
        eprintln!("  {k:?}: {v}");
    }
    eprintln!(
        "blocking SEDs [all, undrawn, undrawn mesh]: data census {:?}, content-layer test \
         {:?}, and creation types 7/8 dropped {:?}; {surfaced} SEDs with a surface material \
         (0x47 extra sets included); {:.1} s",
        blocking[0],
        blocking[1],
        blocking[2],
        started.elapsed().as_secs_f64()
    );
    if exhaustive() {
        let mesh_blockers = rows
            .get(&("mesh", false, false, false))
            .copied()
            .unwrap_or(0);
        assert_eq!(mesh_blockers, 82_139, "§9.3 undrawn mesh blockers");
        // CO12: the data census applies neither run-time filter; the walker's world applies
        // the content-layer test (L = 1, I = 0) and drops creation types 7 and 8.
        assert_eq!(blocking[0], [1_651_097, 97_688, 82_139], "CO12 data census");
        assert_eq!(
            blocking[1],
            [1_607_212, 88_749, 73_509],
            "CO12 content layers"
        );
        assert_eq!(
            blocking[2],
            [1_606_311, 88_748, 73_508],
            "CO12 creation types"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Collision sets of real landblocks.

#[test]
fn collision_sets_of_real_landblocks() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let pids = PhysicsPids::from_dictionary(dict);
    let geometries = cell_geometries(set);
    let cache = ObjectCollisionCache::default();
    let infos = landblock_entries(set, 2);
    let every = if exhaustive() { 211 } else { 2003 };
    let picked = sample(&infos, every);
    let started = Instant::now();
    #[derive(Default, Debug)]
    struct Tally {
        sets: usize,
        triangles: usize,
        bodies: usize,
        statics: usize,
        generated: usize,
        cells: usize,
        sweeps: usize,
        ground_hits: usize,
        max_triangles: usize,
        max_build_ms: f64,
        overlaps_after_sweep: usize,
        /// Most items one sweep's query box holds (against `MAX_QUERY_ITEMS`).
        max_sweep_items: usize,
    }
    let results = par_each(&picked, |(member, e)| {
        let mut t = Tally::default();
        let bytes = read(set, *member, e);
        let info = LandblockInfo::parse_expecting(&bytes, e.did, Some(e.stamp), dict)
            .unwrap_or_else(|err| panic!("{:#010x}: landblock info: {err}", e.did));
        let data_did = e.did - 0x0020_0000;
        let Ok((l, data_bytes)) = set.read_with_entry(data_did) else {
            return t;
        };
        let data = LandblockData::parse(&data_bytes, Some(l.entry.stamp)).expect("data");
        let build = Instant::now();
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.add_grid(Arc::new(GridMesh::terrain(&data)))
            .expect("terrain");
        if let Some(c) = GridMesh::ceiling(&data) {
            b.add_grid(Arc::new(c)).expect("ceiling");
        }
        let resolve = |o: Did| {
            cache.get_or_resolve(o, |o| {
                resolve_object_collision(o, |d| fetch(set, d), dict, &pids)
            })
        };
        b.add_statics(&info, &pids, &StaticOptions::default(), resolve);
        // Generated objects: re-place a few of the landblock's own objects elsewhere, as
        // scenery generation would hand them over (§9.4 API).
        let generated: Vec<GeneratedObject> = info
            .entities
            .iter()
            .filter_map(|s| Some((s.object?, s.placement?)))
            .take(8)
            .enumerate()
            .map(|(k, (object, p))| GeneratedObject {
                object,
                transform: Transform::new(
                    [(k as f32).mul_add(17.0, 5.0), 80.0, p.position[2]],
                    p.rotation,
                    1.0,
                ),
            })
            .collect();
        b.add_generated(&generated, resolve);
        b.add_cells(&info, &pids, |did| geometries.get(&did).cloned());
        let s = b.build();
        let ms = build.elapsed().as_secs_f64() * 1000.0;
        t.sets = 1;
        t.triangles = s.triangle_count();
        t.max_triangles = s.triangle_count();
        t.bodies = s.bodies().len();
        t.statics = s.report().statics_blocking as usize;
        t.generated = s.report().generated_blocking as usize;
        t.cells = s.cells().len();
        t.max_build_ms = ms;
        // Drop a player capsule from 60 m above the terrain at a grid of spots.
        let terrain = s.terrain().expect("terrain").clone();
        for k in 0..64 {
            let (x, y) = (2.5 + 20.0 * (k % 8) as f32, 2.5 + 20.0 * (k / 8) as f32);
            let ground = terrain
                .key_under(x, y, &data.vertex_flags)
                .and_then(|key| terrain.plane(key))
                .and_then(|p| p.height_at(x, y))
                .expect("ground");
            let top = ground + 60.0;
            let cap = Capsule::vertical([x, y, top + 0.35], 1.1, 0.35);
            t.sweeps += 1;
            let reach = cap
                .aabb()
                .zip(cap.translated([0.0, 0.0, -80.0]).aabb())
                .map(|(a, b)| a.union(b).expanded(2.0 * tessera_physics::geometry::SKIN))
                .expect("box");
            let mut items = 0usize;
            s.query_box(&reach, &QueryFilter::all(), |_, _, _| items += 1);
            t.max_sweep_items = t.max_sweep_items.max(items);
            let Some(hit) = s.sweep_capsule(&cap, [0.0, 0.0, -80.0], &QueryFilter::player()) else {
                continue;
            };
            assert!(hit.fraction.is_finite() && hit.normal.iter().all(|c| c.is_finite()));
            let mut start = Vec::new();
            s.contacts(&cap, 0.0, &QueryFilter::player(), &mut start);
            // No body may end deeper than it started (more than half a skin of rounding).
            let mut deepest: HashMap<_, f32> = HashMap::new();
            for c in &start {
                let d = deepest.entry(c.body).or_insert(0.0);
                *d = d.min(c.separation);
            }
            let moved = cap.translated([0.0, 0.0, -80.0 * hit.fraction]);
            let mut contacts = Vec::new();
            s.contacts(&moved, 0.0, &QueryFilter::player(), &mut contacts);
            let skin = tessera_physics::geometry::SKIN;
            if contacts
                .iter()
                .any(|c| c.separation < deepest.get(&c.body).copied().unwrap_or(0.0) - 0.5 * skin)
            {
                t.overlaps_after_sweep += 1;
            }
            if s.body(hit.body)
                .is_some_and(|b| b.source == tessera_physics::geometry::BodySource::Terrain)
            {
                t.ground_hits += 1;
            }
        }
        t
    });
    let mut total = Tally::default();
    for r in results {
        total.sets += r.sets;
        total.triangles += r.triangles;
        total.bodies += r.bodies;
        total.statics += r.statics;
        total.generated += r.generated;
        total.cells += r.cells;
        total.sweeps += r.sweeps;
        total.ground_hits += r.ground_hits;
        total.overlaps_after_sweep += r.overlaps_after_sweep;
        total.max_triangles = total.max_triangles.max(r.max_triangles);
        total.max_build_ms = total.max_build_ms.max(r.max_build_ms);
        total.max_sweep_items = total.max_sweep_items.max(r.max_sweep_items);
    }
    eprintln!(
        "collision sets: {total:?}; {} objects cached; {:.1} s",
        cache.len(),
        started.elapsed().as_secs_f64()
    );
    assert!(total.sets > 0);
    assert_eq!(
        total.overlaps_after_sweep, 0,
        "a sweep ended inside an obstacle"
    );
}

// ---------------------------------------------------------------------------------------
// CO4 for the per-level trees of render meshes (§7, §3.5), and level collision meshes as
// detail shapes.

#[derive(Default, Debug, Clone, Copy)]
struct LevelTally {
    records: usize,
    blocks: usize,
    coded: usize,
    plain: usize,
    renumbered: usize,
    invalid: usize,
    level_collision_triangles: usize,
    level_collision_dropped: usize,
}

impl LevelTally {
    fn add(&mut self, o: &Self) {
        self.records += o.records;
        self.blocks += o.blocks;
        self.coded += o.coded;
        self.plain += o.plain;
        self.renumbered += o.renumbered;
        self.invalid += o.invalid;
        self.level_collision_triangles += o.level_collision_triangles;
        self.level_collision_dropped += o.level_collision_dropped;
    }
}

/// Walks every level tree of `mesh` and checks its keys (§3.5): draw position · 2²⁰ +
/// triangle, or the renumbered form where one single-triangle draw takes no part.
fn check_levels(did: u32, mesh: &tessera_mesh::Mesh) -> LevelTally {
    let mut t = LevelTally {
        records: 1,
        ..LevelTally::default()
    };
    for (n, level) in mesh.levels.iter().enumerate() {
        if n == 0
            && let Some(c) = &level.collision
        {
            let positions = c.positions(mesh, level);
            let tris: Vec<[u16; 3]> = c.triangles.iter().map(|x| x.vertices).collect();
            let shape = ShapeTemplate::from_level_collision(&positions, &tris).expect("shape");
            t.level_collision_triangles += shape.triangles.len();
            t.level_collision_dropped += shape.dropped;
        }
        let Some(tree) = &level.tree else { continue };
        t.blocks += 1;
        let tessera_mesh::CollisionTree::Code(code) = tree else {
            continue;
        };
        t.coded += 1;
        let bytes = code.code.as_deref().expect("tree code kept");
        let walk = TreeCode::new(code.origin, code.scale, bytes)
            .walk()
            .unwrap_or_else(|e| panic!("{did:#010x} level {n}: CO4 walk: {e}"));
        let tri_count = |d: usize| {
            level
                .draws
                .get(d)
                .and_then(|dr| mesh.index_buffers.get(dr.index_buffer as usize))
                .map(|ib| ib.triangle_count())
        };
        let valid = |skip: Option<usize>| {
            walk.keys.iter().all(|&k| {
                let (s, tri) = ((k >> 20) as usize, (k & 0xF_FFFF) as usize);
                let d = match skip {
                    Some(sk) if s >= sk => s + 1,
                    _ => s,
                };
                tri_count(d).is_some_and(|c| tri < c)
            })
        };
        if valid(None) {
            t.plain += 1;
        } else if (0..level.draws.len().saturating_sub(1))
            .any(|d| tri_count(d) == Some(1) && valid(Some(d)))
        {
            t.renumbered += 1;
        } else {
            t.invalid += 1;
        }
    }
    t
}

#[test]
fn co4_mesh_level_trees() {
    let Some(set) = open_set() else { return };
    let started = Instant::now();
    let options = tessera_mesh::DecodeOptions {
        keep_collision: true,
        keep_tree_code: true,
        strict: false,
    };
    let mut entries = entries_of(set, 0x06, |_| true);
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let mut meshes = LevelTally::default();
    for r in par_each(&entries, |(member, e)| {
        let bytes = read(set, *member, e);
        let mesh =
            tessera_mesh::Mesh::decode_expecting(&bytes, Did(e.did), Some(e.stamp), &options)
                .unwrap_or_else(|err| panic!("{:#010x}: mesh: {err}", e.did));
        check_levels(e.did, &mesh)
    }) {
        meshes.add(&r);
    }
    // The embedded render meshes of 0x01 records (§5.7).
    let geometries = cell_geometries(set);
    let stamps: HashMap<u32, u16> =
        entries_of(set, CELL_GEOMETRY_PREFIX, |k| *k == ArchiveKind::General)
            .into_iter()
            .map(|(_, e)| (e.did, e.stamp))
            .collect();
    let cells: Vec<(&Did, &Arc<CellGeometry>)> = geometries.iter().collect();
    let mut embedded = LevelTally::default();
    for r in par_each(&cells, |(did, g)| {
        let rec = g.render_mesh_record();
        let mesh = tessera_mesh::Mesh::decode(&rec, stamps.get(&did.0).copied(), &options)
            .unwrap_or_else(|err| panic!("{did}: embedded mesh: {err}"));
        check_levels(did.0, &mesh)
    }) {
        embedded.add(&r);
    }
    eprintln!(
        "CO4 mesh levels: 0x06 {meshes:?}; 0x01 embedded {embedded:?}; {:.1} s",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(
        meshes.invalid + embedded.invalid,
        0,
        "CO4: keys outside the host"
    );
    if exhaustive() {
        assert_eq!(meshes.records, 47_563, "mesh records");
        assert_eq!(
            (meshes.blocks, meshes.coded),
            (71_167, 70_956),
            "0x06 tree blocks"
        );
        assert_eq!(meshes.renumbered, 12, "0x06 renumbered levels");
        assert_eq!(embedded.coded, 1_280, "0x01 embedded coded levels");
        assert_eq!(
            embedded.blocks - embedded.coded,
            83,
            "0x01 embedded levels without code"
        );
        assert_eq!(embedded.renumbered, 3, "0x01 embedded renumbered levels");
    }
}
