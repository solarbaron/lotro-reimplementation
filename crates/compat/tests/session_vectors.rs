// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic test vectors of `docs/specs/official-session.md` §10.2 (Vectors 1–20), byte
//! for byte, with the "must fail" variants the spec states (a whole-payload sum, swapped
//! seeds, recipients read from the cookie, sequence 1, the former high-bit mask flag, an
//! unreversed referral cookie, a borrowed recipient, sequence or mask, an address family
//! read at referral bytes 8–9, an advanced Ack sequence or word, an extra base byte, an
//! unexchanged identifier, a token count, a missing terminal field), the `0xF5` fixture of
//! §9.3.3 and the empty `0x6C`.

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::initial::{
    AdvertisedArchive, InitialError, InstallRequest, IterationNote, IterationSpan,
    UnsupportedNegotiation, advertisement_order, build_install_advertisement,
    check_language_allowed, fold_language_code,
};
use tessera_compat::session::{
    AuthConnection, BLOB_FLAG_DEFAULT_RECIPIENT, BlobFragment, BlobFragmentHeader, BlobReassembler,
    CHAT_MAX_RECV_RATE, CodecUuids, ConnectionState, DatagramHeader, FLAG_CUMULATIVE_ACK,
    FLAG_HAS_FRAGMENTED_BLOBS, FlowControlHeader, IsaacCipher, MAX_VARIABLE_U32, OptionalHeaders,
    ReferralAuthHeader, SessionAuthHeader, SessionConnection, SessionError, SessionEvent,
    WireCodecs, WorldReferralHeader, check_fragment_slice, combine_packet_checksum,
    compute_datagram_checksum, decode_datagram, decode_message, decode_variable_u32,
    encode_datagram, encode_variable_u32, guid_fields_swapped, max_recv_rate_for_speed,
    parse_packet_payload, payload_checksum, split_blob, verify_packet_checksum,
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
const VECTOR_8_REFERRAL: &str = "D6 39 A0 7B 24 E1 58 92 C0 00 02 09 30 39 55 AA 66 00";
const VECTOR_8_REFERRAL_AUTH: &str =
    "00 00 00 00 00 08 00 02 00 00 00 00 00 00 C8 F4 8B DF 00 00 00 00 92 58 E1 24 7B A0 39 D6";
/// Vector 8's opaque synthetic cookie, unrelated to recipient IDs, seeds and Vector 5's.
const VECTOR_8_COOKIE: [u8; 8] = [0xD6, 0x39, 0xA0, 0x7B, 0x24, 0xE1, 0x58, 0x92];

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
    assert_eq!(wr.cookie, VECTOR_8_COOKIE);
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
            cookie: VECTOR_8_COOKIE,
            world_addr: SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 12345),
        })
    );

    // The fresh world connection: recipient zero, length 8, only 0x00020000, sequence
    // zero, the final four bytes zero, the cookie reversed, nothing else.
    let world_addr = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 9), 12345);
    let mut world = SessionConnection::new(world_addr, 0.3);
    world.set_codec_uuids(UUIDS);
    world
        .start_world_login(VECTOR_8_COOKIE, 0.3)
        .expect("referral auth");
    let sent = world.pop_outbound_datagram().expect("ReferralAuthHeader");
    assert_eq!(sent, hex(VECTOR_8_REFERRAL_AUTH));
    assert_eq!(sent.len(), 30);
    let parsed = decode_datagram(&sent, false, None, true).expect("verifies");
    assert_eq!(parsed.header.header_checksum(), 0xBAF3_70E5);
    assert_eq!(parsed.payload_sum, 0x0E01_1AFA);
    assert_eq!(parsed.header.checksum, 0xC8F4_8BDF);
    assert_eq!(
        parsed.opt_headers.referral_auth,
        Some(ReferralAuthHeader::for_cookie(VECTOR_8_COOKIE))
    );
    // Its State 1 retransmit is the same datagram.
    world.tick(2.4).expect("tick");
    assert_eq!(world.pop_outbound_datagram(), Some(sent.clone()));

    // Must fail: Vector 5's challenge cookie in place of the referral's.
    let mut borrowed_cookie = SessionConnection::new(world_addr, 0.3);
    borrowed_cookie.set_codec_uuids(UUIDS);
    borrowed_cookie
        .start_world_login(VECTOR_5_COOKIE, 0.3)
        .expect("referral auth");
    assert_ne!(borrowed_cookie.pop_outbound_datagram(), Some(sent.clone()));

    // Must fail: the cookie echoed without reversal, and the login connection's recipient,
    // sequence or mask borrowed.
    let unreversed = encode_datagram(
        DatagramHeader::default(),
        &OptionalHeaders {
            referral_auth: Some(ReferralAuthHeader {
                reversed_cookie: VECTOR_8_COOKIE,
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
        referral_auth: Some(ReferralAuthHeader::for_cookie(VECTOR_8_COOKIE)),
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
        // §9.3.9: judged where the selection is known; the session layer makes no connection.
        assert!(std::iter::from_fn(|| conn.pop_event()).any(|e| matches!(
            e,
            SessionEvent::WorldReferralReceived { world_addr, .. } if world_addr == target
        )));
    }
}

// ---------------------------------------------------------------------------
// Vector 9: a standalone Ack under the interim word policy (§4.2 P10-23-POLICY)
// ---------------------------------------------------------------------------

const VECTOR_9_ACK: &str =
    "00 00 12 34 00 04 00 00 40 00 00 00 00 02 CD 2C 07 5E 00 02 56 78 00 00 00 03";

#[test]
fn vector_9_standalone_ack() {
    let hdr = DatagramHeader {
        rec_id: 0x1234,
        seq_num: 2,
        client_word: 2,
        peer_word: 0x5678,
        ..DatagramHeader::default()
    };
    let opt = OptionalHeaders {
        cumulative_ack: Some(3),
        ..OptionalHeaders::default()
    };
    let dgram = encode_datagram(hdr, &opt, &[], None, None).expect("encodes");
    assert_eq!(dgram, hex(VECTOR_9_ACK));
    assert_eq!(dgram.len(), 26);
    let parsed = decode_datagram(&dgram, false, None, true).expect("verifies");
    assert_eq!(parsed.header.header_checksum(), 0xCD28_075B);
    assert_eq!(parsed.payload_sum, 0x0004_0003);
    assert_eq!(parsed.header.checksum, 0xCD2C_075E);

    // The connection's own Ack: after one sequenced packet (sequence 2, word 2) it repeats
    // the sequence and word, consuming no mask and advancing neither.
    let peer = SERVER;
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), peer, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), peer, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_outbound_datagram().is_some() {}
    conn.tick(0.3).expect("the flow-only packet");
    let flow = conn.pop_outbound_datagram().expect("flow");
    let flow = decode_datagram(&flow, false, None, false).expect("flow decodes");
    assert_eq!((flow.header.seq_num, flow.header.client_word), (2, 2));
    conn.tick(2.3).expect("the Ack");
    let mut ack = None;
    while let Some(d) = conn.pop_outbound_datagram() {
        let p = decode_datagram(&d, false, None, false).expect("decodes");
        if p.opt_headers.cumulative_ack.is_some() {
            ack = Some(p);
        }
    }
    let ack = ack.expect("a standalone Ack");
    // The Ack's own header: the outgoing sequence, not the acknowledged one.
    assert_eq!(ack.header.flags, FLAG_CUMULATIVE_ACK);
    assert_eq!((ack.header.seq_num, ack.header.client_word), (2, 2));
    assert_eq!(ack.opt_headers.cumulative_ack, Some(2));
    assert_eq!(conn.send_seq_num(), 2, "no sequence allocated");

    // The Ack drew no word of the outgoing generator: the next sequenced packet carries the
    // word right after the flow-only packet's.
    conn.send_blob(1, 0, 0x10, &[0], 2.4, None).expect("queued");
    let next = conn.pop_outbound_datagram().expect("the next packet");
    let next = decode_datagram(&next, false, None, false).expect("decodes");
    assert_eq!(next.header.seq_num, 3);
    let mut outgoing = IsaacCipher::from_seed(0x5566_7788);
    let flow_word = outgoing.next_mask();
    verify_packet_checksum(flow.header, flow.payload_sum, Some(flow_word)).expect("the first word");
    verify_packet_checksum(next.header, next.payload_sum, Some(outgoing.next_mask()))
        .expect("the second word: the Ack took none");

    // Must fail: an advanced sequence or word, or the acknowledged sequence as the sequence.
    for bad in [
        DatagramHeader { seq_num: 3, ..hdr },
        DatagramHeader {
            client_word: 3,
            ..hdr
        },
    ] {
        assert_ne!(
            encode_datagram(bad, &opt, &[], None, None).expect("encodes"),
            hex(VECTOR_9_ACK)
        );
    }
    // Must fail: drawing an ISAAC mask for the standalone Ack.
    assert_ne!(
        encode_datagram(
            hdr,
            &opt,
            &[],
            Some(IsaacCipher::from_seed(0x5566_7788).next_mask()),
            None
        )
        .expect("encodes"),
        hex(VECTOR_9_ACK)
    );
}

// ---------------------------------------------------------------------------
// Vector 10: `0xEC` serialization only (§9.3.2)
// ---------------------------------------------------------------------------

const VECTOR_10_DATA: &str = "00 05 00 00 10 01 00 00 00 01 00 00 00 09 00 00 00 00 00 02 01 03 00 00 00 FD FF FF FF 07 00 00 00 01 00 00 00 09 00 00 00 56 34 12 EE FF C0 AD 0B 00 00 00 00 00 00 00 00";
const VECTOR_LANGUAGE_KEY: u32 = 0x1000_0005;
const VECTOR_10_HEADER: &str = "01 40 80 EC 39 00";

fn vector_10_archive() -> AdvertisedArchive {
    AdvertisedArchive {
        resource_id: [0x00, 0x00, 0x02, 0x01, 0x09, 0x00, 0x00, 0x00],
        iteration_count: 3,
        spans: vec![IterationSpan { start: 7, len: 3 }],
        notes: vec![IterationNote {
            iteration: 9,
            opaque: 0x0BAD_C0FF_EE12_3456u64.to_le_bytes(),
        }],
    }
}

#[test]
fn vector_10_install_advertisement() {
    let data =
        build_install_advertisement(VECTOR_LANGUAGE_KEY, &[vector_10_archive()]).expect("builds");
    assert_eq!(data, hex(VECTOR_10_DATA));
    assert_eq!(data.len(), 57);
    // The offsets the spec lists.
    assert_eq!(data[0], 0, "one flags byte");
    assert_eq!(&data[1..5], &[0x05, 0x00, 0x00, 0x10], "language key");
    assert_eq!(&data[5..9], &1u32.to_le_bytes(), "status");
    assert_eq!(&data[9..13], &1u32.to_le_bytes(), "N");
    assert_eq!(&data[13..21], &[9, 0, 0, 0, 0, 0, 2, 1], "identifier");
    assert_eq!(&data[21..25], &3u32.to_le_bytes(), "expanded count");
    assert_eq!(&data[25..29], &(-3i32).to_le_bytes(), "run token");
    assert_eq!(&data[29..33], &7u32.to_le_bytes(), "start");
    assert_eq!(&data[33..37], &1u32.to_le_bytes(), "K");
    assert_eq!(&data[37..41], &9u32.to_le_bytes(), "trailing iteration");
    assert_eq!(
        &data[41..49],
        &0x0BAD_C0FF_EE12_3456u64.to_le_bytes(),
        "opaque"
    );
    assert_eq!(&data[49..53], &0u32.to_le_bytes(), "M");
    assert_eq!(&data[53..57], &0u32.to_le_bytes(), "terminal");

    // The small fixture's own fragment header.
    let frags = tessera_compat::session::split_blob(1, 0xFFFF, 0, 0, None, 0xEC, &data).unwrap();
    assert_eq!(frags.len(), 1);
    let mut header = Vec::new();
    frags[0].header.encode(&mut header).expect("encodes");
    assert_eq!(header, hex(VECTOR_10_HEADER));

    // Must fail: an unchanged half order and a token count instead of the expanded count.
    let mut unchanged = vector_10_archive();
    unchanged.resource_id = [0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01];
    assert_ne!(
        build_install_advertisement(VECTOR_LANGUAGE_KEY, &[unchanged]).expect("builds"),
        data
    );
    let mut token_count = vector_10_archive();
    token_count.iteration_count = 1;
    assert_eq!(
        build_install_advertisement(VECTOR_LANGUAGE_KEY, &[token_count]),
        Err(InitialError::UnusableIteration)
    );
    // An extra base byte and an omitted terminal field are not what the builder writes.
    assert_ne!(data[1], 0);
    assert_eq!(data.len(), 53 + 4);
}

// ---------------------------------------------------------------------------
// Vector 11: a synthetic `0xDD` and the installed language rule (§9.3.4)
// ---------------------------------------------------------------------------

const VECTOR_11_DATA: &str = "00 04 03 02 01 07 00 00 00 00 00 00 00";

