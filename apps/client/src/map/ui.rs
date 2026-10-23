// SPDX-License-Identifier: GPL-3.0-or-later
//! The map window and the minimap's feed markers on the game UI (`map-windows.md` §3–§7):
//! `tessera-ui`'s [`MapWindowView`] opened from the install's `map` layout (or Tessera's
//! fallback window), fed with the install's facts ([`MapFacts`]) and the session state.
//!
//! - **Opening** ([`MapUi::open`], [`MapUi::toggle`]): the icon table, the filter values
//!   and switches (Tessera's settings, initialised from `GameplayOptionDefaults`, §6.9,
//!   §8), the map menu's regions and node names (§4.5), the presentation (§3.1) and, the
//!   first time in a session, the player's map (§4.6 Tessera rules). The player's map is
//!   chosen again whenever the player's dungeon changes (§4.6 "When"; the dungeon comes
//!   from the interior cell's `Dungeon_DID`, revision 1, [`super::dungeon`]).
//! - **Notes** (§6.3): on every shown map, the notes `tessera-gamedata` selects for it
//!   (`map-data.md` §4.8: queries, discovery, content layer, creation by the display mask
//!   and the area's node), with the filter applied by the view.
//! - **Feed markers** (§6.7): every `MarkerAdded` of the state is kept on the window and
//!   placed whenever a map is shown; a later `MarkerAdded` of the same id replaces it;
//!   `MarkerRemoved` takes it away. On the minimaps the plain markers are drawn by
//!   [`MapUi::update_radars`] (inside the radius only, §6.8, replaced the same way); quest
//!   markers there are the quest journal's (P3, `quest-journal.md` §8.3). Both syncs run
//!   only when the state's marker revision changed, convert only the markers that changed,
//!   and resolve a record-name label once per record ([`MAX_NAME_CACHE`]).
//! - **Facts arriving late** ([`MapUi::facts_arrived`]): the install's facts load off the
//!   frame; a window opened before them is configured again (icons, menu, filter values,
//!   switches) and the notes of the shown map given, and the settings take the install's
//!   defaults unless the user already changed them.
//! - **Events**: child maps, going up, the map and filter menus, the presentation and
//!   close buttons (the view's [`MapAction`]s); a filter change is kept in the settings.
//! - **Place lookup** ([`MapUi::lookup`], §7.3): shows the first result with the lookup
//!   marker.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_gamedata::map::{NoteSettings, ShownMap, select_notes};
use tessera_net::domain::Text;
use tessera_ui::game::map::{GameMap, MapPoint};
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapmarks::{
    FeedMarker as UiMarker, FilterMenuModel, IconTable, MapNote, MapOverlays, MapTexts,
    RadarContext, marker_tooltip, read_filter,
};
use tessera_ui::game::mapwindow::{
    DEFAULT_FRAME, LookupTarget, MapAction, MapWindowView, PlayerMap, Presentation, Shown,
};
use tessera_ui::game::radar::{Radar, RadarInput};
use tessera_ui::game::worldmap::MapTree;
use tessera_ui::{ElementKey, ImageRef, Ui, UiEvent};

use super::data::{MapFacts, MapSettings};
use super::lookup::{LookupOutcome, PlaceTarget, find_place};
use crate::travel::refs::P5Refs;
use crate::travel::state::{FeedMarker, P5State};

/// Most distinct failure reasons kept (Tessera bound).
pub const MAX_FAILURES: usize = 64;

/// Most record-name labels kept resolved (Tessera bound; beyond it a name is resolved
/// each time its marker changes).
pub const MAX_NAME_CACHE: usize = 4096;

/// What the client keeps from the map window's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapChange {
    /// A filter option's value changed (Tessera's settings, §6.9).
    Filter {
        /// The option's property ID.
        option: u32,
        /// The value.
        value: u64,
    },
    /// The presentation switched (`map_fullscreen`, §3.1).
    Presentation {
        /// Full screen.
        full_screen: bool,
    },
    /// The window was closed by its close control.
    Closed,
}

