// SPDX-License-Identifier: GPL-3.0-or-later
//! Official-server UDP session and transport layer (`docs/specs/official-session.md`).
//!
//! This module implements:
//! - Transport constants, deterministic local port selection, the
//!   [`UdpDatagramTransport`] trait, and single-socket connection demultiplexing (§3).
//! - 2-byte `wirePrefix`, pluggable/runtime-table [`BitTreeCodec`] compression,
//!   20-byte big-endian [`ProtoHeader`], `0xBADD70DD` header/payload checksums,
//!   and strict ascending bit-order optional headers (§4, §6, §8).
//! - Sequence windows, gap tree with saved ISAAC masks, `PakAck` / `Nak` /
//!   `EmptyAck` / `Flow`, [`PackedDword`], compact variable-length `NetBlob`
//!   fragment headers, 436-byte fragmentation, and bounded reassembly (§5).
//! - Pure-Rust 3-key Triple DES (`3des` EDE3) CBC with zero IV and PKCS#7
//!   padding, plus the exact directional ISAAC 32-bit PRNG (§7).
//! - The 9-state connection state machine (`0..8`) and referral hand-off (§6, §8).
//! - Upper-layer [`NetMessage`] envelope framing with bounded zlib decompression
//!   and recursive batch container `0x00000186` / wrapper `0x00000179` (§9).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

// ---------------------------------------------------------------------------
// §3.5 Datagram and Fragment Size Constants
// ---------------------------------------------------------------------------

/// Size of the per-socket raw UDP receive/send staging buffer (`0xFF7C` = `65,404` B, §3.5).
pub const MAX_DATAGRAM_BUF: usize = 0xFF7C;

/// Minimum valid UDP datagram length (`2` B wire prefix + `20` B `ProtoHeader` = `22` B, §3.5).
pub const MIN_DATAGRAM_BYTES: usize = 0x0016;

/// Fixed size of `ProtoHeader` (`20` B, §3.5).
pub const PROTO_HEADER_BYTES: usize = 0x0014;

/// Maximum byte budget for `NetBlob` fragments in a single packet (`464` B, §3.5).
pub const MAX_PACKET_BLOB_BUDGET: usize = 0x01D0;

/// Maximum payload slice placed in a single `NetBlob` fragment (`436` B, §3.5).
pub const MAX_BLOB_FRAG_PAYLOAD: usize = 0x01B4;

/// Maximum number of `NetBlob` fragments permitted in a single UDP packet (`20`, §3.5).
pub const MAX_FRAGS_PER_PACKET: usize = 0x0014;

/// Maximum total reassembled `NetBlob` payload size (`65,520 * 436 = 28,566,720` B, §3.5).
pub const MAX_NETBLOB_BYTES: usize = 0x01B3_E4C0;

/// Hard ceiling (`32 MiB`) enforced by the client's `NetMessage` zlib envelope unpacker (`33,554,432` B, §3.5, §9.1).
pub const MAX_ZLIB_UNCOMPRESSED_BYTES: usize = 0x0200_0000;

/// Maximum number of sequence numbers in a `Nak` or `EmptyAck` header (`114`, §3.5).
pub const MAX_NAK_ENTRIES: usize = 0x0072;

/// Target UDP socket send buffer size (`SO_SNDBUF = 131,072` B, §3.1).
pub const TARGET_SO_SNDBUF: usize = 0x20000;

/// Maximum allowed `ProtoHeader.pakSize` (`0xFF7B` = `65,403` B, §4.3, §10.1).
pub const MAX_PAK_SIZE: u16 = 0xFF7B;

/// Allowed wire bitmask for `ProtoHeader.flags` (`0x0F7FFB00`, §4.3, §10.1 rule 3).
pub const VALID_PROTO_FLAGS_MASK: u32 = 0x0F7F_FB00;

/// Full valid `ProtoHeader.flags` mask including low-byte internal state bits `0x06` (`0x0F7FFB06`, §4.3, §10.1 rule 3).
pub const VALID_PROTO_FLAGS_WITH_INTERNAL_MASK: u32 = 0x0F7F_FB06;

/// Disallowed wire bitmask for `ProtoHeader.flags` (`0xF08004FF`, §4.3, §10.1 rule 3).
pub const INVALID_PROTO_FLAGS_MASK: u32 = !VALID_PROTO_FLAGS_MASK;

/// Mask of optional headers parsed on receive (`0x093FFB00`, §4.3).
pub const RECV_OPT_HEADER_FLAGS_MASK: u32 = 0x093F_FB00;

/// Flags forbidden when received from a server (`0x00400000` `CiceroControl`, plus undefined bits `0x00000400` and `0x00800000`, §4.3, §10.1 rule 7).
pub const SERVER_FORBIDDEN_OPT_FLAGS_MASK: u32 = 0x0040_0000 | 0x0000_0400 | 0x0080_0000;

/// Placeholder magic placed in `ProtoHeader.checksum` (`0x0C..0x0F`) when computing `hdrSum` (§4.4.2).
pub const HEADER_CHECKSUM_PLACEHOLDER_BE: u32 = 0xBADD_70DD;

/// Format UUID `{E795878D-C633-471C-8315-C3DBBEAD2FAA}` at offset `0x00..0x10` of `.dat` `CompressionDictionary` records (§4.1.3).
pub const COMPRESSION_DICT_FORMAT_UUID: [u8; 16] = [
    0x8d, 0x87, 0x95, 0xe7, 0x33, 0xc6, 0x1c, 0x47, 0x83, 0x15, 0xc3, 0xdb, 0xbe, 0xad, 0x2f, 0xaa,
];

/// Fixed header size (`44` B = `0x2C`) of `.dat` `CompressionDictionary` records `0x0E010000`–`0x0E010002` (§4.1.3).
pub const COMPRESSION_DICT_HEADER_BYTES: usize = 0x2C;

/// Root `DataIdMap` DID in `client_general.dat` (`0x28000000`, §4.1.3).
pub const DID_ROOT_DATA_ID_MAP: u32 = 0x2800_0000;

/// `COMPRESSIONDICTIONARY` sub-map DID in `client_general.dat` (`0x28000010`, §4.1.3).
pub const DID_COMPRESSION_DICTIONARY_MAP: u32 = 0x2800_0010;

/// `Client2Server` `CompressionDictionary` record DID in `client_general.dat` (`0x0E010000`, §4.1.3).
pub const DID_DICT_CLIENT_TO_SERVER: u32 = 0x0E01_0000;

/// `Server2Client` `CompressionDictionary` record DID in `client_general.dat` (`0x0E010001`, §4.1.3).
pub const DID_DICT_SERVER_TO_CLIENT: u32 = 0x0E01_0001;

/// `Server2ClientDeferred` redirect `CompressionDictionary` record DID in `client_general.dat` (`0x0E010002`, §4.1.3).
pub const DID_DICT_SERVER_TO_CLIENT_DEFERRED: u32 = 0x0E01_0002;

/// Minimum assigned local recipient ID (`0x0100`, §3.4).
pub const MIN_LOCAL_REC_ID: u16 = 0x0100;

/// Maximum assigned local recipient ID (`0xFEFF`, §3.4).
pub const MAX_LOCAL_REC_ID: u16 = 0xFEFF;

/// Maximum sequence lookahead window (`0x7FFF` = `32,767`, §5.1).
pub const MAX_SEQ_LOOKAHEAD: u32 = 0x7FFF;

/// Maximum count of tracked missing sequence numbers in the gap tree (`160,000`, §5.1).
pub const MAX_MISSING_SEQ_GAPS: usize = 160_000;

/// Maximum count of unacknowledged sent packets in the retransmit table (`130,004`, §5.1).
pub const MAX_UNACKED_SENT_PACKETS: usize = 0x1FBC4;

/// Maximum sequence span in the outbound retransmit table (`0x7FFFFFFF`, §5.1).
pub const MAX_OUTBOUND_SEQ_SPAN: u32 = 0x7FFF_FFFF;

/// Default defensive cap on concurrent incomplete `NetBlob` reassembly entries (`64`, §3.5, §5.6.4).
pub const DEFAULT_MAX_CONCURRENT_BLOBS: usize = 64;

/// Default defensive cap on a single reassembled `NetBlob` buffer (`1 MiB`, §3.5, §5.6.4).
pub const DEFAULT_MAX_REASSEMBLY_BLOB_BYTES: usize = 1024 * 1024;

/// Default defensive cap on a zlib-decompressed `NetMessage` payload (`4 MiB`, §3.5, §9.1).
pub const DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Default defensive cap on recursive container depth (`8`, §3.5, §9.4).
pub const DEFAULT_MAX_CONTAINER_DEPTH: usize = 8;

/// World entity `EnterAOI` compound batch opcode (`0x0000000D` = `13`, §9.4.1).
pub const OPCODE_ENTER_AOI: u32 = 0x0000_000D;

/// World entity `EventBatch` compound batch opcode (`0x0000004C` = `76`, §9.4.1).
pub const OPCODE_EVENT_BATCH: u32 = 0x0000_004C;

/// Post-handshake client -> server `VersionSend` opcode (`0x0000012C` = `300`, §9.3).
pub const OPCODE_VERSION_SEND: u32 = 0x0000_012C;

/// Post-handshake server -> client `VersionResult` opcode (`0x0000012D` = `301`, §9.3).
pub const OPCODE_VERSION_RESULT: u32 = 0x0000_012D;

/// Post-handshake server -> client `AdminMessage` opcode (`0x0000012E` = `302`, §9.3).
pub const OPCODE_ADMIN_MESSAGE: u32 = 0x0000_012E;

/// Post-handshake server -> client `ReqVersionSend` opcode (`0x0000012F` = `303`, §9.3).
pub const OPCODE_REQ_VERSION_SEND: u32 = 0x0000_012F;

/// Post-handshake server -> client `LogonUpdate` / `QueueStatus` opcode (`0x00000130` = `304`, §9.3).
pub const OPCODE_LOGON_UPDATE: u32 = 0x0000_0130;

/// Batch/container game message opcode (`0x00000186` = `390`, §9.4.2).
pub const OPCODE_BATCH_CONTAINER: u32 = 0x0000_0186;

/// Single-message wrapper game message opcode (`0x00000179` = `377`, §9.4.2).
pub const OPCODE_SINGLE_WRAPPER: u32 = 0x0000_0179;

/// Internal version-mismatch error code (`0x0DD6BAE1`, §9.3).
pub const ERROR_VERSION_MISMATCH: u32 = 0x0DD6_BAE1;

// ---------------------------------------------------------------------------
// §4.3 Flag Bit Constants
// ---------------------------------------------------------------------------

/// `ServerSwitch` optional header flag (`0x00000100`, bit 8).
pub const FLAG_SERVER_SWITCH: u32 = 0x0000_0100;
/// `LogonServerAddr` optional header flag (`0x00000200`, bit 9).
pub const FLAG_LOGON_SERVER_ADDR: u32 = 0x0000_0200;
/// Undefined bit 10 flag (`0x00000400`, rejected on wire per §4.3, §10.1 rule 3).
pub const FLAG_EMPTY_HEADER_10: u32 = 0x0000_0400;
/// `Referral` optional header flag (`0x00000800`, bit 11).
pub const FLAG_REFERRAL: u32 = 0x0000_0800;
/// `Nak` optional header flag (`0x00001000`, bit 12).
pub const FLAG_NAK: u32 = 0x0000_1000;
/// `EmptyAck` optional header flag (`0x00002000`, bit 13).
pub const FLAG_EMPTY_ACK: u32 = 0x0000_2000;
/// `PakAck` optional header flag (`0x00004000`, bit 14).
pub const FLAG_PAK_ACK: u32 = 0x0000_4000;
/// `Disconnect` optional header flag (`0x00008000`, bit 15).
pub const FLAG_DISCONNECT: u32 = 0x0000_8000;
/// `LoginRequest` optional header flag (`0x00010000`, bit 16).
pub const FLAG_LOGIN_REQUEST: u32 = 0x0001_0000;
/// `WorldLoginRequest` optional header flag (`0x00020000`, bit 17).
pub const FLAG_WORLD_LOGIN_REQUEST: u32 = 0x0002_0000;
/// `ConnectRequest` optional header flag (`0x00040000`, bit 18).
pub const FLAG_CONNECT_REQUEST: u32 = 0x0004_0000;
/// `ConnectResponse` optional header flag (`0x00080000`, bit 19).
pub const FLAG_CONNECT_RESPONSE: u32 = 0x0008_0000;
/// `NetError` optional header flag (`0x00100000`, bit 20).
pub const FLAG_NET_ERROR: u32 = 0x0010_0000;
/// `NetErrorDisconnect` optional header flag (`0x00200000`, bit 21).
pub const FLAG_NET_ERROR_DISCONNECT: u32 = 0x0020_0000;
/// `CiceroControl` client->server optional header flag (`0x00400000`, bit 22, 4 B `u32` LE, rejected if received from server, §4.3, §10.1 rule 7).
pub const FLAG_CICERO_CONTROL: u32 = 0x0040_0000;
/// Undefined bit 23 flag (`0x00800000`, rejected on wire per §4.3, §10.1 rule 3).
pub const FLAG_RESERVED_23: u32 = 0x0080_0000;
/// `TimeSync` optional header flag (`0x01000000`, bit 24).
pub const FLAG_TIME_SYNC: u32 = 0x0100_0000;
/// `EchoRequest` optional header / `EncryptedChecksum` flag (`0x02000000`, bit 25).
pub const FLAG_ENCRYPTED_CHECKSUM: u32 = 0x0200_0000;
/// Alias for bit 25 when used as the `EchoRequest` optional header on send.
pub const FLAG_ECHO_REQUEST: u32 = 0x0200_0000;
/// `EchoResponse` optional header / `HasNetBlobs` flag (`0x04000000`, bit 26).
pub const FLAG_HAS_NET_BLOBS: u32 = 0x0400_0000;
/// Alias for bit 26 when used as the `EchoResponse` optional header on send.
pub const FLAG_ECHO_RESPONSE: u32 = 0x0400_0000;
/// `Flow` optional header flag (`0x08000000`, bit 27).
pub const FLAG_FLOW: u32 = 0x0800_0000;

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
    /// `ProtoHeader.pakSize` exceeds `0xFF7B` or does not match `decoded_len - 20` (§10.1 rule 1).
    #[error("invalid ProtoHeader pakSize {pak_size} for payload length {actual_payload_len}")]
    InvalidPakSize {
        /// Header `pakSize` field.
        pak_size: u16,
        /// Actual bytes following the 20-byte header.
        actual_payload_len: usize,
    },
    /// `ProtoHeader.flags` has bits outside `0x0FFF7B00` or forbidden server flags (§10.1 rules 3, 7).
    #[error("invalid or forbidden ProtoHeader flags {flags:#010x}")]
    InvalidHeaderFlags {
        /// Raw flags field.
        flags: u32,
    },
    /// Checksum verification failed (§4.4.2, §10.1 rule 6).
    #[error("packet checksum mismatch: expected {expected:#010x}, got {actual:#010x}")]
    ChecksumMismatch {
        /// Expected checksum computed over header and payload.
        expected: u32,
        /// Checksum carried in `ProtoHeader.checksum`.
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
    /// Missing sequence gap count exceeds `160,000` (§5.1, §10.1 rule 5).
    #[error("missing sequence gap count {count} exceeds maximum {MAX_MISSING_SEQ_GAPS}")]
    GapLimitExceeded {
        /// Number of missing gaps.
        count: usize,
    },
    /// Outbound unacknowledged packet table overflowed (`0x00CD0B73` -> `0x8004100F`, §5.1).
    #[error(
        "outbound retransmit window overflow (internal {internal_error:#010x}, hresult {hresult:#010x})"
    )]
    OutboundWindowOverflow {
        /// Internal error code (`0x00CD0B73`).
        internal_error: u32,
        /// Client HRESULT (`0x8004100F`).
        hresult: u32,
    },
    /// Optional header truncated or overran `pakSize` (§10.1 rule 7).
    #[error("optional header {flag:#010x} overran packet payload")]
    OptionalHeaderOverrun {
        /// Flag bit being parsed.
        flag: u32,
    },
    /// Trailing bytes remained in payload when `HasNetBlobs` was not set.
    #[error("{remaining} unparsed trailing bytes after optional headers")]
    TrailingPayloadBytes {
        /// Unconsumed byte count.
        remaining: usize,
    },
    /// `Nak` entry count was `0` or exceeded `114` (`0x72`, §5.3, §10.1 rule 7).
    #[error("invalid Nak entry count {count} (must be in 1..={MAX_NAK_ENTRIES})")]
    InvalidNakCount {
        /// Count value in header.
        count: u32,
    },
    /// `EmptyAck` entry count was `0` or exceeded `114` (`0x72`, §5.4, §10.1 rule 7).
    #[error("invalid EmptyAck entry count {count} (must be in 1..={MAX_NAK_ENTRIES})")]
    InvalidEmptyAckCount {
        /// Count value in header.
        count: u32,
    },
    /// `LoginRequest` version prefix was not `"180804\0"` (§6.2).
    #[error("invalid LoginRequest protocol version string")]
    InvalidLoginRequestVersion,
    /// `LoginRequest` authentication block length or alignment was invalid (§6.2).
    #[error("invalid LoginRequest authBlock length or structure")]
    InvalidAuthBlock,
    /// UTF-16LE string in `LoginRequest` was truncated or invalid UTF-16 (§6.2.1).
    #[error("invalid UTF-16LE string in authentication block")]
    InvalidUtf16String,
    /// Variable-length `PackedDWORD` was truncated or had an invalid prefix (§5.6.1).
    #[error("truncated or invalid PackedDWORD encoding")]
    InvalidPackedDword,
    /// Packet contained more than `20` (`0x14`) `NetBlob` fragments (§5.6.3, §10.1 rule 8).
    #[error("packet fragment count {count} exceeds maximum {MAX_FRAGS_PER_PACKET}")]
    TooManyFragments {
        /// Fragment count.
        count: usize,
    },
    /// `NetBlob` fragment had an invalid `queueId` outside `1..=4` (§5.6.2).
    #[error("invalid NetBlob delivery queue ID {queue_id}")]
    InvalidQueueId {
        /// Decoded `queueId`.
        queue_id: u8,
    },
    /// `NetBlob` fragment had `numFrags == 0` or `numFrags <= fragIndex` (§5.6.3, §10.1 rule 8).
    #[error("invalid NetBlob fragment bounds: fragIndex={frag_index}, numFrags={num_frags}")]
    InvalidFragmentBounds {
        /// Zero-based fragment index.
        frag_index: u32,
        /// Total fragment count.
        num_frags: u32,
    },
    /// `NetBlob` fragment had `fragDataLen >= 0x01D1` (`465`, §5.6.3, §10.1 rule 8).
    #[error("NetBlob fragDataLen {len} is >= 0x01D1 (465)")]
    InvalidFragmentDataLen {
        /// Decoded `fragDataLen`.
        len: u32,
    },
    /// `NetBlob` fragment header odd-length alignment pad byte was non-zero (§5.6.3, §10.1 rule 8).
    #[error("NetBlob fragment alignment pad byte was {pad:#04x}, expected 0x00")]
    NonZeroAlignmentPad {
        /// Non-zero pad byte.
        pad: u8,
    },
    /// `NetBlob` fragment payload overran the packet buffer.
    #[error("NetBlob fragment payload overran packet buffer")]
    FragmentPayloadOverrun,
    /// Reassembled `NetBlob` size exceeds protocol or defensive cap (§5.6.4, §10.1 rule 8).
    #[error("reassembled NetBlob size {total_bytes} exceeds limit {limit}")]
    BlobTooLarge {
        /// Computed total byte size.
        total_bytes: usize,
        /// Maximum allowed byte size.
        limit: usize,
    },
    /// Concurrent incomplete `NetBlob` reassembly table is full.
    #[error("incomplete NetBlob reassembly table full ({max_concurrent} entries)")]
    ReassemblyTableFull {
        /// Cap on concurrent incomplete blobs.
        max_concurrent: usize,
    },
    /// Encrypted `NetBlob` (`blobFlags & 0x08`) arrived without an active 3DES session cipher (§7.2).
    #[error("encrypted NetBlob arrived without an active 3DES session cipher")]
    MissingSessionCipher,
    /// 3DES-CBC ciphertext length is empty or not a multiple of 8 bytes (§7.1).
    #[error("3DES-CBC ciphertext length {len} is not a positive multiple of 8")]
    CiphertextNotBlockAligned {
        /// Ciphertext length in bytes.
        len: usize,
    },
    /// PKCS#7 padding in decrypted 3DES-CBC plaintext is invalid (§7.1).
    #[error("invalid PKCS#7 padding in 3DES-CBC plaintext")]
    InvalidPkcs7Padding,
    /// Unsupported per-connection codec UUID in `ConnectRequest` (`0x00F9982C` -> `0x80041016`, §6.4, §6.7).
    #[error(
        "unsupported per-connection codec UUID (internal {internal_error:#010x}, hresult {hresult:#010x})"
    )]
    UnsupportedCodecUuid {
        /// Requested 16-byte UUID.
        uuid: [u8; 16],
        /// Internal error code (`0x00F9982C`).
        internal_error: u32,
        /// Client HRESULT (`0x80041016`).
        hresult: u32,
    },
    /// `NetMessage` envelope payload is empty (missing `envelopeFlags` byte, §9.1).
    #[error("NetMessage envelope is empty")]
    EmptyMessageEnvelope,
    /// Zlib decompression of a `NetMessage` envelope failed (§9.1).
    #[error("NetMessage zlib decompression failed")]
    ZlibDecompressionFailed,
    /// Zlib-decompressed `NetMessage` exceeded the configured size bound (§9.1).
    #[error("decompressed NetMessage size {size} exceeds limit {limit}")]
    DecompressedMessageTooLarge {
        /// Requested or actual decompressed size.
        size: usize,
        /// Maximum allowed decompressed size.
        limit: usize,
    },
    /// Batch container `0x00000186` sub-message length overran remaining container bytes (§9.2).
    #[error(
        "batch container 0x00000186 sub-message length {sub_len} exceeds remaining {remaining} bytes"
    )]
    ContainerSubMessageOverrun {
        /// Declared sub-message length.
        sub_len: usize,
        /// Remaining bytes in container.
        remaining: usize,
    },
    /// Batch container `0x00000186` exceeded maximum recursion depth (§9.2).
    #[error("batch container 0x00000186 recursion depth exceeded maximum {max_depth}")]
    ContainerRecursionLimitExceeded {
        /// Maximum allowed depth.
        max_depth: usize,
    },
    /// `.dat` `CompressionDictionary` record (`0x0E010000`–`0x0E010002`) is malformed or has an invalid format UUID / version (§4.1.3).
    #[error("invalid CompressionDictionary .dat record")]
    InvalidCompressionDictionary,
    /// `NetMessage` `envelopeFlags` byte has reserved bits `(envelopeFlags & 0xFC) != 0` set (§9.1).
    #[error("invalid NetMessage envelopeFlags {flags:#04x}")]
    InvalidEnvelopeFlags {
        /// Raw `envelopeFlags` byte.
        flags: u8,
    },
    /// `NetMessage` optional routing header (`envelopeFlags & 0x02`) is truncated or has invalid flags (§9.1).
    #[error("invalid NetMessage routing header")]
    InvalidRoutingHeader,
    /// Post-handshake session/version control message (`0x0000012C`..`0x00000130`) or world event batch (`0x0000000D` / `0x0000004C`) is malformed (§9.3, §9.4.1).
    #[error("malformed session control or batch message for opcode {opcode:#010x}")]
    InvalidControlMessage {
        /// Message opcode.
        opcode: u32,
    },
    /// Server rejected client version in `VersionResult` (`0x0000012D`, `status == 0` -> `0x0DD6BAE1`, §9.3).
    #[error("server rejected client version (0x0DD6BAE1): {server_version_info}")]
    VersionRejected {
        /// Server version string from `VersionResult`.
        server_version_info: String,
    },
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

