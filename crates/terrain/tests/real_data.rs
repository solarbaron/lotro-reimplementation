// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain meshing against a user's own install (read-only).
//!
//! For every landblock-data record visited (all regions in a release build; one 16 × 16
//! window of region 1 in a debug build unless `TESSERA_FULL=1`):
//!
//! - [`TerrainBlock::height_at`] agrees with the decoder's `LandblockData::terrain_height`
//!   (`landblock.md` §3.11) at pseudo-random points;
//! - every surface layer converts with [`splat_layers`] and every stored layer triangle is a
//!   triangle of the level-0 mesh (the mesh uses the data's own triangulation, HM6);
//! - a mesh at a pseudo-random level, stitched to its neighbours' levels, is a valid
//!   counter-clockwise cover of the block;
//! - for every east and north neighbour pair with shared edge samples (HM7), the two meshes
//!   (at pseudo-random levels each) have bit-identical edge vertices, normals and segments.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-terrain --test real_data -- --nocapture`. Only counts,
//! timings and DIDs are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::LandblockData;
use tessera_formats::landblock::{GRID_SIDE, LANDBLOCK_SIZE, MAX_LANDBLOCK_COORD};
use tessera_terrain::{
    EdgeLods, HeightSlopeShading, Lod, Neighbours, Side, TerrainBlock, TerrainMesh, VertexNormals,
    splat_layers, triangulate,
};

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

/// Runs `f` over `items` on every core, keeping the input order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
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
                    for (k, item) in items[i..(i + 16).min(items.len())].iter().enumerate() {
                        local.push((i + k, f(item)));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, r)| r).collect()
}

/// Deterministic pseudo-random level for block (x, y).
fn lod_for(region: u8, x: u8, y: u8) -> Lod {
    let mut h = u32::from(region) << 16 | u32::from(x) << 8 | u32::from(y);
    h = h.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    Lod::clamped((h % 6) as u8)
}

fn data_did(region: u8, x: u8, y: u8) -> u32 {
    0x8000_0000 | u32::from(region) << 16 | u32::from(x) << 8 | u32::from(y)
}

#[derive(Default)]
struct Stats {
    blocks: usize,
    flat_blocks: usize,
    height_checks: usize,
    max_height_diff: f32,
    layers: usize,
    layer_triangles: usize,
    covers: usize,
    pairs_matching: usize,
    pairs_mismatched_flat: usize,
    pairs_mismatched_relief: usize,
    seams_checked: usize,
    lod_checks: usize,
    decode_failures: usize,
    t_block: Duration,
    t_lod: Duration,
    t_normals: Duration,
    t_mesh0: Duration,
    meshes0: usize,
}

