// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery records on synthetic payloads (`docs/specs/scenery.md` §4.2, §4.3, §5, §7.2,
//! §13.1): terrain-scene tables (0x12), scene-choice tables (0x11), scene records (0x02) and
//! the scene-reference checker.

mod common;

use std::collections::HashMap;

use common::B;
use tessera_formats::scenery::{
    ChildRef, FloatRange, HeightMode, MAX_CHILD_DEPTH, MAX_PLACEMENT_NODE_DEPTH,
    MAX_SCENE_REFERENCE_DEPTH, MAX_TERRAIN_ENTRY_NAME_LEN, MIN_PLACEMENT_NODE_LEN,
    PLACEMENT_NODE_FIXED_LEN, PlacementNode, Scene, SceneChild, SceneChoice, SceneChoiceEntry,
    SceneChoiceTable, SceneGroup, ScenePair, SceneReferenceChecker, SceneSite, TerrainSceneEntry,
    TerrainSceneTable, decode_scene, decode_scene_choice_table, decode_terrain_scene_table,
};
use tessera_formats::{Did, FormatError, Record};

// ---------------------------------------------------------------------------------------
// 0x12
// ---------------------------------------------------------------------------------------

const TS_DID: u32 = 0x1200_0003;

fn terrain_entry(
    b: B,
    name: &str,
    kind: u32,
    id: u32,
    default: Option<(u32, u32)>,
    pairs: &[(u32, u32)],
) -> B {
    let mut b = b
        .short(name)
        .u32(kind)
        .u32(id)
        .f32(std::f32::consts::FRAC_1_SQRT_2);
    b = match default {
        Some((k, s)) => b.u8(1).u32(k).u32(s),
        None => b.u8(0),
    };
    b = b.u32(pairs.len() as u32);
    for &(k, s) in pairs {
        b = b.u32(k).u32(s);
    }
    b
}

/// A table with the 41-byte default entry of §4.2 and two listed entries.
fn terrain_scene() -> Vec<u8> {
    let b = B::new().u32(TS_DID).u32(0).u8(1);
    let b = terrain_entry(b, "default", 1, 0, Some((1, 0)), &[(1, 0)]);
    let b = b.u32(2);
    let b = terrain_entry(b, "meadow", 2, 7, None, &[(2, 0), (30, 1), (31, 3)]);
    let b = terrain_entry(b, "forest", 2, 9, None, &[(2, 0), (0, 1), (40, 2)]);
    b.build()
}

#[test]
fn terrain_scene_table_decodes_and_looks_up_by_value() {
    let data = terrain_scene();
    // §4.2: the default entry is 41 bytes, so `count` is at record offset 50.
    assert_eq!(&data[50..54], &2u32.to_le_bytes());
    let t = TerrainSceneTable::parse_expecting(&data, TS_DID).unwrap();
    let d = t.default_entry().unwrap();
    assert_eq!((d.name.as_str(), d.kind, d.id), ("default", 1, 0));
    assert_eq!(
        d.default_pair,
        Some(ScenePair {
            scene_key: 1,
            slot: 0
        })
    );
    assert_eq!(t.entries().len(), 2);
    assert_eq!(t.entries()[0].unknown_f, std::f32::consts::FRAC_1_SQRT_2);
    // Pairs are matched by slot value, not position: slot 3 is the third pair.
    assert_eq!(t.scene_key(7, 3), Some(31));
    assert_eq!(t.scene_key(7, 1), Some(30));
    // No pair with slot 2 and no default pair: no scene.
    assert_eq!(t.scene_key(7, 2), None);
    assert_eq!(t.entry_for(7).unwrap().pair_for_slot(2), None);
    // Key 0 means no scene.
    assert_eq!(t.scene_key(9, 1), None);
    assert_eq!(t.scene_key(9, 2), Some(40));
    // Unknown terrain ids (and 0) use the default entry, whose default pair answers any slot.
    assert_eq!(t.scene_key(0, 0), Some(1));
    assert_eq!(t.scene_key(200, 5), Some(1));
    assert!(t.entry(200).is_none());
    // Coarse word: terrain in bits 0–7, slot in bits 8–14, the rest ignored.
    assert_eq!(t.scene_key_for_word(7 | (3 << 8)), Some(31));
    assert_eq!(t.scene_key_for_word(9 | (2 << 8) | (5 << 15)), Some(40));
    // Round trip.
    assert_eq!(t.to_bytes(), data);
    assert_eq!(decode_terrain_scene_table(&data).unwrap(), t);
    let rebuilt = TerrainSceneTable::new(
        Did(TS_DID),
        0,
        t.default_entry().cloned(),
        t.entries().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt, t);
}

