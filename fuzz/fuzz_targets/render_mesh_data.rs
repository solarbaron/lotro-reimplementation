// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render` mesh input (`docs/specs/render.md` §2.1): `MeshData::prepare` /
//! `validate` on arbitrary streams (lengths that disagree, out-of-range indices, bad,
//! overlapping or duplicated submesh ranges, NaN/infinite attributes), with and without
//! wireframe edges, plus `compute_normals`, `compute_tangents`, `flip_winding` and the
//! procedural shape builders. Nothing may panic; an accepted mesh has a consistent,
//! finite summary whose size is linear in the input.
//!
//! Input layout: a 6-byte header (vertex count, index count, submesh count, flags), then
//! the bytes every stream reads its values from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_render::{Indices, MeshConfig, MeshData, MeshLimits, Submesh};

/// Reads little-endian values from the input, cycling, zero when it is empty.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    /// Mostly small, sometimes raw floats (NaN, inf, huge).
    fn coord(&mut self) -> f32 {
        let tag = self.byte();
        if tag & 0x7 == 0 {
            self.f32()
        } else {
            f32::from(self.byte() as i8) / 16.0
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }
    let (head, rest) = data.split_at(6);
    let verts = usize::from(head[0]) + 1;
    let index_count = usize::from(u16::from_le_bytes([head[1], head[2]])) % 600;
    let submeshes = usize::from(head[3] % 6);
    let flags = head[4];
    let opt_len = |bit: u8| if head[5] & bit != 0 { verts + 1 } else { verts };
    let mut b = Bytes { data: rest, pos: 0 };

    let positions = (0..verts)
        .map(|_| [b.coord(), b.coord(), b.coord()])
        .collect();
    let mut m = MeshData {
        label: None,
        positions,
        normals: (flags & 1 != 0).then(|| {
            (0..opt_len(1))
                .map(|_| [b.coord(), b.coord(), b.coord()])
                .collect()
        }),
        tangents: (flags & 2 != 0).then(|| {
            (0..opt_len(2))
                .map(|_| [b.coord(), b.coord(), b.coord(), b.coord()])
                .collect()
        }),
        uvs: (flags & 4 != 0).then(|| (0..opt_len(4)).map(|_| [b.coord(), b.coord()]).collect()),
        colors: (flags & 8 != 0).then(|| {
            (0..opt_len(8))
                .map(|_| [b.coord(), b.coord(), b.coord(), b.coord()])
                .collect()
        }),
        indices: if flags & 16 != 0 {
            Indices::U16(
                (0..index_count)
                    .map(|_| u16::from(b.byte()) % (verts as u16 + 2))
                    .collect(),
            )
        } else {
            Indices::U32(
                (0..index_count)
                    .map(|_| {
                        if flags & 32 != 0 {
                            b.u32()
                        } else {
                            u32::from(b.byte()) % (verts as u32 + 1)
                        }
                    })
                    .collect(),
            )
        },
        submeshes: (0..submeshes)
            .map(|_| Submesh {
                first_index: u32::from(b.byte()) * 3 / 2,
                index_count: if flags & 64 != 0 {
                    b.u32()
                } else {
                    u32::from(b.byte()) * 3
                },
            })
            .collect(),
    };
    let config = MeshConfig {
        limits: MeshLimits {
            max_vertices: 200,
            max_indices: 500,
            max_submeshes: 4,
        },
        wireframe_edges: flags & 128 == 0,
        ..MeshConfig::default()
    };

    if let Ok(prepared) = m.prepare(&config) {
        let info = prepared.info();
        assert_eq!(info.vertex_count as usize, m.positions.len());
        assert_eq!(info.index_count as usize, m.indices.len());
        assert!(info.index_count % 3 == 0 && info.index_count > 0);
        assert!(info.edge_index_count % 2 == 0);
        assert!(info.edge_index_count <= info.index_count * 2);
        if !config.wireframe_edges {
            assert_eq!(info.edge_index_count, 0);
        }
        assert_eq!(
            prepared.gpu_bytes(),
            u64::from(info.vertex_count) * 40
                + u64::from(info.index_count + info.edge_index_count) * 4
        );
        assert!(!info.submeshes.is_empty());
        let covered: u64 = info
            .submeshes
            .iter()
            .map(|s| u64::from(s.range.index_count))
            .sum();
        assert!(covered <= u64::from(info.index_count));
        assert_eq!(m.validate(&config).as_ref().ok(), Some(info));
        assert!(info.sphere.center.is_finite() && info.sphere.radius >= 0.0);
        for s in &info.submeshes {
            assert!(s.range.first_index + s.range.index_count <= info.index_count);
            assert!(s.sphere.radius >= 0.0);
        }
        assert_eq!(info.has_tangents, m.tangents.is_some());
    }

    let mut n = m.clone();
    n.compute_normals();
    assert_eq!(n.normals.as_ref().map(Vec::len), Some(n.positions.len()));
    if n.compute_tangents() {
        assert_eq!(n.tangents.as_ref().map(Vec::len), Some(n.positions.len()));
        assert!(
            n.tangents
                .iter()
                .flatten()
                .all(|t| t.iter().all(|c| c.is_finite()) && t[3].abs() == 1.0)
        );
    }
    m.flip_winding();
    let _ = m.validate(&config);

    // Shape builders with fuzzed (bounded) parameters.
    let segs = u32::from(flags % 24);
    let size = b.coord();
    for shape in [
        MeshData::cube(size),
        MeshData::uv_sphere(size, segs, segs / 2),
        MeshData::plane(size, b.coord(), segs, segs / 3),
        MeshData::tube(size, b.coord(), segs),
    ] {
        let _ = shape.validate(&MeshConfig::default());
    }
});
