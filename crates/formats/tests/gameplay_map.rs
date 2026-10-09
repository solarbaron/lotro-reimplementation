// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the map-data decoders and views (`docs/specs/map-data.md`): the worked
//! examples of §13, the validity, filing and range rules of §4.4–§4.7, the key folding and
//! hashing of §3.3, the kind recognition of §2.3 and the travel and housing views of §8–§9.
//! All bytes and DIDs are invented.

use proptest::prelude::*;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::map::{
    HousingControl, HousingControlPids, LandmarkProperties, MapKind, MapNote, MapNotesTable,
    MapPids, MapWeenieKeys, NoteFiling, NoteInvalid, PlaceIssue, PlacesTable, QuestNoteInfo,
    QuestNotesTable, Ripcord, RouteAction, TravelRoute, TravelSegment, TravelWeb, bit_keys,
    bucket_count, classify_map_kind, classify_set, distance_squared, fold_key_unit, is_null_text,
    place_bucket, place_key_hash, position_flags_consistent, position_is_valid, route_pattern,
    text_is_valid,
};
use tessera_formats::{
    Did, FormatError, Position, PropertyType, PropertyValue, StringArg, StringArgValue, StringArgs,
    StringInfo, StringSource,
};

// ---------------------------------------------------------------------------------------------
// Synthetic schema and byte builders
// ---------------------------------------------------------------------------------------------

const LAYERS: u32 = 0x4CE;
const LAYER: u32 = 0x564;
const ROLE_CONSTRAINT: u32 = 0x700;
const GAME_SPECIFIC: u32 = 0x757;
const ENABLED: u32 = 0x5EB;

