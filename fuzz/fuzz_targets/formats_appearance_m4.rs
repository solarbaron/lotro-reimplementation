// SPDX-License-Identifier: GPL-3.0-or-later
//! M4 assembly semantics on arbitrary bytes (`docs/specs/appearance.md`, `setup.md` part 2):
//!
//! - an appearance record (0x20): block choice for modifiers and dye codes, selection
//!   counts, group priorities and the slot table fed with every block;
//! - a setup (0x04): typed attach map, holding locations, viewport frames, bone checks;
//! - a property set (with a total synthetic schema) read as appearance lists, an item
//!   (worn/held look, holding rule), NPC props and bone-slider definitions;
//! - the race table and an avatar body through a fetch function serving the input as every
//!   record;
//! - the same helpers on a property set *built* from the input with PIDs whose `total` type
//!   is the one each helper reads (raw bytes almost never form typed lists of structs), and
//!   the race table and avatar bodies in a small world serving that set as every object's
//!   set and every entity's embedded set, with a visual template leading to a setup;
//! - the appearance-record and setup checks above also on an appearance record and a setup
//!   *built* from the input and encoded (raw bytes almost never decode to a record with
//!   entries or frames): entries with blocks of base, style and dye variants (also NaN),
//!   material groups and parts with NaN, infinite and fractional priorities, extras, a key
//!   table that may disagree with the selection counts; bone frames anchored at the origin,
//!   at bones or below −1, attachments, placements, named frames and collision volumes with
//!   hostile floats, repeated keys now and then. The encoded bytes must decode again and
//!   re-encode to the same bytes.
//!
//! Nothing may panic; derived values must stay consistent with their sources.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_appearance_m4 -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceExtra, AppearanceListPids, AppearancePart,
    EXTRA_KIND_DID, MaterialGroup, SLOT_ALL, SlotTable,
};
use tessera_formats::avatar::{
    AvatarPids, BoneSliderDefinition, BoneSliderPids, MAX_RACE_ENTRIES, RacePids, RaceTable,
    parse_slider_values, resolve_avatar_body, slider_positions, slider_value_pieces,
};
use tessera_formats::item::{
    HoldingRule, ItemLook, ItemPids, ItemVisual, WardrobeFallbacks, npc_cosmetic_inventory_visible,
    npc_held_items,
};
use tessera_formats::setup::{
    Aabb, BoneFrame, CollisionBox, CollisionCapsule, CollisionSphere, ENTITY_FLAGS_REQUIRED, Frame,
    NamedFrame, PhysicsBounds, Placement, Quaternion, SETUP_SECTION_DID_LIST, SetupSection,
    SetupSectionEntry, TAGFILE_MAGIC, TemplateAppearance,
};
use tessera_formats::{
    AppearanceRecord, Did, EntityDescriptor, EntityHeader, PropertySet, PropertyType, Record,
    Setup, TSize, VisualTemplate, is_property_set_did,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

// `total` type codes (`PropertyType::from_code`) the helpers read.
const STRING: u32 = 1;
const VECTOR3: u32 = 6;
const ENUM: u32 = 8;
const FLOAT: u32 = 9;
const PROPERTY_ID: u32 = 10;
const STRUCT: u32 = 11;
const ARRAY: u32 = 12;
const BITFIELD64: u32 = 14;
const INT: u32 = 15;
const DID_REF: u32 = 20;
const BOOL: u32 = 21;

/// Every name the M4 helpers look up, with the type code its helper reads (`as_did` needs
/// a DID reference, `children` an array or struct, `read_all` a vector, ...). Entry `i`
/// gets PID 24 × (i + 1) + code, so under `total` each PID has exactly that type and no two
/// names share a PID. The two `Element_*` names are not looked up: they give array
/// elements a struct or DID type.
const TABLE: &[(&str, u32)] = &[
    ("Appearance_ClassList", ARRAY),
    ("Appearance_InstanceList", ARRAY),
    ("Appearance_Key", ENUM),
    ("Appearance_AprFile", DID_REF),
    ("Appearance_Modifier", FLOAT),
    ("Appearance_AprToggleFile", DID_REF),
    ("Appearance_ToggleProperty", PROPERTY_ID),
    ("Item_WornAppearanceMapList", ARRAY),
    ("Item_WornAppearance", DID_REF),
    ("Item_AppearanceKey", ENUM),
    ("Item_SpeciesOfWearer", ENUM),
    ("Item_SexOfWearer", ENUM),
    ("PhysObj", DID_REF),
    ("Inventory_DefaultSlot", BITFIELD64),
    ("Inventory_ParentingLocation", ENUM),
    ("Inventory_PaperdollParentingLocation", ENUM),
    ("Item_EquipmentCategory", BITFIELD64),
    ("Item_ClothingColor", FLOAT),
    ("Item_ClothingColorSecondary", FLOAT),
    ("NPC_Cosmetic_Inventory_Item1", DID_REF),
    ("NPC_Cosmetic_Inventory_Item2", DID_REF),
    ("NPC_Cosmetic_Inventory_Item3", DID_REF),
    ("NPC_Cosmetic_Inventory_Item4", DID_REF),
    ("NPC_Cosmetic_Inventory_HoldingLocation1", ENUM),
    ("NPC_Cosmetic_Inventory_HoldingLocation2", ENUM),
    ("NPC_Cosmetic_Inventory_HoldingLocation3", ENUM),
    ("NPC_Cosmetic_Inventory_HoldingLocation4", ENUM),
    ("NPC_Cosmetic_Inventory_Visible", BOOL),
    ("BoneSliderDeformer_SliderDefinitionArray", ARRAY),
    ("BoneSlider_Name", ENUM),
    ("BoneSlider_SliderGroup", ENUM),
    ("BoneSlider_BoneTranformMaskArray", ARRAY),
    ("BoneSlider_LimitMin", VECTOR3),
    ("BoneSlider_LimitMax", VECTOR3),
    ("BoneSlider_BoneIndexArray", ARRAY),
    ("BoneSlider_BoneIndex", INT),
    ("BoneSlider_BoneReverse", BOOL),
    ("BoneSlider_UI_Visible", BOOL),
    ("RaceTable_RaceTableList", ARRAY),
    ("RaceTable_Race", ENUM),
    ("RaceTable_GenderList", ARRAY),
    ("RaceTable_GenderType", ENUM),
    ("RaceTable_Gender_PlayerAvatar", DID_REF),
    ("RaceTable_GenderNullAppearance", DID_REF),
    ("RaceTable_RaceSelect_Entity", DID_REF),
    ("RaceTable_IsTallRace", BOOL),
    ("Render_AdjustableScale", VECTOR3),
    ("AppearanceUI_BoneSliderDefinitionsFile", DID_REF),
    ("AC_BoneSlider_Value_Array", STRING),
    ("Element_Struct", STRUCT),
    ("Element_Did", DID_REF),
];

#[allow(clippy::cast_possible_truncation)] // the table is far shorter than u32::MAX / 24
fn table_pid(i: usize) -> Option<u32> {
    TABLE.get(i).map(|&(_, code)| 24 * (i as u32 + 1) + code)
}

/// Table names map to their typed PIDs. Other names (the species labels of the wardrobe
/// fallbacks) get a hash-derived PID far above the table's.
fn names(n: &str) -> Option<u32> {
    if let Some(i) = TABLE.iter().position(|&(name, _)| name == n) {
        return table_pid(i);
    }
    let h = n
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    Some(0x1000_0000 | (h & 0xFFFF))
}

/// The directory object of the synthetic race table, a species object, a player-avatar
/// object, an avatar entity, a visual template, a setup, an appearance record and a
/// slider-definition set: the DIDs the synthetic world serves.
const DIRECTORY: u32 = 0x7000_0001;
const SPECIES: u32 = 0x7000_0002;
const AVATAR_OBJECT: u32 = 0x7000_0010;
const ENTITY: u32 = 0x4700_0010;
const TEMPLATE: u32 = 0x1F00_0001;
const SETUP: u32 = 0x0400_0001;
const APPEARANCE: u32 = 0x2000_0001;
const SLIDER_SET: u32 = 0x7800_0001;

/// Reads the input front to back; zeros once it is used up.
struct Src<'a> {
    data: &'a [u8],
    at: usize,
}

