// SPDX-License-Identifier: GPL-3.0-or-later
//! The message catalogue of `tessera-protocol.md` §6 as Rust types, with strict body
//! encoding and decoding (§3, §5).

use std::fmt;

use crate::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode, TeleportRefusedCode,
};
use crate::error::{ProtocolError, Result};
use crate::limits::MINOR_LOOKS;
use crate::limits::{
    MAX_ACCOUNT_LEN, MAX_CHARACTERS, MAX_CLOCK_RATE, MAX_DETAIL_TEXT_LEN, MAX_INTEREST_RADIUS,
    MAX_LONG_TEXT_LEN, MAX_MOTION_ENTRIES, MAX_NAME_LEN, MAX_PASSWORD_LEN, MAX_SOFTWARE_NAME_LEN,
    MAX_TEXT_LEN, MAX_TICK_RATE, MIN_MAX_ENTITIES, MIN_TICK_RATE,
};
use crate::types::{
    Appearance, CharacterSummary, Destination, Location, Look, Motion, NAME_S2C, Pose, Vec3,
};
use crate::value::{PropListRule, Property, Stance, decode_prop_list, encode_prop_list};
use crate::wire::{Reader, TextRule, Writer};

/// Which endpoint sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The Tessera client.
    Client,
    /// The Tessera server.
    Server,
}

impl Side {
    /// The other side.
    #[must_use]
    pub fn peer(self) -> Self {
        match self {
            Self::Client => Self::Server,
            Self::Server => Self::Client,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Client => "client",
            Self::Server => "server",
        })
    }
}

/// The four channels of §2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    /// The client-opened bidirectional control stream (types 0x0001–0x00FF).
    Control,
    /// The server's world stream, kind byte 0x57 (types 0x0101–0x01FF).
    World,
    /// The server's chat stream, kind byte 0x43 (types 0x0201–0x02FF).
    Chat,
    /// RFC 9221 datagrams (types 0x0301–0x03FF).
    Datagram,
}

impl Channel {
    /// The channel whose type range holds `ty`, if any.
    #[must_use]
    pub fn for_type(ty: u16) -> Option<Self> {
        match ty {
            0x0001..=0x00FF => Some(Self::Control),
            0x0101..=0x01FF => Some(Self::World),
            0x0201..=0x02FF => Some(Self::Chat),
            0x0301..=0x03FF => Some(Self::Datagram),
            _ => None,
        }
    }

    /// Whether this channel carries length-prefixed frames (every channel but datagrams).
    #[must_use]
    pub fn is_stream(self) -> bool {
        !matches!(self, Self::Datagram)
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Control => "control",
            Self::World => "world",
            Self::Chat => "chat",
            Self::Datagram => "datagram",
        })
    }
}

/// Who may send a message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Client to server only.
    ClientToServer,
    /// Server to client only.
    ServerToClient,
    /// Either side.
    Both,
}

impl Direction {
    /// Whether `sender` may send in this direction.
    #[must_use]
    pub fn allows(self, sender: Side) -> bool {
        match self {
            Self::ClientToServer => sender == Side::Client,
            Self::ServerToClient => sender == Side::Server,
            Self::Both => true,
        }
    }
}

macro_rules! message_types {
    ( $( $variant:ident = $code:expr, $dir:ident; )* ) => {
        /// Every message type of v1.0 (§6.1).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(missing_docs)]
        pub enum MessageType {
            $( $variant, )*
        }

        impl MessageType {
            /// All message types, in code order.
            pub const ALL: &'static [MessageType] = &[ $( MessageType::$variant, )* ];

            /// The type for a wire code, if v1.0 defines it.
            #[must_use]
            pub fn from_code(code: u16) -> Option<Self> {
                match code {
                    $( $code => Some(Self::$variant), )*
                    _ => None,
                }
            }

            /// The wire code.
            #[must_use]
            pub fn code(self) -> u16 {
                match self {
                    $( Self::$variant => $code, )*
                }
            }

            /// The spec's name.
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $( Self::$variant => stringify!($variant), )*
                }
            }

            /// Who may send it.
            #[must_use]
            pub fn direction(self) -> Direction {
                match self {
                    $( Self::$variant => Direction::$dir, )*
                }
            }
        }
    };
}

message_types! {
    Hello = 0x0001, ClientToServer;
    HelloAck = 0x0002, ServerToClient;
    Login = 0x0003, ClientToServer;
    Register = 0x0004, ClientToServer;
    LoginOk = 0x0005, ServerToClient;
    LoginFailed = 0x0006, ServerToClient;
    CharacterCreate = 0x0007, ClientToServer;
    CharacterList = 0x0008, ServerToClient;
    CharacterCreateFailed = 0x0009, ServerToClient;
    EnterWorld = 0x000A, ClientToServer;
    EnterWorldFailed = 0x000B, ServerToClient;
    TeleportRequest = 0x000C, ClientToServer;
    QueryLocation = 0x000D, ClientToServer;
    ChatSend = 0x000E, ClientToServer;
    EmoteSet = 0x000F, ClientToServer;
    StanceSet = 0x0010, ClientToServer;
    AppearanceSet = 0x0011, ClientToServer;
    Ping = 0x0012, Both;
    Pong = 0x0013, Both;
    Disconnect = 0x0014, Both;
    WorldEnter = 0x0101, ServerToClient;
    LocationReport = 0x0102, ServerToClient;
    TeleportRefused = 0x0103, ServerToClient;
    EntityCreate = 0x0104, ServerToClient;
    EntityUpdate = 0x0105, ServerToClient;
    EntityAppearance = 0x0106, ServerToClient;
    EntityDestroy = 0x0107, ServerToClient;
    InterestReset = 0x0108, ServerToClient;
    WorldClock = 0x0109, ServerToClient;
    ChatMessage = 0x0201, ServerToClient;
    ServerNotice = 0x0202, ServerToClient;
    ChatRejected = 0x0203, ServerToClient;
    MoveInput = 0x0301, ClientToServer;
    MotionBatch = 0x0302, ServerToClient;
    TimeRequest = 0x0303, ClientToServer;
    TimeReply = 0x0304, ServerToClient;
}

