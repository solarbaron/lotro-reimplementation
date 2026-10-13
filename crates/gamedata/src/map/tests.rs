// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the map models: the worked examples of `map-windows.md` §10, the
//! census categories of §5.3, the note tests of `map-data.md` §4.8 and the place-lookup
//! rules of §7.3. All DIDs, IDs, names and values are invented.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use tessera_formats::gameplay::map::{MapNote, MapNotesTable, NoteBase, NoteLink};
use tessera_formats::property::{Position, PropertyArray, PropertyEntry, PropertyStruct};
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};

use super::*;

// ---------------------------------------------------------------------------------------------
// Property values
// ---------------------------------------------------------------------------------------------

/// Invented PIDs for every label of [`MapPids`]: 1 + its index.
fn pids() -> MapPids {
    let names: HashMap<String, u32> = MapPids::LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| ((*l).to_owned(), u32::try_from(i + 1).unwrap()))
        .collect();
    MapPids::resolve(&names)
}

fn e(pid: tessera_formats::gameplay::common::Pid, value: PropertyValue) -> PropertyEntry {
    PropertyEntry {
        pid: pid.id().unwrap(),
        value,
    }
}

fn st(fields: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(PropertyStruct {
        fields,
        ..PropertyStruct::default()
    })
}

fn arr(values: Vec<PropertyValue>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: u32::try_from(values.len()).unwrap(),
        elements: values
            .into_iter()
            .map(|value| PropertyEntry { pid: 0, value })
            .collect(),
    })
}

fn lit(s: &str) -> StringInfo {
    crate::quest::tests::literal(s)
}

fn si(s: &str) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(lit(s)))
}

/// A node struct: element, name, image, block, pixel offsets, scale, frame 1,024 × 768.
#[allow(clippy::too_many_arguments)]
fn node(
    p: &MapPids,
    element: u32,
    name: &str,
    image: u32,
    block: (i32, i32),
    offsets: (i32, i32),
    scale: f32,
    region: Option<u32>,
    areas: &[u32],
    children: Vec<PropertyValue>,
) -> PropertyValue {
    let mut f = vec![
        e(p.active_element, PropertyValue::Enum(element)),
        e(p.menu_name, si(name)),
        e(p.image, PropertyValue::DidRef(Did(image))),
        e(p.block_x, PropertyValue::Int(block.0)),
        e(p.block_y, PropertyValue::Int(block.1)),
        e(p.pixel_x, PropertyValue::Int(offsets.0)),
        e(p.pixel_y, PropertyValue::Int(offsets.1)),
        e(p.scale, PropertyValue::Float(scale)),
        e(p.pixel_width, PropertyValue::Int(1024)),
        e(p.pixel_height, PropertyValue::Int(768)),
    ];
    if let Some(r) = region {
        f.push(e(p.region, PropertyValue::Enum(r)));
    }
    if !areas.is_empty() {
        f.push(e(
            p.area_dids,
            arr(areas
                .iter()
                .map(|a| PropertyValue::DidRef(Did(*a)))
                .collect()),
        ));
    }
    if !children.is_empty() {
        f.push(e(p.children, arr(children)));
    }
    st(f)
}

// ---------------------------------------------------------------------------------------------
// §10 worked examples
// ---------------------------------------------------------------------------------------------

/// `map-windows.md` §10.1: node *N* at block (100, 100), offsets (0, 768), scale 0.5.
fn node_n() -> Georef {
    Georef {
        block_x: 100,
        block_y: 100,
        pixel_x: 0,
        pixel_y: 768,
        scale: 0.5,
    }
}

#[test]
fn example_10_1_note_pixel() {
    let g = node_n();
    assert_eq!(g.to_pixel(16_100.0, 16_050.0), (50, 743));
    // The inverse of §3.4 gives the point back at the pixel.
    let (gx, gy) = g.to_world(50.0, 743.0).unwrap();
    assert!((gx - 16_100.0).abs() < 1e-9 && (gy - 16_050.0).abs() < 1e-9);
}

#[test]
fn example_10_2_other_region_is_outside() {
    // §6.1: the block difference is replaced by −2,000; the offset inside the block adds.
    let dx = -2_000.0f32 + 100.0;
    #[allow(clippy::cast_possible_truncation)]
    let px = (dx * 0.5) as i32;
    assert_eq!(px, -950);
}

#[test]
fn example_10_3_dungeon_stretch() {
    let g = Georef {
        block_x: 10,
        block_y: 20,
        pixel_x: 0,
        pixel_y: 2_400,
        scale: 2.0,
    };
    assert_eq!(g.to_pixel(1_700.0, 3_300.0), (200, 2_200));
    // Stretched into 800 × 600 at (112, 84): factors 0.5 and 0.25.
    assert_eq!((112 + 200 / 2, 84 + 2_200 / 4), (212, 634));
}

#[test]
fn truncation_toward_zero() {
    let g = Georef {
        block_x: 0,
        block_y: 0,
        pixel_x: 10,
        pixel_y: 10,
        scale: 0.5,
    };
    // A small negative product truncates to 0 (game-panels.md §8.6.1).
    assert_eq!(g.to_pixel(-1.0, -1.0), (10, 10));
    assert_eq!(g.to_pixel(f32::NAN, 4.0), (10, 8));
}