/// The label of a feed marker: a literal as written, a record's display name from the
/// install (`None` without one: empty).
fn label_of(t: &Text, facts: Option<&MapFacts>) -> String {
    match t {
        Text::Literal(s) => s.clone(),
        Text::Name(d) => facts
            .and_then(|f| f.character.store().display_name(Did(d.0), None))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// A feed marker as `tessera-ui`'s (`map-windows.md` §6.7).
#[must_use]
pub fn ui_marker(id: u32, m: &FeedMarker, facts: Option<&MapFacts>) -> UiMarker {
    with_label(id, m, label_of(&m.label, facts))
}

fn with_label(id: u32, m: &FeedMarker, label: String) -> UiMarker {
    #[allow(clippy::cast_possible_truncation)]
    UiMarker {
        id,
        point: MapPoint {
            global: (m.location.x as f32, m.location.y as f32),
            region: u8::try_from(m.location.region.0).unwrap_or(0),
            instance: 0,
        },
        label,
        icon: m.icon.map(|d| ImageRef::Did(d.0)),
        quest: m.quest.is_some(),
        note_type: m.note_type,
    }
}

/// [`ui_marker`] with record-name labels resolved once per record (`names`, at most
/// [`MAX_NAME_CACHE`]; only with the install's facts).
fn cached_marker(
    names: &mut HashMap<u32, String>,
    id: u32,
    m: &FeedMarker,
    facts: Option<&MapFacts>,
) -> UiMarker {
    let label = match (&m.label, facts) {
        (Text::Name(d), Some(_)) => match names.get(&d.0) {
            Some(l) => l.clone(),
            None => {
                let l = label_of(&m.label, facts);
                if names.len() < MAX_NAME_CACHE {
                    names.insert(d.0, l.clone());
                }
                l
            }
        },
        (t, _) => label_of(t, facts),
    };
    with_label(id, m, label)
}

/// The player's state for the map window from the walker's minimap input (`map-windows.md`
/// §4.6, §6.6): position, heading, the area of the player's block, and the player's
/// dungeon `dungeon` (the client half's sources of §4.6 revision 1, MW34: the interior
/// cell's `Dungeon_DID`, [`super::dungeon::PlayerDungeon`]) with its map data's map
/// ([`player_dungeon_map`]).
#[must_use]
pub fn player_map(input: &RadarInput, facts: Option<&MapFacts>, dungeon: Option<Did>) -> PlayerMap {
    let valid = input.has_player && input.region != 0 && input.x != 0xFF && input.y != 0xFF;
    PlayerMap {
        point: MapPoint {
            global: input.global,
            region: input.region,
            instance: 0,
        },
        heading: input.heading,
        valid,
        area: facts.and_then(|f| f.area_at(input.region, input.x, input.y)),
        dungeon: dungeon.and_then(|d| facts.and_then(|f| player_dungeon_map(f, d))),
    }
}

/// The player's dungeon `dungeon` (a `Dungeon_DID`) with the map of its `Dungeon_MapData`
/// (§4.6 Tessera rules, MW34; the map as §4.7 draws it). `None` when the dungeon is not
/// among the install's dungeon records, has no map data, or its map data names no map
/// region: the player's map is then the outdoor one (§4.6 step 3, Tessera rule).
#[must_use]
pub fn player_dungeon_map(facts: &MapFacts, dungeon: Did) -> Option<(u32, GameMap)> {
    let regions = facts.regions.as_ref()?;
    let d = regions.dungeons.iter().find(|d| d.did == dungeon)?;
    let map = super::lookup::dungeon_game_map(facts, d.map_data?)?;
    Some((dungeon.0, map))
}

/// The map window and the minimap's markers (module docs).
pub struct MapUi {
    refs: Arc<P5Refs>,
    art: P5Art,
    view: Option<MapWindowView>,
    settings: MapSettings,
    /// The user changed a filter or the presentation (the install's defaults arriving
    /// later do not replace them).
    user_settings: bool,
    /// The user's saved values (`[Map]` of Tessera's settings file), laid over the
    /// install's defaults; changes made in the window are recorded here.
    prefs: tessera_settings::MapPrefs,
    /// `prefs` changed since [`MapUi::take_changed_prefs`] last took them.
    prefs_changed: bool,
    /// The feed markers on the window (id → the state's marker as last given).
    on_map: BTreeMap<u32, FeedMarker>,
    /// The state's marker revision `on_map` follows.
    map_rev: Option<u64>,
    /// The minimaps' overlays (plain feed markers).
    radar: MapOverlays,
    /// The plain feed markers on the minimaps (id → the state's marker as last given).
    on_radar: BTreeMap<u32, FeedMarker>,
    /// The state's marker revision `on_radar` follows.
    radar_rev: Option<u64>,
    /// Resolved record-name labels (record DID → name).
    names: HashMap<u32, String>,
    /// The dungeon whose map is shown (the zone of its note query).
    dungeon_zone: Option<Did>,
    /// The player's dungeon as last recorded this session (§4.6 "When"), followed every
    /// frame whether or not a window exists; a window opened without the player's map
    /// starts from it.
    player_dungeon: Option<u32>,
    /// The tree of Tessera's fallback window (empty unless set, [`MapUi::set_fallback_tree`]).
    fallback_tree: MapTree,
    /// A dungeon map was shown before its image size was known: its notes are given once
    /// the size is known ([`MapUi::refresh`]).
    dungeon_notes_pending: bool,
    /// The content layer of the note selection (`map-windows.md` §8: the walker's first
    /// `--content-layers` value).
    layer: u32,
    failures: Vec<String>,
}

impl std::fmt::Debug for MapUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapUi")
            .field("open", &self.view.is_some())
            .field("markers", &self.on_map.len())
            .finish_non_exhaustive()
    }
}

