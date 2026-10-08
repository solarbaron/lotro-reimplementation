// SPDX-License-Identifier: GPL-3.0-or-later
//! Fingerprint text, connect strings, the known-servers file (tessera-protocol.md §4.2,
//! §4.4, §4.5) and the certificate walker of §4.3 on arbitrary input: must never panic, and
//! whatever parses must print back to a form that parses to the same value.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::connect::{ConnectString, KnownServers, ServerKey, format_entry};
use tessera_protocol::fingerprint::{Fingerprint, certificate_fingerprint, certificate_spki};

fuzz_target!(|data: &[u8]| {
    let _ = certificate_spki(data);
    let _ = certificate_fingerprint(data);
    let text = String::from_utf8_lossy(data);
    if let Ok(f) = Fingerprint::parse(&text) {
        assert_eq!(Fingerprint::parse(&f.to_string()), Ok(f));
    }
    if let Ok(c) = ConnectString::parse(&text) {
        assert_eq!(ConnectString::parse(&c.to_string()).as_ref(), Ok(&c));
    }
    if let Ok(k) = ServerKey::parse(&text) {
        assert_eq!(ServerKey::parse(&k.to_string()).as_ref(), Ok(&k));
    }
    let (known, _warnings) = KnownServers::parse(&text);
    let mut rewritten = String::new();
    for (k, f) in known.entries() {
        rewritten.push_str(&format_entry(k, f));
    }
    let (again, warnings) = KnownServers::parse(&rewritten);
    assert!(warnings.is_empty());
    assert_eq!(again, known);
});
