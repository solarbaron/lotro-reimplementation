// SPDX-License-Identifier: GPL-3.0-or-later
//! Entity descriptors (0x47), visual templates (0x1F), setups (0x04), appearance records
//! (0x20) and chain resolution (`docs/specs/setup.md` §3–§9) on synthetic records. Every
//! fixture is built here; none comes from game data.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::rc::Rc;

use common::B;
use proptest::prelude::*;
use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceExtra, AppearancePart, AppearanceRecord,
    MaterialGroup,
};
use tessera_formats::setup::{
    Aabb, BoneFrame, CollisionBox, CollisionCapsule, CollisionSphere, EntityBlock,
    EntityDescriptor, EntityHeader, Frame, MAX_CHAIN_HOPS, NamedFrame, PhysicsBounds, Placement,
    Quaternion, SetupSection, SetupSectionEntry, TAGFILE_MAGIC, TemplateAppearance, TemplateLink,
    VisualTemplate,
};
use tessera_formats::{
    AnyRecord, Did, DidKind, Evidence, FormatError, NoSchema, PropertyType, PropertyValue, Record,
    ResolveError, Setup, TSize, decode_appearance, decode_entity_descriptor, decode_entity_header,
    decode_setup, decode_visual_template, resolve_setup, resolve_setup_did,
};

const INT: u32 = 0x1000_0BBB;

fn schema() -> HashMap<u32, PropertyType> {
    HashMap::from([(INT, PropertyType::Int)])
}

fn frame(seed: f32) -> Frame {
    Frame {
        position: [seed, seed + 1.0, seed + 2.0],
        scale: [1.0, 1.0, 1.0],
        rotation: Quaternion::IDENTITY,
    }
}

fn tagfile() -> Vec<u8> {
    let mut t = TAGFILE_MAGIC.to_vec();
    t.extend_from_slice(&[1, 2, 3, 4]);
    t
}

/// A setup that uses every field.
fn full_setup() -> Setup {
    Setup {
        did: Did(0x0400_0123),
        tagfile: tagfile(),
        attach_map_size: TSize { hint: 0, count: 2 },
        attach_map: vec![(0x1000_0001, 3), (5, 7)],
        bone_frames_hint: 1,
        bone_frames: vec![BoneFrame {
            key: 0x1000_0010,
            key2: 0x1000_0010,
            bone: -1,
            frame: frame(1.0),
        }],
        placements: vec![
            Placement {
                id: 0,
                frame: frame(2.0),
            },
            Placement {
                id: 0,
                frame: frame(3.0),
            },
        ],
        named_frames_hint: 0,
        named_frames: vec![NamedFrame {
            key: 9,
            key2: 9,
            frame: frame(4.0),
        }],
        boxes: vec![CollisionBox {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 2.0],
            frame: Frame {
                position: [0.0, 0.0, 1.0],
                ..Frame::IDENTITY
            },
        }],
        spheres: vec![CollisionSphere {
            centre: [0.0, 0.0, 1.0],
            radius: 0.5,
        }],
        capsules: vec![CollisionCapsule {
            base: [0.0, 0.0, 0.5],
            length: 1.0,
            radius: 0.5,
        }],
        collision_mesh: Did(0x6600_0042),
        physics_bounds: PhysicsBounds {
            aabb: Aabb {
                min: [-1.0, -1.0, 0.0],
                max: [1.0, 1.0, 3.0],
            },
            height: 3.0,
            radius: 1.5,
        },
        parts: vec![Did(0x0600_0001), Did(0x0600_0002)],
    }
}

/// The smallest setup: empty lists, no collision, one part.
fn minimal_setup_bytes(did: u32, part: u32) -> Vec<u8> {
    B::new()
        .u32(did)
        .u32(8)
        .bytes(&TAGFILE_MAGIC)
        .tsize(0, 0) // attach map
        .u32(0) // list A
        .u32(0) // list B
        .u32(0) // list C
        .u8(0) // boxes
        .u8(0) // spheres
        .u8(0) // capsules
        .u32(0) // collision mesh
        .u8(0) // reserved
        .bytes(&[0; 32]) // physics bounds
        .u8(1)
        .u32(part)
        .build()
}

// ------------------------------------------------------------------------- setups

#[test]
fn minimal_setup_decodes() {
    let data = minimal_setup_bytes(0x0400_0001, 0x0600_0007);
    let s = Setup::parse_expecting(&data, 0x0400_0001u32).unwrap();
    assert_eq!(s.parts, vec![Did(0x0600_0007)]);
    assert!(s.tagfile_has_magic());
    assert!(!s.has_collision());
    assert!(s.physics_bounds.is_zero());
    assert_eq!(s.first_unusual_field(), None);
    assert_eq!(s.volume_bounds(), None);
    assert_eq!(s.to_bytes().unwrap(), data);
    assert!(matches!(
        Setup::parse_expecting(&data, 0x0400_0002u32),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn full_setup_round_trips() {
    let s = full_setup();
    let data = s.to_bytes().unwrap();
    let back = decode_setup(&data).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.first_unusual_field(), None);
    // Union of the box (moved up by 1), sphere and capsule.
    let v = back.volume_bounds().unwrap();
    assert!(v.approx_eq(
        Aabb {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 3.0]
        },
        1e-6
    ));
    // Every strict prefix is truncated, never a panic.
    for n in 0..data.len() {
        assert!(Setup::parse(&data[..n]).is_err(), "prefix {n}");
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        Setup::parse(&long),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
}

#[test]
fn setup_rejects_bad_framing() {
    // Wrong prefix.
    let data = minimal_setup_bytes(0x0100_0001, 0x0600_0007);
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
    // Non-zero reserved byte.
    let mut data = minimal_setup_bytes(0x0400_0001, 0x0600_0007);
    let reserved = data.len() - 4 - 1 - 32 - 1;
    data[reserved] = 1;
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::Unsupported { value: 1, .. })
    ));
    // Frame list B whose header disagrees with its count.
    let data = B::new()
        .u32(0x0400_0001)
        .u32(0)
        .tsize(0, 0)
        .u32(0)
        .u32(1)
        .u32(2)
        .bytes(&[0; 44])
        .build();
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::CountMismatch {
            expected: 1,
            found: 2,
            ..
        })
    ));
    // Frame list A whose TSize header disagrees.
    let data = B::new()
        .u32(0x0400_0001)
        .u32(0)
        .tsize(0, 0)
        .u32(2)
        .tsize(0, 1)
        .bytes(&[0; 104])
        .build();
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::CountMismatch { .. })
    ));
    // Hostile lengths and counts fail before allocating.
    let data = B::new().u32(0x0400_0001).u32(u32::MAX).build();
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::Truncated { .. })
    ));
    let data = B::new()
        .u32(0x0400_0001)
        .u32(0)
        .tsize(0, 0x3FFF_FFFF)
        .build();
    assert!(matches!(
        Setup::parse(&data),
        Err(FormatError::Truncated { .. })
    ));
    let data = B::new()
        .u32(0x0400_0001)
        .u32(0)
        .tsize(0, 0)
        .u32(u32::MAX)
        .tsize(0, 0x3FFF_FFFF)
        .build();
    assert!(Setup::parse(&data).is_err());
}

