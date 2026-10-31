// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic test vectors of `docs/specs/official-session.md` §10.2 (Vectors 1–8), byte
//! for byte, with the "must fail" variants the spec states (a whole-payload sum, swapped
//! seeds, recipients read from the cookie, sequence 1, the former high-bit mask flag, an
//! unreversed referral cookie, a borrowed recipient, sequence or mask, an address family
//! read at referral bytes 8–9).

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AuthConnection, BLOB_FLAG_DEFAULT_RECIPIENT, BlobFragmentHeader, CHAT_MAX_RECV_RATE,
    CodecUuids, ConnectionState, DatagramHeader, FLAG_CUMULATIVE_ACK, FlowControlHeader,
    IsaacCipher, MAX_VARIABLE_U32, OptionalHeaders, ReferralAuthHeader, SessionAuthHeader,
    SessionConnection, SessionError, SessionEvent, WireCodecs, WorldReferralHeader,
    combine_packet_checksum, compute_datagram_checksum, decode_datagram, decode_variable_u32,
    encode_datagram, encode_variable_u32, guid_fields_swapped, max_recv_rate_for_speed,
    verify_packet_checksum,
};

/// Parses a spec vector's hex text.
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9000);

// ---------------------------------------------------------------------------
// Vector 1: the checksum primitive (§4.4.1)
// ---------------------------------------------------------------------------

