// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line readers of `tessera-dattool` and `tessera-loadtest` on arbitrary typed
//! arguments (P8 exit criterion 2, decision 0043 §2): `tessera_dattool::hexarg::parse_did`
//! and `parse_prefix` (the DID and DID high-byte arguments) and
//! `tessera_loadtest::runner::Scenario::parse` (the `--scenario` name).
//!
//! The input is (lossy) UTF-8 text; each line is one argument. Nothing may panic. A DID or
//! prefix that reads back is written again in hex and reads back to the same value; a
//! scenario name that is accepted is that scenario's own name.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dattool::hexarg::{parse_did, parse_prefix};
use tessera_loadtest::runner::Scenario;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    for arg in text.split('\n').take(64) {
        if let Ok(did) = parse_did(arg) {
            assert_eq!(parse_did(&format!("{did:#010x}")), Ok(did), "{arg:?}");
            assert_eq!(parse_did(&format!("{did:x}")), Ok(did), "{arg:?}");
        }
        if let Ok(prefix) = parse_prefix(arg) {
            assert_eq!(
                parse_prefix(&format!("{prefix:#04x}")),
                Ok(prefix),
                "{arg:?}"
            );
        }
        if let Some(s) = Scenario::parse(arg) {
            assert_eq!(s.name(), arg);
            assert_eq!(Scenario::parse(s.name()), Some(s));
        }
    }
});
