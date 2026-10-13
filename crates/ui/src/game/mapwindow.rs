// SPDX-License-Identifier: GPL-3.0-or-later
//! The map window of `map-windows.md` (decision 0024 P5): its two presentations (§3.1),
//! the fill and visibility rules of its controls (§3.4), navigation over the map tree
//! (§4.2–§4.5: showing a node, child maps, going up, the map menu), the player's map
//! (§4.6), dungeon maps stretched into `DungeonMap_ImageField` (§4.7), the overlays in the
//! order of §6.2 (notes, feed markers, the player marker), the note-type filter menu
//! (§6.9), `CoordinateDisplay` (§3.4), the presentation of place lookup (§7.3) and the
//! census helpers (§5.4).
//!
//! The view works on the install's `map` layout ([`MapWindowView::open_layout`]) or on
//! Tessera's fallback widgets ([`MapWindowView::open_fallback`]); both carry the same
//! element IDs. It links no data layer: the client gives the tree (read from `MapWindow`
//! by [`MapTree::from_value`] and [`MapTree::read_options`]), the notes of each shown map,
//! the icon table, the filter values, the menu labels and the player's state, and turns
//! the returned [`MapAction`]s into settings writes and note queries.
//!
//! For the P3 quest markers ([`super::quest::markers::QuestMarkers::update_world_map`]) the
//! view keeps a [`WorldMapPanel`] describing what it shows ([`MapWindowView::panel`]).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::env::{ImageRef, ImageState};
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

use super::map::{BLOCK_UNITS, GameMap, Georef, MapPoint};
use super::mapfallback::{self, P5Art};
use super::mapmarks::{
    FeedMarker, FilterItem, FilterMenuModel, IconTable, LOOKUP_MARKER_ID, MapContext, MapNote,
    MapOverlays, MapTexts, NoteShape, OverlayId, collation_key, map_label, read_filter,
};
use super::npc::kit::{ClickKind, put_text};
use super::npcvocab::NpcVocab;
use super::p5vocab::P5Vocab;
use super::popup::{Place, Popup, PopupItem, click};
use super::radar::Radar;
use super::vocab::GameVocab;
use super::worldmap::{DungeonView, MapTree, NodeOptions, WorldMapPanel};
use super::{GameError, find_below, set_image, show, subtree};

/// Draw mode of a node's image (`map-windows.md` MW4: always 2) and of a dungeon map's.
pub const NODE_IMAGE_MODE: u32 = 2;

/// The dungeon map's image field size (`map-windows.md` §3.3: 800 × 600).
pub const DUNGEON_FIELD: (i32, i32) = (800, 600);

/// The node pixel frame when a node gives none (1,024 × 768 in every node here).
pub const DEFAULT_FRAME: (i32, i32) = (1024, 768);

/// The map window's design size (`MapWindow`, 1,024 × 768).
pub const WINDOW_SIZE: (i32, i32) = (1024, 768);

/// The two presentations (`map-windows.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// `MapWindow` shown (the default of `GOP_MapInFullscreenMode`).
    FullScreen,
    /// `MapPanel` shown with `MapField` under `MapPanelPadding`.
    Windowed,
}

/// What the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// A node of the tree.
    Node(usize),
    /// A top-level element that is no node's active element (§4.2 step 3).
    Element(u32),
    /// A dungeon map (§4.7).
    Dungeon,
}

/// Something the client does in answer to the map window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAction {
    /// A map is shown (§4.2 step 5): the client queries that map's notes (an area query
    /// for a node, a zone query for a dungeon map, `map-data.md` §4.8.1) and gives them to
    /// [`MapWindowView::set_notes`].
    Shown(Shown),
    /// The user changed a filter value (§6.9): the client stores it in Tessera's settings
    /// under the option `option` (a property ID).
    FilterChanged {
        /// The option's property ID.
        option: u32,
        /// The new value.
        value: u64,
    },
    /// The presentation was switched (§3.1): the client stores `map_fullscreen`.
    PresentationChanged {
        /// The new presentation is full screen.
        full_screen: bool,
    },
    /// A close control (`Toggle_Map_Panel`, `Toggle_FOW`) was clicked: the client hides
    /// the window.
    Close,
}

/// The player's state the map window reads (`map-windows.md` §4.6, §6.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerMap {
    /// Where the player is.
    pub point: MapPoint,
    /// Compass heading in degrees, clockwise from north.
    pub heading: f32,
    /// Whether the position is valid (CO §4).
    pub valid: bool,
    /// The area of the player's position (`None`: unknown or listed by no node).
    pub area: Option<u32>,
    /// The player's dungeon and its map data (§4.6 Tessera rules), when in one.
    pub dungeon: Option<(u32, GameMap)>,
}

/// What place lookup shows (`map-windows.md` §7.3 "Result").
#[derive(Debug, Clone, PartialEq)]
pub enum LookupTarget {
    /// A node: show it.
    Node(usize),
    /// A dungeon map.
    Dungeon(GameMap),
    /// A position or place key: the player's-map choice of §4.6 step 3 for its area.
    Area(Option<u32>),
}

/// Which popup is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupKind {
    Filter,
    Maps,
}

/// The parts the window finds by element ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapParts {
    /// `MapField`.
    pub field: Option<ElementKey>,
    /// `MapBackground`.
    pub background: Option<ElementKey>,
    /// `MapPanelPadding` (windowed root).
    pub padding: Option<ElementKey>,
    /// `FilterMenu`.
    pub filter_menu: Option<ElementKey>,
    /// `MapMenu`.
    pub map_menu: Option<ElementKey>,
    /// `StablemasterButton`.
    pub stablemaster: Option<ElementKey>,
    /// `CoordinateDisplay`.
    pub coordinates: Option<ElementKey>,
    /// `QuestGuideOptionField`.
    pub quest_guide: Option<ElementKey>,
    /// `MapQuestObjectiveIndicatorsParent`.
    pub indicators: Option<ElementKey>,
    /// `ToggleMapFullscreenCollapse` (in `MapWindow`).
    pub collapse: Option<ElementKey>,
    /// `ToggleMapFullscreenExpand` (in `MapPanel`).
    pub expand: Option<ElementKey>,
}

/// The dungeon map shown (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct DungeonParts {
    /// `DungeonMap`.
    pub root: ElementKey,
    /// `DungeonMap_ImageField`.
    pub image: ElementKey,
    /// The map data.
    pub map: GameMap,
    /// The image's pixel size once known (the canvas is sized to it and stretched).
    pub size: Option<(i32, i32)>,
}

