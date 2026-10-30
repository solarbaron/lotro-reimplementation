// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic integration tests for `tessera_compat::session` (`docs/specs/official-session.md` §3–§10).

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, AUTH_FLAG_HAS_SECOND_STRING, AUTH_TYPE_CHAT, AUTH_TYPE_PRIMARY,
    AuthConnection, AuthServerAddressHeader, BIT_TREE_TABLE_FORMAT_UUID,
    BLOB_FLAG_DEFAULT_RECIPIENT, BLOB_FLAG_ENCRYPTED, BLOB_FLAG_HAS_ATTACHMENT, BindPlan,
    BindPorts, BitTreeCodec, BitTreeNode, BitTreeTable, BlobFragment, BlobFragmentHeader,
    BlobReassembler, CHAT_MAX_RECV_RATE, ClientVersionMessage, ConnectAcceptHeader,
    ConnectChallengeHeader, ConnectionState, ControlMessage, DEFAULT_MAX_CONTAINER_DEPTH,
    DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES, DatagramHeader, DisconnectReason,
    ENVELOPE_FLAG_HAS_ROUTING_HEADER, ENVELOPE_FLAG_ZLIB_COMPRESSED, EnvelopeContext,
    FLAG_AUTH_SERVER_ADDR, FLAG_CLIENT_CONTROL, FLAG_CLOCK_SYNC, FLAG_CONNECT_CHALLENGE,
    FLAG_CUMULATIVE_ACK, FLAG_DISCONNECT, FLAG_FLOW_CONTROL, FLAG_HAS_FRAGMENTED_BLOBS,
    FLAG_MASKED_CHECKSUM, FLAG_OUT_OF_ORDER_ACK, FLAG_SEQUENCE_NAK, FLAG_SERVER_REDIRECT,
    FLAG_TRANSPORT_ERROR, FLAG_UNDEFINED_10, FLAG_UNDEFINED_23, FLAG_WORLD_REFERRAL,
    FlowControlHeader, FragmentAttachment, INVALID_DATAGRAM_FLAGS_MASK, InMemoryUdpTransport,
    IsaacCipher, LOGON_RESULT_QUEUED, LoginQueuePosition, LoginQueueStatusMessage, LogonOutcome,
    MAX_BLOB_FRAG_PAYLOAD, MAX_FRAGS_PER_PACKET, MAX_NAK_ENTRIES, MAX_SEQ_LOOKAHEAD,
    MAX_UNIQUE_PORT_ATTEMPTS, MessageRoutingHeader, OPCODE_BATCH_CONTAINER, OPCODE_CLIENT_VERSION,
    OPCODE_LOGIN_QUEUE_STATUS, OPCODE_SINGLE_WRAPPER, OPCODE_SYSTEM_NOTIFICATION,
    OPCODE_VERSION_REQUEST, OPCODE_VERSION_STATUS, OptionalHeaders, RECEIVE_FORBIDDEN_FLAGS_MASK,
    ReferralAuthHeader, SendChecksumRule, ServerRedirectHeader, SessionAuthHeader,
    SessionConnection, SessionError, SessionEvent, SessionMultiplexer, SystemNotificationMessage,
    TransportErrorHeader, UdpDatagramTransport, VALID_DATAGRAM_FLAGS_MASK, VersionStatusMessage,
    WireCodecs, WorldReferralHeader, check_ticket_text, combine_packet_checksum,
    compute_datagram_checksum, compute_packet_checksum, decode_datagram, decode_message,
    decode_variable_u32, des3_cbc_decrypt, des3_cbc_encrypt, dispatch_message,
    encode_batch_container, encode_datagram, encode_datagram_with_rule, encode_message,
    encode_message_with_routing, encode_single_wrapper, encode_single_wrapper_with_context,
    encode_variable_u32, max_recv_rate_for_speed, next_unique_port, send_payload_checksum,
    sndbuf_probe_midpoint, split_blob, unique_port_candidates, unique_port_ip_value,
    unique_port_seed, verify_packet_checksum,
};

/// The synthetic version stamp of Vector 6 (`official-session.md` §10.2,
/// `official-login.md` §7.2.7).
const VERSION: &str = "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10";

/// An account-only `SessionAuthHeader` of the primary connection (§6.2.1 rule 3).
fn account_auth(account: &str, timestamp: u32) -> Result<SessionAuthHeader, SessionError> {
    SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        VERSION,
        account,
        None,
        None,
        timestamp,
    )
}

/// A primary-connection `SessionAuthHeader` with `ticket` (§6.2.1 rule 1).
fn ticket_auth(account: &str, ticket: &str, timestamp: u32) -> SessionAuthHeader {
    SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        VERSION,
        account,
        Some(ticket),
        None,
        timestamp,
    )
    .expect("ticket header")
}

/// Parses a spec vector's hex text.
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

/// A connection in State 2 whose ISAAC generators a synthetic `ConnectChallengeHeader`
/// seeded (outgoing seed `0x0101_0101`, incoming seed `incoming_seed`); the challenge
/// assigned it `local_rec_id`, the server's ID is `0x0304`. Its outbound queue is drained.
fn challenged_connection(
    peer: SocketAddrV4,
    local_rec_id: u16,
    incoming_seed: u32,
) -> SessionConnection {
    challenged_connection_with_codec(peer, local_rec_id, incoming_seed, [0u8; 16])
}

/// [`challenged_connection`] whose challenge names `codec_uuid` (registered first when it is
/// not null, §7.3).
fn challenged_connection_with_codec(
    peer: SocketAddrV4,
    local_rec_id: u16,
    incoming_seed: u32,
    codec_uuid: [u8; 16],
) -> SessionConnection {
    let mut conn = SessionConnection::new(peer, 0.0);
    if codec_uuid != [0u8; 16] {
        conn.register_codec_uuid(codec_uuid);
    }
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    let challenge = ConnectChallengeHeader {
        server_time: 0.0,
        assigned_client_rec_id: local_rec_id,
        server_rec_id: 0x0304,
        outgoing_seed: 0x0101_0101,
        incoming_seed,
        reserved_dword_14: 0,
        client_conn_idx: 1,
        reserved_word_1a: 0,
        codec_uuid,
    };
    let dgram = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    conn.receive_datagram(&dgram, peer, 0.0, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    while conn.pop_outbound_datagram().is_some() {}
    while conn.pop_event().is_some() {}
    conn
}

// ---------------------------------------------------------------------------
// §3.1–§3.2 Transport & Local Port Selection
// ---------------------------------------------------------------------------

#[test]
fn transport_and_port_selection() {
    let local_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9000);
    let peer_addr = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 10), 9001);

    let mut mem = InMemoryUdpTransport::new(local_addr);
    assert_eq!(mem.local_addr().expect("local_addr"), local_addr);
    mem.send_to(&[0u8; 22], peer_addr).expect("send datagram");
    assert_eq!(mem.outbound_len(), 1);

    // The standard-library transport is made only through the network-access type, which
    // needs a consent proof (official-entry.md §6.1); tests open no socket (§6.4).

    // §3.2 item 3, Case A: an explicit port is tried once.
    let doc = Ipv4Addr::new(192, 0, 2, 10);
    assert_eq!(
        BindPorts::select(2900, true, Some(doc)),
        BindPorts::Explicit(2900)
    );
    assert_eq!(BindPorts::Explicit(2900).candidates(), vec![2900]);

    // Case B, the synthetic example: 192.0.2.10 is 3,221,225,994 (0xC000020A); modulo
    // 64,007 that is 9,712 (0x25F0); the first candidate is 9,968 (0x26F0), the next
    // 10,224 (0x27F0).
    assert_eq!(unique_port_ip_value(doc), 0xC000_020A);
    assert_eq!(unique_port_seed(doc), 9712);
    let ports_b = BindPorts::select(0, true, Some(doc));
    assert_eq!(ports_b, BindPorts::Unique { seed_ip: doc });
    let candidates = ports_b.candidates();
    assert_eq!(candidates.len(), MAX_UNIQUE_PORT_ATTEMPTS);
    assert_eq!(&candidates[..2], &[0x26F0, 0x27F0]);
    assert_eq!(candidates, unique_port_candidates(doc));
    // +256 modulo 65,536, then anything below 1,024 is raised to 1,024: 65,520 wraps to
    // 240 and becomes 1,024.
    assert_eq!(next_unique_port(65_520), 1024);
    assert_eq!(next_unique_port(1024), 1280);
    assert!(candidates.iter().all(|&p| p >= 1024));

    // Case C: no explicit port and either no unique-port computation or no first local
    // address: one bind to port 0.
    assert_eq!(BindPorts::select(0, false, Some(doc)), BindPorts::Ephemeral);
    assert_eq!(BindPorts::select(0, true, None), BindPorts::Ephemeral);
    assert_eq!(
        BindPorts::select(0, true, Some(Ipv4Addr::UNSPECIFIED)),
        BindPorts::Ephemeral
    );
    assert_eq!(BindPorts::Ephemeral.candidates(), vec![0]);

    // §3.2 item 1: the chat transport takes the defaults: the wildcard and Case B.
    assert_eq!(
        BindPlan::chat_defaults(Some(doc)),
        BindPlan {
            address: Ipv4Addr::UNSPECIFIED,
            ports: BindPorts::Unique { seed_ip: doc },
        }
    );

    // SO_SNDBUF binary search probe midpoint
    assert_eq!(sndbuf_probe_midpoint(65536, 131072), Some(98304));
    assert_eq!(sndbuf_probe_midpoint(131071, 131072), None);
}

// ---------------------------------------------------------------------------
// §10.2 Synthetic Test Vectors 1A, 1B, 2, 3, 4
// ---------------------------------------------------------------------------

#[test]
fn vector_1_datagram_checksum_aligned_and_unaligned() {
    // Vector 1A: 4-byte aligned buffer (len = 8)
    let buf_1a = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    assert_eq!(compute_datagram_checksum(&buf_1a), 0x0C0A_0806);

    // Vector 1B: Unaligned buffer with 2 trailing bytes (len = 6)
    let buf_1b = [0x01u8, 0x02, 0x03, 0x04, 0xAA, 0xBB];
    assert_eq!(compute_datagram_checksum(&buf_1b), 0xAEC4_0201);
}

#[test]
fn vector_2_unencoded_cumulative_ack_datagram() {
    let hdr = DatagramHeader {
        rec_id: 0x0102,
        pak_size: 4,
        flags: FLAG_CUMULATIVE_ACK,
        seq_num: 5,
        checksum: 0,
        iteration: 0,
        table: 0,
        time: 0x03E8,
    };
    assert_eq!(hdr.header_checksum(), 0xCEB3_DFBB);

    let opt = OptionalHeaders {
        cumulative_ack: Some(0x0000_000A),
        ..OptionalHeaders::default()
    };
    let dgram = encode_datagram(hdr, &opt, &[], None, None).expect("encode CumulativeAck");
    let expected: [u8; 26] = [
        0x00, 0x00, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x05, 0xD8,
        0xB3, 0xDF, 0xBB, 0x00, 0x00, 0x03, 0xE8, 0x00, 0x00, 0x00, 0x0A,
    ];
    assert_eq!(dgram, expected);

    let decoded = decode_datagram(&dgram, true, None, true).expect("decode CumulativeAck");
    assert_eq!(decoded.wire_prefix, 0x0000);
    assert_eq!(decoded.header.rec_id, 0x0102);
    assert_eq!(decoded.header.pak_size, 4);
    assert_eq!(decoded.header.flags, FLAG_CUMULATIVE_ACK);
    assert_eq!(decoded.header.seq_num, 5);
    assert_eq!(decoded.header.checksum, 0xD8B3_DFBB);
    assert_eq!(decoded.header.time, 0x03E8);
    assert_eq!(decoded.opt_headers.cumulative_ack, Some(10));
}

#[test]
fn vector_3_variable_u32_round_trip() {
    let cases: &[(u32, &[u8])] = &[
        (0x0000_0000, &[0x00]),
        (0x0000_007F, &[0x7F]),
        (0x0000_0080, &[0x80, 0x80]),
        (0x0000_0186, &[0x81, 0x86]),
        (0x0000_3FFF, &[0xBF, 0xFF]),
        (0x0000_4000, &[0xC0, 0x00, 0x00, 0x40]),
        (0x0102_0304, &[0xC1, 0x02, 0x04, 0x03]),
        (0x2000_0000, &[0xE0, 0x00, 0x00, 0x00, 0x20]),
        (0xDEAD_BEEF, &[0xE0, 0xEF, 0xBE, 0xAD, 0xDE]),
    ];

    for &(val, expected_bytes) in cases {
        let mut buf = Vec::new();
        let n = encode_variable_u32(val, &mut buf);
        assert_eq!(n, expected_bytes.len());
        assert_eq!(buf.as_slice(), expected_bytes);

        let (decoded, consumed) = decode_variable_u32(&buf).expect("decode_variable_u32");
        assert_eq!(decoded, val);
        assert_eq!(consumed, expected_bytes.len());
    }
    // §5.6.1: first bytes 0xE1..=0xFF are invalid.
    for b0 in [0xE1u8, 0xFF] {
        assert_eq!(
            decode_variable_u32(&[b0, 0, 0, 0, 0]),
            Err(SessionError::InvalidVariableU32)
        );
    }
    // Vector 3 table row: 0x3000001B uses the 5-byte form.
    let mut buf = Vec::new();
    encode_variable_u32(0x3000_001B, &mut buf);
    assert_eq!(buf, [0xE0, 0x1B, 0x00, 0x00, 0x30]);
}

#[test]
fn vector_4_compact_blob_fragment_header() {
    let hdr = BlobFragmentHeader {
        queue_id: 2,
        ext_word: None,
        blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
        seq_group: 0,
        order_num: 0x0007,
        blob_id: 0x002A,
        frag_index: 0,
        num_frags: 1,
        recipient_id: 2,
        blob_header_word: Some(0x0000_0186),
        frag_data_len: 0x0010,
        attachment: None,
    };

    let mut out = Vec::new();
    let written = hdr.encode(&mut out).expect("encode fragment header");
    let expected: [u8; 10] = [0x02, 0x40, 0x07, 0x00, 0x2A, 0x00, 0x81, 0x86, 0x10, 0x00];
    assert_eq!(written, 10);
    assert_eq!(out, expected);

    let (decoded, consumed) = BlobFragmentHeader::decode(&out).expect("decode fragment header");
    assert_eq!(consumed, 10);
    assert_eq!(decoded, hdr);
}

// ---------------------------------------------------------------------------
// §4.1 Bit-Tree Wire Codec & §7.1–§7.3 3DES-CBC + ISAAC PRNG
// ---------------------------------------------------------------------------

#[test]
fn bit_tree_wire_codec_round_trip() {
    let tables = BitTreeTable::synthetic_dictionary([0x11; 16]);
    let hdr = DatagramHeader {
        rec_id: 0x0234,
        pak_size: 0,
        flags: FLAG_CUMULATIVE_ACK,
        seq_num: 9,
        checksum: 0,
        iteration: 1,
        table: 2,
        time: 500,
    };
    let opt = OptionalHeaders {
        cumulative_ack: Some(9),
        ..OptionalHeaders::default()
    };

    let compressed_dgram =
        encode_datagram(hdr, &opt, &[], None, Some(&tables)).expect("encode compressed");
    // Wire prefix must equal rec_id (0x0234) in big-endian
    assert_eq!(&compressed_dgram[0..2], &[0x02, 0x34]);

    let decoded =
        decode_datagram(&compressed_dgram, true, Some(&tables), true).expect("decode compressed");
    assert_eq!(decoded.wire_prefix, 0x0234);
    assert_eq!(decoded.header.rec_id, 0x0234);
    assert_eq!(decoded.opt_headers.cumulative_ack, Some(9));
}

