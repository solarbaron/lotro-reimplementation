// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-data decoding (`docs/specs/landblock.md` §3) on arbitrary bytes. The first
//! byte selects the directory stamp (0xFF = none, so the §3.9 fallbacks run). The second
//! selects how the rest becomes a payload: as is, or spliced behind a **valid synthetic
//! frame** (header, height grid, optional ceiling grid and ceiling tree, vertex flags,
//! coarse grid) so that inputs far below the 4,456-byte minimum record reach the surface
//! chunks, collision trees, box list and the stamp fallbacks. In the synthetic frame the
//! input also patches the grids: after the seed byte, a count byte and that many (% 16)
//! five-byte patches (u16 sample index % 1,089, u16 height, flag byte) set terrain and
//! ceiling samples and vertex-flag bytes, so the fuzzer chooses the corner heights and the
//! diagonals of any quad.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint), every layer
//! window lies inside its index list, every vertex number is < 1,089, and the height
//! lookup and triangulation helpers work on it: terrain and ceiling heights (bit 7 and bit 0
//! diagonals, §3.5–§3.6) stay within 0–2,000 and return the stored sample exactly at grid
//! points, per-quad triangles of both surfaces, the triangles of every layer (including
//! `whole` layers, §3.8.2), the grid-triangle keys of the collision trees (`collision.md`
//! §6.2) and the tree containers (§3.9).
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_landblock_data -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{
    GRID_SAMPLES, GRID_TRIANGLE_COUNT, GridSurface, LayerKind, SurfaceChunk, grid_triangle_key,
    grid_triangle_quad, vertex_to_height_index,
};
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

/// One fuzz-chosen grid patch: sample index (< 1,089), height, vertex-flag byte.
type Patch = (usize, u16, u8);

fn put_grid(out: &mut Vec<u8>, seed: u8, patches: &[Patch]) {
    let mut grid: Vec<u16> = (0..1089u32).map(|k| (k * u32::from(seed)) as u16).collect();
    for &(k, h, _) in patches {
        grid[k] = h;
    }
    out.extend_from_slice(&1089u32.to_le_bytes());
    for h in grid {
        out.extend_from_slice(&h.to_le_bytes());
    }
}

/// Splits up to 15 five-byte patches off the front of `rest` (a count byte, then the
/// patches); returns them and what is left.
fn patches(rest: &[u8]) -> (Vec<Patch>, &[u8]) {
    let Some((&count, mut rest)) = rest.split_first() else {
        return (Vec::new(), rest);
    };
    let mut out = Vec::new();
    for _ in 0..count % 16 {
        let Some((p, tail)) = rest.split_first_chunk::<5>() else {
            break;
        };
        let k = usize::from(u16::from_le_bytes([p[0], p[1]])) % GRID_SAMPLES;
        out.push((k, u16::from_le_bytes([p[2], p[3]]), p[4]));
        rest = tail;
    }
    (out, rest)
}

