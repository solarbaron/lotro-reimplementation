// SPDX-License-Identifier: GPL-3.0-or-later
//! `Message::decode_body` (tessera-protocol.md §6) for every message type on arbitrary
//! bodies, at negotiated minor 0 or 1 (§3.1: looks and held-item slots): must never panic,
//! and anything it accepts must re-encode, decode to the same message, and re-encode to the
//! same bytes at that minor.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::{Message, MessageType};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    let types = MessageType::ALL;
    let ty = types[usize::from(selector & 0x7F) % types.len()];
    let minor = u16::from(selector >> 7);
    if let Ok(m) = Message::decode_body_minor(ty, body, minor) {
        assert_eq!(m.message_type(), ty);
        let mut out = Vec::new();
        m.encode_body_minor(&mut out, minor)
            .expect("decoded message re-encodes");
        let back = Message::decode_body_minor(ty, &out, minor).expect("re-encoded body decodes");
        assert_eq!(back, m);
        let mut again = Vec::new();
        back.encode_body_minor(&mut again, minor).unwrap();
        assert_eq!(again, out);
        // A message decoded at minor 1 always projects onto the v1.0 layout (§15
        // invariant 10).
        let mut v10 = Vec::new();
        if m.encode_body_minor(&mut v10, 0).is_ok() {
            assert!(Message::decode_body_minor(ty, &v10, 0).is_ok());
        }
    }
});