impl MessageType {
    /// The channel that carries it.
    #[must_use]
    pub fn channel(self) -> Channel {
        // Every defined code lies in a channel range.
        Channel::for_type(self.code()).unwrap_or(Channel::Control)
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// --- Text rules per field (§5.2) ---

const CLIENT_NAME: TextRule = TextRule::sanitised("client_name", 0, MAX_SOFTWARE_NAME_LEN);
const SERVER_NAME: TextRule = TextRule::sanitised("server_name", 0, MAX_SOFTWARE_NAME_LEN);
const MOTD: TextRule = TextRule::sanitised("motd", 0, MAX_LONG_TEXT_LEN);
const ACCOUNT_C2S: TextRule = TextRule::raw("account", MAX_ACCOUNT_LEN);
const PASSWORD_C2S: TextRule = TextRule::raw("password", MAX_PASSWORD_LEN);
const ACCOUNT_S2C: TextRule = TextRule::plain("account", 1, MAX_ACCOUNT_LEN);
const DETAIL: TextRule = TextRule::sanitised("text", 0, MAX_DETAIL_TEXT_LEN);
const NAME_C2S: TextRule = TextRule::raw("name", MAX_NAME_LEN);
const PEER_C2S: TextRule = TextRule::raw("peer", MAX_NAME_LEN);
const TEXT_C2S: TextRule = TextRule::raw("text", MAX_TEXT_LEN);
const DISCONNECT_TEXT: TextRule = TextRule::sanitised("text", 0, MAX_LONG_TEXT_LEN);
const NAME0_S2C: TextRule = TextRule::sanitised("name", 0, MAX_NAME_LEN);
const SENDER_S2C: TextRule = TextRule::sanitised("sender", 0, MAX_NAME_LEN);
const PEER_S2C: TextRule = TextRule::sanitised("peer", 1, MAX_NAME_LEN);
const TEXT_S2C: TextRule = TextRule::sanitised("text", 1, MAX_TEXT_LEN);

// --- Message bodies ---

/// `Hello` (0x0001, C→S, §6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// Highest minor version the client supports.
    pub minor_max: u16,
    /// Capability bits; none are defined in v1.0.
    pub client_caps: u32,
    /// The most entities the client accepts at once, 64–65,535.
    pub max_entities: u16,
    /// Software name and version, for logs (`TEXT0`(64)).
    pub client_name: String,
}

/// Server capability bits of `HelloAck` (§6.2.2). Unknown bits are kept and ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ServerCaps(pub u32);

impl ServerCaps {
    /// Bit 0: `Register` accepted.
    pub const REGISTRATION: Self = Self(1);
    /// Bit 1: `CharacterCreate` accepted.
    pub const CHARACTER_CREATION: Self = Self(2);
    /// Bit 2: `AppearanceSet` accepted.
    pub const APPEARANCE_CHANGE: Self = Self(4);
    /// No capabilities.
    pub const NONE: Self = Self(0);

    /// Whether every bit of `other` is set.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for ServerCaps {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// `HelloAck` (0x0002, S→C, §6.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloAck {
    /// Negotiated minor version.
    pub minor: u16,
    /// Capabilities.
    pub server_caps: ServerCaps,
    /// Simulation rate, 1–120 Hz.
    pub tick_rate: u8,
    /// `MotionBatch` rate, 1 to `tick_rate` Hz.
    pub snapshot_rate: u8,
    /// Interest radius *R* in landblocks, 0–8.
    pub interest_radius: u8,
    /// The operator's display name (`TEXT0`(64)).
    pub server_name: String,
    /// Message of the day (`TEXT0`(1,024)).
    pub motd: String,
}

/// The body of `Login` (0x0003) and `Register` (0x0004), C→S (§6.2.3).
///
/// `Debug` never prints the password.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// Account name (`ACCOUNT`; content checked by the server).
    pub account: String,
    /// Password (`PASSWORD`), exactly as typed.
    pub password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("account", &self.account)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// `LoginOk` (0x0005, S→C, §6.2.5).
#[derive(Debug, Clone, PartialEq)]
pub struct LoginOk {
    /// The account's canonical name.
    pub account: String,
    /// 0–64 characters, names unique.
    pub characters: Vec<CharacterSummary>,
    /// Minor ≥ 1 only: entry *i* is the look of character *i*. Sent as empty looks when this
    /// list is empty; otherwise it must have as many entries as `characters`. Not sent at
    /// minor 0 (and empty after decoding there).
    pub looks: Vec<Look>,
}

/// `LoginFailed` (0x0006, S→C, §6.2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginFailed {
    /// Why.
    pub code: LoginFailedCode,
    /// Seconds to wait, with `RATE_LIMITED`.
    pub retry_after: u16,
    /// Optional detail (`TEXT0`(256)).
    pub text: String,
}

/// `CharacterCreate` (0x0007, C→S, §6.2.7).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreate {
    /// The new character's name (`NAME`; content checked by the server).
    pub name: String,
    /// Its appearance.
    pub appearance: Appearance,
    /// Minor ≥ 1 only: its creation choices (empty: none). Not sent at minor 0.
    pub look: Look,
}

/// `CharacterList` (0x0008, S→C, §6.2.8).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterList {
    /// The account's full list, 0–64 entries.
    pub characters: Vec<CharacterSummary>,
    /// Minor ≥ 1 only: as [`LoginOk::looks`].
    pub looks: Vec<Look>,
}

/// A failure answer with a code and a detail text: `CharacterCreateFailed`,
/// `EnterWorldFailed`, `ChatRejected` (§6.2.9, §6.2.11, §6.4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure<C> {
    /// Why.
    pub code: C,
    /// Optional detail (`TEXT0`(256)).
    pub text: String,
}

/// `EnterWorld` (0x000A, C→S, §6.2.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterWorld {
    /// One of the account's characters.
    pub name: String,
}

/// `TeleportRequest` (0x000C, C→S, §6.2.12).
#[derive(Debug, Clone, PartialEq)]
pub struct TeleportRequest {
    /// Non-zero, increasing.
    pub request_id: u32,
    /// Where to.
    pub destination: Destination,
}

/// A chat channel (§10.1). `Tell` carries the other party's name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChatChannel {
    /// 1.
    Say,
    /// 2.
    Yell,
    /// 3.
    Emote,
    /// 4: the recipient (`ChatSend`), or the other party (`ChatMessage`).
    Tell {
        /// The other party.
        peer: String,
    },
    /// 5: server only.
    System,
}

