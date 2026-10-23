// SPDX-License-Identifier: GPL-3.0-or-later
//! Build time of one landblock's collision set against a user's own install (read-only):
//! the benchmark of the `walker.md` §14.11 performance note, landblock r2 (239, 242), whose
//! bundle job spent most of its time in the collision set (about 160 ms placing the static
//! bodies and about 570 ms building the set's hierarchy on a loaded machine).
//!
//! The set is built as `tessera-world` builds it (terrain and ceiling meshes, statics,
//! cells; generated scenery needs `tessera-world` and is left out). The test reports the
//! time of each phase and checks that the hierarchy built on several threads is the one
//! the single-threaded build gives (`collision.md` §12.1: query results bit-identical).
//! The build-time bound is asserted only with `TESSERA_PERF=1` (`docs/DEVELOPMENT.md`).
//!
//! Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-physics --test collision_build_time -- --nocapture`.
//! Only counts and timings are printed; no game data leaves the install.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::collision::CellGeometry;
use tessera_formats::{Did, LandblockData, LandblockInfo, PropertyDictionary, Record};
use tessera_physics::geometry::{
    Aabb, Bvh, Capsule, CollisionLimits, CollisionSetBuilder, FetchedRecord, GridMesh,
    MAX_BUILD_THREADS, ObjectCollisionCache, PhysicsPids, QueryFilter, StaticOptions,
    build_thread_budget, resolve_object_collision,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

/// Landblock data r2 (239, 242): `0x800RXXYY` (`landblock.md` §2.1).
const R2_239_242: u32 = 0x8002_EFF2;
/// Its landblock info (`0x802RXXYY`).
const R2_239_242_INFO: u32 = R2_239_242 + 0x0020_0000;

/// Bound on the hierarchy build of the set ([`Bvh::build`] over its item boxes, as
/// [`CollisionSetBuilder::build`] does it, with the shared thread budget free) with
/// `TESSERA_PERF=1` (release build, idle machine): well below the 570 ms of the
/// performance note.
const PERF_BVH_MS: f64 = 250.0;

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

fn perf() -> bool {
    std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1")
}

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

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn r2_239_242_collision_build_time() {
    let Some(set) = open_set() else { return };
    let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
    let dict =
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary");
    let pids = PhysicsPids::from_dictionary(&dict);
    let Ok((l, data_bytes)) = set.read_with_entry(R2_239_242) else {
        eprintln!("landblock data {R2_239_242:#010x} not in this install; skipping");
        return;
    };
    let data = LandblockData::parse(&data_bytes, Some(l.entry.stamp)).expect("landblock data");
    let info = match set.read_with_entry(R2_239_242_INFO) {
        Ok((l, bytes)) => Some(
            LandblockInfo::parse_expecting(&bytes, R2_239_242_INFO, Some(l.entry.stamp), &dict)
                .expect("landblock info"),
        ),
        Err(_) => None,
    };

    // Cell geometries the landblock places, decoded before the timed build (the world's
    // collision builder caches them across landblocks).
    let mut geometries: HashMap<Did, Arc<CellGeometry>> = HashMap::new();
    for c in info
        .iter()
        .filter_map(|i| i.cells.as_ref())
        .flat_map(|l| l.items.iter())
    {
        let did = c.geometry;
        if geometries.contains_key(&did) {
            continue;
        }
        if let Ok((l, bytes)) = set.read_with_entry(did.0)
            && let Ok(g) = CellGeometry::parse_expecting(&bytes, did.0, Some(l.entry.stamp), &dict)
        {
            geometries.insert(did, Arc::new(g));
        }
    }

    // Two builds: the first resolves every object (cold cache), the second reuses the
    // resolved templates as a streaming world does for objects it has met before.
    let cache = ObjectCollisionCache::default();
    let mut last = None;
    for pass in ["cold", "warm"] {
        let total = Instant::now();
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.add_grid(Arc::new(GridMesh::terrain(&data)))
            .expect("terrain");
        if let Some(c) = GridMesh::ceiling(&data) {
            b.add_grid(Arc::new(c)).expect("ceiling");
        }
        let grids_ms = ms(total);
        let t = Instant::now();
        let resolve = |o: Did| {
            cache.get_or_resolve(o, |o| {
                resolve_object_collision(o, |d| fetch(&set, d), &dict, &pids)
            })
        };
        if let Some(info) = &info {
            b.add_statics(info, &pids, &StaticOptions::default(), resolve);
        }
        let statics_ms = ms(t);
        let t = Instant::now();
        if let Some(info) = &info {
            b.add_cells(info, &pids, |did| geometries.get(&did).cloned());
        }
        let cells_ms = ms(t);
        let t = Instant::now();
        let s = b.build();
        let build_ms = ms(t);
        eprintln!(
            "r2 (239, 242) {pass}: {} triangles, {} round, {} solid, {} bodies, {} cells, \
             {} hierarchy nodes, {:.1} MiB; grids {grids_ms:.1} ms, statics {statics_ms:.1} ms, \
             cells {cells_ms:.1} ms, set build (hierarchy and cell lookup) {build_ms:.1} ms, \
             total {:.1} ms",
            s.triangle_count(),
            s.round_count(),
            s.solid_box_count(),
            s.bodies().len(),
            s.cells().len(),
            s.bvh().node_count(),
            s.heap_size() as f64 / (1024.0 * 1024.0),
            ms(total),
        );
        last = Some(s);
    }
    let Some(s) = last else { return };

    // The hierarchy over the same item boxes, built on one thread and on several, must be
    // the same tree (same nodes, same item order), so every query visits the same items in
    // the same order (`collision.md` §12.1).
    let boxes = item_boxes(&s);
    let t = Instant::now();
    let one = Bvh::build_with_threads(&boxes, 1);
    let one_ms = ms(t);
    // The shared budget, as the set builds it (no other build runs in this test).
    let t = Instant::now();
    let shared = Bvh::build(&boxes);
    let shared_ms = ms(t);
    // The count is clamped to MAX_BUILD_THREADS by the build; report what it used.
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(MAX_BUILD_THREADS);
    let t = Instant::now();
    let many = Bvh::build_with_threads(&boxes, threads);
    let many_ms = ms(t);
    eprintln!(
        "r2 (239, 242) hierarchy over {} items: 1 thread {one_ms:.1} ms, {threads} threads \
         {many_ms:.1} ms, shared budget ({} threads, at most {MAX_BUILD_THREADS} a build) \
         {shared_ms:.1} ms",
        boxes.len(),
        build_thread_budget(),
    );
    assert!(
        one == many,
        "the parallel hierarchy differs from the single-threaded one"
    );
    assert!(
        one == shared,
        "the shared-budget hierarchy differs from the single-threaded one"
    );
    // A streamed ring: up to 8 concurrent builds, each on its own thread (as on asset
    // workers). A helper starts only while fewer threads than the shared budget (cores less
    // RESERVED_CORES) are at work on builds, callers included; every tree is still the
    // single-threaded one. Only the timings are reported.
    let ring = build_thread_budget().min(8);
    let t = Instant::now();
    let times: Vec<(f64, bool)> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..ring)
            .map(|_| {
                sc.spawn(|| {
                    let t = Instant::now();
                    let b = Bvh::build(&boxes);
                    (ms(t), b == one)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("ring build"))
            .collect()
    });
    let ring_ms = ms(t);
    let slowest = times.iter().map(|t| t.0).fold(0.0f64, f64::max);
    eprintln!(
        "r2 (239, 242) {ring} concurrent shared-budget builds: wall {ring_ms:.1} ms, slowest \
         {slowest:.1} ms"
    );
    assert!(times.iter().all(|t| t.1), "a concurrent build differs");

    // The set built with an explicit count is the same set.
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_grid(Arc::new(GridMesh::terrain(&data)))
        .expect("terrain");
    if let Some(c) = GridMesh::ceiling(&data) {
        b.add_grid(Arc::new(c)).expect("ceiling");
    }
    if let Some(info) = &info {
        b.add_statics(info, &pids, &StaticOptions::default(), |o: Did| {
            cache.get_or_resolve(o, |o| {
                resolve_object_collision(o, |d| fetch(&set, d), &dict, &pids)
            })
        });
        b.add_cells(info, &pids, |did| geometries.get(&did).cloned());
    }
    assert!(
        *b.build_with_threads(1).bvh() == one,
        "the set built on one thread has another hierarchy"
    );
    assert!(
        one == *s.bvh(),
        "the set's hierarchy differs from the single-threaded one"
    );

    // Queries over a grid of spots agree item for item and in order.
    for k in 0..256 {
        let (x, y) = (2.5 + 10.0 * (k % 16) as f32, 2.5 + 10.0 * (k / 16) as f32);
        let q = Capsule::vertical([x, y, 0.0], 1.1, 0.35)
            .aabb()
            .expect("box")
            .union(Aabb {
                min: [x - 2.0, y - 2.0, -500.0],
                max: [x + 2.0, y + 2.0, 2000.0],
            });
        let (mut a, mut b) = (Vec::new(), Vec::new());
        one.query(&q, |i| a.push(i));
        many.query(&q, |i| b.push(i));
        assert_eq!(a, b, "query at ({x}, {y}) differs");
    }
    let mut n = 0usize;
    s.query_box(
        &Aabb {
            min: [0.0, 0.0, -500.0],
            max: [160.0, 160.0, 2000.0],
        },
        &QueryFilter::all(),
        |_, _, _| n += 1,
    );
    assert!(n > 0);

    if perf() {
        assert!(
            shared_ms <= PERF_BVH_MS,
            "hierarchy build of r2 (239, 242) took {shared_ms:.1} ms (bound {PERF_BVH_MS} ms)"
        );
    }
}

/// The item boxes the set's hierarchy indexes, in item order (triangles, then round
/// primitives, then solid boxes; [`CollisionSetBuilder::build`]).
fn item_boxes(s: &tessera_physics::geometry::CollisionSet) -> Vec<Aabb> {
    s.item_boxes()
}