#[test]
fn vector_1_checksum_primitive() {
    // 1A: 8 bytes, two big-endian words and the length term 0x00080000.
    assert_eq!(
        compute_datagram_checksum(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
        0x0610_0A0C
    );
    // 1B: 6 bytes, one word, the tail 0xAABB0000 and the length term 0x00060000.
    assert_eq!(
        compute_datagram_checksum(&[0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB]),
        0xABC3_0304
    );
    // 1C: empty.
    assert_eq!(compute_datagram_checksum(&[]), 0);
}

// ---------------------------------------------------------------------------
// Vector 2: a cumulative-ack datagram (§4.4.2)
// ---------------------------------------------------------------------------

#[test]
fn vector_2_cumulative_ack_datagram() {
    let hdr = DatagramHeader {
        rec_id: 0x0102,
        pak_size: 4,
        flags: FLAG_CUMULATIVE_ACK,
        seq_num: 5,
        checksum: 0,
        client_word: 0x0000,
        peer_word: 0x03E8,
    };
    assert_eq!(hdr.header_checksum(), 0xBBF3_B4CE);
    let opt = OptionalHeaders {
        cumulative_ack: Some(0x0000_000A),
        ..OptionalHeaders::default()
    };
    let dgram = encode_datagram(hdr, &opt, &[], None, None).expect("encode");
    let expected =
        hex("00 00 01 02 00 04 00 00 40 00 00 00 00 05 BB F7 B4 D8 00 00 03 E8 00 00 00 0A");
    assert_eq!(dgram, expected);
    let decoded = decode_datagram(&expected, true, None, true).expect("decodes and verifies");
    assert_eq!(decoded.payload_sum, 0x0004_000A);
    assert_eq!(decoded.header.checksum, 0xBBF7_B4D8);
    assert_eq!(decoded.opt_headers.cumulative_ack, Some(10));
}

// ---------------------------------------------------------------------------
// Vector 3: transport compact integers (§5.6.1)
// ---------------------------------------------------------------------------

#[test]
fn vector_3_transport_compact_integers() {
    let cases: &[(u32, &[u8])] = &[
        (0x0000_0000, &[0x00]),
        (0x0000_007F, &[0x7F]),
        (0x0000_0080, &[0x80, 0x80]),
        (0x0000_0186, &[0x81, 0x86]),
        (0x0000_3FFF, &[0xBF, 0xFF]),
        (0x0000_4000, &[0xC0, 0x00, 0x00, 0x40]),
        (0x0102_0304, &[0xC1, 0x02, 0x04, 0x03]),
        (0x3000_001B, &[0xF0, 0x00, 0x1B, 0x00]),
    ];
    for &(value, bytes) in cases {
        let mut out = Vec::new();
        assert_eq!(encode_variable_u32(value, &mut out), Ok(bytes.len()));
        assert_eq!(out, bytes, "{value:#x}");
        assert_eq!(decode_variable_u32(bytes), Ok((value, bytes.len())));
    }
    // The tier follows from the first byte: 0xE0..=0xFF are four-byte first bytes, not a
    // five-byte escape.
    assert_eq!(
        decode_variable_u32(&[0xE0, 0x00, 0x1B, 0x00, 0x99]),
        Ok((0x2000_001B, 4))
    );
    assert_eq!(
        decode_variable_u32(&[0xFF, 0xFF, 0xFF, 0xFF]),
        Ok((MAX_VARIABLE_U32, 4))
    );
    // Decoder-only non-minimal reads: `80 01` and `C0 00 01 00` both decode to 1; the
    // writer emits `01`.
    assert_eq!(decode_variable_u32(&[0x80, 0x01]), Ok((1, 2)));
    assert_eq!(decode_variable_u32(&[0xC0, 0x00, 0x01, 0x00]), Ok((1, 4)));
    let mut minimal = Vec::new();
    assert_eq!(encode_variable_u32(1, &mut minimal), Ok(1));
    assert_eq!(minimal, [0x01]);
    // Truncated tiers.
    for short in [
        &[0x80u8][..],
        &[0xC0],
        &[0xC0, 0x00],
        &[0xC0, 0x00, 0x00],
        &[],
    ] {
        assert_eq!(
            decode_variable_u32(short),
            Err(SessionError::InvalidVariableU32)
        );
    }
    // A bounded sender refuses values beyond the table, writing nothing.
    let mut out = Vec::new();
    assert_eq!(
        encode_variable_u32(0x4000_0000, &mut out),
        Err(SessionError::VariableU32OutOfRange { value: 0x4000_0000 })
    );
    assert!(out.is_empty());
}

// ---------------------------------------------------------------------------
// Vector 4: a compact fragment header (§5.6.3)
// ---------------------------------------------------------------------------

#[test]
fn vector_4_compact_fragment_header() {
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
    assert_eq!(hdr.encode(&mut out), Ok(10));
    assert_eq!(out, hex("02 40 07 00 2A 00 81 86 10 00"));
    assert_eq!(BlobFragmentHeader::decode(&out), Ok((hdr, 10)));
}

// ---------------------------------------------------------------------------
// Vectors 5–7: the primary challenge, accept, first auth and first exchange (§6.2–§6.4.1)
// ---------------------------------------------------------------------------

/// Vector 5's dictionaries, on-disk UUIDs.
const IN_UUID: [u8; 16] = [
    0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0, 0x01,
];
const OUT_UUID: [u8; 16] = [
    0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x00, 0xAB, 0xCD,
];
const UUIDS: CodecUuids = CodecUuids {
    incoming: IN_UUID,
    outgoing: OUT_UUID,
};

const VECTOR_5_CHALLENGE: &str = "00 00 01 02 00 2C 00 04 00 00 00 00 00 00 95 B5 49 B9 33 44 11 22 40 8F 40 00 00 00 00 00 9A 71 C5 2E B8 06 D3 4F AA BB 03 04 11 22 33 44 55 66 77 88 40 30 20 10 60 50 80 70 90 A0 B0 C0 D0 E0 F0 01";
const VECTOR_5_ACCEPT: &str = "00 00 03 04 00 1C 00 08 00 00 00 00 00 00 E6 F8 1D E3 00 01 11 22 4F D3 06 B8 2E C5 71 9A AA BB CC DD 11 22 33 44 55 66 77 88 99 00 AB CD 00 00 00 00";
/// Vector 5's opaque challenge value (payload bytes 8–15).
const VECTOR_5_COOKIE: [u8; 8] = [0x9A, 0x71, 0xC5, 0x2E, 0xB8, 0x06, 0xD3, 0x4F];
const VECTOR_7_SERVER: &str = "00 00 01 02 00 19 01 00 00 06 00 00 00 02 B6 5D 79 93 33 44 11 22 40 8F 40 00 00 00 00 00 01 02 02 00 00 00 00 00 00 00 92 34 03 00 00 01 02";
const VECTOR_7_CLIENT: &str =
    "00 00 03 04 00 06 08 00 00 02 00 00 00 02 30 71 6F E2 00 02 11 22 71 00 00 00 F4 01";
const VECTOR_8_REFERRAL: &str = "9A 71 C5 2E B8 06 D3 4F C0 00 02 09 30 39 55 AA 66 00";
const VECTOR_8_REFERRAL_AUTH: &str =
    "00 00 00 00 00 08 00 02 00 00 00 00 00 00 39 93 E9 37 00 00 00 00 4F D3 06 B8 2E C5 71 9A";

/// A primary connection in State 1 with Vector 5's dictionaries.
fn started() -> SessionConnection {
    let mut conn = SessionConnection::new(SERVER, 0.0);
    conn.set_codec_uuids(UUIDS);
    let auth = SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10",
        "A",
        None,
        None,
        0,
    )
    .expect("auth");
    conn.start_login(auth, 0.0).expect("login");
    conn.pop_outbound_datagram().expect("SessionAuthHeader");
    conn
}

