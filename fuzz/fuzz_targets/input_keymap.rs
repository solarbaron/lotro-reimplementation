// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input` keymap file (docs/specs/keymap.md §5-§6): binding strings and the TOML
//! document must never panic; the lenient and strict parsers must agree on documents both
//! accept; our own output must parse strictly back to the same keymap; overlay/diff must
//! round-trip; and any keymap must compile against the built-in registry. Bindings built
//! directly from the input bytes (hostile `f32`s, any mouse button number) must either be
//! refused by `Keymap::set` or survive a save and a strict reload unchanged.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_input::{
    AnalogAxis, BindingSpec, Button, Chord, InputMapper, Keymap, Modifiers, MouseButton, Stroke,
    default_keymap, default_registry,
};

/// Build bindings straight from bytes, bypassing the parser.
fn constructed(data: &[u8]) {
    let mut k = Keymap::new();
    for (i, c) in data.as_chunks::<11>().0.iter().take(32).enumerate() {
        let f = |o: usize| f32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]);
        let spec = if c[0] & 1 == 0 {
            let axis = AnalogAxis::ALL[usize::from(c[1]) % AnalogAxis::ALL.len()];
            BindingSpec::Analog {
                axis,
                scale: f(2),
                deadzone: f(6),
            }
        } else {
            let n = u16::from_le_bytes([c[1], c[10]]);
            let stroke = Stroke::button(Button::Mouse(MouseButton::Other(n)));
            let stroke = if c[0] & 2 == 0 {
                stroke
            } else {
                match Stroke::new(
                    Modifiers::from_bits_truncate(c[0] >> 4),
                    stroke.buttons().to_vec(),
                ) {
                    Ok(s) => s,
                    Err(_) => continue,
                }
            };
            BindingSpec::Digital {
                chord: Chord::single(stroke),
                value: f(2),
            }
        };
        let action = format!("a{i}");
        let _ = k.set("c", &action, vec![spec]);
    }
    k.options.sequence_timeout_ms = data
        .get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    match k.to_toml_string() {
        Ok(out) => {
            let again = Keymap::parse(&out).expect("constructed keymap parses strictly");
            assert_eq!(again, k, "constructed keymap changed in a round trip");
        }
        Err(_) => {
            // Only the out-of-range timeout may be refused.
            k.options.sequence_timeout_ms = None;
            let out = k.to_toml_string().expect("normalized bindings serialize");
            assert_eq!(Keymap::parse(&out).expect("strict"), k);
        }
    }
}

fn same_bindings(a: &Keymap, b: &Keymap) -> bool {
    a.sequence_timeout_ms() == b.sequence_timeout_ms() && a.iter().eq(b.iter())
}

fuzz_target!(|data: &[u8]| {
    constructed(data);
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    for line in text.lines().take(64) {
        if let Ok(c) = Chord::parse(line) {
            let shown = c.to_string();
            assert_eq!(Chord::parse(&shown).ok().as_ref(), Some(&c), "{shown}");
        }
    }
    let parsed = Keymap::parse_lenient(text);
    if let Ok(strict) = Keymap::parse(text) {
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.keymap, strict);
    }
    let km = &parsed.keymap;
    if let Ok(out) = km.to_toml_string() {
        let again = Keymap::parse(&out).expect("own output parses strictly");
        assert_eq!(&again, km, "round trip changed the keymap");
    }
    let base = default_keymap();
    let eff = base.overlay(km);
    let diff = eff.diff_from(&base);
    assert!(
        same_bindings(&base.overlay(&diff), &eff),
        "overlay/diff mismatch"
    );
    let (mut mapper, _warnings) = InputMapper::new(default_registry(), &eff);
    mapper.update(std::time::Duration::ZERO);
});
