// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the water module: the worked examples of `water.md` §12, the §6.2
//! steps, volumes (§7), queries and sectors (§9). Every fixture is built here.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::environment::Argb;
use tessera_formats::landblock::{HeightGrid, height_from_raw};
use tessera_formats::setup::Quaternion;
use tessera_formats::water::{
    FOG_ZONE_OVERRIDES, FogOverride, FogZone, FogZoneTable, WaterTable, WaterType, WetWater,
};
use tessera_formats::{BlockMap, Did, PropertySet, PropertyType, Record};
use tessera_physics::WaterQuery;
use tessera_physics::geometry::LiquidFlags;

use super::*;
use crate::coords::{LandblockKey, WorldPos};
use crate::scenery::SceneryWater;
use crate::source::MemoryWorldSource;
use crate::testkit::{TestValue, entity_descriptor, property_set, property_set_record};

const LEVEL_KEY: u32 = 4;
const TYPE_KEY: u32 = 7;
const VOLUME: u32 = 0x4700_0754;
const DEEP_ONLY: u32 = 0x4700_25A2;

fn key(x: u8, y: u8) -> LandblockKey {
    LandblockKey::new(1, x, y).unwrap()
}

fn wet(key: u32, volume: u32) -> WaterType {
    WaterType {
        key,
        wet: Some(WetWater {
            surface: Did(0x3100_0001),
            extra: [Did(0); 4],
            volume_entity: Did(volume),
            value: 24.0,
        }),
    }
}

/// The table of `water.md` §12: level key 4, type key 7, a dry default, key 1 wet, key 2
/// dry; plus key 3 wet with the deep-only entity.
fn table() -> WaterTable {
    WaterTable::new(
        Did(0x1600_0001),
        0,
        LEVEL_KEY,
        TYPE_KEY,
        Some(WaterType { key: 0, wet: None }),
        vec![
            wet(1, VOLUME),
            WaterType { key: 2, wet: None },
            wet(3, DEEP_ONLY),
        ],
    )
}

fn map(k: LandblockKey, pairs: &[(u32, u16)]) -> BlockMap {
    let bytes = crate::testkit::block_map(k, pairs);
    BlockMap::parse(&bytes).unwrap()
}

/// Heights with a minimum stored value of 3,000 (91.55 units) and a maximum of 40,000.
fn heights() -> HeightGrid {
    HeightGrid::from_fn(|i, j| if i == 0 && j == 0 { 3_000 } else { 40_000 })
}

#[test]
fn water_height_is_f32_level_times_1000_times_inv_255() {
    // §6.2 step 6: the f32 nearest 1/255.
    #[allow(clippy::cast_possible_truncation)]
    let nearest = (1.0_f64 / 255.0) as f32;
    assert_eq!(INV_255.to_bits(), nearest.to_bits());
    for level in [0_u16, 1, 21, 23, 38, 86, 96, 211, 255, u16::MAX] {
        let expect = (f32::from(level) * 1000.0) * nearest;
        assert_eq!(
            water_height(level).to_bits(),
            expect.to_bits(),
            "level {level}"
        );
        let exact = f64::from(level) * 1000.0 / 255.0;
        assert!((f64::from(water_height(level)) - exact).abs() <= exact * 2e-7 + 1e-6);
    }
    assert!((water_height(38) - 149.0196).abs() < 1e-4);
    assert!((water_height(86) - 337.2549).abs() < 1e-4);
    assert!((water_height(21) - 82.353).abs() < 1e-3);
    // "Within one unit in the last place" of 1,000 (§6.2 step 6).
    assert_eq!(water_height(255).to_bits() - 1000.0_f32.to_bits(), 1);
}

#[test]
fn worked_example_landblock_water() {
    // water.md §12: key 7 → 1, key 4 → 38; lowest sample 3,000 → 91.55 units.
    let t = table();
    let h = heights();
    assert!((height_from_raw(3_000) - 91.55).abs() < 0.01);
    let w = decide_water(Some(&t), &map(key(1, 1), &[(7, 1), (4, 38)]), &h).unwrap();
    assert_eq!(w.water_type, 1);
    assert_eq!(w.level, 38);
    assert!((w.height - 149.0196).abs() < 1e-4);
    assert_eq!(w.volume_entity(), Did(VOLUME));
    assert_eq!(w.surface(), Did(0x3100_0001));
    assert!(!w.is_fully_submerged());
    // A point at block-local z = 148.5 is under water: 148.5 − 149.0196 < −0.0002.
    assert!(w.is_under(148.5));
    assert!((w.signed_distance(148.5) + 0.5196).abs() < 1e-4);
    // ... and inside the wading box of the volume entity (−1 ≤ −0.5196 < −0.5).
    let template = landblock_volume_template();
    let boxes = template.place_landblock(w.height);
    let flags = boxes
        .iter()
        .filter(|b| b.contains([10.0, 10.0, 148.5]))
        .fold(LiquidFlags::default(), |a, b| a.union(b.flags));
    assert!(
        flags.liquid && flags.wading && !flags.surface && !flags.deep,
        "{flags:?}"
    );
    // With key 4 → 23, h = 90.196 < 91.55: no water although the type is set.
    assert!((water_height(23) - 90.196).abs() < 1e-3);
    assert_eq!(
        decide_water(Some(&t), &map(key(1, 1), &[(7, 1), (4, 23)]), &h),
        Err(DryReason::BelowTerrain)
    );
}

