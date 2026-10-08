// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the scenery module: the worked example of `scenery.md` §14, SC8, SC11,
//! SC12 and the rules of §7.4 on synthetic fixtures.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::landblock::{HeightGrid, VertexFlags};
use tessera_formats::scenery::{
    FloatRange, PlacementNode, Scene, SceneChild, SceneChoice, SceneChoiceEntry, SceneChoiceTable,
    SceneGroup, ScenePair, TerrainSceneEntry, TerrainSceneTable,
};
use tessera_formats::setup::Quaternion;
use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
use tessera_formats::{Did, LandblockData, PropertyEntry, PropertyValue};
use tessera_physics::geometry::{GridMesh, Plane};

use super::hash::{self, SALT_HEADING};
use super::*;
use crate::coords::LandblockKey;
use crate::frills::{DensityThinner, generate_sector};
use crate::source::MemoryWorldSource;
use crate::testkit;

const OBJ_X: Did = Did(0x4700_0010);
const OBJ_Y: Did = Did(0x4700_0011);
const OBJ_ALIGN: Did = Did(0x4700_0020);
const OBJ_PLAIN: Did = Did(0x4700_0021);
const EMPTY: Did = Did(0x0200_0000);
const VERTEX_SCENE: Did = Did(0x0200_1000);

fn key() -> LandblockKey {
    LandblockKey::new(2, 48, 80).expect("key")
}

fn word(terrain: u32, slot: u32) -> u32 {
    terrain | (slot << 8)
}

fn approx(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn assert_q(q: Quaternion, e: [f32; 4], tol: f32) {
    let got = q.to_wxyz();
    assert!(
        got.iter().zip(e).all(|(a, b)| approx(*a, b, tol)),
        "{got:?} vs {e:?}"
    );
}

/// A node with the install's common values: always placed, no displacement, scale 1,
/// any slope, any height above ground, no clearance test, spacing 0.1.
fn node(children: Vec<SceneChild>) -> PlacementNode {
    PlacementNode {
        frequency: 1.0,
        scale_min: [1.0; 3],
        scale_max: [1.0; 3],
        stretch_ref: 1.0,
        slope_max_cos: 1.0,
        slope_min_cos: 0.0,
        above_ground: FloatRange {
            min: -0.0002,
            max: 0.0002,
        },
        above_water: FloatRange {
            min: -0.0002,
            max: 0.0002,
        },
        below_water: FloatRange {
            min: -0.0002,
            max: 0.0002,
        },
        water_depth: FloatRange {
            min: -0.0002,
            max: 0.0002,
        },
        below_ground_min: -0.0002,
        below_ground_max: 0.0002,
        z_range: FloatRange {
            min: -f32::MAX,
            max: f32::MAX,
        },
        spacing: 0.1,
        clearance_scaled: true,
        clear_max_cos: 1.0,
        clear_min_cos: 6.1e-17,
        clear_min_dot: -1.0,
        clear_aspect_tol: 360.0,
        unknown_163: 1.0,
        disp_scaled: true,
        inherit_scale: true,
        disp_relative: true,
        orient_relative: true,
        spacing_scaled: true,
        scale_mode: 4,
        above_ground_mode: 1,
        above_water_mode: 1,
        below_water_mode: 1,
        water_depth_mode: 1,
        below_ground_mode: 2,
        children,
        ..PlacementNode::default()
    }
}

/// The node of the §14 example: displacement ±5, heading 0–359, scale 0.8–1.2.
fn example_node(children: Vec<SceneChild>) -> PlacementNode {
    PlacementNode {
        disp_min: [-5.0, -5.0, 0.0],
        disp_max: [5.0, 5.0, 0.0],
        heading: FloatRange {
            min: 0.0,
            max: 359.0,
        },
        scale_min: [0.8; 3],
        scale_max: [1.2; 3],
        ..node(children)
    }
}

fn scene(did: Did, children: Vec<SceneChild>, frill: Option<PlacementNode>) -> Scene {
    Scene {
        reserved: 0,
        did,
        flag_a: true,
        frill_layer: frill,
        groups: vec![SceneGroup {
            group_id: 1,
            children,
        }],
    }
}

/// Region tables: terrain id 5 slot 1 → key 10 → choices (EMPTY 1.0, VERTEX_SCENE 1.0,
/// EMPTY 2.0), so the §14 vertex picks VERTEX_SCENE; keys 1 and 2 → EMPTY. Terrain type
/// 1 forbids procedural content.
fn tables() -> SceneryTables {
    let one = ScenePair {
        scene_key: 1,
        slot: 0,
    };
    let default = TerrainSceneEntry {
        name: "default".into(),
        kind: 1,
        id: 0,
        unknown_f: std::f32::consts::FRAC_1_SQRT_2,
        default_pair: Some(one),
        pairs: vec![one],
    };
    let listed = TerrainSceneEntry {
        name: "zone".into(),
        kind: 2,
        id: 5,
        unknown_f: std::f32::consts::FRAC_1_SQRT_2,
        default_pair: None,
        pairs: vec![
            ScenePair {
                scene_key: 2,
                slot: 0,
            },
            ScenePair {
                scene_key: 10,
                slot: 1,
            },
        ],
    };
    let ts = TerrainSceneTable::new(Did(0x1200_0001), 0, Some(default), vec![listed]).unwrap();
    let choice = |scene, weight| SceneChoice {
        scene,
        weight,
        flag: 1,
    };
    let entries = vec![
        SceneChoiceEntry {
            key: 1,
            choices: vec![choice(EMPTY, 1.0)],
        },
        SceneChoiceEntry {
            key: 2,
            choices: vec![choice(EMPTY, 1.0)],
        },
        SceneChoiceEntry {
            key: 10,
            choices: vec![
                choice(EMPTY, 1.0),
                choice(VERTEX_SCENE, 1.0),
                choice(EMPTY, 2.0),
            ],
        },
    ];
    let sc = SceneChoiceTable::new(0, Did(0x1100_0001), entries).unwrap();
    let tt = TerrainTypeTable::new(
        Did(0x1900_0001),
        vec![
            TerrainType {
                key: 0,
                procedural: true,
                ..TerrainType::default()
            },
            TerrainType {
                key: 1,
                procedural: false,
                ..TerrainType::default()
            },
        ],
    )
    .unwrap();
    SceneryTables::new(2, Arc::new(ts), Arc::new(sc), Some(Arc::new(tt)))
}

fn scenes(list: Vec<Scene>) -> HashMap<Did, Arc<Scene>> {
    let mut m: HashMap<Did, Arc<Scene>> = list.into_iter().map(|s| (s.did, Arc::new(s))).collect();
    m.entry(EMPTY)
        .or_insert_with(|| Arc::new(scene(EMPTY, vec![], None)));
    m
}

/// Flat data (height 0) with the scenery word only at vertex (3, 7).
fn data_one_vertex() -> LandblockData {
    let mut d = LandblockData::from_heights(key().data_did(), HeightGrid::flat(0));
    d.coarse[17 * 3 + 7] = word(5, 1);
    d
}

struct Fixture {
    data: LandblockData,
    terrain: GridMesh,
    tables: SceneryTables,
    scenes: HashMap<Did, Arc<Scene>>,
    statics: Vec<StaticClearance>,
    water: FixedWater,
    limits: SceneryLimits,
}

impl Fixture {
    fn new(data: LandblockData, list: Vec<Scene>) -> Self {
        let terrain = GridMesh::terrain(&data);
        Self {
            data,
            terrain,
            tables: tables(),
            scenes: scenes(list),
            statics: Vec::new(),
            water: FixedWater(None),
            limits: SceneryLimits::default(),
        }
    }

    fn input(&self) -> SceneryInput<'_> {
        SceneryInput {
            key: key(),
            data: &self.data,
            terrain: &self.terrain,
            tables: &self.tables,
            scenes: &self.scenes,
            water: &self.water,
            statics: &self.statics,
            limits: self.limits,
        }
    }

    fn run(&self) -> GeneratedScenery {
        generate_landblock(&self.input())
    }

    /// The static pass's markers, as the frill pass takes them (§8.2 item 5).
    fn markers(&self) -> Vec<FootprintMarker> {
        self.run().spacing_markers()
    }

    /// The frill pass of vertex (i, j) after the landblock's static pass.
    fn frills(&self, i: usize, j: usize) -> (Vec<FrillRecord>, SceneryStats) {
        frill_vertex(&self.input(), &self.markers(), i, j)
    }

    /// Frill sector (sx, sy) after the landblock's static pass.
    fn sector(
        &self,
        sx: u8,
        sy: u8,
        density: f32,
        thinner: &mut DensityThinner,
    ) -> crate::frills::FrillSector {
        generate_sector(&self.input(), &self.markers(), sx, sy, density, thinner)
    }
}

// ---------------------------------------------------------------------------------------
// §14 worked example
// ---------------------------------------------------------------------------------------

#[test]
fn worked_example_hashes() {
    let (a, b) = (hash::seed_a(2, 48, 80), hash::seed_b(3, 7));
    assert_eq!(b, 0x0035_5731);
    assert_eq!(a, 135);
    // Scene pick.
    let (gx, gy) = (16 * 48 + 3, 16 * 80 + 7);
    assert_eq!((gx, gy), (771, 1287));
    assert_eq!(hash::vertex_hash(gx, gy), 0x7237_E00C);
    let u = hash::vertex_fraction(gx, gy);
    assert_eq!(u, f32::from(0x7237u16) / 65_536.0);
    assert!(approx(u, 0.44615, 1e-5));
    let t = tables();
    let entry = t.scene_choices.get(10).unwrap();
    assert!(approx(u * entry.total_weight(), 1.7846, 1e-4));
    assert_eq!(entry.choose(u).unwrap().scene, VERTEX_SCENE);
    assert_eq!(t.choose_scene(48, 80, 3, 7, word(5, 1)), Some(VERTEX_SCENE));
    // Seeds.
    let s = hash::scene_seed(hash::SCENE_SEED);
    assert_eq!(s, 0x876B_E061);
    let c = hash::child_seed(s, 0);
    assert_eq!(c, 0x876B_E061);
    // Frequency.
    assert_eq!(c.wrapping_add(hash::SALT_FREQUENCY), 0x8884_FEC8);
    assert_eq!(hash::node_hash(a, b, 0x8884_FEC8), 0xEC94_9086);
    let freq = hash::node_random(a, b, c.wrapping_add(hash::SALT_FREQUENCY));
    assert!(approx(freq, 0.924_133, 1e-6));
    // Displacement, heading, scale.
    let rx = hash::node_random(a, b, c.wrapping_add(hash::SALT_DISP_X));
    let ry = hash::node_random(a, b, c.wrapping_add(hash::SALT_DISP_Y));
    assert!(approx(rx, 0.611_084, 1e-6));
    assert!(approx(ry, 0.134_033, 1e-6));
    assert!(approx(-5.0 + rx * 10.0, 1.110_84, 1e-5));
    assert!(approx(-5.0 + ry * 10.0, -3.659_67, 1e-5));
    let rh = hash::node_random(a, b, c.wrapping_add(SALT_HEADING));
    assert!(approx(rh, 0.233_948, 1e-6));
    assert!(approx(rh * 359.0, 83.99, 0.005));
    let rs = hash::node_random(a, b, c.wrapping_add(hash::SALT_SCALE));
    assert!(approx(rs, 0.306_381, 1e-6));
    assert!(approx(0.8 + rs * 0.4, 0.922_55, 1e-5));
    // Second child and frill layer.
    let c1 = hash::child_seed(s, 1);
    assert_eq!(c1, 0xB4AD_9F7A);
    assert!(approx(
        hash::node_random(a, b, c1.wrapping_add(hash::SALT_FREQUENCY)),
        0.752_548,
        1e-6
    ));
    assert_eq!(s.wrapping_add(hash::FRILL_LAYER_SALT), 0x8AEC_236B);
}

#[test]
fn worked_example_orientation() {
    let h = 0.233_948f32 * 359.0;
    let q = math::rot_z(-h);
    assert_q(q, [0.74322, 0.0, 0.0, -0.66905], 2e-5);
    let fwd = q.rotate([0.0, 1.0, 0.0]);
    assert!(approx(fwd[0], 0.99450, 2e-5) && approx(fwd[1], 0.10475, 2e-5));
    let bearing = fwd[0].atan2(fwd[1]).to_degrees();
    assert!(approx(bearing, 83.987, 0.001));
    assert!(approx(q.heading_degrees(), 83.987, 0.001));

    let n = [0.2f32, -0.3, 0.93274];
    let qa = math::align_to_normal(n);
    assert_q(qa, [0.98292, 0.15418, 0.09929, 0.01558], 2e-5);
    let x = qa.rotate([1.0, 0.0, 0.0]);
    let y = qa.rotate([0.0, 1.0, 0.0]);
    assert!(approx(x[0], 0.97980, 2e-5) && approx(x[1], 0.06124, 2e-5));
    assert!(approx(x[2], -0.19039, 2e-5));
    assert!(approx(y[0], 0.0, 2e-5) && approx(y[1], 0.95197, 2e-5));
    assert!(approx(y[2], 0.30619, 2e-5));
    let full = math::mul(qa, q);
    assert_q(full, [0.74094, 0.04816, 0.17695, -0.64604], 2e-5);
    let z = full.rotate([0.0, 0.0, 1.0]);
    for k in 0..3 {
        assert!(approx(z[k], n[k], 1e-4), "{z:?}");
    }
}

#[test]
fn worked_example_plane_band_and_diagonal_tie() {
    let data = LandblockData::from_heights(key().data_did(), HeightGrid::flat(0));
    let mesh = GridMesh::terrain(&data);
    let clear = VertexFlags::filled(0);
    assert_eq!(mesh.client_key_at(17.5, 37.0, &clear), Some(455));
    assert_eq!(mesh.client_key_at(16.0, 36.0, &clear), Some(454));
    assert_eq!(mesh.client_key_at(19.0, 38.5, &clear), Some(455));
    let set = VertexFlags::filled(VertexFlags::SPLIT_SW_NE);
    assert_eq!(mesh.client_key_at(17.5, 37.5, &set), Some(454));
    assert_eq!(mesh.key_under(17.5, 37.5, &set), Some(455));
}

#[test]
fn worked_example_clearance_query() {
    assert_eq!(GridMesh::client_keys_near(12.0, 13.0, 0.5), [133]);
    assert_eq!(GridMesh::client_keys_near(12.0, 12.2, 0.5), [132, 133]);
    assert_eq!(GridMesh::client_keys_near(13.0, 12.0, 0.5), [132]);
}

#[test]
fn worked_example_placement() {
    let f = Fixture::new(
        data_one_vertex(),
        vec![scene(
            VERTEX_SCENE,
            vec![SceneChild::Node(Box::new(example_node(vec![
                SceneChild::Object(OBJ_X),
            ])))],
            None,
        )],
    );
    let g = f.run();
    // Words of 0 select the default entry, whose key 1 names the empty scene (§4.2).
    assert_eq!(g.stats.vertices_with_scene, 289);
    assert_eq!(g.objects.len(), 1, "{:?}", g.stats);
    let o = g.objects[0];
    assert_eq!(o.object, OBJ_X);
    assert_eq!(o.vertex, [3, 7]);
    let base = [3.0 * (10.0 - 0.0002f32), 7.0 * (10.0 - 0.0002f32)];
    assert!(approx(o.position[0], base[0] + 1.110_84, 2e-4), "{o:?}");
    assert!(approx(o.position[1], base[1] - 3.659_67, 2e-4), "{o:?}");
    assert!(approx(o.position[2], 0.0, 1e-6));
    assert_q(o.rotation, [0.74322, 0.0, 0.0, -0.66905], 2e-5);
    for s in o.scale {
        assert!(approx(s, 0.922_55, 1e-5));
    }
    assert_eq!(g.stats.nodes_placed, 1);
}

