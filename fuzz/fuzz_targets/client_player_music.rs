// SPDX-License-Identifier: GPL-3.0-or-later
//! The player-music chat commands (`apps/client/src/audio/player_music/command.rs`,
//! included by path so this crate does not pull in the client) from arbitrary text:
//! nothing may panic, a parsed `/play` has a non-empty name, and only lines starting with a
//! known command word are taken.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/audio/player_music/command.rs"]
mod command;

use command::{MAX_COMMAND_LEN, MusicCommand, parse_command};

fuzz_target!(|data: &[u8]| {
    let line = String::from_utf8_lossy(data);
    let Some(parsed) = parse_command(&line) else {
        return;
    };
    assert!(line.trim_start().starts_with('/'));
    match parsed {
        Ok(MusicCommand::Play { name, .. }) => {
            assert!(!name.is_empty());
            assert!(name.len() <= MAX_COMMAND_LEN);
        }
        Ok(MusicCommand::PlayList { filter: Some(f) }) => assert!(!f.is_empty()),
        Ok(MusicCommand::Instrument { choice: Some(c) }) => assert!(!c.is_empty()),
        Ok(_) | Err(_) => {}
    }
});
