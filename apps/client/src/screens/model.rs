// SPDX-License-Identifier: GPL-3.0-or-later
//! The screen model: the selection and creation screens as one state machine
//! (`character-screens.md` §2.3, §3–§7), independent of the UI tree and the renderer.
//!
//! The model takes the session's events ([`ScreenModel::handle_event`]), the player's
//! actions ([`ScreenModel::apply`]) and the clock ([`ScreenModel::tick`]), and queues the
//! commands the screens send ([`ScreenModel::take_commands`]). A command the session
//! refuses is reported back with [`ScreenModel::send_failed`]. The UI shows what the
//! getters say; tests and the headless driver use the same model.
//!
//! **Flow** (§6, Tessera rule): Select → (New) Race and class → (Continue) Appearance and
//! name → (Create) back to Select with the new row selected → (Enter) world entry. Back
//! goes one stage back; the appearance choices survive going back while race and gender
//! stay the same.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_gamedata::chargen::{ChoiceVerdict, CreationChoice};
use tessera_net::domain::Text;
use tessera_net::model::is_valid_name;
use tessera_net::{
    Appearance, Body, CharacterCommand, CharacterEvent, Command, Event, Look, NetError,
};

use super::backend::{BackendKind, ButtonRule};
use super::controls::{AppearanceChoices, ControlValue, PaletteSampler};
use super::data::ScreenData;
use super::texts::{ErrorLine, OwnText};
use super::turntable::{Turn, Turntable};

/// How long the screen waits for the answer to `CreateCharacter` (§7.1).
pub const CREATE_TIMEOUT: Duration = Duration::from_secs(10);
/// Most lines kept in `CharGen_ErrorTextList` (the oldest go first).
pub const MAX_ERROR_LINES: usize = 8;
/// Most rows of the selection list (hostile-input bound; the session lists at most 64).
pub const MAX_ROWS: usize = 1024;
/// Tessera's character limit shown in the slots label (§3; `scenario-feed.md` §5.1).
pub const SLOT_LIMIT: usize = 64;
/// Longest text the name field keeps, in bytes.
pub const MAX_NAME_INPUT: usize = 256;

/// The screen shown (`character-screens.md` §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Character selection (`CharacterSelect`).
    Select,
    /// Race and class (`RaceClass`).
    RaceClass,
    /// Appearance and name (`Appearance`).
    Appearance,
}

/// One row of the selection list (§3, §8).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterRow {
    /// The name.
    pub name: String,
    /// The body, when the session told it.
    pub body: Option<Body>,
    /// The class key, when told.
    pub class: Option<u32>,
    /// The level, when told.
    pub level: Option<u32>,
    /// The look, when told (or kept in the settings, §7.1).
    pub look: Option<Look>,
}

impl CharacterRow {
    fn named(name: String) -> Self {
        Self {
            name,
            body: None,
            class: None,
            level: None,
            look: None,
        }
    }

    /// The (species, sex) of a character body.
    #[must_use]
    pub fn character_body(&self) -> Option<(u32, u32)> {
        match self.body {
            Some(Body::Character { species, sex }) => Some((species, sex)),
            _ => None,
        }
    }
}

/// The choices of the creation screens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Creation {
    /// The race (species key).
    pub species: Option<u32>,
    /// The gender (sex value).
    pub sex: Option<u32>,
    /// The class key.
    pub class: Option<u32>,
    /// The nationality key.
    pub nationality: Option<u32>,
    /// The appearance choices.
    pub appearance: AppearanceChoices,
    /// The (species, sex) the appearance choices belong to.
    pub appearance_for: Option<(u32, u32)>,
    /// The name field.
    pub name: String,
}

/// A character this screen created, for the settings (§7.1 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedCharacter {
    /// The name.
    pub name: String,
    /// The species.
    pub species: u32,
    /// The sex.
    pub sex: u32,
    /// The class.
    pub class: u32,
    /// The nationality.
    pub nationality: u32,
    /// The look sent (none when no control was set).
    pub look: Option<Look>,
}

