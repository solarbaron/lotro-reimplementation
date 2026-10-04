// SPDX-License-Identifier: GPL-3.0-or-later
//! The Steam text VDF reader (`docs/specs/install-layout.md` §5.3) on arbitrary bytes:
//! `vdf::parse_bytes`, `vdf::parse` on the same text, and the two consumers of a parsed
//! document, `steam::parse_library_folders` and `steam::parse_app_manifest`.
//!
//! Nothing may panic. `parse` and `parse_bytes` must agree on valid UTF-8 input, nesting
//! must stay within `MAX_VDF_DEPTH`, and the extracted fields must keep their invariants
//! (non-empty library paths, a single-component `installdir`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_install::steam::{parse_app_manifest, parse_library_folders};
use tessera_install::vdf::{self, MAX_VDF_DEPTH, VdfBlock, VdfValue};

fn depth(b: &VdfBlock) -> usize {
    b.entries
        .iter()
        .map(|(_, v)| match v {
            VdfValue::Block(inner) => 1 + depth(inner),
            VdfValue::Str(_) => 0,
        })
        .max()
        .unwrap_or(0)
}

fn walk(b: &VdfBlock) {
    for (k, v) in b.iter() {
        // Lookups by an existing key must find something (the last occurrence).
        assert!(b.get(k).is_some(), "lookup of an existing key");
        let _ = (b.get_str(k), b.get_block(k), v.as_str(), v.as_block());
        if let VdfValue::Block(inner) = v {
            walk(inner);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let parsed = vdf::parse_bytes(data);
    if let Ok(text) = std::str::from_utf8(data) {
        assert_eq!(vdf::parse(text), parsed, "str and byte entry points agree");
    }
    let Ok(doc) = parsed else {
        return;
    };
    assert!(depth(&doc) <= MAX_VDF_DEPTH, "nesting bounded");
    walk(&doc);

    for lib in parse_library_folders(&doc) {
        assert!(
            !lib.path.as_os_str().is_empty(),
            "library paths are non-empty"
        );
    }
    if let Some(dir) = parse_app_manifest(&doc).and_then(|m| m.installdir) {
        assert!(
            !dir.is_empty() && dir != "." && dir != ".." && !dir.contains(['/', '\\', '\0']),
            "installdir is one path component"
        );
    }
});