/// Computes the deterministic local UDP port for `attempt` (`k` in `0..12`) from
/// the host-order integer value of the big-endian IPv4 address `ip_be` (§3.2 Case B).
///
/// Formula: `candidate_port = (ip_be % 0xFA07) + (256 * k)`, incrementing `k`
/// whenever `candidate_port <= 0x03FF` (`1023`) so the returned port is always
/// above the privileged port range (`>= 1024`).
#[must_use]
pub fn compute_unique_port(ip_be: u32, attempt: u32) -> u16 {
    let base = ip_be % 0xFA07;
    let mut k = 0u32;
    let mut valid_seen = 0u32;
    loop {
        let candidate = base.wrapping_add(256u32.wrapping_mul(k)) & 0xFFFF;
        if candidate > 0x03FF {
            if valid_seen == attempt {
                return candidate as u16;
            }
            valid_seen = valid_seen.saturating_add(1);
        }
        k = k.saturating_add(1);
    }
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
        (0..12u16)
            .map(|k| explicit_base_port.wrapping_add(k))
            .collect()
    } else if compute_unique && !local_ip.is_unspecified() {
        // Case B: computed unique port for up to 12 attempts
        let ip_be = u32::from_be_bytes(local_ip.octets());
        (0..12u32)
            .map(|attempt| compute_unique_port(ip_be, attempt))
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

/// Standard library non-blocking IPv4 `UdpSocket` transport (§3.1, §3.2).
#[derive(Debug)]
pub struct StdUdpTransport {
    socket: UdpSocket,
    local_addr: SocketAddrV4,
}

impl StdUdpTransport {
    /// Binds a non-blocking IPv4 UDP socket following the §3.2 port-selection rules.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Io`] if binding or configuring non-blocking mode fails.
    pub fn bind(
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<Self, SessionError> {
        let ports = candidate_bind_ports(local_ip, explicit_base_port, compute_unique);
        let mut bound_socket = None;
        for port in ports {
            if let Ok(sock) = UdpSocket::bind(SocketAddrV4::new(local_ip, port)) {
                bound_socket = Some(sock);
                break;
            }
        }
        // Case C fallback to ephemeral port on local_ip, then INADDR_ANY (0.0.0.0:0)
        if bound_socket.is_none() {
            bound_socket = UdpSocket::bind(SocketAddrV4::new(local_ip, 0))
                .or_else(|_| UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)))
                .ok();
        }
        let socket = bound_socket
            .ok_or_else(|| SessionError::Io("failed to bind IPv4 UDP socket".to_string()))?;
        socket.set_nonblocking(true)?;
        let local_addr = match socket.local_addr()? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(SessionError::Io("expected IPv4 socket address".to_string()));
            }
        };
        Ok(Self { socket, local_addr })
    }
}

impl UdpDatagramTransport for StdUdpTransport {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(self.local_addr)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        if buf.len() > MAX_DATAGRAM_BUF {
            return Err(SessionError::DatagramTooLarge { len: buf.len() });
        }
        Ok(self.socket.send_to(buf, SocketAddr::V4(target))?)
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        match self.socket.recv_from(buf) {
            Ok((len, SocketAddr::V4(src))) => Ok(Some((len, src))),
            Ok((_, SocketAddr::V6(_))) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(SessionError::Io(e.to_string())),
        }
    }
}

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

/// Pluggable static bit-tree / Huffman wire codec for datagrams with `wirePrefix != 0x0000` (§4.1.2, §4.1.3).
pub trait BitTreeCodec: std::fmt::Debug + Send + Sync {
    /// Decodes `wire_body` (datagram bytes `2..N`) into `out`, appending at most `max_out` bytes.
    /// Must verify that decoding terminates with the `0x100` end symbol on the final byte boundary.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::WireDecompressionFailed`] if the bitstream is malformed,
    /// fails to end on the final byte boundary, or overflows `max_out`.
    fn decode(
        &self,
        wire_body: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError>;

    /// Encodes `plain_after_rec_id` (bytes `2..20+pakSize` of `[ProtoHeader | Payload]`)
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

/// A leaf node (`isLeaf != 0`) in a `.dat` `CompressionDictionary` (`0x0E010000` / `0x0E010001`, §4.1.3 item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionDictionaryLeaf {
    /// Uncompressed byte sequence emitted by this leaf (`1..=4` bytes, §4.1.3).
    pub symbol_bytes: Vec<u8>,
    /// Bit length of the leaf's Huffman codeword (`1..=32`, §4.1.3).
    pub code_bit_len: u8,
    /// Little-endian codeword consumed LSB-first from bit `0` to `code_bit_len - 1` (§4.1.3).
    pub codeword: u32,
}

/// A node in a `.dat` `CompressionDictionary` (`0x0E010000` / `0x0E010001`, §4.1.3 item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionDictionaryNode {
    /// Leaf node (`isLeaf != 0`).
    Leaf(CompressionDictionaryLeaf),
    /// Internal branch node (`isLeaf == 0`) with **1-based** child node indices (`1..=nodeCount`).
    Branch {
        /// 1-based node index taken when the next input bit is `0`.
        child0_1based: u16,
        /// 1-based node index taken when the next input bit is `1`.
        child1_1based: u16,
    },
}

/// `.dat`-backed `CompressionDictionary` bit-tree wire codec (`0x0E010000`–`0x0E010002`, §4.1.2, §4.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionDictionary {
    /// Format UUID at `0x00..0x10` (`COMPRESSION_DICT_FORMAT_UUID`, §4.1.3).
    pub format_uuid: [u8; 16],
    /// Per-dictionary wire codec UUID at `0x10..0x20` matched against `ConnectRequest.codecUuid` (§4.1.3, §7.4).
    pub codec_uuid: [u8; 16],
    /// Optional redirect DID at `0x20..0x24` (`0x00000000` in `0x0E010000`; `0x0E010001` in `0x0E010001` / `0x0E010002`).
    pub redirect_did: u32,
    /// Format version at `0x24..0x28` (`1` in `0x0E010000` and `0x0E010001`, §4.1.3).
    pub version: u32,
    /// Sequential node definitions (`0..node_count`), where all leaves precede all branches (§4.1.3).
    pub nodes: Vec<CompressionDictionaryNode>,
    /// 0-based index of the first internal branch node (the root of the decoding tree, §4.1.3).
    pub root_node_index: usize,
    /// Fast lookup map from `1..=4`-byte uncompressed sequences to `(codeword, code_bit_len)`.
    leaf_map: HashMap<Vec<u8>, (u32, u8)>,
}

