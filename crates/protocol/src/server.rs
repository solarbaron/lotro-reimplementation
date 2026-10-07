// SPDX-License-Identifier: GPL-3.0-or-later
//! The server's per-connection state machine (`tessera-protocol.md` §12.2), sans I/O.
//!
//! [`ServerSession`] answers `Hello`, enforces the per-connection rules (state, outstanding
//! requests, login attempts, rate limits, timeouts; §4.7, §10.2, §13), answers what needs no
//! world knowledge (`HelloAck`, `Pong`, `TimeReply`, text-class failures) and hands the rest
//! to the server as [`ServerEvent`]s. The server answers those through the session's methods.
//!
//! `now` is server time in microseconds (§11.1) throughout.

use std::time::Duration;

use crate::client::{check_encodable, floor_boundary};
use crate::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode, TeleportRefusedCode,
};
use crate::frame::{FrameDecoder, StreamKind, decode_datagram};
use crate::limits::{
    CHARACTER_SELECT_IDLE, CLOSE_LINGER, CONTROL_RATE_WINDOW, DEFAULT_INTEREST_RADIUS,
    DEFAULT_SERVER_ENTITY_CAP, DEFAULT_SNAPSHOT_RATE, DEFAULT_TICK_RATE, HANDSHAKE_TIMEOUT,
    HELLO_TIMEOUT, LOGIN_TIMEOUT, MAX_CONTROL_PER_SECOND, MAX_DETAIL_TEXT_LEN, MAX_LOGIN_FAILURES,
    MAX_LONG_TEXT_LEN, MAX_MOVE_INPUTS_PER_SECOND, MAX_PINGS_PER_SECOND,
    MAX_TIME_REQUESTS_PER_SECOND, PROTOCOL_MINOR,
};
use crate::message::{
    Channel, CharacterCreate, CharacterList, ChatChannel, ChatSend, Credentials, Disconnect,
    EmoteSet, Failure, Hello, HelloAck, LoginFailed, LoginOk, Message, MessageType, MoveInput,
    ServerCaps, Side, TeleportRefused, TeleportRequest, WorldEnter,
};
use crate::rate::{ChatLimiter, ChatVerdict, SlidingWindow};
use crate::session::{
    ConnectionEnd, Outbox, PeerDatagrams, StreamEnd, StreamRef, Transmit, UsageError, Violation,
    close_reason, micros,
};
use crate::text;
use crate::time::time_reply;
use crate::types::{Appearance, CharacterSummary, Destination};
use crate::value::Stance;
use crate::version::negotiate_minor;

/// Server states, per connection (§12.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerState {
    /// QUIC handshake running.
    Accepting,
    /// Handshake complete; waiting for `Hello`.
    AwaitHello,
    /// `HelloAck` sent; waiting for a successful login.
    AwaitLogin,
    /// `LoginOk` sent.
    CharacterSelect,
    /// `WorldEnter` sent.
    InWorld,
    /// `Disconnect` sent or received.
    Closing,
    /// Connection closed.
    Closed,
}

impl ServerState {
    /// The state's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Accepting => "Accepting",
            Self::AwaitHello => "AwaitHello",
            Self::AwaitLogin => "AwaitLogin",
            Self::CharacterSelect => "CharacterSelect",
            Self::InWorld => "InWorld",
            Self::Closing => "Closing",
            Self::Closed => "Closed",
        }
    }

    fn after_hello_ack(self) -> bool {
        matches!(
            self,
            Self::AwaitLogin | Self::CharacterSelect | Self::InWorld
        )
    }
}

/// What the server states in `HelloAck` and the version range it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerParams {
    /// Lowest minor version accepted (0 for every v1 server today).
    pub min_minor: u16,
    /// Highest minor version implemented.
    pub max_minor: u16,
    /// Capability bits.
    pub caps: ServerCaps,
    /// Simulation rate, Hz (30 in v1).
    pub tick_rate: u8,
    /// `MotionBatch` rate, Hz.
    pub snapshot_rate: u8,
    /// Interest radius *R*, landblocks.
    pub interest_radius: u8,
    /// Display name (`TEXT0`(64)).
    pub server_name: String,
    /// Message of the day (`TEXT0`(1,024)).
    pub motd: String,
}

impl Default for ServerParams {
    fn default() -> Self {
        Self {
            min_minor: 0,
            max_minor: PROTOCOL_MINOR,
            caps: ServerCaps::NONE,
            tick_rate: DEFAULT_TICK_RATE,
            snapshot_rate: DEFAULT_SNAPSHOT_RATE,
            interest_radius: DEFAULT_INTEREST_RADIUS,
            server_name: String::new(),
            motd: String::new(),
        }
    }
}

