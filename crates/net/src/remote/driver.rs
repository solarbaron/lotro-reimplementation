// SPDX-License-Identifier: GPL-3.0-or-later
//! The network side of the test-server backend, without I/O.
//!
//! [`Driver`] wraps the protocol's client state machine
//! ([`ClientSession`]): it feeds it received bytes, logs in once the server greeted, turns
//! its events into [`Inbound`] items for the main thread, schedules `MoveInput`, time and
//! ping requests on a 30 Hz tick, and maps every way the connection can end to one
//! [`Inbound::Closed`]. The I/O loop (`link.rs`) and the fuzzing harness
//! (`harness.rs`) both drive it, so the fuzzed code is the code that runs.

use std::collections::VecDeque;
use std::time::Duration;

use tessera_protocol::client::{ClientConfig, ClientEvent, ClientSession, ClientState, CloseCause};
use tessera_protocol::codes::{AppErrorCode, ClientFailure};
use tessera_protocol::frame::StreamKind;
use tessera_protocol::limits::{DEFAULT_TICK_RATE, STANDING_SPEED};
use tessera_protocol::message::{
    ChatChannel, ChatMessage, Credentials, EntityAppearance, EntityCreate, EntityUpdate,
    LocationCause, LocationReport, MotionEntry, NoticeLevel, WorldClock, WorldEnter,
};
use tessera_protocol::movement::MoveScheduler;
use tessera_protocol::session::{
    ConnectionEnd, PeerDatagrams, StreamEnd, StreamRef, Transmit, Violation,
};
use tessera_protocol::types::{Appearance, Destination, Location, Motion, Vec3};
use tessera_protocol::value::Stance;

use super::stats::{RemoteFailure, RemoteStats, ServerInfo};
use crate::model::DisconnectReason;

/// Simulation tick of the client's movement schedule (§8.2: 30 Hz).
pub(crate) const TICK: Duration = Duration::from_micros(1_000_000 / DEFAULT_TICK_RATE as u64);
/// Interval between application pings (§6.2.18 allows one per second).
const PING_INTERVAL: u64 = 2_000_000;

/// A decoded, validated item for the main thread.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Inbound {
    /// `HelloAck`: what the server said about itself.
    Greeted(ServerInfo),
    /// Logged in.
    Connected { characters: Vec<String> },
    /// `WorldEnter`.
    WorldEntered(Box<WorldEnter>),
    /// `LocationReport`.
    Location(Box<LocationReport>),
    /// A line to show as system chat.
    System(String),
    /// `EntityCreate`.
    Created(Box<EntityCreate>),
    /// `EntityUpdate`.
    Updated(EntityUpdate),
    /// `EntityAppearance`.
    Appearance(Box<EntityAppearance>),
    /// `EntityDestroy`.
    Destroyed(u64),
    /// `InterestReset` with the IDs it destroyed.
    Reset(Vec<u64>),
    /// `WorldClock`.
    Clock(WorldClock),
    /// `ChatMessage`.
    Chat(Box<ChatMessage>),
    /// The fresh entries of a `MotionBatch`.
    Motion {
        server_time: u64,
        entries: Vec<MotionEntry>,
    },
    /// The session ended. Always the last item.
    Closed {
        reason: DisconnectReason,
        failure: Option<RemoteFailure>,
    },
}

impl Inbound {
    /// Movement may be dropped when the main thread lags; nothing else may.
    pub(crate) fn droppable(&self) -> bool {
        matches!(self, Self::Motion { .. })
    }
}

/// A command for the network thread.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NetCommand {
    EnterWorld(String),
    Teleport(Destination),
    QueryLocation,
    Chat(ChatChannel, String),
    Emote { anim: u32, looping: bool },
    Stance(Stance),
    Appearance(Box<Appearance>),
}

/// The player's movement as the main thread last reported it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MoveSlot {
    /// The position epoch the main thread had adopted when it sent this move. Moves of an
    /// older epoch (sent before the client saw a teleport or correction) are ignored.
    pub epoch: u32,
    pub location: Location,
    pub heading: f32,
    pub velocity: Vec3,
    pub motion: Motion,
}

