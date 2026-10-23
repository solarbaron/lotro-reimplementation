// SPDX-License-Identifier: GPL-3.0-or-later
//! Rules of `docs/specs/map-windows.md` on synthetic trees: masks and filters (§6.3, §6.9),
//! the filter menu (MW13, §10.5), placement and the region rule (§6.1, §10.1, §10.2), the
//! dungeon map's stretch (§4.7, §10.3), navigation (§4.2–§4.6, §10.4), the controls
//! (§3.4), the presentations (§3.1), the player marker (§6.6), feed markers (§6.7), the
//! minimap's notes (§6.8), the census helper (§5.4), place lookup (§7.3), and the install
//! layout path through a synthetic `map` layout. All values are invented.

mod p5_common;

use p5_common::ui_common::{array, b, child, did, en, f, field, int, layout, prop, strukt};
use p5_common::*;
use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_ui::game::map::{GameMap, Georef, MapPoint, pixel_in_region};
use tessera_ui::game::mapfallback;
use tessera_ui::game::mapmarks::{
    DefaultMapTexts, FeedMarker, FilterItem, FilterMenuModel, IconTable, LINK_TYPE,
    LOOKUP_MARKER_ID, MapNote, MapOverlays, MapTexts, OverlayId, RadarContext, RadarFilterCommand,
    created_on_map, is_ranged, note_z, radar_filter_command, radar_world_radius, ranged_visible,
    read_filter, type_of_key, visible_under,
};
use tessera_ui::game::mapwindow::{
    LookupTarget, MapAction, MapWindowView, PlayerMap, Presentation, Shown, census_test_point,
};
use tessera_ui::game::radar::{Radar, RadarInput};
use tessera_ui::game::worldmap::{COMMON_NOTE_MASK, MapNode, MapTree, NodeOptions};
use tessera_ui::game::{MapLookup, MapSource};
use tessera_ui::{ElementKey, Ui};

const T: DefaultMapTexts = DefaultMapTexts;

/// The active element of the root (`WorldMap`'s placeholder).
fn world_id() -> u32 {
    tessera_ui::game::P5Vocab::default().ids.world_map
}

const NODE_N: u32 = 0x1000_0201;
const NODE_C: u32 = 0x1000_0202;
const NODE_D: u32 = 0x1000_0203;
const AREA_N: u32 = 0x7000_0001;
const AREA_C: u32 = 0x7000_0002;
const AREA_D: u32 = 0x7000_0003;

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

fn node(
    parent: Option<usize>,
    el: u32,
    map: GameMap,
    region: Option<u32>,
    areas: Vec<u32>,
) -> MapNode {
    MapNode {
        parent,
        depth: 0,
        active_element: el,
        menu_name: None,
        map,
        pixel_size: (1024, 768),
        region,
        areas,
        children: Vec::new(),
    }
}

/// Root `WorldMap` (no region, hidden filter buttons, mask 0), node N (region 1, block
/// (100, 100), pixel (0, 768), scale 0.5, mask bits 37 and 42) with child C (region 1),
/// node D (region 2, prevent zoom out). N and C name the near filter; D the distant one.
fn tree() -> MapTree {
    let pv = tessera_ui::game::P5Vocab::default();
    let mut t = MapTree::default();
    let root_opts = NodeOptions {
        hide_filter_buttons: true,
        quest_guide_disabled: true,
        ..NodeOptions::default()
    };
    t.push(
        node(
            None,
            world_id(),
            gmap(MAP_IMAGE, (0, -600), (0, 768), 0.01),
            None,
            vec![],
        ),
        root_opts,
    );
    let n_opts = NodeOptions {
        display_type: (1 << 42) | (1 << 37) | (1 << 1) | (1 << 47),
        filter_property: pv.pids.gop_map_filter,
        ..NodeOptions::default()
    };
    t.push(
        node(
            Some(0),
            NODE_N,
            gmap(MAP_IMAGE, (100, 100), (0, 768), 0.5),
            Some(1),
            vec![AREA_N],
        ),
        n_opts,
    );
    t.push(
        node(
            Some(1),
            NODE_C,
            gmap(MAP_IMAGE, (100, 100), (0, 768), 1.0),
            Some(1),
            vec![AREA_C],
        ),
        n_opts,
    );
    t.push(
        node(
            Some(0),
            NODE_D,
            gmap(MAP_IMAGE, (10, 10), (0, 768), 0.5),
            Some(2),
            vec![AREA_D],
        ),
        NodeOptions {
            display_type: 1 << 42,
            filter_property: pv.pids.gop_distant_filter,
            prevent_zoom_out: true,
            ..NodeOptions::default()
        },
    );
    t
}

fn open(ui: &mut Ui) -> MapWindowView {
    let (gv, pv, nv) = vocabs();
    let mut v = MapWindowView::open_fallback(ui, &gv, &pv, &nv, tree(), &art(), &T).unwrap();
    v.set_icons(icons());
    v.set_filter_value(pv.pids.gop_map_filter, 1 << 42);
    v
}

fn note(id: u64, bit: u32, global: (f32, f32), region: u8, area: Option<u32>) -> MapNote {
    MapNote {
        id,
        note_type: 1 << bit,
        point: MapPoint {
            global,
            region,
            instance: 0,
        },
        area,
        tooltip: format!("note {id}"),
        text: format!("note {id}"),
        ..MapNote::default()
    }
}

// ----- masks, filters and the menu (§6.3, §6.9) ----------------------------------------

#[test]
fn masks_and_filters() {
    assert_eq!(read_filter(u64::MAX), 0x9B7C_7FFF_FEFF_0002);
    // MWI5: both defaults inside the read mask; the common mask inside the near default.
    assert_eq!(read_filter(0x1B7C_77E7_FEFF_0000), 0x1B7C_77E7_FEFF_0000);
    assert_eq!(read_filter(0x0004_4404_8AE9_0000), 0x0004_4404_8AE9_0000);
    assert_eq!(COMMON_NOTE_MASK & !0x1B7C_77E7_FEFF_0000, 0);
    let o = NodeOptions {
        display_type: 1,
        include_common: true,
        ..NodeOptions::default()
    };
    assert_eq!(o.display_mask(), 1 | COMMON_NOTE_MASK);
    assert!(created_on_map(1 << 5, 0, true));
    assert!(!created_on_map(1 << 5, 1 << 6, false));
    assert!(visible_under(1 << 5, 1 << 5));
    // Link rows: filter bit 1.
    assert!(visible_under(LINK_TYPE, 2));
    assert!(!visible_under(LINK_TYPE, 1));
    assert_eq!(type_of_key(1), Some(1));
    assert_eq!(type_of_key(64), Some(1 << 63));
    assert_eq!(type_of_key(0), None);
    assert_eq!(type_of_key(65), None);
    // MW16 z: leader 2, fellow 1 in front; relative z added.
    assert_eq!(note_z(-1, 0x800_0000, 0), -3);
    assert_eq!(note_z(-1, 0x4_0000_0000, 0), -2);
    assert_eq!(note_z(-1, 1 << 20, 3), 2);
}

