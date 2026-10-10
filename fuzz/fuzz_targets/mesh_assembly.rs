// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-mesh` assembly (`docs/specs/appearance.md` §8): `CharacterAssembler` over
//! synthetic records built from the input.
//!
//! The input describes a small world held in a `MapSource`: part meshes (slot, stub or
//! not, some missing), a setup with holding locations, appearance records (entries,
//! blocks, parts with arbitrary priorities including NaN and infinities, material groups,
//! extras, linked records that may name each other or themselves, §4.7 rule 4), a body
//! with template, class and instance lists naming present, missing and null records (some
//! with toggle files and toggle properties, §3.3), worn and held items with dyes, wield
//! states and slots (the §5.6 placement rows built from synthetic labels), entity chains
//! (some broken), held-entity scales, script elements (§7.4), and an instance property set
//! with NPC props, their visibility flag and a scale. Some DIDs are marked undecodable.
//! Every entry point runs with tight limits and both option values. A stress mode gives
//! blocks many material groups and fills the lists with many elements naming the same few
//! records (the merge path of §4.7 rule 2 and the group-list bound).
//!
//! Nothing may panic, and the result must respect the limits and the slot rules: at most
//! one offered part per slot, hidden exactly when the winning mesh is a stub, held parts
//! only at locations the setup has, diagnostics within the bound, at most `max_parts`
//! parts, the figure-wide group list sorted by priority and within its bound, a model
//! scale that passed the client's check.

#![no_main]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceExtra, AppearanceKeyInfo, AppearanceListPids,
    AppearancePart, AppearanceRecord, MaterialGroup,
};
use tessera_formats::avatar::AvatarBody;
use tessera_formats::item::{HoldingRule, ItemVisual, WardrobeFallbacks, WornAppearanceRef};
use tessera_formats::setup::{BoneFrame, Frame};
use tessera_formats::{
    Did, PropertyArray, PropertyEntry, PropertySet, PropertyStruct, PropertyValue, Setup, TSize,
};
use tessera_mesh::assembly::{
    AssemblyLimits, AssemblyOptions, AssemblyVocabulary, CharacterAssembler, CharacterSpec,
    CosmeticSlotPids, CosmeticSlotVocabulary, EquippedItem, MapSource, PlacementRule,
    ScriptAppearance, ScriptMeshFx, SourceRef, StyleChoice, WieldState,
};
use tessera_mesh::{MeshSummary, SubmeshSummary, VertexFormat};

const SLOTS: [u32; 10] = [
    0x1000_000C,
    0x1000_0003,
    0x1000_0001,
    0x1000_0002,
    0x1000_000E,
    0x1000_000A,
    0x1000_000B,
    1,
    2,
    0,
];
const KEYS: [u32; 5] = [
    0x1000_003A,
    0x1000_0032,
    0x1000_0034,
    0x1000_0022,
    0x1000_0099,
];
const VARIANTS: [f32; 8] = [0.0, 0.01, 0.06, 0.32, 0.5, 1.0, 1.0004, f32::NAN];
const PRIORITIES: [f32; 10] = [
    0.0,
    1.0,
    2.0,
    5.0,
    10.0,
    15.0,
    20.0,
    f32::NAN,
    f32::INFINITY,
    -1.0,
];
const LOCATIONS: [u32; 6] = [1, 2, 0x1000_003B, 0x1000_003C, 0x1000_003D, 0x1000_003F];
const SLOT_BITS: [u64; 10] = [
    0x2, 0x4, 0x8, 0x10, 0x20, 0x80, 0x1_0000, 0x2_0000, 0x4_0000, 0x10_0000,
];
const MESHES: u32 = 24;
const RECORDS: u32 = 8;
const ITEMS: u32 = 10;
const SPECIES: u32 = 0x17;
const SEX: u32 = 0x1000;
const AVATAR: Did = Did(0x7000_0001);
const SETUP: Did = Did(0x0400_0001);

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }
    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        from[usize::from(self.byte()) % from.len()]
    }
    fn small(&mut self, max: u8) -> usize {
        usize::from(self.byte() % (max + 1))
    }
    fn chance(&mut self, in_8: u8) -> bool {
        self.byte() % 8 < in_8
    }
    /// A float from four bytes (any bit pattern, NaN and infinities included).
    fn float(&mut self) -> f32 {
        f32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
}

fn mesh_did(i: u32) -> Did {
    Did(0x0600_0000 + i)
}

