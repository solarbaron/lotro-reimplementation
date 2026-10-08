// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the M4.5 world views against the owner's install: the record
//! tables of every new record kind, landblock products (generated scenery, water,
//! collision), 3D scenes with collision overlays (0x66, 0x01, setups, landblocks), and the
//! sky timeline.
//!
//! Skipped unless `TESSERA_GAME_DIR` is set. The install is only read. Only counts and
//! DIDs are printed. Debug builds sample; `TESSERA_EXPLORER_FULL=1` (or a release build)
//! checks every record of the record families.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_assets::{AssetKey, Decoder, JobContext};
use tessera_explorer::collision_overlay::OverlayKind;
use tessera_explorer::data::DataContext;
use tessera_explorer::decoders::{read_raw, record_product_stamped};
use tessera_explorer::scene::{SceneRequest, SceneSource, build_scene};
use tessera_explorer::world::{
    LandblockDecoder, SkyDecoder, SkyProduct, TIMELINE_SAMPLES, day_timeline, landblock_of,
    sky_timeline,
};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1")
}

/// Every DID of type prefix `prefix` (first archive wins), every `step`-th unless full.
fn dids(ctx: &DataContext, prefix: u8, step: usize) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let mut out = Vec::new();
    for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            out.push(e.expect("directory walk").did);
        }
    }
    out.sort_unstable();
    out.dedup();
    let step = if full() { 1 } else { step.max(1) };
    out.into_iter().step_by(step).collect()
}

#[test]
fn record_tables_of_the_world_records() {
    let Some(dir) = game_dir() else { return };
    let ctx = DataContext::open(&dir, None).expect("open install");
    for (prefix, step, rows_expected) in [
        (0x19u8, 1usize, true),
        (0x12, 1, true),
        (0x11, 1, true),
        (0x02, 40, true),
        (0x16, 1, true),
        (0x17, 1, true),
        (0x15, 1, true),
        (0x1A, 20, true),
        (0x35, 1, true),
        (0x32, 1, true),
        (0x66, 400, true),
        (0x01, 25, true),
        (0x0F, 1, true),
    ] {
        let list = dids(&ctx, prefix, step);
        let mut rows = 0usize;
        let mut empty = 0usize;
        for &did in &list {
            let raw = read_raw(&ctx, did, None).expect("read");
            let p = record_product_stamped(&ctx, did, &raw.bytes, Some(raw.entry.stamp))
                .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
            assert!(!p.summary.is_empty(), "{did:#010x} has no summary");
            for r in &p.rows {
                assert_eq!(r.cells.len(), p.columns.len(), "{did:#010x}");
            }
            if let Some(c) = p.link_column {
                assert!(c < p.columns.len());
            }
            rows += p.rows.len();
            empty += usize::from(p.rows.is_empty());
        }
        eprintln!(
            "prefix {prefix:#04x}: {} records, {rows} rows, {empty} without rows",
            list.len()
        );
        assert!(!list.is_empty(), "no records of prefix {prefix:#04x}");
        if rows_expected {
            assert!(rows > 0, "prefix {prefix:#04x} gave no rows");
        }
    }
}