struct Labels;
impl MapTexts for Labels {
    fn note_type_label(&self, key: u32) -> String {
        match key {
            38 => "beta".into(),
            43 => "Alpha".into(),
            _ => format!("t{key}"),
        }
    }
}

#[test]
fn filter_menu_example_10_5() {
    // Node mask bits {37, 42} (keys 38, 43); near filter bit 42.
    let mask = (1u64 << 37) | (1 << 42) | 1 | 2 | (1 << 45);
    let mut m = FilterMenuModel::build(mask, 1 << 42, &Labels, &IconTable::default());
    assert_eq!(
        m.items.len(),
        4,
        "all, none, two types (bits 0, 1, 45 unlisted)"
    );
    assert_eq!(m.items[0], FilterItem::All);
    assert_eq!(m.items[1], FilterItem::None);
    // Sorted by label after lower-casing: Alpha (42) before beta (37).
    let bits: Vec<(u32, bool)> = m.items[2..]
        .iter()
        .map(|i| match i {
            FilterItem::Type { bit, checked, .. } => (*bit, *checked),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(bits, vec![(42, true), (37, false)]);
    let toggled = m.select(3, 1 << 42);
    assert_eq!(toggled, 0x0000_0420_0000_0000);
    assert_eq!(m.select(1, toggled), 0, "none clears every listed bit");
    assert_eq!(m.select(0, 1 << 3), (1 << 3) | (1 << 37) | (1 << 42));
    assert_eq!(m.select(99, 7), 7);
}

#[test]
fn radar_filter_commands() {
    let mask = 0x9FFE_7FEF_DFF8_0003;
    assert_eq!(
        radar_filter_command(5, mask, RadarFilterCommand::OnlyOffscreen),
        1 << 45
    );
    assert_eq!(
        radar_filter_command(5, mask, RadarFilterCommand::Reset),
        mask
    );
    assert_eq!(
        radar_filter_command(0b11, mask, RadarFilterCommand::Toggle(1)),
        0
    );
    assert_eq!(
        radar_filter_command(0, mask, RadarFilterCommand::Toggle(20)),
        1 << 20
    );
    assert_eq!(
        radar_filter_command(9, mask, RadarFilterCommand::Toggle(70)),
        9
    );
}

#[test]
fn ranges_and_radius() {
    assert!(is_ranged(80.0, 800.0));
    assert!(!is_ranged(0.0, 800.0));
    assert!(ranged_visible(80.0 * 80.0, 80.0, 800.0));
    assert!(ranged_visible(800.0 * 800.0, 80.0, 800.0));
    assert!(!ranged_visible(79.0 * 79.0, 80.0, 800.0));
    assert_eq!(radar_world_radius(81, 1.25), Some(64.8));
    assert_eq!(radar_world_radius(81, 0.0), None);
}

// ----- placement (§6.1, §10.1, §10.2) --------------------------------------------------

#[test]
fn region_rule_example_10_2() {
    let g = Georef {
        block_x: 100,
        block_y: 100,
        pixel_x: 0,
        pixel_y: 768,
        scale: 0.5,
    };
    let p = MapPoint {
        global: (16_100.0, 16_050.0),
        region: 2,
        instance: 0,
    };
    let (px, _) = pixel_in_region(&g, Some(1), Some(0), &p);
    assert_eq!(px, -950);
    // On a map without a region the point's own region is the origin's.
    assert_eq!(pixel_in_region(&g, None, None, &p), (50, 743));
    // Inverse transform.
    assert_eq!(g.to_world(50, 743), Some((16_100.0, 16_050.0)));
}

#[test]
fn example_10_1_notes_on_a_node() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    let a = v.show_node(&mut ui, 1, &T).unwrap();
    assert_eq!(a, Some(MapAction::Shown(Shown::Node(1))));
    v.set_notes(
        &mut ui,
        vec![
            note(1, 42, (16_100.0, 16_050.0), 1, Some(AREA_N)),
            note(2, 37, (16_200.0, 16_100.0), 1, Some(AREA_N)),
            // In a descendant's area: created.
            note(3, 42, (16_300.0, 16_100.0), 1, Some(AREA_C)),
            // Bit 37 created (in the mask) but bit 42 only is in the filter: hidden.
            // In another node's area: not created.
            note(4, 42, (16_300.0, 16_100.0), 1, Some(AREA_D)),
            // An area of no node maps to the root: not created on N.
            note(5, 42, (16_300.0, 16_100.0), 1, Some(0x7FFF_FFFF)),
        ],
        &T,
    );
    assert_eq!(v.overlay_pixel(&ui, OverlayId::Note(1)), Some((50, 743)));
    let b = v
        .overlays
        .map_element(OverlayId::Note(2))
        .expect("B created");
    assert!(!ui.is_visible(b), "bit 37 is not in the filter");
    assert!(v.overlays.map_element(OverlayId::Note(3)).is_some());
    assert!(v.overlays.map_element(OverlayId::Note(4)).is_none());
    assert!(v.overlays.map_element(OverlayId::Note(5)).is_none());
    // The note is a child of the shown map element, in front of it, z −1.
    let a = v.overlays.map_element(OverlayId::Note(1)).unwrap();
    assert_eq!(ui.parent(a), v.shown_element);
    assert_eq!(ui.z_level(a), Some(-1));
    // §6.9: toggling the filter shows B (real pointer clicks, full-screen presentation:
    // the button's click acts once and the popup lies in front of `MapWindow`).
    assert_eq!(v.presentation, Presentation::FullScreen);
    let fm = v.parts.filter_menu.unwrap();
    assert!(click_view(&mut ui, &mut v, fm).is_empty());
    let popup = v.popup_root().expect("filter popup");
    let rows = v.menu().expect("filter menu").item_elements(0);
    // all, none, then bits by label "Type 38" (37) < "Type 43" (42): row 2 is bit 37.
    let rb = ui.displayed_box(rows[2]);
    let hit = ui.hit_test(rb.l + rb.w() / 2, rb.t + rb.h() / 2).unwrap();
    assert!(
        ui.is_ancestor_or_self(popup, hit),
        "the popup lies in front of MapWindow (z {:?})",
        ui.z_level(v.window)
    );
    let acts = click_view(&mut ui, &mut v, rows[2]);
    let pv = tessera_ui::game::P5Vocab::default();
    assert_eq!(
        acts,
        vec![MapAction::FilterChanged {
            option: pv.pids.gop_map_filter,
            value: (1 << 42) | (1 << 37)
        }]
    );
    assert!(ui.is_visible(b));
    assert_eq!(
        v.filter_value(pv.pids.gop_map_filter),
        (1 << 42) | (1 << 37)
    );
}

#[test]
fn dungeon_maps_create_every_type() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_dungeon(&mut ui, gmap(DUNGEON_IMAGE, (10, 20), (0, 300), 1.0), &T)
        .unwrap();
    assert_eq!(v.display_mask(), u64::MAX);
    v.set_notes(
        &mut ui,
        vec![note(1, 9, (1610.0, 3210.0), 3, Some(0x7FFF_FFFF))],
        &T,
    );
    // Created whatever the mask and the area; placed in image pixels.
    assert_eq!(v.overlay_pixel(&ui, OverlayId::Note(1)), Some((10, 290)));
}