#[test]
fn every_step_of_the_rule() {
    let t = table();
    let h = heights();
    let k = key(2, 3);
    assert_eq!(
        decide_water(None, &map(k, &[(7, 1), (4, 38)]), &h),
        Err(DryReason::NoTable)
    );
    assert_eq!(
        decide_water(Some(&t), &map(k, &[(4, 38)]), &h),
        Err(DryReason::NoTypeKey)
    );
    // Type 0 and unknown types fall to the dry default.
    for ty in [0, 37, u16::MAX] {
        assert_eq!(
            decide_water(Some(&t), &map(k, &[(7, ty), (4, 38)]), &h),
            Err(DryReason::DryType)
        );
    }
    assert_eq!(
        decide_water(Some(&t), &map(k, &[(7, 2), (4, 38)]), &h),
        Err(DryReason::DryType)
    );
    let no_default = WaterTable::new(Did(0x1600_0002), 0, 4, 7, None, vec![wet(1, VOLUME)]);
    assert_eq!(
        decide_water(Some(&no_default), &map(k, &[(7, 9), (4, 38)]), &h),
        Err(DryReason::NoEntry)
    );
    assert_eq!(
        decide_water(Some(&t), &map(k, &[(7, 1)]), &h),
        Err(DryReason::NoLevelKey)
    );
    // Strictly above the lowest sample: equal is dry.
    let flat = HeightGrid::flat(0);
    assert_eq!(
        decide_water(Some(&t), &map(k, &[(7, 1), (4, 0)]), &flat),
        Err(DryReason::BelowTerrain)
    );
    let w = decide_water(Some(&t), &map(k, &[(7, 1), (4, 1)]), &flat).unwrap();
    assert!(w.is_fully_submerged());
    // The highest sample plays no part: a block with one deep hole is wet.
    let hole = HeightGrid::from_fn(|i, _| if i == 5 { 0 } else { u16::MAX });
    assert!(decide_water(Some(&t), &map(k, &[(7, 1), (4, 1)]), &hole).is_ok());
    // First listed entry wins over a later duplicate key.
    let dup = WaterTable::new(
        Did(0x1600_0003),
        0,
        4,
        7,
        None,
        vec![wet(5, VOLUME), WaterType { key: 5, wet: None }],
    );
    assert!(decide_water(Some(&dup), &map(k, &[(7, 5), (4, 38)]), &h).is_ok());
    // Keys come from the table, not fixed numbers.
    let other_keys = WaterTable::new(Did(0x1600_0004), 0, 11, 12, None, vec![wet(1, VOLUME)]);
    assert!(decide_water(Some(&other_keys), &map(k, &[(12, 1), (11, 38)]), &h).is_ok());
    assert_eq!(
        decide_water(Some(&other_keys), &map(k, &[(7, 1), (4, 38)]), &h),
        Err(DryReason::NoTypeKey)
    );
}

#[test]
fn under_water_tolerance_is_0_0002() {
    let t = table();
    let w = decide_water(
        Some(&t),
        &map(key(1, 1), &[(7, 1), (4, 255)]),
        &HeightGrid::flat(0),
    )
    .unwrap();
    let h = w.height;
    assert!(!w.is_under(h));
    assert!(!w.is_under(h - 0.0001));
    assert!(w.is_under(h - 0.001));
    assert!(!w.is_under(f32::NAN));
    assert!(!w.is_under(f32::NEG_INFINITY));
    let plane = w.plane().unwrap();
    assert_eq!(plane.level, h);
    assert_eq!(plane.water_type, 1);
}

#[test]
fn decision_is_deterministic() {
    // WA6: a pure function; the same inputs give bit-identical states.
    let t = table();
    let h = HeightGrid::from_fn(|i, j| u16::try_from((i * 997 + j * 31) % 9_000).unwrap());
    for level in 0..=255_u16 {
        let m = map(key(9, 9), &[(7, 1), (4, level)]);
        let a = decide_water(Some(&t), &m, &h);
        let b = decide_water(Some(&t), &m.clone(), &h.clone());
        match (a, b) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.height.to_bits(), b.height.to_bits());
                assert_eq!(a, b);
            }
            (a, b) => assert_eq!(a, b),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fog zones (§5.3, §12)
// ---------------------------------------------------------------------------------------

fn zone(key: u32, active: u8) -> FogZone {
    FogZone {
        key,
        overrides: [FogOverride {
            mode: 3,
            a: 0,
            b: 0,
        }; FOG_ZONE_OVERRIDES],
        near: 500.0,
        far: 10_000.0,
        amount: 0.0,
        value_60: 1.0,
        active,
        colour: Argb(0xFFFF_FFFF),
        light_a: 1.0,
        colour_a1: Argb(0),
        colour_a2: Argb(0),
        light_b: 1.0,
        colour_b: Argb(0),
        colour_c: Argb(0),
        flag_c: false,
        colour_d: Argb(0),
        e: None,
        flag_f: false,
        value_g: 0,
    }
}

