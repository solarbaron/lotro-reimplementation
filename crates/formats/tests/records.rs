// SPDX-License-Identifier: GPL-3.0-or-later
//! String tables, StringInfo, property dictionary, enum mappers, DID maps (spec §3–§8).

mod common;

use std::borrow::Cow;

use common::B;
use tessera_formats::property::{FLAG_HAS_DEFAULT, FLAG_HAS_MAX, FLAG_HAS_MIN, Waveform};
use tessera_formats::string_info::{STRING_INFO_MAX_DEPTH, StringArgValue, StringSource};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, FormatError, PropertyDictionary, PropertyType, PropertyValue,
    Reader, Record, StringInfo, StringTable, name_hash,
};

// ---------------------------------------------------------------- string tables

fn entry(b: B, token: u32, pieces: &[&str], args: &[u32], names: Option<&[&str]>) -> B {
    let mut b = b.u32(token).u32(0).u32(pieces.len() as u32);
    for p in pieces {
        b = b.utf16(p);
    }
    b = b.u32(args.len() as u32);
    for a in args {
        b = b.u32(*a);
    }
    match names {
        None => b.u8(0),
        Some(names) => {
            b = b.u8(1).u32(names.len() as u32);
            for n in names {
                b = b.utf16(n);
            }
            b
        }
    }
}

fn sample_table() -> Vec<u8> {
    let b = B::new().u32(0x2500_0042).u32(1).tsize(0, 3);
    let b = entry(b, 7, &[], &[], None); // p = 0
    let b = entry(b, 3, &["Hello"], &[], None); // p = 1
    let who = name_hash(b"who");
    let what = name_hash(b"what");
    let b = entry(
        b,
        12,
        &["", " gives ", " to you."],
        &[who, what],
        Some(&["who", "what"]),
    );
    b.build()
}

#[test]
fn string_table_parses_all_entry_shapes() {
    let data = sample_table();
    let t = StringTable::parse(&data).unwrap();
    assert_eq!(t.did, Did(0x2500_0042));
    assert_eq!(t.flag, 1);
    assert_eq!(t.size.count, 3);
    assert_eq!(t.entries().len(), 3);

    let empty = t.get(7).unwrap();
    assert!(empty.pieces.is_empty() && empty.arg_ids.is_empty());
    assert_eq!(empty.plain_text(), "");

    assert_eq!(t.get(3).unwrap().plain_text(), "Hello");
    assert!(t.get(999).is_none(), "missing token is a miss (ST7)");

    let e = t.get(12).unwrap();
    assert_eq!(e.arg_names().unwrap().len(), 2);
    assert!(e.arg_names_match_ids());
    let who = name_hash(b"who");
    let rendered = e.render(|id| Some(Cow::Borrowed(if id == who { "Alice" } else { "a sword" })));
    assert_eq!(rendered, "Alice gives a sword to you.");
    assert_eq!(e.plain_text(), " gives  to you.");
}

