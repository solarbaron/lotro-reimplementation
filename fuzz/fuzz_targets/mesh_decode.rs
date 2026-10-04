// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-mesh` (`docs/specs/mesh.md`): `Mesh::decode` with and without a directory
//! stamp, `MeshHeader::parse`, `Mesh::validate`, `Mesh::level_geometry` and the encoder.
//!
//! Input layout: byte 0 selects the mode and the stamp. In raw mode the rest is a record.
//! In synthetic mode the next bytes describe a mesh for `tessera_mesh::synth` (formats,
//! vertex counts, levels), which is encoded and then damaged by (offset, value) edits
//! and an optional cut taken from the remaining bytes, so every run reaches the deep
//! parts of the decoder without a seed corpus.
//!
//! Nothing may panic. Whatever decodes losslessly must encode back to the exact input,
//! and a flattened level must have consistent parts, streams and in-range indices within
//! its limits. A decoded mesh is then edited by hand (a stream shortened or lengthened):
//! validation must report it and flattening must refuse it, without panicking.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec, TreeSpec};
use tessera_mesh::{DecodeOptions, GeometryLimits, LevelKind, Mesh, MeshError, MeshHeader};

const FORMATS: [u32; 12] = [
    0x0000_0112,
    0x0000_0152,
    0x0000_0102,
    0x3000_0112,
    0x3000_0152,
    0x3000_0372,
    0x3001_0112,
    0x3002_0252,
    0x3003_0712,
    0x3004_0152,
    0x30FF_0F72,
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

fn exercise(bytes: &[u8], stamp: Option<u16>) {
    let header = MeshHeader::parse(bytes);
    let Ok(mesh) = Mesh::decode(bytes, stamp, &DecodeOptions::lossless()) else {
        return;
    };
    assert_eq!(header.as_ref().ok(), Some(&mesh.header()));
    assert_eq!(mesh.to_bytes().expect("lossless mesh encodes"), bytes);
    let report = mesh.validate();
    assert!(report.issues.len() <= tessera_mesh::validate::MAX_ISSUES);
    let limits = GeometryLimits {
        max_vertices: 1 << 18,
        max_indices: 1 << 20,
        max_bytes: 32 << 20,
        ..GeometryLimits::default()
    };
    for l in 0..mesh.levels.len() {
        let Ok(g) = mesh.level_geometry(l, &limits) else {
            continue;
        };
        let n = g.positions.len();
        assert!(g.stream_bytes() <= limits.max_bytes);
        assert_eq!(g.normals.len(), n);
        assert!(g.tangents.as_ref().is_none_or(|t| t.len() == n));
        assert!(g.uv_sets.iter().all(|s| s.len() == n));
        assert!(g.colors.as_ref().is_none_or(|c| c.len() == n));
        assert!(g.joints.as_ref().is_none_or(|j| j.len() == n));
        assert!(g.weights.as_ref().is_none_or(|w| w.len() == n));
        for i in 0..g.indices.len() {
            assert!((g.indices.get(i).unwrap() as usize) < n);
        }
        let mut next = 0;
        for p in &g.parts {
            assert_eq!(p.first_index, next);
            next += p.index_count;
        }
        assert_eq!(next as usize, g.indices.len());
    }
    let _ = mesh.heap_size();
    edited(&mesh, bytes);
    // Render-only decoding agrees on the geometry.
    let lite = Mesh::decode(bytes, stamp, &DecodeOptions::render_only()).expect("same record");
    assert_eq!(lite.submeshes.len(), mesh.submeshes.len());
    assert_eq!(lite.levels.len(), mesh.levels.len());
}

/// Shortens or lengthens one stream of one submesh, chosen by the first input bytes.
fn edited(mesh: &Mesh, bytes: &[u8]) {
    let Some(&pick) = bytes.first() else { return };
    let mut m = mesh.clone();
    let n = m.submeshes.len();
    if n == 0 {
        return;
    }
    let which = usize::from(pick) % n;
    let st = &mut m.submeshes[which].streams;
    let grow = pick & 0x80 != 0;
    let changed = match (pick >> 3) % 6 {
        0 => {
            if grow {
                st.positions.push([0.0; 3]);
                true
            } else {
                st.positions.pop().is_some()
            }
        }
        1 => st.normals.as_mut().is_some_and(|v| v.pop().is_some()),
        2 => st.tangents.as_mut().is_some_and(|v| v.pop().is_some()),
        3 => st.uv_sets.last_mut().is_some_and(|v| v.pop().is_some()),
        4 => st.colors.as_mut().is_some_and(|v| v.pop().is_some()),
        _ => st.skin.as_mut().is_some_and(|s| {
            if grow {
                s.weights.extend([0.5; 7]);
                true
            } else {
                s.joints.pop().is_some()
            }
        }),
    };
    if !changed {
        return;
    }
    assert!(
        m.validate()
            .issues
            .iter()
            .any(|i| i.kind == tessera_mesh::IssueKind::StreamLength)
    );
    for (l, level) in m.levels.iter().enumerate() {
        let r = m.level_geometry(l, &GeometryLimits::default());
        // Stream lengths are checked while resolving draws, before any other data check
        // (only the draw-count limit comes first).
        if level.draws.iter().any(|d| d.vertex_block as usize == which) {
            assert!(
                matches!(
                    r,
                    Err(MeshError::StreamLength { .. } | MeshError::LimitExceeded { .. })
                ),
                "{r:?}"
            );
        }
    }
    assert!(m.to_bytes().is_err());
    let _ = m.heap_size();
}

fn synthetic(b: &mut Bytes<'_>) -> Option<(Vec<u8>, u16)> {
    let n = 1 + b.byte() % 3;
    let submeshes: Vec<SubmeshSpec> = (0..n)
        .map(|_| SubmeshSpec {
            format: FORMATS[usize::from(b.byte()) % FORMATS.len()],
            vertices: 3 + u32::from(b.byte() % 40),
        })
        .collect();
    let nl = 1 + b.byte() % 3;
    let levels = (0..nl)
        .map(|_| {
            let c = b.byte();
            let draws = (0..1 + c % 3)
                .map(|_| u32::from(b.byte()) % u32::from(n))
                .collect();
            LevelSpec {
                kind: if c & 0x40 != 0 {
                    LevelKind::Proxy
                } else {
                    LevelKind::Detail
                },
                draws,
                collision: c & 0x08 != 0,
                tree: match (c >> 4) & 3 {
                    0 => TreeSpec::None,
                    1 => TreeSpec::NoCode,
                    _ => TreeSpec::Code {
                        len: u32::from(b.byte() % 32),
                    },
                },
            }
        })
        .collect();
    let spec = MeshSpec {
        did: 0x0600_0000 | u32::from(b.u16()),
        flags: u32::from(b.byte()),
        submeshes,
        levels,
        build_byte: b.byte() & 1 == 1,
    };
    let mesh = spec.build().ok()?;
    let bytes = mesh.to_bytes().expect("synthetic mesh encodes");
    Some((bytes, spec.stamp()))
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let mode = b.byte();
    let stamp = match (mode >> 1) & 3 {
        0 => None,
        1 => Some(7),
        2 => Some(39),
        _ => Some(u16::from(mode >> 3) + 30),
    };
    if mode & 1 == 0 {
        exercise(b.rest(), stamp);
        return;
    }
    let Some((mut bytes, own_stamp)) = synthetic(&mut b) else {
        return;
    };
    // The undamaged record decodes with its own stamp and is clean.
    let clean = Mesh::decode(&bytes, Some(own_stamp), &DecodeOptions::lossless())
        .expect("synthetic mesh decodes");
    assert_eq!(clean.to_bytes().unwrap(), bytes);
    let edits = b.byte() % 8;
    for _ in 0..edits {
        let at = usize::from(b.u16()) % bytes.len();
        bytes[at] = b.byte();
    }
    let cut = b.u16();
    if cut & 0x8000 != 0 {
        bytes.truncate(usize::from(cut & 0x7FFF) % (bytes.len() + 1));
    }
    exercise(&bytes, stamp);
    exercise(&bytes, Some(own_stamp));
});
