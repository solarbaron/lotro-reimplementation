// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery generation and frill sectors (`docs/specs/scenery.md` §7, §8.2) from hostile
//! tables, scenes and landblock data built from arbitrary bytes: scene-reference cycles
//! and chains past the depth cap, huge fan-out and counts, arbitrary node floats and mode
//! bytes, NaN weights, water planes and clearance volumes, random heights, diagonals,
//! coarse words and quad materials.
//!
//! Nothing may panic or hang; the object and frill counts respect the limits; generating
//! the same landblock twice gives bit-identical output (SC8); every static object lies in
//! its block (0 ≤ x, y < 160, also for negative spacings); a static entry with a
//! non-finite rotation never changes the output; long static lists are charged to the
//! work budget (a run stays bounded however many volumes it scans); failing children
//! (missing scene references, scale modes above 8, §7.4 step 15) end their vertex cleanly;
//! the frill pass takes the static pass's footprint markers (§8.2 item 5).
//!
//! The hostile-input readers of the static list (§7.4 step 14a) are fuzzed too: clearance
//! lists built from arbitrary property values ([`volumes_from_values`]) and static lists
//! read from a landblock-info record of arbitrary entities with their own sets, 0x47 chains
//! (loops included) and game objects with `PhysObj` ([`static_clearances`]). Every list
//! ends at its first element that breaks the rule, skips non-finite elements, stores boxes
//! sorted, keeps every value finite and respects the per-entity and per-landblock caps.
//!
//! Run with e.g. `cargo fuzz run world_scenery -- -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::HeightGrid;
use tessera_formats::scenery::{
    FloatRange, PlacementNode, Scene, SceneChild, SceneChoice, SceneChoiceEntry, SceneChoiceTable,
    SceneGroup, ScenePair, TerrainSceneEntry, TerrainSceneTable,
};
use tessera_formats::setup::Quaternion;
use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
use tessera_formats::{
    Did, LandblockData, LandblockInfo, PropertyArray, PropertyEntry, PropertyStruct, PropertyType,
    PropertyValue, TSize,
};
use tessera_physics::geometry::{GridMesh, Plane};
use tessera_world::LandblockKey;
use tessera_world::frills::{DensityThinner, generate_sector};
use tessera_world::scenery::{
    ClearanceBox, ClearanceCylinder, ClearancePids, FixedWater, GeneratedScenery,
    MAX_STATIC_VOLUMES, MAX_VOLUMES_PER_ENTITY, ObjectPropertyCache, SceneryInput, SceneryLimits,
    SceneryTables, StaticClearance, generate_landblock, static_clearances, volumes_from_values,
};
use tessera_world::source::MemoryWorldSource;
use tessera_world::testkit::{self, TestEntity, TestValue};

/// Most scenes per input.
const SCENES: u32 = 8;
/// Most children per group or node.
const CHILDREN: usize = 12;
/// Most nesting levels of nodes built.
const DEPTH: usize = 4;

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn bool(&mut self) -> bool {
        self.u8() & 1 == 1
    }
    /// A float: often a small "interesting" value, sometimes raw bits (NaN, ±∞, huge).
    fn f32(&mut self) -> f32 {
        const PICKS: [f32; 16] = [
            0.0, 1.0, -1.0, 0.5, 0.0002, -0.0002, 5.0, -5.0, 359.0, 0.766, 0.9848, 1e30, -1e30,
            10.0, 160.0, 0.1,
        ];
        let tag = self.u8();
        if tag < 200 {
            PICKS[usize::from(tag % 16)]
        } else if tag < 240 {
            f32::from(self.u16()) / 256.0 - 128.0
        } else {
            f32::from_bits(self.u32())
        }
    }
    fn finite(&mut self) -> f32 {
        let v = self.f32();
        if v.is_finite() { v } else { 1.0 }
    }
    fn range(&mut self) -> FloatRange {
        FloatRange {
            min: self.finite(),
            max: self.finite(),
        }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.finite(), self.finite(), self.finite()]
    }
}

fn scene_did(b: &mut Bytes<'_>) -> Did {
    Did(0x0200_0000 + u32::from(b.u8()) % (SCENES + 1))
}

fn children(b: &mut Bytes<'_>, depth: usize) -> Vec<SceneChild> {
    let n = usize::from(b.u8()) % (CHILDREN + 1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(match b.u8() % 3 {
            0 => SceneChild::Object(Did(0x4700_0000 + u32::from(b.u8()))),
            1 if depth < DEPTH => SceneChild::Node(Box::new(node(b, depth + 1))),
            _ => SceneChild::Scene(scene_did(b)),
        });
    }
    out
}