#[test]
fn des3_cbc_and_isaac_prng_invariants() {
    let key: [u8; 24] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ];
    // Length 0 is not round-trippable: §7.1 rejects a block that is all padding
    // (`des3_cbc_rejects_all_padding_block`).
    for len in [1usize, 7, 8, 15, 16, 43, 512] {
        let plain: Vec<u8> = (0..len).map(|i| (i * 37 + 13) as u8).collect();
        let ct = des3_cbc_encrypt(&key, &plain);
        assert_eq!(ct.len() % 8, 0);
        assert_eq!(ct.len(), plain.len() + (8 - (plain.len() % 8)));
        let dec = des3_cbc_decrypt(&key, &ct).expect("3des decrypt");
        assert_eq!(dec, plain);
    }

    // ISAAC deterministic seeding and reverse consumption across 256-word refill boundary
    let mut isaac_a = IsaacCipher::from_seed(0x1234_5678);
    let mut isaac_b = IsaacCipher::from_seed(0x1234_5678);
    let mut isaac_diff = IsaacCipher::from_seed(0x8765_4321);

    let first_a = isaac_a.next_mask();
    let first_diff = isaac_diff.next_mask();
    assert_ne!(first_a, first_diff);
    assert_eq!(first_a, isaac_b.next_mask());

    // Advance past 256 words to trigger isaac() block regeneration at randcnt == 0
    for _ in 1..300 {
        assert_eq!(isaac_a.next_mask(), isaac_b.next_mask());
    }
}

// ---------------------------------------------------------------------------
// §6.2 SessionAuthHeader and the authType selection rule (§6.2.1)
// ---------------------------------------------------------------------------

/// A synthetic 144-character Base64 ticket text.
fn synthetic_ticket() -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    (0..144usize)
        .map(|i| char::from(ALPHABET[(i * 7) % 64]))
        .collect()
}

/// §10.2 Vector 6: the complete first datagram of the primary connection.
const VECTOR_6: &str = "00 00 00 00 00 75 00 01 00 00 00 00 00 01 08 35 15 01 00 00 00 00 3F 31 38 30 38 30 34 5F 6E 65 74 76 65 72 3A 31 32 33 34 3B 20 64 69 64 76 65 72 3A 30 31 30 32 30 33 30 34 2D 30 35 30 36 2D 30 37 30 38 2D 30 39 30 41 2D 30 42 30 43 30 44 30 45 30 46 31 30 31 00 00 00 01 00 00 00 04 00 00 00 00 00 00 60 05 53 00 55 00 42 00 30 00 31 00 16 00 00 00 15 53 59 4E 54 48 2B 54 49 43 4B 45 54 2F 41 42 43 31 32 33 3D 3D";

#[test]
fn vector_6_first_datagram_is_byte_exact() {
    let header = ticket_auth("SUB01", "SYNTH+TICKET/ABC123==", 0x6000_0000);
    assert_eq!(
        header.net_version,
        "180804_netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"
    );
    assert_eq!(header.net_version.len(), 63);
    assert_eq!((header.auth_type, header.auth_flags), (0x0000_0001, 0x04));

    let server = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9000);
    let mut conn = SessionConnection::new(server, 0.0);
    assert_eq!(conn.local_rec_id(), 0);
    assert_eq!(conn.send_seq_num(), 1);
    // The clock of the first datagram does not reach the wire: `time` is 0 (§4.2).
    conn.start_login(header.clone(), 12.345).unwrap();
    let first = conn.pop_outbound_datagram().expect("first datagram");
    let expected = hex(VECTOR_6);
    assert_eq!(expected.len(), 139);
    assert_eq!(first, expected);

    // Its parts: a 117-byte payload, payloadSum 0xB4C43647, hdrSum 0x5370DEBA.
    let parsed = decode_datagram(&first, false, None, true).unwrap();
    assert_eq!(parsed.header.pak_size, 117);
    assert_eq!(compute_datagram_checksum(&parsed.raw_payload), 0xB4C4_3647);
    assert_eq!(parsed.header.header_checksum(), 0x5370_DEBA);
    assert_eq!(parsed.header.checksum, 0x0835_1501);
    assert_eq!(
        (
            parsed.header.seq_num,
            parsed.header.iteration,
            parsed.header.table,
            parsed.header.time
        ),
        (1, 0, 0, 0)
    );
    assert_eq!(parsed.opt_headers.session_auth, Some(header));

    // The retransmit in State 1 is the same datagram (`seqNum` 1, `time` 0).
    conn.tick(14.5).unwrap();
    assert_eq!(conn.pop_outbound_datagram().expect("retransmit"), expected);
    // Both send checksum rules give the same bytes: one buffer.
    let mut whole = SessionConnection::new(server, 0.0);
    whole.set_send_checksum_rule(SendChecksumRule::WholePayload);
    whole
        .start_login(
            ticket_auth("SUB01", "SYNTH+TICKET/ABC123==", 0x6000_0000),
            0.0,
        )
        .unwrap();
    assert_eq!(whole.pop_outbound_datagram().unwrap(), expected);
}

#[test]
fn session_auth_selection_rule_per_connection() {
    let ticket = synthetic_ticket();
    assert_eq!(ticket.len(), 144);

    // Rule 1: primary connection with a ticket -> 0x00000001 + HasExtraBlob; the ticket
    // travels as its ASCII text after `extraBlobLen = 2 + 144 = 146` and `80 90`.
    let with_ticket = ticket_auth("SUBSCRIPTION", &ticket, 1_000);
    assert_eq!(with_ticket.auth_type, AUTH_TYPE_PRIMARY);
    assert_eq!(with_ticket.auth_flags, AUTH_FLAG_HAS_EXTRA_BLOB);
    assert_eq!(with_ticket.credentials.ticket.as_deref(), Some(&ticket[..]));
    let mut buf = Vec::new();
    let len = with_ticket.encode(&mut buf).unwrap();
    assert_eq!(len, buf.len());
    assert_eq!(buf[0], 63);
    assert_eq!(&buf[1..8], b"180804_");
    let block_len = u32::from_le_bytes([buf[64], buf[65], buf[66], buf[67]]) as usize;
    // Exact, unpadded: the 12-byte prefix, the account string and the extra blob.
    assert_eq!(block_len, 12 + 1 + 2 * 12 + 4 + 146);
    assert_eq!(len, 64 + 4 + block_len);
    assert_eq!(&buf[68..76], &[0x01, 0, 0, 0, 0x04, 0, 0, 0]);
    let blob_at = 68 + 12 + 1 + 2 * 12;
    assert_eq!(&buf[blob_at..blob_at + 4], &146u32.to_le_bytes());
    assert_eq!(&buf[blob_at + 4..blob_at + 6], &[0x80, 0x90]);
    assert_eq!(&buf[blob_at + 6..], ticket.as_bytes());
    let (decoded, consumed) = SessionAuthHeader::decode(&buf).unwrap();
    assert_eq!(consumed, len);
    assert_eq!(decoded, with_ticket);

    // With a password as well: 0x06 and `secondStr`.
    let with_password = SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        VERSION,
        "SUBSCRIPTION",
        Some(&ticket),
        Some("pw"),
        1,
    )
    .unwrap();
    assert_eq!(
        with_password.auth_flags,
        AUTH_FLAG_HAS_EXTRA_BLOB | AUTH_FLAG_HAS_SECOND_STRING
    );
    let mut buf = Vec::new();
    with_password.encode(&mut buf).unwrap();
    assert_eq!(SessionAuthHeader::decode(&buf).unwrap().0, with_password);

    // Rule 2: the chat connection: `netVersion` "180804" (7 wire bytes), 0x40000002, 0x04.
    let chat = SessionAuthHeader::for_connection(
        AuthConnection::Chat,
        "ignored",
        "SUBSCRIPTION",
        Some(&ticket),
        None,
        1,
    )
    .unwrap();
    assert_eq!(chat.net_version, "180804");
    assert_eq!((chat.auth_type, chat.auth_flags), (AUTH_TYPE_CHAT, 0x04));
    let mut buf = Vec::new();
    chat.encode(&mut buf).unwrap();
    assert_eq!(&buf[..7], &[0x06, 0x31, 0x38, 0x30, 0x38, 0x30, 0x34]);
    assert_eq!(&buf[11..19], &[0x02, 0, 0, 0x40, 0x04, 0, 0, 0]);
    assert_eq!(SessionAuthHeader::decode(&buf).unwrap().0, chat);

    // Rule 3: account only, on either connection: authFlags 0, the account string only.
    let account_only = account_auth("SUBSCRIPTION", 1_000).unwrap();
    assert_eq!(
        (account_only.auth_type, account_only.auth_flags),
        (AUTH_TYPE_PRIMARY, 0)
    );
    let mut buf = Vec::new();
    account_only.encode(&mut buf).unwrap();
    assert_eq!(SessionAuthHeader::decode(&buf).unwrap().0, account_only);
    let chat_account =
        SessionAuthHeader::for_connection(AuthConnection::Chat, "", "A", Some(""), None, 0)
            .unwrap();
    assert_eq!(
        (chat_account.auth_type, chat_account.auth_flags),
        (AUTH_TYPE_CHAT, 0)
    );

    // Combinations outside the rule are refused.
    assert_eq!(
        SessionAuthHeader::for_connection(
            AuthConnection::Primary,
            VERSION,
            "A",
            None,
            Some("p"),
            0
        ),
        Err(SessionError::UnsupportedAuthMode {
            auth_type: AUTH_TYPE_PRIMARY,
            auth_flags: AUTH_FLAG_HAS_SECOND_STRING
        })
    );
    assert_eq!(
        SessionAuthHeader::for_connection(
            AuthConnection::Chat,
            "",
            "A",
            Some(&ticket),
            Some("p"),
            0
        ),
        Err(SessionError::UnsupportedAuthMode {
            auth_type: AUTH_TYPE_CHAT,
            auth_flags: 0x06
        })
    );
    // The ticket is text: never decoded, but it must be printable ASCII.
    assert_eq!(check_ticket_text("QU!I"), Ok(()));
    for bad in ["not base64!", "tab\there", "\u{e9}"] {
        assert_eq!(
            SessionAuthHeader::for_connection(
                AuthConnection::Primary,
                VERSION,
                "A",
                Some(bad),
                None,
                0
            ),
            Err(SessionError::InvalidTicketEncoding),
            "{bad:?}"
        );
    }
    assert_eq!(
        check_ticket_text(&"A".repeat(4096)),
        Err(SessionError::InvalidTicketEncoding)
    );
    // A version stamp that does not fit `netVersion` is refused.
    assert_eq!(
        SessionAuthHeader::for_connection(
            AuthConnection::Primary,
            &"x".repeat(200),
            "A",
            None,
            None,
            0
        ),
        Err(SessionError::InvalidSessionAuthVersion)
    );
}

#[test]
fn session_auth_accepts_and_emits_only_the_listed_modes() {
    let header = account_auth("A", 0).unwrap();
    let mut buf = Vec::new();
    header.encode(&mut buf).unwrap();
    // authType at authBlock + 0: after the 64-byte netVersion and the 4-byte authBlockLen.
    let with_mode = |auth_type: u32, auth_flags: u32| {
        let mut bad = buf.clone();
        bad[68..72].copy_from_slice(&auth_type.to_le_bytes());
        bad[72..76].copy_from_slice(&auth_flags.to_le_bytes());
        bad
    };
    for (auth_type, auth_flags) in [
        (AUTH_TYPE_PRIMARY, 0x01),
        (AUTH_TYPE_PRIMARY, 0x02),
        (AUTH_TYPE_PRIMARY, 0x08),
        (AUTH_TYPE_PRIMARY, 0x800),
        (AUTH_TYPE_CHAT, 0x02),
        (AUTH_TYPE_CHAT, 0x06),
        (0x0000_0002, 0x00),
    ] {
        assert_eq!(
            SessionAuthHeader::decode(&with_mode(auth_type, auth_flags)),
            Err(SessionError::UnsupportedAuthMode {
                auth_type,
                auth_flags
            }),
            "({auth_type:#x}, {auth_flags:#x})"
        );
    }
    // The encoder writes auth_type / auth_flags as given and never replaces them.
    let mut bad_mode = header.clone();
    bad_mode.auth_type = 0x0000_0002;
    assert_eq!(
        bad_mode.encode(&mut Vec::new()),
        Err(SessionError::UnsupportedAuthMode {
            auth_type: 2,
            auth_flags: 0
        })
    );
    // Credentials that do not match authFlags are an error, not a silent flag change.
    let mut missing_ticket = ticket_auth("A", "QUJD", 0);
    missing_ticket.credentials.ticket = None;
    assert_eq!(
        missing_ticket.encode(&mut Vec::new()),
        Err(SessionError::AuthCredentialsMismatch {
            auth_flags: AUTH_FLAG_HAS_EXTRA_BLOB
        })
    );
    let mut extra_ticket = header.clone();
    extra_ticket.credentials.ticket = Some("QUJD".to_owned());
    assert_eq!(
        extra_ticket.encode(&mut Vec::new()),
        Err(SessionError::AuthCredentialsMismatch { auth_flags: 0 })
    );
    // `authBlockLen` is exact: a byte after the credentials, inside the block, is refused.
    let mut longer = buf.clone();
    let block_len = u32::from_le_bytes([longer[64], longer[65], longer[66], longer[67]]);
    longer[64..68].copy_from_slice(&(block_len + 1).to_le_bytes());
    longer.push(0);
    assert_eq!(
        SessionAuthHeader::decode(&longer),
        Err(SessionError::InvalidAuthBlock)
    );
    // An `extraBlobLen` that does not match its string is refused.
    let mut buf = Vec::new();
    ticket_auth("A", "QUJD", 0).encode(&mut buf).unwrap();
    let blob_at = 68 + 12 + 3;
    assert_eq!(&buf[blob_at..blob_at + 4], &5u32.to_le_bytes());
    let mut wrong = buf.clone();
    wrong[blob_at + 4] = 3;
    assert_eq!(
        SessionAuthHeader::decode(&wrong),
        Err(SessionError::InvalidAuthBlock)
    );
    // `netVersion` must be "180804" or "180804_" with a stamp.
    let mut bad_version = buf.clone();
    bad_version[1] = b'2';
    assert_eq!(
        SessionAuthHeader::decode(&bad_version),
        Err(SessionError::InvalidSessionAuthVersion)
    );
    let old_magic = [b"180804\0".as_slice(), &[16, 0, 0, 0]].concat();
    assert!(SessionAuthHeader::decode(&old_magic).is_err());
}

// ---------------------------------------------------------------------------
// §6.3–§6.4 ConnectChallengeHeader -> ConnectAcceptHeader (§10.2 Vector 5)
// ---------------------------------------------------------------------------

fn vector_5_challenge() -> ConnectChallengeHeader {
    ConnectChallengeHeader {
        server_time: 1000.0,
        assigned_client_rec_id: 0x0102,
        server_rec_id: 0x0304,
        outgoing_seed: 0x1122_3344,
        incoming_seed: 0x5566_7788,
        reserved_dword_14: 0,
        client_conn_idx: 0x0005,
        reserved_word_1a: 0,
        codec_uuid: [0u8; 16],
    }
}

/// §10.2 Vector 5: the complete challenge datagram.
const VECTOR_5_CHALLENGE: &str = "00 00 00 00 00 2C 00 04 00 00 00 00 00 00 19 AE AB 27 00 00 00 00 00 00 00 00 00 40 8F 40 02 01 04 03 11 22 33 44 55 66 77 88 00 00 00 00 05 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

/// §10.2 Vector 5: the complete accept datagram of the primary connection.
const VECTOR_5_ACCEPT: &str = "00 00 03 04 00 1C 00 08 00 00 00 00 00 01 0D 94 21 04 00 00 00 00 44 33 22 11 03 04 01 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

