// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-data decoding (`docs/specs/landblock.md` §3) on arbitrary bytes. The first
//! byte selects the directory stamp (0xFF = none, so the §3.9 fallbacks run). The second
//! selects how the rest becomes a payload: as is, or spliced behind a **valid synthetic
//! frame** (header, height grid, optional ceiling grid and ceiling tree, vertex flags,
//! coarse grid) so that inputs far below the 4,456-byte minimum record reach the surface
//! chunks, collision trees, box list and the stamp fallbacks.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint), every layer
//! window lies inside its index list, every vertex number is < 1,089, and the height
//! lookup and triangulation helpers work on it.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_landblock_data -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{GRID_SAMPLES, SurfaceChunk, vertex_to_height_index};
use tessera_formats::{LandblockData, decode_landblock_data};

fn check_chunk(c: &SurfaceChunk) {
    for l in &c.layers {
        let tris = c
            .layer_indices(l)
            .expect("decoded layer windows lie inside the list");
        assert_eq!(tris.len(), 3 * usize::from(l.prims));
        let _ = l.kind();
    }
    for &v in &c.indices {
        assert!(usize::from(v) < GRID_SAMPLES);
        assert!(vertex_to_height_index(v).is_some());
    }
    if let Some(q) = &c.quads {
        assert_eq!(q.len(), 1024);
    }
}

const DID: u32 = 0x8001_0203;

fn put_grid(out: &mut Vec<u8>, seed: u8) {
    out.extend_from_slice(&1089u32.to_le_bytes());
    for k in 0..1089u32 {
        out.extend_from_slice(&((k * u32::from(seed)) as u16).to_le_bytes());
    }
}

/// Builds a payload from `mode` and the fuzz bytes `rest`.
fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(4) {
        return rest.to_vec();
    }
    let seed = rest.first().copied().unwrap_or(0);
    let mut out = Vec::with_capacity(8192 + rest.len());
    out.extend_from_slice(&DID.to_le_bytes());
    out.extend_from_slice(&(if seed & 1 != 0 { DID + 0x0020_0000 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    put_grid(&mut out, seed);
    let mut rest = rest;
    match mode % 4 {
        // No ceiling.
        1 => out.push(0),
        // Ceiling without a tree.
        2 => {
            out.push(1);
            put_grid(&mut out, seed.wrapping_add(1));
            out.push(0);
        }
        // Ceiling with a tree whose code length and build byte come from the input.
        _ => {
            out.push(1);
            put_grid(&mut out, seed.wrapping_add(1));
            out.push(1);
            out.extend_from_slice(&[0; 16]);
            let len = usize::from(rest.first().copied().unwrap_or(0) % 32);
            let take = (1 + len + 1).min(rest.len());
            out.extend_from_slice(&(len as u32).to_le_bytes());
            out.extend_from_slice(rest.get(1..take).unwrap_or(&[]));
            rest = rest.get(take..).unwrap_or(&[]);
        }
    }
    // Vertex flags and coarse grid.
    out.extend_from_slice(&1089u32.to_le_bytes());
    out.extend((0..1089u32).map(|k| if k % 3 == 0 { 0x80 } else { 0x7F }));
    out.extend_from_slice(&289u32.to_le_bytes());
    out.extend_from_slice(&[0; 4 * 289]);
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [s, mode, rest @ ..] = input else {
        return;
    };
    let stamp = (*s != 0xFF).then_some(u16::from(*s));
    let data = &payload(*mode, rest)[..];
    let _ = LandblockData::parse_heights(data);
    let Ok(d) = decode_landblock_data(data, stamp) else {
        return;
    };
    // Re-encoding is canonical (shortest VLEs): a fixpoint, never longer than the input,
    // and byte-exact for inputs that already use shortest VLEs.
    let bytes = d.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = LandblockData::parse(&bytes, stamp).expect("re-encoded record decodes");
    assert_eq!(again.to_bytes(), bytes);
    check_chunk(&d.terrain);
    if let Some(c) = &d.ceiling {
        check_chunk(&c.surface);
    }
    let _ = d.first_unusual_field();
    for (x, y) in [(0.0, 0.0), (160.0, 160.0), (77.7, 3.2), (-5.0, 1e9)] {
        let h = d
            .terrain_height(x, y)
            .expect("finite coordinates have a height");
        assert!(h.is_finite() && (0.0..2000.0).contains(&h));
    }
    for i in 0..32 {
        assert!(d.quad_triangles(i, 31 - i).is_some());
    }
    // A record without a stamp may decide build bytes differently, but must not panic.
    let _ = LandblockData::parse(data, None);
});
