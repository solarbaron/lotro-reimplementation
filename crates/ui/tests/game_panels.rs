// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples (§13) and behaviour invariants (§12) of `docs/specs/game-panels.md` on
//! synthetic trees. All values are invented; element IDs, types, states and properties use
//! the placeholder values of `GameVocab::default()`.

mod ui_common;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::ElementDesc;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::escape::{EscapeOutcome, EscapePanels, declined_escape};
use tessera_ui::game::hidewhen::{compare, compare_with_player};
use tessera_ui::game::map::{GameMap, Georef, grid_blocks, tile_range};
use tessera_ui::game::radar::{Radar, RadarInput};
use tessera_ui::game::simple::{mood_index, mood_state, time_of_day_state};
use tessera_ui::game::worldmap::{MAX_DEPTH, MapShown, MapTree};
use tessera_ui::game::{
    GameAction, GameData, GameError, GameLayouts, GamePanels, GameVocab, HideCondition, MapLookup,
    MapSource, NoShortcuts, SandboxData, Shortcut, Shortcuts,
};
use tessera_ui::{ElementKey, Ui, UiEvent};
use ui_common::*;

fn gv() -> GameVocab {
    GameVocab::default()
}

fn arr(elems: Vec<(u32, PropertyValue)>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: elems.len() as u32,
        elements: elems
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
    })
}

fn msg(ui: &Ui, source: ElementKey, m: u32, value: i64) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source,
            source_id: ui.element_id(source).unwrap(),
            msg: m,
            arg: MsgArg::value(value),
        },
    }
}

// ----- the minimap (§8, §13.1–§13.4) ---------------------------------------------------

const RADAR_LAYOUT: u32 = 0x22FF_3000;
const RADAR: u32 = 0x1000_0100;

fn tile_map(x: u8, y: u8) -> GameMap {
    GameMap {
        image: 0x4100_0000 | (u32::from(x) << 8) | u32::from(y),
        layers: Vec::new(),
        block_x: Some(i32::from(x)),
        block_y: Some(i32::from(y)),
        pixel_x: 0,
        pixel_y: 200,
        scale: Some(1.25),
        fog: false,
    }
}

#[derive(Default)]
struct Maps {
    pending: bool,
    missing: Vec<(u8, u8)>,
    dungeon: Option<GameMap>,
    /// Dungeon lookups answer `Pending`.
    dungeon_pending: bool,
    /// Dungeon maps by landblock (X, Y), before `dungeon`.
    dungeons: Vec<((u8, u8), GameMap)>,
    asked: usize,
}

impl MapSource for Maps {
    fn landblock_map(&mut self, _region: u8, x: u8, y: u8) -> MapLookup {
        self.asked += 1;
        if self.pending {
            return MapLookup::Pending;
        }
        if self.missing.contains(&(x, y)) {
            return MapLookup::Ready(None);
        }
        MapLookup::Ready(Some(tile_map(x, y)))
    }

    fn dungeon_map(&mut self, _r: u8, x: u8, y: u8, _cell: u16) -> MapLookup {
        if self.dungeon_pending {
            return MapLookup::Pending;
        }
        if let Some((_, m)) = self.dungeons.iter().find(|(b, _)| *b == (x, y)) {
            return MapLookup::Ready(Some(m.clone()));
        }
        MapLookup::Ready(self.dungeon.clone())
    }
}

fn with_states(mut d: ElementDesc, ids: &[u32]) -> ElementDesc {
    for s in ids {
        d = state(d, *s, 0, vec![], vec![]);
    }
    d
}

/// The image of the synthetic `MinimapPlayerMarker`.
const MARKER_IMG: u32 = 0x4100_0AAA;

/// The synthetic `radar` layout: the radar and the tile template.
fn radar_elements() -> Vec<ElementDesc> {
    let g = gv();
    let (ids, t, vv, pp) = (g.ids, g.types, v().types, v().pids);
    let tiles = typed(ids.radar_map_image, vv.list_box, 0, 0, 600, 600);
    let tiles = prop(tiles, pp.lb_horizontal, b(true));
    let tiles = prop(tiles, pp.lb_max_columns, int(3));
    let tiles = prop(tiles, pp.lb_max_rows, int(3));
    let fog = child(
        field(ids.fog_display, 0, 0, 2048, 1536),
        child(
            field(ids.fog_parent, 0, 0, 2048, 1536),
            typed(ids.fog_field, t.map_fog_of_war, 0, 0, 2048, 1536),
        ),
    );
    let rim = with_states(
        field(ids.minimap_frame, -34, -33, 237, 237),
        &g.states.moods(),
    );
    let time = with_states(
        typed(ids.time_of_day, t.time_of_day_display, 0, 0, 20, 20),
        &[
            g.states.day_time,
            g.states.night_time,
            g.states.unknown_time,
        ],
    );
    let mut viewport = field(ids.minimap_viewport, 27, 0, 170, 170);
    for c in [
        z(tiles, 2, 0),
        z(field(ids.minimap_map_image, 0, -1, 2048, 1536), 2, 1),
        z(fog, 1, 2),
        z(
            media(
                field(ids.player_marker, 0, 3, 28, 28),
                image_item(MARKER_IMG, 2),
            ),
            0,
            3,
        ),
        z(field(ids.quest_objective, 0, 0, 30, 30), 0, 4),
        z(rim, -1, 5),
        z(time, -1, 6),
        z(typed(ids.map_zoom, vv.button, 0, 150, 16, 16), -1, 7),
    ] {
        viewport = child(viewport, c);
    }
    let mut radar = typed(RADAR, t.radar, 0, 0, 224, 260);
    for c in [
        viewport,
        typed(ids.area_label, vv.text, 27, 200, 170, 20),
        typed(ids.coordinate_label, vv.text, 12, 220, 200, 20),
        field(ids.area_quest_highlight, 0, 194, 224, 32),
    ] {
        radar = child(radar, c);
    }
    let template = field(ids.radar_tile_template, 0, 0, 200, 200);
    vec![radar, template]
}

fn radar_ui(env_images: Vec<(u32, u32, u32)>) -> (Ui, GamePanels, ElementKey) {
    let g = gv();
    let mut env = env_with(vec![layout(RADAR_LAYOUT, 224, 260, radar_elements())]);
    for (did, w, h) in env_images {
        add_image(&mut env, did, solid(w, h, 0xFF80_8080));
    }
    let mut ui = ui_with(env, 224, 260);
    let root = ui.open_panel(Did(RADAR_LAYOUT), RADAR).unwrap();
    let mut gp = GamePanels::new(
        g,
        GameLayouts {
            radar: Some(Did(RADAR_LAYOUT)),
            ..GameLayouts::default()
        },
    );
    gp.attach(&mut ui, root, &SandboxData::default(), &NoShortcuts);
    (ui, gp, root)
}

fn outdoor(x: u8, y: u8, local: (f32, f32)) -> RadarInput {
    RadarInput {
        region: 1,
        x,
        y,
        cell: 0,
        global: (
            f32::from(x) * 160.0 + local.0,
            f32::from(y) * 160.0 + local.1,
        ),
        coordinates: Some("c".into()),
        area_name: Some("a".into()),
        ..RadarInput::default()
    }
}

#[test]
fn radar_creation_8_2() {
    let (ui, gp, _) = radar_ui(vec![]);
    let r = &gp.radars[0];
    let p = r.parts.expect("all parts found");
    assert_eq!(r.centre, (85, 85));
    let mb = ui.layout_box(p.marker).unwrap();
    assert_eq!((mb.l, mb.t), (71, 71));
    assert_eq!(
        ui.property(p.marker, v().pids.object_mode),
        Some(&PropertyValue::Enum(1))
    );
    let g = gv();
    let quest = tessera_ui::game::find_below(&ui, r.key, g.ids.quest_objective).unwrap();
    let qh = tessera_ui::game::find_below(&ui, r.key, g.ids.area_quest_highlight).unwrap();
    assert!(!ui.is_visible(quest) && !ui.is_visible(qh));
    assert!(ui.is_visible(p.fog_display));
    // §8.5.3 Tessera rule: the fog draws nothing.
    assert!(ui.base_image(p.fog_field).is_none());
}