#[test]
fn worked_example_fog_zones() {
    // §12: all entries inactive, or key 3 = 0 everywhere: the sky's fog is unchanged.
    let inactive = FogZoneTable::new(Did(0x1700_0001), 0, 3, Some(zone(0, 0)), vec![zone(1, 0)]);
    assert!(fog_zone_corners(&inactive, [Some(0), Some(1), Some(1), None]).is_none());
    let zones = FogZoneTable::new(Did(0x1700_0002), 0, 3, Some(zone(0, 0)), vec![zone(1, 1)]);
    assert!(fog_zone_corners(&zones, [Some(0); 4]).is_none());
    // One active corner (the north neighbour) makes the landblock a fog-zone block.
    let c = fog_zone_corners(&zones, [Some(0), Some(1), Some(0), None]).unwrap();
    assert_eq!(c[0].map(|z| z.key), Some(0));
    assert_eq!(c[1].map(|z| z.key), Some(1));
    assert!(c[3].is_none());
    // Unknown keys select the default.
    assert!(fog_zone_corners(&zones, [Some(9); 4]).is_none());
}

// ---------------------------------------------------------------------------------------
// Volumes (§7)
// ---------------------------------------------------------------------------------------

mod pid {
    pub const MONITORS: u32 = 0x5000_0001;
    pub const MONITOR: u32 = 0x5000_0002;
    pub const BOX_MIN: u32 = 0x5000_0003;
    pub const BOX_MAX: u32 = 0x5000_0004;
    pub const OFFSET: u32 = 0x5000_0005;
    pub const SCALE_WITH_ENTITY: u32 = 0x5000_0006;
    pub const PROPERTIES: u32 = 0x5000_0007;
    pub const NAME: u32 = 0x5000_0008;
    pub const IS_LIQUID: u32 = 0x5000_0010;
    pub const IS_LAVA: u32 = 0x5000_0011;
    pub const SURFACE: u32 = 0x5000_0012;
    pub const WADING: u32 = 0x5000_0013;
    pub const DEEP: u32 = 0x5000_0014;
    pub const MUSIC: u32 = 0x5000_0020;
}

const SET_SURFACE: u32 = 0x7800_0075;
const SET_WADING: u32 = 0x7800_0074;
const SET_DEEP: u32 = 0x7800_0073;
const SET_MUSIC: u32 = 0x7800_0100;

fn schema() -> HashMap<u32, PropertyType> {
    use pid::*;
    [
        (MONITORS, 12),
        (MONITOR, 11),
        (BOX_MIN, 6),
        (BOX_MAX, 6),
        (OFFSET, 6),
        (SCALE_WITH_ENTITY, 21),
        (PROPERTIES, 20),
        (NAME, 8),
        (IS_LIQUID, 21),
        (IS_LAVA, 21),
        (SURFACE, 21),
        (WADING, 21),
        (DEEP, 21),
        (MUSIC, 8),
    ]
    .into_iter()
    .map(|(p, t)| (p, PropertyType::from_code(t)))
    .collect()
}

fn pids() -> WaterPids {
    use pid::*;
    let names: HashMap<&str, u32> = [
        ("Entity_Monitors", MONITORS),
        ("Monitor_BoxMin", BOX_MIN),
        ("Monitor_BoxMax", BOX_MAX),
        ("Monitor_Offset", OFFSET),
        ("Monitor_ScaleWithEntity", SCALE_WITH_ENTITY),
        ("Monitor_VolumePropertiesToApply", PROPERTIES),
        ("Monitor_Name", NAME),
        ("Volume_IsLiquid", IS_LIQUID),
        ("Volume_IsLava", IS_LAVA),
        ("Volume_WaterSurface", SURFACE),
        ("Volume_WaterWading", WADING),
        ("Volume_WaterDeep", DEEP),
    ]
    .into_iter()
    .collect();
    WaterPids::from_names(|n| names.get(n).copied())
}

fn monitor(name: u32, min: [f32; 3], max: [f32; 3], set: u32) -> (u32, TestValue) {
    (
        pid::MONITOR,
        TestValue::Struct(vec![
            (pid::NAME, TestValue::Enum(name)),
            (pid::BOX_MIN, TestValue::Vec3(min)),
            (pid::BOX_MAX, TestValue::Vec3(max)),
            (pid::SCALE_WITH_ENTITY, TestValue::Bool(true)),
            (pid::PROPERTIES, TestValue::Did(set)),
        ]),
    )
}

/// The three monitors of a 0x47000754-like landblock volume entity (§7.1).
fn landblock_monitors() -> Vec<(u32, TestValue)> {
    vec![
        monitor(1, [0.0, 0.0, -0.5], [160.0, 160.0, 0.0], SET_SURFACE),
        monitor(3, [0.0, 0.0, -1.0], [160.0, 160.0, -0.5], SET_WADING),
        monitor(2, [0.0, 0.0, -1600.0], [160.0, 160.0, -1.0], SET_DEEP),
    ]
}

