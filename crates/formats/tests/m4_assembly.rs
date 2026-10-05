// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for the M4 assembly semantics (`docs/specs/appearance.md`, `setup.md`
//! part 2): block choice, part and group priorities, the key table, the slot table,
//! appearance lists, holding locations, the race table, avatar bodies, bone sliders and
//! worn/held items. Every value is invented.

mod common;

use std::collections::HashMap;
use std::convert::Infallible;

use common::B;
use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceKeyInfo, AppearanceListPids, AppearancePart,
    AppearanceRecord, MaterialGroup, PartSource, SLOT_ALL, SlotTable,
};
use tessera_formats::avatar::{
    AvatarPids, BoneSliderDefinition, BoneSliderPids, RacePids, RaceTable, resolve_avatar_body,
    uniform_scale,
};
use tessera_formats::item::{
    HoldingRule, ItemLook, ItemPids, ItemVisual, WardrobeFallbacks, dye_block,
    npc_cosmetic_inventory_visible, npc_held_items, worn_block,
};
use tessera_formats::setup::{
    Anchor, BoneFrame, Frame, NamedFrame, TemplateAppearance, VisualTemplate,
};
use tessera_formats::{
    Did, FormatError, PropertyArray, PropertyEntry, PropertySet, PropertyStruct, PropertyType,
    PropertyValue, Record, Setup, TSize,
};

// --- property-tree helpers ---

fn entry(pid: u32, value: PropertyValue) -> PropertyEntry {
    PropertyEntry { pid, value }
}

fn strukt(fields: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(PropertyStruct {
        size: TSize {
            hint: 0,
            count: fields.len() as u32,
        },
        fields,
    })
}

fn array(elements: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: elements.len() as u32,
        elements,
    })
}

fn set(entries: Vec<PropertyEntry>) -> PropertySet {
    PropertySet {
        size: TSize {
            hint: 0,
            count: entries.len() as u32,
        },
        entries,
    }
}

fn block(variant: f32, parts: &[(f32, u32, u32)]) -> AppearanceBlock {
    AppearanceBlock {
        variant,
        group_param: 0.0,
        groups: vec![],
        parts: parts
            .iter()
            .map(|&(p, slot, mesh)| AppearancePart {
                part_param: p,
                slot,
                mesh: Did(mesh),
            })
            .collect(),
        extras: vec![],
    }
}

// --- appearance records ---

#[test]
fn block_choice_follows_the_spec_example() {
    // appearance.md §12.2.
    let e = AppearanceEntry {
        key: 1,
        blocks: [0.00, 0.01, 0.02, 0.45, 0.80]
            .iter()
            .map(|&v| block(v, &[]))
            .collect(),
    };
    assert_eq!(e.block_for_modifier(0.02).unwrap().variant, 0.02);
    assert_eq!(e.block_for_modifier(0.019_999_5).unwrap().variant, 0.02);
    assert_eq!(
        e.block_for_modifier(1.0).unwrap().variant,
        0.00,
        "no 1.0 block: first"
    );
    assert_eq!(e.selection_count(), 4);
    assert!(e.base_block().is_none());
    // A base block wins when the modifier finds nothing.
    let mut with_base = e.clone();
    with_base.blocks.push(block(1.0, &[]));
    assert_eq!(with_base.block_for_modifier(0.3).unwrap().variant, 1.0);
    // Dye rule (material.md §12.4): within 0.005, else the largest at or below, else first.
    assert_eq!(e.dye_block(0.453).unwrap().variant, 0.45);
    assert_eq!(e.dye_block(0.6).unwrap().variant, 0.45);
    assert_eq!(e.dye_block(-1.0).unwrap().variant, 0.00);
    assert!(e.dye_block(f32::NAN).is_none());
    let empty = AppearanceEntry::default();
    assert!(empty.block_for_modifier(0.0).is_none());
}

#[test]
fn group_priorities_and_key_table() {
    let g = |slot: u32, next: f32| MaterialGroup {
        shader: Did(0),
        material: Did(0x3000_0001),
        a: slot,
        binding_key: 0x1000_0050,
        g: next,
    };
    // Stored floats: group_param 5, then after each group 5, 7, 0 (appearance.md §4.3).
    let b = AppearanceBlock {
        variant: 1.0,
        group_param: 5.0,
        groups: vec![g(SLOT_ALL, 5.0), g(SLOT_ALL, 7.0), g(0x1000_000C, 0.0)],
        parts: vec![],
        extras: vec![],
    };
    assert_eq!(b.group_priority(0), Some(5.0));
    assert_eq!(b.group_priority(1), Some(5.0));
    assert_eq!(b.group_priority(2), Some(7.0));
    assert_eq!(b.group_priority(3), None);
    assert_eq!(b.groups_end(), 0.0);
    let prios: Vec<f32> = b.groups_with_priority().map(|(p, _)| p).collect();
    assert_eq!(prios, vec![5.0, 5.0, 7.0]);
    assert!(b.groups[0].applies_to(0x1000_0001));
    assert!(b.groups[2].applies_to(0x1000_000C) && !b.groups[2].applies_to(0x1000_0001));
    assert!(b.is_material_only());
    assert_eq!(AppearanceBlock::default().group_priority(0), None);

    let rec = |table: u32, end: f32| AppearanceRecord {
        did: Did(0x2000_0001),
        section: 0,
        entries: vec![AppearanceEntry {
            key: 0x1000_0001,
            blocks: vec![
                block(0.0, &[(1.0, 0x1000_000E, 0x0600_0001)]),
                AppearanceBlock {
                    groups: vec![g(SLOT_ALL, end)],
                    ..block(1.0, &[(10.0, 0x1000_000E, 0x0600_0002)])
                },
            ],
        }],
        key_table: vec![(0x1000_0001, table)],
    };
    assert_eq!(rec(1, 0.0).first_unusual_field(), None);
    assert!(rec(1, 0.0).key_table_matches_selection_counts());
    assert_eq!(
        rec(0, 0.0).first_unusual_field(),
        Some("key-table value differs from the entry's selection count")
    );
    assert_eq!(
        rec(1, 3.0).first_unusual_field(),
        Some("float after the last group is not 0.0")
    );
    // Round trip keeps the attribution-neutral framing.
    let r = rec(1, 0.0);
    assert_eq!(AppearanceRecord::parse(&r.to_bytes()).unwrap(), r);
}