#[test]
fn frequency_below_the_draw_skips_the_node() {
    let mk = |freq| {
        Fixture::new(
            data_one_vertex(),
            vec![scene(
                VERTEX_SCENE,
                vec![SceneChild::Node(Box::new(PlacementNode {
                    frequency: freq,
                    ..example_node(vec![SceneChild::Object(OBJ_X)])
                }))],
                None,
            )],
        )
        .run()
    };
    let skipped = mk(0.9);
    assert!(skipped.objects.is_empty());
    assert_eq!(skipped.stats.rejections(Rejection::Frequency), 1);
    assert_eq!(mk(1.0).objects.len(), 1);
    assert_eq!(mk(0.93).objects.len(), 1);
}

// ---------------------------------------------------------------------------------------
// SC11 and SC8
// ---------------------------------------------------------------------------------------

/// Uneven terrain with mixed diagonals and a scene word on every vertex.
fn rough_data() -> LandblockData {
    let heights = HeightGrid::from_fn(|i, j| {
        #[allow(clippy::cast_possible_truncation)]
        let v = (i * 311 + j * 173 + (i * j * 37) % 251) as u16;
        v.wrapping_mul(7)
    });
    let mut d = LandblockData::from_heights(key().data_did(), heights);
    for (k, f) in d.vertex_flags.raw_mut().iter_mut().enumerate() {
        if (k * 7) % 3 == 0 {
            *f = VertexFlags::SPLIT_SW_NE;
        }
    }
    for w in d.coarse.iter_mut() {
        *w = word(5, 1);
    }
    d
}

fn sc11_scenes() -> Vec<Scene> {
    let align = PlacementNode {
        align_to_slope: true,
        no_spacing: true,
        ..example_node(vec![SceneChild::Object(OBJ_ALIGN)])
    };
    let plain = PlacementNode {
        no_spacing: true,
        ..example_node(vec![SceneChild::Object(OBJ_PLAIN)])
    };
    vec![scene(
        VERTEX_SCENE,
        vec![
            SceneChild::Node(Box::new(align)),
            SceneChild::Node(Box::new(plain)),
        ],
        None,
    )]
}

#[test]
fn sc11_orientations() {
    let f = Fixture::new(rough_data(), sc11_scenes());
    let g = f.run();
    let a = hash::seed_a(2, 48, 80);
    let s = hash::scene_seed(hash::SCENE_SEED);
    let (mut aligned, mut plain) = (0, 0);
    for o in &g.objects {
        assert!(math::is_unit(o.rotation, 1e-5), "{o:?}");
        let b = hash::seed_b(u32::from(o.vertex[0]), u32::from(o.vertex[1]));
        if o.object == OBJ_ALIGN {
            let n = f
                .terrain
                .client_plane_at(o.position[0], o.position[1], &f.data.vertex_flags)
                .normal;
            assert!(n[2] < 1.0, "terrain is not flat");
            let z = o.rotation.rotate([0.0, 0.0, 1.0]);
            for k in 0..3 {
                assert!(approx(z[k], n[k], 1e-4), "{z:?} vs {n:?}");
            }
            aligned += 1;
        } else {
            let c = hash::child_seed(s, 1);
            let h = hash::node_random(a, b, c.wrapping_add(SALT_HEADING)) * 359.0;
            let y = o.rotation.rotate([0.0, 1.0, 0.0]);
            let bearing = y[0].atan2(y[1]).to_degrees().rem_euclid(360.0);
            assert!(
                math::bearing_difference(bearing, h) < 0.01,
                "{bearing} vs {h}"
            );
            plain += 1;
        }
    }
    // About a quarter of the vertices pick VERTEX_SCENE (weights 1, 1, 2).
    assert!(aligned > 40 && plain > 40, "{aligned} {plain}");
}

