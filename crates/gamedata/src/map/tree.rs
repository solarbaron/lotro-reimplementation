// SPDX-License-Identifier: GPL-3.0-or-later
//! The map tree of the world-map window (`game-panels.md` §9.1, `map-windows.md` §4.1), the
//! `UI_Map_GameMap` struct of dungeon maps (§4.7, `game-panels.md` §8.5.2) and the
//! georeference transform (`game-panels.md` §8.6.1, `map-windows.md` §3.4).

use std::collections::HashMap;

use tessera_formats::gameplay::common::{PropertyAccess, ValueExt};
use tessera_formats::property::{PropertyStruct, PropertyValue};
use tessera_formats::{Did, StringInfo};

use super::{MAX_AREAS, MAX_DEPTH, MAX_NODES, MIN_SCALE, MapError};

tessera_formats::property_ids! {
    /// Every property the map models read, resolved by dictionary label.
    pub struct MapPids {
        area_data = "UI_Map_AreaData",
        active_element = "UI_Map_ActiveElement",
        menu_name = "UI_Map_MenuName",
        image = "UI_Map_MapImage",
        block_x = "UI_Map_BlockOffsetX",
        block_y = "UI_Map_BlockOffsetY",
        pixel_x = "UI_Map_PixelOffsetX",
        pixel_y = "UI_Map_PixelOffsetY",
        scale = "UI_Map_Scale",
        pixel_width = "UI_Map_PixelWidth",
        pixel_height = "UI_Map_PixelHeight",
        region = "UI_Map_RegionID",
        area_dids = "UI_Map_AreaDIDs_Array",
        children = "UI_Map_AreaData_Array",
        filter_property = "UI_Map_MapNoteFilterProperty",
        include_common = "UI_Map_IncludeCommonMapNoteTypes",
        display_type = "UI_Map_MapNoteDisplayType",
        hide_filter_buttons = "UI_Map_HideFilterButtons",
        hide_player = "UI_Map_HidePlayerMapnote",
        prevent_zoom_out = "UI_Map_PreventZoomOut",
        quest_guide_disabled = "UI_Map_QuestGuideDisabled",
        fog = "UI_Map_FogOfWar",
        game_map = "UI_Map_GameMap",
        dungeon_map_data = "Dungeon_MapData",
        dungeon_minimap = "Dungeon_MiniMapData",
        area_map_data = "Area_MapData",
        dungeon_name = "Dungeon_Name",
        area_name = "Area_Name",
        note_type_array = "UI_Map_MapNoteTypeArray",
        note_type_value = "UI_Map_MapNoteType",
        onscreen = "UI_Map_MapNoteOnscreenImageStruct",
        offscreen = "UI_Map_MapNoteOffscreenImageStruct",
        note_image = "UI_Map_MapNoteImage",
        note_selected = "UI_Map_MapNoteSelectedImage",
        note_highlight = "UI_Map_MapNoteHighlightImage",
        note_menu_image = "UI_Map_MapNoteMenuImage",
        note_rel_z = "UI_Map_MapNoteRelativeZLevel",
        region_map_list = "Region_Map_List",
        region_id = "Region_ID",
        region_name = "Region_Name",
        map_note_type = "MapNote_Type",
        note_game_specific = "MapNote_GameSpecific",
        note_states = "WorldEvent_MapNoteStateArray",
        content_layers = "Entity_ContentLayers",
    }
}

/// A key-like value: an enum, bitfield, property ID, DID or non-negative integer.
pub(crate) fn key_of(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) | PropertyValue::PropertyId(k) => {
            Some(*k)
        }
        PropertyValue::DidRef(d) => Some(d.0),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        PropertyValue::Int64(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn int_of(v: &PropertyValue) -> Option<i32> {
    match v {
        PropertyValue::Int(i) => Some(*i),
        PropertyValue::Int64(i) => i32::try_from(*i).ok(),
        PropertyValue::Enum(k) => i32::try_from(*k).ok(),
        _ => None,
    }
}

fn field(
    s: &PropertyStruct,
    pid: tessera_formats::gameplay::common::Pid,
) -> Option<&PropertyValue> {
    s.get(pid.id()?)
}

/// A georeference (`game-panels.md` §8.6.1): block offsets, pixel offsets and scale
/// (pixels per world unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Georef {
    /// `UI_Map_BlockOffsetX`.
    pub block_x: i32,
    /// `UI_Map_BlockOffsetY`.
    pub block_y: i32,
    /// `UI_Map_PixelOffsetX`.
    pub pixel_x: i32,
    /// `UI_Map_PixelOffsetY`.
    pub pixel_y: i32,
    /// `UI_Map_Scale`.
    pub scale: f32,
}

