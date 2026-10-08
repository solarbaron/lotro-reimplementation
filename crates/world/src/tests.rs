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
    // Block maps are dense (`landblock.md` §2.3 LB-E1): an absent one is a problem (§8.1);
    // an absent properties record is not (LB-E2).
    assert_eq!(b.problems.len(), 1, "{:?}", b.problems);
    let p = &b.problems[0];
    assert_eq!((p.did, p.what), (k.block_map_did(), "block map"));
    assert!(p.category.is_none() && !p.unreadable && !p.transient);
    assert!(b.is_complete());

    // A corrupt info record is a problem, not a failure.
    s.insert(k.info_did(), vec![1, 2, 3], Some(39));
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert!(b.info.is_none() && b.entities.is_empty());
    assert_eq!(b.problems.len(), 2);
    assert_eq!(b.problems[1].category, Some(FailureCategory::Truncated));

    // Info named but absent.
    s.remove(k.info_did());
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert_eq!(b.problems.len(), 2);
    assert!(b.problems[1].category.is_none());
    assert_eq!(b.problems[1].did, k.info_did());

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
        // A named layer that is not active.
        testkit::TestEntity::physics(2, obj::TREE, [2.0, 2.0, 1.0], vec![layers(&[0x1000_02A8])]),
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
        // `All` (2) is present in every layer and instance (`entity-descriptor.md` §4.5.1).
        testkit::TestEntity::physics(6, obj::TREE, [6.0, 6.0, 1.0], vec![layers(&[2])]),
    ];
    s.insert(k.info_did(), testkit::landblock_info(k, &ents), Some(39));
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    let draw: Vec<bool> = b.entities.iter().map(|e| e.draw).collect();
    assert_eq!(draw, vec![true, false, true, true, true, true]);
    // The entity without a frame stands at the landblock origin with the identity
    // rotation (`entity-descriptor.md` §4.4), drawn like the others; it is counted.
    assert_eq!(b.unplaced_entities, 1);
    let origin = &b.entities[4];
    assert_eq!(origin.index, 4);
    assert_eq!(origin.placement.position, [0.0; 3]);
    assert_eq!(origin.placement.rotation.to_wxyz(), [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(b.entities[5].content_layers.as_deref(), Some(&[2u32][..]));

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
    // The rings alone, and the default lookahead (velocity estimated by `update`).
    for lookahead in [Duration::ZERO, StreamConfig::default().lookahead] {
        walk_across_a_strip(lookahead);
    }
}

fn walk_across_a_strip(lookahead: Duration) {
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
            lookahead,
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let y = 3.0 * 160.0 + 80.0;
    let mut x = 80.0;
    let mut max_resident = 0;
    // From cold: only the start block is waited for (so the walk has something resident
    // even when the machine is too busy to finish a bundle during the walk itself); the
    // rest of the rings load while walking.
    let start = WorldPos::new(1, x, y, 0.0);
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while w.bundle_at(&start).is_none() {
        let _ = w.update(start);
        assert!(
            std::time::Instant::now() < deadline,
            "start block not loaded"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    while x < 40.0 * 160.0 {
        let p = WorldPos::new(1, x, y, 0.0);
        let r = w.update(p);
        max_resident = max_resident.max(r.resident_blocks - r.ahead_resident);
        // Nothing resident lies beyond the far radius plus hysteresis (bundles loaded
        // ahead of the rings are not in `resident`).
        for (k, _, _) in w.resident() {
            assert!(k.distance_to(p.xy()) <= 640.0 + 1e-9, "{k} too far");
            assert!(!w.is_ahead(k));
        }
        if lookahead.is_zero() {
            assert_eq!((r.ahead, r.corridor), (0, 0), "{r:?}");
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

/// `settle` with `Duration::MAX` waits without a deadline instead of overflowing the
/// `Instant` arithmetic (audit finding; matches `tessera_assets::Handle::wait`).
#[test]
fn settle_accepts_an_unbounded_timeout() {
    let ctx = context(SyntheticWorld::rectangle(1, 9..=11, 9..=11).build());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    let c = centre(10, 10);
    assert!(w.settle(c, Duration::MAX));
    assert!(w.bundle(key(10, 10)).is_some());
    // Already settled: returns at once.
    assert!(w.settle(c, Duration::MAX));
}

/// Checks the farthest-first property of the admitted set at `focus` (`landblock.md` §8.1,
/// "Residency admission"): every near-ring member is resident, and no deferred ring member
/// is nearer than a resident block that is not merely a seam neighbour of a resident
/// near/mid block (those are pulled in with the block that needs them). Returns the
/// numbers of resident and deferred ring members.
fn assert_farthest_first(w: &WorldStreamer, focus: WorldPos) -> (usize, usize) {
    for k in w.rings().members().keys() {
        if w.ring_of(*k) == Some(Ring::Near) {
            assert!(
                matches!(w.state_of(*k), Some(BlockState::Ready(_))),
                "near block {k} is {:?}",
                w.state_of(*k)
            );
        }
    }
    let neighbour_of_resident = |k: LandblockKey| {
        w.resident()
            .any(|(b, r, _)| r != Ring::Far && b.seam_neighbours().contains(&Some(k)))
    };
    let anchor = w
        .resident()
        .filter(|(k, _, _)| !neighbour_of_resident(*k))
        .map(|(k, _, _)| k.distance_to(focus.xy()))
        .fold(0.0, f64::max);
    let mut deferred = 0;
    for k in w.rings().members().keys() {
        if matches!(w.state_of(*k), Some(BlockState::Deferred)) {
            assert!(
                k.distance_to(focus.xy()) >= anchor,
                "{k} nearer than {anchor}"
            );
            deferred += 1;
        }
    }
    (w.resident().count(), deferred)
}

#[test]
fn residency_budget_evicts_farthest() {
    let ctx = context(SyntheticWorld::rectangle(1, 0..=20, 0..=20).build());
    let pool = pool();
    let probe = bundle_of(&ctx, key(10, 10)).unwrap().heap_bytes;
    // Enough for the seam closures of the near ring (about 50 bundles with these rings)
    // but far less than the whole desired set (about 200).
    let budget = probe * 60;
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
    let c = centre(10, 10);
    assert!(w.settle(c, Duration::from_secs(20)));
    let r = w.update(c);
    assert!(r.deferred > 0, "{r:?}");
    assert!(
        w.resident_bytes() <= budget + probe,
        "{} > {budget}",
        w.resident_bytes()
    );
    // The nearest are resident, the farthest deferred.
    let (resident, deferred) = assert_farthest_first(&w, c);
    assert!(resident > 9 && deferred > 0, "{resident} / {deferred}");
    let max_res = w
        .resident()
        .map(|(k, _, _)| k.distance_to(c.xy()))
        .fold(0.0, f64::max);
    for d in [key(5, 10), key(15, 10)] {
        assert!(matches!(w.state_of(d), Some(BlockState::Deferred)), "{d}");
        assert!(d.distance_to(c.xy()) >= max_res);
    }
    assert_seam_closure(&w, true);

    // Moving east evicts nothing near the new focus; it releases what left the rings and
    // evicts resident blocks that are now the farthest.
    let before = w.stats().clone();
    let east = centre(14, 10);
    assert!(w.settle(east, Duration::from_secs(20)));
    let after = w.stats().clone();
    // Block (5, 10) is 1 360 units from the new focus, beyond far + hysteresis.
    assert!(after.released > before.released);
    assert!(w.state_of(key(5, 10)).is_none());
    assert!(after.evicted > before.evicted, "{before:?} -> {after:?}");
    assert!(w.resident_bytes() <= budget + probe);
    let (resident, deferred) = assert_farthest_first(&w, east);
    assert!(resident > 9 && deferred > 0, "{resident} / {deferred}");
    assert_seam_closure(&w, true);

    // With the near ring forced, a budget of 4 bundles still keeps the near ring and its
    // seam closure resident (and nothing else), at the start and after moving.
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
    let near_ring_resident = |w: &WorldStreamer| {
        let mut n = 0;
        for k in w.rings().members().keys() {
            if w.ring_of(*k) == Some(Ring::Near) {
                assert!(w.bundle(*k).is_some(), "{k}");
                n += 1;
            }
        }
        n
    };
    assert_eq!(near_ring_resident(&w), 9);
    assert_seam_closure(&w, false);
    let before = w.stats().evicted;
    assert!(w.settle(east, Duration::from_secs(20)));
    assert_eq!(near_ring_resident(&w), 9);
    // The old near ring is still in the (mid) rings but over the budget: evicted.
    assert!(w.stats().evicted > before);
    assert!(w.bundle(key(10, 10)).is_none());
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
    // Without the world systems, whose collision sets give every bundle a large common
    // base (their charge is covered by `world_systems_are_charged_to_residency`).
    let ctx = Arc::new(
        WorldContext::load(
            Arc::new(mixed_size_world()),
            BundleOptions::default().without_world_systems(),
        )
        .expect("synthetic context"),
    );
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
            // And after moving (the rings alone: with no velocity, nothing is loaded
            // ahead, so the result does not depend on how fast the loop runs).
            for step in 0..40 {
                let p = WorldPos::new(1, c.x + f64::from(step) * 40.0, c.y + 13.0, 0.0);
                w.update_with_velocity(p, [0.0, 0.0]);
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
        StreamConfig {
            frill_budget: 0,
            ..d.clone()
        },
        StreamConfig {
            max_frill_jobs_per_frame: 0,
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
    // A generous deadline: 441 bundles with world systems take most of a minute in a
    // debug build on a busy machine; the test is about progress, not speed.
    assert!(w.settle(c, Duration::from_secs(300)));
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

#[test]
fn figures_take_their_instance_sets() {
    let mut w = SyntheticWorld::rectangle(1, 3..=3, 4..=4);
    w.figures = true;
    let ctx = context(w.build());
    let b = bundle_of(&ctx, key(3, 4)).unwrap();
    assert!(b.problems.is_empty(), "{:?}", b.problems);
    // Entities: tree (NPC-like instance set), tree (plain instance set), hidden, broken,
    // weenie.
    assert_eq!(b.figures.len(), 2, "{:?}", b.figures);
    let npc = &b.figures[0];
    assert_eq!(npc.kind, FigureKind::Npc);
    assert_eq!(npc.entity, 0);
    assert_eq!(npc.idle, Some(SyntheticWorld::FIGURE_IDLE));
    assert_eq!(npc.random_appearance, None);
    assert_eq!(npc.weenie_type, None);
    let set = npc.instance.as_ref().expect("instance set kept");
    assert!(set.get(testkit::pid::NPC_STARTING_IDLE).is_some());
    assert_eq!(b.entities[0].figure, Some(0));
    assert!(b.entities[0].draw, "the draw rule is unchanged");
    assert_eq!(b.entities[1].figure, None, "no NPC property");

    let weenie = &b.figures[1];
    assert_eq!(weenie.kind, FigureKind::Weenie);
    assert_eq!(weenie.entity, 4);
    assert_eq!(
        weenie.random_appearance,
        Some(SyntheticWorld::FIGURE_RANDOM)
    );
    assert_eq!(weenie.weenie_type, Some(SyntheticWorld::WEENIE_TYPE));
    assert_eq!(
        b.figure_entity(weenie).and_then(|e| e.object),
        Some(Did(obj::WEENIE))
    );
    // Scenery excludes the figures; drawable still includes the NPC-like tree.
    assert_eq!(b.drawable().count(), 2);
    assert_eq!(b.scenery().count(), 1);
    // The rest of the weenie list is dropped; the figures' sets are charged.
    assert!(b.info.as_ref().unwrap().weenies.is_empty());
    let mut plain = SyntheticWorld::rectangle(1, 3..=3, 4..=4);
    plain.figures = false;
    let plain_ctx = context(plain.build());
    let p = bundle_of(&plain_ctx, key(3, 4)).unwrap();
    assert!(b.heap_bytes > p.heap_bytes);
    // Without instance sets only the weenie placement is a figure.
    assert_eq!(p.figures.len(), 1);
    assert_eq!(p.figures[0].kind, FigureKind::Weenie);
    assert!(p.figures[0].instance.is_none());
}

#[test]
fn figures_can_be_turned_off_or_capped() {
    let mut w = SyntheticWorld::rectangle(1, 3..=3, 4..=4);
    w.figures = true;
    let src = Arc::new(w.build());
    let off = WorldContext::load(
        src.clone(),
        BundleOptions {
            keep_figures: false,
            ..BundleOptions::default()
        },
    )
    .unwrap();
    let b = bundle_of(&off, key(3, 4)).unwrap();
    assert!(b.figures.is_empty());
    assert!(b.entities.iter().all(|e| e.figure.is_none()));
    let capped = WorldContext::load(
        src.clone(),
        BundleOptions {
            max_figures_per_block: 1,
            ..BundleOptions::default()
        },
    )
    .unwrap();
    let b = bundle_of(&capped, key(3, 4)).unwrap();
    assert_eq!(b.figures.len(), 1);
    assert_eq!(b.figures_dropped, 1);
    // Keeping the weenie list keeps it whole.
    let keep = WorldContext::load(
        src,
        BundleOptions {
            keep_weenies: true,
            ..BundleOptions::default()
        },
    )
    .unwrap();
    let b = bundle_of(&keep, key(3, 4)).unwrap();
    assert_eq!(b.info.as_ref().unwrap().weenies.len(), 4);
    assert_eq!(b.figures.len(), 2);
}

#[test]
fn figure_content_layers_and_suppression() {
    let k = key(0, 0);
    let mut s = SyntheticWorld::rectangle(1, 0..=0, 0..=0).build();
    let layer = |l: u32| {
        (
            testkit::pid::ENTITY_CONTENT_LAYERS,
            testkit::TestValue::Array(vec![(
                testkit::pid::ENTITY_CONTENT_LAYER,
                testkit::TestValue::Enum(l),
            )]),
        )
    };
    let named = layer(0x1000_02A7);
    let ents = vec![
        // A hidden object with an NPC instance set: not drawable, so not a figure.
        testkit::TestEntity::physics(1, obj::HIDDEN, [1.0, 1.0, 1.0], Vec::new()),
        // A weenie on an inactive (named) content layer.
        testkit::TestEntity {
            kind: 6,
            ..testkit::TestEntity::physics(2, obj::WEENIE, [2.0, 2.0, 1.0], vec![named])
        },
        // A weenie without an object.
        testkit::TestEntity {
            kind: 6,
            object: None,
            ..testkit::TestEntity::physics(3, obj::WEENIE, [3.0, 3.0, 1.0], Vec::new())
        },
    ];
    let weenies = vec![(
        testkit::entity_iid(k, 1),
        vec![(testkit::pid::NPC_STARTING_IDLE, testkit::TestValue::Enum(3))],
    )];
    s.insert(
        k.info_did(),
        testkit::landblock_info_with_weenies(k, &ents, &weenies),
        Some(39),
    );
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    assert!(b.figures.is_empty(), "{:?}", b.figures);

    // Weenies listing only `All` (2) or only `InstanceZero` (1) exist in the shared world
    // (`entity-descriptor.md` §4.5.1, I = 0), whatever the active layers.
    let ents = vec![
        testkit::TestEntity {
            kind: 6,
            ..testkit::TestEntity::physics(4, obj::WEENIE, [4.0, 4.0, 1.0], vec![layer(2)])
        },
        testkit::TestEntity {
            kind: 6,
            ..testkit::TestEntity::physics(5, obj::WEENIE, [5.0, 5.0, 1.0], vec![layer(1)])
        },
    ];
    s.insert(k.info_did(), testkit::landblock_info(k, &ents), Some(39));
    for active in [vec![1], vec![0x1000_02A7], Vec::new()] {
        let ctx = WorldContext::load(
            Arc::new(s.clone()),
            BundleOptions {
                active_content_layers: active.clone(),
                ..BundleOptions::default()
            },
        )
        .unwrap();
        let b = bundle_of(&ctx, k).unwrap();
        assert_eq!(b.figures.len(), 2, "active {active:?}: {:?}", b.figures);
        assert!(b.figures.iter().all(|f| f.kind == FigureKind::Weenie));
    }
}

#[test]
fn figures_with_one_iid_share_one_instance_set() {
    // Hostile data: 200 weenie placements with the same IID (serial 1) and one weenie
    // whose instance set is large. Every figure points to one shared copy, charged once.
    let k = key(0, 0);
    let s = SyntheticWorld::rectangle(1, 0..=0, 0..=0).build();
    let big = (
        testkit::pid::ENTITY_CONTENT_LAYERS,
        testkit::TestValue::Array(
            (0..2_000)
                .map(|_| {
                    (
                        testkit::pid::ENTITY_CONTENT_LAYER,
                        testkit::TestValue::Enum(1),
                    )
                })
                .collect(),
        ),
    );
    let weenies = vec![(testkit::entity_iid(k, 1), vec![big])];
    let make = |n: usize| {
        let ents: Vec<testkit::TestEntity> = (0..n)
            .map(|_| testkit::TestEntity {
                kind: 6,
                ..testkit::TestEntity::physics(1, obj::WEENIE, [1.0, 1.0, 1.0], Vec::new())
            })
            .collect();
        testkit::landblock_info_with_weenies(k, &ents, &weenies)
    };
    let opts = BundleOptions::default().without_world_systems();
    let bundle = |n: usize| {
        let mut s = s.clone();
        s.insert(k.info_did(), make(n), Some(39));
        let ctx = WorldContext::load(Arc::new(s), opts.clone()).unwrap();
        bundle_of(&ctx, k).unwrap()
    };
    let one = bundle(1);
    let many = bundle(200);
    assert_eq!(many.figures.len(), 200);
    let first = many.figures[0].instance.as_ref().expect("instance set");
    assert!(first.value_count() >= 1);
    for f in &many.figures {
        assert!(Arc::ptr_eq(first, f.instance.as_ref().unwrap()));
    }
    assert_eq!(one.figures.len(), 1);
    assert_eq!(one.figures[0].instance.as_deref(), Some(&**first));
}

#[test]
fn rigs_of_thirty_bones_or_more_are_figures() {
    let k = key(0, 0);
    let mut s = SyntheticWorld::rectangle(1, 0..=0, 0..=0).build();
    let schema = testkit::schema();
    // Two rigs: 30 bones (a figure) and 29 bones (scenery).
    for (entity, template, setup, bones) in [
        (0x4700_0020u32, 0x1F00_0020u32, 0x0400_0020u32, 30usize),
        (0x4700_0021, 0x1F00_0021, 0x0400_0021, 29),
    ] {
        s.insert(
            entity,
            testkit::entity_descriptor(entity, template, &[], &schema),
            None,
        );
        s.insert(template, testkit::visual_template(template, setup), None);
        s.insert(
            setup,
            testkit::setup_with_skeleton(setup, &[0x0600_0001], bones),
            None,
        );
    }
    let ents = vec![
        testkit::TestEntity::physics(1, 0x4700_0020, [1.0, 1.0, 1.0], Vec::new()),
        testkit::TestEntity::physics(2, 0x4700_0021, [2.0, 2.0, 1.0], Vec::new()),
        testkit::TestEntity::physics(3, obj::TREE, [3.0, 3.0, 1.0], Vec::new()),
    ];
    s.insert(k.info_did(), testkit::landblock_info(k, &ents), Some(39));
    let src = Arc::new(s);
    let ctx = WorldContext::load(src.clone(), BundleOptions::default()).unwrap();
    let b = bundle_of(&ctx, k).unwrap();
    let bones: Vec<Option<u32>> = b
        .entities
        .iter()
        .map(|e| e.visual.as_ref().and_then(|v| v.bones))
        .collect();
    assert_eq!(bones, vec![Some(30), Some(29), None]);
    assert_eq!(b.figures.len(), 1, "{:?}", b.figures);
    assert_eq!(b.figures[0].kind, FigureKind::Rig);
    assert_eq!(b.figures[0].entity, 0);
    assert!(b.figures[0].instance.is_none());
    assert_eq!(b.scenery().count(), 2);
    // Turned off.
    let off = WorldContext::load(
        src,
        BundleOptions {
            min_rig_bones: 0,
            ..BundleOptions::default()
        },
    )
    .unwrap();
    assert!(bundle_of(&off, k).unwrap().figures.is_empty());
}

// ---------------------------------------------------------------------------------------
// World systems (M4.5): scenery, water, collision and frills through bundles and streaming
// ---------------------------------------------------------------------------------------

/// Stored water level of the synthetic scenery world: h = 12 × 1000 / 255 ≈ 47.06, above
/// every terrain height of blocks X = 10 – 13 (≈ 39 – 55 is not reached: X = 12 starts at
/// stored 1,536 ≈ 46.9), so even-X blocks are wet.
const LEVEL: u16 = 12;

fn scenery_world() -> SyntheticWorld {
    SyntheticWorld {
        scenery: true,
        water_level: Some(LEVEL),
        ..SyntheticWorld::rectangle(1, 10..=13, 20..=21)
    }
}

fn scenery_context(options: BundleOptions) -> Arc<WorldContext> {
    Arc::new(
        WorldContext::load(Arc::new(scenery_world().build()), options)
            .expect("synthetic context")
            .with_area_names(names()),
    )
}

fn objects_of(b: &LandblockBundle) -> Vec<u32> {
    b.generated
        .iter()
        .filter_map(|e| e.object.map(|d| d.0))
        .collect()
}

#[test]
fn context_loads_region_tables_and_systems() {
    let ctx = scenery_context(BundleOptions::default());
    let r = ctx.region(1).expect("region");
    let t = r
        .scenery
        .as_ref()
        .expect("scenery tables from slots 1, 2, 8");
    assert_eq!(t.scene_choices.did, Did(0x1100_0000));
    assert!(t.terrain_types.is_some());
    let sys = ctx.systems();
    assert!(sys.scenes.is_some());
    let water = sys.water.as_ref().expect("water");
    assert!(water.tables().water_table(1).is_some());
    assert!(sys.collision.is_some());
    // No sky description or LOD-class table in the synthetic install.
    assert!(sys.sky.is_none() && sys.lod_classes.is_none());
    assert_eq!(sys.pids.lod_class, Some(testkit::pid::RENDER_LOD_CLASS));
    // Turned off: nothing loads.
    let off = scenery_context(BundleOptions::default().without_world_systems());
    assert!(off.region(1).unwrap().scenery.is_none());
    assert!(off.systems().scenes.is_none() && off.systems().water.is_none());
    assert!(off.systems().collision.is_none());
}

#[test]
fn bundles_carry_scenery_water_and_collision() {
    let ctx = scenery_context(BundleOptions::default());
    let wet = bundle_of(&ctx, key(10, 20)).unwrap();
    let dry = bundle_of(&ctx, key(11, 20)).unwrap();
    assert!(wet.problems.is_empty(), "{:?}", wet.problems);

    // Water: the even block is wet at h = 12 × 1000 / 255.
    let h = f32::from(LEVEL) * 1000.0 / 255.0;
    let w = wet.water.as_ref().expect("water").water().expect("wet");
    assert!((w.height - h).abs() < 1e-4, "{}", w.height);
    // The exact f32 height of `water.md` §6.2 from here on.
    let h = w.height;
    assert!(dry.water.as_ref().expect("water").water().is_none());

    // Scenery: SCENERY on both, ON_WATER (height_mode 1) only where there is water, at h:
    // the water module's plane is wired into scenery's water ranges (scenery.md §11).
    assert_eq!(objects_of(&wet), vec![obj::SCENERY, obj::ON_WATER]);
    assert_eq!(objects_of(&dry), vec![obj::SCENERY]);
    let on_water = &wet.generated[1];
    assert_eq!(on_water.placement.position[2], h);
    let sc = wet.scenery.as_ref().expect("scenery");
    assert!(sc.complete);
    assert_eq!(sc.water.map(|p| p.d), Some(-h));
    assert_eq!(sc.stats.objects, 2);
    assert!(dry.scenery.as_ref().unwrap().water.is_none());

    // Generated objects are placed like statics: resolved, drawn, LOD class from the chain.
    for g in &wet.generated {
        assert_eq!(g.kind, Some(StaticEntityType::Physics));
        assert!(g.draw && g.visual.is_some() && g.iid.is_none());
        assert!(wet.is_generated(g));
    }
    assert_eq!(
        wet.drawable().count(),
        wet.entities.iter().filter(|e| e.draw).count() + 2
    );
    assert_eq!(wet.lod_class_of(&wet.generated[0]), Some(5));
    assert_eq!(wet.lod_class_of(&wet.generated[1]), None);
    assert_eq!(wet.entity_lod_classes.len(), wet.entities.len());

    // Collision: terrain, the SCENERY box as a generated body, nothing for ON_WATER.
    let col = wet.collision.as_ref().expect("collision");
    assert!(col.set.terrain().is_some());
    assert!(col.generated_bodies[0].is_some());
    assert!(col.generated_bodies[1].is_none());
    assert_eq!(col.report().generated_blocking, 1);
    assert_eq!(col.set.water_plane().map(|p| p.level), Some(h));
    assert!(wet.heap_bytes > col.heap_bytes);

    // The same bundle without world systems.
    let off = scenery_context(BundleOptions::default().without_world_systems());
    let plain = bundle_of(&off, key(10, 20)).unwrap();
    assert!(plain.generated.is_empty() && plain.scenery.is_none());
    assert!(plain.water.is_none() && plain.collision.is_none());
    assert!(plain.entity_lod_classes.is_empty());
}

#[test]
fn world_systems_are_charged_to_residency() {
    let on = scenery_context(BundleOptions::default());
    let off = scenery_context(BundleOptions::default().without_world_systems());
    let a = bundle_of(&on, key(10, 20)).unwrap();
    let b = bundle_of(&off, key(10, 20)).unwrap();
    let col = a.collision.as_ref().unwrap().heap_bytes;
    let gen_bytes = a.generated.capacity() * std::mem::size_of::<PlacedEntity>();
    assert!(
        a.heap_bytes >= b.heap_bytes + col + gen_bytes,
        "{} vs {} + {col} + {gen_bytes}",
        a.heap_bytes,
        b.heap_bytes
    );
}

/// Builds every block of `keys` through the bundle store on `workers` workers, requesting
/// them in the given order, and returns the generated lists by key (bit patterns).
fn generated_through_store(
    ctx: &Arc<WorldContext>,
    workers: usize,
    keys: &[LandblockKey],
) -> std::collections::BTreeMap<LandblockKey, Vec<[u32; 11]>> {
    let pool = WorkerPool::new(PoolConfig {
        workers,
        ..PoolConfig::default()
    });
    let store = tessera_assets::AssetStore::new(
        &pool,
        BundleSource::new(Arc::clone(ctx.source())),
        BundleDecoder::new(Arc::clone(ctx)),
        tessera_assets::StoreConfig::default(),
    );
    let handles: Vec<_> = keys
        .iter()
        .map(|k| {
            (
                *k,
                store.request(k.data_did().0, tessera_assets::Priority::Visible),
            )
        })
        .collect();
    handles
        .into_iter()
        .map(|(k, h)| {
            let tessera_assets::AssetState::Ready(b) = h.wait(Duration::from_secs(30)) else {
                panic!("{k} did not load");
            };
            let list = b
                .generated
                .iter()
                .map(|e| {
                    let p = e.placement.position;
                    let q = e.placement.rotation.to_wxyz();
                    [
                        e.object.map_or(0, |d| d.0),
                        p[0].to_bits(),
                        p[1].to_bits(),
                        p[2].to_bits(),
                        q[0].to_bits(),
                        q[1].to_bits(),
                        q[2].to_bits(),
                        q[3].to_bits(),
                        e.scale[0].to_bits(),
                        e.scale[1].to_bits(),
                        e.scale[2].to_bits(),
                    ]
                })
                .collect();
            (k, list)
        })
        .collect()
}

#[test]
fn scenery_is_identical_with_any_worker_count_and_order() {
    let ctx = scenery_context(BundleOptions::default());
    let mut keys: Vec<LandblockKey> = (10..=13u8)
        .flat_map(|x| (20..=21u8).map(move |y| key(x, y)))
        .collect();
    let one = generated_through_store(&ctx, 1, &keys);
    keys.reverse();
    let many = generated_through_store(&ctx, 4, &keys);
    assert_eq!(one, many);
    assert!(one.values().all(|l| !l.is_empty()));
}

#[test]
fn streamer_serves_world_system_lookups_and_frills() {
    let ctx = scenery_context(BundleOptions::default());
    let pool = pool();
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 160.0,
                mid: 320.0,
                far: 480.0,
                hysteresis: 80.0,
            },
            frills: crate::frills::FrillSettings {
                distance_level: 1,
                ..crate::frills::FrillSettings::default()
            },
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let here = centre(10, 20);
    assert!(w.settle(here, Duration::from_secs(30)));
    let r = w.update(here);
    assert!(r.collision_bytes > 0 && r.generated_objects > 0, "{r:?}");
    assert_eq!(r.incomplete_scenery, 0);

    // Water of the resident wet block, and the dry neighbour.
    let h = f64::from(f32::from(LEVEL) * 1000.0 / 255.0);
    let under = WorldPos::new(1, here.x, here.y, h - 1.0);
    assert!(w.is_under_water(&under));
    assert!(w.landblock_water(&under).is_some());
    let dry = LocalPos::new(key(11, 20), [80.0, 80.0, 30.0]).to_world();
    assert!(!w.is_under_water(&dry));
    assert!(w.landblock_water(&dry).is_none());

    // Sky and lights: no sky description; no Light entities.
    assert!(w.sky().is_none() && w.sky_at(&here, 0, 0.0).is_none());
    assert!(w.lights_near(&here, 500.0).is_empty());

    // Draw distances: class 5 without a table keeps the component's initial values.
    let b = Arc::clone(w.bundle(key(10, 20)).unwrap());
    let scenery = &b.generated[0];
    assert_eq!(w.lod_values(&b, scenery, false), LodValues::default());
    let d = w.draw_distance(&b, scenery, 1_000.0, &DistanceSettings::default());
    assert!(d.is_finite() && d > 0.0);

    // Collision: a view over the resident sets; the generated box blocks a sweep.
    let view = w.collision_view(&here, 1);
    assert!(view.members().len() >= 4);
    let col = w.collision(key(10, 20)).unwrap();
    assert!(col.generated_bodies[0].is_some());
    assert!(
        w.cell_at(&here).is_none(),
        "no cells in the synthetic world"
    );

    // Frills stream with the camera: the scenery vertex's sector holds the FRILL object.
    let sector = crate::frills::FrillSectorKey::new(key(10, 20), 8, 8).unwrap();
    let mut ready = None;
    for _ in 0..2_000 {
        let fr = w.update_frills(&here, &here);
        if let Some(s) = w.frill_sector(sector) {
            ready = Some((Arc::clone(s), fr));
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let (s, _) = ready.expect("frill sector generated");
    assert!(
        s.instances.iter().any(|i| i.object == Did(obj::FRILL)),
        "{s:?}"
    );
    assert!(s.instances.iter().all(|i| i.object == Did(obj::FRILL)));
    // Moving far away drops every sector: the first far update reports them leaving.
    let far = centre(13, 21);
    let mut fr = w.update_frills(&far, &far);
    assert!(fr.left > 0, "{fr:?}");
    for _ in 0..2_000 {
        if fr.waiting == 0 && fr.running == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
        fr = w.update_frills(&far, &far);
    }
    assert!(w.frill_sector(sector).is_none());
    // Turning frills off at run time drops every sector and keeps the landblocks.
    let resident = w.resident().count();
    w.set_frill_settings(crate::frills::FrillSettings {
        draw_frills: false,
        ..crate::frills::FrillSettings::default()
    });
    assert_eq!(w.frill_bytes(), 0);
    assert!(w.frill_ring().is_empty());
    let fr = w.update_frills(&here, &here);
    assert_eq!((fr.ready, fr.running, fr.waiting), (0, 0, 0), "{fr:?}");
    assert_eq!(w.resident().count(), resident);
    // Clearing releases water and frills.
    w.clear();
    assert!(w.water_map().is_empty());
    assert_eq!(w.frill_bytes(), 0);
}

#[test]
fn frill_sector_of_a_bundle_without_scenery_is_empty() {
    let ctx = scenery_context(BundleOptions::default().without_world_systems());
    let b = bundle_of(&ctx, key(10, 20)).unwrap();
    let mut thinner = crate::frills::DensityThinner::with_seed(1);
    let s = build_frill_sector(&ctx, &b, 8, 8, 1.0, &mut thinner);
    assert!(s.instances.is_empty());
    assert_eq!(s.key, crate::frills::FrillSectorKey::new(key(10, 20), 8, 8));
    // With scenery: the same call fills it, deterministically at density 1.
    let ctx = scenery_context(BundleOptions::default());
    let b = bundle_of(&ctx, key(10, 20)).unwrap();
    let a = build_frill_sector(&ctx, &b, 8, 8, 1.0, &mut thinner);
    let c = build_frill_sector(
        &ctx,
        &b,
        8,
        8,
        1.0,
        &mut crate::frills::DensityThinner::new(),
    );
    assert!(!a.instances.is_empty());
    assert_eq!(a.instances, c.instances);
}

// ---------------------------------------------------------------------------------------
// Review fixes of the M4.5 world systems
// ---------------------------------------------------------------------------------------

/// Small rings for streaming tests over the scenery worlds.
fn small_rings() -> StreamConfig {
    StreamConfig {
        rings: RingConfig {
            near: 160.0,
            mid: 320.0,
            far: 480.0,
            hysteresis: 80.0,
        },
        ..StreamConfig::default()
    }
}

#[test]
fn block_props_read_palette_values_from_the_shared_descriptor() {
    let k = key(127, 5);
    let mut s = SyntheticWorld::rectangle(1, 127..=128, 5..=5).build();
    let ctx = context(s.clone());
    let b = bundle_of(&ctx, k).unwrap();
    let p = &b.block_props;
    let desc = ctx
        .region(1)
        .and_then(|r| r.block_map_descriptor.as_ref())
        .expect("descriptor");
    assert!(Arc::ptr_eq(p.descriptor.as_ref().unwrap(), desc), "shared");
    let area = p
        .values
        .iter()
        .find(|v| v.key == testkit::key::AREA)
        .unwrap();
    let entry = &desc.entries[area.entry.unwrap() as usize];
    // Not a copy: the value is the descriptor's own.
    assert!(std::ptr::eq(
        p.value_of(area).unwrap(),
        entry.value(area.index).unwrap()
    ));
    assert!(std::ptr::eq(
        p.value(testkit::pid::AREA_DID).unwrap(),
        entry.value(area.index).unwrap()
    ));
    let undescribed = p
        .values
        .iter()
        .find(|v| v.key == testkit::key::UNDESCRIBED)
        .unwrap();
    assert_eq!((undescribed.pid, undescribed.entry), (None, None));
    assert!(p.value_of(undescribed).is_none());

    // Hostile: 20,000 pairs repeating one key. The pairs cost their own size, which is
    // what the bundle charges; the first pair wins.
    let pairs: Vec<(u32, u16)> = (0..20_000u32)
        .map(|n| (testkit::key::AREA, u16::from(n % 2 == 1)))
        .collect();
    s.insert(k.block_map_did(), testkit::block_map(k, &pairs), None);
    let ctx = context(s);
    let b = bundle_of(&ctx, k).unwrap();
    let p = &b.block_props;
    assert_eq!(p.values.len(), 20_000);
    assert_eq!(
        p.heap_bytes(),
        p.values.capacity() * std::mem::size_of::<BlockValue>()
    );
    assert!(p.area.is_some());
}

#[test]
fn scenery_does_not_depend_on_build_water() {
    let on = scenery_context(BundleOptions::default());
    let off = scenery_context(BundleOptions {
        build_water: false,
        ..BundleOptions::default()
    });
    assert!(off.systems().water.is_some(), "the tables load for scenery");
    for k in [key(10, 20), key(11, 20), key(12, 21)] {
        let a = bundle_of(&on, k).unwrap();
        let b = bundle_of(&off, k).unwrap();
        assert_eq!(objects_of(&a), objects_of(&b), "{k}");
        assert_eq!(
            a.scenery.as_ref().unwrap().water.map(|p| p.d),
            b.scenery.as_ref().unwrap().water.map(|p| p.d)
        );
        // A collision set carries the water whenever it is built (ARCHITECTURE §10.3), so
        // with collision the water is built without `build_water` too, the same as with it.
        assert_eq!(
            a.water.as_ref().map(|w| w.water().map(|l| l.height)),
            b.water.as_ref().map(|w| w.water().map(|l| l.height)),
            "{k}"
        );
        assert_eq!(
            a.collision
                .as_ref()
                .unwrap()
                .set
                .water_plane()
                .map(|p| p.level),
            b.collision
                .as_ref()
                .unwrap()
                .set
                .water_plane()
                .map(|p| p.level),
            "{k}"
        );
    }
    let wet = bundle_of(&off, key(10, 20)).unwrap();
    assert_eq!(objects_of(&wet), vec![obj::SCENERY, obj::ON_WATER]);
    assert!(wet.collision.as_ref().unwrap().set.water_plane().is_some());
    // Without `build_water` and without collision there is no landblock water.
    let neither = scenery_context(BundleOptions {
        build_water: false,
        build_collision: false,
        ..BundleOptions::default()
    });
    let dry = bundle_of(&neither, key(10, 20)).unwrap();
    assert!(dry.water.is_none() && dry.collision.is_none());
    assert_eq!(objects_of(&dry), vec![obj::SCENERY, obj::ON_WATER]);
    // Neither option: no water tables.
    let none = scenery_context(BundleOptions {
        build_water: false,
        generate_scenery: false,
        ..BundleOptions::default()
    });
    assert!(none.systems().water.is_none());
}

#[test]
fn unreadable_optional_records_are_problems_not_incomplete_parts() {
    // An info or block-map record the frame reports as unreadable is the data's condition
    // (`landblock.md` §8.1: reported as a problem, the bundle delivered): every part is
    // complete, scenery and collision alike, so the streamer does not rebuild the bundle
    // on every load.
    let k = key(10, 20);
    for bad in [k.info_did(), k.block_map_did()] {
        let src = FailingSource {
            inner: scenery_world().build(),
            bad,
        };
        let ctx = WorldContext::load(Arc::new(src), BundleOptions::default()).unwrap();
        let b = bundle_of(&ctx, k).unwrap();
        assert!(b.scenery.as_ref().expect("scenery").complete, "{bad}");
        assert!(b.collision.as_ref().expect("collision").complete, "{bad}");
        assert!(b.is_complete(), "{bad}");
        assert_eq!(b.problems.len(), 1, "{:?}", b.problems);
        let p = &b.problems[0];
        assert_eq!(p.did, bad);
        assert!(
            p.unreadable && !p.transient && p.category.is_none(),
            "{p:?}"
        );
    }
    let ctx = scenery_context(BundleOptions::default());
    assert!(bundle_of(&ctx, k).unwrap().is_complete());
}

#[test]
fn terrain_mesh_is_charged_without_a_collision_set() {
    let with = scenery_context(BundleOptions {
        build_collision: false,
        ..BundleOptions::default()
    });
    let without = scenery_context(BundleOptions::default().without_world_systems());
    let a = bundle_of(&with, key(10, 20)).unwrap();
    let b = bundle_of(&without, key(10, 20)).unwrap();
    assert!(a.collision.is_none());
    let sc = a.scenery.as_ref().expect("scenery");
    let generated = a.generated.capacity() * std::mem::size_of::<PlacedEntity>();
    assert!(
        a.heap_bytes >= b.heap_bytes + sc.heap_bytes() + crate::bundle::GRID_MESH_BYTES + generated,
        "{} vs {} + {} + {} + {generated}",
        a.heap_bytes,
        b.heap_bytes,
        sc.heap_bytes(),
        crate::bundle::GRID_MESH_BYTES
    );
}

#[test]
fn entity_refs_are_stable_across_clones() {
    let ctx = scenery_context(BundleOptions::default());
    let b = bundle_of(&ctx, key(10, 20)).unwrap();
    let c = b.clone();
    // By address a clone's element is not found; by reference it is.
    assert_eq!(b.lod_class_of(&c.generated[0]), None);
    assert_eq!(c.lod_class(EntityRef::Generated(0)), Some(5));
    assert_eq!(b.lod_class(EntityRef::Generated(1)), None);
    assert_eq!(b.entity_ref(&b.generated[1]), Some(EntityRef::Generated(1)));
    assert_eq!(b.entity_ref(&b.entities[2]), Some(EntityRef::Entity(2)));
    assert!(b.is_generated(&b.generated[0]) && !b.is_generated(&b.entities[0]));
    assert!(b.lod_class(EntityRef::Entity(u32::MAX)).is_none());
    let refs: Vec<EntityRef> = b.entity_refs().map(|(r, _)| r).collect();
    assert_eq!(refs.len(), b.entities.len() + b.generated.len());
    for (r, e) in b.entity_refs() {
        assert!(std::ptr::eq(b.entity(r).unwrap(), e));
        assert_eq!(b.lod_class(r), b.lod_class_of(e));
    }
    // Index 0 names an entity and a generated object; the references differ.
    assert_eq!(b.entities[0].index, b.generated[0].index);
    assert_ne!(EntityRef::Entity(0), EntityRef::Generated(0));
}

#[test]
fn world_inputs_that_fail_transiently_do_not_fail_the_job() {
    // Object records (0x47) refused by a full queue while the world systems read them:
    // only the landblock data fails a job (`landblock.md` §8.1), so the bundle is
    // delivered, marked incomplete with transient problems, and rebuilt on the next load.
    use crate::collision::tests::FlakySource;
    use std::sync::atomic::AtomicBool;
    let src = Arc::new(FlakySource {
        inner: scenery_world().build(),
        prefix: 0x47,
        flaky: AtomicBool::new(false),
    });
    let ctx = Arc::new(WorldContext::load(src.clone(), BundleOptions::default()).unwrap());
    src.flaky.store(true, Ordering::SeqCst);
    let b = bundle_of(&ctx, key(10, 20)).expect("delivered despite the failed reads");
    assert!(!b.is_complete());
    assert!(
        b.problems.iter().any(|p| p.transient && p.unreadable),
        "{:?}",
        b.problems
    );
    assert!(
        b.problems
            .iter()
            .filter(|p| p.transient)
            .all(|p| p.category.is_none())
    );
    assert!(
        b.problems
            .iter()
            .any(|p| p.to_string().contains("[transient]"))
    );

    let pool = pool();
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, small_rings()).unwrap();
    let here = centre(10, 20);
    assert!(w.settle(here, Duration::from_secs(30)));
    // Only the landblocks outside the synthetic world fail (no data record).
    for x in 10..=13u8 {
        for y in 20..=21u8 {
            assert!(
                !matches!(w.state_of(key(x, y)), Some(BlockState::Failed(_))),
                "({x}, {y})"
            );
        }
    }
    let b = w.bundle(key(10, 20)).expect("resident, incomplete");
    assert!(!b.is_complete());
    // Readable again: leave and come back, and the landblock is built again, complete.
    src.flaky.store(false, Ordering::SeqCst);
    assert!(w.settle(centre(10, 120), Duration::from_secs(30)));
    assert!(w.bundle(key(10, 20)).is_none());
    assert!(w.settle(here, Duration::from_secs(30)));
    let b = w.bundle(key(10, 20)).expect("resident again");
    assert!(b.is_complete());
    assert_eq!(objects_of(b), vec![obj::SCENERY, obj::ON_WATER]);
}

#[test]
fn requests_refused_by_a_full_queue_are_retried() {
    // A pool that queues one job: most requests are refused (a full queue is the
    // pipeline's state, not the data's) and retried until every landblock is in.
    let ctx = context(SyntheticWorld::rectangle(1, 0..=12, 0..=12).build());
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        max_queued: 6,
        ..PoolConfig::default()
    });
    let mut w = WorldStreamer::new(ctx, &pool, small_rings()).unwrap();
    let here = centre(6, 6);
    // Whenever an update changes nothing and nothing waits, the near ring is complete:
    // no admitted landblock is left without a request (`walker.md` §7).
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let r = w.update_with_velocity(here, [0.0; 2]);
        let quiet = r.loading == 0
            && r.pending == 0
            && r.waiting_retry == 0
            && r.requested == 0
            && r.integrated == 0
            && r.evicted == 0;
        if quiet {
            assert_eq!((r.near_missing, r.deferred), (0, 0), "{r:?}");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "never settled: {r:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(w.settle(here, Duration::from_secs(60)));
    assert!(w.stats().retried > 0, "{:?}", w.stats());
    assert_eq!(w.stats().failed, 0, "{:?}", w.stats());
    let r = w.update_with_velocity(here, [0.0; 2]);
    assert_eq!((r.pending, r.deferred, r.near_missing), (0, 0, 0), "{r:?}");
    assert_eq!(r.resident_blocks, w.rings().members().len());
}

#[test]
fn incomplete_scenery_is_rebuilt_when_the_landblock_loads_again() {
    use crate::collision::tests::FlakySource;
    use std::sync::atomic::AtomicBool;
    let world = SyntheticWorld {
        scenery: true,
        ..SyntheticWorld::rectangle(1, 0..=30, 20..=21)
    };
    let src = Arc::new(FlakySource {
        inner: world.build(),
        prefix: 0x02,
        flaky: AtomicBool::new(true),
    });
    let ctx = Arc::new(WorldContext::load(src.clone(), BundleOptions::default()).unwrap());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, small_rings()).unwrap();
    let here = centre(5, 20);
    assert!(w.settle(here, Duration::from_secs(30)));
    let r = w.update(here);
    assert!(r.incomplete_scenery > 0, "{r:?}");
    assert!(!w.bundle(key(5, 20)).unwrap().is_complete());
    // Scene records readable again; leave (the bundles stay in the store's cache budget)
    // and come back.
    src.flaky.store(false, Ordering::SeqCst);
    assert!(w.settle(centre(28, 20), Duration::from_secs(30)));
    assert!(w.bundle(key(5, 20)).is_none());
    assert!(w.settle(here, Duration::from_secs(30)));
    let r = w.update(here);
    assert_eq!(r.incomplete_scenery, 0, "{r:?}");
    let b = w.bundle(key(5, 20)).unwrap();
    assert!(b.is_complete());
    assert!(!b.generated.is_empty());
}

#[test]
fn cell_at_finds_cells_owned_by_distant_landblocks() {
    use crate::collision::tests::{cell, info, room, system};
    use tessera_formats::landblock::HeightGrid;
    let ctx = context(SyntheticWorld::rectangle(1, 9..=13, 19..=21).build());
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, StreamConfig::default()).unwrap();
    let sys = system();
    sys.seed_cell_geometry(room(0x0100_0001));
    let src = MemoryWorldSource::new();
    let schema = testkit::schema();
    let resident = |k: LandblockKey, cells| {
        let data = tessera_formats::LandblockData::from_heights(k.data_did(), HeightGrid::flat(0));
        let info: Option<tessera_formats::LandblockInfo> = Some(info(cells));
        let col = sys.build(&CollisionInput {
            key: k,
            data: &data,
            terrain: None,
            info: info.as_ref(),
            generated: &[],
            water: None,
            source: &src,
            schema: &schema,
        });
        let mut b = LandblockBundle::from_data(k, data);
        b.heap_bytes += col.heap_bytes;
        b.collision = Some(Arc::new(col));
        b
    };
    // (10, 20) owns room 1 around block-local x = 92; (12, 20) owns room 3 around
    // block-local x = -230, which lies in (10, 20), two landblocks west of its owner.
    w.insert_resident(resident(
        key(10, 20),
        vec![cell(1, 0x0100_0001, Some([92.0, 80.0, 5.0]), None)],
    ));
    w.insert_resident(resident(key(11, 20), Vec::new()));
    w.insert_resident(resident(
        key(12, 20),
        vec![cell(3, 0x0100_0001, Some([-230.0, 80.0, 5.0]), None)],
    ));
    let at = |x: f64, z: f64| WorldPos::new(1, x, 20.0 * 160.0 + 80.0, z);
    let found = |p: WorldPos| w.cell_at(&p).map(|(k, c)| (k, c.index));
    let p1 = at(1_687.0, 6.0);
    assert_eq!(p1.landblock(), Some(key(10, 20)));
    assert_eq!(
        found(p1),
        Some((key(12, 20), 3)),
        "the distant owner's cell"
    );
    // In both rooms: the point's own landblock is asked first.
    assert_eq!(found(at(1_690.0, 6.0)), Some((key(10, 20), 1)));
    assert_eq!(found(at(1_695.0, 6.0)), Some((key(10, 20), 1)));
    assert_eq!(found(at(1_690.0, 20.0)), None, "above both rooms");
    assert_eq!(found(at(f64::NAN, 6.0)), None);
    // The collision view brings the distant owner's set (radius 0: own landblock + owner;
    // radius 1: (11, 20) too).
    assert_eq!(w.collision_view(&p1, 0).members().len(), 2);
    assert_eq!(w.collision_view(&p1, 1).members().len(), 3);
    // Releasing forgets the cells.
    w.clear();
    assert!(w.cell_at(&p1).is_none());
}

#[test]
fn lights_near_filters_by_radius_and_orders_across_landblocks() {
    let mut s = SyntheticWorld::rectangle(1, 9..=12, 19..=21).build();
    let light = |serial: u32, p: [f32; 3]| testkit::TestEntity {
        kind: 9,
        object: None,
        ..testkit::TestEntity::physics(serial, 0, p, Vec::new())
    };
    let (a, b) = (key(10, 20), key(11, 20));
    s.insert(
        a.info_did(),
        testkit::landblock_info(
            a,
            &[
                light(1, [151.0, 80.0, 10.0]),
                light(2, [158.0, 73.0, 10.0]),
                light(3, [10.0, 80.0, 10.0]),
            ],
        ),
        Some(39),
    );
    s.insert(
        b.info_did(),
        testkit::landblock_info(
            b,
            &[light(1, [4.0, 80.0, 10.0]), light(2, [100.0, 80.0, 10.0])],
        ),
        Some(39),
    );
    let ctx = context(s);
    let pool = pool();
    let mut w = WorldStreamer::new(ctx, &pool, small_rings()).unwrap();
    let here = LocalPos::new(a, [158.0, 80.0, 10.0]).to_world();
    assert!(w.settle(here, Duration::from_secs(20)));
    let got = |r: f64| {
        w.lights_near(&here, r)
            .iter()
            .map(|l| (l.key, l.light.index))
            .collect::<Vec<_>>()
    };
    assert!(got(5.0).is_empty());
    // Distances 6 (across the boundary), 7 and 7 (a tie: by landblock, then index).
    assert_eq!(got(7.0), vec![(b, 0), (a, 0), (a, 1)]);
    assert_eq!(got(110.0), vec![(b, 0), (a, 0), (a, 1), (b, 1)]);
    assert_eq!(got(160.0), vec![(b, 0), (a, 0), (a, 1), (b, 1), (a, 2)]);
    let nearest = w.lights_near(&here, 7.0)[0];
    assert_eq!(
        (nearest.position.x, nearest.position.y, nearest.position.z),
        (11.0 * 160.0 + 4.0, 20.0 * 160.0 + 80.0, 10.0)
    );
    assert!(w.lights_near(&here, -1.0).is_empty());
}

#[test]
fn lights_near_finds_lights_placed_far_from_their_owner() {
    // Interior lights sit in cells that may lie landblocks away from their owner
    // (`walker.md` §13.3 row W-C1): (20, 20) places a light 900 units west of its square,
    // in (14, 20), six landblocks away. `lights_near` finds it from there.
    let mut s = SyntheticWorld::rectangle(1, 10..=22, 18..=22).build();
    let owner = key(20, 20);
    let light = testkit::TestEntity {
        kind: 9,
        object: None,
        interior: true,
        ..testkit::TestEntity::physics(1, 0, [-900.0, 80.0, 10.0], Vec::new())
    };
    s.insert(
        owner.info_did(),
        testkit::landblock_info(owner, &[light]),
        Some(39),
    );
    let ctx = context(s);
    let pool = pool();
    let mut w = WorldStreamer::new(
        ctx,
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 160.0,
                mid: 960.0,
                far: 1_120.0,
                hysteresis: 80.0,
            },
            ..StreamConfig::default()
        },
    )
    .unwrap();
    let here = LocalPos::new(key(14, 20), [80.0, 80.0, 10.0]).to_world();
    assert!(w.settle(here, Duration::from_secs(20)));
    assert!(
        w.bundle(owner).is_some(),
        "the owner is resident (mid ring)"
    );
    let got = w.lights_near(&here, 100.0);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].key, got[0].light.index), (owner, 0));
    assert!((got[0].position.x - (20.0 * 160.0 - 900.0)).abs() < 1e-9);
    // Out of reach of the radius, and in another region, nothing.
    assert!(w.lights_near(&here, 19.0).is_empty(), "20 units away");
    assert!(
        w.lights_near(&WorldPos::new(2, here.x, here.y, here.z), 100.0)
            .is_empty()
    );
    // Releasing the owner forgets its lights.
    w.clear();
    assert!(w.lights_near(&here, 100.0).is_empty());
}

#[test]
fn evicting_or_releasing_a_wet_landblock_removes_its_water() {
    let world = SyntheticWorld {
        scenery: true,
        water_level: Some(LEVEL),
        ..SyntheticWorld::rectangle(1, 10..=22, 20..=21)
    };
    let ctx =
        Arc::new(WorldContext::load(Arc::new(world.build()), BundleOptions::default()).unwrap());
    let keys: Vec<LandblockKey> = (10..=22u8)
        .flat_map(|x| (20..=21u8).map(move |y| key(x, y)))
        .collect();
    let consistent = |w: &WorldStreamer| {
        for &k in &keys {
            assert_eq!(
                w.water_map().get(k).is_some(),
                w.bundle(k).is_some(),
                "{k}: water exactly for resident landblocks"
            );
        }
    };
    let pool = pool();
    // Release: leave the rings.
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, small_rings()).unwrap();
    assert!(w.settle(centre(10, 20), Duration::from_secs(30)));
    assert!(w.water_map().get(key(10, 20)).is_some());
    consistent(&w);
    let before = w.water_map().len();
    assert!(w.settle(centre(21, 20), Duration::from_secs(30)));
    assert!(w.water_map().get(key(10, 20)).is_none());
    assert!(w.stats().released > 0);
    consistent(&w);
    assert!(before > 0);

    // Eviction: a budget of three bundles with an estimate that admits everything first.
    let h = bundle_of(&ctx, key(10, 20)).unwrap().heap_bytes;
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            residency_budget: 3 * h,
            always_admit_near: false,
            initial_size_estimate: 1,
            ..small_rings()
        },
    )
    .unwrap();
    assert!(w.settle(centre(10, 20), Duration::from_secs(30)));
    assert!(w.stats().evicted > 0, "{:?}", w.stats());
    consistent(&w);
    assert!(w.water_map().len() <= 3);
}

