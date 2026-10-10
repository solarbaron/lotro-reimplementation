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
    AssemblyError, AssemblyLimits, AssemblyOptions, AssemblyVocabulary, CharacterAssembler,
    CharacterSpec, DiagnosticCategory as Cat, EquippedItem, LayerKind, ListOrigin, MapSource,
    PlacementRule, RigKind, ScriptAppearance, ScriptMeshFx, SourceRef, StyleChoice, WeaponSlot,
    WieldState,
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
const L_BACK_SHIELD: u32 = 0x1000_0042;

// `Item_EquipmentCategory` is a bitfield: items store bit k − 1 for key k
// (property-system.md §7.3). Keys 10–13 and 21.
const C_1HSWORD: u64 = 1 << 9;
const C_2HAXE: u64 = 1 << 10;
const C_SHIELD: u64 = 1 << 11;
const C_ORB: u64 = 1 << 12;
const C_STAFF: u64 = 1 << 13;
const C_CLASS: u64 = 1 << 20;
const SHORT_SPECIES: u32 = 0x51;

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
const P_VISIBLE: u32 = 123;
const P_UPDATE: u32 = 124;
const P_SECONDARY_SCALE: u32 = 125;
const RANDOM_UNDEF: u32 = 0x1000_0001;

/// The §5.6 tables with the invented keys above (labels from the spec).
fn placement() -> PlacementRule {
    PlacementRule::from_labels(
        |l| match l {
            "Weapon_Primary" => Some(S_PRIMARY),
            "Weapon_Secondary" => Some(S_SECONDARY),
            "Weapon_Ranged" => Some(S_RANGED),
            _ => None,
        },
        |l| match l {
            "1HSword" => Some(C_1HSWORD),
            "2HAxe" => Some(C_2HAXE),
            "Shield" => Some(C_SHIELD),
            "Orb" => Some(C_ORB),
            "2HStaff" => Some(C_STAFF),
            _ => None,
        },
        |l| match l {
            "Hobbit" => Some(SHORT_SPECIES),
            _ => None,
        },
        |l| match l {
            "Sheath_LHip" => Some(L_LHIP),
            "Sheath_RHip" => Some(L_RHIP),
            "Sheath_Back_2H" => Some(L_BACK_2H),
            "Sheath_Back_Bow" => Some(L_BACK_BOW),
            "Sheath_Back_Shield" => Some(L_BACK_SHIELD),
            "Wpn_RHand" => Some(L_WPN_RHAND),
            "Wpn_LHand" => Some(L_LHAND),
            "Wpn_LOffHand" => Some(L_WPN_LOFFHAND),
            "Wpn_Forearm" => Some(L_FOREARM),
            _ => None,
        },
    )
}

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
        placement: placement(),
        avatar_update_visible: Some(P_UPDATE),
        npc_props_visible: Some(P_VISIBLE),
        secondary_scale: Some(P_SECONDARY_SCALE),
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
        ..AssemblyVocabulary::default()
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
        linked: Vec::new(),
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
        template_scale: Some([1.03; 3]),
        entity_scale: Some([0.7; 3]),
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
    spec.styles.push(StyleChoice::modifier(KEY_HAIR, 0.01));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
    assert!(!c.part_in(CREST).unwrap().is_default());
    // §3.2.1: a modifier between blocks takes the greatest variant not above it (0.01).
    spec.styles[0].modifier = 0.5;
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
    assert!(!c.part_in(CREST).unwrap().is_default());
    assert_eq!(c.count(Cat::NoBlock), 0);
    // Just below 0.01 takes 0.00: no tolerance.
    spec.styles[0].modifier = 0.009_999;
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(F));
    // −1.0 and a modifier below every block select nothing: the default part stays.
    for m in [-1.0, -0.5] {
        spec.styles[0].modifier = m;
        let c = asm.assemble_character(&spec).unwrap();
        assert!(c.part_in(CREST).unwrap().is_default(), "{m}");
        assert_eq!(c.count(Cat::NoBlock), 1, "{m}");
    }

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
fn dyes_join_the_figure_wide_group_list() {
    let mut s = world();
    s.items.get_mut(&Did(I_CHEST)).unwrap().clothing_color = Some(0.32);
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[I_CHEST])).unwrap();
    let chest = c.part_in(UPPER).unwrap();
    let kinds: Vec<(LayerKind, u32, f32)> = chest
        .materials
        .iter()
        .map(|l| (l.kind, l.material.0, l.priority))
        .collect();
    // §4.7 rule 3: the record's keys apply in ascending (key mod 23), so the dye key (1)
    // before the worn key (7). §4.10: one list by ascending group priority, ties in
    // application order: the dye block's UpperBody group (priority 0, the float stored
    // after the first group), then its All group (3), then the chest block's group (3).
    assert_eq!(KEY_DYE % 23, 1);
    assert_eq!(KEY_WORN % 23, 7);
    assert_eq!(
        kinds,
        vec![
            (LayerKind::Dye, 0x3000_0E32, 0.0),
            (LayerKind::Dye, 0x3000_0D32, 3.0),
            (LayerKind::Group, CHEST_REC + 0x1000_0000, 3.0),
        ]
    );
    assert_eq!(c.groups.len(), 3);
    // The UpperBody-only dye group does not reach the LowerBody stub, the others do.
    let stub = c.part_in(LOWER).unwrap();
    assert_eq!(stub.materials.len(), 2);
    // Groups apply to every matching part, default parts included (§4.10).
    let feet = c.part_in(FEET).unwrap();
    assert!(feet.is_default());
    assert_eq!(feet.materials.len(), 2);
    // A chosen dye replaces the default; 0.0 emits no dye element; a code between blocks
    // takes the last block below it (§3.2.1) and is noted.
    let mut spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_CHEST).dyed(0.06));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(
        c.part_in(UPPER).unwrap().materials[0].material,
        Did(0x3000_0D06)
    );
    spec.items[0].dye = Some(0.0);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().materials.len(), 1);
    spec.items[0].dye = Some(0.2);
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().materials[0].variant, 0.06);
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
        (Cat::NoPlacement, 1), // the class item: no placement script (§5.3, §5.6)
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
fn held_items_take_their_section_5_6_placement() {
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
    assert_eq!(sword.placement.scale, [1.0; 3]);
    let shield = &c.held[1];
    assert_eq!(
        shield.location, L_FOREARM,
        "wielded shields at Wpn_Forearm; the item's parenting location takes no part"
    );
    assert_eq!(shield.anchor, Anchor::Bone(BONE_LELBOW));
    assert_eq!(shield.frame.position, [0.05, 0.0, 0.0]);
    // Stowed: one-handed primary → left hip; two-handed axe → Sheath_Back_2H (missing on
    // this setup); an orb has no stowed placement.
    let mut spec = CharacterSpec::new(HUMAN, MALE);
    for i in [I_SWORD, I_AXE, I_ORB] {
        spec = spec.wearing(EquippedItem::new(i).with_state(WieldState::Sheathed));
    }
    let c = asm.assemble_character(&spec).unwrap();
    let locs: Vec<(u32, WieldState)> = c.held.iter().map(|h| (h.location, h.state)).collect();
    assert_eq!(
        locs,
        vec![(L_LHIP, WieldState::Sheathed)],
        "{:?}",
        c.diagnostics
    );
    assert_eq!(
        c.count(Cat::HoldingLocationMissing),
        1,
        "no Sheath_Back_2H on this setup"
    );
    assert_eq!(
        c.count(Cat::NoPlacement),
        1,
        "the orb is not drawn while stowed"
    );
    // Wielded, an orb is attached twice: right and left hand.
    let c = asm.assemble_character(&human(&[I_ORB])).unwrap();
    let locs: Vec<u32> = c.held.iter().map(|h| h.location).collect();
    assert_eq!(locs, vec![L_WPN_RHAND, L_LHAND]);
    // A stowed shield sits on the forearm, or on the back in the ranged skill mode, scaled
    // by the wearer's `Script_AdjustableScale_Secondary_Vector`.
    let mut s = world();
    s.bodies.get_mut(&Did(AVATAR)).unwrap().properties = set(vec![(
        P_SECONDARY_SCALE,
        PropertyValue::Vector3([1.1, 1.2, 1.3]),
    )]);
    let mut back = (*s.setups[&Did(SETUP)]).clone();
    back.bone_frames
        .push(location(L_BACK_SHIELD, BONE_HIP as i32, 0.0));
    s.setups.insert(Did(SETUP), Arc::new(back));
    let mut asm = assembler(s);
    let mut spec = CharacterSpec::new(HUMAN, MALE)
        .wearing(EquippedItem::new(I_SHIELD).with_state(WieldState::Sheathed));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held[0].location, L_FOREARM);
    assert_eq!(c.held[0].placement.scale, [1.1, 1.2, 1.3]);
    spec.ranged_skill_mode = true;
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held[0].location, L_BACK_SHIELD);
    // Mounted as well: nothing (§5.6, revised 2026-10-09).
    spec.mounted = true;
    let c = asm.assemble_character(&spec).unwrap();
    assert!(c.held.is_empty());
    assert_eq!(c.count(Cat::NoPlacement), 1);
    // A stowed staff: species rows with an offset and a half turn (§5.6).
    let mut s = world();
    s.items.insert(
        Did(0x7900_0020),
        held_item(S_PRIMARY, SWORD_ENTITY, C_STAFF),
    );
    let mut back = (*s.setups[&Did(SETUP)]).clone();
    back.bone_frames.push(location(L_BACK_2H, 16, 0.0));
    s.setups.insert(Did(SETUP), Arc::new(back));
    s.race.insert((SHORT_SPECIES, MALE), Did(AVATAR));
    let mut asm = assembler(s);
    for (species, y) in [(HUMAN, 0.4), (SHORT_SPECIES, 0.2)] {
        let spec = CharacterSpec::new(species, MALE)
            .wearing(EquippedItem::new(0x7900_0020).with_state(WieldState::Sheathed));
        let c = asm.assemble_character(&spec).unwrap();
        let h = &c.held[0];
        assert_eq!(h.location, L_BACK_2H);
        assert_eq!(h.placement.offset, [0.0, y, 0.0]);
        assert_eq!(h.placement.rotation, [0.0, 180.0, 0.0]);
        // Pitch 180° turns about the location's +X: y and z flip.
        let f = h.placement.applied_to(&h.frame);
        assert!((f.position[1] - y).abs() < 1e-6);
        assert!(
            f.rotation.x.abs() > 0.999 && f.rotation.w.abs() < 1e-6,
            "{f:?}"
        );
    }
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

