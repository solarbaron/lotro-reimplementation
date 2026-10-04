// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic end-to-end tests: context loading, bundles, object resolution, streaming.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tessera_assets::{AssetError, JobContext, PoolConfig, WorkerPool};
use tessera_formats::Did;
use tessera_formats::landblock::{FailureCategory, StaticEntityType};

use crate::testkit::{self, SyntheticWorld, obj};
use crate::*;

fn pool() -> WorkerPool {
    WorkerPool::new(PoolConfig {
        workers: 3,
        ..PoolConfig::default()
    })
}

fn names() -> Arc<FixedAreaNames> {
    let mut f = FixedAreaNames::new();
    f.insert(AreaName {
        area: Did(obj::AREA_A),
        name: Some("West Meadow".into()),
        ..AreaName::default()
    });
    f.insert(AreaName {
        area: Did(obj::AREA_B),
        name: Some("East Fields".into()),
        ..AreaName::default()
    });
    Arc::new(f)
}

fn context(source: MemoryWorldSource) -> Arc<WorldContext> {
    Arc::new(
        WorldContext::load(Arc::new(source), BundleOptions::default())
            .expect("synthetic context")
            .with_area_names(names()),
    )
}

fn key(x: u8, y: u8) -> LandblockKey {
    LandblockKey::new(1, x, y).unwrap()
}

fn bundle_of(ctx: &WorldContext, k: LandblockKey) -> Result<LandblockBundle> {
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let frame = tessera_assets::RecordSource::read_record(&src, k.data_did().0)?;
    build_bundle(ctx, k.data_did(), &frame, &|| false)
}

#[test]
fn context_loads_synthetic_install() {
    let ctx = context(SyntheticWorld::rectangle(1, 10..=12, 20..=21).build());
    let r = ctx.region(1).expect("region 1");
    let m = r.map.expect("map constants");
    assert_eq!(m.origin, [29_360.0, 24_880.0]);
    assert!((m.scale - 0.005).abs() < 1e-9);
    assert!(r.block_map_descriptor.is_some());
    assert_eq!(ctx.regions().len(), 1, "the absent descriptor is skipped");
    assert_eq!(ctx.pids().phys_obj, Some(testkit::pid::PHYS_OBJ));
    assert_eq!(ctx.pids().landblock.area_did, Some(testkit::pid::AREA_DID));
    assert!(ctx.has_landblock(key(10, 20)).unwrap());
    assert!(!ctx.has_landblock(key(13, 20)).unwrap());
    // §8.4 A through the loaded constants.
    let p = LocalPos::new(key(100, 150), [40.0, 80.0, 0.0]).to_world();
    assert_eq!(
        ctx.map_constants(1).unwrap().to_map(p.xy()).to_string(),
        "4.0S, 66.6W"
    );
}

#[test]
fn context_without_dictionary_fails_and_without_regions_degrades() {
    let mut s = SyntheticWorld::rectangle(1, 0..=0, 0..=0).build();
    s.remove(0x2800_0000u32);
    let ctx = WorldContext::load(Arc::new(s.clone()), BundleOptions::default()).unwrap();
    assert!(ctx.regions().is_empty());
    s.remove(0x3400_0000u32);
    assert!(matches!(
        WorldContext::load(Arc::new(s), BundleOptions::default()),
        Err(WorldError::Missing { .. })
    ));
}

