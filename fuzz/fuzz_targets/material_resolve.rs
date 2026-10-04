// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-material::MaterialLibrary` and `tessera-material::dye`
//! (`docs/specs/material.md` §9–§12, `texture-list.md` §5.2): the whole chain surface
//! record → shader record + materials → texture list → surface over an in-memory source
//! built from the input, then texture decoding (capped levels, the resolved chain with its
//! upload plan, cube faces), the dye preview, the dye blend on arbitrary images and dye
//! block selection. Nothing may panic; decoded textures stay
//! within the configured size limit.
//!
//! Input layout: a control byte, then up to six chunks, each a u16 little-endian length
//! and that many bytes: shader record 0x2B000001, materials 0x30000001 and 0x30000002,
//! texture list 0x40000001, surfaces 0x41000001 and 0x41000002. Each chunk's DID is
//! forced. The surface record 0x31000001 is built from the control byte.

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::{Did, RgbaImage};
use tessera_material::{
    DyeColors, DyeLayer, FnSource, LibraryConfig, MaterialLibrary, apply_dye, select_dye_block,
};

const DIDS: [u32; 6] = [
    0x2B00_0001,
    0x3000_0001,
    0x3000_0002,
    0x4000_0001,
    0x4100_0001,
    0x4100_0002,
];
const LIMIT: u32 = 64;

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, mut rest)) = data.split_first() else {
        return;
    };
    let mut store: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for &did in &DIDS {
        let Some(len) = rest.get(..2) else { break };
        let len = usize::from(u16::from_le_bytes([len[0], len[1]]));
        let body = rest.get(2..).unwrap_or(&[]);
        let chunk = &body[..len.min(body.len())];
        rest = &body[chunk.len()..];
        let mut payload = did.to_le_bytes().to_vec();
        payload.extend_from_slice(chunk.get(4..).unwrap_or(&[]));
        store.insert(did, payload);
    }
    let n = usize::from(ctl % 3);
    let mut surface = Vec::new();
    surface.extend_from_slice(&0x3100_0001u32.to_le_bytes());
    surface.extend_from_slice(&0x2B00_0001u32.to_le_bytes());
    surface.extend_from_slice(&u32::from(ctl >> 2).to_le_bytes());
    surface.extend_from_slice(&(n as u32).to_le_bytes());
    for m in &DIDS[1..1 + n] {
        surface.extend_from_slice(&m.to_le_bytes());
    }
    surface.extend_from_slice(&1u16.to_le_bytes());
    store.insert(0x3100_0001, surface);

    let lib = MaterialLibrary::with_config(
        FnSource(move |d: Did| Ok::<_, std::io::Error>(store.get(&d.0).cloned())),
        LibraryConfig {
            max_texture_dimension: LIMIT,
            ..LibraryConfig::default()
        },
    );
    let check = |img: &RgbaImage| {
        assert!(img.width <= LIMIT && img.height <= LIMIT);
        assert_eq!(
            img.pixels.len() as u64,
            u64::from(img.width) * u64::from(img.height) * 4
        );
    };
    if let Ok(t) = lib.texture(Did(0x4000_0001), Some(u32::from(ctl))) {
        check(&t.image);
    }
    // Uncapped and above the limit: the limit still caps the level choice.
    if let Ok(t) = lib.texture(Did(0x4000_0001), Some(u32::MAX)) {
        check(&t.image);
    }
    if let Ok(chain) = lib.texture_chain(Did(0x4000_0001), None) {
        assert!(!chain.levels.is_empty());
        for level in &chain.levels {
            assert!(level.level.width <= LIMIT && level.level.height <= LIMIT);
            assert!(level.level.width > 0 && level.level.height > 0);
            let _ = lib.level_payload(level.did);
        }
        if let Some(plan) = chain.plan {
            assert!(plan.first + plan.uploadable <= plan.stored);
        }
    }
    if let Ok(Some(faces)) = lib.cube_faces(Did(0x4000_0001)) {
        for f in &faces {
            check(&f.image);
        }
    }
    if let Ok(r) = lib.resolve_surface(Did(0x3100_0001)) {
        for slot in &r.slots {
            if let Ok(Some(t)) = lib.slot_texture(&r, slot.id, None) {
                check(&t.image);
            }
        }
        if let Ok(Some(img)) = lib.dye_preview_with(&r, DyeColors::default(), Some(16)) {
            check(&img);
        }
    }

    // The dye blend on images built from the input (consistent or not).
    let w = u32::from(ctl % 5);
    let h = u32::from(ctl / 51);
    let base = RgbaImage {
        width: w,
        height: h,
        pixels: rest.iter().copied().take((w * h * 4) as usize).collect(),
    };
    let mask = RgbaImage {
        width: u32::from(ctl % 3) + 1,
        height: 1,
        pixels: rest
            .iter()
            .rev()
            .copied()
            .take(4 * (usize::from(ctl % 3) + 1))
            .collect(),
    };
    let color = [f32::from(ctl) / 100.0, -1.0, f32::NAN, 1.0];
    if let Ok(img) = apply_dye(&base, &[DyeLayer { mask: &mask, color }], ctl & 1 == 1) {
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.pixels.len(), base.pixels.len());
    }

    let codes: Vec<f32> = rest
        .chunks_exact(4)
        .take(16)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if let Some(i) = select_dye_block(&codes, Some(f32::from(ctl) / 100.0)) {
        assert!(i < codes.len());
    }
});
