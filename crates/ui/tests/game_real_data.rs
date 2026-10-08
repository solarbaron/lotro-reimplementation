// SPDX-License-Identifier: GPL-3.0-or-later
//! The game widgets against a user's own install (read-only): the HUD panels with game
//! types (`game-panels.md` P1, P3, P4, P5, P11, P14) are opened, the game vocabulary is
//! resolved by label, and the sandbox results of §3 hold (GP1, GP7, GP8, GP9, GP12, the
//! radar's parts and placement of §8.2, GP5, GP6); the world map P10 is attached and its
//! tree checked (GP3); the chat window of `chat-window.md` opens from `GOP_ChatWindows` of
//! `GameplayOptionDefaults` (§10 item 1) and takes lines (CW-I1, CW-I2, CW-I4, CW-I8). Only
//! counts are printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test game_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::property_set::PropertySetRecord;
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::chat::{Chat, ChatConfig, OpacityTable, WindowConfig};
use tessera_ui::game::radar::RadarInput;
use tessera_ui::game::worldmap::MapShown;
use tessera_ui::game::{
    GameLayouts, GamePanels, GameVocab, MapLookup, MapSource, NoShortcuts, SandboxData, subtree,
};
use tessera_ui::{Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Mutex<HashMap<u32, Option<Arc<UiLayout>>>>,
}

