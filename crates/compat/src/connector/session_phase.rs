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
//! `official-session.md` §9.3.1–§9.3.9 and decision 0046 ([`crate::initial`]): after State 5
//! the login connection waits for the server's `0xDD`, sends the install advertisement
//! `0xEC` built before the first datagram from the user's own archives, with the recipient of the
//! valid `0xDD` in an explicit field, answers the server's `0xF5` once, likewise, and waits for
//! the character list. Before `WorldEntered` every other message of the login connection is
//! skipped and counted per opcode: no dispatch, no reply and no decode error. An actual `0x12F`
//! switches to the historical path (§9.3.9). `0x12E` and `0xFA` are notices: shown, and they end
//! and restart nothing.
//!
//! # How a session ends
//!
//! One table decides (§9.3.9's outcome table, `TableRow`). Before `Connected`: `LoginRefused`, then
//! `Disconnected` in the same poll. After it no Account refusal goes out: the same row ends in a
//! `Disconnected` of its own. A Tessera limit ends in `Disconnected { Unsupported }` or `{
//! UnsupportedTimeout }`, which carry no text: the client has fixed words for them
//! (`official-entry.md` §8.6) and the local cause goes to the log. The one exception is a valid
//! `0x6D` while selecting, before the referral: `EnterWorldFailed` and back to the list.
//!
//! The close counts are §9.3.9's rows, and only the primary active connection gets a Disconnect: the
//! world connection once a referral made one, else the login connection. A local end, and an
//! established connection's silence, send exactly one when it is established and none in its handshake,
//! and close with no flush wait and no more ticket; a server's closing header and an I/O failure get
//! none. An I/O failure is a send of the primary connection the socket refuses for good
//! (`SessionDriver::flush`); a socket that is full just now is a count, as is a failed read, and a
//! revoked right is `SessionDriver::abandon`. A retired or retiring login connection gets none on
//! any outcome, and its failed sends decide nothing. The
//! user's Cancel, Back, Escape, window close, typed logout and "Leave queue" do the same, with
//! `Disconnected { ClientRequested }`; the one exception is a user's close while the world connection
//! is in its handshake: one on the login connection and none on the world's
//! (`SessionDriver::begin_shutdown`).
//! Required progress is only the preflight, the session's codec, the bodies of `0x12C` and `0x6F`,
//! the first `0x6C`, every `0x90` from the current world peer (an undecodable or conflicting later one
//! is Unsupported, §9.3.11) and a `0x40` whose target resolves to the `0x90`'s ID (Unsupported,
//! §9.3.9); any other complete message that fails is counted and skipped.
//!
//! # The road into the world
//!
//! `0x6F` goes on queue 2 to the fixed destination [`SELECTION_DESTINATION`], with an empty client
//! table. Separate 30-second waits follow, none reset by other traffic ([`EntryWait`]): to the
//! referral, to the world connection's State 5, to a complete valid `0x90`, and from `0x90` to the
//! sourced readiness. Each real step is an `Event::WorldEntryProgress`. Only a `0x90` of the world
//! connection during its own wait starts the last one. A later `0x90` from the world peer that does not
//! read, or names an ID of `0` or `2` or another ID, is Unsupported; one from the login or another
//! peer is counted and skipped without touching the identity.
//!
//! On the world connection each checked message, top-level or embedded in a `0x4C`, is decoded under
//! its named layout with its one shared byte and kept in [`PendingWorld`] only when that holds to
//! exact exhaustion ([`EntryProgress`] counts it); nothing is published. A child that does not read
//! is counted without losing its siblings. `0x7C`, `0x0D`, `0x5E`, `0xC7`, `0x153` and `0xA8` are
//! counted and skipped (OPEN-WORLD-9), never read as readiness.
//!
//! `Event::WorldPlacementPending` starts the client's loading behind its character screen, and the
//! client's `Command::WorldLoaded` sets the local gate. Nothing but that command sets it: no loading
//! time-out does. What makes a message that exact placement is OPEN-WORLD-9, so its one trigger,
//! `SessionDriver::exact_placement`, answers nothing on a production path: only a test's scripted
//! network ([`ConnectorNet::placement_scripted`]) reaches this road, and there the placement is a
//! `0xCE` naming the `0x90`'s ID or a `0xCC` whose explicit recipient it is, never the default
//! recipient 2. Its generation starts at 1 (§9.3.11), and `Command::WorldLoaded` must name it: a
//! stale, duplicate or early one is a local `InvalidCommand`. Later `0xCE` and `0xCC` go on being kept
//! but make no new generation; only a sourced new placement would, and none exists, so production has
//! none and only a test's scripted backend reaches another (`scripted_new_placement_for_tests`).
//!
//! `WorldEntered` needs the server gate as well, then the one `0xF4`, and then the pending state is
//! released and normal input starts. OPEN-WORLD-9 leaves the server gate and the `0xF4` target
//! without a source, so on every production path the entry stops at the readiness wait and ends with
//! `Disconnected { UnsupportedTimeout }`, whose log line names that stage and what was withheld
//! ([`super::pending_world::WITHHELD_BEFORE_F4`]). No readiness indication is made up and no `0xF4`
//! is sent. [`ConnectorNet::server_gate`] and [`ConnectorNet::readiness_specified`] are the two ways
//! a test's synthetic network reaches the code beyond that stage; [`super::net::LiveNet`] has
//! neither.
//!
//! Decision 0046 items 5 and 10 hold `0x19E`, every world `0x30` before `WorldEntered`,
//! `0x30000035`, `0x24` and `0x19A` ([`super::pending_world::WITHHELD_BEFORE_F4`]). Item 13 lifts the
//! `0x13D` hold for its restricted candidate (`official-session.md` §9.3.12): built from the same
//! preflight snapshot as `0xEC` before the first datagram, for the launch template's
//! `--resource-set` list or the built-in default ([`SessionStart::resource_sets`]), and queued once
//! on queue 1 to [`RESOURCE_REPORT_DESTINATION`] right after the first complete character list and
//! before `Connected`, so all its datagrams are earlier than any `0x6F`'s (a selection that found it
//! undecided would be held while it is decided and queued first). Its records are kept until a
//! covering cumulative Ack or the login's close, resent on a NAK with their identical saved bytes and
//! never on a timer, and they stay with the retiring login transport at the referral
//! ([`SessionConnection::send_blob_watched`], [`SessionConnection::retire`]). An omission (any
//! [`ReportOmission`]) withholds the whole report under item 5, is logged as a kind, releases a held
//! selection and never ends the login; the stage line's withheld list names it
//! ([`super::pending_world::withheld_before_f4`]). `0x24` stays held
//! in the world too until OPEN-WORLD-9 settles its body: a `Command::Move` is a local
//! `InvalidCommand` and nothing is sent, not even the historical 38-byte form
//! ([`DriverCounters::moves_held`], §9.3.9 "World send inventory").
//!
//! `0x154`, `0xDC`, `0x7A` and `0xD1` are withheld (§9.3.8): those commands fail locally. A re-entry
//! after a teardown has no settled recipient, queue or body (OPEN-SESSION-16), so it is a local
//! `InvalidCommand`, and the chat server is not opened while OPEN-SESSION-1 leaves its codec without
//! a source: the primary session does not depend on it, and a user's chat-server request fails
//! locally without disconnecting it.
//!
//! §9.3.12's held-command table has the one capability text [`HELD_COMMAND`]: a held command (a move,
//! every chat route, the chat server's room commands, a name check, creation, deletion, a re-entry, a
//! stale `WorldLoaded`) is a local `InvalidCommand` with it, and nothing is sent. Every held command
//! except the count-only `WorldLoaded` race row also gets one plain system line with it
//! (`Event::ServerNotice`, `SessionDriver::held_line`), a held move included; a stale, repeated or
//! pre-placement `WorldLoaded` is counted with no line. The table gives no rate bound for the move's
//! line (question for the writer); the driver's per-pass event cap bounds it. The rule holds in every
//! stage, so a chat line typed while entering or at the list gets the same text and line. The
//! evidence log
//! (only with its local opt-in, [`SessionStart::evidence`]) names the held opcode and its reason.
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
//! world connection, then the login connection.
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
//! the at most [`super::pending_world::MAX_PENDING_ENTITIES`] snapshots released), the events of one chat-server
//! datagram (the chat client is let read one datagram at a time, [`CHAT_DATAGRAMS_PER_STEP`]
//! at most per pass, and is not read at the cap), and the events a pass makes without a
//! datagram: a command's `EnterWorldFailed`, the entry deadline's, and the session's end
//! (`LoginRefused` with its `Disconnected`).
//!
//! # Leaving the world
//!
//! `Command::LeaveWorld` is the typed logout of §9.3.12 and `official-entry.md` §4.6, valid only in the
//! world: `ClientRequested`, exactly one Disconnect on the active world connection, a local close and
//! no drain, and no logout RPC (the historical path is held, `ARCHITECTURE.md` §19.1); the client goes
//! to the account form. While entering it is a local `InvalidCommand`.
//! A world teardown (§7.2 item 3) keeps the session: the client unloads the world on the
//! notice, so the driver goes back to character selection at once with
//! `Event::ReturnedToCharacterSelect` and the first list. Every later `0x6C` is counted and skipped,
//! in every phase, and the list and its private IDs stay as they are (§9.3.9). Entering the world
//! again after a teardown is not supported
//! (OPEN-SESSION-16): a local `InvalidCommand`, nothing is sent, and the session goes on.
//! Moves and chat lines that reach the driver
//! after the teardown (sent before the client saw the return) are refused and counted
//! ([`DriverCounters::outside_world`]), never sent; a refused move stays silent, a refused
//! chat line also gives a system notice (`Event::Notice`), so the player sees that the line
//! was not sent. That outside-world answer is only a test's in-world fixture's
//! ([`ConnectorNet::readiness_specified`]); on every production path §9.3.12's held-command rule
//! answers a move or chat line first, in every stage, with [`HELD_COMMAND`].
//! `ReturnedToCharacterSelect` is Tessera's neutral addition (§8 maps the
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
//! In the world (a test's synthetic path, module docs "The road into the world") portal space ends
//! on the client's `Command::WorldLoaded` or after [`PORTAL_LOADING_TIMEOUT_SEC`] (play §4.5 item
//! 4). The client sends one `WorldLoaded` per placement; the driver keeps the latest with the
//! entity stamp of the placement it answers, and it counts for a portal space whose expected stamp
//! that placement reached. So a `WorldLoaded` of an earlier placement never ends a portal space, and
//! one sent for a placement that came before its `0x5E` (item 2 allows that order) still does
//! (engineering choice, `docs/m8-remediation.md` §15). A signal outside the world and outside a
//! pending placement is ignored.
//!
//! # Ending the session
//!
//! [`SessionDriver::begin_shutdown`] is the user's close (§9.3.9): exactly one standalone Disconnect
//! on the established primary connection, no application flush, no 20-second drain, and each
//! connection closes locally at once. A connection still in its handshake (`SessionAuthSent`,
//! `ChallengeAcceptSent`) sends nothing more (no retransmitted `SessionAuthHeader` or
//! `ReferralAuthHeader`, so no ticket), and a challenge that arrives for it afterwards finds no
//! connection and is not answered. [`SessionDriver::abandon`] ends everything without sending, for
//! a revoked network right.
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

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, MAX_ACCOUNT_DETAIL_LEN,
};
use tessera_net::{
    CharacterSummary, ChatChannel, Command, DisconnectReason, EntityId, Event, Location,
    NoticeLevel, Transform, WorldEntryStage,
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
use crate::play::OutgoingEvent;
use crate::play::adapter::{
    PlayAdapter, PlayCounters, PlaySignal, placement_location, position_location, velocity_or_zero,
};
use crate::play::chat::TextResolver;
use crate::play::inbound::{PlayMessage, decode_play_message};
use crate::play::movement::{MoveStep, PredictedState, wire_heading};
use crate::session::{
    BindPlan, BindPorts, ClientVersionMessage, CodecUuids, ConnectionState, ControlMessage,
    DEFAULT_MAX_CONTAINER_DEPTH, DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
    DisconnectReason as LinkEnd, EVIDENCE_TARGET, KeptBlobFate, LogonOutcome, MAX_DATAGRAM_BUF,
    OPCODE_BATCH_CONTAINER, OPCODE_CLIENT_VERSION, OPCODE_LOGIN_QUEUE_STATUS,
    OPCODE_SINGLE_WRAPPER, ReassembledBlob, ReassemblyCounts, SessionConnection, SessionError,
    SessionEvent, SessionMultiplexer, UdpDatagramTransport, dispatch_message,
    max_recv_rate_for_speed, raw_length_admitted,
};
use crate::world::{
    AccountEvictionNotice, CharacterEnterWorldRequest, CharacterListResponse,
    CharacterOperationError, EncodedPropertyTable, EncodedPropertyValue, EntityCreate,
    EntityDespawn, EntityPlacement, PlayerSessionReady, PortalPositionSync, PortalReadyAck,
    PositionUpdate, PropertyRemoval, PropertyScope, PropertyTableUpdate, ServerShutdownText,
    SinglePropertyUpdate, SubEventBatch, SubEventFrame, VisualDescriptionUpdate, WireReader,
    WorldError, active_target, encode_event_envelope, one_shared_prefix, rebuild_shared_prefix,
};

use super::data::{SchemaView, chat_categories, prepare_preflight, prepare_resource_report};
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
use super::pending_world::{
    DEFAULT_RECIPIENT, DEFAULT_RECIPIENT_DESPAWN_APPLIES, EntitySnapshot, HeldUpdate, PendingWorld,
    ReportStatus, Stored, withheld_before_f4, withheld_summary,
};
use super::report_evidence::{
    ReportEvidence, SummaryPoint, ack_action_direction, ack_action_label, decision_label,
    omission_label, reason_label,
};
use crate::resource_report::ReportOmission;
pub use crate::resource_report::{OPCODE_RESOURCE_SET_REPORT, RESOURCE_REPORT_DESTINATION};

/// The login connection's `0x149` (§9.3.12): counted and skipped there; its first arrival after the
/// report is the report's expected first observable, recorded on the evidence target only.
const OPCODE_LOGIN_149: u32 = 0x149;

/// One line of the evidence log (`official-session.md` §9.3.11 "Required live-log CONTENT"), written
/// only while `$on` (the local opt-in, off by default: [`SessionStart::evidence`]).
macro_rules! evidence {
    ($on:expr, $level:ident, $($rest:tt)*) => {
        if $on {
            tracing::$level!(target: EVIDENCE_TARGET, $($rest)*);
        }
    };
}

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
/// | `0x13D` | 1 | — (§9.3.12; to [`RESOURCE_REPORT_DESTINATION`], decision 0046 item 13) |
/// | `0x6F` | 2 | — (§9.3.8; to [`SELECTION_DESTINATION`]) |
/// | `0x24` | 1, or 3 when discrete ([`movement_is_discrete`]) | `0x05` on queue 3 |
/// | `0xF4` | 1 | — (§9.3.9: observed queue; its trigger is OPEN-WORLD-9) |
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
        | OPCODE_RESOURCE_SET_REPORT
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

/// The locally worded service refusal of a selection (§9.3.9: no number, no service text).
pub const ENTRY_REFUSED: &str = "The official service refused this character's entry to the world.";

/// The one unsupported-capability text of every held command (`official-session.md` §9.3.12
/// "Winning current held-command policy"): the words of its local `InvalidCommand`, and the plain
/// system line a typed command gets (`SessionDriver::held_line`). Nothing is sent for it.
pub const HELD_COMMAND: &str = "Not supported by this version of Tessera yet";

/// The local answer to a selection after a return from the world (§9.3.9, OPEN-SESSION-16; the
/// held-command table's "world re-entry" row): [`HELD_COMMAND`].
pub const ENTRY_REENTRY_NOT_SUPPORTED: &str = HELD_COMMAND;

/// The local failure of a chat-server request (the held-command table's `JoinChannel`/`LeaveChannel`
/// and ChatSession rows): [`HELD_COMMAND`]; the primary session is not touched.
pub const CHAT_UNAVAILABLE: &str = HELD_COMMAND;

/// The local answer to a `Command::Move` while `0x24` is held (§9.3.9 "World send inventory"; the
/// held-command table's first row): [`HELD_COMMAND`].
pub const MOVE_HELD: &str = HELD_COMMAND;

/// Most characters in one character list (§9.3.9): a longer list fails whole, never cut.
pub const MAX_CHARACTERS: usize = 256;

/// Whether a decoded placement can be a location at all (`tessera_net`'s bound): the client's
/// `Session` drops the rest, so such a placement is no exact placement (§9.3.9) and it replaces
/// nothing that is kept.
fn usable_placement(at: &Location, yaw: f32) -> bool {
    at.is_valid() && yaw.is_finite()
}

/// How often the counts of a world entry after `0x90` go to the log while it waits. Thirty-second
/// waits make this at most a handful of lines per entry.
const ENTRY_COUNTS_LOG_SEC: f64 = 5.0;

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

    /// Turns on the connection's evidence log when `on` (§9.3.11 "Required live-log CONTENT": only
    /// with the local opt-in, [`SessionStart::evidence`]).
    fn evidence(&mut self, on: bool, now: f64) {
        let role = if matches!(self.auth, AuthMode::Referral) {
            "world"
        } else {
            "login"
        };
        if on && let Some(c) = self.mux.connection_mut(self.conn) {
            c.enable_evidence(role, now);
        }
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

    /// Whether the connection closed while it still waited for its server's challenge (State `1`).
    fn closed_before_challenge(&self) -> bool {
        matches!(
            self.mux.connection(self.conn).and_then(|c| c.closed_from()),
            Some(ConnectionState::SessionAuthSent)
        )
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
    /// To the connection with this index (`0` login, `1` world).
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
    /// `0xF4` (OPEN-WORLD-9).
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
}

/// What the first-of-a-kind lines after `0x90` name ([`SessionDriver::note_entry_first`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryFirst {
    Batch,
    Creation,
    Position,
    Placement,
}

impl EntryFirst {
    const fn label(self) -> &'static str {
        match self {
            Self::Batch => "batch",
            Self::Creation => "creation",
            Self::Position => "position",
            Self::Placement => "placement",
        }
    }
}

