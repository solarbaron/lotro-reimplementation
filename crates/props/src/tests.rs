// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests over a synthetic install built by [`crate::testkit`].

use std::sync::Arc;

use tessera_formats::{
    BitfieldAny, Did, Position, PropertyType, StringSource, Waveform, WaveformExtra,
};

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::testkit::{self, Def};
use crate::{
    EnumSource, GameData, Label, MemorySource, Property, PropertyRegistry, PropertySet, PropsError,
    Value,
};
use crate::{RecordSource, SourceError};

const NAME: u32 = 0x1000_0001;
const LEVEL: u32 = 0x1000_0002;
const CLASS: u32 = 0x1000_0003;
const FLAGS: u32 = 0x1000_0004;
const REGION: u32 = 0x1000_0005;
const ICON: u32 = 0x1000_0006;
const REF: u32 = 0x1000_0007;
const STATS: u32 = 0x1000_0008;
const STAT: u32 = 0x1000_0009;
const LIST: u32 = 0x1000_000A;
const WIDE: u32 = 0x1000_000B;
const TEXT: u32 = 0x1000_000C;
const BIG: u32 = 0x1000_000D;
const LOST: u32 = 0x1000_000E;
const LOW: u32 = 0x0000_000F;
const TOKEN: u32 = 0x1000_0010;
const WAVE: u32 = 0x1000_0011;
const TIME: u32 = 0x1000_0012;
const TRI: u32 = 0x1000_0013;
const VEC: u32 = 0x1000_0014;
const INST: u32 = 0x1000_0015;
const COLOUR: u32 = 0x1000_0016;
const POS: u32 = 0x1000_0017;
const I64: u32 = 0x1000_0018;
const OPEN_STRUCT: u32 = 0x1000_0019;
const CYCLIC: u32 = 0x1000_001A;

const BASE_MAPPER: u32 = 0x2300_0001;
const CLASS_MAPPER: u32 = 0x2300_0002;

fn defs() -> Vec<Def> {
    vec![
        Def::new(NAME, "Name", 1),
        Def::new(LEVEL, "Level", 15),
        Def::new(CLASS, "Class", 8).data(CLASS_MAPPER),
        Def::new(FLAGS, "Flags", 18).data(CLASS_MAPPER),
        Def::new(REGION, "Region", 8).data(0x2800_0008),
        Def::new(ICON, "Icon", 20).data(0x05),
        Def::new(REF, "Ref", 10).data(0x3400_0000),
        Def::new(STATS, "Stats", 11).children(&[STAT]),
        Def::new(STAT, "Stat", 9),
        Def::new(LIST, "List", 12).children(&[LEVEL]),
        Def::new(WIDE, "Wide", 22).data(CLASS_MAPPER),
        Def::new(TEXT, "Text", 13),
        Def::new(BIG, "Big", 14).data(CLASS_MAPPER),
        Def::new(LOST, "Lost", 8).data(0x2300_0099),
        Def::new(LOW, "Low", 21),
        Def::new(TOKEN, "Token", 2),
        Def::new(WAVE, "Wave", 3),
        Def::new(TIME, "Time", 4),
        Def::new(TRI, "Tri", 5),
        Def::new(VEC, "Vec", 6),
        Def::new(INST, "Inst", 7),
        Def::new(COLOUR, "Colour", 16),
        Def::new(POS, "Pos", 17),
        Def::new(I64, "I64", 19),
        Def::new(OPEN_STRUCT, "Open", 11),
        Def::new(CYCLIC, "Cyclic", 8).data(0x2300_0010),
    ]
}

