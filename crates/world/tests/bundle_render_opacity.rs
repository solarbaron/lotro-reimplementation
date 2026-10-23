// SPDX-License-Identifier: GPL-3.0-or-later
//! The owner property `Render_Opacity` of placed and generated entities, read at bundle
//! build (`particle-runtime.md` §12 rule 15, the static owner's factor): the per-instance
//! set (the weenie list's instance set of the entity's IID, otherwise its own set;
//! `entity-descriptor.md` E26), then the 0x47 chain, on synthetic records. The values are
//! there whether or not the bundle keeps the weenie list.

use std::sync::Arc;

use tessera_assets::RecordSource;
use tessera_formats::PropertyType;
use tessera_world::testkit::{
    SyntheticWorld, TestEntity, TestValue, definitions, dictionary, entity_descriptor, entity_iid,
    landblock_info_with_weenies, obj, pid,
};
use tessera_world::{
    BundleOptions, BundleSource, EntityRef, LandblockBundle, LandblockKey, MemoryWorldSource,
    WorldContext, build_bundle,
};

/// The `Render_Opacity` PID of the test dictionary (float, type 9).
const OPACITY: u32 = 0x1000_0200;
/// A 0x47 object whose own set gives `Render_Opacity` 0.25; it shares the tree's template.
const FADED: u32 = 0x4700_0500;

fn key() -> LandblockKey {
    LandblockKey::new(1, 10, 20).expect("key")
}

/// The definitions of the synthetic dictionary plus [`OPACITY`], named `Render_Opacity`
/// when `named`, otherwise under another name (the same records decode, but the dictionary
/// has no `Render_Opacity`).
fn defs(named: bool) -> Vec<(u32, &'static str, u32)> {
    let mut d = definitions();
    let name = if named {
        "Render_Opacity"
    } else {
        "Render_OpacityOther"
    };
    d.push((OPACITY, name, 9));
    d
}

fn schema() -> std::collections::HashMap<u32, PropertyType> {
    defs(true)
        .into_iter()
        .map(|(p, _, t)| (p, PropertyType::from_code(t)))
        .collect()
}

/// The synthetic world with scenery, `Render_Opacity` in its dictionary (`with_pid`; else
/// the same PID under another name), the
/// [`FADED`] object, the scenery object carrying 0.4, and one landblock of placements that
/// cover the lookup cases.
fn world(with_pid: bool) -> MemoryWorldSource {
    let mut m = SyntheticWorld {
        scenery: true,
        ..SyntheticWorld::rectangle(1, 10..=10, 20..=20)
    }
    .build();
    let schema = schema();
    m.insert(0x3400_0000u32, dictionary(&defs(with_pid)), Some(1));
    m.insert(
        FADED,
        entity_descriptor(
            FADED,
            obj::TREE_TEMPLATE,
            &[(OPACITY, TestValue::Float(0.25))],
            &schema,
        ),
        None,
    );
    m.insert(
        obj::SCENERY,
        entity_descriptor(
            obj::SCENERY,
            obj::SCENERY_TEMPLATE,
            &[
                (pid::RENDER_LOD_CLASS, TestValue::Enum(5)),
                (pid::RENDER_DEPTH_BIAS_TYPE, TestValue::Enum(3)),
                (OPACITY, TestValue::Float(0.4)),
            ],
            &schema,
        ),
        None,
    );
    let k = key();
    let own = |v: f32| vec![(OPACITY, TestValue::Float(v))];
    let entities = [
        // 1: instance set 0.75 wins over the own set's 0.5.
        TestEntity::physics(1, obj::TREE, [5.0, 5.0, 100.0], own(0.5)),
        // 2: an instance set without the property: the own set is not consulted (E26),
        // the tree's chain has none.
        TestEntity::physics(2, obj::TREE, [10.0, 10.0, 100.0], own(0.5)),
        // 3: no instance set: the own set.
        TestEntity::physics(3, obj::TREE, [15.0, 15.0, 100.0], own(0.5)),
        // 4: nothing on the entity: the chain's 0.25.
        TestEntity::physics(4, FADED, [20.0, 20.0, 100.0], Vec::new()),
        // 5: instance set 2.0, clamped to 1.
        TestEntity::physics(5, FADED, [25.0, 25.0, 100.0], Vec::new()),
        // 6: own set −1, clamped to 0.
        TestEntity::physics(6, FADED, [30.0, 30.0, 100.0], own(-1.0)),
        // 7: own set NaN: no value (the default 1 applies); the chain is not reached.
        TestEntity::physics(7, FADED, [35.0, 35.0, 100.0], own(f32::NAN)),
        // 8: nothing anywhere.
        TestEntity::physics(8, obj::TREE, [40.0, 40.0, 100.0], Vec::new()),
    ];
    let weenies = [
        (entity_iid(k, 1), own(0.75)),
        (
            entity_iid(k, 2),
            vec![(pid::RENDER_ADJUSTABLE_SCALE, TestValue::Vec3([1.0; 3]))],
        ),
        (entity_iid(k, 5), own(2.0)),
    ];
    m.insert(
        k.info_did(),
        landblock_info_with_weenies(k, &entities, &weenies),
        Some(39),
    );
    m
}

