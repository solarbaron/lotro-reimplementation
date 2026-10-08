// SPDX-License-Identifier: GPL-3.0-or-later
//! Items: item records, their property groups, value/DPS/armour/durability tables and
//! auxiliary item families.
//!
//! Spec: `docs/specs/items.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ItemClasses`], [`ItemClassKind`], [`item_references`] | Find the item base class *I* and its parent *P* from vendor, barter and treasure references; classify a root class | §2.4, §9.1 |
//! | [`ItemKind`], [`ItemWeenieKinds`], [`ItemView::kind`] | Descriptive kinds, recognised by the property signatures of §2.1 and the `WeenieType` label | §2.1, §2.4 rule 4 |
//! | [`ItemSchema`] and its PID tables | Every label this module reads, resolved against the dictionary | §5, §6, §7 |
//! | [`ItemView`] | Typed accessors over an item's property set, by group; dictionary defaults on request | §5, §14 |
//! | [`Binding`], [`BindingFlags`] | The binding combinations | §5.4 |
//! | [`Requirements`], [`Slots`], [`Munging`], [`Socketable`], [`Disenchant`] | Requirement, slot, scaling, socketable and disenchant groups | §5.2–§5.4, §5.10, §5.11 |
//! | [`ModEntry`] (from `combat`), [`mod_entry_is_well_formed`], [`EffectRef`], [`Socket`] | Stat modifiers, effect structs, socket entries | §5.8, §5.9, §5.11 |
//! | [`ValueTable`], [`item_value`], [`round_half_up`] | Value tables and the item-value rule | §6.1, §9.2 |
//! | [`DpsTable`], [`weapon_damage`], [`damage_range`] | DPS tables, the DPS and damage rules | §6.2, §9.3, §9.4 |
//! | [`armour_value`] over a `combat` [`Progression`] | The armour rule | §9.5 |
//! | [`WearControl`], [`WeaponTimingTable`], [`WeaponTimings`], [`DurationMap`] | Control tables | §6.3, §6.4, §9.6–§9.8 |
//! | [`ItemSetView`], [`SetTier`], [`SetMembership`] | Item sets and set membership | §7.1, §9.9 |
//! | [`RelicView`], [`RunicListEntry`], [`LegacyEffect`] | Relics, runic lists, legacy tables | §7.2, §7.3 |
//! | [`ENTITY_STAMP_LABELS`], [`COPY_LABELS`], [`DERIVED_LABELS`], [`property_category`] | Stamps, copies and derived values | §4.2, §4.3, §4.5 |
//!
//! Everything works on property sets decoded by [`crate::property_set`]; PIDs are found by
//! label at run time and no class index or DID of one install is hard-coded. All derived
//! arithmetic is in IEEE-754 single precision, as §9 requires. Nothing here panics on
//! malformed data: a value of the wrong type reads as absent, and table constructors return
//! [`FormatError`] when a table cannot be indexed as the spec describes.
//!
//! `Mod_Array` entries and progressions are read with the readers of
//! [`crate::gameplay::combat`] (`combat-data.md` §4, §5), which `items.md` §5.8 and §9.5
//! point to, so that there is one implementation of each: a stored −1 floor or ceiling and a
//! null `Mod_Progression` read as "not given" there.

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::did::Did;
use crate::enum_mapper::EnumMapper;
use crate::error::{FormatError, Result};
pub use crate::gameplay::combat::{LevelRange, ModEntry, ModEntryKind, Progression};
use crate::gameplay::combat::{ModifierPids, ProgressionPids, mod_entries};
use crate::gameplay::common::{
    ClassIndex, ClassParents, Pid, PropertyAccess, PropertyDefaults, ValueExt, did_references,
};
use crate::gameplay::quests::{QuestRequirement, QuestStatus};
use crate::property::{PropertyEntry, PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// Top-level item properties (`items.md` §5).
    pub struct ItemPids {
        // §5.1 identity and display
        name = "Name",
        plural_name = "PluralName",
        description = "Description",
        item_class = "Item_Class",
        quality = "Item_Quality",
        equipment_category = "Item_EquipmentCategory",
        weenie_type = "WeenieType",
        auction_category = "Auction_CategoryType",
        sort_type = "Item_Client_Sort_Type",
        material_type = "Item_MaterialType",
        icon_background = "Icon_Layer_BackgroundDID",
        phys_obj = "PhysObj",
        external_appearance = "ExternalAppearanceID",
        worn_appearance_list = "Item_WornAppearanceMapList",
        clothing_color_secondary = "Item_ClothingColorSecondary",
        script_tables = "Entity_AdditionalScriptTables",
        // §5.2 level and scaling
        level = "Item_Level",
        min_level = "Usage_MinLevel",
        max_level = "Usage_MaxLevel",
        distribution = "Item_DistributionType",
        ilevel_crafting_offset = "Item_iLevel_Crafting_Offset",
        ilevel_bonus_limit = "Item_iLevel_Bonus_Limit",
        ilevel_bonus_chance = "Item_iLevel_Bonus_Chance",
        munge_min_level = "ItemMunging_MinMungeLevel",
        munge_max_level = "ItemMunging_MaxMungeLevel",
        munge_override_progression = "ItemMunging_ItemLevelOverrideProgression",
        munge_override_property = "ItemMunging_ItemLevelOverrideProperty",
        // §5.3 economy
        value = "Item_Value",
        value_table = "Item_ValueLookupTable",
        barter_item = "Barter_IsBarterItem",
        force_not_sellable = "Item_ForceNotSellable",
        disenchant_value = "Item_Disenchant_Value",
        disenchant_component = "Item_Disenchant_Component_Result",
        disenchant_trophy_list = "Item_Disenchant_Trophy_List",
        wear_state = "Item_WearState",
        // §5.4 inventory, slots and binding
        max_quantity = "Inventory_MaxQuantity",
        unique = "Inventory_Unique",
        unique_equipped = "Inventory_UniqueEquipped",
        uniqueness_channel = "Item_UniquenessChannel",
        compatible_slot = "Inventory_CompatibleSlot",
        default_slot = "Inventory_DefaultSlot",
        precluded_slot = "Inventory_PrecludedSlot",
        bind_on_acquire = "Inventory_BindOnAcquire",
        bind_to_account = "Inventory_BindToAccount",
        bind_on_equip = "Inventory_BindOnEquip",
        // §5.5 durability and wear
        durability_enum = "Item_DurabilityEnum",
        durability_value = "Item_DurabilityValue",
        max_structure_points_enum = "Item_MaxStructurePointsEnum",
        max_structure_points = "Item_MaxStructurePoints",
        cur_structure_points = "Item_CurStructurePoints",
        repair_cost_progression = "Item_RepairCostPerPointProgression",
        // §5.6 weapons
        dps_table = "Combat_DPS_LUT",
        base_dps = "Combat_BaseDPS",
        damage = "Combat_Damage",
        damage_variance = "Combat_DamageVariance",
        damage_type = "Combat_DamageType",
        weapon_speed = "Combat_WeaponSpeed",
        base_action_duration = "Item_BaseActionDuration",
        anim_duration_mod = "Item_BaseAnimDurationMultiplierMod",
        dps_level = "ItemAdvancement_CombatDPSLevel",
        slayer_genus = "Combat_WeaponSlayerGenus",
        slayer_add_mod = "Combat_WeaponSlayerAddMod",
        // §5.7 armour
        armour_value = "Item_Armor_Value",
        armour_table = "Item_Armor_Value_Lookup_Table",
        // §5.9 granted abilities
        granted_skill = "Item_GrantedSkill",
        granted_trait = "Item_GrantedTrait",
        // §5.10 behaviour objects and requirements
        usage_action = "Usage_Action",
        usage_permission = "Usage_Permission",
        take_permission = "Usage_TakePermission",
        action_handler = "CSM_ActionHandler",
        cooldown = "Usage_CooldownDuration",
        cooldown_channel = "Usage_CooldownChannel",
        equip_cooldown = "Usage_EquipCooldownDuration",
        destroy_on_use = "Usage_DestroyOnUse",
        allowed_in_combat = "Usage_AllowedInCombat",
        allowed_while_mounted = "Usage_AllowedWhileMounted",
        min_usage_dist = "Usage_MinUsageDist",
        decay = "ItemDecay_Duration",
        // §5.11 sockets
        socket_array = "Item_Socket_Array",
        socket_type = "Item_Socket_Type",
        socket_unlock_level = "Item_Socket_Unlock_ILevel",
        gem_min_level = "Item_Socket_GemMinLevel",
        gem_max_level = "Item_Socket_GemMaxLevel",
        socket_operation = "Item_Socket_Operation",
        socket_levelup_increment = "Item_Socket_LevelupRuneIncrement",
        property_set = "Item_PropertySet",
        // §5.12 legendary data
        advancement_item = "ItemAdvancement_Item",
        progression_group = "ItemAdvancement_ProgressionGroupOverride",
        combat_property_mod = "ItemAdvancement_CombatPropertyModDID",
        runic_to_create = "ItemAdvancement_RunicToCreate",
        advancement_title = "ItemAdvancement_Title",
        max_level_upgrades = "Item_MaxLevelUpgrades",
        // §5.13 kind-specific groups and §2.1 signatures
        recipe = "RecipeItem_Recipe",
        barring_recipe = "Usage_BarringCraftRecipe",
        package_trophy_list = "PackageItem_TrophyListTemplate",
        text_title = "TextItem_Title",
        text_content = "TextItem_PrefabContentList",
        players_can_open = "Usage_PlayersCanOpen",
        players_can_close = "Usage_PlayersCanClose",
        key_ring = "Key_CanBePlacedOnKeyRing",
    }
}

crate::property_ids! {
    /// Requirement and limit labels of item usage (`items.md` §5.10). Field types not
    /// given there follow `map-data.md` §8.4.1 and `quests.md` §8.2 (quest requirements,
    /// faction tier).
    pub struct ItemRequirementPids {
        required_classes = "Usage_RequiredClassList",
        required_races = "Usage_RequiredRaces",
        required_faction = "Usage_RequiredFaction",
        required_faction_did = "Usage_RequiredFaction_DataID",
        required_faction_tier = "Usage_RequiredFaction_Tier",
        required_trait = "Usage_RequiredTrait",
        required_trait_array = "Usage_RequiredTrait_Array",
        required_virtue_rank = "Usage_RequiredVirtueRank",
        barring_trait = "Usage_BarringTrait",
        barring_skill = "Usage_BarringSkill",
        barring_emote = "Usage_BarringEmote",
        min_glory_rank = "Usage_MinGloryRank",
        required_craft_profession = "Usage_RequiredCraftProfession",
        required_craft_tier = "Usage_RequiredCraftTier",
        required_craft_mastery = "Usage_RequiredCraftMastery",
        quest_requirements = "Usage_QuestRequirements",
        quest_operator = "Usage_Operator",
        quest_id = "Usage_QuestID",
        quest_status = "Usage_QuestStatus",
        restricted_effect = "Usage_RestrictedEffect",
        required_effect = "Usage_RequiredEffect",
        target_type = "Usage_TargetType",
        valid_target_types = "Usage_ValidTargetTypes",
        skill_to_execute = "Usage_SkillToExecute",
        skill_level = "Usage_SkillLevel",
    }
}

crate::property_ids! {
    /// Fields of `EffectGenerator_EffectStruct` and the four effect lists, shared by items,
    /// sets and relics (`items.md` §5.9). `Mod_Array` fields are
    /// [`crate::gameplay::combat::ModifierPids`].
    pub struct EffectStructPids {
        usage_effects = "EffectGenerator_UsageEffectList",
        equipper_effects = "EffectGenerator_EquipperEffectList",
        target_effects = "EffectGenerator_TargetEffectList",
        hotspot_effects = "EffectGenerator_HotspotEffectList",
        effect_id = "EffectGenerator_EffectID",
        effect_spellcraft = "EffectGenerator_EffectSpellcraft",
        effect_floor = "EffectGenerator_EffectSpellcraftFloor",
        effect_ceiling = "EffectGenerator_EffectSpellcraftCeiling",
        effect_data = "EffectGenerator_EffectDataList",
        crafting_recipe = "Effect_Crafting_Recipe",
        crafting_recipe_list = "Effect_Crafting_RecipeList",
    }
}