#[test]
fn vector_11_install_request_and_rule() {
    let data = hex(VECTOR_11_DATA);
    assert_eq!(data.len(), 13);
    let dd = InstallRequest::from_data(&data).expect("decodes");
    assert_eq!(dd.server_config, 0x0102_0304);
    assert_eq!(dd.name_rule_key, 7);
    assert!(dd.trailing.is_empty());
    // The same fields from what `decode_message` leaves.
    assert_eq!(InstallRequest::from_envelope_body(&data[1..]), Ok(dd));

    // Rule 7 with the full key passes; an empty list or another key fails and suppresses `0xEC`.
    const FULL: u32 = VECTOR_LANGUAGE_KEY;
    assert_eq!(check_language_allowed(&[FULL], FULL), Ok(()));
    assert_eq!(
        check_language_allowed(&[], FULL),
        Err(UnsupportedNegotiation::NameRuleEmpty)
    );
    assert_eq!(
        check_language_allowed(&[FULL], 0x1000_0006),
        Err(UnsupportedNegotiation::LanguageNotAllowed)
    );
    // The fold and the low half are not the full key, in the list or as the scalar.
    assert_eq!(fold_language_code(FULL), 0x1005);
    for not_the_key in [0x1005, 0x0005] {
        assert_eq!(
            check_language_allowed(&[FULL], not_the_key),
            Err(UnsupportedNegotiation::LanguageNotAllowed)
        );
        assert_ne!(
            build_install_advertisement(not_the_key, &[vector_10_archive()])
                .expect("builds")
                .get(1..5),
            Some(&[0x05, 0x00, 0x00, 0x10][..])
        );
    }

    // N = 1 without its four bytes: truncated. Four bytes more with N = 0: trailing bytes.
    let mut n_one = data.clone();
    n_one[9] = 1;
    assert_eq!(
        InstallRequest::from_data(&n_one),
        Err(InitialError::TrailingCount { count: 1 })
    );
    let mut extra = data.clone();
    extra.extend_from_slice(&[1, 0, 0, 0]);
    assert_eq!(
        InstallRequest::from_data(&extra),
        Err(InitialError::TrailingBytes(4))
    );
    // A well-formed non-empty array decodes (outside `P10-10E`'s send policy).
    let mut array = n_one.clone();
    array.extend_from_slice(&[9, 0, 0, 0]);
    assert_eq!(
        InstallRequest::from_data(&array).expect("decodes").trailing,
        [9]
    );
    // The count is checked against the bytes left and the cap before anything else.
    let mut huge = data.clone();
    huge[9..13].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        InstallRequest::from_data(&huge),
        Err(InitialError::TrailingCount { .. })
    ));
    let mut over_cap = data.clone();
    over_cap[9..13].copy_from_slice(&4097u32.to_le_bytes());
    over_cap.resize(13 + 4097 * 4, 0);
    assert!(matches!(
        InstallRequest::from_data(&over_cap),
        Err(InitialError::TrailingCount { count: 4097 })
    ));
    // N = 4,096 with exactly its bytes decodes, and fails only the send policy's empty array.
    let mut at_cap = data.clone();
    at_cap[9..13].copy_from_slice(&4096u32.to_le_bytes());
    at_cap.resize(13 + 4096 * 4, 0);
    assert_eq!(
        InstallRequest::from_data(&at_cap)
            .expect("decodes")
            .trailing
            .len(),
        4096
    );
    // Flags 0x01 with no valid stream and 0x02 up, then truncation at every length.
    let mut flags = data.clone();
    flags[0] = 1;
    assert_eq!(
        InstallRequest::from_data(&flags),
        Err(InitialError::Decompression)
    );
    for bad in [0x02u8, 0x04, 0xFF] {
        flags[0] = bad;
        assert_eq!(
            InstallRequest::from_data(&flags),
            Err(InitialError::UnsupportedFlags(bad))
        );
    }
    // Flags 0x01: a stored-block stream of the same fields, measured after it is inflated.
    let compressed =
        hex("01 0C 00 00 00 78 01 01 0C 00 F3 FF 04 03 02 01 07 00 00 00 00 00 00 00 00 B2 00 12");
    let inflated = InstallRequest::from_data(&compressed).expect("inflates");
    assert_eq!(inflated, InstallRequest::from_data(&data).expect("decodes"));
    let mut wrong_length = compressed.clone();
    wrong_length[1] = 0x0B;
    assert_eq!(
        InstallRequest::from_data(&wrong_length),
        Err(InitialError::Decompression)
    );
    // Any byte after the zlib stream fails, and so does a second stream after it.
    let mut trailing = compressed.clone();
    trailing.push(0x00);
    assert_eq!(
        InstallRequest::from_data(&trailing),
        Err(InitialError::Decompression)
    );
    let mut concatenated = compressed.clone();
    concatenated.extend_from_slice(&compressed[5..]);
    assert_eq!(
        InstallRequest::from_data(&concatenated),
        Err(InitialError::Decompression)
    );
    // Measuring `13 + 4N` on the compressed buffer is not what passes: the rebuilt body is 13.
    assert_eq!(inflated.trailing, Vec::<u32>::new());
    // `N` = 4,096 with its bytes passes the decoder and `N` = 4,097 fails the count bound.
    let mut at_cap = hex("00 04 03 02 01 07 00 00 00 00 10 00 00");
    at_cap.extend_from_slice(&[0u8; 16_384]);
    assert_eq!(
        InstallRequest::from_data(&at_cap)
            .expect("at the cap")
            .trailing
            .len(),
        4096
    );
    let mut over = hex("00 04 03 02 01 07 00 00 00 01 10 00 00");
    over.extend_from_slice(&[0u8; 16_388]);
    assert_eq!(
        InstallRequest::from_data(&over),
        Err(InitialError::TrailingCount { count: 4097 })
    );
    for len in 0..data.len() {
        assert!(InstallRequest::from_data(&data[..len]).is_err(), "{len}");
    }
}

// ---------------------------------------------------------------------------
// Vector 12: the character-selection boundary and the empty list (§9.3.5)
// ---------------------------------------------------------------------------

const VECTOR_12_DATA: &str = "00 77 00 00 00 00 00 00 01 00";
const VECTOR_12_HEADER: &str = "02 40 01 00 01 00 6F 0A";
const VECTOR_12_EMPTY_LIST: &str = "00 00 00 00 00 00";

/// A property schema with no property: the empty tables of these vectors need none.
struct NoProperties;

impl tessera_compat::world::PropertySchemaLookup for NoProperties {
    fn property_type(&self, _pid: u32) -> Option<tessera_compat::world::PropertyType> {
        None
    }
}

#[test]
fn vector_12_character_selection_and_empty_list() {
    use tessera_compat::world::{CharacterEnterWorldRequest, CharacterListResponse};
    let data = CharacterEnterWorldRequest::new(0x0100_0000_0000_0077)
        .to_envelope_bytes()
        .expect("encodes");
    assert_eq!(data, hex(VECTOR_12_DATA));
    assert_eq!(data.len(), 10);
    let frags = tessera_compat::session::split_blob(2, 1, 1, 0, None, 0x6F, &data).unwrap();
    assert_eq!(frags.len(), 1);
    let mut header = Vec::new();
    frags[0].header.encode(&mut header).expect("encodes");
    assert_eq!(header, hex(VECTOR_12_HEADER));
    // A second leading zero must fail the exact-length/ID comparison.
    let mut second = vec![0u8];
    second.extend_from_slice(&data);
    assert!(CharacterEnterWorldRequest::from_bytes(&second, &NoProperties).is_err());

    // The first queue-2 blob of a connection has order 1 and blob ID 1, on the driver's path.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), SERVER, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_outbound_datagram().is_some() {}
    conn.send_blob(2, 0, 0x6F, &data, 0.3, None).expect("sends");
    let mut last = None;
    while let Some(d) = conn.pop_outbound_datagram() {
        last = Some(d);
    }
    let pkt = decode_datagram(&last.expect("the blob"), false, None, false).expect("decodes");
    let mut wire = Vec::new();
    pkt.fragments[0].header.encode(&mut wire).expect("encodes");
    assert_eq!(wire, hex(VECTOR_12_HEADER));
    assert_eq!(pkt.fragments[0].payload, data);

    // The synthetic empty list: flags, u32 count zero, empty table.
    let empty = hex(VECTOR_12_EMPTY_LIST);
    let list = CharacterListResponse::from_bytes(&empty, &NoProperties).expect("decodes");
    assert!(list.characters.is_empty());
    assert_eq!(list.to_bytes().expect("encodes"), empty);
    let body = CharacterListResponse::from_envelope_body(&empty[1..], &NoProperties);
    assert_eq!(body.expect("decodes"), list);
}

// ---------------------------------------------------------------------------
// Vector 13: a standalone NAK under P10-23-POLICY (§9.3.7)
// ---------------------------------------------------------------------------

const VECTOR_13_NAK: &str =
    "00 00 12 34 00 08 00 00 10 00 00 00 00 02 D1 2F D7 5F 00 02 56 78 01 00 00 00 03 00 00 00";

