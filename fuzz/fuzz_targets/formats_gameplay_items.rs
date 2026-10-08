// SPDX-License-Identifier: GPL-3.0-or-later
//! Item views and rules (`tessera_formats::gameplay::items`, `docs/specs/items.md`) on
//! arbitrary bytes.
//!
//! Input: byte 0 selects a mode, byte 1 splits the rest into a header part (read as u32
//! words: a class parent map, reference classes and evaluation inputs) and a body that feeds
//! two property sets:
//!
//! 1. **Decoded**: the body parsed as a bare property set. The synthetic dictionary gives
//!    every label the module reads, and the element and field labels of its arrays and
//!    structs, a small PID and the value type the spec gives it ([`TYPED`]); a PID outside
//!    the dictionary gets a type from its number.
//! 2. **Built**: the body read as a stream of choices that builds a set directly: one group
//!    of labels that a record kind carries together ([`GROUPS`]: an item of some kind, a
//!    value, DPS, timing or wear table, a progression, a set, a relic, ...) plus a few
//!    random labels, every value with its spec type, structs and arrays filled from their
//!    own fields and elements. Random bytes rarely parse into deep well-typed sets, so this
//!    path is what reaches the success paths of the table readers, the modifier, effect,
//!    socket, set-tier, member, runic-list and legacy readers, and the kind rules.
//!
//! With a selector of 0x80 or above, about one value in eight (built) or one label in four
//! (decoded) gets another type, which drives the type-mismatch paths inside otherwise
//! well-typed records. Selector bit 0x40 lets the evaluation level take any value instead of
//! 0–11 (which indexes the small built tables).
//!
//! Nothing may panic; discovered classes must classify as items; table lookups and the
//! value, DPS and armour rules must agree with their definitions; stored −1 bounds must
//! read as open; signature kinds must not change when `WeenieType` labels are added.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_items -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::common::ClassTree;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::items::{
    DpsTable, DurationMap, EffectListKind, ItemClassKind, ItemClasses, ItemKind, ItemSchema,
    ItemSetView, ItemView, ItemWeenieKinds, Progression, RelicView, SetMembership, ValueTable,
    WeaponTimingTable, WeaponTimings, WearControl, armour_value, damage_range,
    equipment_category_keys, item_references, item_value, legacy_effects, mod_entry_is_well_formed,
    round_half_up, runic_list, set_directory, truncate, weapon_damage, weapon_timing_table_dids,
};
use tessera_formats::{
    Did, PropertyEntry, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource,
};