#[test]
fn vector_5_challenge_and_accept() {
    // The challenge parses as §6.3 lays it out.
    let challenge = hex(VECTOR_5_CHALLENGE);
    assert_eq!(challenge.len(), 66);
    let pkt = decode_datagram(&challenge, true, None, true).expect("the challenge verifies");
    assert_eq!(pkt.header.header_checksum(), 0xEF3B_822B);
    assert_eq!(pkt.payload_sum, 0xA679_C78E);
    assert_eq!(pkt.header.checksum, 0x95B5_49B9);
    assert_eq!(
        (
            pkt.header.rec_id,
            pkt.header.client_word,
            pkt.header.peer_word
        ),
        (0x0102, 0x3344, 0x1122)
    );
    let ch = pkt.opt_headers.connect_challenge.expect("challenge");
    assert_eq!(ch.clock_candidate, 1000.0);
    assert_eq!(ch.cookie, VECTOR_5_COOKIE);
    assert_eq!(ch.reserved_16, [0xAA, 0xBB]);
    assert_eq!(ch.outgoing_rec_id, 0x0304);
    assert_eq!(
        (ch.incoming_seed, ch.outgoing_seed),
        (0x1122_3344, 0x5566_7788)
    );
    assert_eq!(ch.incoming_codec_uuid(), IN_UUID);
    assert_eq!(ch.incoming_codec_guid, guid_fields_swapped(IN_UUID));
    // The same bytes come out of the encoder.
    assert_eq!(
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0102,
                client_word: 0x3344,
                peer_word: 0x1122,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                connect_challenge: Some(ch),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        ),
        Ok(challenge.clone())
    );

    // The connection answers with the accept, byte for byte.
    let mut conn = started();
    conn.receive_datagram(&challenge, SERVER, 0.1, WireCodecs::NONE)
        .expect("the challenge is taken");
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    // Recipients from the header and payload bytes 18..19; reading either from the
    // cookie must fail (neither is encoded there).
    assert_eq!(
        (conn.local_rec_id(), conn.server_rec_id()),
        (0x0102, 0x0304)
    );
    for pair in VECTOR_5_COOKIE.chunks(2) {
        for id in [
            u16::from_be_bytes([pair[0], pair[1]]),
            u16::from_le_bytes([pair[0], pair[1]]),
        ] {
            assert_ne!(conn.local_rec_id(), id);
            assert_ne!(conn.server_rec_id(), id);
        }
    }
    assert!(conn.codecs_negotiated());
    let accept = conn.pop_outbound_datagram().expect("accept");
    assert_eq!(accept, hex(VECTOR_5_ACCEPT));
    assert_eq!(accept.len(), 50);
    let parsed = decode_datagram(&accept, false, None, true).expect("the accept verifies");
    assert_eq!(parsed.header.header_checksum(), 0xBDFE_821B);
    assert_eq!(parsed.payload_sum, 0x28F9_9BC8);
    assert_eq!(parsed.header.checksum, 0xE6F8_1DE3);
    let resp = parsed.opt_headers.connect_accept.expect("accept");
    assert_eq!(resp.codec_uuid, OUT_UUID, "the outgoing on-disk UUID");
    assert_eq!(resp.max_recv_rate, 0);
    // Its retransmit is the same datagram (sequence zero, no mask word).
    conn.tick(0.8).expect("tick");
    assert_eq!(conn.pop_outbound_datagram(), Some(hex(VECTOR_5_ACCEPT)));

    // The receive-rate rule (§6.4): floor(ConnectionSpeed × 102.4), 0 by default; the chat
    // connection's historical 262,144.
    assert_eq!(max_recv_rate_for_speed(0.0), 0);
    assert_eq!(max_recv_rate_for_speed(1000.0), 102_400);
    assert_eq!(max_recv_rate_for_speed(1.5), 153);
    assert_eq!(max_recv_rate_for_speed(-3.0), 0);
    assert_eq!(max_recv_rate_for_speed(f64::NAN), 0);
    assert_eq!(CHAT_MAX_RECV_RATE, 262_144);
    let mut rated = SessionConnection::new(SERVER, 0.0);
    rated.set_codec_uuids(UUIDS);
    rated.set_max_recv_rate(max_recv_rate_for_speed(1000.0));
    rated
        .start_login(
            SessionAuthHeader::for_connection(AuthConnection::Primary, "x", "A", None, None, 0)
                .expect("auth"),
            0.0,
        )
        .expect("login");
    rated.pop_outbound_datagram();
    rated
        .receive_datagram(&challenge, SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    let accept = rated.pop_outbound_datagram().expect("accept");
    assert_eq!(&accept[22 + 24..22 + 28], &102_400u32.to_le_bytes());
}