crate::property_ids! {
    /// Shared lookup and control tables (`items.md` §6, §9.5).
    pub struct ItemTablePids {
        base_value_array = "Item_BaseValArray",
        base_value = "Item_BaseVal",
        quality_array = "Item_QualityArray",
        quality = "Item_Quality",
        quality_mod = "Item_QualityModVal",
        dps_array = "Combat_BaseDPSArray",
        dps_quality_array = "Combat_QualityModArray",
        dps_quality = "Combat_Quality",
        dps_mod = "Combat_DPSMod",
        timing_directory = "Item_WeaponActionDurationTable",
        timing_category = "Item_EquipmentCategory",
        timing_array = "Item_WeaponActionDurationDataArray",
        timing_speed = "Item_WeaponSpeed",
        timing_duration = "Item_BaseActionDuration",
        timing_anim_mod = "Item_BaseAnimDurationMultiplierMod",
        wear_msp_array = "ItemWearControl_MaxStructurePointsArray",
        wear_msp_enum = "ItemWearControl_MaxStructurePointsEnum",
        wear_msp_value = "ItemWearControl_MaxStructurePointsValue",
        wear_durability_array = "ItemWearControl_DurabilityArray",
        wear_durability_enum = "ItemWearControl_DurabilityEnum",
        wear_durability_value = "ItemWearControl_DurabilityValue",
        wear_repair_cost_array = "ItemWearControl_RepairCostQualityModArray",
        wear_state_array = "ItemWearControl_ItemWearStateValueModArray",
        wear_skill_gate_array = "ItemWearControl_SkillGateArray",
        cooldown_map = "CooldownControl_DurationMapList",
        decay_map = "ItemDecayControl_DurationMapList",
    }
}

crate::property_ids! {
    /// Item sets and their directory (`items.md` §7.1).
    pub struct ItemSetPids {
        directory = "Set_Directory_GameSetList",
        name = "Set_Name",
        description = "Set_Description",
        icon = "Set_Icon",
        level = "Set_Level",
        level_required = "Set_LevelRequired",
        max_player_level = "Set_MaxPlayerLevel",
        use_average_item_level = "ItemGameSet_UseAverageItemLevelForSetLevel",
        member_list = "Set_MemberList",
        member = "Set_MemberDID",
        tiers = "Set_ActiveCountDataList",
        active_count = "Set_ActiveCount",
        effects = "Set_EffectDataList",
        start_script = "Set_StartScriptID",
        short_description = "Set_ShortDescriptionForCount",
        min_active_members = "Set_MinActiveMembers",
        max_active_members = "Set_MaxActiveMembers",
        active_count_property = "Set_ActiveCountProperty",
        active_level_property = "Set_ActiveLevelProperty",
    }
}

crate::property_ids! {
    /// Relics, runic lists and legacy tables (`items.md` §7.2, §7.3).
    pub struct ItemAuxPids {
        runic_name = "Runic_Name",
        runic_type = "Runic_Type",
        runic_level = "Runic_Level",
        runic_tier = "Runic_Tier",
        runic_required_item_level = "Runic_RequiredItemLevel",
        relic_container_slots = "Relic_ValidContainerSlots",
        runic_deconstruction_value = "Runic_Deconstruction_Value",
        runic_list_array = "RunicList_Array",
        runic_list_frequency = "RunicList_DropFrequency",
        runic_list_target = "RunicList_RunicOrTreasureGroup",
        legacy_effect_array = "ItemAdvancement_Effect_Array",
        legacy_effect = "ItemAdvancement_Effect",
        legacy_weight = "ItemAdvancement_Mod_Weight",
    }
}

crate::property_ids! {
    /// The leaf properties of vendor, barter and treasure-group tables whose DIDs name
    /// items (`items.md` §2.4 step 1).
    pub struct ItemReferencePids {
        vendor_item = "Vendor_Item",
        barter_item = "Barter_Item",
        treasure_item = "TreasureGroupProfile_ItemTable_Item",
    }
}

/// Every PID table this module uses, resolved together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSchema {
    /// Top-level item properties.
    pub item: ItemPids,
    /// Requirement and limit labels.
    pub requirements: ItemRequirementPids,
    /// Effect struct fields and effect lists.
    pub effects: EffectStructPids,
    /// `Mod_Array` and `Mod_Entry` fields (`combat-data.md` §5.1).
    pub modifiers: ModifierPids,
    /// Progression fields (`combat-data.md` §4.1, §4.2), for armour tables (§9.5).
    pub progressions: ProgressionPids,
    /// Lookup and control tables.
    pub tables: ItemTablePids,
    /// Item sets.
    pub sets: ItemSetPids,
    /// Relics, runic lists and legacies.
    pub aux: ItemAuxPids,
    /// Reference sources for class discovery.
    pub references: ItemReferencePids,
}

impl ItemSchema {
    /// Resolves every label against `names` (normally the property dictionary).
    #[must_use]
    pub fn resolve<N: crate::PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            item: ItemPids::resolve(names),
            requirements: ItemRequirementPids::resolve(names),
            effects: EffectStructPids::resolve(names),
            modifiers: ModifierPids::resolve(names),
            progressions: ProgressionPids::resolve(names),
            tables: ItemTablePids::resolve(names),
            sets: ItemSetPids::resolve(names),
            aux: ItemAuxPids::resolve(names),
            references: ItemReferencePids::resolve(names),
        }
    }

    /// Every label of every table, in table order (labels shared by two tables appear
    /// twice).
    #[must_use]
    pub fn labels() -> Vec<&'static str> {
        [
            ItemPids::LABELS,
            ItemRequirementPids::LABELS,
            EffectStructPids::LABELS,
            ModifierPids::LABELS,
            ProgressionPids::LABELS,
            ItemTablePids::LABELS,
            ItemSetPids::LABELS,
            ItemAuxPids::LABELS,
            ItemReferencePids::LABELS,
        ]
        .concat()
    }

    /// The labels the dictionary does not define, across all tables.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.item.missing();
        out.extend(self.requirements.missing());
        out.extend(self.effects.missing());
        out.extend(self.modifiers.missing());
        out.extend(self.progressions.missing());
        out.extend(self.tables.missing());
        out.extend(self.sets.missing());
        out.extend(self.aux.missing());
        out.extend(self.references.missing());
        out
    }
}

/// WEENIECONTENT labels of the control records items depend on (`items.md` §6;
/// `property-system.md` §8.1). Interoperability names; the DIDs they name are build-specific.
pub mod control_labels {
    /// The weapon-timing directory (§6.3).
    pub const WEAPON_ACTION_DURATION_TABLE: &str = "WeaponActionDurationTable";
    /// The wear control (§6.3).
    pub const ITEM_WEAR_CONTROL: &str = "ItemWearControl";
    /// The cooldown control (§6.4).
    pub const COOLDOWN_CONTROL: &str = "CooldownControl";
    /// The item-decay control (§6.4).
    pub const ITEM_DECAY_CONTROL: &str = "ItemDecayControl";
    /// The inventory control (§6.4).
    pub const INVENTORY_CONTROL: &str = "InventoryControl";
    /// The item-munging control (§6.4).
    pub const ITEM_MUNGING_CONTROL: &str = "ItemMungingControl";
    /// The socket control (§6.4).
    pub const ITEM_SOCKET_CONTROL: &str = "ItemSocketControl";
    /// The item-advancement control (§6.4, §7.3).
    pub const ITEM_ADVANCEMENT_CONTROL: &str = "ItemAdvancementControl";
    /// The set directory (§6.4, §7.1).
    pub const GAME_SET_DIRECTORY: &str = "GameSetDirectory";
    /// The relic transmutation directory (§6.4, §7.2).
    pub const RELIC_TRANSMUTATION_DIRECTORY: &str = "RelicTransmutationDirectory";
    /// The paper-item control (§6.4).
    pub const PAPER_ITEM_CONTROL: &str = "PaperItemControl";
    /// The cosmetic companion directory (§6.4, §7.4).
    pub const COSMETIC_PET_DIRECTORY: &str = "CosmeticPetDirectory";
    /// The wardrobe control (§6.4).
    pub const WARDROBE_CONTROL: &str = "WardrobeControl";
    /// A record that is an item; the fast-hint check of §2.4.
    pub const BANK_FILL_ITEM: &str = "BankFillItem";
}

// ---------------------------------------------------------------------------------------------
// Stamps, copies and derived values (§4.2, §4.3, §4.5)
// ---------------------------------------------------------------------------------------------

/// The eight entity stamps of §4.2: present with one value (the dictionary default) in
/// every record under the placeable-entity root and carrying no item information.
pub const ENTITY_STAMP_LABELS: [&str; 8] = [
    "MC_Trait_Loremaster_BredForWar_AttackSpeed",
    "MC_Trait_Loremaster_BredForWar_BleedDamage",
    "MC_Trait_Loremaster_BredForWar_CritChance",
    "MC_Trait_Loremaster_BredForWar_FlankChance",
    "MC_Trait_Loremaster_BredForWar_Threat",
    "BigBattle_OfficerBannerCanHeal",
    "Boss_Zone_1",
    "Skirmish_RoleCombo",
];

/// The copies of §4.3 as (copy, source) label pairs. The first eight copy the entity stamps
/// and are part of the sixteen labels of §4.2; the last three copy item properties.
pub const COPY_LABELS: [(&str, &str); 11] = [
    (
        "Forward_Cached_MC_Trait_Loremaster_BredForWar_AttackSpeed",
        "MC_Trait_Loremaster_BredForWar_AttackSpeed",
    ),
    (
        "Forward_Cached_MC_Trait_Loremaster_BredForWar_BleedDamage",
        "MC_Trait_Loremaster_BredForWar_BleedDamage",
    ),
    (
        "Forward_Cached_MC_Trait_Loremaster_BredForWar_CritChance",
        "MC_Trait_Loremaster_BredForWar_CritChance",
    ),
    (
        "Forward_Cached_MC_Trait_Loremaster_BredForWar_FlankChance",
        "MC_Trait_Loremaster_BredForWar_FlankChance",
    ),
    (
        "Forward_Cached_MC_Trait_Loremaster_BredForWar_Threat",
        "MC_Trait_Loremaster_BredForWar_Threat",
    ),
    ("ForwardSource_Skirmish_RoleCombo", "Skirmish_RoleCombo"),
    ("ForwardSource_Boss_Zone_1", "Boss_Zone_1"),
    (
        "BigBattle_OfficerBannerCanHeal_Cached",
        "BigBattle_OfficerBannerCanHeal",
    ),
    ("Usage_Action_Shadow", "Usage_Action"),
    ("Usage_MinUsageDist_Shadow", "Usage_MinUsageDist"),
    ("Item_CurStructurePoints", "Item_MaxStructurePoints"),
];

/// Number of [`COPY_LABELS`] entries that copy an entity stamp (§4.2's eight copies).
pub const ENTITY_STAMP_COPIES: usize = 8;

/// The sixteen labels of §4.2: the eight stamps and their eight copies.
#[must_use]
pub fn entity_labels() -> [&'static str; 16] {
    let mut out = [""; 16];
    for (o, l) in out.iter_mut().zip(
        ENTITY_STAMP_LABELS
            .iter()
            .chain(COPY_LABELS.iter().take(ENTITY_STAMP_COPIES).map(|(c, _)| c)),
    ) {
        *o = l;
    }
    out
}