/// The map window.
#[derive(Debug, Clone)]
pub struct MapWindowView {
    /// What the window shows, as the P3 quest markers read it (`root` is the root that
    /// holds `MapField` now).
    pub panel: WorldMapPanel,
    /// `MapWindow` (the full-screen root and the tree's holder).
    pub window: ElementKey,
    /// `MapPanel` (the windowed root), when opened.
    pub windowed_root: Option<ElementKey>,
    /// The parts.
    pub parts: MapParts,
    /// The presentation.
    pub presentation: Presentation,
    /// What is shown.
    pub shown: Option<Shown>,
    /// The shown map element (the active element, or the dungeon's image field).
    pub shown_element: Option<ElementKey>,
    /// The dungeon map, when shown.
    pub dungeon: Option<DungeonParts>,
    /// The player marker, when created.
    pub marker: Option<ElementKey>,
    /// The overlays.
    pub overlays: MapOverlays,
    /// The window's icon table (`UI_MapNoteIcons_Map`).
    pub icons: IconTable,
    /// The filter menu of the shown map.
    pub filter_menu: FilterMenuModel,
    /// The filter value in use (read, §6.9).
    pub filter: u64,
    /// The option the shown map's filter is stored in (a property ID).
    pub filter_option: u32,
    /// Whether `CoordinateDisplay` is wanted (Tessera's `map_coordinates`).
    pub show_coordinates: bool,
    /// The shape of `MapNoteTemplate`.
    pub note_shape: NoteShape,
    filter_values: std::collections::BTreeMap<u32, u64>,
    regions: Vec<(u32, String)>,
    node_names: Vec<Option<String>>,
    recorded_dungeon: Option<u32>,
    player: Option<PlayerMap>,
    popup: Option<(PopupKind, Popup)>,
    /// The map menu's open region submenu: (region entry, popup).
    sub: Option<(usize, Popup)>,
    /// The nodes of the open submenu's items, in item order.
    menu_targets: Vec<usize>,
    layout: Option<Did>,
    art: P5Art,
    gv: GameVocab,
    pv: P5Vocab,
    nv: NpcVocab,
    handlers: Vec<(u32, u32)>,
    node_handlers: Vec<(u32, u32)>,
}

/// Pixel frame of node `n`.
fn node_frame(t: &MapTree, n: usize) -> (i32, i32) {
    t.nodes.get(n).map_or(DEFAULT_FRAME, |x| {
        if x.pixel_size.0 > 0 && x.pixel_size.1 > 0 {
            x.pixel_size
        } else {
            DEFAULT_FRAME
        }
    })
}

/// The test point of the census (`map-windows.md` §5.4 step 2): the frame's world
/// rectangle (by the inverse transform of its corner pixels) intersected with the valid
/// block rectangle (0 ≤ *G* < 255 · 160 on both axes), its centre; `None` when the
/// intersection is empty (category C5) or the scale is 0.
#[must_use]
pub fn census_test_point(g: &Georef, frame: (i32, i32)) -> Option<(f32, f32)> {
    let (x0, y0) = g.to_world(0, 0)?;
    let (x1, y1) = g.to_world(frame.0, frame.1)?;
    let max = 255.0 * BLOCK_UNITS;
    let span = |a: f32, b: f32| {
        let (lo, hi) = (a.min(b).max(0.0), a.max(b).min(max));
        (lo < hi).then_some((lo + hi) / 2.0)
    };
    Some((span(x0, x1)?, span(y0, y1)?))
}

impl MapWindowView {
    /// Opens the install's map window from the `map` layout: `MapWindow` (full screen) and
    /// `MapPanel` (windowed, hidden), reads the tree and its options from `MapWindow`'s
    /// `UI_Map_AreaData` and attaches.
    ///
    /// # Errors
    /// A creation error, or a tree error ([`MapTree::from_value`]).
    pub fn open_layout(
        ui: &mut Ui,
        gv: &GameVocab,
        pv: &P5Vocab,
        nv: &NpcVocab,
        layout: Did,
        art: &P5Art,
    ) -> Result<Self, GameError> {
        let window = ui.open_panel(layout, gv.ids.map_window)?;
        let tree = match ui.property(window, gv.pids.map_area_data).cloned() {
            Some(v) => match MapTree::from_value(&v, &gv.pids) {
                Ok(mut t) => {
                    t.read_options(&v, &gv.pids, &pv.pids);
                    t
                }
                Err(e) => {
                    ui.destroy(window);
                    return Err(e);
                }
            },
            None => MapTree::default(),
        };
        let panel = match ui.open_panel(layout, pv.ids.map_panel) {
            Ok(k) => {
                ui.set_shown(k, false);
                Some(k)
            }
            Err(e) => {
                tracing::debug!("map window: no windowed root: {e}");
                None
            }
        };
        Ok(Self::attach(
            ui,
            gv,
            pv,
            nv,
            window,
            panel,
            tree,
            Some(layout),
            art,
        ))
    }

    /// Opens Tessera's fallback map window ([`mapfallback::open_map_window`]) over `tree`
    /// (a synthetic tree, or the install's tree without its layout).
    ///
    /// # Errors
    /// The element cap or an attachment error.
    pub fn open_fallback(
        ui: &mut Ui,
        gv: &GameVocab,
        pv: &P5Vocab,
        nv: &NpcVocab,
        tree: MapTree,
        art: &P5Art,
        texts: &dyn MapTexts,
    ) -> Result<Self, GameError> {
        let (window, panel) = mapfallback::open_map_window(ui, gv, pv, nv, art, texts)?;
        Ok(Self::attach(
            ui,
            gv,
            pv,
            nv,
            window,
            Some(panel),
            tree,
            None,
            art,
        ))
    }

