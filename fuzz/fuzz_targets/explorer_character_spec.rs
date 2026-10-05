// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`'s character-view text options (`apps/explorer/src/charspec.rs`,
//! included by path so this crate does not pull in the GUI toolkit): `--character`,
//! `--wear`, `--clip`, `--time`, `--sliders`, `--times`, `--angles`, `--tile`, `--focus`
//! and the word-list parser, plus the skin-stream conversion of the character viewer
//! (`mesh_skin` in `apps/explorer/src/geometry.rs`).
//!
//! Input layout: byte 0 selects the mode. Text modes take the rest as (lossy) UTF-8 and
//! feed it to one parser, or split it into words for `CharacterArgs::parse_words`; a
//! parsed word list must survive `to_command_line` and parse back to the same value. The
//! skin mode builds a synthetic skinned mesh level with `tessera_mesh::synth`, overwrites
//! its bone streams with input bytes (indices and raw f32 weights, NaN and negative
//! included) and converts them for a skeleton of an input-chosen bone count.
//!
//! Nothing may panic. A converted skin must have one entry per vertex, weights that sum
//! to 1, and no non-zero weight on a bone at or above the bone count; the renderer must
//! accept it or refuse it with an error.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/explorer/src/charspec.rs"]
mod charspec;

#[allow(dead_code)]
#[path = "../../apps/explorer/src/geometry.rs"]
mod geometry;

use charspec::{
    CharacterArgs, parse_angle_list, parse_clip, parse_figure, parse_focus, parse_item,
    parse_sliders, parse_tile, parse_time, parse_times,
};
use libfuzzer_sys::fuzz_target;
use tessera_mesh::GeometryLimits;
use tessera_mesh::synth::MeshSpec;
use tessera_render::MeshConfig;

fn text_mode(mode: u8, s: &str) {
    match mode % 10 {
        0 => {
            if let Ok(f) = parse_figure(s) {
                // The display form parses back to the same figure.
                assert_eq!(parse_figure(&f.to_string()).ok(), Some(f));
            }
        }
        1 => {
            if let Ok(i) = parse_item(s) {
                assert_eq!(parse_item(&i.to_string()).ok(), Some(i));
            }
        }
        2 => {
            if let Ok(c) = parse_clip(s) {
                assert_eq!(parse_clip(&c.to_string()).ok(), Some(c));
            }
        }
        3 => {
            if let Ok(t) = parse_time(s) {
                assert!(t.is_finite() && t >= 0.0);
            }
        }
        4 => {
            if let Ok(b) = parse_sliders(s) {
                assert!(b.len() <= charspec::MAX_SLIDER_BYTES);
            }
        }
        5 => {
            if let Ok(t) = parse_times(s) {
                assert!(!t.is_empty() && t.len() <= charspec::MAX_TIMES);
            }
        }
        6 => {
            if let Ok(a) = parse_angle_list(s) {
                assert!(!a.is_empty() && a.len() <= charspec::MAX_ANGLES);
                assert!(a.iter().all(|[y, p]| y.is_finite() && p.abs() <= 89.0));
            }
        }
        7 => {
            if let Ok([w, h]) = parse_tile(s) {
                assert!((charspec::TILE_SIDES.0..=charspec::TILE_SIDES.1).contains(&w));
                assert!((charspec::TILE_SIDES.0..=charspec::TILE_SIDES.1).contains(&h));
            }
        }
        8 => {
            let _ = parse_focus(s);
        }
        _ => {
            let words: Vec<&str> = s.split(['\n', '\0', ' ']).collect();
            if let Ok(a) = CharacterArgs::parse_words(words) {
                let line = a.to_command_line();
                let again = CharacterArgs::parse_words(line.split(' '))
                    .expect("a printed command line parses");
                assert_eq!(again, a, "{line}");
            }
        }
    }
}

fn skin_mode(data: &[u8]) {
    let Some((&head, rest)) = data.split_first() else {
        return;
    };
    // 1 to 4 influences, 3 to 192 vertices.
    let k = u32::from(head % 4) + 1;
    let vertices = 3 + u32::from(rest.first().copied().unwrap_or(0) % 190);
    let Ok(mesh) = MeshSpec::single(0x3000_0112 | (k << 16), vertices).build() else {
        return;
    };
    let Ok(mut geo) = mesh.level_geometry(0, &GeometryLimits::default()) else {
        return;
    };
    let bones = usize::from(rest.get(1).copied().unwrap_or(0));
    let mut bytes = rest.iter().skip(2).copied();
    let mut next_f32 = || {
        let b: Vec<u8> = (0..4).map(|_| bytes.next().unwrap_or(0)).collect();
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    };
    if let (Some(j), Some(w)) = (&mut geo.joints, &mut geo.weights) {
        for (jj, ww) in j.iter_mut().zip(w.iter_mut()).take(64) {
            for s in 0..4 {
                let v = next_f32();
                jj[s] = v.to_bits() as u8;
                ww[s] = v;
            }
        }
    }
    if let Some(cut) = rest.get(2)
        && cut % 17 == 0
        && let Some(j) = &mut geo.joints
    {
        j.pop();
    }
    let Ok(s) = geometry::mesh_skin(&geo, bones) else {
        return;
    };
    assert_eq!(s.skin.joints.len(), geo.positions.len());
    assert_eq!(s.skin.weights.len(), geo.positions.len());
    for (j, w) in s.skin.joints.iter().zip(&s.skin.weights) {
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "{w:?}");
        for i in 0..4 {
            assert!(w[i] >= 0.0 && w[i].is_finite());
            if w[i] > 0.0 {
                assert!(usize::from(j[i]) < bones);
            }
        }
    }
    let data = geometry::level_mesh_data(&geo, &geometry::ConvertOptions::default(), "f".into());
    let _ = data.prepare_skinned(&s.skin, &MeshConfig::default());
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode >= 0xF0 {
        skin_mode(rest);
        return;
    }
    let s = String::from_utf8_lossy(rest);
    text_mode(mode, &s);
});