impl CompressionDictionary {
    /// Checks the 44-byte `.dat` header and returns `Ok(Some(redirect_did))` if `redirectDid != 0`
    /// and the record is a redirect stub (`version == 0` and `bodyLen == 0`, e.g. `0x0E010002` -> `0x0E010001`, §4.1.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidCompressionDictionary`] if `bytes.len() < 44` or `formatUuid` mismatches.
    pub fn parse_redirect_did(bytes: &[u8]) -> Result<Option<u32>, SessionError> {
        if bytes.len() < COMPRESSION_DICT_HEADER_BYTES {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        if bytes[0..16] != COMPRESSION_DICT_FORMAT_UUID {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        let redirect_did = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        if redirect_did != 0 {
            Ok(Some(redirect_did))
        } else {
            Ok(None)
        }
    }

    /// Parses a `.dat` `CompressionDictionary` record (`0x0E010000` or `0x0E010001`) from its raw record bytes (§4.1.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidCompressionDictionary`] if the 44-byte header, `formatUuid`,
    /// `version`, `bodyLen`, or node table violates the §4.1.3 invariants.
    pub fn from_dat_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < COMPRESSION_DICT_HEADER_BYTES {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        let mut format_uuid = [0u8; 16];
        format_uuid.copy_from_slice(&bytes[0..16]);
        if format_uuid != COMPRESSION_DICT_FORMAT_UUID {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        let mut codec_uuid = [0u8; 16];
        codec_uuid.copy_from_slice(&bytes[16..32]);
        let redirect_did = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        let version = u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);
        if version != 1 && version != 0x0001_0001 {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        let body_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
        if bytes.len() != COMPRESSION_DICT_HEADER_BYTES.saturating_add(body_len) || body_len < 4 {
            return Err(SessionError::InvalidCompressionDictionary);
        }

        let body = &bytes[COMPRESSION_DICT_HEADER_BYTES..];
        let node_count = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        if !(2..=65535).contains(&node_count) {
            return Err(SessionError::InvalidCompressionDictionary);
        }

        let mut cursor = 4usize;
        let mut nodes = Vec::with_capacity(node_count.min(4096));
        let mut leaf_map = HashMap::new();
        let mut root_node_index: Option<usize> = None;

        for idx in 0..node_count {
            let Some(&is_leaf) = body.get(cursor) else {
                return Err(SessionError::InvalidCompressionDictionary);
            };
            cursor += 1;

            if is_leaf != 0 {
                // Invariant (§4.1.3 item 3): all leaf nodes precede all internal branch nodes
                if root_node_index.is_some() {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                let Some(&symbol_len) = body.get(cursor) else {
                    return Err(SessionError::InvalidCompressionDictionary);
                };
                cursor += 1;
                let sym_len = usize::from(symbol_len);
                if !(1..=4).contains(&sym_len) || body.len() < cursor.saturating_add(sym_len) + 1 {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                let symbol_bytes = body[cursor..cursor + sym_len].to_vec();
                cursor += sym_len;

                let code_bit_len = body[cursor];
                cursor += 1;
                if !(1..=32).contains(&code_bit_len) {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                let cw_bytes_len = (usize::from(code_bit_len) >> 3) + 1;
                if cw_bytes_len > 4 || body.len() < cursor.saturating_add(cw_bytes_len) {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                let mut cw_buf = [0u8; 4];
                cw_buf[..cw_bytes_len].copy_from_slice(&body[cursor..cursor + cw_bytes_len]);
                cursor += cw_bytes_len;
                let codeword = u32::from_le_bytes(cw_buf);

                leaf_map.insert(symbol_bytes.clone(), (codeword, code_bit_len));
                nodes.push(CompressionDictionaryNode::Leaf(CompressionDictionaryLeaf {
                    symbol_bytes,
                    code_bit_len,
                    codeword,
                }));
            } else {
                if root_node_index.is_none() {
                    root_node_index = Some(idx);
                }
                if body.len() < cursor + 4 {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                let child0_1based = u16::from_le_bytes([body[cursor], body[cursor + 1]]);
                let child1_1based = u16::from_le_bytes([body[cursor + 2], body[cursor + 3]]);
                cursor += 4;
                if child0_1based == 0
                    || usize::from(child0_1based) > node_count
                    || child1_1based == 0
                    || usize::from(child1_1based) > node_count
                {
                    return Err(SessionError::InvalidCompressionDictionary);
                }
                nodes.push(CompressionDictionaryNode::Branch {
                    child0_1based,
                    child1_1based,
                });
            }
        }

        if cursor != body.len() {
            return Err(SessionError::InvalidCompressionDictionary);
        }
        let Some(root_idx) = root_node_index else {
            return Err(SessionError::InvalidCompressionDictionary);
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

    /// Serializes this `CompressionDictionary` into the exact `.dat` binary record format (`44 + m` bytes, §4.1.3).
    #[must_use]
    pub fn to_dat_bytes(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        for node in &self.nodes {
            match node {
                CompressionDictionaryNode::Leaf(leaf) => {
                    body.push(1);
                    body.push(leaf.symbol_bytes.len() as u8);
                    body.extend_from_slice(&leaf.symbol_bytes);
                    body.push(leaf.code_bit_len);
                    let cw_bytes_len = (usize::from(leaf.code_bit_len) >> 3) + 1;
                    let cw_le = leaf.codeword.to_le_bytes();
                    body.extend_from_slice(&cw_le[..cw_bytes_len.min(4)]);
                }
                CompressionDictionaryNode::Branch {
                    child0_1based,
                    child1_1based,
                } => {
                    body.push(0);
                    body.extend_from_slice(&child0_1based.to_le_bytes());
                    body.extend_from_slice(&child1_1based.to_le_bytes());
                }
            }
        }

        let mut out = Vec::with_capacity(COMPRESSION_DICT_HEADER_BYTES + body.len());
        out.extend_from_slice(&self.format_uuid);
        out.extend_from_slice(&self.codec_uuid);
        out.extend_from_slice(&self.redirect_did.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Constructs a synthetic `.dat` `CompressionDictionary` for unit and integration testing (§4.1.3).
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
        nodes.push(CompressionDictionaryNode::Leaf(CompressionDictionaryLeaf {
            symbol_bytes: zero4,
            code_bit_len: 1,
            codeword: 0,
        }));

        for b in 0..=255u8 {
            let cw = (u32::from(b) << 1) | 1;
            let sb = vec![b];
            leaf_map.insert(sb.clone(), (cw, 9));
            nodes.push(CompressionDictionaryNode::Leaf(CompressionDictionaryLeaf {
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
        nodes.push(CompressionDictionaryNode::Branch {
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
            nodes.push(CompressionDictionaryNode::Branch {
                child0_1based: c0,
                child1_1based: c1,
            });
        }

        Self {
            format_uuid: COMPRESSION_DICT_FORMAT_UUID,
            codec_uuid,
            redirect_did: 0,
            version: 1,
            nodes,
            root_node_index,
            leaf_map,
        }
    }
}

impl BitTreeCodec for CompressionDictionary {
    fn decode(
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
        if total_bits <= 4usize.saturating_add(unused_trailing_bits) {
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

        while bit_pos < valid_bit_end {
            let mut cur = self.root_node_index;
            let mut steps = 0usize;
            loop {
                let node = self
                    .nodes
                    .get(cur)
                    .ok_or(SessionError::WireDecompressionFailed)?;
                match node {
                    CompressionDictionaryNode::Leaf(leaf) => {
                        if (out.len() - start_len).saturating_add(leaf.symbol_bytes.len()) > max_out
                        {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        out.extend_from_slice(&leaf.symbol_bytes);
                        break;
                    }
                    CompressionDictionaryNode::Branch {
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

/// Runtime-loaded tables for the canonical LSB-first bit-tree codec (§4.1.3).
///
/// Per the §4.1.3 OPEN note, bulk lookup tables from the binary are never
/// embedded in the repository; callers load them at runtime from the user's
/// installation or build synthetic tables in tests.
#[derive(Debug, Clone)]
pub struct RuntimeBitTreeTables {
    /// 257-entryLSB-first codewords (`0x00..=0xFF` plus `0x100` end-of-stream marker).
    pub codewords: [u32; 257],
    /// Bit lengths (`1..=32`) for each of the 257 symbols.
    pub bit_lengths: [u8; 257],
    /// 512-entry 9-bit fast lookup table: `<= 0` means resolved symbol `-entry` (`0..=0x100`),
    /// `> 0` means interior node index in `tree_transitions` after consuming 9 bits.
    pub fast_lookup: [i16; 512],
    /// Binary tree node transitions (`[bit0_child, bit1_child, ...]`) for symbols `> 9` bits:
    /// `<= 0` is a leaf for symbol `-entry`; `> 0` is the next node index.
    pub tree_transitions: Vec<[i16; 2]>,
}

impl RuntimeBitTreeTables {
    /// Builds consistent fast-lookup and tree-transition tables from 257 `(codeword_lsb, bit_len)` pairs.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::WireDecompressionFailed`] if any symbol has `bit_len == 0` or `> 24`
    /// or if the prefix code is ambiguous.
    pub fn from_codewords(
        codewords: [u32; 257],
        bit_lengths: [u8; 257],
    ) -> Result<Self, SessionError> {
        let mut fast_lookup = [i16::MAX; 512];
        let mut tree_transitions: Vec<[i16; 2]> = vec![[i16::MAX, i16::MAX]]; // index 0 unused

        for sym in 0..257usize {
            let len = bit_lengths[sym];
            if len == 0 || len > 24 {
                return Err(SessionError::WireDecompressionFailed);
            }
            let code = codewords[sym];
            let leaf_val = -(sym as i16);
            if len <= 9 {
                let prefix = (code as usize) & ((1usize << len) - 1);
                let step = 1usize << len;
                let mut idx = prefix;
                while idx < 512 {
                    if fast_lookup[idx] != i16::MAX {
                        return Err(SessionError::WireDecompressionFailed);
                    }
                    fast_lookup[idx] = leaf_val;
                    idx += step;
                }
            } else {
                let prefix9 = (code as usize) & 0x1FF;
                let mut node_idx = if fast_lookup[prefix9] == i16::MAX {
                    let next_idx = tree_transitions.len();
                    if next_idx > i16::MAX as usize {
                        return Err(SessionError::WireDecompressionFailed);
                    }
                    tree_transitions.push([i16::MAX, i16::MAX]);
                    fast_lookup[prefix9] = next_idx as i16;
                    next_idx
                } else if fast_lookup[prefix9] > 0 {
                    fast_lookup[prefix9] as usize
                } else {
                    return Err(SessionError::WireDecompressionFailed);
                };

                for bit_pos in 9..(len as usize) {
                    let bit = ((code >> bit_pos) & 1) as usize;
                    let is_last = bit_pos + 1 == (len as usize);
                    let current = tree_transitions[node_idx][bit];
                    if is_last {
                        if current != i16::MAX {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        tree_transitions[node_idx][bit] = leaf_val;
                    } else if current == i16::MAX {
                        let next_idx = tree_transitions.len();
                        if next_idx > i16::MAX as usize {
                            return Err(SessionError::WireDecompressionFailed);
                        }
                        tree_transitions.push([i16::MAX, i16::MAX]);
                        tree_transitions[node_idx][bit] = next_idx as i16;
                        node_idx = next_idx;
                    } else if current > 0 {
                        node_idx = current as usize;
                    } else {
                        return Err(SessionError::WireDecompressionFailed);
                    }
                }
            }
        }

        Ok(Self {
            codewords,
            bit_lengths,
            fast_lookup,
            tree_transitions,
        })
    }

    /// Constructs a synthetic canonical 257-symbol bit-tree table for unit and integration testing.
    /// Symbols `0x00..=0xFE` have 8-bit codewords `0x00..=0xFE`; symbols `0xFF` and `0x100`
    /// share prefix `0xFF` with 9-bit codewords `0x0FF` and `0x1FF`.
    #[must_use]
    pub fn synthetic_canonical() -> Self {
        let mut codewords = [0u32; 257];
        let mut bit_lengths = [0u8; 257];
        for sym in 0..255usize {
            codewords[sym] = sym as u32;
            bit_lengths[sym] = 8;
        }
        codewords[255] = 0x0FF;
        bit_lengths[255] = 9;
        codewords[256] = 0x1FF;
        bit_lengths[256] = 9;
        Self::from_codewords(codewords, bit_lengths).unwrap_or(Self {
            codewords,
            bit_lengths,
            fast_lookup: [0; 512],
            tree_transitions: Vec::new(),
        })
    }
}

impl BitTreeCodec for RuntimeBitTreeTables {
    fn decode(
        &self,
        wire_body: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError> {
        let total_bits = wire_body.len().saturating_mul(8);
        let mut bit_pos = 0usize;
        let start_len = out.len();

        while bit_pos < total_bits {
            // Peek up to 9 bits LSB-first
            let mut peek = 0usize;
            for b in 0..9usize {
                let pos = bit_pos + b;
                if pos < total_bits {
                    let byte = wire_body[pos >> 3];
                    let bit = usize::from((byte >> (pos & 7)) & 1);
                    peek |= bit << b;
                }
            }
            let entry = *self
                .fast_lookup
                .get(peek & 0x1FF)
                .ok_or(SessionError::WireDecompressionFailed)?;
            if entry == i16::MAX {
                return Err(SessionError::WireDecompressionFailed);
            }

            let sym: usize = if entry <= 0 {
                let s = (-entry) as usize;
                let len = usize::from(
                    *self
                        .bit_lengths
                        .get(s)
                        .ok_or(SessionError::WireDecompressionFailed)?,
                );
                if len == 0 || len > 9 || bit_pos + len > total_bits {
                    return Err(SessionError::WireDecompressionFailed);
                }
                bit_pos += len;
                s
            } else {
                if bit_pos + 9 > total_bits {
                    return Err(SessionError::WireDecompressionFailed);
                }
                bit_pos += 9;
                let mut node = entry as usize;
                let mut steps = 0usize;
                loop {
                    if bit_pos >= total_bits || steps > 257 {
                        return Err(SessionError::WireDecompressionFailed);
                    }
                    let byte = wire_body[bit_pos >> 3];
                    let bit = usize::from((byte >> (bit_pos & 7)) & 1);
                    bit_pos += 1;
                    steps += 1;
                    let transitions = self
                        .tree_transitions
                        .get(node)
                        .ok_or(SessionError::WireDecompressionFailed)?;
                    let next = transitions[bit];
                    if next == i16::MAX {
                        return Err(SessionError::WireDecompressionFailed);
                    }
                    if next <= 0 {
                        break (-next) as usize;
                    }
                    node = next as usize;
                }
            };

            if sym == 0x100 {
                // End-of-stream symbol: advance to next byte boundary and verify end of input (§4.1.3)
                let rem = bit_pos & 7;
                if rem != 0 {
                    bit_pos += 8 - rem;
                }
                if bit_pos == total_bits {
                    return Ok(out.len() - start_len);
                }
                return Err(SessionError::WireDecompressionFailed);
            }

            if out.len() - start_len >= max_out {
                return Err(SessionError::WireDecompressionFailed);
            }
            out.push(sym as u8);
        }

        // Reaching end of input without encountering symbol 0x100 is an error (§4.1.3, §10.1 rule 2)
        Err(SessionError::WireDecompressionFailed)
    }

    fn encode(
        &self,
        plain_after_rec_id: &[u8],
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<usize, SessionError> {
        let start_len = out.len();
        let mut cur_byte = 0u8;
        let mut bit_in_byte = 0u8;

        let mut push_bits =
            |mut code: u32, mut len: u8, out: &mut Vec<u8>| -> Result<(), SessionError> {
                if len == 0 {
                    return Err(SessionError::WireCompressionFailed);
                }
                while len > 0 {
                    cur_byte |= ((code & 1) as u8) << bit_in_byte;
                    code >>= 1;
                    len -= 1;
                    bit_in_byte += 1;
                    if bit_in_byte == 8 {
                        if out.len() - start_len >= max_out {
                            return Err(SessionError::WireCompressionFailed);
                        }
                        out.push(cur_byte);
                        cur_byte = 0;
                        bit_in_byte = 0;
                    }
                }
                Ok(())
            };

        for &b in plain_after_rec_id {
            let sym = usize::from(b);
            push_bits(self.codewords[sym], self.bit_lengths[sym], out)?;
        }
        // Append end-of-stream symbol 0x100
        push_bits(self.codewords[0x100], self.bit_lengths[0x100], out)?;
        if bit_in_byte != 0 {
            if out.len() - start_len >= max_out {
                return Err(SessionError::WireCompressionFailed);
            }
            out.push(cur_byte);
        }

        Ok(out.len() - start_len)
    }
}

// ---------------------------------------------------------------------------
// §4.2 Twenty-Byte ProtoHeader & §4.4 Checksum Calculation
// ---------------------------------------------------------------------------

/// 20-byte big-endian `ProtoHeader` at the start of every decoded packet (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProtoHeader {
    /// Recipient session/connection ID (`0x0000` before session ID assignment; `>= 0x0100` once assigned).
    pub rec_id: u16,
    /// Byte length of the packet payload immediately following this 20-byte header (`0..=0xFF7B`).
    pub pak_size: u16,
    /// Bitmask of optional headers and packet flags (validated against `0x0FFF7B00`).
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

impl ProtoHeader {
    /// Serializes this `ProtoHeader` into 20 big-endian wire bytes (§4.2).
    #[must_use]
    pub fn to_wire_bytes(self) -> [u8; PROTO_HEADER_BYTES] {
        let mut buf = [0u8; PROTO_HEADER_BYTES];
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

    /// Parses a 20-byte big-endian `ProtoHeader` and validates its `flags` mask (`0x0FFF7B00`, §4.2, §4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `bytes.len() < 20`, `(flags & 0xF00084FF) != 0`, or `pak_size > 0xFF7B`.
    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < PROTO_HEADER_BYTES {
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

        if (flags & INVALID_PROTO_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }
        if pak_size > MAX_PAK_SIZE {
            return Err(SessionError::InvalidPakSize {
                pak_size,
                actual_payload_len: bytes.len().saturating_sub(PROTO_HEADER_BYTES),
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
        calc_checksum(&wire)
    }
}

/// Placeholder constant `0xBADD70DD` (`BA DD 70 DD` in big-endian wire order, §4.4.2).
pub const HEADER_CHECKSUM_MAGIC_BE: u32 = HEADER_CHECKSUM_PLACEHOLDER_BE;

/// Computes the 32-bit word-summing checksum primitive `CalcChecksum(buf, len)` (§4.4.1).
///
/// Full 4-byte words are summed as little-endian `u32` values modulo `2^32`.
/// If `len & 3 != 0`, trailing `1..=3` bytes are packed MSB-first (`<< 24`, `<< 16`, `<< 8`)
/// and added together with `(len << 16)`.
#[must_use]
pub fn calc_checksum(buf: &[u8]) -> u32 {
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

/// Computes the wire `ProtoHeader.checksum` value given `header`, `payload`, and
/// the optional directional ISAAC `xor_mask` (used when `header.flags & 0x02000000 != 0`, §4.4.2).
#[must_use]
pub fn compute_packet_checksum(header: ProtoHeader, payload: &[u8], xor_mask: Option<u32>) -> u32 {
    let hdr_sum = header.header_checksum();
    let payload_sum = calc_checksum(payload);
    let effective_payload_sum = if (header.flags & FLAG_ENCRYPTED_CHECKSUM) != 0 {
        payload_sum ^ xor_mask.unwrap_or(0)
    } else {
        payload_sum
    };
    hdr_sum.wrapping_add(effective_payload_sum)
}

/// Verifies `header.checksum` against `payload` and `xor_mask` (§4.4.2, §10.1 rule 6).
///
/// # Errors
///
/// Returns [`SessionError::ChecksumMismatch`] if the computed checksum does not match `header.checksum`.
pub fn verify_packet_checksum(
    header: ProtoHeader,
    payload: &[u8],
    xor_mask: Option<u32>,
) -> Result<(), SessionError> {
    let expected = compute_packet_checksum(header, payload, xor_mask);
    if header.checksum != expected {
        return Err(SessionError::ChecksumMismatch {
            expected,
            actual: header.checksum,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §5.6.1 PackedDword Variable-Length Integer Encoding
// ---------------------------------------------------------------------------

/// Encodes `value` using the `PackedDword` variable-length integer scheme (§5.6.1),
/// appending `1`, `2`, `4`, or `5` bytes to `out` and returning the number of bytes written.
pub fn encode_packed_dword(value: u32, out: &mut Vec<u8>) -> usize {
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

/// Decodes a `PackedDword` variable-length integer from the start of `bytes` (§5.6.1),
/// returning `(value, bytes_consumed)`.
///
/// # Errors
///
/// Returns [`SessionError::InvalidPackedDword`] if `bytes` has fewer bytes than required
/// by the prefix tag in `bytes[0]` or uses an unrecognized prefix tag.
pub fn decode_packed_dword(bytes: &[u8]) -> Result<(u32, usize), SessionError> {
    let Some(&b0) = bytes.first() else {
        return Err(SessionError::InvalidPackedDword);
    };
    if (b0 & 0x80) == 0 {
        Ok((u32::from(b0), 1))
    } else if (b0 & 0x40) == 0 {
        if bytes.len() < 2 {
            return Err(SessionError::InvalidPackedDword);
        }
        let val = (u32::from(b0 & 0x3F) << 8) | u32::from(bytes[1]);
        Ok((val, 2))
    } else if (b0 & 0x20) == 0 {
        if bytes.len() < 4 {
            return Err(SessionError::InvalidPackedDword);
        }
        // Hybrid endianness (§5.6.1): b2 is bits 0..7, b3 is bits 8..15
        let low16 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let val = (u32::from(b0 & 0x1F) << 24) | (u32::from(bytes[1]) << 16) | u32::from(low16);
        Ok((val, 4))
    } else if b0 == 0xE0 {
        if bytes.len() < 5 {
            return Err(SessionError::InvalidPackedDword);
        }
        let val = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        Ok((val, 5))
    } else {
        Err(SessionError::InvalidPackedDword)
    }
}

// ---------------------------------------------------------------------------
// §7.1–§7.2 Three-Key Triple DES (3DES-EDE3) CBC with Zero IV & PKCS#7 Padding
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

/// Encrypts `plaintext` with 3-Key Triple DES CBC (`IV = [0u8; 8]`) and PKCS#7 padding (§7.1, §7.2).
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

/// Decrypts `ciphertext` with 3-Key Triple DES CBC (`IV = [0u8; 8]`) and strips PKCS#7 padding (§7.1, §7.2).
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
    if pad_len == 0 || pad_len > 8 || pad_len > out.len() {
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
// §7.3 ISAAC 32-Bit PRNG for Checksum Masking
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

/// Directional 32-bit ISAAC PRNG instance for `0x02000000` checksum XOR masking (§7.3).
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
    /// Initializes an ISAAC instance from a 32-bit seed (`seed0` or `seed1`) per §7.3.1:
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

        // Pass 1: mix in randrsl[i..i+8] (all zeros per §7.3.1 step 1)
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

    /// Runs one 256-word ISAAC block generation round (§7.3.1 step 6).
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
    /// regenerating via `isaac()` when `randcnt` reaches `0` (§7.3.2).
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
// §4.3, §5.2–§5.5, §6.2–§6.7, §8.1–§8.3 Optional Headers
// ---------------------------------------------------------------------------

/// `LoginRequest` (`0x00010000`) network protocol version string (`"180804\0"`, 7 bytes, §6.2).
pub const LOGIN_NET_VERSION: [u8; 7] = *b"180804\0";

/// `authFlags` bit 0 (`0x01`): credentials payload is 3DES-CBC encrypted (§6.2.1).
pub const AUTH_FLAG_ENCRYPTED: u32 = 0x01;
/// `authFlags` bit 1 (`0x02`): second `WString` (`secondStr`) is present (§6.2.1).
pub const AUTH_FLAG_HAS_SECOND_STRING: u32 = 0x02;
/// `authFlags` bit 2 (`0x04`): length-prefixed `extraBlob` is present (§6.2.1).
pub const AUTH_FLAG_HAS_EXTRA_BLOB: u32 = 0x04;

fn encode_wstring(s: &str, out: &mut Vec<u8>) {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    encode_packed_dword(utf16.len() as u32, out);
    for unit in utf16 {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

fn decode_wstring(bytes: &[u8]) -> Result<(String, usize), SessionError> {
    let (char_count, pd_len) = decode_packed_dword(bytes)?;
    let byte_len = (char_count as usize)
        .checked_mul(2)
        .ok_or(SessionError::InvalidUtf16String)?;
    let total = pd_len
        .checked_add(byte_len)
        .ok_or(SessionError::InvalidUtf16String)?;
    if bytes.len() < total {
        return Err(SessionError::InvalidUtf16String);
    }
    let mut units = Vec::with_capacity(char_count.min(4096) as usize);
    for &chunk in bytes[pd_len..total].as_chunks::<2>().0 {
        units.push(u16::from_le_bytes(chunk));
    }
    let s = String::from_utf16(&units).map_err(|_| SessionError::InvalidUtf16String)?;
    Ok((s, total))
}

/// Decoded credentials/ticket fields of a `LoginRequest` (`0x00010000`) `authBlock` (§6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCredentials {
    /// Primary UTF-16LE string (`accountOrTicketStr`).
    pub account_or_ticket: String,
    /// Optional second UTF-16LE string (`secondStr`, present when `authFlags & 0x02 != 0`).
    pub second_string: Option<String>,
    /// Optional length-prefixed byte array (`extraBlob`, present when `authFlags & 0x04 != 0`).
    pub extra_blob: Option<Vec<u8>>,
}

impl LoginCredentials {
    /// Serializes the Case A plaintext credentials buffer (§6.2.1).
    #[must_use]
    pub fn encode_case_a(&self) -> Vec<u8> {
        let mut buf = Vec::new();
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

    /// Decodes a Case A plaintext credentials buffer given `auth_flags` (§6.2.1).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if any UTF-16LE string or byte array is truncated.
    pub fn decode_case_a(bytes: &[u8], auth_flags: u32) -> Result<(Self, usize), SessionError> {
        let mut cursor = 0usize;
        let (account_or_ticket, n1) = decode_wstring(&bytes[cursor..])?;
        cursor += n1;
        let second_string = if (auth_flags & AUTH_FLAG_HAS_SECOND_STRING) != 0 {
            let (s2, n2) = decode_wstring(&bytes[cursor..])?;
            cursor += n2;
            Some(s2)
        } else {
            None
        };
        let extra_blob = if (auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB) != 0 {
            if bytes.len() < cursor + 4 {
                return Err(SessionError::InvalidAuthBlock);
            }
            let len = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            if bytes.len() < cursor.saturating_add(len) {
                return Err(SessionError::InvalidAuthBlock);
            }
            let blob = bytes[cursor..cursor + len].to_vec();
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

/// Authentication payload inside `LoginRequestHeader` (§6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginAuthPayload {
    /// Case A (`(authFlags & 0x01) == 0`): unencrypted `LoginCredentials`.
    Unencrypted(LoginCredentials),
    /// Case B (`(authFlags & 0x01) != 0`): `"3des"` key-transport + 3DES-CBC encrypted `LoginCredentials`.
    Encrypted {
        /// Wrapped or raw 24-byte 3DES session key (`encryptedSessionKey`).
        encrypted_session_key: Vec<u8>,
        /// 24-byte symmetric 3DES key used to encrypt/decrypt `credentials`.
        des3_key: [u8; 24],
        /// Plaintext credentials that are encrypted into `ciphertext` on the wire.
        credentials: LoginCredentials,
    },
    /// Case B wire representation before caller supplies an unwrapped 3DES key (when RSA-wrapped).
    EncryptedWire {
        /// Cipher name (`"3des"`).
        cipher_name: String,
        /// Wrapped or raw session key bytes.
        encrypted_session_key: Vec<u8>,
        /// Declared symmetric key length (`0x18` = `24`).
        symmetric_key_len: u32,
        /// Raw 3DES-CBC ciphertext bytes.
        ciphertext: Vec<u8>,
    },
}

/// `LoginRequest` (`0x00010000`) optional header (§6.2, §6.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRequestHeader {
    /// Authentication type / client account type identifier (`u32` LE at `authBlock + 0x00`).
    pub auth_type: u32,
    /// Bitmask controlling optional fields and encryption (`u32` LE at `authBlock + 0x04`).
    pub auth_flags: u32,
    /// 32-bit Unix timestamp in seconds (`u32` LE at `authBlock + 0x08`).
    pub client_timestamp: u32,
    /// Credentials payload (Case A unencrypted or Case B 3DES-CBC encrypted).
    pub payload: LoginAuthPayload,
}

impl LoginRequestHeader {
    /// Serializes this `LoginRequest` (`0x00010000`) optional header into `out` (§6.2, §6.2.1):
    /// writes `"180804\0"` (`7` B), `authBlockPaddedLen` (`u32` LE), and `authBlock` padded
    /// with zeros to `authBlockPaddedLen` (`(rawLen + 3) & !3`).
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        let start = out.len();
        let mut auth_block = Vec::new();
        auth_block.extend_from_slice(&self.auth_type.to_le_bytes());

        let mut flags = self.auth_flags;
        match &self.payload {
            LoginAuthPayload::Unencrypted(creds) => {
                flags &= !AUTH_FLAG_ENCRYPTED;
                if creds.second_string.is_some() {
                    flags |= AUTH_FLAG_HAS_SECOND_STRING;
                }
                if creds.extra_blob.is_some() {
                    flags |= AUTH_FLAG_HAS_EXTRA_BLOB;
                }
                auth_block.extend_from_slice(&flags.to_le_bytes());
                auth_block.extend_from_slice(&self.client_timestamp.to_le_bytes());
                auth_block.extend_from_slice(&creds.encode_case_a());
            }
            LoginAuthPayload::Encrypted {
                encrypted_session_key,
                des3_key,
                credentials,
            } => {
                flags |= AUTH_FLAG_ENCRYPTED;
                if credentials.second_string.is_some() {
                    flags |= AUTH_FLAG_HAS_SECOND_STRING;
                }
                if credentials.extra_blob.is_some() {
                    flags |= AUTH_FLAG_HAS_EXTRA_BLOB;
                }
                auth_block.extend_from_slice(&flags.to_le_bytes());
                auth_block.extend_from_slice(&self.client_timestamp.to_le_bytes());
                // 1. cipherName PString: PackedDWORD(4) + "3des"
                encode_packed_dword(4, &mut auth_block);
                auth_block.extend_from_slice(b"3des");
                // 2. encryptedSessionKey: u32 LE len + bytes
                auth_block.extend_from_slice(&(encrypted_session_key.len() as u32).to_le_bytes());
                auth_block.extend_from_slice(encrypted_session_key);
                // 3. symmetricKeyLen: u32 LE = 24 (0x18)
                auth_block.extend_from_slice(&24u32.to_le_bytes());
                // 4 & 5. ciphertextLen (u32 LE) + 3DES-CBC ciphertext
                let plain = credentials.encode_case_a();
                let ciphertext = des3_cbc_encrypt(des3_key, &plain);
                auth_block.extend_from_slice(&(ciphertext.len() as u32).to_le_bytes());
                auth_block.extend_from_slice(&ciphertext);
            }
            LoginAuthPayload::EncryptedWire {
                cipher_name,
                encrypted_session_key,
                symmetric_key_len,
                ciphertext,
            } => {
                flags |= AUTH_FLAG_ENCRYPTED;
                auth_block.extend_from_slice(&flags.to_le_bytes());
                auth_block.extend_from_slice(&self.client_timestamp.to_le_bytes());
                encode_packed_dword(cipher_name.len() as u32, &mut auth_block);
                auth_block.extend_from_slice(cipher_name.as_bytes());
                auth_block.extend_from_slice(&(encrypted_session_key.len() as u32).to_le_bytes());
                auth_block.extend_from_slice(encrypted_session_key);
                auth_block.extend_from_slice(&symmetric_key_len.to_le_bytes());
                auth_block.extend_from_slice(&(ciphertext.len() as u32).to_le_bytes());
                auth_block.extend_from_slice(ciphertext);
            }
        }

        let padded_len = (auth_block.len() + 3) & !3;
        auth_block.resize(padded_len, 0x00);

        out.extend_from_slice(&LOGIN_NET_VERSION);
        out.extend_from_slice(&(padded_len as u32).to_le_bytes());
        out.extend_from_slice(&auth_block);
        Ok(out.len() - start)
    }

    /// Decodes a `LoginRequest` (`0x00010000`) optional header from `bytes` (§6.2, §6.2.1),
    /// returning `(header, total_bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the version string is not `"180804\0"` or `authBlock` is malformed.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        if bytes.len() < 11 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_LOGIN_REQUEST,
            });
        }
        if bytes[0..7] != LOGIN_NET_VERSION {
            return Err(SessionError::InvalidLoginRequestVersion);
        }
        let padded_len = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
        if !padded_len.is_multiple_of(4) || padded_len < 12 {
            return Err(SessionError::InvalidAuthBlock);
        }
        let total_len = 11usize
            .checked_add(padded_len)
            .ok_or(SessionError::InvalidAuthBlock)?;
        if bytes.len() < total_len {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_LOGIN_REQUEST,
            });
        }
        let block = &bytes[11..total_len];
        let auth_type = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let auth_flags = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        let client_timestamp = u32::from_le_bytes([block[8], block[9], block[10], block[11]]);

        let payload = if (auth_flags & AUTH_FLAG_ENCRYPTED) == 0 {
            let (creds, _) = LoginCredentials::decode_case_a(&block[12..], auth_flags)?;
            LoginAuthPayload::Unencrypted(creds)
        } else {
            let mut pos = 12usize;
            let (cname_len_u32, pd_len) = decode_packed_dword(&block[pos..])?;
            pos += pd_len;
            let cname_len = cname_len_u32 as usize;
            if block.len() < pos.saturating_add(cname_len) {
                return Err(SessionError::InvalidAuthBlock);
            }
            let cipher_name = String::from_utf8_lossy(&block[pos..pos + cname_len]).into_owned();
            pos += cname_len;
            if block.len() < pos + 4 {
                return Err(SessionError::InvalidAuthBlock);
            }
            let ek_len =
                u32::from_le_bytes([block[pos], block[pos + 1], block[pos + 2], block[pos + 3]])
                    as usize;
            pos += 4;
            if block.len() < pos.saturating_add(ek_len).saturating_add(8) {
                return Err(SessionError::InvalidAuthBlock);
            }
            let encrypted_session_key = block[pos..pos + ek_len].to_vec();
            pos += ek_len;
            let symmetric_key_len =
                u32::from_le_bytes([block[pos], block[pos + 1], block[pos + 2], block[pos + 3]]);
            pos += 4;
            let ct_len =
                u32::from_le_bytes([block[pos], block[pos + 1], block[pos + 2], block[pos + 3]])
                    as usize;
            pos += 4;
            if block.len() < pos.saturating_add(ct_len) {
                return Err(SessionError::InvalidAuthBlock);
            }
            let ciphertext = block[pos..pos + ct_len].to_vec();

            // If no RSA wrapping was used (encrypted_session_key is the raw 24-byte 3DES key),
            // automatically decrypt Case B credentials (§6.2.1 Case B item 2).
            if encrypted_session_key.len() == 24 && symmetric_key_len == 24 {
                let mut des3_key = [0u8; 24];
                des3_key.copy_from_slice(&encrypted_session_key);
                let plain = des3_cbc_decrypt(&des3_key, &ciphertext)?;
                let (credentials, _) = LoginCredentials::decode_case_a(&plain, auth_flags)?;
                LoginAuthPayload::Encrypted {
                    encrypted_session_key,
                    des3_key,
                    credentials,
                }
            } else {
                LoginAuthPayload::EncryptedWire {
                    cipher_name,
                    encrypted_session_key,
                    symmetric_key_len,
                    ciphertext,
                }
            }
        };

        Ok((
            Self {
                auth_type,
                auth_flags,
                client_timestamp,
                payload,
            },
            total_len,
        ))
    }
}

/// `WorldLoginRequest` (`0x00020000`, 8 bytes) optional header (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorldLoginRequestHeader {
    /// 64-bit referral cookie (`u64` LE at `+0x00`).
    pub referral_cookie: u64,
}

/// `ConnectRequest` (`0x00040000`, 44 bytes) optional header (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectRequestHeader {
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
    /// 32-bit connection parameter (`u32` LE at `+0x14`).
    pub unknown_14: u32,
    /// 16-bit connection table index echoed in `ConnectResponse` (`u16` LE at `+0x18`).
    pub client_conn_idx: u16,
    /// 2-byte reserved/padding field (`u16` LE at `+0x1A`).
    pub unknown_1a: u16,
    /// 16-byte UUID selecting the per-connection packet codec (`+0x1C..+0x2C`).
    pub codec_uuid: [u8; 16],
}

impl Default for ConnectRequestHeader {
    fn default() -> Self {
        Self {
            server_time: 0.0,
            assigned_client_rec_id: 0,
            server_rec_id: 0,
            outgoing_seed: 0,
            incoming_seed: 0,
            unknown_14: 0,
            client_conn_idx: 0,
            unknown_1a: 0,
            codec_uuid: [0u8; 16],
        }
    }
}

impl ConnectRequestHeader {
    /// Serializes this 44-byte `ConnectRequest` optional header into `out` (§6.3).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.server_time.to_le_bytes());
        out.extend_from_slice(&self.assigned_client_rec_id.to_le_bytes());
        out.extend_from_slice(&self.server_rec_id.to_le_bytes());
        // Outgoing & incoming ISAAC seeds at +0x0C and +0x10 are big-endian on wire (§6.3, §11.2)
        out.extend_from_slice(&self.outgoing_seed.to_be_bytes());
        out.extend_from_slice(&self.incoming_seed.to_be_bytes());
        out.extend_from_slice(&self.unknown_14.to_le_bytes());
        out.extend_from_slice(&self.client_conn_idx.to_le_bytes());
        out.extend_from_slice(&self.unknown_1a.to_le_bytes());
        out.extend_from_slice(&self.codec_uuid);
    }

    /// Decodes a 44-byte `ConnectRequest` optional header from `bytes` (§6.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 44`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 44 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_CONNECT_REQUEST,
            });
        }
        let server_time = f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let assigned_client_rec_id = u16::from_le_bytes([bytes[8], bytes[9]]);
        let server_rec_id = u16::from_le_bytes([bytes[10], bytes[11]]);
        let outgoing_seed = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let incoming_seed = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let unknown_14 = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let client_conn_idx = u16::from_le_bytes([bytes[24], bytes[25]]);
        let unknown_1a = u16::from_le_bytes([bytes[26], bytes[27]]);
        let mut codec_uuid = [0u8; 16];
        codec_uuid.copy_from_slice(&bytes[28..44]);
        Ok(Self {
            server_time,
            assigned_client_rec_id,
            server_rec_id,
            outgoing_seed,
            incoming_seed,
            unknown_14,
            client_conn_idx,
            unknown_1a,
            codec_uuid,
        })
    }
}

/// `ConnectResponse` (`0x00080000`, 28 bytes) optional header (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectResponseHeader {
    /// 64-bit check value (`u64` LE at `+0x00`).
    pub check_value: u64,
    /// Client's 16-bit local connection ID (`u16` LE at `+0x08`).
    pub local_rec_id: u16,
    /// Echo of `clientConnIdx` from `ConnectRequest` (`u16` LE at `+0x0A`).
    pub client_conn_idx: u16,
    /// Echo of the 16-byte `codecUuid` selected from `ConnectRequest` (`+0x0C..+0x1C`).
    pub codec_uuid: [u8; 16],
}

impl ConnectResponseHeader {
    /// Serializes this 28-byte `ConnectResponse` optional header into `out` (§6.4).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.check_value.to_le_bytes());
        out.extend_from_slice(&self.local_rec_id.to_le_bytes());
        out.extend_from_slice(&self.client_conn_idx.to_le_bytes());
        out.extend_from_slice(&self.codec_uuid);
    }

    /// Decodes a 28-byte `ConnectResponse` optional header from `bytes` (§6.4).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 28`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 28 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_CONNECT_RESPONSE,
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

/// `Referral` (`0x00000800`, 18 bytes) optional header (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferralHeader {
    /// 64-bit one-time authentication cookie for the target world server (`u64` LE at `+0x00`).
    pub referral_cookie: u64,
    /// Address family (`2` = `AF_INET`, `u16` LE at `+0x08`).
    pub sin_family: u16,
    /// Target world server IPv4 address and UDP port (`sin_port` BE at `+0x0A`, `sin_addr` BE at `+0x0C`).
    pub target_addr: SocketAddrV4,
    /// 16-bit internal world server identifier (`u16` LE at `+0x10`).
    pub world_server_id: u16,
}

impl ReferralHeader {
    /// Serializes this 18-byte `Referral` optional header into `out` (§6.5).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.referral_cookie.to_le_bytes());
        out.extend_from_slice(&self.sin_family.to_le_bytes());
        out.extend_from_slice(&self.target_addr.port().to_be_bytes());
        out.extend_from_slice(&self.target_addr.ip().octets());
        out.extend_from_slice(&self.world_server_id.to_le_bytes());
    }

    /// Decodes an 18-byte `Referral` optional header from `bytes` (§6.5).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OptionalHeaderOverrun`] if `bytes.len() < 18`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < 18 {
            return Err(SessionError::OptionalHeaderOverrun {
                flag: FLAG_REFERRAL,
            });
        }
        let referral_cookie = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let sin_family = u16::from_le_bytes([bytes[8], bytes[9]]);
        let port = u16::from_be_bytes([bytes[10], bytes[11]]);
        let ip = Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]);
        let world_server_id = u16::from_le_bytes([bytes[16], bytes[17]]);
        Ok(Self {
            referral_cookie,
            sin_family,
            target_addr: SocketAddrV4::new(ip, port),
            world_server_id,
        })
    }
}

/// `ServerSwitch` (`0x00000100`, 8 bytes) optional header (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerSwitchHeader {
    /// Monotonic server-switch sequence number (`u32` LE at `+0x00`).
    pub switch_seq: u32,
    /// Server-switch reason/mode (`u32` LE at `+0x04`).
    pub switch_type: u32,
}

/// `LogonServerAddr` (`0x00000200`, 8 bytes) optional header (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogonServerAddrHeader {
    /// Address family (`2` = `AF_INET`, `u16` LE at `+0x00`).
    pub sin_family: u16,
    /// Logon server IPv4 address and UDP port (`sin_port` BE at `+0x02`, `sin_addr` BE at `+0x04`).
    pub addr: SocketAddrV4,
}

/// `NetError` (`0x00100000`) or `NetErrorDisconnect` (`0x00200000`, 8 bytes) optional header (§6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetErrorHeader {
    /// 32-bit internal transport/auth error identifier (`u32` LE at `+0x00`).
    pub error_code: u32,
    /// 32-bit auxiliary status or zero (`u32` LE at `+0x04`).
    pub error_subcode: u32,
}

impl NetErrorHeader {
    /// Maps `(error_code, error_subcode)` to the client's `HRESULT` per §6.7.
    #[must_use]
    pub fn to_hresult(self) -> u32 {
        map_net_error_to_hresult(self.error_code, self.error_subcode)
    }
}

/// Maps `NetError` / `NetErrorDisconnect` `(error_code, error_subcode)` to the client's `HRESULT` (§6.7).
#[must_use]
pub fn map_net_error_to_hresult(error_code: u32, _error_subcode: u32) -> u32 {
    match error_code {
        0x0000_0000 => 0x0000_0000,
        0x00A7_E948 | 0x7DC4_1B4A => 0x8004_1006,
        0x00CD_0B73 | 0x06F1_6F2E | 0x0DFB_256E => 0x8004_100F,
        0x00F9_982C => 0x8004_1016,
        0x07CF_6BF2 => 0x8004_1012,
        0x0058_3B44 => 0x8004_100E,
        _ => 0x8000_4005,
    }
}

/// `Flow` (`0x08000000`, 6 bytes) optional header (§5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlowHeader {
    /// Number of raw wire bytes received since the previous `Flow` header (`u32` LE at `+0x00`).
    pub bytes_received_delta: u32,
    /// Elapsed time in milliseconds (`u16` LE at `+0x04`).
    pub interval_ms: u16,
}

/// Decoded optional headers from a packet payload in canonical ascending bit order (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionalHeaders {
    /// Bit 8 (`0x00000100`): `ServerSwitch` (8 bytes).
    pub server_switch: Option<ServerSwitchHeader>,
    /// Bit 9 (`0x00000200`): `LogonServerAddr` (8 bytes).
    pub logon_server_addr: Option<LogonServerAddrHeader>,
    /// Bit 10 (`0x00000400`): `EmptyHeader10` (0 bytes, client -> server only).
    pub empty_header_10: bool,
    /// Bit 11 (`0x00000800`): `Referral` (18 bytes).
    pub referral: Option<ReferralHeader>,
    /// Bit 12 (`0x00001000`): `Nak` (`4 + 4 * N` bytes, `1 <= N <= 114`).
    pub nak: Option<Vec<u32>>,
    /// Bit 13 (`0x00002000`): `EmptyAck` (`4 + 4 * N` bytes, `1 <= N <= 114`).
    pub empty_ack: Option<Vec<u32>>,
    /// Bit 14 (`0x00004000`): `PakAck` (`u32` **big-endian**).
    pub pak_ack: Option<u32>,
    /// Bit 15 (`0x00008000`): `Disconnect` (0 bytes).
    pub disconnect: bool,
    /// Bit 16 (`0x00010000`): `LoginRequest` (variable length).
    pub login_request: Option<LoginRequestHeader>,
    /// Bit 17 (`0x00020000`): `WorldLoginRequest` (8 bytes).
    pub world_login_request: Option<WorldLoginRequestHeader>,
    /// Bit 18 (`0x00040000`): `ConnectRequest` (44 bytes).
    pub connect_request: Option<ConnectRequestHeader>,
    /// Bit 19 (`0x00080000`): `ConnectResponse` (28 bytes).
    pub connect_response: Option<ConnectResponseHeader>,
    /// Bit 20 (`0x00100000`): `NetError` (8 bytes).
    pub net_error: Option<NetErrorHeader>,
    /// Bit 21 (`0x00200000`): `NetErrorDisconnect` (8 bytes).
    pub net_error_disconnect: Option<NetErrorHeader>,
    /// Bit 22 (`0x00400000`): `CiceroControl` (`u32` LE, 4 bytes; client -> server only, §4.3).
    pub cicero_control: Option<u32>,
    /// Bit 24 (`0x01000000`): `TimeSync` (`f64` LE, 8 bytes).
    pub time_sync: Option<f64>,
    /// Bit 25 (`0x02000000`): `EchoRequest` (`f32` LE, 4 bytes; send-only optional header).
    pub echo_request: Option<f32>,
    /// Bit 26 (`0x04000000`): `EchoResponse` (`[f32; 2]` LE, 8 bytes; send-only optional header).
    pub echo_response: Option<[f32; 2]>,
    /// Bit 27 (`0x08000000`): `Flow` (6 bytes).
    pub flow: Option<FlowHeader>,
}

impl OptionalHeaders {
    /// Returns the bitmask of optional header flags present in `self` (§4.3).
    #[must_use]
    pub fn flags_mask(&self) -> u32 {
        let mut mask = 0u32;
        if self.server_switch.is_some() {
            mask |= FLAG_SERVER_SWITCH;
        }
        if self.logon_server_addr.is_some() {
            mask |= FLAG_LOGON_SERVER_ADDR;
        }
        if self.empty_header_10 {
            mask |= FLAG_EMPTY_HEADER_10;
        }
        if self.referral.is_some() {
            mask |= FLAG_REFERRAL;
        }
        if self.nak.is_some() {
            mask |= FLAG_NAK;
        }
        if self.empty_ack.is_some() {
            mask |= FLAG_EMPTY_ACK;
        }
        if self.pak_ack.is_some() {
            mask |= FLAG_PAK_ACK;
        }
        if self.disconnect {
            mask |= FLAG_DISCONNECT;
        }
        if self.login_request.is_some() {
            mask |= FLAG_LOGIN_REQUEST;
        }
        if self.world_login_request.is_some() {
            mask |= FLAG_WORLD_LOGIN_REQUEST;
        }
        if self.connect_request.is_some() {
            mask |= FLAG_CONNECT_REQUEST;
        }
        if self.connect_response.is_some() {
            mask |= FLAG_CONNECT_RESPONSE;
        }
        if self.net_error.is_some() {
            mask |= FLAG_NET_ERROR;
        }
        if self.net_error_disconnect.is_some() {
            mask |= FLAG_NET_ERROR_DISCONNECT;
        }
        if self.cicero_control.is_some() {
            mask |= FLAG_CICERO_CONTROL;
        }
        if self.time_sync.is_some() {
            mask |= FLAG_TIME_SYNC;
        }
        if self.echo_request.is_some() {
            mask |= FLAG_ECHO_REQUEST;
        }
        if self.echo_response.is_some() {
            mask |= FLAG_ECHO_RESPONSE;
        }
        if self.flow.is_some() {
            mask |= FLAG_FLOW;
        }
        mask
    }

    /// Serializes all present optional headers into `out` in strict ascending bit order (`0x00000100`..`0x08000000`, §4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if `Nak` or `EmptyAck` count is `0` or `> 114`.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<usize, SessionError> {
        let start = out.len();
        // Bit 8: 0x00000100 ServerSwitch (8 B)
        if let Some(ss) = self.server_switch {
            out.extend_from_slice(&ss.switch_seq.to_le_bytes());
            out.extend_from_slice(&ss.switch_type.to_le_bytes());
        }
        // Bit 9: 0x00000200 LogonServerAddr (8 B)
        if let Some(lsa) = self.logon_server_addr {
            out.extend_from_slice(&lsa.sin_family.to_le_bytes());
            out.extend_from_slice(&lsa.addr.port().to_be_bytes());
            out.extend_from_slice(&lsa.addr.ip().octets());
        }
        // Bit 11: 0x00000800 Referral (18 B)
        if let Some(ref_hdr) = &self.referral {
            ref_hdr.encode(out);
        }
        // Bit 12: 0x00001000 Nak (4 + 4*N B, LE)
        if let Some(seqs) = &self.nak {
            if seqs.is_empty() || seqs.len() > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidNakCount {
                    count: seqs.len() as u32,
                });
            }
            out.extend_from_slice(&(seqs.len() as u32).to_le_bytes());
            for &s in seqs {
                out.extend_from_slice(&s.to_le_bytes());
            }
        }
        // Bit 13: 0x00002000 EmptyAck (4 + 4*N B, LE)
        if let Some(seqs) = &self.empty_ack {
            if seqs.is_empty() || seqs.len() > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidEmptyAckCount {
                    count: seqs.len() as u32,
                });
            }
            out.extend_from_slice(&(seqs.len() as u32).to_le_bytes());
            for &s in seqs {
                out.extend_from_slice(&s.to_le_bytes());
            }
        }
        // Bit 14: 0x00004000 PakAck (4 B, big-endian!)
        if let Some(ack) = self.pak_ack {
            out.extend_from_slice(&ack.to_be_bytes());
        }
        // Bit 15: 0x00008000 Disconnect (0 B)
        // Bit 16: 0x00010000 LoginRequest (variable)
        if let Some(lr) = &self.login_request {
            lr.encode(out)?;
        }
        // Bit 17: 0x00020000 WorldLoginRequest (8 B)
        if let Some(wlr) = self.world_login_request {
            out.extend_from_slice(&wlr.referral_cookie.to_le_bytes());
        }
        // Bit 18: 0x00040000 ConnectRequest (44 B)
        if let Some(cr) = &self.connect_request {
            cr.encode(out);
        }
        // Bit 19: 0x00080000 ConnectResponse (28 B)
        if let Some(resp) = &self.connect_response {
            resp.encode(out);
        }
        // Bit 20: 0x00100000 NetError (8 B)
        if let Some(ne) = self.net_error {
            out.extend_from_slice(&ne.error_code.to_le_bytes());
            out.extend_from_slice(&ne.error_subcode.to_le_bytes());
        }
        // Bit 21: 0x00200000 NetErrorDisconnect (8 B)
        if let Some(ned) = self.net_error_disconnect {
            out.extend_from_slice(&ned.error_code.to_le_bytes());
            out.extend_from_slice(&ned.error_subcode.to_le_bytes());
        }
        // Bit 22: 0x00400000 CiceroControl (4 B u32 LE, Client -> Server only)
        if let Some(cc) = self.cicero_control {
            out.extend_from_slice(&cc.to_le_bytes());
        }
        // Bit 24: 0x01000000 TimeSync (8 B f64 LE)
        if let Some(ts) = self.time_sync {
            out.extend_from_slice(&ts.to_le_bytes());
        }
        // Bit 25: 0x02000000 EchoRequest (4 B f32 LE)
        if let Some(er) = self.echo_request {
            out.extend_from_slice(&er.to_le_bytes());
        }
        // Bit 26: 0x04000000 EchoResponse (8 B 2x f32 LE)
        if let Some([t0, t1]) = self.echo_response {
            out.extend_from_slice(&t0.to_le_bytes());
            out.extend_from_slice(&t1.to_le_bytes());
        }
        // Bit 27: 0x08000000 Flow (6 B)
        if let Some(fl) = self.flow {
            out.extend_from_slice(&fl.bytes_received_delta.to_le_bytes());
            out.extend_from_slice(&fl.interval_ms.to_le_bytes());
        }
        Ok(out.len() - start)
    }

    /// Decodes optional headers from `payload` in strict ascending bit order (§4.3, §10.1 rules 3, 7).
    ///
    /// On the client receive path (`from_server == true`):
    /// - Rejects any wire flags outside `0x0F7FFB00` (`(flags & 0xF08004FF) != 0`, §10.1 rule 3).
    /// - Rejects server-forbidden flag `0x00400000` (`CiceroControl`, §10.1 rule 7).
    /// - Masks `flags` with `RECV_OPT_HEADER_FLAGS_MASK` (`0x093FFB00`) so bits 25 (`EncryptedChecksum`)
    ///   and 26 (`HasNetBlobs`) are not treated as `EchoRequest`/`EchoResponse` optional headers (§4.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if forbidden flags are set, a header overruns `payload`,
    /// or `Nak`/`EmptyAck` counts are outside `1..=114`.
    pub fn decode(
        flags: u32,
        payload: &[u8],
        from_server: bool,
    ) -> Result<(Self, usize), SessionError> {
        if (flags & INVALID_PROTO_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }
        if from_server && (flags & SERVER_FORBIDDEN_OPT_FLAGS_MASK) != 0 {
            return Err(SessionError::InvalidHeaderFlags { flags });
        }

        let mask = if from_server {
            RECV_OPT_HEADER_FLAGS_MASK
        } else {
            RECV_OPT_HEADER_FLAGS_MASK | FLAG_CICERO_CONTROL
        };
        let active = flags & mask;
        let mut hdrs = Self::default();
        let mut cursor = 0usize;

        let need = |cursor: usize, n: usize, flag: u32| -> Result<(), SessionError> {
            if cursor.saturating_add(n) > payload.len() {
                return Err(SessionError::OptionalHeaderOverrun { flag });
            }
            Ok(())
        };

        // Bit 8: 0x00000100 ServerSwitch (8 B)
        if (active & FLAG_SERVER_SWITCH) != 0 {
            need(cursor, 8, FLAG_SERVER_SWITCH)?;
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
            hdrs.server_switch = Some(ServerSwitchHeader {
                switch_seq,
                switch_type,
            });
            cursor += 8;
        }
        // Bit 9: 0x00000200 LogonServerAddr (8 B)
        if (active & FLAG_LOGON_SERVER_ADDR) != 0 {
            need(cursor, 8, FLAG_LOGON_SERVER_ADDR)?;
            let sin_family = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]);
            let port = u16::from_be_bytes([payload[cursor + 2], payload[cursor + 3]]);
            let ip = Ipv4Addr::new(
                payload[cursor + 4],
                payload[cursor + 5],
                payload[cursor + 6],
                payload[cursor + 7],
            );
            hdrs.logon_server_addr = Some(LogonServerAddrHeader {
                sin_family,
                addr: SocketAddrV4::new(ip, port),
            });
            cursor += 8;
        }
        // Bit 10: 0x00000400 EmptyHeader10 (0 B, only allowed when from_server == false)
        if (active & FLAG_EMPTY_HEADER_10) != 0 {
            hdrs.empty_header_10 = true;
        }
        // Bit 11: 0x00000800 Referral (18 B)
        if (active & FLAG_REFERRAL) != 0 {
            hdrs.referral = Some(ReferralHeader::decode(&payload[cursor..])?);
            cursor += 18;
        }
        // Bit 12: 0x00001000 Nak (4 + 4*N B)
        if (active & FLAG_NAK) != 0 {
            need(cursor, 4, FLAG_NAK)?;
            let count = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            if count == 0 || (count as usize) > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidNakCount { count });
            }
            let n = count as usize;
            need(cursor, 4 + n * 4, FLAG_NAK)?;
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
            hdrs.nak = Some(list);
        }
        // Bit 13: 0x00002000 EmptyAck (4 + 4*N B)
        if (active & FLAG_EMPTY_ACK) != 0 {
            need(cursor, 4, FLAG_EMPTY_ACK)?;
            let count = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            if count == 0 || (count as usize) > MAX_NAK_ENTRIES {
                return Err(SessionError::InvalidEmptyAckCount { count });
            }
            let n = count as usize;
            need(cursor, 4 + n * 4, FLAG_EMPTY_ACK)?;
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
            hdrs.empty_ack = Some(list);
        }
        // Bit 14: 0x00004000 PakAck (4 B big-endian)
        if (active & FLAG_PAK_ACK) != 0 {
            need(cursor, 4, FLAG_PAK_ACK)?;
            let ack = u32::from_be_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            hdrs.pak_ack = Some(ack);
            cursor += 4;
        }
        // Bit 15: 0x00008000 Disconnect (0 B)
        if (active & FLAG_DISCONNECT) != 0 {
            hdrs.disconnect = true;
        }
        // Bit 16: 0x00010000 LoginRequest (variable)
        if (active & FLAG_LOGIN_REQUEST) != 0 {
            let (lr, consumed) = LoginRequestHeader::decode(&payload[cursor..])?;
            hdrs.login_request = Some(lr);
            cursor += consumed;
        }
        // Bit 17: 0x00020000 WorldLoginRequest (8 B)
        if (active & FLAG_WORLD_LOGIN_REQUEST) != 0 {
            need(cursor, 8, FLAG_WORLD_LOGIN_REQUEST)?;
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
            hdrs.world_login_request = Some(WorldLoginRequestHeader { referral_cookie });
            cursor += 8;
        }
        // Bit 18: 0x00040000 ConnectRequest (44 B)
        if (active & FLAG_CONNECT_REQUEST) != 0 {
            hdrs.connect_request = Some(ConnectRequestHeader::decode(&payload[cursor..])?);
            cursor += 44;
        }
        // Bit 19: 0x00080000 ConnectResponse (28 B)
        if (active & FLAG_CONNECT_RESPONSE) != 0 {
            hdrs.connect_response = Some(ConnectResponseHeader::decode(&payload[cursor..])?);
            cursor += 28;
        }
        // Bit 20: 0x00100000 NetError (8 B)
        if (active & FLAG_NET_ERROR) != 0 {
            need(cursor, 8, FLAG_NET_ERROR)?;
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
            hdrs.net_error = Some(NetErrorHeader {
                error_code,
                error_subcode,
            });
            cursor += 8;
        }
        // Bit 21: 0x00200000 NetErrorDisconnect (8 B)
        if (active & FLAG_NET_ERROR_DISCONNECT) != 0 {
            need(cursor, 8, FLAG_NET_ERROR_DISCONNECT)?;
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
            hdrs.net_error_disconnect = Some(NetErrorHeader {
                error_code,
                error_subcode,
            });
            cursor += 8;
        }
        // Bit 22: 0x00400000 CiceroControl (4 B u32 LE, only when from_server == false, §4.3)
        if (active & FLAG_CICERO_CONTROL) != 0 {
            need(cursor, 4, FLAG_CICERO_CONTROL)?;
            let cc = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            hdrs.cicero_control = Some(cc);
            cursor += 4;
        }
        // Bit 24: 0x01000000 TimeSync (8 B)
        if (active & FLAG_TIME_SYNC) != 0 {
            need(cursor, 8, FLAG_TIME_SYNC)?;
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
            hdrs.time_sync = Some(ts);
            cursor += 8;
        }
        // Bit 27: 0x08000000 Flow (6 B)
        if (active & FLAG_FLOW) != 0 {
            need(cursor, 6, FLAG_FLOW)?;
            let bytes_received_delta = u32::from_le_bytes([
                payload[cursor],
                payload[cursor + 1],
                payload[cursor + 2],
                payload[cursor + 3],
            ]);
            let interval_ms = u16::from_le_bytes([payload[cursor + 4], payload[cursor + 5]]);
            hdrs.flow = Some(FlowHeader {
                bytes_received_delta,
                interval_ms,
            });
            cursor += 6;
        }

        Ok((hdrs, cursor))
    }
}

// ---------------------------------------------------------------------------
// §5.6.2–§5.6.4 NetBlob Fragment Header, Splitting, and Bounded Reassembly
// ---------------------------------------------------------------------------

/// `blobFlags` bit 0 (`0x01`): 28-byte attachment header follows the fragment header (§5.6.3).
pub const BLOB_FLAG_HAS_ATTACHMENT: u8 = 0x01;
/// `blobFlags` bit 3 (`0x08`): `NetBlob` payload is 3DES-CBC encrypted (§5.6.3, §7.2).
pub const BLOB_FLAG_ENCRYPTED: u8 = 0x08;
/// `blobFlags` bit 4 (`0x10`): `NetBlob` is split across `numFrags > 1` fragments (§5.6.3).
pub const BLOB_FLAG_IS_FRAGMENTED: u8 = 0x10;
/// `blobFlags` bit 6 (`0x40`): `recipientId` defaults to `2` (`0x0000000000000002`, §5.6.3).
pub const BLOB_FLAG_DEFAULT_RECIPIENT: u8 = 0x40;
/// `blobFlags` bit 7 (`0x80`): omit `blobHeaderWord` and `fragDataLen` fields (§5.6.3).
pub const BLOB_FLAG_OMIT_CHUNK_INFO: u8 = 0x80;

/// Default 64-bit recipient ID when `blobFlags & 0x40` is set (§5.6.3).
pub const DEFAULT_BLOB_RECIPIENT_ID: u64 = 2;

/// Optional 28-byte fragment attachment (`blobFlags & 0x01`, field 10 in §5.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FragmentAttachment {
    /// Three 64-bit little-endian words (`24` B).
    pub words64: [u64; 3],
    /// Trailing 32-bit little-endian word (`4` B).
    pub word32: u32,
}

/// Compact variable-length `NetBlob` fragment header (fields 1–10 in §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetBlobFragmentHeader {
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
    /// Total number of fragments in this `NetBlob` (`>= 1`, field 5b).
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

impl NetBlobFragmentHeader {
    /// Serializes this `NetBlob` fragment header into `out`, appending the `0x00` even-byte
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
            encode_packed_dword(u32::from(self.seq_group), out);
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
            encode_packed_dword(self.frag_index, out);
            encode_packed_dword(self.num_frags, out);
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
                encode_packed_dword(self.blob_header_word.unwrap_or(0), out);
            }
            encode_packed_dword(self.frag_data_len, out);
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

    /// Decodes a compact variable-length `NetBlob` fragment header from `bytes` (§5.6.3, §10.1 rule 8),
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
            let (sg, n) = decode_packed_dword(&bytes[cursor..])?;
            cursor += n;
            sg as u16
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
            let (fi, n1) = decode_packed_dword(&bytes[cursor..])?;
            cursor += n1;
            let (nf, n2) = decode_packed_dword(&bytes[cursor..])?;
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
                let (w, n) = decode_packed_dword(&bytes[cursor..])?;
                cursor += n;
                Some(w)
            } else {
                None
            };
            let (fdl, n) = decode_packed_dword(&bytes[cursor..])?;
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

/// A single `NetBlob` fragment (header + payload slice, §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetBlobFragment {
    /// Decoded fragment header.
    pub header: NetBlobFragmentHeader,
    /// Fragment payload slice (`header.frag_data_len` bytes).
    pub payload: Vec<u8>,
}

impl NetBlobFragment {
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

    /// Decodes a single `NetBlobFragment` (header + payload) from `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the header is invalid or the payload slice overruns `bytes`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), SessionError> {
        let (header, h_len) = NetBlobFragmentHeader::decode(bytes)?;
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

/// Splits an outgoing `NetBlob` payload into `ceil(L / 436)` fragments (or 1 empty fragment
/// when `L == 0`) per §5.6.4 and §7.2 (encrypting first with 3DES-CBC if `encrypted == true`).
///
/// # Errors
///
/// Returns [`SessionError`] if `queue_id` is outside `1..=4`, `encrypted` is set without `des3_key`,
/// or the payload exceeds `MAX_NETBLOB_BYTES`.
#[allow(clippy::too_many_arguments)]
pub fn split_net_blob(
    queue_id: u8,
    order_num: u16,
    blob_id: u16,
    seq_group: u16,
    recipient_id: Option<u64>,
    blob_header_word: u32,
    payload: &[u8],
    encrypted: bool,
    des3_key: Option<&[u8; 24]>,
) -> Result<Vec<NetBlobFragment>, SessionError> {
    if !(1..=4).contains(&queue_id) {
        return Err(SessionError::InvalidQueueId { queue_id });
    }
    let effective_payload: Vec<u8> = if encrypted {
        let key = des3_key.ok_or(SessionError::MissingSessionCipher)?;
        des3_cbc_encrypt(key, payload)
    } else {
        payload.to_vec()
    };

    if effective_payload.len() > MAX_NETBLOB_BYTES {
        return Err(SessionError::BlobTooLarge {
            total_bytes: effective_payload.len(),
            limit: MAX_NETBLOB_BYTES,
        });
    }

    let num_frags = if effective_payload.is_empty() {
        1u32
    } else {
        effective_payload.len().div_ceil(MAX_BLOB_FRAG_PAYLOAD) as u32
    };

    let mut base_flags = 0u8;
    if encrypted {
        base_flags |= BLOB_FLAG_ENCRYPTED;
    }
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
        let header = NetBlobFragmentHeader {
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
        out.push(NetBlobFragment {
            header,
            payload: slice.to_vec(),
        });
    }
    Ok(out)
}

/// Completed, reassembled (and if `blobFlags & 0x08`, 3DES-CBC decrypted) `NetBlob` (§5.6.4, §7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReassembledNetBlob {
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
    /// True if `blobFlags & 0x08` (`EncryptedBlob`) was set and decrypted (marks `NetMessage` with `0x80`, §7.2).
    pub was_encrypted: bool,
    /// Reassembled (and decrypted if `was_encrypted`) `NetBlob` payload bytes.
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct IncompleteBlob {
    queue_id: u8,
    order_num: u16,
    seq_group: u16,
    blob_id: u16,
    recipient_id: u64,
    blob_flags: u8,
    num_frags: u32,
    blob_header_word: Option<u32>,
    received_mask: Vec<u32>,
    received_count: u32,
    buffer: Vec<u8>,
    trimmed_len: Option<usize>,
    created_at_sec: f64,
}

/// Bounded `NetBlob` fragment reassembler with `30.0`-second expiration timer (§5.6.4, §7.2).
#[derive(Debug, Clone)]
pub struct NetBlobReassembler {
    incomplete: HashMap<(u16, u8, u16), IncompleteBlob>,
    max_concurrent_blobs: usize,
    max_blob_bytes: usize,
}

impl Default for NetBlobReassembler {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_CONCURRENT_BLOBS,
            DEFAULT_MAX_REASSEMBLY_BLOB_BYTES,
        )
    }
}

impl NetBlobReassembler {
    /// Creates a bounded `NetBlobReassembler` with configurable caps on concurrent incomplete
    /// blobs and per-blob byte size (clamped to `MAX_NETBLOB_BYTES`, §5.6.4).
    #[must_use]
    pub fn new(max_concurrent_blobs: usize, max_blob_bytes: usize) -> Self {
        Self {
            incomplete: HashMap::new(),
            max_concurrent_blobs: max_concurrent_blobs.max(1),
            max_blob_bytes: max_blob_bytes.min(MAX_NETBLOB_BYTES),
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

    /// Ingests a single `NetBlobFragment` from `sender_rec_id` at `now_sec` (§5.6.4, §7.2).
    ///
    /// Returns `Ok(Some(ReassembledNetBlob))` when all fragments of the blob have arrived
    /// (immediately for `numFrags == 1`), or `Ok(None)` when waiting for remaining fragments.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragment bounds or total blob size limits are violated,
    /// or if 3DES-CBC decryption fails on an encrypted blob (`blobFlags & 0x08`).
    pub fn ingest(
        &mut self,
        sender_rec_id: u16,
        frag: NetBlobFragment,
        now_sec: f64,
        des3_key: Option<&[u8; 24]>,
    ) -> Result<Option<ReassembledNetBlob>, SessionError> {
        let hdr = &frag.header;
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
        if min_final_size > MAX_NETBLOB_BYTES {
            return Err(SessionError::BlobTooLarge {
                total_bytes: min_final_size,
                limit: MAX_NETBLOB_BYTES,
            });
        }
        let alloc_size = (hdr.num_frags as usize).saturating_mul(MAX_BLOB_FRAG_PAYLOAD);
        if min_final_size > self.max_blob_bytes || alloc_size > self.max_blob_bytes {
            return Err(SessionError::BlobTooLarge {
                total_bytes: min_final_size.max(alloc_size),
                limit: self.max_blob_bytes,
            });
        }

        // Fast path for single-fragment NetBlobs (numFrags == 1, fragIndex == 0, §5.6.4 rule 2)
        if hdr.num_frags == 1 {
            let was_encrypted = (hdr.blob_flags & BLOB_FLAG_ENCRYPTED) != 0;
            let payload = if was_encrypted {
                let key = des3_key.ok_or(SessionError::MissingSessionCipher)?;
                des3_cbc_decrypt(key, &frag.payload)?
            } else {
                frag.payload
            };
            return Ok(Some(ReassembledNetBlob {
                queue_id: hdr.queue_id,
                order_num: hdr.order_num,
                seq_group: hdr.seq_group,
                blob_id: hdr.blob_id,
                recipient_id: hdr.recipient_id,
                blob_header_word: hdr.blob_header_word.unwrap_or(0),
                was_encrypted,
                payload,
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
                blob_flags: hdr.blob_flags,
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
            let was_encrypted = (completed.blob_flags & BLOB_FLAG_ENCRYPTED) != 0;
            let payload = if was_encrypted {
                let key = des3_key.ok_or(SessionError::MissingSessionCipher)?;
                des3_cbc_decrypt(key, &completed.buffer)?
            } else {
                completed.buffer
            };
            return Ok(Some(ReassembledNetBlob {
                queue_id: completed.queue_id,
                order_num: completed.order_num,
                seq_group: completed.seq_group,
                blob_id: completed.blob_id,
                recipient_id: completed.recipient_id,
                blob_header_word: completed.blob_header_word.unwrap_or(0),
                was_encrypted,
                payload,
            }));
        }

        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// §4.1–§4.4 & §10.1 Full UDP Datagram Framing (Encode & Decode)
// ---------------------------------------------------------------------------

/// A fully decoded UDP datagram (`wirePrefix` + `ProtoHeader` + `OptionalHeaders` + `NetBlobFragment`s).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPacket {
    /// Two-byte wire prefix (`0x0000` if unencoded, or `ProtoHeader.recId` if bit-tree compressed, §4.1.1).
    pub wire_prefix: u16,
    /// Decoded 20-byte `ProtoHeader` (§4.2).
    pub header: ProtoHeader,
    /// Decoded optional headers in ascending bit order (§4.3).
    pub opt_headers: OptionalHeaders,
    /// Decoded `NetBlob` fragments (non-empty only when `header.flags & 0x04000000 != 0`, §5.6.3).
    pub fragments: Vec<NetBlobFragment>,
    /// Raw payload bytes (`header.pak_size` bytes immediately following the 20-byte `ProtoHeader`).
    pub raw_payload: Vec<u8>,
}

/// Encodes a packet (`header`, `opt_headers`, `fragments`) into a raw UDP datagram (§4.1–§4.4).
///
/// - Computes `pak_size` from the serialized optional headers and `NetBlob` fragments.
/// - Sets `FLAG_HAS_NET_BLOBS` (`0x04000000`) if `fragments` is non-empty, and `FLAG_ENCRYPTED_CHECKSUM`
///   (`0x02000000`) if `xor_mask.is_some()`.
/// - Computes `ProtoHeader.checksum` using the `0xBADD70DD` header placeholder and optional `xor_mask` (§4.4.2).
/// - Prepends `0x0000` when `wire_codec` is `None`, or `header.rec_id` (big-endian) followed by
///   the bit-tree compressed `[ProtoHeader[2..20] | payload]` when `wire_codec` is `Some` (§4.1.1, §4.1.2).
///
/// # Errors
///
/// Returns [`SessionError`] if optional headers or fragments are invalid or exceed `MAX_PAK_SIZE`.
pub fn encode_datagram(
    mut header: ProtoHeader,
    opt_headers: &OptionalHeaders,
    fragments: &[NetBlobFragment],
    xor_mask: Option<u32>,
    wire_codec: Option<&dyn BitTreeCodec>,
) -> Result<Vec<u8>, SessionError> {
    if fragments.len() > MAX_FRAGS_PER_PACKET {
        return Err(SessionError::TooManyFragments {
            count: fragments.len(),
        });
    }

    let mut payload = Vec::new();
    opt_headers.encode(&mut payload)?;
    for frag in fragments {
        frag.encode(&mut payload)?;
    }

    if payload.len() > usize::from(MAX_PAK_SIZE) {
        return Err(SessionError::InvalidPakSize {
            pak_size: u16::MAX,
            actual_payload_len: payload.len(),
        });
    }

    header.pak_size = payload.len() as u16;
    header.flags |= opt_headers.flags_mask();
    if !fragments.is_empty() {
        header.flags |= FLAG_HAS_NET_BLOBS;
    }
    if xor_mask.is_some() {
        header.flags |= FLAG_ENCRYPTED_CHECKSUM;
    }
    if (header.flags & INVALID_PROTO_FLAGS_MASK) != 0 {
        return Err(SessionError::InvalidHeaderFlags {
            flags: header.flags,
        });
    }

    header.checksum = compute_packet_checksum(header, &payload, xor_mask);
    let wire_hdr = header.to_wire_bytes();

    if let Some(codec) = wire_codec {
        let mut plain_after_rec_id = Vec::with_capacity(18 + payload.len());
        plain_after_rec_id.extend_from_slice(&wire_hdr[2..20]);
        plain_after_rec_id.extend_from_slice(&payload);

        let mut out = Vec::with_capacity(2 + plain_after_rec_id.len());
        out.extend_from_slice(&header.rec_id.to_be_bytes());
        codec.encode(&plain_after_rec_id, &mut out, MAX_DATAGRAM_BUF - 2)?;
        Ok(out)
    } else {
        let mut out = Vec::with_capacity(2 + PROTO_HEADER_BYTES + payload.len());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&wire_hdr);
        out.extend_from_slice(&payload);
        Ok(out)
    }
}

/// Decodes and validates a raw UDP datagram before sequence-window/ISAAC-mask lookup (§4.1–§4.3, §10.1 rules 1–3, 7, 8).
///
/// If `verify_unmasked_checksum` is `true` and `(header.flags & 0x02000000) == 0`, this function
/// also verifies `ProtoHeader.checksum` immediately. When `(header.flags & 0x02000000) != 0`,
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
        // Prepend the 2-byte big-endian recId from wirePrefix so bytes 0..20 form the full ProtoHeader (§4.1.2)
        out.extend_from_slice(&datagram[0..2]);
        codec.decode(&datagram[2..], &mut out, MAX_DATAGRAM_BUF - 2)?;
        out
    };

    if decoded_buf.len() < PROTO_HEADER_BYTES {
        return Err(SessionError::DatagramTooShort {
            len: decoded_buf.len() + 2,
        });
    }

    let header = ProtoHeader::from_wire_bytes(&decoded_buf[0..PROTO_HEADER_BYTES])?;
    let actual_payload_len = decoded_buf.len() - PROTO_HEADER_BYTES;
    if usize::from(header.pak_size) != actual_payload_len {
        return Err(SessionError::InvalidPakSize {
            pak_size: header.pak_size,
            actual_payload_len,
        });
    }

    let payload = &decoded_buf[PROTO_HEADER_BYTES..];
    if verify_unmasked_checksum && (header.flags & FLAG_ENCRYPTED_CHECKSUM) == 0 {
        verify_packet_checksum(header, payload, None)?;
    }

    let (opt_headers, mut cursor) = OptionalHeaders::decode(header.flags, payload, from_server)?;
    let mut fragments = Vec::new();
    if (header.flags & FLAG_HAS_NET_BLOBS) != 0 {
        while cursor < payload.len() {
            if fragments.len() >= MAX_FRAGS_PER_PACKET {
                return Err(SessionError::TooManyFragments {
                    count: fragments.len() + 1,
                });
            }
            let (frag, consumed) = NetBlobFragment::decode(&payload[cursor..])?;
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

/// Lifecycle state (`0..8`) of a `CConnection` (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectionState {
    /// State `0`: `Idle` / `Uninitialized`.
    Idle = 0,
    /// State `1`: `LoginRequestSent` (retransmits `LoginRequest` or `WorldLoginRequest` every `2.0` s, timeout `20.0` s).
    LoginRequestSent = 1,
    /// State `2`: `ConnectResponseSent` (retransmits `ConnectResponse` every `0.6` s, timeout `20.0` s).
    ConnectResponseSent = 2,
    /// State `3`: `ServerListening` (unused on client).
    ServerListening = 3,
    /// State `4`: `ServerConnectSent` (unused on client).
    ServerConnectSent = 4,
    /// State `5`: `Connected` (established bidirectional session).
    Connected = 5,
    /// State `6`: `DisconnectingFlush` (draining outbound reliable packets, up to `20.0` s).
    DisconnectingFlush = 6,
    /// State `7`: `DisconnectSent` (retransmitting `Disconnect` every `1.0` s, up to `20.0` s).
    DisconnectSent = 7,
    /// State `8`: `Closing` (terminal state before cleanup).
    Closing = 8,
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
    /// Server sent a `Referral` (`0x00000800`) to a world server (§6.5).
    ReferralReceived {
        /// 64-bit referral cookie to present in `WorldLoginRequest` (`0x00020000`).
        referral_cookie: u64,
        /// Resolved world server IPv4 address and UDP port (substituting login server IP if `0.0.0.0`).
        world_addr: SocketAddrV4,
        /// 16-bit internal world server ID.
        world_server_id: u16,
    },
    /// Server sent a `ServerSwitch` (`0x00000100`, §6.6).
    ServerSwitch(ServerSwitchHeader),
    /// Server sent a `LogonServerAddr` (`0x00000200`, §6.6).
    LogonServerAddr(SocketAddrV4),
    /// A complete `NetBlob` was reassembled (and decrypted if `blobFlags & 0x08`, §5.6.4, §7.2).
    NetBlobReceived(ReassembledNetBlob),
    /// Connection updated its round-trip latency estimate from an `EchoResponse` (§8.2).
    LatencyUpdatedMs(f32),
    /// Connection encountered a protocol/handshake error or disconnect (§6.7, §8.3).
    Disconnected {
        /// Internal error code (`0x00A7E948`, `0x06F16F2E`, `0x0DFB256E`, `0x07CF6BF2`, etc.).
        internal_error: u32,
        /// Mapped client `HRESULT` (`0x80041006`, `0x8004100F`, `0x80041012`, `0x8004100E`, etc.).
        hresult: u32,
    },
}

/// Saved outbound sequenced packet in the retransmission table (§5.1, §5.3).
#[derive(Debug, Clone)]
pub struct SentPacketRecord {
    /// Assigned outgoing sequence number (`>= 2`).
    pub seq_num: u32,
    /// Saved outgoing ISAAC XOR mask drawn when `seq_num` was first assigned (§7.3).
    pub xor_mask: u32,
    /// Optional headers carried in the packet.
    pub opt_headers: OptionalHeaders,
    /// `NetBlob` fragments carried in the packet.
    pub fragments: Vec<NetBlobFragment>,
    /// True if the packet still has reliable content that should be retransmitted on `Nak`
    /// (false if it only carried ephemeral `queueId == 1` fragments that have expired, §5.3).
    pub retransmit_eligible: bool,
}

/// Per-connection state machine, sequence window, ISAAC generators, and `NetBlob` reassembler (§3.4, §5–§8).
#[derive(Debug)]
pub struct SessionConnection {
    local_rec_id: u16,
    server_rec_id: u16,
    client_conn_idx: u16,
    remote_addr: SocketAddrV4,
    state: ConnectionState,
    state_entered_at_sec: f64,
    last_recv_time_sec: f64,
    last_ack_nak_sent_sec: f64,
    last_time_sync_sent_sec: f64,
    last_handshake_retransmit_sec: f64,
    last_flow_sent_sec: f64,
    bytes_received_since_flow: u32,
    extended_silence_timeout: bool,
    server_clock_offset_sec: f64,
    smoothed_latency_ms: f32,
    last_switch_seq: u32,

    send_seq_num: u32,
    recv_seq_expected: u32,
    outgoing_isaac: Option<IsaacCipher>,
    incoming_isaac: Option<IsaacCipher>,
    missing_gaps: BTreeMap<u32, u32>,
    retransmit_table: BTreeMap<u32, SentPacketRecord>,
    empty_ack_queue: VecDeque<u32>,
    registered_codec_uuids: Vec<[u8; 16]>,
    active_codec_uuid: [u8; 16],
    des3_key: Option<[u8; 24]>,

    stored_login_request: Option<LoginRequestHeader>,
    stored_world_login_request: Option<WorldLoginRequestHeader>,
    stored_connect_response: Option<ConnectResponseHeader>,

    next_order_num: [u16; 5],
    next_blob_id: u16,
    reassembler: NetBlobReassembler,
    outbound_datagrams: VecDeque<Vec<u8>>,
    events: VecDeque<SessionEvent>,
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
            last_time_sync_sent_sec: now_sec,
            last_handshake_retransmit_sec: now_sec,
            last_flow_sent_sec: now_sec,
            bytes_received_since_flow: 0,
            extended_silence_timeout: false,
            server_clock_offset_sec: 0.0,
            smoothed_latency_ms: 0.0,
            last_switch_seq: 0,

            send_seq_num: 1,
            recv_seq_expected: 1,
            outgoing_isaac: None,
            incoming_isaac: None,
            missing_gaps: BTreeMap::new(),
            retransmit_table: BTreeMap::new(),
            empty_ack_queue: VecDeque::new(),
            registered_codec_uuids: Vec::new(),
            active_codec_uuid: [0u8; 16],
            des3_key: None,

            stored_login_request: None,
            stored_world_login_request: None,
            stored_connect_response: None,

            next_order_num: [0; 5],
            next_blob_id: 1,
            reassembler: NetBlobReassembler::default(),
            outbound_datagrams: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    /// Returns the current connection state (`0..8`, §6.1).
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Returns the local client `recId` (`0x0100..=0xFEFF`).
    #[must_use]
    pub fn local_rec_id(&self) -> u16 {
        self.local_rec_id
    }

    /// Returns the remote server `recId` (`0` prior to `ConnectRequest`).
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

    /// Returns the number of unacknowledged sent packets in the retransmit table (§5.1).
    #[must_use]
    pub fn unacked_sent_count(&self) -> usize {
        self.retransmit_table.len()
    }

    /// Sets the 24-byte 3DES session key used for per-`NetBlob` encryption (`blobFlags & 0x08`, §7.2).
    pub fn set_des3_key(&mut self, key: [u8; 24]) {
        self.des3_key = Some(key);
    }

    /// Enables or disables the `300.0`-second extended silence timeout in State `5` (§8.3).
    pub fn set_extended_silence_timeout(&mut self, enabled: bool) {
        self.extended_silence_timeout = enabled;
    }

    /// Registers a supported 16-byte per-connection codec UUID (§6.4, §7.4).
    pub fn register_codec_uuid(&mut self, uuid: [u8; 16]) {
        if !self.registered_codec_uuids.contains(&uuid) {
            self.registered_codec_uuids.push(uuid);
        }
    }

    /// Pops the next queued outbound UDP datagram ready to transmit to `remote_addr`.
    pub fn pop_outbound_datagram(&mut self) -> Option<Vec<u8>> {
        self.outbound_datagrams.pop_front()
    }

    /// Pops the next emitted [`SessionEvent`].
    pub fn pop_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    fn base_proto_header(&self, seq_num: u32, now_sec: f64) -> ProtoHeader {
        let time_ms = ((now_sec * 1000.0) as u64 & 0xFFFF) as u16;
        ProtoHeader {
            rec_id: self.server_rec_id,
            pak_size: 0,
            flags: 0,
            seq_num,
            checksum: 0,
            iteration: 0,
            table: (self.client_conn_idx & 0xFF) as u8,
            time: time_ms,
        }
    }

    /// Initiates a login-server session by transitioning from State `0` to State `1` and queueing
    /// an unsequenced `LoginRequest` (`0x00010000`) datagram (§6.1, §6.2).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the connection is not in `Idle` state or encoding fails.
    pub fn start_login(
        &mut self,
        login_request: LoginRequestHeader,
        now_sec: f64,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Idle {
            return Err(SessionError::InvalidState { state: self.state });
        }
        if let LoginAuthPayload::Encrypted { des3_key, .. } = &login_request.payload {
            self.des3_key = Some(*des3_key);
        }
        self.stored_login_request = Some(login_request.clone());
        self.state = ConnectionState::LoginRequestSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;

        let hdr = self.base_proto_header(0, now_sec);
        let opt = OptionalHeaders {
            login_request: Some(login_request),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    /// Initiates a world-server session by transitioning from State `0` to State `1` and queueing
    /// an unsequenced `WorldLoginRequest` (`0x00020000`) datagram (§6.1, §6.5).
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
        let wlr = WorldLoginRequestHeader { referral_cookie };
        self.stored_world_login_request = Some(wlr);
        self.state = ConnectionState::LoginRequestSent;
        self.state_entered_at_sec = now_sec;
        self.last_handshake_retransmit_sec = now_sec;

        let hdr = self.base_proto_header(0, now_sec);
        let opt = OptionalHeaders {
            world_login_request: Some(wlr),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    /// Sends a sequenced (`flags & 0x02000000`) packet carrying `opt_headers` and `fragments`,
    /// recording it in the outbound retransmission table with its ISAAC `xor_mask` (§5.1, §7.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::OutboundWindowOverflow`] if unacknowledged packets exceed `130,004`
    /// (`0x1FBC4`) or the sequence span exceeds `0x7FFFFFFF` (§5.1).
    pub fn send_sequenced_packet(
        &mut self,
        mut opt_headers: OptionalHeaders,
        fragments: Vec<NetBlobFragment>,
        retransmit_eligible: bool,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<u32, SessionError> {
        if self.retransmit_table.len() >= MAX_UNACKED_SENT_PACKETS {
            self.state = ConnectionState::Closing;
            self.events.push_back(SessionEvent::Disconnected {
                internal_error: 0x00CD_0B73,
                hresult: 0x8004_100F,
            });
            return Err(SessionError::OutboundWindowOverflow {
                internal_error: 0x00CD_0B73,
                hresult: 0x8004_100F,
            });
        }
        let next_seq = {
            let s = self.send_seq_num.wrapping_add(1);
            if s == 0 { 2 } else { s }
        };
        if let Some((&oldest_seq, _)) = self.retransmit_table.first_key_value()
            && next_seq.wrapping_sub(oldest_seq) > MAX_OUTBOUND_SEQ_SPAN
        {
            self.state = ConnectionState::Closing;
            self.events.push_back(SessionEvent::Disconnected {
                internal_error: 0x00CD_0B73,
                hresult: 0x8004_100F,
            });
            return Err(SessionError::OutboundWindowOverflow {
                internal_error: 0x00CD_0B73,
                hresult: 0x8004_100F,
            });
        }

        // Attach Flow header if >= 1/3 s and >= 1,400 bytes received (§5.5)
        let elapsed_flow = now_sec - self.last_flow_sent_sec;
        if opt_headers.flow.is_none()
            && elapsed_flow >= (1.0 / 3.0)
            && self.bytes_received_since_flow >= 1400
        {
            opt_headers.flow = Some(FlowHeader {
                bytes_received_delta: self.bytes_received_since_flow,
                interval_ms: (elapsed_flow * 1000.0) as u16,
            });
            self.bytes_received_since_flow = 0;
            self.last_flow_sent_sec = now_sec;
        }

        self.send_seq_num = next_seq;
        let xor_mask = self
            .outgoing_isaac
            .as_mut()
            .map_or(0, IsaacCipher::next_mask);

        let hdr = self.base_proto_header(next_seq, now_sec);
        let dgram = encode_datagram(hdr, &opt_headers, &fragments, Some(xor_mask), wire_codec)?;
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

    /// Splits a `NetBlob` into `<= 436`-byte fragments and queues sequenced packets on the connection (§5.6.4).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if fragmentation or packet queueing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn send_net_blob(
        &mut self,
        queue_id: u8,
        seq_group: u16,
        blob_header_word: u32,
        payload: &[u8],
        encrypted: bool,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<Vec<u32>, SessionError> {
        if !(1..=4).contains(&queue_id) {
            return Err(SessionError::InvalidQueueId { queue_id });
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

        let frags = split_net_blob(
            queue_id,
            order_num,
            blob_id,
            seq_group,
            None,
            blob_header_word,
            payload,
            encrypted,
            self.des3_key.as_ref(),
        )?;

        let retransmit_eligible = queue_id == 2 || queue_id == 4;
        let mut seqs = Vec::with_capacity(frags.len());
        for frag in frags {
            let seq = self.send_sequenced_packet(
                OptionalHeaders::default(),
                vec![frag],
                retransmit_eligible,
                now_sec,
                wire_codec,
            )?;
            seqs.push(seq);
        }
        Ok(seqs)
    }

    /// Marks an outbound sequenced packet's unreliable fragments as expired so a future `Nak`
    /// for `seq_num` emits `EmptyAck` (`0x00002000`) instead of retransmitting (§5.3, §5.4).
    pub fn expire_unreliable_sent_packet(&mut self, seq_num: u32) {
        if let Some(rec) = self.retransmit_table.get_mut(&seq_num) {
            rec.retransmit_eligible = false;
        }
    }

    /// Initiates a local graceful disconnect (`State 5 -> State 6`, §6.1, §8.3).
    pub fn request_disconnect(&mut self, now_sec: f64) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::DisconnectingFlush;
            self.state_entered_at_sec = now_sec;
        }
    }

    /// Processes a single incoming UDP datagram from `source_addr` at `now_sec` (§4–§8, §10.1).
    ///
    /// Enforces all hostile-input rules in §10.1 without mutating session state if any check fails.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if any validation rule in §10.1 fails.
    pub fn receive_datagram(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
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
        let pkt = decode_datagram(datagram, true, wire_codec, true)?;
        let hdr = pkt.header;

        // Rule 5 & 6: Sequence window check BEFORE advancing ISAAC PRNG or mutating state (§5.1, §10.1 rules 5, 6)
        let mut is_duplicate_seq = false;
        if (hdr.flags & FLAG_ENCRYPTED_CHECKSUM) != 0 {
            let seq = hdr.seq_num;
            if seq > self.recv_seq_expected.saturating_add(MAX_SEQ_LOOKAHEAD) {
                return Err(SessionError::SequenceOutOfWindow {
                    seq_num: seq,
                    expected: self.recv_seq_expected,
                });
            }

            if seq <= self.recv_seq_expected {
                if let Some(&saved_mask) = self.missing_gaps.get(&seq) {
                    verify_packet_checksum(hdr, &pkt.raw_payload, Some(saved_mask))?;
                    self.missing_gaps.remove(&seq);
                } else {
                    // Duplicate or already processed sequence number: reject if checksum cannot be verified
                    is_duplicate_seq = true;
                }
            } else {
                let new_gaps = (seq - self.recv_seq_expected - 1) as usize;
                if self.missing_gaps.len().saturating_add(new_gaps) > MAX_MISSING_SEQ_GAPS {
                    return Err(SessionError::GapLimitExceeded {
                        count: self.missing_gaps.len().saturating_add(new_gaps),
                    });
                }

                // Clone incoming ISAAC state so that if checksum verification fails,
                // self.incoming_isaac and self.missing_gaps are NOT mutated (§10.1).
                let mut trial_isaac = self
                    .incoming_isaac
                    .clone()
                    .unwrap_or_else(|| IsaacCipher::from_seed(0));
                let mut skipped_masks = Vec::with_capacity(new_gaps);
                for skipped_seq in (self.recv_seq_expected + 1)..seq {
                    skipped_masks.push((skipped_seq, trial_isaac.next_mask()));
                }
                let packet_mask = trial_isaac.next_mask();
                verify_packet_checksum(hdr, &pkt.raw_payload, Some(packet_mask))?;

                // Commit ISAAC state, gap entries, and recv_seq_expected watermark
                self.incoming_isaac = Some(trial_isaac);
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

        // State 2 -> State 5 transition on receiving first valid server packet after ConnectRequest (§6.4)
        if self.state == ConnectionState::ConnectResponseSent
            && pkt.opt_headers.connect_request.is_none()
            && pkt.opt_headers.net_error.is_none()
            && pkt.opt_headers.net_error_disconnect.is_none()
        {
            self.enter_connected_state(now_sec)?;
        }

        // Process optional headers (§5.2–§5.4, §6.3–§6.7, §8.1–§8.3)
        self.process_optional_headers(&pkt.opt_headers, now_sec, wire_codec)?;

        // Process NetBlob fragments if not a duplicate sequenced packet (§5.6.4)
        if !is_duplicate_seq {
            for frag in &pkt.fragments {
                if let Some(blob) = self.reassembler.ingest(
                    self.server_rec_id,
                    frag.clone(),
                    now_sec,
                    self.des3_key.as_ref(),
                )? {
                    self.events.push_back(SessionEvent::NetBlobReceived(blob));
                }
            }
        }

        Ok(pkt)
    }

    fn enter_connected_state(&mut self, now_sec: f64) -> Result<(), SessionError> {
        self.state = ConnectionState::Connected;
        self.state_entered_at_sec = now_sec;
        self.last_time_sync_sent_sec = now_sec;
        self.events.push_back(SessionEvent::Connected {
            local_rec_id: self.local_rec_id,
            server_rec_id: self.server_rec_id,
        });
        // Immediately queue initial TimeSync (0x01000000) upon entering State 5 (§6.4, §8.1)
        let hdr = self.base_proto_header(self.send_seq_num, now_sec);
        let opt = OptionalHeaders {
            time_sync: Some(now_sec + self.server_clock_offset_sec),
            ..OptionalHeaders::default()
        };
        let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
        self.outbound_datagrams.push_back(dgram);
        Ok(())
    }

    fn process_optional_headers(
        &mut self,
        opt: &OptionalHeaders,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        // ConnectRequest (0x00040000, §6.3, §6.4)
        if let Some(cr) = &opt.connect_request {
            let is_null_uuid = cr.codec_uuid == [0u8; 16];
            if !is_null_uuid && !self.registered_codec_uuids.contains(&cr.codec_uuid) {
                self.state = ConnectionState::Closing;
                self.events.push_back(SessionEvent::Disconnected {
                    internal_error: 0x00F9_982C,
                    hresult: 0x8004_1016,
                });
                return Err(SessionError::UnsupportedCodecUuid {
                    uuid: cr.codec_uuid,
                    internal_error: 0x00F9_982C,
                    hresult: 0x8004_1016,
                });
            }
            if cr.assigned_client_rec_id >= MIN_LOCAL_REC_ID {
                self.local_rec_id = cr.assigned_client_rec_id;
            }
            self.server_rec_id = cr.server_rec_id;
            self.client_conn_idx = cr.client_conn_idx;
            self.active_codec_uuid = cr.codec_uuid;
            self.server_clock_offset_sec = cr.server_time - now_sec;
            self.outgoing_isaac = Some(IsaacCipher::from_seed(cr.outgoing_seed));
            self.incoming_isaac = Some(IsaacCipher::from_seed(cr.incoming_seed));

            let resp = ConnectResponseHeader {
                check_value: (cr.server_time.to_bits()) ^ u64::from(self.local_rec_id),
                local_rec_id: self.local_rec_id,
                client_conn_idx: self.client_conn_idx,
                codec_uuid: self.active_codec_uuid,
            };
            self.stored_connect_response = Some(resp);

            let hdr = self.base_proto_header(0, now_sec);
            let resp_opt = OptionalHeaders {
                connect_response: Some(resp),
                ..OptionalHeaders::default()
            };
            let dgram = encode_datagram(hdr, &resp_opt, &[], None, None)?;
            self.outbound_datagrams.push_back(dgram);

            if self.state == ConnectionState::LoginRequestSent {
                self.state = ConnectionState::ConnectResponseSent;
                self.state_entered_at_sec = now_sec;
                self.last_handshake_retransmit_sec = now_sec;
            } else if self.state == ConnectionState::ConnectResponseSent {
                self.enter_connected_state(now_sec)?;
            }
        }

        // PakAck (0x00004000, big-endian u32, §5.2)
        if let Some(ack_seq) = opt.pak_ack {
            self.retransmit_table
                .retain(|&seq, _| (seq.wrapping_sub(ack_seq) as i32) > 0);
        }

        // Nak (0x00001000, §5.3)
        if let Some(nak_seqs) = &opt.nak {
            if let Some(&first_nak) = nak_seqs.first() {
                // Implicitly acknowledge all sent packets strictly before nak_seqs[0] (§5.3)
                self.retransmit_table
                    .retain(|&seq, _| (seq.wrapping_sub(first_nak) as i32) >= 0);
            }
            for &s in nak_seqs {
                if let Some(rec) = self.retransmit_table.get(&s)
                    && rec.retransmit_eligible
                {
                    let hdr = self.base_proto_header(rec.seq_num, now_sec);
                    let dgram = encode_datagram(
                        hdr,
                        &rec.opt_headers,
                        &rec.fragments,
                        Some(rec.xor_mask),
                        wire_codec,
                    )?;
                    self.outbound_datagrams.push_back(dgram);
                } else if !self.empty_ack_queue.contains(&s) {
                    self.empty_ack_queue.push_back(s);
                }
            }
        }

        // EmptyAck (0x00002000, §5.4)
        if let Some(dropped_seqs) = &opt.empty_ack {
            for &s in dropped_seqs {
                self.missing_gaps.remove(&s);
            }
        }

        // Referral (0x00000800, §6.5)
        if let Some(ref_hdr) = &opt.referral {
            let ip = if ref_hdr.target_addr.ip().is_unspecified() {
                *self.remote_addr.ip()
            } else {
                *ref_hdr.target_addr.ip()
            };
            let world_addr = SocketAddrV4::new(ip, ref_hdr.target_addr.port());
            self.events.push_back(SessionEvent::ReferralReceived {
                referral_cookie: ref_hdr.referral_cookie,
                world_addr,
                world_server_id: ref_hdr.world_server_id,
            });
        }

        // ServerSwitch (0x00000100, §6.6): processed only if switchSeq > lastSwitchSeq
        if let Some(ss) = opt.server_switch
            && ss.switch_seq > self.last_switch_seq
        {
            self.last_switch_seq = ss.switch_seq;
            self.events.push_back(SessionEvent::ServerSwitch(ss));
        }

        // LogonServerAddr (0x00000200, §6.6)
        if let Some(lsa) = opt.logon_server_addr {
            self.events
                .push_back(SessionEvent::LogonServerAddr(lsa.addr));
        }

        // TimeSync (0x01000000, §8.1)
        if let Some(server_time) = opt.time_sync {
            self.server_clock_offset_sec = server_time - now_sec;
        }

        // EchoResponse (when decoded in non-server test mode, §8.2)
        if let Some([echoed_client_sec, server_hold_sec]) = opt.echo_response {
            let rtt_sec = (now_sec as f32 - echoed_client_sec - server_hold_sec).max(0.0);
            self.smoothed_latency_ms = rtt_sec * 1000.0;
            self.events
                .push_back(SessionEvent::LatencyUpdatedMs(self.smoothed_latency_ms));
        }

        // NetError (0x00100000) or NetErrorDisconnect (0x00200000, §6.7)
        if let Some(ne) = opt.net_error.or(opt.net_error_disconnect) {
            self.state = ConnectionState::Closing;
            self.state_entered_at_sec = now_sec;
            self.events.push_back(SessionEvent::Disconnected {
                internal_error: ne.error_code,
                hresult: ne.to_hresult(),
            });
        }

        // Graceful Disconnect (0x00008000, §6.7, §8.3)
        if opt.disconnect {
            self.state = ConnectionState::Closing;
            self.state_entered_at_sec = now_sec;
            self.events.push_back(SessionEvent::Disconnected {
                internal_error: 0x07CF_6BF2,
                hresult: 0x8004_1012,
            });
        }

        Ok(())
    }

    /// Runs periodic timers for the connection at `now_sec` (§5.2–§5.4, §6.1, §8.1–§8.3):
    /// - State `1`: retransmits `LoginRequest` / `WorldLoginRequest` every `2.0` s (`20.0` s timeout -> `0x06F16F2E`).
    /// - State `2`: retransmits `ConnectResponse` every `0.6` s (`20.0` s timeout -> `0x06F16F2E`).
    /// - State `5`: checks `60.0` s / `300.0` s silence timeout (`0x0DFB256E`), sends `TimeSync` every `30.0` s,
    ///   flushes `EmptyAck`, sends `Nak` every `0.6` s when gaps exist, or sends `PakAck` every `2.0` s when no gaps exist.
    /// - State `6` / `7`: flushes outbound reliable packets then retransmits `Disconnect` every `1.0` s.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if encoding a periodic control packet fails.
    pub fn tick(
        &mut self,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
    ) -> Result<(), SessionError> {
        self.reassembler.purge_expired(now_sec);

        match self.state {
            ConnectionState::LoginRequestSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.state = ConnectionState::Closing;
                    self.state_entered_at_sec = now_sec;
                    self.events.push_back(SessionEvent::Disconnected {
                        internal_error: 0x06F1_6F2E,
                        hresult: 0x8004_100F,
                    });
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= 2.0 {
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_proto_header(0, now_sec);
                    let opt = OptionalHeaders {
                        login_request: self.stored_login_request.clone(),
                        world_login_request: self.stored_world_login_request,
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, None)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::ConnectResponseSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.state = ConnectionState::Closing;
                    self.state_entered_at_sec = now_sec;
                    self.events.push_back(SessionEvent::Disconnected {
                        internal_error: 0x06F1_6F2E,
                        hresult: 0x8004_100F,
                    });
                    return Ok(());
                }
                if now_sec - self.last_handshake_retransmit_sec >= 0.6
                    && let Some(resp) = self.stored_connect_response
                {
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_proto_header(0, now_sec);
                    let opt = OptionalHeaders {
                        connect_response: Some(resp),
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
                    self.state = ConnectionState::Closing;
                    self.state_entered_at_sec = now_sec;
                    self.events.push_back(SessionEvent::Disconnected {
                        internal_error: 0x0DFB_256E,
                        hresult: 0x8004_100F,
                    });
                    return Ok(());
                }

                // Flush up to 114 EmptyAck entries if any (§5.4)
                if !self.empty_ack_queue.is_empty() {
                    let count = self.empty_ack_queue.len().min(MAX_NAK_ENTRIES);
                    let mut list = Vec::with_capacity(count);
                    for _ in 0..count {
                        if let Some(s) = self.empty_ack_queue.pop_front() {
                            list.push(s);
                        }
                    }
                    let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        empty_ack: Some(list),
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                    self.outbound_datagrams.push_back(dgram);
                }

                // Periodic TimeSync every 30.0 s (§8.1)
                if now_sec - self.last_time_sync_sent_sec >= 30.0 {
                    self.last_time_sync_sent_sec = now_sec;
                    let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        time_sync: Some(now_sec + self.server_clock_offset_sec),
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                    self.outbound_datagrams.push_back(dgram);
                }

                // Nak every 0.6 s when missing_gaps > 0 (§5.3), else PakAck every 2.0 s (§5.2)
                if !self.missing_gaps.is_empty() {
                    if now_sec - self.last_ack_nak_sent_sec >= 0.6 {
                        self.last_ack_nak_sent_sec = now_sec;
                        let list: Vec<u32> = self
                            .missing_gaps
                            .keys()
                            .copied()
                            .take(MAX_NAK_ENTRIES)
                            .collect();
                        let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                        let opt = OptionalHeaders {
                            nak: Some(list),
                            ..OptionalHeaders::default()
                        };
                        let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                        self.outbound_datagrams.push_back(dgram);
                    }
                } else if self.recv_seq_expected > 1 && now_sec - self.last_ack_nak_sent_sec >= 2.0
                {
                    self.last_ack_nak_sent_sec = now_sec;
                    let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        pak_ack: Some(self.recv_seq_expected),
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::DisconnectingFlush => {
                if self.retransmit_table.is_empty() || now_sec - self.state_entered_at_sec >= 20.0 {
                    self.state = ConnectionState::DisconnectSent;
                    self.state_entered_at_sec = now_sec;
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        disconnect: true,
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::DisconnectSent => {
                if now_sec - self.state_entered_at_sec >= 20.0 {
                    self.state = ConnectionState::Closing;
                    self.state_entered_at_sec = now_sec;
                } else if now_sec - self.last_handshake_retransmit_sec >= 1.0 {
                    self.last_handshake_retransmit_sec = now_sec;
                    let hdr = self.base_proto_header(self.send_seq_num, now_sec);
                    let opt = OptionalHeaders {
                        disconnect: true,
                        ..OptionalHeaders::default()
                    };
                    let dgram = encode_datagram(hdr, &opt, &[], None, wire_codec)?;
                    self.outbound_datagrams.push_back(dgram);
                }
            }
            ConnectionState::Idle
            | ConnectionState::ServerListening
            | ConnectionState::ServerConnectSent
            | ConnectionState::Closing => {}
        }

        Ok(())
    }
}

/// Single-socket multi-connection demultiplexer (`EngineClientNet`, §3.4, §6.5).
#[derive(Debug, Default)]
pub struct SessionMultiplexer {
    connections: BTreeMap<u16, SessionConnection>,
    next_local_rec_id: u16,
}

impl SessionMultiplexer {
    /// Creates an empty `SessionMultiplexer` starting `localRecId` allocation at `0x0100` (§3.4).
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            next_local_rec_id: MIN_LOCAL_REC_ID,
        }
    }

    /// Allocates a new `SessionConnection` to `remote_addr` with a unique `localRecId` in `0x0100..=0xFEFF` (§3.4).
    pub fn create_connection(&mut self, remote_addr: SocketAddrV4, now_sec: f64) -> u16 {
        let mut candidate = self
            .next_local_rec_id
            .clamp(MIN_LOCAL_REC_ID, MAX_LOCAL_REC_ID);
        while self.connections.contains_key(&candidate) {
            candidate = if candidate >= MAX_LOCAL_REC_ID {
                MIN_LOCAL_REC_ID
            } else {
                candidate + 1
            };
        }
        self.next_local_rec_id = if candidate >= MAX_LOCAL_REC_ID {
            MIN_LOCAL_REC_ID
        } else {
            candidate + 1
        };
        let conn = SessionConnection::new(candidate, remote_addr, now_sec);
        self.connections.insert(candidate, conn);
        candidate
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
    /// - If `recId >= 0x0100`, looks up the connection by `recId` (or updates its `localRecId` if a
    ///   `ConnectRequest` from `source_addr` is assigning a new ID) and enforces source-address binding (§10.1 rule 4).
    /// - If `recId == 0x0000`, matches the active connection by `source_addr` (`(IPv4, port)`).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if no connection matches or packet validation fails.
    pub fn receive_from_peer(
        &mut self,
        datagram: &[u8],
        source_addr: SocketAddrV4,
        now_sec: f64,
        wire_codec: Option<&dyn BitTreeCodec>,
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

        let target_key = if header_rec_id >= MIN_LOCAL_REC_ID {
            if self.connections.contains_key(&header_rec_id) {
                Some(header_rec_id)
            } else {
                // During ConnectRequest, the server may address the packet to the newly assigned recId
                self.connections
                    .iter()
                    .find(|(_, c)| {
                        c.remote_addr() == source_addr
                            && c.state() == ConnectionState::LoginRequestSent
                    })
                    .map(|(&k, _)| k)
            }
        } else {
            self.connections
                .iter()
                .find(|(_, c)| c.remote_addr() == source_addr)
                .map(|(&k, _)| k)
        };

        let Some(key) = target_key else {
            return Err(SessionError::UnknownRecipient {
                rec_id: header_rec_id,
                source_addr,
            });
        };

        let pkt = {
            let conn = self
                .connections
                .get_mut(&key)
                .ok_or(SessionError::UnknownRecipient {
                    rec_id: header_rec_id,
                    source_addr,
                })?;
            conn.receive_datagram(datagram, source_addr, now_sec, wire_codec)?
        };

        // Re-key if ConnectRequest assigned a new localRecId
        let updated_id = self
            .connections
            .get(&key)
            .map_or(key, SessionConnection::local_rec_id);
        if updated_id != key
            && let Some(conn) = self.connections.remove(&key)
        {
            self.connections.insert(updated_id, conn);
        }

        Ok((updated_id, pkt))
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
            while let Some(dgram) = conn.pop_outbound_datagram() {
                transport.send_to(&dgram, remote)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// §9.1–§9.4 Upper-Layer NetMessage Envelope, Routing Header, Session Control Messages, and Batch Containers
// ---------------------------------------------------------------------------

/// `envelopeFlags` bit 0 (`0x01`): `NetMessage` body is zlib-compressed preceded by `u32` LE uncompressed length (§9.1).
pub const ENVELOPE_FLAG_ZLIB_COMPRESSED: u8 = 0x01;
/// `envelopeFlags` bit 1 (`0x02`): `NetMessage` carries an aligned optional routing header before Offset `0x00` (§9.1).
pub const ENVELOPE_FLAG_HAS_ROUTING_HEADER: u8 = 0x02;
/// In-memory flag bit (`0x80`) set on `NetMessageEnvelope.envelope_flags` when the source `NetBlob` was 3DES-CBC encrypted (§7.2).
pub const ENVELOPE_FLAG_WAS_ENCRYPTED: u8 = 0x80;

/// Optional routing header present when `(envelopeFlags & 0x02) != 0` (§9.1 item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetMessageRoutingHeader {
    /// Bit 0 (`0x0001`): `routingField0` (`u32` LE).
    pub field0: Option<u32>,
    /// Bit 1 (`0x0002`): `routingField1` (`u32` LE).
    pub field1: Option<u32>,
    /// Bit 2 (`0x0004`): `routingField2` (`u64` LE).
    pub field2: Option<u64>,
    /// Bit 3 (`0x0008`): `routingField3` (`u32` LE).
    pub field3: Option<u32>,
}

impl NetMessageRoutingHeader {
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

    /// Decodes a `NetMessageRoutingHeader` from the start of `bytes` (immediately after `envelopeFlags`, §9.1),
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

/// Decoded upper-layer `NetMessage` envelope (§9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetMessageEnvelope {
    /// 32-bit message opcode (`blobHeaderWord`, §9.1).
    pub opcode: u32,
    /// Envelope flags byte (`envelopeFlags`, bit 0 = zlib compressed, bit 1 = has routing header, bit 7 = was 3DES-encrypted).
    pub envelope_flags: u8,
    /// Optional routing header decoded when `(envelopeFlags & 0x02) != 0` (§9.1).
    pub routing_header: Option<NetMessageRoutingHeader>,
    /// Inner message payload bytes starting at Offset `0x00` ready for the opcode's deserializer (§9.1 item 4).
    pub body: Vec<u8>,
}

/// Serializes an upper-layer `NetMessage` envelope payload (`envelopeFlags` + optional routing header +
/// uncompressed or zlib-compressed body prefixed by `u32` LE uncompressed length, §9.1).
///
/// # Errors
///
/// Returns [`SessionError::InvalidEnvelopeFlags`] if `(envelope_flags & 0xFC) != 0` or
/// [`SessionError::ZlibDecompressionFailed`] if zlib compression fails.
pub fn encode_net_message_with_routing(
    mut envelope_flags: u8,
    routing_header: Option<&NetMessageRoutingHeader>,
    body: &[u8],
    compress_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    envelope_flags &= !ENVELOPE_FLAG_WAS_ENCRYPTED;
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
        NetMessageRoutingHeader::default().encode(&mut plain_after_flags);
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

/// Serializes an upper-layer `NetMessage` envelope payload (`envelopeFlags` + uncompressed or
/// zlib-compressed body prefixed by `u32` LE uncompressed length, §9.1).
///
/// # Errors
///
/// Returns [`SessionError`] if `envelope_flags` has reserved bits or zlib compression fails.
pub fn encode_net_message(
    envelope_flags: u8,
    body: &[u8],
    compress_zlib: bool,
) -> Result<Vec<u8>, SessionError> {
    encode_net_message_with_routing(envelope_flags, None, body, compress_zlib)
}

/// Decodes a `NetMessageEnvelope` from a reassembled `NetBlob` payload, performing bounded
/// zlib decompression when `(envelopeFlags & 0x01) != 0` and parsing the optional routing header
/// when `(envelopeFlags & 0x02) != 0` (§9.1, §10.1 rule 9).
///
/// # Errors
///
/// Returns [`SessionError`] if the envelope is empty, `(envelopeFlags & 0xFC) != 0`,
/// `uncompressedLen` exceeds `max_decompressed_bytes` (or `MAX_ZLIB_UNCOMPRESSED_BYTES`),
/// zlib decompression fails, or the routing header is malformed.
pub fn decode_net_message(
    opcode: u32,
    blob_payload: &[u8],
    was_encrypted: bool,
    max_decompressed_bytes: usize,
) -> Result<NetMessageEnvelope, SessionError> {
    let Some((&raw_flags, rest)) = blob_payload.split_first() else {
        return Err(SessionError::EmptyMessageEnvelope);
    };
    if (raw_flags & 0xFC) != 0 {
        return Err(SessionError::InvalidEnvelopeFlags { flags: raw_flags });
    }
    let effective_limit = max_decompressed_bytes.min(MAX_ZLIB_UNCOMPRESSED_BYTES);
    let mut envelope_flags = raw_flags;
    if was_encrypted {
        envelope_flags |= ENVELOPE_FLAG_WAS_ENCRYPTED;
    }

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
        let (rh, consumed) = NetMessageRoutingHeader::decode(after_flags)?;
        (Some(rh), consumed)
    } else {
        (None, 0)
    };

    Ok(NetMessageEnvelope {
        opcode,
        envelope_flags,
        routing_header,
        body: after_flags[body_start..].to_vec(),
    })
}

// ---------------------------------------------------------------------------
// §9.3 Post-Handshake Session/Version Negotiation Messages (0x0000012C..0x00000130)
// ---------------------------------------------------------------------------

fn encode_pstring_ascii(s: &str, out: &mut Vec<u8>) {
    encode_packed_dword(s.len() as u32, out);
    out.extend_from_slice(s.as_bytes());
}

fn decode_pstring_ascii(bytes: &[u8], opcode: u32) -> Result<(String, usize), SessionError> {
    let (len_u32, pd_len) =
        decode_packed_dword(bytes).map_err(|_| SessionError::InvalidControlMessage { opcode })?;
    let len = len_u32 as usize;
    let total = pd_len
        .checked_add(len)
        .ok_or(SessionError::InvalidControlMessage { opcode })?;
    if bytes.len() < total {
        return Err(SessionError::InvalidControlMessage { opcode });
    }
    let s = std::str::from_utf8(&bytes[pd_len..total])
        .map_err(|_| SessionError::InvalidControlMessage { opcode })?
        .to_owned();
    Ok((s, total))
}

/// `VersionSend` (`0x0000012C`, Client -> Server) post-handshake message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSendMessage {
    /// Network and `.dat` archive version stamp string (e.g., `"netver:1; didver:..."`, `PString`).
    pub version_string: String,
}

impl VersionSendMessage {
    /// Serializes `VersionSend` (`0x0000012C`) inner payload starting at Offset `0x00` (§9.3).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        encode_pstring_ascii(&self.version_string, &mut out);
        out
    }

    /// Decodes `VersionSend` (`0x0000012C`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if the `PString` is truncated or invalid UTF-8.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        let (version_string, _) = decode_pstring_ascii(body, OPCODE_VERSION_SEND)?;
        Ok(Self { version_string })
    }
}

/// `VersionResult` (`0x0000012D`, Server -> Client) post-handshake message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionResultMessage {
    /// `u32` LE status (`1` = accepted, `0` = rejected with `ERROR_VERSION_MISMATCH = 0x0DD6BAE1`).
    pub status: u32,
    /// Server version information (`PString`).
    pub server_version_info: String,
}

impl VersionResultMessage {
    /// Serializes `VersionResult` (`0x0000012D`) inner payload starting at Offset `0x00` (§9.3).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.status.to_le_bytes());
        encode_pstring_ascii(&self.server_version_info, &mut out);
        out
    }

    /// Decodes `VersionResult` (`0x0000012D`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `body` is shorter than 5 bytes or `PString` is malformed.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        if body.len() < 4 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_VERSION_RESULT,
            });
        }
        let status = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let (server_version_info, _) = decode_pstring_ascii(&body[4..], OPCODE_VERSION_RESULT)?;
        Ok(Self {
            status,
            server_version_info,
        })
    }

    /// Verifies that `status == 1`, returning [`SessionError::VersionRejected`] (`0x0DD6BAE1`) if `status == 0` (§9.3).
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

/// `LogonUpdate` / `QueueStatus` (`0x00000130`, Server -> Client) post-handshake message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogonUpdateMessage {
    /// `u32` LE logon result status.
    pub logon_result: u32,
    /// UTF-16LE detail string (`WString`: `PackedDWORD` char count + UTF-16LE).
    pub detail_text: String,
    /// `u32` LE queue position.
    pub queue_position: u32,
    /// `u32` LE queue total.
    pub queue_total: u32,
}

impl LogonUpdateMessage {
    /// Serializes `LogonUpdate` (`0x00000130`) inner payload starting at Offset `0x00` (§9.3).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.logon_result.to_le_bytes());
        encode_wstring(&self.detail_text, &mut out);
        out.extend_from_slice(&self.queue_position.to_le_bytes());
        out.extend_from_slice(&self.queue_total.to_le_bytes());
        out
    }

    /// Decodes `LogonUpdate` (`0x00000130`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `body` is truncated or `detail_text` is invalid UTF-16LE.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        if body.len() < 4 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGON_UPDATE,
            });
        }
        let logon_result = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let (detail_text, ws_len) =
            decode_wstring(&body[4..]).map_err(|_| SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGON_UPDATE,
            })?;
        let pos = 4 + ws_len;
        if body.len() < pos + 8 {
            return Err(SessionError::InvalidControlMessage {
                opcode: OPCODE_LOGON_UPDATE,
            });
        }
        let queue_position =
            u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
        let queue_total =
            u32::from_le_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]]);
        Ok(Self {
            logon_result,
            detail_text,
            queue_position,
            queue_total,
        })
    }
}

