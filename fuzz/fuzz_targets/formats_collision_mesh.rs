// SPDX-License-Identifier: GPL-3.0-or-later
//! Stand-alone collision meshes, prefix 0x66 (`docs/specs/collision.md` §4), on arbitrary
//! bytes. The first byte selects the directory stamp (0xFF = none, so the stamp-less
//! `build_byte` fallback runs). The second selects how the rest becomes a payload: as is,
//! or as the tree code of a valid synthetic mesh (a few vertices and triangles chosen by
//! the input), so that short inputs reach the tree container and its `build_byte`.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint, never longer
//! than the input), every triangle names existing vertices, and the tree walks and queries
//! run within their budgets.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::collision::CollisionMesh;

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let (n, rest) = rest.split_first().map_or((0, rest), |(n, r)| (*n, r));
    let verts = usize::from(n % 8) + 1;
    let tris = usize::from(n / 8 % 8);
    let mut out = vec![verts as u8];
    for k in 0..verts {
        for c in [k as f32, (k * 2) as f32, 0.5] {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    out.push(tris as u8);
    for k in 0..tris {
        for i in [k, k + 1, k + 2] {
            out.extend_from_slice(&((i % verts) as u16).to_le_bytes());
        }
    }
    out.push(1);
    for v in [-0.05f32, -0.05, 0.45, 65_536.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    // Code length from the next byte; the rest is code and perhaps a build byte.
    let (len, code) = rest.split_first().map_or((0, rest), |(n, r)| (*n, r));
    let len = usize::from(len).min(code.len());
    out.extend_from_slice(&(len as u32).to_le_bytes());
    out.extend_from_slice(code);
    out
}

fuzz_target!(|input: &[u8]| {
    let [s, mode, rest @ ..] = input else {
        return;
    };
    let stamp = (*s != 0xFF).then_some(u16::from(*s));
    let data = payload(*mode, rest);
    let Ok(m) = CollisionMesh::parse(&data, stamp) else {
        return;
    };
    let bytes = m.to_bytes();
    assert_eq!(bytes.len(), m.encoded_len());
    assert!(bytes.len() <= data.len());
    let again = CollisionMesh::parse(&bytes, stamp).expect("re-encoded record decodes");
    assert_eq!(again.to_bytes(), bytes);
    for t in 0..m.triangles.len() {
        assert!(m.triangle_positions(t).is_some());
    }
    assert!(m.triangle_positions(m.triangles.len()).is_none());
    assert_eq!(m.bounds().is_some(), !m.vertices.is_empty());
    let code = m.tree_code();
    if let Ok(w) = code.walk() {
        let _ = w.keys_below(m.triangles.len() as u64);
    }
    for v in m.vertices.iter().take(8) {
        let _ = code.point_keys(*v);
    }
    if let Some((lo, hi)) = m.bounds() {
        let _ = code.box_keys(lo, hi);
    }
    let _ = CollisionMesh::parse(&data, None);
});
