// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of `show` and `census` against synthetic archives holding the world
//! records of M4.5.
//!
//! The archives come from the project's own test writer (shared with `crates/dat/tests`);
//! every record payload is produced by the `tessera-formats` encoders or written here from
//! the specs' layouts. No game data is involved. Every command gets `--game-dir` pointing at
//! the synthetic install, so no lookup ever reaches an install on the machine.

#[allow(dead_code, clippy::all)]
#[path = "../../../crates/dat/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use common::Builder;
use predicates::prelude::*;
use serde_json::Value;
use tessera_formats::collision::{CollisionMesh, PLANE_TAG_BPFL, TreeBlock};
use tessera_formats::environment::{
    CameraBody, CameraState, DayDescription, DayGroup, GameTime, Keyframe, ListRef, LodClass,
    LodClassTable, Month, SkyDescription, SkyEntry, SkyObject, Weighted, WeightedList,
};
use tessera_formats::landblock::{
    CollisionTree, HeightGrid, LandblockData, RegionDescriptor, RegionDidMap, RegionDidMapEntry,
};
use tessera_formats::reader::write_vle;
use tessera_formats::scenery::{
    PlacementNode, SceneChild, SceneChoice, SceneChoiceEntry, SceneChoiceTable, SceneGroup,
    ScenePair, TerrainSceneEntry, TerrainSceneTable,
};
use tessera_formats::terrain_type::{TerrainType, TerrainTypeTable};
use tessera_formats::water::{FogZone, FogZoneTable, WaterTable, WaterType, WetWater};
use tessera_formats::{Did, Scene};
use tessera_mesh::CollisionTree as MeshTree;
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec};

const TERRAIN_TYPES: u32 = 0x1900_0001;
const TERRAIN_SCENES: u32 = 0x1200_0001;
const SCENE_CHOICES: u32 = 0x1100_0001;
const SCENE: u32 = 0x0200_0001;
const WATER: u32 = 0x1600_0001;
const FOG: u32 = 0x1700_0001;
const SKY: u32 = 0x1500_0007;
const DAY: u32 = 0x1A00_0001;
const GAME_TIME: u32 = 0x3500_0000;
const LOD: u32 = 0x3200_0000;
const CAMERA: u32 = 0x0800_0001;
const MESH: u32 = 0x6600_0001;
const MESH_OLD: u32 = 0x6600_0002;
const MESH_BAD_TREE: u32 = 0x6600_0003;
const CELL: u32 = 0x0100_0ABC;
/// Corrupt fixture: a cell whose embedded render mesh is cut short.
const CELL_BAD_MESH: u32 = 0x0100_0ABD;
/// Corrupt fixture: a cell whose embedded mesh level reports a key of a missing draw.
const CELL_BAD_KEYS: u32 = 0x0100_0ABE;
/// A mesh record (0x06) with one plain and one renumbered level key set.
const RENDER_MESH: u32 = 0x0600_0001;
const RENDER_MESH_RENUMBERED: u32 = 0x0600_0002;
const REGION: u32 = 0x0F00_0001;
const LANDBLOCK: u32 = 0x8001_0203;
const LANDBLOCK_PROPS: u32 = 0x8041_0203;
const LIGHT_MAP_PID: u32 = 0x1000_0101;
const CEILING_LIGHT_MAP_PID: u32 = 0x1000_0102;

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn f32s(v: &mut Vec<u8>, xs: &[f32]) {
    for x in xs {
        v.extend_from_slice(&x.to_le_bytes());
    }
}

fn pascal(v: &mut Vec<u8>, s: &str) {
    write_vle(v, s.len() as u32);
    v.extend_from_slice(s.as_bytes());
}

fn terrain_types() -> Vec<u8> {
    let entries = vec![
        TerrainType {
            key: 1,
            label: "grass".into(),
            draw_order: 2,
            diffuse: Did(0x4000_0010),
            u_scale: 4.0,
            v_scale: 4.0,
            use_opacity_mask: true,
            ..TerrainType::default()
        },
        TerrainType {
            key: 0,
            label: "rock".into(),
            draw_order: 1,
            diffuse: Did(0x4000_0011),
            normal: Did(0x4000_0012),
            use_specular: true,
            ..TerrainType::default()
        },
    ];
    TerrainTypeTable::new(Did(TERRAIN_TYPES), entries)
        .unwrap()
        .to_bytes()
}