const UPPER: u32 = 0x1000_000C;
const LOWER: u32 = 0x1000_0003;
const HANDS: u32 = 0x1000_0001;
const FEET: u32 = 0x1000_0002;
const CREST: u32 = 0x1000_000E;
const HELMET: u32 = 0x1000_000A;
const CAPE: u32 = 0x1000_000B;

#[test]
fn slot_table_follows_the_spec_example() {
    // appearance.md §12.1: meshes A–M and stubs S0–S4 as DIDs 0x060000xx.
    let (a, b, c, d, e, s0) = (0xA1, 0xB1, 0xC1, 0xD1, 0xE1, 0x50);
    let (f, g, s1, h, s2, j, k, l, m, s3, s4) = (
        0xF1, 0x61, 0x51, 0x71, 0x52, 0x81, 0x91, 0xA2, 0xB2, 0x53, 0x54,
    );
    let hair = block(0.0, &[(1.0, CREST, f)]);
    let head = block(1.0, &[(4.0, HELMET, g), (10.0, CREST, s1)]);
    let chest = block(1.0, &[(1.0, UPPER, h), (10.0, LOWER, s2), (10.0, HANDS, c)]);
    let legs = block(1.0, &[(1.0, LOWER, j)]);
    let gloves = block(1.0, &[(1.0, HANDS, k)]);
    let boots = block(1.0, &[(2.0, FEET, l)]);
    let back = block(
        1.0,
        &[(1.0, CAPE, m), (15.0, CREST, s3), (15.0, HELMET, s4)],
    );
    let build = |worn: &[&AppearanceBlock]| {
        let mut t = SlotTable::from_defaults(
            [
                (UPPER, a),
                (LOWER, b),
                (HANDS, c),
                (FEET, d),
                (CREST, e),
                (HELMET, s0),
            ]
            .map(|(s, m)| (s, Did(m))),
        );
        t.offer_block(&hair, 0);
        for (i, w) in worn.iter().enumerate() {
            t.offer_block(w, i as u32 + 1);
        }
        t
    };
    let mesh = |t: &SlotTable, slot: u32| t.get(slot).unwrap().mesh.0;
    let t = build(&[&head, &chest, &legs, &gloves, &boots, &back]);
    assert_eq!(mesh(&t, UPPER), h);
    assert_eq!(mesh(&t, LOWER), s2);
    assert_eq!(mesh(&t, HANDS), c);
    assert_eq!(
        t.get(HANDS).unwrap().source,
        PartSource::Offered(2),
        "bare hands from the chest"
    );
    assert_eq!(mesh(&t, FEET), l);
    assert_eq!(mesh(&t, CREST), s3);
    assert_eq!(mesh(&t, HELMET), s4);
    assert_eq!(mesh(&t, CAPE), m);
    // Remove the back item, then the head item as well.
    let t = build(&[&head, &chest, &legs, &gloves, &boots]);
    assert_eq!((mesh(&t, CREST), mesh(&t, HELMET)), (s1, g));
    let t = build(&[&chest, &legs, &gloves, &boots]);
    assert_eq!((mesh(&t, CREST), mesh(&t, HELMET)), (f, s0));
    assert_eq!(t.get(HELMET).unwrap().source, PartSource::Default);
    assert_eq!(t.get(HELMET).unwrap().priority, f32::NEG_INFINITY);
    // Ties: the later source wins (§4.7 Decision); NaN never wins.
    let mut t = SlotTable::new();
    assert!(t.offer(HANDS, Did(1), 5.0, 0));
    assert!(t.offer(HANDS, Did(2), 5.0, 1));
    assert!(!t.offer(HANDS, Did(3), 4.0, 2));
    assert!(!t.offer(HANDS, Did(4), f32::NAN, 3));
    assert_eq!(t.get(HANDS).unwrap().mesh, Did(2));
    assert_eq!(t.len(), 1);
    assert_eq!(t.iter().count(), 1);
}

// --- appearance lists and templates ---