// ----- the dungeon map (§4.7, §10.3) --------------------------------------------------

#[test]
fn example_10_3_dungeon_stretch() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    let m = gmap(TALL_IMAGE, (10, 20), (0, 2400), 2.0);
    v.show_player_map(
        &mut ui,
        &PlayerMap {
            point: MapPoint {
                global: (1700.0, 3300.0),
                region: 1,
                instance: 0,
            },
            valid: true,
            dungeon: Some((77, m)),
            ..PlayerMap::default()
        },
        &T,
    )
    .unwrap();
    assert_eq!(v.shown, Some(Shown::Dungeon));
    let d = v.dungeon.clone().unwrap();
    assert_eq!(d.size, Some((1600, 2400)));
    let s = ui.node_scale_at_or_above(d.image);
    assert_eq!((s.0, s.1), (0.5, 0.25));
    // The marker at map pixel (200, 2200): centred there in image pixels.
    let mk = v.marker.unwrap();
    let bx = ui.layout_box(mk).unwrap();
    assert_eq!((bx.l + bx.w() / 2, bx.t + bx.h() / 2), (200, 2200));
    // On screen at (112 + 100, 84 + 550) of `DungeonMap`.
    let root = ui.displayed_box(d.root);
    let mb = ui.displayed_box(mk);
    let centre = ((mb.l + mb.r) / 2 - root.l, (mb.t + mb.b) / 2 - root.t);
    // The marker's top-left corner maps to (112 + 186 × 0.5, 84 + 2,186 × 0.25); its
    // displayed size follows `ui-behaviour.md` §9.2.1 ((w + 1)·s − 1), so the centre of the
    // displayed box lies within 2 pixels of (212, 634).
    assert_eq!((mb.l - root.l, mb.t - root.t), (205, 630));
    assert!(
        (centre.0 - 212).abs() <= 2 && (centre.1 - 634).abs() <= 2,
        "{centre:?}"
    );
    // The overlay frame is in front of the image field.
    let kids = ui.children(d.root);
    let pos = |k| kids.iter().position(|c| *c == k).unwrap();
    let overlay = part(
        &ui,
        d.root,
        tessera_ui::game::P5Vocab::default().ids.dungeon_overlay,
    );
    assert!(pos(overlay) > pos(d.image));
}

// ----- navigation (§4.2–§4.6, §10.4) -------------------------------------------------

#[test]
fn showing_the_shown_node_does_nothing() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    let k = v.shown_element;
    assert_eq!(v.show_node(&mut ui, 1, &T).unwrap(), None);
    assert_eq!(v.shown_element, k);
    // Draw mode 2 (MW4).
    let (img, mode) = ui.base_image(k.unwrap()).unwrap();
    assert_eq!((img, mode), (tessera_ui::ImageRef::Did(MAP_IMAGE), 2));
}

#[test]
fn example_10_4_going_up() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 2, &T).unwrap();
    let el = v.shown_element.unwrap();
    let acts = {
        let e = right(&ui, el);
        v.handle_event(&mut ui, &e, &T)
    };
    assert_eq!(acts, vec![MapAction::Shown(Shown::Node(1))]);
    let el = v.shown_element.unwrap();
    {
        let e = right(&ui, el);
        v.handle_event(&mut ui, &e, &T)
    };
    assert_eq!(v.shown, Some(Shown::Node(0)));
    // At the root: nothing (Tessera rule).
    let el = v.shown_element.unwrap();
    assert!(
        {
            let e = right(&ui, el);
            v.handle_event(&mut ui, &e, &T)
        }
        .is_empty()
    );
    assert_eq!(v.shown, Some(Shown::Node(0)));
    // UI_Map_PreventZoomOut: nothing.
    v.show_node(&mut ui, 3, &T).unwrap();
    assert_eq!(v.go_up(&mut ui, &T).unwrap(), None);
    assert_eq!(v.shown, Some(Shown::Node(3)));
    // A dungeon map with the player in a dungeon: the player's outdoor map.
    let player = PlayerMap {
        point: MapPoint {
            global: (16_100.0, 16_050.0),
            region: 1,
            instance: 0,
        },
        valid: true,
        area: Some(AREA_C),
        dungeon: Some((5, gmap(DUNGEON_IMAGE, (100, 100), (0, 300), 1.0))),
        ..PlayerMap::default()
    };
    v.show_player_map(&mut ui, &player, &T).unwrap();
    assert_eq!(v.shown, Some(Shown::Dungeon));
    v.go_up(&mut ui, &T).unwrap();
    assert_eq!(v.shown, Some(Shown::Node(2)));
}

#[test]
fn player_map_unlisted_area_shows_world_map() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    let mut p = PlayerMap {
        point: MapPoint {
            global: (16_100.0, 16_050.0),
            region: 1,
            instance: 0,
        },
        valid: true,
        area: Some(0x7FFF_0000),
        ..PlayerMap::default()
    };
    v.show_player_map(&mut ui, &p, &T).unwrap();
    assert_eq!(v.shown, Some(Shown::Node(0)), "MW9: no region fallback");
    p.area = Some(AREA_N);
    v.show_player_map(&mut ui, &p, &T).unwrap();
    assert_eq!(v.shown, Some(Shown::Node(1)));
    // An invalid position changes nothing.
    p.valid = false;
    p.area = Some(AREA_D);
    assert_eq!(v.show_player_map(&mut ui, &p, &T).unwrap(), None);
    assert_eq!(v.shown, Some(Shown::Node(1)));
}

