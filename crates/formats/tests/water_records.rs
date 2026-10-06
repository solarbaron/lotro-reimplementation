// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for water tables (prefix 0x16, `docs/specs/water.md` §4) and fog-zone
//! tables (prefix 0x17, §5). Every fixture is built here; no game data.

use tessera_formats::environment::Argb;
use tessera_formats::water::{
    FOG_ZONE_ENTRY_LEN, FogOverride, FogZone, FogZoneTable, WaterTable, WaterType, WetWater,
    decode_fog_zone_table, decode_water_table,
};
use tessera_formats::{Did, FormatError, Record};

fn wet(key: u32, surface: u32) -> WaterType {
    WaterType {
        key,
        wet: Some(WetWater {
            surface: Did(surface),
            extra: [Did(0); 4],
            volume_entity: Did(0x4700_0001),
            value: 24.0,
        }),
    }
}

fn dry(key: u32) -> WaterType {
    WaterType { key, wet: None }
}

/// Every proper prefix of `bytes` must fail without panicking.
fn every_prefix_fails<T>(bytes: &[u8], decode: impl Fn(&[u8]) -> Result<T, FormatError>) {
    for n in 0..bytes.len() {
        assert!(decode(&bytes[..n]).is_err(), "prefix of {n} bytes decoded");
    }
}

