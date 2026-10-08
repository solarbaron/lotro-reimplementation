// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of character assembly (`docs/specs/appearance.md` §4, §5, §7, §8, §12):
//! slot replacement and layering, ties, dyes, missing keys and records, held-item slots,
//! static objects and placed NPCs. Every value is invented.

use std::sync::Arc;

use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceExtra, AppearanceKeyInfo, AppearanceListPids,
    AppearancePart, AppearanceRecord, MaterialGroup,
};
use tessera_formats::avatar::AvatarBody;
use tessera_formats::item::{HoldingRule, ItemVisual, WardrobeFallbacks, WornAppearanceRef};
use tessera_formats::setup::{Anchor, BoneFrame, Frame};
use tessera_formats::{
    Did, PropertyArray, PropertyEntry, PropertySet, PropertyStruct, PropertyValue, Setup, TSize,
};
use tessera_mesh::assembly::{
    AssemblyError, AssemblyLimits, AssemblyVocabulary, CharacterAssembler, CharacterSpec,
    DiagnosticCategory as Cat, EquippedItem, LayerKind, ListOrigin, MapSource, RigKind, SourceRef,
    StowRule, StyleChoice, WieldState,
};
use tessera_mesh::{MeshSummary, SubmeshSummary, VertexFormat};

// --- vocabulary (invented keys) ---

const UPPER: u32 = 0x1000_000C;
const LOWER: u32 = 0x1000_0003;
const HANDS: u32 = 0x1000_0001;
const FEET: u32 = 0x1000_0002;
const CREST: u32 = 0x1000_000E;
const HELMET: u32 = 0x1000_000A;
const CAPE: u32 = 0x1000_000B;
const HEAD: u32 = 0x1000_0010;
const DEFAULT_SLOT: u32 = 1;
const ALL: u32 = 2;

const KEY_WORN: u32 = 0x1000_003A;
const KEY_HAIR: u32 = 0x1000_0032;
const KEY_DYE: u32 = 0x1000_0034;
const KEY_DYE2: u32 = 0x1000_0022;

const HUMAN: u32 = 0x17;
const BEORNING: u32 = 0x72;
const MALE: u32 = 0x1000;

const S_HEAD: u64 = 0x2;
const S_CHEST: u64 = 0x4;
const S_LEGS: u64 = 0x8;
const S_GLOVES: u64 = 0x10;
const S_BOOTS: u64 = 0x20;
const S_BACK: u64 = 0x80;
const S_PRIMARY: u64 = 0x1_0000;
const S_SECONDARY: u64 = 0x2_0000;
const S_RANGED: u64 = 0x4_0000;
const S_CLASS: u64 = 0x10_0000;

const L_RHAND: u32 = 1;
const L_LHAND: u32 = 2;
const L_WPN_RHAND: u32 = 0x1000_003B;
const L_WPN_LOFFHAND: u32 = 0x1000_003C;
const L_FOREARM: u32 = 0x1000_003D;
const L_BACK_BOW: u32 = 0x1000_003E;
const L_LHIP: u32 = 0x1000_003F;
const L_BACK_2H: u32 = 0x1000_0040;
const L_RHIP: u32 = 0x1000_0041;

// `Item_EquipmentCategory` is a bitfield: items store bit k − 1 for key k
// (property-system.md §7.3). Keys 10–13 and 21.
const C_1HSWORD: u64 = 1 << 9;
const C_2HAXE: u64 = 1 << 10;
const C_SHIELD: u64 = 1 << 11;
const C_ORB: u64 = 1 << 12;
const C_CLASS: u64 = 1 << 20;

const P_CLASS: u32 = 100;
const P_INSTANCE: u32 = 101;
const P_KEY: u32 = 102;
const P_FILE: u32 = 103;
const P_MOD: u32 = 104;
const P_NPC_ITEM1: u32 = 110;
const P_NPC_LOC1: u32 = 111;
const P_NPC_ITEM2: u32 = 112;
const P_NPC_LOC2: u32 = 113;
const P_IDLE: u32 = 120;
const P_RANDOM: u32 = 121;
const P_SCALE: u32 = 122;
const RANDOM_UNDEF: u32 = 0x1000_0001;

fn vocab() -> AssemblyVocabulary {
    AssemblyVocabulary {
        lists: Some(AppearanceListPids {
            class_list: Some(P_CLASS),
            instance_list: Some(P_INSTANCE),
            key: P_KEY,
            file: P_FILE,
            modifier: Some(P_MOD),
            toggle_file: None,
            toggle_property: None,
        }),
        clothing_color: Some(KEY_DYE),
        clothing_color_secondary: Some(KEY_DYE2),
        upper_body: Some(UPPER),
        fallbacks: WardrobeFallbacks {
            pairs: vec![(BEORNING, HUMAN)],
        },
        holding: HoldingRule {
            slot_locations: vec![
                (S_PRIMARY, L_WPN_RHAND),
                (S_SECONDARY, L_WPN_LOFFHAND),
                (S_RANGED, L_BACK_BOW),
            ],
        },
        stow: StowRule {
            primary_slot: S_PRIMARY,
            secondary_slot: S_SECONDARY,
            one_handed: C_1HSWORD,
            primary_one_handed: Some(L_LHIP),
            secondary_one_handed: Some(L_RHIP),
            by_category: vec![(C_2HAXE, L_BACK_2H)],
        },
        npc_items: vec![
            (P_NPC_ITEM1, Some(P_NPC_LOC1)),
            (P_NPC_ITEM2, Some(P_NPC_LOC2)),
        ],
        idle_animation: Some(P_IDLE),
        random_appearance_type: Some(P_RANDOM),
        random_appearance_none: vec![0, RANDOM_UNDEF],
        render_scale: Some(P_SCALE),
        bone_scale: None,
        avatar_height: None,
        slider_values: None,
    }
}

// --- record builders ---

fn mesh_summary(did: u32, slot: u32, vertices: u32, skinned: bool) -> MeshSummary {
    let format = if skinned { 0x3002_0252 } else { 0x0000_0112 };
    MeshSummary {
        did: Did(did),
        flags: slot,
        surfaces: vec![Did(0x3100_0001)],
        submeshes: vec![SubmeshSummary {
            format: VertexFormat::new(format).unwrap(),
            vertex_count: vertices,
            index_count: 3 * vertices.saturating_sub(2),
        }],
    }
}

fn group(slot: u32, material: u32) -> MaterialGroup {
    MaterialGroup {
        shader: Did(0),
        material: Did(material),
        a: slot,
        binding_key: 0x77,
        g: 0.0,
    }
}

