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
//! - The 9-state connection state machine (`0..8`), the handshake with its
//!   `checkValue` derivation, and the world referral hand-off (§6).
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

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

// ---------------------------------------------------------------------------
// §3.5 Datagram and Fragment Size Constants
// ---------------------------------------------------------------------------

/// Maximum total wire datagram size (`0xFF7C` = `65,404` B: `2` B `wirePrefix` + `20` B
/// `DatagramHeader` + `pakSize` payload, §3.5).
pub const MAX_DATAGRAM_BUF: usize = 0xFF7C;

/// Minimum valid UDP datagram length (`2` B wire prefix + `20` B `DatagramHeader` = `22` B, §3.5).
pub const MIN_DATAGRAM_BYTES: usize = 0x0016;

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

/// Allowed wire bitmask for `DatagramHeader.flags` (`0x0F7FFB00`, §4.3, §10.1 rule 3).
pub const VALID_DATAGRAM_FLAGS_MASK: u32 = 0x0F7F_FB00;

/// Disallowed wire bitmask for `DatagramHeader.flags` (`0xF08004FF`, §4.3, §10.1 rule 3).
pub const INVALID_DATAGRAM_FLAGS_MASK: u32 = !VALID_DATAGRAM_FLAGS_MASK;

/// Defined flag bits that carry no optional header bytes on the wire: bit 25
/// (`MaskedChecksumFlag`) and bit 26 (`HasFragmentedBlobsFlag`) (§4.3).
pub const HEADERLESS_FLAGS_MASK: u32 = FLAG_MASKED_CHECKSUM | FLAG_HAS_FRAGMENTED_BLOBS;

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

/// Minimum assigned local recipient ID (`0x0100`, §3.4).
pub const MIN_LOCAL_REC_ID: u16 = 0x0100;

