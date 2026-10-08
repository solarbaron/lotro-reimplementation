// SPDX-License-Identifier: GPL-3.0-or-later
//! `Message::decode_body` (tessera-protocol.md §6) for every message type on arbitrary
//! bodies: must never panic, and anything it accepts must re-encode, decode to the same
//! message, and re-encode to the same bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::{Message, MessageType};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    let types = MessageType::ALL;
    let ty = types[usize::from(selector) % types.len()];
    if let Ok(m) = Message::decode_body(ty, body) {
        assert_eq!(m.message_type(), ty);
        let mut out = Vec::new();
        m.encode_body(&mut out).expect("decoded message re-encodes");
        let back = Message::decode_body(ty, &out).expect("re-encoded body decodes");
        assert_eq!(back, m);
        let mut again = Vec::new();
        back.encode_body(&mut again).unwrap();
        assert_eq!(again, out);
    }
});
