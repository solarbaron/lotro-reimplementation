// SPDX-License-Identifier: GPL-3.0-or-later
//! Quest markers on the minimap and the world map (`quest-journal.md` §8.3 Tessera rule
//! (feed markers); `game-panels.md` §8.8 and §9.3 notes): a `MarkerAdded` with a quest is
//! drawn at its location by the §8.6.1 transform of the shown map, with its own icon or the
//! view's `Quest` note image; its label is the tooltip; mouse-over switches to the
//! highlight image; on the radar only inside `UI_Radar_RadarRadius` of the player, on the
//! world map only inside the shown map's frame.

use std::collections::BTreeMap;

use tessera_formats::property::PropertyValue;

use crate::env::{ImageRef, ImageState};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::npc::kit::{FNode, instantiate};
use super::super::radar::{Radar, RadarInput};
use super::super::vocab::GameVocab;
use super::super::worldmap::WorldMapPanel;
use super::super::{find_below, move_to_front};
use super::QuestKey;

/// Most quest markers kept (Tessera bound).
pub const MAX_MARKERS: usize = 1024;

/// Size of a marker whose image is not loaded yet (Tessera rule; the element is sized to
/// the image once it is).
pub const DEFAULT_SIZE: i32 = 16;

/// One feed quest marker (`MarkerAdded` with `quest` present).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestMarker {
    /// The feed's marker ID.
    pub id: u32,
    /// The quest.
    pub quest: QuestKey,
    /// Global coordinates (CO §10) of the location.
    pub global: (f32, f32),
    /// The marker's own icon (`icon`), when given.
    pub icon: Option<ImageRef>,
    /// The label (the tooltip).
    pub label: Option<String>,
}

/// The `Quest` note type's images of one view's icon table (`map-data.md` §4.8.5: normal
/// and highlight; the radar's and the map's tables give type `Quest` the same images).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteImages {
    /// The normal image.
    pub normal: Option<ImageRef>,
    /// The highlight image (mouse-over).
    pub highlight: Option<ImageRef>,
}

/// The radar's offset of a marker: its map pixel relative to the player's, from the
/// viewport centre *c* (`game-panels.md` §8.6.2: the map is moved so that the player's
/// pixel lies on *c*).
#[must_use]
pub fn radar_position(
    player_px: (i32, i32),
    marker_px: (i32, i32),
    centre: (i32, i32),
) -> (i32, i32) {
    (
        centre
            .0
            .saturating_add(marker_px.0.saturating_sub(player_px.0)),
        centre
            .1
            .saturating_add(marker_px.1.saturating_sub(player_px.1)),
    )
}

/// Whether a marker lies inside the radar radius: the distance between the player's and
/// the marker's map pixels at most `radius` (Tessera reading of `UI_Radar_RadarRadius`
/// as pixels of the 170-pixel viewport; `quest-journal.md` §8.3 OPEN on the unit).
#[must_use]
pub fn within_radius(player_px: (i32, i32), marker_px: (i32, i32), radius: i32) -> bool {
    let dx = i64::from(marker_px.0) - i64::from(player_px.0);
    let dy = i64::from(marker_px.1) - i64::from(player_px.1);
    let r = i64::from(radius.max(0));
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= r.saturating_mul(r)
}

/// Whether a pixel lies inside a map's frame (`0 ≤ x < w`, `0 ≤ y < h`).
#[must_use]
pub fn in_frame(px: (i32, i32), frame: (i32, i32)) -> bool {
    (0..frame.0).contains(&px.0) && (0..frame.1).contains(&px.1)
}

/// The quest markers and their elements on the radars and the world map.
#[derive(Debug, Clone, Default)]
pub struct QuestMarkers {
    markers: BTreeMap<u32, QuestMarker>,
    radar: BTreeMap<(ElementKey, u32), ElementKey>,
    map: BTreeMap<u32, ElementKey>,
    /// Elements of replaced markers, destroyed at the next update or removal.
    stale: Vec<ElementKey>,
}