/// Stored labels whose values are derived from other values and shared tables (§4.5).
/// `Item_Armor_Value` is derived only where the item has an armour table.
pub const DERIVED_LABELS: [&str; 8] = [
    "Item_Value",
    "Combat_BaseDPS",
    "Combat_Damage",
    "Item_Armor_Value",
    "Item_MaxStructurePoints",
    "Item_DurabilityValue",
    "Item_BaseActionDuration",
    "Item_BaseAnimDurationMultiplierMod",
];

/// How a property of an item's set relates to item information (§4.1). Class-uniform
/// values (§4.4) are an observation per class, not a label property, and are reported as
/// [`ItemPropertyCategory::Authored`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemPropertyCategory {
    /// An entity stamp (§4.2): no item information.
    EntityStamp,
    /// A copy of another property of the same set (§4.3).
    Copy,
    /// A value derived from other values and tables (§4.5, §9).
    Derived,
    /// Everything else (§4.4, §4.6).
    Authored,
}

/// The category of the property with dictionary label `label`. [`COPY_LABELS`] wins over
/// [`DERIVED_LABELS`] (`Item_CurStructurePoints` is a copy).
#[must_use]
pub fn property_category(label: &str) -> ItemPropertyCategory {
    if ENTITY_STAMP_LABELS.contains(&label) {
        ItemPropertyCategory::EntityStamp
    } else if COPY_LABELS.iter().any(|(c, _)| *c == label) {
        ItemPropertyCategory::Copy
    } else if DERIVED_LABELS.contains(&label) {
        ItemPropertyCategory::Derived
    } else {
        ItemPropertyCategory::Authored
    }
}

// ---------------------------------------------------------------------------------------------
// Classification (§2.4, §9.1)
// ---------------------------------------------------------------------------------------------

/// Where a root class stands relative to the item classes (§2.4 rules 2–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemClassKind {
    /// The class is *I* or descends from it: an item.
    Item,
    /// The class descends from *P* but not from *I* (for example the carry-alls of §2.2):
    /// treated as an item by inventory code, but not an item.
    ItemLike,
    /// Neither.
    Other,
}

/// The item base class *I* and its parent *P* (§2.4), found at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemClasses {
    /// *I*: every item's root class is this class or descends from it.
    pub item_base: u32,
    /// *P*: the parent of *I*; its subtree also holds the item-like records.
    pub item_parent: u32,
}

impl ItemClasses {
    /// Finds *P* and *I* from the root classes of the records that vendor, barter and
    /// treasure tables name, one class per reference (§2.4 step 1):
    ///
    /// - *P* is the deepest class that is an ancestor-or-self of every referenced class;
    /// - *I* is the child of *P* whose subtree holds the most references. References whose
    ///   class is *P* itself count for no child. On a tie the lower class index wins (a
    ///   Tessera choice; the spec does not cover ties).
    ///
    /// `None` when there are no references, the classes have no common ancestor, or every
    /// reference names a record of class *P* itself.
    #[must_use]
    pub fn discover<P: ClassParents + ?Sized>(
        parents: &P,
        reference_classes: impl IntoIterator<Item = u32>,
    ) -> Option<Self> {
        let mut per_class: BTreeMap<u32, usize> = BTreeMap::new();
        for c in reference_classes {
            *per_class.entry(c).or_default() += 1;
        }
        let item_parent = parents.common_ancestor(per_class.keys().copied())?;
        let mut per_child: BTreeMap<u32, usize> = BTreeMap::new();
        for (&class, &n) in &per_class {
            if let Some(child) = parents.child_toward(item_parent, class) {
                *per_child.entry(child).or_default() += n;
            }
        }
        // Highest count; on a tie the first (lowest) class in ascending order.
        let mut best: Option<(u32, usize)> = None;
        for (&child, &n) in &per_child {
            if best.is_none_or(|(_, b)| n > b) {
                best = Some((child, n));
            }
        }
        let (item_base, _) = best?;
        Some(Self {
            item_base,
            item_parent,
        })
    }

    /// [`ItemClasses::discover`] over referenced DIDs, looked up in `index`. DIDs that the
    /// index does not hold are ignored.
    #[must_use]
    pub fn discover_from_index<P: ClassParents + ?Sized>(
        parents: &P,
        index: &ClassIndex,
        references: impl IntoIterator<Item = Did>,
    ) -> Option<Self> {
        Self::discover(
            parents,
            references.into_iter().filter_map(|d| index.root_class(d)),
        )
    }

    /// Uses `item_base` as *I* and its parent as *P* (the fast-hint path of §2.4). `None`
    /// when `item_base` has no parent.
    #[must_use]
    pub fn from_item_base<P: ClassParents + ?Sized>(parents: &P, item_base: u32) -> Option<Self> {
        let item_parent = parents.parent(item_base)?;
        Some(Self {
            item_base,
            item_parent,
        })
    }

    /// Classifies a root class (§9.1 steps 1–2).
    #[must_use]
    pub fn classify<P: ClassParents + ?Sized>(&self, parents: &P, class: u32) -> ItemClassKind {
        let mut parent_seen = false;
        for c in parents.lineage(class) {
            if c == self.item_base {
                return ItemClassKind::Item;
            }
            if c == self.item_parent {
                parent_seen = true;
            }
        }
        if parent_seen {
            ItemClassKind::ItemLike
        } else {
            ItemClassKind::Other
        }
    }

    /// Whether `class` is an item class (§2.4 rule 2).
    #[must_use]
    pub fn is_item<P: ClassParents + ?Sized>(&self, parents: &P, class: u32) -> bool {
        self.classify(parents, class) == ItemClassKind::Item
    }

    /// Whether `class` is item-like (§2.4 rule 3): an item or another class under *P*.
    #[must_use]
    pub fn is_item_like<P: ClassParents + ?Sized>(&self, parents: &P, class: u32) -> bool {
        self.classify(parents, class) != ItemClassKind::Other
    }
}

/// The DIDs in `set` that vendor, barter and treasure tables name as items (§2.4 step 1):
/// every non-null DID value, at any depth, whose own PID is one of [`ItemReferencePids`].
#[must_use]
pub fn item_references(set: &PropertySet, pids: &ItemReferencePids) -> Vec<Did> {
    did_references(set)
        .into_iter()
        .filter(|r| {
            pids.vendor_item.is(r.pid) || pids.barter_item.is(r.pid) || pids.treasure_item.is(r.pid)
        })
        .map(|r| r.target)
        .collect()
}

/// A descriptive item kind (§2.1 descriptive labels; §2.4 rule 4).
///
/// The kind is a reading aid for kind-specific code (the explorer card, the sandbox
/// server); the authoritative kind is the root class. [`ItemView::kind`] tries, in order:
/// the signatures of §2.1 that are present in every record of their class or exclusive to
/// it (weapon, recipe scroll, multi-recipe scroll, package, text item, door, key), then
/// the `WeenieType` label ([`ItemWeenieKinds`]), then `Item_WornAppearanceMapList`
/// ([`ItemKind::Wearable`]); [`ItemKind::General`] matches everything else.
///
/// §2.1's in-world interactive objects share the `WeenieType` label `Door` with the
/// doors and read as [`ItemKind::Door`]; the property-modifying tokens read as
/// [`ItemKind::General`]. §2.1 does not name a signature that separates either kind
/// (`items.md` §2.4 OPEN, implementer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKind {
    /// Has `Combat_DPS_LUT`, `Combat_BaseDPS`, `Combat_Damage`, `Combat_DamageVariance`,
    /// `Combat_DamageType` and `Combat_WeaponSpeed`, or `WeenieType` label `Weapon`.
    Weapon,
    /// Has `RecipeItem_Recipe` and `Usage_BarringCraftRecipe`.
    RecipeScroll,
    /// A usage effect whose data list carries `Effect_Crafting_RecipeList`.
    MultiRecipeScroll,
    /// Has `PackageItem_TrophyListTemplate`, or `WeenieType` label `PackageItem`.
    Package,
    /// Has `TextItem_Title` and `TextItem_PrefabContentList`, or `WeenieType` label
    /// `Letter`.
    TextItem,
    /// Has `Usage_PlayersCanOpen` or `Usage_PlayersCanClose`, or `WeenieType` label `Door`
    /// (in-world doors and interactive objects).
    Door,
    /// Has `Key_CanBePlacedOnKeyRing`, or `WeenieType` label `Key`.
    Key,
    /// `WeenieType` label `Jewelry`.
    Jewellery,
    /// `WeenieType` label `Shield`.
    Shield,
    /// `WeenieType` label `Waypoint`: in-world travel points.
    TravelPoint,
    /// `WeenieType` label `Milestone`: in-world milestones.
    Milestone,
    /// `WeenieType` label `Clothing`, or (with no kind-giving label) has
    /// `Item_WornAppearanceMapList`: worn apparel, plus the few general items with a worn
    /// look.
    Wearable,
    /// None of the above.
    General,
}

impl ItemKind {
    /// Every kind, in rule order.
    pub const ALL: [Self; 13] = [
        Self::Weapon,
        Self::RecipeScroll,
        Self::MultiRecipeScroll,
        Self::Package,
        Self::TextItem,
        Self::Door,
        Self::Key,
        Self::Jewellery,
        Self::Shield,
        Self::TravelPoint,
        Self::Milestone,
        Self::Wearable,
        Self::General,
    ];

    /// The kind a `WeenieType` label gives (§2.1 `WeenieType` column). `None` for labels
    /// that give no kind (`Item`, `Money`, `PopupImage`, `Entity`, `Hotspot` and every label
    /// §2.1 does not list).
    #[must_use]
    pub fn from_weenie_label(label: &str) -> Option<Self> {
        Some(match label {
            "Weapon" => Self::Weapon,
            "Clothing" => Self::Wearable,
            "Jewelry" => Self::Jewellery,
            "Shield" => Self::Shield,
            "PackageItem" => Self::Package,
            "Letter" => Self::TextItem,
            "Door" => Self::Door,
            "Key" => Self::Key,
            "Waypoint" => Self::TravelPoint,
            "Milestone" => Self::Milestone,
            _ => return None,
        })
    }
}

/// `WeenieType` enum key → item kind, built from the install's enum mapper (§2.4 rule 4,
/// §11: the mapper the `WeenieType` definition names, and its base chain).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemWeenieKinds {
    by_key: HashMap<u32, ItemKind>,
}

impl ItemWeenieKinds {
    /// Builds the map from (key, label) pairs; for a key listed twice the first label wins.
    #[must_use]
    pub fn from_labels<'a>(labels: impl IntoIterator<Item = (u32, &'a str)>) -> Self {
        let mut seen = HashSet::new();
        let mut by_key = HashMap::new();
        for (key, label) in labels {
            if !seen.insert(key) {
                continue;
            }
            if let Some(kind) = ItemKind::from_weenie_label(label) {
                by_key.insert(key, kind);
            }
        }
        Self { by_key }
    }

    /// Builds the map from a mapper and its base chain, nearest first. A key's label is
    /// taken from the first mapper that names it.
    #[must_use]
    pub fn from_mapper_chain<M: Borrow<EnumMapper>>(chain: &[M]) -> Self {
        Self::from_labels(
            chain
                .iter()
                .flat_map(|m| m.borrow().names().iter().map(|(k, n)| (*k, n.as_str()))),
        )
    }

    /// The kind of `WeenieType` key `key`, if its label gives one.
    #[must_use]
    pub fn kind(&self, key: u32) -> Option<ItemKind> {
        self.by_key.get(&key).copied()
    }

    /// Number of keys whose label gives a kind.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    /// Whether no key gives a kind (signatures only).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

// ---------------------------------------------------------------------------------------------
// Item view (§5)
// ---------------------------------------------------------------------------------------------