/// Maximum assigned local recipient ID (`0xFEFF`, §3.4).
pub const MAX_LOCAL_REC_ID: u16 = 0xFEFF;

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
/// `MaskedChecksumFlag` (`0x02000000`, bit 25): sequenced packet whose payload checksum is
/// ISAAC-masked; carries no optional header bytes (§4.3).
pub const FLAG_MASKED_CHECKSUM: u32 = 0x0200_0000;
/// `HasFragmentedBlobsFlag` (`0x04000000`, bit 26): `FragmentedBlob` fragments follow the
/// optional headers; carries no optional header bytes (§4.3).
pub const FLAG_HAS_FRAGMENTED_BLOBS: u32 = 0x0400_0000;
/// `FlowControlHeader` optional header flag (`0x08000000`, bit 27).
pub const FLAG_FLOW_CONTROL: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors returned by the official-server session and transport layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SessionError {
    /// Datagram is shorter than `MIN_DATAGRAM_BYTES` (22 bytes, §10.1 rule 1).
    #[error("datagram length {len} is shorter than minimum {MIN_DATAGRAM_BYTES} bytes")]
    DatagramTooShort {
        /// Actual length in bytes.
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
    /// `DatagramHeader.flags` has bits outside `0x0F7FFB00` or forbidden server flags (§10.1 rules 3, 7).
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
    /// Source `(IPv4, port)` did not match the existing connection for `recId >= 0x0100` (§10.1 rule 4).
    #[error("source address {actual} does not match peer {expected} for recId {rec_id:#06x}")]
    SourceAddressMismatch {
        /// Recipient ID.
        rec_id: u16,
        /// Expected remote socket address.
        expected: SocketAddrV4,
        /// Actual datagram source address.
        actual: SocketAddrV4,
    },
    /// No active connection matched the incoming datagram (§3.4).
    #[error("no connection found for recId {rec_id:#06x} from {source_addr}")]
    UnknownRecipient {
        /// Recipient ID.
        rec_id: u16,
        /// Source socket address.
        source_addr: SocketAddrV4,
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
    /// `SessionAuthHeader` version prefix was not `"180804\0"` (§6.2).
    #[error("invalid SessionAuth protocol version string")]
    InvalidSessionAuthVersion,
    /// `SessionAuthHeader` authentication block length or alignment was invalid (§6.2).
    #[error("invalid SessionAuth authBlock length or structure")]
    InvalidAuthBlock,
    /// UTF-16LE string in `SessionAuthHeader` was truncated or invalid UTF-16 (§6.2.1).
    #[error("invalid UTF-16LE string in authentication block")]
    InvalidUtf16String,
    /// Variable-length `VariableU32` was truncated or had an invalid prefix (§5.6.1).
    #[error("truncated or invalid VariableU32 encoding")]
    InvalidVariableU32,
    /// Packet contained more than `20` (`0x14`) blob fragments (§5.6.3, §10.1 rule 8).
    #[error("packet fragment count {count} exceeds maximum {MAX_FRAGS_PER_PACKET}")]
    TooManyFragments {
        /// Fragment count.
        count: usize,
    },
    /// Blob fragment had an invalid `queueId` outside `1..=4` (§5.6.2).
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
    /// Blob fragment had `fragDataLen >= 0x01D1` (`465`, §5.6.3, §10.1 rule 8).
    #[error("Fragmented blob fragDataLen {len} is >= 0x01D1 (465)")]
    InvalidFragmentDataLen {
        /// Decoded `fragDataLen`.
        len: u32,
    },
    /// Multi-fragment blob slice would end past the `numFrags * 436` reassembly buffer (§5.6.4).
    #[error(
        "blob fragment {frag_index} with {len} bytes overruns the {buffer_len}-byte reassembly buffer"
    )]
    FragmentOverrunsReassembly {
        /// Decoded `fragIndex`.
        frag_index: u32,
        /// Decoded `fragDataLen`.
        len: u32,
        /// Reassembly buffer size in bytes (`numFrags * 436`).
        buffer_len: usize,
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
    /// session key is ever provisioned, so such a blob is always rejected (§7.1,
    /// `OPEN-SESSION-6`).
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
    /// A peer assigned a `localRecId` that another connection already uses.
    #[error("assigned recipient ID {rec_id:#06x} is already used by another connection")]
    RecipientIdCollision {
        /// The assigned recipient ID.
        rec_id: u16,
    },
    /// Every local recipient ID in `0x0100..=0xFEFF` is in use.
    #[error("no free local recipient ID")]
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
    /// `SessionAuthHeader` `(authType, authFlags)` is not one of the three modes of §6.2.1:
    /// `(0x40000002, 0x04)`, `(0x00000001, 0x00)` or `(0x00000001, 0x02)`.
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
    /// Bytes are left after the last field of a message (`0x00000179`: §9.4.2 lists the
    /// fields only; refusing bytes after `resultCode` is an engineering choice, spec silent).
    #[error("{remaining} trailing bytes after message {opcode:#010x}")]
    TrailingMessageBytes {
        /// Message opcode.
        opcode: u32,
        /// Unparsed byte count.
        remaining: usize,
    },
    /// A GLS ticket string is not valid Base64 (RFC 4648) or decodes to no bytes (§6.2.1).
    #[error("GLS ticket is not valid Base64")]
    InvalidTicketEncoding,
    /// Operation is invalid in the current connection state (§6.1).
    #[error("invalid operation in connection state {state:?}")]
    InvalidState {
        /// Current state.
        state: ConnectionState,
    },
    /// Underlying UDP socket I/O error.
    #[error("UDP socket I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for SessionError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// §3.1–§3.2 Local Port Selection & Transport
// ---------------------------------------------------------------------------

/// The value `ip_be` of §3.2 Case B: the four network-order address bytes read as a
/// host (little-endian) `u32`, so `127.0.0.1` gives `0x0100007F` (§3.2 item 1).
#[must_use]
pub fn unique_port_ip_value(ip: Ipv4Addr) -> u32 {
    u32::from_le_bytes(ip.octets())
}

/// The §3.2 Case B candidate port for attempt index `k`:
/// `candidate_port = (ip_be mod 0xFA07) + 256 * k`. Returns `None` when the candidate is
/// privileged (`<= 0x03FF`; the spec moves on to the next `k`) or, for `k` near `11`, above
/// `0xFFFF`. The spec gives no rule for a candidate above `0xFFFF`; Tessera skips it
/// rather than truncate it (engineering choice, spec silent).
#[must_use]
pub fn compute_unique_port(ip_be: u32, k: u32) -> Option<u16> {
    let candidate = u64::from(ip_be % 0xFA07) + 256 * u64::from(k);
    if candidate <= 0x03FF {
        return None;
    }
    u16::try_from(candidate).ok()
}

/// Returns the up to 12 candidate ports for local socket binding (§3.2 Cases A, B, C).
#[must_use]
pub fn candidate_bind_ports(
    local_ip: Ipv4Addr,
    explicit_base_port: u16,
    compute_unique: bool,
) -> Vec<u16> {
    if explicit_base_port != 0 {
        // Case A: explicit base port + k for k = 0..11
        // (a base port near 0xFFFF gives fewer candidates instead of wrapping to 0:
        // engineering choice, spec silent)
        (0..12u16)
            .filter_map(|k| explicit_base_port.checked_add(k))
            .collect()
    } else if compute_unique && !local_ip.is_unspecified() {
        // Case B: the candidates for k = 0, 1, ... while k < 12; privileged (and, by an
        // engineering choice, out-of-range) candidates use up their k and are skipped.
        let ip_be = unique_port_ip_value(local_ip);
        (0..12u32)
            .filter_map(|k| compute_unique_port(ip_be, k))
            .collect()
    } else {
        // Case C: wildcard ephemeral port
        vec![0]
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
    /// the underlying socket fails.
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
    /// Recipient session/connection ID (`0x0000` before session ID assignment; `>= 0x0100` once assigned).
    pub rec_id: u16,
    /// Byte length of the packet payload immediately following this 20-byte header (`0..=0xFF7B`).
    pub pak_size: u16,
    /// Bitmask of optional headers and packet flags (validated against `0x0F7FFB00`).
    pub flags: u32,
    /// Packet sequence number (`0` for initial handshake; `>= 2` for sequenced packets).
    pub seq_num: u32,
    /// 32-bit packet checksum (`headerChecksum + payloadChecksum`, with ISAAC XOR masking if `flags & 0x02000000`).
    pub checksum: u32,
    /// Routing / referral iteration byte (`0x10`).
    pub iteration: u8,
    /// Server routing table index (`0x11`).
    pub table: u8,
    /// 16-bit millisecond tick delta (`0x12..0x13`).
    pub time: u16,
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
        buf[16] = self.iteration;
        buf[17] = self.table;
        buf[18..20].copy_from_slice(&self.time.to_be_bytes());
        buf
    }

    /// Parses a 20-byte big-endian `DatagramHeader` and validates its `flags` mask (`0x0F7FFB00`, §4.2, §4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `bytes.len() < 20`, `(flags & 0xF08004FF) != 0`, or `pak_size > 0xFF7B`.
    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < DATAGRAM_HEADER_BYTES {
            return Err(SessionError::DatagramTooShort { len: bytes.len() });
        }
        let rec_id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let pak_size = u16::from_be_bytes([bytes[2], bytes[3]]);
        let flags = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let seq_num = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let checksum = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let iteration = bytes[16];
        let table = bytes[17];
        let time = u16::from_be_bytes([bytes[18], bytes[19]]);

        if (flags & INVALID_DATAGRAM_FLAGS_MASK) != 0 {
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
            iteration,
            table,
            time,
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

/// Computes the 32-bit word-summing checksum primitive `compute_datagram_checksum(buf, len)` (§4.4.1).
///
/// Full 4-byte words are summed as little-endian `u32` values modulo `2^32`.
/// If `len & 3 != 0`, trailing `1..=3` bytes are packed MSB-first (`<< 24`, `<< 16`, `<< 8`)
/// and added together with `(len << 16)`.
#[must_use]
pub fn compute_datagram_checksum(buf: &[u8]) -> u32 {
    let len = buf.len() as u32;
    let mut acc = 0u32;
    let (chunks, rem) = buf.as_chunks::<4>();
    for &chunk in chunks {
        let w = u32::from_le_bytes(chunk);
        acc = acc.wrapping_add(w);
    }
    if !rem.is_empty() {
        let mut tail = 0u32;
        let mut shift = 24u32;
        for &b in rem {
            tail |= u32::from(b) << shift;
            shift = shift.saturating_sub(8);
        }
        acc = acc.wrapping_add(tail).wrapping_add(len.wrapping_shl(16));
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

/// Computes the wire `DatagramHeader.checksum` by the receive rule of §4.4.2 step 2:
/// `payloadSum` is [`compute_datagram_checksum`] over the whole `pakSize` payload.
///
/// # Errors
///
/// See [`combine_packet_checksum`].
pub fn compute_packet_checksum(
    header: DatagramHeader,
    payload: &[u8],
    xor_mask: Option<u32>,
) -> Result<u32, SessionError> {
    combine_packet_checksum(header, compute_datagram_checksum(payload), xor_mask)
}

/// `payloadSum` by the send rule of §4.4.2 step 2: [`compute_datagram_checksum`] summed
/// (modulo 2^32) over each attached optional header buffer and over each fragment (its
/// header, alignment pad and attachment together with its payload slice).
///
/// When a buffer other than the last has a length that is not a multiple of 4 (for example
/// the 6-byte `FlowControlHeader`), or any buffer but the first is unaligned, this sum
/// differs from the receive rule's whole-payload sum. The two rules of §4.4.2 are kept as
/// written; how they reconcile is a question for the spec writers.
///
/// # Errors
///
/// Returns [`SessionError`] if a header or fragment fails to encode.
pub fn send_payload_checksum(
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
        let mut buffer = Vec::new();
        frag.encode(&mut buffer)?;
        sum = sum.wrapping_add(compute_datagram_checksum(&buffer));
    }
    Ok(sum)
}

/// Verifies `header.checksum` against `payload` and `xor_mask` by the receive rule
/// (§4.4.2, §10.1 rule 6).
///
/// # Errors
///
/// Returns [`SessionError::ChecksumMismatch`] if the computed checksum does not match
/// `header.checksum`, or [`SessionError::MissingChecksumMask`] (see
/// [`combine_packet_checksum`]).
pub fn verify_packet_checksum(
    header: DatagramHeader,
    payload: &[u8],
    xor_mask: Option<u32>,
) -> Result<(), SessionError> {
    let expected = compute_packet_checksum(header, payload, xor_mask)?;
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

/// Encodes `value` using the `VariableU32` variable-length integer scheme (§5.6.1),
/// appending `1`, `2`, `4`, or `5` bytes to `out` and returning the number of bytes written.
pub fn encode_variable_u32(value: u32, out: &mut Vec<u8>) -> usize {
    if value < 0x80 {
        out.push(value as u8);
        1
    } else if value < 0x4000 {
        let b0 = 0x80u8 | ((value >> 8) as u8 & 0x3F);
        let b1 = (value & 0xFF) as u8;
        out.extend_from_slice(&[b0, b1]);
        2
    } else if value < 0x2000_0000 {
        let b0 = 0xC0u8 | ((value >> 24) as u8 & 0x1F);
        let b1 = ((value >> 16) & 0xFF) as u8;
        // Hybrid endianness (§5.6.1): low 16 bits are stored as a little-endian u16 at b2..b3
        let b2 = (value & 0xFF) as u8;
        let b3 = ((value >> 8) & 0xFF) as u8;
        out.extend_from_slice(&[b0, b1, b2, b3]);
        4
    } else {
        out.push(0xE0);
        out.extend_from_slice(&value.to_le_bytes());
        5
    }
}

/// Decodes a `VariableU32` variable-length integer from the start of `bytes` (§5.6.1),
/// returning `(value, bytes_consumed)`.
///
/// # Errors
///
/// Returns [`SessionError::InvalidVariableU32`] if `bytes` has fewer bytes than required
/// by the prefix tag in `bytes[0]` or uses an unrecognized prefix tag.
pub fn decode_variable_u32(bytes: &[u8]) -> Result<(u32, usize), SessionError> {
    let Some(&b0) = bytes.first() else {
        return Err(SessionError::InvalidVariableU32);
    };
    if (b0 & 0x80) == 0 {
        Ok((u32::from(b0), 1))
    } else if (b0 & 0x40) == 0 {
        if bytes.len() < 2 {
            return Err(SessionError::InvalidVariableU32);
        }
        let val = (u32::from(b0 & 0x3F) << 8) | u32::from(bytes[1]);
        Ok((val, 2))
    } else if (b0 & 0x20) == 0 {
        if bytes.len() < 4 {
            return Err(SessionError::InvalidVariableU32);
        }
        // Hybrid endianness (§5.6.1): b2 is bits 0..7, b3 is bits 8..15
        let low16 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let val = (u32::from(b0 & 0x1F) << 24) | (u32::from(bytes[1]) << 16) | u32::from(low16);
        Ok((val, 4))
    } else if b0 == 0xE0 {
        if bytes.len() < 5 {
            return Err(SessionError::InvalidVariableU32);
        }
        let val = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        Ok((val, 5))
    } else {
        Err(SessionError::InvalidVariableU32)
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

/// Directional 32-bit ISAAC PRNG instance for `0x02000000` checksum XOR masking (§7.2).
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

/// `SessionAuthHeader` (`0x00010000`) literal wire magic (`"180804\0"`, 7 bytes, §6.2).
pub const LOGIN_NET_VERSION: [u8; 7] = *b"180804\0";

/// `authType` of the ticket-authenticated mode (`0x40000002`, §6.2.1 selection rule 1).
pub const AUTH_TYPE_TICKET: u32 = 0x4000_0002;
/// `authType` of the account-only mode and of the alternate ticket encoding (`0x00000001`,
/// §6.2.1 selection rule 2).
pub const AUTH_TYPE_ACCOUNT: u32 = 0x0000_0001;
/// `authFlags` bit 1 (`0x02`, `HasSecondString`): `secondStr` is present (§6.2.1).
pub const AUTH_FLAG_HAS_SECOND_STRING: u32 = 0x02;
/// `authFlags` bit 2 (`0x04`, `HasExtraBlob`): `extraBlob` is present (§6.2.1).
pub const AUTH_FLAG_HAS_EXTRA_BLOB: u32 = 0x04;
/// The three `(authType, authFlags)` modes of §6.2.1 (`official-login.md` §4.2.1), the only
/// ones Tessera emits or accepts: the ticket mode `(0x40000002, 0x04)` (`extraBlob`), the
/// account-only mode `(0x00000001, 0x00)`, and the alternate ticket encoding
/// `(0x00000001, 0x02)` (`secondStr`, accepted from a peer). Bit 0 is `0` on the wire and
/// bits `3..11` are further conditional fields whose layout is not specified, so any other
/// combination is refused (engineering choice, spec silent: §6.2.1 rejects only bits above
/// `0x800`).
pub const SUPPORTED_AUTH_MODES: [(u32, u32); 3] = [
    (AUTH_TYPE_TICKET, AUTH_FLAG_HAS_EXTRA_BLOB),
    (AUTH_TYPE_ACCOUNT, 0),
    (AUTH_TYPE_ACCOUNT, AUTH_FLAG_HAS_SECOND_STRING),
];

/// Whether `(auth_type, auth_flags)` is one of [`SUPPORTED_AUTH_MODES`].
#[must_use]
pub fn is_supported_auth_mode(auth_type: u32, auth_flags: u32) -> bool {
    SUPPORTED_AUTH_MODES.contains(&(auth_type, auth_flags))
}

/// Largest GLS ticket string accepted for Base64 decoding (`4,095` characters, the
/// registry ticket buffer of `official-login.md` §4.1, applied to every ticket: engineering
/// choice, spec silent).
pub const MAX_TICKET_STRING_LEN: usize = 4095;

/// Writes an `AccountString`: 4-tier `VariableU32` UTF-16 code-unit count + UTF-16LE units
/// (§6.2.1).
///
/// The units are written straight from the string, with no intermediate copy: an
/// `AccountString` may be a credential (`official-login.md` §6.3).
fn encode_wstring(s: &str, out: &mut Vec<u8>) {
    let units = s.encode_utf16().count();
    encode_variable_u32(units as u32, out);
    for unit in s.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

/// The encoded length of [`encode_wstring`]'s output for `s`.
fn wstring_encoded_len(s: &str) -> usize {
    let units = s.encode_utf16().count();
    variable_u32_len(units as u32).saturating_add(units.saturating_mul(2))
}

/// The byte length of a 4-tier `VariableU32` encoding of `value` (§5.6.1).
fn variable_u32_len(value: u32) -> usize {
    if value < 0x80 {
        1
    } else if value < 0x4000 {
        2
    } else if value < 0x2000_0000 {
        4
    } else {
        5
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

    /// Hands the bytes to the caller, who then owns the wipe.
    fn into_inner(mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
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

/// Decodes a GLS ticket string with the standard RFC 4648 Base64 alphabet (`A-Z`, `a-z`,
/// `0-9`, `+`, `/`; `=` padding at the end only). The decoded byte count is the
/// `extraBlob` length `blobLen` of `SessionAuthHeader` (§6.2.1, `official-login.md` §3.4).
/// Unpadded input whose length is `2` or `3` modulo `4` is accepted; surrounding ASCII
/// whitespace is trimmed.
///
/// # Errors
///
/// Returns [`SessionError::InvalidTicketEncoding`] for an empty, over-long
/// ([`MAX_TICKET_STRING_LEN`]) or malformed string.
pub fn decode_ticket_base64(ticket: &str) -> Result<Vec<u8>, SessionError> {
    fn sextet(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some(u32::from(b - b'A')),
            b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let raw = ticket.trim().as_bytes();
    if raw.is_empty() || raw.len() > MAX_TICKET_STRING_LEN {
        return Err(SessionError::InvalidTicketEncoding);
    }
    let pad = raw.iter().rev().take_while(|&&b| b == b'=').count();
    let data = raw
        .get(..raw.len() - pad)
        .ok_or(SessionError::InvalidTicketEncoding)?;
    // Padding completes the last 4-character group: at most two `=`, and only then.
    if pad > 2 || (pad > 0 && !raw.len().is_multiple_of(4)) || data.len() % 4 == 1 {
        return Err(SessionError::InvalidTicketEncoding);
    }
    // The exact decoded size is reserved, so the buffer never moves; a failed decode wipes
    // the bytes decoded so far (`official-login.md` §6.3).
    let mut out = SecretBuf::with_capacity(data.len() / 4 * 3 + 2, true);
    for group in data.chunks(4) {
        let mut acc = 0u32;
        for &b in group {
            acc = (acc << 6) | sextet(b).ok_or(SessionError::InvalidTicketEncoding)?;
        }
        acc <<= 6 * (4 - group.len() as u32);
        let mut bytes = acc.to_be_bytes();
        let take = group.len().saturating_sub(1);
        out.buf
            .extend_from_slice(bytes.get(1..1 + take).unwrap_or_default());
        crate::login::zeroize_bytes(&mut bytes);
    }
    if out.buf.is_empty() {
        return Err(SessionError::InvalidTicketEncoding);
    }
    Ok(out.into_inner())
}

/// Credentials payload of a `SessionAuthHeader` `authBlock` (`authFlags` bit 0 clear:
/// unencrypted, §6.2.1).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionAuthCredentials {
    /// `accountOrTicketStr` (`AccountString`): the account / subscription identifier
    /// (`--account`).
    pub account_or_ticket: String,
    /// Optional `secondStr` (`AccountString`, present when `authFlags & 0x02 != 0`).
    pub second_string: Option<String>,
    /// Optional `extraBlob` (`u32` LE `blobLen` + `blobLen` bytes, present when
    /// `authFlags & 0x04 != 0`): the Base64-decoded GLS ticket.
    pub extra_blob: Option<Vec<u8>>,
}

impl std::fmt::Debug for SessionAuthCredentials {
    // The account name and ticket bytes are credentials (`official-login.md` §6.3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuthCredentials")
            .field("account_or_ticket", &"<REDACTED>")
            .field(
                "second_string",
                &self.second_string.as_ref().map(|_| "<REDACTED>"),
            )
            .field("extra_blob_len", &self.extra_blob.as_ref().map(Vec::len))
            .finish()
    }
}

impl Drop for SessionAuthCredentials {
    // The account name and the ticket are wiped when the credentials go away
    // (`official-login.md` §6.3).
    fn drop(&mut self) {
        crate::login::zeroize_string(&mut self.account_or_ticket);
        if let Some(second) = self.second_string.as_mut() {
            crate::login::zeroize_string(second);
        }
        if let Some(blob) = self.extra_blob.as_mut() {
            crate::login::zeroize_bytes(blob);
        }
    }
}

impl SessionAuthCredentials {
    /// The byte length of [`Self::encode`]'s output.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        let mut len = wstring_encoded_len(&self.account_or_ticket);
        if let Some(second) = &self.second_string {
            len = len.saturating_add(wstring_encoded_len(second));
        }
        if let Some(extra) = &self.extra_blob {
            len = len.saturating_add(4).saturating_add(extra.len());
        }
        len
    }

    /// Serializes `credentialsPayload` in the §6.2.1 field order. The buffer is reserved
    /// at its exact size, so no reallocation leaves a copy of the credentials behind
    /// (`official-login.md` §6.3); the caller wipes it.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        encode_wstring(&self.account_or_ticket, &mut buf);
        if let Some(second) = &self.second_string {
            encode_wstring(second, &mut buf);
        }
        if let Some(extra) = &self.extra_blob {
            buf.extend_from_slice(&(extra.len() as u32).to_le_bytes());
            buf.extend_from_slice(extra);
        }
        buf
    }

    /// Decodes `credentialsPayload` for the given `auth_flags` (§6.2.1).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if any string or byte array is truncated.
    pub fn decode(bytes: &[u8], auth_flags: u32) -> Result<(Self, usize), SessionError> {
        let mut cursor = 0usize;
        let (account_or_ticket, n1) = decode_wstring(bytes)?;
        cursor += n1;
        let second_string = if (auth_flags & AUTH_FLAG_HAS_SECOND_STRING) != 0 {
            let rest = bytes.get(cursor..).ok_or(SessionError::InvalidAuthBlock)?;
            let (s2, n2) = decode_wstring(rest)?;
            cursor += n2;
            Some(s2)
        } else {
            None
        };
        let extra_blob = if (auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB) != 0 {
            let len_bytes: [u8; 4] = bytes
                .get(cursor..cursor.saturating_add(4))
                .and_then(|b| b.try_into().ok())
                .ok_or(SessionError::InvalidAuthBlock)?;
            let len = u32::from_le_bytes(len_bytes) as usize;
            cursor += 4;
            let blob = bytes
                .get(cursor..cursor.saturating_add(len))
                .ok_or(SessionError::InvalidAuthBlock)?
                .to_vec();
            cursor += len;
            Some(blob)
        } else {
            None
        };
        Ok((
            Self {
                account_or_ticket,
                second_string,
                extra_blob,
            },
            cursor,
        ))
    }
}

/// `SessionAuthHeader` (`0x00010000`) optional header: the first client packet of a login,
/// world or chat connection (§6.2, §6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAuthHeader {
    /// `authType` (`u32` LE at `authBlock + 0x00`): [`AUTH_TYPE_TICKET`] or [`AUTH_TYPE_ACCOUNT`].
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
    /// Builds the header by the deterministic `authType` / `authFlags` selection rule
    /// (§6.2.1; `official-login.md` §4.2.1):
    ///
    /// 1. With a GLS ticket: `authType = 0x40000002`, `authFlags = 0x04` (`HasExtraBlob`),
    ///    `accountOrTicketStr` = `account`, and `extraBlob` = the Base64-decoded ticket
    ///    (its `blobLen` is the decoded byte count).
    /// 2. Without a ticket: `authType = 0x00000001`, `authFlags = 0`, `accountOrTicketStr` only.
    ///
    /// The same rule applies on the login/world connection and on the chat connection.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidTicketEncoding`] if `ticket_base64` is not valid Base64.
    pub fn for_login(
        account: &str,
        ticket_base64: Option<&str>,
        client_timestamp: u32,
    ) -> Result<Self, SessionError> {
        Ok(match ticket_base64 {
            Some(ticket) => Self {
                auth_type: AUTH_TYPE_TICKET,
                auth_flags: AUTH_FLAG_HAS_EXTRA_BLOB,
                client_timestamp,
                credentials: SessionAuthCredentials {
                    account_or_ticket: account.to_owned(),
                    second_string: None,
                    extra_blob: Some(decode_ticket_base64(ticket)?),
                },
            },
            None => Self {
                auth_type: AUTH_TYPE_ACCOUNT,
                auth_flags: 0,
                client_timestamp,
                credentials: SessionAuthCredentials {
                    account_or_ticket: account.to_owned(),
                    second_string: None,
                    extra_blob: None,
                },
            },
        })
    }

    /// The byte length of [`Self::encode`]'s output: the `7`-byte magic, the `4`-byte
    /// `authBlockPaddedLen` and the `authBlock` padded to a multiple of `4` (§6.2).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        let block = self.credentials.encoded_len().saturating_add(12);
        11usize.saturating_add(block.saturating_add(3) & !3)
    }

    /// Serializes this header into `out` (§6.2): `"180804\0"` (`7` B), `authBlockPaddedLen`
    /// (`u32` LE), and `authBlock` zero-padded to a multiple of `4` bytes. `auth_type` and
    /// `auth_flags` are written as they are; they are never replaced.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnsupportedAuthMode`] if `(auth_type, auth_flags)` is not one
    /// of [`SUPPORTED_AUTH_MODES`], or [`SessionError::AuthCredentialsMismatch`] if the
    /// credentials do not carry exactly the optional fields `auth_flags` names.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        if !is_supported_auth_mode(self.auth_type, self.auth_flags) {
            return Err(SessionError::UnsupportedAuthMode {
                auth_type: self.auth_type,
                auth_flags: self.auth_flags,
            });
        }
        let wants_second = (self.auth_flags & AUTH_FLAG_HAS_SECOND_STRING) != 0;
        let wants_extra = (self.auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB) != 0;
        if wants_second != self.credentials.second_string.is_some()
            || wants_extra != self.credentials.extra_blob.is_some()
        {
            return Err(SessionError::AuthCredentialsMismatch {
                auth_flags: self.auth_flags,
            });
        }
        let start = out.len();
        // `out` is reserved first, so writing the ticket into it never moves it; the two
        // temporary buffers are reserved at their exact sizes (`official-login.md` §6.3).
        out.reserve(self.encoded_len());
        let mut credentials = self.credentials.encode();
        let mut auth_block = Vec::with_capacity((12 + credentials.len() + 3) & !3);
        auth_block.extend_from_slice(&self.auth_type.to_le_bytes());
        auth_block.extend_from_slice(&self.auth_flags.to_le_bytes());
        auth_block.extend_from_slice(&self.client_timestamp.to_le_bytes());
        auth_block.extend_from_slice(&credentials);
        let padded_len = (auth_block.len() + 3) & !3;
        auth_block.resize(padded_len, 0x00);

        out.extend_from_slice(&LOGIN_NET_VERSION);
        out.extend_from_slice(&(padded_len as u32).to_le_bytes());
        out.extend_from_slice(&auth_block);
        // The temporary copies hold the ticket: wipe them (`official-login.md` §6.3).
        crate::login::zeroize_bytes(&mut credentials);
        crate::login::zeroize_bytes(&mut auth_block);
        Ok(out.len() - start)
    }

    /// Decodes a `SessionAuthHeader` from `bytes` (§6.2, §6.2.1), returning
    /// `(header, total_bytes_consumed)`. Both ticket encodings are accepted:
    /// `0x40000002` with `extraBlob`, and the alternate `0x00000001` with `secondStr`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the magic is not `"180804\0"`, `authBlock` is malformed,
    /// or `(authType, authFlags)` is not one of [`SUPPORTED_AUTH_MODES`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        let (Some(magic), Some(len_bytes)) = (bytes.get(0..7), bytes.get(7..11)) else {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_SESSION_AUTH,
            });
        };
        if magic != LOGIN_NET_VERSION {
            return Err(SessionError::InvalidSessionAuthVersion);
        }
        let padded_len =
            u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
        if !padded_len.is_multiple_of(4) || padded_len < 12 {
            return Err(SessionError::InvalidAuthBlock);
        }
        let total_len = 11usize
            .checked_add(padded_len)
            .ok_or(SessionError::InvalidAuthBlock)?;
        let Some(block) = bytes.get(11..total_len) else {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_SESSION_AUTH,
            });
        };
        let word = |at: usize| {
            u32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]])
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
        let (credentials, used) = SessionAuthCredentials::decode(&block[12..], auth_flags)?;
        // Everything after the credentials is the zero padding to `authBlockPaddedLen`.
        if block[12 + used..].iter().any(|&b| b != 0) {
            return Err(SessionError::InvalidAuthBlock);
        }
        Ok((
            Self {
                auth_type,
                auth_flags,
                client_timestamp,
                credentials,
            },
            total_len,
        ))
    }
}