/// `AdminMessage` (`0x0000012E`, Server -> Client) system notification message (§9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminMessage {
    /// UTF-16LE administrator notification text (`WString`).
    pub admin_text: String,
}

impl AdminMessage {
    /// Serializes `AdminMessage` (`0x0000012E`) inner payload starting at Offset `0x00` (§9.3).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        encode_wstring(&self.admin_text, &mut out);
        out
    }

    /// Decodes `AdminMessage` (`0x0000012E`) from inner payload bytes starting at Offset `0x00` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `admin_text` is truncated or invalid UTF-16LE.
    pub fn decode(body: &[u8]) -> Result<Self, SessionError> {
        let (admin_text, _) =
            decode_wstring(body).map_err(|_| SessionError::InvalidControlMessage {
                opcode: OPCODE_ADMIN_MESSAGE,
            })?;
        Ok(Self { admin_text })
    }
}

/// Parsed `CNetLayer` post-handshake session/version negotiation message (`0x0000012C`..`0x00000130`, §9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionControlMessage {
    /// `0x0000012F` (`ReqVersionSend`, Server -> Client, 0 bytes).
    ReqVersionSend,
    /// `0x0000012C` (`VersionSend`, Client -> Server).
    VersionSend(VersionSendMessage),
    /// `0x0000012D` (`VersionResult`, Server -> Client).
    VersionResult(VersionResultMessage),
    /// `0x00000130` (`LogonUpdate` / `QueueStatus`, Server -> Client).
    LogonUpdate(LogonUpdateMessage),
    /// `0x0000012E` (`AdminMessage`, Server -> Client).
    Admin(AdminMessage),
}