#[test]
fn vector_5_challenge_and_accept_are_byte_exact() {
    let challenge = vector_5_challenge();
    // The challenge datagram as the spec gives it, checksum 0x19AEAB27.
    let dgram = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    assert_eq!(dgram, hex(VECTOR_5_CHALLENGE));
    assert_eq!(challenge.check_value(), 0x0201_0403_1122_3344);

    // The primary connection answers with the Vector 5 accept datagram.
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 5), 9000);
    let mut conn = SessionConnection::new(server, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    conn.pop_outbound_datagram().expect("SessionAuthHeader");
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), server, 7.5, WireCodecs::NONE)
        .unwrap();
    // §6.4 step 1: the assigned ID becomes the local ID (Vector 5: 0x0102), the server's ID
    // the remote ID.
    assert_eq!(conn.local_rec_id(), 0x0102);
    assert_eq!(conn.server_rec_id(), 0x0304);
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    let accept = conn.pop_outbound_datagram().expect("ConnectAcceptHeader");
    assert_eq!(accept, hex(VECTOR_5_ACCEPT));
    let parsed = decode_datagram(&accept, false, None, true).unwrap();
    assert_eq!(parsed.header.header_checksum(), 0xFA70_E9BD);
    assert_eq!(compute_datagram_checksum(&parsed.raw_payload), 0x1323_3747);
    let resp = parsed.opt_headers.connect_accept.unwrap();
    assert_eq!(resp.check_value, 0x0201_0403_1122_3344);
    assert_eq!(resp.codec_uuid, [0u8; 16]);
    assert_eq!(resp.max_recv_rate, 0);
    assert_eq!(parsed.header.flags & FLAG_MASKED_CHECKSUM, 0);
    // The State 2 retransmit is the same datagram.
    conn.tick(8.2).unwrap();
    assert_eq!(conn.pop_outbound_datagram().unwrap(), hex(VECTOR_5_ACCEPT));

    // On the chat connection `maxRecvRate` is 0x00040000 (`00 00 04 00` at +0x18).
    let mut chat = SessionConnection::new(server, 0.0);
    chat.set_max_recv_rate(CHAT_MAX_RECV_RATE);
    chat.start_login(
        SessionAuthHeader::for_connection(AuthConnection::Chat, "", "A", None, None, 0).unwrap(),
        0.0,
    )
    .unwrap();
    chat.pop_outbound_datagram().expect("SessionAuthHeader");
    chat.receive_datagram(&hex(VECTOR_5_CHALLENGE), server, 0.1, WireCodecs::NONE)
        .unwrap();
    let accept = chat.pop_outbound_datagram().expect("accept");
    assert_eq!(&accept[22 + 0x18..22 + 0x1C], &[0x00, 0x00, 0x04, 0x00]);
    // checkValue and codecUuid are those of the primary connection's accept.
    assert_eq!(&accept[22..22 + 0x18], &hex(VECTOR_5_ACCEPT)[22..22 + 0x18]);

    // `Net.ConnectionSpeed`: floor(kbps × 102.4), 0 for the default 0.00.
    assert_eq!(max_recv_rate_for_speed(0.0), 0);
    assert_eq!(max_recv_rate_for_speed(1000.0), 102_400);
    assert_eq!(max_recv_rate_for_speed(1.5), 153);
    assert_eq!(max_recv_rate_for_speed(-3.0), 0);
    assert_eq!(max_recv_rate_for_speed(f64::NAN), 0);
}

#[test]
fn assigned_rec_id_zero_keeps_and_out_of_range_is_ignored() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 6), 9000);
    for assigned in [0u16, 0x0001, 0xFF00] {
        let mut conn = SessionConnection::new(server, 0.0);
        conn.start_login(account_auth("A", 0).unwrap(), 0.0)
            .unwrap();
        let mut challenge = vector_5_challenge();
        challenge.assigned_client_rec_id = assigned;
        let dgram = encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders {
                connect_challenge: Some(challenge),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .unwrap();
        conn.receive_datagram(&dgram, server, 0.1, WireCodecs::NONE)
            .unwrap();
        // §3.4: the local ID starts at 0x0000 and stays there.
        assert_eq!(conn.local_rec_id(), 0, "{assigned:#x}");
    }
}

#[test]
fn server_rec_id_zero_keeps_the_packets_rec_id() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 6), 9000);
    let mut conn = SessionConnection::new(server, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    conn.pop_outbound_datagram();
    let mut challenge = vector_5_challenge();
    challenge.server_rec_id = 0;
    let dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0777,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    conn.receive_datagram(&dgram, server, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.server_rec_id(), 0x0777);
    let accept =
        decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, true).unwrap();
    assert_eq!(accept.header.rec_id, 0x0777);
}

#[test]
fn unsupported_codec_uuid_closes_with_reason() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 7), 9000);
    let mut conn = SessionConnection::new(server, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    let mut challenge = vector_5_challenge();
    challenge.codec_uuid = [0xAB; 16];
    let dgram = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        conn.receive_datagram(&dgram, server, 0.1, WireCodecs::NONE)
            .unwrap_err(),
        SessionError::UnsupportedCodecUuid { uuid: [0xAB; 16] }
    );
    assert_eq!(conn.state(), ConnectionState::Closing);
    assert_eq!(
        conn.pop_event(),
        Some(SessionEvent::Disconnected {
            reason: DisconnectReason::UnsupportedCodec
        })
    );
    // A registered dictionary UUID is accepted.
    let mut conn = SessionConnection::new(server, 0.0);
    conn.register_codec_uuid([0xAB; 16]);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    conn.receive_datagram(&dgram, server, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.active_codec_uuid(), [0xAB; 16]);
}

#[test]
fn all_optional_headers_ascending_bit_order() {
    let opt = OptionalHeaders {
        server_redirect: Some(ServerRedirectHeader {
            switch_seq: 11,
            switch_type: 2,
        }),
        auth_server_address: Some(AuthServerAddressHeader {
            sin_family: 2,
            addr: SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 55), 2900),
        }),
        world_referral: Some(WorldReferralHeader {
            referral_cookie: 0x0102_0304_0506_0708,
            sin_family: 2,
            target_addr: SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 99), 9050),
            reserved_word_10: 0,
        }),
        sequence_nak: Some(vec![3, 4]),
        out_of_order_ack: Some(vec![2]),
        cumulative_ack: Some(10),
        disconnect: true,
        session_auth: None,
        referral_auth: None,
        connect_challenge: Some(ConnectChallengeHeader {
            server_time: 1234.5,
            assigned_client_rec_id: 0x0150,
            server_rec_id: 0x0250,
            outgoing_seed: 0x1122_3344,
            incoming_seed: 0x5566_7788,
            reserved_dword_14: 0,
            client_conn_idx: 3,
            reserved_word_1a: 0,
            codec_uuid: [0u8; 16],
        }),
        connect_accept: None,
        transport_error: Some(TransportErrorHeader {
            error_code: 0x00A7_E948,
            error_subcode: 0,
        }),
        transport_fatal_error: None,
        client_control: None,
        clock_sync: Some(4567.89),
        flow_control: Some(FlowControlHeader {
            bytes_received_delta: 2048,
            interval_ms: 500,
        }),
    };

    let flags = opt.flags_mask();
    let mut payload = Vec::new();
    opt.encode(&mut payload).expect("encode optional headers");

    let (decoded, consumed) =
        OptionalHeaders::decode(flags, &payload, true).expect("decode optional headers");
    assert_eq!(consumed, payload.len());
    assert_eq!(decoded, opt);

    // Client-originated ConnectAccept + FlowControl, and ReferralAuth + CumulativeAck (each packet has at most one handshake header per §10.1 rule 8)
    let opt_resp = OptionalHeaders {
        connect_accept: Some(ConnectAcceptHeader {
            check_value: 99,
            codec_uuid: [0u8; 16],
            max_recv_rate: 3,
        }),
        flow_control: Some(FlowControlHeader {
            bytes_received_delta: 1024,
            interval_ms: 250,
        }),
        ..OptionalHeaders::default()
    };
    let mut resp_payload = Vec::new();
    opt_resp.encode(&mut resp_payload).expect("encode resp");
    let (dec_resp, c_resp) = OptionalHeaders::decode(opt_resp.flags_mask(), &resp_payload, false)
        .expect("decode ConnectAccept");
    assert_eq!(c_resp, resp_payload.len());
    assert_eq!(dec_resp, opt_resp);

    let opt_world = OptionalHeaders {
        cumulative_ack: Some(10),
        referral_auth: Some(ReferralAuthHeader {
            referral_cookie: 0xCAFE_BABE_1234_5678,
        }),
        ..OptionalHeaders::default()
    };
    let mut world_payload = Vec::new();
    opt_world.encode(&mut world_payload).expect("encode world");
    let (dec_world, c_world) =
        OptionalHeaders::decode(opt_world.flags_mask(), &world_payload, false)
            .expect("decode ReferralAuth");
    assert_eq!(c_world, world_payload.len());
    assert_eq!(dec_world, opt_world);

    // §6.7: the two confirmed login-refusal codes.
    for code in [0x00A7_E948, 0x7DC4_1B4A] {
        assert!(
            TransportErrorHeader {
                error_code: code,
                error_subcode: 0
            }
            .is_login_refusal()
        );
    }
    assert!(!TransportErrorHeader::default().is_login_refusal());
}

// ---------------------------------------------------------------------------
// §5.1–§5.6, §6.1–§6.5 Handshake, Out-of-Order SequenceNak + Saved ISAAC Mask, and WorldReferral
// ---------------------------------------------------------------------------

#[test]
fn handshake_out_of_order_isaac_recovery_and_referral_flow() {
    let client_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9100);
    let login_server_addr = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 9000);
    let world_server_addr = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 2), 9001);

    let mut transport = InMemoryUdpTransport::new(client_addr);
    let mut mux = SessionMultiplexer::new();
    let login_id = mux
        .create_connection(login_server_addr, 0.0)
        .expect("free recipient ID");

    // Step 1: Client starts login -> State 1 (SessionAuthSent)
    let login_req = account_auth("acct", 1000).expect("SessionAuthHeader");
    mux.connection_mut(login_id)
        .expect("conn")
        .start_login(login_req, 0.0)
        .expect("start_login");
    assert_eq!(
        mux.connection(login_id).expect("conn").state(),
        ConnectionState::SessionAuthSent
    );
    assert_eq!(mux.flush_outbound(&mut transport).expect("flush"), 1);
    let (sent_login_dgram, dst) = transport.pop_outbound().expect("outbound SessionAuth");
    assert_eq!(dst, login_server_addr);
    let parsed_login =
        decode_datagram(&sent_login_dgram, false, None, true).expect("parse SessionAuth");
    assert!(parsed_login.opt_headers.session_auth.is_some());

    // Step 2: Server sends ConnectChallenge (0x00040000) assigning recId = 0x0111
    let outgoing_seed = 0xCAFE_1234u32;
    let incoming_seed = 0xBEEF_5678u32;
    let cr_opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            server_time: 500.0,
            assigned_client_rec_id: 0x0111,
            server_rec_id: 0x0222,
            outgoing_seed,
            incoming_seed,
            reserved_dword_14: 0,
            client_conn_idx: 4,
            reserved_word_1a: 0,
            codec_uuid: [0u8; 16],
        }),
        ..OptionalHeaders::default()
    };
    let cr_dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0111,
            ..DatagramHeader::default()
        },
        &cr_opt,
        &[],
        None,
        None,
    )
    .expect("encode ConnectChallenge");

    let (routed_id, _) = mux
        .receive_from_peer(&cr_dgram, login_server_addr, 0.1, WireCodecs::NONE)
        .expect("receive ConnectChallenge");
    // The handle stays; the connection takes the assigned recipient ID (§3.4).
    assert_eq!(routed_id, login_id);
    let conn = mux.connection(routed_id).expect("conn after rekey");
    assert_eq!(conn.local_rec_id(), 0x0111);
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    assert_eq!(conn.server_rec_id(), 0x0222);

    // Client queued ConnectAccept (0x00080000)
    assert_eq!(mux.flush_outbound(&mut transport).expect("flush"), 1);
    let (resp_dgram, _) = transport.pop_outbound().expect("ConnectAccept");
    let parsed_resp = decode_datagram(&resp_dgram, false, None, true).expect("parse resp");
    assert_eq!(
        parsed_resp
            .opt_headers
            .connect_accept
            .expect("resp")
            .max_recv_rate,
        0
    );
    assert_eq!(parsed_resp.header.rec_id, 0x0222);

    // Step 3: Server sends sequenced packets seq = 2, 3, 4 (multi-fragment fragmented blob),
    // but packet seq = 4 arrives BEFORE seq = 2 and seq = 3!
    let mut server_isaac = IsaacCipher::from_seed(incoming_seed);
    let mask_seq_2 = server_isaac.next_mask();
    let mask_seq_3 = server_isaac.next_mask();
    let mask_seq_4 = server_isaac.next_mask();

    let large_payload: Vec<u8> = (0..1000usize).map(|i| (i & 0xFF) as u8).collect();
    let frags = split_blob(2, 1, 42, 0, None, 0x0000_1234, &large_payload).expect("split 3 frags");
    assert_eq!(frags.len(), 3);

    let make_server_seq_pkt = |seq_num: u32, mask: u32, frag: BlobFragment| -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0111,
                seq_num,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &[frag],
            Some(mask),
            None,
        )
        .expect("encode server seq pkt")
    };

    let pkt_seq_2 = make_server_seq_pkt(2, mask_seq_2, frags[0].clone());
    let pkt_seq_3 = make_server_seq_pkt(3, mask_seq_3, frags[1].clone());
    let pkt_seq_4 = make_server_seq_pkt(4, mask_seq_4, frags[2].clone());

    // Deliver seq = 4 first!
    mux.receive_from_peer(&pkt_seq_4, login_server_addr, 0.2, WireCodecs::NONE)
        .expect("receive seq 4 out of order");
    {
        let conn = mux.connection_mut(routed_id).expect("conn");
        // Connection transitioned State 2 -> State 5 and queued initial ClockSync
        assert_eq!(conn.state(), ConnectionState::Connected);
        assert_eq!(conn.recv_seq_expected(), 4);
        assert_eq!(conn.missing_gap_count(), 2); // gaps for seq 2 and 3

        // Tick at t = 0.9 s (>= 0.6 s) to trigger SequenceNak for missing seqs [2, 3]
        conn.tick(0.9).expect("tick nak");
    }
    // Drain initial ClockSync and SequenceNak
    let flushed = mux.flush_outbound(&mut transport).expect("flush");
    assert_eq!(flushed, 2);
    let _initial_ts = transport.pop_outbound().expect("initial ClockSync");
    let (sequence_nak_dgram, _) = transport.pop_outbound().expect("SequenceNak datagram");
    let parsed_sequence_nak =
        decode_datagram(&sequence_nak_dgram, false, None, true).expect("parse SequenceNak");
    assert_eq!(
        parsed_sequence_nak.opt_headers.sequence_nak,
        Some(vec![2, 3])
    );

    // Now deliver out-of-order seq = 2 and seq = 3; their checksums must verify against saved ISAAC masks!
    mux.receive_from_peer(&pkt_seq_2, login_server_addr, 1.0, WireCodecs::NONE)
        .expect("receive seq 2 using saved ISAAC mask");
    mux.receive_from_peer(&pkt_seq_3, login_server_addr, 1.1, WireCodecs::NONE)
        .expect("receive seq 3 using saved ISAAC mask");

    {
        let conn = mux.connection_mut(routed_id).expect("conn");
        assert_eq!(conn.missing_gap_count(), 0);
        let mut got_blob = None;
        while let Some(ev) = conn.pop_event() {
            if let SessionEvent::BlobReceived(blob) = ev {
                got_blob = Some(blob);
            }
        }
        let blob = got_blob.expect("reassembled 3-fragment fragmented blob");
        assert_eq!(blob.blob_header_word, 0x0000_1234);
        assert_eq!(blob.payload, large_payload);
    }

    // Step 4: Server sends WorldReferral (0x00000800) to world_server_addr
    let ref_dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0111,
            seq_num: 4,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            world_referral: Some(WorldReferralHeader {
                referral_cookie: 0xABCD_EF01_2345_6789,
                sin_family: 2,
                target_addr: world_server_addr,
                reserved_word_10: 9,
            }),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("encode WorldReferral");
    mux.receive_from_peer(&ref_dgram, login_server_addr, 1.2, WireCodecs::NONE)
        .expect("receive WorldReferral");

    let mut referral_event = None;
    while let Some(ev) = mux.connection_mut(routed_id).expect("conn").pop_event() {
        if let SessionEvent::WorldReferralReceived {
            referral_cookie,
            world_addr,
            reserved_word_10,
        } = ev
        {
            referral_event = Some((referral_cookie, world_addr, reserved_word_10));
        }
    }
    let (cookie, resolved_world_addr, world_id) =
        referral_event.expect("WorldReferralReceived event");
    assert_eq!(cookie, 0xABCD_EF01_2345_6789);
    assert_eq!(resolved_world_addr, world_server_addr);
    assert_eq!(world_id, 9);

    // Open new world connection on the same SessionMultiplexer and send ReferralAuth (0x00020000)
    let world_conn_id = mux
        .create_connection(resolved_world_addr, 1.2)
        .expect("free recipient ID");
    mux.connection_mut(world_conn_id)
        .expect("world conn")
        .start_world_login(cookie, 1.2)
        .expect("start_world_login");
    assert_eq!(mux.flush_outbound(&mut transport).expect("flush"), 1);
    let (wlr_dgram, wlr_dst) = transport.pop_outbound().expect("ReferralAuth");
    assert_eq!(wlr_dst, world_server_addr);
    let parsed_wlr = decode_datagram(&wlr_dgram, false, None, true).expect("parse WLR");
    assert_eq!(
        parsed_wlr
            .opt_headers
            .referral_auth
            .expect("wlr")
            .referral_cookie,
        cookie
    );
}

