// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture-list decoding and level selection (`docs/specs/texture-list.md` §2-§5) on
//! arbitrary bytes.
//!
//! Nothing may panic. An accepted list must match its own header (payload length
//! 17 + 4·m, m = level count × surfaces per level), and the effective chain must be a
//! contiguous run of resolving, non-zero levels.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{
    AnyRecord, Did, Record, TextureList, cap_chain_resolution, decode_texture_list,
};

fuzz_target!(|data: &[u8]| {
    let Ok(list) = decode_texture_list(data) else {
        return;
    };
    let m = usize::from(list.level_count) * list.kind.surfaces_per_level();
    assert_eq!(list.surfaces.len(), m);
    assert_eq!(data.len(), 17 + 4 * m);
    assert_eq!(list.kind.code(), data[4]);
    let _ = list.has_unusual_fields();
    let _ = list.levels();
    let _ = list.cube_faces();

    // Resolve a level when a bit of its DID is set, as a stand-in for an archive lookup.
    let chain = list.effective_chain(|d| (d.0 & 1 == 1).then_some((d.0 >> 8) & 0xFFF));
    for w in chain.windows(2) {
        assert_eq!(w[1].index, w[0].index + 1);
    }
    for l in &chain {
        assert_ne!(l.did, Did(0));
        assert_eq!(list.surfaces[l.index], l.did);
    }
    let len = chain.len();
    let capped = cap_chain_resolution(chain, 256, |&s| (s, s / 2));
    assert!(capped.len() <= len);
    assert!(len == 0 || !capped.is_empty());

    assert_eq!(TextureList::parse(data).as_ref(), Ok(&list));
    if let Ok(Some(rec)) = AnyRecord::decode(list.did, data) {
        assert_eq!(rec.did(), list.did);
    }
});