impl ChatChannel {
    /// The wire value.
    #[must_use]
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Say => 1,
            Self::Yell => 2,
            Self::Emote => 3,
            Self::Tell { .. } => 4,
            Self::System => 5,
        }
    }
}

/// `ChatSend` (0x000E, C→S, §6.2.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSend {
    /// Say, Yell, Emote or Tell (System is a protocol violation).
    pub channel: ChatChannel,
    /// The line (`TEXT`; content checked by the server).
    pub text: String,
}

/// `EmoteSet` (0x000F, C→S, §6.2.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmoteSet {
    /// Opaque animation key; 0 stops the current emote.
    pub anim: u32,
    /// Play until stopped.
    pub looping: bool,
}

/// `Disconnect` (0x0014, both directions, §6.2.19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disconnect {
    /// Why (§14.1).
    pub code: AppErrorCode,
    /// Human-readable reason (`TEXT0`(1,024)).
    pub text: String,
}

/// `WorldEnter` (0x0101, S→C, §6.3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldEnter {
    /// The player's entity ID, non-zero.
    pub player: u64,
    /// The character's name.
    pub name: String,
    /// World-instance number (informational).
    pub instance: u32,
    /// Initial position epoch, ≥ 1.
    pub epoch: u32,
    /// Server time of this state, µs.
    pub server_time: u64,
    /// Where the player is.
    pub pose: Pose,
    /// How the player looks.
    pub appearance: Appearance,
    /// Minor ≥ 1 only: the character's look (empty: none). Not sent at minor 0.
    pub look: Look,
}

/// Cause of a `LocationReport` (§6.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocationCause {
    /// 1: answers a teleport, or a teleport the server initiated (request ID 0).
    Teleported,
    /// 2: answers `QueryLocation`; informational.
    Requested,
    /// 3: a movement correction (request ID 0).
    Corrected,
}

/// `LocationReport` (0x0102, S→C, §6.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct LocationReport {
    /// Why.
    pub cause: LocationCause,
    /// The answered request, or 0.
    pub request_id: u32,
    /// New epoch (Teleported, Corrected) or the current one (Requested).
    pub epoch: u32,
    /// µs.
    pub server_time: u64,
    /// The player's pose.
    pub pose: Pose,
}

/// `TeleportRefused` (0x0103, S→C, §6.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeleportRefused {
    /// The refused request.
    pub request_id: u32,
    /// Why.
    pub code: TeleportRefusedCode,
    /// Detail (`TEXT0`(256)).
    pub text: String,
}

/// Kind of a replicated entity (§6.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    /// 0.
    Player,
    /// 1.
    NonPlayer,
    /// 2.
    Creature,
    /// 3.
    Object,
}

/// `EntityCreate` (0x0104, S→C, §6.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityCreate {
    /// Non-zero, not the player, not currently created.
    pub id: u64,
    /// Kind.
    pub kind: EntityKind,
    /// Display name (`NAME0`).
    pub name: String,
    /// DID of the entity's class record (opaque).
    pub class: Option<u32>,
    /// Server time of the state below, µs.
    pub server_time: u64,
    /// Where (the player's current region).
    pub pose: Pose,
    /// m/s.
    pub velocity: Vec3,
    /// Motion byte.
    pub motion: Motion,
    /// How it looks.
    pub appearance: Appearance,
    /// Full property state, no removals.
    pub properties: Vec<Property>,
    /// Minor ≥ 1 only: the entity's look (empty: none; always empty for entities that are
    /// not characters). Not sent at minor 0.
    pub look: Look,
}

/// `EntityUpdate` (0x0105, S→C, §6.3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityUpdate {
    /// A created entity, or the player.
    pub id: u64,
    /// 1–256 changes; `Value::None` removes a property.
    pub changes: Vec<Property>,
}

/// `EntityAppearance` (0x0106, S→C, §6.3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityAppearance {
    /// A created entity, or the player.
    pub id: u64,
    /// Full replacement.
    pub appearance: Appearance,
    /// Minor ≥ 1 only: the entity's look, also a full replacement (empty: none). Not sent at
    /// minor 0.
    pub look: Look,
}

/// Why an entity is destroyed (§6.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DestroyReason {
    /// 0.
    LeftInterest,
    /// 1: gone from the world.
    Removed,
}

/// `EntityDestroy` (0x0107, S→C, §6.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityDestroy {
    /// A created entity (never the player).
    pub id: u64,
    /// Why.
    pub reason: DestroyReason,
}

/// `WorldClock` (0x0109, S→C, §6.3.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldClock {
    /// µs: when the values below hold.
    pub server_time: u64,
    /// Game clock *G* in seconds, finite, ≥ 0.
    pub game_clock: f64,
    /// Game seconds per real second, 0–10,000.
    pub rate: f32,
}

/// `ChatMessage` (0x0201, S→C, §6.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// µs, when the server accepted the line.
    pub server_time: u64,
    /// Channel; `Tell` carries the other party.
    pub channel: ChatChannel,
    /// Flag bit 0: the receiver's own line coming back.
    pub echo: bool,
    /// The speaker's entity ID if created on this connection or the player, else 0.
    pub sender_entity: u64,
    /// The speaker's name; empty for System only.
    pub sender: String,
    /// The line (`TEXT`).
    pub text: String,
}

/// Level of a `ServerNotice` (§6.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeLevel {
    /// 0.
    Info,
    /// 1.
    Warning,
    /// 2: the server is stopping in `seconds`.
    Shutdown,
}

/// `ServerNotice` (0x0202, S→C, §6.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerNotice {
    /// Level.
    pub level: NoticeLevel,
    /// For Shutdown, seconds until the server stops; 0 otherwise.
    pub seconds: u16,
    /// The notice (`TEXT`).
    pub text: String,
}

/// `MoveInput` (0x0301, C→S datagram, §6.5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveInput {
    /// +1 per `MoveInput` (serial arithmetic).
    pub seq: u32,
    /// The client's current position epoch.
    pub epoch: u32,
    /// The player's position.
    pub location: Location,
    /// Radians.
    pub heading: f32,
    /// m/s.
    pub velocity: Vec3,
    /// Motion byte.
    pub motion: Motion,
}

