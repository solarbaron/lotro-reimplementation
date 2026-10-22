// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]

use std::net::{Ipv4Addr, SocketAddrV4};

use libfuzzer_sys::fuzz_target;
use tessera_compat::session::{
    ConnectRequestHeader, ConnectResponseHeader, LoginRequestHeader, NetBlobFragment,
    NetBlobFragmentHeader, NetBlobReassembler, OPCODE_BATCH_CONTAINER, OPCODE_SINGLE_WRAPPER,
    OptionalHeaders, ProtoHeader, ReferralHeader, RuntimeBitTreeTables, SessionConnection,
    calc_checksum, decode_datagram, decode_net_message, decode_packed_dword, des3_cbc_decrypt,
    dispatch_net_message,
};

fuzz_target!(|data: &[u8]| {
    let _ = calc_checksum(data);
    let _ = decode_packed_dword(data);
    let _ = ProtoHeader::from_wire_bytes(data);
    let _ = LoginRequestHeader::decode(data);
    let _ = ConnectRequestHeader::decode(data);
    let _ = ConnectResponseHeader::decode(data);
    let _ = ReferralHeader::decode(data);
    let _ = NetBlobFragmentHeader::decode(data);

    let tables = RuntimeBitTreeTables::synthetic_canonical();
    let _ = decode_datagram(data, true, None, true);
    let _ = decode_datagram(data, false, Some(&tables), false);

    if data.len() >= 4 {
        let flags = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = OptionalHeaders::decode(flags, &data[4..], true);
        let _ = OptionalHeaders::decode(flags, &data[4..], false);
    }

    if let Ok((frag, _)) = NetBlobFragment::decode(data) {
        let mut reassembler = NetBlobReassembler::new(8, 65_536);
        let key = [0x5Au8; 24];
        let _ = reassembler.ingest(0x0100, frag, 1.0, Some(&key));
    }

    let key = [0x3Cu8; 24];
    let _ = des3_cbc_decrypt(&key, data);

    let peer = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 9000);
    let mut conn = SessionConnection::new(0x0100, peer, 0.0);
    conn.set_des3_key(key);
    let _ = conn.receive_datagram(data, peer, 0.5, Some(&tables));

    let _ = decode_net_message(1, data, false, 65_536);
    let _ = dispatch_net_message(OPCODE_BATCH_CONTAINER, data, false, 65_536, 4);
    let _ = dispatch_net_message(OPCODE_SINGLE_WRAPPER, data, true, 65_536, 4);
});