impl MapUi {
    /// No window yet; `settings` are the filter values and switches in use.
    #[must_use]
    pub fn new(refs: Arc<P5Refs>, art: P5Art, settings: MapSettings) -> Self {
        Self {
            refs,
            art,
            view: None,
            settings,
            user_settings: false,
            prefs: tessera_settings::MapPrefs::default(),
            prefs_changed: false,
            on_map: BTreeMap::new(),
            map_rev: None,
            radar: MapOverlays::default(),
            on_radar: BTreeMap::new(),
            radar_rev: None,
            names: HashMap::new(),
            dungeon_zone: None,
            player_dungeon: None,
            fallback_tree: MapTree::default(),
            dungeon_notes_pending: false,
            layer: tessera_gamedata::map::LAYER_INSTANCE_ZERO,
            failures: Vec::new(),
        }
    }

    /// The content layer of the note selection (`map-windows.md` §8: the walker's first
    /// `--content-layers` value; default 1).
    pub fn set_content_layer(&mut self, layer: u32) {
        self.layer = layer;
    }

    /// The tree Tessera's fallback window navigates (used when the install's `map` layout
    /// is missing; synthetic trees in tests). Takes effect at the next new window.
    pub fn set_fallback_tree(&mut self, tree: MapTree) {
        self.fallback_tree = tree;
    }

    /// The player's dungeon as last recorded this session (§4.6 "When").
    #[must_use]
    pub fn player_dungeon(&self) -> Option<u32> {
        self.player_dungeon
    }

    /// The user's saved map settings (`[Map]`, §6.9): laid over the settings in use now and
    /// over the install's defaults when they arrive.
    pub fn set_prefs(&mut self, prefs: tessera_settings::MapPrefs) {
        self.prefs = prefs;
        self.settings = self.settings.with_prefs(&prefs);
    }

    /// The user's map settings when a filter or the presentation changed since the last
    /// call (to be saved).
    pub fn take_changed_prefs(&mut self) -> Option<tessera_settings::MapPrefs> {
        std::mem::take(&mut self.prefs_changed).then_some(self.prefs)
    }

    /// The install's facts arrived after this window was created: the settings take the
    /// install's defaults (`GameplayOptionDefaults`, §6.9) unless the user changed them, an
    /// existing window is configured again (icons, menu, filter values, switches; its
    /// visibility kept) and the shown map's notes are given; the markers are converted
    /// again (record-name labels).
    pub fn facts_arrived(&mut self, ui: &mut Ui, facts: &MapFacts, texts: &dyn MapTexts) {
        if !self.user_settings {
            self.settings = facts.defaults.with_prefs(&self.prefs);
        }
        self.names.clear();
        self.on_map.clear();
        self.map_rev = None;
        self.on_radar.clear();
        self.radar_rev = None;
        let shown_before = self.is_shown(ui);
        let Some(mut view) = self.view.take() else {
            return;
        };
        if !ui.exists(view.window) {
            return;
        }
        self.configure(ui, &mut view, Some(facts));
        let shown = view.shown;
        if shown.is_some() {
            // The view reads the filter at a show; the shown map follows the new values.
            view.filter = read_filter(view.filter_value(view.filter_option));
            view.filter_menu =
                FilterMenuModel::build(view.display_mask(), view.filter, texts, &view.icons);
        }
        self.view = Some(view);
        if !shown_before {
            self.hide(ui);
        }
        if let Some(s) = shown {
            self.give_notes(ui, s, Some(facts), texts);
        }
    }