fn sample_set() -> PropertySet {
    let stats = PropertySet::from_properties([Property::new(STAT, Value::Float(2.5))]);
    let open = PropertySet::from_properties([
        Property::new(NAME, Value::from("inner")),
        Property::new(CLASS, Value::Enum(3)),
    ]);
    PropertySet::from_properties([
        Property::new(NAME, Value::from("Synthetic é")),
        Property::new(LEVEL, Value::Int(-7)),
        Property::new(CLASS, Value::Enum(1)),
        Property::new(FLAGS, Value::Bitfield32(0b101)),
        Property::new(REGION, Value::Enum(1)),
        Property::new(ICON, Value::DidRef(Did(0x4100_0001))),
        Property::new(REF, Value::PropertyId(LEVEL)),
        Property::new(STATS, Value::from(stats)),
        Property::new(
            LIST,
            Value::Array(vec![
                Property::new(LEVEL, Value::Int(5)),
                Property::new(LEVEL, Value::Int(6)),
            ]),
        ),
        Property::new(
            WIDE,
            Value::BitfieldAny(BitfieldAny {
                bits: 10,
                bytes: vec![0b0000_0100, 0b0000_0010],
            }),
        ),
        Property::new(TEXT, Value::from(testkit::literal("hello"))),
        Property::new(BIG, Value::Bitfield64(1 << 1)),
        Property::new(LOST, Value::Enum(4)),
        Property::new(LOW, Value::Bool(true)),
        Property::new(TOKEN, Value::StringToken(42)),
        Property::new(
            WAVE,
            Value::Waveform(Box::new(Waveform {
                subtype: 10,
                values: (0..10).map(|i| i as f32).collect(),
                extra: Some(WaveformExtra {
                    scalar: 0.5,
                    flag: true,
                    pairs: vec![(1.0, 2.0)],
                }),
            })),
        ),
        Property::new(TIME, Value::Timestamp(1.25)),
        Property::new(TRI, Value::TriState(2)),
        Property::new(VEC, Value::Vector3([1.0, -2.0, 3.0])),
        Property::new(INST, Value::InstanceId(u64::MAX)),
        Property::new(COLOUR, Value::Colour([1, 2, 3, 0xFF])),
        Property::new(
            POS,
            Value::Position(
                Position::new(0x80)
                    .with_region(1)
                    .with_landblock(10, 20)
                    .with_cell(0)
                    .with_offset([1.0, 2.0, 3.0])
                    .with_orientation([1.0, 0.0, 0.0, 0.0]),
            ),
        ),
        Property::new(I64, Value::Int64(-(1 << 40))),
        Property::new(OPEN_STRUCT, Value::from(open)),
        Property::new(CYCLIC, Value::Enum(1)),
    ])
}

fn install() -> MemorySource {
    MemorySource::new()
        .with(0x3400_0000u32, testkit::dictionary(&defs()))
        .with(
            BASE_MAPPER,
            testkit::enum_mapper(
                BASE_MAPPER,
                0,
                &[(1, "BASE_ONE"), (2, "BASE_TWO"), (5, "BASE_FIVE")],
                &[(1, "Base One")],
            ),
        )
        .with(
            CLASS_MAPPER,
            testkit::enum_mapper(
                CLASS_MAPPER,
                BASE_MAPPER,
                &[(1, "ONE"), (3, "THREE")],
                &[(3, "Three")],
            ),
        )
        .with(
            0x2300_00F0u32,
            testkit::enum_mapper(0x2300_00F0, 0, &[(0x05, "RenderSurface")], &[]),
        )
        .with(
            0x2300_007Du32,
            testkit::enum_mapper(0x2300_007D, 0x2300_00F0, &[], &[]),
        )
        .with(
            0x2300_0010u32,
            testkit::enum_mapper(0x2300_0010, 0x2300_0011, &[], &[]),
        )
        .with(
            0x2300_0011u32,
            testkit::enum_mapper(0x2300_0011, 0x2300_0010, &[], &[]),
        )
        .with(
            0x2800_0000u32,
            testkit::did_map(
                0x2800_0000,
                &[
                    (0, 0, "UNDEFINED"),
                    (7, 0x2800_0001, "EMAPPER"),
                    (0x11, 0x2800_0008, "REGION"),
                    (9, 0x2800_0099, "DANGLING"),
                ],
                &[(0x20, 0x2800_0024, "OTHER_PLATFORM")],
            ),
        )
        .with(
            0x2800_0001u32,
            testkit::did_map(
                0x2800_0001,
                &[(0, 0, "UNDEFINED"), (1, CLASS_MAPPER, "Classes")],
                &[
                    (2, 0x2300_007D, "DBOType"),
                    (3, 0x2300_0077, "PropertyType"),
                ],
            ),
        )
        .with(
            0x2800_0008u32,
            testkit::did_map(
                0x2800_0008,
                &[(0, 0, "UNDEFINED"), (1, 0x0F00_0001, "Region1")],
                &[],
            ),
        )
        .with(
            0x7900_0001u32,
            testkit::property_set_record(0x7900_0001, &sample_set()),
        )
        .with(0x7000_0001u32, vec![0x01, 0x00, 0x00, 0x70])
        .with(0x7000_0002u32, vec![0x02, 0x00, 0x00, 0x70])
        .with(0x7600_0001u32, vec![0x01, 0x00, 0x00, 0x76])
        .with(
            0x7800_0005u32,
            testkit::property_set_record(
                0x7800_0005,
                &PropertySet::from_properties([Property::new(LEVEL, Value::Int(1))]),
            ),
        )
}

fn data() -> GameData<MemorySource> {
    GameData::new(install())
}