impl Georef {
    /// World point → map pixel (`game-panels.md` §8.6.1): *dx* = *G*x − 160·*bx*,
    /// *dy* = *G*y − 160·*by* in 32-bit floats; *px* = *px*0 + trunc(*dx*·*s*),
    /// *py* = *py*0 − trunc(*dy*·*s*), truncation toward zero (non-finite or out-of-range
    /// products saturate; NaN gives 0).
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn to_pixel(&self, gx: f32, gy: f32) -> (i32, i32) {
        let dx = gx - 160.0 * self.block_x as f32;
        let dy = gy - 160.0 * self.block_y as f32;
        let tx = (dx * self.scale) as i32;
        let ty = (dy * self.scale) as i32;
        (
            self.pixel_x.saturating_add(tx),
            self.pixel_y.saturating_sub(ty),
        )
    }

    /// Map pixel → world point, the inverse of `map-windows.md` §3.4: *G*x = 160·*bx* +
    /// (*px* − *px*0) / *s*, *G*y = 160·*by* + (*py*0 − *py*) / *s*, in f64. `None` when the
    /// scale is 0 or not finite.
    #[must_use]
    pub fn to_world(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        let s = f64::from(self.scale);
        if !s.is_finite() || s == 0.0 {
            return None;
        }
        Some((
            160.0 * f64::from(self.block_x) + (px - f64::from(self.pixel_x)) / s,
            160.0 * f64::from(self.block_y) + (f64::from(self.pixel_y) - py) / s,
        ))
    }

    /// Whether the scale is usable: finite and its magnitude above 0.0002 (category C4
    /// of `map-windows.md` §5.3).
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.scale.is_finite() && self.scale.abs() > MIN_SCALE
    }
}

/// A map image with its georeference: a node's fields or a `UI_Map_GameMap` struct.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameMap {
    /// `UI_Map_MapImage` (`None` when absent or 0).
    pub image: Option<Did>,
    /// `UI_Map_BlockOffsetX`.
    pub block_x: Option<i32>,
    /// `UI_Map_BlockOffsetY`.
    pub block_y: Option<i32>,
    /// `UI_Map_PixelOffsetX`.
    pub pixel_x: Option<i32>,
    /// `UI_Map_PixelOffsetY`.
    pub pixel_y: Option<i32>,
    /// `UI_Map_Scale`.
    pub scale: Option<f32>,
    /// `UI_Map_FogOfWar` (absent = false).
    pub fog: bool,
}

impl GameMap {
    /// Reads the image and georeference fields of a struct (a node, or the
    /// `UI_Map_GameMap` struct of a map-data set). Missing or mistyped fields are absent.
    #[must_use]
    pub fn from_struct(s: &PropertyStruct, p: &MapPids) -> Self {
        let int = |pid| field(s, pid).and_then(int_of);
        Self {
            image: field(s, p.image)
                .and_then(key_of)
                .filter(|d| *d != 0)
                .map(Did),
            block_x: int(p.block_x),
            block_y: int(p.block_y),
            pixel_x: int(p.pixel_x),
            pixel_y: int(p.pixel_y),
            scale: field(s, p.scale).and_then(PropertyValue::as_f32),
            fog: field(s, p.fog).and_then(ValueExt::boolean).unwrap_or(false),
        }
    }

    /// The georeference: block offsets and a scale are required, a missing pixel offset
    /// is 0 (`game-panels.md` §9.1: present in every node).
    #[must_use]
    pub fn georef(&self) -> Option<Georef> {
        Some(Georef {
            block_x: self.block_x?,
            block_y: self.block_y?,
            pixel_x: self.pixel_x.unwrap_or(0),
            pixel_y: self.pixel_y.unwrap_or(0),
            scale: self.scale?,
        })
    }
}

/// The `UI_Map_GameMap` of a map-data set (`map-windows.md` §2: a stand-alone property set
/// named by `Dungeon_MapData`); `None` when the set has no such struct.
#[must_use]
pub fn game_map_of<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Option<GameMap> {
    set.structure(p.game_map)
        .map(|s| GameMap::from_struct(s, p))
}