impl QuestMarkers {
    /// A `MarkerAdded` with a quest: added, or replacing the marker with the same ID
    /// (`scenario-feed.md` §5.3). A replacement whose icon or label changed drops the
    /// marker's elements, so that the next update draws it with the new icon and tooltip.
    /// Past [`MAX_MARKERS`] a new marker is dropped (returns false).
    pub fn add(&mut self, m: QuestMarker) -> bool {
        match self.markers.get(&m.id) {
            Some(old) => {
                if old.icon != m.icon || old.label != m.label {
                    self.forget_elements(m.id);
                }
            }
            None if self.markers.len() >= MAX_MARKERS => {
                tracing::warn!("quest markers: more than {MAX_MARKERS}; marker dropped");
                return false;
            }
            None => {}
        }
        self.markers.insert(m.id, m);
        true
    }

    /// Moves the elements of marker `id` to the stale list.
    fn forget_elements(&mut self, id: u32) {
        let gone: Vec<(ElementKey, u32)> = self
            .radar
            .keys()
            .filter(|(_, i)| *i == id)
            .copied()
            .collect();
        for g in gone {
            if let Some(k) = self.radar.remove(&g) {
                self.stale.push(k);
            }
        }
        if let Some(k) = self.map.remove(&id) {
            self.stale.push(k);
        }
    }

    /// Destroys the elements of replaced and removed markers.
    fn destroy_stale(&mut self, ui: &mut Ui) {
        let stale = std::mem::take(&mut self.stale);
        let live: Vec<ElementKey> = stale.into_iter().filter(|k| ui.exists(*k)).collect();
        ui.destroy_many(&live);
    }

    /// A `MarkerRemoved`: the marker and its elements go.
    pub fn remove(&mut self, ui: &mut Ui, id: u32) {
        self.markers.remove(&id);
        self.forget_elements(id);
        self.destroy_stale(ui);
    }

    /// The markers, by ID.
    #[must_use]
    pub fn markers(&self) -> &BTreeMap<u32, QuestMarker> {
        &self.markers
    }

    /// The element of marker `id` on the radar `radar`, when shown.
    #[must_use]
    pub fn radar_element(&self, radar: ElementKey, id: u32) -> Option<ElementKey> {
        self.radar.get(&(radar, id)).copied()
    }

    /// The element of marker `id` on the world map, when shown.
    #[must_use]
    pub fn map_element(&self, id: u32) -> Option<ElementKey> {
        self.map.get(&id).copied()
    }

    /// Places the markers on a radar after its position update (`game-panels.md` §8.8
    /// note): each inside the radius is a child of `MinimapViewport`, centred on its
    /// pixel relative to the viewport centre, in front of the map and behind the player
    /// marker; the others are hidden. Without a georeference or a valid position every
    /// marker of this radar is hidden.
    pub fn update_radar(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        radar: &Radar,
        input: &RadarInput,
        images: &NoteImages,
    ) {
        self.destroy_stale(ui);
        let Some(viewport) = radar.viewport() else {
            return;
        };
        let radius = ui.prop(radar.key, gv.pids.radar_radius).int();
        let georef = radar.shown_georef().filter(|_| input.valid());
        let player = georef.map(|g| g.to_pixel(input.global.0, input.global.1));
        let ids: Vec<u32> = self.markers.keys().copied().collect();
        for id in ids {
            let m = self.markers[&id].clone();
            let place = match (georef, player) {
                (Some(g), Some(p)) => {
                    let px = g.to_pixel(m.global.0, m.global.1);
                    within_radius(p, px, radius).then(|| radar_position(p, px, radar.centre))
                }
                _ => None,
            };
            let key = (radar.key, id);
            let Some(pos) = place else {
                if let Some(k) = self.radar.get(&key) {
                    ui.set_shown(*k, false);
                }
                continue;
            };
            let el = match self.radar.get(&key).copied().filter(|k| ui.exists(*k)) {
                Some(k) => k,
                None => match make_marker(ui, viewport, &m, images) {
                    Some(k) => {
                        self.radar.insert(key, k);
                        k
                    }
                    None => continue,
                },
            };
            centre_at(ui, el, pos);
            ui.set_shown(el, true);
        }
        if let Some(pm) = radar.player_marker() {
            move_to_front(ui, pm);
        }
    }