/// §10.2 Vector 6: the synthetic primary first-auth datagram.
#[test]
fn vector_6_first_auth_datagram() {
    let ticket = "QUJD".repeat(64);
    let auth = SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10",
        "SYNTH01",
        Some(&ticket),
        None,
        0x6000_0000,
    )
    .expect("auth");
    let mut conn = SessionConnection::new(SERVER, 0.0);
    conn.start_login(auth, 12.5).expect("login");
    let first = conn.pop_outbound_datagram().expect("first datagram");
    let mut expected = hex(
        "00 00 00 00 01 65 00 01 00 00 00 00 00 00 D1 F4 B7 73 00 00 00 00 3F 31 38 30 38 30 34 5F \
         6E 65 74 76 65 72 3A 31 32 33 34 3B 20 64 69 64 76 65 72 3A 30 31 30 32 30 33 30 34 2D 30 \
         35 30 36 2D 30 37 30 38 2D 30 39 30 41 2D 30 42 30 43 30 44 30 45 30 46 31 30 21 01 00 00 \
         01 00 00 00 04 00 00 00 00 00 00 60 07 53 00 59 00 4E 00 54 00 48 00 30 00 31 00 02 01 00 \
         00 81 00",
    );
    expected.extend_from_slice(ticket.as_bytes());
    assert_eq!(expected.len(), 379);
    assert_eq!(first, expected);
    let parsed = decode_datagram(&first, false, None, true).expect("verifies");
    assert_eq!(parsed.header.pak_size, 357);
    assert_eq!(parsed.header.header_checksum(), 0xBAF2_7242);
    assert_eq!(parsed.payload_sum, 0x1702_4531);
    // One whole payload buffer: here the per-buffer and the whole sums agree.
    assert_eq!(compute_datagram_checksum(&parsed.raw_payload), 0x1702_4531);
    assert_eq!(parsed.header.checksum, 0xD1F4_B773);
    assert_eq!(
        parsed.header.seq_num, 0,
        "sequence zero, not the counter's 1"
    );
    // The State 1 retransmit is the same datagram.
    conn.tick(14.6).expect("tick");
    assert_eq!(conn.pop_outbound_datagram(), Some(expected));
}