#[test]
fn sc8_deterministic() {
    let f = Fixture::new(rough_data(), sc11_scenes());
    let a = f.run();
    let b = f.run();
    assert_eq!(a, b);
    let bits = |g: &GeneratedScenery| {
        g.objects
            .iter()
            .flat_map(|o| {
                o.position
                    .into_iter()
                    .chain(o.rotation.to_wxyz())
                    .chain(o.scale)
                    .map(f32::to_bits)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&a), bits(&b));
}

// ---------------------------------------------------------------------------------------
// §7.4 rules
// ---------------------------------------------------------------------------------------

fn one_node(n: PlacementNode) -> Vec<Scene> {
    vec![scene(
        VERTEX_SCENE,
        vec![SceneChild::Node(Box::new(n))],
        None,
    )]
}

#[test]
fn terrain_type_forbids_procedural_content() {
    let mut data = data_one_vertex();
    data.terrain.quads = Some(Box::new([1u16; 1024]));
    let f = Fixture::new(
        data,
        one_node(example_node(vec![SceneChild::Object(OBJ_X)])),
    );
    let g = f.run();
    assert!(g.objects.is_empty());
    assert_eq!(g.stats.rejections(Rejection::TerrainType), 1);
    // `no_checks` skips the test.
    let mut data = data_one_vertex();
    data.terrain.quads = Some(Box::new([1u16; 1024]));
    let f = Fixture::new(
        data,
        one_node(PlacementNode {
            no_checks: true,
            ..example_node(vec![SceneChild::Object(OBJ_X)])
        }),
    );
    assert_eq!(f.run().objects.len(), 1);
    // A quad value that is not a key is ignored.
    let mut data = data_one_vertex();
    data.terrain.quads = Some(Box::new([7u16; 1024]));
    let f = Fixture::new(
        data,
        one_node(example_node(vec![SceneChild::Object(OBJ_X)])),
    );
    assert_eq!(f.run().objects.len(), 1);
}

#[test]
fn slope_bounds() {
    let f = Fixture::new(
        rough_data(),
        one_node(PlacementNode {
            slope_min_cos: 1.0,
            ..example_node(vec![SceneChild::Object(OBJ_X)])
        }),
    );
    let g = f.run();
    assert!(g.objects.is_empty());
    assert!(g.stats.rejections(Rejection::Slope) > 0);
}

#[test]
fn water_height_mode_and_ranges() {
    let n = PlacementNode {
        height_mode: 1,
        ..example_node(vec![SceneChild::Object(OBJ_X)])
    };
    let mut f = Fixture::new(data_one_vertex(), one_node(n));
    let g = f.run();
    assert!(g.objects.is_empty());
    assert_eq!(g.stats.rejections(Rejection::NoWater), 1);
    f.water = FixedWater(Some(level_plane(3.5)));
    let g = f.run();
    assert_eq!(g.objects.len(), 1);
    assert!(approx(g.objects[0].position[2], 3.5, 1e-6));

    // `water_depth_mode` 0 refuses water-covered ground; a dry landblock passes.
    let n = PlacementNode {
        water_depth_mode: 0,
        ..example_node(vec![SceneChild::Object(OBJ_X)])
    };
    let mut f = Fixture::new(data_one_vertex(), one_node(n));
    assert_eq!(f.run().objects.len(), 1);
    f.water = FixedWater(Some(level_plane(2.0)));
    let g = f.run();
    assert!(g.objects.is_empty());
    assert_eq!(g.stats.rejections(Rejection::HeightRange), 1);
    // Water below the ground is not "water over ground".
    f.water = FixedWater(Some(level_plane(-2.0)));
    assert_eq!(f.run().objects.len(), 1);
}

#[test]
fn height_offset_sinks_objects_and_ranges_apply() {
    let n = PlacementNode {
        disp_min: [-5.0, -5.0, -2.0],
        disp_max: [5.0, 5.0, -2.0],
        ..example_node(vec![SceneChild::Object(OBJ_X)])
    };
    // Below ground by 2 with `below_ground_mode` 2 and (−0.0002, 0.0002): rejected.
    let f = Fixture::new(data_one_vertex(), one_node(n.clone()));
    assert!(f.run().objects.is_empty());
    // Mode 1 allows any depth.
    let f = Fixture::new(
        data_one_vertex(),
        one_node(PlacementNode {
            below_ground_mode: 1,
            ..n
        }),
    );
    let g = f.run();
    assert_eq!(g.objects.len(), 1);
    assert!(approx(g.objects[0].position[2], -2.0, 1e-6));
}

#[test]
fn footprints_space_later_nodes() {
    let n = |no_spacing| PlacementNode {
        spacing: 1.0,
        no_spacing,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let two = |a, b| {
        vec![scene(
            VERTEX_SCENE,
            vec![
                SceneChild::Node(Box::new(n(a))),
                SceneChild::Node(Box::new(n(b))),
            ],
            None,
        )]
    };
    let g = Fixture::new(data_one_vertex(), two(false, false)).run();
    assert_eq!(g.objects.len(), 1);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
    let g = Fixture::new(data_one_vertex(), two(false, true)).run();
    assert_eq!(g.objects.len(), 2);
    // A nested node is not spaced against its own (still open) ancestor.
    let nested = PlacementNode {
        spacing: 1.0,
        ..node(vec![SceneChild::Node(Box::new(n(false)))])
    };
    let g = Fixture::new(data_one_vertex(), one_node(nested)).run();
    assert_eq!(g.objects.len(), 1);
}

#[test]
fn static_clearance_volumes_space_nodes() {
    let mut f = Fixture::new(
        data_one_vertex(),
        one_node(node(vec![SceneChild::Object(OBJ_X)])),
    );
    let p = [3.0 * (10.0 - 0.0002f32), 7.0 * (10.0 - 0.0002f32), 0.0];
    f.statics = vec![StaticClearance {
        position: [p[0] + 1.0, p[1], 0.0],
        rotation: Quaternion::IDENTITY,
        boxes: Vec::new(),
        cylinders: vec![ClearanceCylinder {
            low_point: [0.0; 3],
            radius: 0.95,
            height: 2.0,
        }],
    }];
    let g = f.run();
    assert!(g.objects.is_empty());
    assert_eq!(g.stats.rejections(Rejection::StaticSpacing), 1);
    // A box under a rotated entity frame: the point maps inside it.
    f.statics = vec![StaticClearance {
        position: [p[0] + 3.0, p[1], 0.0],
        rotation: math::rot_z(90.0),
        boxes: vec![ClearanceBox {
            min: [-1.0, 2.5, -1.0],
            max: [1.0, 3.5, 1.0],
        }],
        cylinders: Vec::new(),
    }];
    assert!(f.run().objects.is_empty());
    // A degenerate box is skipped.
    f.statics[0].boxes[0] = ClearanceBox {
        min: [0.0; 3],
        max: [0.0001; 3],
    };
    f.statics[0].position = p;
    assert_eq!(f.run().objects.len(), 1);
}

#[test]
fn clearance_requires_flat_consistent_ground() {
    let n = PlacementNode {
        clearance: 3.0,
        clear_min_cos: 0.9999,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(rough_data(), one_node(n.clone())).run();
    assert!(g.objects.is_empty());
    assert!(g.stats.rejections(Rejection::Clearance) > 0);
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_eq!(g.objects.len(), 1);
    // Pairwise test on rough ground.
    let n = PlacementNode {
        clearance: 3.0,
        clear_min_dot: 0.99999,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(rough_data(), one_node(n)).run();
    assert!(g.stats.rejections(Rejection::Clearance) > 0);
}

#[test]
fn outside_block_and_unsupported_scale_modes() {
    let n = PlacementNode {
        disp_min: [-100.0, 0.0, 0.0],
        disp_max: [-100.0, 0.0, 0.0],
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_eq!(g.stats.rejections(Rejection::Outside), 1);
    let n = PlacementNode {
        scale_mode: 5,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_eq!(g.stats.rejections(Rejection::ScaleMode), 1);
    assert_eq!(g.stats.failed_vertices, 0);
}

#[test]
fn scale_modes_above_eight() {
    // §7.4 steps 3 and 15 (SC21): 9 or higher with differing limits the node fails; 10 or
    // higher with equal limits takes `scale_min`. Mode 9 with equal limits is adjusted by
    // step 11 in the client; Tessera does not implement that adjustment and rejects the
    // node (Tessera Decision of step 3).
    let n = |mode, max: f32| PlacementNode {
        scale_mode: mode,
        scale_min: [0.5; 3],
        scale_max: [max; 3],
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let list = |mode, max| {
        vec![scene(
            VERTEX_SCENE,
            vec![
                SceneChild::Node(Box::new(n(mode, max))),
                SceneChild::Object(OBJ_Y),
            ],
            None,
        )]
    };
    for mode in [9u8, 12, 255] {
        let g = Fixture::new(data_one_vertex(), list(mode, 0.8)).run();
        assert!(g.objects.is_empty(), "mode {mode}: the vertex is abandoned");
        assert_eq!(g.stats.failed_vertices, 1);
        assert_eq!(g.stats.rejections(Rejection::ScaleMode), 0);
    }
    let g = Fixture::new(data_one_vertex(), list(12, 0.5)).run();
    assert_eq!(g.objects.len(), 2);
    assert!(approx(g.objects[0].scale[0], 0.5, 1e-6));
    assert_eq!(g.stats.failed_vertices, 0);
    let g = Fixture::new(data_one_vertex(), list(9, 0.5)).run();
    assert_eq!(g.stats.rejections(Rejection::ScaleMode), 1);
    assert_eq!(g.objects.len(), 1);
    // The frequency test comes first: a skipped node does not fail.
    let skipped = PlacementNode {
        frequency: 0.0,
        ..n(12, 0.8)
    };
    let g = Fixture::new(data_one_vertex(), one_node(skipped)).run();
    assert_eq!(g.stats.failed_vertices, 0);
    assert_eq!(g.stats.rejections(Rejection::Frequency), 1);
}

#[test]
fn scene_references_and_child_seeds() {
    // A group object sits at the vertex frame; a referenced scene's node gets the seed of
    // the reference (C_k multiplied again).
    let inner = Did(0x0200_2000);
    let f = Fixture::new(
        data_one_vertex(),
        vec![
            scene(
                VERTEX_SCENE,
                vec![SceneChild::Object(OBJ_Y), SceneChild::Scene(inner)],
                None,
            ),
            scene(
                inner,
                vec![SceneChild::Node(Box::new(example_node(vec![
                    SceneChild::Object(OBJ_X),
                ])))],
                None,
            ),
        ],
    );
    let g = f.run();
    assert_eq!(g.objects.len(), 2);
    assert_eq!(g.objects[0].object, OBJ_Y);
    assert_eq!(g.objects[0].rotation, Quaternion::IDENTITY);
    let (a, b) = (hash::seed_a(2, 48, 80), hash::seed_b(3, 7));
    let c = hash::scene_seed(hash::child_seed(hash::scene_seed(hash::SCENE_SEED), 1));
    let h = hash::node_random(a, b, c.wrapping_add(SALT_HEADING)) * 359.0;
    assert!(approx(g.objects[1].rotation.heading_degrees(), h, 0.01));
}

#[test]
fn max_objects_stops_children() {
    let n = PlacementNode {
        max_objects: 2,
        ..node(vec![
            SceneChild::Object(OBJ_X),
            SceneChild::Object(OBJ_X),
            SceneChild::Object(OBJ_X),
        ])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_eq!(g.objects.len(), 2);
}

#[test]
fn hostile_cycles_depth_and_fanout_are_bounded() {
    // A scene that references itself.
    let g = Fixture::new(
        data_one_vertex(),
        vec![scene(
            VERTEX_SCENE,
            vec![SceneChild::Scene(VERTEX_SCENE), SceneChild::Object(OBJ_X)],
            None,
        )],
    )
    .run();
    assert_eq!(g.stats.cycles, 1);
    assert_eq!(g.objects.len(), 1);
    // A chain deeper than the cap.
    let mut list = Vec::new();
    for k in 0..40u32 {
        let did = Did(VERTEX_SCENE.0 + k);
        list.push(scene(
            did,
            vec![SceneChild::Scene(Did(did.0 + 1)), SceneChild::Object(OBJ_X)],
            None,
        ));
    }
    let g = Fixture::new(data_one_vertex(), list).run();
    assert_eq!(g.stats.depth_capped, 1);
    assert_eq!(g.objects.len(), DEFAULT_MAX_SCENE_DEPTH);
    // Exponential fan-out: 300³ objects requested on every vertex.
    let (q, z) = (Did(0x0200_3000), Did(0x0200_3001));
    let mut f = Fixture::new(
        rough_data(),
        vec![
            scene(VERTEX_SCENE, vec![SceneChild::Scene(q); 300], None),
            scene(q, vec![SceneChild::Scene(z); 300], None),
            scene(z, vec![SceneChild::Object(OBJ_X); 300], None),
        ],
    );
    f.limits.max_work_per_landblock = 1 << 20;
    let start = std::time::Instant::now();
    let g = f.run();
    assert!(g.stats.budget_cut > 0);
    assert!(g.stats.truncated());
    assert!(g.objects.len() <= f.limits.max_objects_per_landblock as usize);
    assert!(start.elapsed().as_secs() < 30);
}

/// Region tables like [`tables`], with terrain id 5 slot 1 → key 10 → `a` alone and slot 2
/// → key 11 → `b` alone, so `word(5, 1)` always picks `a` and `word(5, 2)` always `b`.
fn two_scene_tables(a: Did, b: Did) -> SceneryTables {
    let one = ScenePair {
        scene_key: 1,
        slot: 0,
    };
    let default = TerrainSceneEntry {
        name: "default".into(),
        kind: 1,
        id: 0,
        unknown_f: std::f32::consts::FRAC_1_SQRT_2,
        default_pair: Some(one),
        pairs: vec![one],
    };
    let listed = TerrainSceneEntry {
        name: "zone".into(),
        kind: 2,
        id: 5,
        unknown_f: std::f32::consts::FRAC_1_SQRT_2,
        default_pair: None,
        pairs: vec![
            ScenePair {
                scene_key: 2,
                slot: 0,
            },
            ScenePair {
                scene_key: 10,
                slot: 1,
            },
            ScenePair {
                scene_key: 11,
                slot: 2,
            },
        ],
    };
    let ts = TerrainSceneTable::new(Did(0x1200_0001), 0, Some(default), vec![listed]).unwrap();
    let only = |key, scene| SceneChoiceEntry {
        key,
        choices: vec![SceneChoice {
            scene,
            weight: 1.0,
            flag: 1,
        }],
    };
    let entries = vec![only(1, EMPTY), only(2, EMPTY), only(10, a), only(11, b)];
    let sc = SceneChoiceTable::new(0, Did(0x1100_0001), entries).unwrap();
    let mut t = tables();
    t.terrain_scenes = Arc::new(ts);
    t.scene_choices = Arc::new(sc);
    t
}

/// Flat data with `word(5, 1)` at vertex (3, 7) and `word(5, 2)` at (4, 7).
fn data_two_vertices() -> LandblockData {
    let mut d = LandblockData::from_heights(key().data_did(), HeightGrid::flat(0));
    d.coarse[17 * 3 + 7] = word(5, 1);
    d.coarse[17 * 4 + 7] = word(5, 2);
    d
}

/// A node at a fixed offset `(dx, dy)` from its parent with `spacing`.
fn at(dx: f32, dy: f32, spacing: f32, children: Vec<SceneChild>) -> PlacementNode {
    PlacementNode {
        disp_min: [dx, dy, 0.0],
        disp_max: [dx, dy, 0.0],
        spacing,
        ..node(children)
    }
}

#[test]
fn sc16_open_ancestors_skipped_closed_siblings_tested() {
    let n3 = || at(0.0, 0.0, 1.0, vec![SceneChild::Object(OBJ_Y)]);
    let n2 = |no_spacing| PlacementNode {
        no_spacing,
        ..at(
            0.0,
            0.0,
            1.0,
            vec![SceneChild::Object(OBJ_X), SceneChild::Node(Box::new(n3()))],
        )
    };
    let n1 = || at(0.0, 0.0, 1.0, vec![SceneChild::Object(OBJ_ALIGN)]);
    let run = |children: Vec<PlacementNode>| {
        let list = vec![scene(
            VERTEX_SCENE,
            children
                .into_iter()
                .map(|n| SceneChild::Node(Box::new(n)))
                .collect(),
            None,
        )];
        let g = Fixture::new(data_one_vertex(), list).run();
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>()
    };
    // (a) N1's closed marker rejects N2, so neither N2's object nor N3 appears.
    assert_eq!(run(vec![n1(), n2(false)]), [OBJ_ALIGN]);
    // (b) Without N1, N3 is not spaced against its open parent N2.
    assert_eq!(run(vec![n2(false)]), [OBJ_X, OBJ_Y]);
    // (c) `no_spacing` on N2 skips its test, and N3 inherits it.
    assert_eq!(run(vec![n1(), n2(true)]), [OBJ_ALIGN, OBJ_X, OBJ_Y]);
}

#[test]
fn sc17_markers_of_earlier_vertices_take_part() {
    // Vertex (3, 7) places A's node at its base; vertex (4, 7) places B's node 0.15 east
    // of it (base x 29.9994 and 39.9992).
    let (a, b) = (Did(0x0200_8000), Did(0x0200_8001));
    let run = |spacing| {
        let mut f = Fixture::new(
            data_two_vertices(),
            vec![
                scene(
                    a,
                    vec![SceneChild::Node(Box::new(at(
                        0.0,
                        0.0,
                        spacing,
                        vec![SceneChild::Object(OBJ_X)],
                    )))],
                    None,
                ),
                scene(
                    b,
                    vec![SceneChild::Node(Box::new(at(
                        -9.8498,
                        0.0,
                        spacing,
                        vec![SceneChild::Object(OBJ_Y)],
                    )))],
                    None,
                ),
            ],
        );
        f.tables = two_scene_tables(a, b);
        f.run()
    };
    let g = run(0.1);
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X]
    );
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
    let g = run(0.05);
    assert_eq!(g.objects.len(), 2);
    assert!(approx(
        g.objects[1].position[0] - g.objects[0].position[0],
        0.15,
        1e-4
    ));
}

#[test]
fn sc18_group_nodes_meet_their_static_twin_in_the_frill_pass() {
    let (x, y) = (Did(0x0200_8100), Did(0x0200_8101));
    let grass = || PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Scene(y)])
    };
    let run = |g_no_spacing| {
        let g = PlacementNode {
            no_spacing: g_no_spacing,
            ..at(0.0, 0.0, 0.1, vec![SceneChild::Scene(x)])
        };
        let f = Fixture::new(
            data_one_vertex(),
            vec![
                scene(
                    VERTEX_SCENE,
                    vec![SceneChild::Node(Box::new(g))],
                    Some(grass()),
                ),
                scene(x, vec![], Some(grass())),
                scene(
                    y,
                    vec![SceneChild::Node(Box::new(node(vec![SceneChild::Object(
                        OBJ_Y,
                    )])))],
                    None,
                ),
            ],
        );
        // The static pass places G (and nothing else).
        let st = f.run();
        assert_eq!(st.stats.nodes_placed, 1);
        assert!(st.objects.is_empty());
        let (records, _) = f.frills(3, 7);
        assert!(records.iter().all(|r| r.object.object == OBJ_Y && !r.late));
        records.len()
    };
    // G is rejected by its own static-pass marker: only V's own frill layer yields.
    assert_eq!(run(false), 1);
    assert_eq!(run(true), 2);
}

#[test]
fn frill_layer_nodes_meet_markers_of_later_vertices() {
    // Vertex (3, 7): scene A, empty group, frill layer node at the base (spacing 1).
    // Vertex (4, 7): scene B, a group node placed on (3, 7)'s base. In the frill pass of
    // (3, 7) the layer meets B's static-pass marker although B's vertex comes later.
    let (a, b) = (Did(0x0200_8200), Did(0x0200_8201));
    let run = |b_spacing| {
        let mut f = Fixture::new(
            data_two_vertices(),
            vec![
                scene(
                    a,
                    vec![],
                    Some(at(0.0, 0.0, 1.0, vec![SceneChild::Object(OBJ_Y)])),
                ),
                scene(
                    b,
                    vec![SceneChild::Node(Box::new(at(
                        -9.9998,
                        0.0,
                        b_spacing,
                        vec![SceneChild::Object(OBJ_X)],
                    )))],
                    None,
                ),
            ],
        );
        f.tables = two_scene_tables(a, b);
        assert_eq!(f.run().objects.len(), 1);
        f.frills(3, 7)
    };
    let (records, stats) = run(1.0);
    assert!(records.is_empty());
    assert_eq!(stats.rejections(Rejection::Footprint), 1);
    assert_eq!(run(0.0).0.len(), 1);
}

#[test]
fn sc19_a_failing_child_abandons_the_vertex_and_leaves_its_marker_open() {
    // Vertex (3, 7), scene A: N1 (3 east), N2 (at the base, child: a scene reference), N3
    // (3 west); every node has spacing 1. Vertex (4, 7), scene B: a node on (3, 7)'s base.
    let (a, b, empty_ok) = (Did(0x0200_8300), Did(0x0200_8301), Did(0x0200_8302));
    let missing = Did(0x0200_DEAD);
    let run = |reference| {
        let mut f = Fixture::new(
            data_two_vertices(),
            vec![
                scene(
                    a,
                    vec![
                        SceneChild::Node(Box::new(at(
                            3.0,
                            0.0,
                            1.0,
                            vec![SceneChild::Object(OBJ_X)],
                        ))),
                        SceneChild::Node(Box::new(at(
                            0.0,
                            0.0,
                            1.0,
                            vec![SceneChild::Scene(reference)],
                        ))),
                        SceneChild::Node(Box::new(at(
                            -3.0,
                            0.0,
                            1.0,
                            vec![SceneChild::Object(OBJ_Y)],
                        ))),
                    ],
                    None,
                ),
                scene(
                    b,
                    vec![SceneChild::Node(Box::new(at(
                        -9.9998,
                        0.0,
                        1.0,
                        vec![SceneChild::Object(OBJ_PLAIN)],
                    )))],
                    None,
                ),
                scene(empty_ok, vec![], None),
            ],
        );
        f.tables = two_scene_tables(a, b);
        f.run()
    };
    // Control: a readable reference closes N2's marker, which then rejects B's node.
    let g = run(empty_ok);
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X, OBJ_Y]
    );
    assert_eq!(g.stats.failed_vertices, 0);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
    // A reference to a DID that does not exist: only N1's subtree at (3, 7); N2's marker
    // stays open, so B's node at (4, 7) is placed.
    let g = run(missing);
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X, OBJ_PLAIN]
    );
    assert_eq!(g.stats.failed_vertices, 1);
    assert_eq!(g.stats.missing_scenes, 1);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 0);
    let open: Vec<_> = g.markers.iter().filter(|m| m.open).collect();
    assert_eq!(open.len(), 1);
    assert!(approx(open[0].position[0], 3.0 * 9.9998, 1e-4));
    assert!(!g.spacing_markers().iter().any(|m| m.open));
}

#[test]
fn a_failure_inside_a_frill_layer_stays_in_the_layer() {
    // `scenery.md` §7.4 step 15 (S38, S41): a failure never climbs out of a frill layer; the
    // scene that evaluates a frill layer ignores the layer's result. P's frill layer fails at
    // its first reference, so its second reference yields nothing; the later sibling Q
    // still yields its frill.
    let (p, q, y) = (Did(0x0200_8400), Did(0x0200_8401), Did(0x0200_8402));
    let missing = Did(0x0200_DEAD);
    let layer = |children| PlacementNode {
        no_spacing: true,
        ..node(children)
    };
    let f = |group: Vec<SceneChild>| {
        Fixture::new(
            data_one_vertex(),
            vec![
                scene(VERTEX_SCENE, group, None),
                scene(
                    p,
                    vec![],
                    Some(layer(vec![
                        SceneChild::Scene(missing),
                        SceneChild::Scene(y),
                    ])),
                ),
                scene(q, vec![], Some(layer(vec![SceneChild::Scene(y)]))),
                scene(y, vec![SceneChild::Object(OBJ_Y)], None),
            ],
        )
    };
    let fx = f(vec![SceneChild::Scene(p), SceneChild::Scene(q)]);
    assert_eq!(
        fx.run().stats.failed_vertices,
        0,
        "no frill layer in the static pass"
    );
    let (records, stats) = fx.frills(3, 7);
    assert_eq!(records.len(), 1);
    assert_eq!(stats.failed_frill_layers, 1);
    assert_eq!(stats.failed_vertices, 0);
    // A failure in a group still ends the vertex in the frill pass: Q is never reached.
    let fx = f(vec![
        SceneChild::Scene(q),
        SceneChild::Scene(missing),
        SceneChild::Scene(q),
    ]);
    let (records, stats) = fx.frills(3, 7);
    assert_eq!(records.len(), 1);
    assert_eq!(stats.failed_vertices, 1);
}

#[test]
fn the_frill_state_stays_set_after_a_failed_frill_layer() {
    // `scenery.md` §7.4 step 15 (S41): after a frill layer whose subtree failed, the
    // enclosing scenes continue as after a successful one and the "under a frill layer"
    // state stays set, so a later group object of the vertex still becomes a frill record
    // (§8.1, S26).
    let p = Did(0x0200_8410);
    let missing = Did(0x0200_DEAD);
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Scene(missing)])
    };
    let fx = Fixture::new(
        data_one_vertex(),
        vec![
            scene(
                VERTEX_SCENE,
                vec![SceneChild::Scene(p), SceneChild::Object(OBJ_Y)],
                None,
            ),
            scene(p, vec![], Some(layer)),
        ],
    );
    let (records, stats) = fx.frills(3, 7);
    assert_eq!(stats.failed_frill_layers, 1);
    assert_eq!(stats.failed_vertices, 0);
    assert_eq!(stats.frill_skipped, 0);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].object.object, OBJ_Y);
    assert!(records[0].late, "made after the frill layer, not inside it");
}

