// SPDX-License-Identifier: GPL-3.0-or-later
//! `FrameDecoder` (tessera-protocol.md §5.4) on arbitrary stream bytes, for every stream
//! channel and sender, fed in arbitrary chunk sizes: must never panic, never buffer more than
//! one frame, and every message it yields must re-encode and decode to itself.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::frame::{FrameDecoder, decode_frame_message, frame_to_vec, split_frame};
use tessera_protocol::limits::MAX_FRAME_BYTES;
use tessera_protocol::{Channel, Side};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let channel = match selector % 3 {
        0 => Channel::Control,
        1 => Channel::World,
        _ => Channel::Chat,
    };
    let sender = if selector & 0x80 != 0 {
        Side::Server
    } else {
        Side::Client
    };
    let chunk = usize::from(selector >> 2 & 0x1f) + 1;
    let mut d = FrameDecoder::new(channel, sender);
    'outer: for piece in rest.chunks(chunk) {
        let mut input = piece;
        loop {
            match d.push(&mut input) {
                Ok(Some(m)) => {
                    let bytes = frame_to_vec(&m).expect("decoded message re-encodes");
                    let (raw, used) = split_frame(&bytes).unwrap().unwrap();
                    assert_eq!(used, bytes.len());
                    let back = decode_frame_message(channel, sender, raw.ty, raw.body)
                        .expect("re-encoded message decodes");
                    assert_eq!(back, m);
                }
                Ok(None) => break,
                Err(_) => break 'outer,
            }
            assert!(d.buffered() <= MAX_FRAME_BYTES);
        }
    }
});
