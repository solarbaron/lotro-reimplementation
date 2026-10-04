// SPDX-License-Identifier: GPL-3.0-or-later
//! Primitive encodings and DID classification (spec §1, §2).

mod common;

use common::B;
use tessera_formats::did::name_hash_utf16;
use tessera_formats::reader::{latin1_to_string, tsize_bucket_count, write_vle};
use tessera_formats::{
    Did, DidKind, Evidence, FormatError, LandblockId, LandblockKind, Reader, TSize, name_hash,
};

fn vle(bytes: &[u8]) -> Result<(u32, usize), FormatError> {
    let mut r = Reader::new(bytes);
    let v = r.vle()?;
    Ok((v, r.position()))
}

#[test]
fn vle_one_byte_form() {
    assert_eq!(vle(&[0x00]).unwrap(), (0, 1));
    assert_eq!(vle(&[0x7F]).unwrap(), (0x7F, 1));
}

#[test]
fn vle_two_byte_form() {
    assert_eq!(vle(&[0x80, 0x80]).unwrap(), (0x80, 2));
    assert_eq!(vle(&[0xBF, 0xFF]).unwrap(), (0x3FFF, 2));
    assert_eq!(vle(&[0x81, 0x23]).unwrap(), (0x123, 2));
}

#[test]
fn vle_four_byte_form_low_half_is_little_endian() {
    // Spec §1.2 example: C0 00 9B 6B = 27,547.
    assert_eq!(vle(&[0xC0, 0x00, 0x9B, 0x6B]).unwrap(), (27_547, 4));
    assert_eq!(vle(&[0xC0, 0x00, 0x00, 0x40]).unwrap(), (0x4000, 4));
    assert_eq!(vle(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap(), (0x3FFF_FFFF, 4));
    assert_eq!(vle(&[0xE1, 0x02, 0x04, 0x03]).unwrap(), (0x2102_0304, 4));
    assert_eq!(vle(&[0xDF, 0x12, 0x34, 0x56]).unwrap(), (0x1F12_5634, 4));
}

#[test]
fn vle_escape_form() {
    assert_eq!(
        vle(&[0xE0, 0x78, 0x56, 0x34, 0x12]).unwrap(),
        (0x1234_5678, 5)
    );
    assert_eq!(vle(&[0xE0, 0xFF, 0xFF, 0xFF, 0xFF]).unwrap(), (u32::MAX, 5));
}

#[test]
fn vle_truncation_at_every_byte_is_an_error_and_rewinds() {
    let forms: [&[u8]; 4] = [
        &[0x81, 0x23],
        &[0xC0, 0x00, 0x9B, 0x6B],
        &[0xE1, 0x02, 0x04, 0x03],
        &[0xE0, 0x78, 0x56, 0x34, 0x12],
    ];
    for form in forms {
        for cut in 0..form.len() {
            let mut r = Reader::new(&form[..cut]);
            let err = r.vle().unwrap_err();
            assert!(
                matches!(err, FormatError::Truncated { .. }),
                "{form:02X?} cut at {cut}: {err}"
            );
            assert_eq!(r.position(), 0, "reader must not advance on failure");
        }
    }
}

#[test]
fn vle_writer_uses_shortest_form_and_round_trips() {
    let cases: [(u32, usize); 12] = [
        (0, 1),
        (0x7F, 1),
        (0x80, 2),
        (0x3FFF, 2),
        (0x4000, 4),
        (0x1FFF_FFFF, 4),
        (0x2000_0000, 5), // first byte would collide with the 0xE0 escape
        (0x20FF_FFFF, 5),
        (0x2100_0000, 4),
        (0x3FFF_FFFF, 4),
        (0x4000_0000, 5),
        (u32::MAX, 5),
    ];
    for (value, len) in cases {
        let mut buf = Vec::new();
        write_vle(&mut buf, value);
        assert_eq!(buf.len(), len, "length of {value:#x}");
        assert_eq!(vle(&buf).unwrap(), (value, len), "round trip of {value:#x}");
    }
}

#[test]
fn tsize_reads_hint_and_count() {
    let data = B::new().tsize(0, 5).build();
    let mut r = Reader::new(&data);
    assert_eq!(r.tsize().unwrap(), TSize { hint: 0, count: 5 });
    assert_eq!(TSize { hint: 0, count: 5 }.bucket_count(), Some(11));

    // Unusual hint is accepted (T1), just without a known bucket count.
    let data = B::new().tsize(200, 0x4000).build();
    let mut r = Reader::new(&data);
    let t = r.tsize().unwrap();
    assert_eq!((t.hint, t.count), (200, 0x4000));
    assert_eq!(t.bucket_count(), None);
    assert!(r.is_empty());

    assert_eq!(tsize_bucket_count(10), Some(12_281));
    assert_eq!(tsize_bucket_count(12), Some(49_139));
    assert_eq!(tsize_bucket_count(13), None);
}

#[test]
fn fixed_width_and_bool() {
    let data = B::new()
        .u8(1)
        .u16(0x1234)
        .u32(0xDEAD_BEEF)
        .u64(0x0102_0304_0506_0708)
        .f32(1.5)
        .f64(-2.25)
        .u8(0)
        .u8(2)
        .build();
    let mut r = Reader::new(&data);
    assert!(r.bool8().unwrap());
    assert_eq!(r.u16().unwrap(), 0x1234);
    assert_eq!(r.u32().unwrap(), 0xDEAD_BEEF);
    assert_eq!(r.u64().unwrap(), 0x0102_0304_0506_0708);
    assert_eq!(r.f32().unwrap(), 1.5);
    assert_eq!(r.f64().unwrap(), -2.25);
    assert!(!r.bool8().unwrap());
    assert_eq!(
        r.bool8().unwrap_err(),
        FormatError::InvalidBool {
            offset: 28,
            value: 2
        }
    );
    assert!(matches!(r.u32(), Err(FormatError::Truncated { .. })));
}

#[test]
fn strings() {
    let data = B::new()
        .bytes(&[3, b'C', b'a', 0xE9]) // Pascal, Latin-1 é
        .utf16("Hi")
        .utf16_units(&[0x41, 0xD800, 0x42]) // unpaired surrogate
        .short("STRINGTABLE")
        .build();
    let mut r = Reader::new(&data);
    assert_eq!(r.pascal_string().unwrap(), "Caé");
    assert_eq!(r.utf16_string().unwrap().to_string_lossy(), "Hi");
    let s = r.utf16_string().unwrap();
    assert_eq!(s.units(), &[0x41, 0xD800, 0x42]);
    assert_eq!(s.to_string_lossy(), "A\u{FFFD}B");
    assert_eq!(r.short_string().unwrap(), "STRINGTABLE");
    r.expect_end("test").unwrap();
    assert_eq!(latin1_to_string(&[0xFF, 0x41]), "ÿA");
}

#[test]
fn string_lengths_beyond_input_are_truncation() {
    // UTF-16 count of 0x3FFF units with nothing after it.
    let data = [0xBF, 0xFF];
    let mut r = Reader::new(&data);
    assert!(matches!(
        r.utf16_string(),
        Err(FormatError::Truncated { .. })
    ));
    assert_eq!(r.position(), 0);
    let data = [0xE0, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut r = Reader::new(&data);
    assert!(matches!(
        r.pascal_bytes(),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn ensure_elements_rejects_impossible_counts() {
    let data = [0u8; 16];
    let r = Reader::new(&data);
    assert_eq!(r.ensure_elements(4, 4).unwrap(), 4);
    assert!(r.ensure_elements(5, 4).is_err());
    assert!(r.ensure_elements(u32::MAX, 1).is_err());
}

#[test]
fn name_hash_matches_definition() {
    assert_eq!(name_hash(b""), 0);
    assert_eq!(name_hash(b"a"), 0x61);
    assert_eq!(name_hash(b"ab"), 0x672);
    // Long names exercise the high-nibble fold; the result is always 28 bits.
    let h = name_hash(b"a_very_long_argument_name_for_testing");
    assert!(h < 0x1000_0000);
    let units: Vec<u16> = "ab".encode_utf16().collect();
    assert_eq!(name_hash_utf16(&units), Some(0x672));
    assert_eq!(name_hash_utf16(&[0x41, 0x100]), None);
}

#[test]
fn did_kinds() {
    assert_eq!(Did(0x2500_0001).kind(), DidKind::StringTable);
    assert_eq!(Did(0x2500_0001).kind().evidence(), Evidence::Decoded);
    assert_eq!(Did(0x4100_1234).kind(), DidKind::RenderSurface);
    assert_eq!(Did(0x3400_0000).kind(), DidKind::PropertyDictionary);
    assert_eq!(Did(0x2300_0001).kind(), DidKind::EnumMapper);
    assert_eq!(Did(0x2800_0000).kind(), DidKind::DataIdMap);
    assert_eq!(Did(0x0A00_0FE4).kind(), DidKind::Sound);
    assert_eq!(Did(0x0200_0000).kind(), DidKind::Unknown(0x02));
    assert_eq!(Did(0x0200_0000).kind().evidence(), Evidence::Unknown);
    assert_eq!(Did(0xFFFF_0001).kind(), DidKind::ArchiveMetadata);
    assert_eq!(
        Did(0x8021_0A0B).kind(),
        DidKind::Landblock(LandblockKind::Info)
    );
    assert_eq!(Did(0x4100_0000).prefix(), 0x41);
    assert_eq!(Did(0x4112_3456).serial(), 0x12_3456);
    assert_eq!(format!("{}", Did(0x41AB_CDEF)), "0x41ABCDEF");
}

#[test]
fn landblock_ids_round_trip() {
    let lb = LandblockId::from_did(Did(0x800E_FE07)).unwrap();
    assert_eq!(
        lb,
        LandblockId {
            kind: LandblockKind::Heightmap,
            region: 14,
            x: 0xFE,
            y: 0x07
        }
    );
    assert_eq!(lb.to_did(), Did(0x800E_FE07));
    let other = LandblockId::from_did(Did(0x8051_0102)).unwrap();
    assert_eq!(other.kind, LandblockKind::Other(5));
    assert_eq!(other.to_did(), Did(0x8051_0102));
    assert!(LandblockId::from_did(Did(0x4100_0000)).is_none());
}