#[test]
fn missing_pairs_keys_and_scenes_are_counted() {
    let mut d = data_one_vertex();
    d.coarse[0] = word(5, 9); // slot 9 not in entry 5, no default pair
    d.coarse[1] = word(0, 4); // default entry, default pair (1, 0) → EMPTY
    let g = Fixture::new(d, vec![]).run();
    assert_eq!(g.stats.vertices_without_pair, 1);
    // VERTEX_SCENE is not in the scene source; every other vertex gets the empty scene.
    assert_eq!(g.stats.missing_scenes, 1);
    assert_eq!(g.stats.vertices_with_scene, 288);
    assert!(g.objects.is_empty());
}

// ---------------------------------------------------------------------------------------
// Frill pass (SC12)
// ---------------------------------------------------------------------------------------

#[test]
fn sc12_late_group_objects_are_not_frills() {
    let f_scene = Did(0x0200_4000);
    let g_scene = Did(0x0200_4001);
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Scene(g_scene)])
    };
    let fx = Fixture::new(
        data_one_vertex(),
        vec![
            scene(
                VERTEX_SCENE,
                vec![SceneChild::Scene(f_scene), SceneChild::Object(OBJ_X)],
                None,
            ),
            scene(f_scene, vec![], Some(layer)),
            scene(g_scene, vec![SceneChild::Object(OBJ_Y)], None),
        ],
    );
    // Static pass: X is an entity; the frill layer is ignored.
    let g = fx.run();
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X]
    );
    // Frill pass: Y inside the layer, X late.
    let (records, _) = fx.frills(3, 7);
    assert_eq!(records.len(), 2);
    assert_eq!((records[0].object.object, records[0].late), (OBJ_Y, false));
    assert_eq!((records[1].object.object, records[1].late), (OBJ_X, true));
    let sector = fx.sector(3, 7, 1.0, &mut DensityThinner::with_seed(1));
    assert_eq!(
        sector
            .instances
            .iter()
            .map(|i| i.object)
            .collect::<Vec<_>>(),
        [OBJ_Y]
    );
    assert_eq!(sector.late_discarded, 1);
    // Other sectors of the block see nothing.
    let other = fx.sector(4, 7, 1.0, &mut DensityThinner::with_seed(1));
    assert!(other.instances.is_empty());
}

#[test]
fn frill_density_thins() {
    let g_scene = Did(0x0200_4001);
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Scene(g_scene); 200])
    };
    let fx = Fixture::new(
        data_one_vertex(),
        vec![
            scene(VERTEX_SCENE, vec![], Some(layer)),
            scene(g_scene, vec![SceneChild::Object(OBJ_Y)], None),
        ],
    );
    let all = fx.sector(3, 7, 1.0, &mut DensityThinner::new());
    assert_eq!(all.instances.len(), 200);
    let half = fx.sector(3, 7, 0.5, &mut DensityThinner::with_seed(9));
    assert!(
        (60..140).contains(&half.instances.len()),
        "{}",
        half.instances.len()
    );
    assert_eq!(half.instances.len() + half.thinned as usize, 200);
}

// ---------------------------------------------------------------------------------------
// Property chain (§9.2) and the static list
// ---------------------------------------------------------------------------------------

const PID_F: u32 = 0x1000_7001;
const PID_BOXES: u32 = 0x1000_7002;
const PID_BOX: u32 = 0x1000_7003;
const PID_MIN: u32 = 0x1000_7004;
const PID_MAX: u32 = 0x1000_7005;
const PID_CYLS: u32 = 0x1000_7006;
const PID_CYL: u32 = 0x1000_7007;
const PID_LOW: u32 = 0x1000_7008;
const PID_RADIUS: u32 = 0x1000_7009;
const PID_HEIGHT: u32 = 0x1000_700A;
/// The creation type: the fixed id the validity check reads (`scenery.md` §7.4 step 14a).
const PID_CTYPE: u32 = CREATION_TYPE_PID;

fn schema() -> HashMap<u32, tessera_formats::PropertyType> {
    schema_with_ctype(8)
}

/// The synthetic schema with the creation type declared as property type `code` (8 enum,
/// 10 property ID, 15 int): values written with any test value of four bytes decode with
/// that type (the stored type of `property-system.md` §4.1 is the dictionary's).
fn schema_with_ctype(code: u32) -> HashMap<u32, tessera_formats::PropertyType> {
    let mut s = testkit::schema();
    for (pid, code) in [
        (PID_F, 9),
        (PID_BOXES, 12),
        (PID_BOX, 11),
        (PID_MIN, 6),
        (PID_MAX, 6),
        (PID_CYLS, 12),
        (PID_CYL, 11),
        (PID_LOW, 6),
        (PID_RADIUS, 9),
        (PID_HEIGHT, 9),
        (PID_CTYPE, code),
    ] {
        s.insert(pid, tessera_formats::PropertyType::from_code(code));
    }
    s
}

fn entity_with_extra(
    did: u32,
    parent: u32,
    entries: &[(u32, testkit::TestValue)],
    extra: &[u32],
    schema: &dyn tessera_formats::PropertySchema,
) -> Vec<u8> {
    let plain = testkit::entity_descriptor(did, parent, entries, schema);
    let mut desc = tessera_formats::EntityDescriptor::parse(&plain, schema).unwrap();
    desc.extra_sets = extra.iter().map(|d| Did(*d)).collect();
    desc.to_bytes_with(&testkit::property_set(entries)).unwrap()
}

#[test]
fn sc14_chain_lookup() {
    use testkit::TestValue as V;
    let s = schema();
    let mut src = MemoryWorldSource::new();
    let parent = 0x4700_0100;
    src.insert(
        parent,
        testkit::entity_descriptor(parent, 0x1F00_0001, &[(PID_F, V::Float(0.2))], &s),
        None,
    );
    src.insert(
        0x7800_0001u32,
        testkit::property_set_record(0x7800_0001, &[(PID_F, V::Float(0.3))]),
        None,
    );
    // (object, own entries, extra sets, template, expected value)
    type Case = (u32, Vec<(u32, V)>, Vec<u32>, u32, Option<f32>);
    let cases: [Case; 5] = [
        (0x4700_0101, vec![], vec![], parent, Some(0.2)),
        (
            0x4700_0102,
            vec![(PID_F, V::Float(0.1))],
            vec![],
            parent,
            Some(0.1),
        ),
        (
            0x4700_0103,
            vec![(PID_F, V::Float(0.1))],
            vec![0x7800_0001],
            parent,
            Some(0.1),
        ),
        (0x4700_0104, vec![], vec![0x7800_0001], parent, Some(0.3)),
        (0x4700_0105, vec![], vec![], 0x1F00_0001, None),
    ];
    for (did, entries, extra, tpl, _) in &cases {
        src.insert(
            *did,
            entity_with_extra(*did, *tpl, entries, extra, &s),
            None,
        );
    }
    let cache = ObjectPropertyCache::default();
    for (did, _, _, _, want) in &cases {
        let p = cache
            .get(&src, &s, Did(*did))
            .expect("read")
            .expect("object");
        assert_eq!(p.float(PID_F), *want, "{did:#x}");
        assert!(!p.incomplete);
    }
    assert_eq!(cache.len(), cases.len());
    // A looping chain is cut and reported.
    src.insert(
        0x4700_0200u32,
        testkit::entity_descriptor(0x4700_0200, 0x4700_0201, &[], &s),
        None,
    );
    src.insert(
        0x4700_0201u32,
        testkit::entity_descriptor(0x4700_0201, 0x4700_0200, &[], &s),
        None,
    );
    let p = ObjectProperties::load(&src, &s, Did(0x4700_0200)).unwrap();
    assert!(p.incomplete);
    assert_eq!(p.levels.len(), 2);
}

/// Every clearance PID of the synthetic schema.
fn all_clearance_pids() -> ClearancePids {
    ClearancePids {
        box_list: Some(PID_BOXES),
        cylinder_list: Some(PID_CYLS),
        box_min: Some(PID_MIN),
        box_max: Some(PID_MAX),
        low_point: Some(PID_LOW),
        radius: Some(PID_RADIUS),
        height: Some(PID_HEIGHT),
        phys_obj: Some(testkit::pid::PHYS_OBJ),
        creation_type: PID_CTYPE,
    }
}

fn entry(pid: u32, value: PropertyValue) -> PropertyEntry {
    PropertyEntry { pid, value }
}

fn strukt(fields: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(tessera_formats::PropertyStruct {
        size: tessera_formats::TSize::default(),
        fields,
    })
}

fn array(pid: u32, elements: Vec<PropertyValue>) -> PropertyValue {
    PropertyValue::Array(tessera_formats::PropertyArray {
        count: u32::try_from(elements.len()).unwrap(),
        elements: elements.into_iter().map(|v| entry(pid, v)).collect(),
    })
}

fn box_value(min: [f32; 3], max: [f32; 3]) -> PropertyValue {
    strukt(vec![
        entry(PID_MIN, PropertyValue::Vector3(min)),
        entry(PID_MAX, PropertyValue::Vector3(max)),
    ])
}

fn cyl_value(low: [f32; 3], radius: f32, height: Option<f32>) -> PropertyValue {
    let mut fields = vec![
        entry(PID_LOW, PropertyValue::Vector3(low)),
        entry(PID_RADIUS, PropertyValue::Float(radius)),
    ];
    if let Some(h) = height {
        fields.push(entry(PID_HEIGHT, PropertyValue::Float(h)));
    }
    strukt(fields)
}

#[test]
fn clearance_volumes_from_property_values() {
    // §7.4 step 14a: the first element that does not meet the rule ends the list; an
    // element holding a non-finite value is skipped and the list goes on (Tessera Decision
    // of §13.1); corners are sorted per axis.
    let pids = all_clearance_pids();
    let boxes = array(
        PID_BOX,
        vec![
            box_value([0.0; 3], [1.0; 3]),
            box_value([f32::NAN; 3], [1.0; 3]),
            box_value([1.0, -1.0, 0.0], [-1.0, 1.0, 2.0]),
            PropertyValue::Float(1.0),
            box_value([5.0; 3], [6.0; 3]),
        ],
    );
    let (b, c) = volumes_from_values(Some(&boxes), None, &pids);
    assert!(c.is_empty());
    assert_eq!(
        b,
        [
            ClearanceBox {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            ClearanceBox {
                min: [-1.0, -1.0, 0.0],
                max: [1.0, 1.0, 2.0],
            },
        ]
    );
    // A field of the wrong type ends the list too.
    let wrong = array(
        PID_BOX,
        vec![
            box_value([0.0; 3], [1.0; 3]),
            strukt(vec![
                entry(PID_MIN, PropertyValue::Float(0.0)),
                entry(PID_MAX, PropertyValue::Vector3([1.0; 3])),
            ]),
            box_value([0.0; 3], [2.0; 3]),
        ],
    );
    assert_eq!(volumes_from_values(Some(&wrong), None, &pids).0.len(), 1);
    // Not an array: no volumes.
    let scalar = PropertyValue::Float(1.0);
    assert!(volumes_from_values(Some(&scalar), Some(&scalar), &pids) == (vec![], vec![]));

    // Cylinders: a missing height ends the list; a non-finite height or radius skips the
    // element.
    let cyls = array(
        PID_CYL,
        vec![
            cyl_value([0.0; 3], 1.0, Some(2.0)),
            cyl_value([0.0; 3], 1.0, Some(f32::INFINITY)),
            cyl_value([0.0; 3], f32::NAN, Some(1.0)),
            cyl_value([1.0; 3], 0.5, Some(3.0)),
            cyl_value([0.0; 3], 1.0, None),
            cyl_value([9.0; 3], 1.0, Some(2.0)),
        ],
    );
    let (_, c) = volumes_from_values(None, Some(&cyls), &pids);
    assert_eq!(
        c,
        [
            ClearanceCylinder {
                low_point: [0.0; 3],
                radius: 1.0,
                height: 2.0,
            },
            ClearanceCylinder {
                low_point: [1.0; 3],
                radius: 0.5,
                height: 3.0,
            },
        ]
    );
    // A dictionary without `Entity_Clearance_Height`: no cylinder element can meet the rule.
    let no_height = ClearancePids {
        height: None,
        ..pids
    };
    assert!(
        volumes_from_values(None, Some(&cyls), &no_height)
            .1
            .is_empty()
    );
    // The per-entity cap stops reading.
    let many = array(
        PID_BOX,
        vec![box_value([0.0; 3], [1.0; 3]); MAX_VOLUMES_PER_ENTITY + 10],
    );
    assert_eq!(
        volumes_from_values(Some(&many), None, &pids).0.len(),
        MAX_VOLUMES_PER_ENTITY
    );
}

#[test]
fn sc22a_crossed_box_corners_are_sorted() {
    // SC22 (a): Box_Min (1, −1, 0), Box_Max (−1, 1, 2) at (40, 40, 0), identity rotation:
    // a candidate at (40.5, 40, z) with ρ = 0.1 is rejected, as for (−1, −1, 0) … (1, 1, 2).
    let crossed = ClearanceBox {
        min: [1.0, -1.0, 0.0],
        max: [-1.0, 1.0, 2.0],
    };
    let sorted = ClearanceBox::from_corners(crossed.min, crossed.max);
    assert_eq!(
        sorted,
        ClearanceBox {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 2.0],
        }
    );
    for b in [crossed, sorted] {
        let s = StaticClearance {
            position: [40.0, 40.0, 0.0],
            rotation: Quaternion::IDENTITY,
            boxes: vec![b],
            cylinders: Vec::new(),
        };
        for z in [0.0, 1.0, 50.0] {
            assert!(s.overlaps([40.5, 40.0, z], 0.1, false), "{b:?} z {z}");
        }
        assert!(s.overlaps([40.5, 40.0, 1.0], 0.1, true));
        assert!(!s.overlaps([41.2, 40.0, 1.0], 0.1, false));
    }
    // A box flat on every axis once sorted is skipped.
    let flat = StaticClearance {
        position: [0.0; 3],
        rotation: Quaternion::IDENTITY,
        boxes: vec![ClearanceBox {
            min: [0.0001; 3],
            max: [0.0; 3],
        }],
        cylinders: Vec::new(),
    };
    assert!(!flat.overlaps([0.0; 3], 1.0, false));
}

#[test]
fn sc23_cylinder_with_spacing_3d() {
    // SC23: low point (0, 0, 0), radius 1, height 2, identity frame at the origin, ρ = 0.5.
    let s = StaticClearance {
        position: [0.0; 3],
        rotation: Quaternion::IDENTITY,
        boxes: Vec::new(),
        cylinders: vec![ClearanceCylinder {
            low_point: [0.0; 3],
            radius: 1.0,
            height: 2.0,
        }],
    };
    assert!(s.overlaps([1.4, 0.0, 1.0], 0.5, true));
    assert!(s.overlaps([0.0, 0.0, 3.4], 0.5, true));
    assert!(
        !s.overlaps([1.5, 0.0, 1.0], 0.5, true),
        "2.25 is not < 2.25"
    );
    assert!(!s.overlaps([0.0, 0.0, -1.6], 0.5, true));
    assert!(s.overlaps([0.0, 0.0, -1.4], 0.5, true));
    // Without `spacing_3d` the test is horizontal and inclusive.
    assert!(s.overlaps([1.5, 0.0, 9.0], 0.5, false), "2.25 <= 2.25");
    assert!(!s.overlaps([1.6, 0.0, 1.0], 0.5, false));
    // Through generation: a node at the vertex (z = 0) with ρ = 0.5 and the cylinder's
    // low point straight above it (mapped z = −1.6 or −1.4). The 2D form overlaps in both
    // cases; with `spacing_3d` only the nearer one does.
    let [x, y] = base_xy();
    for (three_d, low, placed) in [
        (true, 1.6f32, 1usize),
        (true, 1.4, 0),
        (false, 1.6, 0),
        (false, 1.4, 0),
    ] {
        let n = PlacementNode {
            spacing: 0.5,
            spacing_3d: three_d,
            ..node(vec![SceneChild::Object(OBJ_X)])
        };
        let mut f = Fixture::new(data_one_vertex(), one_node(n));
        f.statics = vec![StaticClearance {
            position: [x, y, low],
            ..s.clone()
        }];
        let g = f.run();
        assert_eq!(g.objects.len(), placed, "3d {three_d} low {low}");
        assert_eq!(
            g.stats.rejections(Rejection::StaticSpacing),
            u32::from(placed == 0)
        );
    }
}