fn block(variant: f32, parts: &[(f32, u32, u32)], groups: &[(u32, u32)]) -> AppearanceBlock {
    AppearanceBlock {
        variant,
        group_param: 3.0,
        groups: groups.iter().map(|&(s, m)| group(s, m)).collect(),
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

fn record(did: u32, entries: Vec<(u32, Vec<AppearanceBlock>)>) -> Arc<AppearanceRecord> {
    Arc::new(AppearanceRecord {
        did: Did(did),
        section: 0,
        key_table: entries.iter().map(|(k, _)| (*k, 0)).collect(),
        entries: entries
            .into_iter()
            .map(|(key, blocks)| AppearanceEntry { key, blocks })
            .collect(),
    })
}

fn worn_item(slot: u64, record: u32, key: u32, species: u32) -> ItemVisual {
    ItemVisual {
        worn: vec![WornAppearanceRef {
            appearance: Did(record),
            key,
            species,
            sex: MALE,
        }],
        phys_obj: Some(Did(0x4700_9999)),
        default_slot: Some(slot),
        ..ItemVisual::default()
    }
}

fn held_item(slot: u64, entity: u32, category: u64) -> ItemVisual {
    ItemVisual {
        phys_obj: Some(Did(entity)),
        default_slot: Some(slot),
        equipment_category: Some(category),
        ..ItemVisual::default()
    }
}

fn key_info(key: u32, file: u32, modifier: f32) -> AppearanceKeyInfo {
    AppearanceKeyInfo {
        key,
        file: Did(file),
        modifier,
        toggle_file: None,
        toggle_property: None,
    }
}

fn set(entries: Vec<(u32, PropertyValue)>) -> PropertySet {
    PropertySet {
        size: TSize {
            hint: 0,
            count: entries.len() as u32,
        },
        entries: entries
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
    }
}

fn list_value(elements: &[(u32, u32, f32)]) -> PropertyValue {
    let el = |&(k, f, m): &(u32, u32, f32)| PropertyEntry {
        pid: 0,
        value: PropertyValue::Struct(PropertyStruct {
            size: TSize { hint: 0, count: 3 },
            fields: vec![
                PropertyEntry {
                    pid: P_KEY,
                    value: PropertyValue::Enum(k),
                },
                PropertyEntry {
                    pid: P_FILE,
                    value: PropertyValue::DidRef(Did(f)),
                },
                PropertyEntry {
                    pid: P_MOD,
                    value: PropertyValue::Float(m),
                },
            ],
        }),
    };
    PropertyValue::Array(PropertyArray {
        count: elements.len() as u32,
        elements: elements.iter().map(el).collect(),
    })
}

const AVATAR: u32 = 0x7000_0001;
const ENTITY: u32 = 0x4700_0001;
const SETUP: u32 = 0x0400_0001;

/// Meshes of the default body and the items: `appearance.md` §12.1 letters.
const A: u32 = 0x0600_00A1; // UpperBody
const B: u32 = 0x0600_00B1; // LowerBody
const C: u32 = 0x0600_00C1; // Hands (bare)
const D: u32 = 0x0600_00D1; // Feet
const E: u32 = 0x0600_00E1; // Crest (setup hair)
const S0: u32 = 0x0600_0050; // Helmet stub
const F: u32 = 0x0600_00F1; // hair style
const G: u32 = 0x0600_0061; // helmet
const S1: u32 = 0x0600_0051; // Crest stub
const H: u32 = 0x0600_0071; // chest
const S2: u32 = 0x0600_0052; // LowerBody stub
const J: u32 = 0x0600_0081; // leggings
const K: u32 = 0x0600_0091; // gloves
const L: u32 = 0x0600_00A2; // boots
const M: u32 = 0x0600_00B2; // cape
const S3: u32 = 0x0600_0053; // Crest stub (hood)
const S4: u32 = 0x0600_0054; // Helmet stub (hood)
const GAUNTLETS: u32 = 0x0600_00C2;

const HAIR_REC: u32 = 0x2000_0100;
const HEAD_REC: u32 = 0x2000_0001;
const CHEST_REC: u32 = 0x2000_0002;
const LEGS_REC: u32 = 0x2000_0003;
const GLOVES_REC: u32 = 0x2000_0004;
const BOOTS_REC: u32 = 0x2000_0005;
const BACK_REC: u32 = 0x2000_0006;
const WEAPON_REC: u32 = 0x2000_0007;

const I_HEAD: u32 = 0x7900_0001;
const I_CHEST: u32 = 0x7900_0002;
const I_LEGS: u32 = 0x7900_0003;
const I_GLOVES: u32 = 0x7900_0004;
const I_BOOTS: u32 = 0x7900_0005;
const I_BACK: u32 = 0x7900_0006;
const I_GAUNTLETS: u32 = 0x7900_0007;
const I_SWORD: u32 = 0x7900_0010;
const I_SHIELD: u32 = 0x7900_0011;
const I_AXE: u32 = 0x7900_0012;
const I_CLASS: u32 = 0x7900_0013;
const I_ORB: u32 = 0x7900_0014;
const I_RING: u32 = 0x7900_0015;
const I_ELF_ONLY: u32 = 0x7900_0016;
const I_BAD_KEY: u32 = 0x7900_0017;
const I_BROKEN: u32 = 0x7900_0018;

const SWORD_ENTITY: u32 = 0x4700_0100;
const SWORD_SETUP: u32 = 0x0400_0100;
const SHIELD_ENTITY: u32 = 0x4700_0101;
const SHIELD_SETUP: u32 = 0x0400_0101;
const BROKEN_ENTITY: u32 = 0x4700_0102;

const BONE_RWEAPON: u32 = 79;
const BONE_LWEAPON: u32 = 65;
const BONE_LELBOW: u32 = 54;
const BONE_HIP: u32 = 3;

fn location(key: u32, bone: i32, x: f32) -> BoneFrame {
    BoneFrame {
        key,
        key2: key,
        bone,
        frame: Frame {
            position: [x, 0.0, 0.0],
            ..Frame::IDENTITY
        },
    }
}

/// A Human male body: the §12.1 default parts on a skinned humanoid setup, a hair style
/// in its class list, the §12.1 items, a few held items.
fn world() -> MapSource {
    let mut s = MapSource::default();
    s.race.insert((HUMAN, MALE), Did(AVATAR));
    s.race.insert((BEORNING, MALE), Did(AVATAR));
    let body = AvatarBody {
        object: Some(Did(AVATAR)),
        entity: Did(ENTITY),
        chain: vec![Did(ENTITY), Did(0x1F00_0001), Did(SETUP)],
        setup: Did(SETUP),
        template_scale: Some(1.03),
        entity_scale: Some(0.7),
        template_appearance: vec![],
        class_list: vec![key_info(KEY_HAIR, HAIR_REC, 0.0)],
        instance_list: vec![],
        properties: PropertySet::default(),
        bone_slider_definitions: Some(Did(0x7800_0001)),
        slider_values: Some("80ff00".into()),
    };
    s.bodies.insert(Did(AVATAR), body.clone());
    s.bodies.insert(Did(ENTITY), body);
    let setup = Setup {
        did: Did(SETUP),
        tagfile: vec![0x1E, 0x0D, 0xB0, 0xCA],
        parts: [A, B, C, D, E, S0].iter().map(|&d| Did(d)).collect(),
        bone_frames: vec![
            location(L_WPN_RHAND, BONE_RWEAPON as i32, 0.0),
            location(L_WPN_LOFFHAND, BONE_LWEAPON as i32, 0.0),
            location(L_FOREARM, BONE_LELBOW as i32, 0.05),
            location(L_LHIP, BONE_HIP as i32, -0.2),
            location(L_RHIP, BONE_HIP as i32, 0.2),
            location(L_RHAND, BONE_RWEAPON as i32, 0.0),
            location(L_LHAND, -1, 0.3),
        ],
        ..Setup::default()
    };
    s.setups.insert(Did(SETUP), Arc::new(setup));
    for (did, slot, v) in [
        (A, UPPER, 1000),
        (B, LOWER, 800),
        (C, HANDS, 900),
        (D, FEET, 300),
        (E, CREST, 2000),
        (S0, HELMET, 3),
        (F, CREST, 1500),
        (G, HELMET, 400),
        (S1, CREST, 3),
        (H, UPPER, 1200),
        (S2, LOWER, 3),
        (J, LOWER, 700),
        (K, HANDS, 600),
        (L, FEET, 500),
        (M, CAPE, 900),
        (S3, CREST, 3),
        (S4, HELMET, 3),
        (GAUNTLETS, HANDS, 650),
    ] {
        s.meshes.insert(Did(did), mesh_summary(did, slot, v, true));
    }
    // Hair styles 0.00 (F) and 0.01 (the setup's E).
    s.appearances.insert(
        Did(HAIR_REC),
        record(
            HAIR_REC,
            vec![(
                KEY_HAIR,
                vec![
                    block(0.0, &[(1.0, CREST, F)], &[]),
                    block(0.01, &[(1.0, CREST, E)], &[]),
                ],
            )],
        ),
    );
    let dye_table = || {
        (
            KEY_DYE,
            vec![
                block(0.0, &[], &[(ALL, 0x3000_0D00)]),
                block(0.06, &[], &[(ALL, 0x3000_0D06)]),
                block(0.32, &[], &[(ALL, 0x3000_0D32), (UPPER, 0x3000_0E32)]),
            ],
        )
    };
    let worn = |did: u32, parts: &[(f32, u32, u32)]| {
        record(
            did,
            vec![
                (
                    KEY_WORN,
                    vec![block(1.0, parts, &[(ALL, did + 0x1000_0000)])],
                ),
                dye_table(),
            ],
        )
    };
    s.appearances.insert(
        Did(HEAD_REC),
        worn(HEAD_REC, &[(4.0, HELMET, G), (10.0, CREST, S1)]),
    );
    s.appearances.insert(
        Did(CHEST_REC),
        worn(
            CHEST_REC,
            &[(1.0, UPPER, H), (10.0, LOWER, S2), (10.0, HANDS, C)],
        ),
    );
    s.appearances
        .insert(Did(LEGS_REC), worn(LEGS_REC, &[(1.0, LOWER, J)]));
    s.appearances
        .insert(Did(GLOVES_REC), worn(GLOVES_REC, &[(1.0, HANDS, K)]));
    s.appearances
        .insert(Did(BOOTS_REC), worn(BOOTS_REC, &[(2.0, FEET, L)]));
    s.appearances.insert(
        Did(BACK_REC),
        worn(
            BACK_REC,
            &[(1.0, CAPE, M), (15.0, CREST, S3), (15.0, HELMET, S4)],
        ),
    );
    s.appearances.insert(
        Did(WEAPON_REC),
        worn(WEAPON_REC, &[(10.0, HANDS, GAUNTLETS)]),
    );
    for (item, slot, rec) in [
        (I_HEAD, S_HEAD, HEAD_REC),
        (I_CHEST, S_CHEST, CHEST_REC),
        (I_LEGS, S_LEGS, LEGS_REC),
        (I_GLOVES, S_GLOVES, GLOVES_REC),
        (I_BOOTS, S_BOOTS, BOOTS_REC),
        (I_BACK, S_BACK, BACK_REC),
        (I_GAUNTLETS, S_PRIMARY, WEAPON_REC),
    ] {
        s.items
            .insert(Did(item), worn_item(slot, rec, KEY_WORN, HUMAN));
    }
    // Held items.
    s.items
        .insert(Did(I_SWORD), held_item(S_PRIMARY, SWORD_ENTITY, C_1HSWORD));
    let mut shield = held_item(S_SECONDARY, SHIELD_ENTITY, C_SHIELD);
    shield.parenting_location = Some(L_FOREARM);
    s.items.insert(Did(I_SHIELD), shield);
    s.items
        .insert(Did(I_AXE), held_item(S_PRIMARY, SWORD_ENTITY, C_2HAXE));
    s.items
        .insert(Did(I_CLASS), held_item(S_CLASS, SWORD_ENTITY, C_CLASS));
    s.items
        .insert(Did(I_ORB), held_item(S_PRIMARY, SWORD_ENTITY, C_ORB));
    s.items.insert(
        Did(I_RING),
        ItemVisual {
            default_slot: Some(0x400),
            ..ItemVisual::default()
        },
    );
    let mut elf_only = worn_item(S_CHEST, CHEST_REC, KEY_WORN, 0x41);
    elf_only.phys_obj = Some(Did(SWORD_ENTITY));
    s.items.insert(Did(I_ELF_ONLY), elf_only);
    s.items.insert(
        Did(I_BAD_KEY),
        worn_item(S_LEGS, LEGS_REC, 0x1000_0999, HUMAN),
    );
    s.items.insert(
        Did(I_BROKEN),
        held_item(S_PRIMARY, BROKEN_ENTITY, C_1HSWORD),
    );
    for (entity, setup) in [(SWORD_ENTITY, SWORD_SETUP), (SHIELD_ENTITY, SHIELD_SETUP)] {
        s.chains
            .insert(Did(entity), vec![Did(entity), Did(0x1F00_0100), Did(setup)]);
        s.setups.insert(
            Did(setup),
            Arc::new(Setup {
                did: Did(setup),
                parts: vec![Did(setup + 0x0200_0000)],
                ..Setup::default()
            }),
        );
    }
    s
}

fn assembler(s: MapSource) -> CharacterAssembler<MapSource> {
    CharacterAssembler::new(s, vocab())
}

fn human(items: &[u32]) -> CharacterSpec {
    let mut spec = CharacterSpec::new(HUMAN, MALE);
    for &i in items {
        spec = spec.wearing(EquippedItem::new(i));
    }
    spec
}

/// (slot → visible mesh or `None` when hidden) for the slots of §12.1.
fn visible(c: &tessera_mesh::assembly::AssembledCharacter, slot: u32) -> Option<u32> {
    let p = c.part_in(slot).expect("slot filled");
    (!p.hidden).then_some(p.mesh.0)
}

#[test]
fn spec_example_slot_resolution() {
    // appearance.md §12.1, items given out of slot order to check the sorting.
    let mut asm = assembler(world());
    let all = [I_BACK, I_BOOTS, I_GLOVES, I_LEGS, I_CHEST, I_HEAD];
    let c = asm.assemble_character(&human(&all)).unwrap();
    assert_eq!(c.rig, RigKind::Humanoid);
    assert_eq!(visible(&c, UPPER), Some(H));
    assert_eq!(visible(&c, LOWER), None, "S2 (10) beats the leggings (1)");
    assert_eq!(c.part_in(LOWER).unwrap().mesh, Did(S2));
    assert_eq!(
        visible(&c, HANDS),
        Some(C),
        "the chest's bare hands (10) beat gloves"
    );
    assert_eq!(visible(&c, FEET), Some(L));
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(S3), "15 > 10 > 1");
    assert_eq!(visible(&c, CREST), None);
    assert_eq!(c.part_in(HELMET).unwrap().mesh, Did(S4));
    assert_eq!(visible(&c, CAPE), Some(M));
    // One part per slot; the setup's six slots first, in setup order, then the cape.
    let slots: Vec<u32> = c.parts.iter().map(|p| p.slot).collect();
    assert_eq!(slots, vec![UPPER, LOWER, HANDS, FEET, CREST, HELMET, CAPE]);
    let mut hidden: Vec<u32> = c.hidden_slots().collect();
    hidden.sort_unstable();
    assert_eq!(hidden, {
        let mut h = vec![LOWER, CREST, HELMET];
        h.sort_unstable();
        h
    });
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);

    // Remove the back item: Crest → S1 (hidden), Helmet → G.
    let c = asm
        .assemble_character(&human(&[I_HEAD, I_CHEST, I_LEGS, I_GLOVES, I_BOOTS]))
        .unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(S1));
    assert!(c.part_in(CREST).unwrap().hidden);
    assert_eq!(visible(&c, HELMET), Some(G));
    assert!(c.part_in(CAPE).is_none());
    // Remove the head item as well: Crest → F (the class list's style 0.00), Helmet → S0.
    let c = asm
        .assemble_character(&human(&[I_CHEST, I_LEGS, I_GLOVES, I_BOOTS]))
        .unwrap();
    assert_eq!(visible(&c, CREST), Some(F));
    assert_eq!(
        c.part_in(CREST).unwrap().source(),
        Some(SourceRef::ClassList(0))
    );
    let helmet = c.part_in(HELMET).unwrap();
    assert!(helmet.is_default() && helmet.hidden && helmet.mesh == Did(S0));
    // Nothing worn: the default body plus the hair style.
    let c = asm.assemble_character(&human(&[])).unwrap();
    for (slot, mesh) in [(UPPER, A), (LOWER, B), (HANDS, C), (FEET, D)] {
        let p = c.part_in(slot).unwrap();
        assert!(p.is_default() && p.mesh == Did(mesh));
    }
}

