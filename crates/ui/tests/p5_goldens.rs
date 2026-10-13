// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic goldens of the P5 windows (decisions 0031 tier 1 and 0036 §2): the
//! map window with a generated map image, notes and feed markers, a minimap with notes and
//! blips, the travel window, the instance finder and its queue page, each built with the
//! fallback widgets, filled with literal values and generated art, painted by the CPU
//! painter and compared with `tests/golden/p5_<name>.png` (one step per channel, no
//! outliers). `TESSERA_BLESS=1` rewrites the images. No install file is opened.
//!
//! These are **widget goldens**: the windows are filled from hand-built view structs, not
//! from a scenario run (`tessera-ui` links no session layer); the scenario-driven goldens
//! of P5 exit criteria 1 and 3 belong to `apps/client`. The map window's fallback tree is
//! Tessera's provisional reading while `map-windows.md` §8 has its OPEN on a fallback map
//! window.

mod p5_common;

use p5_common::*;
use tessera_formats::property::PropertyValue;
use tessera_ui::game::finder::{
    DefaultFinderTexts, FinderEntry, FinderTemplates, FinderView, FinderWindow,
};
use tessera_ui::game::map::{GameMap, MapPoint};
use tessera_ui::game::mapfallback;
use tessera_ui::game::mapmarks::{
    DefaultMapTexts, FeedMarker, MapNote, MapOverlays, MapTexts, RadarContext,
};
use tessera_ui::game::mapwindow::{MapWindowView, PlayerMap};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::radar::{Radar, RadarInput};
use tessera_ui::game::travel::{
    DefaultTravelTexts, TravelRow, TravelTemplates, TravelView, TravelWindow,
};
use tessera_ui::game::travelfallback;
use tessera_ui::game::worldmap::{MapNode, MapTree, NodeOptions};
use tessera_ui::game::{MapLookup, MapSource};

const VALUES: [u64; 3] = [100_000, 100, 1];

struct Coords;
impl MapTexts for Coords {
    fn coordinates(&self, p: &MapPoint) -> Option<String> {
        Some(format!(
            "{:.1}N, {:.1}E",
            p.global.1 / 160.0,
            p.global.0 / 160.0
        ))
    }
    fn note_type_label(&self, key: u32) -> String {
        match key {
            21 => "Vendor".into(),
            22 => "Trainer".into(),
            _ => format!("Type {key}"),
        }
    }
}

fn gmap(image: u32, block: (i32, i32), pixel: (i32, i32), scale: f32) -> GameMap {
    GameMap {
        image,
        block_x: Some(block.0),
        block_y: Some(block.1),
        pixel_x: pixel.0,
        pixel_y: pixel.1,
        scale: Some(scale),
        ..GameMap::default()
    }
}

fn note(id: u64, bit: u32, global: (f32, f32)) -> MapNote {
    MapNote {
        id,
        note_type: 1 << bit,
        point: MapPoint {
            global,
            region: 1,
            instance: 0,
        },
        area: Some(0x7000_0001),
        tooltip: format!("Note {id}"),
        text: format!("Note {id}"),
        ..MapNote::default()
    }
}

fn marker(id: u32, quest: bool, global: (f32, f32), label: &str) -> FeedMarker {
    FeedMarker {
        id,
        point: MapPoint {
            global,
            region: 1,
            instance: 0,
        },
        label: label.into(),
        quest,
        ..FeedMarker::default()
    }
}