#[test]
fn example_13_1_outdoor_placement() {
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    let input = outdoor(100, 150, (50.0, 80.0));
    gp.update_radar(&mut ui, &input, &mut maps);
    let r = gp.radars[0].clone();
    let p = r.parts.unwrap();
    let items = ui.list_entries(p.tiles);
    assert_eq!(items.len(), 9);
    // Item order: north row first, west column first; the player's block is item 4.
    let blocks = grid_blocks(100, 150);
    assert_eq!(blocks[0], (99, 151));
    assert_eq!(blocks[4], (100, 150));
    assert_eq!(blocks[6], (99, 149));
    for (i, k) in items.iter().enumerate() {
        let bx = ui.layout_box(*k).unwrap();
        assert_eq!((bx.l, bx.t), (200 * (i as i32 % 3), 200 * (i as i32 / 3)));
        let (_, mode) = ui.base_image(*k).expect("tile image");
        assert_eq!(mode, 5);
    }
    let (img, _) = ui.base_image(items[4]).unwrap();
    assert_eq!(img, tessera_ui::ImageRef::Did(tile_map(100, 150).image));
    let g = r.grid_georef.unwrap();
    assert_eq!(g.to_pixel(16_050.0, 24_080.0), (262, 300));
    let tb = ui.layout_box(p.tiles).unwrap();
    assert_eq!((tb.l, tb.t), (-177, -215));
    let mb = ui.layout_box(p.marker).unwrap();
    assert_eq!((mb.l, mb.t), (71, 71));
    // §8.3: outdoors the indoor map and fog are hidden.
    assert!(ui.is_visible(p.tiles));
    assert!(!ui.is_visible(p.map_image) && !ui.is_visible(p.fog_display));
    // Labels (§8.7.4 Tessera rules).
    assert_eq!(ui.text_of(p.area_label), "a");
    assert!(ui.is_visible(p.coord_label));
    // §8.4.1: the grid is rebuilt only when region, X, Y or layer change.
    let asked = maps.asked;
    gp.update_radar(&mut ui, &outdoor(100, 150, (60.0, 80.0)), &mut maps);
    assert_eq!(maps.asked, asked);
    let tb = ui.layout_box(p.tiles).unwrap();
    assert_eq!(tb.l, 85 - 275);
    let mut hidden = outdoor(100, 150, (60.0, 80.0));
    hidden.coordinates = None;
    gp.update_radar(&mut ui, &hidden, &mut maps);
    assert!(!ui.is_visible(p.coord_label));
}

#[test]
fn gp10_player_pixel_on_centre() {
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    for (x, y, lx, ly) in [
        (100u8, 150u8, 0.0f32, 0.0f32),
        (1, 1, 159.9, 0.5),
        (0, 0, 10.0, 10.0),
        (253, 3, 80.0, 80.0),
        (40, 200, 33.3, 121.7),
    ] {
        let input = outdoor(x, y, (lx, ly));
        gp.update_radar(&mut ui, &input, &mut maps);
        let r = &gp.radars[0];
        let p = r.parts.unwrap();
        let (px, py) = r
            .grid_georef
            .unwrap()
            .to_pixel(input.global.0, input.global.1);
        let tb = ui.layout_box(p.tiles).unwrap();
        assert_eq!((tb.l + px, tb.t + py), (85, 85), "block ({x}, {y})");
    }
}

#[test]
fn example_13_2_grid_at_the_world_edge() {
    assert_eq!(tile_range(0), Some((0, 2)));
    assert_eq!(tile_range(1), Some((0, 2)));
    assert_eq!(tile_range(100), Some((99, 101)));
    assert_eq!(tile_range(253), Some((252, 254)));
    assert_eq!(tile_range(254), None);
    assert_eq!(tile_range(255), None);
    assert!(grid_blocks(254, 10).is_empty());
    // §8.6.1 OPEN Tessera rule: no tiles, position unchanged, marker centred.
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    gp.update_radar(&mut ui, &outdoor(100, 150, (50.0, 80.0)), &mut maps);
    let p = gp.radars[0].parts.unwrap();
    gp.update_radar(&mut ui, &outdoor(254, 150, (50.0, 80.0)), &mut maps);
    assert!(ui.list_entries(p.tiles).is_empty());
    let tb = ui.layout_box(p.tiles).unwrap();
    assert_eq!((tb.l, tb.t), (-177, -215));
    let mb = ui.layout_box(p.marker).unwrap();
    assert_eq!((mb.l, mb.t), (71, 71));
}

#[test]
fn radar_waits_for_every_record_and_handles_missing_ones() {
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps {
        pending: true,
        ..Maps::default()
    };
    let input = outdoor(100, 150, (50.0, 80.0));
    gp.update_radar(&mut ui, &input, &mut maps);
    let p = gp.radars[0].parts.unwrap();
    assert!(ui.list_entries(p.tiles).is_empty());
    maps.pending = false;
    // Item 6 (block (99, 149)) has no record: the georeference comes from item 4.
    maps.missing = vec![(99, 149)];
    gp.update_radar(&mut ui, &input, &mut maps);
    let items = ui.list_entries(p.tiles);
    assert_eq!(items.len(), 9);
    assert!(ui.base_image(items[6]).is_none());
    let tb = ui.layout_box(p.tiles).unwrap();
    assert_eq!((tb.l, tb.t), (-177, -215));
}

#[test]
fn example_13_3_content_layer() {
    let mut m = GameMap {
        image: 0x10,
        layers: vec![(1, 0x11), (0x1000_0123, 0x12)],
        ..GameMap::default()
    };
    assert_eq!(m.image_for_layer(0), 0x10);
    assert_eq!(m.image_for_layer(0x1000_0123), 0x12);
    assert_eq!(m.image_for_layer(1), 0x11);
    assert_eq!(m.image_for_layer(0x1000_0999), 0x11);
    m.layers = vec![(0x1000_0123, 0x12)];
    assert_eq!(m.image_for_layer(0x1000_0999), 0x10);
    // The last layer-1 entry wins.
    m.layers = vec![(1, 0x21), (1, 0x22)];
    assert_eq!(m.image_for_layer(7), 0x22);
}

#[test]
fn game_map_from_property_value() {
    let g = gv();
    let p = g.pids;
    let v = strukt(vec![
        (p.map_image, did(0x4100_0001)),
        (
            p.map_image_array,
            arr(vec![(
                1,
                strukt(vec![
                    (p.map_content_layer, en(1)),
                    (p.map_image, did(0x4100_0002)),
                ]),
            )]),
        ),
        (p.map_block_x, int(10)),
        (p.map_block_y, int(20)),
        (p.map_pixel_x, int(-5)),
        (p.map_pixel_y, int(300)),
        (p.map_scale, f(3.0)),
        (p.map_fog_of_war, b(true)),
    ]);
    let m = GameMap::from_value(&v, &p).unwrap();
    assert_eq!(m.image, 0x4100_0001);
    assert_eq!(m.layers, vec![(1, 0x4100_0002)]);
    assert!(m.fog);
    assert_eq!(
        m.georef(),
        Some(Georef {
            block_x: 10,
            block_y: 20,
            pixel_x: -5,
            pixel_y: 300,
            scale: 3.0
        })
    );
    assert!(GameMap::from_value(&int(3), &p).is_none());
    let partial = GameMap::from_value(&strukt(vec![(p.map_scale, f(1.0))]), &p).unwrap();
    assert!(partial.georef().is_none());
}

#[test]
fn example_13_4_indoor_placement() {
    let g = Georef {
        block_x: 10,
        block_y: 20,
        pixel_x: 400,
        pixel_y: 300,
        scale: 3.0,
    };
    assert_eq!(g.to_pixel(1_610.5, 3_205.25), (431, 285));
    assert_eq!(g.to_pixel(1_600.0 - 0.2, 3_200.0), (400, 300));
    let img = 0x4100_0777;
    let (mut ui, mut gp, _) = radar_ui(vec![(img, 800, 600)]);
    let mut maps = Maps {
        dungeon: Some(GameMap {
            image: img,
            block_x: Some(10),
            block_y: Some(20),
            pixel_x: 400,
            pixel_y: 300,
            scale: Some(3.0),
            ..GameMap::default()
        }),
        ..Maps::default()
    };
    let input = RadarInput {
        region: 1,
        x: 10,
        y: 20,
        cell: 0x0100,
        global: (1_610.5, 3_205.25),
        ..RadarInput::default()
    };
    gp.update_radar(&mut ui, &input, &mut maps);
    let p = gp.radars[0].parts.unwrap();
    let mb = ui.layout_box(p.map_image).unwrap();
    assert_eq!((mb.l, mb.t, mb.w(), mb.h()), (-346, -200, 800, 600));
    let fb = ui.layout_box(p.fog_parent).unwrap();
    assert_eq!((fb.l, fb.t), (-346, -200));
    assert_eq!(ui.base_image(p.map_image).unwrap().1, 5);
    assert!(ui.is_visible(p.map_image) && ui.is_visible(p.fog_display));
    assert!(!ui.is_visible(p.tiles));
    // An invalid position changes nothing.
    let mut bad = input.clone();
    bad.cell = 0xFFFF;
    bad.global = (0.0, 0.0);
    gp.update_radar(&mut ui, &bad, &mut maps);
    let mb = ui.layout_box(p.map_image).unwrap();
    assert_eq!((mb.l, mb.t), (-346, -200));
}

