// SPDX-License-Identifier: GPL-3.0-or-later
//! The P5 widgets against a user's own install (read-only): the map window opens from the
//! `map` layout and reads its tree (`map-windows.md` §4.1), every node is shown from its
//! active element (§4.2) with the filter menu built (§6.9), notes and a feed marker are
//! placed, the presentations switch (§3.1), and a dungeon map is shown in `DungeonMap`
//! (§4.7); the travel window, the fellowship maker's finder page, the instance-travel
//! window, the end-travel box and the queue alert open from their `UILAYOUT` labels
//! (`travel-windows.md` §3) with the parts the spec names, and are filled with literal
//! values through the install's row templates and painted. Only counts and labels are
//! printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test p5_real_data -- --nocapture`.

mod p5_common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use p5_common::{art, icons, paint_frames};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::finder::{
    DefaultFinderTexts, FinderEntry, FinderTemplates, FinderView, FinderWindow,
    InstanceTravelWindow, QueueAlert,
};
use tessera_ui::game::map::MapPoint;
use tessera_ui::game::mapmarks::{DefaultMapTexts, FeedMarker, MapNote, OverlayId};
use tessera_ui::game::mapwindow::{MapWindowView, Presentation, Shown};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::travel::{
    DefaultTravelTexts, EndTravelBox, TravelRow, TravelTemplates, TravelView, TravelWindow,
};
use tessera_ui::game::{GameVocab, NpcVocab, P5Vocab};
use tessera_ui::{ElementKey, Ui, UiEnv, Vocab};

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

struct Install {
    ui: Ui,
    gv: GameVocab,
    pv: P5Vocab,
    nv: NpcVocab,
    layouts: DataIdMap,
    labels: UiLabels,
}

impl Install {
    fn layout(&self, label: &str) -> Did {
        self.layouts
            .by_label(label)
            .unwrap_or_else(|| panic!("UILAYOUT {label} (LayoutMissing)"))
    }

    fn id(&self, label: &str) -> u32 {
        self.labels
            .key(UiEnum::ElementId, label)
            .unwrap_or_else(|| panic!("element {label} does not resolve"))
    }

    fn open(&mut self, layout: &str, root: &str) -> ElementKey {
        let (l, r) = (self.layout(layout), self.id(root));
        self.ui
            .open_panel(l, r)
            .unwrap_or_else(|e| panic!("{layout}/{root}: {e} (ElementMissing)"))
    }

    fn template(&self, layout: &str, element: &str) -> Option<ExamTemplate> {
        Some(ExamTemplate {
            layout: self.layouts.by_label(layout)?.0,
            element: self.labels.key(UiEnum::ElementId, element)?,
        })
    }

    /// The labels of `names` that are missing below `root`.
    fn missing(&self, root: ElementKey, names: &[&str]) -> Vec<String> {
        names
            .iter()
            .filter(|n| {
                let Some(id) = self.labels.key(UiEnum::ElementId, n) else {
                    return true;
                };
                self.ui.element_id(root) != Some(id)
                    && tessera_ui::game::find_below(&self.ui, root, id).is_none()
            })
            .map(|n| (*n).to_string())
            .collect()
    }
}

fn install() -> Option<Install> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
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
    let layouts = root
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
    let mut pv = P5Vocab::default();
    pv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let mut nv = NpcVocab::default();
    nv.resolve_with(
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
    println!("P5 unresolved types: {:?}", unresolved(pv.types.entries()));
    println!(
        "P5 unresolved element IDs: {:?}",
        unresolved(pv.ids.entries())
    );
    println!(
        "P5 unresolved states: {:?}",
        unresolved(pv.states.entries())
    );
    println!(
        "P5 unresolved properties: {:?}",
        unresolved(pv.pids.entries())
    );
    let env = InstallEnv {
        set,
        dict,
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab, (1920, 1080));
    gv.register_classes(&mut ui);
    nv.register_classes(&mut ui);
    pv.register_classes(&mut ui);
    Some(Install {
        ui,
        gv,
        pv,
        nv,
        layouts,
        labels,
    })
}

