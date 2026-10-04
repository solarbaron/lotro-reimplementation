// SPDX-License-Identifier: GPL-3.0-or-later
//! Property-set decoding with a synthetic schema (`docs/specs/property-system.md` §4–§6,
//! test guidance §14). Every fixture is built here; none comes from game data.

mod common;

use std::collections::HashMap;

use common::B;
use tessera_formats::property::{PROPERTY_MAX_ARRAY_LEN, PROPERTY_MAX_DEPTH};
use tessera_formats::{
    BitfieldAny, Did, EntryContext, FormatError, NoSchema, Position, PropertyEntry, PropertySchema,
    PropertySet, PropertySetRecord, PropertyType, PropertyValue, Reader, decode_property_set,
    decode_property_value, object_property_set_did,
};

const INT: u32 = 0x1000_0BBB;
const STRUCT: u32 = 0x1000_0AAA;
const BOOL: u32 = 0x1000_0CCC;
const ARRAY: u32 = 0x1000_0DDD;
const ELEM: u32 = 0x1000_0123;
const FLOAT: u32 = 0x0000_0042;
const STR: u32 = 0x1000_0001;
/// A struct that nests itself, for depth tests.
const NEST: u32 = 0x1000_0777;

fn schema() -> HashMap<u32, PropertyType> {
    let mut m = HashMap::new();
    m.insert(INT, PropertyType::Int);
    m.insert(STRUCT, PropertyType::Struct);
    m.insert(BOOL, PropertyType::Bool);
    m.insert(ARRAY, PropertyType::Array);
    m.insert(ELEM, PropertyType::Int);
    m.insert(FLOAT, PropertyType::Float);
    m.insert(STR, PropertyType::String);
    m.insert(NEST, PropertyType::Struct);
    // PIDs 1..=22 have the type with the same code, for the per-type cases.
    for code in 1..=22 {
        m.insert(code, PropertyType::from_code(code));
    }
    m
}

/// The worked example of spec §14.1, byte for byte.
fn spec_example() -> Vec<u8> {
    B::new()
        .u32(0x7800_0ABC)
        .tsize(0, 2)
        .u32(INT)
        .u32(INT)
        .u32(7)
        .u32(STRUCT)
        .u32(STRUCT)
        .tsize(0, 1)
        .u32(BOOL)
        .u32(BOOL)
        .u8(1)
        .build()
}

#[test]
fn worked_example_decodes() {
    let data = spec_example();
    assert_eq!(data.len(), 37);
    let rec = PropertySetRecord::parse_expecting(&data, 0x7800_0ABCu32, &schema()).unwrap();
    assert_eq!(rec.did, Did(0x7800_0ABC));
    assert_eq!(rec.set.len(), 2);
    assert_eq!(rec.set.get(INT), Some(&PropertyValue::Int(7)));
    let s = rec.set.get(STRUCT).unwrap().as_struct().unwrap();
    assert_eq!(s.size.hint, 0);
    assert_eq!(s.get(BOOL), Some(&PropertyValue::Bool(true)));
    assert_eq!(rec.set.is_bucket_ordered(), Some(true));
    assert!(rec.set.structs_bucket_ordered());
    assert_eq!(rec.set.value_count(), 3);
    assert_eq!(rec.set.null_entries(), 0);

    // The same through the free function and the dictionary-less embedded form.
    let rec2 = decode_property_set(&data, &schema()).unwrap();
    assert_eq!(rec, rec2);
    let embedded = PropertySet::parse(&data[4..], &schema()).unwrap();
    assert_eq!(embedded, rec.set);
}

#[test]
fn array_example_and_walk() {
    // The §14.1 array "[5, 6]", inside a set.
    let data = B::new()
        .u32(0x7900_0001)
        .tsize(0, 1)
        .u32(ARRAY)
        .u32(ARRAY)
        .u32(2)
        .u32(ELEM)
        .u32(5)
        .u32(ELEM)
        .u32(6)
        .build();
    let rec = PropertySetRecord::parse(&data, &schema()).unwrap();
    let a = rec.set.get(ARRAY).unwrap().as_array().unwrap();
    assert_eq!(a.count, 2);
    assert_eq!(
        a.elements,
        vec![
            PropertyEntry {
                pid: ELEM,
                value: PropertyValue::Int(5)
            },
            PropertyEntry {
                pid: ELEM,
                value: PropertyValue::Int(6)
            },
        ]
    );
    let mut seen = Vec::new();
    rec.set
        .walk(|v| seen.push((v.depth, v.context, v.entry.pid)));
    assert_eq!(
        seen,
        vec![
            (0, EntryContext::TopLevel, ARRAY),
            (1, EntryContext::ArrayElement { parent: ARRAY }, ELEM),
            (1, EntryContext::ArrayElement { parent: ARRAY }, ELEM),
        ]
    );
}

