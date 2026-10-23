// SPDX-License-Identifier: GPL-3.0-or-later
//! The flow from the start screen to the character screens (`docs/specs/official-entry.md`
//! §9), a state machine with the record directory, the backends and the plugin host
//! injected (§6.4), driven by device input and a caller-supplied clock.
//!
//! | Step | Screen | Next |
//! |---|---|---|
//! | 1 | "Official servers" chosen (or `--official`) | 2 |
//! | 2 | [`Screen::Checking`]: the record check on a worker | accepted: 4; else 3 |
//! | 3 | [`Screen::Warning`] | Continue: write, 4; Decline: start |
//! | 4 | availability | `NotBuilt`: message, start; `Available`: 5 |
//! | 5 | [`Screen::Form`] | Log in: backend, session, `Connect`, 6; Back: start |
//! | 6 | [`Screen::Login`] | `Connected`: 7; a refusal or end: §8.6 |
//! | 7 | [`Screen::Characters`] | — |
//!
//! The acceptance made at step 2 or 3 is held for the run; each backend constructed at step 5
//! gets a new proof from it, without reading the record again. Withdrawing drops it.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use tessera_compat::consent::{
    self, Acceptance, CheckOutcome, NotAcceptedReason, PendingCheck, PendingWithdraw, PendingWrite,
    WarningText, WithdrawReport, WriteError, strings::text,
};
use tessera_compat::{Availability, CompatConfig, CompatError};
use tessera_net::{Backend, Command, Event, Session};

use super::form::{AccountBuffer, AccountForm, FormAction, FormControl};
use super::input::{DeviceInput, DeviceInputKind, Key};
use super::login::{LoginEnd, LoginPresenter, LoginScreen, Message, Target};
use super::warning::{WarningControl, WarningOutcome, WarningScreen};

/// The backends of official play, injected so tests can count constructions and use test
/// doubles (§6.4). The client's one implementation is
/// [`super::backend::ProductionBackends`].
pub trait BackendFactory: std::fmt::Debug {
    /// The availability answer of §4.4.
    fn availability(&self) -> Availability;

    /// Constructs the backend for one login attempt (§6.2: reached only from step 5).
    ///
    /// # Errors
    ///
    /// The compatibility crate's refusal (a proof for another warning text).
    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError>;
}

/// The plugin host as the flow sees it (§4.5): not started while the start, warning, form or
/// login screens show; started at the hand-over of §8.7; stopped (every apartment unloaded)
/// before the start screen or the form shows again.
pub trait PluginHostControl: std::fmt::Debug {
    /// Starts the host (discovery, registry).
    fn start(&mut self);
    /// Unloads every apartment and stops the host.
    fn stop(&mut self);
    /// Whether the host runs.
    fn is_running(&self) -> bool;
    /// Apartments that exist.
    fn apartments(&self) -> usize;
    /// How often the host was started (tests: OE-T7 (d)).
    fn starts(&self) -> u32;
}

/// Where the flow is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The start screen (§4.1).
    Start,
    /// Step 2: "Checking…".
    Checking,
    /// Step 3: the warning screen.
    Warning,
    /// Step 5: the account form.
    Form,
    /// Step 6: the login screens.
    Login,
    /// Step 7: the character screens (module docs of `super`).
    Characters,
}

/// A control of any official screen under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// "Explore offline".
    StartOffline,
    /// "Official servers".
    StartOfficial,
    /// "Withdraw acceptance".
    StartWithdraw,
    /// "Quit".
    StartQuit,
    /// A warning-screen control.
    Warning(WarningControl),
    /// An account-form control.
    Form(FormControl),
    /// A world-list row.
    WorldRow(usize),
    /// "Join".
    Join,
    /// "Back" on the world list.
    WorldsBack,
    /// "Cancel" on the progress screen or the queue panel.
    Cancel,
    /// "Leave queue".
    LeaveQueue,
    /// OK of a message panel.
    MessageOk,
    /// "Back" on the character screens.
    CharactersBack,
}

/// What the flow asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowRequest {
    /// Start the offline walker as without a mode option (sandbox backend).
    ExploreOffline,
    /// End the client (exit code 0).
    Quit,
}

/// The start screen's acceptance line (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceLine<'a> {
    /// A stored record (or the run's acceptance while its write runs): "You accepted … on
    /// ⟨date⟩."
    Stored(&'a str),
    /// An acceptance for this run only: the text ID (`official.consent.saved_failed` or
    /// `official.consent.not_stored`).
    RunOnly(&'static str),
}

/// A message panel over the current screen, with OK (§8.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The message.
    pub message: Message,
}