fn bundle(source: MemoryWorldSource, options: BundleOptions) -> LandblockBundle {
    let ctx = WorldContext::load(Arc::new(source), options).expect("synthetic context");
    let frame = BundleSource::new(Arc::clone(ctx.source()))
        .read_record(key().data_did().0)
        .expect("frame");
    build_bundle(&ctx, key().data_did(), &frame, &|| false).expect("bundle")
}

fn expected() -> Vec<Option<f32>> {
    vec![
        Some(0.75),
        None,
        Some(0.5),
        Some(0.25),
        Some(1.0),
        Some(0.0),
        None,
        None,
    ]
}

#[test]
fn render_opacity_follows_the_entity_lookup_order_without_the_weenie_list() {
    let b = bundle(world(true), BundleOptions::default());
    // The walker's default: the weenie list is dropped, the values are still there.
    assert!(b.info.as_ref().is_some_and(|i| i.weenies.items.is_empty()));
    assert_eq!(b.entities.len(), 8);
    assert_eq!(b.entity_render_opacities, expected());
    for (i, e) in b.entities.iter().enumerate() {
        let r = EntityRef::Entity(u32::try_from(i).unwrap());
        assert_eq!(b.render_opacity(r), expected()[i], "entity {i}");
        assert_eq!(b.render_opacity_of(e), expected()[i], "entity {i}");
    }
    // Generated objects read their chain: the scenery object's 0.4.
    assert!(!b.generated.is_empty(), "scenery generated");
    assert_eq!(b.generated_render_opacities.len(), b.generated.len());
    for g in &b.generated {
        assert_eq!(g.object.map(|d| d.0), Some(obj::SCENERY));
        assert_eq!(b.render_opacity_of(g), Some(0.4));
    }
    // Out of range and copies are not found.
    assert_eq!(b.render_opacity(EntityRef::Entity(99)), None);
    let copy = b.entities[0].clone();
    assert_eq!(b.render_opacity_of(&copy), None);
    assert!(b.is_complete(), "{:?}", b.problems);
}

#[test]
fn keeping_the_weenie_list_gives_the_same_values() {
    let kept = bundle(
        world(true),
        BundleOptions {
            keep_weenies: true,
            ..BundleOptions::default()
        },
    );
    assert!(
        kept.info
            .as_ref()
            .is_some_and(|i| i.weenies.items.len() == 3)
    );
    assert_eq!(kept.entity_render_opacities, expected());
    // Without the world systems (no scenery, no environment) the entities keep theirs.
    let plain = bundle(
        world(true),
        BundleOptions::default().without_world_systems(),
    );
    assert_eq!(plain.entity_render_opacities, expected());
    assert!(plain.generated.is_empty() && plain.generated_render_opacities.is_empty());
}

#[test]
fn without_the_property_in_the_dictionary_there_are_no_values() {
    let b = bundle(world(false), BundleOptions::default());
    assert!(b.entity_render_opacities.is_empty());
    assert!(b.generated_render_opacities.is_empty());
    assert!(b.entities.iter().all(|e| b.render_opacity_of(e).is_none()));
    assert!(b.is_complete(), "{:?}", b.problems);
}