/// Binding flags as stored (§5.4); `None` means the label is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BindingFlags {
    /// `Inventory_BindOnAcquire`.
    pub on_acquire: Option<bool>,
    /// `Inventory_BindToAccount`.
    pub to_account: Option<bool>,
    /// `Inventory_BindOnEquip`.
    pub on_equip: Option<bool>,
}

/// The reading of a binding combination (§5.4; the readings are Reported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binding {
    /// (true, true, false or absent): bound to the account when acquired.
    AccountOnAcquire,
    /// (true, false or absent, false or absent): bound to the character when acquired.
    CharacterOnAcquire,
    /// (false or absent, false or absent, true): bound to the character when equipped.
    CharacterOnEquip,
    /// All false or absent: not bound.
    Unbound,
    /// Any other combination, for example (false, true, true), whose reading is OPEN.
    Other(BindingFlags),
}

impl BindingFlags {
    /// The reading of these flags (§5.4 table; §13.6). A flag that is `None` reads as
    /// false; [`ItemView::binding`] first fills absent flags with their dictionary
    /// defaults (§14 decision), which are false on the install §5.4 describes.
    #[must_use]
    pub fn binding(self) -> Binding {
        let a = self.on_acquire.unwrap_or(false);
        let t = self.to_account.unwrap_or(false);
        let e = self.on_equip.unwrap_or(false);
        match (a, t, e) {
            (true, true, false) => Binding::AccountOnAcquire,
            (true, false, false) => Binding::CharacterOnAcquire,
            (false, false, true) => Binding::CharacterOnEquip,
            (false, false, false) => Binding::Unbound,
            _ => Binding::Other(self),
        }
    }
}

/// Whether a `Mod_Entry` satisfies invariant IT9 (§12): it has `Mod_Modified` (non-zero),
/// `Mod_Op`, and a non-null `Mod_Progression` or a field whose PID equals `Mod_Modified`.
/// The entry is read by [`crate::gameplay::combat::ModEntry::from_struct`], which treats a
/// null `Mod_Progression` as absent (`combat-data.md` §5.1: 0 = none).
#[must_use]
pub fn mod_entry_is_well_formed(entry: &ModEntry) -> bool {
    entry.modified != 0
        && entry.op.is_some()
        && (entry.progression.is_some() || entry.value.is_some())
}

/// The four effect lists of an item (§5.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectListKind {
    /// `EffectGenerator_UsageEffectList`: applied when the item is used (Reported).
    Usage,
    /// `EffectGenerator_EquipperEffectList`: applied while equipped (Reported).
    Equipper,
    /// `EffectGenerator_TargetEffectList`: applied to a weapon's target (Reported).
    Target,
    /// `EffectGenerator_HotspotEffectList`.
    Hotspot,
}

impl EffectListKind {
    /// All four lists, in this order.
    pub const ALL: [Self; 4] = [Self::Usage, Self::Equipper, Self::Target, Self::Hotspot];

    /// The list's property.
    #[must_use]
    pub fn pid(self, pids: &EffectStructPids) -> Pid {
        match self {
            Self::Usage => pids.usage_effects,
            Self::Equipper => pids.equipper_effects,
            Self::Target => pids.target_effects,
            Self::Hotspot => pids.hotspot_effects,
        }
    }
}

/// One `EffectGenerator_EffectStruct` (§5.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectRef<'a> {
    /// `EffectGenerator_EffectID`, null DID included.
    pub effect: Option<Did>,
    /// `EffectGenerator_EffectSpellcraft` (−1 is common; its meaning is OPEN).
    pub spellcraft: Option<f32>,
    /// `EffectGenerator_EffectSpellcraftFloor`.
    pub floor: Option<i32>,
    /// `EffectGenerator_EffectSpellcraftCeiling`.
    pub ceiling: Option<i32>,
    /// The elements of `EffectGenerator_EffectDataList` (arguments for the effect); empty
    /// when absent.
    pub data: &'a [PropertyEntry],
}

impl<'a> EffectRef<'a> {
    /// Reads one effect struct.
    #[must_use]
    pub fn from_struct(s: &'a PropertyStruct, pids: &EffectStructPids) -> Self {
        Self {
            effect: s.did(pids.effect_id),
            spellcraft: s.float(pids.effect_spellcraft),
            floor: s.int(pids.effect_floor),
            ceiling: s.int(pids.effect_ceiling),
            data: s.elements(pids.effect_data),
        }
    }

    /// The value of argument `pid` in the data list (the first element with that PID).
    #[must_use]
    pub fn argument(&self, pid: Pid) -> Option<&'a PropertyValue> {
        let id = pid.id()?;
        self.data.iter().find(|e| e.pid == id).map(|e| &e.value)
    }
}

/// The effect structs of list `kind` in `set`, in stored order.
#[must_use]
pub fn effect_list<'a, A: PropertyAccess + ?Sized>(
    set: &'a A,
    kind: EffectListKind,
    pids: &EffectStructPids,
) -> Vec<EffectRef<'a>> {
    set.structs(kind.pid(pids))
        .map(|s| EffectRef::from_struct(s, pids))
        .collect()
}

/// One `Item_Socket_Array_Entry` (§5.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Socket {
    /// `Item_Socket_Type` bit field (mapper `ItemSocketType`).
    pub socket_type: Option<u64>,
    /// `Item_Socket_Unlock_ILevel`.
    pub unlock_item_level: Option<i32>,
    /// Number of fields in the struct.
    pub field_count: usize,
}

impl Socket {
    /// Whether the entry has exactly its two fields and one socket-type bit (IT10).
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.field_count == 2
            && self.unlock_item_level.is_some()
            && self.socket_type.is_some_and(|t| t.count_ones() == 1)
    }
}

/// The keys named by an equipment-category bit field: bit *i* is key *i* + 1
/// (`items.md` §5.1, `property-system.md` §7.3).
pub fn equipment_category_keys(bits: u64) -> impl Iterator<Item = u32> {
    (0u32..64)
        .filter(move |i| bits >> i & 1 == 1)
        .map(|i| i + 1)
}

/// Weapon properties (§5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponStats {
    /// `Combat_DPS_LUT` (null included).
    pub dps_table: Option<Did>,
    /// `Combat_BaseDPS`.
    pub base_dps: Option<f32>,
    /// `Combat_Damage`: the maximum damage per hit (Reported).
    pub damage: Option<f32>,
    /// `Combat_DamageVariance`.
    pub variance: Option<f32>,
    /// `Combat_DamageType` key.
    pub damage_type: Option<u32>,
    /// `Combat_WeaponSpeed` key.
    pub speed: Option<u32>,
    /// `Item_BaseActionDuration`.
    pub action_duration: Option<f32>,
    /// `Item_BaseAnimDurationMultiplierMod`.
    pub anim_duration_mod: Option<f32>,
    /// The level the DPS lookup uses: `ItemAdvancement_CombatDPSLevel` if present, else
    /// `Item_Level` (§9.3).
    pub dps_level: Option<i32>,
}

/// Durability and wear (§5.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Durability {
    /// `Item_DurabilityEnum` key.
    pub durability_key: Option<u32>,
    /// `Item_DurabilityValue`.
    pub durability_value: Option<f32>,
    /// `Item_MaxStructurePointsEnum` key.
    pub max_structure_points_key: Option<u32>,
    /// `Item_MaxStructurePoints`.
    pub max_structure_points: Option<i32>,
    /// `Item_CurStructurePoints`.
    pub cur_structure_points: Option<i32>,
    /// `Item_RepairCostPerPointProgression` (evaluation OPEN).
    pub repair_cost_progression: Option<Did>,
}

/// Behaviour objects named by an item (§5.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Behaviours {
    /// `Usage_Action` (null included).
    pub action: Option<Did>,
    /// `Usage_Permission`.
    pub permission: Option<Did>,
    /// `Usage_TakePermission`.
    pub take_permission: Option<Did>,
    /// `CSM_ActionHandler`.
    pub action_handler: Option<Did>,
}

/// Inventory slots (§5.4): `ContainerSlot` bit fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Slots {
    /// `Inventory_CompatibleSlot`: slots the item may occupy.
    pub compatible: Option<u64>,
    /// `Inventory_DefaultSlot`: the slot it goes to by default.
    pub default: Option<u64>,
    /// `Inventory_PrecludedSlot`: slots blocked while equipped (Reported).
    pub precluded: Option<u64>,
}

/// Level-scaling ("munging") fields, raw (§5.2; the scaling rule is OPEN).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Munging {
    /// `ItemMunging_MinMungeLevel`.
    pub min_level: Option<i32>,
    /// `ItemMunging_MaxMungeLevel`.
    pub max_level: Option<i32>,
    /// `ItemMunging_ItemLevelOverrideProgression` (null included).
    pub override_progression: Option<Did>,
    /// `ItemMunging_ItemLevelOverrideProperty`: the driving property's PID.
    pub override_property: Option<u32>,
    /// `Item_iLevel_Crafting_Offset` (meaning OPEN).
    pub crafting_offset: Option<i32>,
    /// `Item_iLevel_Bonus_Limit`.
    pub bonus_limit: Option<i32>,
    /// `Item_iLevel_Bonus_Chance`.
    pub bonus_chance: Option<f32>,
}

impl Munging {
    /// Whether no field is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Fields of a socketable (essence or tracery, §5.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Socketable {
    /// Top-level `Item_Socket_Type`: the socket that accepts it.
    pub socket_type: Option<u64>,
    /// `Item_Socket_GemMinLevel` (constraint OPEN).
    pub gem_min_level: Option<i32>,
    /// `Item_Socket_GemMaxLevel` (constraint OPEN).
    pub gem_max_level: Option<i32>,
    /// `Item_Socket_LevelupRuneIncrement`, when an integer.
    pub levelup_increment: Option<i64>,
    /// `Item_Socket_Operation` (null included).
    pub operation: Option<Did>,
    /// `Item_UniquenessChannel` key.
    pub uniqueness_channel: Option<u32>,
    /// `Item_PropertySet`: the set it belongs to (§7.1), null included.
    pub item_set: Option<Did>,
}

/// Disenchanting fields (§5.3; meanings OPEN or label readings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Disenchant {
    /// `Item_Disenchant_Value` (−1 versus 0 is OPEN).
    pub value: Option<i32>,
    /// `Item_Disenchant_Component_Result`: an item (null included).
    pub component: Option<Did>,
    /// `Item_Disenchant_Trophy_List`: a loot list (null included).
    pub trophy_list: Option<Did>,
}

/// `Usage_RequiredFaction` (§5.10): faction record and tier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactionRequirement {
    /// `Usage_RequiredFaction_DataID` (null included).
    pub faction: Option<Did>,
    /// `Usage_RequiredFaction_Tier` key.
    pub tier: Option<u32>,
}

/// One `Usage_RequiredTrait_Array` element (§5.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraitRequirement {
    /// `Usage_RequiredTrait` (null included).
    pub required_trait: Option<Did>,
    /// `Usage_RequiredVirtueRank`, an integer or enum key (the spec does not give the
    /// type).
    pub virtue_rank: Option<i64>,
}