fn world() -> MemoryWorldSource {
    use pid::*;
    let s = schema();
    let mut src = MemoryWorldSource::new();
    src.insert(
        Did(VOLUME),
        entity_descriptor(
            VOLUME,
            0,
            &[(MONITORS, TestValue::Array(landblock_monitors()))],
            &s,
        ),
        None,
    );
    src.insert(
        Did(DEEP_ONLY),
        entity_descriptor(
            DEEP_ONLY,
            0,
            &[(
                MONITORS,
                TestValue::Array(vec![
                    monitor(2, [0.0, 0.0, -1600.0], [160.0, 160.0, 0.0], SET_DEEP),
                    // A music monitor is not a liquid box.
                    monitor(9, [0.0, 0.0, -10.0], [160.0, 160.0, 10.0], SET_MUSIC),
                ]),
            )],
            &s,
        ),
        None,
    );
    let set = |did: u32, entries: &[(u32, TestValue)]| (did, property_set_record(did, entries));
    for (did, bytes) in [
        set(
            SET_SURFACE,
            &[
                (IS_LIQUID, TestValue::Bool(true)),
                (SURFACE, TestValue::Bool(true)),
            ],
        ),
        set(
            SET_WADING,
            &[
                (IS_LIQUID, TestValue::Bool(true)),
                (WADING, TestValue::Bool(true)),
            ],
        ),
        set(
            SET_DEEP,
            &[
                (IS_LIQUID, TestValue::Bool(true)),
                (DEEP, TestValue::Bool(true)),
            ],
        ),
        set(SET_MUSIC, &[(MUSIC, TestValue::Enum(4))]),
    ] {
        src.insert(Did(did), bytes, None);
    }
    src
}

fn landblock_volume_template() -> Arc<VolumeTemplate> {
    let lib = VolumeLibrary::new(pids(), 16);
    lib.template(&world(), &schema(), Did(VOLUME))
        .unwrap()
        .expect("volume template")
}

#[test]
fn monitors_parse_with_offset_and_defaults() {
    use pid::*;
    let s = schema();
    let bytes = property_set(&[(
        MONITORS,
        TestValue::Array(vec![
            (
                MONITOR,
                TestValue::Struct(vec![
                    (BOX_MIN, TestValue::Vec3([-1.0, -2.0, -3.0])),
                    (BOX_MAX, TestValue::Vec3([1.0, 2.0, 3.0])),
                    (OFFSET, TestValue::Vec3([10.0, 0.0, 0.5])),
                    (PROPERTIES, TestValue::Did(SET_DEEP)),
                ]),
            ),
            // No box maximum: skipped.
            (
                MONITOR,
                TestValue::Struct(vec![(BOX_MIN, TestValue::Vec3([0.0; 3]))]),
            ),
            // Not a struct: skipped.
            (NAME, TestValue::Enum(1)),
            // Non-finite corner: skipped.
            (
                MONITOR,
                TestValue::Struct(vec![
                    (BOX_MIN, TestValue::Vec3([f32::NAN, 0.0, 0.0])),
                    (BOX_MAX, TestValue::Vec3([1.0; 3])),
                ]),
            ),
        ]),
    )]);
    let set = PropertySet::parse(&bytes, &s).unwrap();
    let ms = monitors_from_value(set.get(MONITORS).unwrap(), &pids().monitor);
    assert_eq!(ms.len(), 1);
    let m = ms[0];
    assert!(
        m.scale_with_entity,
        "absent ScaleWithEntity defaults to true"
    );
    assert_eq!(m.properties, Some(Did(SET_DEEP)));
    assert_eq!(m.name, None);
    assert_eq!(m.local_box(), ([9.0, -2.0, -2.5], [11.0, 2.0, 3.5]));
    // Not an array: nothing.
    assert!(
        monitors_from_value(&tessera_formats::PropertyValue::Int(3), &pids().monitor).is_empty()
    );
}

#[test]
fn monitor_count_is_capped() {
    use pid::*;
    let s = schema();
    let many: Vec<_> = (0..200)
        .map(|_| monitor(1, [0.0; 3], [1.0; 3], SET_DEEP))
        .collect();
    let bytes = property_set(&[(MONITORS, TestValue::Array(many))]);
    let set = PropertySet::parse(&bytes, &s).unwrap();
    let ms = monitors_from_value(set.get(MONITORS).unwrap(), &pids().monitor);
    assert_eq!(ms.len(), MAX_MONITORS_PER_OBJECT);
}

#[test]
fn landblock_volume_entity_bands() {
    // §7.1: surface zone the top 0.5 units, wading 0.5–1, deep 1–1,600 below h.
    let t = landblock_volume_template();
    assert_eq!(t.boxes.len(), 3);
    let h = water_height(38);
    let boxes = t.place_landblock(h);
    let at = |z: f32| {
        boxes
            .iter()
            .filter(|b| b.contains([80.0, 80.0, z]))
            .fold(LiquidFlags::default(), |a, b| a.union(b.flags))
    };
    let s = at(h - 0.25);
    assert!(s.liquid && s.surface && !s.wading && !s.deep);
    let w = at(h - 0.75);
    assert!(w.liquid && w.wading && !w.surface && !w.deep);
    let d = at(h - 100.0);
    assert!(d.liquid && d.deep && !d.wading);
    assert!(!at(h + 0.01).liquid);
    assert!(!at(h - 1600.5).liquid);
    // The deep-only entity keeps its liquid monitor and drops the music monitor.
    let lib = VolumeLibrary::new(pids(), 16);
    let deep = lib
        .template(&world(), &schema(), Did(DEEP_ONLY))
        .unwrap()
        .unwrap();
    assert_eq!(deep.boxes.len(), 1);
    assert!(deep.boxes[0].flags.deep);
    // Objects without monitors, missing objects and other prefixes have no volume.
    assert!(
        lib.template(&world(), &schema(), Did(0x4700_9999))
            .unwrap()
            .is_none()
    );
    assert!(
        lib.template(&world(), &schema(), Did(0x7800_0073))
            .unwrap()
            .is_none()
    );
    assert_eq!(lib.len(), 2);
}

