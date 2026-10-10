// SPDX-License-Identifier: GPL-3.0-or-later
//! Every limit, rate and default of `docs/specs/tessera-protocol.md`, as named constants.
//!
//! Section references point into that spec. Decoders check lengths and counts against these
//! values before allocating (§13.3).

use std::time::Duration;

// --- Versions and transport (§2, §3) ---

/// Protocol major version of this crate (negotiated by ALPN, §3).
pub const PROTOCOL_MAJOR: u16 = 1;
/// Highest protocol minor version this crate implements (negotiated in `Hello`, §3, §3.1):
/// 1 = v1.1 (looks and held-item slots).
pub const PROTOCOL_MINOR: u16 = 1;
/// The minor version that adds `Equipped` flag bit 3 (`slot`) and the `Look` fields (§3.1).
pub const MINOR_LOOKS: u16 = 1;
/// ALPN protocol ID of major version 1: the ASCII bytes `tessera/1` (§2.1).
pub const ALPN_V1: &[u8] = b"tessera/1";
/// Smallest `max_datagram_frame_size` transport parameter a peer must send (§2.1).
pub const MIN_MAX_DATAGRAM_FRAME_SIZE: u64 = 1_200;
/// Bidirectional streams the client lets the server open (§2.1).
pub const CLIENT_MAX_STREAMS_BIDI: u32 = 0;
/// Unidirectional streams the client lets the server open (§2.1).
pub const CLIENT_MAX_STREAMS_UNI: u32 = 2;
/// Bidirectional streams the server lets the client open (§2.1).
pub const SERVER_MAX_STREAMS_BIDI: u32 = 1;
/// Unidirectional streams the server lets the client open (§2.1).
pub const SERVER_MAX_STREAMS_UNI: u32 = 0;
/// Recommended minimum flow-control window per stream, bytes (§2.1).
pub const STREAM_RECEIVE_WINDOW: u32 = 256 * 1024;
/// Recommended minimum flow-control window per connection, bytes (§2.1).
pub const CONNECTION_RECEIVE_WINDOW: u32 = 1024 * 1024;
/// QUIC `max_idle_timeout` on both sides (§2.1).
pub const IDLE_TIMEOUT: Duration = Duration::from_millis(30_000);
/// The client sends a QUIC PING after this long without sending anything (§2.1).
pub const CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(5);
/// The server MAY send a QUIC PING after this long without sending anything (§2.1).
pub const SERVER_KEEP_ALIVE: Duration = Duration::from_secs(10);
/// Initial path MTU, bytes (§2.1).
pub const INITIAL_MTU: u16 = 1_200;

// --- Streams and framing (§2.3, §5.4, §5.5) ---

/// Kind byte that starts the server's world stream (`W`, §2.3).
pub const STREAM_KIND_WORLD: u8 = 0x57;
/// Kind byte that starts the server's chat stream (`C`, §2.3).
pub const STREAM_KIND_CHAT: u8 = 0x43;
/// Size of a frame's `length` field (§5.4).
pub const FRAME_LENGTH_FIELD: usize = 4;
/// Size of the message type field (§5.4, §5.5).
pub const TYPE_FIELD: usize = 2;
/// Smallest valid frame `length`: the type field alone (§5.4).
pub const MIN_FRAME_LENGTH: u32 = 2;
/// Largest valid frame `length` (§5.4).
pub const MAX_FRAME_LENGTH: u32 = 65_536;
/// Largest message body in a frame: `MAX_FRAME_LENGTH` minus the type field.
pub const MAX_FRAME_BODY: usize = MAX_FRAME_LENGTH as usize - TYPE_FIELD;
/// Most bytes a receiver ever buffers for one frame (§5.4, invariant 1).
pub const MAX_FRAME_BYTES: usize = FRAME_LENGTH_FIELD + MAX_FRAME_LENGTH as usize;
/// Largest datagram message, type included (§5.5).
pub const MAX_DATAGRAM: usize = 1_200;
/// Size senders SHOULD keep every datagram within (§5.5).
pub const RECOMMENDED_MAX_DATAGRAM: usize = 1_100;
/// Smallest usable datagram payload accepted at handshake completion when the QUIC stack
/// reports only the payload it can send, not the peer's `max_datagram_frame_size` (§2.1,
/// §5.5): the 1,100 bytes that always fit a 1,200-byte path MTU. Every v1 datagram (at most
/// 1,084 bytes) fits.
pub const MIN_DATAGRAM_PAYLOAD: usize = RECOMMENDED_MAX_DATAGRAM;

// --- Text classes (§5.2) ---