impl SessionControlMessage {
    /// Encodes this control message into `(opcode, inner_body_bytes)` (§9.3).
    #[must_use]
    pub fn encode(&self) -> (u32, Vec<u8>) {
        match self {
            Self::ReqVersionSend => (OPCODE_REQ_VERSION_SEND, Vec::new()),
            Self::VersionSend(m) => (OPCODE_VERSION_SEND, m.encode()),
            Self::VersionResult(m) => (OPCODE_VERSION_RESULT, m.encode()),
            Self::LogonUpdate(m) => (OPCODE_LOGON_UPDATE, m.encode()),
            Self::Admin(m) => (OPCODE_ADMIN_MESSAGE, m.encode()),
        }
    }

    /// Decodes a `SessionControlMessage` from `(opcode, inner_body_bytes)` (§9.3).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidControlMessage`] if `opcode` is not in `0x0000012C..=0x00000130`
    /// or `body` is malformed.
    pub fn decode(opcode: u32, body: &[u8]) -> Result<Self, SessionError> {
        match opcode {
            OPCODE_REQ_VERSION_SEND => Ok(Self::ReqVersionSend),
            OPCODE_VERSION_SEND => Ok(Self::VersionSend(VersionSendMessage::decode(body)?)),
            OPCODE_VERSION_RESULT => Ok(Self::VersionResult(VersionResultMessage::decode(body)?)),
            OPCODE_LOGON_UPDATE => Ok(Self::LogonUpdate(LogonUpdateMessage::decode(body)?)),
            OPCODE_ADMIN_MESSAGE => Ok(Self::Admin(AdminMessage::decode(body)?)),
            _ => Err(SessionError::InvalidControlMessage { opcode }),
        }
    }
}

