// SPDX-License-Identifier: GPL-3.0-or-later
//! A synthetic install for the selector tests: our own encodings of the layouts in
//! `property-system.md`, `records.md`, `wlib.md` and `map-data.md`, with invented labels,
//! keys and DIDs. No game data.
#![allow(dead_code, clippy::type_complexity)]

use tessera_formats::Did;
use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_gamedata::GameDataStore;
use tessera_props::MemorySource;
use tessera_wstate::ObjectStore;
use tessera_wstate::testkit::{
    LibraryBuilder, did_map, embedded_property_set, put_position, put_string_info_literal,
    put_tsize, put_utf16,
};

// Enum mappers (prefix 0x23).
pub const M_WEENIE: u32 = 0x2300_0001;
pub const M_SLOT: u32 = 0x2300_0002;
pub const M_ITEM_CLASS: u32 = 0x2300_0003;
pub const M_QUALITY: u32 = 0x2300_0004;
pub const M_CLASS: u32 = 0x2300_0005;
pub const M_SKILL_TYPE: u32 = 0x2300_0006;
pub const M_NATURE: u32 = 0x2300_0007;
pub const M_SPECIES: u32 = 0x2300_0008;
pub const M_GENUS: u32 = 0x2300_0009;
pub const M_ALIGN: u32 = 0x2300_000A;
pub const M_AGENT_CLASS: u32 = 0x2300_000B;
pub const M_QUEST_CAT: u32 = 0x2300_000C;
pub const M_TIER: u32 = 0x2300_000D;
pub const M_RACE: u32 = 0x2300_000E;
pub const M_DAMAGE: u32 = 0x2300_000F;
pub const M_NOTE_TYPE: u32 = 0x2300_0010;
pub const M_GENDER: u32 = 0x2300_0011;
/// Base of the slot mapper (tests the base chain).
pub const M_SLOT_BASE: u32 = 0x2300_0012;
/// P6 progression: `Trait_TraitTree_TreeType`.
pub const M_TREE_TYPE: u32 = 0x2300_0013;
/// P6 progression: `Trait_TraitTree_Branch`.
pub const M_BRANCH: u32 = 0x2300_0014;
/// P6 progression: `CraftRecipe_UICategory`.
pub const M_RECIPE_CAT: u32 = 0x2300_0015;