#[test]
fn bundle_contents() {
    let ctx = context(SyntheticWorld::rectangle(1, 127..=128, 5..=5).build());
    let b = bundle_of(&ctx, key(127, 5)).unwrap();
    assert!(b.problems.is_empty(), "{:?}", b.problems);
    assert_eq!(b.key, key(127, 5));
    assert_eq!(b.data_stamp, Some(39));
    // Block map.
    let p = &b.block_props;
    assert_eq!(p.area, Some(Did(obj::AREA_A)));
    assert_eq!(p.music_type, Some(6)); // y odd → index 1
    assert_eq!(p.local_sky, Some(Did(0x6A00_0001)));
    assert_eq!(p.use_ceiling, Some(false));
    let undescribed = p
        .values
        .iter()
        .find(|v| v.key == testkit::key::UNDESCRIBED)
        .unwrap();
    assert_eq!((undescribed.pid, undescribed.index), (None, 7));
    assert_eq!(
        b.area.as_ref().and_then(|a| a.display()),
        Some("West Meadow")
    );
    // Minimap.
    let tile = b.minimap.as_ref().unwrap();
    assert_eq!(tile.block_offset, [127, 5]);
    assert_eq!(
        b.minimap_image(|_| false),
        Some(Did(0x4100_0000 | key(127, 5).packed()))
    );
    // Entities: tree, tree, hidden, broken, weenie.
    assert_eq!(b.entities.len(), 5);
    let e = &b.entities;
    assert!(e[0].draw && e[1].draw);
    assert_eq!(e[0].scale, [2.0; 3]);
    assert_eq!(e[0].kind, Some(StaticEntityType::Physics));
    let v = e[0].visual.as_ref().unwrap();
    assert_eq!(v.setup, Some(Did(obj::TREE_SETUP)));
    assert_eq!(&*v.parts, &obj::TREE_PARTS.map(Did));
    assert_eq!(
        v.chain,
        vec![
            Did(obj::TREE),
            Did(obj::TREE_TEMPLATE),
            Did(obj::TREE_SETUP)
        ]
    );
    assert!(!e[2].draw, "Render_Draw = false on the descriptor");
    assert_eq!(e[2].visual.as_ref().unwrap().draw.render_draw, Some(false));
    assert!(!e[3].draw);
    assert!(e[3].visual.as_ref().unwrap().error.is_some());
    assert_eq!(e[4].kind, Some(StaticEntityType::Weenie));
    assert!(!e[4].draw, "weenies are not drawn in M3");
    let w = e[4].visual.as_ref().unwrap();
    assert_eq!(w.physics, Some(Did(obj::TREE)));
    assert!(w.has_geometry());
    assert_eq!(b.drawable().count(), 2);
    assert!(b.heap_bytes > 4_000);
    // Height: slope in X, continuous across the block edge.
    let h_edge = b.terrain_height(160.0, 10.0).unwrap();
    let east = bundle_of(&ctx, key(128, 5)).unwrap();
    assert!((east.terrain_height(0.0, 10.0).unwrap() - h_edge).abs() < 1e-3);
    assert_eq!(
        east.area.as_ref().and_then(|a| a.display()),
        Some("East Fields")
    );
    // The resolver cached the chains.
    let s = ctx.objects().stats();
    assert!(s.hits > 0 && s.resolved >= 2 && s.failed >= 1);
}

#[test]
fn bundle_tolerates_missing_and_broken_records() {
    let k = key(3, 3);
    let mut s = SyntheticWorld::rectangle(1, 3..=3, 3..=3).build();
    s.remove(k.block_map_did());
    s.remove(k.properties_did());
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert!(b.block_map.is_none() && b.minimap.is_none() && b.area.is_none());
    assert_eq!(b.entities.len(), 5);

    // A corrupt info record is a problem, not a failure.
    s.insert(k.info_did(), vec![1, 2, 3], Some(39));
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert!(b.info.is_none() && b.entities.is_empty());
    assert_eq!(b.problems.len(), 1);
    assert_eq!(b.problems[0].category, Some(FailureCategory::Truncated));

    // Info named but absent.
    s.remove(k.info_did());
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert_eq!(b.problems.len(), 1);
    assert!(b.problems[0].category.is_none());

    // Corrupt data fails the bundle.
    s.insert(k.data_did(), vec![0; 10], Some(39));
    let ctx = context(s);
    assert!(matches!(bundle_of(&ctx, k), Err(WorldError::Format { .. })));
}

#[test]
fn bundle_rejects_bad_input() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=0, 0..=0).build());
    let k = key(0, 0);
    let frame = tessera_assets::RecordSource::read_record(
        &BundleSource::new(Arc::clone(ctx.source())),
        k.data_did().0,
    )
    .unwrap();
    assert!(build_bundle(&ctx, k.data_did(), &frame, &|| false).is_ok());
    assert!(matches!(
        build_bundle(&ctx, k.data_did(), &frame, &|| true),
        Err(WorldError::Cancelled)
    ));
    assert!(build_bundle(&ctx, k.block_map_did(), &frame, &|| false).is_err());
    assert!(build_bundle(&ctx, key(1, 1).data_did(), &frame, &|| false).is_err());
    assert!(build_bundle(&ctx, k.data_did(), &frame[..frame.len() - 1], &|| false).is_err());
    assert!(build_bundle(&ctx, k.data_did(), b"", &|| false).is_err());
    // Through the decoder trait.
    let d = BundleDecoder::new(Arc::clone(&ctx));
    let out =
        tessera_assets::Decoder::decode(&d, k.data_did().0.into(), frame, &JobContext::detached())
            .unwrap();
    assert_eq!(tessera_assets::Decoder::size_of(&d, &out), out.heap_bytes);
}