#[test]
fn vector_13_standalone_nak() {
    let hdr = DatagramHeader {
        rec_id: 0x1234,
        seq_num: 2,
        client_word: 2,
        peer_word: 0x5678,
        ..DatagramHeader::default()
    };
    let opt = OptionalHeaders {
        sequence_nak: Some(vec![3]),
        ..OptionalHeaders::default()
    };
    let dgram = encode_datagram(hdr, &opt, &[], None, None).expect("encodes");
    assert_eq!(dgram, hex(VECTOR_13_NAK));
    assert_eq!(dgram.len(), 30);
    let parsed = decode_datagram(&dgram, false, None, true).expect("verifies");
    assert_eq!(parsed.header.flags, 0x0000_1000);
    assert_eq!(parsed.header.header_checksum(), 0xCD27_D75F);
    assert_eq!(parsed.payload_sum, 0x0408_0000);
    assert_eq!(parsed.header.checksum, 0xD12F_D75F);
    assert_eq!(&dgram[22..], [1, 0, 0, 0, 3, 0, 0, 0], "little-endian");

    // The connection's own NAK repeats the last allocated sequence and word and advances neither.
    // A packet of the server that skips a sequence number leaves that one missing.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), SERVER, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_outbound_datagram().is_some() {}
    conn.tick(0.3).expect("the flow-only packet");
    while conn.pop_outbound_datagram().is_some() {}
    // The server's generator past its first packet and the sequence number it skips.
    let mut server = IsaacCipher::from_seed(0x1122_3344);
    let _ = server.next_mask();
    let _ = server.next_mask();
    let later = encode_datagram(
        DatagramHeader {
            rec_id: conn.local_rec_id(),
            seq_num: 4,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &[],
        Some(server.next_mask()),
        None,
    )
    .expect("encodes");
    conn.receive_datagram(&later, SERVER, 0.4, WireCodecs::NONE)
        .expect("the later packet");
    while conn.pop_outbound_datagram().is_some() {}
    conn.tick(1.1).expect("the NAK");
    let mut nak = None;
    while let Some(d) = conn.pop_outbound_datagram() {
        let p = decode_datagram(&d, false, None, false).expect("decodes");
        if p.opt_headers.sequence_nak.is_some() {
            nak = Some(p);
        }
    }
    let nak = nak.expect("a standalone NAK");
    assert_eq!(nak.opt_headers.sequence_nak, Some(vec![3]));
    // The later packet's flow report waits for its report tick period (§5.5,
    // FLOW_REPORT_ONLY_PERIOD_SEC), so the NAK copies the flow-only packet's sequence 2 and
    // word 2, as in the vector, and takes none.
    assert_eq!((nak.header.seq_num, nak.header.client_word), (2, 2));
    assert_eq!(
        nak.header.flags & tessera_compat::session::FLAG_MASKED_CHECKSUM,
        0
    );
    assert_eq!(conn.send_seq_num(), 2, "no sequence allocated");

    // Must fail: an advanced sequence or word, drawing a mask, or a big-endian count and sequence.
    for bad in [
        DatagramHeader { seq_num: 3, ..hdr },
        DatagramHeader {
            client_word: 3,
            ..hdr
        },
    ] {
        assert_ne!(
            encode_datagram(bad, &opt, &[], None, None).expect("encodes"),
            hex(VECTOR_13_NAK)
        );
    }
    assert_ne!(
        encode_datagram(
            hdr,
            &opt,
            &[],
            Some(IsaacCipher::from_seed(0x5566_7788).next_mask()),
            None
        )
        .expect("encodes"),
        hex(VECTOR_13_NAK)
    );
    let mut big_endian = hex(VECTOR_13_NAK);
    big_endian[22..26].copy_from_slice(&1u32.to_be_bytes());
    big_endian[26..30].copy_from_slice(&3u32.to_be_bytes());
    assert!(decode_datagram(&big_endian, false, None, true).is_err());
}

// ---------------------------------------------------------------------------
// Vector 14: `0xEC` ordering, runs and two-fragment framing (§9.3.7)
// ---------------------------------------------------------------------------

const VECTOR_14_DATA: &str = "00 05 00 00 10 01 00 00 00 06 00 00 00 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 1A 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 02 02 00 00 00 00 00 00 00 00 14 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00 02 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

/// An archive with the given eight wire bytes and an empty iteration list.
fn wire_archive(wire: [u8; 8]) -> AdvertisedArchive {
    let [e, f, g, h, a, b, c, d] = wire;
    AdvertisedArchive {
        resource_id: [a, b, c, d, e, f, g, h],
        iteration_count: 0,
        spans: Vec::new(),
        notes: Vec::new(),
    }
}

#[test]
fn vector_14_order_and_empty_lists() {
    let entries = [
        wire_archive([0x09, 0, 0, 0, 0, 0, 0x02, 0x01]),
        wire_archive([0x02, 0, 0, 0, 0, 0, 0x02, 0x02]),
        wire_archive([0x03, 0, 0, 0, 0, 0, 0, 0]),
        wire_archive([0x1A, 0, 0, 0, 0, 0, 0, 0]),
        wire_archive([0x00, 0x01, 0, 0, 0, 0, 0, 0]),
        wire_archive([0x14, 0, 0, 0, 0, 0, 0, 0]),
    ];
    let expected = hex(VECTOR_14_DATA);
    assert_eq!(expected.len(), 117);
    // The result holds for EVERY input order: all 720 permutations of the six.
    let mut order = [0usize, 1, 2, 3, 4, 5];
    let mut seen = 0;
    permute(&mut order, 0, &mut |perm| {
        seen += 1;
        let archives: Vec<AdvertisedArchive> = perm.iter().map(|i| entries[*i].clone()).collect();
        let data = build_install_advertisement(VECTOR_LANGUAGE_KEY, &archives).expect("builds");
        assert_eq!(data, expected, "{perm:?}");
    });
    assert_eq!(seen, 720);
    // The fixture's remainders.
    for (archive, want) in entries.iter().zip([20, 15, 3, 3, 3, 20]) {
        assert_eq!(advertisement_order(archive.wire_identifier()).0, want);
    }
    // Its unfragmented default-recipient header.
    let frags =
        tessera_compat::session::split_blob(1, 0xFFFF, 0, 0, None, 0xEC, &expected).unwrap();
    let mut header = Vec::new();
    frags[0].header.encode(&mut header).expect("encodes");
    assert_eq!(header, hex("01 40 80 EC 75 00"));
    // C, D, E, B, F, A: none of the spec's counterexamples.
    let ids = |data: &[u8]| -> Vec<[u8; 2]> {
        (0..6)
            .map(|i| [data[13 + i * 16], data[14 + i * 16]])
            .collect()
    };
    assert_eq!(
        ids(&expected),
        [
            [0x03, 0],
            [0x1A, 0],
            [0x00, 0x01],
            [0x02, 0],
            [0x14, 0],
            [0x09, 0]
        ]
    );
}

/// Calls `f` with every permutation of `items[from..]`.
fn permute(items: &mut [usize; 6], from: usize, f: &mut dyn FnMut(&[usize; 6])) {
    if from == items.len() {
        f(items);
        return;
    }
    for i in from..items.len() {
        items.swap(from, i);
        permute(items, from + 1, f);
        items.swap(from, i);
    }
}

/// `official-session.md` Vector 14's run-threshold fixture, in EC bytes after the identifier.
const VECTOR_14_TOKENS: &str =
    "06 00 00 00 FD FF FF FF 07 00 00 00 0D 00 00 00 0E 00 00 00 14 00 00 00";
const VECTOR_14_TRAILING: &str = "01 00 00 00 14 00 00 00 56 34 12 EE FF C0 AD 0B";

fn run_archive(spans: Vec<IterationSpan>, notes: Vec<IterationNote>) -> AdvertisedArchive {
    AdvertisedArchive {
        resource_id: [0x00, 0x00, 0x02, 0x01, 0x09, 0x00, 0x00, 0x00],
        iteration_count: 6,
        spans,
        notes,
    }
}

fn single(start: i32) -> IterationSpan {
    IterationSpan { start, len: 1 }
}

#[test]
fn vector_14_run_threshold() {
    let note = |iteration: u32| IterationNote {
        iteration,
        opaque: 0x0BAD_C0FF_EE12_3456u64.to_le_bytes(),
    };
    let entry_tail = |archive: AdvertisedArchive| {
        let data = build_install_advertisement(VECTOR_LANGUAGE_KEY, &[archive]).expect("builds");
        data[21..data.len() - 8].to_vec()
    };
    let mut want = hex(VECTOR_14_TOKENS);
    want.extend_from_slice(&hex(VECTOR_14_TRAILING));

    // However the record stored them: as singles, as a stored two-value run, or with a run of one.
    let singles = run_archive(
        [7, 8, 9, 13, 14, 20].into_iter().map(single).collect(),
        vec![note(20)],
    );
    assert_eq!(entry_tail(singles), want);
    let stored = run_archive(
        vec![
            IterationSpan { start: 7, len: 3 },
            IterationSpan { start: 13, len: 2 },
            IterationSpan { start: 20, len: 1 },
        ],
        vec![note(20)],
    );
    assert_eq!(entry_tail(stored.clone()), want);

    // A trailing iteration below the highest listed value, or a trailing list not in increasing
    // order, fails the invariant, with no repair.
    let mut low = stored.clone();
    low.notes = vec![note(14)];
    assert_eq!(low.check_invariant(), Err(InitialError::UnusableIteration));
    assert_eq!(
        build_install_advertisement(VECTOR_LANGUAGE_KEY, &[low]),
        Err(InitialError::UnusableIteration)
    );
    let mut swapped = stored.clone();
    swapped.notes = vec![note(20), note(14)];
    assert_eq!(
        swapped.check_invariant(),
        Err(InitialError::UnusableIteration)
    );
    let mut two = stored.clone();
    two.notes = vec![note(14), note(20)];
    assert_eq!(two.check_invariant(), Ok(()));
    let mut too_many = run_archive(Vec::new(), vec![note(1)]);
    too_many.iteration_count = 0;
    assert_eq!(
        too_many.check_invariant(),
        Err(InitialError::UnusableIteration)
    );
    // §9.3.7: a list that repeats an iteration is malformed under Tessera's input policy.
    let repeated = run_archive(
        vec![
            IterationSpan { start: 7, len: 3 },
            IterationSpan { start: 9, len: 3 },
        ],
        Vec::new(),
    );
    assert_eq!(
        repeated.check_invariant(),
        Err(InitialError::UnusableIteration)
    );

    // Listed 7/8/9 with `K` = 2: trailing 8/9 passes; 9/9, 9/8 and 7/9 fail (install-layout §9.3).
    let listed = |notes: Vec<IterationNote>| {
        let mut a = run_archive(vec![IterationSpan { start: 7, len: 3 }], notes);
        a.iteration_count = 3;
        a
    };
    assert_eq!(listed(vec![note(8), note(9)]).check_invariant(), Ok(()));
    for bad in [[9, 9], [9, 8], [7, 9]] {
        assert_eq!(
            listed(vec![note(bad[0]), note(bad[1])]).check_invariant(),
            Err(InitialError::UnusableIteration),
            "{bad:?}"
        );
    }
    // A stored run of two or of one is valid input and goes out as singles.
    for (stored, want) in [
        (
            IterationSpan { start: 13, len: 2 },
            "0D 00 00 00 0E 00 00 00",
        ),
        (IterationSpan { start: 20, len: 1 }, "14 00 00 00"),
    ] {
        let mut a = run_archive(vec![stored], Vec::new());
        a.iteration_count = stored.len;
        let mut tokens = stored.len.to_le_bytes().to_vec();
        tokens.extend_from_slice(&hex(want));
        tokens.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(entry_tail(a), tokens);
    }

    // A stamp-1 record with those runs and no trailing list: the same `N` and tokens, an explicit
    // zero `K`.
    let mut stamp_1 = stored;
    stamp_1.notes = Vec::new();
    let mut want = hex(VECTOR_14_TOKENS);
    want.extend_from_slice(&[0, 0, 0, 0]);
    assert_eq!(entry_tail(stamp_1), want);
}

#[test]
fn vector_14_two_fragments() {
    let archive = AdvertisedArchive {
        resource_id: [0x00, 0x00, 0x02, 0x01, 0x09, 0x00, 0x00, 0x00],
        iteration_count: 110,
        spans: (0..110).map(|i| single(2 * i + 1)).collect(),
        notes: Vec::new(),
    };
    let data = build_install_advertisement(VECTOR_LANGUAGE_KEY, &[archive]).expect("builds");
    assert_eq!(data.len(), 477);
    const RECIPIENT: u64 = 0x0A0B_0C0D_0E0F_1112;
    let frags =
        tessera_compat::session::split_blob(1, 0xFFFF, 1, 0, Some(RECIPIENT), 0xEC, &data).unwrap();
    assert_eq!(frags.len(), 2);
    let header = |i: usize| {
        let mut out = Vec::new();
        frags[i].header.encode(&mut out).expect("encodes");
        out
    };
    assert_eq!(
        header(0),
        hex("01 10 01 00 00 02 12 11 0F 0E 0D 0C 0B 0A 80 EC 81 B4")
    );
    assert_eq!(
        header(1),
        hex("01 10 01 00 01 02 12 11 0F 0E 0D 0C 0B 0A 29 00")
    );
    assert_eq!(frags[0].payload.len(), 436);
    assert_eq!(frags[1].payload.len(), 41);

    // Reassembly in either order gives the same data.
    for order in [[0usize, 1], [1, 0]] {
        let mut r = tessera_compat::session::BlobReassembler::default();
        let mut done = None;
        for i in order {
            done = done.or(r.ingest(0x100, frags[i].clone(), 0.0).unwrap());
        }
        assert_eq!(done.expect("complete").payload, data);
    }

    // The connection's own send with that recipient: the same two headers, in ascending order.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), SERVER, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_outbound_datagram().is_some() {}
    conn.send_blob_retained(1, 0, RECIPIENT, 0xEC, &data, 0.3, None)
        .expect("sends");
    let mut own = Vec::new();
    while let Some(d) = conn.pop_outbound_datagram() {
        let p = decode_datagram(&d, false, None, false).expect("decodes");
        own.extend(p.fragments);
    }
    assert_eq!(own, frags);
}

// ---------------------------------------------------------------------------
// Vector 15: one-prefix application boundaries (§9.3.7)
// ---------------------------------------------------------------------------

#[test]
fn vector_15_one_prefix_boundaries() {
    use tessera_compat::world::{
        AccountEvictionNotice, CharacterOperationError, PlatformAccountIdReport, PlayerSessionReady,
    };
    // `0x19E`: shared flags and the ID.
    let platform = hex("00 88 77 66 55 44 33 22 11");
    let report = PlatformAccountIdReport::from_bytes(&platform).expect("decodes");
    assert_eq!(report.platform_id, 0x1122_3344_5566_7788);
    assert_eq!(report.to_bytes().expect("encodes"), platform);
    let mut second = vec![0u8];
    second.extend_from_slice(&platform);
    assert!(PlatformAccountIdReport::from_bytes(&second).is_err());

    // `0x90`: shared flags, the ID and an empty table.
    let ready = hex("00 77 00 00 00 00 00 00 01 00");
    let session = PlayerSessionReady::from_bytes(&ready, &NoProperties).expect("decodes");
    assert_eq!(session.player_id, 0x0100_0000_0000_0077);
    assert_eq!(
        PlayerSessionReady::from_envelope_body(&ready[1..], &NoProperties).expect("decodes"),
        session
    );
    // A second leading byte shifts the ID.
    let mut second = vec![0u8];
    second.extend_from_slice(&ready);
    assert!(PlayerSessionReady::from_bytes(&second, &NoProperties).is_err());
    assert!(PlayerSessionReady::from_envelope_body(&ready, &NoProperties).is_err());

    // `0x6D`: shared flags and the reason.
    let refused = hex("00 04 03 02 01");
    assert_eq!(
        CharacterOperationError::from_bytes(&refused)
            .expect("decodes")
            .error_code,
        0x0102_0304
    );
    assert_eq!(
        CharacterOperationError::from_envelope_body(&refused[1..])
            .expect("decodes")
            .error_code,
        0x0102_0304
    );
    assert!(CharacterOperationError::from_envelope_body(&refused).is_err());

    // `0xB3`: the checked prefix only; what follows is not read.
    assert_eq!(
        AccountEvictionNotice::reason_of_envelope_body(&[4, 3, 2, 1, 0xFF, 0xFF]),
        Some(0x0102_0304)
    );
    assert_eq!(
        AccountEvictionNotice::reason_of_envelope_body(&[4, 3, 2]),
        None
    );
    // §9.3.9's structural tests, on the complete data: `00` and at least four reason bytes.
    let valid = hex("00 01 00 00 00");
    let body = decode_message(0xB3, &valid, 64).expect("plain").body;
    assert_eq!(
        AccountEvictionNotice::reason_of_envelope_body(&body),
        Some(1)
    );
    // Short, and a nonzero prefix.
    let short = decode_message(0xB3, &hex("00 01 00 00"), 64)
        .expect("plain")
        .body;
    assert_eq!(AccountEvictionNotice::reason_of_envelope_body(&short), None);
    assert!(decode_message(0xB3, &hex("01 01 00 00 00"), 64).is_err());
}

// ---------------------------------------------------------------------------
// The `0xF5` fixture (§9.3.3)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Vector 16: the corrected appearance and a nonempty list (§9.3.8, world §2.8, §3.1.2)
// ---------------------------------------------------------------------------

const VECTOR_16_APPEARANCE: &str = "00 40 07 00 00 00 00 C0 3F 00 00 80 3E 20 00 00 00 01 00 00 00 03 00 00 00 01 01 DF 9B 57 13 E0 AC 68 24 01 00";
const VECTOR_16_LIST: &str = "00 01 00 00 00 77 00 00 00 00 00 00 01 07 46 00 69 00 78 00 74 00 75 00 72 00 65 00 00 00 00 00 00 00 00 00 00 00 40 07 00 00 00 00 C0 3F 00 00 80 3E 20 00 00 00 01 00 00 00 03 00 00 00 01 01 DF 9B 57 13 E0 AC 68 24 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

#[test]
fn vector_16_appearance_and_list() {
    use tessera_compat::world::{
        CharacterListResponse, EntityAppearanceDesc, VisualDescriptionUpdate, WireReader,
        WireWriter, WorldError,
    };
    let data = hex(VECTOR_16_APPEARANCE);
    assert_eq!(data.len(), 37);
    let update = VisualDescriptionUpdate::from_bytes(&data).expect("decodes");
    let desc = &update.visual_desc;
    assert_eq!(desc.flags(), 0x0740);
    assert_eq!(desc.param_a, Some(1.5));
    assert_eq!(desc.param_b, Some(0.25));
    let part = desc.primary_part.as_ref().expect("the part");
    let map = part.sub_map.as_ref().expect("the sub-map");
    assert_eq!(map[0].0, 3);
    assert_eq!(map[0].1.entries[0].key, 0x1357_9BDF);
    assert_eq!(map[0].1.entries[0].value, 0x2468_ACE0);
    assert_eq!(desc.part_map.as_ref().expect("empty map").1.len(), 0);
    // The encoder writes the same order.
    let mut w = WireWriter::new();
    update.encode(&mut w).expect("encodes");
    assert_eq!(w.as_slice(), data.as_slice());

    // Must fail: a third nested word, or scalar B at the end.
    let mut third = hex(
        "00 40 07 00 00 00 00 C0 3F 00 00 80 3E 20 00 00 00 01 00 00 00 03 00 00 00 01 01 DF 9B 57 13 E0 AC 68 24 00 00 00 00 01 00",
    );
    assert!(VisualDescriptionUpdate::from_bytes(&third).is_err());
    third.truncate(1);
    assert!(VisualDescriptionUpdate::from_bytes(&third).is_err());
    let scalar_b_last = hex(
        "00 40 07 00 00 00 00 C0 3F 20 00 00 00 01 00 00 00 03 00 00 00 01 01 DF 9B 57 13 E0 AC 68 24 01 00 00 00 80 3E",
    );
    let read = VisualDescriptionUpdate::from_bytes(&scalar_b_last);
    assert!(
        read.map_or(true, |u| u.visual_desc.param_b != Some(0.25)),
        "scalar B comes before the collections"
    );
    // The base-model branch is unsupported (OPEN-WORLD-9), whatever follows it.
    for tail in [&[][..], &[1, 0, 0, 0], &[1, 0, 0, 0, 2, 0, 0, 0]] {
        let mut bytes = 0x0001u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(tail);
        assert_eq!(
            EntityAppearanceDesc::decode(&mut WireReader::new(&bytes)),
            Err(WorldError::UnsupportedAppearanceVariant)
        );
    }

    // The nonempty list, to exact exhaustion, through the one shared byte.
    let list = hex(VECTOR_16_LIST);
    assert_eq!(list.len(), 99);
    let decoded =
        CharacterListResponse::from_envelope_body(&list[1..], &NoProperties).expect("decodes");
    assert_eq!(decoded.characters.len(), 1);
    assert_eq!(decoded.characters[0].character_id, 0x0100_0000_0000_0077);
    assert_eq!(decoded.characters[0].name, "Fixture");
    assert_eq!(decoded.characters[0].visual_desc, *desc);
    assert_eq!(decoded.to_bytes().expect("encodes"), list);
    // A byte over, a byte short, or a second leading byte fails: no partial list.
    let mut over = list[1..].to_vec();
    over.push(0);
    assert!(CharacterListResponse::from_envelope_body(&over, &NoProperties).is_err());
    assert!(
        CharacterListResponse::from_envelope_body(&list[1..list.len() - 1], &NoProperties).is_err()
    );
    assert!(CharacterListResponse::from_envelope_body(&list, &NoProperties).is_err());
}

// ---------------------------------------------------------------------------
// Vector 17: a standalone Ack before any sequenced send (§9.3.8)
// ---------------------------------------------------------------------------

const VECTOR_17_ACK: &str =
    "00 00 12 34 00 04 00 00 40 00 00 00 00 01 CD 2B 07 5C 00 01 56 78 00 00 00 02";

#[test]
fn vector_17_standalone_ack_before_any_sequenced_send() {
    let hdr = DatagramHeader {
        rec_id: 0x1234,
        seq_num: 1,
        client_word: 1,
        peer_word: 0x5678,
        ..DatagramHeader::default()
    };
    let opt = OptionalHeaders {
        cumulative_ack: Some(2),
        ..OptionalHeaders::default()
    };
    let dgram = encode_datagram(hdr, &opt, &[], None, None).expect("encodes");
    assert_eq!(dgram, hex(VECTOR_17_ACK));
    let parsed = decode_datagram(&dgram, false, None, true).expect("verifies");
    assert_eq!(parsed.header.header_checksum(), 0xCD27_075A);
    assert_eq!(parsed.payload_sum, 0x0004_0002);
    assert_eq!(parsed.header.checksum, 0xCD2B_075C);
    // Must fail: outgoing sequence zero (SessionAuth's wire zero is not this counter).
    assert_ne!(
        encode_datagram(DatagramHeader { seq_num: 0, ..hdr }, &opt, &[], None, None)
            .expect("encodes"),
        hex(VECTOR_17_ACK)
    );
    // The connection itself: before any sequenced send its early control copies the counter's
    // initial 1, also for the closing Disconnect.
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    conn.receive_datagram(&hex(VECTOR_7_SERVER), SERVER, 0.2, WireCodecs::NONE)
        .expect("server packet");
    while conn.pop_outbound_datagram().is_some() {}
    assert_eq!(conn.send_seq_num(), 1);
    conn.queue_standalone_disconnect(1.0).expect("queued");
    let end = conn.pop_outbound_datagram().expect("the Disconnect");
    let end = decode_datagram(&end, false, None, false).expect("decodes");
    assert!(end.opt_headers.disconnect);
    assert_eq!(end.header.seq_num, 1);
    assert_eq!(conn.send_seq_num(), 1, "no sequence allocated");
}

// ---------------------------------------------------------------------------
// Vector 12's explicit form of the current `0x6F` (§9.3.8)
// ---------------------------------------------------------------------------

#[test]
fn vector_12_explicit_selection_header() {
    let data = hex("00 77 00 00 00 00 00 00 01 00");
    let frags =
        tessera_compat::session::split_blob(2, 1, 1, 0, Some(0x0600_0000_0000_0401), 0x6F, &data)
            .expect("split");
    let mut header = Vec::new();
    frags[0].header.encode(&mut header).expect("encodes");
    assert_eq!(
        header,
        hex("02 00 01 00 01 00 01 04 00 00 00 00 00 06 6F 0A")
    );
    // The generic default-recipient form is an alternate, not the service's destination.
    let generic =
        tessera_compat::session::split_blob(2, 1, 1, 0, None, 0x6F, &data).expect("split");
    let mut header = Vec::new();
    generic[0].header.encode(&mut header).expect("encodes");
    assert_eq!(header, hex("02 40 01 00 01 00 6F 0A"));
}

#[test]
fn the_completion_fixture() {
    use tessera_compat::initial::COMPLETION_DATA;
    assert_eq!(COMPLETION_DATA, [0x00]);
    let frags =
        tessera_compat::session::split_blob(1, 0xFFFF, 0, 0, None, 0xF5, &COMPLETION_DATA).unwrap();
    assert_eq!(frags.len(), 1);
    let mut header = Vec::new();
    frags[0].header.encode(&mut header).expect("encodes");
    assert_eq!(header, hex("01 40 80 F5 01 00"));
    assert_eq!(frags[0].payload, [0x00]);
}

// ---------------------------------------------------------------------------
// World §2.8 and §3.1.2: fixed bytes with every exercised appearance branch
// ---------------------------------------------------------------------------

/// World §2.8 with mask `0x07E2`: secondary, scale, scalar A, scalar B, one tint, a primary part
/// with an ID and a sub-map, and a nonempty part map, written by hand and not by the crate's encoder.
const APPEARANCE_ALL_BRANCHES: &str = "E2 07 00 00 11 22 33 44 \
     00 00 80 3F 00 00 00 40 00 00 00 3F \
     00 00 C0 3F \
     00 00 80 3E \
     01 00 00 00 01 00 00 00 02 00 00 00 04 03 02 01 05 00 00 00 06 00 00 00 \
     21 00 00 00 AA BB CC DD 01 00 00 00 03 00 00 00 01 01 DF 9B 57 13 E0 AC 68 24 \
     01 01 09 00 00 00 02 00 00 00 88 77 66 55";

#[test]
fn appearance_with_every_exercised_branch() {
    use tessera_compat::world::{
        EntityAppearanceDesc, VisualDescriptionUpdate, WireReader, WireWriter,
    };
    let bytes = hex(APPEARANCE_ALL_BRANCHES);
    let mut r = WireReader::new(&bytes);
    let desc = EntityAppearanceDesc::decode(&mut r).expect("decodes");
    r.finish().expect("to exact exhaustion");
    assert_eq!(desc.flags(), 0x07E2);
    assert_eq!(desc.secondary_did, Some(0x4433_2211));
    assert_eq!(desc.scale, Some([1.0, 2.0, 0.5]));
    assert_eq!(desc.param_a, Some(1.5));
    assert_eq!(desc.param_b, Some(0.25));
    let tints = desc.tint_entries.as_ref().expect("the tint");
    assert_eq!(
        (tints.len(), tints[0].color_bgra, tints[0].field_e),
        (1, 0x0102_0304, 6)
    );
    let part = desc.primary_part.as_ref().expect("the part");
    assert_eq!(part.part_ids[0], Some(0xDDCC_BBAA));
    let (sub_key, table) = &part.sub_map.as_ref().expect("the sub-map")[0];
    assert_eq!(
        (*sub_key, table.entries[0].key, table.entries[0].value),
        (3, 0x1357_9BDF, 0x2468_ACE0)
    );
    let (_, map) = desc.part_map.as_ref().expect("the part map");
    assert_eq!(map[0].0, 9);
    assert_eq!(map[0].1.part_ids[1], Some(0x5566_7788));
    let mut w = WireWriter::new();
    desc.encode(&mut w).expect("encodes");
    assert_eq!(w.as_slice(), bytes.as_slice());

    // `0x89` has its one shared byte and then this, and a second one does not read.
    let mut data = vec![0x00];
    data.extend_from_slice(&bytes);
    assert_eq!(
        VisualDescriptionUpdate::from_bytes(&data)
            .expect("decodes")
            .visual_desc,
        desc
    );
    let mut doubled = vec![0x00];
    doubled.extend_from_slice(&data);
    assert!(VisualDescriptionUpdate::from_bytes(&doubled).is_err());
    // Must fail: a byte over, or a byte short.
    let mut over = data.clone();
    over.push(0);
    assert!(VisualDescriptionUpdate::from_bytes(&over).is_err());
    assert!(VisualDescriptionUpdate::from_bytes(&data[..data.len() - 1]).is_err());
    // Must fail: scalar B after the collections reads as something else than 0.25.
    let scalar_b_at_the_end: Vec<u8> = {
        let mut moved = bytes.clone();
        let b: Vec<u8> = moved.drain(24..28).collect();
        moved.extend_from_slice(&b);
        moved
    };
    let mut r = WireReader::new(&scalar_b_at_the_end);
    let moved = EntityAppearanceDesc::decode(&mut r);
    assert!(moved.is_err() || r.finish().is_err());
}

/// World §3.1.1–§3.1.2: two entries with real tables, and an account table, to exact exhaustion.
#[test]
fn a_nonempty_list_with_tables_reads_to_exact_exhaustion() {
    use std::collections::BTreeMap;
    use tessera_compat::world::{
        CharacterListResponse, EncodedPropertyTable, EncodedPropertyValue,
        PID_ACCOUNT_NUM_CHARACTERS_ALLOWED, PID_AGENT_SPECIES, PID_CHARGEN_CHARACTER_LEVEL,
        PropertyType, WireWriter,
    };
    let dictionary: BTreeMap<u32, PropertyType> = BTreeMap::from([
        (PID_AGENT_SPECIES, PropertyType::Enum),
        (PID_CHARGEN_CHARACTER_LEVEL, PropertyType::Int),
        (PID_ACCOUNT_NUM_CHARACTERS_ALLOWED, PropertyType::Int),
    ]);
    let table = |pid: u32, value: EncodedPropertyValue| {
        let mut t = EncodedPropertyTable::new();
        t.push(pid, value);
        let mut w = WireWriter::new();
        t.encode(&mut w).expect("encodes");
        w.into_vec()
    };
    let appearance = hex(APPEARANCE_ALL_BRANCHES);
    let entry = |id: u8, name: &[u8]| {
        // `character_id`, a name of its own, `status_or_world`.
        let mut out = vec![id, 0, 0, 0, 0, 0, 0, 1];
        out.push(u8::try_from(name.len()).expect("short"));
        for b in name {
            out.extend_from_slice(&[*b, 0]);
        }
        out.extend_from_slice(&[1, 0, 0, 0]);
        out.extend_from_slice(&table(PID_AGENT_SPECIES, EncodedPropertyValue::Enum(7)));
        out.extend_from_slice(&table(
            PID_CHARGEN_CHARACTER_LEVEL,
            EncodedPropertyValue::Int(9),
        ));
        // `metadata_a`, the appearance, `metadata_b`..`e` and the timestamp.
        out.extend_from_slice(&[2, 0, 0, 0]);
        out.extend_from_slice(&appearance);
        for word in [3u32, u32::MAX, 4, u32::MAX] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&5u64.to_le_bytes());
        out
    };
    let mut list = vec![0x00, 2, 0, 0, 0];
    list.extend_from_slice(&entry(1, b"Fixture"));
    list.extend_from_slice(&entry(2, b"Sample"));
    list.extend_from_slice(&table(
        PID_ACCOUNT_NUM_CHARACTERS_ALLOWED,
        EncodedPropertyValue::Int(3),
    ));
    let decoded =
        CharacterListResponse::from_envelope_body(&list[1..], &dictionary).expect("decodes");
    assert_eq!(decoded.characters.len(), 2);
    assert_eq!(decoded.characters[0].name, "Fixture");
    assert_eq!(decoded.characters[1].name, "Sample");
    assert_eq!(decoded.characters[1].character_id, 0x0100_0000_0000_0002);
    assert_eq!(decoded.characters[1].visual_desc.flags(), 0x07E2);
    assert_eq!(
        decoded.characters[0].property(PID_AGENT_SPECIES),
        Some(&EncodedPropertyValue::Enum(7))
    );
    assert_eq!(
        decoded
            .account_props
            .get(PID_ACCOUNT_NUM_CHARACTERS_ALLOWED),
        Some(&EncodedPropertyValue::Int(3))
    );
    assert_eq!(decoded.to_bytes().expect("encodes"), list);
    // Must fail, with no partial list: a byte over, a byte short, an entry more than there is, an
    // unknown base-model appearance, or a PID the dictionary does not type.
    let mut over = list[1..].to_vec();
    over.push(0);
    assert!(CharacterListResponse::from_envelope_body(&over, &dictionary).is_err());
    assert!(
        CharacterListResponse::from_envelope_body(&list[1..list.len() - 1], &dictionary).is_err()
    );
    let mut more = list[1..].to_vec();
    more[0] = 3;
    assert!(CharacterListResponse::from_envelope_body(&more, &dictionary).is_err());
    let at = list
        .windows(4)
        .position(|w| w == [0xE2, 0x07, 0x00, 0x00])
        .expect("the mask");
    let mut base_model = list[1..].to_vec();
    base_model[at - 1] = 0xE3;
    assert!(CharacterListResponse::from_envelope_body(&base_model, &dictionary).is_err());
    let empty: BTreeMap<u32, PropertyType> = BTreeMap::new();
    assert!(CharacterListResponse::from_envelope_body(&list[1..], &empty).is_err());
}

