// SPDX-License-Identifier: GPL-3.0-or-later
//! The quantised movement input of the character controller
//! (`tessera_physics::controller::{decode_input, pack_input}`, `walker.md` §11.8 items 4
//! and 5): every input byte is a packed value to decode, and every four input bytes are an
//! `f32` (any bits: NaN, infinities, huge values) to pack as a forward and a strafe value.
//!
//! Nothing may panic; a decoded value is finite and within −1 … 1 on both axes, 0 decodes
//! to (0, 0), a packed value is 1 … 255, and packing a decoded value and decoding it again
//! gives values within −1 … 1.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_physics::controller::{decode_input, pack_input};

fn check(x: f32, y: f32) {
    assert!(x.is_finite() && y.is_finite(), "a decoded input is finite");
    assert!((-1.0..=1.0).contains(&x) && (-1.0..=1.0).contains(&y));
}

fuzz_target!(|data: &[u8]| {
    for &packed in data {
        let (x, y) = decode_input(packed);
        check(x, y);
        if packed == 0 {
            assert_eq!((x, y), (0.0, 0.0), "0 decodes to (0, 0)");
        }
        let again = pack_input(y, x);
        assert!(again >= 1, "a packed input is 1 ... 255");
        let (x2, y2) = decode_input(again);
        check(x2, y2);
    }
    for &[a, b, c, d, e, f, g, h] in data.as_chunks::<8>().0 {
        let forward = f32::from_le_bytes([a, b, c, d]);
        let strafe = f32::from_le_bytes([e, f, g, h]);
        let packed = pack_input(forward, strafe);
        assert!(packed >= 1, "a packed input is 1 ... 255");
        let (x, y) = decode_input(packed);
        check(x, y);
    }
});