    /// Attaches to opened roots (`MapWindow` shown, `MapPanel` hidden) with a tree. The
    /// `layout` names the `map` layout the window creates active elements, notes, the
    /// player marker and `DungeonMap` from (`None`: Tessera's fallback elements). The
    /// window starts in the full-screen presentation with nothing shown; the quest-guide
    /// parts and `StablemasterButton` are hidden (§3.4 and §6.5 Tessera rules).
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn attach(
        ui: &mut Ui,
        gv: &GameVocab,
        pv: &P5Vocab,
        nv: &NpcVocab,
        window: ElementKey,
        windowed_root: Option<ElementKey>,
        tree: MapTree,
        layout: Option<Did>,
        art: &P5Art,
    ) -> Self {
        let ids = pv.ids;
        let f =
            |ui: &Ui, root: Option<ElementKey>, id: u32| root.and_then(|r| find_below(ui, r, id));
        let w = Some(window);
        let parts = MapParts {
            field: f(ui, w, ids.map_field),
            background: f(ui, w, gv.ids.map_background),
            padding: f(ui, windowed_root, gv.ids.map_panel_padding),
            filter_menu: f(ui, w, ids.filter_menu),
            map_menu: f(ui, w, ids.map_menu),
            stablemaster: f(ui, w, ids.stablemaster_button),
            coordinates: f(ui, w, ids.coordinate_display),
            quest_guide: f(ui, w, ids.quest_guide_field),
            indicators: f(ui, w, ids.objective_indicators),
            collapse: f(ui, w, ids.fullscreen_collapse),
            expand: f(ui, windowed_root, ids.fullscreen_expand),
        };
        // MW1: the indicators are hidden at once; Tessera rules hide the quest guide and
        // the stable-master button (§3.4, §6.5).
        for k in [parts.indicators, parts.quest_guide, parts.stablemaster] {
            show(ui, k, false);
        }
        let note_shape = Self::read_note_shape(ui, pv, layout);
        let mut v = Self {
            panel: WorldMapPanel {
                root: window,
                layout: layout.unwrap_or(Did(0)),
                map_window: Some(window),
                tree: Some(tree),
                shown: None,
                dungeon: None,
                buttons: Vec::new(),
                marker: None,
                indoor_pending: false,
            },
            window,
            windowed_root,
            parts,
            presentation: Presentation::FullScreen,
            shown: None,
            shown_element: None,
            dungeon: None,
            marker: None,
            overlays: MapOverlays::default(),
            icons: IconTable::default(),
            filter_menu: FilterMenuModel::default(),
            filter: 0,
            filter_option: pv.pids.gop_map_filter,
            show_coordinates: true,
            note_shape,
            filter_values: std::collections::BTreeMap::new(),
            regions: Vec::new(),
            node_names: Vec::new(),
            recorded_dungeon: None,
            player: None,
            popup: None,
            sub: None,
            menu_targets: Vec::new(),
            layout,
            art: art.clone(),
            gv: gv.clone(),
            pv: pv.clone(),
            nv: nv.clone(),
            handlers: Vec::new(),
            node_handlers: Vec::new(),
        };
        let ms = ui.vocab().msgs;
        let mut ids_clicked = vec![
            ids.filter_menu,
            ids.map_menu,
            ids.fullscreen_collapse,
            ids.fullscreen_expand,
            ids.toggle_map_panel,
            ids.toggle_fow,
            gv.ids.map_background,
            ids.note_template,
            ids.note_button,
            ids.player_marker,
            ids.dungeon_map,
            ids.dungeon_image,
            ids.dungeon_overlay,
        ];
        ids_clicked.dedup();
        for id in ids_clicked {
            for m in [ms.button_click, ms.mouse_click, ms.mouse_dbl_click] {
                if id != 0 {
                    ui.register_handler(id, m);
                    v.handlers.push((id, m));
                }
            }
        }
        v
    }

    /// The size and z-level of `MapNoteTemplate`, read from one instance of the layout's
    /// template (destroyed again); the fallback's 30 × 30 at z −1 without a layout.
    fn read_note_shape(ui: &mut Ui, pv: &P5Vocab, layout: Option<Did>) -> NoteShape {
        let Some(l) = layout else {
            return NoteShape::default();
        };
        match ui.create_element(None, l, pv.ids.note_template) {
            Ok(k) => {
                let size = ui
                    .layout_box(k)
                    .map_or(super::mapmarks::NOTE_SIZE, |b| (b.w(), b.h()));
                let z = ui.z_level(k).unwrap_or(super::mapmarks::NOTE_Z);
                ui.destroy(k);
                NoteShape { size, z }
            }
            Err(_) => NoteShape::default(),
        }
    }

    /// The tree.
    #[must_use]
    pub fn tree(&self) -> Option<&MapTree> {
        self.panel.tree.as_ref()
    }

    /// The options of the shown node (defaults for a dungeon map or a non-node element).
    fn shown_options(&self) -> NodeOptions {
        match (self.shown, self.tree()) {
            (Some(Shown::Node(i)), Some(t)) => t.options(i),
            _ => NodeOptions::default(),
        }
    }

    /// Sets the icon table (`UI_MapNoteIcons_Map`, read by the client at start).
    pub fn set_icons(&mut self, icons: IconTable) {
        self.icons = icons;
    }

    /// Sets a filter option's stored value (`option` a property ID; the client's settings,
    /// initialised from `GameplayOptionDefaults`, §6.9). Takes effect at the next show.
    pub fn set_filter_value(&mut self, option: u32, value: u64) {
        self.filter_values.insert(option, value);
    }

    /// The stored value of a filter option (0 when never given).
    #[must_use]
    pub fn filter_value(&self, option: u32) -> u64 {
        self.filter_values.get(&option).copied().unwrap_or(0)
    }

    /// Sets the map menu's data (§4.5): `Region_Map_List` in stored order as (region,
    /// rendered `Region_Name`), and the rendered `UI_Map_MenuName` of each node by index
    /// (`None` for a node whose name is null: left out of the menu, §7.2 OPEN).
    pub fn set_menu(&mut self, regions: Vec<(u32, String)>, node_names: Vec<Option<String>>) {
        self.regions = regions;
        self.node_names = node_names;
    }

    /// The root that holds `MapField` now.
    #[must_use]
    pub fn root(&self) -> ElementKey {
        match self.presentation {
            Presentation::Windowed => self.windowed_root.unwrap_or(self.window),
            Presentation::FullScreen => self.window,
        }
    }

    /// Switches the presentation (§3.1): full screen shows `MapWindow` with `MapField` its
    /// own child; windowed shows `MapPanel` with `MapField` moved under `MapPanelPadding`.
    /// Without a windowed root (or its padding) the window stays full screen.
    pub fn set_presentation(&mut self, ui: &mut Ui, p: Presentation) {
        let (Some(field), Some(panel), Some(padding)) =
            (self.parts.field, self.windowed_root, self.parts.padding)
        else {
            self.presentation = Presentation::FullScreen;
            return;
        };
        let target = match p {
            Presentation::FullScreen => self.window,
            Presentation::Windowed => padding,
        };
        if ui.parent(field) != Some(target) {
            if let Err(e) = ui.attach_raw(field, target) {
                tracing::debug!("map window: MapField not moved: {e}");
                return;
            }
            ui.move_to(field, 0, 0);
        }
        self.presentation = p;
        ui.set_shown(self.window, p == Presentation::FullScreen);
        ui.set_shown(panel, p == Presentation::Windowed);
        self.panel.root = self.root();
    }