// ---------------------------------------------------------------------------
// Vector 18: the current attachment and bit forms (§9.3.9)
// ---------------------------------------------------------------------------

const V18_F5: [u8; 15] = [
    0x01, 0x00, 0x77, 0, 0, 0, 0, 0, 0, 0x01, 0x80, 0xF5, 0x01, 0x00, 0x00,
];
const V18_F4: [u8; 7] = [0x01, 0x40, 0x80, 0xF4, 0x01, 0x00, 0x00];
const V18_ATTACHED: [u8; 43] = [
    0x81, 0x00, 0x01, 0x00, 0x77, 0, 0, 0, 0, 0, 0, 0x01, 0x40, 0x01, 0x01, 0x02, 0x03, 0x04, 0x05,
    0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0xFF,
];
const V18_QUEUE4: [u8; 17] = [
    0x04, 0x04, 0x01, 0x00, 0x01, 0x00, 0x77, 0, 0, 0, 0, 0, 0, 0x01, 0x40, 0x01, 0x00,
];
const V18_EMPTY: [u8; 2] = [0x01, 0xC0];

/// §9.3.9's checksum boundaries, in the driver of [`parse_packet_payload`]: the padded header with
/// its attachment as one buffer, the data as another.
fn fragments_sum(bytes: &[u8]) -> u32 {
    parse_packet_payload(FLAG_HAS_FRAGMENTED_BLOBS, bytes, true)
        .expect("parses")
        .2
}