/// A landblock-info record of `entities` decoded with the synthetic schema.
fn info_of(entities: &[testkit::TestEntity]) -> tessera_formats::LandblockInfo {
    let bytes = testkit::landblock_info(key(), entities);
    tessera_formats::LandblockInfo::parse(&bytes, Some(31), &schema()).unwrap()
}

/// A clearance list of one box (−1, −1, 0) … (1, 1, 2), with `min` and `max` as stored.
fn one_box_list(min: [f32; 3], max: [f32; 3]) -> (u32, testkit::TestValue) {
    use testkit::TestValue as V;
    (
        PID_BOXES,
        V::Array(vec![(
            PID_BOX,
            V::Struct(vec![(PID_MIN, V::Vec3(min)), (PID_MAX, V::Vec3(max))]),
        )]),
    )
}

/// `Entity_CreationType` stored as `value` (its type is the schema's).
fn ctype(value: u32) -> (u32, testkit::TestValue) {
    (PID_CTYPE, testkit::TestValue::Enum(value))
}

#[test]
fn sc22_static_list_lookup_frames_and_list_ends() {
    use testkit::{TestEntity, TestValue as V};
    let sch = schema();
    let pids = all_clearance_pids();
    let tv_box = |min: [f32; 3], max: [f32; 3]| {
        (
            PID_BOX,
            V::Struct(vec![(PID_MIN, V::Vec3(min)), (PID_MAX, V::Vec3(max))]),
        )
    };
    let crossed = tv_box([1.0, -1.0, 0.0], [-1.0, 1.0, 2.0]);
    let obj = 0x4700_0400u32;
    let game = 0x7000_0401u32;
    let game_set = 0x7900_0401u32;
    let mut src = MemoryWorldSource::new();
    // The 0x47 object holds the crossed box and a creation type of class Dynamic.
    src.insert(
        obj,
        testkit::entity_descriptor(
            obj,
            0x1F00_0001,
            &[(PID_BOXES, V::Array(vec![crossed.clone()])), ctype(7)],
            &sch,
        ),
        None,
    );
    // The game object's set has no clearance list, only `PhysObj` and a creation type.
    src.insert(
        game_set,
        testkit::property_set_record(game_set, &[(testkit::pid::PHYS_OBJ, V::Did(obj)), ctype(7)]),
        None,
    );
    let cache = ObjectPropertyCache::default();
    let build = |entities: &[TestEntity]| {
        static_clearances(&info_of(entities), &src, &sch, &pids, &cache).unwrap()
    };

    // (a) Physics at (40, 40, 0) with the crossed box (and a creation type) in its own set.
    let a = build(&[TestEntity::physics(
        1,
        0x4700_0999,
        [40.0, 40.0, 0.0],
        vec![(PID_BOXES, V::Array(vec![crossed.clone()])), ctype(7)],
    )]);
    assert_eq!(a.len(), 1);
    assert_eq!(
        a[0].boxes,
        [ClearanceBox {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 2.0],
        }]
    );
    assert!(a[0].overlaps([40.5, 40.0, 0.0], 0.1, false));

    // (b) A Weenie whose game object's set has no list and whose `PhysObj` holds the box.
    let b = build(&[TestEntity {
        kind: 6,
        ..TestEntity::physics(2, game, [40.0, 40.0, 0.0], vec![])
    }]);
    assert_eq!(b.len(), 1, "the PhysObj chain supplies the box");
    assert!(b[0].overlaps([40.5, 40.0, 0.0], 0.1, false));

    // (c) A Physics entity without a frame: at the origin with the identity rotation.
    let c = build(&[TestEntity {
        position: None,
        ..TestEntity::physics(3, obj, [0.0; 3], vec![])
    }]);
    assert_eq!(c.len(), 1, "a frameless entity is kept");
    assert_eq!(c[0].position, [0.0; 3]);
    assert_eq!(c[0].rotation, Quaternion::IDENTITY);
    assert!(c[0].overlaps([0.5, 0.5, 0.0], 0.1, false));

    // (d) A list whose second element lacks Box_Max: only the first box exists.
    let second = (PID_BOX, V::Struct(vec![(PID_MIN, V::Vec3([10.0; 3]))]));
    let d = build(&[TestEntity::physics(
        4,
        obj,
        [40.0, 40.0, 0.0],
        vec![(
            PID_BOXES,
            V::Array(vec![
                tv_box([-1.0; 3], [1.0; 3]),
                second,
                tv_box([20.0, 0.0, 0.0], [22.0, 1.0, 1.0]),
            ]),
        )],
    )]);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].boxes.len(), 1);
    assert!(
        !d[0].overlaps([61.0, 40.5, 0.5], 0.1, false),
        "third box ignored"
    );
    assert!(d[0].overlaps([40.5, 40.0, 0.0], 0.1, false));

    // The nearest level supplies the whole list: an own empty list hides the object's.
    let own_empty = build(&[TestEntity::physics(
        5,
        obj,
        [40.0, 40.0, 0.0],
        vec![(PID_BOXES, V::Array(vec![]))],
    )]);
    assert!(own_empty.is_empty());

    // Through generation (SC22 (a) and (b)): a node with ρ = 0.1 at the vertex is rejected
    // by an entity 0.5 units west of it.
    let [x, y] = base_xy();
    for list in [&a, &b] {
        let mut f = Fixture::new(
            data_one_vertex(),
            one_node(node(vec![SceneChild::Object(OBJ_X)])),
        );
        f.statics = vec![StaticClearance {
            position: [x - 0.5, y, 0.0],
            ..list[0].clone()
        }];
        let g = f.run();
        assert!(g.objects.is_empty());
        assert_eq!(g.stats.rejections(Rejection::StaticSpacing), 1);
    }
}

#[test]
fn sc22e_clearance_lists_use_the_levels_of_the_entity_type() {
    // SC22 (e) (BIN (ab), S45): beyond its own set a Physics or PatrolPoint entity searches
    // only a 0x47 object's chain, a Weenie only a game object's set and `PhysObj` chain.
    // Every entity here passes the validity check (`Entity_CreationType` 7 as an enum in
    // its own set: class Dynamic, so a Weenie is not dropped as Static).
    use testkit::TestEntity;
    let sch = schema();
    let pids = all_clearance_pids();
    let the_box = one_box_list([-1.0, -1.0, 0.0], [1.0, 1.0, 2.0]);
    let game = 0x7000_0601u32;
    let game_set = 0x7900_0601u32;
    let obj = 0x4700_0602u32;
    let mut src = MemoryWorldSource::new();
    src.insert(
        game_set,
        testkit::property_set_record(game_set, std::slice::from_ref(&the_box)),
        None,
    );
    src.insert(
        obj,
        testkit::entity_descriptor(obj, 0x1F00_0001, std::slice::from_ref(&the_box), &sch),
        None,
    );
    let cache = ObjectPropertyCache::default();
    let at = [40.0, 40.0, 0.0];
    let rejects = |e: TestEntity| {
        let list = static_clearances(&info_of(&[e]), &src, &sch, &pids, &cache).unwrap();
        list.iter()
            .any(|s| s.overlaps([40.5, 40.0, 0.0], 0.1, false))
    };
    // A Physics entity naming a game object whose set holds the box: placed.
    assert!(!rejects(TestEntity::physics(1, game, at, vec![ctype(7)])));
    // A PatrolPoint entity naming a game object: placed too.
    assert!(!rejects(TestEntity {
        kind: 23,
        ..TestEntity::physics(2, game, at, vec![ctype(7)])
    }));
    // A Weenie naming a 0x47 record that holds the box: placed.
    assert!(!rejects(TestEntity {
        kind: 6,
        ..TestEntity::physics(3, obj, at, vec![ctype(7)])
    }));
    // With the box in the entity's own set instead, both are rejected.
    assert!(rejects(TestEntity::physics(
        4,
        game,
        at,
        vec![ctype(7), the_box.clone()]
    )));
    assert!(rejects(TestEntity {
        kind: 6,
        ..TestEntity::physics(5, obj, at, vec![ctype(7), the_box.clone()])
    }));
    // The matching combinations reach the other level: a Physics entity naming the 0x47
    // record and a Weenie naming the game object.
    assert!(rejects(TestEntity::physics(6, obj, at, vec![ctype(7)])));
    assert!(rejects(TestEntity {
        kind: 6,
        ..TestEntity::physics(7, game, at, vec![ctype(7)])
    }));
}

#[test]
fn sc27_cylinder_heights_are_chosen_top_first() {
    // SC27 (BIN (x), S51): low point (0, 0, 0), radius 1, height −2 (top = −2), identity
    // frame, ρ = 0.5 with `spacing_3d`.
    let s = StaticClearance {
        position: [0.0; 3],
        rotation: Quaternion::IDENTITY,
        boxes: Vec::new(),
        cylinders: vec![ClearanceCylinder {
            low_point: [0.0; 3],
            radius: 1.0,
            height: -2.0,
        }],
    };
    // z′ = top = −2: 1.6² = 2.56 is not less than 2.25 (a clamp to −2 … 0 would overlap).
    assert!(!s.overlaps([0.0, 0.0, -0.4], 0.5, true));
    // z′ = cz = 0: 6.25 is not less than 2.25 (a clamp would give −2 and overlap).
    assert!(!s.overlaps([0.0, 0.0, -2.5], 0.5, true));
    // z′ = −2: 0.01 < 2.25.
    assert!(s.overlaps([0.0, 0.0, -1.9], 0.5, true));
    // The order of the choice, for both signs of the height.
    assert_eq!(axis_height(-0.4, 0.0, -2.0), -2.0);
    assert_eq!(axis_height(-2.5, 0.0, -2.0), 0.0);
    assert_eq!(axis_height(5.0, 0.0, -2.0), -2.0);
    assert_eq!(axis_height(1.0, 0.0, 2.0), 1.0);
    assert_eq!(axis_height(3.0, 0.0, 2.0), 2.0);
    assert_eq!(axis_height(-1.0, 0.0, 2.0), 0.0);
}

#[test]
fn sc28_static_list_validity_check() {
    // SC28 (BIN (w), (aa), S50): each entity alone in its landblock at (40, 40, 0), with one
    // box (−1, −1, 0) … (1, 1, 2) in its own set; "kept" means a candidate at (40.5, 40, 0)
    // with ρ = 0.1 is rejected.
    use testkit::TestEntity;
    let the_box = one_box_list([-1.0, -1.0, 0.0], [1.0, 1.0, 2.0]);
    let at = [40.0, 40.0, 0.0];
    let obj_none = 0x4700_0701u32; // a 0x47 object without 0x47E
    let obj_int = 0x4700_0702u32; // a 0x47 object holding 0x47E = 3 (an int with schema 15)
    let game = 0x7000_0703u32; // its set holds 0x47E = 3 (an int with schema 15)
    let game_set = 0x7900_0703u32;
    let game_empty = 0x7000_0704u32; // its set has no 0x47E, its `PhysObj` neither
    let game_empty_set = 0x7900_0704u32;
    let pids = all_clearance_pids();
    // One source per schema: the records hold 0x47E as four raw bytes, typed by the schema.
    let source = |sch: &HashMap<u32, tessera_formats::PropertyType>| {
        let mut src = MemoryWorldSource::new();
        src.insert(
            obj_none,
            testkit::entity_descriptor(obj_none, 0x1F00_0001, &[], sch),
            None,
        );
        src.insert(
            obj_int,
            testkit::entity_descriptor(obj_int, 0x1F00_0001, &[ctype(3)], sch),
            None,
        );
        src.insert(
            game_set,
            testkit::property_set_record(game_set, &[ctype(3)]),
            None,
        );
        src.insert(
            game_empty_set,
            testkit::property_set_record(
                game_empty_set,
                &[(testkit::pid::PHYS_OBJ, testkit::TestValue::Did(obj_none))],
            ),
            None,
        );
        src
    };
    let kept = |code: u32, e: TestEntity| {
        let sch = schema_with_ctype(code);
        let src = source(&sch);
        let cache = ObjectPropertyCache::default();
        let list = static_clearances(&info_of_with(&[e], &sch), &src, &sch, &pids, &cache)
            .expect("static list");
        list.iter()
            .any(|s| s.overlaps([40.5, 40.0, 0.0], 0.1, false))
    };
    let physics = |serial: u32, object: u32, extra: Vec<(u32, testkit::TestValue)>| {
        let mut props = vec![the_box.clone()];
        props.extend(extra);
        TestEntity::physics(serial, object, at, props)
    };
    let weenie = |serial: u32, object: u32, extra: Vec<(u32, testkit::TestValue)>| TestEntity {
        kind: 6,
        ..physics(serial, object, extra)
    };
    const ENUM: u32 = 8;
    const PROPERTY_ID: u32 = 10;
    const INT: u32 = 15;

    // Physics: no `object` field, and object DID 0, are dropped.
    assert!(!kept(
        ENUM,
        TestEntity {
            object: None,
            ..physics(1, 0, vec![ctype(3)])
        }
    ));
    assert!(!kept(ENUM, physics(2, 0, vec![ctype(3)])));
    // An object DID that names no record, with `Entity_CreationType` 3 as an enum: kept.
    assert!(kept(ENUM, physics(3, 0x4700_0999, vec![ctype(3)])));
    // No level has 0x47E: dropped.
    assert!(!kept(ENUM, physics(4, obj_none, vec![])));
    // The 0x47 object holds 0x47E as an int with value 3: dropped (type 15 gives 0).
    assert!(!kept(INT, physics(5, obj_int, vec![])));
    // The own set holds 0x47E as a property ID with value 3: kept.
    assert!(kept(PROPERTY_ID, physics(6, obj_none, vec![ctype(3)])));
    // The own enum 0 hides the object's value: dropped.
    assert!(!kept(ENUM, physics(7, obj_int, vec![ctype(0)])));
    // Weenie naming a game object whose set holds 0x47E as an int 3: kept (becomes 8).
    assert!(kept(INT, weenie(8, game, vec![])));
    // Weenie whose own set holds the enum 0: kept (0 becomes 8).
    assert!(kept(ENUM, weenie(9, game, vec![ctype(0)])));
    // Weenie where no level (own set, game object's set, `PhysObj` chain) has 0x47E.
    assert!(!kept(ENUM, weenie(10, game_empty, vec![])));
    // A Physics entity naming a game object, own enum 0: kept (the adjustment follows the
    // object DID, not the entity type).
    assert!(kept(ENUM, physics(11, game, vec![ctype(0)])));
    // A Physics entity naming a game object without an own 0x47E: dropped although the
    // game object's set has it (only the own set is searched).
    assert!(!kept(ENUM, physics(12, game, vec![])));
    // A Weenie naming a 0x47 object whose class is Static (3, SharedStaticImmobile):
    // dropped; one of class Dynamic (7) is kept.
    assert!(!kept(ENUM, weenie(13, obj_none, vec![ctype(3)])));
    assert!(kept(ENUM, weenie(14, obj_none, vec![ctype(7)])));
    // The fixed id is used whatever the dictionary maps the name to.
    assert_eq!(
        ClearancePids::from_names(|n| (n == "Entity_CreationType").then_some(0x1234)).creation_type,
        CREATION_TYPE_PID
    );
    assert_eq!(CREATION_TYPE_PID, 0x47E);
}