/// One `MotionBatch` entry (37 bytes, §6.5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionEntry {
    /// The entity (never the receiving player).
    pub id: u64,
    /// f32 metres in the receiver's current region frame.
    pub position: Vec3,
    /// Radians.
    pub heading: f32,
    /// m/s.
    pub velocity: Vec3,
    /// Motion byte.
    pub motion: Motion,
}

/// `MotionBatch` (0x0302, S→C datagram, §6.5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct MotionBatch {
    /// µs: the snapshot tick's time.
    pub server_time: u64,
    /// 1–29 entries, each entity at most once.
    pub entries: Vec<MotionEntry>,
}

/// `TimeReply` (0x0304, S→C datagram, §6.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeReply {
    /// *t*₀ echoed.
    pub client_time: u64,
    /// *t*₁: server time on receipt, µs.
    pub server_receive: u64,
    /// *t*₂ ≥ *t*₁: server time on sending, µs.
    pub server_send: u64,
    /// The server tick in progress at *t*₂.
    pub tick: u64,
}

/// Every message of v1.0.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Message {
    Hello(Hello),
    HelloAck(HelloAck),
    Login(Credentials),
    Register(Credentials),
    LoginOk(LoginOk),
    LoginFailed(LoginFailed),
    CharacterCreate(CharacterCreate),
    CharacterList(CharacterList),
    CharacterCreateFailed(Failure<CharacterCreateFailedCode>),
    EnterWorld(EnterWorld),
    EnterWorldFailed(Failure<EnterWorldFailedCode>),
    TeleportRequest(TeleportRequest),
    /// `request_id`, non-zero.
    QueryLocation {
        request_id: u32,
    },
    ChatSend(ChatSend),
    EmoteSet(EmoteSet),
    StanceSet(Stance),
    AppearanceSet(Appearance),
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
    Disconnect(Disconnect),
    WorldEnter(WorldEnter),
    LocationReport(LocationReport),
    TeleportRefused(TeleportRefused),
    EntityCreate(EntityCreate),
    EntityUpdate(EntityUpdate),
    EntityAppearance(EntityAppearance),
    EntityDestroy(EntityDestroy),
    InterestReset,
    WorldClock(WorldClock),
    ChatMessage(ChatMessage),
    ServerNotice(ServerNotice),
    ChatRejected(Failure<ChatRejectedCode>),
    MoveInput(MoveInput),
    MotionBatch(MotionBatch),
    /// *t*₀: the client's monotonic clock, µs.
    TimeRequest {
        client_time: u64,
    },
    TimeReply(TimeReply),
}

impl Message {
    /// The message's type.
    #[must_use]
    pub fn message_type(&self) -> MessageType {
        use MessageType as T;
        match self {
            Self::Hello(_) => T::Hello,
            Self::HelloAck(_) => T::HelloAck,
            Self::Login(_) => T::Login,
            Self::Register(_) => T::Register,
            Self::LoginOk(_) => T::LoginOk,
            Self::LoginFailed(_) => T::LoginFailed,
            Self::CharacterCreate(_) => T::CharacterCreate,
            Self::CharacterList(_) => T::CharacterList,
            Self::CharacterCreateFailed(_) => T::CharacterCreateFailed,
            Self::EnterWorld(_) => T::EnterWorld,
            Self::EnterWorldFailed(_) => T::EnterWorldFailed,
            Self::TeleportRequest(_) => T::TeleportRequest,
            Self::QueryLocation { .. } => T::QueryLocation,
            Self::ChatSend(_) => T::ChatSend,
            Self::EmoteSet(_) => T::EmoteSet,
            Self::StanceSet(_) => T::StanceSet,
            Self::AppearanceSet(_) => T::AppearanceSet,
            Self::Ping { .. } => T::Ping,
            Self::Pong { .. } => T::Pong,
            Self::Disconnect(_) => T::Disconnect,
            Self::WorldEnter(_) => T::WorldEnter,
            Self::LocationReport(_) => T::LocationReport,
            Self::TeleportRefused(_) => T::TeleportRefused,
            Self::EntityCreate(_) => T::EntityCreate,
            Self::EntityUpdate(_) => T::EntityUpdate,
            Self::EntityAppearance(_) => T::EntityAppearance,
            Self::EntityDestroy(_) => T::EntityDestroy,
            Self::InterestReset => T::InterestReset,
            Self::WorldClock(_) => T::WorldClock,
            Self::ChatMessage(_) => T::ChatMessage,
            Self::ServerNotice(_) => T::ServerNotice,
            Self::ChatRejected(_) => T::ChatRejected,
            Self::MoveInput(_) => T::MoveInput,
            Self::MotionBatch(_) => T::MotionBatch,
            Self::TimeRequest { .. } => T::TimeRequest,
            Self::TimeReply(_) => T::TimeReply,
        }
    }

    /// The channel that carries the message.
    #[must_use]
    pub fn channel(&self) -> Channel {
        self.message_type().channel()
    }

    /// Appends the message body (no length, no type) to `out` in the v1.0 layout
    /// (negotiated minor 0), checking every wire rule. On error `out` is left as it was.
    pub fn encode_body(&self, out: &mut Vec<u8>) -> Result<()> {
        self.encode_body_minor(out, 0)
    }

    /// Appends the message body in the layout of negotiated minor `minor` (§3.1): at minor
    /// 0 looks and slots are left off (the v1.0 bytes exactly, §15 invariant 10); at minor
    /// ≥ 1 they are appended. On error `out` is left as it was.
    pub fn encode_body_minor(&self, out: &mut Vec<u8>, minor: u16) -> Result<()> {
        let start = out.len();
        let r = encode_body(self, &mut Writer::with_minor(out, minor));
        if r.is_err() {
            out.truncate(start);
        }
        r
    }

    /// Decodes a body of type `ty` strictly in the v1.0 layout (negotiated minor 0): every
    /// byte must be used (§3).
    pub fn decode_body(ty: MessageType, body: &[u8]) -> Result<Self> {
        Self::decode_body_minor(ty, body, 0)
    }

