// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::session`: datagram framing, optional
//! headers, `SessionAuthHeader`, dictionary records, blob fragments and reassembly, 3DES,
//! the connection state machine, the multiplexer and message-envelope unpacking on
//! arbitrary hostile bytes (`docs/specs/official-session.md`).
#![no_main]

use std::net::{Ipv4Addr, SocketAddrV4};

use libfuzzer_sys::fuzz_target;
use tessera_compat::session::{
    BitTreeTable, BlobFragment, BlobFragmentHeader, BlobReassembler, ConnectAcceptHeader,
    ConnectChallengeHeader, ControlMessage, DatagramHeader, OPCODE_BATCH_CONTAINER,
    OPCODE_CLIENT_VERSION, OPCODE_LOGIN_QUEUE_STATUS, OPCODE_SINGLE_WRAPPER,
    OPCODE_SYSTEM_NOTIFICATION, OPCODE_VERSION_REQUEST, OPCODE_VERSION_STATUS, OptionalHeaders,
    SessionAuthHeader, SessionConnection, SessionMultiplexer, WireCodecs, WorldReferralHeader,
    compute_datagram_checksum, decode_datagram, decode_message, decode_ticket_base64,
    decode_variable_u32, des3_cbc_decrypt, dispatch_message,
};

/// A synthetic record DID (the real one is resolved at run time).
const SYNTHETIC_DICT_DID: u32 = 0x0E01_0001;

fuzz_target!(|data: &[u8]| {
    let _ = compute_datagram_checksum(data);
    let _ = decode_variable_u32(data);
    let _ = DatagramHeader::from_wire_bytes(data);
    let _ = SessionAuthHeader::decode(data);
    let _ = ConnectChallengeHeader::decode(data);
    let _ = ConnectAcceptHeader::decode(data);
    let _ = WorldReferralHeader::decode(data);
    let _ = BlobFragmentHeader::decode(data);

    let _ = BitTreeTable::from_dat_bytes(data);
    let _ = BitTreeTable::parse_redirect_did(data, SYNTHETIC_DICT_DID);
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = decode_ticket_base64(text);
        let _ = SessionAuthHeader::for_login("A", Some(text), 0);
    }

    let tables = BitTreeTable::synthetic_dictionary([0u8; 16]);
    let _ = decode_datagram(data, true, None, true);
    let _ = decode_datagram(data, false, Some(&tables), false);

    if data.len() >= 4 {
        let flags = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = OptionalHeaders::decode(flags, &data[4..], true);
        let _ = OptionalHeaders::decode(flags, &data[4..], false);
    }

    if let Ok((frag, _)) = BlobFragment::decode(data) {
        let mut reassembler = BlobReassembler::new(8, 65_536);
        let _ = reassembler.ingest(0x0100, frag, 1.0);
    }

    let key = [0x3Cu8; 24];
    let _ = des3_cbc_decrypt(&key, data);

    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 9000);
    let codecs = WireCodecs {
        incoming: Some(&tables),
        outgoing: Some(&tables),
    };
    let mut conn = SessionConnection::new(0x0100, peer, 0.0);
    let _ = conn.receive_datagram(data, peer, 0.5, codecs);
    // In State 1 a ConnectChallengeHeader is acted on (official-session.md §6.4).
    let mut started = SessionConnection::new(0x0100, peer, 0.0);
    if let Ok(auth) = SessionAuthHeader::for_login("A", None, 0)
        && started.start_login(auth, 0.0).is_ok()
    {
        let _ = started.receive_datagram(data, peer, 0.5, codecs);
        let _ = started.receive_datagram(data, peer, 0.6, WireCodecs::NONE);
        let _ = started.tick(1.0);
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
        if let (Some(conn), Ok(auth)) = (
            mux.connection_mut(id),
            SessionAuthHeader::for_login("A", None, 0),
        ) {
            let _ = conn.start_login(auth, 0.0);
        }
        let _ = mux.receive_from_peer(data, peer, 0.5, WireCodecs::NONE);
        let _ = mux.receive_from_peer(data, peer, 0.6, codecs);
        let _ = mux.tick(30.0);
    }
});