/// `ReferralAuthHeader` (`0x00020000`, 8 bytes) optional header (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReferralAuthHeader {
    /// 64-bit referral cookie (`u64` LE at `+0x00`).
    pub referral_cookie: u64,
}

/// `ConnectChallengeHeader` (`0x00040000`, 44 bytes) optional header (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectChallengeHeader {
    /// Server's current network clock in seconds (`f64` LE at `+0x00`).
    pub server_time: f64,
    /// 16-bit connection ID assigned to the client, or `0` if keeping existing ID (`u16` LE at `+0x08`).
    pub assigned_client_rec_id: u16,
    /// Server's 16-bit recipient connection ID (`remoteRecId`, `u16` LE at `+0x0A`).
    pub server_rec_id: u16,
    /// Big-endian 32-bit seed for the client's outgoing ISAAC generator (`u32` BE at `+0x0C`).
    pub outgoing_seed: u32,
    /// Big-endian 32-bit seed for the client's incoming ISAAC generator (`u32` BE at `+0x10`).
    pub incoming_seed: u32,
    /// `reserved_dword_14` (`u32` LE at `+0x14`): server-side meaning `OPEN`
    /// (`OPEN-SESSION-1`); read and ignored, `0` when Tessera builds one in tests.
    pub reserved_dword_14: u32,
    /// 16-bit connection table index echoed in `ConnectAcceptHeader` (`u16` LE at `+0x18`).
    pub client_conn_idx: u16,
    /// `reserved_word_1a` (`u16` LE at `+0x1A`): server-side meaning `OPEN`
    /// (`OPEN-SESSION-1`); read and ignored.
    pub reserved_word_1a: u16,
    /// 16-byte UUID selecting the per-connection packet codec (`+0x1C..+0x2C`).
    pub codec_uuid: [u8; 16],
}

impl Default for ConnectChallengeHeader {
    fn default() -> Self {
        Self {
            server_time: 0.0,
            assigned_client_rec_id: 0,
            server_rec_id: 0,
            outgoing_seed: 0,
            incoming_seed: 0,
            reserved_dword_14: 0,
            client_conn_idx: 0,
            reserved_word_1a: 0,
            codec_uuid: [0u8; 16],
        }
    }
}

impl ConnectChallengeHeader {
    /// The 8 wire bytes at `+0x08..+0x0F` (`assignedClientRecId` `u16` LE, `serverRecId`
    /// `u16` LE, `outgoingSeed` `u32` BE) read as a big-endian `u64`: the
    /// `ConnectAcceptHeader.checkValue` (§6.4 derivation rule, §10.2 Vector 5).
    #[must_use]
    pub fn check_value(&self) -> u64 {
        let mut b = [0u8; 8];
        b[0..2].copy_from_slice(&self.assigned_client_rec_id.to_le_bytes());
        b[2..4].copy_from_slice(&self.server_rec_id.to_le_bytes());
        b[4..8].copy_from_slice(&self.outgoing_seed.to_be_bytes());
        u64::from_be_bytes(b)
    }

    /// Serializes this 44-byte `ConnectChallengeHeader` into `out` (§6.3).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.server_time.to_le_bytes());
        out.extend_from_slice(&self.assigned_client_rec_id.to_le_bytes());
        out.extend_from_slice(&self.server_rec_id.to_le_bytes());
        // Outgoing & incoming ISAAC seeds at +0x0C and +0x10 are big-endian on wire (§6.3, §11 item 2)
        out.extend_from_slice(&self.outgoing_seed.to_be_bytes());
        out.extend_from_slice(&self.incoming_seed.to_be_bytes());
        out.extend_from_slice(&self.reserved_dword_14.to_le_bytes());
        out.extend_from_slice(&self.client_conn_idx.to_le_bytes());
        out.extend_from_slice(&self.reserved_word_1a.to_le_bytes());
        out.extend_from_slice(&self.codec_uuid);
    }

    /// Decodes a 44-byte `ConnectChallengeHeader` optional header from `bytes` (§6.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 44`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 44 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_CONNECT_CHALLENGE,
            });
        }
        let server_time = f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let assigned_client_rec_id = u16::from_le_bytes([bytes[8], bytes[9]]);
        let server_rec_id = u16::from_le_bytes([bytes[10], bytes[11]]);
        let outgoing_seed = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let incoming_seed = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let reserved_dword_14 = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let client_conn_idx = u16::from_le_bytes([bytes[24], bytes[25]]);
        let reserved_word_1a = u16::from_le_bytes([bytes[26], bytes[27]]);
        let mut codec_uuid = [0u8; 16];
        codec_uuid.copy_from_slice(&bytes[28..44]);
        Ok(Self {
            server_time,
            assigned_client_rec_id,
            server_rec_id,
            outgoing_seed,
            incoming_seed,
            reserved_dword_14,
            client_conn_idx,
            reserved_word_1a,
            codec_uuid,
        })
    }
}

/// `ConnectAcceptHeader` (`0x00080000`, 28 bytes) optional header (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectAcceptHeader {
    /// `checkValue` (`u64` LE at `+0x00`): [`ConnectChallengeHeader::check_value`] of the
    /// challenge being answered (§6.4).
    pub check_value: u64,
    /// Client's 16-bit local connection ID (`u16` LE at `+0x08`).
    pub local_rec_id: u16,
    /// Echo of `clientConnIdx` from `ConnectChallengeHeader` (`u16` LE at `+0x0A`).
    pub client_conn_idx: u16,
    /// Echo of the 16-byte `codecUuid` selected from `ConnectChallengeHeader` (`+0x0C..+0x1C`).
    pub codec_uuid: [u8; 16],
}

impl ConnectAcceptHeader {
    /// Serializes this 28-byte `ConnectAcceptHeader` into `out` (§6.4).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.check_value.to_le_bytes());
        out.extend_from_slice(&self.local_rec_id.to_le_bytes());
        out.extend_from_slice(&self.client_conn_idx.to_le_bytes());
        out.extend_from_slice(&self.codec_uuid);
    }

    /// Decodes a 28-byte `ConnectAcceptHeader` from `bytes` (§6.4).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 28`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 28 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_CONNECT_ACCEPT,
            });
        }
        let check_value = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let local_rec_id = u16::from_le_bytes([bytes[8], bytes[9]]);
        let client_conn_idx = u16::from_le_bytes([bytes[10], bytes[11]]);
        let mut codec_uuid = [0u8; 16];
        codec_uuid.copy_from_slice(&bytes[12..28]);
        Ok(Self {
            check_value,
            local_rec_id,
            client_conn_idx,
            codec_uuid,
        })
    }
}