#[test]
fn controls_follow_section_3_4() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 0, &T).unwrap();
    let p = v.parts;
    let vis = |ui: &Ui, k: Option<ElementKey>| ui.is_visible(k.unwrap());
    assert!(!vis(&ui, p.filter_menu));
    assert!(!vis(&ui, p.map_menu));
    assert!(!vis(&ui, p.coordinates));
    assert!(!vis(&ui, p.stablemaster));
    // The root's element carries no HidePlayerMapnote here; the marker exists but the
    // player is unknown: hidden.
    v.show_node(&mut ui, 1, &T).unwrap();
    assert!(vis(&ui, p.filter_menu));
    assert!(vis(&ui, p.map_menu));
    assert!(vis(&ui, p.coordinates));
    assert!(!vis(&ui, p.stablemaster));
    assert!(!vis(&ui, p.quest_guide));
    v.set_show_coordinates(&mut ui, false);
    assert!(!vis(&ui, p.coordinates));
}

#[test]
fn presentations_move_map_field() {
    let mut ui = ui(1100, 900);
    let mut v = open(&mut ui);
    let field = v.parts.field.unwrap();
    assert_eq!(ui.parent(field), Some(v.window));
    let pr = v.windowed_root.unwrap();
    let collapse = v.parts.collapse.unwrap();
    // A real pointer click (Button_click and Element_mouse_click) switches once.
    let acts = click_view(&mut ui, &mut v, collapse);
    assert_eq!(
        acts,
        vec![MapAction::PresentationChanged { full_screen: false }]
    );
    assert_eq!(v.presentation, Presentation::Windowed);
    assert_eq!(ui.parent(field), v.parts.padding);
    assert!(ui.is_visible(pr) && !ui.is_visible(v.window));
    assert_eq!(v.panel.root, pr);
    let expand = v.parts.expand.unwrap();
    assert_eq!(
        click_view(&mut ui, &mut v, expand),
        vec![MapAction::PresentationChanged { full_screen: true }]
    );
    assert_eq!(ui.parent(field), Some(v.window));
    // Close.
    let pv = tessera_ui::game::P5Vocab::default();
    let close = part(&ui, v.window, pv.ids.toggle_map_panel);
    assert_eq!(click_view(&mut ui, &mut v, close), vec![MapAction::Close]);
}

#[test]
fn map_menu_of_install_size_lies_on_the_screen() {
    // About the install's largest region (74 named nodes, MW8) on a 1,024 × 768 screen.
    let mut ui = ui(1024, 768);
    let (gv, pv, nv) = vocabs();
    let mut t = tree();
    for i in 0..74u32 {
        t.push(
            node(
                Some(0),
                0x1000_0300 + i,
                gmap(MAP_IMAGE, (100, 100), (0, 768), 0.5),
                Some(1),
                vec![],
            ),
            NodeOptions::default(),
        );
    }
    let n = t.nodes.len();
    let mut v = MapWindowView::open_fallback(&mut ui, &gv, &pv, &nv, t, &art(), &T).unwrap();
    let names: Vec<Option<String>> = (0..n)
        .map(|i| (i >= 4).then(|| format!("Place {i:03}")))
        .collect();
    let regions = [(1, "A"), (2, "B"), (3, "C"), (4, "D"), (14, "E"), (5, "F")];
    v.set_menu(
        regions
            .iter()
            .map(|(r, s)| (*r, (*s).to_string()))
            .collect(),
        names,
    );
    v.show_node(&mut ui, 1, &T).unwrap();
    let mm = v.parts.map_menu.unwrap();
    click_view(&mut ui, &mut v, mm);
    assert!(v.popup_root().is_some(), "map menu");
    let first = v.menu().expect("map menu").item_elements(0)[0];
    click_view(&mut ui, &mut v, first);
    let sub = v.submenu_root().expect("submenu");
    settle(&mut ui);
    // `ui-widgets.md` §19.3: the submenu's list is at most 0.8 × the screen height and
    // scrolls; the submenu lies on the screen.
    let rows = v.menu().expect("map menu").item_elements(1);
    assert_eq!(rows.len(), 74);
    let b = ui.displayed_box(sub);
    assert!(
        b.l >= 0 && b.t >= 0 && b.l + b.w() <= 1024 && b.t + b.h() <= 768,
        "submenu {b:?} off the screen"
    );
    assert!(b.h() <= 768 * 4 / 5 + 4, "list capped: {b:?}");
}

/// A real pointer click on `k` handed to the view: every action of every posted event.
fn click_view(ui: &mut Ui, v: &mut MapWindowView, k: ElementKey) -> Vec<MapAction> {
    let evs = real_click(ui, k);
    let mut out = Vec::new();
    for e in &evs {
        out.extend(v.handle_event(ui, e, &T));
    }
    out
}

fn row_texts(ui: &mut Ui, rows: &[ElementKey]) -> Vec<String> {
    let id = tessera_ui::game::context_menu::MenuIds::default().text;
    rows.iter()
        .map(|r| {
            let t = tessera_ui::game::find_below(ui, *r, id).unwrap();
            ui.text_of(t)
        })
        .collect()
}

#[test]
fn map_menu_lists_regions_and_sorted_names() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.set_menu(
        vec![(2, "South".into()), (1, "North".into())],
        vec![
            None,
            Some("zeta".into()),
            Some("Alpha".into()),
            Some("Delta".into()),
        ],
    );
    v.show_node(&mut ui, 1, &T).unwrap();
    let mm = v.parts.map_menu.unwrap();
    // One real click opens the menu (and does not close it again at once).
    assert!(click_view(&mut ui, &mut v, mm).is_empty());
    assert!(v.popup_root().is_some(), "map menu open");
    let rows = v.menu().unwrap().item_elements(0);
    assert_eq!(row_texts(&mut ui, &rows), vec!["South", "North"]);
    // A region opens its submenu beside its row, sorted by label.
    assert!(click_view(&mut ui, &mut v, rows[1]).is_empty());
    let sub = v.submenu_root().expect("submenu");
    let sub_rows = v.menu().unwrap().item_elements(1);
    assert_eq!(row_texts(&mut ui, &sub_rows), vec!["Alpha", "zeta"]);
    let (r1, s) = (ui.displayed_box(rows[1]), ui.displayed_box(sub));
    assert!(s.l >= r1.l + r1.w() - 4, "beside its row");
    // Another region replaces it.
    assert!(click_view(&mut ui, &mut v, rows[0]).is_empty());
    assert!(v.submenu_root().is_some(), "submenu");
    let sub_rows = v.menu().unwrap().item_elements(1);
    assert_eq!(row_texts(&mut ui, &sub_rows), vec!["Delta"]);
    // An item shows its node and closes both menus.
    let item = sub_rows[0];
    let acts = click_view(&mut ui, &mut v, item);
    assert_eq!(acts, vec![MapAction::Shown(Shown::Node(3))]);
    assert!(!v.popup_open());
    assert!(v.submenu_root().is_none());
    // Right after the menu closed a click on the button does not reopen it (§4.5); later
    // it does, and a second click on the button closes the open menu.
    click_view(&mut ui, &mut v, mm);
    assert!(!v.popup_open(), "within the reopen guard");
    ui.set_time(ui.time() + 1.0);
    click_view(&mut ui, &mut v, mm);
    assert!(v.popup_open());
    click_view(&mut ui, &mut v, mm);
    assert!(!v.popup_open());
}