#[test]
fn marker_turns_by_heading() {
    // §8.6.3 with render.md §6.6: z = 90 − h.
    assert_eq!(Radar::marker_angle(0.0), 90.0);
    assert_eq!(Radar::marker_angle(90.0), 0.0);
    assert_eq!(Radar::marker_angle(180.0), 270.0);
    assert_eq!(Radar::marker_angle(f32::NAN), 0.0);
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    let mut input = outdoor(100, 150, (50.0, 80.0));
    input.heading = 30.0;
    gp.update_radar(&mut ui, &input, &mut maps);
    let p = gp.radars[0].parts.unwrap();
    assert!(ui.owns_surface(p.marker));
    assert_eq!(
        tessera_ui::game::held_rotation(&ui, p.marker),
        [0.0, 0.0, 60.0]
    );
}

#[test]
fn rim_mood_and_time_of_day() {
    let g = gv();
    let s = g.states;
    assert_eq!(mood_state(&s, -11), s.mood_n10);
    assert_eq!(mood_state(&s, -10), s.mood_n10);
    assert_eq!(mood_state(&s, -3), s.mood_n3);
    assert_eq!(mood_state(&s, 0), s.mood_0);
    assert_eq!(mood_state(&s, 7), s.mood_p7);
    assert_eq!(mood_state(&s, 99), s.mood_p10);
    assert_eq!(time_of_day_state(&s, true, Some(true)), s.night_time);
    assert_eq!(time_of_day_state(&s, true, Some(false)), s.day_time);
    assert_eq!(time_of_day_state(&s, true, None), s.unknown_time);
    assert_eq!(time_of_day_state(&s, false, Some(true)), s.unknown_time);
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    let mut input = outdoor(100, 150, (50.0, 80.0));
    input.is_night = Some(false);
    gp.update_radar(&mut ui, &input, &mut maps);
    let r = gp.radars[0].clone();
    assert_eq!(ui.state(r.rim.unwrap()), Some(s.mood_0));
    assert_eq!(ui.state(r.time_of_day.unwrap()), Some(s.day_time));
}

#[test]
fn map_zoom_opens_the_world_map() {
    let (mut ui, mut gp, root) = radar_ui(vec![]);
    let g = gv();
    let zoom = tessera_ui::game::find_below(&ui, root, g.ids.map_zoom).unwrap();
    let ev = msg(&ui, zoom, ui.vocab().msgs.button_click, 0);
    assert_eq!(
        gp.handle_event(&mut ui, &ev),
        vec![GameAction::OpenWorldMap]
    );
}

proptest! {
    #[test]
    fn gp11_mood_monotone_and_saturating(a in -1000i32..1000, b in -1000i32..1000) {
        if a <= b {
            prop_assert!(mood_index(a) <= mood_index(b));
        }
        prop_assert!(mood_index(a) <= 20);
        if a <= -10 { prop_assert_eq!(mood_index(a), 0); }
        if a >= 10 { prop_assert_eq!(mood_index(a), 20); }
    }

    #[test]
    fn gp10_transform_lands_on_centre(x in 1u8..253, y in 1u8..253, lx in 0.0f32..160.0, ly in 0.0f32..160.0) {
        let blocks = grid_blocks(x, y);
        prop_assert_eq!(blocks.len(), 9);
        let i = 6;
        let m = tile_map(blocks[i].0, blocks[i].1);
        let g = tessera_ui::game::map::tile_georef(i, 3, blocks[i], &m).unwrap();
        let gx = f32::from(x) * 160.0 + lx;
        let gy = f32::from(y) * 160.0 + ly;
        let (px, py) = g.to_pixel(gx, gy);
        // The player's pixel lies inside the centre tile (item 4).
        prop_assert!((200..=400).contains(&px), "{px}");
        prop_assert!((200..=400).contains(&py), "{py}");
    }
}

// ----- drag boxes (§4, §13.5, GP7) -------------------------------------------------------

const DBOX_LAYOUT: u32 = 0x22FF_3100;
const DBOX: u32 = 0x1000_0200;
const CONTENT: u32 = 0x1000_0201;
const S_HIDDEN: u32 = 0x50;
const S_VISIBLE: u32 = 0x51;
const S_DRAGGING: u32 = 0x52;
const S_OVER: u32 = 0x53;

fn dbox_ui() -> (Ui, GamePanels, ElementKey) {
    let g = gv();
    let (ids, p, vv) = (g.ids, g.pids, v().types);
    let all = [S_HIDDEN, S_VISIBLE, S_DRAGGING, S_OVER];
    let wants = |d: ElementDesc| with_states(prop(d, p.dbox_child_wants, b(true)), &all);
    let mut root = with_states(typed(DBOX, g.types.hidden_drag_box, 0, 0, 200, 100), &all);
    root = prop(root, p.dbox_hidden_state, en(S_HIDDEN));
    root = prop(root, p.dbox_visible_state, en(S_VISIBLE));
    root = prop(root, p.dbox_dragging_state, en(S_DRAGGING));
    root = prop(root, p.dbox_mouseover_state, en(S_OVER));
    root = child(root, wants(field(ids.drag_box_frame, 0, 0, 200, 100)));
    root = child(
        root,
        wants(typed(ids.drag_box_bar, vv.dragbar, 0, 0, 200, 16)),
    );
    root = child(
        root,
        wants(typed(ids.drag_box_title, vv.text, 0, 0, 200, 16)),
    );
    root = child(root, with_states(field(CONTENT, 0, 20, 200, 80), &all));
    let mut ui = ui_one(DBOX_LAYOUT, 200, 100, vec![root], 800, 600);
    let k = ui.open_panel(Did(DBOX_LAYOUT), DBOX).unwrap();
    let mut gp = GamePanels::new(g, GameLayouts::default());
    gp.attach(&mut ui, k, &SandboxData::default(), &NoShortcuts);
    (ui, gp, k)
}

#[test]
fn example_13_5_drag_box_and_gp7() {
    let (mut ui, mut gp, root) = dbox_ui();
    let b = gp.drag_boxes[0].clone();
    let kids = [b.frame.unwrap(), b.bar.unwrap(), b.title.unwrap()];
    let content = tessera_ui::game::find_below(&ui, root, CONTENT).unwrap();
    let check = |ui: &Ui, s: u32, shown: bool| {
        assert_eq!(ui.state(root), Some(s));
        for k in kids {
            assert_eq!(ui.is_visible(k), shown);
            assert_eq!(ui.state(k), Some(s), "§4.5 passes the state to the frame");
        }
        assert_ne!(
            ui.state(content),
            Some(s),
            "children without the flag keep theirs"
        );
    };
    // GP7: toggle off.
    check(&ui, S_HIDDEN, false);
    gp.toggle_drag_boxes(&mut ui);
    check(&ui, S_VISIBLE, true);
    let over = msg(&ui, kids[1], ui.vocab().msgs.mouse_over_top, 1);
    gp.handle_event(&mut ui, &over);
    check(&ui, S_OVER, true);
    let press = msg(&ui, root, ui.vocab().msgs.mouse_press, 0);
    gp.handle_event(&mut ui, &press);
    check(&ui, S_DRAGGING, true);
    let release = msg(&ui, root, ui.vocab().msgs.mouse_release, 0);
    gp.handle_event(&mut ui, &release);
    check(&ui, S_OVER, true);
    gp.toggle_drag_boxes(&mut ui);
    check(&ui, S_HIDDEN, false);
    assert!(!gp.drag_boxes[0].over && !gp.drag_boxes[0].dragging);
}

// ----- hide test (§6, §13.6, GP9, GP14) --------------------------------------------------

const P: u32 = 0x0000_7777;

fn cmp_value(a: PropertyValue) -> PropertyValue {
    arr(vec![(P, a)])
}

#[test]
fn example_13_6_hide_test() {
    let mut d = SandboxData::default();
    // Int, operator 6: hides when 5 > B.
    let v5 = cmp_value(int(5));
    d.player.insert(P, int(3));
    assert!(compare_with_player(&v5, 6, &d));
    d.player.insert(P, int(5));
    assert!(!compare_with_player(&v5, 6, &d));
    d.player.clear();
    assert!(compare_with_player(&v5, 6, &d), "absent → B = 0");
    assert!(!compare_with_player(&v5, 0, &d), "operator 0 never hides");
    assert!(!compare_with_player(&v5, 7, &d));
    // Boolean with a dictionary default of false.
    let mut d = SandboxData::default();
    d.types.insert(P, 21);
    d.defaults.insert(P, b(false));
    let vt = cmp_value(b(true));
    d.player.insert(P, b(true));
    assert!(compare_with_player(&vt, 3, &d));
    assert!(!compare_with_player(&vt, 2, &d));
    assert!(!compare_with_player(&vt, 6, &d));
    d.player.clear();
    assert!(!compare_with_player(&vt, 3, &d));
    assert!(compare_with_player(&vt, 2, &d));
    assert!(!compare_with_player(&vt, 6, &d));
    // Enum.
    let mut d = SandboxData::default();
    d.types.insert(P, 8);
    let ve = cmp_value(en(7));
    d.player.insert(P, en(7));
    assert!(compare_with_player(&ve, 3, &d));
    assert!(!compare_with_player(&ve, 2, &d));
    d.player.insert(P, en(9));
    assert!(compare_with_player(&ve, 2, &d));
    assert!(!compare_with_player(&ve, 6, &d));
    d.player.insert(P, en(0xFFFF_FFFF));
    assert!(
        compare_with_player(&ve, 6, &d),
        "keys compare as signed 32-bit"
    );
    d.player.clear();
    assert!(compare_with_player(&ve, 2, &d), "absent → key 0");
    assert!(compare_with_player(&ve, 6, &d));
}