#[test]
fn every_type_code_decodes_in_a_set() {
    type Case = (u32, fn(B) -> B, PropertyValue);
    let cases: Vec<Case> = vec![
        (
            1,
            |b| b.pascal("x\u{e9}"),
            PropertyValue::String("x\u{e9}".into()),
        ),
        (2, |b| b.u32(9), PropertyValue::StringToken(9)),
        (4, |b| b.f64(2.5), PropertyValue::Timestamp(2.5)),
        (5, |b| b.u8(2), PropertyValue::TriState(2)),
        (
            6,
            |b| b.f32(1.0).f32(-2.0).f32(3.0),
            PropertyValue::Vector3([1.0, -2.0, 3.0]),
        ),
        (7, |b| b.u64(u64::MAX), PropertyValue::InstanceId(u64::MAX)),
        (8, |b| b.u32(3), PropertyValue::Enum(3)),
        (9, |b| b.f32(0.5), PropertyValue::Float(0.5)),
        (10, |b| b.u32(INT), PropertyValue::PropertyId(INT)),
        (14, |b| b.u64(1 << 63), PropertyValue::Bitfield64(1 << 63)),
        (15, |b| b.u32(u32::MAX), PropertyValue::Int(-1)),
        (
            16,
            |b| b.bytes(&[1, 2, 3, 0xFF]),
            PropertyValue::Colour([1, 2, 3, 0xFF]),
        ),
        (18, |b| b.u32(6), PropertyValue::Bitfield32(6)),
        (19, |b| b.u64(5), PropertyValue::Int64(5)),
        (20, |b| b.u32(0), PropertyValue::DidRef(Did(0))),
        (21, |b| b.u8(0), PropertyValue::Bool(false)),
    ];
    for (code, write, want) in cases {
        let data = write(B::new().u32(0x7800_0001).tsize(0, 1).u32(code).u32(code)).build();
        let rec = PropertySetRecord::parse(&data, &schema())
            .unwrap_or_else(|e| panic!("type {code}: {e}"));
        assert_eq!(rec.set.get(code), Some(&want), "type {code}");
        assert_eq!(want.property_type().code(), code);
    }
}

#[test]
fn waveform_subtypes() {
    let one = B::new().u32(1).f32(0.25).build();
    let v = decode_property_value(&one, PropertyType::Waveform, &NoSchema).unwrap();
    let PropertyValue::Waveform(w) = v else {
        panic!()
    };
    assert_eq!((w.subtype, w.values.as_slice()), (1, &[0.25][..]));

    let zero = B::new().u32(0).build();
    decode_property_value(&zero, PropertyType::Waveform, &NoSchema).unwrap();

    let mut b = B::new().u32(3);
    for i in 0..10 {
        b = b.f32(i as f32);
    }
    let three = b.build();
    let PropertyValue::Waveform(w) =
        decode_property_value(&three, PropertyType::Waveform, &NoSchema).unwrap()
    else {
        panic!()
    };
    assert_eq!(w.values.len(), 10);
    assert!(w.extra.is_none());

    let mut b = B::new().u32(10);
    for _ in 0..10 {
        b = b.f32(1.0);
    }
    let ten = b.f32(2.0).u8(0).u32(1).f32(3.0).f32(4.0).build();
    let PropertyValue::Waveform(w) =
        decode_property_value(&ten, PropertyType::Waveform, &NoSchema).unwrap()
    else {
        panic!()
    };
    assert_eq!(w.extra.unwrap().pairs, vec![(3.0, 4.0)]);
}