fn terrain_scenes() -> Vec<u8> {
    let entry = TerrainSceneEntry {
        name: "meadow".into(),
        kind: 3,
        id: 7,
        unknown_f: 0.5,
        default_pair: Some(ScenePair {
            scene_key: 0x10,
            slot: 0,
        }),
        pairs: vec![ScenePair {
            scene_key: 0x11,
            slot: 2,
        }],
    };
    let default = TerrainSceneEntry {
        name: "default".into(),
        ..TerrainSceneEntry::default()
    };
    TerrainSceneTable::new(Did(TERRAIN_SCENES), 0, Some(default), vec![entry])
        .unwrap()
        .to_bytes()
}

fn scene_choices() -> Vec<u8> {
    let entry = SceneChoiceEntry {
        key: 0x11,
        choices: vec![
            SceneChoice {
                scene: Did(SCENE),
                weight: 3.0,
                flag: 1,
            },
            SceneChoice {
                scene: Did(0x0200_0002),
                weight: 1.0,
                flag: 1,
            },
        ],
    };
    SceneChoiceTable::new(0, Did(SCENE_CHOICES), vec![entry])
        .unwrap()
        .to_bytes()
}

fn scene() -> Vec<u8> {
    let node = PlacementNode {
        frequency: 0.25,
        scale_min: [1.0; 3],
        scale_max: [2.0; 3],
        spacing: 3.0,
        children: vec![SceneChild::Object(Did(0x4700_0042))],
        ..PlacementNode::default()
    };
    Scene {
        did: Did(SCENE),
        flag_a: true,
        frill_layer: None,
        groups: vec![SceneGroup {
            group_id: 1,
            children: vec![
                SceneChild::Node(Box::new(node)),
                SceneChild::Scene(Did(0x0200_0002)),
            ],
        }],
        ..Scene::default()
    }
    .to_bytes()
}

fn water() -> Vec<u8> {
    let entries = vec![
        WaterType { key: 0, wet: None },
        WaterType {
            key: 1,
            wet: Some(WetWater {
                surface: Did(0x3100_0055),
                volume_entity: Did(0x4700_0066),
                value: 24.0,
                ..WetWater::default()
            }),
        },
    ];
    WaterTable::new(Did(WATER), 0, 0x10, 0x11, None, entries).to_bytes()
}

fn fog() -> Vec<u8> {
    let zone = FogZone {
        key: 4,
        near: 10.0,
        far: 200.0,
        ..FogZone::default()
    };
    FogZoneTable::new(Did(FOG), 0, 0x12, None, vec![zone]).to_bytes()
}

fn sky() -> Vec<u8> {
    let list = |d: u32| {
        ListRef::Inline(WeightedList {
            key: 0,
            pairs: vec![Weighted {
                value: Did(d),
                weight: 1.0,
            }],
        })
    };
    SkyDescription {
        did: Did(SKY),
        game_time: Did(GAME_TIME),
        sky_interval_min: 180.0,
        sky_interval_max: 180.0,
        groups: vec![DayGroup {
            day_type: 1,
            chance: 15.0,
            follow: ListRef::Named(3),
            entries: vec![SkyEntry {
                sky_key: 2,
                list: list(DAY),
            }],
        }],
        named_follow_lists: vec![WeightedList {
            key: 3,
            pairs: vec![Weighted {
                value: 1,
                weight: 1.0,
            }],
        }],
        local_defaults: vec![SkyEntry {
            sky_key: 5,
            list: list(DAY),
        }],
        ..SkyDescription::default()
    }
    .to_bytes()
}

fn day() -> Vec<u8> {
    DayDescription {
        did: Did(DAY),
        keyframes: vec![
            Keyframe {
                time: 0.0,
                fog_near: 100.0,
                fog_far: 900.0,
                ..Keyframe::default()
            },
            Keyframe {
                time: 0.5,
                ..Keyframe::default()
            },
        ],
        objects: vec![SkyObject {
            index: 12,
            object: Did(0x4700_0A0A),
            end_time: 1.0,
            ..SkyObject::default()
        }],
        transition_seconds: 2.0,
        ..DayDescription::default()
    }
    .to_bytes()
}

