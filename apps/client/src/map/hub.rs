// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P5 map glue: loads the install's map facts ([`MapFacts`]) off the frame,
//! and runs the map window and the minimap's feed markers ([`MapUi`]) on the game UI.
//!
//! Frame order (windowed client, after the NPC windows and the quest journal):
//! [`MapHub::frame`] takes the frame's UI events (the NPC hub's collector), the minimap's
//! `MapZoom` requests (the native panels hand them over instead of opening P10 while the
//! hub is attached), the map key (`ToggleMapPanel`, M) and the `findplace` lines of the
//! dispatcher, and posts the lookup's lines to the chat. Without the game UI (`--ui` not
//! given, or headless) no window answers `findplace`: [`MapHub::serve_lookups_without_ui`]
//! posts the lookup's lines once the facts are loaded.
//!
//! The filter values and switches live in memory for the run, initialised from the
//! install's `GameplayOptionDefaults` once the facts are loaded (a window created before
//! them takes the defaults when they arrive, [`MapUi::facts_arrived`]): Tessera's settings
//! file has no map keys yet (`map-windows.md` §6.9, §8; OPEN there).

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use tessera_dat::ArchiveSet;
use tessera_input::{KeyCode, RawEvent};
use tessera_ui::UiEvent;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::radar::RadarInput;

use super::data::{MapFacts, MapSettings};
use super::dungeon::{PlayerDungeon, reported_dungeon};
use super::lookup::find_place;
use super::texts::InstallMapTexts;
use super::ui::{MapUi, player_map};
use crate::character::CharacterData;
use crate::commands::LocalLine;
use crate::npc::NpcHub;
use crate::travel::refs::P5Refs;
use crate::ui_host::UiHost;

/// The `--open-panel` names of the map window (`map`, `world-map`; ASCII case ignored).
#[must_use]
pub fn parse_map_panel(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "map" | "world-map"
    )
}

/// The input key of `ToggleMapPanel` (M with no modifier, `travel-windows.md` TW14).
pub const MAP_KEY: KeyCode = KeyCode::M;

/// The modifier keys, each tracked on its own.
const MODIFIERS: [KeyCode; 8] = [
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::SuperLeft,
    KeyCode::SuperRight,
];

/// A key press the P5 hubs act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P5Key {
    /// M alone: `ToggleMapPanel`.
    Map,
    /// Ctrl + M: `toggle_mount` (`walker.md` §16.4, policy exact).
    Mount,
    /// Ctrl + J: `ToggleWorldJoin` (the finder, `travel-windows.md` TW14).
    Finder,
}

/// Follows the modifier keys and reports the P5 key presses (exact modifier sets). Feed it
/// every key event, also while a text field has the keyboard, so a modifier released there
/// is not left held; gate only the reported action on the focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct P5Keys {
    held: [bool; 8],
}

impl P5Keys {
    /// Takes one raw event; the key it triggers, if any.
    pub fn event(&mut self, ev: &RawEvent) -> Option<P5Key> {
        let RawEvent::Key {
            key,
            pressed,
            repeat,
        } = ev
        else {
            return None;
        };
        if let Some(i) = MODIFIERS.iter().position(|k| k == key) {
            self.held[i] = *pressed;
            return None;
        }
        if !*pressed || *repeat {
            return None;
        }
        let ctrl = self.held[2] || self.held[3];
        let others = self.held[0] || self.held[1] || self.held[4..].iter().any(|h| *h);
        match (key, ctrl, others) {
            (KeyCode::M, false, false) => Some(P5Key::Map),
            (KeyCode::M, true, false) => Some(P5Key::Mount),
            (KeyCode::J, true, false) => Some(P5Key::Finder),
            _ => None,
        }
    }

    /// Forgets the held modifiers (focus lost).
    pub fn release_all(&mut self) {
        self.held = [false; 8];
    }
}

/// The runtime's P5 map glue (module docs).
#[derive(Default)]
pub struct MapHub {
    facts: Option<Arc<MapFacts>>,
    loading: Option<JoinHandle<MapFacts>>,
    started: bool,
    ui: Option<MapUi>,
    /// The loaded facts were given to the window ([`MapUi::facts_arrived`]).
    facts_applied: bool,
    open_requested: bool,
    toggle_requested: bool,
    lookups: Vec<String>,
    keys: P5Keys,
    /// A game UI will run the map window (`--ui` in the windowed client): `findplace`
    /// waits for it; otherwise it is answered by [`Self::serve_lookups_without_ui`].
    ui_expected: bool,
    /// The note selection's content layer (`--content-layers`, first value).
    layer: u32,
    /// The user's saved map settings (`[Map]`), given to the window when it is made.
    prefs: tessera_settings::MapPrefs,
    /// Changes of the window's settings are written to Tessera's settings file
    /// ([`Self::with_saved_prefs`]).
    persist: bool,
    /// The player's dungeon (`map-windows.md` §4.6 revision 1: the session's
    /// `Dungeon_CurrentDungeon`, else the interior cell's `Dungeon_DID`, read off the frame).
    dungeon: PlayerDungeon,
}