#[test]
fn content_layer_filter() {
    let k = key(0, 0);
    let mut s = SyntheticWorld::rectangle(1, 0..=0, 0..=0).build();
    let layers = |l: &[u32]| {
        (
            testkit::pid::ENTITY_CONTENT_LAYERS,
            testkit::TestValue::Array(
                l.iter()
                    .map(|&x| {
                        (
                            testkit::pid::ENTITY_CONTENT_LAYER,
                            testkit::TestValue::Enum(x),
                        )
                    })
                    .collect(),
            ),
        )
    };
    let ents = vec![
        testkit::TestEntity::physics(1, obj::TREE, [1.0, 1.0, 1.0], vec![layers(&[1])]),
        testkit::TestEntity::physics(2, obj::TREE, [2.0, 2.0, 1.0], vec![layers(&[2])]),
        testkit::TestEntity::physics(3, obj::TREE, [3.0, 3.0, 1.0], vec![layers(&[])]),
        testkit::TestEntity::physics(
            4,
            obj::HIDDEN,
            [4.0, 4.0, 1.0],
            vec![(testkit::pid::RENDER_DRAW, testkit::TestValue::Bool(true))],
        ),
        testkit::TestEntity {
            position: None,
            ..testkit::TestEntity::physics(5, obj::TREE, [0.0; 3], Vec::new())
        },
    ];
    s.insert(k.info_did(), testkit::landblock_info(k, &ents), Some(39));
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    let draw: Vec<bool> = b.entities.iter().map(|e| e.draw).collect();
    assert_eq!(draw, vec![true, false, true, true]);
    assert_eq!(b.unplaced_entities, 1);
    assert_eq!(b.entities[1].content_layers.as_deref(), Some(&[2u32][..]));

    let opts = BundleOptions {
        active_content_layers: vec![2],
        resolve_objects: false,
        ..BundleOptions::default()
    };
    let ctx = WorldContext::load(Arc::new(s), opts).unwrap();
    let b = bundle_of(&ctx, k).unwrap();
    assert!(b.entities.iter().all(|e| e.visual.is_none() && !e.draw));
}

#[test]
fn walker_streams_across_a_strip() {
    let world = SyntheticWorld::rectangle(1, 0..=40, 0..=6);
    let ctx = context(world.build());
    let pool = pool();
    let rings = RingConfig {
        near: 160.0,
        mid: 320.0,
        far: 480.0,
        hysteresis: 160.0,
    };
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings,
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let y = 3.0 * 160.0 + 80.0;
    let mut x = 80.0;
    let mut max_resident = 0;
    while x < 40.0 * 160.0 {
        let p = WorldPos::new(1, x, y, 0.0);
        let r = w.update(p);
        max_resident = max_resident.max(r.resident_blocks);
        // Nothing resident lies beyond the far radius plus hysteresis.
        for (k, _, _) in w.resident() {
            assert!(k.distance_to(p.xy()) <= 640.0 + 1e-9, "{k} too far");
        }
        x += 40.0;
        std::thread::sleep(Duration::from_micros(300));
    }
    let end = WorldPos::new(1, 39.5 * 160.0, y, 0.0);
    assert!(w.settle(end, Duration::from_secs(20)));
    let near: Vec<LandblockKey> = w
        .resident()
        .filter(|(_, r, _)| *r == Ring::Near)
        .map(|(k, _, _)| k)
        .collect();
    assert!(!near.is_empty());
    assert_eq!(
        w.area_at(&end).and_then(|a| a.display()),
        Some("West Meadow")
    );
    assert_eq!(w.block_props_at(&end).unwrap().music_type, Some(6));
    assert!(w.terrain_height_at(&end).is_some());
    assert_eq!(
        w.map_coords(&end).unwrap(),
        ctx.map_constants(1).unwrap().to_map(end.xy())
    );
    let tiles = w.minimap_tiles(end.landblock().unwrap(), 1, |_| false);
    assert_eq!(tiles.len(), 9);
    let st = w.stats();
    assert!(st.released > 0 && st.integrated > 0, "{st:?}");
    assert!(st.jobs > 0 && st.mean_job() > Duration::ZERO);
    assert!(st.peak_resident_blocks <= 13 * 13);
    assert!(max_resident > 0);
    assert!(w.resident_bytes() > 0);
    assert!(w.sample_memory() >= w.resident_bytes());
    w.clear();
    assert_eq!(w.resident_bytes(), 0);
    assert_eq!(w.resident().count(), 0);
}

