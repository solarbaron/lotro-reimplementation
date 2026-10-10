// SPDX-License-Identifier: GPL-3.0-or-later
//! The screens before world entry (decision 0024 P1): character selection and character
//! creation, driven through the session's character commands and events.
//!
//! Spec: `docs/specs/character-screens.md`; the data of `player-character.md` and
//! `appearance.md`; the looks of decision 0025 (`tessera-protocol.md` v1.1 §7.4,
//! `scenario-feed.md` revision 4).
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`model`] | the state machine: stages, selection list, choices, names, Create, Delete, Enter, time-outs | §2.3, §3–§7 |
//! | [`backend`] | the per-backend rules of the New and Delete buttons, where looks are kept | §7.1, §7.2 |
//! | [`controls`] | appearance controls, values, looks, the receiver rule, the figure's spec | §5.2; `appearance.md` §3.4; protocol §7.4 |
//! | [`data`] | the install's lists, names and control details, loaded once | §4, §5 |
//! | [`turntable`] | turn and zoom | §2.3 |
//! | [`viewport`] | the viewport's scene, camera and figure; UI-scene camera rule | §2.1; `misc-records.md` §4 |
//! | [`ui`] | the chargen layouts in the game UI: element states from the model, clicks to actions | §2–§5 |
//! | [`texts`] | Tessera's own texts and the error lines | §2.3, §7 |
//!
//! # Entry point
//!
//! [`CharacterScreens::open`] takes the [`Session`] (in any state; `Connect` is sent when
//! it is idle) and the install's [`GameDataStore`]. The client's start-up opens them with
//! `--character-select` (`crate::pre_world`); tests and the headless driver use them
//! directly. The viewport's figure is the walker's (`walker::Walker::set_preview`, drawn
//! alone into the viewport's external content by `walker::Walker::draw_preview`):
//! [`viewport::figure_job`] gives it with every creation control applied and the stage's
//! `CharGen_Stage` key, from which it takes its face (`appearance.md` §3.5). Each frame
//! the caller passes the session to [`CharacterScreens::pump`], which polls it, updates the screens, sends their
//! commands and hands every event back to the caller; the player's input arrives as
//! [`Action`]s ([`CharacterScreens::act`], or through [`ui::ScreenUi`]). When
//! [`CharacterScreens::status`] is [`Status::Entered`] the world has been entered and the
//! caller continues with the world (the events of that frame, `WorldEntered` included, were
//! handed back); [`Status::Quit`] and [`Status::Closed`] end the client.
//!
//! # Creation choices in the settings
//!
//! Where the session cannot carry a character's look (a test server at protocol 1.0, the
//! plain sandbox across restarts) the creation choices are kept in Tessera's settings
//! ([`tessera_settings::CreatedCharacters`], keyed by server and name) and applied to the
//! client's own figure of that character only (§7.1 Tessera rule): its body, class and
//! look, where the session did not report them. The settings file is written on a worker
//! thread (a locked read-modify-write that merges this run's new choices into the file's
//! section), never on the frame; [`CharacterScreens::wait_saved`] waits for it.

pub mod backend;
pub mod controls;
pub mod data;
pub mod model;
pub mod texts;
pub mod turntable;
pub mod ui;
pub mod viewport;

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_gamedata::GameDataStore;
use tessera_net::{Body, Event, Look, LookEntry, PropertyValue, Session, SessionState};
use tessera_props::RecordSource;
use tessera_settings::{
    CreatedCharacters, CreationChoices, GraphicsPrefs, SettingsStore, StoredLookEntry, StoredValue,
};
use tessera_text::Strings;

pub use backend::{BackendKind, ButtonRule};
pub use controls::{AppearanceChoices, ControlValue};
pub use data::{ScreenData, ScreenDataError};
pub use model::{Action, CharacterRow, CreatedCharacter, ScreenModel, Stage, Status};
pub use turntable::Turn;
pub use viewport::{FigureChoice, ViewportState, viewport_state};

/// Options of [`CharacterScreens`].
#[derive(Debug, Clone, Default)]
pub struct ScreenOptions {
    /// The key under which creation choices are kept: the client's start-up passes a test
    /// server's connect string (and account) here. `None` derives one from the backend
    /// ([`BackendKind::settings_key`]); a test server has none then, so its choices are
    /// neither kept nor applied.
    pub server_key: Option<String>,
    /// Where the settings live; `None` keeps the choices in memory only (tests).
    pub settings: Option<SettingsStore>,
}