/// Rotation-normalised triangle (same winding).
fn canon(t: [u16; 3]) -> [u16; 3] {
    let k = (0..3).min_by_key(|&k| t[k]).unwrap_or(0);
    [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
}

/// Edge of a mesh as (global position bits, normal bits) and segments.
type EdgeKey = ([u32; 3], [u32; 3]);
fn edge_of(
    m: &TerrainMesh,
    side: Side,
    off: [f32; 2],
) -> (BTreeSet<EdgeKey>, BTreeSet<(EdgeKey, EdgeKey)>) {
    let on = |p: [f32; 3]| match side {
        Side::East => p[0] == LANDBLOCK_SIZE,
        Side::West => p[0] == 0.0,
        Side::North => p[1] == LANDBLOCK_SIZE,
        Side::South => p[1] == 0.0,
    };
    let key = |k: usize| -> EdgeKey {
        let p = m.positions[k];
        (
            [
                (p[0] + off[0]).to_bits(),
                (p[1] + off[1]).to_bits(),
                p[2].to_bits(),
            ],
            m.normals[k].map(f32::to_bits),
        )
    };
    let mut verts = BTreeSet::new();
    let mut segs = BTreeSet::new();
    for t in m.indices.as_chunks::<3>().0 {
        for k in 0..3 {
            let (a, b) = (usize::from(t[k]), usize::from(t[(k + 1) % 3]));
            if on(m.positions[a]) && on(m.positions[b]) {
                let (ka, kb) = (key(a), key(b));
                verts.insert(ka);
                verts.insert(kb);
                segs.insert(if ka < kb { (ka, kb) } else { (kb, ka) });
            }
        }
    }
    (verts, segs)
}

/// Twice the xy area of a mesh triangle, in grid units, from block-local positions.
fn area2(m: &TerrainMesh, t: &[u16]) -> i64 {
    let p = |k: u16| {
        let q = m.positions[usize::from(k)];
        ((q[0] / 5.0) as i64, (q[1] / 5.0) as i64)
    };
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

struct Loaded {
    block: TerrainBlock,
    data: Option<LandblockData>,
    t_block: Duration,
    t_lod: Duration,
}

fn load(set: &ArchiveSet, region: u8, x: u8, y: u8, keep_data: bool) -> Option<Loaded> {
    let (loc, bytes) = set.read_with_entry(data_did(region, x, y)).ok()?;
    let data = LandblockData::parse(&bytes, Some(loc.entry.stamp)).ok()?;
    let t = Instant::now();
    let block = TerrainBlock::from_landblock(&data);
    let t_block = t.elapsed();
    let t = Instant::now();
    let _ = block.lod_errors();
    let t_lod = t.elapsed();
    Some(Loaded {
        block,
        data: keep_data.then_some(data),
        t_block,
        t_lod,
    })
}

/// One row of blocks, their records, and (block setup, LOD error) time.
type LoadedRow = (
    Vec<Option<TerrainBlock>>,
    Vec<Option<LandblockData>>,
    (Duration, Duration),
);

/// Visits rows `ys` and columns `xs` of `region`, keeping a three-row window.
fn run_region(set: &ArchiveSet, region: u8, xs: &[u8], ys: &[u8], stats: &mut Stats) {
    let shading = HeightSlopeShading::default();
    // Row cache: y → blocks (indexed like xs).
    let mut rows: BTreeMap<u8, Vec<Option<TerrainBlock>>> = BTreeMap::new();
    let col = |x: u8| xs.iter().position(|&c| c == x);
    let load_row = |y: u8| -> LoadedRow {
        let loaded = par_map(xs, |&x| load(set, region, x, y, true));
        let mut blocks = Vec::new();
        let mut datas = Vec::new();
        let mut t = (Duration::ZERO, Duration::ZERO);
        for l in loaded {
            match l {
                Some(l) => {
                    t.0 += l.t_block;
                    t.1 += l.t_lod;
                    blocks.push(Some(l.block));
                    datas.push(l.data);
                }
                None => {
                    blocks.push(None);
                    datas.push(None);
                }
            }
        }
        (blocks, datas, t)
    };
    let mut pending_data: BTreeMap<u8, Vec<Option<LandblockData>>> = BTreeMap::new();
    for (yi, &y) in ys.iter().enumerate() {
        // Make rows y and y + 1 resident (y + 1 for north neighbours and normals).
        for &ry in ys.iter().skip(yi).take(2) {
            if let std::collections::btree_map::Entry::Vacant(e) = rows.entry(ry) {
                let (blocks, datas, t) = load_row(ry);
                stats.t_block += t.0;
                stats.t_lod += t.1;
                stats.decode_failures += blocks.iter().filter(|b| b.is_none()).count();
                e.insert(blocks);
                pending_data.insert(ry, datas);
            }
        }
        rows.retain(|&ry, _| ry + 1 >= y);
        let datas = pending_data.remove(&y).unwrap_or_default();
        let get =
            |bx: u8, by: u8| -> Option<&TerrainBlock> { rows.get(&by)?.get(col(bx)?)?.as_ref() };
        let neighbours = |bx: u8, by: u8| {
            Neighbours(Side::ALL.map(|s| {
                let (dx, dy) = s.offset();
                let nx = u8::try_from(i32::from(bx) + dx).ok()?;
                let ny = u8::try_from(i32::from(by) + dy).ok()?;
                (nx <= MAX_LANDBLOCK_COORD && ny <= MAX_LANDBLOCK_COORD)
                    .then(|| get(nx, ny))
                    .flatten()
            }))
        };
        let edges = |bx: u8, by: u8| {
            EdgeLods(Side::ALL.map(|s| {
                let (dx, dy) = s.offset();
                let nx = u8::try_from(i32::from(bx) + dx).ok()?;
                let ny = u8::try_from(i32::from(by) + dy).ok()?;
                get(nx, ny).map(|_| lod_for(region, nx, ny))
            }))
        };
        struct RowOut {
            height_checks: usize,
            max_diff: f32,
            layers: usize,
            layer_triangles: usize,
            covers: usize,
            flat: bool,
            matching: usize,
            mismatched_flat: usize,
            mismatched_relief: usize,
            seams: usize,
            lod_checks: usize,
            t_normals: Duration,
            t_mesh0: Duration,
        }
        let items: Vec<(usize, u8)> = xs.iter().copied().enumerate().collect();
        let outs = par_map(&items, |&(xi, x)| -> Option<RowOut> {
            let block = get(x, y)?;
            let data = datas.get(xi)?.as_ref()?;
            let mut o = RowOut {
                height_checks: 0,
                max_diff: 0.0,
                layers: 0,
                layer_triangles: 0,
                covers: 0,
                flat: block.is_flat(),
                matching: 0,
                mismatched_flat: 0,
                mismatched_relief: 0,
                seams: 0,
                lod_checks: 0,
                t_normals: Duration::ZERO,
                t_mesh0: Duration::ZERO,
            };
            // 1. Height queries.
            let mut k = u32::from(x) << 8 | u32::from(y);
            for _ in 0..16 {
                k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let px = (k >> 8) as f32 / 16_777_216.0 * LANDBLOCK_SIZE;
                k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let py = (k >> 8) as f32 / 16_777_216.0 * LANDBLOCK_SIZE;
                let ours = block.height_at(px, py).expect("finite point");
                let theirs = data.terrain_height(px, py).expect("finite point");
                let d = (ours - theirs).abs();
                assert!(
                    d < 2e-3,
                    "{:#010x} ({px}, {py}): {ours} vs {theirs}",
                    data_did(region, x, y)
                );
                o.max_diff = o.max_diff.max(d);
                o.height_checks += 1;
            }
            // 1b. Level error bounds are exact: on the 2.5-unit raster (every sample and
            // every fine-quad centre, the vertices of the surfaces' common refinement) the
            // running maximum of |level surface − level-0 surface| equals lod_error.
            if !exhaustive() || (usize::from(x) * 7 + usize::from(y) * 3) % 8 == 0 {
                let mut running = 0.0f32;
                for lod in Lod::all() {
                    let mut worst = 0.0f32;
                    for yi in 0..=64u16 {
                        for xi in 0..=64u16 {
                            let (px, py) = (f32::from(xi) * 2.5, f32::from(yi) * 2.5);
                            let d = (block.lod_height_at(lod, px, py).expect("finite")
                                - block.height_at(px, py).expect("finite"))
                            .abs();
                            worst = worst.max(d);
                        }
                    }
                    running = running.max(worst);
                    let e = block.lod_error(lod);
                    assert!(
                        (running - e).abs() <= 1e-3,
                        "{:#010x} {lod}: raster {running} vs lod_error {e}",
                        data_did(region, x, y)
                    );
                }
                o.lod_checks += 1;
            }
            // 2. Splat layers are triangles of the level-0 mesh.
            let layers = splat_layers(&data.terrain)
                .unwrap_or_else(|e| panic!("{:#010x}: {e}", data_did(region, x, y)));
            if !layers.is_empty() {
                let tris: HashSet<[u16; 3]> = triangulate(block, Lod::FINEST, EdgeLods::NONE)
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| canon([t[0], t[1], t[2]]))
                    .collect();
                for l in &layers {
                    for t in l.indices.as_chunks::<3>().0 {
                        assert!(
                            tris.contains(&canon([t[0], t[1], t[2]])),
                            "{:#010x}: layer {} triangle {t:?} is not a mesh triangle",
                            data_did(region, x, y),
                            l.ordinal
                        );
                        o.layer_triangles += 1;
                    }
                }
                o.layers += layers.len();
            }
            // 3. Normals and a stitched mesh at this block's level.
            let nb = neighbours(x, y);
            let t = Instant::now();
            let normals = VertexNormals::compute(block, &nb);
            o.t_normals = t.elapsed();
            let t = Instant::now();
            let m0 = TerrainMesh::build(block, &normals, &shading, Lod::FINEST, EdgeLods::NONE);
            o.t_mesh0 = t.elapsed();
            assert_eq!(m0.positions.len(), GRID_SIDE * GRID_SIDE);
            let lod = lod_for(region, x, y);
            let e = edges(x, y);
            let m = TerrainMesh::build(block, &normals, &shading, lod, e);
            let total: i64 = m
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| {
                    let a = area2(&m, t);
                    assert!(
                        a > 0,
                        "{:#010x}: clockwise triangle",
                        data_did(region, x, y)
                    );
                    a
                })
                .sum();
            assert_eq!(total, 2 * 32 * 32);
            assert!(
                m.normals
                    .iter()
                    .all(|n| n.iter().all(|c| c.is_finite()) && n[2] > 0.0)
            );
            o.covers += 1;
            // 4. Seams with the east and north neighbours.
            for side in [Side::East, Side::North] {
                let (dx, dy) = side.offset();
                let (Ok(nx), Ok(ny)) = (
                    u8::try_from(i32::from(x) + dx),
                    u8::try_from(i32::from(y) + dy),
                ) else {
                    continue;
                };
                let Some(other) = get(nx, ny) else { continue };
                if !block.edge_matches(side, other) {
                    if block.is_flat() || other.is_flat() {
                        o.mismatched_flat += 1;
                    } else {
                        o.mismatched_relief += 1;
                    }
                    continue;
                }
                o.matching += 1;
                // Only fully surrounded blocks have final normals on both sides.
                let onb = neighbours(nx, ny);
                if !(nb.0.iter().all(Option::is_some) && onb.0.iter().all(Option::is_some)) {
                    continue;
                }
                let olod = lod_for(region, nx, ny);
                let mo = TerrainMesh::build(
                    other,
                    &VertexNormals::compute(other, &onb),
                    &shading,
                    olod,
                    edges(nx, ny),
                );
                let off = match side {
                    Side::East => [LANDBLOCK_SIZE, 0.0],
                    _ => [0.0, LANDBLOCK_SIZE],
                };
                let mine = edge_of(&m, side, [0.0, 0.0]);
                let theirs = edge_of(&mo, side.opposite(), off);
                assert_eq!(
                    mine,
                    theirs,
                    "{:#010x} {side:?} seam ({lod} | {olod})",
                    data_did(region, x, y)
                );
                o.seams += 1;
            }
            Some(o)
        });
        for o in outs.into_iter().flatten() {
            stats.blocks += 1;
            stats.flat_blocks += usize::from(o.flat);
            stats.height_checks += o.height_checks;
            stats.max_height_diff = stats.max_height_diff.max(o.max_diff);
            stats.layers += o.layers;
            stats.layer_triangles += o.layer_triangles;
            stats.covers += o.covers;
            stats.pairs_matching += o.matching;
            stats.pairs_mismatched_flat += o.mismatched_flat;
            stats.pairs_mismatched_relief += o.mismatched_relief;
            stats.seams_checked += o.seams;
            stats.lod_checks += o.lod_checks;
            stats.t_normals += o.t_normals;
            stats.t_mesh0 += o.t_mesh0;
            stats.meshes0 += 1;
        }
    }
}

