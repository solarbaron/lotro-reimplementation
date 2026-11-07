// SPDX-License-Identifier: GPL-3.0-or-later
//! Official-server UDP session and transport layer (`docs/specs/official-session.md`).
//!
//! This module implements:
//! - Transport constants, deterministic local port selection, the
//!   [`UdpDatagramTransport`] trait, and single-socket connection demultiplexing (§3).
//! - The 2-byte `wirePrefix`, the `.dat`-backed [`BitTreeTable`] wire codec,
//!   the 20-byte big-endian [`DatagramHeader`], `0xBADD70DD` header/payload
//!   checksums, and optional headers in strict ascending bit order (§4).
//! - Sequence windows, the gap table with saved ISAAC masks, `CumulativeAckHeader`,
//!   `SequenceNakHeader`, `OutOfOrderAckHeader`, `FlowControlHeader`,
//!   [`VariableU32`](encode_variable_u32) integers, compact variable-length
//!   `FragmentedBlob` fragment headers, 436-byte fragmentation, and bounded
//!   reassembly (§5).
//! - The 9-state connection state machine (`0..8`), the handshake (the challenge's
//!   recipients, seeds and directional dictionaries, the accept's reversed cookie), and the
//!   world referral hand-off (§6).
//! - 3-key Triple DES (EDE3) CBC with zero IV and PKCS#7 padding as a primitive
//!   (§7.1; `FragmentedBlob`s with `EncryptedFragmentedBlob` set are always rejected,
//!   `OPEN-SESSION-6`), the directional ISAAC PRNG (§7.2), and codec UUID
//!   negotiation (§7.3).
//! - `ClockSyncHeader`, silence timeouts and disconnects (§8).
//! - The upper-layer [`MessageEnvelope`] with bounded zlib decompression, the
//!   routing header, the post-handshake version messages, and the envelope
//!   containers `0x00000186` / `0x00000179` (§9). World entity batches
//!   `0x0000000D` / `0x0000004C` are decoded by [`crate::world`]
//!   (`official-messages-world.md` §5.1, the canonical layout).
//!
//! # Hostile input
//!
//! Every decoder checks lengths and counts against the remaining bytes before
//! it reads or allocates (§10.1). Growth that a peer can drive is capped: missing
//! sequence gaps ([`MAX_MISSING_SEQ_GAPS`]), the retransmit table
//! ([`MAX_UNACKED_SENT_PACKETS`]), pending out-of-order acknowledgements
//! ([`MAX_PENDING_OUT_OF_ORDER_ACKS`], extra requests are dropped and
//! counted), concurrent and per-blob reassembly
//! ([`DEFAULT_MAX_CONCURRENT_BLOBS`], [`DEFAULT_MAX_REASSEMBLY_BLOB_BYTES`]),
//! container depth, and the decompressed bytes and leaf messages of one
//! container tree ([`dispatch_message`], [`MAX_UNPACKED_MESSAGES`]). A peer
//! cannot replace another connection by assigning its recipient ID.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

// ---------------------------------------------------------------------------
// §3.5 Datagram and Fragment Size Constants
// ---------------------------------------------------------------------------

/// Maximum total wire datagram size (`0xFF7C` = `65,404` B: `2` B `wirePrefix` + `20` B
/// `DatagramHeader` + `pakSize` payload, §3.5).
pub const MAX_DATAGRAM_BUF: usize = 0xFF7C;

/// Minimum valid UDP datagram length (`2` B wire prefix + `20` B `DatagramHeader` = `22` B, §3.5):
/// the raw minimum of a zero wire prefix, and of any datagram once decompressed
/// ([`raw_length_admitted`]).
pub const MIN_DATAGRAM_BYTES: usize = 0x0016;

/// Smallest raw length of a datagram with a non-zero wire prefix: the prefix and one compressed
/// byte ([`raw_length_admitted`]).
pub const MIN_COMPRESSED_DATAGRAM_BYTES: usize = 3;

/// Fixed size of `DatagramHeader` (`20` B, §3.5).
pub const DATAGRAM_HEADER_BYTES: usize = 0x0014;

/// Maximum byte budget for blob fragments in a single packet (`464` B, §3.5).
pub const MAX_PACKET_BLOB_BUDGET: usize = 0x01D0;

/// Maximum payload slice placed in a single blob fragment (`436` B, §3.5).
pub const MAX_BLOB_FRAG_PAYLOAD: usize = 0x01B4;

/// Maximum number of blob fragments permitted in a single UDP packet (`20`, §3.5).
pub const MAX_FRAGS_PER_PACKET: usize = 0x0014;

/// Maximum total reassembled fragmented blob payload size (`65,520 * 436 = 28,566,720` B, §3.5).
pub const MAX_BLOB_BYTES: usize = 0x01B3_E4C0;

/// Hard ceiling (`32 MiB` = `33,554,432` B) on a zlib `MessageEnvelope`'s `uncompressedLen` (§3.5, §9.1).
pub const MAX_ZLIB_UNCOMPRESSED_BYTES: usize = 0x0200_0000;

/// Maximum number of sequence numbers in a `SequenceNakHeader` or `OutOfOrderAckHeader` header (`114`, §3.5).
pub const MAX_NAK_ENTRIES: usize = 0x0072;

/// Target UDP socket send buffer size (`SO_SNDBUF = 131,072` B, §3.1).
pub const TARGET_SO_SNDBUF: usize = 0x20000;

/// Maximum allowed `DatagramHeader.pakSize` (`0xFF7B` = `65,403` B, §4.3, §10.1).
pub const MAX_PAK_SIZE: u16 = 0xFF7B;

/// Permitted wire flag union of `DatagramHeader.flags` (`0x0F7FFB06`, §4.3, §10.1 rule 3):
/// both directions carry the low markers `0x00000002` and `0x00000004`.
pub const VALID_DATAGRAM_FLAGS_MASK: u32 = 0x0F7F_FB06;

/// Forbidden bits of `DatagramHeader.flags` (`0xF08004F9`, the complement of `0x0F7FFB06`):
/// a datagram with any of them set is rejected on receive and never sent (§4.3, §10.1 rule 3).
pub const INVALID_DATAGRAM_FLAGS_MASK: u32 = !VALID_DATAGRAM_FLAGS_MASK;

/// Receive-side forbidden bitmask for `DatagramHeader.flags` (`0xF08004F9`, §4.3, §10.1
/// rule 3); the same value as [`INVALID_DATAGRAM_FLAGS_MASK`].
pub const RECEIVE_FORBIDDEN_FLAGS_MASK: u32 = 0xF080_04F9;

/// The two low marker bits, which carry no optional header bytes: `MaskedChecksumFlag`
/// (`0x00000002`) and `HasFragmentedBlobsFlag` (`0x00000004`) (§4.3).
pub const HEADERLESS_FLAGS_MASK: u32 = FLAG_MASKED_CHECKSUM | FLAG_HAS_FRAGMENTED_BLOBS;

/// The optional fields of bits 25 and 26 (`0x02000000`, 4 bytes; `0x04000000`, 8 bytes):
/// received and checksummed, never sent by Tessera; their meaning is `OPEN-SESSION-9` (§4.3).
pub const OBSERVED_ONLY_FLAGS_MASK: u32 = FLAG_OPTIONAL_WORD_25 | FLAG_OPTIONAL_WORDS_26;

/// Flags rejected when received from a server: `0x00400000` (`ClientControlHeader`, §10.1
/// rule 7) plus the undefined bits `0x00000400` and `0x00800000` (already rejected by rule 3).
pub const SERVER_FORBIDDEN_OPT_FLAGS_MASK: u32 = 0x0040_0000 | 0x0000_0400 | 0x0080_0000;

/// Placeholder magic placed in `DatagramHeader.checksum` (`0x0C..0x0F`) when computing `hdrSum` (§4.4.2).
pub const HEADER_CHECKSUM_PLACEHOLDER_BE: u32 = 0xBADD_70DD;

/// Format UUID `{E795878D-C633-471C-8315-C3DBBEAD2FAA}` at offset `0x00..0x10` of `.dat` `BitTreeTable` records (§4.1.3).
pub const BIT_TREE_TABLE_FORMAT_UUID: [u8; 16] = [
    0x8d, 0x87, 0x95, 0xe7, 0x33, 0xc6, 0x1c, 0x47, 0x83, 0x15, 0xc3, 0xdb, 0xbe, 0xad, 0x2f, 0xaa,
];

/// Fixed header size (`44` B = `0x2C`) of `.dat` `BitTreeTable` records `0x0E010000`–`0x0E010002` (§4.1.3).
pub const BIT_TREE_TABLE_HEADER_BYTES: usize = 0x2C;

// The dictionary record DIDs (`0x0E010000`..`0x0E010002`) and the DID maps that lead to
// them are values observed in one install (`property-system.md` §8.1). They are resolved
// at run time (`crate::login::root_did_map_from_superblock`, the sub-map keys of §4.1.3)
// and appear only as verification values in the tests.

/// Maximum sequence lookahead window (`0x7FFF` = `32,767`, §5.1).
pub const MAX_SEQ_LOOKAHEAD: u32 = 0x7FFF;

/// Maximum count of tracked missing sequence numbers in the gap tree (`160,000`, §5.1).
pub const MAX_MISSING_SEQ_GAPS: usize = 160_000;

/// Maximum count of unacknowledged sent packets in the retransmit table (`0x1FBC4` = `129,988`, §5.1).
pub const MAX_UNACKED_SENT_PACKETS: usize = 0x1FBC4;

/// Maximum sequence span in the outbound retransmit table (`0x7FFFFFFF`, §5.1).
pub const MAX_OUTBOUND_SEQ_SPAN: u32 = 0x7FFF_FFFF;

/// Default defensive cap on concurrent incomplete fragmented blob reassembly entries (`64`, §3.5, §5.6.4).
pub const DEFAULT_MAX_CONCURRENT_BLOBS: usize = 64;

/// Default defensive cap on a single reassembled fragmented blob buffer (`1 MiB`, §3.5, §5.6.4).
pub const DEFAULT_MAX_REASSEMBLY_BLOB_BYTES: usize = 1024 * 1024;

/// Default defensive cap on a zlib-decompressed `MessageEnvelope` payload (`4 MiB`, §3.5, §9.1).
pub const DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Cap on sequence numbers waiting to be reported as out-of-order
/// acknowledgements: nine full headers of [`MAX_NAK_ENTRIES`]. Requests beyond it are
/// dropped and counted (engineering choice, spec silent).
pub const MAX_PENDING_OUT_OF_ORDER_ACKS: usize = 9 * MAX_NAK_ENTRIES;

/// Hard maximum container nesting depth of [`dispatch_message`]; larger
/// caller values are clamped to it, so a chain of wrappers cannot exhaust the
/// stack (engineering choice, spec silent).
pub const MAX_CONTAINER_DEPTH: usize = 32;

/// Cap on leaf messages produced by unpacking one blob's container tree
/// (engineering choice, spec silent).
pub const MAX_UNPACKED_MESSAGES: usize = 65_536;

/// Default defensive cap on recursive container depth (`8`, §3.5, §9.4).
pub const DEFAULT_MAX_CONTAINER_DEPTH: usize = 8;

/// Most queue-3 sequence groups one connection keeps an ordering counter for (§5.6.2
/// P10-23-POLICY, "each queue-3 sequence group"). The client's own outbound table names
/// five; a send on a group past this bound is refused before anything is allocated.
/// Engineering choice, spec silent.
pub const MAX_ORDER_GROUPS: usize = 256;

/// §9.3.8: the one non-resetting deadline of an incoming sequence gap. Only a later fully
/// checksum-valid sequenced packet opens a gap, so datagrams from anyone else cannot start, reset
/// or end this clock. At its end the connection sends one standalone Disconnect and closes with
/// [`DisconnectReason::GapDeadline`] (Tessera policy, not an observed official rule).
pub const MAX_GAP_AGE_SEC: f64 = 30.0;

// Session timers. [`SessionConnection::tick`] (and the multiplexer's cleanup) acts on them
// and [`SessionConnection::next_deadline`] reports them: both read these constants, so the
// two cannot drift apart.

/// Handshake timeout of States `1` (`SessionAuthSent`) and `2` (`ChallengeAcceptSent`):
/// `20.0` s (§6.1 table).
pub const HANDSHAKE_TIMEOUT_SEC: f64 = 20.0;

/// Retransmit interval of the stored `SessionAuthHeader` / `ReferralAuthHeader` in State
/// `1`: `2.0` s (§6.1 table).
pub const SESSION_AUTH_RETRANSMIT_SEC: f64 = 2.0;

/// Retransmit interval of `ConnectAcceptHeader` in State `2`: `0.6` s (§6.1 table).
pub const CONNECT_ACCEPT_RETRANSMIT_SEC: f64 = 0.6;

/// Interval of the periodic `ClockSyncHeader` in State `5`: `30.0` s (§8.1, §6.1 table).
pub const CLOCK_SYNC_INTERVAL_SEC: f64 = 30.0;

/// Interval of `CumulativeAckHeader` in State `5` while the gap table is empty: `2.0` s
/// (§5.2).
pub const CUMULATIVE_ACK_INTERVAL_SEC: f64 = 2.0;

/// Interval of `SequenceNakHeader` in State `5` while gaps exist: `0.6` s (§5.3).
pub const SEQUENCE_NAK_INTERVAL_SEC: f64 = 0.6;

/// The cap on every flow report after the first in State `5`: at most one per `1.0` s per
/// connection, standalone or attached.
///
/// §9.3.12 "FlowControl evidence and bounded policy (P10-10L)" (a closed Tessera policy; native
/// production stays `OPEN-SESSION-13`): the first report goes out once after State `5`. After it
/// every report, standalone or attached to an outgoing application carrier, waits one second since
/// the last successfully sent report on that connection. Only eligible traffic makes a report due:
/// an accepted masked datagram with an ordinary structurally valid fragment (a zero-data opcode
/// blob included, omitted-chunk controls excluded) or an accepted `ClockSyncHeader`. Pure
/// Ack/NAK/OutOfOrderAck/FlowControl traffic and valid omitted controls never do by themselves,
/// but their bytes still enter `bytesReceivedDelta` (§5.5). A due ClockSync never creates a
/// standalone packet (§9.3.12, `OPEN-SESSION-9`), and `intervalMs` wraps modulo 65,536.
///
/// Labelled (question for the writer): "successfully sent" is read as "allocated and queued for
/// the socket"; a packet that fails to encode resets nothing.
pub const FLOW_REPORT_ONLY_PERIOD_SEC: f64 = 1.0;

/// Silence timeout of State `5`: the connection closes once `now - lastRecvTime` exceeds
/// `60.0` s (§8.2, §6.1 table).
pub const SILENCE_TIMEOUT_SEC: f64 = 60.0;

/// Silence timeout of State `5` with the extended timeout flag: `300.0` s (§8.2, §6.1
/// table).
pub const EXTENDED_SILENCE_TIMEOUT_SEC: f64 = 300.0;

/// Longest wait of State `6` (`DisconnectingFlush`) for the retransmit table to drain:
/// `20.0` s (§6.1 table, §8.2).
pub const DISCONNECT_FLUSH_LIMIT_SEC: f64 = 20.0;

/// Retransmit interval of `DisconnectHeader` in State `7` (`DisconnectSent`): `1.0` s
/// (§6.1 table, §8.2).
pub const DISCONNECT_RETRANSMIT_SEC: f64 = 1.0;

/// Time in State `7` after which the connection closes: `20.0` s (§6.1 table).
pub const DISCONNECT_SENT_LIMIT_SEC: f64 = 20.0;

/// Time in State `8` (`Closing`) before the connection is removed: `20.0` s (§6.1 table,
/// "cleanup after 20.0 s").
pub const CLOSING_CLEANUP_SEC: f64 = 20.0;

/// Expiry of an incomplete blob, from its first fragment: `30.0` s (§5.6.4 rule 3).
pub const BLOB_REASSEMBLY_EXPIRY_SEC: f64 = 30.0;

/// `EntityEntryBatchMessage` (`aoi_enter_batch`) opcode (`0x0000000D` = `13`, §9.4.1;
/// decoded by [`crate::world::SubEventBatch`]).
pub const OPCODE_ENTITY_ENTRY_BATCH: u32 = 0x0000_000D;

/// `EntityBatchUpdateMessage` (`sub_event_batch`) opcode (`0x0000004C` = `76`, §9.4.1;
/// decoded by [`crate::world::SubEventBatch`]).
pub const OPCODE_ENTITY_BATCH_UPDATE: u32 = 0x0000_004C;

/// Post-handshake client -> server `ClientVersionMessage` opcode (`0x0000012C` = `300`, §9.3).
pub const OPCODE_CLIENT_VERSION: u32 = 0x0000_012C;

/// Post-handshake server -> client `VersionStatusMessage` opcode (`0x0000012D` = `301`, §9.3).
pub const OPCODE_VERSION_STATUS: u32 = 0x0000_012D;

/// Post-handshake server -> client `SystemNotificationMessage` opcode (`0x0000012E` = `302`, §9.3).
pub const OPCODE_SYSTEM_NOTIFICATION: u32 = 0x0000_012E;

/// Post-handshake server -> client `VersionRequestMessage` opcode (`0x0000012F` = `303`, §9.3).
pub const OPCODE_VERSION_REQUEST: u32 = 0x0000_012F;

/// Post-handshake server -> client `LoginQueueStatusMessage` opcode (`0x00000130` = `304`, §9.3).
pub const OPCODE_LOGIN_QUEUE_STATUS: u32 = 0x0000_0130;

/// `EnvelopeBatchContainer` opcode on the primary login/world connection (`0x00000186` = `390`, §9.4.2).
pub const OPCODE_BATCH_CONTAINER: u32 = 0x0000_0186;

/// `EnvelopeContextWrapper` opcode on the primary login/world connection (`0x00000179` = `377`, §9.4.2).
pub const OPCODE_SINGLE_WRAPPER: u32 = 0x0000_0179;

// ---------------------------------------------------------------------------
// §4.3 Flag Bit Constants
// ---------------------------------------------------------------------------

/// `ServerRedirectHeader` optional header flag (`0x00000100`, bit 8).
pub const FLAG_SERVER_REDIRECT: u32 = 0x0000_0100;
/// `AuthServerAddressHeader` optional header flag (`0x00000200`, bit 9).
pub const FLAG_AUTH_SERVER_ADDR: u32 = 0x0000_0200;
/// Undefined bit 10 (`0x00000400`, rejected on the wire, §4.3, §10.1 rule 3).
pub const FLAG_UNDEFINED_10: u32 = 0x0000_0400;
/// `WorldReferralHeader` optional header flag (`0x00000800`, bit 11).
pub const FLAG_WORLD_REFERRAL: u32 = 0x0000_0800;
/// `SequenceNakHeader` optional header flag (`0x00001000`, bit 12).
pub const FLAG_SEQUENCE_NAK: u32 = 0x0000_1000;
/// `OutOfOrderAckHeader` optional header flag (`0x00002000`, bit 13).
pub const FLAG_OUT_OF_ORDER_ACK: u32 = 0x0000_2000;
/// `CumulativeAckHeader` optional header flag (`0x00004000`, bit 14).
pub const FLAG_CUMULATIVE_ACK: u32 = 0x0000_4000;
/// `DisconnectHeader` optional header flag (`0x00008000`, bit 15).
pub const FLAG_DISCONNECT: u32 = 0x0000_8000;
/// `SessionAuthHeader` optional header flag (`0x00010000`, bit 16).
pub const FLAG_SESSION_AUTH: u32 = 0x0001_0000;
/// `ReferralAuthHeader` optional header flag (`0x00020000`, bit 17).
pub const FLAG_REFERRAL_AUTH: u32 = 0x0002_0000;
/// `ConnectChallengeHeader` optional header flag (`0x00040000`, bit 18).
pub const FLAG_CONNECT_CHALLENGE: u32 = 0x0004_0000;
/// `ConnectAcceptHeader` optional header flag (`0x00080000`, bit 19).
pub const FLAG_CONNECT_ACCEPT: u32 = 0x0008_0000;
/// `TransportErrorHeader` optional header flag (`0x00100000`, bit 20).
pub const FLAG_TRANSPORT_ERROR: u32 = 0x0010_0000;
/// `TransportFatalErrorHeader` optional header flag (`0x00200000`, bit 21).
pub const FLAG_TRANSPORT_FATAL_ERROR: u32 = 0x0020_0000;
/// `ClientControlHeader` client->server optional header flag (`0x00400000`, bit 22, 4 B `u32` LE, rejected if received from server, §4.3, §10.1 rule 7).
pub const FLAG_CLIENT_CONTROL: u32 = 0x0040_0000;
/// Undefined bit 23 (`0x00800000`, rejected on the wire, §4.3, §10.1 rule 3).
pub const FLAG_UNDEFINED_23: u32 = 0x0080_0000;
/// `ClockSyncHeader` optional header flag (`0x01000000`, bit 24).
pub const FLAG_CLOCK_SYNC: u32 = 0x0100_0000;
/// `MaskedChecksumFlag` (`0x00000002`, bit 1): sequenced packet whose payload checksum is
/// masked with the directional ISAAC word of its `seqNum`; no optional header bytes (§4.3).
pub const FLAG_MASKED_CHECKSUM: u32 = 0x0000_0002;
/// `HasFragmentedBlobsFlag` (`0x00000004`, bit 2): `FragmentedBlob` fragments follow the
/// optional headers; no optional header bytes (§4.3).
pub const FLAG_HAS_FRAGMENTED_BLOBS: u32 = 0x0000_0004;
/// `OptionalWord25` (`0x02000000`, bit 25): a four-byte optional field, observed
/// client to server; meaning `OPEN-SESSION-9`; Tessera never sends it (§4.3).
pub const FLAG_OPTIONAL_WORD_25: u32 = 0x0200_0000;
/// `OptionalWords26` (`0x04000000`, bit 26): an eight-byte optional field, observed
/// server to client; meaning `OPEN-SESSION-9`; Tessera never sends it (§4.3).
pub const FLAG_OPTIONAL_WORDS_26: u32 = 0x0400_0000;
/// `FlowControlHeader` optional header flag (`0x08000000`, bit 27).
pub const FLAG_FLOW_CONTROL: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors returned by the official-server session and transport layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SessionError {
    /// Datagram is too short for its form ([`raw_length_admitted`]), or decompressed to less than
    /// a whole `DatagramHeader` (§10.1 rule 1).
    #[error("datagram length {len} is shorter than the minimum of its form")]
    DatagramTooShort {
        /// Raw length in bytes; for a decompressed packet without a whole header, the length it
        /// would have as an unencoded datagram.
        len: usize,
    },
    /// Datagram exceeds `MAX_DATAGRAM_BUF` (65,404 bytes, §3.5).
    #[error("datagram length {len} exceeds maximum buffer {MAX_DATAGRAM_BUF} bytes")]
    DatagramTooLarge {
        /// Actual length in bytes.
        len: usize,
    },
    /// Bit-tree wire compression failed or overflowed staging buffer (§4.1.2).
    #[error("bit-tree wire compression failed")]
    WireCompressionFailed,
    /// Bit-tree wire decompression failed or did not end on byte boundary (§4.1.2, §10.1 rule 2).
    #[error("bit-tree wire decompression failed")]
    WireDecompressionFailed,
    /// Compressed datagram arrived (`wirePrefix != 0x0000`) but no bit-tree codec was configured.
    #[error("compressed wire prefix {wire_prefix:#06x} received without a bit-tree codec")]
    MissingWireCodec {
        /// Non-zero wire prefix.
        wire_prefix: u16,
    },
    /// `DatagramHeader.pakSize` exceeds `0xFF7B` or does not match `decoded_len - 20` (§10.1 rule 1).
    #[error("invalid DatagramHeader pakSize {pak_size} for payload length {actual_payload_len}")]
    InvalidPakSize {
        /// Header `pakSize` field.
        pak_size: u16,
        /// Actual bytes following the 20-byte header.
        actual_payload_len: usize,
    },
    /// `DatagramHeader.flags` has bits outside `0x0F7FFB06` or forbidden server flags (§10.1 rules 3, 7).
    #[error("invalid or forbidden DatagramHeader flags {flags:#010x}")]
    InvalidHeaderFlags {
        /// Raw flags field.
        flags: u32,
    },
    /// Checksum verification failed (§4.4.2, §10.1 rule 6).
    #[error("packet checksum mismatch: expected {expected:#010x}, got {actual:#010x}")]
    ChecksumMismatch {
        /// Expected checksum computed over header and payload.
        expected: u32,
        /// Checksum carried in `DatagramHeader.checksum`.
        actual: u32,
    },
    /// Source `(IPv4, port)` did not match the existing connection of a non-zero `recId` (§10.1 rule 4).
    /// It carries no address or port, in its text or its `Debug` form: errors reach log
    /// lines, which never name one (decision 0044 item 3).
    #[error("source address does not match the peer of recId {rec_id:#06x}")]
    SourceAddressMismatch {
        /// Recipient ID.
        rec_id: u16,
    },
    /// No active connection matched the incoming datagram (§3.4). It carries no address or
    /// port, in its text or its `Debug` form: errors reach log lines, which never name one
    /// (decision 0044 item 3).
    #[error("no connection found for recId {rec_id:#06x}")]
    UnknownRecipient {
        /// Recipient ID.
        rec_id: u16,
    },
    /// Incoming sequence number exceeds `recvSeqExpected + 0x7FFF` (§5.1, §10.1 rule 5).
    #[error("sequence number {seq_num} exceeds lookahead window from {expected}")]
    SequenceOutOfWindow {
        /// Packet sequence number.
        seq_num: u32,
        /// Current expected sequence number.
        expected: u32,
    },
    /// Sequenced packet at or below `recvSeqExpected` that is not a tracked gap: its masked
    /// checksum cannot be verified, so it is dropped before any header acts (§5.3, §10.1 rule 6).
    #[error("sequence number {seq_num} already received (expected watermark {expected})")]
    DuplicateSequence {
        /// Packet sequence number.
        seq_num: u32,
        /// Current expected sequence number.
        expected: u32,
    },
    /// Missing sequence gap count exceeds `160,000` (§5.1, §10.1 rule 5).
    #[error("missing sequence gap count {count} exceeds maximum {MAX_MISSING_SEQ_GAPS}")]
    GapLimitExceeded {
        /// Number of missing gaps.
        count: usize,
    },
    /// The retransmit table holds more than `129,988` unacknowledged packets or spans more
    /// than `0x7FFFFFFF` sequence numbers; the connection closes locally (§5.1).
    #[error("outbound retransmit window overflow")]
    OutboundWindowOverflow,
    /// Optional header truncated or overran `pakSize` (§10.1 rule 7).
    #[error("optional header {flag:#010x} overran packet payload")]
    OptionalHeaderOverrun {
        /// Flag bit being parsed.
        flag: u32,
    },
    /// Trailing bytes remained in payload when `HasFragmentedBlobsFlag` was not set.
    #[error("{remaining} unparsed trailing bytes after optional headers")]
    TrailingPayloadBytes {
        /// Unconsumed byte count.
        remaining: usize,
    },
    /// `SequenceNakHeader` entry count was `0` or exceeded `114` (`0x72`, §5.3, §10.1 rule 7).
    #[error("invalid sequence NAK entry count {count} (must be in 1..={MAX_NAK_ENTRIES})")]
    InvalidSequenceNakCount {
        /// Count value in header.
        count: u32,
    },
    /// `OutOfOrderAckHeader` entry count was `0` or exceeded `114` (`0x72`, §5.4, §10.1 rule 7).
    #[error("invalid OutOfOrderAck entry count {count} (must be in 1..={MAX_NAK_ENTRIES})")]
    InvalidOutOfOrderAckCount {
        /// Count value in header.
        count: u32,
    },
    /// `SessionAuthHeader.netVersion` is not `"180804_"` followed by a
    /// printable ASCII version stamp, or is longer than [`MAX_SESSION_NET_VERSION_LEN`]
    /// (§6.2).
    #[error("invalid SessionAuth netVersion string")]
    InvalidSessionAuthVersion,
    /// `SessionAuthHeader.authBlockLen` does not match the authentication block, or the
    /// block is malformed (§6.2, §6.2.1).
    #[error("invalid SessionAuth authBlock length or structure")]
    InvalidAuthBlock,
    /// UTF-16LE string in `SessionAuthHeader` was truncated or invalid UTF-16 (§6.2.1).
    #[error("invalid UTF-16LE string in authentication block")]
    InvalidUtf16String,
    /// Variable-length `VariableU32` was truncated or had an invalid prefix (§5.6.1).
    #[error("truncated or invalid VariableU32 encoding")]
    InvalidVariableU32,
    /// A value above the transport compact integer's range (`0x3FFFFFFF`, §5.6.1) was to
    /// be sent; wider values are `OPEN-SESSION-12`.
    #[error("value {value:#010x} exceeds the VariableU32 range")]
    VariableU32OutOfRange {
        /// The value refused.
        value: u32,
    },
    /// Packet contained more than `20` (`0x14`) blob fragments (§5.6.3, §10.1 rule 8).
    #[error("packet fragment count {count} exceeds maximum {MAX_FRAGS_PER_PACKET}")]
    TooManyFragments {
        /// Fragment count.
        count: usize,
    },
    /// A fragment on receiver queue `5` or `6`, whose framing is `OPEN-SESSION-12`
    /// (§5.6.2, §5.6.3): the whole datagram is rejected before anything is committed
    /// (checksum generator, sequence, Ack, reassembly, delivery) and it is not acknowledged;
    /// the connection goes on under its timeouts. Counted
    /// ([`HandshakeCounts::unsupported_queue_drops`]).
    #[error("fragment on receiver queue {queue_id}, whose framing is unspecified")]
    UnsupportedReceiveQueue {
        /// The queue ID (`5` or `6`).
        queue_id: u8,
    },
    /// A fragment with `blobFlags` bit `0x20`, unverified (§9.3.9): the datagram is dropped and
    /// counted.
    #[error("fragment with undefined blobFlags bits {blob_flags:#04x}")]
    UndefinedBlobFlags {
        /// The fragment's `blobFlags`.
        blob_flags: u8,
    },
    /// A fragment with `blobFlags` bit `0x02` on a queue other than `1` (`OPEN-SESSION-14`,
    /// §9.3.9). A received datagram with one is rejected whole before anything is committed and
    /// is not acknowledged. Counted ([`HandshakeCounts::unsupported_fragment_drops`]).
    #[error("unsupported fragment on queue {queue_id} with blobFlags {blob_flags:#04x}")]
    UnsupportedFragment {
        /// The fragment's queue.
        queue_id: u8,
        /// The fragment's `blobFlags`.
        blob_flags: u8,
    },
    /// A fragment whose `extWord` has a bit other than the attachment's: unverifiable, so its
    /// datagram is dropped and counted (§9.3.9).
    #[error("fragment with unknown extension bits {ext_word:#06x}")]
    UnknownExtensionBits {
        /// The fragment's `extWord`.
        ext_word: u16,
    },
    /// A send on more queue-3 sequence groups than [`MAX_ORDER_GROUPS`]; nothing was
    /// allocated (§5.6.2 P10-23-POLICY keeps a counter per group).
    #[error("too many queue-3 sequence groups ({limit})")]
    TooManyOrderGroups {
        /// The bound.
        limit: usize,
    },
    /// An unmasked datagram has the fragment marker (`0x00000004`) set and carries a fragment, on any
    /// queue (§9.3.9 closed transport action table, §10.2 Vector 24): its checksum has no key, so
    /// the whole datagram is dropped and counted before any Ack, baseline, terminal result or other
    /// state commit. Standalone unmasked controls and handshakes without fragments keep their own
    /// rules.
    #[error("an unmasked datagram with the fragment marker set (first queue {queue_id})")]
    UnmaskedFragment {
        /// The first fragment's `queueId`.
        queue_id: u8,
    },
    /// An `OmitChunkInfo` (`0x80`) fragment together with fragmentation `0x10`, control bit `0x02`,
    /// encryption `0x08` or the undefined `0x20` (§9.3.9 closed transport action table, §10.2
    /// Vector 24): an invalid omitted-chunk combination. The whole datagram is dropped and
    /// counted before any terminal inference; it is never read as a known-layout `0x02`.
    #[error("invalid omitted-chunk control on queue {queue_id} (blobFlags {blob_flags:#04x})")]
    InvalidOmittedControl {
        /// The fragment's `queueId`.
        queue_id: u8,
        /// The fragment's `blobFlags`.
        blob_flags: u8,
    },
    /// A `ConnectChallengeHeader` whose datagram header `recId` or payload bytes `18..19`
    /// (the incoming and outgoing recipients, §6.3) are zero: Tessera adopts only non-zero
    /// IDs (§6.3; refusing zero is an engineering choice).
    #[error("ConnectChallengeHeader without non-zero recipient IDs")]
    InvalidChallengeRecipient,
    /// Blob fragment had an invalid `queueId`: outside `1..=4` (`5` and `6` on receive are
    /// [`SessionError::UnsupportedReceiveQueue`]) (§5.6.2).
    #[error("invalid fragmented blob delivery queue ID {queue_id}")]
    InvalidQueueId {
        /// Decoded `queueId`.
        queue_id: u8,
    },
    /// Blob fragment had `numFrags == 0` or `numFrags <= fragIndex` (§5.6.3, §10.1 rule 8).
    #[error("invalid blob fragment bounds: fragIndex={frag_index}, numFrags={num_frags}")]
    InvalidFragmentBounds {
        /// Zero-based fragment index.
        frag_index: u32,
        /// Total fragment count.
        num_frags: u32,
    },
    /// Blob fragment's `fragDataLen` breaks the slice rule: `0..=464` for a single fragment; of
    /// several, every non-final one exactly 436 and the final one `1..=436` (§5.6.3, §9.3.9).
    #[error("Fragmented blob fragDataLen {len} breaks the slice length rule")]
    InvalidFragmentDataLen {
        /// Decoded `fragDataLen`.
        len: u32,
    },
    /// Blob fragment header odd-length alignment pad byte was non-zero (§5.6.3, §10.1 rule 8).
    #[error("Blob fragment alignment pad byte was {pad:#04x}, expected 0x00")]
    NonZeroAlignmentPad {
        /// Non-zero pad byte.
        pad: u8,
    },
    /// Blob fragment payload overran the packet buffer.
    #[error("Blob fragment payload overran packet buffer")]
    FragmentPayloadOverrun,
    /// Reassembled fragmented blob size exceeds protocol or defensive cap (§5.6.4, §10.1 rule 8).
    #[error("reassembled fragmented blob size {total_bytes} exceeds limit {limit}")]
    BlobTooLarge {
        /// Computed total byte size.
        total_bytes: usize,
        /// Maximum allowed byte size.
        limit: usize,
    },
    /// Concurrent incomplete fragmented blob reassembly table is full.
    #[error("incomplete fragmented blob reassembly table full ({max_concurrent} entries)")]
    ReassemblyTableFull {
        /// Cap on concurrent incomplete blobs.
        max_concurrent: usize,
    },
    /// A `FragmentedBlob` had `EncryptedFragmentedBlob` (`blobFlags` bit 3, `0x08`) set. No
    /// session key is ever provisioned, so the WHOLE containing datagram drops and is counted
    /// before reassembly, with no committed state or Ack (§7.1, §9.3.9, `OPEN-SESSION-6`).
    #[error("encrypted FragmentedBlob rejected (no session cipher key is provisioned)")]
    EncryptedBlobRejected,
    /// 3DES-CBC ciphertext length is empty or not a multiple of 8 bytes (§7.1).
    #[error("3DES-CBC ciphertext length {len} is not a positive multiple of 8")]
    CiphertextNotBlockAligned {
        /// Ciphertext length in bytes.
        len: usize,
    },
    /// PKCS#7 padding in decrypted 3DES-CBC plaintext is invalid (§7.1).
    #[error("invalid PKCS#7 padding in 3DES-CBC plaintext")]
    InvalidPkcs7Padding,
    /// `ConnectChallengeHeader.codecUuid` is non-null and matches no loaded `BitTreeTable`
    /// dictionary; the connection closes locally (§6.4 step 3, §7.3).
    #[error("unsupported per-connection codec UUID")]
    UnsupportedCodecUuid {
        /// Requested 16-byte UUID.
        uuid: [u8; 16],
    },
    /// Message envelope payload is empty (missing `envelopeFlags` byte, §9.1).
    #[error("Message envelope is empty")]
    EmptyMessageEnvelope,
    /// Zlib decompression of a message envelope failed (§9.1).
    #[error("MessageEnvelope zlib decompression failed")]
    ZlibDecompressionFailed,
    /// Zlib-decompressed `MessageEnvelope` exceeded the configured size bound (§9.1).
    #[error("decompressed MessageEnvelope size {size} exceeds limit {limit}")]
    DecompressedMessageTooLarge {
        /// Requested or actual decompressed size.
        size: usize,
        /// Maximum allowed decompressed size.
        limit: usize,
    },
    /// A container sub-message length overran the remaining container bytes (§9.4.2, §10.1 rule 9).
    #[error(
        "batch container 0x00000186 sub-message length {sub_len} exceeds remaining {remaining} bytes"
    )]
    ContainerSubMessageOverrun {
        /// Declared sub-message length.
        sub_len: usize,
        /// Remaining bytes in container.
        remaining: usize,
    },
    /// Nested containers exceeded the maximum recursion depth (§9.4.2, §10.1 rule 9).
    #[error("batch container 0x00000186 recursion depth exceeded maximum {max_depth}")]
    ContainerRecursionLimitExceeded {
        /// Maximum allowed depth.
        max_depth: usize,
    },
    /// `.dat` `BitTreeTable` record (`0x0E010000`–`0x0E010002`) is malformed or has an invalid format UUID / version (§4.1.3).
    #[error("invalid BitTreeTable .dat record")]
    InvalidBitTreeTable,
    /// `MessageEnvelope` `envelopeFlags` byte has reserved bits `(envelopeFlags & 0xFC) != 0` set (§9.1).
    #[error("invalid message envelopeFlags {flags:#04x}")]
    InvalidEnvelopeFlags {
        /// Raw `envelopeFlags` byte.
        flags: u8,
    },
    /// A checksum-valid fragment conflicts with that same active or remembered identity on queues
    /// 2–4: another body or count (§9.3.8). The recipient is part of the identity (§5.6.4 rule 3).
    #[error("a fragment conflicts with its blob identity")]
    ConflictingBlobIdentity,

    /// `MessageEnvelope` optional routing header (`envelopeFlags & 0x02`) is truncated or has invalid flags (§9.1).
    #[error("invalid MessageEnvelope routing header")]
    InvalidRoutingHeader,
    /// A post-handshake session/version control message (`0x0000012C`..`0x00000130`) or an
    /// envelope container (`0x00000186` / `0x00000179`) is malformed (§9.3, §9.4.2).
    #[error("malformed session control or batch message for opcode {opcode:#010x}")]
    InvalidControlMessage {
        /// Message opcode.
        opcode: u32,
    },
    /// The server rejected the client version (`VersionStatusMessage` `0x0000012D` with
    /// `status` `0`); the connection closes locally (§9.3).
    #[error("server rejected the client version: {server_version_info}")]
    VersionRejected {
        /// Server version string from `VersionStatus`.
        server_version_info: String,
    },
    /// A blob fragment's sequence group does not fit its 16-bit field.
    #[error("Blob fragment sequence group {value} does not fit 16 bits")]
    SequenceGroupOutOfRange {
        /// Decoded value.
        value: u32,
    },
    /// A challenge's incoming recipient ID is one another connection already uses (§3.4).
    #[error("assigned recipient ID {rec_id:#06x} is already used by another connection")]
    RecipientIdCollision {
        /// The assigned recipient ID.
        rec_id: u16,
    },
    /// Every connection handle of a multiplexer is in use.
    #[error("no free connection handle")]
    RecipientIdsExhausted,
    /// Unpacking nested containers produced more output than the configured
    /// budget allows (decompressed bytes or messages, summed over the tree).
    #[error("message unpacking exceeded its budget ({what} {actual} > {limit})")]
    UnpackBudgetExceeded {
        /// The budget that ran out (`"bytes"` or `"messages"`).
        what: &'static str,
        /// The amount that was needed.
        actual: usize,
        /// The budget.
        limit: usize,
    },
    /// The message is receive-only and Tessera never encodes or sends it.
    #[error("message {opcode:#010x} is receive-only and is never sent")]
    ReceiveOnlyMessage {
        /// Message opcode.
        opcode: u32,
    },
    /// `SessionAuthHeader` `(authType, authFlags)` is not one of the modes of §6.2.1
    /// ([`SUPPORTED_AUTH_MODES`]).
    #[error(
        "unsupported SessionAuthHeader mode authType {auth_type:#010x}, authFlags {auth_flags:#010x}"
    )]
    UnsupportedAuthMode {
        /// Raw `authType` word.
        auth_type: u32,
        /// Raw `authFlags` word.
        auth_flags: u32,
    },
    /// The credentials of a `SessionAuthHeader` do not carry exactly the optional fields its
    /// `authFlags` name (`secondStr` for `0x02`, `extraBlob` for `0x04`, §6.2.1).
    #[error("SessionAuthHeader credentials do not match authFlags {auth_flags:#010x}")]
    AuthCredentialsMismatch {
        /// Raw `authFlags` word.
        auth_flags: u32,
    },
    /// A `ConnectChallengeHeader` arrived outside States `1` and `2`; it is logged and the
    /// packet is dropped (§6.4).
    #[error("ConnectChallengeHeader received in connection state {state:?}")]
    UnexpectedConnectChallenge {
        /// Current state.
        state: ConnectionState,
    },
    /// A packet has `MaskedChecksumFlag` set but no ISAAC mask was supplied (§4.4.2).
    #[error("masked checksum without an ISAAC mask")]
    MissingChecksumMask,
    /// A container record count does not fit its wire field.
    #[error("count {count} does not fit its wire field")]
    CountOverflow {
        /// The count.
        count: usize,
    },
    /// Bytes are left after the last field of a message whose reader refuses them (none of
    /// the session layer's own readers does: §9.4.2 ignores bytes after a container).
    #[error("{remaining} trailing bytes after message {opcode:#010x}")]
    TrailingMessageBytes {
        /// Message opcode.
        opcode: u32,
        /// Unparsed byte count.
        remaining: usize,
    },
    /// A GLS ticket string is empty, longer than [`MAX_TICKET_STRING_LEN`] or not printable
    /// ASCII; it is sent as text and never decoded (§6.2.1).
    #[error("GLS ticket is not a printable ASCII string")]
    InvalidTicketEncoding,
    /// Every Case B candidate port was in use ([`MAX_UNIQUE_PORT_ATTEMPTS`] attempts, §3.2,
    /// `OPEN-SESSION-7`).
    #[error("no free local UDP port among the computed candidates")]
    LocalPortsExhausted,
    /// Operation is invalid in the current connection state (§6.1).
    #[error("invalid operation in connection state {state:?}")]
    InvalidState {
        /// Current state.
        state: ConnectionState,
    },
    /// Underlying UDP socket I/O error.
    #[error("UDP socket I/O error: {0}")]
    Io(String),
    /// A send the socket could not take just now ([`crate::net_access`]'s choice of which errors these
    /// are). The datagram is lost like any lost on the way, and the session goes on: the failure is
    /// counted, and only [`Self::Io`] is `official-session.md` §9.3.9's "ordinary I/O failure".
    #[error("UDP socket could not take this send: {0}")]
    IoTransient(String),
    /// The network right was revoked (`official-entry.md` §6.1). Nothing failed on the network: the
    /// connector's thread abandons the session, which is no I/O failure of §9.3.9.
    #[error("the network access was revoked")]
    AccessRevoked,
}

impl From<std::io::Error> for SessionError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// §3.1–§3.2 Local Port Selection & Transport
// ---------------------------------------------------------------------------

/// The modulus of the §3.2 Case B seed (`64,007`).
pub const UNIQUE_PORT_MODULUS: u32 = 64_007;

/// The step added before each Case B bind attempt (`256`, modulo `65,536`, §3.2).
pub const UNIQUE_PORT_STEP: u16 = 256;

/// The lowest Case B port: a candidate below it is raised to it (`1,024`, §3.2).
pub const MIN_UNIQUE_PORT: u16 = 1024;

/// Most Case B bind attempts before Tessera reports local port exhaustion (`256`): a
/// Tessera resource bound, not an observed rule of the official client (§3.2,
/// `OPEN-SESSION-7`, `P9-BIND`).
pub const MAX_UNIQUE_PORT_ATTEMPTS: usize = 256;

/// The unsigned value of §3.2 Case B: the address octets a, b, c, d read as
/// a × 2²⁴ + b × 2¹⁶ + c × 2⁸ + d (the host-order value), so `192.0.2.10` gives
/// `0xC000020A`.
#[must_use]
pub fn unique_port_ip_value(ip: Ipv4Addr) -> u32 {
    u32::from(ip)
}

/// The §3.2 Case B seed: [`unique_port_ip_value`] modulo [`UNIQUE_PORT_MODULUS`]
/// (`9,712` for `192.0.2.10`).
#[must_use]
pub fn unique_port_seed(ip: Ipv4Addr) -> u16 {
    // The remainder is below 64,007, so it fits a `u16`.
    u16::try_from(unique_port_ip_value(ip) % UNIQUE_PORT_MODULUS).unwrap_or(u16::MAX)
}

/// The Case B candidate after `previous` (the seed for the first attempt): `previous + 256`
/// modulo `65,536`, raised to [`MIN_UNIQUE_PORT`] when below it (§3.2).
#[must_use]
pub fn next_unique_port(previous: u16) -> u16 {
    previous.wrapping_add(UNIQUE_PORT_STEP).max(MIN_UNIQUE_PORT)
}

/// The Case B candidates for the first local address `ip`, in the order they are tried:
/// at most [`MAX_UNIQUE_PORT_ATTEMPTS`] (§3.2; `9,968`, `10,224`, ... for `192.0.2.10`).
#[must_use]
pub fn unique_port_candidates(ip: Ipv4Addr) -> Vec<u16> {
    let mut port = unique_port_seed(ip);
    (0..MAX_UNIQUE_PORT_ATTEMPTS)
        .map(|_| {
            port = next_unique_port(port);
            port
        })
        .collect()
}

/// How the local UDP port is chosen (§3.2 item 3). Its `Debug` names the case, not the
/// port or the seed address (decision 0044 item 3).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BindPorts {
    /// Case A: the explicit port (`--outport` or `Net.UserSpecifiedPort`), tried once.
    Explicit(u16),
    /// Case B: the computed unique port, seeded from the first local IPv4 address; an
    /// address-in-use failure moves to the next candidate, any other failure ends.
    Unique {
        /// The first local IPv4 address (non-zero).
        seed_ip: Ipv4Addr,
    },
    /// Case C: port `0` (the system's choice), tried once.
    Ephemeral,
}

impl std::fmt::Debug for BindPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Explicit(_) => f.write_str("Explicit(<port>)"),
            Self::Unique { .. } => f.write_str("Unique { seed_ip: <set> }"),
            Self::Ephemeral => f.write_str("Ephemeral"),
        }
    }
}

impl BindPorts {
    /// The case of §3.2 item 3 for the explicit port `explicit_port` (`0`: none),
    /// `Net.ComputeUniquePort` `compute_unique` and the first local IPv4 address
    /// `first_local` (`None` or `0.0.0.0`: none).
    #[must_use]
    pub fn select(explicit_port: u16, compute_unique: bool, first_local: Option<Ipv4Addr>) -> Self {
        if explicit_port != 0 {
            return Self::Explicit(explicit_port);
        }
        match first_local.filter(|ip| !ip.is_unspecified()) {
            Some(seed_ip) if compute_unique => Self::Unique { seed_ip },
            _ => Self::Ephemeral,
        }
    }

    /// The ports tried, in order: one for Cases A and C, [`unique_port_candidates`] for
    /// Case B.
    #[must_use]
    pub fn candidates(&self) -> Vec<u16> {
        match *self {
            Self::Explicit(port) => vec![port],
            Self::Unique { seed_ip } => unique_port_candidates(seed_ip),
            Self::Ephemeral => vec![0],
        }
    }
}

/// Where and how the session's UDP socket binds (§3.2 items 2 and 3): the local address
/// (`0.0.0.0` unless `Net.BindInterface` or the `localHost` segment of `-h` selects one)
/// and the port case. Its `Debug` says whether the address is the wildcard, not which
/// address (decision 0044 item 3).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BindPlan {
    /// The local IPv4 address bound.
    pub address: Ipv4Addr,
    /// The port case.
    pub ports: BindPorts,
}

impl std::fmt::Debug for BindPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let address = if self.address.is_unspecified() {
            "<wildcard>"
        } else {
            "<selected>"
        };
        f.debug_struct("BindPlan")
            .field("address", &address)
            .field("ports", &self.ports)
            .finish()
    }
}

impl BindPlan {
    /// The plan of the secondary chat-server transport (§3.2 item 1): the defaults
    /// (`BindInterface = ""`, `UserSpecifiedPort = 0`, `ComputeUniquePort = true`), so the
    /// wildcard address and Case B when a first local address is known.
    #[must_use]
    pub fn chat_defaults(first_local: Option<Ipv4Addr>) -> Self {
        Self {
            address: Ipv4Addr::UNSPECIFIED,
            ports: BindPorts::select(0, true, first_local),
        }
    }
}

/// Computes the binary-search sequence of `SO_SNDBUF` buffer sizes over
/// `[current_size, TARGET_SO_SNDBUF]` until the interval width is `<= 1` (§3.1).
#[must_use]
pub fn sndbuf_probe_midpoint(low: usize, high: usize) -> Option<usize> {
    if high <= low || high - low <= 1 {
        None
    } else {
        Some(low + (high - low) / 2)
    }
}

/// Non-blocking IPv4 UDP datagram transport (§3.1).
pub trait UdpDatagramTransport: std::fmt::Debug + Send {
    /// Returns the local bound IPv4 socket address.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if querying the socket address fails.
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError>;

    /// Sends a single UDP datagram to `target`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the datagram exceeds `MAX_DATAGRAM_BUF` or
    /// the underlying socket fails: [`SessionError::Io`] when it refuses the send for good,
    /// [`SessionError::IoTransient`] when it could not take this one just now.
    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError>;

    /// Receives a single non-blocking UDP datagram into `buf`, returning
    /// `Ok(None)` when no datagram is available (`WouldBlock` / `EAGAIN`).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] on underlying socket error.
    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError>;
}

/// The standard-library UDP transport lives behind the network-access type
/// (`docs/specs/official-entry.md` §6.1): it is made only by
/// [`NetworkAccess::bind_udp`](crate::net_access::NetworkAccess::bind_udp).
pub use crate::net_access::StdUdpTransport;

/// Deterministic in-memory UDP datagram transport for synthetic unit and integration tests.
#[derive(Debug)]
pub struct InMemoryUdpTransport {
    local_addr: SocketAddrV4,
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    outbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
}

impl InMemoryUdpTransport {
    /// Creates an in-memory transport bound to `local_addr`.
    #[must_use]
    pub fn new(local_addr: SocketAddrV4) -> Self {
        Self {
            local_addr,
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
        }
    }

    /// Enqueues a synthetic incoming datagram from `source`.
    pub fn push_inbound(&mut self, datagram: Vec<u8>, source: SocketAddrV4) {
        self.inbound.push_back((datagram, source));
    }

    /// Pops the next transmitted datagram and its destination address.
    pub fn pop_outbound(&mut self) -> Option<(Vec<u8>, SocketAddrV4)> {
        self.outbound.pop_front()
    }

    /// Returns the number of queued outbound datagrams.
    #[must_use]
    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }
}

impl UdpDatagramTransport for InMemoryUdpTransport {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(self.local_addr)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        if buf.len() > MAX_DATAGRAM_BUF {
            return Err(SessionError::DatagramTooLarge { len: buf.len() });
        }
        self.outbound.push_back((buf.to_vec(), target));
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        let Some((data, src)) = self.inbound.pop_front() else {
            return Ok(None);
        };
        if data.len() > buf.len() {
            return Err(SessionError::DatagramTooLarge { len: data.len() });
        }
        buf[..data.len()].copy_from_slice(&data);
        Ok(Some((data.len(), src)))
    }
}

// ---------------------------------------------------------------------------
// §4.1 Two-Byte Datagram Wire Prefix and Bit-Tree Compression
// ---------------------------------------------------------------------------

/// Wire codec for datagrams with a non-zero `wirePrefix` (§4.1.2). The specified codec is
/// [`BitTreeTable`], loaded at run time from the user's `client_general.dat` (§4.1.3).
pub trait BitTreeCodec: std::fmt::Debug + Send + Sync {
    /// Decodes `wire_body` (datagram bytes `2..N`) into `out`, appending at most `max_out` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::WireDecompressionFailed`] if the bitstream is malformed,
    /// truncated, or overflows `max_out` (§10.1 rule 2).
    fn decode(
        &self,
        wire_body: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError>;

    /// Encodes `plain_after_rec_id` (bytes `2..20+pakSize` of `[DatagramHeader | Payload]`)
    /// into `out`, writing at most `max_out` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::WireCompressionFailed`] if encoding fails or exceeds `max_out`.
    fn encode(
        &self,
        plain_after_rec_id: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError>;
}

/// The two wire codecs of a connection (§4.1.2, §7.3): `incoming` decompresses received
/// datagrams (the server-to-client dictionary), `outgoing` compresses sent ones (the
/// client-to-server dictionary). They are never swapped: every send uses `outgoing`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WireCodecs<'a> {
    /// Server-to-client dictionary (`None`: no codec attached).
    pub incoming: Option<&'a dyn BitTreeCodec>,
    /// Client-to-server dictionary (`None`: no codec attached).
    pub outgoing: Option<&'a dyn BitTreeCodec>,
}

impl WireCodecs<'_> {
    /// No codec in either direction (every datagram has `wirePrefix` `0x0000`).
    pub const NONE: WireCodecs<'static> = WireCodecs {
        incoming: None,
        outgoing: None,
    };
}

/// A leaf node (`isLeaf != 0`) in a `.dat` `BitTreeTable` (`0x0E010000` / `0x0E010001`, §4.1.3 item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitTreeLeaf {
    /// Uncompressed byte sequence emitted by this leaf (`1..=4` bytes, §4.1.3).
    pub symbol_bytes: Vec<u8>,
    /// Bit length of the leaf's Huffman codeword (`1..=31`, so its codeword fits the
    /// `floor(codeBitLen / 8) + 1 <= 4` stored bytes; observed `1..=27`, §4.1.3).
    pub code_bit_len: u8,
    /// Little-endian codeword consumed LSB-first from bit `0` to `code_bit_len - 1` (§4.1.3).
    pub codeword: u32,
}

/// A node in a `.dat` `BitTreeTable` (`0x0E010000` / `0x0E010001`, §4.1.3 item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitTreeNode {
    /// Leaf node (`isLeaf != 0`).
    Leaf(BitTreeLeaf),
    /// Internal branch node (`isLeaf == 0`) with **1-based** child node indices (`1..=nodeCount`).
    Branch {
        /// 1-based node index taken when the next input bit is `0`.
        child0_1based: u16,
        /// 1-based node index taken when the next input bit is `1`.
        child1_1based: u16,
    },
}

/// `.dat`-backed `BitTreeTable` bit-tree wire codec (`0x0E010000`–`0x0E010002`, §4.1.2, §4.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitTreeTable {
    /// Format UUID at `0x00..0x10` (`BIT_TREE_TABLE_FORMAT_UUID`, §4.1.3).
    pub format_uuid: [u8; 16],
    /// Per-dictionary wire codec UUID at `0x10..0x20` matched against `ConnectChallengeHeader.codecUuid` (§4.1.3, §7.3).
    pub codec_uuid: [u8; 16],
    /// Optional redirect DID at `0x20..0x24` (`0x00000000` in `0x0E010000`; `0x0E010001` in `0x0E010001` / `0x0E010002`).
    pub redirect_did: u32,
    /// Format version at `0x24..0x28` (`1` in `0x0E010000` and `0x0E010001`, §4.1.3).
    pub version: u32,
    /// Sequential node definitions (`0..node_count`), where all leaves precede all branches (§4.1.3).
    pub nodes: Vec<BitTreeNode>,
    /// 0-based index of the first internal branch node (the root of the decoding tree, §4.1.3).
    pub root_node_index: usize,
    /// Fast lookup map from `1..=4`-byte uncompressed sequences to `(codeword, code_bit_len)`.
    leaf_map: HashMap<Vec<u8>, (u32, u8)>,
}

impl BitTreeTable {
    /// Checks the 44-byte `.dat` header of the record `own_did` and returns
    /// `Ok(Some(redirect_did))` when the loader must follow `redirectDid` (`+0x20`) to another
    /// record: only when `redirectDid` is non-zero **and** differs from `own_did` (the stub
    /// `0x0E010002` -> `0x0E010001`). `Ok(None)` means the payload at `+0x24` is the literal
    /// dictionary (`redirectDid` zero, as in `0x0E010000`, or equal to `own_did`, as in
    /// `0x0E010001`) (§4.1.3 item 1).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidBitTreeTable`] if `bytes.len() < 44` or `formatUuid` mismatches.
    pub fn parse_redirect_did(bytes: &[u8], own_did: u32) -> Result<Option<u32>, SessionError> {
        let (Some(format_uuid), Some(redirect)) = (bytes.get(0..16), bytes.get(32..36)) else {
            return Err(SessionError::InvalidBitTreeTable);
        };
        if format_uuid != BIT_TREE_TABLE_FORMAT_UUID {
            return Err(SessionError::InvalidBitTreeTable);
        }
        let redirect_did = u32::from_le_bytes([redirect[0], redirect[1], redirect[2], redirect[3]]);
        if redirect_did != 0 && redirect_did != own_did {
            Ok(Some(redirect_did))
        } else {
            Ok(None)
        }
    }

    /// Parses a `.dat` `BitTreeTable` record (`0x0E010000` or `0x0E010001`) from its raw record bytes (§4.1.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidBitTreeTable`] if the 44-byte header, `formatUuid`,
    /// `version`, `bodyLen`, or node table violates the §4.1.3 invariants.
    pub fn from_dat_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < BIT_TREE_TABLE_HEADER_BYTES {
            return Err(SessionError::InvalidBitTreeTable);
        }
        let mut format_uuid = [0u8; 16];
        format_uuid.copy_from_slice(&bytes[0..16]);
        if format_uuid != BIT_TREE_TABLE_FORMAT_UUID {
            return Err(SessionError::InvalidBitTreeTable);
        }
        let mut codec_uuid = [0u8; 16];
        codec_uuid.copy_from_slice(&bytes[16..32]);
        let redirect_did = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        let version = u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);
        // §4.1.3 item 2: the parser requires `version` to equal `1`.
        if version != 1 {
            return Err(SessionError::InvalidBitTreeTable);
        }
        let body_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
        if bytes.len() != BIT_TREE_TABLE_HEADER_BYTES.saturating_add(body_len) || body_len < 4 {
            return Err(SessionError::InvalidBitTreeTable);
        }

        let body = &bytes[BIT_TREE_TABLE_HEADER_BYTES..];
        let node_count = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        if !(2..=65535).contains(&node_count) {
            return Err(SessionError::InvalidBitTreeTable);
        }

        let mut cursor = 4usize;
        let mut nodes = Vec::with_capacity(node_count.min(4096));
        let mut leaf_map = HashMap::new();
        let mut root_node_index: Option<usize> = None;

        for idx in 0..node_count {
            let Some(&is_leaf) = body.get(cursor) else {
                return Err(SessionError::InvalidBitTreeTable);
            };
            cursor += 1;

            if is_leaf != 0 {
                // Invariant (§4.1.3 item 3): all leaf nodes precede all internal branch nodes
                if root_node_index.is_some() {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                let Some(&symbol_len) = body.get(cursor) else {
                    return Err(SessionError::InvalidBitTreeTable);
                };
                cursor += 1;
                let sym_len = usize::from(symbol_len);
                if !(1..=4).contains(&sym_len) || body.len() < cursor.saturating_add(sym_len) + 1 {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                let symbol_bytes = body[cursor..cursor + sym_len].to_vec();
                cursor += sym_len;

                let code_bit_len = body[cursor];
                cursor += 1;
                if !(1..=31).contains(&code_bit_len) {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                let cw_bytes_len = (usize::from(code_bit_len) >> 3) + 1;
                if cw_bytes_len > 4 || body.len() < cursor.saturating_add(cw_bytes_len) {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                let mut cw_buf = [0u8; 4];
                cw_buf[..cw_bytes_len].copy_from_slice(&body[cursor..cursor + cw_bytes_len]);
                cursor += cw_bytes_len;
                let codeword = u32::from_le_bytes(cw_buf);

                leaf_map.insert(symbol_bytes.clone(), (codeword, code_bit_len));
                nodes.push(BitTreeNode::Leaf(BitTreeLeaf {
                    symbol_bytes,
                    code_bit_len,
                    codeword,
                }));
            } else {
                if root_node_index.is_none() {
                    root_node_index = Some(idx);
                }
                if body.len() < cursor + 4 {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                let child0_1based = u16::from_le_bytes([body[cursor], body[cursor + 1]]);
                let child1_1based = u16::from_le_bytes([body[cursor + 2], body[cursor + 3]]);
                cursor += 4;
                // `0` is a null child (§4.1.3 item 3); decoding rejects a bit that selects it
                // (item 4). Any other index must name a node of the table.
                if usize::from(child0_1based) > node_count
                    || usize::from(child1_1based) > node_count
                {
                    return Err(SessionError::InvalidBitTreeTable);
                }
                nodes.push(BitTreeNode::Branch {
                    child0_1based,
                    child1_1based,
                });
            }
        }

        if cursor != body.len() {
            return Err(SessionError::InvalidBitTreeTable);
        }
        let Some(root_idx) = root_node_index else {
            return Err(SessionError::InvalidBitTreeTable);
        };

        Ok(Self {
            format_uuid,
            codec_uuid,
            redirect_did,
            version,
            nodes,
            root_node_index: root_idx,
            leaf_map,
        })
    }

    /// Serializes this `BitTreeTable` into the exact `.dat` binary record format (`44 + m` bytes, §4.1.3).
    #[must_use]
    pub fn to_dat_bytes(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        for node in &self.nodes {
            match node {
                BitTreeNode::Leaf(leaf) => {
                    body.push(1);
                    body.push(leaf.symbol_bytes.len() as u8);
                    body.extend_from_slice(&leaf.symbol_bytes);
                    body.push(leaf.code_bit_len);
                    let cw_bytes_len = (usize::from(leaf.code_bit_len) >> 3) + 1;
                    let cw_le = leaf.codeword.to_le_bytes();
                    body.extend_from_slice(&cw_le[..cw_bytes_len.min(4)]);
                }
                BitTreeNode::Branch {
                    child0_1based,
                    child1_1based,
                } => {
                    body.push(0);
                    body.extend_from_slice(&child0_1based.to_le_bytes());
                    body.extend_from_slice(&child1_1based.to_le_bytes());
                }
            }
        }

        let mut out = Vec::with_capacity(BIT_TREE_TABLE_HEADER_BYTES + body.len());
        out.extend_from_slice(&self.format_uuid);
        out.extend_from_slice(&self.codec_uuid);
        out.extend_from_slice(&self.redirect_did.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Constructs a synthetic `.dat` `BitTreeTable` for unit and integration testing (§4.1.3).
    ///
    /// Includes all 256 single-byte leaves (`0x00..=0xFF`) plus a 4-byte leaf `[0x00, 0x00, 0x00, 0x00]`
    /// with a 1-bit codeword (`bit 0 = 0`, 1 bit -> 4 zero bytes!) so zero-rich packets compress
    /// below `inputLen + 1` (`isCompressed == 1`) while incompressible random payloads exercise
    /// literal fallback (`isCompressed == 0`, `wireBody[0] == 0x00`).
    #[must_use]
    pub fn synthetic_dictionary(codec_uuid: [u8; 16]) -> Self {
        // Leaf 0: 4-byte [0, 0, 0, 0] -> 1-bit codeword 0b0 (bit_len = 1)
        // Leaves 1..=256: 1-byte [sym] -> 9-bit codeword ((sym << 1) | 1) (bit_len = 9: first bit = 1, then 8 bits LSB-first of sym)
        let mut nodes = Vec::with_capacity(513);
        let mut leaf_map = HashMap::new();

        let zero4 = vec![0u8, 0, 0, 0];
        leaf_map.insert(zero4.clone(), (0u32, 1u8));
        nodes.push(BitTreeNode::Leaf(BitTreeLeaf {
            symbol_bytes: zero4,
            code_bit_len: 1,
            codeword: 0,
        }));

        for b in 0..=255u8 {
            let cw = (u32::from(b) << 1) | 1;
            let sb = vec![b];
            leaf_map.insert(sb.clone(), (cw, 9));
            nodes.push(BitTreeNode::Leaf(BitTreeLeaf {
                symbol_bytes: sb,
                code_bit_len: 9,
                codeword: cw,
            }));
        }

        // Now add internal branch nodes starting at index 257 (root_node_index = 257):
        // Root node (index 257, 1-based 258):
        //   bit 0 -> Leaf 0 (1-based index 1)
        //   bit 1 -> subtree root at index 258 (1-based 259)
        let root_node_index = nodes.len(); // 257
        nodes.push(BitTreeNode::Branch {
            child0_1based: 1,
            child1_1based: 259,
        });

        // Build a complete 8-level binary tree for the 8 bits of `b` (LSB-first):
        // Internal nodes at depth d = 0..7: 255 internal nodes at 0-based indices 258..=512.
        // Heap-like numbering `t` in `1..=255` where `0-based index = 257 + t`:
        // For `t` in `1..=127`:
        //   bit 0 -> `t_left = t + (1 << depth)`? Wait: let's build by `(depth, prefix)`!
        // Even simpler: insert each 1-byte leaf `b` (`0..=255`) into the branch table!
        let mut branch_children: Vec<[u16; 2]> = vec![[0, 0]]; // index 0 in branch_children is 0-based node 258
        for b in 0..=255u8 {
            let leaf_1based = u16::from(b) + 2; // Leaf for `b` is at 0-based `1 + b`, so 1-based `2 + b`
            let mut cur_br = 0usize;
            for bit_idx in 0..8u8 {
                let bit = usize::from((b >> bit_idx) & 1);
                if bit_idx == 7 {
                    branch_children[cur_br][bit] = leaf_1based;
                } else {
                    if branch_children[cur_br][bit] == 0 {
                        let next_br = branch_children.len();
                        branch_children.push([0, 0]);
                        // 0-based node index of `next_br` is `258 + next_br`, so 1-based is `259 + next_br`
                        branch_children[cur_br][bit] = (259 + next_br) as u16;
                    }
                    cur_br = usize::from(branch_children[cur_br][bit]) - 259;
                }
            }
        }
        for &[c0, c1] in &branch_children {
            nodes.push(BitTreeNode::Branch {
                child0_1based: c0,
                child1_1based: c1,
            });
        }

        Self {
            format_uuid: BIT_TREE_TABLE_FORMAT_UUID,
            codec_uuid,
            redirect_did: 0,
            version: 1,
            nodes,
            root_node_index,
            leaf_map,
        }
    }
}

impl BitTreeTable {
    fn decode_into(
        &self,
        wire_body: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError> {
        let Some(&hdr_byte) = wire_body.first() else {
            return Err(SessionError::WireDecompressionFailed);
        };
        let is_compressed = (hdr_byte & 1) != 0;
        let unused_trailing_bits = usize::from((hdr_byte >> 1) & 7);
        let start_len = out.len();

        if !is_compressed {
            // Literal fallback (§4.1.3 item 4): wireBody[0] has bit 0 == 0, followed by raw bytes
            if unused_trailing_bits != 0 {
                return Err(SessionError::WireDecompressionFailed);
            }
            let literal = &wire_body[1..];
            if literal.len() > max_out {
                return Err(SessionError::WireDecompressionFailed);
            }
            out.extend_from_slice(literal);
            return Ok(literal.len());
        }

        let total_bits = wire_body.len().saturating_mul(8);
        if total_bits < 4usize.saturating_add(unused_trailing_bits) {
            return Err(SessionError::WireDecompressionFailed);
        }
        let valid_bit_end = total_bits - unused_trailing_bits;
        if unused_trailing_bits > 0
            && let Some(&last_byte) = wire_body.last()
            && (last_byte >> (8 - unused_trailing_bits)) != 0
        {
            return Err(SessionError::WireDecompressionFailed);
        }

        let mut bit_pos = 4usize;
        let max_steps = self.nodes.len();
        // The decoding tree's root is a branch (§4.1.3 item 3). A caller-built table whose
        // root is a leaf would emit symbols without consuming bits, so it is refused.
        if !matches!(
            self.nodes.get(self.root_node_index),
            Some(BitTreeNode::Branch { .. })
        ) {
            return Err(SessionError::WireDecompressionFailed);
        }

        while bit_pos < valid_bit_end {
            let mut cur = self.root_node_index;
            let mut steps = 0usize;
            loop {
                let node = self
                    .nodes
                    .get(cur)
                    .ok_or(SessionError::WireDecompressionFailed)?;
                match node {
                    BitTreeNode::Leaf(leaf) => {
                        if (out.len() - start_len).saturating_add(leaf.symbol_bytes.len()) > max_out
                        {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        out.extend_from_slice(&leaf.symbol_bytes);
                        break;
                    }
                    BitTreeNode::Branch {
                        child0_1based,
                        child1_1based,
                    } => {
                        if bit_pos >= valid_bit_end || steps >= max_steps {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        let bit = (wire_body[bit_pos >> 3] >> (bit_pos & 7)) & 1;
                        bit_pos += 1;
                        steps += 1;
                        let next_1based = if bit == 0 {
                            *child0_1based
                        } else {
                            *child1_1based
                        };
                        if next_1based == 0 {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        cur = usize::from(next_1based) - 1;
                    }
                }
            }
        }

        Ok(out.len() - start_len)
    }
}

impl BitTreeCodec for BitTreeTable {
    fn decode(
        &self,
        wire_body: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError> {
        // A failed decode leaves `out` as it was: no partial packet bytes survive.
        let start_len = out.len();
        let result = self.decode_into(wire_body, out, max_out);
        if result.is_err() {
            out.truncate(start_len);
        }
        result
    }

    fn encode(
        &self,
        plain_after_rec_id: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError> {
        let mut bit_buf = vec![0u8];
        let mut bit_pos = 4usize; // Reserve 4-bit header at bits 0..3 of wireBody[0] (§4.1.3 item 4)

        let mut pos = 0usize;
        let mut compressed_ok = true;
        while pos < plain_after_rec_id.len() {
            let max_match = (plain_after_rec_id.len() - pos).min(4);
            let mut matched: Option<(usize, u32, u8)> = None;
            for len in (1..=max_match).rev() {
                if let Some(&(cw, bit_len)) = self.leaf_map.get(&plain_after_rec_id[pos..pos + len])
                {
                    matched = Some((len, cw, bit_len));
                    break;
                }
            }
            let Some((sym_len, mut cw, mut bit_len)) = matched else {
                compressed_ok = false;
                break;
            };
            while bit_len > 0 {
                let byte_idx = bit_pos >> 3;
                let bit_in_byte = bit_pos & 7;
                if byte_idx >= bit_buf.len() {
                    bit_buf.push(0);
                }
                bit_buf[byte_idx] |= ((cw & 1) as u8) << bit_in_byte;
                cw >>= 1;
                bit_len -= 1;
                bit_pos += 1;
            }
            pos += sym_len;
        }

        let total_bytes = bit_pos.div_ceil(8).max(1);
        if compressed_ok && total_bytes <= plain_after_rec_id.len() + 1 {
            if total_bytes > max_out {
                return Err(SessionError::WireCompressionFailed);
            }
            let unused_trailing_bits = ((total_bytes * 8) - bit_pos) as u8;
            bit_buf[0] |= (unused_trailing_bits << 1) | 1;
            out.extend_from_slice(&bit_buf[..total_bytes]);
            Ok(total_bytes)
        } else {
            // Literal fallback (§4.1.3 item 4): wireBody[0] = 0x00 followed by raw input bytes
            let fallback_len = 1 + plain_after_rec_id.len();
            if fallback_len > max_out {
                return Err(SessionError::WireCompressionFailed);
            }
            out.push(0x00);
            out.extend_from_slice(plain_after_rec_id);
            Ok(fallback_len)
        }
    }
}

// ---------------------------------------------------------------------------
// §4.2 Twenty-Byte DatagramHeader & §4.4 Checksum Calculation
// ---------------------------------------------------------------------------

/// 20-byte big-endian `DatagramHeader` at the start of every decoded packet (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DatagramHeader {
    /// Recipient connection ID (`0x0000` on the first `SessionAuthHeader` and
    /// `ReferralAuthHeader`; the outgoing recipient on later client packets, the adopted
    /// incoming recipient on server packets, §3.4, §6.3).
    pub rec_id: u16,
    /// Byte length of the packet payload immediately following this 20-byte header (`0..=0xFF7B`).
    pub pak_size: u16,
    /// Bitmask of optional headers and packet flags (permitted union `0x0F7FFB06`, §4.3).
    pub flags: u32,
    /// Packet sequence number: `0` on `SessionAuthHeader` and `ConnectAcceptHeader`, the
    /// newly allocated number (`>= 2`) on a sequenced packet, the current allocated number
    /// on a standalone acknowledgement (§5.1, §6.4.1).
    pub seq_num: u32,
    /// 32-bit packet checksum (`headerChecksum + payloadChecksum`, the payload part masked
    /// by the directional ISAAC word when `MaskedChecksumFlag` is set, §4.4.2).
    pub checksum: u32,
    /// Header bytes `16..17` (`clientWord16`): `0` before the challenge, the transport's
    /// initial outgoing word `1` from the accept on (§4.2, §6.4). Never a clock.
    pub client_word: u16,
    /// Header bytes `18..19` (`peerWord18`): `0` before the challenge, then a copy of the
    /// challenge datagram header's bytes `18..19` (§4.2, §6.4). Opaque; never a clock.
    pub peer_word: u16,
}

impl DatagramHeader {
    /// Serializes this `DatagramHeader` into 20 big-endian wire bytes (§4.2).
    #[must_use]
    pub fn to_wire_bytes(self) -> [u8; DATAGRAM_HEADER_BYTES] {
        let mut buf = [0u8; DATAGRAM_HEADER_BYTES];
        buf[0..2].copy_from_slice(&self.rec_id.to_be_bytes());
        buf[2..4].copy_from_slice(&self.pak_size.to_be_bytes());
        buf[4..8].copy_from_slice(&self.flags.to_be_bytes());
        buf[8..12].copy_from_slice(&self.seq_num.to_be_bytes());
        buf[12..16].copy_from_slice(&self.checksum.to_be_bytes());
        buf[16..18].copy_from_slice(&self.client_word.to_be_bytes());
        buf[18..20].copy_from_slice(&self.peer_word.to_be_bytes());
        buf
    }

    /// Parses a received 20-byte big-endian `DatagramHeader` and validates its `flags`
    /// against the receive mask [`RECEIVE_FORBIDDEN_FLAGS_MASK`] (§4.2, §4.3, §10.1 rule 3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `bytes.len() < 20`, `(flags & 0xF08004F9) != 0`, or `pak_size > 0xFF7B`.
    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        let Some(h) = bytes.first_chunk::<DATAGRAM_HEADER_BYTES>() else {
            return Err(SessionError::DatagramTooShort { len: bytes.len() });
        };
        let rec_id = u16::from_be_bytes([h[0], h[1]]);
        let pak_size = u16::from_be_bytes([h[2], h[3]]);
        let flags = u32::from_be_bytes([h[4], h[5], h[6], h[7]]);
        let seq_num = u32::from_be_bytes([h[8], h[9], h[10], h[11]]);
        let checksum = u32::from_be_bytes([h[12], h[13], h[14], h[15]]);
        let client_word = u16::from_be_bytes([h[16], h[17]]);
        let peer_word = u16::from_be_bytes([h[18], h[19]]);

        if (flags & RECEIVE_FORBIDDEN_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }
        if pak_size > MAX_PAK_SIZE {
            return Err(SessionError::InvalidPakSize {
                pak_size,
                actual_payload_len: bytes.len().saturating_sub(DATAGRAM_HEADER_BYTES),
            });
        }
        Ok(Self {
            rec_id,
            pak_size,
            flags,
            seq_num,
            checksum,
            client_word,
            peer_word,
        })
    }

    /// Computes `hdrSum` for this header by substituting `BA DD 70 DD` at bytes `0x0C..0x0F` (§4.4.2).
    #[must_use]
    pub fn header_checksum(mut self) -> u32 {
        self.checksum = HEADER_CHECKSUM_MAGIC_BE;
        let wire = self.to_wire_bytes();
        compute_datagram_checksum(&wire)
    }
}

/// Placeholder constant `0xBADD70DD` (`BA DD 70 DD` in big-endian wire order, §4.4.2).
pub const HEADER_CHECKSUM_MAGIC_BE: u32 = HEADER_CHECKSUM_PLACEHOLDER_BE;

/// The checksum primitive `compute_datagram_checksum` of one buffer (§4.4.1): the byte
/// length × 65,536 (for every buffer, whatever its length; an empty buffer gives `0`),
/// plus every complete four-byte group read as a big-endian word, plus a final word of
/// the one to three remaining bytes placed in its most significant positions, all modulo
/// 2³².
#[must_use]
pub fn compute_datagram_checksum(buf: &[u8]) -> u32 {
    // The length term, reduced modulo 2³² (a buffer is at most a datagram long).
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    let mut acc = len.wrapping_shl(16);
    let (chunks, rem) = buf.as_chunks::<4>();
    for &chunk in chunks {
        acc = acc.wrapping_add(u32::from_be_bytes(chunk));
    }
    if !rem.is_empty() {
        let mut tail = [0u8; 4];
        for (slot, &b) in tail.iter_mut().zip(rem) {
            *slot = b;
        }
        acc = acc.wrapping_add(u32::from_be_bytes(tail));
    }
    acc
}

/// Combines `hdrSum` of `header` with `payload_sum` into the wire `DatagramHeader.checksum`
/// (§4.4.2 step 3): with `MaskedChecksumFlag` set, `payload_sum` is XORed with `xor_mask`
/// first.
///
/// # Errors
///
/// Returns [`SessionError::MissingChecksumMask`] if `MaskedChecksumFlag` is set and
/// `xor_mask` is `None`. A mask given for an unmasked header is not used.
pub fn combine_packet_checksum(
    header: DatagramHeader,
    payload_sum: u32,
    xor_mask: Option<u32>,
) -> Result<u32, SessionError> {
    let hdr_sum = header.header_checksum();
    let effective_payload_sum = if (header.flags & FLAG_MASKED_CHECKSUM) != 0 {
        payload_sum ^ xor_mask.ok_or(SessionError::MissingChecksumMask)?
    } else {
        payload_sum
    };
    Ok(hdr_sum.wrapping_add(effective_payload_sum))
}

/// `payloadSum` of §4.4.2 step 2, the same rule on send and on receive:
/// [`compute_datagram_checksum`] summed (modulo 2³²) over each optional header buffer in
/// ascending flag-bit order and, for each fragment, over its even-length fragment header (with
/// its 28-byte attachment, if any, in the same buffer, §9.3.9) and over its `fragDataLen`-byte
/// payload slice as separate buffers. A standalone control packet (one optional header, such as
/// `SessionAuthHeader`) is one whole buffer.
///
/// # Errors
///
/// Returns [`SessionError`] if a header or fragment fails to encode.
pub fn payload_checksum(
    opt_headers: &OptionalHeaders,
    fragments: &[BlobFragment],
) -> Result<u32, SessionError> {
    let mut sum = 0u32;
    let secret = opt_headers.session_auth.is_some();
    for buffer in opt_headers.encode_buffers()? {
        // The SessionAuthHeader buffer holds the ticket (`official-login.md` §6.3).
        let buffer = SecretBuf {
            buf: buffer,
            secret,
        };
        sum = sum.wrapping_add(compute_datagram_checksum(&buffer.buf));
    }
    for frag in fragments {
        let mut header = Vec::new();
        frag.header.encode(&mut header)?;
        sum = sum
            .wrapping_add(compute_datagram_checksum(&header))
            .wrapping_add(compute_datagram_checksum(&frag.payload));
    }
    Ok(sum)
}

/// Verifies `header.checksum` against the per-buffer `payload_sum` ([`payload_checksum`];
/// [`DecodedPacket::payload_sum`] on receive) and `xor_mask` (§4.4.2, §10.1 rule 6).
///
/// # Errors
///
/// Returns [`SessionError::ChecksumMismatch`] if the computed checksum does not match
/// `header.checksum`, or [`SessionError::MissingChecksumMask`] (see
/// [`combine_packet_checksum`]).
pub fn verify_packet_checksum(
    header: DatagramHeader,
    payload_sum: u32,
    xor_mask: Option<u32>,
) -> Result<(), SessionError> {
    let expected = combine_packet_checksum(header, payload_sum, xor_mask)?;
    if header.checksum != expected {
        return Err(SessionError::ChecksumMismatch {
            expected,
            actual: header.checksum,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §5.6.1 VariableU32 Variable-Length Integer Encoding
// ---------------------------------------------------------------------------

/// The largest value the transport compact integer holds (`0x3FFFFFFF`, the four-byte
/// tier of §5.6.1).
pub const MAX_VARIABLE_U32: u32 = 0x3FFF_FFFF;

/// Encodes `value` as a transport compact integer (§5.6.1), appending `1`, `2` or `4`
/// bytes to `out` and returning the number written: `0..=127` verbatim; `128..=16,383` as
/// `0x80` plus bits 8–13, then bits 0–7; `16,384..=0x3FFFFFFF` as `0xC0` plus bits 24–29,
/// then bits 16–23, then the low 16 bits little-endian.
///
/// # Errors
///
/// [`SessionError::VariableU32OutOfRange`] for a value above [`MAX_VARIABLE_U32`]: a
/// bounded sender refuses it rather than truncate (§5.6.1; wider values are
/// `OPEN-SESSION-12`). Nothing is written then.
pub fn encode_variable_u32(value: u32, out: &mut Vec<u8>) -> Result<usize, SessionError> {
    if value < 0x80 {
        out.push(value as u8);
        Ok(1)
    } else if value < 0x4000 {
        out.extend_from_slice(&[0x80 | ((value >> 8) as u8 & 0x3F), (value & 0xFF) as u8]);
        Ok(2)
    } else if value <= MAX_VARIABLE_U32 {
        let low = (value & 0xFFFF) as u16;
        out.extend_from_slice(&[0xC0 | ((value >> 24) as u8 & 0x3F), (value >> 16) as u8]);
        out.extend_from_slice(&low.to_le_bytes());
        Ok(4)
    } else {
        Err(SessionError::VariableU32OutOfRange { value })
    }
}

/// Decodes a transport compact integer from the start of `bytes` (§5.6.1), returning
/// `(value, bytes_consumed)`. The tier follows from the first byte alone: top bit clear,
/// one byte; top bits `10`, two bytes; top bits `11` (`0xC0..=0xFF`), four bytes.
///
/// # Errors
///
/// Returns [`SessionError::InvalidVariableU32`] if `bytes` is shorter than its tier.
pub fn decode_variable_u32(bytes: &[u8]) -> Result<(u32, usize), SessionError> {
    let Some(&b0) = bytes.first() else {
        return Err(SessionError::InvalidVariableU32);
    };
    if (b0 & 0x80) == 0 {
        Ok((u32::from(b0), 1))
    } else if (b0 & 0x40) == 0 {
        let Some(&b1) = bytes.get(1) else {
            return Err(SessionError::InvalidVariableU32);
        };
        Ok(((u32::from(b0 & 0x3F) << 8) | u32::from(b1), 2))
    } else {
        let Some(&[_, b1, b2, b3]) = bytes.first_chunk::<4>() else {
            return Err(SessionError::InvalidVariableU32);
        };
        let low16 = u16::from_le_bytes([b2, b3]);
        let val = (u32::from(b0 & 0x3F) << 24) | (u32::from(b1) << 16) | u32::from(low16);
        Ok((val, 4))
    }
}

// ---------------------------------------------------------------------------
// §7.1 Three-Key Triple DES (3DES-EDE3) CBC with Zero IV & PKCS#7 Padding
// ---------------------------------------------------------------------------

const DES_IP: [u8; 64] = [
    58, 50, 42, 34, 26, 18, 10, 2, 60, 52, 44, 36, 28, 20, 12, 4, 62, 54, 46, 38, 30, 22, 14, 6,
    64, 56, 48, 40, 32, 24, 16, 8, 57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11, 3, 61,
    53, 45, 37, 29, 21, 13, 5, 63, 55, 47, 39, 31, 23, 15, 7,
];

const DES_FP: [u8; 64] = [
    40, 8, 48, 16, 56, 24, 64, 32, 39, 7, 47, 15, 55, 23, 63, 31, 38, 6, 46, 14, 54, 22, 62, 30,
    37, 5, 45, 13, 53, 21, 61, 29, 36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27,
    34, 2, 42, 10, 50, 18, 58, 26, 33, 1, 41, 9, 49, 17, 57, 25,
];

const DES_E: [u8; 48] = [
    32, 1, 2, 3, 4, 5, 4, 5, 6, 7, 8, 9, 8, 9, 10, 11, 12, 13, 12, 13, 14, 15, 16, 17, 16, 17, 18,
    19, 20, 21, 20, 21, 22, 23, 24, 25, 24, 25, 26, 27, 28, 29, 28, 29, 30, 31, 32, 1,
];

const DES_P: [u8; 32] = [
    16, 7, 20, 21, 29, 12, 28, 17, 1, 15, 23, 26, 5, 18, 31, 10, 2, 8, 24, 14, 32, 27, 3, 9, 19,
    13, 30, 6, 22, 11, 4, 25,
];

const DES_PC1: [u8; 56] = [
    57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18, 10, 2, 59, 51, 43, 35, 27, 19, 11, 3, 60,
    52, 44, 36, 63, 55, 47, 39, 31, 23, 15, 7, 62, 54, 46, 38, 30, 22, 14, 6, 61, 53, 45, 37, 29,
    21, 13, 5, 28, 20, 12, 4,
];

const DES_PC2: [u8; 48] = [
    14, 17, 11, 24, 1, 5, 3, 28, 15, 6, 21, 10, 23, 19, 12, 4, 26, 8, 16, 7, 27, 20, 13, 2, 41, 52,
    31, 37, 47, 55, 30, 40, 51, 45, 33, 48, 44, 49, 39, 56, 34, 53, 46, 42, 50, 36, 29, 32,
];

const DES_SHIFTS: [u8; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

const DES_SBOX: [[[u8; 16]; 4]; 8] = [
    [
        [14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7],
        [0, 15, 7, 4, 14, 2, 13, 1, 10, 6, 12, 11, 9, 5, 3, 8],
        [4, 1, 14, 8, 13, 6, 2, 11, 15, 12, 9, 7, 3, 10, 5, 0],
        [15, 12, 8, 2, 4, 9, 1, 7, 5, 11, 3, 14, 10, 0, 6, 13],
    ],
    [
        [15, 1, 8, 14, 6, 11, 3, 4, 9, 7, 2, 13, 12, 0, 5, 10],
        [3, 13, 4, 7, 15, 2, 8, 14, 12, 0, 1, 10, 6, 9, 11, 5],
        [0, 14, 7, 11, 10, 4, 13, 1, 5, 8, 12, 6, 9, 3, 2, 15],
        [13, 8, 10, 1, 3, 15, 4, 2, 11, 6, 7, 12, 0, 5, 14, 9],
    ],
    [
        [10, 0, 9, 14, 6, 3, 15, 5, 1, 13, 12, 7, 11, 4, 2, 8],
        [13, 7, 0, 9, 3, 4, 6, 10, 2, 8, 5, 14, 12, 11, 15, 1],
        [13, 6, 4, 9, 8, 15, 3, 0, 11, 1, 2, 12, 5, 10, 14, 7],
        [1, 10, 13, 0, 6, 9, 8, 7, 4, 15, 14, 3, 11, 5, 2, 12],
    ],
    [
        [7, 13, 14, 3, 0, 6, 9, 10, 1, 2, 8, 5, 11, 12, 4, 15],
        [13, 8, 11, 5, 6, 15, 0, 3, 4, 7, 2, 12, 1, 10, 14, 9],
        [10, 6, 9, 0, 12, 11, 7, 13, 15, 1, 3, 14, 5, 2, 8, 4],
        [3, 15, 0, 6, 10, 1, 13, 8, 9, 4, 5, 11, 12, 7, 2, 14],
    ],
    [
        [2, 12, 4, 1, 7, 10, 11, 6, 8, 5, 3, 15, 13, 0, 14, 9],
        [14, 11, 2, 12, 4, 7, 13, 1, 5, 0, 15, 10, 3, 9, 8, 6],
        [4, 2, 1, 11, 10, 13, 7, 8, 15, 9, 12, 5, 6, 3, 0, 14],
        [11, 8, 12, 7, 1, 14, 2, 13, 6, 15, 0, 9, 10, 4, 5, 3],
    ],
    [
        [12, 1, 10, 15, 9, 2, 6, 8, 0, 13, 3, 4, 14, 7, 5, 11],
        [10, 15, 4, 2, 7, 12, 9, 5, 6, 1, 13, 14, 0, 11, 3, 8],
        [9, 14, 15, 5, 2, 8, 12, 3, 7, 0, 4, 10, 1, 13, 11, 6],
        [4, 3, 2, 12, 9, 5, 15, 10, 11, 14, 1, 7, 6, 0, 8, 13],
    ],
    [
        [4, 11, 2, 14, 15, 0, 8, 13, 3, 12, 9, 7, 5, 10, 6, 1],
        [13, 0, 11, 7, 4, 9, 1, 10, 14, 3, 5, 12, 2, 15, 8, 6],
        [1, 4, 11, 13, 12, 3, 7, 14, 10, 15, 6, 8, 0, 5, 9, 2],
        [6, 11, 13, 8, 1, 4, 10, 7, 9, 5, 0, 15, 14, 2, 3, 12],
    ],
    [
        [13, 2, 8, 4, 6, 15, 11, 1, 10, 9, 3, 14, 5, 0, 12, 7],
        [1, 15, 13, 8, 10, 3, 7, 4, 12, 5, 6, 11, 0, 14, 9, 2],
        [7, 11, 4, 1, 9, 12, 14, 2, 0, 6, 10, 13, 15, 3, 5, 8],
        [2, 1, 14, 7, 4, 10, 8, 13, 15, 12, 9, 0, 3, 5, 6, 11],
    ],
];

fn des_key_schedule(key: [u8; 8]) -> [u64; 16] {
    let k = u64::from_be_bytes(key);
    let mut pc1 = 0u64;
    for (i, &pos) in DES_PC1.iter().enumerate() {
        let bit = (k >> (64 - pos)) & 1;
        pc1 |= bit << (55 - i);
    }
    let mut c = ((pc1 >> 28) & 0x0FFF_FFFF) as u32;
    let mut d = (pc1 & 0x0FFF_FFFF) as u32;
    let mut subkeys = [0u64; 16];
    for (round, &shift) in DES_SHIFTS.iter().enumerate() {
        c = ((c << shift) | (c >> (28 - shift))) & 0x0FFF_FFFF;
        d = ((d << shift) | (d >> (28 - shift))) & 0x0FFF_FFFF;
        let cd = (u64::from(c) << 28) | u64::from(d);
        let mut sk = 0u64;
        for (i, &pos) in DES_PC2.iter().enumerate() {
            let bit = (cd >> (56 - pos)) & 1;
            sk |= bit << (47 - i);
        }
        subkeys[round] = sk;
    }
    subkeys
}

fn des_f(r: u32, k: u64) -> u32 {
    let mut e = 0u64;
    for (i, &pos) in DES_E.iter().enumerate() {
        let bit = u64::from((r >> (32 - pos)) & 1);
        e |= bit << (47 - i);
    }
    let x = e ^ k;
    let mut s_out = 0u32;
    for (box_idx, sbox) in DES_SBOX.iter().enumerate() {
        let six = ((x >> (42 - box_idx * 6)) & 0x3F) as usize;
        let row = ((six & 0x20) >> 4) | (six & 0x01);
        let col = (six >> 1) & 0x0F;
        let val = u32::from(sbox[row][col]);
        s_out |= val << (28 - box_idx * 4);
    }
    let mut p_out = 0u32;
    for (i, &pos) in DES_P.iter().enumerate() {
        let bit = (s_out >> (32 - pos)) & 1;
        p_out |= bit << (31 - i);
    }
    p_out
}

fn des_block(block: u64, subkeys: &[u64; 16], decrypt: bool) -> u64 {
    let mut ip = 0u64;
    for (i, &pos) in DES_IP.iter().enumerate() {
        let bit = (block >> (64 - pos)) & 1;
        ip |= bit << (63 - i);
    }
    let mut l = (ip >> 32) as u32;
    let mut r = (ip & 0xFFFF_FFFF) as u32;
    for round in 0..16usize {
        let k = if decrypt {
            subkeys[15 - round]
        } else {
            subkeys[round]
        };
        let next_l = r;
        let next_r = l ^ des_f(r, k);
        l = next_l;
        r = next_r;
    }
    let pre_fp = (u64::from(r) << 32) | u64::from(l);
    let mut fp = 0u64;
    for (i, &pos) in DES_FP.iter().enumerate() {
        let bit = (pre_fp >> (64 - pos)) & 1;
        fp |= bit << (63 - i);
    }
    fp
}

/// 24-byte 3-Key Triple DES (`3des` EDE3) key schedule (§7.1).
#[derive(Debug, Clone)]
pub struct Des3Key {
    k1: [u64; 16],
    k2: [u64; 16],
    k3: [u64; 16],
}

impl Des3Key {
    /// Expands a 24-byte `(K1, K2, K3)` Triple-DES key into round schedules (§7.1).
    #[must_use]
    pub fn new(key_bytes: &[u8; 24]) -> Self {
        let mut k1_b = [0u8; 8];
        let mut k2_b = [0u8; 8];
        let mut k3_b = [0u8; 8];
        k1_b.copy_from_slice(&key_bytes[0..8]);
        k2_b.copy_from_slice(&key_bytes[8..16]);
        k3_b.copy_from_slice(&key_bytes[16..24]);
        Self {
            k1: des_key_schedule(k1_b),
            k2: des_key_schedule(k2_b),
            k3: des_key_schedule(k3_b),
        }
    }

    /// Encrypts a single 64-bit block with EDE3 (`E_K3(D_K2(E_K1(block)))`).
    #[must_use]
    pub fn encrypt_block(&self, block: u64) -> u64 {
        let s1 = des_block(block, &self.k1, false);
        let s2 = des_block(s1, &self.k2, true);
        des_block(s2, &self.k3, false)
    }

    /// Decrypts a single 64-bit block with EDE3 (`D_K1(E_K2(D_K3(block)))`).
    #[must_use]
    pub fn decrypt_block(&self, block: u64) -> u64 {
        let s1 = des_block(block, &self.k3, true);
        let s2 = des_block(s1, &self.k2, false);
        des_block(s2, &self.k1, true)
    }
}

/// Encrypts `plaintext` with 3-Key Triple DES CBC (`IV = [0u8; 8]`) and PKCS#7 padding (§7.1).
#[must_use]
pub fn des3_cbc_encrypt(key: &[u8; 24], plaintext: &[u8]) -> Vec<u8> {
    let ks = Des3Key::new(key);
    let pad_len = 8 - (plaintext.len() % 8);
    let total_len = plaintext.len() + pad_len;
    let mut padded = Vec::with_capacity(total_len);
    padded.extend_from_slice(plaintext);
    padded.resize(total_len, pad_len as u8);

    let mut out = Vec::with_capacity(total_len);
    let mut prev = 0u64; // Zero IV (§7.1)
    for &chunk in padded.as_chunks::<8>().0 {
        let blk = u64::from_be_bytes(chunk);
        let enc = ks.encrypt_block(blk ^ prev);
        out.extend_from_slice(&enc.to_be_bytes());
        prev = enc;
    }
    out
}

/// Decrypts `ciphertext` with 3-Key Triple DES CBC (`IV = [0u8; 8]`) and strips PKCS#7 padding (§7.1).
///
/// # Errors
///
/// Returns [`SessionError::CiphertextNotBlockAligned`] if `ciphertext` is empty or not a multiple of 8 bytes,
/// or [`SessionError::InvalidPkcs7Padding`] if PKCS#7 padding is invalid.
pub fn des3_cbc_decrypt(key: &[u8; 24], ciphertext: &[u8]) -> Result<Vec<u8>, SessionError> {
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(8) {
        return Err(SessionError::CiphertextNotBlockAligned {
            len: ciphertext.len(),
        });
    }
    let ks = Des3Key::new(key);
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = 0u64; // Zero IV (§7.1)
    for &chunk in ciphertext.as_chunks::<8>().0 {
        let blk = u64::from_be_bytes(chunk);
        let dec = ks.decrypt_block(blk) ^ prev;
        out.extend_from_slice(&dec.to_be_bytes());
        prev = blk;
    }
    let Some(&pad_byte) = out.last() else {
        return Err(SessionError::InvalidPkcs7Padding);
    };
    let pad_len = usize::from(pad_byte);
    // §7.1: the receiver verifies `padLen < len`, so a block that is all padding is rejected.
    if pad_len == 0 || pad_len > 8 || pad_len >= out.len() {
        return Err(SessionError::InvalidPkcs7Padding);
    }
    for &b in &out[out.len() - pad_len..] {
        if b != pad_byte {
            return Err(SessionError::InvalidPkcs7Padding);
        }
    }
    out.truncate(out.len() - pad_len);
    Ok(out)
}

// ---------------------------------------------------------------------------
// §7.2 ISAAC 32-Bit PRNG for Checksum Masking
// ---------------------------------------------------------------------------

#[allow(clippy::many_single_char_names, clippy::too_many_arguments)]
fn isaac_mix(
    a: &mut u32,
    b: &mut u32,
    c: &mut u32,
    d: &mut u32,
    e: &mut u32,
    f: &mut u32,
    g: &mut u32,
    h: &mut u32,
) {
    *a ^= b.wrapping_shl(11);
    *d = d.wrapping_add(*a);
    *b = b.wrapping_add(*c);

    *b ^= c.wrapping_shr(2);
    *e = e.wrapping_add(*b);
    *c = c.wrapping_add(*d);

    *c ^= d.wrapping_shl(8);
    *f = f.wrapping_add(*c);
    *d = d.wrapping_add(*e);

    *d ^= e.wrapping_shr(16);
    *g = g.wrapping_add(*d);
    *e = e.wrapping_add(*f);

    *e ^= f.wrapping_shl(10);
    *h = h.wrapping_add(*e);
    *f = f.wrapping_add(*g);

    *f ^= g.wrapping_shr(4);
    *a = a.wrapping_add(*f);
    *g = g.wrapping_add(*h);

    *g ^= h.wrapping_shl(8);
    *b = b.wrapping_add(*g);
    *h = h.wrapping_add(*a);

    *h ^= a.wrapping_shr(9);
    *c = c.wrapping_add(*h);
    *a = a.wrapping_add(*b);
}

/// Directional 32-bit ISAAC PRNG instance for `MaskedChecksumFlag` (`0x00000002`) checksum XOR masking (§7.2).
#[derive(Debug, Clone)]
pub struct IsaacCipher {
    randrsl: [u32; 256],
    randcnt: u32,
    mm: [u32; 256],
    aa: u32,
    bb: u32,
    cc: u32,
}

impl IsaacCipher {
    /// Initializes an ISAAC instance from a 32-bit seed (`outgoingSeed` or `incomingSeed`) per §7.2 item 2:
    /// sets `aa = bb = cc = seed` with `randrsl = [0; 256]`, runs 4 `mix` rounds on the
    /// golden-ratio constant `0x9E3779B9`, performs Pass 1 and Pass 2 over `mm`, and
    /// executes the initial `isaac()` block generation setting `randcnt = 256`.
    #[must_use]
    pub fn from_seed(seed: u32) -> Self {
        let mut state = Self {
            randrsl: [0u32; 256],
            randcnt: 0,
            mm: [0u32; 256],
            aa: seed,
            bb: seed,
            cc: seed,
        };
        let mut a = 0x9E37_79B9u32;
        let mut b = 0x9E37_79B9u32;
        let mut c = 0x9E37_79B9u32;
        let mut d = 0x9E37_79B9u32;
        let mut e = 0x9E37_79B9u32;
        let mut f = 0x9E37_79B9u32;
        let mut g = 0x9E37_79B9u32;
        let mut h = 0x9E37_79B9u32;

        for _ in 0..4 {
            isaac_mix(
                &mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
            );
        }

        // Pass 1: mix in randrsl[i..i+8] (all zeros per §7.2 item 2)
        let mut i = 0usize;
        while i < 256 {
            a = a.wrapping_add(state.randrsl[i]);
            b = b.wrapping_add(state.randrsl[i + 1]);
            c = c.wrapping_add(state.randrsl[i + 2]);
            d = d.wrapping_add(state.randrsl[i + 3]);
            e = e.wrapping_add(state.randrsl[i + 4]);
            f = f.wrapping_add(state.randrsl[i + 5]);
            g = g.wrapping_add(state.randrsl[i + 6]);
            h = h.wrapping_add(state.randrsl[i + 7]);
            isaac_mix(
                &mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
            );
            state.mm[i] = a;
            state.mm[i + 1] = b;
            state.mm[i + 2] = c;
            state.mm[i + 3] = d;
            state.mm[i + 4] = e;
            state.mm[i + 5] = f;
            state.mm[i + 6] = g;
            state.mm[i + 7] = h;
            i += 8;
        }

        // Pass 2: mix in mm[i..i+8]
        i = 0;
        while i < 256 {
            a = a.wrapping_add(state.mm[i]);
            b = b.wrapping_add(state.mm[i + 1]);
            c = c.wrapping_add(state.mm[i + 2]);
            d = d.wrapping_add(state.mm[i + 3]);
            e = e.wrapping_add(state.mm[i + 4]);
            f = f.wrapping_add(state.mm[i + 5]);
            g = g.wrapping_add(state.mm[i + 6]);
            h = h.wrapping_add(state.mm[i + 7]);
            isaac_mix(
                &mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
            );
            state.mm[i] = a;
            state.mm[i + 1] = b;
            state.mm[i + 2] = c;
            state.mm[i + 3] = d;
            state.mm[i + 4] = e;
            state.mm[i + 5] = f;
            state.mm[i + 6] = g;
            state.mm[i + 7] = h;
            i += 8;
        }

        state.isaac();
        state.randcnt = 256;
        state
    }

    /// Runs one 256-word ISAAC block generation round (§7.2 item 3).
    pub fn isaac(&mut self) {
        self.cc = self.cc.wrapping_add(1);
        self.bb = self.bb.wrapping_add(self.cc);

        for i in 0..256usize {
            let x = self.mm[i];
            match i & 3 {
                0 => self.aa ^= self.aa.wrapping_shl(13),
                1 => self.aa ^= self.aa.wrapping_shr(6),
                2 => self.aa ^= self.aa.wrapping_shl(2),
                _ => self.aa ^= self.aa.wrapping_shr(16),
            }
            self.aa = self.mm[(i + 128) & 0xFF].wrapping_add(self.aa);
            let y = self.mm[((x >> 2) as usize) & 0xFF]
                .wrapping_add(self.aa)
                .wrapping_add(self.bb);
            self.mm[i] = y;
            self.bb = self.mm[((y >> 10) as usize) & 0xFF].wrapping_add(x);
            self.randrsl[i] = self.bb;
        }
    }

    /// Returns the next 32-bit XOR mask in reverse index order (`255` down to `0`),
    /// regenerating via `isaac()` when `randcnt` reaches `0` (§7.2 item 4).
    pub fn next_mask(&mut self) -> u32 {
        if self.randcnt == 0 {
            self.isaac();
            self.randcnt = 256;
        }
        self.randcnt -= 1;
        self.randrsl[self.randcnt as usize]
    }
}

// ---------------------------------------------------------------------------
// §4.3, §5.2–§5.5, §6.2–§6.7, §8.1–§8.2 Optional Headers
// ---------------------------------------------------------------------------

/// The protocol stamp that opens every `SessionAuthHeader.netVersion` (`"180804"`, §6.2).
pub const NET_VERSION_STAMP: &str = "180804";

/// `authType` on the primary login/world connection (`0x00000001`, §6.2.1).
pub const AUTH_TYPE_PRIMARY: u32 = 0x0000_0001;
/// `authType` on the secondary chat-server connection (`0x40000002`, §6.2.1).
pub const AUTH_TYPE_CHAT: u32 = 0x4000_0002;
/// `authFlags` bit 1 (`0x02`, `HasSecondString`): `secondStr` is present (§6.2.1).
pub const AUTH_FLAG_HAS_SECOND_STRING: u32 = 0x02;
/// `authFlags` bit 2 (`0x04`, `HasExtraBlob`): `extraBlob` is present (§6.2.1).
pub const AUTH_FLAG_HAS_EXTRA_BLOB: u32 = 0x04;
/// The `(authType, authFlags)` modes of the §6.2.1 selection rule, the only ones Tessera
/// emits or accepts:
///
/// 1. the primary connection with a ticket: `(0x00000001, 0x04)`, or `(0x00000001, 0x06)`
///    with a password as well;
/// 2. the chat connection with a ticket: `(0x40000002, 0x04)`;
/// 3. account only: `(0x00000001, 0x00)` or `(0x40000002, 0x00)`.
///
/// Bit 0 is `0` on the wire and the other bits name no field Tessera can serialize, so any
/// other combination is refused (engineering choice: §6.2.1 rejects only bits above
/// `0x800`).
pub const SUPPORTED_AUTH_MODES: [(u32, u32); 5] = [
    (AUTH_TYPE_PRIMARY, AUTH_FLAG_HAS_EXTRA_BLOB),
    (
        AUTH_TYPE_PRIMARY,
        AUTH_FLAG_HAS_EXTRA_BLOB | AUTH_FLAG_HAS_SECOND_STRING,
    ),
    (AUTH_TYPE_CHAT, AUTH_FLAG_HAS_EXTRA_BLOB),
    (AUTH_TYPE_PRIMARY, 0),
    (AUTH_TYPE_CHAT, 0),
];

/// Whether `(auth_type, auth_flags)` is one of [`SUPPORTED_AUTH_MODES`].
#[must_use]
pub fn is_supported_auth_mode(auth_type: u32, auth_flags: u32) -> bool {
    SUPPORTED_AUTH_MODES.contains(&(auth_type, auth_flags))
}

/// Longest GLS ticket string accepted (`4,095` characters, the registry ticket buffer of
/// `official-login.md` §4.1, applied to every ticket: engineering choice, spec silent).
pub const MAX_TICKET_STRING_LEN: usize = 4095;

/// Longest `SessionAuthHeader.netVersion` accepted or built: `"180804_"`, `"netver:"`, a
/// network version of at most `official-login.md`'s 64 characters, `"; didver:"` and the
/// 36-character GUID (`123` bytes), rounded up. Engineering choice, spec silent.
pub const MAX_SESSION_NET_VERSION_LEN: usize = 128;

/// The chat connection's `netVersion` (§6.2; `official-messages-play.md` §6.6.2): the text
/// observed for one client build (`OBS-L2`), sent with the one-byte count `0x12`.
///
/// **Interim and build-specific.** Its runtime source is `OPEN-SESSION-11`: it is not a
/// release-independent constant, and a later client build may send another text. Tessera
/// sends it until the spec names where the value comes from.
pub const CHAT_NET_VERSION_INTERIM: &str = "180804_chat_fe_2_3";

/// Which connection a `SessionAuthHeader` opens (§6.2, §6.2.1): it decides `netVersion` and
/// `authType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthConnection {
    /// The primary login/world connection: `netVersion` `"180804_"` + the version stamp,
    /// `authType 0x00000001`.
    Primary,
    /// The secondary chat-server connection: `netVersion` [`CHAT_NET_VERSION_INTERIM`],
    /// `authType 0x40000002`.
    Chat,
}

impl AuthConnection {
    /// The `authType` of this connection (§6.2.1).
    #[must_use]
    pub const fn auth_type(self) -> u32 {
        match self {
            Self::Primary => AUTH_TYPE_PRIMARY,
            Self::Chat => AUTH_TYPE_CHAT,
        }
    }

    /// The `netVersion` text of this connection (§6.2): `"180804_"` followed by
    /// `version_string` (`"netver:<netver_string>; didver:<didver_string>"`,
    /// `official-login.md` §2.4.1) on the primary connection, the interim
    /// [`CHAT_NET_VERSION_INTERIM`] on the chat connection (where `version_string` is not
    /// used).
    #[must_use]
    pub fn net_version(self, version_string: &str) -> String {
        match self {
            Self::Primary => format!("{NET_VERSION_STAMP}_{version_string}"),
            Self::Chat => CHAT_NET_VERSION_INTERIM.to_owned(),
        }
    }
}

/// Whether `text` is a `netVersion` Tessera sends or accepts (§6.2): `"180804_"` followed
/// by printable ASCII (`0x20..=0x7E`), at most [`MAX_SESSION_NET_VERSION_LEN`] bytes. The
/// primary text and [`CHAT_NET_VERSION_INTERIM`] both have this form; the earlier bare
/// `"180804"` chat text is withdrawn (§6.2.2).
#[must_use]
pub fn is_valid_net_version(text: &str) -> bool {
    if text.len() > MAX_SESSION_NET_VERSION_LEN {
        return false;
    }
    match text.strip_prefix(NET_VERSION_STAMP) {
        Some(rest) => rest.strip_prefix('_').is_some_and(|stamp| {
            !stamp.is_empty() && stamp.bytes().all(|b| (0x20..=0x7E).contains(&b))
        }),
        None => false,
    }
}

/// Checks a GLS ticket string before it is sent as text in `extraBlob` (§6.2.1; never
/// Base64-decoded): `1` to [`MAX_TICKET_STRING_LEN`] printable ASCII characters
/// (`0x21..=0x7E`, so no whitespace or NUL). Engineering choice: §6.2.1 calls it a raw ASCII
/// Base64 string and gives no further rule.
///
/// # Errors
///
/// [`SessionError::InvalidTicketEncoding`] for any other string.
pub fn check_ticket_text(ticket: &str) -> Result<(), SessionError> {
    let ok = (1..=MAX_TICKET_STRING_LEN).contains(&ticket.len())
        && ticket.bytes().all(|b| (0x21..=0x7E).contains(&b));
    if ok {
        Ok(())
    } else {
        Err(SessionError::InvalidTicketEncoding)
    }
}

/// Writes an `AccountString`: 4-tier `VariableU32` UTF-16 code-unit count + UTF-16LE units
/// (§6.2.1).
///
/// The units are written straight from the string, with no intermediate copy: an
/// `AccountString` may be a credential (`official-login.md` §6.3).
fn encode_wstring(s: &str, out: &mut Vec<u8>) -> Result<(), SessionError> {
    let units = u32::try_from(s.encode_utf16().count())
        .map_err(|_| SessionError::VariableU32OutOfRange { value: u32::MAX })?;
    encode_variable_u32(units, out)?;
    for unit in s.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(())
}

/// The encoded length of [`encode_wstring`]'s output for `s`.
fn wstring_encoded_len(s: &str) -> usize {
    let units = s.encode_utf16().count();
    variable_u32_len(units as u32).saturating_add(units.saturating_mul(2))
}

/// The byte length of the transport compact integer encoding of `value` (§5.6.1); a
/// value above [`MAX_VARIABLE_U32`], which [`encode_variable_u32`] refuses, is measured
/// as four bytes.
fn variable_u32_len(value: u32) -> usize {
    if value < 0x80 {
        1
    } else if value < 0x4000 {
        2
    } else {
        4
    }
}

/// A byte buffer wiped over its whole capacity when it is dropped, if it may hold
/// credentials (`official-login.md` §6.3). Capacities are reserved up front, so no
/// reallocation frees an unwiped copy.
struct SecretBuf {
    buf: Vec<u8>,
    secret: bool,
}

impl SecretBuf {
    fn with_capacity(capacity: usize, secret: bool) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            secret,
        }
    }
}

impl Drop for SecretBuf {
    fn drop(&mut self) {
        if self.secret {
            crate::login::zeroize_vec(&mut self.buf);
        }
    }
}

/// Reads an `AccountString` (§6.2.1) or a session/transport UTF-16LE string (§9.3).
fn decode_wstring(bytes: &[u8]) -> Result<(String, usize), SessionError> {
    let (char_count, pd_len) = decode_variable_u32(bytes)?;
    let byte_len = (char_count as usize)
        .checked_mul(2)
        .ok_or(SessionError::InvalidUtf16String)?;
    let total = pd_len
        .checked_add(byte_len)
        .ok_or(SessionError::InvalidUtf16String)?;
    let Some(raw) = bytes.get(pd_len..total) else {
        return Err(SessionError::InvalidUtf16String);
    };
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&chunk| u16::from_le_bytes(chunk))
        .collect();
    let s = String::from_utf16(&units).map_err(|_| SessionError::InvalidUtf16String)?;
    Ok((s, total))
}

/// Credentials payload of a `SessionAuthHeader` `authBlock` (`authFlags` bit 0 clear:
/// unencrypted, §6.2.1).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionAuthCredentials {
    /// `accountOrTicketStr` (`AccountString`): the account / subscription identifier
    /// (`--account`).
    pub account_or_ticket: String,
    /// Optional `secondStr` (`AccountString`, present when `authFlags & 0x02 != 0`): the
    /// password (`--password`).
    pub second_string: Option<String>,
    /// Optional `extraBlob` (present when `authFlags & 0x04 != 0`): the GLS ticket as its
    /// raw ASCII Base64 text, never decoded. On the wire it is `u32` LE `extraBlobLen`, then
    /// a `VariableU32` byte length and the ticket's ASCII bytes (§6.2.1).
    pub ticket: Option<String>,
}

impl std::fmt::Debug for SessionAuthCredentials {
    // The account name, password and ticket are credentials (`official-login.md` §6.3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuthCredentials")
            .field("account_or_ticket", &"<REDACTED>")
            .field(
                "second_string",
                &self.second_string.as_ref().map(|_| "<REDACTED>"),
            )
            .field("ticket_len", &self.ticket.as_ref().map(String::len))
            .finish()
    }
}

impl Drop for SessionAuthCredentials {
    // The account name, the password and the ticket are wiped when the credentials go away
    // (`official-login.md` §6.3).
    fn drop(&mut self) {
        crate::login::zeroize_string(&mut self.account_or_ticket);
        if let Some(second) = self.second_string.as_mut() {
            crate::login::zeroize_string(second);
        }
        if let Some(ticket) = self.ticket.as_mut() {
            crate::login::zeroize_string(ticket);
        }
    }
}

/// The `extraBlobLen` of a ticket of `len` ASCII bytes: its `VariableU32` length prefix
/// plus the bytes (§6.2.1; `146` for a 144-character ticket).
fn extra_blob_len(len: usize) -> usize {
    variable_u32_len(u32::try_from(len).unwrap_or(u32::MAX)).saturating_add(len)
}

impl SessionAuthCredentials {
    /// The byte length of [`Self::encode`]'s output.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        let mut len = wstring_encoded_len(&self.account_or_ticket);
        if let Some(second) = &self.second_string {
            len = len.saturating_add(wstring_encoded_len(second));
        }
        if let Some(ticket) = &self.ticket {
            len = len
                .saturating_add(4)
                .saturating_add(extra_blob_len(ticket.len()));
        }
        len
    }

    /// Serializes `credentialsPayload` in the §6.2.1 field order, with no padding anywhere:
    /// `accountOrTicketStr`, `secondStr` when present, and `extraBlob` (`u32` LE
    /// `extraBlobLen`, the ticket's `VariableU32` byte length, its ASCII bytes) when
    /// present. The buffer is reserved at its exact size, so no reallocation leaves a copy
    /// of the credentials behind (`official-login.md` §6.3); the caller wipes it.
    ///
    /// # Errors
    ///
    /// [`SessionError::InvalidTicketEncoding`] for a ticket that fails
    /// [`check_ticket_text`].
    pub fn encode(&self) -> Result<Vec<u8>, SessionError> {
        if let Some(ticket) = &self.ticket {
            check_ticket_text(ticket)?;
        }
        let mut buf = Vec::with_capacity(self.encoded_len());
        let written = (|| {
            encode_wstring(&self.account_or_ticket, &mut buf)?;
            if let Some(second) = &self.second_string {
                encode_wstring(second, &mut buf)?;
            }
            if let Some(ticket) = &self.ticket {
                let blob_len = u32::try_from(extra_blob_len(ticket.len()))
                    .map_err(|_| SessionError::InvalidTicketEncoding)?;
                let text_len =
                    u32::try_from(ticket.len()).map_err(|_| SessionError::InvalidTicketEncoding)?;
                buf.extend_from_slice(&blob_len.to_le_bytes());
                encode_variable_u32(text_len, &mut buf)?;
                buf.extend_from_slice(ticket.as_bytes());
            }
            Ok(())
        })();
        if let Err(why) = written {
            // What was written holds credentials (`official-login.md` §6.3).
            crate::login::zeroize_vec(&mut buf);
            return Err(why);
        }
        Ok(buf)
    }

    /// Decodes `credentialsPayload` for the given `auth_flags` (§6.2.1). `extraBlob` must
    /// hold exactly one `VariableU32`-prefixed ASCII string (the ticket text).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if any string is truncated or `extraBlob` is not one
    /// printable ASCII ticket string filling `extraBlobLen` exactly.
    pub fn decode(bytes: &[u8], auth_flags: u32) -> Result<(Self, usize), SessionError> {
        let (account_or_ticket, mut cursor) = decode_wstring(bytes)?;
        // Built before the rest is checked: its `Drop` wipes what was read on every error
        // path (`official-login.md` §6.3).
        let mut creds = Self {
            account_or_ticket,
            second_string: None,
            ticket: None,
        };
        if (auth_flags & AUTH_FLAG_HAS_SECOND_STRING) != 0 {
            let rest = bytes.get(cursor..).ok_or(SessionError::InvalidAuthBlock)?;
            let (s2, n2) = decode_wstring(rest)?;
            cursor += n2;
            creds.second_string = Some(s2);
        }
        if (auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB) != 0 {
            let len_bytes: [u8; 4] = bytes
                .get(cursor..cursor.saturating_add(4))
                .and_then(|b| b.try_into().ok())
                .ok_or(SessionError::InvalidAuthBlock)?;
            let blob_len = u32::from_le_bytes(len_bytes) as usize;
            cursor += 4;
            let blob = bytes
                .get(cursor..cursor.saturating_add(blob_len))
                .ok_or(SessionError::InvalidAuthBlock)?;
            let (text_len, prefix) =
                decode_variable_u32(blob).map_err(|_| SessionError::InvalidAuthBlock)?;
            let text = blob
                .get(prefix..)
                .filter(|t| t.len() == text_len as usize)
                .ok_or(SessionError::InvalidAuthBlock)?;
            let text = std::str::from_utf8(text).map_err(|_| SessionError::InvalidAuthBlock)?;
            check_ticket_text(text)?;
            cursor += blob_len;
            creds.ticket = Some(text.to_owned());
        }
        Ok((creds, cursor))
    }
}

/// `SessionAuthHeader` (`0x00010000`) optional header: the first client packet of a login,
/// world or chat connection (§6.2, §6.2.1). Its wire form is unaligned, with no padding
/// anywhere: `netVersion` (`VariableU32` byte length + ASCII, no NUL), `u32` LE
/// `authBlockLen` (the exact length of `authBlock`), and `authBlock` (`authType`,
/// `authFlags`, `clientTimestamp`, `credentialsPayload`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAuthHeader {
    /// `netVersion`: `"180804_netver:<netver_string>; didver:<didver_string>"` on the
    /// primary connection, [`CHAT_NET_VERSION_INTERIM`] on the chat connection ([`AuthConnection::net_version`]).
    pub net_version: String,
    /// `authType` (`u32` LE at `authBlock + 0x00`): [`AUTH_TYPE_PRIMARY`] or
    /// [`AUTH_TYPE_CHAT`].
    pub auth_type: u32,
    /// `authFlags` (`u32` LE at `authBlock + 0x04`). Together with `auth_type` it must be
    /// one of [`SUPPORTED_AUTH_MODES`], and `credentials` must carry exactly the fields it
    /// names.
    pub auth_flags: u32,
    /// `clientTimestamp`: Unix time in seconds when the transport was initialised
    /// (`u32` LE at `authBlock + 0x08`).
    pub client_timestamp: u32,
    /// `credentialsPayload` (unencrypted, `authFlags` bit 0 is `0`).
    pub credentials: SessionAuthCredentials,
}

impl SessionAuthHeader {
    /// Builds the header by the deterministic selection rule (§6.2.1;
    /// `official-login.md` §4.2.1) for `connection`:
    ///
    /// - `netVersion` is [`AuthConnection::net_version`] of `version_string` (the
    ///   `"netver:<netver_string>; didver:<didver_string>"` of `official-login.md` §2.4.1;
    ///   not used on the chat connection);
    /// - `authType` is [`AuthConnection::auth_type`];
    /// - with a ticket, `authFlags` has `HasExtraBlob` (`0x04`) and `extraBlob` carries the
    ///   ticket text undecoded; with a password as well, `HasSecondString` (`0x02`) and
    ///   `secondStr`; with neither, `authFlags` is `0` (account only).
    ///
    /// An empty ticket or password counts as none.
    ///
    /// # Errors
    ///
    /// [`SessionError::InvalidTicketEncoding`] for a ticket that fails
    /// [`check_ticket_text`], [`SessionError::InvalidSessionAuthVersion`] for a
    /// `netVersion` that fails [`is_valid_net_version`], and
    /// [`SessionError::UnsupportedAuthMode`] for a combination outside
    /// [`SUPPORTED_AUTH_MODES`] (a password without a ticket, or on the chat connection).
    pub fn for_connection(
        connection: AuthConnection,
        version_string: &str,
        account: &str,
        ticket: Option<&str>,
        password: Option<&str>,
        client_timestamp: u32,
    ) -> Result<Self, SessionError> {
        let net_version = connection.net_version(version_string);
        if !is_valid_net_version(&net_version) {
            return Err(SessionError::InvalidSessionAuthVersion);
        }
        let ticket = ticket.filter(|t| !t.is_empty());
        let password = password.filter(|p| !p.is_empty());
        if let Some(t) = ticket {
            check_ticket_text(t)?;
        }
        let mut auth_flags = 0;
        if ticket.is_some() {
            auth_flags |= AUTH_FLAG_HAS_EXTRA_BLOB;
        }
        if password.is_some() {
            auth_flags |= AUTH_FLAG_HAS_SECOND_STRING;
        }
        let auth_type = connection.auth_type();
        if !is_supported_auth_mode(auth_type, auth_flags) {
            return Err(SessionError::UnsupportedAuthMode {
                auth_type,
                auth_flags,
            });
        }
        Ok(Self {
            net_version,
            auth_type,
            auth_flags,
            client_timestamp,
            credentials: SessionAuthCredentials {
                account_or_ticket: account.to_owned(),
                second_string: password.map(str::to_owned),
                ticket: ticket.map(str::to_owned),
            },
        })
    }

    /// The byte length of `authBlock`: the 12-byte prefix and the credentials, unpadded.
    fn auth_block_len(&self) -> usize {
        self.credentials.encoded_len().saturating_add(12)
    }

    /// The byte length of [`Self::encode`]'s output: `netVersion` with its `VariableU32`
    /// prefix, the `4`-byte `authBlockLen` and the unpadded `authBlock` (§6.2).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        let version = self.net_version.len();
        variable_u32_len(u32::try_from(version).unwrap_or(u32::MAX))
            .saturating_add(version)
            .saturating_add(4)
            .saturating_add(self.auth_block_len())
    }

    /// Serializes this header into `out` (§6.2): `netVersion` (`VariableU32` byte length +
    /// ASCII, no NUL), `authBlockLen` (`u32` LE, the exact length) and `authBlock`, with no
    /// padding anywhere. `auth_type` and `auth_flags` are written as they are; they are
    /// never replaced.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidSessionAuthVersion`] for a `net_version` that fails
    /// [`is_valid_net_version`], [`SessionError::UnsupportedAuthMode`] if
    /// `(auth_type, auth_flags)` is not one of [`SUPPORTED_AUTH_MODES`],
    /// [`SessionError::AuthCredentialsMismatch`] if the credentials do not carry exactly
    /// the optional fields `auth_flags` names, or [`SessionError::InvalidTicketEncoding`].
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        if !is_valid_net_version(&self.net_version) {
            return Err(SessionError::InvalidSessionAuthVersion);
        }
        if !is_supported_auth_mode(self.auth_type, self.auth_flags) {
            return Err(SessionError::UnsupportedAuthMode {
                auth_type: self.auth_type,
                auth_flags: self.auth_flags,
            });
        }
        let wants_second = (self.auth_flags & AUTH_FLAG_HAS_SECOND_STRING) != 0;
        let wants_extra = (self.auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB) != 0;
        if wants_second != self.credentials.second_string.is_some()
            || wants_extra != self.credentials.ticket.is_some()
        {
            return Err(SessionError::AuthCredentialsMismatch {
                auth_flags: self.auth_flags,
            });
        }
        let start = out.len();
        // `out` is reserved first, so writing the ticket into it never moves it; the
        // temporary buffer is reserved at its exact size (`official-login.md` §6.3).
        out.reserve(self.encoded_len());
        let version_len = u32::try_from(self.net_version.len())
            .map_err(|_| SessionError::InvalidSessionAuthVersion)?;
        encode_variable_u32(version_len, out)?;
        out.extend_from_slice(self.net_version.as_bytes());
        let mut credentials = self.credentials.encode()?;
        let Ok(block_len) = u32::try_from(credentials.len().saturating_add(12)) else {
            crate::login::zeroize_bytes(&mut credentials);
            return Err(SessionError::InvalidAuthBlock);
        };
        out.extend_from_slice(&block_len.to_le_bytes());
        out.extend_from_slice(&self.auth_type.to_le_bytes());
        out.extend_from_slice(&self.auth_flags.to_le_bytes());
        out.extend_from_slice(&self.client_timestamp.to_le_bytes());
        out.extend_from_slice(&credentials);
        // The temporary copy holds the ticket: wipe it (`official-login.md` §6.3).
        crate::login::zeroize_bytes(&mut credentials);
        Ok(out.len() - start)
    }

    /// Decodes a `SessionAuthHeader` from `bytes` (§6.2, §6.2.1), returning
    /// `(header, total_bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `netVersion` fails [`is_valid_net_version`], the header
    /// overruns `bytes`, `authBlockLen` does not match the block's content exactly, or
    /// `(authType, authFlags)` is not one of [`SUPPORTED_AUTH_MODES`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        let overrun = SessionError::OptionalHeaderOverrun {
            flag: FLAG_SESSION_AUTH,
        };
        let (version_len, prefix) = decode_variable_u32(bytes).map_err(|_| overrun.clone())?;
        let version_len = version_len as usize;
        if version_len > MAX_SESSION_NET_VERSION_LEN {
            return Err(SessionError::InvalidSessionAuthVersion);
        }
        let version_end = prefix.saturating_add(version_len);
        let version = bytes.get(prefix..version_end).ok_or(overrun.clone())?;
        let net_version = std::str::from_utf8(version)
            .ok()
            .filter(|v| is_valid_net_version(v))
            .ok_or(SessionError::InvalidSessionAuthVersion)?
            .to_owned();
        let len_bytes: [u8; 4] = bytes
            .get(version_end..version_end.saturating_add(4))
            .and_then(|b| b.try_into().ok())
            .ok_or(overrun.clone())?;
        let block_len = u32::from_le_bytes(len_bytes) as usize;
        if block_len < 12 {
            return Err(SessionError::InvalidAuthBlock);
        }
        let block_start = version_end + 4;
        let total_len = block_start
            .checked_add(block_len)
            .ok_or(SessionError::InvalidAuthBlock)?;
        let block = bytes.get(block_start..total_len).ok_or(overrun)?;
        let word = |at: usize| {
            block
                .get(at..at + 4)
                .and_then(|b| <[u8; 4]>::try_from(b).ok())
                .map_or(0, u32::from_le_bytes)
        };
        let auth_type = word(0);
        let auth_flags = word(4);
        let client_timestamp = word(8);
        if !is_supported_auth_mode(auth_type, auth_flags) {
            tracing::debug!(
                auth_type,
                auth_flags,
                "SessionAuthHeader with an unsupported authType / authFlags pair refused"
            );
            return Err(SessionError::UnsupportedAuthMode {
                auth_type,
                auth_flags,
            });
        }
        let rest = block.get(12..).unwrap_or_default();
        let (credentials, used) = SessionAuthCredentials::decode(rest, auth_flags)?;
        // `authBlockLen` is the exact length: no padding or other bytes follow (§6.2).
        if used != rest.len() {
            return Err(SessionError::InvalidAuthBlock);
        }
        Ok((
            Self {
                net_version,
                auth_type,
                auth_flags,
                client_timestamp,
                credentials,
            },
            total_len,
        ))
    }
}

/// `ReferralAuthHeader` (`0x00020000`, 8 bytes) optional header (§6.5): exactly the
/// referral cookie's eight bytes in reverse order, nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReferralAuthHeader {
    /// The eight payload bytes as they go on the wire: the `WorldReferralHeader` cookie
    /// reversed ([`Self::for_cookie`]).
    pub reversed_cookie: [u8; 8],
}

impl ReferralAuthHeader {
    /// The `ReferralAuthHeader` answering a `WorldReferralHeader` with `cookie` (its bytes
    /// `0..7`): the eight bytes reversed (§6.5, §10.2 Vector 8).
    #[must_use]
    pub fn for_cookie(cookie: [u8; 8]) -> Self {
        let mut reversed_cookie = cookie;
        reversed_cookie.reverse();
        Self { reversed_cookie }
    }
}

/// Converts a dictionary UUID between its on-disk mixed-endian form (record bytes
/// `16..31`) and the challenge's field-wise network form (§7.3): the first four-byte, the
/// next two-byte and the next two-byte fields are each reversed, the final eight bytes
/// are unchanged. The conversion is its own inverse.
#[must_use]
pub fn guid_fields_swapped(uuid: [u8; 16]) -> [u8; 16] {
    let mut out = uuid;
    out[0..4].reverse();
    out[4..6].reverse();
    out[6..8].reverse();
    out
}

/// `ConnectChallengeHeader` (`0x00040000`, 44 bytes) optional header (§6.3). The incoming
/// recipient ID is not in the payload: it is the challenge datagram header's `recId`
/// (§3.4, §6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectChallengeHeader {
    /// Payload bytes `0..7`: the server-clock candidate, IEEE-754 `f64` big-endian. Parsed
    /// and not used: its initialization use is `OPEN-SESSION-1`; the clock comes from
    /// `ClockSyncHeader` (§6.3, §8.1).
    pub clock_candidate: f64,
    /// Payload bytes `8..15`: the opaque challenge value, reversed into the accept's bytes
    /// `0..7` (§6.3, §6.4). Not a recipient ID or a seed.
    pub cookie: [u8; 8],
    /// Payload bytes `16..17`: unresolved (`OPEN-SESSION-1`); read and ignored, never copied
    /// into the accept or used as a seed or recipient (§6.3).
    pub reserved_16: [u8; 2],
    /// Payload bytes `18..19`, big-endian: the outgoing recipient ID, the `recId` of the
    /// accept and of every later client packet (§3.4, §6.3).
    pub outgoing_rec_id: u16,
    /// Payload bytes `20..23`, big-endian: the seed of the incoming (server-to-client)
    /// ISAAC generator (§6.3, §7.2).
    pub incoming_seed: u32,
    /// Payload bytes `24..27`, big-endian: the seed of the outgoing (client-to-server)
    /// ISAAC generator (§6.3, §7.2).
    pub outgoing_seed: u32,
    /// Payload bytes `28..43`: the incoming dictionary's UUID in field-wise network GUID
    /// order ([`guid_fields_swapped`] of its on-disk bytes, §6.3, §7.3).
    pub incoming_codec_guid: [u8; 16],
}

impl Default for ConnectChallengeHeader {
    fn default() -> Self {
        Self {
            clock_candidate: 0.0,
            cookie: [0u8; 8],
            reserved_16: [0u8; 2],
            outgoing_rec_id: 0,
            incoming_seed: 0,
            outgoing_seed: 0,
            incoming_codec_guid: [0u8; 16],
        }
    }
}

impl ConnectChallengeHeader {
    /// The byte length of the payload (`44`, §4.3).
    pub const LEN: usize = 44;

    /// The accept's first eight payload bytes as the `u64` little-endian `checkValue`: the
    /// cookie (bytes `8..15`) reversed (§6.4, §10.2 Vector 5).
    #[must_use]
    pub fn check_value(&self) -> u64 {
        u64::from_be_bytes(self.cookie)
    }

    /// The incoming dictionary UUID in its on-disk form (§7.3).
    #[must_use]
    pub fn incoming_codec_uuid(&self) -> [u8; 16] {
        guid_fields_swapped(self.incoming_codec_guid)
    }

    /// Serializes this 44-byte `ConnectChallengeHeader` into `out` (§6.3).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.clock_candidate.to_be_bytes());
        out.extend_from_slice(&self.cookie);
        out.extend_from_slice(&self.reserved_16);
        out.extend_from_slice(&self.outgoing_rec_id.to_be_bytes());
        out.extend_from_slice(&self.incoming_seed.to_be_bytes());
        out.extend_from_slice(&self.outgoing_seed.to_be_bytes());
        out.extend_from_slice(&self.incoming_codec_guid);
    }

    /// Decodes a 44-byte `ConnectChallengeHeader` optional header from `bytes` (§6.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 44`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        let Some(b) = bytes.first_chunk::<{ Self::LEN }>() else {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_CONNECT_CHALLENGE,
            });
        };
        let mut clock = [0u8; 8];
        clock.copy_from_slice(&b[0..8]);
        let mut cookie = [0u8; 8];
        cookie.copy_from_slice(&b[8..16]);
        let mut incoming_codec_guid = [0u8; 16];
        incoming_codec_guid.copy_from_slice(&b[28..44]);
        Ok(Self {
            clock_candidate: f64::from_be_bytes(clock),
            cookie,
            reserved_16: [b[16], b[17]],
            outgoing_rec_id: u16::from_be_bytes([b[18], b[19]]),
            incoming_seed: u32::from_be_bytes([b[20], b[21], b[22], b[23]]),
            outgoing_seed: u32::from_be_bytes([b[24], b[25], b[26], b[27]]),
            incoming_codec_guid,
        })
    }
}

/// `maxRecvRate` of the secondary chat-server connection (`0x00040000` = `262,144`
/// bytes/s, §3.2 item 1, §6.4).
pub const CHAT_MAX_RECV_RATE: u32 = 0x0004_0000;

/// The primary connection's `maxRecvRate` for `Net.ConnectionSpeed` `kbps` (§6.4): `0` for
/// the default `0.00` (and for a value that is not a positive finite number), otherwise
/// `floor(kbps × 102.4)`, saturated to `u32`.
#[must_use]
pub fn max_recv_rate_for_speed(kbps: f64) -> u32 {
    if !kbps.is_finite() || kbps <= 0.0 {
        return 0;
    }
    let rate = (kbps * 102.4).floor();
    if rate >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        rate as u32
    }
}

/// `ConnectAcceptHeader` (`0x00080000`, 28 bytes) optional header (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectAcceptHeader {
    /// `checkValue` (`u64` LE at `+0x00`): [`ConnectChallengeHeader::check_value`] of the
    /// challenge being answered, so the payload starts with the cookie reversed (§6.4).
    pub check_value: u64,
    /// The outgoing (client-to-server) dictionary's UUID, its raw on-disk bytes
    /// (`+0x08..+0x17`, §6.4, §7.3); never the incoming UUID or an echo of the challenge.
    pub codec_uuid: [u8; 16],
    /// `maxRecvRate` in bytes/s (`u32` LE at `+0x18`): `0` or the `Net.ConnectionSpeed`
    /// value on the primary connection ([`max_recv_rate_for_speed`]),
    /// [`CHAT_MAX_RECV_RATE`] on the chat connection.
    pub max_recv_rate: u32,
}

impl ConnectAcceptHeader {
    /// Serializes this 28-byte `ConnectAcceptHeader` into `out` (§6.4).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.check_value.to_le_bytes());
        out.extend_from_slice(&self.codec_uuid);
        out.extend_from_slice(&self.max_recv_rate.to_le_bytes());
    }

    /// Decodes a 28-byte `ConnectAcceptHeader` from `bytes` (§6.4).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 28`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        let overrun = || SessionError::OptionalHeaderOverrun {
            flag: FLAG_CONNECT_ACCEPT,
        };
        let check: [u8; 8] = bytes
            .get(0..8)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(overrun)?;
        let codec_uuid: [u8; 16] = bytes
            .get(8..24)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(overrun)?;
        let rate: [u8; 4] = bytes
            .get(24..28)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(overrun)?;
        Ok(Self {
            check_value: u64::from_le_bytes(check),
            codec_uuid,
            max_recv_rate: u32::from_le_bytes(rate),
        })
    }
}

/// `WorldReferralHeader` (`0x00000800`, 18 bytes) optional header (§6.5). No address
/// family is serialized: IPv4 is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldReferralHeader {
    /// Payload bytes `0..7`: the opaque referral cookie, reversed into the
    /// `ReferralAuthHeader` ([`ReferralAuthHeader::for_cookie`]).
    pub cookie: [u8; 8],
    /// The world server: the IPv4 address at bytes `8..11` (network order) and the UDP port
    /// at bytes `12..13` (big-endian).
    pub target_addr: SocketAddrV4,
    /// Payload bytes `14..17`: referral metadata (`OPEN-SESSION-2`), read and ignored, never
    /// echoed in the `ReferralAuthHeader`.
    pub metadata: [u8; 4],
}

impl WorldReferralHeader {
    /// The byte length of the payload (`18`, §4.3).
    pub const LEN: usize = 18;

    /// Serializes this 18-byte `WorldReferralHeader` optional header into `out` (§6.5).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.cookie);
        out.extend_from_slice(&self.target_addr.ip().octets());
        out.extend_from_slice(&self.target_addr.port().to_be_bytes());
        out.extend_from_slice(&self.metadata);
    }

    /// Decodes an 18-byte `WorldReferralHeader` optional header from `bytes` (§6.5).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 18`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        let Some(b) = bytes.first_chunk::<{ Self::LEN }>() else {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_WORLD_REFERRAL,
            });
        };
        let mut cookie = [0u8; 8];
        cookie.copy_from_slice(&b[0..8]);
        let ip = Ipv4Addr::new(b[8], b[9], b[10], b[11]);
        let port = u16::from_be_bytes([b[12], b[13]]);
        Ok(Self {
            cookie,
            target_addr: SocketAddrV4::new(ip, port),
            metadata: [b[14], b[15], b[16], b[17]],
        })
    }
}

/// `ServerRedirectHeader` (`0x00000100`, 4 bytes) optional header (§6.6): four opaque
/// bytes, checksummed as one buffer. Their meaning is `OPEN-SESSION-17`: Tessera parses
/// them as metadata, never reads an address from them and never sends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerRedirectHeader {
    /// The four bytes as received.
    pub bytes: [u8; 4],
}

impl ServerRedirectHeader {
    /// The byte length of the payload (`4`, §4.3, §6.6).
    pub const LEN: usize = 4;
}

/// `AuthServerAddressHeader` (`0x00000200`, 8 bytes) optional header (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthServerAddressHeader {
    /// Address family (`2` = `AF_INET`, `u16` LE at `+0x00`).
    pub sin_family: u16,
    /// Authentication server IPv4 address and UDP port (`sin_port` BE at `+0x02`, `sin_addr` BE at `+0x04`).
    pub addr: SocketAddrV4,
}

/// `TransportErrorHeader` (`0x00100000`) or `TransportFatalErrorHeader` (`0x00200000`),
/// 8 bytes (§6.7). The full set of server `errorCode` values is `OPEN` (`OPEN-SESSION-4`):
/// a receiver logs both numbers and terminates the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransportErrorHeader {
    /// `errorCode`: wire transport / authentication error identifier (`u32` LE at `+0x00`).
    pub error_code: u32,
    /// `errorSubcode`: auxiliary wire error parameter or zero (`u32` LE at `+0x04`).
    pub error_subcode: u32,
}

impl TransportErrorHeader {
    /// `errorCode` `0x00A7E948`: connection refused / authentication rejected during login (§6.7).
    pub const LOGIN_REFUSED_A: u32 = 0x00A7_E948;
    /// `errorCode` `0x7DC41B4A`: connection refused / authentication rejected during login (§6.7).
    pub const LOGIN_REFUSED_B: u32 = 0x7DC4_1B4A;

    /// Whether `error_code` is one of the two confirmed login-refusal codes (§6.7).
    #[must_use]
    pub const fn is_login_refusal(self) -> bool {
        matches!(
            self.error_code,
            Self::LOGIN_REFUSED_A | Self::LOGIN_REFUSED_B
        )
    }
}

/// `FlowControlHeader` (`0x08000000`, 6 bytes) optional header (§5.5): a four-byte and a
/// two-byte field, read little-endian. Their meaning is `OPEN-SESSION-13`; Tessera fills
/// them by the interim P10-22-POLICY of §5.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlowControlHeader {
    /// `u32` LE at `+0x00`: the UDP payload bytes of the connection's valid peer datagrams
    /// since the initial auth transmission or the previous report (interim policy).
    pub bytes_received_delta: u32,
    /// `u16` LE at `+0x04`: the milliseconds since then, floor, modulo 65,536 (interim
    /// policy).
    pub interval_ms: u16,
}

/// Decoded optional headers from a packet payload in canonical ascending bit order (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionalHeaders {
    /// Bit 8 (`0x00000100`): `ServerRedirectHeader` (4 opaque bytes, §6.6).
    pub server_redirect: Option<ServerRedirectHeader>,
    /// Bit 9 (`0x00000200`): `AuthServerAddressHeader` (8 bytes).
    pub auth_server_address: Option<AuthServerAddressHeader>,
    /// Bit 11 (`0x00000800`): `WorldReferralHeader` (18 bytes).
    pub world_referral: Option<WorldReferralHeader>,
    /// Bit 12 (`0x00001000`): `SequenceNakHeader` (`4 + 4 * N` bytes, `1 <= N <= 114`).
    pub sequence_nak: Option<Vec<u32>>,
    /// Bit 13 (`0x00002000`): `OutOfOrderAckHeader` (`4 + 4 * N` bytes, `1 <= N <= 114`).
    pub out_of_order_ack: Option<Vec<u32>>,
    /// Bit 14 (`0x00004000`): `CumulativeAckHeader` (`u32` **big-endian**).
    pub cumulative_ack: Option<u32>,
    /// Bit 15 (`0x00008000`): `DisconnectHeader` (0 bytes).
    pub disconnect: bool,
    /// Bit 16 (`0x00010000`): `SessionAuthHeader` (variable length).
    pub session_auth: Option<SessionAuthHeader>,
    /// Bit 17 (`0x00020000`): `ReferralAuthHeader` (8 bytes).
    pub referral_auth: Option<ReferralAuthHeader>,
    /// Bit 18 (`0x00040000`): `ConnectChallengeHeader` (44 bytes).
    pub connect_challenge: Option<ConnectChallengeHeader>,
    /// Bit 19 (`0x00080000`): `ConnectAcceptHeader` (28 bytes).
    pub connect_accept: Option<ConnectAcceptHeader>,
    /// Bit 20 (`0x00100000`): `TransportErrorHeader` (8 bytes).
    pub transport_error: Option<TransportErrorHeader>,
    /// Bit 21 (`0x00200000`): `TransportFatalErrorHeader` (8 bytes).
    pub transport_fatal_error: Option<TransportErrorHeader>,
    /// Bit 22 (`0x00400000`): `ClientControlHeader` (`u32` LE, 4 bytes; client -> server only, §4.3).
    pub client_control: Option<u32>,
    /// Bit 24 (`0x01000000`): `ClockSyncHeader` (`f64` big-endian, 8 bytes, §8.1).
    pub clock_sync: Option<f64>,
    /// Bit 25 (`0x02000000`): `OptionalWord25` (4 bytes, kept as received). Its meaning is
    /// `OPEN-SESSION-9`; Tessera never sends it (§4.3).
    pub optional_word_25: Option<[u8; 4]>,
    /// Bit 26 (`0x04000000`): `OptionalWords26` (8 bytes, kept as received). Its meaning
    /// is `OPEN-SESSION-9`; Tessera never sends it (§4.3).
    pub optional_words_26: Option<[u8; 8]>,
    /// Bit 27 (`0x08000000`): `FlowControlHeader` (6 bytes).
    pub flow_control: Option<FlowControlHeader>,
}

impl OptionalHeaders {
    /// Returns the bitmask of optional header flags present in `self` (§4.3).
    #[must_use]
    pub fn flags_mask(&self) -> u32 {
        let mut mask = 0u32;
        if self.server_redirect.is_some() {
            mask |= FLAG_SERVER_REDIRECT;
        }
        if self.auth_server_address.is_some() {
            mask |= FLAG_AUTH_SERVER_ADDR;
        }
        if self.world_referral.is_some() {
            mask |= FLAG_WORLD_REFERRAL;
        }
        if self.sequence_nak.is_some() {
            mask |= FLAG_SEQUENCE_NAK;
        }
        if self.out_of_order_ack.is_some() {
            mask |= FLAG_OUT_OF_ORDER_ACK;
        }
        if self.cumulative_ack.is_some() {
            mask |= FLAG_CUMULATIVE_ACK;
        }
        if self.disconnect {
            mask |= FLAG_DISCONNECT;
        }
        if self.session_auth.is_some() {
            mask |= FLAG_SESSION_AUTH;
        }
        if self.referral_auth.is_some() {
            mask |= FLAG_REFERRAL_AUTH;
        }
        if self.connect_challenge.is_some() {
            mask |= FLAG_CONNECT_CHALLENGE;
        }
        if self.connect_accept.is_some() {
            mask |= FLAG_CONNECT_ACCEPT;
        }
        if self.transport_error.is_some() {
            mask |= FLAG_TRANSPORT_ERROR;
        }
        if self.transport_fatal_error.is_some() {
            mask |= FLAG_TRANSPORT_FATAL_ERROR;
        }
        if self.client_control.is_some() {
            mask |= FLAG_CLIENT_CONTROL;
        }
        if self.clock_sync.is_some() {
            mask |= FLAG_CLOCK_SYNC;
        }
        if self.optional_word_25.is_some() {
            mask |= FLAG_OPTIONAL_WORD_25;
        }
        if self.optional_words_26.is_some() {
            mask |= FLAG_OPTIONAL_WORDS_26;
        }
        if self.flow_control.is_some() {
            mask |= FLAG_FLOW_CONTROL;
        }
        mask
    }

    /// Serializes all present optional headers into `out` in strict ascending bit order (`0x00000100`..`0x08000000`, §4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `SequenceNakHeader` or `OutOfOrderAckHeader` count is `0` or `> 114`.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        self.encode_marked(out, &mut Vec::new())
    }

    /// Serializes each present optional header into its own buffer, in ascending bit order
    /// (the per-header buffers of the checksum, §4.4.2 step 2). Headers with no
    /// bytes (`DisconnectHeader`) give no buffer.
    ///
    /// # Errors
    ///
    /// See [`Self::encode`].
    ///
    /// When `session_auth` is present the combined copy is wiped before returning, and the
    /// returned buffers hold the credentials: the caller wipes them (`official-login.md`
    /// §6.3; [`payload_checksum`] does).
    pub fn encode_buffers(&self) -> Result<Vec<Vec<u8>>, SessionError> {
        let mut all = SecretBuf::with_capacity(self.encoded_len(), self.session_auth.is_some());
        let mut marks = Vec::new();
        self.encode_marked(&mut all.buf, &mut marks)?;
        let mut buffers = Vec::with_capacity(marks.len());
        let mut from = 0usize;
        for end in marks {
            if end > from
                && let Some(slice) = all.buf.get(from..end)
            {
                buffers.push(slice.to_vec());
            }
            from = end;
        }
        Ok(buffers)
    }

    /// The byte length of [`Self::encode`]'s output (§4.3 header sizes). Counts outside
    /// `1..=114` are measured as they are; [`Self::encode`] refuses them.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        let list = |seqs: &Option<Vec<u32>>| {
            seqs.as_ref()
                .map_or(0, |s| 4usize.saturating_add(s.len().saturating_mul(4)))
        };
        let fixed = |present: bool, len: usize| if present { len } else { 0 };
        [
            fixed(self.server_redirect.is_some(), ServerRedirectHeader::LEN),
            fixed(self.auth_server_address.is_some(), 8),
            fixed(self.world_referral.is_some(), WorldReferralHeader::LEN),
            list(&self.sequence_nak),
            list(&self.out_of_order_ack),
            fixed(self.cumulative_ack.is_some(), 4),
            self.session_auth
                .as_ref()
                .map_or(0, SessionAuthHeader::encoded_len),
            fixed(self.referral_auth.is_some(), 8),
            fixed(self.connect_challenge.is_some(), 44),
            fixed(self.connect_accept.is_some(), 28),
            fixed(self.transport_error.is_some(), 8),
            fixed(self.transport_fatal_error.is_some(), 8),
            fixed(self.client_control.is_some(), 4),
            fixed(self.clock_sync.is_some(), 8),
            fixed(self.optional_word_25.is_some(), 4),
            fixed(self.optional_words_26.is_some(), 8),
            fixed(self.flow_control.is_some(), 6),
        ]
        .iter()
        .fold(0usize, |acc, &n| acc.saturating_add(n))
    }

    /// [`Self::encode`], pushing the end offset of every header written onto `marks`.
    fn encode_marked(
        &self,
        out: &mut Vec<u8>,
        marks: &mut Vec<usize>,
    ) -> Result<usize, SessionError> {
        let start = out.len();
        // Reserved up front: once the credentials are written, `out` never moves
        // (`official-login.md` §6.3).
        out.reserve(self.encoded_len());
        // Bit 8: 0x00000100 ServerRedirect (4 opaque B, §6.6)
        if let Some(ss) = self.server_redirect {
            out.extend_from_slice(&ss.bytes);
        }
        marks.push(out.len());
        // Bit 9: 0x00000200 AuthServerAddress (8 B)
        if let Some(lsa) = self.auth_server_address {
            out.extend_from_slice(&lsa.sin_family.to_le_bytes());
            out.extend_from_slice(&lsa.addr.port().to_be_bytes());
            out.extend_from_slice(&lsa.addr.ip().octets());
        }
        marks.push(out.len());
        // Bit 11: 0x00000800 WorldReferral (18 B)
        if let Some(ref_hdr) = &self.world_referral {
            ref_hdr.encode(out);
        }
        marks.push(out.len());
        // Bit 12: 0x00001000 SequenceNak (4 + 4*N B, LE)
        if let Some(seqs) = &self.sequence_nak {
            if seqs.is_empty() || seqs.len() > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidSequenceNakCount {
                    count: seqs.len() as u32,
                });
            }
            out.extend_from_slice(&(seqs.len() as u32).to_le_bytes());
            for &s in seqs {
                out.extend_from_slice(&s.to_le_bytes());
            }
        }
        marks.push(out.len());
        // Bit 13: 0x00002000 OutOfOrderAck (4 + 4*N B, LE)
        if let Some(seqs) = &self.out_of_order_ack {
            if seqs.is_empty() || seqs.len() > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidOutOfOrderAckCount {
                    count: seqs.len() as u32,
                });
            }
            out.extend_from_slice(&(seqs.len() as u32).to_le_bytes());
            for &s in seqs {
                out.extend_from_slice(&s.to_le_bytes());
            }
        }
        marks.push(out.len());
        // Bit 14: 0x00004000 CumulativeAck (4 B, big-endian!)
        if let Some(ack) = self.cumulative_ack {
            out.extend_from_slice(&ack.to_be_bytes());
        }
        marks.push(out.len());
        // Bit 15: 0x00008000 Disconnect (0 B)
        marks.push(out.len());
        // Bit 16: 0x00010000 SessionAuth (variable)
        if let Some(lr) = &self.session_auth {
            lr.encode(out)?;
        }
        marks.push(out.len());
        // Bit 17: 0x00020000 ReferralAuth (8 B)
        if let Some(wlr) = self.referral_auth {
            out.extend_from_slice(&wlr.reversed_cookie);
        }
        marks.push(out.len());
        // Bit 18: 0x00040000 ConnectChallenge (44 B)
        if let Some(cr) = &self.connect_challenge {
            cr.encode(out);
        }
        marks.push(out.len());
        // Bit 19: 0x00080000 ConnectAccept (28 B)
        if let Some(resp) = &self.connect_accept {
            resp.encode(out);
        }
        marks.push(out.len());
        // Bit 20: 0x00100000 TransportError (8 B)
        if let Some(ne) = self.transport_error {
            out.extend_from_slice(&ne.error_code.to_le_bytes());
            out.extend_from_slice(&ne.error_subcode.to_le_bytes());
        }
        marks.push(out.len());
        // Bit 21: 0x00200000 TransportFatalError (8 B)
        if let Some(ned) = self.transport_fatal_error {
            out.extend_from_slice(&ned.error_code.to_le_bytes());
            out.extend_from_slice(&ned.error_subcode.to_le_bytes());
        }
        marks.push(out.len());
        // Bit 22: 0x00400000 ClientControl (4 B u32 LE, Client -> Server only)
        if let Some(cc) = self.client_control {
            out.extend_from_slice(&cc.to_le_bytes());
        }
        marks.push(out.len());
        // Bit 24: 0x01000000 ClockSync (8 B f64 big-endian, §8.1)
        if let Some(ts) = self.clock_sync {
            out.extend_from_slice(&ts.to_be_bytes());
        }
        marks.push(out.len());
        // Bit 25: 0x02000000 OptionalWord25 (4 B, as received; OPEN-SESSION-9)
        if let Some(w) = self.optional_word_25 {
            out.extend_from_slice(&w);
        }
        marks.push(out.len());
        // Bit 26: 0x04000000 OptionalWords26 (8 B, as received; OPEN-SESSION-9)
        if let Some(w) = self.optional_words_26 {
            out.extend_from_slice(&w);
        }
        marks.push(out.len());
        // Bit 27: 0x08000000 FlowControl (6 B)
        if let Some(fl) = self.flow_control {
            out.extend_from_slice(&fl.bytes_received_delta.to_le_bytes());
            out.extend_from_slice(&fl.interval_ms.to_le_bytes());
        }
        marks.push(out.len());
        Ok(out.len() - start)
    }

    /// Decodes optional headers from `payload` in strict ascending bit order (§4.3, §10.1 rules 3, 7).
    ///
    /// - Rejects any wire flag in the receive mask (`(flags & 0xF08004F9) != 0`, §4.3,
    ///   §10.1 rule 3).
    /// - On the client receive path (`from_server == true`) rejects `0x00400000`
    ///   (`ClientControlHeader`, §10.1 rule 7).
    /// - The low markers `0x00000002` (`MaskedChecksumFlag`) and `0x00000004`
    ///   (`HasFragmentedBlobsFlag`) carry no optional header bytes
    ///   ([`HEADERLESS_FLAGS_MASK`]); bits 25 and 26 carry four and eight bytes, read in
    ///   ascending order between `ClockSyncHeader` and `FlowControlHeader` (§4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if forbidden flags are set, a header overruns `payload`,
    /// or `SequenceNakHeader`/`OutOfOrderAckHeader` counts are outside `1..=114`.
    pub fn decode(
        flags: u32,
        payload: &[u8],
        from_server: bool,
    ) -> Result<(Self, usize), SessionError> {
        Self::decode_marked(flags, payload, from_server, &mut Vec::new())
    }

    /// [`Self::decode`], pushing the end offset of every header read onto `marks` (the
    /// per-header buffers of the checksum, §4.4.2 step 2).
    fn decode_marked(
        flags: u32,
        payload: &[u8],
        from_server: bool,
        marks: &mut Vec<usize>,
    ) -> Result<(Self, usize), SessionError> {
        if (flags & RECEIVE_FORBIDDEN_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }
        if from_server && (flags & SERVER_FORBIDDEN_OPT_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }

        let active = flags & VALID_DATAGRAM_FLAGS_MASK & !HEADERLESS_FLAGS_MASK;
        let mut hdrs = Self::default();
        let mut cursor = 0usize;

        let need = |cursor: usize, n: usize, flag: u32| -> Result<(), SessionError> {
            if cursor.saturating_add(n) > payload.len() {
                return Err(SessionError::OptionalHeaderOverrun { flag });
            }
            Ok(())
        };

        // Bit 8: 0x00000100 ServerRedirect (4 opaque B, §6.6)
        if (active & FLAG_SERVER_REDIRECT) != 0 {
            need(cursor, ServerRedirectHeader::LEN, FLAG_SERVER_REDIRECT)?;
            hdrs.server_redirect = Some(ServerRedirectHeader {
                bytes: [
                    payload[cursor],
                    payload[cursor + 1],
                    payload[cursor + 2],
                    payload[cursor + 3],
                ],
            });
            cursor += ServerRedirectHeader::LEN;
            marks.push(cursor);
        }
        // Bit 9: 0x00000200 AuthServerAddress (8 B)
        if (active & FLAG_AUTH_SERVER_ADDR) != 0 {
            need(cursor, 8, FLAG_AUTH_SERVER_ADDR)?;
            let sin_family = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]);
            let port = u16::from_be_bytes([payload[cursor + 2], payload[cursor + 3]]);
            let ip = Ipv4Addr::new(
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            );
            hdrs.auth_server_address = Some(AuthServerAddressHeader {
                sin_family,
                addr: SocketAddrV4::new(ip, port),
            });
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 11: 0x00000800 WorldReferral (18 B)
        if (active & FLAG_WORLD_REFERRAL) != 0 {
            hdrs.world_referral = Some(WorldReferralHeader::decode(&payload[cursor..])?);
            cursor += WorldReferralHeader::LEN;
            marks.push(cursor);
        }
        // Bit 12: 0x00001000 SequenceNak (4 + 4*N B)
        if (active & FLAG_SEQUENCE_NAK) != 0 {
            need(cursor, 4, FLAG_SEQUENCE_NAK)?;
            let count = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            if count == 0 || (count as usize) > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidSequenceNakCount { count });
            }
            let n = count as usize;
            need(cursor, 4 + n * 4, FLAG_SEQUENCE_NAK)?;
            cursor += 4;
            let mut list = Vec::with_capacity(n);
            for _ in 0..n {
                list.push(u32::from_le_bytes([
                    payload[cursor],
                    payload[cursor + 1],
                    payload[cursor + 2],
                    payload[cursor + 3],
                ]));
                cursor += 4;
            }
            hdrs.sequence_nak = Some(list);
            marks.push(cursor);
        }
        // Bit 13: 0x00002000 OutOfOrderAck (4 + 4*N B)
        if (active & FLAG_OUT_OF_ORDER_ACK) != 0 {
            need(cursor, 4, FLAG_OUT_OF_ORDER_ACK)?;
            let count = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            if count == 0 || (count as usize) > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidOutOfOrderAckCount { count });
            }
            let n = count as usize;
            need(cursor, 4 + n * 4, FLAG_OUT_OF_ORDER_ACK)?;
            cursor += 4;
            let mut list = Vec::with_capacity(n);
            for _ in 0..n {
                list.push(u32::from_le_bytes([
                    payload[cursor],
                    payload[cursor + 1],
                    payload[cursor + 2],
                    payload[cursor + 3],
                ]));
                cursor += 4;
            }
            hdrs.out_of_order_ack = Some(list);
            marks.push(cursor);
        }
        // Bit 14: 0x00004000 CumulativeAck (4 B big-endian)
        if (active & FLAG_CUMULATIVE_ACK) != 0 {
            need(cursor, 4, FLAG_CUMULATIVE_ACK)?;
            let ack = u32::from_be_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            hdrs.cumulative_ack = Some(ack);
            cursor += 4;
            marks.push(cursor);
        }
        // Bit 15: 0x00008000 Disconnect (0 B)
        if (active & FLAG_DISCONNECT) != 0 {
            hdrs.disconnect = true;
            marks.push(cursor);
        }
        // Bit 16: 0x00010000 SessionAuth (variable)
        if (active & FLAG_SESSION_AUTH) != 0 {
            let (lr, consumed) = SessionAuthHeader::decode(&payload[cursor..])?;
            hdrs.session_auth = Some(lr);
            cursor += consumed;
            marks.push(cursor);
        }
        // Bit 17: 0x00020000 ReferralAuth (8 B)
        if (active & FLAG_REFERRAL_AUTH) != 0 {
            need(cursor, 8, FLAG_REFERRAL_AUTH)?;
            let mut reversed_cookie = [0u8; 8];
            reversed_cookie.copy_from_slice(&payload[cursor..cursor + 8]);
            hdrs.referral_auth = Some(ReferralAuthHeader { reversed_cookie });
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 18: 0x00040000 ConnectChallenge (44 B)
        if (active & FLAG_CONNECT_CHALLENGE) != 0 {
            hdrs.connect_challenge = Some(ConnectChallengeHeader::decode(&payload[cursor..])?);
            cursor += 44;
            marks.push(cursor);
        }
        // Bit 19: 0x00080000 ConnectAccept (28 B)
        if (active & FLAG_CONNECT_ACCEPT) != 0 {
            hdrs.connect_accept = Some(ConnectAcceptHeader::decode(&payload[cursor..])?);
            cursor += 28;
            marks.push(cursor);
        }
        // Bit 20: 0x00100000 TransportError (8 B)
        if (active & FLAG_TRANSPORT_ERROR) != 0 {
            need(cursor, 8, FLAG_TRANSPORT_ERROR)?;
            let error_code = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            let error_subcode = u32::from_le_bytes([
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            ]);
            hdrs.transport_error = Some(TransportErrorHeader {
                error_code,
                error_subcode,
            });
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 21: 0x00200000 TransportFatalError (8 B)
        if (active & FLAG_TRANSPORT_FATAL_ERROR) != 0 {
            need(cursor, 8, FLAG_TRANSPORT_FATAL_ERROR)?;
            let error_code = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            let error_subcode = u32::from_le_bytes([
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            ]);
            hdrs.transport_fatal_error = Some(TransportErrorHeader {
                error_code,
                error_subcode,
            });
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 22: 0x00400000 ClientControlHeader (4 B u32 LE; refused above from a server, §4.3)
        if (active & FLAG_CLIENT_CONTROL) != 0 {
            need(cursor, 4, FLAG_CLIENT_CONTROL)?;
            let cc = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            hdrs.client_control = Some(cc);
            cursor += 4;
            marks.push(cursor);
        }
        // Bit 24: 0x01000000 ClockSync (8 B f64 big-endian, §8.1)
        if (active & FLAG_CLOCK_SYNC) != 0 {
            need(cursor, 8, FLAG_CLOCK_SYNC)?;
            let mut ts = [0u8; 8];
            ts.copy_from_slice(&payload[cursor..cursor + 8]);
            hdrs.clock_sync = Some(f64::from_be_bytes(ts));
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 25: 0x02000000 OptionalWord25 (4 B; meaning OPEN-SESSION-9, kept as received)
        if (active & FLAG_OPTIONAL_WORD_25) != 0 {
            need(cursor, 4, FLAG_OPTIONAL_WORD_25)?;
            let mut w = [0u8; 4];
            w.copy_from_slice(&payload[cursor..cursor + 4]);
            hdrs.optional_word_25 = Some(w);
            cursor += 4;
            marks.push(cursor);
        }
        // Bit 26: 0x04000000 OptionalWords26 (8 B; meaning OPEN-SESSION-9, kept as received)
        if (active & FLAG_OPTIONAL_WORDS_26) != 0 {
            need(cursor, 8, FLAG_OPTIONAL_WORDS_26)?;
            let mut w = [0u8; 8];
            w.copy_from_slice(&payload[cursor..cursor + 8]);
            hdrs.optional_words_26 = Some(w);
            cursor += 8;
            marks.push(cursor);
        }
        // Bit 27: 0x08000000 FlowControl (6 B)
        if (active & FLAG_FLOW_CONTROL) != 0 {
            need(cursor, 6, FLAG_FLOW_CONTROL)?;
            let bytes_received_delta = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            let interval_ms = u16::from_le_bytes([payload[cursor + 4], payload[cursor + 5]]);
            hdrs.flow_control = Some(FlowControlHeader {
                bytes_received_delta,
                interval_ms,
            });
            cursor += 6;
            marks.push(cursor);
        }

        Ok((hdrs, cursor))
    }
}

// ---------------------------------------------------------------------------
// §5.6.2–§5.6.4 Blob Fragment Header, Splitting, and Bounded Reassembly
// ---------------------------------------------------------------------------

/// `blobFlags` bit 0 (`0x01`): no fields of its own in the current reader (§5.6.3, §9.3.9). Kept in
/// [`BlobFragmentHeader::blob_flags`] as metadata, its delivery meaning ignored
/// (`OPEN-SESSION-14`); never sent.
pub const BLOB_FLAG_OBSERVED_01: u8 = 0x01;
/// `blobFlags` bit 2 (`0x04`): as [`BLOB_FLAG_OBSERVED_01`].
pub const BLOB_FLAG_OBSERVED_04: u8 = 0x04;
/// `blobFlags` bit 3 (`0x08`, `EncryptedFragmentedBlob`). Tessera never sets it and rejects
/// every received blob that has it (§7.1, `OPEN-SESSION-6`).
pub const BLOB_FLAG_ENCRYPTED: u8 = 0x08;
/// `blobFlags` bit 4 (`0x10`): fragmented blob is split across `numFrags > 1` fragments (§5.6.3).
pub const BLOB_FLAG_IS_FRAGMENTED: u8 = 0x10;
/// `blobFlags` bit 6 (`0x40`): `recipientId` defaults to `2` (`0x0000000000000002`, §5.6.3).
pub const BLOB_FLAG_DEFAULT_RECIPIENT: u8 = 0x40;
/// `blobFlags` bit 7 (`0x80`): omit `blobHeaderWord` and `fragDataLen` fields (§5.6.3).
pub const BLOB_FLAG_OMIT_CHUNK_INFO: u8 = 0x80;

/// Default 64-bit `recipientId` when `DefaultRecipient` (`blobFlags & 0x40`) is set
/// (`0x0000000000000002`, §5.6.3 field 7, §9.1 item 2).
pub const DEFAULT_BLOB_RECIPIENT_ID: u64 = 2;

/// `extWord` bit `0x0001`: a 28-byte attachment follows the padded header (§5.6.3, §9.3.9).
pub const EXT_WORD_ATTACHMENT: u16 = 0x0001;

/// The `extWord` bits with a known effect; a fragment with any other is unverifiable and its
/// datagram is dropped ([`SessionError::UnknownExtensionBits`], §9.3.9).
pub const EXT_WORD_KNOWN_BITS: u16 = EXT_WORD_ATTACHMENT;

/// The optional 28-byte fragment attachment (field 10 in §5.6.3), selected by `extWord` bit
/// [`EXT_WORD_ATTACHMENT`]. Its meanings are opaque: it is read and nothing acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FragmentAttachment {
    /// Three 64-bit little-endian words (`24` B).
    pub words64: [u64; 3],
    /// Trailing 32-bit little-endian word (`4` B).
    pub word32: u32,
}

/// The highest delivery queue a sender uses (`4`, §5.6.2).
pub const MAX_SEND_QUEUE_ID: u8 = 4;

/// The highest delivery queue a receiver accepts (`4`): receiver slots `5` and `6` are
/// `OPEN-SESSION-12`, so a datagram with a fragment on them is rejected whole, with no commit
/// and no Ack, and counted ([`SessionError::UnsupportedReceiveQueue`]) (§5.6.2, §5.6.3).
pub const MAX_RECEIVE_QUEUE_ID: u8 = 4;

/// `blobFlags` bit 1 (`0x02`): an observed control bit without wire fields whose meaning is
/// `OPEN-SESSION-14`. Accepted on queue `1` and kept in
/// [`BlobFragmentHeader::blob_flags`] as metadata; a fragment on another queue with it is
/// unsupported ([`SessionError::UnsupportedFragment`]); never sent (§5.6.3).
pub const BLOB_FLAG_CONTROL_02: u8 = 0x02;

/// `blobFlags` bit `0x20`, unverified: a received datagram with it is dropped
/// ([`SessionError::UndefinedBlobFlags`]); never sent (§5.6.3, §9.3.9).
pub const UNDEFINED_BLOB_FLAGS: u8 = 0x20;

/// The `blobFlags` bits Tessera sends (`0x08`, `0x10`, `0x40`, `0x80`, §5.6.3);
/// [`BlobFragmentHeader::encode`] clears `0x01`, `0x02`, `0x04` and `0x20`, which are not
/// synthesized on send (§9.3.9).
pub const DEFINED_BLOB_FLAGS: u8 = BLOB_FLAG_ENCRYPTED
    | BLOB_FLAG_IS_FRAGMENTED
    | BLOB_FLAG_DEFAULT_RECIPIENT
    | BLOB_FLAG_OMIT_CHUNK_INFO;

/// The framing part of the receive policy (§9.3.9) for one decoded fragment header: bit `0x20`
/// cannot be verified, so the datagram is dropped and counted, with no Ack and no state
/// committed. Bits `0x01` and `0x04` are parsed and ignored. Bit `0x02` outside queue `1` has
/// known framing: it is judged once the checksum held ([`unsupported_control_bit`]).
///
/// # Errors
///
/// [`SessionError::UndefinedBlobFlags`].
pub fn check_fragment_receive_policy(header: &BlobFragmentHeader) -> Result<(), SessionError> {
    let flags = header.blob_flags;
    if (flags & UNDEFINED_BLOB_FLAGS) != 0 {
        return Err(SessionError::UndefinedBlobFlags { blob_flags: flags });
    }
    Ok(())
}

/// The slice-length rule of §5.6.3 and §9.3.9: a single fragment has `0..=464` bytes; of several,
/// every non-final one exactly 436 and the final one `1..=436`. Checked before anything is copied.
///
/// # Errors
///
/// [`SessionError::InvalidFragmentDataLen`].
pub const fn check_fragment_slice(
    frag_index: u32,
    num_frags: u32,
    len: u32,
) -> Result<(), SessionError> {
    let fits = if num_frags <= 1 {
        len < 0x01D1
    } else if frag_index.saturating_add(1) < num_frags {
        len == MAX_BLOB_FRAG_PAYLOAD as u32
    } else {
        len >= 1 && len <= MAX_BLOB_FRAG_PAYLOAD as u32
    };
    if fits {
        Ok(())
    } else {
        Err(SessionError::InvalidFragmentDataLen { len })
    }
}

/// §9.3.9: `blobFlags` `0x02` on queues `2..=4` on an ordinary opcode blob with complete known
/// framing. Once the packet's checksum held this is the whole transport-level terminal unsupported
/// class, without an Ack (`OPEN-SESSION-14`). An omitted-chunk form with `0x02` is not this class:
/// it is an invalid control and drops first ([`invalid_omitted_control`]).
#[must_use]
pub const fn unsupported_control_bit(header: &BlobFragmentHeader) -> bool {
    header.blob_flags & BLOB_FLAG_CONTROL_02 != 0
        && header.blob_flags & BLOB_FLAG_OMIT_CHUNK_INFO == 0
        && matches!(header.queue_id, 2..=4)
}

/// The `blobFlags` bits an empty control must have clear (§9.3.9): fragmentation `0x10`, control
/// bit `0x02`, encryption `0x08` and the undefined `0x20`.
pub const OMITTED_CONTROL_FORBIDDEN_FLAGS: u8 =
    BLOB_FLAG_IS_FRAGMENTED | BLOB_FLAG_CONTROL_02 | BLOB_FLAG_ENCRYPTED | UNDEFINED_BLOB_FLAGS;

/// §9.3.9's invalid omitted-chunk combination: `OmitChunkInfo` (`0x80`) together with any of
/// [`OMITTED_CONTROL_FORBIDDEN_FLAGS`]. Its whole datagram drops before the known-layout `0x02`
/// terminal test (§10.2 Vector 24: `01 D0 ...` and `02 82 ...` both drop).
#[must_use]
pub const fn invalid_omitted_control(header: &BlobFragmentHeader) -> bool {
    header.blob_flags & BLOB_FLAG_OMIT_CHUNK_INFO != 0
        && header.blob_flags & OMITTED_CONTROL_FORBIDDEN_FLAGS != 0
}

/// Whether a fragment of queue `queue_id` carries `orderNum` (field 3b, §5.6.3): queues
/// `2..=4`.
const fn has_order_num(queue_id: u8) -> bool {
    matches!(queue_id, 2..=4)
}

/// Whether a fragment of queue `queue_id` carries `blobId` (field 4, §5.6.3): queues `2`
/// and `4` always, queue `1` when the blob is fragmented (an unfragmented queue-1 header
/// has none), queue `3` never, fragmented or not (§5.6.2, §5.6.4 rule 3).
const fn has_blob_id(queue_id: u8, fragmented: bool) -> bool {
    matches!(queue_id, 2 | 4) || (queue_id == 1 && fragmented)
}

/// Compact variable-length blob fragment header (fields 1–10 in §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobFragmentHeader {
    /// Delivery queue ID (bits `0..6` of `queueAndOpt`, §5.6.2): `1..=4`.
    pub queue_id: u8,
    /// Optional 16-bit extension word (`Some` when bit 7 `0x80` of `queueAndOpt` is set, field 6).
    pub ext_word: Option<u16>,
    /// Fragment flags byte (`blobFlags`, field 2), as received (bits `0x01`, `0x02` and `0x04`
    /// included, kept as metadata); bits outside [`DEFINED_BLOB_FLAGS`] are sent as `0`
    /// ([`check_fragment_receive_policy`] is the receive policy).
    pub blob_flags: u8,
    /// Sequence group ID (present when `queue_id == 3`, field 3a; defaults to `0`).
    pub seq_group: u16,
    /// Per-queue ordering number (present when `queue_id` in `2..=4`, field 3b; defaults to `0xFFFF`).
    pub order_num: u16,
    /// 16-bit blob identifier (present when `queue_id` in `{2, 4}` or (`queue_id` is `1`
    /// and `IsFragmented`), field 4; `0` otherwise).
    pub blob_id: u16,
    /// Zero-based fragment index (`0..num_frags - 1`, field 5a).
    pub frag_index: u32,
    /// Total number of fragments in this fragmented blob (`>= 1`, field 5b).
    pub num_frags: u32,
    /// 64-bit recipient ID (`2` when `DefaultRecipient` `0x40` is set, else explicit `u64` LE, field 7).
    pub recipient_id: u64,
    /// Upper-layer message opcode / header word (present when `OmitChunkInfo` is clear and `frag_index == 0`, field 8a).
    pub blob_header_word: Option<u32>,
    /// Byte length of this fragment's payload slice (`< 0x01D1`, field 8b).
    pub frag_data_len: u32,
    /// Optional 28-byte attachment (`extWord` bit [`EXT_WORD_ATTACHMENT`], field 10): opaque.
    pub attachment: Option<FragmentAttachment>,
}

impl BlobFragmentHeader {
    /// Serializes this blob fragment header into `out`, appending the `0x00` even-byte
    /// alignment pad (`alignPad`, field 9) when fields 1–8b have an odd byte length (§5.6.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `queue_id` is outside `1..=4` (the sender's queues,
    /// §5.6.2), `num_frags == 0`, `num_frags <= frag_index`, `frag_data_len >= 0x01D1`, or a
    /// compact integer field exceeds [`MAX_VARIABLE_U32`].
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        if !(1..=MAX_SEND_QUEUE_ID).contains(&self.queue_id) {
            return Err(SessionError::InvalidQueueId {
                queue_id: self.queue_id,
            });
        }
        if self.num_frags == 0 || self.num_frags <= self.frag_index {
            return Err(SessionError::InvalidFragmentBounds {
                frag_index: self.frag_index,
                num_frags: self.num_frags,
            });
        }
        if self.frag_data_len >= 0x01D1 {
            return Err(SessionError::InvalidFragmentDataLen {
                len: self.frag_data_len,
            });
        }

        let start = out.len();
        let mut queue_and_opt = self.queue_id & 0x7F;
        // An attachment is selected by the extension word (§5.6.3).
        let ext_word = match (self.ext_word, self.attachment.is_some()) {
            (word, false) => word,
            (word, true) => Some(word.unwrap_or(0) | EXT_WORD_ATTACHMENT),
        };
        if ext_word.is_some() {
            queue_and_opt |= 0x80;
        }
        // 0x01, 0x02, 0x04 and 0x20 go out as 0 (§9.3.9, [`DEFINED_BLOB_FLAGS`]).
        let mut flags = self.blob_flags & DEFINED_BLOB_FLAGS;
        if self.num_frags > 1 {
            flags |= BLOB_FLAG_IS_FRAGMENTED;
        } else {
            flags &= !BLOB_FLAG_IS_FRAGMENTED;
        }

        // Field 1 & Field 2
        out.push(queue_and_opt);
        out.push(flags);

        // Field 3a: seqGroup if queueId == 3
        if self.queue_id == 3 {
            encode_variable_u32(u32::from(self.seq_group), out)?;
        }
        // Field 3b: orderNum if queueId in {2, 3, 4}
        if has_order_num(self.queue_id) {
            out.extend_from_slice(&self.order_num.to_le_bytes());
        }
        // Field 4: blobId if queueId in {2, 4} or (queueId == 1 and IsFragmented)
        if has_blob_id(self.queue_id, (flags & BLOB_FLAG_IS_FRAGMENTED) != 0) {
            out.extend_from_slice(&self.blob_id.to_le_bytes());
        }
        // Field 5a & 5b: fragIndex and numFrags if IsFragmented
        if (flags & BLOB_FLAG_IS_FRAGMENTED) != 0 {
            encode_variable_u32(self.frag_index, out)?;
            encode_variable_u32(self.num_frags, out)?;
        }
        // Field 6: extWord if (queueAndOpt & 0x80) != 0
        if let Some(ext) = ext_word {
            out.extend_from_slice(&ext.to_le_bytes());
        }
        // Field 7: recipientId if (blobFlags & 0x40) == 0
        if (flags & BLOB_FLAG_DEFAULT_RECIPIENT) == 0 {
            out.extend_from_slice(&self.recipient_id.to_le_bytes());
        }
        // Field 8a & 8b: blobHeaderWord (when fragIndex == 0) and fragDataLen if (blobFlags & 0x80) == 0
        if (flags & BLOB_FLAG_OMIT_CHUNK_INFO) == 0 {
            if self.frag_index == 0 {
                encode_variable_u32(self.blob_header_word.unwrap_or(0), out)?;
            }
            encode_variable_u32(self.frag_data_len, out)?;
        }
        // Field 9: alignPad (0x00) if serialized length of fields 1..8b is odd
        let len_so_far = out.len() - start;
        if (len_so_far & 1) != 0 {
            out.push(0x00);
        }
        // Field 10: the 28-byte attachment, after the pad (§5.6.3).
        if let Some(att) = &self.attachment {
            for &w in &att.words64 {
                out.extend_from_slice(&w.to_le_bytes());
            }
            out.extend_from_slice(&att.word32.to_le_bytes());
        }

        Ok(out.len() - start)
    }

    /// Decodes a compact variable-length blob fragment header from `bytes` (§5.6.3, §10.1 rule 8),
    /// returning `(header, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `bytes` is truncated, `queue_id` is outside `1..=4`
    /// ([`SessionError::UnsupportedReceiveQueue`] for `5` and `6`, `OPEN-SESSION-12`),
    /// `num_frags == 0`, `num_frags <= frag_index`, an extension bit is unknown, the slice length
    /// breaks [`check_fragment_slice`], or `alignPad != 0x00`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        if bytes.len() < 2 {
            return Err(SessionError::FragmentPayloadOverrun);
        }
        let queue_and_opt = bytes[0];
        let blob_flags = bytes[1];
        let queue_id = queue_and_opt & 0x7F;
        if matches!(queue_id, 5 | 6) {
            // Queues 5/6 framing is OPEN-SESSION-12: the header's length is unknown, so the
            // whole datagram is rejected with no commit and no Ack (§5.6.3), and counted.
            return Err(SessionError::UnsupportedReceiveQueue { queue_id });
        }
        if !(1..=MAX_RECEIVE_QUEUE_ID).contains(&queue_id) {
            return Err(SessionError::InvalidQueueId { queue_id });
        }
        // §9.3.9 precedence: an invalid omitted-chunk combination drops its whole datagram first,
        // before its other fields are judged and before any terminal inference.
        if blob_flags & BLOB_FLAG_OMIT_CHUNK_INFO != 0
            && blob_flags & OMITTED_CONTROL_FORBIDDEN_FLAGS != 0
        {
            return Err(SessionError::InvalidOmittedControl {
                queue_id,
                blob_flags,
            });
        }

        let mut cursor = 2usize;

        // Field 3a: seqGroup
        let seq_group = if queue_id == 3 {
            let (sg, n) = decode_variable_u32(&bytes[cursor..])?;
            cursor += n;
            u16::try_from(sg).map_err(|_| SessionError::SequenceGroupOutOfRange { value: sg })?
        } else {
            0
        };

        // Field 3b: orderNum
        let order_num = if has_order_num(queue_id) {
            if bytes.len() < cursor + 2 {
                return Err(SessionError::FragmentPayloadOverrun);
            }
            let on = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            cursor += 2;
            on
        } else {
            0xFFFF
        };

        // Field 4: blobId
        let blob_id = if has_blob_id(queue_id, (blob_flags & BLOB_FLAG_IS_FRAGMENTED) != 0) {
            if bytes.len() < cursor + 2 {
                return Err(SessionError::FragmentPayloadOverrun);
            }
            let bid = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            cursor += 2;
            bid
        } else {
            0
        };

        // Field 5a & 5b: fragIndex, numFrags
        let (frag_index, num_frags) = if (blob_flags & BLOB_FLAG_IS_FRAGMENTED) != 0 {
            let (fi, n1) = decode_variable_u32(&bytes[cursor..])?;
            cursor += n1;
            let (nf, n2) = decode_variable_u32(&bytes[cursor..])?;
            cursor += n2;
            (fi, nf)
        } else {
            (0, 1)
        };
        // §5.6.3 field 5b: a flagged fragment has `numFrags >= 2`; `IsFragmented` with a count of 1 is
        // a bad count, which §9.3.9 drops with its whole datagram.
        if num_frags == 0
            || num_frags <= frag_index
            || ((blob_flags & BLOB_FLAG_IS_FRAGMENTED) != 0 && num_frags < 2)
        {
            return Err(SessionError::InvalidFragmentBounds {
                frag_index,
                num_frags,
            });
        }

        // Field 6: extWord
        let ext_word = if (queue_and_opt & 0x80) != 0 {
            if bytes.len() < cursor + 2 {
                return Err(SessionError::FragmentPayloadOverrun);
            }
            let ew = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            cursor += 2;
            // §9.3.9: an unknown bit makes the rest of the header unverifiable.
            if ew & !EXT_WORD_KNOWN_BITS != 0 {
                return Err(SessionError::UnknownExtensionBits { ext_word: ew });
            }
            Some(ew)
        } else {
            None
        };

        // Field 7: recipientId
        let recipient_id = if (blob_flags & BLOB_FLAG_DEFAULT_RECIPIENT) == 0 {
            if bytes.len() < cursor + 8 {
                return Err(SessionError::FragmentPayloadOverrun);
            }
            let rid = u64::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
            cursor += 8;
            rid
        } else {
            DEFAULT_BLOB_RECIPIENT_ID
        };

        // Field 8a & 8b: blobHeaderWord and fragDataLen
        let (blob_header_word, frag_data_len) = if (blob_flags & BLOB_FLAG_OMIT_CHUNK_INFO) == 0 {
            let bhw = if frag_index == 0 {
                let (w, n) = decode_variable_u32(&bytes[cursor..])?;
                cursor += n;
                Some(w)
            } else {
                None
            };
            let (fdl, n) = decode_variable_u32(&bytes[cursor..])?;
            cursor += n;
            (bhw, fdl)
        } else {
            (None, 0)
        };

        check_fragment_slice(frag_index, num_frags, frag_data_len)?;

        // Field 9: alignPad (if cursor is odd, next byte must be 0x00)
        if (cursor & 1) != 0 {
            let Some(&pad) = bytes.get(cursor) else {
                return Err(SessionError::FragmentPayloadOverrun);
            };
            if pad != 0x00 {
                return Err(SessionError::NonZeroAlignmentPad { pad });
            }
            cursor += 1;
        }

        // Field 10: the 28-byte attachment (§9.3.9: the extension word selects it, never a flag).
        let attachment = if ext_word.unwrap_or(0) & EXT_WORD_ATTACHMENT != 0 {
            if bytes.len() < cursor + 28 {
                return Err(SessionError::FragmentPayloadOverrun);
            }
            let mut words64 = [0u64; 3];
            for (idx, slot) in words64.iter_mut().enumerate() {
                let off = cursor + idx * 8;
                *slot = u64::from_le_bytes([
                    bytes[off],
                    bytes[off + 1],
                    bytes[off + 2],
                    bytes[off + 3],
                    bytes[off + 4],
                    bytes[off + 5],
                    bytes[off + 6],
                    bytes[off + 7],
                ]);
            }
            let off32 = cursor + 24;
            let word32 = u32::from_le_bytes([
                bytes[off32],
                bytes[off32 + 1],
                bytes[off32 + 2],
                bytes[off32 + 3],
            ]);
            cursor += 28;
            Some(FragmentAttachment { words64, word32 })
        } else {
            None
        };

        Ok((
            Self {
                queue_id,
                ext_word,
                blob_flags,
                seq_group,
                order_num,
                blob_id,
                frag_index,
                num_frags,
                recipient_id,
                blob_header_word,
                frag_data_len,
                attachment,
            },
            cursor,
        ))
    }
}

/// A single blob fragment (header + payload slice, §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobFragment {
    /// Decoded fragment header.
    pub header: BlobFragmentHeader,
    /// Fragment payload slice (`header.frag_data_len` bytes).
    pub payload: Vec<u8>,
}

impl BlobFragment {
    /// Serializes this fragment (header + payload) into `out`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `payload.len() != header.frag_data_len as usize` or header validation fails.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        if self.payload.len() != self.header.frag_data_len as usize {
            return Err(SessionError::InvalidFragmentDataLen {
                len: self.payload.len() as u32,
            });
        }
        let h_len = self.header.encode(out)?;
        out.extend_from_slice(&self.payload);
        Ok(h_len + self.payload.len())
    }

    /// Decodes a single `BlobFragment` (header + payload) from `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the header is invalid or the payload slice overruns `bytes`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        let (header, h_len) = BlobFragmentHeader::decode(bytes)?;
        let data_len = header.frag_data_len as usize;
        let total = h_len
            .checked_add(data_len)
            .ok_or(SessionError::FragmentPayloadOverrun)?;
        if bytes.len() < total {
            return Err(SessionError::FragmentPayloadOverrun);
        }
        let payload = bytes[h_len..total].to_vec();
        Ok((Self { header, payload }, total))
    }
}

/// Splits an outgoing `FragmentedBlob` payload into `ceil(L / 436)` fragments (or 1 empty
/// fragment when `L == 0`, §5.6.4). The blob is never encrypted (§7.1, `OPEN-SESSION-6`).
/// `recipient_id` `None` or `Some(2)` sets `DefaultRecipient`.
///
/// # Errors
///
/// Returns [`SessionError`] if `queue_id` is outside `1..=4` or the payload exceeds
/// `MAX_BLOB_BYTES`.
pub fn split_blob(
    queue_id: u8,
    order_num: u16,
    blob_id: u16,
    seq_group: u16,
    recipient_id: Option<u64>,
    blob_header_word: u32,
    payload: &[u8],
) -> Result<Vec<BlobFragment>, SessionError> {
    if !(1..=4).contains(&queue_id) {
        return Err(SessionError::InvalidQueueId { queue_id });
    }
    let effective_payload = payload;

    if effective_payload.len() > MAX_BLOB_BYTES {
        return Err(SessionError::BlobTooLarge {
            total_bytes: effective_payload.len(),
            limit: MAX_BLOB_BYTES,
        });
    }

    let num_frags = if effective_payload.is_empty() {
        1u32
    } else {
        effective_payload.len().div_ceil(MAX_BLOB_FRAG_PAYLOAD) as u32
    };

    let mut base_flags = 0u8;
    if num_frags > 1 {
        base_flags |= BLOB_FLAG_IS_FRAGMENTED;
    }
    let rec_id = match recipient_id {
        None | Some(DEFAULT_BLOB_RECIPIENT_ID) => {
            base_flags |= BLOB_FLAG_DEFAULT_RECIPIENT;
            DEFAULT_BLOB_RECIPIENT_ID
        }
        Some(explicit) => explicit,
    };

    let mut out = Vec::with_capacity(num_frags as usize);
    for frag_idx in 0..num_frags {
        let start = (frag_idx as usize) * MAX_BLOB_FRAG_PAYLOAD;
        let end = ((frag_idx as usize + 1) * MAX_BLOB_FRAG_PAYLOAD).min(effective_payload.len());
        let slice = if effective_payload.is_empty() {
            &[][..]
        } else {
            &effective_payload[start..end]
        };
        let header = BlobFragmentHeader {
            queue_id,
            ext_word: None,
            blob_flags: base_flags,
            seq_group: if queue_id == 3 { seq_group } else { 0 },
            order_num: if (2..=4).contains(&queue_id) {
                order_num
            } else {
                0xFFFF
            },
            blob_id,
            frag_index: frag_idx,
            num_frags,
            recipient_id: rec_id,
            blob_header_word: if frag_idx == 0 {
                Some(blob_header_word)
            } else {
                None
            },
            frag_data_len: slice.len() as u32,
            attachment: None,
        };
        out.push(BlobFragment {
            header,
            payload: slice.to_vec(),
        });
    }
    Ok(out)
}

/// A completed, reassembled `FragmentedBlob` (§5.6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReassembledBlob {
    /// Delivery queue ID (`1..=4`, §5.6.2).
    pub queue_id: u8,
    /// Per-queue ordering number (`0xFFFF` for `queueId == 1`).
    pub order_num: u16,
    /// Sequence group (`queueId == 3`).
    pub seq_group: u16,
    /// 16-bit blob ID.
    pub blob_id: u16,
    /// 64-bit recipient ID.
    pub recipient_id: u64,
    /// 32-bit upper-layer message opcode / header word from fragment `0`.
    pub blob_header_word: u32,
    /// Reassembled payload bytes (the `MessageEnvelope` of §9.1).
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct IncompleteBlob {
    queue_id: u8,
    order_num: u16,
    seq_group: u16,
    blob_id: u16,
    recipient_id: u64,
    num_frags: u32,
    blob_header_word: Option<u32>,
    received_mask: Vec<u32>,
    received_count: u32,
    buffer: Vec<u8>,
    trimmed_len: Option<usize>,
    created_at_sec: f64,
}

impl IncompleteBlob {
    /// Whether the part `index` was received.
    fn has_part(&self, index: u32) -> bool {
        self.received_mask
            .get((index as usize) >> 5)
            .is_some_and(|w| w & (1u32 << (index & 0x1F)) != 0)
    }

    /// Whether `other` is the same entry, compared where parts were received
    /// ([`BlobReassembler::same_state`]).
    fn same_parts(&self, other: &Self) -> bool {
        let part = |entry: &Self, index: u32| {
            let offset = (index as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
            let end = if index.saturating_add(1) == entry.num_frags {
                entry.trimmed_len.unwrap_or(offset)
            } else {
                offset.saturating_add(MAX_BLOB_FRAG_PAYLOAD)
            };
            entry.buffer.get(offset..end).map(<[u8]>::to_vec)
        };
        self.queue_id == other.queue_id
            && self.order_num == other.order_num
            && self.seq_group == other.seq_group
            && self.blob_id == other.blob_id
            && self.recipient_id == other.recipient_id
            && self.num_frags == other.num_frags
            && self.blob_header_word == other.blob_header_word
            && self.received_mask == other.received_mask
            && self.received_count == other.received_count
            && self.trimmed_len == other.trimmed_len
            && self.created_at_sec == other.created_at_sec
            && (0..self.num_frags)
                .filter(|i| self.has_part(*i))
                .all(|i| part(self, i) == part(other, i))
    }
}

/// §9.3.9: how long a completed identity is remembered (30 seconds from completion).
///
/// §9.3.9: every 30-second storage expiry, tombstones, baselines and incomplete entries alike,
/// fires at an age greater than or equal to 30 seconds, before the new candidate is considered
/// ([`BlobReassembler::purge_expired`]).
pub const BLOB_TOMBSTONE_SEC: f64 = 30.0;
/// §9.3.9's cap on full-body tombstones of fragmented queue 1 and queues `2..=4`, shared per
/// connection and direction. Each delivered body has its own, so a repeat of a conflicting second
/// body deduplicates too.
pub const MAX_BLOB_TOMBSTONES: usize = 64;
/// §9.3.9: a queue-3 baseline expires this long after the last delivered blob of its key, and the
/// key may then take a new first value.
pub const QUEUE3_BASELINE_SEC: f64 = 30.0;
/// §9.3.9: the retained-body byte cap of completed tombstones per connection and direction
/// (8 MiB), next to the 64-tombstone count. At the end of each datagram the oldest completions are
/// evicted, and counted, until both caps hold.
pub const MAX_TOMBSTONE_BYTES: usize = 8 * 1024 * 1024;

/// A completed identity of fragmented queue 1 or queues `2..=4` (§9.3.9): its full body, opcode
/// and count, compared exactly and never by a digest. The body is shared, so a datagram's
/// transaction can keep the list as it was without copying a byte ([`PacketTransaction`]).
#[derive(Debug, Clone, PartialEq)]
struct BlobTombstone {
    key: BlobKey,
    at: f64,
    opcode: u32,
    num_frags: u32,
    body: Arc<[u8]>,
}

impl BlobTombstone {
    /// Whether one part of a multi-part blob is an exact repeat of this completed identity (§9.3.9):
    /// the same count, the same opcode on part 0, and the same bytes where the part lies, the final
    /// part's length included.
    fn holds_part(&self, hdr: &BlobFragmentHeader, data: &[u8]) -> bool {
        if self.num_frags != hdr.num_frags
            || (hdr.frag_index == 0 && hdr.blob_header_word != Some(self.opcode))
        {
            return false;
        }
        let offset = (hdr.frag_index as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
        let end = offset.saturating_add(data.len());
        let right_length =
            hdr.frag_index.saturating_add(1) < hdr.num_frags || end == self.body.len();
        right_length && self.body.get(offset..end).is_some_and(|b| b == data)
    }
}

/// One change a datagram made to the incomplete table or the queue-3 baselines, for its undoing.
#[derive(Debug)]
enum PacketUndo {
    /// An entry this datagram opened.
    Opened(BlobKey),
    /// A part this datagram wrote: the slot goes free again, so its bytes are not read any more.
    Part {
        key: BlobKey,
        index: u32,
        opcode: Option<u32>,
        trimmed_len: Option<usize>,
    },
    /// An entry this datagram completed or superseded.
    Removed(BlobKey, IncompleteBlob),
    /// A queue-3 baseline this datagram set or refreshed, with its value before.
    Baseline(Queue3Slot, Option<Queue3Baseline>),
}

/// Everything needed to put the reassembler back as it was before a datagram (§9.3.9: nothing is
/// committed until the whole datagram qualifies). Nothing is copied but the counters: entries and
/// tombstone bodies move or are shared.
#[derive(Debug)]
struct PacketTransaction {
    undo: Vec<PacketUndo>,
    counts: ReassemblyCounts,
    superseded: u64,
    /// The tombstones and their bytes, kept at the first change to them.
    tombstones: Option<(VecDeque<BlobTombstone>, usize)>,
}

/// What the reassembler did with what it did not deliver, and its anomalies (§9.3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReassemblyCounts {
    /// Exact retransmissions of a remembered identity, ignored.
    pub duplicates: u64,
    /// Reused identities delivered once with a reuse count (§9.3.9): a different complete body
    /// under a remembered identity, or a queue-3 word at distance zero with no exact tombstone.
    pub reused_identities: u64,
    /// Datagrams dropped whole for a fragment conflicting with an identity in progress.
    pub conflicts_dropped: u64,
    /// Remembered identities evicted at the cap before their time: an ordinary bounded-memory
    /// count, not an anomaly (§9.3.9).
    pub tombstones_evicted: u64,
    /// Completed queue-3 blobs older than the latest accepted of their key, discarded.
    pub queue3_older: u64,
    /// Completed queue-3 blobs at distance 32,768, delivered once without replacing the baseline.
    pub queue3_ambiguous: u64,
    /// Completed queue-3 blobs of a new key at a full [`MAX_ORDER_GROUPS`] map: skipped, their
    /// datagram accepted and acknowledged and their siblings delivered (§10.2 Vector 22).
    pub queue3_groups_over: u64,
    /// `OmitChunkInfo` control fragments, which have no application dispatch (§9.3.9).
    pub empty_controls: u64,
}

/// The identity of a blob (§5.6.4 rule 3): the sender's recipient ID, the queue, the blob ID for
/// queues `1`, `2` and `4` or the sequence group and ordering number for queue `3`, which carries
/// no blob ID (P10-22-POLICY), and the fragment's own 64-bit recipient (field 7, or its default)
/// on every queue. Another recipient is another identity, never a conflict.
type BlobKey = (u16, u8, u16, u16, u64);

/// A queue-3 comparison key (§9.3.9): sender, recipient and sequence group.
type Queue3Slot = (u16, u64, u16);

/// A queue-3 baseline: the latest word of its key, and when a blob of the key was last delivered.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Queue3Baseline {
    word: u16,
    delivered_at: f64,
}

/// What the reassembler did with one completed blob, for the live evidence log (§9.3.11 SESSION-19):
/// fixed words and numbers, never a body. Dropped at the next datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDecision {
    /// The fragment's queue.
    pub queue: u8,
    /// The sequence group (queue 3).
    pub group: u16,
    /// The ordering word.
    pub order: u16,
    /// The fragment's recipient: stays in memory, the log has a token for it.
    pub recipient: u64,
    /// `deliver`, `duplicate`, `older-discard` or `key-cap-skip`.
    pub action: &'static str,
    /// `first`, `newer`, `equal`, `ambiguous`, `older`, or `none` outside queue 3.
    pub distance: &'static str,
    /// Whether the key had a live baseline when the blob completed.
    pub baseline_present: bool,
}

/// The identity of `hdr` ([`BlobKey`]).
///
/// Labelled until the writer restores the definition (`docs/m8-remediation.md`, pass 10j): the
/// queue-3 identity keeps the ordering word, as §5.6.4 rule 3 keys queue-3 reassembly, so its
/// tombstones are per word while its baseline key (sender, recipient, group) is not.
fn blob_key(sender_rec_id: u16, hdr: &BlobFragmentHeader) -> BlobKey {
    if hdr.queue_id == 3 {
        (
            sender_rec_id,
            3,
            hdr.seq_group,
            hdr.order_num,
            hdr.recipient_id,
        )
    } else {
        (
            sender_rec_id,
            hdr.queue_id,
            hdr.blob_id,
            0,
            hdr.recipient_id,
        )
    }
}

/// An empty control (§9.3.9): exactly `OmitChunkInfo` with fragmentation `0x10`, `0x02`, `0x08` and
/// `0x20` all clear. Its opcode and length are zero and it has no application dispatch
/// (`OPEN-SESSION-14` keeps its meaning); `0x01`, `0x04`, the default recipient and a known
/// extension attachment add no application data. Any other omitted-chunk combination is an
/// invalid control, dropped with its datagram before this is asked ([`invalid_omitted_control`]).
const fn is_empty_control(hdr: &BlobFragmentHeader) -> bool {
    hdr.blob_flags & BLOB_FLAG_OMIT_CHUNK_INFO != 0
        && hdr.blob_flags & OMITTED_CONTROL_FORBIDDEN_FLAGS == 0
}

/// Whether a fragment is one of a blob with an identity (§9.3.9: an unfragmented queue-1 message has
/// none). Its decoded blob ID 0 is no blob ID, so it can neither repeat nor conflict with a
/// fragmented queue-1 blob whose ID is 0.
///
/// A fragmented queue-1 blob has this identity and, with queues 2–4, takes a completed tombstone
/// from the shared budget ([`takes_tombstone`]).
const fn has_identity(hdr: &BlobFragmentHeader) -> bool {
    takes_tombstone(hdr.queue_id, hdr.num_frags)
}

/// Whether a completed blob of `queue_id` with `num_frags` parts is remembered as a tombstone
/// (§9.3.9 queue comparison, P10-10J): fragmented queue 1 and queues 2–4 share one 64-tombstone
/// budget per connection and direction; an unfragmented queue-1 blob (ID zero) has no dedup
/// identity (§10.2 Vectors 22 and 24).
const fn takes_tombstone(queue_id: u8, num_frags: u32) -> bool {
    matches!(queue_id, 2..=4) || (queue_id == 1 && num_frags > 1)
}

/// Whether a fragment can join the identity in progress `entry` (§9.3.9: conflicting in-progress
/// fragments are never merged): the same count and, for an index already taken, the same bytes,
/// the final slice's length included.
fn agrees_with(entry: &IncompleteBlob, hdr: &BlobFragmentHeader, data: &[u8]) -> bool {
    if entry.num_frags != hdr.num_frags {
        return false;
    }
    if !entry.has_part(hdr.frag_index) {
        return true;
    }
    let offset = (hdr.frag_index as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
    let same_data = entry
        .buffer
        .get(offset..offset.saturating_add(data.len()))
        .is_some_and(|b| b == data);
    let same_word = hdr.frag_index != 0 || entry.blob_header_word == hdr.blob_header_word;
    let same_length = hdr.frag_index.saturating_add(1) != hdr.num_frags
        || entry.trimmed_len == Some(offset.saturating_add(data.len()));
    same_data && same_word && same_length
}

/// Bounded `FragmentedBlob` reassembler with a `30.0`-second expiration timer (§5.6.4).
#[derive(Debug, Clone)]
pub struct BlobReassembler {
    incomplete: HashMap<BlobKey, IncompleteBlob>,
    max_concurrent_blobs: usize,
    max_blob_bytes: usize,
    superseded: u64,
    /// Completed identities of fragmented queue 1 and queues `2..=4`, oldest first (§9.3.9).
    tombstones: VecDeque<BlobTombstone>,
    /// The bytes of [`Self::tombstones`]' bodies.
    tombstone_bytes: usize,
    /// The latest accepted word per queue-3 comparison key: sender, recipient and group (§9.3.9).
    queue3_latest: HashMap<Queue3Slot, Queue3Baseline>,
    counts: ReassemblyCounts,
    /// What the last datagram's completed blobs came to, for the evidence log.
    decisions: Vec<QueueDecision>,
}

/// Where a queue-3 ordering word lies from the latest accepted one of its comparison key (§9.3.9):
/// the candidate word minus the latest accepted word, modulo 65,536.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderBand {
    /// Distance 1 through 32,767.
    Newer,
    /// Distance zero.
    Equal,
    /// Distance 32,768.
    Ambiguous,
    /// The rest.
    Older,
}

/// The band of `word` from `reference` ([`OrderBand`]).
#[must_use]
pub const fn order_band(word: u16, reference: u16) -> OrderBand {
    match word.wrapping_sub(reference) {
        0 => OrderBand::Equal,
        1..=32_767 => OrderBand::Newer,
        32_768 => OrderBand::Ambiguous,
        _ => OrderBand::Older,
    }
}

impl Default for BlobReassembler {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_CONCURRENT_BLOBS,
            DEFAULT_MAX_REASSEMBLY_BLOB_BYTES,
        )
    }
}

impl BlobReassembler {
    /// Creates a bounded `BlobReassembler` with configurable caps on concurrent incomplete
    /// blobs and per-blob byte size (clamped to `MAX_BLOB_BYTES`, §5.6.4).
    #[must_use]
    pub fn new(max_concurrent_blobs: usize, max_blob_bytes: usize) -> Self {
        Self {
            incomplete: HashMap::new(),
            max_concurrent_blobs: max_concurrent_blobs.max(1),
            max_blob_bytes: max_blob_bytes.min(MAX_BLOB_BYTES),
            superseded: 0,
            tombstones: VecDeque::new(),
            tombstone_bytes: 0,
            queue3_latest: HashMap::new(),
            counts: ReassemblyCounts::default(),
            decisions: Vec::new(),
        }
    }

    /// The counts of what was ignored, discarded or odd (§9.3.8).
    #[must_use]
    pub const fn counts(&self) -> ReassemblyCounts {
        self.counts
    }

    /// The tombstones of `key`, each with one delivered body. A datagram's work starts with
    /// [`Self::purge_expired`], so what is here is live.
    fn tombstones_of<'a>(&'a self, key: &'a BlobKey) -> impl Iterator<Item = &'a BlobTombstone> {
        self.tombstones.iter().filter(move |t| t.key == *key)
    }

    /// Remembers a delivered body as its own tombstone (§9.3.9: "keep each delivered
    /// opcode/body/count as its OWN full tombstone"). Nothing is evicted here: §9.3.9 trims at the
    /// END of the datagram only ([`Self::trim_tombstones`] from [`Self::take_fragments`]), so every
    /// sibling sees the old unexpired tombstones plus the earlier completions of its datagram.
    ///
    /// The staging this allows is bounded (§9.3.9): a datagram has at most
    /// [`MAX_FRAGS_PER_PACKET`] fragments and so at most that many completions, each at most the
    /// reassembly limit (1 MiB by default). The tombstone's body is the one copy its parts are
    /// compared against ([`BlobTombstone::holds_part`] reads slices of it).
    fn remember(&mut self, tombstone: BlobTombstone, tx: &mut PacketTransaction) {
        tx.tombstones
            .get_or_insert_with(|| (self.tombstones.clone(), self.tombstone_bytes));
        self.tombstone_bytes = self.tombstone_bytes.saturating_add(tombstone.body.len());
        self.tombstones.push_back(tombstone);
    }

    /// At the end of a datagram: while either [`MAX_BLOB_TOMBSTONES`] or [`MAX_TOMBSTONE_BYTES`] is
    /// exceeded the oldest completion goes, counted as ordinary bounded memory, not an anomaly
    /// (§9.3.9).
    fn trim_tombstones(&mut self) {
        while self.tombstones.len() > MAX_BLOB_TOMBSTONES
            || self.tombstone_bytes > MAX_TOMBSTONE_BYTES
        {
            let Some(old) = self.tombstones.pop_front() else {
                break;
            };
            self.tombstone_bytes = self.tombstone_bytes.saturating_sub(old.body.len());
            self.counts.tombstones_evicted = self.counts.tombstones_evicted.saturating_add(1);
        }
    }

    /// Sets a queue-3 baseline with a delivery at `now_sec`, keeping what it was for the datagram's
    /// undoing.
    fn set_baseline(
        &mut self,
        slot: Queue3Slot,
        word: u16,
        now_sec: f64,
        tx: &mut PacketTransaction,
    ) {
        let before = self.queue3_latest.insert(
            slot,
            Queue3Baseline {
                word,
                delivered_at: now_sec,
            },
        );
        tx.undo.push(PacketUndo::Baseline(slot, before));
    }

    /// The queue-3 gate for a completed identity (§9.3.9): whether it is delivered, what it was, and
    /// the baseline and the superseding of older incomplete blobs done. The comparison key has the
    /// recipient, so another recipient has its own baseline. Every delivery refreshes its key's
    /// baseline clock ([`QUEUE3_BASELINE_SEC`]).
    ///
    /// Queue 2 delivers on arrival and queue 4 has no order gate, so only queue 3 comes here.
    ///
    /// §9.3.9 closed transport action table, baseline-cap row, and Vector 22: a new key at a full
    /// [`MAX_ORDER_GROUPS`] map skips: no eviction, no baseline, no tombstone and nothing superseded,
    /// while the datagram stays accepted and acknowledged and its siblings deliver. Distance 0 with
    /// no exact retained match delivers once with a reuse count and does not advance (counted by
    /// [`Self::completed`]).
    fn queue3_gate(
        &mut self,
        sender_rec_id: u16,
        recipient: u64,
        group: u16,
        order: u16,
        now_sec: f64,
        tx: &mut PacketTransaction,
    ) -> Queue3Verdict {
        let slot = (sender_rec_id, recipient, group);
        let Some(&latest) = self.queue3_latest.get(&slot) else {
            if self.queue3_latest.len() >= MAX_ORDER_GROUPS {
                self.counts.queue3_groups_over = self.counts.queue3_groups_over.saturating_add(1);
                return Queue3Verdict {
                    deliver: false,
                    action: "key-cap-skip",
                    distance: "first",
                    baseline_present: false,
                };
            }
            self.set_baseline(slot, order, now_sec, tx);
            self.supersede_older(sender_rec_id, recipient, group, order, tx);
            return Queue3Verdict {
                deliver: true,
                action: "deliver",
                distance: "first",
                baseline_present: false,
            };
        };
        let delivered = |distance| Queue3Verdict {
            deliver: true,
            action: "deliver",
            distance,
            baseline_present: true,
        };
        match order_band(order, latest.word) {
            OrderBand::Newer => {
                self.set_baseline(slot, order, now_sec, tx);
                self.supersede_older(sender_rec_id, recipient, group, order, tx);
                delivered("newer")
            }
            OrderBand::Equal => {
                // §9.3.9: delivered once with a reuse count ([`Self::completed`]); the baseline's
                // last-delivery time is refreshed, its word unchanged.
                self.set_baseline(slot, latest.word, now_sec, tx);
                delivered("equal")
            }
            OrderBand::Ambiguous => {
                self.counts.queue3_ambiguous = self.counts.queue3_ambiguous.saturating_add(1);
                self.set_baseline(slot, latest.word, now_sec, tx);
                delivered("ambiguous")
            }
            OrderBand::Older => {
                // [`Self::completed`] discards the older band before this gate; kept for safety.
                self.counts.queue3_older = self.counts.queue3_older.saturating_add(1);
                Queue3Verdict {
                    deliver: false,
                    action: "older-discard",
                    distance: "older",
                    baseline_present: true,
                }
            }
        }
    }

    /// A blob completed under `key` (§9.3.9 queue comparison, P10-10J): an exact repeat of a body
    /// remembered there is ignored at any distance, a different complete body is delivered once
    /// with a reuse count and its own tombstone, and on queue 3 the ordering gate decides.
    /// Fragmented queue 1 and queues 2–4 share the tombstone budget; an unfragmented queue-1 blob
    /// (ID zero) has no dedup identity ([`takes_tombstone`]).
    ///
    /// On queue 3 with a live baseline, a candidate in the older band (distance 32,769..65,535)
    /// ALWAYS discards before any tombstone is compared; otherwise an exact tombstone match
    /// deduplicates at every distance, newer, equal and half-range included, with no delivery or
    /// baseline advance. Distance zero without an exact match delivers once with a reuse count
    /// whether the old body is known different or no longer retained. A complete blob with an
    /// opcode and zero bytes is an ordinary blob here; only an empty control is skipped
    /// ([`is_empty_control`]).
    fn completed(
        &mut self,
        key: BlobKey,
        fields: CompletedFields,
        payload: Vec<u8>,
        tx: &mut PacketTransaction,
        now_sec: f64,
    ) -> Option<ReassembledBlob> {
        let CompletedFields {
            queue_id,
            order_num,
            seq_group,
            blob_id,
            recipient_id,
            opcode,
            num_frags,
        } = fields;
        let decision = |action, distance, baseline_present| QueueDecision {
            queue: queue_id,
            group: seq_group,
            order: order_num,
            recipient: recipient_id,
            action,
            distance,
            baseline_present,
        };
        // Queue 3: expiry ran first (`ingest_packet`); the older band discards before tombstones.
        if queue_id == 3
            && let Some(latest) = self.queue3_latest.get(&(key.0, recipient_id, seq_group))
            && order_band(order_num, latest.word) == OrderBand::Older
        {
            self.counts.queue3_older = self.counts.queue3_older.saturating_add(1);
            self.decisions
                .push(decision("older-discard", "older", true));
            return None;
        }
        let tombstoned = takes_tombstone(queue_id, num_frags);
        let mut reused = false;
        if tombstoned {
            let mut exact = false;
            for t in self.tombstones_of(&key) {
                reused = true;
                exact |= t.opcode == opcode && t.num_frags == num_frags && *t.body == *payload;
            }
            if exact {
                self.counts.duplicates = self.counts.duplicates.saturating_add(1);
                let baseline = self
                    .queue3_latest
                    .contains_key(&(key.0, recipient_id, seq_group));
                self.decisions
                    .push(decision("duplicate", "none", queue_id == 3 && baseline));
                return None;
            }
        }
        if queue_id == 3 {
            let verdict = self.queue3_gate(key.0, recipient_id, seq_group, order_num, now_sec, tx);
            self.decisions.push(decision(
                verdict.action,
                verdict.distance,
                verdict.baseline_present,
            ));
            if !verdict.deliver {
                return None;
            }
            // Distance zero without an exact retained match is a reuse (§9.3.9).
            reused |= verdict.distance == "equal";
        } else {
            self.decisions.push(decision("deliver", "none", false));
        }
        if reused {
            self.counts.reused_identities = self.counts.reused_identities.saturating_add(1);
        }
        if tombstoned {
            self.remember(
                BlobTombstone {
                    key,
                    at: now_sec,
                    opcode,
                    num_frags,
                    body: Arc::from(payload.as_slice()),
                },
                tx,
            );
        }
        Some(ReassembledBlob {
            queue_id,
            order_num,
            seq_group,
            blob_id,
            recipient_id,
            blob_header_word: opcode,
            payload,
        })
    }

    /// How many incomplete queue-3 blobs a newer completed one superseded.
    #[must_use]
    pub const fn superseded(&self) -> u64 {
        self.superseded
    }

    /// A queue-3 blob of `group` for `recipient` completed with `order` as the newest: an older
    /// incomplete one of that comparison key is superseded (§9.3.9). A completed blob is not
    /// touched.
    fn supersede_older(
        &mut self,
        sender_rec_id: u16,
        recipient: u64,
        group: u16,
        order: u16,
        tx: &mut PacketTransaction,
    ) {
        let older: Vec<BlobKey> = self
            .incomplete
            .keys()
            .copied()
            .filter(|&(sender, queue, g, o, r)| {
                sender == sender_rec_id
                    && queue == 3
                    && g == group
                    && r == recipient
                    && order_band(order, o) == OrderBand::Newer
            })
            .collect();
        for key in older {
            if let Some(entry) = self.incomplete.remove(&key) {
                tx.undo.push(PacketUndo::Removed(key, entry));
                self.superseded = self.superseded.saturating_add(1);
            }
        }
    }

    /// Returns how many completed identities are remembered, and the bytes of their bodies.
    #[must_use]
    pub fn tombstone_sizes(&self) -> (usize, usize) {
        (self.tombstones.len(), self.tombstone_bytes)
    }

    /// Returns the number of currently incomplete multi-fragment blobs.
    #[must_use]
    pub fn incomplete_len(&self) -> usize {
        self.incomplete.len()
    }

    /// When the oldest incomplete blob expires ([`BLOB_REASSEMBLY_EXPIRY_SEC`] after its
    /// first fragment, §5.6.4 rule 3); `None` with no incomplete blob.
    fn next_expiry(&self) -> Option<f64> {
        self.incomplete
            .values()
            .map(|entry| entry.created_at_sec + BLOB_REASSEMBLY_EXPIRY_SEC)
            .reduce(f64::min)
    }

    /// Purges incomplete blobs `30.0` seconds old or older (§5.6.4), tombstones
    /// [`BLOB_TOMBSTONE_SEC`] after completion or later and queue-3 baselines
    /// [`QUEUE3_BASELINE_SEC`] or more after their key's last delivery (§9.3.9), which frees their
    /// slots and lets those keys take a new first value.
    ///
    /// §9.3.9: every 30-second storage expiry fires at an age greater than or equal to 30
    /// seconds, before the new candidate is considered; a datagram's fragments all share one
    /// arrival time, so the purge at its start comes before each of its candidates.
    pub fn purge_expired(&mut self, now_sec: f64) {
        self.incomplete
            .retain(|_, entry| now_sec - entry.created_at_sec < BLOB_REASSEMBLY_EXPIRY_SEC);
        self.tombstones
            .retain(|t| now_sec - t.at < BLOB_TOMBSTONE_SEC);
        self.tombstone_bytes = self.tombstones.iter().map(|t| t.body.len()).sum();
        self.queue3_latest
            .retain(|_, b| now_sec - b.delivered_at < QUEUE3_BASELINE_SEC);
    }

    /// What the last datagram's completed blobs came to (the evidence log).
    pub fn take_decisions(&mut self) -> Vec<QueueDecision> {
        std::mem::take(&mut self.decisions)
    }

    /// The checks of one fragment that need no table state (§5.6.4 rules 1, 4, 5; §7.1;
    /// §10.1 rule 8).
    fn check_fragment(&self, frag: &BlobFragment) -> Result<(), SessionError> {
        let hdr = &frag.header;
        // §9.3.9 precedence: an invalid omitted-chunk combination drops first, before any other
        // fragment rule and before any terminal inference.
        if invalid_omitted_control(hdr) {
            return Err(SessionError::InvalidOmittedControl {
                queue_id: hdr.queue_id,
                blob_flags: hdr.blob_flags,
            });
        }
        // §7.1 (SESSION-6): an encrypted blob drops its whole datagram before reassembly.
        if (hdr.blob_flags & BLOB_FLAG_ENCRYPTED) != 0 {
            tracing::debug!(blob_id = hdr.blob_id, "encrypted FragmentedBlob rejected");
            return Err(SessionError::EncryptedBlobRejected);
        }
        if hdr.num_frags == 0
            || hdr.num_frags <= hdr.frag_index
            || ((hdr.blob_flags & BLOB_FLAG_IS_FRAGMENTED) != 0 && hdr.num_frags < 2)
        {
            return Err(SessionError::InvalidFragmentBounds {
                frag_index: hdr.frag_index,
                num_frags: hdr.num_frags,
            });
        }
        check_fragment_slice(hdr.frag_index, hdr.num_frags, hdr.frag_data_len)?;
        if frag.payload.len() != hdr.frag_data_len as usize {
            return Err(SessionError::InvalidFragmentDataLen {
                len: hdr.frag_data_len,
            });
        }

        // §5.6.4 rule 1: the worst-case buffer of a fragmented blob, `numFrags * 436`, fits the
        // configured limit before anything is allocated, the same check for a final and a non-final
        // part. So at the default limit a count of 2404 passes and 2405 fails, whatever the final
        // length, which deliberately excludes some shorter totals. `max_blob_bytes` is never above
        // the architectural ceiling (`Self::new`).
        if hdr.num_frags > 1 {
            let allocated = (hdr.num_frags as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
            if allocated > self.max_blob_bytes {
                return Err(SessionError::BlobTooLarge {
                    total_bytes: allocated,
                    limit: self.max_blob_bytes,
                });
            }
        }
        Ok(())
    }

    /// The checks of a datagram's fragments that need no table state: what passes here is
    /// well-formed, and these are safe to run before its checksum.
    ///
    /// # Errors
    ///
    /// The error of the first fragment that fails (§5.6.4 rules 1, 4, 5; §7.1; §10.1 rule 8).
    pub fn check_fragments(&self, fragments: &[BlobFragment]) -> Result<(), SessionError> {
        fragments.iter().try_for_each(|f| self.check_fragment(f))
    }

    /// Takes one datagram's fragments in order, all or nothing (§9.3.9: nothing is committed until
    /// the whole datagram qualifies, and a bad sibling prevents all sibling delivery): this is the
    /// one place that judges a fragment, so a caller commits a sequence only after a datagram that
    /// has already gone in whole, and nothing can fail after that commit. A refused datagram leaves
    /// the table as it was. Expired entries and tombstones go first (§5.6.4 rule 3, §9.3.9), which
    /// any later call at this time would do too.
    ///
    /// Returns the blobs delivered, in the order they completed. §9.3.9 delivers no empty control,
    /// exact repeat of a remembered identity or older queue-3 word.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::check_fragments`], [`SessionError::ConflictingBlobIdentity`] for a
    /// fragment that conflicts with the identity in progress, in this datagram or before it, and
    /// [`SessionError::ReassemblyTableFull`] when a fragment would open an entry past the cap.
    pub fn ingest_packet(
        &mut self,
        sender_rec_id: u16,
        fragments: &[BlobFragment],
        now_sec: f64,
    ) -> Result<Vec<ReassembledBlob>, SessionError> {
        self.check_fragments(fragments)?;
        self.purge_expired(now_sec);
        self.decisions.clear();
        let mut tx = self.begin_packet();
        match self.take_fragments(sender_rec_id, fragments, now_sec, &mut tx) {
            Ok(blobs) => Ok(blobs),
            Err(err) => {
                self.roll_back(tx);
                Err(err)
            }
        }
    }

    /// Answers what [`Self::ingest_packet`] would answer for this datagram now, by running that same
    /// code and undoing it: the table is as it was afterwards, past the expiry that call makes too.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::ingest_packet`].
    pub fn check_packet(
        &mut self,
        sender_rec_id: u16,
        fragments: &[BlobFragment],
        now_sec: f64,
    ) -> Result<(), SessionError> {
        self.check_fragments(fragments)?;
        self.purge_expired(now_sec);
        let mut tx = self.begin_packet();
        let judged = self.take_fragments(sender_rec_id, fragments, now_sec, &mut tx);
        self.roll_back(tx);
        judged.map(|_| ())
    }

    /// Counts a datagram dropped for a conflicting identity (§9.3.9).
    pub fn note_conflict_dropped(&mut self) {
        self.counts.conflicts_dropped = self.counts.conflicts_dropped.saturating_add(1);
    }

    /// One fragment as a datagram of its own ([`Self::ingest_packet`]).
    ///
    /// Returns `Ok(Some(ReassembledBlob))` when all fragments of the blob have arrived
    /// (immediately for `numFrags == 1`) and it is to be delivered, or `Ok(None)` when waiting for
    /// remaining fragments or when §9.3.9 does not deliver it.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::ingest_packet`].
    pub fn ingest(
        &mut self,
        sender_rec_id: u16,
        frag: BlobFragment,
        now_sec: f64,
    ) -> Result<Option<ReassembledBlob>, SessionError> {
        self.ingest_packet(sender_rec_id, std::slice::from_ref(&frag), now_sec)
            .map(|mut blobs| blobs.pop())
    }

    /// Whether `other` holds the same table: the same entries with the same bytes where a part was
    /// received, the same tombstones, baselines and counts. Bytes of a free slot are never read, so
    /// they are not compared. For tests and fuzz targets.
    #[doc(hidden)]
    #[must_use]
    pub fn same_state(&self, other: &Self) -> bool {
        self.counts == other.counts
            && self.superseded == other.superseded
            && self.tombstones == other.tombstones
            && self.tombstone_bytes == other.tombstone_bytes
            && self.queue3_latest == other.queue3_latest
            && self.incomplete.len() == other.incomplete.len()
            && self
                .incomplete
                .iter()
                .all(|(key, a)| other.incomplete.get(key).is_some_and(|b| a.same_parts(b)))
    }

    fn begin_packet(&self) -> PacketTransaction {
        PacketTransaction {
            undo: Vec::new(),
            counts: self.counts,
            superseded: self.superseded,
            tombstones: None,
        }
    }

    /// Puts back everything `tx` recorded, newest first.
    fn roll_back(&mut self, tx: PacketTransaction) {
        for change in tx.undo.into_iter().rev() {
            match change {
                PacketUndo::Opened(key) => {
                    self.incomplete.remove(&key);
                }
                PacketUndo::Part {
                    key,
                    index,
                    opcode,
                    trimmed_len,
                } => {
                    if let Some(entry) = self.incomplete.get_mut(&key) {
                        if let Some(word) = entry.received_mask.get_mut((index as usize) >> 5) {
                            *word &= !(1u32 << (index & 0x1F));
                        }
                        entry.received_count = entry.received_count.saturating_sub(1);
                        entry.blob_header_word = opcode;
                        entry.trimmed_len = trimmed_len;
                    }
                }
                PacketUndo::Removed(key, entry) => {
                    self.incomplete.insert(key, entry);
                }
                PacketUndo::Baseline(slot, Some(baseline)) => {
                    self.queue3_latest.insert(slot, baseline);
                }
                PacketUndo::Baseline(slot, None) => {
                    self.queue3_latest.remove(&slot);
                }
            }
        }
        self.counts = tx.counts;
        self.superseded = tx.superseded;
        self.decisions.clear();
        if let Some((tombstones, bytes)) = tx.tombstones {
            self.tombstones = tombstones;
            self.tombstone_bytes = bytes;
        }
    }

    fn take_fragments(
        &mut self,
        sender_rec_id: u16,
        fragments: &[BlobFragment],
        now_sec: f64,
        tx: &mut PacketTransaction,
    ) -> Result<Vec<ReassembledBlob>, SessionError> {
        let mut blobs = Vec::new();
        let staged_before = self.tombstones.len();
        for frag in fragments {
            if let Some(blob) = self.take_fragment(sender_rec_id, frag, now_sec, tx)? {
                blobs.push(blob);
            }
        }
        // §9.3.9 staging bound: one completion per fragment at most, each within the reassembly
        // limit, so the transient history before trimming stays bounded.
        debug_assert!(self.tombstones.len().saturating_sub(staged_before) <= MAX_FRAGS_PER_PACKET);
        debug_assert!(
            self.tombstones
                .iter()
                .skip(staged_before)
                .all(|t| t.body.len() <= self.max_blob_bytes.max(MAX_PACKET_BLOB_BUDGET))
        );
        // §9.3.9: count and byte trimming at the END of the datagram only.
        self.trim_tombstones();
        Ok(blobs)
    }

    /// The one judgement of one well-formed fragment ([`Self::check_fragments`]). An empty control
    /// is skipped before any identity is looked at (§9.3.9: empty controls create no baseline,
    /// tombstone, incomplete entry or delivery).
    fn take_fragment(
        &mut self,
        sender_rec_id: u16,
        frag: &BlobFragment,
        now_sec: f64,
        tx: &mut PacketTransaction,
    ) -> Result<Option<ReassembledBlob>, SessionError> {
        let hdr = &frag.header;
        if is_empty_control(hdr) {
            self.counts.empty_controls = self.counts.empty_controls.saturating_add(1);
            return Ok(None);
        }
        let key = blob_key(sender_rec_id, hdr);

        // §9.3.9 "Retained old parts" and §10.2 Vector 24: a part of a fragmented identity is
        // compared with EVERY retained completed tombstone of its key first. An exact old part is
        // ignored and counted, whether or not an entry of that identity is open and whether or not
        // that entry already holds its index: it opens nothing, fills and overwrites no slot, and
        // does not reject the accepted datagram, so its siblings still count. Only a differing
        // NON-old part at an already filled index is a conflict (`agrees_with` below).
        //
        // §9.3.9 and §5.6.4 rule 3 settle the cost: a reused identity whose new body shares such a
        // part cannot be told from a repeat, in either order of its parts. The part is ignored and,
        // once acknowledged, need not come again, so that new body may be LOST, not merely delayed;
        // its entry expires under §5.6.4. Nothing is taken from the old body (never merged).
        if hdr.num_frags > 1
            && has_identity(hdr)
            && self
                .tombstones_of(&key)
                .any(|t| t.holds_part(hdr, &frag.payload))
        {
            self.counts.duplicates = self.counts.duplicates.saturating_add(1);
            return Ok(None);
        }
        if has_identity(hdr)
            && let Some(entry) = self.incomplete.get(&key)
            && !agrees_with(entry, hdr, &frag.payload)
        {
            return Err(SessionError::ConflictingBlobIdentity);
        }

        // Fast path for single-fragment blobs (numFrags == 1, fragIndex == 0, §5.6.4 rule 2)
        if hdr.num_frags == 1 {
            let fields = CompletedFields {
                queue_id: hdr.queue_id,
                order_num: hdr.order_num,
                seq_group: hdr.seq_group,
                blob_id: hdr.blob_id,
                recipient_id: hdr.recipient_id,
                opcode: hdr.blob_header_word.unwrap_or(0),
                num_frags: 1,
            };
            return Ok(self.completed(key, fields, frag.payload.clone(), tx, now_sec));
        }

        if !self.incomplete.contains_key(&key) {
            // §9.3.9: a NEW incomplete entry at the 64-entry cap drops and counts the whole
            // datagram, with no Ack and no terminal result; no existing entry is evicted.
            if self.incomplete.len() >= self.max_concurrent_blobs {
                return Err(SessionError::ReassemblyTableFull {
                    max_concurrent: self.max_concurrent_blobs,
                });
            }
            let num_frags = hdr.num_frags as usize;
            self.incomplete.insert(
                key,
                IncompleteBlob {
                    queue_id: hdr.queue_id,
                    order_num: hdr.order_num,
                    seq_group: hdr.seq_group,
                    blob_id: hdr.blob_id,
                    recipient_id: hdr.recipient_id,
                    num_frags: hdr.num_frags,
                    blob_header_word: None,
                    received_mask: vec![0u32; num_frags.div_ceil(32)],
                    received_count: 0,
                    buffer: vec![0u8; num_frags.saturating_mul(MAX_BLOB_FRAG_PAYLOAD)],
                    trimmed_len: None,
                    created_at_sec: now_sec,
                },
            );
            tx.undo.push(PacketUndo::Opened(key));
        }
        let Some(entry) = self.incomplete.get_mut(&key) else {
            return Ok(None);
        };

        let bit_mask = 1u32 << (hdr.frag_index & 0x1F);
        let offset = (hdr.frag_index as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
        let end = offset.saturating_add(frag.payload.len());
        if let (Some(word), Some(slot)) = (
            entry.received_mask.get_mut((hdr.frag_index as usize) >> 5),
            entry.buffer.get_mut(offset..end),
        ) && *word & bit_mask == 0
        {
            tx.undo.push(PacketUndo::Part {
                key,
                index: hdr.frag_index,
                opcode: entry.blob_header_word,
                trimmed_len: entry.trimmed_len,
            });
            *word |= bit_mask;
            entry.received_count = entry.received_count.saturating_add(1);
            slot.copy_from_slice(&frag.payload);
            if hdr.frag_index == 0 {
                entry.blob_header_word = hdr.blob_header_word;
            }
            if hdr.frag_index.saturating_add(1) == hdr.num_frags {
                entry.trimmed_len = Some(end);
            }
        }

        if entry.received_count != entry.num_frags {
            return Ok(None);
        }
        let Some(done) = self.incomplete.remove(&key) else {
            return Ok(None);
        };
        let body = done
            .buffer
            .get(..done.trimmed_len.unwrap_or(done.buffer.len()))
            .unwrap_or_default()
            .to_vec();
        let fields = CompletedFields {
            queue_id: done.queue_id,
            order_num: done.order_num,
            seq_group: done.seq_group,
            blob_id: done.blob_id,
            recipient_id: done.recipient_id,
            opcode: done.blob_header_word.unwrap_or(0),
            num_frags: done.num_frags,
        };
        // The full entry stays with the transaction until the datagram has gone in whole.
        tx.undo.push(PacketUndo::Removed(key, done));
        Ok(self.completed(key, fields, body, tx, now_sec))
    }
}

/// What the queue-3 gate decided ([`BlobReassembler::queue3_gate`]).
#[derive(Debug, Clone, Copy)]
struct Queue3Verdict {
    deliver: bool,
    action: &'static str,
    distance: &'static str,
    baseline_present: bool,
}

/// What [`BlobReassembler::completed`] needs of a completed blob besides its body.
#[derive(Debug, Clone, Copy)]
struct CompletedFields {
    queue_id: u8,
    order_num: u16,
    seq_group: u16,
    blob_id: u16,
    recipient_id: u64,
    opcode: u32,
    num_frags: u32,
}

// ---------------------------------------------------------------------------
// §4.1–§4.4 & §10.1 Full UDP Datagram Framing (Encode & Decode)
// ---------------------------------------------------------------------------

/// A fully decoded UDP datagram (`wirePrefix` + `DatagramHeader` + `OptionalHeaders` + `BlobFragment`s).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPacket {
    /// Two-byte wire prefix (`0x0000` if unencoded, or `DatagramHeader.recId` if bit-tree compressed, §4.1.1).
    pub wire_prefix: u16,
    /// Decoded 20-byte `DatagramHeader` (§4.2).
    pub header: DatagramHeader,
    /// Decoded optional headers in ascending bit order (§4.3).
    pub opt_headers: OptionalHeaders,
    /// Decoded blob fragments (non-empty only when `header.flags & 0x00000004 != 0`, §5.6.3).
    pub fragments: Vec<BlobFragment>,
    /// Raw payload bytes (`header.pak_size` bytes immediately following the 20-byte `DatagramHeader`).
    pub raw_payload: Vec<u8>,
    /// The per-buffer `payloadSum` of §4.4.2 step 2 over the received bytes as they are:
    /// each optional header, each fragment header (its alignment pad and attachment included,
    /// as one buffer, §9.3.9) and each fragment's data slice as its own buffer.
    pub payload_sum: u32,
}

/// Encodes a packet (`header`, `opt_headers`, `fragments`) into a raw UDP datagram (§4.1–§4.4).
///
/// - Computes `pak_size` from the serialized optional headers and blob fragments.
/// - Sets `HasFragmentedBlobsFlag` (`0x00000004`) if `fragments` is non-empty, and
///   `MaskedChecksumFlag` (`0x00000002`) if `xor_mask.is_some()`.
/// - Computes `DatagramHeader.checksum` from the `0xBADD70DD` header placeholder, the
///   per-buffer `payloadSum` ([`payload_checksum`]) and the optional `xor_mask` (§4.4.2).
/// - Prepends `0x0000` when `wire_codec` is `None`, or `header.rec_id` (big-endian) followed by
///   the bit-tree compressed `[DatagramHeader[2..20] | payload]` when `wire_codec` is `Some` (§4.1.1, §4.1.2).
///   A packet with a `SessionAuthHeader` is a control packet and is always written
///   unencoded (§4.1.1); `wire_codec` is not used for it.
/// - Every intermediate buffer of a packet with a `SessionAuthHeader` is wiped when it is
///   dropped (`official-login.md` §6.3); the returned datagram belongs to the caller.
///
/// # Errors
///
/// Returns [`SessionError`] if optional headers or fragments are invalid, a flag outside
/// `0x0F7FFB06` is set, or the payload exceeds `MAX_PAK_SIZE`.
pub fn encode_datagram(
    mut header: DatagramHeader,
    opt_headers: &OptionalHeaders,
    fragments: &[BlobFragment],
    xor_mask: Option<u32>,
    wire_codec: Option<&dyn BitTreeCodec>,
) -> Result<Vec<u8>, SessionError> {
    if fragments.len() > MAX_FRAGS_PER_PACKET {
        return Err(SessionError::TooManyFragments {
            count: fragments.len(),
        });
    }

    // A SessionAuthHeader carries the ticket: every intermediate copy is reserved at its
    // exact size and wiped when it is dropped, on the error paths too (`official-login.md`
    // §6.3). The returned datagram belongs to the caller.
    let secret = opt_headers.session_auth.is_some();
    let mut frag_bytes = Vec::new();
    for frag in fragments {
        frag.encode(&mut frag_bytes)?;
    }
    let mut payload = SecretBuf::with_capacity(
        opt_headers.encoded_len().saturating_add(frag_bytes.len()),
        secret,
    );
    opt_headers.encode(&mut payload.buf)?;
    payload.buf.extend_from_slice(&frag_bytes);

    let Ok(pak_size) = u16::try_from(payload.buf.len()) else {
        return Err(SessionError::InvalidPakSize {
            pak_size: u16::MAX,
            actual_payload_len: payload.buf.len(),
        });
    };
    if pak_size > MAX_PAK_SIZE {
        return Err(SessionError::InvalidPakSize {
            pak_size,
            actual_payload_len: payload.buf.len(),
        });
    }

    header.pak_size = pak_size;
    header.flags |= opt_headers.flags_mask();
    if !fragments.is_empty() {
        header.flags |= FLAG_HAS_FRAGMENTED_BLOBS;
    }
    if xor_mask.is_some() {
        header.flags |= FLAG_MASKED_CHECKSUM;
    }
    if (header.flags & INVALID_DATAGRAM_FLAGS_MASK) != 0 {
        return Err(SessionError::InvalidHeaderFlags {
            flags: header.flags,
        });
    }

    // §4.4.2 step 2: `payloadSum` per optional header buffer and per fragment header and
    // data slice; the receiver sums the same buffers ([`decode_datagram`]).
    let payload_sum = payload_checksum(opt_headers, fragments)?;
    header.checksum = combine_packet_checksum(header, payload_sum, xor_mask)?;
    let wire_hdr = header.to_wire_bytes();

    // A SessionAuthHeader packet is a control packet and always goes out unencoded
    // (§4.1.1), so the ticket never passes through a codec's own buffers.
    match wire_codec.filter(|_| !secret) {
        Some(codec) => {
            let mut plain_after_rec_id = SecretBuf::with_capacity(18 + payload.buf.len(), secret);
            plain_after_rec_id.buf.extend_from_slice(&wire_hdr[2..20]);
            plain_after_rec_id.buf.extend_from_slice(&payload.buf);

            let mut out = Vec::with_capacity(2 + plain_after_rec_id.buf.len());
            out.extend_from_slice(&header.rec_id.to_be_bytes());
            codec.encode(&plain_after_rec_id.buf, &mut out, MAX_DATAGRAM_BUF - 2)?;
            Ok(out)
        }
        None => {
            let mut out = Vec::with_capacity(2 + DATAGRAM_HEADER_BYTES + payload.buf.len());
            out.extend_from_slice(&0u16.to_be_bytes());
            out.extend_from_slice(&wire_hdr);
            out.extend_from_slice(&payload.buf);
            Ok(out)
        }
    }
}

/// The optional headers, the fragments and the per-buffer `payloadSum` (§4.4.2 step 2)
/// of a decoded packet's `payload` for header `flags` (§4.3, §5.6.3, §10.1 rules 3, 7, 8).
///
/// # Errors
///
/// Returns [`SessionError`] if a header or fragment is malformed, trailing bytes remain or
/// a fragment breaks the receive policy of [`check_fragment_receive_policy`].
pub fn parse_packet_payload(
    flags: u32,
    payload: &[u8],
    from_server: bool,
) -> Result<(OptionalHeaders, Vec<BlobFragment>, u32), SessionError> {
    let mut marks = Vec::new();
    let (opt_headers, mut cursor) =
        OptionalHeaders::decode_marked(flags, payload, from_server, &mut marks)?;
    let mut sum = 0u32;
    let mut from = 0usize;
    for end in marks {
        if let Some(buffer) = payload.get(from..end) {
            sum = sum.wrapping_add(compute_datagram_checksum(buffer));
        }
        from = end;
    }
    let mut fragments = Vec::new();
    if (flags & FLAG_HAS_FRAGMENTED_BLOBS) != 0 {
        while let Some(rest) = payload.get(cursor..).filter(|r| !r.is_empty()) {
            if fragments.len() >= MAX_FRAGS_PER_PACKET {
                return Err(SessionError::TooManyFragments {
                    count: fragments.len() + 1,
                });
            }
            let (frag, consumed) = BlobFragment::decode(rest)?;
            // §9.3.9's receive policy, before any checksum buffer of an unverifiable fragment
            // is formed.
            check_fragment_receive_policy(&frag.header)?;
            let header_len = consumed.saturating_sub(frag.payload.len());
            if let Some(header) = rest.get(..header_len) {
                sum = sum
                    .wrapping_add(compute_datagram_checksum(header))
                    .wrapping_add(compute_datagram_checksum(&frag.payload));
            }
            cursor += consumed;
            fragments.push(frag);
        }
    } else if cursor < payload.len() {
        return Err(SessionError::TrailingPayloadBytes {
            remaining: payload.len() - cursor,
        });
    }
    Ok((opt_headers, fragments, sum))
}

/// Whether a datagram's raw length lets it reach the decoder (§10.1 rule 1).
///
/// Engineering reading of §10.1 rule 1, pending the writer's answer: the rule does not say whether
/// its `22` bytes come before or after wire decompression, and a compressed datagram can be much
/// shorter on the wire than its decompressed header (§4.1, §4.2). So a zero wire prefix keeps the
/// raw minimum of [`MIN_DATAGRAM_BYTES`], a non-zero prefix needs only itself and one compressed
/// byte ([`MIN_COMPRESSED_DATAGRAM_BYTES`]), and [`decode_datagram`] still requires the whole
/// header and `pakSize` after decompression.
#[must_use]
pub fn raw_length_admitted(datagram: &[u8]) -> bool {
    match datagram.first_chunk::<2>() {
        None => false,
        Some([0, 0]) => datagram.len() >= MIN_DATAGRAM_BYTES,
        Some(_) => datagram.len() >= MIN_COMPRESSED_DATAGRAM_BYTES,
    }
}

/// Decodes and validates a raw UDP datagram before sequence-window/ISAAC-mask lookup (§4.1–§4.3, §10.1 rules 1–3, 7, 8).
///
/// The payload is parsed first, so that its per-buffer `payloadSum`
/// ([`DecodedPacket::payload_sum`], §4.4.2) is known. If `verify_unmasked_checksum` is
/// `true` and `MaskedChecksumFlag` (`0x00000002`) is clear, `DatagramHeader.checksum` is
/// verified at once. A masked packet's checksum is verified with
/// [`verify_packet_checksum`] after the sequence window check, with the sequence number's
/// ISAAC `xor_mask` (§5.1, §10.1 rules 5–6).
///
/// # Errors
///
/// Returns [`SessionError`] if any hostile-input rule in §10.1 is violated.
pub fn decode_datagram(
    datagram: &[u8],
    from_server: bool,
    wire_codec: Option<&dyn BitTreeCodec>,
    verify_unmasked_checksum: bool,
) -> Result<DecodedPacket, SessionError> {
    if !raw_length_admitted(datagram) {
        return Err(SessionError::DatagramTooShort {
            len: datagram.len(),
        });
    }
    if datagram.len() > MAX_DATAGRAM_BUF {
        return Err(SessionError::DatagramTooLarge {
            len: datagram.len(),
        });
    }

    let wire_prefix = u16::from_be_bytes([datagram[0], datagram[1]]);
    let decoded_buf: Vec<u8> = if wire_prefix == 0x0000 {
        datagram[2..].to_vec()
    } else {
        let codec = wire_codec.ok_or(SessionError::MissingWireCodec { wire_prefix })?;
        let mut out = Vec::with_capacity(datagram.len());
        // Prepend the 2-byte big-endian recId from wirePrefix so bytes 0..20 form the full DatagramHeader (§4.1.2)
        out.extend_from_slice(&datagram[0..2]);
        codec.decode(&datagram[2..], &mut out, MAX_DATAGRAM_BUF - 2)?;
        out
    };

    let Some(payload) = decoded_buf.get(DATAGRAM_HEADER_BYTES..) else {
        return Err(SessionError::DatagramTooShort {
            len: decoded_buf.len() + 2,
        });
    };
    let header = DatagramHeader::from_wire_bytes(&decoded_buf)?;
    if usize::from(header.pak_size) != payload.len() {
        return Err(SessionError::InvalidPakSize {
            pak_size: header.pak_size,
            actual_payload_len: payload.len(),
        });
    }

    let (opt_headers, fragments, payload_sum) =
        parse_packet_payload(header.flags, payload, from_server)?;
    if verify_unmasked_checksum && (header.flags & FLAG_MASKED_CHECKSUM) == 0 {
        verify_packet_checksum(header, payload_sum, None)?;
    }

    Ok(DecodedPacket {
        wire_prefix,
        header,
        opt_headers,
        fragments,
        raw_payload: payload.to_vec(),
        payload_sum,
    })
}

// ---------------------------------------------------------------------------
// §6.1 Connection State Machine (0..8), Reliability, and Multiplexer
// ---------------------------------------------------------------------------

/// Lifecycle state (`0..8`) of a session connection (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectionState {
    /// State `0`: `Idle` / `Uninitialized`.
    Idle = 0,
    /// State `1`: `SessionAuthSent` (retransmits `SessionAuthHeader` or `ReferralAuthHeader` every `2.0` s, timeout `20.0` s).
    SessionAuthSent = 1,
    /// State `2`: `ChallengeAcceptSent` (retransmits `ConnectAcceptHeader` every `0.6` s, timeout `20.0` s).
    ChallengeAcceptSent = 2,
    /// State `3`: `ServerListening` (unused on client).
    ServerListening = 3,
    /// State `4`: `ServerConnectSent` (unused on client).
    ServerConnectSent = 4,
    /// State `5`: `Connected` (established bidirectional session).
    Connected = 5,
    /// State `6`: `DisconnectingFlush` (draining outbound reliable packets, up to `20.0` s).
    DisconnectingFlush = 6,
    /// State `7`: `DisconnectSent` (retransmitting `DisconnectHeader` every `1.0` s, up to `20.0` s).
    DisconnectSent = 7,
    /// State `8`: `Closing` (terminal state before cleanup).
    Closing = 8,
}

/// Why a [`SessionConnection`] closed (§5.1, §6.1, §6.4, §6.7, §8.2). Only wire values and
/// Tessera's own reasons appear here; no client-internal status code is modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// No `ConnectChallengeHeader` (State `1`) or no further server packet (State `2`)
    /// arrived within `20.0` s (§6.1).
    HandshakeTimeout,
    /// No valid packet for `60.0` s (or `300.0` s with the extended timeout) in State `5` (§8.2).
    SilenceTimeout,
    /// The server sent `DisconnectHeader` (`0x00008000`, §8.2).
    ServerDisconnect,
    /// The server sent `TransportErrorHeader` (`fatal == false`) or
    /// `TransportFatalErrorHeader` (`fatal == true`) (§6.7).
    TransportError {
        /// Whether the header was the fatal variant (`0x00200000`).
        fatal: bool,
        /// Wire `errorCode`.
        error_code: u32,
        /// Wire `errorSubcode`.
        error_subcode: u32,
    },
    /// The challenge's dictionary UUID was null, did not match the install's incoming
    /// dictionary, or the connection has no dictionaries (§7.3: an explicit
    /// unsupported-negotiation result; the chat connection's dictionary source is
    /// `OPEN-SESSION-1`).
    UnsupportedCodec,
    /// The retransmit table overflowed its limits (§5.1).
    OutboundWindowOverflow,
    /// A local disconnect finished: `DisconnectHeader` was retransmitted for `20.0` s in
    /// State `7` (§6.1, §8.2).
    LocalDisconnect,
    /// A checksum-valid later packet opened a sequence gap that stayed open for
    /// [`MAX_GAP_AGE_SEC`] (§9.3.9: the outcome table's deadline row).
    GapDeadline,
    /// A checksum-valid `blobFlags` `0x02` on queues 2–4 (§9.3.9: the whole transport-level
    /// terminal unsupported class).
    UnsupportedTraffic,
    /// A challenge with a zero recipient or peer word (§9.3.9: unsupported at once, every auth
    /// retry cancelled, no reply).
    InvalidChallenge,
}

impl DisconnectReason {
    /// A neutral label for a log line: a kind, never a server's number (§9.3.8).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HandshakeTimeout => "handshake timeout",
            Self::SilenceTimeout => "silence timeout",
            Self::ServerDisconnect => "server disconnect",
            Self::TransportError { fatal: true, .. } => "server fatal error",
            Self::TransportError { fatal: false, .. } => "server transport error",
            Self::LocalDisconnect => "local disconnect",
            Self::UnsupportedCodec => "unsupported dictionary",
            Self::GapDeadline => "sequence gap deadline",
            Self::OutboundWindowOverflow => "outbound window overflow",
            Self::UnsupportedTraffic => "unsupported packet form",
            Self::InvalidChallenge => "invalid challenge",
        }
    }
}

/// High-level events emitted by a [`SessionConnection`] when processing incoming datagrams or timers.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// Connection transitioned to `ConnectionState::Connected` (State `5`).
    Connected {
        /// Assigned client local `recId`.
        local_rec_id: u16,
        /// Assigned remote server `recId`.
        server_rec_id: u16,
    },
    /// Server sent a `WorldReferralHeader` (`0x00000800`) to a world server (§6.5). A
    /// referral whose address or port is zero raises no event: its fallback is
    /// `OPEN-SESSION-2`.
    WorldReferralReceived {
        /// The referral's opaque cookie (payload bytes `0..7`), for
        /// [`SessionConnection::start_world_login`], which reverses it.
        cookie: [u8; 8],
        /// The world server's IPv4 address and UDP port (payload bytes `8..13`).
        world_addr: SocketAddrV4,
    },
    /// Server sent an `AuthServerAddressHeader` (`0x00000200`, §6.6).
    AuthServerAddress(SocketAddrV4),
    /// A complete `FragmentedBlob` was reassembled (§5.6.4).
    BlobReceived(ReassembledBlob),
    /// The connection closed (§5.1, §6.1, §6.4, §6.7, §8.2).
    Disconnected {
        /// Why it closed.
        reason: DisconnectReason,
    },
}

/// What an accepted packet changes in the sequence state (computed before any commit).
#[derive(Debug)]
enum SequenceCommit {
    /// Unsequenced packet: nothing.
    Unsequenced,
    /// A missing sequence number arrived: remove it from the gap table.
    FillGap(u32),
    /// A new highest sequence number: the advanced generator, the masks saved for the
    /// skipped numbers, and the new `recvSeqExpected`.
    Advance {
        isaac: Box<IsaacCipher>,
        skipped_masks: Vec<(u32, u32)>,
        seq: u32,
    },
}

/// A missing incoming sequence number (§5.3, §7.2).
#[derive(Debug, Clone, Copy)]
struct SequenceGap {
    /// The ISAAC mask drawn for it when a later packet was accepted.
    mask: u32,
    /// The `now_sec` of that packet: gaps open in sequence order, so the lowest sequence number
    /// is the oldest ([`MAX_GAP_AGE_SEC`]).
    opened_at: f64,
}

/// Saved outbound sequenced packet in the retransmission table (§5.1, §5.3).
#[derive(Debug, Clone)]
pub struct SentPacketRecord {
    /// Assigned outgoing sequence number (`>= 2`).
    pub seq_num: u32,
    /// Saved outgoing ISAAC XOR mask drawn when `seq_num` was first assigned (§7.2).
    pub xor_mask: u32,
    /// Saved client word of header bytes `16..17`, reused on retransmission (§4.2
    /// P10-22-POLICY).
    pub client_word: u16,
    /// Optional headers carried in the packet.
    pub opt_headers: OptionalHeaders,
    /// Blob fragments carried in the packet.
    pub fragments: Vec<BlobFragment>,
    /// True if the packet still has reliable content that should be retransmitted on `SequenceNakHeader`
    /// (false if it only carried ephemeral `queueId == 1` fragments that have expired, §5.3).
    pub retransmit_eligible: bool,
    /// When [`Self::retransmit_eligible`] is false: the packet is resent on a
    /// `SequenceNakHeader` all the same, until it is acknowledged or the connection ends
    /// ([`SessionConnection::send_blob_retained`]).
    pub retained: bool,
    /// The packet carries the retained `0xF4` ([`SessionConnection::send_blob_kept`]): unlike the
    /// generic §5.3 record, the implicit acknowledgement below a NAK's first number never frees
    /// it; only an explicit cumulative Ack covering it or the connection's close does (§5.3 F4
    /// exception, §9.3.9 F4 retention, §10.2 Vector 24).
    pub kept_until_acked: bool,
}

/// The last thing the transport did with a watched kept blob ([`SessionConnection::send_blob_watched`],
/// the item-13 report of `official-session.md` §9.3.12) that its peer caused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeptBlobAction {
    /// A cumulative Ack covered every record of it: its retention ended.
    CoveredByCumulativeAck,
    /// A `SequenceNakHeader` named one of its records, which was resent with its saved bytes.
    NakResend,
    /// A `SequenceNakHeader` named one of its records after the record was gone, so an
    /// `OutOfOrderAckHeader` answers it (§5.3, §5.4).
    OutOfOrderAck,
}

impl KeptBlobAction {
    /// A neutral word for an evidence line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CoveredByCumulativeAck => "covered_by_cumulative_ack",
            Self::NakResend => "nak_resend",
            Self::OutOfOrderAck => "out_of_order_ack",
        }
    }
}

/// What the transport did with a watched kept blob, for the evidence log: counts and the
/// connection's own sequence numbers only, never a byte of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeptBlobFate {
    /// The private sequence of its first transmission (its first datagram).
    pub first_sequence: u32,
    /// How many datagrams it took.
    pub datagrams: u32,
    /// A cumulative Ack covered every one of its records.
    pub covered_by_cumulative_ack: bool,
    /// Resends of its records on a `SequenceNakHeader`.
    pub nak_resends: u64,
    /// NAKs for its records answered with an `OutOfOrderAckHeader`.
    pub out_of_order_acks: u64,
    /// The latest of those, `None` while the peer did none of them.
    pub last: Option<KeptBlobAction>,
}

/// Most datagrams of one watched blob whose sequence numbers are kept (a resource-set report of
/// two 4096-bit fields fills at most three; engineering choice, spec silent).
const MAX_WATCHED_SEQUENCES: usize = 8;

/// The watched kept blob of a connection and its sequences.
#[derive(Debug, Clone, Default)]
struct WatchedBlob {
    seqs: Vec<u32>,
    fate: KeptBlobFate,
}

/// What one connection counted of its handshake and its datagrams, for diagnostics: counts
/// only, never an address, a port or datagram bytes (decision 0044 item 3). Engineering
/// choice, spec silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HandshakeCounts {
    /// `SessionAuthHeader` datagrams queued by [`SessionConnection::start_login`] (§6.2).
    pub session_auth: u64,
    /// `ReferralAuthHeader` datagrams queued by [`SessionConnection::start_world_login`]
    /// (§6.5).
    pub referral_auth: u64,
    /// Retransmits of the stored `SessionAuthHeader` or `ReferralAuthHeader` in State `1`
    /// (§6.1, every `2.0` s).
    pub auth_retransmits: u64,
    /// `ConnectAcceptHeader` datagrams queued in answer to a `ConnectChallengeHeader` (§6.4).
    pub connect_accept: u64,
    /// Retransmits of the `ConnectAcceptHeader` in State `2` (§6.4, every `0.6` s).
    pub connect_accept_retransmits: u64,
    /// `ConnectChallengeHeader`s that passed the §10.1 checks and were acted on (§6.3,
    /// §6.4), an unsupported `codecUuid` included.
    pub challenges: u64,
    /// Datagrams handed to the socket ([`SessionMultiplexer::flush_outbound`]).
    pub sent: u64,
    /// Datagrams the socket refused to send.
    pub send_failures: u64,
    /// Received datagrams that passed every §10.1 check and were taken whole (their
    /// optional headers acted on and their fragments ingested); a datagram refused at any
    /// step is not counted.
    pub received_ok: u64,
    /// Received datagrams rejected whole, with no commit and no Ack, because a fragment was
    /// on receiver queue `5` or `6`, whose framing is `OPEN-SESSION-12`
    /// ([`SessionError::UnsupportedReceiveQueue`]).
    pub unsupported_queue_drops: u64,
    /// Received datagrams rejected whole, with no commit and no Ack, because a fragment had
    /// `blobFlags` bit `0x02` outside queue `1` (`OPEN-SESSION-14`)
    /// ([`SessionError::UnsupportedFragment`]).
    pub unsupported_fragment_drops: u64,
    /// Received datagrams a connected connection refused before committing anything,
    /// whatever the reason, a duplicate of a sequence number already taken apart. A diagnostic:
    /// such datagrams are not authenticated, and [`MAX_GAP_AGE_SEC`] is what ends a stall.
    pub refused_while_connected: u64,
    /// Retained packets resent on a `SequenceNakHeader`
    /// ([`SessionConnection::send_blob_retained`]).
    pub retained_resends: u64,
}

/// The install's two directional dictionary UUIDs, each the raw on-disk bytes `16..31` of
/// its record (§4.1.3, §7.3): kept apart, never merged into one acceptable list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodecUuids {
    /// The incoming (server-to-client) dictionary's UUID (key `0x10000001` after its
    /// redirect): a challenge must name it, in its field-wise converted form (§6.3, §7.3).
    pub incoming: [u8; 16],
    /// The outgoing (client-to-server) dictionary's UUID (key `0x10000002`): written
    /// verbatim into the accept (§6.4, §7.3).
    pub outgoing: [u8; 16],
}

/// The client word of header bytes `16..17` in the accept (§4.2, §6.4).
///
/// After the accept the word follows the interim P10-22-POLICY of §4.2 (unverified on the
/// official service, `OPEN-SESSION-8`): it advances by one (modulo 65,536) for each newly
/// allocated sequenced packet, so sequences `2` and `3` carry `2` and `3`, and a
/// retransmission keeps the packet's saved word. Unsequenced packets after the accept carry
/// the word of the last allocated packet without advancing it (§4.2 P10-23-POLICY, §10.2
/// Vector 9).
pub const INITIAL_CLIENT_WORD: u16 = 1;

/// The log target of a live official run's evidence (`official-session.md` §9.3.9 "Evidence for
/// transport producers"; `docs/DEVELOPMENT.md` "Reading a live official run"). Its lines show which
/// labelled policy Tessera actually used, for correlating a refusal: never an address, port, account,
/// ticket, seed, cookie, name, ID, path, identifier or payload. They cannot show the official
/// producer's rules.
pub const EVIDENCE_TARGET: &str = "official_evidence";
/// Most lines of one connection's packets that carry something (a fragment or a resend), past the
/// first of each opcode (at most [`MAX_EVIDENCE_OPCODES`] more). Acks and flow reports cannot use
/// these up.
pub const MAX_EVIDENCE_SEND_LINES: u32 = 400;
/// Most lines of one connection's standalone Acks, NAKs, OutOfOrderAcks and flow-only reports.
pub const MAX_EVIDENCE_CONTROL_LINES: u32 = 120;
/// Most lines of one connection's handshake packets and Disconnects.
pub const MAX_EVIDENCE_LIFECYCLE_LINES: u32 = 64;
/// Most lines of one connection's accepted datagrams and delivered blobs, past the first of each
/// opcode (at most [`MAX_EVIDENCE_OPCODES`] more).
pub const MAX_EVIDENCE_RECEIVE_LINES: u32 = 400;
/// Most lines of one connection's queue decisions that discard, skip or repeat a blob or deliver at an
/// odd distance, past the first of each class. They do not spend the receive lines: the discards of a
/// crowded entry come after many deliveries.
pub const MAX_EVIDENCE_QUEUE_LINES: u32 = 64;
/// Most lines of one connection's plain queue-3 deliveries (first or newer), past the first of each
/// class. A plain delivery past these has no line: its blob line has its group and order, and the
/// totals count it.
pub const MAX_EVIDENCE_QUEUE_DELIVER_LINES: u32 = 32;
/// The classes of queue decision, each with one line that no budget can refuse: four actions by six
/// distances at most ([`QueueDecision`]).
pub const MAX_EVIDENCE_QUEUE_CLASSES: usize = 24;
/// §9.3.7's cap on distinct opcodes, for the first-of-each lines of each direction.
pub const MAX_EVIDENCE_OPCODES: usize = 256;
/// The newest lines past their budget that are kept and written when the connection ends, so that
/// what came just before a close is in the log however long the connection ran.
pub const MAX_EVIDENCE_TAIL_LINES: usize = 64;
/// Most evidence lines of one connection, totals included. The tail and the totals go out once, at
/// its close or when it is dropped, and nothing is written after them.
pub const MAX_EVIDENCE_LINES_PER_CONNECTION: usize = (MAX_EVIDENCE_SEND_LINES
    + MAX_EVIDENCE_CONTROL_LINES
    + MAX_EVIDENCE_LIFECYCLE_LINES
    + MAX_EVIDENCE_RECEIVE_LINES
    + MAX_EVIDENCE_QUEUE_LINES
    + MAX_EVIDENCE_QUEUE_DELIVER_LINES)
    as usize
    + MAX_EVIDENCE_QUEUE_CLASSES
    + 2 * MAX_EVIDENCE_OPCODES
    + MAX_EVIDENCE_TAIL_LINES
    + 1;
/// What the flow accumulator counts (§5.5 P10-22-POLICY).
const FLOW_SCOPE: &str = "udp-payload-of-accepted-datagrams";

/// How the local word of a sent packet came to be (§9.3.9 "Evidence for transport producers").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordInput {
    /// A sequenced send: one more (P10-22-POLICY).
    Sequenced,
    /// A standalone control: kept as it is (§9.3.9).
    ControlUnchanged,
    /// A resend: the saved word.
    ResendSaved,
    /// The accept: the interim first word (§4.2, §6.4).
    AcceptInitial,
    /// A handshake packet that does not move the word: the auth, and a repeated accept.
    HandshakeUnchanged,
}

impl WordInput {
    const fn label(self) -> &'static str {
        match self {
            Self::Sequenced => "sequenced-plus-one",
            Self::ControlUnchanged => "control-unchanged",
            Self::ResendSaved => "resend-saved",
            Self::AcceptInitial => "accept-initial",
            Self::HandshakeUnchanged => "handshake-unchanged",
        }
    }
}

/// What kind of packet a send line is (§9.3.9: retransmit versus new send).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendKind {
    New,
    Resend,
    Ack,
    Nak,
    OutOfOrderAck,
    Auth,
    AuthResend,
    Accept,
    AcceptResend,
    Disconnect,
    DisconnectResend,
}

impl SendKind {
    const fn label(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Resend => "resend",
            Self::Ack => "ack",
            Self::Nak => "nak",
            Self::OutOfOrderAck => "out-of-order-ack",
            Self::Auth => "auth",
            Self::AuthResend => "auth-resend",
            Self::Accept => "accept",
            Self::AcceptResend => "accept-resend",
            Self::Disconnect => "disconnect",
            Self::DisconnectResend => "disconnect-resend",
        }
    }
}

/// One sent packet's evidence line. Nothing of a packet's contents is copied here but the flow
/// counters and the first fragment's numbers: no ticket, cookie or body.
#[derive(Debug, Clone, Copy)]
struct SendEvidence {
    kind: SendKind,
    seq: u32,
    word_before: u16,
    word_after: u16,
    input: WordInput,
    flow_event: &'static str,
    flow: Option<FlowControlHeader>,
    /// What the flow accumulator holds as this line is written ([`SessionConnection::note_send`]).
    flow_accumulated: u32,
    /// The first fragment's numbers, and how many.
    fragment: Option<SentFragment>,
    fragments: usize,
}

/// A send line's first fragment (§9.3.11 SESSION-19): its queue, opcode (its first part only), index
/// and count, and what was allocated for it. The recipient stays in memory: the line has a token.
#[derive(Debug, Clone, Copy)]
struct SentFragment {
    queue: u8,
    opcode: Option<u32>,
    part: u32,
    parts: u32,
    group: u16,
    order: u16,
    blob_id: u16,
    recipient: u64,
    default_form: bool,
}

impl SentFragment {
    /// Which of the sender's interim counters gave this fragment its numbers (§5.6.2).
    const fn allocation_kind(&self) -> &'static str {
        match (self.queue, has_blob_id(self.queue, self.parts > 1)) {
            (3, _) => "group-order",
            (1, true) => "blob-id",
            (1, false) => "none",
            (_, true) => "order-and-blob-id",
            (_, false) => "order",
        }
    }
}

impl SendEvidence {
    fn of(kind: SendKind, seq: u32, opt: &OptionalHeaders, frags: &[BlobFragment]) -> Self {
        Self {
            kind,
            seq,
            word_before: 0,
            word_after: 0,
            input: WordInput::ControlUnchanged,
            flow_event: "none",
            flow: opt.flow_control,
            flow_accumulated: 0,
            fragment: frags.first().map(|f| {
                let h = &f.header;
                SentFragment {
                    queue: h.queue_id,
                    opcode: h.blob_header_word,
                    part: h.frag_index,
                    parts: h.num_frags,
                    group: h.seq_group,
                    order: h.order_num,
                    blob_id: h.blob_id,
                    recipient: h.recipient_id,
                    default_form: h.blob_flags & BLOB_FLAG_DEFAULT_RECIPIENT != 0,
                }
            }),
            fragments: frags.len(),
        }
    }

    const fn word(mut self, before: u16, after: u16, input: WordInput) -> Self {
        self.word_before = before;
        self.word_after = after;
        self.input = input;
        self
    }
}

/// One line of a connection's evidence log, as it is written or kept for the tail.
#[derive(Debug, Clone, Copy)]
enum EvidenceLine {
    Send {
        line: SendEvidence,
        first_of_opcode: bool,
        recipient_token: u32,
    },
    Datagram {
        seq: u32,
        bytes: usize,
        fragments: usize,
        gaps: usize,
    },
    Blob {
        queue: u8,
        opcode: u32,
        group: u16,
        order: u16,
        bytes: usize,
        first: bool,
        recipient_token: u32,
        default_recipient: bool,
    },
    /// What the queue rules did with a completed blob: every one of queue 3, and any other that
    /// was not delivered.
    Queue {
        decision: QueueDecision,
        recipient_token: u32,
    },
}

/// A line past its budget, with the stage and time it belongs to.
#[derive(Debug, Clone, Copy)]
struct KeptLine {
    stage: &'static str,
    ms: u64,
    line: EvidenceLine,
}

/// A connection's evidence log: off unless the connector turns it on (only its official runs do), and
/// bounded ([`MAX_EVIDENCE_LINES_PER_CONNECTION`]). Each class of line has its own budget, so Acks
/// and flow reports cannot use up the lines of what carries an opcode. A line past its budget goes
/// to a ring of the newest ([`MAX_EVIDENCE_TAIL_LINES`]), written with `tail=true` at the
/// connection's close, so what came before the close is there; only what falls out of the ring is
/// counted as not shown. The tail and the totals line go out once, at the close or when the
/// connection is dropped. The log ends there: a closed connection still routes by its ID and may
/// take valid datagrams, and those are neither written nor counted in anything that is written.
#[derive(Debug)]
struct Evidence {
    role: &'static str,
    stage: &'static str,
    stage_since: f64,
    send_left: u32,
    control_left: u32,
    lifecycle_left: u32,
    receive_left: u32,
    queue_left: u32,
    queue_deliver_left: u32,
    /// The (action, distance) classes that have had a line.
    queue_classes: BTreeSet<(&'static str, &'static str)>,
    /// The reassembler's counts at the last datagram, for the totals.
    reassembly: ReassemblyCounts,
    /// Plain queue-3 deliveries, with or without a line.
    queue3_plain_delivered: u64,
    opcodes: BTreeSet<u32>,
    sent_opcodes: BTreeSet<u32>,
    /// The recipients seen, in order: a line names the ordinal, never the ID (§9.3.11's tokens).
    recipients: Vec<u64>,
    tail: VecDeque<KeptLine>,
    sends: u64,
    resends: u64,
    controls: u64,
    handshakes: u64,
    handshake_resends: u64,
    disconnects: u64,
    queued_not_sent: u64,
    datagrams: u64,
    blobs: u64,
    not_shown: u64,
    /// The tail and the totals are out, and nothing more is written.
    finished: bool,
}

impl Evidence {
    fn new(role: &'static str, now_sec: f64) -> Self {
        Self {
            role,
            stage: "handshake",
            stage_since: now_sec,
            send_left: MAX_EVIDENCE_SEND_LINES,
            control_left: MAX_EVIDENCE_CONTROL_LINES,
            lifecycle_left: MAX_EVIDENCE_LIFECYCLE_LINES,
            receive_left: MAX_EVIDENCE_RECEIVE_LINES,
            queue_left: MAX_EVIDENCE_QUEUE_LINES,
            queue_deliver_left: MAX_EVIDENCE_QUEUE_DELIVER_LINES,
            queue_classes: BTreeSet::new(),
            reassembly: ReassemblyCounts::default(),
            queue3_plain_delivered: 0,
            opcodes: BTreeSet::new(),
            sent_opcodes: BTreeSet::new(),
            recipients: Vec::new(),
            tail: VecDeque::new(),
            sends: 0,
            resends: 0,
            controls: 0,
            handshakes: 0,
            handshake_resends: 0,
            disconnects: 0,
            queued_not_sent: 0,
            datagrams: 0,
            blobs: 0,
            not_shown: 0,
            finished: false,
        }
    }

    /// The ordinal of `recipient` on this connection, from 1; 0 past [`MAX_EVIDENCE_OPCODES`] of
    /// them.
    fn recipient_token(&mut self, recipient: u64) -> u32 {
        if let Some(i) = self.recipients.iter().position(|r| *r == recipient) {
            return (i as u32).saturating_add(1);
        }
        if self.recipients.len() >= MAX_EVIDENCE_OPCODES {
            return 0;
        }
        self.recipients.push(recipient);
        self.recipients.len() as u32
    }

    /// Milliseconds since the stage started, floored.
    fn ms(&self, now_sec: f64) -> u64 {
        ((now_sec - self.stage_since).max(0.0) * 1000.0) as u64
    }

    /// Whether a line may go out on the budget `left`, which it then uses.
    fn budgeted(left: &mut u32) -> bool {
        if *left == 0 {
            return false;
        }
        *left -= 1;
        true
    }

    /// Writes `line` now when `shown`, or keeps it for the tail; nothing once the log has ended.
    fn put(&mut self, line: EvidenceLine, shown: bool, now_sec: f64) {
        if self.finished {
            return;
        }
        let kept = KeptLine {
            stage: self.stage,
            ms: self.ms(now_sec),
            line,
        };
        if shown {
            self.write(kept, false);
            return;
        }
        if self.tail.len() >= MAX_EVIDENCE_TAIL_LINES {
            self.tail.pop_front();
            self.not_shown += 1;
        }
        self.tail.push_back(kept);
    }

    fn write(&self, kept: KeptLine, tail: bool) {
        let KeptLine { stage, ms, line } = kept;
        match line {
            EvidenceLine::Send {
                line,
                first_of_opcode,
                recipient_token,
            } => {
                let f = line.fragment;
                let flow = line.flow.unwrap_or(FlowControlHeader {
                    bytes_received_delta: 0,
                    interval_ms: 0,
                });
                tracing::info!(
                    target: EVIDENCE_TARGET,
                    role = self.role,
                    stage,
                    ms,
                    dir = "send",
                    kind = line.kind.label(),
                    seq = line.seq,
                    word_before = line.word_before,
                    word_after = line.word_after,
                    word_input = line.input.label(),
                    queue = f.map_or(0, |f| f.queue),
                    opcode = format_args!("{:#x}", f.and_then(|f| f.opcode).unwrap_or(0)),
                    part = f.map_or(0, |f| f.part),
                    parts = f.map_or(0, |f| f.parts),
                    group = f.map_or(0, |f| f.group),
                    order = f.map_or(0, |f| f.order),
                    blob_id = f.map_or(0, |f| f.blob_id),
                    allocation_kind = f.map_or("none", |f| f.allocation_kind()),
                    recipient_token,
                    target_resolution_kind = match f {
                        Some(f) if f.default_form => "default-recipient",
                        Some(_) => "explicit",
                        None => "none",
                    },
                    fragments = line.fragments,
                    first_of_opcode,
                    flow_event = line.flow_event,
                    flow_reported_bytes = flow.bytes_received_delta,
                    flow_reported_ms = flow.interval_ms,
                    flow_accumulated = line.flow_accumulated,
                    flow_scope = FLOW_SCOPE,
                    tail,
                    "official evidence: packet"
                );
            }
            EvidenceLine::Datagram {
                seq,
                bytes,
                fragments,
                gaps,
            } => {
                tracing::info!(
                    target: EVIDENCE_TARGET,
                    role = self.role,
                    stage,
                    ms,
                    dir = "recv",
                    seq,
                    bytes,
                    fragments,
                    gaps,
                    tail,
                    "official evidence: datagram"
                );
            }
            EvidenceLine::Blob {
                queue,
                opcode,
                group,
                order,
                bytes,
                first,
                recipient_token,
                default_recipient,
            } => {
                tracing::info!(
                    target: EVIDENCE_TARGET,
                    role = self.role,
                    stage,
                    ms,
                    dir = "recv",
                    queue,
                    opcode = format_args!("{opcode:#x}"),
                    group,
                    order,
                    bytes,
                    first_of_opcode = first,
                    recipient_token,
                    target_resolution_kind = if default_recipient {
                        "default-recipient"
                    } else {
                        "raw-recipient"
                    },
                    tail,
                    "official evidence: blob"
                );
            }
            EvidenceLine::Queue {
                decision,
                recipient_token,
            } => {
                tracing::info!(
                    target: EVIDENCE_TARGET,
                    role = self.role,
                    stage,
                    ms,
                    dir = "recv",
                    queue = decision.queue,
                    group = decision.group,
                    order = decision.order,
                    recipient_token,
                    queue_action = decision.action,
                    distance_class = decision.distance,
                    baseline_present = decision.baseline_present,
                    tail,
                    "official evidence: queue"
                );
            }
        }
    }

    fn sent(&mut self, line: SendEvidence, now_sec: f64) {
        let mut first = false;
        let shown = match line.kind {
            SendKind::New | SendKind::Resend => {
                if line.kind == SendKind::New {
                    self.sends += 1;
                    let opcode = line.fragment.and_then(|f| f.opcode);
                    first = opcode.is_some_and(|op| {
                        self.sent_opcodes.len() < MAX_EVIDENCE_OPCODES
                            && self.sent_opcodes.insert(op)
                    });
                } else {
                    self.resends += 1;
                }
                // A sequenced packet with no fragment (a flow-only report) is no carrier.
                if line.fragment.is_none() && line.kind == SendKind::New {
                    Self::budgeted(&mut self.control_left)
                } else {
                    first || Self::budgeted(&mut self.send_left)
                }
            }
            SendKind::Ack | SendKind::Nak | SendKind::OutOfOrderAck => {
                self.controls += 1;
                Self::budgeted(&mut self.control_left)
            }
            SendKind::Auth | SendKind::Accept => {
                self.handshakes += 1;
                Self::budgeted(&mut self.lifecycle_left)
            }
            SendKind::AuthResend | SendKind::AcceptResend => {
                self.handshake_resends += 1;
                Self::budgeted(&mut self.lifecycle_left)
            }
            SendKind::Disconnect | SendKind::DisconnectResend => {
                self.disconnects += 1;
                Self::budgeted(&mut self.lifecycle_left)
            }
        };
        let recipient_token = line
            .fragment
            .map_or(0, |f| self.recipient_token(f.recipient));
        self.put(
            EvidenceLine::Send {
                line,
                first_of_opcode: first,
                recipient_token,
            },
            shown,
            now_sec,
        );
    }

    fn accepted(&mut self, seq: u32, len: usize, fragments: usize, gaps: usize, now_sec: f64) {
        self.datagrams += 1;
        let shown = Self::budgeted(&mut self.receive_left);
        let line = EvidenceLine::Datagram {
            seq,
            bytes: len,
            fragments,
            gaps,
        };
        self.put(line, shown, now_sec);
    }

    fn delivered(&mut self, blob: &ReassembledBlob, now_sec: f64) {
        self.blobs += 1;
        let first =
            self.opcodes.len() < MAX_EVIDENCE_OPCODES && self.opcodes.insert(blob.blob_header_word);
        let shown = first || Self::budgeted(&mut self.receive_left);
        let line = EvidenceLine::Blob {
            queue: blob.queue_id,
            opcode: blob.blob_header_word,
            group: blob.seq_group,
            order: blob.order_num,
            bytes: blob.payload.len(),
            first,
            recipient_token: self.recipient_token(blob.recipient_id),
            default_recipient: blob.recipient_id == DEFAULT_BLOB_RECIPIENT_ID,
        };
        self.put(line, shown, now_sec);
    }

    /// What the queue rules did with a completed blob ([`EvidenceLine::Queue`]). The first of each class
    /// always has its line, so the first key-cap skip of a crowded entry is shown however much came
    /// before it. These lines spend no receive line. Past its budget a plain delivery has no line at
    /// all, not even in the tail, where it would push out what came before a close; any other goes to
    /// the tail. The totals have the counts either way.
    fn queued(&mut self, decision: QueueDecision, now_sec: f64) {
        if decision.queue != 3 && decision.action == "deliver" {
            return;
        }
        let first = self.queue_classes.len() < MAX_EVIDENCE_QUEUE_CLASSES
            && self
                .queue_classes
                .insert((decision.action, decision.distance));
        let plain = decision.action == "deliver" && matches!(decision.distance, "first" | "newer");
        if plain {
            self.queue3_plain_delivered = self.queue3_plain_delivered.saturating_add(1);
        }
        let budget = if plain {
            &mut self.queue_deliver_left
        } else {
            &mut self.queue_left
        };
        let shown = first || Self::budgeted(budget);
        if plain && !shown {
            return;
        }
        let recipient_token = self.recipient_token(decision.recipient);
        self.put(
            EvidenceLine::Queue {
                decision,
                recipient_token,
            },
            shown,
            now_sec,
        );
    }

    /// The tail and the totals, once: at the connection's close, or when it is dropped.
    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        while let Some(kept) = self.tail.pop_front() {
            self.write(kept, true);
        }
        tracing::info!(
            target: EVIDENCE_TARGET,
            role = self.role,
            stage = self.stage,
            sends = self.sends,
            resends = self.resends,
            controls = self.controls,
            handshakes = self.handshakes,
            handshake_resends = self.handshake_resends,
            disconnects = self.disconnects,
            queued_not_sent = self.queued_not_sent,
            datagrams = self.datagrams,
            blobs = self.blobs,
            opcodes = self.opcodes.len(),
            sent_opcodes = self.sent_opcodes.len(),
            queue3_plain_delivered = self.queue3_plain_delivered,
            queue3_groups_over = self.reassembly.queue3_groups_over,
            queue3_older = self.reassembly.queue3_older,
            queue3_ambiguous = self.reassembly.queue3_ambiguous,
            reused_identities = self.reassembly.reused_identities,
            duplicates = self.reassembly.duplicates,
            tombstones_evicted = self.reassembly.tombstones_evicted,
            lines_not_shown = self.not_shown,
            "official evidence: connection totals"
        );
    }
}

impl Drop for Evidence {
    fn drop(&mut self) {
        self.finish();
    }
}

/// The interim flow-report counters of §5.5 and the closed P10-10L schedule of §9.3.12
/// (native production `OPEN-SESSION-13`).
///
/// Byte counting and report eligibility are separate: every accepted datagram adds its bytes to
/// the accumulator, but only eligible traffic ([`FlowReport::count`]) makes a later report due.
/// Pure Ack/NAK/OutOfOrderAck/flow-only traffic alone never does (§9.3.12), so a server that only
/// acknowledges gets one first report and nothing more, and its bytes wait for the next report
/// that eligible traffic makes due. Every report after the first, standalone or attached, waits
/// [`FLOW_REPORT_ONLY_PERIOD_SEC`] after the last report sent on the connection.
#[derive(Debug, Clone, Copy)]
struct FlowReport {
    /// UDP payload bytes of this connection's accepted peer datagrams since `since_sec`, modulo
    /// 2³² (§9.3.12).
    bytes: u32,
    /// The initial auth transmission, or the last report sent.
    since_sec: f64,
    /// Eligible traffic ([`FlowReport::count`]) arrived since the last report.
    eligible: bool,
    /// The first FlowControl-only packet after State `5` went out.
    initial_sent: bool,
    /// When the last report, standalone or attached, went out (meaningful once `initial_sent`).
    last_report_sec: f64,
}

impl FlowReport {
    fn new(now_sec: f64) -> Self {
        Self {
            bytes: 0,
            since_sec: now_sec,
            eligible: false,
            initial_sent: false,
            last_report_sec: now_sec,
        }
    }

    /// When a tick in State `5` sends a FlowControl-only packet (§9.3.12 P10-10L,
    /// [`FLOW_REPORT_ONLY_PERIOD_SEC`]): at once for the first one, then only once eligible
    /// traffic arrived that no other report carried, and not before one second after the
    /// previous report, standalone or attached. `None` when nothing is due. The tick and
    /// [`SessionConnection::next_deadline`] both read this, so they agree.
    fn flow_only_due_at(&self) -> Option<f64> {
        if !self.initial_sent {
            Some(f64::NEG_INFINITY)
        } else if self.eligible {
            Some(self.last_report_sec + FLOW_REPORT_ONLY_PERIOD_SEC)
        } else {
            None
        }
    }

    /// Whether a report rides on an outgoing sequenced packet at `now_sec` (§9.3.12: "piggyback on
    /// an ordinary outgoing application carrier only when eligible AND due"): after the first
    /// report, with eligible traffic unreported, and one second after the last report sent.
    fn attach_due(&self, now_sec: f64) -> bool {
        self.initial_sent
            && self.eligible
            && now_sec >= self.last_report_sec + FLOW_REPORT_ONLY_PERIOD_SEC
    }

    /// A FlowControl-only packet went out at `now_sec`.
    fn note_flow_only(&mut self, now_sec: f64) {
        self.note_report(now_sec);
        self.initial_sent = true;
    }

    /// A report, standalone or attached, was sent at `now_sec`: the counters and eligibility
    /// start again and the one-second cap restarts (§9.3.12).
    fn note_report(&mut self, now_sec: f64) {
        self.bytes = 0;
        self.since_sec = now_sec;
        self.eligible = false;
        self.last_report_sec = now_sec;
    }

    /// Counts one accepted peer datagram of `len` bytes; `eligible` when it makes a report due.
    ///
    /// §9.3.12 closed eligibility list: a datagram with at least one accepted ordinary fragment
    /// that is not an omitted-chunk control, or one with a `ClockSyncHeader`. Every other accepted
    /// datagram, pure Ack/NAK/OutOfOrderAck/flow-only traffic included, still adds its bytes. The
    /// counter wraps modulo 2³² (§9.3.12).
    fn count(&mut self, len: usize, eligible: bool) {
        // A datagram is at most a few kilobytes; the truncation is the modulo-2³² rule itself.
        self.bytes = self.bytes.wrapping_add(len as u32);
        self.eligible |= eligible;
    }

    /// The report for `now_sec`: the bytes counted and the elapsed milliseconds (floor,
    /// modulo 65,536, §5.5 and §9.3.12; it never saturates).
    fn header(&self, now_sec: f64) -> FlowControlHeader {
        let elapsed_ms = ((now_sec - self.since_sec).max(0.0) * 1000.0).floor();
        FlowControlHeader {
            bytes_received_delta: self.bytes,
            interval_ms: ((elapsed_ms as u64) & 0xFFFF) as u16,
        }
    }
}

/// The ordering and blob-ID counters of one connection and direction (§5.6.2 P10-23-POLICY,
/// interim and unverified on the official service, `OPEN-SESSION-19`): nothing is shared
/// between queues or between queue-3 sequence groups. Fresh on every connection.
#[derive(Debug, Clone)]
struct OrderCounters {
    /// The next ordering number of queues `2` and `4`.
    order_2: u16,
    order_4: u16,
    /// The next ordering number of each queue-3 sequence group used so far, at most
    /// [`MAX_ORDER_GROUPS`]; a group not here starts at `1`.
    order_groups: BTreeMap<u16, u16>,
    /// The next blob ID of queues `1` (fragmented blobs only), `2` and `4`.
    blob_1: u16,
    blob_2: u16,
    blob_4: u16,
}

/// A blob's allocated `orderNum` and `blobId`, committed only once its fragments are built.
#[derive(Debug, Clone, Copy)]
struct OrderAllocation {
    order_num: u16,
    blob_id: u16,
}

impl OrderCounters {
    /// Every counter starts at `1` (§5.6.2).
    fn new() -> Self {
        Self {
            order_2: 1,
            order_4: 1,
            order_groups: BTreeMap::new(),
            blob_1: 1,
            blob_2: 1,
            blob_4: 1,
        }
    }

    /// The values the next logical blob of `queue_id` takes (nothing changes yet). Queue `1`
    /// carries no ordering word on the wire (`0xFFFF`), an unfragmented queue-1 blob has the
    /// implicit blob ID `0`, and queue `3` never has one (§5.6.3 field 4).
    fn peek(
        &self,
        queue_id: u8,
        seq_group: u16,
        fragmented: bool,
    ) -> Result<OrderAllocation, SessionError> {
        let order_num = match queue_id {
            2 => self.order_2,
            3 => match self.order_groups.get(&seq_group) {
                Some(&next) => next,
                None if self.order_groups.len() >= MAX_ORDER_GROUPS => {
                    return Err(SessionError::TooManyOrderGroups {
                        limit: MAX_ORDER_GROUPS,
                    });
                }
                None => 1,
            },
            4 => self.order_4,
            _ => 0xFFFF,
        };
        let blob_id = if has_blob_id(queue_id, fragmented) {
            match queue_id {
                1 => self.blob_1,
                2 => self.blob_2,
                _ => self.blob_4,
            }
        } else {
            0
        };
        Ok(OrderAllocation { order_num, blob_id })
    }

    /// Commits [`Self::peek`]'s values: an ordering number advances modulo 65,536 and may be
    /// `0`; a blob ID wraps 65,535 to `1` and skips `0`; an unfragmented queue-1 blob
    /// advances nothing (§5.6.2).
    fn commit(&mut self, queue_id: u8, seq_group: u16, fragmented: bool) {
        match queue_id {
            2 => self.order_2 = self.order_2.wrapping_add(1),
            3 => {
                let next = self.order_groups.entry(seq_group).or_insert(1);
                *next = next.wrapping_add(1);
            }
            4 => self.order_4 = self.order_4.wrapping_add(1),
            _ => {}
        }
        if has_blob_id(queue_id, fragmented) {
            let id = match queue_id {
                1 => &mut self.blob_1,
                2 => &mut self.blob_2,
                _ => &mut self.blob_4,
            };
            *id = if *id == u16::MAX { 1 } else { *id + 1 };
        }
    }
}

/// Per-connection state machine, sequence window, ISAAC generators, and fragmented blob reassembler (§3.4, §5–§8).
pub struct SessionConnection {
    local_rec_id: u16,
    server_rec_id: u16,
    max_recv_rate: u32,
    remote_addr: SocketAddrV4,
    state: ConnectionState,
    state_entered_at_sec: f64,
    last_recv_time_sec: f64,
    last_ack_nak_sent_sec: f64,
    last_clock_sync_sent_sec: f64,
    last_handshake_retransmit_sec: f64,
    flow: FlowReport,
    extended_silence_timeout: bool,
    server_clock_offset_sec: f64,
    adopted_challenge: Option<(u16, ConnectChallengeHeader)>,
    client_word: u16,
    peer_word: u16,

    send_seq_num: u32,
    recv_seq_expected: u32,
    outgoing_isaac: Option<IsaacCipher>,
    incoming_isaac: Option<IsaacCipher>,
    missing_gaps: BTreeMap<u32, SequenceGap>,
    retransmit_table: BTreeMap<u32, SentPacketRecord>,
    out_of_order_ack_queue: VecDeque<u32>,
    dropped_out_of_order_acks: u64,
    codec_uuids: Option<CodecUuids>,
    codecs_negotiated: bool,

    stored_session_auth: Option<SessionAuthHeader>,
    stored_referral_auth: Option<ReferralAuthHeader>,
    stored_connect_accept: Option<ConnectAcceptHeader>,

    order_counters: OrderCounters,
    reassembler: BlobReassembler,
    /// [`Self::retire`] was called.
    retiring: bool,
    outbound_datagrams: VecDeque<Vec<u8>>,
    events: VecDeque<SessionEvent>,

    counts: HandshakeCounts,
    /// The evidence log of an official run ([`Evidence`]); `None` for everything else.
    evidence: Option<Box<Evidence>>,
    closed_from: Option<ConnectionState>,
    /// The blob [`Self::send_blob_watched`] sent, for the evidence of what became of it.
    watched: Option<WatchedBlob>,
}

impl std::fmt::Debug for SessionConnection {
    // No datagram bytes, credentials, cookies or ISAAC state: queued datagrams may carry the
    // ticket (`official-login.md` §6.3). No peer address either: a `Debug` form can reach a
    // log line, which never names one (decision 0044 item 3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConnection")
            .field("local_rec_id", &self.local_rec_id)
            .field("server_rec_id", &self.server_rec_id)
            .field("state", &self.state)
            .field("send_seq_num", &self.send_seq_num)
            .field("recv_seq_expected", &self.recv_seq_expected)
            .field("missing_gaps", &self.missing_gaps.len())
            .field("unacked_sent", &self.retransmit_table.len())
            .field("outbound_datagrams", &self.outbound_datagrams.len())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl Drop for SessionConnection {
    // Queued datagrams may carry a SessionAuthHeader with the ticket: wipe every one still
    // queued (`official-login.md` §6.3). The stored header wipes its own credentials.
    fn drop(&mut self) {
        for dgram in &mut self.outbound_datagrams {
            crate::login::zeroize_vec(dgram);
        }
    }
}

impl SessionConnection {
    /// Creates a new `SessionConnection` to `remote_addr` in State `0` (`Idle`) with
    /// `localRecId = 0x0000`, `remoteRecId = 0x0000` and `sendSeqNum = 1` (pre-incremented,
    /// so the first sequenced packet carries `2`): both IDs come from a
    /// `ConnectChallengeHeader` later (§3.4, §5.1, §6.1, §6.2). Its `maxRecvRate` is `0`
    /// until [`Self::set_max_recv_rate`] (§6.4), and it has no dictionary UUIDs until
    /// [`Self::set_codec_uuids`] (§7.3).
    #[must_use]
    pub fn new(remote_addr: SocketAddrV4, now_sec: f64) -> Self {
        Self {
            local_rec_id: 0,
            server_rec_id: 0,
            max_recv_rate: 0,
            remote_addr,
            state: ConnectionState::Idle,
            state_entered_at_sec: now_sec,
            last_recv_time_sec: now_sec,
            last_ack_nak_sent_sec: now_sec,
            last_clock_sync_sent_sec: now_sec,
            last_handshake_retransmit_sec: now_sec,
            flow: FlowReport::new(now_sec),
            extended_silence_timeout: false,
            server_clock_offset_sec: 0.0,
            adopted_challenge: None,
            client_word: 0,
            peer_word: 0,

            send_seq_num: 1,
            recv_seq_expected: 1,
            outgoing_isaac: None,
            incoming_isaac: None,
            missing_gaps: BTreeMap::new(),
            retransmit_table: BTreeMap::new(),
            out_of_order_ack_queue: VecDeque::new(),
            dropped_out_of_order_acks: 0,
            codec_uuids: None,
            codecs_negotiated: false,

            stored_session_auth: None,
            stored_referral_auth: None,
            stored_connect_accept: None,

            order_counters: OrderCounters::new(),
            reassembler: BlobReassembler::default(),
            retiring: false,
            outbound_datagrams: VecDeque::new(),
            events: VecDeque::new(),

            counts: HandshakeCounts::default(),
            evidence: None,
            closed_from: None,
            watched: None,
        }
    }

    /// Turns on this connection's evidence log under the neutral `role` ([`EVIDENCE_TARGET`]). Only
    /// the connector's official runs do.
    pub fn enable_evidence(&mut self, role: &'static str, now_sec: f64) {
        self.evidence = Some(Box::new(Evidence::new(role, now_sec)));
    }

    /// The stage the next evidence lines belong to, whose clock starts now.
    pub fn evidence_stage(&mut self, stage: &'static str, now_sec: f64) {
        if let Some(e) = self.evidence.as_mut() {
            e.stage = stage;
            e.stage_since = now_sec;
        }
    }

    /// Sets the `maxRecvRate` this connection sends in its `ConnectAcceptHeader` (§6.4): `0`
    /// or [`max_recv_rate_for_speed`] on the primary connection, [`CHAT_MAX_RECV_RATE`] on
    /// the chat connection.
    pub fn set_max_recv_rate(&mut self, rate: u32) {
        self.max_recv_rate = rate;
    }

    /// The `maxRecvRate` of this connection's `ConnectAcceptHeader` (§6.4).
    #[must_use]
    pub fn max_recv_rate(&self) -> u32 {
        self.max_recv_rate
    }

    /// What this connection counted of its handshake and its datagrams.
    #[must_use]
    pub fn handshake_counts(&self) -> HandshakeCounts {
        self.counts
    }

    /// The state the connection was in when it closed (State `8`), `None` while it has not
    /// closed.
    #[must_use]
    pub fn closed_from(&self) -> Option<ConnectionState> {
        self.closed_from
    }

    /// Returns the current connection state (`0..8`, §6.1).
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Whether the connection has been in State `8` (`Closing`) for `20.0` s and may be
    /// removed (§6.1).
    #[must_use]
    pub fn closed_for_cleanup(&self, now_sec: f64) -> bool {
        self.state == ConnectionState::Closing
            && now_sec - self.state_entered_at_sec >= CLOSING_CLEANUP_SEC
    }

    /// Returns the local (incoming) `recId`: `0x0000` until a `ConnectChallengeHeader`'s
    /// datagram header gives a non-zero one (§3.4, §6.3).
    #[must_use]
    pub fn local_rec_id(&self) -> u16 {
        self.local_rec_id
    }

    /// Returns the outgoing `recId` (`0` prior to `ConnectChallengeHeader`, then its payload
    /// bytes `18..19`, §6.3).
    #[must_use]
    pub fn server_rec_id(&self) -> u16 {
        self.server_rec_id
    }

    /// Returns the remote peer `(IPv4, port)` socket address.
    #[must_use]
    pub fn remote_addr(&self) -> SocketAddrV4 {
        self.remote_addr
    }

    /// Returns the current `recvSeqExpected` contiguous sequence watermark (§5.1).
    #[must_use]
    pub fn recv_seq_expected(&self) -> u32 {
        self.recv_seq_expected
    }

    /// Returns the current `sendSeqNum` counter (§5.1).
    #[must_use]
    pub fn send_seq_num(&self) -> u32 {
        self.send_seq_num
    }

    /// Returns the number of missing sequence numbers currently tracked in the gap tree (§5.1, §5.3).
    #[must_use]
    pub fn missing_gap_count(&self) -> usize {
        self.missing_gaps.len()
    }

    /// Returns how many out-of-order acknowledgement requests were dropped
    /// because [`MAX_PENDING_OUT_OF_ORDER_ACKS`] were already waiting.
    #[must_use]
    pub fn dropped_out_of_order_acks(&self) -> u64 {
        self.dropped_out_of_order_acks
    }

    /// Returns the number of out-of-order acknowledgements waiting to be sent.
    #[must_use]
    pub fn pending_out_of_order_acks(&self) -> usize {
        self.out_of_order_ack_queue.len()
    }

    /// Returns the number of unacknowledged sent packets in the retransmit table (§5.1).
    #[must_use]
    pub fn unacked_sent_count(&self) -> usize {
        self.retransmit_table.len()
    }

    /// Whether the challenge named the install's incoming dictionary, so the directional
    /// codecs are in use (§7.3).
    #[must_use]
    pub fn codecs_negotiated(&self) -> bool {
        self.codecs_negotiated
    }

    /// Enables or disables the `300.0`-second extended silence timeout in State `5` (§8.2).
    pub fn set_extended_silence_timeout(&mut self, enabled: bool) {
        self.extended_silence_timeout = enabled;
    }

    /// Sets the install's directional dictionary UUIDs (§7.3). A connection without them
    /// answers every `ConnectChallengeHeader` with the unsupported-negotiation result
    /// ([`DisconnectReason::UnsupportedCodec`]); the chat connection has none, its
    /// dictionary source being `OPEN-SESSION-1` (§6.3).
    pub fn set_codec_uuids(&mut self, uuids: CodecUuids) {
        self.codec_uuids = Some(uuids);
    }

    /// Pops the next queued outbound UDP datagram ready to transmit to `remote_addr`.
    ///
    /// The caller owns the returned bytes: a `SessionAuthHeader` datagram carries the
    /// ticket, so wipe it once it is sent (`official-login.md` §6.3);
    /// [`SessionMultiplexer::flush_outbound`] does. Datagrams still queued are wiped when
    /// the connection is dropped.
    pub fn pop_outbound_datagram(&mut self) -> Option<Vec<u8>> {
        self.outbound_datagrams.pop_front()
    }

    /// Pops the next emitted [`SessionEvent`].
    pub fn pop_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    /// The `DatagramHeader` of an outgoing packet with `seq_num` (§4.2): `recId` is the
    /// outgoing recipient (`0x0000` before the challenge); bytes `16..19` are the client
    /// word and the copied peer word (both `0` before the challenge, §4.2, §6.4).
    fn base_datagram_header(&self, seq_num: u32) -> DatagramHeader {
        DatagramHeader {
            rec_id: self.server_rec_id,
            pak_size: 0,
            flags: 0,
            seq_num,
            checksum: 0,
            client_word: self.client_word,
            peer_word: self.peer_word,
        }
    }

    /// Encodes one of this connection's own datagrams. Tessera never sends the optional
    /// fields of bits 25 and 26 (§4.3).
    fn encode_own(
        header: DatagramHeader,
        opt: &OptionalHeaders,
        fragments: &[BlobFragment],
        xor_mask: Option<u32>,
        codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u8>, SessionError> {
        if (opt.flags_mask() & OBSERVED_ONLY_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags {
                flags: opt.flags_mask(),
            });
        }
        encode_datagram(header, opt, fragments, xor_mask, codec)
    }

    /// Drops the stored `SessionAuthHeader` (its credentials are wiped on drop) and
    /// `ReferralAuthHeader` once State `1` is left (`official-login.md` §6.3).
    fn forget_auth(&mut self) {
        self.stored_session_auth = None;
        self.stored_referral_auth = None;
    }

    /// Records `reason`, moves to State `8` (`Closing`) and emits [`SessionEvent::Disconnected`].
    fn close(&mut self, reason: DisconnectReason, now_sec: f64) {
        if self.state != ConnectionState::Closing {
            self.closed_from = Some(self.state);
        }
        self.forget_auth();
        self.state = ConnectionState::Closing;
        self.state_entered_at_sec = now_sec;
        self.events.push_back(SessionEvent::Disconnected { reason });
    }

    /// Whether the directional codecs were negotiated: the `ConnectChallengeHeader` named
    /// the install's incoming dictionary (§7.3).
    fn codec_negotiated(&self) -> bool {
        self.codecs_negotiated
    }

    /// The wire codec for a normal sequenced data packet: only on a connection that
    /// negotiated the directional codecs and once the outgoing recipient is known (§4.1.1,
    /// §4.1.2, §7.3); otherwise the packet goes out with `wirePrefix` `0x0000`. Control packets
    /// always go out with `wirePrefix` `0x0000` (§4.1.1).
    fn data_codec<'c>(
        &self,
        wire_codec: Option<&'c dyn BitTreeCodec>,
    ) -> Option<&'c dyn BitTreeCodec> {
        if self.server_rec_id == 0 || !self.codec_negotiated() {
            None
        } else {
            wire_codec
        }
    }

    /// The wire codec for received datagrams: none on a connection without a negotiated
    /// `codecUuid` (§4.1.1, §7.3), so a non-zero `wirePrefix` is then refused
    /// ([`SessionError::MissingWireCodec`]).
    fn receive_codec<'c>(
        &self,
        wire_codec: Option<&'c dyn BitTreeCodec>,
    ) -> Option<&'c dyn BitTreeCodec> {
        if self.codec_negotiated() {
            wire_codec
        } else {
            None
        }
    }

    /// Initiates a session by transitioning from State `0` to State `1` and queueing an
    /// unsequenced `SessionAuthHeader` (`0x00010000`) datagram (§6.1, §6.2): `wirePrefix`
    /// `0x0000`, `recId` `0x0000`, `seqNum` `0` (not the sequenced-packet counter, §5.1),
    /// the final four header bytes `0`. Build the header with
    /// [`SessionAuthHeader::for_connection`].
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the connection is not in `Idle` state or encoding fails.
    pub fn start_login(
        &mut self,
        session_auth: SessionAuthHeader,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Idle {
            return Err(SessionError::InvalidState { state: self.state });
        }
        self.stored_session_auth = Some(session_auth.clone());
        self.state = ConnectionState::SessionAuthSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;
        // §5.5: the flow counters start at the initial auth transmission.
        self.flow = FlowReport::new(now_sec);

        // §5.1, §6.2: sequence zero, independent of the next sequenced-packet counter.
        let hdr = self.base_datagram_header(0);
        let opt = OptionalHeaders {
            session_auth: Some(session_auth),
            ..OptionalHeaders::default()
        };
        let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        self.counts.session_auth = self.counts.session_auth.saturating_add(1);
        self.note_handshake(SendKind::Auth, now_sec);
        Ok(())
    }

    /// Initiates a world-server session by transitioning from State `0` to State `1` and
    /// queueing an unsequenced `ReferralAuthHeader` (`0x00020000`) datagram (§6.1, §6.5):
    /// `wirePrefix` `0x0000`, `recId` `0x0000`, `seqNum` `0`, the final four header bytes
    /// `0`, and as payload only the eight bytes of the referral's `cookie` reversed
    /// (§10.2 Vector 8). Call it on a fresh connection: nothing of the login connection
    /// (recipients, masks, counters) is carried over.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the connection is not in `Idle` state or encoding fails.
    pub fn start_world_login(&mut self, cookie: [u8; 8], now_sec: f64) -> Result<(), SessionError> {
        if self.state != ConnectionState::Idle {
            return Err(SessionError::InvalidState { state: self.state });
        }
        let wlr = ReferralAuthHeader::for_cookie(cookie);
        self.stored_referral_auth = Some(wlr);
        self.state = ConnectionState::SessionAuthSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;
        // §5.5: the flow counters start at the initial auth transmission.
        self.flow = FlowReport::new(now_sec);

        // §4.2, §6.5: sequence zero.
        let hdr = self.base_datagram_header(0);
        let opt = OptionalHeaders {
            referral_auth: Some(wlr),
            ..OptionalHeaders::default()
        };
        let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        self.counts.referral_auth = self.counts.referral_auth.saturating_add(1);
        self.note_handshake(SendKind::Auth, now_sec);
        Ok(())
    }

    /// Sends a sequenced (`MaskedChecksumFlag` `0x00000002`) packet carrying `opt_headers`
    /// and `fragments`, recording it in the outbound retransmission table with its ISAAC
    /// `xor_mask` and client word (§4.2, §5.1, §7.2). The packet allocates the next sequence
    /// number (the first is `2`) and draws its outgoing mask; a `ClockSyncHeader` carried
    /// here uses that number and mask, not a second word (§6.4.1). The packet is compressed
    /// with `outgoing_codec` (the client-to-server dictionary) once the codecs are
    /// negotiated (§4.1.2, §7.3).
    ///
    /// The interim flow-report policy of §5.5 and §9.3.12 (P10-10L) applies: in State `5` the
    /// first sequenced packet is always the FlowControl-only one (it is sent first when it
    /// has not gone out yet), and a report is attached to the packet when `opt_headers` carries
    /// none, eligible traffic arrived since the last report (pure acknowledgements alone
    /// make none due) and one second has passed since the last report sent. A `ClockSyncHeader` is attached once
    /// `30.0` s have passed since State `5` or the last one (§8.1): Tessera sends ClockSync
    /// only on sequenced packets (`OPEN-SESSION-9`), so its timer is no deadline.
    ///
    /// Nothing is allocated (sequence number, ISAAC word, client word) unless the datagram
    /// encodes.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidState`] before a `ConnectChallengeHeader` has seeded the
    /// outgoing ISAAC generator (§7.2), or [`SessionError::OutboundWindowOverflow`] if
    /// unacknowledged packets exceed `129,988` (`0x1FBC4`) or the sequence span exceeds
    /// `0x7FFFFFFF` (§5.1).
    pub fn send_sequenced_packet(
        &mut self,
        opt_headers: OptionalHeaders,
        fragments: Vec<BlobFragment>,
        retransmit_eligible: bool,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        if self.outgoing_isaac.is_none() {
            return Err(SessionError::InvalidState { state: self.state });
        }
        if self.state == ConnectionState::Connected && !self.flow.initial_sent {
            self.send_flow_only(now_sec, outgoing_codec)?;
        }
        self.allocate_sequenced(
            opt_headers,
            fragments,
            retransmit_eligible,
            now_sec,
            outgoing_codec,
        )
    }

    /// Sends one FlowControl-only sequenced packet with the counters of §5.5: the first one after
    /// State `5` (§6.4.1) and later ones when eligible traffic found no other packet to ride on
    /// (§9.3.12 P10-10L).
    fn send_flow_only(
        &mut self,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        let opt = OptionalHeaders {
            flow_control: Some(self.flow.header(now_sec)),
            ..OptionalHeaders::default()
        };
        // A FlowControl-only packet has nothing to retransmit: a negative acknowledgement
        // of it is answered with OutOfOrderAckHeader (§5.3).
        let flow_event = if self.flow.initial_sent {
            "report-unreported"
        } else {
            "report-first"
        };
        let seq = self.allocate_sequenced_with(
            opt,
            Vec::new(),
            false,
            flow_event,
            now_sec,
            outgoing_codec,
        )?;
        self.flow.note_flow_only(now_sec);
        Ok(seq)
    }

    /// [`Self::allocate_sequenced_with`] with the pending flow report and the due ClockSync
    /// attached.
    fn allocate_sequenced(
        &mut self,
        mut opt_headers: OptionalHeaders,
        fragments: Vec<BlobFragment>,
        retransmit_eligible: bool,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        let attach_flow = self.flow.attach_due(now_sec) && opt_headers.flow_control.is_none();
        if attach_flow {
            opt_headers.flow_control = Some(self.flow.header(now_sec));
        }
        // §8.1: the 30.0 s ClockSync timer runs from State 5; a due header rides on this
        // packet (no standalone ClockSync, §5.1, §6.1 State 5, OPEN-SESSION-9).
        let attach_clock = self.state == ConnectionState::Connected
            && now_sec - self.last_clock_sync_sent_sec >= CLOCK_SYNC_INTERVAL_SEC
            && opt_headers.clock_sync.is_none();
        if attach_clock {
            opt_headers.clock_sync = Some(now_sec + self.server_clock_offset_sec);
        }
        let flow_event = if attach_flow {
            "report-attached"
        } else {
            "none"
        };
        let seq = self.allocate_sequenced_with(
            opt_headers,
            fragments,
            retransmit_eligible,
            flow_event,
            now_sec,
            outgoing_codec,
        )?;
        if attach_flow {
            self.flow.note_report(now_sec);
        }
        if attach_clock {
            self.last_clock_sync_sent_sec = now_sec;
        }
        Ok(seq)
    }

    /// Allocates the next sequence number, ISAAC word and client word for a packet of
    /// `opt_headers` and `fragments` and queues it, committing nothing when it fails to
    /// encode.
    fn allocate_sequenced_with(
        &mut self,
        opt_headers: OptionalHeaders,
        fragments: Vec<BlobFragment>,
        retransmit_eligible: bool,
        flow_event: &'static str,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        // §7.2: a sequenced packet's checksum needs the outgoing ISAAC word.
        let Some(outgoing) = self.outgoing_isaac.as_ref() else {
            return Err(SessionError::InvalidState { state: self.state });
        };
        // §4.3: Tessera never sends bits 25 and 26; refused before a number is used up.
        if (opt_headers.flags_mask() & OBSERVED_ONLY_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags {
                flags: opt_headers.flags_mask(),
            });
        }
        if self.retransmit_table.len() >= MAX_UNACKED_SENT_PACKETS {
            self.close(DisconnectReason::OutboundWindowOverflow, now_sec);
            self.finish_evidence_at_close();
            return Err(SessionError::OutboundWindowOverflow);
        }
        let next_seq = {
            let s = self.send_seq_num.wrapping_add(1);
            if s == 0 { 2 } else { s }
        };
        if let Some((&oldest_seq, _)) = self.retransmit_table.first_key_value()
            && next_seq.wrapping_sub(oldest_seq) > MAX_OUTBOUND_SEQ_SPAN
        {
            self.close(DisconnectReason::OutboundWindowOverflow, now_sec);
            self.finish_evidence_at_close();
            return Err(SessionError::OutboundWindowOverflow);
        }

        // The mask comes from a copy of the generator and the word from a local: both are
        // committed only once the datagram encoded (§7.2 item 4, §4.2 P10-22-POLICY).
        let mut generator = outgoing.clone();
        let xor_mask = generator.next_mask();
        let client_word = self.client_word.wrapping_add(1);
        let mut hdr = self.base_datagram_header(next_seq);
        hdr.client_word = client_word;
        let codec = self.data_codec(outgoing_codec);
        let dgram = Self::encode_own(hdr, &opt_headers, &fragments, Some(xor_mask), codec)?;

        let mut line = SendEvidence::of(SendKind::New, next_seq, &opt_headers, &fragments).word(
            self.client_word,
            client_word,
            WordInput::Sequenced,
        );
        line.flow_event = flow_event;
        self.note_send(line, now_sec);
        self.outgoing_isaac = Some(generator);
        self.send_seq_num = next_seq;
        self.client_word = client_word;
        self.retransmit_table.insert(
            next_seq,
            SentPacketRecord {
                seq_num: next_seq,
                xor_mask,
                client_word,
                opt_headers,
                fragments,
                retransmit_eligible,
                retained: false,
                kept_until_acked: false,
            },
        );
        self.outbound_datagrams.push_back(dgram);
        Ok(next_seq)
    }

    /// Splits a `FragmentedBlob` into `<= 436`-byte fragments and queues sequenced packets on
    /// the connection (§5.6.4). `payload` is the `MessageEnvelope` (§9.1); the blob goes to
    /// the default recipient (`0x0000000000000002`, §5.6.3 field 7) and is never encrypted
    /// (§7.1). See [`Self::send_blob_to`] for an explicit recipient.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    pub fn send_blob(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        self.send_blob_to(
            queue_id,
            seq_group,
            DEFAULT_BLOB_RECIPIENT_ID,
            blob_header_word,
            payload,
            now_sec,
            outgoing_codec,
        )
    }

    /// [`Self::send_blob`] addressed to `recipient_id` (§5.6.3 field 7, §9.1 item 2): the
    /// default recipient `2` sets `DefaultRecipient` (`0x40`), any other value is written
    /// explicitly.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn send_blob_to(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        recipient_id: u64,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        self.send_blob_inner(
            queue_id,
            seq_group,
            recipient_id,
            blob_header_word,
            payload,
            now_sec,
            outgoing_codec,
            false,
        )
    }

    /// [`Self::send_blob_to`] whose packets stay resendable on a `SequenceNakHeader` even on an
    /// unreliable queue, with their saved sequence number, mask, client word and data (§4.2,
    /// §5.3), until they are acknowledged or the connection ends. Once one is gone a request
    /// for it is answered with `OutOfOrderAckHeader` as for any expired fragment (§5.4).
    ///
    /// For the initial `0xEC` and the client's `0xF5` (§9.3.7): they stay for the entire active
    /// negotiation, not for a fixed time, and they always carry the explicit recipient field.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn send_blob_retained(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        recipient_id: u64,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        self.send_blob_inner(
            queue_id,
            seq_group,
            recipient_id,
            blob_header_word,
            payload,
            now_sec,
            outgoing_codec,
            true,
        )
    }

    /// [`Self::send_blob`], in its default-recipient form, whose packets stay resendable on a
    /// `SequenceNakHeader` like [`Self::send_blob_retained`]'s.
    ///
    /// For `0xF4`, whose current form has flag `0x40` (§9.3.9, Vector 18). §9.3.9 "F4 retention" and
    /// the §5.3 F4 exception: the record stays until an explicit cumulative Ack covers it or the
    /// connection closes; the implicit acknowledgement below a NAK's first number never frees it.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    pub fn send_blob_kept(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        let seqs = self.send_blob(
            queue_id,
            seq_group,
            blob_header_word,
            payload,
            now_sec,
            outgoing_codec,
        )?;
        for seq in &seqs {
            if let Some(rec) = self.retransmit_table.get_mut(seq) {
                rec.retained = true;
                rec.kept_until_acked = true;
            }
        }
        Ok(seqs)
    }

    /// [`Self::send_blob_retained`] to `recipient_id` whose records are kept like the `0xF4`'s
    /// ([`Self::send_blob_kept`]) and watched for the evidence log ([`Self::watched_fate`]).
    ///
    /// For decision 0046 item 13's `0x13D` (`official-session.md` §9.3.12). Labelled choice (C8,
    /// pending writer answer to remediation §31 fix 2): before the referral its records stay until
    /// a cumulative Ack covers them or the connection closes; a `SequenceNakHeader` naming one
    /// resends its saved bytes with the same sequence number, word and checksum, as for the
    /// retained `0xEC` and `0xF5`; like the `0xF4` it is exempt from the implicit acknowledgement
    /// below a NAK's first number; and nothing resends it on a timer. At the referral
    /// [`Self::retire`] drops it like every other application record. One blob is watched per
    /// connection: a second call watches the newer one.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn send_blob_watched(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        recipient_id: u64,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        let seqs = self.send_blob_retained(
            queue_id,
            seq_group,
            recipient_id,
            blob_header_word,
            payload,
            now_sec,
            outgoing_codec,
        )?;
        for seq in &seqs {
            if let Some(rec) = self.retransmit_table.get_mut(seq) {
                rec.kept_until_acked = true;
            }
        }
        let kept: Vec<u32> = seqs.iter().copied().take(MAX_WATCHED_SEQUENCES).collect();
        self.watched = kept.first().copied().map(|first| WatchedBlob {
            fate: KeptBlobFate {
                first_sequence: first,
                datagrams: u32::try_from(seqs.len()).unwrap_or(u32::MAX),
                ..KeptBlobFate::default()
            },
            seqs: kept,
        });
        Ok(seqs)
    }

    /// What became of the blob [`Self::send_blob_watched`] sent, `None` when none was.
    #[must_use]
    pub fn watched_fate(&self) -> Option<KeptBlobFate> {
        self.watched.as_ref().map(|w| w.fate)
    }

    #[allow(clippy::too_many_arguments)]
    fn send_blob_inner(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        recipient_id: u64,
        blob_header_word: u32,
        payload: &[u8],
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
        retained: bool,
    ) -> Result<Vec<u32>, SessionError> {
        if !(1..=4).contains(&queue_id) {
            return Err(SessionError::InvalidQueueId { queue_id });
        }
        // §7.2: check before any order number or blob ID is used up.
        if self.outgoing_isaac.is_none() {
            return Err(SessionError::InvalidState { state: self.state });
        }
        // §5.6.1: the opcode must fit the compact integer, checked before anything is
        // allocated (a payload too large is refused by `split_blob` before that too).
        if blob_header_word > MAX_VARIABLE_U32 {
            return Err(SessionError::VariableU32OutOfRange {
                value: blob_header_word,
            });
        }
        if payload.len() > MAX_BLOB_BYTES {
            return Err(SessionError::BlobTooLarge {
                total_bytes: payload.len(),
                limit: MAX_BLOB_BYTES,
            });
        }
        let is_multi = payload.len() > MAX_BLOB_FRAG_PAYLOAD;
        // §5.6.2 P10-23-POLICY: allocated once per logical blob, retained on every fragment
        // and retransmission, and committed only once the fragments are built.
        let allocated = self.order_counters.peek(queue_id, seq_group, is_multi)?;
        let frags = split_blob(
            queue_id,
            allocated.order_num,
            allocated.blob_id,
            seq_group,
            Some(recipient_id),
            blob_header_word,
            payload,
        )?;
        self.order_counters.commit(queue_id, seq_group, is_multi);

        let retransmit_eligible = queue_id == 2 || queue_id == 4;
        let mut seqs = Vec::with_capacity(frags.len());
        for mut frag in frags {
            // §9.3.7 (P10-10F): `0xEC` and the client's `0xF5` always carry the eight-byte
            // recipient field, also for the default recipient `2`.
            if retained {
                frag.header.blob_flags &= !BLOB_FLAG_DEFAULT_RECIPIENT;
            }
            let seq = self.send_sequenced_packet(
                OptionalHeaders::default(),
                vec![frag],
                retransmit_eligible,
                now_sec,
                outgoing_codec,
            )?;
            if let Some(rec) = self.retransmit_table.get_mut(&seq) {
                rec.retained = retained;
            }
            seqs.push(seq);
        }
        Ok(seqs)
    }

    /// Marks an outbound sequenced packet's unreliable fragments as expired so a future `SequenceNakHeader`
    /// for `seq_num` emits `OutOfOrderAckHeader` (`0x00002000`) instead of retransmitting (§5.3, §5.4).
    pub fn expire_unreliable_sent_packet(&mut self, seq_num: u32) {
        if let Some(rec) = self.retransmit_table.get_mut(&seq_num) {
            rec.retransmit_eligible = false;
            rec.retained = false;
        }
    }

    /// Drops the datagrams queued and not yet handed to the socket and everything it could
    /// resend. A cancelled login uses this so that nothing it queued in its last pass goes out
    /// before its goodbye, also on a later `SequenceNakHeader`; the sequence state stays as it
    /// is, and the connection is on its way out.
    pub fn discard_unsent_datagrams(&mut self) {
        if let Some(e) = self.evidence.as_mut() {
            e.queued_not_sent += self.outbound_datagrams.len() as u64;
        }
        self.outbound_datagrams.clear();
        self.retransmit_table.clear();
    }

    /// Queues one standalone `DisconnectHeader` at once, with no flush wait (§9.3.7, "Unsequenced
    /// controls"). The caller then closes the connection locally. Nothing is sent before State 5.
    /// What was queued and not sent yet is discarded first ([`Self::discard_unsent_datagrams`]): a
    /// local terminal close sends at most that one Disconnect (§9.3.8), and no application data
    /// follows the decision.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the packet cannot be encoded.
    pub fn queue_standalone_disconnect(&mut self, now_sec: f64) -> Result<(), SessionError> {
        if self.state != ConnectionState::Connected {
            return Ok(());
        }
        self.discard_unsent_datagrams();
        self.queue_control(OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        })?;
        self.note_control(SendKind::Disconnect, now_sec);
        Ok(())
    }

    /// This login connection was referred away (§9.3.9 "Login connection at valid referral",
    /// §9.3.12 "Login transport lifetime"): from now on no outcome of its own sends a Disconnect,
    /// while its transport-only Ack, NAK, clock and flow handling goes on, under its normal timers,
    /// for as long as the world connection lives. No application or credential send is permitted
    /// on it.
    ///
    /// Labelled (question for the writer): every outbound record that carries application data (the
    /// retained `0xEC` and `0xF5`, a `0x13D` or any other blob) is dropped here, so a later login NAK
    /// for one is answered with an OutOfOrderAck instead of an application resend. Packets that carry
    /// only transport headers keep their generic §5.3 handling.
    pub fn retire(&mut self) {
        self.retiring = true;
        self.retransmit_table
            .retain(|_, rec| rec.fragments.is_empty());
    }

    /// The one Disconnect of a close this connection decides itself, unless it is retiring.
    fn closing_disconnect(&mut self, now_sec: f64) {
        if !self.retiring && self.queue_standalone_disconnect(now_sec).is_err() {
            tracing::debug!("official session: the closing Disconnect was not encoded");
        }
    }

    /// What the reassembler ignored, discarded or found odd (§9.3.9).
    #[must_use]
    pub const fn reassembly_counts(&self) -> ReassemblyCounts {
        self.reassembler.counts()
    }

    /// How many incomplete queue-3 blobs a newer completed one superseded (§9.3.8).
    #[must_use]
    pub fn superseded_blobs(&self) -> u64 {
        self.reassembler.superseded()
    }

    /// Initiates a local graceful disconnect (`State 5 -> State 6`, §6.1, §8.2).
    pub fn request_disconnect(&mut self, now_sec: f64) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::DisconnectingFlush;
            self.state_entered_at_sec = now_sec;
        }
    }

    /// Processes a single incoming UDP datagram from `source_addr` at `now_sec` (§4–§8, §10.1).
    ///
    /// `codecs.incoming` decompresses the datagram; anything sent in reply (a retransmission
    /// after a `SequenceNakHeader`) is compressed with `codecs.outgoing` (§4.1.2).
    ///
    /// Every §10.1 check runs first, the fragments included (encrypted bit, size limits and
    /// reassembly table capacity); session state (ISAAC generators, the gap table,
    /// `recvSeqExpected`, the receive time and optional header effects) changes only after
    /// all of them pass.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if any validation rule in §10.1 fails,
    /// [`SessionError::UnexpectedConnectChallenge`] for a `ConnectChallengeHeader` outside
    /// States `1` and `2` (§6.4), or [`SessionError::InvalidState`] for a masked packet before
    /// a `ConnectChallengeHeader` has seeded the incoming ISAAC generator (§7.2).
    pub fn receive_datagram(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        codecs: WireCodecs<'_>,
    ) -> Result<DecodedPacket, SessionError> {
        self.receive_datagram_guarded(datagram, source_addr, now_sec, codecs, &|_| false)
    }

    /// [`Self::receive_datagram`] with a check for recipient IDs other
    /// connections already use: a `ConnectChallengeHeader` that assigns such an ID
    /// is refused before any connection state changes.
    fn receive_datagram_guarded(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        codecs: WireCodecs<'_>,
        rec_id_in_use: &dyn Fn(u16) -> bool,
    ) -> Result<DecodedPacket, SessionError> {
        let result =
            self.receive_datagram_lines(datagram, source_addr, now_sec, codecs, rec_id_in_use);
        self.finish_evidence_at_close();
        result
    }

    /// A connection that closed writes its tail and totals as soon as the pass that closed it has
    /// written its own lines, and not only when it is dropped. Later passes find the log ended
    /// ([`Evidence::finish`]).
    fn finish_evidence_at_close(&mut self) {
        if self.state == ConnectionState::Closing
            && let Some(e) = self.evidence.as_mut()
        {
            e.finish();
        }
    }

    fn receive_datagram_lines(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        codecs: WireCodecs<'_>,
        rec_id_in_use: &dyn Fn(u16) -> bool,
    ) -> Result<DecodedPacket, SessionError> {
        // Rule 4: Source address binding for existing connections
        if source_addr != self.remote_addr {
            return Err(SessionError::SourceAddressMismatch {
                rec_id: self.local_rec_id,
            });
        }

        // Decode framing & structural bounds first without mutating connection state: a
        // datagram refused here commits no checksum generator, sequence, Ack, reassembly or
        // delivery state and is not acknowledged; the connection goes on under its timeouts
        // (§5.6.3 P10-23-POLICY).
        let pkt = decode_datagram(datagram, true, self.receive_codec(codecs.incoming), true)
            .map_err(|err| self.refused(err))?;
        let hdr = pkt.header;

        // §9.3.9 closed transport action table, first row, and §10.2 Vector 24: an UNMASKED datagram
        // with flag 0x00000004 AND any fragment drops whole on ANY queue, before any Ack, baseline,
        // terminal Unsupported or other state commit, even when its keyless checksum is valid.
        // Fragments are only read under that flag ([`parse_packet_payload`]). Standalone unmasked
        // controls and handshakes without fragments keep their own rules below.
        if hdr.flags & FLAG_MASKED_CHECKSUM == 0
            && hdr.flags & FLAG_HAS_FRAGMENTED_BLOBS != 0
            && !pkt.fragments.is_empty()
        {
            let queue_id = pkt.fragments.first().map_or(0, |f| f.header.queue_id);
            return Err(self.refused(SessionError::UnmaskedFragment { queue_id }));
        }

        // §6.3: the incoming recipient is the challenge datagram header's recId and the outgoing
        // one payload bytes 18..19; Tessera adopts only non-zero IDs. §9.3.9: a zero recipient is
        // unsupported at once, every auth retry cancelled and no reply, once its datagram has passed
        // the checks below. A challenge after one was adopted is judged there too.
        //
        // Labelled (`docs/m8-remediation.md` §27 fix 9): "zero" is the checksum-valid header
        // recipient or payload bytes 18..19. The header's opaque peer word is not tested. A
        // checksum is verified here only with the masked flag clear: a challenge with that flag
        // has proved nothing, so it ends nothing and is refused below like any other masked
        // packet before the generators are seeded.
        let zero_recipient = (hdr.flags & FLAG_MASKED_CHECKSUM) == 0
            && pkt
                .opt_headers
                .connect_challenge
                .as_ref()
                .is_some_and(|cr| hdr.rec_id == 0 || cr.outgoing_rec_id == 0);

        if let Some(cr) = &pkt.opt_headers.connect_challenge {
            // §6.4: a ConnectChallengeHeader is acted on only in States 1 and 2.
            if !matches!(
                self.state,
                ConnectionState::SessionAuthSent | ConnectionState::ChallengeAcceptSent
            ) {
                tracing::debug!(
                    state = ?self.state,
                    "ConnectChallengeHeader outside States 1 and 2 dropped"
                );
                let err = SessionError::UnexpectedConnectChallenge { state: self.state };
                return Err(self.refused(err));
            }
            // §6.1, §6.4.1: in State 2 only a repeat of the adopted challenge is answered
            // (with the pending accept); any other is refused before anything changes.
            if !zero_recipient
                && self.state == ConnectionState::ChallengeAcceptSent
                && !self.repeats_adopted_challenge(hdr.rec_id, cr)
            {
                tracing::debug!("a different ConnectChallengeHeader in State 2 dropped");
                return Err(SessionError::UnexpectedConnectChallenge { state: self.state });
            }
            // §3.4: when another active connection occupies the adopted ID, the challenge
            // is ignored (nothing changes, nothing is answered).
            let id = hdr.rec_id;
            if !zero_recipient && id != self.local_rec_id && rec_id_in_use(id) {
                tracing::debug!(
                    rec_id = id,
                    "assigned recipient ID already in use; challenge ignored"
                );
                return Err(SessionError::RecipientIdCollision { rec_id: id });
            }
        }

        // The reassembly key's sender ID, computed once for the check and the ingest: a
        // ConnectChallengeHeader in this packet sets the outgoing recipient before its
        // fragments are ingested (§6.3), so its fragments are keyed by the new ID.
        let sender_rec_id = pkt
            .opt_headers
            .connect_challenge
            .as_ref()
            .map_or(self.server_rec_id, |cr| cr.outgoing_rec_id);

        // Rule 8: well-formed fragments, before any checksum work.
        self.reassembler
            .check_fragments(&pkt.fragments)
            .map_err(|err| self.refused(err))?;

        // Rules 5 & 6: sequence window and checksum, computed on copies (§5.1, §7.2).
        let commit = if (hdr.flags & FLAG_MASKED_CHECKSUM) != 0 {
            self.check_sequenced(hdr, pkt.payload_sum)
                .map_err(|err| self.refused(err))?
        } else {
            SequenceCommit::Unsequenced
        };

        // §9.3.9: `blobFlags` `0x02` on queues 2–4 is judged here, with the checksum verified and
        // nothing committed: one terminal result, no Ack. Only a masked datagram reaches it: an
        // unmasked one with the fragment marker was dropped above.
        let bit = pkt
            .fragments
            .iter()
            .find(|f| unsupported_control_bit(&f.header));
        let unsupported = bit.map(|f| SessionError::UnsupportedFragment {
            queue_id: f.header.queue_id,
            blob_flags: f.header.blob_flags,
        });

        // §9.3.9: the whole datagram goes through the reassembler before the commit below, so no
        // refusal can come after it: a conflict or a full table drops the datagram with nothing
        // applied, its siblings not delivered and its sequence free to be asked for again. A
        // terminal datagram is only judged: a conflict in it drops it like any other.
        let judged = if unsupported.is_some() || zero_recipient {
            self.reassembler
                .check_packet(sender_rec_id, &pkt.fragments, now_sec)
                .map(|()| Vec::new())
        } else {
            self.reassembler
                .ingest_packet(sender_rec_id, &pkt.fragments, now_sec)
        };
        let blobs = match judged {
            Ok(blobs) => blobs,
            Err(err) => {
                if matches!(err, SessionError::ConflictingBlobIdentity) {
                    self.reassembler.note_conflict_dropped();
                }
                return Err(self.refused(err));
            }
        };
        // The zero recipient's terminal result: its datagram passed every check above, so a malformed
        // or refused sibling has already dropped it whole, as for a non-zero recipient, with the auth
        // retries going on. Nothing of it was taken.
        if zero_recipient {
            tracing::debug!("ConnectChallengeHeader with a zero recipient");
            if self.state == ConnectionState::SessionAuthSent {
                self.close(DisconnectReason::InvalidChallenge, now_sec);
            }
            return Err(SessionError::InvalidChallengeRecipient);
        }
        if let Some(err) = unsupported {
            self.closing_disconnect(now_sec);
            self.close(DisconnectReason::UnsupportedTraffic, now_sec);
            return Err(self.refused(err));
        }

        // Every check passed: commit the sequence state.
        match commit {
            SequenceCommit::Unsequenced => {}
            SequenceCommit::FillGap(seq) => {
                self.missing_gaps.remove(&seq);
            }
            SequenceCommit::Advance {
                isaac,
                skipped_masks,
                seq,
            } => {
                self.incoming_isaac = Some(*isaac);
                for (skipped_seq, mask) in skipped_masks {
                    self.missing_gaps.insert(
                        skipped_seq,
                        SequenceGap {
                            mask,
                            opened_at: now_sec,
                        },
                    );
                }
                self.recv_seq_expected = seq;
            }
        }

        // Valid packet accepted: update the receive timestamp, and count its UDP payload
        // bytes (wire prefix and compressed bytes included) for the next flow report (§5.5).
        // Only eligible traffic makes a report due (§9.3.12 P10-10L closed list,
        // [`FlowReport::count`]): an accepted ordinary fragment (not an omitted control) or a
        // ClockSync. A duplicate sequenced packet was dropped above and counts nothing.
        self.last_recv_time_sec = now_sec;
        let eligible = pkt.opt_headers.clock_sync.is_some()
            || pkt.fragments.iter().any(|f| !is_empty_control(&f.header));
        self.flow.count(datagram.len(), eligible);

        // State 2 -> State 5 (§6.4.1): this packet came from the adopted peer and passed the
        // framing, directional decompression, checksum and sequence checks above. A
        // duplicate challenge does not confirm State 5 ([`Self::accept_challenge`]).
        if self.state == ConnectionState::ChallengeAcceptSent
            && pkt.opt_headers.connect_challenge.is_none()
            && pkt.opt_headers.transport_error.is_none()
            && pkt.opt_headers.transport_fatal_error.is_none()
        {
            self.enter_connected_state(now_sec);
        }

        // Process optional headers (§5.2–§5.4, §6.3–§6.7, §8.1–§8.2)
        let headers =
            self.process_optional_headers(&pkt.opt_headers, hdr, now_sec, codecs.outgoing);

        // The blobs this committed datagram completed (duplicate sequenced packets were dropped
        // above, §5.6.4). A challenge that named no usable dictionary closed the connection, and
        // nothing of its datagram is delivered; a resend that could not be encoded takes nothing
        // back from a datagram already committed.
        if headers.is_ok() || self.state != ConnectionState::Closing {
            let decisions = self.reassembler.take_decisions();
            if let Some(e) = self.evidence.as_mut() {
                e.reassembly = self.reassembler.counts();
                for decision in decisions {
                    e.queued(decision, now_sec);
                }
            }
            for blob in blobs {
                if let Some(e) = self.evidence.as_mut() {
                    e.delivered(&blob, now_sec);
                }
                self.events.push_back(SessionEvent::BlobReceived(blob));
            }
        }
        headers?;

        if let Some(e) = self.evidence.as_mut() {
            let gaps = self.missing_gaps.len();
            e.accepted(
                hdr.seq_num,
                datagram.len(),
                pkt.fragments.len(),
                gaps,
                now_sec,
            );
        }
        // Counted only once the whole datagram was taken (diagnostics, `HandshakeCounts`).
        self.counts.received_ok = self.counts.received_ok.saturating_add(1);
        Ok(pkt)
    }

    /// Counts a datagram refused before anything was committed, for the diagnostics, and hands
    /// the error back (§9.3.8). The count closes nothing, because such a datagram is not
    /// authenticated: what ends a stall is a stage's deadline, the silence timeout or the
    /// deadline of a sequence gap ([`MAX_GAP_AGE_SEC`]). A duplicate of a sequence number already taken is ordinary
    /// retransmission overlap and is not a refusal; only State `5` counts, since the handshake
    /// has its own `20.0` s timeout.
    fn refused(&mut self, err: SessionError) -> SessionError {
        match &err {
            // §5.6.2, §5.6.3: queues 5/6 (OPEN-SESSION-12).
            SessionError::UnsupportedReceiveQueue { .. } => {
                self.counts.unsupported_queue_drops =
                    self.counts.unsupported_queue_drops.saturating_add(1);
            }
            // §5.6.3: blobFlags 0x02 outside queue 1 (OPEN-SESSION-14).
            SessionError::UnsupportedFragment { .. } => {
                self.counts.unsupported_fragment_drops =
                    self.counts.unsupported_fragment_drops.saturating_add(1);
            }
            _ => {}
        }
        if self.state == ConnectionState::Connected
            && !matches!(err, SessionError::DuplicateSequence { .. })
        {
            self.counts.refused_while_connected =
                self.counts.refused_while_connected.saturating_add(1);
        }
        err
    }

    /// Checks a masked (sequenced) packet against the window, the gap table and its ISAAC
    /// word without changing any state (§5.1, §5.3, §7.2, §10.1 rules 5 and 6).
    fn check_sequenced(
        &self,
        hdr: DatagramHeader,
        payload_sum: u32,
    ) -> Result<SequenceCommit, SessionError> {
        // §7.2: masked packets need the incoming generator a ConnectChallengeHeader seeds.
        let Some(incoming) = self.incoming_isaac.as_ref() else {
            tracing::debug!("masked packet before the ISAAC generators were seeded");
            return Err(SessionError::InvalidState { state: self.state });
        };
        let seq = hdr.seq_num;
        if seq > self.recv_seq_expected.saturating_add(MAX_SEQ_LOOKAHEAD) {
            return Err(SessionError::SequenceOutOfWindow {
                seq_num: seq,
                expected: self.recv_seq_expected,
            });
        }
        if seq <= self.recv_seq_expected {
            let Some(gap) = self.missing_gaps.get(&seq) else {
                // Duplicate or already processed sequence number: its ISAAC mask was consumed
                // and not kept, so the checksum cannot be verified. Drop the packet before any
                // optional header acts or the receive timestamp moves (§10.1 rule 6).
                return Err(SessionError::DuplicateSequence {
                    seq_num: seq,
                    expected: self.recv_seq_expected,
                });
            };
            verify_packet_checksum(hdr, payload_sum, Some(gap.mask))?;
            return Ok(SequenceCommit::FillGap(seq));
        }
        let new_gaps = (seq - self.recv_seq_expected - 1) as usize;
        if self.missing_gaps.len().saturating_add(new_gaps) > MAX_MISSING_SEQ_GAPS {
            return Err(SessionError::GapLimitExceeded {
                count: self.missing_gaps.len().saturating_add(new_gaps),
            });
        }
        let mut isaac = incoming.clone();
        let mut skipped_masks = Vec::with_capacity(new_gaps);
        for skipped_seq in (self.recv_seq_expected + 1)..seq {
            skipped_masks.push((skipped_seq, isaac.next_mask()));
        }
        let packet_mask = isaac.next_mask();
        verify_packet_checksum(hdr, payload_sum, Some(packet_mask))?;
        Ok(SequenceCommit::Advance {
            isaac: Box::new(isaac),
            skipped_masks,
            seq,
        })
    }

    fn enter_connected_state(&mut self, now_sec: f64) {
        self.state = ConnectionState::Connected;
        self.state_entered_at_sec = now_sec;
        self.last_clock_sync_sent_sec = now_sec;
        self.events.push_back(SessionEvent::Connected {
            local_rec_id: self.local_rec_id,
            server_rec_id: self.server_rec_id,
        });
        // §8.1: no ClockSyncHeader on entry (that rule is withdrawn); the periodic timer
        // runs from here.
    }

    /// Answers a `ConnectChallengeHeader` that arrived in a packet with `DatagramHeader`
    /// `incoming` (§6.3, §6.4, §7.2, §7.3).
    fn accept_challenge(
        &mut self,
        cr: &ConnectChallengeHeader,
        incoming: DatagramHeader,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        self.counts.challenges = self.counts.challenges.saturating_add(1);
        if self.state == ConnectionState::ChallengeAcceptSent {
            return self.repeat_accept(cr, incoming, now_sec);
        }
        // §7.3: the challenge must name the install's incoming dictionary, compared after the
        // field-wise conversion; a null UUID, a mismatch or a connection without
        // dictionaries is an explicit unsupported-negotiation result.
        let named = cr.incoming_codec_uuid();
        let Some(uuids) = self
            .codec_uuids
            .filter(|u| named != [0u8; 16] && u.incoming == named)
        else {
            self.close(DisconnectReason::UnsupportedCodec, now_sec);
            return Err(SessionError::UnsupportedCodecUuid {
                uuid: cr.incoming_codec_guid,
            });
        };
        // §6.3: the incoming recipient from the challenge datagram header, the outgoing
        // one from payload bytes 18..19 (both checked non-zero and free before). The
        // clock candidate is parsed and not used (OPEN-SESSION-1); the clock comes from
        // ClockSyncHeader (§8.1).
        self.local_rec_id = incoming.rec_id;
        self.server_rec_id = cr.outgoing_rec_id;
        self.adopted_challenge = Some((incoming.rec_id, *cr));
        self.codecs_negotiated = true;
        // §4.2, §6.4: the accept's own word and the challenge header's bytes 18..19.
        let word_before = self.client_word;
        self.client_word = INITIAL_CLIENT_WORD;
        self.peer_word = incoming.peer_word;
        // §7.2: the incoming generator from payload bytes 20..23, the outgoing one from
        // 24..27.
        self.outgoing_isaac = Some(IsaacCipher::from_seed(cr.outgoing_seed));
        self.incoming_isaac = Some(IsaacCipher::from_seed(cr.incoming_seed));

        // §6.4: the cookie reversed, the outgoing dictionary's on-disk UUID and this
        // connection's maxRecvRate.
        let resp = ConnectAcceptHeader {
            check_value: cr.check_value(),
            codec_uuid: uuids.outgoing,
            max_recv_rate: self.max_recv_rate,
        };
        self.stored_connect_accept = Some(resp);

        // Unsequenced, uncompressed, sequence zero (no ISAAC word, §6.4).
        let hdr = self.base_datagram_header(0);
        let resp_opt = OptionalHeaders {
            connect_accept: Some(resp),
            ..OptionalHeaders::default()
        };
        let dgram = Self::encode_own(hdr, &resp_opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        self.counts.connect_accept = self.counts.connect_accept.saturating_add(1);
        let line = SendEvidence::of(SendKind::Accept, 0, &resp_opt, &[]).word(
            word_before,
            self.client_word,
            WordInput::AcceptInitial,
        );
        self.note_send(line, now_sec);

        // Only State 1 reaches here (State 2 answers with [`Self::repeat_accept`]).
        self.state = ConnectionState::ChallengeAcceptSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;
        // State 1 is left: the stored credentials and cookie are no longer retransmitted.
        self.forget_auth();
        Ok(())
    }

    /// Whether a challenge with datagram `recId` `rec_id` repeats the adopted one: the
    /// same recipients, cookie, seeds and dictionary GUID (engineering choice, spec silent;
    /// the clock candidate and the ignored bytes `16..17` are not compared).
    fn repeats_adopted_challenge(&self, rec_id: u16, cr: &ConnectChallengeHeader) -> bool {
        self.adopted_challenge.is_some_and(|(adopted_id, adopted)| {
            adopted_id == rec_id
                && adopted.cookie == cr.cookie
                && adopted.outgoing_rec_id == cr.outgoing_rec_id
                && adopted.incoming_seed == cr.incoming_seed
                && adopted.outgoing_seed == cr.outgoing_seed
                && adopted.incoming_codec_guid == cr.incoming_codec_guid
        })
    }

    /// A challenge in State `2` (§6.1, §6.4.1): it only causes the pending accept to be
    /// resent; it does not confirm State `5` and nothing is adopted again. A challenge that
    /// does not repeat the adopted one was refused before anything changed
    /// ([`Self::repeats_adopted_challenge`]).
    fn repeat_accept(
        &mut self,
        cr: &ConnectChallengeHeader,
        incoming: DatagramHeader,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        let Some(resp) = self
            .stored_connect_accept
            .filter(|_| self.repeats_adopted_challenge(incoming.rec_id, cr))
        else {
            return Err(SessionError::UnexpectedConnectChallenge { state: self.state });
        };
        self.last_handshake_retransmit_sec = now_sec;
        let hdr = self.base_datagram_header(0);
        let opt = OptionalHeaders {
            connect_accept: Some(resp),
            ..OptionalHeaders::default()
        };
        let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        self.counts.connect_accept_retransmits =
            self.counts.connect_accept_retransmits.saturating_add(1);
        self.note_handshake(SendKind::AcceptResend, now_sec);
        Ok(())
    }

    /// A NAK for `seq` was answered by `action`: counted when `seq` is the watched blob's.
    fn note_watched(&mut self, seq: u32, action: KeptBlobAction) {
        let Some(w) = self.watched.as_mut().filter(|w| w.seqs.contains(&seq)) else {
            return;
        };
        match action {
            KeptBlobAction::NakResend => {
                w.fate.nak_resends = w.fate.nak_resends.saturating_add(1);
            }
            KeptBlobAction::OutOfOrderAck => {
                w.fate.out_of_order_acks = w.fate.out_of_order_acks.saturating_add(1);
            }
            KeptBlobAction::CoveredByCumulativeAck => {}
        }
        w.fate.last = Some(action);
    }

    fn process_optional_headers(
        &mut self,
        opt: &OptionalHeaders,
        incoming: DatagramHeader,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        // ConnectChallengeHeader (0x00040000, §6.3, §6.4)
        if let Some(cr) = &opt.connect_challenge {
            self.accept_challenge(cr, incoming, now_sec)?;
        }

        // CumulativeAckHeader (0x00004000, big-endian u32, §5.2)
        if let Some(ack_seq) = opt.cumulative_ack {
            self.retransmit_table
                .retain(|&seq, _| (seq.wrapping_sub(ack_seq) as i32) > 0);
            if let Some(w) = self.watched.as_mut()
                && !w.fate.covered_by_cumulative_ack
                && w.seqs
                    .iter()
                    .all(|&seq| (seq.wrapping_sub(ack_seq) as i32) <= 0)
            {
                w.fate.covered_by_cumulative_ack = true;
                w.fate.last = Some(KeptBlobAction::CoveredByCumulativeAck);
            }
        }

        // SequenceNakHeader (0x00001000, §5.3)
        if let Some(nak_seqs) = &opt.sequence_nak {
            if let Some(&first_nak) = nak_seqs.first() {
                // Implicitly acknowledge all sent packets strictly before nak_seqs[0] (§5.3),
                // except a retained `0xF4`: the §5.3 F4 exception and §9.3.9's F4 retention end it
                // only with an explicit cumulative Ack covering it or the connection's close. The
                // watched `0x13D` is kept the same way ([`Self::send_blob_watched`], labelled C8
                // choice). The EC and F5 records follow the generic rule. An inbound
                // OutOfOrderAck never ends it either (below).
                self.retransmit_table.retain(|&seq, rec| {
                    rec.kept_until_acked || (seq.wrapping_sub(first_nak) as i32) >= 0
                });
            }
            // Resends go out with the outgoing (client-to-server) dictionary (§4.1.2).
            let codec = self.data_codec(outgoing_codec);
            for &s in nak_seqs {
                let resend = self
                    .retransmit_table
                    .get(&s)
                    .filter(|rec| rec.retransmit_eligible || rec.retained);
                if let Some(rec) = resend {
                    if !rec.retransmit_eligible {
                        self.counts.retained_resends =
                            self.counts.retained_resends.saturating_add(1);
                    }
                    // The saved sequence number, mask and client word (§4.2, §5.3).
                    let mut hdr = self.base_datagram_header(rec.seq_num);
                    hdr.client_word = rec.client_word;
                    let dgram = Self::encode_own(
                        hdr,
                        &rec.opt_headers,
                        &rec.fragments,
                        Some(rec.xor_mask),
                        codec,
                    )?;
                    let mut line = SendEvidence::of(
                        SendKind::Resend,
                        rec.seq_num,
                        &rec.opt_headers,
                        &rec.fragments,
                    )
                    .word(
                        rec.client_word,
                        rec.client_word,
                        WordInput::ResendSaved,
                    );
                    // The report it carries is the saved one, not a new report.
                    if rec.opt_headers.flow_control.is_some() {
                        line.flow_event = "resend-saved-report";
                    }
                    self.note_send(line, now_sec);
                    self.outbound_datagrams.push_back(dgram);
                    self.note_watched(s, KeptBlobAction::NakResend);
                } else if self.out_of_order_ack_queue.len() >= MAX_PENDING_OUT_OF_ORDER_ACKS {
                    self.dropped_out_of_order_acks =
                        self.dropped_out_of_order_acks.saturating_add(1);
                } else if !self.out_of_order_ack_queue.contains(&s) {
                    self.out_of_order_ack_queue.push_back(s);
                    self.note_watched(s, KeptBlobAction::OutOfOrderAck);
                } else {
                    self.note_watched(s, KeptBlobAction::OutOfOrderAck);
                }
            }
        }

        // OutOfOrderAckHeader (0x00002000, §5.4): the listed sequence numbers will never
        // arrive. `recv_seq_expected` is already the highest sequence received, so dropping
        // the gaps is all that is needed for acknowledgements to resume. It names the SERVER's
        // sequence space: it never touches the outbound retransmit table, so a retained `0xF4`
        // stays even when a number collides (§9.3.9 F4 retention, §10.2 Vector 24).
        if let Some(dropped_seqs) = &opt.out_of_order_ack {
            for &s in dropped_seqs {
                self.missing_gaps.remove(&s);
            }
        }

        // WorldReferralHeader (0x00000800, §6.5): the address and port as received. A zero address
        // or port is judged where the selection is known (§9.3.9 "Referral endpoint validity"):
        // unsupported required progress for a pending selection, a skip otherwise, and never a bind
        // or a send to it. So the session layer makes no connection from this event.
        if let Some(ref_hdr) = &opt.world_referral {
            self.events.push_back(SessionEvent::WorldReferralReceived {
                cookie: ref_hdr.cookie,
                world_addr: ref_hdr.target_addr,
            });
        }

        // ServerRedirectHeader (0x00000100, §6.6): four opaque bytes, no state effect
        // (OPEN-SESSION-17).

        // AuthServerAddressHeader (0x00000200, §6.6)
        if let Some(lsa) = opt.auth_server_address {
            self.events
                .push_back(SessionEvent::AuthServerAddress(lsa.addr));
        }

        // ClockSyncHeader (0x01000000, §8.1): the synchronized network clock offset.
        if let Some(server_time) = opt.clock_sync.filter(|t| t.is_finite()) {
            self.server_clock_offset_sec = server_time - now_sec;
        }

        // TransportErrorHeader (0x00100000) or TransportFatalErrorHeader (0x00200000, §6.7):
        // log both numbers and terminate (OPEN-SESSION-4). §6.7 lists errorCode 0 as
        // "normal / no error" but gives a receiver only the one rule, so a header with code 0
        // terminates the session too (engineering choice, spec silent).
        for (fatal, err) in [
            (false, opt.transport_error),
            (true, opt.transport_fatal_error),
        ] {
            if let Some(ne) = err
                && self.state != ConnectionState::Closing
            {
                // §9.3.8: the numbers are for the local debug log only. Under the evidence target, so
                // the documented directive turns on this line and not the crate's other debug lines,
                // and only with the evidence log's local opt-in (§9.3.11 "Required live-log
                // CONTENT"); without it the line names no number.
                match self.evidence.as_ref() {
                    Some(e) => tracing::debug!(
                        target: EVIDENCE_TARGET,
                        role = e.role,
                        direction = "recv",
                        fatal,
                        error_code = format_args!("{:#010x}", ne.error_code),
                        error_subcode = format_args!("{:#010x}", ne.error_subcode),
                        "official evidence: transport error from server; closing the connection"
                    ),
                    None => tracing::debug!(
                        fatal,
                        "official session: transport error from server; closing the connection"
                    ),
                }
                self.close(
                    DisconnectReason::TransportError {
                        fatal,
                        error_code: ne.error_code,
                        error_subcode: ne.error_subcode,
                    },
                    now_sec,
                );
            }
        }

        // Graceful DisconnectHeader (0x00008000, §8.2). §8.2 states the State 5 rule; in
        // the handshake (States 1, 2) and while a local disconnect runs (States 6, 7) the
        // connection closes as well (engineering choice, spec silent).
        if opt.disconnect && self.state != ConnectionState::Closing {
            self.close(DisconnectReason::ServerDisconnect, now_sec);
        }

        Ok(())
    }

    /// Runs periodic timers for the connection at `now_sec` (§5.2–§5.4, §6.1, §8.1–§8.2):
    /// - State `1`: retransmits `SessionAuthHeader` / `ReferralAuthHeader` every `2.0` s
    ///   (`20.0` s handshake timeout).
    /// - State `2`: retransmits `ConnectAcceptHeader` every `0.6` s (`20.0` s timeout).
    /// - State `5`: checks the `60.0` s / `300.0` s silence timeout, flushes
    ///   `OutOfOrderAckHeader` (no standalone ClockSync: the header rides on a sequenced
    ///   packet once its `30.0` s are up, [`Self::send_sequenced_packet`], `OPEN-SESSION-9`),
    ///   sends the FlowControl-only packets of the §9.3.12 P10-10L policy (the first tick
    ///   after State `5`, then a tick after eligible traffic that no other report carried, one
    ///   [`FLOW_REPORT_ONLY_PERIOD_SEC`] after the last report, standalone or attached; pure
    ///   acknowledgements make none due), and
    ///   sends `SequenceNakHeader` every `0.6` s when gaps exist, or `CumulativeAckHeader`
    ///   every `2.0` s when none exist.
    /// - State `6` / `7`: flushes outbound reliable packets, then retransmits
    ///   `DisconnectHeader` every `1.0` s.
    ///
    /// Every control packet here is unsequenced and goes out with `wirePrefix` `0x0000`
    /// (§4.1.1); the FlowControl-only packets are sequenced and use no codec here (see
    /// [`Self::tick_with_codec`]).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if encoding a periodic packet fails.
    pub fn tick(&mut self, now_sec: f64) -> Result<(), SessionError> {
        self.tick_with_codec(now_sec, None)
    }

    /// [`Self::tick`], compressing its sequenced FlowControl-only packets with
    /// `outgoing_codec` (the client-to-server dictionary, §4.1.2, §6.4.1) once the codecs
    /// are negotiated.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if encoding a periodic packet fails.
    pub fn tick_with_codec(
        &mut self,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        let result = self.tick_timers(now_sec, outgoing_codec);
        self.finish_evidence_at_close();
        result
    }

    fn tick_timers(
        &mut self,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        self.reassembler.purge_expired(now_sec);

        match self.state {
            ConnectionState::SessionAuthSent => {
                if now_sec - self.state_entered_at_sec >= HANDSHAKE_TIMEOUT_SEC {
                    self.close(DisconnectReason::HandshakeTimeout, now_sec);
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= SESSION_AUTH_RETRANSMIT_SEC {
                    self.last_handshake_retransmit_sec = now_sec;
                    // The stored first datagram again, with sequence zero (§5.1, §6.5).
                    let hdr = self.base_datagram_header(0);
                    let opt = OptionalHeaders {
                        session_auth: self.stored_session_auth.clone(),
                        referral_auth: self.stored_referral_auth,
                        ..OptionalHeaders::default()
                    };
                    let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
                    self.outbound_datagrams.push_back(dgram);
                    self.counts.auth_retransmits = self.counts.auth_retransmits.saturating_add(1);
                    self.note_handshake(SendKind::AuthResend, now_sec);
                }
            }
            ConnectionState::ChallengeAcceptSent => {
                if now_sec - self.state_entered_at_sec >= HANDSHAKE_TIMEOUT_SEC {
                    self.close(DisconnectReason::HandshakeTimeout, now_sec);
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= CONNECT_ACCEPT_RETRANSMIT_SEC
                    && let Some(resp) = self.stored_connect_accept
                {
                    self.last_handshake_retransmit_sec = now_sec;
                    // Sequence zero, as the first accept (§6.4).
                    let hdr = self.base_datagram_header(0);
                    let opt = OptionalHeaders {
                        connect_accept: Some(resp),
                        ..OptionalHeaders::default()
                    };
                    let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
                    self.outbound_datagrams.push_back(dgram);
                    self.counts.connect_accept_retransmits =
                        self.counts.connect_accept_retransmits.saturating_add(1);
                    self.note_handshake(SendKind::AcceptResend, now_sec);
                }
            }
            ConnectionState::Connected => {
                let silence_limit = if self.extended_silence_timeout {
                    EXTENDED_SILENCE_TIMEOUT_SEC
                } else {
                    SILENCE_TIMEOUT_SEC
                };
                if now_sec - self.last_recv_time_sec > silence_limit {
                    // §9.3.9: exactly one Disconnect on the affected connection, no drain.
                    self.closing_disconnect(now_sec);
                    self.close(DisconnectReason::SilenceTimeout, now_sec);
                    return Ok(());
                }

                // §9.3.8: one non-resetting deadline; a gap that stays open ends the connection
                // rather than being NAKed forever.
                if self
                    .oldest_gap_opened_at()
                    .is_some_and(|t| now_sec - t > MAX_GAP_AGE_SEC)
                {
                    tracing::info!(
                        refused = self.counts.refused_while_connected,
                        gaps = self.missing_gaps.len(),
                        "official session: a sequence gap stayed open; closing the connection"
                    );
                    // §9.3.8: one Disconnect if established, and this is the last moment it is,
                    // before the connection stops being State 5.
                    self.closing_disconnect(now_sec);
                    self.close(DisconnectReason::GapDeadline, now_sec);
                    return Ok(());
                }

                // Flush up to 114 OutOfOrderAckHeader entries if any (§5.4)
                if !self.out_of_order_ack_queue.is_empty() {
                    let count = self.out_of_order_ack_queue.len().min(MAX_NAK_ENTRIES);
                    let list: Vec<u32> = self.out_of_order_ack_queue.drain(..count).collect();
                    self.queue_control(OptionalHeaders {
                        out_of_order_ack: Some(list),
                        ..OptionalHeaders::default()
                    })?;
                    self.note_control(SendKind::OutOfOrderAck, now_sec);
                }

                // §9.3.12 P10-10L: the first FlowControl-only packet, then one for eligible
                // traffic no other report carried, one FLOW_REPORT_ONLY_PERIOD_SEC after the last
                // report sent, standalone or attached.
                //
                // Labelled choice (C8, pending writer answer to remediation §31 fix 5): "may send"
                // is read as "sends": a due standalone report goes at the FIRST maintenance tick at
                // or after its due instant. A carrier at the same instant takes the report first
                // (the driver queues a pass's carriers before its tick), which clears the
                // eligibility, so that instant carries one report, on the carrier.
                if self.flow.flow_only_due_at().is_some_and(|t| now_sec >= t) {
                    self.send_flow_only(now_sec, outgoing_codec)?;
                }

                // SequenceNakHeader every 0.6 s while gaps exist (§5.3), else
                // CumulativeAckHeader every 2.0 s (§5.2)
                if !self.missing_gaps.is_empty() {
                    if now_sec - self.last_ack_nak_sent_sec >= SEQUENCE_NAK_INTERVAL_SEC {
                        self.last_ack_nak_sent_sec = now_sec;
                        let list: Vec<u32> = self
                            .missing_gaps
                            .keys()
                            .copied()
                            .take(MAX_NAK_ENTRIES)
                            .collect();
                        self.queue_control(OptionalHeaders {
                            sequence_nak: Some(list),
                            ..OptionalHeaders::default()
                        })?;
                        self.note_control(SendKind::Nak, now_sec);
                    }
                } else if self.recv_seq_expected > 1
                    && now_sec - self.last_ack_nak_sent_sec >= CUMULATIVE_ACK_INTERVAL_SEC
                {
                    self.last_ack_nak_sent_sec = now_sec;
                    self.queue_control(OptionalHeaders {
                        cumulative_ack: Some(self.recv_seq_expected),
                        ..OptionalHeaders::default()
                    })?;
                    self.note_control(SendKind::Ack, now_sec);
                }
            }
            ConnectionState::DisconnectingFlush => {
                if !self.has_reliable_unacked()
                    || now_sec - self.state_entered_at_sec >= DISCONNECT_FLUSH_LIMIT_SEC
                {
                    self.state = ConnectionState::DisconnectSent;
                    self.state_entered_at_sec = now_sec;
                    self.last_handshake_retransmit_sec = now_sec;
                    self.queue_control(OptionalHeaders {
                        disconnect: true,
                        ..OptionalHeaders::default()
                    })?;
                    self.note_control(SendKind::Disconnect, now_sec);
                }
            }
            ConnectionState::DisconnectSent => {
                if now_sec - self.state_entered_at_sec >= DISCONNECT_SENT_LIMIT_SEC {
                    self.close(DisconnectReason::LocalDisconnect, now_sec);
                } else if now_sec - self.last_handshake_retransmit_sec >= DISCONNECT_RETRANSMIT_SEC
                {
                    self.last_handshake_retransmit_sec = now_sec;
                    self.queue_control(OptionalHeaders {
                        disconnect: true,
                        ..OptionalHeaders::default()
                    })?;
                    self.note_control(SendKind::DisconnectResend, now_sec);
                }
            }
            ConnectionState::Idle
            | ConnectionState::ServerListening
            | ConnectionState::ServerConnectSent
            | ConnectionState::Closing => {}
        }

        Ok(())
    }

    /// The earliest time (on the clock of `now_sec`) at which [`Self::tick`] or the
    /// multiplexer's cleanup has work on this connection: the handshake retransmits and
    /// timeouts (§6.1), the out-of-order acknowledgements waiting (§5.4, due at once), the
    /// first FlowControl-only packet (§5.5, due at once) and a later one for unreported
    /// eligible traffic ([`FLOW_REPORT_ONLY_PERIOD_SEC`] after the previous, §9.3.12), the
    /// `SequenceNakHeader` or `CumulativeAckHeader` (§5.2, §5.3), the silence timeout
    /// (§8.2), the disconnect's flush, retransmit and end (§6.1, §8.2), the cleanup after
    /// [`CLOSING_CLEANUP_SEC`] in State `8` (§6.1) and the expiry of an incomplete blob
    /// (§5.6.4). `None` when no timer runs (States `0`, `3`, `4` with no
    /// incomplete blob). A time already past is returned as it is: the caller ticks at once.
    /// Every timer read here is one [`Self::tick`] (or the multiplexer's cleanup) acts on
    /// when it is due, from the same constants ([`HANDSHAKE_TIMEOUT_SEC`] and the others),
    /// so a reported time never stays in the past. Queued outbound datagrams are no
    /// deadline ([`SessionMultiplexer::next_deadline`]).
    ///
    /// A timeout that fires only once `now - t > limit` (the silence timeout, the blob
    /// expiry) is due at `t + limit`; a tick at exactly that time may find it not yet over,
    /// so the caller waits a little more (the connector waits at least 1 ms).
    #[must_use]
    pub fn next_deadline(&self) -> Option<f64> {
        let mut next = self.reassembler.next_expiry();
        let mut at = |t: f64| next = Some(next.map_or(t, |n: f64| n.min(t)));
        match self.state {
            ConnectionState::SessionAuthSent => {
                at(self.state_entered_at_sec + HANDSHAKE_TIMEOUT_SEC);
                at(self.last_handshake_retransmit_sec + SESSION_AUTH_RETRANSMIT_SEC);
            }
            ConnectionState::ChallengeAcceptSent => {
                at(self.state_entered_at_sec + HANDSHAKE_TIMEOUT_SEC);
                if self.stored_connect_accept.is_some() {
                    at(self.last_handshake_retransmit_sec + CONNECT_ACCEPT_RETRANSMIT_SEC);
                }
            }
            ConnectionState::Connected => {
                let silence_limit = if self.extended_silence_timeout {
                    EXTENDED_SILENCE_TIMEOUT_SEC
                } else {
                    SILENCE_TIMEOUT_SEC
                };
                at(self.last_recv_time_sec + silence_limit);
                if !self.out_of_order_ack_queue.is_empty() {
                    // Flushed on the next tick (§5.4).
                    at(f64::NEG_INFINITY);
                }
                // The first FlowControl-only packet goes out on the first tick after
                // State 5, a later one a report tick period after the previous (§5.5).
                if let Some(t) = self.flow.flow_only_due_at() {
                    at(t);
                }
                if let Some(opened) = self.oldest_gap_opened_at() {
                    at(opened + MAX_GAP_AGE_SEC);
                    at(self.last_ack_nak_sent_sec + SEQUENCE_NAK_INTERVAL_SEC);
                } else if self.recv_seq_expected > 1 {
                    at(self.last_ack_nak_sent_sec + CUMULATIVE_ACK_INTERVAL_SEC);
                }
            }
            ConnectionState::DisconnectingFlush => {
                if !self.has_reliable_unacked() {
                    at(f64::NEG_INFINITY);
                } else {
                    at(self.state_entered_at_sec + DISCONNECT_FLUSH_LIMIT_SEC);
                }
            }
            ConnectionState::DisconnectSent => {
                at(self.state_entered_at_sec + DISCONNECT_SENT_LIMIT_SEC);
                at(self.last_handshake_retransmit_sec + DISCONNECT_RETRANSMIT_SEC);
            }
            ConnectionState::Closing => at(self.state_entered_at_sec + CLOSING_CLEANUP_SEC),
            ConnectionState::Idle
            | ConnectionState::ServerListening
            | ConnectionState::ServerConnectSent => {}
        }
        next
    }

    /// A standalone control went into the queue: one evidence line, with the copied sequence and the
    /// unchanged word (§9.3.9).
    fn note_control(&mut self, kind: SendKind, now_sec: f64) {
        let (seq, word) = (self.send_seq_num, self.client_word);
        let line = SendEvidence::of(kind, seq, &OptionalHeaders::default(), &[]).word(
            word,
            word,
            WordInput::ControlUnchanged,
        );
        self.note_send(line, now_sec);
    }

    /// A handshake packet that does not move the word went into the queue, with sequence zero (§5.1,
    /// §6.2, §6.4). Nothing of its ticket or cookie is in the line.
    fn note_handshake(&mut self, kind: SendKind, now_sec: f64) {
        let word = self.client_word;
        let line = SendEvidence::of(kind, 0, &OptionalHeaders::default(), &[]).word(
            word,
            word,
            WordInput::HandshakeUnchanged,
        );
        self.note_send(line, now_sec);
    }

    /// One send line, with what the flow accumulator holds now on every kind of line (§9.3.9): a
    /// line that does not report leaves the accumulator as it is, and shows that.
    fn note_send(&mut self, mut line: SendEvidence, now_sec: f64) {
        line.flow_accumulated = self.flow.bytes;
        if let Some(e) = self.evidence.as_mut() {
            e.sent(line, now_sec);
        }
    }

    /// When the oldest open sequence gap opened (its sequence number is the lowest).
    fn oldest_gap_opened_at(&self) -> Option<f64> {
        self.missing_gaps
            .first_key_value()
            .map(|(_, gap)| gap.opened_at)
    }

    /// Whether a sent packet with reliable content still waits for its acknowledgement
    /// (State `6` drains these, §6.1): packets with only unreliable content, such as the
    /// FlowControl-only ones, are never resent (§5.3) and do not hold the flush.
    fn has_reliable_unacked(&self) -> bool {
        self.retransmit_table
            .values()
            .any(|r| r.retransmit_eligible)
    }

    /// Queues an unsequenced control packet that goes out uncompressed without
    /// `MaskedChecksumFlag` (§4.1.1): standalone Ack, SequenceNak, OutOfOrderAck and Disconnect
    /// copy the last allocated outgoing sequence number and the current client word, advance
    /// neither and draw no mask (§9.3.7 "Unsequenced controls", P10-23-POLICY; Vectors 9 and 13).
    ///
    /// §9.3.8: before any sequenced send this is the initialized counter `1` (Vector 17), later
    /// the last allocated sequence; the current interim local word is kept and not advanced, and
    /// no ISAAC word is consumed.
    fn queue_control(&mut self, opt: OptionalHeaders) -> Result<(), SessionError> {
        let hdr = self.base_datagram_header(self.send_seq_num);
        let dgram = Self::encode_own(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }
}

/// Single-socket multi-connection demultiplexer (§3.4, §6.5).
///
/// Each connection is held under a handle the multiplexer gives out
/// ([`Self::create_connection`]) and keeps for the connection's life. The handle is not the
/// connection's recipient ID: a new connection has `localRecId = 0x0000` (§3.4) and takes
/// the ID its server's `ConnectChallengeHeader` assigns, when no other connection of this
/// multiplexer occupies that slot (otherwise the challenge is ignored).
#[derive(Debug, Default)]
pub struct SessionMultiplexer {
    connections: BTreeMap<u16, SessionConnection>,
    next_handle: u16,
    /// Recipient IDs of removed connections; their traffic is refused until another
    /// connection takes the ID (engineering choice, spec silent).
    retired: BTreeSet<u16>,
}

impl SessionMultiplexer {
    /// Creates an empty `SessionMultiplexer`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new `SessionConnection` to `remote_addr` (State `0`, `localRecId = 0x0000`,
    /// §3.4) and returns its handle.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::RecipientIdsExhausted`] when every handle is in use (the
    /// search is bounded, it never spins).
    pub fn create_connection(
        &mut self,
        remote_addr: SocketAddrV4,
        now_sec: f64,
    ) -> Result<u16, SessionError> {
        let mut candidate = self.next_handle;
        let mut probes = 0u32;
        while self.connections.contains_key(&candidate) {
            probes += 1;
            if probes > u32::from(u16::MAX) {
                return Err(SessionError::RecipientIdsExhausted);
            }
            candidate = candidate.wrapping_add(1);
        }
        self.next_handle = candidate.wrapping_add(1);
        self.connections
            .insert(candidate, SessionConnection::new(remote_addr, now_sec));
        Ok(candidate)
    }

    /// Removes the connection with `handle` and returns it. Datagrams addressed to its
    /// recipient ID are refused (not routed by source address to another connection) until
    /// another connection takes the ID.
    pub fn remove_connection(&mut self, handle: u16) -> Option<SessionConnection> {
        let conn = self.connections.remove(&handle)?;
        if conn.local_rec_id() != 0 {
            self.retired.insert(conn.local_rec_id());
        }
        Some(conn)
    }

    /// Returns a shared reference to the connection with `handle`.
    #[must_use]
    pub fn connection(&self, handle: u16) -> Option<&SessionConnection> {
        self.connections.get(&handle)
    }

    /// Returns a mutable reference to the connection with `handle`.
    pub fn connection_mut(&mut self, handle: u16) -> Option<&mut SessionConnection> {
        self.connections.get_mut(&handle)
    }

    /// The handle of the connection whose `localRecId` is `rec_id` (non-zero), if any.
    #[must_use]
    pub fn handle_of(&self, rec_id: u16) -> Option<u16> {
        if rec_id == 0 {
            return None;
        }
        self.connections
            .iter()
            .find(|(_, c)| c.local_rec_id() == rec_id)
            .map(|(&h, _)| h)
    }

    /// Routes an incoming UDP datagram from `source_addr` per §3.4 and §10.1 rule 4:
    /// - If `recId` (the non-zero `wirePrefix`, or `DatagramHeader.recId` behind a zero
    ///   prefix) is non-zero and a connection adopted it with `source_addr` as its peer,
    ///   that connection takes the packet (§6.3: any adopted non-zero ID; the earlier range
    ///   check is superseded).
    /// - Otherwise (`recId` `0`, an ID no connection adopted, or one whose peer is another
    ///   address) the connection whose remote `(IPv4, port)` equals `source_addr` takes it;
    ///   if that connection is past State `1` (its `localRecId` is non-zero) and the packet's
    ///   `recId` is neither `0x0000` nor that `localRecId`, the packet is rejected.
    /// - With no match the datagram is rejected.
    ///
    /// Returns the handle of the connection that took it.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if no connection matches or packet validation fails.
    pub fn receive_from_peer(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        codecs: WireCodecs<'_>,
    ) -> Result<(u16, DecodedPacket), SessionError> {
        if !raw_length_admitted(datagram) {
            return Err(SessionError::DatagramTooShort {
                len: datagram.len(),
            });
        }
        let wire_prefix = u16::from_be_bytes([datagram[0], datagram[1]]);
        let header_rec_id = if wire_prefix == 0x0000 {
            u16::from_be_bytes([datagram[2], datagram[3]])
        } else {
            wire_prefix
        };
        let unknown = SessionError::UnknownRecipient {
            rec_id: header_rec_id,
        };

        if self.retired.contains(&header_rec_id) {
            tracing::debug!(
                rec_id = header_rec_id,
                "datagram for a removed connection refused"
            );
            return Err(unknown);
        }
        // Step 2: the slot `recId`, from its own peer's address.
        let by_id = (header_rec_id != 0)
            .then(|| {
                self.connections
                    .iter()
                    .find(|(_, c)| {
                        c.local_rec_id() == header_rec_id && c.remote_addr() == source_addr
                    })
                    .map(|(&h, _)| h)
            })
            .flatten();
        // Step 3: by source address. A closing or never-started connection takes nothing; a
        // connection in the handshake (States 1, 2) is preferred over the others with the
        // same peer (engineering choice, spec silent: §3.4 says "one whose remote address
        // matches").
        let target_key = by_id.or_else(|| {
            let candidates = || {
                self.connections.iter().filter(|(_, c)| {
                    c.remote_addr() == source_addr
                        && !matches!(c.state(), ConnectionState::Closing | ConnectionState::Idle)
                })
            };
            let found = candidates()
                .find(|(_, c)| {
                    matches!(
                        c.state(),
                        ConnectionState::SessionAuthSent | ConnectionState::ChallengeAcceptSent
                    )
                })
                .or_else(|| candidates().next());
            found
                .filter(|(_, c)| {
                    let local = c.local_rec_id();
                    local == 0 || header_rec_id == 0 || header_rec_id == local
                })
                .map(|(&k, _)| k)
        });

        let Some(key) = target_key else {
            return Err(unknown);
        };

        // Take the connection out so the slot check can see every other connection; it
        // goes back under its handle either way.
        let mut conn = self.connections.remove(&key).ok_or(unknown)?;
        let result = {
            let others = &self.connections;
            conn.receive_datagram_guarded(datagram, source_addr, now_sec, codecs, &|id| {
                others.values().any(|c| c.local_rec_id() == id)
            })
        };
        if conn.local_rec_id() != 0 {
            self.retired.remove(&conn.local_rec_id());
        }
        self.connections.insert(key, conn);
        result.map(|pkt| (key, pkt))
    }

    /// Runs [`SessionConnection::tick`] on every connection and then removes (as
    /// [`Self::remove_connection`] does) those that have been in State `8` (`Closing`) for
    /// `20.0` s (§6.1). Returns the IDs removed.
    ///
    /// # Errors
    ///
    /// Returns the first [`SessionError`] a connection's tick reports; the other
    /// connections are still ticked.
    pub fn tick(&mut self, now_sec: f64) -> Result<Vec<u16>, SessionError> {
        self.tick_with_codec(now_sec, None)
    }

    /// [`Self::tick`] with [`SessionConnection::tick_with_codec`]: sequenced
    /// FlowControl-only packets are compressed with `outgoing_codec`.
    ///
    /// # Errors
    ///
    /// As [`Self::tick`].
    pub fn tick_with_codec(
        &mut self,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u16>, SessionError> {
        let mut first_err = None;
        for conn in self.connections.values_mut() {
            if let Err(err) = conn.tick_with_codec(now_sec, outgoing_codec) {
                first_err.get_or_insert(err);
            }
        }
        let removed = self.reap_closed(now_sec);
        match first_err {
            Some(err) => Err(err),
            None => Ok(removed),
        }
    }

    /// Removes the connections that have been in State `8` (`Closing`) for at least
    /// `20.0` s (§6.1 "cleanup after 20.0 s"), returning their IDs.
    pub fn reap_closed(&mut self, now_sec: f64) -> Vec<u16> {
        let done: Vec<u16> = self
            .connections
            .iter()
            .filter(|(_, c)| c.closed_for_cleanup(now_sec))
            .map(|(&k, _)| k)
            .collect();
        for &id in &done {
            self.remove_connection(id);
        }
        done
    }

    /// The earliest timer of its connections ([`SessionConnection::next_deadline`]: the
    /// retransmits, keep-alives, acknowledgements, handshake and silence timeouts, the
    /// disconnect and the cleanup of State `8`, §5–§8), on the clock of `now_sec`: the time
    /// at which [`Self::tick`] next has work. A timer already due gives `now_sec`; `None`
    /// when no connection has a timer (nothing happens until a datagram or a call).
    ///
    /// Datagrams waiting in an outbound queue are not a deadline: the pass that queues them
    /// also flushes them ([`Self::flush_outbound`]), and when a send fails the rest of the
    /// queue stays. A transport that keeps failing (a revoked right fails every send) would
    /// make "outbound not empty" due at every pass, so the caller would wake every 1 ms for
    /// as long as the error lasts; instead the next timer, or the caller's longest wait,
    /// tries the queue again.
    #[must_use]
    pub fn next_deadline(&self, now_sec: f64) -> Option<f64> {
        self.connections
            .values()
            .filter_map(SessionConnection::next_deadline)
            .reduce(f64::min)
            .map(|t| t.max(now_sec))
    }

    /// Flushes all queued outbound datagrams across all connections to `transport`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the underlying transport fails.
    pub fn flush_outbound<T: UdpDatagramTransport>(
        &mut self,
        transport: &mut T,
    ) -> Result<usize, SessionError> {
        let mut count = 0usize;
        for conn in self.connections.values_mut() {
            let remote = conn.remote_addr();
            while let Some(mut dgram) = conn.pop_outbound_datagram() {
                let sent = transport.send_to(&dgram, remote);
                // A sent (or failed) datagram may hold the ticket (`official-login.md` §6.3).
                crate::login::zeroize_vec(&mut dgram);
                if sent.is_err() {
                    conn.counts.send_failures = conn.counts.send_failures.saturating_add(1);
                }
                sent?;
                conn.counts.sent = conn.counts.sent.saturating_add(1);
                count += 1;
            }
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// §9.1–§9.4 Upper-Layer MessageEnvelope Envelope, Routing Header, Session Control Messages, and Batch Containers
// ---------------------------------------------------------------------------

/// `envelopeFlags` bit 0 (`0x01`): `MessageEnvelope` body is zlib-compressed preceded by `u32` LE uncompressed length (§9.1).
pub const ENVELOPE_FLAG_ZLIB_COMPRESSED: u8 = 0x01;
/// `envelopeFlags` bit 1 (`0x02`): `MessageEnvelope` carries an aligned optional routing header before Offset `0x00` (§9.1).
pub const ENVELOPE_FLAG_HAS_ROUTING_HEADER: u8 = 0x02;

/// Optional routing header present when `(envelopeFlags & 0x02) != 0` (§9.1 item 3):
/// `routingFlags` (`u16` LE), then `routingField0..3` as flagged, padded to a multiple of 4
/// bytes from the start of `routingFlags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MessageRoutingHeader {
    /// Bit 0 (`0x0001`): `routingField0` (`u32` LE).
    pub field0: Option<u32>,
    /// Bit 1 (`0x0002`): `routingField1` (`u32` LE).
    pub field1: Option<u32>,
    /// Bit 2 (`0x0004`): `routingField2` (`u64` LE).
    pub field2: Option<u64>,
    /// Bit 3 (`0x0008`): `routingField3` (`u32` LE).
    pub field3: Option<u32>,
}

impl MessageRoutingHeader {
    /// Returns the 16-bit `routingFlags` bitmask (`0x0000..=0x000F`) for this routing header (§9.1).
    #[must_use]
    pub fn routing_flags(&self) -> u16 {
        let mut flags = 0u16;
        if self.field0.is_some() {
            flags |= 0x0001;
        }
        if self.field1.is_some() {
            flags |= 0x0002;
        }
        if self.field2.is_some() {
            flags |= 0x0004;
        }
        if self.field3.is_some() {
            flags |= 0x0008;
        }
        flags
    }

    /// Encodes this routing header into `out`, including the mandatory 2-byte alignment pad (`(relOffset + 3) & ~3`, §9.1).
    pub fn encode(&self, out: &mut Vec<u8>) -> usize {
        let start = out.len();
        out.extend_from_slice(&self.routing_flags().to_le_bytes());
        if let Some(f0) = self.field0 {
            out.extend_from_slice(&f0.to_le_bytes());
        }
        if let Some(f1) = self.field1 {
            out.extend_from_slice(&f1.to_le_bytes());
        }
        if let Some(f2) = self.field2 {
            out.extend_from_slice(&f2.to_le_bytes());
        }
        if let Some(f3) = self.field3 {
            out.extend_from_slice(&f3.to_le_bytes());
        }
        let rel = out.len() - start;
        let aligned = (rel + 3) & !3;
        out.resize(start + aligned, 0x00);
        aligned
    }

    /// Decodes a `MessageRoutingHeader` from the start of `bytes` (immediately after `envelopeFlags`, §9.1),
    /// returning `(header, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidRoutingHeader`] if `bytes` is truncated, `routingFlags` has bits outside `0x000F`,
    /// or the 2-byte alignment pad is missing.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        if bytes.len() < 2 {
            return Err(SessionError::InvalidRoutingHeader);
        }
        let flags = u16::from_le_bytes([bytes[0], bytes[1]]);
        // §9.1 defines bits 0..3; a field behind another bit has no known size, so the
        // header is refused (engineering choice, spec silent). The two pad bytes are skipped
        // without a value check (§9.1 calls them zero but gives no receiver rule).
        if (flags & !0x000F) != 0 {
            return Err(SessionError::InvalidRoutingHeader);
        }
        let mut cursor = 2usize;
        let field0 = if (flags & 0x0001) != 0 {
            if bytes.len() < cursor + 4 {
                return Err(SessionError::InvalidRoutingHeader);
            }
            let v = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            cursor += 4;
            Some(v)
        } else {
            None
        };
        let field1 = if (flags & 0x0002) != 0 {
            if bytes.len() < cursor + 4 {
                return Err(SessionError::InvalidRoutingHeader);
            }
            let v = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            cursor += 4;
            Some(v)
        } else {
            None
        };
        let field2 = if (flags & 0x0004) != 0 {
            if bytes.len() < cursor + 8 {
                return Err(SessionError::InvalidRoutingHeader);
            }
            let v = u64::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
            cursor += 8;
            Some(v)
        } else {
            None
        };
        let field3 = if (flags & 0x0008) != 0 {
            if bytes.len() < cursor + 4 {
                return Err(SessionError::InvalidRoutingHeader);
            }
            let v = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            cursor += 4;
            Some(v)
        } else {
            None
        };

        let aligned = (cursor + 3) & !3;
        if bytes.len() < aligned {
            return Err(SessionError::InvalidRoutingHeader);
        }
        Ok((
            Self {
                field0,
                field1,
                field2,
                field3,
            },
            aligned,
        ))
    }
}

/// Decoded upper-layer message envelope (§9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEnvelope {
    /// 32-bit message opcode (`blobHeaderWord`, §9.1).
    pub opcode: u32,
    /// Envelope flags byte as received (`envelopeFlags`: bit 0 = zlib compressed, bit 1 = has
    /// routing header).
    pub envelope_flags: u8,
    /// Optional routing header decoded when `(envelopeFlags & 0x02) != 0` (§9.1).
    pub routing_header: Option<MessageRoutingHeader>,
    /// Inner message payload bytes starting at Offset `0x00` ready for the opcode's deserializer (§9.1 item 4).
    pub body: Vec<u8>,
    /// `contextId` and `resultCode` of the nearest `EnvelopeContextWrapper` (`0x00000179`)
    /// this message was unwrapped from by [`dispatch_message`] (§9.4.2); `None` otherwise.
    pub wrapper_context: Option<EnvelopeContext>,
}

/// The trailing fields of an `EnvelopeContextWrapper` (`0x00000179`, §9.4.2 items 4–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvelopeContext {
    /// `contextId` (`u64` LE).
    pub context_id: u64,
    /// `resultCode` (`u32` LE).
    pub result_code: u32,
}

/// Serializes an upper-layer message envelope payload (`envelopeFlags` + optional routing header +
/// uncompressed or zlib-compressed body prefixed by `u32` LE uncompressed length, §9.1).
///
/// # Errors
///
/// Returns [`SessionError::InvalidEnvelopeFlags`] if `(envelope_flags & 0xFC) != 0` or
/// [`SessionError::ZlibDecompressionFailed`] if zlib compression fails.
pub fn encode_message_with_routing(
    mut envelope_flags: u8,
    routing_header: Option<&MessageRoutingHeader>,
    body: &[u8],
    compress_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    if (envelope_flags & 0xFC) != 0 {
        return Err(SessionError::InvalidEnvelopeFlags {
            flags: envelope_flags,
        });
    }

    let mut plain_after_flags = Vec::new();
    if let Some(rh) = routing_header {
        envelope_flags |= ENVELOPE_FLAG_HAS_ROUTING_HEADER;
        rh.encode(&mut plain_after_flags);
    } else if (envelope_flags & ENVELOPE_FLAG_HAS_ROUTING_HEADER) != 0 {
        MessageRoutingHeader::default().encode(&mut plain_after_flags);
    }
    plain_after_flags.extend_from_slice(body);

    if compress_zlib {
        envelope_flags |= ENVELOPE_FLAG_ZLIB_COMPRESSED;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, &plain_after_flags)
            .map_err(|_| SessionError::ZlibDecompressionFailed)?;
        let compressed = encoder
            .finish()
            .map_err(|_| SessionError::ZlibDecompressionFailed)?;
        let mut out = Vec::with_capacity(1 + 4 + compressed.len());
        out.push(envelope_flags);
        out.extend_from_slice(&(plain_after_flags.len() as u32).to_le_bytes());
        out.extend_from_slice(&compressed);
        Ok(out)
    } else {
        envelope_flags &= !ENVELOPE_FLAG_ZLIB_COMPRESSED;
        let mut out = Vec::with_capacity(1 + plain_after_flags.len());
        out.push(envelope_flags);
        out.extend_from_slice(&plain_after_flags);
        Ok(out)
    }
}

/// Serializes an upper-layer message envelope payload (`envelopeFlags` + uncompressed or
/// zlib-compressed body prefixed by `u32` LE uncompressed length, §9.1).
///
/// # Errors
///
/// Returns [`SessionError`] if `envelope_flags` has reserved bits or zlib compression fails.
pub fn encode_message(
    envelope_flags: u8,
    body: &[u8],
    compress_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    encode_message_with_routing(envelope_flags, None, body, compress_zlib)
}

/// Decodes a `MessageEnvelope` from a reassembled fragmented blob payload, performing bounded
/// zlib decompression when `(envelopeFlags & 0x01) != 0` and parsing the optional routing header
/// when `(envelopeFlags & 0x02) != 0` (§9.1, §10.1 rule 9).
///
/// # Errors
///
/// Returns [`SessionError`] if the envelope is empty, `(envelopeFlags & 0xFC) != 0`,
/// `uncompressedLen` exceeds `max_decompressed_bytes` (or `MAX_ZLIB_UNCOMPRESSED_BYTES`),
/// zlib decompression fails, or the routing header is malformed.
pub fn decode_message(
    opcode: u32,
    blob_payload: &[u8],
    max_decompressed_bytes: usize,
) -> Result<MessageEnvelope, SessionError> {
    let Some((&raw_flags, rest)) = blob_payload.split_first() else {
        return Err(SessionError::EmptyMessageEnvelope);
    };
    if (raw_flags & 0xFC) != 0 {
        return Err(SessionError::InvalidEnvelopeFlags { flags: raw_flags });
    }
    let effective_limit = max_decompressed_bytes.min(MAX_ZLIB_UNCOMPRESSED_BYTES);
    let envelope_flags = raw_flags;

    let decompressed_buf: Vec<u8>;
    let after_flags: &[u8] = if (raw_flags & ENVELOPE_FLAG_ZLIB_COMPRESSED) == 0 {
        if rest.len() > effective_limit {
            return Err(SessionError::DecompressedMessageTooLarge {
                size: rest.len(),
                limit: effective_limit,
            });
        }
        rest
    } else {
        if rest.len() < 4 {
            return Err(SessionError::ZlibDecompressionFailed);
        }
        let declared_len = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        if declared_len > effective_limit {
            return Err(SessionError::DecompressedMessageTooLarge {
                size: declared_len,
                limit: effective_limit,
            });
        }
        let z_stream = &rest[4..];
        let decoder = ZlibDecoder::new(z_stream);
        let mut limited = decoder.take((effective_limit as u64).saturating_add(1));
        let mut out = Vec::with_capacity(declared_len.min(65536));
        limited
            .read_to_end(&mut out)
            .map_err(|_| SessionError::ZlibDecompressionFailed)?;
        // §9.3.8: the stream finishes exactly at the end of its slice; trailing bytes and
        // concatenated streams fail.
        if limited.get_ref().total_in() != z_stream.len() as u64 {
            return Err(SessionError::ZlibDecompressionFailed);
        }
        if out.len() > effective_limit {
            return Err(SessionError::DecompressedMessageTooLarge {
                size: out.len(),
                limit: effective_limit,
            });
        }
        if out.len() != declared_len {
            return Err(SessionError::ZlibDecompressionFailed);
        }
        decompressed_buf = out;
        &decompressed_buf
    };

    let (routing_header, body_start) = if (raw_flags & ENVELOPE_FLAG_HAS_ROUTING_HEADER) != 0 {
        let (rh, consumed) = MessageRoutingHeader::decode(after_flags)?;
        (Some(rh), consumed)
    } else {
        (None, 0)
    };

    Ok(MessageEnvelope {
        opcode,
        envelope_flags,
        routing_header,
        body: after_flags.get(body_start..).unwrap_or_default().to_vec(),
        wrapper_context: None,
    })
}

// ---------------------------------------------------------------------------
// §9.3 Post-Handshake Session/Version Negotiation Messages (0x0000012C..0x00000130)
// ---------------------------------------------------------------------------

fn encode_pstring_ascii(s: &str, out: &mut Vec<u8>) -> Result<(), SessionError> {
    let len = u32::try_from(s.len()).map_err(|_| SessionError::CountOverflow { count: s.len() })?;
    encode_variable_u32(len, out)?;
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

fn decode_pstring_ascii(bytes: &[u8], opcode: u32) -> Result<(String, usize), SessionError> {
    let (len_u32, pd_len) =
        decode_variable_u32(bytes).map_err(|_| SessionError::InvalidControlMessage { opcode })?;
    let len = len_u32 as usize;
    let total = pd_len
        .checked_add(len)
        .ok_or(SessionError::InvalidControlMessage { opcode })?;
    if bytes.len() < total {
        return Err(SessionError::InvalidControlMessage { opcode });
    }
    let raw = &bytes[pd_len..total];
    if !raw.is_ascii() {
        return Err(SessionError::InvalidControlMessage { opcode });
    }
    let s = std::str::from_utf8(raw)
        .map_err(|_| SessionError::InvalidControlMessage { opcode })?
        .to_owned();
    Ok((s, total))
}

/// `ClientVersionMessage` (`0x0000012C`, Client -> Server) post-handshake message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientVersionMessage {
    /// `versionString`: `"netver:<netver_string>; didver:<didver_string>"` (`VariableU32`
    /// byte length + ASCII bytes, no NUL; `official-login.md` §2.4.1–§2.4.2).
    pub version_string: String,
}

impl ClientVersionMessage {
    /// Serializes `ClientVersionMessage` (`0x0000012C`) inner payload starting at Offset `0x00`
    /// (§9.3) with [`crate::login::encode_client_version_payload`], the one encoder of the
    /// version string (`official-login.md` §2.4.2).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if the string is not ASCII or too long.
    pub fn encode(&self) -> Result<Vec<u8>, SessionError> {
        crate::login::encode_client_version_payload(&self.version_string).map_err(|_| {
            SessionError::InvalidControlMessage {
                opcode: OPCODE_CLIENT_VERSION,
            }
        })
    }

    /// Decodes `ClientVersionMessage` (`0x0000012C`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if the string is truncated or not ASCII.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        let (version_string, _) = decode_pstring_ascii(body, OPCODE_CLIENT_VERSION)?;
        Ok(Self { version_string })
    }
}

/// `VersionStatusMessage` (`0x0000012D`, Server -> Client) post-handshake message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionStatusMessage {
    /// `status` (`u32` LE): `1` = accepted, `0` = rejected (the connection closes locally).
    pub status: u32,
    /// `serverVersionInfo` (`VariableU32` byte length + ASCII bytes).
    pub server_version_info: String,
}

impl VersionStatusMessage {
    /// Serializes `VersionStatusMessage` (`0x0000012D`) inner payload starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// [`SessionError::VariableU32OutOfRange`] for a text longer than the compact
    /// integer holds (§5.6.1).
    pub fn encode(&self) -> Result<Vec<u8>, SessionError> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.status.to_le_bytes());
        encode_pstring_ascii(&self.server_version_info, &mut out)?;
        Ok(out)
    }

    /// Decodes `VersionStatusMessage` (`0x0000012D`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `body` is shorter than 5 bytes or `serverVersionInfo` is malformed.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        if body.len() < 4 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_VERSION_STATUS,
            });
        }
        let status = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let (server_version_info, _) = decode_pstring_ascii(&body[4..], OPCODE_VERSION_STATUS)?;
        Ok(Self {
            status,
            server_version_info,
        })
    }

    /// Verifies that `status == 1`, returning [`SessionError::VersionRejected`] otherwise (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::VersionRejected`] when `self.status == 0`.
    pub fn ensure_accepted(&self) -> Result<(), SessionError> {
        if self.status == 1 {
            Ok(())
        } else {
            Err(SessionError::VersionRejected {
                server_version_info: self.server_version_info.clone(),
            })
        }
    }
}

/// `logonResult` of a `LoginQueueStatusMessage`: logon accepted (`S_OK`, §9.3).
pub const LOGON_RESULT_ACCEPTED: u32 = 0x0000_0000;

/// `logonResult` of a `LoginQueueStatusMessage`: the account waits in the UDP login queue;
/// only then do `queuePosition` and `queueTotal` follow `detailText` (§9.3).
pub const LOGON_RESULT_QUEUED: u32 = 0x8004_1464;

/// The place in the UDP login queue a `LoginQueueStatusMessage` reports (§9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginQueuePosition {
    /// `queuePosition` (`u32` LE).
    pub position: u32,
    /// `queueTotal` (`u32` LE).
    pub total: u32,
}

/// What a `LoginQueueStatusMessage`'s `logonResult` means (§9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogonOutcome {
    /// `0x00000000`: logon accepted.
    Accepted,
    /// `0x80041464`: waiting in the UDP login queue.
    Queued(LoginQueuePosition),
    /// Any other negative value (bit 31 set): logon rejected; the client ends the session.
    Refused(u32),
    /// A positive value other than `0`: §9.3 names none; Tessera acts on nothing
    /// (engineering choice, spec silent).
    Unknown(u32),
}

/// `LoginQueueStatusMessage` (`0x00000130`, Server -> Client) post-handshake message,
/// optional and omitted when no login queue is active (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginQueueStatusMessage {
    /// `logonResult` (`u32` LE).
    pub login_result: u32,
    /// `detailText` (4-tier `VariableU32` UTF-16 code-unit count + UTF-16LE).
    pub detail_text: String,
    /// `queuePosition` and `queueTotal` (`u32` LE each), present only when `login_result`
    /// is [`LOGON_RESULT_QUEUED`].
    pub queue: Option<LoginQueuePosition>,
}

impl LoginQueueStatusMessage {
    /// What `logonResult` means (§9.3). A queued result without its position (§9.3.8: all optional
    /// fields may be absent) counts as position and total `0`, which the driver reports as absent.
    #[must_use]
    pub fn outcome(&self) -> LogonOutcome {
        match self.login_result {
            LOGON_RESULT_ACCEPTED => LogonOutcome::Accepted,
            LOGON_RESULT_QUEUED => LogonOutcome::Queued(self.queue.unwrap_or(LoginQueuePosition {
                position: 0,
                total: 0,
            })),
            code if code & 0x8000_0000 != 0 => LogonOutcome::Refused(code),
            code => LogonOutcome::Unknown(code),
        }
    }

    /// Serializes `LoginQueueStatusMessage` (`0x00000130`) inner payload starting at Offset
    /// `0x00` (§9.3): `logonResult`, `detailText`, then `queuePosition` and `queueTotal` only
    /// when `queue` is set and `logonResult` is [`LOGON_RESULT_QUEUED`].
    ///
    /// # Errors
    ///
    /// [`SessionError::VariableU32OutOfRange`] for a text longer than the compact
    /// integer holds (§5.6.1).
    pub fn encode(&self) -> Result<Vec<u8>, SessionError> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.login_result.to_le_bytes());
        encode_wstring(&self.detail_text, &mut out)?;
        if let Some(q) = self
            .queue
            .filter(|_| self.login_result == LOGON_RESULT_QUEUED)
        {
            out.extend_from_slice(&q.position.to_le_bytes());
            out.extend_from_slice(&q.total.to_le_bytes());
        }
        Ok(out)
    }

    /// Decodes `LoginQueueStatusMessage` (`0x00000130`) from inner payload bytes starting at
    /// Offset `0x00` (§9.3). `queuePosition` and `queueTotal` are read only when
    /// `logonResult` is [`LOGON_RESULT_QUEUED`]; bytes after the last field are not read
    /// (engineering choice, spec silent). A queued result may end right after its text: §9.3.8
    /// emits only the fields actually available, and all optional fields may be absent. A part of
    /// the two words is truncated, not absent.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `body` is truncated or `detail_text` is invalid UTF-16LE.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        let bad = || SessionError::InvalidControlMessage {
            opcode: OPCODE_LOGIN_QUEUE_STATUS,
        };
        let result: [u8; 4] = body
            .get(0..4)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(bad)?;
        let login_result = u32::from_le_bytes(result);
        let (detail_text, ws_len) =
            decode_wstring(body.get(4..).unwrap_or_default()).map_err(|_| bad())?;
        let pos = 4 + ws_len;
        let queue = if login_result == LOGON_RESULT_QUEUED && body.len() > pos {
            let word = |at: usize| {
                body.get(at..at + 4)
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_le_bytes)
                    .ok_or_else(bad)
            };
            Some(LoginQueuePosition {
                position: word(pos)?,
                total: word(pos + 4)?,
            })
        } else {
            None
        };
        Ok(Self {
            login_result,
            detail_text,
            queue,
        })
    }
}

/// `SystemNotificationMessage` (`0x0000012E`, Server -> Client) system notification message (§9.3).
///
/// A server notice such as a shutdown warning. Tessera decodes it and shows
/// its text as a system line; it acts on nothing and is never encoded or sent
/// (decision 0042 item 2, clarification), so this type has no encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNotificationMessage {
    /// `notificationText` (4-tier `VariableU32` UTF-16 code-unit count + UTF-16LE).
    pub text: String,
}

impl SystemNotificationMessage {
    /// Decodes `SystemNotificationMessage` (`0x0000012E`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `text` is truncated or invalid UTF-16LE.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        let (text, _) = decode_wstring(body).map_err(|_| SessionError::InvalidControlMessage {
            opcode: OPCODE_SYSTEM_NOTIFICATION,
        })?;
        Ok(Self { text })
    }
}

/// Parsed post-handshake session/version negotiation message (`0x0000012C`..`0x00000130`, §9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    /// `0x0000012F` (`VersionRequestMessage`, Server -> Client, 0 bytes).
    VersionRequest,
    /// `0x0000012C` (`ClientVersionMessage`, Client -> Server).
    ClientVersion(ClientVersionMessage),
    /// `0x0000012D` (`VersionStatusMessage`, Server -> Client).
    VersionStatus(VersionStatusMessage),
    /// `0x00000130` (`LoginQueueStatusMessage`, Server -> Client).
    LoginQueueStatus(LoginQueueStatusMessage),
    /// `0x0000012E` (`SystemNotificationMessage`, Server -> Client; decoded and
    /// shown, never sent).
    SystemNotification(SystemNotificationMessage),
}

impl ControlMessage {
    /// Encodes this control message into `(opcode, inner_body_bytes)` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::ReceiveOnlyMessage`] for
    /// [`ControlMessage::SystemNotification`], which Tessera never sends
    /// (decision 0042 item 2).
    pub fn encode(&self) -> Result<(u32, Vec<u8>), SessionError> {
        Ok(match self {
            Self::VersionRequest => (OPCODE_VERSION_REQUEST, Vec::new()),
            Self::ClientVersion(m) => (OPCODE_CLIENT_VERSION, m.encode()?),
            Self::VersionStatus(m) => (OPCODE_VERSION_STATUS, m.encode()?),
            Self::LoginQueueStatus(m) => (OPCODE_LOGIN_QUEUE_STATUS, m.encode()?),
            Self::SystemNotification(_) => {
                return Err(SessionError::ReceiveOnlyMessage {
                    opcode: OPCODE_SYSTEM_NOTIFICATION,
                });
            }
        })
    }

    /// Decodes a `ControlMessage` from `(opcode, inner_body_bytes)` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `opcode` is not in `0x0000012C..=0x00000130`
    /// or `body` is malformed.
    pub fn decode(opcode: u32, body: &[u8]) -> Result<Self, SessionError> {
        match opcode {
            // §9.3: 0x12F is read as empty. The first server blobs of a login connection
            // are other queue-1 opcodes (0x149, 0xAA, §6.4.1), which callers record or skip;
            // 0x12C answers only a 0x12F, never the entry to State 5 (OPEN-SESSION-15).
            OPCODE_VERSION_REQUEST => Ok(Self::VersionRequest),
            OPCODE_CLIENT_VERSION => Ok(Self::ClientVersion(ClientVersionMessage::decode(body)?)),
            OPCODE_VERSION_STATUS => Ok(Self::VersionStatus(VersionStatusMessage::decode(body)?)),
            OPCODE_LOGIN_QUEUE_STATUS => Ok(Self::LoginQueueStatus(
                LoginQueueStatusMessage::decode(body)?,
            )),
            OPCODE_SYSTEM_NOTIFICATION => Ok(Self::SystemNotification(
                SystemNotificationMessage::decode(body)?,
            )),
            _ => Err(SessionError::InvalidControlMessage { opcode }),
        }
    }
}

// ---------------------------------------------------------------------------
// §9.4.2 MessageEnvelope Containers (0x00000186 and 0x00000179, primary login/world connection)
// ---------------------------------------------------------------------------

/// Serializes a batch container (`0x00000186`) envelope payload from `(sub_opcode, sub_envelope_payload)`
/// pairs (§9.4.2), optionally zlib-compressing the outer container envelope.
///
/// # Errors
///
/// Returns [`SessionError`] if outer envelope compression fails.
pub fn encode_batch_container(
    sub_envelopes: &[(u32, Vec<u8>)],
    compress_outer_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    let count_of =
        |n: usize| u32::try_from(n).map_err(|_| SessionError::CountOverflow { count: n });
    let mut body = Vec::new();
    encode_variable_u32(count_of(sub_envelopes.len())?, &mut body)?;
    for (sub_opcode, sub_payload) in sub_envelopes {
        encode_variable_u32(*sub_opcode, &mut body)?;
        body.extend_from_slice(&count_of(sub_payload.len())?.to_le_bytes());
        body.extend_from_slice(sub_payload);
    }
    encode_message(0, &body, compress_outer_zlib)
}

/// Serializes a single-message wrapper (`0x00000179`) envelope payload around `(sub_opcode, sub_envelope_payload)`
/// with explicit `context_id` (`u64` LE) and `result_code` (`u32` LE, §9.4.2).
///
/// # Errors
///
/// Returns [`SessionError`] if outer envelope compression fails.
pub fn encode_single_wrapper_with_context(
    sub_opcode: u32,
    sub_envelope_payload: &[u8],
    context_id: u64,
    result_code: u32,
    compress_outer_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    let len =
        u32::try_from(sub_envelope_payload.len()).map_err(|_| SessionError::CountOverflow {
            count: sub_envelope_payload.len(),
        })?;
    let mut body = Vec::new();
    encode_variable_u32(sub_opcode, &mut body)?;
    body.extend_from_slice(&len.to_le_bytes());
    body.extend_from_slice(sub_envelope_payload);
    body.extend_from_slice(&context_id.to_le_bytes());
    body.extend_from_slice(&result_code.to_le_bytes());
    encode_message(0, &body, compress_outer_zlib)
}

/// Serializes a single-message wrapper (`0x00000179`) envelope payload around `(sub_opcode, sub_envelope_payload)`
/// with default `context_id = 0` and `result_code = 0` (§9.4.2).
///
/// # Errors
///
/// Returns [`SessionError`] if outer envelope compression fails.
pub fn encode_single_wrapper(
    sub_opcode: u32,
    sub_envelope_payload: &[u8],
    compress_outer_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    encode_single_wrapper_with_context(sub_opcode, sub_envelope_payload, 0, 0, compress_outer_zlib)
}

/// Decodes a message envelope and recursively unpacks `EnvelopeBatchContainer`
/// (`0x00000186`) and `EnvelopeContextWrapper` (`0x00000179`) into leaf [`MessageEnvelope`]
/// items (§9.1, §9.4.2). This is the primary login/world connection's opcode space; on the
/// chat connection the same numbers are chat events (§9.4.2).
///
/// `max_decompressed_bytes` is a budget for the whole container tree, not per
/// message: the zlib output of every envelope in the tree is summed against
/// it, and at most [`MAX_UNPACKED_MESSAGES`] leaf messages are produced. This
/// stops small nested compressed sub-messages from expanding without bound.
/// Callers normally pass [`DEFAULT_MAX_CONTAINER_DEPTH`] (`8`) as `max_depth`.
///
/// A blob that arrived with `EncryptedFragmentedBlob` set never gets here: the reassembler
/// refuses it (§7.1, `OPEN-SESSION-6`).
///
/// # Errors
///
/// Returns [`SessionError`] if any envelope or container sub-message is malformed, overruns
/// remaining container bytes (`ContainerSubMessageOverrun`), exceeds `max_depth`
/// (clamped to [`MAX_CONTAINER_DEPTH`])
/// (`ContainerRecursionLimitExceeded`), exceeds the unpack budget
/// (`UnpackBudgetExceeded`).
pub fn dispatch_message(
    opcode: u32,
    blob_payload: &[u8],
    max_decompressed_bytes: usize,
    max_depth: usize,
) -> Result<Vec<MessageEnvelope>, SessionError> {
    let mut out = Vec::new();
    let max_decompressed_bytes = max_decompressed_bytes.min(MAX_ZLIB_UNCOMPRESSED_BYTES);
    let mut budget = UnpackBudget {
        byte_limit: max_decompressed_bytes,
        bytes_used: 0,
    };
    dispatch_message_inner(
        &mut UnpackContext {
            max_decompressed_bytes,
            max_depth: max_depth.min(MAX_CONTAINER_DEPTH),
            budget: &mut budget,
            out: &mut out,
        },
        opcode,
        blob_payload,
        0,
        None,
    )?;
    Ok(out)
}

/// Running totals of one [`dispatch_message`] call.
#[derive(Debug)]
struct UnpackBudget {
    byte_limit: usize,
    bytes_used: usize,
}

#[derive(Debug)]
struct UnpackContext<'a> {
    max_decompressed_bytes: usize,
    max_depth: usize,
    budget: &'a mut UnpackBudget,
    out: &'a mut Vec<MessageEnvelope>,
}

/// Reads one `(VariableU32 sub_opcode, u32 LE sub_len, sub_len bytes)` record
/// at `*cursor` of a container body.
fn read_container_record<'b>(
    bytes: &'b [u8],
    cursor: &mut usize,
) -> Result<(u32, &'b [u8]), SessionError> {
    let rest = bytes.get(*cursor..).unwrap_or_default();
    let (sub_opcode, n_op) = decode_variable_u32(rest)?;
    *cursor += n_op;
    let len_bytes: [u8; 4] = bytes
        .get(*cursor..cursor.saturating_add(4))
        .and_then(|b| b.try_into().ok())
        .ok_or(SessionError::ContainerSubMessageOverrun {
            sub_len: 4,
            remaining: bytes.len().saturating_sub(*cursor),
        })?;
    let sub_len = u32::from_le_bytes(len_bytes) as usize;
    *cursor += 4;
    let remaining = bytes.len().saturating_sub(*cursor);
    if sub_len > remaining {
        return Err(SessionError::ContainerSubMessageOverrun { sub_len, remaining });
    }
    let sub = bytes
        .get(*cursor..*cursor + sub_len)
        .ok_or(SessionError::ContainerSubMessageOverrun { sub_len, remaining })?;
    *cursor += sub_len;
    Ok((sub_opcode, sub))
}

fn dispatch_message_inner(
    ctx: &mut UnpackContext<'_>,
    opcode: u32,
    blob_payload: &[u8],
    depth: usize,
    wrapper_context: Option<EnvelopeContext>,
) -> Result<(), SessionError> {
    if depth > ctx.max_depth {
        return Err(SessionError::ContainerRecursionLimitExceeded {
            max_depth: ctx.max_depth,
        });
    }
    // A compressed envelope may only inflate into what is left of the tree's
    // budget, so peak memory stays within it.
    let compressed = blob_payload
        .first()
        .is_some_and(|&flags| flags & ENVELOPE_FLAG_ZLIB_COMPRESSED != 0);
    let remaining = ctx.budget.byte_limit.saturating_sub(ctx.budget.bytes_used);
    let limit = if compressed {
        ctx.max_decompressed_bytes.min(remaining)
    } else {
        ctx.max_decompressed_bytes
    };
    let env = match decode_message(opcode, blob_payload, limit) {
        Err(SessionError::DecompressedMessageTooLarge { size, .. })
            if compressed && limit < ctx.max_decompressed_bytes =>
        {
            return Err(SessionError::UnpackBudgetExceeded {
                what: "bytes",
                actual: ctx.budget.bytes_used.saturating_add(size),
                limit: ctx.budget.byte_limit,
            });
        }
        other => other?,
    };
    if compressed {
        // The inflated length was checked against `limit` (<= `remaining`).
        let inflated = blob_payload
            .get(1..5)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map_or(0, |b| u32::from_le_bytes(b) as usize);
        ctx.budget.bytes_used = ctx.budget.bytes_used.saturating_add(inflated);
    }
    if opcode == OPCODE_BATCH_CONTAINER {
        let bytes = &env.body;
        let (count_u32, mut cursor) = decode_variable_u32(bytes)?;
        // Each record is at least a 1-byte opcode, a 4-byte length and a 1-byte envelope.
        let remaining = bytes.len().saturating_sub(cursor);
        if (count_u32 as usize).saturating_mul(6) > remaining {
            return Err(SessionError::ContainerSubMessageOverrun {
                sub_len: (count_u32 as usize).saturating_mul(6),
                remaining,
            });
        }
        for _ in 0..count_u32 {
            let (sub_opcode, sub_slice) = read_container_record(bytes, &mut cursor)?;
            dispatch_message_inner(ctx, sub_opcode, sub_slice, depth + 1, wrapper_context)?;
        }
        // The reader stops after the `M` records and ignores any trailing bytes (§9.4.2).
    } else if opcode == OPCODE_SINGLE_WRAPPER {
        let bytes = &env.body;
        let mut cursor = 0usize;
        let (sub_opcode, sub_slice) = read_container_record(bytes, &mut cursor)?;
        // `contextId` (`u64` LE) and `resultCode` (`u32` LE) follow the inner payload and end
        // the message (§9.4.2 items 4–5).
        let (Some(id_bytes), Some(code_bytes)) = (
            bytes
                .get(cursor..cursor.saturating_add(8))
                .and_then(|b| <[u8; 8]>::try_from(b).ok()),
            bytes
                .get(cursor.saturating_add(8)..cursor.saturating_add(12))
                .and_then(|b| <[u8; 4]>::try_from(b).ok()),
        ) else {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_SINGLE_WRAPPER,
            });
        };
        // The reader stops after `resultCode` and ignores any trailing bytes (§9.4.2).
        let context = EnvelopeContext {
            context_id: u64::from_le_bytes(id_bytes),
            result_code: u32::from_le_bytes(code_bytes),
        };
        dispatch_message_inner(ctx, sub_opcode, sub_slice, depth + 1, Some(context))?;
    } else {
        if ctx.out.len() >= MAX_UNPACKED_MESSAGES {
            return Err(SessionError::UnpackBudgetExceeded {
                what: "messages",
                actual: ctx.out.len() + 1,
                limit: MAX_UNPACKED_MESSAGES,
            });
        }
        let mut env = env;
        env.wrapper_context = wrapper_context;
        ctx.out.push(env);
    }

    Ok(())
}

#[cfg(test)]
mod flow_report_tests {
    use super::{FLOW_REPORT_ONLY_PERIOD_SEC, FlowReport};

    /// §9.3.12: the u32 byte counter wraps modulo 2³²; it does not saturate.
    #[test]
    fn the_byte_counter_wraps() {
        let mut flow = FlowReport::new(0.0);
        flow.count(usize::try_from(u32::MAX).unwrap_or(usize::MAX), false);
        flow.count(10, true);
        assert_eq!(flow.header(0.5).bytes_received_delta, 9);
    }

    /// §9.3.12: an attached report waits one second after the last report sent, standalone or
    /// attached, and only with eligible traffic.
    #[test]
    fn an_attached_report_obeys_the_cap() {
        let mut flow = FlowReport::new(0.0);
        assert!(!flow.attach_due(5.0), "the first report is standalone");
        flow.note_flow_only(0.0);
        flow.count(20, false);
        assert!(!flow.attach_due(5.0), "pure acknowledgements make none due");
        flow.count(20, true);
        assert!(!flow.attach_due(FLOW_REPORT_ONLY_PERIOD_SEC - 0.01));
        assert!(flow.attach_due(FLOW_REPORT_ONLY_PERIOD_SEC));
        flow.note_report(1.5);
        flow.count(20, true);
        assert!(!flow.attach_due(2.0));
        assert_eq!(
            flow.flow_only_due_at(),
            Some(1.5 + FLOW_REPORT_ONLY_PERIOD_SEC)
        );
    }
}
