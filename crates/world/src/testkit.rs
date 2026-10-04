// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic records and worlds for tests, tools and fuzz seeds.
//!
//! Everything here is Tessera's own encoding of the layouts in `docs/specs/` (property
//! dictionary and sets, DID maps, region and block-map descriptors, landblock records,
//! entity chains); no game data is involved. [`SyntheticWorld`] assembles a complete
//! [`MemoryWorldSource`] that [`crate::WorldContext::load`] accepts.

use std::collections::BTreeMap;

use tessera_formats::landblock::BlockMapEntry;
use tessera_formats::landblock::{GRID_SAMPLES, HeightGrid};
use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_formats::setup::{
    ENTITY_FLAGS_REQUIRED, EntityHeader, SETUP_SECTION_DID_LIST, SetupSection, SetupSectionEntry,
};
use tessera_formats::{
    BlockMap, Did, EntityDescriptor, LandblockData, PropertySet, PropertyType, Setup, TSize,
    VisualTemplate,
};

use crate::coords::LandblockKey;
use crate::source::MemoryWorldSource;

/// A property value to encode.
#[derive(Debug, Clone, PartialEq)]
pub enum TestValue {
    /// Type 21.
    Bool(bool),
    /// Type 15.
    Int(i32),
    /// Type 8.
    Enum(u32),
    /// Type 9.
    Float(f32),
    /// Type 6.
    Vec3([f32; 3]),
    /// Type 20.
    Did(u32),
    /// Type 16.
    Colour([u8; 4]),
    /// Type 11: fields (pid, value), written with echo in bucket order.
    Struct(Vec<(u32, TestValue)>),
    /// Type 12: elements (pid, value), written without echo in stored order.
    Array(Vec<(u32, TestValue)>),
}

