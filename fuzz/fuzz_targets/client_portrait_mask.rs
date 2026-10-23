// SPDX-License-Identifier: GPL-3.0-or-later
//! The portraits' opacity-mask loader (`apps/client/src/portrait/mask.rs`, included by path
//! so this crate does not pull in the window and GPU stack; `ui-widgets.md` §12.3 step 2):
//! a texture list and its surfaces from arbitrary bytes. Nothing may panic; a decoded mask
//! is never larger than the bound and its pixel buffer matches its size.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/portrait/mask.rs"]
mod mask;

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::Did;
use tessera_props::{MemorySource, RecordSource};

const LIST: u32 = 0x4000_0E4B;

fuzz_target!(|data: &[u8]| {
    // The input is split at its first two 0xFF bytes: the list record, then two surface
    // records (without a marker, the list only).
    let mut parts = data.splitn(3, |b| *b == 0xFF);
    let list = parts.next().unwrap_or_default();
    let mut source = MemorySource::new();
    source.insert(Did(LIST), list.to_vec());
    for (i, p) in parts.enumerate() {
        source.insert(Did(0x4100_0000 + i as u32), p.to_vec());
    }
    // The surfaces a parsed list names get suffixes of the input.
    if let Ok(Some(bytes)) = source.read_record(Did(LIST))
        && let Ok(l) =
            <tessera_formats::texture_list::TextureList as tessera_formats::Record>::parse(&bytes)
    {
        for (i, s) in l.surfaces.iter().take(4).enumerate() {
            let payload = data.get(i * 7..).unwrap_or_default().to_vec();
            source.insert(*s, payload);
        }
    }
    for did in [Did(LIST), Did(0x4100_0000)] {
        if let Ok(m) = mask::load_mask(&source, did) {
            assert!(m.width <= mask::MAX_MASK_SIDE && m.height <= mask::MAX_MASK_SIDE);
            assert_eq!(
                m.rgba.len() as u64,
                u64::from(m.width) * u64::from(m.height) * 4
            );
        }
    }
    let mut px = data.to_vec();
    mask::alpha_from_red(&mut px);
});
