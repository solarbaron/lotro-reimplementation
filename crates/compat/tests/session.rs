// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic integration tests for `tessera_compat::session` (`docs/specs/official-session.md` §3–§10).

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, AUTH_FLAG_HAS_SECOND_STRING, AUTH_TYPE_ACCOUNT, AUTH_TYPE_TICKET,
    AuthServerAddressHeader, BIT_TREE_TABLE_FORMAT_UUID, BLOB_FLAG_DEFAULT_RECIPIENT,
    BLOB_FLAG_ENCRYPTED, BLOB_FLAG_HAS_ATTACHMENT, BitTreeCodec, BlobFragment, BlobFragmentHeader,
    BlobReassembler, ClientVersionMessage, ConnectAcceptHeader, ConnectChallengeHeader,
    ConnectionState, ControlMessage, DEFAULT_MAX_CONTAINER_DEPTH,
    DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES, DID_DICT_CLIENT_TO_SERVER, DID_DICT_SERVER_TO_CLIENT,
    DID_DICT_SERVER_TO_CLIENT_DEFERRED, DatBitTreeNode, DatBitTreeTable, DatagramHeader,
    DisconnectReason, ENVELOPE_FLAG_HAS_ROUTING_HEADER, ENVELOPE_FLAG_ZLIB_COMPRESSED,
    FLAG_AUTH_SERVER_ADDR, FLAG_CLIENT_CONTROL, FLAG_CLOCK_SYNC, FLAG_CONNECT_CHALLENGE,
    FLAG_CUMULATIVE_ACK, FLAG_DISCONNECT, FLAG_FLOW_CONTROL, FLAG_HAS_FRAGMENTED_BLOBS,
    FLAG_MASKED_CHECKSUM, FLAG_OUT_OF_ORDER_ACK, FLAG_SEQUENCE_NAK, FLAG_SERVER_REDIRECT,
    FLAG_TRANSPORT_ERROR, FLAG_UNDEFINED_10, FLAG_UNDEFINED_23, FLAG_WORLD_REFERRAL,
    FlowControlHeader, FragmentAttachment, INVALID_DATAGRAM_FLAGS_MASK, InMemoryUdpTransport,
    IsaacCipher, LoginQueueStatusMessage, MAX_BLOB_FRAG_PAYLOAD, MAX_FRAGS_PER_PACKET,
    MAX_NAK_ENTRIES, MAX_SEQ_LOOKAHEAD, MessageRoutingHeader, OPCODE_BATCH_CONTAINER,
    OPCODE_CLIENT_VERSION, OPCODE_LOGIN_QUEUE_STATUS, OPCODE_SINGLE_WRAPPER,
    OPCODE_SYSTEM_NOTIFICATION, OPCODE_VERSION_REQUEST, OPCODE_VERSION_STATUS, OptionalHeaders,
    ReferralAuthHeader, ServerRedirectHeader, SessionAuthCredentials, SessionAuthHeader,
    SessionConnection, SessionError, SessionEvent, SessionMultiplexer, SystemNotificationMessage,
    TransportErrorHeader, UdpDatagramTransport, VALID_DATAGRAM_FLAGS_MASK, VersionStatusMessage,
    WorldReferralHeader, candidate_bind_ports, compute_datagram_checksum, compute_packet_checksum,
    compute_unique_port, decode_datagram, decode_message, decode_ticket_base64,
    decode_variable_u32, des3_cbc_decrypt, des3_cbc_encrypt, dispatch_message,
    encode_batch_container, encode_datagram, encode_message, encode_message_with_routing,
    encode_single_wrapper, encode_single_wrapper_with_context, encode_variable_u32,
    sndbuf_probe_midpoint, split_blob, verify_packet_checksum,
};

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

    // Case A: explicit base port
    let ports_a = candidate_bind_ports(Ipv4Addr::new(192, 0, 2, 10), 2900, false);
    assert_eq!(ports_a.len(), 12);
    assert_eq!(ports_a[0], 2900);
    assert_eq!(ports_a[11], 2911);

    // Case B: computed unique port > 0x03FF
    let ip_be = u32::from_be_bytes([192, 0, 2, 10]);
    for attempt in 0..12 {
        let p = compute_unique_port(ip_be, attempt);
        assert!(p > 0x03FF);
    }
    // Even if ip_be % 0xFA07 <= 0x03FF (e.g. ip_be = 100), returned port skips privileged range
    let p_low = compute_unique_port(100, 0);
    assert!(p_low > 0x03FF);
    assert_eq!(p_low, 100 + 256 * 4);

    // Case C: ephemeral wildcard
    assert_eq!(
        candidate_bind_ports(Ipv4Addr::UNSPECIFIED, 0, false),
        vec![0]
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
    let tables = DatBitTreeTable::synthetic_dictionary([0x11; 16]);
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

/// A synthetic 144-character Base64 ticket (108 bytes, no padding).
fn synthetic_ticket() -> (String, Vec<u8>) {
    let raw: Vec<u8> = (0..108u8).map(|i| i.wrapping_mul(7)).collect();
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for group in raw.chunks(3) {
        let n = (u32::from(group[0]) << 16) | (u32::from(group[1]) << 8) | u32::from(group[2]);
        for shift in [18, 12, 6, 0] {
            text.push(char::from(ALPHABET[((n >> shift) & 0x3F) as usize]));
        }
    }
    (text, raw)
}

#[test]
fn session_auth_selection_rule_and_ticket_length() {
    let (ticket, raw) = synthetic_ticket();
    assert_eq!(ticket.len(), 144);
    assert_eq!(decode_ticket_base64(&ticket).unwrap(), raw);

    // Rule 1: ticket present -> 0x40000002 + HasExtraBlob; blobLen is the decoded length.
    let with_ticket = SessionAuthHeader::for_login("SUBSCRIPTION", Some(&ticket), 1_000).unwrap();
    assert_eq!(with_ticket.auth_type, AUTH_TYPE_TICKET);
    assert_eq!(with_ticket.auth_flags, AUTH_FLAG_HAS_EXTRA_BLOB);
    assert_eq!(
        with_ticket.credentials.extra_blob.as_deref(),
        Some(&raw[..])
    );
    let mut buf = Vec::new();
    let len = with_ticket.encode(&mut buf).unwrap();
    assert_eq!(&buf[0..7], b"180804\0");
    let padded = u32::from_le_bytes([buf[7], buf[8], buf[9], buf[10]]) as usize;
    assert_eq!(padded % 4, 0);
    assert_eq!(len, 11 + padded);
    // authType 0x40000002 and authFlags 4 at the start of authBlock.
    assert_eq!(
        &buf[11..19],
        &[0x02, 0x00, 0x00, 0x40, 0x04, 0x00, 0x00, 0x00]
    );
    // ... and blobLen 108 (0x6C) after the 12-character account string.
    let blob_len_at = 11 + 12 + 1 + 2 * "SUBSCRIPTION".len();
    assert_eq!(&buf[blob_len_at..blob_len_at + 4], &108u32.to_le_bytes());
    let (decoded, consumed) = SessionAuthHeader::decode(&buf).unwrap();
    assert_eq!(consumed, len);
    assert_eq!(decoded, with_ticket);

    // Rule 2: no ticket -> 0x00000001, authFlags 0, the account string only.
    let account_only = SessionAuthHeader::for_login("SUBSCRIPTION", None, 1_000).unwrap();
    assert_eq!(account_only.auth_type, AUTH_TYPE_ACCOUNT);
    assert_eq!(account_only.auth_flags, 0);
    let mut buf = Vec::new();
    account_only.encode(&mut buf).unwrap();
    assert_eq!(SessionAuthHeader::decode(&buf).unwrap().0, account_only);

    // The alternate receiver encoding: 0x00000001 with the ticket in secondStr.
    let alternate = SessionAuthHeader {
        auth_type: AUTH_TYPE_ACCOUNT,
        auth_flags: AUTH_FLAG_HAS_SECOND_STRING,
        client_timestamp: 5,
        credentials: SessionAuthCredentials {
            account_or_ticket: "SUBSCRIPTION".to_owned(),
            second_string: Some(ticket.clone()),
            extra_blob: None,
        },
    };
    let mut buf = Vec::new();
    alternate.encode(&mut buf).unwrap();
    assert_eq!(SessionAuthHeader::decode(&buf).unwrap().0, alternate);

    // Padding variants of the decoded length.
    assert_eq!(decode_ticket_base64("QUJD").unwrap(), b"ABC");
    assert_eq!(decode_ticket_base64("QUI=").unwrap(), b"AB");
    assert_eq!(decode_ticket_base64("QQ==").unwrap(), b"A");
    assert_eq!(decode_ticket_base64("QQ").unwrap(), b"A");
    for bad in ["", "Q", "QQ=", "Q===", "QU=I", "QU!I"] {
        assert_eq!(
            decode_ticket_base64(bad),
            Err(SessionError::InvalidTicketEncoding),
            "{bad:?}"
        );
    }
    assert_eq!(
        SessionAuthHeader::for_login("A", Some("not base64!"), 0),
        Err(SessionError::InvalidTicketEncoding)
    );
}

#[test]
fn session_auth_refuses_encrypted_and_unknown_flags() {
    let header = SessionAuthHeader::for_login("A", None, 0).unwrap();
    let mut buf = Vec::new();
    header.encode(&mut buf).unwrap();
    // authFlags lives at authBlock + 4 = datagram offset 15.
    for flags in [0x01u32, 0x08, 0x800, 0x1000] {
        let mut bad = buf.clone();
        bad[15..19].copy_from_slice(&flags.to_le_bytes());
        assert_eq!(
            SessionAuthHeader::decode(&bad),
            Err(SessionError::UnsupportedAuthFlags { flags })
        );
    }
    let mut bad_header = header.clone();
    bad_header.auth_flags = 0x01;
    assert_eq!(
        bad_header.encode(&mut Vec::new()),
        Err(SessionError::UnsupportedAuthFlags { flags: 0x01 })
    );
    // Non-zero padding after the credentials is malformed.
    let mut bad_pad = buf.clone();
    *bad_pad.last_mut().unwrap() = 0x55;
    assert_eq!(
        SessionAuthHeader::decode(&bad_pad),
        Err(SessionError::InvalidAuthBlock)
    );
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

#[test]
fn vector_5_connect_accept_derivation() {
    let challenge = vector_5_challenge();
    let mut wire = Vec::new();
    challenge.encode(&mut wire);
    assert_eq!(
        wire,
        [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x8F, 0x40, 0x02, 0x01, 0x04, 0x03, 0x11, 0x22,
            0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]
    );
    assert_eq!(challenge.check_value(), 0x0201_0403_1122_3344);

    // The connection answers with the Vector 5 ConnectAcceptHeader bytes.
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 5), 9000);
    let mut conn = SessionConnection::new(0x0200, server, 0.0);
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
        .unwrap();
    conn.pop_outbound_datagram().expect("SessionAuthHeader");
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
    conn.receive_datagram(&dgram, server, 0.1, None).unwrap();
    // §6.4 step 1: the assigned ID becomes the local ID (Vector 5: 0x0102).
    assert_eq!(conn.local_rec_id(), 0x0102);
    assert_eq!(conn.server_rec_id(), 0x0304);
    let accept = conn.pop_outbound_datagram().expect("ConnectAcceptHeader");
    // wirePrefix 0, 20-byte header, then the 28-byte ConnectAcceptHeader.
    assert_eq!(
        &accept[22..50],
        &[
            0x44, 0x33, 0x22, 0x11, 0x03, 0x04, 0x01, 0x02, 0x02, 0x01, 0x05, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        ]
    );
    let parsed = decode_datagram(&accept, false, None, true).unwrap();
    let resp = parsed.opt_headers.connect_accept.unwrap();
    assert_eq!(resp.check_value, 0x0201_0403_1122_3344);
    assert_eq!(resp.local_rec_id, 0x0102);
}