/// [`info_of`] decoded with schema `sch`.
fn info_of_with(
    entities: &[testkit::TestEntity],
    sch: &HashMap<u32, tessera_formats::PropertyType>,
) -> tessera_formats::LandblockInfo {
    let bytes = testkit::landblock_info(key(), entities);
    tessera_formats::LandblockInfo::parse(&bytes, Some(31), sch).unwrap()
}

#[test]
fn sc29_frill_values_follow_their_stored_types() {
    // SC29 (§8.3 rule 8; S52, S54): booleans only from stored booleans, floats only from
    // stored floats; a NaN intensity acts as 0.
    use crate::frills::{FrillObjectValues, FrillPids, FrillSettings};
    let (p_opaque, p_anim, p_tint) = (0x1000_7101u32, 0x1000_7102u32, 0x1000_7103u32);
    let pids = FrillPids {
        opaque: Some(p_opaque),
        animation_intensity: Some(p_anim),
        terrain_color_intensity: Some(p_tint),
        ..FrillPids::default()
    };
    let props = |entries: Vec<(u32, PropertyValue)>| ObjectProperties {
        levels: vec![ChainLevel {
            did: Did(0x4700_0801),
            sets: vec![tessera_formats::PropertySet {
                size: tessera_formats::TSize::default(),
                entries: entries
                    .into_iter()
                    .map(|(pid, value)| PropertyEntry { pid, value })
                    .collect(),
            }],
        }],
        incomplete: false,
    };
    let on = FrillSettings {
        terrain_color: true,
        ..FrillSettings::default()
    };
    // `Render_Frill_IsOpaque` stored as an integer 0 counts as absent: opaque (default).
    let v = FrillObjectValues::resolve(
        Some(&props(vec![(p_opaque, PropertyValue::Int(0))])),
        &pids,
        &on,
    );
    assert!(v.opaque);
    // A NaN animation intensity acts as 0: not animated.
    let v = FrillObjectValues::resolve(
        Some(&props(vec![(p_anim, PropertyValue::Float(f32::NAN))])),
        &pids,
        &on,
    );
    assert_eq!(v.animation_intensity, 0.0);
    assert!(!v.is_animated());
    // A NaN terrain-colour intensity with terrain colour on acts as 0.
    let v = FrillObjectValues::resolve(
        Some(&props(vec![(p_tint, PropertyValue::Float(f32::NAN))])),
        &pids,
        &on,
    );
    assert_eq!(v.terrain_color_intensity, 0.0);
    // An animation intensity stored as an integer 1 counts as absent (default 0).
    let v = FrillObjectValues::resolve(
        Some(&props(vec![(p_anim, PropertyValue::Int(1))])),
        &pids,
        &on,
    );
    assert_eq!(v.animation_intensity, 0.0);
    // ±∞ clamp to 1 and 0.
    let v = FrillObjectValues::resolve(
        Some(&props(vec![
            (p_anim, PropertyValue::Float(f32::INFINITY)),
            (p_tint, PropertyValue::Float(f32::NEG_INFINITY)),
        ])),
        &pids,
        &on,
    );
    assert_eq!(
        (v.animation_intensity, v.terrain_color_intensity),
        (1.0, 0.0)
    );
}

#[test]
fn water_trait_forms() {
    let k = key();
    assert_eq!(NoWater.water_plane(k), None);
    assert_eq!(
        FixedWater(Some(level_plane(4.0))).water_plane(k),
        Some(level_plane(4.0))
    );
    let f = |key: LandblockKey| (key.x() == 48).then(|| level_plane(1.0));
    assert_eq!(f.water_plane(k), Some(level_plane(1.0)));
    let p: Plane = level_plane(2.5);
    assert_eq!(p.height_at(10.0, 20.0), Some(2.5));
}

// ---------------------------------------------------------------------------------------
// Hostile-input regressions
// ---------------------------------------------------------------------------------------

/// Statics that never overlap anything near the block: `n` entities of `boxes` boxes each,
/// far outside the block.
fn far_statics(n: usize, boxes: usize) -> Vec<StaticClearance> {
    (0..n)
        .map(|_| StaticClearance {
            position: [10_000.0, 10_000.0, 0.0],
            rotation: Quaternion::IDENTITY,
            boxes: vec![
                ClearanceBox {
                    min: [-1.0; 3],
                    max: [1.0; 3],
                };
                boxes
            ],
            cylinders: Vec::new(),
        })
        .collect()
}

#[test]
fn static_scan_is_charged_to_the_work_budget() {
    // Four sibling nodes per vertex, none rejected before step 14, on every vertex.
    let n = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let mut data = data_one_vertex();
    for w in data.coarse.iter_mut() {
        *w = word(5, 1);
    }
    let list = vec![scene(
        VERTEX_SCENE,
        vec![SceneChild::Node(Box::new(n)); 4],
        None,
    )];
    let mut f = Fixture::new(data, list);
    let free = f.run();
    assert!(!free.stats.truncated());
    assert!(free.objects.len() > 40);
    // 1,000 entities of 15 boxes: 16,000 items, 1,000 work units per scan.
    f.statics = far_statics(1_000, 15);
    f.limits.max_work_per_landblock = 1 << 14;
    let start = std::time::Instant::now();
    let g = f.run();
    assert!(g.stats.budget_cut > 0, "{:?}", g.stats);
    assert!(g.stats.truncated());
    assert!(g.objects.len() < free.objects.len());
    assert!(start.elapsed().as_secs() < 30);
    // The statics are far away: a scan the budget could not pay for is not booked as a
    // static-spacing (or any other) rejection.
    assert_eq!(
        g.stats.rejections(Rejection::StaticSpacing),
        0,
        "{:?}",
        g.stats
    );
    assert_eq!(g.stats.rejected.iter().sum::<u32>(), 0, "{:?}", g.stats);
    // A small static list costs one unit per scan and changes nothing here.
    f.statics = far_statics(3, 2);
    f.limits = SceneryLimits::default();
    let g = f.run();
    assert_eq!(g.objects, free.objects);
    assert!(!g.stats.truncated());
}

#[test]
fn negative_spacing_never_places_outside_the_block() {
    let n = PlacementNode {
        spacing: -1e6,
        disp_min: [5000.0, -3000.0, 0.0],
        disp_max: [5000.0, -3000.0, 0.0],
        no_checks: true,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let mut data = data_one_vertex();
    for w in data.coarse.iter_mut() {
        *w = word(5, 1);
    }
    let g = Fixture::new(data, one_node(n)).run();
    assert!(g.objects.is_empty(), "{:?}", g.objects.first());
    assert!(g.stats.rejections(Rejection::Outside) > 0);
    // A negative spacing with a position inside the block still places (ρ < 0 only
    // widens the client's bound; the 0 ≤ p < 160 guard leaves it inside).
    let n = PlacementNode {
        spacing: -5.0,
        no_checks: true,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_eq!(g.objects.len(), 1);
    for o in &g.objects {
        assert!((0.0..160.0).contains(&o.position[0]) && (0.0..160.0).contains(&o.position[1]));
    }
}

#[test]
fn sum_stats_saturates() {
    let mut a = SceneryStats {
        nodes_tested: u32::MAX - 1,
        scenes_unavailable: u32::MAX,
        ..SceneryStats::default()
    };
    a.rejected[Rejection::Footprint.index()] = u32::MAX;
    let b = SceneryStats {
        nodes_tested: 5,
        scenes_unavailable: 1,
        rejected: [7; Rejection::COUNT],
        ..SceneryStats::default()
    };
    let s = sum_stats([&a, &b, &b]);
    assert_eq!(s.nodes_tested, u32::MAX);
    assert_eq!(s.scenes_unavailable, u32::MAX);
    assert_eq!(s.rejections(Rejection::Footprint), u32::MAX);
    assert_eq!(s.rejections(Rejection::Slope), 14);
}

#[test]
fn rejection_indices_cover_every_variant() {
    // Exhaustive inside the crate: a new variant fails to compile here until it is given
    // an index and added to `ALL`.
    let index = |r: Rejection| match r {
        Rejection::Frequency => 0,
        Rejection::ScaleMode => 1,
        Rejection::Outside => 2,
        Rejection::TerrainType => 3,
        Rejection::Slope => 4,
        Rejection::NoWater => 5,
        Rejection::HeightRange => 6,
        Rejection::Clearance => 7,
        Rejection::StaticSpacing => 8,
        Rejection::Footprint => 9,
    };
    assert_eq!(Rejection::ALL.len(), Rejection::COUNT);
    for (k, r) in Rejection::ALL.iter().enumerate() {
        assert_eq!(r.index(), k);
        assert_eq!(index(*r), k);
        assert!(r.index() < SceneryStats::default().rejected.len());
    }
}

/// A record source that fails with a pipeline error (not the data's) the first `fails`
/// times a DID is read.
struct Flaky {
    inner: MemoryWorldSource,
    fails: std::sync::Mutex<u32>,
}

impl crate::source::WorldSource for Flaky {
    fn read_stamped(
        &self,
        did: Did,
    ) -> Result<Option<crate::source::StampedRecord>, tessera_assets::AssetError> {
        {
            let mut f = self.fails.lock().unwrap();
            if *f > 0 {
                *f -= 1;
                return Err(tessera_assets::AssetError::Shutdown);
            }
        }
        self.inner.read_stamped(did)
    }
}

#[test]
fn transient_read_failures_are_not_cached() {
    let s = scene(VERTEX_SCENE, vec![SceneChild::Object(OBJ_X)], None);
    let mut inner = MemoryWorldSource::new();
    inner.insert(VERTEX_SCENE.0, s.to_bytes(), None);
    let flaky = Arc::new(Flaky {
        inner,
        fails: std::sync::Mutex::new(1),
    });
    let lib = SceneLibrary::new(flaky.clone());
    let e = lib.scene(VERTEX_SCENE).expect_err("pipeline failure");
    assert!(!e.is_data_error());
    assert!(lib.is_empty(), "a pipeline failure is not cached");
    assert!(lib.scene(VERTEX_SCENE).expect("read").is_some());
    // A missing scene is a data failure: `Ok(None)`, cached.
    assert!(lib.scene(Did(0x0200_7777)).expect("read").is_none());
    assert_eq!(lib.len(), 2);

    // Generation reports the failure as an incomplete result.
    *flaky.fails.lock().unwrap() = 1;
    let lib = SceneLibrary::new(flaky.clone());
    let data = data_one_vertex();
    let terrain = GridMesh::terrain(&data);
    let t = tables();
    let input = SceneryInput {
        key: key(),
        data: &data,
        terrain: &terrain,
        tables: &t,
        scenes: &lib,
        water: &NoWater,
        statics: &[],
        limits: SceneryLimits::default(),
    };
    // Vertex (0, 0) reads EMPTY first; it is missing from the source (a data failure)
    // only after the one pipeline failure has been spent on it.
    let first = generate_landblock(&input);
    assert_eq!(first.stats.scenes_unavailable, 1);
    assert!(!first.stats.is_complete());
    let again = generate_landblock(&input);
    assert!(again.stats.is_complete());
    assert_eq!(again.objects.len(), 1);

    // Object properties: the same rule.
    let sch = schema();
    let mut inner = MemoryWorldSource::new();
    inner.insert(
        0x4700_0101u32,
        testkit::entity_descriptor(
            0x4700_0101,
            0x1F00_0001,
            &[(PID_F, testkit::TestValue::Float(0.5))],
            &sch,
        ),
        None,
    );
    let flaky = Flaky {
        inner,
        fails: std::sync::Mutex::new(1),
    };
    let cache = ObjectPropertyCache::default();
    assert!(cache.get(&flaky, &sch, Did(0x4700_0101)).is_err());
    assert!(cache.is_empty());
    let p = cache
        .get(&flaky, &sch, Did(0x4700_0101))
        .expect("read")
        .expect("object");
    assert_eq!(p.float(PID_F), Some(0.5));
    assert!(
        cache
            .get(&flaky, &sch, Did(0x4700_0999))
            .expect("read")
            .is_none()
    );
    assert_eq!(cache.len(), 2);
}

#[test]
fn object_cache_follows_its_source() {
    let sch = schema();
    let make = |v: f32| {
        let mut m = MemoryWorldSource::new();
        m.insert(
            0x4700_0101u32,
            testkit::entity_descriptor(
                0x4700_0101,
                0x1F00_0001,
                &[(PID_F, testkit::TestValue::Float(v))],
                &sch,
            ),
            None,
        );
        m
    };
    let (one, two) = (make(1.0), make(2.0));
    let cache = ObjectPropertyCache::default();
    let get = |src: &MemoryWorldSource| {
        cache
            .get(src, &sch, Did(0x4700_0101))
            .unwrap()
            .unwrap()
            .float(PID_F)
    };
    assert_eq!(get(&one), Some(1.0));
    assert_eq!(get(&two), Some(2.0));
    assert_eq!(get(&one), Some(1.0));
}

/// A chain of `n` scenes, each holding a `nest`-deep node nest whose innermost node
/// references the next scene.
fn deep_chain(n: u32, nest: usize) -> Vec<Scene> {
    (0..n)
        .map(|k| {
            let did = Did(VERTEX_SCENE.0 + k);
            let mut inner = PlacementNode {
                no_spacing: true,
                ..node(vec![
                    SceneChild::Scene(Did(did.0 + 1)),
                    SceneChild::Object(OBJ_X),
                ])
            };
            for _ in 1..nest {
                inner = PlacementNode {
                    no_spacing: true,
                    ..node(vec![SceneChild::Node(Box::new(inner))])
                };
            }
            scene(did, vec![SceneChild::Node(Box::new(inner))], None)
        })
        .collect()
}

#[test]
fn deep_recursion_fits_a_small_stack() {
    // The std default for spawned threads (the asset workers set no stack size), in the
    // debug build the tests run in.
    let run = |list: Vec<Scene>, depth: usize| {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let mut f = Fixture::new(data_one_vertex(), list);
                f.limits.max_scene_depth = depth;
                f.run()
            })
            .unwrap()
            .join()
            .expect("no stack overflow")
    };
    // Raising the scene-depth limit past the hard limit is clamped.
    let g = run(deep_chain(100, 16), 105);
    assert!(g.stats.depth_capped > 0);
    assert_eq!(g.objects.len(), MAX_SCENE_DEPTH_LIMIT);
    // A node nest deeper than any decoder produces is cut by the recursion bound.
    let g = run(deep_chain(1, 2 * MAX_RECURSION_DEPTH), 16);
    assert!(g.stats.depth_capped > 0);
    assert!(g.objects.is_empty());
    // The default limits are unchanged: 16 scenes.
    let g = run(deep_chain(40, 16), DEFAULT_MAX_SCENE_DEPTH);
    assert_eq!(g.objects.len(), DEFAULT_MAX_SCENE_DEPTH);
}