#[test]
fn volume_cache_is_bounded() {
    let lib = VolumeLibrary::new(pids(), 2);
    let w = world();
    for did in [VOLUME, DEEP_ONLY, 0x4700_0001, 0x4700_0002, 0x4700_0003] {
        let _ = lib.template(&w, &schema(), Did(did)).unwrap();
        assert!(lib.len() <= 2);
    }
}

#[test]
fn placed_volume_follows_its_frame() {
    // A placed volume turned 90° about +Z, scaled 2, at (100, 50, 20).
    let t = VolumeTemplate {
        object: Did(0x4700_0042),
        boxes: vec![VolumeBox {
            min: [-15.0, -5.0, -3.0],
            max: [15.0, 5.0, 0.0],
            scale_with_entity: true,
            flags: LiquidFlags {
                liquid: true,
                deep: true,
                ..LiquidFlags::default()
            },
            properties: Did(0x7800_0073),
            name: Some(2),
        }],
        unreadable_sets: 0,
    };
    let q = Quaternion::from_yaw(std::f32::consts::FRAC_PI_2);
    let boxes = t.place([100.0, 50.0, 20.0], q, [2.0; 3]);
    assert_eq!(boxes.len(), 1);
    let b = &boxes[0];
    // Local +X becomes world +Y: the box spans x 100 ± 10, y 50 ± 30, z 14–20.
    assert!(b.contains([109.0, 79.0, 15.0]));
    assert!(!b.contains([111.0, 50.0, 15.0]));
    assert!(!b.contains([100.0, 81.0, 15.0]));
    assert!(!b.contains([100.0, 50.0, 20.5]));
    let bw = BlockWater {
        key: key(1, 1),
        decision: Err(DryReason::DryType),
        boxes,
        placed_volume_entities: 1,
        boxes_left_out: 0,
    };
    let s = bw.water_at([105.0, 60.0, 15.0]).unwrap();
    assert!((s.surface - 20.0).abs() < 1e-4, "{}", s.surface);
    assert!(s.flags.deep);
    assert!(bw.water_at([105.0, 60.0, 21.0]).is_none());
    assert!(bw.is_under_water([105.0, 60.0, 15.0]));
    assert!(!bw.is_under_water([5.0, 5.0, 15.0]));
    // Without ScaleWithEntity the box keeps scale 1.
    let mut fixed = t.clone();
    fixed.boxes[0].scale_with_entity = false;
    let b = fixed.place([100.0, 50.0, 20.0], q, [2.0; 3]);
    assert!(!b[0].contains([109.0, 79.0, 15.0]));
    assert!(b[0].contains([104.0, 64.0, 18.0]));
    // Frames the physics layer rejects are skipped.
    assert!(t.place([f32::NAN, 0.0, 0.0], q, [1.0; 3]).is_empty());
    assert!(t.place([0.0; 3], q, [0.0; 3]).is_empty());
}

fn wet_block(k: LandblockKey, level: u16) -> BlockWater {
    let sys = WaterSystem::new(
        WaterTables::new([(1, Arc::new(table()))], []),
        VolumeLibrary::new(pids(), 16),
    );
    sys.block_water_from(
        &world(),
        &schema(),
        k,
        Some(&map(k, &[(7, 1), (4, level)])),
        &heights(),
        &[],
        &[1],
    )
    .unwrap()
}

#[test]
fn block_water_query_and_surface() {
    let bw = wet_block(key(1, 1), 38);
    let h = water_height(38);
    assert!(bw.water().is_some());
    assert_eq!(bw.boxes.len(), 3);
    // The surface of every band is the plane's level, bit for bit.
    for z in [h - 0.1, h - 0.7, h - 50.0, h - 1599.0] {
        let s = bw.water_at([20.0, 30.0, z]).unwrap();
        assert_eq!(s.surface.to_bits(), h.to_bits(), "z {z}");
    }
    assert!(bw.water_at([20.0, 30.0, h + 0.5]).is_none());
    assert!(bw.water_at([f32::NAN, 30.0, h - 1.0]).is_none());
    // Under water: plane test first (§6.3), boxes second.
    assert!(bw.is_under_water([20.0, 30.0, h - 0.001]));
    // Within 0.0002 of the plane the plane test fails, but the point is inside the
    // surface box, so the volume query (Inferred in §6.3) answers "under water".
    assert!(!bw.water().unwrap().is_under(h - 0.0001));
    assert!(bw.is_under_water([20.0, 30.0, h - 0.0001]));
    assert!(!bw.is_under_water([20.0, 30.0, h + 0.01]));
    // Below the deep box the plane still says "under water".
    assert!(bw.is_under_water([20.0, 30.0, h - 2000.0]));
    // Off the block the plane does not apply.
    assert!(!bw.is_under_water([-1.0, 30.0, h - 2000.0]));
    // Scenery's plane: normal +Z, offset −h.
    let p = bw.water_plane(key(1, 1)).unwrap();
    assert_eq!(p.normal, [0.0, 0.0, 1.0]);
    assert_eq!(p.d, -h);
    assert!(bw.water_plane(key(1, 2)).is_none());
    // A dry block has no plane and no boxes.
    let dry = wet_block(key(1, 1), 23);
    assert_eq!(dry.decision, Err(DryReason::BelowTerrain));
    assert!(dry.boxes.is_empty());
    assert!(dry.water_at([20.0, 30.0, 10.0]).is_none());
}

