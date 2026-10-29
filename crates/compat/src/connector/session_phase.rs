// SPDX-License-Identifier: GPL-3.0-or-later
//! The UDP part of the live session: the login connection with its version exchange
//! (`official-session.md` §6, §9.3), the character list and world entry
//! (`official-messages-world.md` §1.1.4, §3, §4), the world connection after the referral
//! (`official-session.md` §6.5), the play messages through
//! [`crate::play::adapter::PlayAdapter`] (`official-messages-play.md` §4–§8), the chat
//! server (§6.6) and the in-game ticket refresh (`official-login.md` §4.3).
//!
//! # One multiplexer per connection
//!
//! The login connection and every world connection each get a multiplexer of their own,
//! all on the session's one socket. `official-session.md` §3.4 does not say whether a world
//! server may assign the client the recipient ID the login connection has (or had): a
//! shared multiplexer would refuse such a challenge (the ID is in use, or retired once
//! removed). With one multiplexer per connection the IDs of one server never meet another's
//! (engineering choice, spec silent (question with the spec writers); tested). A datagram
//! goes to the connection whose recipient ID it names and whose server address it comes
//! from (§3.4 step 2), else to the newest connection with that server address (step 3): the
//! world connection, then the replaced ones (newest first), then the login connection.
//!
//! A datagram that names the recipient ID of a connection but comes from another address is
//! discarded and counted ([`DriverCounters::address_mismatches`]): this is how Tessera reads
//! step 2's "it also verifies that the packet's source IPv4 address and UDP port match"
//! (a failed check is not a lookup miss, so step 3 does not apply). One exception follows
//! from the multiplexer per connection: while a connection from that address still waits
//! for its server's challenge (state `SessionAuthSent`), its server cannot know any ID yet
//! and may name one another server assigned; such a datagram goes to step 3. Engineering
//! choice, spec silent (question with the spec writers).
//!
//! # Bounded events
//!
//! Once the driver holds [`DRIVER_EVENT_CAP`] events it reads no datagram (from the session
//! socket or the chat server's), takes no session event from its connections and leaves the
//! messages of a received blob in its inbox (at most one blob's messages, which the session
//! layer bounds by bytes) until events are taken. It goes over the cap only by what one
//! unit of work adds after the last check: the events of one message (on world entry, of
//! the at most [`MAX_ENTRY_MESSAGES`] messages held for it), the events of one chat-server
//! datagram (the chat client is let read one datagram at a time, [`CHAT_DATAGRAMS_PER_STEP`]
//! at most per pass, and is not read at the cap), and the events a pass makes without a
//! datagram: a command's `EnterWorldFailed`, the entry deadline's, and the session's end
//! (`LoginRefused` with its `Disconnected`).
//!
//! # Leaving the world
//!
//! `Command::LeaveWorld` sends the logout request (`official-messages-play.md` §7.2 item 1).
//! A world teardown (§7.2 item 3) keeps the session: the client unloads the world on the
//! notice, so the driver goes back to character selection at once with
//! `Event::ReturnedToCharacterSelect` and the latest list it has (a `0x6C` that came while
//! in the world counts). The `0x6C` that follows the teardown is then a later list: its
//! changes surface as `CharacterInfo` (new names) and `CharacterRemoved` (names gone), as
//! every later list at character selection does. That row mapping is an engineering choice
//! pending a spec answer (`docs/m8-remediation.md` §15, P §8): §8 maps a later list only to
//! `Connected` or `CharacterInfo`, and says neither whether a later list replaces the whole
//! list nor how a name that is gone is surfaced. Moves and chat lines that reach the driver
//! after the teardown (sent before the client saw the return) are refused and counted
//! ([`DriverCounters::outside_world`]), never sent; a refused move stays silent, a refused
//! chat line also gives a system notice (`Event::Notice`), so the player sees that the line
//! was not sent. `ReturnedToCharacterSelect` is Tessera's neutral addition (§8 maps the
//! teardown to the later list; engineering choice pending a spec answer, listed in
//! `docs/m8-remediation.md` §15).
//!
//! # Entities
//!
//! In the world, batches (`0x0D`, `0x4C`, `official-messages-world.md` §5.1) are unpacked
//! in order with their target (§1.1.3; the one exception, a `0x52` directly after a `0x98`
//! in a `0x0D` to the default recipient, is [`super::entities::sub_event_target`]), the
//! entity lifecycle and property events (`0x98`, `0x40`, `0x52`, `0x88`, `0x92`, `0x0E`,
//! `0xE7`, `0x89`) go to the [`EntityFeed`] and the rest to the play layer. The feed turns
//! them into `EntityCreated`, `AppearanceChanged` and `EntityDespawned` ([`super::entities`]
//! has the mapping); the positions of every entity but the local player go to the feed too:
//! they place the entities it waits for, and the remote motion of created entities (`0xCE`,
//! `0xBF`, and `0x59` as a move to its destination) is `EntityMoved`. `0xED` and `0xBD`
//! reach no neutral event and are counted ([`FeedCounters`]).
//!
//! A `0x0D` inside a batch is read with a 1-byte tail flag after its frames or without
//! one, each counted: `official-messages-world.md` §5.1 covers only a top-level `0x0D`
//! (spec silent; question in `docs/m8-remediation.md` §15 (W §5.1)). Only the feed and batch messages
//! must start with `base_flags = 0` here; the play decoders check their own bodies.
//!
//! The feed's events of one top-level message are made when it has been read
//! ([`EntityFeed::flush`]: at most one creation, one appearance change and one move per
//! entity), and the play-layer events of one message are capped at
//! [`MAX_MESSAGE_PLAY_EVENTS`] (the rest are dropped and counted,
//! [`DriverCounters::message_events_dropped`]). Once a message has reached that cap, the
//! texts of its further play messages are not looked up at all (no string-table read for
//! an event that would be dropped; counted, [`DriverCounters::texts_skipped`]), while their
//! signals (such as the world teardown) and an end of the session are still handled. With
//! the despawns (one per entity shown before the message), that bounds what one message
//! adds over [`DRIVER_EVENT_CAP`] (module docs "Bounded events"). Creations whose template
//! or name table the install has not read yet wait for a later pass
//! ([`super::entities::MAX_TEMPLATE_LOADS_PER_PASS`],
//! [`super::entities::MAX_TEXT_LOADS_PER_PASS`]), retried once at the start of each pass.
//!
//! Portal space ends on the client's `Command::WorldLoaded` or after
//! [`PORTAL_LOADING_TIMEOUT_SEC`] (§4.5 item 4). Each portal space starts with a fresh
//! loading state; the client sends one `WorldLoaded` per placement, and the driver takes
//! only one that arrives while it is in portal space (one signal per portal space; a
//! signal outside portal space is ignored).
//!
//! # Ending the session
//!
//! [`SessionDriver::begin_shutdown`] sends the transport disconnect only on connected
//! connections (and lets those already disconnecting finish). A connection still in its
//! handshake (`SessionAuthSent`, `ChallengeAcceptSent`), the chat server's included, is
//! closed locally: nothing more is sent on it (no retransmitted `SessionAuthHeader` or
//! `ReferralAuthHeader`, so no ticket), and a challenge that arrives for it afterwards finds
//! no connection and is not answered. Engineering choice, spec silent (question with the
//! spec writers): `official-session.md` §8.2 describes the disconnect of a connected
//! session only. [`SessionDriver::abandon`] ends everything without sending, for a revoked
//! network right.
//!
//! # Handshake diagnostics
//!
//! Each connection logs a start line, a `debug` line every
//! [`super::diag::HANDSHAKE_REPORT_SEC`] while it is in its handshake, and one `info` line
//! with its counts when it closes from its handshake ([`super::diag`] has the fields;
//! [`SessionDriver::handshake_report`] gives them to tests). Every datagram the session
//! sends uses [`SessionStart::send_checksum`], the chat-server connection's too
//! ([`ChatConfig::with_send_checksum`]); the receive rule never changes. A cancel or a
//! revoked network right during a handshake logs the counts once
//! ([`super::diag::HandshakeReport::log_ended`]).

use std::collections::VecDeque;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

use tessera_net::account::{AccountEvent, LoginRefusal, LoginStage};
use tessera_net::model::{MAX_NAME_LEN, is_valid_name};
use tessera_net::{
    CharacterEvent, CharacterSummary, ChatChannel, Command, DisconnectReason, EntityId, Event,
    NoticeLevel, Transform, Vec3,
};

use crate::chat_session::state::INITIAL_CONNECT_POLICY;
use crate::chat_session::{ChatClient, ChatCodecs, ChatConfig};
use crate::login::{
    HttpResponse, LoginError, RefreshOutcome, SessionHandoff, TicketRefresher, VersionInfo,
    zeroize_vec,
};
use crate::play::adapter::{
    PlayAdapter, PlayCounters, PlaySignal, placement_location, position_location, velocity_or_zero,
};
use crate::play::chat::TextResolver;
use crate::play::inbound::{PlayMessage, decode_play_message};
use crate::play::movement::{MovementInput, PredictedState};
use crate::play::{OutgoingEvent, clean_display_name};
use crate::session::{
    ClientVersionMessage, ConnectionState, ControlMessage, DEFAULT_MAX_CONTAINER_DEPTH,
    DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES, DisconnectReason as LinkEnd, MAX_DATAGRAM_BUF,
    MAX_LOCAL_REC_ID, MIN_DATAGRAM_BYTES, MIN_LOCAL_REC_ID, OPCODE_CLIENT_VERSION,
    OPCODE_LOGIN_QUEUE_STATUS, SendChecksumRule, SessionError, SessionEvent, SessionMultiplexer,
    UdpDatagramTransport, dispatch_message,
};
use crate::world::{
    AccountEvictionNotice, CharacterEnterWorldRequest, CharacterListResponse,
    CharacterOperationError, EntityCreate, EntityDespawn, PlayerSessionReady, PropertyRemoval,
    PropertyScope, PropertyTableUpdate, SinglePropertyUpdate, SubEventBatch, SubEventFrame,
    VisualDescriptionUpdate, WireReader, WorldError, encode_event_envelope,
};

use super::data::{SchemaView, chat_categories};
use super::diag::{
    AuthMode, BindCase, HANDSHAKE_REPORT_SEC, HandshakeReport, LinkDiag, LocalSource, RejectKind,
    RemoteForm, RemoteName, carries_challenge,
};
use super::entities::{
    CreateOutcome, EntityFeed, FeedCounters, FeedOutput, SubTarget, sub_event_target,
};
use super::login_phase::PlainSteps;
use super::net::{ConnectorNet, SendHttpClient};

/// Delivery queue of every message the client sends on the login and world connections.
/// Engineering choice, spec silent (question with the spec writers; `official-session.md`
/// §5.6.2 names the queues, not which message uses which): queue 2 (`ReliableOrdered`), as
/// the chat client does.
pub const SEND_QUEUE: u8 = 2;

/// Most datagrams read per step.
pub const MAX_DATAGRAMS_PER_STEP: usize = 512;

/// Most events the driver lets wait before it stops reading (module docs "Bounded events";
/// `ARCHITECTURE.md` §19.4: reading stops once 256 items wait).
pub const DRIVER_EVENT_CAP: usize = super::READ_PAUSE;

/// The system notice for a chat line refused because the session left the world before
/// the client saw the return (module docs "Leaving the world"). Tessera's text, an
/// engineering choice (spec silent).
pub const NOT_SENT_OUTSIDE_WORLD: &str = "Your line was not sent: you are no longer in the world.";

/// Most old world connections kept (a later referral replaces the world connection).
pub const MAX_OLD_LINKS: usize = 4;

/// Most characters kept from one character list (the list's own bound is the bytes of the
/// message; this bounds the names the session offers).
pub const MAX_CHARACTERS: usize = 256;

/// The two wire values of `TransportErrorHeader` that mean "connection refused /
/// authentication rejected during login" (`official-session.md` §6.7).
pub const LOGIN_REFUSAL_CODES: [u32; 2] = [0x00A7_E948, 0x7DC4_1B4A];