impl Src<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.at).copied().unwrap_or(0);
        self.at = self.at.saturating_add(1);
        b
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn below(&mut self, n: u8) -> u8 {
        self.u8() % n
    }

    fn table_pid(&mut self) -> u32 {
        let i = usize::from(self.u8()) % TABLE.len();
        table_pid(i).unwrap_or(STRUCT)
    }

    fn did(&mut self) -> u32 {
        match self.below(10) {
            0 => 0,
            1 => DIRECTORY,
            2 => SPECIES,
            3 => AVATAR_OBJECT,
            4 => ENTITY,
            5 => APPEARANCE,
            6 => SLIDER_SET,
            7 => TEMPLATE,
            _ => self.u32(),
        }
    }

    fn key(&mut self) -> u32 {
        match self.below(6) {
            0 => u32::from(self.below(4)),
            1 => 0x1000,
            2 => 0x2000,
            3 => 0x17,
            4 => self.table_pid(),
            _ => self.u32(),
        }
    }

    fn float(&mut self) -> f32 {
        match self.below(8) {
            0 => 0.0,
            1 => 1.0,
            2 => -1.0,
            3 => 0.5,
            4 => 2.0,
            5 => f32::NAN,
            _ => f32::from_bits(self.u32()),
        }
    }
}

/// Appends a value of type `code` drawn from `src`. Containers stop growing at depth 3.
fn put_value(out: &mut Vec<u8>, src: &mut Src<'_>, code: u32, depth: u8) {
    match code {
        STRING => {
            // Slider byte strings are hexadecimal digit pairs (`parse_slider_values`).
            let n = src.below(9);
            out.push(n);
            for _ in 0..n {
                out.push(b"0123456789ABCDEFxz"[usize::from(src.below(18))]);
            }
        }
        VECTOR3 => {
            for _ in 0..3 {
                out.extend_from_slice(&src.float().to_le_bytes());
            }
        }
        FLOAT => out.extend_from_slice(&src.float().to_le_bytes()),
        ENUM | PROPERTY_ID => out.extend_from_slice(&src.key().to_le_bytes()),
        STRUCT => {
            let n = if depth >= 3 { 0 } else { src.below(6) };
            // TSize: hint byte, then the count as a one-byte VLE (n < 128).
            out.push(0);
            let at = out.len();
            out.push(0);
            let mut seen = Vec::new();
            for _ in 0..n {
                let pid = src.table_pid();
                // Struct field PIDs must be unique.
                if seen.contains(&pid) {
                    continue;
                }
                seen.push(pid);
                out.extend_from_slice(&pid.to_le_bytes());
                out.extend_from_slice(&pid.to_le_bytes());
                put_value(out, src, pid % 24, depth + 1);
            }
            #[allow(clippy::cast_possible_truncation)] // at most 5 fields
            if let Some(c) = out.get_mut(at) {
                *c = seen.len() as u8;
            }
        }
        ARRAY => {
            let n = if depth >= 3 { 0 } else { src.below(5) };
            out.extend_from_slice(&u32::from(n).to_le_bytes());
            for _ in 0..n {
                // Lists hold structs or DIDs; other element types are rarer.
                let pid = match src.below(4) {
                    0 | 1 => names("Element_Struct").unwrap_or(STRUCT),
                    2 => names("Element_Did").unwrap_or(DID_REF),
                    _ => src.table_pid(),
                };
                out.extend_from_slice(&pid.to_le_bytes());
                put_value(out, src, pid % 24, depth + 1);
            }
        }
        BITFIELD64 => {
            let v = match src.below(3) {
                0 => 0x1_0000,
                1 => u64::MAX,
                _ => u64::from(src.u32()) | (u64::from(src.u32()) << 32),
            };
            out.extend_from_slice(&v.to_le_bytes());
        }
        INT => out.extend_from_slice(&(i32::from(src.u8() as i8)).to_le_bytes()),
        DID_REF => out.extend_from_slice(&src.did().to_le_bytes()),
        BOOL => out.push(src.u8() & 1),
        // Not produced: every table code is handled above.
        _ => out.extend_from_slice(&src.u32().to_le_bytes()),
    }
}