fn node(b: &mut Bytes<'_>, depth: usize) -> PlacementNode {
    let flags = b.u16();
    let bit = |k: u32| flags & (1 << k) != 0;
    PlacementNode {
        disp_min: b.v3(),
        disp_max: b.v3(),
        heading: b.range(),
        angle2: b.range(),
        angle3: b.range(),
        frequency: b.finite(),
        scale_min: b.v3(),
        scale_max: b.v3(),
        stretch_ref: 1.0,
        slope_max_cos: b.finite(),
        slope_min_cos: b.finite(),
        align_to_slope: bit(0),
        face_direction: bit(1),
        above_ground: b.range(),
        above_water: b.range(),
        below_water: b.range(),
        water_depth: b.range(),
        below_ground_min: b.finite(),
        z_range: FloatRange {
            min: if bit(2) {
                f32::NEG_INFINITY
            } else {
                b.finite()
            },
            max: if bit(3) { f32::INFINITY } else { b.finite() },
        },
        spacing: b.finite(),
        clearance: b.finite(),
        clearance_scaled: bit(4),
        clear_max_cos: b.finite(),
        clear_min_cos: b.finite(),
        clear_min_dot: b.finite(),
        clear_aspect_tol: b.finite(),
        max_objects: u32::from(b.u8() % 4),
        unknown_163: 1.0,
        disp_scaled: bit(5),
        inherit_scale: bit(6),
        disp_relative: bit(7),
        orient_relative: bit(8),
        spacing_scaled: bit(9),
        no_checks: bit(10),
        spacing_3d: bit(11),
        unknown_174: false,
        no_spacing: bit(12),
        height_mode: b.u8() % 12,
        scale_mode: if bit(13) { b.u8() } else { 4 },
        above_ground_mode: b.u8() % 4,
        above_water_mode: b.u8() % 4,
        below_water_mode: b.u8() % 4,
        water_depth_mode: b.u8() % 4,
        below_ground_mode: b.u8() % 4,
        children: children(b, depth),
        below_ground_max: b.finite(),
    }
}

fn tables(b: &mut Bytes<'_>) -> SceneryTables {
    let pair = |b: &mut Bytes<'_>| ScenePair {
        scene_key: u32::from(b.u8() % 6),
        slot: u32::from(b.u8() % 4),
    };
    let entry = |b: &mut Bytes<'_>, id: u32| {
        let n = usize::from(b.u8() % 5);
        TerrainSceneEntry {
            name: String::new(),
            kind: 2,
            id,
            unknown_f: std::f32::consts::FRAC_1_SQRT_2,
            default_pair: b.bool().then(|| pair(b)),
            pairs: (0..n).map(|_| pair(b)).collect(),
        }
    };
    let default = b.bool().then(|| entry(b, 0));
    let listed = (1..=u32::from(b.u8() % 6)).map(|id| entry(b, id)).collect();
    let ts = TerrainSceneTable::new(Did(0x1200_0001), 0, default, listed).expect("0x12 parts");
    let choices = (0..6u32)
        .map(|key| SceneChoiceEntry {
            key,
            choices: (0..b.u8() % 4)
                .map(|_| SceneChoice {
                    scene: scene_did(b),
                    weight: b.f32(),
                    flag: 1,
                })
                .collect(),
        })
        .collect();
    let sc = SceneChoiceTable::new(0, Did(0x1100_0001), choices).expect("0x11 parts");
    let tt = b.bool().then(|| {
        let n = u32::from(b.u8() % 4) + 1;
        let entries = (0..n)
            .map(|key| TerrainType {
                key,
                procedural: b.bool(),
                ..TerrainType::default()
            })
            .collect();
        Arc::new(TerrainTypeTable::new(Did(0x1900_0001), entries).expect("0x19 parts"))
    });
    SceneryTables::new(1, Arc::new(ts), Arc::new(sc), tt)
}