const P_KEY: u32 = 0x1000_0201;
const P_FILE: u32 = 0x1000_0202;
const P_MOD: u32 = 0x1000_0203;
const P_TOGGLE_FILE: u32 = 0x1000_0204;
const P_TOGGLE_PROP: u32 = 0x1000_0205;
const P_CLASS_LIST: u32 = 0x1000_0206;
const P_INSTANCE_LIST: u32 = 0x1000_0207;
const P_VISIBLE: u32 = 0x1000_0208;
const P_PHYS_OBJ: u32 = 0x1000_0210;
const P_SCALE: u32 = 0x1000_0211;
const P_SLIDER_FILE: u32 = 0x1000_0212;
const P_SLIDER_VALUES: u32 = 0x1000_0213;
const P_CONTROLS: u32 = 0x1000_0214;

fn list_pids() -> AppearanceListPids {
    AppearanceListPids {
        class_list: Some(P_CLASS_LIST),
        instance_list: Some(P_INSTANCE_LIST),
        key: P_KEY,
        file: P_FILE,
        modifier: Some(P_MOD),
        toggle_file: Some(P_TOGGLE_FILE),
        toggle_property: Some(P_TOGGLE_PROP),
    }
}

fn key_info(key: u32, file: u32, modifier: f32, toggle: Option<u32>) -> PropertyEntry {
    let mut f = vec![
        entry(P_KEY, PropertyValue::Enum(key)),
        entry(P_FILE, PropertyValue::DidRef(Did(file))),
        entry(P_MOD, PropertyValue::Float(modifier)),
    ];
    if let Some(t) = toggle {
        f.push(entry(P_TOGGLE_FILE, PropertyValue::DidRef(Did(t))));
        f.push(entry(P_TOGGLE_PROP, PropertyValue::PropertyId(P_VISIBLE)));
    }
    entry(0x1000_0220, strukt(f))
}

#[test]
fn appearance_lists_read_and_toggle() {
    let names = |n: &str| -> Option<u32> {
        Some(match n {
            "Appearance_Key" => P_KEY,
            "Appearance_AprFile" => P_FILE,
            "Appearance_Modifier" => P_MOD,
            "Appearance_ClassList" => P_CLASS_LIST,
            _ => return None,
        })
    };
    let pids = AppearanceListPids::from_names(&names).unwrap();
    assert_eq!(pids.instance_list, None);
    assert!(matches!(
        AppearanceListPids::from_names(&|_: &str| None),
        Err(FormatError::UnknownName { .. })
    ));

    let s = set(vec![
        entry(
            P_CLASS_LIST,
            array(vec![
                key_info(0x1000_0009, 0x2000_0001, 0.0, Some(0x2000_0101)),
                // An element without a file is skipped.
                entry(
                    0x1000_0220,
                    strukt(vec![entry(P_KEY, PropertyValue::Enum(1))]),
                ),
            ]),
        ),
        entry(
            P_INSTANCE_LIST,
            array(vec![key_info(0x1000_0032, 0x2000_0002, 0.01, None)]),
        ),
        entry(P_VISIBLE, PropertyValue::Bool(true)),
    ]);
    let pids = list_pids();
    let list = pids.read_lists(&s);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].key, 0x1000_0009);
    assert_eq!(list[0].toggle_file, Some(Did(0x2000_0101)));
    assert_eq!(
        list[0].file_in(&s),
        Did(0x2000_0101),
        "the toggle property is true"
    );
    assert_eq!(list[0].file_for(|_| false), Did(0x2000_0001));
    assert_eq!(list[1].modifier, 0.01);
    assert_eq!(list[1].file_in(&s), Did(0x2000_0002));

    let t = TemplateAppearance {
        appearance: Did(0x2000_0003),
        modifiers_size: TSize::default(),
        modifiers: vec![(0x1000_0034, 0.32)],
    };
    let els: Vec<AppearanceKeyInfo> = t.elements().collect();
    assert_eq!(els[0].file, Did(0x2000_0003));
    assert_eq!(els[0].modifier, 0.32);
    assert_eq!(t.modifier(0x1000_0034), Some(0.32));
    let vt = VisualTemplate {
        vec3: Some([1.03; 3]),
        ..VisualTemplate::default()
    };
    assert_eq!(vt.uniform_scale(), Some(1.03));
    let skew = VisualTemplate {
        vec3: Some([1.0, 2.0, 1.0]),
        ..VisualTemplate::default()
    };
    assert_eq!(skew.uniform_scale(), None);
    assert_eq!(uniform_scale(&PropertyValue::Vector3([0.7; 3])), Some(0.7));
    assert_eq!(uniform_scale(&PropertyValue::Float(1.02)), Some(1.02));
    assert_eq!(
        uniform_scale(&PropertyValue::Vector3([1.0, 0.7, 1.0])),
        None
    );
    assert_eq!(uniform_scale(&PropertyValue::Float(-1.0)), None);
}

// --- setups ---