#[test]
fn map_window_from_the_install() {
    let Some(mut ins) = install() else { return };
    let t = DefaultMapTexts;
    let layout = ins.layout("map");
    let (gv, pv) = (ins.gv.clone(), ins.pv.clone());
    let ui = &mut ins.ui;
    let mut v = MapWindowView::open_layout(ui, &gv, &pv, layout, &art()).expect("map window");
    v.set_icons(icons());
    v.set_filter_value(pv.pids.gop_map_filter, u64::MAX);
    v.set_filter_value(pv.pids.gop_distant_filter, u64::MAX);
    let n = v.tree().map_or(0, |t| t.nodes.len());
    println!("map tree: {n} nodes");
    assert!(n > 0, "the map tree has nodes");
    let missing: Vec<&str> = ["MapField", "FilterMenu", "MapMenu", "CoordinateDisplay"]
        .into_iter()
        .zip([
            v.parts.field,
            v.parts.filter_menu,
            v.parts.map_menu,
            v.parts.coordinates,
        ])
        .filter(|(_, k)| k.is_none())
        .map(|(l, _)| l)
        .collect();
    assert!(missing.is_empty(), "map window parts missing: {missing:?}");
    assert!(v.parts.background.is_some(), "MapBackground");
    assert!(v.windowed_root.is_some(), "MapPanel");
    // Every node shows from its active element, with its image (MW4) and its filter menu.
    let mut failures = Vec::new();
    let mut hidden_marker = 0usize;
    let mut menu_items = 0usize;
    for i in 0..n {
        match v.show_node(ui, i, &t) {
            Ok(_) => {
                if !v.draws_image(ui) {
                    failures.push((i, "no image"));
                }
                if v.marker.is_none() {
                    hidden_marker += 1;
                }
                menu_items += v.filter_menu.items.len();
            }
            Err(e) => {
                println!("node {i}: {e}");
                failures.push((i, "not shown"));
            }
        }
    }
    println!(
        "nodes shown: {}, without player marker (UI_Map_HidePlayerMapnote): {hidden_marker}, \
         filter-menu items in all: {menu_items}",
        n - failures.len()
    );
    assert!(failures.is_empty(), "nodes that failed: {failures:?}");
    // Notes and a feed marker on the first node with a georeference inside the world.
    let node = (1..n).find(|i| {
        v.tree()
            .and_then(|t| t.georef(*i))
            .and_then(|g| tessera_ui::game::mapwindow::census_test_point(&g, (1024, 768)))
            .is_some()
    });
    if let Some(i) = node {
        v.show_node(ui, i, &t).unwrap();
        let g = v.tree().unwrap().georef(i).unwrap();
        let p = tessera_ui::game::mapwindow::census_test_point(&g, (1024, 768)).unwrap();
        let region = v.tree().unwrap().nodes[i]
            .region
            .and_then(|r| u8::try_from(r).ok())
            .unwrap_or(1);
        let point = MapPoint {
            global: p,
            region,
            instance: 0,
        };
        let area = v.tree().unwrap().nodes[i].areas.first().copied();
        v.set_notes(
            ui,
            vec![MapNote {
                id: 1,
                note_type: 1 << 20,
                point,
                area,
                ..MapNote::default()
            }],
            &t,
        );
        v.add_marker(
            ui,
            FeedMarker {
                id: 7,
                point,
                label: "census".into(),
                ..FeedMarker::default()
            },
            &t,
        );
        let px = v
            .overlay_pixel(ui, OverlayId::Marker(7))
            .expect("marker placed");
        let want = g.to_pixel(p.0, p.1);
        assert!(
            (px.0 - want.0).abs() <= 1 && (px.1 - want.1).abs() <= 1,
            "marker at {px:?}, transform {want:?}"
        );
    }
    // Presentations.
    v.set_presentation(ui, Presentation::Windowed);
    assert_eq!(v.presentation, Presentation::Windowed);
    v.set_presentation(ui, Presentation::FullScreen);
    // A dungeon map in `DungeonMap` (the first node's map data stands in for a dungeon's).
    let m = v.tree().unwrap().nodes[0].map.clone();
    v.show_dungeon(ui, m, &t).expect("DungeonMap");
    assert_eq!(v.shown, Some(Shown::Dungeon));
    paint_frames(ui);
    v.detach(ui);
}

