// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render` skinned mesh input (`docs/specs/render.md` §2.4): `MeshSkin::validate`,
//! `MeshData::prepare_skinned` and `validate_skinned` on arbitrary bone streams (lengths
//! that disagree with the vertex count, palette sizes 0 or above 256, bone indices outside
//! the palette, negative, NaN, infinite, unnormalised or all-zero weights) on top of a
//! small valid or invalid mesh. Nothing may panic; an accepted skin has a consistent
//! summary and the prepared mesh is linear in the input.
//!
//! Input layout: a 5-byte header (vertex count, palette size low/high, flags, weight mode),
//! then the bytes every stream reads its values from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_render::{MAX_PALETTE_BONES, MeshConfig, MeshData, MeshLimits, MeshSkin, RenderError};

/// Reads values from the input, cycling, zero when it is empty.
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
    fn f32(&mut self) -> f32 {
        f32::from_bits(u32::from_le_bytes([
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
        ]))
    }
    /// Weights: mostly a valid partition of unity, sometimes perturbed or raw bits.
    fn weights(&mut self, mode: u8) -> [f32; 4] {
        match mode % 4 {
            0 => {
                let raw = [self.byte(), self.byte(), self.byte(), self.byte()].map(f32::from);
                let sum: f32 = raw.iter().sum();
                if sum > 0.0 {
                    raw.map(|w| w / sum)
                } else {
                    [1.0, 0.0, 0.0, 0.0]
                }
            }
            1 => {
                let k = usize::from(self.byte() % 4);
                let mut w = [0.0; 4];
                w[k] = 1.0 + f32::from(self.byte() as i8) * 1e-5;
                w
            }
            2 => [self.f32(), self.f32(), self.f32(), self.f32()],
            _ => [
                f32::from(self.byte()) / 255.0,
                f32::from(self.byte() as i8) / 127.0,
                0.0,
                0.0,
            ],
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let (head, rest) = data.split_at(5);
    let verts = usize::from(head[0] % 64) + 3;
    let palette_size = u16::from_le_bytes([head[1], head[2] & 0x01]);
    let flags = head[3];
    let mode = head[4];
    let mut b = Bytes { data: rest, pos: 0 };

    let positions: Vec<[f32; 3]> = (0..verts)
        .map(|i| [i as f32 * 0.1, f32::from(b.byte()) / 64.0, (i % 3) as f32])
        .collect();
    // A fan of triangles, sometimes with a broken index.
    let mut indices: Vec<u32> = (1..verts as u32 - 1).flat_map(|i| [0, i, i + 1]).collect();
    if flags & 1 != 0 {
        indices[0] = verts as u32 + 1;
    }
    let mesh = MeshData::new(positions, indices);
    let mut skin_len = |bit: u8| {
        if flags & bit != 0 {
            verts + usize::from(b.byte() % 3) - 1
        } else {
            verts
        }
    };
    let joint_count = skin_len(2);
    let weight_count = skin_len(4);
    let joints: Vec<[u8; 4]> = (0..joint_count)
        .map(|_| {
            [b.byte(), b.byte(), b.byte(), b.byte()].map(|j| {
                if flags & 8 != 0 {
                    j
                } else {
                    // Below the palette (u16 arithmetic: 256 does not fit u8).
                    (u16::from(j) % palette_size.clamp(1, 256)) as u8
                }
            })
        })
        .collect();
    let weights: Vec<[f32; 4]> = (0..weight_count).map(|_| b.weights(mode)).collect();
    let skin = MeshSkin {
        joints,
        weights,
        palette_size,
    };
    let config = MeshConfig {
        limits: MeshLimits {
            max_vertices: 200,
            max_indices: 500,
            max_submeshes: 4,
        },
        wireframe_edges: flags & 16 == 0,
        ..MeshConfig::default()
    };

    let summary = skin.validate(verts);
    if let Ok(info) = summary {
        assert_eq!(info.palette_size, palette_size);
        assert!(usize::from(palette_size) <= MAX_PALETTE_BONES && palette_size > 0);
        assert!(u16::from(info.max_joint) < palette_size);
        assert!((1..=4).contains(&info.max_influences));
        assert_eq!(skin.joints.len(), verts);
        assert_eq!(skin.weights.len(), verts);
        for w in &skin.weights {
            assert!(w.iter().all(|x| x.is_finite() && *x >= 0.0));
        }
    } else {
        assert!(matches!(summary, Err(RenderError::InvalidSkin(_))));
    }

    match mesh.prepare_skinned(&skin, &config) {
        Ok(prepared) => {
            let info = prepared.info();
            assert!(summary.is_ok(), "prepare accepted a skin validate rejected");
            assert_eq!(info.skin, summary.ok());
            assert_eq!(
                prepared.gpu_bytes(),
                u64::from(info.vertex_count) * (48 + 12)
                    + u64::from(info.index_count + info.edge_index_count) * 4
            );
            assert_eq!(
                mesh.validate_skinned(&skin, &config).ok(),
                Some(info.clone())
            );
        }
        Err(RenderError::InvalidSkin(_)) => {
            assert!(summary.is_err());
            assert!(mesh.prepare(&config).is_ok(), "mesh errors come first");
        }
        Err(_) => assert!(mesh.prepare(&config).is_err()),
    }
});
