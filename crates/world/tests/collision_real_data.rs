// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision residency against a user's own install (read-only). Skipped unless
//! `TESSERA_GAME_DIR` points at an install.
//!
//! - `terrain_render_triangles_equal_collision_triangles`: ROADMAP M4.5 exit criterion 2
//!   (`collision.md` §6.1–§6.2, §6.4): for every landblock-data record of the install
//!   (390,150 in the owner's), the 2,048 triangles that `tessera-terrain` renders at level
//!   0 equal the collision triangles of `tessera-physics`, bit for bit in their vertex
//!   positions and with the same orientation; ceilings (drawn facing down) equal theirs as
//!   vertex sets.
//! - `cells_and_portals_of_landblock_info`: the collision residency's cells for every
//!   landblock-info record with cells: every cell is listed with its portals, neighbour
//!   links are reciprocal (`collision.md` §5.8), the cells the set places answer the
//!   point-in-cell lookup at the centre of their bounds (when inside their volume), and
//!   portal polygons of linked cells coincide (within 1 cm) in the landblock frame for the
//!   share §5.8 reports (all but 862 entries). No portal is left out by the per-set
//!   budgets.
//! - `streamer_finds_cells_owned_by_distant_landblocks`: settles the streamer around a
//!   landblock whose cells reach two landblocks beyond its own square (region 4, landblock
//!   (252, 152)) and checks that every placed cell of every resident landblock is found by
//!   `WorldStreamer::cell_at` at its volume's centre, wherever that centre lies
//!   (`collision.md` §5.8.1), and that the collision view there carries the owner's set.
//!
//! Debug builds sample every 31st landblock unless `TESSERA_FULL=1`; run
//! `cargo test --release -p tessera-world --test collision_real_data -- --nocapture` for the
//! full criteria. Only counts are printed; no game data leaves the install.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::{Did, LandblockData, LandblockInfo};
use tessera_physics::geometry::GridMesh;
use tessera_terrain::{
    EdgeLods, HeightSlopeShading, Lod, Neighbours, TerrainBlock, TerrainMesh, VertexNormals,
};
use tessera_world::{
    BundleOptions, CollisionInput, LandblockKey, LocalPos, RingConfig, StreamConfig, WorldContext,
    WorldPos, WorldSource, WorldStreamer,
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

fn workers() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

/// Every DID of the cell archives in `lo..=hi`, sorted, sampled in debug builds.
fn cell_dids(set: &ArchiveSet, lo: u32, hi: u32) -> Vec<u32> {
    let mut dids = Vec::new();
    for m in set.members() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(lo..=hi) {
            dids.push(e.expect("directory walk").did);
        }
    }
    dids.sort_unstable();
    dids.dedup();
    if full() {
        dids
    } else {
        dids.into_iter().step_by(31).collect()
    }
}

/// Runs `f` on every item over `workers()` threads; `f` returns counters to add up.
fn par_sum<const N: usize>(items: &[u32], f: impl Fn(u32) -> [usize; N] + Sync) -> [usize; N] {
    let next = AtomicUsize::new(0);
    let total: std::sync::Mutex<[usize; N]> = std::sync::Mutex::new([0; N]);
    std::thread::scope(|s| {
        for _ in 0..workers() {
            s.spawn(|| {
                let mut mine = [0usize; N];
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&did) = items.get(k) else { break };
                    for (a, b) in mine.iter_mut().zip(f(did)) {
                        *a += b;
                    }
                }
                let mut t = total.lock().unwrap();
                for (a, b) in t.iter_mut().zip(mine) {
                    *a += b;
                }
            });
        }
    });
    total.into_inner().unwrap()
}

type Tri = [[u32; 3]; 3];