#[test]
fn facade_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<GameData>();
    check::<GameData<MemorySource>>();
    check::<GameData<Arc<tessera_dat::ArchiveSet>>>();
    check::<PropertySet>();
    check::<PropsError>();
}

#[test]
fn registry_lookups() {
    let gd = data();
    let reg = gd.registry().unwrap();
    assert_eq!(reg.len(), defs().len());
    assert_eq!(reg.name(LEVEL), Some("Level"));
    assert_eq!(reg.pid("Level"), Some(LEVEL));
    assert_eq!(reg.pid("Nope"), None);
    assert_eq!(reg.property_type(STATS), Some(PropertyType::Struct));
    assert_eq!(reg.property_type(0xDEAD), None);
    assert_eq!(
        reg.enum_source(CLASS),
        Some(EnumSource::Mapper(Did(CLASS_MAPPER)))
    );
    assert_eq!(
        reg.enum_source(REGION),
        Some(EnumSource::DidMap(Did(0x2800_0008)))
    );
    assert_eq!(
        reg.enum_source(FLAGS),
        Some(EnumSource::Mapper(Did(CLASS_MAPPER)))
    );
    assert_eq!(reg.enum_source(LEVEL), None);
    assert_eq!(reg.dbo_type(ICON), Some(5));
    assert_eq!(reg.dbo_type(LEVEL), None);
    assert_eq!(reg.children(STATS), &[STAT]);
    assert!(reg.allows_child(STATS, STAT));
    assert!(!reg.allows_child(STATS, LEVEL));
    assert!(reg.allows_child(OPEN_STRUCT, LEVEL));
    assert_eq!(reg.default_value(LEVEL), None);
    assert_eq!(reg.iter().count(), defs().len());
    // Second call returns the cached instance.
    assert!(Arc::ptr_eq(reg, gd.registry().unwrap()));
}

#[test]
fn every_type_round_trips() {
    let gd = data();
    let set = gd.properties(Did(0x7000_0001)).unwrap().unwrap();
    let expected = sample_set();
    assert_eq!(set.len(), expected.len());
    for p in &expected {
        assert_eq!(set.get(p.pid), Some(&p.value), "pid {:#x}", p.pid);
    }
    let reg = gd.registry().unwrap();
    assert_eq!(
        set.get_by_name(reg, "Name").and_then(Value::as_str),
        Some("Synthetic é")
    );
    assert_eq!(
        set.get_by_name(reg, "Level").and_then(Value::as_i32),
        Some(-7)
    );
    assert_eq!(
        set.get_by_name(reg, "Stats")
            .and_then(Value::as_struct)
            .and_then(|s| s.get_by_name(reg, "Stat"))
            .and_then(Value::as_f32),
        Some(2.5)
    );
    let named: Vec<_> = set.iter_named(reg).map(|(_, n, _)| n).collect();
    assert!(named.iter().all(Option::is_some));
    // Every property type 1-22 is present.
    let mut types: Vec<u32> = Vec::new();
    set.walk(|_, p| types.push(p.value.property_type().code()));
    types.sort_unstable();
    types.dedup();
    assert_eq!(types, (1..=22).collect::<Vec<_>>());
}

#[test]
fn object_rule() {
    let gd = data();
    assert!(gd.properties(Did(0x7000_0001)).unwrap().is_some());
    assert!(gd.object_properties(Did(0x7000_0001)).unwrap().is_some());
    // No 0x79 partner, no 0x7F partner, and 0x77 never maps.
    assert!(gd.properties(Did(0x7000_0002)).unwrap().is_none());
    assert!(gd.properties(Did(0x7600_0001)).unwrap().is_none());
    assert!(gd.properties(Did(0x7700_0001)).unwrap().is_none());
    // Stand-alone set.
    let s = gd.properties(Did(0x7800_0005)).unwrap().unwrap();
    assert_eq!(s.get(LEVEL), Some(&Value::Int(1)));
    assert!(gd.property_set(Did(0x7800_0006)).unwrap().is_none());
    // Wrong prefixes.
    assert!(matches!(
        gd.properties(Did(0x4100_0001)),
        Err(PropsError::NotPropertySource { .. })
    ));
    assert!(matches!(
        gd.property_set(Did(0x7000_0001)),
        Err(PropsError::NotPropertySource { .. })
    ));
    assert!(matches!(
        gd.object_properties(Did(0x7900_0001)),
        Err(PropsError::NotPropertySource { .. })
    ));
}

