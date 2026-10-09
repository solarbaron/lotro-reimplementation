// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` game chat entry-line rules (`chat-window.md` §6.5, §6.6, §7.1): name lists
//! split by the §7.1 rule, the mode words built from them and the switch test, and the
//! key-repeat guard, on arbitrary text. Nothing may panic, loop without end or allocate
//! without bound. (The command line itself, registry, pipeline and history, is the
//! client's command dispatcher since decision 0020; the `client_command_line` target fuzzes
//! it.)
//!
//! Input layout: the bytes as (lossy) UTF-8; the first line is the typed text, the other
//! lines are mode-table entries.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::game::chat::command::{split_names, trim_ws};
use tessera_ui::game::chat::{ChannelModes, CommandSetup, KeyRepeatGuard, ModeKind};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut lines = text.split('\n');
    let typed = lines.next().unwrap_or("");
    let mut rows = Vec::new();
    for (i, l) in lines.enumerate() {
        for n in split_names(l) {
            assert!(!n.is_empty());
            assert_eq!(n.as_str(), trim_ws(&n));
            assert!(!n.contains([';', ':', ',']));
        }
        let kind = match i % 3 {
            0 => ModeKind::Plain,
            1 => ModeKind::Reply,
            _ => ModeKind::TellAgain,
        };
        rows.push((l.to_string(), i as u32, kind));
    }
    let setup = CommandSetup::default();
    let modes = ChannelModes::new(&rows, &setup);
    let _ = modes.switch_word(typed);
    let _ = modes.command_word(5);
    let mut g = KeyRepeatGuard::default();
    for (i, ch) in typed.chars().take(4096).enumerate() {
        let _ = g.check(ch, i as f64 * 0.01);
    }
});