/// Where the screens are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Shown.
    Open,
    /// `EnterWorld` sent for this name.
    Entering(String),
    /// The world was entered with this name (`WorldEntered`): the screens close.
    Entered(String),
    /// The player quit from the selection screen.
    Quit,
    /// The session ended (the reason).
    Closed(String),
}

/// Something the player did.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Select row *i* of the selection list (a click).
    Select(usize),
    /// Enter the world with the selected character (Enter, or a double click).
    Enter,
    /// New character (→ race and class).
    New,
    /// Delete the selected character: opens the confirmation (§7.2); a second Delete on
    /// the same row confirms it.
    Delete,
    /// Answer the confirmation dialog.
    ConfirmDelete(bool),
    /// Quit (selection screen).
    Quit,
    /// One stage back.
    Back,
    /// Race and class → appearance and name.
    Continue,
    /// Choose a race (species key).
    Race(u32),
    /// Choose a gender (sex value).
    Gender(u32),
    /// Choose a class.
    Class(u32),
    /// Choose a nationality.
    Nationality(u32),
    /// Set appearance control *index* (stored order).
    Control(usize, ControlValue),
    /// Randomise every control with this seed (§5.2 OPEN, Tessera rule).
    Randomise(u64),
    /// The name field's text.
    Name(String),
    /// A random name with this seed (`player-character.md` §8.5).
    RandomName(u64),
    /// Create the character (§7.1).
    Create,
    /// A turn button went down.
    TurnPress(Turn),
    /// A turn button went up.
    TurnRelease(Turn),
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
    /// The `GOP_AvatarUpdateVisible` check box (§2.3).
    AvatarUpdateVisible(bool),
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    name: String,
    since: Instant,
    created: CreatedCharacter,
}

/// The screens' state machine (module docs).
#[derive(Debug, Clone)]
pub struct ScreenModel {
    data: Arc<ScreenData>,
    backend: BackendKind,
    stage: Stage,
    rows: Vec<CharacterRow>,
    selected: Option<usize>,
    last_entered: Option<String>,
    creation: Creation,
    pending: Option<Pending>,
    /// The last creation that timed out (§7.1): a late `CharacterInfo` for it still keeps
    /// its choices.
    timed_out: Option<Pending>,
    confirm: Option<String>,
    delete_refused: bool,
    errors: VecDeque<ErrorLine>,
    outbox: Vec<Command>,
    turntable: Turntable,
    avatar_update_visible: bool,
    status: Status,
    created: Vec<CreatedCharacter>,
    figure_loading: bool,
}

impl ScreenModel {
    /// A model over `data` for a session on `backend`, at the selection screen with an
    /// empty list (the list comes with `Connected`).
    #[must_use]
    pub fn new(data: Arc<ScreenData>, backend: BackendKind) -> Self {
        Self {
            data,
            backend,
            stage: Stage::Select,
            rows: Vec::new(),
            selected: None,
            last_entered: None,
            creation: Creation::default(),
            pending: None,
            timed_out: None,
            confirm: None,
            delete_refused: false,
            errors: VecDeque::new(),
            outbox: Vec::new(),
            turntable: Turntable::default(),
            avatar_update_visible: true,
            status: Status::Open,
            created: Vec::new(),
            figure_loading: false,
        }
    }

    // ------------------------------------------------------------------------------------
    // Getters
    // ------------------------------------------------------------------------------------

    /// The data.
    #[must_use]
    pub fn data(&self) -> &Arc<ScreenData> {
        &self.data
    }

    /// The backend the rules follow.
    #[must_use]
    pub fn backend(&self) -> &BackendKind {
        &self.backend
    }

    /// The stage shown.
    #[must_use]
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// The selection list.
    #[must_use]
    pub fn rows(&self) -> &[CharacterRow] {
        &self.rows
    }

    /// The selected row.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The selected row's character.
    #[must_use]
    pub fn selected_row(&self) -> Option<&CharacterRow> {
        self.rows.get(self.selected?)
    }

    /// The creation choices.
    #[must_use]
    pub fn creation(&self) -> &Creation {
        &self.creation
    }

    /// The lines of `CharGen_ErrorTextList`, oldest first.
    pub fn errors(&self) -> impl Iterator<Item = &ErrorLine> {
        self.errors.iter()
    }