/// (label, type code, enum mapper) of every property the synthetic install uses. PIDs are
/// 0x1000_0001 + index.
pub const PROPS: &[(&str, u32, u32)] = &[
    ("WeenieType", 8, M_WEENIE),
    ("Inventory_DefaultSlot", 18, M_SLOT),
    ("Inventory_CompatibleSlot", 18, M_SLOT),
    ("Item_Class", 8, M_ITEM_CLASS),
    ("Item_Quality", 8, M_QUALITY),
    ("Item_Level", 15, 0),
    ("Usage_MinLevel", 15, 0),
    ("Usage_RequiredClassList", 12, 0),
    ("Usage_ClassType", 8, M_CLASS),
    ("Inventory_MaxQuantity", 15, 0),
    ("Icon_Layer_ImageDID", 20, 0),
    ("Name", 13, 0),
    ("Vendor_Item", 20, 0),
    ("VendorInventory_Items", 12, 0),
    ("Skill_Name", 13, 0),
    ("Skill_SkillType", 18, M_SKILL_TYPE),
    ("Skill_Harmful", 21, 0),
    ("Skill_IsMonsterSkill", 21, 0),
    ("Skill_InductionAction", 20, 0),
    ("Skill_Toggle_Hook_Number", 15, 0),
    ("Trait_Nature", 8, M_NATURE),
    ("Trait_Minimum_Level", 15, 0),
    ("AdvTable_Class", 8, M_CLASS),
    ("AdvTable_AvailableSkillEntryList", 12, 0),
    ("AdvTable_AvailableSkillEntry", 11, 0),
    ("AdvTable_Skill", 20, 0),
    ("AdvTable_AvailableMountSkillEntryList", 12, 0),
    ("AdvTable_ClassCharacteristic_List", 12, 0),
    ("AdvTable_ClassCharacteristic", 11, 0),
    ("AdvTable_Trait_WC", 20, 0),
    ("Agent_Species", 8, M_SPECIES),
    ("Agent_Genus", 18, M_GENUS),
    ("Agent_Alignment", 8, M_ALIGN),
    ("Agent_Class", 8, M_AGENT_CLASS),
    ("Appearance_ClassList", 12, 0),
    ("Appearance_Entry", 11, 0),
    ("Vendor_InventoryList", 12, 0),
    ("Vendor_Inventory", 20, 0),
    ("SkillTrainer_Class", 8, M_CLASS),
    ("QuestDispenser_ActiveQuests", 12, 0),
    ("QuestDispenser_Quest", 20, 0),
    ("Quest_Name", 13, 0),
    ("Quest_ObjectiveArray", 12, 0),
    ("Quest_CompletionObjective", 11, 0),
    ("Quest_RoleArray", 12, 0),
    ("Quest_Category", 8, M_QUEST_CAT),
    ("Quest_ChallengeLevel", 15, 0),
    ("Quest_IsAccomplishment", 21, 0),
    ("NPCComment_Title", 13, 0),
    ("NPCComment_Content", 13, 0),
    ("Title_String", 13, 0),
    ("Title_Category", 8, 0),
    ("Emote_CommandString", 13, 0),
    ("Emote_ScriptID", 20, 0),
    ("Reputation_Faction_Name", 13, 0),
    ("Reputation_Faction_AdvancementTable", 20, 0),
    ("LootGen_TrophyList", 12, 0),
    ("CraftRecipe_Profession", 20, 0),
    ("CraftRecipe_Tier", 8, M_TIER),
    ("CraftRecipe_ResultItem", 20, 0),
    ("CraftProfession_TierArray", 12, 0),
    ("TravelDisplayName", 13, 0),
    ("TravelDestinationIsSwiftTravel", 21, 0),
    ("Area_Territory", 20, 0),
    ("Area_Region", 20, 0),
    ("Area_RegionID", 15, 0),
    ("Dungeon_Name", 13, 0),
    ("PrivateEncounterTemplate_ContentLayer", 8, 0),
    ("RaceTable_RaceTableList", 12, 0),
    ("RaceTable_Race", 8, M_RACE),
    ("RaceTable_GenderType", 8, M_GENDER),
    ("Effect_DamageType", 8, M_DAMAGE),
    ("MapNote_Type", 14, M_NOTE_TYPE),
    ("RaceTable_GenderList", 12, 0),
    ("RaceTable_GenderStruct", 11, 0),
    ("AdvTable_PrerequisiteSkill", 20, 0),
    ("CraftRecipe_TierArray", 12, 0),
    ("CraftRecipe_Name", 13, 0),
    ("CraftProfession_Name", 13, 0),
    ("PrivateEncounterTemplate_Name", 13, 0),
    ("Area_Name", 13, 0),
    ("Trait_Name", 13, 0),
    // P6 progression attributes (appended: earlier PIDs stay as they were).
    ("Trait_Tier", 15, 0),
    ("Trait_PointBasedTrait_PointCostProgression", 20, 0),
    ("Trait_TraitTree_TreeType", 8, M_TREE_TYPE),
    ("Trait_TraitTree_TraitArray", 12, 0),
    ("Trait_TraitTree_TraitStruct", 11, 0),
    ("Trait_TraitTree_Trait", 20, 0),
    ("Trait_TraitTree_Branch", 8, M_BRANCH),
    ("CraftRecipe_UICategory", 8, M_RECIPE_CAT),
    ("CraftRecipe_OneTimeRecipe", 21, 0),
    ("CraftRecipe_CooldownDuration", 8, 0),
    ("CraftRecipe_CriticalResultItem", 20, 0),
    ("CraftRecipe_RequiredFacilityEffect", 20, 0),
    ("CraftRecipe_InductionAction", 20, 0),
    ("CraftVocation_ProfessionArray", 12, 0),
    ("CraftVocation_Profession", 20, 0),
    ("CraftVocation_Name", 13, 0),
    ("CraftGuild_Profession", 20, 0),
    ("Usage_RequiredCraftProfession", 20, 0),
    ("Induction_ActionDID", 20, 0),
];

/// The PID of `label`.
pub fn pid(label: &str) -> u32 {
    let i = PROPS
        .iter()
        .position(|(l, _, _)| *l == label)
        .unwrap_or_else(|| panic!("label {label} not in the synthetic dictionary"));
    0x1000_0001 + u32::try_from(i).unwrap()
}

