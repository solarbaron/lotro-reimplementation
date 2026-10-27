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
//!
//! **Names** are compared in Unicode Normalization Form C (§7.1, CS16): two names are the
//! same when their NFC forms are equal code point for code point ([`same_name`]), since
//! Tessera's test server stores and lists the NFC form of a created name.

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

/// Whether two character names are the same (`character-screens.md` §7.1): their Unicode
/// NFC forms are equal, code point for code point.
#[must_use]
pub fn same_name(a: &str, b: &str) -> bool {
    use unicode_normalization::UnicodeNormalization as _;
    a == b || a.nfc().eq(b.nfc())
}

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
    /// Locked controls (stored indices; §5.2.2 rule 4: they start locked when their struct
    /// says so; Tessera rule: randomise skips them).
    pub locked: std::collections::BTreeSet<usize>,
    /// Locked groups (`AppearanceUI_Group` keys; §5.2.2 group lock, Tessera rule:
    /// randomise skips every control of a locked group).
    pub locked_groups: std::collections::BTreeSet<u32>,
    /// Collapsed groups (`AppearanceUI_Group` keys; §5.2.2 Tessera rule: a collapsed
    /// group hides its controls; groups start expanded).
    pub collapsed_groups: std::collections::BTreeSet<u32>,
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
    /// One hot click of a turn button (`Button_hot_click`, §2.3).
    TurnClick(Turn),
    /// A drag across the viewport starts at this pixel column.
    DragStart(f32),
    /// The drag is at this pixel column.
    DragTo(f32),
    /// The drag ends.
    DragEnd,
    /// Wheel notches over the viewport (positive: away from the player, zoom in).
    Wheel(f32),
    /// Zoom in (the zoom-in button).
    ZoomIn,
    /// Zoom out (the zoom-out button).
    ZoomOut,
    /// The pointer moved onto (true) or off the selection screen's figure (§2.1.4
    /// `CharGen_Action` `Hover`).
    FigureHover(bool),
    /// A control's lock button (§5.2.2 rule 4).
    ToggleLock(usize),
    /// A group's lock button (§5.2.2 group template; group key).
    ToggleGroupLock(u32),
    /// A group's expand (false) or collapse (true) button (§5.2.2 group template).
    CollapseGroup(u32, bool),
    /// The `GOP_AvatarUpdateVisible` check box (§2.3).
    AvatarUpdateVisible(bool),
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    name: String,
    since: Instant,
    created: CreatedCharacter,
}