/// `appearance.md` §5.6 "Stowed instruments" (A38): a ranged-slot instrument is attached
/// while stowed by the wearer's sheathed-instrument script (here a fixed answer of the
/// source), with the node's location, offset, rotations and scale; wielded it is not drawn.
#[test]
fn stowed_instruments_come_from_the_sheathed_instrument_script() {
    const C_INSTRUMENT: u64 = 1 << 21;
    const I_LUTE: u32 = 0x7900_0030;
    const KEY: u32 = 0x1000_2BA0;
    let mut s = world();
    s.items
        .insert(Did(I_LUTE), held_item(S_RANGED, SWORD_ENTITY, C_INSTRUMENT));
    let node = ScriptMeshFx {
        entity: Did(SWORD_ENTITY),
        location: L_BACK_BOW,
        offset: [0.0, -0.1, 0.02],
        rotation: [180.0, 20.0, 0.0],
        scale: [1.0; 3],
    };
    s.script_mesh_fx.insert((Did(AVATAR), KEY), vec![node]);
    let mut back = (*s.setups[&Did(SETUP)]).clone();
    back.bone_frames.push(location(L_BACK_BOW, 16, 0.0));
    s.setups.insert(Did(SETUP), Arc::new(back));
    let mut v = vocab();
    v.instrument_category = C_INSTRUMENT;
    v.sheathed_instrument_script = Some(KEY);
    let mut asm = CharacterAssembler::new(s, v);
    let spec = CharacterSpec::new(HUMAN, MALE)
        .wearing(EquippedItem::new(I_LUTE).with_state(WieldState::Sheathed));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.held.len(), 1, "{:?}", c.diagnostics);
    let h = &c.held[0];
    assert_eq!((h.location, h.item), (L_BACK_BOW, Some(Did(I_LUTE))));
    assert_eq!(h.placement.offset, [0.0, -0.1, 0.02]);
    assert_eq!(h.placement.rotation, [180.0, 20.0, 0.0]);
    let c = asm.assemble_character(&human(&[I_LUTE])).unwrap();
    assert!(c.held.is_empty(), "not drawn while wielded");
    assert_eq!(c.count(Cat::NoPlacement), 1);
}

