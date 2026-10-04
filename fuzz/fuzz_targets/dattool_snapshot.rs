// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-dattool` snapshot files (`apps/dattool/src/snapfile.rs`): the decoder must
//! reject any byte string without panicking and within its memory bound, and a decoded
//! index must encode and decode again to the same record count.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_dattool::snapfile::{decode, encode, looks_like_snapshot};

fuzz_target!(|data: &[u8]| {
    let _ = looks_like_snapshot(data);
    if let Ok(index) = decode(data) {
        let n = index.records();
        if let Ok(bytes) = encode(&index) {
            let again = decode(&bytes).expect("own encoding decodes");
            assert_eq!(again.records(), n);
        }
    }
});