/// A 16 × 16 window of `region` around its first block with relief found on a coarse scan.
fn debug_window(set: &ArchiveSet, region: u8) -> (Vec<u8>, Vec<u8>) {
    for y in (8..240u8).step_by(8) {
        for x in (8..240u8).step_by(8) {
            if let Ok(bytes) = set.read(data_did(region, x, y))
                && let Ok((_, _, h)) = LandblockData::parse_heights(&bytes)
                && !h.is_constant()
            {
                let x0 = x.saturating_sub(8);
                let y0 = y.saturating_sub(8);
                return ((x0..x0 + 16).collect(), (y0..y0 + 16).collect());
            }
        }
    }
    ((120..136).collect(), (120..136).collect())
}

#[test]
fn terrain_meshes_real_landblocks() {
    let Some(set) = open_set() else { return };
    let regions: Vec<u8> = {
        let mut r: Vec<u8> = set
            .members()
            .iter()
            .filter_map(|m| match m.id.kind {
                ArchiveKind::Cell(r) => u8::try_from(r).ok(),
                _ => None,
            })
            .collect();
        r.sort_unstable();
        r.dedup();
        r
    };
    let started = Instant::now();
    let mut total = Stats::default();
    for &region in &regions {
        if !exhaustive() && region != 1 {
            continue;
        }
        // Region archives without landblock data (e.g. 15) have no K=0 record at (0, 0).
        if set.read(data_did(region, 0, 0)).is_err() {
            eprintln!("region {region}: no landblock data");
            continue;
        }
        let (xs, ys) = if exhaustive() {
            let all: Vec<u8> = (0..=MAX_LANDBLOCK_COORD).collect();
            (all.clone(), all)
        } else {
            debug_window(set, region)
        };
        let mut s = Stats::default();
        run_region(set, region, &xs, &ys, &mut s);
        eprintln!(
            "region {region}: {} blocks ({} flat), {} decode failures, {} height checks \
             (max diff {:.2e}), {} layers / {} layer triangles on mesh triangles, {} stitched \
             covers, neighbour pairs: {} matching ({} seams compared), {} mismatched (flat), {} \
             mismatched (relief), {} exact LOD-error checks",
            s.blocks,
            s.flat_blocks,
            s.decode_failures,
            s.height_checks,
            s.max_height_diff,
            s.layers,
            s.layer_triangles,
            s.covers,
            s.pairs_matching,
            s.seams_checked,
            s.pairs_mismatched_flat,
            s.pairs_mismatched_relief,
            s.lod_checks
        );
        assert_eq!(s.decode_failures, 0, "region {region}");
        assert_eq!(s.pairs_mismatched_relief, 0, "region {region}: HM7");
        total.blocks += s.blocks;
        total.seams_checked += s.seams_checked;
        total.lod_checks += s.lod_checks;
        total.layer_triangles += s.layer_triangles;
        total.t_block += s.t_block;
        total.t_lod += s.t_lod;
        total.t_normals += s.t_normals;
        total.t_mesh0 += s.t_mesh0;
        total.meshes0 += s.meshes0;
    }
    if total.blocks == 0 {
        eprintln!("no landblock data found");
        return;
    }
    let per = |d: Duration| d.as_secs_f64() * 1e6 / total.meshes0.max(1) as f64;
    eprintln!(
        "total: {} blocks, {} seams, {} layer triangles in {:.1} s; per block (one thread): \
         TerrainBlock {:.1} µs, LOD errors {:.1} µs, normals {:.1} µs, LOD0 mesh {:.1} µs",
        total.blocks,
        total.seams_checked,
        total.layer_triangles,
        started.elapsed().as_secs_f64(),
        per(total.t_block),
        per(total.t_lod),
        per(total.t_normals),
        per(total.t_mesh0)
    );
    assert!(total.seams_checked > 0, "no seam was compared");
    assert!(total.lod_checks > 0, "no LOD error was checked");
}