/// Requirements and limits of item usage (§5.10), as stored. Their semantics are
/// Reported (LC); nothing here evaluates them.
#[derive(Debug, Clone, PartialEq)]
pub struct Requirements<'a> {
    /// `Usage_MinLevel` as stored (dictionary default: [`ItemView::min_level_or_default`]).
    pub min_level: Option<i32>,
    /// `Usage_MaxLevel`.
    pub max_level: Option<i32>,
    /// `Usage_RequiredClassList`: `Usage_ClassType` keys in stored order.
    pub classes: Vec<u32>,
    /// `Usage_RequiredRaces` elements, raw (the entry fields are not specified).
    pub races: &'a [PropertyEntry],
    /// `Usage_RequiredFaction`.
    pub faction: Option<FactionRequirement>,
    /// `Usage_RequiredTrait`.
    pub required_trait: Option<Did>,
    /// `Usage_RequiredTrait_Array` elements.
    pub required_traits: Vec<TraitRequirement>,
    /// `Usage_BarringTrait`.
    pub barring_trait: Option<Did>,
    /// `Usage_BarringSkill`.
    pub barring_skill: Option<Did>,
    /// `Usage_BarringCraftRecipe`.
    pub barring_recipe: Option<Did>,
    /// `Usage_BarringEmote`.
    pub barring_emote: Option<Did>,
    /// `Usage_MinGloryRank`.
    pub min_glory_rank: Option<i32>,
    /// `Usage_RequiredCraftProfession`.
    pub craft_profession: Option<Did>,
    /// `Usage_RequiredCraftTier` key.
    pub craft_tier: Option<u32>,
    /// `Usage_RequiredCraftMastery` key.
    pub craft_mastery: Option<u32>,
    /// `Usage_QuestRequirements` entries, empty slots included (`quests.md` §8.2).
    pub quests: Vec<QuestRequirement>,
    /// `Usage_RestrictedEffect`.
    pub restricted_effect: Option<Did>,
    /// `Usage_RequiredEffect`.
    pub required_effect: Option<Did>,
}

/// An integer or an enum key, widened.
fn number(v: &PropertyValue) -> Option<i64> {
    v.integer().or_else(|| v.enum_key().map(i64::from))
}

/// Typed, label-aware view of an item's property set (§5). Every accessor returns `None`
/// when the property is absent, undefined in the dictionary, or of another type; the
/// `*_or_default` accessors and [`ItemView::binding`] apply the dictionary default of an
/// absent property instead (§14 decision).
#[derive(Debug, Clone, Copy)]
pub struct ItemView<'a> {
    set: &'a PropertySet,
    schema: &'a ItemSchema,
}

impl<'a> ItemView<'a> {
    /// A view of `set`.
    #[must_use]
    pub fn new(set: &'a PropertySet, schema: &'a ItemSchema) -> Self {
        Self { set, schema }
    }

