// SPDX-License-Identifier: GPL-3.0-or-later
//! The game-widget dispatcher: finds the game-type instances of an opened panel, runs
//! their post-creation and routes the client's events to them (`game-panels.md` §3).

use tessera_formats::did::Did;

use crate::events::{Message, UiEvent};
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::data::{GameData, MapSource, Shortcuts};
use super::dragbox::HiddenDragBox;
use super::hidewhen::{HideWhenField, is_condition_property};
use super::quickslot::QuickslotBar;
use super::radar::{Radar, RadarInput};
use super::simple::{self, TutorialHighlighter};
use super::vocab::GameVocab;
use super::worldmap::{MapShown, WorldMapPanel};
use super::{GameError, subtree};

/// The layouts game code creates elements from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameLayouts {
    /// The `radar` layout (`RadarMapImage_Template`, §8.4.4).
    pub radar: Option<Did>,
    /// The `quickslots` layout (§10.1).
    pub quickslots: Option<Did>,
    /// The `map` layout (§9).
    pub map: Option<Did>,
}

/// Something the client does in answer to a game widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameAction {
    /// `MapZoom` was clicked (§8.7.5 Tessera rule: open P10).
    OpenWorldMap,
    /// The world map now shows this node (§9.2).
    MapNodeShown(usize),
}

/// The game widgets of every attached panel.
#[derive(Debug, Clone, Default)]
pub struct GamePanels {
    /// The labels in use.
    pub vocab: GameVocab,
    /// The layouts game code creates from.
    pub layouts: GameLayouts,
    /// Drag-box frames (§4).
    pub drag_boxes: Vec<HiddenDragBox>,
    /// Tutorial highlighters (§5).
    pub highlighters: Vec<TutorialHighlighter>,
    /// Conditional fields (§6).
    pub hide_fields: Vec<HideWhenField>,
    /// Minimaps (§8).
    pub radars: Vec<Radar>,
    /// Quickslot bars (§10).
    pub bars: Vec<QuickslotBar>,
    /// The world map (§9), once attached.
    pub world_map: Option<WorldMapPanel>,
    /// Tessera's current quickslot page (§7 Tessera rule: 1).
    pub page: u32,
    attached: std::collections::BTreeSet<ElementKey>,
    /// Game-code handler registrations made for a controller: (controller element,
    /// element ID, message). [`GamePanels::prune`] removes those of destroyed controllers.
    registrations: Vec<(ElementKey, u32, u32)>,
}

impl GamePanels {
    /// A dispatcher with these labels and layouts.
    #[must_use]
    pub fn new(vocab: GameVocab, layouts: GameLayouts) -> Self {
        Self {
            vocab,
            layouts,
            page: 1,
            ..Self::default()
        }
    }

    /// Finds the game-type instances in the subtree of `root` (pre-order) and runs each
    /// type's post-creation (§3): drag boxes reset, highlighters and pip displays hide,
    /// fields evaluate their hide test, the page changer shows, the radar finds its parts,
    /// quickslot bars build, and the types Tessera hides in the sandbox (§11) hide.
    /// Elements already attached are skipped.
    pub fn attach(
        &mut self,
        ui: &mut Ui,
        root: ElementKey,
        data: &dyn GameData,
        shortcuts: &dyn Shortcuts,
    ) {
        let gv = self.vocab.clone();
        let t = gv.types;
        let ms = ui.vocab().msgs;
        for k in subtree(ui, root) {
            if self.attached.contains(&k) {
                continue;
            }
            let Some(ty) = ui.element_type(k) else {
                continue;
            };
            let mut known = true;
            match ty {
                x if x == t.hidden_drag_box => {
                    let b = HiddenDragBox::post_create(ui, &gv, k);
                    if let Some(id) = b.bar.and_then(|bar| ui.element_id(bar)) {
                        self.register(ui, k, id, ms.mouse_over_top);
                    }
                    self.drag_boxes.push(b);
                }
                x if x == t.tutorial_highlighter => {
                    self.highlighters
                        .push(TutorialHighlighter::post_create(ui, &gv, k));
                }
                x if x == t.hide_when_field => {
                    let f = HideWhenField { key: k };
                    f.evaluate(ui, &gv, data);
                    self.hide_fields.push(f);
                }
                x if x == t.pip_display => simple::pip_display_post_create(ui, k),
                x if x == t.page_changer => simple::page_changer_post_create(ui, k, self.page),
                x if x == t.radar => {
                    let r = Radar::post_create(ui, &gv, k, self.layouts.radar);
                    if let Some(z) = super::find_below(ui, k, gv.ids.map_zoom)
                        && let Some(id) = ui.element_id(z)
                    {
                        self.register(ui, k, id, ms.button_click);
                    }
                    self.radars.push(r);
                }
                x if x == t.map_fog_of_war => simple::fog_post_create(ui, k),
                x if x == t.quickslot_bar => {
                    self.bars.push(QuickslotBar::post_create(
                        ui,
                        &gv,
                        k,
                        self.layouts.quickslots,
                        shortcuts,
                    ));
                }
                x if x == t.housing_panel_button
                    || x == t.housing_decoration_button
                    || x == t.medal_icon
                    || x == t.raid_lock
                    || x == t.chest_lock =>
                {
                    simple::hidden_type_post_create(ui, k);
                }
                _ => known = false,
            }
            if known {
                self.attached.insert(k);
            }
        }
    }

