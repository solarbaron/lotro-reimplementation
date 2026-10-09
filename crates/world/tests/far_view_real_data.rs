// SPDX-License-Identifier: GPL-3.0-or-later
//! The far view of `tessera-world` against a user's own install (read-only; skipped unless
//! `TESSERA_GAME_DIR` is set).
//!
//! - `uniquedb_entries`: the four UNIQUEDB keys of `far-view.md` §6.1 and §7.3 name existing
//!   records of the expected kinds (FV-I13, existence and kind only).
//! - `flight_residency_and_mesh_invariants`: flies the viewer across a row of region 1 one
//!   landblock at a time (with one long jump, a discard) at `LandscapeDrawDistance` High
//!   (*F* = 4) with the original's near field (*N* = 1). Every step reads the listed
//!   sectors and imposter tables, builds every batch and places every sprite, then checks
//!   FV-I2 (sector list), FV-I3 and FV-I4 (grid), FV-I5 (far and near batches partition the
//!   covered landblocks), FV-I6 (grid and stitching triangle counts), mesh sanity (indices
//!   in range, positions inside the sector square, heights inside the record's bounds,
//!   texture coordinates in 0–1), closed seams (every vertex shared by two sectors' far
//!   batches has the same height in both), sprite placement rules (§9.3) and FV-I12 for the
//!   distant set.
//!
//! Tunables: `TESSERA_FARVIEW_REGION` (default 1), `TESSERA_FARVIEW_STEPS` (default 10 in a
//! debug build, 40 in release). Only counts and timings are printed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_world::far_view::{
    Cell, FarViewRecords, FarViewSettings, FarViewState, NearField, SectorKey, SpriteView, Viewer,
    grid::EDGES, mesh, read_imposter_table, read_sector,
};
use tessera_world::{BundleOptions, LandblockKey, WorldContext, WorldPos};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn env_num<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok()?.parse().ok()
}

fn context(dir: &PathBuf) -> (Arc<ArchiveSet>, WorldContext) {
    let set = Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    (set, ctx)
}

#[test]
fn uniquedb_entries() {
    let Some(dir) = install() else { return };
    let (_, ctx) = context(&dir);
    let r = FarViewRecords::load(&ctx).expect("UNIQUEDB entries");
    let exists = |d: Option<Did>| d.is_some_and(|d| ctx.source().contains(d).unwrap_or(false));
    for (what, d, prefix) in [
        ("plain template", r.plain_template, 0x2B),
        ("normal-mapped template", r.normal_template, 0x2B),
        ("detail texture", r.detail_texture, 0x40),
        ("horizon mesh", r.horizon_mesh, 0x06),
    ] {
        assert!(exists(d), "FV-I13: {what} missing");
        assert_eq!(d.map(Did::prefix), Some(prefix), "FV-I13: {what} kind");
    }
    eprintln!("UNIQUEDB far-view entries: 4 of 4 present");
}

