// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive file names as the archive set reads them from a directory listing
//! (`tessera-dat`, `ArchiveId::from_file_name`; grammar of `docs/specs/install-layout.md`
//! §3.1, kinds of `dat-container.md` §2.1): arbitrary names, and names assembled from
//! input-chosen prefixes, keys, aux parts and suffixes in mixed case so most runs reach
//! the key grammar.
//!
//! Nothing may panic. An accepted name has a canonical file name that parses back to the
//! same identity, routing tags and regions agree with the kind, and where the installer's
//! own reader (`tessera_install::parse_archive_name`) accepts the same name the two
//! canonical names agree up to ASCII case.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dat::{ArchiveId, ArchiveKind};

/// Pieces the structured mode assembles names from (syntax of the grammar only).
const PREFIXES: [&str; 4] = ["client_", "CLIENT_", "Client_", "clien_"];
const KEYS: [&str; 12] = [
    "general",
    "GameLogic",
    "mesh",
    "surface",
    "highres",
    "sound",
    "anim",
    "local_",
    "cell_",
    "map_",
    "",
    "_aux_",
];
const SUFFIXES: [&str; 5] = [".dat", ".datx", ".DATX", ".Dat", ""];

fn check(name: &str) {
    let Some(id) = ArchiveId::from_file_name(name) else {
        return;
    };
    let canonical = id.file_name();
    assert_eq!(ArchiveId::from_file_name(&canonical).as_ref(), Some(&id));
    assert!(!id.to_string().is_empty());
    if id.part == 0 {
        assert!(canonical.ends_with(".dat"));
    } else {
        assert!(canonical.ends_with(".datx"));
    }
    match &id.kind {
        ArchiveKind::Cell(r) | ArchiveKind::Map(r) => assert_eq!(id.kind.region(), Some(*r)),
        _ => assert_eq!(id.kind.region(), None),
    }
    assert_eq!(
        id.kind.tag().is_none(),
        matches!(id.kind, ArchiveKind::Other(_))
    );
    if let Some(other) = tessera_install::parse_archive_name(name) {
        assert!(
            other.file_name().eq_ignore_ascii_case(&canonical),
            "{name:?}: {canonical:?} vs {:?}",
            other.file_name()
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode & 1 == 0 {
        check(&String::from_utf8_lossy(rest));
        return;
    }
    // Structured: prefix, key, free middle, optional aux part, suffix.
    let [p, k, s, aux, middle_len, tail @ ..] = rest else {
        return;
    };
    let n = usize::from(*middle_len).min(tail.len());
    let (middle, digits) = tail.split_at(n);
    let mut name = String::new();
    name.push_str(PREFIXES[usize::from(*p) % PREFIXES.len()]);
    name.push_str(KEYS[usize::from(*k) % KEYS.len()]);
    name.push_str(&String::from_utf8_lossy(middle));
    if aux & 1 == 1 {
        name.push_str(if aux & 2 == 0 { "_aux_" } else { "_AUX_" });
        name.push_str(&String::from_utf8_lossy(digits));
    }
    name.push_str(SUFFIXES[usize::from(*s) % SUFFIXES.len()]);
    check(&name);
});