// ---------------------------------------------------------------------------
// §9.1–§9.2 MessageEnvelope Zlib Envelope & Recursive 0x00000186 Batch Container
// ---------------------------------------------------------------------------

#[test]
fn message_envelope_zlib_and_recursive_batch_container_0x186() {
    let sub1 = encode_message(0, &[0x10, 0x20, 0x30], false).expect("encode sub1");
    let sub2 = encode_message(0, &[0x40; 200], true).expect("encode compressed sub2");
    let wrapped_sub2 =
        encode_single_wrapper(0x0000_0202, &sub2, false).expect("encode 0x179 wrapper");

    let batch_payload = encode_batch_container(
        &[
            (0x0000_0201, sub1),
            (tessera_compat::session::OPCODE_SINGLE_WRAPPER, wrapped_sub2),
        ],
        true,
    )
    .expect("encode 0x186 batch container");

    let dispatched = dispatch_message(
        OPCODE_BATCH_CONTAINER,
        &batch_payload,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
        DEFAULT_MAX_CONTAINER_DEPTH,
    )
    .expect("dispatch_message");

    assert_eq!(dispatched.len(), 2);
    assert_eq!(dispatched[0].opcode, 0x0000_0201);
    assert_eq!(dispatched[0].body, vec![0x10, 0x20, 0x30]);
    assert_eq!(dispatched[1].opcode, 0x0000_0202);
    assert_eq!(dispatched[1].body, vec![0x40; 200]);
    assert_ne!(
        dispatched[1].envelope_flags & ENVELOPE_FLAG_ZLIB_COMPRESSED,
        0
    );

    // 0x179 carries contextId (u64) and resultCode (u32) after the inner payload (§9.4.2).
    let inner = encode_message(0, &[0x00, 0x01], false).unwrap();
    let wrapped = encode_single_wrapper_with_context(0x0000_0203, &inner, 7, 0, false).unwrap();
    let out = dispatch_message(
        OPCODE_SINGLE_WRAPPER,
        &wrapped,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
        DEFAULT_MAX_CONTAINER_DEPTH,
    )
    .unwrap();
    assert_eq!(out[0].opcode, 0x0000_0203);
    assert_eq!(
        out[0].wrapper_context,
        Some(EnvelopeContext {
            context_id: 7,
            result_code: 0,
        })
    );
    // Messages not unwrapped from a 0x179 carry no context.
    assert_eq!(dispatched[0].wrapper_context, None);
    assert_eq!(
        dispatched[1].wrapper_context,
        Some(EnvelopeContext::default())
    );
    // §9.4.2: bytes after resultCode are ignored.
    let mut long_body = Vec::new();
    encode_variable_u32(0x0000_0203, &mut long_body);
    long_body.extend_from_slice(&u32::try_from(inner.len()).unwrap().to_le_bytes());
    long_body.extend_from_slice(&inner);
    long_body.extend_from_slice(&7u64.to_le_bytes());
    long_body.extend_from_slice(&9u32.to_le_bytes());
    long_body.push(0xEE);
    let long = encode_message(0, &long_body, false).unwrap();
    let unwrapped = dispatch_message(
        OPCODE_SINGLE_WRAPPER,
        &long,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
        DEFAULT_MAX_CONTAINER_DEPTH,
    )
    .expect("trailing bytes are ignored");
    assert_eq!(unwrapped.len(), 1);
    assert_eq!(unwrapped[0].opcode, 0x0000_0203);
    assert_eq!(
        unwrapped[0].wrapper_context,
        Some(EnvelopeContext {
            context_id: 7,
            result_code: 9,
        })
    );
    let truncated = &wrapped[..wrapped.len() - 1];
    assert_eq!(
        dispatch_message(
            OPCODE_SINGLE_WRAPPER,
            truncated,
            DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
            DEFAULT_MAX_CONTAINER_DEPTH,
        ),
        Err(SessionError::InvalidControlMessage {
            opcode: OPCODE_SINGLE_WRAPPER
        })
    );
}

// ---------------------------------------------------------------------------
// §10.1 Hostile-Input Rejection Rules (Rules 1 through 8)
// ---------------------------------------------------------------------------

#[test]
fn hostile_input_rejection_rules_1_to_8() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 10), 9000);
    let wrong_peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 99), 9000);

    // Rule 1: Datagram shorter than 22 bytes
    assert!(matches!(
        decode_datagram(&[0u8; 21], true, None, true),
        Err(SessionError::DatagramTooShort { len: 21 })
    ));

    // Rule 1: pakSize mismatch with decoded_len - 20
    let mut bad_pak_size = [0u8; 22];
    bad_pak_size[4..6].copy_from_slice(&4u16.to_be_bytes()); // pak_size = 4, but 0 payload bytes
    assert!(matches!(
        decode_datagram(&bad_pak_size, true, None, true),
        Err(SessionError::InvalidPakSize {
            pak_size: 4,
            actual_payload_len: 0
        })
    ));

    // Rule 2: bit-tree decompression failure (a stream that ends inside a codeword).
    let tables = BitTreeTable::synthetic_dictionary([0x22; 16]);
    let mut bad_compressed = [0u8; 22];
    bad_compressed[0] = 0x01;
    bad_compressed[2] = 0x01; // isCompressed, unusedTrailingBits 0
    bad_compressed[21] = 0x80; // a final `1` bit starts a 9-bit leaf that never ends
    assert!(matches!(
        decode_datagram(&bad_compressed, true, Some(&tables), false),
        Err(SessionError::WireDecompressionFailed)
    ));

    // Rule 3: Disallowed DatagramHeader.flags bits outside 0x0FFF7B00
    let mut bad_flags = [0u8; 22];
    bad_flags[6..10].copy_from_slice(&0x0000_0001u32.to_be_bytes());
    assert!(matches!(
        decode_datagram(&bad_flags, true, None, false),
        Err(SessionError::InvalidHeaderFlags { flags: 1 })
    ));

    // Rule 3: undefined bit 0x00000400
    let forbidden_hdr = DatagramHeader {
        rec_id: 0x0100,
        flags: FLAG_UNDEFINED_10,
        ..DatagramHeader::default()
    };
    let mut forbidden_dgram = vec![0x00, 0x00];
    let mut fh = forbidden_hdr;
    fh.checksum = compute_packet_checksum(fh, &[], None).unwrap();
    forbidden_dgram.extend_from_slice(&fh.to_wire_bytes());
    assert!(matches!(
        decode_datagram(&forbidden_dgram, true, None, true),
        Err(SessionError::InvalidHeaderFlags { .. })
    ));

    // Rule 4, 5, 6: Source address binding, sequence lookahead > 0x7FFF, and checksum mismatch without mutating state
    let mut conn = challenged_connection(peer, 0x0100, 0x0BAD_5EED);
    let valid_empty = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        None,
        None,
    )
    .expect("valid empty");

    // Rule 4: wrong source address
    assert!(matches!(
        conn.receive_datagram(&valid_empty, wrong_peer, 0.1, WireCodecs::NONE),
        Err(SessionError::SourceAddressMismatch { .. })
    ));

    // Rule 5: sequence number > recvSeqExpected + 0x7FFF
    let out_of_window = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            seq_num: 1 + MAX_SEQ_LOOKAHEAD + 1,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        Some(0),
        None,
    )
    .expect("encode out of window");
    assert!(matches!(
        conn.receive_datagram(&out_of_window, peer, 0.1, WireCodecs::NONE),
        Err(SessionError::SequenceOutOfWindow { .. })
    ));
    assert_eq!(conn.recv_seq_expected(), 1);

    // Rule 6: bad checksum on sequenced packet must not advance recv_seq_expected or add gaps
    let mut bad_csum = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            seq_num: 5,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        Some(0xDEAD_BEEF),
        None,
    )
    .expect("encode seq 5");
    bad_csum[14] ^= 0xFF; // corrupt checksum byte
    assert!(matches!(
        conn.receive_datagram(&bad_csum, peer, 0.1, WireCodecs::NONE),
        Err(SessionError::ChecksumMismatch { .. })
    ));
    assert_eq!(conn.recv_seq_expected(), 1);
    assert_eq!(conn.missing_gap_count(), 0);

    // Rule 7: SequenceNak count > 114
    let mut bad_nak_payload = Vec::new();
    bad_nak_payload.extend_from_slice(&((MAX_NAK_ENTRIES as u32) + 1).to_le_bytes());
    assert!(matches!(
        OptionalHeaders::decode(FLAG_SEQUENCE_NAK, &bad_nak_payload, true),
        Err(SessionError::InvalidSequenceNakCount { count: 115 })
    ));

    // Rule 8: blob fragment bounds (numFrags <= fragIndex, non-zero alignPad, fragDataLen >= 0x01D1, > 20 frags)
    let bad_bounds_bytes = [0x02u8, 0x50, 0x01, 0x00, 0x01, 0x00, 0x02, 0x02, 0x00, 0x00]; // fragIndex=2, numFrags=2
    assert!(matches!(
        BlobFragmentHeader::decode(&bad_bounds_bytes),
        Err(SessionError::InvalidFragmentBounds {
            frag_index: 2,
            num_frags: 2
        })
    ));

    // Non-zero alignPad (modify byte 9 of Vector 4 from 0x00 to 0x7F)
    let bad_pad_bytes = [0x02u8, 0x40, 0x07, 0x00, 0x2A, 0x00, 0x81, 0x86, 0x10, 0x7F];
    assert!(matches!(
        BlobFragmentHeader::decode(&bad_pad_bytes),
        Err(SessionError::NonZeroAlignmentPad { pad: 0x7F })
    ));

    // fragDataLen >= 0x01D1 (465)
    let mut bad_len_hdr = BlobFragmentHeader {
        queue_id: 2,
        ext_word: None,
        blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
        seq_group: 0,
        order_num: 1,
        blob_id: 1,
        frag_index: 0,
        num_frags: 1,
        recipient_id: 2,
        blob_header_word: Some(1),
        frag_data_len: 0x01D1,
        attachment: None,
    };
    let mut tmp = Vec::new();
    assert!(matches!(
        bad_len_hdr.encode(&mut tmp),
        Err(SessionError::InvalidFragmentDataLen { len: 0x01D1 })
    ));

    // More than 20 fragments in a packet
    bad_len_hdr.frag_data_len = 0;
    let dummy_frag = BlobFragment {
        header: bad_len_hdr,
        payload: Vec::new(),
    };
    let too_many = vec![dummy_frag; MAX_FRAGS_PER_PACKET + 1];
    assert!(matches!(
        encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders::default(),
            &too_many,
            None,
            None
        ),
        Err(SessionError::TooManyFragments { count: 21 })
    ));

    // Attachment round-trip check & defensive reassembly cap check
    let mut reassembler = BlobReassembler::new(2, MAX_BLOB_FRAG_PAYLOAD * 2);
    let oversized_frag = BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: Some(0x1234),
            blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT | BLOB_FLAG_HAS_ATTACHMENT,
            seq_group: 0,
            order_num: 1,
            blob_id: 9,
            frag_index: 0,
            num_frags: 5, // 5 * 436 > 2 * 436 cap
            recipient_id: 2,
            blob_header_word: Some(1),
            frag_data_len: 10,
            attachment: Some(FragmentAttachment {
                words64: [1, 2, 3],
                word32: 4,
            }),
        },
        payload: vec![0u8; 10],
    };
    assert!(matches!(
        reassembler.ingest(1, oversized_frag, 0.0),
        Err(SessionError::BlobTooLarge { .. })
    ));

    // Verify verify_packet_checksum helper directly
    let hdr_check = DatagramHeader {
        rec_id: 1,
        pak_size: 0,
        flags: FLAG_CLOCK_SYNC
            | FLAG_DISCONNECT
            | FLAG_SERVER_REDIRECT
            | FLAG_AUTH_SERVER_ADDR
            | FLAG_WORLD_REFERRAL
            | FLAG_SEQUENCE_NAK
            | FLAG_OUT_OF_ORDER_ACK
            | FLAG_CONNECT_CHALLENGE
            | FLAG_TRANSPORT_ERROR
            | FLAG_FLOW_CONTROL
            | FLAG_HAS_FRAGMENTED_BLOBS
            | FLAG_MASKED_CHECKSUM,
        seq_num: 2,
        checksum: 0,
        iteration: 0,
        table: 0,
        time: 0,
    };
    assert!(verify_packet_checksum(hdr_check, &[], Some(0)).is_err());

    // Verify zlib bomb protection on decode_message
    let mut fake_bomb = vec![ENVELOPE_FLAG_ZLIB_COMPRESSED];
    fake_bomb.extend_from_slice(&(10_000_000u32).to_le_bytes());
    assert!(matches!(
        decode_message(1, &fake_bomb, 1024),
        Err(SessionError::DecompressedMessageTooLarge {
            size: 10_000_000,
            limit: 1024
        })
    ));
}

// ---------------------------------------------------------------------------
// §4.1.2–§4.1.3 .dat BitTreeTable & 4-Bit wireBody[0] Header
// ---------------------------------------------------------------------------