#[test]
fn ties_go_to_the_later_slot_and_styles_replace_modifiers() {
    let mut s = world();
    // A chest whose bare hands are at 10, gauntlets (Weapon_Primary) at 10 as well.
    let mut asm = assembler(s.clone());
    let c = asm
        .assemble_character(&human(&[I_GAUNTLETS, I_CHEST]))
        .unwrap();
    let hands = c.part_in(HANDS).unwrap();
    assert_eq!(
        hands.mesh,
        Did(GAUNTLETS),
        "§4.7: the later slot wins a tie"
    );
    assert_eq!(hands.source(), Some(SourceRef::Item(0)));

    // The chosen style replaces the list modifier: style 0.01 is the setup's own hair.
    let mut spec = human(&[]);
    spec.styles.push(StyleChoice {
        key: KEY_HAIR,
        modifier: 0.01,
    });
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
    assert!(!c.part_in(CREST).unwrap().is_default());
    // A modifier no block has: rule 3 (no 1.0 block, so the first) and a diagnostic.
    spec.styles[0].modifier = 0.5;
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(F));
    assert_eq!(c.count(Cat::BlockFallback), 1);

    // Species fallback (§2.3): a Beorning wears the Human struct.
    s.items.get_mut(&Did(I_LEGS)).unwrap().worn[0].species = HUMAN;
    let mut asm = assembler(s);
    let mut spec = CharacterSpec::new(BEORNING, MALE).wearing(EquippedItem::new(I_LEGS));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(LOWER).unwrap().mesh, Did(J));
    spec.species = 0x99;
    assert!(matches!(
        asm.assemble_character(&spec),
        Err(AssemblyError::NoSuchBody { species: 0x99, .. })
    ));
}

