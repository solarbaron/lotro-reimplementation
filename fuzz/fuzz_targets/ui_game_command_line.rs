// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` game chat command line (`chat-window.md` §6.3, §6.5, §6.6, §7, §8.2): an
//! arbitrary typed line goes through the tokenizer, the command word and arguments, the
//! whole run-line pipeline with `;` substitutions (values taken from the input, so they
//! nest and recurse), the history, the registry and `IsCommand`, the mode words and the
//! key-repeat guard. Nothing may panic, loop without end or allocate without bound.
//!
//! Input layout: the bytes as (lossy) UTF-8; the first line is the typed text, the other
//! lines are registry entries, mode-table entries and substitution values.

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_ui::game::chat::command::{arguments, split_names, trim_ws};
use tessera_ui::game::chat::{
    ChannelModes, CommandHost, CommandInfo, CommandLine, CommandRegistry, KeyRepeatGuard, ModeKind,
    execute, tokenize,
};

struct Host {
    reg: CommandRegistry,
    subs: BTreeMap<String, String>,
    calls: usize,
}

impl CommandHost for Host {
    fn lookup(&self, word: &str) -> Option<CommandInfo> {
        self.reg.lookup(word)
    }

    fn run(&mut self, _info: CommandInfo, word: &str, args: &str, tokens: &[String]) -> i32 {
        self.calls += 1;
        assert!(!word.is_empty());
        assert_eq!(args, trim_ws(args));
        assert!(tokens.iter().all(|t| !t.is_empty()));
        1
    }

    fn substitute(&mut self, token: &str) -> Option<String> {
        self.subs.get(token).cloned()
    }
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut lines = text.split('\n');
    let typed = lines.next().unwrap_or("");
    let rest: Vec<&str> = lines.collect();
    let mut reg = CommandRegistry::default();
    let mut subs = BTreeMap::new();
    let mut rows = Vec::new();
    for (i, l) in rest.iter().enumerate() {
        let info = CommandInfo {
            id: i as u32,
            substitutes: i % 2 == 0,
        };
        match i % 3 {
            0 => {
                reg.register_list(l, info);
            }
            1 => {
                if let Some((k, v)) = l.split_once('=') {
                    subs.insert(k.to_string(), v.to_string());
                }
            }
            _ => rows.push((l.to_string(), i as u32, ModeKind::Plain)),
        }
        let _ = split_names(l);
    }
    for t in tokenize(typed) {
        assert!(!t.is_empty());
        let _ = reg.is_command(&t);
    }
    let trimmed = trim_ws(typed);
    if let Some(w) = tokenize(trimmed).first() {
        let a = arguments(trimmed, w);
        assert_eq!(a.as_str(), trim_ws(&a));
    }
    let mut host = Host {
        reg,
        subs,
        calls: 0,
    };
    let _ = execute(typed, &mut host);
    let mut cl = CommandLine::default();
    for _ in 0..3 {
        let _ = cl.run_line(typed, "say", "target", &mut host);
        assert!(cl.history.lines.len() < 20);
        let _ = cl.history.older();
        let _ = cl.history.newer();
    }
    let modes = ChannelModes::new(&rows, &cl.setup);
    let _ = modes.switch_word(typed);
    let _ = modes.command_word(5);
    let mut g = KeyRepeatGuard::default();
    for (i, ch) in typed.chars().take(4096).enumerate() {
        let _ = g.check(ch, i as f64 * 0.01);
    }
});