#[test]
fn string_table_checks_did_and_trailing_bytes() {
    let mut data = sample_table();
    assert!(StringTable::parse_expecting(&data, Did(0x2500_0042)).is_ok());
    assert_eq!(
        StringTable::parse_expecting(&data, Did(0x2500_0043)).unwrap_err(),
        FormatError::DidMismatch {
            expected: 0x2500_0043,
            found: 0x2500_0042
        }
    );
    data.push(0);
    assert!(matches!(
        StringTable::parse(&data),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
    data.pop();
    data[3] = 0x41;
    assert!(matches!(
        StringTable::parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn string_table_huge_count_is_rejected_without_allocating() {
    let data = B::new().u32(0x2500_0001).u32(1).tsize(12, u32::MAX).build();
    assert!(matches!(
        StringTable::parse(&data),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn string_table_truncated_everywhere_is_an_error() {
    let data = sample_table();
    for cut in 0..data.len() {
        assert!(StringTable::parse(&data[..cut]).is_err(), "cut at {cut}");
    }
}

// ---------------------------------------------------------------- StringInfo

#[test]
fn string_info_literal_and_reference() {
    let lit = StringInfo::parse(&B::new().si_literal("Sword").build()).unwrap();
    assert_eq!(lit.source, StringSource::Literal("Sword".into()));
    assert!(lit.args.is_none());

    let r = StringInfo::parse(&B::new().si_ref(0x55, 0x2500_0001).build()).unwrap();
    assert_eq!(
        r.source,
        StringSource::Table {
            token: 0x55,
            table: Did(0x2500_0001)
        }
    );
}

#[test]
fn string_info_with_arguments() {
    let data = B::new()
        .u8(0) // table reference
        .u32(1)
        .u32(0x2500_0002)
        .u8(1) // has_args
        .pascal("a")
        .pascal("")
        .pascal("c")
        .vle(3)
        // kind 2: float
        .u8(2)
        .u32(name_hash(b"x"))
        .u8(1)
        .f32(2.5)
        // kind 4: integer
        .u8(4)
        .u32(name_hash(b"n"))
        .u8(1)
        .vle(300)
        // kind 1: nested
        .u8(1)
        .u32(name_hash(b"s"))
        .si_literal("inner")
        .build();
    let si = StringInfo::parse(&data).unwrap();
    let args = si.args.unwrap();
    assert_eq!(
        args.labels,
        ["a".to_string(), String::new(), "c".to_string()]
    );
    assert_eq!(args.values.len(), 3);
    assert_eq!(args.values[0].value, StringArgValue::Float(2.5));
    assert_eq!(args.values[1].value, StringArgValue::Integer(300));
    assert_eq!(args.values[1].var_id, name_hash(b"n"));
    match &args.values[2].value {
        StringArgValue::Nested(inner) => {
            assert_eq!(inner.source, StringSource::Literal("inner".into()));
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn nested(levels: usize) -> Vec<u8> {
    // Each level is a literal with one nested argument; the innermost has none.
    let mut b = B::new();
    for _ in 1..levels {
        b = b
            .u8(1)
            .utf16("")
            .u8(1)
            .pascal("")
            .pascal("")
            .pascal("")
            .vle(1)
            .u8(1)
            .u32(0);
    }
    b.si_literal("leaf").build()
}

#[test]
fn string_info_depth_limit() {
    assert!(StringInfo::parse(&nested(STRING_INFO_MAX_DEPTH)).is_ok());
    assert!(matches!(
        StringInfo::parse(&nested(STRING_INFO_MAX_DEPTH + 1)),
        Err(FormatError::DepthLimit { .. })
    ));
}

#[test]
fn string_info_rejects_bad_terminator_kind_and_marker() {
    let bad_term = B::new().u8(1).utf16("x").u8(0).u8(0).u8(0).build();
    assert!(matches!(
        StringInfo::parse(&bad_term),
        Err(FormatError::Malformed { .. })
    ));
    let head = || {
        B::new()
            .u8(1)
            .utf16("")
            .u8(1)
            .pascal("")
            .pascal("")
            .pascal("")
            .vle(1)
    };
    let bad_kind = head().u8(3).u32(0).u8(1).u32(0).build();
    assert!(matches!(
        StringInfo::parse(&bad_kind),
        Err(FormatError::Malformed { .. })
    ));
    let bad_marker = head().u8(2).u32(0).u8(0).f32(1.0).build();
    assert!(matches!(
        StringInfo::parse(&bad_marker),
        Err(FormatError::Malformed { .. })
    ));
}

// ---------------------------------------------------------------- property dictionary

/// Appends one (pid, definition) pair.
fn def(b: B, pid: u32, ty: u32, data: u32, flags: u32, values: impl Fn(B) -> B) -> B {
    let b = b
        .u32(pid)
        .u32(pid)
        .u32(ty)
        .u32(0x1000_0001)
        .u32(4)
        .u32(data)
        .u32(0)
        .u32(flags);
    let b = values(b);
    b.u32(0x3FC0_0000)
        .bytes(&[2, 2, 4, 0, 1])
        .vle(0)
        .u32(0)
        .u32(0)
}

const ALL: u32 = FLAG_HAS_DEFAULT | FLAG_HAS_MIN | FLAG_HAS_MAX;

/// (type code, writer of one value, expected decoded value)
type ValueCase = (u32, fn(B) -> B, PropertyValue);

fn value_cases() -> Vec<ValueCase> {
    vec![
        (1, |b| b.pascal("abc"), PropertyValue::String("abc".into())),
        (2, |b| b.u32(77), PropertyValue::StringToken(77)),
        (
            3,
            |b| b.u32(1).f32(0.5),
            PropertyValue::Waveform(Waveform {
                subtype: 1,
                values: vec![0.5],
                extra: None,
            }),
        ),
        (4, |b| b.f64(1e9), PropertyValue::Timestamp(1e9)),
        (5, |b| b.u8(2), PropertyValue::TriState(2)),
        (
            6,
            |b| b.f32(1.0).f32(2.0).f32(3.0),
            PropertyValue::Vector3([1.0, 2.0, 3.0]),
        ),
        (7, |b| b.u64(1 << 40), PropertyValue::InstanceId(1 << 40)),
        (8, |b| b.u32(5), PropertyValue::Enum(5)),
        (9, |b| b.f32(0.25), PropertyValue::Float(0.25)),
        (
            13,
            |b| b.si_literal("hi"),
            PropertyValue::StringInfo(Box::new(StringInfo {
                source: StringSource::Literal("hi".into()),
                args: None,
            })),
        ),
        (14, |b| b.u64(0xF0), PropertyValue::Bitfield64(0xF0)),
        (15, |b| b.u32(-5i32 as u32), PropertyValue::Int(-5)),
        (
            16,
            |b| b.bytes(&[1, 2, 3, 4]),
            PropertyValue::Colour([1, 2, 3, 4]),
        ),
        (18, |b| b.u32(0x10), PropertyValue::Bitfield32(0x10)),
        (19, |b| b.u64(-9i64 as u64), PropertyValue::Int64(-9)),
        (
            20,
            |b| b.u32(0x4100_0001),
            PropertyValue::DidRef(Did(0x4100_0001)),
        ),
        (21, |b| b.u8(1), PropertyValue::Bool(true)),
        (
            22,
            |b| b.vle(10).bytes(&[0xFF, 0x03]),
            PropertyValue::BitfieldAny {
                bits: 10,
                bytes: vec![0xFF, 0x03],
            },
        ),
    ]
}

fn dictionary(defs: impl Fn(B) -> B, def_count: u32, names: &[(u32, &str)]) -> Vec<u8> {
    let mut b = B::new()
        .u32(0x3400_0000)
        .bytes(&[0; 8])
        .tsize(0, names.len() as u32);
    for (pid, name) in names {
        b = b.u32(*pid).pascal(name);
    }
    b = b.u16(0).tsize(0, def_count);
    defs(b).build()
}

#[test]
fn property_values_of_every_specified_type() {
    for (code, write, expected) in value_cases() {
        let data = dictionary(
            |b| def(b, 0x1000_0010, code, 0, ALL, |b| write(write(write(b)))),
            1,
            &[(0, "Invalid"), (0x1000_0010, "Prop")],
        );
        let d = PropertyDictionary::parse(&data).unwrap_or_else(|e| panic!("type {code}: {e}"));
        let def = d.definition(0x1000_0010).unwrap();
        assert_eq!(def.ty, PropertyType::from_code(code));
        assert_eq!(def.default.as_ref(), Some(&expected), "type {code}");
        assert_eq!(def.minimum.as_ref(), Some(&expected), "type {code}");
        assert_eq!(def.maximum.as_ref(), Some(&expected), "type {code}");
    }
}

#[test]
fn property_waveform_subtypes() {
    let mut r_data = B::new().u32(0).build();
    let mut r = Reader::new(&r_data);
    let v = PropertyValue::read(&mut r, PropertyType::Waveform).unwrap();
    assert_eq!(
        v,
        PropertyValue::Waveform(Waveform {
            subtype: 0,
            values: vec![],
            extra: None
        })
    );

    let mut b = B::new().u32(10);
    for i in 0..10 {
        b = b.f32(i as f32);
    }
    r_data = b
        .f32(9.5)
        .u8(1)
        .u32(2)
        .f32(1.0)
        .f32(2.0)
        .f32(3.0)
        .f32(4.0)
        .build();
    let mut r = Reader::new(&r_data);
    let PropertyValue::Waveform(w) = PropertyValue::read(&mut r, PropertyType::Waveform).unwrap()
    else {
        panic!("not a waveform")
    };
    assert_eq!(w.values.len(), 10);
    let extra = w.extra.unwrap();
    assert_eq!(extra.pairs, vec![(1.0, 2.0), (3.0, 4.0)]);
    assert!(extra.flag);
    assert!(r.is_empty());

    let mut b = B::new().u32(5);
    for _ in 0..10 {
        b = b.f32(1.0);
    }
    let data = b.build();
    let mut r = Reader::new(&data);
    PropertyValue::read(&mut r, PropertyType::Waveform).unwrap();
    assert!(r.is_empty());
}

#[test]
fn property_dictionary_layout_lookups_and_children() {
    let data = dictionary(
        |b| {
            let b = def(b, 0x1000_0001, 9, 0, FLAG_HAS_DEFAULT, |b| b.f32(1.0));
            // Types 10, 11, 12 never store values even with the flags set.
            let b = def(b, 0x1000_0002, 10, 0x3400_0000, ALL, |b| b);
            let b = def(b, 0x1000_0003, 11, 0, ALL, |b| b);
            // Array with one child and a subset list.
            b.u32(0x1000_0004)
                .u32(0x1000_0004)
                .u32(12)
                .u32(0)
                .u32(4)
                .u32(0)
                .u32(0)
                .u32(0)
                .u32(0x3FC0_0000)
                .bytes(&[2, 2, 4, 0, 1])
                .vle(1)
                .u32(0x1000_0001)
                .u32(0x1000_0001)
                .u32(1)
                .u32(0x1000_0001)
                .u32(0)
        },
        4,
        &[
            (0, "Invalid"),
            (0x1000_0001, "Speed"),
            (0x1000_0002, "Ref"),
            (0x1000_0003, "Thing"),
            (0x1000_0004, "Things"),
        ],
    );
    let d = PropertyDictionary::parse(&data).unwrap();
    assert_eq!(d.names().len(), 5);
    assert_eq!(d.definitions().len(), 4);
    assert_eq!(d.name(0x1000_0001), Some("Speed"));
    assert_eq!(d.pid("Things"), Some(0x1000_0004));
    assert_eq!(
        d.definition(0x1000_0001).unwrap().default,
        Some(PropertyValue::Float(1.0))
    );
    let pid_ref = d.definition(0x1000_0002).unwrap();
    assert_eq!(pid_ref.data, Did(0x3400_0000));
    assert!(pid_ref.default.is_none());
    let arr = d.definition(0x1000_0004).unwrap();
    assert_eq!(arr.ty, PropertyType::Array);
    assert_eq!(arr.children, vec![0x1000_0001]);
    assert_eq!(arr.subset, vec![0x1000_0001]);
    assert_eq!(arr.field_d, [2, 2, 4, 0, 1]);
    assert!(d.definition(0).is_none());
}

#[test]
fn property_dictionary_rejects_bad_marker_echo_and_unsupported_type() {
    let names = [(0x1000_0001, "P")];
    let bad_marker = {
        let mut d = dictionary(|b| def(b, 0x1000_0001, 9, 0, 0, |b| b), 1, &names);
        let pos = d.len() - (4 + 4 + 1 + 5 + 4);
        d[pos..pos + 4].copy_from_slice(&0x3F80_0000u32.to_le_bytes());
        d
    };
    assert!(matches!(
        PropertyDictionary::parse(&bad_marker),
        Err(FormatError::Malformed { .. })
    ));

    let bad_echo = dictionary(
        |b| {
            b.u32(0x1000_0001)
                .u32(0x1000_0002)
                .bytes(&[0; 24])
                .u32(0x3FC0_0000)
                .bytes(&[0; 5])
                .vle(0)
                .u32(0)
                .u32(0)
        },
        1,
        &names,
    );
    assert!(matches!(
        PropertyDictionary::parse(&bad_echo),
        Err(FormatError::Malformed { .. })
    ));

    let position = dictionary(
        |b| def(b, 0x1000_0001, 17, 0, FLAG_HAS_DEFAULT, |b| b.u32(0)),
        1,
        &names,
    );
    assert_eq!(
        PropertyDictionary::parse(&position).unwrap_err(),
        FormatError::UnsupportedPropertyType { code: 17 }
    );
}

// ---------------------------------------------------------------- enum mappers

fn mapper(did: u32, base: u32, names: &[(u32, &str)], displays: &[(u32, &str)]) -> Vec<u8> {
    let mut b = B::new().u32(did).u32(base).tsize(0, names.len() as u32);
    for (k, n) in names {
        b = b.u32(*k).pascal(n);
    }
    b = b.tsize(0, displays.len() as u32);
    for (k, d) in displays {
        b = b.u32(*k).si_literal(d);
    }
    b.build()
}

#[test]
fn enum_mapper_and_base_chain() {
    let base = EnumMapper::parse(&mapper(
        0x2300_0001,
        0,
        &[(1, "One"), (2, "Two")],
        &[(1, "one")],
    ))
    .unwrap();
    let child = EnumMapper::parse(&mapper(0x2300_0002, 0x2300_0001, &[(3, "Three")], &[])).unwrap();
    assert_eq!(child.base, Did(0x2300_0001));
    assert_eq!(child.name(3), Some("Three"));
    assert_eq!(child.name(1), None);
    let fetch = |did: Did| (did == base.did).then_some(&base);
    assert_eq!(child.resolve(fetch, |m| m.name(2)), Some("Two"));
    assert_eq!(child.resolve(fetch, |m| m.name(9)), None);
    let disp = child.resolve(fetch, |m| m.display(1)).unwrap();
    assert_eq!(disp.source, StringSource::Literal("one".into()));

    // A self-referencing base chain terminates.
    let cyclic = EnumMapper::parse(&mapper(0x2300_0003, 0x2300_0003, &[], &[])).unwrap();
    let c2 = cyclic.clone();
    assert_eq!(cyclic.resolve(|_| Some(&c2), |m| m.name(1)), None);
}

// ---------------------------------------------------------------- DID maps

fn did_map_group(b: B, entries: &[(u32, u32, &str)]) -> B {
    let mut b = b.tsize(1, entries.len() as u32);
    for (k, v, _) in entries {
        b = b.u32(*k).u32(*v);
    }
    b = b.tsize(1, entries.len() as u32);
    for (k, _, l) in entries {
        b = b.u32(*k).short(l);
    }
    b
}

#[test]
fn did_map_merges_groups() {
    let b = B::new().u32(0x2800_0000);
    let b = did_map_group(
        b,
        &[
            (0, 0, "UNDEFINED"),
            (1, 0x2800_0004, "STRINGTABLE"),
            (2, 0x2800_0001, "EMAPPER"),
        ],
    );
    let b = did_map_group(b, &[(9, 0x2800_000B, "FONT")]);
    let m = DataIdMap::parse(&b.build()).unwrap();
    assert_eq!(m.groups().len(), 2);
    assert_eq!(m.entries().len(), 4);
    assert_eq!(m.by_label("STRINGTABLE"), Some(Did(0x2800_0004)));
    assert_eq!(m.by_label("FONT"), Some(Did(0x2800_000B)));
    assert_eq!(m.get(9).unwrap().group, 1);
    assert_eq!(m.by_label("missing"), None);
}

#[test]
fn did_map_mismatched_counts_and_empty_groups() {
    let data = B::new()
        .u32(0x2800_0001)
        .tsize(1, 1)
        .u32(1)
        .u32(0x2300_0001)
        .tsize(1, 0)
        .build();
    assert!(matches!(
        DataIdMap::parse(&data),
        Err(FormatError::Malformed { .. })
    ));
    let data = B::new()
        .u32(0x2800_0001)
        .tsize(1, 0)
        .tsize(1, 0)
        .tsize(1, 0)
        .tsize(1, 0)
        .build();
    let m = DataIdMap::parse(&data).unwrap();
    assert_eq!(m.groups().len(), 2);
    assert!(m.entries().is_empty());
    // A DID with no group at all is truncated.
    assert!(DataIdMap::parse(&0x2800_0001u32.to_le_bytes()).is_err());
}