    /// Registers a game-code handler for a controller and remembers it.
    fn register(&mut self, ui: &mut Ui, owner: ElementKey, id: u32, msg: u32) {
        ui.register_handler(id, msg);
        self.registrations.push((owner, id, msg));
    }

    /// Removes one registration made by [`GamePanels::register`].
    fn unregister(&mut self, ui: &mut Ui, owner: ElementKey, id: u32, msg: u32) {
        if let Some(i) = self
            .registrations
            .iter()
            .position(|r| *r == (owner, id, msg))
        {
            self.registrations.remove(i);
            ui.unregister_handler(id, msg);
        }
    }

    /// Drops the controllers of destroyed elements, and removes the game-code handler
    /// registrations they made (so that the listener registry's counts return to zero when
    /// a panel closes). The client calls it after closing or destroying panels.
    pub fn prune(&mut self, ui: &mut Ui) {
        self.drag_boxes.retain(|b| ui.exists(b.key));
        self.highlighters.retain(|h| ui.exists(h.key));
        self.hide_fields.retain(|f| ui.exists(f.key));
        self.radars.retain(|r| ui.exists(r.key));
        self.bars.retain(|b| ui.exists(b.key));
        if let Some(w) = self.world_map.as_mut()
            && !ui.exists(w.root)
        {
            w.detach(ui);
            self.world_map = None;
        }
        self.attached.retain(|k| ui.exists(*k));
        let (gone, kept): (Vec<_>, Vec<_>) = self
            .registrations
            .drain(..)
            .partition(|(owner, _, _)| !ui.exists(*owner));
        self.registrations = kept;
        for (_, id, msg) in gone {
            ui.unregister_handler(id, msg);
        }
    }

    /// The input action `ToggleHiddenDragBoxes` (§4.3 step 2): every drag box toggles. While
    /// a box's frame is shown it has game-code handlers for its own mouse press and
    /// release (§4.4: only then can the pointer reach the frame).
    pub fn toggle_drag_boxes(&mut self, ui: &mut Ui) {
        let gv = self.vocab.clone();
        let ms = ui.vocab().msgs;
        let mut changes = Vec::new();
        for b in &mut self.drag_boxes {
            let was = b.shown;
            b.toggle(ui, &gv);
            if b.shown != was {
                changes.push((b.key, b.shown));
            }
        }
        for (key, shown) in changes {
            let Some(id) = ui.element_id(key) else {
                continue;
            };
            if shown {
                self.register(ui, key, id, ms.mouse_press);
                self.register(ui, key, id, ms.mouse_release);
            } else {
                self.unregister(ui, key, id, ms.mouse_press);
                self.unregister(ui, key, id, ms.mouse_release);
                ui.recompute_mouse_visible(key);
            }
        }
    }

    /// Re-evaluates every `HideWhenField` (§6.1: a player property changed).
    pub fn refresh_player(&mut self, ui: &mut Ui, data: &dyn GameData) {
        let gv = self.vocab.clone();
        for f in &self.hide_fields {
            f.evaluate(ui, &gv, data);
        }
    }

    /// A request (from code other than the hide test) to show or hide element `k` (§6.1):
    /// for a `HideWhenField` a show happens only while its hide test is false, and a hide
    /// always happens. Returns whether `k` is an attached `HideWhenField`; for any other
    /// element nothing is done and the caller applies the request itself. The client routes
    /// every show request for elements of attached panels through this.
    pub fn request_shown(
        &mut self,
        ui: &mut Ui,
        k: ElementKey,
        show: bool,
        data: &dyn GameData,
    ) -> bool {
        let gv = self.vocab.clone();
        let Some(f) = self.hide_fields.iter().find(|f| f.key == k) else {
            return false;
        };
        f.request_shown(ui, &gv, data, show);
        true
    }

    /// Property `pid` of element `k` changed (§6.1): a `HideWhenField` re-evaluates when
    /// `pid` is one of its condition properties (§6.2). Returns whether it re-evaluated.
    pub fn element_property_changed(
        &mut self,
        ui: &mut Ui,
        k: ElementKey,
        pid: u32,
        data: &dyn GameData,
    ) -> bool {
        let gv = self.vocab.clone();
        if !is_condition_property(&gv, pid) {
            return false;
        }
        let Some(f) = self.hide_fields.iter().find(|f| f.key == k) else {
            return false;
        };
        f.evaluate(ui, &gv, data);
        true
    }