/// `appearance.md` §5.5 "Creature cosmetic slots" (A39): a slot's prop is attached while
/// the props are visible and its `_Hide` is false; sheathed it goes to
/// `Sheath_Back_Shield` with the fixed values of its location, unsheathed to its own
/// location with its rotations and scale.
#[test]
fn creature_cosmetic_slots_follow_their_sheath_state() {
    use tessera_mesh::assembly::{CosmeticSlotPids, CosmeticSlotVocabulary};
    const P_SLOT_ENTITY: u32 = 130;
    const P_SLOT_LOC: u32 = 131;
    const P_SLOT_SCALE: u32 = 132;
    const P_SLOT_YAW: u32 = 133;
    const P_SLOT_HIDE: u32 = 134;
    const P_SHEATHED: u32 = 135;
    const PROP: u32 = 0x4700_0301;
    let mut s = world();
    s.chains
        .insert(Did(PROP), vec![Did(PROP), Did(SWORD_SETUP)]);
    let mut wearer = (*s.setups[&Did(SETUP)]).clone();
    wearer
        .bone_frames
        .push(location(L_BACK_SHIELD, BONE_HIP as i32, 0.0));
    s.setups.insert(Did(SETUP), Arc::new(wearer));
    let mut v = vocab();
    v.cosmetic = CosmeticSlotVocabulary {
        slots: vec![CosmeticSlotPids {
            entity: Some(P_SLOT_ENTITY),
            holding_location: Some(P_SLOT_LOC),
            scale: Some(P_SLOT_SCALE),
            yaw: Some(P_SLOT_YAW),
            hide: Some(P_SLOT_HIDE),
            ..CosmeticSlotPids::default()
        }],
        is_sheathed: Some(P_SHEATHED),
        sheathed: Some(1),
        combat_defined: Some(3),
        sheath_back_shield: Some(L_BACK_SHIELD),
        left_hand: Some(L_LHAND),
        left_off_hand: Some(L_WPN_LOFFHAND),
        right_hand: Some(L_WPN_RHAND),
        forearm: Some(L_FOREARM),
        ..CosmeticSlotVocabulary::default()
    };
    let mut asm = CharacterAssembler::new(s, v);
    let slot = |sheath: u32, hide: bool| {
        set(vec![
            (P_SLOT_ENTITY, PropertyValue::DidRef(Did(PROP))),
            (P_SLOT_LOC, PropertyValue::Enum(L_WPN_LOFFHAND)),
            (P_SLOT_SCALE, PropertyValue::Vector3([0.5, 0.6, 0.7])),
            (P_SLOT_YAW, PropertyValue::Float(45.0)),
            (P_SLOT_HIDE, PropertyValue::Bool(hide)),
            (P_SHEATHED, PropertyValue::Enum(sheath)),
        ])
    };
    // Unsheathed: its own location, rotation and scale, offset 0.
    let c = asm
        .assemble_object(Did(ENTITY), Some(&slot(2, false)))
        .unwrap();
    let h = c
        .held
        .iter()
        .find(|h| h.source == SourceRef::CosmeticSlot(1))
        .expect("slot prop");
    assert_eq!(h.location, L_WPN_LOFFHAND);
    assert_eq!(h.placement.rotation, [45.0, 0.0, 0.0]);
    assert_eq!(h.placement.scale, [0.5, 0.6, 0.7]);
    assert_eq!(h.placement.offset, [0.0; 3]);
    // Sheathed: from Wpn_LOffHand to Sheath_Back_Shield, offset (−0.15, 0.2, 0), yaw 210;
    // the slot's rotation is not used.
    let c = asm
        .assemble_object(Did(ENTITY), Some(&slot(1, false)))
        .unwrap();
    let h = c
        .held
        .iter()
        .find(|h| h.source == SourceRef::CosmeticSlot(1))
        .expect("slot prop");
    assert_eq!(h.location, L_BACK_SHIELD);
    assert_eq!(h.placement.offset, [-0.15, 0.2, 0.0]);
    assert_eq!(h.placement.rotation, [210.0, 0.0, 0.0]);
    assert_eq!(h.placement.scale, [0.5, 0.6, 0.7]);
    // Hidden: not drawn.
    let c = asm
        .assemble_object(Did(ENTITY), Some(&slot(1, true)))
        .unwrap();
    assert!(
        !c.held
            .iter()
            .any(|h| h.source == SourceRef::CosmeticSlot(1))
    );
}