// ---------------------------------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------------------------------

#[test]
fn tree_order_areas_and_skips() {
    let p = pids();
    let leaf_a = node(
        &p,
        0x103,
        "A",
        0x4100_0003,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[0x7000_0001, 0x7000_0002],
        vec![],
    );
    let leaf_b = node(
        &p,
        0x104,
        "B",
        0x4100_0004,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[0x7000_0002],
        vec![],
    );
    let child = node(
        &p,
        0x102,
        "C",
        0x4100_0002,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[],
        vec![leaf_a, leaf_b],
    );
    // A node without a menu name is not added.
    let nameless = st(vec![e(p.active_element, PropertyValue::Enum(0x105))]);
    let other = node(
        &p,
        0x106,
        "D",
        0x4100_0006,
        (1, 1),
        (0, 0),
        1.0,
        Some(2),
        &[],
        vec![],
    );
    let root = node(
        &p,
        0x101,
        "World",
        0x4100_0001,
        (0, 0),
        (0, 0),
        0.02,
        None,
        &[],
        vec![child, nameless, other],
    );
    let t = MapTree::from_value(&root, &p).unwrap();
    let elements: Vec<u32> = t.nodes.iter().map(|n| n.active_element).collect();
    assert_eq!(
        elements,
        vec![0x101, 0x102, 0x103, 0x104, 0x106],
        "pre-order"
    );
    assert_eq!(t.nodes[2].parent, Some(1));
    assert_eq!(t.nodes[1].children, vec![2, 3]);
    // First registration in depth-first order wins; the conflict is recorded.
    assert_eq!(t.node_for_area(Did(0x7000_0002)), Some(2));
    assert_eq!(t.area_conflicts, vec![(Did(0x7000_0002), 2, 3)]);
    // Unlisted areas show the root (map-windows.md §4.6).
    assert_eq!(t.node_for_position(Some(Did(0x7000_0099))), Some(0));
    assert_eq!(t.node_for_position(Some(Did(0x7000_0001))), Some(2));
    assert!(t.is_within(2, 1) && t.is_within(2, 0) && !t.is_within(4, 1));
    assert_eq!(t.node_by_element(0x106), Some(4));
}

#[test]
fn repeated_active_element_is_reported() {
    let p = pids();
    let first = node(
        &p,
        0x700,
        "A",
        0x4100_0002,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[],
        vec![],
    );
    let second = node(
        &p,
        0x700,
        "B",
        0x4100_0003,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[],
        vec![],
    );
    let root = node(
        &p,
        0x6FF,
        "World",
        0x4100_0001,
        (0, 0),
        (0, 0),
        1.0,
        None,
        &[],
        vec![first, second],
    );
    let tree = MapTree::from_value(&root, &p).unwrap();
    assert_eq!(tree.nodes.len(), 3, "both nodes are kept");
    assert_eq!(
        tree.node_by_element(0x700),
        Some(1),
        "the first keeps the element"
    );
    assert_eq!(tree.element_conflicts, vec![(0x700, 1, 2)]);
}

#[test]
fn tree_rejects_bad_roots_and_deep_trees() {
    let p = pids();
    assert!(matches!(
        MapTree::from_value(&PropertyValue::Int(3), &p),
        Err(MapError::Malformed(_))
    ));
    let mut v = node(&p, 1, "x", 1, (0, 0), (0, 0), 1.0, None, &[], vec![]);
    for i in 0..=MAX_DEPTH + 1 {
        v = node(
            &p,
            2 + u32::try_from(i).unwrap(),
            "x",
            1,
            (0, 0),
            (0, 0),
            1.0,
            None,
            &[],
            vec![v],
        );
    }
    assert!(matches!(
        MapTree::from_value(&v, &p),
        Err(MapError::TooLarge { .. })
    ));
}

#[test]
fn display_mask_includes_common_mask() {
    let p = pids();
    let mut v = node(&p, 1, "x", 1, (0, 0), (0, 0), 1.0, Some(1), &[], vec![]);
    if let PropertyValue::Struct(s) = &mut v {
        s.fields
            .push(e(p.display_type, PropertyValue::Bitfield64(1 << 42)));
        s.fields
            .push(e(p.include_common, PropertyValue::Bool(true)));
    }
    let t = MapTree::from_value(&v, &p).unwrap();
    assert_eq!(t.nodes[0].display_mask(), (1 << 42) | COMMON_NOTE_MASK);
}

// ---------------------------------------------------------------------------------------------
// Regions and the census (§5)
// ---------------------------------------------------------------------------------------------