#[test]
fn compare_table_by_type() {
    // Floats and NaN.
    assert!(compare(1, 9, &f(2.0), &f(1.0)));
    assert!(!compare(3, 9, &f(f32::NAN), &f(f32::NAN)));
    assert!(compare(2, 9, &f(f32::NAN), &f(1.0)));
    for op in [1, 4, 5, 6] {
        assert!(!compare(op, 9, &f(f32::NAN), &f(1.0)));
    }
    // 32-bit types with equality only.
    for code in [5u32, 10, 18, 20, 21] {
        let a = PropertyValue::Bitfield32(4);
        assert!(compare(3, code, &a, &a));
        assert!(!compare(2, code, &a, &a));
        for op in [1, 4, 5, 6] {
            assert!(!compare(op, code, &a, &PropertyValue::Bitfield32(0)));
        }
    }
    // Own-routine types never hide (Tessera rule of the §6.2 OPEN).
    for code in [1u32, 2, 3, 4, 6, 7, 11, 12, 13, 14, 16, 17, 19, 22, 0, 99] {
        for op in 0..8 {
            assert!(!compare(op, code, &int(1), &int(1)));
        }
    }
    // Every operator on ints.
    let t = |op, a, b| compare(op, 15, &int(a), &int(b));
    assert!(t(1, 2, 2) && t(1, 3, 2) && !t(1, 1, 2));
    assert!(t(4, -1, 0) && !t(4, 0, 0));
    assert!(t(5, 0, 0) && !t(5, 1, 0));
    assert!(t(6, 1, 0) && !t(6, 0, 0));
}

const HW_LAYOUT: u32 = 0x22FF_3200;
const LEVEL_SCALED: u32 = 0x0000_8888;

#[test]
fn gp9_level_fields_in_the_sandbox() {
    let g = gv();
    let p = g.pids;
    let ty = g.types.hide_when_field;
    let level = prop(
        prop(
            typed(0x1000_0301, ty, 0, 0, 20, 10),
            p.hw_compare_operator,
            en(2),
        ),
        p.hw_compare_value,
        arr(vec![(LEVEL_SCALED, int(0))]),
    );
    let scaled = prop(
        prop(
            typed(0x1000_0302, ty, 0, 0, 20, 10),
            p.hw_compare_operator,
            en(3),
        ),
        p.hw_compare_value,
        arr(vec![(LEVEL_SCALED, int(0))]),
    );
    let monster = prop(
        typed(0x1000_0303, ty, 0, 0, 20, 10),
        p.hw_monster_player,
        b(true),
    );
    let did_field = prop(
        typed(0x1000_0304, ty, 0, 0, 20, 10),
        p.hw_invalid_data_id,
        en(0x55),
    );
    let mut root = field(0x1000_0300, 0, 0, 100, 100);
    for c in [level, scaled, monster, did_field] {
        root = child(root, c);
    }
    let mut ui = ui_one(HW_LAYOUT, 100, 100, vec![root], 800, 600);
    let k = ui.open_panel(Did(HW_LAYOUT), 0x1000_0300).unwrap();
    let mut gp = GamePanels::new(g, GameLayouts::default());
    let mut data = SandboxData::default();
    data.types.insert(LEVEL_SCALED, 15);
    gp.attach(&mut ui, k, &data, &NoShortcuts);
    let el = |ui: &Ui, id| tessera_ui::game::find_below(ui, k, id).unwrap();
    assert!(
        ui.is_visible(el(&ui, 0x1000_0301)),
        "PlayerLevel_HWField visible"
    );
    assert!(
        !ui.is_visible(el(&ui, 0x1000_0302)),
        "PlayerScaledLevel_HWField hidden"
    );
    assert!(ui.is_visible(el(&ui, 0x1000_0303)), "not a monster player");
    assert!(
        !ui.is_visible(el(&ui, 0x1000_0304)),
        "row 12: absent DID hides"
    );
    // The player becomes level-scaled.
    data.player.insert(LEVEL_SCALED, int(42));
    gp.refresh_player(&mut ui, &data);
    assert!(!ui.is_visible(el(&ui, 0x1000_0301)));
    assert!(ui.is_visible(el(&ui, 0x1000_0302)));
    // Without a local player the test stops after row 6.
    data.local_player = false;
    gp.refresh_player(&mut ui, &data);
    assert!(ui.is_visible(el(&ui, 0x1000_0304)));
}

// ----- highlighters, pips, page changer, hidden types (§5, §7, §11, GP8) ----------------

const MISC_LAYOUT: u32 = 0x22FF_3300;

#[test]
fn gp8_highlighters_pips_page_changer_and_hidden_types() {
    let g = gv();
    let t = g.types;
    let st = v().states;
    let hl = with_states(
        prop(
            typed(0x1000_0401, t.tutorial_highlighter, 0, 0, 10, 10),
            g.pids.tutorial_type,
            en(3),
        ),
        &[st.normal, st.highlight],
    );
    let pip = typed(0x1000_0402, t.pip_display, 0, 0, 10, 10);
    let changer = child(
        prop(
            typed(0x1000_0403, t.page_changer, 0, 0, 30, 10),
            v().pids.hide,
            b(false),
        ),
        typed(0x1000_0404, v().types.text, 10, 0, 10, 10),
    );
    let mut root = field(0x1000_0400, 0, 0, 100, 100);
    for c in [
        hl,
        pip,
        changer,
        typed(0x1000_0405, t.housing_panel_button, 0, 0, 5, 5),
        typed(0x1000_0406, t.medal_icon, 0, 0, 5, 5),
        typed(0x1000_0407, t.raid_lock, 0, 0, 5, 5),
        typed(0x1000_0408, t.chest_lock, 0, 0, 5, 5),
    ] {
        root = child(root, c);
    }
    let mut ui = ui_one(MISC_LAYOUT, 100, 100, vec![root], 800, 600);
    let k = ui.open_panel(Did(MISC_LAYOUT), 0x1000_0400).unwrap();
    let mut gp = GamePanels::new(g, GameLayouts::default());
    gp.attach(&mut ui, k, &SandboxData::default(), &NoShortcuts);
    let el = |ui: &Ui, id| tessera_ui::game::find_below(ui, k, id).unwrap();
    assert!(!ui.is_visible(el(&ui, 0x1000_0401)));
    assert_eq!(ui.state(el(&ui, 0x1000_0401)), Some(st.normal));
    assert!(!ui.is_visible(el(&ui, 0x1000_0402)));
    assert!(ui.is_visible(el(&ui, 0x1000_0403)));
    assert_eq!(ui.text_of(el(&ui, 0x1000_0404)), "1");
    for id in 0x1000_0405..=0x1000_0408 {
        assert!(!ui.is_visible(el(&ui, id)));
    }
    // A tutorial event for another type changes nothing; for its own type it highlights.
    gp.tutorial_event(&mut ui, Some(4));
    assert!(!ui.is_visible(el(&ui, 0x1000_0401)));
    gp.tutorial_event(&mut ui, Some(3));
    assert!(ui.is_visible(el(&ui, 0x1000_0401)));
    assert_eq!(ui.state(el(&ui, 0x1000_0401)), Some(st.highlight));
    gp.tutorial_event(&mut ui, None);
    assert!(!ui.is_visible(el(&ui, 0x1000_0401)));
    // Attaching twice adds nothing.
    let n = gp.highlighters.len();
    gp.attach(&mut ui, k, &SandboxData::default(), &NoShortcuts);
    assert_eq!(gp.highlighters.len(), n);
}

// ----- quickslot bars (§10, GP12) --------------------------------------------------------

const QS_LAYOUT: u32 = 0x22FF_3400;
const QS_LIST: u32 = 0x1000_0501;

struct OneShortcut;

impl Shortcuts for OneShortcut {
    fn shortcut(&self, index: i32) -> Shortcut {
        if index == 15 {
            Shortcut::Kind(1)
        } else {
            Shortcut::Empty
        }
    }

    fn key_label(&self, index: i32) -> Option<String> {
        (index == 15).then(|| "K".to_string())
    }
}