#[test]
fn vector_18_explicit_f5_and_default_f4_send_forms() {
    // The explicit-recipient `0xF5` reply, as the client's own encoder writes it.
    let f5 = split_blob(1, 0, 0, 0, Some(0x0100_0000_0000_0077), 0xF5, &[0x00]).unwrap();
    let mut wire = Vec::new();
    f5[0].encode(&mut wire).unwrap();
    assert_eq!(wire, V18_F5);
    // The generic default-recipient form of 0xF5 is not this send form.
    let default_form = split_blob(1, 0, 0, 0, None, 0xF5, &[0x00]).unwrap();
    let mut other = Vec::new();
    default_form[0].encode(&mut other).unwrap();
    assert_ne!(other, V18_F5);
    assert_eq!(other[1] & 0x40, 0x40);
    // `0xF4`: the header is four bytes, then the pad of an odd length is in the padded header, and
    // the body is one zero byte, with no extension and no second prefix.
    let f4 = split_blob(1, 0, 0, 0, None, 0xF4, &[0x00]).unwrap();
    let mut wire = Vec::new();
    f4[0].encode(&mut wire).unwrap();
    assert_eq!(wire, V18_F4);
    let (frag, used) = BlobFragment::decode(&V18_F4).unwrap();
    assert_eq!(used, V18_F4.len());
    assert_eq!(frag.header.ext_word, None);
    assert_eq!(frag.header.blob_header_word, Some(0xF4));
    assert_eq!(frag.header.recipient_id, 2);
    assert_eq!(frag.payload, [0x00]);
}

/// `0xF4`'s header is six bytes with its pad (the row's "4" counts without the length and the pad;
/// §5.6.3 and the bytes decide, `docs/m8-remediation.md` §27 fix 3), and it is one checksum buffer. The
/// body is a separate one, which a zero byte cannot show: with data `FF` the sum of the two buffers
/// differs from the checksum of the whole fragment.
#[test]
fn vector_18_f4_header_and_body_are_separate_checksum_buffers() {
    let frag = &split_blob(1, 0, 0, 0, None, 0xF4, &[0xFF]).unwrap()[0];
    let mut whole = Vec::new();
    frag.encode(&mut whole).unwrap();
    assert_eq!(whole, [0x01, 0x40, 0x80, 0xF4, 0x01, 0x00, 0xFF]);
    let (header, used) = BlobFragmentHeader::decode(&whole).unwrap();
    assert_eq!(used, 6, "with the pad");
    assert_eq!(header.frag_data_len, 1);
    let opt = OptionalHeaders::default();
    // The primitive by hand: `014080F4 + 01000000 + 6 << 16`, then `FF000000 + 1 << 16`.
    let required = payload_checksum(&opt, std::slice::from_ref(frag)).unwrap();
    assert_eq!(required, 0x0147_80F4);
    assert_eq!(
        required,
        compute_datagram_checksum(&whole[..used])
            .wrapping_add(compute_datagram_checksum(&whole[used..]))
    );
    // A whole-fragment checksum, the form that must not be used, differs. The difference is the
    // one the builder note's pair (0xA09C5C63 required, 0xA19D5B63 whole-fragment) has, whatever the
    // datagram header that pair was summed with.
    let whole_fragment = compute_datagram_checksum(&whole);
    assert_eq!(whole_fragment, 0x0248_7FF4);
    assert_eq!(
        whole_fragment.wrapping_sub(required),
        0xA19D_5B63u32.wrapping_sub(0xA09C_5C63)
    );
}

/// `blobFlags` `0x01` with no extension word adds no field, and its data follows the padded header.
#[test]
fn vector_18_flags_01_without_an_extension_word_adds_no_field() {
    let wire = [0x01, 0x41, 0x80, 0xF4, 0x01, 0x00, 0x00];
    let (frag, used) = BlobFragment::decode(&wire).unwrap();
    assert_eq!(used, wire.len());
    assert_eq!(frag.header.ext_word, None);
    assert_eq!(frag.header.attachment, None);
    assert_eq!(frag.header.blob_flags, 0x41);
    assert_eq!(frag.header.blob_header_word, Some(0xF4));
    assert_eq!(frag.payload, [0x00]);
}

#[test]
fn vector_18_extension_attachment_is_one_checksum_buffer_with_the_header() {
    let (frag, used) = BlobFragment::decode(&V18_ATTACHED).unwrap();
    assert_eq!(used, V18_ATTACHED.len());
    let h = &frag.header;
    assert_eq!(h.ext_word, Some(0x0001));
    assert_eq!(h.recipient_id, 0x0100_0000_0000_0077);
    assert_eq!(h.blob_header_word, Some(0x40));
    assert_eq!(frag.payload, [0xFF]);
    let a = h.attachment.expect("attachment");
    assert_eq!(a.words64[0], 0x0807_0605_0403_0201);
    assert_eq!(a.words64[2], 0x1817_1615_1413_1211);
    assert_eq!(a.word32, 0x1C1B_1A19);
    // Re-encoding gives the same bytes.
    let mut wire = Vec::new();
    frag.encode(&mut wire).unwrap();
    assert_eq!(wire, V18_ATTACHED);

    // Header and attachment are one buffer, the data another.
    let (head, data) = V18_ATTACHED.split_at(V18_ATTACHED.len() - 1);
    let right = compute_datagram_checksum(head).wrapping_add(compute_datagram_checksum(data));
    assert_eq!(fragments_sum(&V18_ATTACHED), right);
    // MUST fail: a separately padded attachment buffer on this unaligned combined length.
    let (header, attachment) = head.split_at(14);
    let wrong = compute_datagram_checksum(header)
        .wrapping_add(compute_datagram_checksum(attachment))
        .wrapping_add(compute_datagram_checksum(data));
    assert_ne!(right, wrong);
    // MUST fail: truncation within the 28 bytes.
    for cut in 15..head.len() {
        assert!(
            BlobFragmentHeader::decode(&V18_ATTACHED[..cut]).is_err(),
            "{cut}"
        );
    }
    // MUST fail: an unknown extension bit.
    let mut unknown = V18_ATTACHED;
    unknown[2] = 0x03;
    assert_eq!(
        BlobFragmentHeader::decode(&unknown),
        Err(SessionError::UnknownExtensionBits { ext_word: 0x0003 })
    );
    // MUST fail: treating the attachment's first bytes as the next fragment: without the extension
    // word's bit they parse as queue 1, and nothing of this fixture does then.
    let mut no_bit = V18_ATTACHED;
    no_bit[2] = 0x00;
    assert!(parse_packet_payload(FLAG_HAS_FRAGMENTED_BLOBS, &no_bit, true).is_err());
    // MUST fail: the withdrawn grammar. `blobFlags` 0x01 with no extension word has no attachment,
    // so the 28 bytes after the header are not skipped.
    let mut flag_grammar = vec![0x01, 0x01];
    flag_grammar.extend_from_slice(&V18_ATTACHED[4..12]);
    flag_grammar.extend_from_slice(&V18_ATTACHED[12..]);
    let (flagged, used) = BlobFragmentHeader::decode(&flag_grammar).unwrap();
    assert_eq!(flagged.attachment, None);
    assert_eq!(used, 12);
}

#[test]
fn vector_18_bit_04_and_the_empty_control() {
    let (frag, used) = BlobFragment::decode(&V18_QUEUE4).unwrap();
    assert_eq!(used, V18_QUEUE4.len());
    assert_eq!(frag.header.queue_id, 4);
    assert_eq!(frag.header.blob_flags, 0x04, "kept as metadata");
    assert_eq!(frag.header.order_num, 1);
    assert_eq!(frag.header.blob_id, 1);
    assert_eq!(frag.header.blob_header_word, Some(0x40));
    assert_eq!(frag.payload, [0x00]);
    let mut r = BlobReassembler::default();
    assert_eq!(
        r.ingest(1, frag, 0.0)
            .unwrap()
            .expect("dispatched")
            .blob_header_word,
        0x40
    );

    // No opcode, length or data, no application dispatch, and the next header right after it.
    let (empty, used) = BlobFragment::decode(&V18_EMPTY).unwrap();
    assert_eq!(used, 2);
    assert_eq!(empty.header.blob_header_word, None);
    assert_eq!(empty.header.frag_data_len, 0);
    assert_eq!(empty.payload, Vec::<u8>::new());
    let mut r = BlobReassembler::default();
    assert_eq!(r.ingest(1, empty, 0.0).unwrap(), None);
    assert_eq!(r.counts().empty_controls, 1);
    let mut both = V18_EMPTY.to_vec();
    both.extend_from_slice(&V18_F4);
    let (_, frags, _) = parse_packet_payload(FLAG_HAS_FRAGMENTED_BLOBS, &both, true).unwrap();
    assert_eq!(frags.len(), 2);
    assert_eq!(frags[1].header.blob_header_word, Some(0xF4));
}

