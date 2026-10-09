// SPDX-License-Identifier: GPL-3.0-or-later
//! The original's key-mapping file format as written (`keymap.md` §12.13),
//! to bytes in memory. Tessera never writes the original's file; this exists
//! so that the reader can be checked against it (I6) and for tests.

use std::fmt::Write as _;

use super::model::{KeyMap, STANDARD_DEVICE_NAMES, Vocabulary, activation, activation_text};
use super::names::{control_name, device_type_word, sub_control_word};

/// Quote a token when it is empty or holds a separator, `#` or a bracket;
/// escape `"` and `\` (§12.13).
fn token(s: &str) -> String {
    let needs = s.is_empty()
        || s.bytes().any(|b| {
            matches!(
                b,
                b' ' | b'\t' | b'#' | b',' | b'[' | b']' | b'(' | b')' | b'{' | b'}'
            )
        });
    let mut out = String::with_capacity(s.len() + 2);
    if needs {
        out.push('"');
    }
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    if needs {
        out.push('"');
    }
    out
}

/// Write `km` in the original's layout, with CRLF line ends. Command and
/// section names come from `vocab` (an ID without a name is written in
/// decimal); controls are named from the table of their device's type.
#[must_use]
pub fn write_key_map(km: &KeyMap, vocab: &Vocabulary) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("# Key map written by Tessera for testing (keymap.md section 12.13).\n");
    s.push_str("#\n\n");
    s.push_str("\"Tessera\" [ 00000000-0000-0000-0000-000000000000 ]\n\n");
    let dev_type = |i: u8| km.devices.get(usize::from(i)).map_or(0, |d| d.kind);
    let control = |device: u8, sub: u8, code: u16| -> String {
        let name = control_name(dev_type(device), code).unwrap_or_default();
        let mut c = format!("{device} {}", token(&name));
        if sub != 0
            && let Some(w) = sub_control_word(sub)
        {
            let _ = write!(c, " {w}");
        }
        c
    };
    s.push_str("Devices\n[\n");
    for d in &km.devices {
        let guid = vocab
            .standard_index(d.guid)
            .and_then(|i| STANDARD_DEVICE_NAMES.get(i))
            .map_or_else(|| d.guid.to_string(), |n| (*n).to_owned());
        let word = device_type_word(d.kind).unwrap_or("");
        let _ = writeln!(s, "  {} [ {guid} ]", token(word));
    }
    s.push_str("]\n\nMetaKeys\n[\n");
    for m in &km.meta_keys {
        let c = m.control;
        let _ = writeln!(s, "  {} [ {} ]", m.bit, control(c.device, c.sub, c.code));
    }
    s.push_str("]\n\nBindings\n[\n");
    for (i, sec) in km.sections.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        let name = vocab
            .input_maps
            .name(sec.input_map)
            .map_or_else(|| sec.input_map.to_string(), str::to_owned);
        let _ = writeln!(s, "  {}", token(&name));
        if sec.bindings.is_empty() {
            continue;
        }
        s.push_str("  [\n");
        for b in &sec.bindings {
            let cmd = vocab
                .commands
                .name(b.command)
                .map_or_else(|| b.command.to_string(), str::to_owned);
            let origin = if b.user { "User" } else { "Old" };
            let c = b.control;
            let _ = write!(
                s,
                "    {} [ {origin} [ {} ]",
                token(&cmd),
                control(c.device, c.sub, c.code)
            );
            if b.mask != 0 || b.activation != activation::DEFAULT {
                let _ = write!(s, " 0x{:08x}", b.mask);
                if b.activation != activation::DEFAULT {
                    match activation_text(b.activation) {
                        Some(w) => {
                            let _ = write!(s, " {}", token(&w));
                        }
                        None if b.activation & 0x7FFF_FFFF == 0 => s.push_str(" \"\""),
                        None => {}
                    }
                }
            }
            s.push_str(" ]\n");
        }
        s.push_str("  ]\n");
    }
    s.push_str("]\n");
    s.replace('\n', "\r\n").into_bytes()
}