    /// The underlying property set.
    #[must_use]
    pub fn set(&self) -> &'a PropertySet {
        self.set
    }

    /// The item PID table.
    #[must_use]
    pub fn pids(&self) -> &'a ItemPids {
        &self.schema.item
    }

    /// The stored value of `pid`, else its dictionary default (§14 decision;
    /// `property-system.md` §2.2).
    #[must_use]
    pub fn value_or_default<D: PropertyDefaults + ?Sized>(
        &self,
        pid: Pid,
        defaults: &'a D,
    ) -> Option<&'a PropertyValue> {
        self.set.value_or_default(pid, defaults)
    }

    /// Whether the set carries the properties every item has (invariant IT1): `Item_Class`,
    /// `WeenieType`, `Name`, `PhysObj`, `Usage_Action`, `Icon_Layer_BackgroundDID`.
    #[must_use]
    pub fn has_item_signature(&self) -> bool {
        let p = self.pids();
        [
            p.item_class,
            p.weenie_type,
            p.name,
            p.phys_obj,
            p.usage_action,
            p.icon_background,
        ]
        .iter()
        .all(|&pid| self.set.has(pid))
    }

    /// The descriptive kind (§2.1, §2.4 rule 4; rule order of [`ItemKind`]): signatures
    /// first, then the `WeenieType` label through `weenie`, then the worn-look list.
    #[must_use]
    pub fn kind(&self, weenie: &ItemWeenieKinds) -> ItemKind {
        let p = self.pids();
        let s = self.set;
        if [
            p.dps_table,
            p.base_dps,
            p.damage,
            p.damage_variance,
            p.damage_type,
            p.weapon_speed,
        ]
        .iter()
        .all(|&pid| s.has(pid))
        {
            ItemKind::Weapon
        } else if s.has(p.recipe) && s.has(p.barring_recipe) {
            ItemKind::RecipeScroll
        } else if self.is_multi_recipe() {
            ItemKind::MultiRecipeScroll
        } else if s.has(p.package_trophy_list) {
            ItemKind::Package
        } else if s.has(p.text_title) && s.has(p.text_content) {
            ItemKind::TextItem
        } else if s.has(p.players_can_open) || s.has(p.players_can_close) {
            ItemKind::Door
        } else if s.has(p.key_ring) {
            ItemKind::Key
        } else if let Some(k) = self.weenie_type().and_then(|k| weenie.kind(k)) {
            k
        } else if s.has(p.worn_appearance_list) {
            ItemKind::Wearable
        } else {
            ItemKind::General
        }
    }

    /// [`ItemView::kind`] from the signatures alone, without `WeenieType` labels.
    #[must_use]
    pub fn signature_kind(&self) -> ItemKind {
        self.kind(&ItemWeenieKinds::default())
    }

    fn is_multi_recipe(&self) -> bool {
        self.effects(EffectListKind::Usage).iter().any(|e| {
            e.argument(self.schema.effects.crafting_recipe_list)
                .is_some()
        })
    }

    // §5.1 identity and display

    /// `Name`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids().name)
    }
    /// `PluralName`.
    #[must_use]
    pub fn plural_name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids().plural_name)
    }
    /// `Description`.
    #[must_use]
    pub fn description(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids().description)
    }
    /// `Item_Class` key.
    #[must_use]
    pub fn item_class(&self) -> Option<u32> {
        self.set.enum_key(self.pids().item_class)
    }
    /// `Item_Quality` key.
    #[must_use]
    pub fn quality(&self) -> Option<u32> {
        self.set.enum_key(self.pids().quality)
    }
    /// `Item_EquipmentCategory` bit field ([`equipment_category_keys`] lists its keys).
    #[must_use]
    pub fn equipment_category(&self) -> Option<u64> {
        self.set.bitfield(self.pids().equipment_category)
    }
    /// `WeenieType` key.
    #[must_use]
    pub fn weenie_type(&self) -> Option<u32> {
        self.set.enum_key(self.pids().weenie_type)
    }
    /// `Auction_CategoryType` key.
    #[must_use]
    pub fn auction_category(&self) -> Option<u32> {
        self.set.enum_key(self.pids().auction_category)
    }
    /// `Item_Client_Sort_Type` key.
    #[must_use]
    pub fn sort_type(&self) -> Option<u32> {
        self.set.enum_key(self.pids().sort_type)
    }
    /// `Item_MaterialType` key (use OPEN).
    #[must_use]
    pub fn material_type(&self) -> Option<u32> {
        self.set.enum_key(self.pids().material_type)
    }
    /// `PhysObj`.
    #[must_use]
    pub fn phys_obj(&self) -> Option<Did> {
        self.set.did(self.pids().phys_obj)
    }
    /// `ExternalAppearanceID` (meaning OPEN), null included.
    #[must_use]
    pub fn external_appearance(&self) -> Option<Did> {
        self.set.did(self.pids().external_appearance)
    }
    /// The `ScriptTable_DBScriptTable` DIDs of `Entity_AdditionalScriptTables`, null DIDs
    /// skipped.
    #[must_use]
    pub fn script_tables(&self) -> Vec<Did> {
        self.set.dids(self.pids().script_tables).collect()
    }

    // §5.2 level and scaling

    /// `Item_Level` as stored.
    #[must_use]
    pub fn level(&self) -> Option<i32> {
        self.set.int(self.pids().level)
    }
    /// `Item_Level`, else its dictionary default.
    #[must_use]
    pub fn level_or_default<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<i32> {
        self.set
            .value_or_default(self.pids().level, defaults)?
            .int()
    }
    /// `Usage_MinLevel` as stored.
    #[must_use]
    pub fn min_level(&self) -> Option<i32> {
        self.set.int(self.pids().min_level)
    }
    /// `Usage_MinLevel`, else its dictionary default.
    #[must_use]
    pub fn min_level_or_default<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<i32> {
        self.set
            .value_or_default(self.pids().min_level, defaults)?
            .int()
    }
    /// `Usage_MaxLevel`.
    #[must_use]
    pub fn max_level(&self) -> Option<i32> {
        self.set.int(self.pids().max_level)
    }
    /// `Item_DistributionType` key.
    #[must_use]
    pub fn distribution(&self) -> Option<u32> {
        self.set.enum_key(self.pids().distribution)
    }
    /// The level-scaling fields, raw.
    #[must_use]
    pub fn munging(&self) -> Munging {
        let p = self.pids();
        Munging {
            min_level: self.set.int(p.munge_min_level),
            max_level: self.set.int(p.munge_max_level),
            override_progression: self.set.did(p.munge_override_progression),
            override_property: self.set.property_ref(p.munge_override_property),
            crafting_offset: self.set.int(p.ilevel_crafting_offset),
            bonus_limit: self.set.int(p.ilevel_bonus_limit),
            bonus_chance: self.set.float(p.ilevel_bonus_chance),
        }
    }

    // §5.3 economy

    /// `Item_Value` (copper, Reported).
    #[must_use]
    pub fn value(&self) -> Option<i32> {
        self.set.int(self.pids().value)
    }
    /// `Item_ValueLookupTable`, null DID included (null means "no table", §9.2 step 1).
    #[must_use]
    pub fn value_table(&self) -> Option<Did> {
        self.set.did(self.pids().value_table)
    }
    /// `Barter_IsBarterItem` (meaning OPEN).
    #[must_use]
    pub fn is_barter_item(&self) -> Option<bool> {
        self.set.boolean(self.pids().barter_item)
    }
    /// `Item_ForceNotSellable`.
    #[must_use]
    pub fn force_not_sellable(&self) -> Option<bool> {
        self.set.boolean(self.pids().force_not_sellable)
    }
    /// The disenchanting fields.
    #[must_use]
    pub fn disenchant(&self) -> Disenchant {
        let p = self.pids();
        Disenchant {
            value: self.set.int(p.disenchant_value),
            component: self.set.did(p.disenchant_component),
            trophy_list: self.set.did(p.disenchant_trophy_list),
        }
    }
    /// `Item_WearState` key.
    #[must_use]
    pub fn wear_state(&self) -> Option<u32> {
        self.set.enum_key(self.pids().wear_state)
    }

    // §5.4 inventory

    /// `Inventory_MaxQuantity`.
    #[must_use]
    pub fn max_quantity(&self) -> Option<i32> {
        self.set.int(self.pids().max_quantity)
    }
    /// `Inventory_Unique`.
    #[must_use]
    pub fn unique(&self) -> Option<bool> {
        self.set.boolean(self.pids().unique)
    }
    /// `Inventory_UniqueEquipped` (meaning OPEN).
    #[must_use]
    pub fn unique_equipped(&self) -> Option<bool> {
        self.set.boolean(self.pids().unique_equipped)
    }
    /// `Item_UniquenessChannel` key.
    #[must_use]
    pub fn uniqueness_channel(&self) -> Option<u32> {
        self.set.enum_key(self.pids().uniqueness_channel)
    }
    /// The three slot bit fields.
    #[must_use]
    pub fn slots(&self) -> Slots {
        let p = self.pids();
        Slots {
            compatible: self.set.bitfield(p.compatible_slot),
            default: self.set.bitfield(p.default_slot),
            precluded: self.set.bitfield(p.precluded_slot),
        }
    }
    /// The three binding flags as stored.
    #[must_use]
    pub fn binding_flags(&self) -> BindingFlags {
        let p = self.pids();
        BindingFlags {
            on_acquire: self.set.boolean(p.bind_on_acquire),
            to_account: self.set.boolean(p.bind_to_account),
            on_equip: self.set.boolean(p.bind_on_equip),
        }
    }
    /// The three binding flags, each absent flag replaced by its dictionary default (§14
    /// decision). A flag stays `None` only when it is absent and has no boolean default.
    #[must_use]
    pub fn binding_flags_or_default<D: PropertyDefaults + ?Sized>(
        &self,
        defaults: &D,
    ) -> BindingFlags {
        let p = self.pids();
        let flag = |pid| self.set.value_or_default(pid, defaults)?.boolean();
        BindingFlags {
            on_acquire: flag(p.bind_on_acquire),
            to_account: flag(p.bind_to_account),
            on_equip: flag(p.bind_on_equip),
        }
    }
    /// The binding reading (§5.4) of [`ItemView::binding_flags_or_default`].
    #[must_use]
    pub fn binding<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Binding {
        self.binding_flags_or_default(defaults).binding()
    }

    // §5.5 durability

    /// Durability and wear values.
    #[must_use]
    pub fn durability(&self) -> Durability {
        let p = self.pids();
        Durability {
            durability_key: self.set.enum_key(p.durability_enum),
            durability_value: self.set.float(p.durability_value),
            max_structure_points_key: self.set.enum_key(p.max_structure_points_enum),
            max_structure_points: self.set.int(p.max_structure_points),
            cur_structure_points: self.set.int(p.cur_structure_points),
            repair_cost_progression: self.set.did(p.repair_cost_progression),
        }
    }

    // §5.6 weapons

    /// Weapon values; `None` unless the set has `Combat_DPS_LUT`.
    #[must_use]
    pub fn weapon(&self) -> Option<WeaponStats> {
        let p = self.pids();
        let dps_table = self.set.did(p.dps_table)?;
        Some(WeaponStats {
            dps_table: Some(dps_table),
            base_dps: self.set.float(p.base_dps),
            damage: self.set.float(p.damage),
            variance: self.set.float(p.damage_variance),
            damage_type: self.set.enum_key(p.damage_type),
            speed: self.set.enum_key(p.weapon_speed),
            action_duration: self.set.float(p.base_action_duration),
            anim_duration_mod: self.set.float(p.anim_duration_mod),
            dps_level: self.set.int(p.dps_level).or_else(|| self.level()),
        })
    }
    /// `Combat_WeaponSlayerGenus` and `Combat_WeaponSlayerAddMod` (Reported).
    #[must_use]
    pub fn slayer(&self) -> (Option<u64>, Option<f32>) {
        let p = self.pids();
        (
            self.set.bitfield(p.slayer_genus),
            self.set.float(p.slayer_add_mod),
        )
    }

    // §5.7 armour

    /// `Item_Armor_Value`.
    #[must_use]
    pub fn armour_value(&self) -> Option<i32> {
        self.set.int(self.pids().armour_value)
    }
    /// `Item_Armor_Value_Lookup_Table`, null included.
    #[must_use]
    pub fn armour_table(&self) -> Option<Did> {
        self.set.did(self.pids().armour_table)
    }

    // §5.8, §5.9

    /// The `Mod_Array` entries, read as `combat-data.md` §5.1 describes (empty and
    /// field-less entries included; [`ModEntry::is_skipped`] tells them apart).
    #[must_use]
    pub fn mods(&self) -> Vec<ModEntry> {
        mod_entries(self.set, &self.schema.modifiers)
    }
    /// The effect structs of list `kind`.
    #[must_use]
    pub fn effects(&self, kind: EffectListKind) -> Vec<EffectRef<'a>> {
        effect_list(self.set, kind, &self.schema.effects)
    }
    /// `Item_GrantedSkill` (null included).
    #[must_use]
    pub fn granted_skill(&self) -> Option<Did> {
        self.set.did(self.pids().granted_skill)
    }
    /// `Item_GrantedTrait` (null included).
    #[must_use]
    pub fn granted_trait(&self) -> Option<Did> {
        self.set.did(self.pids().granted_trait)
    }

    // §5.10

    /// The four behaviour objects.
    #[must_use]
    pub fn behaviours(&self) -> Behaviours {
        let p = self.pids();
        Behaviours {
            action: self.set.did(p.usage_action),
            permission: self.set.did(p.usage_permission),
            take_permission: self.set.did(p.take_permission),
            action_handler: self.set.did(p.action_handler),
        }
    }
    /// The requirements and limits, as stored.
    #[must_use]
    pub fn requirements(&self) -> Requirements<'a> {
        let r = &self.schema.requirements;
        let s = self.set;
        Requirements {
            min_level: self.min_level(),
            max_level: self.max_level(),
            classes: s
                .elements(r.required_classes)
                .iter()
                .filter_map(ValueExt::enum_key)
                .collect(),
            races: s.elements(r.required_races),
            faction: s.structure(r.required_faction).map(|f| FactionRequirement {
                faction: f.did(r.required_faction_did),
                tier: f.enum_key(r.required_faction_tier),
            }),
            required_trait: s.did(r.required_trait),
            required_traits: s
                .structs(r.required_trait_array)
                .map(|t| TraitRequirement {
                    required_trait: t.did(r.required_trait),
                    virtue_rank: t.get_pid(r.required_virtue_rank).and_then(number),
                })
                .collect(),
            barring_trait: s.did(r.barring_trait),
            barring_skill: s.did(r.barring_skill),
            barring_recipe: s.did(self.pids().barring_recipe),
            barring_emote: s.did(r.barring_emote),
            min_glory_rank: s.int(r.min_glory_rank),
            craft_profession: s.did(r.required_craft_profession),
            craft_tier: s.enum_key(r.required_craft_tier),
            craft_mastery: s.enum_key(r.required_craft_mastery),
            quests: s
                .structs(r.quest_requirements)
                .map(|q| QuestRequirement {
                    operator: q.enum_key(r.quest_operator).unwrap_or(0),
                    quest: q.did(r.quest_id),
                    status: q.enum_key(r.quest_status).map(QuestStatus),
                })
                .collect(),
            restricted_effect: s.did(r.restricted_effect),
            required_effect: s.did(r.required_effect),
        }
    }
    /// `Usage_CooldownDuration` key.
    #[must_use]
    pub fn cooldown(&self) -> Option<u32> {
        self.set.enum_key(self.pids().cooldown)
    }
    /// `Usage_CooldownChannel` key: the shared-cooldown group.
    #[must_use]
    pub fn cooldown_channel(&self) -> Option<u32> {
        self.set.enum_key(self.pids().cooldown_channel)
    }
    /// `Usage_EquipCooldownDuration` key (0 = none).
    #[must_use]
    pub fn equip_cooldown(&self) -> Option<u32> {
        self.set.enum_key(self.pids().equip_cooldown)
    }
    /// `ItemDecay_Duration` key.
    #[must_use]
    pub fn decay(&self) -> Option<u32> {
        self.set.enum_key(self.pids().decay)
    }
    /// `Usage_DestroyOnUse`, else its dictionary default.
    #[must_use]
    pub fn destroy_on_use<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<bool> {
        self.set
            .value_or_default(self.pids().destroy_on_use, defaults)?
            .boolean()
    }
    /// `Usage_AllowedInCombat`, else its dictionary default.
    #[must_use]
    pub fn allowed_in_combat<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<bool> {
        self.set
            .value_or_default(self.pids().allowed_in_combat, defaults)?
            .boolean()
    }
    /// `Usage_AllowedWhileMounted`, else its dictionary default.
    #[must_use]
    pub fn allowed_while_mounted<D: PropertyDefaults + ?Sized>(
        &self,
        defaults: &D,
    ) -> Option<bool> {
        self.set
            .value_or_default(self.pids().allowed_while_mounted, defaults)?
            .boolean()
    }
    /// `Usage_MinUsageDist`, else its dictionary default.
    #[must_use]
    pub fn min_usage_distance<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<f32> {
        self.set
            .value_or_default(self.pids().min_usage_dist, defaults)?
            .float()
    }

    // §5.11

    /// The `Item_Socket_Array` entries, in stored order.
    #[must_use]
    pub fn sockets(&self) -> Vec<Socket> {
        let p = self.pids();
        self.set
            .structs(p.socket_array)
            .map(|s| Socket {
                socket_type: s.bitfield(p.socket_type),
                unlock_item_level: s.int(p.socket_unlock_level),
                field_count: s.fields.len(),
            })
            .collect()
    }
    /// A socketable's top-level `Item_Socket_Type`: the socket that accepts it.
    #[must_use]
    pub fn accepted_socket_type(&self) -> Option<u64> {
        self.set.bitfield(self.pids().socket_type)
    }
    /// The socketable fields (§5.11).
    #[must_use]
    pub fn socketable(&self) -> Socketable {
        let p = self.pids();
        Socketable {
            socket_type: self.accepted_socket_type(),
            gem_min_level: self.set.int(p.gem_min_level),
            gem_max_level: self.set.int(p.gem_max_level),
            levelup_increment: self
                .set
                .get_pid(p.socket_levelup_increment)
                .and_then(ValueExt::integer),
            operation: self.set.did(p.socket_operation),
            uniqueness_channel: self.uniqueness_channel(),
            item_set: self.item_set(),
        }
    }
    /// `Item_PropertySet`: the set a socketable belongs to (§7.1), null included.
    #[must_use]
    pub fn item_set(&self) -> Option<Did> {
        self.set.did(self.pids().property_set)
    }

    // §5.12, §5.13

    /// Whether `ItemAdvancement_Item` is true (an advancing item).
    #[must_use]
    pub fn is_advancing(&self) -> bool {
        self.set.boolean(self.pids().advancement_item) == Some(true)
    }
    /// `ItemAdvancement_ProgressionGroupOverride` (null included).
    #[must_use]
    pub fn progression_group(&self) -> Option<Did> {
        self.set.did(self.pids().progression_group)
    }
    /// `ItemAdvancement_Title` (null included).
    #[must_use]
    pub fn advancement_title(&self) -> Option<Did> {
        self.set.did(self.pids().advancement_title)
    }
    /// `ItemAdvancement_RunicToCreate` (null included).
    #[must_use]
    pub fn runic_to_create(&self) -> Option<Did> {
        self.set.did(self.pids().runic_to_create)
    }
    /// `Item_MaxLevelUpgrades`.
    #[must_use]
    pub fn max_level_upgrades(&self) -> Option<i32> {
        self.set.int(self.pids().max_level_upgrades)
    }
    /// `RecipeItem_Recipe`.
    #[must_use]
    pub fn recipe(&self) -> Option<Did> {
        self.set.did(self.pids().recipe)
    }
}

// ---------------------------------------------------------------------------------------------
// Tables (§6, §9)
// ---------------------------------------------------------------------------------------------

/// Rounds to the nearest integer with halves rounded up (§9.2 step 5). `None` for NaN,
/// infinities and results outside `i64`.
#[must_use]
pub fn round_half_up(x: f32) -> Option<i64> {
    if !x.is_finite() {
        return None;
    }
    // f32 → f64 is exact and adding 0.5 to it is exact, so the floor rounds the f32 value.
    let r = (f64::from(x) + 0.5).floor();
    // `as` saturates; the range check rejects saturated results.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    ((-9.0e18..9.0e18).contains(&r)).then_some(r as i64)
}

/// Truncates toward zero (§9.5 step 3). `None` for NaN, infinities and results outside
/// `i64`.
#[must_use]
pub fn truncate(x: f32) -> Option<i64> {
    if !x.is_finite() {
        return None;
    }
    let t = f64::from(x).trunc();
    #[allow(clippy::cast_possible_truncation)]
    ((-9.0e18..9.0e18).contains(&t)).then_some(t as i64)
}

/// The (key, factor) element of a quality table whose key is `key`.
fn keyed_lookup<T: Copy>(rows: &[(u32, T)], key: u32) -> Option<T> {
    rows.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
}

/// Element `level` (1-based) of `values`.
fn one_based<T: Copy>(values: &[T], level: i32) -> Option<T> {
    let i = usize::try_from(level).ok()?.checked_sub(1)?;
    values.get(i).copied()
}