impl std::fmt::Debug for MapHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapHub")
            .field("facts", &self.facts)
            .field("ui", &self.ui)
            .finish_non_exhaustive()
    }
}

/// Most `findplace` lines queued between two frames (Tessera bound).
const MAX_LOOKUPS: usize = 8;

impl MapHub {
    /// Nothing loaded; `open` (from `--open-panel map`) opens the window once the player
    /// is in the world.
    #[must_use]
    pub fn new(open: bool) -> Self {
        Self {
            open_requested: open,
            layer: tessera_gamedata::map::LAYER_INSTANCE_ZERO,
            ..Self::default()
        }
    }

    /// Whether a game UI will run the map window (`--ui` in the windowed client).
    #[must_use]
    pub fn with_ui(mut self, ui: bool) -> Self {
        self.ui_expected = ui;
        self
    }

    /// The content layer of the note selection (`map-windows.md` §8: the walker's first
    /// `--content-layers` value; 1 when none is given).
    #[must_use]
    pub fn with_content_layer(mut self, layer: Option<u32>) -> Self {
        self.layer = layer.unwrap_or(tessera_gamedata::map::LAYER_INSTANCE_ZERO);
        self
    }

    /// The user's map settings from Tessera's settings file (`[Map]`, `map-windows.md`
    /// §6.9): laid over the install's defaults, and changes made in the window are written
    /// back (off the frame). Without a readable settings file nothing is saved.
    #[must_use]
    pub fn with_saved_prefs(mut self) -> Self {
        match tessera_settings::SettingsStore::open_default() {
            Ok(store) => {
                let (prefs, warnings) =
                    tessera_settings::MapPrefs::from_settings(&store.load().settings);
                for w in warnings {
                    tracing::warn!("settings: {w}");
                }
                self.prefs = prefs;
                self.persist = true;
            }
            Err(e) => tracing::debug!("settings unavailable ({e}); map settings not saved"),
        }
        self
    }

    /// Whether the facts' loader runs.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// Whether loading the facts was started.
    #[must_use]
    pub fn started(&self) -> bool {
        self.started
    }

    /// The facts, once loaded.
    #[must_use]
    pub fn facts(&self) -> Option<&Arc<MapFacts>> {
        self.facts.as_ref()
    }

    /// The window, once the UI is attached.
    #[must_use]
    pub fn ui(&self) -> Option<&MapUi> {
        self.ui.as_ref()
    }

    /// Starts loading the facts on a worker thread (once).
    pub fn start(
        &mut self,
        character: Arc<CharacterData>,
        set: Arc<ArchiveSet>,
        root: Option<PathBuf>,
    ) {
        if self.started {
            return;
        }
        self.started = true;
        match std::thread::Builder::new()
            .name("tessera-p5-map".into())
            .spawn(move || MapFacts::load(character, &set, root.as_deref()))
        {
            Ok(h) => self.loading = Some(h),
            Err(e) => tracing::warn!("map facts: no loader thread: {e}"),
        }
    }

    /// Takes the facts when loaded (never blocks).
    pub fn poll(&mut self) {
        if self.loading.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(h) = self.loading.take()
        {
            match h.join() {
                Ok(f) => {
                    tracing::info!(facts = ?f, "map facts loaded");
                    self.facts = Some(Arc::new(f));
                }
                Err(_) => tracing::warn!("the map loader panicked"),
            }
        }
    }

    /// Uses already loaded facts (tests, tools).
    pub fn set_loaded(&mut self, facts: Arc<MapFacts>) {
        self.facts = Some(facts);
    }

    /// One raw input event of the windowed client with the game UI. Every event updates
    /// the modifier state; while `focused` (a UI element has the keyboard) no key is
    /// triggered. Otherwise the P5 key it triggers (M toggles the map window in the next
    /// [`Self::frame`]; the caller handles the others).
    pub fn key(&mut self, ev: &RawEvent, focused: bool) -> Option<P5Key> {
        let k = self.keys.event(ev)?;
        if focused {
            return None;
        }
        if k == P5Key::Map {
            self.toggle_requested = true;
        }
        Some(k)
    }

    /// The window lost the keyboard.
    pub fn release_keys(&mut self) {
        self.keys.release_all();
    }