fn data(b: &mut Bytes<'_>, key: LandblockKey) -> LandblockData {
    let mode = b.u8();
    let base = b.u16();
    let step = b.u16();
    let heights = HeightGrid::from_fn(|i, j| match mode % 3 {
        0 => base,
        #[allow(clippy::cast_possible_truncation)]
        1 => base.wrapping_add(step.wrapping_mul((i * 33 + j) as u16)),
        #[allow(clippy::cast_possible_truncation)]
        _ => base ^ ((i * 7919 + j * 104_729) as u16).wrapping_mul(step),
    });
    let mut d = LandblockData::from_heights(key.data_did(), heights);
    let fl = b.u8();
    for (k, f) in d.vertex_flags.raw_mut().iter_mut().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let k = k as u8;
        *f = if k.wrapping_mul(fl) & 1 == 1 { 0x80 } else { 0 };
    }
    let words: Vec<u32> = (0..4)
        .map(|_| u32::from(b.u8() % 7) | (u32::from(b.u8() % 5) << 8))
        .collect();
    for (k, w) in d.coarse.iter_mut().enumerate() {
        *w = words[k % words.len()];
    }
    if b.bool() {
        let m = b.u8() % 6;
        d.terrain.quads = Some(Box::new(std::array::from_fn(|k| {
            #[allow(clippy::cast_possible_truncation)]
            let v = (k as u16).wrapping_mul(u16::from(m)) % 6;
            v
        })));
    }
    d
}

// Clearance PIDs of the synthetic schema (`testkit::schema` plus these).
const PID_BOXES: u32 = 0x1000_7002;
const PID_BOX: u32 = 0x1000_7003;
const PID_MIN: u32 = 0x1000_7004;
const PID_MAX: u32 = 0x1000_7005;
const PID_CYLS: u32 = 0x1000_7006;
const PID_CYL: u32 = 0x1000_7007;
const PID_LOW: u32 = 0x1000_7008;
const PID_RADIUS: u32 = 0x1000_7009;
const PID_HEIGHT: u32 = 0x1000_700A;
const PID_CTYPE: u32 = 0x1000_700B;

fn clearance_schema() -> HashMap<u32, PropertyType> {
    let mut s = testkit::schema();
    for (pid, code) in [
        (PID_BOXES, 12),
        (PID_BOX, 11),
        (PID_MIN, 6),
        (PID_MAX, 6),
        (PID_CYLS, 12),
        (PID_CYL, 11),
        (PID_LOW, 6),
        (PID_RADIUS, 9),
        (PID_HEIGHT, 9),
        (PID_CTYPE, 8),
    ] {
        s.insert(pid, PropertyType::from_code(code));
    }
    s
}

fn clearance_pids(b: &mut Bytes<'_>) -> ClearancePids {
    ClearancePids {
        box_list: Some(PID_BOXES),
        cylinder_list: Some(PID_CYLS),
        box_min: Some(PID_MIN),
        box_max: Some(PID_MAX),
        low_point: Some(PID_LOW),
        radius: Some(PID_RADIUS),
        height: b.bool().then_some(PID_HEIGHT),
        phys_obj: Some(testkit::pid::PHYS_OBJ),
        creation_type: b.bool().then_some(PID_CTYPE),
    }
}

/// A raw float for a clearance value: often small, sometimes NaN or ±∞.
fn hostile(b: &mut Bytes<'_>) -> f32 {
    match b.u8() % 8 {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => f32::NEG_INFINITY,
        _ => b.f32(),
    }
}

fn hostile_v3(b: &mut Bytes<'_>) -> [f32; 3] {
    [hostile(b), hostile(b), hostile(b)]
}

/// One list element as a property value: usually a well-formed struct, sometimes a field
/// missing, of the wrong type, or not a struct at all.
fn element_value(b: &mut Bytes<'_>, cylinder: bool) -> PropertyValue {
    let field = |pid, value| PropertyEntry { pid, value };
    let mut fields = if cylinder {
        vec![
            field(PID_LOW, PropertyValue::Vector3(hostile_v3(b))),
            field(PID_RADIUS, PropertyValue::Float(hostile(b))),
            field(PID_HEIGHT, PropertyValue::Float(hostile(b))),
        ]
    } else {
        vec![
            field(PID_MIN, PropertyValue::Vector3(hostile_v3(b))),
            field(PID_MAX, PropertyValue::Vector3(hostile_v3(b))),
        ]
    };
    match b.u8() % 10 {
        0 => return PropertyValue::Float(hostile(b)),
        1 => {
            let k = usize::from(b.u8()) % fields.len();
            fields.remove(k);
        }
        2 => {
            let k = usize::from(b.u8()) % fields.len();
            if let Some(f) = fields.get_mut(k) {
                f.value = PropertyValue::Int(i32::from(b.u8()));
            }
        }
        _ => {}
    }
    PropertyValue::Struct(PropertyStruct {
        size: TSize::default(),
        fields,
    })
}