/// An embedded property set (TSize, then PID, PID echo, value) built from `src`, using
/// the table's PIDs so that the M4 helpers find the types they read.
fn synth_set(src: &mut Src<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    put_value(&mut out, src, STRUCT, 0);
    out
}

/// The property-set helpers: appearance lists, item look, NPC props, bone sliders.
fn check_set(set: &PropertySet) {
    if let Ok(p) = AppearanceListPids::from_names(&names) {
        for e in p.read_lists(set) {
            let f = e.file_in(set);
            assert!(f == e.file || Some(f) == e.toggle_file);
        }
    }
    if let Ok(p) = ItemPids::from_names(&names) {
        let item = ItemVisual::from_property_set(set, &p);
        let fb = WardrobeFallbacks::from_species_labels(|l| names(l).map(|k| k & 0xFF));
        match item.look(0x17, 0x1000, &fb) {
            ItemLook::Worn { reference, .. } => {
                assert!(item.worn.contains(&reference));
                assert_eq!(reference.sex, 0x1000);
            }
            ItemLook::Held { phys_obj } => {
                assert_eq!(Some(phys_obj), item.phys_obj);
                assert!(item.worn.is_empty() && !phys_obj.is_null());
            }
            ItemLook::Invisible => {}
        }
        let rule = HoldingRule {
            slot_locations: vec![(0x1_0000, 1), (u64::MAX, 2)],
        };
        let _ = rule.location(&item, true);
        let _ = rule.location(&item, false);
    }
    let held = npc_held_items(set, &names);
    assert!(held.len() <= 4);
    for h in &held {
        assert!((1..=4).contains(&h.index) && !h.entity.is_null());
    }
    let _ = npc_cosmetic_inventory_visible(set, &names);
    if let Ok(p) = BoneSliderPids::from_names(&names) {
        let defs = BoneSliderDefinition::read_all(set, &p);
        for d in &defs {
            let _ = d.value_for(0x80);
            let _ = d.channel_bits();
        }
        assert_eq!(
            slider_positions(defs.len(), &[0x10, 0x20]).len(),
            defs.len()
        );
    }
}