    /// The full-screen placement (§3.1 Tessera rule): `MapWindow` centred on the screen,
    /// scaled uniformly by the largest factor that fits (at most `ui_scale` × 2).
    pub fn fit_full_screen(&mut self, ui: &mut Ui, ui_scale: f32) {
        let (sw, sh) = ui.screen_size();
        #[allow(clippy::cast_precision_loss)]
        let s = (sw as f32 / WINDOW_SIZE.0 as f32)
            .min(sh as f32 / WINDOW_SIZE.1 as f32)
            .min(ui_scale.max(0.01) * 2.0);
        if s.is_finite() && s > 0.0 {
            ui.scale_panel(self.window, s, false);
        }
        let d = ui.displayed_box(self.window);
        let lb = ui.layout_box(self.window).unwrap_or_default();
        let x = sw.saturating_sub(d.w()) / 2;
        let y = sh.saturating_sub(d.h()) / 2;
        ui.move_to(
            self.window,
            lb.l.saturating_add(x.saturating_sub(d.l)),
            lb.t.saturating_add(y.saturating_sub(d.t)),
        );
    }

    /// Removes the shown map (its element, notes and marker).
    fn clear_shown(&mut self, ui: &mut Ui) {
        self.close_popup(ui);
        self.overlays.clear_map(ui);
        for (id, m) in self.node_handlers.drain(..) {
            ui.unregister_handler(id, m);
        }
        if let Some(m) = self.marker.take()
            && ui.exists(m)
        {
            ui.destroy(m);
        }
        if let Some(d) = self.dungeon.take()
            && ui.exists(d.root)
        {
            ui.destroy(d.root);
        }
        if let Some(k) = self.shown_element.take()
            && ui.exists(k)
        {
            ui.destroy(k);
        }
        self.shown = None;
        self.panel.shown = None;
        self.panel.dungeon = None;
        self.panel.marker = None;
        self.overlays.prune(ui);
    }

    /// Creates element `id` under `parent` from the layout, else `fallback`.
    fn create(
        &self,
        ui: &mut Ui,
        parent: ElementKey,
        id: u32,
        fallback: impl FnOnce(&mut Ui) -> Option<ElementKey>,
    ) -> Option<ElementKey> {
        if let Some(l) = self.layout {
            match ui.create_element(Some(parent), l, id) {
                Ok(k) => return Some(k),
                Err(e) => tracing::debug!("map window: element {id:#x} not created: {e}"),
            }
        }
        fallback(ui)
    }

    /// Shows node `i` (§4.2). Showing the node already shown does nothing (returns
    /// `Ok(None)`). Otherwise the previous map goes, the active element is created under
    /// `MapBackground` with the node's image (draw mode 2), its child-map elements get
    /// click handlers, the filter is read and the filter menu rebuilt, the controls follow
    /// §3.4, the player marker is created (§6.6) and the overlays placed (table notes
    /// arrive with [`MapWindowView::set_notes`] after [`MapAction::Shown`]).
    ///
    /// # Errors
    /// [`GameError::NoSuchNode`], [`GameError::MissingElement`] without `MapBackground`,
    /// or a creation error.
    pub fn show_node(
        &mut self,
        ui: &mut Ui,
        i: usize,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        let node = self
            .tree()
            .and_then(|t| t.nodes.get(i))
            .cloned()
            .ok_or(GameError::NoSuchNode(i))?;
        if self.shown == Some(Shown::Node(i)) && self.shown_element.is_some_and(|k| ui.exists(k)) {
            return Ok(None);
        }
        let bg = self
            .parts
            .background
            .ok_or(GameError::MissingElement(self.gv.ids.map_background))?;
        self.clear_shown(ui);
        let frame = self.tree().map_or(DEFAULT_FRAME, |t| node_frame(t, i));
        let k = self
            .create(ui, bg, node.active_element, |ui| {
                mapfallback::active_element(ui, bg, node.active_element, frame)
            })
            .ok_or(GameError::Ui(crate::UiError::TooManyElements))?;
        set_image(ui, k, node.map.image, NODE_IMAGE_MODE);
        self.shown = Some(Shown::Node(i));
        self.shown_element = Some(k);
        self.panel.shown = Some((i, k));
        self.register_child_maps(ui, k);
        self.after_show(ui, texts);
        Ok(Some(MapAction::Shown(Shown::Node(i))))
    }

    /// Shows a top-level element that is no node's active element (§4.2 step 3): as the
    /// layout draws it, with no note, marker, menu or filter update.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] without `MapBackground` or the element.
    pub fn show_element(&mut self, ui: &mut Ui, id: u32) -> Result<Option<MapAction>, GameError> {
        let bg = self
            .parts
            .background
            .ok_or(GameError::MissingElement(self.gv.ids.map_background))?;
        self.clear_shown(ui);
        let k = self
            .create(ui, bg, id, |_| None)
            .ok_or(GameError::MissingElement(id))?;
        tracing::debug!("map window: element {id:#x} is no node's active element");
        self.shown = Some(Shown::Element(id));
        self.shown_element = Some(k);
        self.register_child_maps(ui, k);
        Ok(Some(MapAction::Shown(Shown::Element(id))))
    }

    /// Shows a dungeon map (§4.7): `DungeonMap` under `MapBackground`; its image field
    /// gets object mode 3 at the image's size, the image with draw mode 2, then object
    /// mode 1 at its own 800 × 600 (so the image and everything on it is stretched,
    /// distortion included); the overlay frame is in front. While the image is loading the
    /// canvas keeps its size; [`MapWindowView::refresh`] finishes the step.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] without `MapBackground` or the image field, or a
    /// creation error.
    pub fn show_dungeon(
        &mut self,
        ui: &mut Ui,
        map: GameMap,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        let bg = self
            .parts
            .background
            .ok_or(GameError::MissingElement(self.gv.ids.map_background))?;
        self.clear_shown(ui);
        let (pv, art) = (self.pv.clone(), self.art.clone());
        let root = self
            .create(ui, bg, pv.ids.dungeon_map, |ui| {
                mapfallback::dungeon_map(ui, &pv, &art, bg)
            })
            .ok_or(GameError::MissingElement(pv.ids.dungeon_map))?;
        let Some(image) = find_below(ui, root, pv.ids.dungeon_image) else {
            ui.destroy(root);
            return Err(GameError::MissingElement(pv.ids.dungeon_image));
        };
        // Step 4, Tessera rule: the fog is hidden.
        show(ui, find_below(ui, root, pv.ids.dungeon_fog), false);
        set_image(ui, image, map.image, NODE_IMAGE_MODE);
        self.dungeon = Some(DungeonParts {
            root,
            image,
            map: map.clone(),
            size: None,
        });
        self.shown = Some(Shown::Dungeon);
        self.shown_element = Some(image);
        self.fit_dungeon(ui);
        self.panel.dungeon = Some(DungeonView {
            map,
            element: image,
            size: self
                .dungeon
                .as_ref()
                .and_then(|d| d.size)
                .map(|(w, h)| (w.unsigned_abs(), h.unsigned_abs())),
        });
        self.after_show(ui, texts);
        Ok(Some(MapAction::Shown(Shown::Dungeon)))
    }

