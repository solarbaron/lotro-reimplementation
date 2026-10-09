// SPDX-License-Identifier: GPL-3.0-or-later
//! The world map, panel P10 (`game-panels.md` §9): the map tree in `MapWindow`'s
//! `UI_Map_AreaData`, the choice of the node to show, its georeference, child-map buttons,
//! the dungeon map shown indoors (§9.4) and Tessera's composition of the panel with the
//! player marker (§9.5).

use std::collections::HashMap;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::string_info::StringInfo;

use crate::env::{ImageRef, ImageState};
use crate::props::{array_elements, as_key};
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

use super::data::{MapLookup, MapSource};
use super::map::{GameMap, Georef};
use super::radar::{Radar, RadarInput};
use super::vocab::{GamePids, GameVocab};
use super::{
    GameError, code_field, find_below, move_to_back, move_to_front, set_image, set_image_ref,
    subtree,
};

/// Most nodes of a map tree (Tessera cap; the data has 227).
pub const MAX_NODES: usize = 4096;

/// Deepest node of a map tree (Tessera cap; the data reaches depth 5).
pub const MAX_DEPTH: usize = 32;

/// Most area DIDs over the whole tree (Tessera cap; the data has 541).
pub const MAX_AREAS: usize = 1 << 16;

/// Draw mode used for a node's image when its active element has no Image item (Tessera
/// rule; draw mode 2, Normal).
pub const DEFAULT_IMAGE_MODE: u32 = 2;

/// One node of the map tree (§9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct MapNode {
    /// The parent node (`None` for the root).
    pub parent: Option<usize>,
    /// Depth (0 for the root).
    pub depth: usize,
    /// `UI_Map_ActiveElement`: the top-level element of the `map` layout that shows it.
    pub active_element: u32,
    /// `UI_Map_MenuName`.
    pub menu_name: Option<StringInfo>,
    /// Image and georeference (`UI_Map_MapImage`, offsets, scale).
    pub map: GameMap,
    /// `UI_Map_PixelWidth` × `UI_Map_PixelHeight`.
    pub pixel_size: (i32, i32),
    /// `UI_Map_RegionID` (absent on the root).
    pub region: Option<u32>,
    /// `UI_Map_AreaDIDs_Array`.
    pub areas: Vec<u32>,
    /// Child nodes, in stored order.
    pub children: Vec<usize>,
}

/// The world-map tree (§9.1). Node 0 is the root.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapTree {
    /// Nodes in pre-order.
    pub nodes: Vec<MapNode>,
    by_area: HashMap<u32, usize>,
    by_element: HashMap<u32, usize>,
}