/// Longest wait for the world after `EnterWorld` (`0x6F`) before the entry fails and the
/// character list is offered again. Engineering choice, spec silent (question with the spec
/// writers).
pub const ENTER_WORLD_TIMEOUT_SEC: f64 = 60.0;

/// Longest wait in portal space for the client's [`Command::WorldLoaded`] before portal space
/// ends anyway (`official-messages-play.md` §4.5 item 4: "or the portal-space loading timeout
/// expires"; the spec gives no value). Engineering choice pending a spec answer (listed in
/// `docs/m8-remediation.md` §15).
pub const PORTAL_LOADING_TIMEOUT_SEC: f64 = 30.0;

/// Deepest batch nesting read (`official-messages-world.md` §5.1; §7.1 bounds recursion):
/// a top-level batch is depth 0, so a `0x4C` inside it is depth 1. Engineering choice,
/// spec silent.
pub const MAX_BATCH_DEPTH: usize = 4;

/// Most play-layer events one top-level world message makes (a batch can hold many chat
/// lines); the rest are dropped and counted. Engineering choice, spec silent.
pub const MAX_MESSAGE_PLAY_EVENTS: usize = DRIVER_EVENT_CAP;

/// Most play messages kept while the world is being entered (those before `0x90`, such as
/// `0x43`), handed to the play layer once the world is entered. Engineering choice, spec
/// silent.
pub const MAX_ENTRY_MESSAGES: usize = 64;

/// Most bytes of the play messages kept while the world is being entered.
pub const MAX_ENTRY_BYTES: usize = 256 * 1024;

/// Shortest time between two log lines of one kind of refused datagram; the lines between
/// are counted (engineering choice).
pub const LOG_INTERVAL_SEC: f64 = 10.0;

/// The wait before a chat-server attachment that failed is tried again: the retry interval
/// of the Initial Connect Policy (`official-messages-play.md` §6.6.3, 30 s). §6.6.3 does not
/// say when the initial delay and when the retry interval applies to an attempt that could
/// not even bind: engineering choice, spec silent (question with the spec writers).
pub const CHAT_RETRY_SEC: f64 = INITIAL_CONNECT_POLICY.retry_interval_sec;

/// Most chat-server datagrams read in one pass (module docs "Bounded events"). Engineering
/// choice.
pub const CHAT_DATAGRAMS_PER_STEP: usize = 64;

/// What the refresh thread sends back: the answer and the client, for the next refresh.
type RefreshReply = (Result<HttpResponse, LoginError>, SendHttpClient);

/// A chat-server socket that hands out only as many datagrams as the driver allows
/// (module docs "Bounded events"). Every socket of the chat client (the current one and the
/// old ones finishing their disconnect) shares one allowance: a read with no allowance left
/// finds nothing, and the datagram waits in the socket.
#[derive(Debug)]
pub struct Metered<T> {
    inner: T,
    allowance: Arc<AtomicUsize>,
}

impl<T: UdpDatagramTransport> UdpDatagramTransport for Metered<T> {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        self.inner.local_addr()
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        self.inner.send_to(buf, target)
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        if self.allowance.load(Ordering::SeqCst) == 0 {
            return Ok(None);
        }
        let got = self.inner.recv_from(buf)?;
        if got.is_some() {
            // One thread polls the chat client: a load and a store cannot race.
            let left = self.allowance.load(Ordering::SeqCst).saturating_sub(1);
            self.allowance.store(left, Ordering::SeqCst);
        }
        Ok(got)
    }
}

/// One log line per [`LOG_INTERVAL_SEC`] for a kind of event; the rest are counted.
#[derive(Debug, Clone, Copy, Default)]
struct LogLimit {
    next: f64,
    suppressed: u64,
}

impl LogLimit {
    /// Whether to log now, with how many lines were left out since the last.
    fn allow(&mut self, now: f64) -> Option<u64> {
        if now < self.next {
            self.suppressed = self.suppressed.saturating_add(1);
            return None;
        }
        self.next = now + LOG_INTERVAL_SEC;
        Some(std::mem::take(&mut self.suppressed))
    }
}

/// The ticket refresh running on its thread.
#[derive(Debug)]
struct Refreshing {
    rx: mpsc::Receiver<RefreshReply>,
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

/// One connection with its own multiplexer (module docs), and its handshake diagnostics
/// ([`super::diag`]).
struct Link {
    mux: SessionMultiplexer,
    conn: u16,
    peer: SocketAddrV4,
    diag: LinkDiag,
    auth: AuthMode,
    remote: (RemoteForm, RemoteName),
}

impl std::fmt::Debug for Link {
    // No peer address: a `Debug` form can reach a log line, which never names one
    // (decision 0044 item 3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("mux", &self.mux)
            .field("conn", &self.conn)
            .field("diag", &self.diag)
            .field("auth", &self.auth)
            .field("remote", &self.remote)
            .finish_non_exhaustive()
    }
}

impl Link {
    /// A connection to `peer` (of the address form `remote`) whose datagrams use the send
    /// checksum `rule`.
    fn new(
        peer: SocketAddrV4,
        uuids: &[[u8; 16]],
        rule: SendChecksumRule,
        auth: AuthMode,
        remote: (RemoteForm, RemoteName),
        now: f64,
    ) -> Result<Self, SessionError> {
        let mut mux = SessionMultiplexer::new();
        let conn = mux.create_connection(peer, now)?;
        if let Some(c) = mux.connection_mut(conn) {
            for u in uuids {
                c.register_codec_uuid(*u);
            }
            c.set_send_checksum_rule(rule);
        }
        Ok(Self {
            mux,
            conn,
            peer,
            diag: LinkDiag::new(now),
            auth,
            remote,
        })
    }

    /// The `ConnectChallengeHeader`s the connection acted on so far.
    fn challenges(&self) -> u64 {
        self.mux
            .connection(self.conn)
            .map_or(0, |c| c.handshake_counts().challenges)
    }

    /// Whether the connection closed from its handshake (States `1` and `2`).
    fn closed_in_handshake(&self) -> bool {
        matches!(
            self.mux.connection(self.conn).and_then(|c| c.closed_from()),
            Some(ConnectionState::SessionAuthSent | ConnectionState::ChallengeAcceptSent)
        )
    }

    fn state(&self) -> Option<ConnectionState> {
        self.mux.connection(self.conn).map(|c| c.state())
    }

    fn live(&self) -> bool {
        matches!(
            self.state(),
            Some(
                ConnectionState::SessionAuthSent
                    | ConnectionState::ChallengeAcceptSent
                    | ConnectionState::Connected
                    | ConnectionState::DisconnectingFlush
                    | ConnectionState::DisconnectSent
            )
        )
    }

    fn flushing(&self) -> bool {
        matches!(
            self.state(),
            Some(ConnectionState::DisconnectingFlush | ConnectionState::DisconnectSent)
        )
    }

    fn disconnect(&mut self, now: f64) {
        if let Some(c) = self.mux.connection_mut(self.conn) {
            c.request_disconnect(now);
        }
    }

    /// Whether the connection is still in its handshake (`SessionAuthSent`,
    /// `ChallengeAcceptSent`).
    fn in_handshake(&self) -> bool {
        matches!(
            self.state(),
            Some(ConnectionState::SessionAuthSent | ConnectionState::ChallengeAcceptSent)
        )
    }

    /// Closes the connection locally without sending anything (module docs "Ending the
    /// session"): it is taken out of its multiplexer, which refuses later datagrams for its
    /// ID, and its unsent datagrams (which may hold the ticket) are wiped.
    fn close_quietly(&mut self) {
        if let Some(mut c) = self.mux.remove_connection(self.conn) {
            while let Some(mut d) = c.pop_outbound_datagram() {
                zeroize_vec(&mut d);
            }
        }
    }

    /// The end of the session for this connection: the graceful disconnect when it is
    /// connected, a local close when it is still in its handshake.
    fn end_session(&mut self, now: f64) {
        if self.in_handshake() {
            self.close_quietly();
        } else {
            self.disconnect(now);
        }
    }

    /// Whether the connection still waits for its server's challenge (no ID assigned yet).
    fn awaiting_challenge(&self) -> bool {
        self.state() == Some(ConnectionState::SessionAuthSent)
    }

    fn pop_event(&mut self) -> Option<SessionEvent> {
        self.mux.connection_mut(self.conn)?.pop_event()
    }

    /// Drops the waiting events (a replaced connection, or the shutdown).
    fn drain_events(&mut self) {
        while self.pop_event().is_some() {}
    }
}

/// A message of a received blob, waiting for [`SessionDriver::message`] (module docs
/// "Bounded events").
#[derive(Debug)]
struct Inbound {
    opcode: u32,
    recipient: u64,
    body: Vec<u8>,
    world: bool,
}

/// Where a received datagram goes (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// To the connection with this index (`0` login, `1` world, `2 + i` replaced one `i`).
    Link(usize),
    /// It names a connection's ID from another address: discarded.
    AddressMismatch,
    /// No connection has its address: discarded.
    Unknown,
}

/// Where the session is (`official-messages-world.md` §3.6).
#[derive(Debug, Clone, PartialEq)]
enum Stage {
    /// The login connection's handshake and version exchange; `Connected` not yet sent.
    Login,
    /// The character list was delivered (`Connected` sent).
    Characters,
    /// `0x6F` sent for the character; waiting for the world.
    Entering {
        /// The character's ID.
        id: u64,
        /// Its name.
        name: String,
        /// When `0x6F` was sent.
        since: f64,
    },
    /// In the world as `player`.
    InWorld {
        /// The player entity.
        player: u64,
        /// The character's name.
        name: String,
    },
}

/// What the driver needs to start.
#[derive(Debug)]
pub struct SessionStart {
    /// The hand-off values (`official-login.md` §4.2).
    pub handoff: SessionHandoff,
    /// The login server's address, resolved before the hand-off (an unusable address is a
    /// refusal of the chosen world, `official-login.md` §5.1 `S7`).
    pub login_peer: SocketAddrV4,
    /// The steps of this login that went over plain HTTP: the ticket refresh asks its URL
    /// as every step of the chain does (`crate::connector::login_phase` "HTTPS first, then
    /// the address as given") and logs its own step once.
    pub plain: PlainSteps,
    /// The ticket refresh (`official-login.md` §4.3).
    pub refresher: TicketRefresher,
    /// The chat server's address, resolved before the hand-off with the login server's
    /// ([`resolve_chat_endpoint`]); `None` without one.
    pub chat_endpoint: Option<SocketAddrV4>,
    /// The version string's parts (§2.4.1).
    pub version: VersionInfo,
    /// The wire codecs.
    pub codecs: ChatCodecs,
    /// The dictionaries' `codecUuid`s.
    pub codec_uuids: Vec<[u8; 16]>,
    /// The install's tables.
    pub schema: SchemaView,
    /// The attempt's cancel flag (the connector's): the ticket refresh thread looks at it
    /// right before its request.
    pub cancel: Arc<AtomicBool>,
    /// The `payloadSum` rule of every datagram the session sends (`official-session.md`
    /// §4.4.2; [`super::diag`] "The send checksum switch"); the default is the spec's
    /// sender rule.
    pub send_checksum: SendChecksumRule,
}

/// Why the session could not start (before any datagram was sent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartFailure {
    /// The login server's address is unusable (`F06`).
    Address,
    /// The socket could not be bound (or the network right was revoked).
    Bind,
    /// The ticket cannot be sent (`F09`).
    Ticket,
    /// The attempt was cancelled before anything was sent.
    Cancelled,
}