#[test]
fn map_window_with_notes_and_markers() {
    let (gv, pv, nv) = vocabs();
    let mut ui = ui(1024, 768);
    let mut t = MapTree::default();
    t.push(
        MapNode {
            parent: None,
            depth: 0,
            active_element: 0x1000_0200,
            menu_name: None,
            map: GameMap::default(),
            pixel_size: (1024, 768),
            region: None,
            areas: vec![],
            children: Vec::new(),
        },
        NodeOptions::default(),
    );
    t.push(
        MapNode {
            parent: Some(0),
            depth: 0,
            active_element: 0x1000_0201,
            menu_name: None,
            map: gmap(MAP_IMAGE, (100, 100), (0, 768), 0.5),
            pixel_size: (1024, 768),
            region: Some(1),
            areas: vec![0x7000_0001],
            children: Vec::new(),
        },
        NodeOptions {
            display_type: (1 << VENDOR_BIT) | (1 << TRAINER_BIT),
            filter_property: pv.pids.gop_map_filter,
            ..NodeOptions::default()
        },
    );
    let mut v = MapWindowView::open_fallback(&mut ui, &gv, &pv, &nv, t, &art(), &Coords).unwrap();
    v.set_icons(icons());
    v.set_filter_value(
        pv.pids.gop_map_filter,
        (1 << VENDOR_BIT) | (1 << TRAINER_BIT),
    );
    v.show_node(&mut ui, 1, &Coords).unwrap();
    v.set_notes(
        &mut ui,
        vec![
            note(1, VENDOR_BIT, (16_400.0, 16_900.0)),
            note(2, VENDOR_BIT, (16_700.0, 17_100.0)),
            note(3, TRAINER_BIT, (17_200.0, 16_500.0)),
            note(4, TRAINER_BIT, (16_300.0, 16_300.0)),
        ],
        &Coords,
    );
    v.add_marker(
        &mut ui,
        marker(1, true, (17_000.0, 16_800.0), "The old mill"),
        &Coords,
    );
    v.add_marker(
        &mut ui,
        marker(2, false, (16_600.0, 16_400.0), "Camp"),
        &Coords,
    );
    let mut own = marker(3, false, (17_400.0, 17_000.0), "Signal fire");
    own.icon = Some(art_ref(33));
    v.add_marker(&mut ui, own, &Coords);
    v.update_player(
        &mut ui,
        &PlayerMap {
            point: MapPoint {
                global: (16_800.0, 16_700.0),
                region: 1,
                instance: 0,
            },
            heading: 45.0,
            valid: true,
            ..PlayerMap::default()
        },
    );
    settle(&mut ui);
    let d = ui.displayed_box(v.shown_element.unwrap());
    v.pointer_moved(&mut ui, d.l + 600, d.t + 300, &Coords);
    // The filter menu, opened.
    let fm = v.parts.filter_menu.unwrap();
    let e = button(&ui, fm);
    v.handle_event(&mut ui, &e, &Coords);
    let window = v.window;
    let mut img = capture(&mut ui, window);
    overlay_surfaces(&ui, window, &mut img, &[v.marker.unwrap()]);
    golden("map_window", &img);
    let popup = v.popup_root().unwrap();
    golden("map_filter_menu", &capture(&mut ui, popup));
}

struct Indoor(GameMap);
impl MapSource for Indoor {
    fn landblock_map(&mut self, _: u8, _: u8, _: u8) -> MapLookup {
        MapLookup::Ready(None)
    }
    fn dungeon_map(&mut self, _: u8, _: u8, _: u8, _: u16) -> MapLookup {
        MapLookup::Ready(Some(self.0.clone()))
    }
}

#[test]
fn minimap_with_notes() {
    let (gv, pv, nv) = vocabs();
    let mut ui = ui(300, 300);
    let root = mapfallback::minimap(&mut ui, &gv, &nv, &art()).unwrap();
    ui.set_property(root, gv.pids.radar_radius, PropertyValue::Int(81));
    let mut r = Radar::post_create(&mut ui, &gv, root, None);
    let input = RadarInput {
        region: 1,
        x: 10,
        y: 20,
        cell: 0x100,
        global: (1720.0, 3290.0),
        heading: 30.0,
        area_name: Some("Hollow Keep".into()),
        coordinates: Some("20.6N, 10.8E".into()),
        ..RadarInput::default()
    };
    r.update(
        &mut ui,
        &gv,
        &input,
        &mut Indoor(gmap(DUNGEON_IMAGE, (10, 20), (0, 300), 1.25)),
    );
    let mut o = MapOverlays::default();
    o.set_notes(
        &mut ui,
        vec![
            note(1, VENDOR_BIT, (1750.0, 3300.0)),
            note(2, TRAINER_BIT, (1700.0, 3270.0)),
            note(3, VENDOR_BIT, (1900.0, 3450.0)),
            note(4, VENDOR_BIT, (1500.0, 3290.0)),
        ],
    );
    o.add_marker(&mut ui, marker(9, true, (1735.0, 3320.0), "Quest"));
    let table = icons();
    o.update_radar(
        &mut ui,
        &gv,
        &pv,
        &r,
        &input,
        &RadarContext {
            icons: &table,
            filter: (1 << VENDOR_BIT) | (1 << TRAINER_BIT),
            player_height: 0.0,
            instance: 0,
        },
        &DefaultMapTexts,
    );
    // The viewport's canvas (map and notes) with its nested surfaces: the rim notes and
    // the player marker; then the rim and labels of the root's canvas are not part of
    // this image.
    settle(&mut ui);
    let vp = r.viewport().unwrap();
    let mut img = capture(&mut ui, vp);
    let mut kids: Vec<_> = ui.children(vp);
    kids.retain(|k| ui.owns_surface(*k));
    overlay_surfaces(&ui, vp, &mut img, &kids);
    golden("minimap", &img);
}