fn schema(pid: u32) -> Option<PropertyType> {
    match pid {
        LAYERS => Some(PropertyType::Array),
        LAYER => Some(PropertyType::Enum),
        ROLE_CONSTRAINT => Some(PropertyType::String),
        GAME_SPECIFIC => Some(PropertyType::Struct),
        ENABLED => Some(PropertyType::Bool),
        _ => None,
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// §13.1, the first position: flags 0x9B; region 2; landblock (0x10, 0x20); cell 0; 1, 2, 3.
const POS_OUTDOOR: &str = "9B 02 10 20 00 00 00 00 80 3F 00 00 00 40 00 00 40 40";
/// §13.1, the second position: flags 0x1B; cell 0x0105; 10, 20, 30.
const POS_INDOOR: &str = "1B 02 10 20 05 01 00 00 20 41 00 00 A0 41 00 00 F0 41";

fn places_example() -> Vec<u8> {
    let mut b = hex("04 00 00 0E 00 02");
    b.extend(hex("02 61 00 62 00"));
    b.extend(hex(POS_OUTDOOR));
    b.extend(hex("00 00 00 00 00 00 00 00"));
    b.extend(hex("01 42 00"));
    b.extend(hex(POS_INDOOR));
    b.extend(hex("30 00 00 70 30 00 00 70"));
    b
}

/// §13.2.
fn note_row() -> Vec<u8> {
    let mut b = hex(POS_OUTDOOR);
    b.extend(hex("10 00 00 70 00 00 00 00 20 00 00 70"));
    b.extend(hex("CE 04 00 00 01 00 00 00 64 05 00 00 01 00 00 00"));
    b.extend(hex("00 11 00 00 00 01 00 00 25 00 01 00"));
    b.extend(hex("00 00 00 00"));
    b.extend(hex("00 00 01 00"));
    b.extend(hex("00 00 00 00 20 00 00 00"));
    b.extend(hex("00"));
    b.extend(hex("00 00 00 00 00 00 00 00 00 00 00 00"));
    b.extend(hex("00 00"));
    b
}

/// §13.3.
fn link_row() -> Vec<u8> {
    let mut b = hex(POS_OUTDOOR);
    b.extend(hex("10 00 00 70 00 00 00 00 20 00 00 70"));
    b.extend(hex("CE 04 00 00 00 00 00 00"));
    b.extend(hex("00 00 00 00 00 00 00 00 00 00 01 00"));
    b.extend(hex("00 00 00 00"));
    b.extend(hex("00 00 00 00"));
    b.extend(hex("01 00 00 00 00 00 00 00"));
    b.extend(hex("00"));
    b.extend(hex(POS_INDOOR));
    b.extend(hex("30 00 00 70"));
    b.extend(hex("00 00 00 00 00 00 00 00 00 00 00 00"));
    b.extend(hex("00 00"));
    b
}

/// §13.4.
fn quest_row() -> Vec<u8> {
    let mut b = hex(POS_INDOOR);
    b.extend(hex("00 00 00 00 30 00 00 70 40 00 00 70"));
    b.extend(hex("CE 04 00 00 00 00 00 00"));
    b.extend(hex("00 01"));
    b.extend(ROLE_CONSTRAINT.to_le_bytes());
    b.extend(ROLE_CONSTRAINT.to_le_bytes());
    b.extend(hex("03 61 62 63"));
    b
}

fn table(did: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let mut b = did.to_le_bytes().to_vec();
    b.extend(u32::try_from(rows.len()).unwrap().to_le_bytes());
    for r in rows {
        b.extend(r);
    }
    b
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn always(_: Did, _: u32) -> bool {
    true
}

fn note(rows: &[Vec<u8>]) -> MapNotesTable {
    MapNotesTable::parse(&table(0x0E00_0006, rows), &schema).unwrap()
}

// ---------------------------------------------------------------------------------------------
// §3 Named places
// ---------------------------------------------------------------------------------------------

#[test]
fn places_worked_example() {
    let bytes = places_example();
    assert_eq!(bytes.len(), 66);
    let t = PlacesTable::parse_expecting(&bytes, Did(0x0E00_0004)).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(t.size.hint, 0);
    assert_eq!(t.places[0].name.to_string(), "ab");
    assert_eq!(t.places[1].name.to_string(), "B");
    assert_eq!(t.places[0].zone(), None);
    assert_eq!(t.places[1].zone(), Some(Did(0x7000_0030)));

    // Hashes and buckets of §13.1.
    assert_eq!(place_key_hash(&units("ab")), 0x672);
    assert_eq!(place_key_hash(&units("AB")), 0x672);
    assert_eq!(place_bucket(&units("ab"), 0), Some(0));
    assert_eq!(place_bucket(&units("B"), 0), Some(10));
    assert_eq!(t.is_bucket_ordered(), Some(true));
    assert!(t.issues().is_empty(), "{:?}", t.issues());

    // §3.4 lookups.
    let first = t.position_of(&units("AB")).unwrap();
    assert_eq!(first.offset(), Some([1.0, 2.0, 3.0]));
    assert_eq!(t.area_of(&units("AB")), Some(Did(0)));
    let second = t.get_str("b").unwrap();
    assert_eq!(second.position.cell(), Some(0x0105));
    assert_eq!(second.area, Did(0x7000_0030));
    assert!(t.get_str("c").is_none());
    // The key ends at a unit 0.
    assert!(t.get(&[0x61, 0x62, 0, 0x7A]).is_some());

    assert!(matches!(
        PlacesTable::parse_expecting(&bytes, Did(0x0E00_0005)),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn places_reject_malformed_input() {
    let bytes = places_example();
    // Trailing byte (MI1).
    let mut long = bytes.clone();
    long.push(0);
    assert!(matches!(
        PlacesTable::parse(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
    // Truncated anywhere.
    for n in 0..bytes.len() {
        assert!(PlacesTable::parse(&bytes[..n]).is_err(), "prefix {n}");
    }
    // A hint above 22.
    let mut hint = bytes.clone();
    hint[4] = 23;
    assert!(matches!(
        PlacesTable::parse(&hint),
        Err(FormatError::Malformed { .. })
    ));
    // Hints 13–22 are accepted.
    hint[4] = 22;
    assert_eq!(PlacesTable::parse(&hint).unwrap().size.hint, 22);
    // A count no input could hold.
    let mut count = hex("04 00 00 0E 00 E0 FF FF FF 0F");
    count.extend([0u8; 16]);
    assert!(PlacesTable::parse(&count).is_err());
}

#[test]
fn places_issues_order_duplicates_slots() {
    // "B" first, then "ab": bucket 10 then 0, out of order. A duplicate "AB". Slots differ.
    let mut b = hex("04 00 00 0E 00 03");
    b.extend(hex("01 42 00"));
    b.extend(hex(POS_INDOOR));
    b.extend(hex("30 00 00 70 31 00 00 70"));
    b.extend(hex("02 61 00 62 00"));
    b.extend(hex(POS_OUTDOOR));
    b.extend(hex("00 00 00 00 00 00 00 00"));
    b.extend(hex("02 41 00 42 00"));
    b.extend(hex("00")); // flag byte 0: nothing present, invalid position
    b.extend(hex("00 00 00 00 00 00 00 00"));
    let t = PlacesTable::parse(&b).unwrap();
    assert_eq!(t.is_bucket_ordered(), Some(false));
    let issues = t.issues();
    assert!(issues.contains(&PlaceIssue::SlotsDiffer { index: 0 }));
    assert!(issues.contains(&PlaceIssue::OutOfBucketOrder {
        index: 1,
        non_ascii: false
    }));
    assert!(issues.contains(&PlaceIssue::DuplicateKey { index: 2, first: 1 }));
    assert!(issues.contains(&PlaceIssue::InvalidPosition { index: 2 }));
    // Flag byte 0 is what a writer emits for all-absent values, so MI4 holds.
    assert!(!issues.contains(&PlaceIssue::PositionFlags { index: 2 }));
    // The first entry of a duplicated key wins.
    assert_eq!(t.find(&units("aB")), Some(1));
}

#[test]
fn places_issues_non_ascii_order_and_nul() {
    // "B" (bucket 10 for hint 0), then "\u{C4}" (folds to 0xE4: bucket 0xE4 % 11 = 8), then a
    // name with a unit 0 in the middle.
    let mut b = hex("04 00 00 0E 00 03");
    b.extend(hex("01 42 00"));
    b.extend(hex(POS_OUTDOOR));
    b.extend([0u8; 8]);
    b.extend(hex("01 C4 00"));
    b.extend(hex(POS_OUTDOOR));
    b.extend([0u8; 8]);
    b.extend(hex("03 7A 00 00 00 62 00"));
    b.extend(hex(POS_OUTDOOR));
    b.extend([0u8; 8]);
    let t = PlacesTable::parse(&b).unwrap();
    assert_eq!(place_bucket(&[0x00C4], 0), Some(8));
    let issues = t.issues();
    assert!(issues.contains(&PlaceIssue::OutOfBucketOrder {
        index: 1,
        non_ascii: true
    }));
    assert!(issues.contains(&PlaceIssue::NameHasNul { index: 2 }));
    assert!(!issues.contains(&PlaceIssue::NameHasNul { index: 0 }));
    // The key of the third entry ends at its unit 0 (§3.3.1).
    assert_eq!(t.find(&units("z")), Some(2));
    assert_eq!(t.find(&units("\u{E4}")), Some(1));
}

#[test]
fn key_folding() {
    for u in 0..0x80u16 {
        let expect = if (0x41..=0x5A).contains(&u) {
            u + 0x20
        } else {
            u
        };
        assert_eq!(fold_key_unit(u), expect);
    }
    assert_eq!(fold_key_unit(0x00C4), 0x00E4); // Latin capital A with diaeresis
    assert_eq!(fold_key_unit(0x03A3), 0x03C3); // Greek capital sigma
    assert_eq!(fold_key_unit(0x0130), 0x0069); // simple mapping, not the full one
    assert_eq!(fold_key_unit(0xD801), 0xD801); // surrogates unchanged
    assert_eq!(fold_key_unit(0x00E4), 0x00E4);
    // The full 16-bit lowered unit enters the hash.
    assert_eq!(place_key_hash(&[0x00C4]), 0xE4);
    assert_eq!(place_key_hash(&[0x0100]), 0x101);
    // Bucket counts of every hint (PS §17.2).
    assert_eq!(bucket_count(0), Some(11));
    assert_eq!(bucket_count(10), Some(12_281));
    assert_eq!(bucket_count(13), Some(98_299));
    assert_eq!(bucket_count(22), Some(50_331_599));
    assert_eq!(bucket_count(23), None);
}

// ---------------------------------------------------------------------------------------------
// §4 Map notes
// ---------------------------------------------------------------------------------------------

#[test]
fn map_note_worked_example() {
    let row = note_row();
    assert_eq!(row.len(), 89);
    let t = note(&[row]);
    let n = &t.notes[0];
    assert_eq!(n.base.area, Did(0x7000_0010));
    assert_eq!(n.base.dungeon, Did(0));
    assert_eq!(n.base.entity, Did(0x7000_0020));
    assert_eq!(n.base.layers_pid(), Some(LAYERS));
    assert_eq!(n.base.layer_keys(), vec![1]);
    assert!(!is_null_text(&n.text));
    assert_eq!(n.level, 0x10000);
    assert_eq!(n.level_keys().collect::<Vec<_>>(), vec![0x11]);
    assert_eq!(n.note_type, 1 << 37);
    assert_eq!(n.type_bit(), Some(37));
    assert_eq!(n.type_keys().collect::<Vec<_>>(), vec![0x26]);
    assert!(!n.is_link() && n.link.is_none() && !n.flag_b);
    assert!(n.extra.is_empty());
    assert_eq!(n.validity(&always), Ok(()));
    assert_eq!(
        n.filing(),
        NoteFiling::Landblock {
            region: 2,
            key: 1_048_608
        }
    );
    assert!(n.zone_link().is_none());
    let ix = t.index();
    assert_eq!(ix.in_landblock(2, 0x10, 0x20), &[0]);
    assert!(ix.ranged.is_empty() && ix.by_dungeon.is_empty());
}

#[test]
fn link_row_worked_example() {
    let row = link_row();
    assert_eq!(row.len(), 103);
    let t = note(&[note_row(), row]);
    let n = &t.notes[1];
    assert!(n.is_link());
    assert!(is_null_text(&n.text));
    assert!(n.base.layer_keys().is_empty());
    let link = n.link.unwrap();
    assert_eq!(link.target, Did(0x7000_0030));
    assert_eq!(link.position.cell(), Some(0x0105));
    // Link rows skip the text, level and range checks.
    assert_eq!(n.validity(&|_: Did, _: u32| false), Ok(()));
    assert_eq!(n.zone_link().map(|l| l.target), Some(Did(0x7000_0030)));
    let ix = t.index();
    assert_eq!(ix.links_into(Did(0x7000_0030)), &[1]);
    assert_eq!(ix.in_landblock(2, 0x10, 0x20), &[0, 1]);
}

#[test]
fn link_into_own_dungeon_is_not_registered() {
    let mut row = link_row();
    // dungeon slot = the link target.
    row[22..26].copy_from_slice(&0x7000_0030u32.to_le_bytes());
    let t = note(&[row]);
    assert_eq!(t.notes[0].filing(), NoteFiling::Dungeon(Did(0x7000_0030)));
    assert!(t.notes[0].zone_link().is_none());
    assert_eq!(t.index().in_dungeon(Did(0x7000_0030)), &[0]);
}

#[test]
fn map_note_table_rejects_malformed_input() {
    let bytes = table(0x0E00_0006, &[note_row(), link_row()]);
    for n in 0..bytes.len() {
        assert!(
            MapNotesTable::parse(&bytes[..n], &schema).is_err(),
            "prefix {n}"
        );
    }
    let mut long = bytes.clone();
    long.push(0);
    assert!(matches!(
        MapNotesTable::parse(&long, &schema),
        Err(FormatError::TrailingBytes { .. })
    ));
    // flag_b above 1 (MI5): offset 8 + 18 + 12 + 16 + 12 + 4 + 4 + 8.
    let mut flag = bytes.clone();
    flag[8 + 74] = 2;
    assert!(matches!(
        MapNotesTable::parse(&flag, &schema),
        Err(FormatError::InvalidBool { value: 2, .. })
    ));
    // A row count above remaining / 53.
    let mut count = bytes.clone();
    count[4..8].copy_from_slice(&5u32.to_le_bytes());
    assert!(matches!(
        MapNotesTable::parse(&count, &schema),
        Err(FormatError::Truncated { .. })
    ));
    // A layers PID the schema does not know.
    let mut pid = bytes.clone();
    pid[8 + 30] = 0xCF;
    assert!(MapNotesTable::parse(&pid, &schema).is_err());
}

#[test]
fn map_note_extra_set() {
    // `extra` holding MapNote_GameSpecific (a struct) with one field, MapNote_Enabled = true.
    let mut row = note_row();
    row.truncate(row.len() - 2);
    row.extend(hex("00 01"));
    row.extend(GAME_SPECIFIC.to_le_bytes());
    row.extend(GAME_SPECIFIC.to_le_bytes());
    row.extend(hex("00 01"));
    row.extend(ENABLED.to_le_bytes());
    row.extend(ENABLED.to_le_bytes());
    row.extend(hex("01"));
    let t = note(&[row.clone()]);
    let extra = &t.notes[0].extra;
    assert_eq!(extra.len(), 1);
    let PropertyValue::Struct(s) = extra.get(GAME_SPECIFIC).unwrap() else {
        panic!("not a struct");
    };
    assert_eq!(s.fields.len(), 1);
    assert_eq!(s.fields[0].pid, ENABLED);
    assert_eq!(s.fields[0].value, PropertyValue::Bool(true));
    // Every prefix of the longer row fails.
    let bytes = table(0x0E00_0006, &[row]);
    for n in 0..bytes.len() {
        assert!(
            MapNotesTable::parse(&bytes[..n], &schema).is_err(),
            "prefix {n}"
        );
    }
}

#[test]
fn embedded_sets_reject_hints_above_22() {
    // PS §5.3 step 1, §17.2: a hint of 23 or more is malformed; 22 is accepted.
    for (hint, ok) in [(22u8, true), (23, false), (200, false)] {
        let mut row = note_row();
        let at = row.len() - 2;
        row[at] = hint;
        let r = MapNotesTable::parse(&table(0x0E00_0006, &[row]), &schema);
        if ok {
            assert_eq!(r.unwrap().notes[0].extra.size.hint, hint);
        } else {
            assert!(
                matches!(r, Err(FormatError::Malformed { .. })),
                "extra hint {hint}: {r:?}"
            );
        }

        let mut row = quest_row();
        // position 18, three DIDs 12, empty layers array 8: the set starts at 38.
        assert_eq!(row[38..40], [0x00, 0x01]);
        row[38] = hint;
        let r = QuestNotesTable::parse(&table(0x0E40_0000, &[row]), &schema);
        if ok {
            assert_eq!(r.unwrap().notes[0].quest_info.size.hint, hint);
        } else {
            assert!(
                matches!(r, Err(FormatError::Malformed { .. })),
                "quest_info hint {hint}: {r:?}"
            );
        }
    }
}

#[test]
fn smallest_row_is_53_bytes() {
    let mut b = hex("00"); // empty position
    b.extend([0u8; 12]);
    b.extend([0u8; 4]); // null layers property
    b.extend(hex("01 00 00 01 00")); // empty literal
    b.extend([0u8; 17]);
    b.extend([0u8; 12]);
    b.extend(hex("00 00"));
    assert_eq!(b.len(), 53);
    let t = note(&[b]);
    let n = &t.notes[0];
    assert!(n.base.layers.is_none());
    assert_eq!(n.validity(&always), Err(NoteInvalid::Position));
}

fn with_position(n: &MapNote, p: Position) -> MapNote {
    let mut n = n.clone();
    n.base.position = p;
    n
}

#[test]
fn validity_rules() {
    let t = note(&[note_row()]);
    let n = &t.notes[0];
    let good = n.base.position;
    assert!(position_is_valid(&good));

    // §4.4.1 tests 1–6.
    let base = Position::new(0)
        .with_region(2)
        .with_landblock(1, 2)
        .with_cell(0);
    assert!(position_is_valid(&base));
    let checks = [
        Position::new(0).with_landblock(1, 2).with_cell(0),
        Position::new(0).with_region(2).with_cell(0),
        Position::new(0)
            .with_region(2)
            .with_landblock(0xFF, 2)
            .with_cell(0),
        Position::new(0)
            .with_region(2)
            .with_landblock(1, 0xFF)
            .with_cell(0),
        Position::new(0).with_region(2).with_landblock(1, 2),
        base.with_cell(0xFFFF),
        base.with_offset([f32::NAN, 0.0, 0.0]),
        base.with_offset([0.0, f32::INFINITY, 0.0]),
        base.with_orientation([1.0, f32::NAN, 0.0, 0.0]),
        base.with_orientation([0.9, 0.0, 0.0, 0.0]),
    ];
    for p in checks {
        assert!(!position_is_valid(&p), "{p:?}");
        assert_eq!(
            with_position(n, p).validity(&always),
            Err(NoteInvalid::Position)
        );
    }
    // A turn about the vertical axis is unit length.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    assert!(position_is_valid(
        &base.with_orientation([half, 0.0, 0.0, half])
    ));
    // 0x40 without a region byte is region 0, so the position fails.
    assert!(!position_is_valid(
        &Position::new(0x40).with_landblock(1, 2).with_cell(0)
    ));

    let mut m = n.clone();
    m.note_type = 0;
    assert_eq!(m.validity(&always), Err(NoteInvalid::NoType));

    assert_eq!(n.validity(&|_: Did, _: u32| false), Err(NoteInvalid::Text));
    let mut m = n.clone();
    m.text = StringInfo::parse(&hex("00 00 00 00 00 01 00 00 25 00 01 00")).unwrap();
    assert_eq!(m.validity(&always), Err(NoteInvalid::Text));
    m.text = StringInfo::parse(&hex("01 00 00 01 00")).unwrap();
    assert_eq!(m.validity(&|_: Did, _: u32| false), Ok(()));

    let mut m = n.clone();
    m.level = 0;
    assert_eq!(m.validity(&always), Err(NoteInvalid::NoLevel));

    let mut m = n.clone();
    m.min_range = 10.0;
    m.max_range = 9.0;
    assert_eq!(m.validity(&always), Err(NoteInvalid::Ranges));
    m.max_range = 10.0 - 0.0001;
    assert_eq!(m.validity(&always), Ok(()));
}

#[test]
fn text_test_checks_table_and_token() {
    let text = StringInfo::parse(&hex("00 11 00 00 00 01 00 00 25 00 01 00")).unwrap();
    assert!(text_is_valid(&text, &|t: Did, k: u32| t
        == Did(0x2500_0001)
        && k == 0x11));
    assert!(!text_is_valid(&text, &|_: Did, k: u32| k == 0x12));
}

#[test]
fn text_test_checks_nested_arguments() {
    // The Tessera rule of §4.4.2: a nested StringInfo argument must pass the test; any other
    // argument passes.
    let reference = |token: u32, args: Vec<StringArg>| StringInfo {
        source: StringSource::Table {
            token,
            table: Did(0x2500_0001),
        },
        args: (!args.is_empty()).then(|| {
            Box::new(StringArgs {
                labels: Default::default(),
                values: args,
            })
        }),
    };
    let nested = |inner: StringInfo| StringArg {
        var_id: 0xAABB_CCDD,
        value: StringArgValue::Nested(Box::new(inner)),
    };
    let only_0x11 = |t: Did, k: u32| t == Did(0x2500_0001) && k == 0x11;
    let good = reference(0x11, vec![nested(reference(0x11, vec![]))]);
    assert!(text_is_valid(&good, &only_0x11));
    let bad = reference(0x11, vec![nested(reference(0x12, vec![]))]);
    assert!(!text_is_valid(&bad, &only_0x11));
    let null_inner = reference(0x11, vec![nested(reference(0, vec![]))]);
    assert!(!text_is_valid(&null_inner, &always));
    // A literal nested argument passes, and so does a number.
    let literal = StringInfo::parse(&hex("01 00 00 01 00")).unwrap();
    let mixed = reference(
        0x11,
        vec![
            nested(literal),
            StringArg {
                var_id: 1,
                value: StringArgValue::Integer(7),
            },
        ],
    );
    assert!(text_is_valid(&mixed, &only_0x11));
    // Two levels down still count.
    let deep = reference(
        0x11,
        vec![nested(reference(
            0x11,
            vec![nested(reference(0x13, vec![]))],
        ))],
    );
    assert!(!text_is_valid(&deep, &only_0x11));
}

#[test]
fn filing_and_radar_ranges() {
    let t = note(&[note_row()]);
    let mut n = t.notes[0].clone();
    // One range zero: filed by landblock, not range-tested.
    n.min_range = 0.0;
    n.max_range = 800.0;
    assert!(matches!(n.filing(), NoteFiling::Landblock { .. }));
    assert!(!n.is_ranged());
    assert!(n.radar_shows(1.0e9));
    // Both non-zero: ranged.
    n.min_range = 80.0;
    assert_eq!(n.filing(), NoteFiling::Ranged);
    assert!(n.is_ranged());
    assert!(n.radar_shows(80.0 * 80.0));
    assert!(n.radar_shows(800.0 * 800.0));
    assert!(n.radar_shows(400.0 * 400.0));
    assert!(!n.radar_shows(79.0 * 79.0));
    assert!(!n.radar_shows(801.0 * 801.0));
    assert_eq!(distance_squared([0.0, 0.0, 0.0], [3.0, 4.0, 12.0]), 169.0);
    // A dungeon wins over ranges.
    n.base.dungeon = Did(0x7000_0040);
    assert_eq!(n.filing(), NoteFiling::Dungeon(Did(0x7000_0040)));
    let ix = tessera_formats::gameplay::map::MapNoteIndex::build(&[n]);
    assert_eq!(ix.in_dungeon(Did(0x7000_0040)), &[0]);
}

#[test]
fn position_flag_rules() {
    let t = PlacesTable::parse(&places_example()).unwrap();
    for p in t.iter() {
        assert!(position_flags_consistent(&p.position));
    }
    // 0x9B without the 0x80 bit disagrees with cell 0.
    let p = Position::new(0x1B)
        .with_region(2)
        .with_landblock(0x10, 0x20)
        .with_cell(0)
        .with_offset([1.0, 2.0, 3.0]);
    assert!(!position_flags_consistent(&p));
    assert!(!position_flags_consistent(
        &Position::new(0x80).with_region(1)
    ));
    assert!(!position_flags_consistent(
        &Position::new(0).with_instance(3)
    ));
}

#[test]
fn bits_to_keys() {
    assert_eq!(
        bit_keys(0xF0000).collect::<Vec<_>>(),
        vec![0x11, 0x12, 0x13, 0x14]
    );
    assert_eq!(bit_keys(1).collect::<Vec<_>>(), vec![1]);
    assert_eq!(bit_keys(1 << 63).collect::<Vec<_>>(), vec![64]);
}

// ---------------------------------------------------------------------------------------------
// §5 Dungeon quest notes
// ---------------------------------------------------------------------------------------------

#[test]
fn quest_note_worked_example() {
    let row = quest_row();
    assert_eq!(row.len(), 52);
    let bytes = table(0x0E40_0000, &[row.clone(), row]);
    let t = QuestNotesTable::parse_expecting(&bytes, Did(0x0E40_0000), &schema).unwrap();
    assert_eq!(t.len(), 2);
    let n = &t.notes[0];
    assert_eq!(n.base.dungeon, Did(0x7000_0030));
    assert_eq!(n.base.entity, Did(0x7000_0040));
    assert!(n.base.position_values().is_interior());
    assert!(position_is_valid(&n.base.position));
    let names = |l: &str| (l == "QuestDispenser_RoleConstraint").then_some(ROLE_CONSTRAINT);
    let pids = MapPids::resolve(&names);
    let info = QuestNoteInfo::read(&n.quest_info, &pids);
    assert_eq!(info.role_constraint.as_deref(), Some("abc"));
    assert!(info.active_quests.is_empty());
    assert_eq!(t.by_dungeon().get(&Did(0x7000_0030)).unwrap(), &vec![0, 1]);

    for n in 0..bytes.len() {
        assert!(
            QuestNotesTable::parse(&bytes[..n], &schema).is_err(),
            "prefix {n}"
        );
    }
    let mut count = bytes.clone();
    count[4..8].copy_from_slice(&6u32.to_le_bytes());
    assert!(QuestNotesTable::parse(&count, &schema).is_err());
}

// ---------------------------------------------------------------------------------------------
// §2.3, §8, §9 Views
// ---------------------------------------------------------------------------------------------

/// Synthetic PIDs: label → 0x1000 + its index in [`MapPids::LABELS`].
fn pids() -> (MapPids, impl Fn(&str) -> u32) {
    let id = |l: &str| {
        0x1000 + u32::try_from(MapPids::LABELS.iter().position(|x| *x == l).unwrap()).unwrap()
    };
    let names = move |l: &str| MapPids::LABELS.contains(&l).then(|| id(l));
    (MapPids::resolve(&names), id)
}

#[test]
fn kinds_and_travel_views() {
    let (p, id) = pids();
    assert!(p.missing().is_empty());
    // The `WeenieType` key labelled `Landmark` (§2.3); 0x4D is invented.
    let wk = MapWeenieKeys::from_labels([(0x10, "Door"), (0x4D, "Landmark")]);
    assert_eq!(wk.landmark, Some(0x4D));

    // §13.5: station W with home D1 selling route R for 500.
    let w = set(vec![
        entry(id("TravelHomeLocation"), did(0x7000_00D1)),
        entry(
            id("TravelDestinationRecordArray"),
            array(vec![entry(
                0x9999,
                structure(vec![
                    entry(id("TravelDestinationRoute"), did(0x7000_00A1)),
                    entry(id("TravelDestinationCost"), PropertyValue::Int(500)),
                ]),
            )]),
        ),
    ]);
    assert_eq!(classify_set(&w, &p, &wk), Some(MapKind::TravelWeb));
    let web = TravelWeb::read(&w, &p).unwrap();
    assert_eq!(web.home, Did(0x7000_00D1));
    assert_eq!(web.offers.len(), 1);
    assert_eq!(web.offers[0].route, Did(0x7000_00A1));
    assert_eq!(web.offers[0].cost, Some(500));

    let r = set(vec![
        entry(id("TravelRoute_Destination"), did(0x7000_00D2)),
        entry(id("TravelRoute_TravelMode"), did(0x7000_00E1)),
        entry(
            id("TravelRoute_ActionArray"),
            array(vec![
                entry(0x9998, did(0x7000_0051)),
                entry(0x9998, did(0x7000_0052)),
                entry(0x9998, did(0x7000_0053)),
            ]),
        ),
        entry(id("Usage_MinLevel"), PropertyValue::Int(20)),
    ]);
    assert_eq!(classify_set(&r, &p, &wk), Some(MapKind::TravelRoute));
    let route = TravelRoute::read(&r, &p).unwrap();
    assert_eq!(route.destination, Did(0x7000_00D2));
    assert_eq!(route.mode, Some(Did(0x7000_00E1)));
    assert_eq!(route.actions.len(), 3);
    assert_eq!(route.min_level, Some(20));
    // A route set missing its destination is an error.
    assert!(matches!(
        TravelRoute::read(&set(vec![]), &p),
        Err(FormatError::MissingProperty { .. })
    ));

    let seg = set(vec![entry(
        id("TravelSegment_HeadName"),
        PropertyValue::String("head".into()),
    )]);
    assert_eq!(classify_set(&seg, &p, &wk), Some(MapKind::TravelSegment));
    assert!(TravelSegment::read(&seg, &p).is_well_formed());
    let both = set(vec![
        entry(
            id("TravelSegment_HeadName"),
            PropertyValue::String("x".into()),
        ),
        entry(
            id("TravelSegment_SubSegmentArray"),
            array(vec![entry(0x9997, did(0x7000_0061))]),
        ),
    ]);
    assert!(!TravelSegment::read(&both, &p).is_well_formed());

    let pad = set(vec![entry(
        id("TravelPad_Location"),
        PropertyValue::String("key".into()),
    )]);
    assert_eq!(classify_set(&pad, &p, &wk), Some(MapKind::TravelPad));
    // An NPC with a pad location and a station is not a pad.
    let npc = set(vec![
        entry(
            id("TravelPad_Location"),
            PropertyValue::String("key".into()),
        ),
        entry(id("TravelWebWC"), did(0x7000_0099)),
    ]);
    assert_eq!(classify_set(&npc, &p, &wk), None);
    let sm = tessera_formats::gameplay::map::StableMaster::read(&npc, &p).unwrap();
    assert_eq!(sm.station, Some(Did(0x7000_0099)));
    assert_eq!(sm.pad.as_deref(), Some("key"));

    let hop = set(vec![entry(
        id("TravelHop_Delay"),
        PropertyValue::Float(0.0),
    )]);
    assert_eq!(classify_set(&hop, &p, &wk), Some(MapKind::TravelHop));

    let house = set(vec![
        entry(id("House_NeighborhoodTemplate"), did(0x7000_0071)),
        entry(id("House_Type"), PropertyValue::Enum(3)),
    ]);
    assert_eq!(classify_set(&house, &p, &wk), Some(MapKind::House));
    let h = tessera_formats::gameplay::map::House::read(&house, &p).unwrap();
    assert_eq!((h.template, h.house_type), (Did(0x7000_0071), Some(3)));

    let template = set(vec![
        entry(id("NeighborhoodTemplate_HouseList"), did(0x7000_0072)),
        entry(
            id("NeighborhoodTemplate_Boundaries"),
            array(vec![entry(
                0x9996,
                PropertyValue::Position(Position::new(0).with_region(1).with_landblock(3, 4)),
            )]),
        ),
    ]);
    assert_eq!(
        classify_set(&template, &p, &wk),
        Some(MapKind::NeighbourhoodTemplate)
    );
    let tv = tessera_formats::gameplay::map::NeighbourhoodTemplate::read(&template, &p).unwrap();
    assert_eq!(tv.boundaries[0].landblock(), Some((3, 4)));

    assert_eq!(classify_set(&set(vec![]), &p, &wk), None);
    // `classify_map_kind` never reports a pad; `classify_set` does.
    assert_eq!(classify_map_kind(&pad, &p, &wk), None);

    // Zones (WS §11.1).
    let zone = |label: &str| set(vec![entry(id(label), did(0x7000_0001))]);
    assert_eq!(
        classify_set(&zone("Area_Territory"), &p, &wk),
        Some(MapKind::Area)
    );
    assert_eq!(
        classify_set(&zone("Area_Region"), &p, &wk),
        Some(MapKind::Territory)
    );
    assert_eq!(
        classify_set(
            &set(vec![entry(id("Area_RegionID"), PropertyValue::Enum(2))]),
            &p,
            &wk
        ),
        Some(MapKind::Region)
    );
    assert_eq!(
        classify_set(&zone("Dungeon_Name"), &p, &wk),
        Some(MapKind::Dungeon)
    );

    // §2.3 (MD44): a landmark is `WeenieType` `Landmark` with `MapNote_Type`; the music is
    // optional (MD49), and without the mapper key no set is a landmark.
    let landmark = |music: bool| {
        let mut entries = vec![
            entry(id("WeenieType"), PropertyValue::Enum(0x4D)),
            entry(id("MapNote_Type"), PropertyValue::Bitfield32(1)),
        ];
        if music {
            entries.push(entry(id("Landmark_Music"), PropertyValue::Enum(7)));
            entries.push(entry(
                id("Landmark_OverrideMountedCombat_Allow"),
                PropertyValue::Bool(true),
            ));
        }
        set(entries)
    };
    assert_eq!(
        classify_set(&landmark(true), &p, &wk),
        Some(MapKind::Landmark)
    );
    assert_eq!(
        classify_set(&landmark(false), &p, &wk),
        Some(MapKind::Landmark)
    );
    assert_eq!(
        classify_set(&landmark(true), &p, &MapWeenieKeys::default()),
        None
    );
    let no_note = set(vec![
        entry(id("WeenieType"), PropertyValue::Enum(0x4D)),
        entry(id("Landmark_Music"), PropertyValue::Enum(7)),
    ]);
    assert_eq!(classify_set(&no_note, &p, &wk), None);
    let lp = LandmarkProperties::read(&landmark(true), &p);
    assert_eq!(
        (
            lp.music,
            lp.mounted_combat_allow,
            lp.mounted_combat_disallow
        ),
        (Some(7), Some(true), None)
    );
    assert_eq!(
        LandmarkProperties::read(&landmark(false), &p),
        LandmarkProperties::default()
    );

    // §11 (MD48): ripcord field types.
    let rc = set(vec![
        entry(
            id("RipCord_WorldProperty"),
            PropertyValue::PropertyId(0x1000_0777),
        ),
        entry(
            id("RipCord_WorldPropertyTableProperty"),
            PropertyValue::PropertyId(0x1000_0778),
        ),
        entry(id("RipCord_UIElement_ID"), PropertyValue::Enum(0x1000_0055)),
        entry(id("RipCord_UIElement_Layout"), did(0x2200_0012)),
        entry(
            id("RipCord_DisableEditingInAdminPanel"),
            PropertyValue::Bool(false),
        ),
        entry(id("RipCord_Name"), PropertyValue::String("n".into())),
    ]);
    assert_eq!(classify_set(&rc, &p, &wk), Some(MapKind::Ripcord));
    let r = Ripcord::read(&rc, &p);
    assert_eq!(r.world_property, Some(0x1000_0777));
    assert_eq!(r.table_property, Some(0x1000_0778));
    assert_eq!(r.array_element, None);
    assert_eq!(r.ui_element_id, Some(0x1000_0055));
    assert_eq!(r.ui_element_layout, Some(Did(0x2200_0012)));
    assert_eq!(r.disable_editing, Some(false));
    assert_eq!(r.name.as_deref(), Some("n"));
}

// §9.7 (MD47): the housing-control record, read by its full labels.
#[test]
fn housing_control_view() {
    let id = |l: &str| {
        0x2000
            + u32::try_from(
                HousingControlPids::LABELS
                    .iter()
                    .position(|x| *x == l)
                    .unwrap(),
            )
            .unwrap()
    };
    let names = move |l: &str| HousingControlPids::LABELS.contains(&l).then(|| id(l));
    let p = HousingControlPids::resolve(&names);
    assert!(p.missing().is_empty());
    let hc = set(vec![
        entry(
            id("HousingControl_HouseTypeInfoArray"),
            array(vec![entry(
                0x9990,
                structure(vec![
                    entry(id("HousingControl_HouseType"), PropertyValue::Enum(3)),
                    entry(id("HousingControl_HousePrice"), PropertyValue::Int(1_000)),
                    entry(id("HousingControl_HouseUpkeep"), PropertyValue::Int(100)),
                    entry(id("HousingControl_Icon32"), did(0x4100_0003)),
                ]),
            )]),
        ),
        entry(
            id("HousingControl_HouseValueTierArray"),
            array(vec![entry(
                0x9991,
                structure(vec![
                    entry(id("HousingControl_HouseValueTier"), PropertyValue::Enum(2)),
                    entry(
                        id("HousingControl_HouseValueTier_PriceModifier"),
                        PropertyValue::Float(1.5),
                    ),
                    entry(
                        id("HousingControl_HouseValueTier_UpkeepPriceModifier"),
                        PropertyValue::Float(2.0),
                    ),
                ]),
            )]),
        ),
        entry(
            id("HousingControl_HousingGroupInfo_Kinship"),
            array(vec![entry(
                0x9992,
                structure(vec![
                    entry(
                        id("HousingControl_HousingGroupInfo_Group"),
                        PropertyValue::Enum(1),
                    ),
                    entry(
                        id("HousingControl_HousingGroupInfo_DefaultPermission"),
                        PropertyValue::Bitfield32(0b101),
                    ),
                ]),
            )]),
        ),
        entry(
            id("HousingControl_UpkeepInterval"),
            PropertyValue::Int(604_800),
        ),
        entry(
            id("HousingControl_MithrilToWritConversionRate"),
            PropertyValue::Float(0.5),
        ),
        entry(id("HousingControl_ForSaleSignDataID"), did(0x7000_0123)),
        entry(
            id("HousingControl_NeighborhoodContentLayer"),
            PropertyValue::Enum(9),
        ),
    ]);
    let h = HousingControl::read(&hc, &p);
    assert_eq!(h.house_types.len(), 1);
    let t = h.house_types[0];
    assert_eq!(
        (t.house_type, t.price, t.upkeep),
        (Some(3), Some(1_000), Some(100))
    );
    assert_eq!(t.icons[2], Some(Did(0x4100_0003)));
    assert_eq!(t.mount_tokens, None);
    assert_eq!(h.value_tiers[0].price_modifier, Some(1.5));
    assert_eq!(h.value_tiers[0].upkeep_modifier, Some(2.0));
    assert!(h.personal_groups.is_empty());
    assert_eq!(h.kinship_groups[0].default_permission, Some(0b101));
    assert_eq!(h.kinship_groups[0].cant_change_permission, None);
    assert_eq!(h.upkeep_interval, Some(604_800));
    assert_eq!(h.writ_conversion_rate, Some(0.5));
    assert_eq!(h.for_sale_sign, Some(Did(0x7000_0123)));
    assert_eq!(h.neighborhood_content_layer, Some(9));
    assert_eq!(
        HousingControl::read(&set(vec![]), &p),
        HousingControl::default()
    );
}

#[test]
fn action_patterns() {
    use RouteAction::{Hop as H, Pad as P, Segment as S};
    assert_eq!(route_pattern(&[S, H, S]), "S H S");
    assert_eq!(route_pattern(&[S, S, S]), "S+");
    assert_eq!(route_pattern(&[H, P]), "H P");
    assert_eq!(route_pattern(&[S, S, H, S, S]), "S+ H S+");
    assert_eq!(route_pattern(&[S]), "S");
    assert_eq!(route_pattern(&[]), "");
}

#[test]
fn place_references_in_sets() {
    let t = PlacesTable::parse(&places_example()).unwrap();
    let s = set(vec![
        entry(1, PropertyValue::String("AB".into())),
        entry(
            2,
            structure(vec![entry(3, PropertyValue::String("b".into()))]),
        ),
        entry(4, PropertyValue::String("nowhere".into())),
    ]);
    let mut found = Vec::new();
    tessera_formats::gameplay::map::for_each_place_reference(&s, &t, |r| {
        found.push((r.pid, r.parent, r.place));
    });
    assert_eq!(found, vec![(1, None, 0), (3, Some(2), 1)]);
}

// ---------------------------------------------------------------------------------------------
// Hostile input
// ---------------------------------------------------------------------------------------------

proptest! {
    #[test]
    fn decoders_never_panic(data in proptest::collection::vec(any::<u8>(), 0..400)) {
        let _ = PlacesTable::parse(&data);
        let _ = MapNotesTable::parse(&data, &schema);
        let _ = QuestNotesTable::parse(&data, &schema);
    }

    #[test]
    fn mutated_examples_never_panic(
        index in 0usize..300,
        byte in any::<u8>(),
        which in 0usize..3,
    ) {
        let mut data = match which {
            0 => places_example(),
            1 => table(0x0E00_0006, &[note_row(), link_row()]),
            _ => table(0x0E40_0000, &[quest_row()]),
        };
        let i = index % data.len();
        data[i] = byte;
        let _ = PlacesTable::parse(&data);
        if let Ok(t) = MapNotesTable::parse(&data, &schema) {
            let ix = t.index();
            for n in &t.notes {
                let _ = n.validity(&always);
                prop_assert_eq!(n.link.is_some(), n.is_link());
            }
            prop_assert!(ix.ranged.len() <= t.len());
        }
        let _ = QuestNotesTable::parse(&data, &schema);
    }
}