#[test]
fn position_every_flag_bit() {
    // All payload bits plus 0x40 and 0x80, which carry no bytes.
    let data = B::new()
        .u8(0xFF)
        .u8(1)
        .u8(10)
        .u8(20)
        .u16(7)
        .u16(0)
        .f32(1.0)
        .f32(2.0)
        .f32(3.0)
        .f32(1.0)
        .f32(0.0)
        .f32(0.0)
        .f32(0.0)
        .build();
    let v = decode_property_value(&data, PropertyType::Position, &NoSchema).unwrap();
    assert_eq!(
        v,
        PropertyValue::Position(
            Position::new(0xFF)
                .with_region(1)
                .with_landblock(10, 20)
                .with_instance(7)
                .with_cell(0)
                .with_offset([1.0, 2.0, 3.0])
                .with_orientation([1.0, 0.0, 0.0, 0.0])
        )
    );
    let PropertyValue::Position(p) = v else {
        unreachable!()
    };
    assert_eq!(p.flags(), 0xFF);
    assert_eq!(p.region(), Some(1));
    assert_eq!(p.landblock(), Some((10, 20)));
    assert_eq!(p.instance(), Some(7));
    assert_eq!(p.cell(), Some(0));
    assert_eq!(p.offset(), Some([1.0, 2.0, 3.0]));
    assert_eq!(p.orientation(), Some([1.0, 0.0, 0.0, 0.0]));
    // Flag 0 is a position with no fields; 0xC0 likewise has no payload.
    for flags in [0u8, 0x40, 0x80, 0xC0] {
        let v = decode_property_value(&[flags], PropertyType::Position, &NoSchema).unwrap();
        assert_eq!(v, PropertyValue::Position(Position::new(flags)));
        let PropertyValue::Position(p) = v else {
            unreachable!()
        };
        assert_eq!(
            (p.region(), p.landblock(), p.instance(), p.cell()),
            (None, None, None, None)
        );
        assert_eq!((p.offset(), p.orientation()), (None, None));
    }
    // One bit at a time.
    let sizes = [
        (0x01u8, 1usize),
        (0x02, 2),
        (0x04, 2),
        (0x08, 2),
        (0x10, 12),
        (0x20, 16),
    ];
    for (bit, size) in sizes {
        let mut data = vec![bit];
        data.resize(1 + size, 0);
        decode_property_value(&data, PropertyType::Position, &NoSchema)
            .unwrap_or_else(|e| panic!("bit {bit:#x}: {e}"));
        assert!(
            decode_property_value(&data[..size], PropertyType::Position, &NoSchema).is_err(),
            "bit {bit:#x} truncated"
        );
    }
}

#[test]
fn variable_bitfield() {
    let data = B::new().vle(10).bytes(&[0b1000_0001, 0b10]).build();
    let v = decode_property_value(&data, PropertyType::BitfieldAny, &NoSchema).unwrap();
    let PropertyValue::BitfieldAny(bf) = v else {
        panic!()
    };
    assert_eq!(
        bf,
        BitfieldAny {
            bits: 10,
            bytes: vec![0x81, 0x02]
        }
    );
    assert_eq!(bf.set_bits().collect::<Vec<_>>(), vec![0, 7, 9]);
    assert!(!bf.bit(10));
    let empty = decode_property_value(&[0], PropertyType::BitfieldAny, &NoSchema).unwrap();
    assert_eq!(empty, PropertyValue::BitfieldAny(BitfieldAny::default()));
    assert!(decode_property_value(&[16, 0], PropertyType::BitfieldAny, &NoSchema).is_err());
}

#[test]
fn string_info_values() {
    let data = B::new().si_ref(0x10, 0x2500_0001).build();
    let v = decode_property_value(&data, PropertyType::StringInfo, &NoSchema).unwrap();
    assert!(matches!(v, PropertyValue::StringInfo(_)));
}

#[test]
fn struct_hints_order_and_open_fields() {
    // An open struct may hold any PID, but its type must be known: PID 23 is not in the
    // schema, so decoding stops.
    let data = B::new().u8(1).vle(1).u32(23).u32(23).u8(0).build();
    assert!(matches!(
        decode_property_value(&data, PropertyType::Struct, &schema()),
        Err(FormatError::UnknownProperty { pid: 23, offset: 2 })
    ));

    let ordered = B::new()
        .u8(1)
        .vle(3)
        .u32(21)
        .u32(21)
        .u8(1)
        .u32(FLOAT)
        .u32(FLOAT)
        .f32(1.0)
        .u32(INT)
        .u32(INT)
        .u32(0)
        .build();
    // Hint 1 has 23 buckets: 21 % 23 = 21, then 0x42 % 23 = 20, so this is NOT in bucket
    // order. It still decodes; order is reported, not enforced.
    let v = decode_property_value(&ordered, PropertyType::Struct, &schema()).unwrap();
    let s = v.as_struct().unwrap();
    assert_eq!(s.size.hint, 1);
    assert_eq!(s.fields.len(), 3);
    assert_eq!(s.is_bucket_ordered(), Some(false));

    let ordered = B::new()
        .u8(1)
        .vle(2)
        .u32(FLOAT)
        .u32(FLOAT)
        .f32(1.0)
        .u32(21)
        .u32(21)
        .u8(1)
        .build();
    let v = decode_property_value(&ordered, PropertyType::Struct, &schema()).unwrap();
    assert_eq!(v.as_struct().unwrap().is_bucket_ordered(), Some(true));
    assert_eq!(v.children().len(), 2);
}