/// A client request or notification for the server to act on.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ServerEvent {
    /// `Hello` was accepted and `HelloAck` queued.
    Greeted(Hello),
    /// `Login` or `Register` with a valid `ACCOUNT` and a non-empty password. Answer with
    /// [`ServerSession::accept_login`] or [`ServerSession::reject_login`].
    Login {
        /// The credentials (verify against Argon2id; never log the password).
        credentials: Credentials,
        /// True for `Register`.
        register: bool,
    },
    /// `CharacterCreate` with a valid `NAME`. Answer with
    /// [`ServerSession::character_created`] or [`ServerSession::reject_character`].
    CharacterCreate(CharacterCreate),
    /// `EnterWorld` with a valid `NAME`. Answer with [`ServerSession::enter_world`] or
    /// [`ServerSession::reject_enter_world`].
    EnterWorld {
        /// The character.
        name: String,
    },
    /// `TeleportRequest` (a `Named` place name is valid). Answer exactly once on the world
    /// stream. It supersedes an unresolved earlier request: answer that one with
    /// `TeleportRefused(CANCELLED)` (§6.2.12).
    Teleport(TeleportRequest),
    /// `TeleportRequest` with an invalid `Named` place name. The session has already answered
    /// it with `TeleportRefused(INVALID_DESTINATION)`. It still supersedes an unresolved
    /// earlier request: answer that one with `TeleportRefused(CANCELLED)` (§6.2.12).
    TeleportInvalid(TeleportRequest),
    /// `QueryLocation`.
    QueryLocation {
        /// The request ID to echo.
        request_id: u32,
    },
    /// `ChatSend` within the rate limit, with valid `TEXT` and peer `NAME`. Deliver it, or
    /// answer with [`ServerSession::reject_chat`].
    Chat(ChatSend),
    /// `EmoteSet`.
    Emote(EmoteSet),
    /// `StanceSet`.
    Stance(Stance),
    /// `AppearanceSet` (answer every one with `EntityAppearance` for the player).
    Appearance(Appearance),
    /// `MoveInput` while InWorld and within 30 per second; validate with
    /// [`crate::movement::MovementValidator`].
    Move(MoveInput),
    /// `Pong` for the server's `Ping`.
    Pong {
        /// The nonce.
        nonce: u64,
        /// Round-trip time, µs.
        rtt: u64,
    },
    /// The connection is over: the client sent `Disconnect` or closed the connection
    /// (`client` true), or the session or the server ended it. A session reports its end
    /// exactly once: with this event, or with an input that returned a [`Violation`].
    Closed {
        /// The code.
        code: AppErrorCode,
        /// Whether the client initiated it.
        client: bool,
    },
}

/// The server's side of one connection (§12.2).
#[derive(Debug)]
pub struct ServerSession {
    params: ServerParams,
    state: ServerState,
    deadline: Option<u64>,
    login_deadline: Option<u64>,
    outbox: Outbox,
    hello: Option<Hello>,
    minor: u16,
    login_outstanding: bool,
    failures: u32,
    create_outstanding: bool,
    enter_outstanding: bool,
    entered: bool,
    control_rate: SlidingWindow,
    ping_rate: SlidingWindow,
    own_ping_rate: SlidingWindow,
    time_rate: SlidingWindow,
    move_rate: SlidingWindow,
    chat: ChatLimiter,
    ping_outstanding: Option<(u64, u64)>,
    next_nonce: u64,
    control: FrameDecoder,
    close_code: Option<AppErrorCode>,
    /// The end of the session was reported (a `Closed` event or a returned violation).
    reported: bool,
    /// Bytes or messages arrived on the control stream (so a `Disconnect` can be sent on it).
    control_seen: bool,
    dropped: u64,
}

