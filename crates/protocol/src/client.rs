// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's connection state machine (`tessera-protocol.md` §12.1), sans I/O.
//!
//! [`ClientSession`] checks every received message against the client's state (§6.1, §9.3,
//! §13), tracks the entities created on the connection, the position epoch and the clock
//! offset, and produces the messages to send. The network layer (the `tessera-net`
//! test-server backend) runs QUIC and TLS, feeds received bytes in and drains
//! [`Transmit`]s out.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::codes::{AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, ClientFailure};
use crate::codes::{EnterWorldFailedCode, LoginFailedCode};
use crate::error::ProtocolError;
use crate::frame::{FrameDecoder, StreamKind, decode_datagram};
use crate::limits::{
    CLOSE_LINGER, ENTER_WORLD_TIMEOUT, HANDSHAKE_TIMEOUT, HELLO_TIMEOUT, LOGIN_ANSWER_TIMEOUT,
    MAX_MOVE_INPUTS_PER_SECOND, MAX_PINGS_PER_SECOND, PROTOCOL_MINOR,
};
use crate::message::{
    Channel, CharacterCreate, ChatChannel, ChatMessage, ChatSend, Credentials, Disconnect,
    EmoteSet, EnterWorld, EntityAppearance, EntityCreate, EntityUpdate, Failure, Hello, HelloAck,
    LocationCause, LocationReport, LoginFailed, Message, MessageType, MotionEntry, MoveInput,
    ServerCaps, ServerNotice, Side, TeleportRefused, TeleportRequest, TimeReply, WorldClock,
    WorldEnter,
};
use crate::rate::SlidingWindow;
use crate::session::{
    ConnectionEnd, Outbox, PeerDatagrams, StreamEnd, StreamRef, Transmit, UsageError, Violation,
    close_reason, micros,
};
use crate::time::{ClockOffsetEstimator, TimeSample};
use crate::types::{Appearance, CharacterSummary, Destination, Location, Look, Motion, Vec3};
use crate::value::Stance;

/// Client states (§12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientState {
    /// Created.
    Idle,
    /// QUIC/TLS handshake running.
    Handshaking,
    /// `Hello` sent, waiting for `HelloAck`.
    Greeting,
    /// `HelloAck` received; logging in.
    Authenticating,
    /// Logged in; choosing a character.
    CharacterSelect,
    /// `EnterWorld` sent.
    EnteringWorld,
    /// `WorldEnter` processed.
    InWorld,
    /// `Disconnect` sent or received, or a fatal error.
    Closing,
    /// Connection closed.
    Closed,
}

impl ClientState {
    /// The state's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Handshaking => "Handshaking",
            Self::Greeting => "Greeting",
            Self::Authenticating => "Authenticating",
            Self::CharacterSelect => "CharacterSelect",
            Self::EnteringWorld => "EnteringWorld",
            Self::InWorld => "InWorld",
            Self::Closing => "Closing",
            Self::Closed => "Closed",
        }
    }

    fn after_hello_ack(self) -> bool {
        matches!(
            self,
            Self::Authenticating | Self::CharacterSelect | Self::EnteringWorld | Self::InWorld
        )
    }
}

/// Client parameters sent in `Hello` (§6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Highest minor version supported.
    pub minor_max: u16,
    /// Capability bits (none defined in v1.0).
    pub client_caps: u32,
    /// The most entities accepted at once, ≥ 64.
    pub max_entities: u16,
    /// Software name and version for the server's logs (`TEXT0`(64)).
    pub client_name: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            minor_max: PROTOCOL_MINOR,
            client_caps: 0,
            max_entities: 1024,
            client_name: format!("tessera {}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Why the client's connection ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseCause {
    /// The server sent `Disconnect`.
    ServerDisconnect {
        /// Its code.
        code: AppErrorCode,
        /// Its text.
        text: String,
    },
    /// The client sent `Disconnect` (the user asked, or a local decision).
    LocalDisconnect {
        /// The code sent.
        code: AppErrorCode,
    },
    /// The peer closed the QUIC connection with this application code and reason.
    PeerClosed {
        /// The application code.
        code: AppErrorCode,
        /// The reason phrase, sanitised and cut to 64 bytes.
        reason: String,
    },
    /// A local transport or I/O error ended the connection (Appendix A `TransportFailed`).
    Transport(String),
    /// The server broke a rule; closed with `PROTOCOL_VIOLATION` (or as the violation says).
    Violation(Violation),
    /// A client-local failure (§14.2).
    Failure(ClientFailure),
}