#[test]
fn residency_charge_is_the_terms_of_the_spec() {
    // `landblock.md` §8.1, "Residency charge": the data's grids and surface lists plus four
    // times the stored bytes of the other records. Placed entities, figures, their instance
    // sets and block-map pairs are decoded from those records and not charged again.
    let k = key(3, 4);
    let mut world = SyntheticWorld::rectangle(1, 3..=3, 4..=4);
    world.figures = true;
    let src = world.build();
    let ctx = Arc::new(
        WorldContext::load(
            Arc::new(src.clone()),
            BundleOptions::default().without_world_systems(),
        )
        .expect("synthetic context"),
    );
    let b = bundle_of(&ctx, k).unwrap();
    assert!(!b.entities.is_empty() && !b.figures.is_empty());
    assert!(b.figures.iter().any(|f| f.instance.is_some()));
    assert!(!b.block_props.values.is_empty());
    let stored = |d: Did| {
        src.read_stamped(d)
            .unwrap()
            .map_or(0, |(bytes, _)| bytes.len())
    };
    let others = stored(k.info_did()) + stored(k.block_map_did()) + stored(k.properties_did());
    assert!(others > 0);
    let data_only = LandblockBundle::from_data(k, b.data.clone()).heap_bytes;
    assert_eq!(b.heap_bytes, data_only + 4 * others);
    // The world parts come on top (`world_systems_are_charged_to_residency`).
    let on = context(src);
    assert!(bundle_of(&on, k).unwrap().heap_bytes > b.heap_bytes);
}

