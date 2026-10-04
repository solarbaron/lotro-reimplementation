// SPDX-License-Identifier: GPL-3.0-or-later
//! Chain resolution (`docs/specs/setup.md` §3.1) over a small in-memory store built from
//! arbitrary bytes: up to eight records, each a u16 length and that many bytes, stored under
//! the DID they carry (the second word for 0x1F-style records, whose first word is flags).
//! Resolution starts at the first record's DID.
//!
//! Nothing may panic, every chain must terminate, and a resolved chain must end at its
//! setup within the hop limit. [`resolve_setup_did`] must agree with [`resolve_setup`] on
//! the path, and a fetch error injected for one DID must come back as a fetch error.

#![no_main]

use std::collections::HashMap;
use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::setup::MAX_CHAIN_HOPS;
use tessera_formats::{Did, ResolveError, resolve_setup, resolve_setup_did};

fn word(b: &[u8], at: usize) -> Option<u32> {
    let w = b.get(at..at + 4)?;
    Some(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

fuzz_target!(|data: &[u8]| {
    let mut store: HashMap<Did, &[u8]> = HashMap::new();
    let mut first = None;
    let mut rest = data;
    for _ in 0..8 {
        let Some((len, tail)) = rest.split_first_chunk::<2>() else {
            break;
        };
        let len = usize::from(u16::from_le_bytes(*len)).min(tail.len());
        let (rec, tail) = tail.split_at(len);
        rest = tail;
        let Some(w0) = word(rec, 0) else { continue };
        let key = match w0 >> 24 {
            0x04 | 0x47 => w0,
            _ => word(rec, 4).unwrap_or(w0),
        };
        first.get_or_insert(Did(key));
        store.insert(Did(key), rec);
    }
    let Some(start) = first else { return };
    let mut fetches = 0usize;
    let result = resolve_setup(start, |d| {
        fetches += 1;
        Ok::<_, Infallible>(store.get(&d).copied())
    });
    assert!(fetches <= MAX_CHAIN_HOPS + 1);
    let path = resolve_setup_did(start, |d| Ok::<_, Infallible>(store.get(&d).copied()));
    if let Ok(chain) = &result {
        assert!(chain.hops() <= MAX_CHAIN_HOPS);
        assert_eq!(chain.path.last(), Some(&chain.setup.did));
        assert_eq!(chain.start(), start);
        assert_eq!(path.as_ref().ok(), Some(&chain.path));
    }
    if let Ok(path) = &path {
        assert!(path.len() <= MAX_CHAIN_HOPS + 1);
        assert_eq!(path.first(), Some(&start));
        assert_eq!(path.last().map(|d| d.prefix()), Some(0x04));
        // Failing the read of the last record fetched reports that DID as a fetch error.
        let failing = *path.iter().rev().nth(1).unwrap_or(&start);
        let e = resolve_setup(start, |d| {
            if d == failing {
                Err(())
            } else {
                Ok(store.get(&d).copied())
            }
        });
        assert!(matches!(e, Err(ResolveError::Fetch { did, .. }) if did == failing.0));
    }
});
