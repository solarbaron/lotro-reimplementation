// SPDX-License-Identifier: GPL-3.0-or-later
//! The minimap, the game type `Radar` (`game-panels.md` §8): a 3 × 3 grid of landblock
//! tiles outdoors or the dungeon map indoors, scrolled so that the player's map pixel lies
//! on the viewport centre, inside the round disc, with a turning marker, the rim in its
//! mood state, the time of day and the two labels.

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::env::{ImageRef, ImageState};
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

use super::data::{MapLookup, MapSource};
use super::map::{GameMap, Georef, georef_order, grid_blocks, tile_georef};
use super::simple::{mood_state, time_of_day_state};
use super::vocab::GameVocab;
use super::{find_below, set_image, switch_if_defined};

/// Draw mode of the tile and indoor-map images (§8.4.4 step 2, §8.5.2: Mix colour, keeps
/// the destination alpha).
pub const MAP_DRAW_MODE: u32 = 5;

/// Columns of the outdoor grid (§8.1: 3 rows × 3 columns).
pub const GRID_COLUMNS: usize = 3;

/// What the radar reads on each position update. The client fills it from the walker and
/// the sandbox (`game-panels.md` §8.9).
#[derive(Debug, Clone, PartialEq)]
pub struct RadarInput {
    /// Region (CO §7; 0 is invalid).
    pub region: u8,
    /// Landblock X (0xFF is invalid).
    pub x: u8,
    /// Landblock Y (0xFF is invalid).
    pub y: u8,
    /// Cell: 0 outdoors, 0xFFFF invalid, else an interior cell.
    pub cell: u16,
    /// Global coordinates (CO §10) of the player.
    pub global: (f32, f32),
    /// Compass heading in degrees, clockwise from north (CO §11.4).
    pub heading: f32,
    /// The player's `Volume_ContentLayer` (0 when absent).
    pub content_layer: u32,
    /// The player's `Mood_Level` (the sandbox: 0).
    pub mood: i32,
    /// `World_IsNight`, when present.
    pub is_night: Option<bool>,
    /// Whether a local player exists (§8.7.3).
    pub has_player: bool,
    /// `AreaText_Label` text (§8.7.4 Tessera rule: the area name of the player's block).
    pub area_name: Option<String>,
    /// `CoordinateText_Label` text; `None` hides the label (§8.7.4 Tessera rule: shown while
    /// Tessera's own setting, default on, is on).
    pub coordinates: Option<String>,
}

impl Default for RadarInput {
    fn default() -> Self {
        Self {
            region: 0,
            x: 0xFF,
            y: 0xFF,
            cell: 0xFFFF,
            global: (0.0, 0.0),
            heading: 0.0,
            content_layer: 0,
            mood: 0,
            is_night: None,
            has_player: true,
            area_name: None,
            coordinates: None,
        }
    }
}

impl RadarInput {
    /// Whether the position is valid (CO §7: region ≠ 0, X ≠ 0xFF, Y ≠ 0xFF, cell ≠ 0xFFFF).
    #[must_use]
    pub fn valid(&self) -> bool {
        self.region != 0 && self.x != 0xFF && self.y != 0xFF && self.cell != 0xFFFF
    }
}

/// The elements a radar uses (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarParts {
    /// `MinimapViewport`.
    pub viewport: ElementKey,
    /// `RadarMapImage` (outdoor tile list box).
    pub tiles: ElementKey,
    /// `MinimapMapImage` (indoor map).
    pub map_image: ElementKey,
    /// `MinimapPlayerMarker`.
    pub marker: ElementKey,
    /// `AreaText_Label`.
    pub area_label: ElementKey,
    /// `CoordinateText_Label`.
    pub coord_label: ElementKey,
    /// `MinimapFogOfWarDisplay`.
    pub fog_display: ElementKey,
    /// `MinimapFogOfWarParent`.
    pub fog_parent: ElementKey,
    /// `MinimapFogOfWarField`.
    pub fog_field: ElementKey,
}

/// One `Radar` (§8).
#[derive(Debug, Clone, PartialEq)]
pub struct Radar {
    /// The radar element.
    pub key: ElementKey,
    /// The parts; `None` when one is missing (§8.2 step 2: the minimap shows no map).
    pub parts: Option<RadarParts>,
    /// `Minimap_Frame`, the rim.
    pub rim: Option<ElementKey>,
    /// `TimeOfDay`.
    pub time_of_day: Option<ElementKey>,
    /// The viewport centre *c* (§8.2 step 1).
    pub centre: (i32, i32),
    /// The layout holding `RadarMapImage_Template` (the `radar` layout).
    pub tile_layout: Option<Did>,
    /// (region, X, Y, content layer) of the last completed grid build.
    pub last_build: Option<(u8, u8, u8, u32)>,
    /// The outdoor georeference of the last build.
    pub grid_georef: Option<Georef>,
    /// The indoor map shown, with the image size applied (`None` until known).
    pub indoor: Option<(GameMap, Option<(u32, u32)>)>,
    /// The (region, X, Y) of the position whose dungeon-map lookup last answered; `None`
    /// once the indoor map was cleared.
    pub indoor_block: Option<(u8, u8, u8)>,
    /// Whether `MinimapMapImage` has no image (cleared by the radar).
    pub map_image_blank: bool,
    /// Whether the last update placed the map outdoors.
    pub outdoors: Option<bool>,
}