fn record_did(b: &mut Bytes<'_>) -> Did {
    match b.byte() % 10 {
        0 => Did(0),
        1 => Did(0x2000_0BAD),
        n => Did(0x2000_0000 + u32::from(n) % RECORDS),
    }
}

fn block(b: &mut Bytes<'_>, stress: bool) -> AppearanceBlock {
    let parts = (0..b.small(5))
        .map(|_| AppearancePart {
            part_param: b.pick(&PRIORITIES),
            slot: b.pick(&SLOTS),
            mesh: mesh_did(u32::from(b.byte()) % (MESHES + 4)),
        })
        .collect();
    let n_groups = if stress {
        40 + b.small(200)
    } else {
        b.small(3)
    };
    let groups = (0..n_groups)
        .map(|_| MaterialGroup {
            shader: Did(u32::from(b.byte() & 1) * 0x2B00_0001),
            material: Did(0x3000_0000 + u32::from(b.byte())),
            a: b.pick(&SLOTS),
            binding_key: u32::from(b.byte()),
            g: b.pick(&PRIORITIES),
        })
        .collect();
    let extras = if b.chance(1) {
        vec![AppearanceExtra {
            kind: 9,
            slot: 0,
            did: Did(0x0400_0002),
        }]
    } else {
        vec![]
    };
    AppearanceBlock {
        variant: b.pick(&VARIANTS),
        group_param: b.pick(&PRIORITIES),
        groups,
        parts,
        extras,
    }
}

fn list(b: &mut Bytes<'_>, stress: bool) -> Vec<AppearanceKeyInfo> {
    let n = if stress { 20 + b.small(60) } else { b.small(5) };
    (0..n)
        .map(|_| {
            let toggle = b.chance(2);
            AppearanceKeyInfo {
                key: if b.chance(1) { 0 } else { b.pick(&KEYS) },
                file: record_did(b),
                modifier: b.pick(&VARIANTS),
                toggle_file: toggle.then(|| record_did(b)),
                toggle_property: toggle.then(|| b.pick(&[30u32, 31])),
            }
        })
        .collect()
}

fn list_value(elements: &[AppearanceKeyInfo]) -> PropertyValue {
    let el = |e: &AppearanceKeyInfo| PropertyEntry {
        pid: 0,
        value: PropertyValue::Struct(PropertyStruct {
            size: TSize { hint: 0, count: 3 },
            fields: vec![
                PropertyEntry {
                    pid: 3,
                    value: PropertyValue::Enum(e.key),
                },
                PropertyEntry {
                    pid: 4,
                    value: PropertyValue::DidRef(e.file),
                },
                PropertyEntry {
                    pid: 5,
                    value: PropertyValue::Float(e.modifier),
                },
            ],
        }),
    };
    PropertyValue::Array(PropertyArray {
        count: elements.len() as u32,
        elements: elements.iter().map(el).collect(),
    })
}