#[test]
fn vector_18_fragment_lengths_are_checked_before_anything_is_copied() {
    // A two-part blob, queue 1, blob ID 1: `numFrags` 2, default recipient, and `len` of the
    // fragment at `index`.
    let part = |index: u32, len: u32, data: usize| {
        let mut wire = vec![0x01, 0x10 | 0x40, 0x01, 0x00];
        encode_variable_u32(index, &mut wire).unwrap();
        encode_variable_u32(2, &mut wire).unwrap();
        if index == 0 {
            encode_variable_u32(0x40, &mut wire).unwrap();
        }
        encode_variable_u32(len, &mut wire).unwrap();
        if wire.len() % 2 != 0 {
            wire.push(0);
        }
        wire.extend(std::iter::repeat_n(0x5Au8, data));
        wire
    };
    // 436 then 1 passes.
    let mut r = BlobReassembler::default();
    let (a, _) = BlobFragment::decode(&part(0, 436, 436)).unwrap();
    let (b, _) = BlobFragment::decode(&part(1, 1, 1)).unwrap();
    assert_eq!(r.ingest(1, a, 0.0).unwrap(), None);
    assert_eq!(
        r.ingest(1, b, 0.0)
            .unwrap()
            .expect("complete")
            .payload
            .len(),
        437
    );
    // A non-final of 100 and a final of 464 fail in the header, before the slice is copied; the
    // data need not even be there.
    for (index, len) in [(0, 100), (1, 464), (1, 0), (0, 437)] {
        assert_eq!(
            BlobFragment::decode(&part(index, len, 0)),
            Err(SessionError::InvalidFragmentDataLen { len })
        );
    }
    // A single fragment may have 0..=464 bytes.
    assert!(check_fragment_slice(0, 1, 0).is_ok());
    assert!(check_fragment_slice(0, 1, 464).is_ok());
    assert!(check_fragment_slice(0, 1, 465).is_err());
    // A nonzero pad, or an index equal to its count.
    let mut pad = V18_F4;
    pad[5] = 1;
    assert_eq!(
        BlobFragmentHeader::decode(&pad),
        Err(SessionError::NonZeroAlignmentPad { pad: 1 })
    );
    assert!(matches!(
        BlobFragment::decode(&part(2, 1, 1)),
        Err(SessionError::InvalidFragmentBounds { .. })
    ));
}

// ---------------------------------------------------------------------------
// Vector 19: the first-world server control and fragment composition (§9.3.9)
// ---------------------------------------------------------------------------

#[test]
fn vector_19_first_world_datagram() {
    const COMPLETE: [u8; 64] = [
        0x12, 0x34, 0x00, 0x2C, 0x01, 0x00, 0x01, 0x06, 0x00, 0x00, 0x00, 0x02, 0xD6, 0x42, 0xFD,
        0xB4, 0x00, 0x01, 0x56, 0x78, 0x78, 0x56, 0x34, 0x12, 0x40, 0x45, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x02, 0x77, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x81, 0x49, 0x04,
        0x00, 0x00, 0x00, 0x01, 0x07, 0x02, 0x40, 0x01, 0x00, 0x01, 0x00, 0x80, 0x88, 0x04, 0x00,
        0x00, 0x00, 0x01, 0x09,
    ];
    let mut seed = IsaacCipher::from_seed(0x1357_2468);
    let mask = seed.next_mask();
    assert_eq!(mask, 0xCD17_9AD8);

    let header = DatagramHeader::from_wire_bytes(&COMPLETE).unwrap();
    assert_eq!(header.rec_id, 0x1234);
    assert_eq!(header.flags, 0x0100_0106);
    assert_eq!(header.seq_num, 2);
    assert_eq!(header.checksum, 0xD642_FDB4);
    assert_eq!(header.peer_word, 0x5678);

    let (opt, frags, sum) = parse_packet_payload(header.flags, &COMPLETE[20..], true).unwrap();
    assert_eq!(opt.clock_sync, Some(42.0));
    assert!(opt.server_redirect.is_some());
    assert_eq!(frags.len(), 2);
    assert_eq!(
        frags[0].header.blob_flags, 0x02,
        "queue 1: kept as metadata"
    );
    assert_eq!(frags[0].header.blob_header_word, Some(0x149));
    assert_eq!(frags[1].header.blob_header_word, Some(0x88));
    verify_packet_checksum(header, sum, Some(mask)).unwrap();

    // Each row is its own buffer.
    let rows: [&[u8]; 6] = [
        &[0x78, 0x56, 0x34, 0x12],
        &[0x40, 0x45, 0, 0, 0, 0, 0, 0],
        &[
            0x01, 0x02, 0x77, 0, 0, 0, 0, 0, 0, 0x01, 0x81, 0x49, 0x04, 0x00,
        ],
        &[0x00, 0x00, 0x01, 0x07],
        &[0x02, 0x40, 0x01, 0x00, 0x01, 0x00, 0x80, 0x88, 0x04, 0x00],
        &[0x00, 0x00, 0x01, 0x09],
    ];
    let by_rows = rows.iter().fold(0u32, |sum, r| {
        sum.wrapping_add(compute_datagram_checksum(r))
    });
    assert_eq!(by_rows, sum);

    // MUST fail: a stale mask, the whole payload as one buffer.
    let mut login_seed = IsaacCipher::from_seed(0x1357_2469);
    assert!(verify_packet_checksum(header, sum, Some(login_seed.next_mask())).is_err());
    let whole = compute_datagram_checksum(&COMPLETE[20..]);
    assert!(verify_packet_checksum(header, whole, Some(mask)).is_err());
    // MUST fail: an eight-byte redirect shifts every boundary.
    let mut eight = COMPLETE[20..24].to_vec();
    eight.extend_from_slice(&[0; 4]);
    eight.extend_from_slice(&COMPLETE[24..]);
    assert!(parse_packet_payload(header.flags, &eight, true).is_err());
    // MUST fail: a second prefix byte makes the data one byte longer than its slice length says.
    let mut second = COMPLETE[20..].to_vec();
    second.insert(46 - 20, 0x00);
    assert!(parse_packet_payload(header.flags, &second, true).is_err());
}

// ---------------------------------------------------------------------------
// Vector 20: all exercised appearance branches, in order (§9.3.9, world §2.8)
// ---------------------------------------------------------------------------

const VECTOR_20: &str = "00 E2 07 00 00 78 56 34 12 00 00 80 3F 00 00 00 40 00 00 40 40 00 00 C0 3F 00 00 80 3E 01 00 00 00 03 00 00 00 05 00 00 00 10 20 40 80 07 00 00 00 09 00 00 00 02 00 00 00 44 33 22 11 01 01 DD CC BB AA 01 00 00 00 88 77 66 55";

#[test]
fn vector_20_all_exercised_appearance_branches() {
    use tessera_compat::world::VisualDescriptionUpdate;
    let data = hex(VECTOR_20);
    assert_eq!(data.len(), 75);
    let d = VisualDescriptionUpdate::from_bytes(&data)
        .expect("exact exhaustion")
        .visual_desc;
    assert_eq!(d.flags(), 0x07E2);
    assert_eq!(d.secondary_did, Some(0x1234_5678));
    assert_eq!(d.scale, Some([1.0, 2.0, 3.0]));
    assert_eq!((d.param_a, d.param_b), (Some(1.5), Some(0.25)));
    let tints = d.tint_entries.as_ref().expect("tints");
    assert_eq!(tints.len(), 1);
    assert_eq!((tints[0].slot, tints[0].key), (3, 5));
    // B=0x10, G=0x20, R=0x40, A=0x80.
    assert_eq!(tints[0].color_bgra, 0x8040_2010);
    assert_eq!(tints[0].color_bgra.to_le_bytes(), [0x10, 0x20, 0x40, 0x80]);
    assert_eq!((tints[0].field_d, tints[0].field_e), (7, 9));
    let primary = d.primary_part.as_ref().expect("primary");
    assert_eq!(primary.part_ids, [None, Some(0x1122_3344), None, None]);
    let (hint, map) = d.part_map.as_ref().expect("part map");
    assert_eq!(*hint, 1);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].0, 0xAABB_CCDD);
    assert_eq!(map[0].1.part_ids, [Some(0x5566_7788), None, None, None]);

    // Re-encoding gives the same 75 bytes: the order is the table's.
    assert_eq!(
        VisualDescriptionUpdate { visual_desc: d }
            .to_bytes()
            .expect("encodes"),
        data
    );
    // MUST fail: a trailing byte; a truncation anywhere.
    let mut longer = data.clone();
    longer.push(0);
    assert!(VisualDescriptionUpdate::from_bytes(&longer).is_err());
    for cut in 1..data.len() {
        assert!(
            VisualDescriptionUpdate::from_bytes(&data[..cut]).is_err(),
            "{cut}"
        );
    }
    // MUST fail: scalar B after the collections: moving its four bytes to the end shifts the tint
    // count.
    let mut moved = data[..25].to_vec();
    moved.extend_from_slice(&data[29..]);
    moved.extend_from_slice(&data[25..29]);
    assert!(VisualDescriptionUpdate::from_bytes(&moved).is_err());
    // MUST fail: tint before scalar B.
    let mut tint_first = data[..25].to_vec();
    tint_first.extend_from_slice(&data[29..53]);
    tint_first.extend_from_slice(&data[25..29]);
    tint_first.extend_from_slice(&data[53..]);
    assert!(VisualDescriptionUpdate::from_bytes(&tint_first).is_err());
    // MUST fail: an omitted part-map key.
    let mut no_key = data[..63].to_vec();
    no_key.extend_from_slice(&data[67..]);
    assert!(VisualDescriptionUpdate::from_bytes(&no_key).is_err());
    // MUST fail: an unknown part bit 0x10.
    let mut bit = data.clone();
    bit[67] = 0x11;
    assert!(VisualDescriptionUpdate::from_bytes(&bit).is_err());
    // MUST fail: the TSize `01 E0 00 00 00 00`, a three-tier count that does not fit what is left.
    let mut tsize = data[..61].to_vec();
    tsize.extend_from_slice(&[0x01, 0xE0, 0, 0, 0, 0]);
    tsize.extend_from_slice(&data[63..]);
    assert!(VisualDescriptionUpdate::from_bytes(&tsize).is_err());
    // A hint past the table.
    let mut hint23 = data.clone();
    hint23[61] = 23;
    assert!(VisualDescriptionUpdate::from_bytes(&hint23).is_err());
    // The base-model bit is unsupported, not read as one guessed form.
    let mut base = data.clone();
    base[1] = 0xE3;
    assert!(VisualDescriptionUpdate::from_bytes(&base).is_err());
}

// ---------------------------------------------------------------------------
// Vector 22: queue comparison and generation policies (§9.3.9)
// ---------------------------------------------------------------------------

const V22_A: &[u8] = &[0x11];
const V22_B: &[u8] = &[0x22];
const V22_C: &[u8] = &[0x33];

/// One complete queue-3 blob of recipient 0x77, sequence group 5, on sender 1.
fn v22_blob(order: u16, body: &[u8]) -> BlobFragment {
    v22_keyed(0x77, order, body)
}

fn v22_keyed(recipient: u64, order: u16, body: &[u8]) -> BlobFragment {
    split_blob(3, order, 0, 5, Some(recipient), 0x40, body)
        .expect("split")
        .remove(0)
}

/// Whether the blob is delivered at `t`.
fn v22_take(r: &mut BlobReassembler, f: BlobFragment, t: f64) -> bool {
    r.ingest(1, f, t).expect("accepted").is_some()
}

#[test]
fn vector_22_distances() {
    // First, then continuing over the wrap.
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(65_535, V22_A), 0.0), "first");
    assert!(v22_take(&mut r, v22_blob(0, V22_B), 0.0), "distance 1");
    assert!(v22_take(&mut r, v22_blob(32_767, V22_A), 0.0), "32,767");
    // 32,768 delivers once with an anomaly and leaves the baseline.
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(0, V22_C), 0.0));
    assert!(v22_take(&mut r, v22_blob(32_768, V22_A), 0.0));
    assert_eq!(r.counts().queue3_ambiguous, 1);
    assert!(
        v22_take(&mut r, v22_blob(1, V22_B), 0.0),
        "baseline stayed 0"
    );
    // 32,769 is older.
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(0, V22_C), 0.0));
    assert!(!v22_take(&mut r, v22_blob(32_769, V22_A), 0.0));
    assert_eq!(r.counts().queue3_older, 1);
}

#[test]
fn vector_22_an_older_word_discards_even_a_conflicting_body_with_a_tombstone() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(6, V22_A), 0.0));
    assert!(v22_take(&mut r, v22_blob(7, V22_C), 0.0));
    assert!(!v22_take(&mut r, v22_blob(6, V22_B), 0.0));
    assert_eq!(r.counts().queue3_older, 1);
    assert_eq!(r.counts().reused_identities, 0);
}

/// Each delivered body has its own tombstone, so a repeat of the first deduplicates after the second
/// (§28 fix 8's observable test of "retain both").
#[test]
fn vector_22_equal_word_bodies_each_keep_a_tombstone() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(7, V22_A), 0.0));
    assert!(
        v22_take(&mut r, v22_blob(7, V22_B), 0.0),
        "once, an anomaly"
    );
    assert_eq!(r.counts().reused_identities, 1);
    assert!(!v22_take(&mut r, v22_blob(7, V22_B), 0.0), "B again");
    assert!(!v22_take(&mut r, v22_blob(7, V22_A), 0.0), "A again");
    assert_eq!(r.counts().duplicates, 2);
    assert_eq!(r.tombstone_sizes().0, 2);
}