fn qs_ui(offset: i32, shortcuts: &dyn Shortcuts) -> (Ui, GamePanels) {
    let g = gv();
    let (ids, pp, vv) = (g.ids, v().pids, v().types);
    let list = prop(
        prop(
            prop(
                typed(QS_LIST, vv.list_box, 0, 0, 420, 35),
                pp.lb_horizontal,
                b(true),
            ),
            pp.lb_max_columns,
            int(12),
        ),
        pp.lb_max_rows,
        int(1),
    );
    let bar = child(
        prop(
            prop(
                typed(0x1000_0500, g.types.quickslot_bar, 0, 0, 420, 35),
                g.pids.quickslot_listbox,
                en(QS_LIST),
            ),
            g.pids.quickslot_offset,
            int(offset),
        ),
        list,
    );
    let button = child(
        child(
            typed(ids.quickslot_button, vv.button, 0, 0, 35, 35),
            typed(ids.quickslot_label, vv.text, 0, 0, 35, 12),
        ),
        field(ids.quickslot_overlay, 0, 0, 35, 35),
    );
    let empty = field(ids.quickslot_empty, 0, 0, 35, 35);
    let mut ui = ui_one(QS_LAYOUT, 420, 35, vec![bar, button, empty], 800, 600);
    let k = ui.open_panel(Did(QS_LAYOUT), 0x1000_0500).unwrap();
    let mut gp = GamePanels::new(
        g,
        GameLayouts {
            quickslots: Some(Did(QS_LAYOUT)),
            ..GameLayouts::default()
        },
    );
    gp.attach(&mut ui, k, &SandboxData::default(), shortcuts);
    (ui, gp)
}

#[test]
fn gp12_empty_bar_shows_slots_only_during_a_drag() {
    let (mut ui, mut gp) = qs_ui(0, &NoShortcuts);
    let bar = gp.bars[0].clone();
    let list = bar.list.unwrap();
    let entries = ui.list_entries(list);
    assert_eq!(entries.len(), 12);
    let g = gv();
    for (i, e) in entries.iter().enumerate() {
        assert_eq!(ui.property(*e, g.pids.shortcut_index), Some(&int(i as i32)));
        assert!(!ui.is_visible(*e));
        assert!(tessera_ui::game::find_below(&ui, *e, g.ids.quickslot_empty).is_some());
        let bx = ui.layout_box(*e).unwrap();
        assert_eq!((bx.l, bx.t), (35 * i as i32, 0));
    }
    gp.handle_event(&mut ui, &UiEvent::DragBegan { source: list });
    assert!(entries.iter().all(|e| ui.is_visible(*e)));
    gp.handle_event(
        &mut ui,
        &UiEvent::DragEnded {
            source: list,
            target: None,
            accepted: false,
        },
    );
    assert!(entries.iter().all(|e| !ui.is_visible(*e)));
}

#[test]
fn quickslot_offset_labels_and_filled_slots() {
    let (mut ui, mut gp) = qs_ui(12, &OneShortcut);
    let g = gv();
    let bar = gp.bars[0].clone();
    let entries = ui.list_entries(bar.list.unwrap());
    assert_eq!(
        ui.property(entries[0], g.pids.shortcut_index),
        Some(&int(12))
    );
    assert_eq!(
        ui.property(entries[11], g.pids.shortcut_index),
        Some(&int(23))
    );
    assert!(ui.is_visible(entries[3]), "slot 15 holds a shortcut");
    let label = tessera_ui::game::find_below(&ui, entries[3], g.ids.quickslot_label).unwrap();
    assert_eq!(ui.text_of(label), "K");
    gp.handle_event(
        &mut ui,
        &UiEvent::DragEnded {
            source: entries[0],
            target: None,
            accepted: false,
        },
    );
    assert!(ui.is_visible(entries[3]));
    // Rebuilding keeps 12 slots.
    gp.bars[0].rebuild(&mut ui, &g, &OneShortcut).unwrap();
    assert_eq!(ui.list_entries(bar.list.unwrap()).len(), 12);
}

// ----- the world map (§9) ----------------------------------------------------------------

const MAP_LAYOUT: u32 = 0x22FF_3500;
const E0: u32 = 0x1000_0600;
const E1: u32 = 0x1000_0601;
const E2: u32 = 0x1000_0602;
const E3: u32 = 0x1000_0603;
const BUTTON: u32 = 0x1000_0610;

fn node(fields: Vec<(u32, PropertyValue)>, children: Vec<PropertyValue>) -> PropertyValue {
    let p = gv().pids;
    let mut f = fields;
    if !children.is_empty() {
        f.push((
            p.map_children,
            arr(children.into_iter().map(|c| (p.map_area_data, c)).collect()),
        ));
    }
    strukt(f)
}

fn map_tree_value() -> PropertyValue {
    let p = gv().pids;
    let geo = |e: u32, img: u32| {
        vec![
            (p.map_active_element, en(e)),
            (p.map_image, did(img)),
            (p.map_block_x, int(0)),
            (p.map_block_y, int(0)),
            (p.map_pixel_x, int(0)),
            (p.map_pixel_y, int(768)),
            (p.map_scale, f(0.02)),
            (p.map_pixel_width, int(1024)),
            (p.map_pixel_height, int(768)),
        ]
    };
    let areas = |a: &[u32]| {
        (
            p.map_area_dids,
            arr(a.iter().map(|x| (0x55, did(*x))).collect()),
        )
    };
    let mut c = geo(E3, 0x4100_0F03);
    c.extend([(p.map_region, en(2)), areas(&[0x21])]);
    let mut b1 = geo(E2, 0x4100_0F02);
    b1.extend([(p.map_region, en(2)), areas(&[0x20])]);
    let mut a = geo(E1, 0x4100_0F01);
    a.extend([(p.map_region, en(1)), areas(&[0x10, 0x11])]);
    node(
        geo(E0, 0x4100_0F00),
        vec![node(a, vec![]), node(b1, vec![node(c, vec![])])],
    )
}

#[test]
fn map_tree_and_node_choice() {
    let g = gv();
    let t = MapTree::from_value(&map_tree_value(), &g.pids).unwrap();
    assert_eq!(t.nodes.len(), 4);
    assert_eq!(t.nodes[0].depth, 0);
    assert_eq!(t.nodes[0].children, vec![1, 2]);
    assert_eq!(t.nodes[2].children, vec![3]);
    assert_eq!(t.nodes[3].parent, Some(2));
    assert_eq!(t.choose(Some(0x21), Some(2)), Some(3));
    assert_eq!(t.choose(Some(0x11), None), Some(1));
    assert_eq!(
        t.choose(Some(0x99), Some(2)),
        Some(2),
        "region at the smallest depth"
    );
    assert_eq!(t.choose(None, Some(5)), Some(0), "root");
    assert_eq!(t.node_by_element(E2), Some(2));
    assert_eq!(t.georef(1).unwrap().to_pixel(100.0, 100.0), (2, 766));
    // Hostile input.
    assert!(MapTree::from_value(&int(1), &g.pids).is_err());
    let mut deep = node(vec![], vec![]);
    for _ in 0..(MAX_DEPTH + 2) {
        deep = node(vec![], vec![deep]);
    }
    assert!(MapTree::from_value(&deep, &g.pids).is_err());
}

const MAP_ROOT: u32 = 0x1000_0620;
const OVERLAY_A: u32 = 0x1000_0630;
const OVERLAY_B: u32 = 0x1000_0631;
const OVERLAY_C: u32 = 0x1000_0632;
const DUNGEON_IMG: u32 = 0x4100_0D00;

/// The synthetic `map` layout: P10's root with `MapPanelPadding` and `MapBackground` (with
/// `overlays` as children), `MapWindow` with `tree`, and the active elements E0–E3 (E1 has
/// a child-map button naming E2).
fn map_elements(overlays: Vec<ElementDesc>, tree: PropertyValue) -> Vec<ElementDesc> {
    let g = gv();
    let (ids, pp, vv) = (g.ids, g.pids, v().types);
    let mut bg = field(ids.map_background, 5, 26, 1024, 768);
    for o in overlays {
        bg = child(bg, o);
    }
    let panel = child(
        child(
            field(MAP_ROOT, 0, 0, 1034, 800),
            field(ids.map_panel_padding, 5, 26, 1024, 768),
        ),
        bg,
    );
    let window = prop(
        field(ids.map_window, 0, 0, 1024, 768),
        pp.map_area_data,
        tree,
    );
    let e1 = child(
        field(E1, 0, 0, 1024, 768),
        prop(
            typed(BUTTON, vv.button, 10, 10, 20, 20),
            pp.map_child_map,
            en(E2),
        ),
    );
    vec![
        panel,
        window,
        field(E0, 0, 0, 1024, 768),
        e1,
        field(E2, 0, 0, 1024, 768),
        field(E3, 0, 0, 1024, 768),
    ]
}

