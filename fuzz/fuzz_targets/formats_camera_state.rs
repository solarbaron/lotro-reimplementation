// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera-state records (prefix 0x08, `docs/specs/walker.md` §11.5) on arbitrary bytes. The
//! first byte selects whether the rest is the payload as is, or follows a header with valid
//! flags and a controller type 1–4 taken from the selector, so that short inputs reach the
//! controller bodies.
//!
//! Nothing may panic. An accepted record re-encodes byte for byte and its size matches the
//! controller type.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_camera_state -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::environment::{CameraState, decode_camera_state};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(24 + rest.len());
    out.extend_from_slice(&0x4700_0002u32.to_le_bytes());
    for v in [0.15f32, 0.1, 2000.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[1, 0, 1, 0]);
    out.extend_from_slice(&(u32::from((mode >> 1) & 3) + 1).to_le_bytes());
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(c) = decode_camera_state(data) else {
        return;
    };
    let bytes = c.to_bytes();
    assert_eq!(bytes, data, "exact re-encoding");
    assert_eq!(c.encoded_len(), data.len());
    let again = CameraState::parse(&bytes).expect("re-encoded record decodes");
    assert_eq!(again.body.controller_type(), c.body.controller_type());
    let _ = c.body.fov();
    let _ = c.body.target();
});