#[test]
fn terrain_scene_table_without_default_entry() {
    let b = B::new().u32(TS_DID).u32(5).u8(0).u32(1);
    let data = terrain_entry(b, "x", 2, 3, None, &[(4, 0)]).build();
    let t = TerrainSceneTable::parse(&data).unwrap();
    assert_eq!(t.reserved, 5);
    assert!(t.default_entry().is_none());
    assert_eq!(t.scene_key(3, 0), Some(4));
    assert_eq!(t.scene_key(4, 0), None);
    assert_eq!(t.to_bytes(), data);
}

#[test]
fn terrain_scene_table_rejects_bad_input() {
    let data = terrain_scene();
    for len in 0..data.len() {
        assert!(TerrainSceneTable::parse(&data[..len]).is_err(), "{len}");
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        TerrainSceneTable::parse(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
    // has_default is read as a bool8 (scenery.md §4.2 OPEN on its type): 2 is rejected.
    let mut bad = data.clone();
    bad[8] = 2;
    assert!(matches!(
        TerrainSceneTable::parse(&bad),
        Err(FormatError::InvalidBool { .. })
    ));
    // A first name-length byte of 0x80 or more is the unverified two-byte form (§4.2 OPEN).
    let mut bad = data.clone();
    bad[9] = 0x80;
    assert!(matches!(
        TerrainSceneTable::parse(&bad),
        Err(FormatError::Unsupported { value: 0x80, .. })
    ));
    // Hostile counts.
    let bad = B::new().u32(TS_DID).u32(0).u8(0).u32(u32::MAX).build();
    assert!(matches!(
        TerrainSceneTable::parse(&bad),
        Err(FormatError::Truncated { .. })
    ));
    let b = B::new().u32(TS_DID).u32(0).u8(0).u32(1);
    let bad = b
        .short("x")
        .u32(2)
        .u32(1)
        .f32(0.7)
        .u8(0)
        .u32(u32::MAX)
        .build();
    assert!(matches!(
        TerrainSceneTable::parse(&bad),
        Err(FormatError::Truncated { .. })
    ));
    // Wrong prefix.
    let mut bad = data.clone();
    bad[3] = 0x11;
    assert!(matches!(
        TerrainSceneTable::parse(&bad),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn repeated_terrain_ids_resolve_to_the_first() {
    let t = TerrainSceneTable::new(
        Did(TS_DID),
        0,
        None,
        vec![
            TerrainSceneEntry {
                id: 4,
                pairs: vec![ScenePair {
                    scene_key: 10,
                    slot: 0,
                }],
                ..TerrainSceneEntry::default()
            },
            TerrainSceneEntry {
                id: 4,
                pairs: vec![ScenePair {
                    scene_key: 11,
                    slot: 0,
                }],
                ..TerrainSceneEntry::default()
            },
        ],
    )
    .unwrap();
    assert_eq!(t.scene_key(4, 0), Some(10));
    assert_eq!(TerrainSceneTable::parse(&t.to_bytes()).unwrap(), t);
}

// ---------------------------------------------------------------------------------------
// 0x11
// ---------------------------------------------------------------------------------------

const SC_DID: u32 = 0x1100_0003;

fn scene_choice() -> Vec<u8> {
    B::new()
        .u32(0)
        .u32(SC_DID)
        .u32(2)
        .u32(30)
        .u32(3)
        .u32(0x0200_0010)
        .f32(1.0)
        .u8(1)
        .u32(0x0200_0011)
        .f32(1.0)
        .u8(1)
        .u32(0x0200_0012)
        .f32(2.0)
        .u8(1)
        .u32(1)
        .u32(1)
        .u32(0x0200_0000)
        .f32(1.0)
        .u8(1)
        .build()
}

#[test]
fn scene_choice_table_decodes_and_chooses() {
    let data = scene_choice();
    let t = SceneChoiceTable::parse_expecting(&data, SC_DID).unwrap();
    assert_eq!(t.reserved, 0);
    assert_eq!(t.entries().len(), 2);
    let e = t.get(30).unwrap();
    assert_eq!(e.choices.len(), 3);
    assert_eq!(e.total_weight(), 4.0);
    assert_eq!(e.choices[2].flag, 1);
    // §14 worked example: u = 0.44615, T = 1.7846 → running sums 1.0, 2.0 → the second.
    assert_eq!(e.choose(0.44615).unwrap().scene, Did(0x0200_0011));
    assert_eq!(e.choose(0.0).unwrap().scene, Did(0x0200_0010));
    assert_eq!(e.choose(0.6).unwrap().scene, Did(0x0200_0012));
    // Strictly greater: T = 1.0 exactly does not pick the first.
    assert_eq!(e.choose(0.25).unwrap().scene, Did(0x0200_0011));
    // Past the total (or NaN), the last is picked.
    assert_eq!(e.choose(1.5).unwrap().scene, Did(0x0200_0012));
    assert_eq!(e.choose(f32::NAN).unwrap().scene, Did(0x0200_0012));
    assert_eq!(
        t.get(1).unwrap().choose(0.99).unwrap().scene,
        Did(0x0200_0000)
    );
    assert!(SceneChoiceEntry::default().choose(0.5).is_none());
    assert!(t.get(2).is_none());
    assert_eq!(t.to_bytes(), data);
    assert_eq!(decode_scene_choice_table(&data).unwrap(), t);
    let rebuilt = SceneChoiceTable::new(0, Did(SC_DID), t.entries().to_vec()).unwrap();
    assert_eq!(rebuilt, t);
    assert!(SceneChoiceTable::new(0, Did(TS_DID), vec![]).is_err());
}

#[test]
fn scene_choice_table_rejects_bad_input() {
    let data = scene_choice();
    for len in 0..data.len() {
        assert!(SceneChoiceTable::parse(&data[..len]).is_err(), "{len}");
    }
    let mut long = data.clone();
    long.extend_from_slice(&[0, 0]);
    assert!(matches!(
        SceneChoiceTable::parse(&long),
        Err(FormatError::TrailingBytes { count: 2, .. })
    ));
    // The DID is the second u32 (prologue xD, records.md §1.6).
    let mut swapped = data.clone();
    swapped[..4].copy_from_slice(&SC_DID.to_le_bytes());
    swapped[4..8].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        SceneChoiceTable::parse(&swapped),
        Err(FormatError::WrongRecordType { .. })
    ));
    for (count, n) in [(u32::MAX, 0), (1, u32::MAX)] {
        let bad = B::new()
            .u32(0)
            .u32(SC_DID)
            .u32(count)
            .u32(1)
            .u32(n)
            .bytes(&[0; 32])
            .build();
        assert!(matches!(
            SceneChoiceTable::parse(&bad),
            Err(FormatError::Truncated { .. })
        ));
    }
    // Unknown flag values are kept (meaning OPEN).
    let t = SceneChoiceTable::new(
        7,
        Did(SC_DID),
        vec![SceneChoiceEntry {
            key: 5,
            choices: vec![SceneChoice {
                scene: Did(0x0200_0001),
                weight: 0.5,
                flag: 9,
            }],
        }],
    )
    .unwrap();
    assert_eq!(SceneChoiceTable::parse(&t.to_bytes()).unwrap(), t);
}

// ---------------------------------------------------------------------------------------
// 0x02
// ---------------------------------------------------------------------------------------

const SCENE_DID: u32 = 0x0200_0040;

/// The fixed part of a placement node (`scenery.md` §5.3, offsets 0–186) with the most
/// common observed values, `frequency` = `freq` and `child_count` = `children`.
fn node_fixed(b: B, freq: f32, children: u32) -> B {
    let start = b.0.len();
    let b = b
        .f32(-5.0)
        .f32(-5.0)
        .f32(0.0) // 0 disp_min
        .f32(5.0)
        .f32(5.0)
        .f32(0.0) // 12 disp_max
        .f32(0.0)
        .f32(359.0) // 24 heading
        .f32(0.0)
        .f32(0.0) // 32 angle2
        .f32(0.0)
        .f32(0.0) // 40 angle3
        .f32(freq) // 48 frequency
        .f32(0.8)
        .f32(0.8)
        .f32(0.8) // 52 scale_min
        .f32(1.2)
        .f32(1.2)
        .f32(1.2) // 64 scale_max
        .f32(1.0) // 76 stretch_ref
        .f32(1.0) // 80 slope_max_cos
        .f32(0.766) // 84 slope_min_cos
        .u8(1) // 88 align_to_slope
        .u8(0) // 89 face_direction
        .f32(-0.0002)
        .f32(0.0002) // 90 above_ground
        .f32(-0.0002)
        .f32(0.0002) // 98 above_water
        .f32(-0.0002)
        .f32(0.0002) // 106 below_water
        .f32(0.0)
        .f32(50.0) // 114 water_depth
        .f32(-0.0002) // 122 below_ground_min
        .f32(-f32::MAX)
        .f32(f32::MAX) // 126 z_range
        .f32(0.1) // 134 spacing
        .f32(0.5) // 138 clearance
        .u8(1) // 142 clearance_scaled
        .f32(1.0) // 143 clear_max_cos
        .f32(0.766) // 147 clear_min_cos
        .f32(0.9848) // 151 clear_min_dot
        .f32(360.0) // 155 clear_aspect_tol
        .u32(0) // 159 max_objects
        .f32(1.0) // 163 unknown_163
        .bytes(&[1, 1, 1, 1, 1, 0, 0, 0, 1]) // 167–175 bools
        .bytes(&[1, 4, 1, 1, 0, 2, 2]); // 176–182 modes
    assert_eq!(b.0.len() - start, 183, "child_count is at node offset 183");
    let b = b.u32(children);
    assert_eq!(b.0.len() - start, PLACEMENT_NODE_FIXED_LEN);
    b
}

/// A scene with a frill layer (two scene references), and one group holding a node with
/// an object, a nested node (with one object) and a scene reference; then a bare object.
fn scene() -> Vec<u8> {
    let b = B::new().u32(0).u32(SCENE_DID).u8(1).u8(1);
    let b = node_fixed(b, 1.0, 2)
        .u8(3)
        .u32(0x0200_0001)
        .u8(3)
        .u32(0x0200_0002)
        .f32(0.0002);
    let b = b.u32(1).u32(1).u32(2).u8(2);
    let b = node_fixed(b, 0.5, 3).u8(1).u32(0x4700_0001).u8(2);
    let b = node_fixed(b, 0.25, 1).u8(1).u32(0x4700_0002).f32(0.5);
    let b = b.u8(3).u32(0x0200_0003).f32(0.0002);
    b.u8(1).u32(0x4700_0003).build()
}

#[test]
fn scene_decodes_with_the_spec_offsets() {
    let data = scene();
    let s = Scene::parse_expecting(&data, SCENE_DID).unwrap();
    assert_eq!((s.reserved, s.did, s.flag_a), (0, Did(SCENE_DID), true));
    let f = s.frill_layer.as_ref().unwrap();
    assert_eq!(f.frequency, 1.0);
    assert_eq!(
        f.children,
        [
            SceneChild::Scene(Did(0x0200_0001)),
            SceneChild::Scene(Did(0x0200_0002))
        ]
    );
    assert_eq!(f.below_ground_max, 0.0002);
    assert_eq!(s.groups.len(), 1);
    let g = s.placement_group().unwrap();
    assert_eq!(g.children.len(), 2);
    let SceneChild::Node(n) = &g.children[0] else {
        panic!("node expected")
    };
    assert_eq!(n.disp_min, [-5.0, -5.0, 0.0]);
    assert_eq!(n.disp_max, [5.0, 5.0, 0.0]);
    assert_eq!(
        n.heading,
        FloatRange {
            min: 0.0,
            max: 359.0
        }
    );
    assert_eq!(n.frequency, 0.5);
    assert_eq!(n.scale_min, [0.8; 3]);
    assert_eq!(n.scale_max, [1.2; 3]);
    assert_eq!((n.slope_max_cos, n.slope_min_cos), (1.0, 0.766));
    assert!(n.align_to_slope && !n.face_direction);
    assert_eq!(
        n.water_depth,
        FloatRange {
            min: 0.0,
            max: 50.0
        }
    );
    assert_eq!(
        n.z_range,
        FloatRange {
            min: -f32::MAX,
            max: f32::MAX
        }
    );
    assert_eq!((n.spacing, n.clearance), (0.1, 0.5));
    assert!(n.clearance_scaled);
    assert_eq!(
        (
            n.clear_max_cos,
            n.clear_min_cos,
            n.clear_min_dot,
            n.clear_aspect_tol
        ),
        (1.0, 0.766, 0.9848, 360.0)
    );
    assert_eq!((n.max_objects, n.unknown_163), (0, 1.0));
    assert!(n.disp_scaled && n.inherit_scale && n.disp_relative && n.orient_relative);
    assert!(n.spacing_scaled && !n.no_checks && !n.spacing_3d && !n.unknown_174);
    assert!(n.no_spacing);
    assert_eq!(n.height_mode_kind(), HeightMode::Water);
    assert_eq!(n.scale_mode, 4);
    assert_eq!(
        (
            n.above_ground_mode,
            n.above_water_mode,
            n.below_water_mode,
            n.water_depth_mode,
            n.below_ground_mode
        ),
        (1, 1, 0, 2, 2)
    );
    assert_eq!(
        n.below_ground(),
        FloatRange {
            min: -0.0002,
            max: 0.0002
        }
    );
    assert_eq!(n.children.len(), 3);
    let SceneChild::Node(inner) = &n.children[1] else {
        panic!("nested node expected")
    };
    assert_eq!((inner.frequency, inner.below_ground_max), (0.25, 0.5));
    assert_eq!(g.children[1], SceneChild::Object(Did(0x4700_0003)));

    // Traversal order, sites and depths.
    let seen: Vec<(SceneSite, usize, u8)> = s
        .descendants()
        .map(|ChildRef { site, depth, child }| (site, depth, child.type_code()))
        .collect();
    use SceneSite::{FrillLayer, Group};
    assert_eq!(
        seen,
        [
            (FrillLayer, 2, 3),
            (FrillLayer, 2, 3),
            (Group(0), 1, 2),
            (Group(0), 2, 1),
            (Group(0), 2, 2),
            (Group(0), 3, 1),
            (Group(0), 2, 3),
            (Group(0), 1, 1),
        ]
    );
    let objects: Vec<u32> = s.objects().map(|d| d.0).collect();
    assert_eq!(objects, [0x4700_0001, 0x4700_0002, 0x4700_0003]);
    let refs: Vec<u32> = s.scene_references().map(|d| d.0).collect();
    assert_eq!(refs, [0x0200_0001, 0x0200_0002, 0x0200_0003]);
    let depths: Vec<usize> = s.nodes().map(|(_, d, _)| d).collect();
    assert_eq!(depths, [1, 1, 2]);
    assert!(!s.is_empty());

    assert_eq!(s.to_bytes(), data);
    assert_eq!(decode_scene(&data).unwrap(), s);
}

#[test]
fn empty_scene_is_22_bytes() {
    // §5.4: reserved, DID, flag_a, no frill layer, one group with no children.
    let data = B::new()
        .u32(0)
        .u32(0x0200_0000)
        .u8(1)
        .u8(0)
        .u32(1)
        .u32(1)
        .u32(0)
        .build();
    assert_eq!(data.len(), 22);
    let s = Scene::parse(&data).unwrap();
    assert!(s.is_empty());
    assert!(s.frill_layer.is_none());
    assert_eq!(s.descendants().count(), 0);
    assert_eq!(s.to_bytes(), data);
    // flag_a 0 is accepted (Tessera Decision of §5.1), 2 is not a bool8.
    let mut zero = data.clone();
    zero[8] = 0;
    assert!(!Scene::parse(&zero).unwrap().flag_a);
    let mut two = data.clone();
    two[8] = 2;
    assert!(matches!(
        Scene::parse(&two),
        Err(FormatError::InvalidBool { .. })
    ));
    // No groups at all, and a group id other than 1 (no placement group).
    let none = B::new().u32(0).u32(0x0200_0000).u8(1).u8(0).u32(0).build();
    assert!(Scene::parse(&none).unwrap().placement_group().is_none());
    let other = B::new()
        .u32(0)
        .u32(0x0200_0000)
        .u8(1)
        .u8(0)
        .u32(2)
        .u32(2)
        .u32(0)
        .u32(1)
        .u32(0)
        .build();
    let s = Scene::parse(&other).unwrap();
    assert!(std::ptr::eq(s.placement_group().unwrap(), &s.groups[1]));
}

#[test]
fn scene_rejects_truncation_and_trailing_bytes() {
    let data = scene();
    for len in 0..data.len() {
        assert!(Scene::parse(&data[..len]).is_err(), "{len}");
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        Scene::parse(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
    let mut bad = data.clone();
    bad[7] = 0x47;
    assert!(matches!(
        Scene::parse(&bad),
        Err(FormatError::WrongRecordType { .. })
    ));
}

/// A scene whose group holds one node per level, `levels` deep.
fn nested(levels: usize) -> Vec<u8> {
    let mut b = B::new()
        .u32(0)
        .u32(SCENE_DID)
        .u8(1)
        .u8(0)
        .u32(1)
        .u32(1)
        .u32(1);
    for i in 0..levels {
        b = node_fixed(b.u8(2), 1.0, u32::from(i + 1 < levels));
    }
    for _ in 0..levels {
        b = b.f32(0.0002);
    }
    b.build()
}

#[test]
fn node_nesting_depth_is_capped() {
    let ok = Scene::parse(&nested(MAX_PLACEMENT_NODE_DEPTH)).unwrap();
    assert_eq!(
        ok.nodes().map(|(_, d, _)| d).max(),
        Some(MAX_PLACEMENT_NODE_DEPTH)
    );
    assert!(matches!(
        Scene::parse(&nested(MAX_PLACEMENT_NODE_DEPTH + 1)),
        Err(FormatError::DepthLimit { .. })
    ));
    // A frill layer is level 1 as well.
    let mut b = B::new().u32(0).u32(SCENE_DID).u8(1).u8(1);
    for i in 0..=MAX_PLACEMENT_NODE_DEPTH {
        b = node_fixed(b, 1.0, u32::from(i < MAX_PLACEMENT_NODE_DEPTH));
        if i < MAX_PLACEMENT_NODE_DEPTH {
            b = b.u8(2);
        }
    }
    assert!(matches!(
        Scene::parse(&b.build()),
        Err(FormatError::DepthLimit { .. })
    ));
}

#[test]
fn child_types_and_floats_are_checked() {
    let base = B::new()
        .u32(0)
        .u32(SCENE_DID)
        .u8(1)
        .u8(0)
        .u32(1)
        .u32(1)
        .u32(1);
    // Type 4 is known to the client but never occurs: unsupported.
    let bad = base.clone().u8(4).u32(0).bytes(&[0; 16]).build();
    assert!(matches!(
        Scene::parse(&bad),
        Err(FormatError::Unsupported { value: 4, .. })
    ));
    for t in [0u8, 5, 0xFF] {
        let bad = base.clone().u8(t).u32(0).build();
        assert!(matches!(
            Scene::parse(&bad),
            Err(FormatError::Malformed { .. })
        ));
    }
    // NaN and infinities in node fields are malformed (§13.1); z_range may be infinite.
    let good = node_fixed(base.clone().u8(2), 1.0, 0).f32(0.0).build();
    let node_start = good.len() - MIN_PLACEMENT_NODE_LEN;
    assert!(Scene::parse(&good).is_ok());
    for (off, v, ok) in [
        (48, f32::NAN, false),
        (0, f32::INFINITY, false),
        (155, f32::NEG_INFINITY, false),
        (187, f32::NAN, false),
        (126, f32::NEG_INFINITY, true),
        (130, f32::INFINITY, true),
        (130, f32::NAN, false),
    ] {
        let mut d = good.clone();
        d[node_start + off..node_start + off + 4].copy_from_slice(&v.to_le_bytes());
        assert_eq!(Scene::parse(&d).is_ok(), ok, "offset {off} = {v}");
    }
    // Every bool8 of the node rejects 2.
    for off in [88, 89, 142, 167, 168, 169, 170, 171, 172, 173, 174, 175] {
        let mut d = good.clone();
        d[node_start + off] = 2;
        assert!(
            matches!(Scene::parse(&d), Err(FormatError::InvalidBool { .. })),
            "bool at {off}"
        );
    }
    // Mode bytes are kept as stored.
    let mut d = good.clone();
    d[node_start + 176] = 9;
    d[node_start + 177] = 7;
    let s = Scene::parse(&d).unwrap();
    let (_, _, n) = s.nodes().next().unwrap();
    assert_eq!(
        (n.height_mode_kind(), n.scale_mode),
        (HeightMode::NoSnap, 7)
    );
    assert_eq!(HeightMode::from_raw(5), HeightMode::BaseZero);
    assert_eq!(HeightMode::from_raw(0), HeightMode::Terrain);
    assert_eq!(HeightMode::from_raw(200), HeightMode::NoSnap);
}

#[test]
fn hostile_scene_counts_fail_before_allocating() {
    for (groups, children) in [(u32::MAX, 0), (1, u32::MAX), (1, 0x1000_0000)] {
        let bad = B::new()
            .u32(0)
            .u32(SCENE_DID)
            .u8(1)
            .u8(0)
            .u32(groups)
            .u32(1)
            .u32(children)
            .bytes(&[1; 40])
            .build();
        assert!(matches!(
            Scene::parse(&bad),
            Err(FormatError::Truncated { .. })
        ));
    }
    // Many tiny children stay within the budget and decode.
    let mut b = B::new()
        .u32(0)
        .u32(SCENE_DID)
        .u8(1)
        .u8(0)
        .u32(1)
        .u32(1)
        .u32(10_000);
    for i in 0..10_000u32 {
        b = b.u8(1).u32(0x4700_0000 | i);
    }
    assert_eq!(Scene::parse(&b.build()).unwrap().objects().count(), 10_000);
}

#[test]
fn scene_round_trips_from_values() {
    let node = PlacementNode {
        frequency: 0.3,
        z_range: FloatRange {
            min: f32::NEG_INFINITY,
            max: f32::INFINITY,
        },
        children: vec![
            SceneChild::Object(Did(0x4700_0009)),
            SceneChild::Node(Box::default()),
        ],
        ..PlacementNode::default()
    };
    let s = Scene {
        reserved: 3,
        did: Did(SCENE_DID),
        flag_a: false,
        frill_layer: Some(PlacementNode::default()),
        groups: vec![
            SceneGroup {
                group_id: 1,
                children: vec![SceneChild::Node(Box::new(node))],
            },
            SceneGroup::default(),
        ],
    };
    let bytes = s.to_bytes();
    assert_eq!(Scene::parse(&bytes).unwrap(), s);
}

// ---------------------------------------------------------------------------------------
// Scene-reference graph (§13.1, SC2)
// ---------------------------------------------------------------------------------------

/// A scene whose single group references `refs`.
fn refs_scene(did: u32, refs: &[u32]) -> Scene {
    Scene {
        did: Did(did),
        flag_a: true,
        groups: vec![SceneGroup {
            group_id: 1,
            children: refs.iter().map(|&r| SceneChild::Scene(Did(r))).collect(),
        }],
        ..Scene::default()
    }
}

fn graph(edges: &[(u32, &[u32])]) -> HashMap<Did, Scene> {
    edges
        .iter()
        .map(|&(d, r)| (Did(d), refs_scene(d, r)))
        .collect()
}

#[test]
fn reference_checker_measures_and_refuses_cycles() {
    let g = graph(&[
        (1, &[2, 3]),
        (2, &[4]),
        (3, &[4, 4]),
        (4, &[]),
        (5, &[6]),
        (6, &[5]),
        (7, &[7]),
        (8, &[99]),
    ]);
    let mut c = SceneReferenceChecker::new();
    let mut fetches = 0;
    let mut fetch = |d: Did| {
        fetches += 1;
        g.get(&d)
    };
    assert_eq!(c.height(Did(1), &mut fetch).unwrap(), 3);
    assert_eq!(c.height(Did(4), &mut fetch).unwrap(), 1);
    // The diamond (1 → 2 → 4, 1 → 3 → 4) loads each scene once, memoised across calls.
    assert_eq!(fetches, 4);
    assert!(matches!(
        c.height(Did(5), |d| g.get(&d)),
        Err(FormatError::ReferenceChain { .. })
    ));
    assert!(matches!(
        c.height(Did(7), |d| g.get(&d)),
        Err(FormatError::ReferenceChain { did: 7, .. })
    ));
    assert!(matches!(
        c.height(Did(8), |d| g.get(&d)),
        Err(FormatError::DanglingReference { from: 8, to: 99 })
    ));
    assert!(matches!(
        c.height(Did(100), |d| g.get(&d)),
        Err(FormatError::DanglingReference { from: 0, to: 100 })
    ));
}

#[test]
fn reference_checker_caps_the_path_length() {
    let n = MAX_SCENE_REFERENCE_DEPTH as u32;
    // A chain of n scenes (n − 1 references) is allowed; n + 1 is not.
    let chain: Vec<(u32, Vec<u32>)> = (1..=n + 1)
        .map(|i| (i, if i <= n { vec![i + 1] } else { vec![] }))
        .collect();
    let g: HashMap<Did, Scene> = chain
        .iter()
        .map(|(d, r)| (Did(*d), refs_scene(*d, r)))
        .collect();
    let mut c = SceneReferenceChecker::new();
    assert_eq!(
        c.height(Did(2), |d| g.get(&d)).unwrap(),
        MAX_SCENE_REFERENCE_DEPTH
    );
    // Memoised heights still count against the cap from a longer path.
    assert!(matches!(
        c.height(Did(1), |d| g.get(&d)),
        Err(FormatError::ReferenceChain { .. })
    ));
    assert!(matches!(
        SceneReferenceChecker::new().height(Did(1), |d| g.get(&d)),
        Err(FormatError::ReferenceChain { .. })
    ));
    // A smaller limit.
    let mut small = SceneReferenceChecker::with_limit(2);
    assert_eq!(small.height(Did(n), |d| g.get(&d)).unwrap(), 2);
    assert!(small.height(Did(n - 1), |d| g.get(&d)).is_err());
}

/// A scene whose deepest node, at nesting level `levels`, holds one object: through a
/// group chain (`frill` false) or through the frill layer and its nested nodes.
fn chain_with_object(levels: usize, frill: bool) -> Vec<u8> {
    let mut b = B::new().u32(0).u32(SCENE_DID).u8(1).u8(u8::from(frill));
    if !frill {
        b = b.u32(1).u32(1).u32(1);
    }
    for i in 0..levels {
        if frill && i == 0 {
            b = node_fixed(b, 1.0, 1);
        } else {
            b = node_fixed(b.u8(2), 1.0, 1);
        }
        if i + 1 == levels {
            b = b.u8(1).u32(0x4700_0001);
        }
    }
    for _ in 0..levels {
        b = b.f32(0.0002);
    }
    if frill {
        b = b.u32(0);
    }
    b.build()
}

#[test]
fn child_depths_follow_the_nesting_level() {
    // Regression: an object under a level-16 group node is at depth 17, which the decoder
    // accepts; the fuzz harness asserted depth ≤ 16. Node children stay within the cap.
    for frill in [false, true] {
        let s = Scene::parse(&chain_with_object(MAX_PLACEMENT_NODE_DEPTH, frill)).unwrap();
        let max_node = s
            .descendants()
            .filter(|c| matches!(c.child, SceneChild::Node(_)))
            .map(|c| c.depth)
            .max();
        let object = s
            .descendants()
            .find(|c| matches!(c.child, SceneChild::Object(_)))
            .unwrap();
        assert_eq!(object.depth, MAX_CHILD_DEPTH, "frill {frill}");
        assert_eq!(MAX_CHILD_DEPTH, MAX_PLACEMENT_NODE_DEPTH + 1);
        assert_eq!(
            s.nodes().map(|(_, d, _)| d).max(),
            Some(MAX_PLACEMENT_NODE_DEPTH),
            "frill {frill}"
        );
        // Without a frill layer the last level comes from a group child; with one, the
        // frill layer root is level 1 and its first nested node level 2.
        assert_eq!(
            max_node,
            Some(MAX_PLACEMENT_NODE_DEPTH),
            "frill {frill}: deepest node child"
        );
        assert_eq!(s.objects().count(), 1);
        assert!(Scene::parse(&chain_with_object(MAX_PLACEMENT_NODE_DEPTH + 1, frill)).is_err());
    }
    // The two sites count the same way: a node one level below the frill layer root and a
    // group child's child are both level 2.
    let s = Scene::parse(&chain_with_object(2, true)).unwrap();
    let depths: Vec<(SceneSite, usize, u8)> = s
        .descendants()
        .map(|c| (c.site, c.depth, c.child.type_code()))
        .collect();
    assert_eq!(
        depths,
        [(SceneSite::FrillLayer, 2, 2), (SceneSite::FrillLayer, 3, 1)]
    );
    let s = Scene::parse(&chain_with_object(2, false)).unwrap();
    let depths: Vec<(SceneSite, usize, u8)> = s
        .descendants()
        .map(|c| (c.site, c.depth, c.child.type_code()))
        .collect();
    assert_eq!(
        depths,
        [
            (SceneSite::Group(0), 1, 2),
            (SceneSite::Group(0), 2, 2),
            (SceneSite::Group(0), 3, 1)
        ]
    );
}

#[test]
fn slot_lookup_uses_an_index_with_first_match_by_value() {
    // Regression: lookups scanned every pair, so one hostile entry with ~125k pairs made
    // each per-vertex lookup O(pairs). The table now keeps a sorted slot index; it must
    // give the same answers as a scan, including the first pair when a slot repeats.
    // (scene key i + 100, slot 3i), stored in descending slot order.
    let mut pairs: Vec<(u32, u32)> = (0..20_000u32).rev().map(|i| (i + 100, i * 3)).collect();
    pairs.push((7, 9)); // slot 9 again after (103, 9): never chosen
    pairs.insert(0, (5, 1)); // slot 1, first
    pairs.push((6, 1)); // slot 1 again, later: never chosen
    let b = B::new().u32(TS_DID).u32(0).u8(1);
    let b = terrain_entry(b, "default", 1, 0, Some((1, 0)), &[(11, 2), (12, 2)]);
    let b = b.u32(1);
    let data = terrain_entry(b, "big", 2, 7, Some((99, 0)), &pairs).build();
    let t = TerrainSceneTable::parse(&data).unwrap();
    assert_eq!(t.to_bytes(), data);
    let entry = t.entry(7).unwrap();
    for slot in (0..400).chain([59_997, 59_998, 1_000_000, u32::MAX]) {
        assert_eq!(
            t.pair_for(7, slot),
            entry.pair_for_slot(slot),
            "slot {slot}"
        );
        assert_eq!(
            t.pair_for(200, slot),
            t.default_entry().unwrap().pair_for_slot(slot),
            "default entry, slot {slot}"
        );
    }
    assert_eq!(t.scene_key(7, 1), Some(5));
    assert_eq!(t.scene_key(7, 9), Some(103));
    assert_eq!(t.scene_key(7, 2), Some(99)); // no pair: the entry's default pair
    assert_eq!(t.scene_key(200, 2), Some(11)); // default entry: first pair with slot 2
    assert_eq!(t.scene_key(200, 3), Some(1)); // default entry's default pair
    // A table built from parts indexes the same way.
    let rebuilt = TerrainSceneTable::new(
        Did(TS_DID),
        0,
        t.default_entry().cloned(),
        t.entries().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt, t);
    assert_eq!(rebuilt.scene_key(7, 1), Some(5));
}

#[test]
fn terrain_scene_names_must_round_trip() {
    // Regression: names longer than 127 bytes were cut and non-Latin-1 characters became
    // '?' when re-encoding, with no error from `new`.
    let entry = |name: String| TerrainSceneEntry {
        name,
        id: 3,
        pairs: vec![ScenePair {
            scene_key: 2,
            slot: 0,
        }],
        ..TerrainSceneEntry::default()
    };
    for bad in [
        "n".repeat(MAX_TERRAIN_ENTRY_NAME_LEN + 1),
        "n".repeat(200),
        "snow\u{2603}".to_string(),
    ] {
        let listed = TerrainSceneTable::new(Did(TS_DID), 0, None, vec![entry(bad.clone())]);
        assert!(
            matches!(listed, Err(FormatError::Malformed { .. })),
            "{bad:?}"
        );
        let default = TerrainSceneTable::new(Did(TS_DID), 0, Some(entry(bad.clone())), vec![]);
        assert!(
            matches!(default, Err(FormatError::Malformed { .. })),
            "{bad:?}"
        );
    }
    for good in [
        "n".repeat(MAX_TERRAIN_ENTRY_NAME_LEN),
        "Gr\u{fc}n \u{ff}".to_string(),
        String::new(),
    ] {
        let t = TerrainSceneTable::new(
            Did(TS_DID),
            0,
            Some(entry(good.clone())),
            vec![entry(good.clone())],
        )
        .unwrap();
        let back = TerrainSceneTable::parse(&t.to_bytes()).unwrap();
        assert_eq!(back, t, "{good:?}");
    }
}

#[test]
fn is_empty_counts_children_not_placement() {
    // A scene whose only children are in a group with id 2 has children, but the
    // generator places nothing from its groups (no group with id 1).
    let data = B::new()
        .u32(0)
        .u32(SCENE_DID)
        .u8(1)
        .u8(0)
        .u32(1)
        .u32(2)
        .u32(1)
        .u8(1)
        .u32(0x4700_0001)
        .build();
    let s = Scene::parse(&data).unwrap();
    assert!(!s.is_empty());
    assert!(s.placement_group().is_none());
}
