// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::session`: datagram framing, optional
//! headers, `SessionAuthHeader`, dictionary records, blob fragments and reassembly, 3DES,
//! the connection state machine, the multiplexer and message-envelope unpacking on
//! arbitrary hostile bytes (`docs/specs/official-session.md`).
#![no_main]

use std::net::{Ipv4Addr, SocketAddrV4};

use libfuzzer_sys::fuzz_target;
use tessera_compat::session::{
    AuthConnection, BindPorts, BitTreeTable, BlobFragment, BlobFragmentHeader, BlobReassembler,
    CodecUuids, ConnectAcceptHeader, ConnectChallengeHeader, ControlMessage, DatagramHeader,
    OPCODE_BATCH_CONTAINER, OPCODE_CLIENT_VERSION, OPCODE_LOGIN_QUEUE_STATUS,
    OPCODE_SINGLE_WRAPPER, OPCODE_SYSTEM_NOTIFICATION, OPCODE_VERSION_REQUEST,
    OPCODE_VERSION_STATUS, OptionalHeaders, SessionAuthHeader, SessionConnection,
    SessionMultiplexer, WireCodecs, WorldReferralHeader, check_ticket_text,
    compute_datagram_checksum, decode_datagram, decode_message, decode_variable_u32,
    des3_cbc_decrypt, dispatch_message, encode_datagram, encode_variable_u32, guid_fields_swapped,
    is_valid_net_version, max_recv_rate_for_speed, parse_packet_payload, unique_port_candidates,
};

/// The synthetic version stamp (`official-session.md` §10.2 Vector 6).
const VERSION: &str = "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10";

/// An account-only `SessionAuthHeader` of the primary connection (§6.2.1 rule 3).
fn account_auth(
    account: &str,
    timestamp: u32,
) -> Result<SessionAuthHeader, tessera_compat::session::SessionError> {
    SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        VERSION,
        account,
        None,
        None,
        timestamp,
    )
}

/// A synthetic record DID (the real one is resolved at run time).
const SYNTHETIC_DICT_DID: u32 = 0x0E01_0001;

/// Synthetic directional dictionary UUIDs: only a connection whose challenge named the
/// incoming one decompresses (official-session.md §4.1.1, §7.3).
const FUZZ_CODEC_UUIDS: CodecUuids = CodecUuids {
    incoming: [
        0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0,
        0x01,
    ],
    outgoing: [0x5A; 16],
};