/// `appearance.md` §3.3 Rule (confirmed 2026-10-09): a toggle property other than
/// `GOP_AvatarUpdateVisible` that the figure does not carry reads its dictionary default.
#[test]
fn absent_toggle_properties_read_their_default() {
    const P_TOGGLE: u32 = 140;
    const ALT_REC: u32 = 0x2000_0300;
    let toggled = |default: Option<bool>| {
        let mut s = world();
        if let Some(d) = default {
            s.defaults.insert(P_TOGGLE, PropertyValue::Bool(d));
        }
        s.appearances.insert(
            Did(ALT_REC),
            record(
                ALT_REC,
                vec![(KEY_HAIR, vec![block(0.0, &[(1.0, HELMET, G)], &[])])],
            ),
        );
        let body = s.bodies.get_mut(&Did(AVATAR)).unwrap();
        for e in &mut body.class_list {
            if e.key == KEY_HAIR {
                e.toggle_file = Some(Did(ALT_REC));
                e.toggle_property = Some(P_TOGGLE);
            }
        }
        let mut asm = assembler(s);
        let c = asm.assemble_character(&human(&[])).unwrap();
        c.parts.iter().any(|p| p.mesh == Did(G) && !p.hidden)
    };
    assert!(
        toggled(Some(true)),
        "the default true picks the toggle file"
    );
    assert!(!toggled(Some(false)));
    assert!(!toggled(None), "no default: false");
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
    assert_eq!(c.model_scale(), [1.0; 3]);
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
    // The ClothingColor element's group joins the figure-wide list (§4.10).
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
    assert!((c.model_scale()[0] - 1.03 * 0.7 * 0.9).abs() < 1e-6);

    // A random-appearance figure keeps its lists (§7.4 Rule); without the script elements
    // that choose its outfit it is reported.
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
    assert_eq!(
        c.part_in(UPPER).unwrap().mesh,
        Did(PIECE),
        "the list applies"
    );
    assert_eq!(c.held.len(), 1, "held props are kept");
    // With the Appearance nodes its start-up scripts ran: applied after the lists, so an
    // equal-priority script piece wins (§4.7 rules 1–2).
    const SCRIPT_REC: u32 = 0x2000_0210;
    const SCRIPT_PIECE: u32 = 0x0600_0211;
    asm.source_mut().meshes.insert(
        Did(SCRIPT_PIECE),
        mesh_summary(SCRIPT_PIECE, UPPER, 4000, true),
    );
    asm.source_mut().appearances.insert(
        Did(SCRIPT_REC),
        record(
            SCRIPT_REC,
            vec![(
                KEY_WORN,
                vec![block(1.0, &[(1.0, UPPER, SCRIPT_PIECE)], &[])],
            )],
        ),
    );
    let scripts = [ScriptAppearance {
        record: Did(SCRIPT_REC),
        key: KEY_WORN,
        modifier: 1.0,
    }];
    let c = asm
        .assemble_object_with(Did(ENTITY), Some(&instance), &scripts)
        .unwrap();
    assert_eq!(c.count(Cat::RandomAppearance), 0);
    let upper = c.part_in(UPPER).unwrap();
    assert_eq!(upper.mesh, Did(SCRIPT_PIECE));
    assert_eq!(upper.source(), Some(SourceRef::Script(0)));
    // A script element naming a record the lists named keeps the record's first position:
    // here the NPC record (list element 0) comes first again, so the script's own record,
    // applied after it, still wins the tie.
    let scripts = [
        ScriptAppearance {
            record: Did(SCRIPT_REC),
            key: KEY_WORN,
            modifier: 1.0,
        },
        ScriptAppearance {
            record: Did(NPC_REC),
            key: KEY_WORN,
            modifier: 1.0,
        },
    ];
    let c = asm
        .assemble_object_with(Did(ENTITY), Some(&instance), &scripts)
        .unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(SCRIPT_PIECE));

    // A lone dye element recolours every matching part, default parts included (§4.10:
    // no pairing, no report).
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_DYE, NPC_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert_eq!(c.random_appearance, None);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let upper = c.part_in(UPPER).unwrap();
    assert!(upper.is_default());
    assert_eq!(upper.materials[0].material, Did(0x3000_0A00));

    // `NPC_Cosmetic_Inventory_Visible` false hides the props (§5.5 Rule).
    let instance = set(vec![
        (P_NPC_ITEM1, PropertyValue::DidRef(Did(PROP))),
        (P_NPC_LOC1, PropertyValue::Enum(L_LHAND)),
        (P_VISIBLE, PropertyValue::Bool(false)),
    ]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert!(c.held.is_empty());
    assert_eq!(c.count(Cat::PropsHidden), 1);

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
    assert!((c.model_scale()[2] - 1.03 * 0.7).abs() < 1e-6);
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
            ("Inventory_DefaultSlot", "Weapon_Ranged", 7),
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
    let rule = &v.placement;
    let primary = 1 << 4;
    let secondary = 1 << 5;
    let (sword_1h, sword_2h, club_2h, shield, medium_shield) =
        (1 << 1, 1 << 8, 1 << 10, 1 << 20, 1u64 << 45);
    assert_eq!(rule.slot_bits[0], primary);
    assert_eq!(rule.slot_bits[1], secondary);
    assert_eq!(rule.weapon_slot(primary), Some(WeaponSlot::Primary));
    assert_eq!(rule.weapon_slot(secondary), Some(WeaponSlot::Secondary));
    assert_eq!(rule.weapon_slot(1 << 9), None);
    let stowed = |category: u64, slot: u64| {
        rule.row(slot, category, 0, false, true)
            .and_then(|r| r.placements.first())
            .map(|p| p.location)
    };
    assert_eq!(stowed(sword_2h, primary), Some(BACK_2H_SWORD), "2HSword");
    assert_eq!(stowed(club_2h, primary), Some(BACK_2H), "2HClub");
    assert_eq!(stowed(sword_1h, primary), Some(LHIP), "1HSword: left hip");
    assert_eq!(
        stowed(sword_1h, secondary),
        Some(RHIP),
        "1HSword in the secondary slot: right hip"
    );
    // Shields stay on the forearm out of the ranged skill mode (no forearm label here, so
    // the row draws nothing), on the back in it; bits above 31 are kept.
    for c in [shield, medium_shield] {
        let row = rule.row(secondary, c, 0, false, true).expect("shield row");
        assert!(row.placements.is_empty());
        assert_eq!(
            rule.row(secondary, c, 0, true, true)
                .and_then(|r| r.placements.first())
                .map(|p| p.location),
            Some(BACK_SHIELD)
        );
    }
    // A bow is a ranged item: stowed on the back in the ranged slot, and in the primary
    // slot no row matches.
    let bow = 1 << 13;
    assert_eq!(stowed(bow, 1 << 6), Some(BACK_BOW));
    assert!(rule.row(primary, bow, 0, false, true).is_none());
    assert!(rule.row(primary, 0, 0, false, true).is_none());
    assert!(
        rule.row(primary, 1 << 30, 0, false, true).is_none(),
        "unknown"
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
fn material_only_list_blocks_join_the_figure_wide_list() {
    // A selection table of NPC pieces (appearance.md §4.11): material-only blocks under a
    // key other than ClothingColor are not paired with anything; their groups join the
    // figure-wide list and apply to every matching part (§4.10, §15).
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
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    // The default lower body takes it too.
    assert_eq!(
        c.part_in(LOWER).unwrap().materials[0].material,
        Did(0x3000_5E01)
    );

    // Alone: onto the default parts, without a report.
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_SEL, SEL_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let upper = c.part_in(UPPER).unwrap();
    assert!(upper.is_default());
    assert_eq!(upper.materials[0].material, Did(0x3000_5E00));
    assert_eq!(upper.materials[0].kind, LayerKind::Group);

    // A block with neither parts nor groups changes nothing.
    let instance = set(vec![(P_INSTANCE, list_value(&[(KEY_EMPTY, SEL_REC, 0.0)]))]);
    let c = asm.assemble_object(Did(ENTITY), Some(&instance)).unwrap();
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    assert!(c.groups.is_empty());
}

#[test]
fn a_record_named_again_keeps_its_first_position() {
    // §4.7 rule 2: the class list names the chest record (piece and dye 0.32); a worn item
    // on the same record merges into that record's place: its key replaces the piece's
    // modifier and source, its dye element replaces the dye's modifier (§4.9, §4.10).
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
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(
        c.part_in(UPPER).unwrap().source(),
        Some(SourceRef::ClassList(0))
    );
    assert_eq!(
        layers(&c),
        vec![
            (LayerKind::Dye, 0x3000_0E32),
            (LayerKind::Dye, 0x3000_0D32),
            (LayerKind::Group, CHEST_REC + 0x1000_0000),
        ]
    );
    let spec = CharacterSpec::new(HUMAN, MALE).wearing(EquippedItem::new(I_CHEST).dyed(0.06));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().source(), Some(SourceRef::Item(0)));
    assert_eq!(
        layers(&c),
        vec![
            (LayerKind::Dye, 0x3000_0D06),
            (LayerKind::Group, CHEST_REC + 0x1000_0000),
        ]
    );
}

#[test]
fn keys_of_one_record_apply_in_key_table_order() {
    // §4.7 rule 3: ascending (key mod 23); among keys of one remainder the one added last
    // first. Two keys with remainder 4 offer equal-priority parts for one slot: the key
    // added first is applied last and wins.
    const REC: u32 = 0x2000_0500;
    let (k1, k2) = (23 * 1000 + 4, 23 * 2000 + 4);
    let mut s = world();
    s.appearances.insert(
        Did(REC),
        record(
            REC,
            vec![
                (k1, vec![block(1.0, &[(5.0, UPPER, H)], &[])]),
                (k2, vec![block(1.0, &[(5.0, UPPER, J)], &[])]),
            ],
        ),
    );
    s.meshes.insert(Did(J), mesh_summary(J, UPPER, 700, true));
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list =
        vec![key_info(k1, REC, 1.0), key_info(k2, REC, 1.0)];
    let mut asm = assembler(s.clone());
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(H));
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list =
        vec![key_info(k2, REC, 1.0), key_info(k1, REC, 1.0)];
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(J));
}