/// Reads a (key, factor) struct array: key from `key_pid` (enum), factor from `value_pid`
/// (float). Elements that are not structs or lack either field are errors.
fn quality_rows<A: PropertyAccess + ?Sized>(
    set: &A,
    array: Pid,
    key_pid: Pid,
    value_pid: Pid,
    what: &'static str,
) -> Result<Vec<(u32, f32)>> {
    let arr = set.require_array(array, what)?;
    arr.elements
        .iter()
        .map(|e| {
            let s = e
                .value
                .as_struct()
                .ok_or_else(|| malformed(what, "element is not a struct"))?;
            let k = s.require_enum(key_pid, what)?;
            let v = s.require_float(value_pid, what)?;
            Ok((k, v))
        })
        .collect()
}

fn malformed(what: &'static str, reason: &'static str) -> FormatError {
    FormatError::Malformed {
        what,
        offset: 0,
        reason: reason.into(),
    }
}

/// A value table (class 1688 here, §6.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ValueTable {
    /// `Item_BaseValArray`: element *k* (1-based) is the base value at item level *k*.
    pub base_values: Vec<f32>,
    /// `Item_QualityArray`: (quality key, factor).
    pub quality_factors: Vec<(u32, f32)>,
}

impl ValueTable {
    const WHAT: &'static str = "value table";

    /// Reads a value table's property set.
    ///
    /// Fails with [`FormatError::MissingProperty`] / [`FormatError::PropertyTypeMismatch`]
    /// when an array or field is absent or mistyped, and [`FormatError::Malformed`] when an
    /// element is not a struct.
    pub fn from_set<A: PropertyAccess + ?Sized>(set: &A, pids: &ItemTablePids) -> Result<Self> {
        let arr = set.require_array(pids.base_value_array, Self::WHAT)?;
        let base_values =
            arr.elements
                .iter()
                .map(|e| {
                    let s = e.value.as_struct().ok_or_else(|| {
                        malformed(Self::WHAT, "base value element is not a struct")
                    })?;
                    s.require_float(pids.base_value, Self::WHAT)
                })
                .collect::<Result<Vec<_>>>()?;
        let quality_factors = quality_rows(
            set,
            pids.quality_array,
            pids.quality,
            pids.quality_mod,
            Self::WHAT,
        )?;
        Ok(Self {
            base_values,
            quality_factors,
        })
    }

    /// The base value at `level` (1-based); `None` outside the array.
    #[must_use]
    pub fn base_value(&self, level: i32) -> Option<f32> {
        one_based(&self.base_values, level)
    }

    /// The factor of quality key `quality`.
    #[must_use]
    pub fn quality_factor(&self, quality: u32) -> Option<f32> {
        keyed_lookup(&self.quality_factors, quality)
    }

    /// §9.2 steps 2–5: base value × quality factor in single precision, rounded half up.
    /// `None` when the level is outside the array or the quality has no factor.
    #[must_use]
    pub fn value(&self, level: i32, quality: u32) -> Option<i64> {
        let b = self.base_value(level)?;
        let m = self.quality_factor(quality)?;
        round_half_up(b * m)
    }
}

/// The item value of §9.2: 0 without a table (`table` is `None` for an absent or null
/// `Item_ValueLookupTable`), else [`ValueTable::value`].
#[must_use]
pub fn item_value(table: Option<&ValueTable>, level: i32, quality: u32) -> Option<i64> {
    match table {
        None => Some(0),
        Some(t) => t.value(level, quality),
    }
}

/// A DPS table (class 1660 here, §6.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DpsTable {
    /// `Combat_BaseDPSArray`: element *k* (1-based) is the base DPS at level *k*.
    pub base_dps: Vec<f32>,
    /// `Combat_QualityModArray`: (quality key, factor).
    pub quality_factors: Vec<(u32, f32)>,
}

impl DpsTable {
    const WHAT: &'static str = "DPS table";

    /// Reads a DPS table's property set. Errors as [`ValueTable::from_set`]; a base-DPS
    /// element that is not a float is [`FormatError::Malformed`].
    pub fn from_set<A: PropertyAccess + ?Sized>(set: &A, pids: &ItemTablePids) -> Result<Self> {
        let arr = set.require_array(pids.dps_array, Self::WHAT)?;
        let base_dps = arr
            .elements
            .iter()
            .map(|e| {
                e.value
                    .float()
                    .ok_or_else(|| malformed(Self::WHAT, "base DPS element is not a float"))
            })
            .collect::<Result<Vec<_>>>()?;
        let quality_factors = quality_rows(
            set,
            pids.dps_quality_array,
            pids.dps_quality,
            pids.dps_mod,
            Self::WHAT,
        )?;
        Ok(Self {
            base_dps,
            quality_factors,
        })
    }

    /// The base DPS at `level` (1-based).
    #[must_use]
    pub fn base_dps(&self, level: i32) -> Option<f32> {
        one_based(&self.base_dps, level)
    }

    /// The factor of quality key `quality`.
    #[must_use]
    pub fn quality_factor(&self, quality: u32) -> Option<f32> {
        keyed_lookup(&self.quality_factors, quality)
    }

    /// §9.3: base DPS at `level` × quality factor, a single-precision product. `level` is
    /// [`WeaponStats::dps_level`].
    #[must_use]
    pub fn dps(&self, level: i32, quality: u32) -> Option<f32> {
        Some(self.base_dps(level)? * self.quality_factor(quality)?)
    }
}

/// §9.4: the stored `Combat_Damage` = DPS × duration × 2 ÷ (2 − variance), in single
/// precision. `None` when the result is not finite.
#[must_use]
pub fn weapon_damage(dps: f32, duration: f32, variance: f32) -> Option<f32> {
    let d = dps * duration * 2.0 / (2.0 - variance);
    d.is_finite().then_some(d)
}

/// The damage range per hit as (minimum, maximum), with maximum = `Combat_Damage` and
/// minimum = maximum × (1 − variance) (§9.4; the reading is Reported).
#[must_use]
pub fn damage_range(damage: f32, variance: f32) -> (f32, f32) {
    (damage * (1.0 - variance), damage)
}

/// §9.5: the armour progression (`Item_Armor_Value_Lookup_Table`, read with
/// [`Progression::from_properties`] and `nested` = false) evaluated at `level` as
/// `combat-data.md` §4.10 describes (Decision D1 clamp, Decision D3 stored value at a key,
/// single-precision stored-slope form), then truncated toward zero. `None` when the
/// progression gives no number (no points, keys that do not strictly increase, or a value
/// that is not numeric) or the result is not finite.
#[must_use]
pub fn armour_value(progression: &Progression, level: i32) -> Option<i64> {
    truncate(progression.value_at(i64::from(level))?.as_f32()?)
}

/// Reads a (key, number) struct array whose field labels the spec does not give: the key is
/// the first enum field and the number the first float (or int, converted) field of each
/// struct. Elements without both are skipped (logged).
fn generic_keyed_rows(elements: &[PropertyEntry]) -> Vec<(u32, f32)> {
    let mut out = Vec::new();
    for e in elements {
        let Some(s) = e.value.as_struct() else {
            tracing::debug!("keyed table element is not a struct; skipped");
            continue;
        };
        let key = s.fields.iter().find_map(|f| f.value.enum_key());
        #[allow(clippy::cast_precision_loss)]
        let value = s.fields.iter().find_map(|f| match f.value {
            PropertyValue::Float(v) => Some(v),
            PropertyValue::Int(v) => Some(v as f32),
            _ => None,
        });
        match (key, value) {
            (Some(k), Some(v)) => out.push((k, v)),
            _ => tracing::debug!("keyed table element without an enum key and a number; skipped"),
        }
    }
    out
}

/// The wear control (§6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WearControl {
    /// `ItemWearControl_MaxStructurePointsArray`: (enum key, structure points).
    pub max_structure_points: Vec<(u32, i32)>,
    /// `ItemWearControl_DurabilityArray`: (enum key, durability value).
    pub durability: Vec<(u32, f32)>,
    /// `ItemWearControl_RepairCostQualityModArray`: (quality key, factor); role OPEN.
    pub repair_cost_quality: Vec<(u32, f32)>,
    /// `ItemWearControl_ItemWearStateValueModArray`: (wear-state key, factor); role OPEN.
    pub wear_state_value: Vec<(u32, f32)>,
    /// `ItemWearControl_SkillGateArray`: (key, number); role OPEN.
    pub skill_gate: Vec<(u32, f32)>,
}

impl WearControl {
    const WHAT: &'static str = "wear control";

