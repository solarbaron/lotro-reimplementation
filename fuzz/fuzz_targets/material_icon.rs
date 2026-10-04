// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-material::icon` (`docs/specs/icon-layers.md`): decoding a surface record to a
//! layer under a size limit, and composing up to four decoded or synthetic layers.
//! Nothing may panic; a composed icon has the canvas size and a straight-alpha buffer of
//! the right length.
//!
//! Input layout: one byte of options (limit and layer roles), then the surface payload.
//! The payload's DID is forced to a surface DID so the decoder gets past the prefix check.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{Did, RgbaImage};
use tessera_material::icon::{LayerRole, compose_images_with_limit, decode_layer_with_limit};

const ROLES: [LayerRole; 5] = [
    LayerRole::Background,
    LayerRole::Underlay,
    LayerRole::Shadow,
    LayerRole::Image,
    LayerRole::Overlay,
];

fn synthetic(w: u32, h: u32, seed: u8) -> RgbaImage {
    let pixels = (0..w * h * 4)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect();
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&opts, rest)) = data.split_first() else {
        return;
    };
    let limit = 1 + u32::from(opts & 0x3F) * 8;
    let did = Did(0x4100_0000 | u32::from(opts));
    let mut payload = did.0.to_le_bytes().to_vec();
    payload.extend_from_slice(rest.get(4..).unwrap_or(&[]));

    let decoded = decode_layer_with_limit(did, &payload, limit).ok();
    if let Some(img) = &decoded {
        assert!(img.width <= limit && img.height <= limit);
        assert_eq!(
            img.pixels.len() as u64,
            u64::from(img.width) * u64::from(img.height) * 4
        );
    }

    let a = synthetic(u32::from(opts % 7), u32::from(opts % 5) + 1, opts);
    let b = synthetic(u32::from(opts % 3) + 2, u32::from(opts % 9), opts ^ 0x5A);
    let mut layers: Vec<(LayerRole, &RgbaImage)> = vec![
        (ROLES[usize::from(opts) % ROLES.len()], &a),
        (ROLES[usize::from(opts >> 3) % ROLES.len()], &b),
    ];
    if let Some(img) = &decoded {
        layers.push((ROLES[usize::from(opts >> 5) % ROLES.len()], img));
    }
    if let Ok(Some(icon)) = compose_images_with_limit(&layers, limit.max(16)) {
        assert_eq!(
            icon.pixels.len() as u64,
            u64::from(icon.width) * u64::from(icon.height) * 4
        );
    }
});