#[test]
fn enum_labels_follow_base_chain() {
    let gd = data();
    let m = Did(CLASS_MAPPER);
    // Name from the derived mapper, display from the base.
    let l = gd.enum_label(m, 1).unwrap();
    assert_eq!(l.name.as_deref(), Some("ONE"));
    assert!(matches!(
        l.display.as_ref().map(|d| &d.source),
        Some(StringSource::Literal(u)) if u.to_string_lossy() == "Base One"
    ));
    assert_eq!(
        gd.enum_label(m, 2).unwrap().name.as_deref(),
        Some("BASE_TWO")
    );
    let three = gd.enum_label(m, 3).unwrap();
    assert_eq!(three.name.as_deref(), Some("THREE"));
    assert!(three.display.is_some());
    assert!(!gd.enum_label(m, 9).unwrap().is_resolved());
    // Missing mapper: unresolved, not an error.
    assert!(!gd.enum_label(Did(0x2300_0099), 1).unwrap().is_resolved());
    // Cycle terminates.
    assert!(!gd.enum_label(Did(0x2300_0010), 1).unwrap().is_resolved());
}

#[test]
fn labels_per_type() {
    let gd = data();
    let l = gd.label(CLASS, &Value::Enum(3));
    assert_eq!(l.is_resolved(), Some(true));
    assert_eq!(l.to_string(), "THREE");

    match gd.label(FLAGS, &Value::Bitfield32(0b101)) {
        Label::Bits { bits, .. } => {
            let got: Vec<_> = bits
                .iter()
                .map(|b| {
                    (
                        b.bit,
                        b.label.key,
                        b.label.name.as_deref().map(str::to_owned),
                    )
                })
                .collect();
            assert_eq!(
                got,
                vec![
                    (0, 1, Some("ONE".to_owned())),
                    (2, 3, Some("THREE".to_owned()))
                ]
            );
        }
        other => panic!("unexpected {other:?}"),
    }
    let wide = Value::BitfieldAny(BitfieldAny {
        bits: 10,
        bytes: vec![0b100, 0b10],
    });
    // Bit 2 -> key 3 (THREE), bit 9 -> key 10 (unlabelled).
    let l = gd.label(WIDE, &wide);
    assert_eq!(l.is_resolved(), Some(false));
    assert_eq!(l.to_string(), "THREE | bit9");
    assert_eq!(
        gd.label(BIG, &Value::Bitfield64(0b10)).to_string(),
        "BASE_TWO"
    );

    match gd.label(REGION, &Value::Enum(1)) {
        Label::DidMapKey { label, target, .. } => {
            assert_eq!(label.as_deref(), Some("Region1"));
            assert_eq!(target, Some(Did(0x0F00_0001)));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(gd.label(REGION, &Value::Enum(7)).is_resolved(), Some(false));

    assert_eq!(
        gd.label(REF, &Value::PropertyId(LEVEL)).to_string(),
        "Level"
    );
    assert_eq!(
        gd.label(REF, &Value::PropertyId(0)).is_resolved(),
        Some(true)
    );
    assert_eq!(
        gd.label(REF, &Value::PropertyId(0xBAD)).is_resolved(),
        Some(false)
    );

    match gd.label(ICON, &Value::DidRef(Did(0x4100_0001))) {
        Label::DidRef {
            dbo_type, dbo_name, ..
        } => {
            assert_eq!(dbo_type, Some(5));
            assert_eq!(dbo_name.as_deref(), Some("RenderSurface"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(gd.label(LOST, &Value::Enum(4)).is_resolved(), Some(false));
    assert_eq!(gd.label(LEVEL, &Value::Int(4)), Label::None);
    assert_eq!(gd.label(LEVEL, &Value::Int(4)).is_resolved(), None);
    // A value whose PID is not defined has no label source.
    assert_eq!(gd.label(0xDEAD, &Value::Enum(1)), Label::None);
}

#[test]
fn did_map_navigation() {
    let gd = data();
    assert_eq!(gd.root_did_map().unwrap().len(), 5);
    assert_eq!(
        gd.lookup_path(&["EMAPPER", "Classes"]).unwrap(),
        Some(Did(CLASS_MAPPER))
    );
    // Second-group labels are merged (§8.3).
    assert_eq!(
        gd.lookup_path(&["EMAPPER", "DBOType"]).unwrap(),
        Some(Did(0x2300_007D))
    );
    assert_eq!(
        gd.dbo_type_name(5).unwrap().as_deref(),
        Some("RenderSurface")
    );
    assert_eq!(gd.dbo_type_name(6).unwrap(), None);
    // A dangling target gives its DID, but no mapper.
    assert_eq!(
        gd.lookup_path(&["EMAPPER", "PropertyType"]).unwrap(),
        Some(Did(0x2300_0077))
    );
    assert!(gd.enum_mapper_by_name("PropertyType").unwrap().is_none());
    assert!(gd.enum_mapper_by_name("Classes").unwrap().is_some());
    assert_eq!(gd.lookup_path(&["DANGLING", "x"]).unwrap(), None);
    assert_eq!(gd.lookup_path(&["UNDEFINED"]).unwrap(), None);
    assert_eq!(gd.lookup_path(&["NOPE"]).unwrap(), None);
    assert_eq!(gd.lookup_path(&[]).unwrap(), None);
    assert_eq!(gd.lookup_path(&["EMAPPER"; 17]).unwrap(), None);
    // A path through a non-map target is a miss.
    assert_eq!(gd.lookup_path(&["EMAPPER", "Classes", "x"]).unwrap(), None);
    let region = gd.did_map_by_label("REGION").unwrap().unwrap();
    assert_eq!(region.by_label("Region1"), Some(Did(0x0F00_0001)));
    let e = gd.did_map_entry(Did(0x2800_0008), 1).unwrap().unwrap();
    assert_eq!(&*e.label, "Region1");
    assert!(gd.did_map_entry(Did(0x2800_0077), 1).unwrap().is_none());
    // Wrong-kind DIDs are misses, not decode errors.
    assert!(gd.enum_mapper(Did(0x2800_0001)).unwrap().is_none());
    assert!(gd.did_map(Did(CLASS_MAPPER)).unwrap().is_none());
    assert!(gd.cached_records() > 0);
}

#[test]
fn resolve_tree() {
    let gd = data();
    let set = gd.properties(Did(0x7000_0001)).unwrap().unwrap();
    let resolved = gd.resolve(&set).unwrap();
    assert_eq!(resolved.len(), set.len());
    let mut total = 0;
    let mut named = 0;
    for r in &resolved {
        r.walk(|_, p| {
            total += 1;
            named += usize::from(p.name.is_some());
            assert_eq!(p.declared, Some(p.value.property_type()));
        });
    }
    assert_eq!(total, set.value_count());
    assert_eq!(named, total);
    let stats = resolved.iter().find(|r| r.pid == STATS).unwrap();
    assert_eq!(stats.children.len(), 1);
    assert_eq!(stats.children[0].name.as_deref(), Some("Stat"));
    let open = resolved.iter().find(|r| r.pid == OPEN_STRUCT).unwrap();
    assert_eq!(open.children[1].label.to_string(), "THREE");
}

#[test]
fn missing_and_corrupt_dictionary() {
    let gd = GameData::new(MemorySource::new());
    assert!(matches!(gd.registry(), Err(PropsError::Missing { .. })));
    assert_eq!(gd.label(LEVEL, &Value::Int(1)), Label::None);
    assert!(gd.resolve(&PropertySet::new()).is_err());

    let mut bytes = testkit::dictionary(&defs());
    bytes.truncate(bytes.len() / 2);
    let gd = GameData::new(Flaky::new(MemorySource::new().with(0x3400_0000u32, bytes)));
    assert!(matches!(gd.registry(), Err(PropsError::Decode { .. })));
    // A corrupt dictionary is remembered: labelling many values reads it once.
    for _ in 0..1000 {
        assert_eq!(gd.label(CLASS, &Value::Enum(3)), Label::None);
    }
    assert!(matches!(gd.registry(), Err(PropsError::Decode { .. })));
    assert_eq!(gd.source().reads(), 1);

    // A supplied registry skips the record.
    let reg = Arc::new(PropertyRegistry::parse(&testkit::dictionary(&defs())).unwrap());
    let gd = GameData::new(MemorySource::new()).with_registry(reg);
    assert_eq!(gd.registry().unwrap().len(), defs().len());
}

#[test]
fn corrupt_records_are_errors_not_panics() {
    let mut src = install();
    // Mapper stored under the wrong DID.
    src.insert(
        0x2300_0055u32,
        testkit::enum_mapper(0x2300_0056, 0, &[(1, "X")], &[]),
    );
    // Truncated mapper.
    let mut m = testkit::enum_mapper(0x2300_0057, 0, &[(1, "X")], &[]);
    m.pop();
    src.insert(0x2300_0057u32, m);
    // Set with an undefined PID.
    let bad = PropertySet::from_properties([Property::new(0x1000_0FFF, Value::Int(1))]);
    src.insert(
        0x7900_0003u32,
        testkit::property_set_record(0x7900_0003, &bad),
    );
    // Set with trailing bytes.
    let mut trailing = testkit::property_set_record(0x7900_0004, &sample_set());
    trailing.push(0);
    src.insert(0x7900_0004u32, trailing);
    // Set stored under the wrong DID.
    src.insert(
        0x7900_0005u32,
        testkit::property_set_record(0x7900_0006, &sample_set()),
    );
    let gd = GameData::new(src);
    assert!(matches!(
        gd.enum_mapper(Did(0x2300_0055)),
        Err(PropsError::Decode { .. })
    ));
    assert!(gd.enum_mapper(Did(0x2300_0057)).is_err());
    // Cached failure is returned again.
    assert!(gd.enum_mapper(Did(0x2300_0057)).is_err());
    assert!(gd.enum_label(Did(0x2300_0057), 1).is_err());
    for did in [0x7000_0003, 0x7000_0004, 0x7000_0005] {
        assert!(
            matches!(gd.properties(Did(did)), Err(PropsError::Decode { .. })),
            "{did:#x}"
        );
    }
}

#[test]
fn mutated_records_never_panic() {
    let gd = data();
    let base = testkit::property_set_record(0x7900_0001, &sample_set());
    let reg = gd.registry().unwrap();
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..4000 {
        let mut b = base.clone();
        match next() % 3 {
            0 => {
                let n = (next() as usize) % b.len();
                b.truncate(n);
            }
            1 => {
                for _ in 0..4 {
                    let i = (next() as usize) % b.len();
                    b[i] = next() as u8;
                }
            }
            _ => {
                let i = (next() as usize) % b.len();
                b.splice(i..i, [0xFF, 0xFF, 0xFF, 0x7F]);
            }
        }
        if let Ok(set) = PropertySet::parse_record(&b, Did(0x7900_0001), reg) {
            let _ = gd.resolve(&set);
        }
        let _ = PropertySet::parse_embedded(b.get(4..).unwrap_or(&[]), Did(0), reg);
    }
}

#[test]
fn concurrent_use() {
    let gd = data();
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                let set = gd.properties(Did(0x7000_0001)).unwrap().unwrap();
                let r = gd.resolve(&set).unwrap();
                assert_eq!(r.len(), set.len());
            });
        }
    });
}

/// Same properties, in any order (decoded sets come in bucket order).
fn assert_same_props(a: &PropertySet, b: &PropertySet) {
    assert_eq!(a.len(), b.len());
    for p in b {
        assert_eq!(a.get(p.pid), Some(&p.value), "pid {:#x}", p.pid);
    }
}

/// A source that counts reads and fails the first read of chosen DIDs.
struct Flaky {
    inner: MemorySource,
    fail_once: Mutex<HashSet<u32>>,
    reads: AtomicUsize,
}

impl Flaky {
    fn new(inner: MemorySource) -> Self {
        Self {
            inner,
            fail_once: Mutex::new(HashSet::new()),
            reads: AtomicUsize::new(0),
        }
    }

    fn failing(self, dids: &[u32]) -> Self {
        self.fail_once.lock().unwrap().extend(dids);
        self
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl RecordSource for Flaky {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail_once.lock().unwrap().remove(&did.0) {
            return Err("transient I/O error".into());
        }
        self.inner.read_record(did)
    }
}

#[test]
fn failing_base_mapper_keeps_upper_labels() {
    const TOP: u32 = 0x2300_0020;
    const BAD_BASE: u32 = 0x2300_0021;
    let mut src = install();
    src.insert(
        TOP,
        testkit::enum_mapper(TOP, BAD_BASE, &[(1, "ONE")], &[(2, "Two")]),
    );
    let mut bad = testkit::enum_mapper(BAD_BASE, 0, &[(1, "X"), (3, "THREE")], &[]);
    bad.truncate(bad.len() - 1);
    src.insert(BAD_BASE, bad);
    let gd = GameData::new(src);
    // The top mapper's name survives a base that does not decode, like a missing base.
    let l = gd.enum_label(Did(TOP), 1).unwrap();
    assert_eq!(l.name.as_deref(), Some("ONE"));
    assert!(l.display.is_none());
    let two = gd.enum_label(Did(TOP), 2).unwrap();
    assert!(two.name.is_none() && two.display.is_some());
    assert!(!gd.enum_label(Did(TOP), 3).unwrap().is_resolved());
    // The failing mapper itself is still an error when asked for directly.
    assert!(gd.enum_label(Did(BAD_BASE), 1).is_err());

    // Same through `label` for an enum and a bitfield using this mapper.
    let mut d = defs();
    d.push(Def::new(0x1000_0040, "E", 8).data(TOP));
    d.push(Def::new(0x1000_0041, "B", 18).data(TOP));
    let mut src = install().with(0x3400_0000u32, testkit::dictionary(&d));
    src.insert(TOP, testkit::enum_mapper(TOP, BAD_BASE, &[(1, "ONE")], &[]));
    let mut bad = testkit::enum_mapper(BAD_BASE, 0, &[], &[]);
    bad.pop();
    src.insert(BAD_BASE, bad);
    let gd = GameData::new(src);
    let l = gd.label(0x1000_0040, &Value::Enum(1));
    assert_eq!(l.to_string(), "ONE");
    assert_eq!(l.is_resolved(), Some(true));
    assert_eq!(
        gd.label(0x1000_0041, &Value::Bitfield32(0b11)).to_string(),
        "ONE | bit1"
    );
}

#[test]
fn read_errors_are_retried_decode_errors_are_cached() {
    let mut src = install();
    let mut corrupt = testkit::enum_mapper(0x2300_0057, 0, &[(1, "X")], &[]);
    corrupt.pop();
    src.insert(0x2300_0057u32, corrupt);
    let gd = GameData::new(Flaky::new(src).failing(&[CLASS_MAPPER, 0x2800_0008, 0x3400_0000]));

    // Registry: the failed read is not remembered.
    assert!(matches!(gd.registry(), Err(PropsError::Source { .. })));
    assert!(gd.registry().is_ok());

    // Enum mapper: first read fails, the next call reads again and succeeds.
    assert!(matches!(
        gd.enum_mapper(Did(CLASS_MAPPER)),
        Err(PropsError::Source { .. })
    ));
    assert!(gd.enum_mapper(Did(CLASS_MAPPER)).unwrap().is_some());
    assert_eq!(gd.label(CLASS, &Value::Enum(3)).to_string(), "THREE");

    // DID map likewise.
    assert!(gd.did_map(Did(0x2800_0008)).is_err());
    assert!(gd.did_map(Did(0x2800_0008)).unwrap().is_some());

    // A record that does not decode is read once and then answered from the cache.
    let before = gd.source().reads();
    for _ in 0..5 {
        assert!(matches!(
            gd.enum_mapper(Did(0x2300_0057)),
            Err(PropsError::Decode { .. })
        ));
    }
    assert_eq!(gd.source().reads(), before + 1);
    // So is an absent one.
    let before = gd.source().reads();
    for _ in 0..5 {
        assert!(gd.enum_mapper(Did(0x2300_0099)).unwrap().is_none());
    }
    assert_eq!(gd.source().reads(), before + 1);
}

#[test]
fn null_references_need_no_label() {
    // No root DID map at all, so no DB-object type has a name.
    let gd = GameData::new(MemorySource::new().with(0x3400_0000u32, testkit::dictionary(&defs())));
    let null = gd.label(ICON, &Value::DidRef(Did(0)));
    assert!(matches!(
        null,
        Label::DidRef {
            dbo_type: Some(5),
            dbo_name: None,
            ..
        }
    ));
    assert_eq!(null.is_resolved(), Some(true));
    assert_eq!(
        gd.label(ICON, &Value::DidRef(Did(0x4100_0001)))
            .is_resolved(),
        Some(false)
    );
}

#[test]
fn pid_zero_is_none_even_though_the_dictionary_names_it() {
    let gd = GameData::new(install().with(
        0x3400_0000u32,
        testkit::dictionary_with_names(&defs(), &[(0, "Invalid")]),
    ));
    assert_eq!(gd.registry().unwrap().name(0), Some("Invalid"));
    let l = gd.label(REF, &Value::PropertyId(0));
    assert_eq!(l, Label::Property { pid: 0, name: None });
    assert_eq!(l.to_string(), "none");
    assert_eq!(l.is_resolved(), Some(true));
    assert_eq!(
        gd.label(REF, &Value::PropertyId(LEVEL)).to_string(),
        "Level"
    );
}

#[test]
fn embedded_set_followed_by_other_data() {
    let gd = data();
    let reg = gd.registry().unwrap();
    let rec = testkit::property_set_record(0x7900_0001, &sample_set());
    let embedded = &rec[4..];
    let mut buf = embedded.to_vec();
    buf.extend_from_slice(&[0xAB, 0xCD, 0xEF]);
    let (set, used) = PropertySet::parse_embedded_prefix(&buf, Did(0x4400_0001), reg).unwrap();
    assert_eq!(used, embedded.len());
    assert_same_props(&set, &sample_set());
    // The exact-length form rejects the trailing bytes.
    assert!(PropertySet::parse_embedded(&buf, Did(0x4400_0001), reg).is_err());
    // Reader form: the reader is left on the next field.
    let mut r = tessera_formats::Reader::new(&buf);
    PropertySet::read_embedded(&mut r, Did(0x4400_0001), reg).unwrap();
    assert_eq!(r.remaining(), 3);
    // Truncated: an error, not a panic.
    assert!(PropertySet::parse_embedded_prefix(&buf[..10], Did(0), reg).is_err());
}

#[test]
fn records_with_trailing_bytes_decode_leniently() {
    let mut src = install();
    let mut trailing = testkit::property_set_record(0x7900_0004, &sample_set());
    trailing.extend_from_slice(&[0, 0]);
    src.insert(0x7900_0004u32, trailing.clone());
    let gd = GameData::new(src);
    assert!(gd.property_set(Did(0x7900_0004)).is_err());
    let (set, extra) = gd.property_set_prefix(Did(0x7900_0004)).unwrap().unwrap();
    assert_eq!(extra, 2);
    assert_same_props(&set, &sample_set());
    let (_, extra) = gd.property_set_prefix(Did(0x7900_0001)).unwrap().unwrap();
    assert_eq!(extra, 0);
    assert!(gd.property_set_prefix(Did(0x7900_0009)).unwrap().is_none());
    assert!(matches!(
        gd.property_set_prefix(Did(0x7000_0001)),
        Err(PropsError::NotPropertySource { .. })
    ));
    // The DID check still applies.
    let reg = gd.registry().unwrap();
    assert!(matches!(
        PropertySet::parse_record_prefix(&trailing, Did(0x7900_0005), reg),
        Err(PropsError::Decode { .. })
    ));
}

#[test]
fn wide_bitfields_are_labelled_within_bounds() {
    let gd = data();
    let wide = Value::BitfieldAny(BitfieldAny {
        bits: 8 << 20,
        bytes: vec![0xFF; 1 << 20],
    });
    let l = gd.label(WIDE, &wide);
    let Label::Bits { bits, omitted, .. } = &l else {
        panic!("unexpected {l:?}");
    };
    assert_eq!(bits.len(), crate::game_data::MAX_LABELLED_BITS);
    assert_eq!(*omitted, (8u64 << 20) - bits.len() as u64);
    assert_eq!(bits[0].label.name.as_deref(), Some("ONE"));
    assert_eq!(bits[2].label.name.as_deref(), Some("THREE"));
    assert_eq!(l.is_resolved(), Some(false));
    assert!(l.to_string().ends_with(&format!("| +{omitted} more")));

    // Across a whole resolve, the bit labels share one budget.
    let n = crate::game_data::RESOLVE_MAX_BIT_LABELS / 64 + 10;
    let set = PropertySet::from_properties([Property::new(
        LIST,
        Value::Array(
            (0..n)
                .map(|_| Property::new(BIG, Value::Bitfield64(u64::MAX)))
                .collect(),
        ),
    )]);
    let resolved = gd.resolve(&set).unwrap();
    let mut labelled = 0usize;
    let mut omitted = 0u64;
    resolved[0].walk(|_, p| {
        if let Label::Bits {
            bits, omitted: o, ..
        } = &p.label
        {
            labelled += bits.len();
            omitted += o;
        }
    });
    assert_eq!(labelled, crate::game_data::RESOLVE_MAX_BIT_LABELS);
    assert_eq!(labelled as u64 + omitted, 64 * n as u64);
}

#[test]
fn resolved_tree_can_outlive_its_set() {
    let gd = data();
    let set = gd.properties(Did(0x7000_0001)).unwrap().unwrap();
    let borrowed = gd.resolve(&set).unwrap();
    let owned: Vec<crate::ResolvedProperty<'static>> = borrowed
        .clone()
        .into_iter()
        .map(|r| r.into_owned())
        .collect();
    drop(borrowed);
    drop(set);
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for r in &owned {
            r.walk(|d, p| seen.push((d, p.pid, p.value.property_type(), p.label.to_string())));
        }
        (owned, seen)
    });
    let (owned, seen) = handle.join().unwrap();
    let level = owned.iter().find(|r| r.pid == LEVEL).unwrap();
    assert_eq!(*level.value, Value::Int(-7));
    // Containers keep their type but hand their contents to `children`.
    let stats = owned.iter().find(|r| r.pid == STATS).unwrap();
    assert_eq!(stats.value.property_type(), PropertyType::Struct);
    assert!(stats.value.children().is_empty());
    assert_eq!(stats.children[0].name.as_deref(), Some("Stat"));
    assert_eq!(*stats.children[0].value, Value::Float(2.5));
    // Depths come with the walk: the list's elements sit one level down.
    assert!(seen.contains(&(1, LEVEL, PropertyType::Int, String::new())));
    assert_eq!(seen.iter().filter(|s| s.0 == 0).count(), owned.len());
}
