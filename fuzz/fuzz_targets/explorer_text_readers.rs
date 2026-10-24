// SPDX-License-Identifier: GPL-3.0-or-later
//! The explorer's small text and file readers on arbitrary input (P8 exit criterion 2,
//! decision 0043 §2), linked from the `tessera-explorer` library:
//!
//! - `cli::parse_rgb_arg`, `cli::parse_pair_arg`, `cli::parse_angles_arg` (command-line
//!   values) and `cli::Args::try_parse_checked` (a whole command line);
//! - `parse_values` (the string view's test-value box);
//! - `fonts::check_font`, which `fonts::load_font` hands a font file's bytes to after its
//!   size cap (the font is then parsed by skrifa).
//!
//! Byte 0 selects the mode, the rest is the input ((lossy) UTF-8 text, raw bytes for the font
//! mode). Nothing may panic. A colour reads back from its own hex; angles are finite; an
//! accepted command line has at most `charspec::MAX_ITEMS` worn items.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_explorer::charspec::MAX_ITEMS;
use tessera_explorer::cli::{Args, parse_angles_arg, parse_pair_arg, parse_rgb_arg};
use tessera_explorer::fonts::check_font;
use tessera_explorer::parse_values;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let text = String::from_utf8_lossy(rest);
    match mode % 4 {
        0 => {
            for arg in text.split('\n').take(32) {
                if let Ok([r, g, b]) = parse_rgb_arg(arg) {
                    let back = format!("{r:02x}{g:02x}{b:02x}");
                    assert_eq!(parse_rgb_arg(&back), Ok([r, g, b]), "{arg:?}");
                }
                let _ = parse_pair_arg(arg);
                if let Ok([yaw, pitch]) = parse_angles_arg(arg) {
                    assert!(yaw.is_finite() && pitch.is_finite(), "{arg:?}");
                }
            }
        }
        1 => {
            let words =
                std::iter::once("tessera-explorer").chain(text.split(['\n', '\0']).take(64));
            if let Ok(args) = Args::try_parse_checked(words) {
                assert!(args.wear.len() <= MAX_ITEMS);
            }
        }
        2 => {
            let _ = parse_values(&text);
        }
        _ => {
            let _ = check_font(rest);
        }
    }
});