/// The selection and creation screens over a session (module docs).
#[derive(Debug)]
pub struct CharacterScreens {
    model: ScreenModel,
    options: ScreenOptions,
    created: CreatedCharacters,
    avatar_update_visible: bool,
    scratch: Vec<Event>,
    saver: Saver,
}

/// Settings writes of the screens: at most one on a worker thread, the rest queued and
/// coalesced until it ends.
#[derive(Debug, Default)]
struct Saver {
    /// The write in flight; it returns the file's merged section when it wrote one.
    running: Option<JoinHandle<Option<CreatedCharacters>>>,
    /// New creation choices not yet handed to a write.
    created: Vec<CreationChoices>,
    /// A new `GOP_AvatarUpdateVisible` value not yet handed to a write.
    graphics: Option<bool>,
}

impl Saver {
    fn is_idle(&self) -> bool {
        self.running.is_none() && self.created.is_empty() && self.graphics.is_none()
    }
}

/// One locked read-modify-write of the settings file: the new choices are merged into the
/// section as the file holds it now (another process may have added to it), and the
/// graphics option is set. Returns the merged section when one was written.
fn save_settings(
    store: &SettingsStore,
    created: &[CreationChoices],
    graphics: Option<bool>,
) -> Option<CreatedCharacters> {
    let mut merged = None;
    let result = store.update(|s| {
        if !created.is_empty() {
            let (mut current, warn) = CreatedCharacters::from_settings(s);
            for w in warn {
                tracing::debug!("settings: {w}");
            }
            for c in created {
                current.remember(c.clone());
            }
            current.store(s);
            merged = Some(current);
        }
        if let Some(on) = graphics {
            let (mut g, _) = GraphicsPrefs::from_settings(s);
            g.avatar_update_visible = on;
            g.store(s);
        }
    });
    match result {
        Ok(_) => merged,
        Err(e) => {
            tracing::warn!("character-screen settings not saved: {e}");
            None
        }
    }
}

/// A stored look entry as a session look entry.
#[must_use]
pub fn look_entry_from_stored(e: &StoredLookEntry) -> Option<LookEntry> {
    let value = match &e.value {
        StoredValue::Bool(b) => PropertyValue::Bool(*b),
        StoredValue::Int(v) => PropertyValue::Int(*v),
        StoredValue::Int64(v) => PropertyValue::Int64(*v),
        StoredValue::Float(v) => PropertyValue::Float(*v),
        StoredValue::Enum(v) => PropertyValue::Enum(*v),
        StoredValue::Bitfield32(v) => PropertyValue::Bitfield32(*v),
        StoredValue::Bitfield64(v) => PropertyValue::Bitfield64(*v),
        StoredValue::Did(v) => PropertyValue::Did(tessera_net::DataId(*v)),
        StoredValue::String(s) => PropertyValue::String(s.clone()),
        StoredValue::Colour(c) => PropertyValue::Colour(*c),
    };
    value.is_look_value().then_some(LookEntry {
        property: e.property,
        value,
    })
}

/// A session look entry as a stored one (`None` for kinds a look cannot carry).
#[must_use]
pub fn stored_from_look_entry(e: &LookEntry) -> Option<StoredLookEntry> {
    let value = match &e.value {
        PropertyValue::Bool(b) => StoredValue::Bool(*b),
        PropertyValue::Int(v) => StoredValue::Int(*v),
        PropertyValue::Int64(v) => StoredValue::Int64(*v),
        PropertyValue::Float(v) => StoredValue::Float(*v),
        PropertyValue::Enum(v) => StoredValue::Enum(*v),
        PropertyValue::Bitfield32(v) => StoredValue::Bitfield32(*v),
        PropertyValue::Bitfield64(v) => StoredValue::Bitfield64(*v),
        PropertyValue::Did(d) => StoredValue::Did(d.0),
        PropertyValue::String(s) => StoredValue::String(s.clone()),
        PropertyValue::Colour(c) => StoredValue::Colour(*c),
        _ => return None,
    };
    Some(StoredLookEntry {
        property: e.property,
        value,
    })
}