/// One node of the map tree (`game-panels.md` §9.1, `map-windows.md` §4.1, MW3).
#[derive(Debug, Clone, PartialEq)]
pub struct MapNode {
    /// The parent node (`None` for the root).
    pub parent: Option<usize>,
    /// Depth (0 for the root).
    pub depth: usize,
    /// `UI_Map_ActiveElement` (0 when absent).
    pub active_element: u32,
    /// `UI_Map_MenuName`.
    pub menu_name: Option<StringInfo>,
    /// Image and georeference.
    pub map: GameMap,
    /// `UI_Map_PixelWidth` × `UI_Map_PixelHeight` (0 when absent: the image's size is used).
    pub pixel_size: (u32, u32),
    /// `UI_Map_RegionID` (absent on the root).
    pub region: Option<u32>,
    /// `UI_Map_AreaDIDs_Array`, stored order, zeros dropped.
    pub areas: Vec<Did>,
    /// Children in stored order.
    pub children: Vec<usize>,
    /// `UI_Map_MapNoteDisplayType`.
    pub display_type: Option<u64>,
    /// `UI_Map_IncludeCommonMapNoteTypes`.
    pub include_common: bool,
    /// `UI_Map_MapNoteFilterProperty` (an option PID).
    pub filter_property: Option<u32>,
    /// `UI_Map_HideFilterButtons`.
    pub hide_filter_buttons: bool,
    /// `UI_Map_HidePlayerMapnote`.
    pub hide_player_marker: bool,
    /// `UI_Map_PreventZoomOut`.
    pub prevent_zoom_out: bool,
}

impl MapNode {
    /// The node's display mask (`map-windows.md` §2, MW3): `UI_Map_MapNoteDisplayType`
    /// (0 when absent), ORed with [`super::COMMON_NOTE_MASK`] when
    /// `UI_Map_IncludeCommonMapNoteTypes` is true.
    #[must_use]
    pub fn display_mask(&self) -> u64 {
        let own = self.display_type.unwrap_or(0);
        if self.include_common {
            own | super::COMMON_NOTE_MASK
        } else {
            own
        }
    }
}

/// The world-map tree. Node 0 is the root; nodes are in depth-first pre-order with children
/// in stored order (the census order of `map-windows.md` §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapTree {
    /// Nodes in pre-order.
    pub nodes: Vec<MapNode>,
    by_area: HashMap<Did, usize>,
    by_element: HashMap<u32, usize>,
    /// Areas listed by two nodes: (area, node kept, node ignored) (`map-data.md` §4.8.4
    /// Tessera rule: the first in depth-first order keeps it).
    pub area_conflicts: Vec<(Did, usize, usize)>,
    /// Active elements of two nodes (MWI1 violations): (element, node kept for
    /// [`MapTree::node_by_element`], node ignored), with a warning.
    pub element_conflicts: Vec<(u32, usize, usize)>,
}