fn game_time() -> Vec<u8> {
    GameTime {
        did: Did(GAME_TIME),
        day_length: 11_160.0,
        days_per_year: 372,
        months: vec![
            Month {
                start_day: 0,
                name: 0x100,
            },
            Month {
                start_day: 31,
                name: 0x101,
            },
        ],
        weekdays: vec![0x200, 0x201],
        ..GameTime::default()
    }
    .to_bytes()
}

fn lod() -> Vec<u8> {
    LodClassTable {
        did: Did(LOD),
        entries: (1..=3)
            .map(|c| LodClass {
                class: c,
                values: [c as f32 * 10.0; 8],
            })
            .collect(),
    }
    .to_bytes()
}

fn camera() -> Vec<u8> {
    CameraState {
        entity: Did(0x4700_0001),
        collision_smoothing: 0.2,
        near_clip: 0.1,
        far_clip: 2_000.0,
        uses_physics: true,
        uses_water_physics: true,
        follows_under_water: false,
        auto_transitions_disabled: false,
        body: CameraBody::Cinematic(Default::default()),
    }
    .to_bytes()
}

fn mesh(build_byte: Option<u8>, code: Vec<u8>) -> Vec<u8> {
    CollisionMesh {
        vertices: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
        triangles: vec![[0, 1, 2], [1, 3, 2]],
        tree: TreeBlock {
            origin: [0.0; 3],
            scale: 65_536.0,
            code,
            build_byte,
        },
        ..CollisionMesh::default()
    }
    .to_bytes()
}

/// A synthetic render mesh (`mesh.md` §3) for stamp 39 whose level 0 carries `code` as its
/// tree. `draws` gives the vertex count of each submesh, one draw each.
fn render_mesh(did: u32, draws: &[u32], code: Vec<u8>) -> Vec<u8> {
    let mut spec = MeshSpec::single(0x0000_0112, 8);
    spec.did = did;
    spec.submeshes = draws
        .iter()
        .map(|&vertices| SubmeshSpec {
            format: 0x0000_0112,
            vertices,
        })
        .collect();
    spec.levels = vec![LevelSpec::detail((0..draws.len() as u32).collect())];
    let mut m = spec.build().unwrap();
    let Some(MeshTree::Code(c)) = &mut m.levels[0].tree else {
        panic!("detail level without tree code");
    };
    c.code_len = code.len() as u32;
    c.code = Some(code);
    m.to_bytes().unwrap()
}

/// A cell (`collision.md` §5.1) with one plane, one portal, no physics mesh, an empty
/// property set and an embedded render mesh whose one level reports key 0.
fn cell() -> Vec<u8> {
    cell_with(CELL, &render_mesh(CELL, &[8], vec![0x30])[4..])
}

/// [`cell`] with `did` and the embedded render mesh `body` (a mesh record without its DID).
fn cell_with(did: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    u32le(&mut b, did);
    b.push(0);
    u32le(&mut b, PLANE_TAG_BPFL);
    f32s(&mut b, &[0.0, 0.0, 1.0, 0.0]);
    u32le(&mut b, 1); // portals
    b.push(1);
    u32le(&mut b, 4);
    f32s(
        &mut b,
        &[4.0, -4.0, 0.0, 4.0, 4.0, 0.0, 4.0, 4.0, 4.0, 4.0, -4.0, 4.0],
    );
    u32le(&mut b, 0); // walls
    b.push(0); // hull
    b.extend_from_slice(&[0, 0]); // empty property set
    u32le(&mut b, 1);
    b.push(0);
    b.extend_from_slice(body);
    b
}