/// A UI with the `map` layout (and, with `radar`, the `radar` layout with an attached
/// minimap), P10 opened. Returns the UI, the dispatcher and P10's root.
fn map_ui(
    overlays: Vec<ElementDesc>,
    tree: PropertyValue,
    radar: bool,
    images: Vec<(u32, u32, u32)>,
) -> (Ui, GamePanels, ElementKey) {
    let g = gv();
    let mut layouts = vec![layout(MAP_LAYOUT, 1034, 800, map_elements(overlays, tree))];
    if radar {
        layouts.push(layout(RADAR_LAYOUT, 224, 260, radar_elements()));
    }
    let mut env = env_with(layouts);
    for (d, w, h) in images {
        add_image(&mut env, d, solid(w, h, 0xFF80_8080));
    }
    let mut ui = ui_with(env, 1034, 800);
    let mut gp = GamePanels::new(
        g,
        GameLayouts {
            map: Some(Did(MAP_LAYOUT)),
            radar: radar.then_some(Did(RADAR_LAYOUT)),
            ..GameLayouts::default()
        },
    );
    if radar {
        let r = ui.open_panel(Did(RADAR_LAYOUT), RADAR).unwrap();
        gp.attach(&mut ui, r, &SandboxData::default(), &NoShortcuts);
        assert!(gp.radars[0].parts.is_some());
    }
    let root = ui.open_panel(Did(MAP_LAYOUT), MAP_ROOT).unwrap();
    (ui, gp, root)
}

/// A valid position in region `region`, block (10, 20), cell `cell`.
fn map_pos(region: u8, cell: u16, global: (f32, f32)) -> RadarInput {
    RadarInput {
        region,
        x: 10,
        y: 20,
        cell,
        global,
        heading: 30.0,
        ..RadarInput::default()
    }
}

fn find(ui: &Ui, root: ElementKey, id: u32) -> ElementKey {
    tessera_ui::game::find_below(ui, root, id).unwrap()
}

/// Whether posting `m` from `source` raises a game-code event (the listener registry holds
/// a handler for its element ID).
fn raises_game_event(ui: &mut Ui, source: ElementKey, m: u32) -> bool {
    ui.take_events();
    ui.post(source, m, MsgArg::default());
    ui.take_events()
        .iter()
        .any(|e| matches!(e, UiEvent::Game { message } if message.msg == m))
}

#[test]
fn world_map_panel_shows_and_navigates() {
    let g = gv();
    let ids = g.ids;
    let (mut ui, mut gp, root) = map_ui(vec![], map_tree_value(), false, vec![]);
    let n = gp
        .attach_world_map(
            &mut ui,
            root,
            Some(0x10),
            &map_pos(1, 0, (0.0, 0.0)),
            &mut Maps::default(),
        )
        .unwrap();
    assert_eq!(n, Some(MapShown::Node(1)));
    let w = gp.world_map.clone().unwrap();
    let padding = find(&ui, root, ids.map_panel_padding);
    assert_eq!(ui.parent(w.map_window.unwrap()), Some(padding));
    let (_, shown) = w.shown.unwrap();
    let bg = find(&ui, root, ids.map_background);
    assert_eq!(ui.parent(shown), Some(bg));
    assert_eq!(
        ui.base_image(shown).unwrap().0,
        tessera_ui::ImageRef::Did(0x4100_0F01)
    );
    // A click on the child-map button shows node 2.
    let button = find(&ui, shown, BUTTON);
    let ev = msg(&ui, button, ui.vocab().msgs.button_click, 0);
    assert_eq!(
        gp.handle_event(&mut ui, &ev),
        vec![GameAction::MapNodeShown(2)]
    );
    assert!(!ui.exists(shown));
    let (i, k) = gp.world_map.as_ref().unwrap().shown.unwrap();
    assert_eq!((i, ui.element_id(k)), (2, Some(E2)));
    // Region only: the region's node at the smallest depth.
    let n = gp
        .attach_world_map(
            &mut ui,
            root,
            None,
            &map_pos(2, 0, (0.0, 0.0)),
            &mut Maps::default(),
        )
        .unwrap();
    assert_eq!(n, Some(MapShown::Node(2)));
}

/// §9.5 Tessera rule: the shown node's active element is the **first** child of
/// `MapBackground`, whatever the z-levels and read orders of the overlays from P10's layout.
#[test]
fn world_map_active_element_is_the_first_child_of_map_background() {
    let overlays = vec![
        z(field(OVERLAY_A, 0, 0, 10, 10), 1, 1),
        z(field(OVERLAY_B, 0, 0, 10, 10), 0, 0),
        z(field(OVERLAY_C, 0, 0, 10, 10), -1500, 9),
    ];
    let (mut ui, mut gp, root) = map_ui(overlays, map_tree_value(), false, vec![]);
    let g = gv();
    let bg = find(&ui, root, g.ids.map_background);
    let before: Vec<_> = ui.children(bg);
    assert_eq!(before.len(), 3);
    gp.attach_world_map(
        &mut ui,
        root,
        Some(0x10),
        &map_pos(1, 0, (0.0, 0.0)),
        &mut Maps::default(),
    )
    .unwrap();
    let (_, shown) = gp.world_map.as_ref().unwrap().shown.unwrap();
    let kids = ui.children(bg);
    assert_eq!(kids.len(), 4);
    assert_eq!(kids[0], shown, "the map is drawn behind its overlays");
    assert_eq!(&kids[1..], &before[..], "the overlays keep their order");
    // Navigating keeps the rule.
    let button = find(&ui, shown, BUTTON);
    let ev = msg(&ui, button, ui.vocab().msgs.button_click, 0);
    gp.handle_event(&mut ui, &ev);
    let (_, shown) = gp.world_map.as_ref().unwrap().shown.unwrap();
    let kids = ui.children(bg);
    assert_eq!(kids.len(), 4);
    assert_eq!(kids[0], shown);
    assert_eq!(ui.element_id(shown), Some(E2));
}

/// §9.4 and §9.5 Tessera rules: indoors the dungeon map is shown in the same window with its
/// own georeference; the marker carries `MinimapPlayerMarker`'s image and sits on the
/// player's pixel, turned by the heading.
#[test]
fn world_map_indoors_shows_the_dungeon_map_and_the_marker() {
    let (mut ui, mut gp, root) = map_ui(
        vec![z(field(OVERLAY_A, 0, 0, 10, 10), 1, 1)],
        map_tree_value(),
        true,
        vec![(DUNGEON_IMG, 1600, 1200)],
    );
    let g = gv();
    let bg = find(&ui, root, g.ids.map_background);
    let dungeon = GameMap {
        image: DUNGEON_IMG,
        block_x: Some(10),
        block_y: Some(20),
        pixel_x: 400,
        pixel_y: 300,
        scale: Some(3.0),
        ..GameMap::default()
    };
    let mut maps = Maps {
        dungeon: Some(dungeon.clone()),
        ..Maps::default()
    };
    let indoor = map_pos(1, 0x0100, (1_610.5, 3_205.25));
    let shown = gp
        .attach_world_map(&mut ui, root, Some(0x10), &indoor, &mut maps)
        .unwrap();
    assert_eq!(shown, Some(MapShown::Dungeon));
    let w = gp.world_map.clone().unwrap();
    assert!(w.shown.is_none());
    let d = w.dungeon.clone().unwrap();
    let kids = ui.children(bg);
    assert_eq!(kids[0], d.element, "first child of MapBackground");
    assert_eq!(
        ui.base_image(d.element).unwrap().0,
        tessera_ui::ImageRef::Did(DUNGEON_IMG)
    );
    let b = ui.layout_box(d.element).unwrap();
    assert_eq!((b.l, b.t, b.w(), b.h()), (0, 0, 1600, 1200));
    // The marker: the radar marker's image, 28 x 28, centred on (431, 285), last child.
    let m = w.marker.unwrap();
    assert_eq!(
        *kids.last().unwrap(),
        m,
        "drawn over the map and the overlays"
    );
    assert_eq!(
        ui.base_image(m).unwrap().0,
        tessera_ui::ImageRef::Did(MARKER_IMG)
    );
    assert!(ui.is_visible(m));
    let mb = ui.layout_box(m).unwrap();
    assert_eq!((mb.l, mb.t, mb.w(), mb.h()), (417, 271, 28, 28));
    assert_eq!(
        tessera_ui::game::held_rotation(&ui, m)[2],
        Radar::marker_angle(30.0)
    );
    // A position update moves the marker (3 pixels per unit east).
    let mut moved = indoor.clone();
    moved.global.0 += 10.0;
    gp.update_world_map(&mut ui, &moved, &mut maps);
    let mb = ui.layout_box(m).unwrap();
    assert_eq!((mb.l, mb.t), (447, 271));
    // An invalid position changes nothing.
    gp.update_world_map(&mut ui, &RadarInput::default(), &mut maps);
    assert_eq!(ui.layout_box(m).unwrap().l, 447);
    // Outdoors again: the node for the area replaces the dungeon map.
    let outdoor = map_pos(1, 0, (100.0, 100.0));
    let shown = gp
        .attach_world_map(&mut ui, root, Some(0x10), &outdoor, &mut maps)
        .unwrap();
    assert_eq!(shown, Some(MapShown::Node(1)));
    assert!(!ui.exists(d.element));
    let w = gp.world_map.clone().unwrap();
    assert!(w.dungeon.is_none());
    assert_eq!(ui.children(bg)[0], w.shown.unwrap().1);
    // Node 1: (100, 100) -> (2, 766) on the node's map.
    let mb = ui.layout_box(m).unwrap();
    assert_eq!((mb.l, mb.t), (2 - 14, 766 - 14));
}