/// The network side of one connection; see the module documentation.
#[derive(Debug)]
pub(crate) struct Driver {
    session: ClientSession,
    credentials: Option<Credentials>,
    register: bool,
    pub(crate) pending: VecDeque<Inbound>,
    pub(crate) stats: RemoteStats,
    closed: bool,
    scheduler: MoveScheduler,
    tick: u64,
    current: Option<MoveSlot>,
    last_sent_heading: Option<f32>,
    forced: bool,
    next_ping: u64,
}

impl Driver {
    pub(crate) fn new(client: ClientConfig, credentials: Credentials, register: bool) -> Self {
        Self {
            session: ClientSession::new(client),
            credentials: Some(credentials),
            register,
            pending: VecDeque::new(),
            stats: RemoteStats {
                state: ClientState::Idle.name(),
                ..RemoteStats::default()
            },
            closed: false,
            scheduler: MoveScheduler::new(),
            tick: 0,
            current: None,
            last_sent_heading: None,
            forced: false,
            next_ping: 0,
        }
    }

    /// Whether the end of the session was reported ([`Inbound::Closed`] queued).
    pub(crate) fn closed(&self) -> bool {
        self.closed
    }

    /// The next thing the I/O layer must do.
    pub(crate) fn poll_transmit(&mut self) -> Option<Transmit> {
        let t = self.session.poll_transmit();
        if let Some(Transmit::Message(m)) = &t
            && m.message_type() == tessera_protocol::MessageType::MoveInput
        {
            self.stats.moves_sent += 1;
        }
        t
    }

    // --- lifecycle ---

    /// The QUIC handshake starts.
    pub(crate) fn connect(&mut self, now: u64) {
        if self.session.connect(now).is_err() {
            self.fail_local("connect in the wrong state");
        }
    }

    /// The QUIC/TLS handshake succeeded and the key check passed: sends `Hello`.
    pub(crate) fn handshake_complete(&mut self, now: u64, datagrams: PeerDatagrams) {
        match self.session.handshake_complete(now, datagrams) {
            Ok(Some(e)) => self.on_event(e, now),
            Ok(None) => {}
            Err(e) => self.fail_local(&format!("cannot send Hello: {e}")),
        }
        self.refresh_state();
    }

    /// The handshake failed locally (§14.2).
    pub(crate) fn handshake_failed(
        &mut self,
        failure: ClientFailure,
        presented: Option<tessera_protocol::fingerprint::Fingerprint>,
        detail: &str,
    ) {
        let _ = self.session.handshake_failed(failure);
        let text = if detail.is_empty() {
            failure.name().to_owned()
        } else {
            format!("{}: {detail}", failure.name())
        };
        let reason = if failure == ClientFailure::Timeout {
            DisconnectReason::TimedOut
        } else {
            DisconnectReason::TransportFailed(text)
        };
        self.report_closed(reason, Some(RemoteFailure::Connect { failure, presented }));
        self.refresh_state();
    }

    /// The QUIC connection is gone.
    pub(crate) fn connection_closed(&mut self, end: ConnectionEnd) {
        if let Some(e) = self.session.connection_closed(end.clone()) {
            self.on_event(e, 0);
        } else if !self.closed {
            // The session had already ended (for example a violation it acted on) but
            // nothing was reported yet: report the transport's view.
            let (reason, failure) = close_reason_for_end(&end);
            self.report_closed(reason, failure);
        }
        self.refresh_state();
    }

    /// The main thread is gone or asked to disconnect: `Disconnect(NO_ERROR)` (§12.3).
    pub(crate) fn disconnect(&mut self, now: u64) {
        if let Some(e) = self.session.disconnect(AppErrorCode::NoError, "", now) {
            self.on_event(e, now);
        }
        self.refresh_state();
    }

    // --- input ---

    pub(crate) fn on_control(&mut self, data: &[u8], now: u64) {
        let r = self.session.on_control_data(data, now);
        self.absorb(r, now);
    }