fn list_value(b: &mut Bytes<'_>, cylinder: bool) -> PropertyValue {
    if b.u8().is_multiple_of(8) {
        return PropertyValue::Float(1.0);
    }
    // Sometimes longer than the per-entity cap.
    let n = if b.u8().is_multiple_of(16) {
        MAX_VOLUMES_PER_ENTITY + usize::from(b.u8())
    } else {
        usize::from(b.u8() % 8)
    };
    let pid = if cylinder { PID_CYL } else { PID_BOX };
    let elements = (0..n)
        .map(|_| PropertyEntry {
            pid,
            value: element_value(b, cylinder),
        })
        .collect::<Vec<_>>();
    PropertyValue::Array(PropertyArray {
        count: u32::try_from(elements.len()).unwrap_or(u32::MAX),
        elements,
    })
}

/// How many leading elements of a list meet the rule of §7.4 step 14a (an upper bound of
/// the volumes read from it).
fn leading_valid(v: &PropertyValue, pids: &ClearancePids, cylinder: bool) -> usize {
    let Some(a) = v.as_array() else { return 0 };
    let has = |s: &PropertyStruct, pid: Option<u32>, vector: bool| {
        pid.and_then(|p| s.get(p)).is_some_and(|x| {
            if vector {
                matches!(x, PropertyValue::Vector3(_))
            } else {
                matches!(x, PropertyValue::Float(_))
            }
        })
    };
    a.elements
        .iter()
        .take_while(|e| {
            e.value.as_struct().is_some_and(|s| {
                if cylinder {
                    has(s, pids.low_point, true)
                        && has(s, pids.radius, false)
                        && has(s, pids.height, false)
                } else {
                    has(s, pids.box_min, true) && has(s, pids.box_max, true)
                }
            })
        })
        .count()
}

fn check_volumes(boxes: &[ClearanceBox], cylinders: &[ClearanceCylinder]) {
    assert!(boxes.len() <= MAX_VOLUMES_PER_ENTITY && cylinders.len() <= MAX_VOLUMES_PER_ENTITY);
    for bx in boxes {
        assert!(bx.min.iter().chain(&bx.max).all(|c| c.is_finite()));
        assert!(
            (0..3).all(|k| bx.min[k] <= bx.max[k]),
            "box stored unsorted"
        );
    }
    for c in cylinders {
        assert!(c.low_point.iter().all(|v| v.is_finite()));
        assert!(c.radius.is_finite() && c.height.is_finite());
    }
}