#[test]
fn missing_landblocks_fail_softly() {
    let ctx = context(SyntheticWorld::rectangle(1, 10..=10, 10..=10).build());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    let p = LocalPos::new(key(10, 10), [80.0, 80.0, 0.0]).to_world();
    assert!(w.settle(p, Duration::from_secs(20)));
    assert!(w.bundle(key(10, 10)).is_some());
    assert!(matches!(
        w.state_of(key(11, 10)),
        Some(BlockState::Failed(AssetError::NotFound { .. }))
    ));
    let far = LocalPos::new(key(11, 10), [80.0, 80.0, 0.0]).to_world();
    assert!(w.area_at(&far).is_none());
    assert!(w.state_of(key(200, 200)).is_none());
}

#[test]
fn residency_budget_evicts_farthest() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=20, 0..=20).build());
    let pool = pool();
    let probe = bundle_of(&ctx, key(10, 10)).unwrap().heap_bytes;
    let budget = probe * 20;
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 160.0,
                mid: 480.0,
                far: 800.0,
                hysteresis: 160.0,
            },
            residency_budget: budget,
            initial_size_estimate: probe,
            always_admit_near: false,
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let c = LocalPos::new(key(10, 10), [80.0, 80.0, 0.0]).to_world();
    assert!(w.settle(c, Duration::from_secs(20)));
    let r = w.update(c);
    assert!(r.deferred > 0, "{r:?}");
    assert!(
        w.resident_bytes() <= budget + probe,
        "{} > {budget}",
        w.resident_bytes()
    );
    // The nearest are resident, the farthest deferred.
    let max_res = w
        .resident()
        .map(|(k, _, _)| k.distance_to(c.xy()))
        .fold(0.0, f64::max);
    for d in [key(5, 10), key(15, 10)] {
        assert!(matches!(w.state_of(d), Some(BlockState::Deferred)));
        assert!(d.distance_to(c.xy()) >= max_res);
    }
    // Shrinking the reachable set by moving evicts nothing near; moving far evicts.
    let east = LocalPos::new(key(14, 10), [80.0, 80.0, 0.0]).to_world();
    assert!(w.settle(east, Duration::from_secs(20)));
    assert!(w.stats().evicted > 0 || w.stats().released > 0);
    assert!(w.resident_bytes() <= budget + probe);
    assert_seam_closure(&w, true);

    // With the near ring forced, a budget of 4 bundles still keeps the near ring and its
    // seam closure resident (and nothing else).
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            residency_budget: probe * 4,
            always_admit_near: true,
            ..w.config().clone()
        },
    )
    .unwrap();
    assert!(w.settle(c, Duration::from_secs(20)));
    for k in w.rings().members().keys() {
        if w.ring_of(*k) == Some(Ring::Near) {
            assert!(w.bundle(*k).is_some(), "{k}");
        }
    }
    assert_seam_closure(&w, false);
}

/// A source that delays every read, to keep requests in flight.
struct SlowSource {
    inner: MemoryWorldSource,
    delay: Duration,
    reads: AtomicU64,
}

impl WorldSource for SlowSource {
    fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(self.delay);
        self.inner.read_stamped(did)
    }
}