/// §9.4: a pending dungeon lookup shows the outdoor choice first and the dungeon map once
/// the lookup answers; a dungeon without a map keeps the node.
#[test]
fn world_map_retries_a_pending_dungeon_map() {
    let (mut ui, mut gp, root) = map_ui(vec![], map_tree_value(), false, vec![]);
    let mut maps = Maps {
        dungeon_pending: true,
        ..Maps::default()
    };
    let indoor = map_pos(1, 0x0100, (1_600.0, 3_200.0));
    let shown = gp
        .attach_world_map(&mut ui, root, Some(0x10), &indoor, &mut maps)
        .unwrap();
    assert_eq!(shown, Some(MapShown::Node(1)));
    assert!(gp.world_map.as_ref().unwrap().indoor_pending);
    maps.dungeon_pending = false;
    maps.dungeon = Some(GameMap {
        image: DUNGEON_IMG,
        ..GameMap::default()
    });
    gp.update_world_map(&mut ui, &indoor, &mut maps);
    let w = gp.world_map.as_ref().unwrap();
    assert!(!w.indoor_pending);
    assert!(w.dungeon.is_some() && w.shown.is_none());
    assert!(w.player_pixel(indoor.global).is_none(), "no georeference");
    // A dungeon without a map: the node.
    maps.dungeon = None;
    let shown = gp
        .attach_world_map(&mut ui, root, Some(0x10), &indoor, &mut maps)
        .unwrap();
    assert_eq!(shown, Some(MapShown::Node(1)));
}

/// Attaching twice to the same P10 reuses the panel; a tree error leaves nothing behind;
/// pruning a destroyed P10 removes its child-map button handlers; errors name what is
/// missing.
#[test]
fn world_map_reattach_tree_errors_and_prune() {
    let g = gv();
    let (mut ui, mut gp, root) = map_ui(vec![], map_tree_value(), false, vec![]);
    let maps = &mut Maps::default();
    let pos = map_pos(1, 0, (0.0, 0.0));
    gp.attach_world_map(&mut ui, root, Some(0x10), &pos, maps)
        .unwrap();
    let first = gp.world_map.as_ref().unwrap().shown.unwrap().1;
    gp.attach_world_map(&mut ui, root, Some(0x10), &pos, maps)
        .unwrap();
    let padding = find(&ui, root, g.ids.map_panel_padding);
    let bg = find(&ui, root, g.ids.map_background);
    assert_eq!(ui.children(padding).len(), 1, "one MapWindow");
    assert_eq!(ui.children(bg), vec![first], "one node element");
    // Another area on the same panel: the old node element goes.
    gp.attach_world_map(&mut ui, root, Some(0x20), &pos, maps)
        .unwrap();
    assert!(!ui.exists(first));
    assert_eq!(ui.children(bg).len(), 1);
    assert_eq!(ui.children(padding).len(), 1);
    // Back to node 1 (whose button has a handler); then P10 is destroyed and pruned.
    gp.attach_world_map(&mut ui, root, Some(0x10), &pos, maps)
        .unwrap();
    let click = ui.vocab().msgs.button_click;
    let probe = ui.create_element(None, Did(MAP_LAYOUT), E1).unwrap();
    let probe_button = find(&ui, probe, BUTTON);
    assert!(raises_game_event(&mut ui, probe_button, click));
    ui.destroy(root);
    gp.prune(&mut ui);
    assert!(gp.world_map.is_none());
    assert!(
        !raises_game_event(&mut ui, probe_button, click),
        "the child-map handler is gone"
    );
    // Errors.
    let root = ui.open_panel(Did(MAP_LAYOUT), MAP_ROOT).unwrap();
    gp.attach_world_map(&mut ui, root, None, &pos, maps)
        .unwrap();
    let w = gp.world_map.as_mut().unwrap();
    assert_eq!(w.show_node(&mut ui, &g, 99), Err(GameError::NoSuchNode(99)));
    let mut no_layout = GamePanels::new(g.clone(), GameLayouts::default());
    assert_eq!(
        no_layout.attach_world_map(&mut ui, root, None, &pos, maps),
        Err(GameError::NoLayout("map"))
    );
    // A malformed tree: MapWindow is not left under MapPanelPadding.
    let (mut ui, mut gp, root) = map_ui(vec![], int(1), false, vec![]);
    assert!(
        gp.attach_world_map(&mut ui, root, None, &pos, maps)
            .is_err()
    );
    let padding = find(&ui, root, g.ids.map_panel_padding);
    assert!(ui.children(padding).is_empty());
    assert!(gp.world_map.is_none());
}

/// Pruning destroyed panels removes the game-code handlers their controllers registered
/// (the minimap's `MapZoom`, a drag box's bar and, while shown, its frame).
#[test]
fn prune_unregisters_controller_handlers() {
    let g = gv();
    let (mut ui, mut gp, root) = radar_ui(vec![]);
    let click = ui.vocab().msgs.button_click;
    let probe = ui.create_element(None, Did(RADAR_LAYOUT), RADAR).unwrap();
    let zoom = find(&ui, probe, g.ids.map_zoom);
    assert!(raises_game_event(&mut ui, zoom, click));
    ui.destroy(root);
    gp.prune(&mut ui);
    assert!(gp.radars.is_empty());
    assert!(!raises_game_event(&mut ui, zoom, click));

    let (mut ui, mut gp, root) = dbox_ui();
    let ms = ui.vocab().msgs;
    gp.toggle_drag_boxes(&mut ui);
    let probe = ui.create_element(None, Did(DBOX_LAYOUT), DBOX).unwrap();
    let bar = find(&ui, probe, g.ids.drag_box_bar);
    assert!(raises_game_event(&mut ui, probe, ms.mouse_press));
    assert!(raises_game_event(&mut ui, bar, ms.mouse_over_top));
    ui.destroy(root);
    gp.prune(&mut ui);
    assert!(gp.drag_boxes.is_empty());
    assert!(!raises_game_event(&mut ui, probe, ms.mouse_press));
    assert!(!raises_game_event(&mut ui, probe, ms.mouse_release));
    assert!(!raises_game_event(&mut ui, bar, ms.mouse_over_top));
}

/// §8.3: with an invalid position nothing changes, the labels included.
#[test]
fn radar_invalid_position_keeps_the_labels() {
    let (mut ui, mut gp, _) = radar_ui(vec![]);
    let mut maps = Maps::default();
    gp.update_radar(&mut ui, &outdoor(10, 20, (80.0, 80.0)), &mut maps);
    let p = gp.radars[0].parts.unwrap();
    assert_eq!(ui.text_of(p.area_label), "a");
    assert!(ui.is_visible(p.coord_label));
    gp.update_radar(&mut ui, &RadarInput::default(), &mut maps);
    assert_eq!(ui.text_of(p.area_label), "a");
    assert_eq!(ui.text_of(p.coord_label), "c");
    assert!(ui.is_visible(p.coord_label));
}

/// While the dungeon-map lookup for another landblock is pending, the previous dungeon's
/// map is not shown (nor used to place the map); in the same landblock it stays.
#[test]
fn radar_pending_dungeon_map_hides_the_previous_one() {
    let a = 0x4100_0A00;
    let (mut ui, mut gp, _) = radar_ui(vec![(a, 800, 600)]);
    let map_a = GameMap {
        image: a,
        block_x: Some(10),
        block_y: Some(20),
        pixel_x: 400,
        pixel_y: 300,
        scale: Some(3.0),
        ..GameMap::default()
    };
    let mut maps = Maps {
        dungeons: vec![((10, 20), map_a)],
        ..Maps::default()
    };
    let inside = |x: u8, cell: u16| RadarInput {
        region: 1,
        x,
        y: 20,
        cell,
        global: (f32::from(x) * 160.0 + 10.0, 3_205.0),
        ..RadarInput::default()
    };
    gp.update_radar(&mut ui, &inside(10, 0x0100), &mut maps);
    let p = gp.radars[0].parts.unwrap();
    assert_eq!(
        ui.base_image(p.map_image).unwrap().0,
        tessera_ui::ImageRef::Did(a)
    );
    // Another cell of the same landblock, lookup pending: the map stays.
    maps.dungeon_pending = true;
    gp.update_radar(&mut ui, &inside(10, 0x0101), &mut maps);
    assert_eq!(
        ui.base_image(p.map_image).unwrap().0,
        tessera_ui::ImageRef::Did(a)
    );
    // Another landblock, pending: dungeon A's map is not shown.
    let at = ui.layout_box(p.map_image).unwrap();
    gp.update_radar(&mut ui, &inside(11, 0x0100), &mut maps);
    assert_eq!(ui.base_image(p.map_image), None);
    let now = ui.layout_box(p.map_image).unwrap();
    assert_eq!(
        (now.l, now.t),
        (at.l, at.t),
        "not placed with A's georeference"
    );
    // Dungeon A, outdoors, then a pending interior of the same landblock: cleared.
    maps.dungeon_pending = false;
    gp.update_radar(&mut ui, &inside(10, 0x0100), &mut maps);
    assert!(ui.base_image(p.map_image).is_some());
    gp.update_radar(&mut ui, &outdoor(10, 20, (10.0, 10.0)), &mut maps);
    maps.dungeon_pending = true;
    gp.update_radar(&mut ui, &inside(10, 0x0100), &mut maps);
    assert_eq!(ui.base_image(p.map_image), None);
    assert!(gp.radars[0].indoor.is_none());
}