/// (label, value type code of `property-system.md` §3, element or field labels).
///
/// Types are the ones `items.md` §5–§7, `combat-data.md` §4.1 / §5.1, `map-data.md` §8.4.1
/// and `quests.md` §8.2 give; where no spec states a type, the one the reader accepts. A
/// struct lists its fields, an array its element labels. Labels starting with `fuzz.` are
/// synthetic element labels (the readers do not look at element labels) and stand-ins: the
/// constant field of a modifier (`fuzz.stat`) and the unlabelled key and number fields of
/// the keyed tables (`items.md` §6.3, §6.4 OPEN).
type Typed = (&'static str, u32, &'static [&'static str]);

const TYPED: &[Typed] = &[
    // §5.1 identity and display
    ("Name", 13, &[]),
    ("PluralName", 13, &[]),
    ("Description", 13, &[]),
    ("Item_Class", 8, &[]),
    ("Item_Quality", 8, &[]),
    ("Item_EquipmentCategory", 14, &[]),
    ("WeenieType", 8, &[]),
    ("Auction_CategoryType", 8, &[]),
    ("Item_Client_Sort_Type", 8, &[]),
    ("Item_MaterialType", 8, &[]),
    ("Icon_Layer_BackgroundDID", 20, &[]),
    ("PhysObj", 20, &[]),
    ("ExternalAppearanceID", 20, &[]),
    ("Item_WornAppearanceMapList", 12, &["fuzz.worn"]),
    ("fuzz.worn", 11, &["fuzz.int", "fuzz.did"]),
    ("Item_ClothingColorSecondary", 9, &[]),
    ("Entity_AdditionalScriptTables", 12, &["fuzz.did"]),
    // §5.2
    ("Item_Level", 15, &[]),
    ("Usage_MinLevel", 15, &[]),
    ("Usage_MaxLevel", 15, &[]),
    ("Item_DistributionType", 8, &[]),
    ("Item_iLevel_Crafting_Offset", 15, &[]),
    ("Item_iLevel_Bonus_Limit", 15, &[]),
    ("Item_iLevel_Bonus_Chance", 9, &[]),
    ("ItemMunging_MinMungeLevel", 15, &[]),
    ("ItemMunging_MaxMungeLevel", 15, &[]),
    ("ItemMunging_ItemLevelOverrideProgression", 20, &[]),
    ("ItemMunging_ItemLevelOverrideProperty", 10, &[]),
    // §5.3
    ("Item_Value", 15, &[]),
    ("Item_ValueLookupTable", 20, &[]),
    ("Barter_IsBarterItem", 21, &[]),
    ("Item_ForceNotSellable", 21, &[]),
    ("Item_Disenchant_Value", 15, &[]),
    ("Item_Disenchant_Component_Result", 20, &[]),
    ("Item_Disenchant_Trophy_List", 20, &[]),
    ("Item_WearState", 8, &[]),
    // §5.4
    ("Inventory_MaxQuantity", 15, &[]),
    ("Inventory_Unique", 21, &[]),
    ("Inventory_UniqueEquipped", 21, &[]),
    ("Item_UniquenessChannel", 8, &[]),
    ("Inventory_CompatibleSlot", 18, &[]),
    ("Inventory_DefaultSlot", 18, &[]),
    ("Inventory_PrecludedSlot", 18, &[]),
    ("Inventory_BindOnAcquire", 21, &[]),
    ("Inventory_BindToAccount", 21, &[]),
    ("Inventory_BindOnEquip", 21, &[]),
    // §5.5
    ("Item_DurabilityEnum", 8, &[]),
    ("Item_DurabilityValue", 9, &[]),
    ("Item_MaxStructurePointsEnum", 8, &[]),
    ("Item_MaxStructurePoints", 15, &[]),
    ("Item_CurStructurePoints", 15, &[]),
    ("Item_RepairCostPerPointProgression", 20, &[]),
    // §5.6
    ("Combat_DPS_LUT", 20, &[]),
    ("Combat_BaseDPS", 9, &[]),
    ("Combat_Damage", 9, &[]),
    ("Combat_DamageVariance", 9, &[]),
    ("Combat_DamageType", 8, &[]),
    ("Combat_WeaponSpeed", 8, &[]),
    ("Item_BaseActionDuration", 9, &[]),
    ("Item_BaseAnimDurationMultiplierMod", 9, &[]),
    ("ItemAdvancement_CombatDPSLevel", 15, &[]),
    ("Combat_WeaponSlayerGenus", 18, &[]),
    ("Combat_WeaponSlayerAddMod", 9, &[]),
    // §5.7
    ("Item_Armor_Value", 15, &[]),
    ("Item_Armor_Value_Lookup_Table", 20, &[]),
    // §5.8 (`combat-data.md` §5.1)
    ("Mod_Array", 12, &["Mod_Entry"]),
    (
        "Mod_Entry",
        11,
        &[
            "Mod_Modified",
            "Mod_Op",
            "fuzz.stat",
            "Mod_Progression",
            "Mod_ProgressionFloor",
            "Mod_ProgressionCeiling",
            "Mod_ModifierList",
            "Mod_ModifierOp",
            "Mod_DescriptionOverride",
        ],
    ),
    ("Mod_Modified", 10, &[]),
    ("Mod_Op", 8, &[]),
    ("fuzz.stat", 9, &[]),
    ("Mod_Progression", 20, &[]),
    ("Mod_ProgressionFloor", 15, &[]),
    ("Mod_ProgressionCeiling", 15, &[]),
    ("Mod_ModifierList", 12, &["Mod_ModifierProperty"]),
    ("Mod_ModifierProperty", 10, &[]),
    ("Mod_ModifierOp", 8, &[]),
    ("Mod_DescriptionOverride", 13, &[]),
    ("Mod_DescriptionOverrideProgression", 20, &[]),
    ("Mod_NextRankDescriptionOverride", 13, &[]),
    ("Mod_NextRankDescriptionOverrideProgression", 20, &[]),
    // §5.9
    ("EffectGenerator_UsageEffectList", 12, &["fuzz.effect"]),
    ("EffectGenerator_EquipperEffectList", 12, &["fuzz.effect"]),
    ("EffectGenerator_TargetEffectList", 12, &["fuzz.effect"]),
    ("EffectGenerator_HotspotEffectList", 12, &["fuzz.effect"]),
    (
        "fuzz.effect",
        11,
        &[
            "EffectGenerator_EffectID",
            "EffectGenerator_EffectSpellcraft",
            "EffectGenerator_EffectSpellcraftFloor",
            "EffectGenerator_EffectSpellcraftCeiling",
            "EffectGenerator_EffectDataList",
        ],
    ),
    ("EffectGenerator_EffectID", 20, &[]),
    ("EffectGenerator_EffectSpellcraft", 9, &[]),
    ("EffectGenerator_EffectSpellcraftFloor", 15, &[]),
    ("EffectGenerator_EffectSpellcraftCeiling", 15, &[]),
    (
        "EffectGenerator_EffectDataList",
        12,
        &["Effect_Crafting_Recipe", "Effect_Crafting_RecipeList"],
    ),
    ("Effect_Crafting_Recipe", 20, &[]),
    (
        "Effect_Crafting_RecipeList",
        12,
        &["Effect_Crafting_Recipe"],
    ),
    ("Item_GrantedSkill", 20, &[]),
    ("Item_GrantedTrait", 20, &[]),
    // §5.10
    ("Usage_Action", 20, &[]),
    ("Usage_Permission", 20, &[]),
    ("Usage_TakePermission", 20, &[]),
    ("CSM_ActionHandler", 20, &[]),
    ("Usage_CooldownDuration", 8, &[]),
    ("Usage_CooldownChannel", 8, &[]),
    ("Usage_EquipCooldownDuration", 8, &[]),
    ("Usage_DestroyOnUse", 21, &[]),
    ("Usage_AllowedInCombat", 21, &[]),
    ("Usage_AllowedWhileMounted", 21, &[]),
    ("Usage_MinUsageDist", 9, &[]),
    ("ItemDecay_Duration", 8, &[]),
    ("Usage_RequiredClassList", 12, &["fuzz.enum"]),
    ("Usage_RequiredRaces", 12, &["fuzz.enum"]),
    (
        "Usage_RequiredFaction",
        11,
        &["Usage_RequiredFaction_DataID", "Usage_RequiredFaction_Tier"],
    ),
    ("Usage_RequiredFaction_DataID", 20, &[]),
    ("Usage_RequiredFaction_Tier", 8, &[]),
    ("Usage_RequiredTrait", 20, &[]),
    ("Usage_RequiredTrait_Array", 12, &["fuzz.required_trait"]),
    (
        "fuzz.required_trait",
        11,
        &["Usage_RequiredTrait", "Usage_RequiredVirtueRank"],
    ),
    ("Usage_RequiredVirtueRank", 15, &[]),
    ("Usage_BarringTrait", 20, &[]),
    ("Usage_BarringSkill", 20, &[]),
    ("Usage_BarringEmote", 20, &[]),
    ("Usage_MinGloryRank", 15, &[]),
    ("Usage_RequiredCraftProfession", 20, &[]),
    ("Usage_RequiredCraftTier", 8, &[]),
    ("Usage_RequiredCraftMastery", 8, &[]),
    ("Usage_QuestRequirements", 12, &["fuzz.quest_requirement"]),
    (
        "fuzz.quest_requirement",
        11,
        &["Usage_Operator", "Usage_QuestID", "Usage_QuestStatus"],
    ),
    ("Usage_Operator", 8, &[]),
    ("Usage_QuestID", 20, &[]),
    ("Usage_QuestStatus", 8, &[]),
    ("Usage_RestrictedEffect", 20, &[]),
    ("Usage_RequiredEffect", 20, &[]),
    ("Usage_TargetType", 18, &[]),
    ("Usage_ValidTargetTypes", 18, &[]),
    ("Usage_SkillToExecute", 20, &[]),
    ("Usage_SkillLevel", 15, &[]),
    // §5.11
    ("Item_Socket_Array", 12, &["fuzz.socket"]),
    (
        "fuzz.socket",
        11,
        &["Item_Socket_Type", "Item_Socket_Unlock_ILevel"],
    ),
    ("Item_Socket_Type", 14, &[]),
    ("Item_Socket_Unlock_ILevel", 15, &[]),
    ("Item_Socket_GemMinLevel", 15, &[]),
    ("Item_Socket_GemMaxLevel", 15, &[]),
    ("Item_Socket_Operation", 20, &[]),
    ("Item_Socket_LevelupRuneIncrement", 15, &[]),
    ("Item_PropertySet", 20, &[]),
    // §5.12, §5.13
    ("ItemAdvancement_Item", 21, &[]),
    ("ItemAdvancement_ProgressionGroupOverride", 20, &[]),
    ("ItemAdvancement_CombatPropertyModDID", 20, &[]),
    ("ItemAdvancement_RunicToCreate", 20, &[]),
    ("ItemAdvancement_Title", 20, &[]),
    ("Item_MaxLevelUpgrades", 15, &[]),
    ("RecipeItem_Recipe", 20, &[]),
    ("Usage_BarringCraftRecipe", 20, &[]),
    ("PackageItem_TrophyListTemplate", 20, &[]),
    ("TextItem_Title", 13, &[]),
    ("TextItem_PrefabContentList", 12, &["fuzz.did"]),
    ("Usage_PlayersCanOpen", 21, &[]),
    ("Usage_PlayersCanClose", 21, &[]),
    ("Key_CanBePlacedOnKeyRing", 21, &[]),
    // §6.1, §6.2
    ("Item_BaseValArray", 12, &["fuzz.base_value"]),
    ("fuzz.base_value", 11, &["Item_BaseVal"]),
    ("Item_BaseVal", 9, &[]),
    ("Item_QualityArray", 12, &["fuzz.quality"]),
    ("fuzz.quality", 11, &["Item_Quality", "Item_QualityModVal"]),
    ("Item_QualityModVal", 9, &[]),
    ("Combat_BaseDPSArray", 12, &["Combat_BaseDPS"]),
    ("Combat_QualityModArray", 12, &["fuzz.dps_quality"]),
    ("fuzz.dps_quality", 11, &["Combat_Quality", "Combat_DPSMod"]),
    ("Combat_Quality", 8, &[]),
    ("Combat_DPSMod", 9, &[]),
    // §6.3, §6.4
    ("Item_WeaponActionDurationTable", 12, &["fuzz.did"]),
    ("Item_WeaponActionDurationDataArray", 12, &["fuzz.timing"]),
    (
        "fuzz.timing",
        11,
        &[
            "Item_WeaponSpeed",
            "Item_BaseActionDuration",
            "Item_BaseAnimDurationMultiplierMod",
        ],
    ),
    ("Item_WeaponSpeed", 8, &[]),
    ("ItemWearControl_MaxStructurePointsArray", 12, &["fuzz.msp"]),
    (
        "fuzz.msp",
        11,
        &[
            "ItemWearControl_MaxStructurePointsEnum",
            "ItemWearControl_MaxStructurePointsValue",
        ],
    ),
    ("ItemWearControl_MaxStructurePointsEnum", 8, &[]),
    ("ItemWearControl_MaxStructurePointsValue", 15, &[]),
    ("ItemWearControl_DurabilityArray", 12, &["fuzz.durability"]),
    (
        "fuzz.durability",
        11,
        &[
            "ItemWearControl_DurabilityEnum",
            "ItemWearControl_DurabilityValue",
        ],
    ),
    ("ItemWearControl_DurabilityEnum", 8, &[]),
    ("ItemWearControl_DurabilityValue", 9, &[]),
    (
        "ItemWearControl_RepairCostQualityModArray",
        12,
        &["fuzz.keyed"],
    ),
    (
        "ItemWearControl_ItemWearStateValueModArray",
        12,
        &["fuzz.keyed"],
    ),
    ("ItemWearControl_SkillGateArray", 12, &["fuzz.keyed"]),
    ("CooldownControl_DurationMapList", 12, &["fuzz.keyed"]),
    ("ItemDecayControl_DurationMapList", 12, &["fuzz.keyed"]),
    ("fuzz.keyed", 11, &["fuzz.enum", "fuzz.number", "fuzz.int"]),
    // `combat-data.md` §4.1, §4.2, §4.6
    ("Progression_MinimumIndexValue", 15, &[]),
    ("PropertyProgression_Array", 12, &["fuzz.stat"]),
    (
        "FloatProgression_Array",
        12,
        &["FloatProgression_FloatValue"],
    ),
    ("FloatProgression_FloatValue", 9, &[]),
    (
        "LinearInterpolatingProgression_Array",
        12,
        &["LinearInterpolatingProgression_ControlPoint"],
    ),
    (
        "LinearInterpolatingProgression_ControlPoint",
        11,
        &[
            "LinearInterpolatingProgression_Key",
            "LinearInterpolatingProgression_Value",
        ],
    ),
    ("LinearInterpolatingProgression_Key", 15, &[]),
    ("LinearInterpolatingProgression_Value", 9, &[]),
    (
        "StringInfoProgression_Array",
        12,
        &["StringInfoProgression_Value"],
    ),
    ("StringInfoProgression_Value", 13, &[]),
    (
        "DataIDProgression_Array",
        12,
        &["DataIDProgression_DataIDValue"],
    ),
    ("DataIDProgression_DataIDValue", 20, &[]),
    ("FloatProgression_ValueProperty", 10, &[]),
    ("LinearInterpolatingProgression_ValueProperty", 10, &[]),
    (
        "ProgressionProgression_InnerRepresentedPropertyName",
        10,
        &[],
    ),
    ("Progression_Type", 8, &[]),
    ("Progression_PointTier", 8, &[]),
    ("SparseDIDProgression_Array", 12, &["fuzz.sparse"]),
    (
        "fuzz.sparse",
        11,
        &[
            "SparseDIDProgressionEntry_Key",
            "SparseDIDProgressionEntry_DID",
        ],
    ),
    ("SparseDIDProgressionEntry_Key", 9, &[]),
    ("SparseDIDProgressionEntry_DID", 20, &[]),
    // §7.1
    ("Set_Directory_GameSetList", 12, &["fuzz.did"]),
    ("Set_Name", 13, &[]),
    ("Set_Description", 13, &[]),
    ("Set_Icon", 20, &[]),
    ("Set_Level", 15, &[]),
    ("Set_LevelRequired", 21, &[]),
    ("Set_MaxPlayerLevel", 15, &[]),
    ("ItemGameSet_UseAverageItemLevelForSetLevel", 21, &[]),
    ("Set_MemberList", 12, &["Set_MemberDID"]),
    ("Set_MemberDID", 20, &[]),
    ("Set_ActiveCountDataList", 12, &["fuzz.tier"]),
    (
        "fuzz.tier",
        11,
        &[
            "Set_ActiveCount",
            "Mod_Array",
            "Set_EffectDataList",
            "Set_StartScriptID",
            "Set_ShortDescriptionForCount",
        ],
    ),
    ("Set_ActiveCount", 15, &[]),
    ("Set_EffectDataList", 12, &["fuzz.effect"]),
    ("Set_StartScriptID", 8, &[]),
    ("Set_ShortDescriptionForCount", 13, &[]),
    ("Set_MinActiveMembers", 15, &[]),
    ("Set_MaxActiveMembers", 15, &[]),
    ("Set_ActiveCountProperty", 10, &[]),
    ("Set_ActiveLevelProperty", 10, &[]),
    // §7.2, §7.3
    ("Runic_Name", 13, &[]),
    ("Runic_Type", 18, &[]),
    ("Runic_Level", 15, &[]),
    ("Runic_Tier", 15, &[]),
    ("Runic_RequiredItemLevel", 15, &[]),
    ("Relic_ValidContainerSlots", 18, &[]),
    ("Runic_Deconstruction_Value", 15, &[]),
    ("RunicList_Array", 12, &["fuzz.runic"]),
    (
        "fuzz.runic",
        11,
        &["RunicList_DropFrequency", "RunicList_RunicOrTreasureGroup"],
    ),
    ("RunicList_DropFrequency", 8, &[]),
    ("RunicList_RunicOrTreasureGroup", 20, &[]),
    ("ItemAdvancement_Effect_Array", 12, &["fuzz.legacy"]),
    (
        "fuzz.legacy",
        11,
        &["ItemAdvancement_Effect", "ItemAdvancement_Mod_Weight"],
    ),
    ("ItemAdvancement_Effect", 20, &[]),
    ("ItemAdvancement_Mod_Weight", 15, &[]),
    // §2.4 reference sources (nested at any depth)
    (
        "fuzz.references",
        12,
        &[
            "Vendor_Item",
            "Barter_Item",
            "TreasureGroupProfile_ItemTable_Item",
        ],
    ),
    ("Vendor_Item", 20, &[]),
    ("Barter_Item", 20, &[]),
    ("TreasureGroupProfile_ItemTable_Item", 20, &[]),
    // Stand-ins.
    ("fuzz.int", 15, &[]),
    ("fuzz.number", 9, &[]),
    ("fuzz.enum", 8, &[]),
    ("fuzz.did", 20, &[]),
];

/// Label groups that one record carries together; the builder starts from one of them.
const GROUPS: &[&[&str]] = &[
    // An item of each signature kind, with the common item labels.
    &[
        "Item_Class",
        "WeenieType",
        "Name",
        "PhysObj",
        "Usage_Action",
        "Icon_Layer_BackgroundDID",
        "Item_Level",
        "Item_Quality",
        "Item_Value",
        "Item_ValueLookupTable",
        "Inventory_BindOnAcquire",
        "Inventory_BindToAccount",
        "Inventory_BindOnEquip",
        "Mod_Array",
        "Item_Socket_Array",
        "EffectGenerator_EquipperEffectList",
        "Item_WornAppearanceMapList",
    ],
    &[
        "Combat_DPS_LUT",
        "Combat_BaseDPS",
        "Combat_Damage",
        "Combat_DamageVariance",
        "Combat_DamageType",
        "Combat_WeaponSpeed",
        "Item_BaseActionDuration",
        "Item_BaseAnimDurationMultiplierMod",
        "ItemAdvancement_CombatDPSLevel",
        "Item_EquipmentCategory",
        "Item_Level",
        "Item_Quality",
        "EffectGenerator_TargetEffectList",
    ],
    &[
        "RecipeItem_Recipe",
        "Usage_BarringCraftRecipe",
        "EffectGenerator_UsageEffectList",
        "Usage_DestroyOnUse",
        "Usage_CooldownDuration",
        "Usage_CooldownChannel",
    ],
    &[
        "PackageItem_TrophyListTemplate",
        "TextItem_Title",
        "TextItem_PrefabContentList",
        "Usage_PlayersCanOpen",
        "Usage_PlayersCanClose",
        "Key_CanBePlacedOnKeyRing",
        "WeenieType",
    ],
    &[
        "Item_DurabilityEnum",
        "Item_DurabilityValue",
        "Item_MaxStructurePointsEnum",
        "Item_MaxStructurePoints",
        "Item_CurStructurePoints",
        "Item_RepairCostPerPointProgression",
        "Item_Armor_Value",
        "Item_Armor_Value_Lookup_Table",
        "Inventory_CompatibleSlot",
        "Inventory_DefaultSlot",
        "Inventory_PrecludedSlot",
    ],
    &[
        "Usage_RequiredClassList",
        "Usage_RequiredRaces",
        "Usage_RequiredFaction",
        "Usage_RequiredTrait",
        "Usage_RequiredTrait_Array",
        "Usage_QuestRequirements",
        "Usage_MinGloryRank",
        "Usage_RequiredCraftProfession",
        "Usage_RequiredCraftTier",
        "Usage_MinLevel",
        "Usage_MaxLevel",
    ],
    &[
        "Item_Socket_Type",
        "Item_Socket_GemMinLevel",
        "Item_Socket_GemMaxLevel",
        "Item_Socket_Operation",
        "Item_Socket_LevelupRuneIncrement",
        "Item_UniquenessChannel",
        "Item_PropertySet",
        "ItemMunging_MinMungeLevel",
        "ItemMunging_ItemLevelOverrideProperty",
        "Item_iLevel_Bonus_Chance",
        "Item_Disenchant_Value",
        "Item_Disenchant_Component_Result",
    ],
    // Tables and control records.
    &["Item_BaseValArray", "Item_QualityArray"],
    &["Combat_BaseDPSArray", "Combat_QualityModArray"],
    &[
        "LinearInterpolatingProgression_Array",
        "LinearInterpolatingProgression_ValueProperty",
    ],
    &["FloatProgression_Array", "Progression_MinimumIndexValue"],
    &["PropertyProgression_Array", "Progression_MinimumIndexValue"],
    &[
        "Item_EquipmentCategory",
        "Item_WeaponActionDurationDataArray",
    ],
    &[
        "ItemWearControl_MaxStructurePointsArray",
        "ItemWearControl_DurabilityArray",
        "ItemWearControl_RepairCostQualityModArray",
        "ItemWearControl_ItemWearStateValueModArray",
        "ItemWearControl_SkillGateArray",
    ],
    &[
        "CooldownControl_DurationMapList",
        "ItemDecayControl_DurationMapList",
        "Item_WeaponActionDurationTable",
    ],
    // Sets, relics, runic lists, legacies, reference sources.
    &[
        "Set_Directory_GameSetList",
        "Set_Name",
        "Set_Level",
        "Set_LevelRequired",
        "Set_MemberList",
        "Set_ActiveCountDataList",
        "Set_ActiveCountProperty",
        "Set_ActiveLevelProperty",
        "Item_PropertySet",
    ],
    &[
        "Runic_Name",
        "Runic_Type",
        "Runic_Level",
        "Runic_Tier",
        "Runic_RequiredItemLevel",
        "Mod_Array",
    ],
    &["RunicList_Array", "ItemAdvancement_Effect_Array"],
    &["fuzz.references", "fuzz.references"],
];

/// The synthetic `WeenieType` labels: (key, label) for every label §2.1 lists.
const WEENIE_LABELS: &[(u32, &str)] = &[
    (1, "Item"),
    (2, "Clothing"),
    (3, "Weapon"),
    (4, "Jewelry"),
    (5, "PackageItem"),
    (6, "Shield"),
    (7, "Letter"),
    (8, "Door"),
    (9, "Waypoint"),
    (10, "Key"),
    (11, "Milestone"),
    (12, "Money"),
    (13, "PopupImage"),
    (14, "Entity"),
    (15, "Hotspot"),
];

struct Fixture {
    names: HashMap<String, u32>,
    /// PID → spec type code.
    types: HashMap<u32, u32>,
    /// PID of each [`TYPED`] entry, by index.
    pids: Vec<u32>,
    /// Label → index into [`TYPED`].
    index: HashMap<&'static str, usize>,
    schema: ItemSchema,
    weenie: ItemWeenieKinds,
}

/// PIDs from 1 up: small, so that random bytes hit them.
fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let mut names: HashMap<String, u32> = HashMap::new();
        let mut next = 0u32;
        let mut pid_of = |label: &str, names: &mut HashMap<String, u32>| {
            *names.entry(label.to_owned()).or_insert_with(|| {
                next += 1;
                next
            })
        };
        let mut types = HashMap::new();
        let mut pids = Vec::new();
        let mut index = HashMap::new();
        for (i, &(label, code, _)) in TYPED.iter().enumerate() {
            let pid = pid_of(label, &mut names);
            types.insert(pid, code);
            pids.push(pid);
            index.entry(label).or_insert(i);
        }
        for label in ItemSchema::labels() {
            pid_of(label, &mut names);
        }
        let schema = ItemSchema::resolve(&names);
        let weenie = ItemWeenieKinds::from_labels(WEENIE_LABELS.iter().copied());
        Fixture {
            names,
            types,
            pids,
            index,
            schema,
            weenie,
        }
    })
}