#[test]
fn arrays_empty_mixed_and_oversized() {
    let empty = B::new().u32(0).build();
    let v = decode_property_value(&empty, PropertyType::Array, &NoSchema).unwrap();
    assert_eq!(v.as_array().unwrap().elements.len(), 0);

    let mixed = B::new()
        .u32(3)
        .u32(ELEM)
        .u32(1)
        .u32(FLOAT)
        .f32(2.0)
        .u32(ELEM)
        .u32(1)
        .build();
    let v = decode_property_value(&mixed, PropertyType::Array, &schema()).unwrap();
    let pids: Vec<u32> = v.children().iter().map(|e| e.pid).collect();
    assert_eq!(pids, vec![ELEM, FLOAT, ELEM]);

    // A count far beyond the remaining bytes is rejected before any allocation.
    let huge = B::new().u32(900_000).u32(ELEM).u32(1).build();
    assert!(matches!(
        decode_property_value(&huge, PropertyType::Array, &schema()),
        Err(FormatError::Truncated { .. })
    ));
    let over_cap = B::new().u32(PROPERTY_MAX_ARRAY_LEN + 1).build();
    assert!(matches!(
        decode_property_value(&over_cap, PropertyType::Array, &schema()),
        Err(FormatError::Malformed { .. })
    ));
}

fn one_entry_set(pid: u32, echo: u32, value: &[u8]) -> Vec<u8> {
    B::new()
        .u32(0x7800_0001)
        .tsize(0, 1)
        .u32(pid)
        .u32(echo)
        .bytes(value)
        .build()
}

#[test]
fn malformed_sets_are_rejected() {
    let s = schema();
    // Echo mismatch.
    let bad = one_entry_set(INT, INT + 1, &[0; 4]);
    assert!(matches!(
        PropertySetRecord::parse(&bad, &s),
        Err(FormatError::Malformed { .. })
    ));
    // Unknown PID.
    let unknown = one_entry_set(0x1000_9999, 0x1000_9999, &[0; 4]);
    assert_eq!(
        PropertySetRecord::parse(&unknown, &s).unwrap_err(),
        FormatError::UnknownProperty {
            pid: 0x1000_9999,
            offset: 6
        }
    );
    // Trailing bytes, also visible through parse_prefix.
    let mut trailing = one_entry_set(INT, INT, &[1, 0, 0, 0]);
    trailing.extend_from_slice(&[0xAA, 0xBB]);
    assert!(matches!(
        PropertySetRecord::parse(&trailing, &s),
        Err(FormatError::TrailingBytes { count: 2, .. })
    ));
    let (rec, left) = PropertySetRecord::parse_prefix(&trailing, &s).unwrap();
    assert_eq!((rec.set.len(), left), (1, 2));
    // Wrong prefix.
    let mut wrong = spec_example();
    wrong[3] = 0x70;
    assert!(matches!(
        PropertySetRecord::parse(&wrong, &s),
        Err(FormatError::WrongRecordType { .. })
    ));
    // DID mismatch.
    assert!(matches!(
        PropertySetRecord::parse_expecting(&spec_example(), 0x7800_0001u32, &s),
        Err(FormatError::DidMismatch { .. })
    ));
    // Duplicate top-level PID.
    let dup = B::new()
        .u32(0x7800_0001)
        .tsize(0, 2)
        .u32(INT)
        .u32(INT)
        .u32(1)
        .u32(INT)
        .u32(INT)
        .u32(2)
        .build();
    assert!(matches!(
        PropertySetRecord::parse(&dup, &s),
        Err(FormatError::Malformed { .. })
    ));
    // Bool that is not 0/1, attributed to its property.
    let bad_bool = one_entry_set(BOOL, BOOL, &[2]);
    let err = PropertySetRecord::parse(&bad_bool, &s).unwrap_err();
    assert!(matches!(
        err,
        FormatError::PropertyValue {
            pid: BOOL,
            type_code: 21,
            offset: 14,
            ..
        }
    ));
    assert!(matches!(err.root_cause(), FormatError::InvalidBool { .. }));
    // Truncation inside a nested value is attributed to the innermost property.
    let nested_trunc = B::new()
        .u32(0x7800_0001)
        .tsize(0, 1)
        .u32(STRUCT)
        .u32(STRUCT)
        .tsize(0, 1)
        .u32(INT)
        .u32(INT)
        .u16(0)
        .build();
    let err = PropertySetRecord::parse(&nested_trunc, &s).unwrap_err();
    assert!(
        matches!(err, FormatError::PropertyValue { pid: INT, .. }),
        "{err}"
    );
    // Unknown type code in the schema.
    let odd = |pid: u32| (pid == 0x99).then_some(PropertyType::Unknown(99));
    let err = PropertySetRecord::parse(&one_entry_set(0x99, 0x99, &[0]), &odd).unwrap_err();
    assert!(matches!(
        err.root_cause(),
        FormatError::UnsupportedPropertyType { code: 99 }
    ));
}