fuzz_target!(|data: &[u8]| {
    let _ = compute_datagram_checksum(data);
    // The compact integer (§5.6.1): whatever decodes re-encodes to a value of the same
    // tier width or less, and decodes back.
    if let Ok((value, used)) = decode_variable_u32(data) {
        let mut out = Vec::new();
        let written = encode_variable_u32(value, &mut out).expect("a decoded value encodes");
        assert!(written <= used);
        assert_eq!(decode_variable_u32(&out), Ok((value, written)));
    }
    let _ = DatagramHeader::from_wire_bytes(data);
    let _ = SessionAuthHeader::decode(data);
    let _ = ConnectChallengeHeader::decode(data);
    let _ = ConnectAcceptHeader::decode(data);
    let _ = WorldReferralHeader::decode(data);
    let _ = BlobFragmentHeader::decode(data);

    let _ = BitTreeTable::from_dat_bytes(data);
    let _ = BitTreeTable::parse_redirect_did(data, SYNTHETIC_DICT_DID);
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = check_ticket_text(text);
        let _ = is_valid_net_version(text);
        // The first datagram of either connection with a hostile ticket and version stamp
        // (§6.2–§6.2.1): it either encodes and decodes back, or is refused.
        for connection in [AuthConnection::Primary, AuthConnection::Chat] {
            if let Ok(auth) =
                SessionAuthHeader::for_connection(connection, text, "A", Some(text), None, 0)
            {
                let mut wire = Vec::new();
                if auth.encode(&mut wire).is_ok() {
                    assert_eq!(
                        SessionAuthHeader::decode(&wire).ok().map(|(h, _)| h),
                        Some(auth)
                    );
                }
            }
        }
        let _ = tessera_compat::net_access::TlsFailure::from_tls_text(text);
    }
    // The handshake diagnostics read a refused datagram once more (no checksum, no codec).
    let _ = tessera_compat::connector::diag::carries_challenge(data);
    // The per-buffer checksum (official-session.md §4.4.2) is the same on send and on
    // receive: what decoded and re-encodes verifies.
    if let Ok(pkt) = decode_datagram(data, true, None, false) {
        let mask = (pkt.header.flags & tessera_compat::session::FLAG_MASKED_CHECKSUM != 0)
            .then_some(0x1234_5678);
        if let Ok(again) = encode_datagram(pkt.header, &pkt.opt_headers, &pkt.fragments, mask, None)
        {
            let back = decode_datagram(&again, true, None, false).expect("re-encoded decodes");
            assert!(
                tessera_compat::session::verify_packet_checksum(
                    back.header,
                    back.payload_sum,
                    mask
                )
                .is_ok()
            );
        }
    }

    let tables = BitTreeTable::synthetic_dictionary([0u8; 16]);
    let _ = decode_datagram(data, true, None, true);
    let _ = decode_datagram(data, false, Some(&tables), false);

    if data.len() >= 4 {
        let flags = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = OptionalHeaders::decode(flags, &data[4..], true);
        let _ = OptionalHeaders::decode(flags, &data[4..], false);
        let _ = parse_packet_payload(flags, &data[4..], true);
        let _ = guid_fields_swapped(
            data.get(..16)
                .and_then(|b| <[u8; 16]>::try_from(b).ok())
                .unwrap_or_default(),
        );
    }

    if let Ok((frag, _)) = BlobFragment::decode(data) {
        let mut reassembler = BlobReassembler::new(8, 65_536);
        let _ = reassembler.ingest(0x0100, frag, 1.0);
    }

    let key = [0x3Cu8; 24];
    let _ = des3_cbc_decrypt(&key, data);

    // §3.2 Case B candidates and the §6.4 rate for arbitrary inputs.
    if let Some(octets) = data.get(..4) {
        let ip = Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]);
        let ports = unique_port_candidates(ip);
        assert!(ports.len() <= tessera_compat::session::MAX_UNIQUE_PORT_ATTEMPTS);
        assert!(ports.iter().all(|&p| p >= 1024));
        let _ = BindPorts::select(0, true, Some(ip)).candidates();
    }
    if let Some(speed) = data.get(..8).and_then(|b| <[u8; 8]>::try_from(b).ok()) {
        let _ = max_recv_rate_for_speed(f64::from_le_bytes(speed));
    }

    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 9000);
    let codecs = WireCodecs {
        incoming: Some(&tables),
        outgoing: Some(&tables),
    };
    let mut conn = SessionConnection::new(peer, 0.0);
    let _ = conn.receive_datagram(data, peer, 0.5, codecs);
    // In State 1 a ConnectChallengeHeader is acted on (official-session.md §6.4). The
    // dictionary UUIDs let a challenge that names the incoming one attach the codecs (§7.3).
    let mut started = SessionConnection::new(peer, 0.0);
    started.set_codec_uuids(FUZZ_CODEC_UUIDS);
    if let Ok(auth) = account_auth("A", 0)
        && started.start_login(auth, 0.0).is_ok()
    {
        let _ = started.receive_datagram(data, peer, 0.5, codecs);
        let _ = started.receive_datagram(data, peer, 0.6, WireCodecs::NONE);
        let _ = started.tick(1.0);
    }
    // A connection whose synthetic challenge negotiated a codec takes compressed datagrams.
    let mut negotiated = SessionConnection::new(peer, 0.0);
    negotiated.set_codec_uuids(FUZZ_CODEC_UUIDS);
    let challenge = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            outgoing_rec_id: 0x0304,
            incoming_seed: 2,
            outgoing_seed: 1,
            incoming_codec_guid: guid_fields_swapped(FUZZ_CODEC_UUIDS.incoming),
            ..ConnectChallengeHeader::default()
        }),
        ..OptionalHeaders::default()
    };
    let challenge_header = DatagramHeader {
        rec_id: 0x0102,
        ..DatagramHeader::default()
    };
    if let Ok(auth) = account_auth("A", 0)
        && negotiated.start_login(auth, 0.0).is_ok()
        && let Ok(dgram) = encode_datagram(challenge_header, &challenge, &[], None, None)
        && negotiated
            .receive_datagram(&dgram, peer, 0.1, WireCodecs::NONE)
            .is_ok()
    {
        let _ = negotiated.receive_datagram(data, peer, 0.5, codecs);
    }

    let _ = decode_message(1, data, 65_536);
    let _ = dispatch_message(OPCODE_BATCH_CONTAINER, data, 65_536, 4);
    let _ = dispatch_message(OPCODE_SINGLE_WRAPPER, data, 65_536, 4);
    for opcode in [
        OPCODE_CLIENT_VERSION,
        OPCODE_VERSION_STATUS,
        OPCODE_SYSTEM_NOTIFICATION,
        OPCODE_VERSION_REQUEST,
        OPCODE_LOGIN_QUEUE_STATUS,
    ] {
        let _ = ControlMessage::decode(opcode, data);
    }

    let mut mux = SessionMultiplexer::new();
    if let Ok(id) = mux.create_connection(peer, 0.0) {
        if let (Some(conn), Ok(auth)) = (mux.connection_mut(id), account_auth("A", 0)) {
            conn.set_codec_uuids(FUZZ_CODEC_UUIDS);
            let _ = conn.start_login(auth, 0.0);
        }
        let _ = mux.receive_from_peer(data, peer, 0.5, WireCodecs::NONE);
        let _ = mux.receive_from_peer(data, peer, 0.6, codecs);
        let _ = mux.tick(30.0);
    }
});