fn region() -> Vec<u8> {
    let map = |d: u32| {
        RegionDidMap::new(
            1,
            &[RegionDidMapEntry {
                key: 0,
                did: Did(d),
            }],
        )
        .unwrap()
    };
    RegionDescriptor {
        did: Did(REGION),
        region: 1,
        map_image: Did(0x4100_000F),
        value_12: 2,
        map_origin: [1, 2],
        map_offset: [0.0; 2],
        map_scale: 0.005,
        maps: [
            map(SCENE_CHOICES),
            map(TERRAIN_SCENES),
            map(0x1400_0001), // encounter table: not in the archive
            map(SKY),
            map(WATER),
            map(FOG),
            map(0x1800_0001),
            RegionDidMap::default(),
            RegionDidMap::default(),
        ],
        trailing: Did(TERRAIN_TYPES),
    }
    .to_bytes()
}

/// A landblock with a terrain tree (one leaf for key 5); `build_byte` as the stamp says.
fn landblock(build_byte: Option<u8>) -> Vec<u8> {
    let mut d = LandblockData::from_heights(Did(LANDBLOCK), HeightGrid::flat(1000));
    d.terrain_tree = Some(CollisionTree {
        origin: [0.0; 3],
        scale: 1.0,
        code: vec![0x35],
        build_byte,
    });
    d.to_bytes()
}

/// A property dictionary (`property-system.md` §2) defining the two light-map PIDs as DIDs.
fn dictionary() -> Vec<u8> {
    let names = [
        (0u32, "Invalid"),
        (LIGHT_MAP_PID, "Landblock_LightMap"),
        (CEILING_LIGHT_MAP_PID, "Landblock_CeilingLightMap"),
    ];
    let mut b = Vec::new();
    u32le(&mut b, 0x3400_0000);
    b.extend_from_slice(&[0; 8]);
    b.push(0);
    write_vle(&mut b, names.len() as u32);
    for (pid, name) in names {
        u32le(&mut b, pid);
        pascal(&mut b, name);
    }
    b.extend_from_slice(&[0, 0]);
    b.push(0);
    write_vle(&mut b, 2);
    for pid in [LIGHT_MAP_PID, CEILING_LIGHT_MAP_PID] {
        for v in [pid, pid, 20, 0x1000_0001, 4, 0, 0, 0] {
            u32le(&mut b, v);
        }
        u32le(&mut b, 0x3FC0_0000);
        b.extend_from_slice(&[2, 2, 4, 0, 1]);
        write_vle(&mut b, 0);
        u32le(&mut b, 0);
        u32le(&mut b, 0);
    }
    b
}

fn landblock_properties() -> Vec<u8> {
    let mut b = Vec::new();
    u32le(&mut b, LANDBLOCK_PROPS);
    b.push(0);
    write_vle(&mut b, 1);
    u32le(&mut b, LIGHT_MAP_PID);
    u32le(&mut b, LIGHT_MAP_PID);
    u32le(&mut b, 0x4100_9999);
    b
}

struct Fixture {
    _dir: tempfile::TempDir,
    game: PathBuf,
}

/// `stamp` overrides the directory stamp of the last record added.
fn set_stamp(b: &mut Builder, stamp: u16) {
    if let Some(r) = b.records.last_mut() {
        r.stamp = stamp;
    }
}

