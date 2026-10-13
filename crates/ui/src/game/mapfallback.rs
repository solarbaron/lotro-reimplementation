// SPDX-License-Identifier: GPL-3.0-or-later
//! Fallback map widgets (Tessera rule, decision 0031 tier 1 and runs without an install):
//! the map window's two roots, the elements the map window creates from the `map` layout
//! (a node's active element, `DungeonMap`, `MapPlayerMarker`), and a minimap, all built
//! from code-made elements that carry the install's element IDs ([`GameVocab`],
//! [`P5Vocab`]) at the design rects `map-windows.md` §3.2–§3.3 and `game-panels.md` §8.1
//! give, with the client's art ([`P5Art`]). The controllers attach to these trees exactly
//! as to the install's.
//!
//! `map-windows.md` §8 has an implementer OPEN: the spec defines no fallback map window.
//! Until it does, this module's tree is Tessera's provisional reading: the elements of
//! §3.2's table with their rects and z-levels, `MapPanel` reduced to a frame, the title and
//! `MapPanelPadding` at (5, 26), and no child-map buttons in a fallback active element.

use tessera_formats::property::PropertyValue;

use crate::env::ImageRef;
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};
use crate::widgets::WidgetClass;

use super::GameError;
use super::mapmarks::{MapLabel, MapTexts, map_label};
use super::npc::kit::{FNode, FallbackArt, colours, instantiate, open_root};
use super::npcvocab::NpcVocab;
use super::p5vocab::P5Vocab;
use super::vocab::GameVocab;

/// The art of the P5 fallback widgets: generated or original CC0 images the client (or a
/// test) provides by [`ImageRef`]. Every field is optional; a missing image draws nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct P5Art {
    /// The P3 fallback art (window, title bar, panel, rows, buttons, close, tabs, entry,
    /// slot, coins).
    pub base: FallbackArt,
    /// `MapPadding`'s background art (behind the map).
    pub map_padding: Option<ImageRef>,
    /// `MapDecoration`'s frame art (in front of the map; drawn with alpha).
    pub map_decoration: Option<ImageRef>,
    /// The player marker (`MapPlayerMarker`; the minimap's `MinimapPlayerMarker`).
    pub player_marker: Option<ImageRef>,
    /// `DungeonMap_OverlayField`'s frame.
    pub dungeon_frame: Option<ImageRef>,
    /// A check mark (popup check items, the finder's check marks).
    pub check: Option<ImageRef>,
    /// The travel row's swift icon (20 × 20).
    pub swift: Option<ImageRef>,
    /// The travel row's unknown-destination overlay (400 × 22).
    pub unknown: Option<ImageRef>,
    /// The travel row's failed-requirements overlay (400 × 22).
    pub failed: Option<ImageRef>,
    /// The travel row's ghosted overlay.
    pub ghosted: Option<ImageRef>,
    /// The minimap's rim.
    pub rim: Option<ImageRef>,
    /// An alert icon (the queue alert).
    pub alert: Option<ImageRef>,
}

/// Sets an FNode's z-level.
pub(crate) fn with_z(mut n: FNode, z: i32) -> FNode {
    n.desc.block.z_level = z;
    n
}

/// Gives an FNode an object mode.
pub(crate) fn with_mode(n: FNode, ui: &Ui, mode: u32) -> FNode {
    let p = ui.vocab().pids.object_mode;
    n.prop(p, PropertyValue::Enum(mode))
}

/// A small fallback button with a label.
fn button(
    ui: &Ui,
    nv: &NpcVocab,
    art: &P5Art,
    id: u32,
    rect: (i32, i32, i32, i32),
    t: &str,
) -> FNode {
    FNode::button(
        id,
        rect.0,
        rect.1,
        rect.2,
        rect.3,
        (&art.base.button, &art.base.button_ghosted),
        nv.pids.text_font_color,
        ui.vocab().states.ghosted,
        t,
    )
}