#[test]
fn dat_bit_tree_table_and_4bit_wire_body_header() {
    let codec_uuid = [
        0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0,
        0x01,
    ];
    let dict = BitTreeTable::synthetic_dictionary(codec_uuid);
    let dat_bytes = dict.to_dat_bytes();

    // Verify 44-byte header fields
    assert_eq!(&dat_bytes[0..16], &BIT_TREE_TABLE_FORMAT_UUID);
    assert_eq!(&dat_bytes[16..32], &codec_uuid);
    assert_eq!(
        u32::from_le_bytes([dat_bytes[36], dat_bytes[37], dat_bytes[38], dat_bytes[39]]),
        1
    );

    let parsed = BitTreeTable::from_dat_bytes(&dat_bytes).expect("parse .dat dictionary");
    assert_eq!(parsed, dict);

    // §4.1.3 item 2: the parser requires version == 1.
    let mut dat_v10001 = dat_bytes.clone();
    dat_v10001[36..40].copy_from_slice(&0x0001_0001u32.to_le_bytes());
    assert_eq!(
        BitTreeTable::from_dat_bytes(&dat_v10001),
        Err(SessionError::InvalidBitTreeTable)
    );

    // Redirect rule (§4.1.3 item 1): follow redirectDid only when non-zero and not the
    // record's own DID. The DIDs are the spec's verification values (resolved at run time).
    const DID_DICT_CLIENT_TO_SERVER: u32 = 0x0E01_0000;
    const DID_DICT_SERVER_TO_CLIENT: u32 = 0x0E01_0001;
    const DID_DICT_SERVER_TO_CLIENT_DEFERRED: u32 = 0x0E01_0002;
    let mut stub = vec![0u8; 44];
    stub[0..16].copy_from_slice(&BIT_TREE_TABLE_FORMAT_UUID);
    stub[32..36].copy_from_slice(&DID_DICT_SERVER_TO_CLIENT.to_le_bytes());
    assert_eq!(
        BitTreeTable::parse_redirect_did(&stub, DID_DICT_SERVER_TO_CLIENT_DEFERRED)
            .expect("parse redirect stub"),
        Some(DID_DICT_SERVER_TO_CLIENT)
    );
    assert_eq!(
        BitTreeTable::parse_redirect_did(&stub, DID_DICT_SERVER_TO_CLIENT).unwrap(),
        None
    );
    let mut literal = stub.clone();
    literal[32..36].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        BitTreeTable::parse_redirect_did(&literal, DID_DICT_CLIENT_TO_SERVER).unwrap(),
        None
    );

    // Case 1: Zero-rich payload compresses via 4-byte [0,0,0,0] leaf -> isCompressed == 1 (wireBody[0] & 1 == 1)
    let compressible_input = vec![0u8; 64];
    let mut wire_body = Vec::new();
    let written = parsed
        .encode(&compressible_input, &mut wire_body, 1024)
        .expect("compress zero-rich");
    assert_eq!(written, wire_body.len());
    assert_eq!(wire_body[0] & 1, 1, "expected isCompressed == 1");
    assert!(wire_body.len() < compressible_input.len());

    let mut decompressed = Vec::new();
    parsed
        .decode(&wire_body, &mut decompressed, 1024)
        .expect("decompress zero-rich");
    assert_eq!(decompressed, compressible_input);

    // Case 2: Non-zero payload where 9-bit leaves exceed inputLen + 1 -> literal fallback (wireBody[0] == 0x00)
    let incompressible_input: Vec<u8> = (1..=32u8).collect();
    let mut fallback_body = Vec::new();
    parsed
        .encode(&incompressible_input, &mut fallback_body, 1024)
        .expect("encode literal fallback");
    assert_eq!(
        fallback_body[0], 0x00,
        "expected literal fallback wireBody[0] == 0x00"
    );
    assert_eq!(&fallback_body[1..], &incompressible_input[..]);

    let mut fallback_dec = Vec::new();
    parsed
        .decode(&fallback_body, &mut fallback_dec, 1024)
        .expect("decode literal fallback");
    assert_eq!(fallback_dec, incompressible_input);

    // Full UDP datagram round-trip with wirePrefix != 0x0000 using BitTreeTable
    let hdr = DatagramHeader {
        rec_id: 0x0142,
        seq_num: 12,
        ..DatagramHeader::default()
    };
    let frag = BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: None,
            blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
            seq_group: 0,
            order_num: 1,
            blob_id: 0x0010,
            frag_index: 0,
            num_frags: 1,
            recipient_id: 2,
            blob_header_word: Some(0x0000_012C),
            frag_data_len: 32,
            attachment: None,
        },
        payload: vec![0xAA; 32],
    };
    let dgram = encode_datagram(
        hdr,
        &OptionalHeaders::default(),
        &[frag],
        Some(0xABCD_EF01),
        Some(&parsed),
    )
    .expect("encode dgram");
    assert_eq!(u16::from_be_bytes([dgram[0], dgram[1]]), 0x0142);
    let dec_pkt =
        decode_datagram(&dgram, true, Some(&parsed), false).expect("decode compressed dgram");
    assert_eq!(dec_pkt.header.rec_id, 0x0142);
    assert_eq!(dec_pkt.fragments.len(), 1);
    assert_eq!(dec_pkt.fragments[0].payload, vec![0xAA; 32]);
    verify_packet_checksum(dec_pkt.header, &dec_pkt.raw_payload, Some(0xABCD_EF01))
        .expect("verify checksum");
}

// ---------------------------------------------------------------------------
// §4.3 & §10.1 Reconciled DatagramHeader.flags Mask (0x0F7FFB00 / 0x0F7FFB06)
// ---------------------------------------------------------------------------

#[test]
fn reconciled_datagram_header_flags_mask_0x0f7ffb00() {
    assert_eq!(VALID_DATAGRAM_FLAGS_MASK, 0x0F7F_FB00);
    assert_eq!(INVALID_DATAGRAM_FLAGS_MASK, 0xF080_04FF);
    // §4.3 / §10.1 rule 3: the receive mask is the complement of 0x0F7FFB06.
    assert_eq!(RECEIVE_FORBIDDEN_FLAGS_MASK, 0xF080_04F9);
    assert_eq!(RECEIVE_FORBIDDEN_FLAGS_MASK, !0x0F7F_FB06);

    // 0x00008000 (Disconnect) IS valid on the wire
    let disc_dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("encode Disconnect packet");
    let dec_disc = decode_datagram(&disc_dgram, true, None, true).expect("decode Disconnect");
    assert!(dec_disc.opt_headers.disconnect);

    // 0x00000400 (bit 10), 0x00800000 (bit 23), bits 28..31, bit 0 and bits 3..7 are
    // rejected on receive (0xF08004F9)
    for bad_bit in [
        FLAG_UNDEFINED_10,
        FLAG_UNDEFINED_23,
        0x1000_0000,
        0x8000_0000,
        0x0000_0001,
        0x0000_0008,
        0x0000_0080,
    ] {
        let mut raw = [0u8; 20];
        raw[4..8].copy_from_slice(&bad_bit.to_be_bytes());
        assert!(matches!(
            DatagramHeader::from_wire_bytes(&raw),
            Err(SessionError::InvalidHeaderFlags { flags }) if flags == bad_bit
        ));
    }
    // ... while the low-byte bits 0x02 and 0x04 are ignored on receive: a packet that
    // carries them is accepted (its checksum covers the bits as sent).
    for ignored in [0x0000_0002u32, 0x0000_0004, 0x0000_0006] {
        let mut raw = [0u8; 20];
        raw[4..8].copy_from_slice(&ignored.to_be_bytes());
        assert_eq!(
            DatagramHeader::from_wire_bytes(&raw).unwrap().flags,
            ignored
        );
        let mut hdr = DatagramHeader {
            rec_id: 0x0100,
            flags: ignored | FLAG_DISCONNECT,
            ..DatagramHeader::default()
        };
        hdr.checksum = compute_packet_checksum(hdr, &[], None).unwrap();
        let mut dgram = vec![0u8, 0];
        dgram.extend_from_slice(&hdr.to_wire_bytes());
        let pkt = decode_datagram(&dgram, true, None, true).expect("accepted");
        assert!(pkt.opt_headers.disconnect);
    }
    // The sender never sets them: they are outside the send mask.
    assert!(
        encode_datagram(
            DatagramHeader {
                flags: 0x0000_0002,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &[],
            None,
            None
        )
        .is_err()
    );

    // 0x00400000 (ClientControl, 4 B u32 LE) is valid Client -> Server (from_server == false),
    // but rejected when received from a server (from_server == true, Rule 7)
    let cc_opt = OptionalHeaders {
        client_control: Some(0xCAFE_1234),
        ..OptionalHeaders::default()
    };
    let cc_dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            ..DatagramHeader::default()
        },
        &cc_opt,
        &[],
        None,
        None,
    )
    .expect("encode ClientControl client->server");
    let dec_client =
        decode_datagram(&cc_dgram, false, None, true).expect("decode client ClientControl");
    assert_ne!(dec_client.header.flags & FLAG_CLIENT_CONTROL, 0);
    assert_eq!(dec_client.opt_headers.client_control, Some(0xCAFE_1234));
    assert!(matches!(
        decode_datagram(&cc_dgram, true, None, true),
        Err(SessionError::InvalidHeaderFlags { .. })
    ));
}

// ---------------------------------------------------------------------------
// §9.1–§9.4 MessageEnvelope Routing Header, Post-Handshake Control Messages, and World Event Batches
// ---------------------------------------------------------------------------

#[test]
fn message_routing_header_and_post_handshake_control_messages() {
    // 1. §9.1 envelopeFlags bit 1 (0x02 HasRoutingHeader) with u16 LE routingFlags + 2-byte alignment pad
    let routing = MessageRoutingHeader {
        field0: Some(0x1122_3344),
        field1: None,
        field2: Some(0x0102_0304_0506_0708),
        field3: Some(0x99AA_BBCC),
    };
    let inner_body = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let env_uncompressed = encode_message_with_routing(
        ENVELOPE_FLAG_HAS_ROUTING_HEADER,
        Some(&routing),
        &inner_body,
        false,
    )
    .expect("encode with routing uncompressed");
    // routing header size: 2 (flags) + 4 (f0) + 8 (f2) + 4 (f3) = 18 -> aligned to 20 bytes (+ 1 byte envelopeFlags + 4 bytes body = 25 bytes)
    assert_eq!(env_uncompressed.len(), 1 + 20 + 4);
    let dec_uncomp = decode_message(
        0x0000_1000,
        &env_uncompressed,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
    )
    .expect("decode with routing uncompressed");
    assert_eq!(dec_uncomp.routing_header, Some(routing));
    assert_eq!(dec_uncomp.body, inner_body);

    // Zlib-compressed + routing header (envelopeFlags = 0x03)
    let env_compressed = encode_message_with_routing(
        ENVELOPE_FLAG_HAS_ROUTING_HEADER,
        Some(&routing),
        &inner_body,
        true,
    )
    .expect("encode with routing compressed");
    assert_eq!(
        env_compressed[0],
        ENVELOPE_FLAG_ZLIB_COMPRESSED | ENVELOPE_FLAG_HAS_ROUTING_HEADER
    );
    let dec_comp = decode_message(
        0x0000_1000,
        &env_compressed,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
    )
    .expect("decode with routing compressed");
    assert_eq!(dec_comp.routing_header, Some(routing));
    assert_eq!(dec_comp.body, inner_body);

    // Reject reserved envelopeFlags bits (envelopeFlags & 0xFC != 0)
    assert!(matches!(
        decode_message(1, &[0x04, 0x00], DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES),
        Err(SessionError::InvalidEnvelopeFlags { flags: 0x04 })
    ));

    // 2. §9.3 Post-handshake session/version negotiation messages (0x0000012C..0x00000130)
    let controls = [
        ControlMessage::VersionRequest,
        ControlMessage::ClientVersion(ClientVersionMessage {
            version_string: "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10".to_owned(),
        }),
        ControlMessage::VersionStatus(VersionStatusMessage {
            status: 1,
            server_version_info: "1.2.3.4".to_owned(),
        }),
        ControlMessage::LoginQueueStatus(LoginQueueStatusMessage {
            login_result: LOGON_RESULT_QUEUED,
            detail_text: "Queueing".to_owned(),
            queue: Some(LoginQueuePosition {
                position: 3,
                total: 25,
            }),
        }),
        ControlMessage::SystemNotification(SystemNotificationMessage {
            text: "Server maintenance in 30 minutes".to_owned(),
        }),
    ];
    let expected_opcodes = [
        OPCODE_VERSION_REQUEST,
        OPCODE_CLIENT_VERSION,
        OPCODE_VERSION_STATUS,
        OPCODE_LOGIN_QUEUE_STATUS,
        OPCODE_SYSTEM_NOTIFICATION,
    ];
    for (msg, &expected_op) in controls.iter().zip(expected_opcodes.iter()) {
        if let ControlMessage::SystemNotification(_) = msg {
            // Receive-only (decision 0042 item 2): never encoded. Build the
            // server's body by hand (VariableU32 code-unit count + UTF-16LE).
            assert_eq!(
                msg.encode(),
                Err(SessionError::ReceiveOnlyMessage {
                    opcode: OPCODE_SYSTEM_NOTIFICATION
                })
            );
            let text = "Server maintenance in 30 minutes";
            let mut bytes = Vec::new();
            encode_variable_u32(
                u32::try_from(text.encode_utf16().count()).unwrap(),
                &mut bytes,
            );
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            let decoded = ControlMessage::decode(expected_op, &bytes).expect("decode notice");
            assert_eq!(&decoded, msg);
            continue;
        }
        let (op, bytes) = msg.encode().expect("sendable control message");
        assert_eq!(op, expected_op);
        let decoded = ControlMessage::decode(op, &bytes).expect("decode control msg");
        assert_eq!(&decoded, msg);
    }

    // VersionStatusMessage status == 0 -> VersionRejected
    let rejected = VersionStatusMessage {
        status: 0,
        server_version_info: "mismatch".to_owned(),
    };
    assert!(matches!(
        rejected.ensure_accepted(),
        Err(SessionError::VersionRejected { .. })
    ));

    // ClientVersionMessage strings are ASCII.
    let mut non_ascii = Vec::new();
    encode_variable_u32(2, &mut non_ascii);
    non_ascii.extend_from_slice("é".as_bytes());
    assert_eq!(
        ClientVersionMessage::decode(&non_ascii),
        Err(SessionError::InvalidControlMessage {
            opcode: OPCODE_CLIENT_VERSION
        })
    );
}

// ---------------------------------------------------------------------------
// Hostile-input hardening regressions (§5.6.4, §7.1, §10.1 rules 6 and 8)
// ---------------------------------------------------------------------------

fn multi_frag(frag_index: u32, num_frags: u32, len: usize) -> BlobFragment {
    BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: None,
            blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
            seq_group: 0,
            order_num: 1,
            blob_id: 7,
            frag_index,
            num_frags,
            recipient_id: 2,
            blob_header_word: if frag_index == 0 { Some(0x55) } else { None },
            frag_data_len: len as u32,
            attachment: None,
        },
        payload: vec![0xA5; len],
    }
}

#[test]
fn reassembly_rejects_final_fragment_past_buffer_end() {
    let mut reassembler = BlobReassembler::default();
    // numFrags = 2, fragIndex = 1, fragDataLen = 464: 436 + 464 > 2 * 436.
    let err = reassembler
        .ingest(1, multi_frag(1, 2, 464), 0.0)
        .expect_err("final fragment overruns the reassembly buffer");
    assert_eq!(
        err,
        SessionError::FragmentOverrunsReassembly {
            frag_index: 1,
            len: 464,
            buffer_len: 2 * MAX_BLOB_FRAG_PAYLOAD,
        }
    );
    assert_eq!(reassembler.incomplete_len(), 0);

    // The largest final slice that fits (436 bytes) still completes the blob.
    assert!(
        reassembler
            .ingest(1, multi_frag(0, 2, 436), 0.0)
            .expect("first fragment")
            .is_none()
    );
    let blob = reassembler
        .ingest(1, multi_frag(1, 2, 436), 0.0)
        .expect("final fragment")
        .expect("completed blob");
    assert_eq!(blob.payload.len(), 2 * MAX_BLOB_FRAG_PAYLOAD);
    assert_eq!(blob.blob_header_word, 0x55);
}

#[test]
fn duplicate_sequenced_packet_is_dropped_before_headers_act() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 20), 9000);
    let mut conn = challenged_connection(peer, 0x0100, 0x0000_0042);
    let mask_seq_2 = IsaacCipher::from_seed(0x0000_0042).next_mask();
    let seq_hdr = DatagramHeader {
        rec_id: 0x0100,
        seq_num: 2,
        ..DatagramHeader::default()
    };
    let first = encode_datagram(
        seq_hdr,
        &OptionalHeaders::default(),
        &[],
        Some(mask_seq_2),
        None,
    )
    .expect("encode seq 2");
    conn.receive_datagram(&first, peer, 0.1, WireCodecs::NONE)
        .expect("accept seq 2");
    assert_eq!(conn.recv_seq_expected(), 2);
    while conn.pop_event().is_some() {}
    let state_before = conn.state();

    // A replay of seq 2 (and an older seq 1) carrying Disconnect and TransportError must not act.
    let hostile_opt = OptionalHeaders {
        disconnect: true,
        transport_error: Some(TransportErrorHeader {
            error_code: 0x1234_5678,
            error_subcode: 0,
        }),
        ..OptionalHeaders::default()
    };
    for seq_num in [2u32, 1] {
        let replay = encode_datagram(
            DatagramHeader { seq_num, ..seq_hdr },
            &hostile_opt,
            &[],
            Some(mask_seq_2),
            None,
        )
        .expect("encode replay");
        assert_eq!(
            conn.receive_datagram(&replay, peer, 50.0, WireCodecs::NONE)
                .expect_err("duplicate sequenced packet is dropped"),
            SessionError::DuplicateSequence {
                seq_num,
                expected: 2,
            }
        );
    }
    assert_eq!(conn.state(), state_before);
    assert_eq!(conn.recv_seq_expected(), 2);
    assert!(conn.pop_event().is_none());
}

