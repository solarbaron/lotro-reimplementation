// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's command-line pipeline (`apps/client/src/commands/line.rs`,
//! `chat-window.md` §7.2–§7.4 and §6.3, decision 0020), included by path, on arbitrary
//! text: nothing may panic; tokens are never empty (CW-I13) and, without quotes, equal the
//! whitespace split; arguments never start or end with whitespace (CW-I11); the history
//! stays bounded and free of duplicates (CW-I9).

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/commands/line.rs"]
mod line;

fuzz_target!(|data: &[u8]| {
    let (prefix, rest) = match data.split_first() {
        Some((&b, rest)) if b % 4 == 0 => ("#", rest),
        Some((_, rest)) => ("/", rest),
        None => ("/", data),
    };
    let text = String::from_utf8_lossy(rest);
    let tokens = line::tokenize(&text);
    assert!(tokens.iter().all(|t| !t.is_empty()));
    if !text.contains('"') {
        let split: Vec<&str> = text
            .split(line::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(tokens, split);
    }
    match line::prepare(&text, prefix, "say", "") {
        Ok(Some(inv)) => {
            assert!(!inv.word.is_empty());
            assert_eq!(inv.tokens.first(), Some(&inv.word));
            assert_eq!(line::trim(&inv.arguments), inv.arguments);
            if inv.plain {
                assert_eq!(inv.word, "say");
            }
        }
        Ok(None) => {}
        Err(line::LineError::TooLong) => assert!(text.len() > line::MAX_LINE_LEN),
    }
    let mut history = line::History::new();
    for piece in text.split('\n') {
        history.add(piece);
        let _ = history.older();
    }
    let lines: Vec<&str> = history.lines().collect();
    assert!(lines.len() < line::HISTORY_CAPACITY);
    for (i, a) in lines.iter().enumerate() {
        assert!(lines.iter().skip(i + 1).all(|b| b != a));
    }
});