#[test]
fn vector_7_first_masks() {
    // The first word of each directional generator, for sequence 2.
    assert_eq!(IsaacCipher::from_seed(0x1122_3344).next_mask(), 0x828E_2147);
    assert_eq!(IsaacCipher::from_seed(0x5566_7788).next_mask(), 0x0F7E_EDD9);
}

#[test]
fn vector_7_first_sequenced_exchange() {
    let server = hex(VECTOR_7_SERVER);
    assert_eq!(server.len(), 47);
    // Three checksum buffers: ClockSync, the 14-byte fragment header, the data.
    let pkt = decode_datagram(&server, true, None, false).expect("parses");
    assert_eq!(pkt.header.flags, 0x0100_0006);
    assert_eq!(pkt.header.header_checksum(), 0xF037_8220);
    assert_eq!(pkt.payload_sum, 0x44AB_D634);
    assert_eq!(pkt.opt_headers.clock_sync, Some(1000.0));
    let payload = &pkt.raw_payload;
    assert_eq!(
        pkt.payload_sum,
        compute_datagram_checksum(&payload[..8])
            .wrapping_add(compute_datagram_checksum(&payload[8..22]))
            .wrapping_add(compute_datagram_checksum(&payload[22..]))
    );
    verify_packet_checksum(pkt.header, pkt.payload_sum, Some(0x828E_2147))
        .expect("the incoming mask verifies");

    // The connection takes it after Vector 5: State 2 -> State 5, one blob.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    while conn.pop_outbound_datagram().is_some() {}
    conn.receive_datagram(&server, SERVER, 0.2, WireCodecs::NONE)
        .expect("the first server packet verifies");
    assert_eq!(conn.state(), ConnectionState::Connected);
    assert_eq!(conn.recv_seq_expected(), 2);
    let mut blobs = Vec::new();
    while let Some(ev) = conn.pop_event() {
        if let SessionEvent::BlobReceived(blob) = ev {
            blobs.push(blob);
        }
    }
    let [blob] = blobs.as_slice() else {
        panic!("{blobs:?}");
    };
    assert_eq!(
        (blob.queue_id, blob.recipient_id, blob.blob_header_word),
        (1, 2, 0x1234)
    );
    assert_eq!(blob.payload, [0x00, 0x01, 0x02]);
    assert!(
        conn.pop_outbound_datagram().is_none(),
        "no ClockSync on entry"
    );

    // The client's first sequenced packet, on the first tick after State 5: FlowControl
    // only, sequence 2, the first outgoing word, the client word 2 (§4.2 P10-22-POLICY)
    // and the copied peer word. The connection's own counters give 113 bytes (the 66 of
    // the challenge and the 47 of the server packet) and 500 ms since the auth went out
    // at 0.0 (§5.5 P10-22-POLICY).
    conn.tick(0.5).expect("tick");
    let client = conn.pop_outbound_datagram().expect("client packet");
    assert!(
        conn.pop_outbound_datagram().is_none(),
        "only the flow report"
    );
    assert_eq!(client, hex(VECTOR_7_CLIENT));
    assert_eq!(conn.send_seq_num(), 2);
    let parsed = decode_datagram(&client, false, None, false).expect("parses");
    assert_eq!(parsed.header.header_checksum(), 0xC5F7_8209);
    assert_eq!(parsed.payload_sum, 0x6507_0000);
    assert_eq!(parsed.header.checksum, 0x3071_6FE2);
    assert_eq!(
        parsed.opt_headers.flow_control,
        Some(FlowControlHeader {
            bytes_received_delta: 113,
            interval_ms: 500,
        })
    );
    // Nothing new arrived: the next tick sends no further report.
    conn.tick(0.6).expect("tick");
    assert!(conn.pop_outbound_datagram().is_none());
}