/// The full-screen root `MapWindow` with `MapField` and its parts (`map-windows.md` §3.2).
fn map_window_node(
    ui: &Ui,
    gv: &GameVocab,
    pv: &P5Vocab,
    nv: &NpcVocab,
    art: &P5Art,
    texts: &dyn MapTexts,
) -> FNode {
    let (ids, p) = (pv.ids, nv.pids);
    // Tessera's fallback reading: `MapBackground` is drawn in `MapWindow`'s canvas (no
    // object mode), since the fallback window is shown at its design size.
    let background =
        with_z(FNode::field(gv.ids.map_background, 0, 0, 1024, 768), -1500).children([
            with_z(FNode::field(ids.objective_indicators, 0, 0, 1024, 768), -5).hidden(),
            with_z(
                FNode::text(
                    ids.coordinate_display,
                    52,
                    730,
                    450,
                    25,
                    p.text_font_color,
                    colours::WHITE,
                    "",
                ),
                -10,
            ),
            with_z(FNode::field(ids.quest_guide_field, 38, 714, 188, 20), -1500)
                .hidden()
                .child(FNode::field(ids.quest_guide_checkbox, 0, 0, 20, 20)),
        ]);
    let menus = with_z(FNode::field(ids.menu_scale_parent, 52, 18, 800, 25), -1500).children([
        button(
            ui,
            nv,
            art,
            ids.filter_menu,
            (0, 0, 150, 25),
            &map_label(texts, MapLabel::FilterMenu),
        ),
        button(
            ui,
            nv,
            art,
            ids.map_menu,
            (151, 0, 150, 25),
            &map_label(texts, MapLabel::MapMenu),
        ),
        button(ui, nv, art, ids.stablemaster_button, (302, 0, 200, 23), "").hidden(),
    ]);
    let field = with_z(FNode::field(ids.map_field, 0, 0, 1024, 768), -1500).children([
        with_z(FNode::field(ids.map_padding, 0, 0, 1024, 768), -1400).image(&art.map_padding),
        background,
        menus,
    ]);
    let close = |id: u32, x: i32| {
        FNode::button(
            id,
            x,
            0,
            16,
            16,
            (&art.base.close, &art.base.close),
            p.text_font_color,
            ui.vocab().states.ghosted,
            "",
        )
    };
    with_z(FNode::field(gv.ids.map_window, 0, 0, 1024, 768), -1500).children([
        field,
        with_z(FNode::field(ids.map_decoration, -73, 0, 1168, 768), -1600)
            .image(&art.map_decoration),
        with_z(FNode::field(ids.map_buttons, 985, 2, 39, 16), -3000).children([
            close(ids.fullscreen_collapse, 0),
            close(ids.toggle_map_panel, 20),
        ]),
    ])
}

/// The windowed root `MapPanel` (`map-windows.md` §3.2: 1,038 × 802, `MapPanelPadding` at
/// (5, 26, 1,024, 768)).
fn map_panel_node(
    ui: &Ui,
    gv: &GameVocab,
    pv: &P5Vocab,
    nv: &NpcVocab,
    art: &P5Art,
    texts: &dyn MapTexts,
) -> FNode {
    let (ids, p) = (pv.ids, nv.pids);
    let small = |id: u32, x: i32| {
        FNode::button(
            id,
            x,
            5,
            16,
            16,
            (&art.base.close, &art.base.close),
            p.text_font_color,
            ui.vocab().states.ghosted,
            "",
        )
    };
    FNode::field(ids.map_panel, 0, 0, 1038, 802)
        .image(&art.base.window)
        .children([
            FNode::field(0, 0, 0, 1038, 26)
                .image(&art.base.title_bar)
                .child(FNode::text(
                    ids.panel_title,
                    10,
                    5,
                    600,
                    16,
                    p.text_font_color,
                    colours::TITLE,
                    &map_label(texts, MapLabel::Title),
                )),
            small(ids.fullscreen_expand, 996),
            small(ids.toggle_map_panel, 1016),
            FNode::field(gv.ids.map_panel_padding, 5, 26, 1024, 768),
        ])
}

/// Opens the fallback map window: `MapWindow` (full screen, shown) and `MapPanel`
/// (windowed, hidden). Returns (`MapWindow`, `MapPanel`).
///
/// # Errors
/// The element cap or an attachment error.
pub fn open_map_window(
    ui: &mut Ui,
    gv: &GameVocab,
    pv: &P5Vocab,
    nv: &NpcVocab,
    art: &P5Art,
    texts: &dyn MapTexts,
) -> Result<(ElementKey, ElementKey), GameError> {
    let w = map_window_node(ui, gv, pv, nv, art, texts);
    let window = open_root(ui, &w)?;
    let p = map_panel_node(ui, gv, pv, nv, art, texts);
    let panel = match open_root(ui, &p) {
        Ok(k) => k,
        Err(e) => {
            ui.destroy(window);
            return Err(e);
        }
    };
    ui.set_shown(panel, false);
    Ok((window, panel))
}