impl CharacterScreens {
    /// The screens over `data` for `session`.
    #[must_use]
    pub fn new(session: &Session, data: Arc<ScreenData>, options: ScreenOptions) -> Self {
        let mut created = CreatedCharacters::default();
        let mut avatar_update_visible = true;
        if let Some(store) = &options.settings {
            let loaded = store.load();
            let (c, warn) = CreatedCharacters::from_settings(&loaded.settings);
            for w in warn {
                tracing::warn!("settings: {w}");
            }
            created = c;
            let (g, warn) = GraphicsPrefs::from_settings(&loaded.settings);
            for w in warn {
                tracing::warn!("settings: {w}");
            }
            avatar_update_visible = g.avatar_update_visible;
        }
        let mut model = ScreenModel::new(data, BackendKind::of(session));
        model.set_avatar_update_visible(avatar_update_visible);
        Self {
            model,
            options,
            created,
            avatar_update_visible,
            scratch: Vec::new(),
            saver: Saver::default(),
        }
    }

    /// Loads the screens' data from `store` (with `strings` for random names) and opens the
    /// screens for `session` (the entry point meant for the client's start-up, which does
    /// not call it yet; module docs).
    ///
    /// # Errors
    ///
    /// When the creation lists cannot be read ([`ScreenDataError`]).
    pub fn open<S: RecordSource>(
        session: &Session,
        store: &GameDataStore<S>,
        strings: Option<&Strings>,
        options: ScreenOptions,
    ) -> Result<Self, ScreenDataError> {
        let data = Arc::new(ScreenData::load(store, strings)?);
        Ok(Self::new(session, data, options))
    }

    /// The model (for the UI and the viewport).
    #[must_use]
    pub fn model(&self) -> &ScreenModel {
        &self.model
    }

    /// The model, mutable (tests; the palette sampler of randomise).
    pub fn model_mut(&mut self) -> &mut ScreenModel {
        &mut self.model
    }

    /// Where the screens are.
    #[must_use]
    pub fn status(&self) -> &Status {
        self.model.status()
    }

    /// The viewport's scene, camera and figure.
    #[must_use]
    pub fn viewport(&self) -> ViewportState {
        viewport_state(&self.model)
    }

    /// The settings key of this session (`None`: choices are neither kept nor applied,
    /// [`BackendKind::settings_key`]).
    #[must_use]
    pub fn settings_key(&self) -> Option<String> {
        self.model
            .backend()
            .settings_key(self.options.server_key.as_deref())
    }

    /// The creation choices kept in the settings.
    #[must_use]
    pub fn kept(&self) -> &CreatedCharacters {
        &self.created
    }

    /// Takes one action of the player.
    pub fn act(&mut self, action: Action, now: Instant) {
        self.model.apply(action, now);
    }

    /// One frame: sends `Connect` when the session is idle, polls the session, feeds its
    /// events to the screens (they are also appended to `out` for the caller, in order),
    /// sends the screens' commands, advances the time-outs and keeps new creation choices
    /// in the settings.
    pub fn pump(&mut self, session: &mut Session, now: Instant, out: &mut Vec<Event>) {
        if session.state() == SessionState::Idle
            && let Err(e) = session.send(now, tessera_net::Command::Connect)
        {
            tracing::warn!("connect refused: {e}");
        }
        self.scratch.clear();
        session.poll_into(now, &mut self.scratch);
        self.model.set_backend(BackendKind::of(session));
        let events = std::mem::take(&mut self.scratch);
        self.handle_events(&events, now);
        out.extend_from_slice(&events);
        self.scratch = events;
        self.flush(session, now);
    }

    /// Feeds events the caller polled itself (instead of [`CharacterScreens::pump`]).
    pub fn handle_events(&mut self, events: &[Event], now: Instant) {
        let mut rows_changed = false;
        for e in events {
            self.model.handle_event(e, now);
            rows_changed |= matches!(e, Event::Connected { .. } | Event::Character(_));
        }
        if rows_changed {
            self.apply_kept_choices();
        }
    }

    /// Sends the queued commands (a refusal goes back to the model), advances clocks, and
    /// keeps new creation choices and the avatar-update option in the settings (written on
    /// a worker thread; nothing here waits for the file).
    pub fn flush(&mut self, session: &mut Session, now: Instant) {
        for cmd in self.model.take_commands() {
            if let Err(e) = session.send(now, cmd.clone()) {
                tracing::info!(command = cmd.name(), "refused: {e}");
                self.model.send_failed(&cmd, &e);
            }
        }
        self.model.tick(now);
        self.keep_created();
        self.keep_graphics();
        self.pump_saver(false);
    }