    /// The view, when open.
    #[must_use]
    pub fn view(&self) -> Option<&MapWindowView> {
        self.view.as_ref()
    }

    /// The view, mutably (tests, the census).
    pub fn view_mut(&mut self) -> Option<&mut MapWindowView> {
        self.view.as_mut()
    }

    /// The minimaps' overlays (the plain feed markers).
    #[must_use]
    pub fn radar_overlays(&self) -> &MapOverlays {
        &self.radar
    }

    /// The settings in use.
    #[must_use]
    pub fn settings(&self) -> MapSettings {
        self.settings
    }

    /// Distinct failure reasons so far.
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Records a failure; logged once per distinct reason.
    fn fail(&mut self, why: String) {
        if self.failures.contains(&why) {
            return;
        }
        tracing::warn!("map window: {why}");
        if self.failures.len() < MAX_FAILURES {
            self.failures.push(why);
        }
    }

    /// Whether the window exists and is shown.
    #[must_use]
    pub fn is_shown(&self, ui: &Ui) -> bool {
        self.view
            .as_ref()
            .is_some_and(|v| ui.exists(v.root()) && ui.is_visible(v.root()))
    }

    /// Opens the window (or shows it again): the install's `map` layout, else the
    /// fallback window. Returns its root.
    ///
    /// # Errors
    /// When neither can be built.
    pub fn open(
        &mut self,
        ui: &mut Ui,
        facts: Option<&MapFacts>,
        player: Option<&PlayerMap>,
        texts: &dyn MapTexts,
    ) -> Result<ElementKey, String> {
        if let Some(v) = self.view.as_ref()
            && ui.exists(v.root())
        {
            let root = v.root();
            ui.set_shown(root, true);
            ui.request_activation(root);
            return Ok(root);
        }
        self.view = None;
        self.on_map.clear();
        self.map_rev = None;
        let refs = Arc::clone(&self.refs);
        let (gv, pv, nv) = (&refs.gv, &refs.pv, &refs.nv);
        nv.register_classes(ui);
        pv.register_classes(ui);
        let view = match self.refs.map {
            Some(layout) => MapWindowView::open_layout(ui, gv, pv, nv, layout, &self.art)
                .map_err(|e| e.to_string()),
            None => Err("no install layout".to_owned()),
        };
        let mut view = match view {
            Ok(v) => v,
            Err(e) => {
                if self.refs.map.is_some() {
                    self.fail(format!("LayoutMissing ({e}); fallback window"));
                }
                let tree = self.fallback_tree.clone();
                MapWindowView::open_fallback(ui, gv, pv, nv, tree, &self.art, texts)
                    .map_err(|e| e.to_string())?
            }
        };
        self.configure(ui, &mut view, facts);
        // §4.6 "When": the dungeon recorded this session, so that only a later change of
        // the player's dungeon runs the selection (a window opened by place lookup keeps
        // the place it shows).
        view.set_recorded_dungeon(self.player_dungeon);
        let root = view.root();
        self.view = Some(view);
        if let Some(p) = player {
            self.show_player_map(ui, p, facts, texts);
        }
        if let Some(v) = self.view.as_ref() {
            ui.set_shown(v.root(), true);
            ui.request_activation(v.root());
        }
        Ok(root)
    }

