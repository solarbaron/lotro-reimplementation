// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic unit tests of the geometry module.

use std::sync::Arc;

use proptest::prelude::*;
use tessera_formats::Did;
use tessera_formats::landblock::{GridSurface, HeightGrid, VertexFlags};
use tessera_formats::setup::{CollisionBox, CollisionCapsule, CollisionSphere, Frame, Quaternion};
use tessera_formats::{PropertyEntry, PropertySet, PropertyValue, Setup};

use super::math::{self, d3};
use super::narrow::{self, Convex};
use super::*;

fn flat_terrain(raw: u16) -> Arc<GridMesh> {
    Arc::new(GridMesh::from_grid(
        GridSurface::Terrain,
        &HeightGrid::flat(raw),
        &VertexFlags::filled(0),
    ))
}

fn player_capsule(feet: [f32; 3]) -> Capsule {
    Capsule::vertical([feet[0], feet[1], feet[2] + 0.35], 1.1, 0.35)
}

fn wall_template() -> ShapeTemplate {
    // A 1 × 10 × 4 m box, x from 0 to 1.
    let mut t = ShapeTemplate::default();
    t.push_box(&CollisionBox {
        min: [0.0, -5.0, 0.0],
        max: [1.0, 5.0, 4.0],
        frame: Frame::IDENTITY,
    })
    .expect("box");
    t
}

fn set_with_wall(flags: CollisionFlags) -> CollisionSet {
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_grid(flat_terrain(0)).expect("terrain");
    b.add_object(
        BodySource::Other { tag: 1 },
        flags,
        BodyPurpose::Movement,
        &wall_template(),
        &Transform::new([50.0, 50.0, 0.0], Quaternion::IDENTITY, 1.0),
    )
    .expect("wall");
    b.build()
}

#[test]
fn falling_capsule_lands_on_flat_terrain() {
    let set = CollisionSet::from_grids(flat_terrain(0), None);
    let c = player_capsule([20.0, 20.0, 2.0]);
    let hit = set
        .sweep_capsule(&c, [0.0, 0.0, -3.0], &QueryFilter::player())
        .expect("hit");
    let travelled = 3.0 * hit.fraction;
    let gap = 2.0 - travelled;
    assert!(
        (2.0 * SKIN * 0.99..=2.5 * SKIN * 1.01).contains(&gap),
        "gap {gap}"
    );
    assert!((hit.normal[2] - 1.0).abs() < 1e-6);
    assert_eq!(hit.face_normal, Some([0.0, 0.0, 1.0]));
    assert_eq!(
        set.body(hit.body).map(|b| b.source),
        Some(BodySource::Terrain)
    );
    assert_eq!(hit.kind, FeatureKind::Triangle);
    assert!(hit.feature < 2048);
}

#[test]
fn sliding_along_the_ground_is_free_and_moving_away_too() {
    let set = CollisionSet::from_grids(flat_terrain(0), None);
    let c = player_capsule([20.0, 20.0, 0.0015]);
    assert!(
        set.sweep_capsule(&c, [3.0, 1.0, 0.0], &QueryFilter::player())
            .is_none()
    );
    assert!(
        set.sweep_capsule(&c, [0.0, 0.0, 1.0], &QueryFilter::player())
            .is_none()
    );
    // Moving down from 1.5 skins: blocked at once (closer than the 2-skin rest gap).
    let hit = set
        .sweep_capsule(&c, [1.0, 0.0, -0.1], &QueryFilter::player())
        .expect("blocked");
    assert!(hit.fraction <= 0.006, "fraction {}", hit.fraction);
    // Inside the skin and moving down: blocked at once.
    let touching = player_capsule([20.0, 20.0, 0.0008]);
    let hit = set
        .sweep_capsule(&touching, [1.0, 0.0, -0.1], &QueryFilter::player())
        .expect("blocked");
    assert_eq!(hit.fraction, 0.0);
}

#[test]
fn wall_blocks_and_pass_through_follows_the_mover() {
    let set = set_with_wall(CollisionFlags::default());
    let c = player_capsule([45.0, 50.0, 0.01]);
    let hit = set
        .sweep_capsule(&c, [10.0, 0.0, 0.0], &QueryFilter::player())
        .expect("wall");
    // The capsule's front (x + 0.35) stops just before x = 50.
    let front = 45.0 + 10.0 * hit.fraction + 0.35;
    assert!((50.0 - front) >= 2.0 * SKIN * 0.99 && (50.0 - front) <= 2.5 * SKIN * 1.01);
    assert!(
        (hit.normal[0] + 1.0).abs() < 1e-6,
        "normal {:?}",
        hit.normal
    );

    // Ethereal to Player: the player passes, the camera does not.
    let ghost = set_with_wall(CollisionFlags {
        ethereal_to: Some(ETHEREAL_PLAYER),
        ..CollisionFlags::default()
    });
    assert!(
        ghost
            .sweep_capsule(&c, [10.0, 0.0, 0.0], &QueryFilter::player())
            .is_none()
    );
    assert!(
        ghost
            .sweep_capsule(&c, [10.0, 0.0, 0.0], &QueryFilter::camera())
            .is_some()
    );
}

#[test]
fn no_tunnelling_through_a_thin_triangle() {
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let tri = ShapeTemplate::from_triangle_list(
        &[[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]],
        &[[0, 1, 2]],
    )
    .expect("tri");
    b.add_shape(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &tri,
    )
    .expect("shape");
    let set = b.build();
    let sphere = Capsule {
        a: [-50.0, 0.0, 0.0],
        b: [-50.0, 0.0, 0.0],
        radius: 0.1,
    };
    let hit = set
        .sweep_capsule(&sphere, [100.0, 0.0, 0.0], &QueryFilter::all())
        .expect("hit");
    let x = -50.0 + 100.0 * f64::from(hit.fraction);
    assert!(x < -0.1 && x > -0.1 - 3.0 * f64::from(SKIN), "x {x}");
    // A ray through the same triangle, from the other side.
    let ray = set
        .raycast([5.0, 0.2, 0.0], [-10.0, 0.0, 0.0], &QueryFilter::all())
        .expect("ray");
    assert!((ray.normal[0] - 1.0).abs() < 1e-6);
    assert_eq!(ray.face_normal, Some([1.0, 0.0, 0.0]));
}

#[test]
fn penetration_is_reported_only_when_moving_in() {
    let set = set_with_wall(CollisionFlags::default());
    // Capsule overlapping the wall's west face by 0.1.
    let c = player_capsule([49.75, 50.0, 1.0]);
    let into = set
        .sweep_capsule(&c, [1.0, 0.0, 0.0], &QueryFilter::player())
        .expect("blocked");
    assert_eq!(into.fraction, 0.0);
    assert!((into.gap + 0.1).abs() < 1e-4, "gap {}", into.gap);
    assert!(
        set.sweep_capsule(&c, [-1.0, 0.0, 0.0], &QueryFilter::player())
            .is_none()
    );
    let mut contacts = Vec::new();
    set.contacts(&c, 0.01, &QueryFilter::player(), &mut contacts);
    let wall: Vec<_> = contacts
        .iter()
        .filter(|k| {
            set.body(k.body)
                .is_some_and(|b| b.source == BodySource::Other { tag: 1 })
        })
        .collect();
    assert!(!wall.is_empty());
    assert!(wall.iter().all(|k| (k.separation + 0.1).abs() < 1e-4));
    assert!(wall.iter().all(|k| k.normal[0] < -0.99));
}

#[test]
fn contacts_report_the_ground_with_its_face_normal() {
    let set = CollisionSet::from_grids(flat_terrain(0), None);
    let c = player_capsule([12.5, 12.5, 0.002]);
    let mut out = Vec::new();
    set.contacts(&c, 0.05, &QueryFilter::player(), &mut out);
    assert!(!out.is_empty());
    for k in &out {
        assert!((k.separation - 0.002).abs() < 1e-5);
        assert_eq!(k.face_normal, Some([0.0, 0.0, 1.0]));
    }
}