// ---------------------------------------------------------------------------
// §9.4.1 EngineClientNet World Entity Batch Messages (0x0000000D EnterAOI & 0x0000004C EventBatch)
// ---------------------------------------------------------------------------

/// A single gameplay sub-event embedded inside `0x0000000D` (`EnterAOI`) or `0x0000004C` (`EventBatch`, §9.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSubEvent {
    /// 32-bit gameplay event opcode (`PackedDWORD`).
    pub sub_opcode: u32,
    /// Raw gameplay event payload starting directly with its `u8 base_flags = 0x00` at `[0]` (no `envelopeFlags` byte, §9.4.1).
    pub sub_payload: Vec<u8>,
}

/// Decoded `EngineClientNet` world entity batch (`0x0000000D` `EnterAOI` or `0x0000004C` `EventBatch`, §9.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEventBatch {
    /// Outer opcode (`0x0000000D` or `0x0000004C`).
    pub opcode: u32,
    /// Base gameplay-event flags byte at Offset `0x00` (`0x00`, §9.2, §9.4.1).
    pub base_flags: u8,
    /// Ordered sequence of embedded gameplay sub-events targeting the outer `NetBlob`'s `recipientId`.
    pub sub_events: Vec<WorldSubEvent>,
    /// Trailing byte present on `0x0000000D` (`EnterAOI`) immediately after the `N` sub-events; `None` on `0x0000004C` (`EventBatch`, §9.4.1).
    pub enter_aoi_tail_flag: Option<u8>,
}