    fn configure(&self, ui: &mut Ui, view: &mut MapWindowView, facts: Option<&MapFacts>) {
        view.set_icons(facts.map_or_else(IconTable::default, |f| f.map_icons.clone()));
        let pv = &self.refs.pv;
        let (near, distant) =
            facts.map_or((pv.pids.gop_map_filter, pv.pids.gop_distant_filter), |f| {
                (
                    if f.filter_pids.0 != 0 {
                        f.filter_pids.0
                    } else {
                        pv.pids.gop_map_filter
                    },
                    if f.filter_pids.1 != 0 {
                        f.filter_pids.1
                    } else {
                        pv.pids.gop_distant_filter
                    },
                )
            });
        view.set_filter_value(near, self.settings.near);
        view.set_filter_value(distant, self.settings.distant);
        if let Some(f) = facts {
            let names = view
                .tree()
                .map(|t| {
                    t.nodes
                        .iter()
                        .map(|n| n.menu_name.as_ref().and_then(|i| f.render(i)))
                        .collect()
                })
                .unwrap_or_default();
            view.set_menu(f.region_menu.clone(), names);
        }
        view.set_show_coordinates(ui, self.settings.coordinates);
        view.set_presentation(
            ui,
            if self.settings.fullscreen {
                Presentation::FullScreen
            } else {
                Presentation::Windowed
            },
        );
        view.fit_full_screen(ui, 1.0);
    }

    /// Opens the window when closed, hides it when shown (`ToggleMapPanel`).
    pub fn toggle(
        &mut self,
        ui: &mut Ui,
        facts: Option<&MapFacts>,
        player: Option<&PlayerMap>,
        texts: &dyn MapTexts,
    ) {
        if self.is_shown(ui) {
            self.hide(ui);
        } else if let Err(e) = self.open(ui, facts, player, texts) {
            self.fail(e);
        }
    }

    /// Hides the window (the shown map is kept, MWI8).
    pub fn hide(&mut self, ui: &mut Ui) {
        if let Some(v) = self.view.as_ref() {
            for k in [Some(v.window), v.windowed_root].into_iter().flatten() {
                if ui.exists(k) {
                    ui.set_shown(k, false);
                }
            }
        }
    }