#[test]
fn travel_windows_from_the_install() {
    let Some(mut ins) = install() else { return };
    let tt = DefaultTravelTexts;
    let ft = DefaultFinderTexts;
    // The travel window (§4.2).
    let root = ins.open("travel", "Travel_Window");
    let missing = ins.missing(
        root,
        &[
            "Travel_NameText",
            "Travel_CloseButton",
            "TravelListBox",
            "TravelList_SearchText",
            "Travel_NoDestinationField",
            "Travel_Currency_MinLevel",
            "Travel_RouteStatus",
            "Travel_SwiftTravel",
            "Travel_Territory_Field",
            "Travel_Area_Field",
            "Travel_Currency_GoButton",
            "Travel_Currency_DestinationCost",
            "Travel_Currency_DestinationCost_Free",
        ],
    );
    assert!(
        missing.is_empty(),
        "travel window parts missing: {missing:?}"
    );
    let templates = TravelTemplates {
        row: ins.template("travel", "Travel_ListItem"),
        header: ins.template("travel", "Travel_ListItem_Header"),
    };
    println!(
        "travel templates: row {}, header {}",
        templates.row.is_some(),
        templates.header.is_some()
    );
    let (pv, nv) = (ins.pv.clone(), ins.nv.clone());
    let a = art();
    let ui = &mut ins.ui;
    let mut w = TravelWindow::attach(ui, &nv, &pv, root, templates, &a);
    let row = |entry: u16, name: &str, territory: &str, coin: u64| TravelRow {
        entry,
        name: name.into(),
        territory: territory.into(),
        area: "Area".into(),
        coin,
        coins: coin_counts(coin, &[100_000, 100, 1]),
        min_level: 10,
        discovered: entry != 3,
        met: true,
        usable: entry != 4,
        swift: entry == 2,
    };
    w.show(
        ui,
        &TravelView {
            window: 1,
            title: "Travel".into(),
            player_level: 20,
            rows: vec![
                row(1, "One", "North", 0),
                row(2, "Two", "North", 2500),
                row(3, "Three", "South", 90),
                row(4, "Four", "", 12_000),
            ],
        },
        &tt,
    );
    assert_eq!(w.rows.len(), 4);
    w.select(ui, Some(1), &tt);
    paint_frames(ui);
    // The fellowship maker's finder page (§6.2).
    let root = ins.open("fellowshipmaker", "FellowshipMaker_Window");
    let ftemplates = FinderTemplates {
        row: ins.template("fellowshipmaker", "FellowshipMaker_EncounterDisplayBase"),
        header: ins.template("fellowshipmaker", "FellowshipMaker_EncounterDisplayHeader"),
        header2: ins.template(
            "fellowshipmaker",
            "FellowshipMaker_EncounterDisplayHeader_Tier2",
        ),
        confirm: ins.template("fellowshipmaker", "FellowshipMakerConfirmPage"),
    };
    println!(
        "finder templates: row {}, header {}, header2 {}",
        ftemplates.row.is_some(),
        ftemplates.header.is_some(),
        ftemplates.header2.is_some()
    );
    let ui = &mut ins.ui;
    let mut f = FinderWindow::attach(ui, &nv, &pv, root, ftemplates, &a, &ft);
    let entries = vec![
        FinderEntry {
            entry: 1,
            name: "One".into(),
            kind: 2,
            kind_label: "Kind".into(),
            category: 1,
            category_label: "Cat".into(),
            sizes: vec![(1, "Solo".into())],
            ..FinderEntry::default()
        },
        FinderEntry {
            entry: 2,
            name: "Two".into(),
            kind: 1,
            kind_label: "Kind 1".into(),
            ..FinderEntry::default()
        },
    ];
    f.show(
        ui,
        &FinderView {
            window: 2,
            entries,
            featured: Some(1),
            player_level: 50,
        },
        &ft,
    );
    println!(
        "finder parts: tree {}, join {}, size menu {}, featured {}, confirm page {}",
        f.parts.tree.is_some(),
        f.parts.join.is_some(),
        f.parts.size_menu.is_some(),
        f.parts.featured.is_some(),
        f.parts.confirm_page.is_some()
    );
    assert!(
        f.parts.join.is_some(),
        "FellowshipMakerLaunchNow_JoinButton"
    );
    assert!(
        f.parts.tree.is_some(),
        "FellowshipMakerLaunchNow_EncounterTree"
    );
    assert_eq!(f.rows.len(), 2, "two finder rows");
    f.select(ui, Some(0), &ft);
    f.set_queue(ui, 2, Some(30), 0.0, &ft);
    paint_frames(ui);
    // The instance-travel window (§4.11).
    let root = ins.open("pe_travel", "RootPETravelField");
    let ui = &mut ins.ui;
    let mut it = InstanceTravelWindow::attach(ui, &pv, root, &ft);
    it.show(ui, 3, "Name", "Intro");
    // The end-travel box (§5.1) and the queue alert (§7.2).
    let boxes = {
        let l = ins.layout("gameplayactivatables");
        ins.ui.open_activatables(l)
    };
    let ui = &mut ins.ui;
    let end = boxes
        .iter()
        .copied()
        .find(|k| tessera_ui::game::find_below(ui, *k, pv.ids.end_travel).is_some());
    println!("end-travel box found: {}", end.is_some());
    if let Some(k) = end {
        let b = EndTravelBox::attach(ui, &pv, k, &tt);
        b.set_shown(ui, true);
    }
    let alert_layout = ins.layout("alert");
    let alert_id = ins.pv.ids.queue_alert;
    let ui = &mut ins.ui;
    match ui.create_element(None, alert_layout, alert_id) {
        Ok(k) => {
            let a = QueueAlert::attach(ui, &pv, k);
            a.set_state(ui, 2, &ft);
            println!("queue alert: created");
        }
        Err(e) => panic!("FellowshipMakerWaitingInQueueAlertField: {e} (ElementMissing)"),
    }
    paint_frames(ui);
}
