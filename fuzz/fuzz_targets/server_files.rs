// SPDX-License-Identifier: GPL-3.0-or-later
//! tessera-server's file parsers fed arbitrary text (test-server.md §3.4–§3.6, §4, §5): the
//! content check (`check-content`, with and without the SPDX rule), the configuration, the
//! accounts file, PHC hash strings, character files and the world-state file. Operators and
//! anyone with write access to the data directory can hand-edit all of them. None may panic,
//! and what parses must be written back to text that parses to the same value.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_server::Config;
use tessera_server::accounts::{AccountsFile, PhcHash};
use tessera_server::characters::{CharacterFile, WorldState};
use tessera_server::content;

fuzz_target!(|data: &[u8]| {
    let Some((&which, rest)) = data.split_first() else {
        return;
    };
    // Every reader refuses files over 4 MiB before parsing; stay far below.
    if rest.len() > 256 * 1024 {
        return;
    }
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    match which % 7 {
        0 => {
            let _ = content::check_text(text, true);
        }
        1 => {
            let (c, findings) = content::check_text(text, false);
            if let Some(c) = c
                && findings.is_empty()
            {
                assert!(
                    c.check_invariants().is_empty(),
                    "{:?}",
                    c.check_invariants()
                );
            }
        }
        2 => {
            let _ = Config::parse(text);
        }
        3 => {
            if let Ok(f) = AccountsFile::parse(text) {
                let written = f.to_toml();
                let back = AccountsFile::parse(&written).expect("accounts file re-parses");
                assert_eq!(back, f);
            }
        }
        4 => {
            // Parsing only: verifying would run Argon2id with the string's parameters.
            if let Some(h) = PhcHash::parse(text) {
                assert_eq!(PhcHash::parse(&h.to_phc()), Some(h));
            }
        }
        5 => {
            if let Ok((f, _)) = CharacterFile::parse(text, "a") {
                let written = f.to_toml();
                let (back, _) = CharacterFile::parse(&written, "a")
                    .unwrap_or_else(|e| panic!("character file re-parses: {e}\n{written}"));
                assert_eq!(back, f);
            }
        }
        _ => {
            let (w, _) = WorldState::parse(text);
            let (back, warnings) = WorldState::parse(&w.to_toml());
            assert!(warnings.is_empty());
            assert_eq!(back, w);
        }
    }
});