    /// A server-initiated unidirectional stream announced its kind byte.
    pub(crate) fn on_uni_stream(&mut self, kind_byte: u8) -> Option<StreamKind> {
        let r = self.session.on_uni_stream(kind_byte);
        self.refresh_state();
        match r {
            Ok(k) => k,
            Err(v) => {
                self.violation(&v);
                None
            }
        }
    }

    pub(crate) fn on_stream(&mut self, kind: StreamKind, data: &[u8], now: u64) {
        let r = self.session.on_stream_data(kind, data, now);
        self.absorb(r, now);
    }

    pub(crate) fn on_stream_end(&mut self, stream: StreamRef, end: StreamEnd) {
        if let Err(v) = self.session.on_stream_end(stream, end) {
            self.violation(&v);
        }
        self.refresh_state();
    }

    pub(crate) fn on_datagram(&mut self, data: &[u8], now: u64) {
        self.stats.datagrams_received += 1;
        match self.session.on_datagram(data, now) {
            Ok(Some(e)) => self.on_event(e, now),
            Ok(None) => {}
            Err(v) => self.violation(&v),
        }
        self.refresh_state();
    }

    /// Whether the I/O layer may read the stream of `kind` now (§2.3 rule 5).
    pub(crate) fn may_read(&self, kind: StreamKind) -> bool {
        self.session.may_read(kind)
    }

    fn absorb(&mut self, r: Result<Vec<ClientEvent>, Violation>, now: u64) {
        match r {
            Ok(events) => {
                for e in events {
                    self.on_event(e, now);
                }
            }
            Err(v) => self.violation(&v),
        }
        self.refresh_state();
    }

    // --- commands ---

    pub(crate) fn command(&mut self, cmd: NetCommand, now: u64) {
        let r = match cmd {
            NetCommand::EnterWorld(name) => self.session.enter_world(name, now),
            NetCommand::Teleport(d) => self.session.teleport(d).map(|_| ()),
            NetCommand::QueryLocation => self.session.query_location().map(|_| ()),
            NetCommand::Chat(channel, text) => self.session.chat(channel, text),
            NetCommand::Emote { anim, looping } => self.session.emote(anim, looping),
            NetCommand::Stance(s) => self.session.stance(s),
            NetCommand::Appearance(a) => self.session.set_appearance(*a),
        };
        if let Err(e) = r {
            // The main thread validated the command against its own view of the state;
            // the network may have moved on (for example a close in flight). Not fatal.
            tracing::debug!(error = %e, "command not sent");
        }
        self.refresh_state();
    }

    /// One 30 Hz step at client time `now`: state timeouts, time sync, ping, and the
    /// `MoveInput` schedule (§8.2) with `latest`, the main thread's newest move.
    pub(crate) fn tick(&mut self, now: u64, latest: Option<MoveSlot>) {
        if let Some(e) = self.session.handle_timeout(now) {
            self.on_event(e, now);
        }
        let state = self.session.state();
        if matches!(
            state,
            ClientState::Authenticating
                | ClientState::CharacterSelect
                | ClientState::EnteringWorld
                | ClientState::InWorld
        ) {
            if self.session.time_request_due(now) {
                let _ = self.session.time_request(now);
            }
            if now >= self.next_ping && self.session.ping(now).is_ok() {
                self.next_ping = now.saturating_add(PING_INTERVAL);
            }
            let _ = self.session.server_time(now);
            self.stats.clock_offset_us = self.session.clock().offset();
            self.stats.clock_delay_us = self.session.clock().best_delay();
        }
        if state == ClientState::InWorld {
            self.schedule_move(now, latest);
        }
        self.tick = self.tick.wrapping_add(1);
        self.refresh_state();
    }