#[test]
fn linked_records_apply_first_and_cycles_stop() {
    // §4.2, §4.7 rule 4: a record's linked records take the same key and modifier first;
    // a record already on the path is skipped and reported (Tessera Decision).
    const OWN: u32 = 0x2000_0600;
    const LINKED: u32 = 0x2000_0601;
    let mut s = world();
    let mut own = (*record(
        OWN,
        vec![(KEY_WORN, vec![block(1.0, &[(5.0, UPPER, H)], &[])])],
    ))
    .clone();
    own.linked = vec![Did(LINKED), Did(0x2000_0BAD)];
    let mut linked = (*record(
        LINKED,
        vec![(
            KEY_WORN,
            vec![block(1.0, &[(5.0, UPPER, A)], &[(ALL, 0x3000_0001)])],
        )],
    ))
    .clone();
    linked.linked = vec![Did(OWN)];
    s.appearances.insert(Did(OWN), Arc::new(own));
    s.appearances.insert(Did(LINKED), Arc::new(linked));
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = vec![key_info(KEY_WORN, OWN, 1.0)];
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[])).unwrap();
    // The linked record's part came first; the record's own part, at equal priority, wins.
    assert_eq!(c.part_in(UPPER).unwrap().mesh, Did(H));
    assert!(!c.part_in(UPPER).unwrap().is_default());
    assert_eq!(
        c.groups.len(),
        1,
        "the linked block's group joined the list"
    );
    assert_eq!(c.count(Cat::LinkedRecordCycle), 1, "{:?}", c.diagnostics);
    assert_eq!(
        c.count(Cat::AppearanceMissing),
        1,
        "the missing link is skipped"
    );
}