fn fixture(corrupt_day: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    std::fs::create_dir_all(&game).unwrap();

    let mut g = Builder::new();
    g.fanout = 4;
    g.stored(TERRAIN_TYPES, terrain_types());
    g.stored(TERRAIN_SCENES, terrain_scenes());
    g.stored(SCENE_CHOICES, scene_choices());
    g.stored(SCENE, scene());
    g.stored(WATER, water());
    g.stored(FOG, fog());
    g.stored(SKY, sky());
    let mut d = day();
    if corrupt_day {
        d.truncate(d.len() - 3);
        // A mesh that decodes but whose tree reports key 2 for its two triangles (CO4).
        g.stored(MESH_BAD_TREE, mesh(Some(1), vec![0x32]));
        // A surface record (decoded by tessera-material) that is only its DID.
        g.stored(0x3100_0001, 0x3100_0001u32.to_le_bytes().to_vec());
        // A cell whose render mesh (part 11) ends early: the cell must not count as decoded.
        let mesh = render_mesh(CELL_BAD_MESH, &[8], vec![0x30]);
        g.stored(
            CELL_BAD_MESH,
            cell_with(CELL_BAD_MESH, &mesh[4..mesh.len() - 5]),
        );
        // A cell whose render-mesh level reports draw 1 of a one-draw level (§3.5).
        let mesh = render_mesh(CELL_BAD_KEYS, &[8], vec![0x53, 0x00, 0x10, 0x00, 0x00]);
        g.stored(CELL_BAD_KEYS, cell_with(CELL_BAD_KEYS, &mesh[4..]));
    }
    g.stored(DAY, d);
    g.stored(GAME_TIME, game_time());
    g.stored(LOD, lod());
    g.stored(CAMERA, camera());
    // Stamp 39 (the writer's default): the tree's build byte is present.
    g.stored(MESH, mesh(Some(1), vec![0x31]));
    g.stored(MESH_OLD, mesh(None, vec![0x30]));
    set_stamp(&mut g, 33);
    g.stored(CELL, cell());
    g.stored(REGION, region());
    // Mesh records: key 0x100003 is draw 1, triangle 3, valid only with the single-triangle
    // draw 1 left out of the numbering (`collision.md` §3.5).
    g.stored(RENDER_MESH, render_mesh(RENDER_MESH, &[8], vec![0x31]));
    g.stored(
        RENDER_MESH_RENUMBERED,
        render_mesh(
            RENDER_MESH_RENUMBERED,
            &[8, 3, 8],
            vec![0x53, 0x00, 0x10, 0x00, 0x03],
        ),
    );
    std::fs::write(game.join("client_general.dat"), g.build().bytes).unwrap();

    let mut gl = Builder::new();
    gl.dataset = 0x0101_0000;
    gl.stored(0x3400_0000, dictionary());
    std::fs::write(game.join("client_gamelogic.dat"), gl.build().bytes).unwrap();

    let mut c = Builder::new();
    c.dataset = 0x0203_0000;
    c.subset = 1;
    c.stored(LANDBLOCK, landblock(Some(1)));
    c.stored(LANDBLOCK_PROPS, landblock_properties());
    std::fs::write(game.join("client_cell_1.dat"), c.build().bytes).unwrap();

    Fixture { _dir: dir, game }
}

fn tool() -> Command {
    let mut c = Command::cargo_bin("tessera-dattool").unwrap();
    c.env_remove("TESSERA_GAME_DIR").env("RUST_LOG", "error");
    c
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn show_json(f: &Fixture, did: u32) -> Value {
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "show",
            s(&f.game),
            &format!("{did:08x}"),
        ])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    v["record"].clone()
}

fn show_text(f: &Fixture, did: u32, extra: &[&str]) -> String {
    let did = format!("{did:08x}");
    let mut args = vec!["--game-dir", s(&f.game), "show", s(&f.game), &did];
    args.extend_from_slice(extra);
    let a = tool().args(&args).assert().success();
    String::from_utf8(a.get_output().stdout.clone()).unwrap()
}

#[test]
fn help_lists_show_and_census() {
    tool()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("census"));
}

#[test]
fn show_terrain_types_and_scene_tables() {
    let f = fixture(false);
    let v = show_json(&f, TERRAIN_TYPES);
    assert_eq!(v["type"], "terrain type table");
    assert_eq!(v["entries"].as_array().unwrap().len(), 2);
    assert_eq!(v["entries"][0]["label"], "grass");
    assert_eq!(v["entries"][1]["normal"], "0x40000012");
    // Text lists by key: "rock" (key 0) before "grass" (key 1).
    let t = show_text(&f, TERRAIN_TYPES, &[]);
    assert!(
        t.contains("terrain type table 0x19000001: 2 entries"),
        "{t}"
    );
    let rock = t.find("rock").unwrap();
    assert!(rock < t.find("grass").unwrap());
    assert!(t.contains("O-----"), "{t}");
    // --max-list cuts text lists and says so.
    let t = show_text(&f, TERRAIN_TYPES, &["--max-list", "1"]);
    assert!(t.contains("... 1 more"), "{t}");

    let v = show_json(&f, TERRAIN_SCENES);
    assert_eq!(v["default_entry"]["name"], "default");
    assert_eq!(v["entries"][0]["id"], 7);
    assert_eq!(v["entries"][0]["pairs"][0]["slot"], 2);
    assert!(show_text(&f, TERRAIN_SCENES, &[]).contains("2:0x11"));

    let v = show_json(&f, SCENE_CHOICES);
    assert_eq!(v["entries"][0]["total_weight"], 4.0);
    assert_eq!(v["entries"][0]["choices"][0]["scene"], "0x02000001");

    let v = show_json(&f, SCENE);
    assert_eq!(v["objects"], 1);
    assert_eq!(v["scene_references"], 1);
    assert_eq!(v["groups"][0]["children"][0]["node"]["frequency"], 0.25);
    let t = show_text(&f, SCENE, &[]);
    assert!(t.contains("object 0x47000042"), "{t}");
    assert!(t.contains("scene 0x02000002"), "{t}");
}