/// The flow's configuration.
#[derive(Debug)]
pub struct FlowConfig {
    /// Where the acceptance record is (`None`: the platform names no config directory; every
    /// check gives not accepted and every write fails).
    pub record_dir: Option<PathBuf>,
    /// The shown warning text.
    pub text: Arc<WarningText>,
    /// Tessera's settings file for `[Official] last_world` (`None`: nothing is remembered).
    pub settings_file: Option<PathBuf>,
    /// `[Official] last_world` as read at start-up.
    pub last_world: Option<String>,
    /// `--load-plugin` names queued for this run (dropped when official play is chosen,
    /// §4.5).
    pub queued_plugin_loads: usize,
}

/// The flow (module docs).
#[derive(Debug)]
pub struct Flow {
    config: FlowConfig,
    factory: Box<dyn BackendFactory>,
    plugins: Box<dyn PluginHostControl>,
    screen: Screen,
    notice: Option<Notice>,
    /// The start screen's line: the date of a valid acceptance (§4.1).
    accepted_date: Option<String>,
    /// The text ID of the start screen's line while the run's acceptance is not stored: the
    /// record write failed (`.saved_failed`, an invalid key file included, §5.6) or no record
    /// is stored on this system (`.not_stored`, Windows).
    run_only: Option<&'static str>,
    start_check: Option<PendingCheck>,
    pending_check: Option<PendingCheck>,
    warning: Option<WarningScreen>,
    acceptance: Option<Acceptance>,
    pending_write: Option<PendingWrite>,
    /// A withdrawal running on the consent worker (§5.5; ARCH §21).
    pending_withdraw: Option<PendingWithdraw>,
    form: Option<AccountForm>,
    login: Option<(Session, LoginPresenter)>,
    /// The account name of the running login, only for `BadCredentials` (§8.6).
    kept_account: Option<AccountBuffer>,
    /// The session handed over to the character screens (§8.7).
    characters: Option<(Session, Vec<tessera_net::CharacterSummary>)>,
    request: Option<FlowRequest>,
    constructions: usize,
    plugin_loads_dropped: bool,
    /// Keys down now (device input), so the warning ignores keys held when it appears.
    down: HashSet<Key>,
    pointer: (f64, f64),
    pressed_on: Option<Control>,
    events: Vec<Event>,
    last_world: Option<String>,
    /// The `last_world` writer thread, started with the first write (§8.5; the frame thread
    /// only queues names to it, ARCH §21).
    writer: Option<SettingsWriter>,
}

impl Flow {
    /// A flow on the start screen.
    #[must_use]
    pub fn new(
        config: FlowConfig,
        factory: Box<dyn BackendFactory>,
        plugins: Box<dyn PluginHostControl>,
    ) -> Self {
        let last_world = config.last_world.clone();
        let mut f = Self {
            config,
            factory,
            plugins,
            screen: Screen::Start,
            notice: None,
            accepted_date: None,
            run_only: None,
            start_check: None,
            pending_check: None,
            warning: None,
            acceptance: None,
            pending_write: None,
            pending_withdraw: None,
            form: None,
            login: None,
            kept_account: None,
            characters: None,
            request: None,
            constructions: 0,
            plugin_loads_dropped: false,
            down: HashSet::new(),
            pointer: (-1.0, -1.0),
            pressed_on: None,
            events: Vec::new(),
            last_world,
            writer: None,
        };
        f.go_start();
        f
    }

    /// The screen that shows.
    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The message panel over the screen, if one shows.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// The start screen's acceptance date: of a record that passed the check, else of the
    /// run's in-memory acceptance (a failed write, every acceptance on Windows). The line of
    /// §4.1 with Withdraw shows while it is `Some`.
    #[must_use]
    pub fn accepted_date(&self) -> Option<&str> {
        self.accepted_date
            .as_deref()
            .or_else(|| self.acceptance.as_ref().map(Acceptance::accepted_date))
    }

