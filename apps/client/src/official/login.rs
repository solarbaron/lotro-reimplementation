// SPDX-License-Identifier: GPL-3.0-or-later
//! The login screens over the abstract backend (`docs/specs/official-entry.md` §8.3–§8.6):
//! progress, the world list, the queue panel, and how each refusal or end of the session
//! is presented and where it leads.
//!
//! [`LoginPresenter`] is pure: it takes the session's events and the user's actions with a
//! caller-supplied clock, and returns the commands to send. It draws nothing and names no
//! other client module, so the fuzz target `client_official_entry` includes this file by
//! path. Every text is one of Tessera's own (`tessera_compat::consent::strings`); service
//! text (world names, notes, details) is shown as plain text after `sanitize_text`.

use std::time::{Duration, Instant};

use tessera_compat::consent::strings::{fill, text};
use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, MAX_ACCOUNT_DETAIL_LEN,
    MAX_WORLD_NAME_LEN, MAX_WORLD_NOTE_LEN, MAX_WORLDS, WorldId, WorldLoad, WorldRefusal,
    WorldStatus,
};
use tessera_net::model::{CharacterSummary, sanitize_text};
use tessera_net::{Command, DisconnectReason, Event};

/// No event for this long while the progress screen shows ends the login (§8.3).
pub const WATCHDOG: Duration = Duration::from_secs(60);
/// The queue panel updates at most 4 times a second (§8.4).
pub const QUEUE_REFRESH: Duration = Duration::from_millis(250);
/// Log in stays disabled this long after any failure (§8.6).
pub const FAILURE_WAIT: Duration = Duration::from_secs(2);
/// The `RateLimited` wait without a `retry_after_seconds` (§8.6).
pub const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

/// Which login screen shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginScreen {
    /// The progress panel with its stage text and Cancel.
    Progress {
        /// "Logging in…", "Getting the list of worlds…" or "Joining ⟨world⟩…".
        text: String,
    },
    /// The world list.
    Worlds,
    /// The queue panel.
    Queue {
        /// Its lines (§8.4).
        lines: Vec<String>,
    },
}

/// One row of the world list (§8.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldRow {
    /// The backend's number.
    pub id: WorldId,
    /// Name (sanitised).
    pub name: String,
    /// Status text.
    pub status: &'static str,
    /// Load text (empty without one).
    pub load: &'static str,
    /// Note (sanitised, empty without one).
    pub note: String,
    /// Whether it can be chosen (`Closed` and `Restricted` are ghosted).
    pub choosable: bool,
}

/// A message panel with an OK button (§8.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Tessera's text.
    pub text: String,
    /// "The service said: ⟨text⟩", when the service gave a detail.
    pub detail: Option<String>,
}

/// Where the flow goes when the login ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The account form; with `keep_account` refilled with the account name and the
    /// password focused (`BadCredentials`).
    AccountForm {
        /// Keep the account name.
        keep_account: bool,
    },
    /// The start screen.
    StartScreen,
    /// The character screens with the session's character list (§8.7).
    CharacterScreens(Vec<CharacterSummary>),
}

/// How the login ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginEnd {
    /// Where to go.
    pub target: Target,
    /// The message to show there.
    pub message: Option<Message>,
    /// How long Log in stays disabled (§8.6).
    pub wait: Duration,
    /// The wait is a `RateLimited` one (the form shows a countdown).
    pub rate_limited: bool,
    /// The session should be dropped after a `Disconnect` was sent (watchdog, Cancel, Back).
    pub drop_session: bool,
}

