// SPDX-License-Identifier: GPL-3.0-or-later
//! The UDP part of the live session: the login connection with its initial exchange
//! (`official-session.md` §6, §9.3; "The initial exchange" below), the character list and world
//! entry
//! (`official-messages-world.md` §1.1.4, §3, §4), the world connection after the referral
//! (`official-session.md` §6.5), the play messages through
//! [`crate::play::adapter::PlayAdapter`] (`official-messages-play.md` §4–§8), the chat
//! server (§6.6) and the in-game ticket refresh (`official-login.md` §4.3).
//!
//! # The initial exchange
//!
//! `official-session.md` §9.3.1–§9.3.8 and decision 0046 ([`crate::initial`]): after State 5
//! the login connection waits for the server's `0xDD`, sends the install advertisement
//! `0xEC` built before the first datagram from the user's own archives, with the recipient of the
//! valid `0xDD` in an explicit field, answers the server's `0xF5` once, likewise, and waits for
//! the character list. Before `WorldEntered` every other message of the login connection is
//! skipped and counted per opcode: no dispatch, no reply and no decode error. An actual `0x12F`
//! switches to the historical path (§9.3.8). `0x12E` is shown and never ends anything.
//!
//! # How a session ends
//!
//! One table decides (§9.3.8's phase table, `TableRow`). Before `Connected`: `LoginRefused`, then
//! `Disconnected` in the same poll. After it no Account refusal goes out: the same row ends in a
//! `Disconnected` of its own, with Tessera's words for a Tessera limit. The one exception is a
//! valid `0x6D` while selecting: `EnterWorldFailed` and back to the list. A local end sends one
//! standalone Disconnect when the connection is established, closes with no flush wait and sends
//! the ticket no more; a server's closing header gets no reply. The user's "Leave queue" ends the
//! attempt the same way, with `Disconnected { ClientRequested }` (`official-entry.md` §8.1).
//!
//! # The road into the world
//!
//! `0x6F` goes on queue 2 to the fixed reserved destination [`SELECTION_DESTINATION`], with an empty
//! client table. Separate 30-second waits follow, none reset by other traffic ([`EntryWait`]): to
//! the referral, to the world connection's State 5, to a complete valid `0x90`, and from `0x90` to
//! readiness. Pre-referral server `0x30` are counted and discarded. On the world connection the
//! checked messages are read with their one shared byte and counted, never published; `0x7C` and
//! `0x0D` are counted as unsupported (OPEN-WORLD-9).
//!
//! `WorldEntered` needs valid server readiness, local scene readiness and a sourced `0xF4`.
//! OPEN-WORLD-9 and OPEN-SESSION-18 leave the server's readiness and the `0xF4` target without a
//! source, so on every production path the entry stops at the readiness wait and ends with a
//! `Disconnected` that names that stage. No readiness indication is made up and no `0xF4` is sent.
//! [`ConnectorNet::readiness_specified`] is `false` on [`super::net::LiveNet`]: only a test's
//! synthetic network reaches the in-world code below, which waits for those answers.
//!
//! Omitted under decision 0046 item 5, each a known difference from the observed client: `0x13D`
//! and `0x19E` (OPEN-SESSION-20) and the first world `0x30` (OPEN-SESSION-16). `0x154`, `0xDC`,
//! `0x7A` and `0xD1` are withheld (§9.3.8): those commands fail locally. A re-entry after a teardown
//! has no settled queue or body (OPEN-SESSION-16), and the chat server is not opened while
//! OPEN-SESSION-1 leaves its codec without a source: the primary session does not depend on it, and
//! a user's chat-server request fails locally without disconnecting it.
//!
//! # One multiplexer per connection
//!
//! The login connection and every world connection each get a multiplexer of their own,
//! all on the session's one socket. `official-session.md` §3.4 bullet 2 and §6.4 step 1 say
//! a challenge that assigns an ID another active connection holds is ignored, while §3.4
//! step 3's example has a world server assign the ID the disconnecting login connection
//! still holds. Tessera follows the example: with one multiplexer per connection the IDs of
//! one server never meet another's, so such a challenge is accepted (engineering choice
//! where the two passages differ; question in `docs/m8-remediation.md` §15; tested). A datagram
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
//! list nor how a name that is gone is surfaced. Entering the world again after a teardown is
//! not supported (OPEN-SESSION-16): nothing is sent and the session ends with Tessera's own text.
//! Moves and chat lines that reach the driver
//! after the teardown (sent before the client saw the return) are refused and counted
//! ([`DriverCounters::outside_world`]), never sent; a refused move stays silent, a refused
//! chat line also gives a system notice (`Event::Notice`), so the player sees that the line
//! was not sent. `ReturnedToCharacterSelect` is Tessera's neutral addition (§8 maps the
//! teardown to the later list; engineering choice pending a spec answer, listed in
//! `docs/m8-remediation.md` §15).
//!
//! # Entities
//!
//! The target of a message is the fragment's `recipientId`. `official-messages-world.md`
//! §1.1.2 makes the routing header's `routingField0` the target when it has one
//! ([`crate::world::active_target`]), but that field is a `u32` (`official-session.md` §9.1)
//! while world InstanceIDs are 64-bit, and the spec does not say how one maps to the
//! other. Until it does, a routed envelope keeps `recipientId` and is counted
//! ([`DriverCounters::routed_envelopes`]), so no `0xD0` asks for an ID made from 32 bits
//! (engineering choice, question in `docs/m8-remediation.md` §15). In the world, batches (`0x0D`, `0x4C`, §5.1) are
//! unpacked in order with their target (§1.1.3; the one exception, the `0x52` each `0x98`
//! takes as its initial property set, goes to the created entity:
//! [`super::entities::pair_initial_properties`], [`super::entities::paired_target`]), the
//! entity lifecycle and property events (`0x98`, `0x40`, `0x52`, `0x88`, `0x92`, `0x0E`,
//! `0xE7`, `0x89`) go to the [`EntityFeed`] and the rest to the play layer. The feed turns
//! them into `EntityCreated`, `AppearanceChanged` and `EntityDespawned` ([`super::entities`]
//! has the mapping); the positions of every entity but the local player go to the feed too:
//! they place the entities it waits for, and the remote motion of created entities (`0xCE`,
//! `0xBF`, and `0x59` paths followed in time) is `EntityMoved`. `0xED` and `0xBD`
//! reach no neutral event and are counted ([`FeedCounters`]).
//!
//! Every `0x0D`, a nested one too, ends with its 1-byte tail flag
//! (`official-messages-world.md` §5.1 row 4). Only the feed and batch messages must start
//! with `base_flags = 0` here; the play decoders check their own bodies.
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
//! [`PORTAL_LOADING_TIMEOUT_SEC`] (§4.5 item 4). The client sends one `WorldLoaded` per
//! placement; the driver keeps the latest with the entity stamp of the placement it
//! answers, and it counts for a portal space whose expected stamp that placement reached.
//! So a `WorldLoaded` of an earlier placement never ends a portal space, and one sent for a
//! placement that came before its `0x5E` (item 2 allows that order) still does
//! (engineering choice, `docs/m8-remediation.md` §15). A signal outside the world is
//! ignored.
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
//! [`SessionDriver::handshake_report`] gives them to tests). Every datagram is sent and
//! verified with the per-buffer checksum of `official-session.md` §4.4.2. A cancel or a
//! revoked network right during a handshake logs the counts once
//! ([`super::diag::HandshakeReport::log_ended`]).
//!
//! With the datagram dump on ([`SessionStart::dump`], [`super::dump`]; off by default) the
//! session's socket is a [`Tap`]: it is armed only around the flush of a connection whose
//! first datagram is not yet written, so that datagram is written exactly as the socket took
//! it (credentials replaced), and the connection's counts are appended when it closes or is
//! let go. Each chat-server socket does the same for its own connection ([`Metered`]).

use std::collections::VecDeque;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

use tessera_net::account::{AccountCommand, AccountEvent, LoginRefusal, LoginStage};
use tessera_net::model::{MAX_NAME_LEN, is_valid_name};
use tessera_net::{
    CharacterEvent, CharacterSummary, ChatChannel, Command, DisconnectReason, EntityId, Event,
    NoticeLevel, Transform,
};

use crate::chat_session::state::INITIAL_CONNECT_POLICY;
use crate::chat_session::{ChatClient, ChatCodecs, ChatConfig};
use crate::initial::{
    COMPLETION_DATA, CompletionStep, InitialStages, InstallRequest, Negotiation, OPCODE_COMPLETION,
    OPCODE_INSTALL_ADVERTISEMENT, OPCODE_INSTALL_REQUEST, PreparedAdvertisement,
    UnsupportedNegotiation, check_language_allowed,
};
use crate::login::{
    HttpResponse, LoginError, NetPreferences, RefreshOutcome, SessionHandoff, TicketRefresher,
    VersionInfo, zeroize_vec,
};
use crate::play::adapter::{
    PlayAdapter, PlayCounters, PlaySignal, placement_location, position_location, velocity_or_zero,
};
use crate::play::chat::TextResolver;
use crate::play::inbound::{PlayMessage, decode_play_message};
use crate::play::movement::{MoveStep, PredictedState};
use crate::play::{OutgoingEvent, clean_display_name};
use crate::session::{
    BindPlan, BindPorts, ClientVersionMessage, CodecUuids, ConnectionState, ControlMessage,
    DEFAULT_MAX_CONTAINER_DEPTH, DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
    DisconnectReason as LinkEnd, LogonOutcome, MAX_DATAGRAM_BUF, MIN_DATAGRAM_BYTES,
    OPCODE_BATCH_CONTAINER, OPCODE_CLIENT_VERSION, OPCODE_LOGIN_QUEUE_STATUS,
    OPCODE_SINGLE_WRAPPER, ReassembledBlob, ReassemblyCounts, SessionError, SessionEvent,
    SessionMultiplexer, UdpDatagramTransport, dispatch_message, max_recv_rate_for_speed,
};
use crate::world::{
    AccountEvictionNotice, CharacterEnterWorldRequest, CharacterListResponse,
    CharacterOperationError, EntityCreate, EntityDespawn, PlayerSessionReady, PropertyRemoval,
    PropertyScope, PropertyTableUpdate, SinglePropertyUpdate, SubEventBatch, SubEventFrame,
    VisualDescriptionUpdate, WireReader, WorldError, active_target, encode_event_envelope,
    one_shared_prefix, rebuild_shared_prefix,
};

use super::data::{SchemaView, chat_categories, prepare_advertisement};
use super::diag::{
    AuthMode, BindCase, HANDSHAKE_REPORT_SEC, HandshakeReport, LinkDiag, LocalSource,
    LoginServerEntry, RejectKind, RemoteForm, RemoteName, carries_challenge, error_kind,
};
use super::dump::{
    AtClose, Caught, ConnectionDump, DumpDir, DumpKind, Endpoint, SocketDump, Tap, Target,
};
use super::entities::{
    CreateOutcome, EntityFeed, FeedCounters, FeedOutput, SubTarget, pair_initial_properties,
    paired_target,
};
use super::login_phase::PlainSteps;
use super::net::{ConnectorNet, SendHttpClient};

/// `0x0000019E` (`ClientPlatformIdMessage`, `official-session.md` §5.6.2).
pub const OPCODE_CLIENT_PLATFORM_ID: u32 = 0x0000_019E;
/// `0x0000006F` (`CharacterSelectMessage`, §5.6.2).
pub const OPCODE_CHARACTER_SELECT: u32 = 0x0000_006F;
/// The fixed reserved protocol service destination of the initial `0x6F` (and `0x19E`, which is
/// omitted): `official-session.md` §9.3.8, BIN-L2-10G-DEST. A built-in routing constant, not a
/// character, account or install value; always in the explicit eight-byte recipient form.
pub const SELECTION_DESTINATION: u64 = 0x0600_0000_0000_0401;
/// `0x00000024` (`LocalMovementStep`, §5.6.2; `official-messages-play.md` §4.2).
pub const OPCODE_MOVEMENT_STEP: u32 = 0x0000_0024;
/// `0x000000F4` (`portal_ready_ack`, §5.6.2; `official-messages-play.md` §4.5).
pub const OPCODE_PORTAL_READY_ACK: u32 = 0x0000_00F4;
/// `0x00000030` (`entity_method_call`, §5.6.2).
pub const OPCODE_ENTITY_METHOD_CALL: u32 = 0x0000_0030;