#[test]
fn leaving_cancels_in_flight_requests() {
    let src = Arc::new(SlowSource {
        inner: SyntheticWorld::rectangle(1, 0..=60, 0..=3).build(),
        delay: Duration::from_millis(5),
        reads: AtomicU64::new(0),
    });
    let ctx = Arc::new(WorldContext::load(src.clone(), BundleOptions::default()).unwrap());
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, StreamConfig::default()).unwrap();
    let a = LocalPos::new(key(5, 1), [80.0, 80.0, 0.0]).to_world();
    let r = w.update(a);
    assert!(r.requested > 10);
    // Teleport far away before most finished.
    let b = LocalPos::new(key(55, 1), [80.0, 80.0, 0.0]).to_world();
    let r = w.update(b);
    assert!(r.left > 0);
    assert!(r.cancelled > 0, "{r:?}");
    assert!(w.settle(b, Duration::from_secs(60)));
    assert!(w.resident().all(|(k, _, _)| k.x() >= 40));
    assert!(w.stats().cancelled > 0);
}

#[test]
fn region_change_releases_everything() {
    let mut s = SyntheticWorld::rectangle(1, 0..=2, 0..=2).build();
    // Region 2 shares nothing; its blocks are simply missing.
    s.insert(
        LandblockKey::new(2, 1, 1).unwrap().data_did(),
        testkit::landblock_data(LandblockKey::new(2, 1, 1).unwrap(), false, |_, _| 7),
        Some(39),
    );
    let ctx = context(s);
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    assert!(w.settle(WorldPos::new(1, 240.0, 240.0, 0.0), Duration::from_secs(20)));
    assert!(w.resident().count() >= 9);
    let p2 = WorldPos::new(2, 240.0, 240.0, 0.0);
    assert!(w.settle(p2, Duration::from_secs(20)));
    assert!(w.resident().all(|(k, _, _)| k.region() == 2));
    assert_eq!(w.resident().count(), 1);
    assert!(w.map_coords(&p2).is_none(), "region 2 has no descriptor");
    let flat = w.terrain_height_at(&p2).unwrap();
    assert!((flat - 7.0 / 32.768).abs() < 1e-4);
}

#[test]
fn invalid_config_is_rejected() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=0, 0..=0).build());
    let pool = pool();
    let cfg = StreamConfig {
        rings: RingConfig {
            near: 10.0,
            mid: 5.0,
            far: 20.0,
            hysteresis: 0.0,
        },
        ..StreamConfig::default()
    };
    assert!(matches!(
        WorldStreamer::new(ctx, &pool, cfg),
        Err(WorldError::Config(_))
    ));
}

/// A source that panics when reading one DID and counts those reads.
struct PanicSource {
    inner: MemoryWorldSource,
    bad: Did,
    bad_reads: AtomicU64,
}

impl WorldSource for PanicSource {
    fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
        if did == self.bad {
            self.bad_reads.fetch_add(1, Ordering::Relaxed);
            panic!("hostile record (synthetic test panic)");
        }
        self.inner.read_stamped(did)
    }
}

/// A source whose reads of one DID fail with an I/O-style error.
struct FailingSource {
    inner: MemoryWorldSource,
    bad: Did,
}

impl WorldSource for FailingSource {
    fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
        if did == self.bad {
            return Err(AssetError::source_error(
                did.0,
                std::io::Error::other("bad sector"),
            ));
        }
        self.inner.read_stamped(did)
    }
}

fn centre(x: u8, y: u8) -> WorldPos {
    LocalPos::new(key(x, y), [80.0, 80.0, 0.0]).to_world()
}

/// A 21 × 21 world whose odd-X blocks carry 400 entities, so bundle sizes differ a lot.
fn mixed_size_world() -> MemoryWorldSource {
    let mut s = SyntheticWorld::rectangle(1, 0..=20, 0..=20).build();
    let ents: Vec<testkit::TestEntity> = (0..400u32)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let p = [(i % 20) as f32 * 8.0, (i / 20) as f32 * 8.0, 1.0];
            testkit::TestEntity::physics(i + 1, obj::TREE, p, Vec::new())
        })
        .collect();
    for y in 0..=20u8 {
        for x in (1..=20u8).step_by(2) {
            s.insert(
                key(x, y).info_did(),
                testkit::landblock_info(key(x, y), &ents),
                Some(39),
            );
        }
    }
    s
}