#[test]
fn dyes_layer_onto_the_items_parts() {
    let mut s = world();
    s.items.get_mut(&Did(I_CHEST)).unwrap().clothing_color = Some(0.32);
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[I_CHEST])).unwrap();
    let chest = c.part_in(UPPER).unwrap();
    let kinds: Vec<(LayerKind, u32)> = chest
        .materials
        .iter()
        .map(|l| (l.kind, l.material.0))
        .collect();
    // The block's own group, then both dye groups (All and UpperBody) of block 0.32.
    assert_eq!(
        kinds,
        vec![
            (LayerKind::Group, CHEST_REC + 0x1000_0000),
            (LayerKind::Dye, 0x3000_0D32),
            (LayerKind::Dye, 0x3000_0E32),
        ]
    );
    assert_eq!(
        chest.materials[0].priority, 3.0,
        "group_param is the first priority"
    );
    // The UpperBody-only dye group does not reach the LowerBody stub.
    let stub = c.part_in(LOWER).unwrap();
    assert_eq!(stub.materials.len(), 2);
    // A chosen dye replaces the default; 0.0 means none; an unknown code is inexact.
    let mut spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_CHEST).dyed(0.06));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(
        c.part_in(UPPER).unwrap().materials[1].material,
        Did(0x3000_0D06)
    );
    spec.items[0].dye = Some(0.0);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().materials.len(), 1);
    spec.items[0].dye = Some(0.2);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().materials[1].variant, 0.06);
    assert_eq!(c.count(Cat::DyeInexact), 1);
    // A second channel the record has no table for.
    spec.items[0].secondary_dye = Some(0.1);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.count(Cat::DyeEntryMissing), 1);
}