#[test]
fn landblocks_scenes_and_sky() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open install"));
    let job = JobContext::detached();

    // Landblock products: a sample of the landblock-data records.
    let data: Vec<u32> = dids(&ctx, 0x80, 1)
        .into_iter()
        .filter(|d| (d >> 20) & 0xF == 0)
        .collect();
    let step = if full() { 97 } else { 1_999 };
    let decoder = LandblockDecoder(Arc::clone(&ctx));
    let (mut blocks, mut generated, mut statics, mut wet, mut cells) = (0, 0, 0, 0, 0);
    let mut passes = 0usize;
    let mut with_generated = None;
    let mut with_cells = None;
    let started = std::time::Instant::now();
    for &did in data.iter().step_by(step) {
        let p = decoder
            .decode(AssetKey::new(did, 0), Vec::new(), &job)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(p.heights.len(), 33 * 33);
        assert!(p.heights.iter().all(|h| h.is_finite()));
        assert!(p.range.0 <= p.range.1);
        assert!(!p.scenery.is_empty() && !p.water_rows.is_empty() && !p.collision.is_empty());
        for o in p.generated.iter().chain(&p.statics) {
            assert!(o.position.iter().all(|v| v.is_finite()));
            // Blocking a walker needs a body; a body need not block (ethereal to Player).
            assert!(!o.blocks || o.collides);
        }
        passes += p
            .generated
            .iter()
            .chain(&p.statics)
            .filter(|o| o.collides && !o.blocks)
            .count();
        blocks += 1;
        generated += p.generated.len();
        statics += p.statics.len();
        wet += usize::from(p.water.is_some());
        cells += p.cells.len();
        if !p.generated.is_empty() && with_generated.is_none() {
            with_generated = Some(did);
        }
        if !p.cells.is_empty() && with_cells.is_none() {
            with_cells = Some(did);
        }
    }
    eprintln!(
        "landblocks: {blocks} of {} built in {} ms: {generated} generated objects, {statics} \
         statics ({passes} with collision a walker passes through), {wet} wet, {cells} cells; \
         with scenery {with_generated:#010x?}, with cells {with_cells:#010x?}",
        data.len(),
        started.elapsed().as_millis()
    );
    assert!(blocks > 0);
    assert!(generated > 0, "no generated scenery in the sample");

    // 3D scenes: landblocks with their collision overlays.
    for did in [with_generated, with_cells].into_iter().flatten() {
        let s = build_scene(&ctx, did, SceneRequest::default(), &job).expect("landblock scene");
        assert_eq!(s.source, SceneSource::Landblock);
        assert!(s.drawable());
        assert!(s.collision.iter().any(|o| o.kind == OverlayKind::Terrain));
        assert!(
            s.collision
                .iter()
                .all(|o| o.prepared.is_some() || o.triangles == 0)
        );
        eprintln!(
            "landblock scene {did:#010x}: {} overlays, {} overlay triangles, radius {:.1}",
            s.collision.len(),
            s.collision.iter().map(|o| o.triangles).sum::<usize>(),
            s.sphere.1
        );
        // Another record of the same landblock (its info record) shows the same scene.
        let info = did | 0x0020_0000;
        assert_eq!(landblock_of(did), landblock_of(info));
        let other = build_scene(&ctx, info, SceneRequest::default(), &job).expect("info scene");
        assert_eq!(other.source, SceneSource::Landblock);
        let summary =
            |s: &tessera_explorer::scene::SceneProduct| -> Vec<(OverlayKind, usize, usize)> {
                s.collision
                    .iter()
                    .map(|o| (o.kind, o.triangles, o.lines.len()))
                    .collect()
            };
        assert_eq!(summary(&other), summary(&s), "{did:#010x} vs {info:#010x}");
        assert_eq!(other.sphere, s.sphere);
        // The scene is framed on the terrain and objects, not on deep liquid boxes.
        assert!(s.sphere.1 < 2_000.0, "{did:#010x}: radius {}", s.sphere.1);
    }

    // Collision meshes, cells and setups.
    for (prefix, step) in [(0x66u8, 997usize), (0x01, 53), (0x04, 2_003)] {
        let (mut n, mut with) = (0usize, 0usize);
        for did in dids(&ctx, prefix, step) {
            match build_scene(&ctx, did, SceneRequest::default(), &job) {
                Ok(s) => {
                    n += 1;
                    with += usize::from(!s.collision.is_empty());
                    if prefix == 0x66 {
                        assert!(s.drawable(), "{did:#010x}");
                        assert!(s.collision.iter().any(|o| o.kind == OverlayKind::Mesh));
                    }
                }
                Err(e) if prefix == 0x04 => eprintln!("setup {did:#010x}: {e}"),
                Err(e) => panic!("{did:#010x}: {e}"),
            }
        }
        eprintln!("prefix {prefix:#04x}: {n} scenes, {with} with collision overlays");
        assert!(with > 0);
    }

    // Sky: every sky key under every day type on one day; day descriptions.
    let skies = dids(&ctx, 0x15, 1);
    let sky = SkyDecoder(Arc::clone(&ctx));
    for did in skies {
        let p = sky
            .decode(AssetKey::new(did, 0), Vec::new(), &job)
            .expect("sky");
        let SkyProduct::Sky {
            library,
            keys,
            day_types,
            days,
        } = p
        else {
            panic!("a sky description gave a day product");
        };
        let mut chosen = 0;
        let day = library.calendar().read(0.0).day_number;
        for &k in &keys {
            for &t in &day_types {
                let tl = sky_timeline(&library, k, t, day, TIMELINE_SAMPLES);
                assert_eq!(tl.values.len(), TIMELINE_SAMPLES);
                assert!(tl.values.iter().all(|v| v.sun_brightness.is_finite()
                    && v.fog_far.is_finite()
                    && v.sun_heading.is_finite()));
                chosen += usize::from(tl.day.is_some());
            }
        }
        eprintln!(
            "sky {did:#010x}: {} keys, {} day types, {days} day descriptions, {chosen} timelines \
             with a chosen day",
            keys.len(),
            day_types.len()
        );
        assert!(chosen > 0);
    }
    let mut keyframes = 0;
    for did in dids(&ctx, 0x1A, 37) {
        let SkyProduct::Day { day, timeline } = sky
            .decode(AssetKey::new(did, 0), Vec::new(), &job)
            .expect("day")
        else {
            panic!("a day description gave a sky product");
        };
        let tl = day_timeline(&day, TIMELINE_SAMPLES);
        assert!(tl.values.iter().all(|v| v.sun_brightness.is_finite()));
        // The worker's timeline is the pure function's.
        assert_eq!(*timeline, tl);
        keyframes += tl.keyframes.len();
    }
    eprintln!("day descriptions: {keyframes} keyframes sampled");
}