impl ServerSession {
    /// A session for a connection whose QUIC handshake started at `now` (Accepting, 10 s).
    #[must_use]
    pub fn new(params: ServerParams, now: u64) -> Self {
        let control_max = MAX_CONTROL_PER_SECOND
            .saturating_mul(u32::try_from(CONTROL_RATE_WINDOW.as_secs()).unwrap_or(5));
        Self {
            params,
            state: ServerState::Accepting,
            deadline: Some(now.saturating_add(micros(HANDSHAKE_TIMEOUT))),
            login_deadline: None,
            outbox: Outbox::default(),
            hello: None,
            minor: 0,
            login_outstanding: false,
            failures: 0,
            create_outstanding: false,
            enter_outstanding: false,
            entered: false,
            control_rate: SlidingWindow::new(control_max, CONTROL_RATE_WINDOW),
            ping_rate: SlidingWindow::new(MAX_PINGS_PER_SECOND, Duration::from_secs(1)),
            own_ping_rate: SlidingWindow::new(MAX_PINGS_PER_SECOND, Duration::from_secs(1)),
            time_rate: SlidingWindow::new(MAX_TIME_REQUESTS_PER_SECOND, Duration::from_secs(1)),
            move_rate: SlidingWindow::new(MAX_MOVE_INPUTS_PER_SECOND, Duration::from_secs(1)),
            chat: ChatLimiter::new(),
            ping_outstanding: None,
            next_nonce: 1,
            control: FrameDecoder::new(Channel::Control, Side::Client),
            close_code: None,
            reported: false,
            control_seen: false,
            dropped: 0,
        }
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> ServerState {
        self.state
    }

    /// The client's `Hello`, once received.
    #[must_use]
    pub fn hello(&self) -> Option<&Hello> {
        self.hello.as_ref()
    }

    /// The negotiated minor version.
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.minor
    }

    /// The most entities this client accepts at once: `Hello.max_entities`, capped at the
    /// server's default cap of 1,024 (§9.4).
    #[must_use]
    pub fn entity_budget(&self) -> usize {
        self.hello.as_ref().map_or(0, |h| {
            usize::from(h.max_entities).min(DEFAULT_SERVER_ENTITY_CAP)
        })
    }