impl Radar {
    /// Post-creation (§8.2): the viewport centre, the parts (stop if one is missing), the
    /// marker in object mode 1 centred on *c*, the fog display shown, the quest objective
    /// and the quest highlight hidden.
    pub fn post_create(
        ui: &mut Ui,
        gv: &GameVocab,
        key: ElementKey,
        tile_layout: Option<Did>,
    ) -> Self {
        let ids = gv.ids;
        let find = |id: u32| find_below(ui, key, id);
        let viewport = find(ids.minimap_viewport);
        let centre = viewport
            .and_then(|v| ui.layout_box(v))
            .map_or((0, 0), |b| (b.w() / 2, b.h() / 2));
        let parts = (|| {
            Some(RadarParts {
                viewport: viewport?,
                tiles: find(ids.radar_map_image)?,
                map_image: find(ids.minimap_map_image)?,
                marker: find(ids.player_marker)?,
                area_label: find(ids.area_label)?,
                coord_label: find(ids.coordinate_label)?,
                fog_display: find(ids.fog_display)?,
                fog_parent: find(ids.fog_parent)?,
                fog_field: find(ids.fog_field)?,
            })
        })();
        let rim = find(ids.minimap_frame);
        let time_of_day = find(ids.time_of_day);
        let quest = find(ids.quest_objective);
        let highlight = find(ids.area_quest_highlight);
        let r = Self {
            key,
            parts,
            rim,
            time_of_day,
            centre,
            tile_layout,
            last_build: None,
            grid_georef: None,
            indoor: None,
            indoor_block: None,
            map_image_blank: false,
            outdoors: None,
        };
        let Some(p) = r.parts else {
            tracing::debug!("radar: a part is missing; no map is shown");
            return r;
        };
        // Step 3.
        let mode = ui.vocab().pids.object_mode;
        ui.set_property(p.marker, mode, PropertyValue::Enum(object_mode::STRETCH));
        if let Some(b) = ui.layout_box(p.marker) {
            ui.move_to(p.marker, centre.0 - b.w() / 2, centre.1 - b.h() / 2);
        }
        // Step 4.
        ui.set_shown(p.fog_display, true);
        for k in [quest, highlight].into_iter().flatten() {
            ui.set_shown(k, false);
        }
        r
    }

    /// The rotation angle (degrees, Tessera's positive = counter-clockwise convention,
    /// `render.md` §6.6) that turns the east-pointing marker clockwise on screen by
    /// *h* − 90° (§8.6.3): 90 − *h*, reduced to [0, 360).
    #[must_use]
    pub fn marker_angle(heading: f32) -> f32 {
        let a = 90.0 - heading;
        if a.is_finite() {
            a.rem_euclid(360.0)
        } else {
            0.0
        }
    }

    /// A position update (§8.3): indoor or outdoor map, then placement, marker, rim, time
    /// of day and labels. With an invalid position nothing that depends on the position
    /// changes (map, placement, marker, labels); the rim and the time of day follow their
    /// own inputs (§8.7.2, §8.7.3).
    pub fn update(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) {
        self.update_rim_and_time(ui, gv, input);
        let Some(p) = self.parts else { return };
        if !input.valid() {
            return;
        }
        self.update_labels(ui, &p, input);
        let georef = if input.cell != 0 {
            ui.set_shown(p.map_image, true);
            ui.set_shown(p.fog_display, true);
            ui.set_shown(p.tiles, false);
            self.outdoors = Some(false);
            self.update_indoor(ui, &p, input, maps)
        } else {
            ui.set_shown(p.map_image, false);
            ui.set_shown(p.fog_display, false);
            ui.set_shown(p.tiles, true);
            self.outdoors = Some(true);
            // Leaving an interior forgets its map, so that a later interior whose lookup is
            // pending does not show it.
            self.clear_indoor(ui, &p);
            self.update_grid(ui, gv, &p, input, maps);
            self.grid_georef
        };
        // §8.6.2: scrolling (no georeference: position unchanged, §8.6.1 OPEN rule).
        if let Some(g) = georef {
            let (px, py) = g.to_pixel(input.global.0, input.global.1);
            let to = (
                self.centre.0.saturating_sub(px),
                self.centre.1.saturating_sub(py),
            );
            if input.cell != 0 {
                ui.move_to(p.map_image, to.0, to.1);
                ui.move_to(p.fog_parent, to.0, to.1);
            } else {
                ui.move_to(p.tiles, to.0, to.1);
            }
        }
        // §8.6.3.
        ui.window_set_rotation(p.marker, [0.0, 0.0, Self::marker_angle(input.heading)]);
    }