    /// The name of the open delete confirmation.
    #[must_use]
    pub fn confirming(&self) -> Option<&str> {
        self.confirm.as_deref()
    }

    /// The turntable.
    #[must_use]
    pub fn turntable(&self) -> &Turntable {
        &self.turntable
    }

    /// The `GOP_AvatarUpdateVisible` option.
    #[must_use]
    pub fn avatar_update_visible(&self) -> bool {
        self.avatar_update_visible
    }

    /// Where the screens are.
    #[must_use]
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// The name of the creation awaiting its answer.
    #[must_use]
    pub fn pending_creation(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.name.as_str())
    }

    /// The slots label (§3): used / allowed.
    #[must_use]
    pub fn slots(&self) -> (usize, usize) {
        (self.rows.len(), SLOT_LIMIT)
    }

    /// Whether the current figure is still loading (§4.3 loading notice and race blocker).
    #[must_use]
    pub fn figure_loading(&self) -> bool {
        self.figure_loading
    }

    /// The New button (§3, §7.1).
    #[must_use]
    pub fn new_button(&self) -> ButtonRule {
        self.backend.new_button()
    }

    /// The Delete button (§7.2).
    #[must_use]
    pub fn delete_button(&self) -> ButtonRule {
        self.backend
            .delete_button(self.selected.is_some(), self.delete_refused)
    }

    /// The Enter button: enabled while a character is selected and nothing is entering.
    #[must_use]
    pub fn enter_button(&self) -> ButtonRule {
        if self.selected.is_some() && self.status == Status::Open {
            ButtonRule::Enabled
        } else {
            ButtonRule::Ghosted(None)
        }
    }

    /// The Back button (§2.3): hidden on the selection screen.
    #[must_use]
    pub fn back_button(&self) -> ButtonRule {
        match self.stage {
            Stage::Select => ButtonRule::Hidden,
            _ => ButtonRule::Enabled,
        }
    }

    /// The Continue button (§2.3): shown on race and class only, ghosted while the choice
    /// is not valid (a click still reaches the screen).
    #[must_use]
    pub fn continue_button(&self) -> ButtonRule {
        match self.stage {
            Stage::RaceClass if self.race_class_valid() => ButtonRule::Enabled,
            Stage::RaceClass => ButtonRule::Ghosted(None),
            _ => ButtonRule::Hidden,
        }
    }

    /// The Create button (§5.4, §7.1): on the appearance stage, ghosted while a creation
    /// awaits its answer.
    #[must_use]
    pub fn create_button(&self) -> ButtonRule {
        match self.stage {
            Stage::Appearance if self.pending.is_some() => ButtonRule::Ghosted(None),
            Stage::Appearance => ButtonRule::Enabled,
            _ => ButtonRule::Hidden,
        }
    }

    /// Whether the avatar-update toggle is shown (§2.3: race-and-class and appearance
    /// stages, while the chosen species is 0x49 or 0x51).
    #[must_use]
    pub fn shows_avatar_update_toggle(&self) -> bool {
        self.stage != Stage::Select
            && self
                .creation
                .species
                .is_some_and(tessera_gamedata::chargen::shows_avatar_update_toggle)
    }

    /// The characters created in this run that the session does not carry the look of
    /// (§7.1 Tessera rule), drained; the caller keeps them in the settings.
    pub fn take_created(&mut self) -> Vec<CreatedCharacter> {
        std::mem::take(&mut self.created)
    }

    /// The commands queued since the last call, in order.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.outbox)
    }

    // ------------------------------------------------------------------------------------
    // Inputs
    // ------------------------------------------------------------------------------------

    /// Updates the backend (its capabilities become known once greeted).
    pub fn set_backend(&mut self, backend: BackendKind) {
        self.backend = backend;
    }

    /// Sets the `GOP_AvatarUpdateVisible` option (from the settings) without an action.
    pub fn set_avatar_update_visible(&mut self, on: bool) {
        self.avatar_update_visible = on;
    }

    /// Sets the turn buttons' first repeat interval read from the data.
    pub fn set_turn_interval(&mut self, first: Duration) {
        self.turntable = Turntable::with_first_interval(first);
    }

    /// Reports whether the figure for the current choice is still loading.
    pub fn set_figure_loading(&mut self, loading: bool) {
        self.figure_loading = loading;
    }

    /// Gives a row without a look the look kept in the settings (§7.1 Tessera rule).
    pub fn set_kept_look(&mut self, name: &str, look: Look) {
        self.set_kept_choices(name, None, None, Some(look));
    }

    /// Gives row `name` the creation choices kept in the settings (§7.1 Tessera rule) where
    /// the session did not report them: the body (the plain sandbox lists names only), the
    /// class and the look. What the session reported is never replaced.
    pub fn set_kept_choices(
        &mut self,
        name: &str,
        body: Option<Body>,
        class: Option<u32>,
        look: Option<Look>,
    ) {
        let Some(r) = self.rows.iter_mut().find(|r| r.name == name) else {
            return;
        };
        if r.body.is_none() {
            r.body = body;
        }
        if r.class.is_none() {
            r.class = class;
        }
        if r.look.is_none() {
            r.look = look;
        }
    }

    fn error(&mut self, line: ErrorLine) {
        if self.errors.len() >= MAX_ERROR_LINES {
            self.errors.pop_front();
        }
        self.errors.push_back(line);
    }

    fn own_error(&mut self, t: OwnText) {
        self.error(ErrorLine::Own(t));
    }

    fn set_stage(&mut self, stage: Stage) {
        if self.stage != stage {
            self.stage = stage;
            self.turntable.reset_angle();
            self.errors.clear();
        }
    }

    fn row_index(&self, name: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.name == name)
    }

    /// Takes one session event (`_now`: the frame's time, for answers that start clocks).
    pub fn handle_event(&mut self, event: &Event, _now: Instant) {
        match event {
            Event::Connected { characters } => {
                self.rows = characters
                    .iter()
                    .take(MAX_ROWS)
                    .map(|c| CharacterRow::named(c.name.clone()))
                    .collect();
                self.selected = self
                    .last_entered
                    .as_deref()
                    .and_then(|n| self.row_index(n))
                    .or((!self.rows.is_empty()).then_some(0));
            }
            Event::Character(c) => self.character_event(c),
            Event::WorldEntered { name, .. } => {
                self.last_entered = Some(name.clone());
                self.status = Status::Entered(name.clone());
            }
            Event::Disconnected { reason } => {
                let why = format!("{reason:?}");
                self.own_error(OwnText::Disconnected(why.clone()));
                self.status = Status::Closed(why);
                self.pending = None;
                self.timed_out = None;
            }
            _ => {}
        }
    }

    fn character_event(&mut self, c: &CharacterEvent) {
        match c {
            CharacterEvent::CharacterInfo {
                name,
                body,
                class,
                level,
                look,
            } => {
                let i = match self.row_index(name) {
                    Some(i) => i,
                    None if self.rows.len() < MAX_ROWS => {
                        self.rows.push(CharacterRow::named(name.clone()));
                        self.rows.len() - 1
                    }
                    None => {
                        tracing::warn!("character list over {MAX_ROWS} rows; entry ignored");
                        return;
                    }
                };
                let row = &mut self.rows[i];
                if body.is_some() {
                    row.body = *body;
                }
                if class.is_some() {
                    row.class = *class;
                }
                if level.is_some() {
                    row.level = *level;
                }
                if let Some(l) = look {
                    row.look = Some((**l).clone());
                }
                if self.selected.is_none() {
                    self.selected = Some(i);
                }
                let answers_pending = self.pending.as_ref().is_some_and(|p| &p.name == name);
                if answers_pending && self.timed_out.as_ref().is_some_and(|p| &p.name == name) {
                    // A retry of a timed-out creation: the retry's answer counts.
                    self.timed_out = None;
                }
                let late =
                    !answers_pending && self.timed_out.as_ref().is_some_and(|p| &p.name == name);
                if late {
                    // The answer came after the time-out (§7.1): the screen stays where it
                    // is, but the row gets the choices and they are kept.
                    let p = self.timed_out.take().expect("checked");
                    if self.rows[i].look.is_none() {
                        self.rows[i].look.clone_from(&p.created.look);
                    }
                    if self.rows[i].body.is_none() {
                        self.rows[i].body = Some(Body::Character {
                            species: p.created.species,
                            sex: p.created.sex,
                        });
                    }
                    if !self.backend.carries_look() {
                        self.created.push(p.created);
                    }
                } else if answers_pending {
                    let p = self.pending.take().expect("checked");
                    if self.rows[i].look.is_none() {
                        self.rows[i].look.clone_from(&p.created.look);
                    }
                    if self.rows[i].body.is_none() {
                        self.rows[i].body = Some(Body::Character {
                            species: p.created.species,
                            sex: p.created.sex,
                        });
                    }
                    if !self.backend.carries_look() {
                        self.created.push(p.created);
                    }
                    self.selected = Some(i);
                    self.creation.name.clear();
                    self.set_stage(Stage::Select);
                }
            }
            CharacterEvent::CharacterRefused { name, reason } => {
                if self.pending.as_ref().is_some_and(|p| &p.name == name) {
                    self.pending = None;
                }
                if self.timed_out.as_ref().is_some_and(|p| &p.name == name) {
                    self.timed_out = None;
                }
                self.error(ErrorLine::Reason(reason.clone()));
            }
            CharacterEvent::CharacterRemoved { name } => {
                let Some(i) = self.row_index(name) else {
                    return;
                };
                let selected_name = self.selected_row().map(|r| r.name.clone());
                self.rows.remove(i);
                self.selected = selected_name
                    .and_then(|n| self.row_index(&n))
                    .or_else(|| (!self.rows.is_empty()).then(|| i.min(self.rows.len() - 1)));
            }
            _ => {}
        }
    }

    /// A command the session refused (`Session::send` returned `err`).
    pub fn send_failed(&mut self, command: &Command, err: &NetError) {
        let why = err.to_string();
        match command {
            Command::Character(CharacterCommand::CreateCharacter { name, .. }) => {
                if self.pending.as_ref().is_some_and(|p| &p.name == name) {
                    self.pending = None;
                }
                self.own_error(OwnText::CreateRefused(why));
            }
            Command::Character(CharacterCommand::DeleteCharacter { .. }) => {
                self.delete_refused = true;
                self.own_error(OwnText::DeleteRefused);
            }
            Command::EnterWorld { .. } => {
                self.status = Status::Open;
                self.own_error(OwnText::EnterRefused(why));
            }
            _ => tracing::debug!("screen command refused: {why}"),
        }
    }

    /// Advances clocks: the creation time-out (§7.1) and the turntable.
    pub fn tick(&mut self, now: Instant) {
        if let Some(p) = &self.pending
            && now.saturating_duration_since(p.since) >= CREATE_TIMEOUT
        {
            self.timed_out = self.pending.take();
            self.own_error(OwnText::CreateTimedOut);
        }
        self.turntable.update(now);
    }

    /// Takes one action of the player.
    pub fn apply(&mut self, action: Action, now: Instant) {
        if !matches!(self.status, Status::Open) {
            return;
        }
        match action {
            Action::Select(i) => {
                if i < self.rows.len() && self.stage == Stage::Select {
                    if self.selected != Some(i) {
                        self.confirm = None;
                    }
                    if self.selected != Some(i) {
                        self.turntable.reset_angle();
                    }
                    self.selected = Some(i);
                }
            }
            Action::Enter => self.enter(),
            Action::New => self.new_character(),
            Action::Delete => {
                if self.stage == Stage::Select && self.delete_button().is_enabled() {
                    let name = self.selected_row().map(|r| r.name.clone());
                    if name.is_some() && self.confirm == name {
                        // A second Delete on the same row answers the confirmation.
                        self.apply(Action::ConfirmDelete(true), now);
                    } else {
                        self.confirm = name;
                    }
                }
            }
            Action::ConfirmDelete(yes) => {
                if let Some(name) = self.confirm.take()
                    && yes
                    && self.delete_button().is_enabled()
                {
                    self.outbox
                        .push(Command::Character(CharacterCommand::DeleteCharacter {
                            name,
                        }));
                }
            }
            Action::Quit => {
                if self.stage == Stage::Select {
                    self.status = Status::Quit;
                }
            }
            Action::Back => match self.stage {
                Stage::Select => {}
                Stage::RaceClass => self.set_stage(Stage::Select),
                Stage::Appearance => self.set_stage(Stage::RaceClass),
            },
            Action::Continue => {
                if self.stage == Stage::RaceClass {
                    if self.race_class_valid() {
                        self.enter_appearance();
                    } else {
                        self.own_error(OwnText::ChoiceIncomplete);
                    }
                }
            }
            Action::Race(species) => self.choose_race(species),
            // Gender and class are chosen on the race-and-class stage only (§4), like the
            // race: a change on the appearance stage would send a body whose controls are
            // another avatar's.
            Action::Gender(sex) => {
                if self.stage == Stage::RaceClass
                    && let Some(s) = self.creation.species
                    && self
                        .data
                        .chargen
                        .race(s)
                        .is_some_and(|r| r.gender(sex).is_some())
                    && self.creation.sex != Some(sex)
                {
                    self.creation.sex = Some(sex);
                    self.turntable.reset_angle();
                }
            }
            Action::Class(class) => {
                if self.stage == Stage::RaceClass
                    && let Some(s) = self.creation.species
                    && self.data.chargen.class_order(s).contains(&class)
                {
                    self.creation.class = Some(class);
                }
            }
            Action::Nationality(n) => {
                if let Some(s) = self.creation.species
                    && self
                        .data
                        .chargen
                        .race(s)
                        .is_some_and(|r| r.record.nationalities.contains(&n))
                {
                    self.creation.nationality = Some(n);
                }
            }
            Action::Control(index, value) => {
                if let Some(d) = self.current_avatar() {
                    let d = d.clone();
                    self.creation.appearance.set(&d, index, value);
                }
            }
            Action::Randomise(seed) => self.randomise(seed, None),
            Action::Name(text) => {
                let mut t = text;
                if t.len() > MAX_NAME_INPUT {
                    let mut cut = MAX_NAME_INPUT;
                    while !t.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    t.truncate(cut);
                }
                self.creation.name = t.replace(['\n', '\r'], "");
            }
            Action::RandomName(seed) => self.random_name(seed),
            Action::Create => self.create(now),
            Action::TurnPress(t) => self.turntable.press(t, now),
            Action::TurnRelease(t) => self.turntable.release(t, now),
            Action::ZoomIn => self.turntable.zoom_in(),
            Action::ZoomOut => self.turntable.zoom_out(),
            Action::AvatarUpdateVisible(on) => self.avatar_update_visible = on,
        }
    }

    /// Randomises the appearance with an optional palette sampler (colours change only with
    /// one).
    pub fn randomise(&mut self, seed: u64, sampler: Option<&dyn PaletteSampler>) {
        if self.stage != Stage::Appearance {
            return;
        }
        let Some(d) = self.current_avatar().cloned() else {
            return;
        };
        let nationality = self.creation.nationality.unwrap_or(0);
        let data = Arc::clone(&self.data);
        self.creation.appearance.randomise(
            &d,
            seed,
            &|c| data.chargen.palette_for(&c.control, nationality),
            sampler,
        );
    }

    fn enter(&mut self) {
        if self.stage != Stage::Select {
            return;
        }
        let Some(name) = self.selected_row().map(|r| r.name.clone()) else {
            return;
        };
        self.last_entered = Some(name.clone());
        self.status = Status::Entering(name.clone());
        self.outbox.push(Command::EnterWorld { character: name });
    }

    fn new_character(&mut self) {
        if self.stage != Stage::Select || !self.new_button().is_enabled() {
            return;
        }
        let selected = self.selected_row().and_then(CharacterRow::character_body);
        let c = &self.data.chargen;
        let Some((species, sex)) = c.default_race_and_gender(selected) else {
            return;
        };
        let class = c.class_after_race_change(None, species);
        // A new creation starts from the defaults of §6 (the avatar's own values, the first
        // nationality, an empty name); the keep rule of step 3 holds only for Back.
        self.creation = Creation {
            species: Some(species),
            sex: Some(sex),
            class,
            ..Creation::default()
        };
        self.set_stage(Stage::RaceClass);
    }

    fn choose_race(&mut self, species: u32) {
        if self.stage != Stage::RaceClass || self.creation.species == Some(species) {
            return;
        }
        let c = &self.data.chargen;
        if c.race(species).is_none() {
            return;
        }
        let sex = c.gender_after_race_change(self.creation.sex, species);
        let class = c.class_after_race_change(self.creation.class, species);
        self.creation.species = Some(species);
        self.creation.sex = sex;
        self.creation.class = class;
        self.turntable.reset_angle();
    }

    fn race_class_valid(&self) -> bool {
        let (Some(species), Some(sex), Some(class)) = (
            self.creation.species,
            self.creation.sex,
            self.creation.class,
        ) else {
            return false;
        };
        let c = &self.data.chargen;
        c.race_with_gender(species, sex).is_some() && c.class_order(species).contains(&class)
    }

    fn enter_appearance(&mut self) {
        let (Some(species), Some(sex)) = (self.creation.species, self.creation.sex) else {
            return;
        };
        let keep = self.creation.appearance_for == Some((species, sex));
        if !keep {
            self.creation.appearance = AppearanceChoices::default();
            self.creation.appearance_for = Some((species, sex));
            self.creation.nationality = self.data.chargen.default_nationality(species);
            self.creation.name.clear();
        }
        self.set_stage(Stage::Appearance);
    }

    /// The control details of the chosen (race, gender).
    #[must_use]
    pub fn current_avatar(&self) -> Option<&super::controls::AvatarDetail> {
        self.data.avatar(self.creation.species?, self.creation.sex?)
    }

    fn random_name(&mut self, seed: u64) {
        let (Some(species), Some(sex)) = (self.creation.species, self.creation.sex) else {
            return;
        };
        let Some(names) = &self.data.names else {
            self.own_error(OwnText::NoRandomName);
            return;
        };
        let g = names.random_name(species, sex, seed);
        if g.name.is_empty() {
            self.own_error(OwnText::NoRandomName);
            return;
        }
        self.creation.name = g.name;
        if !g.accepted {
            self.own_error(OwnText::RandomNameUnchecked);
        }
    }

    /// The check of the name before sending (§5.3): the session's name rule, then the
    /// install's name check when there is one.
    ///
    /// # Errors
    ///
    /// Why the name cannot be used.
    pub fn check_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("the name is empty".into());
        }
        if !is_valid_name(name) {
            return Err("it is not a valid name for the session".into());
        }
        if let Some(n) = &self.data.names {
            n.check(name).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn create(&mut self, now: Instant) {
        if self.stage != Stage::Appearance || self.pending.is_some() {
            return;
        }
        let c = &self.creation;
        let (Some(species), Some(sex), Some(class), Some(nationality)) =
            (c.species, c.sex, c.class, c.nationality)
        else {
            self.own_error(OwnText::ChoiceInvalid);
            return;
        };
        let verdict = self.data.chargen.check(&CreationChoice {
            species,
            sex,
            nationality,
            class,
        });
        if verdict != ChoiceVerdict::Valid {
            self.own_error(OwnText::ChoiceInvalid);
            return;
        }
        let name = c.name.clone();
        if let Err(why) = self.check_name(&name) {
            self.own_error(OwnText::NameInvalid(why));
            return;
        }
        let look = self
            .current_avatar()
            .and_then(|d| self.creation.appearance.look(d));
        let appearance = Appearance::new(Body::Character { species, sex }).with_look(look.clone());
        self.outbox
            .push(Command::Character(CharacterCommand::CreateCharacter {
                name: name.clone(),
                appearance: Some(Box::new(appearance)),
                class: Some(class),
                nationality: Some(nationality),
            }));
        self.pending = Some(Pending {
            name: name.clone(),
            since: now,
            created: CreatedCharacter {
                name,
                species,
                sex,
                class,
                nationality,
                look,
            },
        });
    }

    /// The reason text of a refusal line for tests and logs (literal reasons only).
    #[must_use]
    pub fn error_texts(&self) -> Vec<String> {
        self.errors
            .iter()
            .map(|e| {
                e.text(&|t| match t {
                    Text::Literal(s) => Some(s.clone()),
                    _ => None,
                })
            })
            .collect()
    }
}