#[test]
fn unresolved_links_are_diagnostics() {
    let mut s = world();
    // The legs record names a mesh that does not exist: the slot keeps its winner.
    Arc::make_mut(s.appearances.get_mut(&Did(LEGS_REC)).unwrap()).entries[0].blocks[0]
        .parts
        .push(AppearancePart {
            part_param: 50.0,
            slot: FEET,
            mesh: Did(0x0600_DEAD),
        });
    // Block extras are reported.
    Arc::make_mut(s.appearances.get_mut(&Did(GLOVES_REC)).unwrap()).entries[0].blocks[0]
        .extras
        .push(AppearanceExtra {
            kind: 9,
            slot: 0,
            did: Did(0x0400_0002),
        });
    let body = s.bodies.get_mut(&Did(AVATAR)).unwrap();
    body.class_list.push(key_info(KEY_HAIR, 0, 0.0)); // null DID
    body.class_list.push(key_info(KEY_HAIR, 0x2000_0BAD, 0.0)); // missing record
    body.class_list.push(key_info(0x1000_0777, HAIR_REC, 0.0)); // key missing
    body.slider_values = Some("zz".into());
    let mut asm = assembler(s);
    let items = [
        I_LEGS,
        I_GLOVES,
        I_RING,
        I_ELF_ONLY,
        I_BAD_KEY,
        0x7900_0BAD,
        I_CLASS,
        I_BROKEN,
    ];
    let c = asm.assemble_character(&human(&items)).unwrap();
    let expect = [
        (Cat::NullAppearance, 1),
        (Cat::AppearanceMissing, 1),
        (Cat::KeyMissing, 2), // the list element and the item with a key its record lacks
        (Cat::MeshMissing, 1),
        (Cat::BlockExtraIgnored, 1),
        (Cat::ItemInvisible, 1),
        (Cat::NoWardrobeStruct, 1),
        (Cat::ItemMissing, 1),
        (Cat::NoHoldingLocation, 1),
        (Cat::HeldChainBroken, 1),
        (Cat::SliderValuesInvalid, 1),
    ];
    for (cat, n) in expect {
        assert_eq!(c.count(cat), n, "{cat}: {:?}", c.diagnostics);
    }
    assert_eq!(c.diagnostics.len(), 12, "{:?}", c.diagnostics);
    assert_eq!(c.part_in(LOWER).unwrap().mesh, Did(J));
    assert_eq!(
        c.part_in(FEET).unwrap().mesh,
        Did(D),
        "missing mesh: default kept"
    );
    assert_eq!(c.part_in(HANDS).unwrap().mesh, Did(K));
    // `appearance.md` §6.3 (A16, A29): a piece without a hexadecimal digit reads 0 (still
    // noted), and the empty piece follows an even-length string.
    assert_eq!(c.sliders.values, vec![0, 0]);
    assert!(c.held.is_empty());
    // A body that does not exist and one whose setup is missing are errors.
    let mut s = world();
    s.setups.remove(&Did(SETUP));
    let mut asm = assembler(s);
    assert!(matches!(
        asm.assemble_character(&human(&[])),
        Err(AssemblyError::Setup { .. })
    ));
    assert!(matches!(
        asm.assemble_object(Did(0x7000_0BAD), None),
        Err(AssemblyError::Body { .. })
    ));
}

#[test]
fn held_items_get_their_slot_and_offset() {
    let mut asm = assembler(world());
    let spec = human(&[I_SWORD, I_SHIELD]);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held.len(), 2);
    let sword = &c.held[0];
    assert_eq!(sword.location, L_WPN_RHAND);
    assert_eq!(sword.anchor, Anchor::Bone(BONE_RWEAPON));
    assert_eq!(sword.setup, Did(SWORD_SETUP));
    assert_eq!(sword.parts, vec![Did(SWORD_SETUP + 0x0200_0000)]);
    assert_eq!(sword.chain.first(), Some(&Did(SWORD_ENTITY)));
    let shield = &c.held[1];
    assert_eq!(shield.location, L_FOREARM, "the item's parenting location");
    assert_eq!(shield.anchor, Anchor::Bone(BONE_LELBOW));
    assert_eq!(shield.frame.position, [0.05, 0.0, 0.0]);
    // Stowed: one-handed primary → left hip; two-handed axe → back; orb stays in hand.
    let mut spec = CharacterSpec::new(HUMAN, MALE);
    for i in [I_SWORD, I_AXE, I_ORB] {
        spec = spec.wearing(EquippedItem::new(i).with_state(WieldState::Sheathed));
    }
    let c = asm.assemble_character(&spec).unwrap();
    let locs: Vec<(u32, WieldState)> = c.held.iter().map(|h| (h.location, h.state)).collect();
    assert_eq!(locs.len(), 2, "{:?}", c.diagnostics);
    assert_eq!(locs[0], (L_LHIP, WieldState::Sheathed));
    assert_eq!(locs[1], (L_WPN_RHAND, WieldState::Sheathed));
    assert_eq!(
        c.count(Cat::HoldingLocationMissing),
        1,
        "no Sheath_Back_2H on this setup"
    );
    // Bounded: a figure holds at most `max_held` items.
    let mut asm = assembler(world()).with_limits(AssemblyLimits {
        max_held: 1,
        max_items: 3,
        ..AssemblyLimits::default()
    });
    let spec = human(&[I_SWORD, I_SWORD, I_SWORD, I_SWORD]);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held.len(), 1);
    assert_eq!(c.count(Cat::LimitExceeded), 3, "{:?}", c.diagnostics);
}

#[test]
fn static_objects_are_unchanged() {
    let mut s = MapSource::default();
    let parts = [0x0600_1001, 0x0600_1002, 0x0600_1001];
    for d in parts {
        s.meshes
            .insert(Did(d), mesh_summary(d, DEFAULT_SLOT, 40, false));
    }
    let setup = Setup {
        did: Did(0x0400_1000),
        parts: parts.iter().map(|&d| Did(d)).collect(),
        ..Setup::default()
    };
    s.setups.insert(setup.did, Arc::new(setup));
    s.bodies.insert(
        Did(0x4700_1000),
        AvatarBody {
            object: None,
            entity: Did(0x4700_1000),
            chain: vec![Did(0x4700_1000), Did(0x0400_1000)],
            setup: Did(0x0400_1000),
            template_scale: None,
            entity_scale: None,
            template_appearance: vec![],
            class_list: vec![],
            instance_list: vec![],
            properties: PropertySet::default(),
            bone_slider_definitions: None,
            slider_values: None,
        },
    );
    let mut asm = assembler(s);
    let c = asm.assemble_object(Did(0x4700_1000), None).unwrap();
    assert_eq!(c.rig, RigKind::Static);
    let meshes: Vec<u32> = c.parts.iter().map(|p| p.mesh.0).collect();
    assert_eq!(
        meshes,
        parts.to_vec(),
        "every part, in order, duplicates kept"
    );
    assert!(c.parts.iter().all(|p| p.is_default() && !p.hidden));
    assert!(c.diagnostics.is_empty());
    assert_eq!(c.model_scale(), 1.0);
    assert_eq!(c.clip_inputs.start, Did(0x4700_1000));
}