    /// Messages dropped under §13.2 so far.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The next thing to transmit.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.outbox.pop()
    }

    /// The time (µs) at which [`handle_timeout`](Self::handle_timeout) must be called.
    #[must_use]
    pub fn poll_timeout(&self) -> Option<u64> {
        match (self.deadline, self.login_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The QUIC handshake completed. `datagrams` is what the QUIC stack reports about the
    /// client's datagram support (with quinn: `transport::peer_datagrams`). Moves to
    /// AwaitHello (10 s) and returns `None`, or closes with `DATAGRAMS_REQUIRED` and returns
    /// the close event.
    pub fn handshake_complete(
        &mut self,
        now: u64,
        datagrams: PeerDatagrams,
    ) -> Result<Option<ServerEvent>, UsageError> {
        if self.state != ServerState::Accepting {
            return Err(UsageError::WrongState(self.state.name()));
        }
        if !datagrams.acceptable() {
            self.close_now(AppErrorCode::DatagramsRequired);
            self.close_code = Some(AppErrorCode::DatagramsRequired);
            return Ok(self.closed_event(AppErrorCode::DatagramsRequired, false));
        }
        self.state = ServerState::AwaitHello;
        self.deadline = Some(now.saturating_add(micros(HELLO_TIMEOUT)));
        Ok(None)
    }

    /// The `Closed` event, the first time the end is reported.
    fn closed_event(&mut self, code: AppErrorCode, client: bool) -> Option<ServerEvent> {
        if self.reported {
            return None;
        }
        self.reported = true;
        Some(ServerEvent::Closed { code, client })
    }

    /// Handles a passed deadline at `now`. Returns a close event when a timeout ended the
    /// session.
    pub fn handle_timeout(&mut self, now: u64) -> Option<ServerEvent> {
        if self.login_deadline.is_some_and(|d| now >= d) && self.state == ServerState::AwaitLogin {
            self.login_deadline = None;
            return self.disconnect(AppErrorCode::AuthTimeout, "", now);
        }
        let deadline = self.deadline?;
        if now < deadline {
            return None;
        }
        self.deadline = None;
        match self.state {
            ServerState::Accepting => {
                self.close_now(AppErrorCode::AuthTimeout);
                self.closed_event(AppErrorCode::AuthTimeout, false)
            }
            // No control stream exists until the client sends on it (the server cannot open
            // one), so a Disconnect could not be sent: close at once.
            ServerState::AwaitHello if !self.control_seen => {
                self.close_now(AppErrorCode::AuthTimeout);
                self.closed_event(AppErrorCode::AuthTimeout, false)
            }
            ServerState::AwaitHello => self.disconnect(AppErrorCode::AuthTimeout, "", now),
            ServerState::CharacterSelect => self.disconnect(AppErrorCode::Idle, "", now),
            ServerState::Closing => {
                let code = self.close_code.unwrap_or(AppErrorCode::NoError);
                self.close_now(code);
                None
            }
            _ => None,
        }
    }

    /// The QUIC connection is closed. Returns the close event unless the end was already
    /// reported: `client` true with the client's code when it closed the connection, else
    /// `false` with the code the session closed with (`NO_ERROR` when none, such as an idle
    /// timeout or a transport error).
    pub fn connection_closed(&mut self, end: ConnectionEnd) -> Option<ServerEvent> {
        self.state = ServerState::Closed;
        self.deadline = None;
        self.login_deadline = None;
        self.outbox.clear();
        let (code, client) = match end {
            ConnectionEnd::PeerClosed { code, .. } => (code, true),
            ConnectionEnd::IdleTimeout | ConnectionEnd::Transport(_) | ConnectionEnd::Local => {
                (self.close_code.unwrap_or(AppErrorCode::NoError), false)
            }
        };
        self.closed_event(code, client)
    }

    fn close_now(&mut self, code: AppErrorCode) {
        self.outbox.push(Transmit::Close {
            code,
            reason: close_reason(code.name()),
        });
        self.state = ServerState::Closed;
        self.deadline = None;
        self.login_deadline = None;
    }

    /// Acts on a violation by the client: closes with its code and returns it. While Closing
    /// or Closed the session is already ending: the input is dropped and `None` returned, so
    /// a queued `Disconnect` is kept and nothing is sent after a `Close`.
    fn fail(&mut self, v: Violation) -> Option<Violation> {
        if matches!(self.state, ServerState::Closing | ServerState::Closed) {
            tracing::debug!(error = %v, state = self.state.name(), "input dropped while closing");
            self.dropped += 1;
            return None;
        }
        tracing::debug!(error = %v, state = self.state.name(), "closing: client broke the protocol");
        self.reported = true;
        self.outbox.clear();
        let code = v.app_code();
        self.outbox.push(Transmit::Close {
            code,
            reason: close_reason(&v.to_string()),
        });
        self.close_code = Some(code);
        self.state = ServerState::Closed;
        self.deadline = None;
        self.login_deadline = None;
        Some(v)
    }

    fn violation_or_none<T>(&mut self, v: Violation) -> Result<Option<T>, Violation> {
        match self.fail(v) {
            Some(v) => Err(v),
            None => Ok(None),
        }
    }

    /// Ends the session (§12.3): `Disconnect(code, text)` on the control stream, FIN, then
    /// Closing with a 2 s linger (kick, shutdown, `LOGGED_IN_ELSEWHERE`, ...). Before the
    /// client has sent anything on the control stream (Accepting, or AwaitHello with no
    /// bytes), no stream exists and the connection is closed at once with `code`. Returns
    /// the close event, or `None` when the session was already ending.
    pub fn disconnect(&mut self, code: AppErrorCode, text: &str, now: u64) -> Option<ServerEvent> {
        match self.state {
            ServerState::Closing | ServerState::Closed => return None,
            ServerState::Accepting => self.close_now(code),
            ServerState::AwaitHello if !self.control_seen => self.close_now(code),
            _ => {
                let mut t = text::sanitise(text).into_owned();
                t.truncate(floor_boundary(&t, MAX_LONG_TEXT_LEN));
                self.outbox
                    .send(Message::Disconnect(Disconnect { code, text: t }));
                self.outbox.push(Transmit::FinishControl);
                self.state = ServerState::Closing;
                self.close_code = Some(code);
                self.deadline = Some(now.saturating_add(micros(CLOSE_LINGER)));
                self.login_deadline = None;
            }
        }
        self.close_code = Some(code);
        self.closed_event(code, false)
    }

    /// The client finished, reset or stopped a stream (§2.3 rule 4): the control stream, or
    /// (STOP_SENDING) the world or chat stream. Outside Closing and Closed this is a
    /// violation (`Violation::Stream`). While Closing it is expected and ignored, including a
    /// partial frame left at the end.
    pub fn on_stream_end(&mut self, stream: StreamRef, end: StreamEnd) -> Result<(), Violation> {
        if matches!(self.state, ServerState::Closing | ServerState::Closed) {
            return Ok(());
        }
        let v = match (stream, end) {
            (StreamRef::Control, StreamEnd::Finished) if self.control.finish().is_err() => {
                Violation::Decode(crate::error::ProtocolError::Truncated)
            }
            (_, StreamEnd::Finished) => Violation::Stream("FIN outside Closing"),
            (_, StreamEnd::Reset) => Violation::Stream("RESET_STREAM outside Closing"),
            (_, StreamEnd::Stopped) => Violation::Stream("STOP_SENDING outside Closing"),
        };
        match self.fail(v) {
            Some(v) => Err(v),
            None => Ok(()),
        }
    }

    /// The client opened a stream beyond its single control stream (a second bidirectional
    /// stream, possible once QUIC grants new stream credit, or any unidirectional stream):
    /// a violation outside Closing and Closed (§2.3 rule 1). The network layer does not read
    /// it.
    pub fn on_extra_stream(&mut self) -> Result<(), Violation> {
        match self.fail(Violation::Stream("extra client stream")) {
            Some(v) => Err(v),
            None => Ok(()),
        }
    }

    // --- receiving ---

    /// Feeds bytes read from the control stream; returns the events of complete messages.
    pub fn on_control_data(
        &mut self,
        data: &[u8],
        now: u64,
    ) -> Result<Vec<ServerEvent>, Violation> {
        let mut input = data;
        let mut events = Vec::new();
        if !data.is_empty() {
            self.control_seen = true;
        }
        loop {
            // While Closing the stream is still read for the client's Disconnect (§12.3).
            if self.state == ServerState::Closed {
                return Ok(events);
            }
            match self.control.push(&mut input) {
                Ok(Some(m)) => {
                    if let Some(e) = self.on_control(m, now)? {
                        events.push(e);
                    }
                }
                Ok(None) => return Ok(events),
                Err(e) => {
                    return match self.fail(Violation::Decode(e)) {
                        Some(v) => Err(v),
                        None => Ok(events),
                    };
                }
            }
        }
    }

    /// Feeds one datagram. Datagrams the state does not allow, and those over the rate
    /// limits, are dropped.
    pub fn on_datagram(&mut self, data: &[u8], now: u64) -> Result<Option<ServerEvent>, Violation> {
        if matches!(self.state, ServerState::Closing | ServerState::Closed) {
            return Ok(None);
        }
        let msg = match decode_datagram(Side::Client, data) {
            Ok(m) => m,
            Err(e) => return self.violation_or_none(Violation::Decode(e)),
        };
        Ok(match msg {
            Message::TimeRequest { client_time } => {
                if self.state.after_hello_ack() && self.time_rate.try_record(now) {
                    self.outbox.send(Message::TimeReply(time_reply(
                        client_time,
                        now,
                        now,
                        self.params.tick_rate,
                    )));
                } else {
                    self.dropped += 1;
                }
                None
            }
            Message::MoveInput(m)
                if self.state == ServerState::InWorld && self.move_rate.try_record(now) =>
            {
                Some(ServerEvent::Move(m))
            }
            _ => {
                self.dropped += 1;
                None
            }
        })
    }

    fn unexpected(&self, ty: MessageType) -> Violation {
        Violation::UnexpectedMessage {
            ty,
            state: self.state.name(),
        }
    }

    /// Handles one decoded control message from the client.
    /// While Closing only the client's `Disconnect` is acted on: the connection is then
    /// closed with `NO_ERROR` at once (§12.3 step 2).
    pub fn on_control(&mut self, msg: Message, now: u64) -> Result<Option<ServerEvent>, Violation> {
        self.control_seen = true;
        match self.state {
            ServerState::Closed => return Ok(None),
            ServerState::Closing => {
                if let Message::Disconnect(d) = msg {
                    self.outbox.clear();
                    self.close_now(AppErrorCode::NoError);
                    return Ok(self.closed_event(d.code, true));
                }
                self.dropped += 1;
                return Ok(None);
            }
            _ => {}
        }
        let ty = msg.message_type();
        if ty.channel() != Channel::Control || !ty.direction().allows(Side::Client) {
            return self.violation_or_none(Violation::Decode(
                crate::error::ProtocolError::WrongDirection {
                    ty: ty.code(),
                    sender: Side::Client,
                },
            ));
        }
        if !self.control_rate.try_record(now) {
            // The end is reported by the returned violation, not by an event.
            let _ = self.disconnect(AppErrorCode::Flooding, "", now);
            return Err(Violation::Flooding);
        }
        if self.state == ServerState::CharacterSelect {
            self.deadline = Some(now.saturating_add(micros(CHARACTER_SELECT_IDLE)));
        }
        match self.on_control_inner(msg, now) {
            Ok(e) => Ok(e),
            Err(v) => self.violation_or_none(v),
        }
    }

    fn on_control_inner(
        &mut self,
        msg: Message,
        now: u64,
    ) -> Result<Option<ServerEvent>, Violation> {
        let ty = msg.message_type();
        let state = self.state;
        match msg {
            Message::Hello(h) => {
                if state != ServerState::AwaitHello {
                    return Err(self.unexpected(ty));
                }
                self.hello = Some(h.clone());
                match negotiate_minor(h.minor_max, self.params.min_minor, self.params.max_minor) {
                    None => Ok(self.disconnect(AppErrorCode::UnsupportedVersion, "", now)),
                    Some(minor) => {
                        self.minor = minor;
                        let p = &self.params;
                        let mut server_name = text::sanitise(&p.server_name).into_owned();
                        server_name.truncate(floor_boundary(
                            &server_name,
                            crate::limits::MAX_SOFTWARE_NAME_LEN,
                        ));
                        let mut motd = text::sanitise(&p.motd).into_owned();
                        motd.truncate(floor_boundary(&motd, MAX_LONG_TEXT_LEN));
                        let ack = Message::HelloAck(HelloAck {
                            minor,
                            server_caps: p.caps,
                            tick_rate: p.tick_rate,
                            snapshot_rate: p.snapshot_rate,
                            interest_radius: p.interest_radius,
                            server_name,
                            motd,
                        });
                        if check_encodable(&ack).is_err() {
                            return Ok(self.disconnect(AppErrorCode::InternalError, "", now));
                        }
                        self.outbox.send(ack);
                        self.state = ServerState::AwaitLogin;
                        self.deadline = None;
                        self.login_deadline = Some(now.saturating_add(micros(LOGIN_TIMEOUT)));
                        Ok(Some(ServerEvent::Greeted(h)))
                    }
                }
            }
            Message::Login(c) | Message::Register(c) => {
                let register = ty == MessageType::Register;
                if state != ServerState::AwaitLogin {
                    return Err(self.unexpected(ty));
                }
                if self.login_outstanding {
                    return Err(Violation::RequestOutstanding(ty));
                }
                if register && !self.params.caps.contains(ServerCaps::REGISTRATION) {
                    return Err(Violation::CapabilityMissing(ty));
                }
                self.login_outstanding = true;
                if !text::is_valid_account(&c.account) {
                    return Ok(self
                        .reject_login(LoginFailedCode::InvalidAccountName, 0, "", now)
                        .ok()
                        .flatten());
                }
                if c.password.is_empty() {
                    return Ok(self
                        .reject_login(LoginFailedCode::BadCredentials, 0, "", now)
                        .ok()
                        .flatten());
                }
                Ok(Some(ServerEvent::Login {
                    credentials: c,
                    register,
                }))
            }
            Message::CharacterCreate(c) => {
                if state != ServerState::CharacterSelect {
                    return Err(self.unexpected(ty));
                }
                if !self.params.caps.contains(ServerCaps::CHARACTER_CREATION) {
                    return Err(Violation::CapabilityMissing(ty));
                }
                if self.create_outstanding || self.enter_outstanding {
                    return Err(Violation::RequestOutstanding(ty));
                }
                self.create_outstanding = true;
                if !text::is_valid_name(&c.name) {
                    self.reject_character(CharacterCreateFailedCode::NameInvalid, "")
                        .ok();
                    return Ok(None);
                }
                Ok(Some(ServerEvent::CharacterCreate(c)))
            }
            Message::EnterWorld(e) => {
                if state != ServerState::CharacterSelect || self.entered {
                    return Err(self.unexpected(ty));
                }
                if self.create_outstanding || self.enter_outstanding {
                    return Err(Violation::RequestOutstanding(ty));
                }
                self.enter_outstanding = true;
                if !text::is_valid_name(&e.name) {
                    self.reject_enter_world(EnterWorldFailedCode::UnknownCharacter, "")
                        .ok();
                    return Ok(None);
                }
                Ok(Some(ServerEvent::EnterWorld { name: e.name }))
            }
            Message::TeleportRequest(t) => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                if let Destination::Named { name } = &t.destination
                    && !text::is_valid_place(name)
                {
                    self.outbox.send(Message::TeleportRefused(TeleportRefused {
                        request_id: t.request_id,
                        code: TeleportRefusedCode::InvalidDestination,
                        text: String::new(),
                    }));
                    return Ok(Some(ServerEvent::TeleportInvalid(t)));
                }
                Ok(Some(ServerEvent::Teleport(t)))
            }
            Message::QueryLocation { request_id } => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                Ok(Some(ServerEvent::QueryLocation { request_id }))
            }
            Message::ChatSend(c) => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                match self.chat.check(now) {
                    ChatVerdict::Allowed => {}
                    ChatVerdict::RateLimited => {
                        self.send_chat_rejected(ChatRejectedCode::RateLimited);
                        return Ok(None);
                    }
                    ChatVerdict::Flooding => {
                        return Ok(self.disconnect(AppErrorCode::Flooding, "", now));
                    }
                }
                let invalid = if !text::is_valid_text(&c.text) {
                    Some(ChatRejectedCode::InvalidText)
                } else if let ChatChannel::Tell { peer } = &c.channel
                    && !text::is_valid_name(peer)
                {
                    Some(ChatRejectedCode::UnknownRecipient)
                } else {
                    None
                };
                if let Some(code) = invalid {
                    return Ok(self.reject_chat(code, "", now).ok().flatten());
                }
                Ok(Some(ServerEvent::Chat(c)))
            }
            Message::EmoteSet(e) => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                Ok(Some(ServerEvent::Emote(e)))
            }
            Message::StanceSet(s) => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                Ok(Some(ServerEvent::Stance(s)))
            }
            Message::AppearanceSet(a) => {
                if state != ServerState::InWorld {
                    return Err(self.unexpected(ty));
                }
                if !self.params.caps.contains(ServerCaps::APPEARANCE_CHANGE) {
                    return Err(Violation::CapabilityMissing(ty));
                }
                Ok(Some(ServerEvent::Appearance(a)))
            }
            Message::Ping { nonce } => {
                if !state.after_hello_ack() {
                    return Err(self.unexpected(ty));
                }
                if self.ping_rate.try_record(now) {
                    self.outbox.send(Message::Pong { nonce });
                } else {
                    self.dropped += 1;
                }
                Ok(None)
            }
            Message::Pong { nonce } => {
                if !state.after_hello_ack() {
                    return Err(self.unexpected(ty));
                }
                match self.ping_outstanding {
                    Some((n, sent)) if n == nonce => {
                        self.ping_outstanding = None;
                        Ok(Some(ServerEvent::Pong {
                            nonce,
                            rtt: now.saturating_sub(sent),
                        }))
                    }
                    _ => {
                        self.dropped += 1;
                        Ok(None)
                    }
                }
            }
            Message::Disconnect(d) => {
                if state == ServerState::AwaitHello {
                    // "Client: after Hello" (§6.1).
                    return Err(self.unexpected(ty));
                }
                self.outbox.clear();
                self.close_now(AppErrorCode::NoError);
                Ok(self.closed_event(d.code, true))
            }
            _ => Err(self.unexpected(ty)),
        }
    }

    // --- answering ---

    /// Answers the outstanding `Login`/`Register` with `LoginOk` and opens the chat stream.
    /// CharacterSelect then ends with `Disconnect(IDLE)` after 15 minutes without a client
    /// message.
    pub fn accept_login(
        &mut self,
        account: String,
        characters: Vec<CharacterSummary>,
        now: u64,
    ) -> Result<(), UsageError> {
        if self.state != ServerState::AwaitLogin || !self.login_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        let msg = Message::LoginOk(LoginOk {
            account,
            characters,
        });
        check_encodable(&msg)?;
        self.outbox.send(msg);
        self.outbox.push(Transmit::OpenStream(StreamKind::Chat));
        self.login_outstanding = false;
        self.login_deadline = None;
        self.state = ServerState::CharacterSelect;
        self.deadline = Some(now.saturating_add(micros(CHARACTER_SELECT_IDLE)));
        Ok(())
    }

    /// Answers the outstanding `Login`/`Register` with `LoginFailed`. `SERVER_FULL` and
    /// `SERVER_ERROR` are followed by `Disconnect(SERVER_FULL / INTERNAL_ERROR)`, and the
    /// third failure on the connection by `Disconnect(AUTH_FAILED)` (§4.7); the close event
    /// is then returned.
    pub fn reject_login(
        &mut self,
        code: LoginFailedCode,
        retry_after: u16,
        detail: &str,
        now: u64,
    ) -> Result<Option<ServerEvent>, UsageError> {
        if self.state != ServerState::AwaitLogin || !self.login_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        let code = code.canonical();
        let retry_after = if code == LoginFailedCode::RateLimited {
            retry_after
        } else {
            0
        };
        self.outbox.send(Message::LoginFailed(LoginFailed {
            code,
            retry_after,
            text: detail_text(detail),
        }));
        self.login_outstanding = false;
        self.failures += 1;
        Ok(match code.canonical() {
            LoginFailedCode::ServerFull => self.disconnect(AppErrorCode::ServerFull, "", now),
            LoginFailedCode::ServerError => self.disconnect(AppErrorCode::InternalError, "", now),
            _ if self.failures >= MAX_LOGIN_FAILURES => {
                self.disconnect(AppErrorCode::AuthFailed, "", now)
            }
            _ => None,
        })
    }

    /// Answers the outstanding `CharacterCreate` with the account's full list.
    pub fn character_created(
        &mut self,
        characters: Vec<CharacterSummary>,
    ) -> Result<(), UsageError> {
        if self.state != ServerState::CharacterSelect || !self.create_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        let msg = Message::CharacterList(CharacterList { characters });
        check_encodable(&msg)?;
        self.outbox.send(msg);
        self.create_outstanding = false;
        Ok(())
    }

    /// Answers the outstanding `CharacterCreate` with `CharacterCreateFailed`.
    pub fn reject_character(
        &mut self,
        code: CharacterCreateFailedCode,
        detail: &str,
    ) -> Result<(), UsageError> {
        if self.state != ServerState::CharacterSelect || !self.create_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        self.outbox.send(Message::CharacterCreateFailed(Failure {
            code,
            text: detail_text(detail),
        }));
        self.create_outstanding = false;
        Ok(())
    }

    /// Answers the outstanding `EnterWorld` with success: opens the world stream and sends
    /// `WorldEnter` as its first frame. The server follows with `WorldClock`, the player's
    /// own `EntityUpdate` and the initial `EntityCreate`s (§9.5) through
    /// [`send`](Self::send).
    pub fn enter_world(&mut self, enter: WorldEnter) -> Result<(), UsageError> {
        if self.state != ServerState::CharacterSelect || !self.enter_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        let msg = Message::WorldEnter(enter);
        check_encodable(&msg)?;
        self.outbox.push(Transmit::OpenStream(StreamKind::World));
        self.outbox.send(msg);
        self.enter_outstanding = false;
        self.entered = true;
        self.state = ServerState::InWorld;
        self.deadline = None;
        Ok(())
    }

    /// Answers the outstanding `EnterWorld` with `EnterWorldFailed`.
    pub fn reject_enter_world(
        &mut self,
        code: EnterWorldFailedCode,
        detail: &str,
    ) -> Result<(), UsageError> {
        if self.state != ServerState::CharacterSelect || !self.enter_outstanding {
            return Err(UsageError::WrongState(self.state.name()));
        }
        self.outbox.send(Message::EnterWorldFailed(Failure {
            code,
            text: detail_text(detail),
        }));
        self.enter_outstanding = false;
        Ok(())
    }

    fn send_chat_rejected(&mut self, code: ChatRejectedCode) {
        self.outbox.send(Message::ChatRejected(Failure {
            code,
            text: String::new(),
        }));
    }

    /// Rejects a `ChatSend` (unknown recipient, muted, ...). Every rejection counts towards
    /// `Disconnect(FLOODING)` (20 within 60 s, §10.2); the close event is then returned.
    pub fn reject_chat(
        &mut self,
        code: ChatRejectedCode,
        detail: &str,
        now: u64,
    ) -> Result<Option<ServerEvent>, UsageError> {
        if self.state != ServerState::InWorld {
            return Err(UsageError::WrongState(self.state.name()));
        }
        self.outbox.send(Message::ChatRejected(Failure {
            code,
            text: detail_text(detail),
        }));
        if self.chat.record_rejection(now) == ChatVerdict::Flooding {
            return Ok(self.disconnect(AppErrorCode::Flooding, "", now));
        }
        Ok(None)
    }

    /// Sends a server message the state allows: world-stream messages while InWorld,
    /// chat-stream messages after `LoginOk`, `MotionBatch` while InWorld, `Ping` after
    /// `HelloAck` (at most one per second). Answers handled by the methods above are
    /// refused here.
    pub fn send(&mut self, msg: Message, now: u64) -> Result<(), UsageError> {
        let ty = msg.message_type();
        let allowed = match ty.channel() {
            Channel::World => self.state == ServerState::InWorld && ty != MessageType::WorldEnter,
            Channel::Chat => matches!(
                self.state,
                ServerState::CharacterSelect | ServerState::InWorld
            ),
            Channel::Datagram => {
                ty == MessageType::MotionBatch && self.state == ServerState::InWorld
            }
            Channel::Control => ty == MessageType::Ping && self.state.after_hello_ack(),
        };
        if !allowed {
            return Err(UsageError::WrongState(self.state.name()));
        }
        check_encodable(&msg)?;
        if let Message::Ping { nonce } = msg {
            if !self.own_ping_rate.try_record(now) {
                return Err(UsageError::RateLimited);
            }
            self.ping_outstanding = Some((nonce, now));
        }
        self.outbox.send(msg);
        Ok(())
    }

    /// Sends a `Ping` with a fresh nonce; returns the nonce.
    pub fn ping(&mut self, now: u64) -> Result<u64, UsageError> {
        let nonce = self.next_nonce;
        self.send(Message::Ping { nonce }, now)?;
        self.next_nonce = self.next_nonce.wrapping_add(1);
        Ok(nonce)
    }
}

fn detail_text(detail: &str) -> String {
    let mut t = text::sanitise(detail).into_owned();
    t.truncate(floor_boundary(&t, MAX_DETAIL_TEXT_LEN));
    t
}