/// Queue `1`, `Unreliable` (`official-session.md` §5.6.2).
pub const QUEUE_UNRELIABLE: u8 = 1;
/// Queue `2`, `ReliableOrdered` (§5.6.2).
pub const QUEUE_RELIABLE_ORDERED: u8 = 2;
/// Queue `3`, `Sequenced` (§5.6.2).
pub const QUEUE_SEQUENCED: u8 = 3;
/// Queue `4`, `ReliableUnordered` (§5.6.2).
pub const QUEUE_RELIABLE_UNORDERED: u8 = 4;

/// The delivery queue and sequence group of a message the client sends on the login or
/// world connection, by the client outbound table of `official-session.md` §5.6.2:
///
/// | Opcode | Queue | `seqGroup` |
/// |---|---|---|
/// | `0x12C` | 4 | — |
/// | `0xEC`, `0xF5` | 1 | — (§9.3.2, §9.3.3) |
/// | `0x19E` | 1 | — (§9.3.8; omitted under decision 0046) |
/// | `0x6F` | 2 | — (§9.3.8; to [`SELECTION_DESTINATION`]) |
/// | `0x24` | 1, or 3 when discrete ([`movement_is_discrete`]) | `0x05` on queue 3 |
/// | `0xF4` | 1 | — (§9.3.8: observed queue; its trigger is OPEN-WORLD-9) |
/// | `0x30` | 3 | `0x0A` |
/// | `0x28` | 3 | `0x04` |
/// | `0x5F` | 3 | `0x08` |
/// | `0xC9` | 3 | `0x0E` |
///
/// `payload` is the inner payload (`base_flags` at offset `0`), read only for `0x24`. An
/// opcode the table does not list goes on queue 4 with group `0` (engineering choice, spec
/// silent: reliable, with no sequence-group semantics; the spec lists no other client
/// opcode).
#[must_use]
pub fn outbound_queue(opcode: u32, payload: &[u8]) -> (u8, u16) {
    match opcode {
        OPCODE_CLIENT_VERSION => (QUEUE_RELIABLE_UNORDERED, 0),
        OPCODE_INSTALL_ADVERTISEMENT
        | OPCODE_COMPLETION
        | OPCODE_CLIENT_PLATFORM_ID
        | OPCODE_PORTAL_READY_ACK => (QUEUE_UNRELIABLE, 0),
        OPCODE_CHARACTER_SELECT => (QUEUE_RELIABLE_ORDERED, 0),
        OPCODE_MOVEMENT_STEP if !movement_is_discrete(payload) => (QUEUE_UNRELIABLE, 0),
        OPCODE_MOVEMENT_STEP => (QUEUE_SEQUENCED, 0x05),
        OPCODE_ENTITY_METHOD_CALL => (QUEUE_SEQUENCED, 0x0A),
        0x0000_0028 => (QUEUE_SEQUENCED, 0x04),
        0x0000_005F => (QUEUE_SEQUENCED, 0x08),
        0x0000_00C9 => (QUEUE_SEQUENCED, 0x0E),
        _ => (QUEUE_RELIABLE_UNORDERED, 0),
    }
}

/// Whether a `0x24` inner payload carries a discrete movement action or state change, which
/// goes on queue 3 (`official-messages-play.md` §4.2; `official-session.md` §5.6.2), rather
/// than continuous movement alone (queue 1). Tessera's reading (engineering choice: the
/// spec does not define "discrete"): a slot with queued actions or properties
/// (`slot_flags` bits `0x01`, `0x10` at `+0x12..+0x15`), or a newest step whose
/// `move_state` (`+0x11`) or jump and walk flags (`0x02`, `0x40`) differ from the step
/// before it (`+0x10`, `+0x14`). A payload too short to read counts as discrete.
#[must_use]
pub fn movement_is_discrete(payload: &[u8]) -> bool {
    let (Some(states), Some(flags)) = (payload.get(0x0E..0x12), payload.get(0x12..0x16)) else {
        return true;
    };
    let queued = flags.iter().any(|f| f & 0x11 != 0);
    let changed = states.get(2) != states.get(3)
        || flags.get(2).map(|f| f & 0x42) != flags.get(3).map(|f| f & 0x42);
    queued || changed
}

/// Most datagrams read per step.
pub const MAX_DATAGRAMS_PER_STEP: usize = 512;

/// Most events the driver lets wait before it stops reading (module docs "Bounded events";
/// `ARCHITECTURE.md` §19.4: reading stops once 256 items wait).
pub const DRIVER_EVENT_CAP: usize = super::READ_PAUSE;

/// The system notice for a chat line refused because the session left the world before
/// the client saw the return (module docs "Leaving the world"). Tessera's text, an
/// engineering choice (spec silent).
pub const NOT_SENT_OUTSIDE_WORLD: &str = "Your line was not sent: you are no longer in the world.";

/// The end of a session whose `0x90` cannot be read (§9.3.8's unsupported row, world §4.2: never
/// shifted by a guessed second byte). Tessera's text.
pub const UNSUPPORTED_SESSION_START: &str = "Tessera cannot read the world's session start yet. \
     This is a Tessera limit, not a fault of your account.";

/// The end of a session whose `0x6D` or `0xB3` cannot be read (§9.3.8). Tessera's text.
pub const UNSUPPORTED_ENTRY_ANSWER: &str = "Tessera cannot read the server's answer yet. \
     This is a Tessera limit, not a fault of your account.";

/// The end of a character list that cannot be read after `Connected` (§9.3.8). Tessera's text.
pub const UNSUPPORTED_LATER_LIST: &str = "Tessera cannot read this character list yet. \
     This is a Tessera limit, not a fault of your account.";

/// The end of a session whose `0x6F` cannot be built or queued, or of a re-entry after a teardown,
/// which has no settled queue or body (§9.3.8, OPEN-SESSION-16). Tessera's text.
pub const ENTRY_NOT_SUPPORTED: &str = "Tessera cannot enter the world again on the official \
     service yet. This is a Tessera limit, not a fault of your account.";

/// The locally worded service refusal of a selection (§9.3.8: no number, no service text).
pub const ENTRY_REFUSED: &str = "The official service refused this character's entry to the world.";

/// The local failure of a chat-server request while OPEN-SESSION-1 leaves its codec without a
/// source (§9.3.8): the primary session is not touched.
pub const CHAT_UNAVAILABLE: &str = "the chat server is not available in this version of Tessera";

/// Most old world connections kept (a later referral replaces the world connection).
pub const MAX_OLD_LINKS: usize = 4;

/// Most characters kept from one character list (the list's own bound is the bytes of the
/// message; this bounds the names the session offers).
pub const MAX_CHARACTERS: usize = 256;

/// The two wire values of `TransportErrorHeader` that mean "connection refused /
/// authentication rejected during login" (`official-session.md` §6.7).
pub const LOGIN_REFUSAL_CODES: [u32; 2] = [0x00A7_E948, 0x7DC4_1B4A];

/// Each wait on the road into the world (§9.3.8: Tessera policy, on the monotonic clock).
pub const ENTER_WORLD_TIMEOUT_SEC: f64 = 30.0;

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
///
/// With the datagram dump on ([`super::dump`]) each socket also writes the first datagram
/// it sends (the chat connection's `SessionAuthHeader`) and its counts when it is dropped.
pub struct Metered<T> {
    inner: T,
    allowance: Arc<AtomicUsize>,
    dump: Option<SocketDump>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Metered<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metered")
            .field("inner", &self.inner)
            .field("allowance", &self.allowance)
            .field("dump", &self.dump)
            .finish()
    }
}

impl<T: UdpDatagramTransport> UdpDatagramTransport for Metered<T> {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        self.inner.local_addr()
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        let sent = self.inner.send_to(buf, target);
        if let Some(d) = self.dump.as_mut() {
            let inner = &self.inner;
            d.sent(buf, target, sent.is_ok(), || inner.local_addr().ok());
        }
        sent
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        if self.allowance.load(Ordering::SeqCst) == 0 {
            return Ok(None);
        }
        let got = self.inner.recv_from(buf)?;
        if got.is_some() {
            if let Some(d) = self.dump.as_mut() {
                d.received();
            }
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
    /// The datagram dump of this connection ([`super::dump`]), when it is on.
    dump: Option<ConnectionDump>,
    /// The login connection's initial-exchange stages, for its dump; `None` on the others.
    initial: Option<InitialStages>,
    /// The ordering number the next queue-2 blob from the server would continue with
    /// (`SessionDriver::note_queue2`): the first completed one is the baseline (§9.3.8).
    queue2_next: Option<u16>,
}

impl Drop for Link {
    // A connection that is let go without a close event (replaced, reaped after its
    // disconnect, the session's end) still gets its counts in its dump.
    fn drop(&mut self) {
        self.close_dump("the connection was let go (replaced, finished or the session ended)");
    }
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
    /// A connection to `peer` (of the address form `remote`) with the install's directional
    /// dictionary UUIDs `uuids` (`official-session.md` §7.3) and whose
    /// `ConnectAcceptHeader` carries `max_recv_rate` (§6.4); `dump`: its datagram dump,
    /// when that is on.
    fn new(
        peer: SocketAddrV4,
        uuids: CodecUuids,
        max_recv_rate: u32,
        auth: AuthMode,
        remote: (RemoteForm, RemoteName),
        dump: Option<Endpoint>,
        now: f64,
    ) -> Result<Self, SessionError> {
        let mut mux = SessionMultiplexer::new();
        let conn = mux.create_connection(peer, now)?;
        if let Some(c) = mux.connection_mut(conn) {
            c.set_codec_uuids(uuids);
            c.set_max_recv_rate(max_recv_rate);
        }
        Ok(Self {
            mux,
            conn,
            peer,
            diag: LinkDiag::new(now),
            auth,
            remote,
            dump: dump.map(ConnectionDump::new),
            initial: None,
            queue2_next: None,
        })
    }

    /// Whether the dump still waits for this connection's first datagram.
    fn wants_first_dump(&self) -> bool {
        self.dump.as_ref().is_some_and(ConnectionDump::wants_first)
    }

    /// Writes the dump of the first datagram `caught`, sent from `local`.
    fn write_first_dump(&mut self, dir: &DumpDir, caught: &Caught, local: Option<SocketAddrV4>) {
        if let Some(d) = self.dump.as_mut() {
            d.first(dir, caught, local);
        }
    }

    /// Appends the connection's counts to its dump (once), for an end with `reason`.
    fn close_dump(&mut self, reason: &str) {
        let Some(dump) = self.dump.as_mut() else {
            return;
        };
        let conn = self.mux.connection(self.conn);
        let counts = conn.map(|c| c.handshake_counts()).unwrap_or_default();
        dump.close(&AtClose {
            reason: reason.to_owned(),
            state: conn.map(|c| c.closed_from().unwrap_or_else(|| c.state())),
            sent: counts.sent,
            send_failures: counts.send_failures,
            auth_sent: counts
                .session_auth
                .saturating_add(counts.referral_auth)
                .saturating_add(counts.auth_retransmits),
            auth_retransmits: counts.auth_retransmits,
            accept_sent: counts
                .connect_accept
                .saturating_add(counts.connect_accept_retransmits),
            accept_retransmits: Some(counts.connect_accept_retransmits),
            received: self.diag.received,
            received_ok: Some(counts.received_ok),
            challenges: Some(counts.challenges),
            initial: self.initial,
        });
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

    /// The connection's recipient ID, once its server's challenge assigned one (§3.4).
    fn rec_id(&self) -> Option<u16> {
        self.mux
            .connection(self.conn)
            .map(|c| c.local_rec_id())
            .filter(|&id| id != 0)
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
        self.close_dump("closed locally in its handshake (a cancel, the session's end)");
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
    /// The fragment header's delivery queue.
    queue: u8,
    /// The envelope's flags byte (§9.1).
    envelope_flags: u8,
    /// The blob itself was this message, not a container around it (§9.4.2).
    top_level: bool,
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

/// What a pending selection waits for (§9.3.8): each has its own 30-second clock, which only the
/// next stage ends and unrelated traffic does not reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryWait {
    /// From `0x6F`'s allocation to a valid referral.
    Referral,
    /// From the referral to the new world connection's State 5.
    WorldConnection,
    /// From that State 5 to a complete valid `0x90`.
    SessionStart,
    /// From `0x90` to `WorldEntered`: valid server readiness, local scene readiness and a sourced
    /// `0xF4` (OPEN-WORLD-9, OPEN-SESSION-18).
    Readiness,
}

impl EntryWait {
    /// A neutral label for a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Referral => "waiting for the referral",
            Self::WorldConnection => "connecting to the world server",
            Self::SessionStart => "waiting for the world session's start",
            Self::Readiness => "waiting for world readiness",
        }
    }

