// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared gameplay helpers (`tessera_formats::gameplay::common`) on arbitrary bytes:
//! the object-record header (`docs/specs/wstate.md` §3.1), an object paired with its
//! property set (`property-system.md` §6.2), DID-reference extraction and the reverse
//! index walk, and class ancestry over an arbitrary (possibly cyclic) parent map
//! (`wlib.md` §9).
//!
//! Input: byte 0 splits the rest into an object payload of up to 255 bytes and a property-set
//! payload; the property set is decoded with a total synthetic schema. The same bytes, read
//! as u32 pairs, form a parent map.
//!
//! Nothing may panic, walks must end within the class count, and every DID reference found
//! must be non-null and present in the reverse index.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_common -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::PropertyType;
use tessera_formats::gameplay::common::{
    ClassParents, ClassTree, GameObjectRecord, ObjectRoot, ReverseDidIndex, did_references,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn u32s(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
}

fuzz_target!(|input: &[u8]| {
    let [split, rest @ ..] = input else {
        return;
    };
    let at = usize::from(*split).min(rest.len());
    let (object, set) = rest.split_at(at);

    if let Ok(root) = ObjectRoot::read(object) {
        assert!(matches!(root.did.prefix(), 0x70..=0x77));
        assert!(ObjectRoot::read_expecting(object, root.did).is_ok());
    }
    for set_payload in [None, Some(set)] {
        let Ok(rec) = GameObjectRecord::decode(object, set_payload, &total) else {
            continue;
        };
        let refs = did_references(&rec.properties);
        let mut index = ReverseDidIndex::new();
        index.add_record(rec.did(), &rec.properties, |_| true);
        assert_eq!(index.len(), refs.len());
        for r in &refs {
            assert!(!r.target.is_null());
            assert!(
                index
                    .referrers(r.target)
                    .iter()
                    .any(|e| e.source == rec.did())
            );
            let hits = index.walk_referrers(r.target, 4, |_| true);
            assert!(hits.len() <= 1);
        }
    }

    // Class ancestry on an arbitrary map.
    let words: Vec<u32> = u32s(rest).collect();
    let tree = ClassTree::from_edges(
        words
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[a, b]| (a % 64, b % 64)),
    );
    for c in 0..64u32 {
        assert!(tree.lineage(c).count() <= tree.step_limit() + 1);
        if let Some(child) = tree.child_toward(0, c) {
            assert!(tree.is_a(c, child));
        }
    }
    if let Some(a) = tree.common_ancestor(words.iter().map(|w| w % 64).take(8)) {
        for w in words.iter().map(|w| w % 64).take(8) {
            assert!(tree.is_a(w, a));
        }
    }
});