impl UiEnv for InstallEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let mut c = self.cache.lock().ok()?;
        c.entry(did.0)
            .or_insert_with(|| {
                let payload = self.set.read(did.0).ok()?;
                UiLayout::parse_expecting(&payload, did.0, &*self.dict)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

const HUD: [(&str, &str, &str); 6] = [
    ("P1", "gameplayactivatables", "GamePlay_RadarBox"),
    ("P3", "toolbar", "ToolbarField"),
    ("P4", "toolbar", "AvatarPanel"),
    ("P5", "toolbar", "OpponentPanel"),
    ("P11", "gameplayactivatables", "GamePlay_QuestTrackerBox"),
    ("P14", "gameplayactivatables", "GamePlay_Undocked_CtrlBar"),
];

#[test]
fn game_widgets_on_the_native_panels() {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        }
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };
    let registry = PropertyRegistry::from_dictionary(dict.clone());
    let dict = Arc::new(dict);
    let root = did_map(&set, Did(ROOT_DID_MAP)).expect("root DID map");
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(&set, d))
        .expect("EMAPPER map");
    let labels = UiLabels::load(
        |label| emapper.by_label(label),
        |did| {
            let payload = set.read(did.0).ok()?;
            EnumMapper::parse_expecting(&payload, did).ok()
        },
    );
    let uilayout = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .expect("UILAYOUT map");
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mut gv = GameVocab::default();
    gv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    println!("unresolved types: {:?}", unresolved(gv.types.entries()));
    println!("unresolved element IDs: {:?}", unresolved(gv.ids.entries()));
    println!("unresolved states: {:?}", unresolved(gv.states.entries()));
    println!("unresolved properties: {:?}", unresolved(gv.pids.entries()));

    let env = InstallEnv {
        set: set.clone(),
        dict: dict.clone(),
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab.clone(), (1920, 1080));
    gv.register_classes(&mut ui);
    let layouts = GameLayouts {
        radar: uilayout.by_label("radar"),
        quickslots: uilayout.by_label("quickslots"),
        map: uilayout.by_label("map"),
    };
    println!("layouts found: {layouts:?}");
    let mut gp = GamePanels::new(gv.clone(), layouts);
    let data = SandboxData::default();
    let mut roots = Vec::new();
    for (name, label, root_label) in HUD {
        let layout = uilayout.by_label(label).expect("UILAYOUT label");
        let id = labels
            .key(UiEnum::ElementId, root_label)
            .expect("root element label");
        let k = ui.open_panel(layout, id).expect("panel opens");
        gp.attach(&mut ui, k, &data, &NoShortcuts);
        println!(
            "{name}: drag boxes {}, highlighters {}, fields {}, radars {}, bars {}",
            gp.drag_boxes.len(),
            gp.highlighters.len(),
            gp.hide_fields.len(),
            gp.radars.len(),
            gp.bars.len()
        );
        roots.push((name, k));
    }
    // GP1 and GP7.
    let hidden = labels.key(UiEnum::StateId, "hidden");
    assert!(!gp.drag_boxes.is_empty());
    for b in &gp.drag_boxes {
        let kids = [b.frame, b.bar, b.title];
        assert!(
            kids.iter().all(Option::is_some),
            "GP1: three frame children"
        );
        for k in kids.into_iter().flatten() {
            assert!(!ui.is_visible(k), "GP7: frame child hidden");
        }
        for pid in [
            gv.pids.dbox_hidden_state,
            gv.pids.dbox_visible_state,
            gv.pids.dbox_dragging_state,
            gv.pids.dbox_mouseover_state,
        ] {
            assert!(ui.prop(b.key, pid).is_set(), "GP1: state property");
        }
        if let Some(h) = hidden {
            assert_eq!(ui.prop(b.key, gv.pids.dbox_hidden_state).key(), h);
        }
    }
    // GP8.
    for h in &gp.highlighters {
        assert!(!ui.is_visible(h.key), "GP8: highlighter hidden");
    }
    let mut pips = 0;
    for (_, r) in &roots {
        for k in subtree(&ui, *r) {
            if ui.element_type(k) == Some(gv.types.pip_display) {
                pips += 1;
                assert!(!ui.is_visible(k), "GP8: pip display hidden");
            }
        }
    }
    println!("pip displays {pips}");
    // GP9.
    for (label, visible) in [
        ("PlayerLevel_HWField", true),
        ("PlayerScaledLevel_HWField", false),
        ("OpponentLevel_HWField", true),
    ] {
        let Some(id) = labels.key(UiEnum::ElementId, label) else {
            println!("{label}: label not found");
            continue;
        };
        let found: Vec<_> = roots
            .iter()
            .flat_map(|(_, r)| subtree(&ui, *r))
            .filter(|k| ui.element_id(*k) == Some(id))
            .collect();
        println!("{label}: {} instance(s)", found.len());
        for k in found {
            assert_eq!(ui.is_visible(k), visible, "GP9 {label}");
        }
    }
    // GP12 and GP6.
    for b in &gp.bars {
        if let Some(list) = b.list {
            let entries = ui.list_entries(list);
            let lw = ui.design_box(list).map_or(0, |d| d.w());
            println!("quickslot bar: {} slots, list box {lw} wide", entries.len());
            if layouts.quickslots.is_some() {
                assert_eq!(entries.len(), 12);
                assert!(entries.iter().all(|e| !ui.is_visible(*e)), "GP12");
                let d = ui.design_box(entries[0]).unwrap();
                assert_eq!((d.w(), d.h()), (35, 35), "GP6: button template");
            }
        }
    }
    let main_bar = gp.bars.first().and_then(|b| b.list).expect("P3 has a bar");
    assert_eq!(
        ui.design_box(main_bar).unwrap().w(),
        12 * 35,
        "GP6: main bar"
    );
    // §8.2.
    let r = gp.radars.first().expect("P1 has a radar");
    println!("radar parts found: {}", r.parts.is_some());
    assert!(r.parts.is_some());
    assert_eq!(r.centre, (85, 85));
    let mb = ui.layout_box(r.parts.unwrap().marker).unwrap();
    assert_eq!((mb.l, mb.t), (71, 71));
    // GP5.
    let tiles = r.parts.unwrap().tiles;
    let d = ui.design_box(tiles).unwrap();
    assert_eq!((d.w(), d.h()), (600, 600), "GP5: RadarMapImage");
    assert_eq!(ui.prop(tiles, vocab.pids.lb_max_rows).int(), 3, "GP5: rows");
    assert_eq!(
        ui.prop(tiles, vocab.pids.lb_max_columns).int(),
        3,
        "GP5: columns"
    );
    let template = ui
        .create_element(
            None,
            layouts.radar.expect("radar layout"),
            gv.ids.radar_tile_template,
        )
        .expect("RadarMapImage_Template");
    let d = ui.design_box(template).unwrap();
    assert_eq!((d.w(), d.h()), (200, 200), "GP5: template");
    ui.destroy(template);

    // GP3: P10's map tree, through the world-map attachment of §9.5.
    let map_layout = layouts.map.expect("map layout");
    let map_root_id = labels
        .key(UiEnum::ElementId, "MapPanel")
        .expect("MapPanel label");
    let map_root = ui.open_panel(map_layout, map_root_id).expect("P10 opens");
    let shown = gp
        .attach_world_map(&mut ui, map_root, None, &RadarInput::default(), &mut NoMaps)
        .expect("world map attaches");
    assert_eq!(
        shown,
        Some(MapShown::Node(0)),
        "the root without a position"
    );
    let w = gp.world_map.as_ref().unwrap();
    let tree = w.tree.as_ref().expect("UI_Map_AreaData");
    let map_l = ui_layout(&set, &dict, map_layout);
    let mut elements = std::collections::BTreeSet::new();
    let mut areas = HashMap::<u32, usize>::new();
    for n in &tree.nodes {
        assert!(
            map_l.top_level_index(n.active_element).is_some(),
            "GP3: active element is a top-level element"
        );
        assert!(elements.insert(n.active_element), "GP3: distinct");
        for a in &n.areas {
            *areas.entry(*a).or_default() += 1;
        }
    }
    assert!(areas.values().all(|c| *c == 1), "GP3: one node per area");
    let depth = tree.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    println!(
        "world map: {} nodes, depth {depth}, {} areas, marker {}",
        tree.nodes.len(),
        areas.len(),
        w.marker.is_some()
    );
    // The counts the spec gives for this install (GP3); a game update may change them.
    assert_eq!(tree.nodes.len(), 227, "GP3: nodes");
    assert_eq!(areas.len(), 541, "GP3: areas");
    assert!(w.marker.is_some(), "§9.5: the marker from the minimap");
    let (_, active) = w.shown.unwrap();
    let bg = ui.parent(active).expect("MapBackground");
    assert_eq!(ui.children(bg).first(), Some(&active), "§9.5: first child");

    // The chat window.
    let chat_layout = uilayout.by_label("chatwindow").expect("chatwindow layout");
    let layout = ui_layout(&set, &dict, chat_layout);
    let ids: Vec<i32> = layout
        .elements
        .iter()
        .filter_map(|d| {
            d.block
                .properties
                .entries
                .iter()
                .find(|e| e.pid == gv.pids.chat_window_id)
                .map(|e| tessera_ui::props::as_int(&e.value))
        })
        .collect();
    println!("chat window IDs {ids:?}");
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (1..=9).collect::<Vec<_>>(), "CW-I1");
    // CW-I1 and CW-I2 on the resolved windows: open all nine without tabs.
    let mut probe = Chat::new(gv.clone(), chat_layout);
    let all = ChatConfig {
        windows: (1..=9)
            .map(|id| WindowConfig {
                id,
                visible: false,
                tabs: Vec::new(),
            })
            .collect(),
    };
    probe.open(&mut ui, &all).expect("nine chat windows open");
    let primaries: Vec<i32> = probe
        .windows
        .iter()
        .filter(|w| w.primary)
        .map(|w| w.id)
        .collect();
    assert_eq!(primaries, vec![1], "CW-I1: exactly one primary, ID 1");
    for w in &probe.windows {
        assert_eq!(w.entry.is_some(), w.primary, "CW-I2");
    }
    for w in &probe.windows {
        ui.destroy(w.root);
    }
    // §10 item 1: the configuration from `GameplayOptionDefaults` (UNIQUEDB key 0x68).
    let unique = root
        .by_label("UNIQUEDB")
        .and_then(|d| did_map(&set, d))
        .expect("UNIQUEDB map");
    let defaults = property_set(&set, &dict, &unique, 0x68).expect("GameplayOptionDefaults");
    let cfg = ChatConfig::from_value(
        defaults
            .set
            .get(gv.pids.gop_chat_windows)
            .expect("GOP_ChatWindows"),
        &gv.pids,
    )
    .expect("GOP_ChatWindows decodes");
    println!(
        "GOP_ChatWindows: {} window(s), tabs {:?}",
        cfg.windows.len(),
        cfg.windows.iter().map(|w| w.tabs.len()).collect::<Vec<_>>()
    );
    assert_eq!(cfg.windows.len(), 1);
    assert_eq!(cfg.windows[0].id, 1);
    assert_eq!(cfg.windows[0].tabs.len(), 2);
    assert!(cfg.windows[0].tabs.iter().all(|t| t.name.is_some()));
    // CW-I4: the opacity ranges (UNIQUEDB key 0x1000000C).
    let opac = property_set(&set, &dict, &unique, 0x1000_000C).expect("UI_ChatOpacities");
    let table = opac
        .set
        .iter()
        .map(|e| OpacityTable::from_value(&e.value, &gv.pids))
        .find(|t| !t.ranges.is_empty())
        .expect("an opacity array");
    let mut ranges = table.ranges.clone();
    ranges.sort_unstable();
    println!("opacity ranges {}", ranges.len());
    assert!(
        ranges.iter().all(|(lo, hi, _)| lo <= hi),
        "CW-I4: min <= max"
    );
    assert_eq!(ranges.first().map(|r| r.0), Some(0), "CW-I4: from 0");
    assert_eq!(ranges.last().map(|r| r.1), Some(100), "CW-I4: to 100");
    for pair in ranges.windows(2) {
        assert_eq!(pair[0].1 + 1, pair[1].0, "CW-I4: disjoint, no gap");
    }
    let mut chat = Chat::new(gv.clone(), chat_layout);
    chat.open(&mut ui, &cfg).expect("chat window opens");
    let w = &chat.windows[0];
    assert!(w.primary, "CW-I1: window 1 is primary");
    assert!(w.entry.is_some(), "CW-I2");
    assert_eq!(w.tabs.len(), 2);
    let list = w.tabs[0].list.expect("ChatLogListbox");
    let cap = ui.prop(list, vocab.pids.lb_max_rows).int();
    println!("chat line cap {cap}");
    assert!(cap > 0, "CW-I8");
    assert_eq!(chat.post_line(&mut ui, 0x80, "x"), 1);
    assert_eq!(ui.list_entries(list).len(), 1);
}

/// A minimap and world-map data source with no records (the checks need none).
struct NoMaps;

impl MapSource for NoMaps {
    fn landblock_map(&mut self, _region: u8, _x: u8, _y: u8) -> MapLookup {
        MapLookup::Ready(None)
    }

    fn dungeon_map(&mut self, _region: u8, _x: u8, _y: u8, _cell: u16) -> MapLookup {
        MapLookup::Ready(None)
    }
}

/// The property-set record a `UNIQUEDB` key names.
fn property_set(
    set: &ArchiveSet,
    dict: &PropertyDictionary,
    unique: &DataIdMap,
    key: u32,
) -> Option<PropertySetRecord> {
    let did = unique.get(key)?.value;
    let payload = set.read(did.0).ok()?;
    PropertySetRecord::parse_expecting(&payload, did, dict).ok()
}

fn ui_layout(set: &ArchiveSet, dict: &PropertyDictionary, did: Did) -> UiLayout {
    let payload = set.read(did.0).expect("read layout");
    UiLayout::parse_expecting(&payload, did.0, dict).expect("layout decodes")
}