/// The sector keys of `region` that have a record, from the directory (no reads).
fn present_sectors(set: &ArchiveSet, region: u8) -> Vec<SectorKey> {
    let mut out = Vec::new();
    for m in set.members() {
        if m.id.kind != ArchiveKind::Cell(u32::from(region)) {
            continue;
        }
        let lo = 0x8050_0000 | (u32::from(region) << 16);
        for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
            let e = e.expect("directory walk");
            let (x, y) = (((e.did >> 8) & 0xFF) as u8, (e.did & 0xFF) as u8);
            if let Some(k) = SectorKey::new(region, x / 3, y / 3)
                && k.corner() == (x, y)
            {
                out.push(k);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Default)]
struct Totals {
    steps: usize,
    sectors_read: usize,
    missing: usize,
    failed: usize,
    batches: usize,
    grid_triangles: usize,
    stitch_triangles: usize,
    shared_vertices: usize,
    sprites: usize,
    distant_live_max: usize,
    distant_entries_max: usize,
    /// Smallest distance / view distance of any entry seen.
    nearest_ratio: f64,
    max_step_ms: f64,
}

#[test]
#[allow(clippy::too_many_lines)]
fn flight_residency_and_mesh_invariants() {
    let Some(dir) = install() else { return };
    let (set, ctx) = context(&dir);
    let region: u8 = env_num("TESSERA_FARVIEW_REGION").unwrap_or(1);
    let steps: usize =
        env_num("TESSERA_FARVIEW_STEPS").unwrap_or(if cfg!(debug_assertions) { 10 } else { 40 });
    let present = present_sectors(&set, region);
    assert!(!present.is_empty(), "region {region} has no sectors");
    // Start in the middle of the row with the most sectors.
    let mut rows: HashMap<u8, Vec<u8>> = HashMap::new();
    for k in &present {
        rows.entry(k.sy()).or_default().push(k.sx());
    }
    let (sy, xs) = rows
        .iter()
        .max_by_key(|(sy, v)| (v.len(), std::cmp::Reverse(**sy)))
        .map(|(sy, v)| (*sy, v.clone()))
        .expect("a row");
    let start_x = xs[xs.len() / 4].saturating_mul(3);
    let y = sy * 3 + 1;
    eprintln!(
        "region {region}: {} sectors; flying row Y {y} from X {start_x}, {steps} steps",
        present.len()
    );
    let settings = FarViewSettings {
        near_batches: true,
        ..FarViewSettings::from_level(3)
    };
    let f = settings.far_radius;
    let mut state = FarViewState::new(settings, ctx.pids().landblock.entity_content_layers);
    let view = SpriteView::default();
    let mut totals = Totals::default();
    let mut tables: HashMap<Did, bool> = HashMap::new();
    let path: Vec<u8> = (0..steps)
        .map(|i| {
            start_x
                .saturating_add(u8::try_from(i).unwrap_or(u8::MAX))
                .min(252)
        })
        .collect();
    for (step, x) in path
        .iter()
        .copied()
        .chain([path.last().copied().unwrap_or(0).saturating_sub(40)])
        .enumerate()
    {
        let t = Instant::now();
        let landblock = LandblockKey::new(region, x, y).expect("landblock");
        let viewer = Viewer {
            landblock,
            instance: 0,
        };
        // Up to three passes: records, then batches and tables, then sprites.
        for _ in 0..4 {
            let w = state.update(Some(viewer), NearField::Radius(1), view);
            for k in &w.load_sectors {
                let r = read_sector(&**ctx.source(), *k, ctx.schema());
                match &r {
                    Ok(Some(_)) => totals.sectors_read += 1,
                    Ok(None) => totals.missing += 1,
                    Err(_) => totals.failed += 1,
                }
                state.sector_loaded(*k, r);
            }
            for d in &w.load_tables {
                let r = read_imposter_table(&**ctx.source(), *d);
                tables.insert(*d, matches!(r, Ok(Some(_))));
                state.table_loaded(*d, r);
            }
            for j in &w.build {
                let m = j.run();
                assert!(m.is_ok(), "batch build failed: {:?}", m.as_ref().err());
                state.built(j.key, j.generation, m);
            }
            for j in &w.sprites {
                state.sprites_placed(j.key, j.epoch, j.run());
            }
        }
        assert!(state.settled(), "step {step}: not settled");
        let ms = t.elapsed().as_secs_f64() * 1e3;
        totals.max_step_ms = totals.max_step_ms.max(ms);
        totals.steps += 1;

        // FV-I2.
        let listed = state.listed().to_vec();
        assert!(listed.len() <= ((2 * f + 1) * (2 * f + 1)) as usize);
        for k in &listed {
            let (cx, cy) = k.corner();
            assert!(u32::from(cx.abs_diff(x)) <= 3 * f + 2, "FV-I2");
            assert!(u32::from(cy.abs_diff(y)) <= 3 * f + 2, "FV-I2");
        }
        // FV-I3, FV-I4.
        let grid = state.grid().expect("grid");
        let mut covered = 0u32;
        for ((gx, gy), c) in grid.cells() {
            let cheb = gx.abs_diff(x).max(gy.abs_diff(y));
            match c {
                Cell::Near(l) => {
                    assert!(cheb <= 1 && l == 0, "FV-I3");
                    covered += 1;
                }
                Cell::Far(l) => {
                    assert!(cheb > 1 && l >= 1, "FV-I3");
                    covered += 1;
                }
                Cell::Uncovered => {}
            }
            for (dx, dy) in EDGES {
                if let (Some(a), Some(b)) = (
                    c.lod(),
                    grid.cell(i32::from(gx) + dx, i32::from(gy) + dy).lod(),
                ) {
                    assert!(a.abs_diff(b) <= 1, "FV-I4");
                }
            }
        }
        // FV-I5, FV-I6, mesh sanity, seams.
        let mut batch_blocks = 0u32;
        let mut heights: HashMap<(i64, i64), (f32, SectorKey)> = HashMap::new();
        for k in &listed {
            let Some(slot) = state.slot(*k) else { continue };
            let Some(rec) = slot.ready() else { continue };
            let m = slot.meshes.as_ref().expect("built");
            let d = slot.descriptors.expect("descriptors");
            let (cx, cy) = k.corner();
            for (batch, far) in [
                (&m.terrain, true),
                (m.near_terrain.as_ref().expect("near"), false),
            ] {
                batch_blocks += batch.landblocks;
                totals.batches += 1;
                let (mut want_grid, mut want_stitch) = (0usize, 0usize);
                for desc in d.iter().filter(|x| x.present && x.far == far) {
                    let q = 32usize >> desc.lod;
                    want_grid += q * q * 2;
                    for e in desc.edges.iter().filter(|e| e.stitch) {
                        want_stitch += mesh::stitch_triangle_count(desc.lod, e.neighbour_lod);
                    }
                }
                assert_eq!(batch.grid_triangles(), want_grid, "FV-I6 grid");
                assert_eq!(batch.stitch_triangles(), want_stitch, "FV-I6 stitching");
                totals.grid_triangles += want_grid;
                totals.stitch_triangles += want_stitch;
                let n = batch.positions.len();
                assert!(batch.indices.iter().all(|i| (*i as usize) < n));
                for (p, uv) in batch.positions.iter().zip(&batch.uvs) {
                    assert!((0.0..=480.0).contains(&p[0]) && (0.0..=480.0).contains(&p[1]));
                    assert!(
                        p[2] >= rec.terrain.bounds_min[2] - 1e-3
                            && p[2] <= rec.terrain.bounds_max[2] + 1e-3,
                        "height outside the record's bounds"
                    );
                    assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]));
                }
                if !far {
                    continue;
                }
                // Seams: the same global point from two sectors has the same height.
                for p in &batch.positions {
                    #[allow(clippy::cast_possible_truncation)]
                    let g = (
                        (f64::from(cx) * 160.0 + f64::from(p[0])).round() as i64 * 1000,
                        (f64::from(cy) * 160.0 + f64::from(p[1])).round() as i64 * 1000,
                    );
                    match heights.get(&g) {
                        Some((z, other)) if *other != *k => {
                            assert_eq!(*z, p[2], "seam height differs between sectors");
                            totals.shared_vertices += 1;
                        }
                        Some(_) => {}
                        None => {
                            heights.insert(g, (p[2], *k));
                        }
                    }
                }
            }
            // Sprites (§9.2, §9.3).
            if let Some((_, sprites)) = &slot.sprites {
                totals.sprites += sprites.len();
                for s in sprites.iter() {
                    assert!(s.size.is_finite() && s.size > 0.0 && s.distance >= 0.0);
                }
            }
        }
        assert_eq!(batch_blocks, covered, "FV-I5");
        // FV-I12.
        let at = WorldPos::new(
            region,
            f64::from(x) * 160.0 + 80.0,
            f64::from(y) * 160.0 + 80.0,
            f64::from(state.near_floor().unwrap_or(300.0)) + 2.0,
        );
        let _ = state.distant_mut().walk(&at, 0, 1.0, |_| false, |_| true);
        let live = state.distant().live().count();
        totals.distant_entries_max = totals.distant_entries_max.max(state.distant().len());
        for e in state.distant().entries() {
            let r = e.distance_sq(&at).sqrt() / f64::from(e.view_distance_sq).sqrt();
            if totals.nearest_ratio == 0.0 || r < totals.nearest_ratio {
                totals.nearest_ratio = r;
            }
        }
        totals.distant_live_max = totals.distant_live_max.max(live);
        for e in state.distant().live() {
            assert!(e.distance_sq(&at) < f64::from(e.view_distance_sq), "FV-I12");
            assert!(listed.contains(&e.sector), "FV-I12: sector not listed");
        }
    }
    // FV-I12, positive side: stand on a distant entry and walk; it must be created.
    let sample = state.distant().entries().next().cloned();
    if let Some(e) = sample
        && let Some(landblock) = e.position.landblock()
    {
        let viewer = Viewer {
            landblock,
            instance: 0,
        };
        for _ in 0..4 {
            let w = state.update(Some(viewer), NearField::Radius(1), view);
            for k in &w.load_sectors {
                let r = read_sector(&**ctx.source(), *k, ctx.schema());
                state.sector_loaded(*k, r);
            }
            for d in &w.load_tables {
                state.table_loaded(*d, read_imposter_table(&**ctx.source(), *d));
            }
            for j in &w.build {
                state.built(j.key, j.generation, j.run());
            }
            for j in &w.sprites {
                state.sprites_placed(j.key, j.epoch, j.run());
            }
        }
        let c = state
            .distant_mut()
            .walk(&e.position, 0, 1.0, |_| false, |_| true);
        assert!(
            state.distant().is_live(e.iid),
            "FV-I12: an entry at the viewer's position is not created"
        );
        totals.distant_live_max = totals.distant_live_max.max(c.create.len());
        for l in state.distant().live() {
            assert!(
                l.distance_sq(&e.position) < f64::from(l.view_distance_sq),
                "FV-I12"
            );
        }
    }
    eprintln!(
        "far view flight: {} steps, {} sector reads ({} missing, {} failed), {} batches, \
         {} grid and {} stitching triangles, {} shared seam vertices, {} sprites placed, \
         {} imposter tables ({} present), up to {} far entities live of {} entries (nearest at {:.2} of its view distance), slowest step {:.1} ms \
         (reads and builds on one thread)",
        totals.steps,
        totals.sectors_read,
        totals.missing,
        totals.failed,
        totals.batches,
        totals.grid_triangles,
        totals.stitch_triangles,
        totals.shared_vertices,
        totals.sprites,
        tables.len(),
        tables.values().filter(|p| **p).count(),
        totals.distant_live_max,
        totals.distant_entries_max,
        totals.nearest_ratio,
        totals.max_step_ms,
    );
    assert_eq!(totals.failed, 0, "sector records failed to decode");
    assert!(totals.sectors_read > 0);
    assert!(totals.shared_vertices > 0, "no seams checked");
}