#[test]
fn frame_classes() {
    // Inside: block (100, 100), 1,024 × 768 at scale 0.5 covers 2,048 × 1,536 units.
    let (c, centre) = classify_frame(&node_n(), 1_024, 768);
    assert_eq!(c, RegionClass::Inside);
    let (x, y) = centre.unwrap();
    assert!((x - 17_024.0).abs() < 1e-9 && (y - 16_768.0).abs() < 1e-9);
    // Partly outside: the frame starts left of the grid.
    let g = Georef {
        block_x: 0,
        block_y: 10,
        pixel_x: 100,
        pixel_y: 768,
        scale: 1.0,
    };
    let (c, centre) = classify_frame(&g, 1_024, 768);
    assert_eq!(c, RegionClass::FramePartlyOutside);
    let (x, _) = centre.unwrap();
    assert!((x - 462.0).abs() < 1e-9, "centre of 0 … 924");
    // Outside: wholly below the grid.
    let g = Georef {
        block_x: 0,
        block_y: -600,
        pixel_x: 0,
        pixel_y: 768,
        scale: 0.02,
    };
    assert_eq!(
        classify_frame(&g, 1_024, 768),
        (RegionClass::FrameOutside, None)
    );
    // Zero scale has no inverse.
    let g = Georef {
        scale: 0.0,
        ..node_n()
    };
    assert_eq!(classify_frame(&g, 1_024, 768).0, RegionClass::FrameOutside);
    assert!(!g.is_usable());
    assert!(marker_in_frame(&node_n(), 1_024, 768, 17_024.0, 16_768.0));
    assert!(!marker_in_frame(&node_n(), 1_024, 768, 30_000.0, 16_768.0));
}

/// An in-memory [`MapData`].
#[derive(Default)]
struct MemMap {
    pids: Option<MapPids>,
    layout: Option<MapLayout>,
    sets: HashMap<Did, Arc<PropertySet>>,
    images: HashMap<Did, ImageInfo>,
    dungeons: Vec<DungeonRef>,
    areas: Vec<AreaRef>,
}

impl MapData for MemMap {
    fn pids(&self) -> &MapPids {
        self.pids.as_ref().unwrap()
    }
    fn map_layout(&self) -> Result<MapLayout, String> {
        self.layout.clone().ok_or_else(|| "no layout".to_owned())
    }
    fn map_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        Ok(self.sets.get(&did).cloned())
    }
    fn image(&self, did: Did) -> ImageInfo {
        self.images.get(&did).cloned().unwrap_or(ImageInfo::Missing)
    }
    fn dungeons(&self) -> Result<Vec<DungeonRef>, String> {
        Ok(self.dungeons.clone())
    }
    fn areas(&self) -> Result<Vec<AreaRef>, String> {
        Ok(self.areas.clone())
    }
}

fn game_map_set(
    p: &MapPids,
    image: u32,
    block: (i32, i32),
    offsets: (i32, i32),
    scale: f32,
) -> Arc<PropertySet> {
    let gm = st(vec![
        e(p.image, PropertyValue::DidRef(Did(image))),
        e(p.block_x, PropertyValue::Int(block.0)),
        e(p.block_y, PropertyValue::Int(block.1)),
        e(p.pixel_x, PropertyValue::Int(offsets.0)),
        e(p.pixel_y, PropertyValue::Int(offsets.1)),
        e(p.scale, PropertyValue::Float(scale)),
    ]);
    Arc::new(PropertySet {
        entries: vec![e(p.game_map, gm)],
        ..PropertySet::default()
    })
}

fn dungeon(
    did: u32,
    name: &str,
    map: Option<u32>,
    minimap: Option<u32>,
    region: Option<u8>,
) -> DungeonRef {
    DungeonRef {
        did: Did(did),
        name: Some(lit(name)),
        map_data: map.map(Did),
        minimap: minimap.map(Did),
        position: region.map(|r| {
            Position::new(0)
                .with_region(r)
                .with_landblock(10, 10)
                .with_cell(0)
                .with_offset([1.0, 1.0, 0.0])
        }),
    }
}

fn census_fixture() -> MemMap {
    let p = pids();
    let ok = |el, img| {
        node(
            &p,
            el,
            "n",
            img,
            (100, 100),
            (0, 768),
            0.5,
            Some(1),
            &[],
            vec![],
        )
    };
    let children = vec![
        ok(0x202, 0x4100_0002), // inside
        ok(0x2FF, 0x4100_0002), // C1: no element
        ok(0x203, 0x4100_00EE), // C2: image missing
        ok(0x204, 0x4100_00BB), // C3: bad image
        node(
            &p,
            0x205,
            "n",
            0x4100_0002,
            (100, 100),
            (0, 768),
            0.0001,
            Some(1),
            &[],
            vec![],
        ), // C4
        node(
            &p,
            0x206,
            "n",
            0x4100_0002,
            (0, 10),
            (100, 768),
            1.0,
            Some(2),
            &[],
            vec![],
        ), // C6
    ];
    let root = node(
        &p,
        0x201,
        "World",
        0x4100_0001,
        (0, -600),
        (0, 768),
        0.02,
        None,
        &[],
        children,
    );
    let mut m = MemMap {
        layout: Some(MapLayout {
            root: Some(root),
            top_level: [0x201, 0x202, 0x203, 0x204, 0x205, 0x206]
                .into_iter()
                .collect(),
        }),
        ..MemMap::default()
    };
    m.images
        .insert(Did(0x4100_0001), ImageInfo::Size(1_024, 768));
    m.images
        .insert(Did(0x4100_0002), ImageInfo::Size(1_024, 768));
    m.images
        .insert(Did(0x4100_00BB), ImageInfo::Bad("bad".into()));
    m.images
        .insert(Did(0x4100_0010), ImageInfo::Size(1_600, 2_400));
    m.sets.insert(
        Did(0x7800_0010),
        game_map_set(&p, 0x4100_0010, (10, 20), (0, 2_400), 2.0),
    );
    m.sets.insert(
        Did(0x7800_0011),
        game_map_set(&p, 0x4100_00EE, (10, 20), (0, 2_400), 2.0),
    );
    m.dungeons = vec![
        dungeon(
            0x7000_0302,
            "Second",
            Some(0x7800_0010),
            Some(0x7800_0013),
            Some(4),
        ),
        dungeon(0x7000_0301, "First", Some(0x7800_0010), None, None),
        dungeon(0x7000_0303, "Lost", Some(0x7800_0011), None, None),
        dungeon(0x7000_0304, "Radar", None, Some(0x7800_0012), None),
    ];
    m.areas = vec![AreaRef {
        did: Did(0x7000_0401),
        level: GeoLevel::Area,
        name: Some(lit("Vale")),
        parent: None,
        region_id: None,
        map_data: Some(Did(0x7800_0014)),
    }];
    m.pids = Some(p);
    m
}