impl MapTree {
    /// Decodes the tree from the root node struct (`UI_Map_AreaData`). Hostile input is
    /// bounded by [`MAX_NODES`], [`MAX_DEPTH`] and [`MAX_AREAS`].
    ///
    /// # Errors
    /// [`GameError::Malformed`] when the root is not a struct; [`GameError::TooLarge`] when
    /// a cap is exceeded.
    pub fn from_value(root: &PropertyValue, p: &GamePids) -> Result<Self, GameError> {
        if root.as_struct().is_none() {
            return Err(GameError::Malformed(p.map_area_data));
        }
        let mut t = Self::default();
        let mut areas_total = 0usize;
        // (value, parent, depth), processed depth-first so nodes are in pre-order.
        let mut stack: Vec<(&PropertyValue, Option<usize>, usize)> = vec![(root, None, 0)];
        while let Some((v, parent, depth)) = stack.pop() {
            if t.nodes.len() >= MAX_NODES {
                return Err(GameError::TooLarge {
                    what: "map tree",
                    cap: MAX_NODES,
                });
            }
            if depth > MAX_DEPTH {
                return Err(GameError::TooLarge {
                    what: "map tree depth",
                    cap: MAX_DEPTH,
                });
            }
            let Some(s) = v.as_struct() else { continue };
            let idx = t.nodes.len();
            let int = |pid: u32| match s.get(pid) {
                Some(PropertyValue::Int(i)) => Some(*i),
                _ => None,
            };
            let mut areas = Vec::new();
            if let Some(a) = s.get(p.map_area_dids) {
                for e in array_elements(a) {
                    areas_total += 1;
                    if areas_total > MAX_AREAS {
                        return Err(GameError::TooLarge {
                            what: "map areas",
                            cap: MAX_AREAS,
                        });
                    }
                    let d = as_key(&e.value);
                    if d != 0 {
                        areas.push(d);
                    }
                }
            }
            let menu_name = match s.get(p.map_menu_name) {
                Some(PropertyValue::StringInfo(si)) => Some((**si).clone()),
                _ => None,
            };
            let node = MapNode {
                parent,
                depth,
                active_element: s.get(p.map_active_element).map_or(0, as_key),
                menu_name,
                map: GameMap::from_value(v, p).unwrap_or_default(),
                pixel_size: (
                    int(p.map_pixel_width).unwrap_or(0),
                    int(p.map_pixel_height).unwrap_or(0),
                ),
                region: s.get(p.map_region).map(as_key),
                areas,
                children: Vec::new(),
            };
            if let Some(par) = parent
                && let Some(pn) = t.nodes.get_mut(par)
            {
                pn.children.push(idx);
            }
            for a in &node.areas {
                t.by_area.entry(*a).or_insert(idx);
            }
            if node.active_element != 0 {
                t.by_element.entry(node.active_element).or_insert(idx);
            }
            t.nodes.push(node);
            if let Some(c) = s.get(p.map_children) {
                for e in array_elements(c).iter().rev() {
                    stack.push((&e.value, Some(idx), depth + 1));
                }
            }
        }
        Ok(t)
    }

    /// The node that lists area `area` (§9.4; each area is in exactly one node, G27).
    #[must_use]
    pub fn node_for_area(&self, area: u32) -> Option<usize> {
        self.by_area.get(&area).copied()
    }

    /// The node whose active element is `element` (§9.2 child-map buttons).
    #[must_use]
    pub fn node_by_element(&self, element: u32) -> Option<usize> {
        self.by_element.get(&element).copied()
    }

    /// The node to show outdoors (§9.4 Tessera rule): the node listing the player's area;
    /// else the node with the player's region at the smallest depth (first in pre-order on
    /// a tie); else the root. `None` only for an empty tree.
    #[must_use]
    pub fn choose(&self, area: Option<u32>, region: Option<u32>) -> Option<usize> {
        if self.nodes.is_empty() {
            return None;
        }
        if let Some(n) = area.and_then(|a| self.node_for_area(a)) {
            return Some(n);
        }
        if let Some(r) = region
            && let Some((i, _)) = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.region == Some(r))
                .min_by_key(|(i, n)| (n.depth, *i))
        {
            return Some(i);
        }
        Some(0)
    }

    /// The georeference of node `i` (§9.3).
    #[must_use]
    pub fn georef(&self, i: usize) -> Option<Georef> {
        self.nodes.get(i)?.map.georef()
    }
}

/// What the world map shows (§9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapShown {
    /// A node of the map tree.
    Node(usize),
    /// The dungeon map of the player's interior (§8.5.1).
    Dungeon,
}

/// The dungeon map shown indoors (§9.4 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub struct DungeonView {
    /// The dungeon's `UI_Map_GameMap`.
    pub map: GameMap,
    /// The code-made element that draws it (first child of `MapBackground`).
    pub element: ElementKey,
    /// The image's pixel size once known (the element is resized to it).
    pub size: Option<(u32, u32)>,
}