/// Counters for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DriverCounters {
    /// Datagrams refused by the session layer.
    pub refused_datagrams: u64,
    /// Datagrams from an address no connection has.
    pub unknown_sources: u64,
    /// Datagrams naming a connection's recipient ID from another address (module docs).
    pub address_mismatches: u64,
    /// Referrals ignored because the attempt was cancelled.
    pub cancelled_referrals: u64,
    /// Messages that failed to decode.
    pub decode_errors: u64,
    /// Opcodes this driver does not handle.
    pub unhandled: u64,
    /// Outgoing messages that could not be queued.
    pub send_failures: u64,
    /// Referrals outside world entry and the world, ignored.
    pub stray_referrals: u64,
    /// Play messages dropped while the world was being entered (over the bounds).
    pub entry_overflow: u64,
    /// Failed chat-server attachments.
    pub chat_failures: u64,
    /// Socket reads that failed (diagnostics, [`super::diag`]).
    pub recv_errors: u64,
    /// Datagrams from another address (counted in `unknown_sources` or
    /// `address_mismatches`) that carried a `ConnectChallengeHeader` while a connection was
    /// in its handshake (diagnostics, [`super::diag`]).
    pub challenge_from_other_address: u64,
    /// Moves and chat lines that arrived outside the world (after a teardown, before the
    /// client saw the return), refused and not sent.
    pub outside_world: u64,
    /// `WorldLoaded` signals outside portal space, ignored.
    pub stray_loaded: u64,
    /// `0x89` descriptors with a base model, which cannot be decoded (the `model_b` gate
    /// is OPEN, `official-messages-world.md` §2.8); also counted in `decode_errors`.
    pub appearance_base_model: u64,
    /// Play-layer events dropped over [`MAX_MESSAGE_PLAY_EVENTS`] in one message.
    pub message_events_dropped: u64,
    /// Texts not looked up because their message had reached [`MAX_MESSAGE_PLAY_EVENTS`]
    /// (the play layer may count them as unresolved too).
    pub texts_skipped: u64,
}

/// The resolver of a message past [`MAX_MESSAGE_PLAY_EVENTS`]: it looks nothing up and
/// counts the texts it was asked for (module docs "Entities").
#[derive(Debug, Default)]
struct SkippedTexts(std::cell::Cell<u64>);

impl TextResolver for SkippedTexts {
    fn resolve(&self, _text: &crate::world::LocalizedTokenText) -> Option<String> {
        self.0.set(self.0.get().saturating_add(1));
        None
    }
}

/// The UDP session (module docs).
pub struct SessionDriver<N: ConnectorNet> {
    transport: N::Udp,
    codecs: ChatCodecs,
    codec_uuids: Vec<[u8; 16]>,
    login: Link,
    world: Option<Link>,
    old: Vec<Link>,
    version: VersionInfo,
    version_sent: bool,
    schema: SchemaView,
    stage: Stage,
    characters: Vec<(u64, String)>,
    /// The latest character list came on the world connection (after a return from the
    /// world): `0x6F` goes there then.
    list_on_world: bool,
    /// The client's [`Command::WorldLoaded`] for the current portal space.
    loaded: bool,
    /// When the current portal space was first seen (for [`PORTAL_LOADING_TIMEOUT_SEC`]).
    portal_since: Option<f64>,
    adapter: PlayAdapter,
    feed: EntityFeed,
    /// Play-layer events the current top-level world message made.
    message_events: usize,
    chat: Option<ChatClient<Metered<N::Udp>>>,
    chat_allowance: Arc<AtomicUsize>,
    chat_endpoint: Option<SocketAddrV4>,
    chat_retry_at: f64,
    chat_warned: bool,
    handoff: SessionHandoff,
    refresher: TicketRefresher,
    plain: PlainSteps,
    refreshing: Option<Refreshing>,
    background_http: Option<SendHttpClient>,
    cancel: Arc<AtomicBool>,
    client_timestamp: u32,
    events: VecDeque<Event>,
    ended: bool,
    shutting_down: bool,
    counters: DriverCounters,
    buf: Vec<u8>,
    pending_out: VecDeque<OutgoingEvent>,
    inbox: VecDeque<Inbound>,
    entry_messages: Vec<(u32, u64, Vec<u8>)>,
    entry_bytes: usize,
    refused_log: LogLimit,
    unknown_log: LogLimit,
    send_log: LogLimit,
    send_checksum: SendChecksumRule,
    bind: BindCase,
    local_source: LocalSource,
}

impl<N: ConnectorNet> std::fmt::Debug for SessionDriver<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionDriver")
            .field("stage", &self.stage)
            .field("login", &self.login.state())
            .field("world", &self.world.as_ref().and_then(Link::state))
            .field("chat", &self.chat.is_some())
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

impl<N: ConnectorNet> Drop for SessionDriver<N> {
    fn drop(&mut self) {
        self.stop_refresh();
    }
}

fn unix_seconds() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX))
}

/// A character name as the session offers it: cleaned and cut to 64 bytes; `None` when it
/// is not a valid name then.
fn character_name(name: &str) -> Option<String> {
    let n = clean_display_name(name, MAX_NAME_LEN);
    is_valid_name(&n).then_some(n)
}

/// The `0x24` input of a neutral move (`official-messages-play.md` §8; the mapping of a
/// position and velocity onto the input axes is not in the spec). Engineering choice, spec
/// silent (question with the spec writers): the velocity's direction relative to the
/// heading, as forward and strafe axes in `[-1, 1]`, no turn, movement state `0`.
#[must_use]
pub fn movement_input(heading: f32, velocity: Vec3) -> MovementInput {
    let (vx, vy) = (velocity.x, velocity.y);
    let speed = (vx * vx + vy * vy).sqrt();
    if !speed.is_finite() || speed < 1e-3 || !heading.is_finite() {
        return MovementInput::default();
    }
    // `tessera_net::model`: heading θ faces (cos θ, sin θ); +X east, +Y north, +Z up. The
    // strafe axis is positive to the right of the facing.
    let (s, c) = heading.sin_cos();
    let forward = (vx * c + vy * s) / speed;
    let strafe = (vx * s - vy * c) / speed;
    MovementInput {
        move_state: 0,
        strafe: strafe.clamp(-1.0, 1.0),
        forward: forward.clamp(-1.0, 1.0),
        turn: 0.0,
    }
}

/// The chat server's address (`official-messages-play.md` §6.1.1: `--chatserver`, which the
/// launcher fills from the world's `ChatServerUrl`, §3.6), or `None` without one. A name
/// lookup: the connector resolves it before its last cancel check ahead of the session.
pub fn resolve_chat_endpoint<N: ConnectorNet>(
    net: &mut N,
    handoff: &SessionHandoff,
) -> Option<SocketAddrV4> {
    let raw = handoff.chat_server.trim();
    if raw.is_empty() {
        return None;
    }
    let host = crate::login::LoginHost::parse(raw).ok()?;
    let port = host.remote_port(None).ok()?;
    net.resolve(&host.remote_host, port).ok()
}

/// `n` moved so that it never falls between a `LoginRefused` and the `Disconnected` after it
/// (`official-entry.md` §8.1: both arrive in the same poll): one more is taken then.
#[must_use]
pub fn refusal_cut(events: &[Event], n: usize) -> usize {
    let n = n.min(events.len());
    let refused = n
        .checked_sub(1)
        .and_then(|i| events.get(i))
        .is_some_and(|e| matches!(e, Event::Account(AccountEvent::LoginRefused { .. })));
    if refused && matches!(events.get(n), Some(Event::Disconnected { .. })) {
        n + 1
    } else {
        n
    }
}

/// The recipient ID a datagram names (`official-session.md` §3.4 step 1): the non-zero
/// wire prefix, else `DatagramHeader.recId`.
fn header_rec_id(datagram: &[u8]) -> Option<u16> {
    if datagram.len() < MIN_DATAGRAM_BYTES {
        return None;
    }
    let prefix = u16::from_be_bytes([*datagram.first()?, *datagram.get(1)?]);
    if prefix != 0 {
        return Some(prefix);
    }
    Some(u16::from_be_bytes([*datagram.get(2)?, *datagram.get(3)?]))
}