/// Something the application should know, produced by a received message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ClientEvent {
    /// `HelloAck` received: the client may log in.
    HelloAccepted(HelloAck),
    /// `LoginOk`: logged in, with the character list.
    LoggedIn {
        /// The account's canonical name.
        account: String,
        /// Its characters.
        characters: Vec<CharacterSummary>,
        /// Minor ≥ 1: one look per character (empty looks: none); empty at minor 0.
        looks: Vec<Look>,
    },
    /// `LoginFailed`. The client may try again unless a `Disconnect` follows.
    LoginFailed(LoginFailed),
    /// `CharacterList` after a successful `CharacterCreate`.
    CharacterList {
        /// The account's full list.
        characters: Vec<CharacterSummary>,
        /// Minor ≥ 1: one look per character; empty at minor 0.
        looks: Vec<Look>,
    },
    /// `CharacterCreateFailed`.
    CharacterCreateFailed(Failure<CharacterCreateFailedCode>),
    /// `EnterWorldFailed`; back in CharacterSelect.
    EnterWorldFailed(Failure<EnterWorldFailedCode>),
    /// `WorldEnter`: in the world.
    WorldEntered(WorldEnter),
    /// `LocationReport`. For Teleported and Corrected the client moves its avatar, sets the
    /// velocity to 0 and sends a `MoveInput` with the new epoch at once (§8.4).
    Location(LocationReport),
    /// `TeleportRefused`.
    TeleportRefused(TeleportRefused),
    /// `EntityCreate` of a new entity.
    EntityCreated(EntityCreate),
    /// `EntityUpdate` of a created entity or the player.
    EntityUpdated(EntityUpdate),
    /// `EntityAppearance` of a created entity or the player.
    AppearanceChanged(EntityAppearance),
    /// `EntityDestroy`.
    EntityDestroyed(crate::message::EntityDestroy),
    /// `InterestReset`: every listed entity is gone.
    InterestReset(Vec<u64>),
    /// `WorldClock`.
    WorldClock(WorldClock),
    /// `ChatMessage`.
    Chat(ChatMessage),
    /// `ServerNotice`.
    Notice(ServerNotice),
    /// `ChatRejected`.
    ChatRejected(Failure<ChatRejectedCode>),
    /// The fresh entries of a `MotionBatch` (stale and unknown ones removed, §8.6).
    Motion {
        /// The batch's snapshot time.
        server_time: u64,
        /// Entries to apply.
        entries: Vec<MotionEntry>,
    },
    /// A time-sync sample was taken (§11.2).
    TimeSample(TimeSample),
    /// `Pong` for the outstanding `Ping`.
    Pong {
        /// The nonce.
        nonce: u64,
        /// Round-trip time, µs.
        rtt: u64,
    },
    /// The connection is over (Closing or Closed). A session reports its end exactly once:
    /// with this event, or with an input that returned a [`Violation`].
    Closed(CloseCause),
}

#[derive(Debug, Clone)]
struct World {
    player: u64,
    epoch: u32,
    region: u16,
    entities: BTreeMap<u64, u64>,
    reset_since_report: bool,
    pending_teleport: Option<u32>,
}

/// The client's side of one connection (§12.1).
#[derive(Debug)]
pub struct ClientSession {
    config: ClientConfig,
    state: ClientState,
    deadline: Option<u64>,
    outbox: Outbox,
    hello_ack: Option<HelloAck>,
    login_outstanding: bool,
    login_retry_at: Option<u64>,
    create_outstanding: bool,
    account: Option<String>,
    world: Option<World>,
    next_request: u32,
    move_seq: u32,
    move_rate: SlidingWindow,
    ping_rate: SlidingWindow,
    peer_ping_rate: SlidingWindow,
    ping_outstanding: Option<(u64, u64)>,
    next_nonce: u64,
    time: ClockOffsetEstimator,
    control: FrameDecoder,
    world_stream: Option<FrameDecoder>,
    chat_stream: Option<FrameDecoder>,
    uni_streams: u8,
    cause: Option<CloseCause>,
    dropped: u64,
}