/// A 65th completion evicts the oldest and counts; its repeat is then a fresh delivery. A repeat after
/// the 30 seconds likewise.
#[test]
fn vector_22_tombstones_are_bounded_and_expire() {
    let mut r = BlobReassembler::default();
    for order in 0..=64u16 {
        assert!(v22_take(&mut r, v22_blob(order, V22_A), 0.0), "{order}");
    }
    assert_eq!(r.tombstone_sizes().0, 64);
    assert_eq!(r.counts().tombstones_evicted, 1);
    // The evicted identity's word is now older than the baseline, so it is the distance's discard; on
    // queue 2 the same repeat has no gate and is delivered.
    let queue2 = |id: u16, body: &[u8]| {
        split_blob(2, id, id, 0, Some(0x77), 0x40, body)
            .expect("split")
            .remove(0)
    };
    let mut r = BlobReassembler::default();
    for id in 0..=64u16 {
        assert!(v22_take(&mut r, queue2(id, V22_A), 0.0), "{id}");
    }
    assert!(
        v22_take(&mut r, queue2(0, V22_A), 0.0),
        "evicted: delivered"
    );
    assert!(!v22_take(&mut r, queue2(64, V22_A), 0.0), "still kept");
    // Past its 30 seconds a repeat is delivered again; within them it is not.
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, queue2(1, V22_A), 0.0));
    assert!(!v22_take(&mut r, queue2(1, V22_A), 30.0), "at 30 s");
    assert!(v22_take(&mut r, queue2(1, V22_A), 30.5), "past it");
}

/// Queue 2 and queue 4 deliver with a repeated order word, and exact identities still deduplicate.
#[test]
fn vector_22_queues_2_and_4_have_no_order_gate() {
    for queue in [2u8, 4] {
        let blob = |id: u16, body: &[u8]| {
            split_blob(queue, 9, id, 0, Some(0x77), 0x40, body)
                .expect("split")
                .remove(0)
        };
        let mut r = BlobReassembler::default();
        assert!(v22_take(&mut r, blob(1, V22_A), 0.0));
        assert!(v22_take(&mut r, blob(2, V22_B), 0.0), "the same order word");
        assert!(!v22_take(&mut r, blob(2, V22_B), 0.0), "an exact repeat");
    }
}

/// A baseline expires 30 seconds after the last delivered blob of its key, and only a delivery
/// refreshes it (§28 fix 8's row: baseline 100, then 40000 after 30 s).
#[test]
fn vector_22_a_baseline_expires_after_the_last_delivery() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(100, V22_A), 0.0));
    assert!(!v22_take(&mut r, v22_blob(40_000, V22_B), 29.0), "older");
    assert!(
        !v22_take(&mut r, v22_blob(40_000, V22_B), 30.0),
        "a discard refreshes nothing, and 30 s is still inside"
    );
    assert!(
        v22_take(&mut r, v22_blob(40_000, V22_C), 30.5),
        "a new first value"
    );
    // An equal-word delivery refreshes it.
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(100, V22_A), 0.0));
    assert!(v22_take(&mut r, v22_blob(100, V22_B), 20.0));
    assert!(
        !v22_take(&mut r, v22_blob(40_000, V22_C), 45.0),
        "still older"
    );
}

/// A 257th key at a full map skips without eviction and takes no tombstone; expiry lets it in.
#[test]
fn vector_22_a_new_key_at_a_full_map_skips() {
    let mut r = BlobReassembler::default();
    for recipient in 0..256u64 {
        assert!(v22_take(
            &mut r,
            v22_keyed(0x1000 + recipient, 1, V22_A),
            0.0
        ));
    }
    let tombstones = r.tombstone_sizes().0;
    assert!(!v22_take(&mut r, v22_keyed(0x9999, 1, V22_B), 1.0));
    assert_eq!(r.counts().queue3_groups_over, 1);
    assert_eq!(r.tombstone_sizes().0, tombstones, "no tombstone");
    // The live keys stay: the first key's older word is still older.
    assert!(!v22_take(&mut r, v22_keyed(0x1000, 0, V22_B), 1.0));
    // Its sibling in the same datagram is delivered, and the datagram is taken whole.
    let done = r
        .ingest_packet(
            1,
            &[v22_keyed(0x9999, 2, V22_B), v22_keyed(0x1001, 2, V22_C)],
            1.0,
        )
        .expect("accepted");
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].recipient_id, 0x1001);
    // Past the 30 seconds the key can take its first word.
    assert!(v22_take(&mut r, v22_keyed(0x9999, 3, V22_B), 40.0));
}

/// An empty omitted-chunk control makes no baseline, tombstone or entry.
#[test]
fn vector_22_an_empty_control_leaves_nothing() {
    let (control, used) = BlobFragment::decode(&[0x01, 0xC0]).unwrap();
    assert_eq!(used, 2);
    let mut r = BlobReassembler::default();
    assert!(r.ingest(1, control, 0.0).unwrap().is_none());
    assert_eq!(r.counts().empty_controls, 1);
    assert_eq!(r.tombstone_sizes(), (0, 0));
    assert_eq!(r.incomplete_len(), 0);
}

// ---------------------------------------------------------------------------
// Vector 23: attachment arithmetic and combined part IDs (§10.2)
// ---------------------------------------------------------------------------

#[test]
fn vector_23_attachment_sums() {
    let (head, data) = V18_ATTACHED.split_at(V18_ATTACHED.len() - 1);
    assert_eq!((head.len(), data.len()), (14 + 28, 1));
    assert_eq!(
        compute_datagram_checksum(head).wrapping_add(compute_datagram_checksum(data)),
        0xA09C_5C63
    );
    assert_eq!(fragments_sum(&V18_ATTACHED), 0xA09C_5C63);
    assert_ne!(
        fragments_sum(&V18_ATTACHED),
        0xA19D_5B63,
        "the whole fragment"
    );
    assert_eq!(compute_datagram_checksum(&V18_ATTACHED), 0xA19D_5B63);
    let (header, attachment) = head.split_at(14);
    assert_eq!(
        compute_datagram_checksum(header)
            .wrapping_add(compute_datagram_checksum(attachment))
            .wrapping_add(compute_datagram_checksum(data)),
        0x928E_6A71,
        "separate buffers"
    );
    // The empty control's attachment is one buffer, with no data contribution, and the next header
    // follows at once; a present zero extension adds no attachment.
    let mut attached = vec![0x81, 0xC0, 0x01, 0x00];
    attached.extend(0x01..=0x1Cu8);
    let (frag, used) = BlobFragment::decode(&attached).unwrap();
    assert_eq!(used, attached.len());
    assert!(frag.payload.is_empty());
    assert_eq!(
        fragments_sum(&attached),
        compute_datagram_checksum(&attached)
    );
    let zero_ext = [0x81, 0x40, 0x00, 0x00, 0x80, 0xF4, 0x01, 0x00, 0x00];
    let (frag, used) = BlobFragment::decode(&zero_ext).unwrap();
    assert_eq!((used, frag.payload.as_slice()), (9, &[0x00][..]));
    assert_eq!(frag.header.attachment, None);
}

// ---------------------------------------------------------------------------
// Vector 22, additional rows (§9.3.9 queue comparison, P10-10J)
// ---------------------------------------------------------------------------

/// One complete queue-2 blob of recipient 0x77 with blob ID `id`.
fn v22_queue2(id: u16, body: &[u8]) -> BlobFragment {
    split_blob(2, id, id, 0, Some(0x77), 0x40, body)
        .expect("split")
        .remove(0)
}

/// On queue 3 the older band discards BEFORE any tombstone is compared: an older exact repeat is an
/// older-word discard, not a duplicate.
#[test]
fn vector_22_the_older_band_comes_before_the_tombstone() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(6, V22_A), 0.0));
    assert!(v22_take(&mut r, v22_blob(7, V22_C), 0.0));
    assert!(
        !v22_take(&mut r, v22_blob(6, V22_A), 0.0),
        "older exact repeat"
    );
    assert_eq!(r.counts().queue3_older, 1);
    assert_eq!(r.counts().duplicates, 0);
}

/// Baseline 7, candidate 8/A already in a retained exact tombstone: deduplicates before the newer
/// advance, and the baseline stays 7. The tombstone (8, A) is made while the baseline is elsewhere:
/// 8 delivered at distance 32,768 leaves the baseline, then 7 is newer than it.
#[test]
fn vector_22_an_exact_tombstone_deduplicates_before_a_newer_advance() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(32_776, V22_C), 0.0), "first");
    assert!(v22_take(&mut r, v22_blob(8, V22_A), 0.0), "32,768: once");
    assert_eq!(r.counts().queue3_ambiguous, 1);
    assert!(v22_take(&mut r, v22_blob(7, V22_B), 0.0), "32,767: newer");
    // Baseline 7; 8/A is distance 1 with an exact tombstone.
    assert!(!v22_take(&mut r, v22_blob(8, V22_A), 0.0), "deduplicated");
    assert_eq!(r.counts().duplicates, 1);
    // Still 7: a word 7 is equal (delivered, a reuse), not older.
    let reused = r.counts().reused_identities;
    assert!(
        v22_take(&mut r, v22_blob(7, V22_C), 0.0),
        "baseline stayed 7"
    );
    assert_eq!(r.counts().reused_identities, reused + 1);
    assert_eq!(r.counts().queue3_older, 0);
}

/// A complete ordinary opcode blob with ZERO data on queue 3 establishes and advances a baseline,
/// unlike an omitted-chunk control.
#[test]
fn vector_22_a_zero_length_opcode_blob_sets_a_baseline() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(5, &[]), 0.0), "first");
    assert!(v22_take(&mut r, v22_blob(6, &[]), 0.0), "newer");
    assert!(!v22_take(&mut r, v22_blob(5, V22_A), 0.0), "older than 6");
    assert_eq!(r.counts().queue3_older, 1);
}

/// A fragmented queue-1 completed identity 9/A repeated deduplicates with the shared tombstone
/// budget; an unfragmented queue-1 blob (ID 0) is never deduplicated.
#[test]
fn vector_22_fragmented_queue_1_takes_a_tombstone() {
    let body: Vec<u8> = (0..437u16).map(|i| (i & 0xFF) as u8).collect();
    let parts = split_blob(1, 0, 9, 0, None, 0x1234, &body).expect("split");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].header.blob_id, 9);
    let mut r = BlobReassembler::default();
    assert_eq!(r.ingest_packet(1, &parts, 0.0).expect("A").len(), 1);
    assert_eq!(
        r.tombstone_sizes().0,
        1,
        "one tombstone of the shared budget"
    );
    assert!(r.ingest_packet(1, &parts, 1.0).expect("again").is_empty());
    assert_eq!(
        r.counts().duplicates,
        2,
        "each old part is ignored and counted"
    );
    assert_eq!(r.incomplete_len(), 0, "no entry opened");
    // Unfragmented queue 1 has no identity: delivered every time, no tombstone.
    let plain = split_blob(1, 0, 0, 0, None, 0x55, V22_A).expect("split");
    assert!(v22_take(&mut r, plain[0].clone(), 2.0));
    assert!(v22_take(&mut r, plain[0].clone(), 2.0));
    assert_eq!(r.tombstone_sizes().0, 1);
    // The budget is shared: 64 queue-2 completions evict the queue-1 tombstone first.
    for id in 0..64u16 {
        assert!(v22_take(&mut r, v22_queue2(id, V22_A), 3.0));
    }
    assert_eq!(r.counts().tombstones_evicted, 1);
    assert_eq!(r.ingest_packet(1, &parts, 4.0).expect("evicted").len(), 1);
}

/// Fill 64 same-direction tombstones, then a 65th distinct completion, then repeat the evicted body's
/// identity and data while its queue-3 baseline is retained: the eviction is a bounded-memory count
/// only, and the repeat at the equal word with no exact tombstone delivers with a reuse count. The
/// same covers "baseline 7, equal 7/A, no retained tombstone".
#[test]
fn vector_22_an_evicted_tombstone_at_the_equal_word_is_a_reuse() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(7, V22_A), 0.0));
    for id in 0..64u16 {
        assert!(v22_take(&mut r, v22_queue2(id, V22_B), 1.0));
    }
    assert_eq!(r.counts().tombstones_evicted, 1);
    assert_eq!(r.counts().reused_identities, 0, "eviction is no anomaly");
    assert!(v22_take(&mut r, v22_blob(7, V22_A), 2.0), "delivered again");
    assert_eq!(r.counts().reused_identities, 1);
    assert_eq!(r.counts().duplicates, 0);
    // The baseline's word did not change: 6 is still older.
    assert!(!v22_take(&mut r, v22_blob(6, V22_C), 2.0));
}

/// Repeat a completion after its 30-second tombstone expiry while the baseline still lives through a
/// later equal-word delivery: delivered with a reuse count, not an exact-duplicate claim.
#[test]
fn vector_22_a_repeat_after_the_tombstone_expired_is_a_reuse() {
    let mut r = BlobReassembler::default();
    assert!(v22_take(&mut r, v22_blob(7, V22_A), 0.0));
    assert!(
        v22_take(&mut r, v22_blob(7, V22_B), 20.0),
        "equal word, refreshes"
    );
    let reused = r.counts().reused_identities;
    assert!(
        v22_take(&mut r, v22_blob(7, V22_A), 35.0),
        "A's tombstone is gone"
    );
    assert_eq!(r.counts().reused_identities, reused + 1);
    assert_eq!(r.counts().duplicates, 0);
    assert!(
        !v22_take(&mut r, v22_blob(7, V22_B), 36.0),
        "B's tombstone lives"
    );
}