    /// The rim's mood state (§8.7.2) and the time of day (§8.7.3).
    pub fn update_rim_and_time(&self, ui: &mut Ui, gv: &GameVocab, input: &RadarInput) {
        if let Some(rim) = self.rim {
            switch_if_defined(ui, rim, mood_state(&gv.states, input.mood));
        }
        if let Some(t) = self.time_of_day {
            let s = time_of_day_state(&gv.states, input.has_player, input.is_night);
            switch_if_defined(ui, t, s);
        }
    }

    fn update_labels(&self, ui: &mut Ui, p: &RadarParts, input: &RadarInput) {
        ui.set_text(p.area_label, input.area_name.clone());
        match &input.coordinates {
            Some(t) => {
                ui.set_text(p.coord_label, Some(t.clone()));
                ui.set_shown(p.coord_label, true);
            }
            None => ui.set_shown(p.coord_label, false),
        }
    }

    fn update_indoor(
        &mut self,
        ui: &mut Ui,
        p: &RadarParts,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) -> Option<Georef> {
        let block = (input.region, input.x, input.y);
        match maps.dungeon_map(input.region, input.x, input.y, input.cell) {
            MapLookup::Pending => {
                // Tessera rule: while the lookup for another landblock is pending, the map
                // of the previous one is not shown (nor placed with its georeference).
                if self.indoor_block != Some(block) {
                    self.clear_indoor(ui, p);
                }
            }
            MapLookup::Ready(None) => {
                // §8.5.1 Tessera rule: no map.
                self.clear_indoor(ui, p);
                self.indoor_block = Some(block);
            }
            MapLookup::Ready(Some(m)) => {
                let same = self.indoor.as_ref().is_some_and(|(o, _)| o.same_as(&m));
                if !same {
                    set_image(ui, p.map_image, m.image, MAP_DRAW_MODE);
                    self.map_image_blank = m.image == 0;
                    self.indoor = Some((m, None));
                }
                self.indoor_block = Some(block);
            }
        }
        // §8.5.2 step 1: resize to the image once its size is known.
        if let Some((m, size @ None)) = self.indoor.as_mut()
            && m.image != 0
            && let ImageState::Ready(img) = ui.env.image(&ImageRef::Did(m.image))
        {
            let (w, h) = (img.width(), img.height());
            *size = Some((w, h));
            let wi = i32::try_from(w).unwrap_or(i32::MAX);
            let hi = i32::try_from(h).unwrap_or(i32::MAX);
            ui.resize(p.map_image, wi, hi);
        }
        self.indoor.as_ref().and_then(|(m, _)| m.georef())
    }

    /// Forgets the indoor map and clears `MinimapMapImage`'s image (once).
    fn clear_indoor(&mut self, ui: &mut Ui, p: &RadarParts) {
        self.indoor = None;
        self.indoor_block = None;
        if !self.map_image_blank {
            set_image(ui, p.map_image, 0, MAP_DRAW_MODE);
            self.map_image_blank = true;
        }
    }

    fn update_grid(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        p: &RadarParts,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) {
        let key = (input.region, input.x, input.y, input.content_layer);
        if self.last_build == Some(key) {
            return;
        }
        let blocks = grid_blocks(input.x, input.y);
        let mut records = Vec::with_capacity(blocks.len());
        let mut pending = false;
        for &(bx, by) in &blocks {
            match maps.landblock_map(input.region, bx, by) {
                MapLookup::Ready(m) => records.push(m),
                MapLookup::Pending => pending = true,
            }
        }
        if pending {
            return;
        }
        self.last_build = Some(key);
        // Georeference (§8.6.1): item 6, 4, then the others.
        self.grid_georef = georef_order(records.len()).into_iter().find_map(|i| {
            records[i]
                .as_ref()
                .and_then(|m| tile_georef(i, GRID_COLUMNS, blocks[i], m))
        });
        if self.grid_georef.is_none() {
            // §8.6.1 OPEN, Tessera rule: no tiles, position unchanged.
            for e in ui.list_entries(p.tiles) {
                ui.list_remove(p.tiles, e);
            }
            return;
        }
        let layout = self
            .tile_layout
            .or_else(|| ui.layout_of(self.key).map(|(d, _)| d));
        let mut items = ui.list_entries(p.tiles);
        for (i, rec) in records.iter().enumerate() {
            let item = match items.get(i) {
                Some(k) => *k,
                None => {
                    let Some(layout) = layout else { break };
                    match ui.create_element(None, layout, gv.ids.radar_tile_template) {
                        Ok(k) => {
                            if ui.list_add(p.tiles, k, None).is_err() {
                                ui.destroy(k);
                                break;
                            }
                            items.push(k);
                            k
                        }
                        Err(e) => {
                            tracing::debug!("radar: tile template not created: {e}");
                            break;
                        }
                    }
                }
            };
            let image = rec
                .as_ref()
                .map_or(0, |m| m.image_for_layer(input.content_layer));
            set_image(ui, item, image, MAP_DRAW_MODE);
        }
        for extra in items.iter().skip(records.len()) {
            ui.list_remove(p.tiles, *extra);
        }
    }
}