/// `WorldReferralHeader` (`0x00000800`, 18 bytes) optional header (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldReferralHeader {
    /// 64-bit one-time authentication cookie for the target world server (`u64` LE at `+0x00`).
    pub referral_cookie: u64,
    /// Address family (`2` = `AF_INET`, `u16` LE at `+0x08`).
    pub sin_family: u16,
    /// Target world server IPv4 address and UDP port (`sin_port` BE at `+0x0A`, `sin_addr` BE at `+0x0C`).
    pub target_addr: SocketAddrV4,
    /// `reserved_word_10` (`worldServerId`, `u16` LE at `+0x10`): server-side meaning `OPEN`
    /// (`OPEN-SESSION-2`); read and ignored for connection setup, `0` in tests.
    pub reserved_word_10: u16,
}

impl WorldReferralHeader {
    /// Serializes this 18-byte `WorldReferralHeader` optional header into `out` (§6.5).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.referral_cookie.to_le_bytes());
        out.extend_from_slice(&self.sin_family.to_le_bytes());
        out.extend_from_slice(&self.target_addr.port().to_be_bytes());
        out.extend_from_slice(&self.target_addr.ip().octets());
        out.extend_from_slice(&self.reserved_word_10.to_le_bytes());
    }

    /// Decodes an 18-byte `WorldReferralHeader` optional header from `bytes` (§6.5).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 18`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 18 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_WORLD_REFERRAL,
            });
        }
        let referral_cookie = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let sin_family = u16::from_le_bytes([bytes[8], bytes[9]]);
        let port = u16::from_be_bytes([bytes[10], bytes[11]]);
        let ip = Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]);
        let reserved_word_10 = u16::from_le_bytes([bytes[16], bytes[17]]);
        Ok(Self {
            referral_cookie,
            sin_family,
            target_addr: SocketAddrV4::new(ip, port),
            reserved_word_10,
        })
    }
}

/// `ServerRedirectHeader` (`0x00000100`, 8 bytes) optional header (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerRedirectHeader {
    /// `switchSeq`: monotonic server-redirect sequence number, processed only when greater
    /// than the last one seen (`u32` LE at `+0x00`).
    pub switch_seq: u32,
    /// `switchType`: server redirect reason / mode code (`u32` LE at `+0x04`).
    pub switch_type: u32,
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

/// `FlowControlHeader` (`0x08000000`, 6 bytes) optional header (§5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlowControlHeader {
    /// Number of raw wire bytes received since the previous `FlowControlHeader` header (`u32` LE at `+0x00`).
    pub bytes_received_delta: u32,
    /// Elapsed time in milliseconds (`u16` LE at `+0x04`).
    pub interval_ms: u16,
}

/// Decoded optional headers from a packet payload in canonical ascending bit order (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionalHeaders {
    /// Bit 8 (`0x00000100`): `ServerRedirectHeader` (8 bytes).
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
    /// Bit 24 (`0x01000000`): `ClockSyncHeader` (`f64` LE, 8 bytes).
    pub clock_sync: Option<f64>,
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
    /// (the per-header buffers of the send checksum rule, §4.4.2 step 2). Headers with no
    /// bytes (`DisconnectHeader`) give no buffer.
    ///
    /// # Errors
    ///
    /// See [`Self::encode`].
    ///
    /// When `session_auth` is present the combined copy is wiped before returning, and the
    /// returned buffers hold the credentials: the caller wipes them (`official-login.md`
    /// §6.3; [`send_payload_checksum`] does).
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
            fixed(self.server_redirect.is_some(), 8),
            fixed(self.auth_server_address.is_some(), 8),
            fixed(self.world_referral.is_some(), 18),
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
        // Bit 8: 0x00000100 ServerRedirect (8 B)
        if let Some(ss) = self.server_redirect {
            out.extend_from_slice(&ss.switch_seq.to_le_bytes());
            out.extend_from_slice(&ss.switch_type.to_le_bytes());
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
            out.extend_from_slice(&wlr.referral_cookie.to_le_bytes());
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
        // Bit 24: 0x01000000 ClockSync (8 B f64 LE)
        if let Some(ts) = self.clock_sync {
            out.extend_from_slice(&ts.to_le_bytes());
        }
        marks.push(out.len());
        // Bits 25 (MaskedChecksumFlag) and 26 (HasFragmentedBlobsFlag) carry no bytes (§4.3).
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
    /// - Rejects any wire flags outside `0x0F7FFB00` (`(flags & 0xF08004FF) != 0`, §10.1 rule 3).
    /// - On the client receive path (`from_server == true`) rejects `0x00400000`
    ///   (`ClientControlHeader`, §10.1 rule 7).
    /// - Bits 25 (`MaskedChecksumFlag`) and 26 (`HasFragmentedBlobsFlag`) carry no optional
    ///   header bytes ([`HEADERLESS_FLAGS_MASK`], §4.3).
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
        if (flags & INVALID_DATAGRAM_FLAGS_MASK) != 0 {
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

        // Bit 8: 0x00000100 ServerRedirect (8 B)
        if (active & FLAG_SERVER_REDIRECT) != 0 {
            need(cursor, 8, FLAG_SERVER_REDIRECT)?;
            let switch_seq = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            let switch_type = u32::from_le_bytes([
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            ]);
            hdrs.server_redirect = Some(ServerRedirectHeader {
                switch_seq,
                switch_type,
            });
            cursor += 8;
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
        }
        // Bit 11: 0x00000800 WorldReferral (18 B)
        if (active & FLAG_WORLD_REFERRAL) != 0 {
            hdrs.world_referral = Some(WorldReferralHeader::decode(&payload[cursor..])?);
            cursor += 18;
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
        }
        // Bit 15: 0x00008000 Disconnect (0 B)
        if (active & FLAG_DISCONNECT) != 0 {
            hdrs.disconnect = true;
        }
        // Bit 16: 0x00010000 SessionAuth (variable)
        if (active & FLAG_SESSION_AUTH) != 0 {
            let (lr, consumed) = SessionAuthHeader::decode(&payload[cursor..])?;
            hdrs.session_auth = Some(lr);
            cursor += consumed;
        }
        // Bit 17: 0x00020000 ReferralAuth (8 B)
        if (active & FLAG_REFERRAL_AUTH) != 0 {
            need(cursor, 8, FLAG_REFERRAL_AUTH)?;
            let referral_cookie = u64::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            ]);
            hdrs.referral_auth = Some(ReferralAuthHeader { referral_cookie });
            cursor += 8;
        }
        // Bit 18: 0x00040000 ConnectChallenge (44 B)
        if (active & FLAG_CONNECT_CHALLENGE) != 0 {
            hdrs.connect_challenge = Some(ConnectChallengeHeader::decode(&payload[cursor..])?);
            cursor += 44;
        }
        // Bit 19: 0x00080000 ConnectAccept (28 B)
        if (active & FLAG_CONNECT_ACCEPT) != 0 {
            hdrs.connect_accept = Some(ConnectAcceptHeader::decode(&payload[cursor..])?);
            cursor += 28;
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
        }
        // Bit 24: 0x01000000 ClockSync (8 B)
        if (active & FLAG_CLOCK_SYNC) != 0 {
            need(cursor, 8, FLAG_CLOCK_SYNC)?;
            let ts = f64::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            ]);
            hdrs.clock_sync = Some(ts);
            cursor += 8;
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
        }

        Ok((hdrs, cursor))
    }
}

// ---------------------------------------------------------------------------
// §5.6.2–§5.6.4 Blob Fragment Header, Splitting, and Bounded Reassembly
// ---------------------------------------------------------------------------

/// `blobFlags` bit 0 (`0x01`): 28-byte attachment header follows the fragment header (§5.6.3).
pub const BLOB_FLAG_HAS_ATTACHMENT: u8 = 0x01;
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

/// Optional 28-byte fragment attachment (`blobFlags & 0x01`, field 10 in §5.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FragmentAttachment {
    /// Three 64-bit little-endian words (`24` B).
    pub words64: [u64; 3],
    /// Trailing 32-bit little-endian word (`4` B).
    pub word32: u32,
}

/// Compact variable-length blob fragment header (fields 1–10 in §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobFragmentHeader {
    /// Delivery queue ID (`1..=4`, bits `0..6` of `queueAndOpt`, §5.6.2).
    pub queue_id: u8,
    /// Optional 16-bit extension word (`Some` when bit 7 `0x80` of `queueAndOpt` is set, field 6).
    pub ext_word: Option<u16>,
    /// Fragment flags byte (`blobFlags`, field 2).
    pub blob_flags: u8,
    /// Sequence group ID (present when `queue_id == 3`, field 3a; defaults to `0`).
    pub seq_group: u16,
    /// Per-queue ordering number (present when `queue_id` in `2..=4`, field 3b; defaults to `0xFFFF`).
    pub order_num: u16,
    /// 16-bit blob identifier (present when `queue_id` in `{2, 4}` or (`queue_id == 1` and `IsFragmented`), field 4).
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
    /// Optional 28-byte attachment (`blobFlags & 0x01`, field 10).
    pub attachment: Option<FragmentAttachment>,
}

impl BlobFragmentHeader {
    /// Serializes this blob fragment header into `out`, appending the `0x00` even-byte
    /// alignment pad (`alignPad`, field 9) when fields 1–8b have an odd byte length (§5.6.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `queue_id` is outside `1..=4`, `num_frags == 0`,
    /// `num_frags <= frag_index`, or `frag_data_len >= 0x01D1`.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        if !(1..=4).contains(&self.queue_id) {
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
        if self.ext_word.is_some() {
            queue_and_opt |= 0x80;
        }
        let mut flags = self.blob_flags;
        if self.num_frags > 1 {
            flags |= BLOB_FLAG_IS_FRAGMENTED;
        } else {
            flags &= !BLOB_FLAG_IS_FRAGMENTED;
        }
        if self.attachment.is_some() {
            flags |= BLOB_FLAG_HAS_ATTACHMENT;
        } else {
            flags &= !BLOB_FLAG_HAS_ATTACHMENT;
        }

        // Field 1 & Field 2
        out.push(queue_and_opt);
        out.push(flags);