#[test]
fn census_categories_and_elements() {
    let m = census_fixture();
    let r = MapRegions::load(&m);
    assert!(r.layout_error.is_none());
    let classes: Vec<RegionClass> = r.regions.iter().map(|x| x.class).collect();
    assert_eq!(
        classes,
        vec![
            RegionClass::FrameOutside, // the root (C5)
            RegionClass::Inside,
            RegionClass::NoElement,
            RegionClass::NoImage,
            RegionClass::BadImage,
            RegionClass::NoGeoreference,
            RegionClass::FramePartlyOutside,
            RegionClass::Inside,  // dungeon map 0x78000010
            RegionClass::NoImage, // dungeon map 0x78000011 (image missing)
        ]
    );
    let c = r.census();
    assert_eq!((c.nodes, c.dungeons, c.total()), (7, 2, 9));
    assert_eq!(c.drawn, 4, "C5, C6 and the two Inside maps are drawn");
    assert_eq!(c.marked, 3);
    assert!(c.failures.is_empty());
    assert_eq!(c.count(RegionClass::FrameOutside), 1);
    assert_eq!(
        r.excluded,
        [
            (Did(0x7800_0012), ExcludedSet::MiniMapOnly),
            (Did(0x7800_0013), ExcludedSet::MiniMapOnly),
            (Did(0x7800_0014), ExcludedSet::AreaMapData),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>()
    );
    // The dungeon map takes the lowest-DID dungeon's name and the first known region.
    let d = &r.regions[7];
    assert_eq!(d.dungeons, vec![Did(0x7000_0301), Did(0x7000_0302)]);
    assert_eq!(d.name, Some(lit("First")));
    assert_eq!(d.location.unwrap().region, 4);
    assert_eq!(d.frame, Some((1_600, 2_400)));
    // Selector elements (scenario-feed.md §8.3 map-region).
    let n = r.regions[1].candidate();
    assert_eq!((n.did, n.row), (Did(0x202), 2));
    assert_eq!(n.location.unwrap().region, 1);
    let dc = d.candidate();
    assert_eq!((dc.did, dc.row), (Did(0x7800_0010), 0));
    assert!(r.find(Did(0x202), 2).is_some());
    assert!(r.find(Did(0x202), 3).is_none());
    assert!(r.find(Did(0x7800_0010), 0).is_some());
    // C1–C5 regions have no location.
    assert!(r.regions[0].location.is_none() && r.regions[2].location.is_none());
    assert_eq!(r.regions[1].kind.keyword(), "node");
    assert_eq!(d.kind.keyword(), "dungeon");
}

#[test]
fn census_without_layout_lists_dungeon_maps() {
    let mut m = census_fixture();
    m.layout = None;
    let r = MapRegions::load(&m);
    assert_eq!(r.layout_error.as_deref(), Some("no layout"));
    assert_eq!(r.census().nodes, 0);
    assert_eq!(r.census().dungeons, 2);
}

// ---------------------------------------------------------------------------------------------
// Notes (map-data.md §4.8, map-windows.md §6.3)
// ---------------------------------------------------------------------------------------------

fn layers(keys: &[u32]) -> Option<PropertyEntry> {
    Some(PropertyEntry {
        pid: 0x4CE,
        value: arr(keys.iter().map(|k| PropertyValue::Enum(*k)).collect()),
    })
}

fn note(type_bits: u64, region: u8, block: (u8, u8), offset: [f32; 2], area: u32) -> MapNote {
    MapNote {
        base: NoteBase {
            position: Position::new(0)
                .with_region(region)
                .with_landblock(block.0, block.1)
                .with_cell(0)
                .with_offset([offset[0], offset[1], 0.0]),
            area: Did(area),
            dungeon: Did(0),
            entity: Did(0x7000_0900),
            layers: layers(&[]),
        },
        text: lit("Note"),
        icon: Did(0),
        level: 1,
        note_type: type_bits,
        flag_b: false,
        link: None,
        min_range: 0.0,
        max_range: 0.0,
        discovery: 0,
        extra: PropertySet::default(),
    }
}

#[test]
fn content_layer_and_discovery_tests() {
    let s = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: u64::MAX,
    };
    let mut n = note(1 << 42, 1, (100, 100), [100.0, 50.0], 0);
    assert!(passes_tests(&n, &s), "an empty list passes");
    n.base.layers = layers(&[LAYER_INSTANCE_ZERO]);
    assert!(passes_tests(&n, &s), "InstanceZero matches layer 0");
    assert!(!passes_tests(&n, &NoteSettings { layer: 5, ..s }));
    n.base.layers = layers(&[0x1000_0001, LAYER_ALL]);
    assert!(
        passes_tests(&n, &NoteSettings { layer: 5, ..s }),
        "All matches"
    );
    n.base.layers = None;
    assert!(!passes_tests(&n, &s), "a null list fails");
    let mut d = note(1 << 42, 1, (100, 100), [100.0, 50.0], 0);
    d.discovery = 7;
    assert!(passes_tests(&d, &s));
    assert!(!passes_tests(
        &d,
        &NoteSettings {
            discovery_all: false,
            ..s
        }
    ));
}