#[test]
fn show_water_fog_sky_day_time_lod_camera() {
    let f = fixture(false);
    let v = show_json(&f, WATER);
    assert_eq!(v["entries"][1]["wet"]["surface"], "0x31000055");
    assert!(v["entries"][0]["wet"].is_null());
    let t = show_text(&f, WATER, &[]);
    assert!(t.contains("2 listed entries"), "{t}");
    assert!(t.contains("dry"), "{t}");

    let v = show_json(&f, FOG);
    assert_eq!(v["entries"][0]["far"], 200.0);

    let v = show_json(&f, SKY);
    assert_eq!(v["game_time"], "0x35000000");
    assert_eq!(v["groups"][0]["follow"]["named"], 3);
    assert_eq!(v["day_descriptions"], 2);
    let t = show_text(&f, SKY, &[]);
    assert!(t.contains("0x1A000001(1)"), "{t}");
    assert!(t.contains("named follow list 0x3"), "{t}");

    let v = show_json(&f, DAY);
    assert_eq!(v["keyframes"].as_array().unwrap().len(), 2);
    assert_eq!(v["objects"][0]["index"], 12);
    assert!(show_text(&f, DAY, &[]).contains("2 keyframes, 1 sky objects"));

    let v = show_json(&f, GAME_TIME);
    assert_eq!(v["days_per_year"], 372);
    assert_eq!(v["months"][1]["start_day"], 31);

    let v = show_json(&f, LOD);
    assert_eq!(v["classes"].as_array().unwrap().len(), 3);
    assert!(show_text(&f, LOD, &[]).contains("3 classes"));

    let v = show_json(&f, CAMERA);
    assert_eq!(v["controller_type"], 3);
    assert_eq!(v["entity"], "0x47000001");
    assert!(show_text(&f, CAMERA, &[]).contains("cinematic"));
}

#[test]
fn show_collision_records_with_their_stamps() {
    let f = fixture(false);
    let v = show_json(&f, MESH);
    assert_eq!(v["tree"]["build_byte"], 1);
    assert_eq!(v["tree"]["walk"]["keys"], 1);
    assert_eq!(v["tree"]["walk"]["max_key"], 1);
    assert_eq!(v["tree"]["walk"]["keys_in_range"], true);
    assert_eq!(v["layout_rule"], "stamp");
    // Stamp 33: no build byte.
    let v = show_json(&f, MESH_OLD);
    assert!(v["tree"]["build_byte"].is_null());
    let t = show_text(&f, MESH, &[]);
    assert!(t.contains("4 vertices, 2 triangles"), "{t}");
    assert!(t.contains("walk: 1 keys"), "{t}");

    let v = show_json(&f, CELL);
    assert_eq!(v["portals"][0]["lead"], 1);
    assert_eq!(v["planes"].as_array().unwrap().len(), 1);
    assert!(v["physics_mesh"].is_null());
    assert!(show_text(&f, CELL, &[]).contains("1 portals"));
}