fn bits(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

/// A triangle rotated so its smallest vertex comes first (orientation kept).
fn oriented(t: Tri) -> Tri {
    let k = (0..3).min_by_key(|&k| t[k]).unwrap_or(0);
    [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
}

/// A triangle as a sorted vertex set (orientation dropped).
fn unoriented(mut t: Tri) -> Tri {
    t.sort_unstable();
    t
}

/// The level-0 render triangles of `block` (positions from [`TerrainMesh`]).
fn render_triangles(block: &TerrainBlock, normals: &VertexNormals) -> Vec<Tri> {
    let mesh = TerrainMesh::build(
        block,
        normals,
        &HeightSlopeShading::default(),
        Lod::new(0).expect("level 0"),
        EdgeLods::NONE,
    );
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| t.map(|i| bits(mesh.positions[usize::from(i)])))
        .collect()
}

fn collision_triangles(mesh: &GridMesh) -> Vec<Tri> {
    mesh.triangles().map(|(_, t)| t.map(bits)).collect()
}

fn same(mut a: Vec<Tri>, mut b: Vec<Tri>, norm: fn(Tri) -> Tri) -> bool {
    a.iter_mut().for_each(|t| *t = norm(*t));
    b.iter_mut().for_each(|t| *t = norm(*t));
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

#[test]
fn terrain_render_triangles_equal_collision_triangles() {
    let Some(dir) = install() else { return };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let dids = cell_dids(&set, 0x8000_0000, 0x800F_FFFF);
    let started = Instant::now();
    let first_bad = std::sync::Mutex::new(None::<u32>);
    let [blocks, terrain_equal, ceilings, ceiling_equal, triangles] = par_sum(&dids, |did| {
        let Some((bytes, stamp)) = WorldSource::read_stamped(&set, Did(did)).expect("read") else {
            return [0; 5];
        };
        let data = LandblockData::parse_expecting(&bytes, did, stamp).expect("data decodes");
        let block = TerrainBlock::from_landblock(&data);
        let render = render_triangles(&block, &VertexNormals::compute(&block, &Neighbours::NONE));
        let collision = collision_triangles(&GridMesh::terrain(&data));
        let n = render.len();
        let t_ok = render.len() == 2_048 && same(render, collision, oriented);
        let (mut c, mut c_ok) = (0, 0);
        if let (Some(cb), Some(cm)) = (TerrainBlock::from_ceiling(&data), GridMesh::ceiling(&data))
        {
            c = 1;
            let render = render_triangles(&cb, &VertexNormals::ceiling(&cb, &Neighbours::NONE));
            c_ok = usize::from(same(render, collision_triangles(&cm), unoriented));
        }
        if !t_ok || c != c_ok {
            first_bad.lock().unwrap().get_or_insert(did);
        }
        [1, usize::from(t_ok), c, c_ok, n]
    });
    eprintln!(
        "{blocks} landblocks ({triangles} render triangles) in {:.1} s: terrain equal in \
         {terrain_equal}; {ceilings} ceilings, equal in {ceiling_equal}",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(
        terrain_equal,
        blocks,
        "terrain differs, first {:?}",
        first_bad.lock().unwrap().map(|d| format!("{d:#010x}"))
    );
    assert_eq!(ceiling_equal, ceilings, "ceilings differ");
    assert_eq!(triangles, 2_048 * blocks);
    if full() {
        assert_eq!(blocks, 390_150, "landblock-data records");
    }
}

#[test]
fn cells_and_portals_of_landblock_info() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(
        Arc::clone(&set),
        BundleOptions {
            generate_scenery: false,
            load_environment: false,
            ..BundleOptions::default()
        },
    )
    .expect("world context");
    let collision = Arc::clone(ctx.systems().collision.as_ref().expect("collision system"));
    let dids = cell_dids(&set, 0x8020_0000, 0x802F_FFFF);
    let started = Instant::now();
    let [
        infos,
        cells,
        placed,
        available,
        portals,
        reciprocal,
        corner_pairs,
        coincide,
        found,
        left_out,
    ] = par_sum(&dids, |did| {
        let Some((bytes, stamp)) = ctx.source().read_stamped(Did(did)).expect("read") else {
            return [0; 10];
        };
        let info =
            LandblockInfo::parse_expecting(&bytes, did, stamp, ctx.schema()).expect("info decodes");
        if info.cells().is_empty() {
            return [0; 10];
        }
        let key = LandblockKey::from_did(did).expect("key");
        let Some((bytes, stamp)) = ctx.source().read_stamped(key.data_did()).expect("read") else {
            return [0; 10];
        };
        let data = LandblockData::parse_expecting(&bytes, key.data_did().0, stamp).expect("data");
        let built = collision.build(&CollisionInput {
            key,
            data: &data,
            terrain: None,
            info: Some(&info),
            generated: &[],
            water: None,
            source: &**ctx.source(),
            schema: ctx.schema(),
        });
        let mut out = [0usize; 10];
        out[0] = 1;
        out[9] = built.portals_left_out as usize;
        out[1] = built.cells.len();
        out[2] = built.set.cells().len();
        for c in &built.cells {
            out[3] += usize::from(c.geometry_available);
            for p in &c.portals {
                out[4] += 1;
                let Some(n) = built.cell(p.neighbour) else {
                    continue;
                };
                let Some(back) = n.portals.get(usize::from(p.back_portal)) else {
                    continue;
                };
                if back.neighbour == c.index {
                    out[5] += 1;
                }
                if !p.corners.is_empty() && p.corners.len() == back.corners.len() {
                    out[6] += 1;
                    let close = p.corners.iter().all(|a| {
                        back.corners
                            .iter()
                            .any(|b| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>() <= 1e-4)
                    });
                    out[7] += usize::from(close);
                }
            }
        }
        // Every placed cell answers the lookup at its volume's centre (the mean of
        // its hull vertices, a point inside a convex cell) with itself or with a cell
        // that also contains the point (overlaps: lowest number wins, §5.8.1).
        for pc in built.set.cells().cells() {
            let Some(b) = pc.phantom_bounds else { continue };
            let mid = [
                0.5 * (b.min[0] + b.max[0]),
                0.5 * (b.min[1] + b.max[1]),
                0.5 * (b.min[2] + b.max[2]),
            ];
            if pc.contains(mid) {
                out[8] += usize::from(built.set.cell_at(mid).is_some());
            } else {
                out[8] += 1;
            }
        }
        out
    });
    eprintln!(
        "{infos} landblock-info records with cells in {:.1} s: {cells} cells ({placed} placed, \
         {available} with geometry), {portals} portals, {reciprocal} reciprocal; \
         {coincide} of {corner_pairs} portal pairs coincide within 1 cm; lookup found {found} \
         of {placed}",
        started.elapsed().as_secs_f64()
    );
    assert!(cells > 0);
    assert_eq!(reciprocal, portals, "neighbour links are reciprocal (§5.8)");
    assert_eq!(found, placed, "point-in-cell at the cells' centres");
    assert_eq!(left_out, 0, "no portal left out by the per-set budgets");
    if full() {
        assert_eq!(cells, 104_271, "cells of landblock info");
        assert_eq!(placed, 104_269, "cells placed (two carry no offset)");
        assert_eq!(portals, 393_842, "neighbour entries");
        // §5.8: 862 neighbour entries do not coincide within 1 cm; the 10 entries that
        // touch the two unplaced cells have no polygon here (392,980 − 10 coincide).
        assert_eq!(portals - corner_pairs, 10, "entries without a polygon");
        assert_eq!(
            corner_pairs - coincide,
            862,
            "portal pairs farther than 1 cm"
        );
    }
}

#[test]
fn streamer_finds_cells_owned_by_distant_landblocks() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = Arc::new(
        WorldContext::from_install(
            Arc::clone(&set),
            BundleOptions {
                generate_scenery: false,
                load_environment: false,
                ..BundleOptions::default()
            },
        )
        .expect("world context"),
    );
    // The landblock whose cell 3 lies two landblocks west of its square.
    let owner = LandblockKey::new(4, 252, 152).expect("key");
    if !ctx.has_landblock(owner).expect("directory") {
        eprintln!("{owner} not in this install; skipping");
        return;
    }
    let pool = WorkerPool::new(PoolConfig {
        workers: workers(),
        ..PoolConfig::default()
    });
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 320.0,
                mid: 480.0,
                far: 640.0,
                hysteresis: 80.0,
            },
            ..StreamConfig::default()
        },
    )
    .expect("streamer");
    let focus = LocalPos::new(owner, [80.0, 80.0, 0.0]).to_world();
    let started = Instant::now();
    assert!(w.settle(focus, Duration::from_secs(900)), "settles");
    let settle = started.elapsed();
    let (mut checked, mut outside_window, mut found) = (0usize, 0usize, 0usize);
    let mut missed: Vec<(LandblockKey, u16)> = Vec::new();
    for (key, _, b) in w.resident() {
        let Some(col) = &b.collision else { continue };
        let o = key.origin();
        for pc in col.set.cells().cells() {
            let Some(bb) = pc.phantom_bounds else {
                continue;
            };
            let mid: [f32; 3] = std::array::from_fn(|i| 0.5 * (bb.min[i] + bb.max[i]));
            // Only centres that the owner's own lookup answers (inside a convex volume).
            if col.set.cell_at(mid).is_none() {
                continue;
            }
            checked += 1;
            let beyond = |v: f32| !(-160.0..320.0).contains(&v);
            outside_window += usize::from(beyond(mid[0]) || beyond(mid[1]));
            let p = WorldPos::new(
                key.region(),
                o[0] + f64::from(mid[0]),
                o[1] + f64::from(mid[1]),
                f64::from(mid[2]),
            );
            if w.cell_at(&p).is_some() {
                found += 1;
            } else {
                missed.push((key, pc.index));
            }
        }
    }
    let owner_bundle = w.bundle(owner).expect("owner resident");
    let owner_col = owner_bundle.collision.as_ref().expect("collision");
    let cell3 = owner_col
        .set
        .cells()
        .get(3)
        .and_then(|c| c.phantom_bounds)
        .expect("cell 3 placed");
    let mid: [f32; 3] = std::array::from_fn(|i| 0.5 * (cell3.min[i] + cell3.max[i]));
    let o = owner.origin();
    let p = WorldPos::new(
        4,
        o[0] + f64::from(mid[0]),
        o[1] + f64::from(mid[1]),
        f64::from(mid[2]),
    );
    let p_block = p.landblock().expect("on the grid");
    let view = w.collision_view(&p, 0);
    eprintln!(
        "settled {} landblocks in {:.1} s; {checked} cell centres checked, {outside_window} \
         beyond the 3 x 3 neighbourhood of their owner, {found} found; owner cell 3 lies in \
         {p_block} (chessboard distance {:?}); view members {}",
        w.resident().count(),
        settle.as_secs_f64(),
        p_block.chebyshev(owner),
        view.members().len()
    );
    assert!(missed.is_empty(), "cells not found: {missed:?}");
    assert!(checked > 0 && outside_window > 0);
    assert!(p_block.chebyshev(owner).is_some_and(|d| d >= 2));
    assert!(w.cell_at(&p).is_some(), "owner cell 3 from {p_block}");
    assert!(
        view.members()
            .iter()
            .any(|(s, _)| s.landblock().is_some_and(|id| (id.x, id.y) == (252, 152))),
        "the view at the cell carries its owner's set"
    );
}