#[test]
fn non_finite_static_frames_never_overlap() {
    let base = StaticClearance {
        position: [10.0, 10.0, 0.0],
        rotation: Quaternion::IDENTITY,
        boxes: vec![ClearanceBox {
            min: [-1.0; 3],
            max: [1.0; 3],
        }],
        cylinders: vec![ClearanceCylinder {
            low_point: [0.0; 3],
            radius: 1.0,
            height: 1.0,
        }],
    };
    assert!(base.overlaps([10.5, 10.0, 0.0], 0.0, false));
    assert!(!base.overlaps([150.0, 150.0, 0.0], 0.0, false));
    for rotation in [
        Quaternion {
            w: f32::NAN,
            ..Quaternion::IDENTITY
        },
        Quaternion {
            w: 1e30,
            x: 1e30,
            y: 0.0,
            z: 0.0,
        },
    ] {
        let s = StaticClearance {
            rotation,
            ..base.clone()
        };
        assert!(!s.overlaps([150.0, 150.0, 0.0], 0.0, false), "{rotation:?}");
    }
    let nan_pos = StaticClearance {
        position: [f32::NAN, 0.0, 0.0],
        ..base.clone()
    };
    assert!(!nan_pos.overlaps([150.0, 150.0, 0.0], 0.0, false));
    // The static list drops non-finite rotations and normalises far-from-unit ones.
    use super::clearance::usable_rotation;
    assert_eq!(
        usable_rotation(Quaternion {
            w: f32::NAN,
            ..Quaternion::IDENTITY
        }),
        None
    );
    assert_eq!(
        usable_rotation(Quaternion {
            w: 0.0,
            ..Quaternion::IDENTITY
        }),
        None
    );
    let doubled = usable_rotation(Quaternion {
        w: 2.0,
        ..Quaternion::IDENTITY
    })
    .unwrap();
    assert_eq!(doubled, Quaternion::IDENTITY);
    let s = StaticClearance {
        rotation: doubled,
        ..base.clone()
    };
    assert!(s.overlaps([10.6, 10.0, 0.0], 0.0, false));
    let near = Quaternion {
        w: 1.0 + 1e-5,
        ..Quaternion::IDENTITY
    };
    assert_eq!(usable_rotation(near), Some(near), "stored as is");
}

#[test]
fn static_list_is_capped() {
    use testkit::{TestEntity, TestValue as V};
    let sch = schema();
    let pids = ClearancePids {
        box_list: Some(PID_BOXES),
        box_min: Some(PID_MIN),
        box_max: Some(PID_MAX),
        ..ClearancePids::default()
    };
    let one_box = (
        PID_BOX,
        V::Struct(vec![
            (PID_MIN, V::Vec3([-1.0; 3])),
            (PID_MAX, V::Vec3([1.0; 3])),
        ]),
    );
    // Every entity passes the validity check (creation type 7 in its own set).
    let list = vec![(PID_BOXES, V::Array(vec![one_box.clone(); 300])), ctype(7)];
    // 70 entities of 256 boxes (the per-entity cap) each, and one whose boxes come from
    // its object.
    let mut entities: Vec<TestEntity> = (0..70)
        .map(|k| TestEntity::physics(k, 0x4700_0300, [20.0, 20.0, 0.0], list.clone()))
        .collect();
    entities.insert(
        0,
        TestEntity::physics(99, 0x4700_0301, [30.0, 30.0, 0.0], vec![ctype(7)]),
    );
    let mut src = MemoryWorldSource::new();
    src.insert(
        0x4700_0301u32,
        testkit::entity_descriptor(
            0x4700_0301,
            0x1F00_0001,
            &[(PID_BOXES, V::Array(vec![one_box; 3]))],
            &sch,
        ),
        None,
    );
    let bytes = testkit::landblock_info(key(), &entities);
    let info = tessera_formats::LandblockInfo::parse(&bytes, Some(31), &sch).unwrap();
    let cache = ObjectPropertyCache::default();
    let list = static_clearances(&info, &src, &sch, &pids, &cache).unwrap();
    let total: usize = list.iter().map(StaticClearance::volume_count).sum();
    assert_eq!(total, MAX_STATIC_VOLUMES);
    assert_eq!(list[0].boxes.len(), 3, "from the object");
    assert!(
        list[1..]
            .iter()
            .all(|s| s.boxes.len() <= MAX_VOLUMES_PER_ENTITY)
    );
    assert_eq!(
        list.len(),
        1 + (MAX_STATIC_VOLUMES - 3).div_ceil(MAX_VOLUMES_PER_ENTITY)
    );
}

#[test]
fn pairwise_clearance_pairs_triangles_with_equal_normals() {
    // Flat ground: every collected triangle has the normal (0, 0, 1), self-dot 1.
    let n = |dot| PlacementNode {
        clearance: 3.0,
        clear_min_dot: dot,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n(1.0))).run();
    assert_eq!(g.objects.len(), 1);
    let g = Fixture::new(data_one_vertex(), one_node(n(1.5))).run();
    assert!(g.objects.is_empty());
    assert_eq!(g.stats.rejections(Rejection::Clearance), 1);
}

// ---------------------------------------------------------------------------------------
// Spec branches without earlier coverage
// ---------------------------------------------------------------------------------------

/// Data with height rising along +X only (raw 400 per sample) and the scenery word at
/// vertex (3, 7).
fn ramp_data() -> LandblockData {
    #[allow(clippy::cast_possible_truncation)]
    let heights = HeightGrid::from_fn(|i, _| (i * 400) as u16);
    let mut d = LandblockData::from_heights(key().data_did(), heights);
    d.coarse[17 * 3 + 7] = word(5, 1);
    d
}

fn fixed(v: f32) -> FloatRange {
    FloatRange { min: v, max: v }
}

/// The vertex (3, 7) base position.
fn base_xy() -> [f32; 2] {
    [3.0 * (10.0 - 0.0002f32), 7.0 * (10.0 - 0.0002f32)]
}

fn assert_q_sign(q: Quaternion, e: [f32; 4], tol: f32) {
    let got = q.to_wxyz();
    let same = got.iter().zip(e).all(|(a, b)| approx(*a, b, tol));
    let neg = got.iter().zip(e).all(|(a, b)| approx(*a, -b, tol));
    assert!(same || neg, "{got:?} vs ±{e:?}");
}

#[test]
fn face_direction_turns_the_heading_by_the_slope_bearing() {
    let n = |align| PlacementNode {
        face_direction: true,
        align_to_slope: align,
        heading: fixed(30.0),
        angle2: fixed(20.0),
        angle3: fixed(40.0),
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    // Flat ground: no bearing term; h = 30 + 180; a2, a3 not applied.
    let g = Fixture::new(data_one_vertex(), one_node(n(false))).run();
    assert_q_sign(g.objects[0].rotation, [-0.25882, 0.0, 0.0, -0.96593], 2e-5);
    // Ground rising to +X: the normal leans to −X, atan2(−n.x, n.y) = 90°, h = 300.
    let f = Fixture::new(ramp_data(), one_node(n(false)));
    let g = f.run();
    let o = g.objects[0];
    let nrm = f
        .terrain
        .client_plane_at(o.position[0], o.position[1], &f.data.vertex_flags)
        .normal;
    assert!(nrm[0] < -0.01 && nrm[1].abs() < 1e-6, "{nrm:?}");
    assert_q_sign(o.rotation, [-0.86603, 0.0, 0.0, -0.5], 2e-5);
    // With `align_to_slope`: Q_align(n) · Rz(−300); model Z goes to the normal.
    let g = Fixture::new(ramp_data(), one_node(n(true))).run();
    let q = g.objects[0].rotation;
    assert_q_sign(
        q,
        math::mul(math::align_to_normal(nrm), math::rot_z(-300.0)).to_wxyz(),
        2e-5,
    );
    let z = q.rotate([0.0, 0.0, 1.0]);
    for k in 0..3 {
        assert!(approx(z[k], nrm[k], 1e-4), "{z:?}");
    }
    // Absolute: a parent orientation is not composed even with `orient_relative`.
    let parent = PlacementNode {
        heading: fixed(77.0),
        no_spacing: true,
        ..node(vec![SceneChild::Node(Box::new(PlacementNode {
            no_spacing: true,
            ..n(false)
        }))])
    };
    let g = Fixture::new(data_one_vertex(), one_node(parent)).run();
    assert_q_sign(g.objects[0].rotation, [-0.25882, 0.0, 0.0, -0.96593], 2e-5);
}

#[test]
fn tilt_angles_compose_after_the_heading() {
    // Q_local = Rz(−30) · Ry(40) · Rx(20), hand-computed.
    let n = PlacementNode {
        heading: fixed(30.0),
        angle2: fixed(20.0),
        angle3: fixed(40.0),
        orient_relative: false,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let g = Fixture::new(data_one_vertex(), one_node(n)).run();
    assert_q_sign(
        g.objects[0].rotation,
        [0.87851, 0.24479, 0.28311, -0.29688],
        2e-5,
    );
    // With `orient_relative`, the parent's orientation is applied first.
    let child = PlacementNode {
        heading: fixed(30.0),
        angle2: fixed(20.0),
        angle3: fixed(40.0),
        no_spacing: true,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let parent = PlacementNode {
        heading: fixed(90.0),
        no_spacing: true,
        ..node(vec![SceneChild::Node(Box::new(child))])
    };
    let g = Fixture::new(data_one_vertex(), one_node(parent)).run();
    let want = math::mul(
        math::rot_z(-90.0),
        Quaternion {
            w: 0.87851,
            x: 0.24479,
            y: 0.28311,
            z: -0.29688,
        },
    );
    assert_q_sign(g.objects[0].rotation, want.to_wxyz(), 5e-5);
}

#[test]
fn height_modes_two_to_eight_nine_and_others() {
    // Ground at raw 1000 everywhere (z ≈ 30.5); no checks so height ranges do not apply.
    let mut data = data_one_vertex();
    data.heights = HeightGrid::flat(1000);
    let ground = data.heights.height(6, 14).unwrap();
    assert!(ground > 30.0);
    let mk = |mode: u8, dz: (f32, f32)| PlacementNode {
        height_mode: mode,
        no_checks: true,
        disp_min: [0.0, 0.0, dz.0],
        disp_max: [0.0, 0.0, dz.1],
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let z = |mode, dz| {
        Fixture::new(data.clone(), one_node(mk(mode, dz)))
            .run()
            .objects[0]
            .position[2]
    };
    let (a, b) = (hash::seed_a(2, 48, 80), hash::seed_b(3, 7));
    let c = hash::child_seed(hash::scene_seed(hash::SCENE_SEED), 0);
    let r = |salt: u32| hash::node_random(a, b, c.wrapping_add(salt));
    for mode in 2..=8 {
        assert_eq!(z(mode, (0.0, 0.0)), 0.0, "mode {mode}");
        // The step-11 offset (salt 0x28D0642D) is still added.
        let want = 1.0 + r(hash::SALT_DISP_X) * 2.0;
        assert!(approx(z(mode, (1.0, 3.0)), want, 1e-5), "mode {mode}");
    }
    // Mode 9: the z displacement of step 4 (salt SALT_DISP_Z) from the parent height, and
    // no step-11 offset.
    let want = ground + 1.0 + r(hash::SALT_DISP_Z) * 2.0;
    assert!(approx(z(9, (1.0, 3.0)), want, 1e-4));
    assert!((r(hash::SALT_DISP_Z) - r(hash::SALT_DISP_X)).abs() > 1e-3);
    // Any other value keeps the parent height, then adds the offset.
    let mut ramp = ramp_data();
    ramp.coarse[17 * 3 + 7] = word(5, 1);
    let parent_z = ramp.heights.height(6, 14).unwrap();
    let n = PlacementNode {
        disp_min: [3.0, 0.0, 0.5],
        disp_max: [3.0, 0.0, 0.5],
        ..mk(11, (0.5, 0.5))
    };
    let g = Fixture::new(ramp, one_node(n)).run();
    assert!(approx(g.objects[0].position[2], parent_z + 0.5, 1e-5));
    assert!(approx(g.objects[0].position[0], base_xy()[0] + 3.0, 1e-4));
}

#[test]
fn spacing_3d_measures_height_in_markers_and_boxes() {
    // Two nodes at the same x, y; the second 10 units higher (height mode 9).
    let pair = |three_d: bool| {
        let n = |dz: f32| PlacementNode {
            spacing: 1.0,
            spacing_3d: three_d,
            height_mode: 9,
            disp_min: [0.0, 0.0, dz],
            disp_max: [0.0, 0.0, dz],
            above_ground_mode: 3,
            ..node(vec![SceneChild::Object(OBJ_X)])
        };
        vec![scene(
            VERTEX_SCENE,
            vec![
                SceneChild::Node(Box::new(n(0.0))),
                SceneChild::Node(Box::new(n(10.0))),
            ],
            None,
        )]
    };
    let g = Fixture::new(data_one_vertex(), pair(false)).run();
    assert_eq!(g.objects.len(), 1);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
    let g = Fixture::new(data_one_vertex(), pair(true)).run();
    assert_eq!(g.objects.len(), 2);

    // A box 5–6 units above the candidate: overlaps in 2D only.
    let n = |three_d| PlacementNode {
        spacing: 1.0,
        spacing_3d: three_d,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let [x, y] = base_xy();
    let statics = vec![StaticClearance {
        position: [x, y, 0.0],
        rotation: Quaternion::IDENTITY,
        boxes: vec![ClearanceBox {
            min: [-1.0, -1.0, 5.0],
            max: [1.0, 1.0, 6.0],
        }],
        cylinders: Vec::new(),
    }];
    let mut f = Fixture::new(data_one_vertex(), one_node(n(false)));
    f.statics.clone_from(&statics);
    assert!(f.run().objects.is_empty());
    let mut f = Fixture::new(data_one_vertex(), one_node(n(true)));
    f.statics = statics;
    assert_eq!(f.run().objects.len(), 1);
}

#[test]
fn frill_pass_markers() {
    // §8.2 item 5: a group node (state clear) leaves a marker that spaces the frill layer.
    let layer = |no_spacing| PlacementNode {
        spacing: 1.0,
        no_spacing,
        ..node(vec![SceneChild::Object(OBJ_Y)])
    };
    let group = |spacing| PlacementNode {
        spacing,
        ..node(vec![SceneChild::Object(OBJ_X)])
    };
    let fx = |spacing, no_spacing| {
        Fixture::new(
            data_one_vertex(),
            vec![scene(
                VERTEX_SCENE,
                vec![SceneChild::Node(Box::new(group(spacing)))],
                Some(layer(no_spacing)),
            )],
        )
    };
    // The group node is rejected by its own static-pass twin, the layer by the same
    // marker.
    let (records, stats) = fx(1.0, false).frills(3, 7);
    assert!(records.is_empty());
    assert_eq!(stats.rejections(Rejection::Footprint), 2);
    // A radius-0 marker never overlaps; `no_spacing` skips the marker test.
    assert_eq!(fx(0.0, false).frills(3, 7).0.len(), 1);
    let (records, stats) = fx(1.0, true).frills(3, 7);
    assert_eq!(records.len(), 1);
    assert_eq!(stats.rejections(Rejection::Footprint), 1);

    // Nodes placed while the state is set leave no marker: two spaced siblings under the
    // layer at the same position are both placed.
    let sib = PlacementNode {
        spacing: 1.0,
        ..node(vec![SceneChild::Object(OBJ_Y)])
    };
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![
            SceneChild::Node(Box::new(sib.clone())),
            SceneChild::Node(Box::new(sib)),
        ])
    };
    let f = Fixture::new(
        data_one_vertex(),
        vec![scene(VERTEX_SCENE, vec![], Some(layer))],
    );
    let (records, stats) = f.frills(3, 7);
    assert_eq!(records.len(), 2);
    assert_eq!(stats.rejections(Rejection::Footprint), 0);

    // The frill pass is spaced against the static pass's markers of every vertex:
    // vertices (15, 7) and (16, 7) place group nodes 9.9998 apart with radius 5. The static
    // pass places the first and spaces the second against it; in the frill pass of sector
    // (15, 7) the first meets its own twin's marker and the second the first's.
    let g5 = PlacementNode {
        spacing: 5.0,
        disp_min: [-5.0, 0.0, 0.0],
        disp_max: [-5.0, 0.0, 0.0],
        ..node(vec![])
    };
    let mut data = LandblockData::from_heights(key().data_did(), HeightGrid::flat(0));
    data.coarse[17 * 15 + 7] = word(5, 1);
    data.coarse[17 * 16 + 7] = word(5, 1);
    // Both vertices must pick VERTEX_SCENE: make it the only choice of key 10.
    let mut f = Fixture::new(data, one_node(g5));
    let mut t = tables();
    let entries = vec![
        SceneChoiceEntry {
            key: 1,
            choices: vec![SceneChoice {
                scene: EMPTY,
                weight: 1.0,
                flag: 1,
            }],
        },
        SceneChoiceEntry {
            key: 10,
            choices: vec![SceneChoice {
                scene: VERTEX_SCENE,
                weight: 1.0,
                flag: 1,
            }],
        },
    ];
    t.scene_choices = Arc::new(SceneChoiceTable::new(0, Did(0x1100_0001), entries).unwrap());
    f.tables = t;
    let g = f.run();
    assert_eq!(g.stats.nodes_placed, 1);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
    let s = f.sector(15, 7, 1.0, &mut DensityThinner::with_seed(3));
    assert_eq!(s.stats.nodes_placed, 0);
    assert_eq!(s.stats.rejections(Rejection::Footprint), 2);
    // Without the static pass's markers (an empty list) nothing spaces them.
    let s = generate_sector(
        &f.input(),
        &[],
        15,
        7,
        1.0,
        &mut DensityThinner::with_seed(3),
    );
    assert_eq!(s.stats.nodes_placed, 2);
}

#[test]
fn frill_layer_of_a_scene_reached_through_a_node() {
    // §8.2 item 3: the referenced scene's frill layer runs in the frill pass.
    let r = Did(0x0200_5000);
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Object(OBJ_Y)])
    };
    let f = Fixture::new(
        data_one_vertex(),
        vec![
            scene(
                VERTEX_SCENE,
                vec![SceneChild::Node(Box::new(PlacementNode {
                    no_spacing: true,
                    ..node(vec![SceneChild::Scene(r)])
                }))],
                None,
            ),
            scene(r, vec![], Some(layer)),
        ],
    );
    assert!(f.run().objects.is_empty(), "frill layers are not entities");
    let (records, _) = f.frills(3, 7);
    assert_eq!(records.len(), 1);
    assert_eq!((records[0].object.object, records[0].late), (OBJ_Y, false));
}