impl ClientSession {
    /// A session in state Idle.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            state: ClientState::Idle,
            deadline: None,
            outbox: Outbox::default(),
            hello_ack: None,
            login_outstanding: false,
            login_retry_at: None,
            create_outstanding: false,
            account: None,
            world: None,
            next_request: 1,
            move_seq: 0,
            move_rate: SlidingWindow::new(MAX_MOVE_INPUTS_PER_SECOND, Duration::from_secs(1)),
            ping_rate: SlidingWindow::new(MAX_PINGS_PER_SECOND, Duration::from_secs(1)),
            peer_ping_rate: SlidingWindow::new(MAX_PINGS_PER_SECOND, Duration::from_secs(1)),
            ping_outstanding: None,
            next_nonce: 1,
            time: ClockOffsetEstimator::new(),
            control: FrameDecoder::new(Channel::Control, Side::Server),
            world_stream: None,
            chat_stream: None,
            uni_streams: 0,
            cause: None,
            dropped: 0,
        }
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// The server's `HelloAck`, once received.
    #[must_use]
    pub fn hello_ack(&self) -> Option<&HelloAck> {
        self.hello_ack.as_ref()
    }

    /// The negotiated minor version (§3.1): `HelloAck.minor`, 0 before it. Messages of
    /// [`Transmit::Message`] are encoded under it
    /// ([`frame_to_vec_minor`](crate::frame::frame_to_vec_minor)).
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.hello_ack.as_ref().map_or(0, |a| a.minor)
    }

    /// The account name from `LoginOk`.
    #[must_use]
    pub fn account(&self) -> Option<&str> {
        self.account.as_deref()
    }

    /// The player's entity ID once in the world.
    #[must_use]
    pub fn player(&self) -> Option<u64> {
        self.world.as_ref().map(|w| w.player)
    }

    /// The current position epoch once in the world.
    #[must_use]
    pub fn epoch(&self) -> Option<u32> {
        self.world.as_ref().map(|w| w.epoch)
    }

    /// The current region once in the world.
    #[must_use]
    pub fn region(&self) -> Option<u16> {
        self.world.as_ref().map(|w| w.region)
    }

    /// IDs of the entities currently created on this connection (not the player).
    pub fn entities(&self) -> impl Iterator<Item = u64> + '_ {
        self.world.iter().flat_map(|w| w.entities.keys().copied())
    }

    /// The request ID of the `TeleportRequest` sent last, while the server has not answered
    /// it. The server answers within 10 s (§6.2.12); an earlier unresolved request is
    /// answered with `CANCELLED` and no longer counts as pending.
    #[must_use]
    pub fn pending_teleport(&self) -> Option<u32> {
        self.world.as_ref().and_then(|w| w.pending_teleport)
    }

    /// The client time (µs) before which `Login`/`Register` must not be sent after
    /// `LoginFailed(RATE_LIMITED)` (§4.7), if one is in force.
    #[must_use]
    pub fn login_retry_at(&self) -> Option<u64> {
        self.login_retry_at
    }

    /// The clock-offset estimator (§11.2).
    #[must_use]
    pub fn clock(&self) -> &ClockOffsetEstimator {
        &self.time
    }

    /// Messages dropped under §13.2 so far.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Why the connection ended, once it has.
    #[must_use]
    pub fn close_cause(&self) -> Option<&CloseCause> {
        self.cause.as_ref()
    }

    /// The next thing to transmit.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.outbox.pop()
    }

    /// The time (µs) at which [`handle_timeout`](Self::handle_timeout) must be called.
    #[must_use]
    pub fn poll_timeout(&self) -> Option<u64> {
        self.deadline
    }

    /// Whether the network layer may read the stream of `kind` now (§2.3 rule 5): chat after
    /// `LoginOk` was processed, world after `EnterWorld` was sent.
    #[must_use]
    pub fn may_read(&self, kind: StreamKind) -> bool {
        match kind {
            StreamKind::Chat => matches!(
                self.state,
                ClientState::CharacterSelect | ClientState::EnteringWorld | ClientState::InWorld
            ),
            StreamKind::World => {
                matches!(
                    self.state,
                    ClientState::EnteringWorld | ClientState::InWorld
                )
            }
        }
    }

    // --- lifecycle ---

    /// Starts the connection: Idle → Handshaking (10 s timeout).
    pub fn connect(&mut self, now: u64) -> Result<(), UsageError> {
        self.require(ClientState::Idle)?;
        self.state = ClientState::Handshaking;
        self.deadline = Some(now.saturating_add(micros(HANDSHAKE_TIMEOUT)));
        Ok(())
    }

    /// The QUIC/TLS handshake completed and the pin check of §4.3 passed. `datagrams` is
    /// what the QUIC stack reports about the server's datagram support (with quinn:
    /// `transport::peer_datagrams`). Sends `Hello` and moves to Greeting (returns `None`), or
    /// closes with `DATAGRAMS_REQUIRED` and returns the close event.
    pub fn handshake_complete(
        &mut self,
        now: u64,
        datagrams: PeerDatagrams,
    ) -> Result<Option<ClientEvent>, UsageError> {
        self.require(ClientState::Handshaking)?;
        if !datagrams.acceptable() {
            self.fail_violation(Violation::DatagramsRequired);
            return Ok(Some(ClientEvent::Closed(CloseCause::Violation(
                Violation::DatagramsRequired,
            ))));
        }
        let hello = Message::Hello(Hello {
            minor_max: self.config.minor_max,
            client_caps: self.config.client_caps,
            max_entities: self.config.max_entities,
            client_name: self.config.client_name.clone(),
        });
        check_encodable(&hello)?;
        self.outbox.send(hello);
        self.state = ClientState::Greeting;
        self.deadline = Some(now.saturating_add(micros(HELLO_TIMEOUT)));
        Ok(None)
    }

    /// The TLS handshake failed locally (§14.2: `TLS_FAILURE`, `KEY_MISMATCH`, `KEY_UNKNOWN`,
    /// `NO_COMMON_VERSION`, `TIMEOUT`). The session is Closed.
    pub fn handshake_failed(&mut self, failure: ClientFailure) -> ClientEvent {
        self.state = ClientState::Closed;
        self.deadline = None;
        self.cause = Some(CloseCause::Failure(failure));
        ClientEvent::Closed(CloseCause::Failure(failure))
    }

    /// The QUIC connection is closed. Returns the close event unless the end was already
    /// reported. The peer's reason phrase is sanitised and cut to 64 bytes (§12.3, §13.1).
    pub fn connection_closed(&mut self, end: ConnectionEnd) -> Option<ClientEvent> {
        let was = self.state;
        self.state = ClientState::Closed;
        self.deadline = None;
        self.outbox.clear();
        if self.cause.is_some() || was == ClientState::Closed {
            return None;
        }
        let cause = match end {
            ConnectionEnd::PeerClosed { code, reason } => CloseCause::PeerClosed {
                code,
                reason: close_reason(&crate::text::sanitise(&reason)),
            },
            ConnectionEnd::IdleTimeout | ConnectionEnd::Local => {
                CloseCause::Failure(ClientFailure::Timeout)
            }
            ConnectionEnd::Transport(detail) => CloseCause::Transport(detail),
        };
        self.cause = Some(cause.clone());
        Some(ClientEvent::Closed(cause))
    }

    /// Handles a passed deadline at `now`. Returns the close event when a state timeout
    /// fired.
    pub fn handle_timeout(&mut self, now: u64) -> Option<ClientEvent> {
        let deadline = self.deadline?;
        if now < deadline {
            return None;
        }
        self.deadline = None;
        match self.state {
            ClientState::Closing => {
                let code = match &self.cause {
                    Some(CloseCause::LocalDisconnect { code }) => *code,
                    _ => AppErrorCode::NoError,
                };
                self.outbox.push(Transmit::Close {
                    code,
                    reason: close_reason(code.name()),
                });
                self.state = ClientState::Closed;
                None
            }
            ClientState::Handshaking
            | ClientState::Greeting
            | ClientState::Authenticating
            | ClientState::EnteringWorld => {
                self.outbox.push(Transmit::Close {
                    code: AppErrorCode::NoError,
                    reason: close_reason("TIMEOUT"),
                });
                self.state = ClientState::Closed;
                let cause = CloseCause::Failure(ClientFailure::Timeout);
                self.cause = Some(cause.clone());
                Some(ClientEvent::Closed(cause))
            }
            _ => None,
        }
    }

    // --- receiving ---

    /// Feeds bytes read from the control stream. Returns the events of every complete
    /// message. On a violation the close is queued and the error returned.
    pub fn on_control_data(
        &mut self,
        data: &[u8],
        now: u64,
    ) -> Result<Vec<ClientEvent>, Violation> {
        let mut input = data;
        let mut events = Vec::new();
        loop {
            if self.state == ClientState::Closed {
                return Ok(events);
            }
            match self.control.push(&mut input) {
                Ok(Some(m)) => {
                    if let Some(e) = self.on_message(Channel::Control, m, now)? {
                        events.push(e);
                    }
                }
                Ok(None) => return Ok(events),
                Err(e) => {
                    return match self.fail_violation(Violation::Decode(e)) {
                        Some(v) => Err(v),
                        None => Ok(events),
                    };
                }
            }
        }
    }

    /// The server opened a unidirectional stream whose first byte is `kind_byte`. Returns its
    /// kind; an unknown kind, a duplicate or a third stream is a violation. Returns `None`
    /// while Closing or Closed: the stream is ignored (not read).
    pub fn on_uni_stream(&mut self, kind_byte: u8) -> Result<Option<StreamKind>, Violation> {
        if matches!(self.state, ClientState::Closing | ClientState::Closed) {
            return Ok(None);
        }
        let kind = match StreamKind::from_byte(kind_byte) {
            Ok(k) => k,
            Err(e) => return self.violation_or_none(Violation::Decode(e)),
        };
        let slot = match kind {
            StreamKind::World => &mut self.world_stream,
            StreamKind::Chat => &mut self.chat_stream,
        };
        if slot.is_some() || self.uni_streams >= 2 {
            return self.violation_or_none(Violation::Stream("second stream of the same kind"));
        }
        let minor = self.hello_ack.as_ref().map_or(0, |a| a.minor);
        let slot = match kind {
            StreamKind::World => &mut self.world_stream,
            StreamKind::Chat => &mut self.chat_stream,
        };
        *slot = Some(FrameDecoder::with_minor(
            kind.channel(),
            Side::Server,
            minor,
        ));
        self.uni_streams += 1;
        Ok(Some(kind))
    }

    fn violation_or_none<T>(&mut self, v: Violation) -> Result<Option<T>, Violation> {
        match self.fail_violation(v) {
            Some(v) => Err(v),
            None => Ok(None),
        }
    }

    /// The server finished, reset or stopped a stream (§2.3 rule 4). Outside Closing and
    /// Closed this is a violation (`Violation::Stream`). In those states it is expected and
    /// ignored, including a partial frame left at the end.
    pub fn on_stream_end(&mut self, stream: StreamRef, end: StreamEnd) -> Result<(), Violation> {
        if matches!(self.state, ClientState::Closing | ClientState::Closed) {
            return Ok(());
        }
        let truncated = end == StreamEnd::Finished
            && match stream {
                StreamRef::Control => self.control.finish().is_err(),
                StreamRef::Uni(StreamKind::World) => self
                    .world_stream
                    .as_ref()
                    .is_some_and(|d| d.finish().is_err()),
                StreamRef::Uni(StreamKind::Chat) => self
                    .chat_stream
                    .as_ref()
                    .is_some_and(|d| d.finish().is_err()),
                StreamRef::Untyped => true,
            };
        let v = match (end, truncated) {
            (StreamEnd::Finished, true) => Violation::Decode(ProtocolError::Truncated),
            (StreamEnd::Finished, false) => Violation::Stream("FIN outside Closing"),
            (StreamEnd::Reset, _) => Violation::Stream("RESET_STREAM outside Closing"),
            (StreamEnd::Stopped, _) => Violation::Stream("STOP_SENDING outside Closing"),
        };
        match self.fail_violation(v) {
            Some(v) => Err(v),
            None => Ok(()),
        }
    }

    /// Feeds bytes read from the world or chat stream (after its kind byte).
    pub fn on_stream_data(
        &mut self,
        kind: StreamKind,
        data: &[u8],
        now: u64,
    ) -> Result<Vec<ClientEvent>, Violation> {
        let mut input = data;
        let mut events = Vec::new();
        loop {
            // The client stops reading the world and chat streams once it is closing.
            if matches!(self.state, ClientState::Closing | ClientState::Closed) {
                return Ok(events);
            }
            let decoder = match kind {
                StreamKind::World => self.world_stream.as_mut(),
                StreamKind::Chat => self.chat_stream.as_mut(),
            };
            let Some(decoder) = decoder else {
                return match self.fail_violation(Violation::Stream("data on an unopened stream")) {
                    Some(v) => Err(v),
                    None => Ok(events),
                };
            };
            match decoder.push(&mut input) {
                Ok(Some(m)) => {
                    if let Some(e) = self.on_message(kind.channel(), m, now)? {
                        events.push(e);
                    }
                }
                Ok(None) => return Ok(events),
                Err(e) => {
                    return match self.fail_violation(Violation::Decode(e)) {
                        Some(v) => Err(v),
                        None => Ok(events),
                    };
                }
            }
        }
    }

    /// Feeds one received datagram. Datagrams the state does not allow are dropped (§5.5),
    /// and so is everything received while Closing or Closed.
    pub fn on_datagram(&mut self, data: &[u8], now: u64) -> Result<Option<ClientEvent>, Violation> {
        if matches!(self.state, ClientState::Closing | ClientState::Closed) {
            self.dropped += 1;
            return Ok(None);
        }
        match decode_datagram(Side::Server, data) {
            Ok(m) => self.on_message(Channel::Datagram, m, now),
            Err(e) => self.violation_or_none(Violation::Decode(e)),
        }
    }

    /// Handles one decoded message that arrived on `channel` (the decoders have already
    /// checked type ranges and direction).
    pub fn on_message(
        &mut self,
        channel: Channel,
        msg: Message,
        now: u64,
    ) -> Result<Option<ClientEvent>, Violation> {
        if matches!(self.state, ClientState::Closed) {
            return Ok(None);
        }
        let ty = msg.message_type();
        if ty.channel() != channel || !ty.direction().allows(Side::Server) {
            return self.violation_or_none(Violation::Decode(ProtocolError::WrongChannel {
                ty: ty.code(),
                channel,
            }));
        }
        if self.state == ClientState::Closing {
            // Stopped sending; still honour a Disconnect from the server (§12.3 step 2). The
            // end was already reported when the client's own Disconnect was sent.
            if let Message::Disconnect(d) = msg {
                self.on_disconnect(d);
            } else {
                self.dropped += 1;
            }
            return Ok(None);
        }
        let result = match channel {
            Channel::Control => self.on_control(msg, now),
            Channel::World => self.on_world(msg),
            Channel::Chat => self.on_chat(msg),
            Channel::Datagram => Ok(self.on_dgram(msg, now)),
        };
        match result {
            Ok(e) => Ok(e),
            Err(v) => self.violation_or_none(v),
        }
    }

    fn unexpected(&self, ty: MessageType) -> Violation {
        Violation::UnexpectedMessage {
            ty,
            state: self.state.name(),
        }
    }

    fn on_control(&mut self, msg: Message, now: u64) -> Result<Option<ClientEvent>, Violation> {
        let ty = msg.message_type();
        match msg {
            Message::HelloAck(ack) => {
                if self.state != ClientState::Greeting {
                    return Err(self.unexpected(ty));
                }
                if ack.minor > self.config.minor_max {
                    return Err(Violation::MinorTooHigh {
                        minor: ack.minor,
                        max: self.config.minor_max,
                    });
                }
                self.control.set_minor(ack.minor);
                self.hello_ack = Some(ack.clone());
                self.state = ClientState::Authenticating;
                self.deadline = None;
                Ok(Some(ClientEvent::HelloAccepted(ack)))
            }
            Message::LoginOk(ok) => {
                if self.state != ClientState::Authenticating || !self.login_outstanding {
                    return Err(self.unexpected(ty));
                }
                self.login_outstanding = false;
                self.account = Some(ok.account.clone());
                self.state = ClientState::CharacterSelect;
                self.deadline = None;
                Ok(Some(ClientEvent::LoggedIn {
                    account: ok.account,
                    characters: ok.characters,
                    looks: ok.looks,
                }))
            }
            Message::LoginFailed(f) => {
                if self.state != ClientState::Authenticating || !self.login_outstanding {
                    return Err(self.unexpected(ty));
                }
                self.login_outstanding = false;
                self.deadline = None;
                // §4.7: no retry before `retry_after` seconds.
                self.login_retry_at = (f.code == LoginFailedCode::RateLimited && f.retry_after > 0)
                    .then(|| now.saturating_add(u64::from(f.retry_after) * 1_000_000));
                Ok(Some(ClientEvent::LoginFailed(f)))
            }
            Message::CharacterList(l) => {
                if self.state != ClientState::CharacterSelect || !self.create_outstanding {
                    return Err(self.unexpected(ty));
                }
                self.create_outstanding = false;
                Ok(Some(ClientEvent::CharacterList {
                    characters: l.characters,
                    looks: l.looks,
                }))
            }
            Message::CharacterCreateFailed(f) => {
                if self.state != ClientState::CharacterSelect || !self.create_outstanding {
                    return Err(self.unexpected(ty));
                }
                self.create_outstanding = false;
                Ok(Some(ClientEvent::CharacterCreateFailed(f)))
            }
            Message::EnterWorldFailed(f) => {
                if self.state != ClientState::EnteringWorld {
                    return Err(self.unexpected(ty));
                }
                self.state = ClientState::CharacterSelect;
                self.deadline = None;
                Ok(Some(ClientEvent::EnterWorldFailed(f)))
            }
            Message::Ping { nonce } => {
                if !self.state.after_hello_ack() {
                    return Err(self.unexpected(ty));
                }
                if self.peer_ping_rate.try_record(now) {
                    self.outbox.send(Message::Pong { nonce });
                }
                Ok(None)
            }
            Message::Pong { nonce } => {
                if !self.state.after_hello_ack() {
                    return Err(self.unexpected(ty));
                }
                match self.ping_outstanding {
                    Some((n, sent)) if n == nonce => {
                        self.ping_outstanding = None;
                        Ok(Some(ClientEvent::Pong {
                            nonce,
                            rtt: now.saturating_sub(sent),
                        }))
                    }
                    _ => {
                        self.dropped += 1;
                        tracing::debug!(nonce, "unsolicited Pong dropped");
                        Ok(None)
                    }
                }
            }
            Message::Disconnect(d) => Ok(Some(self.on_disconnect(d))),
            _ => Err(self.unexpected(ty)),
        }
    }

    fn on_disconnect(&mut self, d: Disconnect) -> ClientEvent {
        self.outbox.clear();
        self.outbox.push(Transmit::Close {
            code: AppErrorCode::NoError,
            reason: String::new(),
        });
        self.state = ClientState::Closed;
        self.deadline = None;
        let cause = CloseCause::ServerDisconnect {
            code: d.code,
            text: d.text,
        };
        if self.cause.is_none() {
            self.cause = Some(cause.clone());
        }
        ClientEvent::Closed(cause)
    }

    fn on_world(&mut self, msg: Message) -> Result<Option<ClientEvent>, Violation> {
        let ty = msg.message_type();
        if let Message::WorldEnter(e) = msg {
            if self.state != ClientState::EnteringWorld || self.world.is_some() {
                return Err(self.unexpected(ty));
            }
            self.world = Some(World {
                player: e.player,
                epoch: e.epoch,
                region: e.pose.location.region,
                entities: BTreeMap::new(),
                reset_since_report: false,
                pending_teleport: None,
            });
            self.state = ClientState::InWorld;
            self.deadline = None;
            return Ok(Some(ClientEvent::WorldEntered(e)));
        }
        if self.state != ClientState::InWorld {
            return Err(self.unexpected(ty));
        }
        let max_entities = usize::from(self.config.max_entities);
        let Some(w) = self.world.as_mut() else {
            return Err(Violation::World("no WorldEnter"));
        };
        match msg {
            Message::LocationReport(r) => {
                let region = r.pose.location.region;
                match r.cause {
                    LocationCause::Requested => {
                        if r.epoch != w.epoch {
                            return Err(Violation::World("REQUESTED report with another epoch"));
                        }
                    }
                    LocationCause::Teleported | LocationCause::Corrected => {
                        if r.epoch != w.epoch.wrapping_add(1) {
                            return Err(Violation::World("epoch is not the previous epoch + 1"));
                        }
                        if region != w.region {
                            if r.cause == LocationCause::Corrected {
                                return Err(Violation::World("region changed by a correction"));
                            }
                            if !w.reset_since_report || !w.entities.is_empty() {
                                return Err(Violation::World(
                                    "region change without InterestReset first",
                                ));
                            }
                            w.region = region;
                        }
                        w.epoch = r.epoch;
                        if r.cause == LocationCause::Teleported {
                            // The InterestReset that allows a region change is used up by
                            // the teleport report it precedes (§9.5); a REQUESTED or
                            // CORRECTED report in between does not use it.
                            w.reset_since_report = false;
                            if r.request_id != 0 && w.pending_teleport == Some(r.request_id) {
                                w.pending_teleport = None;
                            }
                        }
                    }
                }
                Ok(Some(ClientEvent::Location(r)))
            }
            Message::TeleportRefused(t) => {
                if w.pending_teleport == Some(t.request_id) {
                    w.pending_teleport = None;
                }
                Ok(Some(ClientEvent::TeleportRefused(t)))
            }
            Message::EntityCreate(c) => {
                if c.id == w.player {
                    return Err(Violation::World("EntityCreate for the player"));
                }
                if c.pose.location.region != w.region {
                    return Err(Violation::World("EntityCreate in another region"));
                }
                if w.entities.contains_key(&c.id) {
                    self.dropped += 1;
                    tracing::warn!(id = c.id, "EntityCreate for an entity already created");
                    return Ok(None);
                }
                if w.entities.len() >= max_entities {
                    return Err(Violation::World("more entities than Hello.max_entities"));
                }
                w.entities.insert(c.id, c.server_time);
                Ok(Some(ClientEvent::EntityCreated(c)))
            }
            Message::EntityUpdate(u) => {
                if u.id != w.player && !w.entities.contains_key(&u.id) {
                    self.dropped += 1;
                    tracing::warn!(id = u.id, "EntityUpdate for an entity not created");
                    return Ok(None);
                }
                Ok(Some(ClientEvent::EntityUpdated(u)))
            }
            Message::EntityAppearance(a) => {
                if a.id != w.player && !w.entities.contains_key(&a.id) {
                    self.dropped += 1;
                    tracing::warn!(id = a.id, "EntityAppearance for an entity not created");
                    return Ok(None);
                }
                Ok(Some(ClientEvent::AppearanceChanged(a)))
            }
            Message::EntityDestroy(d) => {
                if d.id == w.player {
                    return Err(Violation::World("EntityDestroy for the player"));
                }
                if w.entities.remove(&d.id).is_none() {
                    self.dropped += 1;
                    tracing::warn!(id = d.id, "EntityDestroy for an entity not created");
                    return Ok(None);
                }
                Ok(Some(ClientEvent::EntityDestroyed(d)))
            }
            Message::InterestReset => {
                let ids: Vec<u64> = std::mem::take(&mut w.entities).into_keys().collect();
                w.reset_since_report = true;
                Ok(Some(ClientEvent::InterestReset(ids)))
            }
            Message::WorldClock(c) => Ok(Some(ClientEvent::WorldClock(c))),
            other => Err(self.unexpected(other.message_type())),
        }
    }

    fn on_chat(&mut self, msg: Message) -> Result<Option<ClientEvent>, Violation> {
        if !matches!(
            self.state,
            ClientState::CharacterSelect | ClientState::EnteringWorld | ClientState::InWorld
        ) {
            return Err(self.unexpected(msg.message_type()));
        }
        Ok(Some(match msg {
            Message::ChatMessage(m) => ClientEvent::Chat(m),
            Message::ServerNotice(n) => ClientEvent::Notice(n),
            Message::ChatRejected(f) => ClientEvent::ChatRejected(f),
            other => return Err(self.unexpected(other.message_type())),
        }))
    }

    fn on_dgram(&mut self, msg: Message, now: u64) -> Option<ClientEvent> {
        match msg {
            Message::MotionBatch(b) => {
                let Some(w) = self
                    .world
                    .as_mut()
                    .filter(|_| self.state == ClientState::InWorld)
                else {
                    self.dropped += 1;
                    return None;
                };
                let mut fresh = Vec::with_capacity(b.entries.len());
                for e in b.entries {
                    match w.entities.get_mut(&e.id) {
                        Some(last) if b.server_time > *last => {
                            *last = b.server_time;
                            fresh.push(e);
                        }
                        _ => self.dropped += 1,
                    }
                }
                if fresh.is_empty() {
                    None
                } else {
                    Some(ClientEvent::Motion {
                        server_time: b.server_time,
                        entries: fresh,
                    })
                }
            }
            Message::TimeReply(r) => self.on_time_reply(&r, now),
            _ => {
                self.dropped += 1;
                None
            }
        }
    }

    fn on_time_reply(&mut self, r: &TimeReply, now: u64) -> Option<ClientEvent> {
        if !self.state.after_hello_ack() {
            self.dropped += 1;
            return None;
        }
        match self.time.on_reply(r, now) {
            Some(s) => Some(ClientEvent::TimeSample(s)),
            None => {
                self.dropped += 1;
                None
            }
        }
    }

    /// Acts on a violation by the server: closes with its code and returns it. While Closing
    /// or Closed the session is already ending: the input is dropped and `None` returned, so
    /// a queued `Disconnect` is kept and nothing is sent after a `Close`.
    fn fail_violation(&mut self, v: Violation) -> Option<Violation> {
        if matches!(self.state, ClientState::Closing | ClientState::Closed) {
            tracing::debug!(error = %v, state = self.state.name(), "input dropped while closing");
            self.dropped += 1;
            return None;
        }
        tracing::warn!(error = %v, state = self.state.name(), "closing: server broke the protocol");
        self.outbox.clear();
        self.outbox.push(Transmit::Close {
            code: v.app_code(),
            reason: close_reason(&v.to_string()),
        });
        self.state = ClientState::Closed;
        self.deadline = None;
        if self.cause.is_none() {
            self.cause = Some(CloseCause::Violation(v.clone()));
        }
        Some(v)
    }

    // --- sending ---

    fn require(&self, state: ClientState) -> Result<(), UsageError> {
        if self.state == state {
            Ok(())
        } else {
            Err(UsageError::WrongState(self.state.name()))
        }
    }

    fn caps(&self) -> ServerCaps {
        self.hello_ack
            .as_ref()
            .map_or(ServerCaps::NONE, |a| a.server_caps)
    }

    fn send_checked(&mut self, msg: Message) -> Result<(), UsageError> {
        check_encodable_minor(&msg, self.minor())?;
        self.outbox.send(msg);
        Ok(())
    }

    /// Sends `Login` (Authenticating, none outstanding; 30 s answer timeout).
    pub fn login(&mut self, credentials: Credentials, now: u64) -> Result<(), UsageError> {
        self.send_login(Message::Login(credentials), now)
    }

    /// Sends `Register` (needs `REGISTRATION`).
    pub fn register(&mut self, credentials: Credentials, now: u64) -> Result<(), UsageError> {
        if !self.caps().contains(ServerCaps::REGISTRATION) {
            return Err(UsageError::CapabilityMissing);
        }
        self.send_login(Message::Register(credentials), now)
    }

    fn send_login(&mut self, msg: Message, now: u64) -> Result<(), UsageError> {
        self.require(ClientState::Authenticating)?;
        if self.login_outstanding {
            return Err(UsageError::Outstanding);
        }
        if self.login_retry_at.is_some_and(|at| now < at) {
            return Err(UsageError::RateLimited);
        }
        self.send_checked(msg)?;
        self.login_outstanding = true;
        self.deadline = Some(now.saturating_add(micros(LOGIN_ANSWER_TIMEOUT)));
        Ok(())
    }

    /// Sends `CharacterCreate` (CharacterSelect, `CHARACTER_CREATION`, none outstanding)
    /// without a look.
    pub fn create_character(
        &mut self,
        name: String,
        appearance: Appearance,
    ) -> Result<(), UsageError> {
        self.create_character_with_look(name, appearance, Look::none())
    }

    /// Sends `CharacterCreate` with a look (§6.2.7). The look travels only at negotiated
    /// minor ≥ 1; at minor 0 it is left off (§3.1).
    pub fn create_character_with_look(
        &mut self,
        name: String,
        appearance: Appearance,
        look: Look,
    ) -> Result<(), UsageError> {
        self.require(ClientState::CharacterSelect)?;
        if !self.caps().contains(ServerCaps::CHARACTER_CREATION) {
            return Err(UsageError::CapabilityMissing);
        }
        if self.create_outstanding {
            return Err(UsageError::Outstanding);
        }
        self.send_checked(Message::CharacterCreate(CharacterCreate {
            name,
            appearance,
            look,
        }))?;
        self.create_outstanding = true;
        Ok(())
    }

    /// Sends `EnterWorld` (CharacterSelect, nothing outstanding; at most once per
    /// connection; 30 s timeout).
    pub fn enter_world(&mut self, name: String, now: u64) -> Result<(), UsageError> {
        self.require(ClientState::CharacterSelect)?;
        if self.create_outstanding {
            return Err(UsageError::Outstanding);
        }
        if self.world_stream.is_some() || self.world.is_some() {
            return Err(UsageError::WrongState("world already entered"));
        }
        self.send_checked(Message::EnterWorld(EnterWorld { name }))?;
        self.state = ClientState::EnteringWorld;
        self.deadline = Some(now.saturating_add(micros(ENTER_WORLD_TIMEOUT)));
        Ok(())
    }

    fn require_in_world(&self) -> Result<(), UsageError> {
        self.require(ClientState::InWorld)
    }

    fn take_request_id(&mut self) -> u32 {
        let id = self.next_request;
        self.next_request = self.next_request.wrapping_add(1).max(1);
        id
    }

    /// Sends `TeleportRequest`; returns its request ID. A newer request supersedes an
    /// unresolved one (the server cancels it).
    pub fn teleport(&mut self, destination: Destination) -> Result<u32, UsageError> {
        self.require_in_world()?;
        let request_id = self.take_request_id();
        self.send_checked(Message::TeleportRequest(TeleportRequest {
            request_id,
            destination,
        }))?;
        if let Some(w) = self.world.as_mut() {
            w.pending_teleport = Some(request_id);
        }
        Ok(request_id)
    }

    /// Sends `QueryLocation`; returns its request ID.
    pub fn query_location(&mut self) -> Result<u32, UsageError> {
        self.require_in_world()?;
        let request_id = self.take_request_id();
        self.send_checked(Message::QueryLocation { request_id })?;
        Ok(request_id)
    }

    /// Sends `ChatSend`. System is refused.
    pub fn chat(&mut self, channel: ChatChannel, text: String) -> Result<(), UsageError> {
        self.require_in_world()?;
        self.send_checked(Message::ChatSend(ChatSend { channel, text }))
    }

    /// Sends `EmoteSet` (anim 0 stops the emote).
    pub fn emote(&mut self, anim: u32, looping: bool) -> Result<(), UsageError> {
        self.require_in_world()?;
        self.send_checked(Message::EmoteSet(EmoteSet { anim, looping }))
    }

    /// Sends `StanceSet`.
    pub fn stance(&mut self, stance: Stance) -> Result<(), UsageError> {
        self.require_in_world()?;
        self.send_checked(Message::StanceSet(stance))
    }

    /// Sends `AppearanceSet` (needs `APPEARANCE_CHANGE`).
    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<(), UsageError> {
        self.require_in_world()?;
        if !self.caps().contains(ServerCaps::APPEARANCE_CHANGE) {
            return Err(UsageError::CapabilityMissing);
        }
        self.send_checked(Message::AppearanceSet(appearance))
    }

    /// Sends `Ping` (after `HelloAck`, at most one per second); returns its nonce.
    pub fn ping(&mut self, now: u64) -> Result<u64, UsageError> {
        if !self.state.after_hello_ack() {
            return Err(UsageError::WrongState(self.state.name()));
        }
        if !self.ping_rate.try_record(now) {
            return Err(UsageError::RateLimited);
        }
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1);
        self.outbox.send(Message::Ping { nonce });
        self.ping_outstanding = Some((nonce, now));
        Ok(nonce)
    }

    /// Sends a `MoveInput` datagram with the next `seq` and the current epoch (InWorld, at
    /// most 30 per second). Use [`crate::movement::MoveScheduler`] for the 15 Hz / 1 Hz
    /// rules.
    pub fn move_input(
        &mut self,
        location: Location,
        heading: f32,
        velocity: Vec3,
        motion: Motion,
        now: u64,
    ) -> Result<(), UsageError> {
        self.require_in_world()?;
        let epoch = self.world.as_ref().map_or(0, |w| w.epoch);
        let msg = Message::MoveInput(MoveInput {
            seq: self.move_seq,
            epoch,
            location,
            heading,
            velocity,
            motion,
        });
        check_encodable(&msg)?;
        if !self.move_rate.try_record(now) {
            return Err(UsageError::RateLimited);
        }
        self.move_seq = self.move_seq.wrapping_add(1);
        self.outbox.send(msg);
        Ok(())
    }

    /// Whether a `TimeRequest` is due at client time `now` (§11.2 schedule).
    #[must_use]
    pub fn time_request_due(&self, now: u64) -> bool {
        self.state.after_hello_ack() && self.time.request_due(now)
    }

    /// Sends a `TimeRequest` datagram stamped with `now` (after `HelloAck`).
    pub fn time_request(&mut self, now: u64) -> Result<(), UsageError> {
        if !self.state.after_hello_ack() {
            return Err(UsageError::WrongState(self.state.name()));
        }
        self.time.on_request_sent(now);
        self.outbox.send(Message::TimeRequest { client_time: now });
        Ok(())
    }

    /// Advances the clock-offset slew and returns the estimated server time at client time
    /// `now` (§11.2).
    pub fn server_time(&mut self, now: u64) -> Option<u64> {
        self.time.update(now);
        self.time.server_time(now)
    }

    /// Disconnects (§12.3): sends `Disconnect(code)` and finishes the control stream when
    /// `Hello` was sent, else closes at once. A user-requested disconnect uses `NO_ERROR`.
    pub fn disconnect(&mut self, code: AppErrorCode, text: &str, now: u64) -> Option<ClientEvent> {
        match self.state {
            ClientState::Closing | ClientState::Closed => None,
            ClientState::Idle | ClientState::Handshaking => {
                self.outbox.push(Transmit::Close {
                    code,
                    reason: close_reason(text),
                });
                self.state = ClientState::Closed;
                self.deadline = None;
                let cause = CloseCause::LocalDisconnect { code };
                self.cause = Some(cause.clone());
                Some(ClientEvent::Closed(cause))
            }
            _ => {
                let text = crate::text::sanitise(text);
                let mut text = text.into_owned();
                text.truncate(floor_boundary(&text, crate::limits::MAX_LONG_TEXT_LEN));
                self.outbox.clear();
                self.outbox
                    .send(Message::Disconnect(Disconnect { code, text }));
                self.outbox.push(Transmit::FinishControl);
                self.state = ClientState::Closing;
                self.deadline = Some(now.saturating_add(micros(CLOSE_LINGER)));
                let cause = CloseCause::LocalDisconnect { code };
                self.cause = Some(cause.clone());
                Some(ClientEvent::Closed(cause))
            }
        }
    }
}

