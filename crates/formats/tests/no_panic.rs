// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests: decoders never panic on arbitrary input, and primitives round-trip.

use proptest::prelude::*;
use tessera_formats::bcn::{BcFormat, decode_bcn};
use tessera_formats::reader::write_vle;
use tessera_formats::{
    DataIdMap, EnumMapper, PropertyDictionary, PropertyType, PropertyValue, Reader, Record,
    SoundRecord, StringInfo, StringTable, Surface, WaveInfo, name_hash,
};

/// Random tail behind a DID with the given prefix, so decoding gets past the prologue.
fn with_prefix(prefix: u8) -> impl Strategy<Value = Vec<u8>> {
    (
        any::<[u8; 3]>(),
        proptest::collection::vec(any::<u8>(), 0..512),
    )
        .prop_map(move |(serial, tail)| {
            let mut v = vec![serial[0], serial[1], serial[2], prefix];
            v.extend(tail);
            v
        })
}

/// Random bytes biased toward small values so counts and lengths are often plausible.
fn small_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(prop_oneof![0u8..4, any::<u8>()], 0..512)
}

fn all_decoders(data: &[u8]) {
    let _ = StringTable::parse(data);
    let _ = StringInfo::parse(data);
    let _ = PropertyDictionary::parse(data);
    let _ = EnumMapper::parse(data);
    let _ = DataIdMap::parse(data);
    let _ = WaveInfo::parse(data);
    if let Ok(s) = SoundRecord::parse(data) {
        let _ = s.container();
        let _ = WaveInfo::parse(s.audio);
    }
    if let Ok(s) = Surface::parse(data) {
        let _ = s.to_rgba8();
    }
}

/// Classic ELF hash (clears the high nibble inside the loop), used as an oracle.
fn elf_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &c in name {
        h = (h << 4).wrapping_add(u32::from(c));
        let g = h & 0xF000_0000;
        if g != 0 {
            h ^= g >> 24;
        }
        h &= !g;
    }
    h
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        all_decoders(&data);
    }

    #[test]
    fn small_valued_bytes_never_panic(data in small_bytes()) {
        all_decoders(&data);
    }

    #[test]
    fn string_table_tail_never_panics(data in with_prefix(0x25)) {
        let _ = StringTable::parse(&data);
    }

    #[test]
    fn enum_mapper_tail_never_panics(data in with_prefix(0x23)) {
        let _ = EnumMapper::parse(&data);
    }

    #[test]
    fn did_map_tail_never_panics(data in with_prefix(0x28)) {
        let _ = DataIdMap::parse(&data);
    }

    #[test]
    fn property_dictionary_tail_never_panics(data in with_prefix(0x34)) {
        let _ = PropertyDictionary::parse(&data);
    }

    #[test]
    fn sound_tail_never_panics(data in with_prefix(0x0A)) {
        if let Ok(s) = SoundRecord::parse(&data) {
            let _ = WaveInfo::parse(s.audio);
        }
    }

    #[test]
    fn surface_headers_never_panic(
        w in 0u32..70, h in 0u32..70,
        code in prop_oneof![
            Just(20u32), Just(21), Just(28), Just(500),
            Just(0x3154_5844), Just(0x3354_5844), Just(0x3554_5844), any::<u32>()
        ],
        body in proptest::collection::vec(any::<u8>(), 0..4096),
        fix_len in any::<bool>(),
    ) {
        let len = if fix_len {
            tessera_formats::SurfaceFormat::from_code(code)
                .and_then(|f| f.expected_len(w, h))
                .unwrap_or(body.len() as u64) as usize
        } else {
            body.len()
        };
        let mut data = Vec::new();
        for v in [0x4100_0000u32, 0, w, h, code, len as u32] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        data.extend(body.iter().copied().cycle().take(len.min(1 << 16)));
        if let Ok(s) = Surface::parse(&data)
            && let Ok(img) = s.to_rgba8()
            && !s.is_jpeg()
        {
            prop_assert_eq!(img.pixels.len() as u64, u64::from(w) * u64::from(h) * 4);
        }
    }

    #[test]
    fn bcn_never_panics(
        w in 0u32..40, h in 0u32..40,
        data in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        for f in [BcFormat::Bc1, BcFormat::Bc2, BcFormat::Bc3] {
            if let Some(img) = decode_bcn(f, w, h, &data) {
                prop_assert_eq!(img.len(), (w * h * 4) as usize);
            }
        }
    }

    #[test]
    fn property_values_never_panic(
        code in 0u32..25,
        data in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let mut r = Reader::new(&data);
        let _ = PropertyValue::read(&mut r, PropertyType::from_code(code));
    }

    #[test]
    fn reader_primitives_never_panic(data in proptest::collection::vec(any::<u8>(), 0..64)) {
        let mut r = Reader::new(&data);
        while !r.is_empty() {
            let before = r.position();
            let ok = match before % 6 {
                0 => r.vle().is_ok(),
                1 => r.tsize().is_ok(),
                2 => r.pascal_string().is_ok(),
                3 => r.utf16_string().is_ok(),
                4 => r.short_string().is_ok(),
                _ => r.bool8().is_ok(),
            };
            if !ok {
                prop_assert!(r.position() <= data.len());
                break;
            }
            prop_assert!(r.position() > before);
        }
    }

    #[test]
    fn vle_round_trips(v in any::<u32>()) {
        let mut buf = Vec::new();
        write_vle(&mut buf, v);
        let mut r = Reader::new(&buf);
        prop_assert_eq!(r.vle().unwrap(), v);
        prop_assert!(r.is_empty());
    }

    #[test]
    fn name_hash_matches_elf_oracle(name in proptest::collection::vec(any::<u8>(), 0..64)) {
        prop_assert_eq!(name_hash(&name), elf_hash(&name) & 0x0FFF_FFFF);
        prop_assert!(name_hash(&name) < 0x1000_0000);
    }
}