/// A source whose reads of one record prefix fail as the archive's (data) errors.
struct PrefixDataErrorSource {
    inner: MemoryWorldSource,
    prefix: u8,
}

impl WorldSource for PrefixDataErrorSource {
    fn read_stamped(&self, did: Did) -> std::result::Result<Option<StampedRecord>, AssetError> {
        if did.prefix() == self.prefix {
            return Err(AssetError::source_error(
                did.0,
                std::io::Error::other("bad sector"),
            ));
        }
        self.inner.read_stamped(did)
    }
}

#[test]
fn data_failures_of_world_inputs_leave_the_parts_complete() {
    // Object records (0x47) the archive cannot read: a data failure, absorbed where the
    // input is read (`landblock.md` §8.1, OPEN on data errors in world-system inputs). The
    // parts are built without them, complete, and the bundle is cached like any other.
    let src = PrefixDataErrorSource {
        inner: scenery_world().build(),
        prefix: 0x47,
    };
    let ctx = WorldContext::load(Arc::new(src), BundleOptions::default()).unwrap();
    let b = bundle_of(&ctx, key(10, 20)).expect("delivered");
    assert!(b.is_complete(), "{:?}", b.problems);
    assert!(b.problems.iter().all(|p| !p.transient), "{:?}", b.problems);
    assert!(b.scenery.as_ref().expect("scenery built").complete);
    let water = b.water.as_ref().expect("water built");
    assert!(water.water().is_some(), "the plane needs no object record");
    let col = b.collision.as_ref().expect("collision built");
    assert!(col.complete);
    assert!(col.set.water_plane().is_some(), "the set is not dry");
}