// ----- the player marker (§6.6) -------------------------------------------------------

#[test]
fn player_marker_centred_and_hidden_at_the_edge() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    let mut p = PlayerMap {
        point: MapPoint {
            global: (16_100.0, 16_050.0),
            region: 1,
            instance: 0,
        },
        heading: 0.0,
        valid: true,
        ..PlayerMap::default()
    };
    v.update_player(&mut ui, &p);
    let m = v.marker.unwrap();
    assert!(ui.is_visible(m));
    let b = ui.layout_box(m).unwrap();
    assert_eq!((b.l + 14, b.t + 14), (50, 743));
    assert_eq!(ui.z_level(m), Some(-500));
    // 2 pixels from the bottom: its box would leave the frame.
    p.point.global = (16_100.0, 16_000.0 + 0.0);
    v.update_player(&mut ui, &p);
    assert!(!ui.is_visible(m));
    // Another region: far off, hidden.
    p.point.global = (16_100.0, 16_050.0);
    p.point.region = 2;
    v.update_player(&mut ui, &p);
    assert!(!ui.is_visible(m));
}

// ----- feed markers (§6.7) -----------------------------------------------------------

fn marker(id: u32, quest: bool, global: (f32, f32), region: u8) -> FeedMarker {
    FeedMarker {
        id,
        point: MapPoint {
            global,
            region,
            instance: 0,
        },
        label: format!("marker {id}"),
        quest,
        ..FeedMarker::default()
    }
}

#[test]
fn feed_markers_icons_order_and_updates() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    v.set_notes(
        &mut ui,
        vec![note(1, 42, (16_100.0, 16_050.0), 1, Some(AREA_N))],
        &T,
    );
    v.add_marker(&mut ui, marker(10, true, (16_200.0, 16_100.0), 1), &T);
    v.add_marker(&mut ui, marker(11, false, (16_300.0, 16_100.0), 1), &T);
    let mut typed = marker(12, false, (16_400.0, 16_100.0), 1);
    typed.note_type = Some(VENDOR_BIT + 1);
    v.add_marker(&mut ui, typed, &T);
    let mut own = marker(13, true, (16_500.0, 16_100.0), 1);
    own.icon = Some(art_ref(33));
    v.add_marker(&mut ui, own, &T);
    v.add_marker(&mut ui, marker(14, false, (16_100.0, 16_050.0), 2), &T);
    let icon = |ui: &Ui, id: u32| {
        let k = v.overlays.map_element(OverlayId::Marker(id)).unwrap();
        let b = ui.children(k)[0];
        ui.base_image(b).map(|(i, _)| i)
    };
    assert_eq!(icon(&ui, 10), Some(art_ref(26)), "quest: type bit 1");
    assert_eq!(icon(&ui, 11), Some(art_ref(28)), "plain: type bit 47");
    assert_eq!(icon(&ui, 12), Some(art_ref(29)), "note-type key 21");
    assert_eq!(icon(&ui, 13), Some(art_ref(33)), "own icon");
    assert!(
        v.overlays.map_element(OverlayId::Marker(14)).is_none(),
        "another region"
    );
    // Not filtered: the filter has no bit 47.
    let k11 = v.overlays.map_element(OverlayId::Marker(11)).unwrap();
    assert!(ui.is_visible(k11));
    // Markers after the notes in read order (in front), z −1.
    let parent = v.shown_element.unwrap();
    let kids = ui.children(parent);
    let pos = |k| kids.iter().position(|c| *c == k).unwrap();
    let n1 = v.overlays.map_element(OverlayId::Note(1)).unwrap();
    assert!(pos(k11) > pos(n1));
    assert_eq!(ui.z_level(k11), Some(-1));
    // The marker is in front of the notes and behind the player marker.
    let m = v.marker.unwrap();
    assert!(pos(m) > pos(k11));
    // Replace, then remove; markers persist across map changes.
    v.add_marker(&mut ui, marker(11, false, (16_600.0, 16_100.0), 1), &T);
    assert_eq!(
        v.overlay_pixel(&ui, OverlayId::Marker(11)),
        Some((300, 718))
    );
    v.show_node(&mut ui, 2, &T).unwrap();
    assert!(v.overlays.map_element(OverlayId::Marker(11)).is_some());
    assert!(v.remove_marker(&mut ui, 11, &T));
    assert!(v.overlays.map_element(OverlayId::Marker(11)).is_none());
    // Tooltip: label, then coordinates when the client gives them.
    struct Coords;
    impl MapTexts for Coords {
        fn coordinates(&self, _: &MapPoint) -> Option<String> {
            Some("1.0S, 2.0W".into())
        }
    }
    let m10 = v.overlays.markers()[&10].clone();
    assert_eq!(
        tessera_ui::game::mapmarks::marker_tooltip(&Coords, &m10),
        "marker 10\n1.0S, 2.0W"
    );
}

#[test]
fn place_lookup_marker() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    let pt = MapPoint {
        global: (16_100.0, 16_050.0),
        region: 1,
        instance: 0,
    };
    v.show_lookup(
        &mut ui,
        LookupTarget::Area(Some(AREA_N)),
        Some((pt, "Here".into())),
        &T,
    )
    .unwrap();
    assert_eq!(v.shown, Some(Shown::Node(1)));
    assert_eq!(
        v.overlay_pixel(&ui, OverlayId::Marker(LOOKUP_MARKER_ID)),
        Some((50, 743))
    );
    // One at a time.
    v.show_lookup(
        &mut ui,
        LookupTarget::Node(2),
        Some((pt, "There".into())),
        &T,
    )
    .unwrap();
    assert_eq!(v.overlays.markers().len(), 1);
    v.clear_lookup(&mut ui, &T);
    assert!(v.overlays.markers().is_empty());
}