/// The world-map panel P10 as Tessera composes it (§9.5 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldMapPanel {
    /// The panel root (`MapPanel`).
    pub root: ElementKey,
    /// The `map` layout.
    pub layout: Did,
    /// `MapWindow`, created as a child of `MapPanelPadding`.
    pub map_window: Option<ElementKey>,
    /// The tree read from `MapWindow`'s `UI_Map_AreaData`.
    pub tree: Option<MapTree>,
    /// The node shown and its active element.
    pub shown: Option<(usize, ElementKey)>,
    /// The dungeon map shown instead of a node (indoors).
    pub dungeon: Option<DungeonView>,
    /// The element IDs of the child-map buttons registered for the shown node.
    pub buttons: Vec<u32>,
    /// The player marker (§9.5), once its image is known.
    pub marker: Option<ElementKey>,
    /// Whether the last indoor lookup was pending: position updates retry it.
    pub indoor_pending: bool,
}

impl WorldMapPanel {
    /// Creates `MapWindow` under `MapPanelPadding` and reads the tree (§9.1, §9.5). On a
    /// tree error the window is destroyed again.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] when `MapPanelPadding` is missing, a creation error,
    /// or a tree error.
    pub fn attach(
        ui: &mut Ui,
        gv: &GameVocab,
        root: ElementKey,
        layout: Did,
    ) -> Result<Self, GameError> {
        let padding = find_below(ui, root, gv.ids.map_panel_padding)
            .ok_or(GameError::MissingElement(gv.ids.map_panel_padding))?;
        let window = ui.create_element(Some(padding), layout, gv.ids.map_window)?;
        let tree = match ui.property(window, gv.pids.map_area_data) {
            Some(v) => match MapTree::from_value(v, &gv.pids) {
                Ok(t) => Some(t),
                Err(e) => {
                    ui.destroy(window);
                    return Err(e);
                }
            },
            None => None,
        };
        Ok(Self {
            root,
            layout,
            map_window: Some(window),
            tree,
            shown: None,
            dungeon: None,
            buttons: Vec::new(),
            marker: None,
            indoor_pending: false,
        })
    }