    /// The player's map (§4.6): on every open of a new window and on `MapZoom`.
    pub fn show_player_map(
        &mut self,
        ui: &mut Ui,
        player: &PlayerMap,
        facts: Option<&MapFacts>,
        texts: &dyn MapTexts,
    ) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        if let Some((id, _)) = &player.dungeon {
            // The player's dungeon map shows the notes of that dungeon (§4.2 step 5, MW11).
            self.dungeon_zone = Some(Did(*id));
        }
        let r = view.show_player_map(ui, player, texts);
        self.after(ui, r, facts, texts);
    }

    /// One frame: markers follow `state`, the player marker follows `player`, then the
    /// UI events. Returns what the client keeps (settings changes, a close).
    pub fn frame(
        &mut self,
        ui: &mut Ui,
        state: &P5State,
        facts: Option<&MapFacts>,
        player: Option<&PlayerMap>,
        events: &[UiEvent],
        texts: &dyn MapTexts,
    ) -> Vec<MapChange> {
        let mut out = Vec::new();
        // §4.6 "When": the player's dungeon is recorded whenever it changes, with or without
        // a window; with a window the change runs the selection below.
        let now = player.map(|p| p.dungeon.as_ref().map(|(id, _)| *id));
        let dungeon_changed = now.is_some_and(|d| d != self.player_dungeon);
        if let Some(d) = now {
            self.player_dungeon = d;
        }
        let Some(view) = self.view.as_mut() else {
            return out;
        };
        if !ui.exists(view.window) {
            self.view = None;
            self.on_map.clear();
            self.map_rev = None;
            return out;
        }
        view.refresh(ui, texts);
        // Feed markers (§6.7 item 7), only when the state's markers changed: the removed
        // ones go, the new and replaced ones are converted and given.
        let markers = state.markers();
        let rev = state.markers_revision();
        if self.map_rev != Some(rev) || self.on_map.len() != markers.len() {
            self.map_rev = Some(rev);
            let gone: Vec<u32> = self
                .on_map
                .keys()
                .filter(|id| !markers.contains_key(id))
                .copied()
                .collect();
            for id in gone {
                view.remove_marker(ui, id, texts);
                self.on_map.remove(&id);
            }
            for (id, m) in markers {
                if self.on_map.get(id) != Some(m) {
                    view.add_marker(ui, cached_marker(&mut self.names, *id, m, facts), texts);
                    self.on_map.insert(*id, m.clone());
                }
            }
        }
        if let Some(p) = player
            && p.valid
        {
            view.update_player(ui, p);
        }
        // §4.6 "When": the selection runs when the player's dungeon changes (entering one,
        // leaving it, or another one); navigation and lookup never change the record.
        if dungeon_changed && let Some(p) = player {
            self.show_player_map(ui, p, facts, texts);
        }
        let Some(view) = self.view.as_mut() else {
            return out;
        };
        // A dungeon map shown before its image size was known: its notes, once it is.
        let notes_due = self.dungeon_notes_pending
            && view.shown == Some(Shown::Dungeon)
            && view.dungeon.as_ref().is_some_and(|d| d.size.is_some());
        if notes_due {
            self.give_notes(ui, Shown::Dungeon, facts, texts);
        }
        let Some(view) = self.view.as_mut() else {
            return out;
        };
        let mut actions = Vec::new();
        for ev in events {
            actions.extend(view.handle_event(ui, ev, texts));
        }
        for a in actions {
            match a {
                MapAction::Shown(s) => self.give_notes(ui, s, facts, texts),
                MapAction::FilterChanged { option, value } => {
                    self.user_settings = true;
                    self.keep_filter(option, value, facts);
                    out.push(MapChange::Filter { option, value });
                }
                MapAction::PresentationChanged { full_screen } => {
                    self.user_settings = true;
                    self.settings.fullscreen = full_screen;
                    self.prefs.fullscreen = Some(full_screen);
                    self.prefs_changed = true;
                    out.push(MapChange::Presentation { full_screen });
                }
                MapAction::Close => {
                    self.hide(ui);
                    out.push(MapChange::Closed);
                }
            }
        }
        out
    }

    fn keep_filter(&mut self, option: u32, value: u64, facts: Option<&MapFacts>) {
        let pv = &self.refs.pv;
        let (near, distant) = facts.map_or((0, 0), |f| f.filter_pids);
        if option == near || option == pv.pids.gop_map_filter {
            self.settings.near = value;
            self.prefs.near = Some(value);
            self.prefs_changed = true;
        } else if option == distant || option == pv.pids.gop_distant_filter {
            self.settings.distant = value;
            self.prefs.distant = Some(value);
            self.prefs_changed = true;
        }
        if let Some(v) = self.view.as_mut() {
            v.set_filter_value(option, value);
        }
    }

    fn after(
        &mut self,
        ui: &mut Ui,
        r: Result<Option<MapAction>, tessera_ui::game::GameError>,
        facts: Option<&MapFacts>,
        texts: &dyn MapTexts,
    ) {
        match r {
            Ok(Some(MapAction::Shown(s))) => self.give_notes(ui, s, facts, texts),
            Ok(_) => {}
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// The notes of the shown map (§6.3) from `tessera-gamedata`'s selection.
    fn give_notes(
        &mut self,
        ui: &mut Ui,
        s: Shown,
        facts: Option<&MapFacts>,
        texts: &dyn MapTexts,
    ) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        if !matches!(s, Shown::Dungeon) {
            self.dungeon_zone = None;
        }
        // A dungeon map's notes need its image size (the frame of its georeference).
        self.dungeon_notes_pending =
            matches!(s, Shown::Dungeon) && view.dungeon.as_ref().is_none_or(|d| d.size.is_none());
        let notes = facts.map(|f| {
            notes_for(
                f,
                view,
                s,
                self.dungeon_zone,
                self.layer,
                &self.settings,
                texts,
            )
        });
        view.set_notes(ui, notes.unwrap_or_default(), texts);
    }

    /// Images arrived: a dungeon map whose image size was unknown is fitted
    /// ([`MapWindowView::refresh`]) and, once the size is known, its notes are given.
    pub fn refresh(&mut self, ui: &mut Ui, facts: Option<&MapFacts>, texts: &dyn MapTexts) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        view.refresh(ui, texts);
        if self.dungeon_notes_pending
            && view.shown == Some(Shown::Dungeon)
            && view.dungeon.as_ref().is_some_and(|d| d.size.is_some())
        {
            self.give_notes(ui, Shown::Dungeon, facts, texts);
        }
    }

    /// Shows a dungeon map for `zone` (the place lookup, the census).
    pub fn show_dungeon(
        &mut self,
        ui: &mut Ui,
        zone: Option<Did>,
        map: GameMap,
        facts: Option<&MapFacts>,
        texts: &dyn MapTexts,
    ) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        self.dungeon_zone = zone;
        let r = view.show_dungeon(ui, map, texts);
        self.after(ui, r, facts, texts);
    }

    /// `findplace TEXT` (§7.3): the result lines; the first result is shown with the
    /// lookup marker (the window opens when closed). Without text the lookup marker goes.
    pub fn lookup(
        &mut self,
        ui: &mut Ui,
        facts: Option<&MapFacts>,
        text: &str,
        player: Option<&PlayerMap>,
        texts: &dyn MapTexts,
    ) -> Vec<String> {
        if text.trim().is_empty() {
            if let Some(v) = self.view.as_mut() {
                v.clear_lookup(ui, texts);
            }
            return Vec::new();
        }
        let Some(f) = facts else {
            return vec!["Place lookup needs the game data.".to_owned()];
        };
        let shown_region = self
            .view
            .as_ref()
            .and_then(|v| v.context())
            .and_then(|c| c.region);
        let LookupOutcome { lines, show } =
            find_place(f, text, shown_region, player.map(|p| p.point.region));
        let Some((target, marker)) = show else {
            return lines;
        };
        if let Err(e) = self.open(ui, facts, None, texts) {
            self.fail(e);
            return lines;
        }
        let Some(view) = self.view.as_mut() else {
            return lines;
        };
        let target = match target {
            PlaceTarget::Node(el) => match view.tree().and_then(|t| t.node_by_element(el)) {
                Some(n) => LookupTarget::Node(n),
                None => LookupTarget::Area(None),
            },
            PlaceTarget::Dungeon { map, zone } => {
                // The dungeon's notes are those of its zone (§4.2 step 5, MW11).
                self.dungeon_zone = Some(zone);
                LookupTarget::Dungeon(map)
            }
            PlaceTarget::Area(a) => LookupTarget::Area(a),
        };
        let r = view.show_lookup(ui, target, marker, texts);
        self.after(ui, r, facts, texts);
        lines
    }

    /// The plain feed markers on every minimap of `radars` for the player's position
    /// `input` (§6.8 Tessera rule; quest markers are the quest journal's).
    pub fn update_radars(
        &mut self,
        ui: &mut Ui,
        radars: &[Radar],
        input: &RadarInput,
        state: &P5State,
        facts: Option<&MapFacts>,
        texts: &dyn MapTexts,
    ) {
        // Only when the state's markers changed: removed (or now quest) markers go, new
        // and replaced plain markers are given again (§6.7 item 7 on the minimap, §6.8).
        let markers = state.markers();
        let rev = state.markers_revision();
        if self.radar_rev != Some(rev) {
            self.radar_rev = Some(rev);
            let gone: Vec<u32> = self
                .on_radar
                .keys()
                .filter(|id| markers.get(id).is_none_or(|m| m.quest.is_some()))
                .copied()
                .collect();
            for id in gone {
                self.radar.remove_marker(ui, id);
                self.on_radar.remove(&id);
            }
            for (id, m) in markers {
                if m.quest.is_none() && self.on_radar.get(id) != Some(m) {
                    self.radar
                        .add_marker(ui, cached_marker(&mut self.names, *id, m, facts));
                    self.on_radar.insert(*id, m.clone());
                }
            }
        }
        if radars.is_empty() {
            return;
        }
        let empty = IconTable::default();
        let ctx = RadarContext {
            icons: facts.map_or(&empty, |f| &f.radar_icons),
            filter: self.settings.radar,
            player_height: 0.0,
            instance: 0,
        };
        for r in radars {
            self.radar
                .update_radar(ui, &self.refs.gv, &self.refs.pv, r, input, &ctx, texts);
        }
    }

    /// Destroys the window (the UI is going away).
    pub fn close(&mut self, ui: &mut Ui) {
        if let Some(mut v) = self.view.take() {
            v.detach(ui);
            for k in [Some(v.window), v.windowed_root].into_iter().flatten() {
                if ui.exists(k) {
                    ui.destroy(k);
                }
            }
        }
        self.on_map.clear();
        self.map_rev = None;
    }
}