/// The race table and the avatar bodies through `fetch`. The walks must stop on cycles,
/// limits and bad records.
fn check_world<B: AsRef<[u8]>>(fetch: impl Fn(Did) -> Result<Option<B>, Infallible> + Copy) {
    if let Ok(p) = RacePids::from_names(&names)
        && let Ok(t) = RaceTable::read(Did(DIRECTORY), &total, &p, fetch)
    {
        assert!(t.entries.len() <= MAX_RACE_ENTRIES);
        assert!(t.entries.iter().all(|e| !e.player_avatar.is_null()));
        assert!(t.avatars().len() <= t.entries.len());
    }
    if let Ok(p) = AvatarPids::from_names(&names) {
        let _ = resolve_avatar_body(Did(AVATAR_OBJECT), &total, &p, fetch);
        if let Ok(body) = resolve_avatar_body(Did(ENTITY), &total, &p, fetch) {
            let all = body.appearance_files();
            assert_eq!(
                all.len(),
                body.template_appearance.len() + body.class_list.len() + body.instance_list.len()
            );
            // An instance list of the placed figure replaces the entity's own.
            let own = body.class_list.clone();
            let placed = body.appearance_files_with_instance(Some(&own));
            assert_eq!(
                placed.len(),
                body.template_appearance.len() + 2 * body.class_list.len()
            );
        }
    }
}

/// A world served from one synthetic property set: every property-set DID gets a record
/// holding it, every entity an entity descriptor embedding it (linked to the template), and
/// the template a record naming the setup and an appearance section.
struct World {
    set: Vec<u8>,
    template: Vec<u8>,
}

