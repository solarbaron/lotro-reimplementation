// SPDX-License-Identifier: GPL-3.0-or-later
//! `Packet::decode` on arbitrary bytes: must never panic, and anything it
//! accepts must re-encode to the same bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_net::transport::packet::Packet;

fuzz_target!(|data: &[u8]| {
    if let Ok(packet) = Packet::decode(data) {
        let mut out = Vec::with_capacity(data.len());
        packet.encode(&mut out).expect("decoded packet re-encodes");
        assert_eq!(out, data, "decode/encode round trip");
    }
});