/// §10.2 Vector 7: "Reusing the incoming seed for the client, swapping the two seeds,
/// using sequence 1, using a whole-payload sum for the server, or using the former
/// high-bit mask flag must fail these fixtures."
#[test]
fn vector_7_must_fail_variants() {
    let server = hex(VECTOR_7_SERVER);
    let client = hex(VECTOR_7_CLIENT);
    let server_pkt = decode_datagram(&server, true, None, false).expect("parses");
    let client_pkt = decode_datagram(&client, false, None, false).expect("parses");
    let in_first = IsaacCipher::from_seed(0x1122_3344).next_mask();
    let out_first = IsaacCipher::from_seed(0x5566_7788).next_mask();
    // The right masks verify.
    assert!(
        verify_packet_checksum(server_pkt.header, server_pkt.payload_sum, Some(in_first)).is_ok()
    );
    assert!(
        verify_packet_checksum(client_pkt.header, client_pkt.payload_sum, Some(out_first)).is_ok()
    );
    // The incoming seed reused for the client, and the seeds swapped.
    assert!(
        verify_packet_checksum(client_pkt.header, client_pkt.payload_sum, Some(in_first)).is_err()
    );
    assert!(
        verify_packet_checksum(server_pkt.header, server_pkt.payload_sum, Some(out_first)).is_err()
    );
    // The second word (as if the accept had consumed the first).
    let mut second = IsaacCipher::from_seed(0x1122_3344);
    second.next_mask();
    assert!(
        verify_packet_checksum(
            server_pkt.header,
            server_pkt.payload_sum,
            Some(second.next_mask())
        )
        .is_err()
    );
    // Sequence 1 instead of 2.
    let mut seq_one = server_pkt.header;
    seq_one.seq_num = 1;
    assert!(verify_packet_checksum(seq_one, server_pkt.payload_sum, Some(in_first)).is_err());
    let mut client_seq_one = client_pkt.header;
    client_seq_one.seq_num = 1;
    assert!(
        verify_packet_checksum(client_seq_one, client_pkt.payload_sum, Some(out_first)).is_err()
    );
    // A whole-payload sum for the server packet.
    let whole = compute_datagram_checksum(&server_pkt.raw_payload);
    assert_ne!(whole, server_pkt.payload_sum);
    assert!(verify_packet_checksum(server_pkt.header, whole, Some(in_first)).is_err());
    // The former high-bit mask and blob flags (0x02000000, 0x04000000): now four- and
    // eight-byte optional fields, so the packet no longer parses or verifies.
    let mut high = server.clone();
    high[6..10].copy_from_slice(&0x0700_0000u32.to_be_bytes());
    let refused = match decode_datagram(&high, true, None, false) {
        Err(_) => true,
        Ok(p) => verify_packet_checksum(p.header, p.payload_sum, Some(in_first)).is_err(),
    };
    assert!(refused);

    // On a connection: the swapped seeds refuse the server packet before any commit.
    let mut conn = started();
    let mut swapped = hex(VECTOR_5_CHALLENGE);
    swapped[22 + 20..22 + 24].copy_from_slice(&0x5566_7788u32.to_be_bytes());
    swapped[22 + 24..22 + 28].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    let mut hdr = decode_datagram(&swapped, true, None, false)
        .expect("parses")
        .header;
    let sum = decode_datagram(&swapped, true, None, false)
        .expect("parses")
        .payload_sum;
    hdr.checksum = combine_packet_checksum(hdr, sum, None).expect("sum");
    swapped[2..22].copy_from_slice(&hdr.to_wire_bytes());
    conn.receive_datagram(&swapped, SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    assert!(matches!(
        conn.receive_datagram(&server, SERVER, 0.2, WireCodecs::NONE),
        Err(SessionError::ChecksumMismatch { .. })
    ));
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    assert_eq!(conn.recv_seq_expected(), 1);
}

/// Vector 7 with directional compression (§4.1.2, §6.4.1, §7.3), on synthetic
/// dictionaries: the server packet compressed with the incoming dictionary verifies, and the
/// client's first sequenced packet goes out compressed with the outgoing one, decoding to
/// Vector 7's bytes.
#[test]
fn vector_7_with_directional_compression() {
    use tessera_compat::session::{BitTreeCodec, BitTreeTable};
    let incoming = BitTreeTable::synthetic_dictionary(IN_UUID);
    let outgoing = BitTreeTable::synthetic_dictionary(OUT_UUID);
    let codecs = WireCodecs {
        incoming: Some(&incoming),
        outgoing: Some(&outgoing),
    };
    // The server packet, compressed: wire prefix = the client's incoming recipient.
    let plain = hex(VECTOR_7_SERVER);
    let mut compressed = 0x0102u16.to_be_bytes().to_vec();
    incoming
        .encode(&plain[4..], &mut compressed, 65_402)
        .expect("compress");
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, codecs)
        .expect("challenge");
    while conn.pop_outbound_datagram().is_some() {}
    conn.receive_datagram(&compressed, SERVER, 0.2, codecs)
        .expect("the compressed first server packet verifies");
    assert_eq!(conn.state(), ConnectionState::Connected);
    conn.tick_with_codec(0.5, codecs.outgoing).expect("tick");
    let sent = conn.pop_outbound_datagram().expect("client packet");
    assert_eq!(
        &sent[..2],
        &[0x03, 0x04],
        "wire prefix = the outgoing recipient"
    );
    let parsed = decode_datagram(&sent, false, Some(&outgoing), false).expect("decompresses");
    // The report counts the received bytes as they came: the compressed server packet.
    let delta = u32::try_from(66 + compressed.len()).expect("small");
    assert_eq!(
        parsed.opt_headers.flow_control,
        Some(FlowControlHeader {
            bytes_received_delta: delta,
            interval_ms: 500,
        })
    );
    let mut again = vec![0, 0];
    again.extend_from_slice(&parsed.header.to_wire_bytes());
    again.extend_from_slice(&parsed.raw_payload);
    let mut expected = hex(VECTOR_7_CLIENT);
    if delta == 113 {
        assert_eq!(again, expected);
    } else {
        // Vector 7 with this delta: the same header and the checksum over the new bytes.
        expected[22..26].copy_from_slice(&delta.to_le_bytes());
        assert_eq!(again[..14], expected[..14]);
        assert_eq!(again[18..], expected[18..]);
        verify_packet_checksum(
            parsed.header,
            parsed.payload_sum,
            Some(IsaacCipher::from_seed(0x5566_7788).next_mask()),
        )
        .expect("the first outgoing word");
    }
}