#[test]
fn no_spacing_passes_through_scene_references() {
    // §7.4 step 2 (the client's rule, BIN (o), S37): a scene reference that is a child of
    // node N is instantiated with N's combined switches, so the referenced scene's node
    // inherits `no_spacing*`. The wrapper node has `no_checks`, so it is never tested
    // itself; only the inner node meets the first node's closed marker (radius 1.0, same
    // position).
    let r = Did(0x0200_6000);
    let spaced = |obj| PlacementNode {
        spacing: 1.0,
        ..node(vec![SceneChild::Object(obj)])
    };
    let list = |pass: bool| {
        vec![
            scene(
                VERTEX_SCENE,
                vec![
                    SceneChild::Node(Box::new(spaced(OBJ_X))),
                    SceneChild::Node(Box::new(PlacementNode {
                        no_spacing: pass,
                        no_checks: true,
                        ..node(vec![SceneChild::Scene(r)])
                    })),
                ],
                None,
            ),
            scene(r, vec![SceneChild::Node(Box::new(spaced(OBJ_Y)))], None),
        ]
    };
    let g = Fixture::new(data_one_vertex(), list(true)).run();
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X, OBJ_Y]
    );
    assert_eq!(g.stats.nodes_placed, 3);
    assert_eq!(g.stats.rejections(Rejection::Footprint), 0);
    // Without the switch the wrapper is still placed (it skips its own tests) and its
    // scene reference runs, but the inner node overlaps the first node's marker.
    let g = Fixture::new(data_one_vertex(), list(false)).run();
    assert_eq!(
        g.objects.iter().map(|o| o.object).collect::<Vec<_>>(),
        [OBJ_X]
    );
    assert_eq!(
        g.stats.nodes_tested, 3,
        "the inner node was reached and tested"
    );
    assert_eq!(g.stats.nodes_placed, 2, "first node and wrapper");
    assert_eq!(g.stats.rejections(Rejection::Footprint), 1);
}

#[test]
fn frill_pass_appends_no_markers() {
    // §8.2 item 5 shortcut: the frill pass appends no markers of its own, so they never
    // count against `max_markers`. With a cap of one marker the static pass is cut at the
    // second group node; the frill pass (no markers) still reaches the frill layer.
    let g = |obj| PlacementNode {
        no_spacing: true,
        spacing: 0.0,
        ..node(vec![SceneChild::Object(obj)])
    };
    let layer = PlacementNode {
        no_spacing: true,
        ..node(vec![SceneChild::Object(OBJ_Y)])
    };
    let mut f = Fixture::new(
        data_one_vertex(),
        vec![scene(
            VERTEX_SCENE,
            vec![
                SceneChild::Node(Box::new(g(OBJ_X))),
                SceneChild::Node(Box::new(g(OBJ_ALIGN))),
            ],
            Some(layer),
        )],
    );
    f.limits.max_markers = 1;
    let st = f.run();
    assert_eq!(st.markers.len(), 1);
    assert!(st.stats.budget_cut > 0);
    let (records, stats) = f.frills(3, 7);
    assert_eq!(stats.budget_cut, 0, "{stats:?}");
    assert_eq!(
        records.iter().map(|r| r.object.object).collect::<Vec<_>>(),
        [OBJ_Y]
    );
}

#[test]
fn pids_from_a_context_dictionary() {
    let mut src = MemoryWorldSource::new();
    src.insert(
        crate::context::PROPERTY_DICTIONARY_DID,
        testkit::dictionary(&[
            (0x207, "Entity_Clearance_BoxList", 12),
            (0x4CF, "Entity_Clearance_CylinderList", 12),
            (0x300, "Render_Frill_IsOpaque", 21),
        ]),
        None,
    );
    let ctx =
        crate::WorldContext::load(Arc::new(src), crate::BundleOptions::default()).expect("context");
    let c = ClearancePids::from_context(&ctx).unwrap();
    assert_eq!(
        (c.box_list, c.cylinder_list, c.radius),
        (Some(0x207), Some(0x4CF), None)
    );
    let f = crate::frills::FrillPids::from_context(&ctx).unwrap();
    assert_eq!((f.opaque, f.allow_reduction), (Some(0x300), None));
}

/// A record source that, while `fail` names a DID, reads it as missing (`Ok(None)`, a data
/// fault) or fails it with a pipeline error (not the data's).
struct FailOne {
    inner: MemoryWorldSource,
    fail: std::sync::Mutex<Option<(u32, bool)>>,
}

impl crate::source::WorldSource for FailOne {
    fn read_stamped(
        &self,
        did: Did,
    ) -> Result<Option<crate::source::StampedRecord>, tessera_assets::AssetError> {
        if let Some((d, pipeline)) = *self.fail.lock().unwrap()
            && d == did.0
        {
            return if pipeline {
                Err(tessera_assets::AssetError::Shutdown)
            } else {
                Ok(None)
            };
        }
        self.inner.read_stamped(did)
    }
}

#[test]
fn region_table_pipeline_failures_are_reported_for_a_retry() {
    // A pipeline failure (full queue, shutdown, panic) while reading slot 1, 2 or 8 leaves
    // the region out for this load and lists it for a retry; a data fault (missing table)
    // leaves it out for good (§4.1 Tessera Decision); a missing slot-8 table only drops
    // the terrain-type test (§4.1, §7.4 step 7).
    let mut world = testkit::SyntheticWorld::rectangle(1, 10..=10, 20..=20);
    world.scenery = true;
    let src = Arc::new(FailOne {
        inner: world.build(),
        fail: std::sync::Mutex::new(None),
    });
    let ctx =
        crate::WorldContext::load(src.clone(), crate::BundleOptions::default()).expect("context");
    let set = |f: Option<(u32, bool)>| *src.fail.lock().unwrap() = f;

    let all = SceneryTables::load_all_regions(&ctx);
    assert!(all.retry.is_empty());
    assert!(
        all.tables
            .get(&1)
            .is_some_and(|t| t.terrain_types.is_some())
    );

    for did in [0x1100_0000, 0x1200_0000, 0x1900_0000] {
        set(Some((did, true)));
        let all = SceneryTables::load_all_regions(&ctx);
        assert!(!all.tables.contains_key(&1), "{did:#010x}");
        assert_eq!(
            all.retry.iter().copied().collect::<Vec<_>>(),
            [1],
            "{did:#010x}"
        );
        assert!(SceneryTables::for_all_regions(&ctx).is_empty());
        // The retry succeeds once the pipeline is back.
        set(None);
        assert!(SceneryTables::for_region(&ctx, 1).is_ok());
    }

    // Data faults are not retried.
    for did in [0x1100_0000, 0x1200_0000] {
        set(Some((did, false)));
        let all = SceneryTables::load_all_regions(&ctx);
        assert!(all.tables.is_empty() && all.retry.is_empty(), "{did:#010x}");
    }
    set(Some((0x1900_0000, false)));
    let all = SceneryTables::load_all_regions(&ctx);
    assert!(all.retry.is_empty());
    assert!(
        all.tables
            .get(&1)
            .is_some_and(|t| t.terrain_types.is_none())
    );
}

#[test]
fn world_context_retries_region_tables_after_a_pipeline_failure() {
    // A pipeline failure while the context loads a region's tables leaves the region
    // without them only until a later request loads them (§4.1 implementer note); the
    // landblocks decoded after that get scenery.
    let mut world = testkit::SyntheticWorld::rectangle(1, 10..=10, 20..=20);
    world.scenery = true;
    let src = Arc::new(FailOne {
        inner: world.build(),
        fail: std::sync::Mutex::new(Some((0x1200_0000, true))),
    });
    let set = |f: Option<(u32, bool)>| *src.fail.lock().unwrap() = f;
    let ctx =
        crate::WorldContext::load(src.clone(), crate::BundleOptions::default()).expect("context");
    let k = LandblockKey::new(1, 10, 20).expect("key");
    let bundle = |ctx: &crate::WorldContext| {
        let bs = crate::BundleSource::new(Arc::clone(ctx.source()));
        let frame = tessera_assets::RecordSource::read_record(&bs, k.data_did().0).expect("frame");
        crate::bundle::build_bundle(ctx, k.data_did(), &frame, &|| false).expect("bundle")
    };

    // As loaded: no tables, a pending retry, and the scene library ready for it.
    assert!(ctx.regions()[&1].scenery.is_none());
    assert_eq!(ctx.scenery_retry_pending().collect::<Vec<_>>(), [1]);
    assert!(ctx.systems().scenes.is_some());
    // Still failing: the request finds nothing and keeps the retry. The landblock built
    // meanwhile has no scenery and is incomplete (§4.1 Tessera Decision), so it is built
    // again on its next load.
    assert!(ctx.scenery_tables(1).is_none());
    let waiting = bundle(&ctx);
    assert!(waiting.scenery.is_none());
    assert!(!waiting.is_complete(), "built while the tables wait");
    assert!(
        waiting
            .problems
            .iter()
            .any(|p| p.transient && p.what == "scenery"),
        "{:?}",
        waiting.problems
    );
    assert_eq!(ctx.scenery_retry_pending().collect::<Vec<_>>(), [1]);
    // A frill sector built meanwhile is not generated (it is tried again later), whether
    // from the bundle built while the tables waited or from any other.
    let mut thinner = DensityThinner::with_seed(3);
    assert!(
        crate::streamer::try_build_frill_sector(&ctx, &waiting, 8, 8, 1.0, &mut thinner).is_none()
    );
    // The pipeline is back: the next request loads the tables, for good.
    set(None);
    // The bundle built while they waited still yields no sector: it is rebuilt first.
    assert!(
        crate::streamer::try_build_frill_sector(&ctx, &waiting, 8, 8, 1.0, &mut thinner).is_none()
    );
    let b = bundle(&ctx);
    let sc = b.scenery.as_ref().expect("scenery after the retry");
    assert!(sc.complete);
    assert!(b.is_complete());
    assert!(
        crate::streamer::try_build_frill_sector(&ctx, &b, 8, 8, 1.0, &mut thinner).is_some(),
        "generated once the tables are there"
    );
    assert!(!b.generated.is_empty());
    assert!(ctx.region(1).is_some_and(|r| r.scenery.is_some()));
    assert_eq!(ctx.scenery_retry_pending().count(), 0);
    set(Some((0x1200_0000, true)));
    assert!(ctx.scenery_tables(1).is_some(), "remembered once loaded");

    // A data fault on the retry ends it: the region has no scenery for good.
    let ctx =
        crate::WorldContext::load(src.clone(), crate::BundleOptions::default()).expect("context");
    assert_eq!(ctx.scenery_retry_pending().collect::<Vec<_>>(), [1]);
    set(Some((0x1200_0000, false)));
    assert!(ctx.scenery_tables(1).is_none());
    assert_eq!(ctx.scenery_retry_pending().count(), 0);
    // A landblock of a region whose tables fail on the data is complete, without scenery.
    let final_ = bundle(&ctx);
    assert!(final_.scenery.is_none());
    assert!(
        !final_
            .problems
            .iter()
            .any(|p| p.transient && p.what == "scenery"),
        "{:?}",
        final_.problems
    );
    set(None);
    assert!(
        ctx.scenery_tables(1).is_none(),
        "a data fault is not retried"
    );

    // Without scenery generation nothing is pending.
    set(Some((0x1200_0000, true)));
    let off = crate::WorldContext::load(
        src.clone(),
        crate::BundleOptions::default().without_world_systems(),
    )
    .expect("context");
    assert_eq!(off.scenery_retry_pending().count(), 0);
    assert!(off.scenery_tables(1).is_none());
}
