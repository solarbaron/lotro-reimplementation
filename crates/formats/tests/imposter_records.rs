// SPDX-License-Identifier: GPL-3.0-or-later
//! Imposter tables (`docs/specs/misc-records.md` §5) from synthetic bytes: layout, cell
//! coordinates (I7), the mirrored *u* pair, re-encoding and hostile input.

use tessera_formats::imposter::IMPOSTER_ENTRY_SIZE;
use tessera_formats::{Did, FormatError, ImposterEntry, ImposterTable, Record, TSize};

fn table(n: u32) -> ImposterTable {
    let entries = (0..n)
        .map(|i| ImposterEntry {
            name_key: 100 + i * 23,
            column: u8::try_from(i % 16).unwrap_or(0),
            row: u8::try_from(i / 16).unwrap_or(0),
            value_a: 2.0 + i as f32,
            value_b: -0.25,
        })
        .collect();
    ImposterTable {
        did: Did(0x2701_1B36),
        size: TSize { hint: 1, count: n },
        entries,
        colour_atlas: Did(0x4000_1234),
        second_atlas: Did(0x4000_1235),
        cell_u: 0.060_546_875,
        cell_v: 0.484_375,
    }
}

#[test]
fn layout_round_trip() {
    let t = table(19);
    let bytes = t.to_bytes();
    // DID, hint, VLE count (1 byte), 19 entries, two DIDs, two floats.
    assert_eq!(bytes.len(), 4 + 1 + 1 + 19 * IMPOSTER_ENTRY_SIZE + 16);
    assert_eq!(&bytes[..4], &[0x36, 0x1B, 0x01, 0x27]);
    let back = ImposterTable::parse_expecting(&bytes, 0x2701_1B36u32).expect("decodes");
    assert_eq!(back, t);
    assert_eq!(back.to_bytes(), bytes);
    assert_eq!(back.get(100 + 23).map(|e| e.column), Some(1));
    assert!(back.get(7).is_none());
}

#[test]
fn cell_coordinates_follow_i7() {
    // n = 19: C = 16, R = 2 (§5.4 example).
    let t = table(19);
    let e = t.entries[17];
    assert_eq!((e.column, e.row), (1, 1));
    let c = t.cell(&e).expect("finite");
    assert!((c.u0 - 1.0 / 16.0).abs() < 1e-6);
    assert!((c.v0 - 0.5).abs() < 1e-6);
    assert!((c.u1 - (1.0 / 16.0 + 0.060_546_875)).abs() < 1e-6);
    assert!((c.v1 - (0.5 + 0.484_375)).abs() < 1e-6);
    // Mirrored: left gets u1 − 1, right u0 − 1.
    let (l, r) = c.u_pair(true);
    assert!((l - (c.u1 - 1.0)).abs() < 1e-6 && (r - (c.u0 - 1.0)).abs() < 1e-6);
    assert_eq!(c.u_pair(false), (c.u0, c.u1));
}

#[test]
fn did_for_landblock() {
    assert_eq!(ImposterTable::did_for(1, 100, 50), Did(0x2701_511B));
    assert_eq!(ImposterTable::did_for(14, 0, 254), Did(0x270E_00F3));
}

#[test]
fn hostile_input() {
    let bytes = table(3).to_bytes();
    // Truncated anywhere: an error, never a panic.
    for n in 0..bytes.len() {
        assert!(ImposterTable::parse(&bytes[..n]).is_err());
    }
    // Trailing bytes.
    let mut long = bytes.clone();
    long.push(0);
    assert!(ImposterTable::parse(&long).is_err());
    // A huge count is refused before allocating.
    let mut huge = 0x2701_0000u32.to_le_bytes().to_vec();
    huge.extend_from_slice(&[1, 0xE0, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert!(ImposterTable::parse(&huge).is_err());
    // Another record type.
    let mut other = bytes;
    other[3] = 0x28;
    assert!(matches!(
        ImposterTable::parse(&other),
        Err(FormatError::WrongRecordType { .. })
    ));
    // Non-finite floats decode but give no cell.
    let mut t = table(1);
    t.cell_u = f32::NAN;
    let back = ImposterTable::parse(&t.to_bytes()).expect("decodes");
    assert!(back.cell(&back.entries[0]).is_none());
}