    /// §4.7 step 2 once the image's size is known.
    fn fit_dungeon(&mut self, ui: &mut Ui) -> bool {
        let Some(d) = self.dungeon.as_mut() else {
            return false;
        };
        if d.size.is_some() || d.map.image == 0 {
            return false;
        }
        let ImageState::Ready(img) = ui.env.image(&ImageRef::Did(d.map.image)) else {
            return false;
        };
        let w = i32::try_from(img.width()).unwrap_or(i32::MAX).max(1);
        let h = i32::try_from(img.height()).unwrap_or(i32::MAX).max(1);
        let om = ui.vocab().pids.object_mode;
        let field = ui
            .layout_box(d.image)
            .map_or(DUNGEON_FIELD, |b| (b.w(), b.h()));
        let field = if field.0 > 0 && field.1 > 0 {
            field
        } else {
            DUNGEON_FIELD
        };
        ui.set_property(d.image, om, PropertyValue::Enum(object_mode::CANVAS_RESIZE));
        ui.resize(d.image, w, h);
        ui.set_property(d.image, om, PropertyValue::Enum(object_mode::STRETCH));
        ui.resize(d.image, field.0, field.1);
        d.size = Some((w, h));
        if let Some(pd) = self.panel.dungeon.as_mut() {
            pd.size = Some((w.unsigned_abs(), h.unsigned_abs()));
        }
        true
    }

    /// Images arrived (the client's image cache): a dungeon map whose size was unknown is
    /// fitted and the overlays and marker are placed again.
    pub fn refresh(&mut self, ui: &mut Ui, texts: &dyn MapTexts) {
        if self.fit_dungeon(ui) {
            self.place_overlays(ui, texts);
            self.place_marker(ui);
        }
    }