#[test]
fn water_table_worked_example() {
    // water.md §12: 61 bytes.
    let mut bytes = Vec::new();
    for v in [0x1600_0001u32, 0, 4, 7] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.push(1);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(1);
    bytes.push(2);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(0);
    for v in [0x3100_0001u32, 0, 0, 0, 0, 0x4700_0001] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&24.0f32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.push(1);
    assert_eq!(bytes.len(), 61);

    let t = WaterTable::parse_expecting(&bytes, 0x1600_0001).unwrap();
    assert_eq!(t.level_key, 4);
    assert_eq!(t.type_key, 7);
    assert_eq!(t.default, Some(dry(0)));
    assert_eq!(t.entries(), &[wet(1, 0x3100_0001), dry(2)]);
    assert_eq!(t.to_bytes(), bytes);
    assert_eq!(
        t,
        WaterTable::new(
            Did(0x1600_0001),
            0,
            4,
            7,
            Some(dry(0)),
            vec![wet(1, 0x3100_0001), dry(2)]
        )
    );
    // Look-ups: listed key, dry key, unknown key → default.
    assert!(!t.select(1).unwrap().is_dry());
    assert!(t.select(2).unwrap().is_dry());
    assert_eq!(t.select(99).unwrap().key, 0);
    assert!(t.get(99).is_none());
    every_prefix_fails(&bytes, decode_water_table);
    let mut long = bytes.clone();
    long.push(0);
    assert!(matches!(
        decode_water_table(&long),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
}

#[test]
fn water_table_first_entry_wins_and_no_default() {
    let t = WaterTable::new(
        Did(0x1600_0002),
        0,
        4,
        7,
        None,
        vec![
            wet(5, 0x3100_0005),
            wet(3, 0x3100_0003),
            wet(5, 0x3100_0009),
        ],
    );
    let back = decode_water_table(&t.to_bytes()).unwrap();
    assert_eq!(back, t);
    // §4.4: the first in stored order.
    assert_eq!(
        back.select(5).unwrap().wet.unwrap().surface,
        Did(0x3100_0005)
    );
    assert_eq!(back.select(3).unwrap().key, 3);
    assert!(back.select(0).is_none());
}

#[test]
fn water_table_hostile() {
    let t = WaterTable::new(
        Did(0x1600_0001),
        0,
        4,
        7,
        Some(dry(0)),
        vec![wet(1, 0x3100_0001)],
    );
    let bytes = t.to_bytes();
    // `has_default` ≥ 2.
    let mut b = bytes.clone();
    b[16] = 2;
    assert!(matches!(
        decode_water_table(&b),
        Err(FormatError::InvalidBool { offset: 16, .. })
    ));
    // `dry` ≥ 2 in the listed entry (offset 17 + 5 + 1 + 4).
    let mut b = bytes.clone();
    b[27] = 3;
    assert!(matches!(
        decode_water_table(&b),
        Err(FormatError::InvalidBool { offset: 27, .. })
    ));
    // Non-finite `value` (last four bytes).
    for v in [f32::NAN, f32::INFINITY] {
        let mut b = bytes.clone();
        let n = b.len();
        b[n - 4..].copy_from_slice(&v.to_le_bytes());
        assert!(matches!(
            decode_water_table(&b),
            Err(FormatError::Malformed { .. })
        ));
    }
    // A count larger than the bytes left (VLE 0x3FFF).
    let mut b = bytes[..22].to_vec();
    b.extend_from_slice(&[0xBF, 0xFF]);
    assert!(matches!(
        decode_water_table(&b),
        Err(FormatError::Truncated { .. })
    ));
    // Wrong prefix (0x17 is the fog-zone table).
    let mut b = bytes;
    b[3] = 0x17;
    assert!(matches!(
        decode_water_table(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
}

fn zone(key: u32, active: u8) -> FogZone {
    FogZone {
        key,
        overrides: [FogOverride {
            mode: if active != 0 { 4 } else { 3 },
            a: 0,
            b: 0,
        }; 7],
        near: if active != 0 { -50.0 } else { 500.0 },
        far: if active != 0 { 1600.0 } else { 10_000.0 },
        amount: if active != 0 { 0.9 } else { 0.0 },
        value_60: 1.0,
        active,
        colour: Argb(0xFFFF_FFFF),
        light_a: 1.0,
        colour_a1: Argb(0xFF80_8080),
        colour_a2: Argb(0xFF80_8080),
        light_b: 0.8,
        colour_b: Argb(0xFF40_4040),
        colour_c: Argb(0xFF20_2020),
        flag_c: false,
        colour_d: Argb(0xFF10_1010),
        e: None,
        flag_f: false,
        value_g: 0,
    }
}

#[test]
fn fog_zone_table_round_trip() {
    // A default-only table is 137 bytes (§5.1).
    let t = FogZoneTable::new(Did(0x1700_0001), 0, 3, Some(zone(0, 0)), Vec::new());
    let bytes = t.to_bytes();
    assert_eq!(bytes.len(), 137);
    let back = FogZoneTable::parse_expecting(&bytes, 0x1700_0001).unwrap();
    assert_eq!(back, t);
    assert!(!back.any_active());
    assert_eq!(back.select(5).unwrap().key, 0);
    every_prefix_fails(&bytes, decode_fog_zone_table);

    // Nine zones: 1,244 bytes (§5.1).
    let entries: Vec<FogZone> = (1..=9).map(|k| zone(k, 1)).collect();
    let t = FogZoneTable::new(Did(0x1700_0000), 0, 3, Some(zone(0, 0)), entries);
    let bytes = t.to_bytes();
    assert_eq!(bytes.len(), 1244);
    let back = decode_fog_zone_table(&bytes).unwrap();
    assert_eq!(back, t);
    assert!(back.any_active());
    assert!(back.select(4).unwrap().is_active());
    assert!(!back.select(0).unwrap().is_active());
    assert_eq!(back.entries().len(), 9);
}

#[test]
fn fog_zone_optional_value_e() {
    let mut z = zone(1, 7);
    z.e = Some((Did(0x4700_0009), 42));
    assert_eq!(z.encoded_len(), FOG_ZONE_ENTRY_LEN + 4);
    let t = FogZoneTable::new(Did(0x1700_0002), 0, 3, None, vec![z]);
    let bytes = t.to_bytes();
    assert_eq!(bytes.len(), 14 + FOG_ZONE_ENTRY_LEN + 4);
    let back = decode_fog_zone_table(&bytes).unwrap();
    assert_eq!(back.entries()[0].e, Some((Did(0x4700_0009), 42)));
    // `active` is a raw byte: any non-zero value is active.
    assert!(back.entries()[0].is_active());
    assert!(back.default.is_none());
    assert!(back.select(2).is_none());
    every_prefix_fails(&bytes, decode_fog_zone_table);
}

#[test]
fn fog_zone_table_hostile() {
    let t = FogZoneTable::new(Did(0x1700_0001), 0, 3, Some(zone(0, 0)), Vec::new());
    let bytes = t.to_bytes();
    // Entry starts at 13; near at 13 + 4 + 63 = 80, far 84, amount 88, value_60 92, then
    // `active` (1) and `colour` (4), light_a 101, colour_a1 and colour_a2, light_b 113.
    // water.md §11: every one of the six floats must be finite.
    let z = zone(0, 0);
    for (at, stored) in [
        (80usize, z.near),
        (84, z.far),
        (88, z.amount),
        (92, z.value_60),
        (101, z.light_a),
        (113, z.light_b),
    ] {
        assert_eq!(&bytes[at..at + 4], &stored.to_le_bytes(), "float at {at}");
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut b = bytes.clone();
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
            assert!(
                matches!(
                    decode_fog_zone_table(&b),
                    Err(FormatError::Malformed { offset, .. }) if offset == at
                ),
                "float at {at} = {v}"
            );
        }
    }
    // `flag_c` (13 + 4 + 63 + 16 + 1 + 4 + 4 + 4 + 4 + 4 + 4 + 4 = 125) ≥ 2.
    let mut b = bytes.clone();
    b[125] = 2;
    assert!(matches!(
        decode_fog_zone_table(&b),
        Err(FormatError::InvalidBool { offset: 125, .. })
    ));
    // `has_default` ≥ 2.
    let mut b = bytes.clone();
    b[12] = 9;
    assert!(matches!(
        decode_fog_zone_table(&b),
        Err(FormatError::InvalidBool { offset: 12, .. })
    ));
    // Count larger than the bytes left.
    let mut b = bytes.clone();
    let n = b.len();
    b[n - 1] = 1;
    assert!(matches!(
        decode_fog_zone_table(&b),
        Err(FormatError::Truncated { .. })
    ));
    // Wrong prefix.
    let mut b = bytes;
    b[3] = 0x16;
    assert!(matches!(
        decode_fog_zone_table(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn fog_zone_e_with_zero_did_encodes_without_value_e() {
    // water.md §5.2 field 18: `value_e` is stored only after a non-zero `did_e`. A `Some`
    // with a zero DID cannot be stored; it is written as `did_e` 0 and decodes as `None`.
    let mut z = zone(1, 1);
    z.e = Some((Did(0), 9));
    assert_eq!(z.encoded_len(), FOG_ZONE_ENTRY_LEN);
    let t = FogZoneTable::new(Did(0x1700_0002), 0, 3, None, vec![z]);
    let bytes = t.to_bytes();
    assert_eq!(bytes.len(), 14 + FOG_ZONE_ENTRY_LEN);
    let back = decode_fog_zone_table(&bytes).unwrap();
    assert_eq!(back.entries()[0].e, None);
    z.e = None;
    assert_eq!(back.entries(), &[z]);
}

#[test]
fn encodable_reports_non_finite_floats() {
    let mut w = wet(1, 0x3100_0001);
    assert!(w.is_encodable());
    assert!(dry(2).is_encodable());
    w.wet.as_mut().unwrap().value = f32::NAN;
    assert!(!w.is_encodable());
    let mut z = zone(1, 1);
    assert!(z.is_encodable());
    z.light_b = f32::INFINITY;
    assert!(!z.is_encodable());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "non-finite")]
fn water_table_with_non_finite_value_asserts_in_debug() {
    let mut w = wet(1, 0x3100_0001);
    w.wet.as_mut().unwrap().value = f32::NAN;
    let _ = WaterTable::new(Did(0x1600_0001), 0, 4, 7, None, vec![w]).to_bytes();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "non-finite")]
fn fog_zone_table_with_non_finite_float_asserts_in_debug() {
    let mut z = zone(1, 1);
    z.value_60 = f32::NAN;
    let _ = FogZoneTable::new(Did(0x1700_0001), 0, 3, Some(z), Vec::new()).to_bytes();
}