#[test]
fn example_10_1_notes_selected_on_node() {
    let p = pids();
    let mut n = node(
        &p,
        0x501,
        "N",
        0x4100_0001,
        (100, 100),
        (0, 768),
        0.5,
        Some(1),
        &[0x7000_0A01],
        vec![],
    );
    if let PropertyValue::Struct(s) = &mut n {
        s.fields
            .push(e(p.display_type, PropertyValue::Bitfield64(1 << 42)));
    }
    let root = node(
        &p,
        0x500,
        "World",
        0x4100_0000,
        (0, 0),
        (0, 0),
        0.02,
        None,
        &[],
        vec![n],
    );
    let tree = MapTree::from_value(&root, &p).unwrap();
    let mut world = note(1 << 47, 1, (100, 100), [50.0, 50.0], 0x7000_0A01);
    world.extra = PropertySet {
        entries: vec![e(p.note_states, arr(vec![]))],
        ..PropertySet::default()
    };
    let table = MapNotesTable {
        did: Did(0x0E00_0006),
        notes: vec![
            note(1 << 42, 1, (100, 100), [100.0, 50.0], 0x7000_0A01), // A
            note(1 << 37, 1, (101, 100), [40.0, 100.0], 0x7000_0A01), // B: not in the mask
            note(1 << 42, 2, (100, 100), [100.0, 50.0], 0x7000_0A01), // another region
            note(1 << 42, 1, (100, 100), [10.0, 10.0], 0x7000_0B02),  // area of no node → root
            world,
        ],
    };
    let index = table.index();
    let settings = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: 1 << 42,
    };
    let render = |i: &StringInfo| (i == &lit("Note")).then(|| "Note".to_owned());
    let mut mask_all = tree.clone();
    let sel = select_notes(
        &table,
        &index,
        &tree,
        ShownMap::Node(1),
        (1_024, 768),
        &settings,
        &p,
        &render,
    );
    assert_eq!(sel.len(), 1, "only A is created");
    assert_eq!(sel[0].row, 0);
    assert_eq!(sel[0].pixel, Some((50, 743)));
    assert!(sel[0].visible);
    assert_eq!(sel[0].text.as_deref(), Some("Note"));
    // With bit 47 in the mask, the world-event row is created but hidden (no world
    // properties) and not visible under the filter.
    mask_all.nodes[1].display_type = Some((1 << 42) | (1 << 47));
    let sel = select_notes(
        &table,
        &index,
        &mask_all,
        ShownMap::Node(1),
        (1_024, 768),
        &settings,
        &p,
        &render,
    );
    let w = sel.iter().find(|n| n.row == 4).unwrap();
    assert!(w.world_event_hidden && !w.visible);
    // The filter read mask clears bits outside 0x9B7C7FFFFEFF0002.
    assert_eq!(read_filter(u64::MAX), FILTER_READ_MASK);
}