#[test]
fn setup_attachment_tables_are_typed() {
    let frame = Frame {
        position: [0.0, 0.1, 0.0],
        ..Frame::IDENTITY
    };
    let s = Setup {
        attach_map: vec![(0x1000_0010, 18), (0x1000_0020, 79)],
        bone_frames: vec![
            BoneFrame {
                key: 0x1000_003B,
                key2: 0x1000_003B,
                bone: 79,
                frame,
            },
            BoneFrame {
                key: 1,
                key2: 1,
                bone: -1,
                frame: Frame::IDENTITY,
            },
        ],
        named_frames: vec![NamedFrame {
            key: 2,
            key2: 2,
            frame,
        }],
        tagfile: tessera_formats::setup::TAGFILE_MAGIC.to_vec(),
        ..Setup::default()
    };
    assert_eq!(s.first_unusual_field(), None);
    assert_eq!(s.bone_for(0x1000_0020), Some(79));
    assert_eq!(s.bone_for(0x1000_0099), None);
    let h = s.holding_location(0x1000_003B).unwrap();
    assert_eq!(h.anchor, Anchor::Bone(79));
    assert_eq!(h.frame, frame);
    assert_eq!(s.holding_location(1).unwrap().anchor, Anchor::Origin);
    assert!(s.holding_location(7).is_none());
    assert_eq!(s.holding_locations().count(), 2);
    assert_eq!(s.attachments().count(), 2);
    assert_eq!(s.viewport_frame(2), Some(&frame));
    assert_eq!(s.bone_issue(89), None);
    assert_eq!(
        s.bone_issue(62),
        Some("attach_map bone index not below the bone count")
    );

    let mut dup = s.clone();
    dup.attach_map.push((0x1000_0010, 3));
    assert_eq!(
        dup.first_unusual_field(),
        Some("attach_map keys not unique")
    );
    let mut dup = s.clone();
    dup.attach_map.push((0x1000_0030, 18));
    assert_eq!(
        dup.bone_issue(89),
        Some("attach_map bone indices not unique")
    );
    let mut bad = s.clone();
    bad.bone_frames[1].bone = -2;
    assert!(bad.holding_location(1).is_none());
    assert_eq!(bad.bone_issue(89), Some("frame list A bone below -1"));
}

// --- items ---

const P_MAP_LIST: u32 = 0x1000_0300;
const P_WORN: u32 = 0x1000_0301;
const P_ITEM_KEY: u32 = 0x1000_0302;
const P_SPECIES: u32 = 0x1000_0303;
const P_SEX: u32 = 0x1000_0304;
const P_SLOT: u32 = 0x1000_0305;
const P_PARENT: u32 = 0x1000_0306;
const P_PAPERDOLL: u32 = 0x1000_0307;
const P_DYE: u32 = 0x1000_0308;
const HUMAN: u32 = 0x17;
const BEORNING: u32 = 0x72;
const ELF: u32 = 0x41;
const MALE: u32 = 0x1000;
const FEMALE: u32 = 0x2000;

fn item_pids() -> ItemPids {
    ItemPids {
        worn_map_list: Some(P_MAP_LIST),
        worn_appearance: P_WORN,
        appearance_key: P_ITEM_KEY,
        species_of_wearer: P_SPECIES,
        sex_of_wearer: P_SEX,
        phys_obj: Some(P_PHYS_OBJ),
        default_slot: Some(P_SLOT),
        parenting_location: Some(P_PARENT),
        paperdoll_parenting_location: Some(P_PAPERDOLL),
        clothing_color: Some(P_DYE),
        ..ItemPids::default()
    }
}

fn map_struct(record: u32, key: u32, species: u32, sex: u32) -> PropertyEntry {
    entry(
        0x1000_0310,
        strukt(vec![
            entry(P_WORN, PropertyValue::DidRef(Did(record))),
            entry(P_ITEM_KEY, PropertyValue::Enum(key)),
            entry(P_SPECIES, PropertyValue::Enum(species)),
            entry(P_SEX, PropertyValue::Enum(sex)),
        ]),
    )
}