/// `NAME` / `NAME0`: most bytes of a name.
pub const MAX_NAME_LEN: usize = 64;
/// `TEXT`: most bytes of a chat line or notice.
pub const MAX_TEXT_LEN: usize = 1_024;
/// `ACCOUNT`: most bytes of an account name.
pub const MAX_ACCOUNT_LEN: usize = 32;
/// `PASSWORD`: most bytes of a password.
pub const MAX_PASSWORD_LEN: usize = 256;
/// `PLACE`: most bytes of a place name in a `Named` destination.
pub const MAX_PLACE_LEN: usize = 256;
/// `Hello.client_name` and `HelloAck.server_name`: `TEXT0`(64).
pub const MAX_SOFTWARE_NAME_LEN: usize = 64;
/// `HelloAck.motd` and `Disconnect.text`: `TEXT0`(1,024).
pub const MAX_LONG_TEXT_LEN: usize = 1_024;
/// The detail text of `LoginFailed`, `CharacterCreateFailed`, `EnterWorldFailed`,
/// `TeleportRefused` and `ChatRejected`: `TEXT0`(256).
pub const MAX_DETAIL_TEXT_LEN: usize = 256;
/// Largest String property value, bytes (§7.2).
pub const MAX_STRING_VALUE_LEN: usize = 1_024;

// --- Common structures (§5.3) ---

/// Largest absolute Location coordinate, metres (§5.3).
pub const MAX_LOCATION_COORDINATE: f64 = 1.0e7;
/// Side of a landblock, metres (`coordinates.md` §4).
pub const LANDBLOCK_SIZE: f64 = 160.0;
/// Largest landblock index on either axis.
pub const MAX_LANDBLOCK: u8 = 254;
/// Region-global extent of the landblock grid on either axis, metres (255 · 160).
pub const GRID_EXTENT: f64 = 40_800.0;
/// Largest block-local |x| or |y| of a position an Anchor owns, metres (§5.3).
pub const MAX_ANCHOR_OFFSET: f64 = 1_280.0;
/// Most equipped items in an Appearance (§5.3).
pub const MAX_EQUIPMENT: usize = 32;
/// Largest Appearance scale; the range is (0, 100] (§5.3).
pub const MAX_APPEARANCE_SCALE: f32 = 100.0;
/// Smallest `Equipped.slot` (minor ≥ 1, §5.3): the data's equipment slot key.
pub const MIN_EQUIP_SLOT: u8 = 1;
/// Largest `Equipped.slot` (minor ≥ 1, §5.3).
pub const MAX_EQUIP_SLOT: u8 = 64;
/// Most entries of a `Look` (minor ≥ 1, §5.3).
pub const MAX_LOOK_ENTRIES: usize = 48;
/// Largest encoding of a `Look`, count byte included (minor ≥ 1, §5.3).
pub const MAX_LOOK_BYTES: usize = 768;
/// Longest String value of a look entry, bytes (each 0x21–0x7E, §5.3).
pub const MAX_LOOK_STRING_LEN: usize = 128;
/// Most characters in `LoginOk` and `CharacterList` (§6.2.5, §6.2.8).
pub const MAX_CHARACTERS: usize = 64;

// --- Hello / HelloAck (§6.2.1, §6.2.2) ---

/// Smallest `Hello.max_entities` (§6.2.1).
pub const MIN_MAX_ENTITIES: u16 = 64;
/// Smallest `HelloAck.tick_rate`, Hz.
pub const MIN_TICK_RATE: u8 = 1;
/// Largest `HelloAck.tick_rate`, Hz.
pub const MAX_TICK_RATE: u8 = 120;
/// Tick rate of v1 servers, Hz (§6.2.2, §11.1).
pub const DEFAULT_TICK_RATE: u8 = 30;
/// Default `MotionBatch` rate, Hz (§6.2.2, §8.5).
pub const DEFAULT_SNAPSHOT_RATE: u8 = 15;
/// Largest `HelloAck.interest_radius`, landblocks (§6.2.2).
pub const MAX_INTEREST_RADIUS: u8 = 8;
/// Default interest radius *R*, landblocks (§9.2).
pub const DEFAULT_INTEREST_RADIUS: u8 = 2;
/// Server cap on entities created at once on one connection unless the server raises it
/// (§9.4).
pub const DEFAULT_SERVER_ENTITY_CAP: usize = 1_024;

// --- Properties (§7) ---

/// Most entries in a PropList (§7.2).
pub const MAX_PROPERTIES: usize = 256;
/// Deepest nesting of a property value; a top-level value has depth 1 (§7.2).
pub const MAX_VALUE_DEPTH: usize = 4;
/// Most elements of an Array value (§7.2).
pub const MAX_ARRAY_LEN: usize = 256;
/// Most fields of a Struct value (§7.2).
pub const MAX_STRUCT_FIELDS: usize = 64;
/// Largest encoding of one top-level value, tag included, bytes (§7.2).
pub const MAX_VALUE_BYTES: usize = 8_192;