    /// Decodes a body of type `ty` strictly in the layout of negotiated minor `minor`
    /// (§3.1). `Hello` and `HelloAck` are frozen and decode the same at every minor.
    pub fn decode_body_minor(ty: MessageType, body: &[u8], minor: u16) -> Result<Self> {
        let mut r = Reader::with_minor(body, minor);
        let m = decode_body(ty, &mut r)?;
        r.finish()?;
        Ok(m)
    }
}

// --- helpers ---

/// Appends a look at minor ≥ 1; nothing at minor 0 (§3.1).
fn encode_look(w: &mut Writer<'_>, look: &Look) -> Result<()> {
    if w.minor() >= MINOR_LOOKS {
        look.encode(w)?;
    }
    Ok(())
}

/// Reads a look at minor ≥ 1; the empty look at minor 0.
fn decode_look(r: &mut Reader<'_>) -> Result<Look> {
    if r.minor() >= MINOR_LOOKS {
        Look::decode(r)
    } else {
        Ok(Look::none())
    }
}

/// Appends the `looks` list of `LoginOk` / `CharacterList` at minor ≥ 1: exactly
/// `characters` entries (all empty when `looks` is empty).
fn encode_looks(w: &mut Writer<'_>, characters: usize, looks: &[Look]) -> Result<()> {
    if w.minor() < MINOR_LOOKS {
        return Ok(());
    }
    if !looks.is_empty() && looks.len() != characters {
        return Err(count_err("looks", looks.len(), characters, characters));
    }
    // characters <= 64 (encode_characters checked it).
    w.u8(u8::try_from(characters).unwrap_or(u8::MAX));
    if looks.is_empty() {
        for _ in 0..characters {
            Look::none().encode(w)?;
        }
    } else {
        for l in looks {
            l.encode(w)?;
        }
    }
    Ok(())
}

/// Reads the `looks` list at minor ≥ 1: its count must equal `characters` (§13.1).
fn decode_looks(r: &mut Reader<'_>, characters: usize) -> Result<Vec<Look>> {
    if r.minor() < MINOR_LOOKS {
        return Ok(Vec::new());
    }
    let n = usize::from(r.u8()?);
    if n != characters {
        return Err(count_err("looks", n, characters, characters));
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(Look::decode(r)?);
    }
    Ok(out)
}

fn enum_err(field: &'static str, value: impl Into<u64>) -> ProtocolError {
    ProtocolError::InvalidEnum {
        field,
        value: value.into(),
    }
}

fn nonzero_u32(field: &'static str, v: u32) -> Result<u32> {
    if v == 0 {
        Err(ProtocolError::OutOfRange(field))
    } else {
        Ok(v)
    }
}

fn nonzero_u64(field: &'static str, v: u64) -> Result<u64> {
    if v == 0 {
        Err(ProtocolError::OutOfRange(field))
    } else {
        Ok(v)
    }
}

fn count_err(field: &'static str, count: usize, min: usize, max: usize) -> ProtocolError {
    ProtocolError::Count {
        field,
        count,
        min,
        max,
    }
}

fn encode_characters(w: &mut Writer<'_>, list: &[CharacterSummary]) -> Result<()> {
    if list.len() > MAX_CHARACTERS {
        return Err(count_err("characters", list.len(), 0, MAX_CHARACTERS));
    }
    check_unique_names(list)?;
    w.u8(u8::try_from(list.len()).unwrap_or(u8::MAX));
    for c in list {
        c.encode(w)?;
    }
    Ok(())
}

fn check_unique_names(list: &[CharacterSummary]) -> Result<()> {
    for (i, a) in list.iter().enumerate() {
        if list.iter().skip(i + 1).any(|b| b.name == a.name) {
            return Err(ProtocolError::Duplicate("characters"));
        }
    }
    Ok(())
}

fn decode_characters(r: &mut Reader<'_>) -> Result<Vec<CharacterSummary>> {
    let n = usize::from(r.u8()?);
    if n > MAX_CHARACTERS {
        return Err(count_err("characters", n, 0, MAX_CHARACTERS));
    }
    let mut list = Vec::with_capacity(n);
    for _ in 0..n {
        list.push(CharacterSummary::decode(r)?);
    }
    check_unique_names(&list)?;
    Ok(list)
}

fn check_hello(h: &Hello) -> Result<()> {
    if h.max_entities < MIN_MAX_ENTITIES {
        return Err(ProtocolError::OutOfRange("max_entities"));
    }
    Ok(())
}

fn check_hello_ack(a: &HelloAck) -> Result<()> {
    if !(MIN_TICK_RATE..=MAX_TICK_RATE).contains(&a.tick_rate) {
        return Err(ProtocolError::OutOfRange("tick_rate"));
    }
    if a.snapshot_rate < 1 || a.snapshot_rate > a.tick_rate {
        return Err(ProtocolError::OutOfRange("snapshot_rate"));
    }
    if a.interest_radius > MAX_INTEREST_RADIUS {
        return Err(ProtocolError::OutOfRange("interest_radius"));
    }
    Ok(())
}

fn check_world_clock(c: &WorldClock) -> Result<()> {
    if !c.game_clock.is_finite() {
        return Err(ProtocolError::NonFinite("game_clock"));
    }
    if c.game_clock < 0.0 {
        return Err(ProtocolError::OutOfRange("game_clock"));
    }
    if !c.rate.is_finite() {
        return Err(ProtocolError::NonFinite("rate"));
    }
    if !(0.0..=MAX_CLOCK_RATE).contains(&c.rate) {
        return Err(ProtocolError::OutOfRange("rate"));
    }
    Ok(())
}

fn check_chat_message(m: &ChatMessage) -> Result<()> {
    if m.channel == ChatChannel::System {
        if m.sender_entity != 0 {
            return Err(ProtocolError::OutOfRange("sender_entity"));
        }
        if !m.sender.is_empty() {
            return Err(ProtocolError::OutOfRange("sender"));
        }
    } else if m.sender.is_empty() {
        return Err(ProtocolError::OutOfRange("sender"));
    }
    Ok(())
}

fn check_motion_entries(entries: &[MotionEntry]) -> Result<()> {
    let n = entries.len();
    if !(1..=MAX_MOTION_ENTRIES).contains(&n) {
        return Err(count_err("entries", n, 1, MAX_MOTION_ENTRIES));
    }
    for (i, a) in entries.iter().enumerate() {
        if entries.iter().skip(i + 1).any(|b| b.id == a.id) {
            return Err(ProtocolError::Duplicate("motion entries"));
        }
    }
    Ok(())
}

