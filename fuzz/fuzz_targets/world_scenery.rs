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
//! work budget (a run stays bounded however many volumes it scans).
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
use tessera_formats::{Did, LandblockData};
use tessera_physics::geometry::{GridMesh, Plane};
use tessera_world::LandblockKey;
use tessera_world::frills::{DensityThinner, generate_sector};
use tessera_world::scenery::{
    ClearanceBox, ClearanceCylinder, FixedWater, GeneratedScenery, SceneryInput, SceneryLimits,
    SceneryTables, StaticClearance, generate_landblock,
};

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
        scale_mode: if bit(13) { b.u8() % 9 } else { 4 },
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

    let density = b.f32();
    let mut thinner = DensityThinner::with_seed(u64::from(b.u32()));
    for _ in 0..2 {
        let (sx, sy) = (b.u8() % 17, b.u8() % 17);
        let sector = generate_sector(&input, sx, sy, density, &mut thinner);
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
