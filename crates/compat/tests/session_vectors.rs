// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic test vectors of `docs/specs/official-session.md` §10.2 (Vectors 1–15), byte
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
        assert!(
            !std::iter::from_fn(|| conn.pop_event())
                .any(|e| matches!(e, SessionEvent::WorldReferralReceived { .. }))
        );
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
    // The flow report of the tick took sequence 3 and word 3; the NAK copies them and takes none.
    assert_eq!((nak.header.seq_num, nak.header.client_word), (3, 3));
    assert_eq!(
        nak.header.flags & tessera_compat::session::FLAG_MASKED_CHECKSUM,
        0
    );
    assert_eq!(conn.send_seq_num(), 3, "no sequence allocated");

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
    conn.queue_standalone_disconnect().expect("queued");
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