#[test]
fn items_resolve_worn_or_held() {
    let pids = item_pids();
    let fallbacks = WardrobeFallbacks::from_species_labels(|l| match l {
        "Beorning" => Some(BEORNING),
        "Human" => Some(HUMAN),
        "HighElf" => Some(0x75),
        "Elf" => Some(ELF),
        _ => None,
    });
    assert_eq!(fallbacks.pairs, vec![(BEORNING, HUMAN), (0x75, ELF)]);
    assert_eq!(fallbacks.fallback(BEORNING), Some(HUMAN));
    assert_eq!(fallbacks.fallback(HUMAN), None);

    let chest = set(vec![
        entry(
            P_MAP_LIST,
            array(vec![
                map_struct(0x2000_1E54, 0x1000_0700, HUMAN, MALE),
                map_struct(0x2000_1E55, 0x1000_0700, HUMAN, FEMALE),
                map_struct(0x2000_1E59, 0x1000_0700, ELF, MALE),
            ]),
        ),
        entry(P_PHYS_OBJ, PropertyValue::DidRef(Did(0x4700_0001))),
        entry(P_DYE, PropertyValue::Float(0.06)),
    ]);
    let v = ItemVisual::from_property_set(&chest, &pids);
    assert_eq!(v.worn.len(), 3);
    assert_eq!(
        v.worn_for(HUMAN, FEMALE, &fallbacks).unwrap().appearance,
        Did(0x2000_1E55)
    );
    // Beorning borrows the Human structs (§2.3 Decision).
    match v.look(BEORNING, MALE, &fallbacks) {
        ItemLook::Worn { reference, dye, .. } => {
            assert_eq!(reference.appearance, Did(0x2000_1E54));
            assert_eq!(dye, Some(0.06));
        }
        l => panic!("{l:?}"),
    }
    // A wearer with no struct sees nothing, although the item has a PhysObj.
    assert_eq!(v.look(0x51, MALE, &fallbacks), ItemLook::Invisible);

    let sword = set(vec![
        entry(P_PHYS_OBJ, PropertyValue::DidRef(Did(0x4700_0002))),
        entry(P_SLOT, PropertyValue::Bitfield32(0x1_0000)),
    ]);
    let shield = set(vec![
        entry(P_PHYS_OBJ, PropertyValue::DidRef(Did(0x4700_0003))),
        entry(P_SLOT, PropertyValue::Bitfield32(0x2_0000)),
        entry(P_PARENT, PropertyValue::Enum(0x1000_0050)),
        entry(P_PAPERDOLL, PropertyValue::Enum(0x1000_0051)),
    ]);
    let ring = set(vec![entry(P_SLOT, PropertyValue::Bitfield32(0x400))]);
    let (sword, shield, ring) = (
        ItemVisual::from_property_set(&sword, &pids),
        ItemVisual::from_property_set(&shield, &pids),
        ItemVisual::from_property_set(&ring, &pids),
    );
    assert_eq!(
        sword.look(HUMAN, MALE, &fallbacks),
        ItemLook::Held {
            phys_obj: Did(0x4700_0002)
        }
    );
    assert_eq!(ring.look(HUMAN, MALE, &fallbacks), ItemLook::Invisible);
    let rule = HoldingRule {
        slot_locations: vec![(0x1_0000, 0x1000_003B), (0x2_0000, 0x1000_0021)],
    };
    assert_eq!(rule.location(&sword, true), Some(0x1000_003B));
    assert_eq!(rule.location(&shield, true), Some(0x1000_0050));
    assert_eq!(rule.location(&shield, false), Some(0x1000_0051));
    assert_eq!(rule.location(&ring, true), None);
    // A javelin-like ranged item with only an in-world override: the override applies in
    // the world, the slot's paperdoll location in a paperdoll view (`skeleton-anim.md`
    // §8.5), never the in-world override.
    let javelin = ItemVisual::from_property_set(
        &set(vec![
            entry(P_PHYS_OBJ, PropertyValue::DidRef(Did(0x4700_0004))),
            entry(P_SLOT, PropertyValue::Bitfield32(0x4_0000)),
            entry(P_PARENT, PropertyValue::Enum(0x1000_0029)),
        ]),
        &pids,
    );
    let rule = HoldingRule {
        slot_locations: vec![
            (0x1_0000, 0x1000_003B),
            (0x2_0000, 0x1000_0021),
            (0x4_0000, 0x1000_0030),
        ],
    };
    assert_eq!(rule.location(&javelin, true), Some(0x1000_0029));
    assert_eq!(rule.location(&javelin, false), Some(0x1000_0030));
    // An item with only a paperdoll override keeps the slot default in the world.
    let mut paperdoll_only = javelin.clone();
    paperdoll_only.parenting_location = None;
    paperdoll_only.paperdoll_parenting_location = Some(0x1000_0031);
    assert_eq!(rule.location(&paperdoll_only, true), Some(0x1000_0030));
    assert_eq!(rule.location(&paperdoll_only, false), Some(0x1000_0031));

    // Wardrobe-record blocks: the 1.0 block of the item's key, dye blocks of the same record.
    let rec = AppearanceRecord {
        did: Did(0x2000_1E54),
        entries: vec![
            AppearanceEntry {
                key: 0x1000_0700,
                blocks: vec![block(1.0, &[(1.0, UPPER, 0x0600_0001)])],
            },
            AppearanceEntry {
                key: 0x1000_0034,
                blocks: vec![block(0.0, &[]), block(0.06, &[]), block(0.32, &[])],
            },
        ],
        ..AppearanceRecord::default()
    };
    assert_eq!(worn_block(&rec, 0x1000_0700).unwrap().parts.len(), 1);
    assert!(worn_block(&rec, 0x1000_0701).is_none());
    assert_eq!(dye_block(&rec, 0x1000_0034, 0.06).unwrap().variant, 0.06);
    assert!(
        dye_block(&rec, 0x1000_0034, 0.0).is_none(),
        "0.0 means no dye"
    );

    // NPC props (§5.5).
    let names = |n: &str| -> Option<u32> {
        Some(match n {
            "NPC_Cosmetic_Inventory_Item1" => 0x1000_0400,
            "NPC_Cosmetic_Inventory_HoldingLocation1" => 0x1000_0401,
            "NPC_Cosmetic_Inventory_Item2" => 0x1000_0402,
            "NPC_Cosmetic_Inventory_Visible" => 0x1000_0403,
            _ => return None,
        })
    };
    let npc = set(vec![
        entry(0x1000_0400, PropertyValue::DidRef(Did(0x4700_0009))),
        entry(0x1000_0401, PropertyValue::Enum(1)),
        entry(0x1000_0402, PropertyValue::DidRef(Did(0))),
    ]);
    let held = npc_held_items(&npc, &names);
    assert_eq!(held.len(), 1);
    assert_eq!(
        (held[0].index, held[0].entity, held[0].location),
        (1, Did(0x4700_0009), Some(1))
    );
    assert_eq!(npc_cosmetic_inventory_visible(&npc, &names), None);
    // `NPC_Cosmetic_Inventory_Visible` has no specified meaning (OPEN): the props are still
    // returned and the flag is reported separately.
    let mut hidden = npc.clone();
    hidden
        .entries
        .push(entry(0x1000_0403, PropertyValue::Bool(false)));
    assert_eq!(npc_held_items(&hidden, &names), held);
    assert_eq!(npc_cosmetic_inventory_visible(&hidden, &names), Some(false));
}