#[test]
fn block_water_fills_a_collision_set() {
    use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder};
    let bw = wet_block(key(1, 1), 38);
    let mut builder = CollisionSetBuilder::new(CollisionLimits::default());
    bw.apply_to(&mut builder).unwrap();
    let set = builder.build();
    let s = set.water_at([10.0, 10.0, water_height(38) - 0.75]).unwrap();
    assert!(s.flags.wading);
    assert_eq!(s.surface, water_height(38));
}

#[test]
fn water_map_world_queries() {
    let wm = WaterMap::new();
    assert!(wm.is_empty());
    let k = key(10, 10);
    assert!(wm.insert(Arc::new(wet_block(k, 38))));
    // A dry neighbour with a placed volume crossing the shared edge.
    let mut east = wet_block(key(11, 10), 23);
    east.boxes = VolumeTemplate {
        object: Did(0x4700_0042),
        boxes: vec![VolumeBox {
            min: [-5.0, -5.0, -2.0],
            max: [5.0, 5.0, 0.0],
            scale_with_entity: true,
            flags: LiquidFlags {
                liquid: true,
                deep: true,
                ..LiquidFlags::default()
            },
            properties: Did(0x7800_0073),
            name: None,
        }],
        unreadable_sets: 0,
    }
    .place([1.0, 80.0, 500.0], Quaternion::IDENTITY, [1.0; 3]);
    assert!(wm.insert(Arc::new(east)));
    assert_eq!(wm.len(), 2);
    let h = water_height(38);
    let origin = k.origin();
    let pos = |x: f64, y: f64, z: f64| WorldPos::new(1, origin[0] + x, origin[1] + y, z);
    assert!(wm.is_under_water(&pos(50.0, 50.0, f64::from(h) - 1.0)));
    assert!(!wm.is_under_water(&pos(50.0, 50.0, f64::from(h) + 1.0)));
    let s = wm.water_at(&pos(50.0, 50.0, f64::from(h) - 3.0)).unwrap();
    assert_eq!(s.surface, h);
    assert_eq!(wm.landblock_water(&pos(1.0, 1.0, 0.0)).unwrap().height, h);
    // The neighbour's box reaches 4 units into this landblock (x 156–160 here).
    let s = wm.water_at(&pos(158.0, 80.0, 499.0)).unwrap();
    assert!((s.surface - 500.0).abs() < 1e-3);
    assert!(wm.is_under_water(&pos(158.0, 80.0, 499.0)));
    assert!(SceneryWater::water_plane(&wm, k).is_some());
    assert!(SceneryWater::water_plane(&wm, key(11, 10)).is_none());
    assert!(SceneryWater::water_plane(&wm, key(50, 50)).is_none());
    assert!(wm.remove(k).is_some());
    assert!(!wm.is_under_water(&pos(50.0, 50.0, f64::from(h) - 1.0)));
    // Off-grid and non-finite positions are dry.
    assert!(!wm.is_under_water(&WorldPos::new(1, -5.0, 0.0, 0.0)));
    assert!(wm.water_at(&WorldPos::new(1, f64::NAN, 0.0, 0.0)).is_none());
}

#[test]
fn sector_pairs_are_distinct_and_indexed() {
    let t = table();
    let h = heights();
    // Sector (3, 6): blocks at x 3–5, y 6–8; levels by column, one dry block.
    let levels = |k: LandblockKey| match (k.x(), k.y()) {
        (4, 7) => 23,
        (5, _) => 96,
        _ => 38,
    };
    let sw = SectorWater::build(key(5, 8), |k| {
        decide_water(Some(&t), &map(k, &[(7, 1), (4, levels(k))]), &h).ok()
    });
    assert_eq!(sw.origin, Some(key(3, 6)));
    assert_eq!(sector_origin(key(5, 8)), key(3, 6));
    assert_eq!(sector_origin(key(254, 254)), key(252, 252));
    assert!(sw.has_water());
    assert_eq!(sw.pairs.len(), 2);
    assert_eq!(sw.pairs[0].water_type, 1);
    assert_eq!(sw.pairs[0].height.to_bits(), water_height(38).to_bits());
    assert_eq!(sw.pairs[1].height.to_bits(), water_height(96).to_bits());
    assert_eq!(sw.blocks[1][1], None);
    assert_eq!(sw.blocks[0][0], Some(0));
    assert_eq!(sw.blocks[2][2], Some(1));
    // The stored form is the type byte then the f32 (§9).
    let b = sw.pairs[0].to_bytes();
    assert_eq!(b[0], 1);
    assert_eq!(
        f32::from_le_bytes([b[1], b[2], b[3], b[4]]),
        water_height(38)
    );
    // The grid edge: 255 landblocks per side is a multiple of 3, so the last sector
    // (252–254 on both axes) is complete. All nine landblocks are asked for, X-major, and
    // every one of them gets its pair.
    let mut asked = Vec::new();
    let edge = SectorWater::build(key(254, 254), |k| {
        asked.push((k.x(), k.y()));
        decide_water(Some(&t), &map(k, &[(7, 1), (4, 38)]), &h).ok()
    });
    let want: Vec<(u8, u8)> = (252..=254)
        .flat_map(|x| (252..=254).map(move |y| (x, y)))
        .collect();
    assert_eq!(asked, want);
    assert_eq!(edge.origin, Some(key(252, 252)));
    assert_eq!(edge.pairs.len(), 1);
    assert_eq!(edge.blocks, [[Some(0); 3]; 3]);
    // A sector whose landblocks are all dry or absent has no water.
    let dry = SectorWater::build(key(254, 254), |_| None);
    assert!(!dry.has_water());
    assert_eq!(dry.blocks, [[None; 3]; 3]);
}