// ---------------------------------------------------------------------------
// Vector 8: a referral and its ReferralAuthHeader (§6.5)
// ---------------------------------------------------------------------------

#[test]
fn vector_8_referral_to_referral_auth() {
    let referral = hex(VECTOR_8_REFERRAL);
    let wr = WorldReferralHeader::decode(&referral).expect("18 bytes");
    assert_eq!(wr.cookie, VECTOR_5_COOKIE);
    assert_eq!(
        wr.target_addr,
        SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 12345)
    );
    assert_eq!(wr.metadata, [0x55, 0xAA, 0x66, 0x00]);
    let mut back = Vec::new();
    wr.encode(&mut back);
    assert_eq!(back, referral);
    // Bytes 8–9 read as an address family must fail: the port would then be `02 09`.
    assert_ne!(u16::from_be_bytes([referral[10], referral[11]]), 12345);
    assert_ne!(wr.target_addr.port(), 0x0209);

    // A connected login connection reports it.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), SERVER, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_event().is_some() {}
    let dgram = encode_datagram(
        DatagramHeader {
            rec_id: 0x0102,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            world_referral: Some(wr),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("referral datagram");
    conn.receive_datagram(&dgram, SERVER, 0.3, WireCodecs::NONE)
        .expect("referral");
    let event = std::iter::from_fn(|| conn.pop_event())
        .find(|e| matches!(e, SessionEvent::WorldReferralReceived { .. }));
    assert_eq!(
        event,
        Some(SessionEvent::WorldReferralReceived {
            cookie: VECTOR_5_COOKIE,
            world_addr: SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 12345),
        })
    );

    // The fresh world connection: recipient zero, length 8, only 0x00020000, sequence
    // zero, the final four bytes zero, the cookie reversed, nothing else.
    let world_addr = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 12345);
    let mut world = SessionConnection::new(world_addr, 0.3);
    world.set_codec_uuids(UUIDS);
    world
        .start_world_login(VECTOR_5_COOKIE, 0.3)
        .expect("referral auth");
    let sent = world.pop_outbound_datagram().expect("ReferralAuthHeader");
    assert_eq!(sent, hex(VECTOR_8_REFERRAL_AUTH));
    assert_eq!(sent.len(), 30);
    let parsed = decode_datagram(&sent, false, None, true).expect("verifies");
    assert_eq!(parsed.header.header_checksum(), 0xBAF3_70E5);
    assert_eq!(parsed.payload_sum, 0x7EA0_7852);
    assert_eq!(parsed.header.checksum, 0x3993_E937);
    assert_eq!(
        parsed.opt_headers.referral_auth,
        Some(ReferralAuthHeader::for_cookie(VECTOR_5_COOKIE))
    );
    // Its State 1 retransmit is the same datagram.
    world.tick(2.4).expect("tick");
    assert_eq!(world.pop_outbound_datagram(), Some(sent.clone()));

    // Must fail: the cookie echoed without reversal, and the login connection's recipient,
    // sequence or mask borrowed.
    let unreversed = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            referral_auth: Some(ReferralAuthHeader {
                reversed_cookie: VECTOR_5_COOKIE,
            }),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("encode");
    assert_ne!(unreversed, sent);
    let auth_only = OptionalHeaders {
        referral_auth: Some(ReferralAuthHeader::for_cookie(VECTOR_5_COOKIE)),
        ..OptionalHeaders::default()
    };
    for borrowed in [
        DatagramHeader {
            rec_id: 0x0304,
            ..DatagramHeader::default()
        },
        DatagramHeader {
            seq_num: 2,
            ..DatagramHeader::default()
        },
        DatagramHeader {
            client_word: 2,
            peer_word: 0x1122,
            ..DatagramHeader::default()
        },
    ] {
        assert_ne!(
            encode_datagram(borrowed, &auth_only, &[], None, None).expect("encode"),
            sent
        );
    }
    assert_ne!(
        encode_datagram(
            DatagramHeader::default(),
            &auth_only,
            &[],
            Some(IsaacCipher::from_seed(0x5566_7788).next_mask()),
            None
        )
        .expect("encode"),
        sent
    );

    // A referral with a zero address or port raises no event (OPEN-SESSION-2).
    for target in [
        SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 12345),
        SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 0),
    ] {
        let dgram = encode_datagram(
            DatagramHeader {
                rec_id: 0x0102,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                world_referral: Some(WorldReferralHeader {
                    target_addr: target,
                    ..wr
                }),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .expect("referral datagram");
        conn.receive_datagram(&dgram, SERVER, 0.4, WireCodecs::NONE)
            .expect("taken");
        assert!(
            !std::iter::from_fn(|| conn.pop_event())
                .any(|e| matches!(e, SessionEvent::WorldReferralReceived { .. }))
        );
    }
}