#[test]
fn coordinate_display_follows_the_pointer() {
    struct Coords;
    impl MapTexts for Coords {
        fn coordinate_display(&self, p: &MapPoint) -> Option<String> {
            Some(format!("{:.0},{:.0} r{}", p.global.0, p.global.1, p.region))
        }
    }
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    settle(&mut ui);
    let d = ui.displayed_box(v.shown_element.unwrap());
    v.pointer_moved(&mut ui, d.l + 50, d.t + 743, &Coords);
    let cd = v.parts.coordinates.unwrap();
    assert_eq!(ui.text_of(cd), "16100,16050 r1");
    // Outside the map: unchanged.
    v.pointer_moved(&mut ui, -10, -10, &Coords);
    assert_eq!(ui.text_of(cd), "16100,16050 r1");
}

// ----- the census helper (§5.4) ------------------------------------------------------

#[test]
fn census_test_points() {
    let g = Georef {
        block_x: 100,
        block_y: 100,
        pixel_x: 0,
        pixel_y: 768,
        scale: 0.5,
    };
    let p = census_test_point(&g, (1024, 768)).unwrap();
    assert_eq!(p, (16_000.0 + 1024.0, 16_000.0 + 768.0));
    // C5: the frame lies below the world.
    let far = Georef {
        block_x: 0,
        block_y: -600,
        pixel_x: 0,
        pixel_y: 768,
        scale: 0.01,
    };
    assert_eq!(census_test_point(&far, (1024, 768)), None);
    let zero = Georef { scale: 0.0, ..g };
    assert_eq!(census_test_point(&zero, (1024, 768)), None);
}

// ----- the minimap (§6.8) -----------------------------------------------------------

struct Dungeon(GameMap);
impl MapSource for Dungeon {
    fn landblock_map(&mut self, _: u8, _: u8, _: u8) -> MapLookup {
        MapLookup::Ready(None)
    }
    fn dungeon_map(&mut self, _: u8, _: u8, _: u8, _: u16) -> MapLookup {
        MapLookup::Ready(Some(self.0.clone()))
    }
}

fn radar_setup(ui: &mut Ui) -> (Radar, RadarInput) {
    let (gv, _, nv) = vocabs();
    let root = mapfallback::minimap(ui, &gv, &nv, &art()).unwrap();
    ui.set_property(root, gv.pids.radar_radius, PropertyValue::Int(81));
    let mut r = Radar::post_create(ui, &gv, root, None);
    let input = RadarInput {
        region: 1,
        x: 10,
        y: 20,
        cell: 0x100,
        global: (1700.0, 3300.0),
        ..RadarInput::default()
    };
    let mut maps = Dungeon(gmap(DUNGEON_IMAGE, (10, 20), (0, 300), 1.25));
    r.update(ui, &gv, &input, &mut maps);
    (r, input)
}

#[test]
fn minimap_notes_inside_and_outside_the_radius() {
    let mut ui = ui(400, 400);
    let (gv, pv, _) = vocabs();
    let (radar, input) = radar_setup(&mut ui);
    let mut o = MapOverlays::default();
    let mut notes = vec![
        // Inside: 40 units east.
        note(1, VENDOR_BIT, (1740.0, 3300.0), 1, None),
        // Outside, with an off-screen image: on the rim to the north.
        note(2, VENDOR_BIT, (1700.0, 3500.0), 1, None),
        // Outside, no off-screen image: not drawn.
        note(3, TRAINER_BIT, (1700.0, 3500.0), 1, None),
        // Filtered out.
        note(4, 30, (1710.0, 3300.0), 1, None),
        // Another region.
        note(5, VENDOR_BIT, (1710.0, 3300.0), 2, None),
    ];
    let mut link = note(6, 0, (1705.0, 3300.0), 1, None);
    link.note_type = LINK_TYPE | (1 << VENDOR_BIT);
    notes.push(link);
    o.set_notes(&mut ui, notes);
    o.add_marker(&mut ui, marker(20, false, (1720.0, 3300.0), 1));
    o.add_marker(&mut ui, marker(21, true, (1700.0, 3500.0), 1));
    let table = icons();
    let ctx = RadarContext {
        icons: &table,
        filter: (1 << VENDOR_BIT) | (1 << TRAINER_BIT),
        player_height: 0.0,
        instance: 0,
    };
    o.update_radar(&mut ui, &gv, &pv, &radar, &input, &ctx, &T);
    let shown = |id| o.radar_element(radar.key, id).filter(|k| ui.is_visible(*k));
    let k1 = shown(OverlayId::Note(1)).expect("inside");
    let b1 = ui.layout_box(k1).unwrap();
    // 40 units × 1.25 = 50 pixels east of the centre (85, 85).
    assert_eq!((b1.l + b1.w() / 2, b1.t + b1.h() / 2), (135, 85));
    let k2 = shown(OverlayId::Note(2)).expect("rim");
    let b2 = ui.layout_box(k2).unwrap();
    assert_eq!((b2.l + b2.w() / 2, b2.t + b2.h() / 2), (85, 4));
    let btn = ui.children(k2)[0];
    assert_eq!(ui.base_image(btn).map(|(i, _)| i), Some(art_ref(31)));
    assert!(shown(OverlayId::Note(3)).is_none());
    assert!(shown(OverlayId::Note(4)).is_none());
    assert!(shown(OverlayId::Note(5)).is_none());
    assert!(
        shown(OverlayId::Note(6)).is_none(),
        "link rows are not drawn"
    );
    assert!(shown(OverlayId::Marker(20)).is_some());
    assert!(
        shown(OverlayId::Marker(21)).is_none(),
        "feed markers: inside only"
    );
    // Behind the player marker.
    let vp = radar.viewport().unwrap();
    let kids = ui.children(vp);
    assert_eq!(kids.last().copied(), radar.player_marker());
}

// ----- the install path through a synthetic `map` layout ---------------------------------

const MAP_LAYOUT: u32 = 0x22FF_5000;