/// A type code for a PID outside the dictionary.
fn type_of(pid: u32) -> u32 {
    pid % 24
}

/// Another type of the families the readers use, chosen by `salt`.
fn other_type(code: u32, salt: u32) -> u32 {
    const FAMILY: [u32; 10] = [8, 9, 10, 11, 12, 13, 15, 18, 20, 21];
    let pick = FAMILY[(salt as usize) % FAMILY.len()];
    if pick == code { 19 } else { pick }
}

struct Choices<'a> {
    data: &'a [u8],
    at: usize,
}

impl Choices<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.at).copied().unwrap_or(0);
        self.at = self.at.saturating_add(1);
        b
    }
}

/// Entries a built set may hold in all (bounds the work per input).
const BUILD_BUDGET: usize = 384;

/// Builds an entry of label `TYPED[i]` from the choices in `c`.
fn build_entry(
    f: &Fixture,
    c: &mut Choices<'_>,
    i: usize,
    depth: usize,
    budget: &mut usize,
    mismatch: bool,
) -> PropertyEntry {
    let (_, spec_code, children) = TYPED.get(i).copied().unwrap_or(("fuzz.int", 15, &[]));
    let pid = f.pids.get(i).copied().unwrap_or(1);
    let mut code = spec_code;
    if mismatch && c.byte().is_multiple_of(8) {
        code = other_type(code, u32::from(c.byte()));
    }
    let b = c.byte();
    let child = |label: &str| f.index.get(label).copied();
    let value = match code {
        11 => {
            let mut fields = Vec::new();
            if depth < 5 {
                for label in children {
                    if *budget == 0 {
                        break;
                    }
                    if c.byte().is_multiple_of(8) {
                        continue; // a missing field
                    }
                    if let Some(j) = child(label) {
                        *budget -= 1;
                        fields.push(build_entry(f, c, j, depth + 1, budget, mismatch));
                    }
                }
                if c.byte().is_multiple_of(16) && *budget > 0 {
                    *budget -= 1;
                    let j = usize::from(c.byte()) % TYPED.len();
                    fields.push(build_entry(f, c, j, depth + 1, budget, mismatch));
                }
            }
            structure(fields)
        }
        12 => {
            let mut elements = Vec::new();
            if depth < 5 && !children.is_empty() {
                for _ in 0..(b % 8) {
                    if *budget == 0 {
                        break;
                    }
                    let pick = usize::from(c.byte()) % children.len();
                    if let Some(j) = children.get(pick).and_then(|l| child(l)) {
                        *budget -= 1;
                        elements.push(build_entry(f, c, j, depth + 1, budget, mismatch));
                    }
                }
            }
            array(elements)
        }
        8 => PropertyValue::Enum(u32::from(b % 16)),
        9 => PropertyValue::Float(match b {
            250 => f32::NAN,
            251 => f32::INFINITY,
            252 => -1.0,
            253 => f32::MAX,
            _ => f32::from(b) / 8.0 - 4.0,
        }),
        // Property IDs mostly name the constant field of a modifier, so that inline entries
        // occur.
        10 => PropertyValue::PropertyId(match b % 4 {
            0 | 1 => f.names.get("fuzz.stat").copied().unwrap_or(0),
            2 => 0,
            _ => u32::from(b),
        }),
        13 => PropertyValue::StringInfo(Box::new(StringInfo {
            source: StringSource::Table {
                token: u32::from(b),
                table: Did(0x2500_0001),
            },
            args: None,
        })),
        14 => PropertyValue::Bitfield64(if b.is_multiple_of(5) {
            u64::from(b)
        } else {
            1u64 << (b % 64)
        }),
        18 => PropertyValue::Bitfield32(1u32 << (b % 32)),
        20 => did(if b.is_multiple_of(9) {
            0
        } else {
            0x7000_0000 | u32::from(b % 9)
        }),
        21 => PropertyValue::Bool(b & 1 == 1),
        // 15 and anything else: small signed ints, −1 and the extremes included.
        _ => PropertyValue::Int(match b {
            250 => -1,
            251 => 0,
            252 => i32::MAX,
            253 => i32::MIN,
            _ => i32::from(b % 32),
        }),
    };
    entry(pid, value)
}