impl MapTree {
    /// Decodes the tree from the root node struct (`UI_Map_AreaData`). A node without an
    /// active element or a menu name is not added (with its subtree; `map-windows.md`
    /// §4.1). Bounded by [`MAX_NODES`], [`MAX_DEPTH`] and [`MAX_AREAS`].
    ///
    /// # Errors
    ///
    /// [`MapError::Malformed`] when the root is not a struct, [`MapError::TooLarge`] when a
    /// bound is passed.
    pub fn from_value(root: &PropertyValue, p: &MapPids) -> Result<Self, MapError> {
        if root.as_struct().is_none() {
            return Err(MapError::Malformed("UI_Map_AreaData is not a struct"));
        }
        let mut t = Self::default();
        let mut areas_total = 0usize;
        let mut stack: Vec<(&PropertyValue, Option<usize>, usize)> = vec![(root, None, 0)];
        while let Some((v, parent, depth)) = stack.pop() {
            if depth > MAX_DEPTH {
                return Err(MapError::TooLarge {
                    what: "map tree depth",
                    cap: MAX_DEPTH,
                });
            }
            let Some(s) = v.as_struct() else { continue };
            let active_element = field(s, p.active_element).and_then(key_of).unwrap_or(0);
            let menu_name = field(s, p.menu_name)
                .and_then(ValueExt::string_info)
                .cloned();
            if active_element == 0 || menu_name.is_none() {
                tracing::warn!(
                    depth,
                    "map node without active element or menu name skipped"
                );
                continue;
            }
            if t.nodes.len() >= MAX_NODES {
                return Err(MapError::TooLarge {
                    what: "map tree",
                    cap: MAX_NODES,
                });
            }
            let idx = t.nodes.len();
            let mut areas = Vec::new();
            if let Some(a) = field(s, p.area_dids).and_then(PropertyValue::as_array) {
                for e in &a.elements {
                    areas_total += 1;
                    if areas_total > MAX_AREAS {
                        return Err(MapError::TooLarge {
                            what: "map areas",
                            cap: MAX_AREAS,
                        });
                    }
                    if let Some(d) = key_of(&e.value).filter(|d| *d != 0) {
                        areas.push(Did(d));
                    }
                }
            }
            let size = |pid| {
                field(s, pid)
                    .and_then(int_of)
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or(0)
            };
            let flag = |pid| field(s, pid).and_then(ValueExt::boolean).unwrap_or(false);
            let node = MapNode {
                parent,
                depth,
                active_element,
                menu_name,
                map: GameMap::from_struct(s, p),
                pixel_size: (size(p.pixel_width), size(p.pixel_height)),
                region: field(s, p.region).and_then(key_of),
                areas,
                children: Vec::new(),
                display_type: field(s, p.display_type).and_then(ValueExt::bitfield),
                include_common: flag(p.include_common),
                filter_property: field(s, p.filter_property).and_then(key_of),
                hide_filter_buttons: flag(p.hide_filter_buttons),
                hide_player_marker: flag(p.hide_player),
                prevent_zoom_out: flag(p.prevent_zoom_out),
            };
            if let Some(par) = parent
                && let Some(pn) = t.nodes.get_mut(par)
            {
                pn.children.push(idx);
            }
            for a in &node.areas {
                match t.by_area.get(a) {
                    Some(&kept) => {
                        tracing::warn!(area = %a, kept, ignored = idx, "area listed by two map nodes");
                        t.area_conflicts.push((*a, kept, idx));
                    }
                    None => {
                        t.by_area.insert(*a, idx);
                    }
                }
            }
            match t.by_element.entry(node.active_element) {
                std::collections::hash_map::Entry::Occupied(kept) => {
                    // MWI1 (`map-windows.md` §9): readers report violations as warnings.
                    tracing::warn!(
                        element = node.active_element,
                        kept = *kept.get(),
                        ignored = idx,
                        "active element of two map nodes"
                    );
                    t.element_conflicts
                        .push((node.active_element, *kept.get(), idx));
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(idx);
                }
            }
            t.nodes.push(node);
            if let Some(c) = field(s, p.children).and_then(PropertyValue::as_array) {
                for e in c.elements.iter().rev() {
                    stack.push((&e.value, Some(idx), depth + 1));
                }
            }
        }
        Ok(t)
    }

    /// The node an area is registered to (`map-data.md` §4.8.4 step 3), if any.
    #[must_use]
    pub fn node_for_area(&self, area: Did) -> Option<usize> {
        self.by_area.get(&area).copied()
    }

    /// The node whose active element is `element` (`map-windows.md` §4.3).
    #[must_use]
    pub fn node_by_element(&self, element: u32) -> Option<usize> {
        self.by_element.get(&element).copied()
    }

    /// The player's map outdoors (`map-windows.md` §4.6 step 3, MW9): the node of the
    /// area, else the root. `None` only for an empty tree.
    #[must_use]
    pub fn node_for_position(&self, area: Option<Did>) -> Option<usize> {
        if self.nodes.is_empty() {
            return None;
        }
        Some(area.and_then(|a| self.node_for_area(a)).unwrap_or(0))
    }

    /// Whether `node` is `ancestor` or one of its descendants (parent links walked toward
    /// the root, at most [`MAX_DEPTH`] + 1 steps).
    #[must_use]
    pub fn is_within(&self, node: usize, ancestor: usize) -> bool {
        let mut at = Some(node);
        for _ in 0..=MAX_DEPTH {
            match at {
                Some(n) if n == ancestor => return true,
                Some(n) => at = self.nodes.get(n).and_then(|x| x.parent),
                None => return false,
            }
        }
        false
    }
}