/// A `map` layout: `MapWindow` with the tree, `MapField`, `MapBackground`, the menus and
/// the coordinate display; `MapPanel`; two active elements (N with a child-map button to
/// C, and a field naming a top-level element that is no node); `MapNoteTemplate`,
/// `MapPlayerMarker`, `DungeonMap`; and an extra top-level element.
fn map_layout() -> tessera_formats::ui_layout::UiLayout {
    let (gv, pv, _) = vocabs();
    let (g, p) = (gv.pids, pv.pids);
    let ids = pv.ids;
    let node_struct = |el: u32, region: Option<u32>, area: u32, kids: Vec<PropertyValue>| {
        let mut fields = vec![
            (g.map_active_element, en(el)),
            (g.map_image, did(MAP_IMAGE)),
            (g.map_block_x, int(100)),
            (g.map_block_y, int(100)),
            (g.map_pixel_x, int(0)),
            (g.map_pixel_y, int(768)),
            (g.map_scale, f(0.5)),
            (g.map_pixel_width, int(1024)),
            (g.map_pixel_height, int(768)),
            (g.map_area_dids, array(g.map_area_dids, vec![did(area)])),
            (p.map_display_type, PropertyValue::Bitfield64(1 << 42)),
            (
                p.map_filter_property,
                PropertyValue::PropertyId(p.gop_map_filter),
            ),
        ];
        if let Some(r) = region {
            fields.push((g.map_region, en(r)));
        } else {
            fields.push((p.map_hide_filter_buttons, b(true)));
        }
        if !kids.is_empty() {
            fields.push((g.map_children, array(g.map_children, kids)));
        }
        strukt(fields)
    };
    let c = node_struct(NODE_C, Some(1), AREA_C, vec![]);
    let n = node_struct(NODE_N, Some(1), AREA_N, vec![c]);
    let root = node_struct(world_id(), None, 0x7000_0009, vec![n]);
    let window = prop(
        field(gv.ids.map_window, 0, 0, 1024, 768),
        g.map_area_data,
        root,
    );
    let background = prop(
        child(
            field(gv.ids.map_background, 0, 0, 1024, 768),
            field(ids.coordinate_display, 52, 730, 450, 25),
        ),
        tessera_ui::Vocab::default().pids.object_mode,
        en(1),
    );
    let menus = child(
        child(
            field(ids.menu_scale_parent, 52, 18, 800, 25),
            field(ids.filter_menu, 0, 0, 150, 25),
        ),
        field(ids.map_menu, 151, 0, 150, 25),
    );
    let mfield = child(
        child(field(ids.map_field, 0, 0, 1024, 768), background),
        menus,
    );
    let window = child(window, mfield);
    let panel = child(
        field(ids.map_panel, 0, 0, 1038, 802),
        field(gv.ids.map_panel_padding, 5, 26, 1024, 768),
    );
    let node_n = child(
        field(NODE_N, 0, 0, 1024, 768),
        prop(
            field(0x1000_0300, 10, 10, 60, 30),
            g.map_child_map,
            en(NODE_C),
        ),
    );
    let node_n = child(
        node_n,
        prop(
            field(0x1000_0301, 100, 10, 60, 30),
            g.map_child_map,
            en(0x1000_0400),
        ),
    );
    let node_c = prop(
        field(NODE_C, 0, 0, 1024, 768),
        p.map_hide_player_note,
        b(true),
    );
    let world = field(world_id(), 0, 0, 1024, 768);
    let other = field(0x1000_0400, 0, 0, 1024, 768);
    let note_t = child(
        field(ids.note_template, 0, 0, 30, 30),
        field(ids.note_button, 0, 0, 30, 30),
    );
    let marker = field(ids.player_marker, 0, 0, 28, 28);
    let dungeon = child(
        child(
            field(ids.dungeon_map, 0, 0, 1024, 768),
            prop(
                field(ids.dungeon_image, 112, 84, 800, 600),
                tessera_ui::Vocab::default().pids.object_mode,
                en(1),
            ),
        ),
        field(ids.dungeon_overlay, 0, 0, 1024, 768),
    );
    layout(
        MAP_LAYOUT,
        1024,
        768,
        vec![
            window, panel, node_n, node_c, world, other, note_t, marker, dungeon,
        ],
    )
}

#[test]
fn install_layout_path() {
    let mut ui = ui_layouts(1280, 960, vec![map_layout()]);
    let (gv, pv, _) = vocabs();
    let mut v = MapWindowView::open_layout(&mut ui, &gv, &pv, Did(MAP_LAYOUT), &art()).unwrap();
    let t = v.tree().unwrap();
    assert_eq!(t.nodes.len(), 3);
    assert_eq!(t.options(1).display_mask(), 1 << 42);
    assert!(t.options(0).hide_filter_buttons);
    assert_eq!(t.options(1).filter_property, pv.pids.gop_map_filter);
    assert!(!ui.is_visible(v.windowed_root.unwrap()));
    // Showing N from the layout; its child-map button shows C.
    v.show_node(&mut ui, 1, &T).unwrap();
    let k = v.shown_element.unwrap();
    assert_eq!(ui.element_id(k), Some(NODE_N));
    let btn = part(&ui, k, 0x1000_0300);
    let acts = {
        let e = left(&ui, btn);
        v.handle_event(&mut ui, &e, &T)
    };
    assert_eq!(acts, vec![MapAction::Shown(Shown::Node(2))]);
    // C carries UI_Map_HidePlayerMapnote: no marker.
    assert!(v.marker.is_none());
    // Back to N; the other button names an element that is no node (§4.2 step 3).
    v.show_node(&mut ui, 1, &T).unwrap();
    let k = v.shown_element.unwrap();
    let btn = part(&ui, k, 0x1000_0301);
    let acts = {
        let e = double(&ui, btn);
        v.handle_event(&mut ui, &e, &T)
    };
    assert_eq!(acts, vec![MapAction::Shown(Shown::Element(0x1000_0400))]);
    assert!(v.marker.is_none());
    assert!(v.context().is_none());
    // The note template's shape comes from the layout.
    assert_eq!(v.note_shape.size, (30, 30));
    // Notes on the layout's map use the template instance shape.
    v.show_node(&mut ui, 1, &T).unwrap();
    v.set_notes(
        &mut ui,
        vec![note(1, 42, (16_100.0, 16_050.0), 1, Some(AREA_N))],
        &T,
    );
    assert_eq!(v.overlay_pixel(&ui, OverlayId::Note(1)), Some((50, 743)));
    v.detach(&mut ui);
}

