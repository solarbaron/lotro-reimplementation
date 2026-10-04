// SPDX-License-Identifier: GPL-3.0-or-later
//! The world walker's pure input handling (`apps/client/src/walker/parse.rs` and
//! `geometry.rs`, included by path so this crate does not pull in the window and GPU
//! stack): `--fly-path` and path-point parsing, place-name normalisation and matching,
//! HUD line sanitising, the mesh-level conversion of static-object parts, the texture
//! composition and the box-filter downscale.
//!
//! Input layout: byte 0 selects the mode. Text modes take the rest as (lossy) UTF-8.
//! The mesh mode takes the rest as a mesh record; the texture mode builds small images
//! (of consistent or deliberately wrong pixel length) from the rest.
//!
//! Nothing may panic. Parsed paths have finite ends inside the region grid; normalised
//! names contain only lowercase alphanumerics and single spaces; sanitised lines are short
//! and free of control characters; converted levels keep one submesh per draw and
//! in-range ranges; composed textures keep the base size and downscaled images fit the
//! cap.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/walker/geometry.rs"]
mod geometry;
#[allow(dead_code)]
#[path = "../../apps/client/src/walker/parse.rs"]
mod parse;

use geometry::{AlphaFrom, compose_texture, downscale_to, image_ok, level_mesh_data};
use libfuzzer_sys::fuzz_target;
use parse::{
    MAX_HUD_LINE, match_score, normalize_name, parse_fly_path, parse_path_point, sanitize_line,
};
use tessera_formats::RgbaImage;
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_render::MeshConfig;

const LIMITS: GeometryLimits = GeometryLimits {
    max_vertices: 1 << 16,
    max_indices: 1 << 18,
    max_draws: 64,
    max_bytes: 8 << 20,
    max_amplification: 64,
};

const GRID: f64 = 255.0 * 160.0;

fn text(data: &[u8]) {
    let s = String::from_utf8_lossy(data);
    if let Ok(p) = parse_fly_path(&s) {
        for g in [p.from, p.to] {
            assert!(g.iter().all(|v| v.is_finite() && (0.0..=GRID).contains(v)));
        }
        assert!(p.length().is_finite());
        let mid = p.at(p.length() * 0.5);
        assert!(mid.iter().all(|v| v.is_finite()));
        assert!(p.heading().is_finite());
    }
    if let Ok(p) = parse_path_point(&s) {
        assert!(p.g.iter().all(|v| (0.0..=GRID).contains(v)));
    }
    let n = normalize_name(&s);
    assert!(!n.starts_with(' ') && !n.ends_with(' ') && !n.contains("  "));
    assert!(n.chars().all(|c| c == ' ' || c.is_alphanumeric()));
    let mut mid = s.len() / 2;
    while !s.is_char_boundary(mid) {
        mid -= 1;
    }
    let (a, b) = s.split_at(mid);
    let _ = match_score(&normalize_name(a), &normalize_name(b));
    assert_eq!(match_score(&n, &n), (!n.is_empty()).then_some(3));
    let line = sanitize_line(&s);
    assert!(line.chars().count() <= MAX_HUD_LINE);
    assert!(!line.chars().any(char::is_control));
}

fn mesh(record: &[u8]) {
    let Ok(mesh) = Mesh::decode(record, None, &DecodeOptions::render_only()) else {
        return;
    };
    let Some(level) = geometry::display_level(&mesh) else {
        return;
    };
    let Ok(geo) = mesh.level_geometry(level, &LIMITS) else {
        return;
    };
    let data = level_mesh_data(&geo, "fuzz".into());
    assert_eq!(data.positions.len(), geo.positions.len());
    assert_eq!(data.submeshes.len(), geo.parts.len());
    assert!(data.colors.is_none() && data.tangents.is_none());
    for s in &data.submeshes {
        assert!(u64::from(s.first_index) + u64::from(s.index_count) <= data.indices.len() as u64);
    }
    let _ = data.prepare(&MeshConfig::default());
}

fn image(bytes: &[u8], w: u32, h: u32, consistent: bool) -> RgbaImage {
    let len = (w * h * 4) as usize;
    let mut pixels: Vec<u8> = bytes.iter().copied().cycle().take(len).collect();
    pixels.resize(len, 0x33);
    if !consistent {
        pixels.truncate(len / 3);
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

fn textures(data: &[u8]) {
    let b = |i: usize| data.get(i).copied().unwrap_or(0);
    let (w, h) = (u32::from(b(0) % 40), u32::from(b(1) % 40));
    let (aw, ah) = (u32::from(b(2) % 20) + 1, u32::from(b(3) % 20) + 1);
    let base = image(data, w, h, b(4) & 1 == 0);
    let alpha = image(data.get(5..).unwrap_or(&[]), aw, ah, b(4) & 2 == 0);
    let mode = match b(4) >> 2 & 3 {
        0 => AlphaFrom::Opaque,
        1 => AlphaFrom::Own,
        _ => AlphaFrom::Image(&alpha),
    };
    let with_base = b(4) & 16 == 0;
    if let Some(out) = compose_texture(with_base.then_some(&base), mode) {
        assert!(image_ok(&out));
        if with_base {
            assert_eq!((out.width, out.height), (base.width, base.height));
        }
        let cap = u32::from(b(6) % 16);
        let small = downscale_to(out, cap);
        assert!(image_ok(&small));
        assert!(small.width <= cap.max(1) && small.height <= cap.max(1));
    }
    let _ = downscale_to(base, 3);
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 3 {
        0 => text(rest),
        1 => mesh(rest),
        _ => textures(rest),
    }
});
