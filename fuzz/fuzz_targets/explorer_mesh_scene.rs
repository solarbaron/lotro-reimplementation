// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`'s mesh-viewer conversions (`apps/explorer/src/geometry.rs`,
//! included by path so this crate does not pull in the GUI toolkit): a decoded mesh level
//! into renderer streams (`level_mesh_data`) and through `MeshData::prepare`, and the
//! texture compositions the viewer uploads (`base_texture`, `alpha_mask`, `thumbnail`,
//! `fit_image`, `grid_cell`), and the bounds helpers (`transform_aabb`,
//! `y_up_to_z_up`).
//!
//! Input layout: byte 0 selects the mode, the level and the conversion options. In raw
//! mode the rest is a mesh record. In synthetic mode the next bytes describe a mesh for
//! `tessera_mesh::synth`, which is encoded, damaged by an optional cut, and decoded, so
//! runs reach the conversion without a seed corpus. The last bytes also feed small
//! images (of consistent or deliberately wrong pixel length) to the texture helpers.
//!
//! Nothing may panic. A converted level must keep one renderer submesh per draw, the
//! level's stream lengths, and in-range submesh ranges; preparation may refuse data but
//! never panics. Texture helpers must refuse inconsistent images and keep output sizes.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/explorer/src/geometry.rs"]
mod geometry;

use geometry::{
    AlphaSource, ConvertOptions, alpha_mask, base_texture, fit_image, grid_cell, image_ok,
    level_mesh_data, thumbnail, transform_aabb, y_up_to_z_up,
};
use libfuzzer_sys::fuzz_target;
use tessera_formats::RgbaImage;
use tessera_mesh::synth::{LevelSpec, MeshSpec, SubmeshSpec};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_render::MeshConfig;

const FORMATS: [u32; 8] = [
    0x0000_0112,
    0x0000_0152,
    0x3000_0112,
    0x3000_0152,
    0x3000_0372,
    0x3001_0112,
    0x3002_0252,
    0x3004_0152,
];

/// Small limits so a hostile record cannot make one run slow.
const LIMITS: GeometryLimits = GeometryLimits {
    max_vertices: 1 << 16,
    max_indices: 1 << 18,
    max_draws: 64,
    max_bytes: 8 << 20,
    max_amplification: 64,
};

fn synth(data: &[u8]) -> Option<Vec<u8>> {
    let b = |i: usize| data.get(i).copied().unwrap_or(0);
    let n = usize::from(b(0) % 4) + 1;
    let submeshes = (0..n)
        .map(|i| SubmeshSpec {
            format: FORMATS[usize::from(b(1 + i)) % FORMATS.len()],
            vertices: 3 + u32::from(b(5 + i) % 40),
        })
        .collect();
    let mut draws: Vec<u32> = (0..n as u32).collect();
    if b(9) & 1 == 1 {
        draws.reverse();
    }
    let split = usize::from(b(10)) % n;
    let mut levels = vec![LevelSpec::detail(draws[..=split].to_vec())];
    if split + 1 < n {
        levels.push(LevelSpec::proxy(draws[split + 1..].to_vec()));
    }
    let spec = MeshSpec {
        did: 0x0600_0001,
        flags: 1,
        submeshes,
        levels,
        build_byte: b(11) & 1 == 1,
    };
    let mut bytes = spec.build().ok()?.to_bytes().ok()?;
    let cut = u16::from_le_bytes([b(12), b(13)]);
    if cut != 0 && usize::from(cut) < bytes.len() {
        bytes.truncate(bytes.len() - usize::from(cut));
    }
    Some(bytes)
}