impl World {
    fn new(set: Vec<u8>, src: &mut Src<'_>) -> Self {
        let modifiers = (0..src.below(3))
            .map(|_| (src.key(), src.float()))
            .collect::<Vec<_>>();
        let scale = src.float();
        let template = VisualTemplate {
            // Bit 0x001 is set in every template (`setup.md` §5.2); the field bits are
            // derived by `to_bytes`.
            flags: 1,
            did: Did(TEMPLATE),
            parent: None,
            vec3: Some([scale; 3]),
            value_a: None,
            value_b: None,
            appearance: Some(TemplateAppearance {
                appearance: Did(APPEARANCE),
                modifiers_size: TSize::default(),
                modifiers,
            }),
            setup_section: Some(SetupSection {
                size: TSize::default(),
                entries: vec![SetupSectionEntry {
                    key: 1,
                    kind: SETUP_SECTION_DID_LIST,
                    dids: vec![Did(SETUP)],
                }],
            }),
        }
        .to_bytes();
        Self { set, template }
    }

    fn fetch(&self, did: Did) -> Result<Option<Vec<u8>>, Infallible> {
        Ok(if is_property_set_did(did) {
            let mut out = did.0.to_le_bytes().to_vec();
            out.extend_from_slice(&self.set);
            Some(out)
        } else if did.prefix() == 0x47 {
            EntityDescriptor {
                header: EntityHeader {
                    flags: ENTITY_FLAGS_REQUIRED,
                    did,
                    kind: 7,
                    template: Did(TEMPLATE),
                    block: None,
                },
                properties: PropertySet::default(),
                extra_sets: Vec::new(),
            }
            .to_bytes_with(&self.set)
            .ok()
        } else if did == Did(TEMPLATE) {
            Some(self.template.clone())
        } else {
            None
        })
    }
}

/// Block choice, dye blocks, selection counts, group priorities and the slot table.
fn check_appearance(a: &AppearanceRecord) {
    let _ = a.first_unusual_field();
    let _ = a.key_table_matches_selection_counts();
    let mut slots = SlotTable::new();
    for (i, e) in a.entries.iter().enumerate() {
        let n = e.selection_count();
        assert!(n as usize <= e.blocks.len());
        for m in [0.0f32, 0.01, 0.5, 1.0, -1.0, f32::NAN] {
            if let Some(b) = e.block_for_modifier(m) {
                assert!(e.blocks.iter().any(|x| std::ptr::eq(x, b)));
            } else {
                assert!(e.blocks.is_empty());
            }
            match e.dye_block(m) {
                Some(b) => assert!(e.blocks.iter().any(|x| std::ptr::eq(x, b))),
                None => assert!(e.blocks.is_empty() || m.is_nan()),
            }
        }
        if let Some(b) = e.base_block() {
            assert!(e.blocks.iter().any(|x| std::ptr::eq(x, b)));
        }
        for b in &e.blocks {
            let n = b.groups_with_priority().count();
            assert_eq!(n, b.groups.len());
            for g in 0..=b.groups.len() {
                assert_eq!(b.group_priority(g).is_some(), g < b.groups.len());
            }
            let _ = b.groups_end();
            #[allow(clippy::cast_possible_truncation)] // a handful of entries
            slots.offer_block(b, i as u32);
        }
    }
    let parts: Vec<&AppearancePart> = a
        .entries
        .iter()
        .flat_map(|e| &e.blocks)
        .flat_map(|b| &b.parts)
        .collect();
    assert!(slots.len() <= parts.len());
    // Every slot holds a part offered for it.
    for sp in slots.iter() {
        assert!(
            parts.iter().any(|p| p.slot == sp.slot && p.mesh == sp.mesh),
            "slot {:#x} holds a part never offered",
            sp.slot
        );
    }
}

/// Attachments, holding locations, viewport frames and bone checks.
fn check_setup(s: &Setup) {
    let _ = s.first_unusual_field();
    for n in [0usize, 1, 62, 89, 256] {
        let _ = s.bone_issue(n);
    }
    assert!(s.holding_locations().count() <= s.bone_frames.len());
    for f in &s.bone_frames {
        if let Some(h) = s.holding_location(f.key) {
            assert_eq!(h.key, f.key);
        }
    }
    for a in s.attachments() {
        assert!(s.bone_for(a.bone_type).is_some());
    }
    for f in &s.named_frames {
        assert!(s.viewport_frame(f.key).is_some());
    }
    let _ = s.volume_bounds();
    let _ = s.has_collision();
}