#[test]
fn dungeon_zone_query_and_links() {
    let p = pids();
    let tree = MapTree::default();
    let zone = Did(0x7000_0C01);
    let other = Did(0x7000_0C02);
    let mut inside = note(1 << 20, 1, (5, 5), [0.0, 0.0], 0x7000_0C01);
    inside.base.dungeon = zone;
    inside.base.position = inside.base.position.with_cell(0x100);
    let mut link = note(1, 1, (5, 5), [1.0, 1.0], 0x7000_0C01);
    link.base.dungeon = zone;
    link.base.position = link.base.position.with_cell(0x100);
    link.link = Some(NoteLink {
        position: Position::new(0)
            .with_region(1)
            .with_landblock(6, 6)
            .with_cell(0x101)
            .with_offset([0.0, 0.0, 0.0]),
        target: other,
    });
    let mut beyond = note(1 << 21, 1, (6, 6), [0.0, 0.0], 0x7000_0C02);
    beyond.base.dungeon = other;
    let table = MapNotesTable {
        did: Did(1),
        notes: vec![inside, link, beyond],
    };
    let index = table.index();
    let settings = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: 0b10 | (1 << 20),
    };
    let sel = select_notes(
        &table,
        &index,
        &tree,
        ShownMap::Dungeon {
            zone,
            georef: None,
            frame: (800, 600),
        },
        (800, 600),
        &settings,
        &p,
        &|_| None,
    );
    let rows: Vec<usize> = sel.iter().map(|n| n.row).collect();
    assert_eq!(
        rows,
        vec![0, 1, 2],
        "zone rows, then the linked zone's rows"
    );
    assert!(sel[1].link && sel[1].visible, "link rows test filter bit 1");
    assert!(!sel[2].visible, "bit 21 is not in the filter");
    // The door is the row's own entry here: it stands at its `position` (block 5, 5).
    assert_eq!(sel[1].location.map(|l| (l.x, l.y)), Some((801.0, 801.0)));

    // Shown from the linked zone, the door is that zone's link entry: it stands at its
    // `link_position` inside the zone (`map-data.md` §4.6), not at the door outside.
    let georef = Georef {
        block_x: 6,
        block_y: 6,
        pixel_x: 0,
        pixel_y: 99,
        scale: 1.0,
    };
    let sel = select_notes(
        &table,
        &index,
        &tree,
        ShownMap::Dungeon {
            zone: other,
            georef: Some(georef),
            frame: (100, 100),
        },
        (100, 100),
        &settings,
        &p,
        &|_| None,
    );
    let rows: Vec<usize> = sel.iter().map(|n| n.row).collect();
    assert_eq!(rows, vec![1, 2], "the link entry, then the zone's own row");
    assert_eq!(sel[0].location.map(|l| (l.x, l.y)), Some((960.0, 960.0)));
    assert_eq!(sel[0].pixel, Some((0, 99)));
}

#[test]
fn note_pixel_frame_excludes_far_edge() {
    let p = pids();
    let tree = MapTree::default();
    let zone = Did(0x7000_0D01);
    let georef = Georef {
        block_x: 0,
        block_y: 0,
        pixel_x: 0,
        pixel_y: 0,
        scale: 1.0,
    };
    let at = |x: f32| {
        let mut n = note(1 << 20, 1, (0, 0), [x, 0.0], 0);
        n.base.dungeon = zone;
        n
    };
    // x = 99 is the last pixel of a 100-wide frame; x = 100 is past it.
    let table = MapNotesTable {
        did: Did(1),
        notes: vec![at(99.0), at(100.0)],
    };
    let index = table.index();
    let settings = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: u64::MAX,
    };
    let sel = select_notes(
        &table,
        &index,
        &tree,
        ShownMap::Dungeon {
            zone,
            georef: Some(georef),
            frame: (100, 100),
        },
        (100, 100),
        &settings,
        &p,
        &|_| None,
    );
    assert_eq!(sel.len(), 2);
    assert_eq!(sel[0].pixel, Some((99, 0)));
    assert_eq!(sel[1].pixel, None, "px == width is outside the frame");
}