    /// Removes everything the panel created: the shown node or dungeon map with its button
    /// handlers, the marker and `MapWindow`. Safe after the panel itself was destroyed.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.hide_node(ui);
        if let Some(m) = self.marker.take() {
            ui.destroy(m);
        }
        if let Some(w) = self.map_window.take() {
            ui.destroy(w);
        }
    }

    /// Shows node `i` (§9.2 and §9.5 Tessera rules): its active element becomes the
    /// **first** child of `MapBackground` (drawn behind the quest indicators and the
    /// coordinate display), drawn with the node's `UI_Map_MapImage`; the previous node's
    /// element (or dungeon map) is destroyed. The child-map buttons of the new element get
    /// game-code handlers for `Button_click`.
    ///
    /// # Errors
    /// [`GameError::NoSuchNode`] for an index outside the tree,
    /// [`GameError::MissingElement`] when `MapBackground` is missing, or a creation error.
    pub fn show_node(&mut self, ui: &mut Ui, gv: &GameVocab, i: usize) -> Result<(), GameError> {
        let node = self
            .tree
            .as_ref()
            .and_then(|t| t.nodes.get(i))
            .cloned()
            .ok_or(GameError::NoSuchNode(i))?;
        let bg = find_below(ui, self.root, gv.ids.map_background)
            .ok_or(GameError::MissingElement(gv.ids.map_background))?;
        self.hide_node(ui);
        let k = ui.create_element(Some(bg), self.layout, node.active_element)?;
        move_to_back(ui, k);
        let mode = ui.base_image(k).map_or(DEFAULT_IMAGE_MODE, |(_, m)| m);
        set_image(ui, k, node.map.image, mode);
        let click = ui.vocab().msgs.button_click;
        for b in subtree(ui, k) {
            if ui.prop(b, gv.pids.map_child_map).is_set()
                && let Some(id) = ui.element_id(b)
                && !self.buttons.contains(&id)
            {
                ui.register_handler(id, click);
                self.buttons.push(id);
            }
        }
        self.shown = Some((i, k));
        Ok(())
    }

    /// Shows a dungeon map (§9.4 Tessera rule, indoors): a code-made element, the first
    /// child of `MapBackground`, draws the map's `UI_Map_MapImage` with draw mode
    /// [`DEFAULT_IMAGE_MODE`] at (0, 0) and is resized to the image's pixel size once it is
    /// known; the map's georeference places the marker. The shown node is destroyed.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] when `MapBackground` is missing, or a creation error.
    pub fn show_dungeon(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        map: GameMap,
    ) -> Result<(), GameError> {
        if self
            .dungeon
            .as_ref()
            .is_some_and(|d| d.map.same_as(&map) && ui.exists(d.element))
        {
            return Ok(());
        }
        let bg = find_below(ui, self.root, gv.ids.map_background)
            .ok_or(GameError::MissingElement(gv.ids.map_background))?;
        self.hide_node(ui);
        let (w, h) = ui.layout_box(bg).map_or((0, 0), |b| (b.w(), b.h()));
        let k = code_field(ui, bg, w, h)?;
        move_to_back(ui, k);
        set_image(ui, k, map.image, DEFAULT_IMAGE_MODE);
        self.dungeon = Some(DungeonView {
            map,
            element: k,
            size: None,
        });
        self.fit_dungeon(ui);
        Ok(())
    }

    /// Resizes the dungeon element to its image once the image's size is known.
    fn fit_dungeon(&mut self, ui: &mut Ui) {
        let Some(d) = self.dungeon.as_mut() else {
            return;
        };
        if d.size.is_some() || d.map.image == 0 {
            return;
        }
        if let ImageState::Ready(img) = ui.env.image(&ImageRef::Did(d.map.image)) {
            let (w, h) = (img.width(), img.height());
            d.size = Some((w, h));
            let wi = i32::try_from(w).unwrap_or(i32::MAX);
            let hi = i32::try_from(h).unwrap_or(i32::MAX);
            ui.resize(d.element, wi, hi);
        }
    }

    /// Shows the map for the player's position (§9.4 Tessera rule): indoors (a valid
    /// position in an interior cell) the dungeon map that `maps` gives for it; outdoors, or
    /// indoors when the dungeon has no map or its lookup is still pending, the tree node of
    /// [`MapTree::choose`] for `area` and the position's region (a pending lookup is
    /// retried by [`WorldMapPanel::update_position`]). Then the marker is placed.
    ///
    /// # Errors
    /// As [`WorldMapPanel::show_node`] and [`WorldMapPanel::show_dungeon`].
    pub fn show_for_position(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        area: Option<u32>,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) -> Result<Option<MapShown>, GameError> {
        self.indoor_pending = false;
        if input.valid() && input.cell != 0 {
            match maps.dungeon_map(input.region, input.x, input.y, input.cell) {
                MapLookup::Ready(Some(m)) => {
                    self.show_dungeon(ui, gv, m)?;
                    self.place_marker(ui, input);
                    return Ok(Some(MapShown::Dungeon));
                }
                MapLookup::Ready(None) => {}
                MapLookup::Pending => self.indoor_pending = true,
            }
        }
        let region = (input.region != 0).then_some(u32::from(input.region));
        let node = self.tree.as_ref().and_then(|t| t.choose(area, region));
        if let Some(n) = node
            && self.shown.map(|(i, _)| i) != Some(n)
        {
            self.show_node(ui, gv, n)?;
        }
        self.place_marker(ui, input);
        Ok(node.map(MapShown::Node))
    }

    /// A position update (§9.3, §9.5): retries a pending indoor lookup (showing the dungeon
    /// map once it is ready), then moves the marker to the player's pixel. An invalid
    /// position changes nothing.
    ///
    /// # Errors
    /// As [`WorldMapPanel::show_dungeon`].
    pub fn update_position(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) -> Result<(), GameError> {
        if !input.valid() {
            return Ok(());
        }
        if self.indoor_pending {
            if input.cell == 0 {
                self.indoor_pending = false;
            } else {
                match maps.dungeon_map(input.region, input.x, input.y, input.cell) {
                    MapLookup::Pending => {}
                    MapLookup::Ready(None) => self.indoor_pending = false,
                    MapLookup::Ready(Some(m)) => {
                        self.indoor_pending = false;
                        self.show_dungeon(ui, gv, m)?;
                    }
                }
            }
        }
        self.fit_dungeon(ui);
        self.place_marker(ui, input);
        Ok(())
    }

    /// Creates the player marker (§9.5 Tessera rule) with `MinimapPlayerMarker`'s image and
    /// size: a code-made element in object mode 1 (so that it turns with the heading as the
    /// minimap's marker does, §8.6.3), the **last** child of `MapBackground` (drawn over the
    /// map and its overlays), hidden until it is placed. Replaces an earlier marker.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] when `MapBackground` is missing, or a creation error.
    pub fn set_marker(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        image: &ImageRef,
        draw_mode: u32,
        size: (i32, i32),
    ) -> Result<(), GameError> {
        let bg = find_below(ui, self.root, gv.ids.map_background)
            .ok_or(GameError::MissingElement(gv.ids.map_background))?;
        if let Some(old) = self.marker.take() {
            ui.destroy(old);
        }
        let k = code_field(ui, bg, size.0, size.1)?;
        move_to_front(ui, k);
        set_image_ref(ui, k, image, draw_mode);
        let mode = ui.vocab().pids.object_mode;
        ui.set_property(k, mode, PropertyValue::Enum(object_mode::STRETCH));
        ui.set_shown(k, false);
        self.marker = Some(k);
        Ok(())
    }

    /// Places the marker centred on the player's pixel of the shown map, turned by the
    /// heading (§8.6.3); hidden when the shown map has no georeference.
    fn place_marker(&mut self, ui: &mut Ui, input: &RadarInput) {
        let Some(m) = self.marker else { return };
        if !input.valid() {
            return;
        }
        let shown_el = self
            .dungeon
            .as_ref()
            .map(|d| d.element)
            .or(self.shown.map(|(_, k)| k));
        let Some((px, py)) = self.player_pixel(input.global) else {
            ui.set_shown(m, false);
            return;
        };
        let origin = shown_el
            .and_then(|k| ui.layout_box(k))
            .map_or((0, 0), |b| (b.l, b.t));
        let (w, h) = ui.layout_box(m).map_or((0, 0), |b| (b.w(), b.h()));
        ui.move_to(
            m,
            origin.0.saturating_add(px).saturating_sub(w / 2),
            origin.1.saturating_add(py).saturating_sub(h / 2),
        );
        ui.set_shown(m, true);
        ui.window_set_rotation(m, [0.0, 0.0, Radar::marker_angle(input.heading)]);
    }

    /// Removes the shown node's element (or the dungeon map) and the node's button
    /// handlers.
    pub fn hide_node(&mut self, ui: &mut Ui) {
        let click = ui.vocab().msgs.button_click;
        for id in self.buttons.drain(..) {
            ui.unregister_handler(id, click);
        }
        if let Some((_, k)) = self.shown.take() {
            ui.destroy(k);
        }
        if let Some(d) = self.dungeon.take() {
            ui.destroy(d.element);
        }
    }

    /// A click on a child-map button (§9.2 OPEN, Tessera rule): the node whose active
    /// element the button names, or `None` when it names no node.
    #[must_use]
    pub fn child_map_target(&self, ui: &Ui, gv: &GameVocab, button: ElementKey) -> Option<usize> {
        let target = ui.prop(button, gv.pids.map_child_map).key();
        self.tree.as_ref()?.node_by_element(target)
    }

    /// The player's pixel on the shown map (§9.3): the dungeon map's georeference indoors,
    /// else the shown node's.
    #[must_use]
    pub fn player_pixel(&self, global: (f32, f32)) -> Option<(i32, i32)> {
        let g = match &self.dungeon {
            Some(d) => d.map.georef()?,
            None => {
                let (i, _) = self.shown?;
                self.tree.as_ref()?.georef(i)?
            }
        };
        Some(g.to_pixel(global.0, global.1))
    }
}