    fn schedule_move(&mut self, now: u64, latest: Option<MoveSlot>) {
        let epoch = self.session.epoch().unwrap_or(0);
        if let Some(m) = latest
            && m.epoch == epoch
            && m.location.region == self.session.region().unwrap_or(m.location.region)
        {
            self.current = Some(m);
        }
        let Some(m) = self.current else {
            return;
        };
        let turning = self
            .last_sent_heading
            .is_some_and(|h| (h - m.heading).abs() > 1e-4);
        // §8.2 "while the avatar moves or turns": vertical motion (a jump in place, a fall,
        // flying up or down) is motion too, though it leaves the gait Standing.
        let speed = m.velocity.horizontal_length().hypot(m.velocity.z);
        let moving = speed >= STANDING_SPEED || turning;
        let forced = std::mem::take(&mut self.forced);
        if self.scheduler.on_tick(self.tick, moving, m.motion, forced) {
            self.send_move(&m, now);
        }
    }

    fn send_move(&mut self, m: &MoveSlot, now: u64) {
        match self
            .session
            .move_input(m.location, m.heading, m.velocity, m.motion, now)
        {
            Ok(()) => self.last_sent_heading = Some(m.heading),
            Err(e) => tracing::debug!(error = %e, "MoveInput not sent"),
        }
    }

    // --- events ---

    fn on_event(&mut self, e: ClientEvent, now: u64) {
        match e {
            ClientEvent::HelloAccepted(ack) => {
                let info = ServerInfo::from(&ack);
                self.stats.server = Some(info.clone());
                self.push(Inbound::Greeted(info));
                self.login(now);
            }
            ClientEvent::LoggedIn { characters, .. } => {
                self.forget_credentials();
                self.push(Inbound::Connected {
                    characters: characters.into_iter().map(|c| c.name).collect(),
                });
            }
            ClientEvent::LoginFailed(f) => {
                let text = with_detail(&format!("login failed: {}", f.code.name()), &f.text);
                self.end_with(
                    now,
                    DisconnectReason::ServerClosed(text),
                    RemoteFailure::Login {
                        code: f.code,
                        retry_after: f.retry_after,
                    },
                );
            }
            ClientEvent::EnterWorldFailed(f) => {
                let text = with_detail(
                    &format!("cannot enter the world: {}", f.code.name()),
                    &f.text,
                );
                self.push(Inbound::System(text.clone()));
                self.end_with(
                    now,
                    DisconnectReason::ServerClosed(text),
                    RemoteFailure::EnterWorld { code: f.code },
                );
            }
            ClientEvent::WorldEntered(w) => {
                self.current = Some(MoveSlot {
                    epoch: w.epoch,
                    location: w.pose.location,
                    heading: w.pose.heading,
                    velocity: Vec3::default(),
                    motion: Motion::STANDING,
                });
                self.forced = true;
                self.push(Inbound::WorldEntered(Box::new(w)));
            }
            ClientEvent::Location(r) => {
                if matches!(
                    r.cause,
                    LocationCause::Teleported | LocationCause::Corrected
                ) {
                    if r.cause == LocationCause::Corrected {
                        self.stats.corrections += 1;
                    }
                    // §8.4: adopt the pose with zero velocity and send a MoveInput with
                    // the new epoch at once.
                    let m = MoveSlot {
                        epoch: r.epoch,
                        location: r.pose.location,
                        heading: r.pose.heading,
                        velocity: Vec3::default(),
                        motion: Motion::STANDING,
                    };
                    self.current = Some(m);
                    self.send_move(&m, now);
                }
                self.push(Inbound::Location(Box::new(r)));
            }
            ClientEvent::TeleportRefused(t) => {
                self.push(Inbound::System(with_detail(
                    &format!("teleport refused: {}", t.code.name()),
                    &t.text,
                )));
            }
            ClientEvent::EntityCreated(c) => self.push(Inbound::Created(Box::new(c))),
            ClientEvent::EntityUpdated(u) => self.push(Inbound::Updated(u)),
            ClientEvent::AppearanceChanged(a) => self.push(Inbound::Appearance(Box::new(a))),
            ClientEvent::EntityDestroyed(d) => self.push(Inbound::Destroyed(d.id)),
            ClientEvent::InterestReset(ids) => self.push(Inbound::Reset(ids)),
            ClientEvent::WorldClock(c) => self.push(Inbound::Clock(c)),
            ClientEvent::Chat(m) => self.push(Inbound::Chat(Box::new(m))),
            ClientEvent::Notice(n) => {
                let text = match n.level {
                    NoticeLevel::Info => n.text,
                    NoticeLevel::Warning => format!("warning: {}", n.text),
                    NoticeLevel::Shutdown => {
                        format!("server stopping in {} s: {}", n.seconds, n.text)
                    }
                };
                self.push(Inbound::System(text));
            }
            ClientEvent::ChatRejected(f) => {
                self.push(Inbound::System(with_detail(
                    &format!("chat not sent: {}", f.code.name()),
                    &f.text,
                )));
            }
            ClientEvent::Motion {
                server_time,
                entries,
            } => {
                self.stats.motion_batches += 1;
                self.push(Inbound::Motion {
                    server_time,
                    entries,
                });
            }
            ClientEvent::TimeSample(_) => self.stats.time_samples += 1,
            ClientEvent::Pong { rtt, .. } => {
                self.stats.ping_rtt = Some(Duration::from_micros(rtt));
            }
            ClientEvent::Closed(cause) => {
                let (reason, failure) = close_reason(&cause);
                self.report_closed(reason, failure);
            }
            other => tracing::debug!(event = ?other, "client event ignored"),
        }
    }