#[test]
fn setup_unusual_fields_are_reported() {
    let mut s = full_setup();
    s.tagfile = vec![1, 2, 3];
    assert!(s.first_unusual_field().is_some());
    let mut s = full_setup();
    s.parts.push(Did(0x0100_0001));
    assert!(s.first_unusual_field().is_some());
    let mut s = full_setup();
    s.bone_frames[0].key2 = 1;
    assert!(s.first_unusual_field().is_some());
    let mut s = full_setup();
    s.parts = vec![Did(0); 256];
    assert!(s.to_bytes().is_err());
}

fn rotated_box(min: [f32; 3], max: [f32; 3], position: [f32; 3], q: [f32; 4]) -> CollisionBox {
    CollisionBox {
        min,
        max,
        frame: Frame {
            position,
            rotation: Quaternion {
                w: q[0],
                x: q[1],
                y: q[2],
                z: q[3],
            },
            ..Frame::IDENTITY
        },
    }
}

#[test]
fn quaternion_order_and_rotation_direction() {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    // +90° about +Z, stored (w, x, y, z).
    let q = Quaternion {
        w: s,
        x: 0.0,
        y: 0.0,
        z: s,
    };
    assert_eq!(q.to_wxyz(), [s, 0.0, 0.0, s]);
    assert_eq!(q.to_xyzw(), [0.0, 0.0, s, s]);
    assert_eq!(Quaternion::IDENTITY.to_xyzw(), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(Quaternion::default(), Quaternion::IDENTITY);
    assert_eq!(Frame::default(), Frame::IDENTITY);
    assert_eq!(EntityBlock::default().rotation, Quaternion::IDENTITY);
    // Active rotation: +X goes to +Y.
    let v = q.rotate([1.0, 0.0, 0.0]);
    assert!((v[0]).abs() < 1e-6 && (v[1] - 1.0).abs() < 1e-6 && v[2].abs() < 1e-6);
    assert_eq!(
        Quaternion::IDENTITY.rotate([1.0, 2.0, 3.0]),
        [1.0, 2.0, 3.0]
    );
    // Normalisation: a scaled quaternion is the same rotation; degenerate ones are refused.
    let n = Quaternion {
        w: 2.0 * s,
        x: 0.0,
        y: 0.0,
        z: 2.0 * s,
    }
    .normalized()
    .unwrap();
    assert!((n.w - s).abs() < 1e-6 && (n.z - s).abs() < 1e-6);
    let zero = Quaternion {
        w: 0.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    assert_eq!(zero.normalized(), None);
    assert_eq!(
        Quaternion {
            w: f32::NAN,
            ..Quaternion::IDENTITY
        }
        .normalized(),
        None
    );
}

#[test]
fn box_bounds_apply_rotation_then_position() {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    // Unrotated: corners moved by the position.
    let b = rotated_box(
        [0.0; 3],
        [2.0, 1.0, 1.0],
        [10.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
    );
    assert_eq!(
        b.aabb(),
        Some(Aabb {
            min: [10.0, 0.0, 0.0],
            max: [12.0, 1.0, 1.0]
        })
    );
    // The negated identity is the same rotation.
    let b = rotated_box([0.0; 3], [2.0, 1.0, 1.0], [0.0; 3], [-1.0, 0.0, 0.0, 0.0]);
    assert_eq!(b.aabb().unwrap().max, [2.0, 1.0, 1.0]);
    // +90° about +Z: x' = −y, y' = x; then moved by the position.
    let b = rotated_box(
        [0.0; 3],
        [2.0, 1.0, 1.0],
        [10.0, 0.0, 5.0],
        [s, 0.0, 0.0, s],
    );
    assert!(b.aabb().unwrap().approx_eq(
        Aabb {
            min: [9.0, 0.0, 5.0],
            max: [10.0, 2.0, 6.0]
        },
        1e-5
    ));
    // 180° about +Z, stored (0, 0, 0, 1).
    let b = rotated_box([0.0; 3], [2.0, 1.0, 1.0], [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    assert!(b.aabb().unwrap().approx_eq(
        Aabb {
            min: [-2.0, -1.0, 0.0],
            max: [0.0, 0.0, 1.0]
        },
        1e-6
    ));
    // 45° about +Z: a unit cube centred on the origin widens to ±√2/2.
    let h = (std::f32::consts::FRAC_PI_8).cos();
    let k = (std::f32::consts::FRAC_PI_8).sin();
    let b = rotated_box([-0.5; 3], [0.5; 3], [0.0; 3], [h, 0.0, 0.0, k]);
    let v = b.aabb().unwrap();
    assert!((v.max[0] - s).abs() < 1e-5 && (v.min[1] + s).abs() < 1e-5);
    assert!((v.max[2] - 0.5).abs() < 1e-6);
    // A setup with a rotated box now has volume bounds.
    let mut setup = full_setup();
    setup.boxes = vec![rotated_box(
        [0.0; 3],
        [2.0, 1.0, 1.0],
        [0.0; 3],
        [0.0, 0.0, 0.0, 1.0],
    )];
    setup.spheres.clear();
    setup.capsules.clear();
    assert!(setup.volume_bounds().unwrap().approx_eq(
        Aabb {
            min: [-2.0, -1.0, 0.0],
            max: [0.0, 0.0, 1.0]
        },
        1e-6
    ));
}

#[test]
fn box_bounds_refuse_unconfirmed_or_invalid_frames() {
    let id = [1.0, 0.0, 0.0, 0.0];
    // Scale is not applied (unconfirmed), so a scaled box has no bounds.
    let mut b = rotated_box([0.0; 3], [1.0; 3], [0.0; 3], id);
    b.frame.scale = [2.0; 3];
    assert_eq!(b.aabb(), None);
    // Inverted local box, non-finite corner, position or rotation, zero rotation.
    assert_eq!(rotated_box([1.0; 3], [0.0; 3], [0.0; 3], id).aabb(), None);
    assert_eq!(
        rotated_box([f32::NAN; 3], [1.0; 3], [0.0; 3], id).aabb(),
        None
    );
    assert_eq!(
        rotated_box([0.0; 3], [f32::INFINITY; 3], [0.0; 3], id).aabb(),
        None
    );
    assert_eq!(
        rotated_box([0.0; 3], [1.0; 3], [f32::NAN; 3], id).aabb(),
        None
    );
    assert_eq!(
        rotated_box([0.0; 3], [1.0; 3], [0.0; 3], [f32::NAN, 0.0, 0.0, 0.0]).aabb(),
        None
    );
    assert_eq!(
        rotated_box([0.0; 3], [1.0; 3], [0.0; 3], [0.0; 4]).aabb(),
        None
    );
    // Huge but finite values that overflow to infinity once moved.
    assert_eq!(
        rotated_box([0.0; 3], [f32::MAX; 3], [f32::MAX; 3], id).aabb(),
        None
    );
}

#[test]
fn hostile_volumes_give_no_bounds() {
    let c = CollisionCapsule {
        base: [0.0, 0.0, 1.0],
        length: 2.0,
        radius: 0.5,
    };
    assert_eq!(c.aabb().unwrap().max[2], 3.5);
    assert_eq!(c.aabb().unwrap().min[2], 0.5);
    // The three cases a review found accepted: negative radius, infinite radius, NaN
    // capsule length. Each survives a round trip through the encoder and decoder.
    let cases: [(Vec<CollisionSphere>, Vec<CollisionCapsule>); 4] = [
        (
            vec![CollisionSphere {
                centre: [0.0; 3],
                radius: -5.0,
            }],
            vec![],
        ),
        (
            vec![CollisionSphere {
                centre: [0.0; 3],
                radius: f32::INFINITY,
            }],
            vec![],
        ),
        (
            vec![],
            vec![CollisionCapsule {
                base: [0.0; 3],
                length: f32::NAN,
                radius: 1.0,
            }],
        ),
        (
            vec![],
            vec![CollisionCapsule {
                base: [0.0; 3],
                length: -1.0,
                radius: 1.0,
            }],
        ),
    ];
    for (spheres, capsules) in cases {
        let mut s = full_setup();
        s.boxes.clear();
        s.spheres = spheres;
        s.capsules = capsules;
        let back = Setup::parse(&s.to_bytes().unwrap()).unwrap();
        assert_eq!(back.volume_bounds(), None);
        assert!(back.first_unusual_field().is_some());
        // Order does not matter: a valid volume first or last gives the same answer.
        let mut s2 = back.clone();
        s2.spheres.insert(
            0,
            CollisionSphere {
                centre: [0.0; 3],
                radius: 1.0,
            },
        );
        assert_eq!(s2.volume_bounds(), None);
        s2.spheres.rotate_left(1);
        assert_eq!(s2.volume_bounds(), None);
    }
    // Non-finite stored physics bounds are reported too.
    let mut s = full_setup();
    s.physics_bounds.aabb.max[0] = f32::NAN;
    assert!(s.first_unusual_field().is_some());
    let mut s = full_setup();
    s.physics_bounds.aabb.min = [5.0; 3];
    assert!(s.first_unusual_field().is_some());
    // Aabb helpers.
    assert!(
        !Aabb {
            min: [1.0; 3],
            max: [0.0; 3]
        }
        .is_valid()
    );
    assert!(Aabb::default().is_valid());
    assert_eq!(Aabb::from_points([]), None);
    assert_eq!(Aabb::from_points([[0.0, f32::NAN, 0.0]]), None);
}

// ------------------------------------------------------------------ visual templates

fn template(flags: u32) -> VisualTemplate {
    let bit = |b: u32| flags & b != 0;
    VisualTemplate {
        flags,
        did: Did(0x1F00_0010),
        parent: bit(0x002).then_some(Did(0x1F00_0001)),
        vec3: bit(0x020).then_some([0.3, 0.3, 0.3]),
        value_a: bit(0x040).then_some(1.5),
        value_b: bit(0x400).then_some(2.5),
        appearance: bit(0x100).then(|| TemplateAppearance {
            appearance: Did(0x2000_0005),
            modifiers_size: TSize { hint: 0, count: 2 },
            modifiers: vec![(0x1000_0034, 1.0), (0x1000_0035, 0.5)],
        }),
        setup_section: bit(0x200).then(|| SetupSection {
            size: TSize { hint: 0, count: 1 },
            entries: vec![SetupSectionEntry {
                key: 1,
                kind: 9,
                dids: vec![Did(0x0400_0123)],
            }],
        }),
    }
}

#[test]
fn template_flag_combinations_round_trip() {
    // Every combination of the census (§5.2), plus a few unobserved but legal ones.
    for flags in [
        0x201, 0x103, 0x601, 0x301, 0x641, 0x003, 0x241, 0x221, 0x261, 0x001, 0x763,
    ] {
        let t = template(flags);
        let data = t.to_bytes();
        let back = VisualTemplate::parse_expecting(&data, 0x1F00_0010u32).unwrap();
        assert_eq!(back, t, "{flags:#x}");
        for n in 0..data.len() {
            assert!(VisualTemplate::parse(&data[..n]).is_err());
        }
    }
    let t = decode_visual_template(&template(0x201).to_bytes()).unwrap();
    assert_eq!(t.setup(), Some(Did(0x0400_0123)));
    assert_eq!(t.first_unusual_field(), None);
    let t = template(0x103);
    assert_eq!(t.setup(), None);
    assert_eq!(t.first_unusual_field(), None);
    assert!(template(0x001).first_unusual_field().is_some());
    assert!(template(0x203).first_unusual_field().is_some());
}

#[test]
fn template_encoding_derives_flags_from_fields() {
    // A parent with its bit clear is still written (and the bit set), not dropped.
    let mut t = template(0x201);
    t.parent = Some(Did(0x1F00_0002));
    assert_eq!(t.encoded_flags(), 0x203);
    let back = VisualTemplate::parse(&t.to_bytes()).unwrap();
    assert_eq!(back.parent, Some(Did(0x1F00_0002)));
    assert_eq!(back.flags, 0x203);
    // A bit set without its field writes nothing for it (no invented DID 0).
    let mut t = template(0x763);
    t.parent = None;
    t.vec3 = None;
    t.appearance = None;
    let data = t.to_bytes();
    let back = VisualTemplate::parse(&data).unwrap();
    assert_eq!(back.flags, 0x641);
    assert_eq!(back.parent, None);
    assert_eq!(back.vec3, None);
    assert_eq!(back.appearance, None);
    assert_eq!(back.value_a, t.value_a);
    assert_eq!(back.setup(), t.setup());
    assert_eq!(back.to_bytes(), data);
    // Stale TSize counts are ignored: counts come from the lists.
    let mut t = template(0x301);
    t.setup_section.as_mut().unwrap().size.count = 7;
    t.appearance.as_mut().unwrap().modifiers_size.count = 9;
    let back = VisualTemplate::parse(&t.to_bytes()).unwrap();
    assert_eq!(back.setup_section.unwrap().size.count, 1);
    assert_eq!(back.appearance.unwrap().modifiers_size.count, 2);
}

#[test]
fn template_stream_order_follows_the_table() {
    // flags 0x641: DID, value_a, value_b, setup section.
    let data = B::new()
        .u32(0x641)
        .u32(0x1F00_0002)
        .f32(1.25)
        .f32(2.5)
        .tsize(0, 1)
        .u32(1)
        .u32(9)
        .u32(1)
        .u32(0x0400_0007)
        .build();
    let t = VisualTemplate::parse(&data).unwrap();
    assert_eq!(t.value_a, Some(1.25));
    assert_eq!(t.value_b, Some(2.5));
    assert_eq!(t.setup(), Some(Did(0x0400_0007)));
    assert_eq!(t.to_bytes(), data);
}

#[test]
fn template_rejects_unsupported_layouts() {
    let unsupported = |data: Vec<u8>| {
        assert!(
            matches!(
                VisualTemplate::parse(&data),
                Err(FormatError::Unsupported { .. })
            ),
            "{data:02x?}"
        );
    };
    // Unknown flag bit, missing base bit.
    unsupported(B::new().u32(0x801).u32(0x1F00_0001).build());
    unsupported(B::new().u32(0x200).u32(0x1F00_0001).build());
    // Setup-section element type other than 9.
    unsupported(
        B::new()
            .u32(0x201)
            .u32(0x1F00_0001)
            .tsize(0, 1)
            .u32(1)
            .u32(8)
            .u32(0)
            .build(),
    );
    // Appearance header other than (33, 2, 1).
    unsupported(
        B::new()
            .u32(0x101)
            .u32(0x1F00_0001)
            .u32(33)
            .u32(3)
            .u32(1)
            .build(),
    );
    // DID with another prefix.
    assert!(matches!(
        VisualTemplate::parse(&B::new().u32(0x001).u32(0x0400_0001).build()),
        Err(FormatError::WrongRecordType { .. })
    ));
}

// --------------------------------------------------------------- entity descriptors

fn entity_bytes(flags: u32, did: u32, template: u32, extra: &[u32]) -> Vec<u8> {
    let mut b = B::new().u32(flags).u32(did).u32(7).u32(template);
    if flags & 0x10 != 0 {
        b = b
            .f32(0.0)
            .f32(0.0)
            .f32(-3.0)
            .f32(1.0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0);
    }
    // Embedded property set: one Int entry with its PID echo.
    b = b.tsize(0, 1).u32(INT).u32(INT).u32(42);
    if flags & 0x200 != 0 {
        b = b.u8(extra.len() as u8);
        for &d in extra {
            b = b.u32(d);
        }
    }
    b.build()
}

#[test]
fn entity_descriptor_layouts_decode() {
    for flags in [0x8B, 0x9B, 0x28B, 0x29B] {
        let data = entity_bytes(flags, 0x4700_0001, 0x1F00_0002, &[0x7800_0003, 0x7800_0004]);
        let e = EntityDescriptor::parse_expecting(&data, 0x4700_0001u32, &schema()).unwrap();
        assert_eq!(e.header.flags, flags);
        assert_eq!(e.header.kind, 7);
        assert_eq!(e.template_link(), TemplateLink::Template(Did(0x1F00_0002)));
        assert_eq!(e.properties.get(INT), Some(&PropertyValue::Int(42)));
        assert_eq!(e.header.block.is_some(), flags & 0x10 != 0);
        if let Some(b) = e.header.block {
            assert_eq!(
                b,
                EntityBlock {
                    vector: [0.0, 0.0, -3.0],
                    rotation: Quaternion::IDENTITY
                }
            );
        }
        assert_eq!(e.extra_sets.len(), if flags & 0x200 != 0 { 2 } else { 0 });
        assert_eq!(e.first_unusual_field(), None);
        // Round trip with the property set's own bytes, found by property_set_bytes.
        let pset = B::new().tsize(0, 1).u32(INT).u32(INT).u32(42).build();
        assert_eq!(e.property_set_bytes(&data), Some(pset.as_slice()));
        assert_eq!(
            e.header.encoded_len(),
            if flags & 0x10 != 0 { 44 } else { 16 }
        );
        assert_eq!(e.to_bytes_with(&pset).unwrap(), data);
        assert_eq!(e.property_set_bytes(&data[..10]), None);
        // The header alone needs no schema.
        let h = decode_entity_header(&data).unwrap();
        assert_eq!(h, e.header);
        assert_eq!(decode_entity_descriptor(&data, &schema()).unwrap(), e);
        for n in 0..data.len() {
            assert!(EntityDescriptor::parse(&data[..n], &schema()).is_err());
        }
    }
    // Unknown PID without a schema.
    let data = entity_bytes(0x8B, 0x4700_0001, 0x4700_0002, &[]);
    assert!(matches!(
        EntityDescriptor::parse(&data, &NoSchema),
        Err(FormatError::UnknownProperty { .. })
    ));
    assert_eq!(
        EntityHeader::parse(&data).unwrap().template_link(),
        TemplateLink::Entity(Did(0x4700_0002))
    );
    let (_, trailing) =
        EntityDescriptor::parse_prefix(&[data.clone(), vec![0]].concat(), &schema()).unwrap();
    assert_eq!(trailing, 1);
    assert!(matches!(
        EntityDescriptor::parse(&[data, vec![0]].concat(), &schema()),
        Err(FormatError::TrailingBytes { .. })
    ));
}

#[test]
fn entity_encoding_derives_flags_from_fields() {
    let pset = B::new().tsize(0, 1).u32(INT).u32(INT).u32(42).build();
    let data = entity_bytes(0x8B, 0x4700_0001, 0x1F00_0002, &[]);
    let e = EntityDescriptor::parse(&data, &schema()).unwrap();
    // Bit 0x10 without a block: nothing is invented (previously a zero quaternion).
    let mut x = e.clone();
    x.header.flags |= 0x10;
    assert_eq!(x.to_bytes_with(&pset).unwrap(), data);
    // A block without its bit: written, with the bit.
    let mut x = e.clone();
    x.header.block = Some(EntityBlock::default());
    let back = EntityDescriptor::parse(&x.to_bytes_with(&pset).unwrap(), &schema()).unwrap();
    assert_eq!(back.header.flags, 0x9B);
    assert_eq!(back.header.block, Some(EntityBlock::default()));
    // A 0x78 list without its bit: written, with the bit.
    let mut x = e;
    x.extra_sets = vec![Did(0x7800_0001)];
    let back = EntityDescriptor::parse(&x.to_bytes_with(&pset).unwrap(), &schema()).unwrap();
    assert_eq!(back.header.flags, 0x28B);
    assert_eq!(back.extra_sets, vec![Did(0x7800_0001)]);
}

#[test]
fn entity_descriptor_rejects_unsupported_flags() {
    for flags in [0x0B, 0x8A, 0x48B, 0x8000_008B] {
        let data = entity_bytes(flags, 0x4700_0001, 0x1F00_0002, &[]);
        assert!(
            matches!(
                EntityHeader::parse(&data),
                Err(FormatError::Unsupported { .. })
            ),
            "{flags:#x}"
        );
    }
    let data = entity_bytes(0x8B, 0x7800_0001, 0x1F00_0002, &[]);
    assert!(matches!(
        EntityHeader::parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
    let data = entity_bytes(0x8B, 0x4700_0001, 0x0600_0002, &[]);
    let e = EntityDescriptor::parse(&data, &schema()).unwrap();
    assert!(matches!(e.template_link(), TemplateLink::Other(_)));
    assert!(e.first_unusual_field().is_some());
}

// --------------------------------------------------------------- appearance records

fn appearance() -> AppearanceRecord {
    let group = MaterialGroup {
        shader: Did(0x2B00_0001),
        material: Did(0x3000_0002),
        a: 2,
        binding_key: 0x1000_0050,
        g: 0.0,
    };
    let part = |mesh: u32| AppearancePart {
        part_param: 10.0,
        slot: 0x1000_000C,
        mesh: Did(mesh),
    };
    AppearanceRecord {
        did: Did(0x2000_0100),
        section: 0,
        entries: vec![
            AppearanceEntry {
                key: 0x1000_0001,
                blocks: vec![
                    AppearanceBlock {
                        variant: 0.01,
                        group_param: 0.0,
                        groups: vec![group],
                        parts: vec![],
                        extras: vec![],
                    },
                    AppearanceBlock {
                        variant: 1.0,
                        group_param: 5.0,
                        groups: vec![group, group],
                        parts: vec![part(0x0600_0001), part(0x0600_0002)],
                        extras: vec![AppearanceExtra {
                            kind: 9,
                            slot: 0x1000_0003,
                            did: Did(0x0400_0001),
                        }],
                    },
                ],
            },
            AppearanceEntry {
                key: 0x1000_0002,
                blocks: vec![AppearanceBlock {
                    variant: 0.0,
                    group_param: 0.0,
                    groups: vec![],
                    parts: vec![part(0x0600_0003)],
                    extras: vec![],
                }],
            },
        ],
        key_table: vec![(0x1000_0001, 0), (0x1000_0002, 1)],
    }
}

#[test]
fn appearance_round_trips() {
    let a = appearance();
    let data = a.to_bytes();
    let back = AppearanceRecord::parse_expecting(&data, 0x2000_0100u32).unwrap();
    assert_eq!(back, a);
    assert_eq!(decode_appearance(&data).unwrap(), a);
    assert_eq!(back.first_unusual_field(), None);
    let e = back.entry(0x1000_0001).unwrap();
    let base = e.base_block().unwrap();
    assert!(base.is_base());
    assert_eq!(
        base.meshes().collect::<Vec<_>>(),
        vec![Did(0x0600_0001), Did(0x0600_0002)]
    );
    // The only block is the base even when its variant is not 1.0.
    assert_eq!(
        back.entry(0x1000_0002)
            .unwrap()
            .base_block()
            .unwrap()
            .variant,
        0.0
    );
    assert_eq!(back.key_table_value(0x1000_0002), Some(1));
    assert!(back.entry(0x1000_0003).is_none());
    for n in 0..data.len() {
        assert!(AppearanceRecord::parse(&data[..n]).is_err());
    }
}

#[test]
fn appearance_rejects_bad_framing() {
    let good = appearance().to_bytes();
    // reserved word.
    let mut d = good.clone();
    d[4] = 1;
    assert!(matches!(
        AppearanceRecord::parse(&d),
        Err(FormatError::Unsupported { .. })
    ));
    // Duplicate entry keys.
    let mut a = appearance();
    a.entries[1].key = a.entries[0].key;
    a.key_table[1].0 = a.key_table[0].0;
    assert!(matches!(
        AppearanceRecord::parse(&a.to_bytes()),
        Err(FormatError::Malformed { .. })
    ));
    // Key table with another count.
    let mut a = appearance();
    a.key_table.pop();
    assert!(matches!(
        AppearanceRecord::parse(&a.to_bytes()),
        Err(FormatError::CountMismatch { .. })
    ));
    // Extra of another type.
    let mut a = appearance();
    a.entries[0].blocks[1].extras[0].kind = 8;
    assert!(matches!(
        AppearanceRecord::parse(&a.to_bytes()),
        Err(FormatError::Unsupported { value: 8, .. })
    ));
    // Key-table tag other than 1: the tag sits right before the table count.
    let mut d = good.clone();
    let tag = d.len() - 1 - 2 * 8 - 1;
    d[tag] = 2;
    assert!(matches!(
        AppearanceRecord::parse(&d),
        Err(FormatError::Unsupported { value: 2, .. })
    ));
    // Non-zero block end word: the last block's end word sits before the tag.
    let mut d = good;
    d[tag - 4] = 1;
    assert!(matches!(
        AppearanceRecord::parse(&d),
        Err(FormatError::Unsupported { value: 1, .. })
    ));
    // Hostile entry count.
    let d = B::new()
        .u32(0x2000_0001)
        .u32(0)
        .u8(0)
        .vle(0x3FFF_FFFF)
        .build();
    assert!(matches!(
        AppearanceRecord::parse(&d),
        Err(FormatError::Truncated { .. })
    ));
    // Unusual: key table naming other keys.
    let mut a = appearance();
    a.key_table[0].0 = 0x1000_0009;
    assert!(a.first_unusual_field().is_some());
}

// ------------------------------------------------------------------------ dispatch

#[test]
fn any_record_dispatches_new_types() {
    let setup = full_setup().to_bytes().unwrap();
    let rec = AnyRecord::decode(0x0400_0123u32, &setup).unwrap().unwrap();
    assert!(matches!(rec, AnyRecord::Setup(_)));
    assert_eq!(rec.type_name(), "setup");
    assert_eq!(rec.did(), Did(0x0400_0123));
    let t = template(0x201).to_bytes();
    assert!(matches!(
        AnyRecord::decode(0x1F00_0010u32, &t).unwrap().unwrap(),
        AnyRecord::VisualTemplate(_)
    ));
    let a = appearance().to_bytes();
    assert!(matches!(
        AnyRecord::decode(0x2000_0100u32, &a).unwrap().unwrap(),
        AnyRecord::Appearance(_)
    ));
    // Entity descriptors need a schema and are not dispatched.
    assert!(!AnyRecord::is_decodable(0x4700_0001u32));
    for did in [0x0400_0001u32, 0x1F00_0001, 0x2000_0001, 0x4700_0001] {
        assert_eq!(Did(did).kind().evidence(), Evidence::Decoded);
    }
    assert_eq!(Did(0x0400_0001).kind(), DidKind::Setup);
    assert_eq!(Did(0x0400_0001).kind().name(), "setup");
    assert_eq!(Did(0x0100_0001).kind().name(), "collision geometry");
    // Variant names follow the decoders (0x01 is not a setup, §2.2).
    assert_eq!(Did(0x0100_0001).kind(), DidKind::CollisionGeometry);
    assert_eq!(Did(0x1F00_0001).kind(), DidKind::VisualTemplate);
    assert_eq!(Did(0x4700_0001).kind(), DidKind::EntityDescriptor);
    assert_eq!(format!("{:?}", Did(0x0400_0001).kind()), "Setup");
    assert!(!AnyRecord::is_decodable(0x0100_0001u32));
    #[allow(deprecated)]
    {
        assert_eq!(DidKind::Skeleton, DidKind::Setup);
        assert_eq!(DidKind::HeldItemTemplate, DidKind::VisualTemplate);
        assert_eq!(DidKind::PhysicsDescriptor, DidKind::EntityDescriptor);
    }
}

// ---------------------------------------------------------------------- resolution

fn store(records: &[(u32, Vec<u8>)]) -> HashMap<Did, Vec<u8>> {
    records.iter().map(|(d, b)| (Did(*d), b.clone())).collect()
}

fn template_with_setup(did: u32, setup: u32) -> Vec<u8> {
    B::new()
        .u32(0x201)
        .u32(did)
        .tsize(0, 1)
        .u32(1)
        .u32(9)
        .u32(1)
        .u32(setup)
        .build()
}

fn template_with_parent(did: u32, parent: u32) -> Vec<u8> {
    B::new().u32(0x003).u32(did).u32(parent).build()
}

#[test]
fn chains_resolve() {
    let s = store(&[
        (
            0x4700_0001,
            entity_bytes(0x8B, 0x4700_0001, 0x4700_0002, &[]),
        ),
        (
            0x4700_0002,
            entity_bytes(0x9B, 0x4700_0002, 0x1F00_0001, &[]),
        ),
        (0x1F00_0001, template_with_parent(0x1F00_0001, 0x1F00_0002)),
        (0x1F00_0002, template_with_setup(0x1F00_0002, 0x0400_0001)),
        (0x0400_0001, minimal_setup_bytes(0x0400_0001, 0x0600_0009)),
    ]);
    let fetch = |d: Did| Ok::<_, Infallible>(s.get(&d).map(Vec::as_slice));
    let chain = resolve_setup(Did(0x4700_0001), fetch).unwrap();
    assert_eq!(
        chain.path,
        vec![
            Did(0x4700_0001),
            Did(0x4700_0002),
            Did(0x1F00_0001),
            Did(0x1F00_0002),
            Did(0x0400_0001)
        ]
    );
    assert_eq!(chain.hops(), 4);
    assert_eq!(chain.start(), Did(0x4700_0001));
    assert_eq!(chain.parts(), &[Did(0x0600_0009)]);
    // Starting at a template or at the setup itself.
    assert_eq!(resolve_setup(Did(0x1F00_0002), fetch).unwrap().hops(), 1);
    assert_eq!(resolve_setup(Did(0x0400_0001), fetch).unwrap().hops(), 0);
    // The DID-only resolver follows the same path and never reads the setup.
    let fetched = RefCell::new(Vec::new());
    let counting = |d: Did| {
        fetched.borrow_mut().push(d);
        Ok::<_, Infallible>(s.get(&d).map(Vec::as_slice))
    };
    let path = resolve_setup_did(Did(0x4700_0001), counting).unwrap();
    assert_eq!(path, chain.path);
    assert!(!fetched.borrow().contains(&Did(0x0400_0001)));
    assert_eq!(fetched.borrow().len(), 4);
    // A setup that does not exist is not noticed by it, but is by resolve_setup.
    let path = resolve_setup_did(Did(0x0400_7777), counting).unwrap();
    assert_eq!(path, vec![Did(0x0400_7777)]);
    assert!(matches!(
        resolve_setup(Did(0x0400_7777), fetch),
        Err(ResolveError::Format(FormatError::DanglingReference {
            from: 0,
            to: 0x0400_7777
        }))
    ));
}

/// A fetch failure that is not "record absent".
#[derive(Debug, PartialEq)]
struct ReadFailed(u32);

impl std::fmt::Display for ReadFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inflate failed for {:#010x}", self.0)
    }
}

impl std::error::Error for ReadFailed {}

#[test]
fn fetch_failures_are_not_dangling_references() {
    let s = Rc::new(store(&[
        (
            0x4700_0001,
            entity_bytes(0x8B, 0x4700_0001, 0x1F00_0002, &[]),
        ),
        (0x1F00_0002, template_with_setup(0x1F00_0002, 0x0400_0001)),
        (0x0400_0001, minimal_setup_bytes(0x0400_0001, 0x0600_0009)),
    ]));
    // Reading the template fails: the error names it and keeps the caller's error.
    let failing = |bad: u32| {
        let s = Rc::clone(&s);
        move |d: Did| {
            if d.0 == bad {
                Err(ReadFailed(d.0))
            } else {
                Ok(s.get(&d).cloned())
            }
        }
    };
    let e = resolve_setup(Did(0x4700_0001), failing(0x1F00_0002)).unwrap_err();
    assert!(e.format_error().is_none());
    match &e {
        ResolveError::Fetch { did, source } => {
            assert_eq!(*did, 0x1F00_0002);
            assert_eq!(*source, ReadFailed(0x1F00_0002));
        }
        ResolveError::Format(f) => panic!("unexpected {f}"),
    }
    assert!(std::error::Error::source(&e).is_some());
    assert!(e.to_string().contains("0x1f000002"));
    // The same when the setup itself fails to read; the DID-only resolver does not read it.
    let e = resolve_setup(Did(0x4700_0001), failing(0x0400_0001)).unwrap_err();
    assert!(matches!(
        e,
        ResolveError::Fetch {
            did: 0x0400_0001,
            ..
        }
    ));
    assert!(resolve_setup_did(Did(0x4700_0001), failing(0x0400_0001)).is_ok());
    // And when the start fails.
    let e = resolve_setup_did(Did(0x4700_0001), failing(0x4700_0001)).unwrap_err();
    assert!(matches!(
        e,
        ResolveError::Fetch {
            did: 0x4700_0001,
            ..
        }
    ));
}

#[test]
fn broken_chains_are_reported() {
    let s = store(&[
        // Cycle between two entities.
        (
            0x4700_0001,
            entity_bytes(0x8B, 0x4700_0001, 0x4700_0002, &[]),
        ),
        (
            0x4700_0002,
            entity_bytes(0x8B, 0x4700_0002, 0x4700_0001, &[]),
        ),
        // Dangling template link.
        (
            0x4700_0003,
            entity_bytes(0x8B, 0x4700_0003, 0x1F00_0099, &[]),
        ),
        // Link to an unexpected prefix.
        (
            0x4700_0004,
            entity_bytes(0x8B, 0x4700_0004, 0x0600_0001, &[]),
        ),
        // Template whose setup section names a mesh.
        (0x1F00_0005, template_with_setup(0x1F00_0005, 0x0600_0001)),
        // Template with neither setup nor parent.
        (0x1F00_0006, B::new().u32(0x001).u32(0x1F00_0006).build()),
        // A record stored under the wrong DID.
        (0x1F00_0007, template_with_setup(0x1F00_0008, 0x0400_0001)),
        // A template whose parent is a setup.
        (0x1F00_0009, template_with_parent(0x1F00_0009, 0x0400_0001)),
    ]);
    let fetch = |d: Did| Ok::<_, Infallible>(s.get(&d).cloned());
    let err = |d: u32| match resolve_setup(Did(d), fetch).unwrap_err() {
        ResolveError::Format(e) => e,
        ResolveError::Fetch { source, .. } => match source {},
    };
    assert!(matches!(
        err(0x4700_0001),
        FormatError::ReferenceChain { .. }
    ));
    assert_eq!(
        err(0x4700_0003),
        FormatError::DanglingReference {
            from: 0x4700_0003,
            to: 0x1F00_0099
        }
    );
    assert_eq!(
        err(0x4700_0042),
        FormatError::DanglingReference {
            from: 0,
            to: 0x4700_0042
        }
    );
    assert!(matches!(
        err(0x4700_0004),
        FormatError::ReferenceChain {
            did: 0x4700_0004,
            ..
        }
    ));
    assert!(matches!(
        err(0x1F00_0005),
        FormatError::ReferenceChain { .. }
    ));
    assert!(matches!(
        err(0x1F00_0006),
        FormatError::ReferenceChain { .. }
    ));
    assert!(matches!(
        err(0x1F00_0009),
        FormatError::ReferenceChain { .. }
    ));
    let e = err(0x1F00_0007);
    assert!(matches!(
        e,
        FormatError::InRecord {
            did: 0x1F00_0007,
            ..
        }
    ));
    assert!(matches!(e.root_cause(), FormatError::DidMismatch { .. }));
    assert!(matches!(
        err(0x0600_0001),
        FormatError::ReferenceChain { .. }
    ));
}

#[test]
fn long_chains_are_cut_off() {
    let mut records = Vec::new();
    for i in 0..40u32 {
        let did = 0x4700_0100 + i;
        records.push((did, entity_bytes(0x8B, did, did + 1, &[])));
    }
    let s = store(&records);
    let e = resolve_setup(Did(0x4700_0100), |d| {
        Ok::<_, Infallible>(s.get(&d).cloned())
    })
    .unwrap_err();
    assert!(
        matches!(e, ResolveError::Format(FormatError::ReferenceChain { .. })),
        "{e}"
    );
    // A chain of exactly MAX_CHAIN_HOPS links still resolves.
    let mut records = Vec::new();
    let hops = MAX_CHAIN_HOPS as u32;
    for i in 0..hops - 1 {
        let did = 0x4700_0200 + i;
        let next = if i + 1 == hops - 1 {
            0x1F00_0001
        } else {
            did + 1
        };
        records.push((did, entity_bytes(0x8B, did, next, &[])));
    }
    records.push((0x1F00_0001, template_with_setup(0x1F00_0001, 0x0400_0001)));
    records.push((0x0400_0001, minimal_setup_bytes(0x0400_0001, 0x0600_0001)));
    let s = store(&records);
    let chain = resolve_setup(Did(0x4700_0200), |d| {
        Ok::<_, Infallible>(s.get(&d).cloned())
    })
    .unwrap();
    assert_eq!(chain.hops(), MAX_CHAIN_HOPS);
}

// ------------------------------------------------------------------------ proptest

fn arb_frame() -> impl Strategy<Value = Frame> {
    (any::<[i8; 3]>(), any::<[i8; 3]>(), any::<[i8; 4]>()).prop_map(|(p, s, q)| Frame {
        position: p.map(f32::from),
        scale: s.map(f32::from),
        rotation: Quaternion {
            w: f32::from(q[0]),
            x: f32::from(q[1]),
            y: f32::from(q[2]),
            z: f32::from(q[3]),
        },
    })
}

proptest! {
    #[test]
    fn setups_round_trip(
        tag in proptest::collection::vec(any::<u8>(), 0..64),
        hint in 0u8..13,
        attach in proptest::collection::vec(any::<(u32, u32)>(), 0..8),
        a in proptest::collection::vec((any::<u32>(), any::<i32>(), arb_frame()), 0..4),
        b in proptest::collection::vec((any::<u32>(), arb_frame()), 0..4),
        c in proptest::collection::vec((any::<u32>(), arb_frame()), 0..4),
        spheres in proptest::collection::vec(any::<[i8; 4]>(), 0..4),
        parts in proptest::collection::vec(any::<u32>(), 0..20),
    ) {
        let s = Setup {
            did: Did(0x0400_0001),
            tagfile: tag,
            attach_map_size: TSize { hint, count: attach.len() as u32 },
            attach_map: attach,
            bone_frames_hint: if a.is_empty() { 0 } else { hint },
            bone_frames: a.into_iter().map(|(k, bone, frame)| BoneFrame { key: k, key2: k, bone, frame }).collect(),
            placements: b.into_iter().map(|(id, frame)| Placement { id, frame }).collect(),
            named_frames_hint: 0,
            named_frames: c.into_iter().map(|(k, frame)| NamedFrame { key: k, key2: k, frame }).collect(),
            spheres: spheres.into_iter().map(|v| CollisionSphere {
                centre: [f32::from(v[0]), f32::from(v[1]), f32::from(v[2])],
                radius: f32::from(v[3]),
            }).collect(),
            parts: parts.into_iter().map(Did).collect(),
            ..Setup::default()
        };
        let data = s.to_bytes().unwrap();
        prop_assert_eq!(Setup::parse(&data).unwrap(), s);
    }

    #[test]
    fn appearances_round_trip(
        section in any::<u8>(),
        entries in proptest::collection::vec(
            proptest::collection::vec(
                (any::<i8>(), proptest::collection::vec(any::<[u32; 4]>(), 0..3),
                 proptest::collection::vec(any::<(u32, u32)>(), 0..3)),
                0..3),
            0..6),
    ) {
        let a = AppearanceRecord {
            did: Did(0x2000_0001),
            section,
            entries: entries.into_iter().enumerate().map(|(i, blocks)| AppearanceEntry {
                key: 0x1000_0000 + i as u32,
                blocks: blocks.into_iter().map(|(v, groups, parts)| AppearanceBlock {
                    variant: f32::from(v),
                    group_param: 0.0,
                    groups: groups.into_iter().map(|g| MaterialGroup {
                        shader: Did(g[0]), material: Did(g[1]), a: g[2], binding_key: g[3], g: 1.0,
                    }).collect(),
                    parts: parts.into_iter().map(|(slot, mesh)| AppearancePart {
                        part_param: 10.0, slot, mesh: Did(mesh),
                    }).collect(),
                    extras: vec![],
                }).collect(),
            }).collect(),
            key_table: vec![],
        };
        let mut a = a;
        a.key_table = a.entries.iter().map(|e| (e.key, 0)).collect();
        let data = a.to_bytes();
        prop_assert_eq!(AppearanceRecord::parse(&data).unwrap(), a);
    }

    #[test]
    fn prefixed_small_bytes_never_panic(
        flags in prop_oneof![Just(0x201u32), Just(0x103), Just(0x8B), Just(0x28B), any::<u32>()],
        serial in any::<[u8; 3]>(),
        prefix in prop_oneof![Just(0x04u8), Just(0x1F), Just(0x20), Just(0x47)],
        tail in proptest::collection::vec(prop_oneof![0u8..4, any::<u8>()], 0..512),
    ) {
        let did = [serial[0], serial[1], serial[2], prefix];
        let lead: Vec<u8> = did.iter().copied().chain(tail.iter().copied()).collect();
        let flagged: Vec<u8> = flags.to_le_bytes().iter().chain(&did).chain(&tail).copied().collect();
        for data in [&lead, &flagged] {
            let _ = Setup::parse(data);
            let _ = AppearanceRecord::parse(data);
            let _ = VisualTemplate::parse(data);
            let _ = EntityDescriptor::parse(data, &schema());
            let _ = AnyRecord::decode(u32::from_le_bytes(did), data);
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = Setup::parse(&data);
        let _ = VisualTemplate::parse(&data);
        let _ = AppearanceRecord::parse(&data);
        let _ = EntityHeader::parse(&data);
        let _ = EntityDescriptor::parse(&data, &schema());
    }
}