/// Encodes an `EngineClientNet` world entity batch (`0x0000000D` `EnterAOI` or `0x0000004C` `EventBatch`)
/// inner payload (starting at Offset `0x00` with `base_flags`, §9.4.1).
#[must_use]
pub fn encode_world_event_batch(batch: &WorldEventBatch) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(batch.base_flags);
    encode_packed_dword(batch.sub_events.len() as u32, &mut out);
    for ev in &batch.sub_events {
        encode_packed_dword(ev.sub_opcode, &mut out);
        out.extend_from_slice(&(ev.sub_payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&ev.sub_payload);
    }
    if batch.opcode == OPCODE_ENTER_AOI {
        out.push(batch.enter_aoi_tail_flag.unwrap_or(0));
    }
    out
}

/// Decodes an `EngineClientNet` world entity batch (`0x0000000D` `EnterAOI` or `0x0000004C` `EventBatch`)
/// from inner payload bytes (starting at Offset `0x00` after the outer `envelopeFlags`, §9.4.1, §10.1 rule 9).
///
/// # Errors
///
/// Returns [`SessionError`] if `opcode` is not `0x0000000D` or `0x0000004C`, any `subPayloadLen`
/// overruns the remaining buffer (`ContainerSubMessageOverrun`), or the trailing `EnterAOI` byte is missing.
pub fn decode_world_event_batch(opcode: u32, body: &[u8]) -> Result<WorldEventBatch, SessionError> {
    if opcode != OPCODE_ENTER_AOI && opcode != OPCODE_EVENT_BATCH {
        return Err(SessionError::InvalidControlMessage { opcode });
    }
    let Some((&base_flags, rest)) = body.split_first() else {
        return Err(SessionError::InvalidControlMessage { opcode });
    };
    let (count_u32, mut cursor) = decode_packed_dword(rest)?;
    let count = count_u32 as usize;
    let mut sub_events = Vec::with_capacity(count.min(256));
    for _ in 0..count {
        let (sub_opcode, n_op) = decode_packed_dword(&rest[cursor..])?;
        cursor += n_op;
        if rest.len() < cursor + 4 {
            return Err(SessionError::ContainerSubMessageOverrun {
                sub_len: 4,
                remaining: rest.len().saturating_sub(cursor),
            });
        }
        let sub_len = u32::from_le_bytes([
            rest[cursor],
            rest[cursor + 1],
            rest[cursor + 2],
            rest[cursor + 3],
        ]) as usize;
        cursor += 4;
        let remaining = rest.len().saturating_sub(cursor);
        if sub_len > remaining {
            return Err(SessionError::ContainerSubMessageOverrun { sub_len, remaining });
        }
        let sub_payload = rest[cursor..cursor + sub_len].to_vec();
        cursor += sub_len;
        sub_events.push(WorldSubEvent {
            sub_opcode,
            sub_payload,
        });
    }
    let enter_aoi_tail_flag = if opcode == OPCODE_ENTER_AOI {
        let Some(&tail) = rest.get(cursor) else {
            return Err(SessionError::InvalidControlMessage { opcode });
        };
        Some(tail)
    } else {
        None
    };

    Ok(WorldEventBatch {
        opcode,
        base_flags,
        sub_events,
        enter_aoi_tail_flag,
    })
}