// --- race table and avatar bodies through synthetic records ---

const R_LIST: u32 = 0x1000_0500;
const R_RACE: u32 = 0x1000_0501;
const R_GENDERS: u32 = 0x1000_0502;
const R_TYPE: u32 = 0x1000_0503;
const R_AVATAR: u32 = 0x1000_0504;
const R_GENDER_STRUCT: u32 = 0x1000_0505;
const R_SPECIES_REF: u32 = 0x1000_0506;

fn schema(pid: u32) -> Option<PropertyType> {
    Some(match pid {
        R_LIST | R_GENDERS | P_CLASS_LIST | P_INSTANCE_LIST => PropertyType::Array,
        R_GENDER_STRUCT | 0x1000_0220 | P_CONTROLS => PropertyType::Struct,
        R_RACE | R_TYPE | P_KEY => PropertyType::Enum,
        R_AVATAR | R_SPECIES_REF | P_PHYS_OBJ | P_FILE | P_TOGGLE_FILE | P_SLIDER_FILE => {
            PropertyType::DidRef
        }
        P_MOD => PropertyType::Float,
        P_SCALE => PropertyType::Vector3,
        P_TOGGLE_PROP => PropertyType::PropertyId,
        P_VISIBLE => PropertyType::Bool,
        P_SLIDER_VALUES => PropertyType::String,
        _ => return None,
    })
}

/// An entry with PID echo (top level and struct fields).
fn e_echo(pid: u32, value: B) -> B {
    B::new().u32(pid).u32(pid).bytes(&value.build())
}

fn prop_set_record(did: u32, entries: &[B]) -> Vec<u8> {
    let mut b = B::new().u32(did).tsize(0, entries.len() as u32);
    for e in entries {
        b = b.bytes(&e.0);
    }
    b.build()
}

fn embedded(entries: &[B]) -> B {
    let mut b = B::new().tsize(0, entries.len() as u32);
    for e in entries {
        b = b.bytes(&e.0);
    }
    b
}

fn world() -> HashMap<Did, Vec<u8>> {
    let mut m = HashMap::new();
    // Directory object 0x70000001 → species object 0x70000002 (Human) with a male entry
    // whose avatar is 0x70000010, and a female entry without an avatar (skipped).
    m.insert(
        Did(0x7900_0001),
        prop_set_record(
            0x7900_0001,
            &[e_echo(
                R_LIST,
                B::new().u32(1).u32(R_SPECIES_REF).u32(0x7000_0002),
            )],
        ),
    );
    let gender = |ty: u32, avatar: Option<u32>| {
        let mut fields = vec![e_echo(R_TYPE, B::new().u32(ty))];
        if let Some(a) = avatar {
            fields.push(e_echo(R_AVATAR, B::new().u32(a)));
        }
        B::new()
            .u32(R_GENDER_STRUCT)
            .bytes(&embedded(&fields).build())
    };
    m.insert(
        Did(0x7900_0002),
        prop_set_record(
            0x7900_0002,
            &[
                e_echo(R_RACE, B::new().u32(HUMAN)),
                e_echo(
                    R_GENDERS,
                    B::new()
                        .u32(2)
                        .bytes(&gender(MALE, Some(0x7000_0010)).build())
                        .bytes(&gender(FEMALE, None).build()),
                ),
            ],
        ),
    );
    // The avatar object names its entity.
    m.insert(
        Did(0x7900_0010),
        prop_set_record(
            0x7900_0010,
            &[e_echo(P_PHYS_OBJ, B::new().u32(0x4700_0010))],
        ),
    );
    // Entity 0x47000010 → template 0x1F000010 (scale 1.05) → setup 0x04000010. The entity
    // has a class list (one element with a toggle file) and an instance list.
    let key_info = B::new().u32(0x1000_0220).bytes(
        &embedded(&[
            e_echo(P_KEY, B::new().u32(0x1000_0032)),
            e_echo(P_FILE, B::new().u32(0x2000_0002)),
            e_echo(P_MOD, B::new().f32(0.0)),
            e_echo(P_TOGGLE_FILE, B::new().u32(0x2000_0102)),
            e_echo(P_TOGGLE_PROP, B::new().u32(P_VISIBLE)),
        ])
        .build(),
    );
    let controls = embedded(&[e_echo(P_SLIDER_FILE, B::new().u32(0x7800_0001))]);
    let entity = B::new()
        .u32(0x8B)
        .u32(0x4700_0010)
        .u32(7)
        .u32(0x1F00_0010)
        .bytes(
            &embedded(&[
                e_echo(P_SCALE, B::new().f32(0.7).f32(0.7).f32(0.7)),
                e_echo(P_CLASS_LIST, B::new().u32(1).bytes(&key_info.build())),
                e_echo(
                    P_INSTANCE_LIST,
                    B::new()
                        .u32(1)
                        .bytes(&key_info_plain(0x1000_0040, 0x2000_0003).build()),
                ),
                e_echo(P_VISIBLE, B::new().u8(1)),
                e_echo(P_CONTROLS, controls),
                e_echo(P_SLIDER_VALUES, B::new().pascal("80ff")),
            ])
            .build(),
        )
        .build();
    m.insert(Did(0x4700_0010), entity);
    let template = B::new()
        .u32(0x221)
        .u32(0x1F00_0010)
        .f32(1.05)
        .f32(1.05)
        .f32(1.05)
        .tsize(0, 1)
        .u32(1)
        .u32(9)
        .u32(1)
        .u32(0x0400_0010)
        .build();
    m.insert(Did(0x1F00_0010), template);
    m
}