impl<N: ConnectorNet> SessionDriver<N> {
    /// Opens the login connection (`official-session.md` §3.2–§3.3, §6.2): binds the
    /// socket and queues `SessionAuthHeader` to the resolved login server. `stop` is looked
    /// at right before the local-address probe, right before the bind and right before the
    /// first send: once it holds, nothing is probed, bound or sent.
    ///
    /// # Errors
    ///
    /// [`StartFailure`] before any datagram was sent.
    pub fn start(
        net: &mut N,
        start: SessionStart,
        now: f64,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, StartFailure> {
        let host = start
            .handoff
            .login_host()
            .map_err(|_| StartFailure::Address)?;
        let peer = start.login_peer;
        if stop() {
            return Err(StartFailure::Cancelled);
        }
        // The local address: the two-colon `-h` form's `localHost`, else the address the
        // system routes towards the server (`NetworkAccess::local_ipv4_toward` says why),
        // else `0.0.0.0`.
        let from_host: Option<std::net::Ipv4Addr> =
            host.local_host.as_deref().and_then(|h| h.parse().ok());
        let (local_ip, local_source) = match from_host {
            Some(ip) => (ip, LocalSource::HostSegment),
            None => match net.local_ipv4_toward(peer) {
                Some(ip) => (ip, LocalSource::Routed),
                None => (std::net::Ipv4Addr::UNSPECIFIED, LocalSource::Unspecified),
            },
        };
        if stop() {
            return Err(StartFailure::Cancelled);
        }
        let base_port = start.handoff.local_bind_port;
        let transport = net
            .bind_udp(local_ip, base_port, true)
            .map_err(|_| StartFailure::Bind)?;
        let bind = transport.local_addr().map_or(BindCase::Unknown, |b| {
            BindCase::of(local_ip, base_port, true, b)
        });
        let remote = (
            RemoteForm::of_host(host.local_host.is_some(), host.port.is_some()),
            RemoteName::of_host(&host.remote_host),
        );
        let client_timestamp = unix_seconds();
        let auth = start
            .handoff
            .session_auth_header(client_timestamp)
            .map_err(|_| StartFailure::Ticket)?;
        let mode = AuthMode::of(auth.auth_type, auth.auth_flags);
        let mut login = Link::new(
            peer,
            &start.codec_uuids,
            start.send_checksum,
            mode,
            remote,
            now,
        )
        .map_err(|_| StartFailure::Bind)?;
        if let Some(c) = login.mux.connection_mut(login.conn) {
            c.start_login(auth, now).map_err(|_| StartFailure::Ticket)?;
        }
        let categories = chat_categories(start.schema.0.as_ref());
        let mut driver = Self {
            transport,
            codecs: start.codecs,
            codec_uuids: start.codec_uuids,
            login,
            world: None,
            old: Vec::new(),
            version: start.version,
            version_sent: false,
            schema: start.schema,
            stage: Stage::Login,
            characters: Vec::new(),
            list_on_world: false,
            loaded: false,
            portal_since: None,
            adapter: PlayAdapter::new(categories),
            feed: EntityFeed::default(),
            message_events: 0,
            chat: None,
            chat_allowance: Arc::new(AtomicUsize::new(0)),
            chat_endpoint: start.chat_endpoint,
            chat_retry_at: 0.0,
            chat_warned: false,
            handoff: start.handoff,
            refresher: start.refresher,
            plain: start.plain,
            refreshing: None,
            background_http: Some(net.background_http()),
            cancel: start.cancel,
            client_timestamp,
            events: VecDeque::new(),
            ended: false,
            shutting_down: false,
            counters: DriverCounters::default(),
            buf: vec![0u8; MAX_DATAGRAM_BUF],
            pending_out: VecDeque::new(),
            inbox: VecDeque::new(),
            entry_messages: Vec::new(),
            entry_bytes: 0,
            refused_log: LogLimit::default(),
            unknown_log: LogLimit::default(),
            send_log: LogLimit::default(),
            send_checksum: start.send_checksum,
            bind,
            local_source,
        };
        if stop() {
            return Err(StartFailure::Cancelled);
        }
        driver.report(&driver.login, false, now).log_start();
        driver.flush(now);
        Ok(driver)
    }

    /// The events produced since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    /// At most `n` of the events produced, oldest first; the rest wait for the next call. A
    /// `LoginRefused` is never parted from its `Disconnected` ([`refusal_cut`]: one more
    /// is taken then).
    pub fn take_events_up_to(&mut self, n: usize) -> Vec<Event> {
        let n = refusal_cut(self.events.make_contiguous(), n);
        self.events.drain(..n).collect()
    }

    /// How many events wait to be taken.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Whether the driver holds [`DRIVER_EVENT_CAP`] events (it reads nothing more).
    fn full(&self) -> bool {
        self.events.len() >= DRIVER_EVENT_CAP
    }

    /// Whether events wait to be taken.
    #[must_use]
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Whether the session ended (a `Disconnected` was produced, or the shutdown began).
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Whether the session is in the world.
    #[must_use]
    pub fn in_world(&self) -> bool {
        matches!(self.stage, Stage::InWorld { .. })
    }

    /// The counters.
    #[must_use]
    pub fn counters(&self) -> DriverCounters {
        self.counters
    }

    /// The entity feed's counters (module docs "Entities").
    #[must_use]
    pub fn feed_counters(&self) -> FeedCounters {
        self.feed.counters()
    }

    /// The entities the feed tracks and those of them waiting for a position (fuzzing and
    /// tests: the feed's bounds).
    #[doc(hidden)]
    #[must_use]
    pub fn feed_sizes(&self) -> (usize, usize) {
        (self.feed.tracked(), self.feed.pending())
    }

    /// The play layer's counters (texts, chat categories, admin messages, positions).
    #[must_use]
    pub fn play_counters(&self) -> PlayCounters {
        self.adapter.counters()
    }

    /// Whether a ticket refresh thread is running (tests).
    #[must_use]
    pub fn refresh_running(&self) -> bool {
        self.refreshing.is_some()
    }

    /// The local recipient IDs of the login and world connections (tests).
    #[must_use]
    pub fn connection_ids(&self) -> (u16, Option<u16>) {
        (self.login.conn, self.world.as_ref().map(|w| w.conn))
    }

    /// The states of the login and world connections (tests).
    #[must_use]
    pub fn connection_states(&self) -> (Option<ConnectionState>, Option<ConnectionState>) {
        (
            self.login.state(),
            self.world.as_ref().and_then(Link::state),
        )
    }

    /// The states of the replaced world connections, oldest first (tests).
    #[must_use]
    pub fn old_connection_states(&self) -> Vec<Option<ConnectionState>> {
        self.old.iter().map(Link::state).collect()
    }

    fn end(&mut self, reason: DisconnectReason) {
        if self.ended {
            return;
        }
        if matches!(self.stage, Stage::InWorld { .. }) {
            self.log_world_end();
        }
        self.ended = true;
        self.stop_refresh();
        self.events.push_back(Event::Disconnected { reason });
    }

    /// What the world feed counted, once per world (debug).
    fn log_world_end(&self) {
        tracing::debug!(
            feed = ?self.feed.counters(),
            appearance_base_model = self.counters.appearance_base_model,
            message_events_dropped = self.counters.message_events_dropped,
            texts_skipped = self.counters.texts_skipped,
            "official world: entity feed counters"
        );
    }

    fn refuse(&mut self, reason: LoginRefusal) {
        if self.ended {
            return;
        }
        self.events
            .push_back(Event::Account(AccountEvent::LoginRefused {
                reason,
                retry_after_seconds: None,
                detail: None,
            }));
        self.end(DisconnectReason::TransportFailed(
            "official login refused".to_owned(),
        ));
    }

    /// One pass: read the socket (when `read` and below [`DRIVER_EVENT_CAP`]; the connector
    /// stops reading while its event queue is full), run the connections, the chat client
    /// and the ticket refresh, and send what is due (acknowledgements and keep-alives go out
    /// either way). Once `stop` holds (the attempt was cancelled) no referral is accepted,
    /// no ticket refresh is started and nothing is sent: the chat client is not run and
    /// nothing is ticked or flushed, so a connection still in its handshake sends no
    /// retransmitted `SessionAuthHeader` or `ReferralAuthHeader` and no `ConnectAccept` for
    /// a challenge read in this pass; the shutdown that follows closes it without a word
    /// (module docs "Ending the session"). Returns whether anything arrived or was produced.
    pub fn step(&mut self, now: f64, read: bool, stop: &dyn Fn() -> bool) -> bool {
        if self.ended {
            return false;
        }
        let before = self.events.len();
        let read = read && !self.full();
        self.feed.begin_pass();
        self.feed_deferred();
        let read_any = read && self.receive(now, stop) > 0;
        self.run_links(now, stop);
        self.run_refresh(now, stop);
        if stop() {
            return read_any || self.events.len() != before;
        }
        if read && !self.ended && !self.full() {
            // The chat client reads its own socket: it pauses with the session's. It ticks
            // and flushes its own connection, so it runs only before the cancel.
            self.run_chat(now);
        }
        self.enter_deadline(now);
        self.portal_step(now);
        self.send_pending(now);
        self.tick(now);
        if stop() {
            // Cancelled while this pass ran: what the tick queued (it may hold the ticket)
            // is not sent; the shutdown wipes a handshake connection's queue.
            return read_any || self.events.len() != before;
        }
        self.flush(now);
        self.report_waiting(now);
        read_any || self.events.len() != before
    }

    /// `0x6F` was sent and the world has not been entered within
    /// [`ENTER_WORLD_TIMEOUT_SEC`]: the entry fails and the character list is offered again.
    fn enter_deadline(&mut self, now: f64) {
        let Stage::Entering { name, since, .. } = &self.stage else {
            return;
        };
        if now - since < ENTER_WORLD_TIMEOUT_SEC {
            return;
        }
        let character = name.clone();
        tracing::info!("official session: the world did not answer the entry");
        if let Some(mut w) = self.world.take() {
            w.end_session(now);
            self.keep_old(w);
        }
        self.entry_messages.clear();
        self.entry_bytes = 0;
        self.stage = Stage::Characters;
        self.events.push_back(Event::EnterWorldFailed {
            character,
            reason: "The world did not answer.".to_owned(),
        });
    }

    /// Portal space (`official-messages-play.md` §4.5 item 4): it ends, and `0xF4` goes out,
    /// once the server's stamp allows and the client said its loading is done
    /// ([`Command::WorldLoaded`]), or once [`PORTAL_LOADING_TIMEOUT_SEC`] have passed since
    /// portal space was first seen.
    fn portal_step(&mut self, now: f64) {
        if !matches!(self.stage, Stage::InWorld { .. }) {
            return;
        }
        if !self.adapter.teleport().in_portal_space() {
            self.portal_since = None;
            return;
        }
        let since = *self.portal_since.get_or_insert(now);
        let timed_out = now - since >= PORTAL_LOADING_TIMEOUT_SEC;
        if let Some(ack) = self.adapter.portal_loading(self.loaded, timed_out) {
            if !self.loaded {
                tracing::info!("official session: portal space ended by its time-out");
            }
            self.loaded = false;
            self.portal_since = None;
            self.pending_out.push_back(ack);
        }
    }

    /// Starts the portal-space loading state afresh: when a portal space begins (world
    /// entry, `official-messages-world.md` §4.3.3), so a `WorldLoaded` of an earlier
    /// placement never ends it, and when the world is left.
    fn fresh_portal_loading(&mut self) {
        self.loaded = false;
        self.portal_since = None;
    }

    /// The world was torn down (`official-messages-play.md` §7.2 item 3): the session stays
    /// open, and the client is back at character selection at once with the latest list
    /// (module docs "Leaving the world"). The list that follows the teardown comes on the
    /// world connection (§7.2 item 3 keeps it open), so a later `0x6F` goes there too.
    fn left_world(&mut self) {
        tracing::info!("official session: the world was left for character selection");
        self.log_world_end();
        self.stage = Stage::Characters;
        self.list_on_world = self.world.is_some();
        self.entry_messages.clear();
        self.entry_bytes = 0;
        self.fresh_portal_loading();
        // The client drops every entity with the world (`ReturnedToCharacterSelect`).
        self.feed.reset(None);
        let characters = self
            .characters
            .iter()
            .map(|(_, name)| CharacterSummary { name: name.clone() })
            .collect();
        self.events
            .push_back(Event::ReturnedToCharacterSelect { characters });
    }

    /// Where a datagram from `src` naming `rec_id` goes (module docs).
    fn route(&self, rec_id: Option<u16>, src: SocketAddrV4) -> Route {
        let candidates = || {
            self.world
                .iter()
                .map(|w| (1usize, w))
                .chain(self.old.iter().enumerate().rev().map(|(i, l)| (2 + i, l)))
                .chain(std::iter::once((0usize, &self.login)))
        };
        let in_range = rec_id.filter(|id| (MIN_LOCAL_REC_ID..=MAX_LOCAL_REC_ID).contains(id));
        if let Some(id) = in_range {
            // Step 2: the connection with that ID, from its own server's address.
            if let Some((i, _)) = candidates().find(|(_, l)| l.conn == id && l.peer == src) {
                return Route::Link(i);
            }
            let named = candidates().any(|(_, l)| l.conn == id);
            let unassigned = candidates().any(|(_, l)| l.peer == src && l.awaiting_challenge());
            if named && !unassigned {
                return Route::AddressMismatch;
            }
        }
        // Step 3: a lookup miss goes by the source address.
        candidates()
            .find(|(_, l)| l.peer == src)
            .map_or(Route::Unknown, |(i, _)| Route::Link(i))
    }

    /// The handshake diagnostics of `link` (the world connection when `world`) at `now`.
    fn report(&self, link: &Link, world: bool, now: f64) -> HandshakeReport {
        let conn = link.mux.connection(link.conn);
        HandshakeReport {
            world,
            state: conn.map(|c| c.closed_from().unwrap_or_else(|| c.state())),
            elapsed_sec: (now - link.diag.started).max(0.0),
            counts: conn.map(|c| c.handshake_counts()).unwrap_or_default(),
            link: link.diag,
            other_address: self
                .counters
                .unknown_sources
                .saturating_add(self.counters.address_mismatches),
            challenge_from_other_address: self.counters.challenge_from_other_address,
            recv_errors: self.counters.recv_errors,
            bind: self.bind,
            local: self.local_source,
            remote: link.remote.0,
            remote_name: link.remote.1,
            auth: link.auth,
            send_checksum: self.send_checksum,
        }
    }

    /// The handshake diagnostics of the login connection, or of the world connection when
    /// `world` (`None` without one), at `now` ([`super::diag`]).
    #[must_use]
    pub fn handshake_report(&self, world: bool, now: f64) -> Option<HandshakeReport> {
        self.link_of(world).map(|l| self.report(l, world, now))
    }

    /// The login connection, or the world connection when `world`.
    fn link_of(&self, world: bool) -> Option<&Link> {
        if world {
            self.world.as_ref()
        } else {
            Some(&self.login)
        }
    }

    /// The `debug` report of every connection still in its handshake, every
    /// [`HANDSHAKE_REPORT_SEC`] ([`super::diag`] "What is logged").
    fn report_waiting(&mut self, now: f64) {
        for world in [false, true] {
            let Some(l) = self.link_of(world) else {
                continue;
            };
            if !l.in_handshake() || now < l.diag.next_report {
                continue;
            }
            self.report(l, world, now).log_waiting();
            let l = if world {
                self.world.as_mut()
            } else {
                Some(&mut self.login)
            };
            if let Some(l) = l {
                l.diag.next_report = now + HANDSHAKE_REPORT_SEC;
            }
        }
    }

    /// Counts `datagram`, from an address no connection takes, when it carries a
    /// `ConnectChallengeHeader` while a connection is in its handshake ([`super::diag`]).
    fn challenge_from_elsewhere(&mut self, datagram: &[u8]) {
        let waiting =
            self.login.in_handshake() || self.world.as_ref().is_some_and(Link::in_handshake);
        if waiting && carries_challenge(datagram) {
            self.counters.challenge_from_other_address =
                self.counters.challenge_from_other_address.saturating_add(1);
        }
    }

    /// Logs the failure line once for each connection still in its handshake, which is
    /// about to be closed without a `Disconnected` event (a cancel, a revoked right).
    fn log_abandoned_handshakes(&self, reason: &str, now: f64) {
        for world in [false, true] {
            if let Some(l) = self.link_of(world).filter(|l| l.in_handshake()) {
                self.report(l, world, now).log_ended(reason);
            }
        }
    }

    fn link_mut(&mut self, index: usize) -> Option<&mut Link> {
        match index {
            0 => Some(&mut self.login),
            1 => self.world.as_mut(),
            i => self.old.get_mut(i.checked_sub(2)?),
        }
    }

    /// Reads up to [`MAX_DATAGRAMS_PER_STEP`] datagrams, handling each before the next is
    /// read, and stops at [`DRIVER_EVENT_CAP`] events; returns how many were read.
    fn receive(&mut self, now: f64, stop: &dyn Fn() -> bool) -> usize {
        let mut buf = std::mem::take(&mut self.buf);
        buf.resize(MAX_DATAGRAM_BUF, 0);
        let mut read = 0usize;
        for _ in 0..MAX_DATAGRAMS_PER_STEP {
            // The shutdown reads on for the disconnect's answers; a session that ended
            // otherwise reads nothing more.
            if self.full() || (self.ended && !self.shutting_down) {
                break;
            }
            let (len, src) = match self.transport.recv_from(&mut buf) {
                Ok(Some(r)) => r,
                Ok(None) => break,
                Err(e) => {
                    self.counters.recv_errors = self.counters.recv_errors.saturating_add(1);
                    if let Some(skipped) = self.refused_log.allow(now) {
                        tracing::debug!(error = %e, skipped, "official session: socket read failed");
                    }
                    break;
                }
            };
            read += 1;
            let Some(datagram) = buf.get(..len) else {
                break;
            };
            let index = match self.route(header_rec_id(datagram), src) {
                Route::Link(i) => i,
                Route::AddressMismatch => {
                    self.counters.address_mismatches += 1;
                    self.challenge_from_elsewhere(datagram);
                    if let Some(skipped) = self.unknown_log.allow(now) {
                        tracing::debug!(
                            skipped,
                            "official session: datagram for a connection from another address"
                        );
                    }
                    continue;
                }
                Route::Unknown => {
                    self.counters.unknown_sources += 1;
                    self.challenge_from_elsewhere(datagram);
                    if let Some(skipped) = self.unknown_log.allow(now) {
                        tracing::debug!(
                            skipped,
                            "official session: datagram from an unknown address"
                        );
                    }
                    continue;
                }
            };
            let codecs = self.codecs.clone();
            let Some(link) = self.link_mut(index) else {
                continue;
            };
            link.diag.received = link.diag.received.saturating_add(1);
            let in_handshake = link.in_handshake();
            let challenges_before = link.challenges();
            let result = link
                .mux
                .receive_from_peer(datagram, src, now, codecs.wire());
            match result {
                Ok((id, _)) => link.conn = id,
                Err(e) => {
                    link.diag.rejected.add(RejectKind::of(&e));
                    // A challenge the connection acted on and then refused (an unsupported
                    // codec) is already in its own count.
                    if in_handshake
                        && link.challenges() == challenges_before
                        && carries_challenge(datagram)
                    {
                        link.diag.challenge_in_rejected =
                            link.diag.challenge_in_rejected.saturating_add(1);
                    }
                    self.counters.refused_datagrams += 1;
                    if let Some(skipped) = self.refused_log.allow(now) {
                        tracing::debug!(error = %e, skipped, "official session: datagram refused");
                    }
                }
            }
            // What this datagram brought is handled before the next is read, so the cap
            // holds.
            self.run_links(now, stop);
        }
        self.buf = buf;
        read
    }

    /// Hands the inbox and the connections' session events on (login first, then world)
    /// until none is left, the session ended or [`DRIVER_EVENT_CAP`] events wait; what is
    /// left waits for the next pass.
    fn run_links(&mut self, now: f64, stop: &dyn Fn() -> bool) {
        for old in &mut self.old {
            // A replaced connection only finishes its disconnect.
            old.drain_events();
        }
        loop {
            if self.ended || self.full() {
                return;
            }
            if let Some(m) = self.inbox.pop_front() {
                self.message(m.opcode, m.recipient, &m.body, m.world, now);
                continue;
            }
            let next = match self.login.pop_event() {
                Some(e) => Some((e, false)),
                None => self
                    .world
                    .as_mut()
                    .and_then(Link::pop_event)
                    .map(|e| (e, true)),
            };
            let Some((e, world)) = next else {
                return;
            };
            self.link_event(e, world, now, stop);
        }
    }

    fn link_event(&mut self, event: SessionEvent, world: bool, now: f64, stop: &dyn Fn() -> bool) {
        match event {
            SessionEvent::Connected { .. } => {
                tracing::info!(world, "official session: connection established");
                if self.stage == Stage::Login {
                    // The progress screen's watchdog (`official-entry.md` §8.3) stays
                    // alive while the version exchange and the character list follow.
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginProgress {
                            stage: LoginStage::JoiningWorld,
                        }));
                }
            }
            SessionEvent::BlobReceived(blob) => {
                match dispatch_message(
                    blob.blob_header_word,
                    &blob.payload,
                    DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
                    DEFAULT_MAX_CONTAINER_DEPTH,
                ) {
                    Ok(envelopes) => {
                        // Handled by `run_links` within the cap (module docs).
                        self.inbox.extend(envelopes.into_iter().map(|env| Inbound {
                            opcode: env.opcode,
                            recipient: blob.recipient_id,
                            body: env.body,
                            world,
                        }));
                    }
                    Err(e) => {
                        self.counters.decode_errors += 1;
                        tracing::debug!(error = %e, "official session: message refused");
                    }
                }
            }
            SessionEvent::WorldReferralReceived {
                referral_cookie,
                world_addr,
                ..
            } => {
                if stop() {
                    // No world login after a cancel.
                    self.counters.cancelled_referrals += 1;
                    tracing::debug!("official session: referral after the cancel ignored");
                } else {
                    self.referral(referral_cookie, world_addr, now);
                }
            }
            SessionEvent::ServerRedirect(_) | SessionEvent::AuthServerAddress(_) => {
                // §6.6 names no client action that this session needs.
                tracing::debug!("official session: redirect header ignored");
            }
            SessionEvent::Disconnected { reason } => self.link_closed(reason, world, now),
        }
    }

    fn link_closed(&mut self, reason: LinkEnd, world: bool, now: f64) {
        tracing::info!(world, ?reason, "official session: connection closed");
        if let Some(l) = self.link_of(world).filter(|l| l.closed_in_handshake()) {
            self.report(l, world, now).log_failed(&reason);
        }
        // After the referral the login connection is not needed any more.
        if !world && self.world.is_some() {
            return;
        }
        let refused_login = matches!(
            reason,
            LinkEnd::TransportError { error_code, .. } if LOGIN_REFUSAL_CODES.contains(&error_code)
        );
        if self.stage == Stage::Login {
            match reason {
                _ if refused_login => self.refuse(LoginRefusal::AccountNotAllowed),
                LinkEnd::HandshakeTimeout | LinkEnd::SilenceTimeout => {
                    self.end(DisconnectReason::TimedOut);
                }
                LinkEnd::UnsupportedCodec => self.refuse(LoginRefusal::ClientNotAccepted),
                _ => self.end(DisconnectReason::ServerClosed(String::new())),
            }
            return;
        }
        let r = match reason {
            LinkEnd::HandshakeTimeout | LinkEnd::SilenceTimeout => DisconnectReason::TimedOut,
            LinkEnd::LocalDisconnect => DisconnectReason::ClientRequested,
            LinkEnd::ServerDisconnect => DisconnectReason::ServerClosed(String::new()),
            _ => DisconnectReason::TransportFailed("the official connection failed".to_owned()),
        };
        self.end(r);
    }

    fn keep_old(&mut self, link: Link) {
        self.old.push(link);
        if self.old.len() > MAX_OLD_LINKS {
            self.old.remove(0);
        }
    }

    /// A `WorldReferralHeader` (`official-session.md` §6.5): a new world connection on the
    /// same socket, in a multiplexer of its own (module docs). Accepted only while the world
    /// is being entered or in it (`official-messages-world.md` §3.6 shows the referral only
    /// there); a replaced world connection is disconnected gracefully and kept until its
    /// disconnect is out. Both: engineering choice, spec silent (question with the spec
    /// writers).
    fn referral(&mut self, cookie: u64, addr: SocketAddrV4, now: f64) {
        if !matches!(self.stage, Stage::Entering { .. } | Stage::InWorld { .. }) {
            self.counters.stray_referrals += 1;
            tracing::debug!("official session: referral outside world entry ignored");
            return;
        }
        let form = if addr.ip() == self.login.peer.ip() {
            RemoteForm::ReferralLoginIp
        } else {
            RemoteForm::ReferralOwnIp
        };
        match Link::new(
            addr,
            &self.codec_uuids,
            self.send_checksum,
            AuthMode::Referral,
            (form, RemoteName::FromReferral),
            now,
        ) {
            Ok(mut link) => {
                if let Some(c) = link.mux.connection_mut(link.conn)
                    && let Err(e) = c.start_world_login(cookie, now)
                {
                    tracing::warn!(error = %e, "official session: referral not started");
                    return;
                }
                if let Some(mut old) = self.world.replace(link) {
                    // Still in its handshake: closed without a word (module docs "Ending
                    // the session"), so it sends no more `ReferralAuthHeader`.
                    old.end_session(now);
                    self.keep_old(old);
                }
                tracing::info!("official session: referred to the world server");
                if let Some(w) = self.world.as_ref() {
                    self.report(w, true, now).log_start();
                }
            }
            Err(e) => tracing::warn!(error = %e, "official session: referral refused"),
        }
    }

    fn message(&mut self, opcode: u32, recipient: u64, body: &[u8], world: bool, now: f64) {
        if (OPCODE_CLIENT_VERSION..=OPCODE_LOGIN_QUEUE_STATUS).contains(&opcode) {
            match ControlMessage::decode(opcode, body) {
                Ok(m) => self.control(m, now),
                Err(e) => {
                    self.counters.decode_errors += 1;
                    tracing::debug!(opcode, error = %e, "official session: control message refused");
                }
            }
            return;
        }
        if body.first() != Some(&0) {
            self.counters.decode_errors += 1;
            return;
        }
        match opcode {
            CharacterListResponse::EVENT_ID => self.character_list(body, world),
            CharacterOperationError::EVENT_ID => self.character_error(body),
            PlayerSessionReady::EVENT_ID => self.session_ready(body),
            _ => self.play(opcode, recipient, body, world),
        }
    }

    fn control(&mut self, m: ControlMessage, now: f64) {
        match m {
            ControlMessage::VersionRequest => {
                if self.version_sent {
                    return;
                }
                let msg = ClientVersionMessage {
                    version_string: self.version.version_string(),
                };
                match msg.encode() {
                    Ok(body) => {
                        // Direct-payload message: `envelopeFlags` then the body (§9.2).
                        let env = encode_event_envelope(&body);
                        if self.send_on_login(OPCODE_CLIENT_VERSION, &env, now) {
                            self.version_sent = true;
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "official session: version not encoded"),
                }
            }
            ControlMessage::VersionStatus(s) => {
                if s.ensure_accepted().is_err() {
                    tracing::info!("official session: the service did not accept this version");
                    self.refuse(LoginRefusal::ClientNotAccepted);
                }
            }
            ControlMessage::LoginQueueStatus(q) => {
                if self.stage == Stage::Login {
                    // `logonResult`'s values are not in the spec; the position and total
                    // are shown when non-zero. Engineering choice, spec silent (question
                    // with the spec writers).
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginQueueChanged {
                            position: (q.queue_position > 0).then_some(q.queue_position),
                            length: (q.queue_total > 0).then_some(q.queue_total),
                            wait_seconds: None,
                        }));
                }
            }
            ControlMessage::SystemNotification(n) => {
                self.events.push_back(Event::ServerNotice {
                    level: NoticeLevel::Info,
                    seconds: 0,
                    text: n.text,
                });
            }
            ControlMessage::ClientVersion(_) => {
                self.counters.unhandled += 1;
            }
        }
    }

    fn character_list(&mut self, body: &[u8], world: bool) {
        let list = match CharacterListResponse::from_bytes(body, &self.schema) {
            Ok(l) => l,
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(error = %e, "official session: character list refused");
                return;
            }
        };
        let fresh: Vec<(u64, String)> = list
            .characters
            .iter()
            .filter_map(|c| character_name(&c.name).map(|n| (c.character_id, n)))
            .take(MAX_CHARACTERS)
            .collect();
        let old = std::mem::replace(&mut self.characters, fresh);
        self.list_on_world = world && self.world.is_some();
        if matches!(self.stage, Stage::Characters | Stage::Entering { .. }) {
            // A later list at character selection (the one after a teardown among them,
            // `official-messages-play.md` §8): its changes, names gone first.
            for (_, name) in &old {
                if !self.characters.iter().any(|(_, n)| n == name) {
                    self.events
                        .push_back(Event::Character(CharacterEvent::CharacterRemoved {
                            name: name.clone(),
                        }));
                }
            }
            for (_, name) in &self.characters {
                if !old.iter().any(|(_, n)| n == name) {
                    self.events
                        .push_back(Event::Character(CharacterEvent::CharacterInfo {
                            name: name.clone(),
                            body: None,
                            class: None,
                            level: None,
                            look: None,
                        }));
                }
            }
            tracing::debug!(
                count = self.characters.len(),
                "official session: characters listed again"
            );
            return;
        }
        // In the world the list is kept for the return (it counts as the latest).
        if self.stage == Stage::Login {
            self.stage = Stage::Characters;
            let characters = self
                .characters
                .iter()
                .map(|(_, name)| CharacterSummary { name: name.clone() })
                .collect();
            tracing::info!(
                count = self.characters.len(),
                "official session: characters listed"
            );
            self.events.push_back(Event::Connected { characters });
        }
    }

    fn character_error(&mut self, body: &[u8]) {
        let code = CharacterOperationError::from_bytes(body)
            .map(|e| e.error_code)
            .ok();
        if let Stage::Entering { name, .. } = &self.stage {
            let character = name.clone();
            self.stage = Stage::Characters;
            self.entry_messages.clear();
            self.entry_bytes = 0;
            self.events.push_back(Event::EnterWorldFailed {
                character,
                reason: match code {
                    Some(c) => format!("The world refused the character (code {c})."),
                    None => "The world refused the character.".to_owned(),
                },
            });
        } else {
            tracing::debug!(
                ?code,
                "official session: character error outside world entry"
            );
        }
    }

    fn session_ready(&mut self, body: &[u8]) {
        let ready = match PlayerSessionReady::from_bytes(body, &self.schema) {
            Ok(r) => r,
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(error = %e, "official session: session start refused");
                return;
            }
        };
        let name = match &self.stage {
            Stage::Entering { name, .. } => name.clone(),
            _ => {
                tracing::debug!("official session: session start outside world entry");
                return;
            }
        };
        self.adapter
            .enter_world(ready.player_id, Some(name.clone()));
        self.feed.reset(Some(ready.player_id));
        // World entry is a portal space (the adapter's `enter_world`).
        self.fresh_portal_loading();
        self.stage = Stage::InWorld {
            player: ready.player_id,
            name: name.clone(),
        };
        self.events.push_back(Event::WorldEntered {
            player: EntityId(ready.player_id),
            name,
            transform: Transform::default(),
        });
        // What arrived before `0x90` (the world name `0x43` among it) now reaches the play
        // layer, in order.
        let held = std::mem::take(&mut self.entry_messages);
        self.entry_bytes = 0;
        for (opcode, recipient, body) in held {
            self.play_in_world(opcode, recipient, &body);
            if self.ended || !matches!(self.stage, Stage::InWorld { .. }) {
                return;
            }
        }
    }

    /// A play message. `0xB3` (eviction) ends the session in every stage:
    /// `official-messages-world.md` §3.4 has the server send it whenever it disconnects or
    /// boots the account, in no particular stage (§3.6 draws it at the character list and
    /// during world entry; ending the session on it during the login and in the world as
    /// well is Tessera's reading, engineering choice). While the world is being entered the others are
    /// kept for [`Self::session_ready`]; in the world they go to the play layer when they
    /// come from the world connection, or from the login connection when there was no
    /// referral.
    fn play(&mut self, opcode: u32, recipient: u64, body: &[u8], world: bool) {
        if opcode == AccountEvictionNotice::EVENT_ID {
            self.eviction(opcode, recipient, body);
            return;
        }
        if !world && self.world.is_some() {
            self.counters.unhandled += 1;
            tracing::trace!(
                opcode,
                "official session: login-connection message after the referral"
            );
            return;
        }
        match self.stage {
            Stage::InWorld { .. } => self.play_in_world(opcode, recipient, body),
            Stage::Entering { .. } => {
                if self.entry_messages.len() >= MAX_ENTRY_MESSAGES
                    || self.entry_bytes.saturating_add(body.len()) > MAX_ENTRY_BYTES
                {
                    self.counters.entry_overflow += 1;
                    return;
                }
                self.entry_bytes += body.len();
                self.entry_messages.push((opcode, recipient, body.to_vec()));
            }
            Stage::Login | Stage::Characters => {
                self.counters.unhandled += 1;
                tracing::trace!(
                    opcode,
                    "official session: message outside the world ignored"
                );
            }
        }
    }

    fn eviction(&mut self, opcode: u32, recipient: u64, body: &[u8]) {
        match decode_play_message(opcode, recipient, body, &self.schema, &self.schema) {
            Ok(Some(msg)) => {
                let out = self.adapter.handle(&msg, &self.schema);
                let reason = out.events.into_iter().find_map(|e| match e {
                    Event::Disconnected { reason } => Some(reason),
                    _ => None,
                });
                tracing::info!("official session: the server ended the session");
                self.end(reason.unwrap_or_else(|| DisconnectReason::ServerClosed(String::new())));
            }
            Ok(None) | Err(_) => {
                // A malformed eviction still ends the session: the server said it closes.
                self.counters.decode_errors += 1;
                self.end(DisconnectReason::ServerClosed(String::new()));
            }
        }
    }

    /// A message in the world (module docs "Entities"): batches are unpacked, the entity
    /// lifecycle and property events go to the [`EntityFeed`], the rest to the play layer;
    /// the feed's events of the message follow once it has been read.
    fn play_in_world(&mut self, opcode: u32, recipient: u64, body: &[u8]) {
        self.message_events = 0;
        self.world_message(opcode, recipient, body, 0);
        if !self.in_world_still() {
            return;
        }
        let mut out = FeedOutput::default();
        self.feed.flush(&self.schema, &mut out);
        self.feed_output(out);
    }

    /// Creations put off for their templates or name tables (module docs "Entities"),
    /// retried once per pass, before the pass reads anything (so they come before the new
    /// placements of the pass in its read budgets), and only below [`DRIVER_EVENT_CAP`].
    fn feed_deferred(&mut self) {
        if !self.in_world_still() || !self.feed.has_deferred() || self.full() {
            return;
        }
        let mut out = FeedOutput::default();
        self.feed.retry_deferred(&self.schema, &mut out);
        self.feed_output(out);
    }

    /// Whether the driver still takes world messages.
    fn in_world_still(&self) -> bool {
        !self.ended && matches!(self.stage, Stage::InWorld { .. })
    }

    fn feed_output(&mut self, out: FeedOutput) {
        self.events.extend(out.events);
        self.pending_out.extend(out.outgoing);
    }

    /// Whether `opcode` is a batch or an entity lifecycle or property event the feed reads.
    fn feed_opcode(opcode: u32) -> bool {
        matches!(
            opcode,
            SubEventBatch::EVENT_ID_AOI_ENTER
                | SubEventBatch::EVENT_ID_SUB_EVENT
                | EntityCreate::EVENT_ID
                | EntityDespawn::EVENT_ID
                | PropertyTableUpdate::EVENT_ID_ENTITY
                | VisualDescriptionUpdate::EVENT_ID
        ) || SinglePropertyUpdate::scope_of(opcode) == Some(PropertyScope::Entity)
            || PropertyRemoval::scope_of(opcode) == Some(PropertyScope::Entity)
    }

    /// One world message at batch depth `depth` (`0`: a top-level message); the outcome of
    /// a `0x98` (for [`sub_event_target`]).
    fn world_message(
        &mut self,
        opcode: u32,
        recipient: u64,
        body: &[u8],
        depth: usize,
    ) -> Option<CreateOutcome> {
        if !Self::feed_opcode(opcode) {
            self.play_message(opcode, recipient, body);
            return None;
        }
        if body.first() != Some(&0) {
            self.counters.decode_errors += 1;
            return None;
        }
        let mut out = FeedOutput::default();
        let mut outcome = None;
        let decoded: Result<(), WorldError> = match opcode {
            SubEventBatch::EVENT_ID_AOI_ENTER | SubEventBatch::EVENT_ID_SUB_EVENT => {
                self.batch(opcode, recipient, body, depth);
                return None;
            }
            EntityCreate::EVENT_ID => EntityCreate::from_bytes(body).map(|c| {
                outcome = Some(self.feed.create(c, &mut out));
            }),
            EntityDespawn::EVENT_ID => {
                EntityDespawn::from_bytes(body).map(|_| self.feed.despawn(recipient, &mut out))
            }
            PropertyTableUpdate::EVENT_ID_ENTITY => {
                let mut r = WireReader::new(body);
                PropertyTableUpdate::decode(&mut r, opcode, &self.schema)
                    .and_then(|u| r.finish().map(|()| u))
                    .map(|u| {
                        self.feed.properties(
                            recipient,
                            &u.properties.entries,
                            &self.schema,
                            &mut out,
                        );
                    })
            }
            VisualDescriptionUpdate::EVENT_ID => VisualDescriptionUpdate::from_bytes(body)
                .map(|u| self.feed.visual(recipient, &u.visual_desc, &mut out)),
            op if SinglePropertyUpdate::scope_of(op) == Some(PropertyScope::Entity) => {
                let mut r = WireReader::new(body);
                SinglePropertyUpdate::decode(&mut r, op, &self.schema)
                    .and_then(|u| r.finish().map(|()| u))
                    .map(|u| {
                        self.feed.properties(
                            recipient,
                            std::iter::once(&u.entry),
                            &self.schema,
                            &mut out,
                        );
                    })
            }
            op => {
                let mut r = WireReader::new(body);
                PropertyRemoval::decode(&mut r, op)
                    .and_then(|u| r.finish().map(|()| u))
                    .map(|u| {
                        self.feed
                            .remove_property(recipient, u.property_id, &mut out)
                    })
            }
        };
        if let Err(e) = decoded {
            self.counters.decode_errors += 1;
            if matches!(e, WorldError::UnsupportedAppearanceBaseModel) {
                self.counters.appearance_base_model += 1;
            }
            tracing::debug!(opcode, error = %e, "official world: entity message refused");
        }
        self.feed_output(out);
        outcome
    }

    /// A batch `0x0D` or `0x4C` (`official-messages-world.md` §5.1): its sub-events in
    /// order, each with the target [`sub_event_target`] gives (module docs "Entities"). A
    /// `0x0D` inside a batch is read with or without its tail flag (counted), a batch
    /// deeper than [`MAX_BATCH_DEPTH`] is not read (counted).
    fn batch(&mut self, opcode: u32, recipient: u64, body: &[u8], depth: usize) {
        if depth >= MAX_BATCH_DEPTH {
            self.feed.counters_mut().deep_batches += 1;
            return;
        }
        let aoi = opcode == SubEventBatch::EVENT_ID_AOI_ENTER;
        let decoded = if depth > 0 && aoi {
            // The frames as a `0x4C` would have them, then 0 or 1 byte of tail (spec silent;
            // question in `docs/m8-remediation.md` §15 (W §5.1)).
            let mut r = WireReader::new(body);
            SubEventBatch::decode(&mut r, SubEventBatch::EVENT_ID_SUB_EVENT).and_then(|b| {
                match r.remaining() {
                    0 => self.feed.counters_mut().nested_aoi_no_tail += 1,
                    1 => self.feed.counters_mut().nested_aoi_tail += 1,
                    _ => return r.finish().map(|()| b),
                }
                Ok(b)
            })
        } else {
            SubEventBatch::from_bytes(body, opcode)
        };
        let batch = match decoded {
            Ok(b) => b,
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(opcode, error = %e, "official world: batch refused");
                return;
            }
        };
        let mut previous = None;
        for frame in batch.frames {
            let after = previous.take();
            match frame {
                SubEventFrame::Null { .. } => self.feed.counters_mut().null_sub_events += 1,
                SubEventFrame::Event {
                    sub_event_id,
                    payload,
                } => match sub_event_target(aoi, sub_event_id, recipient, after) {
                    SubTarget::Skip => self.feed.counters_mut().overflow += 1,
                    SubTarget::To(target) => {
                        previous = self.world_message(sub_event_id, target, &payload, depth + 1);
                        if !self.in_world_still() {
                            return;
                        }
                    }
                },
            }
        }
    }

    /// A message of the play layer (`official-messages-play.md` §4–§8). The positions of
    /// every entity but the local player go to the feed ([`EntityFeed::place`]).
    fn play_message(&mut self, opcode: u32, recipient: u64, body: &[u8]) {
        let decoded = decode_play_message(opcode, recipient, body, &self.schema, &self.schema);
        let msg = match decoded {
            Ok(Some(m)) => m,
            Ok(None) => {
                self.counters.unhandled += 1;
                return;
            }
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(opcode, error = %e, "official session: play message refused");
                return;
            }
        };
        let placement = match &msg {
            PlayMessage::PositionUpdate(u) => Some((
                u.entity_id,
                placement_location(&u.placement).map(|(l, _, h)| (l, h)),
                velocity_or_zero(u.velocity),
            )),
            PlayMessage::SetPosition { entity, msg } => Some((
                *entity,
                msg.position
                    .as_ref()
                    .and_then(|p| position_location(p, None))
                    .map(|(l, _, h)| (l, h)),
                msg.velocity.unwrap_or_default(),
            )),
            _ => None,
        };
        if let Some((id, at, velocity)) = placement
            && !self.feed.is_player(id)
        {
            let mut out = FeedOutput::default();
            self.feed.place(id, at, velocity, &mut out);
            self.feed_output(out);
            return;
        }
        let out = if self.message_events >= MAX_MESSAGE_PLAY_EVENTS {
            // Its events would be dropped: no text is looked up (module docs "Entities").
            let skipped = SkippedTexts::default();
            let out = self.adapter.handle(&msg, &skipped);
            self.counters.texts_skipped += skipped.0.get();
            out
        } else {
            self.adapter.handle(&msg, &self.schema)
        };
        self.pending_out.extend(out.outgoing);
        for e in out.events {
            if let Event::Disconnected { reason } = e {
                self.end(reason);
                return;
            }
            if self.message_events >= MAX_MESSAGE_PLAY_EVENTS {
                self.counters.message_events_dropped += 1;
                continue;
            }
            self.message_events += 1;
            self.events.push_back(e);
        }
        for s in out.signals {
            match s {
                PlaySignal::LeftWorld { .. } => {
                    // §7.2's teardown: back to character selection on the same session.
                    self.left_world();
                    return;
                }
                PlaySignal::Path {
                    entity, waypoints, ..
                } => {
                    let mut out = FeedOutput::default();
                    self.feed.path(entity, &waypoints, &mut out);
                    self.feed_output(out);
                }
                PlaySignal::MotionController { .. } => {
                    self.feed.counters_mut().motion_controllers += 1;
                }
                PlaySignal::AnimLayers { .. } => self.feed.counters_mut().anim_layers += 1,
                _ => {}
            }
        }
    }

    fn send_on(link: &mut Link, codecs: &ChatCodecs, opcode: u32, env: &[u8], now: f64) -> bool {
        let Some(c) = link.mux.connection_mut(link.conn) else {
            return false;
        };
        match c.send_blob(SEND_QUEUE, 0, opcode, env, now, codecs.outgoing.as_deref()) {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(opcode, error = %e, "official session: message not queued");
                false
            }
        }
    }

    /// Sends `0x6F` on the connection that delivered the latest character list: the login
    /// connection at first, the world connection after a return from the world
    /// (`official-messages-play.md` §7.2 item 3 keeps that connection open). Which
    /// connection carries a `0x6F` after a return is not in the spec: engineering choice
    /// pending a spec answer (listed in `docs/m8-remediation.md` §15).
    fn send_entry(&mut self, env: &[u8], now: f64) -> bool {
        let opcode = CharacterEnterWorldRequest::EVENT_ID;
        match self.world.as_mut() {
            Some(w) if self.list_on_world => {
                let ok = Self::send_on(w, &self.codecs, opcode, env, now);
                if !ok {
                    self.counters.send_failures += 1;
                }
                ok
            }
            _ => self.send_on_login(opcode, env, now),
        }
    }

    fn send_on_login(&mut self, opcode: u32, env: &[u8], now: f64) -> bool {
        let ok = Self::send_on(&mut self.login, &self.codecs, opcode, env, now);
        if !ok {
            self.counters.send_failures += 1;
        }
        ok
    }

    fn send_pending(&mut self, now: f64) {
        while let Some(ev) = self.pending_out.pop_front() {
            let (opcode, payload) = ev.into_parts();
            let env = encode_event_envelope(&payload);
            let ok = match &mut self.world {
                Some(w) => Self::send_on(w, &self.codecs, opcode, &env, now),
                None => Self::send_on(&mut self.login, &self.codecs, opcode, &env, now),
            };
            if !ok {
                self.counters.send_failures += 1;
            }
        }
    }

    fn links_mut(&mut self) -> Vec<&mut Link> {
        let mut links: Vec<&mut Link> = Vec::with_capacity(2 + self.old.len());
        links.push(&mut self.login);
        if let Some(w) = self.world.as_mut() {
            links.push(w);
        }
        links.extend(self.old.iter_mut());
        links
    }

    fn tick(&mut self, now: f64) {
        let mut failed = None;
        for l in self.links_mut() {
            if let Err(e) = l.mux.tick(now) {
                failed = Some(e);
            }
        }
        if let Some(e) = failed
            && let Some(skipped) = self.send_log.allow(now)
        {
            tracing::debug!(error = %e, skipped, "official session: tick failed");
        }
        self.old.retain(Link::live);
    }

    /// Sends what the connections have queued; a failure is logged at most once per
    /// [`LOG_INTERVAL_SEC`] (a revoked right fails every send).
    fn flush(&mut self, now: f64) {
        let mut links: Vec<&mut Link> = Vec::with_capacity(2 + self.old.len());
        links.push(&mut self.login);
        if let Some(w) = self.world.as_mut() {
            links.push(w);
        }
        links.extend(self.old.iter_mut());
        let mut failed = None;
        for l in links {
            if let Err(e) = l.mux.flush_outbound(&mut self.transport) {
                failed = Some(e);
            }
        }
        if let Some(e) = failed
            && let Some(skipped) = self.send_log.allow(now)
        {
            tracing::debug!(error = %e, skipped, "official session: send failed");
        }
    }

    /// Runs the chat client, letting it read one datagram per poll while the driver is
    /// below [`DRIVER_EVENT_CAP`], at most [`CHAT_DATAGRAMS_PER_STEP`] (module docs "Bounded
    /// events").
    fn run_chat(&mut self, now: f64) {
        let Some(chat) = self.chat.as_mut() else {
            return;
        };
        for _ in 0..CHAT_DATAGRAMS_PER_STEP {
            if self.events.len() >= DRIVER_EVENT_CAP {
                break;
            }
            self.chat_allowance.store(1, Ordering::SeqCst);
            match chat.poll(now, &self.schema) {
                Ok(p) => self.events.extend(p.events),
                Err(e) => {
                    tracing::debug!(error = %e, "official chat session: poll failed");
                    break;
                }
            }
            if self.chat_allowance.load(Ordering::SeqCst) != 0 {
                // Nothing was read: the sockets are empty.
                break;
            }
        }
        self.chat_allowance.store(0, Ordering::SeqCst);
    }

    /// Ends the refresh thread: its stop flag is set (a request not yet sent is not sent)
    /// and it is joined if it has finished. A request already on the wire cannot be
    /// aborted (the HTTP crate has no cancel): it ends by its own timeout, its answer is
    /// dropped (and wiped) unread, and the reaper's revocation ends its client's right.
    fn stop_refresh(&mut self) {
        if let Some(r) = self.refreshing.take() {
            r.stop.store(true, Ordering::SeqCst);
            if r.thread.is_finished() {
                let _ = r.thread.join();
            }
            if let Ok((Ok(mut resp), _)) = r.rx.try_recv() {
                zeroize_vec(&mut resp.body);
            }
        }
    }

    /// The in-game ticket refresh (`official-login.md` §4.3), on a thread of its own so a
    /// slow answer never stalls the session. None is started once the shutdown began or
    /// `stop` holds.
    fn run_refresh(&mut self, now: f64, stop: &dyn Fn() -> bool) {
        if self.shutting_down || self.ended {
            return;
        }
        if let Some(r) = &self.refreshing {
            match r.rx.try_recv() {
                Ok((result, http)) => {
                    if let Some(done) = self.refreshing.take() {
                        let _ = done.thread.join();
                    }
                    self.background_http = Some(http);
                    let mut result = result;
                    let outcome = self.refresher.handle_refresh_response(result.as_ref(), now);
                    if let Ok(resp) = result.as_mut() {
                        zeroize_vec(&mut resp.body);
                    }
                    self.refresh_outcome(outcome);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.refreshing = None,
            }
            return;
        }
        if stop() {
            return;
        }
        let Some(req) = self.refresher.poll_refresh_request(now) else {
            return;
        };
        let mut req = req;
        let Some(mut http) = self.background_http.take() else {
            req.zeroize_body();
            return;
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let cancelled = Arc::clone(&self.cancel);
        let plain = self.plain.clone();
        let spawned = std::thread::Builder::new()
            .name("tessera-official-refresh".into())
            .spawn(move || {
                let mut req = req;
                // Right before the request: the driver's stop (shutdown) and the attempt's
                // cancel (`Disconnect`, drop), which the connector sets from its own thread.
                if thread_stop.load(Ordering::SeqCst) || cancelled.load(Ordering::SeqCst) {
                    req.zeroize_body();
                    return;
                }
                let given = std::mem::take(&mut req.url);
                // Checked again before every request of the exchange, so a stop or cancel
                // while the HTTPS form is tried sends no plain request after it.
                let halted =
                    || thread_stop.load(Ordering::SeqCst) || cancelled.load(Ordering::SeqCst);
                let result = super::login_phase::exchange(
                    &mut super::net::StopCheckedHttp::new(&mut *http, &halted),
                    super::login_phase::ChainStep::TicketRefresh,
                    &given,
                    &mut req,
                    &plain,
                );
                req.zeroize_body();
                drop(req);
                let _ = tx.send((result, http));
            });
        match spawned {
            Ok(thread) => self.refreshing = Some(Refreshing { rx, stop, thread }),
            Err(_) => tracing::warn!("official session: ticket refresh thread not started"),
        }
    }

    fn refresh_outcome(&mut self, outcome: RefreshOutcome) {
        match outcome {
            RefreshOutcome::Refreshed {
                notification: Some(n),
            } => {
                self.handoff.update_ticket(n.ticket());
                if let Some(chat) = self.chat.as_mut()
                    && chat
                        .config_mut()
                        .set_ticket(Some(n.ticket().to_owned()))
                        .is_err()
                {
                    tracing::debug!("official chat session: refreshed ticket refused");
                }
                tracing::info!("official session: ticket refreshed");
            }
            RefreshOutcome::Refreshed { notification: None } => {}
            RefreshOutcome::FailedWillRetry { attempt_count, .. } => {
                tracing::info!(attempt_count, "official session: ticket refresh failed");
            }
            RefreshOutcome::Exhausted(_) => {
                tracing::warn!("official session: ticket refresh exhausted; the session goes on");
            }
        }
    }

    /// A neutral command (`official-messages-play.md` §8, `official-messages-world.md` §6).
    ///
    /// # Errors
    ///
    /// [`tessera_net::NetError::InvalidCommand`] for commands this session cannot carry.
    pub fn command(&mut self, cmd: &Command, now: f64) -> Result<(), tessera_net::NetError> {
        use tessera_net::NetError;
        if self.ended {
            return Err(NetError::InvalidCommand("the official session ended"));
        }
        match cmd {
            Command::EnterWorld { character } => {
                if self.stage != Stage::Characters {
                    return Err(NetError::InvalidCommand("not at character selection"));
                }
                let Some((id, name)) = self
                    .characters
                    .iter()
                    .find(|(_, n)| n == character)
                    .cloned()
                else {
                    self.events.push_back(Event::EnterWorldFailed {
                        character: character.clone(),
                        reason: "No such character on this account.".to_owned(),
                    });
                    return Ok(());
                };
                let env = CharacterEnterWorldRequest::new(id)
                    .to_envelope_bytes()
                    .map_err(|_| NetError::InvalidCommand("character not encodable"))?;
                if !self.send_entry(&env, now) {
                    self.events.push_back(Event::EnterWorldFailed {
                        character: name,
                        reason: "The request could not be sent.".to_owned(),
                    });
                    return Ok(());
                }
                self.stage = Stage::Entering {
                    id,
                    name,
                    since: now,
                };
                Ok(())
            }
            Command::LeaveWorld => {
                if !matches!(self.stage, Stage::InWorld { .. }) {
                    return Err(NetError::InvalidCommand("not in the world"));
                }
                // §7.2 item 1: `0x851E002F`, unless a countdown already runs (its answer
                // comes as system lines, then the teardown).
                match self.adapter.request_logout() {
                    Ok(Some(ev)) => self.pending_out.push_back(ev),
                    Ok(None) => {
                        tracing::debug!("official session: a logout countdown already runs");
                    }
                    Err(_) => return Err(NetError::InvalidCommand("not in the world")),
                }
                Ok(())
            }
            Command::WorldLoaded => {
                // §4.5 item 4: the client's loading for its latest placement is done. One
                // signal per portal space: outside one it is ignored (module docs).
                if matches!(self.stage, Stage::InWorld { .. })
                    && self.adapter.teleport().in_portal_space()
                {
                    self.loaded = true;
                } else {
                    self.counters.stray_loaded += 1;
                    tracing::debug!("official session: loading signal outside portal space");
                }
                Ok(())
            }
            Command::Move { .. } | Command::Chat { .. }
                if !matches!(self.stage, Stage::InWorld { .. }) =>
            {
                // Sent before the client saw the return (module docs "Leaving the world").
                self.counters.outside_world += 1;
                tracing::info!(
                    command = cmd.name(),
                    "official session: not in the world; refused, not sent"
                );
                // A refused move stays silent; a refused chat line is a line the player
                // typed, so a system notice says it was not sent. `ServerNotice`, not
                // `ChatRejected`: the session is back at character selection, where a
                // world event is not surfaced. Text and event are an engineering choice
                // (spec silent, `docs/m8-remediation.md` §15). At most one per command, and
                // none once the driver holds its cap of events (module docs "Bounded
                // events").
                if matches!(cmd, Command::Chat { .. }) && self.events.len() < DRIVER_EVENT_CAP {
                    self.events.push_back(Event::ServerNotice {
                        level: NoticeLevel::Info,
                        seconds: 0,
                        text: NOT_SENT_OUTSIDE_WORLD.to_owned(),
                    });
                }
                Err(NetError::InvalidCommand("not in the world"))
            }
            Command::Move {
                location,
                heading,
                velocity,
                ..
            } => {
                let input = movement_input(*heading, *velocity);
                let predicted = PredictedState {
                    location: *location,
                    heading: *heading,
                };
                let out = self.adapter.movement_step(input, now, Some(predicted));
                self.pending_out.extend(out);
                Ok(())
            }
            Command::Chat {
                channel: ChatChannel::Fellowship | ChatChannel::Named { .. },
                ..
            }
            | Command::Social(
                tessera_net::SocialCommand::JoinChannel { .. }
                | tessera_net::SocialCommand::LeaveChannel { .. },
            ) => {
                let chat = self
                    .chat
                    .as_mut()
                    .ok_or(NetError::InvalidCommand("the chat server is not connected"))?;
                chat.command(cmd, now)
                    .map_err(|_| NetError::InvalidCommand("chat-server command refused"))
            }
            Command::Chat { .. } => {
                let out = self
                    .adapter
                    .command(cmd)
                    .map_err(|_| NetError::InvalidCommand("chat command refused"))?;
                self.pending_out.extend(out);
                Ok(())
            }
            other => {
                tracing::debug!(
                    command = other.name(),
                    "official session: command not carried"
                );
                Err(NetError::InvalidCommand(
                    "this command is not supported on the official servers yet",
                ))
            }
        }
    }

    /// Ends the session gracefully (`official-session.md` §8.2, `official-messages-play.md`
    /// §7.2 item 4: the transport disconnect on every connected connection; module docs
    /// "Ending the session": a connection still in its handshake is closed without sending).
    /// Play messages not yet queued on a connection are dropped. No ticket refresh starts
    /// after this, and a running one is stopped.
    pub fn begin_shutdown(&mut self, now: f64) {
        self.ended = true;
        self.shutting_down = true;
        // Nobody takes them any more; reading must go on for the disconnect's answers.
        self.events.clear();
        self.inbox.clear();
        self.pending_out.clear();
        self.entry_messages.clear();
        self.entry_bytes = 0;
        self.stop_refresh();
        self.log_abandoned_handshakes("cancelled", now);
        for l in self.links_mut() {
            l.end_session(now);
        }
        self.old.retain(Link::live);
        if let Some(chat) = self.chat.as_mut() {
            chat.close(now);
        }
        self.tick(now);
        self.flush(now);
    }

    /// Ends the session without sending anything (the network right was revoked: nothing
    /// could be sent): every connection is closed locally and the chat client dropped. A
    /// connection still in its handshake logs its counts once, at `now`.
    pub fn abandon(&mut self, now: f64) {
        self.ended = true;
        self.shutting_down = true;
        self.events.clear();
        self.inbox.clear();
        self.pending_out.clear();
        self.entry_messages.clear();
        self.entry_bytes = 0;
        self.stop_refresh();
        self.log_abandoned_handshakes("network right revoked", now);
        for l in self.links_mut() {
            l.close_quietly();
        }
        self.old.clear();
        self.chat = None;
    }

    /// One pass of the shutdown: runs the connections until their `Disconnect` is out.
    /// Returns whether anything is still flushing.
    pub fn shutdown_step(&mut self, now: f64) -> bool {
        // `ended` holds: `run_links` hands nothing on, and no referral is accepted.
        self.receive(now, &|| true);
        for l in self.links_mut() {
            l.drain_events();
        }
        self.inbox.clear();
        if let Some(chat) = self.chat.as_mut() {
            // Its events are dropped: it may read all it has.
            self.chat_allowance.store(usize::MAX, Ordering::SeqCst);
            let _ = chat.poll(now, &self.schema);
            self.chat_allowance.store(0, Ordering::SeqCst);
        }
        self.tick(now);
        self.flush(now);
        self.login.flushing()
            || self.world.as_ref().is_some_and(Link::flushing)
            || self.old.iter().any(Link::flushing)
    }

    /// Opens the chat-server session with `binder` (`official-messages-play.md` §6.6). A
    /// failure is tried again after [`CHAT_RETRY_SEC`] (§6.6.3's Initial Connect Policy),
    /// with one warning. `stop` is looked at right before the socket is bound: once it holds
    /// (the attempt was cancelled) nothing is bound and the chat session is not opened.
    pub fn attach_chat(
        &mut self,
        binder: Box<dyn FnMut() -> Result<N::Udp, crate::play::PlayError> + Send>,
        player: u64,
        name: &str,
        now: f64,
        stop: &dyn Fn() -> bool,
    ) {
        let Some(endpoint) = self.chat_endpoint else {
            return;
        };
        if stop() || self.ended {
            return;
        }
        let config = ChatConfig::new(
            endpoint,
            self.handoff.subscription_name(),
            Some(self.handoff.gls_ticket().to_owned()),
            self.client_timestamp,
        )
        .with_send_checksum(self.send_checksum);
        let mut binder = binder;
        let allowance = Arc::clone(&self.chat_allowance);
        let metered: crate::chat_session::client::TransportBinder<Metered<N::Udp>> =
            Box::new(move || {
                binder().map(|inner| Metered {
                    inner,
                    allowance: Arc::clone(&allowance),
                })
            });
        match ChatClient::with_binder(metered, config, self.codecs.clone(), now) {
            Ok(mut chat) => {
                chat.set_self_descriptor(player, name);
                self.chat = Some(chat);
            }
            Err(e) => {
                self.chat_failed(now);
                tracing::debug!(error = %e, "official chat session not started");
            }
        }
    }

    /// The chat-server session could not be opened (no socket, or the network has no
    /// chat binder): the next attempt waits [`CHAT_RETRY_SEC`].
    pub fn chat_failed(&mut self, now: f64) {
        self.counters.chat_failures += 1;
        self.chat_retry_at = now + CHAT_RETRY_SEC;
        if !self.chat_warned {
            self.chat_warned = true;
            tracing::warn!("official chat session could not be opened; retrying every 30 s");
        }
    }

    /// When the next pass has timed work, on the clock of `now` (the connector waits until
    /// then, `worker` "Waiting"): the earliest timer of the connections
    /// ([`SessionMultiplexer::next_deadline`], replaced connections finishing their
    /// disconnect included), the entry deadline ([`ENTER_WORLD_TIMEOUT_SEC`]), the next
    /// chat-server attachment ([`Self::wants_chat`]) and the next ticket refresh
    /// (`official-login.md` §4.3); `now` when messages wait in the inbox below the event
    /// cap. A time already past is `now`; `None` once the session ended without a shutdown,
    /// or when nothing is timed.
    ///
    /// The chat client and the reply of a running refresh have no deadline of their own
    /// here: the connector's longest wait (50 ms) bounds how late they are looked at.
    #[must_use]
    pub fn next_deadline(&self, now: f64) -> Option<f64> {
        if self.ended && !self.shutting_down {
            return None;
        }
        let mut next: Option<f64> = None;
        let mut at = |t: f64| next = Some(next.map_or(t, |n| n.min(t)));
        let links = std::iter::once(&self.login)
            .chain(self.world.iter())
            .chain(self.old.iter());
        for l in links {
            if let Some(t) = l.mux.next_deadline(now) {
                at(t);
            }
        }
        if !self.ended {
            if let Stage::Entering { since, .. } = &self.stage {
                at(since + ENTER_WORLD_TIMEOUT_SEC);
            }
            // Portal space's loading time-out (§4.5 item 4; `portal_step` notes when it
            // began on its first pass).
            if matches!(self.stage, Stage::InWorld { .. })
                && self.adapter.teleport().in_portal_space()
            {
                at(self.portal_since.unwrap_or(now) + PORTAL_LOADING_TIMEOUT_SEC);
            }
            if matches!(self.stage, Stage::InWorld { .. })
                && self.chat.is_none()
                && self.chat_endpoint.is_some()
            {
                at(self.chat_retry_at);
            }
            // The conditions under which `run_refresh` acts on a past deadline
            // (`TicketRefresher::poll_refresh_request`'s guards and no
            // refresh running): a refresh that step would not send is no deadline, so it can
            // never hold the loop at its shortest wait.
            let r = &self.refresher;
            if self.refreshing.is_none()
                && r.lifetime_sec() > 0
                && r.attempt_count() <= 2
                && !r.ticket().is_empty()
                && !r.account_name().is_empty()
                && !r.auth_server_url().is_empty()
            {
                at(r.start_time_sec() + r.next_refresh_offset_sec());
            }
            if !self.inbox.is_empty() && !self.full() {
                at(now);
            }
        }
        next.map(|t| t.max(now))
    }

    /// The player and name once in the world, for [`Self::attach_chat`], when an attempt
    /// is due.
    #[must_use]
    pub fn wants_chat(&self, now: f64) -> Option<(u64, String)> {
        match &self.stage {
            Stage::InWorld { player, name }
                if self.chat.is_none()
                    && self.chat_endpoint.is_some()
                    && !self.ended
                    && now >= self.chat_retry_at =>
            {
                Some((*player, name.clone()))
            }
            _ => None,
        }
    }
}