    /// Tessera's own text for this wait's end (§9.3.8: a locally worded entry deadline). The
    /// readiness wait is a Tessera limit on every production path, and says so.
    #[must_use]
    pub const fn deadline_text(self) -> &'static str {
        match self {
            Self::Referral => {
                "The official service did not send Tessera to a world server within Tessera's own \
                 time limit for this step."
            }
            Self::WorldConnection => {
                "Tessera could not finish connecting to the world server within Tessera's own time \
                 limit for this step."
            }
            Self::SessionStart => {
                "The world server did not start the session within Tessera's own time limit for \
                 this step."
            }
            Self::Readiness => {
                "Tessera reached the world server, but this version cannot finish entering the \
                 world yet. This is a Tessera limit, not a fault of your account."
            }
        }
    }
}

/// How far a world entry got, for the log lines: flags and counts, no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryProgress {
    /// The referral arrived.
    pub referred: bool,
    /// The world connection reached State 5.
    pub world_connected: bool,
    /// A complete valid `0x90` arrived.
    pub session_started: bool,
    /// `0x4C` batches whose framing held, and the sub-events in them.
    pub batches: u64,
    /// Sub-events in those batches.
    pub sub_events: u64,
    /// `0x98` and `0x52`, top-level or in a batch.
    pub creations: u64,
    /// `0xCE`, top-level or in a batch.
    pub positions: u64,
    /// `0xCC`.
    pub placements: u64,
    /// `0x5E`.
    pub portal_spaces: u64,
    /// `0x7C` or `0xC7`: seen, not acted on (OPEN-WORLD-9).
    pub readiness_seen: u64,
    /// `0x0D`: counted, no current path (OPEN-WORLD-9).
    pub unsupported_batches: u64,
    /// Other messages of the world connection before `WorldEntered`.
    pub other: u64,
    /// Messages of a checked opcode that did not decode: not populated, not dispatched.
    pub unread: u64,
}

/// A row of `official-session.md` §9.3.8's phase table. What it means before and after `Connected`
/// is [`SessionDriver::end_by_table`]'s.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRow {
    /// A local limit: preflight, codec, unbuildable body or destination, schema or variant, with
    /// Tessera's text.
    Unsupported(String),
    /// A stage deadline or a verified gap's deadline, with Tessera's text.
    Deadline(String),
    /// A valid `0xB3`, a server Disconnect, a listed refusal code, a fatal error, `0x12D` status 0.
    ServerRefused,
    /// `TransportError` code 0 or an unlisted code, or a negative `0x130`.
    ServerOther,
    /// Ordinary silence.
    Silence,
    /// An I/O failure.
    Io,
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
        /// What it waits for now.
        wait: EntryWait,
        /// When that wait began.
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

/// What the driver needs to start. Its `Debug` names no endpoint (decision 0044 item 3).
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
    /// The dictionaries' directional UUIDs (`official-session.md` §7.3).
    pub codec_uuids: CodecUuids,
    /// The install's tables.
    pub schema: SchemaView,
    /// The attempt's cancel flag (the connector's): the ticket refresh thread looks at it
    /// right before its request.
    pub cancel: Arc<AtomicBool>,
    /// The `[Net]` keys of `UserPreferences.ini` (`official-session.md` §3.2 item 1): the
    /// bind address, the port case and the `maxRecvRate` of the login and world
    /// connections.
    pub net: NetPreferences,
    /// Which `<loginservers>` entry [`Self::login_peer`] is (`official-login.md` §3.3;
    /// [`super::diag`] "The login server switch"), for the datagram dump.
    pub login_server_entry: LoginServerEntry,
    /// The datagram dump's directory ([`super::dump`]); `None`: off (the default).
    pub dump: Option<DumpDir>,
}

