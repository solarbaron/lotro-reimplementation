// SPDX-License-Identifier: GPL-3.0-or-later
//! The `tessera-formats` building blocks (`docs/specs/records.md` §1-§2, §4, §6) on
//! arbitrary bytes: the `Reader` primitives driven by an op stream, `StringInfo::read`,
//! `PropertyValue::read` for every property type, and DID classification helpers.
//!
//! Nothing may panic, reads must never move the cursor past the end, and the VLE writer
//! and reader must round-trip.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::DidKind;
use tessera_formats::reader::{latin1_to_string, write_vle};
use tessera_formats::{
    AudioContainer, Did, LandblockId, PropertyType, PropertyValue, Reader, StringInfo,
    is_catalogued_prefix, name_hash, name_hash_utf16,
};

/// Highest property type code worth trying explicitly (codes above map to `Unknown`).
const MAX_PROPERTY_CODE: u32 = 24;

fn primitives(ops: &[u8], body: &[u8]) {
    let mut r = Reader::new(body);
    for &op in ops.iter().take(256) {
        let before = r.position();
        let ok = match op % 18 {
            0 => r.u8().is_ok(),
            1 => r.u16().is_ok(),
            2 => r.u32().is_ok(),
            3 => r.u64().is_ok(),
            4 => r.i32().is_ok(),
            5 => r.i64().is_ok(),
            6 => r.f32().is_ok(),
            7 => r.f64().is_ok(),
            8 => r.bool8().is_ok(),
            9 => r.did().is_ok(),
            10 => r.vle().is_ok(),
            11 => r.tsize().map(|t| t.bucket_count()).is_ok(),
            12 => r.pascal_bytes().is_ok(),
            13 => r.pascal_string().is_ok(),
            14 => r.utf16_string().map(|s| s.to_string_lossy()).is_ok(),
            15 => r.short_string().is_ok(),
            16 => r.skip(usize::from(op >> 4)).is_ok(),
            _ => r.ensure_elements(u32::from(op), 4).is_ok(),
        };
        assert!(r.position() <= body.len(), "cursor stays in bounds");
        assert_eq!(r.position() + r.remaining(), body.len());
        if !ok {
            assert!(r.position() >= before);
        }
    }
    let _ = r.rest();
    let _ = r.expect_end("fuzz");
}

fuzz_target!(|data: &[u8]| {
    // Op stream / body split at a position chosen by the first byte.
    if let Some((&split, rest)) = data.split_first() {
        let (ops, body) = rest.split_at(usize::from(split).min(rest.len()));
        primitives(ops, body);
    }

    // VLE round trip for any u32 taken from the input.
    if let Some(w) = data.get(..4) {
        let v = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
        let mut buf = Vec::new();
        write_vle(&mut buf, v);
        let mut r = Reader::new(&buf);
        assert_eq!(r.vle().ok(), Some(v), "VLE round trip");
        assert!(r.is_empty());

        let did = Did(v);
        let _ = (
            did.kind(),
            DidKind::of(did).evidence(),
            DidKind::of(did).name(),
        );
        let _ = is_catalogued_prefix(did.prefix());
        if let Some(lb) = LandblockId::from_did(did) {
            assert_eq!(lb.to_did(), did, "landblock round trip");
        }
    }

    let _ = StringInfo::read(&mut Reader::new(data));
    for code in 0..=MAX_PROPERTY_CODE {
        let ty = PropertyType::from_code(code);
        assert_eq!(ty.code(), code);
        let _ = PropertyValue::read(&mut Reader::new(data), ty);
    }

    let h = name_hash(data);
    assert!(h < 0x1000_0000);
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let _ = name_hash_utf16(&units);
    let _ = latin1_to_string(data);
    let _ = AudioContainer::sniff(data).extension();
});