/// A map whose scale is NaN (corrupt data) is the same map as itself, so that the minimap
/// does not set its image again on every position update.
#[test]
fn game_map_with_a_nan_scale_equals_itself() {
    let m = GameMap {
        image: 1,
        scale: Some(f32::NAN),
        ..GameMap::default()
    };
    assert!(m.same_as(&m.clone()));
    let mut other = m.clone();
    other.scale = Some(1.0);
    assert!(!m.same_as(&other));
    other.scale = None;
    assert!(!m.same_as(&other));
}

/// `tile_georef` is public: an item index far outside the grid gives `None`, never an
/// overflow.
#[test]
fn tile_georef_does_not_overflow() {
    use tessera_ui::game::map::tile_georef;
    let m = tile_map(1, 1);
    assert_eq!(tile_georef(20_000_000, usize::MAX, (0, 0), &m), None);
    assert_eq!(tile_georef(usize::MAX, 1, (0, 0), &m), None);
    assert_eq!(tile_georef(3 * 10_737_418, 3, (0, 0), &m), None);
    let g = tile_georef(4, 3, (0, 0), &m).unwrap();
    assert_eq!((g.pixel_x, g.pixel_y), (200, 400));
}

/// §10.2: a rebuild hides every empty slot, also during a drag; the next drag shows them.
#[test]
fn quickslot_rebuild_during_a_drag_hides_empty_slots() {
    let (mut ui, mut gp) = qs_ui(0, &NoShortcuts);
    let g = gv();
    let list = gp.bars[0].list.unwrap();
    gp.handle_event(&mut ui, &UiEvent::DragBegan { source: list });
    assert!(ui.list_entries(list).iter().all(|e| ui.is_visible(*e)));
    gp.bars[0].rebuild(&mut ui, &g, &NoShortcuts).unwrap();
    let entries = ui.list_entries(list);
    assert_eq!(entries.len(), 12);
    assert!(entries.iter().all(|e| !ui.is_visible(*e)));
    gp.handle_event(
        &mut ui,
        &UiEvent::DragEnded {
            source: list,
            target: None,
            accepted: false,
        },
    );
    gp.handle_event(&mut ui, &UiEvent::DragBegan { source: list });
    assert!(entries.iter().all(|e| ui.is_visible(*e)));
    // Errors name what is missing.
    let mut bar = gp.bars[0].clone();
    bar.layout = None;
    assert_eq!(
        bar.rebuild(&mut ui, &g, &NoShortcuts),
        Err(GameError::NoLayout("quickslots"))
    );
    bar.list = None;
    assert_eq!(
        bar.rebuild(&mut ui, &g, &NoShortcuts),
        Err(GameError::MissingElement(QS_LIST))
    );
    bar.list_id = 0;
    assert_eq!(
        bar.rebuild(&mut ui, &g, &NoShortcuts),
        Err(GameError::MissingProperty(g.pids.quickslot_listbox))
    );
}

/// §6.1: a show request on a field whose hide test is true is refused; a hide request
/// always hides; a change of a condition property on the element re-evaluates it.
#[test]
fn hide_when_field_show_requests_and_property_changes() {
    let g = gv();
    let p = g.pids;
    let ty = g.types.hide_when_field;
    let monster = prop(
        typed(0x1000_0303, ty, 0, 0, 20, 10),
        p.hw_monster_player,
        b(true),
    );
    let plain = typed(0x1000_0305, ty, 0, 0, 20, 10);
    let other = field(0x1000_0306, 0, 0, 20, 10);
    let mut root = field(0x1000_0300, 0, 0, 100, 100);
    for c in [monster, plain, other] {
        root = child(root, c);
    }
    let mut ui = ui_one(HW_LAYOUT, 100, 100, vec![root], 800, 600);
    let k = ui.open_panel(Did(HW_LAYOUT), 0x1000_0300).unwrap();
    let mut gp = GamePanels::new(g.clone(), GameLayouts::default());
    let mut data = SandboxData::default();
    data.conditions.push(HideCondition::MonsterPlayer);
    gp.attach(&mut ui, k, &data, &NoShortcuts);
    let monster = find(&ui, k, 0x1000_0303);
    let plain = find(&ui, k, 0x1000_0305);
    let other = find(&ui, k, 0x1000_0306);
    assert!(!ui.is_visible(monster));
    assert!(gp.request_shown(&mut ui, monster, true, &data));
    assert!(!ui.is_visible(monster), "show refused while the test hides");
    assert!(gp.request_shown(&mut ui, plain, false, &data));
    assert!(!ui.is_visible(plain), "hide always hides");
    assert!(gp.request_shown(&mut ui, plain, true, &data));
    assert!(ui.is_visible(plain));
    assert!(
        !gp.request_shown(&mut ui, other, false, &data),
        "not a field"
    );
    assert!(ui.is_visible(other));
    // The condition property changes on the element.
    ui.set_property(monster, p.hw_monster_player, b(false));
    assert!(gp.element_property_changed(&mut ui, monster, p.hw_monster_player, &data));
    assert!(ui.is_visible(monster));
    assert!(!gp.element_property_changed(&mut ui, monster, 0x1234, &data));
    ui.set_property(plain, p.hw_monster_player, b(true));
    assert!(gp.element_property_changed(&mut ui, plain, p.hw_monster_player, &data));
    assert!(!ui.is_visible(plain));
}

// ----- Escape (§16, §13.7, GP15, GP16) --------------------------------------------------

const ESC_LAYOUT: u32 = 0x22FF_3600;

#[test]
fn example_13_7_escape_and_gp15_gp16() {
    let hud = 0x1000_0700;
    let chat = 0x1000_0701;
    let menu = 0x1000_0702;
    let quests = 0x1000_0703;
    let els = [hud, chat, menu, quests]
        .iter()
        .map(|id| field(*id, 0, 0, 50, 50))
        .collect();
    let mut ui = ui_one(ESC_LAYOUT, 50, 50, els, 800, 600);
    let open = |ui: &mut Ui, id| ui.open_panel(Did(ESC_LAYOUT), id).unwrap();
    let hud = open(&mut ui, hud);
    let chat = open(&mut ui, chat);
    let menu = open(&mut ui, menu);
    let quests = open(&mut ui, quests);
    ui.set_shown(menu, false);
    ui.set_shown(quests, false);
    let panels = EscapePanels {
        on_demand: vec![quests],
        main_menu: Some(menu),
    };
    // Step 1.
    ui.activate(hud);
    assert_eq!(
        declined_escape(&mut ui, &panels, true, false),
        EscapeOutcome::ShowedMainMenu(menu)
    );
    assert_eq!(ui.active_panel(), Some(menu));
    // Step 2.
    assert_eq!(
        declined_escape(&mut ui, &panels, true, false),
        EscapeOutcome::HidActive(menu)
    );
    assert!(!ui.is_visible(menu));
    // Step 3.
    ui.set_shown(quests, true);
    ui.activate(quests);
    assert_eq!(
        declined_escape(&mut ui, &panels, true, false),
        EscapeOutcome::HidActive(quests)
    );
    assert!(ui.is_visible(hud) && ui.is_visible(chat), "HUD panels stay");
    // Step 5.
    ui.activate(chat);
    assert_eq!(
        declined_escape(&mut ui, &panels, true, false),
        EscapeOutcome::ShowedMainMenu(menu)
    );
    // Releases and repeats do nothing.
    assert_eq!(
        declined_escape(&mut ui, &panels, false, false),
        EscapeOutcome::Ignored
    );
    assert_eq!(
        declined_escape(&mut ui, &panels, true, true),
        EscapeOutcome::Ignored
    );
    // P6 not created yet.
    let none = EscapePanels::default();
    ui.activate(hud);
    assert_eq!(
        declined_escape(&mut ui, &none, true, false),
        EscapeOutcome::OpenMainMenu
    );
}

#[test]
fn gp_data_defaults_are_the_sandbox() {
    let d = SandboxData::default();
    assert!(d.has_local_player());
    assert!(d.player_property(1).is_none());
    assert_eq!(NoShortcuts.shortcut(0), Shortcut::Empty);
}