#[test]
fn placed_npcs_follow_section_7_4() {
    let mut s = world();
    // An NPC record: a full-body piece with stubs over the rig's head and hair (as N1 of
    // the M4 checklist), plus a dye table paired by a ClothingColor element.
    const NPC_REC: u32 = 0x2000_0200;
    const PIECE: u32 = 0x0600_0201;
    const PROP: u32 = 0x4700_0300;
    s.meshes
        .insert(Did(PIECE), mesh_summary(PIECE, UPPER, 5000, true));
    s.meshes
        .insert(Did(0x0600_0202), mesh_summary(0x0600_0202, HEAD, 3, true));
    s.appearances.insert(
        Did(NPC_REC),
        record(
            NPC_REC,
            vec![
                (
                    KEY_WORN,
                    vec![block(
                        1.0,
                        &[
                            (1.0, UPPER, PIECE),
                            (1.0, CREST, S1),
                            (1.0, HEAD, 0x0600_0202),
                        ],
                        &[],
                    )],
                ),
                (
                    KEY_DYE,
                    vec![
                        block(0.0, &[], &[(ALL, 0x3000_0A00)]),
                        block(0.06, &[], &[(ALL, 0x3000_0A06)]),
                    ],
                ),
            ],
        ),
    );
    s.chains
        .insert(Did(PROP), vec![Did(PROP), Did(SWORD_SETUP)]);
    let instance = set(vec![
        (
            P_INSTANCE,
            list_value(&[(KEY_WORN, NPC_REC, 1.0), (KEY_DYE, NPC_REC, 0.06)]),
        ),
        (P_NPC_ITEM1, PropertyValue::DidRef(Did(PROP))),
        (P_NPC_LOC1, PropertyValue::Enum(L_LHAND)),
        (P_NPC_ITEM2, PropertyValue::DidRef(Did(PROP))),
        (P_IDLE, PropertyValue::Enum(0x7)),
        (P_RANDOM, PropertyValue::Enum(RANDOM_UNDEF)),
        (P_SCALE, PropertyValue::Vector3([0.9, 0.9, 0.9])),
    ]);
    let mut asm = assembler(s.clone());
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.instance_list_origin, ListOrigin::Instance);
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(PIECE));
    // The piece's stubs win over the class list's hair (both priority 1: later wins).
    assert!(c.part_in(CREST).unwrap().hidden);
    assert!(
        c.part_in(HEAD).unwrap().hidden,
        "a slot the setup lacks is appended"
    );
    // The ClothingColor element dyes the parts of its own record.
    let dyed = &c.part_in(UPPER).unwrap().materials;
    assert_eq!(dyed.len(), 1);
    assert_eq!(dyed[0].material, Did(0x3000_0A06));
    // Props: item 1 in LHand (an origin-anchored location), item 2 without a location.
    assert_eq!(c.held.len(), 1);
    assert_eq!(c.held[0].source, SourceRef::NpcProp(1));
    assert_eq!(c.held[0].anchor, Anchor::Origin);
    assert_eq!(c.count(Cat::NoHoldingLocation), 1);
    assert_eq!(c.clip_inputs.starting_idle_animation, Some(0x7));
    assert!(c.clip_inputs.instance_properties.is_some());
    assert_eq!(c.random_appearance, None, "Undef is no random appearance");
    assert!((c.model_scale() - 1.03 * 0.7 * 0.9).abs() < 1e-6);

    // A random-appearance figure: default parts and held props only (§7.4 Decision); its
    // lists (class list and instance list) are skipped.
    let instance = set(vec![
        (P_RANDOM, PropertyValue::Enum(0xDB)),
        (
            P_INSTANCE,
            list_value(&[(KEY_WORN, NPC_REC, 1.0), (KEY_DYE, NPC_REC, 0.0)]),
        ),
        (P_NPC_ITEM1, PropertyValue::DidRef(Did(PROP))),
        (P_NPC_LOC1, PropertyValue::Enum(L_LHAND)),
    ]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.random_appearance, Some(0xDB));
    assert_eq!(c.count(Cat::RandomAppearance), 1);
    assert_eq!(c.count(Cat::UnpairedDye), 0);
    assert!(c.parts.iter().all(|p| p.is_default()), "{:?}", c.parts);
    assert!(c.parts.iter().all(|p| p.materials.is_empty()));
    assert!(
        !c.parts.iter().any(|p| p.mesh == Did(F)),
        "the class list's hair style is skipped too"
    );
    assert_eq!(c.held.len(), 1, "held props are kept");

    // An unpaired dye element (no random type) dyes the default parts.
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_DYE, NPC_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.random_appearance, None);
    assert_eq!(c.count(Cat::UnpairedDye), 1);
    let upper = c.part_in(UPPER).unwrap();
    assert!(upper.is_default());
    assert_eq!(upper.materials[0].material, Did(0x3000_0A00));

    // A weenie object: its own property set supplies the list when there is no instance.
    s.objects.insert(
        Did(AVATAR),
        set(vec![(P_INSTANCE, list_value(&[(KEY_WORN, NPC_REC, 1.0)]))]),
    );
    let mut asm = assembler(s);
    let c = asm.assemble_object(Did(AVATAR), None).unwrap();
    assert_eq!(c.instance_list_origin, ListOrigin::Object);
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(PIECE));
    assert_eq!(c.clip_inputs.start, Did(AVATAR));
}

#[test]
fn sliders_and_skeleton_are_passed_through() {
    let mut asm = assembler(world());
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.skeleton.setup, Did(SETUP));
    assert_eq!(&*c.skeleton.tagfile, &[0x1E, 0x0D, 0xB0, 0xCA]);
    // Figures on the same setup share the tagfile bytes instead of copying them.
    let again = asm.assemble_character(&human(&[I_CHEST])).unwrap();
    assert!(Arc::ptr_eq(&c.skeleton.tagfile, &again.skeleton.tagfile));
    // A replaced setup record is copied anew.
    let mut replaced = (*c.setup).clone();
    replaced.tagfile = vec![9, 9];
    asm.source_mut()
        .setups
        .insert(Did(SETUP), Arc::new(replaced));
    let fresh = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(&*fresh.skeleton.tagfile, &[9, 9]);
    assert_eq!(c.sliders.definitions, Some(Did(0x7800_0001)));
    assert_eq!(c.sliders.values, vec![0x80, 0xFF, 0x00, 0]);
    let mut spec = human(&[]);
    spec.slider_values = Some("10".into());
    spec.bone_scale = Some(0.5);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.sliders.values, vec![0x10, 0]);
    assert_eq!(c.sliders.bone_scale, Some(0.5));
    assert_eq!(c.clip_inputs.start, Did(AVATAR));
    assert_eq!(c.clip_inputs.entity, Did(ENTITY));
    assert!((c.model_scale() - 1.03 * 0.7).abs() < 1e-6);
}