#[test]
fn link_targets_scale_linearly() {
    // Many link rows with distinct targets in one zone: the selection stays fast (the
    // distinct-target set is not a linear scan per row).
    let p = pids();
    let tree = MapTree::default();
    let zone = Did(0x7000_0E01);
    let notes: Vec<MapNote> = (0..20_000u32)
        .map(|i| {
            let mut l = note(1, 1, (5, 5), [1.0, 1.0], 0);
            l.base.dungeon = zone;
            l.link = Some(NoteLink {
                position: Position::new(0)
                    .with_region(1)
                    .with_landblock(6, 6)
                    .with_cell(0x101)
                    .with_offset([0.0, 0.0, 0.0]),
                target: Did(0x7100_0000 + i),
            });
            l
        })
        .collect();
    let table = MapNotesTable { did: Did(1), notes };
    let index = table.index();
    let settings = NoteSettings {
        layer: 0,
        discovery_all: true,
        filter: u64::MAX,
    };
    let start = std::time::Instant::now();
    let sel = select_notes(
        &table,
        &index,
        &tree,
        ShownMap::Dungeon {
            zone,
            georef: None,
            frame: (100, 100),
        },
        (100, 100),
        &settings,
        &p,
        &|_| None,
    );
    assert_eq!(sel.len(), MAX_NOTES_SHOWN.min(20_000));
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn icon_tables_by_type_value() {
    let p = pids();
    let images = |a: u32| {
        st(vec![
            e(p.note_image, PropertyValue::DidRef(Did(a))),
            e(p.note_selected, PropertyValue::DidRef(Did(a + 1))),
            e(p.note_highlight, PropertyValue::DidRef(Did(a + 2))),
        ])
    };
    let set = PropertySet {
        entries: vec![e(
            p.note_type_array,
            arr(vec![
                st(vec![
                    e(p.note_type_value, PropertyValue::Bitfield64(1 << 47)),
                    e(p.onscreen, images(0x4100_0100)),
                    e(p.note_rel_z, PropertyValue::Int(-2)),
                ]),
                st(vec![
                    e(p.note_type_value, PropertyValue::Bitfield64(1 << 47)),
                    e(p.onscreen, images(0x4100_0200)),
                ]),
                st(vec![
                    e(p.note_type_value, PropertyValue::Bitfield64(1 << 1)),
                    e(p.onscreen, images(0x4100_0300)),
                    e(p.offscreen, images(0x4100_0400)),
                ]),
            ]),
        )],
        ..PropertySet::default()
    };
    let t = NoteIcons::read(&set, &p);
    let w = t.for_bit(47).unwrap();
    assert_eq!(
        w.onscreen.normal,
        Some(Did(0x4100_0100)),
        "first entry wins"
    );
    assert_eq!(w.relative_z, -2);
    assert!(w.offscreen.is_none());
    assert_eq!(
        t.for_bit(1).unwrap().offscreen.unwrap().highlight,
        Some(Did(0x4100_0402))
    );
    assert!(
        t.get((1 << 1) | (1 << 47)).is_none(),
        "matched on the whole value"
    );
    assert!(t.for_bit(64).is_none());
}

// ---------------------------------------------------------------------------------------------
// Place lookup (§7.3)
// ---------------------------------------------------------------------------------------------

#[test]
fn normalisation_and_scores() {
    assert_eq!(normalize_name("  Bree-land  Forest!"), "bree land forest");
    assert_eq!(normalize_name("Ålborg Ñandú"), "alborg nandu");
    assert_eq!(match_score("bree land", "bree land"), Some(3));
    assert_eq!(match_score("bree land", "bree"), Some(2));
    assert_eq!(match_score("north bree land", "bree"), Some(1));
    assert_eq!(
        match_score("breeland", "bree"),
        None,
        "not at a word boundary"
    );
    assert_eq!(match_score("x", ""), None);
}

fn entry(kind: PlaceKind, name: &str, id: u32, region: Option<u32>) -> PlaceEntry {
    PlaceEntry {
        kind,
        name: name.to_owned(),
        norm: normalize_name(name),
        id,
        region,
        size: 0,
        node: None,
        dungeon_map: None,
        position: None,
    }
}

#[test]
fn ranking_and_cap() {
    let mut idx = PlaceIndex {
        entries: vec![
            entry(PlaceKind::Node, "Vale", 0x10, Some(1)),
            entry(PlaceKind::Area, "Vale", 0x7000_0002, Some(2)),
            entry(PlaceKind::Dungeon, "Vale", 0x7000_0003, Some(2)),
            entry(PlaceKind::Area, "Vale Road", 0x7000_0004, Some(1)),
            entry(PlaceKind::Territory, "Old Vale", 0x7000_0005, Some(1)),
        ],
    };
    let hits = idx.lookup("vale", None, Some(1));
    let ids: Vec<u32> = hits.iter().map(|h| h.entry.id).collect();
    // Exact first, the shown region first among equals, then the more specific kind.
    assert_eq!(
        ids,
        vec![0x10, 0x7000_0003, 0x7000_0002, 0x7000_0004, 0x7000_0005]
    );
    let hits = idx.lookup("vale", None, None);
    assert_eq!(hits[0].entry.kind, PlaceKind::Dungeon);
    for i in 0..20 {
        idx.entries
            .push(entry(PlaceKind::Area, "Hill", 0x7100_0000 + i, None));
    }
    assert_eq!(idx.lookup("hill", None, None).len(), MAX_PLACE_RESULTS);
    assert!(idx.lookup("   ", None, None).is_empty());
    assert!(idx.lookup("nowhere", None, None).is_empty());
}

#[test]
fn place_index_from_regions() {
    let m = census_fixture();
    let r = MapRegions::load(&m);
    let render = |i: &StringInfo| match &i.source {
        tessera_formats::StringSource::Literal(s) => Some(String::from_utf16_lossy(&s.0)),
        tessera_formats::StringSource::Table { .. } => None,
    };
    let idx = PlaceIndex::build(&r, &render);
    let kinds: BTreeSet<PlaceKind> = idx.entries.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&PlaceKind::Node));
    assert!(kinds.contains(&PlaceKind::Area));
    assert!(kinds.contains(&PlaceKind::Dungeon));
    let hit = idx.lookup("second", None, None);
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].entry.dungeon_map, Some(Did(0x7800_0010)));
    assert_eq!(hit[0].entry.position.unwrap().region, 4);
    assert_eq!(hit[0].entry.region, Some(4));
}

/// A tree with a root (no areas) and one child node listing area A1 only; area records for
/// a listed and an unlisted area, their territories and regions.
fn unlisted_fixture() -> MapRegions {
    let p = pids();
    let child = node(
        &p,
        0x602,
        "Child",
        0x4100_0002,
        (1, 1),
        (0, 0),
        1.0,
        Some(1),
        &[0x7000_0001],
        vec![],
    );
    let root = node(
        &p,
        0x601,
        "World",
        0x4100_0001,
        (0, 0),
        (0, 0),
        1.0,
        None,
        &[],
        vec![child],
    );
    let area =
        |did: u32, level: GeoLevel, name: &str, parent: Option<u32>, region: Option<u32>| AreaRef {
            did: Did(did),
            level,
            name: Some(lit(name)),
            parent: parent.map(Did),
            region_id: region,
            map_data: None,
        };
    MapRegions {
        tree: MapTree::from_value(&root, &p).unwrap(),
        areas: vec![
            area(
                0x7000_0001,
                GeoLevel::Area,
                "Listed Vale",
                Some(0x7000_0010),
                None,
            ),
            area(
                0x7000_0002,
                GeoLevel::Area,
                "Lost Vale",
                Some(0x7000_0011),
                None,
            ),
            area(
                0x7000_0010,
                GeoLevel::Territory,
                "Listed Land",
                Some(0x7000_0020),
                None,
            ),
            area(
                0x7000_0011,
                GeoLevel::Territory,
                "Lost Land",
                Some(0x7000_0021),
                None,
            ),
            area(0x7000_0020, GeoLevel::Region, "Listed Realm", None, Some(1)),
            area(0x7000_0021, GeoLevel::Region, "Lost Realm", None, Some(2)),
        ],
        ..MapRegions::default()
    }
}

