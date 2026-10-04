// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_wstate::ClassLibrary::parse` on arbitrary bytes (`docs/specs/wlib.md`): the
//! chunk walk, class table, attribute lists, parent map (cycle check) and singleton list
//! must never panic and must stay within bounded memory. Derived queries (ancestors,
//! is-a, attribute lookup) run on whatever decodes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_wstate::ClassLibrary;

fuzz_target!(|data: &[u8]| {
    let Ok(lib) = ClassLibrary::parse(data) else {
        return;
    };
    let n = u32::try_from(lib.len()).unwrap_or(u32::MAX);
    for c in lib.classes().take(4096) {
        let _ = lib.ancestors(c.index).count();
        let _ = lib.is_a(c.index, 0);
        let _ = lib.attributes(c.index).len();
        let _ = c.serialized_size();
    }
    for &s in lib.singletons().iter().take(64) {
        let _ = lib.singleton_did(s);
    }
    let _ = lib.parent(n.saturating_sub(1));
    let _ = lib.warnings().len();
});