impl std::fmt::Debug for SessionStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionStart")
            .field("handoff", &self.handoff)
            .field("login_peer", &"<set>")
            .field("refresher", &self.refresher)
            .field("chat_endpoint", &self.chat_endpoint.map(|_| "<set>"))
            .field("version", &self.version)
            .field("codec_uuids", &"<set>")
            .field("net", &self.net)
            .field("login_server_entry", &self.login_server_entry)
            .field("dump", &self.dump.is_some())
            .finish_non_exhaustive()
    }
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
    /// The install cannot answer the initial exchange (`official-session.md` §9.3.7's preflight):
    /// nothing was sent, so the ticket is not spent.
    Unsupported(UnsupportedNegotiation),
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
    /// `WorldLoaded` signals outside the world, ignored.
    pub stray_loaded: u64,
    /// `VersionStatus` values other than 0 and 1 (`official-session.md` §9.3), ignored.
    pub unknown_version_status: u64,
    /// Envelopes whose routing header named a `routingField0` other than the fragment's
    /// `recipientId`; they keep `recipientId` (module docs "Entities").
    pub routed_envelopes: u64,
    /// Fellowship lines sent on the world server because the chat server had no
    /// fellowship room or was not ready (`official-messages-play.md` §6.1.3).
    pub fellowship_via_world: u64,
    /// Portal spaces begun by the server's `0x5E` (`official-messages-play.md` §4.5 item 2).
    pub portal_spaces: u64,
    /// Play-layer events dropped over [`MAX_MESSAGE_PLAY_EVENTS`] in one message.
    pub message_events_dropped: u64,
    /// Texts not looked up because their message had reached [`MAX_MESSAGE_PLAY_EVENTS`]
    /// (the play layer may count them as unresolved too).
    pub texts_skipped: u64,
    /// Results that ended the login ([`UnsupportedNegotiation`]): the waits' and the others'.
    pub unsupported_results: u64,
    /// Selections not sent: a re-entry after a teardown, or a `0x6F` that could not be built.
    pub entries_not_sent: u64,
    /// Queue-2 blobs whose ordering word repeated or went back (§9.3.8: counted, not waited for).
    pub queue2_repeats: u64,
    /// Queue-2 blobs whose ordering word skipped ahead (as [`Self::queue2_repeats`]).
    pub queue2_gaps: u64,
    /// What the connections' reassemblers ignored, discarded or found odd (§9.3.8), as of
    /// [`SessionDriver::counters`].
    pub reassembly: ReassemblyCounts,
    /// Valid `0x6D` at the idle list (§9.3.8: counted, no event).
    pub idle_refusals: u64,
    /// Malformed `0x12E`, `0x12D` or `0x130` (§9.3.8: counted and discarded).
    pub malformed_control: u64,
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
    /// The session's one socket; armed around a connection's flush while its dump waits
    /// for its first datagram ([`Self::flush`]).
    transport: Tap<N::Udp>,
    codecs: ChatCodecs,
    codec_uuids: CodecUuids,
    login: Link,
    world: Option<Link>,
    old: Vec<Link>,
    version: VersionInfo,
    /// How far the initial exchange got (`official-session.md` §9.3, [`crate::initial`]).
    initial: Negotiation,
    /// The advertisement built before the first datagram; taken once, when `0xEC` goes out
    /// (one per primary login connection, decision 0046 item 2).
    advertisement: Option<PreparedAdvertisement>,
    schema: SchemaView,
    stage: Stage,
    characters: Vec<(u64, String)>,
    /// [`ConnectorNet::readiness_specified`]: `false` on every production path.
    readiness_specified: bool,
    /// [`ConnectorNet::chat_codec_specified`]: `false` on every production path.
    chat_codec_specified: bool,
    /// [`ConnectorNet::re_entry_allowed`]: `false` on every production path.
    re_entry_allowed: bool,
    /// The session has been in the world and left it: a re-entry is not supported
    /// (OPEN-SESSION-16).
    returned_from_world: bool,
    /// How far the world entry got (§9.3.8), for its log lines.
    entry_progress: EntryProgress,
    /// A referral arrived, so the login connection was disconnected gracefully
    /// (`official-session.md` §6.5 step 2): its close no longer ends the session.
    login_referred: bool,
    /// The client's latest [`Command::WorldLoaded`] in this world: `Some(stamp)` with the
    /// entity stamp of the placement it answers (`None` inside while no stamp is known).
    /// It counts for a portal space whose expected stamp that placement reached
    /// ([`Self::loading_done`]).
    loaded: Option<Option<u8>>,
    /// When the current portal space was first seen (for [`PORTAL_LOADING_TIMEOUT_SEC`]).
    portal_since: Option<f64>,
    adapter: PlayAdapter,
    feed: EntityFeed,
    /// Play-layer events the current top-level world message made.
    message_events: usize,
    chat: Option<ChatClient<Metered<N::Udp>>>,
    /// The chat client's user-room generation the play layer's user channel slots were
    /// last bound to ([`Self::bind_user_rooms`]); `None`: not bound to this chat client.
    user_rooms_bound: Option<u64>,
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
    bind: BindCase,
    local_source: LocalSource,
    /// The first local IPv4 address (the Case B seed, §3.2), for the chat transport too.
    first_local: Option<std::net::Ipv4Addr>,
    /// The `maxRecvRate` of the login and world connections (§6.4).
    max_recv_rate: u32,
    /// The login connection's `netVersion` (for its start line).
    net_version: String,
    /// The datagram dump's directory ([`super::dump`]); `None`: off.
    dump: Option<DumpDir>,
    /// Synthetic tests only: dictionary UUIDs for the chat connection, so a test reaches
    /// the chat protocol above the handshake (live runs have none, OPEN-SESSION-1).
    #[cfg(test)]
    chat_codec_uuids: Option<CodecUuids>,
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
        // §9.3.7's preflight comes before the ticket: its failure spends nothing.
        let advertisement =
            prepare_advertisement(start.schema.0.as_ref()).map_err(StartFailure::Unsupported)?;
        // §3.2 item 2: the wildcard address unless the two-colon `-h` form's `localHost`
        // (which overrides) or `Net.BindInterface` selects one. A `localHost` segment that
        // is given still overrides `Net.BindInterface` when it is not an IPv4 literal (or
        // is `0.0.0.0`): it selects the wildcard (engineering choice, no name lookup).
        let host_segment = host
            .local_host
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty());
        let from_host: Option<std::net::Ipv4Addr> = host_segment
            .and_then(|h| h.parse().ok())
            .filter(|ip: &std::net::Ipv4Addr| !ip.is_unspecified());
        let (address, local_source) = match (host_segment, from_host, start.net.bind_address()) {
            (_, Some(ip), _) => (ip, LocalSource::HostSegment),
            (Some(_), None, _) | (None, None, None) => {
                (std::net::Ipv4Addr::UNSPECIFIED, LocalSource::Wildcard)
            }
            (None, None, Some(ip)) => (ip, LocalSource::BindInterface),
        };
        // The first local IPv4 address seeds Case B (`NetworkAccess::local_ipv4_toward`
        // says which address stands for it).
        let first_local = net.local_ipv4_toward(peer);
        if stop() {
            return Err(StartFailure::Cancelled);
        }
        // §3.2 item 3: a non-zero `--outport` overrides `Net.UserSpecifiedPort`.
        let explicit_port = match start.handoff.local_bind_port {
            0 => start.net.user_specified_port,
            port => port,
        };
        let ports = BindPorts::select(explicit_port, start.net.compute_unique_port, first_local);
        let transport = net
            .bind_udp(BindPlan { address, ports })
            .map_err(|_| StartFailure::Bind)?;
        let bind = BindCase::of(&ports);
        let max_recv_rate = max_recv_rate_for_speed(start.net.connection_speed_kbps);
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
        let net_version = auth.net_version.clone();
        let dump = start.dump.as_ref().map(|_| Endpoint {
            kind: DumpKind::Login,
            target: Target::LoginServer(start.login_server_entry),
            remote_form: remote.0.label(),
            remote_name: remote.1.label(),
            local_source: local_source.label(),
            bind: bind.label(),
        });
        let mut login = Link::new(
            peer,
            start.codec_uuids,
            max_recv_rate,
            mode,
            remote,
            dump,
            now,
        )
        .map_err(|_| StartFailure::Bind)?;
        login.initial = Some(InitialStages::default());
        if let Some(c) = login.mux.connection_mut(login.conn) {
            c.start_login(auth, now).map_err(|_| StartFailure::Ticket)?;
        }
        let categories = chat_categories(start.schema.0.as_ref());
        let mut driver = Self {
            transport: Tap::new(transport),
            codecs: start.codecs,
            codec_uuids: start.codec_uuids,
            login,
            world: None,
            old: Vec::new(),
            version: start.version,
            initial: Negotiation::new(),
            advertisement: Some(advertisement),
            schema: start.schema,
            stage: Stage::Login,
            characters: Vec::new(),
            readiness_specified: net.readiness_specified(),
            chat_codec_specified: net.chat_codec_specified(),
            re_entry_allowed: net.re_entry_allowed(),
            returned_from_world: false,
            entry_progress: EntryProgress::default(),
            login_referred: false,
            loaded: None,
            portal_since: None,
            adapter: PlayAdapter::new(categories),
            feed: EntityFeed::default(),
            message_events: 0,
            chat: None,
            user_rooms_bound: None,
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
            bind,
            local_source,
            first_local,
            max_recv_rate,
            net_version,
            dump: start.dump,
            #[cfg(test)]
            chat_codec_uuids: None,
        };
        if stop() {
            return Err(StartFailure::Cancelled);
        }
        driver
            .report(&driver.login, false, now)
            .log_start(Some(&driver.net_version));
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
        let mut counters = self.counters;
        let links = std::iter::once(&self.login)
            .chain(self.world.iter())
            .chain(self.old.iter());
        for l in links {
            if let Some(c) = l.mux.connection(l.conn) {
                let r = c.reassembly_counts();
                let sum = &mut counters.reassembly;
                sum.duplicates = sum.duplicates.saturating_add(r.duplicates);
                sum.queue3_older = sum.queue3_older.saturating_add(r.queue3_older);
                sum.queue3_equal = sum.queue3_equal.saturating_add(r.queue3_equal);
                sum.queue3_ambiguous = sum.queue3_ambiguous.saturating_add(r.queue3_ambiguous);
                sum.queue3_groups_over =
                    sum.queue3_groups_over.saturating_add(r.queue3_groups_over);
            }
        }
        counters
    }

    /// How far the world entry got (§9.3.8).
    #[must_use]
    pub const fn entry_progress(&self) -> EntryProgress {
        self.entry_progress
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

    /// The local recipient IDs of the login and world connections, `0` before a challenge
    /// assigned one (tests).
    #[must_use]
    pub fn connection_ids(&self) -> (u16, Option<u16>) {
        (
            self.login.rec_id().unwrap_or(0),
            self.world.as_ref().map(|w| w.rec_id().unwrap_or(0)),
        )
    }

    /// The first local IPv4 address (`official-session.md` §3.2 item 2), the Case B seed of
    /// the chat transport's socket too ([`ConnectorNet::chat_binder`]).
    #[must_use]
    pub fn first_local_ipv4(&self) -> Option<std::net::Ipv4Addr> {
        self.first_local
    }

    /// How far the initial exchange got (`official-session.md` §9.3, [`crate::initial`]).
    #[must_use]
    pub fn initial_stages(&self) -> InitialStages {
        self.initial.stages()
    }

    /// The latest valid `0xDD`'s opaque word (tests: it is kept and never compared).
    #[must_use]
    pub fn initial_server_config(&self) -> Option<u32> {
        self.initial.server_config()
    }

    /// The skipped messages by opcode (§9.3.7).
    #[must_use]
    pub fn skip_counters(&self) -> &crate::initial::SkipCounters {
        self.initial.skip_counters()
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
            message_events_dropped = self.counters.message_events_dropped,
            texts_skipped = self.counters.texts_skipped,
            "official world: entity feed counters"
        );
    }

    /// Ends the session by one row of §9.3.8's phase table: `LoginRefused` and `Disconnected`
    /// before `Connected`, the row's `Disconnected` after it. `goodbye` sends one standalone
    /// Disconnect on each established connection (a local end); either way every connection
    /// closes at once, with no flush wait, and the ticket is not sent again. A second call does
    /// nothing.
    fn end_by_table(&mut self, row: TableRow, goodbye: bool, why: &str, now: f64) {
        if self.ended {
            return;
        }
        self.counters.unsupported_results = self.counters.unsupported_results.saturating_add(1);
        self.sync_initial();
        tracing::info!(reason = why, "official session: ended");
        if self.stage == Stage::Login && !self.login.in_handshake() {
            self.report(&self.login, false, now).log_login_ended(why);
        }
        self.log_entry_end(why);
        if goodbye {
            self.say_goodbye(why, now);
        } else {
            for l in self.links_mut() {
                if l.live() {
                    l.close_dump(why);
                    l.close_quietly();
                }
            }
        }
        if self.stage == Stage::Login {
            let (reason, note, ended) = match row {
                TableRow::Unsupported(text) => (
                    LoginRefusal::UnsupportedStep,
                    Some(text.clone()),
                    DisconnectReason::TransportFailed(text),
                ),
                TableRow::Deadline(text) => (
                    LoginRefusal::UnsupportedStep,
                    Some(text),
                    DisconnectReason::TimedOut,
                ),
                TableRow::ServerRefused => (
                    LoginRefusal::ClientNotAccepted,
                    None,
                    DisconnectReason::ServerClosed(String::new()),
                ),
                TableRow::ServerOther => (
                    LoginRefusal::Other,
                    None,
                    DisconnectReason::ServerClosed(String::new()),
                ),
                TableRow::Silence => (
                    LoginRefusal::ServiceUnavailable,
                    None,
                    DisconnectReason::TimedOut,
                ),
                TableRow::Io => (
                    LoginRefusal::ServiceUnavailable,
                    None,
                    DisconnectReason::TransportFailed(String::new()),
                ),
            };
            self.events
                .push_back(Event::Account(AccountEvent::LoginRefused {
                    reason,
                    retry_after_seconds: None,
                    detail: None,
                    note,
                }));
            self.end(ended);
        } else {
            self.end(match row {
                TableRow::Unsupported(text) => DisconnectReason::TransportFailed(text),
                TableRow::Deadline(text) => DisconnectReason::TimedOutLocally(text),
                TableRow::ServerRefused | TableRow::ServerOther => {
                    DisconnectReason::ServerClosed(String::new())
                }
                TableRow::Silence => DisconnectReason::TimedOut,
                TableRow::Io => DisconnectReason::TransportFailed(String::new()),
            });
        }
    }

    /// The line of a world entry that ends: how far it got, in counts (§9.3.8).
    fn log_entry_end(&self, why: &str) {
        if let Stage::Entering { wait, .. } = &self.stage {
            let world = self
                .world
                .as_ref()
                .and_then(|l| l.mux.connection(l.conn))
                .map(|c| c.handshake_counts().refused_while_connected)
                .unwrap_or(0);
            tracing::info!(
                stage = wait.label(),
                reason = why,
                progress = ?self.entry_progress,
                refused_world_datagrams = world,
                "official world entry: stopped"
            );
        }
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
        self.feed.set_clock(now);
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
        self.initial_deadline(now);
        self.advance_paths();
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

    /// A wait of the pending selection ended (§9.3.8): the row's `Disconnected`, with a local text
    /// naming the stage and no Account event.
    fn enter_deadline(&mut self, now: f64) {
        let Stage::Entering { since, wait, .. } = &self.stage else {
            return;
        };
        if now - since < ENTER_WORLD_TIMEOUT_SEC {
            return;
        }
        let wait = *wait;
        self.end_by_table(
            TableRow::Deadline(wait.deadline_text().to_owned()),
            true,
            wait.label(),
            now,
        );
    }

    /// A valid `0x6D` while selecting: `EnterWorldFailed` and back to the list (§9.3.8). After a
    /// referral no connection is left for a later `0x6F`, so the session ends too (engineering
    /// choice, spec silent: `docs/m8-remediation.md` §15).
    fn fail_entry(&mut self, reason: &str, ended: DisconnectReason, now: f64) {
        let Stage::Entering { name, .. } = &self.stage else {
            return;
        };
        let character = name.clone();
        if let Some(mut w) = self.world.take() {
            w.end_session(now);
            self.keep_old(w);
        }
        self.entry_messages.clear();
        self.entry_bytes = 0;
        self.stage = Stage::Characters;
        self.events.push_back(Event::EnterWorldFailed {
            character,
            reason: reason.to_owned(),
        });
        if self.login_referred {
            self.end(ended);
        }
    }

    /// The initial exchange's wait ended ([`Negotiation`], §9.3.8): the attempt ends with that
    /// wait's own result. Stage changes emit their own progress events; nothing timer-only is
    /// sent to keep the client's watchdog quiet, which three 30-second stages cannot pass.
    fn initial_deadline(&mut self, now: f64) {
        if self.stage != Stage::Login || self.ended {
            return;
        }
        if let Some(why) = self.initial.expired(now) {
            self.refuse_unsupported(why, now);
        }
    }

    /// One [`UnsupportedNegotiation`] ends the attempt (§9.3.8): its deadline or unsupported row.
    fn refuse_unsupported(&mut self, why: UnsupportedNegotiation, now: f64) {
        let text = why.detail().to_owned();
        let row = if why.is_deadline() {
            TableRow::Deadline(text)
        } else {
            TableRow::Unsupported(text)
        };
        // The Display form names the kind (never a value), so the log tells the checks apart.
        let reason = why.to_string();
        self.end_by_table(row, true, &reason, now);
    }

    /// One standalone Disconnect on each established connection (none on a connection still in
    /// its handshake), and every connection closes locally: no flush wait, no ticket resent
    /// (§9.3.7).
    fn say_goodbye(&mut self, why: &str, now: f64) {
        for l in self.links_mut() {
            if l.in_handshake() {
                // Its queue may hold the ticket.
                l.close_dump(why);
                l.close_quietly();
            } else if let Some(c) = l.mux.connection_mut(l.conn) {
                let _ = c.queue_standalone_disconnect();
            }
        }
        self.flush(now);
        // A connection the session layer closed already keeps its counts for the report.
        for l in self.links_mut() {
            if l.live() {
                l.close_dump(why);
                l.close_quietly();
            }
        }
    }

    /// Copies the initial exchange's stages to the login connection, for its dump.
    fn sync_initial(&mut self) {
        self.login.initial = Some(self.initial.stages());
    }

    /// The stages after a step: synced, one log line and a progress event.
    fn initial_step(&mut self, what: &str, now: f64) {
        self.sync_initial();
        self.report(&self.login, false, now).log_initial(what);
        self.events
            .push_back(Event::Account(AccountEvent::LoginProgress {
                stage: LoginStage::JoiningWorld,
            }));
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
        let done = self.loading_done();
        if let Some(ack) = self.adapter.portal_loading(done, timed_out) {
            if !done {
                tracing::info!("official session: portal space ended by its time-out");
            }
            self.portal_since = None;
            self.pending_out.push_back(ack);
        }
    }

    /// Whether the client's loading is done for the placement the current portal space
    /// waits for (§4.5 item 4): its latest `WorldLoaded` answers a placement whose stamp
    /// reached the expected stamp, so a `WorldLoaded` sent before a `0x5E` with that stamp
    /// (item 2 lets the placement come first) counts, and one of an earlier placement does
    /// not. While no stamp is known (no stamped placement in this world yet) any
    /// `WorldLoaded` of this world counts; the stamp wait itself is the adapter's.
    /// Engineering choice: the spec ties loading to the placement, not to a message
    /// (`docs/m8-remediation.md` §15).
    fn loading_done(&self) -> bool {
        let t = self.adapter.teleport();
        match (self.loaded, t.stamp_known()) {
            (Some(Some(s)), true) => {
                crate::play::movement::stamp_newer_or_equal(s, t.expected_stamp())
            }
            (Some(None), false) => true,
            _ => false,
        }
    }

    /// Starts the loading state afresh: at world entry (`official-messages-world.md`
    /// §4.3.3), so no `WorldLoaded` of another world counts, and when the world is left.
    fn fresh_portal_loading(&mut self) {
        self.loaded = None;
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
        self.returned_from_world = true;
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
        // Any adopted non-zero ID (`official-session.md` §6.3: the earlier range check is
        // superseded).
        if let Some(id) = rec_id.filter(|id| *id != 0) {
            // Step 2: the connection with that ID, from its own server's address.
            if let Some((i, _)) =
                candidates().find(|(_, l)| l.rec_id() == Some(id) && l.peer == src)
            {
                return Route::Link(i);
            }
            let named = candidates().any(|(_, l)| l.rec_id() == Some(id));
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
            initial: if world {
                InitialStages::default()
            } else {
                self.initial.stages()
            },
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

    /// The line of a login that ends before its character list for a reason of no refusal of the
    /// initial exchange (a cancel, the client's watchdog, a revoked network right, leaving the
    /// queue, or the server's own end of it), past its handshake: the handshake's own line covers
    /// the rest. An attempt that already ended has logged its line.
    fn log_login_end(&mut self, reason: &str, now: f64) {
        if self.ended || self.stage != Stage::Login || self.login.in_handshake() {
            return;
        }
        self.sync_initial();
        self.report(&self.login, false, now).log_login_ended(reason);
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
                        tracing::debug!(
                            kind = %error_kind(&e),
                            skipped,
                            "official session: datagram refused"
                        );
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
                // Once the attempt is cancelled the login answers nothing (`step`).
                if self.stage == Stage::Login && stop() {
                    self.initial.ignored();
                } else {
                    self.message(&m, now);
                }
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
                if world
                    && let Stage::Entering { wait, since, .. } = &mut self.stage
                    && *wait == EntryWait::WorldConnection
                {
                    *wait = EntryWait::SessionStart;
                    *since = now;
                    self.entry_progress.world_connected = true;
                    tracing::info!(stage = wait.label(), "official world entry: next stage");
                }
                if self.stage == Stage::Login {
                    if !world {
                        self.initial.connected(now);
                    }
                    // The progress screen's watchdog (`official-entry.md` §8.3) stays
                    // alive while the version exchange and the character list follow.
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginProgress {
                            stage: LoginStage::JoiningWorld,
                        }));
                }
            }
            SessionEvent::BlobReceived(blob) => {
                if blob.queue_id == QUEUE_RELIABLE_ORDERED {
                    self.note_queue2(world, blob.order_num);
                }
                self.initial_container(&blob, world, stop);
                match dispatch_message(
                    blob.blob_header_word,
                    &blob.payload,
                    DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
                    DEFAULT_MAX_CONTAINER_DEPTH,
                ) {
                    Ok(envelopes) => {
                        // Handled by `run_links` within the cap (module docs). The target is
                        // the fragment's `recipientId`: §1.1.2 names `routingField0` the
                        // destination InstanceID, but it is a `u32` and InstanceIDs are 64-bit
                        // (module docs "Entities"), so a routed envelope is counted and keeps
                        // `recipientId`, and no `0xD0` asks for an ID made from 32 bits.
                        let routed = envelopes
                            .iter()
                            .filter(|env| {
                                active_target(blob.recipient_id, env.routing_header.as_ref())
                                    != blob.recipient_id
                            })
                            .count();
                        if routed > 0 {
                            self.counters.routed_envelopes += routed as u64;
                            tracing::debug!(
                                routed,
                                "official session: routed envelope kept on its recipient"
                            );
                        }
                        self.inbox.extend(envelopes.into_iter().map(|env| Inbound {
                            top_level: env.opcode == blob.blob_header_word,
                            opcode: env.opcode,
                            recipient: blob.recipient_id,
                            envelope_flags: env.envelope_flags,
                            body: env.body,
                            world,
                            queue: blob.queue_id,
                        }));
                    }
                    Err(e) => {
                        self.counters.decode_errors += 1;
                        tracing::debug!(kind = %error_kind(&e), "official session: message refused");
                        if !stop() {
                            self.envelope_refused(&blob, world, now);
                        }
                    }
                }
            }
            SessionEvent::WorldReferralReceived { cookie, world_addr } => {
                if stop() {
                    // No world login after a cancel.
                    self.counters.cancelled_referrals += 1;
                    tracing::debug!("official session: referral after the cancel ignored");
                } else {
                    self.referral(cookie, world_addr, now);
                }
            }
            SessionEvent::AuthServerAddress(_) => {
                // §6.6 names no client action that this session needs.
                tracing::debug!("official session: redirect header ignored");
            }
            SessionEvent::Disconnected { reason } => self.link_closed(reason, world, now),
        }
    }

    /// §9.3.8: a fresh queue-2 identity is delivered when it completes, and an ordering word that
    /// repeats, goes back or skips ahead is an anomaly count only. The first completed one of each
    /// connection is the comparison baseline.
    fn note_queue2(&mut self, world: bool, order: u16) {
        let link = if world {
            self.world.as_mut()
        } else {
            Some(&mut self.login)
        };
        let Some(link) = link else {
            return;
        };
        if let Some(next) = link.queue2_next {
            let ahead = order.wrapping_sub(next);
            if ahead >= 0x8000 {
                self.counters.queue2_repeats = self.counters.queue2_repeats.saturating_add(1);
                return;
            }
            if ahead != 0 {
                self.counters.queue2_gaps = self.counters.queue2_gaps.saturating_add(1);
            }
        }
        link.queue2_next = Some(order.wrapping_add(1));
    }

    fn link_closed(&mut self, reason: LinkEnd, world: bool, now: f64) {
        tracing::info!(
            world,
            reason = reason.label(),
            "official session: connection closed"
        );
        if !world {
            self.sync_initial();
        }
        if let Some(l) = self.link_of(world).filter(|l| l.closed_in_handshake()) {
            self.report(l, world, now).log_failed(&reason);
        }
        let l = if world {
            self.world.as_mut()
        } else {
            Some(&mut self.login)
        };
        if let Some(l) = l {
            l.close_dump(&format!("{reason:?}"));
        }
        // After the referral the login connection is not needed any more: it was
        // disconnected on the referral (`official-session.md` §6.5 step 2).
        if !world && (self.world.is_some() || self.login_referred) {
            return;
        }
        let (row, goodbye) = match reason {
            LinkEnd::LocalDisconnect => {
                self.end(DisconnectReason::ClientRequested);
                return;
            }
            LinkEnd::HandshakeTimeout | LinkEnd::SilenceTimeout => (TableRow::Silence, false),
            LinkEnd::ServerDisconnect => (TableRow::ServerRefused, false),
            LinkEnd::TransportError {
                fatal, error_code, ..
            } => {
                if fatal || LOGIN_REFUSAL_CODES.contains(&error_code) {
                    (TableRow::ServerRefused, false)
                } else {
                    (TableRow::ServerOther, false)
                }
            }
            // Handshake-stage unsupported: nothing more is sent, and no ticket.
            LinkEnd::UnsupportedCodec => (
                TableRow::Unsupported(UnsupportedNegotiation::Dictionary.detail().to_owned()),
                false,
            ),
            // The session layer sent the one Disconnect already.
            LinkEnd::UnsupportedTraffic => (
                TableRow::Unsupported(UnsupportedNegotiation::TrafficRefused.detail().to_owned()),
                true,
            ),
            LinkEnd::GapDeadline => (
                TableRow::Deadline(UnsupportedNegotiation::GapDeadline.detail().to_owned()),
                true,
            ),
            LinkEnd::OutboundWindowOverflow => (TableRow::Io, true),
        };
        self.end_by_table(row, goodbye, reason.label(), now);
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
    /// writers). The first referral also starts the graceful disconnect of the login
    /// connection (§6.5 step 2).
    fn referral(&mut self, cookie: [u8; 8], addr: SocketAddrV4, now: f64) {
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
        let dump = self.dump.as_ref().map(|_| Endpoint {
            kind: DumpKind::World,
            target: Target::Referral,
            remote_form: form.label(),
            remote_name: RemoteName::FromReferral.label(),
            local_source: self.local_source.label(),
            bind: self.bind.label(),
        });
        match Link::new(
            addr,
            self.codec_uuids,
            self.max_recv_rate,
            AuthMode::Referral,
            (form, RemoteName::FromReferral),
            dump,
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
                // §6.5 step 2: the old login connection starts its graceful disconnect
                // (State 6, then 7) while both connections stay on the socket.
                if !self.login_referred {
                    self.login_referred = true;
                    if self.login.live() && !self.login.flushing() {
                        self.login.end_session(now);
                    }
                }
                tracing::info!("official session: referred to the world server");
                self.entry_progress.referred = true;
                // §9.3.8: the referral ends its wait and starts the next. A later referral of
                // the same entry starts nothing again, so the entry stays bounded.
                if let Stage::Entering { wait, since, .. } = &mut self.stage
                    && *wait == EntryWait::Referral
                {
                    *wait = EntryWait::WorldConnection;
                    *since = now;
                }
                if let Some(w) = self.world.as_ref() {
                    self.report(w, true, now).log_start(None);
                }
            }
            Err(e) => tracing::warn!(error = %e, "official session: referral refused"),
        }
    }

    fn message(&mut self, m: &Inbound, now: f64) {
        let (opcode, recipient, body, world) = (m.opcode, m.recipient, m.body.as_slice(), m.world);
        if (OPCODE_CLIENT_VERSION..=OPCODE_LOGIN_QUEUE_STATUS).contains(&opcode) {
            match ControlMessage::decode(opcode, body) {
                Ok(m) => self.control(m, now),
                Err(e) => {
                    // §9.3.8: malformed notice data is counted and discarded, not dispatched.
                    self.counters.decode_errors += 1;
                    self.counters.malformed_control =
                        self.counters.malformed_control.saturating_add(1);
                    tracing::debug!(
                        opcode,
                        kind = %error_kind(&e),
                        "official session: control message refused"
                    );
                }
            }
            return;
        }
        // These have their own handling, in the current one-byte form (§9.1, §9.3.8): none goes
        // through the historical base-flags test below.
        match opcode {
            OPCODE_INSTALL_REQUEST => return self.install_request(m, now),
            OPCODE_COMPLETION => return self.completion(m, now),
            CharacterListResponse::EVENT_ID => return self.character_list(m, now),
            PlayerSessionReady::EVENT_ID if self.stage != Stage::Login => {
                return self.session_ready(m, now);
            }
            CharacterOperationError::EVENT_ID => return self.character_error(m, now),
            AccountEvictionNotice::EVENT_ID => return self.eviction(m, now),
            _ => {}
        }
        if self.login_window_skips(m) {
            return;
        }
        // §9.3.8: the checked family has one shared byte, consumed once. It is put back so the
        // decoders read it as they read a batch's sub-event; a compressed or routed form is an
        // unsupported variant, not dispatched. Every other opcode has no checked boundary and is
        // only reached on the synthetic in-world path (module docs).
        let rebuilt;
        let data = if one_shared_prefix(opcode) {
            if m.envelope_flags != 0 {
                self.counters.decode_errors += 1;
                self.entry_progress.unread = self.entry_progress.unread.saturating_add(1);
                return;
            }
            rebuilt = rebuild_shared_prefix(body);
            rebuilt.as_slice()
        } else {
            body
        };
        // On every production path the entry stops here (module docs): counted, nothing published.
        if world && matches!(self.stage, Stage::Entering { .. }) && !self.readiness_specified {
            self.note_pending(opcode, data);
            return;
        }
        if data.first() != Some(&0) {
            self.counters.decode_errors += 1;
            return;
        }
        self.play(opcode, recipient, data, world);
    }

    /// Whether `m` is passed over on the login connection (§9.3.8's application dispatch scope):
    /// before `WorldEntered` everything without its own handling is bounded (reassembled already),
    /// counted per opcode, not dispatched, not answered and no decode error. That includes the
    /// server `0x30`s before a referral, never buffered for later replay.
    fn login_window_skips(&mut self, m: &Inbound) -> bool {
        if m.world || matches!(self.stage, Stage::InWorld { .. }) {
            return false;
        }
        self.initial.skipped(m.opcode, m.body.len());
        self.sync_initial();
        true
    }

    /// A container's own opcode on the login connection before the list (§9.4.2): counted like any
    /// skipped message, also when it holds nothing. Its contents are looked at as usual.
    fn initial_container(&mut self, blob: &ReassembledBlob, world: bool, stop: &dyn Fn() -> bool) {
        let opcode = blob.blob_header_word;
        if matches!(opcode, OPCODE_BATCH_CONTAINER | OPCODE_SINGLE_WRAPPER)
            && !world
            && self.stage == Stage::Login
            && !stop()
        {
            self.initial.skipped(opcode, blob.payload.len());
            self.sync_initial();
        }
    }

    /// A blob whose envelope did not decode (§9.1): when something waits on it, the session goes
    /// on as it would for an unreadable body of that message (§9.3.8).
    fn envelope_refused(&mut self, blob: &ReassembledBlob, world: bool, now: f64) {
        match blob.blob_header_word {
            PlayerSessionReady::EVENT_ID if matches!(self.stage, Stage::Entering { .. }) => {
                return self.end_by_table(
                    TableRow::Unsupported(UNSUPPORTED_SESSION_START.to_owned()),
                    true,
                    "session start unreadable",
                    now,
                );
            }
            CharacterOperationError::EVENT_ID | AccountEvictionNotice::EVENT_ID => {
                return self.refusal_unread(now);
            }
            _ => {}
        }
        if world || self.stage != Stage::Login {
            if blob.blob_header_word == CharacterListResponse::EVENT_ID && !world {
                self.refuse_list(UNSUPPORTED_LATER_LIST, now);
            }
            return;
        }
        let why = match blob.blob_header_word {
            OPCODE_INSTALL_REQUEST if blob.queue_id == QUEUE_UNRELIABLE => {
                UnsupportedNegotiation::MalformedRequest
            }
            OPCODE_INSTALL_REQUEST => return self.initial.ignored(),
            OPCODE_COMPLETION
                if blob.queue_id == QUEUE_UNRELIABLE
                    && self.initial.completion_step() != CompletionStep::Repeat =>
            {
                self.initial.completion_seen();
                UnsupportedNegotiation::UnexpectedCompletion
            }
            OPCODE_COMPLETION => return self.initial.ignored(),
            CharacterListResponse::EVENT_ID => UnsupportedNegotiation::ListEnvelope,
            _ => return,
        };
        self.refuse_unsupported(why, now);
    }

    /// A character list that cannot be read after `Connected`: §9.3.8's unsupported row.
    fn refuse_list(&mut self, text: &str, now: f64) {
        self.end_by_table(
            TableRow::Unsupported(text.to_owned()),
            true,
            "character list unreadable",
            now,
        );
    }

    /// The server's `0xDD` (§9.3.8). Only flags `00` or valid zlib-only `01` on queue 1 of the
    /// primary login connection, during the initial stage, can trigger `0xEC`, and only the first,
    /// which must have an empty array. A later well-formed one, its array empty or not, only updates
    /// the opaque word and is counted: nothing is sent, no wait starts again and the install is not
    /// looked at again. One on another queue or connection, or after `Connected`, is counted and
    /// skipped. A malformed one before the initial stage ends fails.
    fn install_request(&mut self, m: &Inbound, now: f64) {
        let on_primary =
            !m.world && m.queue == QUEUE_UNRELIABLE && m.top_level && self.stage == Stage::Login;
        if !on_primary {
            self.initial.ignored();
            return;
        }
        if m.envelope_flags > 1 {
            self.refuse_unsupported(UnsupportedNegotiation::MalformedRequest, now);
            return;
        }
        let request = match InstallRequest::from_envelope_body(&m.body) {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(kind = %error_kind(&e), "official session: install request refused");
                self.refuse_unsupported(UnsupportedNegotiation::MalformedRequest, now);
                return;
            }
        };
        if !self.initial.wants_request() {
            self.initial.later_request(request.server_config);
            return;
        }
        self.initial.request_seen(request.server_config);
        self.initial_step("install request", now);
        if let Err(why) = self.send_advertisement(&request, m.recipient, now) {
            self.refuse_unsupported(why, now);
        }
    }

    /// The checks of §9.3.7 that depend on `0xDD`, then `0xEC` on queue 1, to the recipient the
    /// valid `0xDD` named.
    fn send_advertisement(
        &mut self,
        request: &InstallRequest,
        recipient: u64,
        now: f64,
    ) -> Result<(), UnsupportedNegotiation> {
        if !request.trailing.is_empty() {
            return Err(UnsupportedNegotiation::RequestArray);
        }
        let Some(prepared) = self.advertisement.take() else {
            return Err(UnsupportedNegotiation::Advertisement);
        };
        let languages = self
            .schema
            .0
            .name_rule_languages(request.name_rule_key)
            .map_err(UnsupportedNegotiation::NameRule)?;
        check_language_allowed(&languages, prepared.language_key)?;
        // `send_sequenced_packet` allocates the FlowControl-only packet first when it has not
        // gone out (§5.5, §6.4.1), so `0xEC` follows it. §9.3.7 (P10-10F): the logical recipient
        // of the valid `0xDD`, its default form resolved to 2, always in the explicit eight-byte
        // field; flags `0x10` when fragmented, no extension word (Vector 14).
        if !self.send_retained_on_login(
            OPCODE_INSTALL_ADVERTISEMENT,
            recipient,
            &prepared.data,
            now,
        ) {
            return Err(UnsupportedNegotiation::AdvertisementNotSent);
        }
        tracing::info!(
            entries = prepared.entries,
            bytes = prepared.data.len(),
            "official session: install advertisement sent"
        );
        self.initial.advertisement_sent(now);
        self.initial_step("advertisement sent", now);
        Ok(())
    }

    /// The server's `0xF5` (§9.3.8). One on another queue or connection is counted and skipped
    /// with no state or timer change, and after `Connected` or the one reply any body is counted
    /// and ignored. Before `0xEC` it is unsupported. In the window from `0xEC`'s last fragment,
    /// exactly `00` gets one constructed `00`, with the recipient it named in the explicit field,
    /// and anything else fails. Nothing is fetched or applied.
    fn completion(&mut self, m: &Inbound, now: f64) {
        if m.world || self.stage != Stage::Login || m.queue != QUEUE_UNRELIABLE || !m.top_level {
            self.initial.ignored();
            return;
        }
        match self.initial.completion_step() {
            CompletionStep::Repeat => {
                self.initial.ignored();
                return;
            }
            CompletionStep::Unsupported => {
                self.initial.completion_seen();
                self.refuse_unsupported(UnsupportedNegotiation::UnexpectedCompletion, now);
                return;
            }
            CompletionStep::Guarded => {}
        }
        self.initial.completion_seen();
        if m.envelope_flags != 0 || !m.body.is_empty() {
            self.refuse_unsupported(UnsupportedNegotiation::UnexpectedCompletion, now);
            return;
        }
        if !self.send_retained_on_login(OPCODE_COMPLETION, m.recipient, &COMPLETION_DATA, now) {
            self.refuse_unsupported(UnsupportedNegotiation::CompletionNotSent, now);
            return;
        }
        self.initial.completion_sent(now);
        self.initial_step("completion sent", now);
    }

    /// The historical messages and the notice (§9.3, §9.3.8): not observed in either initial
    /// conversation, so these reasons are policy. A notice is shown in every stage and never ends
    /// or resets anything.
    fn control(&mut self, m: ControlMessage, now: f64) {
        match m {
            ControlMessage::VersionRequest => {
                if self.stage != Stage::Login {
                    self.counters.unhandled += 1;
                    return;
                }
                if self.initial.version_request_too_late() {
                    self.refuse_unsupported(UnsupportedNegotiation::LateVersionRequest, now);
                    return;
                }
                // §9.3: only an actual `0x12F` asks for `0x12C`, once.
                if !self.initial.version_requested(now) {
                    return;
                }
                self.initial_step("version request", now);
                let msg = ClientVersionMessage {
                    version_string: self.version.version_string(),
                };
                // Direct-payload message: `envelopeFlags` then the body (§9.2).
                let sent = msg.encode().is_ok_and(|body| {
                    let env = encode_event_envelope(&body);
                    self.send_on_login(OPCODE_CLIENT_VERSION, &env, now)
                });
                if !sent {
                    self.refuse_unsupported(UnsupportedNegotiation::AdvertisementNotSent, now);
                }
            }
            ControlMessage::VersionStatus(s) => match s.status {
                1 if self.stage == Stage::Login => {
                    self.initial.version_accepted(now);
                    self.initial_step("version accepted", now);
                }
                1 => {}
                0 => {
                    self.end_by_table(TableRow::ServerRefused, true, "version not accepted", now);
                }
                status => {
                    // §9.3 defines 0 and 1 only: another value is logged and ignored
                    // (engineering choice, `docs/m8-remediation.md` §15).
                    self.counters.unknown_version_status += 1;
                    tracing::debug!(
                        status,
                        "official session: version status not defined; ignored"
                    );
                }
            },
            ControlMessage::LoginQueueStatus(q) => match q.outcome() {
                // §9.3.8: only the fields actually there; one absolute limit, never extended.
                LogonOutcome::Queued(at) if self.stage == Stage::Login => {
                    if self.initial.queued(now) {
                        self.sync_initial();
                    }
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginQueueChanged {
                            position: (at.position > 0).then_some(at.position),
                            length: (at.total > 0).then_some(at.total),
                            wait_seconds: None,
                        }));
                }
                // §9.3.8: ready; the 30-second list wait starts and the progress screen comes back.
                LogonOutcome::Accepted if self.stage == Stage::Login => {
                    self.initial.queue_ready(now);
                    self.initial_step("queue ready", now);
                }
                // §9.3.8: `LoginRefused { Other }` and closes. The number is for the debug log.
                LogonOutcome::Refused(code) => {
                    tracing::debug!(
                        logon_result = format_args!("{code:#010x}"),
                        "official session: the service refused the logon"
                    );
                    self.end_by_table(TableRow::ServerOther, true, "logon refused", now);
                }
                LogonOutcome::Unknown(code) => {
                    self.counters.unhandled += 1;
                    tracing::debug!(
                        logon_result = format_args!("{code:#010x}"),
                        "official session: logon result with no meaning in the spec ignored"
                    );
                }
                LogonOutcome::Queued(_) | LogonOutcome::Accepted => {}
            },
            ControlMessage::SystemNotification(n) => {
                if matches!(self.stage, Stage::InWorld { .. }) {
                    self.events.push_back(Event::ServerNotice {
                        level: NoticeLevel::Info,
                        seconds: 0,
                        text: n.text,
                    });
                } else {
                    // `official-entry.md` §8.1: at most 256 bytes.
                    let text =
                        clean_display_name(&n.text, tessera_net::account::MAX_ACCOUNT_DETAIL_LEN);
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginNotice { text }));
                }
            }
            ControlMessage::ClientVersion(_) => {
                self.counters.unhandled += 1;
            }
        }
    }

    /// The character list (§9.3.8): the count straight after the one flags byte, every entry and
    /// the account table to exact exhaustion. One that cannot be read ends the attempt, with no
    /// forward scan, no partial list and no character invented; so does a compressed or routed
    /// one. An unknown appearance form (OPEN-WORLD-9) is such a result. `0x13D` and `0x19E` are not
    /// emitted (decision 0046 item 5); the client goes to selection after the list. The account
    /// table belongs to this list's character-screen state and triggers nothing.
    fn character_list(&mut self, m: &Inbound, now: f64) {
        if m.envelope_flags != 0 {
            self.counters.decode_errors += 1;
            tracing::debug!("official session: compressed or routed character list refused");
            if self.stage == Stage::Login {
                self.refuse_unsupported(UnsupportedNegotiation::ListEnvelope, now);
            } else {
                self.refuse_list(UNSUPPORTED_LATER_LIST, now);
            }
            return;
        }
        let list = match CharacterListResponse::from_envelope_body(&m.body, &self.schema) {
            Ok(l) => l,
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(kind = %error_kind(&e), "official session: character list refused");
                if self.stage == Stage::Login {
                    self.refuse_unsupported(UnsupportedNegotiation::ListDecode, now);
                } else {
                    self.refuse_list(UNSUPPORTED_LATER_LIST, now);
                }
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
            self.initial.list_received();
            self.initial_step("character list", now);
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

    /// `0x6D` (§9.3.8, Vector 15): the shared byte and the `u32` reason, to exact exhaustion. While
    /// selecting it fails the pending entry without an automatic retry, with a locally worded
    /// refusal; at the idle list it is counted with no event; before the list no selection exists,
    /// which is unsupported. One that cannot be read ends the session. The number is for the debug
    /// log only.
    fn character_error(&mut self, m: &Inbound, now: f64) {
        let code = (m.envelope_flags == 0)
            .then(|| CharacterOperationError::from_envelope_body(&m.body).ok())
            .flatten()
            .map(|e| e.error_code);
        let Some(code) = code else {
            self.counters.decode_errors += 1;
            return self.refusal_unread(now);
        };
        tracing::debug!(code, "official session: character operation refused");
        match &self.stage {
            Stage::Login => {
                self.refuse_unsupported(UnsupportedNegotiation::RefusalWithoutSelection, now);
            }
            Stage::Entering { .. } => {
                self.fail_entry(
                    ENTRY_REFUSED,
                    DisconnectReason::ServerClosed(String::new()),
                    now,
                );
            }
            Stage::Characters | Stage::InWorld { .. } => {
                self.counters.idle_refusals = self.counters.idle_refusals.saturating_add(1);
            }
        }
    }

    /// A `0x6D` or `0xB3` that cannot be read: §9.3.8's malformed row.
    fn refusal_unread(&mut self, now: f64) {
        if self.stage == Stage::Login {
            self.refuse_unsupported(UnsupportedNegotiation::MalformedRefusal, now);
        } else {
            self.end_by_table(
                TableRow::Unsupported(UNSUPPORTED_ENTRY_ANSWER.to_owned()),
                true,
                "refusal unreadable",
                now,
            );
        }
    }

    /// `0x90` (§9.3.8, Vector 15): the ID straight after the one shared flags byte, then the
    /// property table to exact exhaustion. A compressed or routed one or one that fails is the
    /// unsupported row with [`UNSUPPORTED_SESSION_START`]; the ID is never shifted by a guessed
    /// second byte. It starts the readiness wait and is not `WorldEntered` by itself. On every
    /// production path the entry then waits for what no spec gives yet (module docs).
    fn session_ready(&mut self, m: &Inbound, now: f64) {
        let decoded = if m.envelope_flags == 0 {
            PlayerSessionReady::from_envelope_body(&m.body, &self.schema).ok()
        } else {
            None
        };
        let Some(ready) = decoded else {
            self.counters.decode_errors += 1;
            tracing::debug!("official session: session start refused");
            if matches!(self.stage, Stage::Entering { .. }) {
                self.end_by_table(
                    TableRow::Unsupported(UNSUPPORTED_SESSION_START.to_owned()),
                    true,
                    "session start unreadable",
                    now,
                );
            }
            return;
        };
        let Stage::Entering {
            name, wait, since, ..
        } = &mut self.stage
        else {
            tracing::debug!("official session: session start outside world entry");
            return;
        };
        let name = name.clone();
        if !matches!(*wait, EntryWait::Readiness) {
            *wait = EntryWait::Readiness;
            *since = now;
            self.entry_progress.session_started = true;
            tracing::info!(stage = wait.label(), "official world entry: next stage");
        }
        if !self.readiness_specified {
            return;
        }
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

    /// What a message of the world connection tells about the entry (§9.3.8): counts only, and
    /// only for what decodes under its named layout. `data` has its one shared byte.
    fn note_pending(&mut self, opcode: u32, data: &[u8]) {
        let p = &mut self.entry_progress;
        match opcode {
            SubEventBatch::EVENT_ID_SUB_EVENT => match SubEventBatch::from_bytes(data, opcode) {
                Ok(batch) => {
                    p.batches = p.batches.saturating_add(1);
                    for frame in &batch.frames {
                        p.sub_events = p.sub_events.saturating_add(1);
                        if let SubEventFrame::Event { sub_event_id, .. } = frame {
                            match *sub_event_id {
                                EntityCreate::EVENT_ID | PropertyTableUpdate::EVENT_ID_ENTITY => {
                                    p.creations = p.creations.saturating_add(1);
                                }
                                0xCE => p.positions = p.positions.saturating_add(1),
                                _ => {}
                            }
                        }
                    }
                }
                Err(_) => p.unread = p.unread.saturating_add(1),
            },
            SubEventBatch::EVENT_ID_AOI_ENTER => {
                p.unsupported_batches = p.unsupported_batches.saturating_add(1);
            }
            EntityCreate::EVENT_ID | PropertyTableUpdate::EVENT_ID_ENTITY => {
                p.creations = p.creations.saturating_add(1);
            }
            0xCE => p.positions = p.positions.saturating_add(1),
            0xCC => p.placements = p.placements.saturating_add(1),
            0x5E => p.portal_spaces = p.portal_spaces.saturating_add(1),
            0x7C | 0xC7 => p.readiness_seen = p.readiness_seen.saturating_add(1),
            _ => p.other = p.other.saturating_add(1),
        }
    }

    /// A play message. While the world is being entered they are kept for
    /// [`Self::session_ready`]; in the world they go to the play layer when they come from the
    /// world connection, or from the login connection when there was no referral.
    fn play(&mut self, opcode: u32, recipient: u64, body: &[u8], world: bool) {
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
                // §9.3.8: not published before `WorldEntered`, and bounded. Only the synthetic
                // in-world path gets here (`Self::message`).
                self.note_pending(opcode, body);
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

    /// `0xB3` (§9.3.8, Vector 15): the shared byte and the `u32` reason are read; the text after
    /// them has no checked codec, so it is neither decoded nor dispatched. It ends the session in
    /// every stage: the row's refusal before `Connected`, `Disconnected { ServerClosed }` after it.
    /// One without its checked prefix is the malformed row.
    fn eviction(&mut self, m: &Inbound, now: f64) {
        let reason = (m.envelope_flags == 0)
            .then(|| AccountEvictionNotice::reason_of_envelope_body(&m.body))
            .flatten();
        let Some(reason) = reason else {
            self.counters.decode_errors += 1;
            return self.refusal_unread(now);
        };
        tracing::debug!(reason, "official session: the server ended the session");
        self.end_by_table(
            TableRow::ServerRefused,
            true,
            "the server ended the session",
            now,
        );
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

    /// The due moves of the `0x59` paths the feed follows (`official-messages-play.md`
    /// §4.6.1; [`EntityFeed::advance_paths`]), below [`DRIVER_EVENT_CAP`].
    fn advance_paths(&mut self) {
        if !self.in_world_still() || self.full() {
            return;
        }
        let mut out = FeedOutput::default();
        self.feed.advance_paths(&mut out);
        self.feed.flush(&self.schema, &mut out);
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
    /// a `0x98` (for [`paired_target`]).
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
            tracing::debug!(opcode, error = %e, "official world: entity message refused");
        }
        self.feed_output(out);
        outcome
    }

    /// A batch `0x0D` or `0x4C` (`official-messages-world.md` §5.1): its sub-events in
    /// order with the batch's target (§5.1 items 1 and 3), except that each `0x98` takes
    /// the first non-null `0x52` after it that an earlier `0x98` did not take
    /// ([`pair_initial_properties`], §5.1 item 2): that `0x52` is applied to the created
    /// entity right after its creation ([`paired_target`]) and is not dispatched again at
    /// its own place. Every `0x0D`, nested or not, ends with its tail flag (§5.1 row 4); a
    /// batch deeper than [`MAX_BATCH_DEPTH`] is not read (counted).
    fn batch(&mut self, opcode: u32, recipient: u64, body: &[u8], depth: usize) {
        if depth >= MAX_BATCH_DEPTH {
            self.feed.counters_mut().deep_batches += 1;
            return;
        }
        let batch = match SubEventBatch::from_bytes(body, opcode) {
            Ok(b) => b,
            Err(e) => {
                self.counters.decode_errors += 1;
                tracing::debug!(opcode, error = %e, "official world: batch refused");
                return;
            }
        };
        // For each `0x98`, the index of its initial property set.
        let mut partner = vec![None; batch.frames.len()];
        for (j, create) in pair_initial_properties(&batch.frames)
            .into_iter()
            .enumerate()
        {
            if let Some(slot) = create.and_then(|c| partner.get_mut(c)) {
                *slot = Some(j);
            }
        }
        let mut consumed = vec![false; batch.frames.len()];
        for (i, frame) in batch.frames.iter().enumerate() {
            if consumed.get(i).copied().unwrap_or(false) {
                continue;
            }
            let SubEventFrame::Event {
                sub_event_id,
                payload,
            } = frame
            else {
                self.feed.counters_mut().null_sub_events += 1;
                continue;
            };
            let outcome = self.world_message(*sub_event_id, recipient, payload, depth + 1);
            if !self.in_world_still() {
                return;
            }
            let Some(j) = partner.get(i).copied().flatten() else {
                continue;
            };
            if let Some(c) = consumed.get_mut(j) {
                *c = true;
            }
            let Some(SubEventFrame::Event {
                sub_event_id,
                payload,
            }) = batch.frames.get(j)
            else {
                continue;
            };
            // A `0x98` that failed to decode has no outcome: its initial properties
            // belong to an entity that was never made, not to the batch's target.
            let Some(outcome) = outcome else {
                self.feed.counters_mut().orphaned_initial_properties += 1;
                continue;
            };
            match paired_target(Some(outcome), recipient) {
                SubTarget::Skip => self.feed.counters_mut().overflow += 1,
                SubTarget::To(target) => {
                    self.world_message(*sub_event_id, target, payload, depth + 1);
                    if !self.in_world_still() {
                        return;
                    }
                }
            }
        }
    }

    /// A message of the play layer (`official-messages-play.md` §4–§8). The positions of
    /// every entity but the local player go to the feed ([`EntityFeed::place`]).
    fn play_message(&mut self, opcode: u32, recipient: u64, body: &[u8]) {
        if opcode == AccountEvictionNotice::EVENT_ID {
            // Inside a batch: its boundary there is not rechecked, and no text is dispatched
            // (`Self::eviction`).
            tracing::debug!("official session: the server ended the session");
            self.end(DisconnectReason::ServerClosed(String::new()));
            return;
        }
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
                    entity,
                    path_type,
                    speed_param,
                    waypoints,
                } => {
                    // §4.6.1: along the path at `speed_param` metres per second.
                    let mut out = FeedOutput::default();
                    self.feed
                        .path_along(entity, path_type, speed_param, &waypoints, &mut out);
                    self.feed_output(out);
                }
                PlaySignal::PortalSpace { scene_id, .. } => {
                    // §4.5 item 2: a portal space begins with both gates clear and its
                    // loading time-out starts. A `WorldLoaded` counts only for the placement
                    // it answers (`Self::loading_done`), so one of an earlier placement
                    // never ends it, and one of a placement that came before this `0x5E`
                    // still does (module docs). The scene comes with the placement the
                    // client loads; no neutral event carries the scene ID.
                    self.portal_since = None;
                    self.counters.portal_spaces += 1;
                    tracing::debug!(scene_id, "official world: portal space began");
                }
                PlaySignal::MotionController { .. } => {
                    self.feed.counters_mut().motion_controllers += 1;
                }
                PlaySignal::AnimLayers { .. } => self.feed.counters_mut().anim_layers += 1,
                _ => {}
            }
        }
    }

    /// Queues the envelope `env` of `opcode` (inner payload `payload`) on `link`, on the
    /// queue and sequence group of [`outbound_queue`].
    fn send_on(
        link: &mut Link,
        codecs: &ChatCodecs,
        opcode: u32,
        payload: &[u8],
        env: &[u8],
        now: f64,
    ) -> bool {
        let Some(c) = link.mux.connection_mut(link.conn) else {
            return false;
        };
        let (queue, group) = outbound_queue(opcode, payload);
        match c.send_blob(queue, group, opcode, env, now, codecs.outgoing.as_deref()) {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(opcode, error = %e, "official session: message not queued");
                false
            }
        }
    }

    /// Sends `0x6F` on queue 2 of the login connection to [`SELECTION_DESTINATION`], in the
    /// explicit recipient form, with no extension word (§9.3.8, Vector 12). Its body is
    /// [`CharacterEnterWorldRequest`]'s with the empty client table of §9.3.7–§9.3.8.
    fn send_entry(&mut self, env: &[u8], now: f64) -> bool {
        let opcode = CharacterEnterWorldRequest::EVENT_ID;
        let (queue, group) = outbound_queue(opcode, &[]);
        let sent = self
            .login
            .mux
            .connection_mut(self.login.conn)
            .map(|connection| {
                connection.send_blob_to(
                    queue,
                    group,
                    SELECTION_DESTINATION,
                    opcode,
                    env,
                    now,
                    self.codecs.outgoing.as_deref(),
                )
            });
        match sent {
            Some(Ok(_)) => true,
            Some(Err(e)) => {
                self.counters.send_failures += 1;
                tracing::debug!(opcode, error = %e, "official session: message not queued");
                false
            }
            None => false,
        }
    }

    /// `0xEC` and the client's `0xF5` (§9.3.7, "Queue-1 retention"): on queue 1 to `recipient`,
    /// and resendable on a NAK until acknowledged or the attempt ends.
    fn send_retained_on_login(
        &mut self,
        opcode: u32,
        recipient: u64,
        data: &[u8],
        now: f64,
    ) -> bool {
        let (queue, group) = outbound_queue(opcode, &[]);
        let sent = self
            .login
            .mux
            .connection_mut(self.login.conn)
            .map(|connection| {
                connection.send_blob_retained(
                    queue,
                    group,
                    recipient,
                    opcode,
                    data,
                    now,
                    self.codecs.outgoing.as_deref(),
                )
            });
        match sent {
            Some(Ok(_)) => true,
            Some(Err(e)) => {
                self.counters.send_failures += 1;
                tracing::debug!(opcode, error = %e, "official session: message not queued");
                false
            }
            None => false,
        }
    }

    fn send_on_login(&mut self, opcode: u32, env: &[u8], now: f64) -> bool {
        // Only `0x24`'s queue depends on its payload (`outbound_queue`); this path never
        // carries one.
        let ok = Self::send_on(&mut self.login, &self.codecs, opcode, &[], env, now);
        if !ok {
            self.counters.send_failures += 1;
        }
        ok
    }

    fn send_pending(&mut self, now: f64) {
        while let Some(ev) = self.pending_out.pop_front() {
            let (opcode, payload) = ev.into_parts();
            // §9.3.8: `0xF4` is one shared zero byte, serialized once. Everything else here has
            // no checked boundary and is only reached on the synthetic in-world path.
            let env = if opcode == OPCODE_PORTAL_READY_ACK {
                payload.clone()
            } else {
                encode_event_envelope(&payload)
            };
            let ok = match &mut self.world {
                Some(w) => Self::send_on(w, &self.codecs, opcode, &payload, &env, now),
                None => Self::send_on(&mut self.login, &self.codecs, opcode, &payload, &env, now),
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
        // The sequenced FlowControl-only packets a tick may send go out with the outgoing
        // dictionary (`official-session.md` §5.5, §6.4.1).
        let outgoing = self.codecs.outgoing.clone();
        for l in self.links_mut() {
            if let Err(e) = l.mux.tick_with_codec(now, outgoing.as_deref()) {
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
            // The dump's tap is armed only around the flush of a connection whose first
            // datagram is still to be written ([`super::dump`]).
            let tap = self.dump.is_some() && l.wants_first_dump();
            if tap {
                self.transport.arm();
            }
            if let Err(e) = l.mux.flush_outbound(&mut self.transport) {
                failed = Some(e);
            }
            if tap
                && let Some(caught) = self.transport.take()
                && let Some(dir) = self.dump.as_ref()
            {
                l.write_first_dump(dir, &caught, self.transport.local_addr().ok());
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
        self.bind_user_rooms();
    }

    /// Binds the play layer's user channel slots to the chat client's user rooms in join
    /// order (`official-messages-play.md` §6.7) when they changed since the last binding;
    /// without a chat client no slot is bound.
    fn bind_user_rooms(&mut self) {
        let Some(chat) = self.chat.as_ref() else {
            if self.user_rooms_bound.take().is_some() {
                self.adapter
                    .categories_mut()
                    .set_user_rooms(std::iter::empty());
            }
            return;
        };
        let generation = chat.user_rooms_generation();
        if self.user_rooms_bound == Some(generation) {
            return;
        }
        chat.bind_user_rooms(self.adapter.categories_mut());
        self.user_rooms_bound = Some(generation);
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
                // §9.3.8, OPEN-SESSION-16: a re-entry after a teardown has no settled queue or body.
                if self.returned_from_world && !self.re_entry_allowed {
                    self.counters.entries_not_sent =
                        self.counters.entries_not_sent.saturating_add(1);
                    self.end_by_table(
                        TableRow::Unsupported(ENTRY_NOT_SUPPORTED.to_owned()),
                        true,
                        "re-entry not supported",
                        now,
                    );
                    return Ok(());
                }
                let sent = CharacterEnterWorldRequest::new(id)
                    .to_envelope_bytes()
                    .is_ok_and(|env| self.send_entry(&env, now));
                if !sent {
                    self.counters.entries_not_sent =
                        self.counters.entries_not_sent.saturating_add(1);
                    self.end_by_table(
                        TableRow::Unsupported(ENTRY_NOT_SUPPORTED.to_owned()),
                        true,
                        "selection not sent",
                        now,
                    );
                    return Ok(());
                }
                self.entry_progress = EntryProgress::default();
                tracing::info!("official session: character selected; waiting for the referral");
                self.stage = Stage::Entering {
                    id,
                    name,
                    wait: EntryWait::Referral,
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
                // §4.5 item 4: the client's loading for its latest placement is done. It is
                // kept with that placement's stamp, also outside portal space: a `0x5E`
                // may follow its placement (item 2; module docs).
                if matches!(self.stage, Stage::InWorld { .. }) {
                    let t = self.adapter.teleport();
                    self.loaded = Some(t.stamp_known().then(|| t.stamp()));
                } else {
                    self.counters.stray_loaded += 1;
                    tracing::debug!("official session: loading signal outside the world");
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
                on_ground,
            } => {
                // `official-messages-play.md` §8 row `0x24`: the axes and both headings.
                // The jump bit while the controller is off the ground and rising is
                // Tessera's reading: the spec is silent on its source (`MoveStep::from_move`,
                // `docs/m8-remediation.md` §15).
                let step =
                    MoveStep::from_move(*heading, [velocity.x, velocity.y, velocity.z], *on_ground);
                let predicted = PredictedState {
                    location: *location,
                    heading: *heading,
                };
                let out = self.adapter.move_step(step, now, Some(predicted));
                self.pending_out.extend(out);
                Ok(())
            }
            Command::Chat {
                channel: ChatChannel::Fellowship,
                ..
            } => {
                // `official-messages-play.md` §6.1.3: Fellowship is dual. The chat server's
                // fellowship room when it is there, else the world server's `0x84EC001B`
                // with the Fellowship category (§8).
                let via_chat = match self.chat.as_mut() {
                    Some(chat) => match chat.command(cmd, now) {
                        Ok(()) => Some(Ok(())),
                        Err(
                            crate::play::PlayError::ChatNotReady
                            | crate::play::PlayError::UnmappedChannel(_),
                        ) => None,
                        Err(_) => {
                            Some(Err(NetError::InvalidCommand("chat-server command refused")))
                        }
                    },
                    None => None,
                };
                if let Some(r) = via_chat {
                    return r;
                }
                self.counters.fellowship_via_world += 1;
                let out = self
                    .adapter
                    .command(cmd)
                    .map_err(|_| NetError::InvalidCommand("chat command refused"))?;
                self.pending_out.extend(out);
                Ok(())
            }
            Command::Chat {
                channel: ChatChannel::Named { .. },
                ..
            }
            | Command::Social(
                tessera_net::SocialCommand::JoinChannel { .. }
                | tessera_net::SocialCommand::LeaveChannel { .. },
            ) => {
                let chat = self
                    .chat
                    .as_mut()
                    .ok_or(NetError::InvalidCommand(CHAT_UNAVAILABLE))?;
                let sent = chat
                    .command(cmd, now)
                    .map_err(|_| NetError::InvalidCommand("chat-server command refused"));
                self.bind_user_rooms();
                sent
            }
            Command::Chat { .. } => {
                let out = self
                    .adapter
                    .command(cmd)
                    .map_err(|_| NetError::InvalidCommand("chat command refused"))?;
                self.pending_out.extend(out);
                Ok(())
            }
            // `official-entry.md` §8.1: a new world list if the backend can, else the session's
            // end. This phase has no list to send: one standalone Disconnect, a local close and
            // nothing more, so no `0xEC` follows.
            Command::Account(AccountCommand::LeaveLoginQueue) if self.stage == Stage::Login => {
                tracing::info!("official session: the user left the login queue");
                self.log_login_end("left the queue", now);
                self.say_goodbye("left the login queue", now);
                self.end(DisconnectReason::ClientRequested);
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
        self.log_login_end("cancelled", now);
        // What the login queued in its last pass (`0xEC` among it) stays unsent: nothing more
        // goes out after a cancel but the goodbye.
        if self.stage == Stage::Login
            && let Some(connection) = self.login.mux.connection_mut(self.login.conn)
        {
            connection.discard_unsent_datagrams();
        }
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
        self.log_login_end("network right revoked", now);
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
        self.bind_user_rooms();
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

    /// Synthetic tests only: [`ConnectorNet::re_entry_allowed`], as if re-entry were specified.
    #[cfg(test)]
    pub fn forget_the_return_for_tests(&mut self) {
        self.re_entry_allowed = true;
    }

    /// Synthetic tests only: the chat connections attached from now on accept a challenge
    /// naming `uuids.incoming` (see [`ChatConfig::with_codec_uuids`]).
    #[cfg(test)]
    pub fn set_chat_codec_uuids_for_tests(&mut self, uuids: CodecUuids) {
        self.chat_codec_uuids = Some(uuids);
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
        if stop() || self.ended || !self.chat_codec_specified {
            return;
        }
        // `official-session.md` §6.2.1: `clientTimestamp` is the Unix time when the
        // session is initialised; the chat session is initialised here, not with the login
        // connection. Its reconnects keep it (one chat session; engineering choice,
        // `docs/m8-remediation.md` §15).
        let config = ChatConfig::new(
            endpoint,
            self.handoff.subscription_name(),
            Some(self.handoff.gls_ticket().to_owned()),
            unix_seconds(),
        )
        .with_first_local(self.first_local);
        // No dictionary UUIDs for chat: its dictionary source is OPEN-SESSION-1, so its
        // challenge stays an explicit unsupported result (`official-session.md` §6.3, §7.3).
        #[cfg(test)]
        let config = match self.chat_codec_uuids {
            Some(uuids) => config.with_codec_uuids(uuids),
            None => config,
        };
        let mut binder = binder;
        let allowance = Arc::clone(&self.chat_allowance);
        let dump = self
            .dump
            .clone()
            .map(|dir| (dir, self.chat_dump_endpoint()));
        let metered: crate::chat_session::client::TransportBinder<Metered<N::Udp>> =
            Box::new(move || {
                binder().map(|inner| Metered {
                    inner,
                    allowance: Arc::clone(&allowance),
                    dump: dump
                        .as_ref()
                        .map(|(dir, endpoint)| SocketDump::new(dir.clone(), *endpoint)),
                })
            });
        match ChatClient::with_binder(metered, config, self.codecs.clone(), now) {
            Ok(mut chat) => {
                chat.set_self_descriptor(player, name);
                self.chat = Some(chat);
                // A new chat client: its rooms are bound from scratch.
                self.user_rooms_bound = None;
                self.bind_user_rooms();
            }
            Err(e) => {
                self.chat_failed(now);
                tracing::debug!(error = %e, "official chat session not started");
            }
        }
    }

    /// What the chat connections' datagram dumps say about their endpoint: the
    /// `--chatserver` value's form, and the chat transport's defaults (§3.2 item 1: the
    /// wildcard address and the bind case of [`BindPlan::chat_defaults`]).
    fn chat_dump_endpoint(&self) -> Endpoint {
        let host = crate::login::LoginHost::parse(self.handoff.chat_server.trim()).ok();
        let plan = BindPlan::chat_defaults(self.first_local);
        Endpoint {
            kind: DumpKind::Chat,
            target: Target::ChatServer,
            remote_form: host.as_ref().map_or("not known", |h| {
                RemoteForm::of_host(h.local_host.is_some(), h.port.is_some()).label()
            }),
            remote_name: host
                .as_ref()
                .map_or("not known", |h| RemoteName::of_host(&h.remote_host).label()),
            local_source: LocalSource::Wildcard.label(),
            bind: BindCase::of(&plan.ports).label(),
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
            if self.stage == Stage::Login
                && let Some(t) = self.initial.deadline()
            {
                at(t);
            }
            // Portal space's loading time-out (§4.5 item 4; `portal_step` notes when it
            // began on its first pass), until the client gate is set: then the exit waits
            // for the server's `0x7C` / `0xC7` alone.
            // Only while it lies ahead: past it, the exit waits for the stamp's `0xCE` or the
            // server gate, which arrive as datagrams and wake the loop anyway.
            let portal_timeout = self.portal_since.unwrap_or(now) + PORTAL_LOADING_TIMEOUT_SEC;
            if matches!(self.stage, Stage::InWorld { .. })
                && self.adapter.teleport().in_portal_space()
                && !self.adapter.teleport().client_ready()
                && now < portal_timeout
            {
                at(portal_timeout);
            }
            if self.chat_codec_specified
                && matches!(self.stage, Stage::InWorld { .. })
                && self.chat.is_none()
                && self.chat_endpoint.is_some()
            {
                at(self.chat_retry_at);
            }
            // The next move of a followed `0x59` path (§4.6.1); none at the event cap, where
            // the pass would not advance it.
            if matches!(self.stage, Stage::InWorld { .. })
                && !self.full()
                && let Some(t) = self.feed.next_path_at()
            {
                at(t);
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
                if self.chat_codec_specified
                    && self.chat.is_none()
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