fn location_cause_from(v: u8) -> Result<LocationCause> {
    match v {
        1 => Ok(LocationCause::Teleported),
        2 => Ok(LocationCause::Requested),
        3 => Ok(LocationCause::Corrected),
        v => Err(enum_err("cause", v)),
    }
}

fn location_cause_to(c: LocationCause) -> u8 {
    match c {
        LocationCause::Teleported => 1,
        LocationCause::Requested => 2,
        LocationCause::Corrected => 3,
    }
}

fn check_location_report(m: &LocationReport) -> Result<()> {
    if m.cause == LocationCause::Corrected && m.request_id != 0 {
        return Err(ProtocolError::OutOfRange("request_id"));
    }
    Ok(())
}

fn entity_kind_from(v: u8) -> Result<EntityKind> {
    match v {
        0 => Ok(EntityKind::Player),
        1 => Ok(EntityKind::NonPlayer),
        2 => Ok(EntityKind::Creature),
        3 => Ok(EntityKind::Object),
        v => Err(enum_err("kind", v)),
    }
}

fn entity_kind_to(k: EntityKind) -> u8 {
    match k {
        EntityKind::Player => 0,
        EntityKind::NonPlayer => 1,
        EntityKind::Creature => 2,
        EntityKind::Object => 3,
    }
}

const CREATE_PROPS: PropListRule = PropListRule {
    allow_none: false,
    min: 0,
};
const UPDATE_PROPS: PropListRule = PropListRule {
    allow_none: true,
    min: 1,
};

fn encode_body(m: &Message, w: &mut Writer<'_>) -> Result<()> {
    match m {
        Message::Hello(h) => {
            check_hello(h)?;
            w.u16(h.minor_max);
            w.u32(h.client_caps);
            w.u16(h.max_entities);
            w.str(CLIENT_NAME, &h.client_name)?;
        }
        Message::HelloAck(a) => {
            check_hello_ack(a)?;
            w.u16(a.minor);
            w.u32(a.server_caps.0);
            w.u8(a.tick_rate);
            w.u8(a.snapshot_rate);
            w.u8(a.interest_radius);
            w.str(SERVER_NAME, &a.server_name)?;
            w.str(MOTD, &a.motd)?;
        }
        Message::Login(c) | Message::Register(c) => {
            w.str(ACCOUNT_C2S, &c.account)?;
            w.str(PASSWORD_C2S, &c.password)?;
        }
        Message::LoginOk(ok) => {
            w.str(ACCOUNT_S2C, &ok.account)?;
            encode_characters(w, &ok.characters)?;
            encode_looks(w, ok.characters.len(), &ok.looks)?;
        }
        Message::LoginFailed(f) => {
            w.u16(f.code.to_u16());
            w.u16(f.retry_after);
            w.str(DETAIL, &f.text)?;
        }
        Message::CharacterCreate(c) => {
            w.str(NAME_C2S, &c.name)?;
            c.appearance.encode(w)?;
            encode_look(w, &c.look)?;
        }
        Message::CharacterList(l) => {
            encode_characters(w, &l.characters)?;
            encode_looks(w, l.characters.len(), &l.looks)?;
        }
        Message::CharacterCreateFailed(f) => {
            w.u16(f.code.to_u16());
            w.str(DETAIL, &f.text)?;
        }
        Message::EnterWorld(e) => w.str(NAME_C2S, &e.name)?,
        Message::EnterWorldFailed(f) => {
            w.u16(f.code.to_u16());
            w.str(DETAIL, &f.text)?;
        }
        Message::TeleportRequest(t) => {
            w.u32(nonzero_u32("request_id", t.request_id)?);
            t.destination.encode(w)?;
        }
        Message::QueryLocation { request_id } => {
            w.u32(nonzero_u32("request_id", *request_id)?);
        }
        Message::ChatSend(c) => {
            match &c.channel {
                ChatChannel::System => return Err(enum_err("channel", 5u8)),
                ChatChannel::Tell { peer } => {
                    w.u8(4);
                    w.str(PEER_C2S, peer)?;
                }
                other => w.u8(other.to_u8()),
            }
            w.str(TEXT_C2S, &c.text)?;
        }
        Message::EmoteSet(e) => {
            w.u32(e.anim);
            w.bool(e.looping);
        }
        Message::StanceSet(s) => w.u8(s.to_u8()),
        Message::AppearanceSet(a) => a.encode(w)?,
        Message::Ping { nonce } | Message::Pong { nonce } => w.u64(*nonce),
        Message::Disconnect(d) => {
            w.u16(d.code.to_u16());
            w.str(DISCONNECT_TEXT, &d.text)?;
        }
        Message::WorldEnter(e) => {
            w.u64(nonzero_u64("player", e.player)?);
            w.str(NAME_S2C, &e.name)?;
            w.u32(e.instance);
            w.u32(nonzero_u32("epoch", e.epoch)?);
            w.u64(e.server_time);
            e.pose.encode(w)?;
            e.appearance.encode(w)?;
            encode_look(w, &e.look)?;
        }
        Message::LocationReport(l) => {
            check_location_report(l)?;
            w.u8(location_cause_to(l.cause));
            w.u32(l.request_id);
            w.u32(l.epoch);
            w.u64(l.server_time);
            l.pose.encode(w)?;
        }
        Message::TeleportRefused(t) => {
            w.u32(t.request_id);
            w.u16(t.code.to_u16());
            w.str(DETAIL, &t.text)?;
        }
        Message::EntityCreate(c) => {
            w.u64(nonzero_u64("id", c.id)?);
            w.u8(entity_kind_to(c.kind));
            w.str(NAME0_S2C, &c.name)?;
            match c.class {
                None => w.u8(0),
                Some(d) => {
                    w.u8(1);
                    w.u32(d);
                }
            }
            w.u64(c.server_time);
            c.pose.encode(w)?;
            c.velocity.encode(w, "velocity")?;
            w.u8(c.motion.to_byte());
            c.appearance.encode(w)?;
            encode_prop_list(w, &c.properties, CREATE_PROPS)?;
            encode_look(w, &c.look)?;
        }
        Message::EntityUpdate(u) => {
            w.u64(u.id);
            encode_prop_list(w, &u.changes, UPDATE_PROPS)?;
        }
        Message::EntityAppearance(a) => {
            w.u64(a.id);
            a.appearance.encode(w)?;
            encode_look(w, &a.look)?;
        }
        Message::EntityDestroy(d) => {
            w.u64(d.id);
            w.u8(match d.reason {
                DestroyReason::LeftInterest => 0,
                DestroyReason::Removed => 1,
            });
        }
        Message::InterestReset => {}
        Message::WorldClock(c) => {
            check_world_clock(c)?;
            w.u64(c.server_time);
            w.f64("game_clock", c.game_clock)?;
            w.f32("rate", c.rate)?;
        }
        Message::ChatMessage(c) => {
            check_chat_message(c)?;
            w.u64(c.server_time);
            w.u8(c.channel.to_u8());
            w.u8(u8::from(c.echo));
            w.u64(c.sender_entity);
            w.str(SENDER_S2C, &c.sender)?;
            if let ChatChannel::Tell { peer } = &c.channel {
                w.str(PEER_S2C, peer)?;
            }
            w.str(TEXT_S2C, &c.text)?;
        }
        Message::ServerNotice(n) => {
            w.u8(match n.level {
                NoticeLevel::Info => 0,
                NoticeLevel::Warning => 1,
                NoticeLevel::Shutdown => 2,
            });
            w.u16(n.seconds);
            w.str(TEXT_S2C, &n.text)?;
        }
        Message::ChatRejected(f) => {
            w.u16(f.code.to_u16());
            w.str(DETAIL, &f.text)?;
        }
        Message::MoveInput(m) => {
            w.u32(m.seq);
            w.u32(m.epoch);
            m.location.encode(w)?;
            w.f32("heading", m.heading)?;
            m.velocity.encode(w, "velocity")?;
            w.u8(m.motion.to_byte());
        }
        Message::MotionBatch(b) => {
            check_motion_entries(&b.entries)?;
            w.u64(b.server_time);
            w.u8(u8::try_from(b.entries.len()).unwrap_or(u8::MAX));
            for e in &b.entries {
                w.u64(e.id);
                e.position.encode(w, "position")?;
                w.f32("heading", e.heading)?;
                e.velocity.encode(w, "velocity")?;
                w.u8(e.motion.to_byte());
            }
        }
        Message::TimeRequest { client_time } => w.u64(*client_time),
        Message::TimeReply(t) => {
            if t.server_send < t.server_receive {
                return Err(ProtocolError::OutOfRange("server_send"));
            }
            w.u64(t.client_time);
            w.u64(t.server_receive);
            w.u64(t.server_send);
            w.u64(t.tick);
        }
    }
    Ok(())
}