#[test]
fn water_lookups_answer_from_the_collision_water_without_build_water() {
    // `build_water` off, scenery and collision on: the set carries the water
    // (ARCHITECTURE §10.3), so the bundle and the streamer's lookups have it too, the same
    // plane the controller reads (`landblock.md` §8.1, water OPENs).
    let h = f64::from(f32::from(LEVEL) * 1000.0 / 255.0);
    let pool = pool();
    let here = centre(10, 20);
    let under = WorldPos::new(1, here.x, here.y, h - 1.0);
    let ctx = scenery_context(BundleOptions {
        build_water: false,
        ..BundleOptions::default()
    });
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, small_rings()).unwrap();
    assert!(w.settle(here, Duration::from_secs(30)));
    assert!(w.is_under_water(&under));
    assert!(w.landblock_water(&under).is_some());
    let b = w.bundle(key(10, 20)).expect("resident");
    let plane = b.collision.as_ref().unwrap().set.water_plane().unwrap();
    assert!((f64::from(plane.level) - h).abs() < 1e-3);
    // With scenery off as well, the tables do not load: nothing has water.
    let ctx = scenery_context(BundleOptions {
        build_water: false,
        generate_scenery: false,
        ..BundleOptions::default()
    });
    let mut w = WorldStreamer::new(Arc::clone(&ctx), &pool, small_rings()).unwrap();
    assert!(w.settle(here, Duration::from_secs(30)));
    assert!(!w.is_under_water(&under));
    assert!(w.landblock_water(&under).is_none());
    let b = w.bundle(key(10, 20)).expect("resident");
    assert!(b.water.is_none());
    assert!(b.collision.as_ref().unwrap().set.water_plane().is_none());
}