/// A set built from the choices: one group of [`GROUPS`] (each label kept with probability
/// 7/8), then up to three random labels of [`TYPED`].
fn build_set(f: &Fixture, c: &mut Choices<'_>, mismatch: bool) -> PropertySet {
    let mut budget = BUILD_BUDGET;
    let group = GROUPS
        .get(usize::from(c.byte()) % GROUPS.len())
        .copied()
        .unwrap_or(&[]);
    let mut entries = Vec::new();
    for label in group {
        if c.byte().is_multiple_of(8) {
            continue;
        }
        if let Some(&i) = f.index.get(label) {
            entries.push(build_entry(f, c, i, 0, &mut budget, mismatch));
        }
    }
    for _ in 0..(c.byte() % 4) {
        let i = usize::from(c.byte()) % TYPED.len();
        entries.push(build_entry(f, c, i, 0, &mut budget, mismatch));
    }
    set(entries)
}

fn u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
        .collect()
}

fuzz_target!(|input: &[u8]| {
    let [selector, split, rest @ ..] = input else {
        return;
    };
    let mismatch = *selector >= 0x80;
    let at = usize::from(*split).min(rest.len());
    let (head, body) = rest.split_at(at);
    let words = u32s(head);
    let word = |i: usize| words.get(i).copied().unwrap_or(0);
    // Levels inside small built tables unless selector bit 0x40 asks for any value.
    #[allow(clippy::cast_possible_wrap)]
    let level = if selector & 0x40 == 0 {
        (word(0) % 12) as i32
    } else {
        word(0) as i32
    };
    let quality = word(1) % 8;

    // Class discovery on an arbitrary map.
    let tree = ClassTree::from_edges(
        words
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[a, b]| (a % 32, b % 32)),
    );
    if let Some(c) = ItemClasses::discover(&tree, words.iter().map(|w| w % 32).take(16)) {
        assert_eq!(c.classify(&tree, c.item_base), ItemClassKind::Item);
        assert_ne!(c.classify(&tree, c.item_parent), ItemClassKind::Other);
    }

    let x = f32::from_bits(word(2));
    let _ = round_half_up(x);
    let _ = truncate(x);
    let _ = weapon_damage(x, f32::from_bits(word(3)), f32::from_bits(word(4)));
    let _ = damage_range(x, f32::from_bits(word(4)));
    assert!(equipment_category_keys(u64::from(word(5))).all(|k| (1..=64).contains(&k)));

    let f = fixture();

    // 1. The body decoded with the typed synthetic dictionary.
    let salt = u32::from(*selector);
    let schema = |pid: u32| {
        let code = match f.types.get(&pid) {
            Some(&spec) if !mismatch || !(pid ^ salt).is_multiple_of(4) => spec,
            Some(&spec) => other_type(spec, pid.wrapping_add(salt)),
            None => type_of(pid),
        };
        Some(PropertyType::from_code(code))
    };
    if let Ok(decoded) = PropertySet::parse(body, &schema) {
        exercise(f, &decoded, level, quality, word(5));
    }

    // 2. A set built from the body.
    let mut choices = Choices { data: body, at: 0 };
    let built = build_set(f, &mut choices, mismatch);
    exercise(f, &built, level, quality, word(5));
});