/// The login screens' state (module docs).
#[derive(Debug)]
pub struct LoginPresenter {
    screen: LoginScreen,
    rows: Vec<WorldRow>,
    selected: Option<usize>,
    /// The display name of the last world chosen (`[Official] last_world`, §8.5).
    last_world: Option<String>,
    /// The world chosen in this login.
    chosen: Option<(WorldId, String)>,
    /// A world name to remember (taken by the flow, which writes the settings).
    remember: Option<String>,
    /// A world-list message (a refusal) while the session stays.
    message: Option<Message>,
    last_event: Instant,
    queue_shown: Option<Instant>,
    queue_pending: Option<Vec<String>>,
    refused: Option<(LoginRefusal, Option<u32>, Option<String>)>,
    /// The user left the queue: a `Disconnected { ClientRequested }` ends at the form.
    leaving_queue: bool,
    end: Option<LoginEnd>,
    commands: Vec<Command>,
}

fn status_text(s: WorldStatus) -> &'static str {
    match s {
        WorldStatus::Open => text("official.worlds.status.open"),
        WorldStatus::Busy => text("official.worlds.status.busy"),
        WorldStatus::Full => text("official.worlds.status.full"),
        WorldStatus::Closed => text("official.worlds.status.closed"),
        WorldStatus::Restricted => text("official.worlds.status.restricted"),
        _ => text("official.worlds.status.unknown"),
    }
}

fn load_text(l: Option<WorldLoad>) -> &'static str {
    match l {
        Some(WorldLoad::Low) => text("official.worlds.load.low"),
        Some(WorldLoad::Medium) => text("official.worlds.load.medium"),
        Some(WorldLoad::High) => text("official.worlds.load.high"),
        _ => "",
    }
}

/// Service text as shown: `sanitize_text`, cut at `max` bytes.
fn plain(s: &str, max: usize) -> String {
    sanitize_text(s, max).into_owned()
}

fn detail_line(d: &str) -> String {
    fill(
        text("official.error.detail"),
        &[("text", &plain(d, MAX_ACCOUNT_DETAIL_LEN))],
    )
}

/// The queue panel's lines (§8.4).
#[must_use]
pub fn queue_lines(
    world: &str,
    position: Option<u32>,
    length: Option<u32>,
    wait_seconds: Option<u32>,
) -> Vec<String> {
    let mut lines = vec![fill(text("official.queue.waiting"), &[("world", world)])];
    if let Some(p) = position {
        let p = p.to_string();
        lines.push(match length {
            Some(l) => fill(
                text("official.queue.position_of"),
                &[("position", &p), ("length", &l.to_string())],
            ),
            None => fill(text("official.queue.position"), &[("position", &p)]),
        });
    }
    if let Some(w) = wait_seconds {
        lines.push(if w < 60 {
            text("official.queue.wait_short").to_owned()
        } else {
            fill(
                text("official.queue.wait"),
                &[("minutes", &w.div_ceil(60).to_string())],
            )
        });
    }
    if position.is_none() && length.is_none() && wait_seconds.is_none() {
        lines.push(text("official.queue.unknown").to_owned());
    }
    lines
}

impl LoginPresenter {
    /// A login just started (`Connect` sent at `now`); `last_world` from the settings.
    #[must_use]
    pub fn new(now: Instant, last_world: Option<String>) -> Self {
        Self {
            screen: LoginScreen::Progress {
                text: text("official.login.authenticating").to_owned(),
            },
            rows: Vec::new(),
            selected: None,
            last_world: last_world.map(|w| plain(&w, MAX_WORLD_NAME_LEN)),
            chosen: None,
            remember: None,
            message: None,
            last_event: now,
            queue_shown: None,
            queue_pending: None,
            refused: None,
            leaving_queue: false,
            end: None,
            commands: Vec::new(),
        }
    }

    /// The screen that shows.
    #[must_use]
    pub fn screen(&self) -> &LoginScreen {
        &self.screen
    }

    /// The world list's rows, in the backend's order.
    #[must_use]
    pub fn rows(&self) -> &[WorldRow] {
        &self.rows
    }

    /// The selected row.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The world list's message (a refusal), if one shows.
    #[must_use]
    pub fn message(&self) -> Option<&Message> {
        self.message.as_ref()
    }