    /// Registers left, double-left and right clicks on the child-map elements of a shown
    /// element (§4.3) and on the element itself (§4.4).
    fn register_child_maps(&mut self, ui: &mut Ui, k: ElementKey) {
        let ms = ui.vocab().msgs;
        let pid = self.gv.pids.map_child_map;
        let mut ids: Vec<u32> = subtree(ui, k)
            .into_iter()
            .filter(|e| *e == k || ui.prop(*e, pid).is_set())
            .filter_map(|e| ui.element_id(e))
            .filter(|id| *id != 0)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            for m in [ms.button_click, ms.mouse_click, ms.mouse_dbl_click] {
                ui.register_handler(id, m);
                self.node_handlers.push((id, m));
            }
        }
    }

    /// §4.2 step 5 after a node or dungeon map is shown.
    fn after_show(&mut self, ui: &mut Ui, texts: &dyn MapTexts) {
        let dungeon = self.shown == Some(Shown::Dungeon);
        let opts = self.shown_options();
        self.filter_option = if opts.filter_property != 0 && !dungeon {
            opts.filter_property
        } else {
            self.pv.pids.gop_map_filter
        };
        self.filter = read_filter(self.filter_value(self.filter_option));
        let mask = self.display_mask();
        self.filter_menu = FilterMenuModel::build(mask, self.filter, texts, &self.icons);
        self.apply_controls(ui);
        self.create_marker(ui);
        self.place_overlays(ui, texts);
        self.place_marker(ui);
    }

    /// The display mask of the shown map (all bits on a dungeon map).
    #[must_use]
    pub fn display_mask(&self) -> u64 {
        match self.shown {
            Some(Shown::Dungeon) => u64::MAX,
            Some(Shown::Node(_)) => self.shown_options().display_mask(),
            _ => 0,
        }
    }

    /// Whether the shown map is the root (`WorldMap`).
    fn is_root(&self) -> bool {
        self.shown == Some(Shown::Node(0))
    }

    /// §3.4 steps 1–2 with the Tessera rules: `UI_Map_HideFilterButtons` hides
    /// `FilterMenu`, `MapMenu`, `StablemasterButton` and `CoordinateDisplay`; otherwise
    /// `FilterMenu` (not on `WorldMap`) and `MapMenu` show, `StablemasterButton` stays
    /// hidden and `CoordinateDisplay` follows `map_coordinates`.
    fn apply_controls(&mut self, ui: &mut Ui) {
        let hide = self.shown_options().hide_filter_buttons;
        let p = self.parts;
        show(ui, p.filter_menu, !hide && !self.is_root());
        show(ui, p.map_menu, !hide);
        show(ui, p.stablemaster, false);
        show(ui, p.coordinates, !hide && self.show_coordinates);
        show(ui, p.quest_guide, false);
        show(ui, p.indicators, false);
    }

    /// Sets Tessera's `map_coordinates` (§3.4 Tessera rule) and applies it.
    pub fn set_show_coordinates(&mut self, ui: &mut Ui, on: bool) {
        self.show_coordinates = on;
        if self.shown.is_some() {
            self.apply_controls(ui);
        }
    }

    /// The placement context of the shown map (`None` while no node or dungeon map is
    /// shown).
    #[must_use]
    pub fn context(&self) -> Option<MapContext> {
        let parent = self.shown_element?;
        match self.shown? {
            Shown::Node(i) => {
                let t = self.tree()?;
                let n = t.nodes.get(i)?;
                Some(MapContext {
                    parent,
                    georef: n.map.georef(),
                    frame: node_frame(t, i),
                    region: n.region.and_then(|r| u8::try_from(r).ok()),
                    dungeon: false,
                    node: Some(i),
                    display_mask: t.options(i).display_mask(),
                    filter: self.filter,
                })
            }
            Shown::Dungeon => {
                let d = self.dungeon.as_ref()?;
                Some(MapContext {
                    parent,
                    georef: d.map.georef(),
                    frame: d.size.unwrap_or(DUNGEON_FIELD),
                    region: None,
                    dungeon: true,
                    node: None,
                    display_mask: u64::MAX,
                    filter: self.filter,
                })
            }
            Shown::Element(_) => None,
        }
    }

    /// Places the overlays on the shown map again.
    fn place_overlays(&mut self, ui: &mut Ui, texts: &dyn MapTexts) {
        let Some(ctx) = self.context() else {
            self.overlays.clear_map(ui);
            return;
        };
        let tree = self.panel.tree.as_ref();
        self.overlays.place_on_map(
            ui,
            &self.pv,
            &ctx,
            tree,
            &self.icons,
            texts,
            self.note_shape,
        );
    }

    /// Gives the notes of the shown map (§6.3, §6.4; after [`MapAction::Shown`]) and places
    /// them.
    pub fn set_notes(&mut self, ui: &mut Ui, notes: Vec<MapNote>, texts: &dyn MapTexts) {
        self.overlays.set_notes(ui, notes);
        self.place_overlays(ui, texts);
    }

    /// A `MarkerAdded` (§6.7): kept across map changes and placed on the shown map.
    ///
    /// Only that marker's element is made (and the placed markers with a greater id again,
    /// for the id order); the notes stay.
    pub fn add_marker(&mut self, ui: &mut Ui, m: FeedMarker, texts: &dyn MapTexts) -> bool {
        let id = m.id;
        let ok = self.overlays.add_marker(ui, m);
        if ok && let Some(ctx) = self.context() {
            self.overlays.place_feed_marker(
                ui,
                &self.pv,
                &ctx,
                &self.icons,
                texts,
                self.note_shape,
                id,
            );
        }
        ok
    }

    /// A `MarkerRemoved` (§6.7 item 7): the marker's elements go; nothing else is placed
    /// again.
    pub fn remove_marker(&mut self, ui: &mut Ui, id: u32, _texts: &dyn MapTexts) -> bool {
        self.overlays.remove_marker(ui, id)
    }

    /// The centre of an overlay's element in map pixels of the shown map (the census's
    /// check, §5.4 step 4), when it is placed.
    #[must_use]
    pub fn overlay_pixel(&self, ui: &Ui, id: OverlayId) -> Option<(i32, i32)> {
        let k = self.overlays.map_element(id)?;
        let b = ui.layout_box(k)?;
        Some((b.l.saturating_add(b.w() / 2), b.t.saturating_add(b.h() / 2)))
    }

    /// Whether the shown map element draws an image (the census's first check).
    #[must_use]
    pub fn draws_image(&self, ui: &Ui) -> bool {
        self.shown_element
            .and_then(|k| ui.base_image(k))
            .is_some_and(|(img, _)| !matches!(img, ImageRef::Did(0)))
    }

    // ----- the player marker (§6.6) -----------------------------------------------------

    fn create_marker(&mut self, ui: &mut Ui) {
        let Some(parent) = self.shown_element else {
            return;
        };
        if ui.prop(parent, self.pv.pids.map_hide_player_note).bool() {
            return;
        }
        let (pv, art) = (self.pv.clone(), self.art.clone());
        let k = self.create(ui, parent, pv.ids.player_marker, |ui| {
            mapfallback::player_marker(ui, &pv, &art, parent)
        });
        if let Some(k) = k {
            let om = ui.vocab().pids.object_mode;
            ui.set_property(k, om, PropertyValue::Enum(object_mode::STRETCH));
            ui.set_shown(k, false);
            self.marker = Some(k);
            self.panel.marker = Some(k);
        }
    }

    /// The player moved or turned (§6.6 "Updates"): the marker is placed again.
    pub fn update_player(&mut self, ui: &mut Ui, player: &PlayerMap) {
        if player.valid {
            self.player = Some(player.clone());
        }
        self.place_marker(ui);
    }

    /// §6.6: centred on the player's pixel, hidden when its box would leave the frame,
    /// turned by the heading.
    fn place_marker(&mut self, ui: &mut Ui) {
        let Some(m) = self.marker else { return };
        let (Some(ctx), Some(p)) = (self.context(), self.player.as_ref()) else {
            ui.set_shown(m, false);
            return;
        };
        let Some((x, y)) = ctx.pixel(&p.point) else {
            ui.set_shown(m, false);
            return;
        };
        let (w, h) = ui.layout_box(m).map_or((0, 0), |b| (b.w(), b.h()));
        let (xl, yl) = (i64::from(x), i64::from(y));
        let (hw, hh) = (i64::from(w / 2), i64::from(h / 2));
        let inside = xl - hw >= 0
            && xl + hw <= i64::from(ctx.frame.0)
            && yl - hh >= 0
            && yl + hh <= i64::from(ctx.frame.1);
        if !inside {
            ui.set_shown(m, false);
            return;
        }
        ui.move_to(m, x.saturating_sub(w / 2), y.saturating_sub(h / 2));
        ui.set_shown(m, true);
        ui.window_set_rotation(m, [0.0, 0.0, Radar::marker_angle(p.heading)]);
    }

    // ----- navigation -------------------------------------------------------------------

    /// The player's map (§4.6, MW9): the player's dungeon is recorded when it changes; with
    /// a recorded dungeon whose map is not shown, its dungeon map; else, with a valid
    /// position, the node of the area (`WorldMap` for an area listed by no node).
    ///
    /// # Errors
    /// As [`MapWindowView::show_node`] and [`MapWindowView::show_dungeon`].
    pub fn show_player_map(
        &mut self,
        ui: &mut Ui,
        player: &PlayerMap,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        self.update_player(ui, player);
        match &player.dungeon {
            Some((id, map)) => {
                self.recorded_dungeon = Some(*id);
                let showing = self
                    .dungeon
                    .as_ref()
                    .is_some_and(|d| d.map.same_as(map) && ui.exists(d.root));
                if !showing {
                    return self.show_dungeon(ui, map.clone(), texts);
                }
                return Ok(None);
            }
            None => self.recorded_dungeon = None,
        }
        self.show_outdoor(ui, player, texts)
    }

    /// §4.6 step 3.
    fn show_outdoor(
        &mut self,
        ui: &mut Ui,
        player: &PlayerMap,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        if !player.valid {
            return Ok(None);
        }
        let Some(n) = self.tree().and_then(|t| t.player_node(player.area)) else {
            return Ok(None);
        };
        self.show_node(ui, n, texts)
    }

    /// Going up (§4.4 with its Tessera rules): on a node with `UI_Map_PreventZoomOut`
    /// nothing; else the parent node; at a node without parent, when a dungeon map is
    /// shown and the player is in a dungeon, the player's outdoor map; else nothing.
    ///
    /// # Errors
    /// As [`MapWindowView::show_node`].
    pub fn go_up(
        &mut self,
        ui: &mut Ui,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        match self.shown {
            Some(Shown::Node(i)) => {
                if self.shown_options().prevent_zoom_out {
                    return Ok(None);
                }
                match self
                    .tree()
                    .and_then(|t| t.nodes.get(i))
                    .and_then(|n| n.parent)
                {
                    Some(p) => self.show_node(ui, p, texts),
                    None => Ok(None),
                }
            }
            Some(Shown::Dungeon) => {
                let player = self
                    .player
                    .clone()
                    .filter(|_| self.recorded_dungeon.is_some());
                match player {
                    Some(p) => self.show_outdoor(ui, &p, texts),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// Place lookup (§7.3 "Result"): shows the target and sets the lookup marker (a plain
    /// feed marker with [`LOOKUP_MARKER_ID`], no icon) at `marker`, replacing an earlier
    /// one (one exists at a time).
    ///
    /// # Errors
    /// As [`MapWindowView::show_node`] and [`MapWindowView::show_dungeon`].
    pub fn show_lookup(
        &mut self,
        ui: &mut Ui,
        target: LookupTarget,
        marker: Option<(MapPoint, String)>,
        texts: &dyn MapTexts,
    ) -> Result<Option<MapAction>, GameError> {
        self.overlays.remove_marker(ui, LOOKUP_MARKER_ID);
        if let Some((point, label)) = marker {
            self.overlays.add_marker(
                ui,
                FeedMarker {
                    id: LOOKUP_MARKER_ID,
                    point,
                    label,
                    ..FeedMarker::default()
                },
            );
        }
        let action = match target {
            LookupTarget::Node(n) => self.show_node(ui, n, texts)?,
            LookupTarget::Dungeon(m) => self.show_dungeon(ui, m, texts)?,
            LookupTarget::Area(a) => {
                let n = self.tree().and_then(|t| t.player_node(a));
                match n {
                    Some(n) => self.show_node(ui, n, texts)?,
                    None => None,
                }
            }
        };
        if action.is_none() {
            self.place_overlays(ui, texts);
        }
        Ok(action)
    }

    /// `findplace` without text (§7.3): the lookup marker goes.
    pub fn clear_lookup(&mut self, ui: &mut Ui, texts: &dyn MapTexts) {
        self.remove_marker(ui, LOOKUP_MARKER_ID, texts);
    }

    // ----- pointer and events -------------------------------------------------------------

    /// The pointer moved to screen position (`x`, `y`) (§3.4 `CoordinateDisplay`): over the
    /// shown map, the pointed map pixel (× pixel frame ÷ on-screen size, truncated) is
    /// converted back to a world point and its coordinate text is shown; otherwise, or when
    /// the conversion fails, the text is left unchanged.
    pub fn pointer_moved(&mut self, ui: &mut Ui, x: i32, y: i32, texts: &dyn MapTexts) {
        let (Some(ctx), Some(cd)) = (self.context(), self.parts.coordinates) else {
            return;
        };
        let Some(g) = ctx.georef else { return };
        let d = ui.displayed_box(ctx.parent);
        if d.w() <= 0 || d.h() <= 0 || !d.contains(x, y) {
            return;
        }
        let px = i64::from(x - d.l) * i64::from(ctx.frame.0) / i64::from(d.w());
        let py = i64::from(y - d.t) * i64::from(ctx.frame.1) / i64::from(d.h());
        let (Ok(px), Ok(py)) = (i32::try_from(px), i32::try_from(py)) else {
            return;
        };
        let Some(global) = g.to_world(px, py) else {
            return;
        };
        let region = ctx
            .region
            .or_else(|| self.player.as_ref().map(|p| p.point.region))
            .unwrap_or(0);
        let point = MapPoint {
            global,
            region,
            instance: 0,
        };
        if let Some(t) = texts.coordinate_display(&point) {
            put_text(ui, Some(cd), &t);
        }
    }

    /// Handles one UI event: child-map clicks (§4.3), right clicks (§4.4), the player
    /// marker (§4.3), the filter and map menus (§4.5, §6.9), the presentation and close
    /// buttons (§3.1). Returns what the client does.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn MapTexts,
    ) -> Vec<MapAction> {
        let mut out = Vec::new();
        if let Some(i) = self.sub.as_mut().and_then(|(_, p)| p.handle_event(ui, ev)) {
            let target = self.menu_targets.get(i).copied();
            self.close_popup(ui);
            if let Some(n) = target {
                match self.show_node(ui, n, texts) {
                    Ok(Some(a)) => out.push(a),
                    Ok(None) => {}
                    Err(e) => tracing::debug!("map window: {e}"),
                }
            }
            return out;
        }
        if let Some(i) = self
            .popup
            .as_mut()
            .and_then(|(_, p)| p.handle_event(ui, ev))
        {
            self.popup_selected(ui, i, &mut out);
            return out;
        }
        let Some((src, kind)) = click(ui, ev) else {
            return out;
        };
        if !ui.exists(src) {
            return out;
        }
        let inside = ui.is_ancestor_or_self(self.window, src)
            || self
                .windowed_root
                .is_some_and(|r| ui.is_ancestor_or_self(r, src));
        if !inside {
            return out;
        }
        let ids = self.pv.ids;
        let id = ui.element_id(src).unwrap_or(0);
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let push = |r: Result<Option<MapAction>, GameError>, out: &mut Vec<MapAction>| match r {
            Ok(Some(a)) => out.push(a),
            Ok(None) => {}
            Err(e) => tracing::debug!("map window: {e}"),
        };
        if kind == ClickKind::Right {
            self.close_popup(ui);
            let r = self.go_up(ui, texts);
            push(r, &mut out);
            return out;
        }
        if matches!(kind, ClickKind::Left | ClickKind::Button) {
            if hit(self.parts.filter_menu)
                && self.parts.filter_menu.is_some_and(|k| ui.is_visible(k))
            {
                self.toggle_popup(ui, PopupKind::Filter, texts);
                return out;
            }
            if hit(self.parts.map_menu) && self.parts.map_menu.is_some_and(|k| ui.is_visible(k)) {
                self.toggle_popup(ui, PopupKind::Maps, texts);
                return out;
            }
            if id == ids.fullscreen_collapse || id == ids.fullscreen_expand {
                let p = if self.presentation == Presentation::FullScreen {
                    Presentation::Windowed
                } else {
                    Presentation::FullScreen
                };
                self.set_presentation(ui, p);
                out.push(MapAction::PresentationChanged {
                    full_screen: self.presentation == Presentation::FullScreen,
                });
                return out;
            }
            if id == ids.toggle_map_panel || id == ids.toggle_fow {
                self.close_popup(ui);
                out.push(MapAction::Close);
                return out;
            }
        }
        if !matches!(
            kind,
            ClickKind::Left | ClickKind::Button | ClickKind::Double
        ) {
            return out;
        }
        // §4.3: the player marker on an outdoor node with the player in a dungeon.
        if self.marker.is_some_and(|m| ui.is_ancestor_or_self(m, src))
            && matches!(self.shown, Some(Shown::Node(_)))
            && let Some((_, map)) = self.player.as_ref().and_then(|p| p.dungeon.clone())
        {
            let r = self.show_dungeon(ui, map, texts);
            push(r, &mut out);
            return out;
        }
        // §4.3: any element with `UI_Map_Child_Map` (the source or an ancestor below the
        // shown element).
        let pid = self.gv.pids.map_child_map;
        let mut cur = Some(src);
        let mut depth = 0;
        while let Some(c) = cur {
            if ui.prop(c, pid).is_set() {
                let target = ui.prop(c, pid).key();
                let node = self.tree().and_then(|t| t.node_by_element(target));
                let r = match node {
                    Some(n) => self.show_node(ui, n, texts),
                    None => self.show_element(ui, target),
                };
                push(r, &mut out);
                return out;
            }
            if Some(c) == self.parts.background || depth > 64 {
                break;
            }
            depth += 1;
            cur = ui.parent(c);
        }
        out
    }

    fn close_popup(&mut self, ui: &mut Ui) {
        self.close_sub(ui);
        if let Some((_, p)) = self.popup.take() {
            p.close(ui);
        }
    }

    fn close_sub(&mut self, ui: &mut Ui) {
        self.menu_targets.clear();
        if let Some((_, p)) = self.sub.take() {
            p.close(ui);
        }
    }

    fn toggle_popup(&mut self, ui: &mut Ui, kind: PopupKind, texts: &dyn MapTexts) {
        let open = self.popup.as_ref().map(|(k, _)| *k);
        self.close_popup(ui);
        if open == Some(kind) {
            return;
        }
        let (anchor, items) = match kind {
            PopupKind::Filter => (self.parts.filter_menu, self.filter_items(texts)),
            PopupKind::Maps => (self.parts.map_menu, self.map_items()),
        };
        let Some(anchor) = anchor else { return };
        let width = if kind == PopupKind::Maps { 260 } else { 220 };
        let check = self.art.check.clone();
        match Popup::open(
            ui,
            &self.nv,
            &self.art.base,
            check.as_ref(),
            anchor,
            width,
            items,
        ) {
            Ok(p) => self.popup = Some((kind, p)),
            Err(e) => tracing::debug!("map window: popup not opened: {e}"),
        }
    }

    fn filter_items(&self, texts: &dyn MapTexts) -> Vec<PopupItem> {
        self.filter_menu
            .items
            .iter()
            .map(|it| match it {
                FilterItem::All => {
                    PopupItem::command(&map_label(texts, super::mapmarks::MapLabel::FilterAll))
                }
                FilterItem::None => {
                    PopupItem::command(&map_label(texts, super::mapmarks::MapLabel::FilterNone))
                }
                FilterItem::Type {
                    label,
                    icon,
                    checked,
                    ..
                } => PopupItem {
                    text: label.clone(),
                    check: Some(*checked),
                    icon: icon.clone(),
                    ..PopupItem::default()
                },
            })
            .collect()
    }

    /// The map menu's items (§4.5): one item per entry of `Region_Map_List` in stored
    /// order; choosing one opens its submenu ([`Self::region_items`]).
    fn map_items(&self) -> Vec<PopupItem> {
        self.regions
            .iter()
            .map(|(_, name)| PopupItem::command(name))
            .collect()
    }

    /// The submenu of region entry `r` (§4.5 and its Tessera rule): the region's named
    /// nodes sorted by label (ties by tree order), with the nodes they show.
    fn region_items(&self, r: usize) -> (Vec<PopupItem>, Vec<usize>) {
        let (Some(t), Some((region, _))) = (self.panel.tree.as_ref(), self.regions.get(r)) else {
            return (Vec::new(), Vec::new());
        };
        let mut nodes: Vec<(Vec<char>, usize, String)> = t
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.region == Some(*region))
            .filter_map(|(i, _)| {
                let label = self.node_names.get(i).cloned().flatten()?;
                Some((collation_key(&label), i, label))
            })
            .collect();
        nodes.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        nodes.truncate(super::popup::MAX_ROWS);
        nodes
            .into_iter()
            .map(|(_, i, label)| (PopupItem::command(&label), i))
            .unzip()
    }

    /// Opens the submenu of region entry `r` beside its row of the map menu (a second
    /// choice of the same region closes it).
    fn open_sub(&mut self, ui: &mut Ui, r: usize) {
        let open = self.sub.as_ref().map(|(i, _)| *i);
        self.close_sub(ui);
        if open == Some(r) {
            return;
        }
        let Some(row) = self
            .popup
            .as_ref()
            .and_then(|(_, p)| p.rows.get(r).copied())
        else {
            return;
        };
        let (items, targets) = self.region_items(r);
        if items.is_empty() {
            return;
        }
        match Popup::open_at(
            ui,
            &self.nv,
            &self.art.base,
            None,
            row,
            Place::Beside,
            260,
            items,
        ) {
            Ok(p) => {
                self.sub = Some((r, p));
                self.menu_targets = targets;
            }
            Err(e) => tracing::debug!("map window: submenu not opened: {e}"),
        }
    }

    fn popup_selected(&mut self, ui: &mut Ui, i: usize, out: &mut Vec<MapAction>) {
        let Some(kind) = self.popup.as_ref().map(|(k, _)| *k) else {
            return;
        };
        match kind {
            PopupKind::Filter => {
                let new = self.filter_menu.select(i, self.filter);
                if new != self.filter {
                    self.filter = new;
                    let stored = self.filter_value(self.filter_option);
                    // Bits outside the read mask stay as stored.
                    let value = (stored & !super::mapmarks::FILTER_READ_MASK) | new;
                    self.filter_values.insert(self.filter_option, value);
                    self.overlays.apply_filter(ui, new);
                    out.push(MapAction::FilterChanged {
                        option: self.filter_option,
                        value,
                    });
                }
                if let Some((_, p)) = self.popup.as_mut() {
                    for (j, it) in self.filter_menu.items.iter().enumerate() {
                        if let FilterItem::Type { checked, .. } = it {
                            p.set_checked(ui, j, *checked);
                        }
                    }
                }
            }
            PopupKind::Maps => self.open_sub(ui, i),
        }
    }

    /// Whether a popup (filter or map menu) is open.
    #[must_use]
    pub fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    /// The open popup's root, when one is open.
    #[must_use]
    pub fn popup_root(&self) -> Option<ElementKey> {
        self.popup.as_ref().map(|(_, p)| p.root)
    }

    /// The open submenu's root (the map menu's region submenu), when one is open.
    #[must_use]
    pub fn submenu_root(&self) -> Option<ElementKey> {
        self.sub.as_ref().map(|(_, p)| p.root)
    }

    /// Removes everything the view created and its handlers (the roots stay; the client
    /// closes them).
    pub fn detach(&mut self, ui: &mut Ui) {
        self.clear_shown(ui);
        for (id, m) in self.handlers.drain(..) {
            ui.unregister_handler(id, m);
        }
    }
}