    /// The start screen's acceptance line (§4.1), while a valid acceptance exists: the date of
    /// a stored record, else the text of an acceptance that holds for this run only.
    #[must_use]
    pub fn acceptance_line(&self) -> Option<AcceptanceLine<'_>> {
        if let Some(d) = self.accepted_date.as_deref() {
            return Some(AcceptanceLine::Stored(d));
        }
        let acc = self.acceptance.as_ref()?;
        Some(match self.run_only {
            Some(id) => AcceptanceLine::RunOnly(id),
            // The write still runs: the run's acceptance date until the check of the start
            // screen sees the record.
            None => AcceptanceLine::Stored(acc.accepted_date()),
        })
    }

    /// The account-name copy of the running attempt, every byte (tests: OE-T19).
    #[cfg(test)]
    pub(crate) fn kept_account_raw(&self) -> Option<&[u8]> {
        self.kept_account.as_ref().map(|k| k.raw().as_slice())
    }

    /// The warning screen, while it shows.
    #[must_use]
    pub fn warning(&self) -> Option<&WarningScreen> {
        self.warning.as_ref()
    }

    /// The warning screen, mutable (the view reports the laid-out text and the scroll range).
    pub fn warning_mut(&mut self) -> Option<&mut WarningScreen> {
        self.warning.as_mut()
    }

    /// The account form, while it shows.
    #[must_use]
    pub fn form(&self) -> Option<&AccountForm> {
        self.form.as_ref()
    }

    /// The login presenter, while the login runs.
    #[must_use]
    pub fn login(&self) -> Option<&LoginPresenter> {
        self.login.as_ref().map(|(_, p)| p)
    }

    /// The character list handed over (step 7).
    #[must_use]
    pub fn characters(&self) -> Option<&[tessera_net::CharacterSummary]> {
        self.characters.as_ref().map(|(_, c)| c.as_slice())
    }

    /// Backends constructed so far (tests: OE-T1).
    #[must_use]
    pub fn constructions(&self) -> usize {
        self.constructions
    }

    /// Whether an acceptance holds for this run.
    #[must_use]
    pub fn has_acceptance(&self) -> bool {
        self.acceptance.is_some()
    }

    /// The plugin host (tests: OE-T7 (d)).
    #[must_use]
    pub fn plugin_host(&self) -> &dyn PluginHostControl {
        self.plugins.as_ref()
    }

    /// Whether the `--load-plugin` names were dropped because official play was chosen.
    #[must_use]
    pub fn plugin_loads_dropped(&self) -> bool {
        self.plugin_loads_dropped
    }

    /// The window's next request.
    pub fn take_request(&mut self) -> Option<FlowRequest> {
        self.request.take()
    }

    /// Waits until the writer thread has written every queued `last_world` (tests).
    pub fn wait_writes(&mut self) {
        if let Some(w) = self.writer.as_ref() {
            let (tx, rx) = mpsc::sync_channel(1);
            if w.tx.send(WriterJob::Flush(tx)).is_ok() {
                let _ = rx.recv();
            }
        }
    }

    /// Ends the writer thread after its queued writes (after the event loop, never in a
    /// frame).
    pub fn finish_writes(&mut self) {
        if let Some(w) = self.pending_withdraw.take() {
            let _ = w.wait();
        }
        if let Some(w) = self.writer.take() {
            drop(w.tx);
            let _ = w.handle.join();
        }
    }

    /// The message for a record write that failed (§5.5): `.not_stored` where no record is
    /// stored (Windows), else `.saved_failed`.
    fn write_failed(&mut self, e: &WriteError) {
        let id = if matches!(e, WriteError::NotStored) {
            "official.consent.not_stored"
        } else {
            "official.consent.saved_failed"
        };
        self.run_only = Some(id);
        self.show(Message {
            text: text(id).to_owned(),
            detail: None,
        });
    }

    fn show(&mut self, message: Message) {
        self.notice = Some(Notice { message });
    }

    /// Shows the start screen: the plugin host is stopped first (§4.5), and the record is
    /// checked for the line of §4.1.
    fn go_start(&mut self) {
        if self.plugins.is_running() || self.plugins.apartments() > 0 {
            self.plugins.stop();
        }
        self.warning = None;
        if let Some(f) = self.form.as_mut() {
            f.clear();
        }
        self.form = None;
        self.end_login(false);
        self.screen = Screen::Start;
        self.accepted_date = None;
        self.start_check = self
            .config
            .record_dir
            .clone()
            .map(|d| consent::spawn_check(d, Arc::clone(&self.config.text)));
    }

    /// Step 1: the user chose "Official servers" (or `--official` chose it).
    pub fn choose_official(&mut self) {
        if self.screen != Screen::Start {
            return;
        }
        if self.pending_withdraw.is_some() {
            // A withdrawal still runs: its result decides whether an acceptance holds.
            return;
        }
        if self.config.queued_plugin_loads > 0 && !self.plugin_loads_dropped {
            tracing::warn!(
                dropped = self.config.queued_plugin_loads,
                "official play: {} --load-plugin name(s) dropped (no plugin loads before the login)",
                self.config.queued_plugin_loads
            );
            self.plugin_loads_dropped = true;
        }
        self.notice = None;
        self.start_check = None;
        self.screen = Screen::Checking;
        self.pending_check = self
            .config
            .record_dir
            .clone()
            .map(|d| consent::spawn_check(d, Arc::clone(&self.config.text)));
    }

    /// The check of step 2 finished.
    fn checked(&mut self, outcome: CheckOutcome) {
        match outcome {
            CheckOutcome::Accepted(acc) => {
                self.acceptance = Some(acc);
                self.step4();
            }
            // An acceptance of this run (a write that failed) counts for the run (§5.5).
            CheckOutcome::NotAccepted(_)
                if self
                    .acceptance
                    .as_ref()
                    .is_some_and(|a| a.digest() == self.config.text.digest()) =>
            {
                self.step4();
            }
            CheckOutcome::NotAccepted(reason) => {
                self.acceptance = None;
                self.warning = Some(WarningScreen::new(
                    self.config.record_dir.clone(),
                    Arc::clone(&self.config.text),
                    reason == NotAcceptedReason::TextChanged,
                    self.down.iter().copied(),
                ));
                self.screen = Screen::Warning;
            }
        }
    }

    /// Step 4: availability (§4.4), after the warning.
    fn step4(&mut self) {
        self.warning = None;
        match self.factory.availability() {
            Availability::Available => {
                self.form = Some(AccountForm::new());
                self.screen = Screen::Form;
            }
            Availability::NotBuilt => {
                self.go_start();
                self.show(Message {
                    text: text("official.not_available").to_owned(),
                    detail: None,
                });
            }
        }
    }

    /// Step 5: Log in. The backend gets a new proof from the run's acceptance and the
    /// credentials (§6.2, §7.3); a new session sends `Connect`.
    fn log_in(
        &mut self,
        credentials: tessera_compat::Credentials,
        account: AccountBuffer,
        now: Instant,
    ) {
        let Some(acc) = self.acceptance.as_ref() else {
            // Unreachable by construction: the form shows only after step 2 or 3.
            self.go_start();
            return;
        };
        let config = CompatConfig::new(acc.proof(), credentials);
        let backend = match self.factory.construct(config) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("official login: no backend ({e})");
                let mut form = AccountForm::new();
                form.block_until(now + super::login::FAILURE_WAIT, false);
                self.form = Some(form);
                self.show(Message {
                    text: text("official.error.other").to_owned(),
                    detail: None,
                });
                return;
            }
        };
        self.constructions += 1;
        let mut session = Session::from_boxed(backend);
        if let Err(e) = session.send(now, Command::Connect) {
            tracing::warn!("official login: Connect refused ({e})");
        }
        self.form = None;
        self.kept_account = Some(account);
        self.login = Some((session, LoginPresenter::new(now, self.last_world.clone())));
        self.screen = Screen::Login;
        tracing::info!("official login started");
    }

    /// Drops the running login (sending `Disconnect` when asked) and wipes the kept account.
    fn end_login(&mut self, disconnect: bool) {
        if let Some((mut session, _)) = self.login.take()
            && disconnect
        {
            let _ = session.send(Instant::now(), Command::Disconnect);
        }
        if let Some(k) = self.kept_account.as_mut() {
            k.clear();
        }
    }

    /// The login ended (§8.6, §8.7).
    fn login_ended(&mut self, end: LoginEnd, now: Instant) {
        let Some((session, _)) = self.login.take() else {
            return;
        };
        tracing::info!(target = ?std::mem::discriminant(&end.target), "official login ended");
        // The account-name copy is used only for the `BadCredentials` refill and overwritten
        // with zeros as soon as the outcome is known (§7.3).
        let refill = match (&end.target, self.kept_account.as_ref()) {
            (Target::AccountForm { keep_account: true }, Some(k)) => {
                Some(AccountForm::with_account(k))
            }
            _ => None,
        };
        if let Some(k) = self.kept_account.as_mut() {
            k.clear();
        }
        match end.target {
            Target::CharacterScreens(characters) => {
                // §8.7: the placeholder; the plugin host stays stopped (§4.5) until real
                // character screens take the session.
                self.characters = Some((session, characters));
                self.screen = Screen::Characters;
            }
            Target::AccountForm { .. } => {
                drop(session);
                let mut form = refill.unwrap_or_default();
                form.block_until(now + end.wait, end.rate_limited);
                self.form = Some(form);
                self.screen = Screen::Form;
                if let Some(m) = end.message {
                    self.show(m);
                }
            }
            Target::StartScreen => {
                drop(session);
                self.go_start();
                if let Some(m) = end.message {
                    self.show(m);
                }
            }
        }
    }

    /// Leaves the character placeholder (Back, Escape or closing the window): the session ends
    /// (`Disconnect`), the plugin host is stopped if anything started it, the account form
    /// shows with both fields empty (§8.7, §4.5).
    fn leave_characters(&mut self) {
        if let Some((mut session, _)) = self.characters.take() {
            let _ = session.send(Instant::now(), Command::Disconnect);
        }
        self.plugins.stop();
        self.form = Some(AccountForm::new());
        self.screen = Screen::Form;
    }

    fn quit(&mut self) {
        if let Some(f) = self.form.as_mut() {
            f.clear();
        }
        self.end_login(true);
        if self.characters.is_some() {
            self.leave_characters();
        }
        if self.plugins.is_running() {
            self.plugins.stop();
        }
        self.request = Some(FlowRequest::Quit);
    }

    /// Withdraws the acceptance (§5.5): both files and the run's acceptance. The work runs on
    /// the consent worker (ARCH §21): a record write still running finishes there first, so it
    /// cannot bring the record back afterwards; [`Self::withdrawn`] applies the report.
    fn withdraw(&mut self) {
        if self.pending_withdraw.is_some() {
            return;
        }
        self.start_check = None;
        self.pending_withdraw = Some(consent::spawn_withdraw(
            self.config.record_dir.clone(),
            self.pending_write.take(),
        ));
    }

    /// The withdrawal finished. Both files gone: the run's acceptance is dropped too and the
    /// line disappears. A file left (§5.6: the log names it): a failure notice shows, and the
    /// line is recomputed: a removed record no longer gives the stored line, the run's
    /// acceptance (kept, as the withdrawal did not complete) shows the run-only line, and a
    /// new check of what is left on disk restores the stored line only if the record still
    /// validates.
    fn withdrawn(&mut self, report: WithdrawReport) {
        if report.complete() {
            tracing::info!("official-server warning: acceptance withdrawn");
            self.acceptance = None;
            self.accepted_date = None;
            self.run_only = None;
            self.start_check = None;
            self.show(Message {
                text: text("official.consent.withdrawn").to_owned(),
                detail: None,
            });
            return;
        }
        tracing::warn!(
            record_removed = report.record_removed,
            key_removed = report.key_removed,
            "official-server warning: withdrawing did not complete"
        );
        self.accepted_date = None;
        if self.acceptance.is_some() && self.run_only.is_none() {
            self.run_only = Some(if consent::STORES_RECORDS {
                "official.consent.saved_failed"
            } else {
                "official.consent.not_stored"
            });
        }
        self.start_check = self
            .config
            .record_dir
            .clone()
            .map(|d| consent::spawn_check(d, Arc::clone(&self.config.text)));
        self.show(Message {
            text: text("official.consent.withdraw_failed").to_owned(),
            detail: None,
        });
    }

    /// Whether a withdrawal runs.
    #[must_use]
    pub fn withdrawing(&self) -> bool {
        self.pending_withdraw.is_some()
    }

    /// Blocks until the withdrawal finished (tests), then ticks.
    pub fn wait_withdraw(&mut self, now: Instant) {
        if let Some(w) = self.pending_withdraw.take() {
            let report = w.wait();
            self.withdrawn(report);
        }
        self.tick(now);
    }

    /// Advances the flow to `now`: the record check and write, the login session.
    pub fn tick(&mut self, now: Instant) {
        if let Some(c) = self.start_check.as_mut()
            && let Some(outcome) = c.poll()
        {
            self.start_check = None;
            if let CheckOutcome::Accepted(acc) = &outcome {
                self.accepted_date = Some(acc.accepted_date().to_owned());
            }
        }
        if let Some(c) = self.pending_check.as_mut()
            && let Some(outcome) = c.poll()
        {
            self.pending_check = None;
            self.checked(outcome);
        } else if self.screen == Screen::Checking && self.config.record_dir.is_none() {
            self.checked(CheckOutcome::NotAccepted(NotAcceptedReason::Missing));
        }
        if let Some(w) = self.pending_write.as_mut()
            && let Some(result) = w.poll()
        {
            self.pending_write = None;
            if let Err(e) = result {
                self.write_failed(&e);
            }
        }
        if let Some(w) = self.pending_withdraw.as_mut()
            && let Some(report) = w.poll()
        {
            self.pending_withdraw = None;
            self.withdrawn(report);
        }
        self.tick_login(now);
    }

    fn tick_login(&mut self, now: Instant) {
        let mut events = std::mem::take(&mut self.events);
        let mut ended = None;
        let mut remember = None;
        if let Some((session, presenter)) = self.login.as_mut() {
            events.clear();
            session.poll_into(now, &mut events);
            for e in &events {
                presenter.on_event(e, now);
            }
            presenter.tick(now);
            for c in presenter.take_commands() {
                if let Err(e) = session.send(now, c) {
                    tracing::debug!("official login: command refused ({e})");
                }
            }
            remember = presenter.take_remember();
            ended = presenter.end().cloned();
        }
        self.events = events;
        if let Some(name) = remember {
            self.remember_world(name);
        }
        if let Some(end) = ended {
            self.login_ended(end, now);
        }
        if let Some((session, _)) = self.characters.as_mut() {
            let mut events = std::mem::take(&mut self.events);
            events.clear();
            session.poll_into(now, &mut events);
            let ended = events
                .iter()
                .any(|e| matches!(e, Event::Disconnected { .. }));
            self.events = events;
            if ended {
                self.leave_characters();
            }
        }
    }

    /// Queues `[Official] last_world` to the writer thread (§8.5): the frame thread never
    /// waits for the settings file (ARCH §21).
    fn remember_world(&mut self, name: String) {
        let name = tessera_net::model::sanitize_text(&name, 64).into_owned();
        self.last_world = Some(name.clone());
        let Some(path) = self.config.settings_file.clone() else {
            return;
        };
        if self.writer.is_none() {
            self.writer = SettingsWriter::spawn();
        }
        if let Some(w) = self.writer.as_ref()
            && w.tx.send(WriterJob::Save(path, name)).is_err()
        {
            tracing::warn!("official: last world not saved (no writer)");
        }
    }

    /// One device-input event (§5.4 rule 1: handed directly by the window-event handler);
    /// `hit` names the control under a window position in the current layout.
    pub fn device_input(&mut self, ev: &DeviceInput, hit: &dyn Fn(f64, f64) -> Option<Control>) {
        match ev.kind() {
            DeviceInputKind::Key {
                key, pressed: true, ..
            } => {
                self.down.insert(*key);
            }
            DeviceInputKind::Key {
                key,
                pressed: false,
                ..
            } => {
                self.down.remove(key);
            }
            DeviceInputKind::FocusLost => self.down.clear(),
            DeviceInputKind::PointerMoved { x, y } => self.pointer = (*x, *y),
            _ => {}
        }
        if matches!(ev.kind(), DeviceInputKind::CloseRequested) {
            self.close_requested(ev);
            return;
        }
        if self.screen == Screen::Warning {
            self.warning_input(ev, hit);
            return;
        }
        if self.notice.is_some() {
            self.notice_input(ev, hit);
            return;
        }
        match self.screen {
            Screen::Form => self.form_input(ev, hit),
            Screen::Start | Screen::Checking | Screen::Login | Screen::Characters => {
                self.buttons_input(ev, hit);
            }
            Screen::Warning => {}
        }
    }

    /// Closing the window (§4.6 "Closing the window"): only on the start screen does it end
    /// the client; on every other official screen it is that screen's way back, one step
    /// towards the start screen. A message panel (the not-available message of §4.4 and the
    /// other notices, a login screen's message) is its OK; the warning declines; the account
    /// form is its Back; the progress, world-list and queue screens are Cancel (`Disconnect`,
    /// the account form); the character placeholder is its Back (§8.7). The "Checking…" screen
    /// of step 2 goes back to the start screen without an acceptance.
    fn close_requested(&mut self, ev: &DeviceInput) {
        let now = ev.at();
        self.pressed_on = None;
        match self.screen {
            Screen::Characters => {
                self.notice = None;
                self.leave_characters();
            }
            Screen::Warning => {
                tracing::info!("official-server warning declined (window closed)");
                self.go_start();
            }
            _ if self.notice.is_some() => self.notice = None,
            Screen::Start => {
                self.quit();
            }
            Screen::Checking => {
                self.pending_check = None;
                self.go_start();
            }
            Screen::Form => {
                // The form zeroes both fields on its Back.
                self.form_input(ev, &|_, _| None);
            }
            Screen::Login => {
                if let Some((_, p)) = self.login.as_mut() {
                    if p.message().is_some() {
                        p.dismiss_message();
                        return;
                    }
                    p.cancel();
                }
                // Send the `Disconnect` and show the account form now, not at the next frame.
                self.tick_login(now);
            }
        }
    }

    fn warning_input(&mut self, ev: &DeviceInput, hit: &dyn Fn(f64, f64) -> Option<Control>) {
        let Some(w) = self.warning.as_mut() else {
            return;
        };
        let warning_hit = |x: f64, y: f64| match hit(x, y) {
            Some(Control::Warning(c)) => Some(c),
            _ => None,
        };
        match w.input(ev, &warning_hit) {
            Some(WarningOutcome::Accepted { acceptance, write }) => {
                self.acceptance = Some(acceptance);
                self.pending_write = Some(write);
                self.step4();
            }
            Some(WarningOutcome::Declined) => {
                tracing::info!("official-server warning declined");
                self.go_start();
            }
            None => {}
        }
    }

    fn notice_input(&mut self, ev: &DeviceInput, hit: &dyn Fn(f64, f64) -> Option<Control>) {
        let ok = match ev.kind() {
            DeviceInputKind::Key {
                key: Key::Escape | Key::Enter | Key::Space,
                pressed: true,
                held_before: false,
                repeat: false,
                ..
            } => true,
            DeviceInputKind::PointerButton { pressed: true } => {
                self.pressed_on = hit(self.pointer.0, self.pointer.1);
                false
            }
            DeviceInputKind::PointerButton { pressed: false } => {
                let p = self.pressed_on.take();
                p == Some(Control::MessageOk) && hit(self.pointer.0, self.pointer.1) == p
            }
            _ => false,
        };
        if ok {
            self.notice = None;
        }
    }

    fn form_input(&mut self, ev: &DeviceInput, hit: &dyn Fn(f64, f64) -> Option<Control>) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let form_hit = |x: f64, y: f64| match hit(x, y) {
            Some(Control::Form(c)) => Some(c),
            _ => None,
        };
        match form.input(ev, &form_hit) {
            Some(FormAction::LogIn {
                credentials,
                account,
            }) => self.log_in(credentials, account, ev.at()),
            Some(FormAction::Back) => self.go_start(),
            None => {}
        }
    }

    /// Clicks and keys of the start, checking, login and character screens.
    fn buttons_input(&mut self, ev: &DeviceInput, hit: &dyn Fn(f64, f64) -> Option<Control>) {
        let clicked = match ev.kind() {
            DeviceInputKind::PointerButton { pressed: true } => {
                self.pressed_on = hit(self.pointer.0, self.pointer.1);
                if let (Some(Control::WorldRow(i)), Some((_, p))) =
                    (self.pressed_on, self.login.as_mut())
                {
                    p.select(i);
                }
                None
            }
            DeviceInputKind::PointerButton { pressed: false } => {
                let p = self.pressed_on.take();
                (p.is_some() && hit(self.pointer.0, self.pointer.1) == p)
                    .then_some(p)
                    .flatten()
            }
            DeviceInputKind::Key {
                key,
                pressed: true,
                held_before: false,
                repeat,
                ..
            } => {
                self.key(*key, *repeat, ev.at());
                None
            }
            _ => None,
        };
        if let Some(c) = clicked {
            self.click(c, ev.at());
        }
    }

    fn key(&mut self, key: Key, repeat: bool, now: Instant) {
        match (self.screen, key) {
            (Screen::Start, Key::Escape) if !repeat => self.quit(),
            (Screen::Login, k) => {
                let Some((_, p)) = self.login.as_mut() else {
                    return;
                };
                if p.message().is_some() {
                    if matches!(k, Key::Escape | Key::Enter) && !repeat {
                        p.dismiss_message();
                    }
                    return;
                }
                match (p.screen().clone(), k) {
                    (LoginScreen::Worlds, Key::Up) => p.move_selection(-1),
                    (LoginScreen::Worlds, Key::Down) => p.move_selection(1),
                    (LoginScreen::Worlds, Key::Enter) if !repeat => p.join(now),
                    (LoginScreen::Queue { .. }, Key::Escape) if !repeat => p.leave_queue(now),
                    (_, Key::Escape) if !repeat => p.cancel(),
                    _ => {}
                }
            }
            (Screen::Characters, Key::Escape) if !repeat => self.leave_characters(),
            _ => {}
        }
    }

    fn click(&mut self, c: Control, now: Instant) {
        match (self.screen, c) {
            (Screen::Start, Control::StartOffline) => {
                self.request = Some(FlowRequest::ExploreOffline);
            }
            (Screen::Start, Control::StartOfficial) => self.choose_official(),
            (Screen::Start, Control::StartWithdraw) if self.accepted_date().is_some() => {
                self.withdraw();
            }
            (Screen::Start, Control::StartQuit) => self.quit(),
            (Screen::Login, c) => {
                let Some((_, p)) = self.login.as_mut() else {
                    return;
                };
                match c {
                    Control::MessageOk => p.dismiss_message(),
                    Control::Join => p.join(now),
                    Control::WorldRow(i) => {
                        // A second click on the selected row joins (a double click).
                        if p.selected() == Some(i) {
                            p.join(now);
                        } else {
                            p.select(i);
                        }
                    }
                    Control::WorldsBack | Control::Cancel => p.cancel(),
                    Control::LeaveQueue => p.leave_queue(now),
                    _ => {}
                }
            }
            (Screen::Characters, Control::CharactersBack) => self.leave_characters(),
            _ => {}
        }
    }

    /// Whether the record check of step 2 runs (the "Checking…" screen).
    #[must_use]
    pub fn checking(&self) -> bool {
        self.pending_check.is_some()
    }

    /// Blocks until the step-2 or start-screen check finished (tests).
    pub fn wait_checks(&mut self, now: Instant) {
        if let Some(c) = self.start_check.take()
            && let CheckOutcome::Accepted(acc) = c.wait()
        {
            self.accepted_date = Some(acc.accepted_date().to_owned());
        }
        if let Some(c) = self.pending_check.take() {
            let outcome = c.wait();
            self.checked(outcome);
        }
        self.tick(now);
    }

    /// Blocks until the record write finished (tests), then ticks.
    pub fn wait_write(&mut self, now: Instant) {
        if let Some(w) = self.pending_write.take()
            && let Err(e) = w.wait()
        {
            self.write_failed(&e);
        }
        self.tick(now);
    }

    /// The login screen that shows, while the login runs.
    #[must_use]
    pub fn login_screen(&self) -> Option<&LoginScreen> {
        self.login().map(LoginPresenter::screen)
    }

    /// Duration helper for views: the remaining `RateLimited` seconds.
    #[must_use]
    pub fn rate_limit_left(&self, now: Instant) -> Option<u64> {
        self.form.as_ref().and_then(|f| f.rate_limit_left(now))
    }
}