#[test]
fn spheres_and_capsules_collide() {
    let mut setup = Setup::default();
    setup.spheres.push(CollisionSphere {
        centre: [0.0, 0.0, 1.0],
        radius: 1.0,
    });
    setup.capsules.push(CollisionCapsule {
        base: [5.0, 0.0, 0.5],
        length: 2.0,
        radius: 0.5,
    });
    let t = ShapeTemplate::from_setup(&setup, None).expect("template");
    assert_eq!((t.spheres.len(), t.capsules.len()), (1, 1));
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_object(
        BodySource::Other { tag: 9 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &t,
        &Transform::new([10.0, 10.0, 0.0], Quaternion::IDENTITY, 2.0),
    )
    .expect("object");
    let set = b.build();
    // The sphere scaled by 2: centre (10, 10, 2), radius 2.
    let probe = Capsule {
        a: [0.0, 10.0, 2.0],
        b: [0.0, 10.0, 2.0],
        radius: 0.5,
    };
    let hit = set
        .sweep_capsule(&probe, [20.0, 0.0, 0.0], &QueryFilter::all())
        .expect("sphere");
    let x = 20.0 * hit.fraction;
    assert!((x - 7.5).abs() < 0.003, "x {x}");
    assert_eq!(hit.kind, FeatureKind::Round);
    assert_eq!(hit.feature, 0);
    // The capsule: placed at (20, 10, 1)–(20, 10, 5), radius 1.
    let probe = Capsule {
        a: [20.0, 0.0, 3.0],
        b: [20.0, 0.0, 3.0],
        radius: 0.0,
    };
    let hit = set
        .sweep_capsule(&probe, [0.0, 20.0, 0.0], &QueryFilter::all())
        .expect("capsule");
    assert!((20.0 * hit.fraction - 9.0).abs() < 0.003);
    assert_eq!(hit.feature, 1);
}

#[test]
fn rotated_boxes_follow_the_box_frame_and_the_object_transform() {
    let mut setup = Setup::default();
    setup.boxes.push(CollisionBox {
        min: [-1.0, -1.0, 0.0],
        max: [1.0, 1.0, 2.0],
        frame: Frame {
            position: [3.0, 0.0, 0.0],
            scale: [1.0; 3],
            rotation: Quaternion::from_yaw(std::f32::consts::FRAC_PI_4),
        },
    });
    let t = ShapeTemplate::from_setup(&setup, None).expect("template");
    assert_eq!(t.triangles.len(), 12);
    let b = t.bounds().expect("bounds");
    let r = std::f32::consts::SQRT_2;
    assert!((b.min[0] - (3.0 - r)).abs() < 1e-5 && (b.max[1] - r).abs() < 1e-5);
    // Turn the object by 90°: the box moves to (0, 3).
    let placed = t
        .place(&Transform::new(
            [0.0; 3],
            Quaternion::from_yaw(std::f32::consts::FRAC_PI_2),
            1.0,
        ))
        .expect("placed");
    let pb = placed.bounds().expect("bounds");
    assert!((pb.centre()[0]).abs() < 1e-5 && (pb.centre()[1] - 3.0).abs() < 1e-5);
    // Hostile transforms are refused.
    assert!(
        t.place(&Transform::new(
            [f32::NAN, 0.0, 0.0],
            Quaternion::IDENTITY,
            1.0
        ))
        .is_err()
    );
    assert!(
        t.place(&Transform::new([0.0; 3], Quaternion::IDENTITY, 0.0))
            .is_err()
    );
}

#[test]
fn cell_lookup_prefers_the_lowest_number_and_honours_rotation() {
    let room = Arc::new(
        CellVolume::new([
            ([1.0, 0.0, 0.0], 2.0),
            ([-1.0, 0.0, 0.0], 2.0),
            ([0.0, 1.0, 0.0], 1.0),
            ([0.0, -1.0, 0.0], 1.0),
            ([0.0, 0.0, 1.0], 0.0),
            ([0.0, 0.0, -1.0], 3.0),
        ])
        .expect("volume"),
    );
    let hull: Vec<[f32; 3]> = (0..8)
        .map(|k| {
            [
                if k & 1 == 0 { -2.0 } else { 2.0 },
                if k & 2 == 0 { -1.0 } else { 1.0 },
                if k & 4 == 0 { 0.0 } else { 3.0 },
            ]
        })
        .collect();
    let turned = CellPlacement::new(
        [10.0, 10.0, 0.0],
        Quaternion::from_yaw(std::f32::consts::FRAC_PI_2),
    )
    .expect("placement");
    let mut cells = CellSet::new();
    cells.push(
        PlacedCell::with_volume(9, Did(0x0100_0009), room.clone(), &hull, turned).expect("cell"),
    );
    cells.push(
        PlacedCell::with_volume(4, Did(0x0100_0004), room.clone(), &hull, turned).expect("cell"),
    );
    // Turned by 90°: the room spans x 9–11 and y 8–12.
    assert_eq!(cells.cell_at([10.9, 11.9, 1.0]).map(|c| c.index), Some(4));
    assert!(cells.cell_at([11.5, 10.0, 1.0]).is_none());
    // Far from the origin: no lookup at all (§5.8.1 step 1).
    let far = CellPlacement::new([3000.0, 0.0, 0.0], Quaternion::IDENTITY).expect("placement");
    let mut cells = CellSet::new();
    cells.push(PlacedCell::with_volume(1, Did(0x0100_0001), room, &hull, far).expect("cell"));
    assert!(cells.get(1).is_some_and(|c| c.contains([3000.0, 0.0, 1.0])));
    assert!(cells.cell_at([3000.0, 0.0, 1.0]).is_none());
    // Hostile input.
    assert!(CellVolume::new([([f32::NAN, 0.0, 0.0], 1.0)]).is_err());
    assert!(
        CellPlacement::new(
            [0.0; 3],
            Quaternion {
                w: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        )
        .is_err()
    );
}

#[test]
fn content_layer_test() {
    let shared = [1u32];
    assert!(content_layers_pass(None, &shared, 0));
    assert!(content_layers_pass(Some(&[]), &shared, 0));
    assert!(content_layers_pass(Some(&[2]), &shared, 0));
    assert!(content_layers_pass(Some(&[1]), &shared, 0));
    assert!(!content_layers_pass(Some(&[0x1000_008D]), &shared, 0));
    assert!(content_layers_pass(Some(&[0x1000_008D]), &[0x1000_008D], 3));
    // A list naming only 1 disappears in an instanced copy under another layer.
    assert!(!content_layers_pass(Some(&[1]), &[0x1000_008D], 3));
    assert!(content_layers_pass(Some(&[1]), &[1], 3));
    // Regression: e = 1 passes in the shared world (I = 0) whatever the active layers, also
    // with none active (`entity-descriptor.md` §4.5.1).
    assert!(content_layers_pass(Some(&[1]), &[], 0));
    assert!(content_layers_pass(Some(&[1]), &[0x1000_008D], 0));
    assert!(!content_layers_pass(Some(&[1]), &[], 3));
    assert!(content_layers_pass(Some(&[2]), &[], 3));
    assert!(!content_layers_pass(Some(&[0x1000_008D]), &[], 0));
}

#[test]
fn flags_layer_and_classify() {
    let pids = PhysicsPids {
        no_physics: Some(10),
        ethereal_to_type: Some(11),
        always_uncontrolled_surface: Some(12),
        ..PhysicsPids::default()
    };
    let set = |entries: &[(u32, PropertyValue)]| PropertySet {
        entries: entries
            .iter()
            .map(|(pid, value)| PropertyEntry {
                pid: *pid,
                value: value.clone(),
            })
            .collect(),
        ..PropertySet::default()
    };
    let sed = set(&[(10, PropertyValue::Bool(false))]);
    let obj = set(&[
        (10, PropertyValue::Bool(true)),
        (11, PropertyValue::Bitfield32(1 << 29)),
        (12, PropertyValue::Bool(true)),
    ]);
    let f = CollisionFlags::from_set(&sed, &pids).or(CollisionFlags::from_set(&obj, &pids));
    assert!(!f.has_no_physics(), "the SED's own value wins");
    assert!(f.passes(ETHEREAL_PLAYER) && !f.passes(ETHEREAL_CAMERA));
    assert!(f.is_always_uncontrolled());
    assert!(!static_blocks_walker(true, true, &f, SetupCollision::Mesh));
    // A static the client creates (creation type 3, §9.3 item 3) with no other flags.
    let solid = CollisionFlags {
        creation_type: Some(CREATION_TYPE_SHARED_STATIC_IMMOBILE),
        ..CollisionFlags::default()
    };
    assert!(static_blocks_walker(
        true,
        true,
        &solid,
        SetupCollision::Volumes
    ));
    assert!(!static_blocks_walker(
        true,
        false,
        &solid,
        SetupCollision::Mesh
    ));
    assert!(!static_blocks_walker(
        true,
        true,
        &solid,
        SetupCollision::None
    ));
    assert!(generated_blocks_walker(&solid, SetupCollision::Mesh));
    // No creation type anywhere counts as 0 and is refused; generated objects pass.
    let none = CollisionFlags::default();
    assert!(none.refuses_static_creation());
    assert!(!static_blocks_walker(
        true,
        true,
        &none,
        SetupCollision::Mesh
    ));
    assert!(static_has_walker_collision(&none, SetupCollision::Mesh));
    assert!(generated_blocks_walker(&none, SetupCollision::Mesh));
    assert_eq!(ethereality_bit(0x1E), Some(ETHEREAL_PLAYER));
    assert_eq!(ethereality_bit(2), Some(ETHEREAL_CAMERA));
    assert_eq!(ethereality_bit(0), None);
}

fn property_set(entries: &[(u32, PropertyValue)]) -> PropertySet {
    PropertySet {
        entries: entries
            .iter()
            .map(|(pid, value)| PropertyEntry {
                pid: *pid,
                value: value.clone(),
            })
            .collect(),
        ..PropertySet::default()
    }
}

/// `collision.md` §9.3 item 3, C29: the SED's own `Entity_CreationType` first, then the
/// chain; 0, 7 and 8 are refused; no value anywhere, or a found value that is not an enum
/// or property-ID value, counts as 0 (and a wrong-typed value ends the lookup).
#[test]
fn creation_type_is_the_seds_own_value_first() {
    const CT: u32 = 20;
    let pids = PhysicsPids {
        creation_type: Some(CT),
        ..PhysicsPids::default()
    };
    let flags = |v: Option<PropertyValue>| {
        let entries: Vec<_> = v.into_iter().map(|v| (CT, v)).collect();
        CollisionFlags::from_set(&property_set(&entries), &pids)
    };
    let effective = |own: Option<PropertyValue>, object: Option<PropertyValue>| {
        CollisionFlags::of_static(flags(own), flags(object))
    };
    let e = |k: u32| Some(PropertyValue::Enum(k));
    // The SED's own value wins over the object's.
    assert!(!effective(e(3), e(8)).refuses_static_creation());
    assert!(effective(e(7), e(3)).refuses_static_creation());
    assert!(effective(e(8), e(3)).refuses_static_creation());
    assert!(effective(e(0), e(3)).refuses_static_creation());
    // The chain when the SED has none.
    assert!(effective(None, e(8)).refuses_static_creation());
    assert!(!effective(None, e(9)).refuses_static_creation());
    // None anywhere: 0.
    let none = effective(None, None);
    assert_eq!(none.creation_type_value(), CREATION_TYPE_INVALID);
    assert!(none.refuses_static_creation() && !none.is_server_created());
    // Property-ID values count (C41); integers and other types read as 0 and stop the
    // lookup (§9.3 point 1, changed 2026-10-08).
    assert!(!effective(Some(PropertyValue::PropertyId(5)), e(8)).refuses_static_creation());
    assert_eq!(
        effective(Some(PropertyValue::PropertyId(9)), None).creation_type_value(),
        9
    );
    for wrong in [
        PropertyValue::Int(5),
        PropertyValue::Int(-1),
        PropertyValue::Bitfield32(3),
        PropertyValue::Int64(3),
        PropertyValue::Bool(true),
        PropertyValue::Float(3.0),
    ] {
        let f = effective(Some(wrong.clone()), e(3));
        assert_eq!(f.creation_type, Some(0), "{wrong:?}");
        assert!(f.refuses_static_creation(), "{wrong:?}");
    }
    // A dictionary without the name: nothing is found, every static is refused.
    let blind = CollisionFlags::from_set(
        &property_set(&[(CT, PropertyValue::Enum(3))]),
        &PhysicsPids::default(),
    );
    assert!(blind.refuses_static_creation());
}

/// Finding: the §9.1 booleans accepted integers and enum keys. Only boolean values count;
/// another type is not present and the lookup goes on along the chain.
#[test]
fn physics_booleans_read_only_boolean_values() {
    let pids = PhysicsPids {
        no_physics: Some(10),
        always_uncontrolled_surface: Some(12),
        can_ledge_climb: Some(13),
        ..PhysicsPids::default()
    };
    let wrong = property_set(&[
        (10, PropertyValue::Int(1)),
        (12, PropertyValue::Enum(1)),
        (13, PropertyValue::Int(0)),
    ]);
    let f = CollisionFlags::from_set(&wrong, &pids);
    assert_eq!(
        (
            f.no_physics,
            f.always_uncontrolled_surface,
            f.can_ledge_climb
        ),
        (None, None, None)
    );
    let object = property_set(&[
        (10, PropertyValue::Bool(false)),
        (13, PropertyValue::Bool(true)),
    ]);
    let f = CollisionFlags::of_static(f, CollisionFlags::from_set(&object, &pids));
    assert_eq!((f.no_physics, f.can_ledge_climb), (Some(false), Some(true)));
}

/// `collision.md` §9.1 "Value types" (BIN, C41, changed 2026-10-08): an ethereality value is
/// used only when it is a Bitfield32 (type 18), as its 32 bits; any other type, the 64-bit
/// and variable-width bitfields included, reads as no categories and ends the lookup, as
/// in the client.
#[test]
fn ethereality_reads_only_bitfield32_values() {
    let pids = PhysicsPids {
        ethereality_type: Some(11),
        ethereal_to_type: Some(12),
        placement_ethereal_to_type: Some(13),
        ..PhysicsPids::default()
    };
    let player = i32::try_from(ETHEREAL_PLAYER).expect("bit 29");
    for wrong in [
        PropertyValue::Int(player),
        PropertyValue::Int64(i64::from(player)),
        PropertyValue::Enum(1 << 29),
        PropertyValue::Bool(true),
        PropertyValue::Bitfield64(ETHEREAL_PLAYER),
    ] {
        let set = property_set(&[
            (11, wrong.clone()),
            (12, wrong.clone()),
            (13, wrong.clone()),
        ]);
        let f = CollisionFlags::from_set(&set, &pids);
        assert_eq!(
            (f.ethereality, f.ethereal_to, f.placement_ethereal_to),
            (Some(0), Some(0), Some(0)),
            "{wrong:?}"
        );
        assert!(!f.passes(ETHEREAL_PLAYER), "{wrong:?}");
        // The lookup stops there: the chain's value is not consulted.
        let object = property_set(&[(12, PropertyValue::Bitfield32(1 << 1))]);
        let f = CollisionFlags::of_static(f, CollisionFlags::from_set(&object, &pids));
        assert_eq!(f.ethereal_to, Some(0), "{wrong:?}");
        assert!(!f.passes(ETHEREAL_CAMERA), "{wrong:?}");
    }
    let right = property_set(&[
        (11, PropertyValue::Bitfield32(1)),
        (12, PropertyValue::Bitfield32(1 << 29)),
    ]);
    let f = CollisionFlags::from_set(&right, &pids);
    assert_eq!(
        (f.ethereality, f.ethereal_to),
        (Some(1), Some(ETHEREAL_PLAYER))
    );
    assert!(f.passes(ETHEREAL_PLAYER));
}

/// `collision.md` §8.1 steps 1–4 (BIN, C42) with Tessera's upper bound (§9.3 point 2): the
/// physics scale is uniform, z of `Physics_AdjustableScale` × z of `Model_Scale`.
#[test]
fn physics_scale_is_the_product_of_the_z_components() {
    assert_eq!(physics_scale(None, None), 1.0);
    assert_eq!(physics_scale(Some([0.7; 3]), None), 0.7);
    assert_eq!(physics_scale(Some([2.0; 3]), Some([1.5; 3])), 3.0);
    // Non-uniform: z is used (x and y play no part).
    assert_eq!(physics_scale(Some([5.0, 6.0, 0.5]), None), 0.5);
    assert_eq!(physics_scale(Some([1.0; 3]), Some([9.0, 9.0, 2.0])), 2.0);
    // Within 0.0002 of 1: 1 (the initial scale is kept).
    assert_eq!(physics_scale(Some([1.0001; 3]), None), 1.0);
    // Not finite, below 0.0002 or above Tessera's 10^4: 1.
    assert_eq!(physics_scale(Some([1.0, 1.0, f32::NAN]), None), 1.0);
    assert_eq!(physics_scale(Some([f32::INFINITY; 3]), None), 1.0);
    assert_eq!(physics_scale(Some([0.0001; 3]), None), 1.0);
    assert_eq!(physics_scale(Some([-2.0; 3]), None), 1.0);
    assert_eq!(physics_scale(Some([200.0; 3]), Some([100.0; 3])), 1.0);
    assert_eq!(physics_scale(Some([100.0; 3]), Some([100.0; 3])), 10_000.0);
    // No upper limit in the client below Tessera's bound; the product may exceed each
    // factor's own range.
    assert_eq!(physics_scale(Some([0.01; 3]), Some([0.05; 3])), 0.01 * 0.05);
}

/// A liquid box's column span agrees with its containment test, for tilted and scaled
/// boxes too (`water.md` §7.4 surface, exact instead of probed).
#[test]
fn liquid_column_span_matches_contains() {
    let n = (0.3_f32 * 0.3 + 0.2 * 0.2 + 0.1 * 0.1 + 0.9 * 0.9).sqrt();
    let tilted = Quaternion {
        w: 0.9 / n,
        x: 0.3 / n,
        y: 0.2 / n,
        z: 0.1 / n,
    };
    for rotation in [Quaternion::IDENTITY, Quaternion::from_yaw(0.7), tilted] {
        let t = Transform {
            position: [20.0, 30.0, 5.0],
            rotation,
            scale: [1.5, 1.5, 1.5],
        };
        let b = LiquidBox::new(
            [-4.0, -3.0, -6.0],
            [4.0, 3.0, 1.0],
            &t,
            LiquidFlags::default(),
        )
        .expect("box");
        let mut hits = 0;
        for i in 0..21 {
            for j in 0..21 {
                #[allow(clippy::cast_precision_loss)]
                let (x, y) = (10.0 + i as f32, 20.0 + j as f32);
                let span = b.column_span(x, y);
                for k in 0..61 {
                    #[allow(clippy::cast_precision_loss)]
                    let z = -10.0 + 0.4 * k as f32;
                    if b.contains([x, y, z]) {
                        let (lo, hi) = span.expect("a span where the box contains a point");
                        let z = f64::from(z);
                        assert!(lo - 1e-9 <= z && z <= hi + 1e-9, "{x} {y} {z}: {lo} {hi}");
                    }
                }
                if let Some((lo, hi)) = span {
                    hits += 1;
                    #[allow(clippy::cast_possible_truncation)]
                    let mid = (0.5 * (lo + hi)) as f32;
                    assert!(b.contains([x, y, mid]), "{x} {y}: {lo} {hi}");
                }
            }
        }
        assert!(hits > 50, "{rotation:?}: {hits}");
    }
    assert_eq!(
        LiquidBox::new(
            [0.0; 3],
            [1.0; 3],
            &Transform::IDENTITY,
            LiquidFlags::default()
        )
        .expect("box")
        .column_span(2.0, 0.5),
        None
    );
}

/// Finding: the liquid flags accepted integers and enum keys (`water.md` §7.2 types them
/// boolean).
#[test]
fn liquid_flags_read_only_boolean_values() {
    let pids = LiquidPids {
        is_liquid: Some(1),
        water_deep: Some(2),
        ..LiquidPids::default()
    };
    let wrong = property_set(&[(1, PropertyValue::Int(1)), (2, PropertyValue::Enum(1))]);
    assert_eq!(LiquidFlags::from_set(&wrong, &pids), LiquidFlags::default());
    let right = property_set(&[
        (1, PropertyValue::Bool(true)),
        (2, PropertyValue::Bool(true)),
    ]);
    let f = LiquidFlags::from_set(&right, &pids);
    assert!(f.liquid && f.deep);
}

/// `collision.md` §9.3 (the paragraph after item 3): `Entity_ContentLayers` and
/// `Physics_AdjustableScale` use the SED-then-chain lookup; a found value ends it.
#[test]
fn layer_list_and_scale_use_the_sed_then_chain_lookup() {
    const LAYERS: u32 = 30;
    const SCALE: u32 = 31;
    let pids = PhysicsPids {
        content_layers: Some(LAYERS),
        adjustable_scale: Some(SCALE),
        ..PhysicsPids::default()
    };
    let list = |l: &[u32]| {
        PropertyValue::Array(tessera_formats::PropertyArray {
            elements: l
                .iter()
                .map(|&k| PropertyEntry {
                    pid: 99,
                    value: PropertyValue::Enum(k),
                })
                .collect(),
            ..tessera_formats::PropertyArray::default()
        })
    };
    let lookups = |e: &[(u32, PropertyValue)]| EntityLookups::from_set(&property_set(e), &pids);
    let object = lookups(&[
        (LAYERS, list(&[0x1000_008D])),
        (SCALE, PropertyValue::Vector3([2.0; 3])),
    ]);
    // The SED has neither: the object's values.
    let l = lookups(&[]).or(object.clone());
    assert_eq!(l.layer_list(), Some(&[0x1000_008D][..]));
    assert_eq!(l.scale(), [2.0; 3]);
    assert!(!content_layers_pass(l.layer_list(), &[1], 0));
    // The SED's own values win.
    let l = lookups(&[
        (LAYERS, list(&[1])),
        (SCALE, PropertyValue::Vector3([0.5; 3])),
    ])
    .or(object.clone());
    assert_eq!(l.layer_list(), Some(&[1][..]));
    assert_eq!(l.scale(), [0.5; 3]);
    // Found but wrong: no list (the SED exists) and scale 1, the object is not consulted.
    let l = lookups(&[
        (LAYERS, PropertyValue::Int(7)),
        (SCALE, PropertyValue::Vector3([0.0; 3])),
    ])
    .or(object);
    assert_eq!(l.layer_list(), None);
    assert!(content_layers_pass(l.layer_list(), &[1], 0));
    assert_eq!(l.scale(), [1.0; 3]);
    // Nothing anywhere: no list, scale 1.
    let l = EntityLookups::default();
    assert_eq!((l.layer_list(), l.scale()), (None, [1.0; 3]));
}

#[test]
fn views_offset_sets_and_report_set_indices() {
    let a = CollisionSet::from_grids(flat_terrain(0), None);
    // The east neighbour is 10 m higher (raw 327.675 per metre: 2,000 / 65,535 per step).
    let b = CollisionSet::from_grids(flat_terrain(328), None);
    let mut view = CollisionView::new();
    view.push(&a, [0.0; 3]);
    view.push(&b, [160.0, 0.0, 0.0]);
    let c = player_capsule([200.0, 10.0, 20.0]);
    let hit = view
        .sweep_capsule(&c, [0.0, 0.0, -30.0], &QueryFilter::player())
        .expect("ground");
    assert_eq!(hit.set, 1);
    assert!((hit.point[0] - 200.0).abs() < 1e-3, "point {:?}", hit.point);
    let ground = 328.0 * 2000.0 / 65_535.0;
    let feet = 20.0 - 30.0 * hit.fraction;
    assert!((feet - ground) > 0.0 && (feet - ground) < 0.003);
    assert!(view.body(1, hit.body).is_some());
    assert!(view.body(2, hit.body).is_none());
}

#[test]
fn limits_are_enforced() {
    // max_rounds is high, so only the body limit can refuse the third body.
    let mut b = CollisionSetBuilder::new(CollisionLimits {
        max_triangles: 3000,
        max_rounds: 100,
        max_bodies: 2,
        max_cells: 0,
    });
    b.add_grid(flat_terrain(0)).expect("terrain fits");
    assert!(b.add_grid(flat_terrain(0)).is_err(), "triangles");
    let wall = wall_template();
    let r = b.add_object(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &wall,
        &Transform::IDENTITY,
    );
    assert!(r.is_ok());
    assert!(matches!(
        b.add_object(
            BodySource::Other { tag: 0 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &wall,
            &Transform::IDENTITY,
        ),
        Err(crate::PhysicsError::TooLarge {
            what: "bodies",
            count: 3,
            limit: 2
        })
    ));
    let set = b.build();
    assert_eq!(set.bodies().len(), 2);
    // The round limit on its own: one solid box fits, a second does not.
    let mut b = CollisionSetBuilder::new(CollisionLimits {
        max_rounds: 1,
        ..CollisionLimits::default()
    });
    let add = |b: &mut CollisionSetBuilder| {
        b.add_object(
            BodySource::Other { tag: 0 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &wall,
            &Transform::IDENTITY,
        )
    };
    assert!(add(&mut b).is_ok());
    assert!(matches!(
        add(&mut b),
        Err(crate::PhysicsError::TooLarge {
            what: "round primitives",
            ..
        })
    ));
}

#[test]
fn hostile_queries_return_nothing() {
    let set = set_with_wall(CollisionFlags::default());
    let bad = Capsule {
        a: [f32::NAN, 0.0, 0.0],
        b: [0.0; 3],
        radius: 1.0,
    };
    assert!(
        set.sweep_capsule(&bad, [1.0, 0.0, 0.0], &QueryFilter::all())
            .is_none()
    );
    let ok = player_capsule([10.0, 10.0, 1.0]);
    assert!(
        set.sweep_capsule(&ok, [f32::INFINITY, 0.0, 0.0], &QueryFilter::all())
            .is_none()
    );
    let mut out = Vec::new();
    set.contacts(&bad, 1.0, &QueryFilter::all(), &mut out);
    set.contacts(&ok, f32::NAN, &QueryFilter::all(), &mut out);
    assert!(out.is_empty());
    assert!(set.cell_at([f32::NAN, 0.0, 0.0]).is_none());
    assert!(GridMesh::client_keys_near(f32::NAN, 0.0, 1.0).is_empty());
    assert!(GridMesh::client_keys_near(10.0, 10.0, -1.0).is_empty());
}

#[test]
fn detail_bodies_are_for_the_camera_only() {
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let positions = [
        Some([0.0, -1.0, -1.0]),
        Some([0.0, 1.0, -1.0]),
        Some([0.0, 0.0, 1.0]),
        None,
    ];
    let t = ShapeTemplate::from_level_collision(&positions, &[[0, 1, 2], [0, 1, 3]]).expect("t");
    assert_eq!((t.triangles.len(), t.dropped), (1, 1));
    b.add_shape(
        BodySource::Other { tag: 3 },
        CollisionFlags::default(),
        BodyPurpose::Detail,
        &t,
    )
    .expect("shape");
    let set = b.build();
    let ray = |f: &QueryFilter| set.raycast([-5.0, 0.0, 0.0], [10.0, 0.0, 0.0], f);
    assert!(ray(&QueryFilter::player()).is_none());
    assert!(ray(&QueryFilter::camera()).is_some());
}

#[test]
fn radius_query_matches_its_rules_on_a_grid_of_points() {
    // Every collected key lies in a quad that the circle reaches, and step 4 collects both
    // triangles exactly when the diagonal segment is within r.
    for xi in 0..40 {
        for yi in 0..40 {
            #[allow(clippy::cast_precision_loss)]
            let (x, y) = (xi as f32 * 4.1 - 3.0, yi as f32 * 4.1 - 3.0);
            let keys = GridMesh::client_keys_near(x, y, 2.5);
            for pair in keys.windows(2) {
                assert!(pair[0] < pair[1] || pair[0] / 64 != pair[1] / 64 || pair[0] > pair[1]);
            }
            for k in keys {
                assert!(k < 2048);
            }
        }
    }
}

#[test]
fn disc_query_finds_the_triangles_under_the_disc() {
    let mesh = flat_terrain(0);
    // A tiny disc in the south-west half of quad (2, 2) with the SE-NW diagonal.
    assert_eq!(mesh.keys_in_disc(11.0, 11.0, 0.1), vec![132]);
    assert_eq!(mesh.keys_in_disc(14.0, 14.0, 0.1), vec![133]);
    // Across the diagonal: both.
    assert_eq!(mesh.keys_in_disc(12.5, 12.5, 0.1), vec![132, 133]);
    // A big disc: whole quads.
    assert_eq!(mesh.keys_in_disc(80.0, 80.0, 400.0).len(), 2048);
    assert!(mesh.keys_in_disc(-10.0, 80.0, 1.0).is_empty());
}

fn tri_strategy() -> impl Strategy<Value = [[f32; 3]; 3]> {
    let p = || (-5.0f32..5.0, -5.0f32..5.0, -5.0f32..5.0).prop_map(|(x, y, z)| [x, y, z]);
    (p(), p(), p()).prop_map(|(a, b, c)| [a, b, c])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// After a sweep the capsule never overlaps the obstacle.
    #[test]
    fn sweeps_never_end_inside(
        tri in tri_strategy(),
        start in (-12.0f32..12.0, -12.0f32..12.0, -12.0f32..12.0),
        delta in (-20.0f32..20.0, -20.0f32..20.0, -20.0f32..20.0),
        r in 0.0f32..1.5,
        len in 0.0f32..2.0,
    ) {
        let item = Convex::Triangle(tri.map(d3));
        let a = [start.0, start.1, start.2];
        let cap = Capsule::vertical(a, len, r);
        let (p, q) = (d3(cap.a), d3(cap.b));
        let (gap0, _, _) = narrow::separation(p, q, f64::from(r), &item, [0.0, 0.0, 1.0]);
        prop_assume!(gap0 > 0.01);
        let dd = [f64::from(delta.0), f64::from(delta.1), f64::from(delta.2)];
        let skin = f64::from(SKIN);
        let end_t = narrow::time_of_impact(p, q, f64::from(r), dd, &item, skin, gap0).map_or(1.0, |h| h.t);
        let off = math::scale(dd, end_t);
        let (gap, _, _) = narrow::separation(math::add(p, off), math::add(q, off), f64::from(r), &item, [0.0, 0.0, 1.0]);
        prop_assert!(gap > 0.5 * skin, "ended with gap {}", gap);
        // And sampling the path finds no overlap before the end.
        for k in 0..=64 {
            let t = end_t * f64::from(k) / 64.0;
            let off = math::scale(dd, t);
            let (g, _, _) = narrow::separation(math::add(p, off), math::add(q, off), f64::from(r), &item, [0.0, 0.0, 1.0]);
            prop_assert!(g > 0.0, "overlap at t {} (gap {})", t, g);
        }
    }

    /// The hierarchy finds exactly what a linear scan finds.
    #[test]
    fn bvh_matches_brute_force(
        boxes in proptest::collection::vec(
            ((-50.0f32..50.0, -50.0f32..50.0, -50.0f32..50.0), (0.0f32..8.0, 0.0f32..8.0, 0.0f32..8.0)),
            0..300,
        ),
        q in ((-60.0f32..60.0, -60.0f32..60.0, -60.0f32..60.0), (0.0f32..30.0, 0.0f32..30.0, 0.0f32..30.0)),
    ) {
        let boxes: Vec<Aabb> = boxes
            .into_iter()
            .map(|((x, y, z), (a, b, c))| Aabb { min: [x, y, z], max: [x + a, y + b, z + c] })
            .collect();
        let query = Aabb { min: [q.0.0, q.0.1, q.0.2], max: [q.0.0 + q.1.0, q.0.1 + q.1.1, q.0.2 + q.1.2] };
        let bvh = Bvh::build(&boxes);
        let mut found = Vec::new();
        bvh.query(&query, |i| found.push(i));
        found.sort_unstable();
        let expected: Vec<u32> = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.overlaps(&query))
            .map(|(i, _)| u32::try_from(i).unwrap_or(u32::MAX))
            .collect();
        prop_assert_eq!(found, expected);
        prop_assert!(bvh.node_count() <= 2 * boxes.len().max(1));
    }

    /// Segment–triangle distance is never larger than sampled distances and never
    /// smaller than the true distance by more than rounding.
    #[test]
    fn segment_triangle_distance_is_minimal(
        tri in tri_strategy(),
        p in (-8.0f32..8.0, -8.0f32..8.0, -8.0f32..8.0),
        q in (-8.0f32..8.0, -8.0f32..8.0, -8.0f32..8.0),
    ) {
        let t = tri.map(d3);
        let (pp, qq) = (d3([p.0, p.1, p.2]), d3([q.0, q.1, q.2]));
        let (s, c) = narrow::segment_triangle(pp, qq, t[0], t[1], t[2]);
        let d = math::len(math::sub(s, c));
        for k in 0..=32 {
            let x = math::lerp(pp, qq, f64::from(k) / 32.0);
            let y = narrow::point_triangle(x, t[0], t[1], t[2]);
            prop_assert!(d <= math::len(math::sub(x, y)) + 1e-9);
        }
    }
}

#[test]
#[ignore = "timing probe"]
fn bvh_build_timing() {
    let mut boxes = Vec::new();
    let mut x = 12345u32;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        #[allow(clippy::cast_precision_loss)]
        let v = x as f32 / u32::MAX as f32;
        v
    };
    for _ in 0..900_000 {
        let p = [rnd() * 160.0, rnd() * 160.0, rnd() * 50.0];
        boxes.push(Aabb {
            min: p,
            max: [p[0] + rnd(), p[1] + rnd(), p[2] + rnd()],
        });
    }
    let t = std::time::Instant::now();
    let bvh = Bvh::build(&boxes);
    eprintln!(
        "bvh: {} items, {} nodes, {:?}",
        boxes.len(),
        bvh.node_count(),
        t.elapsed()
    );
}

#[test]
fn shared_types_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<GridMesh>();
    check::<CollisionSet>();
    check::<CollisionSetBuilder>();
    check::<CollisionView<'static>>();
    check::<CellSet>();
    check::<ObjectCollisionCache>();
    check::<ShapeTemplate>();
    check::<Bvh>();
}

#[test]
fn identical_inputs_give_identical_results() {
    let build = || set_with_wall(CollisionFlags::default());
    let (a, b) = (build(), build());
    let c = player_capsule([44.0, 49.0, 0.3]);
    for k in 0..32u8 {
        let d = [
            f32::from(k) * 0.37 - 3.0,
            f32::from(k % 5) - 2.0,
            -f32::from(k % 3),
        ];
        let (ha, hb) = (
            a.sweep_capsule(&c, d, &QueryFilter::player()),
            b.sweep_capsule(&c, d, &QueryFilter::player()),
        );
        assert_eq!(
            ha.map(|h| (
                h.fraction.to_bits(),
                h.normal.map(f32::to_bits),
                h.body,
                h.feature
            )),
            hb.map(|h| (
                h.fraction.to_bits(),
                h.normal.map(f32::to_bits),
                h.body,
                h.feature
            ))
        );
    }
}

#[test]
fn unresolvable_objects_report_why_and_get_no_shape() {
    let pids = PhysicsPids::default();
    let missing = resolve_object_collision(
        Did(0x4700_1234),
        |_| Ok(None),
        &tessera_formats::NoSchema,
        &pids,
    );
    assert!(missing.error.is_some() && missing.template.is_none());
    let failing = resolve_object_collision(
        Did(0x4700_1234),
        |_| Err("disk on fire".to_owned()),
        &tessera_formats::NoSchema,
        &pids,
    );
    assert!(failing.error.as_ref().is_some_and(|e| matches!(
        e,
        crate::PhysicsError::Record { did: 0x4700_1234, message, .. } if message.contains("disk on fire")
    )));
    let garbage = resolve_object_collision(
        Did(0x4700_1234),
        |_| {
            Ok(Some(FetchedRecord {
                bytes: vec![0xFF; 7],
                stamp: None,
            }))
        },
        &tessera_formats::NoSchema,
        &pids,
    );
    assert!(garbage.error.is_some() && garbage.template.is_none());
    // A cache keeps one result per object.
    let cache = ObjectCollisionCache::new(1);
    let first = cache.get_or_resolve(Did(1), |o| ObjectCollision {
        object: o,
        ..ObjectCollision::default()
    });
    let again = cache.get_or_resolve(Did(1), |_| unreachable!("cached"));
    assert!(Arc::ptr_eq(&first, &again));
    let _ = cache.get_or_resolve(Did(2), |o| ObjectCollision {
        object: o,
        ..ObjectCollision::default()
    });
    assert_eq!(cache.len(), 1, "bounded");
}

// ------------------------------------------------------------------ regressions (review)

/// Finding: a capsule resting on flat ground was stopped at every internal edge of the
/// coplanar terrain triangles when it slid parallel to the ground.
#[test]
fn a_landed_capsule_slides_across_coplanar_edges() {
    let set = CollisionSet::from_grids(flat_terrain(0), None);
    let filter = QueryFilter::player();
    for start in [[21.0, 22.0], [20.0, 20.0], [3.3, 151.7]] {
        let c = Capsule::vertical([start[0], start[1], 2.0], 1.1, 0.35);
        let land = set
            .sweep_capsule(&c, [0.0, 0.0, -3.0], &filter)
            .expect("lands");
        let rested = c.translated([0.0, 0.0, -3.0 * land.fraction]);
        // 10 m across several quad edges and diagonals, in several directions.
        for d in [
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [7.0, 7.0, 0.0],
            [-6.0, 8.0, 0.0],
            [3.0, -9.0, 0.0],
        ] {
            assert_eq!(
                set.sweep_capsule(&rested, d, &filter),
                None,
                "start {start:?} slide {d:?}"
            );
        }
        // Still blocked when moving down into the ground.
        let down = set
            .sweep_capsule(&rested, [1.0, 0.0, -0.5], &filter)
            .expect("ground");
        assert_eq!(down.fraction, 0.0);
    }
}

/// The same along a wall face: rest against the wall, then slide parallel to it across
/// the face's diagonal.
#[test]
fn a_capsule_resting_against_a_wall_slides_along_it() {
    let set = set_with_wall(CollisionFlags::default());
    let filter = QueryFilter::player();
    let c = Capsule::vertical([45.0, 48.0, 2.5], 0.5, 0.35);
    let hit = set
        .sweep_capsule(&c, [10.0, 0.0, 0.0], &filter)
        .expect("wall");
    let rested = c.translated([10.0 * hit.fraction, 0.0, 0.0]);
    assert_eq!(set.sweep_capsule(&rested, [0.0, 4.0, 0.0], &filter), None);
    assert_eq!(set.sweep_capsule(&rested, [0.0, -4.0, 1.0], &filter), None);
    assert!(
        set.sweep_capsule(&rested, [1.0, 0.0, 0.0], &filter)
            .is_some()
    );
}

/// Finding: a capsule within one skin of the ground (after rounding or depenetration) was
/// blocked at t = 0 by the coplanar neighbour across a nearby edge.
#[test]
fn a_capsule_within_a_skin_of_the_ground_slides_too() {
    let set = CollisionSet::from_grids(flat_terrain(0), None);
    let filter = QueryFilter::player();
    for gap in [0.000_01f32, 0.0001, 0.0005, 0.0008, 0.001, 0.0015] {
        // Feet 3 mm from the x = 25 edge and the diagonal of quad (4, 4).
        let c = player_capsule([24.997, 22.0, gap]);
        for d in [[10.0, 0.0, 0.0], [-10.0, 0.0, 0.0], [5.0, 5.0, 0.0]] {
            assert_eq!(set.sweep_capsule(&c, d, &filter), None, "gap {gap} {d:?}");
        }
        assert!(set.sweep_capsule(&c, [1.0, 0.0, -0.1], &filter).is_some());
    }
}

/// Finding: a triangle pierced by the capsule's core reported its winding normal on either
/// side, and a separation that ignored the depth.
#[test]
fn a_pierced_triangle_pushes_towards_the_core_centre() {
    // One triangle at z = 0, wound clockwise seen from above (winding normal −Z).
    let tri = ShapeTemplate::from_triangle_list(
        &[[0.0, 0.0, 0.0], [0.0, 4.0, 0.0], [4.0, 0.0, 0.0]],
        &[[0, 1, 2]],
    )
    .expect("tri");
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_shape(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &tri,
    )
    .expect("shape");
    let set = b.build();
    for (low, expected) in [
        (0.1f32, -0.25f32),
        (0.01, -0.34),
        (-0.01, -0.36),
        (-0.1, -0.45),
    ] {
        let c = Capsule::vertical([1.0, 1.0, low], 1.1, 0.35);
        let mut out = Vec::new();
        set.contacts(&c, 0.0, &QueryFilter::all(), &mut out);
        assert_eq!(out.len(), 1, "low {low}");
        let k = out[0];
        assert!(
            (k.normal[2] - 1.0).abs() < 1e-6,
            "low {low}: {:?}",
            k.normal
        );
        assert_eq!(k.face_normal, Some([0.0, 0.0, 1.0]));
        assert!(
            (k.separation - expected).abs() < 1e-5,
            "low {low}: {}",
            k.separation
        );
        // Pushing out along the contact clears the triangle.
        let out_c = c.translated(k.normal.map(|n| n * (-k.separation + 0.01)));
        let mut after = Vec::new();
        set.contacts(&out_c, 0.0, &QueryFilter::all(), &mut after);
        assert!(after.is_empty(), "low {low}: {after:?}");
        if low < 0.0 {
            // Pierced: moving out is free, moving further in is blocked.
            assert!(
                set.sweep_capsule(&c, [0.0, 0.0, 1.0], &QueryFilter::all())
                    .is_none()
            );
            let blocked = set
                .sweep_capsule(&c, [0.0, 0.0, -1.0], &QueryFilter::all())
                .expect("blocked");
            assert_eq!(blocked.fraction, 0.0);
        }
    }
}

fn big_box_set(with_terrain: bool) -> CollisionSet {
    let mut t = ShapeTemplate::default();
    t.push_box(&CollisionBox {
        min: [-5.0, -5.0, -5.0],
        max: [5.0, 5.0, 5.0],
        frame: Frame::IDENTITY,
    })
    .expect("box");
    assert_eq!(t.boxes.len(), 1);
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    if with_terrain {
        b.add_grid(flat_terrain(0)).expect("terrain");
    }
    b.add_object(
        BodySource::Other { tag: 7 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &t,
        &Transform::new([50.0, 50.0, 10.0], Quaternion::IDENTITY, 1.0),
    )
    .expect("box");
    b.build()
}

/// Finding: setup boxes were hollow shells; a capsule inside got no contact and was held by
/// the inner faces.
#[test]
fn setup_boxes_are_solid() {
    let set = big_box_set(false);
    assert_eq!(set.solid_box_count(), 1);
    let inside = Capsule::vertical([50.0, 50.0, 9.0], 1.0, 0.3);
    let mut out = Vec::new();
    set.contacts(&inside, 0.0, &QueryFilter::all(), &mut out);
    assert_eq!(out.len(), 1, "{out:?}");
    let k = out[0];
    // The bottom face (−z) is the first of the nearest faces (5 m away for x, y and −z).
    assert_eq!(k.normal, [0.0, 0.0, -1.0]);
    assert!((k.separation + 5.3).abs() < 1e-5, "{}", k.separation);
    assert_eq!((k.kind, k.feature), (FeatureKind::Triangle, 0));
    // Leaving through that face is free; going deeper is blocked at once.
    assert!(
        set.sweep_capsule(&inside, [0.0, 0.0, -100.0], &QueryFilter::all())
            .is_none()
    );
    let deeper = set
        .sweep_capsule(&inside, [0.0, 0.0, 1.0], &QueryFilter::all())
        .expect("blocked");
    assert_eq!(deeper.fraction, 0.0);
    // Near the +x face the push goes that way.
    let near = Capsule::vertical([54.5, 50.0, 9.0], 1.0, 0.3);
    out.clear();
    set.contacts(&near, 0.0, &QueryFilter::all(), &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].normal, [1.0, 0.0, 0.0]);
    assert!((out[0].separation + 0.8).abs() < 1e-5);
    assert_eq!(out[0].feature, 10);
    // From outside the box still stops a capsule at its face.
    let outside = Capsule::vertical([40.0, 50.0, 9.0], 1.0, 0.3);
    let hit = set
        .sweep_capsule(&outside, [10.0, 0.0, 0.0], &QueryFilter::all())
        .expect("face");
    let front = 40.0 + 10.0 * hit.fraction + 0.3;
    assert!(front < 45.0 && front > 45.0 - 0.003, "front {front}");
}

/// Finding: a disc on the east or north edge of the landblock found no triangle.
#[test]
fn disc_query_on_the_far_edges_finds_quad_31() {
    let mesh = flat_terrain(0);
    let east = mesh.keys_in_disc(160.0, 80.0, 0.0);
    assert!(!east.is_empty(), "east edge");
    assert!(east.iter().all(|&k| (k / 2) % 32 == 31), "{east:?}");
    let north = mesh.keys_in_disc(80.0, 160.0, 0.0);
    assert!(
        !north.is_empty() && north.iter().all(|&k| k / 64 == 31),
        "{north:?}"
    );
    assert!(!mesh.keys_in_disc(160.0, 160.0, 0.0).is_empty());
    assert!(!mesh.keys_in_disc(160.0, 80.0, 1e-6).is_empty());
    // Beyond the edge: nothing.
    assert!(mesh.keys_in_disc(160.5, 80.0, 0.0).is_empty());
    assert!(mesh.keys_in_disc(200.0, 80.0, 0.0).is_empty());
}

fn room_volume() -> (Arc<CellVolume>, Vec<[f32; 3]>) {
    let room = Arc::new(
        CellVolume::new([
            ([1.0, 0.0, 0.0], 1.0),
            ([-1.0, 0.0, 0.0], 1.0),
            ([0.0, 1.0, 0.0], 1.0),
            ([0.0, -1.0, 0.0], 1.0),
            ([0.0, 0.0, 1.0], 0.0),
            ([0.0, 0.0, -1.0], 2.0),
        ])
        .expect("volume"),
    );
    let hull = (0..8)
        .map(|k| {
            [
                if k & 1 == 0 { -1.0 } else { 1.0 },
                if k & 2 == 0 { -1.0 } else { 1.0 },
                if k & 4 == 0 { 0.0 } else { 2.0 },
            ]
        })
        .collect();
    (room, hull)
}

/// Findings: cells pushed in descending order cost O(n²), and the lookup scanned every
/// cell. Cells now append, sort once in `finish`, and the lookup goes through a hierarchy;
/// the answers stay the lowest number (the earlier of duplicates).
#[test]
fn cells_in_any_order_give_the_lowest_number() {
    let (room, hull) = room_volume();
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    // 20,000 cells in descending order on a 100 × 100 grid of rooms, two per room.
    for n in (0..20_000u16).rev() {
        let k = n / 2;
        let at = [f32::from(k % 100) * 3.0, f32::from(k / 100) * 3.0, 0.0];
        let placement = CellPlacement::new(at, Quaternion::IDENTITY).expect("placement");
        let cell = PlacedCell::with_volume(
            n,
            Did(0x0100_0000 | u32::from(n)),
            room.clone(),
            &hull,
            placement,
        )
        .expect("cell");
        b.add_cell(cell, None).expect("fits");
    }
    let set = b.build();
    let cells = set.cells().cells();
    assert!(cells.windows(2).all(|w| w[0].index <= w[1].index));
    // Room (7, 3) holds cells 614 and 615.
    assert_eq!(set.cell_at([21.2, 9.5, 1.0]).map(|h| h.cell), Some(614));
    assert!(set.cell_at([22.5, 9.5, 1.0]).is_none());
    assert_eq!(set.cells().get(615).map(|c| c.index), Some(615));
    // Duplicates: the one pushed first wins, before and after finish.
    let mut cs = CellSet::new();
    let p = CellPlacement::new([0.0; 3], Quaternion::IDENTITY).expect("placement");
    for (n, g) in [(5u16, 1u32), (3, 2), (3, 3)] {
        cs.push(PlacedCell::with_volume(n, Did(g), room.clone(), &hull, p).expect("cell"));
    }
    assert_eq!(
        cs.cell_at([0.0, 0.0, 1.0]).map(|c| c.geometry),
        Some(Did(2))
    );
    assert_eq!(cs.get(3).map(|c| c.geometry), Some(Did(2)));
    cs.finish();
    assert_eq!(
        cs.cell_at([0.0, 0.0, 1.0]).map(|c| c.geometry),
        Some(Did(2))
    );
    assert_eq!(cs.get(3).map(|c| c.geometry), Some(Did(2)));
    assert_eq!(cs.cells()[0].index, 3);
}

/// Finding: one hostile volume shared by many cells made every lookup cost cells × planes.
#[test]
fn cell_plane_budgets_are_enforced() {
    let too_many = (0..=MAX_CELL_PLANES).map(|_| ([0.0, 0.0, 1.0], 1.0));
    assert!(matches!(
        CellVolume::new(too_many),
        Err(crate::PhysicsError::TooLarge { .. })
    ));
    let big = Arc::new(
        CellVolume::new((0..MAX_CELL_PLANES).map(|_| ([0.0, 0.0, 1.0], 1.0))).expect("volume"),
    );
    let (_, hull) = room_volume();
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let p = CellPlacement::new([0.0; 3], Quaternion::IDENTITY).expect("placement");
    let fit = MAX_SET_CELL_PLANES / MAX_CELL_PLANES;
    for n in 0..fit {
        let n = u16::try_from(n).expect("small");
        b.add_cell(
            PlacedCell::with_volume(n, Did(1), big.clone(), &hull, p).expect("cell"),
            None,
        )
        .expect("within the budget");
    }
    assert!(matches!(
        b.add_cell(
            PlacedCell::with_volume(9999, Did(1), big, &hull, p).expect("cell"),
            None
        ),
        Err(crate::PhysicsError::TooLarge {
            what: "cell planes",
            ..
        })
    ));
    assert!(
        PlacedCell::with_volume(
            0,
            Did(1),
            room_volume().0,
            &vec![[0.0; 3]; MAX_CELL_HULL + 1],
            p
        )
        .is_err()
    );
}

/// Finding (§12.1): `add_cell` and `add_cell_with_flags` never checked or charged the
/// per-set hull budget; only `add_cells` did.
#[test]
fn direct_cell_adds_charge_the_hull_budget() {
    let (room, _) = room_volume();
    let hull = vec![[0.5_f32, 0.5, 1.0]; MAX_CELL_HULL];
    let p = CellPlacement::new([0.0; 3], Quaternion::IDENTITY).expect("placement");
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let fit = MAX_SET_CELL_HULL / MAX_CELL_HULL;
    for n in 0..fit {
        let n = u16::try_from(n).expect("small");
        let cell = PlacedCell::with_volume(n, Did(1), room.clone(), &hull, p).expect("cell");
        assert_eq!(cell.hull_count(), MAX_CELL_HULL);
        b.add_cell(cell, None).expect("within the budget");
    }
    let over = PlacedCell::with_volume(9999, Did(1), room.clone(), &hull, p).expect("cell");
    assert!(matches!(
        b.add_cell(over, None),
        Err(crate::PhysicsError::TooLarge {
            what: "cell hull vertices",
            ..
        })
    ));
    assert_eq!(b.report().cells as usize, fit);
    // A cell without hull vertices still fits.
    let empty = PlacedCell::with_volume(10_000, Did(1), room, &[], p).expect("cell");
    b.add_cell(empty, None).expect("no hull vertices");
}

/// Finding (§5.3 "Flags of the cell body"): the cell body got every §9.1 field of the
/// cell record's property set, so ethereality, `AlwaysUncontrolledSurface` or `NoPhysics`
/// could make it pass-through, no support, or (not) solid. It carries only
/// `Physics_SurfacePropertiesToApply` and `Physics_CanLedgeClimb`.
#[test]
fn cell_body_carries_only_the_two_cell_flags() {
    let (room, hull) = room_volume();
    let p = CellPlacement::new([0.0; 3], Quaternion::IDENTITY).expect("placement");
    let flags = CollisionFlags {
        no_physics: Some(true),
        ethereality: Some(ETHEREAL_PLAYER),
        ethereal_to: Some(ETHEREAL_PLAYER | ETHEREAL_CAMERA),
        placement_ethereal_to: Some(ETHEREAL_PLAYER),
        always_uncontrolled_surface: Some(true),
        surface_properties: Some(Did(0x7800_0001)),
        can_ledge_climb: Some(false),
        creation_type: Some(7),
    };
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let cell = PlacedCell::with_volume(1, Did(0x0100_0001), room, &hull, p).expect("cell");
    let id = b
        .add_cell_with_flags(cell, Some(&wall_template()), flags)
        .expect("fits")
        .expect("a body");
    let set = b.build();
    let body = set.body(id).expect("body");
    assert_eq!(
        body.flags,
        CollisionFlags {
            surface_properties: Some(Did(0x7800_0001)),
            can_ledge_climb: Some(false),
            ..CollisionFlags::default()
        }
    );
    assert!(QueryFilter::player().accepts(body));
    assert!(QueryFilter::camera().accepts(body));
    assert!(!body.flags.is_always_uncontrolled());
}

/// Finding (§5.3 implementer OPEN, §12.1): a cell whose physics mesh hit a set-level limit
/// was dropped from the cell lookup too, unlike a mesh over the template bound. Both now
/// refuse only the body and place the cell.
#[test]
fn cell_mesh_over_a_set_limit_still_places_the_cell() {
    let (room, hull) = room_volume();
    let p = CellPlacement::new([0.0; 3], Quaternion::IDENTITY).expect("placement");
    for limits in [
        CollisionLimits {
            max_bodies: 0,
            ..CollisionLimits::default()
        },
        CollisionLimits {
            max_triangles: 1,
            ..CollisionLimits::default()
        },
        CollisionLimits {
            max_rounds: 0,
            ..CollisionLimits::default()
        },
    ] {
        let mut b = CollisionSetBuilder::new(limits);
        let cell =
            PlacedCell::with_volume(4, Did(0x0100_0004), room.clone(), &hull, p).expect("cell");
        let body = b
            .add_cell(cell, Some(&wall_template()))
            .expect("the cell is placed");
        assert_eq!(body, None, "{limits:?}");
        let r = *b.report();
        assert_eq!(
            (r.cells, r.over_limit, r.cells_refused),
            (1, 1, 0),
            "{limits:?}"
        );
        let set = b.build();
        assert_eq!(set.cell_at([0.0, 0.0, 1.0]).map(|h| h.cell), Some(4));
        assert_eq!(set.bodies().len(), 0);
    }
}

/// Finding: a full set still placed (allocated and transformed) every further object
/// before refusing it. The limits are now checked first: even an invalid transform reports
/// the limit, because nothing is placed.
#[test]
fn full_sets_refuse_objects_before_placing_them() {
    let mut b = CollisionSetBuilder::new(CollisionLimits {
        max_triangles: 12,
        ..CollisionLimits::default()
    });
    let wall = wall_template();
    b.add_object(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &wall,
        &Transform::IDENTITY,
    )
    .expect("fits");
    let bad = Transform::new([f32::NAN, 0.0, 0.0], Quaternion::IDENTITY, 1.0);
    assert!(matches!(
        b.add_object(
            BodySource::Other { tag: 1 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &wall,
            &bad,
        ),
        Err(crate::PhysicsError::TooLarge {
            what: "triangles",
            ..
        })
    ));
    // Solid boxes count as round primitives: the wall has no sphere or capsule, so only
    // its box can exceed a round limit of 0 (with a valid transform, nothing else fails).
    assert!(wall.spheres.is_empty() && wall.capsules.is_empty() && wall.boxes.len() == 1);
    let mut b = CollisionSetBuilder::new(CollisionLimits {
        max_rounds: 0,
        ..CollisionLimits::default()
    });
    assert!(matches!(
        b.add_object(
            BodySource::Other { tag: 0 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &wall,
            &Transform::IDENTITY,
        ),
        Err(crate::PhysicsError::TooLarge {
            what: "round primitives",
            count: 1,
            limit: 0
        })
    ));
    assert!(b.build().bodies().is_empty());
}

/// Finding: geometry stacked at one spot made one query visit (and report) millions of
/// items. A query now stops at `MAX_QUERY_ITEMS` per set.
#[test]
fn queries_visit_a_bounded_number_of_items() {
    let n = MAX_QUERY_ITEMS + 1000;
    let positions = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
    let tris = vec![[0u16, 1, 2]; n];
    let t = ShapeTemplate::from_triangle_list(&positions, &tris).expect("template");
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_shape(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &t,
    )
    .expect("shape");
    let set = b.build();
    let c = Capsule::vertical([0.0, 0.0, 0.1], 1.0, 0.3);
    let mut out = Vec::new();
    set.contacts(&c, 0.0, &QueryFilter::all(), &mut out);
    assert_eq!(out.len(), MAX_QUERY_ITEMS);
    let hit = set
        .sweep_capsule(
            &Capsule::vertical([0.0, 0.0, 2.0], 1.0, 0.3),
            [0.0, 0.0, -3.0],
            &QueryFilter::all(),
        )
        .expect("hit");
    assert!(hit.fraction > 0.5);
}

/// Finding: sets carried no water and were not keyed by landblock; views took free f32
/// offsets. Decision 0013 "Positions" and "Water".
#[test]
fn sets_carry_their_landblock_and_water() {
    use tessera_formats::{LandblockId, LandblockKind};
    let id = |x: u8, y: u8| LandblockId {
        kind: LandblockKind::Info,
        region: 1,
        x,
        y,
    };
    let wet = |x: u8, y: u8, level: f32| {
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.add_grid(flat_terrain(0)).expect("terrain");
        b.set_landblock(id(x, y));
        b.set_water_plane(Some(WaterPlane::new(level, 4).expect("plane")));
        // A deep box turned by 90° and scaled by 2: entity box x 0–10, y 0–5, z −8–0.
        let deep = LiquidBox::new(
            [0.0, 0.0, -4.0],
            [5.0, 2.5, 0.0],
            &Transform::new(
                [100.0, 100.0, level],
                Quaternion::from_yaw(std::f32::consts::FRAC_PI_2),
                2.0,
            ),
            LiquidFlags {
                liquid: true,
                deep: true,
                ..LiquidFlags::default()
            },
        )
        .expect("box");
        b.add_liquid_box(deep).expect("fits");
        b.build()
    };
    let a = wet(10, 20, 5.0);
    let east = wet(11, 20, 7.0);
    assert_eq!(a.landblock(), Some(id(10, 20)));
    assert_eq!(
        a.water_plane().map(|w| (w.level, w.water_type)),
        Some((5.0, 4))
    );
    assert!(
        a.water_plane()
            .is_some_and(|w| w.is_under(4.9) && !w.is_under(4.9999))
    );
    // Turned by 90°: the box spans x 95–100, y 100–110 in the landblock frame.
    assert!(a.liquid_at([97.0, 105.0, 0.0]).deep);
    assert!(a.liquid_at([97.0, 105.0, 0.0]).liquid);
    assert!(a.liquid_at([105.0, 97.0, 0.0]).is_empty());
    assert!(a.liquid_at([97.0, 105.0, -3.1]).is_empty());
    assert!(a.liquid_at([97.0, 105.0, 5.5]).is_empty());
    // A view around the mover's landblock, in the region frame.
    let origin = landblock_origin(&id(10, 20));
    assert_eq!(origin, [1600.0, 3200.0, 0.0]);
    let mut view = CollisionView::with_origin(origin);
    view.push_landblock(&east).expect("east");
    view.push_landblock(&a).expect("a");
    // Ascending landblock order whatever the push order.
    assert_eq!(view.members()[0].1, [0.0, 0.0, 0.0]);
    assert_eq!(view.members()[1].1, [160.0, 0.0, 0.0]);
    let mover = [1600.0 + 160.0 + 97.0, 3200.0 + 105.0, 1.0];
    let p = view.to_view(mover);
    assert_eq!(p, [257.0, 105.0, 1.0]);
    assert_eq!(view.to_region(p), mover);
    assert!(view.liquid_at(p).deep);
    assert_eq!(view.water_plane_at(p[0], p[1]).map(|w| w.level), Some(7.0));
    assert_eq!(view.water_plane_at(10.0, 10.0).map(|w| w.level), Some(5.0));
    assert!(view.water_plane_at(-10.0, 10.0).is_none());
    // The ground of the east set seen from the view.
    let c = Capsule::vertical([200.0, 10.0, 2.0], 1.0, 0.3);
    let hit = view
        .sweep_capsule(&c, [0.0, 0.0, -3.0], &QueryFilter::player())
        .expect("ground");
    assert_eq!(hit.set, 1);
    // Refused: unkeyed sets and other regions.
    let plain = CollisionSet::from_grids(flat_terrain(0), None);
    assert!(view.push_landblock(&plain).is_err());
    let mut other = CollisionSetBuilder::new(CollisionLimits::default());
    other.set_landblock(LandblockId {
        region: 2,
        ..id(10, 21)
    });
    let other = other.build();
    assert!(view.push_landblock(&other).is_err());
    // Hostile water input.
    assert!(WaterPlane::new(f32::NAN, 0).is_err());
    assert!(
        LiquidBox::new(
            [1.0; 3],
            [0.0; 3],
            &Transform::IDENTITY,
            LiquidFlags::default()
        )
        .is_err()
    );
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let one = LiquidBox::new(
        [0.0; 3],
        [1.0; 3],
        &Transform::IDENTITY,
        LiquidFlags::default(),
    )
    .expect("box");
    for _ in 0..MAX_LIQUID_BOXES {
        b.add_liquid_box(one).expect("fits");
    }
    assert!(b.add_liquid_box(one).is_err());
}

/// Finding: the cache was bounded by entries only.
#[test]
fn the_collision_cache_is_bounded_by_bytes() {
    let big = |o: Did| ObjectCollision {
        object: o,
        template: Some(Arc::new(ShapeTemplate {
            triangles: vec![[[0.0; 3]; 3]; 1000],
            ..ShapeTemplate::default()
        })),
        ..ObjectCollision::default()
    };
    let one = big(Did(0)).heap_size();
    let cache = ObjectCollisionCache::with_limits(100, 3 * one);
    for k in 0..3 {
        let _ = cache.get_or_resolve(Did(k), big);
    }
    assert_eq!(cache.len(), 3);
    assert!(cache.bytes() <= 3 * one);
    let _ = cache.get_or_resolve(Did(3), big);
    assert_eq!(cache.len(), 1, "cleared when the bytes would be exceeded");
    // A result larger than the whole bound is returned but not cached.
    let small = ObjectCollisionCache::with_limits(100, one / 2);
    let r = small.get_or_resolve(Did(9), big);
    assert!(r.template.is_some());
    assert!(small.is_empty());
}

/// The same across a finely tessellated floor mesh and across a landblock boundary in a
/// view (the clearance is taken over every set).
#[test]
fn slides_cross_fine_meshes_and_landblock_edges() {
    // A 20 × 20 m floor of 0.25 m quads at z = 1, as one 0x66-like body.
    let n = 80u16;
    let positions: Vec<[f32; 3]> = (0..=n)
        .flat_map(|j| (0..=n).map(move |i| [f32::from(i) * 0.25, f32::from(j) * 0.25, 1.0]))
        .collect();
    let at = |i: u16, j: u16| j * (n + 1) + i;
    let tris: Vec<[u16; 3]> = (0..n)
        .flat_map(|j| (0..n).map(move |i| (i, j)))
        .flat_map(|(i, j)| {
            [
                [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
                [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
            ]
        })
        .collect();
    let floor = ShapeTemplate::from_triangle_list(&positions, &tris).expect("floor");
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_object(
        BodySource::Other { tag: 0 },
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &floor,
        &Transform::new([10.0, 10.0, 0.0], Quaternion::IDENTITY, 1.0),
    )
    .expect("floor");
    let set = b.build();
    let c = Capsule::vertical([12.1, 12.2, 3.0], 1.1, 0.35);
    let land = set
        .sweep_capsule(&c, [0.0, 0.0, -3.0], &QueryFilter::player())
        .expect("lands");
    let rested = c.translated([0.0, 0.0, -3.0 * land.fraction]);
    for d in [[15.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.3, 14.0, 0.0]] {
        assert_eq!(
            set.sweep_capsule(&rested, d, &QueryFilter::player()),
            None,
            "{d:?}"
        );
    }
    // Two flat landblocks side by side; a capsule half a skin above the ground near the
    // shared edge walks across it.
    let a = CollisionSet::from_grids(flat_terrain(0), None);
    let east = CollisionSet::from_grids(flat_terrain(0), None);
    let mut view = CollisionView::new();
    view.push(&a, [0.0; 3]);
    view.push(&east, [160.0, 0.0, 0.0]);
    for gap in [0.0005f32, 0.0015, 0.0022] {
        let c = player_capsule([159.5, 80.3, gap]);
        assert_eq!(
            view.sweep_capsule(&c, [10.0, 0.0, 0.0], &QueryFilter::player()),
            None,
            "gap {gap}"
        );
    }
}