    /// Reads the wear control's property set. The structure-point and durability arrays
    /// are required and read by label; the other three are read by field type (their field
    /// labels are not specified) and are empty when absent.
    pub fn from_set<A: PropertyAccess + ?Sized>(set: &A, pids: &ItemTablePids) -> Result<Self> {
        let msp = set.require_array(pids.wear_msp_array, Self::WHAT)?;
        let max_structure_points = msp
            .elements
            .iter()
            .map(|e| {
                let s = e
                    .value
                    .as_struct()
                    .ok_or_else(|| malformed(Self::WHAT, "element is not a struct"))?;
                Ok((
                    s.require_enum(pids.wear_msp_enum, Self::WHAT)?,
                    s.require_int(pids.wear_msp_value, Self::WHAT)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let durability = quality_rows(
            set,
            pids.wear_durability_array,
            pids.wear_durability_enum,
            pids.wear_durability_value,
            Self::WHAT,
        )?;
        Ok(Self {
            max_structure_points,
            durability,
            repair_cost_quality: generic_keyed_rows(set.elements(pids.wear_repair_cost_array)),
            wear_state_value: generic_keyed_rows(set.elements(pids.wear_state_array)),
            skill_gate: generic_keyed_rows(set.elements(pids.wear_skill_gate_array)),
        })
    }

    /// §9.6: the structure points of `Item_MaxStructurePointsEnum` key `key`.
    #[must_use]
    pub fn max_structure_points(&self, key: u32) -> Option<i32> {
        keyed_lookup(&self.max_structure_points, key)
    }

    /// §9.6: the durability value of `Item_DurabilityEnum` key `key`.
    #[must_use]
    pub fn durability_value(&self, key: u32) -> Option<f32> {
        keyed_lookup(&self.durability, key)
    }
}

/// The DIDs of the weapon-timing tables listed by the timing directory record
/// (`Item_WeaponActionDurationTable`, §6.3), null DIDs skipped.
#[must_use]
pub fn weapon_timing_table_dids<A: PropertyAccess + ?Sized>(
    directory: &A,
    pids: &ItemTablePids,
) -> Vec<Did> {
    directory.dids(pids.timing_directory).collect()
}

/// One timing entry: (speed key, action duration, animation-duration modifier).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponTiming {
    /// `Item_WeaponSpeed` key.
    pub speed: u32,
    /// `Item_BaseActionDuration`.
    pub action_duration: f32,
    /// `Item_BaseAnimDurationMultiplierMod`.
    pub anim_duration_mod: f32,
}

/// A weapon-timing table (class 1691 here, §6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeaponTimingTable {
    /// `Item_EquipmentCategory` bit field.
    pub category: u64,
    /// `Item_WeaponActionDurationDataArray` entries.
    pub entries: Vec<WeaponTiming>,
}

impl WeaponTimingTable {
    const WHAT: &'static str = "weapon timing table";

    /// Reads a timing table's property set.
    pub fn from_set<A: PropertyAccess + ?Sized>(set: &A, pids: &ItemTablePids) -> Result<Self> {
        let category = set.require_as(
            pids.timing_category,
            Self::WHAT,
            crate::PropertyType::Bitfield64,
            ValueExt::bitfield,
        )?;
        let arr = set.require_array(pids.timing_array, Self::WHAT)?;
        let entries = arr
            .elements
            .iter()
            .map(|e| {
                let s = e
                    .value
                    .as_struct()
                    .ok_or_else(|| malformed(Self::WHAT, "element is not a struct"))?;
                Ok(WeaponTiming {
                    speed: s.require_enum(pids.timing_speed, Self::WHAT)?,
                    action_duration: s.require_float(pids.timing_duration, Self::WHAT)?,
                    anim_duration_mod: s.require_float(pids.timing_anim_mod, Self::WHAT)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { category, entries })
    }
}

/// All weapon-timing tables (§9.7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeaponTimings {
    /// The tables, in directory order.
    pub tables: Vec<WeaponTimingTable>,
}

impl WeaponTimings {
    /// §9.7: the entry of the table whose category equals `category`, with speed `speed`.
    #[must_use]
    pub fn lookup(&self, category: u64, speed: u32) -> Option<WeaponTiming> {
        self.tables
            .iter()
            .filter(|t| t.category == category)
            .find_map(|t| t.entries.iter().find(|e| e.speed == speed))
            .copied()
    }
}

/// A duration map of the cooldown or decay control (§6.4, §9.8): enum key → seconds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DurationMap {
    /// (key, seconds).
    pub rows: Vec<(u32, f32)>,
}

impl DurationMap {
    /// Reads list `list` (`CooldownControl_DurationMapList` or
    /// `ItemDecayControl_DurationMapList`) of a control record. The entry field labels are
    /// not specified, so each struct is read by type: its first enum field is the key and
    /// its first float (or int) field the seconds. Absent list → empty map.
    #[must_use]
    pub fn from_set<A: PropertyAccess + ?Sized>(set: &A, list: Pid) -> Self {
        Self {
            rows: generic_keyed_rows(set.elements(list)),
        }
    }

    /// The seconds of key `key`; key 0 ("none") gives 0.0 when it has no row.
    #[must_use]
    pub fn seconds(&self, key: u32) -> Option<f32> {
        keyed_lookup(&self.rows, key).or((key == 0).then_some(0.0))
    }

    /// Whether `key` is 0 or has a row (§9.8 coverage).
    #[must_use]
    pub fn covers(&self, key: u32) -> bool {
        key == 0 || keyed_lookup(&self.rows, key).is_some()
    }
}

// ---------------------------------------------------------------------------------------------
// Item sets (§7.1, §9.9)
// ---------------------------------------------------------------------------------------------

/// The set DIDs listed by the `GameSetDirectory` record (`Set_Directory_GameSetList`),
/// null DIDs skipped.
#[must_use]
pub fn set_directory<A: PropertyAccess + ?Sized>(directory: &A, pids: &ItemSetPids) -> Vec<Did> {
    directory.dids(pids.directory).collect()
}

/// One bonus tier (`Set_ActiveCountData`, §7.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SetTier<'a> {
    /// `Set_ActiveCount`.
    pub active_count: Option<i32>,
    /// The tier's `Mod_Array` entries (`combat-data.md` §5.1).
    pub mods: Vec<ModEntry>,
    /// The tier's `Set_EffectDataList` structs.
    pub effects: Vec<EffectRef<'a>>,
    /// `Set_StartScriptID` key.
    pub start_script: Option<u32>,
    /// `Set_ShortDescriptionForCount`.
    pub short_description: Option<&'a StringInfo>,
}

/// Typed view of a set record's property set (§7.1).
#[derive(Debug, Clone, Copy)]
pub struct ItemSetView<'a> {
    set: &'a PropertySet,
    pids: &'a ItemSetPids,
    schema: &'a ItemSchema,
}

impl<'a> ItemSetView<'a> {
    /// A view of `set`.
    #[must_use]
    pub fn new(set: &'a PropertySet, schema: &'a ItemSchema) -> Self {
        Self {
            set,
            pids: &schema.sets,
            schema,
        }
    }

    /// `Set_Name`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids.name)
    }
    /// `Set_Description`.
    #[must_use]
    pub fn description(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids.description)
    }
    /// `Set_Icon`.
    #[must_use]
    pub fn icon(&self) -> Option<Did> {
        self.set.did(self.pids.icon)
    }
    /// `Set_Level`.
    #[must_use]
    pub fn level(&self) -> Option<i32> {
        self.set.int(self.pids.level)
    }
    /// `Set_LevelRequired` (a boolean; meaning OPEN).
    #[must_use]
    pub fn level_required(&self) -> Option<bool> {
        self.set.boolean(self.pids.level_required)
    }
    /// `Set_MaxPlayerLevel`.
    #[must_use]
    pub fn max_player_level(&self) -> Option<i32> {
        self.set.int(self.pids.max_player_level)
    }
    /// Whether the set has a `Set_MemberList`.
    #[must_use]
    pub fn has_member_list(&self) -> bool {
        self.set.has(self.pids.member_list)
    }
    /// The `Set_MemberList` DIDs in stored order, null DIDs skipped.
    #[must_use]
    pub fn members(&self) -> Vec<Did> {
        self.set.dids(self.pids.member_list).collect()
    }
    /// The bonus tiers in stored order.
    #[must_use]
    pub fn tiers(&self) -> Vec<SetTier<'a>> {
        let p = self.pids;
        self.set
            .structs(p.tiers)
            .map(|s| SetTier {
                active_count: s.int(p.active_count),
                mods: mod_entries(s, &self.schema.modifiers),
                effects: s
                    .structs(p.effects)
                    .map(|e| EffectRef::from_struct(e, &self.schema.effects))
                    .collect(),
                start_script: s.enum_key(p.start_script),
                short_description: s.string_info(p.short_description),
            })
            .collect()
    }
    /// Whether the tiers' `Set_ActiveCount` values strictly ascend (IT11). A tier without a
    /// count fails the test.
    #[must_use]
    pub fn tiers_ascending(&self) -> bool {
        let counts: Option<Vec<i32>> = self.tiers().iter().map(|t| t.active_count).collect();
        counts.is_some_and(|c| c.windows(2).all(|w| w[0] < w[1]))
    }
    /// `Set_ActiveCountProperty` (sets without a member list).
    #[must_use]
    pub fn active_count_property(&self) -> Option<u32> {
        self.set.property_ref(self.pids.active_count_property)
    }
    /// `Set_ActiveLevelProperty` (sets without a member list).
    #[must_use]
    pub fn active_level_property(&self) -> Option<u32> {
        self.set.property_ref(self.pids.active_level_property)
    }
}

/// Set membership (§9.9): the members of a set are the items in its member list plus the
/// items whose `Item_PropertySet` names it; the reverse index item → set.
#[derive(Debug, Clone, Default)]
pub struct SetMembership {
    members: BTreeMap<Did, Vec<Did>>,
    set_of: HashMap<Did, Did>,
    conflicts: Vec<(Did, Did, Did)>,
}

impl SetMembership {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, set: Did, item: Did) {
        if item.is_null() || set.is_null() {
            return;
        }
        match self.set_of.get(&item) {
            Some(&s) if s == set => return,
            Some(&s) => {
                tracing::debug!(%item, first = %s, second = %set, "item in two sets");
                self.conflicts.push((item, s, set));
                return;
            }
            None => {}
        }
        self.set_of.insert(item, set);
        self.members.entry(set).or_default().push(item);
    }

    /// §9.9 step 2: the member list of `set`.
    pub fn add_member_list(&mut self, set: Did, members: impl IntoIterator<Item = Did>) {
        self.members.entry(set).or_default();
        for m in members {
            self.add(set, m);
        }
    }

    /// §9.9 step 3: an item whose `Item_PropertySet` names `set`.
    pub fn add_property_set_member(&mut self, item: Did, set: Did) {
        self.add(set, item);
    }

    /// The members of `set`, in insertion order.
    #[must_use]
    pub fn members_of(&self, set: Did) -> &[Did] {
        self.members.get(&set).map_or(&[], Vec::as_slice)
    }

    /// The set `item` belongs to.
    #[must_use]
    pub fn set_of(&self, item: Did) -> Option<Did> {
        self.set_of.get(&item).copied()
    }

    /// Every set with at least one recorded list or member, ascending.
    pub fn sets(&self) -> impl Iterator<Item = Did> + '_ {
        self.members.keys().copied()
    }

    /// Items that were added to a second set, as (item, first set, second set). The first
    /// assignment is kept. Empty on valid data (§9.9 step 4, IT11).
    #[must_use]
    pub fn conflicts(&self) -> &[(Did, Did, Did)] {
        &self.conflicts
    }

    /// Number of items with a set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.set_of.len()
    }

    /// Whether no item has a set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set_of.is_empty()
    }
}

// ---------------------------------------------------------------------------------------------
// Relics, runic lists, legacies (§7.2, §7.3)
// ---------------------------------------------------------------------------------------------

/// Typed view of a relic (class 2219 here, §7.2).
#[derive(Debug, Clone, Copy)]
pub struct RelicView<'a> {
    set: &'a PropertySet,
    pids: &'a ItemAuxPids,
    modifiers: &'a ModifierPids,
}

impl<'a> RelicView<'a> {
    /// A view of `set`.
    #[must_use]
    pub fn new(set: &'a PropertySet, schema: &'a ItemSchema) -> Self {
        Self {
            set,
            pids: &schema.aux,
            modifiers: &schema.modifiers,
        }
    }
    /// `Runic_Name`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.pids.runic_name)
    }
    /// `Runic_Type` bit field.
    #[must_use]
    pub fn runic_type(&self) -> Option<u64> {
        self.set.bitfield(self.pids.runic_type)
    }
    /// `Runic_Level`.
    #[must_use]
    pub fn level(&self) -> Option<i32> {
        self.set.int(self.pids.runic_level)
    }
    /// `Runic_Tier` (an int or an enum key; the spec does not give the type).
    #[must_use]
    pub fn tier(&self) -> Option<i64> {
        number(self.set.get_pid(self.pids.runic_tier)?)
    }
    /// `Runic_RequiredItemLevel`.
    #[must_use]
    pub fn required_item_level(&self) -> Option<i32> {
        self.set.int(self.pids.runic_required_item_level)
    }
    /// The relic's `Mod_Array` entries (`combat-data.md` §5.1).
    #[must_use]
    pub fn mods(&self) -> Vec<ModEntry> {
        mod_entries(self.set, self.modifiers)
    }
}

/// One `RunicList_Array` element (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunicListEntry {
    /// `RunicList_DropFrequency` key.
    pub drop_frequency: Option<u32>,
    /// `RunicList_RunicOrTreasureGroup`: a relic or a runic treasure group.
    pub target: Option<Did>,
}

/// The entries of a runic list's `RunicList_Array`.
#[must_use]
pub fn runic_list<A: PropertyAccess + ?Sized>(set: &A, pids: &ItemAuxPids) -> Vec<RunicListEntry> {
    set.structs(pids.runic_list_array)
        .map(|s| RunicListEntry {
            drop_frequency: s.enum_key(pids.runic_list_frequency),
            target: s.did(pids.runic_list_target),
        })
        .collect()
}

/// One `ItemAdvancement_Effect_Array` element of a legacy table (class 2134 here, §7.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyEffect {
    /// `ItemAdvancement_Effect`.
    pub effect: Option<Did>,
    /// `ItemAdvancement_Mod_Weight` (int or float; the spec does not give the type).
    pub weight: Option<f64>,
}

/// The entries of a legacy table's `ItemAdvancement_Effect_Array`.
#[must_use]
pub fn legacy_effects<A: PropertyAccess + ?Sized>(
    set: &A,
    pids: &ItemAuxPids,
) -> Vec<LegacyEffect> {
    set.structs(pids.legacy_effect_array)
        .map(|s| LegacyEffect {
            effect: s.did(pids.legacy_effect),
            weight: s
                .get_pid(pids.legacy_weight)
                .and_then(PropertyValue::as_f64),
        })
        .collect()
}