#[test]
fn show_region_slots_and_landblocks() {
    let f = fixture(false);
    let v = show_json(&f, REGION);
    let slots = v["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 8);
    assert_eq!(slots[0]["did"], "0x11000001");
    assert_eq!(slots[0]["present"], true);
    assert_eq!(slots[2]["present"], false);
    assert_eq!(slots[7]["table"], "terrain type table");
    let t = show_text(&f, REGION, &[]);
    assert!(t.contains("encounter table"), "{t}");

    let v = show_json(&f, LANDBLOCK);
    assert_eq!(v["terrain_tree"]["build_byte"], 1);
    assert_eq!(v["terrain_tree"]["walk"]["keys"], 1);
    assert_eq!(v["terrain_tree"]["walk"]["max_key"], 5);
    assert!(v["ceiling"].is_null());
    let t = show_text(&f, LANDBLOCK, &[]);
    assert!(t.contains("terrain tree: origin"), "{t}");
    assert!(t.contains("ceiling: none"), "{t}");

    // Light maps need the dictionary, found in the set's gamelogic archive.
    let v = show_json(&f, LANDBLOCK_PROPS);
    assert_eq!(v["light_maps"]["terrain"], "0x41009999");
    assert!(v["light_maps"]["ceiling"].is_null());
    assert_eq!(v["dictionary"], "gamelogic");
    let t = show_text(&f, LANDBLOCK_PROPS, &[]);
    assert!(
        t.contains("light maps: terrain 0x41009999, ceiling -"),
        "{t}"
    );
    // A single cell archive: the dictionary comes from the gamelogic archive next to it.
    let a = tool()
        .args([
            "--game-dir",
            s(&f.game),
            "show",
            s(&f.game.join("client_cell_1.dat")),
            &format!("{LANDBLOCK_PROPS:08x}"),
        ])
        .assert()
        .success();
    assert!(String::from_utf8_lossy(&a.get_output().stdout).contains("terrain 0x41009999"));
}

#[test]
fn show_refuses_kinds_without_a_decoder_and_reports_decode_errors() {
    let f = fixture(true);
    tool()
        .args(["--game-dir", s(&f.game), "show", s(&f.game), "14000001"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no decoder"));
    tool()
        .args([
            "--game-dir",
            s(&f.game),
            "show",
            s(&f.game),
            &format!("{DAY:08x}"),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "does not decode as a day description",
        ));
    // `cat` names the decoded type of a decodable record and points at `show`.
    tool()
        .args([
            "--game-dir",
            s(&f.game),
            "cat",
            s(&f.game),
            &format!("{LOD:08x}"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "decoded: LOD-class table (use `show`",
        ));
}

#[test]
fn census_counts_the_world_records() {
    let f = fixture(false);
    let a = tool()
        .args(["--json", "--game-dir", s(&f.game), "census", s(&f.game)])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["complete"], true);
    assert_eq!(v["dictionary"], "gamelogic");
    let kinds = v["kinds"].as_array().unwrap();
    let row = |p: &str| kinds.iter().find(|k| k["prefix"] == p).unwrap().clone();
    assert_eq!(kinds.len(), 14, "{kinds:?}"); // one row per M4.5 prefix
    let tt = row("0x19");
    assert_eq!(tt["records"], 1);
    assert_eq!(tt["decoded"], 1);
    assert_eq!(tt["items"], 2);
    assert_eq!(tt["item_label"], "entries");
    assert_eq!(row("0x66")["records"], 2);
    assert_eq!(row("0x66")["items"], 4);
    assert_eq!(row("0x16")["items"], 2);
    assert_eq!(row("0x32")["items"], 3);
    assert_eq!(row("0x0F")["items"], 1);
    assert_eq!(row("0x15")["payload_bytes"], sky().len());
    // 0x01 counts only with its embedded render mesh decoded; that mesh's level tree is
    // walked as a host of its own.
    assert_eq!(row("0x01")["decoded"], 1);
    let trees = v["trees"].as_array().unwrap();
    let host = |h: &str| {
        trees
            .iter()
            .find(|t| t["host"] == h)
            .unwrap_or_else(|| panic!("no {h} in {trees:?}"))
            .clone()
    };
    assert_eq!(trees.len(), 2, "{trees:?}");
    assert_eq!(host("0x66 collision meshes")["walked"], 2);
    assert_eq!(host("0x01 embedded mesh levels")["walked"], 1);
    assert_eq!(host("0x01 embedded mesh levels")["keys_out_of_range"], 0);
    // Meshes (0x06) only when asked for; their level trees are walked and a renumbered
    // level (§3.5) is counted, not failed.
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "06",
        ])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["kinds"][0]["kind"], "mesh");
    assert_eq!(v["kinds"][0]["decoded"], 2);
    assert_eq!(v["kinds"][0]["item_label"], "levels");
    assert_eq!(v["trees"][0]["host"], "0x06 mesh levels");
    assert_eq!(v["trees"][0]["walked"], 2);
    assert_eq!(v["trees"][0]["renumbered"], 1);
    assert_eq!(v["trees"][0]["keys_out_of_range"], 0);
    // Landblocks only when asked for.
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "80",
        ])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    let kinds = v["kinds"].as_array().unwrap();
    assert_eq!(kinds.len(), 2);
    let props = kinds
        .iter()
        .find(|k| k["kind"] == "landblock properties")
        .unwrap();
    assert_eq!(props["items"], 1);
    let trees = v["trees"].as_array().unwrap();
    assert_eq!(trees[0]["host"], "0x80 terrain trees");
    assert_eq!(trees[0]["walked"], 1);
    // Text output.
    tool()
        .args(["--game-dir", s(&f.game), "census", s(&f.game)])
        .assert()
        .success()
        .stdout(predicate::str::contains("terrain type table"))
        .stdout(predicate::str::contains("0x66 collision meshes"))
        .stdout(predicate::str::contains("0x01 embedded mesh levels"))
        .stdout(predicate::str::contains(
            "every selected record decoded, every tree in them walked with valid keys",
        ));
}