/// The readers of §7.4 step 14a on hostile values and records.
fn fuzz_static_list(b: &mut Bytes<'_>, key: LandblockKey) {
    let pids = clearance_pids(b);
    // Lists as property values.
    let boxes = list_value(b, false);
    let cylinders = list_value(b, true);
    let (vb, vc) = volumes_from_values(Some(&boxes), Some(&cylinders), &pids);
    check_volumes(&vb, &vc);
    assert!(vb.len() <= leading_valid(&boxes, &pids, false));
    assert!(vc.len() <= leading_valid(&cylinders, &pids, true));

    // Lists read from a landblock-info record through the lookup of
    // `entity-descriptor.md` §4.9.
    let schema = clearance_schema();
    let tv_list = |b: &mut Bytes<'_>, cylinder: bool| -> TestValue {
        let n = usize::from(b.u8() % 6);
        let elements = (0..n)
            .map(|_| {
                if cylinder {
                    let mut f = vec![
                        (PID_LOW, TestValue::Vec3(hostile_v3(b))),
                        (PID_RADIUS, TestValue::Float(hostile(b))),
                    ];
                    if !b.u8().is_multiple_of(4) {
                        f.push((PID_HEIGHT, TestValue::Float(hostile(b))));
                    }
                    (PID_CYL, TestValue::Struct(f))
                } else if b.u8().is_multiple_of(5) {
                    (PID_BOX, TestValue::Float(1.0))
                } else {
                    (
                        PID_BOX,
                        TestValue::Struct(vec![
                            (PID_MIN, TestValue::Vec3(hostile_v3(b))),
                            (PID_MAX, TestValue::Vec3(hostile_v3(b))),
                        ]),
                    )
                }
            })
            .collect();
        TestValue::Array(elements)
    };
    let own = |b: &mut Bytes<'_>| -> Vec<(u32, TestValue)> {
        let mut e = Vec::new();
        if b.bool() {
            e.push((PID_BOXES, tv_list(b, false)));
        }
        if b.bool() {
            e.push((PID_CYLS, tv_list(b, true)));
        }
        if b.bool() {
            e.push((PID_CTYPE, TestValue::Enum(u32::from(b.u8() % 12))));
        }
        e
    };
    let mut src = MemoryWorldSource::new();
    // Two 0x47 objects whose templates may loop, and a game object naming one by `PhysObj`.
    let (o1, o2) = (0x4700_0001u32, 0x4700_0002u32);
    let t1 = if b.bool() { o2 } else { 0x1F00_0001 };
    let e1 = own(b);
    src.insert(o1, testkit::entity_descriptor(o1, t1, &e1, &schema), None);
    let e2 = own(b);
    src.insert(o2, testkit::entity_descriptor(o2, o1, &e2, &schema), None);
    let mut g = own(b);
    if b.bool() {
        g.push((
            testkit::pid::PHYS_OBJ,
            TestValue::Did(if b.bool() { o1 } else { 0x7000_0001 }),
        ));
    }
    src.insert(
        0x7900_0001u32,
        testkit::property_set_record(0x7900_0001, &g),
        None,
    );
    let n = usize::from(b.u8() % 6);
    let mut entities: Vec<TestEntity> = (0..n)
        .map(|k| {
            let kind = [6, 7, 23, 9, 0][usize::from(b.u8() % 5)];
            let object = match b.u8() % 5 {
                0 => None,
                1 => Some(0),
                2 => Some(0x7000_0001),
                3 => Some(o2),
                _ => Some(o1),
            };
            let position = b.bool().then(|| hostile_v3(b));
            let rotation = [hostile(b), hostile(b), hostile(b), hostile(b)];
            TestEntity {
                kind,
                serial: u32::try_from(k).unwrap_or(0),
                object,
                position,
                rotation,
                properties: own(b),
                interior: false,
            }
        })
        .collect();
    // Sometimes many entities with long lists: the landblock cap must hold.
    if b.u8().is_multiple_of(16) {
        let one = (
            PID_BOX,
            TestValue::Struct(vec![
                (PID_MIN, TestValue::Vec3([-1.0; 3])),
                (PID_MAX, TestValue::Vec3([1.0; 3])),
            ]),
        );
        let list = vec![(PID_BOXES, TestValue::Array(vec![one; 300]))];
        for k in 0..70 {
            entities.push(TestEntity::physics(100 + k, o1, [10.0; 3], list.clone()));
        }
    }
    let bytes = testkit::landblock_info(key, &entities);
    let Ok(info) = LandblockInfo::parse(&bytes, Some(31), &schema) else {
        return;
    };
    let cache = ObjectPropertyCache::default();
    let list = static_clearances(&info, &src, &schema, &pids, &cache)
        .expect("a memory source has no failures outside the data");
    let total: usize = list.iter().map(StaticClearance::volume_count).sum();
    assert!(total <= MAX_STATIC_VOLUMES);
    for s in &list {
        assert!(!s.is_empty());
        assert!(s.position.iter().all(|c| c.is_finite()));
        assert!(s.rotation.to_wxyz().iter().all(|c| c.is_finite()));
        check_volumes(&s.boxes, &s.cylinders);
    }
    let again = static_clearances(&info, &src, &schema, &pids, &cache).expect("cached");
    assert_eq!(list, again);
}

fn bits(g: &GeneratedScenery) -> Vec<u32> {
    g.objects
        .iter()
        .flat_map(|o| {
            o.position
                .into_iter()
                .chain(o.rotation.to_wxyz())
                .chain(o.scale)
                .map(f32::to_bits)
                .chain([o.object.0])
        })
        .collect()
}