    /// Places the markers on the world map after it showed a map or the player moved
    /// (`game-panels.md` §9.3 note): each whose location maps into the shown map's frame
    /// is a child of `MapBackground`, centred on its pixel of the shown map (offset by the
    /// map element's position), behind the player marker; the others are hidden.
    ///
    /// # Errors
    /// [`GameError::MissingElement`] when `MapBackground` is missing.
    pub fn update_world_map(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        map: &WorldMapPanel,
        images: &NoteImages,
    ) -> Result<(), GameError> {
        self.destroy_stale(ui);
        let bg = find_below(ui, map.root, gv.ids.map_background)
            .ok_or(GameError::MissingElement(gv.ids.map_background))?;
        let origin = map
            .shown_element()
            .and_then(|k| ui.layout_box(k))
            .map_or((0, 0), |b| (b.l, b.t));
        let frame = map.shown_frame();
        let ids: Vec<u32> = self.markers.keys().copied().collect();
        for id in ids {
            let m = self.markers[&id].clone();
            let px = map
                .player_pixel(m.global)
                .filter(|px| frame.is_some_and(|f| in_frame(*px, f)));
            let Some(px) = px else {
                if let Some(k) = self.map.get(&id) {
                    ui.set_shown(*k, false);
                }
                continue;
            };
            let el = match self.map.get(&id).copied().filter(|k| ui.exists(*k)) {
                Some(k) => k,
                None => match make_marker(ui, bg, &m, images) {
                    Some(k) => {
                        self.map.insert(id, k);
                        k
                    }
                    None => continue,
                },
            };
            centre_at(
                ui,
                el,
                (origin.0.saturating_add(px.0), origin.1.saturating_add(px.1)),
            );
            ui.set_shown(el, true);
        }
        map.raise_marker(ui);
        Ok(())
    }

    /// Forgets the elements of destroyed parents (closed panels).
    pub fn prune(&mut self, ui: &Ui) {
        self.radar.retain(|_, k| ui.exists(*k));
        self.map.retain(|_, k| ui.exists(*k));
    }
}

/// The size of an image when it is loaded.
fn image_size(ui: &mut Ui, img: &ImageRef) -> Option<(i32, i32)> {
    match ui.env.image(img) {
        ImageState::Ready(i) => Some((
            i32::try_from(i.width()).unwrap_or(i32::MAX),
            i32::try_from(i.height()).unwrap_or(i32::MAX),
        )),
        _ => None,
    }
}

/// A marker element: a code-made Button whose `normal` state shows the marker's icon or
/// the note image, and whose rollover states show the highlight image (the icon itself
/// when the marker names one; Tessera rule), with the label as its tooltip.
fn make_marker(
    ui: &mut Ui,
    parent: ElementKey,
    m: &QuestMarker,
    images: &NoteImages,
) -> Option<ElementKey> {
    let normal = m.icon.clone().or_else(|| images.normal.clone());
    let high = if m.icon.is_some() {
        m.icon.clone()
    } else {
        images.highlight.clone().or_else(|| images.normal.clone())
    };
    let (w, h) = normal
        .as_ref()
        .and_then(|i| image_size(ui, i))
        .unwrap_or((DEFAULT_SIZE, DEFAULT_SIZE));
    let st = ui.vocab().states;
    let node = FNode::new(
        WidgetClass::Button,
        0,
        0,
        0,
        w.clamp(1, 256),
        h.clamp(1, 256),
    )
    .image(&normal)
    .state(st.normal, normal.as_ref(), vec![])
    .state(st.normal_rollover, high.as_ref(), vec![])
    .state(st.highlight_rollover, high.as_ref(), vec![]);
    let k = instantiate(ui, parent, &node)?;
    if let Some(l) = &m.label {
        ui.set_tooltip_text(k, Some(super::super::npc::kit::cut(l)));
        let p = ui.vocab().pids.tooltips_on;
        ui.set_property(k, p, PropertyValue::Bool(true));
    }
    Some(k)
}

/// Moves `k` so that its centre lies on `pos` (parent coordinates).
fn centre_at(ui: &mut Ui, k: ElementKey, pos: (i32, i32)) {
    let (w, h) = ui.layout_box(k).map_or((0, 0), |b| (b.w(), b.h()));
    ui.move_to(k, pos.0.saturating_sub(w / 2), pos.1.saturating_sub(h / 2));
}