#[test]
fn mixed_sizes_do_not_thrash_at_a_fixed_focus() {
    let ctx = context(mixed_size_world());
    let small = bundle_of(&ctx, key(10, 10)).unwrap().heap_bytes;
    let large = bundle_of(&ctx, key(11, 10)).unwrap().heap_bytes;
    assert!(large > 4 * small, "{small} vs {large}");
    let pool = pool();
    let c = centre(10, 10);
    for budget in [
        large * 3 + small,
        small * 20 + large * 7,
        large * 12,
        large * 30 + small * 5,
    ] {
        let mut w = WorldStreamer::new(
            Arc::clone(&ctx),
            &pool,
            StreamConfig {
                rings: RingConfig {
                    near: 160.0,
                    mid: 480.0,
                    far: 800.0,
                    hysteresis: 160.0,
                },
                residency_budget: budget,
                always_admit_near: false,
                initial_size_estimate: small,
                ..StreamConfig::default()
            },
        )
        .unwrap();
        assert!(w.settle(c, Duration::from_secs(30)), "budget {budget}");
        let before = w.stats().clone();
        for _ in 0..200 {
            let r = w.update(c);
            assert!(r.resident_bytes <= budget, "budget {budget}: {r:?}");
            std::thread::sleep(Duration::from_micros(200));
        }
        let after = w.stats();
        assert_eq!(after.evicted, before.evicted, "budget {budget}");
        assert_eq!(after.requested, before.requested, "budget {budget}");
        assert_eq!(after.integrated, before.integrated, "budget {budget}");
        // Eviction is strictly farthest first: nothing deferred is nearer than a resident
        // block that is not merely a seam neighbour of a resident near/mid block (those
        // are pulled in with the block that needs them).
        let neighbour_of_resident = |k: LandblockKey| {
            w.resident()
                .any(|(b, r, _)| r != Ring::Far && b.seam_neighbours().contains(&Some(k)))
        };
        let anchor = w
            .resident()
            .filter(|(k, _, _)| !neighbour_of_resident(*k))
            .map(|(k, _, _)| k.distance_to(c.xy()))
            .fold(0.0, f64::max);
        for k in w.rings().members().keys() {
            if matches!(w.state_of(*k), Some(BlockState::Deferred)) {
                assert!(k.distance_to(c.xy()) >= anchor, "budget {budget}: {k}");
            }
        }
        assert_seam_closure(&w, true);
    }
}

/// Every resident near-ring block (and with `mid`, every resident mid-ring block) has its
/// desired seam neighbours resident.
fn assert_seam_closure(w: &WorldStreamer, mid: bool) -> usize {
    let mut checked = 0;
    for (k, ring, _) in w.resident() {
        if ring == Ring::Far || (ring == Ring::Mid && !mid) {
            continue;
        }
        for n in k.seam_neighbours().into_iter().flatten() {
            match w.state_of(n) {
                None | Some(BlockState::Ready(_) | BlockState::Failed(_)) => {}
                Some(other) => panic!("{k} ({ring:?}): neighbour {n} is {other:?}"),
            }
            checked += 1;
        }
    }
    checked
}

#[test]
fn seam_neighbours_of_near_and_mid_blocks_stay_resident() {
    let ctx = context(SyntheticWorld::rectangle(1, 80..=120, 80..=120).build());
    let est = bundle_of(&ctx, key(100, 100)).unwrap().heap_bytes;
    let pool = pool();
    let c = centre(100, 100);
    for always_admit_near in [false, true] {
        for n in [1usize, 60, 120] {
            let what = format!("budget x{n}, forced near {always_admit_near}");
            let mut w = WorldStreamer::new(
                Arc::clone(&ctx),
                &pool,
                StreamConfig {
                    residency_budget: est * n,
                    initial_size_estimate: est,
                    always_admit_near,
                    ..StreamConfig::default()
                },
            )
            .unwrap();
            assert!(w.settle(c, Duration::from_secs(30)), "{what}");
            // Without forcing, mid-ring blocks have their neighbours too; with forcing,
            // near-ring blocks always do (mid-ring donors may wait for budget).
            let checked = assert_seam_closure(&w, !always_admit_near);
            if always_admit_near || n > 1 {
                assert!(checked > 0, "{what}");
                let near = w
                    .resident()
                    .find(|(_, r, _)| *r == Ring::Near)
                    .map(|(k, _, _)| k)
                    .unwrap();
                assert!(
                    w.seam_neighbours(near).iter().all(Option::is_some),
                    "{what}"
                );
            }
            // And after moving.
            for step in 0..40 {
                let p = WorldPos::new(1, c.x + f64::from(step) * 40.0, c.y + 13.0, 0.0);
                w.update(p);
            }
            assert!(
                w.settle(
                    WorldPos::new(1, c.x + 1600.0, c.y + 13.0, 0.0),
                    Duration::from_secs(30)
                ),
                "{what}"
            );
            assert_seam_closure(&w, !always_admit_near);
            if !always_admit_near {
                assert!(w.resident_bytes() <= est * n, "{what}");
            }
        }
    }
}