    /// Dismisses the world list's message (OK).
    pub fn dismiss_message(&mut self) {
        self.message = None;
    }

    /// How the login ended, once it has.
    #[must_use]
    pub fn end(&self) -> Option<&LoginEnd> {
        self.end.as_ref()
    }

    /// The commands to send now.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    /// The world name to remember, once one was chosen (§8.5).
    pub fn take_remember(&mut self) -> Option<String> {
        self.remember.take()
    }

    fn chosen_name(&self) -> String {
        self.chosen
            .as_ref()
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    }

    fn finish(&mut self, end: LoginEnd) {
        if self.end.is_none() {
            self.end = Some(end);
        }
    }

    /// One event of the session (already checked and cleaned by it).
    pub fn on_event(&mut self, event: &Event, now: Instant) {
        if self.end.is_some() {
            return;
        }
        self.last_event = now;
        match event {
            Event::Account(e) => self.on_account(e, now),
            Event::Connected { characters } => self.finish(LoginEnd {
                target: Target::CharacterScreens(characters.clone()),
                message: None,
                wait: Duration::ZERO,
                rate_limited: false,
                drop_session: false,
            }),
            Event::Disconnected { reason } => self.on_disconnected(reason),
            _ => {}
        }
    }

    fn on_account(&mut self, e: &AccountEvent, now: Instant) {
        match e {
            AccountEvent::LoginProgress { stage } => {
                let t = match stage {
                    LoginStage::Authenticating => text("official.login.authenticating").to_owned(),
                    LoginStage::ListingWorlds => text("official.login.listing").to_owned(),
                    _ => fill(
                        text("official.login.joining"),
                        &[("world", &self.chosen_name())],
                    ),
                };
                self.screen = LoginScreen::Progress { text: t };
            }
            AccountEvent::WorldList { worlds } => {
                let keep = self
                    .selected
                    .and_then(|i| self.rows.get(i))
                    .map(|r| r.name.clone());
                self.rows = worlds
                    .iter()
                    .take(MAX_WORLDS)
                    .map(|w| WorldRow {
                        id: w.id,
                        name: plain(&w.name, MAX_WORLD_NAME_LEN),
                        status: status_text(w.status),
                        load: load_text(w.load),
                        note: w
                            .note
                            .as_deref()
                            .map(|n| plain(n, MAX_WORLD_NOTE_LEN))
                            .unwrap_or_default(),
                        choosable: w.status.can_be_chosen(),
                    })
                    .collect();
                let by_name = |name: &str, rows: &[WorldRow]| {
                    rows.iter().position(|r| r.choosable && r.name == name)
                };
                self.selected = keep
                    .as_deref()
                    .and_then(|n| by_name(n, &self.rows))
                    .or_else(|| {
                        self.last_world
                            .as_deref()
                            .and_then(|n| by_name(n, &self.rows))
                    })
                    .or_else(|| self.rows.iter().position(|r| r.choosable));
                self.screen = LoginScreen::Worlds;
                self.queue_shown = None;
                self.queue_pending = None;
            }
            AccountEvent::LoginQueueChanged {
                position,
                length,
                wait_seconds,
            } => {
                let lines = queue_lines(&self.chosen_name(), *position, *length, *wait_seconds);
                let due = self
                    .queue_shown
                    .is_none_or(|t| now.saturating_duration_since(t) >= QUEUE_REFRESH);
                if due || !matches!(self.screen, LoginScreen::Queue { .. }) {
                    self.screen = LoginScreen::Queue { lines };
                    self.queue_shown = Some(now);
                    self.queue_pending = None;
                } else {
                    self.queue_pending = Some(lines);
                }
            }
            AccountEvent::WorldRefused {
                world,
                reason,
                detail,
            } => {
                let name = self
                    .rows
                    .iter()
                    .find(|r| r.id == *world)
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| self.chosen_name());
                let id = match reason {
                    WorldRefusal::Full => "official.error.world_full",
                    WorldRefusal::Closed => "official.error.world_closed",
                    WorldRefusal::NotAllowed => "official.error.world_not_allowed",
                    _ => "official.error.world_other",
                };
                self.message = Some(Message {
                    text: fill(text(id), &[("world", &name)]),
                    detail: detail.as_deref().map(detail_line),
                });
                self.screen = LoginScreen::Worlds;
                self.queue_shown = None;
                self.queue_pending = None;
            }
            AccountEvent::LoginRefused {
                reason,
                retry_after_seconds,
                detail,
            } => {
                self.refused = Some((*reason, *retry_after_seconds, detail.clone()));
            }
            _ => {}
        }
    }

    fn on_disconnected(&mut self, reason: &DisconnectReason) {
        if let Some((refusal, retry, detail)) = self.refused.take() {
            self.finish(refusal_end(refusal, retry, detail.as_deref()));
            return;
        }
        let form = |message: Option<Message>| LoginEnd {
            target: Target::AccountForm {
                keep_account: false,
            },
            message,
            wait: FAILURE_WAIT,
            rate_limited: false,
            drop_session: false,
        };
        let end = match reason {
            DisconnectReason::ClientRequested => LoginEnd {
                wait: if self.leaving_queue {
                    Duration::ZERO
                } else {
                    FAILURE_WAIT
                },
                ..form(None)
            },
            DisconnectReason::TimedOut => form(Some(Message {
                text: text("official.error.no_answer").to_owned(),
                detail: None,
            })),
            DisconnectReason::ServerClosed(why) => form(Some(Message {
                text: text("official.error.server_closed").to_owned(),
                detail: (!why.trim().is_empty()).then(|| detail_line(why)),
            })),
            // `TransportFailed` and any later reason.
            _ => form(Some(Message {
                text: text("official.error.transport").to_owned(),
                detail: None,
            })),
        };
        self.finish(end);
    }

    /// Advances the clock: the watchdog (§8.3) and a held queue update (§8.4).
    pub fn tick(&mut self, now: Instant) {
        if self.end.is_some() {
            return;
        }
        if matches!(self.screen, LoginScreen::Progress { .. })
            && now.saturating_duration_since(self.last_event) >= WATCHDOG
        {
            self.commands.push(Command::Disconnect);
            self.finish(LoginEnd {
                target: Target::AccountForm {
                    keep_account: false,
                },
                message: Some(Message {
                    text: text("official.error.no_answer").to_owned(),
                    detail: None,
                }),
                wait: FAILURE_WAIT,
                rate_limited: false,
                drop_session: true,
            });
            return;
        }
        if let (Some(lines), Some(shown)) = (self.queue_pending.as_ref(), self.queue_shown)
            && now.saturating_duration_since(shown) >= QUEUE_REFRESH
        {
            self.screen = LoginScreen::Queue {
                lines: lines.clone(),
            };
            self.queue_shown = Some(now);
            self.queue_pending = None;
        }
    }

    /// Moves the world-list selection by `delta` rows among the rows that can be chosen.
    pub fn move_selection(&mut self, delta: i32) {
        if !matches!(self.screen, LoginScreen::Worlds) || self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as i64;
        let mut i = self.selected.map_or(-1, |s| s as i64);
        for _ in 0..n {
            i = (i + i64::from(delta.signum())).rem_euclid(n);
            if self.rows.get(i as usize).is_some_and(|r| r.choosable) {
                self.selected = Some(i as usize);
                return;
            }
        }
    }

    /// Selects row `i` when it can be chosen (a click).
    pub fn select(&mut self, i: usize) {
        if self.rows.get(i).is_some_and(|r| r.choosable) {
            self.selected = Some(i);
        }
    }

    /// Join (or a double click): sends `ChooseWorld` for the selected row (§8.3).
    pub fn join(&mut self, now: Instant) {
        if !matches!(self.screen, LoginScreen::Worlds) || self.message.is_some() {
            return;
        }
        let Some(row) = self.selected.and_then(|i| self.rows.get(i)) else {
            return;
        };
        if !row.choosable {
            return;
        }
        let (id, name) = (row.id, row.name.clone());
        self.commands
            .push(Command::Account(AccountCommand::ChooseWorld { world: id }));
        self.screen = LoginScreen::Progress {
            text: fill(text("official.login.joining"), &[("world", &name)]),
        };
        self.remember = Some(name.clone());
        self.last_world = Some(name.clone());
        self.chosen = Some((id, name));
        self.last_event = now;
    }

    /// Cancel on the progress screen or the queue panel, or Back on the world list: sends
    /// `Disconnect` and ends at the account form.
    pub fn cancel(&mut self) {
        if self.end.is_some() {
            return;
        }
        self.commands.push(Command::Disconnect);
        self.finish(LoginEnd {
            target: Target::AccountForm {
                keep_account: false,
            },
            message: None,
            wait: Duration::ZERO,
            rate_limited: false,
            drop_session: true,
        });
    }

    /// "Leave queue": sends `LeaveLoginQueue`; a new world list or the session's end follows.
    pub fn leave_queue(&mut self, now: Instant) {
        if matches!(self.screen, LoginScreen::Queue { .. }) && self.end.is_none() {
            self.commands
                .push(Command::Account(AccountCommand::LeaveLoginQueue));
            self.leaving_queue = true;
            self.screen = LoginScreen::Progress {
                text: text("official.login.listing").to_owned(),
            };
            self.last_event = now;
        }
    }
}