#[test]
fn null_entries_count_but_are_not_stored() {
    let data = B::new()
        .u32(0x7800_0001)
        .tsize(0, 3)
        .u32(0)
        .u32(INT)
        .u32(INT)
        .u32(4)
        .u32(0)
        .build();
    let rec = PropertySetRecord::parse(&data, &schema()).unwrap();
    assert_eq!(rec.set.len(), 1);
    assert_eq!(rec.set.null_entries(), 2);

    // In an array, a null element has no value either.
    let arr = B::new().u32(2).u32(0).u32(ELEM).u32(9).build();
    let v = decode_property_value(&arr, PropertyType::Array, &schema()).unwrap();
    assert_eq!(v.as_array().unwrap().null_entries(), 1);
}

fn nested_structs(levels: usize) -> Vec<u8> {
    // NEST { NEST { … { INT } } } with `levels` struct levels below the top-level entry.
    let mut b = B::new().u32(0x7800_0001).tsize(0, 1).u32(NEST).u32(NEST);
    for _ in 0..levels {
        b = b.tsize(0, 1).u32(NEST).u32(NEST);
    }
    b.tsize(0, 1).u32(INT).u32(INT).u32(1).build()
}

#[test]
fn depth_limit() {
    // Top-level NEST at depth 0, `levels` nested NESTs, then INT at depth levels + 1.
    let ok = nested_structs(PROPERTY_MAX_DEPTH - 2);
    let rec = PropertySetRecord::parse(&ok, &schema()).unwrap();
    let mut max = 0;
    rec.set.walk(|v| max = max.max(v.depth));
    assert_eq!(max, PROPERTY_MAX_DEPTH - 1);

    let too_deep = nested_structs(PROPERTY_MAX_DEPTH - 1);
    let err = PropertySetRecord::parse(&too_deep, &schema()).unwrap_err();
    assert!(
        matches!(err.root_cause(), FormatError::DepthLimit { .. }),
        "{err}"
    );
}

#[test]
fn schemas_from_closures_and_dictionary_free_values() {
    let by_code = |pid: u32| Some(PropertyType::from_code(pid % 23));
    let data = B::new()
        .u32(0x7800_0001)
        .tsize(0, 1)
        .u32(21)
        .u32(21)
        .u8(1)
        .build();
    let rec = PropertySetRecord::parse(&data, &by_code).unwrap();
    assert_eq!(rec.set.get(21), Some(&PropertyValue::Bool(true)));
    assert_eq!(by_code.property_type(15), Some(PropertyType::Int));

    // PropertyValue::read with no schema handles empty structs and arrays.
    let mut r = Reader::new(&[0, 0]);
    assert!(PropertyValue::read(&mut r, PropertyType::Struct).is_ok());
    let mut r = Reader::new(&[0, 1, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert!(matches!(
        PropertyValue::read(&mut r, PropertyType::Struct),
        Err(FormatError::UnknownProperty { pid: 1, .. })
    ));
}

#[test]
fn object_rule() {
    assert_eq!(
        object_property_set_did(Did(0x7000_00AB)),
        Some(Did(0x7900_00AB))
    );
    assert_eq!(
        object_property_set_did(Did(0x7600_00AB)),
        Some(Did(0x7F00_00AB))
    );
    assert_eq!(object_property_set_did(Did(0x7900_00AB)), None);
}

#[test]
fn hostile_counts_do_not_allocate() {
    // TSize count of 2^30 with no data, at top level and in a struct.
    let top = B::new().u32(0x7800_0001).tsize(0, 0x3FFF_FFFF).build();
    assert!(matches!(
        PropertySetRecord::parse(&top, &schema()),
        Err(FormatError::Truncated { .. })
    ));
    let s = B::new().tsize(0, 0x3FFF_FFFF).build();
    assert!(decode_property_value(&s, PropertyType::Struct, &schema()).is_err());
    // Truncating the worked example at every length fails cleanly.
    let full = spec_example();
    for n in 0..full.len() {
        assert!(
            PropertySetRecord::parse(&full[..n], &schema()).is_err(),
            "len {n}"
        );
    }
}