fn travel_rows() -> Vec<TravelRow> {
    let r = |entry: u16, name: &str, territory: &str, area: &str, coin: u64| TravelRow {
        entry,
        name: name.into(),
        territory: territory.into(),
        area: area.into(),
        coin,
        coins: coin_counts(coin, &VALUES),
        min_level: 1,
        discovered: true,
        met: true,
        usable: true,
        ..TravelRow::default()
    };
    let mut rows = vec![
        r(1, "Bramblegate", "The Westmarch", "Bramble Hills", 0),
        r(2, "Millbrook Crossing", "The Westmarch", "Millbrook", 2500),
        r(
            3,
            "Harrow Point",
            "The Eastern Dales",
            "Harrow Fields",
            12_000,
        ),
        r(4, "Ferry Landing", "", "", 90),
        r(5, "Stonecairn", "The Eastern Dales", "Cairn Downs", 4500),
        r(6, "Old Watchtower", "The Westmarch", "Bramble Hills", 300),
    ];
    rows[1].swift = true;
    rows[1].min_level = 20;
    rows[2].discovered = false;
    rows[4].usable = false;
    rows[4].min_level = 60;
    rows[5].met = false;
    rows
}

#[test]
fn travel_window() {
    let (_, pv, nv) = vocabs();
    let mut ui = ui(800, 800);
    let texts = DefaultTravelTexts;
    let a = art();
    let root = travelfallback::travel(&mut ui, &nv, &pv, &a, &texts).unwrap();
    let mut w = TravelWindow::attach(&mut ui, &nv, &pv, root, TravelTemplates::default(), &a);
    w.show(
        &mut ui,
        &TravelView {
            window: 1,
            title: "Travel with Bram".into(),
            player_level: 40,
            rows: travel_rows(),
        },
        &texts,
    );
    let k = w
        .rows
        .iter()
        .find(|(_, i)| *i == 1)
        .map(|(k, _)| *k)
        .unwrap();
    let e = left(&ui, k);
    w.handle_event(&mut ui, &e, &texts);
    golden("travel", &capture(&mut ui, root));
}

fn finder_entries() -> Vec<FinderEntry> {
    let e = |entry: u16, name: &str, kind: u32, kl: &str, cat: u32, cl: &str| FinderEntry {
        entry,
        template: 0x70F0_0000 + u32::from(entry),
        name: name.into(),
        kind,
        kind_label: kl.into(),
        category: cat,
        category_label: cl.into(),
        sizes: vec![(1, "Solo".into()), (3, "Small".into())],
        tiers: vec![(1, "T1".into()), (2, "T2".into())],
        level_range: Some((20, 140)),
        ..FinderEntry::default()
    };
    let mut v = vec![
        e(1, "The Sunken Archive", 2, "Instances", 13, "Ruins"),
        e(2, "Hall of Echoes", 2, "Instances", 13, "Ruins"),
        e(3, "Bridge Defence", 1, "Skirmishes", 4, "Defence"),
        e(4, "Wardens' Gate (6)", 1, "Skirmishes", 0, "Other"),
        e(5, "Wardens' Gate (12)", 1, "Skirmishes", 0, "Other"),
        e(6, "Frost Festival", 0x10_0000, "Seasonal", 7, "Festivals"),
    ];
    v[3].group = Some((2, "Wardens' Gate".into()));
    v[3].sizes = vec![(6, "Six".into())];
    v[4].group = Some((2, "Wardens' Gate".into()));
    v[4].sizes = vec![(12, "Twelve".into())];
    v[5].locked = true;
    v
}

fn open_finder(ui: &mut tessera_ui::Ui) -> FinderWindow {
    let (_, pv, nv) = vocabs();
    let a = art();
    let t = DefaultFinderTexts;
    let root = travelfallback::finder(ui, &nv, &pv, &a, &t).unwrap();
    let mut w = FinderWindow::attach(ui, &nv, &pv, root, FinderTemplates::default(), &a, &t);
    w.show(
        ui,
        &FinderView {
            window: 2,
            entries: finder_entries(),
            featured: Some(2),
            player_level: 65,
        },
        &t,
    );
    w
}

#[test]
fn instance_finder() {
    let mut ui = ui(1000, 640);
    let mut w = open_finder(&mut ui);
    let t = DefaultFinderTexts;
    let k = w
        .rows
        .iter()
        .find(|(_, r)| *r == 3)
        .map(|(k, _)| *k)
        .unwrap();
    let e = left(&ui, k);
    w.handle_event(&mut ui, &e, &t);
    w.choose_size(&mut ui, 1, &t);
    let root = w.root;
    golden("finder", &capture(&mut ui, root));
}

#[test]
fn instance_finder_queue() {
    let mut ui = ui(1000, 640);
    let mut w = open_finder(&mut ui);
    let t = DefaultFinderTexts;
    w.set_queue(&mut ui, 2, Some(125), 0.0, &t);
    w.tick(&mut ui, 10.0, &t);
    let root = w.root;
    golden("finder_queue", &capture(&mut ui, root));
}