fn vocab() -> AssemblyVocabulary {
    AssemblyVocabulary {
        lists: Some(AppearanceListPids {
            class_list: Some(1),
            instance_list: Some(2),
            key: 3,
            file: 4,
            modifier: Some(5),
            toggle_file: None,
            toggle_property: None,
        }),
        clothing_color: Some(KEYS[2]),
        clothing_color_secondary: Some(KEYS[3]),
        upper_body: Some(SLOTS[0]),
        fallbacks: WardrobeFallbacks {
            pairs: vec![(0x72, SPECIES), (SPECIES, 0x72)],
        },
        holding: HoldingRule {
            slot_locations: vec![(0x1_0000, LOCATIONS[2]), (0x2_0000, LOCATIONS[3])],
        },
        // The §5.6 rows with synthetic keys: category bits 0–4 and 32 (keys 1–5, 33).
        placement: PlacementRule::from_labels(
            |l| match l {
                "Weapon_Primary" => Some(0x1_0000),
                "Weapon_Secondary" => Some(0x2_0000),
                "Weapon_Ranged" => Some(0x4_0000),
                _ => None,
            },
            |l| match l {
                "1HSword" => Some(0x1),
                "2HSword" => Some(0x2),
                "2HStaff" => Some(0x4),
                "Orb" => Some(0x8),
                "Shield" => Some(0x10),
                "Bow" => Some(1 << 32),
                _ => None,
            },
            |l| (l == "Hobbit").then_some(0x51),
            |l| match l {
                "Sheath_LHip" => Some(LOCATIONS[5]),
                "Sheath_RHip" => Some(LOCATIONS[0]),
                "Sheath_Back_2H" | "Sheath_Back_2H_Sword" => Some(LOCATIONS[4]),
                "Sheath_Back_Bow" => Some(LOCATIONS[1]),
                "Wpn_RHand" => Some(LOCATIONS[2]),
                "Wpn_LHand" | "Wpn_LOffHand" => Some(LOCATIONS[3]),
                "Wpn_Forearm" => Some(LOCATIONS[1]),
                _ => None,
            },
        ),
        avatar_update_visible: Some(30),
        avatar_face_compositing: Some(97),
        npc_props_visible: Some(23),
        secondary_scale: Some(24),
        npc_items: vec![(10, Some(11)), (12, Some(13)), (14, None)],
        idle_animation: Some(20),
        random_appearance_type: Some(21),
        random_appearance_none: vec![0, 1],
        render_scale: Some(22),
        bone_scale: None,
        avatar_height: None,
        slider_values: None,
        // §5.6 stowed instruments (category bit 5) and §5.5 creature cosmetic slots.
        instrument_category: 0x20,
        sheathed_instrument_script: Some(0x1000_2BA0),
        item_instrument_type: Some(25),
        cached_instrument_type: Some(26),
        physobj_ranged: Some(27),
        cosmetic: CosmeticSlotVocabulary {
            slots: vec![CosmeticSlotPids {
                entity: Some(31),
                holding_location: Some(32),
                scale: Some(33),
                yaw: Some(34),
                pitch: Some(35),
                roll: Some(36),
                hide: Some(37),
            }],
            is_sheathed: Some(38),
            sheathed: Some(1),
            combat_defined: Some(3),
            combat_violent: Some(39),
            life_state: Some(40),
            life_state_down: vec![2],
            classification: Some(41),
            monster_player: Some(1),
            sheath_back_shield: Some(LOCATIONS[0]),
            left_hand: Some(LOCATIONS[3]),
            left_off_hand: Some(LOCATIONS[3]),
            right_hand: Some(LOCATIONS[2]),
            forearm: Some(LOCATIONS[1]),
        },
    }
}