/// Builds a payload from `mode` and the fuzz bytes `rest`; also returns the patched sample
/// indices (empty in raw mode).
fn payload(mode: u8, rest: &[u8]) -> (Vec<u8>, Vec<usize>) {
    if mode.is_multiple_of(4) {
        return (rest.to_vec(), Vec::new());
    }
    let seed = rest.first().copied().unwrap_or(0);
    let (patches, rest) = patches(rest.get(1..).unwrap_or(&[]));
    let mut out = Vec::with_capacity(8192 + rest.len());
    out.extend_from_slice(&DID.to_le_bytes());
    out.extend_from_slice(&(if seed & 1 != 0 { DID + 0x0020_0000 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    put_grid(&mut out, seed, &patches);
    let mut rest = rest;
    match mode % 4 {
        // No ceiling.
        1 => out.push(0),
        // Ceiling without a tree.
        2 => {
            out.push(1);
            put_grid(&mut out, seed.wrapping_add(1), &patches);
            out.push(0);
        }
        // Ceiling with a tree whose code length and code come from the input. Bit 0x20 of
        // the length byte appends one more input byte as the `build_byte`, which a tree has
        // iff the stamp is >= 38 (landblock.md §3.9); with the bit clear, trees of the
        // older layout (stamps <= 33, or the stamp-less fallback) can be accepted too.
        _ => {
            out.push(1);
            put_grid(&mut out, seed.wrapping_add(1), &patches);
            out.push(1);
            out.extend_from_slice(&[0; 16]);
            let first = rest.first().copied().unwrap_or(0);
            let len = usize::from(first % 32);
            let build = usize::from(first & 0x20 != 0);
            let take = (1 + len + build).min(rest.len());
            out.extend_from_slice(&(len as u32).to_le_bytes());
            out.extend_from_slice(rest.get(1..take).unwrap_or(&[]));
            rest = rest.get(take..).unwrap_or(&[]);
        }
    }
    // Vertex flags and coarse grid.
    out.extend_from_slice(&1089u32.to_le_bytes());
    let mut flags: Vec<u8> = (0..1089u32)
        .map(|k| if k % 3 == 0 { 0x80 } else { 0x7F })
        .collect();
    for &(k, _, f) in &patches {
        flags[k] = f;
    }
    out.extend_from_slice(&flags);
    out.extend_from_slice(&289u32.to_le_bytes());
    out.extend_from_slice(&[0; 4 * 289]);
    out.extend_from_slice(rest);
    (out, patches.iter().map(|p| p.0).collect())
}

fuzz_target!(|input: &[u8]| {
    let [s, mode, rest @ ..] = input else {
        return;
    };
    let stamp = (*s != 0xFF).then_some(u16::from(*s));
    let (data, patched) = payload(*mode, rest);
    let data = &data[..];
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
    // Every layer's triangles: the stored window, all grid triangles for a whole layer.
    for surface in [GridSurface::Terrain, GridSurface::Ceiling] {
        let Some(c) = d.surface_chunk(surface) else {
            continue;
        };
        for l in &c.layers {
            let tris = d.layer_triangles(surface, l).expect("decoded layer");
            match l.kind() {
                LayerKind::Whole => assert_eq!(tris.len(), GRID_TRIANGLE_COUNT),
                LayerKind::Empty => assert!(tris.is_empty()),
                LayerKind::Masked | LayerKind::Opaque => {
                    assert_eq!(tris.len(), usize::from(l.prims));
                }
            }
        }
    }
    // At a grid sample the height is the stored sample, exactly (both surfaces).
    let corners = [33 * 31 + 31, 33 * 32 + 32, 32, 33 * 32];
    for k in patched.iter().copied().chain(corners) {
        let (i, j) = (k / 33, k % 33);
        let (x, y) = (5.0 * i as f32, 5.0 * j as f32);
        assert_eq!(
            d.terrain_height(x, y),
            d.heights.height(i, j),
            "terrain ({i}, {j})"
        );
        if let Some(c) = &d.ceiling {
            assert_eq!(
                d.ceiling_height(x, y),
                c.heights.height(i, j),
                "ceiling ({i}, {j})"
            );
        }
    }
    let _ = d.first_unusual_field();
    for (x, y) in [
        (0.0, 0.0),
        (160.0, 160.0),
        (77.7, 3.2),
        (-5.0, 1e9),
        (160.0, 157.3),
        (159.999, 159.998),
    ] {
        let h = d
            .terrain_height(x, y)
            .expect("finite coordinates have a height");
        // The largest stored sample, 65,535, is 2,000 units (landblock.md §3.4).
        assert!(h.is_finite() && (0.0..=2000.0).contains(&h));
        let n = d
            .terrain_normal(x, y)
            .expect("finite coordinates have a normal");
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!(n[2] > 0.0 && (len - 1.0).abs() < 1e-3, "{n:?}");
    }
    for i in 0..32 {
        assert!(d.quad_triangles(i, 31 - i).is_some());
        assert_eq!(
            d.ceiling_quad_triangles(i, i).is_some(),
            d.ceiling.is_some()
        );
    }
    assert!(d.quad_triangles(32, 0).is_none() && d.ceiling_quad_triangles(0, 32).is_none());
    for (x, y) in [
        (0.0, 0.0),
        (160.0, 160.0),
        (12.5, 99.9),
        (-1.0, f32::INFINITY),
    ] {
        match d.ceiling_height(x, y) {
            Some(h) => assert!(h.is_finite() && (0.0..=2000.0).contains(&h)),
            None => assert!(d.ceiling.is_none()),
        }
    }
    // Every key names a triangle of a present surface; keys >= 2,048 name none.
    let key_seed = rest.first().copied().map_or(0u16, u16::from);
    for key in [0u16, 1, 2047, 2048, key_seed * 8, u16::MAX] {
        let (i, j, west) = match grid_triangle_quad(key) {
            Some(q) => q,
            None => {
                assert!(key >= 2048);
                assert!(d.grid_triangle(GridSurface::Terrain, key).is_none());
                continue;
            }
        };
        assert_eq!(grid_triangle_key(i, j, west), Some(key));
        for surface in [GridSurface::Terrain, GridSurface::Ceiling] {
            let present = d.surface_heights(surface).is_some();
            let t = d.grid_triangle(surface, key);
            assert_eq!(t.is_some(), present);
            if let Some(p) = d.grid_triangle_positions(surface, key) {
                assert!(p.iter().flatten().all(|v| v.is_finite()));
            }
            if let Some(tree) = d.surface_tree(surface) {
                assert_eq!(tree.to_bytes().len(), tree.stored_len());
            }
        }
    }
    // A record without a stamp may decide build bytes differently, but must not panic.
    let _ = LandblockData::parse(data, None);
});