/// Which of those lines went out for this entry.
#[derive(Debug, Clone, Copy, Default)]
struct EntryFirsts {
    batch: bool,
    creation: bool,
    position: bool,
    placement: bool,
}

impl EntryFirsts {
    const fn of(&mut self, what: EntryFirst) -> &mut bool {
        match what {
            EntryFirst::Batch => &mut self.batch,
            EntryFirst::Creation => &mut self.creation,
            EntryFirst::Position => &mut self.position,
            EntryFirst::Placement => &mut self.placement,
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
    /// An exact placement of the local player arrived.
    pub placed: bool,
    /// The client said its loading of that placement is done.
    pub loaded: bool,
    /// `0x4C` batches whose framing held.
    pub batches: u64,
    /// Sub-events in those batches.
    pub sub_events: u64,
    /// `0x98` and `0x52` kept.
    pub creations: u64,
    /// `0x88` and `0x92` kept.
    pub properties: u64,
    /// `0xCE` kept.
    pub positions: u64,
    /// `0xCC` kept.
    pub placements: u64,
    /// The generation of the placement last given to the client (§9.3.11).
    pub placement_generation: u64,
    /// `0x89` kept.
    pub appearances: u64,
    /// `0x40` that left a despawn record (§9.3.9).
    pub despawns: u64,
    /// Checked messages refused by a despawn record: no resurrection.
    pub despawn_blocked: u64,
    /// New keys counted and skipped at the pending store's key cap (§9.3.9), never a terminal result.
    pub target_cap_hits: u64,
    /// Data counted and skipped at the pending store's byte cap.
    pub bytes_cap_hits: u64,
    /// `0x7C`, `0xC7`, `0x5E` or `0x153`: seen, not acted on (OPEN-WORLD-9).
    pub readiness_seen: u64,
    /// `0x0D`: counted, no current path (OPEN-WORLD-9).
    pub unsupported_batches: u64,
    /// `0xA8`: counted until its action variant is rechecked (§9.3.9).
    pub actions: u64,
    /// Checked messages whose target was the logical default 2: whose entity is OPEN-WORLD-9. They
    /// are kept under that key within the bounds and discarded at the release.
    pub default_target: u64,
    /// Checked messages for an ID nothing has announced yet, kept under that raw key.
    pub no_target: u64,
    /// Explicit `0x40`s to an unannounced nonlocal ID that deleted the pending data kept under that
    /// raw key, with no tombstone (§9.3.9, Vector 24).
    pub unannounced_cleared: u64,
    /// Other messages of the world connection before `WorldEntered`.
    pub other: u64,
    /// Checked messages, top-level or embedded, that did not decode to exact exhaustion or whose
    /// placement is no valid location: not counted above, not kept and not dispatched.
    pub unread: u64,
    /// `0x52` children paired with a `0x98` that did not decode: they belong to an entity that was
    /// never announced, not to the batch's target.
    pub orphaned_initial: u64,
}

/// A row of `official-session.md` §9.3.9's outcome table. What it means before and after `Connected`
/// is [`SessionDriver::end_by_table`]'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRow {
    /// A required step Tessera cannot take: preflight, codec, unbuildable body or destination, the
    /// first list or the first `0x90`.
    Unsupported,
    /// A stage, readiness or verified gap's deadline.
    Deadline,
    /// A valid `0xB3`, a server Disconnect, a listed refusal code, a fatal error, `0x12D` status 0.
    ServerRefused,
    /// `TransportError` code 0 or an unlisted code, or a negative `0x130`.
    ServerOther,
    /// Ordinary handshake expiry or silence.
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
    /// `0x6F` sent for the character; on the way to `WorldEntered`.
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
    /// The evidence log (`official-session.md` §9.3.11 "Required live-log CONTENT"): a local
    /// opt-in, `false` by default ([`super::diag::evidence_from_env`]). Off, no line goes to
    /// [`EVIDENCE_TARGET`].
    pub evidence: bool,
    /// The launch's resource-set list for decision 0046 item 13's report (`official-session.md`
    /// §9.3.12 steps 4–5): the `--resource-set` value of the expanded launch template, `Ok(None)`
    /// without one (the built-in default), or the configuration omission it gives
    /// ([`crate::resource_report::template_resource_set_option`]).
    pub resource_sets: Result<Option<String>, ReportOmission>,
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
            .field(
                "resource_sets",
                &self.resource_sets.as_ref().map(Option::is_some),
            )
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
    /// In-world updates of the `0x90`'s ID merged into its local hold for want of a validated `0x98`
    /// (§9.3.9; [`PendingWorld::hold_update`]).
    pub local_held: u64,
    /// Opcodes this driver does not handle.
    pub unhandled: u64,
    /// Outgoing messages that could not be queued.
    pub send_failures: u64,
    /// Referrals outside world entry and the world, ignored.
    pub stray_referrals: u64,
    /// Messages of the login connection after a valid referral (§9.3.9: retired, skipped).
    pub retired_messages: u64,
    /// Character lists that failed or came while selecting or in the world (§9.3.9: counted and
    /// skipped; the last valid list stays).
    pub later_lists_skipped: u64,
    /// `0x12E` and `0xFA` that could not be read, or past 256 bytes (§9.3.9: not truncated).
    pub notices_dropped: u64,
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
    /// `Command::Move`s not sent while `0x24` is held (decision 0046 item 10).
    pub moves_held: u64,
    /// `WorldLoaded` signals outside the world and without a pending placement of their generation,
    /// refused; in the world too while OPEN-WORLD-9 holds that path (ARCHITECTURE.md §19.1).
    pub stray_loaded: u64,
    /// Typed commands of §9.3.12's held-command table refused in the session with the capability
    /// text ([`HELD_COMMAND`]); nothing was sent for them.
    pub commands_held: u64,
    /// Valid `0x90`s that repeated the entry's ID: counted, the first table stays pending (§9.3.11: a
    /// later well-formed one with the same ID keeps the FIRST table, whatever its own table).
    pub session_starts_repeated: u64,
    /// `0x90`s after `WorldEntered`, readable or not: counted and skipped with no rebinding and no
    /// table replacement (§9.3.11).
    pub session_starts_after_entry: u64,
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
    /// Queue-2 blobs whose ordering word repeated or went back (§9.3.9: counted, not waited for).
    pub queue2_repeats: u64,
    /// Queue-2 blobs whose ordering word skipped ahead (as [`Self::queue2_repeats`]).
    pub queue2_gaps: u64,
    /// What the connections' reassemblers ignored, discarded or found odd (§9.3.9), as of
    /// [`SessionDriver::counters`].
    pub reassembly: ReassemblyCounts,
    /// `0x6D` with no selection pending before a referral, valid or not, and an unreadable `0xB3`
    /// after `Connected` (§9.3.9: counted, no event).
    pub refusals_skipped: u64,
    /// Malformed `0x12E`, `0x12D` or `0x130` (§9.3.9: counted and discarded).
    pub malformed_control: u64,
    /// Resource-set reports `0x13D` queued (decision 0046 item 13: at most one per login).
    pub resource_reports_sent: u64,
    /// Resource-set reports omitted under decision 0046 item 5 (at most one per login).
    pub resource_reports_omitted: u64,
}

/// Decision 0046 item 13's report in one driver (`official-session.md` §9.3.12).
#[derive(Debug)]
enum ReportState {
    /// Decided at the start, not acted on yet: the complete data, or why it is omitted.
    Pending(Result<Vec<u8>, ReportOmission>),
    /// Acted on at the first complete character list; nothing more happens in this login.
    Decided(ReportStatus),
}

impl ReportState {
    fn status(&self) -> ReportStatus {
        match self {
            Self::Pending(_) => ReportStatus::Undecided,
            Self::Decided(status) => *status,
        }
    }
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
    version: VersionInfo,
    /// How far the initial exchange got (`official-session.md` §9.3, [`crate::initial`]).
    initial: Negotiation,
    /// The advertisement built before the first datagram; taken once, when `0xEC` goes out
    /// (one per primary login connection, decision 0046 item 2).
    advertisement: Option<PreparedAdvertisement>,
    /// Decision 0046 item 13's report (`official-session.md` §9.3.12): its candidate or omission,
    /// decided once from the preflight snapshot, then whether it went out. One per driver, so a new
    /// login attempt gets a fresh decision.
    resource_report: ReportState,
    /// The report's evidence fields ([`super::report_evidence`]): its datagrams' sequences, its
    /// position against the first `0x6F`, a held selection and the first `0x149` after it. Its
    /// transport transitions are logged as they come, each one (bounded by construction in the
    /// session layer, [`SessionConnection::take_watched_transitions`]).
    report_evidence: ReportEvidence,
    /// The login transport's latest fate of the report and how many of its records it still
    /// retained, read at every [`Self::log_report_fate`] until a close released them, so a summary
    /// written after the login connection closed or is gone still has them.
    report_fate: (Option<KeptBlobFate>, usize),
    /// A close (the retiring login's or the session's) released the report's records.
    report_released: bool,
    schema: SchemaView,
    stage: Stage,
    characters: Vec<(u64, String)>,
    /// [`ConnectorNet::placement_scripted`]: `false` on every production path (OPEN-WORLD-9).
    placement_scripted: bool,
    /// The held opcodes that have had their evidence line ([`Self::held_evidence`]).
    held_logged: std::collections::BTreeSet<u32>,
    /// Whether the evidence log is on ([`SessionStart::evidence`]): the local opt-in of
    /// `official-session.md` §9.3.11, off by default. Off, no line goes to [`EVIDENCE_TARGET`].
    evidence_log: bool,
    /// The evidence log's current stage and when it started ([`Self::note_stage`]).
    evidence_stage: (&'static str, f64),
    /// When the attempt started, for the stage lines.
    evidence_start: f64,
    /// When the exact placement came and when the client said it had loaded it.
    placed_at: Option<f64>,
    loaded_at: Option<f64>,
    /// When the entry's counts last went to the log.
    entry_counts_at: f64,
    /// [`ConnectorNet::readiness_specified`]: `false` on every production path.
    readiness_specified: bool,
    /// [`ConnectorNet::chat_codec_specified`]: `false` on every production path.
    chat_codec_specified: bool,
    /// [`ConnectorNet::re_entry_allowed`]: `false` on every production path.
    re_entry_allowed: bool,
    /// [`ConnectorNet::server_gate`]: `None` on every production path.
    server_gate: Option<u32>,
    /// The session has been in the world and left it: a re-entry is not supported
    /// (OPEN-SESSION-16).
    returned_from_world: bool,
    /// How far the world entry got (§9.3.9), for its log lines.
    entry_progress: EntryProgress,
    /// Which first-of-a-kind lines of this entry went out ([`Self::note_entry_first`]).
    entry_firsts: EntryFirsts,
    /// What the world connection sent before `WorldEntered` (§9.3.9).
    pending: PendingWorld,
    /// The first valid `0x90`'s ID on this entry (§9.3.11): a repeat is counted and a different ID
    /// is unsupported. No silent rebinding.
    world_identity: Option<u64>,
    /// The session properties the first `0x90`'s table gave, once released at `WorldEntered`
    /// (`official-messages-world.md` §4.2). Nothing reads them yet: the world clock of §4.4.2 is not
    /// wired to this driver.
    session_properties: BTreeMap<u32, EncodedPropertyValue>,
    /// A send of the primary connection failed for good: the next look ends the session by the I/O row.
    send_failed: bool,
    /// The exact placement of the local player last given to the client.
    placement: Option<(Location, f32)>,
    /// That placement's generation, `0` before the first (§9.3.11).
    placement_generation: u64,
    /// The client's `WorldLoaded` answered [`Self::placement`].
    local_ready: bool,
    /// The server gate of the entry (never set on a production path).
    server_ready: bool,
    /// A valid referral arrived: the login connection retires with no Disconnect
    /// (`official-session.md` §9.3.9), and its close no longer ends the session.
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
    refused_log: LogLimit,
    unknown_log: LogLimit,
    send_log: LogLimit,
    /// A complete message refused: its lines are on the evidence target and a server could send
    /// them for as long as a session stays open (`docs/DEVELOPMENT.md` "Reading a live official
    /// run").
    message_log: LogLimit,
    /// The opt-in `skipped_property_id` lines ([`Self::log_skipped_property`]): bounded like the
    /// refusal lines, with their own budget so one never starves the other.
    property_log: LogLimit,
    /// The time of the message being handled, for [`Self::message_log`].
    log_clock: f64,
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

/// A list's names as the session offers them, in server order (§9.3.9's first-screen contract), each
/// with its ID kept for the selection. Each is validated as decoded, and a bad one is never cleaned
/// into a good one: `None`, the whole list fails, for more than [`MAX_CHARACTERS`] entries, a name
/// that is empty or whitespace only, holds a control character or is longer than 64 UTF-8 bytes, or
/// a repeated ID or name, compared as decoded with no normalization. (A lone surrogate never gets
/// here: the wire reader refuses it.) No entry is dropped or cut.
fn listed_names<'a>(
    entries: impl ExactSizeIterator<Item = (u64, &'a str)>,
) -> Option<Vec<(u64, String)>> {
    if entries.len() > MAX_CHARACTERS {
        return None;
    }
    let mut names: Vec<(u64, String)> = Vec::with_capacity(entries.len());
    for (id, name) in entries {
        if !tessera_net::model::is_valid_name(name)
            || name.chars().all(char::is_whitespace)
            || names.iter().any(|(i, n)| *i == id || n == name)
        {
            return None;
        }
        names.push((id, name.to_owned()));
    }
    Some(names)
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
    if !raw_length_admitted(datagram) {
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
        let preflight =
            prepare_preflight(start.schema.0.as_ref()).map_err(StartFailure::Unsupported)?;
        // §9.3.12, decision 0046 item 13: the report's candidate from the same snapshot. An
        // omission is never a start failure: the login goes on under item 5.
        let resource_report = ReportState::Pending(prepare_resource_report(
            start.schema.0.as_ref(),
            &preflight.archives,
            start
                .resource_sets
                .as_ref()
                .map(Option::as_deref)
                .map_err(|k| *k),
        ));
        let advertisement = preflight.advertisement;
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
        login.evidence(start.evidence, now);
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
            version: start.version,
            initial: Negotiation::new(),
            advertisement: Some(advertisement),
            resource_report,
            report_evidence: ReportEvidence::default(),
            report_fate: (None, 0),
            report_released: false,
            schema: start.schema,
            stage: Stage::Login,
            characters: Vec::new(),
            placement_scripted: net.placement_scripted(),
            held_logged: std::collections::BTreeSet::new(),
            evidence_log: start.evidence,
            evidence_stage: ("handshake", now),
            evidence_start: now,
            placed_at: None,
            loaded_at: None,
            entry_counts_at: now,
            readiness_specified: net.readiness_specified(),
            chat_codec_specified: net.chat_codec_specified(),
            re_entry_allowed: net.re_entry_allowed(),
            server_gate: net.server_gate(),
            returned_from_world: false,
            entry_progress: EntryProgress::default(),
            entry_firsts: EntryFirsts::default(),
            pending: PendingWorld::default(),
            world_identity: None,
            session_properties: BTreeMap::new(),
            send_failed: false,
            placement: None,
            placement_generation: 0,
            local_ready: false,
            server_ready: false,
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
            refused_log: LogLimit::default(),
            unknown_log: LogLimit::default(),
            send_log: LogLimit::default(),
            message_log: LogLimit::default(),
            property_log: LogLimit::default(),
            log_clock: now,
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
        let links = std::iter::once(&self.login).chain(self.world.iter());
        for l in links {
            if let Some(c) = l.mux.connection(l.conn) {
                let r = c.reassembly_counts();
                let sum = &mut counters.reassembly;
                sum.duplicates = sum.duplicates.saturating_add(r.duplicates);
                sum.reused_identities = sum.reused_identities.saturating_add(r.reused_identities);
                sum.conflicts_dropped = sum.conflicts_dropped.saturating_add(r.conflicts_dropped);
                sum.tombstones_evicted =
                    sum.tombstones_evicted.saturating_add(r.tombstones_evicted);
                sum.empty_controls = sum.empty_controls.saturating_add(r.empty_controls);
                sum.queue3_older = sum.queue3_older.saturating_add(r.queue3_older);
                sum.queue3_ambiguous = sum.queue3_ambiguous.saturating_add(r.queue3_ambiguous);
                sum.queue3_groups_over =
                    sum.queue3_groups_over.saturating_add(r.queue3_groups_over);
            }
        }
        counters
    }

    /// A session property the released first `0x90` table gave (`official-messages-world.md` §4.2).
    #[must_use]
    pub fn session_property(&self, pid: u32) -> Option<&EncodedPropertyValue> {
        self.session_properties.get(&pid)
    }

    /// How far the world entry got (§9.3.9).
    #[must_use]
    pub const fn entry_progress(&self) -> EntryProgress {
        self.entry_progress
    }

    /// The entities and bytes the pending world state keeps (§9.3.9; fuzzing and tests: its
    /// bounds).
    #[must_use]
    pub fn pending_sizes(&self) -> (usize, usize) {
        (self.pending.entities(), self.pending.bytes())
    }

    /// The number of the local player's messages held after the release for want of a validated
    /// `0x98` (§9.3.9; tests).
    #[must_use]
    pub fn local_hold_len(&self) -> usize {
        self.pending.local_hold().map_or(0, |h| h.messages.len())
    }

    /// The local player's held snapshot, in release order (§9.3.9; tests).
    #[must_use]
    pub fn local_hold(&self) -> Option<EntitySnapshot> {
        self.pending.local_hold()
    }

    /// Whether a selection is refused locally: the session has been in the world and left it, and
    /// a re-entry has no settled recipient, queue or body (§9.3.9, OPEN-SESSION-16). The connector
    /// answers `InvalidCommand` for it from here on.
    #[must_use]
    pub const fn re_entry_blocked(&self) -> bool {
        self.returned_from_world && !self.re_entry_allowed
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

    /// The open sequence gaps of the login and world connections (tests).
    #[must_use]
    pub fn sequence_gaps(&self) -> (usize, usize) {
        let gaps = |l: &Link| {
            l.mux
                .connection(l.conn)
                .map_or(0, SessionConnection::missing_gap_count)
        };
        (gaps(&self.login), self.world.as_ref().map_or(0, gaps))
    }

    fn end(&mut self, reason: DisconnectReason) {
        if self.ended {
            return;
        }
        self.log_report_summary(SummaryPoint::Close);
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

    /// Ends the session by one row of §9.3.9's outcome table: `LoginRefused` and `Disconnected`
    /// before `Connected`, the row's `Disconnected` after it. `goodbye` sends one standalone
    /// Disconnect on each established connection (a local end); either way every connection
    /// closes at once, with no flush wait, and the ticket is not sent again. `why` is the local
    /// cause, for the log only (`official-entry.md` §8.6). A second call does nothing.
    fn end_by_table(&mut self, row: TableRow, goodbye: bool, why: &str, now: f64) {
        if self.ended {
            return;
        }
        self.counters.unsupported_results = self.counters.unsupported_results.saturating_add(1);
        self.log_report_summary(SummaryPoint::Close);
        self.sync_initial();
        tracing::info!(reason = why, "official session: ended");
        if self.stage == Stage::Login && !self.login.in_handshake() {
            self.report(&self.login, false, now).log_login_ended(why);
        }
        self.log_entry_end(why);
        self.pending.discard();
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
        let ended = match row {
            TableRow::Unsupported => DisconnectReason::Unsupported,
            TableRow::Deadline => DisconnectReason::UnsupportedTimeout,
            TableRow::ServerRefused | TableRow::ServerOther => {
                DisconnectReason::ServerClosed(String::new())
            }
            TableRow::Silence => DisconnectReason::TimedOut,
            TableRow::Io => DisconnectReason::TransportFailed(String::new()),
        };
        if self.stage == Stage::Login {
            let reason = match row {
                TableRow::Unsupported | TableRow::Deadline => LoginRefusal::UnsupportedStep,
                TableRow::ServerRefused => LoginRefusal::ClientNotAccepted,
                TableRow::ServerOther => LoginRefusal::Other,
                TableRow::Silence | TableRow::Io => LoginRefusal::ServiceUnavailable,
            };
            self.events
                .push_back(Event::Account(AccountEvent::LoginRefused {
                    reason,
                    retry_after_seconds: None,
                    detail: None,
                }));
        }
        self.end(ended);
    }

    /// The stage line of a world entry that ends: how far it got, in counts, and what was withheld
    /// on the way (§9.3.9).
    fn log_entry_end(&self, why: &str) {
        if self.ended {
            return;
        }
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
                pending_entities = self.pending.entities(),
                pending_bytes = self.pending.bytes(),
                withheld = %withheld_summary(self.resource_report.status()),
                "official world entry: stopped"
            );
            // §9.3.11 "Required live-log CONTENT": what was withheld, one line per family.
            for w in withheld_before_f4(self.resource_report.status()) {
                evidence!(
                    self.evidence_log,
                    info,
                    direction = "send",
                    event_kind = "withheld",
                    action = "not sent",
                    stage = wait.label(),
                    held_opcode = w.family,
                    withholding_reason = w.held_by,
                    "official evidence: withheld before 0xF4"
                );
            }
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
        // The flush that started the session.
        if self.send_failed {
            self.end_on_send_failure(now);
            return true;
        }
        let before = self.events.len();
        let read = read && !self.full();
        self.feed.begin_pass();
        self.feed.set_clock(now);
        self.feed_deferred();
        let read_any = read && self.receive(now, stop) > 0;
        self.run_links(now, stop);
        self.log_report_fate();
        self.run_refresh(now, stop);
        if stop() {
            return read_any || self.events.len() != before;
        }
        if read && !self.ended && !self.full() {
            // The chat client reads its own socket: it pauses with the session's. It ticks
            // and flushes its own connection, so it runs only before the cancel.
            self.run_chat(now);
        }
        self.try_finish_entry(now);
        self.log_entry_counts(now);
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
        if !self.send_failed {
            // What this pass queued is on the wire: a `0x149` read from the next pass on came
            // after it (`first_149_after_report`).
            self.report_evidence.flushed();
        }
        self.end_on_send_failure(now);
        self.report_waiting(now);
        read_any || self.events.len() != before
    }

    /// A wait of the pending selection ended (§9.3.9): `Disconnected { UnsupportedTimeout }`, with
    /// no Account event and the stage named in the log, not a claim of service silence.
    fn enter_deadline(&mut self, now: f64) {
        let Stage::Entering { since, wait, .. } = &self.stage else {
            return;
        };
        if now - since < ENTER_WORLD_TIMEOUT_SEC {
            return;
        }
        let (wait, since) = (*wait, *since);
        self.log_deadline(wait, since, now);
        self.end_by_table(TableRow::Deadline, true, wait.label(), now);
    }

    /// The entry's deadline with its two gates apart (§9.3.9, `docs/m8-remediation.md` §27 "Builder"):
    /// the server's, which has no source on a production path, and the client's, with the load time
    /// against the 30-second clock. A load that outlasts the clock is told from a gate nothing can
    /// open.
    fn log_deadline(&self, wait: EntryWait, since: f64, now: f64) {
        let server_gate = match (self.server_gate, self.server_ready) {
            (None, _) => "no source (OPEN-WORLD-9)",
            (Some(_), false) => "closed",
            (Some(_), true) => "open",
        };
        let local_gate = match (self.placed_at, self.local_ready) {
            (None, _) => "no placement",
            (Some(_), false) => "loading",
            (Some(_), true) => "open",
        };
        let load_ms = self.placed_at.map_or(0, |placed| {
            ((self.loaded_at.unwrap_or(now) - placed).max(0.0) * 1000.0) as u64
        });
        evidence!(
            self.evidence_log,
            info,
            stage = wait.label(),
            waited_ms = ((now - since).max(0.0) * 1000.0) as u64,
            limit_ms = (ENTER_WORLD_TIMEOUT_SEC * 1000.0) as u64,
            server_gate,
            local_gate,
            placement_generation = self.placement_generation,
            load_ms,
            "official evidence: entry deadline"
        );
        self.log_counts_line();
    }

    /// A valid `0x6D` for the pending selection, before a valid referral: `EnterWorldFailed` and back
    /// to the existing list, with no Disconnect (§9.3.9). The stage line says where it stopped and
    /// what was withheld before the refusal (decision 0046 item 9).
    fn fail_entry(&mut self) {
        self.log_entry_end("selection refused");
        let Stage::Entering { name, .. } = &self.stage else {
            return;
        };
        let character = name.clone();
        self.pending.discard();
        self.stage = Stage::Characters;
        self.events.push_back(Event::EnterWorldFailed {
            character,
            reason: ENTRY_REFUSED.to_owned(),
        });
    }

    /// The end of the entry (§9.3.9): once both gates hold, one `0xF4`, which ends the readiness
    /// clock, then `WorldEntered`, and then the pending state is released, one latest snapshot per
    /// entity, spawn before property and motion. The server gate has no source on a production path
    /// (OPEN-WORLD-9), so nothing here runs there (module docs).
    fn try_finish_entry(&mut self, now: f64) {
        if self.ended || !self.server_ready || !self.local_ready {
            return;
        }
        let Stage::Entering {
            name,
            wait: EntryWait::Readiness,
            ..
        } = &self.stage
        else {
            return;
        };
        let (Some(player), Some((location, yaw))) = (self.pending.player(), self.placement) else {
            return;
        };
        let name = name.clone();
        let sent = PortalReadyAck.to_bytes().is_ok_and(|env| {
            self.world
                .as_mut()
                .is_some_and(|w| Self::send_ready(w, &self.codecs, &env, now))
        });
        if !sent {
            self.counters.send_failures += 1;
            self.end_by_table(
                TableRow::Unsupported,
                true,
                "the ready message was not sent",
                now,
            );
            return;
        }
        tracing::info!("official world entry: ready message sent");
        self.note_stage("in world", now);
        self.adapter
            .enter_world_acknowledged(player, Some(name.clone()));
        self.feed.reset(Some(player));
        self.fresh_portal_loading();
        self.stage = Stage::InWorld {
            player,
            name: name.clone(),
        };
        self.events.push_back(Event::WorldEntered {
            player: EntityId(player),
            name,
            transform: Transform {
                position: location.position(),
                heading: yaw,
            },
        });
        self.release_pending();
    }

    /// The pending state goes to the play layer and the store is empty (§9.3.9): every spawn, then the
    /// first `0x90`'s table, then the merged properties and motion.
    fn release_pending(&mut self) {
        let table = self.pending.take_session_table();
        let (spawns, states) = self.pending.take_snapshots();
        if !self.release_snapshots(&spawns) {
            return;
        }
        if let Some(table) = table {
            self.release_session_table(&table);
        }
        self.release_snapshots(&states);
    }

    /// Plays `snapshots`; whether the session is still in the world.
    fn release_snapshots(&mut self, snapshots: &[EntitySnapshot]) -> bool {
        for snapshot in snapshots {
            for (opcode, body) in &snapshot.messages {
                self.play_in_world(*opcode, snapshot.entity, body);
                if !self.in_world_still() {
                    return false;
                }
            }
        }
        true
    }

    /// The first `0x90`'s table becomes the session's properties (`official-messages-world.md` §4.2).
    /// It read to exact exhaustion when it was kept; the count goes to the log, never a value.
    fn release_session_table(&mut self, table: &[u8]) {
        let mut reader = WireReader::new(table);
        let decoded = EncodedPropertyTable::decode(&mut reader, &self.schema)
            .and_then(|t| reader.finish().map(|()| t));
        self.session_properties.clear();
        match decoded {
            Ok(table) => {
                for entry in table.entries {
                    if let Some(value) = entry.value {
                        self.session_properties.insert(entry.pid, value);
                    }
                }
            }
            Err(_) => self.counters.decode_errors += 1,
        }
        evidence!(
            self.evidence_log,
            info,
            bytes = table.len(),
            properties = self.session_properties.len(),
            "official evidence: session table released"
        );
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

    /// One [`UnsupportedNegotiation`] ends the attempt (§9.3.9): its deadline or unsupported row.
    /// The cause goes to the log; the row has Tessera's fixed words. The named failures of the
    /// `0xDD`, `0xF5`, `0x12F`, `0x12D`, `0x130` and pre-`Connected` `0xB3` rules end here too,
    /// taking those sentences as terminal (labelled, `docs/m8-remediation.md` §27 fix 9: they will
    /// probably join the closed class).
    fn refuse_unsupported(&mut self, why: UnsupportedNegotiation, now: f64) {
        let row = if why.is_deadline() {
            TableRow::Deadline
        } else {
            TableRow::Unsupported
        };
        // The Display form names the kind (never a value), so the log tells the checks apart.
        let reason = why.to_string();
        self.end_by_table(row, true, &reason, now);
    }

    /// A local terminal close (§9.3.9): exactly one standalone Disconnect on the primary active
    /// connection when it is established, none in its handshake, none on a retired or retiring login
    /// connection; then every connection closes locally, with no flush wait and no ticket resent.
    fn say_goodbye(&mut self, why: &str, now: f64) {
        self.close_links(self.world.is_some(), why, now);
    }

    /// [`Self::say_goodbye`] with the one Disconnect's connection named. A connection in its handshake
    /// sends nothing.
    fn close_links(&mut self, on_world: bool, why: &str, now: f64) {
        let target = if on_world {
            self.world.as_mut()
        } else {
            Some(&mut self.login)
        };
        if let Some(l) = target
            && !l.in_handshake()
            && let Some(c) = l.mux.connection_mut(l.conn)
        {
            let _ = c.queue_standalone_disconnect(now);
        }
        // Its queue may hold the ticket.
        for l in self.links_mut() {
            if l.in_handshake() {
                l.close_dump(why);
                l.close_quietly();
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

    /// A new stage of the attempt (`docs/DEVELOPMENT.md` "Reading a live official run"): one line
    /// with its name, how long the last one took and the time since the attempt started, and the
    /// connections' evidence lines count from here. Stage names are fixed words.
    fn note_stage(&mut self, stage: &'static str, now: f64) {
        let (previous, since) = self.evidence_stage;
        evidence!(
            self.evidence_log,
            info,
            stage,
            previous,
            previous_ms = ((now - since).max(0.0) * 1000.0) as u64,
            since_start_ms = ((now - self.evidence_start).max(0.0) * 1000.0) as u64,
            "official evidence: stage"
        );
        self.evidence_stage = (stage, now);
        for l in self.links_mut() {
            if let Some(c) = l.mux.connection_mut(l.conn) {
                c.evidence_stage(stage, now);
            }
        }
    }

    /// The counts of what the world connection sent after `0x90`: at most one line per
    /// [`ENTRY_COUNTS_LOG_SEC`] during the readiness wait, and no value.
    fn log_entry_counts(&mut self, now: f64) {
        if !matches!(
            self.stage,
            Stage::Entering {
                wait: EntryWait::Readiness,
                ..
            }
        ) || now - self.entry_counts_at < ENTRY_COUNTS_LOG_SEC
        {
            return;
        }
        self.entry_counts_at = now;
        self.log_counts_line();
    }

    fn log_counts_line(&self) {
        let p = &self.entry_progress;
        let queues = self
            .world
            .as_ref()
            .and_then(|l| l.mux.connection(l.conn))
            .map(|c| c.reassembly_counts())
            .unwrap_or_default();
        evidence!(
            self.evidence_log,
            info,
            batches = p.batches,
            sub_events = p.sub_events,
            creations = p.creations,
            properties = p.properties,
            positions = p.positions,
            placements = p.placements,
            appearances = p.appearances,
            despawns = p.despawns,
            despawn_blocked = p.despawn_blocked,
            pending_target_cap_hit = p.target_cap_hits,
            pending_bytes_cap_hit = p.bytes_cap_hits,
            send_failures = self.counters.send_failures,
            placement_generation = p.placement_generation,
            readiness_seen = p.readiness_seen,
            unsupported_batches = p.unsupported_batches,
            actions = p.actions,
            default_target = p.default_target,
            no_target = p.no_target,
            unannounced_cleared = p.unannounced_cleared,
            other = p.other,
            unread = p.unread,
            pending_entities = self.pending.entities(),
            pending_bytes = self.pending.bytes(),
            session_table_bytes = self.pending.session_table_bytes(),
            queue3_groups_over = queues.queue3_groups_over,
            queue3_older = queues.queue3_older,
            queue3_ambiguous = queues.queue3_ambiguous,
            reused_identities = queues.reused_identities,
            duplicates = queues.duplicates,
            tombstones_evicted = queues.tombstones_evicted,
            "official evidence: counts after the session start"
        );
    }

    /// The stages after a step: synced, one log line and a progress event.
    fn initial_step(&mut self, what: &'static str, now: f64) {
        self.note_stage(what, now);
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
    /// (module docs "Leaving the world").
    fn left_world(&mut self) {
        tracing::info!("official session: the world was left for character selection");
        self.log_world_end();
        self.stage = Stage::Characters;
        self.returned_from_world = true;
        self.pending.discard();
        self.session_properties.clear();
        self.placement = None;
        self.local_ready = false;
        self.server_ready = false;
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
            _ => self.world.as_mut(),
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
                        evidence!(self.evidence_log, debug,
                            direction = "recv",
                            kind = %error_kind(&e),
                            raw_len = datagram.len(),
                            prefix_zero = datagram.starts_with(&[0, 0]),
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
                // §9.3.12 "Login transport lifetime" (P10-10L): the referred login connection keeps
                // its transport-only Ack, NAK, clock and flow handling while the world lives; world
                // State 5 does not end it.
                if world
                    && let Stage::Entering { wait, since, .. } = &mut self.stage
                    && *wait == EntryWait::WorldConnection
                {
                    *wait = EntryWait::SessionStart;
                    *since = now;
                    self.entry_progress.world_connected = true;
                    tracing::info!(stage = wait.label(), "official world entry: next stage");
                    let label = wait.label();
                    self.note_stage(label, now);
                    self.events.push_back(Event::WorldEntryProgress {
                        stage: WorldEntryStage::WorldHandshake,
                    });
                }
                if self.stage == Stage::Login {
                    if !world {
                        self.initial.connected(now);
                    }
                    self.note_stage("login connection established", now);
                    // The progress screen's watchdog (`official-entry.md` §8.3) stays
                    // alive while the version exchange and the character list follow.
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginProgress {
                            stage: LoginStage::JoiningWorld,
                        }));
                }
            }
            // §9.3.9: nothing the retiring login connection says concerns the session any more.
            SessionEvent::BlobReceived(blob) if !world && self.login_referred => {
                self.counters.retired_messages = self.counters.retired_messages.saturating_add(1);
                if blob.blob_header_word == OPCODE_LOGIN_149 {
                    self.note_login_149();
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
                        if let Some(skipped) = self.message_log.allow(now) {
                            evidence!(self.evidence_log, debug,
                                direction = "recv",
                                kind = %error_kind(&e),
                                skipped,
                                "official session: message refused"
                            );
                        }
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
                    self.referral(cookie, world_addr, world, now);
                }
            }
            SessionEvent::AuthServerAddress(_) => {
                // §6.6 names no client action that this session needs.
                tracing::debug!("official session: redirect header ignored");
            }
            SessionEvent::Disconnected { reason } => self.link_closed(reason, world, now),
        }
    }

    /// §9.3.9: a fresh queue-2 identity is delivered when it completes, and an ordering word that
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
        let (stage, since) = self.evidence_stage;
        evidence!(
            self.evidence_log,
            info,
            role = if world { "world" } else { "login" },
            reason = reason.label(),
            retiring = !world && self.login_referred,
            stage,
            stage_ms = ((now - since).max(0.0) * 1000.0) as u64,
            "official evidence: connection closed"
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
        // §9.3.9: nothing the retiring login connection does decides the session's end. Its close
        // releases the report it still kept (§9.3.12), which its own summary line records.
        if !world && (self.world.is_some() || self.login_referred) {
            if self.login_referred {
                self.log_report_summary(SummaryPoint::LoginClose);
            }
            return;
        }
        // §9.3.12 "Old-install rejection": a non-fatal TransportError carrying either login-refusal
        // code of the §9.3.9 TransportError row ([`LOGIN_REFUSAL_CODES`]) before any challenge is
        // the condition PreChallengeInstallRejected, a private diagnostic only: one local line with
        // the advice to check that the install is up to date, never a claim that it is out of date
        // and never a value. A TransportFatalError is not this condition. The `ClientNotAccepted`
        // refusal and its panel below are unchanged.
        if let LinkEnd::TransportError {
            error_code,
            fatal: false,
            ..
        } = &reason
            && !world
            && LOGIN_REFUSAL_CODES.contains(error_code)
            && self.login.closed_before_challenge()
        {
            tracing::info!(
                diagnostic = "PreChallengeInstallRejected",
                "official session: the login service refused this client before its challenge; \
                 check that the install is up to date"
            );
        }
        let (row, goodbye) = match reason {
            LinkEnd::LocalDisconnect => {
                self.end(DisconnectReason::ClientRequested);
                return;
            }
            // The session layer sent the one Disconnect already, if it was established; the close
            // row has it flushed. A handshake expiry had nothing to send.
            LinkEnd::HandshakeTimeout | LinkEnd::SilenceTimeout => (TableRow::Silence, true),
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
            LinkEnd::UnsupportedCodec | LinkEnd::InvalidChallenge => (TableRow::Unsupported, false),
            // The session layer sent the one Disconnect already.
            LinkEnd::UnsupportedTraffic => (TableRow::Unsupported, true),
            LinkEnd::GapDeadline => (TableRow::Deadline, true),
            // Labelled (`docs/m8-remediation.md` §28 fixes 14, 15): the Io row until the writer gives
            // this its own, so no Disconnect (§9.3.9's I/O row).
            LinkEnd::OutboundWindowOverflow => (TableRow::Io, false),
        };
        self.end_by_table(row, goodbye, reason.label(), now);
    }

    /// A `WorldReferralHeader` (`official-session.md` §6.5): a new world connection on the same
    /// socket, in a multiplexer of its own (module docs). §9.3.9: only the login connection's first,
    /// for a pending selection; any other is counted and skipped, with no connection made and no
    /// clock replaced. A zero address or port for that selection is unsupported required progress: one
    /// close, and no bind or send to it. The login connection sends no Disconnect at a valid referral:
    /// it retires, its later events no longer concern the session, and its transport-only handling
    /// goes on while the world lives (§9.3.12 "Login transport lifetime").
    fn referral(&mut self, cookie: [u8; 8], addr: SocketAddrV4, world: bool, now: f64) {
        let awaited = matches!(
            self.stage,
            Stage::Entering {
                wait: EntryWait::Referral,
                ..
            }
        );
        if world || !awaited {
            self.counters.stray_referrals += 1;
            tracing::debug!("official session: referral with no pending selection ignored");
            return;
        }
        // A named required-progress failure (§9.3.9): `end_by_table` gives the LoginRefused
        // UnsupportedStep then Disconnected Unsupported pair before `Connected` and Disconnected
        // Unsupported after it, which is where a selection's referral always comes.
        if addr.ip().is_unspecified() || addr.port() == 0 {
            self.end_by_table(
                TableRow::Unsupported,
                true,
                "referral with no endpoint",
                now,
            );
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
                link.evidence(self.evidence_log, now);
                if let Some(c) = link.mux.connection_mut(link.conn)
                    && let Err(e) = c.start_world_login(cookie, now)
                {
                    tracing::warn!(error = %e, "official session: referral not started");
                    return;
                }
                self.world = Some(link);
                // §9.3.9: zero Disconnect. The login connection's applications retire now, while its
                // bounded transport-only Ack/NAK/clock/flow goes on while the world connection
                // lives, on the shared socket (§9.3.12 "Login transport lifetime").
                self.login_referred = true;
                self.log_report_summary(SummaryPoint::Referral);
                if let Some(c) = self.login.mux.connection_mut(self.login.conn) {
                    c.retire();
                }
                tracing::info!("official session: referred to the world server");
                self.entry_progress.referred = true;
                // §9.3.9: the referral ends its wait and starts the next.
                if let Stage::Entering { wait, since, .. } = &mut self.stage {
                    *wait = EntryWait::WorldConnection;
                    *since = now;
                }
                self.note_stage(EntryWait::WorldConnection.label(), now);
                self.events.push_back(Event::WorldEntryProgress {
                    stage: WorldEntryStage::Referral,
                });
                if let Some(w) = self.world.as_ref() {
                    self.report(w, true, now).log_start(None);
                }
            }
            Err(e) => tracing::warn!(error = %e, "official session: referral refused"),
        }
    }

    fn message(&mut self, m: &Inbound, now: f64) {
        self.log_clock = now;
        let (opcode, recipient, body, world) = (m.opcode, m.recipient, m.body.as_slice(), m.world);
        if opcode == OPCODE_LOGIN_149 && !world {
            self.note_login_149();
        }
        if (OPCODE_CLIENT_VERSION..=OPCODE_LOGIN_QUEUE_STATUS).contains(&opcode) {
            match ControlMessage::decode(opcode, body) {
                Ok(m) => self.control(m, now),
                Err(e) => {
                    // §9.3.9: malformed notice data is counted and discarded, not dispatched.
                    self.counters.decode_errors += 1;
                    self.counters.malformed_control =
                        self.counters.malformed_control.saturating_add(1);
                    if let Some(skipped) = self.message_log.allow(now) {
                        evidence!(self.evidence_log, debug,
                            direction = "recv",
                            opcode = format_args!("{opcode:#x}"),
                            kind = %error_kind(&e),
                            skipped,
                            "official session: control message refused"
                        );
                    }
                }
            }
            return;
        }
        // These have their own handling, in the current one-byte form (§9.1, §9.3.9): none goes
        // through the historical base-flags test below.
        match opcode {
            OPCODE_INSTALL_REQUEST => return self.install_request(m, now),
            OPCODE_COMPLETION => return self.completion(m, now),
            CharacterListResponse::EVENT_ID => return self.character_list(m, now),
            PlayerSessionReady::EVENT_ID if self.session_start_due(m.world) => {
                return self.session_ready(m, now);
            }
            PlayerSessionReady::EVENT_ID if self.session_start_after_entry() => return,
            CharacterOperationError::EVENT_ID => return self.character_error(m),
            AccountEvictionNotice::EVENT_ID => return self.eviction(m, now),
            ServerShutdownText::EVENT_ID => return self.shutdown_notice(m),
            _ => {}
        }
        if self.login_window_skips(m) {
            return;
        }
        // §9.3.9: the checked family has one shared byte, consumed once. It is put back so the
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
        // On the way to `WorldEntered` the world connection's messages are kept or counted, never
        // published (module docs).
        if world && matches!(self.stage, Stage::Entering { .. }) {
            self.pending_message(opcode, recipient, data, 0, now);
            return;
        }
        if data.first() != Some(&0) {
            self.counters.decode_errors += 1;
            return;
        }
        self.play(opcode, recipient, data, world);
    }

    /// Whether a `0x90` of this connection is judged as a session start (§9.3.9, §9.3.11): only the
    /// current world peer's, from that connection's State 5 until `WorldEntered`, so a later one is
    /// compared with the first. A login or other peer's `0x90` goes to the bounded skips without
    /// touching the world identity (§9.3.11). After `WorldEntered` every later `0x90` is counted and
    /// skipped, with no rebinding and no table replacement ([`Self::session_start_after_entry`]). A
    /// network with [`ConnectorNet::readiness_specified`] is a test's synthetic script, which may
    /// carry the world on the login connection (module docs).
    fn session_start_due(&self, world: bool) -> bool {
        match &self.stage {
            Stage::Entering { wait, .. } if !self.readiness_specified => {
                world && matches!(*wait, EntryWait::SessionStart | EntryWait::Readiness)
            }
            Stage::Entering { .. } => true,
            Stage::Login | Stage::Characters | Stage::InWorld { .. } => false,
        }
    }

    /// A `0x90` after `WorldEntered` (§9.3.11, Vector 24 pending extension): counted and skipped,
    /// readable or not, with no identity rebinding and no table replacement; its in-world meaning
    /// stays OPEN-WORLD-9. Answers whether it was one.
    fn session_start_after_entry(&mut self) -> bool {
        if !matches!(self.stage, Stage::InWorld { .. }) {
            return false;
        }
        self.counters.session_starts_after_entry =
            self.counters.session_starts_after_entry.saturating_add(1);
        true
    }

    /// Whether `m` is passed over on the login connection (§9.3.9's application dispatch scope):
    /// before `WorldEntered` everything without its own handling is bounded (reassembled already),
    /// counted per opcode, not dispatched, not answered and no decode error. That includes the
    /// server `0x30`s before a referral, never buffered for later replay, and `0xB0` on the primary
    /// login connection (§9.3.12 "B0 on primary login"): counted and skipped, with no reply, list
    /// change, stage transition or stage-clock reset; only its verified datagram resets the
    /// independent silence clock, in the session layer.
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
    /// on as it would for an unreadable body of that message (§9.3.9). Anything else is counted.
    fn envelope_refused(&mut self, blob: &ReassembledBlob, world: bool, now: f64) {
        match blob.blob_header_word {
            // §9.3.11: every required `0x90` from the current world peer must read, first or later,
            // until `WorldEntered`; after `Connected` this is Disconnected Unsupported alone
            // (`end_by_table`). After `WorldEntered` it is counted and skipped.
            PlayerSessionReady::EVENT_ID if self.session_start_after_entry() => return,
            PlayerSessionReady::EVENT_ID if self.session_start_due(world) => {
                return self.end_by_table(
                    TableRow::Unsupported,
                    true,
                    "session start unreadable",
                    now,
                );
            }
            AccountEvictionNotice::EVENT_ID => return self.refusal_unread(now),
            CharacterOperationError::EVENT_ID => {
                self.counters.refusals_skipped = self.counters.refusals_skipped.saturating_add(1);
                return;
            }
            _ => {}
        }
        if world || self.stage != Stage::Login {
            if blob.blob_header_word == CharacterListResponse::EVENT_ID {
                self.counters.later_lists_skipped =
                    self.counters.later_lists_skipped.saturating_add(1);
            }
            return;
        }
        let why = match blob.blob_header_word {
            // §9.3.9 stage table: before `Connected` (this is the login stage) a queue-1 `0xDD` that
            // does not read, or with flags `02..FF`, is unsupported in every stage.
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

    /// The server's `0xDD` (§9.3.9). Only flags `00` or valid zlib-only `01` on queue 1 of the
    /// primary login connection, during the initial stage, can trigger `0xEC`, and only the first,
    /// which must have an empty array. A later well-formed one, its array empty or not, only updates
    /// the opaque word and is counted: nothing is sent, no wait starts again and the install is not
    /// looked at again. One on another queue or connection, or after `Connected`, is counted and
    /// skipped. Before `Connected` a malformed one fails in every stage, and so does flags `02..FF`
    /// (§9.3.9 stage table, DD column).
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
            // §9.3.9 `Queued130` row: a first one while queued is held for the ready status
            // (`Negotiation::request_not_wanted`).
            self.initial.request_not_wanted(request, m.recipient);
            self.sync_initial();
            return;
        }
        self.initial.request_seen(request.server_config);
        self.initial_step("install request", now);
        if let Err(why) = self.send_advertisement(&request, m.recipient, now) {
            self.refuse_unsupported(why, now);
        }
    }

    /// After a ready status brought the stage back to `AwaitDD`: the `0xDD` held from the queue, if
    /// any, is the first valid one now and sends `0xEC` once (§9.3.9 status rule, Vector 25,
    /// `Negotiation::ready_before_request`). The ready branches then check the reinstated
    /// post-queue `0xDD` deadline at once ([`Self::initial_deadline`]), so a `0xDD` later in the
    /// same pass never answers a deadline that has already passed (the labelled §32 fix 2 choice
    /// in `Negotiation::ready_before_request`); a held one has moved the stage on already.
    fn release_held_request(&mut self, now: f64) {
        let Some((request, recipient)) = self.initial.take_held_request() else {
            return;
        };
        self.initial_step("install request (held from the queue)", now);
        if let Err(why) = self.send_advertisement(&request, recipient, now) {
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

    /// The server's `0xF5` (§9.3.9). One on another queue or connection is counted and skipped
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

    /// The historical messages and the notice (§9.3, §9.3.9): not observed in either initial
    /// conversation, so these reasons are policy. After `Connected` every historical version and
    /// queue message is counted and skipped; a notice is shown in every stage and never ends or
    /// resets anything.
    fn control(&mut self, m: ControlMessage, now: f64) {
        if let ControlMessage::SystemNotification(n) = &m {
            self.notice(&n.text);
            return;
        }
        if self.stage != Stage::Login {
            self.counters.unhandled += 1;
            return;
        }
        match m {
            ControlMessage::VersionRequest => {
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
                1 => {
                    // §9.3.9: a real stage change only; a repeat is counted and changes no clock.
                    if self.initial.version_accepted(now) {
                        self.initial_step("version accepted", now);
                        self.release_held_request(now);
                        self.initial_deadline(now);
                    } else {
                        self.sync_initial();
                    }
                }
                // §9.3.9: refused, closed with no reply.
                0 => {
                    self.end_by_table(TableRow::ServerRefused, false, "version not accepted", now);
                }
                _ => self.refuse_unsupported(UnsupportedNegotiation::UnknownVersionStatus, now),
            },
            ControlMessage::LoginQueueStatus(q) => match q.outcome() {
                // §9.3.9: only the fields actually there; one absolute limit, never extended. A
                // later queued status reinstates the SAME limit; if it has already passed, the
                // attempt fails at once (Vector 25).
                LogonOutcome::Queued(at) => {
                    self.initial.queued(now);
                    self.sync_initial();
                    if let Some(why) = self.initial.expired(now) {
                        self.refuse_unsupported(why, now);
                        return;
                    }
                    self.events
                        .push_back(Event::Account(AccountEvent::LoginQueueChanged {
                            position: (at.position > 0).then_some(at.position),
                            length: (at.total > 0).then_some(at.total),
                            wait_seconds: None,
                        }));
                }
                // §9.3.9: ready; the 30-second list wait starts and the progress screen comes back.
                LogonOutcome::Accepted => {
                    if self.initial.queue_ready(now) {
                        self.initial_step("queue ready", now);
                        self.release_held_request(now);
                        self.initial_deadline(now);
                    } else {
                        self.sync_initial();
                    }
                }
                // §9.3.9: `LoginRefused { Other }`, closed with no reply. The number is for the
                // debug log.
                LogonOutcome::Refused(code) => {
                    evidence!(
                        self.evidence_log,
                        debug,
                        direction = "recv",
                        logon_result = format_args!("{code:#010x}"),
                        "official session: the service refused the logon"
                    );
                    self.end_by_table(TableRow::ServerOther, false, "logon refused", now);
                }
                LogonOutcome::Unknown(code) => {
                    evidence!(
                        self.evidence_log,
                        debug,
                        direction = "recv",
                        logon_result = format_args!("{code:#010x}"),
                        "official session: logon result with no meaning in the spec"
                    );
                    self.refuse_unsupported(UnsupportedNegotiation::UnknownQueueStatus, now);
                }
            },
            ControlMessage::SystemNotification(_) | ControlMessage::ClientVersion(_) => {
                self.counters.unhandled += 1;
            }
        }
    }

    /// `0x12E` or a resolved `0xFA` (§9.3.9): plain informational text in every phase, past 256 UTF-8
    /// bytes dropped and counted, never cut into another notice. It advances no stage, resets no
    /// clock and has no reply. The client cleans it (`official-entry.md` §8.2).
    fn notice(&mut self, text: &str) {
        if text.len() > MAX_ACCOUNT_DETAIL_LEN {
            self.counters.notices_dropped = self.counters.notices_dropped.saturating_add(1);
            return;
        }
        self.events
            .push_back(Event::Account(AccountEvent::LoginNotice {
                text: text.to_owned(),
            }));
    }

    /// `0xFA` (§9.3.9): decoded by its historical bounded text schema and resolved, or counted.
    fn shutdown_notice(&mut self, m: &Inbound) {
        let text = (m.envelope_flags == 0)
            .then(|| ServerShutdownText::from_bytes(&m.body).ok())
            .flatten()
            .and_then(|t| TextResolver::resolve(&self.schema, &t.last_words));
        match text {
            Some(text) => self.notice(&text),
            None => {
                self.counters.notices_dropped = self.counters.notices_dropped.saturating_add(1);
            }
        }
    }

    /// The first character list (§9.3.9): the count straight after the one flags byte, every entry and
    /// the account table to exact exhaustion, and then the first-screen contract ([`listed_names`]).
    /// One that fails ends the attempt, with no forward scan, no partial list and no character
    /// invented, and so does a compressed or routed one; an unknown appearance form (OPEN-WORLD-9) is
    /// such a failure. Every later list is counted and skipped in every phase: the list and its
    /// private IDs stay as they are. `Connected` carries names only: no attribute is guessed. `0x19E`
    /// is not emitted (decision 0046 item 5); decision 0046 item 13's `0x13D` is queued once here,
    /// before `Connected`, or omitted ([`Self::resource_report`]). The account table belongs to this
    /// list's character-screen state and triggers nothing.
    fn character_list(&mut self, m: &Inbound, now: f64) {
        if self.stage != Stage::Login {
            self.counters.later_lists_skipped = self.counters.later_lists_skipped.saturating_add(1);
            return;
        }
        let list = if m.envelope_flags == 0 {
            match CharacterListResponse::from_envelope_body(&m.body, &self.schema) {
                Err(e) => {
                    tracing::debug!(kind = %error_kind(&e), "official session: character list refused");
                    self.log_skipped_property(&e, "character list");
                    Err(UnsupportedNegotiation::ListDecode)
                }
                Ok(list) => listed_names(
                    list.characters
                        .iter()
                        .map(|c| (c.character_id, c.name.as_str())),
                )
                .ok_or(UnsupportedNegotiation::ListNames),
            }
        } else {
            tracing::debug!("official session: compressed or routed character list refused");
            Err(UnsupportedNegotiation::ListEnvelope)
        };
        let names = match list {
            Ok(names) => names,
            Err(why) => {
                self.counters.decode_errors += 1;
                self.refuse_unsupported(why, now);
                return;
            }
        };
        self.characters = names;
        self.initial.list_received();
        self.initial_step("character list", now);
        // §9.3.12: after the first complete list and before `Connected`, so before any `0x6F`.
        self.resource_report(now);
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

    /// `0x6D` (§9.3.9, Vector 15): exactly the shared byte and the `u32` reason. For a pending
    /// selection before a valid referral it fails the entry without an automatic retry, with a
    /// locally worded refusal; anything else, unreadable or not, is counted with no event, a valid
    /// one with no selection pending too (§9.3.9 additional-condition table). The number
    /// is for the debug log only.
    fn character_error(&mut self, m: &Inbound) {
        let code = (m.envelope_flags == 0)
            .then(|| CharacterOperationError::from_envelope_body(&m.body).ok())
            .flatten()
            .map(|e| e.error_code);
        let waiting = matches!(
            self.stage,
            Stage::Entering {
                wait: EntryWait::Referral,
                ..
            }
        );
        match code {
            Some(code) if waiting && !m.world => {
                evidence!(
                    self.evidence_log,
                    debug,
                    direction = "recv",
                    code,
                    "official session: character operation refused"
                );
                self.fail_entry();
            }
            _ => {
                self.counters.refusals_skipped = self.counters.refusals_skipped.saturating_add(1);
            }
        }
    }

    /// A `0xB3` that is not structurally valid (§9.3.9): unsupported before `Connected`, else
    /// counted and skipped.
    fn refusal_unread(&mut self, now: f64) {
        self.counters.decode_errors += 1;
        if self.stage == Stage::Login {
            self.refuse_unsupported(UnsupportedNegotiation::MalformedRefusal, now);
        } else {
            self.counters.refusals_skipped = self.counters.refusals_skipped.saturating_add(1);
        }
    }

    /// `0x90` (§9.3.9, §9.3.11, Vector 15): the ID straight after the one shared flags byte, then the
    /// property table to exact exhaustion. The first of an entry must read, uncompressed and unrouted,
    /// with an ID other than zero and the logical default 2, or that is the unsupported row; the ID is
    /// never shifted by a guessed second byte. It starts the readiness wait and is not `WorldEntered` by
    /// itself: its identity and its encoded table are kept pending (module docs). A later valid one
    /// repeating the ID is counted and skipped, the first table staying as it is (§9.3.11), a later
    /// valid one with another ID, zero or the logical 2 is unsupported (no silent rebinding), and
    /// every `0x90` of the current world peer that does not read, first or later, is unsupported
    /// (§9.3.11). These come after `Connected`, so the outcome is Disconnected Unsupported with no
    /// LoginRefused pair ([`Self::end_by_table`]).
    fn session_ready(&mut self, m: &Inbound, now: f64) {
        // After `WorldEntered` a later one is counted and skipped, never Unsupported (§9.3.11).
        if self.session_start_after_entry() {
            return;
        }
        // A compressed or routed form is an unsupported variant, refused like a body that does not
        // read.
        let decoded = (m.envelope_flags == 0)
            .then(|| PlayerSessionReady::from_envelope_body(&m.body, &self.schema));
        let ready = match decoded {
            Some(Ok(ready)) => ready,
            other => {
                self.counters.decode_errors += 1;
                tracing::debug!("official session: session start refused");
                if let Some(Err(e)) = &other {
                    self.log_skipped_property(e, "session start");
                }
                self.end_by_table(TableRow::Unsupported, true, "session start unreadable", now);
                return;
            }
        };
        if let Some(first) = self.world_identity {
            if first == ready.player_id {
                self.counters.session_starts_repeated =
                    self.counters.session_starts_repeated.saturating_add(1);
            } else {
                // The named "later conflicting 90" failure (§9.3.9): the table's Unsupported pair
                // before `Connected`, Disconnected Unsupported after it ([`Self::end_by_table`]).
                self.end_by_table(
                    TableRow::Unsupported,
                    true,
                    "session start with another ID",
                    now,
                );
            }
            return;
        }
        let Stage::Entering {
            name, wait, since, ..
        } = &mut self.stage
        else {
            tracing::debug!("official session: session start outside world entry");
            return;
        };
        if ready.player_id == 0 || ready.player_id == DEFAULT_RECIPIENT {
            self.end_by_table(TableRow::Unsupported, true, "session start with no ID", now);
            return;
        }
        let name = name.clone();
        *wait = EntryWait::Readiness;
        *since = now;
        self.entry_progress.session_started = true;
        tracing::info!(stage = wait.label(), "official world entry: next stage");
        self.world_identity = Some(ready.player_id);
        self.entry_counts_at = now;
        self.note_stage(EntryWait::Readiness.label(), now);
        // What follows the ID read to exact exhaustion above.
        if let Some(table) = m.body.get(size_of::<u64>()..) {
            self.pending.keep_session_table(table);
        }
        if self.readiness_specified {
            return self.enter_at_session_start(ready.player_id, name);
        }
        self.events.push_back(Event::WorldEntryProgress {
            stage: WorldEntryStage::Identity,
        });
        let kept = self.pending.set_player(ready.player_id);
        if let Some((at, yaw)) = self.exact_placement(ready.player_id, kept) {
            self.local_placement(at, yaw, now);
        }
    }

    /// [`ConnectorNet::readiness_specified`]: a test's fixture for the in-world code, which enters
    /// at `0x90` with no gate and no `0xF4` of its own (module docs).
    fn enter_at_session_start(&mut self, player: u64, name: String) {
        self.adapter.enter_world(player, Some(name.clone()));
        self.feed.reset(Some(player));
        // World entry is a portal space (the adapter's `enter_world`).
        self.fresh_portal_loading();
        self.stage = Stage::InWorld {
            player,
            name: name.clone(),
        };
        self.events.push_back(Event::WorldEntered {
            player: EntityId(player),
            name,
            transform: Transform::default(),
        });
        self.release_pending();
    }

    /// The one trigger of `WorldPlacementPending` (§9.3.9, `docs/m8-remediation.md` §27 fix 6): whether
    /// `at`, a location kept for `entity`, is the local player's exact current placement. It answers
    /// nothing until OPEN-WORLD-9 names the source; a test's scripted network stands in for it.
    fn exact_placement(&self, entity: u64, at: Option<(Location, f32)>) -> Option<(Location, f32)> {
        if !self.placement_scripted || self.pending.player() != Some(entity) {
            return None;
        }
        at
    }

    /// The exact placement of the local player while entering (§9.3.9, §9.3.11): generation 1, and
    /// the client starts loading. A repeated update of the same placement is kept but makes no new
    /// generation and does not clear the local gate.
    fn local_placement(&mut self, at: Location, yaw: f32, now: f64) {
        if self.placement.is_none() {
            self.new_placement(at, yaw, now);
        }
    }

    /// One sourced placement: its generation, the old load invalidated, and the one pending event.
    fn new_placement(&mut self, at: Location, yaw: f32, now: f64) {
        let Stage::Entering { name, .. } = &self.stage else {
            return;
        };
        let character = name.clone();
        self.entry_progress.placed = true;
        self.placement_generation = self.placement_generation.saturating_add(1);
        self.entry_progress.placement_generation = self.placement_generation;
        self.local_ready = false;
        self.entry_progress.loaded = false;
        self.loaded_at = None;
        self.events.push_back(Event::WorldEntryProgress {
            stage: WorldEntryStage::Placement,
        });
        self.placement = Some((at, yaw));
        tracing::info!(
            generation = self.placement_generation,
            "official world entry: placement pending"
        );
        self.placed_at = Some(now);
        // ARCHITECTURE.md §19.1: the anchor is optional; no placement form of the session names an
        // interior owner yet (OPEN-WORLD-9), so none is given.
        self.events.push_back(Event::WorldPlacementPending {
            character,
            position: at,
            anchor: None,
            yaw,
            generation: self.placement_generation,
        });
    }

    /// Synthetic tests only: a later sourced placement (§9.3.11's scripted case). Nothing on a
    /// production path names one, so nothing else makes a new generation.
    #[cfg(test)]
    pub fn scripted_new_placement_for_tests(&mut self, at: Location, yaw: f32, now: f64) {
        self.new_placement(at, yaw, now);
    }

    /// What a message of the world connection is on the way to `WorldEntered` (§9.3.9). `data` has
    /// its one shared byte, and each embedded slice its own. A checked message is kept only when it
    /// decodes to exact exhaustion; a child of a batch that does not is counted without touching its
    /// siblings. The rest is counted and skipped: no unknown RPC runs and nothing is read as
    /// readiness, except that a test's [`ConnectorNet::server_gate`] stands in for its source. A full
    /// store counts and skips: it never ends the entry ([`Self::note_stored`]).
    fn pending_message(&mut self, opcode: u32, target: u64, data: &[u8], depth: usize, now: f64) {
        let (key, stored) = match opcode {
            SubEventBatch::EVENT_ID_SUB_EVENT => {
                self.pending_batch(opcode, target, data, depth, now);
                return;
            }
            EntityCreate::EVENT_ID => match EntityCreate::from_bytes(data) {
                Ok(c) => {
                    let stored = self.pending.create(c.entity_id, data);
                    self.entry_progress.creations += u64::from(stored == Stored::Kept);
                    self.note_entry_first(EntryFirst::Creation, stored, depth, now);
                    (c.entity_id, stored)
                }
                Err(_) => return self.pending_unread(),
            },
            PropertyTableUpdate::EVENT_ID_ENTITY => {
                match PropertyTableUpdate::entry_spans(data, &self.schema) {
                    Ok(entries) => {
                        let stored = self.pending.table(target, &entries);
                        self.entry_progress.creations += u64::from(stored == Stored::Kept);
                        (target, stored)
                    }
                    Err(e) => {
                        self.log_skipped_property(&e, "pending");
                        return self.pending_unread();
                    }
                }
            }
            op if SinglePropertyUpdate::scope_of(op) == Some(PropertyScope::Entity) => {
                let mut r = WireReader::new(data);
                match SinglePropertyUpdate::decode(&mut r, op, &self.schema)
                    .and_then(|u| r.finish().map(|()| u))
                {
                    Ok(u) => {
                        let stored = self.pending.single(target, op, u.entry.pid, data);
                        self.entry_progress.properties += u64::from(stored == Stored::Kept);
                        (target, stored)
                    }
                    Err(e) => {
                        self.log_skipped_property(&e, "pending");
                        return self.pending_unread();
                    }
                }
            }
            VisualDescriptionUpdate::EVENT_ID => match VisualDescriptionUpdate::from_bytes(data) {
                Ok(_) => {
                    let stored = self.pending.appearance(target, data);
                    self.entry_progress.appearances += u64::from(stored == Stored::Kept);
                    (target, stored)
                }
                Err(_) => return self.pending_unread(),
            },
            // §9.3.9, §9.3.12 Question 1: the pending key is the RAW logical recipient until the
            // controlled-target resolver is sourced; the leading eight-byte word is never the key, and
            // the conditional selector (the attachment form's eight-byte word) is metadata only.
            PositionUpdate::EVENT_ID => match PositionUpdate::from_bytes(data) {
                Ok(u) => {
                    let at = placement_location(&u.placement).map(|(l, _, h)| (l, h));
                    if at.is_some_and(|(l, h)| !usable_placement(&l, h)) {
                        return self.pending_unread();
                    }
                    let selector = match u.placement {
                        EntityPlacement::Attachment { parent_id, .. } if parent_id != 0 => {
                            Some(parent_id)
                        }
                        _ => None,
                    };
                    let stored = self.pending.position(target, data, at, selector);
                    self.entry_progress.positions += u64::from(stored == Stored::Kept);
                    self.note_entry_first(EntryFirst::Position, stored, depth, now);
                    // The placement is required state: a full store does not lose it.
                    if stored != Stored::Despawned
                        && let Some((l, h)) = self.exact_placement(target, at)
                    {
                        self.local_placement(l, h, now);
                    }
                    (target, stored)
                }
                Err(_) => return self.pending_unread(),
            },
            PortalPositionSync::EVENT_ID => {
                match PortalPositionSync::from_bytes(data, &self.schema) {
                    Ok(p) => {
                        let at = position_location(&p.position, Some(wire_heading(p.heading_raw)))
                            .map(|(l, _, h)| (l, h));
                        if at.is_some_and(|(l, h)| !usable_placement(&l, h)) {
                            return self.pending_unread();
                        }
                        let stored = self.pending.sync(target, data, at);
                        self.entry_progress.placements += u64::from(stored == Stored::Kept);
                        self.note_entry_first(EntryFirst::Placement, stored, depth, now);
                        if stored != Stored::Despawned
                            && let Some((l, h)) = self.exact_placement(target, at)
                        {
                            self.local_placement(l, h, now);
                        }
                        (target, stored)
                    }
                    Err(_) => return self.pending_unread(),
                }
            }
            // §9.3.9: nothing is published before `WorldEntered`: a record stays in the store, and no
            // later update of the key brings it back.
            EntityDespawn::EVENT_ID => {
                // Vector 18's top-level form is the one zero byte the envelope has taken already;
                // a batch's child keeps its own.
                if !data.is_empty() && EntityDespawn::from_bytes(data).is_err() {
                    return self.pending_unread();
                }
                if target == DEFAULT_RECIPIENT && !DEFAULT_RECIPIENT_DESPAWN_APPLIES {
                    self.entry_progress.default_target =
                        self.entry_progress.default_target.saturating_add(1);
                    return;
                }
                // §9.3.9 and Vector 24: a `0x40` whose target resolves to the announced `0x90` ID,
                // explicit-recipient form included, is required-progress Unsupported. It deletes
                // nothing: the local pending snapshot stays until the teardown. This always comes
                // after `Connected`, so `end_by_table` gives Disconnected Unsupported alone.
                if self.pending.player() == Some(target) {
                    self.end_by_table(
                        TableRow::Unsupported,
                        true,
                        "despawn of the local player before entry",
                        now,
                    );
                    return;
                }
                // §9.3.9, Vector 24: only an explicit, resolved target records a despawn; an explicit
                // `0x40` to an unannounced nonlocal ID deletes every pending datum kept under that
                // raw key, takes no tombstone and is counted and skipped, so a later `0x98` starts
                // clean. Labelled choice (C8, pending writer answer to remediation §32's note on a
                // `0x40` nested in `0x4C`): a batch's `0x40` takes this same road.
                if !self.pending.despawn_applies(target) {
                    if self.pending.forget_unannounced(target) {
                        self.entry_progress.unannounced_cleared =
                            self.entry_progress.unannounced_cleared.saturating_add(1);
                    }
                    self.entry_progress.no_target = self.entry_progress.no_target.saturating_add(1);
                    return;
                }
                let stored = self.pending.despawn(target);
                self.entry_progress.despawns += u64::from(stored == Stored::Kept);
                (target, stored)
            }
            SubEventBatch::EVENT_ID_AOI_ENTER => {
                self.entry_progress.unsupported_batches =
                    self.entry_progress.unsupported_batches.saturating_add(1);
                return;
            }
            // §9.3.9: `0xA8` counts and skips without pending storage until its variant is settled.
            0xA8 => {
                self.entry_progress.actions = self.entry_progress.actions.saturating_add(1);
                return;
            }
            0x7C | 0xC7 | 0x5E | 0x153 => {
                self.entry_progress.readiness_seen =
                    self.entry_progress.readiness_seen.saturating_add(1);
                if self.server_gate == Some(opcode) {
                    self.server_ready = true;
                }
                return;
            }
            // Every server `0x30`, `0x60` and `0x19A` before `WorldEntered` too: counted and discarded
            // (§9.3.9), no RPC runs and nothing is kept for later.
            _ => {
                self.entry_progress.other = self.entry_progress.other.saturating_add(1);
                return;
            }
        };
        self.note_stored(opcode, key, stored);
    }

    /// The counts of what the store did: never a terminal result, whatever it answered (§9.3.9).
    fn note_stored(&mut self, opcode: u32, key: u64, stored: Stored) {
        let p = &mut self.entry_progress;
        match stored {
            Stored::Kept if opcode == EntityDespawn::EVENT_ID => {}
            Stored::Kept if key == DEFAULT_RECIPIENT => {
                p.default_target = p.default_target.saturating_add(1);
            }
            Stored::Kept if !self.pending.resolved(key) && self.pending.player() != Some(key) => {
                p.no_target = p.no_target.saturating_add(1);
            }
            Stored::Kept => {}
            Stored::NoTarget if key == DEFAULT_RECIPIENT => {
                p.default_target = p.default_target.saturating_add(1);
            }
            Stored::NoTarget => p.no_target = p.no_target.saturating_add(1),
            Stored::Despawned => p.despawn_blocked = p.despawn_blocked.saturating_add(1),
            Stored::KeyCap => p.target_cap_hits = p.target_cap_hits.saturating_add(1),
            Stored::ByteCap => p.bytes_cap_hits = p.bytes_cap_hits.saturating_add(1),
        }
    }

    /// The first batch, creation, position and placement after the valid `0x90`: one line each, with
    /// the milliseconds since that `0x90`, so the order of these lines is the order of the events
    /// (`docs/m8-remediation.md` §27 "Builder"; the counts lines give neither). A placement here is a
    /// `0xCC` that read, kept or not: nothing says whose it is (OPEN-WORLD-9). No ID and no value.
    fn note_entry_first(&mut self, what: EntryFirst, stored: Stored, depth: usize, now: f64) {
        let Stage::Entering {
            wait: EntryWait::Readiness,
            since,
            ..
        } = &self.stage
        else {
            return;
        };
        if std::mem::replace(self.entry_firsts.of(what), true) {
            return;
        }
        evidence!(
            self.evidence_log,
            info,
            first = what.label(),
            since_session_start_ms = ((now - *since).max(0.0) * 1000.0) as u64,
            in_batch = depth > 0,
            kept = stored == Stored::Kept,
            "official evidence: first after the session start"
        );
    }

    /// A checked message that did not read: counted, nothing kept (§9.3.9).
    fn pending_unread(&mut self) {
        self.entry_progress.unread = self.entry_progress.unread.saturating_add(1);
    }

    /// A `0x4C` on the way to `WorldEntered`: its known children one by one, each with its own one
    /// shared byte. The `0x52` a `0x98` takes as its initial set goes to the created entity
    /// (`official-messages-world.md` §5.1). A batch that does not frame is counted.
    fn pending_batch(&mut self, opcode: u32, target: u64, data: &[u8], depth: usize, now: f64) {
        if depth >= MAX_BATCH_DEPTH {
            return self.pending_unread();
        }
        let Ok(batch) = SubEventBatch::from_bytes(data, opcode) else {
            return self.pending_unread();
        };
        self.entry_progress.batches = self.entry_progress.batches.saturating_add(1);
        self.note_entry_first(EntryFirst::Batch, Stored::Kept, depth, now);
        let pairs = pair_initial_properties(&batch.frames);
        for (i, frame) in batch.frames.iter().enumerate() {
            let SubEventFrame::Event {
                sub_event_id,
                payload,
            } = frame
            else {
                continue;
            };
            self.entry_progress.sub_events = self.entry_progress.sub_events.saturating_add(1);
            // The `0x98` this `0x52` belongs to, when one took it. One that did not decode made no
            // entity: its initial properties are nobody's, and never the batch's target.
            let paired = pairs
                .get(i)
                .copied()
                .flatten()
                .and_then(|c| batch.frames.get(c));
            let child_target = match paired {
                Some(SubEventFrame::Event { payload, .. }) => {
                    let Ok(create) = EntityCreate::from_bytes(payload) else {
                        self.entry_progress.orphaned_initial =
                            self.entry_progress.orphaned_initial.saturating_add(1);
                        continue;
                    };
                    create.entity_id
                }
                Some(SubEventFrame::Null { .. }) | None => target,
            };
            self.pending_message(*sub_event_id, child_target, payload, depth + 1, now);
            if self.ended {
                return;
            }
        }
    }

    /// A play message: in the world it goes to the play layer when it comes from the world
    /// connection, or from the login connection when there was no referral.
    fn play(&mut self, opcode: u32, recipient: u64, body: &[u8], world: bool) {
        if !world && self.world.is_some() {
            self.counters.unhandled += 1;
            tracing::trace!(
                opcode,
                "official session: login-connection message after the referral"
            );
            return;
        }
        if matches!(self.stage, Stage::InWorld { .. }) {
            self.play_in_world(opcode, recipient, body);
        } else {
            self.counters.unhandled += 1;
            tracing::trace!(
                opcode,
                "official session: message outside the world ignored"
            );
        }
    }

    /// `0xB3` (§9.3.9, Vector 15): valid means the first byte `00` and at least five bytes. The
    /// shared byte and the `u32` reason are read; what follows them is neither parsed nor shown. It
    /// ends the session in every stage with no closing reply: the row's refusal before `Connected`,
    /// `Disconnected { ServerClosed }` after it.
    fn eviction(&mut self, m: &Inbound, now: f64) {
        let reason = (m.envelope_flags == 0)
            .then(|| AccountEvictionNotice::reason_of_envelope_body(&m.body))
            .flatten();
        let Some(reason) = reason else {
            return self.refusal_unread(now);
        };
        evidence!(
            self.evidence_log,
            debug,
            direction = "recv",
            reason,
            "official session: the server ended the session"
        );
        self.end_by_table(
            TableRow::ServerRefused,
            false,
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
        if self.pending.holds(recipient) && self.hold_local(opcode, recipient, body) {
            return None;
        }
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
            self.log_message_refused(opcode, &e, "entity");
            self.log_skipped_property(&e, "entity");
        }
        self.feed_output(out);
        // §9.3.9: a validated `0x98` for the local player's ID permits its held properties and
        // motion, after the spawn ([`PendingWorld::take_local_hold`]).
        if let Some(CreateOutcome::Player(id)) = outcome
            && let Some(hold) = self.pending.take_local_hold(id)
        {
            for (op, body) in &hold.messages {
                self.world_message(*op, id, body, depth);
                if !self.in_world_still() {
                    break;
                }
            }
        }
        outcome
    }

    /// §9.3.9: the `0x90`'s ID with no validated `0x98` gets no update before one. A validated in-world
    /// `0x52`, entity `0x88`/`0x92` or removal, `0x89`, `0xCE` or `0xCC` keyed to it (its RAW logical
    /// recipient, as on the way to `WorldEntered`) merges into the local hold, so the `0x98` replays the
    /// LATEST state after the spawn, never an older one over newer live state. Whether the message was
    /// taken; one that does not read goes the ordinary way and is counted there.
    fn hold_local(&mut self, opcode: u32, recipient: u64, body: &[u8]) -> bool {
        let stored = match opcode {
            PropertyTableUpdate::EVENT_ID_ENTITY => {
                let Ok(entries) = PropertyTableUpdate::entry_spans(body, &self.schema) else {
                    return false;
                };
                self.pending
                    .hold_update(recipient, HeldUpdate::Table(&entries))
            }
            op if SinglePropertyUpdate::scope_of(op) == Some(PropertyScope::Entity) => {
                let mut r = WireReader::new(body);
                let Ok(u) = SinglePropertyUpdate::decode(&mut r, op, &self.schema)
                    .and_then(|u| r.finish().map(|()| u))
                else {
                    return false;
                };
                self.pending.hold_update(
                    recipient,
                    HeldUpdate::Single {
                        opcode: op,
                        property: u.entry.pid,
                        body,
                    },
                )
            }
            op if PropertyRemoval::scope_of(op) == Some(PropertyScope::Entity) => {
                let mut r = WireReader::new(body);
                let Ok(u) =
                    PropertyRemoval::decode(&mut r, op).and_then(|u| r.finish().map(|()| u))
                else {
                    return false;
                };
                self.pending
                    .hold_update(recipient, HeldUpdate::Removal(u.property_id))
            }
            VisualDescriptionUpdate::EVENT_ID => {
                if VisualDescriptionUpdate::from_bytes(body).is_err() {
                    return false;
                }
                self.pending
                    .hold_update(recipient, HeldUpdate::Appearance(body))
            }
            PositionUpdate::EVENT_ID => {
                if PositionUpdate::from_bytes(body).is_err() {
                    return false;
                }
                self.pending
                    .hold_update(recipient, HeldUpdate::Position(body))
            }
            PortalPositionSync::EVENT_ID => {
                if PortalPositionSync::from_bytes(body, &self.schema).is_err() {
                    return false;
                }
                self.pending.hold_update(recipient, HeldUpdate::Sync(body))
            }
            _ => return false,
        };
        if stored.is_some() {
            self.counters.local_held = self.counters.local_held.saturating_add(1);
        }
        stored.is_some()
    }

    /// The opt-in `skipped_property_id` evidence of §9.3.11 ("Required live-log CONTENT"): when a
    /// table decode stopped at a property the schema does not know, its numeric key and type
    /// category, never its value or account data. The schema has no type for an unknown key, so its
    /// category is `not-in-schema` (labelled, Tessera's word). Other errors log nothing here. The lines
    /// are rate-bounded like the refusal lines, with the count of those left out.
    fn log_skipped_property(&mut self, error: &WorldError, what: &'static str) {
        if let WorldError::UnknownPropertyId(pid) = error
            && let Some(skipped) = self.property_log.allow(self.log_clock)
        {
            evidence!(
                self.evidence_log,
                debug,
                direction = "recv",
                what,
                skipped_property_id = format_args!("{pid:#x}"),
                skipped_property_type_category = "not-in-schema",
                skipped,
                "official evidence: property key skipped"
            );
        }
    }

    /// One bounded line for a complete message that did not decode: its opcode and the kind of error,
    /// never a value of it.
    fn log_message_refused(&mut self, opcode: u32, error: &impl std::fmt::Debug, what: &str) {
        if let Some(skipped) = self.message_log.allow(self.log_clock) {
            evidence!(self.evidence_log, debug,
                direction = "recv",
                what,
                opcode = format_args!("{opcode:#x}"),
                kind = %error_kind(error),
                skipped,
                "official session: message refused"
            );
        }
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
                self.log_message_refused(opcode, &e, "batch");
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
                self.log_message_refused(opcode, &e, "play");
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

    /// `0xF4` on queue 1 in its default-recipient form (§9.3.9, Vector 18), kept until an explicit
    /// cumulative Ack covering it or the close and sent again on a NAK (§9.3.9 "F4 retention", §5.3
    /// F4 exception): the implicit acknowledgement below a NAK's first number never frees it, and an
    /// inbound `OutOfOrderAck` names the server's own sequences, so it ends no retention (Vector 24).
    fn send_ready(link: &mut Link, codecs: &ChatCodecs, env: &[u8], now: f64) -> bool {
        let Some(c) = link.mux.connection_mut(link.conn) else {
            return false;
        };
        let (queue, group) = outbound_queue(OPCODE_PORTAL_READY_ACK, env);
        match c.send_blob_kept(
            queue,
            group,
            OPCODE_PORTAL_READY_ACK,
            env,
            now,
            codecs.outgoing.as_deref(),
        ) {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(error = %e, "official session: the ready message was not queued");
                false
            }
        }
    }

    /// Decision 0046 item 13 (`official-session.md` §9.3.12): once per login, at the first complete
    /// character list. An eligible candidate is queued on queue 1, group 0, to
    /// [`RESOURCE_REPORT_DESTINATION`], unfragmented with blob ID 0 under the current word, sequence
    /// and flow policy; its sequences are taken now and all its datagrams are queued now, so every
    /// fragment's first transmission is in an earlier datagram than any later `0x6F`. Nothing waits
    /// for its Ack, a response or `0x149`, and no timeout is added. Its records are kept until a
    /// cumulative Ack covers them or the login closes, resent on a NAK and never on a timer, also
    /// after the referral ([`SessionConnection::send_blob_watched`], [`SessionConnection::retire`]).
    /// An omission (preflight, encoding or enqueue), withholds the whole report under item 5, the
    /// login goes on, and a held selection goes at once ([`Self::send_entry`]). The log line names
    /// the outcome and a kind only, never a key, label, mask or DID; the evidence lines add the
    /// private sequence of every report-bearing datagram. A second call does nothing.
    fn resource_report(&mut self, now: f64) {
        if !matches!(self.resource_report, ReportState::Pending(_)) {
            return;
        }
        let ReportState::Pending(candidate) = std::mem::replace(
            &mut self.resource_report,
            ReportState::Decided(ReportStatus::Undecided),
        ) else {
            return;
        };
        let status = match candidate {
            Ok(data) => match self.send_resource_report(&data, now) {
                Some(sequences) => {
                    self.report_evidence.sent(&sequences);
                    ReportStatus::Sent
                }
                None => ReportStatus::Omitted(ReportOmission::Enqueue),
            },
            Err(kind) => ReportStatus::Omitted(kind),
        };
        self.resource_report = ReportState::Decided(status);
        let selection_held = self.report_evidence.selection_held();
        match status {
            ReportStatus::Sent => {
                self.counters.resource_reports_sent =
                    self.counters.resource_reports_sent.saturating_add(1);
                tracing::info!("official session: resource-set report sent");
                evidence!(
                    self.evidence_log,
                    info,
                    direction = "send",
                    event_kind = "resource-set report",
                    action = "sent",
                    raw_opcode = format_args!("{OPCODE_RESOURCE_SET_REPORT:#x}"),
                    first_of_opcode = true,
                    queue = QUEUE_UNRELIABLE,
                    sequence_group = 0,
                    recipient = "explicit protocol destination",
                    report_decision = decision_label(status),
                    report_reason = reason_label(status),
                    report_sequence = self.report_evidence.sequence().unwrap_or(0),
                    report_datagrams = self.report_evidence.sequences().len(),
                    report_position = self.report_evidence.position_label(status),
                    selection_held,
                    "official evidence: resource-set report"
                );
                // One record per report-bearing datagram, with its initial action `none`.
                for (datagram, &sequence) in self.report_evidence.sequences().iter().enumerate() {
                    evidence!(
                        self.evidence_log,
                        info,
                        direction = "send",
                        event_kind = "resource-set report",
                        report_sequence = sequence,
                        report_datagram = datagram,
                        report_ack_action = ack_action_label(None),
                        report_retained = true,
                        "official evidence: resource-set report datagram"
                    );
                }
            }
            ReportStatus::Omitted(kind) => {
                self.counters.resource_reports_omitted =
                    self.counters.resource_reports_omitted.saturating_add(1);
                tracing::info!(
                    reason = kind.label(),
                    "official session: resource-set report omitted"
                );
                evidence!(
                    self.evidence_log,
                    info,
                    direction = "send",
                    event_kind = "resource-set report",
                    action = "omitted",
                    report_decision = decision_label(status),
                    report_reason = reason_label(status),
                    report_omission = omission_label(status),
                    report_position = self.report_evidence.position_label(status),
                    selection_held,
                    "official evidence: resource-set report"
                );
                self.held_evidence(OPCODE_RESOURCE_SET_REPORT, kind.withholding_reason());
            }
            ReportStatus::Undecided => {}
        }
    }

    /// Queues `0x13D`'s complete data `data` on queue 1 of the login connection to
    /// [`RESOURCE_REPORT_DESTINATION`] in the explicit recipient form (§9.3.12), kept and watched
    /// ([`SessionConnection::send_blob_watched`]). The sequences of its datagrams, `None` when it was
    /// not queued (an enqueue omission).
    fn send_resource_report(&mut self, data: &[u8], now: f64) -> Option<Vec<u32>> {
        let (queue, group) = outbound_queue(OPCODE_RESOURCE_SET_REPORT, &[]);
        let sent = self
            .login
            .mux
            .connection_mut(self.login.conn)
            .map(|connection| {
                connection.send_blob_watched(
                    queue,
                    group,
                    RESOURCE_REPORT_DESTINATION,
                    OPCODE_RESOURCE_SET_REPORT,
                    data,
                    now,
                    self.codecs.outgoing.as_deref(),
                )
            });
        match sent {
            Some(Ok(seqs)) if !seqs.is_empty() => Some(seqs),
            Some(Ok(_)) | None => None,
            Some(Err(e)) => {
                self.counters.send_failures += 1;
                tracing::debug!(error = %e, "official session: resource-set report not queued");
                None
            }
        }
    }

    /// Every transition of what the login connection's transport did with each report-bearing
    /// datagram since the last call (§9.3.11 item-13 row): one evidence line each, with its
    /// direction (`recv` for the peer's covering Ack, `send` for Tessera's NAK resend or
    /// OutOfOrderAck), the action before and after and whether the record is still retained. Not
    /// rate-limited: the session layer bounds the transitions by construction
    /// ([`SessionConnection::take_watched_transitions`]). Repeated NAK resends of one record are one
    /// transition; their count is in each summary line ([`Self::log_report_summary`]).
    fn log_report_fate(&mut self) {
        let Some(connection) = self.login.mux.connection_mut(self.login.conn) else {
            return;
        };
        let transitions = connection.take_watched_transitions();
        if !self.report_released && connection.watched_fate().is_some() {
            self.report_fate = (connection.watched_fate(), connection.watched_retained());
        }
        if !self.evidence_log {
            return;
        }
        for t in transitions {
            evidence!(
                self.evidence_log,
                info,
                direction = ack_action_direction(t.to),
                event_kind = "resource-set report",
                report_sequence = t.sequence,
                report_datagram = t.datagram,
                report_ack_from = ack_action_label(t.from),
                report_ack_action = ack_action_label(Some(t.to)),
                report_retained = t.retained,
                login_retiring = self.login_referred,
                "official evidence: resource-set report transport"
            );
        }
    }

    /// A `0x149` arrived on the login connection: the first one after the report is placed against
    /// the first `0x6F` (§9.3.12 "expected FIRST observable"), on one evidence line.
    fn note_login_149(&mut self) {
        if let Some(first) = self.report_evidence.note_149() {
            evidence!(
                self.evidence_log,
                info,
                direction = "recv",
                event_kind = "resource-set report",
                first_149_after_report = first.label(),
                "official evidence: first 0x149 after the resource-set report"
            );
        }
    }

    /// The report's summary line at `at`: once at the referral, once at the retiring login
    /// connection's own close and once at the session's close, so at most three per login. Each
    /// has the decision, its first sequence, the transport's latest action (`none` when the peer did
    /// nothing with it), the NAK resends and OutOfOrderAcks so far, how many of its datagrams were
    /// still retained (at a close: what that close released), its position against the first
    /// `0x6F`, whether a selection was held for it and where the first `0x149` after it arrived.
    /// Transitions between the summaries get their own lines.
    fn log_report_summary(&mut self, at: SummaryPoint) {
        self.log_report_fate();
        let (fate, retained) = self.report_fate;
        if matches!(at, SummaryPoint::LoginClose | SummaryPoint::Close) {
            // The close releases what was still retained; a later summary has none left.
            self.report_fate.1 = 0;
            self.report_released = true;
        }
        if !self.evidence_log || !self.report_evidence.take_summary(at) {
            return;
        }
        let status = self.resource_report.status();
        evidence!(
            self.evidence_log,
            info,
            at = at.label(),
            report_decision = decision_label(status),
            report_reason = reason_label(status),
            report_omission = omission_label(status),
            report_sequence = self.report_evidence.sequence().unwrap_or(0),
            report_datagrams = self.report_evidence.sequences().len(),
            report_ack_action = ack_action_label(fate.and_then(|f| f.last)),
            report_nak_resends = fate.map_or(0, |f| f.nak_resends),
            report_out_of_order_acks = fate.map_or(0, |f| f.out_of_order_acks),
            report_retained = retained,
            report_position = self.report_evidence.position_label(status),
            selection_held = self.report_evidence.selection_held(),
            first_149_after_report = self.report_evidence.first_149().label(),
            "official evidence: resource-set report summary"
        );
    }

    /// Sends `0x6F` on queue 2 of the login connection to [`SELECTION_DESTINATION`], in the
    /// explicit recipient form, with no extension word (§9.3.8, Vector 12). Its body is
    /// [`CharacterEnterWorldRequest`]'s with the one-byte empty client table of §9.3.9.
    fn send_entry(&mut self, env: &[u8], now: f64) -> bool {
        // §9.3.12: every report fragment's first transmission comes in an earlier datagram than
        // any `0x6F`. A selection follows `Connected`, which follows the report's decision; should
        // it ever not, the selection is held for the decision, which is made and queued first, here,
        // and released at once on any omission (preflight, encoding or enqueue).
        if matches!(self.resource_report, ReportState::Pending(_)) {
            self.report_evidence.hold_selection();
        }
        self.resource_report(now);
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
            Some(Ok(seqs)) => {
                if let Some(position) = self.report_evidence.selection_sent(&seqs) {
                    evidence!(
                        self.evidence_log,
                        info,
                        direction = "send",
                        event_kind = "resource-set report",
                        report_sequence = self
                            .report_evidence
                            .sequences()
                            .last()
                            .copied()
                            .unwrap_or(0),
                        report_position = position.label(),
                        selection_held = self.report_evidence.selection_held(),
                        "official evidence: resource-set report before the selection"
                    );
                }
                true
            }
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
            // §9.3.9: `0xF4` is one shared zero byte, serialized once. Everything else here has
            // no checked boundary and is only reached on the synthetic in-world path.
            let env = if opcode == OPCODE_PORTAL_READY_ACK {
                payload.clone()
            } else {
                encode_event_envelope(&payload)
            };
            let link = self.world.as_mut().unwrap_or(&mut self.login);
            let ok = if opcode == OPCODE_PORTAL_READY_ACK {
                Self::send_ready(link, &self.codecs, &env, now)
            } else {
                Self::send_on(link, &self.codecs, opcode, &payload, &env, now)
            };
            if !ok {
                self.counters.send_failures += 1;
            }
        }
    }

    fn links_mut(&mut self) -> Vec<&mut Link> {
        let mut links: Vec<&mut Link> = Vec::with_capacity(2);
        links.push(&mut self.login);
        if let Some(w) = self.world.as_mut() {
            links.push(w);
        }
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
    }

    /// Sends what the connections have queued; a failure is logged at most once per
    /// [`LOG_INTERVAL_SEC`] (a revoked right fails every send). A send of the primary connection that
    /// the socket refuses for good ([`SessionError::Io`]) is §9.3.9's I/O row, which the caller's
    /// [`Self::end_on_send_failure`] takes; the same on a retiring login connection decides nothing,
    /// and every other failure is only counted.
    fn flush(&mut self, now: f64) {
        let world_is_primary = self.world.is_some();
        let mut links: Vec<(bool, &mut Link)> = Vec::with_capacity(2);
        links.push((!world_is_primary, &mut self.login));
        if let Some(w) = self.world.as_mut() {
            links.push((true, w));
        }
        let mut failed = None;
        for (primary, l) in links {
            // The dump's tap is armed only around the flush of a connection whose first
            // datagram is still to be written ([`super::dump`]).
            let tap = self.dump.is_some() && l.wants_first_dump();
            if tap {
                self.transport.arm();
            }
            if let Err(e) = l.mux.flush_outbound(&mut self.transport) {
                self.send_failed |= primary && matches!(e, SessionError::Io(_));
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

    /// §9.3.9's I/O row after a failed send ([`Self::flush`]): `TransportFailed`, no Disconnect on any
    /// connection and every retry gone, with `LoginRefused { ServiceUnavailable }` before `Connected`.
    fn end_on_send_failure(&mut self, now: f64) {
        if std::mem::take(&mut self.send_failed) {
            self.end_by_table(TableRow::Io, false, "a send failed", now);
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
                // §9.3.9, OPEN-SESSION-16: a re-entry after a teardown has no settled recipient,
                // queue or body: no `0x6F`. The connector refuses it before it gets here
                // ([`Self::re_entry_blocked`]); one already on its way gets the session back to the
                // list, and the session goes on.
                if self.re_entry_blocked() {
                    self.counters.entries_not_sent =
                        self.counters.entries_not_sent.saturating_add(1);
                    self.events.push_back(Event::EnterWorldFailed {
                        character: name,
                        reason: ENTRY_REENTRY_NOT_SUPPORTED.to_owned(),
                    });
                    return Err(NetError::InvalidCommand(ENTRY_REENTRY_NOT_SUPPORTED));
                }
                let sent = CharacterEnterWorldRequest::new(id)
                    .to_envelope_bytes()
                    .is_ok_and(|env| self.send_entry(&env, now));
                if !sent {
                    self.counters.entries_not_sent =
                        self.counters.entries_not_sent.saturating_add(1);
                    self.end_by_table(TableRow::Unsupported, true, "selection not sent", now);
                    return Ok(());
                }
                self.pending.discard();
                self.world_identity = None;
                self.placement = None;
                self.placement_generation = 0;
                self.local_ready = false;
                self.server_ready = false;
                self.entry_progress = EntryProgress::default();
                self.entry_firsts = EntryFirsts::default();
                self.placed_at = None;
                self.loaded_at = None;
                tracing::info!("official session: character selected; waiting for the referral");
                self.note_stage(EntryWait::Referral.label(), now);
                self.stage = Stage::Entering {
                    id,
                    name,
                    wait: EntryWait::Referral,
                    since: now,
                };
                Ok(())
            }
            // §9.3.12 (the logout policy after the held-command table), `official-entry.md` §4.6
            // ("InWorld typed /logout"), ARCHITECTURE.md §19.1: valid only in the world. It closes `ClientRequested` with one Disconnect on the active world
            // connection, no logout RPC and no drain; the client goes to the account form. While
            // entering it is a local `InvalidCommand` (the entering screen's Cancel is the way out).
            Command::LeaveWorld => {
                if !matches!(self.stage, Stage::InWorld { .. }) {
                    return Err(NetError::InvalidCommand("not in the world"));
                }
                tracing::info!("official session: typed logout closes the session");
                self.log_entry_end("typed logout");
                self.pending.discard();
                self.pending_out.clear();
                self.user_close("typed logout", now);
                if let Some(chat) = self.chat.as_mut() {
                    chat.close(now);
                }
                self.end(DisconnectReason::ClientRequested);
                Ok(())
            }
            Command::WorldLoaded { generation } => {
                match &self.stage {
                    // ARCHITECTURE.md §19.1: while OPEN-WORLD-9 is open the official in-world
                    // placement and `WorldLoaded` path is held: a local `InvalidCommand`, counted,
                    // with no system line (§9.3.12 held-command table, the count-only `WorldLoaded`
                    // row) and nothing sent. A test's synthetic in-world fixture still reaches the
                    // arm below.
                    Stage::InWorld { .. } if !self.readiness_specified => {
                        self.counters.stray_loaded += 1;
                        return Err(NetError::InvalidCommand(HELD_COMMAND));
                    }
                    // §4.5 item 4: the client's loading for its latest placement is done. It is
                    // kept with that placement's stamp, also outside portal space: a `0x5E`
                    // may follow its placement (item 2; module docs). The in-world signal names no
                    // generation (labelled, `docs/m8-remediation.md` §28 fix 27).
                    Stage::InWorld { .. } => {
                        let t = self.adapter.teleport();
                        self.loaded = Some(t.stamp_known().then(|| t.stamp()));
                    }
                    // §9.3.11: the local gate of the pending placement it names; no server readiness.
                    Stage::Entering { since, .. }
                        if self.placement.is_some()
                            && !self.local_ready
                            && *generation == Some(self.placement_generation) =>
                    {
                        let waited = now - *since;
                        self.local_ready = true;
                        self.entry_progress.loaded = true;
                        self.loaded_at = Some(now);
                        tracing::info!("official world entry: the client loaded the placement");
                        // The load time against the readiness clock (§27 "Builder").
                        evidence!(
                            self.evidence_log,
                            info,
                            world_loaded_generation = self.placement_generation,
                            load_ms = self
                                .placed_at
                                .map_or(0, |placed| ((now - placed).max(0.0) * 1000.0) as u64),
                            readiness_clock_ms = (waited.max(0.0) * 1000.0) as u64,
                            limit_ms = (ENTER_WORLD_TIMEOUT_SEC * 1000.0) as u64,
                            "official evidence: client loading done"
                        );
                    }
                    // §9.3.11, §9.3.12's held-command table: stale, duplicate or before the
                    // placement. A local `InvalidCommand` with the capability text, counted, no gate
                    // change and nothing sent; no system line (the table's count-only `WorldLoaded`
                    // row).
                    _ => {
                        self.counters.stray_loaded += 1;
                        tracing::debug!("official session: loading signal with no placement");
                        return Err(NetError::InvalidCommand(HELD_COMMAND));
                    }
                }
                Ok(())
            }
            // §9.3.12's held-command table ("all official phases"): while OPEN-WORLD-9 is open a
            // move is held in every stage, before `0xF4` (decision 0046 item 10) and after it: a
            // local `InvalidCommand` with the capability text, the bounded plain system line (only
            // `WorldLoaded` is exempt from it) and nothing sent. The historical 38-byte form is not
            // sent in its place, and no history is kept for a reconciliation that cannot come. A
            // test's in-world fixture still reaches the arm below. The table gives no rate bound for
            // this line (question for the writer); the per-pass event cap bounds it.
            Command::Move { .. } if !self.readiness_specified => {
                self.counters.moves_held = self.counters.moves_held.saturating_add(1);
                self.held_line(
                    cmd,
                    OPCODE_MOVEMENT_STEP,
                    "decision 0046 item 10, OPEN-WORLD-9",
                );
                Err(NetError::InvalidCommand(MOVE_HELD))
            }
            // §9.3.12's held-command table ("all official phases"): every chat route (Say, Yell,
            // Emote, the dual routes and their aliases) is held while the current `0x30` send prefix
            // and source stay open (OPEN-SESSION-16), and the chat server is disabled, whether the
            // session is in the world, entering it or at the list: a local `InvalidCommand` with
            // the capability text, one plain system line and nothing sent, never a fallback route.
            // It comes before the outside-world arm below, so the text is the same in every stage.
            // A test's synthetic in-world fixture (`ConnectorNet::readiness_specified`) still
            // reaches the routes below.
            Command::Chat { .. } | Command::Emote { .. } if !self.readiness_specified => {
                self.held_line(cmd, OPCODE_ENTITY_METHOD_CALL, "OPEN-SESSION-16");
                Err(NetError::InvalidCommand(HELD_COMMAND))
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
                // Only a test's in-world fixture gets here (the held arm above).
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
                let Some(chat) = self.chat.as_mut() else {
                    // §9.3.12's held-command table: no secondary connection, no room operation.
                    self.held_line(cmd, 0, "ChatSession disabled");
                    return Err(NetError::InvalidCommand(CHAT_UNAVAILABLE));
                };
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
            // §9.3.12's held-command table: every other command (name check, creation, deletion,
            // interaction and the rest) is held: the capability text and nothing sent.
            other => {
                tracing::debug!(
                    command = other.name(),
                    "official session: command not carried"
                );
                Err(NetError::InvalidCommand(HELD_COMMAND))
            }
        }
    }

    /// A typed command of §9.3.12's held-command table refused here: counted, one plain system line
    /// with the capability text ([`HELD_COMMAND`], `official-entry.md` §8.7 "P10-10J display policy":
    /// the client shows it in the chat window's System lines or its System overlay), none once the
    /// driver holds its cap of events (module docs "Bounded events"), and nothing sent.
    fn held_line(&mut self, cmd: &Command, opcode: u32, reason: &'static str) {
        self.counters.commands_held = self.counters.commands_held.saturating_add(1);
        tracing::debug!(
            command = cmd.name(),
            "official session: held command refused, not sent"
        );
        self.held_evidence(opcode, reason);
        if self.events.len() < DRIVER_EVENT_CAP {
            self.events.push_back(Event::ServerNotice {
                level: NoticeLevel::Info,
                seconds: 0,
                text: HELD_COMMAND.to_owned(),
            });
        }
    }

    /// The evidence line of a held send (§9.3.11 "Required live-log CONTENT": `held_opcode`,
    /// `withholding_reason`): the first of each opcode only, so moves at their rate add one line.
    /// `opcode` 0 is a route with no opcode of its own (the disabled chat server).
    fn held_evidence(&mut self, opcode: u32, reason: &'static str) {
        if !self.evidence_log || !self.held_logged.insert(opcode) {
            return;
        }
        evidence!(
            self.evidence_log,
            info,
            direction = "send",
            event_kind = "held",
            action = "not sent",
            held_opcode = format_args!("{opcode:#x}"),
            first_of_opcode = true,
            withholding_reason = reason,
            "official evidence: held send"
        );
    }

    /// A user's close (§9.3.9): one Disconnect on the established primary connection. While the world
    /// connection is in its handshake the one Disconnect is the login connection's, and the world's
    /// auth retries end with none. That row names Cancel; Quit, Escape and window close take the same
    /// here (labelled, `docs/m8-remediation.md` §28 fix 15).
    fn user_close(&mut self, why: &str, now: f64) {
        let on_world = self.world.as_ref().is_some_and(|w| !w.in_handshake());
        self.close_links(on_world, why, now);
    }

    /// The user's close (§9.3.9; module docs "Ending the session"): one Disconnect by its rows, no
    /// application flush, no drain and a local close at once; nothing on a connection still in its
    /// handshake. No ticket refresh starts after this, and a running one is stopped.
    pub fn begin_shutdown(&mut self, now: f64) {
        self.log_report_summary(SummaryPoint::Close);
        self.log_login_end("cancelled", now);
        self.log_entry_end("cancelled");
        self.ended = true;
        self.shutting_down = true;
        // Nobody takes them any more; reading must go on for the chat client's answers.
        self.events.clear();
        self.inbox.clear();
        self.pending_out.clear();
        self.pending.discard();
        self.stop_refresh();
        self.log_abandoned_handshakes("cancelled", now);
        self.user_close("the user closed the session", now);
        if let Some(chat) = self.chat.as_mut() {
            chat.close(now);
        }
    }

    /// Ends the session without sending anything (the network right was revoked: nothing
    /// could be sent): every connection is closed locally and the chat client dropped. A
    /// connection still in its handshake logs its counts once, at `now`.
    pub fn abandon(&mut self, now: f64) {
        self.log_report_summary(SummaryPoint::Close);
        self.log_login_end("network right revoked", now);
        self.log_entry_end("network right revoked");
        self.ended = true;
        self.shutting_down = true;
        self.events.clear();
        self.inbox.clear();
        self.pending_out.clear();
        self.pending.discard();
        self.stop_refresh();
        self.log_abandoned_handshakes("network right revoked", now);
        for l in self.links_mut() {
            l.close_quietly();
        }
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
        self.login.flushing() || self.world.as_ref().is_some_and(Link::flushing)
    }

    /// Puts decision 0046 item 13's report back before its decision with `candidate`, for a test of
    /// the selection's hold (§9.3.12): a production driver decides it at the list.
    #[cfg(test)]
    pub fn pend_report_for_tests(&mut self, candidate: Result<Vec<u8>, ReportOmission>) {
        self.resource_report = ReportState::Pending(candidate);
        self.report_evidence = ReportEvidence::default();
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
    /// ([`SessionMultiplexer::next_deadline`]), the entry deadline ([`ENTER_WORLD_TIMEOUT_SEC`]), the next
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
        let links = std::iter::once(&self.login).chain(self.world.iter());
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

#[cfg(test)]
mod list_tests {
    use super::{MAX_CHARACTERS, listed_names};

    fn names(entries: &[(u64, &str)]) -> Option<Vec<(u64, String)>> {
        listed_names(entries.iter().copied())
    }

    /// §9.3.9's first-screen contract: every row either passes as decoded or fails the whole list, so
    /// no row disappears silently and none is cleaned, and the order is the server's.
    #[test]
    fn a_list_passes_whole_or_fails_whole() {
        let ok = names(&[(2, "Zeta"), (1, "Alpha")]).expect("valid");
        assert_eq!(ok, [(2, "Zeta".to_owned()), (1, "Alpha".to_owned())]);
        assert_eq!(names(&[]), Some(Vec::new()), "an empty list is valid");
        let long = "x".repeat(65);
        for bad in [
            vec![(1, "Good"), (2, "")],
            vec![(1, "Good"), (2, " \t ")],
            vec![(1, "Good"), (2, long.as_str())],
            vec![(1, "Good"), (1, "Other")],
            vec![(1, "Good"), (2, "Good")],
        ] {
            assert_eq!(names(&bad), None, "{bad:?}");
        }
        // §9.3.9, OE-T20: nothing is cleaned into a valid name, and names compare as decoded.
        assert_eq!(names(&[(1, "Go\u{7}od")]), None);
        assert_eq!(names(&[(1, "Good"), (2, "Ba\u{85}d")]), None);
        assert_eq!(names(&[(1, "\u{3000}\u{a0}")]), None, "only whitespace");
        assert!(
            names(&[(1, "Good"), (2, "good")]).is_some(),
            "case-sensitive"
        );
        assert!(
            names(&[(1, "\u{e9}"), (2, "e\u{301}")]).is_some(),
            "no normalization"
        );
        let exactly = "x".repeat(64);
        assert!(names(&[(1, exactly.as_str())]).is_some(), "64 bytes");
        assert_eq!(names(&[(1, &"\u{e9}".repeat(33))]), None, "66 UTF-8 bytes");
        // Exactly the bound passes, one more fails.
        let many: Vec<(u64, String)> = (0..=MAX_CHARACTERS as u64)
            .map(|i| (i + 1, format!("N{i}")))
            .collect();
        let refs: Vec<(u64, &str)> = many.iter().map(|(i, n)| (*i, n.as_str())).collect();
        assert!(names(&refs[..MAX_CHARACTERS]).is_some());
        assert_eq!(names(&refs), None);
    }
}