impl TestValue {
    /// The property type code of the value.
    #[must_use]
    pub fn type_code(&self) -> u32 {
        match self {
            Self::Bool(_) => 21,
            Self::Int(_) => 15,
            Self::Enum(_) => 8,
            Self::Float(_) => 9,
            Self::Vec3(_) => 6,
            Self::Did(_) => 20,
            Self::Colour(_) => 16,
            Self::Struct(_) => 11,
            Self::Array(_) => 12,
        }
    }
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn f32le(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Smallest hint with 2 · B ≥ n (`property-system.md` §5.4 PS-H2) and its bucket count.
fn hint_for(n: usize) -> (u8, u32) {
    for (h, &b) in TSIZE_BUCKET_COUNTS.iter().enumerate() {
        if (b as usize) * 2 >= n {
            // h < 13.
            #[allow(clippy::cast_possible_truncation)]
            return (h as u8, b);
        }
    }
    (12, TSIZE_BUCKET_COUNTS[12])
}

fn tsize(out: &mut Vec<u8>, hint: u8, n: usize) {
    out.push(hint);
    write_vle(out, u32::try_from(n).unwrap_or(u32::MAX));
}

/// Writes a TSize header and returns the items in bucket order for it.
fn bucketed<T: Clone>(out: &mut Vec<u8>, items: &[T], key: impl Fn(&T) -> u32) -> Vec<T> {
    let (hint, b) = hint_for(items.len());
    tsize(out, hint, items.len());
    let mut v = items.to_vec();
    v.sort_by_key(|i| key(i) % b);
    v
}

fn pascal(out: &mut Vec<u8>, s: &str) {
    write_vle(out, u32::try_from(s.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(s.as_bytes());
}

/// Encodes a value (`property-system.md` §4).
pub fn write_value(out: &mut Vec<u8>, v: &TestValue) {
    match v {
        TestValue::Bool(b) => out.push(u8::from(*b)),
        TestValue::Int(i) => out.extend_from_slice(&i.to_le_bytes()),
        TestValue::Enum(k) | TestValue::Did(k) => u32le(out, *k),
        TestValue::Float(f) => f32le(out, *f),
        TestValue::Vec3(v) => v.iter().for_each(|c| f32le(out, *c)),
        TestValue::Colour(c) => out.extend_from_slice(c),
        TestValue::Struct(fields) => {
            for (pid, val) in bucketed(out, fields, |f| f.0) {
                u32le(out, pid);
                u32le(out, pid);
                write_value(out, &val);
            }
        }
        TestValue::Array(elems) => {
            u32le(out, u32::try_from(elems.len()).unwrap_or(u32::MAX));
            for (pid, val) in elems {
                u32le(out, *pid);
                write_value(out, val);
            }
        }
    }
}

/// An embedded property set: TSize, then (pid, pid echo, value) in bucket order
/// (`property-system.md` §5.1).
#[must_use]
pub fn property_set(entries: &[(u32, TestValue)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (pid, v) in bucketed(&mut out, entries, |e| e.0) {
        u32le(&mut out, pid);
        u32le(&mut out, pid);
        write_value(&mut out, &v);
    }
    out
}

/// A property-set record (0x78/0x79): own DID, then the set.
#[must_use]
pub fn property_set_record(did: u32, entries: &[(u32, TestValue)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&property_set(entries));
    out
}

/// A property dictionary 0x34000000 (`property-system.md` §2) defining `(pid, name, type
/// code)`.
#[must_use]
pub fn dictionary(defs: &[(u32, &str, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, 0x3400_0000);
    out.extend_from_slice(&[0; 8]);
    for (pid, name, _) in bucketed(&mut out, defs, |d| d.0) {
        u32le(&mut out, pid);
        pascal(&mut out, name);
    }
    out.extend_from_slice(&[0, 0]);
    for (pid, _, ty) in bucketed(&mut out, defs, |d| d.0) {
        u32le(&mut out, pid);
        u32le(&mut out, pid); // echo
        u32le(&mut out, ty);
        u32le(&mut out, 0); // field_a
        u32le(&mut out, 4); // field_b
        u32le(&mut out, 0); // data
        u32le(&mut out, 0); // field_c
        u32le(&mut out, 0); // flags: no default/min/max
        u32le(&mut out, 0x3FC0_0000); // marker
        out.extend_from_slice(&[2, 2, 4, 0, 1]);
        write_vle(&mut out, 0); // children
        u32le(&mut out, 0); // subset
        u32le(&mut out, 0); // terminator
    }
    out
}

/// A DID map (`records.md` §8): one group of `(key, value, label)` and an empty second
/// group.
#[must_use]
pub fn did_map(did: u32, entries: &[(u32, u32, &str)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    let sorted = bucketed(&mut out, entries, |e| e.0);
    for (k, v, _) in &sorted {
        u32le(&mut out, *k);
        u32le(&mut out, *v);
    }
    let (hint, _) = hint_for(entries.len());
    tsize(&mut out, hint, entries.len());
    for (k, _, label) in &sorted {
        u32le(&mut out, *k);
        out.push(u8::try_from(label.len()).unwrap_or(u8::MAX));
        out.extend_from_slice(&label.as_bytes()[..label.len().min(255)]);
    }
    // Second group, empty.
    tsize(&mut out, 1, 0);
    tsize(&mut out, 1, 0);
    out
}

/// A 114-byte region descriptor (`environment.md` §3.1) whose slot 7 names
/// `block_map_descriptor`.
#[must_use]
pub fn region_descriptor(
    did: u32,
    region: u32,
    origin_cells: [u32; 2],
    scale: f32,
    block_map_descriptor: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(114);
    u32le(&mut out, did);
    u32le(&mut out, region);
    f32le(&mut out, 8.0);
    u32le(&mut out, 2);
    u32le(&mut out, origin_cells[0]);
    u32le(&mut out, origin_cells[1]);
    u32le(&mut out, 0);
    u32le(&mut out, 0);
    f32le(&mut out, scale);
    let tables = [
        0x1100_0000,
        0x1200_0000,
        0x1400_0000,
        0x1500_0000,
        0x1600_0000,
        0x1700_0000,
        block_map_descriptor,
    ];
    for t in tables {
        out.extend_from_slice(&[1, 1]);
        u32le(&mut out, 0);
        u32le(&mut out, t);
    }
    out.extend_from_slice(&[1, 0, 1, 0]);
    u32le(&mut out, 0x1900_0000);
    out
}

/// A block-map descriptor (prefix 0x18, `landblock.md` §5.2): entries of
/// `(pid, key, palette)`.
#[must_use]
pub fn block_map_descriptor(did: u32, entries: &[(u32, u32, Vec<TestValue>)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    u32le(&mut out, 0);
    for (pid, key, palette) in bucketed(&mut out, entries, |e| e.1) {
        u32le(&mut out, pid);
        u32le(&mut out, pid);
        u32le(&mut out, key);
        out.push(1);
        u32le(&mut out, u32::try_from(palette.len()).unwrap_or(u32::MAX));
        for v in &palette {
            u32le(&mut out, pid);
            write_value(&mut out, v);
        }
    }
    out
}

/// A block map (`landblock.md` §5.3) with `(key, index)` pairs.
#[must_use]
pub fn block_map(key: LandblockKey, pairs: &[(u32, u16)]) -> Vec<u8> {
    let (hint, b) = hint_for(pairs.len());
    let mut entries: Vec<BlockMapEntry> = pairs
        .iter()
        .map(|&(key, index)| BlockMapEntry { key, index })
        .collect();
    entries.sort_by_key(|e| e.key % b);
    BlockMap {
        did: key.block_map_did(),
        size: TSize {
            hint,
            count: u32::try_from(pairs.len()).unwrap_or(u32::MAX),
        },
        entries,
    }
    .to_bytes()
}

/// A landblock-data record with heights `h(i, j)` (stored units, i along X) and the
/// header's info DID set when `has_info`.
#[must_use]
pub fn landblock_data(
    key: LandblockKey,
    has_info: bool,
    h: impl Fn(usize, usize) -> u16,
) -> Vec<u8> {
    let grid = HeightGrid::from_fn(h);
    let mut d = LandblockData::from_heights(key.data_did(), grid);
    if has_info {
        d.info = Some(key.info_did());
    }
    debug_assert_eq!(d.heights.raw().len(), GRID_SAMPLES);
    d.to_bytes()
}

/// A static entity to encode (`entity-descriptor.md` §3).
#[derive(Debug, Clone, PartialEq)]
pub struct TestEntity {
    /// Type code (7 = Physics).
    pub kind: u32,
    /// Serial within the landblock.
    pub serial: u32,
    /// The object DID.
    pub object: Option<u32>,
    /// Block-local position, or none.
    pub position: Option<[f32; 3]>,
    /// Rotation (w, x, y, z).
    pub rotation: [f32; 4],
    /// Property entries.
    pub properties: Vec<(u32, TestValue)>,
    /// Interior flag.
    pub interior: bool,
}

impl TestEntity {
    /// A Physics entity placing `object` at `position`, unrotated, with `properties`.
    #[must_use]
    pub fn physics(
        serial: u32,
        object: u32,
        position: [f32; 3],
        properties: Vec<(u32, TestValue)>,
    ) -> Self {
        Self {
            kind: 7,
            serial,
            object: Some(object),
            position: Some(position),
            rotation: [1.0, 0.0, 0.0, 0.0],
            properties,
            interior: false,
        }
    }
}

fn write_entity(out: &mut Vec<u8>, key: LandblockKey, e: &TestEntity) {
    let mut flags = 0x002 | 0x004 | 0x080;
    if e.object.is_some() {
        flags |= 0x008;
    }
    if e.position.is_some() {
        flags |= 0x010;
    }
    if e.interior {
        flags |= 0x400;
    }
    u32le(out, flags);
    u32le(out, e.kind);
    let iid = (1u64 << 56)
        | (u64::from(key.region()) << 52)
        | (u64::from(key.x()) << 44)
        | (u64::from(key.y()) << 36)
        | u64::from(e.serial);
    out.extend_from_slice(&iid.to_le_bytes());
    if let Some(o) = e.object {
        u32le(out, o);
    }
    if let Some(p) = e.position {
        p.iter().for_each(|c| f32le(out, *c));
        e.rotation.iter().for_each(|c| f32le(out, *c));
    }
    out.extend_from_slice(&property_set(&e.properties));
}

/// A landblock-info record (`landblock.md` §4.1) with no cells, links or weenies, the
/// given entities, and the trailer that directory stamp 39 implies.
#[must_use]
pub fn landblock_info(key: LandblockKey, entities: &[TestEntity]) -> Vec<u8> {
    let mut out = Vec::new();
    u32le(&mut out, key.info_did().0);
    u32le(&mut out, 1); // flags: cells
    u32le(&mut out, 0); // size hint
    tsize(&mut out, 0, 0); // cells
    u32le(&mut out, 0); // reserved
    u32le(&mut out, u32::try_from(entities.len()).unwrap_or(u32::MAX));
    for e in entities {
        write_entity(&mut out, key, e);
    }
    u32le(&mut out, 0); // links
    out.extend_from_slice(&property_set(&[])); // landblock properties
    u32le(&mut out, 0); // reserved
    tsize(&mut out, 0, 0); // weenies
    u32le(&mut out, 0); // trailer (stamp >= 31)
    out
}

/// A landblock-properties record (`landblock.md` §6.1).
#[must_use]
pub fn landblock_properties(key: LandblockKey, entries: &[(u32, TestValue)]) -> Vec<u8> {
    let mut out = key.properties_did().0.to_le_bytes().to_vec();
    out.extend_from_slice(&property_set(entries));
    out
}

/// An entity descriptor (0x47) linking to `template` with property entries.
///
/// # Panics
///
/// When `schema` does not define the PIDs used (test helper).
#[must_use]
pub fn entity_descriptor(
    did: u32,
    template: u32,
    entries: &[(u32, TestValue)],
    schema: &dyn tessera_formats::PropertySchema,
) -> Vec<u8> {
    let set_bytes = property_set(entries);
    let properties = PropertySet::parse(&set_bytes, schema).expect("testkit property set");
    EntityDescriptor {
        header: EntityHeader {
            flags: ENTITY_FLAGS_REQUIRED,
            did: Did(did),
            kind: 7,
            template: Did(template),
            block: None,
        },
        properties,
        extra_sets: Vec::new(),
    }
    .to_bytes_with(&set_bytes)
    .expect("testkit entity descriptor")
}

/// A visual template (0x1F) naming `setup`.
#[must_use]
pub fn visual_template(did: u32, setup: u32) -> Vec<u8> {
    VisualTemplate {
        flags: 0x201,
        did: Did(did),
        parent: None,
        vec3: None,
        value_a: None,
        value_b: None,
        appearance: None,
        setup_section: Some(SetupSection {
            size: TSize { hint: 0, count: 1 },
            entries: vec![SetupSectionEntry {
                key: 1,
                kind: SETUP_SECTION_DID_LIST,
                dids: vec![Did(setup)],
            }],
        }),
    }
    .to_bytes()
}

/// A setup (0x04) with part meshes `parts`.
///
/// # Panics
///
/// If the setup encoder refuses the value (test helper).
#[must_use]
pub fn setup(did: u32, parts: &[u32]) -> Vec<u8> {
    Setup {
        did: Did(did),
        parts: parts.iter().map(|&p| Did(p)).collect(),
        ..Setup::default()
    }
    .to_bytes()
    .expect("testkit setup")
}

/// PIDs of the synthetic dictionary (arbitrary values chosen for these tests).
pub mod pid {
    /// `UI_Map_GameMap` (struct).
    pub const UI_MAP_GAME_MAP: u32 = 0x1000_0101;
    /// `UI_Map_MapImage` (DID).
    pub const UI_MAP_MAP_IMAGE: u32 = 0x1000_0102;
    /// `UI_Map_BlockOffsetX` (int).
    pub const UI_MAP_BLOCK_OFFSET_X: u32 = 0x1000_0103;
    /// `UI_Map_BlockOffsetY` (int).
    pub const UI_MAP_BLOCK_OFFSET_Y: u32 = 0x1000_0104;
    /// `UI_Map_PixelOffsetX` (int).
    pub const UI_MAP_PIXEL_OFFSET_X: u32 = 0x1000_0105;
    /// `UI_Map_PixelOffsetY` (int).
    pub const UI_MAP_PIXEL_OFFSET_Y: u32 = 0x1000_0106;
    /// `UI_Map_Scale` (float).
    pub const UI_MAP_SCALE: u32 = 0x1000_0107;
    /// `Landblock_LightMap` (DID).
    pub const LANDBLOCK_LIGHT_MAP: u32 = 0x1000_0108;
    /// `Render_AdjustableScale` (vector).
    pub const RENDER_ADJUSTABLE_SCALE: u32 = 0x1000_0109;
    /// `Entity_ContentLayers` (array).
    pub const ENTITY_CONTENT_LAYERS: u32 = 0x1000_010A;
    /// `Entity_ContentLayer` (enum; array element).
    pub const ENTITY_CONTENT_LAYER: u32 = 0x1000_010B;
    /// `Render_Draw` (bool).
    pub const RENDER_DRAW: u32 = 0x1000_010C;
    /// `Render_DrawMeshes` (bool).
    pub const RENDER_DRAW_MESHES: u32 = 0x1000_010D;
    /// `Render_NeverAddToScene` (bool).
    pub const RENDER_NEVER_ADD_TO_SCENE: u32 = 0x1000_010E;
    /// `Landblock_UseCeiling` (bool).
    pub const LANDBLOCK_USE_CEILING: u32 = 0x1000_010F;
    /// `Area_DID` (DID).
    pub const AREA_DID: u32 = 0x1000_0110;
    /// `Environment_LocalSky` (DID).
    pub const ENVIRONMENT_LOCAL_SKY: u32 = 0x1000_0111;
    /// `Ambient_MusicType` (enum).
    pub const AMBIENT_MUSIC_TYPE: u32 = 0x1000_0112;
    /// `Ambient_MusicRegion` (enum).
    pub const AMBIENT_MUSIC_REGION: u32 = 0x1000_0113;
    /// `Ambient_SoundEnum_BlockMapOverride` (enum).
    pub const AMBIENT_SOUND_OVERRIDE: u32 = 0x1000_0114;
    /// `PhysObj` (DID).
    pub const PHYS_OBJ: u32 = 0x1000_0115;
    /// `Physics_AdjustableScale` (vector).
    pub const PHYSICS_ADJUSTABLE_SCALE: u32 = 0x1000_0116;
}

/// The synthetic dictionary's definitions: (pid, name, type code).
#[must_use]
pub fn definitions() -> Vec<(u32, &'static str, u32)> {
    use pid::*;
    vec![
        (UI_MAP_GAME_MAP, "UI_Map_GameMap", 11),
        (UI_MAP_MAP_IMAGE, "UI_Map_MapImage", 20),
        (UI_MAP_BLOCK_OFFSET_X, "UI_Map_BlockOffsetX", 15),
        (UI_MAP_BLOCK_OFFSET_Y, "UI_Map_BlockOffsetY", 15),
        (UI_MAP_PIXEL_OFFSET_X, "UI_Map_PixelOffsetX", 15),
        (UI_MAP_PIXEL_OFFSET_Y, "UI_Map_PixelOffsetY", 15),
        (UI_MAP_SCALE, "UI_Map_Scale", 9),
        (LANDBLOCK_LIGHT_MAP, "Landblock_LightMap", 20),
        (RENDER_ADJUSTABLE_SCALE, "Render_AdjustableScale", 6),
        (PHYSICS_ADJUSTABLE_SCALE, "Physics_AdjustableScale", 6),
        (ENTITY_CONTENT_LAYERS, "Entity_ContentLayers", 12),
        (ENTITY_CONTENT_LAYER, "Entity_ContentLayer", 8),
        (RENDER_DRAW, "Render_Draw", 21),
        (RENDER_DRAW_MESHES, "Render_DrawMeshes", 21),
        (RENDER_NEVER_ADD_TO_SCENE, "Render_NeverAddToScene", 21),
        (LANDBLOCK_USE_CEILING, "Landblock_UseCeiling", 21),
        (AREA_DID, "Area_DID", 20),
        (ENVIRONMENT_LOCAL_SKY, "Environment_LocalSky", 20),
        (AMBIENT_MUSIC_TYPE, "Ambient_MusicType", 8),
        (AMBIENT_MUSIC_REGION, "Ambient_MusicRegion", 8),
        (
            AMBIENT_SOUND_OVERRIDE,
            "Ambient_SoundEnum_BlockMapOverride",
            8,
        ),
        (PHYS_OBJ, "PhysObj", 20),
    ]
}

/// The synthetic schema as a map (for decoding without a dictionary record).
#[must_use]
pub fn schema() -> std::collections::HashMap<u32, PropertyType> {
    definitions()
        .into_iter()
        .map(|(p, _, t)| (p, PropertyType::from_code(t)))
        .collect()
}

/// Block-map keys of the synthetic descriptor (§5.5 numbering).
pub mod key {
    /// `Landblock_UseCeiling`.
    pub const USE_CEILING: u32 = 0x0000_0009;
    /// `Ambient_MusicType`.
    pub const MUSIC_TYPE: u32 = 0x1000_000C;
    /// `Area_DID`.
    pub const AREA: u32 = 0x1000_000D;
    /// `Environment_LocalSky`.
    pub const LOCAL_SKY: u32 = 0x1000_000F;
    /// A key without a descriptor entry (§5.6).
    pub const UNDESCRIBED: u32 = 0x0000_0004;
}

/// Object DIDs of the synthetic world.
pub mod obj {
    /// A drawable physics object: 0x47 → 0x1F → 0x04 with two parts.
    pub const TREE: u32 = 0x4700_0001;
    /// Its template.
    pub const TREE_TEMPLATE: u32 = 0x1F00_0001;
    /// Its setup.
    pub const TREE_SETUP: u32 = 0x0400_0001;
    /// Its parts.
    pub const TREE_PARTS: [u32; 2] = [0x0600_0001, 0x0600_0002];
    /// A physics object whose descriptor sets `Render_Draw` = false; it shares the tree's
    /// template.
    pub const HIDDEN: u32 = 0x4700_0002;
    /// A physics object whose chain is broken (missing template).
    pub const BROKEN: u32 = 0x4700_0003;
    /// A game object (weenie) whose property set names [`TREE`] as `PhysObj`.
    pub const WEENIE: u32 = 0x7000_0001;
    /// An area object.
    pub const AREA_A: u32 = 0x7000_0100;
    /// Another area object.
    pub const AREA_B: u32 = 0x7000_0101;
}

/// Builder of a synthetic install: dictionary, DID maps, one region descriptor and
/// block-map descriptor, an entity chain, and a rectangle of landblocks.
#[derive(Debug, Clone)]
pub struct SyntheticWorld {
    /// Region number.
    pub region: u8,
    /// Landblocks with data: (X, Y) → whether it has an info record with entities.
    pub blocks: BTreeMap<(u8, u8), bool>,
    /// Entities placed in every block that has info.
    pub entities_per_block: u32,
}

impl SyntheticWorld {
    /// A world of region `region` covering X in `xs` and Y in `ys`; every block has info.
    #[must_use]
    pub fn rectangle(
        region: u8,
        xs: std::ops::RangeInclusive<u8>,
        ys: std::ops::RangeInclusive<u8>,
    ) -> Self {
        let mut blocks = BTreeMap::new();
        for y in ys {
            for x in xs.clone() {
                blocks.insert((x, y), true);
            }
        }
        Self {
            region,
            blocks,
            entities_per_block: 4,
        }
    }

    /// The area object of block (X, Y): [`obj::AREA_A`] west of X = 128, else
    /// [`obj::AREA_B`].
    #[must_use]
    pub fn area_of(x: u8) -> u32 {
        if x < 128 { obj::AREA_A } else { obj::AREA_B }
    }

    /// Height (stored units) of grid sample (i, j) of block (X, Y): a smooth slope in X,
    /// continuous across block edges.
    #[must_use]
    pub fn height(x: u8, i: usize, _y: u8, _j: usize) -> u16 {
        let g = u32::from(x) * 32 + u32::try_from(i).unwrap_or(0);
        u16::try_from((g * 4).min(65_535)).unwrap_or(u16::MAX)
    }

    /// Builds the records.
    ///
    /// # Panics
    ///
    /// If a testkit encoder fails (test helper).
    #[must_use]
    pub fn build(&self) -> MemoryWorldSource {
        use pid::*;
        let schema = schema();
        let mut m = MemoryWorldSource::new();
        m.insert(0x3400_0000u32, dictionary(&definitions()), Some(1));
        m.insert(
            0x2800_0000u32,
            did_map(
                0x2800_0000,
                &[(0x11, 0x2800_0008, "REGION"), (0x1, 0x2800_0001, "EMAPPER")],
            ),
            None,
        );
        m.insert(
            0x2800_0008u32,
            did_map(
                0x2800_0008,
                &[
                    (u32::from(self.region), 0x0F00_0000, "Synthetic"),
                    (99, 0x0F00_0063, "Absent"),
                ],
            ),
            None,
        );
        m.insert(
            0x0F00_0000u32,
            region_descriptor(
                0x0F00_0000,
                u32::from(self.region),
                [1468, 1244],
                0.005,
                0x1800_0000,
            ),
            None,
        );
        m.insert(
            0x1800_0000u32,
            block_map_descriptor(
                0x1800_0000,
                &[
                    (
                        AREA_DID,
                        key::AREA,
                        vec![TestValue::Did(obj::AREA_A), TestValue::Did(obj::AREA_B)],
                    ),
                    (
                        AMBIENT_MUSIC_TYPE,
                        key::MUSIC_TYPE,
                        vec![TestValue::Enum(5), TestValue::Enum(6)],
                    ),
                    (
                        ENVIRONMENT_LOCAL_SKY,
                        key::LOCAL_SKY,
                        vec![TestValue::Did(0x6A00_0001)],
                    ),
                    (
                        LANDBLOCK_USE_CEILING,
                        key::USE_CEILING,
                        vec![TestValue::Bool(false), TestValue::Bool(true)],
                    ),
                ],
            ),
            None,
        );
        // Entity chain.
        m.insert(
            obj::TREE,
            entity_descriptor(obj::TREE, obj::TREE_TEMPLATE, &[], &schema),
            None,
        );
        m.insert(
            obj::HIDDEN,
            entity_descriptor(
                obj::HIDDEN,
                obj::TREE_TEMPLATE,
                &[(RENDER_DRAW, TestValue::Bool(false))],
                &schema,
            ),
            None,
        );
        m.insert(
            obj::BROKEN,
            entity_descriptor(obj::BROKEN, 0x1F00_00FF, &[], &schema),
            None,
        );
        m.insert(
            obj::TREE_TEMPLATE,
            visual_template(obj::TREE_TEMPLATE, obj::TREE_SETUP),
            None,
        );
        m.insert(
            obj::TREE_SETUP,
            setup(obj::TREE_SETUP, &obj::TREE_PARTS),
            None,
        );
        m.insert(
            obj::WEENIE + 0x0900_0000,
            property_set_record(
                obj::WEENIE + 0x0900_0000,
                &[(PHYS_OBJ, TestValue::Did(obj::TREE))],
            ),
            None,
        );

        for (&(x, y), &has_info) in &self.blocks {
            let Ok(key) = LandblockKey::new(self.region, x, y) else {
                continue;
            };
            m.insert(
                key.data_did(),
                landblock_data(key, has_info, |i, j| Self::height(x, i, y, j)),
                Some(39),
            );
            let area_index = u16::from(Self::area_of(x) == obj::AREA_B);
            m.insert(
                key.block_map_did(),
                block_map(
                    key,
                    &[
                        (key::AREA, area_index),
                        (key::MUSIC_TYPE, u16::from(y % 2 == 1)),
                        (key::LOCAL_SKY, 0),
                        (key::USE_CEILING, 0),
                        (key::UNDESCRIBED, 7),
                    ],
                ),
                Some(39),
            );
            m.insert(
                key.properties_did(),
                landblock_properties(
                    key,
                    &[(
                        UI_MAP_GAME_MAP,
                        TestValue::Struct(vec![
                            (UI_MAP_MAP_IMAGE, TestValue::Did(0x4100_0000 | key.packed())),
                            (UI_MAP_BLOCK_OFFSET_X, TestValue::Int(i32::from(x))),
                            (UI_MAP_BLOCK_OFFSET_Y, TestValue::Int(i32::from(y))),
                            (UI_MAP_PIXEL_OFFSET_X, TestValue::Int(0)),
                            (UI_MAP_PIXEL_OFFSET_Y, TestValue::Int(200)),
                            (UI_MAP_SCALE, TestValue::Float(1.25)),
                        ]),
                    )],
                ),
                Some(39),
            );
            if has_info {
                let mut entities = Vec::new();
                for s in 0..self.entities_per_block {
                    let object = match s % 4 {
                        0 | 1 => obj::TREE,
                        2 => obj::HIDDEN,
                        _ => obj::BROKEN,
                    };
                    // s ≤ entities_per_block; positions stay inside the block.
                    #[allow(clippy::cast_precision_loss)]
                    let p = (s % 32) as f32 * 5.0;
                    entities.push(TestEntity::physics(
                        s + 1,
                        object,
                        [p, p, 100.0],
                        vec![(RENDER_ADJUSTABLE_SCALE, TestValue::Vec3([2.0; 3]))],
                    ));
                }
                // One weenie placement.
                entities.push(TestEntity {
                    kind: 6,
                    ..TestEntity::physics(
                        self.entities_per_block + 1,
                        obj::WEENIE,
                        [80.0, 80.0, 1.0],
                        Vec::new(),
                    )
                });
                m.insert(key.info_did(), landblock_info(key, &entities), Some(39));
            }
        }
        m
    }
}