/// A frame with hostile values now and then.
fn synth_frame(src: &mut Src<'_>) -> Frame {
    if src.below(3) == 0 {
        return Frame::IDENTITY;
    }
    Frame {
        position: [src.float(), src.float(), src.float()],
        scale: [src.float(), src.float(), src.float()],
        rotation: Quaternion {
            w: src.float(),
            x: src.float(),
            y: src.float(),
            z: src.float(),
        },
    }
}

/// Priorities: whole numbers 0–100 as observed, and fractions, negatives, NaN, infinity.
fn priority(src: &mut Src<'_>) -> f32 {
    match src.below(10) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => -1.0,
        3 => 0.5,
        _ => f32::from(src.below(101)),
    }
}

/// An appearance record (`setup.md` §8) built from `src`.
fn synth_appearance(src: &mut Src<'_>) -> AppearanceRecord {
    let mut entries: Vec<AppearanceEntry> = Vec::new();
    for _ in 0..src.below(5) {
        let key = 0x1000_0000 | u32::from(src.below(8));
        if entries.iter().any(|e| e.key == key) {
            continue; // keys are unique
        }
        let blocks = (0..src.below(5))
            .map(|_| {
                let groups: Vec<MaterialGroup> = (0..src.below(4))
                    .map(|_| MaterialGroup {
                        shader: Did(if src.below(2) == 0 { 0 } else { 0x2B00_0001 }),
                        material: Did(0x3000_0000 | u32::from(src.u8())),
                        a: match src.below(3) {
                            0 => SLOT_ALL,
                            _ => 0x1000_0010 + u32::from(src.below(4)),
                        },
                        binding_key: u32::from(src.below(4)),
                        g: priority(src),
                    })
                    .collect();
                AppearanceBlock {
                    variant: match src.below(8) {
                        0..=2 => 1.0,
                        3 => 0.0,
                        4 => 0.01,
                        5 => 0.02,
                        6 => f32::NAN,
                        _ => src.float(),
                    },
                    group_param: if groups.is_empty() {
                        0.0
                    } else {
                        priority(src)
                    },
                    groups,
                    parts: (0..src.below(5))
                        .map(|_| AppearancePart {
                            part_param: priority(src),
                            slot: 0x1000_0010 + u32::from(src.below(4)),
                            mesh: Did(if src.below(8) == 0 {
                                0x0400_0001
                            } else {
                                0x0600_0000 | u32::from(src.u8())
                            }),
                        })
                        .collect(),
                    extras: (0..src.below(2))
                        .map(|_| AppearanceExtra {
                            kind: EXTRA_KIND_DID,
                            slot: 0x1000_0010,
                            did: Did(SETUP),
                        })
                        .collect(),
                }
            })
            .collect();
        entries.push(AppearanceEntry { key, blocks });
    }
    // One key-table pair per entry: mostly the entry's key and selection count.
    let key_table = entries
        .iter()
        .map(|e| match src.below(8) {
            0 => (e.key, e.selection_count() + 1),
            1 => (0x1000_00FF, 0),
            _ => (e.key, e.selection_count()),
        })
        .collect();
    AppearanceRecord {
        did: Did(APPEARANCE),
        section: src.below(7),
        entries,
        key_table,
    }
}