/// Reads `set` with every view and table reader and checks the rules.
#[allow(clippy::too_many_lines)]
fn exercise(f: &Fixture, set: &PropertySet, level: i32, quality: u32, bits: u32) {
    let schema = &f.schema;
    let no_defaults: HashMap<u32, PropertyValue> = HashMap::new();

    // Item view.
    let v = ItemView::new(set, schema);
    let signature = v.signature_kind();
    let kind = v.kind(&f.weenie);
    if !matches!(signature, ItemKind::General | ItemKind::Wearable) {
        assert_eq!(kind, signature, "signatures win over WeenieType labels");
    }
    assert_eq!(v.binding(&no_defaults), v.binding_flags().binding());
    let _ = (
        v.has_item_signature(),
        v.durability(),
        v.slots(),
        v.munging(),
    );
    let _ = (v.name(), v.level(), v.value(), v.value_table(), v.weapon());
    let _ = (
        v.armour_value(),
        v.armour_table(),
        v.behaviours(),
        v.sockets(),
    );
    let _ = (v.cooldown(), v.equip_cooldown(), v.decay(), v.item_set());
    let _ = (v.socketable(), v.disenchant(), v.requirements(), v.slayer());
    let _ = (v.granted_skill(), v.granted_trait(), v.cooldown_channel());
    let _ = (v.min_level_or_default(&no_defaults), v.script_tables());
    for s in v.sockets() {
        if s.is_well_formed() {
            assert_eq!(s.field_count, 2);
        }
    }
    for m in v.mods() {
        assert_ne!(m.range.floor, Some(-1), "−1 floor reads as open");
        assert_ne!(m.range.ceiling, Some(-1), "−1 ceiling reads as open");
        assert!(m.progression.is_none_or(|d| !d.is_null()));
        if !m.range.is_ranged() {
            assert!(m.applies_at(Some(i64::from(level))));
        }
        if mod_entry_is_well_formed(&m) {
            assert_ne!(m.modified, 0);
        }
    }
    for k in EffectListKind::ALL {
        for e in v.effects(k) {
            let _ = e.argument(schema.effects.crafting_recipe_list);
        }
    }
    let _ = item_references(set, &schema.references);

    // Tables.
    let t = &schema.tables;
    if let Ok(table) = ValueTable::from_set(set, t) {
        let r = item_value(Some(&table), level, quality);
        if let (Some(b), Some(m)) = (table.base_value(level), table.quality_factor(quality)) {
            assert_eq!(r, round_half_up(b * m));
        } else {
            assert_eq!(r, None);
        }
    }
    if let Ok(table) = DpsTable::from_set(set, t) {
        let dps = table.dps(level, quality);
        if let (Some(b), Some(m)) = (table.base_dps(level), table.quality_factor(quality)) {
            assert_eq!(dps.map(f32::to_bits), Some((b * m).to_bits()));
        }
    }
    if let Ok(p) = Progression::from_properties(set, false, &schema.progressions) {
        let y = p.value_at(i64::from(level));
        if p.keys_increasing() && !p.is_empty() {
            assert!(y.is_some(), "a well-formed progression gives a value");
        }
        let _ = armour_value(&p, level);
    }
    if let Ok(w) = WearControl::from_set(set, t) {
        let _ = (w.max_structure_points(quality), w.durability_value(quality));
    }
    if let Ok(table) = WeaponTimingTable::from_set(set, t) {
        let timings = WeaponTimings {
            tables: vec![table],
        };
        for speed in 0..8 {
            if let Some(hit) = timings.lookup(u64::from(bits), speed) {
                assert_eq!(hit.speed, speed);
            }
        }
    }
    let _ = weapon_timing_table_dids(set, t);
    for list in [t.cooldown_map, t.decay_map] {
        let durations = DurationMap::from_set(set, list);
        assert!(durations.covers(0));
        let _ = durations.seconds(quality);
    }

    // Sets, relics, runic lists, legacies.
    let s = ItemSetView::new(set, schema);
    let tiers = s.tiers();
    for tier in &tiers {
        for m in &tier.mods {
            assert_ne!(m.range.floor, Some(-1));
        }
    }
    let _ = (s.tiers_ascending(), s.members(), s.level(), s.name());
    let _ = (s.active_count_property(), s.active_level_property());
    let mut membership = SetMembership::new();
    for (i, d) in set_directory(set, &schema.sets).into_iter().enumerate() {
        membership.add_member_list(d, s.members());
        if let Some(m) = v.item_set() {
            membership.add_property_set_member(m, d);
        }
        if i > 64 {
            break;
        }
    }
    for (item, first, second) in membership.conflicts() {
        assert_eq!(membership.set_of(*item), Some(*first));
        assert_ne!(first, second);
    }
    let r = RelicView::new(set, schema);
    let _ = (r.name(), r.runic_type(), r.level(), r.tier(), r.mods());
    let _ = runic_list(set, &schema.aux);
    let _ = legacy_effects(set, &schema.aux);
}