    /// Waits until every settings write of the screens has reached the file (tests, and
    /// before the screens are dropped).
    pub fn wait_saved(&mut self) {
        while !self.saver.is_idle() {
            self.pump_saver(true);
        }
    }

    /// Collects a finished write (or waits for it with `block`) and starts the next one
    /// with what was queued meanwhile.
    fn pump_saver(&mut self, block: bool) {
        if let Some(h) = &self.saver.running {
            if !block && !h.is_finished() {
                return;
            }
            let h = self.saver.running.take().expect("checked");
            match h.join() {
                Ok(Some(merged)) => {
                    // The file's section with this run's choices; choices queued since
                    // stay remembered on top of it.
                    self.created = merged;
                    for c in &self.saver.created {
                        self.created.remember(c.clone());
                    }
                }
                Ok(None) => {}
                Err(_) => tracing::warn!("the settings writer panicked"),
            }
        }
        if self.saver.created.is_empty() && self.saver.graphics.is_none() {
            return;
        }
        let created = std::mem::take(&mut self.saver.created);
        let graphics = self.saver.graphics.take();
        let Some(store) = self.options.settings.clone() else {
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("tessera-chargen-settings".into())
            .spawn({
                let created = created.clone();
                move || save_settings(&store, &created, graphics)
            });
        match spawned {
            Ok(h) => self.saver.running = Some(h),
            Err(e) => {
                tracing::debug!("no settings thread ({e}); writing on this thread");
                if let Some(store) = &self.options.settings
                    && let Some(merged) = save_settings(store, &created, graphics)
                {
                    self.created = merged;
                }
            }
        }
    }

    /// Gives rows the creation choices kept in the settings (§7.1 Tessera rule) where the
    /// session stores no look: its body, class and look, each only where the session did
    /// not report it (the plain sandbox lists names only). A look kept for another body
    /// than the one the session reports is not applied.
    fn apply_kept_choices(&mut self) {
        if self.model.backend().carries_look() {
            return;
        }
        let Some(key) = self.settings_key() else {
            return;
        };
        let kept: Vec<(String, Body, Option<u32>, Option<Look>)> = self
            .model
            .rows()
            .iter()
            .filter(|r| r.look.is_none() || r.body.is_none() || r.class.is_none())
            .filter_map(|r| {
                let c = self.created.get(&key, &r.name)?;
                let body = Body::Character {
                    species: c.species,
                    sex: c.sex,
                };
                let same_body = r.body.is_none_or(|b| b == body);
                let entries = c.look.iter().filter_map(look_entry_from_stored).collect();
                let look = Look::canonical(entries).0.filter(|_| same_body);
                Some((r.name.clone(), body, c.class.filter(|_| same_body), look))
            })
            .collect();
        for (name, body, class, look) in kept {
            self.model.set_kept_choices(&name, Some(body), class, look);
        }
    }

    fn keep_created(&mut self) {
        let new = self.model.take_created();
        if new.is_empty() {
            return;
        }
        let Some(key) = self.settings_key() else {
            tracing::debug!("no settings key for this server; creation choices not kept");
            return;
        };
        for c in new {
            let choices = CreationChoices {
                server: key.clone(),
                name: c.name,
                species: c.species,
                sex: c.sex,
                class: Some(c.class),
                nationality: Some(c.nationality),
                look: c
                    .look
                    .iter()
                    .flat_map(|l| l.entries.iter())
                    .filter_map(stored_from_look_entry)
                    .collect(),
            };
            if self.created.remember(choices.clone()) {
                // Only the new choices are written: they are merged into the section the
                // file holds when the write runs (another client may have added to it).
                self.saver.created.push(choices);
            } else {
                tracing::warn!("creation choices not kept (key not valid)");
            }
        }
    }

    fn keep_graphics(&mut self) {
        let on = self.model.avatar_update_visible();
        if on == self.avatar_update_visible {
            return;
        }
        self.avatar_update_visible = on;
        // Toggles coalesce: the last value queued is the one written.
        self.saver.graphics = Some(on);
    }
}

impl Drop for CharacterScreens {
    fn drop(&mut self) {
        self.wait_saved();
    }
}