// --- World clock (§6.3.9) ---

/// Largest `WorldClock.rate`, game seconds per real second.
pub const MAX_CLOCK_RATE: f32 = 10_000.0;

// --- Datagrams (§6.5) ---

/// Size of a `MoveInput` datagram, bytes.
pub const MOVE_INPUT_LEN: usize = 53;
/// Size of the `MotionBatch` header (type, time, count), bytes.
pub const MOTION_BATCH_HEADER_LEN: usize = 11;
/// Size of one `MotionBatch` entry, bytes.
pub const MOTION_ENTRY_LEN: usize = 37;
/// Most entries in one `MotionBatch` (§6.5.2).
pub const MAX_MOTION_ENTRIES: usize = 29;
/// Size of a `TimeRequest` datagram, bytes.
pub const TIME_REQUEST_LEN: usize = 10;
/// Size of a `TimeReply` datagram, bytes.
pub const TIME_REPLY_LEN: usize = 34;
/// Most `MotionBatch` datagrams per observer per snapshot (§8.5, §9.4).
pub const MAX_MOTION_DATAGRAMS_PER_SNAPSHOT: usize = 16;

// --- Movement (§8) ---

/// Walking speed when the `WALK_SPEED` property is absent, m/s (§7.3).
pub const DEFAULT_WALK_SPEED: f32 = 2.0;
/// Running speed when the `RUN_SPEED` property is absent, m/s (§7.3).
pub const DEFAULT_RUN_SPEED: f32 = 6.0;
/// Largest `WALK_SPEED` / `RUN_SPEED`, m/s; the range is (0, 100] (§7.3).
pub const MAX_MOVE_SPEED: f32 = 100.0;
/// Horizontal speed below which a grounded avatar is Standing, m/s (§8.1).
pub const STANDING_SPEED: f32 = 0.15;
/// *V*ₕ, m/s (§8.3): the fastest controlled speed of the character controller, 6.5 m/s,
/// plus 30 %. It does not depend on `WALK_SPEED` or `RUN_SPEED` (§7.3).
pub const MAX_HORIZONTAL_SPEED: f64 = 8.5;
/// *V*ᵤ, m/s (§8.3).
pub const MAX_UP_SPEED: f64 = 10.0;
/// *V*_d, m/s (§8.3): the controller's speed cap, above its terminal velocities.
pub const MAX_DOWN_SPEED: f64 = 120.0;
/// *W*, the allowance window (§8.3).
pub const ALLOWANCE_WINDOW: Duration = Duration::from_secs(1);
/// Lowest x or y of the movement extent, metres (§8.3).
pub const EXTENT_MIN: f64 = -1_280.0;
/// Highest x or y of the movement extent, metres (§8.3).
pub const EXTENT_MAX: f64 = 40_800.0 + 1_280.0;
/// Longest settling window a server may apply after an epoch whose starting height it does
/// not know (§8.3 "Server relaxations" item 1).
pub const MAX_SETTLING_WINDOW: Duration = Duration::from_secs(10);
/// Largest descent-credit reserve *S* a server may keep, metres (§8.3 "Server relaxations"
/// item 2).
pub const MAX_DESCENT_CREDIT: f64 = 100.0;
/// Most corrections the server sends per second (§8.3).
pub const MAX_CORRECTIONS_PER_SECOND: u32 = 4;
/// Most `MoveInput`s per second (§8.2, §13.1).
pub const MAX_MOVE_INPUTS_PER_SECOND: u32 = 30;
/// Ticks after stopping in which the client repeats its rest state (§8.2), and snapshots
/// after an entity stops in which the server still includes it (§8.5).
pub const REST_REDUNDANCY: u32 = 3;
/// Recommended client interpolation delay *D* (§8.6).
pub const INTERPOLATION_DELAY: Duration = Duration::from_millis(150);
/// Longest client extrapolation past the newest snapshot (§8.6).
pub const MAX_EXTRAPOLATION: Duration = Duration::from_millis(250);
/// Extra distance beyond *V*ₕ · Δt that makes two snapshots a snap (§8.6), metres.
pub const SNAP_MARGIN: f64 = 5.0;

// --- Chat (§10) ---

/// Say and Emote range, horizontal metres (§10.1).
pub const SAY_RANGE: f64 = 40.0;
/// Capacity of the chat token bucket (§10.2).
pub const CHAT_BUCKET_CAPACITY: u32 = 5;
/// Time to refill one chat token (§10.2).
pub const CHAT_REFILL: Duration = Duration::from_secs(2);
/// Chat rejections within [`CHAT_FLOOD_WINDOW`] that lead to `Disconnect(FLOODING)` (§10.2).
pub const CHAT_FLOOD_REJECTIONS: u32 = 20;
/// Window of the chat flooding rule (§10.2).
pub const CHAT_FLOOD_WINDOW: Duration = Duration::from_secs(60);