#[test]
fn integration_overshoots_the_budget_until_the_next_update() {
    // Integration does not check the budget (`landblock.md` §8.1, OPEN on the transient
    // overshoot): bundles admitted on a too-small estimate all become resident, and the
    // next update's admission, with real sizes, evicts back within the budget.
    let ctx = context(SyntheticWorld::rectangle(1, 0..=20, 0..=20).build());
    let h = bundle_of(&ctx, key(10, 10)).unwrap().heap_bytes;
    let budget = 3 * h;
    let pool = pool();
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            residency_budget: budget,
            always_admit_near: false,
            initial_size_estimate: 1,
            max_integrations_per_frame: 10_000,
            frame_budget: Duration::from_secs(60),
            lookahead: Duration::ZERO,
            ..small_rings()
        },
    )
    .unwrap();
    let c = centre(10, 10);
    let r = w.update_with_velocity(c, [0.0, 0.0]);
    assert!(r.requested > 3, "{r:?}");
    // Let the jobs finish before the next update integrates them.
    // (Synthetic bundles build in well under a millisecond.)
    std::thread::sleep(Duration::from_millis(500));
    let r = w.update_with_velocity(c, [0.0, 0.0]);
    assert!(r.integrated > 3, "{r:?}");
    assert!(r.resident_bytes > budget, "{r:?}");
    let r = w.update_with_velocity(c, [0.0, 0.0]);
    assert!(r.evicted > 0, "{r:?}");
    assert!(r.resident_bytes <= budget, "{r:?}");
}