#[test]
fn census_fails_on_a_bad_record_and_a_bad_prefix() {
    let f = fixture(true);
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "1a",
        ])
        .assert()
        .code(1);
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["failures_total"], 1);
    assert_eq!(v["failures"][0]["did"], "0x1A000001");
    assert_eq!(v["failures"][0]["category"], "Truncated");
    tool()
        .args(["--game-dir", s(&f.game), "census", s(&f.game)])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("failures (3 listed of 3)"))
        .stdout(predicate::str::contains("FAILED"));
    // A tree key out of range is a failure even though every record decodes.
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "66",
        ])
        .assert()
        .code(1);
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["failures_total"], 0);
    assert_eq!(v["trees"][0]["keys_out_of_range"], 1);
    assert_eq!(v["tree_failures"][0]["did"], "0x66000003");
    // A cell whose embedded render mesh does not decode is a failure of its 0x01 record;
    // a level key of a missing draw is a tree problem.
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "01",
        ])
        .assert()
        .code(1);
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["kinds"][0]["records"], 3);
    assert_eq!(v["kinds"][0]["decoded"], 2);
    assert_eq!(v["failures_total"], 1);
    assert_eq!(v["failures"][0]["did"], "0x01000ABD");
    assert_eq!(v["failures"][0]["category"], "RenderMesh/Truncated");
    assert_eq!(v["trees"][0]["host"], "0x01 embedded mesh levels");
    assert_eq!(v["trees"][0]["walked"], 2);
    assert_eq!(v["trees"][0]["keys_out_of_range"], 1);
    assert_eq!(v["tree_failures"][0]["did"], "0x01000ABE");
    // The material family goes through tessera-material.
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "31",
        ])
        .assert()
        .code(1);
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["kinds"][0]["kind"], "surface descriptor");
    assert_eq!(v["kinds"][0]["failed"], 1);
    assert_eq!(v["failures"][0]["category"], "Truncated");
    tool()
        .args([
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "14",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no decoder for prefix 0x14"));
}

/// `show` and `census` read only: the synthetic install is byte-identical afterwards.
#[test]
fn show_and_census_leave_the_install_untouched() {
    let f = fixture(false);
    let snapshot = |dir: &Path| {
        let mut v: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let b = std::fs::read(&p).unwrap();
                (p, b)
            })
            .collect();
        v.sort();
        v
    };
    let before = snapshot(&f.game);
    tool()
        .args([
            "--game-dir",
            s(&f.game),
            "census",
            s(&f.game),
            "--prefix",
            "80",
        ])
        .assert()
        .success();
    tool()
        .args([
            "--game-dir",
            s(&f.game),
            "show",
            s(&f.game),
            &format!("{REGION:08x}"),
        ])
        .assert()
        .success();
    assert_eq!(before, snapshot(&f.game));
}