#[test]
fn equipment_categories_are_bit_masks() {
    // `from_lookup` turns category labels into bits (key k → bit k − 1, property-system.md
    // §7.3), as it does for slot labels; items store such masks. The keys are arbitrary
    // and synthetic; only the labels come from the spec. One category key lies above 32 so
    // that a mask bit above 31 is exercised.
    const LHIP: u32 = 0x0A00_0001;
    const RHIP: u32 = 0x0A00_0002;
    const BACK_2H: u32 = 0x0A00_0003;
    const BACK_2H_SWORD: u32 = 0x0A00_0004;
    const BACK_BOW: u32 = 0x0A00_0005;
    const BACK_SHIELD: u32 = 0x0A00_0006;
    let keys = |property: &str, label: &str| -> Option<u32> {
        let table: &[(&str, &str, u32)] = &[
            ("Inventory_DefaultSlot", "Weapon_Primary", 5),
            ("Inventory_DefaultSlot", "Weapon_Secondary", 6),
            ("Item_EquipmentCategory", "1HSword", 2),
            ("Item_EquipmentCategory", "2HSword", 9),
            ("Item_EquipmentCategory", "2HClub", 11),
            ("Item_EquipmentCategory", "Bow", 14),
            ("Item_EquipmentCategory", "Shield", 21),
            ("Item_EquipmentCategory", "MediumShield", 46),
            ("Inventory_ParentingLocation", "Sheath_LHip", LHIP),
            ("Inventory_ParentingLocation", "Sheath_RHip", RHIP),
            ("Inventory_ParentingLocation", "Sheath_Back_2H", BACK_2H),
            (
                "Inventory_ParentingLocation",
                "Sheath_Back_2H_Sword",
                BACK_2H_SWORD,
            ),
            ("Inventory_ParentingLocation", "Sheath_Back_Bow", BACK_BOW),
            (
                "Inventory_ParentingLocation",
                "Sheath_Back_Shield",
                BACK_SHIELD,
            ),
        ];
        table
            .iter()
            .find(|(p, l, _)| *p == property && *l == label)
            .map(|e| e.2)
    };
    let names = |_: &str| -> Option<u32> { None };
    let v = AssemblyVocabulary::from_lookup(&names, keys, |_| None);
    let stow = &v.stow;
    let primary = 1 << 4;
    let secondary = 1 << 5;
    let (sword_1h, sword_2h, club_2h, bow, shield, medium_shield) =
        (1 << 1, 1 << 8, 1 << 10, 1 << 13, 1 << 20, 1u64 << 45);
    assert_eq!(stow.primary_slot, primary);
    assert_eq!(stow.secondary_slot, secondary);
    assert_eq!(stow.one_handed, sword_1h);
    assert!(stow.by_category.contains(&(sword_2h, BACK_2H_SWORD)));
    assert!(stow.by_category.contains(&(club_2h, BACK_2H)));
    assert!(stow.by_category.contains(&(bow, BACK_BOW)));
    assert!(stow.by_category.contains(&(shield, BACK_SHIELD)));
    assert!(
        stow.by_category.contains(&(medium_shield, BACK_SHIELD)),
        "bits above 31 are kept"
    );
    let item = |category: u64, slot: u64| ItemVisual {
        default_slot: Some(slot),
        equipment_category: Some(category),
        ..ItemVisual::default()
    };
    assert_eq!(
        stow.location(&item(sword_2h, primary)),
        Some(BACK_2H_SWORD),
        "2HSword"
    );
    assert_eq!(
        stow.location(&item(club_2h, primary)),
        Some(BACK_2H),
        "2HClub"
    );
    assert_eq!(stow.location(&item(bow, primary)), Some(BACK_BOW), "Bow");
    assert_eq!(
        stow.location(&item(shield, secondary)),
        Some(BACK_SHIELD),
        "Shield"
    );
    assert_eq!(
        stow.location(&item(medium_shield, secondary)),
        Some(BACK_SHIELD),
        "MediumShield (bit 45)"
    );
    let sword = item(sword_1h, primary);
    assert_eq!(stow.location(&sword), Some(LHIP), "1HSword: left hip");
    assert_eq!(
        stow.location_in(&sword, Some(secondary)),
        Some(RHIP),
        "1HSword in the secondary slot: right hip"
    );
    assert_eq!(stow.location(&item(0, primary)), None);
    assert_eq!(
        stow.location(&item(1 << 30, primary)),
        None,
        "unknown: in hand"
    );
}

#[test]
fn the_equipped_slot_decides_hand_and_stow() {
    // Dual wielding: the same one-handed weapon (default slot Weapon_Primary) in both
    // weapon slots (appearance.md §5.2–§5.4).
    let mut asm = assembler(world());
    let spec = CharacterSpec::new(HUMAN, MALE)
        .wearing(EquippedItem::new(I_SWORD).in_slot(S_SECONDARY))
        .wearing(EquippedItem::new(I_SWORD));
    let c = asm.assemble_character(&spec).unwrap();
    let held: Vec<(SourceRef, u32, Anchor)> = c
        .held
        .iter()
        .map(|h| (h.source, h.location, h.anchor))
        .collect();
    assert_eq!(
        held,
        vec![
            (SourceRef::Item(1), L_WPN_RHAND, Anchor::Bone(BONE_RWEAPON)),
            (
                SourceRef::Item(0),
                L_WPN_LOFFHAND,
                Anchor::Bone(BONE_LWEAPON)
            ),
        ],
        "ascending slot order, each in its own hand"
    );
    let sheathed = |i: EquippedItem| i.with_state(WieldState::Sheathed);
    let spec = CharacterSpec::new(HUMAN, MALE)
        .wearing(sheathed(EquippedItem::new(I_SWORD)))
        .wearing(sheathed(EquippedItem::new(I_SWORD).in_slot(S_SECONDARY)));
    let c = asm.assemble_character(&spec).unwrap();
    let locs: Vec<u32> = c.held.iter().map(|h| h.location).collect();
    assert_eq!(locs, vec![L_LHIP, L_RHIP]);
    // Slot 0 means the default slot.
    let spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_SWORD).in_slot(0));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held[0].location, L_WPN_RHAND);
}

#[test]
fn material_only_list_blocks_pair_with_their_records_parts() {
    // A selection table of NPC pieces (appearance.md §4.11): material-only blocks under a
    // key other than ClothingColor apply with the record's parts.
    const SEL_REC: u32 = 0x2000_0300;
    const KEY_SEL: u32 = 0x1000_1AC8;
    const KEY_EMPTY: u32 = 0x1000_1AC9;
    let mut s = world();
    s.appearances.insert(
        Did(SEL_REC),
        record(
            SEL_REC,
            vec![
                (KEY_WORN, vec![block(1.0, &[(1.0, UPPER, H)], &[])]),
                (
                    KEY_SEL,
                    vec![
                        block(0.0, &[], &[(ALL, 0x3000_5E00)]),
                        block(0.01, &[], &[(ALL, 0x3000_5E01)]),
                    ],
                ),
                (KEY_EMPTY, vec![block(0.0, &[], &[])]),
            ],
        ),
    );
    let mut asm = assembler(s);
    let instance = set(vec![(
        P_INSTANCE,
        list_value(&[(KEY_WORN, SEL_REC, 1.0), (KEY_SEL, SEL_REC, 0.01)]),
    )]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    let upper = c.part_in(UPPER).unwrap();
    assert_eq!(upper.mesh, Did(H));
    assert_eq!(upper.source(), Some(SourceRef::InstanceList(0)));
    let layers: Vec<(LayerKind, u32)> = upper
        .materials
        .iter()
        .map(|l| (l.kind, l.material.0))
        .collect();
    assert_eq!(layers, vec![(LayerKind::Group, 0x3000_5E01)]);
    assert_eq!(c.count(Cat::MaterialOnly), 0, "{:?}", c.diagnostics);
    // Only the record's parts take it: the default lower body does not.
    assert!(c.part_in(LOWER).unwrap().materials.is_empty());

    // Unpaired: onto the default parts, as an unpaired dye (Decision).
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_SEL, SEL_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.count(Cat::UnpairedDye), 1);
    assert_eq!(c.count(Cat::MaterialOnly), 0);
    let upper = c.part_in(UPPER).unwrap();
    assert!(upper.is_default());
    assert_eq!(upper.materials[0].material, Did(0x3000_5E00));
    assert_eq!(upper.materials[0].kind, LayerKind::Group);

    // A block with neither parts nor groups has nothing to apply.
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_EMPTY, SEL_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.count(Cat::MaterialOnly), 1);
    assert_eq!(c.count(Cat::UnpairedDye), 0);
}