// ---------------------------------------------------------------------------
// Vector 24: old-part, mask and retention policies (§10.2)
// ---------------------------------------------------------------------------

/// The two parts of Vector 24's invented identity 9, opcode 0x1234, on `queue` (1, fragmented, or 2):
/// `first` (436 bytes) then `last` (one byte).
fn v24_parts(queue: u8, first: u8, last: u8) -> Vec<BlobFragment> {
    let mut body = vec![first; 436];
    body.push(last);
    let parts = split_blob(queue, 3, 9, 0, None, 0x1234, &body).expect("split");
    assert_eq!(parts.len(), 2);
    parts
}

/// A reassembler holding the completed tombstone of (X, Y) on `queue`.
fn v24_after_xy(queue: u8) -> BlobReassembler {
    let mut r = BlobReassembler::default();
    let xy = v24_parts(queue, 0x11, 0x22);
    assert_eq!(r.ingest_packet(1, &xy, 0.0).expect("X, Y").len(), 1);
    r
}

#[test]
fn vector_24_old_parts_on_queues_1_and_2() {
    for queue in [1u8, 2] {
        let x = v24_parts(queue, 0x11, 0x22)[0].clone();
        let y = v24_parts(queue, 0x11, 0x22)[1].clone();
        let z = v24_parts(queue, 0x11, 0x33)[1].clone();
        let p = v24_parts(queue, 0x44, 0x33)[0].clone();

        // Old X, then new Z: X opens no entry; Z opens one with only index 1; no completion.
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, x.clone(), 1.0).unwrap(), None);
        assert_eq!(r.incomplete_len(), 0, "queue {queue}: X opens nothing");
        assert_eq!(r.ingest(1, z.clone(), 1.0).unwrap(), None);
        assert_eq!(r.incomplete_len(), 1, "queue {queue}");

        // New Z, then old X: X fills no slot, no fabricated (X, Z).
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, z.clone(), 1.0).unwrap(), None);
        assert_eq!(r.ingest(1, x.clone(), 1.0).unwrap(), None, "queue {queue}");
        assert_eq!(r.incomplete_len(), 1);
        // Then new P fills index 0 and completes (P, Z) once.
        let done = r.ingest(1, p.clone(), 1.0).unwrap().expect("(P, Z)");
        let mut expected = vec![0x44; 436];
        expected.push(0x33);
        assert_eq!(done.payload, expected, "queue {queue}");
        assert_eq!(r.incomplete_len(), 0);

        // Repeated old Y while the new entry is open: ignored, no overwrite, no new entry.
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, p.clone(), 1.0).unwrap(), None);
        let before = r.clone();
        let dups = r.counts().duplicates;
        assert_eq!(r.ingest(1, y.clone(), 1.0).unwrap(), None);
        assert_eq!(r.counts().duplicates, dups + 1, "queue {queue}");
        assert_eq!(r.incomplete_len(), 1);
        // The open entry still takes its real last part.
        assert!(r.ingest(1, z.clone(), 1.0).unwrap().is_some());
        let mut r = before;

        // A different non-old part for an already filled index: the whole datagram drops, the
        // prior entry kept, no sibling delivered.
        let other = v24_parts(queue, 0x55, 0x33)[0].clone();
        let sibling = split_blob(4, 1, 1, 0, None, 0x77, &[1])
            .expect("split")
            .remove(0);
        let snapshot = r.clone();
        assert_eq!(
            r.ingest_packet(1, &[sibling, other], 1.0),
            Err(SessionError::ConflictingBlobIdentity),
            "queue {queue}"
        );
        assert!(r.same_state(&snapshot));

        // An exact repeat of the filled current non-old part: no change, accepted.
        assert_eq!(r.ingest(1, p.clone(), 1.0).unwrap(), None);
        assert_eq!(r.incomplete_len(), 1);
    }
}

/// Vector 24 row 4 with the entry opened by new Z, which holds index 1: old Y lands on a FILLED
/// index and is still an exact old part, ignored and counted, never a conflict (§9.3.9 "Retained
/// old parts": compare against every retained tombstone, "even if another body of that identity
/// is open"). The mirror case, an entry opened by new P at index 0 then old X, is the same. A
/// datagram carrying old Y with new P is accepted whole and completes (P, Z) once.
#[test]
fn vector_24_old_part_at_a_filled_index_is_ignored() {
    for queue in [1u8, 2] {
        let x = v24_parts(queue, 0x11, 0x22)[0].clone();
        let y = v24_parts(queue, 0x11, 0x22)[1].clone();
        let z = v24_parts(queue, 0x11, 0x33)[1].clone();
        let p = v24_parts(queue, 0x44, 0x33)[0].clone();

        // New Z opens index 1, then old Y at that filled index: ignored, no overwrite.
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, z.clone(), 1.0).unwrap(), None);
        let dups = r.counts().duplicates;
        assert_eq!(r.ingest(1, y.clone(), 1.1), Ok(None), "queue {queue}");
        assert_eq!(r.counts().duplicates, dups + 1, "queue {queue}");
        assert_eq!(r.incomplete_len(), 1);
        // Z, not Y, stays in the slot: new P completes (P, Z).
        let done = r.ingest(1, p.clone(), 1.2).unwrap().expect("(P, Z)");
        let mut expected = vec![0x44; 436];
        expected.push(0x33);
        assert_eq!(done.payload, expected, "queue {queue}");

        // Mirror: new P opens index 0, then old X at that filled index: ignored.
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, p.clone(), 1.0).unwrap(), None);
        let dups = r.counts().duplicates;
        assert_eq!(r.ingest(1, x.clone(), 1.1), Ok(None), "queue {queue}");
        assert_eq!(r.counts().duplicates, dups + 1, "queue {queue}");
        assert_eq!(r.incomplete_len(), 1);
        let done = r.ingest(1, z.clone(), 1.2).unwrap().expect("(P, Z)");
        assert_eq!(done.payload, expected, "queue {queue}");

        // One datagram carrying old Y with new P after Z opened index 1: accepted whole, old Y
        // ignored, P completes (P, Z) once.
        let mut r = v24_after_xy(queue);
        assert_eq!(r.ingest(1, z.clone(), 1.0).unwrap(), None);
        let done = r
            .ingest_packet(1, &[y.clone(), p.clone()], 1.1)
            .expect("the datagram is accepted");
        assert_eq!(done.len(), 1, "queue {queue}");
        assert_eq!(done[0].payload, expected, "queue {queue}");
        assert_eq!(r.incomplete_len(), 0);
    }
}

/// Vector 5's connection after its challenge: masked packets verify with the first incoming word.
fn v24_challenged() -> SessionConnection {
    let mut conn = started();
    conn.receive_datagram(&hex(VECTOR_5_CHALLENGE), SERVER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    while conn.pop_outbound_datagram().is_some() {}
    conn
}

/// A server datagram to Vector 5's recipient with one raw fragment `header` then `data`: masked as
/// sequence 2 with the first incoming word (Vector 7), or unmasked and unsequenced.
fn v24_raw(header: &[u8], data: &[u8], masked: bool) -> Vec<u8> {
    let mut payload = header.to_vec();
    payload.extend_from_slice(data);
    let mut hdr = DatagramHeader {
        rec_id: 0x0102,
        flags: FLAG_HAS_FRAGMENTED_BLOBS | if masked { 0x0000_0002 } else { 0 },
        seq_num: if masked { 2 } else { 0 },
        pak_size: u16::try_from(payload.len()).expect("small"),
        ..DatagramHeader::default()
    };
    let sum = compute_datagram_checksum(header).wrapping_add(compute_datagram_checksum(data));
    let mask = masked.then_some(0x828E_2147);
    hdr.checksum = combine_packet_checksum(hdr, sum, mask).expect("checksum");
    let mut d = vec![0u8, 0];
    d.extend_from_slice(&hdr.to_wire_bytes());
    d.extend_from_slice(&payload);
    d
}

/// Nothing changed on the connection: no event, no reply, State 2 kept.
fn v24_untouched(conn: &mut SessionConnection) {
    assert_eq!(conn.state(), ConnectionState::ChallengeAcceptSent);
    assert_eq!(conn.recv_seq_expected(), 1);
    assert!(conn.pop_event().is_none());
    assert!(conn.pop_outbound_datagram().is_none());
}

const VECTOR_24_UNMASKED: &str =
    "00 00 12 34 00 09 00 00 00 04 00 00 00 02 D0 F3 BB 66 00 02 56 78 03 42 00 01 00 80 F4 01 00";

/// The unmasked-fragment fixture: its keyless checksum is valid, and it still drops before any
/// baseline, Ack or terminal state. The analogous unmasked queue-1 and ordinary queue-3 fragments
/// drop too; masked queue-1 0x02 is accepted, masked known-layout queue-3 0x02 is terminal.
#[test]
fn vector_24_unmasked_fragments_drop_on_every_queue() {
    let fixture = hex(VECTOR_24_UNMASKED);
    let pkt = decode_datagram(&fixture, true, None, false).expect("the checksum is valid");
    assert_eq!(pkt.header.checksum, 0xD0F3_BB66);
    assert_eq!(pkt.fragments[0].header.queue_id, 3);
    let mut conn = SessionConnection::new(SERVER, 0.0);
    assert_eq!(
        conn.receive_datagram(&fixture, SERVER, 0.1, WireCodecs::NONE),
        Err(SessionError::UnmaskedFragment { queue_id: 3 })
    );
    let mut conn = v24_challenged();
    assert_eq!(
        conn.receive_datagram(&fixture, SERVER, 0.2, WireCodecs::NONE),
        Err(SessionError::UnmaskedFragment { queue_id: 3 })
    );
    v24_untouched(&mut conn);

    // The analogous unmasked queue-1 (ordinary, and with 0x02) and ordinary queue-3 fragments.
    let q1 = [0x01, 0x40, 0x80, 0xF4, 0x01, 0x00];
    let q1_02 = [0x01, 0x42, 0x80, 0xF4, 0x01, 0x00];
    let q3 = [0x03, 0x40, 0x00, 0x01, 0x00, 0x80, 0xF4, 0x01];
    for (header, queue) in [(&q1[..], 1u8), (&q1_02[..], 1), (&q3[..], 3)] {
        let d = v24_raw(header, &[0x00], false);
        assert_eq!(
            conn.receive_datagram(&d, SERVER, 0.3, WireCodecs::NONE),
            Err(SessionError::UnmaskedFragment { queue_id: queue })
        );
        v24_untouched(&mut conn);
    }

    // Masked queue-1 0x02: the normal accept row.
    let d = v24_raw(&q1_02, &[0x00], true);
    conn.receive_datagram(&d, SERVER, 0.4, WireCodecs::NONE)
        .expect("accepted");
    assert_eq!(conn.state(), ConnectionState::Connected);
    // Masked known-layout ordinary queue-3 0x02: terminal.
    let mut conn = v24_challenged();
    let q3_02 = [0x03, 0x42, 0x00, 0x01, 0x00, 0x80, 0xF4, 0x01];
    let d = v24_raw(&q3_02, &[0x00], true);
    assert!(matches!(
        conn.receive_datagram(&d, SERVER, 0.4, WireCodecs::NONE),
        Err(SessionError::UnsupportedFragment { queue_id: 3, .. })
    ));
    assert_eq!(conn.state(), ConnectionState::Closing);
}

/// Empty controls: `01 C0` accepts with no dispatch, baseline or tombstone. Fragmentation, 0x02,
/// encryption 0x08 or the undefined 0x20 with OmitChunkInfo is an invalid control: the whole datagram
/// drops, and a 0x82 form never terminates.
#[test]
fn vector_24_empty_and_invalid_controls() {
    let mut conn = v24_challenged();
    let d = v24_raw(&[0x01, 0xC0], &[], true);
    conn.receive_datagram(&d, SERVER, 0.2, WireCodecs::NONE)
        .expect("an empty control is accepted");
    assert_eq!(conn.state(), ConnectionState::Connected);
    assert!(
        !std::iter::from_fn(|| conn.pop_event())
            .any(|e| matches!(e, SessionEvent::BlobReceived(_))),
        "no dispatch"
    );

    let invalid: [(&[u8], u8, u8); 5] = [
        (&[0x01, 0xD0, 0x01, 0x00, 0x00, 0x02], 1, 0xD0),
        (
            &[
                0x02, 0x82, 0x01, 0x00, 0x01, 0x00, 0x77, 0, 0, 0, 0, 0, 0, 0x01,
            ],
            2,
            0x82,
        ),
        (&[0x01, 0xC2], 1, 0xC2),
        (&[0x01, 0xC8], 1, 0xC8),
        (&[0x01, 0xE0], 1, 0xE0),
    ];
    for (header, queue_id, blob_flags) in invalid {
        let expected = Err(SessionError::InvalidOmittedControl {
            queue_id,
            blob_flags,
        });
        assert_eq!(
            BlobFragment::decode(header).map(|_| ()),
            expected.clone(),
            "{header:02X?}"
        );
        let mut conn = v24_challenged();
        let d = v24_raw(header, &[], true);
        assert_eq!(
            conn.receive_datagram(&d, SERVER, 0.2, WireCodecs::NONE)
                .map(|_| ()),
            expected,
            "{header:02X?}"
        );
        v24_untouched(&mut conn);
    }
}
