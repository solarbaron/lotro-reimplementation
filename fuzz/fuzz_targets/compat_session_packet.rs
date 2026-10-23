// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::session`: datagram framing, optional
//! headers, blob fragments and reassembly, 3DES, the connection state machine,
//! the multiplexer and message-envelope unpacking on arbitrary hostile bytes.
#![no_main]

use std::net::{Ipv4Addr, SocketAddrV4};

use libfuzzer_sys::fuzz_target;
use tessera_compat::session::{
    BlobFragment, BlobFragmentHeader, BlobReassembler, ConnectAcceptHeader, ConnectChallengeHeader,
    DatagramHeader, OPCODE_BATCH_CONTAINER, OPCODE_SINGLE_WRAPPER, OptionalHeaders,
    RuntimeBitTreeTables, SessionAuthHeader, SessionConnection, SessionMultiplexer,
    WorldReferralHeader, compute_datagram_checksum, decode_datagram, decode_message,
    decode_variable_u32, des3_cbc_decrypt, dispatch_message,
};

fuzz_target!(|data: &[u8]| {
    let _ = compute_datagram_checksum(data);
    let _ = decode_variable_u32(data);
    let _ = DatagramHeader::from_wire_bytes(data);
    let _ = SessionAuthHeader::decode(data);
    let _ = ConnectChallengeHeader::decode(data);
    let _ = ConnectAcceptHeader::decode(data);
    let _ = WorldReferralHeader::decode(data);
    let _ = BlobFragmentHeader::decode(data);

    let tables = RuntimeBitTreeTables::synthetic_canonical();
    let _ = decode_datagram(data, true, None, true);
    let _ = decode_datagram(data, false, Some(&tables), false);

    if data.len() >= 4 {
        let flags = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = OptionalHeaders::decode(flags, &data[4..], true);
        let _ = OptionalHeaders::decode(flags, &data[4..], false);
    }

    if let Ok((frag, _)) = BlobFragment::decode(data) {
        let mut reassembler = BlobReassembler::new(8, 65_536);
        let key = [0x5Au8; 24];
        let _ = reassembler.ingest(0x0100, frag, 1.0, Some(&key));
    }

    let key = [0x3Cu8; 24];
    let _ = des3_cbc_decrypt(&key, data);

    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 9000);
    let mut conn = SessionConnection::new(0x0100, peer, 0.0);
    conn.set_des3_key(key);
    let _ = conn.receive_datagram(data, peer, 0.5, Some(&tables));

    let _ = decode_message(1, data, false, 65_536);
    let _ = dispatch_message(OPCODE_BATCH_CONTAINER, data, false, 65_536, 4);
    let _ = dispatch_message(OPCODE_SINGLE_WRAPPER, data, true, 65_536, 4);

    let mut mux = SessionMultiplexer::new();
    if mux.create_connection(peer, 0.0).is_ok() {
        let _ = mux.receive_from_peer(data, peer, 0.5, None);
        let _ = mux.receive_from_peer(data, peer, 0.6, Some(&tables));
    }
});