#[test]
fn panicking_job_is_not_retried_while_desired() {
    let k = key(5, 5);
    let src = Arc::new(PanicSource {
        inner: SyntheticWorld::rectangle(1, 0..=40, 0..=10).build(),
        bad: k.data_did(),
        bad_reads: AtomicU64::new(0),
    });
    let ctx = Arc::new(WorldContext::load(src.clone(), BundleOptions::default()).unwrap());
    let pool = pool();
    let rings = RingConfig {
        near: 160.0,
        mid: 320.0,
        far: 480.0,
        hysteresis: 160.0,
    };
    let mut w = WorldStreamer::new(
        ctx,
        &pool,
        StreamConfig {
            rings,
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let c = centre(5, 5);
    assert!(w.settle(c, Duration::from_secs(20)));
    let failed = w.stats().failed;
    for _ in 0..300 {
        let r = w.update(c);
        assert_eq!(r.requested, 0);
    }
    assert_eq!(src.bad_reads.load(Ordering::Relaxed), 1);
    assert_eq!(w.stats().failed, failed);
    assert!(matches!(
        w.state_of(k),
        Some(BlockState::Failed(AssetError::Panicked { .. }))
    ));
    // Leaving and coming back tries once more.
    assert!(w.settle(centre(30, 5), Duration::from_secs(20)));
    assert!(w.state_of(k).is_none());
    assert!(w.settle(c, Duration::from_secs(20)));
    assert_eq!(src.bad_reads.load(Ordering::Relaxed), 2);
}

#[test]
fn unreadable_optional_record_still_delivers_terrain() {
    let k = key(1, 1);
    let src = Arc::new(FailingSource {
        inner: SyntheticWorld::rectangle(1, 0..=3, 0..=3).build(),
        bad: k.properties_did(),
    });
    let ctx = Arc::new(WorldContext::load(src, BundleOptions::default()).unwrap());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    let c = centre(1, 1);
    assert!(w.settle(c, Duration::from_secs(20)));
    let b = w
        .bundle(k)
        .expect("resident despite the unreadable properties record");
    assert!(w.terrain_height_at(&c).is_some());
    assert!(b.minimap.is_none() && b.properties.is_none());
    assert_eq!(b.problems.len(), 1, "{:?}", b.problems);
    let p = &b.problems[0];
    assert!(p.unreadable && p.category.is_none());
    assert_eq!(p.did, k.properties_did());
    assert!(p.to_string().contains("bad sector"), "{p}");
    assert!(
        p.to_string().starts_with("landblock properties 0x8041"),
        "{p}"
    );
    // Entities and block map are intact.
    assert_eq!(b.entities.len(), 5);
    assert!(b.block_map.is_some());
}

#[test]
fn bad_focus_is_ignored() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=10, 0..=10).build());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    // Before any valid focus: nothing happens.
    let r = w.update(WorldPos::new(1, f64::NAN, 0.0, 0.0));
    assert!(r.focus_rejected && r.requested == 0 && r.entered == 0);
    assert!(w.focus().is_none());
    let c = centre(5, 5);
    assert!(w.settle(c, Duration::from_secs(20)));
    let resident = w.resident().count();
    assert!(resident > 0);
    for bad in [
        WorldPos::new(1, f64::NAN, 0.0, 0.0),
        WorldPos::new(1, 0.0, f64::INFINITY, 0.0),
        WorldPos::new(1, 0.0, 0.0, f64::NAN),
        WorldPos::new(0, 100.0, 100.0, 0.0),
        WorldPos::new(16, 100.0, 100.0, 0.0),
    ] {
        let r = w.update(bad);
        assert!(r.focus_rejected, "{bad:?}");
        assert_eq!((r.left, r.entered, r.evicted), (0, 0, 0), "{bad:?}");
        assert_eq!(r.resident_blocks, resident, "{bad:?}");
        assert_eq!(w.focus(), Some(c));
    }
    assert_eq!(w.stats().rejected_focus, 6);
    // Explicit clear still releases everything.
    w.clear();
    assert_eq!(w.resident().count(), 0);
}