    fn login(&mut self, now: u64) {
        // Taken, not cloned: the backend keeps no copy of the password once it was handed
        // to the protocol layer for the wire.
        let Some(credentials) = self.credentials.take() else {
            return;
        };
        let r = if self.register {
            self.session.register(credentials, now)
        } else {
            self.session.login(credentials, now)
        };
        if let Err(e) = r {
            let text = format!("cannot log in: {e}");
            self.end_with(
                now,
                DisconnectReason::TransportFailed(text.clone()),
                RemoteFailure::Transport(text),
            );
        }
    }

    /// Sends `Disconnect(NO_ERROR)` and reports `reason` (instead of the session's
    /// "client requested").
    fn end_with(&mut self, now: u64, reason: DisconnectReason, failure: RemoteFailure) {
        self.report_closed(reason, Some(failure));
        let _ = self.session.disconnect(AppErrorCode::NoError, "", now);
    }

    fn violation(&mut self, v: &Violation) {
        let text = format!("protocol violation by the server: {v}");
        self.report_closed(
            DisconnectReason::TransportFailed(text.clone()),
            Some(RemoteFailure::Violation(v.to_string())),
        );
    }

    fn fail_local(&mut self, why: &str) {
        let _ = self.session.disconnect(AppErrorCode::InternalError, why, 0);
        self.report_closed(
            DisconnectReason::TransportFailed(why.to_owned()),
            Some(RemoteFailure::Transport(why.to_owned())),
        );
    }

    fn report_closed(&mut self, reason: DisconnectReason, failure: Option<RemoteFailure>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.forget_credentials();
        self.pending.push_back(Inbound::Closed { reason, failure });
    }

    /// Drops the credentials if they were never used, overwriting the password first (best
    /// effort, as [`Password`](super::Password) does).
    fn forget_credentials(&mut self) {
        if let Some(c) = self.credentials.take() {
            let mut bytes = c.password.into_bytes();
            bytes.fill(0);
            std::hint::black_box(&bytes);
        }
    }

    fn push(&mut self, item: Inbound) {
        if self.closed {
            return;
        }
        self.pending.push_back(item);
        self.stats.pending_peak = self.stats.pending_peak.max(self.pending.len());
    }

    fn refresh_state(&mut self) {
        self.stats.state = self.session.state().name();
        self.stats.pending = self.pending.len();
        self.stats.protocol_dropped = self.session.dropped();
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.forget_credentials();
    }
}

fn with_detail(head: &str, detail: &str) -> String {
    if detail.is_empty() {
        head.to_owned()
    } else {
        format!("{head}: {detail}")
    }
}

