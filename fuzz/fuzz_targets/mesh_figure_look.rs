// SPDX-License-Identifier: GPL-3.0-or-later
//! The figure look (`tessera_mesh::assembly::look`, `docs/specs/appearance.md` §3.5) on
//! arbitrary input: look candidates (attach, layer, colour and edit nodes with arbitrary
//! fields, picked and stopped flags) built from the first bytes, and record bytes for every
//! DID the look reads (compositor records and surfaces) cut from the rest. The look runs
//! with the face-compositing option from the input, twice through one cache (the second
//! run must give the same images).
//!
//! Nothing may panic; every composed image must match its size.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run mesh_figure_look -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::script_table::resolve::{
    ClipResolution, LookCandidate, LookEffect, MaterialEdit,
};
use tessera_formats::script_table::{
    CompositorColourNode, CompositorLayerNode, TextureCompositorNode,
};
use tessera_mesh::assembly::AssemblyOptions;
use tessera_mesh::assembly::look::{LookCache, LookLimits, figure_look};

fn u32_at(b: &[u8], i: usize) -> u32 {
    let mut w = [0u8; 4];
    for (k, x) in w.iter_mut().enumerate() {
        *x = b.get(i + k).copied().unwrap_or(0);
    }
    u32::from_le_bytes(w)
}

fuzz_target!(|data: &[u8]| {
    let n = usize::from(data.first().copied().unwrap_or(0) % 16);
    let mut look = Vec::new();
    let mut at = 1;
    for _ in 0..n {
        let kind = data.get(at).copied().unwrap_or(0);
        let flags = data.get(at + 1).copied().unwrap_or(0);
        let (a, b, c, d) = (
            u32_at(data, at + 2) % 4,
            u32_at(data, at + 6) % 4,
            u32_at(data, at + 10),
            u32_at(data, at + 14),
        );
        at += 18;
        // Small key spaces so names, layers and colours meet.
        let effect = match kind % 4 {
            0 => LookEffect::AttachCompositor(TextureCompositorNode {
                compositor: a,
                file: Did(0x3300_0000 | (b & 3)),
                mesh_type: c % 4,
                material_type: d % 4,
            }),
            1 => LookEffect::CompositorLayer(CompositorLayerNode {
                compositor: a,
                layer: b,
                image_number: c as i32,
                opacity: f32::from_bits(d),
            }),
            2 => LookEffect::CompositorColour(CompositorColourNode {
                compositor: a,
                colour: b,
                color: c.to_le_bytes(),
            }),
            _ => LookEffect::EditMaterial(MaterialEdit {
                material_property: 2000 + (c % 2000),
                mesh_type: a,
                material_type: b,
                priority: d as i32,
                color_value: d.to_le_bytes(),
                texture_value: Did(0),
                bool_value: false,
            }),
        };
        look.push(LookCandidate {
            effect,
            driven: false,
            picked: flags & 1 == 0,
            chance: 1.0,
            stopped: flags & 2 != 0,
            table: Did(0),
            scriptlet: Did(0),
            node_id: 0,
            scripts: vec![],
        });
    }
    let rest = data.get(at..).unwrap_or(&[]);
    let resolution = ClipResolution {
        look,
        ..ClipResolution::default()
    };
    // Each DID reads a slice of the rest chosen by its low bits.
    let read = |d: Did| -> Result<Option<Vec<u8>>, Infallible> {
        if rest.is_empty() {
            return Ok(None);
        }
        let start = (d.0 as usize).wrapping_mul(31) % rest.len();
        let mut v = rest[start..].to_vec();
        // Records start with their own DID.
        if v.len() >= 4 {
            v[..4].copy_from_slice(&d.0.to_le_bytes());
        }
        Ok(Some(v))
    };
    let limits = LookLimits {
        max_texels: 1 << 16,
        max_compositor_bytes: 1 << 20,
        max_figure_bytes: 3 << 19,
        ..LookLimits::default()
    };
    let options = AssemblyOptions {
        avatar_face_compositing: data.first().is_none_or(|b| b & 0x80 == 0),
        ..AssemblyOptions::default()
    };
    let cache = LookCache::default();
    let out = figure_look(&resolution, read, options, &limits, Some(&cache));
    let again = figure_look(&resolution, read, options, &limits, Some(&cache));
    assert_eq!(out.compositors.len(), again.compositors.len());
    for (a, b) in out.compositors.iter().zip(&again.compositors) {
        assert_eq!(a.image, b.image, "the cache changes no image");
    }
    if !options.avatar_face_compositing {
        assert!(out.compositors.is_empty());
    }
    for c in &out.compositors {
        if let Some(img) = &c.image {
            assert_eq!(
                img.pixels.len() as u64,
                u64::from(img.width) * u64::from(img.height) * 4
            );
        }
        let _ = out.compositor_for(c.mesh_type, c.material_type);
    }
    for e in &out.edits {
        let _ = out.colour_edits_for(e.mesh_type, e.material_type);
    }
});