/// A setup (`setup.md` §6) built from `src`.
fn synth_setup(src: &mut Src<'_>) -> Setup {
    let key = |src: &mut Src<'_>| 0x1000_0000 | u32::from(src.below(6));
    let mut tagfile = Vec::new();
    if src.below(4) != 0 {
        tagfile.extend_from_slice(&TAGFILE_MAGIC);
    }
    for _ in 0..src.below(8) {
        tagfile.push(src.u8());
    }
    let has_volumes = src.below(2) == 0;
    Setup {
        did: Did(SETUP),
        tagfile,
        attach_map_size: TSize { hint: 1, count: 0 },
        attach_map: (0..src.below(6))
            .map(|_| (u32::from(src.below(8)), u32::from(src.below(100))))
            .collect(),
        bone_frames_hint: 1,
        bone_frames: (0..src.below(6))
            .map(|_| {
                let k = key(src);
                BoneFrame {
                    key: k,
                    key2: if src.below(8) == 0 { k + 1 } else { k },
                    bone: match src.below(6) {
                        0 => -1,
                        1 => -2,
                        2 => i32::MAX,
                        _ => i32::from(src.below(90)),
                    },
                    frame: synth_frame(src),
                }
            })
            .collect(),
        placements: (0..src.below(3))
            .map(|i| Placement {
                id: u32::from(i),
                frame: synth_frame(src),
            })
            .collect(),
        named_frames_hint: 1,
        named_frames: (0..src.below(4))
            .map(|_| {
                let k = u32::from(src.below(5));
                NamedFrame {
                    key: k,
                    key2: k,
                    frame: synth_frame(src),
                }
            })
            .collect(),
        boxes: if has_volumes {
            (0..src.below(3))
                .map(|_| CollisionBox {
                    min: [src.float(), src.float(), src.float()],
                    max: [src.float(), src.float(), src.float()],
                    frame: synth_frame(src),
                })
                .collect()
        } else {
            Vec::new()
        },
        spheres: (0..src.below(3))
            .map(|_| CollisionSphere {
                centre: [src.float(), src.float(), src.float()],
                radius: src.float(),
            })
            .collect(),
        capsules: (0..src.below(3))
            .map(|_| CollisionCapsule {
                base: [src.float(), src.float(), src.float()],
                length: src.float(),
                radius: src.float(),
            })
            .collect(),
        collision_mesh: Did(if src.below(2) == 0 { 0 } else { 0x6600_0001 }),
        physics_bounds: PhysicsBounds {
            aabb: Aabb {
                min: [src.float(), src.float(), src.float()],
                max: [src.float(), src.float(), src.float()],
            },
            height: src.float(),
            radius: src.float(),
        },
        parts: (0..src.below(5))
            .map(|_| Did(0x0600_0000 | u32::from(src.u8())))
            .collect(),
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(a) = AppearanceRecord::parse(data) {
        check_appearance(&a);
    }
    if let Ok(s) = Setup::parse(data) {
        check_setup(&s);
    }
    if let Ok(set) = PropertySet::parse(data, &total) {
        check_set(&set);
    }
    if let Ok(s) = std::str::from_utf8(data) {
        if let Some(b) = parse_slider_values(s) {
            assert_eq!(b.len() * 2, s.len());
        }
        // Pieces of two characters, the last one odd or the empty piece (§6.3, A29).
        let pieces = slider_value_pieces(s);
        assert!(!pieces.is_empty() && pieces.len() <= s.len() / 2 + 1);
        assert!(
            slider_positions(pieces.len(), &pieces)
                .iter()
                .all(|t| t.is_finite())
        );
    }
    // Every DID serves the input.
    check_world(|_: Did| Ok::<_, Infallible>(Some(data)));
    // A structured property set, so that lists, structs and DIDs reach the helpers with
    // the types they read (raw bytes almost never do).
    let mut src = Src { data, at: 0 };
    let set = synth_set(&mut src);
    match PropertySet::parse(&set, &total) {
        Ok(s) => check_set(&s),
        Err(e) => panic!("synthetic property set rejected: {e}"),
    }
    let world = World::new(set, &mut src);
    check_world(|d| world.fetch(d));
    // A structured appearance record and setup.
    let bytes = synth_appearance(&mut src).to_bytes();
    match AppearanceRecord::parse(&bytes) {
        Ok(a) => {
            assert_eq!(a.to_bytes(), bytes, "appearance re-encoding");
            check_appearance(&a);
        }
        Err(e) => panic!("synthetic appearance record rejected: {e}"),
    }
    let bytes = synth_setup(&mut src)
        .to_bytes()
        .expect("synthetic setup encodes");
    match Setup::parse(&bytes) {
        Ok(s) => {
            assert_eq!(s.to_bytes().ok(), Some(bytes), "setup re-encoding");
            check_setup(&s);
        }
        Err(e) => panic!("synthetic setup rejected: {e}"),
    }
});