#[test]
fn des3_cbc_rejects_all_padding_block() {
    let key: [u8; 24] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ];
    // Encrypting an empty plaintext yields one block of eight 0x08 bytes: padLen == len,
    // which §7.1 (`padLen < len`) forbids on decryption.
    let ct = des3_cbc_encrypt(&key, &[]);
    assert_eq!(ct.len(), 8);
    assert_eq!(
        des3_cbc_decrypt(&key, &ct),
        Err(SessionError::InvalidPkcs7Padding)
    );
    // One byte of plaintext (padLen = 7 < 8) still decrypts.
    let ct1 = des3_cbc_encrypt(&key, &[0x42]);
    assert_eq!(des3_cbc_decrypt(&key, &ct1).expect("decrypt"), vec![0x42]);
}

// ---------------------------------------------------------------------------
// Conformance regressions (§4.1, §5.6.4, §6.7, §7.1, §3.4)
// ---------------------------------------------------------------------------

#[test]
fn encrypted_fragmented_blob_is_always_rejected() {
    let mut frag = multi_frag(0, 1, 8);
    frag.header.blob_flags |= BLOB_FLAG_ENCRYPTED;
    let mut reassembler = BlobReassembler::default();
    assert_eq!(
        reassembler.ingest(1, frag, 0.0),
        Err(SessionError::EncryptedBlobRejected)
    );
    // split_blob never sets the bit.
    for f in split_blob(2, 0, 1, 0, None, 0x10, &[0u8; 1000]).unwrap() {
        assert_eq!(f.header.blob_flags & BLOB_FLAG_ENCRYPTED, 0);
    }
}

#[test]
fn bit_tree_null_child_is_rejected_at_traversal() {
    let mut dict = BitTreeTable::synthetic_dictionary([0x33; 16]);
    // Make the root's `0` branch null; the table still loads (§4.1.3 item 3).
    let root = dict.root_node_index;
    dict.nodes[root] = BitTreeNode::Branch {
        child0_1based: 0,
        child1_1based: 259,
    };
    let reloaded = BitTreeTable::from_dat_bytes(&dict.to_dat_bytes()).expect("null child loads");
    let mut out = Vec::new();
    // Compressed header byte with bit 4 (the first code bit) = 0 selects the null child.
    assert_eq!(
        reloaded.decode(&[0x01, 0x00], &mut out, 64),
        Err(SessionError::WireDecompressionFailed)
    );
}

#[test]
fn control_packets_stay_uncompressed_and_errors_close_with_reason() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 8), 9000);
    let codec = BitTreeTable::synthetic_dictionary([0x44; 16]);
    let mut conn = SessionConnection::new(server, 0.0);
    conn.register_codec_uuid([0x44; 16]);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    let mut challenge = vector_5_challenge();
    challenge.codec_uuid = [0x44; 16];
    let dgram = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    conn.receive_datagram(
        &dgram,
        server,
        0.1,
        WireCodecs {
            incoming: Some(&codec),
            outgoing: Some(&codec),
        },
    )
    .unwrap();
    while conn.pop_outbound_datagram().is_some() {}
    // A sequenced data packet goes out compressed (wirePrefix = remoteRecId) ...
    conn.send_blob(2, 0, 0x10, &[0x00; 64], 0.2, Some(&codec))
        .unwrap();
    let data = conn.pop_outbound_datagram().unwrap();
    assert_eq!(u16::from_be_bytes([data[0], data[1]]), 0x0304);
    // ... while the State 2 ConnectAcceptHeader retransmit stays unencoded.
    conn.tick(1.0).unwrap();
    let retransmit = conn.pop_outbound_datagram().unwrap();
    assert_eq!(&retransmit[0..2], &[0x00, 0x00]);

    // TransportErrorHeader: numbers kept, connection closed (OPEN-SESSION-4).
    let err = encode_datagram(
        DatagramHeader {
            rec_id: 0x0102,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            transport_error: Some(TransportErrorHeader {
                error_code: 0x00A7_E948,
                error_subcode: 3,
            }),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap();
    conn.receive_datagram(&err, server, 1.1, WireCodecs::NONE)
        .unwrap();
    let mut reason = None;
    while let Some(ev) = conn.pop_event() {
        if let SessionEvent::Disconnected { reason: r } = ev {
            reason = Some(r);
        }
    }
    assert_eq!(
        reason,
        Some(DisconnectReason::TransportError {
            fatal: false,
            error_code: 0x00A7_E948,
            error_subcode: 3,
        })
    );
    assert_eq!(conn.state(), ConnectionState::Closing);
}

#[test]
fn handshake_timeout_closes_with_reason() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 9), 9000);
    let mut conn = SessionConnection::new(server, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    conn.tick(20.0).unwrap();
    assert_eq!(
        conn.pop_event(),
        Some(SessionEvent::Disconnected {
            reason: DisconnectReason::HandshakeTimeout
        })
    );
}

#[test]
fn multiplexer_falls_back_to_source_address_on_id_miss() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 10), 9000);
    let mut mux = SessionMultiplexer::new();
    let id = mux.create_connection(server, 0.0).unwrap();
    mux.connection_mut(id)
        .unwrap()
        .start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    // recId 0x0777 names no connection: the source address decides (§3.4 step 3).
    let dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0777,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        None,
        None,
    )
    .unwrap();
    let (routed, _) = mux
        .receive_from_peer(&dgram, server, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(routed, id);
    let stranger = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 11), 9000);
    assert!(matches!(
        mux.receive_from_peer(&dgram, stranger, 0.1, WireCodecs::NONE),
        Err(SessionError::UnknownRecipient { .. })
    ));
}

// ---------------------------------------------------------------------------
// Fix round 1: send checksum, codecs, ISAAC seeding, challenge states, pre-validation
// ---------------------------------------------------------------------------

/// The Vector 4 fragment: single fragment, queue 2, default recipient, 16 payload bytes.
fn vector_4_fragment() -> BlobFragment {
    BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: None,
            blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
            seq_group: 0,
            order_num: 0x0007,
            blob_id: 0x002A,
            frag_index: 0,
            num_frags: 1,
            recipient_id: 2,
            blob_header_word: Some(0x0000_0186),
            frag_data_len: 16,
            attachment: None,
        },
        payload: (0..16u8).collect(),
    }
}

/// §4.4.2 step 2: the sender sums `compute_datagram_checksum` per optional header buffer and
/// per fragment; with the unaligned 6-byte `FlowControlHeader` in front of the Vector 4
/// fragment this differs from the receiver's whole-payload sum.
#[test]
fn send_checksum_is_summed_per_buffer() {
    let flow = FlowControlHeader {
        bytes_received_delta: 0x0102_0304,
        interval_ms: 0x0506,
    };
    let opt = OptionalHeaders {
        flow_control: Some(flow),
        ..OptionalHeaders::default()
    };
    let frag = vector_4_fragment();
    let mut flow_bytes = Vec::new();
    opt.encode(&mut flow_bytes).unwrap();
    assert_eq!(flow_bytes, [0x04, 0x03, 0x02, 0x01, 0x06, 0x05]);
    let mut frag_bytes = Vec::new();
    frag.encode(&mut frag_bytes).unwrap();
    assert_eq!(
        &frag_bytes[..10],
        &[0x02, 0x40, 0x07, 0x00, 0x2A, 0x00, 0x81, 0x86, 0x10, 0x00]
    );
    // §4.4.2 step 2, send rule: one sum per optional header buffer, then the fragment's
    // header (10 bytes, 2 mod 4) and its 16-byte payload slice as separate buffers.
    let (frag_header, frag_payload) = frag_bytes.split_at(10);
    let per_buffer = compute_datagram_checksum(&flow_bytes)
        .wrapping_add(compute_datagram_checksum(frag_header))
        .wrapping_add(compute_datagram_checksum(frag_payload));
    assert_eq!(
        send_payload_checksum(&opt, std::slice::from_ref(&frag)).unwrap(),
        per_buffer
    );
    let whole = [flow_bytes.clone(), frag_bytes.clone()].concat();
    assert_ne!(per_buffer, compute_datagram_checksum(&whole));
    // Without the flow header the fragment alone is still two buffers, and its 10-byte
    // header makes the two rules differ.
    let frag_only = compute_datagram_checksum(frag_header)
        .wrapping_add(compute_datagram_checksum(frag_payload));
    assert_eq!(
        send_payload_checksum(&OptionalHeaders::default(), std::slice::from_ref(&frag)).unwrap(),
        frag_only
    );
    assert_ne!(frag_only, compute_datagram_checksum(&frag_bytes));

    let hdr = DatagramHeader {
        rec_id: 0x0102,
        seq_num: 1,
        ..DatagramHeader::default()
    };
    let dgram = encode_datagram_with_rule(
        hdr,
        &opt,
        std::slice::from_ref(&frag),
        None,
        None,
        SendChecksumRule::PerBuffer,
    )
    .unwrap();
    let parsed = decode_datagram(&dgram, true, None, false).unwrap();
    assert_eq!(
        parsed.header.checksum,
        combine_packet_checksum(parsed.header, per_buffer, None).unwrap()
    );
    // The receive rule sums the whole payload, so this packet does not verify by it.
    assert!(matches!(
        decode_datagram(&dgram, true, None, true),
        Err(SessionError::ChecksumMismatch { .. })
    ));
    // `encode_datagram` (the synthetic peers' encoder) uses the receive rule.
    let peer = encode_datagram(hdr, &opt, std::slice::from_ref(&frag), None, None).unwrap();
    decode_datagram(&peer, true, None, true).expect("the receive rule verifies");
    // With one buffer the rules agree (Vector 2 is such a packet).
    let ack = OptionalHeaders {
        cumulative_ack: Some(10),
        ..OptionalHeaders::default()
    };
    assert_eq!(
        encode_datagram_with_rule(hdr, &ack, &[], None, None, SendChecksumRule::PerBuffer).unwrap(),
        encode_datagram(hdr, &ack, &[], None, None).unwrap()
    );
}

/// A masked checksum needs its ISAAC word (§4.4.2 step 3).
#[test]
fn masked_checksum_requires_a_mask() {
    let hdr = DatagramHeader {
        flags: FLAG_MASKED_CHECKSUM,
        seq_num: 2,
        ..DatagramHeader::default()
    };
    assert_eq!(
        compute_packet_checksum(hdr, &[], None),
        Err(SessionError::MissingChecksumMask)
    );
    assert!(compute_packet_checksum(hdr, &[], Some(7)).is_ok());
}

/// A codec that counts its encodes, so a test can tell which dictionary compressed a send.
#[derive(Debug, Default)]
struct CountingCodec {
    inner: Option<BitTreeTable>,
    encodes: std::sync::atomic::AtomicUsize,
}

impl CountingCodec {
    fn new() -> Self {
        Self {
            inner: Some(BitTreeTable::synthetic_dictionary([0u8; 16])),
            encodes: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn encodes(&self) -> usize {
        self.encodes.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl BitTreeCodec for CountingCodec {
    fn decode(&self, wire: &[u8], out: &mut Vec<u8>, max: usize) -> Result<usize, SessionError> {
        self.inner.as_ref().unwrap().decode(wire, out, max)
    }

    fn encode(&self, plain: &[u8], out: &mut Vec<u8>, max: usize) -> Result<usize, SessionError> {
        self.encodes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.as_ref().unwrap().encode(plain, out, max)
    }
}

/// §4.1.2: a retransmission after a `SequenceNakHeader` is compressed with the outgoing
/// dictionary, never the incoming one that decompressed the NAK.
#[test]
fn nak_resend_uses_the_outgoing_codec() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 30), 9000);
    let incoming = CountingCodec::new();
    let outgoing = CountingCodec::new();
    let codecs = WireCodecs {
        incoming: Some(&incoming),
        outgoing: Some(&outgoing),
    };
    let mut conn = challenged_connection_with_codec(peer, 0x0100, 0x0000_0099, [0x55; 16]);
    let seq = conn
        .send_blob(2, 0, 0x10, &[0x00; 32], 0.1, codecs.outgoing)
        .unwrap();
    assert_eq!(seq, [2]);
    let first = conn.pop_outbound_datagram().unwrap();
    assert_eq!(outgoing.encodes(), 1);
    let nak = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            sequence_nak: Some(vec![2]),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        Some(&incoming),
    )
    .unwrap();
    let encodes_before = incoming.encodes();
    conn.receive_datagram(&nak, peer, 0.2, codecs).unwrap();
    let mut resent = None;
    while let Some(d) = conn.pop_outbound_datagram() {
        resent = Some(d);
    }
    // The same seqNum and fragments; the time and so the header checksum are updated (§5.3).
    let resent = resent.expect("a retransmission");
    let first = decode_datagram(&first, false, Some(&outgoing), false).unwrap();
    let again = decode_datagram(&resent, false, Some(&outgoing), false).unwrap();
    assert_eq!(again.header.seq_num, 2);
    assert_eq!(again.fragments, first.fragments);
    assert_eq!(outgoing.encodes(), 2);
    assert_eq!(incoming.encodes(), encodes_before);
}

/// §7.2: before a `ConnectChallengeHeader` seeds ISAAC, masked packets are refused on
/// receive and sequenced sends are refused, with nothing changed.
#[test]
fn isaac_must_be_seeded_before_sequenced_traffic() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 31), 9000);
    let mut conn = SessionConnection::new(peer, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    while conn.pop_outbound_datagram().is_some() {}
    let masked = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            seq_num: 2,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        Some(IsaacCipher::from_seed(0).next_mask()),
        None,
    )
    .unwrap();
    assert_eq!(
        conn.receive_datagram(&masked, peer, 0.1, WireCodecs::NONE),
        Err(SessionError::InvalidState {
            state: ConnectionState::SessionAuthSent
        })
    );
    assert_eq!(conn.recv_seq_expected(), 1);
    assert_eq!(
        conn.send_sequenced_packet(OptionalHeaders::default(), Vec::new(), true, 0.1, None),
        Err(SessionError::InvalidState {
            state: ConnectionState::SessionAuthSent
        })
    );
    assert_eq!(
        conn.send_blob(2, 0, 0x10, &[1, 2, 3], 0.1, None),
        Err(SessionError::InvalidState {
            state: ConnectionState::SessionAuthSent
        })
    );
    assert_eq!(conn.send_seq_num(), 1);
    assert!(conn.pop_outbound_datagram().is_none());
}

fn challenge_datagram(assigned: u16) -> Vec<u8> {
    let mut challenge = vector_5_challenge();
    challenge.assigned_client_rec_id = assigned;
    encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            connect_challenge: Some(challenge),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .unwrap()
}

/// §6.4: a `ConnectChallengeHeader` is acted on only in States 1 and 2.
#[test]
fn challenge_outside_states_1_and_2_is_dropped() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 32), 9000);
    // State 0: never started.
    let mut idle = SessionConnection::new(peer, 0.0);
    assert_eq!(
        idle.receive_datagram(&challenge_datagram(0x0102), peer, 0.1, WireCodecs::NONE),
        Err(SessionError::UnexpectedConnectChallenge {
            state: ConnectionState::Idle
        })
    );
    assert_eq!(idle.local_rec_id(), 0);
    assert_eq!(idle.server_rec_id(), 0);
    assert!(idle.pop_outbound_datagram().is_none());

    // State 2 acts on it (and moves to State 5, §6.1); State 5 drops a later one.
    let mut conn = challenged_connection(peer, 0x0150, 1);
    conn.receive_datagram(&challenge_datagram(0), peer, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.state(), ConnectionState::Connected);
    while conn.pop_outbound_datagram().is_some() {}
    assert_eq!(
        conn.receive_datagram(&challenge_datagram(0x0102), peer, 0.2, WireCodecs::NONE),
        Err(SessionError::UnexpectedConnectChallenge {
            state: ConnectionState::Connected
        })
    );
    assert_eq!(conn.local_rec_id(), 0x0150);
    assert!(conn.pop_outbound_datagram().is_none());
}

