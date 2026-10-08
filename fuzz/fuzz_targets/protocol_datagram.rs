// SPDX-License-Identifier: GPL-3.0-or-later
//! `decode_datagram` (tessera-protocol.md §5.5, §6.5) on arbitrary bytes from either side,
//! and property values (§7.2): must never panic; accepted input re-encodes byte-exactly
//! (datagram messages carry no text, so there is nothing to sanitise).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::Side;
use tessera_protocol::frame::{decode_datagram, encode_datagram};
use tessera_protocol::value::Value;

fuzz_target!(|data: &[u8]| {
    for side in [Side::Client, Side::Server] {
        if let Ok(m) = decode_datagram(side, data) {
            let out = encode_datagram(&m).expect("decoded datagram re-encodes");
            assert_eq!(out, data, "datagram round trip");
        }
    }
    for allow_none in [false, true] {
        if let Ok((v, used)) = Value::decode(data, allow_none) {
            assert!(used <= data.len());
            let mut out = Vec::new();
            v.encode(&mut out, allow_none)
                .expect("decoded value re-encodes");
            let (back, n) = Value::decode(&out, allow_none).expect("re-encoded value decodes");
            assert_eq!(n, out.len());
            assert_eq!(back, v);
        }
    }
});
