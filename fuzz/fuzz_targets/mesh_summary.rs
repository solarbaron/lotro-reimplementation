// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-mesh` (`docs/specs/mesh.md` §3, §5.7, §6.2, §7): `MeshSummary::parse` and the
//! skin helpers (`Mesh::skinned_level`, `Skin::influence_list`, `SkinnedLevel::part_streams`).
//!
//! Input layout: byte 0 selects raw or synthetic mode. In raw mode the rest is a record. In
//! synthetic mode the next bytes describe a mesh for `tessera_mesh::synth` (formats with and
//! without skinning, vertex counts, levels), which is encoded and then damaged by
//! (offset, value) edits and an optional cut.
//!
//! Nothing may panic. Whatever the full decoder accepts, the summary must accept with the
//! same DID, flags, surfaces, vertex and triangle counts. A decoded mesh's skinned level
//! must report bones below the skeleton bone count it is given, or refuse with
//! `JointOutOfRange`; a synthetic mesh's influence lists must match the §6.2 rule; a part's
//! streams must use exactly the bones its `PartSkin` reports. A hand-edited part range never
//! panics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh, MeshError, MeshSummary};

const FORMATS: [u32; 8] = [
    0x0000_0112,
    0x0000_0152,
    0x3000_0112,
    0x3001_0112,
    0x3002_0252,
    0x3003_0712,
    0x3004_0152,
    0x0005_0002,
];

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn rest(&self) -> &[u8] {
        self.data.get(self.pos..).unwrap_or(&[])
    }
}

fn exercise(bytes: &[u8], bone_count: usize, synthetic: bool) {
    let summary = MeshSummary::parse(bytes);
    if let Ok(s) = &summary {
        let _ = (
            s.vertex_count(),
            s.triangle_count(),
            s.is_stub(),
            s.is_skinned(),
        );
        assert!(s.submeshes.len() == s.surfaces.len());
    }
    let Ok(mesh) = Mesh::decode(bytes, None, &DecodeOptions::lossless()) else {
        return;
    };
    let s = summary.expect("a record the decoder accepts summarises");
    assert_eq!(s.did, mesh.did);
    assert_eq!(s.flags, mesh.flags);
    assert_eq!(s.surfaces, mesh.surfaces);
    assert_eq!(s.is_skinned(), mesh.has_skinned_submesh());
    let vertices: u64 = mesh
        .submeshes
        .iter()
        .map(|m| u64::from(m.vertex_count))
        .sum();
    assert_eq!(s.vertex_count(), vertices);
    let triangles: u64 = mesh
        .index_buffers
        .iter()
        .map(|b| b.triangle_count() as u64)
        .sum();
    assert_eq!(s.triangle_count(), triangles);
    for sm in &mesh.submeshes {
        let list = sm.computed_influence_list();
        let nonzero = sm
            .streams
            .skin
            .as_ref()
            .map_or(0, |k| k.weights.iter().filter(|w| **w != 0.0).count());
        assert_eq!(list.len(), nonzero);
        if synthetic {
            assert!(sm.influence_list_matches());
        }
        if let Some(st) = sm.skin_streams() {
            assert_eq!(st.joints.len(), sm.vertex_count as usize);
        }
    }
    let limits = GeometryLimits {
        max_vertices: 1 << 18,
        max_indices: 1 << 20,
        max_bytes: 32 << 20,
        ..GeometryLimits::default()
    };
    for level in 0..mesh.levels.len() {
        match mesh.skinned_level(level, &limits, Some(bone_count)) {
            Ok(sl) => {
                assert_eq!(sl.parts.len(), sl.geometry.parts.len());
                if let Some(m) = sl.max_joint {
                    assert!(usize::from(m) < bone_count);
                }
                for (i, p) in sl.parts.iter().enumerate() {
                    assert!(p.bones_used.windows(2).all(|w| w[0] < w[1]));
                    let Some(streams) = sl.part_streams(i) else {
                        continue;
                    };
                    let (joints, weights) = (streams.joints, streams.weights);
                    let part = &sl.geometry.parts[i];
                    assert_eq!(joints.len(), part.vertex_count as usize);
                    assert_eq!(weights.len(), joints.len());
                    // Every bone a non-zero four-influence weight uses is one the part's
                    // skin uses.
                    for (j, w) in joints.iter().zip(weights) {
                        for k in 0..4 {
                            if w[k] != 0.0 {
                                assert!(p.bones_used.binary_search(&j[k]).is_ok());
                            }
                        }
                    }
                }
                // A hand-edited range past the end (or overflowing u32) is refused.
                let mut edited = sl.clone();
                if let Some(part) = edited.geometry.parts.first_mut() {
                    part.base_vertex = u32::MAX;
                    part.vertex_count = u32::MAX;
                    assert!(edited.part_streams(0).is_none());
                }
            }
            Err(MeshError::JointOutOfRange { joint, .. }) => {
                assert!(usize::from(joint) >= bone_count);
            }
            Err(_) => {}
        }
    }
}

fn synthetic(b: &mut Bytes<'_>) -> Option<Vec<u8>> {
    let n = 1 + b.byte() % 3;
    let submeshes: Vec<SubmeshSpec> = (0..n)
        .map(|_| SubmeshSpec {
            format: FORMATS[usize::from(b.byte()) % FORMATS.len()],
            vertices: 3 + u32::from(b.byte() % 40),
        })
        .collect();
    let draws = (0..n).map(u32::from).collect();
    let spec = MeshSpec {
        did: 0x0600_0000 | u32::from(b.u16()),
        flags: 0x1000_0000 | u32::from(b.byte()),
        submeshes,
        levels: vec![LevelSpec::detail(draws)],
        build_byte: false,
    };
    spec.build().ok()?.to_bytes().ok()
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let mode = b.byte();
    let bone_count = usize::from(b.byte()) + 1;
    if mode & 1 == 0 {
        exercise(b.rest(), bone_count, false);
        return;
    }
    let Some(mut bytes) = synthetic(&mut b) else {
        return;
    };
    exercise(&bytes, bone_count, true);
    let edits = b.byte() % 8;
    for _ in 0..edits {
        let at = usize::from(b.u16()) % bytes.len();
        bytes[at] = b.byte();
    }
    let cut = b.u16();
    if cut & 0x8000 != 0 {
        bytes.truncate(usize::from(cut & 0x7FFF) % (bytes.len() + 1));
    }
    exercise(&bytes, bone_count, false);
});