pub(crate) fn floor_boundary(s: &str, max: usize) -> usize {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Checks that a message encodes in the v1.0 layout (so a send never fails later in the
/// network layer).
pub(crate) fn check_encodable(msg: &Message) -> Result<(), UsageError> {
    check_encodable_minor(msg, 0)
}

/// Checks that a message encodes under negotiated minor `minor`.
pub(crate) fn check_encodable_minor(msg: &Message, minor: u16) -> Result<(), UsageError> {
    let mut scratch = Vec::new();
    msg.encode_body_minor(&mut scratch, minor)?;
    let limit = if msg.channel().is_stream() {
        crate::limits::MAX_FRAME_BODY
    } else {
        crate::limits::MAX_DATAGRAM - crate::limits::TYPE_FIELD
    };
    if scratch.len() > limit {
        return Err(UsageError::Invalid(if msg.channel().is_stream() {
            ProtocolError::BodyTooLarge(scratch.len())
        } else {
            ProtocolError::DatagramTooLarge(scratch.len() + crate::limits::TYPE_FIELD)
        }));
    }
    Ok(())
}

/// A convenience for login failure handling: whether the client may send another `Login`
/// after this failure (the server follows `SERVER_FULL` and `SERVER_ERROR` with
/// `Disconnect`).
#[must_use]
pub fn login_retry_allowed(code: LoginFailedCode) -> bool {
    !matches!(
        code,
        LoginFailedCode::ServerFull | LoginFailedCode::ServerError
    )
}