        // Field 3a: seqGroup if queueId == 3
        if self.queue_id == 3 {
            encode_variable_u32(u32::from(self.seq_group), out);
        }
        // Field 3b: orderNum if queueId in {2, 3, 4}
        if (2..=4).contains(&self.queue_id) {
            out.extend_from_slice(&self.order_num.to_le_bytes());
        }
        // Field 4: blobId if queueId in {2, 4} or (queueId == 1 and IsFragmented)
        if self.queue_id == 2
            || self.queue_id == 4
            || (self.queue_id == 1 && (flags & BLOB_FLAG_IS_FRAGMENTED) != 0)
        {
            out.extend_from_slice(&self.blob_id.to_le_bytes());
        }
        // Field 5a & 5b: fragIndex and numFrags if IsFragmented
        if (flags & BLOB_FLAG_IS_FRAGMENTED) != 0 {
            encode_variable_u32(self.frag_index, out);
            encode_variable_u32(self.num_frags, out);
        }
        // Field 6: extWord if (queueAndOpt & 0x80) != 0
        if let Some(ext) = self.ext_word {
            out.extend_from_slice(&ext.to_le_bytes());
        }
        // Field 7: recipientId if (blobFlags & 0x40) == 0
        if (flags & BLOB_FLAG_DEFAULT_RECIPIENT) == 0 {
            out.extend_from_slice(&self.recipient_id.to_le_bytes());
        }
        // Field 8a & 8b: blobHeaderWord (when fragIndex == 0) and fragDataLen if (blobFlags & 0x80) == 0
        if (flags & BLOB_FLAG_OMIT_CHUNK_INFO) == 0 {
            if self.frag_index == 0 {
                encode_variable_u32(self.blob_header_word.unwrap_or(0), out);
            }
            encode_variable_u32(self.frag_data_len, out);
        }
        // Field 9: alignPad (0x00) if serialized length of fields 1..8b is odd
        let len_so_far = out.len() - start;
        if (len_so_far & 1) != 0 {
            out.push(0x00);
        }
        // Field 10: 28-byte attachment if (blobFlags & 0x01) != 0
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
    /// Returns [`SessionError`] if `bytes` is truncated, `queue_id` is outside `1..=4`,
    /// `num_frags == 0`, `num_frags <= frag_index`, `frag_data_len >= 0x01D1`, or `alignPad != 0x00`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        if bytes.len() < 2 {
            return Err(SessionError::FragmentPayloadOverrun);
        }
        let queue_and_opt = bytes[0];
        let blob_flags = bytes[1];
        let queue_id = queue_and_opt & 0x7F;
        if !(1..=4).contains(&queue_id) {
            return Err(SessionError::InvalidQueueId { queue_id });
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
        let order_num = if (2..=4).contains(&queue_id) {
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
        let blob_id = if queue_id == 2
            || queue_id == 4
            || (queue_id == 1 && (blob_flags & BLOB_FLAG_IS_FRAGMENTED) != 0)
        {
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
        if num_frags == 0 || num_frags <= frag_index {
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

        if frag_data_len >= 0x01D1 {
            return Err(SessionError::InvalidFragmentDataLen { len: frag_data_len });
        }

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

        // Field 10: 28-byte attachment
        let attachment = if (blob_flags & BLOB_FLAG_HAS_ATTACHMENT) != 0 {
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
    /// Delivery queue ID (`1..=4`).
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

/// Bounded `FragmentedBlob` reassembler with a `30.0`-second expiration timer (§5.6.4).
#[derive(Debug, Clone)]
pub struct BlobReassembler {
    incomplete: HashMap<(u16, u8, u16), IncompleteBlob>,
    max_concurrent_blobs: usize,
    max_blob_bytes: usize,
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
        }
    }

    /// Returns the number of currently incomplete multi-fragment blobs.
    #[must_use]
    pub fn incomplete_len(&self) -> usize {
        self.incomplete.len()
    }

    /// Purges incomplete blobs older than `30.0` seconds (§5.6.4).
    pub fn purge_expired(&mut self, now_sec: f64) {
        self.incomplete
            .retain(|_, entry| now_sec - entry.created_at_sec <= 30.0);
    }

    /// The checks of one fragment that need no table state (§5.6.4 rules 1, 4, 5; §7.1;
    /// §10.1 rule 8).
    fn check_fragment(&self, frag: &BlobFragment) -> Result<(), SessionError> {
        let hdr = &frag.header;
        if (hdr.blob_flags & BLOB_FLAG_ENCRYPTED) != 0 {
            tracing::debug!(blob_id = hdr.blob_id, "encrypted FragmentedBlob rejected");
            return Err(SessionError::EncryptedBlobRejected);
        }
        if hdr.num_frags == 0 || hdr.num_frags <= hdr.frag_index {
            return Err(SessionError::InvalidFragmentBounds {
                frag_index: hdr.frag_index,
                num_frags: hdr.num_frags,
            });
        }
        if hdr.frag_data_len >= 0x01D1 || frag.payload.len() != hdr.frag_data_len as usize {
            return Err(SessionError::InvalidFragmentDataLen {
                len: hdr.frag_data_len,
            });
        }

        // Protocol ceiling check: (numFrags - 1) * 436 + fragDataLen <= 0x01B3E4C0 (§5.6.4 rule 1, §10.1 rule 8)
        let min_final_size = (hdr.num_frags as usize - 1)
            .saturating_mul(MAX_BLOB_FRAG_PAYLOAD)
            .saturating_add(hdr.frag_data_len as usize);
        if min_final_size > MAX_BLOB_BYTES {
            return Err(SessionError::BlobTooLarge {
                total_bytes: min_final_size,
                limit: MAX_BLOB_BYTES,
            });
        }
        // §5.6.4 rule 1: the reassembled size must also fit the configured limit. The
        // buffer itself is `numFrags * 436` bytes (rule 4), at most 435 bytes above it.
        let alloc_size = (hdr.num_frags as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
        if min_final_size > self.max_blob_bytes {
            return Err(SessionError::BlobTooLarge {
                total_bytes: min_final_size,
                limit: self.max_blob_bytes,
            });
        }
        // Multi-fragment slices land at `fragIndex * 436` in a `numFrags * 436` buffer (§5.6.4
        // rules 4, 5). A slice longer than 436 bytes is only accepted where it still fits that
        // buffer, so the final fragment of a blob cannot exceed 436 bytes (engineering choice,
        // spec silent: the spec does not say what a receiver does with a longer final slice).
        if hdr.num_frags > 1 {
            let slice_end = (hdr.frag_index as usize)
                .saturating_mul(MAX_BLOB_FRAG_PAYLOAD)
                .saturating_add(frag.payload.len());
            if slice_end > alloc_size {
                return Err(SessionError::FragmentOverrunsReassembly {
                    frag_index: hdr.frag_index,
                    len: hdr.frag_data_len,
                    buffer_len: alloc_size,
                });
            }
        }
        Ok(())
    }

    /// Checks every fragment of one packet as [`Self::ingest`] would, without changing the
    /// table: the per-fragment rules, a `numFrags` that matches an open entry, and room for
    /// the entries the packet would open (expired entries count as gone, §5.6.4 rule 3,
    /// §10.1 rule 8). A packet that passes is then ingested fragment by fragment.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::ingest`].
    pub fn check_packet(
        &self,
        sender_rec_id: u16,
        fragments: &[BlobFragment],
        now_sec: f64,
    ) -> Result<(), SessionError> {
        let live = |entry: &IncompleteBlob| now_sec - entry.created_at_sec <= 30.0;
        let mut new_keys: Vec<((u16, u8, u16), u32)> = Vec::new();
        for frag in fragments {
            self.check_fragment(frag)?;
            let hdr = &frag.header;
            if hdr.num_frags == 1 {
                continue;
            }
            let key = (sender_rec_id, hdr.queue_id, hdr.blob_id);
            let open = self
                .incomplete
                .get(&key)
                .filter(|e| live(e))
                .map(|e| e.num_frags)
                .or_else(|| new_keys.iter().find(|(k, _)| *k == key).map(|&(_, n)| n));
            match open {
                Some(n) if n != hdr.num_frags => {
                    return Err(SessionError::InvalidFragmentBounds {
                        frag_index: hdr.frag_index,
                        num_frags: hdr.num_frags,
                    });
                }
                Some(_) => {}
                None => new_keys.push((key, hdr.num_frags)),
            }
        }
        let live_count = self.incomplete.values().filter(|e| live(e)).count();
        if live_count.saturating_add(new_keys.len()) > self.max_concurrent_blobs {
            return Err(SessionError::ReassemblyTableFull {
                max_concurrent: self.max_concurrent_blobs,
            });
        }
        Ok(())
    }

    /// Ingests a single `BlobFragment` from `sender_rec_id` at `now_sec` (§5.6.4).
    ///
    /// Returns `Ok(Some(ReassembledBlob))` when all fragments of the blob have arrived
    /// (immediately for `numFrags == 1`), or `Ok(None)` when waiting for remaining fragments.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragment bounds or total blob size limits are violated,
    /// or [`SessionError::EncryptedBlobRejected`] for a fragment with
    /// `EncryptedFragmentedBlob` set (§7.1, `OPEN-SESSION-6`).
    pub fn ingest(
        &mut self,
        sender_rec_id: u16,
        frag: BlobFragment,
        now_sec: f64,
    ) -> Result<Option<ReassembledBlob>, SessionError> {
        self.check_fragment(&frag)?;
        let hdr = &frag.header;
        let alloc_size = (hdr.num_frags as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);

        // Fast path for single-fragment blobs (numFrags == 1, fragIndex == 0, §5.6.4 rule 2)
        if hdr.num_frags == 1 {
            return Ok(Some(ReassembledBlob {
                queue_id: hdr.queue_id,
                order_num: hdr.order_num,
                seq_group: hdr.seq_group,
                blob_id: hdr.blob_id,
                recipient_id: hdr.recipient_id,
                blob_header_word: hdr.blob_header_word.unwrap_or(0),
                payload: frag.payload,
            }));
        }

        self.purge_expired(now_sec);
        let key_tuple = (sender_rec_id, hdr.queue_id, hdr.blob_id);
        if !self.incomplete.contains_key(&key_tuple)
            && self.incomplete.len() >= self.max_concurrent_blobs
        {
            return Err(SessionError::ReassemblyTableFull {
                max_concurrent: self.max_concurrent_blobs,
            });
        }

        let entry = self.incomplete.entry(key_tuple).or_insert_with(|| {
            let mask_words = (hdr.num_frags as usize).div_ceil(32);
            IncompleteBlob {
                queue_id: hdr.queue_id,
                order_num: hdr.order_num,
                seq_group: hdr.seq_group,
                blob_id: hdr.blob_id,
                recipient_id: hdr.recipient_id,
                num_frags: hdr.num_frags,
                blob_header_word: None,
                received_mask: vec![0u32; mask_words],
                received_count: 0,
                buffer: vec![0u8; alloc_size],
                trimmed_len: None,
                created_at_sec: now_sec,
            }
        });

        if entry.num_frags != hdr.num_frags {
            return Err(SessionError::InvalidFragmentBounds {
                frag_index: hdr.frag_index,
                num_frags: hdr.num_frags,
            });
        }

        let word_idx = (hdr.frag_index as usize) >> 5;
        let bit_mask = 1u32 << (hdr.frag_index & 0x1F);
        if (entry.received_mask[word_idx] & bit_mask) == 0 {
            entry.received_mask[word_idx] |= bit_mask;
            entry.received_count += 1;

            let offset = (hdr.frag_index as usize) * MAX_BLOB_FRAG_PAYLOAD;
            entry.buffer[offset..offset + frag.payload.len()].copy_from_slice(&frag.payload);
            if hdr.frag_index == 0 {
                entry.blob_header_word = hdr.blob_header_word;
            }
            if hdr.frag_index + 1 == hdr.num_frags {
                entry.trimmed_len = Some(offset + frag.payload.len());
            }
        }

        if entry.received_count == entry.num_frags {
            let Some(mut completed) = self.incomplete.remove(&key_tuple) else {
                return Ok(None);
            };
            if let Some(final_len) = completed.trimmed_len {
                completed.buffer.truncate(final_len);
            }
            return Ok(Some(ReassembledBlob {
                queue_id: completed.queue_id,
                order_num: completed.order_num,
                seq_group: completed.seq_group,
                blob_id: completed.blob_id,
                recipient_id: completed.recipient_id,
                blob_header_word: completed.blob_header_word.unwrap_or(0),
                payload: completed.buffer,
            }));
        }

        Ok(None)
    }
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
    /// Decoded blob fragments (non-empty only when `header.flags & 0x04000000 != 0`, §5.6.3).
    pub fragments: Vec<BlobFragment>,
    /// Raw payload bytes (`header.pak_size` bytes immediately following the 20-byte `DatagramHeader`).
    pub raw_payload: Vec<u8>,
}

/// Encodes a packet (`header`, `opt_headers`, `fragments`) into a raw UDP datagram (§4.1–§4.4).
///
/// - Computes `pak_size` from the serialized optional headers and blob fragments.
/// - Sets `FLAG_HAS_FRAGMENTED_BLOBS` (`0x04000000`) if `fragments` is non-empty, and `FLAG_MASKED_CHECKSUM`
///   (`0x02000000`) if `xor_mask.is_some()`.
/// - Computes `DatagramHeader.checksum` using the `0xBADD70DD` header placeholder, the
///   send rule's per-buffer `payloadSum` ([`send_payload_checksum`]) and optional `xor_mask`
///   (§4.4.2).
/// - Prepends `0x0000` when `wire_codec` is `None`, or `header.rec_id` (big-endian) followed by
///   the bit-tree compressed `[DatagramHeader[2..20] | payload]` when `wire_codec` is `Some` (§4.1.1, §4.1.2).
///   A packet with a `SessionAuthHeader` is a control packet and is always written
///   unencoded (§4.1.1); `wire_codec` is not used for it.
/// - Every intermediate buffer of a packet with a `SessionAuthHeader` is wiped when it is
///   dropped (`official-login.md` §6.3); the returned datagram belongs to the caller.
///
/// # Errors
///
/// Returns [`SessionError`] if optional headers or fragments are invalid or exceed `MAX_PAK_SIZE`.
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

    if payload.buf.len() > usize::from(MAX_PAK_SIZE) {
        return Err(SessionError::InvalidPakSize {
            pak_size: u16::MAX,
            actual_payload_len: payload.buf.len(),
        });
    }

    header.pak_size = payload.buf.len() as u16;
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

    // §4.4.2 step 2, send rule: `payloadSum` is summed per optional header buffer and per
    // fragment (see [`send_payload_checksum`]); the receiver sums the whole payload.
    let payload_sum = send_payload_checksum(opt_headers, fragments)?;
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

/// Decodes and validates a raw UDP datagram before sequence-window/ISAAC-mask lookup (§4.1–§4.3, §10.1 rules 1–3, 7, 8).
///
/// If `verify_unmasked_checksum` is `true` and `(header.flags & 0x02000000) == 0`, this function
/// also verifies `DatagramHeader.checksum` immediately. When `(header.flags & 0x02000000) != 0`,
/// callers should verify the checksum via [`verify_packet_checksum`] after checking the sequence
/// window and obtaining the sequence number's ISAAC `xor_mask` (§5.1, §10.1 rules 5–6).
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
    if datagram.len() < MIN_DATAGRAM_BYTES {
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

    if decoded_buf.len() < DATAGRAM_HEADER_BYTES {
        return Err(SessionError::DatagramTooShort {
            len: decoded_buf.len() + 2,
        });
    }

    let header = DatagramHeader::from_wire_bytes(&decoded_buf[0..DATAGRAM_HEADER_BYTES])?;
    let actual_payload_len = decoded_buf.len() - DATAGRAM_HEADER_BYTES;
    if usize::from(header.pak_size) != actual_payload_len {
        return Err(SessionError::InvalidPakSize {
            pak_size: header.pak_size,
            actual_payload_len,
        });
    }

    let payload = &decoded_buf[DATAGRAM_HEADER_BYTES..];
    if verify_unmasked_checksum && (header.flags & FLAG_MASKED_CHECKSUM) == 0 {
        verify_packet_checksum(header, payload, None)?;
    }

    let (opt_headers, mut cursor) = OptionalHeaders::decode(header.flags, payload, from_server)?;
    let mut fragments = Vec::new();
    if (header.flags & FLAG_HAS_FRAGMENTED_BLOBS) != 0 {
        while cursor < payload.len() {
            if fragments.len() >= MAX_FRAGS_PER_PACKET {
                return Err(SessionError::TooManyFragments {
                    count: fragments.len() + 1,
                });
            }
            let (frag, consumed) = BlobFragment::decode(&payload[cursor..])?;
            cursor += consumed;
            fragments.push(frag);
        }
    } else if cursor < payload.len() {
        return Err(SessionError::TrailingPayloadBytes {
            remaining: payload.len() - cursor,
        });
    }

    Ok(DecodedPacket {
        wire_prefix,
        header,
        opt_headers,
        fragments,
        raw_payload: payload.to_vec(),
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
    /// `ConnectChallengeHeader.codecUuid` matched no loaded dictionary (§6.4 step 3).
    UnsupportedCodec,
    /// The retransmit table overflowed its limits (§5.1).
    OutboundWindowOverflow,
    /// A local disconnect finished: `DisconnectHeader` was retransmitted for `20.0` s in
    /// State `7` (§6.1, §8.2).
    LocalDisconnect,
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
    /// Server sent a `WorldReferralHeader` (`0x00000800`) to a world server (§6.5).
    WorldReferralReceived {
        /// 64-bit referral cookie to present in `ReferralAuthHeader` (`0x00020000`).
        referral_cookie: u64,
        /// Resolved world server IPv4 address and UDP port (the login server's address when
        /// `sin_addr` is `0.0.0.0`).
        world_addr: SocketAddrV4,
        /// `reserved_word_10` as received (meaning `OPEN`, `OPEN-SESSION-2`).
        reserved_word_10: u16,
    },
    /// Server sent a `ServerRedirectHeader` (`0x00000100`, §6.6).
    ServerRedirect(ServerRedirectHeader),
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

/// Saved outbound sequenced packet in the retransmission table (§5.1, §5.3).
#[derive(Debug, Clone)]
pub struct SentPacketRecord {
    /// Assigned outgoing sequence number (`>= 2`).
    pub seq_num: u32,
    /// Saved outgoing ISAAC XOR mask drawn when `seq_num` was first assigned (§7.2).
    pub xor_mask: u32,
    /// Optional headers carried in the packet.
    pub opt_headers: OptionalHeaders,
    /// Blob fragments carried in the packet.
    pub fragments: Vec<BlobFragment>,
    /// True if the packet still has reliable content that should be retransmitted on `SequenceNakHeader`
    /// (false if it only carried ephemeral `queueId == 1` fragments that have expired, §5.3).
    pub retransmit_eligible: bool,
}

/// Per-connection state machine, sequence window, ISAAC generators, and fragmented blob reassembler (§3.4, §5–§8).
pub struct SessionConnection {
    local_rec_id: u16,
    server_rec_id: u16,
    client_conn_idx: u16,
    remote_addr: SocketAddrV4,
    state: ConnectionState,
    state_entered_at_sec: f64,
    last_recv_time_sec: f64,
    last_ack_nak_sent_sec: f64,
    last_clock_sync_sent_sec: f64,
    last_handshake_retransmit_sec: f64,
    last_flow_sent_sec: f64,
    bytes_received_since_flow: u32,
    extended_silence_timeout: bool,
    server_clock_offset_sec: f64,
    last_switch_seq: u32,

    send_seq_num: u32,
    recv_seq_expected: u32,
    outgoing_isaac: Option<IsaacCipher>,
    incoming_isaac: Option<IsaacCipher>,
    missing_gaps: BTreeMap<u32, u32>,
    retransmit_table: BTreeMap<u32, SentPacketRecord>,
    out_of_order_ack_queue: VecDeque<u32>,
    dropped_out_of_order_acks: u64,
    registered_codec_uuids: Vec<[u8; 16]>,
    active_codec_uuid: [u8; 16],

    stored_session_auth: Option<SessionAuthHeader>,
    stored_referral_auth: Option<ReferralAuthHeader>,
    stored_connect_accept: Option<ConnectAcceptHeader>,

    next_order_num: [u16; 5],
    next_blob_id: u16,
    reassembler: BlobReassembler,
    outbound_datagrams: VecDeque<Vec<u8>>,
    events: VecDeque<SessionEvent>,
}

impl std::fmt::Debug for SessionConnection {
    // No datagram bytes, credentials, cookies or ISAAC state: queued datagrams may carry the
    // ticket (`official-login.md` §6.3).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConnection")
            .field("local_rec_id", &self.local_rec_id)
            .field("server_rec_id", &self.server_rec_id)
            .field("remote_addr", &self.remote_addr)
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
    /// Creates a new `SessionConnection` in State `0` (`Idle`) bound to `local_rec_id` (`0x0100..=0xFEFF`)
    /// and `remote_addr` (§3.4, §5.1, §6.1).
    #[must_use]
    pub fn new(local_rec_id: u16, remote_addr: SocketAddrV4, now_sec: f64) -> Self {
        Self {
            local_rec_id: local_rec_id.clamp(MIN_LOCAL_REC_ID, MAX_LOCAL_REC_ID),
            server_rec_id: 0,
            client_conn_idx: 0,
            remote_addr,
            state: ConnectionState::Idle,
            state_entered_at_sec: now_sec,
            last_recv_time_sec: now_sec,
            last_ack_nak_sent_sec: now_sec,
            last_clock_sync_sent_sec: now_sec,
            last_handshake_retransmit_sec: now_sec,
            last_flow_sent_sec: now_sec,
            bytes_received_since_flow: 0,
            extended_silence_timeout: false,
            server_clock_offset_sec: 0.0,
            last_switch_seq: 0,

            send_seq_num: 1,
            recv_seq_expected: 1,
            outgoing_isaac: None,
            incoming_isaac: None,
            missing_gaps: BTreeMap::new(),
            retransmit_table: BTreeMap::new(),
            out_of_order_ack_queue: VecDeque::new(),
            dropped_out_of_order_acks: 0,
            registered_codec_uuids: Vec::new(),
            active_codec_uuid: [0u8; 16],

            stored_session_auth: None,
            stored_referral_auth: None,
            stored_connect_accept: None,

            next_order_num: [0; 5],
            next_blob_id: 1,
            reassembler: BlobReassembler::default(),
            outbound_datagrams: VecDeque::new(),
            events: VecDeque::new(),
        }
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
        self.state == ConnectionState::Closing && now_sec - self.state_entered_at_sec >= 20.0
    }

    /// Returns the local client `recId` (`0x0100..=0xFEFF`).
    #[must_use]
    pub fn local_rec_id(&self) -> u16 {
        self.local_rec_id
    }

    /// Returns the remote server `recId` (`0` prior to `ConnectChallengeHeader`).
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

    /// The negotiated codec UUID (all zeros when no `BitTreeTable` codec is attached, §7.3).
    #[must_use]
    pub fn active_codec_uuid(&self) -> [u8; 16] {
        self.active_codec_uuid
    }

    /// Enables or disables the `300.0`-second extended silence timeout in State `5` (§8.2).
    pub fn set_extended_silence_timeout(&mut self, enabled: bool) {
        self.extended_silence_timeout = enabled;
    }

    /// Registers the 16-byte `codecUuid` (`+0x10..+0x1F`) of a loaded `BitTreeTable`
    /// dictionary (§6.4 step 3, §7.3).
    pub fn register_codec_uuid(&mut self, uuid: [u8; 16]) {
        if !self.registered_codec_uuids.contains(&uuid) {
            self.registered_codec_uuids.push(uuid);
        }
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

    /// The `DatagramHeader` of an outgoing packet. `iteration` and `table` are sent as `0`:
    /// §4.2 names the fields but not the values a client sends (engineering choice, spec
    /// silent).
    fn base_datagram_header(&self, seq_num: u32, now_sec: f64) -> DatagramHeader {
        let time_ms = ((now_sec * 1000.0) as u64 & 0xFFFF) as u16;
        DatagramHeader {
            rec_id: self.server_rec_id,
            pak_size: 0,
            flags: 0,
            seq_num,
            checksum: 0,
            iteration: 0,
            table: 0,
            time: time_ms,
        }
    }

    /// Drops the stored `SessionAuthHeader` (its credentials are wiped on drop) and
    /// `ReferralAuthHeader` once State `1` is left (`official-login.md` §6.3).
    fn forget_auth(&mut self) {
        self.stored_session_auth = None;
        self.stored_referral_auth = None;
    }

    /// Records `reason`, moves to State `8` (`Closing`) and emits [`SessionEvent::Disconnected`].
    fn close(&mut self, reason: DisconnectReason, now_sec: f64) {
        self.forget_auth();
        self.state = ConnectionState::Closing;
        self.state_entered_at_sec = now_sec;
        self.events.push_back(SessionEvent::Disconnected { reason });
    }

    /// Whether a `BitTreeTable` codec was negotiated: the `ConnectChallengeHeader` named a
    /// non-null `codecUuid` (§7.3).
    fn codec_negotiated(&self) -> bool {
        self.active_codec_uuid != [0u8; 16]
    }

    /// The wire codec for a normal sequenced data packet: only on a connection that
    /// negotiated a non-null `codecUuid` and once `remoteRecId` is known (§4.1.1, §4.1.2,
    /// §7.3); otherwise the packet goes out with `wirePrefix` `0x0000`. Control packets
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
    /// unsequenced `SessionAuthHeader` (`0x00010000`) datagram (§6.1, §6.2). Build the header
    /// with [`SessionAuthHeader::for_login`].
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

        let hdr = self.base_datagram_header(0, now_sec);
        let opt = OptionalHeaders {
            session_auth: Some(session_auth),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    /// Initiates a world-server session by transitioning from State `0` to State `1` and queueing
    /// an unsequenced `ReferralAuthHeader` (`0x00020000`) datagram (§6.1, §6.5).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the connection is not in `Idle` state or encoding fails.
    pub fn start_world_login(
        &mut self,
        referral_cookie: u64,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Idle {
            return Err(SessionError::InvalidState { state: self.state });
        }
        let wlr = ReferralAuthHeader { referral_cookie };
        self.stored_referral_auth = Some(wlr);
        self.state = ConnectionState::SessionAuthSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;

        let hdr = self.base_datagram_header(0, now_sec);
        let opt = OptionalHeaders {
            referral_auth: Some(wlr),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    /// Sends a sequenced (`flags & 0x02000000`) packet carrying `opt_headers` and `fragments`,
    /// recording it in the outbound retransmission table with its ISAAC `xor_mask` (§5.1, §7.2).
    /// The packet is compressed with `outgoing_codec` (the client-to-server dictionary) once
    /// `remoteRecId` is known (§4.1.2).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidState`] before a `ConnectChallengeHeader` has seeded the
    /// outgoing ISAAC generator (§7.2), or [`SessionError::OutboundWindowOverflow`] if
    /// unacknowledged packets exceed `129,988` (`0x1FBC4`) or the sequence span exceeds
    /// `0x7FFFFFFF` (§5.1).
    pub fn send_sequenced_packet(
        &mut self,
        mut opt_headers: OptionalHeaders,
        fragments: Vec<BlobFragment>,
        retransmit_eligible: bool,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        // §7.2: a sequenced packet's checksum needs the outgoing ISAAC word.
        if self.outgoing_isaac.is_none() {
            return Err(SessionError::InvalidState { state: self.state });
        }
        if self.retransmit_table.len() >= MAX_UNACKED_SENT_PACKETS {
            self.close(DisconnectReason::OutboundWindowOverflow, now_sec);
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
            return Err(SessionError::OutboundWindowOverflow);
        }

        // Attach FlowControl header if >= 1/3 s and >= 1,400 bytes received (§5.5)
        let elapsed_flow = now_sec - self.last_flow_sent_sec;
        if opt_headers.flow_control.is_none()
            && elapsed_flow >= (1.0 / 3.0)
            && self.bytes_received_since_flow >= 1400
        {
            opt_headers.flow_control = Some(FlowControlHeader {
                bytes_received_delta: self.bytes_received_since_flow,
                // floor(elapsed_sec * 1000.0) truncated to u16: the low 16 bits (§5.5).
                interval_ms: ((elapsed_flow * 1000.0) as u64 & 0xFFFF) as u16,
            });
            self.bytes_received_since_flow = 0;
            self.last_flow_sent_sec = now_sec;
        }

        let Some(outgoing) = self.outgoing_isaac.as_mut() else {
            return Err(SessionError::InvalidState { state: self.state });
        };
        let xor_mask = outgoing.next_mask();
        self.send_seq_num = next_seq;

        let hdr = self.base_datagram_header(next_seq, now_sec);
        let codec = self.data_codec(outgoing_codec);
        let dgram = encode_datagram(hdr, &opt_headers, &fragments, Some(xor_mask), codec)?;
        self.retransmit_table.insert(
            next_seq,
            SentPacketRecord {
                seq_num: next_seq,
                xor_mask,
                opt_headers,
                fragments,
                retransmit_eligible,
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
        if !(1..=4).contains(&queue_id) {
            return Err(SessionError::InvalidQueueId { queue_id });
        }
        // §7.2: check before any order number or blob ID is used up.
        if self.outgoing_isaac.is_none() {
            return Err(SessionError::InvalidState { state: self.state });
        }
        let order_num = if (2..=4).contains(&queue_id) {
            let on = self.next_order_num[usize::from(queue_id)];
            self.next_order_num[usize::from(queue_id)] = on.wrapping_add(1);
            on
        } else {
            0xFFFF
        };
        let is_multi = payload.len() > MAX_BLOB_FRAG_PAYLOAD;
        let blob_id = if queue_id == 2 || queue_id == 4 || (queue_id == 1 && is_multi) {
            let bid = self.next_blob_id;
            self.next_blob_id = if bid == u16::MAX { 1 } else { bid + 1 };
            bid
        } else {
            0
        };

        let frags = split_blob(
            queue_id,
            order_num,
            blob_id,
            seq_group,
            Some(recipient_id),
            blob_header_word,
            payload,
        )?;

        let retransmit_eligible = queue_id == 2 || queue_id == 4;
        let mut seqs = Vec::with_capacity(frags.len());
        for frag in frags {
            let seq = self.send_sequenced_packet(
                OptionalHeaders::default(),
                vec![frag],
                retransmit_eligible,
                now_sec,
                outgoing_codec,
            )?;
            seqs.push(seq);
        }
        Ok(seqs)
    }

    /// Marks an outbound sequenced packet's unreliable fragments as expired so a future `SequenceNakHeader`
    /// for `seq_num` emits `OutOfOrderAckHeader` (`0x00002000`) instead of retransmitting (§5.3, §5.4).
    pub fn expire_unreliable_sent_packet(&mut self, seq_num: u32) {
        if let Some(rec) = self.retransmit_table.get_mut(&seq_num) {
            rec.retransmit_eligible = false;
        }
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
        // Rule 4: Source address binding for existing connections
        if source_addr != self.remote_addr {
            return Err(SessionError::SourceAddressMismatch {
                rec_id: self.local_rec_id,
                expected: self.remote_addr,
                actual: source_addr,
            });
        }

        // Decode framing & structural bounds first without mutating connection state
        let pkt = decode_datagram(datagram, true, self.receive_codec(codecs.incoming), true)?;
        let hdr = pkt.header;

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
                return Err(SessionError::UnexpectedConnectChallenge { state: self.state });
            }
            // A peer may not take a recipient ID held by another connection.
            let id = cr.assigned_client_rec_id;
            if Self::accepts_assigned_rec_id(id) && id != self.local_rec_id && rec_id_in_use(id) {
                tracing::debug!(rec_id = id, "assigned recipient ID already in use");
                return Err(SessionError::RecipientIdCollision { rec_id: id });
            }
        }

        // The reassembly key's sender ID, computed once for the check and the ingest: a
        // ConnectChallengeHeader in this packet sets `serverRecId` before its fragments are
        // ingested (§6.4 step 1), so its fragments are keyed by the new ID.
        let sender_rec_id = pkt
            .opt_headers
            .connect_challenge
            .as_ref()
            .map_or(self.server_rec_id, |cr| cr.server_rec_id);

        // Rule 8: every fragment is checked before anything is committed.
        self.reassembler
            .check_packet(sender_rec_id, &pkt.fragments, now_sec)?;

        // Rules 5 & 6: sequence window and checksum, computed on copies (§5.1, §7.2).
        let commit = if (hdr.flags & FLAG_MASKED_CHECKSUM) != 0 {
            self.check_sequenced(hdr, &pkt.raw_payload)?
        } else {
            SequenceCommit::Unsequenced
        };

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
                    self.missing_gaps.insert(skipped_seq, mask);
                }
                self.recv_seq_expected = seq;
            }
        }

        // Valid packet accepted: update receive timestamp and flow counter
        self.last_recv_time_sec = now_sec;
        self.bytes_received_since_flow = self
            .bytes_received_since_flow
            .saturating_add(datagram.len() as u32);

        // State 2 -> State 5 on the next valid server packet after ConnectChallengeHeader (§6.4)
        if self.state == ConnectionState::ChallengeAcceptSent
            && pkt.opt_headers.connect_challenge.is_none()
            && pkt.opt_headers.transport_error.is_none()
            && pkt.opt_headers.transport_fatal_error.is_none()
        {
            self.enter_connected_state(now_sec)?;
        }

        // Process optional headers (§5.2–§5.4, §6.3–§6.7, §8.1–§8.2)
        self.process_optional_headers(&pkt.opt_headers, now_sec, codecs.outgoing)?;

        // Process blob fragments (checked above; duplicate sequenced packets were dropped,
        // §5.6.4)
        for frag in &pkt.fragments {
            if let Some(blob) = self
                .reassembler
                .ingest(sender_rec_id, frag.clone(), now_sec)?
            {
                self.events.push_back(SessionEvent::BlobReceived(blob));
            }
        }

        Ok(pkt)
    }

    /// Checks a masked (sequenced) packet against the window, the gap table and its ISAAC
    /// word without changing any state (§5.1, §5.3, §7.2, §10.1 rules 5 and 6).
    fn check_sequenced(
        &self,
        hdr: DatagramHeader,
        payload: &[u8],
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
            let Some(&saved_mask) = self.missing_gaps.get(&seq) else {
                // Duplicate or already processed sequence number: its ISAAC mask was consumed
                // and not kept, so the checksum cannot be verified. Drop the packet before any
                // optional header acts or the receive timestamp moves (§10.1 rule 6).
                return Err(SessionError::DuplicateSequence {
                    seq_num: seq,
                    expected: self.recv_seq_expected,
                });
            };
            verify_packet_checksum(hdr, payload, Some(saved_mask))?;
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
        verify_packet_checksum(hdr, payload, Some(packet_mask))?;
        Ok(SequenceCommit::Advance {
            isaac: Box::new(isaac),
            skipped_masks,
            seq,
        })
    }

    /// Whether a `ConnectChallengeHeader` may assign `id` as the local recipient ID: a
    /// value in `0x0100..=0xFEFF` (§3.4). `0` keeps the existing ID (§6.3).
    fn accepts_assigned_rec_id(id: u16) -> bool {
        (MIN_LOCAL_REC_ID..=MAX_LOCAL_REC_ID).contains(&id)
    }

    fn enter_connected_state(&mut self, now_sec: f64) -> Result<(), SessionError> {
        self.state = ConnectionState::Connected;
        self.state_entered_at_sec = now_sec;
        self.last_clock_sync_sent_sec = now_sec;
        self.events.push_back(SessionEvent::Connected {
            local_rec_id: self.local_rec_id,
            server_rec_id: self.server_rec_id,
        });
        // Immediately queue the initial ClockSyncHeader (0x01000000) on entering State 5 (§6.4, §8.1)
        let hdr = self.base_datagram_header(self.send_seq_num, now_sec);
        let opt = OptionalHeaders {
            clock_sync: Some(now_sec + self.server_clock_offset_sec),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    /// Answers a `ConnectChallengeHeader` (§6.4 steps 1–4).
    fn accept_challenge(
        &mut self,
        cr: &ConnectChallengeHeader,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        // Step 3: a non-null codec UUID must match a loaded dictionary (§7.3).
        let is_null_uuid = cr.codec_uuid == [0u8; 16];
        if !is_null_uuid && !self.registered_codec_uuids.contains(&cr.codec_uuid) {
            self.close(DisconnectReason::UnsupportedCodec, now_sec);
            return Err(SessionError::UnsupportedCodecUuid {
                uuid: cr.codec_uuid,
            });
        }
        // Step 1: take the assigned local ID (`0` keeps the existing one, §6.3), the server's
        // recipient ID and connection index, and the server clock.
        if Self::accepts_assigned_rec_id(cr.assigned_client_rec_id) {
            self.local_rec_id = cr.assigned_client_rec_id;
        } else if cr.assigned_client_rec_id != 0 {
            tracing::debug!(
                assigned = cr.assigned_client_rec_id,
                "assigned recipient ID outside 0x0100..=0xFEFF ignored"
            );
        }
        self.server_rec_id = cr.server_rec_id;
        self.client_conn_idx = cr.client_conn_idx;
        self.active_codec_uuid = cr.codec_uuid;
        self.server_clock_offset_sec = cr.server_time - now_sec;
        // Step 2: seed the directional ISAAC generators (§7.2).
        self.outgoing_isaac = Some(IsaacCipher::from_seed(cr.outgoing_seed));
        self.incoming_isaac = Some(IsaacCipher::from_seed(cr.incoming_seed));

        // Step 4: ConnectAcceptHeader with the derived checkValue.
        let resp = ConnectAcceptHeader {
            check_value: cr.check_value(),
            local_rec_id: self.local_rec_id,
            client_conn_idx: self.client_conn_idx,
            codec_uuid: self.active_codec_uuid,
        };
        self.stored_connect_accept = Some(resp);

        // Unsequenced packets after session setup carry the last assigned `sendSeqNum`
        // without `MaskedChecksumFlag` (§4.2, §5.1); only the packets before the
        // ConnectChallengeHeader carry `0`.
        let hdr = self.base_datagram_header(self.send_seq_num, now_sec);
        let resp_opt = OptionalHeaders {
            connect_accept: Some(resp),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &resp_opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);

        if self.state == ConnectionState::SessionAuthSent {
            self.state = ConnectionState::ChallengeAcceptSent;
            self.state_entered_at_sec = now_sec;
            self.last_handshake_retransmit_sec = now_sec;
            // State 1 is left: the stored credentials and cookie are no longer retransmitted.
            self.forget_auth();
        } else if self.state == ConnectionState::ChallengeAcceptSent {
            self.enter_connected_state(now_sec)?;
        }
        Ok(())
    }

    fn process_optional_headers(
        &mut self,
        opt: &OptionalHeaders,
        now_sec: f64,
        outgoing_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        // ConnectChallengeHeader (0x00040000, §6.3, §6.4)
        if let Some(cr) = &opt.connect_challenge {
            self.accept_challenge(cr, now_sec)?;
        }

        // CumulativeAckHeader (0x00004000, big-endian u32, §5.2)
        if let Some(ack_seq) = opt.cumulative_ack {
            self.retransmit_table
                .retain(|&seq, _| (seq.wrapping_sub(ack_seq) as i32) > 0);
        }

        // SequenceNakHeader (0x00001000, §5.3)
        if let Some(nak_seqs) = &opt.sequence_nak {
            if let Some(&first_nak) = nak_seqs.first() {
                // Implicitly acknowledge all sent packets strictly before nak_seqs[0] (§5.3)
                self.retransmit_table
                    .retain(|&seq, _| (seq.wrapping_sub(first_nak) as i32) >= 0);
            }
            // Resends go out with the outgoing (client-to-server) dictionary (§4.1.2).
            let codec = self.data_codec(outgoing_codec);
            for &s in nak_seqs {
                if let Some(rec) = self.retransmit_table.get(&s)
                    && rec.retransmit_eligible
                {
                    let hdr = self.base_datagram_header(rec.seq_num, now_sec);
                    let dgram = encode_datagram(
                        hdr,
                        &rec.opt_headers,
                        &rec.fragments,
                        Some(rec.xor_mask),
                        codec,
                    )?;
                    self.outbound_datagrams.push_back(dgram);
                } else if self.out_of_order_ack_queue.len() >= MAX_PENDING_OUT_OF_ORDER_ACKS {
                    self.dropped_out_of_order_acks =
                        self.dropped_out_of_order_acks.saturating_add(1);
                } else if !self.out_of_order_ack_queue.contains(&s) {
                    self.out_of_order_ack_queue.push_back(s);
                }
            }
        }

        // OutOfOrderAckHeader (0x00002000, §5.4): the listed sequence numbers will never
        // arrive. `recv_seq_expected` is already the highest sequence received, so dropping
        // the gaps is all that is needed for acknowledgements to resume.
        if let Some(dropped_seqs) = &opt.out_of_order_ack {
            for &s in dropped_seqs {
                self.missing_gaps.remove(&s);
            }
        }

        // WorldReferralHeader (0x00000800, §6.5)
        if let Some(ref_hdr) = &opt.world_referral {
            let ip = if ref_hdr.target_addr.ip().is_unspecified() {
                *self.remote_addr.ip()
            } else {
                *ref_hdr.target_addr.ip()
            };
            let world_addr = SocketAddrV4::new(ip, ref_hdr.target_addr.port());
            self.events.push_back(SessionEvent::WorldReferralReceived {
                referral_cookie: ref_hdr.referral_cookie,
                world_addr,
                reserved_word_10: ref_hdr.reserved_word_10,
            });
        }

        // ServerRedirectHeader (0x00000100, §6.6): only when switchSeq is above the last one seen
        if let Some(ss) = opt.server_redirect
            && ss.switch_seq > self.last_switch_seq
        {
            self.last_switch_seq = ss.switch_seq;
            self.events.push_back(SessionEvent::ServerRedirect(ss));
        }

        // AuthServerAddressHeader (0x00000200, §6.6)
        if let Some(lsa) = opt.auth_server_address {
            self.events
                .push_back(SessionEvent::AuthServerAddress(lsa.addr));
        }

        // ClockSyncHeader (0x01000000, §8.1)
        if let Some(server_time) = opt.clock_sync {
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
                tracing::warn!(
                    fatal,
                    error_code = format_args!("{:#010x}", ne.error_code),
                    error_subcode = format_args!("{:#010x}", ne.error_subcode),
                    "transport error from server; closing the connection"
                );
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
    /// - State `5`: checks the `60.0` s / `300.0` s silence timeout, sends `ClockSyncHeader` every
    ///   `30.0` s, flushes `OutOfOrderAckHeader`, sends `SequenceNakHeader` every `0.6` s when gaps
    ///   exist, or `CumulativeAckHeader` every `2.0` s when none exist.
    /// - State `6` / `7`: flushes outbound reliable packets, then retransmits
    ///   `DisconnectHeader` every `1.0` s.
    ///
    /// Every control packet here is unsequenced and goes out with `wirePrefix` `0x0000`
    /// (§4.1.1), so no codec is needed.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if encoding a periodic control packet fails.
    pub fn tick(&mut self, now_sec: f64) -> Result<(), SessionError> {
        self.reassembler.purge_expired(now_sec);

        match self.state {
            ConnectionState::SessionAuthSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.close(DisconnectReason::HandshakeTimeout, now_sec);
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= 2.0 {
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_datagram_header(0, now_sec);
                    let opt = OptionalHeaders {
                        session_auth: self.stored_session_auth.clone(),
                        referral_auth: self.stored_referral_auth,
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::ChallengeAcceptSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.close(DisconnectReason::HandshakeTimeout, now_sec);
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= 0.6
                    && let Some(resp) = self.stored_connect_accept
                {
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_datagram_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        connect_accept: Some(resp),
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::Connected => {
                let silence_limit = if self.extended_silence_timeout {
                    300.0
                } else {
                    60.0
                };
                if now_sec - self.last_recv_time_sec > silence_limit {
                    self.close(DisconnectReason::SilenceTimeout, now_sec);
                    return Ok(());
                }

                // Flush up to 114 OutOfOrderAckHeader entries if any (§5.4)
                if !self.out_of_order_ack_queue.is_empty() {
                    let count = self.out_of_order_ack_queue.len().min(MAX_NAK_ENTRIES);
                    let list: Vec<u32> = self.out_of_order_ack_queue.drain(..count).collect();
                    self.queue_control(
                        now_sec,
                        OptionalHeaders {
                            out_of_order_ack: Some(list),
                            ..OptionalHeaders::default()
                        },
                    )?;
                }

                // Periodic ClockSyncHeader every 30.0 s (§8.1)
                if now_sec - self.last_clock_sync_sent_sec >= 30.0 {
                    self.last_clock_sync_sent_sec = now_sec;
                    self.queue_control(
                        now_sec,
                        OptionalHeaders {
                            clock_sync: Some(now_sec + self.server_clock_offset_sec),
                            ..OptionalHeaders::default()
                        },
                    )?;
                }

                // SequenceNakHeader every 0.6 s while gaps exist (§5.3), else
                // CumulativeAckHeader every 2.0 s (§5.2)
                if !self.missing_gaps.is_empty() {
                    if now_sec - self.last_ack_nak_sent_sec >= 0.6 {
                        self.last_ack_nak_sent_sec = now_sec;
                        let list: Vec<u32> = self
                            .missing_gaps
                            .keys()
                            .copied()
                            .take(MAX_NAK_ENTRIES)
                            .collect();
                        self.queue_control(
                            now_sec,
                            OptionalHeaders {
                                sequence_nak: Some(list),
                                ..OptionalHeaders::default()
                            },
                        )?;
                    }
                } else if self.recv_seq_expected > 1 && now_sec - self.last_ack_nak_sent_sec >= 2.0
                {
                    self.last_ack_nak_sent_sec = now_sec;
                    self.queue_control(
                        now_sec,
                        OptionalHeaders {
                            cumulative_ack: Some(self.recv_seq_expected),
                            ..OptionalHeaders::default()
                        },
                    )?;
                }
            }
            ConnectionState::DisconnectingFlush => {
                if self.retransmit_table.is_empty() || now_sec - self.state_entered_at_sec >= 20.0 {
                    self.state = ConnectionState::DisconnectSent;
                    self.state_entered_at_sec = now_sec;
                    self.last_handshake_retransmit_sec = now_sec;
                    self.queue_control(
                        now_sec,
                        OptionalHeaders {
                            disconnect: true,
                            ..OptionalHeaders::default()
                        },
                    )?;
                }
            }
            ConnectionState::DisconnectSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.close(DisconnectReason::LocalDisconnect, now_sec);
                } else if now_sec - self.last_handshake_retransmit_sec >= 1.0 {
                    self.last_handshake_retransmit_sec = now_sec;
                    self.queue_control(
                        now_sec,
                        OptionalHeaders {
                            disconnect: true,
                            ..OptionalHeaders::default()
                        },
                    )?;
                }
            }
            ConnectionState::Idle
            | ConnectionState::ServerListening
            | ConnectionState::ServerConnectSent
            | ConnectionState::Closing => {}
        }

        Ok(())
    }

    /// Queues an unsequenced control packet that carries the last assigned `seqNum` without
    /// `MaskedChecksumFlag` and goes out uncompressed (§4.1.1, §5.1).
    fn queue_control(&mut self, now_sec: f64, opt: OptionalHeaders) -> Result<(), SessionError> {
        let hdr = self.base_datagram_header(self.send_seq_num, now_sec);
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }
}

/// Single-socket multi-connection demultiplexer (§3.4, §6.5).
#[derive(Debug, Default)]
pub struct SessionMultiplexer {
    connections: BTreeMap<u16, SessionConnection>,
    next_local_rec_id: u16,
    /// Local IDs of removed connections; their traffic is refused until the ID is given
    /// out again (engineering choice, spec silent).
    retired: BTreeSet<u16>,
}

impl SessionMultiplexer {
    /// Creates an empty `SessionMultiplexer` starting `localRecId` allocation at `0x0100` (§3.4).
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            next_local_rec_id: MIN_LOCAL_REC_ID,
            retired: BTreeSet::new(),
        }
    }

    /// Allocates a new `SessionConnection` to `remote_addr` with a unique `localRecId` in `0x0100..=0xFEFF` (§3.4).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::RecipientIdsExhausted`] when every `localRecId`
    /// in the range is in use (the search is bounded, it never spins).
    pub fn create_connection(
        &mut self,
        remote_addr: SocketAddrV4,
        now_sec: f64,
    ) -> Result<u16, SessionError> {
        let next_of = |id: u16| {
            if id >= MAX_LOCAL_REC_ID {
                MIN_LOCAL_REC_ID
            } else {
                id + 1
            }
        };
        let mut candidate = self
            .next_local_rec_id
            .clamp(MIN_LOCAL_REC_ID, MAX_LOCAL_REC_ID);
        let mut probes = 0u32;
        while self.connections.contains_key(&candidate) {
            probes += 1;
            if probes > u32::from(MAX_LOCAL_REC_ID - MIN_LOCAL_REC_ID) {
                return Err(SessionError::RecipientIdsExhausted);
            }
            candidate = next_of(candidate);
        }
        self.next_local_rec_id = if candidate >= MAX_LOCAL_REC_ID {
            MIN_LOCAL_REC_ID
        } else {
            candidate + 1
        };
        let conn = SessionConnection::new(candidate, remote_addr, now_sec);
        self.connections.insert(candidate, conn);
        self.retired.remove(&candidate);
        Ok(candidate)
    }

    /// Removes the connection with `local_rec_id` and returns it. Allocation moves on past
    /// the ID, and datagrams addressed to it are refused (not routed by source address to
    /// another connection) until the ID is allocated again.
    pub fn remove_connection(&mut self, local_rec_id: u16) -> Option<SessionConnection> {
        let conn = self.connections.remove(&local_rec_id)?;
        self.retired.insert(local_rec_id);
        Some(conn)
    }

    /// Returns a shared reference to the connection with `local_rec_id`.
    #[must_use]
    pub fn connection(&self, local_rec_id: u16) -> Option<&SessionConnection> {
        self.connections.get(&local_rec_id)
    }

    /// Returns a mutable reference to the connection with `local_rec_id`.
    pub fn connection_mut(&mut self, local_rec_id: u16) -> Option<&mut SessionConnection> {
        self.connections.get_mut(&local_rec_id)
    }

    /// Routes an incoming UDP datagram from `source_addr` per §3.4:
    /// - If `recId` (the non-zero `wirePrefix`, or `DatagramHeader.recId` behind a zero prefix)
    ///   lies in `0x0100..=0xFEFF` and names a connection, that connection takes the packet
    ///   and enforces source-address binding (§10.1 rule 4).
    /// - Otherwise (`recId < 0x0100`, `recId > 0xFEFF`, or a lookup miss, e.g. the
    ///   `ConnectChallengeHeader` that assigns a new ID) the first connection whose remote
    ///   `(IPv4, port)` equals `source_addr` takes it.
    /// - With no match the datagram is rejected.
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
        if datagram.len() < MIN_DATAGRAM_BYTES {
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

        if self.retired.contains(&header_rec_id) {
            tracing::debug!(
                rec_id = header_rec_id,
                "datagram for a removed connection refused"
            );
            return Err(SessionError::UnknownRecipient {
                rec_id: header_rec_id,
                source_addr,
            });
        }
        let by_id = (MIN_LOCAL_REC_ID..=MAX_LOCAL_REC_ID)
            .contains(&header_rec_id)
            .then_some(header_rec_id)
            .filter(|id| self.connections.contains_key(id));
        // Address fallback (§3.4 step 3): a closing or never-started connection takes
        // nothing; a connection in the handshake (States 1, 2) is preferred over the others
        // with the same peer (engineering choice, spec silent: §3.4 says "one whose remote
        // address matches").
        let target_key = by_id.or_else(|| {
            let candidates = || {
                self.connections.iter().filter(|(_, c)| {
                    c.remote_addr() == source_addr
                        && !matches!(c.state(), ConnectionState::Closing | ConnectionState::Idle)
                })
            };
            candidates()
                .find(|(_, c)| {
                    matches!(
                        c.state(),
                        ConnectionState::SessionAuthSent | ConnectionState::ChallengeAcceptSent
                    )
                })
                .or_else(|| candidates().next())
                .map(|(&k, _)| k)
        });

        let Some(key) = target_key else {
            return Err(SessionError::UnknownRecipient {
                rec_id: header_rec_id,
                source_addr,
            });
        };

        // Take the connection out so the collision check can see every other
        // connection; it goes back under its (possibly new) key either way.
        let mut conn = self
            .connections
            .remove(&key)
            .ok_or(SessionError::UnknownRecipient {
                rec_id: header_rec_id,
                source_addr,
            })?;
        let result = {
            let others = &self.connections;
            conn.receive_datagram_guarded(datagram, source_addr, now_sec, codecs, &|id| {
                others.contains_key(&id)
            })
        };
        // The guard refused any ID in use, so the new key is always free.
        let updated_id = if self.connections.contains_key(&conn.local_rec_id()) {
            key
        } else {
            conn.local_rec_id()
        };
        self.connections.insert(updated_id, conn);
        self.retired.remove(&updated_id);
        result.map(|pkt| (updated_id, pkt))
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
        let mut first_err = None;
        for conn in self.connections.values_mut() {
            if let Err(err) = conn.tick(now_sec) {
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
                sent?;
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

fn encode_pstring_ascii(s: &str, out: &mut Vec<u8>) {
    encode_variable_u32(s.len() as u32, out);
    out.extend_from_slice(s.as_bytes());
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
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.status.to_le_bytes());
        encode_pstring_ascii(&self.server_version_info, &mut out);
        out
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

/// `LoginQueueStatusMessage` (`0x00000130`, Server -> Client) post-handshake message,
/// optional and omitted when no login queue is active (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginQueueStatusMessage {
    /// `logonResult` (`u32` LE).
    pub login_result: u32,
    /// `detailText` (4-tier `VariableU32` UTF-16 code-unit count + UTF-16LE).
    pub detail_text: String,
    /// `queuePosition` (`u32` LE).
    pub queue_position: u32,
    /// `queueTotal` (`u32` LE).
    pub queue_total: u32,
}

impl LoginQueueStatusMessage {
    /// Serializes `LoginQueueStatusMessage` (`0x00000130`) inner payload starting at Offset `0x00` (§9.3).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.login_result.to_le_bytes());
        encode_wstring(&self.detail_text, &mut out);
        out.extend_from_slice(&self.queue_position.to_le_bytes());
        out.extend_from_slice(&self.queue_total.to_le_bytes());
        out
    }

    /// Decodes `LoginQueueStatusMessage` (`0x00000130`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `body` is truncated or `detail_text` is invalid UTF-16LE.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        if body.len() < 4 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGIN_QUEUE_STATUS,
            });
        }
        let login_result = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let (detail_text, ws_len) =
            decode_wstring(&body[4..]).map_err(|_| SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGIN_QUEUE_STATUS,
            })?;
        let pos = 4 + ws_len;
        if body.len() < pos + 8 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGIN_QUEUE_STATUS,
            });
        }
        let queue_position =
            u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
        let queue_total =
            u32::from_le_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]]);
        Ok(Self {
            login_result,
            detail_text,
            queue_position,
            queue_total,
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
            Self::VersionStatus(m) => (OPCODE_VERSION_STATUS, m.encode()),
            Self::LoginQueueStatus(m) => (OPCODE_LOGIN_QUEUE_STATUS, m.encode()),
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
    encode_variable_u32(count_of(sub_envelopes.len())?, &mut body);
    for (sub_opcode, sub_payload) in sub_envelopes {
        encode_variable_u32(*sub_opcode, &mut body);
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
    encode_variable_u32(sub_opcode, &mut body);
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
        // Bytes after the last record are not described by §9.4.2; they are ignored
        // (engineering choice, spec silent).
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
        // §9.4.2 lists the five fields only and says nothing of bytes after `resultCode`;
        // refusing them is an engineering choice (spec silent), the conservative reading.
        let trailing = bytes.len().saturating_sub(cursor.saturating_add(12));
        if trailing != 0 {
            return Err(SessionError::TrailingMessageBytes {
                opcode: OPCODE_SINGLE_WRAPPER,
                remaining: trailing,
            });
        }
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