fn staircase_box(z: f32, height: f32) -> tessera_physics::geometry::LiquidBox {
    VolumeTemplate {
        object: Did(0x4700_0042),
        boxes: vec![VolumeBox {
            min: [0.0, 0.0, 0.0],
            max: [10.0, 10.0, height],
            scale_with_entity: true,
            flags: LiquidFlags {
                liquid: true,
                deep: true,
                ..LiquidFlags::default()
            },
            properties: Did(0x7800_0073),
            name: None,
        }],
        unreadable_sets: 0,
    }
    .place([0.0, 0.0, z], Quaternion::IDENTITY, [1.0; 3])[0]
}

/// The surface walk of the former implementation: join every touching interval until
/// nothing changes (quadratic; kept here as the reference for the sweep).
fn reference_top(p: [f32; 3], boxes: &[tessera_physics::geometry::LiquidBox]) -> Option<f32> {
    let inside = boxes.iter().any(|b| b.contains(p));
    if !inside {
        return None;
    }
    let iv: Vec<(f64, f64)> = boxes
        .iter()
        .filter_map(|b| {
            // Axis-aligned boxes with identity rotation and scale 1: the column interval.
            let (x, y) = (
                p[0] - b.transform.position[0],
                p[1] - b.transform.position[1],
            );
            (b.min[0] <= x && x <= b.max[0] && b.min[1] <= y && y <= b.max[1]).then(|| {
                let z = f64::from(b.transform.position[2]);
                (z + f64::from(b.min[2]), z + f64::from(b.max[2]))
            })
        })
        .collect();
    let mut top = f64::from(p[2]);
    loop {
        let next = iv
            .iter()
            .filter(|(lo, hi)| *lo <= top + 1e-3 && *hi > top)
            .map(|(_, hi)| *hi)
            .fold(top, f64::max);
        if next <= top {
            break;
        }
        top = next;
    }
    #[allow(clippy::cast_possible_truncation)]
    Some((top as f32).max(p[2]))
}

#[test]
fn surface_sweep_matches_the_fixed_point_and_is_not_quadratic() {
    // Random stacks, compared with the join-until-stable reference.
    let mut seed = 0x2545_F491_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f32::from(u16::try_from(seed >> 16).unwrap_or(0)) / 65_536.0
    };
    for _ in 0..200 {
        let n = 1 + (next() * 12.0) as usize;
        let boxes: Vec<_> = (0..n)
            .map(|_| staircase_box(next() * 20.0, 0.1 + next() * 4.0))
            .collect();
        let p = [5.0, 5.0, next() * 20.0];
        let got = column_water(p, None, boxes.iter().map(|b| (b, [0.0; 3])));
        let want = reference_top(p, &boxes);
        assert_eq!(got.map(|s| s.surface), want, "{p:?} {boxes:?}");
    }
    // A staircase of 60,000 boxes, each overlapping the next: the former walk needed one
    // pass over all boxes per step (minutes here); the sweep is O(n log n).
    let n = 60_000_u16;
    let boxes: Vec<_> = (0..n)
        .rev()
        .map(|i| staircase_box(0.5 * f32::from(i), 0.5005))
        .collect();
    let start = std::time::Instant::now();
    let s = column_water([5.0, 5.0, 0.1], None, boxes.iter().map(|b| (b, [0.0; 3]))).unwrap();
    let elapsed = start.elapsed();
    let want = 0.5 * f32::from(n - 1) + 0.5005;
    assert!((s.surface - want).abs() < 1e-2, "{} vs {want}", s.surface);
    assert!(elapsed.as_secs() < 10, "column walk took {elapsed:?}");
}

fn placed(object: u32, x: f32) -> crate::bundle::PlacedEntity {
    crate::bundle::PlacedEntity {
        index: 0,
        iid: None,
        kind: None,
        object: Some(Did(object)),
        placement: tessera_formats::landblock::Placement::new([x, 0.0, 0.0], Quaternion::IDENTITY)
            .unwrap(),
        scale: [1.0; 3],
        interior: false,
        content_layers: None,
        own_draw: crate::objects::DrawFlags::default(),
        visual: None,
        draw: false,
        figure: None,
    }
}