    /// Opens the window in the next frame (`MapZoom`, `--open-panel map`).
    pub fn request_open(&mut self) {
        self.open_requested = true;
    }

    /// A `findplace` line (§7.3): runs in the next frame with the player in the world
    /// (with the game UI) or once the facts are loaded (without it). `false` when
    /// `MAX_LOOKUPS` are already waiting (the line is dropped).
    pub fn request_lookup(&mut self, text: String) -> bool {
        if self.lookups.len() < MAX_LOOKUPS {
            self.lookups.push(text);
            true
        } else {
            false
        }
    }

    /// Whether `findplace` lines wait that no map window will answer.
    #[must_use]
    pub fn lookups_without_ui(&self) -> bool {
        !self.ui_expected && !self.lookups.is_empty()
    }

    /// Without the game UI: the lines of the waiting `findplace` lookups (§7.3 results or
    /// "No place found."; nothing is shown). They keep waiting while `pending` (the facts
    /// are still to come); without facts they are answered with an error line.
    /// `player_region` is the region of a coordinate text.
    pub fn serve_lookups_without_ui(
        &mut self,
        pending: bool,
        player_region: Option<u8>,
    ) -> Vec<LocalLine> {
        if self.ui_expected || self.lookups.is_empty() {
            return Vec::new();
        }
        let Some(facts) = self.facts.clone() else {
            if pending {
                return Vec::new();
            }
            self.lookups.clear();
            return vec![LocalLine::error("Place lookup needs the game data.")];
        };
        let mut out = Vec::new();
        for t in std::mem::take(&mut self.lookups) {
            if t.trim().is_empty() {
                continue;
            }
            out.extend(
                find_place(&facts, &t, None, player_region)
                    .lines
                    .into_iter()
                    .map(LocalLine::log),
            );
        }
        out
    }

    /// One frame of the map window on `host` (module docs).
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &crate::net::World,
        refs: Option<&Arc<P5Refs>>,
        npc: &NpcHub,
        events: &[UiEvent],
        radar: Option<&RadarInput>,
    ) {
        if let Some(p) = host.panels() {
            p.set_world_map_external(true);
            if p.take_world_map_requests() > 0 {
                self.open_requested = true;
            }
        }
        let Some(refs) = refs.cloned() else {
            if !self.lookups.is_empty() {
                self.lookups.clear();
                host.post_notices(["The map is not ready yet.".to_owned()]);
            }
            return;
        };
        if world.player().is_none() {
            // Lookups wait for the player (the window opens in the world).
            return;
        }
        let lookups = std::mem::take(&mut self.lookups);
        let facts = self.facts.clone();
        // The facts' defaults: at creation when loaded, else when they arrive.
        let fresh = facts.is_some() && !self.facts_applied;
        self.facts_applied |= facts.is_some();
        let created = self.ui.is_none();
        let layer = self.layer;
        let prefs = self.prefs;
        let ui = self.ui.get_or_insert_with(|| {
            let settings = facts
                .as_ref()
                .map_or_else(MapSettings::default, |f| f.defaults);
            let mut m = MapUi::new(refs, P5Art::default(), settings);
            m.set_content_layer(layer);
            m.set_prefs(prefs);
            m
        });
        let data = npc.data().cloned();
        let texts = InstallMapTexts::new(facts.as_ref().map(|f| &f.constants), data.as_deref());
        // §4.6 revision 1: the session's `Dungeon_CurrentDungeon` first, else the interior
        // cell's `Dungeon_DID`.
        let reader = facts.as_ref().and_then(|f| f.cell_reader.as_ref());
        let reported = reported_dungeon(
            world.player_state(),
            facts.as_ref().and_then(|f| f.current_dungeon_pid),
        );
        let dungeon = radar.and_then(|r| self.dungeon.update(r, reported, reader));
        let player = radar.map(|r| player_map(r, facts.as_deref(), dungeon));
        let mut lines = Vec::new();
        {
            let mut hui = host.ui_mut();
            if fresh
                && !created
                && let Some(f) = facts.as_deref()
            {
                ui.facts_arrived(&mut hui, f, &texts);
            }
            if std::mem::take(&mut self.open_requested) {
                match ui.open(&mut hui, facts.as_deref(), player.as_ref(), &texts) {
                    Ok(_) => {
                        if let Some(p) = &player {
                            ui.show_player_map(&mut hui, p, facts.as_deref(), &texts);
                        }
                    }
                    Err(e) => tracing::warn!("map window: {e}"),
                }
            }
            if std::mem::take(&mut self.toggle_requested) {
                ui.toggle(&mut hui, facts.as_deref(), player.as_ref(), &texts);
            }
            for t in lookups {
                lines.extend(ui.lookup(&mut hui, facts.as_deref(), &t, player.as_ref(), &texts));
            }
            for c in ui.frame(
                &mut hui,
                world.p5(),
                facts.as_deref(),
                player.as_ref(),
                events,
                &texts,
            ) {
                tracing::debug!(?c, "map window");
            }
        }
        if let (Some(panels), Some(input)) = (host.panels(), radar) {
            ui.update_radars(
                &mut host.ui_mut(),
                &panels.game.radars,
                input,
                world.p5(),
                facts.as_deref(),
                &texts,
            );
        }
        if !lines.is_empty() {
            host.post_notices(lines);
        }
        if let Some(p) = ui.take_changed_prefs() {
            self.prefs = p;
            if self.persist {
                save_prefs(p);
            }
        }
    }
}