/// The notes of shown map `s` (§6.3): `tessera-gamedata` selects the table notes for the
/// node (by its active element) or the dungeon zone; the view places them and applies the
/// filter. World-event rows without a holding state are left out (`map-windows.md` §8).
fn notes_for(
    f: &MapFacts,
    view: &MapWindowView,
    s: Shown,
    zone: Option<Did>,
    layer: u32,
    settings: &MapSettings,
    texts: &dyn MapTexts,
) -> Vec<MapNote> {
    let (Some(regions), Some(table), Some(index), Some(pids)) = (
        f.regions.as_ref(),
        f.notes.as_ref(),
        f.note_index.as_ref(),
        f.pids.as_ref(),
    ) else {
        return Vec::new();
    };
    let (shown, frame) = match s {
        Shown::Node(i) => {
            let Some(n) = view.tree().and_then(|t| t.nodes.get(i)) else {
                return Vec::new();
            };
            let Some(gi) = regions.tree.node_by_element(n.active_element) else {
                return Vec::new();
            };
            let frame = if n.pixel_size.0 > 0 && n.pixel_size.1 > 0 {
                n.pixel_size
            } else {
                DEFAULT_FRAME
            };
            (ShownMap::Node(gi), frame)
        }
        Shown::Dungeon => {
            let (Some(zone), Some(d)) = (zone, view.dungeon.as_ref()) else {
                return Vec::new();
            };
            let Some(size) = d.size else {
                return Vec::new();
            };
            let georef = d.map.georef().map(|g| tessera_gamedata::map::Georef {
                block_x: g.block_x,
                block_y: g.block_y,
                pixel_x: g.pixel_x,
                pixel_y: g.pixel_y,
                scale: g.scale,
            });
            (
                ShownMap::Dungeon {
                    zone,
                    georef,
                    frame: (size.0.unsigned_abs(), size.1.unsigned_abs()),
                },
                size,
            )
        }
        Shown::Element(_) => return Vec::new(),
    };
    let ns = NoteSettings {
        layer,
        discovery_all: settings.discovery_all,
        filter: view.filter,
    };
    let frame = (frame.0.unsigned_abs(), frame.1.unsigned_abs());
    let render = |i: &tessera_formats::StringInfo| f.render(i);
    select_notes(
        table,
        index,
        &regions.tree,
        shown,
        frame,
        &ns,
        pids,
        &render,
    )
    .into_iter()
    .filter(|n| !n.world_event_hidden && !n.link)
    .filter_map(|n| {
        let l = n.location?;
        #[allow(clippy::cast_possible_truncation)]
        let point = MapPoint {
            global: (l.x as f32, l.y as f32),
            region: u8::try_from(l.region).ok()?,
            instance: 0,
        };
        let text = n.text.clone().unwrap_or_default();
        // MW15 from the install (§6.3 revision 1), Tessera's wording when it is missing.
        let tooltip = texts
            .note_tooltip(&text, n.type_bits, &point)
            .unwrap_or_else(|| {
                marker_tooltip(
                    texts,
                    &UiMarker {
                        point,
                        label: text.clone(),
                        ..UiMarker::default()
                    },
                )
            });
        Some(MapNote {
            id: u64::try_from(n.row).unwrap_or(u64::MAX),
            note_type: n.type_bits,
            point,
            area: None,
            tooltip,
            text,
            icons: None,
            offscreen: None,
            range: None,
            height: 0.0,
            // `tessera-gamedata` applied the creation tests (display mask, area node).
            runtime: true,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::Ref;
    use tessera_net::model::{Location, RegionId};

    #[test]
    fn markers_convert_with_their_kind() {
        let m = FeedMarker {
            location: Location::new(RegionId(1), 16_100.0, 16_050.0, 0.0),
            label: Text::Literal("Camp".into()),
            icon: None,
            quest: Some(Ref::Literal("Q".into())),
            note_type: Some(48),
        };
        let u = ui_marker(9, &m, None);
        assert_eq!(u.id, 9);
        assert!(u.quest);
        assert_eq!(u.point.region, 1);
        assert_eq!(u.label, "Camp");
        assert_eq!(u.note_type, Some(48));
    }

    #[test]
    fn player_map_validity() {
        let input = RadarInput {
            region: 1,
            x: 100,
            y: 100,
            has_player: true,
            ..RadarInput::default()
        };
        assert!(player_map(&input, None, None).valid);
        let bad = RadarInput { x: 0xFF, ..input };
        assert!(!player_map(&bad, None, None).valid);
    }
}
