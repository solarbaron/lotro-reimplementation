// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain-scene tables (0x12) and scene-choice tables (0x11), `docs/specs/scenery.md`
//! §4.2–§4.3, on arbitrary bytes. Bit 0 of the first byte picks the decoder; bit 1 whether
//! the rest is the payload as is or follows a valid prologue (0x12: own DID, reserved word
//! and `has_default` from bit 2; 0x11: reserved word and own DID), so that short inputs
//! reach the entries.
//!
//! Nothing may panic. An accepted table re-encodes canonically (a fixpoint, never longer
//! than the input), and its lookups work on it: by terrain id and slot (the table's slot
//! index must agree with a scan of the entry's pairs), by coarse word, by scene key, and the
//! weighted choice.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_scene_tables -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_formats::scenery::{
    SceneChoiceTable, TerrainSceneTable, decode_scene_choice_table, decode_terrain_scene_table,
};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode & 2 == 0 {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(9 + rest.len());
    if mode & 1 == 0 {
        out.extend_from_slice(&0x1200_0001u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.push((mode >> 2) & 1);
    } else {
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0x1100_0001u32.to_le_bytes());
    }
    out.extend_from_slice(rest);
    out
}

fn terrain_scene(data: &[u8], probe: &[u8]) {
    let Ok(t) = decode_terrain_scene_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = TerrainSceneTable::parse(&bytes).expect("re-encoded table decodes");
    // Compared as bytes: `unknown_f` may be NaN.
    assert_eq!(again.to_bytes(), bytes);
    assert_eq!(t.did(), again.did());
    for e in t.entries() {
        assert!(t.entry(e.id).is_some());
        let first = t.entry_for(e.id).expect("listed id has an entry");
        for p in &e.pairs {
            let found = e.pair_for_slot(p.slot).expect("pair by its own slot");
            assert_eq!(found.slot, p.slot);
            assert_eq!(t.pair_for(e.id, p.slot), first.pair_for_slot(p.slot));
        }
    }
    for w in probe.as_chunks::<4>().0 {
        let word = u32::from_le_bytes(*w);
        let (terrain, slot) = (word & 0xFF, (word >> 8) & 0x7F);
        let k = t.scene_key_for_word(word);
        assert_ne!(k, Some(0));
        assert_eq!(k, t.scene_key(terrain, slot));
        // The slot index agrees with a scan of the entry's pairs.
        let scanned = t.entry_for(terrain).and_then(|e| e.pair_for_slot(slot));
        assert_eq!(t.pair_for(terrain, slot), scanned);
    }
}

fn scene_choice(data: &[u8], probe: &[u8]) {
    let Ok(t) = decode_scene_choice_table(data) else {
        return;
    };
    let bytes = t.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = SceneChoiceTable::parse(&bytes).expect("re-encoded table decodes");
    // Compared as bytes: weights may be NaN.
    assert_eq!(again.to_bytes(), bytes);
    for e in t.entries() {
        assert!(t.get(e.key).is_some());
        let _ = e.total_weight();
        for u in probe.as_chunks::<2>().0.iter().take(8) {
            let u = f32::from(u16::from_le_bytes(*u)) / 65_536.0;
            assert_eq!(e.choose(u).is_some(), !e.choices.is_empty());
        }
        let _ = e.choose(f32::NAN);
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    // The input's tail doubles as lookup probes.
    let probe = &rest[rest.len().saturating_sub(32)..];
    if mode & 1 == 0 {
        terrain_scene(data, probe);
    } else {
        scene_choice(data, probe);
    }
});