// Value encodings (`property-system.md` §4).
pub fn v_u32(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}
pub fn v_i32(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}
pub fn v_u64(v: u64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}
pub fn v_bool(v: bool) -> Vec<u8> {
    vec![u8::from(v)]
}
pub fn v_text(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    put_string_info_literal(&mut out, s);
    out
}
/// An array of (element PID, value bytes).
pub fn v_array(elements: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = u32::try_from(elements.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    for (p, v) in elements {
        out.extend_from_slice(&p.to_le_bytes());
        out.extend_from_slice(v);
    }
    out
}
/// An array of DIDs under element PID `element`.
pub fn v_dids(element: &str, dids: &[u32]) -> Vec<u8> {
    let e = pid(element);
    v_array(&dids.iter().map(|d| (e, v_u32(*d))).collect::<Vec<_>>())
}
/// A struct (embedded property set).
pub fn v_struct(fields: &[(u32, Vec<u8>)]) -> Vec<u8> {
    embedded_property_set(fields)
}

fn tsize_for(n: usize) -> (u8, u32) {
    for (h, &b) in TSIZE_BUCKET_COUNTS.iter().enumerate() {
        if (b as usize) * 2 >= n {
            return (u8::try_from(h).unwrap(), b);
        }
    }
    (12, TSIZE_BUCKET_COUNTS[12])
}

fn pascal(out: &mut Vec<u8>, s: &str) {
    write_vle(out, u32::try_from(s.len()).unwrap());
    out.extend_from_slice(s.as_bytes());
}

/// The property dictionary (DID 0x34000000) with enum mapper links in the `data` field.
pub fn dictionary() -> Vec<u8> {
    let defs: Vec<(u32, &str, u32, u32)> =
        PROPS.iter().map(|&(l, t, m)| (pid(l), l, t, m)).collect();
    let mut out = 0x3400_0000u32.to_le_bytes().to_vec();
    out.extend_from_slice(&[0; 8]);
    let (hint, b) = tsize_for(defs.len());
    let mut names: Vec<(u32, &str)> = defs.iter().map(|d| (d.0, d.1)).collect();
    names.sort_by_key(|n| n.0 % b);
    out.push(hint);
    write_vle(&mut out, u32::try_from(names.len()).unwrap());
    for (p, n) in &names {
        out.extend_from_slice(&p.to_le_bytes());
        pascal(&mut out, n);
    }
    out.extend_from_slice(&[0, 0]);
    let mut sorted = defs.clone();
    sorted.sort_by_key(|d| d.0 % b);
    out.push(hint);
    write_vle(&mut out, u32::try_from(sorted.len()).unwrap());
    for (p, _, t, m) in sorted {
        out.extend_from_slice(&p.to_le_bytes());
        out.extend_from_slice(&p.to_le_bytes());
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // field_a
        out.extend_from_slice(&4u32.to_le_bytes()); // field_b
        out.extend_from_slice(&m.to_le_bytes()); // data: the enum mapper
        out.extend_from_slice(&0u32.to_le_bytes()); // field_c
        out.extend_from_slice(&0u32.to_le_bytes()); // flags
        out.extend_from_slice(&0x3FC0_0000u32.to_le_bytes());
        out.extend_from_slice(&[2, 2, 4, 0, 1]);
        write_vle(&mut out, 0); // children
        out.extend_from_slice(&0u32.to_le_bytes()); // subsets
        out.extend_from_slice(&0u32.to_le_bytes()); // terminator
    }
    out
}

/// An enum mapper record (`records.md` §7.1) with literal display texts.
pub fn enum_mapper(did: u32, base: u32, names: &[(u32, &str)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&base.to_le_bytes());
    let (hint, b) = tsize_for(names.len());
    let mut sorted = names.to_vec();
    sorted.sort_by_key(|n| n.0 % b);
    out.push(hint);
    write_vle(&mut out, u32::try_from(sorted.len()).unwrap());
    for (k, n) in &sorted {
        out.extend_from_slice(&k.to_le_bytes());
        pascal(&mut out, n);
    }
    out.push(hint);
    write_vle(&mut out, u32::try_from(sorted.len()).unwrap());
    for (k, n) in &sorted {
        out.extend_from_slice(&k.to_le_bytes());
        put_string_info_literal(&mut out, &format!("display {n}"));
    }
    out
}

// Class indices of the synthetic library. Combat classes use the indices of
// `CombatClasses::observed()` (the decoder's table, `combat-data.md` §2.1).
pub const C_ITEM_PARENT: u32 = 100;
pub const C_ITEM_BASE: u32 = 101;
pub const C_ITEM_SUB: u32 = 102;
pub const C_ITEM_LIKE: u32 = 103;
pub const C_CREATURE_ROOT: u32 = 200;
pub const C_MOB: u32 = 201;
pub const C_NPC: u32 = 202;
pub const C_AVATAR: u32 = 203;
pub const C_OTHER: u32 = 300;
pub const C_EFFECT_BASE: u32 = 698;
pub const C_EFFECT_INSTANT_VITAL: u32 = 725;
pub const C_EFFECT_MOUNT: u32 = 2459;
pub const C_SKILL: u32 = 827;
pub const C_CLASS_TABLE: u32 = 853;
pub const C_TRAIT_BASE: u32 = 1479;
pub const C_TRAIT: u32 = 1480;
pub const C_TRAIT_TREE: u32 = 3486;
pub const C_INDUCTION: u32 = 1195;

/// The class library (DID 0x56000000).
pub fn library() -> Vec<u8> {
    LibraryBuilder::new(4000)
        .parent(C_ITEM_BASE as usize, C_ITEM_PARENT)
        .parent(C_ITEM_SUB as usize, C_ITEM_BASE)
        .parent(C_ITEM_LIKE as usize, C_ITEM_PARENT)
        .parent(C_MOB as usize, C_CREATURE_ROOT)
        .parent(C_NPC as usize, C_CREATURE_ROOT)
        .parent(C_AVATAR as usize, C_CREATURE_ROOT)
        .parent(C_EFFECT_INSTANT_VITAL as usize, C_EFFECT_BASE)
        .parent(C_EFFECT_MOUNT as usize, C_EFFECT_BASE)
        .parent(C_TRAIT as usize, C_TRAIT_BASE)
        .build()
}

/// A synthetic install under construction.
#[derive(Debug)]
pub struct Synth {
    pub src: MemorySource,
    pub objects: Vec<Did>,
    weenie: Vec<(u32, u32, String)>,
    uniquedb: Vec<(u32, u32, String)>,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    /// The dictionary, mappers and library; no objects.
    pub fn new() -> Self {
        let mut src = MemorySource::new();
        src.insert(0x3400_0000u32, dictionary());
        src.insert(0x5600_0000u32, library());
        let mappers: &[(u32, u32, &[(u32, &str)])] = &[
            (
                M_WEENIE,
                0,
                &[
                    (1, "Monster"),
                    (2, "RealNPC"),
                    (3, "GameplayContainer"),
                    (4, "Weapon"),
                    (5, "Clothing"),
                    (6, "Landmark"),
                    (7, "Item"),
                ],
            ),
            (
                M_SLOT,
                M_SLOT_BASE,
                &[(0, "None"), (1, "Head"), (2, "Chest")],
            ),
            (M_SLOT_BASE, 0, &[(5, "Equipment"), (6, "Backpack")]),
            (M_ITEM_CLASS, 0, &[(1, "Sword"), (2, "Food")]),
            (M_QUALITY, 0, &[(1, "Common"), (2, "Rare")]),
            (M_CLASS, 0, &[(23, "Minstrel"), (24, "Hunter")]),
            (M_SKILL_TYPE, 0, &[(1, "Melee"), (2, "Ranged"), (3, "Song")]),
            (M_NATURE, 0, &[(1, "Virtue"), (2, "Racial")]),
            (M_SPECIES, 0, &[(1, "Orc"), (2, "Elf")]),
            (M_GENUS, 0, &[(1, "Beast"), (2, "Undead")]),
            (M_ALIGN, 0, &[(1, "Good"), (2, "Evil")]),
            (M_AGENT_CLASS, 0, &[(1, "Soldier"), (2, "Archer")]),
            (M_QUEST_CAT, 0, &[(10, "Epic"), (11, "Hobby")]),
            (M_TIER, 0, &[(1, "Apprentice"), (2, "Journeyman")]),
            (M_RACE, 0, &[(23, "Man"), (81, "Halfling")]),
            (M_DAMAGE, 0, &[(1, "Common"), (2, "Fire")]),
            (
                M_NOTE_TYPE,
                0,
                &[(1, "Link"), (2, "Vendor"), (3, "Trainer")],
            ),
            (M_GENDER, 0, &[(0x1000, "Male"), (0x2000, "Female")]),
            (M_TREE_TYPE, 0, &[(1, "Burglar"), (2, "Hunter")]),
            (M_BRANCH, 0, &[(5, "Blue"), (6, "Red"), (7, "Yellow")]),
            (M_RECIPE_CAT, 0, &[(7, "Swords"), (8, "Axes")]),
        ];
        for (did, base, names) in mappers {
            src.insert(*did, enum_mapper(*did, *base, names));
        }
        Self {
            src,
            objects: Vec::new(),
            weenie: Vec::new(),
            uniquedb: Vec::new(),
        }
    }

    /// Adds object `did` of root `class` with a property set of (label, value) entries
    /// (`None`: no property-set record at all).
    pub fn object(
        &mut self,
        did: u32,
        class: u32,
        entries: Option<&[(&str, Vec<u8>)]>,
    ) -> &mut Self {
        let mut root = did.to_le_bytes().to_vec();
        root.extend_from_slice(&class.to_le_bytes());
        self.src.insert(did, root);
        if let Some(entries) = entries {
            let e: Vec<(u32, Vec<u8>)> = entries.iter().map(|(l, v)| (pid(l), v.clone())).collect();
            let set_did = did + 0x0900_0000;
            let mut rec = set_did.to_le_bytes().to_vec();
            rec.extend_from_slice(&embedded_property_set(&e));
            self.src.insert(set_did, rec);
        }
        self.objects.push(Did(did));
        self
    }

    /// Inserts a raw record (and lists it as an object when its prefix is 0x70–0x77).
    pub fn raw(&mut self, did: u32, payload: Vec<u8>, list: bool) -> &mut Self {
        self.src.insert(did, payload);
        if list {
            self.objects.push(Did(did));
        }
        self
    }

    /// Lists an object DID that no record backs.
    pub fn list_only(&mut self, did: u32) -> &mut Self {
        self.objects.push(Did(did));
        self
    }

    /// Names `did` by a WEENIECONTENT label.
    pub fn weenie(&mut self, key: u32, did: u32, label: &str) -> &mut Self {
        self.weenie.push((key, did, label.to_string()));
        self
    }

    /// Adds a table record named by a UNIQUEDB label.
    pub fn table(&mut self, key: u32, did: u32, label: &str, payload: Vec<u8>) -> &mut Self {
        self.src.insert(did, payload);
        self.uniquedb.push((key, did, label.to_string()));
        self
    }

    /// The store over everything added (root DID map, WEENIECONTENT and UNIQUEDB maps).
    pub fn build(&self) -> GameDataStore<MemorySource> {
        let mut src = self.src.clone();
        let w: Vec<(u32, u32, &str)> = self
            .weenie
            .iter()
            .map(|(k, d, l)| (*k, *d, l.as_str()))
            .collect();
        let u: Vec<(u32, u32, &str)> = self
            .uniquedb
            .iter()
            .map(|(k, d, l)| (*k, *d, l.as_str()))
            .collect();
        src.insert(0x2800_0001u32, did_map(0x2800_0001, &w));
        src.insert(0x2800_0002u32, did_map(0x2800_0002, &u));
        src.insert(
            0x2800_0000u32,
            did_map(
                0x2800_0000,
                &[
                    (0x0A, 0x2800_0001, "WEENIECONTENT"),
                    (0x0F, 0x2800_0002, "UNIQUEDB"),
                ],
            ),
        );
        GameDataStore::new(ObjectStore::new(src), self.objects.clone())
    }
}

/// A places table (`map-data.md` §3.1): (name, region, landblock, cell, offset, area).
pub fn places_table(did: u32, entries: &[(&str, u8, (u8, u8), u16, [f32; 3], u32)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    put_tsize(&mut out, 0, u32::try_from(entries.len()).unwrap());
    for &(name, region, lb, cell, xyz, area) in entries {
        put_utf16(&mut out, name);
        put_position(&mut out, region, lb, cell, xyz);
        out.extend_from_slice(&area.to_le_bytes());
        out.extend_from_slice(&area.to_le_bytes());
    }
    out
}

/// A map-notes table (`map-data.md` §4.1): (region, landblock, cell, offset, entity, type
/// bits); no link rows.
pub fn map_notes_table(did: u32, rows: &[(u8, (u8, u8), u16, [f32; 3], u32, u64)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&u32::try_from(rows.len()).unwrap().to_le_bytes());
    for &(region, lb, cell, xyz, entity, ty) in rows {
        assert_eq!(ty & 1, 0, "no link rows in this builder");
        put_position(&mut out, region, lb, cell, xyz);
        out.extend_from_slice(&0u32.to_le_bytes()); // area
        out.extend_from_slice(&0u32.to_le_bytes()); // dungeon
        out.extend_from_slice(&entity.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // layers: null PID
        put_string_info_literal(&mut out, "note");
        out.extend_from_slice(&0u32.to_le_bytes()); // icon
        out.extend_from_slice(&0u32.to_le_bytes()); // level
        out.extend_from_slice(&ty.to_le_bytes());
        out.push(0); // flag_b
        out.extend_from_slice(&0f32.to_le_bytes());
        out.extend_from_slice(&0f32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // discovery
        put_tsize(&mut out, 0, 0); // extra set
    }
    out
}