/// A fallback active element (a node's map, `map-windows.md` §4.2): a Field of the pixel
/// frame with the node's element ID; the map image is set by the map window.
pub(crate) fn active_element(
    ui: &mut Ui,
    parent: ElementKey,
    id: u32,
    frame: (i32, i32),
) -> Option<ElementKey> {
    instantiate(ui, parent, &FNode::field(id, 0, 0, frame.0, frame.1))
}

/// A fallback `DungeonMap` (`map-windows.md` §3.3, §4.7): `DungeonMap_ImageField` at (112,
/// 84, 800, 600) in object mode 1 and the frame `DungeonMap_OverlayField` over the whole
/// 1,024 × 768, read after the image field (so in front).
pub(crate) fn dungeon_map(
    ui: &mut Ui,
    pv: &P5Vocab,
    art: &P5Art,
    parent: ElementKey,
) -> Option<ElementKey> {
    let ids = pv.ids;
    let node = with_z(FNode::field(ids.dungeon_map, 0, 0, 1024, 768), -500).children([
        with_mode(
            FNode::field(ids.dungeon_image, 112, 84, 800, 600),
            ui,
            object_mode::STRETCH,
        ),
        FNode::field(ids.dungeon_overlay, 0, 0, 1024, 768).image(&art.dungeon_frame),
    ]);
    instantiate(ui, parent, &node)
}

/// A fallback `MapPlayerMarker` (`map-windows.md` §3.3: 28 × 28, z −500, object mode 1).
pub(crate) fn player_marker(
    ui: &mut Ui,
    pv: &P5Vocab,
    art: &P5Art,
    parent: ElementKey,
) -> Option<ElementKey> {
    let n = with_mode(
        with_z(FNode::field(pv.ids.player_marker, 0, 0, 28, 28), -500),
        ui,
        object_mode::STRETCH,
    )
    .image(&art.player_marker);
    instantiate(ui, parent, &n)
}

/// A fallback minimap (`game-panels.md` §8.1): a root Field of the radar's type holding
/// `MinimapViewport` (27, 0, 170, 170) with `RadarMapImage` (a horizontal 3 × 3 list
/// box), `MinimapMapImage`, the fog elements, `MinimapPlayerMarker` (0, 3, 28, 28), the
/// rim, and the two labels below. [`super::radar::Radar::post_create`] attaches to it as
/// to the install's `radar` layout; there are no tiles without that layout (the indoor
/// map is shown).
///
/// # Errors
/// The element cap or an attachment error.
pub fn minimap(
    ui: &mut Ui,
    gv: &GameVocab,
    nv: &NpcVocab,
    art: &P5Art,
) -> Result<ElementKey, GameError> {
    let ids = gv.ids;
    let v = ui.vocab().clone();
    let cp = nv.pids.text_font_color;
    let tiles = FNode::new(WidgetClass::ListBox, ids.radar_map_image, 0, 0, 600, 600)
        .prop(v.pids.lb_horizontal, PropertyValue::Bool(true))
        .prop(v.pids.lb_max_columns, PropertyValue::Int(3))
        .prop(v.pids.lb_max_rows, PropertyValue::Int(3));
    let viewport = with_mode(
        FNode::field(ids.minimap_viewport, 27, 0, 170, 170),
        ui,
        object_mode::CANVAS_RESIZE,
    )
    .image(&art.base.panel)
    .children([
        tiles,
        FNode::field(ids.minimap_map_image, 0, -1, 2048, 1536),
        FNode::field(ids.fog_display, 0, 0, 2048, 1536).child(
            FNode::field(ids.fog_parent, 0, 0, 2048, 1536).child(FNode::field(
                ids.fog_field,
                0,
                0,
                2048,
                1536,
            )),
        ),
        with_z(
            with_mode(
                FNode::field(ids.player_marker, 0, 3, 28, 28),
                ui,
                object_mode::STRETCH,
            ),
            -100,
        )
        .image(&art.player_marker),
        FNode::field(ids.quest_objective, 0, 0, 30, 30).hidden(),
    ]);
    let root = FNode::field(0, 0, 0, 224, 244)
        .typed(gv.types.radar)
        .children([
            viewport,
            FNode::field(ids.minimap_frame, -7, -33, 237, 237).image(&art.rim),
            FNode::text(ids.area_label, 27, 200, 170, 20, cp, colours::TITLE, ""),
            FNode::text(
                ids.coordinate_label,
                12,
                220,
                200,
                20,
                cp,
                colours::WHITE,
                "",
            ),
            FNode::field(ids.area_quest_highlight, 0, 194, 224, 32).hidden(),
        ]);
    open_root(ui, &root)
}