/// §10.1: every fragment is checked before the sequence state, ISAAC, the receive time or
/// an optional header changes; one bad fragment drops the whole packet, so no fragment of
/// it is lost after a commit.
#[test]
fn a_bad_fragment_rejects_the_packet_before_any_commit() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 33), 9000);
    let seed = 0x0000_7777;
    let mut conn = challenged_connection(peer, 0x0100, seed);
    let mut server = IsaacCipher::from_seed(seed);
    let mask_2 = server.next_mask();
    let good = vector_4_fragment();
    let mut encrypted = vector_4_fragment();
    encrypted.header.blob_flags |= BLOB_FLAG_ENCRYPTED;
    let hdr = DatagramHeader {
        rec_id: 0x0100,
        seq_num: 2,
        ..DatagramHeader::default()
    };
    let hostile = encode_datagram(
        hdr,
        &OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        },
        &[good.clone(), encrypted],
        Some(mask_2),
        None,
    )
    .unwrap();
    // The send rule's checksum is per buffer; rebuild it by the receive rule so only the
    // fragment is wrong.
    let mut hostile = hostile;
    let mut parsed = decode_datagram(&hostile, true, None, false).unwrap();
    parsed.header.checksum =
        compute_packet_checksum(parsed.header, &parsed.raw_payload, Some(mask_2)).unwrap();
    hostile[2..22].copy_from_slice(&parsed.header.to_wire_bytes());
    assert_eq!(
        conn.receive_datagram(&hostile, peer, 0.1, WireCodecs::NONE),
        Err(SessionError::EncryptedBlobRejected)
    );
    assert_eq!(conn.recv_seq_expected(), 1);
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    assert!(conn.pop_event().is_none());

    // The generator did not move: the next packet with seq 2 and its mask is accepted.
    let fine = encode_datagram(
        hdr,
        &OptionalHeaders::default(),
        std::slice::from_ref(&good),
        Some(mask_2),
        None,
    )
    .unwrap();
    conn.receive_datagram(&fine, peer, 0.2, WireCodecs::NONE)
        .expect("seq 2 with the unchanged ISAAC word");
    assert_eq!(conn.recv_seq_expected(), 2);
}

/// §10.1 rule 8: a packet that would open more incomplete blobs than the table holds is
/// refused whole, before its optional headers act.
#[test]
fn reassembly_table_capacity_is_checked_before_commit() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 34), 9000);
    let mut conn = challenged_connection(peer, 0x0100, 3);
    let open = |blob_id: u16| {
        let mut f = multi_frag(0, 2, 8);
        f.header.blob_id = blob_id;
        f
    };
    // 64 incomplete blobs over four unsequenced packets of 16 fragments.
    for packet in 0..4u16 {
        let frags: Vec<BlobFragment> = (0..16).map(|i| open(packet * 16 + i + 1)).collect();
        let dgram = encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders::default(),
            &frags,
            None,
            None,
        )
        .unwrap();
        let mut dgram = dgram;
        let mut parsed = decode_datagram(&dgram, true, None, false).unwrap();
        parsed.header.checksum =
            compute_packet_checksum(parsed.header, &parsed.raw_payload, None).unwrap();
        dgram[2..22].copy_from_slice(&parsed.header.to_wire_bytes());
        conn.receive_datagram(&dgram, peer, 0.1, WireCodecs::NONE)
            .unwrap();
    }
    while conn.pop_outbound_datagram().is_some() {}
    while conn.pop_event().is_some() {}
    let state = conn.state();
    let one_more = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        },
        &[open(999)],
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        conn.receive_datagram(&one_more, peer, 0.2, WireCodecs::NONE),
        Err(SessionError::ReassemblyTableFull { max_concurrent: 64 })
    );
    assert_eq!(conn.state(), state, "DisconnectHeader did not act");
    assert!(conn.pop_event().is_none());
}

/// §5.5: `intervalMs` is `floor(elapsed * 1000)` truncated to `u16` (the low 16 bits).
#[test]
fn flow_control_interval_keeps_the_low_16_bits() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 35), 9000);
    let mut conn = challenged_connection(peer, 0x0100, 4);
    // Receive at least 1,400 bytes (unsequenced packets with single-fragment blobs).
    for i in 0..4u16 {
        let mut frag = multi_frag(0, 1, 400);
        frag.header.blob_id = i;
        let dgram = encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders::default(),
            &[frag],
            None,
            None,
        )
        .unwrap();
        conn.receive_datagram(&dgram, peer, 0.1, WireCodecs::NONE)
            .unwrap();
    }
    while conn.pop_outbound_datagram().is_some() {}
    // 100.0 s after the connection was made: 100,000 ms, whose low 16 bits are 34,464.
    conn.send_sequenced_packet(OptionalHeaders::default(), Vec::new(), true, 100.0, None)
        .unwrap();
    let sent = decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, false).unwrap();
    let flow = sent
        .opt_headers
        .flow_control
        .expect("FlowControlHeader attached");
    assert_eq!(flow.interval_ms, (100_000u32 & 0xFFFF) as u16);
    assert!(flow.bytes_received_delta >= 1400);
}

/// `send_blob_to` writes an explicit `recipientId`; the default recipient `2` uses the
/// `DefaultRecipient` bit (§5.6.3 field 7).
#[test]
fn send_blob_to_an_explicit_recipient() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 36), 9000);
    let mut conn = challenged_connection(peer, 0x0100, 5);
    conn.send_blob_to(2, 0, 0x0100_0000_0000_0009, 0x52, &[0, 1], 0.1, None)
        .unwrap();
    conn.send_blob_to(2, 0, 2, 0x52, &[0, 1], 0.1, None)
        .unwrap();
    let explicit =
        decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, false).unwrap();
    let default =
        decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, false).unwrap();
    assert_eq!(
        explicit.fragments[0].header.recipient_id,
        0x0100_0000_0000_0009
    );
    assert_eq!(
        explicit.fragments[0].header.blob_flags & BLOB_FLAG_DEFAULT_RECIPIENT,
        0
    );
    assert_ne!(
        default.fragments[0].header.blob_flags & BLOB_FLAG_DEFAULT_RECIPIENT,
        0
    );
}

/// A removed connection's handle is not given out again at once, and traffic for its
/// recipient ID is refused, not routed to another connection with the same peer.
#[test]
fn removed_connections_are_refused_and_ids_move_on() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 37), 9000);
    let mut mux = SessionMultiplexer::new();
    let a = mux.create_connection(server, 0.0).unwrap();
    mux.connection_mut(a)
        .unwrap()
        .start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    mux.receive_from_peer(&challenge_datagram(0x0300), server, 0.05, WireCodecs::NONE)
        .unwrap();
    assert_eq!(mux.connection(a).unwrap().local_rec_id(), 0x0300);
    let b = mux.create_connection(server, 0.0).unwrap();
    mux.connection_mut(b)
        .unwrap()
        .start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    assert!(mux.remove_connection(a).is_some());
    assert!(mux.connection(a).is_none());
    let c = mux.create_connection(server, 0.0).unwrap();
    assert_ne!(c, a);
    let to_a = encode_datagram(
        DatagramHeader {
            rec_id: 0x0300,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        None,
        None,
    )
    .unwrap();
    assert!(matches!(
        mux.receive_from_peer(&to_a, server, 0.1, WireCodecs::NONE),
        Err(SessionError::UnknownRecipient { .. })
    ));
    assert_eq!(
        mux.connection(b).unwrap().state(),
        ConnectionState::SessionAuthSent
    );
}

/// R2: a connection that timed out (State 8) takes no traffic by address; a new
/// connection to the same server gets the challenge, and the closed one is reaped after
/// the 20 s cleanup of §6.1.
#[test]
fn closing_connections_take_no_address_routed_traffic_and_are_reaped() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 38), 9000);
    let mut mux = SessionMultiplexer::new();
    let auth = || account_auth("A", 0).unwrap();
    let old = mux.create_connection(server, 0.0).unwrap();
    mux.connection_mut(old)
        .unwrap()
        .start_login(auth(), 0.0)
        .unwrap();
    assert!(mux.tick(20.0).unwrap().is_empty());
    assert_eq!(
        mux.connection(old).unwrap().state(),
        ConnectionState::Closing
    );
    let new = mux.create_connection(server, 20.0).unwrap();
    mux.connection_mut(new)
        .unwrap()
        .start_login(auth(), 20.0)
        .unwrap();
    let (routed, _) = mux
        .receive_from_peer(&challenge_datagram(0), server, 20.5, WireCodecs::NONE)
        .unwrap();
    assert_eq!(routed, new);
    assert_eq!(
        mux.connection(new).unwrap().state(),
        ConnectionState::ChallengeAcceptSent
    );
    assert_eq!(mux.tick(40.0).unwrap(), vec![old]);
    assert!(mux.connection(old).is_none());
}

/// R7: the end of a local disconnect is reported with its own reason.
#[test]
fn local_disconnect_ends_with_a_reason() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 39), 9000);
    let mut conn = challenged_connection(peer, 0x0100, 6);
    conn.receive_datagram(&challenge_datagram(0), peer, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.state(), ConnectionState::Connected);
    while conn.pop_event().is_some() {}
    conn.request_disconnect(1.0);
    conn.tick(1.0).unwrap();
    assert_eq!(conn.state(), ConnectionState::DisconnectSent);
    conn.tick(21.0).unwrap();
    assert_eq!(conn.state(), ConnectionState::Closing);
    assert_eq!(
        conn.pop_event(),
        Some(SessionEvent::Disconnected {
            reason: DisconnectReason::LocalDisconnect
        })
    );
}

/// R4: the connection's `Debug` shows no credentials, cookie or datagram bytes.
#[test]
fn connection_debug_redacts_credentials() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 40), 9000);
    let mut conn = SessionConnection::new(peer, 0.0);
    conn.start_login(ticket_auth("canary-account", "Y2FuYXJ5", 0), 0.0)
        .unwrap();
    let text = format!("{conn:?}");
    assert!(!text.contains("canary"));
    assert!(text.contains("outbound_datagrams: 1"));
    let header = ticket_auth("canary-account", "Y2FuYXJ5", 0);
    assert!(!format!("{header:?}").contains("canary"));
}

/// Rebuilds an unmasked datagram's checksum by the receive rule (§4.4.2), so a test packet
/// with several header and fragment buffers is accepted.
fn with_receive_rule_checksum(mut dgram: Vec<u8>) -> Vec<u8> {
    let mut parsed = decode_datagram(&dgram, true, None, false).unwrap();
    parsed.header.checksum =
        compute_packet_checksum(parsed.header, &parsed.raw_payload, None).unwrap();
    dgram[2..22].copy_from_slice(&parsed.header.to_wire_bytes());
    dgram
}

/// §6.4 step 1, §5.6.4, §10.1 rule 8: the fragments of a packet that carries a
/// `ConnectChallengeHeader` are checked and ingested under one sender key, the challenge's
/// `serverRecId`.
#[test]
fn fragments_beside_a_challenge_are_keyed_by_the_new_server_id() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 41), 9000);
    let mut conn = SessionConnection::new(peer, 0.0);
    conn.start_login(account_auth("A", 0).unwrap(), 0.0)
        .unwrap();
    // While `serverRecId` is 0, blob 7 opens with 3 fragments.
    let early = with_receive_rule_checksum(
        encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders::default(),
            &[multi_frag(0, 3, 8)],
            None,
            None,
        )
        .unwrap(),
    );
    conn.receive_datagram(&early, peer, 0.1, WireCodecs::NONE)
        .unwrap();
    // The challenge packet opens blob 7 with 2 fragments. It is keyed by serverRecId
    // 0x0304, so it does not clash with the 3-fragment blob under ID 0.
    let opening = with_receive_rule_checksum(
        encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders {
                connect_challenge: Some(vector_5_challenge()),
                ..OptionalHeaders::default()
            },
            &[multi_frag(0, 2, MAX_BLOB_FRAG_PAYLOAD)],
            None,
            None,
        )
        .unwrap(),
    );
    conn.receive_datagram(&opening, peer, 0.2, WireCodecs::NONE)
        .expect("checked and ingested under serverRecId 0x0304");
    assert_eq!(conn.server_rec_id(), 0x0304);
    while conn.pop_event().is_some() {}
    // Its second fragment completes the blob under the same key.
    let closing = with_receive_rule_checksum(
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0102,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &[multi_frag(1, 2, 8)],
            None,
            None,
        )
        .unwrap(),
    );
    conn.receive_datagram(&closing, peer, 0.3, WireCodecs::NONE)
        .unwrap();
    let mut blobs = Vec::new();
    while let Some(ev) = conn.pop_event() {
        if let SessionEvent::BlobReceived(blob) = ev {
            blobs.push(blob);
        }
    }
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].payload.len(), MAX_BLOB_FRAG_PAYLOAD + 8);
}

/// §4.1.1, §7.3: with an all-zero `codecUuid` nothing is compressed or decompressed: sends
/// carry `wirePrefix` `0x0000` and a datagram with a non-zero `wirePrefix` is refused. With a
/// negotiated `codecUuid` both directions use the codecs.
#[test]
fn null_codec_uuid_means_no_wire_compression() {
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 42), 9000);
    let codec = CountingCodec::new();
    let codecs = WireCodecs {
        incoming: Some(&codec),
        outgoing: Some(&codec),
    };
    let compressed = encode_datagram(
        DatagramHeader {
            rec_id: 0x0100,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            cumulative_ack: Some(0x1234_5678),
            clock_sync: Some(12_345.678),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        Some(&codec),
    )
    .unwrap();
    assert_eq!(u16::from_be_bytes([compressed[0], compressed[1]]), 0x0100);
    assert!(
        compressed.len() >= 22,
        "long enough to reach the prefix check"
    );

    // Null codecUuid: plain send, compressed receive refused.
    let mut plain = challenged_connection(peer, 0x0100, 5);
    assert_eq!(plain.active_codec_uuid(), [0u8; 16]);
    let encodes = codec.encodes();
    plain
        .send_blob(2, 0, 0x10, &[0x00; 32], 0.1, codecs.outgoing)
        .unwrap();
    let sent = plain.pop_outbound_datagram().unwrap();
    assert_eq!(&sent[0..2], &[0x00, 0x00]);
    assert_eq!(codec.encodes(), encodes);
    decode_datagram(&sent, false, None, false).expect("an unencoded datagram");
    assert_eq!(
        plain.receive_datagram(&compressed, peer, 0.2, codecs),
        Err(SessionError::MissingWireCodec {
            wire_prefix: 0x0100
        })
    );
    assert_eq!(plain.state(), ConnectionState::ChallengeAcceptSent);

    // Negotiated codecUuid: compressed send with wirePrefix = remoteRecId, compressed
    // receive accepted.
    let mut negotiated = challenged_connection_with_codec(peer, 0x0100, 5, [0x55; 16]);
    let encodes = codec.encodes();
    negotiated
        .send_blob(2, 0, 0x10, &[0x00; 32], 0.1, codecs.outgoing)
        .unwrap();
    let sent = negotiated.pop_outbound_datagram().unwrap();
    assert_eq!(u16::from_be_bytes([sent[0], sent[1]]), 0x0304);
    assert_eq!(codec.encodes(), encodes + 1);
    negotiated
        .receive_datagram(&compressed, peer, 0.2, codecs)
        .expect("decompressed with the incoming codec");
    assert_eq!(negotiated.state(), ConnectionState::Connected);
}