/// What identifies the stage camera in view (§2.1.3): the stage, the selected row on the
/// selection screen, the body.
type CameraKey = (Stage, Option<usize>, Option<(u32, u32)>);

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
    hover: bool,
    /// The selection figure is the clicked figure (§2.3: a `SelectLeft` on the viewport
    /// over it); it keeps its `CharGen_Action` while the pointer moves on or off it.
    clicked: bool,
    /// The pointer is on the selection figure now (the last [`Action::FigureHover`]).
    pointer_on_figure: bool,
    /// The stage camera the turntable was last reset for (§2.1.3: setting it resets).
    camera_for: Option<CameraKey>,
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
            hover: false,
            clicked: false,
            pointer_on_figure: false,
            camera_for: None,
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

    /// Whether the pointer is on the selection screen's figure (§2.1.4).
    #[must_use]
    pub fn hovering_figure(&self) -> bool {
        self.hover
    }

    /// The (species, sex) of the figure in view: the selected character's body on the
    /// selection screen, the chosen race and gender on the creation stages.
    #[must_use]
    pub fn body_in_view(&self) -> Option<(u32, u32)> {
        match self.stage {
            Stage::Select => self.selected_row().and_then(CharacterRow::character_body),
            _ => self.creation.species.zip(self.creation.sex),
        }
    }

    /// The stage camera keys in view (§2.1.3): the race table's
    /// `RaceTable_Character_Select_Camera` on the selection screen, `RaceTable_Chargen_Camera`
    /// on the creation stages, and the gender entry's zoom variant of the same.
    #[must_use]
    pub fn stage_cameras(&self) -> (Option<u32>, Option<u32>) {
        let Some((species, sex)) = self.body_in_view() else {
            return (None, None);
        };
        let race = self.data.chargen.race_with_gender(species, sex);
        let gender = race.and_then(|r| r.gender(sex));
        match self.stage {
            Stage::Select => (
                race.and_then(|r| r.record.select_camera),
                gender.and_then(|g| g.select_zoom_camera),
            ),
            _ => (
                race.and_then(|r| r.record.chargen_camera),
                gender.and_then(|g| g.chargen_zoom_camera),
            ),
        }
    }

    /// The camera rig in view (§2.1.3): the base and zoom frames from the race scene's
    /// elements named by the stage cameras, turned about the figure's place; `None` when
    /// the scene or a frame is missing (a missing zoom frame falls back to the base).
    #[must_use]
    pub fn camera_rig(&self) -> Option<super::scene::CameraRig> {
        let (species, _) = self.body_in_view()?;
        let scene = self.data.scene(species)?;
        let (base, zoom) = self.stage_cameras();
        let base = scene.frame_of(base?)?;
        let zoom = zoom.and_then(|z| scene.frame_of(z)).unwrap_or(base);
        Some(super::scene::CameraRig {
            base: base.into(),
            zoom: zoom.into(),
            pivot: scene.figure_frame().position,
        })
    }

    /// The turn buttons (§2.3): ghosted while a zoom move runs.
    #[must_use]
    pub fn turn_buttons(&self) -> ButtonRule {
        if self.turntable.turn_ghosted() {
            ButtonRule::Ghosted(None)
        } else {
            ButtonRule::Enabled
        }
    }

    /// The zoom-in button (§2.3): ghosted while the zoom target is at its end.
    #[must_use]
    pub fn zoom_in_button(&self) -> ButtonRule {
        if self.turntable.zoom_in_ghosted() {
            ButtonRule::Ghosted(None)
        } else {
            ButtonRule::Enabled
        }
    }

    /// The zoom-out button (§2.3).
    #[must_use]
    pub fn zoom_out_button(&self) -> ButtonRule {
        if self.turntable.zoom_out_ghosted() {
            ButtonRule::Ghosted(None)
        } else {
            ButtonRule::Enabled
        }
    }

    /// A zoom button (§2.3): the timed move to `to` from the camera as it is now.
    fn zoom_to(&mut self, to: f32, now: Instant) {
        self.follow_camera();
        self.turntable.update(now);
        let rig = self.camera_rig();
        self.turntable.zoom_to(to, rig.as_ref(), now);
    }

    /// Resets the turntable when the stage camera in view changed (a new stage, race,
    /// gender or selected character; §2.1.3).
    fn follow_camera(&mut self) {
        let body = self.body_in_view();
        let row = (self.stage == Stage::Select)
            .then_some(self.selected)
            .flatten();
        let key = (self.stage, row, body);
        if self.camera_for != Some(key) {
            self.camera_for = Some(key);
            self.turntable.reset();
        }
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
        let Some(r) = self.rows.iter_mut().find(|r| same_name(&r.name, name)) else {
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
            self.hover = false;
            self.clicked = false;
            self.pointer_on_figure = false;
            self.errors.clear();
            self.follow_camera();
        }
    }

    /// Sets the character entered last on this session (screens that re-open after a
    /// return to character selection select it again; `Event::ReturnedToCharacterSelect`).
    pub fn set_last_entered(&mut self, name: Option<String>) {
        self.last_entered = name;
    }

    fn row_index(&self, name: &str) -> Option<usize> {
        self.rows.iter().position(|r| same_name(&r.name, name))
    }

    /// Takes one session event (`_now`: the frame's time, for answers that start clocks).
    pub fn handle_event(&mut self, event: &Event, _now: Instant) {
        match event {
            Event::Connected { characters } => self.set_rows(characters),
            Event::ReturnedToCharacterSelect { characters } => {
                // Back from the world on the same session (`official-messages-play.md`
                // §7.2): the selection screen again, with the list as it stands now; the
                // character entered last is selected.
                self.stage = Stage::Select;
                self.status = Status::Open;
                self.pending = None;
                self.timed_out = None;
                self.confirm = None;
                self.set_rows(characters);
            }
            Event::Character(c) => self.character_event(c),
            Event::WorldEntered { name, .. } => {
                self.last_entered = Some(name.clone());
                self.status = Status::Entered(name.clone());
            }
            Event::EnterWorldFailed { reason, .. } => {
                // The session is back at character selection and kept
                // (`tessera-protocol.md` §6.2.11, Appendix A): the selection screen stays
                // with the reason as an error line and Enter enabled again, as for a
                // refused `EnterWorld` (§7.3, Tessera rule).
                if matches!(self.status, Status::Entering(_)) {
                    self.status = Status::Open;
                }
                self.own_error(OwnText::EnterRefused(reason.clone()));
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

    /// The selection list from a character list (`Connected`, `ReturnedToCharacterSelect`):
    /// the character entered last is selected, else the first.
    fn set_rows(&mut self, characters: &[tessera_net::CharacterSummary]) {
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
                let answers_pending = self
                    .pending
                    .as_ref()
                    .is_some_and(|p| same_name(&p.name, name));
                if answers_pending
                    && self
                        .timed_out
                        .as_ref()
                        .is_some_and(|p| same_name(&p.name, name))
                {
                    // A retry of a timed-out creation: the retry's answer counts.
                    self.timed_out = None;
                }
                let late = !answers_pending
                    && self
                        .timed_out
                        .as_ref()
                        .is_some_and(|p| same_name(&p.name, name));
                if late {
                    // The answer came after the time-out (§7.1): the screen stays where it
                    // is, but the row gets the choices and they are kept.
                    let mut p = self.timed_out.take().expect("checked");
                    // Kept under the name as the session lists it (NFC on a test server).
                    p.created.name.clone_from(name);
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
                    let mut p = self.pending.take().expect("checked");
                    p.created.name.clone_from(name);
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
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|p| same_name(&p.name, name))
                {
                    self.pending = None;
                }
                if self
                    .timed_out
                    .as_ref()
                    .is_some_and(|p| same_name(&p.name, name))
                {
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
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|p| same_name(&p.name, name))
                {
                    self.pending = None;
                }
                self.own_error(OwnText::CreateRefused(why));
            }
            Command::Character(CharacterCommand::DeleteCharacter { .. }) => {
                self.delete_refused = true;
                self.own_error(OwnText::DeleteRefused);
            }
            Command::EnterWorld { .. } => {
                // §7.3: the selection screen stays with Enter enabled. A refusal while a
                // creation that timed out on this screen is still unanswered is the
                // backend's outstanding-creation rule (§7.1.1 step 6).
                self.status = Status::Open;
                if matches!(err, NetError::InvalidCommand(_)) && self.timed_out.is_some() {
                    self.own_error(OwnText::CreationStillOutstanding);
                } else {
                    self.own_error(OwnText::EnterRefused(why));
                }
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
        self.follow_camera();
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
                        self.hover = false;
                        self.clicked = false;
                        self.pointer_on_figure = false;
                    }
                    self.selected = Some(i);
                    self.follow_camera();
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
                    self.follow_camera();
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
            Action::TurnClick(t) => {
                self.follow_camera();
                self.turntable.turn_click(t, now);
            }
            Action::DragStart(x) => {
                // The press is a `SelectLeft` on the viewport: the figure under the pointer
                // becomes the clicked figure, or none (§2.3, CS32).
                if self.stage == Stage::Select {
                    self.clicked = self.pointer_on_figure;
                }
                self.follow_camera();
                self.turntable.drag_start(x);
            }
            Action::DragTo(x) => self.turntable.drag_to(x),
            Action::DragEnd => self.turntable.drag_end(),
            Action::Wheel(n) => {
                self.follow_camera();
                self.turntable.wheel(n, now);
            }
            Action::ZoomIn => self.zoom_to(1.0, now),
            Action::ZoomOut => self.zoom_to(0.0, now),
            // §2.1.4, §2.3 (CS32): the figure gets `Hover` when it becomes the object under
            // the pointer and `Idle` when it stops being it, unless it is the clicked one.
            Action::FigureHover(on) => {
                let on = on && self.stage == Stage::Select;
                self.pointer_on_figure = on;
                if !self.clicked {
                    self.hover = on;
                }
            }
            Action::ToggleGroupLock(group) => {
                if self.stage == Stage::Appearance && !self.creation.locked_groups.remove(&group) {
                    self.creation.locked_groups.insert(group);
                }
            }
            Action::CollapseGroup(group, collapsed) => {
                if self.stage == Stage::Appearance {
                    if collapsed {
                        self.creation.collapsed_groups.insert(group);
                    } else {
                        self.creation.collapsed_groups.remove(&group);
                    }
                }
            }
            Action::ToggleLock(index) => {
                if self.stage == Stage::Appearance
                    && self
                        .current_avatar()
                        .is_some_and(|d| d.control(index).is_some())
                    && !self.creation.locked.remove(&index)
                {
                    self.creation.locked.insert(index);
                }
            }
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
        let before = self.creation.appearance.clone();
        self.creation.appearance.randomise(
            &d,
            seed,
            &|c| data.chargen.palette_for(&c.control, nationality),
            sampler,
        );
        // Locked controls, and every control of a locked group, keep their value (Tessera
        // rules of §5.2.2).
        let group_locked: Vec<usize> = d
            .controls
            .iter()
            .filter(|c| {
                c.control
                    .group
                    .is_some_and(|g| self.creation.locked_groups.contains(&g))
            })
            .map(|c| c.control.index)
            .collect();
        for i in self.creation.locked.iter().chain(&group_locked) {
            match before.values.get(i) {
                Some(v) => {
                    self.creation.appearance.values.insert(*i, v.clone());
                }
                None => {
                    self.creation.appearance.values.remove(i);
                }
            }
        }
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
        self.follow_camera();
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
            self.creation.locked = self
                .data
                .avatar(species, sex)
                .map(|d| {
                    d.controls
                        .iter()
                        .filter(|c| c.meta.default_locked)
                        .map(|c| c.control.index)
                        .collect()
                })
                .unwrap_or_default();
            self.creation.locked_groups.clear();
            self.creation.collapsed_groups.clear();
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