#[test]
fn toggle_files_follow_the_option_store() {
    // §3.3: the toggle file replaces the file when the toggle property reads true through
    // the figure's lookup; the option store answers the updated-avatar option first.
    const PLAIN: u32 = 0x2000_0700;
    const TOGGLE: u32 = 0x2000_0701;
    let mut s = world();
    for (did, mesh) in [(PLAIN, F), (TOGGLE, E)] {
        s.appearances.insert(
            Did(did),
            record(
                did,
                vec![(KEY_HAIR, vec![block(0.0, &[(1.0, CREST, mesh)], &[])])],
            ),
        );
    }
    let element = |property: u32| AppearanceKeyInfo {
        toggle_file: Some(Did(TOGGLE)),
        toggle_property: Some(property),
        ..key_info(KEY_HAIR, PLAIN, 0.0)
    };
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = vec![element(P_UPDATE)];
    let mut asm = assembler(s.clone());
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(
        c.part_in(CREST).unwrap().mesh,
        Did(E),
        "option on by default"
    );
    assert!(!c.part_in(CREST).unwrap().is_default());
    assert_eq!(c.watched_properties, vec![P_UPDATE]);
    let mut asm = assembler(s.clone()).with_options(AssemblyOptions {
        avatar_update_visible: false,
        ..AssemblyOptions::default()
    });
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(
        c.part_in(CREST).unwrap().mesh,
        Did(F),
        "option off: the plain file"
    );
    // Another toggle property reads through the figure's own sets; absent reads false.
    const OTHER: u32 = 130;
    s.bodies.get_mut(&Did(AVATAR)).unwrap().class_list = vec![element(OTHER)];
    let mut asm = assembler(s.clone());
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(F));
    s.bodies.get_mut(&Did(AVATAR)).unwrap().properties =
        set(vec![(OTHER, PropertyValue::Bool(true))]);
    let mut asm = assembler(s);
    let c = asm.assemble_character(&human(&[])).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
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
    // The class list and the leggings name the legs record with one key: one record use
    // (§4.7 rule 2), applied once, so the mesh is reported once.
    assert_eq!(c.count(Cat::MeshUndecodable), 1, "{:?}", c.diagnostics);
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
    // groups each, none applying to a part (the worst case of the layer scan). They merge
    // into one record use with two keys (§4.7 rule 2), so each block is applied once.
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