/// The values are charged to the residency of the bundle.
#[test]
fn render_opacities_are_charged() {
    let with = bundle(world(true), BundleOptions::default());
    let without = bundle(world(false), BundleOptions::default());
    let bytes = (with.entity_render_opacities.capacity()
        + with.generated_render_opacities.capacity())
        * std::mem::size_of::<Option<f32>>();
    assert!(bytes > 0);
    // The dictionary is not part of the charge; the info record is the same in both.
    assert_eq!(with.heap_bytes, without.heap_bytes + bytes);
}

/// A source whose reads of 0x47 records are counted and, while `failing` is set, fail for
/// a reason that is not the data's (a full queue).
struct ObjectReads {
    inner: MemoryWorldSource,
    failing: std::sync::atomic::AtomicBool,
    reads: std::sync::atomic::AtomicUsize,
}

impl tessera_world::WorldSource for ObjectReads {
    fn read_stamped(
        &self,
        did: tessera_formats::Did,
    ) -> Result<Option<tessera_world::StampedRecord>, tessera_assets::AssetError> {
        use std::sync::atomic::Ordering;
        if did.0 >> 24 == 0x47 {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.failing.load(Ordering::SeqCst) {
                return Err(tessera_assets::AssetError::QueueFull { capacity: 0 });
            }
        }
        self.inner.read_stamped(did)
    }
}

/// The 0x47 chain is read only when the per-instance set does not hold the property:
/// entities that carry their own value cost no record read, and a chain that cannot be
/// read now does not make the bundle incomplete when it is not needed. With the world
/// systems and object resolution off, this pass is the only reader of 0x47 records.
#[test]
fn the_chain_is_read_only_when_the_entity_holds_no_value() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let k = key();
    let own = |v: f32| vec![(OPACITY, TestValue::Float(v))];
    let options = BundleOptions {
        resolve_objects: false,
        ..BundleOptions::default().without_world_systems()
    };
    let run = |entities: &[TestEntity], weenies: &[(u64, Vec<(u32, TestValue)>)]| {
        let mut inner = world(true);
        inner.insert(
            k.info_did(),
            landblock_info_with_weenies(k, entities, weenies),
            Some(39),
        );
        let src = Arc::new(ObjectReads {
            inner,
            failing: AtomicBool::new(false),
            reads: AtomicUsize::new(0),
        });
        let ctx = WorldContext::load(Arc::clone(&src) as _, options.clone()).expect("context");
        src.failing.store(true, Ordering::SeqCst);
        src.reads.store(0, Ordering::SeqCst);
        let frame = BundleSource::new(Arc::clone(ctx.source()))
            .read_record(k.data_did().0)
            .expect("frame");
        let b = build_bundle(&ctx, k.data_did(), &frame, &|| false).expect("bundle");
        (b, src.reads.load(Ordering::SeqCst))
    };
    // Every value is on the entity (its own set, or the instance set of its IID).
    let (b, reads) = run(
        &[
            TestEntity::physics(1, FADED, [5.0, 5.0, 100.0], own(0.5)),
            TestEntity::physics(2, FADED, [10.0, 10.0, 100.0], Vec::new()),
            TestEntity::physics(3, obj::TREE, [15.0, 15.0, 100.0], own(f32::NAN)),
        ],
        &[(entity_iid(k, 2), own(0.75))],
    );
    assert_eq!(reads, 0, "no chain read");
    assert_eq!(b.entity_render_opacities, vec![Some(0.5), Some(0.75), None]);
    assert!(b.is_complete(), "{:?}", b.problems);
    // An entity without its own value needs the chain; its failed read is reported.
    let (b, reads) = run(
        &[
            TestEntity::physics(1, FADED, [5.0, 5.0, 100.0], own(0.5)),
            TestEntity::physics(2, FADED, [10.0, 10.0, 100.0], Vec::new()),
        ],
        &[],
    );
    assert!(reads > 0, "the chain of entity 2 is read");
    assert_eq!(b.entity_render_opacities, vec![Some(0.5), None]);
    assert!(!b.is_complete(), "a needed chain could not be read");
}