/// The end of a login after `LoginRefused` (§8.6).
#[must_use]
pub fn refusal_end(reason: LoginRefusal, retry: Option<u32>, detail: Option<&str>) -> LoginEnd {
    let (id, target) = match reason {
        LoginRefusal::BadCredentials => (
            "official.error.bad_credentials",
            Target::AccountForm { keep_account: true },
        ),
        LoginRefusal::AccountNotAllowed => (
            "official.error.account_not_allowed",
            Target::AccountForm {
                keep_account: false,
            },
        ),
        LoginRefusal::UnsupportedStep => ("official.error.unsupported_step", Target::StartScreen),
        LoginRefusal::ClientNotAccepted => {
            ("official.error.client_not_accepted", Target::StartScreen)
        }
        LoginRefusal::RateLimited => (
            "official.error.rate_limited",
            Target::AccountForm {
                keep_account: false,
            },
        ),
        LoginRefusal::ServiceUnavailable => (
            "official.error.service_unavailable",
            Target::AccountForm {
                keep_account: false,
            },
        ),
        LoginRefusal::NotAvailable => ("official.not_available", Target::StartScreen),
        _ => (
            "official.error.other",
            Target::AccountForm {
                keep_account: false,
            },
        ),
    };
    let rate_limited = reason == LoginRefusal::RateLimited;
    let wait = if rate_limited {
        retry.map_or(DEFAULT_RATE_LIMIT_WAIT, |s| {
            Duration::from_secs(u64::from(s))
        })
    } else {
        FAILURE_WAIT
    };
    let msg_text = if rate_limited {
        fill(text(id), &[("seconds", &wait.as_secs().to_string())])
    } else {
        text(id).to_owned()
    };
    let shows_detail = !matches!(
        reason,
        LoginRefusal::BadCredentials | LoginRefusal::NotAvailable
    );
    LoginEnd {
        target,
        message: Some(Message {
            text: msg_text,
            detail: detail
                .filter(|d| shows_detail && !d.trim().is_empty())
                .map(detail_line),
        }),
        wait: wait.max(FAILURE_WAIT),
        rate_limited,
        drop_session: false,
    }
}