/// An `Appearance_KeyInfo` struct value without toggle or modifier.
fn key_info_plain(key: u32, file: u32) -> B {
    B::new().u32(0x1000_0220).bytes(
        &embedded(&[
            e_echo(P_KEY, B::new().u32(key)),
            e_echo(P_FILE, B::new().u32(file)),
        ])
        .build(),
    )
}

fn names(n: &str) -> Option<u32> {
    Some(match n {
        "RaceTable_RaceTableList" => R_LIST,
        "RaceTable_Race" => R_RACE,
        "RaceTable_GenderList" => R_GENDERS,
        "RaceTable_GenderType" => R_TYPE,
        "RaceTable_Gender_PlayerAvatar" => R_AVATAR,
        "PhysObj" => P_PHYS_OBJ,
        "Render_AdjustableScale" => P_SCALE,
        "Appearance_ClassList" => P_CLASS_LIST,
        "Appearance_InstanceList" => P_INSTANCE_LIST,
        "Appearance_Key" => P_KEY,
        "Appearance_AprFile" => P_FILE,
        "Appearance_Modifier" => P_MOD,
        "Appearance_AprToggleFile" => P_TOGGLE_FILE,
        "Appearance_ToggleProperty" => P_TOGGLE_PROP,
        "AppearanceUI_BoneSliderDefinitionsFile" => P_SLIDER_FILE,
        "AC_BoneSlider_Value_Array" => P_SLIDER_VALUES,
        _ => return None,
    })
}

#[test]
fn race_table_and_avatar_body_resolve() {
    let m = world();
    let fetch = |d: Did| Ok::<_, Infallible>(m.get(&d).cloned());
    let race = RaceTable::read(
        Did(0x7000_0001),
        &schema,
        &RacePids::from_names(&names).unwrap(),
        fetch,
    )
    .unwrap();
    assert_eq!(race.entries.len(), 1);
    let e = race.entry(HUMAN, MALE).unwrap();
    assert_eq!(e.player_avatar, Did(0x7000_0010));
    assert_eq!(e.species_object, Did(0x7000_0002));
    assert_eq!(race.avatars(), vec![Did(0x7000_0010)]);
    assert!(race.entry(HUMAN, FEMALE).is_none());

    let pids = AvatarPids::from_names(&names).unwrap();
    let b = resolve_avatar_body(e.player_avatar, &schema, &pids, fetch).unwrap();
    assert_eq!(b.object, Some(Did(0x7000_0010)));
    assert_eq!(b.entity, Did(0x4700_0010));
    assert_eq!(
        b.chain,
        vec![Did(0x4700_0010), Did(0x1F00_0010), Did(0x0400_0010)]
    );
    assert_eq!(b.setup, Did(0x0400_0010));
    assert_eq!(b.template_scale, Some(1.05));
    assert_eq!(b.entity_scale, Some(0.7));
    assert!((b.model_scale() - 0.735).abs() < 1e-6);
    assert_eq!(b.bone_slider_definitions, Some(Did(0x7800_0001)));
    assert_eq!(b.slider_values.as_deref(), Some("80ff"));
    assert_eq!(b.class_list.len(), 1);
    assert_eq!(b.instance_list.len(), 1);
    let files = b.appearance_files();
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].1,
        Did(0x2000_0102),
        "toggle file: the toggle property is true"
    );
    assert_eq!(files[1].1, Did(0x2000_0003), "the entity's instance list");
    // A placed figure whose instance set has its own list: it replaces the entity's
    // (§7.4 step 4), the class list stays.
    let lists = AppearanceListPids::from_names(&names).unwrap();
    let instance_set = set(vec![entry(
        P_INSTANCE_LIST,
        array(vec![entry(
            0x1000_0220,
            strukt(vec![
                entry(P_KEY, PropertyValue::Enum(0x1000_0041)),
                entry(P_FILE, PropertyValue::DidRef(Did(0x2000_0004))),
            ]),
        )]),
    )]);
    let own = lists.read_list(&instance_set, P_INSTANCE_LIST);
    let files = b.appearance_files_with_instance(Some(&own));
    let dids: Vec<Did> = files.iter().map(|f| f.1).collect();
    assert_eq!(dids, vec![Did(0x2000_0102), Did(0x2000_0004)]);
    assert_eq!(b.appearance_files_with_instance(None), b.appearance_files());
    // Starting from the entity gives the same body without an object.
    let b2 = resolve_avatar_body(Did(0x4700_0010), &schema, &pids, fetch).unwrap();
    assert_eq!((b2.object, b2.setup), (None, b.setup));

    // Broken links are reported.
    let mut broken = world();
    broken.remove(&Did(0x7900_0010));
    let fetch = |d: Did| Ok::<_, Infallible>(broken.get(&d).cloned());
    assert!(matches!(
        resolve_avatar_body(Did(0x7000_0010), &schema, &pids, fetch),
        Err(tessera_formats::ResolveError::Format(
            FormatError::DanglingReference { .. }
        ))
    ));
}