fn convert(record: &[u8], selector: u8) {
    let Ok(mesh) = Mesh::decode(record, None, &DecodeOptions::render_only()) else {
        return;
    };
    if mesh.levels.is_empty() {
        return;
    }
    let level = usize::from(selector >> 4) % mesh.levels.len();
    let Ok(geo) = mesh.level_geometry(level, &LIMITS) else {
        return;
    };
    let options = ConvertOptions {
        vertex_colors: selector & 2 != 0,
        uv_set: usize::from(selector >> 2 & 3),
    };
    let data = level_mesh_data(&geo, &options, "fuzz".into());
    assert_eq!(data.positions.len(), geo.positions.len());
    assert_eq!(data.submeshes.len(), geo.parts.len());
    assert_eq!(data.indices.len(), geo.indices.len());
    if let Some(c) = &data.colors {
        assert!(options.vertex_colors);
        assert_eq!(c.len(), data.positions.len());
        assert!(c.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
        // Vertex alpha never reaches the renderer (it would become coverage).
        assert!(c.iter().all(|v| v[3] == 1.0));
    }
    if let Some(uv) = &data.uvs {
        assert_eq!(uv.len(), data.positions.len());
    }
    for s in &data.submeshes {
        assert!(u64::from(s.first_index) + u64::from(s.index_count) <= data.indices.len() as u64);
    }
    if let Ok(prepared) = data.prepare(&MeshConfig::default()) {
        assert_eq!(prepared.info().vertex_count as usize, data.positions.len());
    }
}

fn image(bytes: &[u8], w: u32, h: u32, consistent: bool) -> RgbaImage {
    let len = (w * h * 4) as usize;
    let mut pixels: Vec<u8> = bytes.iter().copied().cycle().take(len).collect();
    pixels.resize(len, 0x5A);
    if !consistent {
        pixels.truncate(len / 2);
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

fn textures(data: &[u8]) {
    let b = |i: usize| data.get(i).copied().unwrap_or(0);
    let (w, h) = (u32::from(b(0) % 33), u32::from(b(1) % 33));
    let (w2, h2) = (u32::from(b(2) % 17) + 1, u32::from(b(3) % 17) + 1);
    let flags = b(4);
    let a = image(data, w, h, flags & 1 == 0);
    let o = image(data.get(5..).unwrap_or(&[]), w2, h2, flags & 2 == 0);
    for src in [AlphaSource::Opaque, AlphaSource::Own, AlphaSource::From(&o)] {
        if let Some(t) = base_texture(Some(&a), src) {
            assert!(image_ok(&t));
            assert_eq!((t.width, t.height), (a.width, a.height));
        }
        if let Some(t) = base_texture(None, src) {
            assert!(image_ok(&t));
        }
    }
    for invert in [false, true] {
        if let Some(m) = alpha_mask(&a, invert) {
            assert!(image_ok(&m));
            assert!(m.pixels.chunks(4).all(|p| p[3] == 255 && p[0] == p[1]));
        }
    }
    let side = u32::from(flags >> 2);
    if let Some(t) = thumbnail(&a, side) {
        assert!(image_ok(&t));
        assert!(t.width <= side.max(1) && t.height <= side.max(1));
    }
    if let Some(f) = fit_image(&a, side) {
        assert!(image_ok(&f));
        assert!(f.width.max(f.height) <= side);
    }
    let r = f32::from_le_bytes([b(6), b(7), b(8), b(9)]);
    let cell = grid_cell(r);
    assert!(cell.is_finite() && cell > 0.0);
    // Bounds under an arbitrary (possibly degenerate) transform stay ordered or are
    // refused.
    let mut m = [0f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = f32::from(b(i % 16)) / 16.0 - 8.0;
    }
    m[15] = 1.0;
    let lo = [r, -r, f32::from(b(10))];
    let hi = [r.abs(), r, f32::from(b(11))];
    if let Some((l, h)) = transform_aabb(&m, lo, hi) {
        assert!((0..3).all(|i| l[i] <= h[i] && l[i].is_finite() && h[i].is_finite()));
    }
    let v = y_up_to_z_up(lo);
    assert_eq!(v[0].to_bits(), lo[0].to_bits());
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let (tex, mesh_bytes) = rest.split_at(rest.len().min(16));
    textures(tex);
    if mode & 1 == 0 {
        convert(mesh_bytes, mode);
    } else if let Some(record) = synth(mesh_bytes) {
        convert(&record, mode);
    }
});
