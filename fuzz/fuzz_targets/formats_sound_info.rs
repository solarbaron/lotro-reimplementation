// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound-info (0x2A) and sound-payload (0x0A) decoding (`docs/specs/sound.md` §3, §4, §12.1,
//! §12.2) on arbitrary bytes, plus a Sound / SoundEnvironment script node (`sound.md` §9.1,
//! §9.4) wrapped in a synthetic scriptlet so the typed node path runs on fuzzed field bytes.
//!
//! Nothing may panic. An accepted sound info must consume the input exactly and agree with
//! its own flag accessors; an accepted payload must satisfy its length identity and its
//! strict check must not panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::script_table::SoundNode;
use tessera_formats::{
    AnyRecord, NoSchema, NodeData, Record, Scriptlet, SoundInfo, SoundInfoFlags, WaveInfo,
    decode_sound, decode_sound_info,
};

/// A scriptlet with one root node of `kind` whose data is `data`, an empty base block
/// and no ports or targets.
fn scriptlet_with(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut b = vec![1u8, kind];
    b.extend_from_slice(data);
    // Base: two empty TSize maps, no scriptlet parameters, UseParentIntensity 1,
    // multiplier 1, variance 0, probability 1, node ID 1.
    b.extend_from_slice(&[1, 0, 1, 0, 0, 1]);
    for f in [1.0f32, 0.0, 1.0] {
        b.extend_from_slice(&f.to_le_bytes());
    }
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // ports
    b.extend_from_slice(&0u32.to_le_bytes()); // targets
    b.extend_from_slice(&0u32.to_le_bytes()); // dependencies
    b
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = decode_sound_info(data) {
        assert!(data.len() > SoundInfo::FIXED_SIZE);
        assert_eq!(s.did().prefix(), 0x2A);
        let f = s.flags;
        assert_eq!(f.unknown_bits() & SoundInfoFlags::KNOWN, 0);
        assert_eq!(f.is_non_positional(), !f.uses_distance_law());
        assert_eq!(s.is_loop(), f.bits() & SoundInfoFlags::LOOP != 0);
        let _ = s.payload_is_sound();
        let _ = s.validate();
    }
    if let Ok(Some(r)) = AnyRecord::decode(0x2A00_0001u32, data) {
        assert_eq!(r.did().0, 0x2A00_0001);
    }
    if let Ok(s) = decode_sound(data) {
        assert_eq!(s.record_len(), data.len());
        let _ = s.validate(s.did, data.len());
        let _ = s.storage_format().raw();
        let _ = s.reserved();
        // A strict WAV walk must not panic on any payload the envelope accepts.
        if let Ok(w) = WaveInfo::parse(s.audio) {
            let align = usize::from(w.block_align);
            assert!(align == 0 || w.data.len() % align == 0);
        }
    }
    // The first bytes as a Sound node (43 bytes) and a SoundEnvironment node (8 bytes).
    let n = data.len().min(SoundNode::SIZE);
    if let Ok(s) = Scriptlet::parse(&scriptlet_with(48, &data[..n]), &NoSchema) {
        assert_eq!(n, SoundNode::SIZE);
        let node = s.roots[0].sound().expect("typed sound node");
        let (lo, hi) = node.random_distance_range();
        // Always finite, non-negative and ordered, whatever is stored (NaN, ±infinity).
        assert!(lo.is_finite() && hi.is_finite() && 0.0 <= lo && lo <= hi);
        let _ = s.roots[0].is_sound_placeholder();
    }
    let n = data.len().min(8);
    if let Ok(s) = Scriptlet::parse(&scriptlet_with(62, &data[..n]), &NoSchema) {
        assert_eq!(n, 8);
        assert!(matches!(s.roots[0].data, NodeData::SoundEnvironment { .. }));
    }
});