/// Writes the map settings to Tessera's settings file on a short-lived thread (a locked
/// read-modify-write; failures are logged).
fn save_prefs(prefs: tessera_settings::MapPrefs) {
    let job = move || {
        let store = match tessera_settings::SettingsStore::open_default() {
            Ok(s) => s,
            Err(e) => {
                tracing::info!("map settings not saved: {e}");
                return;
            }
        };
        if let Err(e) = store.update(|s| prefs.store(s)) {
            tracing::info!("map settings not saved: {e}");
        }
    };
    if let Err(e) = std::thread::Builder::new()
        .name("tessera-map-settings".into())
        .spawn(job)
    {
        tracing::info!("map settings not saved: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: KeyCode, pressed: bool) -> RawEvent {
        RawEvent::Key {
            key: k,
            pressed,
            repeat: false,
        }
    }

    #[test]
    fn keys_need_exact_modifiers() {
        let mut k = P5Keys::default();
        assert_eq!(k.event(&key(KeyCode::M, true)), Some(P5Key::Map));
        assert_eq!(k.event(&key(KeyCode::ControlLeft, true)), None);
        assert_eq!(k.event(&key(KeyCode::M, true)), Some(P5Key::Mount));
        assert_eq!(k.event(&key(KeyCode::J, true)), Some(P5Key::Finder));
        assert_eq!(k.event(&key(KeyCode::ShiftLeft, true)), None);
        assert_eq!(k.event(&key(KeyCode::M, true)), None);
        k.release_all();
        assert_eq!(k.event(&key(KeyCode::M, true)), Some(P5Key::Map));
        assert!(parse_map_panel(" Map "));
        assert!(!parse_map_panel("P10"));
    }

    #[test]
    fn modifiers_released_under_focus_are_not_left_held() {
        let mut hub = MapHub::new(false);
        // Ctrl pressed in the world, released while the chat line has the keyboard.
        assert_eq!(hub.key(&key(KeyCode::ControlLeft, true), false), None);
        assert_eq!(hub.key(&key(KeyCode::ControlLeft, false), true), None);
        // M typed into the chat triggers nothing.
        assert_eq!(hub.key(&key(KeyCode::M, true), true), None);
        assert!(!hub.toggle_requested);
        // Back in the world, M is the map key, not Ctrl + M.
        assert_eq!(hub.key(&key(KeyCode::M, true), false), Some(P5Key::Map));
        assert!(hub.toggle_requested);
        // Ctrl + M is not the map key (the walker's `toggle_mount` action takes it).
        let mut plain = MapHub::new(false);
        assert_eq!(plain.key(&key(KeyCode::ControlRight, true), false), None);
        assert_eq!(plain.key(&key(KeyCode::M, true), false), Some(P5Key::Mount));
        assert!(!plain.toggle_requested);
    }

    #[test]
    fn lookups_without_ui_are_answered_or_refused() {
        let mut hub = MapHub::new(false).with_ui(false);
        assert!(hub.request_lookup("Bree".into()));
        assert!(hub.lookups_without_ui());
        // The facts are still loading: the line waits.
        assert!(hub.serve_lookups_without_ui(true, None).is_empty());
        assert!(hub.lookups_without_ui());
        // No facts will come: one error line.
        let lines = hub.serve_lookups_without_ui(false, None);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].kind, crate::commands::LineKind::Error);
        assert!(!hub.lookups_without_ui());
        for _ in 0..MAX_LOOKUPS {
            assert!(hub.request_lookup("x".into()));
        }
        assert!(!hub.request_lookup("y".into()));
        // With the game UI the window answers.
        let mut ui = MapHub::new(false).with_ui(true);
        assert!(ui.request_lookup("Bree".into()));
        assert!(!ui.lookups_without_ui());
        assert!(ui.serve_lookups_without_ui(false, None).is_empty());
    }
}