/// `appearance.md` §3.4 "Starting values on the creation screen": a style control writes an
/// instance-list element with its own record (and toggle record), replacing the instance
/// list's elements of its key in place or being appended; the class list keeps its own.
#[test]
fn written_style_elements_replace_or_join_the_instance_list() {
    const HAIR2_REC: u32 = 0x2000_0101;
    const HAIR2: u32 = 0x0600_00E2;
    let mut s = world();
    s.meshes
        .insert(Did(HAIR2), mesh_summary(HAIR2, CREST, 1600, true));
    s.appearances.insert(
        Did(HAIR2_REC),
        record(
            HAIR2_REC,
            vec![(KEY_HAIR, vec![block(0.0, &[(1.0, CREST, HAIR2)], &[])])],
        ),
    );
    let written = |file: u32, modifier: f32, toggle: Option<u32>| {
        StyleChoice::element(AppearanceKeyInfo {
            key: KEY_HAIR,
            file: Did(file),
            modifier,
            toggle_file: toggle.map(Did),
            toggle_property: toggle.map(|_| P_UPDATE),
        })
    };
    // Appended: the entity has no instance list; the written element follows the class
    // list's hair and wins the slot.
    let mut asm = assembler(s.clone());
    let mut spec = human(&[]);
    spec.styles.push(written(HAIR2_REC, 0.0, None));
    let c = asm.assemble_character(&spec).unwrap();
    let crest = c.part_in(CREST).unwrap();
    assert_eq!(crest.mesh, Did(HAIR2));
    assert_eq!(crest.source(), Some(SourceRef::InstanceList(0)));
    // The toggle record replaces the file while the updated-avatar option is on (§3.3).
    let mut spec = human(&[]);
    spec.styles.push(written(HAIR2_REC, 0.01, Some(HAIR_REC)));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
    assert!(c.watched_properties.contains(&P_UPDATE));
    // In place: an instance-list element of the key is replaced, not doubled; a later
    // written element replaces an earlier one.
    s.bodies.get_mut(&Did(AVATAR)).unwrap().instance_list =
        vec![key_info(KEY_HAIR, HAIR_REC, 0.01)];
    let mut asm = assembler(s);
    let mut spec = human(&[]);
    spec.styles.push(written(HAIR_REC, 0.01, None));
    spec.styles.push(written(HAIR2_REC, 0.0, None));
    let c = asm.assemble_character(&spec).unwrap();
    let crest = c.part_in(CREST).unwrap();
    assert_eq!(crest.mesh, Did(HAIR2));
    assert_eq!(crest.source(), Some(SourceRef::InstanceList(0)));
    // A modifier-only choice still replaces every list's modifier of its key.
    let mut spec = human(&[]);
    spec.styles.push(StyleChoice::modifier(KEY_HAIR, 0.01));
    let c = asm.assemble_character(&spec).unwrap();
    assert_eq!(c.part_in(CREST).unwrap().mesh, Did(E));
}
