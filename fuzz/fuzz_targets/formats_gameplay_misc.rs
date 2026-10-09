// SPDX-License-Identifier: GPL-3.0-or-later
//! Miscellaneous record decoders (`tessera_formats::gameplay::misc`,
//! `docs/specs/misc-records.md`) on arbitrary bytes: image compositors and their
//! composition (§3), UI scenes (§4), compute shaders and their DXBC container (§6), script
//! channels (§7), the name-filter and taboo tables (§9, §10) and the compression-dictionary
//! envelope (§11), and the percentage display of modifier values (§13.1). Imposter tables
//! (§5) have their own target, `formats_imposter_table`.
//!
//! Input: byte 0 picks the decoder, the rest is the payload. UI-scene descriptors use a
//! total synthetic schema. A decoded compositor is composed with synthetic images whose
//! sizes come from their DIDs and a selection taken from the payload's tail.
//!
//! Nothing may panic; writers must reproduce what the readers decoded.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_misc -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::misc::*;
use tessera_formats::{Did, PropertyType, PropertyValue, Record, RgbaImage};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn synthetic_image(did: Did) -> RgbaImage {
    let w = did.0 & 7;
    let h = (did.0 >> 3) & 7;
    RgbaImage {
        width: w,
        height: h,
        pixels: did.0.to_le_bytes().repeat((w * h) as usize),
    }
}

fn compose(c: &ImageCompositor, tail: &[u8]) {
    let mut words = tail
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .cycle();
    let selection: Vec<LayerSelection> = if tail.len() < 8 {
        c.preview_selection()
    } else {
        c.layers
            .iter()
            .map(|_| LayerSelection {
                option: words.next().unwrap_or(1) % 4,
                opacity: f32::from_bits(words.next().unwrap_or(0)),
            })
            .collect()
    };
    let images: HashMap<Did, RgbaImage> = c
        .required_images(&selection)
        .into_iter()
        // Leave some images unloaded.
        .filter(|d| d.0 % 5 != 0)
        .map(|d| (d, synthetic_image(d)))
        .collect();
    let colour = |k: u32| Some([f32::from_bits(k), 0.5, -0.5, 1.0]);
    if let Ok(Some(out)) = c.compose(&selection, colour, |d| images.get(&d)) {
        assert_eq!(out.pixels.len(), (out.width * out.height * 4) as usize);
    }
    let mut inst = CompositorInstance::new(c);
    for l in &c.layers {
        inst.set_layer(c, l.layer_key, 1, 0.75);
    }
    let _ = inst.compose(c, |d| images.get(&d));
}

fuzz_target!(|input: &[u8]| {
    let [selector, data @ ..] = input else {
        return;
    };
    match selector % 8 {
        0 => {
            if let Ok(c) = ImageCompositor::parse(data) {
                assert_eq!(c.to_bytes(), data);
                assert_eq!(c.stored_len(), data.len());
                let _ = c.first_unusual_field();
                assert!(c.dependencies().iter().all(|d| !d.is_null()));
                compose(&c, data.get(data.len().saturating_sub(64)..).unwrap_or(&[]));
            }
        }
        1 => {
            if let Ok(s) = UiScene::parse(data, &total) {
                let _ = s.first_camera();
                assert!(s.objects().count() <= s.entities.len());
            }
        }
        2 => {
            if let Ok(cs) = ComputeShader::parse(data) {
                assert_eq!(cs.blob.len() + 8, data.len());
                let _ = cs.is_consistent();
            }
            let _ = DxbcContainer::parse(data);
        }
        3 => {
            if let Ok(sc) = ScriptChannels::parse(data) {
                let bytes = sc.to_bytes();
                assert_eq!(ScriptChannels::parse(&bytes).expect("re-parse"), sc);
                let _ = sc.script_keys();
            }
        }
        4 => {
            let _ = NameFilterTable::parse(data);
        }
        5 => {
            if let Ok(t) = TabooTable::parse(data) {
                let _ = (t.keys_match_headers(), t.pattern_count());
            }
        }
        6 => {
            if let Ok(e) = CompressionDictionaryEnvelope::parse(data) {
                assert_eq!(
                    data.len(),
                    COMPRESSION_DICTIONARY_HEADER_LEN + e.body_len as usize
                );
            }
        }
        _ => {
            // Pixel arithmetic and placement on arbitrary values.
            let w = |i: usize| {
                data.get(i..i + 4)
                    .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            };
            let p = placement_window(
                w(0),
                w(4),
                w(8) % 4096,
                w(12) % 4096,
                w(16) % 4096,
                w(20) % 4096,
            );
            assert!(p.columns <= (w(8) % 4096).max(1) && p.rows <= (w(12) % 4096).max(1));
            let px = w(24).to_le_bytes();
            let _ = mix(px, w(28).to_le_bytes(), px[0]);
            let _ = masked_multiply(
                px,
                colour_bytes([
                    f32::from_bits(w(32)),
                    f32::from_bits(w(36)),
                    f32::from_bits(w(40)),
                ]),
                (px[1] & 1 == 0).then_some(px[2]),
            );
            let _ = opacity_byte(f32::from_bits(w(44)));
            // §13.1 (X4): the percentage display of a modifier value.
            let (multiply, flag) = (px[3] & 1 == 0, px[3] & 2 == 0);
            let v = f32::from_bits(w(52));
            let d = modifier_value_display(&PropertyValue::Float(v), multiply, flag)
                .expect("a float is shown");
            assert_eq!(d.percent_template, flag);
            assert_eq!(d.direction.is_some(), flag && multiply);
            if !flag {
                assert_eq!(d.number.to_bits(), f64::from(v).to_bits());
            }
            #[allow(clippy::cast_possible_wrap)]
            let i = modifier_value_display(&PropertyValue::Int(w(56) as i32), multiply, flag)
                .expect("an int is shown");
            assert!(i.direction.is_none());
            #[allow(clippy::cast_possible_wrap)]
            let expected = f64::from(w(56) as i32);
            assert_eq!(i.number.to_bits(), expected.to_bits());
        }
    }
});
