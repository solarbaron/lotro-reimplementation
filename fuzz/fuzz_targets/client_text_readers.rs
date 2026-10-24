// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's small text readers on arbitrary typed input (P8 exit criterion 2, decision
//! 0043 §2), linked from the `tessera-client` library:
//!
//! - `commands::split_tell` (the typed `tell` arguments, `chat-window.md`);
//! - `screens::controls::parse_sliders` (slider bytes of a value string,
//!   `appearance.md` §6.3);
//! - `cli::parse_layer`, `cli::parse_surface_did`, `cli::parse_size` (command-line numbers);
//! - `cli::edit_password`, the line editor behind `cli::read_password` over raw terminal
//!   bytes;
//! - the `--open-panel` name readers `map::parse_map_panel`, `quests::parse_journal_panel`,
//!   `travel::parse_travel_panel`, `social::ui::parse_open`,
//!   `ui_host::client::parse_p1_panel` and `ui_host::parse_panel`.
//!
//! Byte 0 selects the mode, byte 1 is a count for the slider mode, the rest is the input
//! ((lossy) UTF-8 text, raw bytes for the password editor). Nothing may panic. The name and
//! text of a tell are pieces of the input; the slider list has exactly
//! `min(count, MAX_SLIDERS)` bytes; a size is inside 1..=16384 on both sides; a surface DID
//! has the render-surface prefix; an edited password is valid UTF-8 of at most
//! `MAX_PASSWORD_BYTES` bytes with no control character; a bag panel is bag 1–6.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_client::cli::{
    MAX_PASSWORD_BYTES, edit_password, parse_layer, parse_size, parse_surface_did,
};
use tessera_client::commands::split_tell;
use tessera_client::screens::controls::{MAX_SLIDERS, parse_sliders};
use tessera_client::ui_host::client::{P1Panel, parse_p1_panel};

/// Whether `part` lies inside `whole` (same allocation, by address).
fn inside(part: &str, whole: &str) -> bool {
    let (p, w) = (part.as_ptr() as usize, whole.as_ptr() as usize);
    part.is_empty() || (p >= w && p + part.len() <= w + whole.len())
}

fuzz_target!(|data: &[u8]| {
    let [mode, count, rest @ ..] = data else {
        return;
    };
    let text = String::from_utf8_lossy(rest);
    match mode % 5 {
        0 => {
            let (name, said) = split_tell(&text);
            assert!(inside(name, &text) && inside(said, &text));
            assert!(name.len() + said.len() <= text.len());
        }
        1 => {
            let n = usize::from(*count);
            let bytes = parse_sliders(&text, n);
            assert_eq!(bytes.len(), n.min(MAX_SLIDERS));
        }
        2 => {
            for arg in text.split('\n').take(32) {
                let _ = parse_layer(arg);
                if let Ok(did) = parse_surface_did(arg) {
                    assert_eq!(did >> 24, 0x41, "{arg:?}");
                }
                if let Ok((w, h)) = parse_size(arg) {
                    assert!((1..=16_384).contains(&w) && (1..=16_384).contains(&h));
                }
            }
        }
        3 => {
            if let Ok(pw) = edit_password(rest.iter().map(|&b| Ok(b))) {
                assert!(pw.len() <= MAX_PASSWORD_BYTES);
                assert!(!pw.chars().any(|c| (c as u32) < 0x20));
            }
        }
        _ => {
            for name in text.split('\n').take(32) {
                let _ = tessera_client::map::parse_map_panel(name);
                let _ = tessera_client::quests::parse_journal_panel(name);
                let _ = tessera_client::travel::parse_travel_panel(name);
                let _ = tessera_client::social::ui::parse_open(name);
                let _ = tessera_client::ui_host::parse_panel(name);
                if let Some(P1Panel::Bag(b)) = parse_p1_panel(name) {
                    assert!((1..=6).contains(&b), "{name:?}");
                }
            }
        }
    }
});