fn decode_body(ty: MessageType, r: &mut Reader<'_>) -> Result<Message> {
    use MessageType as T;
    Ok(match ty {
        T::Hello => {
            let h = Hello {
                minor_max: r.u16()?,
                client_caps: r.u32()?,
                max_entities: r.u16()?,
                client_name: r.str(CLIENT_NAME)?,
            };
            check_hello(&h)?;
            Message::Hello(h)
        }
        T::HelloAck => {
            let a = HelloAck {
                minor: r.u16()?,
                server_caps: ServerCaps(r.u32()?),
                tick_rate: r.u8()?,
                snapshot_rate: r.u8()?,
                interest_radius: r.u8()?,
                server_name: r.str(SERVER_NAME)?,
                motd: r.str(MOTD)?,
            };
            check_hello_ack(&a)?;
            Message::HelloAck(a)
        }
        T::Login | T::Register => {
            let c = Credentials {
                account: r.str(ACCOUNT_C2S)?,
                password: r.str(PASSWORD_C2S)?,
            };
            if ty == T::Login {
                Message::Login(c)
            } else {
                Message::Register(c)
            }
        }
        T::LoginOk => {
            let account = r.str(ACCOUNT_S2C)?;
            let characters = decode_characters(r)?;
            let looks = decode_looks(r, characters.len())?;
            Message::LoginOk(LoginOk {
                account,
                characters,
                looks,
            })
        }
        T::LoginFailed => Message::LoginFailed(LoginFailed {
            code: LoginFailedCode::from_u16(r.u16()?),
            retry_after: r.u16()?,
            text: r.str(DETAIL)?,
        }),
        T::CharacterCreate => Message::CharacterCreate(CharacterCreate {
            name: r.str(NAME_C2S)?,
            appearance: Appearance::decode(r)?,
            look: decode_look(r)?,
        }),
        T::CharacterList => {
            let characters = decode_characters(r)?;
            let looks = decode_looks(r, characters.len())?;
            Message::CharacterList(CharacterList { characters, looks })
        }
        T::CharacterCreateFailed => Message::CharacterCreateFailed(Failure {
            code: CharacterCreateFailedCode::from_u16(r.u16()?),
            text: r.str(DETAIL)?,
        }),
        T::EnterWorld => Message::EnterWorld(EnterWorld {
            name: r.str(NAME_C2S)?,
        }),
        T::EnterWorldFailed => Message::EnterWorldFailed(Failure {
            code: EnterWorldFailedCode::from_u16(r.u16()?),
            text: r.str(DETAIL)?,
        }),
        T::TeleportRequest => Message::TeleportRequest(TeleportRequest {
            request_id: nonzero_u32("request_id", r.u32()?)?,
            destination: Destination::decode(r)?,
        }),
        T::QueryLocation => Message::QueryLocation {
            request_id: nonzero_u32("request_id", r.u32()?)?,
        },
        T::ChatSend => {
            let channel = match r.u8()? {
                1 => ChatChannel::Say,
                2 => ChatChannel::Yell,
                3 => ChatChannel::Emote,
                4 => ChatChannel::Tell {
                    peer: r.str(PEER_C2S)?,
                },
                v => return Err(enum_err("channel", v)),
            };
            Message::ChatSend(ChatSend {
                channel,
                text: r.str(TEXT_C2S)?,
            })
        }
        T::EmoteSet => Message::EmoteSet(EmoteSet {
            anim: r.u32()?,
            looping: r.bool("looping")?,
        }),
        T::StanceSet => {
            let v = r.u8()?;
            Message::StanceSet(Stance::from_u8(v).ok_or_else(|| enum_err("stance", v))?)
        }
        T::AppearanceSet => Message::AppearanceSet(Appearance::decode(r)?),
        T::Ping => Message::Ping { nonce: r.u64()? },
        T::Pong => Message::Pong { nonce: r.u64()? },
        T::Disconnect => Message::Disconnect(Disconnect {
            code: AppErrorCode::from_u16(r.u16()?),
            text: r.str(DISCONNECT_TEXT)?,
        }),
        T::WorldEnter => Message::WorldEnter(WorldEnter {
            player: nonzero_u64("player", r.u64()?)?,
            name: r.str(NAME_S2C)?,
            instance: r.u32()?,
            epoch: nonzero_u32("epoch", r.u32()?)?,
            server_time: r.u64()?,
            pose: Pose::decode(r)?,
            appearance: Appearance::decode(r)?,
            look: decode_look(r)?,
        }),
        T::LocationReport => {
            let l = LocationReport {
                cause: location_cause_from(r.u8()?)?,
                request_id: r.u32()?,
                epoch: r.u32()?,
                server_time: r.u64()?,
                pose: Pose::decode(r)?,
            };
            check_location_report(&l)?;
            Message::LocationReport(l)
        }
        T::TeleportRefused => Message::TeleportRefused(TeleportRefused {
            request_id: r.u32()?,
            code: TeleportRefusedCode::from_u16(r.u16()?),
            text: r.str(DETAIL)?,
        }),
        T::EntityCreate => Message::EntityCreate(EntityCreate {
            id: nonzero_u64("id", r.u64()?)?,
            kind: entity_kind_from(r.u8()?)?,
            name: r.str(NAME0_S2C)?,
            class: if r.opt("class")? {
                Some(r.u32()?)
            } else {
                None
            },
            server_time: r.u64()?,
            pose: Pose::decode(r)?,
            velocity: Vec3::decode(r, "velocity")?,
            motion: Motion::from_byte(r.u8()?)?,
            appearance: Appearance::decode(r)?,
            properties: decode_prop_list(r, CREATE_PROPS)?,
            look: decode_look(r)?,
        }),
        T::EntityUpdate => Message::EntityUpdate(EntityUpdate {
            id: r.u64()?,
            changes: decode_prop_list(r, UPDATE_PROPS)?,
        }),
        T::EntityAppearance => Message::EntityAppearance(EntityAppearance {
            id: r.u64()?,
            appearance: Appearance::decode(r)?,
            look: decode_look(r)?,
        }),
        T::EntityDestroy => Message::EntityDestroy(EntityDestroy {
            id: r.u64()?,
            reason: match r.u8()? {
                0 => DestroyReason::LeftInterest,
                1 => DestroyReason::Removed,
                v => return Err(enum_err("reason", v)),
            },
        }),
        T::InterestReset => Message::InterestReset,
        T::WorldClock => {
            let c = WorldClock {
                server_time: r.u64()?,
                game_clock: r.f64("game_clock")?,
                rate: r.f32("rate")?,
            };
            check_world_clock(&c)?;
            Message::WorldClock(c)
        }
        T::ChatMessage => {
            let server_time = r.u64()?;
            let ch = r.u8()?;
            if !(1..=5).contains(&ch) {
                return Err(enum_err("channel", ch));
            }
            let flags = r.u8()?;
            if flags & 0xFE != 0 {
                return Err(ProtocolError::ReservedBits("flags"));
            }
            let sender_entity = r.u64()?;
            let sender = r.str(SENDER_S2C)?;
            let channel = match ch {
                1 => ChatChannel::Say,
                2 => ChatChannel::Yell,
                3 => ChatChannel::Emote,
                4 => ChatChannel::Tell {
                    peer: r.str(PEER_S2C)?,
                },
                _ => ChatChannel::System,
            };
            let m = ChatMessage {
                server_time,
                channel,
                echo: flags & 1 != 0,
                sender_entity,
                sender,
                text: r.str(TEXT_S2C)?,
            };
            check_chat_message(&m)?;
            Message::ChatMessage(m)
        }
        T::ServerNotice => Message::ServerNotice(ServerNotice {
            level: match r.u8()? {
                0 => NoticeLevel::Info,
                1 => NoticeLevel::Warning,
                2 => NoticeLevel::Shutdown,
                v => return Err(enum_err("level", v)),
            },
            seconds: r.u16()?,
            text: r.str(TEXT_S2C)?,
        }),
        T::ChatRejected => Message::ChatRejected(Failure {
            code: ChatRejectedCode::from_u16(r.u16()?),
            text: r.str(DETAIL)?,
        }),
        T::MoveInput => Message::MoveInput(MoveInput {
            seq: r.u32()?,
            epoch: r.u32()?,
            location: Location::decode(r)?,
            heading: r.f32("heading")?,
            velocity: Vec3::decode(r, "velocity")?,
            motion: Motion::from_byte(r.u8()?)?,
        }),
        T::MotionBatch => {
            let server_time = r.u64()?;
            let n = usize::from(r.u8()?);
            if !(1..=MAX_MOTION_ENTRIES).contains(&n) {
                return Err(count_err("entries", n, 1, MAX_MOTION_ENTRIES));
            }
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                entries.push(MotionEntry {
                    id: r.u64()?,
                    position: Vec3::decode(r, "position")?,
                    heading: r.f32("heading")?,
                    velocity: Vec3::decode(r, "velocity")?,
                    motion: Motion::from_byte(r.u8()?)?,
                });
            }
            check_motion_entries(&entries)?;
            Message::MotionBatch(MotionBatch {
                server_time,
                entries,
            })
        }
        T::TimeRequest => Message::TimeRequest {
            client_time: r.u64()?,
        },
        T::TimeReply => {
            let t = TimeReply {
                client_time: r.u64()?,
                server_receive: r.u64()?,
                server_send: r.u64()?,
                tick: r.u64()?,
            };
            if t.server_send < t.server_receive {
                return Err(ProtocolError::OutOfRange("server_send"));
            }
            Message::TimeReply(t)
        }
    })
}