// --- Time (§11) ---

/// Time-sync samples a client keeps (§11.2).
pub const TIME_SAMPLES: usize = 8;
/// Initial time requests after `HelloAck` (§11.2).
pub const TIME_BURST: u32 = 4;
/// Interval of the initial time requests (§11.2).
pub const TIME_BURST_INTERVAL: Duration = Duration::from_millis(250);
/// Interval of time requests after the burst (§11.2).
pub const TIME_INTERVAL: Duration = Duration::from_secs(5);
/// Replies to requests older than this are ignored (§11.2).
pub const TIME_REPLY_MAX_AGE: Duration = Duration::from_secs(10);
/// Estimate change above which the client steps instead of slewing (§11.2).
pub const TIME_STEP_THRESHOLD: Duration = Duration::from_millis(100);
/// Largest slew per second of real time (§11.2).
pub const TIME_MAX_SLEW_PER_SECOND: Duration = Duration::from_millis(5);
/// Most `TimeRequest`s a server answers per second per connection (§11.2).
pub const MAX_TIME_REQUESTS_PER_SECOND: u32 = 4;
/// Most `TimeRequest` timestamps the client remembers for matching replies: what the server
/// answers within [`TIME_REPLY_MAX_AGE`] at [`MAX_TIME_REQUESTS_PER_SECOND`] (§11.2, §13.2).
pub const MAX_OUTSTANDING_TIME_REQUESTS: usize = 40;

// --- Control (§6.2.18, §13.1) ---

/// Most `Ping`s an endpoint sends per second (§6.2.18).
pub const MAX_PINGS_PER_SECOND: u32 = 1;
/// Control messages a client may send per second, averaged over
/// [`CONTROL_RATE_WINDOW`] (§13.1).
pub const MAX_CONTROL_PER_SECOND: u32 = 50;
/// Averaging window of the control rate limit (§13.1).
pub const CONTROL_RATE_WINDOW: Duration = Duration::from_secs(5);
/// Failed logins after which the server sends `Disconnect(AUTH_FAILED)` (§4.7).
pub const MAX_LOGIN_FAILURES: u32 = 3;
/// A server answers a `TeleportRequest` within this time (§6.2.12).
pub const TELEPORT_ANSWER_TIME: Duration = Duration::from_secs(10);

// --- Timeouts (§12) ---

/// Client: QUIC/TLS handshake (§12.1). Server: Accepting (§12.2).
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Client: waiting for `HelloAck` (§12.1). Server: waiting for `Hello` (§12.2).
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Client: waiting for the answer to one `Login`/`Register` (§12.1).
pub const LOGIN_ANSWER_TIMEOUT: Duration = Duration::from_secs(30);
/// Server: from `HelloAck` to a successful login (§12.2).
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(60);
/// Client: waiting for `WorldEnter` or `EnterWorldFailed` (§12.1).
pub const ENTER_WORLD_TIMEOUT: Duration = Duration::from_secs(30);
/// Server: CharacterSelect without a message (§12.2).
pub const CHARACTER_SELECT_IDLE: Duration = Duration::from_secs(15 * 60);
/// Linger in Closing before closing the connection (§12.3).
pub const CLOSE_LINGER: Duration = Duration::from_secs(2);
/// Longest reason phrase of a QUIC CONNECTION_CLOSE, bytes (§12.3).
pub const MAX_CLOSE_REASON_LEN: usize = 64;

// --- Interest and budgets (§9.4) ---

/// New `EntityCreate`s per observer per tick (§9.4).
pub const MAX_CREATES_PER_TICK: usize = 16;
/// World-stream bytes written per observer per tick (§9.4).
pub const WORLD_BYTES_PER_TICK: usize = 64 * 1024;
/// World-stream backlog at which creates and updates stop (§9.4).
pub const WORLD_BACKLOG_PAUSE: usize = 1024 * 1024;
/// World-stream backlog below which creates and updates resume (§9.4).
pub const WORLD_BACKLOG_RESUME: usize = 256 * 1024;
/// World-stream backlog that disconnects at once with `SLOW_CONSUMER` (§9.4).
pub const WORLD_BACKLOG_FATAL: usize = 4 * 1024 * 1024;
/// How long the backlog may stay above [`WORLD_BACKLOG_PAUSE`] (§9.4).
pub const WORLD_BACKLOG_GRACE: Duration = Duration::from_secs(30);
/// How long a full control queue is tolerated before `SLOW_CONSUMER` (§13.3).
pub const CONTROL_QUEUE_GRACE: Duration = Duration::from_secs(5);
