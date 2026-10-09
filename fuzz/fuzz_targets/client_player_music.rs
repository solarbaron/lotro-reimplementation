// SPDX-License-Identifier: GPL-3.0-or-later
//! The player-music chat commands (`apps/client/src/audio/player_music/command.rs`) on the
//! tokens of the client's command-line pipeline (`apps/client/src/commands/line.rs`), both
//! included by path so this crate does not pull in the client: nothing may panic, a parsed
//! `/play` has a non-empty name, and only the music words are taken.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/audio/player_music/command.rs"]
mod command;

#[allow(dead_code)]
#[path = "../../apps/client/src/commands/line.rs"]
mod line;

use command::{MAX_TOKENS, MusicCommand, MusicWord, parse_args};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let Ok(Some(inv)) = line::prepare(&text, line::DEFAULT_PREFIX, "say", "") else {
        return;
    };
    let Some(word) = MusicWord::from_name(&inv.word) else {
        return;
    };
    assert!(!inv.plain, "plain text runs the channel word");
    match parse_args(word, inv.args_tokens()) {
        Ok(MusicCommand::Play { name, .. }) => {
            assert_eq!(word, MusicWord::Play);
            assert!(!name.is_empty());
            assert!(name.len() <= line::MAX_LINE_LEN);
        }
        Ok(MusicCommand::PlayList { filter: Some(f) }) => assert!(!f.is_empty()),
        Ok(MusicCommand::Instrument { choice: Some(c) }) => assert!(!c.is_empty()),
        Ok(_) => assert!(inv.args_tokens().len() <= MAX_TOKENS),
        Err(_) => {}
    }
});