#[test]
fn list_dyes_do_not_override_a_worn_items_dye() {
    let mut s = world();
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = vec![
        key_info(KEY_WORN, CHEST_REC, 1.0),
        key_info(KEY_DYE, CHEST_REC, 0.32),
    ];
    let mut asm = assembler(s);
    let layers = |c: &tessera_mesh::assembly::AssembledCharacter| -> Vec<(LayerKind, u32)> {
        c.part_in(UPPER)
            .unwrap()
            .materials
            .iter()
            .map(|l| (l.kind, l.material.0))
            .collect()
    };
    // The list's own piece takes the list's dye.
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(
        c.part_in(UPPER).unwrap().source(),
        Some(SourceRef::ClassList(0))
    );
    assert_eq!(
        layers(&c),
        vec![
            (LayerKind::Group, CHEST_REC + 0x1000_0000),
            (LayerKind::Dye, 0x3000_0D32),
            (LayerKind::Dye, 0x3000_0E32),
        ]
    );
    // A worn item on the same record keeps its chosen dye (§4.9).
    let spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_CHEST).dyed(0.06));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().source(), Some(SourceRef::Item(0)));
    assert_eq!(
        layers(&c),
        vec![
            (LayerKind::Group, CHEST_REC + 0x1000_0000),
            (LayerKind::Dye, 0x3000_0D06),
        ]
    );
}

#[test]
fn dye_code_zero_on_lists_and_on_items() {
    // Lists: modifier 0.0 picks the 0.00 block (appearance.md §3.2.1 rule 1). Items: code
    // 0.0 means no dye (material.md §12.4). Which reading lists follow is OPEN
    // (appearance.md §15); both paths are pinned here.
    let mut s = world();
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = vec![
        key_info(KEY_WORN, CHEST_REC, 1.0),
        key_info(KEY_DYE, CHEST_REC, 0.0),
    ];
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[])).unwrap();
    let m: Vec<u32> = c
        .part_in(UPPER)
        .unwrap()
        .materials
        .iter()
        .map(|l| l.material.0)
        .collect();
    assert_eq!(m, vec![CHEST_REC + 0x1000_0000, 0x3000_0D00]);
    let mut asm = assembler(world());
    let spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_CHEST).dyed(0.0));
    let c = asm.assemble_character(&spec).unwrap();
    let m: Vec<u32> = c
        .part_in(UPPER)
        .unwrap()
        .materials
        .iter()
        .map(|l| l.material.0)
        .collect();
    assert_eq!(m, vec![CHEST_REC + 0x1000_0000]);
}

#[test]
fn undecodable_records_keep_their_category() {
    let mut s = world();
    // The leggings' mesh exists but does not decode; two inputs name it.
    s.invalid.insert(Did(J));
    s.invalid.insert(Did(I_BOOTS));
    s.invalid.insert(Did(GLOVES_REC));
    s.bodies
        .get_mut(&Did(AVATAR))
        .unwrap()
        .class_list
        .push(key_info(KEY_WORN, LEGS_REC, 1.0));
    let mut asm = assembler(s);
    let c = asm
        .assemble_character(&human(&[I_LEGS, I_BOOTS, I_GLOVES]))
        .unwrap();
    assert_eq!(c.count(Cat::MeshUndecodable), 2, "{:?}", c.diagnostics);
    assert_eq!(c.count(Cat::MeshMissing), 0);
    assert_eq!(c.count(Cat::ItemUndecodable), 1);
    assert_eq!(c.count(Cat::AppearanceUndecodable), 1);
    assert_eq!(
        c.part_in(LOWER).unwrap().mesh,
        Did(B),
        "the default is kept"
    );
}

#[test]
fn many_list_elements_on_one_record_share_their_layers() {
    // 256 part elements and 256 dye elements naming one record whose blocks carry 1000
    // groups each, none applying to a part (the worst case of the layer scan). Each
    // distinct block is stored once, so this is cheap.
    const BIG: u32 = 0x2000_0400;
    const NOWHERE: u32 = 0x1000_0777;
    let groups: Vec<(u32, u32)> = (0..1000).map(|i| (NOWHERE, 0x3000_0000 + i)).collect();
    let mut s = world();
    s.appearances.insert(
        Did(BIG),
        record(
            BIG,
            vec![
                (KEY_WORN, vec![block(1.0, &[(1.0, UPPER, H)], &groups)]),
                (KEY_DYE, vec![block(0.0, &[], &groups)]),
            ],
        ),
    );
    let mut list = vec![key_info(KEY_WORN, BIG, 1.0); 256];
    list.extend(vec![key_info(KEY_DYE, BIG, 0.0); 256]);
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = list;
    let mut asm = assembler(s.clone());
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(H));
    assert!(c.parts.iter().all(|p| p.materials.is_empty()));
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);

    // A tighter layer budget cuts the sets and reports it; assembly still succeeds.
    let mut asm = assembler(s).with_limits(AssemblyLimits {
        max_total_layers: 1500,
        ..AssemblyLimits::default()
    });
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.count(Cat::LimitExceeded), 1, "{:?}", c.diagnostics);
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(H));
}

#[test]
fn defaults_and_offers_share_the_part_budget() {
    let all = [I_BACK, I_BOOTS, I_GLOVES, I_LEGS, I_CHEST, I_HEAD];
    let mut asm = assembler(world()).with_limits(AssemblyLimits {
        max_parts: 8,
        ..AssemblyLimits::default()
    });
    let c = asm.assemble_character(&human(&all)).unwrap();
    assert!(c.parts.len() <= 8, "{}", c.parts.len());
    assert!(c.count(Cat::LimitExceeded) >= 1);
    // The setup's six defaults alone may fill the budget.
    let mut asm = assembler(world()).with_limits(AssemblyLimits {
        max_parts: 6,
        ..AssemblyLimits::default()
    });
    let c = asm.assemble_character(&human(&all)).unwrap();
    assert_eq!(c.parts.len(), 6);
    assert!(c.parts.iter().all(|p| p.is_default()));
}