#[test]
fn bone_slider_definitions_read() {
    let pids = BoneSliderPids {
        definitions: 1,
        name: Some(2),
        group: Some(3),
        masks: Some(4),
        limit_min: 5,
        limit_max: 6,
        bones: 7,
        bone_index: 8,
        bone_reverse: Some(9),
        visible: Some(10),
    };
    let slider = |visible: bool, min: f32, max: f32| {
        let mut f = vec![
            entry(2, PropertyValue::Enum(0x1000_0001)),
            entry(3, PropertyValue::Enum(6)),
            entry(4, array(vec![entry(11, PropertyValue::Bitfield32(1 << 6))])),
            entry(5, PropertyValue::Vector3([min; 3])),
            entry(6, PropertyValue::Vector3([max; 3])),
            entry(
                7,
                array(vec![
                    entry(
                        12,
                        strukt(vec![
                            entry(8, PropertyValue::Int(12)),
                            entry(9, PropertyValue::Int(1)),
                        ]),
                    ),
                    entry(
                        12,
                        strukt(vec![
                            entry(8, PropertyValue::Int(13)),
                            entry(9, PropertyValue::Int(0)),
                        ]),
                    ),
                ]),
            ),
        ];
        if !visible {
            f.push(entry(10, PropertyValue::Bool(false)));
        }
        entry(13, strukt(f))
    };
    let s = set(vec![entry(
        1,
        array(vec![
            slider(true, 0.9, 1.1),
            slider(false, 0.8, 0.8),
            entry(13, PropertyValue::Int(0)), // not a struct: skipped
        ]),
    )]);
    let defs = BoneSliderDefinition::read_all(&s, &pids);
    assert_eq!(defs.len(), 2);
    assert!(defs[0].visible && !defs[1].visible);
    assert_eq!(defs[0].channel_bits(), 1 << 6);
    assert_eq!(defs[0].bones.len(), 2);
    assert!(defs[0].bones[0].reverse && !defs[0].bones[1].reverse);
    assert_eq!(defs[0].value_for(255), [1.1; 3]);
    assert_eq!(defs[1].value_for(0), [0.8; 3]);
}

#[test]
fn key_table_check_and_slot_table_are_linear() {
    // 100,000 entries and key-table rows, and a block of 100,000 distinct-slot parts (a few
    // MB of hostile record): both used to be quadratic.
    const N: u32 = 100_000;
    let rec = AppearanceRecord {
        did: Did(0x2000_0001),
        entries: (0..N)
            .map(|k| AppearanceEntry {
                key: 0x1000_0000 + k,
                blocks: vec![block(1.0, &[])],
            })
            .collect(),
        key_table: (0..N).rev().map(|k| (0x1000_0000 + k, 0)).collect(),
        ..AppearanceRecord::default()
    };
    let started = std::time::Instant::now();
    assert!(rec.key_table_matches_selection_counts());
    let mut wrong = rec.clone();
    wrong.key_table[7].1 = 1;
    assert!(!wrong.key_table_matches_selection_counts());
    wrong.key_table[7] = (0x0FFF_FFFF, 0);
    assert!(
        !wrong.key_table_matches_selection_counts(),
        "key without entry"
    );

    let parts: Vec<(f32, u32, u32)> = (0..N).map(|i| (1.0, i, 0x0600_0000 + i)).collect();
    let big = block(1.0, &parts);
    let mut slots = SlotTable::from_defaults((0..N).step_by(2).map(|i| (i, Did(0x0600_0001))));
    slots.offer_block(&big, 3);
    assert_eq!(slots.len(), N as usize);
    // First-filled order is kept: defaults first, then the new slots.
    assert_eq!(slots.iter().next().unwrap().slot, 0);
    assert_eq!(slots.iter().nth(1).unwrap().slot, 2);
    assert_eq!(slots.get(5).unwrap().mesh, Did(0x0600_0005));
    assert_eq!(slots.get(5).unwrap().source, PartSource::Offered(3));
    assert!(
        !slots.offer(5, Did(0x0600_0099), 0.5, 4),
        "lower priority loses"
    );
    assert!(
        slots.offer(5, Did(0x0600_0098), 1.0, 4),
        "a tie: the later source wins"
    );
    assert_eq!(slots.get(5).unwrap().mesh, Did(0x0600_0098));
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
}