#[test]
fn hostile_trees_do_not_panic() {
    // A tree whose nodes have no georeference, zero scale and odd frames.
    let mut t = MapTree::default();
    let mut n = node(None, 0x1000_0901, GameMap::default(), None, vec![]);
    n.pixel_size = (0, -5);
    t.push(n, NodeOptions::default());
    t.push(
        node(
            Some(0),
            0x1000_0902,
            gmap(0, (0, 0), (0, 0), 0.0),
            Some(1),
            vec![],
        ),
        NodeOptions {
            display_type: u64::MAX,
            ..NodeOptions::default()
        },
    );
    let (gv, pv, nv) = vocabs();
    let mut ui = ui(1024, 768);
    let mut v = MapWindowView::open_fallback(&mut ui, &gv, &pv, &nv, t, &art(), &T).unwrap();
    for i in [0, 1, 5] {
        let _ = v.show_node(&mut ui, i, &T);
        v.set_notes(
            &mut ui,
            vec![note(1, 3, (f32::NAN, f32::INFINITY), 0, None)],
            &T,
        );
        v.add_marker(&mut ui, marker(1, true, (1e30, -1e30), 0), &T);
        v.update_player(
            &mut ui,
            &PlayerMap {
                valid: true,
                point: MapPoint {
                    global: (f32::NAN, 0.0),
                    region: 1,
                    instance: 0,
                },
                heading: f32::NAN,
                ..PlayerMap::default()
            },
        );
        let _ = v.go_up(&mut ui, &T);
    }
    let _ = v.show_dungeon(&mut ui, GameMap::default(), &T);
    let _ = v.show_element(&mut ui, 0x1234);
    v.pointer_moved(&mut ui, 5, 5, &T);
    settle(&mut ui);
}

#[test]
fn p3_quest_markers_still_draw_on_the_map_window() {
    use tessera_ui::game::quest::QuestKey;
    use tessera_ui::game::quest::markers::{NoteImages, QuestMarker, QuestMarkers};
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    let (gv, _, _) = vocabs();
    let mut q = QuestMarkers::default();
    q.add(QuestMarker {
        id: 3,
        quest: QuestKey::default(),
        global: (16_100.0, 16_050.0),
        icon: None,
        label: Some("Quest".into()),
    });
    let images = NoteImages {
        normal: Some(art_ref(26)),
        highlight: Some(art_ref(27)),
    };
    q.update_world_map(&mut ui, &gv, &v.panel, &images).unwrap();
    let k = q.map_element(3).expect("the P3 marker is placed");
    let b = ui.layout_box(k).unwrap();
    assert_eq!((b.l + b.w() / 2, b.t + b.h() / 2), (50, 743));
}

// ----- review fixes: incremental markers, repeated note ids, the radar's instance -----

#[test]
fn markers_are_placed_without_replacing_the_notes() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    let notes: Vec<MapNote> = (1..=20)
        .map(|i| note(i, 42, (16_100.0 + i as f32, 16_050.0), 1, Some(AREA_N)))
        .collect();
    v.set_notes(&mut ui, notes, &T);
    let note_keys = |v: &MapWindowView| -> Vec<ElementKey> {
        (1..=20)
            .map(|i| v.overlays.map_element(OverlayId::Note(i)).unwrap())
            .collect()
    };
    let before = note_keys(&v);
    // Out of id order: the elements still end up in id order, in front of the notes.
    for id in [30, 10, 20] {
        v.add_marker(&mut ui, marker(id, false, (16_200.0, 16_100.0), 1), &T);
    }
    assert_eq!(note_keys(&v), before, "notes are not re-created");
    let parent = v.shown_element.unwrap();
    let kids = ui.children(parent);
    let pos = |id| {
        let k = v.overlays.map_element(OverlayId::Marker(id)).unwrap();
        kids.iter().position(|c| *c == k).unwrap()
    };
    assert!(pos(10) < pos(20) && pos(20) < pos(30));
    let last_note = before
        .iter()
        .map(|k| kids.iter().position(|c| c == k).unwrap())
        .max()
        .unwrap();
    assert!(pos(10) > last_note);
    // Removal: only that marker's element goes.
    let k30 = v.overlays.map_element(OverlayId::Marker(30)).unwrap();
    assert!(v.remove_marker(&mut ui, 20, &T));
    assert_eq!(note_keys(&v), before);
    assert_eq!(v.overlays.map_element(OverlayId::Marker(30)), Some(k30));
    assert!(v.overlays.map_element(OverlayId::Marker(20)).is_none());
    assert_eq!(ui.children(parent).len(), kids.len() - 1);
}

#[test]
fn repeated_note_ids_do_not_leak_elements() {
    let mut ui = ui(1024, 768);
    let mut v = open(&mut ui);
    v.show_node(&mut ui, 1, &T).unwrap();
    let parent = v.shown_element.unwrap();
    let base = ui.children(parent).len();
    v.set_notes(
        &mut ui,
        vec![
            note(7, 42, (16_100.0, 16_050.0), 1, Some(AREA_N)),
            note(7, 42, (16_150.0, 16_050.0), 1, Some(AREA_N)),
            note(8, 42, (16_120.0, 16_050.0), 1, Some(AREA_N)),
        ],
        &T,
    );
    assert_eq!(v.overlays.notes().len(), 2, "the first of id 7 is kept");
    for id in 0..5 {
        v.add_marker(&mut ui, marker(id, false, (16_200.0, 16_100.0), 1), &T);
    }
    v.refresh(&mut ui, &T);
    assert_eq!(ui.children(parent).len(), base + 2 + 5);
}

#[test]
fn minimap_draws_the_players_instance() {
    let mut ui = ui(400, 400);
    let (gv, pv, _) = vocabs();
    let (radar, input) = radar_setup(&mut ui);
    let mut o = MapOverlays::default();
    let mut n = note(1, VENDOR_BIT, (1740.0, 3300.0), 1, None);
    n.point.instance = 5;
    o.set_notes(&mut ui, vec![n]);
    let mut m = marker(20, false, (1720.0, 3300.0), 1);
    m.point.instance = 5;
    o.add_marker(&mut ui, m);
    let table = icons();
    let mut ctx = RadarContext {
        icons: &table,
        filter: 1 << VENDOR_BIT,
        player_height: 0.0,
        instance: 5,
    };
    o.update_radar(&mut ui, &gv, &pv, &radar, &input, &ctx, &T);
    let shown = |ui: &Ui, o: &MapOverlays, id| {
        o.radar_element(radar.key, id)
            .is_some_and(|k| ui.is_visible(k))
    };
    assert!(shown(&ui, &o, OverlayId::Note(1)), "same instance");
    assert!(shown(&ui, &o, OverlayId::Marker(20)), "same instance");
    ctx.instance = 0;
    o.update_radar(&mut ui, &gv, &pv, &radar, &input, &ctx, &T);
    assert!(!shown(&ui, &o, OverlayId::Note(1)), "another instance");
    assert!(!shown(&ui, &o, OverlayId::Marker(20)), "another instance");
    // Updates keep the notes and markers.
    assert_eq!(o.notes().len(), 1);
    assert_eq!(o.markers().len(), 1);
}