/// `official-login.md` §6.3: the buffers that hold the ticket are sized exactly, so no
/// reallocation frees an unwiped copy, and a `SessionAuthHeader` packet is never passed to a
/// codec (§4.1.1).
#[test]
fn session_auth_buffers_are_sized_exactly_and_never_compressed() {
    let ticket = synthetic_ticket();
    let auth = ticket_auth("account\u{e9}", &ticket, 7);
    let credentials = auth.credentials.encode().unwrap();
    assert_eq!(credentials.len(), auth.credentials.encoded_len());
    assert_eq!(credentials.capacity(), credentials.len());
    // The ticket is carried as its text, never decoded (§6.2.1).
    assert!(credentials.ends_with(ticket.as_bytes()));

    let mut out = Vec::new();
    let written = auth.encode(&mut out).unwrap();
    assert_eq!(written, auth.encoded_len());
    assert_eq!(out.len(), auth.encoded_len());

    let opt = OptionalHeaders {
        session_auth: Some(auth.clone()),
        cumulative_ack: Some(3),
        flow_control: Some(FlowControlHeader {
            bytes_received_delta: 1,
            interval_ms: 2,
        }),
        ..OptionalHeaders::default()
    };
    let mut all = Vec::new();
    assert_eq!(opt.encode(&mut all).unwrap(), opt.encoded_len());
    assert_eq!(all.len(), opt.encoded_len());

    let codec = CountingCodec::new();
    let dgram = encode_datagram(DatagramHeader::default(), &opt, &[], None, Some(&codec)).unwrap();
    assert_eq!(codec.encodes(), 0);
    assert_eq!(&dgram[0..2], &[0x00, 0x00]);
    assert_eq!(dgram.len(), 2 + 20 + opt.encoded_len());
    let parsed = decode_datagram(&dgram, false, None, false).unwrap();
    assert_eq!(parsed.opt_headers.session_auth, Some(auth));
}

/// [`OptionalHeaders::encoded_len`] matches the encoder for every header (§4.3).
#[test]
fn optional_headers_encoded_len_matches_encode() {
    let opt = OptionalHeaders {
        server_redirect: Some(ServerRedirectHeader {
            switch_seq: 1,
            switch_type: 2,
        }),
        auth_server_address: Some(AuthServerAddressHeader {
            sin_family: 2,
            addr: SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 9000),
        }),
        world_referral: Some(WorldReferralHeader {
            referral_cookie: 0x77,
            sin_family: 2,
            target_addr: SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 2), 9001),
            reserved_word_10: 0,
        }),
        sequence_nak: Some(vec![4, 5, 6]),
        out_of_order_ack: Some(vec![9]),
        cumulative_ack: Some(3),
        disconnect: true,
        session_auth: Some(account_auth("A", 0).unwrap()),
        referral_auth: Some(ReferralAuthHeader {
            referral_cookie: 0x55,
        }),
        connect_challenge: Some(vector_5_challenge()),
        connect_accept: Some(ConnectAcceptHeader {
            check_value: 1,
            codec_uuid: [0u8; 16],
            max_recv_rate: 2,
        }),
        transport_error: Some(TransportErrorHeader {
            error_code: 1,
            error_subcode: 2,
        }),
        transport_fatal_error: Some(TransportErrorHeader {
            error_code: 3,
            error_subcode: 4,
        }),
        client_control: Some(7),
        clock_sync: Some(1.5),
        flow_control: Some(FlowControlHeader {
            bytes_received_delta: 1,
            interval_ms: 2,
        }),
    };
    let mut out = Vec::new();
    assert_eq!(opt.encode(&mut out).unwrap(), opt.encoded_len());
    let buffers = opt.encode_buffers().unwrap();
    assert_eq!(
        buffers.iter().map(Vec::len).sum::<usize>(),
        opt.encoded_len()
    );
    assert_eq!(OptionalHeaders::default().encoded_len(), 0);
}

/// §9.3: `queuePosition` and `queueTotal` follow `detailText` only for `0x80041464`; `0` is
/// accepted; any other negative `logonResult` refuses.
#[test]
fn login_queue_status_reads_the_queue_only_when_queued() {
    let queued = LoginQueueStatusMessage {
        login_result: LOGON_RESULT_QUEUED,
        detail_text: "wait".to_owned(),
        queue: Some(LoginQueuePosition {
            position: 4,
            total: 9,
        }),
    };
    let body = queued.encode();
    assert_eq!(&body[..4], &[0x64, 0x14, 0x04, 0x80]);
    assert_eq!(body.len(), 4 + 1 + 8 + 8);
    assert_eq!(LoginQueueStatusMessage::decode(&body).unwrap(), queued);
    assert_eq!(
        queued.outcome(),
        LogonOutcome::Queued(LoginQueuePosition {
            position: 4,
            total: 9
        })
    );
    // Queued but the position is missing: malformed.
    assert!(LoginQueueStatusMessage::decode(&body[..body.len() - 1]).is_err());

    // Accepted: `logonResult` and an empty `detailText` (5 bytes after envelopeFlags).
    let accepted = [0u8, 0, 0, 0, 0];
    let msg = LoginQueueStatusMessage::decode(&accepted).unwrap();
    assert_eq!(msg.queue, None);
    assert_eq!(msg.outcome(), LogonOutcome::Accepted);
    // A position given with an accepted result is not read, and not written.
    let with_extra = [&accepted[..], &[1, 0, 0, 0, 2, 0, 0, 0]].concat();
    assert_eq!(
        LoginQueueStatusMessage::decode(&with_extra).unwrap().queue,
        None
    );
    let not_written = LoginQueueStatusMessage {
        login_result: 0,
        detail_text: String::new(),
        queue: Some(LoginQueuePosition {
            position: 1,
            total: 2,
        }),
    };
    assert_eq!(not_written.encode(), accepted);

    // Refused: any other negative value.
    let refused = [0x05u8, 0x40, 0x00, 0x80, 0x00];
    assert_eq!(
        LoginQueueStatusMessage::decode(&refused).unwrap().outcome(),
        LogonOutcome::Refused(0x8000_4005)
    );
    assert_eq!(
        LoginQueueStatusMessage::decode(&[7, 0, 0, 0, 0])
            .unwrap()
            .outcome(),
        LogonOutcome::Unknown(7)
    );
}

/// §3.4: a new connection waits at `localRecId` 0; a challenge that assigns a slot another
/// connection of the multiplexer occupies is ignored.
#[test]
fn an_occupied_slot_ignores_the_challenge() {
    let first = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 61), 9000);
    let second = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 62), 9000);
    let mut mux = SessionMultiplexer::new();
    let a = mux.create_connection(first, 0.0).unwrap();
    let b = mux.create_connection(second, 0.0).unwrap();
    assert_ne!(a, b);
    for (h, _) in [(a, first), (b, second)] {
        let conn = mux.connection_mut(h).unwrap();
        assert_eq!(conn.local_rec_id(), 0);
        conn.start_login(account_auth("A", 0).unwrap(), 0.0)
            .unwrap();
    }
    let challenge = |assigned: u16| {
        let mut c = vector_5_challenge();
        c.assigned_client_rec_id = assigned;
        encode_datagram(
            DatagramHeader::default(),
            &OptionalHeaders {
                connect_challenge: Some(c),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .unwrap()
    };
    let (taken, _) = mux
        .receive_from_peer(&challenge(0x0200), first, 0.1, WireCodecs::NONE)
        .unwrap();
    assert_eq!(taken, a);
    assert_eq!(mux.connection(a).unwrap().local_rec_id(), 0x0200);
    assert_eq!(mux.handle_of(0x0200), Some(a));
    // The second server assigns the same slot: ignored, nothing changes or is answered.
    while mux
        .connection_mut(b)
        .unwrap()
        .pop_outbound_datagram()
        .is_some()
    {}
    assert_eq!(
        mux.receive_from_peer(&challenge(0x0200), second, 0.2, WireCodecs::NONE)
            .map(|(h, _)| h),
        Err(SessionError::RecipientIdCollision { rec_id: 0x0200 })
    );
    let b_conn = mux.connection_mut(b).unwrap();
    assert_eq!(b_conn.local_rec_id(), 0);
    assert_eq!(b_conn.state(), ConnectionState::SessionAuthSent);
    assert!(b_conn.pop_outbound_datagram().is_none());
    // A free slot is taken.
    mux.receive_from_peer(&challenge(0x0201), second, 0.3, WireCodecs::NONE)
        .unwrap();
    assert_eq!(mux.connection(b).unwrap().local_rec_id(), 0x0201);
}

/// §5.6.2–§5.6.3: `blobId` is on the wire for queue 3 when the blob is fragmented; a
/// receiver accepts queues 5 and 6 with the `orderNum` + `blobId` framing of 2 and 4.
#[test]
fn queue_3_fragments_carry_a_blob_id_and_queues_5_and_6_are_received() {
    let frags = split_blob(3, 9, 0x0042, 0x13, None, 0x6F, &vec![7u8; 500]).unwrap();
    assert_eq!(frags.len(), 2);
    for f in &frags {
        let mut wire = Vec::new();
        f.header.encode(&mut wire).unwrap();
        // queueAndOpt, blobFlags (DefaultRecipient | IsFragmented), seqGroup 0x13,
        // orderNum 9, blobId 0x0042.
        assert_eq!(&wire[..7], &[0x03, 0x50, 0x13, 0x09, 0x00, 0x42, 0x00]);
        let (back, _) = BlobFragmentHeader::decode(&wire).unwrap();
        assert_eq!(back.blob_id, 0x0042);
        assert_eq!(back.seq_group, 0x13);
    }
    // Unfragmented queue 3: no blobId.
    let single = split_blob(3, 9, 0x0042, 0x13, None, 0x6F, &[1, 2]).unwrap();
    let mut wire = Vec::new();
    single[0].header.encode(&mut wire).unwrap();
    assert_eq!(&wire[..5], &[0x03, 0x40, 0x13, 0x09, 0x00]);
    assert_eq!(BlobFragmentHeader::decode(&wire).unwrap().0.blob_id, 0);

    // Queues 5 and 6: orderNum and blobId, decoded on receive; never sent.
    for queue in [5u8, 6] {
        let raw = [queue, 0x40, 0x07, 0x00, 0x2A, 0x00, 0x05, 0x01];
        let (hdr, used) = BlobFragmentHeader::decode(&raw).unwrap();
        assert_eq!(used, 8);
        assert_eq!(
            (hdr.queue_id, hdr.order_num, hdr.blob_id, hdr.frag_data_len),
            (queue, 7, 0x2A, 1)
        );
        assert_eq!(
            hdr.encode(&mut Vec::new()),
            Err(SessionError::InvalidQueueId { queue_id: queue })
        );
    }
    assert!(matches!(
        BlobFragmentHeader::decode(&[7, 0x40, 0, 0, 0, 0, 0, 0]),
        Err(SessionError::InvalidQueueId { queue_id: 7 })
    ));
}

/// §4.2: `time` is set on a packet with `ClockSyncHeader` or `FlowControlHeader` (network
/// milliseconds modulo 65,536) and is 0 otherwise (`OPEN-SESSION-8`); `iteration` and
/// `table` are always 0.
#[test]
fn the_time_field_is_set_only_with_a_timing_header() {
    assert_eq!(tessera_compat::session::network_time_ms(1.5), 1500);
    assert_eq!(tessera_compat::session::network_time_ms(65.5361), 0);
    assert_eq!(tessera_compat::session::network_time_ms(f64::NAN), 0);
    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 70), 9000);
    let mut conn = challenged_connection(peer, 0x0150, 1);
    // The next server packet connects (State 5) and queues the initial ClockSyncHeader.
    let mut ack = DatagramHeader {
        rec_id: 0x0150,
        flags: FLAG_CUMULATIVE_ACK,
        pak_size: 4,
        ..DatagramHeader::default()
    };
    ack.checksum = compute_packet_checksum(ack, &[0, 0, 0, 0], None).unwrap();
    let mut dgram = vec![0u8, 0];
    dgram.extend_from_slice(&ack.to_wire_bytes());
    dgram.extend_from_slice(&[0, 0, 0, 0]);
    conn.receive_datagram(&dgram, peer, 2.25, WireCodecs::NONE)
        .unwrap();
    assert_eq!(conn.state(), ConnectionState::Connected);
    let sync = decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, true).unwrap();
    let net_time = sync.opt_headers.clock_sync.expect("ClockSyncHeader");
    assert_eq!(
        sync.header.time,
        tessera_compat::session::network_time_ms(net_time)
    );
    assert_ne!(sync.header.time, 0);
    assert_eq!((sync.header.iteration, sync.header.table), (0, 0));
    // A sequenced blob without a timing header: `time` 0.
    conn.send_blob(4, 0, 0x12C, &[0, 1, 2], 2.5, None).unwrap();
    let blob = decode_datagram(&conn.pop_outbound_datagram().unwrap(), false, None, false).unwrap();
    assert!(blob.opt_headers.flow_control.is_none());
    assert_eq!(blob.header.time, 0);
    assert_eq!(blob.header.seq_num, 2);
}

/// §4.4.2 step 2 with the client's own fragment layouts (§5.6.2–§5.6.3), hand-built: a
/// queue-4 `0x12C` fragment and a queue-3, sequence-group-0x13 `0x6F` fragment. The header
/// bytes are written out from the §5.6.3 table, and the expected sums were worked out from
/// §4.4.1 by hand, not with the crate's functions.
#[test]
fn send_checksum_vectors_for_the_version_and_entry_fragments() {
    // `0x12C`: queue 4, default recipient, orderNum 1, blobId 1, header word 0x12C
    // (VariableU32 `81 2C`), 7 payload bytes, one pad byte: a 10-byte header (2 mod 4).
    let version_payload: Vec<u8> = (0..7u8).collect();
    let frags = split_blob(4, 1, 1, 0, None, 0x0000_012C, &version_payload).unwrap();
    let [version] = frags.as_slice() else {
        panic!("{frags:?}");
    };
    let mut bytes = Vec::new();
    version.encode(&mut bytes).unwrap();
    assert_eq!(
        &bytes[..10],
        &[0x04, 0x40, 0x01, 0x00, 0x01, 0x00, 0x81, 0x2C, 0x07, 0x00]
    );
    assert_eq!(&bytes[10..], version_payload.as_slice());
    let sum = send_payload_checksum(&OptionalHeaders::default(), frags.as_slice()).unwrap();
    assert_eq!(sum, 0x3A9A_4705, "per buffer");
    assert_eq!(
        compute_datagram_checksum(&bytes),
        0x3897_430E,
        "whole payload"
    );

    // `0x6F`: queue 3, sequence group 0x13, orderNum 5, no blobId (unfragmented), header
    // word 0x6F, 9 payload bytes; 7 header bytes and one pad byte (0 mod 4). The sums still
    // differ from the whole-payload one: the payload's tail carries its own length term.
    let entry_payload = [0x00, 0x42, 0, 0, 0, 0, 0, 0, 0x02];
    let frags = split_blob(3, 5, 0, 0x13, None, 0x0000_006F, &entry_payload).unwrap();
    let mut bytes = Vec::new();
    frags[0].encode(&mut bytes).unwrap();
    assert_eq!(
        &bytes[..8],
        &[0x03, 0x40, 0x13, 0x05, 0x00, 0x6F, 0x09, 0x00]
    );
    assert_eq!(
        send_payload_checksum(&OptionalHeaders::default(), frags.as_slice()).unwrap(),
        0x0725_F103
    );
    assert_eq!(
        compute_datagram_checksum(&bytes),
        0x072D_F103,
        "whole payload"
    );
    // With the 6-byte `FlowControlHeader` in front, three buffers.
    let opt = OptionalHeaders {
        flow_control: Some(FlowControlHeader {
            bytes_received_delta: 0x0102_0304,
            interval_ms: 0x0506,
        }),
        ..OptionalHeaders::default()
    };
    assert_eq!(
        send_payload_checksum(&opt, frags.as_slice()).unwrap(),
        0x0E32_F407
    );
}
