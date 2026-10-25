// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat-server session's login gate, heartbeat and retry policies
//! (`docs/specs/official-messages-play.md` §6.6.2–§6.6.3), without I/O: the driver feeds
//! times and transport events and carries out the returned [`ChatAction`]s.

/// Idle time without any received event before a heartbeat is sent (§6.6.3).
pub const HEARTBEAT_INTERVAL_SEC: f64 = 15.0;
/// Idle time, with a heartbeat outstanding, before the session is declared dead (§6.6.3).
pub const DROP_TIMEOUT_SEC: f64 = 60.0;
/// Time to wait for `0x165` after the transport connected. The spec says only "or the
/// connection attempt times out"; the session handshake timeout of `official-session.md`
/// §6.1 (20 s) is reused (engineering choice).
pub const LOGIN_STATUS_TIMEOUT_SEC: f64 = 20.0;

/// A connection retry policy (§6.6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetryPolicy {
    /// Delay before the first attempt of the session, seconds.
    pub initial_delay_sec: f64,
    /// Most attempts (`u32::MAX`: unlimited).
    pub max_retries: u32,
    /// Delay before each further attempt, seconds.
    pub retry_interval_sec: f64,
}

/// Until the first successful login: 1 s, unlimited, every 30 s.
pub const INITIAL_CONNECT_POLICY: RetryPolicy = RetryPolicy {
    initial_delay_sec: 1.0,
    max_retries: u32::MAX,
    retry_interval_sec: 30.0,
};

/// After a successful login: 1 s, unlimited, every 10 s.
pub const RECONNECT_POLICY: RetryPolicy = RetryPolicy {
    initial_delay_sec: 1.0,
    max_retries: u32::MAX,
    retry_interval_sec: 10.0,
};

/// The session's phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChatPhase {
    /// Waiting for the next connection attempt at `until`.
    Waiting {
        /// When to connect.
        until: f64,
    },
    /// The transport handshake is running.
    Connecting,
    /// The transport is connected (State 5); waiting for `0x165` since `since`.
    AwaitingLogin {
        /// When the transport connected.
        since: f64,
    },
    /// Logged in: `is_connected = 1`, `is_ready = 1`.
    Ready,
    /// Closed by the client, or out of retries.
    Closed,
}

/// What the driver must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    /// Start a connection attempt (a fresh session connection with the auth header).
    Connect,
    /// Send `0x17E` (`chat_heartbeat_req`).
    SendHeartbeat,
    /// Tear the transport down; a reconnection is scheduled.
    Disconnect,
}

/// The state machine of §6.6.2–§6.6.3.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSessionState {
    phase: ChatPhase,
    logged_in_once: bool,
    last_recv: f64,
    heartbeat_sent: bool,
    attempts: u32,
}

impl ChatSessionState {
    /// A session created at `now`; the first attempt follows after the initial delay.
    #[must_use]
    pub fn new(now: f64) -> Self {
        Self {
            phase: ChatPhase::Waiting {
                until: now + INITIAL_CONNECT_POLICY.initial_delay_sec,
            },
            logged_in_once: false,
            last_recv: now,
            heartbeat_sent: false,
            attempts: 0,
        }
    }

    /// The phase.
    #[must_use]
    pub fn phase(&self) -> ChatPhase {
        self.phase
    }

    /// Whether the chat server accepted the login (`is_ready = 1`).
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.phase == ChatPhase::Ready
    }

    /// Whether a heartbeat is outstanding (`heartbeat_sent = 1`).
    #[must_use]
    pub fn heartbeat_sent(&self) -> bool {
        self.heartbeat_sent
    }

    /// The active retry policy.
    #[must_use]
    pub fn policy(&self) -> RetryPolicy {
        if self.logged_in_once {
            RECONNECT_POLICY
        } else {
            INITIAL_CONNECT_POLICY
        }
    }

    fn schedule_retry(&mut self, now: f64) {
        self.heartbeat_sent = false;
        let policy = self.policy();
        self.phase = if self.attempts >= policy.max_retries {
            ChatPhase::Closed
        } else {
            ChatPhase::Waiting {
                until: now + policy.retry_interval_sec,
            }
        };
    }

    /// Advances the timers.
    pub fn tick(&mut self, now: f64) -> Option<ChatAction> {
        match self.phase {
            ChatPhase::Waiting { until } if now >= until => {
                self.attempts = self.attempts.saturating_add(1);
                self.phase = ChatPhase::Connecting;
                Some(ChatAction::Connect)
            }
            ChatPhase::AwaitingLogin { since } if now - since > LOGIN_STATUS_TIMEOUT_SEC => {
                self.schedule_retry(now);
                Some(ChatAction::Disconnect)
            }
            ChatPhase::Ready => {
                let idle = now - self.last_recv;
                if self.heartbeat_sent && idle > DROP_TIMEOUT_SEC {
                    self.schedule_retry(now);
                    Some(ChatAction::Disconnect)
                } else if !self.heartbeat_sent && idle > HEARTBEAT_INTERVAL_SEC {
                    self.heartbeat_sent = true;
                    Some(ChatAction::SendHeartbeat)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// The transport reached State 5 (`Connected`): wait for `0x165` (§6.6.2 item 3).
    pub fn on_transport_connected(&mut self, now: f64) {
        if self.phase == ChatPhase::Connecting {
            self.phase = ChatPhase::AwaitingLogin { since: now };
        }
    }

    /// Any chat event arrived (§6.6.3 rule 1).
    pub fn on_event(&mut self, now: f64) {
        self.last_recv = now;
        self.heartbeat_sent = false;
    }

    /// `0x165` arrived with `status` (§6.6.2 item 3). Returns
    /// [`ChatAction::Disconnect`] on a refusal.
    pub fn on_login_status(&mut self, status: i32, now: f64) -> Option<ChatAction> {
        if !matches!(self.phase, ChatPhase::AwaitingLogin { .. }) {
            return None;
        }
        if status >= 0 {
            self.phase = ChatPhase::Ready;
            self.logged_in_once = true;
            self.attempts = 0;
            self.last_recv = now;
            self.heartbeat_sent = false;
            None
        } else {
            self.schedule_retry(now);
            Some(ChatAction::Disconnect)
        }
    }

    /// The transport failed or closed: reconnect under the active policy.
    pub fn on_transport_lost(&mut self, now: f64) {
        if matches!(
            self.phase,
            ChatPhase::Connecting | ChatPhase::AwaitingLogin { .. } | ChatPhase::Ready
        ) {
            self.schedule_retry(now);
        }
    }

    /// The client closes the session for good.
    pub fn close(&mut self) {
        self.phase = ChatPhase::Closed;
        self.heartbeat_sent = false;
    }
}