/// A job of the `last_world` writer thread.
#[derive(Debug)]
enum WriterJob {
    /// Write this name to this settings file.
    Save(PathBuf, String),
    /// Answer once every earlier job is done.
    Flush(mpsc::SyncSender<()>),
}

/// The `last_world` writer thread (§8.5) and its queue.
#[derive(Debug)]
struct SettingsWriter {
    tx: mpsc::Sender<WriterJob>,
    handle: std::thread::JoinHandle<()>,
}

impl SettingsWriter {
    fn spawn() -> Option<Self> {
        let (tx, rx) = mpsc::channel::<WriterJob>();
        let handle = std::thread::Builder::new()
            .name("tessera-official-settings".into())
            .spawn(move || {
                for job in rx {
                    match job {
                        WriterJob::Save(path, name) => save_last_world(path, name),
                        WriterJob::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .ok()?;
        Some(Self { tx, handle })
    }
}

/// Writes `[Official] last_world = name` with the settings crate's locked update.
fn save_last_world(path: PathBuf, name: String) {
    let store = tessera_settings::SettingsStore::new(path);
    let r = store.update(|s| {
        let section = s
            .extra
            .entry("Official")
            .or_insert_with(|| tessera_settings::toml::Value::Table(Default::default()));
        if let Some(t) = section.as_table_mut() {
            t.insert(
                "last_world".to_owned(),
                tessera_settings::toml::Value::String(name),
            );
        }
    });
    if let Err(e) = r {
        tracing::warn!("official: last world not saved: {e}");
    }
}