fn world(b: &mut Bytes<'_>, stress: bool) -> (MapSource, PropertySet, CharacterSpec) {
    let mut s = MapSource::default();
    for i in 0..MESHES {
        if b.chance(1) {
            continue; // missing mesh
        }
        let vertices = if b.chance(2) {
            3
        } else {
            4 + u32::from(b.byte())
        };
        let format = if b.chance(6) {
            0x3002_0252
        } else {
            0x0000_0112
        };
        s.meshes.insert(
            mesh_did(i),
            MeshSummary {
                did: mesh_did(i),
                flags: b.pick(&SLOTS),
                surfaces: vec![Did(0x3100_0001)],
                submeshes: vec![SubmeshSummary {
                    format: VertexFormat::new(format).unwrap(),
                    vertex_count: vertices,
                    index_count: 3,
                }],
            },
        );
    }
    let setup = Setup {
        did: SETUP,
        tagfile: vec![b.byte(); b.small(16)],
        parts: (0..b.small(12))
            .map(|_| mesh_did(u32::from(b.byte()) % (MESHES + 2)))
            .collect(),
        bone_frames: (0..b.small(6))
            .map(|_| {
                let key = b.pick(&LOCATIONS);
                BoneFrame {
                    key,
                    key2: key,
                    bone: i32::from(b.byte() as i8),
                    frame: Frame::IDENTITY,
                }
            })
            .collect(),
        ..Setup::default()
    };
    s.setups.insert(SETUP, Arc::new(setup));
    for r in 0..RECORDS {
        let did = Did(0x2000_0000 + r);
        let entries = (0..b.small(4))
            .map(|_| AppearanceEntry {
                key: b.pick(&KEYS),
                blocks: (0..b.small(4)).map(|_| block(b, stress)).collect(),
            })
            .collect();
        let linked = (0..b.small(2)).map(|_| record_did(b)).collect();
        s.appearances.insert(
            did,
            Arc::new(AppearanceRecord {
                did,
                linked,
                section: 0,
                entries,
                key_table: vec![],
            }),
        );
    }
    let body = AvatarBody {
        object: Some(AVATAR),
        entity: Did(0x4700_0001),
        chain: vec![Did(0x4700_0001), SETUP],
        setup: SETUP,
        template_scale: Some([b.pick(&PRIORITIES); 3]),
        entity_scale: b
            .chance(3)
            .then(|| [b.pick(&PRIORITIES), 1.0, b.pick(&PRIORITIES)]),
        template_appearance: list(b, false),
        class_list: list(b, stress),
        instance_list: list(b, stress),
        properties: PropertySet::default(),
        bone_slider_definitions: None,
        slider_values: b
            .chance(4)
            .then(|| "80zz80".chars().take(b.small(6)).collect()),
    };
    s.bodies.insert(AVATAR, body);
    s.race.insert((SPECIES, SEX), AVATAR);
    // The sheathed-instrument script's attachments (§5.6), fuzzed.
    let attachments = (0..b.byte() % 4)
        .map(|_| ScriptMeshFx {
            entity: Did(0x4700_0100 + u32::from(b.byte() % 5)),
            location: b.pick(&LOCATIONS),
            offset: [b.float(), b.float(), b.float()],
            rotation: [b.float(), b.float(), b.float()],
            scale: [b.float(), b.float(), b.float()],
        })
        .collect();
    s.script_mesh_fx.insert((AVATAR, 0x1000_2BA0), attachments);
    // Held entities 0x47000100.. with chains; some broken.
    for e in 0..4u32 {
        let entity = Did(0x4700_0100 + e);
        if b.chance(6) {
            let setup = Did(0x0400_0100 + e);
            s.chains.insert(entity, vec![entity, setup]);
            if b.chance(3) {
                s.held_scales.insert(entity, [b.pick(&PRIORITIES); 3]);
            }
            if b.chance(7) {
                s.setups.insert(
                    setup,
                    Arc::new(Setup {
                        did: setup,
                        parts: vec![mesh_did(e)],
                        ..Setup::default()
                    }),
                );
            }
        }
    }
    let mut spec = CharacterSpec::new(SPECIES, SEX);
    for i in 0..ITEMS {
        let did = Did(0x7900_0000 + i);
        let worn = (0..b.small(2))
            .map(|_| WornAppearanceRef {
                appearance: record_did(b),
                key: b.pick(&KEYS),
                species: b.pick(&[SPECIES, 0x72, 0x51]),
                sex: SEX,
            })
            .collect();
        s.items.insert(
            did,
            ItemVisual {
                worn,
                phys_obj: b
                    .chance(5)
                    .then(|| Did(0x4700_0100 + u32::from(b.byte() % 5))),
                default_slot: b.chance(7).then(|| b.pick(&SLOT_BITS)),
                parenting_location: b.chance(1).then(|| b.pick(&LOCATIONS)),
                paperdoll_parenting_location: None,
                // A mask of category bits 0–3 (keys 1–4).
                equipment_category: Some(u64::from(b.byte() % 16)),
                clothing_color: b.chance(4).then(|| b.pick(&VARIANTS)),
                clothing_color_secondary: b.chance(2).then(|| b.pick(&VARIANTS)),
            },
        );
    }
    for _ in 0..b.small(12) {
        let mut item = EquippedItem::new(Did(0x7900_0000 + u32::from(b.byte()) % (ITEMS + 1)));
        if b.chance(3) {
            item = item.dyed(b.pick(&VARIANTS));
        }
        if b.chance(4) {
            item = item.with_state(WieldState::Sheathed);
        }
        if b.chance(2) {
            item = item.in_slot(b.pick(&SLOT_BITS));
        }
        spec = spec.wearing(item);
    }
    spec.ranged_skill_mode = b.chance(3);
    for _ in 0..b.small(2) {
        spec.styles.push(StyleChoice {
            key: b.pick(&KEYS),
            modifier: b.pick(&VARIANTS),
        });
    }
    let mut entries = Vec::new();
    if b.chance(4) {
        entries.push(PropertyEntry {
            pid: 2,
            value: list_value(&list(b, stress)),
        });
    }
    // Some DIDs exist but do not decode.
    for _ in 0..b.small(3) {
        let did = match b.byte() % 3 {
            0 => mesh_did(u32::from(b.byte()) % MESHES),
            1 => Did(0x2000_0000 + u32::from(b.byte()) % RECORDS),
            _ => Did(0x7900_0000 + u32::from(b.byte()) % ITEMS),
        };
        s.invalid.insert(did);
    }
    for pid in [10u32, 12, 14] {
        if b.chance(4) {
            entries.push(PropertyEntry {
                pid,
                value: PropertyValue::DidRef(Did(0x4700_0100 + u32::from(b.byte() % 5))),
            });
            entries.push(PropertyEntry {
                pid: pid + 1,
                value: PropertyValue::Enum(b.pick(&LOCATIONS)),
            });
        }
    }
    entries.push(PropertyEntry {
        pid: 21,
        value: PropertyValue::Enum(u32::from(b.byte() % 4)),
    });
    entries.push(PropertyEntry {
        pid: 22,
        value: PropertyValue::Vector3([b.pick(&PRIORITIES); 3]),
    });
    if b.chance(4) {
        entries.push(PropertyEntry {
            pid: 23,
            value: PropertyValue::Bool(b.chance(4)),
        });
    }
    if b.chance(2) {
        entries.push(PropertyEntry {
            pid: 31,
            value: PropertyValue::Bool(b.chance(4)),
        });
    }
    let instance = PropertySet {
        size: TSize {
            hint: 0,
            count: entries.len() as u32,
        },
        entries,
    };
    (s, instance, spec)
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let stress = b.byte() & 0x7 == 0x7;
    let (source, instance, spec) = world(&mut b, stress);
    let scripts: Vec<ScriptAppearance> = (0..b.small(4))
        .map(|_| ScriptAppearance {
            record: record_did(&mut b),
            key: if b.chance(1) { 0 } else { b.pick(&KEYS) },
            modifier: b.pick(&VARIANTS),
        })
        .collect();
    let options = AssemblyOptions {
        avatar_update_visible: b.chance(4),
        avatar_face_compositing: b.chance(4),
    };
    let stubs: HashMap<Did, bool> = source
        .meshes
        .iter()
        .filter(|(d, _)| !source.invalid.contains(d))
        .map(|(d, m)| (*d, m.is_stub()))
        .collect();
    let locations: HashSet<u32> = source.setups[&SETUP]
        .bone_frames
        .iter()
        .filter(|f| f.bone >= -1)
        .map(|f| f.key)
        .collect();
    let limits = AssemblyLimits {
        max_items: 8,
        max_list_elements: if stress { 160 } else { 10 },
        max_parts: 24,
        max_held: 3,
        max_layers: 6,
        max_total_layers: if stress { 2048 } else { 64 },
        max_diagnostics: 32,
        max_applications: if stress { 256 } else { 32 },
    };
    let mut asm = CharacterAssembler::new(source, vocab())
        .with_limits(limits)
        .with_options(options);
    let results = [
        asm.assemble_character(&spec),
        asm.assemble_character_with(&spec, &scripts),
        asm.assemble_object(AVATAR, Some(&instance)),
        asm.assemble_object_with(AVATAR, Some(&instance), &scripts),
        asm.assemble_object(AVATAR, None),
    ];
    for c in results.into_iter().flatten() {
        assert!(c.diagnostics.len() <= limits.max_diagnostics);
        assert!(c.held.len() <= limits.max_held);
        let mut offered_slots = HashSet::new();
        for p in &c.parts {
            assert!(p.materials.len() <= limits.max_layers);
            if let Some(&stub) = stubs.get(&p.mesh) {
                assert_eq!(p.hidden, stub);
            }
            if !p.is_default() {
                assert!(offered_slots.insert(p.slot), "one offered part per slot");
                assert!(!p.priority.is_nan());
                assert!(matches!(
                    p.source(),
                    Some(
                        SourceRef::TemplateSection(_)
                            | SourceRef::ClassList(_)
                            | SourceRef::InstanceList(_)
                            | SourceRef::Item(_)
                            | SourceRef::Script(_)
                    )
                ));
            }
        }
        assert!(c.parts.len() <= limits.max_parts);
        for h in &c.held {
            assert!(locations.contains(&h.location));
            assert_eq!(h.chain.last(), Some(&h.setup));
            let s = c.held_scale(h);
            let _ = h.placement.applied_to(&h.frame);
            assert!(s.iter().all(|x| !x.is_nan()));
        }
        assert!(c.groups.len() <= limits.max_total_layers);
        assert!(
            c.groups
                .windows(2)
                .all(|w| w[0].priority.total_cmp(&w[1].priority).is_le()),
            "the group list is sorted by priority"
        );
        let m = c.model_scale();
        assert!(m.iter().all(|x| x.is_finite() && *x >= 0.0002));
    }
});