// ---------------------------------------------------------------------------
// §9.4.2 CNetLayer / ChatNet Envelope Containers (0x00000186 and 0x00000179)
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
    let mut body = Vec::new();
    encode_packed_dword(sub_envelopes.len() as u32, &mut body);
    for (sub_opcode, sub_payload) in sub_envelopes {
        encode_packed_dword(*sub_opcode, &mut body);
        body.extend_from_slice(&(sub_payload.len() as u32).to_le_bytes());
        body.extend_from_slice(sub_payload);
    }
    encode_net_message(0, &body, compress_outer_zlib)
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
    let mut body = Vec::new();
    encode_packed_dword(sub_opcode, &mut body);
    body.extend_from_slice(&(sub_envelope_payload.len() as u32).to_le_bytes());
    body.extend_from_slice(sub_envelope_payload);
    body.extend_from_slice(&context_id.to_le_bytes());
    body.extend_from_slice(&result_code.to_le_bytes());
    encode_net_message(0, &body, compress_outer_zlib)
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

/// Decodes a `NetMessage` envelope and recursively unpacks batch containers (`0x00000186`)
/// and single-message wrappers (`0x00000179`) into leaf [`NetMessageEnvelope`] items (§9.1, §9.4.2).
///
/// # Errors
///
/// Returns [`SessionError`] if any envelope or container sub-message is malformed, overruns
/// remaining container bytes (`ContainerSubMessageOverrun`), or exceeds `max_depth`
/// (`ContainerRecursionLimitExceeded`).
pub fn dispatch_net_message(
    opcode: u32,
    blob_payload: &[u8],
    was_encrypted: bool,
    max_decompressed_bytes: usize,
    max_depth: usize,
) -> Result<Vec<NetMessageEnvelope>, SessionError> {
    let mut out = Vec::new();
    dispatch_net_message_inner(
        opcode,
        blob_payload,
        was_encrypted,
        max_decompressed_bytes,
        max_depth,
        0,
        &mut out,
    )?;
    Ok(out)
}

fn dispatch_net_message_inner(
    opcode: u32,
    blob_payload: &[u8],
    was_encrypted: bool,
    max_decompressed_bytes: usize,
    max_depth: usize,
    depth: usize,
    out: &mut Vec<NetMessageEnvelope>,
) -> Result<(), SessionError> {
    if depth > max_depth {
        return Err(SessionError::ContainerRecursionLimitExceeded { max_depth });
    }
    let env = decode_net_message(opcode, blob_payload, was_encrypted, max_decompressed_bytes)?;
    let inherited_encrypted = (env.envelope_flags & ENVELOPE_FLAG_WAS_ENCRYPTED) != 0;

    if opcode == OPCODE_BATCH_CONTAINER {
        let bytes = &env.body;
        let (count_u32, mut cursor) = decode_packed_dword(bytes)?;
        let count = count_u32 as usize;
        for _ in 0..count {
            let (sub_opcode, n_op) = decode_packed_dword(&bytes[cursor..])?;
            cursor += n_op;
            if bytes.len() < cursor + 4 {
                return Err(SessionError::ContainerSubMessageOverrun {
                    sub_len: 4,
                    remaining: bytes.len().saturating_sub(cursor),
                });
            }
            let sub_len = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            let remaining = bytes.len().saturating_sub(cursor);
            if sub_len > remaining {
                return Err(SessionError::ContainerSubMessageOverrun { sub_len, remaining });
            }
            let sub_slice = &bytes[cursor..cursor + sub_len];
            cursor += sub_len;
            dispatch_net_message_inner(
                sub_opcode,
                sub_slice,
                inherited_encrypted,
                max_decompressed_bytes,
                max_depth,
                depth + 1,
                out,
            )?;
        }
    } else if opcode == OPCODE_SINGLE_WRAPPER {
        let bytes = &env.body;
        let (sub_opcode, mut cursor) = decode_packed_dword(bytes)?;
        if bytes.len() < cursor + 4 {
            return Err(SessionError::ContainerSubMessageOverrun {
                sub_len: 4,
                remaining: bytes.len().saturating_sub(cursor),
            });
        }
        let sub_len = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]) as usize;
        cursor += 4;
        let remaining = bytes.len().saturating_sub(cursor);
        if sub_len > remaining {
            return Err(SessionError::ContainerSubMessageOverrun { sub_len, remaining });
        }
        let sub_slice = &bytes[cursor..cursor + sub_len];
        dispatch_net_message_inner(
            sub_opcode,
            sub_slice,
            inherited_encrypted,
            max_decompressed_bytes,
            max_depth,
            depth + 1,
            out,
        )?;
    } else {
        out.push(env);
    }

    Ok(())
}