fuzz_target!(|input: &[u8]| {
    let mut b = Bytes(input);
    let key = LandblockKey::new(1 + b.u8() % 6, b.u8() % 255, b.u8() % 255).expect("key");
    if b.u8() % 4 == 0 {
        fuzz_static_list(&mut b, key);
        return;
    }
    let tables = tables(&mut b);
    let mut scenes: HashMap<Did, Arc<Scene>> = HashMap::new();
    for k in 0..=SCENES {
        if k > 0 && !b.bool() {
            continue;
        }
        let did = Did(0x0200_0000 + k);
        let group = children(&mut b, 0);
        let frill_layer = b.bool().then(|| node(&mut b, 1));
        scenes.insert(
            did,
            Arc::new(Scene {
                reserved: 0,
                did,
                flag_a: true,
                frill_layer,
                groups: vec![SceneGroup {
                    group_id: 1,
                    children: group,
                }],
            }),
        );
    }
    let data = data(&mut b, key);
    let terrain = GridMesh::terrain(&data);
    let water = FixedWater(b.bool().then(|| Plane {
        normal: [b.f32(), b.f32(), b.f32()],
        d: b.f32(),
    }));
    let mut statics: Vec<StaticClearance> = (0..b.u8() % 3)
        .map(|_| StaticClearance {
            position: [b.f32(), b.f32(), b.f32()],
            rotation: Quaternion {
                w: b.f32(),
                x: b.f32(),
                y: b.f32(),
                z: b.f32(),
            },
            boxes: vec![ClearanceBox {
                min: [b.f32(), b.f32(), b.f32()],
                max: [b.f32(), b.f32(), b.f32()],
            }],
            cylinders: vec![ClearanceCylinder {
                low_point: [b.f32(), b.f32(), b.f32()],
                radius: b.f32(),
                height: b.f32(),
            }],
        })
        .collect();
    // Sometimes a long static list far from the block: it only costs work.
    if b.u8() % 8 == 0 {
        let n = usize::from(b.u8()) * 4;
        let boxes = usize::from(b.u8() % 32);
        let far = StaticClearance {
            position: [1e4, 1e4, 0.0],
            rotation: Quaternion::IDENTITY,
            boxes: vec![
                ClearanceBox {
                    min: [-1.0; 3],
                    max: [1.0; 3],
                };
                boxes
            ],
            cylinders: Vec::new(),
        };
        statics.extend(std::iter::repeat_n(far, n));
    }
    let limits = SceneryLimits {
        max_scene_depth: usize::from(b.u8() % 20),
        max_work_per_vertex: 1 << 12,
        max_work_per_landblock: 1 << 16,
        max_objects_per_vertex: 1 << 10,
        max_objects_per_landblock: 1 << 14,
        max_markers: 1 << 12,
    };
    let input = SceneryInput {
        key,
        data: &data,
        terrain: &terrain,
        tables: &tables,
        scenes: &scenes,
        water: &water,
        statics: &statics,
        limits,
    };
    let first = generate_landblock(&input);
    assert!(first.objects.len() <= limits.max_objects_per_landblock as usize);
    assert!(first.stats.is_complete());
    assert!(
        first
            .objects
            .iter()
            .all(|o| (0.0..160.0).contains(&o.position[0]) && (0.0..160.0).contains(&o.position[1])),
        "a static object outside its block"
    );
    let second = generate_landblock(&input);
    assert_eq!(bits(&first), bits(&second));
    assert_eq!(first.stats, second.stats);

    // A static entry with a non-finite rotation overlaps nothing. It adds to the cost of
    // each scan, so compare only runs that no budget cut.
    if !first.stats.truncated() {
        let mut more = statics.clone();
        more.push(StaticClearance {
            position: [80.0, 80.0, 0.0],
            rotation: Quaternion {
                w: f32::NAN,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            boxes: vec![ClearanceBox {
                min: [-1e3; 3],
                max: [1e3; 3],
            }],
            cylinders: vec![ClearanceCylinder {
                low_point: [0.0; 3],
                radius: 1e3,
                height: 1.0,
            }],
        });
        let third = generate_landblock(&SceneryInput {
            statics: &more,
            ..input
        });
        if !third.stats.truncated() {
            assert_eq!(bits(&first), bits(&third));
        }
    }

    assert!(first.markers.len() <= limits.max_markers as usize);
    let markers = first.spacing_markers();
    assert!(markers.iter().all(|m| !m.open && m.radius > 0.0));

    let density = b.f32();
    let mut thinner = DensityThinner::with_seed(u64::from(b.u32()));
    for _ in 0..2 {
        let (sx, sy) = (b.u8() % 17, b.u8() % 17);
        let sector = generate_sector(&input, &markers, sx, sy, density, &mut thinner);
        assert!(sector.instances.len() <= 4 * limits.max_objects_per_vertex as usize);
        if sx < 16 && sy < 16 {
            assert!(sector.key.is_some());
            assert!(
                sector
                    .instances
                    .iter()
                    .all(|i| (0.0..160.0).contains(&i.position[0])
                        && (0.0..160.0).contains(&i.position[1]))
            );
        } else {
            assert!(sector.key.is_none() && sector.instances.is_empty());
        }
    }
});