/// The reason and failure of a session's end (Appendix A).
fn close_reason(cause: &CloseCause) -> (DisconnectReason, Option<RemoteFailure>) {
    match cause {
        CloseCause::ServerDisconnect { code, text } => (
            DisconnectReason::ServerClosed(with_detail(code.effective().name(), text)),
            Some(RemoteFailure::Server { code: *code }),
        ),
        CloseCause::LocalDisconnect { .. } => (DisconnectReason::ClientRequested, None),
        CloseCause::PeerClosed { code, reason } => (
            DisconnectReason::ServerClosed(with_detail(code.effective().name(), reason)),
            Some(RemoteFailure::Server { code: *code }),
        ),
        CloseCause::Transport(detail) => (
            DisconnectReason::TransportFailed(detail.clone()),
            Some(RemoteFailure::Transport(detail.clone())),
        ),
        CloseCause::Violation(v) => (
            DisconnectReason::TransportFailed(format!("protocol violation by the server: {v}")),
            Some(RemoteFailure::Violation(v.to_string())),
        ),
        CloseCause::Failure(f) => (
            if *f == ClientFailure::Timeout {
                DisconnectReason::TimedOut
            } else {
                DisconnectReason::TransportFailed(f.name().to_owned())
            },
            Some(RemoteFailure::Connect {
                failure: *f,
                presented: None,
            }),
        ),
    }
}

fn close_reason_for_end(end: &ConnectionEnd) -> (DisconnectReason, Option<RemoteFailure>) {
    match end {
        ConnectionEnd::PeerClosed { code, reason } => close_reason(&CloseCause::PeerClosed {
            code: *code,
            reason: tessera_protocol::session::close_reason(&tessera_protocol::text::sanitise(
                reason,
            )),
        }),
        ConnectionEnd::IdleTimeout => (DisconnectReason::TimedOut, None),
        ConnectionEnd::Transport(d) => close_reason(&CloseCause::Transport(d.clone())),
        ConnectionEnd::Local => (DisconnectReason::ClientRequested, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_protocol::frame::frame_to_vec;
    use tessera_protocol::message::{HelloAck, LoginOk, Message, ServerCaps};

    fn driver() -> Driver {
        let mut d = Driver::new(
            ClientConfig::default(),
            Credentials {
                account: "alice".into(),
                password: "pw".into(),
            },
            false,
        );
        d.connect(0);
        d.handshake_complete(1, PeerDatagrams::MaxPayload(1200));
        d
    }

    fn hello_ack() -> Vec<u8> {
        frame_to_vec(&Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "t".into(),
            motd: String::new(),
        }))
        .unwrap()
    }

    #[test]
    fn logs_in_after_hello_ack() {
        let mut d = driver();
        let mut sent = Vec::new();
        while let Some(t) = d.poll_transmit() {
            sent.push(t);
        }
        assert!(matches!(&sent[..], [Transmit::Message(Message::Hello(_))]));
        d.on_control(&hello_ack(), 2);
        let t = d.poll_transmit().unwrap();
        assert!(matches!(t, Transmit::Message(Message::Login(_))));
        let ok = frame_to_vec(&Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![],
        }))
        .unwrap();
        d.on_control(&ok, 3);
        assert!(matches!(d.pending.pop_front(), Some(Inbound::Greeted(_))));
        assert_eq!(
            d.pending.pop_front(),
            Some(Inbound::Connected { characters: vec![] })
        );
        assert!(d.credentials.is_none(), "credentials forgotten after login");
    }

    #[test]
    fn garbage_closes_once() {
        let mut d = driver();
        d.on_control(&[1, 0, 0, 0, 1], 2);
        assert!(d.closed());
        d.on_control(&[1, 0, 0, 0, 1], 3);
        d.connection_closed(ConnectionEnd::IdleTimeout);
        let closed = d
            .pending
            .iter()
            .filter(|i| matches!(i, Inbound::Closed { .. }))
            .count();
        assert_eq!(closed, 1);
        match d.pending.back() {
            Some(Inbound::Closed {
                reason: DisconnectReason::TransportFailed(t),
                failure: Some(RemoteFailure::Violation(_)),
            }) => assert!(t.contains("violation")),
            other => panic!("{other:?}"),
        }
    }
}