    /// A tutorial-highlight event (§5 step 2).
    pub fn tutorial_event(&mut self, ui: &mut Ui, highlighted: Option<u32>) {
        for h in &mut self.highlighters {
            h.tutorial_event(ui, highlighted);
        }
    }

    /// A position update for every minimap (§8.3).
    pub fn update_radar(&mut self, ui: &mut Ui, input: &RadarInput, maps: &mut dyn MapSource) {
        let gv = self.vocab.clone();
        for r in &mut self.radars {
            r.update(ui, &gv, input, maps);
        }
    }

    /// Attaches the world-map panel P10 opened at `root` (§9.5) and shows the map of §9.4
    /// for the player's position: the dungeon map indoors, else the tree node for `area`
    /// (the `Area_DID` of the player's block) and the position's region. The player marker
    /// takes the image and size of the first minimap's `MinimapPlayerMarker` (none without
    /// an attached minimap).
    ///
    /// Calling it again for the same root (for example after `MapZoom` on an open panel)
    /// reuses the attached panel; for another root the old panel's elements and handlers
    /// are removed first.
    ///
    /// # Errors
    /// [`GameError::NoLayout`] without the `map` layout, or an error of
    /// [`WorldMapPanel::attach`] or [`WorldMapPanel::show_for_position`].
    pub fn attach_world_map(
        &mut self,
        ui: &mut Ui,
        root: ElementKey,
        area: Option<u32>,
        input: &RadarInput,
        maps: &mut dyn MapSource,
    ) -> Result<Option<MapShown>, GameError> {
        let gv = self.vocab.clone();
        let layout = self.layouts.map.ok_or(GameError::NoLayout("map"))?;
        let reuse = self.world_map.as_ref().is_some_and(|w| {
            w.root == root && w.layout == layout && w.map_window.is_some_and(|m| ui.exists(m))
        });
        if !reuse {
            if let Some(mut old) = self.world_map.take() {
                old.detach(ui);
            }
            let mut w = WorldMapPanel::attach(ui, &gv, root, layout)?;
            let marker = self
                .radars
                .iter()
                .filter_map(|r| r.parts)
                .find_map(|p| Some((ui.base_image(p.marker)?, ui.layout_box(p.marker)?)));
            if let Some(((image, mode), b)) = marker
                && let Err(e) = w.set_marker(ui, &gv, &image, mode, (b.w(), b.h()))
            {
                tracing::debug!("world map: no player marker: {e}");
            }
            self.world_map = Some(w);
        }
        let Some(w) = self.world_map.as_mut() else {
            return Ok(None);
        };
        w.show_for_position(ui, &gv, area, input, maps)
    }

    /// A position update for the attached world map (§9.3–§9.5): a pending indoor map is
    /// retried and the marker moves to the player's pixel.
    pub fn update_world_map(&mut self, ui: &mut Ui, input: &RadarInput, maps: &mut dyn MapSource) {
        let gv = self.vocab.clone();
        if let Some(w) = self.world_map.as_mut()
            && let Err(e) = w.update_position(ui, &gv, input, maps)
        {
            tracing::debug!("world map not updated: {e}");
        }
    }

    /// Routes one UI event (game-code handler messages, drag begin and end).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Vec<GameAction> {
        let mut out = Vec::new();
        match ev {
            UiEvent::Game { message } => self.game_message(ui, message, &mut out),
            UiEvent::DragBegan { .. } => {
                for b in &mut self.bars {
                    b.drag_begin(ui);
                }
            }
            UiEvent::DragEnded { .. } => {
                for b in &mut self.bars {
                    b.drag_end(ui);
                }
            }
            _ => {}
        }
        out
    }

    fn game_message(&mut self, ui: &mut Ui, m: &Message, out: &mut Vec<GameAction>) {
        let gv = self.vocab.clone();
        for b in &mut self.drag_boxes {
            b.message(ui, &gv, m);
        }
        let ms = ui.vocab().msgs;
        if m.msg != ms.button_click {
            return;
        }
        if m.source_id == gv.ids.map_zoom {
            out.push(GameAction::OpenWorldMap);
            return;
        }
        if let Some(w) = self.world_map.as_mut()
            && ui.prop(m.source, gv.pids.map_child_map).is_set()
            && let Some(n) = w.child_map_target(ui, &gv, m.source)
        {
            match w.show_node(ui, &gv, n) {
                Ok(()) => out.push(GameAction::MapNodeShown(n)),
                Err(e) => tracing::debug!("world map node {n} not shown: {e}"),
            }
        }
    }
}