#[test]
fn assigned_rec_id_zero_keeps_and_out_of_range_is_ignored() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 6), 9000);
    for (assigned, expected) in [(0u16, 0x0150u16), (0x0001, 0x0150), (0xFF00, 0x0150)] {
        let mut conn = SessionConnection::new(0x0150, server, 0.0);
        conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
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
        conn.receive_datagram(&dgram, server, 0.1, None).unwrap();
        assert_eq!(conn.local_rec_id(), expected);
    }
}

#[test]
fn unsupported_codec_uuid_closes_with_reason() {
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 7), 9000);
    let mut conn = SessionConnection::new(0x0150, server, 0.0);
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
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
        conn.receive_datagram(&dgram, server, 0.1, None)
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
    let mut conn = SessionConnection::new(0x0150, server, 0.0);
    conn.register_codec_uuid([0xAB; 16]);
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
        .unwrap();
    conn.receive_datagram(&dgram, server, 0.1, None).unwrap();
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
            local_rec_id: 0x0150,
            client_conn_idx: 3,
            codec_uuid: [0u8; 16],
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
    let login_req = SessionAuthHeader::for_login("acct", None, 1000).expect("SessionAuthHeader");
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
        .receive_from_peer(&cr_dgram, login_server_addr, 0.1, None)
        .expect("receive ConnectChallenge");
    assert_eq!(routed_id, 0x0111);
    let conn = mux.connection(routed_id).expect("conn after rekey");
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
            .local_rec_id,
        0x0111
    );

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
    mux.receive_from_peer(&pkt_seq_4, login_server_addr, 0.2, None)
        .expect("receive seq 4 out of order");
    {
        let conn = mux.connection_mut(routed_id).expect("conn");
        // Connection transitioned State 2 -> State 5 and queued initial ClockSync
        assert_eq!(conn.state(), ConnectionState::Connected);
        assert_eq!(conn.recv_seq_expected(), 4);
        assert_eq!(conn.missing_gap_count(), 2); // gaps for seq 2 and 3

        // Tick at t = 0.9 s (>= 0.6 s) to trigger SequenceNak for missing seqs [2, 3]
        conn.tick(0.9, None).expect("tick nak");
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
    mux.receive_from_peer(&pkt_seq_2, login_server_addr, 1.0, None)
        .expect("receive seq 2 using saved ISAAC mask");
    mux.receive_from_peer(&pkt_seq_3, login_server_addr, 1.1, None)
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
    mux.receive_from_peer(&ref_dgram, login_server_addr, 1.2, None)
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
        false,
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

    // A blob that arrived encrypted is refused (§7.1, OPEN-SESSION-6).
    assert_eq!(
        dispatch_message(
            OPCODE_BATCH_CONTAINER,
            &batch_payload,
            true,
            DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
            DEFAULT_MAX_CONTAINER_DEPTH,
        ),
        Err(SessionError::EncryptedBlobRejected)
    );

    // 0x179 carries contextId (u64) and resultCode (u32) after the inner payload (§9.4.2).
    let inner = encode_message(0, &[0x00, 0x01], false).unwrap();
    let wrapped = encode_single_wrapper_with_context(0x0000_0203, &inner, 7, 0, false).unwrap();
    let out = dispatch_message(
        OPCODE_SINGLE_WRAPPER,
        &wrapped,
        false,
        DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
        DEFAULT_MAX_CONTAINER_DEPTH,
    )
    .unwrap();
    assert_eq!(out[0].opcode, 0x0000_0203);
    let truncated = &wrapped[..wrapped.len() - 1];
    assert_eq!(
        dispatch_message(
            OPCODE_SINGLE_WRAPPER,
            truncated,
            false,
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
    let tables = DatBitTreeTable::synthetic_dictionary([0x22; 16]);
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
    fh.checksum = compute_packet_checksum(fh, &[], None);
    forbidden_dgram.extend_from_slice(&fh.to_wire_bytes());
    assert!(matches!(
        decode_datagram(&forbidden_dgram, true, None, true),
        Err(SessionError::InvalidHeaderFlags { .. })
    ));

    // Rule 4, 5, 6: Source address binding, sequence lookahead > 0x7FFF, and checksum mismatch without mutating state
    let mut conn = SessionConnection::new(0x0100, peer, 0.0);
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
        conn.receive_datagram(&valid_empty, wrong_peer, 0.1, None),
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
        conn.receive_datagram(&out_of_window, peer, 0.1, None),
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
        conn.receive_datagram(&bad_csum, peer, 0.1, None),
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
// §4.1.2–§4.1.3 .dat DatBitTreeTable & 4-Bit wireBody[0] Header
// ---------------------------------------------------------------------------

#[test]
fn dat_bit_tree_table_and_4bit_wire_body_header() {
    let codec_uuid = [
        0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0,
        0x01,
    ];
    let dict = DatBitTreeTable::synthetic_dictionary(codec_uuid);
    let dat_bytes = dict.to_dat_bytes();

    // Verify 44-byte header fields
    assert_eq!(&dat_bytes[0..16], &BIT_TREE_TABLE_FORMAT_UUID);
    assert_eq!(&dat_bytes[16..32], &codec_uuid);
    assert_eq!(
        u32::from_le_bytes([dat_bytes[36], dat_bytes[37], dat_bytes[38], dat_bytes[39]]),
        1
    );

    let parsed = DatBitTreeTable::from_dat_bytes(&dat_bytes).expect("parse .dat dictionary");
    assert_eq!(parsed, dict);

    // §4.1.3 item 2: the parser requires version == 1.
    let mut dat_v10001 = dat_bytes.clone();
    dat_v10001[36..40].copy_from_slice(&0x0001_0001u32.to_le_bytes());
    assert_eq!(
        DatBitTreeTable::from_dat_bytes(&dat_v10001),
        Err(SessionError::InvalidBitTreeTable)
    );

    // Redirect rule (§4.1.3 item 1): follow redirectDid only when non-zero and not the
    // record's own DID.
    let mut stub = vec![0u8; 44];
    stub[0..16].copy_from_slice(&BIT_TREE_TABLE_FORMAT_UUID);
    stub[32..36].copy_from_slice(&DID_DICT_SERVER_TO_CLIENT.to_le_bytes());
    assert_eq!(
        DatBitTreeTable::parse_redirect_did(&stub, DID_DICT_SERVER_TO_CLIENT_DEFERRED)
            .expect("parse redirect stub"),
        Some(DID_DICT_SERVER_TO_CLIENT)
    );
    assert_eq!(
        DatBitTreeTable::parse_redirect_did(&stub, DID_DICT_SERVER_TO_CLIENT).unwrap(),
        None
    );
    let mut literal = stub.clone();
    literal[32..36].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        DatBitTreeTable::parse_redirect_did(&literal, DID_DICT_CLIENT_TO_SERVER).unwrap(),
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

    // Full UDP datagram round-trip with wirePrefix != 0x0000 using DatBitTreeTable
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

    // 0x00000400 (bit 10) and 0x00800000 (bit 23) are undefined and rejected by Rule 3 (0xF08004FF)
    for bad_bit in [
        FLAG_UNDEFINED_10,
        FLAG_UNDEFINED_23,
        0x1000_0000,
        0x0000_0002,
    ] {
        let mut raw = [0u8; 20];
        raw[4..8].copy_from_slice(&bad_bit.to_be_bytes());
        assert!(matches!(
            DatagramHeader::from_wire_bytes(&raw),
            Err(SessionError::InvalidHeaderFlags { flags }) if flags == bad_bit
        ));
    }

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
            login_result: 0,
            detail_text: "Queueing".to_owned(),
            queue_position: 3,
            queue_total: 25,
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
    let mut conn = SessionConnection::new(0x0100, peer, 0.0);
    // Without a ConnectChallenge the incoming ISAAC stream is seeded with 0.
    let mask_seq_2 = IsaacCipher::from_seed(0).next_mask();
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
    conn.receive_datagram(&first, peer, 0.1, None)
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
            conn.receive_datagram(&replay, peer, 50.0, None)
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
    let mut dict = DatBitTreeTable::synthetic_dictionary([0x33; 16]);
    // Make the root's `0` branch null; the table still loads (§4.1.3 item 3).
    let root = dict.root_node_index;
    dict.nodes[root] = DatBitTreeNode::Branch {
        child0_1based: 0,
        child1_1based: 259,
    };
    let reloaded = DatBitTreeTable::from_dat_bytes(&dict.to_dat_bytes()).expect("null child loads");
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
    let codec = DatBitTreeTable::synthetic_dictionary([0x44; 16]);
    let mut conn = SessionConnection::new(0x0150, server, 0.0);
    conn.register_codec_uuid([0x44; 16]);
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
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
    conn.receive_datagram(&dgram, server, 0.1, Some(&codec))
        .unwrap();
    while conn.pop_outbound_datagram().is_some() {}
    // A sequenced data packet goes out compressed (wirePrefix = remoteRecId) ...
    conn.send_blob(2, 0, 0x10, &[0x00; 64], 0.2, Some(&codec))
        .unwrap();
    let data = conn.pop_outbound_datagram().unwrap();
    assert_eq!(u16::from_be_bytes([data[0], data[1]]), 0x0304);
    // ... while the State 2 ConnectAcceptHeader retransmit stays unencoded.
    conn.tick(1.0, Some(&codec)).unwrap();
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
    conn.receive_datagram(&err, server, 1.1, None).unwrap();
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
    let mut conn = SessionConnection::new(0x0150, server, 0.0);
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).unwrap(), 0.0)
        .unwrap();
    conn.tick(20.0, None).unwrap();
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
    let (routed, _) = mux.receive_from_peer(&dgram, server, 0.1, None).unwrap();
    assert_eq!(routed, id);
    let stranger = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 11), 9000);
    assert!(matches!(
        mux.receive_from_peer(&dgram, stranger, 0.1, None),
        Err(SessionError::UnknownRecipient { .. })
    ));
}