#[test]
fn stream_config_is_validated() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=0, 0..=0).build());
    let pool = pool();
    let d = StreamConfig::default();
    assert!(d.validate().is_ok());
    for bad in [
        StreamConfig {
            frame_budget: Duration::ZERO,
            ..d.clone()
        },
        StreamConfig {
            max_integrations_per_frame: 0,
            ..d.clone()
        },
        StreamConfig {
            residency_budget: 0,
            ..d.clone()
        },
    ] {
        assert!(bad.validate().is_err());
        assert!(matches!(
            WorldStreamer::new(Arc::clone(&ctx), &pool, bad),
            Err(WorldError::Config(_))
        ));
    }
}

#[test]
fn integration_progresses_when_bookkeeping_exceeds_the_frame_budget() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=20, 0..=20).build());
    let pool = pool();
    let mut w = WorldStreamer::new(
        ctx,
        &pool,
        StreamConfig {
            // Large valid rings and a frame budget no update can meet.
            rings: RingConfig {
                near: 480.0,
                mid: 2_000.0,
                far: 6_240.0,
                hysteresis: 160.0,
            },
            frame_budget: Duration::from_nanos(1),
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let c = centre(10, 10);
    assert!(w.settle(c, Duration::from_secs(60)));
    assert_eq!(w.resident().count(), 21 * 21);
}

#[test]
fn landblock_presence_without_reading() {
    // `has_landblock` goes through `WorldSource::contains`, which a counting source can
    // observe: no payload read.
    struct Counting {
        inner: MemoryWorldSource,
        reads: AtomicU64,
    }
    impl WorldSource for Counting {
        fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
            self.reads.fetch_add(1, Ordering::Relaxed);
            self.inner.read_stamped(did)
        }
        fn contains(&self, did: Did) -> std::result::Result<bool, AssetError> {
            self.inner.contains(did)
        }
    }
    let src = Arc::new(Counting {
        inner: SyntheticWorld::rectangle(1, 3..=3, 3..=3).build(),
        reads: AtomicU64::new(0),
    });
    let ctx = WorldContext::load(src.clone(), BundleOptions::default()).unwrap();
    let before = src.reads.load(Ordering::Relaxed);
    assert!(ctx.has_landblock(key(3, 3)).unwrap());
    assert!(!ctx.has_landblock(key(4, 3)).unwrap());
    assert_eq!(src.reads.load(Ordering::Relaxed), before);
}

#[test]
fn wrong_bundle_key_is_reported_as_such() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=0, 0..=0).build());
    let k = key(0, 0);
    let e = build_bundle(&ctx, k.block_map_did(), b"", &|| false).unwrap_err();
    assert!(matches!(e, WorldError::NotABundleKey { .. }), "{e}");
    assert!(e.to_string().contains("not a landblock-data DID"));
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let e = tessera_assets::RecordSource::read_record(&src, 0x4100_0000).unwrap_err();
    assert!(e.to_string().contains("not a landblock-data DID"), "{e}");
}

#[test]
fn object_cache_is_bounded() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=0, 0..=0).build());
    let r = ObjectResolver::new(
        Arc::clone(ctx.source()),
        Arc::new(testkit::schema()),
        ctx.pids().clone(),
        2,
    );
    for d in [obj::TREE, obj::HIDDEN, 0x4700_7777, 0x4100_0000, obj::TREE] {
        let _ = r.resolve(Did(d));
        assert!(r.stats().cached <= 2, "{:?}", r.stats());
    }
    // Results stay correct across clears.
    let t = r.resolve(Did(obj::TREE));
    assert_eq!(t.setup, Some(Did(obj::TREE_SETUP)));
    assert!(r.resolve(Did(0x4100_0000)).error.is_some());
    r.clear();
    assert_eq!(r.stats().cached, 0);
    assert!(r.cached(Did(obj::TREE)).is_none());
}