#[test]
fn liquid_boxes_per_landblock_are_capped_at_the_collision_set_limit() {
    use tessera_physics::geometry::MAX_LIQUID_BOXES;
    // 5,000 entities naming the same object with one liquid monitor.
    let entities: Vec<_> = (0..5_000_u16)
        .map(|i| placed(DEEP_ONLY, f32::from(i % 160)))
        .collect();
    let lib = VolumeLibrary::new(pids(), 16);
    let pv = placed_volumes(&lib, &world(), &schema(), &entities, &[1], 100).unwrap();
    assert_eq!(pv.boxes.len(), 100);
    assert_eq!((pv.entities, pv.left_out), (5_000, 4_900));
    let sys = WaterSystem::new(
        WaterTables::new([(1, Arc::new(table()))], []),
        VolumeLibrary::new(pids(), 16),
    );
    let k = key(1, 1);
    let bw = sys
        .block_water_from(
            &world(),
            &schema(),
            k,
            Some(&map(k, &[(7, 1), (4, 38)])),
            &heights(),
            &entities,
            &[1],
        )
        .unwrap();
    // The volume entity's 3 boxes plus 5,000 placed: capped at the set's limit.
    assert_eq!(bw.boxes.len(), MAX_LIQUID_BOXES);
    assert_eq!(bw.boxes_left_out as usize, 3 + 5_000 - MAX_LIQUID_BOXES);
    assert_eq!(bw.placed_volume_entities, 5_000);
    // So the collision set accepts every box.
    let mut builder = tessera_physics::geometry::CollisionSetBuilder::new(
        tessera_physics::geometry::CollisionLimits::default(),
    );
    bw.apply_to(&mut builder).unwrap();
    // The queries still answer.
    assert!(bw.water_at([5.0, 5.0, water_height(38) - 2.0]).is_some());
}

/// Finding (clean verification, `water.md` §7.4): the walker's column query and the
/// controller's water once snapped to the plane under different conditions (a level just
/// below the feet). Both now use `tessera-physics`' rule, so they agree there.
#[test]
fn column_water_agrees_with_the_controller_near_the_plane() {
    use tessera_physics::LiquidWater;
    use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder, WaterPlane};
    // A box from −1 to 0.0005 over feet at 0.
    let boxes = [staircase_box(-1.0, 1.0005)];
    for level in [-0.0004_f32, 0.0, 0.0003, 0.0012, -0.002, 0.5] {
        let plane = WaterPlane::new(level, 1).unwrap();
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.set_water_plane(Some(plane));
        b.add_liquid_box(boxes[0]).unwrap();
        let set = b.build();
        for p in [[5.0, 5.0, 0.0], [5.0, 5.0, -0.5], [5.0, 5.0, 0.0004]] {
            let world = column_water(p, Some(plane), boxes.iter().map(|b| (b, [0.0; 3])));
            assert_eq!(world, LiquidWater(&set).water_at(p), "{level} {p:?}");
            if let Some(w) = world {
                assert!(w.surface >= p[2]);
            }
        }
    }
    // The plane 0.4 mm below the feet, within 1 mm of the top: the feet's height.
    let plane = WaterPlane::new(-0.0004, 1).unwrap();
    let s = column_water(
        [5.0, 5.0, 0.0],
        Some(plane),
        boxes.iter().map(|b| (b, [0.0; 3])),
    )
    .unwrap();
    assert_eq!(s.surface, 0.0);
}

/// Regression (fuzz `world_water`, CI run 40049524233): a box flat in one box-frame axis
/// (0 to 0 in y) under a rotation that tilts that axis by 2e-30 from horizontal. The
/// column span treated the vertical line as parallel to the axis and found water, while
/// the box-frame point test put the point 4e-34 outside, so `water_at` reported water
/// where `is_under_water` did not. Containment now is the span test (`water.md` §7.4: in
/// water when a liquid box contains the point).
#[test]
fn water_at_and_under_water_agree_for_a_nearly_flat_tilted_box() {
    let template = VolumeTemplate {
        object: Did(0x4700_0001),
        boxes: vec![VolumeBox {
            min: [0.0, 0.0, 0.0],
            max: [1.0e-7, 0.0, 80.0],
            scale_with_entity: false,
            flags: LiquidFlags {
                liquid: true,
                ..LiquidFlags::default()
            },
            properties: Did(0x7800_0073),
            name: None,
        }],
        unreadable_sets: 0,
    };
    let rotation = Quaternion {
        w: 0.0,
        x: 0.0,
        y: 1.0e-30,
        z: 1.0,
    };
    let boxes = template.place([0.0; 3], rotation, [1.0; 3]);
    assert_eq!(boxes.len(), 1);
    let bw = BlockWater {
        key: key(10, 10),
        decision: Err(DryReason::NoTable),
        boxes,
        placed_volume_entities: 0,
        boxes_left_out: 0,
    };
    for z in [0.0_f32, 0.0002, 40.0, 80.0, 80.5, -0.5] {
        let p = [0.0, 0.0, z];
        let sample = bw.water_at(p);
        assert_eq!(sample.is_some(), bw.is_under_water(p), "{p:?}");
        assert_eq!(sample.is_some(), bw.liquid_at(p).liquid, "{p:?}");
    }
    assert!(bw.is_under_water([0.0, 0.0, 0.0002]));
    assert!(!bw.is_under_water([0.0, 0.0, 80.5]));
}