fn literal_render(i: &StringInfo) -> Option<String> {
    match &i.source {
        tessera_formats::StringSource::Literal(s) => Some(String::from_utf16_lossy(&s.0)),
        tessera_formats::StringSource::Table { .. } => None,
    }
}

#[test]
fn unlisted_places_show_the_root() {
    let r = unlisted_fixture();
    let idx = PlaceIndex::build(&r, &literal_render);
    let node_of = |name: &str| {
        idx.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.node)
            .unwrap()
    };
    assert_eq!(node_of("Listed Vale"), Some(1));
    assert_eq!(
        node_of("Listed Land"),
        Some(1),
        "first listed area of the territory"
    );
    assert_eq!(
        node_of("Listed Realm"),
        Some(1),
        "first listed area of the region"
    );
    // `map-windows.md` §4.1, §4.6 step 3: an area no node lists maps to the root.
    assert_eq!(node_of("Lost Vale"), Some(0));
    assert_eq!(node_of("Lost Land"), Some(0));
    assert_eq!(node_of("Lost Realm"), Some(0));
    let region_of = |name: &str| idx.entries.iter().find(|e| e.name == name).unwrap().region;
    assert_eq!(region_of("Listed Vale"), Some(1));
    assert_eq!(region_of("Lost Land"), Some(2));
    // An empty tree has no node to show.
    let empty = MapRegions {
        tree: MapTree::default(),
        ..unlisted_fixture()
    };
    let idx = PlaceIndex::build(&empty, &literal_render);
    assert!(idx.entries.iter().all(|e| e.node.is_none()));
}

#[test]
fn place_index_scales_linearly() {
    // Many area records in one node and many territories that own none of them: the build
    // stays fast (no per-record scan of the area list).
    let p = pids();
    let n_areas = 20_000u32;
    let listed: Vec<u32> = (0..n_areas).map(|i| 0x7000_0000 + i).collect();
    let root = node(
        &p,
        0x601,
        "World",
        0x4100_0001,
        (0, 0),
        (0, 0),
        1.0,
        None,
        &listed,
        vec![],
    );
    let mut areas: Vec<AreaRef> = listed
        .iter()
        .map(|&d| AreaRef {
            did: Did(d),
            level: GeoLevel::Area,
            name: Some(lit("Field")),
            parent: Some(Did(0x7100_0000)),
            region_id: None,
            map_data: None,
        })
        .collect();
    areas.extend((0..5_000u32).map(|i| AreaRef {
        did: Did(0x7200_0000 + i),
        level: GeoLevel::Territory,
        name: Some(lit("Moor")),
        parent: None,
        region_id: None,
        map_data: None,
    }));
    let r = MapRegions {
        tree: MapTree::from_value(&root, &p).unwrap(),
        areas,
        ..MapRegions::default()
    };
    let start = std::time::Instant::now();
    let idx = PlaceIndex::build(&r, &literal_render);
    assert_eq!(idx.entries.len(), 1 + 20_000 + 5_000);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn map_region_family_and_filters() {
    use crate::filter::{Compiled, Pred, matches_element};
    use crate::query::{AttrType, Attribute, Family, MAP_REGION_KINDS, attribute};

    assert_eq!(Family::from_keyword("map-region"), Some(Family::MapRegion));
    assert!(Family::MapRegion.is_table());
    assert_eq!(Family::ALL.last(), Some(&Family::Did), "did stays last");
    assert_eq!(
        attribute(Family::MapRegion, "kind"),
        Some((Attribute::Kind, AttrType::Keyword(MAP_REGION_KINDS)))
    );
    assert_eq!(
        attribute(Family::MapRegion, "region"),
        Some((Attribute::Region, AttrType::Integer))
    );
    let m = census_fixture();
    let r = MapRegions::load(&m);
    let elements = crate::tables::map_region_elements(&r);
    assert_eq!(elements.len(), r.regions.len());
    let kind = |k: &str| Compiled {
        attribute: Attribute::Kind,
        pred: Pred::Keywords(vec![k.into()]),
    };
    let region = |v: i64| Compiled {
        attribute: Attribute::Region,
        pred: Pred::Integers(vec![v]),
    };
    let count = |c: &Compiled| {
        elements
            .iter()
            .filter(|e| matches_element(Family::MapRegion, e, c))
            .count()
    };
    assert_eq!(count(&kind("dungeon")), 2);
    assert_eq!(count(&kind("node")), 7);
    // Region 1: the inside node; region 2: the C6 node; region 4: the dungeon map.
    assert_eq!(count(&region(1)), 1);
    assert_eq!(count(&region(2)), 1);
    assert_eq!(count(&region(4)), 1);
    assert_eq!(count(&region(0)), 0, "no location matches nothing");
}
