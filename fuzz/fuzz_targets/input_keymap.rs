// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input` keymap file (docs/specs/keymap.md §5-§6): binding strings and the TOML
//! document must never panic; the lenient and strict parsers must agree on documents both
//! accept; our own output must parse strictly back to the same keymap; overlay/diff must
//! round-trip; and any keymap must compile against the built-in registry. Bindings built
//! directly from the input bytes (hostile `f32`s, any mouse button number) must either be
//! refused by `Keymap::set` or survive a save and a strict reload unchanged (a retired
//! `text_entry` name is refused). Documents built
//! from the bytes exercise the `on` field (valid and invalid values; `on` on a sequence, a
//! chord, a modifier hold, a wheel click and an axis action) through both readers and
//! `Keymap::set`, and the migration of the retired `text_entry` actions (§5.3, §6.3).

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_input::{
    Activation, AnalogAxis, BindingSpec, Button, Chord, InputMapper, Keymap, Modifiers,
    MouseButton, RETIRED_TEXT_ENTRY_ACTIONS, Stroke, default_keymap, default_registry,
};

const ON_WORDS: [&str; 9] = [
    "press",
    "release",
    "tap",
    "double",
    "double_nearby",
    "Tap",
    "",
    "hold",
    "press ",
];
const INPUTS: [&str; 9] = [
    "MouseLeft",
    "Ctrl+MouseRight",
    "G G",
    "MouseLeft+MouseRight",
    "Shift",
    "WheelUp",
    "Enter",
    "Ctrl+K Ctrl+C",
    "PadSouth",
];
const ACTIONS: [&str; 9] = [
    "select_left",
    "move_forward",
    "text_submit",
    "text_cancel",
    "text_history_prev",
    "text_history_next",
    "open_chat",
    "menu",
    "text_line_up",
];
const CONTEXTS: [&str; 4] = ["text_entry", "pointer", "movement", "ui.dialog"];

/// A TOML document of `on` entries and retired names, built from the bytes.
fn on_document(data: &[u8]) -> String {
    let mut tables: BTreeMap<&str, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    for c in data.as_chunks::<4>().0.iter().take(24) {
        let ctx = CONTEXTS[usize::from(c[0]) % CONTEXTS.len()];
        let action = ACTIONS[usize::from(c[1]) % ACTIONS.len()];
        let input = INPUTS[usize::from(c[2]) % INPUTS.len()];
        let on = ON_WORDS[usize::from(c[3]) % ON_WORDS.len()];
        let entry = if c[3] & 0x80 != 0 {
            format!("\"{input}\"")
        } else if c[3] & 0x40 != 0 {
            format!("{{ axis = \"MouseX\", on = \"{on}\" }}")
        } else {
            format!("{{ input = \"{input}\", on = \"{on}\" }}")
        };
        tables
            .entry(ctx)
            .or_default()
            .entry(action)
            .or_default()
            .push(entry);
    }
    let mut doc = String::from("version = 1\n");
    for (ctx, actions) in tables {
        doc += &format!("[bindings.\"{ctx}\"]\n");
        for (action, entries) in actions {
            doc += &format!("\"{action}\" = [{}]\n", entries.join(", "));
        }
    }
    doc
}

/// `on` through both readers and `Keymap::set`.
fn on_field(data: &[u8]) {
    let text = on_document(data);
    let lenient = Keymap::parse_lenient(&text);
    let strict = Keymap::parse(&text);
    if let Ok(strict) = &strict {
        assert_eq!(&lenient.keymap, strict);
        for (old, _) in RETIRED_TEXT_ENTRY_ACTIONS {
            assert!(strict.get("text_entry", old).is_none(), "strict kept {old}");
        }
    }
    let km = &lenient.keymap;
    // Migration: the lenient reader leaves no retired name in `text_entry`.
    for (old, _) in RETIRED_TEXT_ENTRY_ACTIONS {
        assert!(km.get("text_entry", old).is_none(), "lenient kept {old}");
    }
    // Every kept non-`press` entry is a single stroke with one button, not a wheel click.
    for (_, _, list) in km.iter() {
        for b in list {
            if let BindingSpec::Digital { chord, .. } = b
                && chord.activation() != Activation::Press
            {
                let [stroke] = chord.strokes() else {
                    panic!("`on` kept on a sequence: {chord}");
                };
                let [button] = stroke.buttons() else {
                    panic!("`on` kept on a chord or a modifier hold: {chord}");
                };
                assert!(!matches!(button, Button::Wheel(_)), "`on` on a wheel click");
            }
        }
    }
    // Keymap::set with every `on` on every input: refused exactly when not allowed.
    for input in INPUTS {
        let Ok(chord) = Chord::parse(input) else {
            continue;
        };
        for on in Activation::ALL {
            if let Ok(c) = chord.clone().with_activation(on) {
                let mut k = Keymap::new();
                k.set("pointer", "select_left", vec![BindingSpec::chord(c)])
                    .expect("a chord with an allowed `on` is accepted");
                let out = k.to_toml_string().expect("serialises");
                assert_eq!(Keymap::parse(&out).expect("strict"), k);
            }
        }
    }
    // An axis action with a non-`press` `on` compiles (disabled with a warning).
    let (mut m, _) = InputMapper::new(default_registry(), &default_keymap().overlay(km));
    m.update(std::time::Duration::ZERO);
}

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
        // Bit 2: an action of `text_entry`, retired names included (§6.3);
        // a retired name there is refused, so the save below reads back.
        let (ctx, action) = if c[0] & 4 == 0 {
            ("c", format!("a{i}"))
        } else {
            let a = ACTIONS[usize::from(c[9]) % ACTIONS.len()];
            ("text_entry", a.to_owned())
        };
        let retired = ctx == "text_entry"
            && RETIRED_TEXT_ENTRY_ACTIONS
                .iter()
                .any(|(old, _)| *old == action);
        let set = k.set(ctx, &action, vec![spec]);
        assert!(!(retired && set.is_ok()), "retired {action} accepted");
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
    on_field(data);
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
