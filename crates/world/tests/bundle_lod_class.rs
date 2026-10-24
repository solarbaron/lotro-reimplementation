// SPDX-License-Identifier: GPL-3.0-or-later
//! `Render_LODClass` (`walker.md` §11.4.3, W22) and `Render_DepthBias_Type` (`decals.md`
//! §6.5) of placed entities, read at bundle build in the lookup order of
//! `entity-descriptor.md` E26: the per-instance set (the weenie list's instance set of the
//! entity's IID when there is one, otherwise the SED's own set, never both), then the 0x47
//! chain. Synthetic records only.

use std::sync::Arc;

use tessera_assets::RecordSource;
use tessera_world::testkit::{
    SyntheticWorld, TestEntity, TestValue, entity_iid, landblock_info_with_weenies, obj, pid,
};
use tessera_world::{
    BundleOptions, BundleSource, EntityRef, LandblockBundle, LandblockKey, WorldContext,
    build_bundle,
};

fn key() -> LandblockKey {
    LandblockKey::new(1, 10, 20).expect("key")
}

fn sets(class: u32, bias: u32) -> Vec<(u32, TestValue)> {
    vec![
        (pid::RENDER_LOD_CLASS, TestValue::Enum(class)),
        (pid::RENDER_DEPTH_BIAS_TYPE, TestValue::Enum(bias)),
    ]
}

/// The synthetic world with one landblock of placements naming the scenery object, whose
/// own set gives class 5 and depth-bias type 3, and one naming the tree, which has neither.
fn bundle(options: BundleOptions) -> LandblockBundle {
    let mut m = SyntheticWorld {
        scenery: true,
        ..SyntheticWorld::rectangle(1, 10..=10, 20..=20)
    }
    .build();
    let k = key();
    let entities = [
        // 1: instance set (7, 1) wins over the own set (6, 2) and the chain.
        TestEntity::physics(1, obj::SCENERY, [5.0, 5.0, 100.0], sets(6, 2)),
        // 2: an instance set without either property: the own set is not consulted, the
        // chain's (5, 3) apply.
        TestEntity::physics(2, obj::SCENERY, [10.0, 10.0, 100.0], sets(6, 2)),
        // 3: no instance set: the own set (6, 2).
        TestEntity::physics(3, obj::SCENERY, [15.0, 15.0, 100.0], sets(6, 2)),
        // 4: nothing on the entity: the chain's (5, 3).
        TestEntity::physics(4, obj::SCENERY, [20.0, 20.0, 100.0], Vec::new()),
        // 5: an instance set without the properties, the own set with them, a chain
        // without them: nothing.
        TestEntity::physics(5, obj::TREE, [25.0, 25.0, 100.0], sets(6, 2)),
    ];
    let other = vec![(pid::RENDER_ADJUSTABLE_SCALE, TestValue::Vec3([1.0; 3]))];
    let weenies = [
        (entity_iid(k, 1), sets(7, 1)),
        (entity_iid(k, 2), other.clone()),
        (entity_iid(k, 5), other),
    ];
    m.insert(
        k.info_did(),
        landblock_info_with_weenies(k, &entities, &weenies),
        Some(39),
    );
    let ctx = WorldContext::load(Arc::new(m), options).expect("synthetic context");
    let frame = BundleSource::new(Arc::clone(ctx.source()))
        .read_record(k.data_did().0)
        .expect("frame");
    build_bundle(&ctx, k.data_did(), &frame, &|| false).expect("bundle")
}

fn check(b: &LandblockBundle) {
    assert_eq!(b.entities.len(), 5);
    let expected = [
        (Some(7), Some(1)),
        (Some(5), Some(3)),
        (Some(6), Some(2)),
        (Some(5), Some(3)),
        (None, None),
    ];
    for (i, want) in expected.iter().enumerate() {
        let r = EntityRef::Entity(u32::try_from(i).unwrap());
        assert_eq!(
            (b.lod_class(r), b.depth_bias_type(r)),
            *want,
            "entity {}",
            i + 1
        );
    }
    // Generated scenery reads its chain only.
    assert!(!b.generated.is_empty(), "scenery generated");
    for g in &b.generated {
        assert_eq!(b.lod_class_of(g), Some(5));
        assert_eq!(b.depth_bias_type_of(g), Some(3));
    }
    assert!(b.is_complete(), "{:?}", b.problems);
}

#[test]
fn classes_follow_the_entity_lookup_order() {
    check(&bundle(BundleOptions::default()));
}

#[test]
fn keeping_the_weenie_list_gives_the_same_classes() {
    check(&bundle(BundleOptions {
        keep_weenies: true,
        ..BundleOptions::default()
    }));
}
