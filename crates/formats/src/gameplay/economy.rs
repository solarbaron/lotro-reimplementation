// SPDX-License-Identifier: GPL-3.0-or-later
//! Economy tables: loot generation, crafting recipes, vendors, barter and currency.
//!
//! Spec: `docs/specs/economy.md`.
//!
//! Every view here reads the **property form** of a record (its property set at
//! DID + 0x09000000), which the spec treats as authoritative (§1). Fields are found by their
//! dictionary labels ([`EconomyPids`]); record kinds are recognised by their characteristic
//! property ([`EconomyKind`], §3.4), never by class index or DID.
//!
//! | Item | Records | Spec |
//! |---|---|---|
//! | [`EconomyPids`], [`EconomyDefaults`], [`EconomyReader`] | labels, dictionary defaults, the reader | §1, §3 |
//! | [`EconomyKind`], [`EconomyReader::kind`], [`KindByClass`] | characteristic properties | §3.4 |
//! | [`TrophyList`], [`TreasureGroupProfile`], [`TreasureList`], [`WeightedTreasureTable`] | loot lists and pick-one tables | §4.1–§4.4 |
//! | [`FilteredTreasureList`], [`EntityFilter`] | filtered lists | §4.5 |
//! | [`LevelBasedLootTable`], [`SpeciesLevelLootTable`], [`PvpLevelLootTable`] | level and species tables | §4.6, §4.7, §9.5 |
//! | [`WorldEventLevelLootTable`], [`WorldEventLootTableDirectory`] | world-event tables | §4.8 |
//! | [`RunicLevelLootTable`], [`RunicList`], [`RunicTreasureGroup`] | runic loot | §4.9 |
//! | [`SkirmishTreasureLookup`], [`SkirmishCustomLootLookupTable`] | skirmish lookups | §4.10 |
//! | [`HobbyRewardProfile`] | hobby rewards | §4.11 |
//! | [`LootControl`], [`LootDirectoryControl`] | loot control singletons, drop-frequency resolution | §4.12 |
//! | [`Recipe`], [`Profession`], [`Vocation`], [`CraftGuild`], [`CraftControl`], [`CraftDirectory`] | crafting | §5 |
//! | [`BarterProfile`], [`BarterList`], [`FloatProgression`], [`lookup_quantity`] | barter, look-up quantities | §6.1, §6.2 |
//! | [`VendorInventory`], [`NpcVendor`], [`PerkVendor`] | vendors | §6.3, §6.4 |
//! | [`Currency`], [`CurrencyDirectory`], [`CurrencyControl`], [`StoreLinks`] | currency and store | §7, §8 |
//! | [`WeightedTable`], [`BarterEntry`] | native codecs of the object form (classes 175 and 1942) | §9.2, §9.3 |
//! | [`Invariant`], [`Violation`], [`EconomyRecord::check`] | invariants E-I3–E-I13 | §12 |
//! | [`CrossLinks`] | "drops from", "sold by", "bartered for", "crafted by", "used in" | §10 |
//!
//! Server behaviour (rolls, prices, crafting outcomes, §11.2–§11.6) is not implemented: the
//! spec leaves it open. [`cumulative_pick`] is the distribution-only weighted pick that
//! §11.3 allows an offline backend to use.
//!
//! # Implementation choices
//!
//! Engineering choices the spec leaves to the implementation:
//!
//! 1. **Types.** A field that is present with another value type than the spec's tables give
//!    fails the whole record with [`FormatError::PropertyTypeMismatch`]; an absent field takes
//!    its default (the dictionary default when the definition has one, else the default the
//!    spec lists, [`EconomyDefaults`]). The same holds for the elements of arrays and for
//!    shapes the spec states (a range bound of `EntityFilter_PropertyRange` is a one-element
//!    array, §4.5; a store bundle holds DID elements, §8). One place accepts several value
//!    types: the values of a float progression (§6.2: `Progression_MinimumIndexValue`
//!    as a 32- or 64-bit int, an element of `FloatProgression_Array` as a float or an int,
//!    bare or as the `FloatProgression_FloatValue` field of a struct).
//! 2. **Invariants** (§12) are reported as warnings ([`Violation`]), never as decode errors.
//! 3. **Repeated keys.** When a control table holds a key twice (drop frequency, XP reward),
//!    the first row wins.
//! 4. **Several characteristic properties** (§3.4): the set is classified by the first kind in
//!    [`EconomyKind::ALL`] order.
//! 5. **Sums.** [`WeightedTable::from_weights`] saturates subtree sums at 2³² − 1.
//! 6. **Object-form words.** Mirrors of signed fields ([`TrophyEntry::mirror`],
//!    [`ProfileEntry::mirror`], [`HobbyEntry::mirror`]) give a negative value as its
//!    two's-complement word; [`LevelBasedLootTable::mirror_maps`] leaves out negative levels.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::gameplay::common::{
    Pid, PropertyAccess, PropertyDefaults, Referrer, ReverseDidIndex, ValueExt,
    for_each_did_reference,
};
use crate::property::{PropertyEntry, PropertyNames, PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Property labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// Labels of the loot family (§4).
    pub struct LootPids {
        trophy_list = "LootGen_TrophyList",
        trophy_item_or_profile = "LootGen_TrophyList_ItemOrProfile",
        trophy_drop_frequency = "LootGen_TrophyList_DropFrequency",
        trophy_quantity = "LootGen_TrophyList_ItemQuantity",
        trophy_group_drop = "LootGen_TrophyList_GroupDrop",
        description = "Description",
        icon_override = "Icon_Layer_ImageOverrideDID",
        inventory_max_quantity = "Inventory_MaxQuantity",
        tgp_table = "TreasureGroupProfile_ItemTable",
        tgp_item = "TreasureGroupProfile_ItemTable_Item",
        tgp_weight = "TreasureGroupProfile_ItemTable_Weight",
        tgp_quantity = "TreasureGroupProfile_ItemTable_Quantity",
        treasure_list = "LootGen_TreasureList",
        treasure_list_profile = "LootGen_TreasureList_GroupProfile",
        treasure_list_weight = "LootGen_TreasureList_Weight",
        weighted_table = "LootGen_WeightedTreasureTable",
        weighted_trophy_list = "LootGen_WeightedTreasureTable_TrophyList",
        weighted_weight = "LootGen_WeightedTreasureTable_TrophyList_Weight",
        filtered_array = "LootGen_FilteredTreasureListArray",
        filtered_entry = "LootGen_FilteredTrophyTable_EntryDID",
        filter_array = "EntityFilter_Array",
        filter_property_set = "EntityFilter_PropertySet",
        filter_property_range = "EntityFilter_PropertyRange",
        filter_range_min = "EntityFilter_PropertyRange_Min",
        filter_range_max = "EntityFilter_PropertyRange_Max",
        filter_world_event = "EntityFilter_WorldEvent",
        world_event_condition = "WorldEvent_Condition",
        world_event = "WorldEvent_WorldEvent",
        world_event_operator = "WorldEvent_Operator",
        world_event_value = "WorldEvent_ConditionValue",
        world_event_all_conditions = "WorldEvent_AllConditionList",
        level_table = "LevelBasedLootTable_LootTable",
        level_table_level = "LevelBasedLootTable_Level",
        level_table_trophy_list = "LevelBasedLootTable_TrophyList",
        level_table_treasure = "LevelBasedLootTable_TreasureProfile",
        species_table = "SpeciesLevelLootTable",
        agent_species = "Agent_Species",
        agent_subspecies = "Agent_Subspecies",
        agent_class = "Agent_Class",
        examination_type = "MonsterLevel_ExaminationModStatType",
        level_table_ref = "LevelBasedLootTable_LevelTable",
        pvp_table = "PVPLevelLootTable",
        we_level_table = "WorldEventLevelLootTable_LootTable",
        we_level = "WorldEventLevelLootTable_Level",
        we_trophy_template = "WorldEventLevelLootTable_TrophyListTemplate",
        we_directory = "WorldEventLootTableDirectory_Array",
        we_directory_level_table = "WorldEventLootTableDirectory_LevelTable",
        runic_level_table = "RunicLevelLootTable_LootTable",
        runic_level_level = "RunicLevelLootTable_Level",
        runic_level_trophy = "RunicLevelLootTable_TrophyList",
        runic_level_runic = "RunicLevelLootTable_RunicTrophyList",
        runic_list = "RunicList_Array",
        runic_list_target = "RunicList_RunicOrTreasureGroup",
        runic_list_drop_frequency = "RunicList_DropFrequency",
        runic_group = "RunicTreasureGroup_Array",
        runic_group_relic = "RunicTreasureGroup_Runic",
        runic_group_weight = "RunicTreasureGroup_Weight",
        runic_pull_counts = "RunicTreasureGroup_PullCount_Array",
        runic_pull_count = "RunicTreasureGroup_PullCount",
        skirmish_encounter = "SkirmishTreasureLookup_EncounterDID",
        skirmish_match_parameters = "SkirmishTreasureLookup_SkirmishMatchParametersDID",
        skirmish_trophy = "SkirmishTreasureLookup_TrophyListTemplateDID",
        skirmish_treasure = "SkirmishTreasureLookup_TreasureListTemplateDID",
        skirmish_barter = "SkirmishTreasureLookup_BarterTrophyListDID",
        skirmish_reputation = "SkirmishTreasureLookup_ReputationTrophyListDID",
        skirmish_custom_table = "SkirmishCustomLootLookupTable_Array",
        hobby_progression = "HobbyRewardProfile_AdvancementProgression",
        hobby_items = "HobbyRewardProfile_ItemArray",
        hobby_item = "HobbyRewardProfile_Item",
        hobby_weight = "HobbyRewardProfile_Weight",
        hobby_min = "HobbyRewardProfile_MinProficiency",
        hobby_max = "HobbyRewardProfile_MaxProficiency",
        hobby_meta_score = "HobbyRewardProfile_MetaScore",
        hobby_time_available = "HobbyRewardProfile_TimeAvailable",
        control_drop_table = "LootGenControl_DropFrequencyTable",
        control_drop_label = "LootGenControl_DropFrequency_Label",
        control_drop_fraction = "LootGenControl_DropFrequency_Percentage",
        control_money_table = "LootGenControl_MoneyDropTable",
        control_money_maximum = "LootGenControl_MoneyDropTable_DropAmountMaximum",
        control_money_variance = "LootGenControl_MoneyDropTable_DropAmountVariance",
        control_modifier_table = "LootGenControl_MoneyDropModifierTable",
        control_modifier_label = "LootGenControl_MoneyDropModifier_Label",
        control_modifier_multiplier = "LootGenControl_MoneyDropModifier_Multiplier",
        directory_species_tables = "LootGenControl_SpeciesLevelLootTableArray",
        directory_pvp_table = "LootGenControl_PVPLevelLootTable",
    }
}

crate::property_ids! {
    /// Labels of the crafting family (§5).
    pub struct CraftPids {
        recipe_profession = "CraftRecipe_Profession",
        recipe_tier = "CraftRecipe_Tier",
        recipe_tier_array = "CraftRecipe_TierArray",
        recipe_ingredients = "CraftRecipe_IngredientList",
        recipe_ingredient = "CraftRecipe_Ingredient",
        recipe_ingredient_quantity = "CraftRecipe_IngredientQuantity",
        recipe_ingredient_consumed = "CraftRecipe_IngredientIsConsumed",
        recipe_optional_ingredients = "CraftRecipe_OptionalIngredientList",
        recipe_crit_bonus = "CraftRecipe_IngredientCritBonus",
        recipe_pack = "CraftRecipe_IngredientPack",
        recipe_pack_quantity = "CraftRecipe_IngredientPackQuantity",
        recipe_result = "CraftRecipe_ResultItem",
        recipe_result_quantity = "CraftRecipe_ResultItemQuantity",
        recipe_critical_result = "CraftRecipe_CriticalResultItem",
        recipe_critical_quantity = "CraftRecipe_CriticalResultItemQuantity",
        recipe_critical_chance = "CraftRecipe_CriticalSuccessChance",
        recipe_multi_output = "CraftRecipe_MultiOutputArray",
        recipe_xp_reward = "CraftRecipe_XPReward",
        recipe_xp_progression = "CraftRecipe_XPAward_Progression",
        recipe_xp_driving_property = "CraftRecipe_XPAward_Progression_DrivingProperty",
        recipe_xp_driver = "CraftRecipe_XPAward_Driver",
        recipe_level_munge = "CraftRecipe_LevelMungeOutput",
        recipe_ui_category = "CraftRecipe_UICategory",
        recipe_execution_time = "CraftRecipe_ExecutionTime",
        recipe_cooldown = "CraftRecipe_CooldownDuration",
        recipe_cooldown_reset_cost = "CraftRecipe_Cooldown_MithrilResetCost",
        recipe_one_time = "CraftRecipe_OneTimeRecipe",
        recipe_guild = "CraftRecipe_RequiredCraftGuild",
        recipe_facility = "CraftRecipe_RequiredFacilityEffect",
        recipe_induction = "CraftRecipe_InductionAction",
        recipe_animation = "CraftRecipe_Animation",
        recipe_name = "CraftRecipe_Name",
        recipe_name_normal = "CraftRecipe_NameItemOnNormalSuccess",
        recipe_name_critical = "CraftRecipe_NameItemOnCriticalSuccess",
        recipe_result_icon = "CraftRecipe_Field_ResultIcon",
        recipe_critical_icon = "CraftRecipe_Field_CritResultIcon",
        critical_effects = "EffectGenerator_CraftingCriticalSuccessEffectList",
        effect_id = "EffectGenerator_EffectID",
        effect_spellcraft = "EffectGenerator_EffectSpellcraft",
        crafting_level_offset = "Item_iLevel_Crafting_Offset",
        profession_tiers = "CraftProfession_TierArray",
        profession_tier = "CraftProfession_Tier",
        profession_recipes = "CraftProfession_RecipeArray",
        profession_proficiency_cap = "CraftProfession_ProficiencyXPCap",
        profession_mastery_cap = "CraftProfession_MasteryXPCap",
        profession_level_gate = "CraftProfession_TierLevelCapGate",
        profession_proficiency_title = "CraftProfession_ProficiencyTitle",
        profession_mastery_title = "CraftProfession_MasteryTitle",
        profession_completed_proficiency =
            "CraftProfession_CompletedProficiencyLevelCharacteristic",
        profession_completed_mastery = "CraftProfession_CompletedMasteryLevelCharacteristic",
        profession_tier_opening = "CraftProfession_TierOpeningCharacteristic",
        profession_auto_proficiency = "CraftProfession_AutoCompleteProficiency",
        profession_auto_mastery = "CraftProfession_AutoCompleteMastery",
        profession_tutorial_hint = "CraftProfession_ProficiencyTutorialHint",
        profession_item = "CraftProfession_Item",
        profession_tool = "CraftProfession_RequiredTool",
        profession_characteristic = "CraftProfession_Characteristic",
        profession_enabled = "CraftProfession_Enabled_PropertyName",
        profession_proficiency_level = "CraftProfession_ProficiencyLevel_PropertyName",
        profession_proficiency_xp = "CraftProfession_ProficiencyXP_PropertyName",
        profession_mastery_level = "CraftProfession_MasteryLevel_PropertyName",
        profession_mastery_xp = "CraftProfession_MasteryXP_PropertyName",
        profession_completed_proficiency_level =
            "CraftProfession_CompletedProficiencyLevel_PropertyName",
        profession_completed_mastery_level = "CraftProfession_CompletedMasteryLevel_PropertyName",
        profession_critical_chance_modifier =
            "CraftProfession_CriticalChanceAddModifier_PropertyName",
        profession_start_time = "CraftProfession_StartTime_PropertyName",
        profession_extra_recipes = "CraftProfession_ExtraRecipeArray_PropertyName",
        profession_open_level = "CraftProfession_OpenLevel_PropertyName",
        profession_tier_open_level = "CraftProfession_TierOpenLevel_PropertyName",
        profession_xp_acceleration = "CraftProfession_AllowsXPAcceleration",
        profession_name = "CraftProfession_Name",
        vocation_professions = "CraftVocation_ProfessionArray",
        vocation_items = "CraftVocation_ItemArray",
        vocation_name = "CraftVocation_Name",
        guild_profession = "CraftGuild_Profession",
        guild_faction = "CraftGuild_Faction",
        guild_required_tier = "CraftGuild_RequiredCraftTier",
        guild_quest_category = "CraftGuild_QuestCategory",
        control_xp_rewards = "CraftControl_XPRewardArray",
        control_xp_key = "CraftControl_XPRewardEnum",
        control_xp_value = "CraftControl_XPRewardValue",
        control_tier_levels = "CraftControl_TierToMungeLevelArray",
        usage_min_level = "Usage_MinLevel",
        control_remove_quests = "CraftControl_ChangeVocation_RemoveQuestsArray",
        control_profession = "CraftControl_Profession",
        control_quests = "CraftControl_RemoveQuestsArray",
        directory_vocations = "CraftDirectory_VocationArray",
        directory_professions = "CraftDirectory_ProfessionArray",
        directory_guilds = "CraftDirectory_CraftGuildArray",
        recipe_item_recipe = "RecipeItem_Recipe",
    }
}

crate::property_ids! {
    /// Labels of vendors, barter, currency, store records and the look-up rule (§6–§8).
    pub struct VendorPids {
        barter_lists = "Barter_ItemListArray",
        barter_cost = "Barter_ItemStruct",
        barter_item = "Barter_Item",
        barter_quantity = "Barter_ItemQuantity",
        barter_lookup = "Barter_ItemQuantity_LookupTable",
        barter_reward = "Barter_RewardItemStruct",
        barter_uses_min_level = "Barter_ItemUsesMinUsageLevelForCost",
        barter_reputation = "Barter_RewardReputationStruct",
        barter_faction = "Barter_ReputationFaction",
        barter_reputation_tier = "Barter_ReputationRewardTier",
        barter_sound = "Barter_SoundCategory",
        barter_name = "Barter_Profile_Name",
        barter_munge_level = "Barter_ItemMungeLevelOverride",
        permissions = "DefaultPermissionBlobStruct",
        inventory_items = "VendorInventory_Items",
        vendor_inventories = "Vendor_InventoryList",
        vendor_items = "Vendor_ItemList",
        vendor_sell_modifier = "Vendor_SellModifier",
        vendor_sells_wearables = "Vendor_SellsWearableItems",
        vendor_buys_items = "Vendor_BuysItems",
        vendor_discounts = "Vendor_DiscountList",
        barter_profiles = "Barter_ProfileArray",
        barter_use_tabs = "Barter_Profile_UseTabs",
        offered_professions = "Craft_OfferedProfessionArray",
        offered_guilds = "Craft_OfferedCraftGuildArray",
        offered_vocations = "Craft_OfferedVocationArray",
        perk_inventory = "PerkVendor_InventoryList",
        currency_value = "Currency_Value",
        currency_object = "Currency_ObjectDID",
        currency_name = "Currency_Name",
        currency_singular = "Currency_Name_Singular",
        currency_plural = "Currency_Name_Plural",
        currency_list = "Currency_CurrencyList",
        currency_garnishment = "Currency_Control_GarnishmentPercentage",
        currency_gold_cap_item = "Currency_Control_GoldCapWebstoreItem",
        store_inventory_item = "WebStoreInventoryItem_DataID",
        store_bundle = "WebStoreAccountItem_BundleItemArray",
        store_account_item = "WebStoreAccountItem_DataID",
        progression_minimum_index = "Progression_MinimumIndexValue",
        float_progression = "FloatProgression_Array",
        float_progression_value = "FloatProgression_FloatValue",
        item_level = "Item_Level",
        usage_min_level = "Usage_MinLevel",
    }
}

/// Every label this module reads, resolved against one dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EconomyPids {
    /// Loot tables (§4).
    pub loot: LootPids,
    /// Crafting (§5).
    pub craft: CraftPids,
    /// Vendors, barter, currency, store (§6–§8).
    pub vendor: VendorPids,
}

impl EconomyPids {
    /// Resolves every label against `names` (normally the property dictionary).
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            loot: LootPids::resolve(names),
            craft: CraftPids::resolve(names),
            vendor: VendorPids::resolve(names),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            loot: LootPids::unresolved(),
            craft: CraftPids::unresolved(),
            vendor: VendorPids::unresolved(),
        }
    }

    /// The labels the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.loot.missing();
        out.extend(self.craft.missing());
        out.extend(self.vendor.missing());
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------------------------

/// The dictionary defaults that matter for this family (§1 "absent field"): the value of a
/// field that is not stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EconomyDefaults {
    /// `LootGen_TrophyList_ItemQuantity` (§4.1).
    pub trophy_quantity: i32,
    /// `TreasureGroupProfile_ItemTable_Quantity` (§4.2).
    pub profile_quantity: i32,
    /// `LevelBasedLootTable_Level` (§4.6).
    pub level_table_level: i32,
    /// `WorldEventLevelLootTable_Level` (§4.8): the spec lists no default, so only the
    /// dictionary's, if its definition has one.
    pub world_event_level: Option<i32>,
    /// `RunicLevelLootTable_Level` (§4.9, E2).
    pub runic_level: i32,
    /// `CraftRecipe_ResultItemQuantity` (§5.1).
    pub result_quantity: i32,
    /// `CraftRecipe_CriticalResultItemQuantity` (§5.1).
    pub critical_quantity: i32,
    /// `CraftRecipe_IngredientIsConsumed` (§5.1).
    pub ingredient_consumed: bool,
    /// `CraftRecipe_IngredientCritBonus` (§5.1).
    pub crit_bonus: f32,
    /// `Barter_ItemMungeLevelOverride` (§6.1).
    pub munge_level_override: i32,
    /// `Vendor_SellModifier` (§6.3).
    pub sell_modifier: f32,
}

impl EconomyDefaults {
    /// The defaults the spec lists (§4.1, §4.2, §4.6, §5.1, §6.1, §6.3).
    pub const SPEC: Self = Self {
        trophy_quantity: 1,
        profile_quantity: 1,
        level_table_level: 1,
        world_event_level: None,
        runic_level: 1,
        result_quantity: 1,
        critical_quantity: 1,
        ingredient_consumed: true,
        crit_bonus: 0.0,
        munge_level_override: -1,
        sell_modifier: -1.0,
    };

    /// Reads the defaults from the dictionary. A property whose definition has no default
    /// of the expected type keeps the value of [`EconomyDefaults::SPEC`] (logged at debug
    /// level).
    #[must_use]
    pub fn resolve<D: PropertyDefaults + ?Sized>(defaults: &D, pids: &EconomyPids) -> Self {
        let spec = Self::SPEC;
        let int = |pid: Pid, fallback: i32| {
            dictionary_default(defaults, pid, ValueExt::int).unwrap_or(fallback)
        };
        let float = |pid: Pid, fallback: f32| {
            dictionary_default(defaults, pid, ValueExt::float).unwrap_or(fallback)
        };
        let boolean = |pid: Pid, fallback: bool| {
            dictionary_default(defaults, pid, ValueExt::boolean).unwrap_or(fallback)
        };
        Self {
            trophy_quantity: int(pids.loot.trophy_quantity, spec.trophy_quantity),
            profile_quantity: int(pids.loot.tgp_quantity, spec.profile_quantity),
            level_table_level: int(pids.loot.level_table_level, spec.level_table_level),
            world_event_level: dictionary_default(defaults, pids.loot.we_level, ValueExt::int)
                .or(spec.world_event_level),
            runic_level: int(pids.loot.runic_level_level, spec.runic_level),
            result_quantity: int(pids.craft.recipe_result_quantity, spec.result_quantity),
            critical_quantity: int(pids.craft.recipe_critical_quantity, spec.critical_quantity),
            ingredient_consumed: boolean(
                pids.craft.recipe_ingredient_consumed,
                spec.ingredient_consumed,
            ),
            crit_bonus: float(pids.craft.recipe_crit_bonus, spec.crit_bonus),
            munge_level_override: int(pids.vendor.barter_munge_level, spec.munge_level_override),
            sell_modifier: float(pids.vendor.vendor_sell_modifier, spec.sell_modifier),
        }
    }
}

impl Default for EconomyDefaults {
    fn default() -> Self {
        Self::SPEC
    }
}

fn dictionary_default<D: PropertyDefaults + ?Sized, T>(
    defaults: &D,
    pid: Pid,
    f: impl FnOnce(&PropertyValue) -> Option<T>,
) -> Option<T> {
    let found = pid
        .id()
        .and_then(|id| defaults.default_value(id))
        .and_then(f);
    if found.is_none() {
        tracing::debug!(
            label = pid.label(),
            "no dictionary default; spec value used"
        );
    }
    found
}

// ---------------------------------------------------------------------------------------------
// Kinds
// ---------------------------------------------------------------------------------------------

/// The kind of an economy record (§2, §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EconomyKind {
    /// Trophy list, including the barter and reputation subclasses (§4.1).
    TrophyList,
    /// Treasure group profile, both classes (§4.2).
    TreasureGroupProfile,
    /// Treasure list (§4.3).
    TreasureList,
    /// Weighted treasure table (§4.4).
    WeightedTreasureTable,
    /// Filtered treasure list (§4.5).
    FilteredTreasureList,
    /// Level-based loot table (§4.6).
    LevelBasedLootTable,
    /// Species level loot table (§4.7).
    SpeciesLevelLootTable,
    /// PVP level loot table (§4.7).
    PvpLevelLootTable,
    /// World-event level loot table (§4.8).
    WorldEventLevelLootTable,
    /// World-event loot table directory (§4.8).
    WorldEventLootTableDirectory,
    /// Runic level loot table (§4.9).
    RunicLevelLootTable,
    /// Runic list (§4.9; counted with the item-advancement family in §2).
    RunicList,
    /// Runic treasure group (§4.9).
    RunicTreasureGroup,
    /// Skirmish treasure lookup (§4.10).
    SkirmishTreasureLookup,
    /// Skirmish custom loot lookup table (§4.10).
    SkirmishCustomLootLookupTable,
    /// Hobby reward profile (§4.11).
    HobbyRewardProfile,
    /// Loot control singleton (§4.12).
    LootControl,
    /// Loot directory control singleton (§4.12).
    LootDirectoryControl,
    /// Recipe (§5.1).
    Recipe,
    /// Profession (§5.2).
    Profession,
    /// Vocation (§5.3).
    Vocation,
    /// Craft guild (§5.3).
    CraftGuild,
    /// Craft control (§5.4).
    CraftControl,
    /// Craft directory (§5.4).
    CraftDirectory,
    /// Barter profile (§6.1).
    BarterProfile,
    /// Vendor inventory (§6.3).
    VendorInventory,
    /// Perk vendor (§6.4).
    PerkVendor,
    /// Currency denomination (§7).
    Currency,
    /// Currency directory (§7).
    CurrencyDirectory,
    /// Currency control (§7).
    CurrencyControl,
}

impl EconomyKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 30] = [
        Self::TrophyList,
        Self::TreasureGroupProfile,
        Self::TreasureList,
        Self::WeightedTreasureTable,
        Self::FilteredTreasureList,
        Self::LevelBasedLootTable,
        Self::SpeciesLevelLootTable,
        Self::PvpLevelLootTable,
        Self::WorldEventLevelLootTable,
        Self::WorldEventLootTableDirectory,
        Self::RunicLevelLootTable,
        Self::RunicList,
        Self::RunicTreasureGroup,
        Self::SkirmishTreasureLookup,
        Self::SkirmishCustomLootLookupTable,
        Self::HobbyRewardProfile,
        Self::LootControl,
        Self::LootDirectoryControl,
        Self::Recipe,
        Self::Profession,
        Self::Vocation,
        Self::CraftGuild,
        Self::CraftControl,
        Self::CraftDirectory,
        Self::BarterProfile,
        Self::VendorInventory,
        Self::PerkVendor,
        Self::Currency,
        Self::CurrencyDirectory,
        Self::CurrencyControl,
    ];

    /// Whether the kind is a loot table: the classes of §2.1 and the runic list (the records
    /// a "drops from" walk passes through, §10.2 step 4).
    #[must_use]
    pub fn is_loot_table(self) -> bool {
        matches!(
            self,
            Self::TrophyList
                | Self::TreasureGroupProfile
                | Self::TreasureList
                | Self::WeightedTreasureTable
                | Self::FilteredTreasureList
                | Self::LevelBasedLootTable
                | Self::SpeciesLevelLootTable
                | Self::PvpLevelLootTable
                | Self::WorldEventLevelLootTable
                | Self::WorldEventLootTableDirectory
                | Self::RunicLevelLootTable
                | Self::RunicList
                | Self::RunicTreasureGroup
                | Self::SkirmishTreasureLookup
                | Self::SkirmishCustomLootLookupTable
                | Self::HobbyRewardProfile
                | Self::LootControl
                | Self::LootDirectoryControl
        )
    }

    /// Whether a "drops from" walk shows the kind as a source of its own (§10.2 step 5):
    /// species and PVP tables, world-event and skirmish lookups.
    #[must_use]
    pub fn is_special_source(self) -> bool {
        matches!(
            self,
            Self::SpeciesLevelLootTable
                | Self::PvpLevelLootTable
                | Self::WorldEventLevelLootTable
                | Self::WorldEventLootTableDirectory
                | Self::SkirmishTreasureLookup
                | Self::SkirmishCustomLootLookupTable
        )
    }

    /// The characteristic properties of the kind (§3.4).
    #[must_use]
    pub fn characteristic(self, p: &EconomyPids) -> Pid {
        match self {
            Self::TrophyList => p.loot.trophy_list,
            Self::TreasureGroupProfile => p.loot.tgp_table,
            Self::TreasureList => p.loot.treasure_list,
            Self::WeightedTreasureTable => p.loot.weighted_table,
            Self::FilteredTreasureList => p.loot.filtered_array,
            Self::LevelBasedLootTable => p.loot.level_table,
            Self::SpeciesLevelLootTable => p.loot.species_table,
            Self::PvpLevelLootTable => p.loot.pvp_table,
            Self::WorldEventLevelLootTable => p.loot.we_level_table,
            Self::WorldEventLootTableDirectory => p.loot.we_directory,
            Self::RunicLevelLootTable => p.loot.runic_level_table,
            Self::RunicList => p.loot.runic_list,
            Self::RunicTreasureGroup => p.loot.runic_group,
            Self::SkirmishTreasureLookup => p.loot.skirmish_trophy,
            Self::SkirmishCustomLootLookupTable => p.loot.skirmish_custom_table,
            Self::HobbyRewardProfile => p.loot.hobby_items,
            Self::LootControl => p.loot.control_drop_table,
            Self::LootDirectoryControl => p.loot.directory_species_tables,
            Self::Recipe => p.craft.recipe_result,
            Self::Profession => p.craft.profession_tiers,
            Self::Vocation => p.craft.vocation_professions,
            Self::CraftGuild => p.craft.guild_profession,
            Self::CraftControl => p.craft.control_xp_rewards,
            Self::CraftDirectory => p.craft.directory_professions,
            Self::BarterProfile => p.vendor.barter_lists,
            Self::VendorInventory => p.vendor.inventory_items,
            Self::PerkVendor => p.vendor.perk_inventory,
            Self::Currency => p.vendor.currency_value,
            Self::CurrencyDirectory => p.vendor.currency_list,
            Self::CurrencyControl => p.vendor.currency_garnishment,
        }
    }
}

/// Root classes learned from records whose kind the characteristic property showed, so that
/// records without one (the empty vendor inventories, §3.4) can be classified by class.
///
/// A class seen with two different kinds is not used.
#[derive(Debug, Clone, Default)]
pub struct KindByClass {
    classes: HashMap<u32, Option<EconomyKind>>,
}

impl KindByClass {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a record of `class` has kind `kind`.
    pub fn learn(&mut self, class: u32, kind: EconomyKind) {
        let slot = self.classes.entry(class).or_insert(Some(kind));
        if *slot != Some(kind) {
            if slot.is_some() {
                tracing::debug!(class, "root class seen with two economy kinds");
            }
            *slot = None;
        }
    }

    /// The kind of `class`, if every record of it seen so far had that one kind.
    #[must_use]
    pub fn kind(&self, class: u32) -> Option<EconomyKind> {
        self.classes.get(&class).copied().flatten()
    }

    /// The learned classes of `kind`, ascending.
    #[must_use]
    pub fn classes_of(&self, kind: EconomyKind) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .classes
            .iter()
            .filter(|&(_, k)| *k == Some(kind))
            .map(|(&c, _)| c)
            .collect();
        v.sort_unstable();
        v
    }
}

// ---------------------------------------------------------------------------------------------
// Typed field access
// ---------------------------------------------------------------------------------------------

fn mismatch(
    what: &'static str,
    pid: Pid,
    expected: &'static str,
    v: &PropertyValue,
) -> FormatError {
    FormatError::PropertyTypeMismatch {
        what,
        label: pid.label(),
        expected,
        found: v.property_type(),
    }
}

/// The value of `pid` converted by `f`: absent → `None`; present with another type →
/// [`FormatError::PropertyTypeMismatch`].
fn typed<'s, A: PropertyAccess + ?Sized, T>(
    s: &'s A,
    pid: Pid,
    what: &'static str,
    expected: &'static str,
    f: impl FnOnce(&'s PropertyValue) -> Option<T>,
) -> Result<Option<T>> {
    let Some(v) = s.get_pid(pid) else {
        return Ok(None);
    };
    f(v).map(Some)
        .ok_or_else(|| mismatch(what, pid, expected, v))
}

fn opt_int<A: PropertyAccess + ?Sized>(s: &A, pid: Pid, what: &'static str) -> Result<Option<i32>> {
    typed(s, pid, what, "int", ValueExt::int)
}

fn opt_enum<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<u32>> {
    typed(s, pid, what, "enum", ValueExt::enum_key)
}

fn opt_bool<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<bool>> {
    typed(s, pid, what, "bool", ValueExt::boolean)
}

fn opt_float<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<f32>> {
    typed(s, pid, what, "float", ValueExt::float)
}

fn opt_bits<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<u64>> {
    typed(s, pid, what, "bitfield", ValueExt::bitfield)
}

fn opt_pid_ref<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<u32>> {
    typed(s, pid, what, "property ID", ValueExt::property_ref)
}

/// A DID field; the null DID is returned as stored.
fn opt_did<A: PropertyAccess + ?Sized>(s: &A, pid: Pid, what: &'static str) -> Result<Option<Did>> {
    typed(s, pid, what, "DID", ValueExt::did_ref)
}

/// A DID field; absent and null both give `None`.
fn nonnull_did<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<Did>> {
    Ok(opt_did(s, pid, what)?.filter(|d| !d.is_null()))
}

/// A DID field; absent gives the null DID.
fn did_or_null<A: PropertyAccess + ?Sized>(s: &A, pid: Pid, what: &'static str) -> Result<Did> {
    Ok(opt_did(s, pid, what)?.unwrap_or_default())
}

fn opt_text<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<StringInfo>> {
    typed(s, pid, what, "StringInfo", |v| v.string_info().cloned())
}

fn opt_struct<'s, A: PropertyAccess + ?Sized>(
    s: &'s A,
    pid: Pid,
    what: &'static str,
) -> Result<Option<&'s PropertyStruct>> {
    typed(s, pid, what, "struct", PropertyValue::as_struct)
}

/// The elements of array `pid`; absent → empty.
fn elements<'s, A: PropertyAccess + ?Sized>(
    s: &'s A,
    pid: Pid,
    what: &'static str,
) -> Result<&'s [PropertyEntry]> {
    Ok(typed(s, pid, what, "array", PropertyValue::as_array)?
        .map_or(&[][..], |a| a.elements.as_slice()))
}

/// The struct elements of array `pid`; any other element is an error.
fn struct_elements<'s, A: PropertyAccess + ?Sized>(
    s: &'s A,
    pid: Pid,
    what: &'static str,
) -> Result<Vec<&'s PropertyStruct>> {
    elements(s, pid, what)?
        .iter()
        .map(|e| {
            e.value
                .as_struct()
                .ok_or_else(|| mismatch(what, pid, "struct elements", &e.value))
        })
        .collect()
}

/// The DID elements of array `pid`, in stored order, null DIDs kept; any other element is
/// an error.
fn did_elements<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Vec<Did>> {
    elements(s, pid, what)?
        .iter()
        .map(|e| {
            e.value
                .did_ref()
                .ok_or_else(|| mismatch(what, pid, "DID elements", &e.value))
        })
        .collect()
}

/// The enum elements of array `pid`, in stored order.
fn enum_elements<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Vec<u32>> {
    elements(s, pid, what)?
        .iter()
        .map(|e| {
            e.value
                .enum_key()
                .ok_or_else(|| mismatch(what, pid, "enum elements", &e.value))
        })
        .collect()
}

/// The property-ID elements of array `pid`, in stored order.
fn pid_elements<A: PropertyAccess + ?Sized>(
    s: &A,
    pid: Pid,
    what: &'static str,
) -> Result<Vec<u32>> {
    elements(s, pid, what)?
        .iter()
        .map(|e| {
            e.value
                .property_ref()
                .ok_or_else(|| mismatch(what, pid, "property-ID elements", &e.value))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Loot views (§4)
// ---------------------------------------------------------------------------------------------

/// One drop candidate of a trophy list (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrophyEntry {
    /// An item, a treasure group profile or a treasure list (null when absent).
    pub target: Did,
    /// The drop-frequency key; `None` when absent.
    pub drop_frequency: Option<u32>,
    /// `ItemQuantity`, with the default filled in.
    pub quantity: i32,
    /// Whether `ItemQuantity` was stored.
    pub quantity_stored: bool,
    /// `GroupDrop` (absent: false).
    pub group_drop: bool,
}

impl TrophyEntry {
    /// The attribute values of the matching class-1233 object (§9.4, §13.1): group drop,
    /// drop-frequency key, quantity, target DID, and the fifth attribute (0 in all observed;
    /// OPEN in §14). Negative quantities are given as their two's-complement word.
    #[must_use]
    pub fn mirror(&self) -> [u32; 5] {
        [
            u32::from(self.group_drop),
            self.drop_frequency.unwrap_or(0),
            self.quantity as u32,
            self.target.0,
            0,
        ]
    }
}

/// Trophy list (classes 1237, 1917, 1918; §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct TrophyList {
    /// Entries in stored order.
    pub entries: Vec<TrophyEntry>,
    /// `Description` (26 lists).
    pub description: Option<StringInfo>,
    /// `Icon_Layer_ImageOverrideDID` (a render surface).
    pub icon: Option<Did>,
    /// `Inventory_MaxQuantity`.
    pub max_quantity: Option<i32>,
}

/// One weighted item of a treasure group profile (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileEntry {
    /// The item (null when absent).
    pub item: Did,
    /// The weight (absent: 0).
    pub weight: i32,
    /// `Quantity`, with the default filled in.
    pub quantity: i32,
    /// Whether `Quantity` was stored.
    pub quantity_stored: bool,
}

impl ProfileEntry {
    /// The attribute values of the matching class-2810 object (§9.4): quantity, item DID. A
    /// negative quantity is given as its two's-complement word.
    #[must_use]
    pub fn mirror(&self) -> [u32; 2] {
        [self.quantity as u32, self.item.0]
    }
}

/// Treasure group profile (classes 1235, 2372; §4.2): one entry is chosen with probability
/// weight ÷ sum of weights (Reported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasureGroupProfile {
    /// Entries in stored order.
    pub entries: Vec<ProfileEntry>,
}

impl TreasureGroupProfile {
    /// The weights in stored order (negative weights count as 0).
    #[must_use]
    pub fn weights(&self) -> Vec<u32> {
        self.entries
            .iter()
            .map(|e| clamp_weight(e.weight))
            .collect()
    }
}

/// A (target, weight) pair of a pick-one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedRef {
    /// The target (null when absent).
    pub target: Did,
    /// The weight (absent: 0).
    pub weight: i32,
}

/// Treasure list (class 1236; §4.3): a pick-one table of treasure group profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasureList {
    /// Entries in stored order.
    pub entries: Vec<WeightedRef>,
}

/// Weighted treasure table (class 3307; §4.4): a pick-one table of trophy lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedTreasureTable {
    /// Entries in stored order.
    pub entries: Vec<WeightedRef>,
}

impl WeightedTreasureTable {
    /// The entries sorted by ascending weight, as the object form stores them (§9.4). Equal
    /// weights keep their stored order here; the object form's order among equal weights is
    /// OPEN (§14), so compare runs of equal weights as multisets.
    #[must_use]
    pub fn sorted_by_weight(&self) -> Vec<WeightedRef> {
        let mut v = self.entries.clone();
        v.sort_by_key(|e| e.weight);
        v
    }
}

/// One tested property value of an entity filter (§4.5): the element's PID names the
/// tested property, its value is the allowed value or bound.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyTest {
    /// PID of the tested property.
    pub pid: u32,
    /// The allowed value or the bound.
    pub value: PropertyValue,
}

/// A world-event condition (`WorldEvent_Condition`; §4.5, §4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldEventCondition {
    /// `WorldEvent_WorldEvent` (null when absent).
    pub world_event: Did,
    /// `WorldEvent_Operator` key.
    pub operator: Option<u32>,
    /// `WorldEvent_ConditionValue` (int, type 15; E1). Absent: `None` (dictionary default
    /// −1 on the observed install).
    pub value: Option<i32>,
}

/// One filter of a filtered-treasure-list entry (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub enum EntityFilter {
    /// `EntityFilter_PropertySet`: allowed values (may be empty).
    PropertySet(Vec<PropertyTest>),
    /// `EntityFilter_PropertyRange`: the element of the minimum and of the maximum array
    /// (each a one-element array; absent: `None`).
    PropertyRange {
        /// Lower bound.
        min: Option<PropertyTest>,
        /// Upper bound.
        max: Option<PropertyTest>,
    },
    /// `EntityFilter_WorldEvent`.
    WorldEvent(Option<WorldEventCondition>),
    /// Another element kind the definition allows (§4.5); kept by PID only.
    Other {
        /// The element's PID.
        pid: u32,
    },
}

impl EntityFilter {
    /// The PID of the property a property-set or range filter tests, if one PID is shared by
    /// all its values (§4.5).
    #[must_use]
    pub fn tested_property(&self) -> Option<u32> {
        match self {
            Self::PropertySet(v) => {
                let first = v.first()?.pid;
                v.iter().all(|t| t.pid == first).then_some(first)
            }
            Self::PropertyRange { min, max } => match (min, max) {
                (Some(a), Some(b)) if a.pid == b.pid => Some(a.pid),
                (Some(a), None) | (None, Some(a)) => Some(a.pid),
                _ => None,
            },
            _ => None,
        }
    }
}

/// One (table, filters) entry of a filtered treasure list (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredEntry {
    /// `LootGen_FilteredTrophyTable_EntryDID`: a trophy list or weighted table.
    pub table: Did,
    /// The filters, in stored order.
    pub filters: Vec<EntityFilter>,
}

/// Filtered treasure list (class 3666; §4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredTreasureList {
    /// Entries in stored order.
    pub entries: Vec<FilteredEntry>,
}

/// One level of a level-based loot table (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelLootEntry {
    /// `LevelBasedLootTable_Level`, with the default (1, §4.6) filled in.
    pub level: i32,
    /// Whether `LevelBasedLootTable_Level` was stored.
    pub level_stored: bool,
    /// Trophy list, `None` for 0.
    pub trophy_list: Option<Did>,
    /// Treasure list (`…_TreasureProfile`), `None` for 0.
    pub treasure_list: Option<Did>,
}

/// Level-based loot table (class 3669; §4.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelBasedLootTable {
    /// Entries in stored order.
    pub entries: Vec<LevelLootEntry>,
}

impl LevelBasedLootTable {
    /// The entry whose stored level is `level`.
    #[must_use]
    pub fn at_level(&self, level: i32) -> Option<&LevelLootEntry> {
        self.entries.iter().find(|e| e.level == level)
    }

    /// The object-form maps (§9.1): level → trophy list and level → treasure list, the
    /// non-zero entries only. Entries with a negative level are left out (no map key).
    #[must_use]
    pub fn mirror_maps(&self) -> (BTreeMap<u32, u32>, BTreeMap<u32, u32>) {
        let mut trophies = BTreeMap::new();
        let mut treasures = BTreeMap::new();
        for e in &self.entries {
            let Ok(level) = u32::try_from(e.level) else {
                continue;
            };
            if let Some(t) = e.trophy_list {
                trophies.insert(level, t.0);
            }
            if let Some(t) = e.treasure_list {
                treasures.insert(level, t.0);
            }
        }
        (trophies, treasures)
    }
}

/// One entry of a species level loot table (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeciesEntry {
    /// `Agent_Species` key.
    pub species: u32,
    /// `Agent_Subspecies` key (absent: 0).
    pub subspecies: u32,
    /// `MonsterLevel_ExaminationModStatType` key (absent: 0).
    pub examination_type: u32,
    /// `LevelBasedLootTable_LevelTable`.
    pub level_table: Did,
}

impl SpeciesEntry {
    /// The object-form key (§9.5, §13.5): examination type × 2³² + species × 2²⁰ +
    /// subspecies × 2⁴. `None` when the fields would overlap (species ≥ 2¹², subspecies ≥
    /// 2¹⁶), a packing the spec does not cover.
    #[must_use]
    pub fn key(&self) -> Option<u64> {
        species_key(self.species, self.subspecies, self.examination_type)
    }
}

/// The species-table key of §9.5; see [`SpeciesEntry::key`].
#[must_use]
pub fn species_key(species: u32, subspecies: u32, examination_type: u32) -> Option<u64> {
    if species >= 1 << 12 || subspecies >= 1 << 16 {
        return None;
    }
    Some(
        (u64::from(examination_type) << 32)
            | (u64::from(species) << 20)
            | (u64::from(subspecies) << 4),
    )
}

/// Species level loot table (class 2001; §4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeciesLevelLootTable {
    /// Entries in stored order.
    pub entries: Vec<SpeciesEntry>,
}

impl SpeciesLevelLootTable {
    /// The object-form map (§9.1, §9.5): key → level table. Entries whose key cannot be
    /// formed are left out; a repeated key keeps the first entry.
    #[must_use]
    pub fn mirror_map(&self) -> BTreeMap<u64, Did> {
        let mut out = BTreeMap::new();
        for e in &self.entries {
            if let Some(k) = e.key() {
                out.entry(k).or_insert(e.level_table);
            }
        }
        out
    }
}

/// One entry of the PVP level loot table (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PvpEntry {
    /// `Agent_Species` key.
    pub species: u32,
    /// `Agent_Class` key (absent: 0).
    pub class: u32,
    /// `LevelBasedLootTable_LevelTable`.
    pub level_table: Did,
}

impl PvpEntry {
    /// The object-form key (§9.5): species × 2¹⁶. Only defined while the class key is 0;
    /// where a non-zero class goes is OPEN (§14), so other entries give `None`.
    #[must_use]
    pub fn key(&self) -> Option<u64> {
        (self.class == 0).then_some(u64::from(self.species) << 16)
    }
}

/// PVP level loot table (class 3671; §4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpLevelLootTable {
    /// Entries in stored order.
    pub entries: Vec<PvpEntry>,
}

/// One level of a world-event level loot table (§4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldEventLevelEntry {
    /// `WorldEventLevelLootTable_Level`; when absent, the dictionary default
    /// ([`EconomyDefaults::world_event_level`]), `None` when there is none.
    pub level: Option<i32>,
    /// Trophy-list template, `None` for 0.
    pub trophy_list: Option<Did>,
}

/// World-event level loot table (class 2127; §4.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEventLevelLootTable {
    /// Entries in stored order.
    pub entries: Vec<WorldEventLevelEntry>,
}

/// One entry of the world-event loot table directory (§4.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEventDirectoryEntry {
    /// The level table.
    pub level_table: Did,
    /// `WorldEvent_AllConditionList` conditions, in stored order.
    pub conditions: Vec<WorldEventCondition>,
}

/// World-event loot table directory (class 2128; §4.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEventLootTableDirectory {
    /// Entries in stored order.
    pub entries: Vec<WorldEventDirectoryEntry>,
}

/// One level of a runic level loot table (§4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunicLevelEntry {
    /// `RunicLevelLootTable_Level`, with the default (1, §4.9) filled in; equal to the
    /// entry's position + 1 in every entry of the observed install (E2).
    pub level: i32,
    /// Whether `RunicLevelLootTable_Level` was stored.
    pub level_stored: bool,
    /// Trophy list, `None` for 0.
    pub trophy_list: Option<Did>,
    /// Runic list, `None` for 0.
    pub runic_list: Option<Did>,
}

/// Runic level loot table (class 2227; §4.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunicLevelLootTable {
    /// Entries in stored order.
    pub entries: Vec<RunicLevelEntry>,
}

/// One entry of a runic list (§4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunicListEntry {
    /// A relic or a runic treasure group.
    pub target: Did,
    /// Drop-frequency key (same mapper as the trophy lists).
    pub drop_frequency: Option<u32>,
}

/// Runic list (class 2229; §4.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunicList {
    /// Entries in stored order.
    pub entries: Vec<RunicListEntry>,
}

/// One pull-count entry of a runic treasure group (§4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullCount {
    /// `RunicTreasureGroup_PullCount`.
    pub count: Option<i32>,
    /// `RunicTreasureGroup_Weight` (absent: 0).
    pub weight: i32,
}

/// Runic treasure group (class 2239; §4.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunicTreasureGroup {
    /// Relics with weights, stored order.
    pub relics: Vec<WeightedRef>,
    /// Pull counts with weights, stored order.
    pub pull_counts: Vec<PullCount>,
}

/// Skirmish treasure lookup (class 2728; §4.10). Null DIDs are `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkirmishTreasureLookup {
    /// Private-encounter template.
    pub encounter: Option<Did>,
    /// Match-parameter record.
    pub match_parameters: Option<Did>,
    /// Trophy-list template.
    pub trophy_list: Option<Did>,
    /// Treasure-list template.
    pub treasure_list: Option<Did>,
    /// Barter trophy list.
    pub barter_trophy_list: Option<Did>,
    /// Reputation trophy list.
    pub reputation_trophy_list: Option<Did>,
}

/// Skirmish custom loot lookup table (class 2729; §4.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkirmishCustomLootLookupTable {
    /// The lookups, stored order.
    pub lookups: Vec<Did>,
}

/// One entry of a hobby reward profile (§4.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HobbyEntry {
    /// The item.
    pub item: Did,
    /// Weight (absent: 0).
    pub weight: i32,
    /// Minimum proficiency.
    pub min_proficiency: Option<i32>,
    /// Maximum proficiency.
    pub max_proficiency: Option<i32>,
    /// Meta score.
    pub meta_score: Option<i32>,
    /// Time-available key.
    pub time_available: Option<u32>,
}

impl HobbyEntry {
    /// The attribute values of the matching class-2154 object (§9.4): min proficiency,
    /// weight, time-available key, meta score, max proficiency, item; absent values as 0,
    /// negative values as their two's-complement words.
    #[must_use]
    pub fn mirror(&self) -> [u32; 6] {
        [
            self.min_proficiency.unwrap_or(0) as u32,
            self.weight as u32,
            self.time_available.unwrap_or(0),
            self.meta_score.unwrap_or(0) as u32,
            self.max_proficiency.unwrap_or(0) as u32,
            self.item.0,
        ]
    }
}

/// Hobby reward profile (class 2153; §4.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HobbyRewardProfile {
    /// `HobbyRewardProfile_AdvancementProgression`.
    pub progression: Option<Did>,
    /// Entries in stored order.
    pub entries: Vec<HobbyEntry>,
}

/// One row of the drop-frequency table (§4.12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropFrequency {
    /// Drop-frequency key.
    pub key: Option<u32>,
    /// The probability as a fraction (the label says percentage; §4.12).
    pub fraction: Option<f32>,
}

/// One row of the money-drop table (§4.12). The unit is OPEN (§11.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoneyDrop {
    /// `…_DropAmountMaximum`.
    pub maximum: Option<i32>,
    /// `…_DropAmountVariance`.
    pub variance: Option<i32>,
}

/// One row of the money-drop modifier table (§4.12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoneyModifier {
    /// Modifier key.
    pub key: Option<u32>,
    /// Multiplier.
    pub multiplier: Option<f32>,
}

/// Loot control singleton (class 1673, WEENIECONTENT label `LootGenControl`; §4.12).
#[derive(Debug, Clone, PartialEq)]
pub struct LootControl {
    /// Drop-frequency table, stored order.
    pub drop_frequencies: Vec<DropFrequency>,
    /// Money-drop table, stored order.
    pub money_drops: Vec<MoneyDrop>,
    /// Money-drop modifier table, stored order.
    pub money_modifiers: Vec<MoneyModifier>,
}

impl LootControl {
    /// The probability of drop-frequency key `key` (§4.12 "drop-frequency resolution"): the
    /// fraction of the first row whose label is `key`.
    #[must_use]
    pub fn drop_fraction(&self, key: u32) -> Option<f32> {
        self.drop_frequencies
            .iter()
            .find(|d| d.key == Some(key))
            .and_then(|d| d.fraction)
    }
}

/// Loot directory control singleton (class 1674, label `LootGenDirectoryControl`; §4.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootDirectoryControl {
    /// Species level loot tables.
    pub species_tables: Vec<Did>,
    /// The PVP level loot table.
    pub pvp_table: Option<Did>,
}

// ---------------------------------------------------------------------------------------------
// Crafting views (§5)
// ---------------------------------------------------------------------------------------------

/// One ingredient (`CraftRecipe_IngredientEntry`; §5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ingredient {
    /// The item.
    pub item: Did,
    /// Quantity.
    pub quantity: Option<i32>,
    /// Consumed (default filled in).
    pub consumed: bool,
    /// Critical-chance bonus (default filled in; optional ingredients only).
    pub crit_bonus: f32,
}

/// One multi-output entry (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiOutput {
    /// The ingredient that selects this output.
    pub ingredient: Did,
    /// Result item.
    pub result: Did,
    /// Critical result item, `None` for 0.
    pub critical_result: Option<Did>,
}

/// One effect of `EffectGenerator_CraftingCriticalSuccessEffectList` (§5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectRef {
    /// `EffectGenerator_EffectID`.
    pub effect: Did,
    /// `EffectGenerator_EffectSpellcraft`.
    pub spellcraft: Option<f32>,
}

/// The two recipe variants (§5.1 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeVariant {
    /// `CraftRecipe_Tier` and `CraftRecipe_Cooldown_MithrilResetCost`.
    SingleTier,
    /// `CraftRecipe_TierArray`, `CraftRecipe_XPAward_Driver` and
    /// `CraftRecipe_LevelMungeOutput`.
    TierArray,
}

/// Recipe (class 1024; §5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// `CraftRecipe_Profession`.
    pub profession: Option<Did>,
    /// `CraftRecipe_Tier`.
    pub tier: Option<u32>,
    /// `CraftRecipe_TierArray` keys; `None` when absent.
    pub tier_array: Option<Vec<u32>>,
    /// `CraftRecipe_Cooldown_MithrilResetCost`.
    pub cooldown_reset_cost: Option<i32>,
    /// `CraftRecipe_XPAward_Driver`.
    pub xp_driver: Option<i32>,
    /// `CraftRecipe_LevelMungeOutput`.
    pub level_munge_output: Option<bool>,
    /// Required ingredients, stored order.
    pub ingredients: Vec<Ingredient>,
    /// Optional ingredients, stored order.
    pub optional_ingredients: Vec<Ingredient>,
    /// `CraftRecipe_IngredientPack`, `None` for absent or 0.
    pub ingredient_pack: Option<Did>,
    /// `CraftRecipe_IngredientPackQuantity`.
    pub ingredient_pack_quantity: Option<i32>,
    /// `CraftRecipe_ResultItem` (null when absent).
    pub result: Did,
    /// `CraftRecipe_ResultItemQuantity`, default filled in.
    pub result_quantity: i32,
    /// `CraftRecipe_CriticalResultItem`, `None` for absent or 0.
    pub critical_result: Option<Did>,
    /// `CraftRecipe_CriticalResultItemQuantity`, default filled in.
    pub critical_quantity: i32,
    /// `CraftRecipe_CriticalSuccessChance` (a fraction).
    pub critical_chance: Option<f32>,
    /// Multi-output entries, stored order.
    pub multi_outputs: Vec<MultiOutput>,
    /// `CraftRecipe_XPReward` key.
    pub xp_reward: Option<u32>,
    /// `CraftRecipe_XPAward_Progression`.
    pub xp_progression: Option<Did>,
    /// `CraftRecipe_XPAward_Progression_DrivingProperty` (a PID).
    pub xp_driving_property: Option<u32>,
    /// `CraftRecipe_UICategory` key.
    pub ui_category: Option<u32>,
    /// `CraftRecipe_ExecutionTime` (unit OPEN, §11.5).
    pub execution_time: Option<f32>,
    /// `CraftRecipe_CooldownDuration` key.
    pub cooldown: Option<u32>,
    /// `CraftRecipe_OneTimeRecipe` (absent: false).
    pub one_time: bool,
    /// `CraftRecipe_RequiredCraftGuild`, `None` for 0.
    pub required_guild: Option<Did>,
    /// `CraftRecipe_RequiredFacilityEffect`, `None` for 0.
    pub required_facility: Option<Did>,
    /// `CraftRecipe_InductionAction`, `None` for 0.
    pub induction_action: Option<Did>,
    /// `CraftRecipe_Animation` key.
    pub animation: Option<u32>,
    /// `CraftRecipe_Name`.
    pub name: Option<StringInfo>,
    /// `CraftRecipe_NameItemOnNormalSuccess` (absent: false).
    pub name_item_on_normal: bool,
    /// `CraftRecipe_NameItemOnCriticalSuccess` (absent: false).
    pub name_item_on_critical: bool,
    /// `CraftRecipe_Field_ResultIcon` (a render surface), `None` for 0.
    pub result_icon: Option<Did>,
    /// `CraftRecipe_Field_CritResultIcon`, `None` for 0.
    pub critical_icon: Option<Did>,
    /// Critical-success effects, stored order.
    pub critical_effects: Vec<EffectRef>,
}

impl Recipe {
    /// The variant by §5.1 rule 1; `None` when the record has properties of both variants
    /// or the defining property of neither.
    #[must_use]
    pub fn variant(&self) -> Option<RecipeVariant> {
        let single = self.tier.is_some() || self.cooldown_reset_cost.is_some();
        let array = self.tier_array.is_some()
            || self.xp_driver.is_some()
            || self.level_munge_output.is_some();
        match (single, array) {
            (true, false) if self.tier.is_some() && self.cooldown_reset_cost.is_some() => {
                Some(RecipeVariant::SingleTier)
            }
            (false, true)
                if self.tier_array.is_some()
                    && self.xp_driver.is_some()
                    && self.level_munge_output.is_some() =>
            {
                Some(RecipeVariant::TierArray)
            }
            _ => None,
        }
    }

    /// The tier list of the object form (§9.1, recipe attribute 5): `[tier]` or the tier
    /// array.
    #[must_use]
    pub fn tiers(&self) -> Vec<u32> {
        match (&self.tier_array, self.tier) {
            (Some(a), _) => a.clone(),
            (None, Some(t)) => vec![t],
            (None, None) => Vec::new(),
        }
    }

    /// Items this recipe produces (§10.2 "crafted by"): result, critical result and the
    /// results of every multi-output entry; non-null, in that order, repeats removed.
    #[must_use]
    pub fn produced_items(&self) -> Vec<Did> {
        let mut out = Vec::new();
        let mut push = |d: Did| {
            if !d.is_null() && !out.contains(&d) {
                out.push(d);
            }
        };
        push(self.result);
        if let Some(c) = self.critical_result {
            push(c);
        }
        for m in &self.multi_outputs {
            push(m.result);
            if let Some(c) = m.critical_result {
                push(c);
            }
        }
        out
    }

    /// Items this recipe consumes or accepts (§10.2 "used in"): required and optional
    /// ingredients, multi-output ingredients and the ingredient pack; non-null, repeats
    /// removed.
    #[must_use]
    pub fn used_items(&self) -> Vec<Did> {
        let mut out = Vec::new();
        let mut push = |d: Did| {
            if !d.is_null() && !out.contains(&d) {
                out.push(d);
            }
        };
        for i in self.ingredients.iter().chain(&self.optional_ingredients) {
            push(i.item);
        }
        for m in &self.multi_outputs {
            push(m.ingredient);
        }
        if let Some(p) = self.ingredient_pack {
            push(p);
        }
        out
    }
}

/// One tier of a profession (§5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfessionTier {
    /// `CraftProfession_Tier` key.
    pub tier: Option<u32>,
    /// `CraftProfession_RecipeArray`, stored order.
    pub recipes: Vec<Did>,
    /// `CraftProfession_ProficiencyXPCap`.
    pub proficiency_xp_cap: Option<i32>,
    /// `CraftProfession_MasteryXPCap`.
    pub mastery_xp_cap: Option<i32>,
    /// `CraftProfession_TierLevelCapGate`.
    pub level_cap_gate: Option<i32>,
    /// `CraftProfession_ProficiencyTitle`.
    pub proficiency_title: Option<Did>,
    /// `CraftProfession_MasteryTitle`.
    pub mastery_title: Option<Did>,
    /// `CraftProfession_CompletedProficiencyLevelCharacteristic`.
    pub completed_proficiency: Option<Did>,
    /// `CraftProfession_CompletedMasteryLevelCharacteristic`.
    pub completed_mastery: Option<Did>,
    /// `CraftProfession_TierOpeningCharacteristic` (absent in tier 1).
    pub tier_opening: Option<Did>,
    /// `CraftProfession_AutoCompleteProficiency`.
    pub auto_complete_proficiency: Option<bool>,
    /// `CraftProfession_AutoCompleteMastery`.
    pub auto_complete_mastery: Option<bool>,
    /// `CraftProfession_ProficiencyTutorialHint` key.
    pub tutorial_hint: Option<u32>,
}

/// The per-character properties that hold a profession's state (§5.2): the twelve
/// `CraftProfession_*_PropertyName` PIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfessionStateProperties {
    /// `…_Enabled_PropertyName`.
    pub enabled: Option<u32>,
    /// `…_ProficiencyLevel_PropertyName`.
    pub proficiency_level: Option<u32>,
    /// `…_ProficiencyXP_PropertyName`.
    pub proficiency_xp: Option<u32>,
    /// `…_MasteryLevel_PropertyName`.
    pub mastery_level: Option<u32>,
    /// `…_MasteryXP_PropertyName`.
    pub mastery_xp: Option<u32>,
    /// `…_CompletedProficiencyLevel_PropertyName`.
    pub completed_proficiency_level: Option<u32>,
    /// `…_CompletedMasteryLevel_PropertyName`.
    pub completed_mastery_level: Option<u32>,
    /// `…_CriticalChanceAddModifier_PropertyName`.
    pub critical_chance_modifier: Option<u32>,
    /// `…_StartTime_PropertyName`.
    pub start_time: Option<u32>,
    /// `…_ExtraRecipeArray_PropertyName`.
    pub extra_recipes: Option<u32>,
    /// `…_OpenLevel_PropertyName`.
    pub open_level: Option<u32>,
    /// `…_TierOpenLevel_PropertyName`.
    pub tier_open_level: Option<u32>,
}

impl ProfessionStateProperties {
    /// The twelve PIDs in the order of §5.2.
    #[must_use]
    pub fn all(&self) -> [Option<u32>; 12] {
        [
            self.enabled,
            self.proficiency_level,
            self.proficiency_xp,
            self.mastery_level,
            self.mastery_xp,
            self.completed_proficiency_level,
            self.completed_mastery_level,
            self.critical_chance_modifier,
            self.start_time,
            self.extra_recipes,
            self.open_level,
            self.tier_open_level,
        ]
    }
}

/// Profession (class 1023; §5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Profession {
    /// Tier entries, stored order.
    pub tiers: Vec<ProfessionTier>,
    /// `CraftProfession_Item`.
    pub item: Option<Did>,
    /// `CraftProfession_RequiredTool` bits.
    pub required_tool: Option<u64>,
    /// `CraftProfession_Characteristic`, `None` for 0.
    pub characteristic: Option<Did>,
    /// The per-character state properties.
    pub state_properties: ProfessionStateProperties,
    /// `CraftProfession_AllowsXPAcceleration` (absent: false).
    pub allows_xp_acceleration: bool,
    /// `CraftProfession_Name`.
    pub name: Option<StringInfo>,
}

impl Profession {
    /// The tool-category keys of the required-tool bitfield: bit *i* is key *i* + 1
    /// (§5.2, `property-system.md` §7.3).
    #[must_use]
    pub fn required_tool_keys(&self) -> Vec<u32> {
        let bits = self.required_tool.unwrap_or(0);
        (0..64u32)
            .filter(|i| bits & (1u64 << i) != 0)
            .map(|i| i + 1)
            .collect()
    }

    /// Every recipe the tier arrays list, in tier order.
    pub fn recipes(&self) -> impl Iterator<Item = Did> + '_ {
        self.tiers.iter().flat_map(|t| t.recipes.iter().copied())
    }
}

/// Vocation (class 1026; §5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Vocation {
    /// `CraftVocation_ProfessionArray`, stored order.
    pub professions: Vec<Did>,
    /// `CraftVocation_ItemArray`, stored order.
    pub items: Vec<Did>,
    /// `CraftVocation_Name`.
    pub name: Option<StringInfo>,
}

/// Craft guild (class 2278; §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftGuild {
    /// `CraftGuild_Profession`.
    pub profession: Option<Did>,
    /// `CraftGuild_Faction`.
    pub faction: Option<Did>,
    /// `CraftGuild_RequiredCraftTier` key.
    pub required_tier: Option<u32>,
    /// `CraftGuild_QuestCategory` key.
    pub quest_category: Option<u32>,
}

/// One row of the craft control's experience table (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XpReward {
    /// `CraftControl_XPRewardEnum` key.
    pub key: Option<u32>,
    /// `CraftControl_XPRewardValue`.
    pub value: Option<i32>,
}

/// One row of the craft control's tier → munge-level table (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierLevel {
    /// `CraftProfession_Tier` key.
    pub tier: Option<u32>,
    /// `Usage_MinLevel`.
    pub min_level: Option<i32>,
}

/// One row of the craft control's change-vocation table (§5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveQuests {
    /// `CraftControl_Profession`.
    pub profession: Option<Did>,
    /// `CraftControl_RemoveQuestsArray` quests, stored order.
    pub quests: Vec<Did>,
}

/// Craft control singleton (class 1665, label `CraftControl`; §5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftControl {
    /// Experience table.
    pub xp_rewards: Vec<XpReward>,
    /// Tier → munge level.
    pub tier_levels: Vec<TierLevel>,
    /// Quests removed on a change of vocation.
    pub remove_quests: Vec<RemoveQuests>,
}

impl CraftControl {
    /// The experience value of key `key` (first matching row).
    #[must_use]
    pub fn xp_value(&self, key: u32) -> Option<i32> {
        self.xp_rewards
            .iter()
            .find(|r| r.key == Some(key))
            .and_then(|r| r.value)
    }
}

/// Craft directory singleton (class 1021, label `CraftDirectory`; §5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftDirectory {
    /// Vocations.
    pub vocations: Vec<Did>,
    /// Professions.
    pub professions: Vec<Did>,
    /// Craft guilds.
    pub guilds: Vec<Did>,
}

// ---------------------------------------------------------------------------------------------
// Vendor, barter, currency and store views (§6–§8)
// ---------------------------------------------------------------------------------------------

/// One cost of a barter list (`Barter_ItemStruct`; §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarterCost {
    /// `Barter_Item`; DID 0 when absent (§6.1).
    pub item: Did,
    /// `Barter_ItemQuantity`.
    pub quantity: Option<i32>,
    /// `Barter_ItemQuantity_LookupTable`, `None` for absent or 0.
    pub lookup_table: Option<Did>,
}

/// The reward of a barter list (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarterReward {
    /// `Barter_RewardItemStruct`.
    Item {
        /// `Barter_Item`.
        item: Did,
        /// `Barter_ItemQuantity`.
        quantity: Option<i32>,
        /// `Barter_ItemUsesMinUsageLevelForCost` (absent: false).
        uses_min_usage_level: bool,
    },
    /// `Barter_RewardReputationStruct`.
    Reputation {
        /// `Barter_ReputationFaction`.
        faction: Did,
        /// `Barter_ReputationRewardTier` key.
        tier: Option<u32>,
    },
}

/// One element of a barter list, in stored order (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarterElement {
    /// A cost.
    Cost(BarterCost),
    /// A reward.
    Reward(BarterReward),
    /// `Barter_SoundCategory` key.
    SoundCategory(u32),
    /// An element of another kind, by PID.
    Other(u32),
}

/// The list shapes of §6.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BarterShape {
    /// 1–3 costs, then a reward item.
    Item {
        /// Number of costs (1–3).
        costs: usize,
    },
    /// 1–3 costs, then a reputation reward, then optionally a sound category.
    Reputation {
        /// Number of costs (1–3).
        costs: usize,
        /// Whether a sound category follows.
        sound: bool,
    },
}

/// Most cost structs a barter list holds (§6.1, and the fixed layout of §9.3).
pub const MAX_BARTER_COSTS: usize = 3;

/// One trade (`Barter_ItemList`; §6.1): its elements in stored order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarterList {
    /// Elements in stored order.
    pub elements: Vec<BarterElement>,
}

impl BarterList {
    /// The costs, stored order.
    pub fn costs(&self) -> impl Iterator<Item = &BarterCost> {
        self.elements.iter().filter_map(|e| match e {
            BarterElement::Cost(c) => Some(c),
            _ => None,
        })
    }

    /// The reward, if the list has exactly one.
    #[must_use]
    pub fn reward(&self) -> Option<&BarterReward> {
        let mut it = self.elements.iter().filter_map(|e| match e {
            BarterElement::Reward(r) => Some(r),
            _ => None,
        });
        let first = it.next()?;
        it.next().is_none().then_some(first)
    }

    /// The sound-category key, if the list has exactly one.
    #[must_use]
    pub fn sound_category(&self) -> Option<u32> {
        let mut it = self.elements.iter().filter_map(|e| match e {
            BarterElement::SoundCategory(k) => Some(*k),
            _ => None,
        });
        let first = it.next()?;
        it.next().is_none().then_some(first)
    }

    /// The shape of the list (§6.1), or `None` when it matches none (E-I9): 1–3 costs, then
    /// one reward, then at most one sound category, in that stored order. A sound category
    /// after a reward item is not one of the observed shapes.
    #[must_use]
    pub fn shape(&self) -> Option<BarterShape> {
        let costs = self
            .elements
            .iter()
            .take_while(|e| matches!(e, BarterElement::Cost(_)))
            .count();
        if !(1..=MAX_BARTER_COSTS).contains(&costs) {
            return None;
        }
        match self.elements.get(costs..)? {
            [BarterElement::Reward(BarterReward::Item { .. })] => Some(BarterShape::Item { costs }),
            [BarterElement::Reward(BarterReward::Reputation { .. })] => {
                Some(BarterShape::Reputation {
                    costs,
                    sound: false,
                })
            }
            [
                BarterElement::Reward(BarterReward::Reputation { .. }),
                BarterElement::SoundCategory(_),
            ] => Some(BarterShape::Reputation { costs, sound: true }),
            _ => None,
        }
    }

    /// The 49-byte barter entry (class 1942) the object form stores for this list (§9.3), or
    /// `None` when the list has no single reward or more than three costs.
    ///
    /// `quantity` gives the stored quantity of each cost: normally
    /// [`BarterCost::quantity`], and for a look-up cost the value of §6.2. A cost whose
    /// quantity it cannot give (`None`) makes the whole entry `None`.
    pub fn expected_entry(
        &self,
        profile: Did,
        mut quantity: impl FnMut(&BarterCost) -> Option<u32>,
    ) -> Option<BarterEntry> {
        let mut entry = BarterEntry {
            profile,
            ..BarterEntry::default()
        };
        match *self.reward()? {
            BarterReward::Item {
                item,
                quantity,
                uses_min_usage_level,
            } => {
                entry.reward_item = item;
                entry.reward_quantity = quantity.unwrap_or(0) as u32;
                entry.uses_min_usage_level = uses_min_usage_level;
            }
            BarterReward::Reputation { faction, tier } => {
                entry.reputation_faction = faction;
                entry.reputation_tier = tier.unwrap_or(0);
            }
        }
        for (i, cost) in self.costs().enumerate() {
            let slot = entry.costs.get_mut(i)?;
            *slot = (cost.item, quantity(cost)?);
        }
        entry.sound_category = self.sound_category().unwrap_or(0);
        Some(entry)
    }
}

/// Barter profile (class 1943; §6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct BarterProfile {
    /// The trades, stored order.
    pub lists: Vec<BarterList>,
    /// `Barter_Profile_Name`.
    pub name: Option<StringInfo>,
    /// `Barter_ItemMungeLevelOverride`, default filled in (role OPEN, §6.2).
    pub munge_level_override: i32,
    /// `DefaultPermissionBlobStruct`: the profile's usage requirements, undecoded.
    pub permissions: Option<PropertyStruct>,
}

/// A float progression as the look-up rule reads it (§6.2 step 3): the minimum index value
/// and the value array. The progression format itself belongs to the progression spec.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatProgression {
    /// `Progression_MinimumIndexValue`.
    pub minimum_index: i64,
    /// `FloatProgression_Array` values (`FloatProgression_FloatValue`), stored order.
    pub values: Vec<f64>,
}

impl FloatProgression {
    /// The value at index value `index_value` (§6.2 step 4): position = index value −
    /// minimum, clamped to the first and last element. `None` for an empty progression.
    #[must_use]
    pub fn value_at(&self, index_value: i64) -> Option<f64> {
        let last = self.values.len().checked_sub(1)?;
        let pos = index_value.saturating_sub(self.minimum_index).max(0);
        let pos = usize::try_from(pos).unwrap_or(usize::MAX).min(last);
        self.values.get(pos).copied()
    }
}

/// The look-up quantity of §6.2: the progression's value at the reward item's level, when it
/// is a whole number that fits a u32. `None` otherwise (the spec has no rounding rule).
#[must_use]
pub fn lookup_quantity(progression: &FloatProgression, level: i64) -> Option<u32> {
    let v = progression.value_at(level)?;
    if v.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&v) {
        return None;
    }
    Some(v as u32)
}

/// The vendor inventory (class 1619; §6.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VendorInventory {
    /// `VendorInventory_Items`, stored order (empty when the array is absent).
    pub items: Vec<Did>,
}

/// The vendor properties of an NPC record (§6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct NpcVendor {
    /// `Vendor_InventoryList`, stored order.
    pub inventories: Vec<Did>,
    /// `Vendor_ItemList`, stored order (derived from the inventories, §6.3).
    pub item_list: Vec<Did>,
    /// `Vendor_SellModifier`, default filled in.
    pub sell_modifier: f32,
    /// `Vendor_SellsWearableItems` (absent: false).
    pub sells_wearable_items: bool,
    /// `Vendor_BuysItems` (absent: false).
    pub buys_items: bool,
    /// `Vendor_DiscountList` PIDs.
    pub discounts: Vec<u32>,
    /// `Barter_ProfileArray`.
    pub barter_profiles: Vec<Did>,
    /// `Barter_Profile_UseTabs` (absent: false).
    pub barter_use_tabs: bool,
    /// `Craft_OfferedProfessionArray`.
    pub offered_professions: Vec<Did>,
    /// `Craft_OfferedCraftGuildArray`.
    pub offered_guilds: Vec<Did>,
    /// `Craft_OfferedVocationArray`.
    pub offered_vocations: Vec<Did>,
}

impl NpcVendor {
    /// Checks E-I11: the item set of `Vendor_ItemList` equals the union of the items of the
    /// inventories, without duplicates. `inventory` looks an inventory up; an inventory it
    /// cannot find makes the check fail.
    pub fn item_list_matches<'a>(
        &self,
        mut inventory: impl FnMut(Did) -> Option<&'a VendorInventory>,
    ) -> bool {
        let mut union = BTreeSet::new();
        for &inv in &self.inventories {
            let Some(inv) = inventory(inv) else {
                return false;
            };
            union.extend(inv.items.iter().copied());
        }
        let listed: BTreeSet<Did> = self.item_list.iter().copied().collect();
        listed.len() == self.item_list.len() && listed == union
    }
}

/// Perk vendor (class 1796; §6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerkVendor {
    /// `PerkVendor_InventoryList` perks, stored order.
    pub perks: Vec<Did>,
}

/// Currency denomination (class 1028; §7).
#[derive(Debug, Clone, PartialEq)]
pub struct Currency {
    /// `Currency_Value`: one unit in units of the smallest denomination.
    pub value: Option<i32>,
    /// `Currency_ObjectDID`: the coin item.
    pub object: Option<Did>,
    /// `Currency_Name`.
    pub name: Option<StringInfo>,
    /// `Currency_Name_Singular`.
    pub singular: Option<StringInfo>,
    /// `Currency_Name_Plural`.
    pub plural: Option<StringInfo>,
}

/// Currency directory (class 1029; §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyDirectory {
    /// `Currency_CurrencyList`, stored order.
    pub currencies: Vec<Did>,
}

/// Currency control (class 1996; §7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrencyControl {
    /// `Currency_Control_GarnishmentPercentage` (a fraction).
    pub garnishment: Option<f32>,
    /// `Currency_Control_GoldCapWebstoreItem`.
    pub gold_cap_item: Option<Did>,
}

/// The item links of a store record (§8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoreLinks {
    /// `WebStoreInventoryItem_DataID`, `None` for absent or 0.
    pub inventory_item: Option<Did>,
    /// The `WebStoreAccountItem_DataID` elements of `WebStoreAccountItem_BundleItemArray`,
    /// stored order, null DIDs left out.
    pub bundle: Vec<Did>,
}

// ---------------------------------------------------------------------------------------------
// Records and the reader
// ---------------------------------------------------------------------------------------------

/// A decoded economy record.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum EconomyRecord {
    TrophyList(TrophyList),
    TreasureGroupProfile(TreasureGroupProfile),
    TreasureList(TreasureList),
    WeightedTreasureTable(WeightedTreasureTable),
    FilteredTreasureList(FilteredTreasureList),
    LevelBasedLootTable(LevelBasedLootTable),
    SpeciesLevelLootTable(SpeciesLevelLootTable),
    PvpLevelLootTable(PvpLevelLootTable),
    WorldEventLevelLootTable(WorldEventLevelLootTable),
    WorldEventLootTableDirectory(WorldEventLootTableDirectory),
    RunicLevelLootTable(RunicLevelLootTable),
    RunicList(RunicList),
    RunicTreasureGroup(RunicTreasureGroup),
    SkirmishTreasureLookup(SkirmishTreasureLookup),
    SkirmishCustomLootLookupTable(SkirmishCustomLootLookupTable),
    HobbyRewardProfile(HobbyRewardProfile),
    LootControl(LootControl),
    LootDirectoryControl(LootDirectoryControl),
    Recipe(Box<Recipe>),
    Profession(Profession),
    Vocation(Vocation),
    CraftGuild(CraftGuild),
    CraftControl(CraftControl),
    CraftDirectory(CraftDirectory),
    BarterProfile(BarterProfile),
    VendorInventory(VendorInventory),
    PerkVendor(PerkVendor),
    Currency(Currency),
    CurrencyDirectory(CurrencyDirectory),
    CurrencyControl(CurrencyControl),
}

impl EconomyRecord {
    /// The record's kind.
    #[must_use]
    pub fn kind(&self) -> EconomyKind {
        match self {
            Self::TrophyList(_) => EconomyKind::TrophyList,
            Self::TreasureGroupProfile(_) => EconomyKind::TreasureGroupProfile,
            Self::TreasureList(_) => EconomyKind::TreasureList,
            Self::WeightedTreasureTable(_) => EconomyKind::WeightedTreasureTable,
            Self::FilteredTreasureList(_) => EconomyKind::FilteredTreasureList,
            Self::LevelBasedLootTable(_) => EconomyKind::LevelBasedLootTable,
            Self::SpeciesLevelLootTable(_) => EconomyKind::SpeciesLevelLootTable,
            Self::PvpLevelLootTable(_) => EconomyKind::PvpLevelLootTable,
            Self::WorldEventLevelLootTable(_) => EconomyKind::WorldEventLevelLootTable,
            Self::WorldEventLootTableDirectory(_) => EconomyKind::WorldEventLootTableDirectory,
            Self::RunicLevelLootTable(_) => EconomyKind::RunicLevelLootTable,
            Self::RunicList(_) => EconomyKind::RunicList,
            Self::RunicTreasureGroup(_) => EconomyKind::RunicTreasureGroup,
            Self::SkirmishTreasureLookup(_) => EconomyKind::SkirmishTreasureLookup,
            Self::SkirmishCustomLootLookupTable(_) => EconomyKind::SkirmishCustomLootLookupTable,
            Self::HobbyRewardProfile(_) => EconomyKind::HobbyRewardProfile,
            Self::LootControl(_) => EconomyKind::LootControl,
            Self::LootDirectoryControl(_) => EconomyKind::LootDirectoryControl,
            Self::Recipe(_) => EconomyKind::Recipe,
            Self::Profession(_) => EconomyKind::Profession,
            Self::Vocation(_) => EconomyKind::Vocation,
            Self::CraftGuild(_) => EconomyKind::CraftGuild,
            Self::CraftControl(_) => EconomyKind::CraftControl,
            Self::CraftDirectory(_) => EconomyKind::CraftDirectory,
            Self::BarterProfile(_) => EconomyKind::BarterProfile,
            Self::VendorInventory(_) => EconomyKind::VendorInventory,
            Self::PerkVendor(_) => EconomyKind::PerkVendor,
            Self::Currency(_) => EconomyKind::Currency,
            Self::CurrencyDirectory(_) => EconomyKind::CurrencyDirectory,
            Self::CurrencyControl(_) => EconomyKind::CurrencyControl,
        }
    }
}

/// Reads economy records from property sets: the labels and defaults of one dictionary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EconomyReader {
    /// The resolved labels.
    pub pids: EconomyPids,
    /// The defaults of absent fields.
    pub defaults: EconomyDefaults,
}

impl EconomyReader {
    /// A reader for `dict`: labels and defaults resolved from it.
    #[must_use]
    pub fn new<D: PropertyNames + PropertyDefaults + ?Sized>(dict: &D) -> Self {
        let pids = EconomyPids::resolve(dict);
        let defaults = EconomyDefaults::resolve(dict, &pids);
        Self { pids, defaults }
    }

    /// A reader from resolved labels and explicit defaults (synthetic tests).
    #[must_use]
    pub const fn from_parts(pids: EconomyPids, defaults: EconomyDefaults) -> Self {
        Self { pids, defaults }
    }

    /// The kind whose characteristic property `set` holds (§3.4). A set holding several is
    /// given the first kind in [`EconomyKind::ALL`] order (none does in the observed data).
    #[must_use]
    pub fn kind<A: PropertyAccess + ?Sized>(&self, set: &A) -> Option<EconomyKind> {
        EconomyKind::ALL
            .into_iter()
            .find(|&k| set.has(k.characteristic(&self.pids)))
    }

    /// Classifies and decodes `set`; `Ok(None)` when it holds no characteristic property.
    pub fn decode<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Option<EconomyRecord>> {
        self.kind(set).map(|k| self.decode_as(k, set)).transpose()
    }

    /// Decodes `set` as a record of `kind` (for records known by class, such as the empty
    /// vendor inventories, §3.4).
    pub fn decode_as<A: PropertyAccess + ?Sized>(
        &self,
        kind: EconomyKind,
        set: &A,
    ) -> Result<EconomyRecord> {
        Ok(match kind {
            EconomyKind::TrophyList => EconomyRecord::TrophyList(self.trophy_list(set)?),
            EconomyKind::TreasureGroupProfile => {
                EconomyRecord::TreasureGroupProfile(self.treasure_group_profile(set)?)
            }
            EconomyKind::TreasureList => EconomyRecord::TreasureList(self.treasure_list(set)?),
            EconomyKind::WeightedTreasureTable => {
                EconomyRecord::WeightedTreasureTable(self.weighted_treasure_table(set)?)
            }
            EconomyKind::FilteredTreasureList => {
                EconomyRecord::FilteredTreasureList(self.filtered_treasure_list(set)?)
            }
            EconomyKind::LevelBasedLootTable => {
                EconomyRecord::LevelBasedLootTable(self.level_based_loot_table(set)?)
            }
            EconomyKind::SpeciesLevelLootTable => {
                EconomyRecord::SpeciesLevelLootTable(self.species_level_loot_table(set)?)
            }
            EconomyKind::PvpLevelLootTable => {
                EconomyRecord::PvpLevelLootTable(self.pvp_level_loot_table(set)?)
            }
            EconomyKind::WorldEventLevelLootTable => {
                EconomyRecord::WorldEventLevelLootTable(self.world_event_level_loot_table(set)?)
            }
            EconomyKind::WorldEventLootTableDirectory => {
                EconomyRecord::WorldEventLootTableDirectory(self.world_event_directory(set)?)
            }
            EconomyKind::RunicLevelLootTable => {
                EconomyRecord::RunicLevelLootTable(self.runic_level_loot_table(set)?)
            }
            EconomyKind::RunicList => EconomyRecord::RunicList(self.runic_list(set)?),
            EconomyKind::RunicTreasureGroup => {
                EconomyRecord::RunicTreasureGroup(self.runic_treasure_group(set)?)
            }
            EconomyKind::SkirmishTreasureLookup => {
                EconomyRecord::SkirmishTreasureLookup(self.skirmish_treasure_lookup(set)?)
            }
            EconomyKind::SkirmishCustomLootLookupTable => {
                EconomyRecord::SkirmishCustomLootLookupTable(self.skirmish_custom_table(set)?)
            }
            EconomyKind::HobbyRewardProfile => {
                EconomyRecord::HobbyRewardProfile(self.hobby_reward_profile(set)?)
            }
            EconomyKind::LootControl => EconomyRecord::LootControl(self.loot_control(set)?),
            EconomyKind::LootDirectoryControl => {
                EconomyRecord::LootDirectoryControl(self.loot_directory_control(set)?)
            }
            EconomyKind::Recipe => EconomyRecord::Recipe(Box::new(self.recipe(set)?)),
            EconomyKind::Profession => EconomyRecord::Profession(self.profession(set)?),
            EconomyKind::Vocation => EconomyRecord::Vocation(self.vocation(set)?),
            EconomyKind::CraftGuild => EconomyRecord::CraftGuild(self.craft_guild(set)?),
            EconomyKind::CraftControl => EconomyRecord::CraftControl(self.craft_control(set)?),
            EconomyKind::CraftDirectory => {
                EconomyRecord::CraftDirectory(self.craft_directory(set)?)
            }
            EconomyKind::BarterProfile => EconomyRecord::BarterProfile(self.barter_profile(set)?),
            EconomyKind::VendorInventory => {
                EconomyRecord::VendorInventory(self.vendor_inventory(set)?)
            }
            EconomyKind::PerkVendor => EconomyRecord::PerkVendor(self.perk_vendor(set)?),
            EconomyKind::Currency => EconomyRecord::Currency(self.currency(set)?),
            EconomyKind::CurrencyDirectory => {
                EconomyRecord::CurrencyDirectory(self.currency_directory(set)?)
            }
            EconomyKind::CurrencyControl => {
                EconomyRecord::CurrencyControl(self.currency_control(set)?)
            }
        })
    }

    // --- Loot ---------------------------------------------------------------------------

    /// Trophy list (§4.1).
    pub fn trophy_list<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<TrophyList> {
        const WHAT: &str = "trophy list";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.trophy_list, WHAT)?
            .into_iter()
            .map(|e| {
                let quantity = opt_int(e, p.trophy_quantity, WHAT)?;
                Ok(TrophyEntry {
                    target: did_or_null(e, p.trophy_item_or_profile, WHAT)?,
                    drop_frequency: opt_enum(e, p.trophy_drop_frequency, WHAT)?,
                    quantity: quantity.unwrap_or(self.defaults.trophy_quantity),
                    quantity_stored: quantity.is_some(),
                    group_drop: opt_bool(e, p.trophy_group_drop, WHAT)?.unwrap_or(false),
                })
            })
            .collect::<Result<_>>()?;
        Ok(TrophyList {
            entries,
            description: opt_text(set, p.description, WHAT)?,
            icon: nonnull_did(set, p.icon_override, WHAT)?,
            max_quantity: opt_int(set, p.inventory_max_quantity, WHAT)?,
        })
    }

    /// Treasure group profile (§4.2).
    pub fn treasure_group_profile<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<TreasureGroupProfile> {
        const WHAT: &str = "treasure group profile";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.tgp_table, WHAT)?
            .into_iter()
            .map(|e| {
                let quantity = opt_int(e, p.tgp_quantity, WHAT)?;
                Ok(ProfileEntry {
                    item: did_or_null(e, p.tgp_item, WHAT)?,
                    weight: opt_int(e, p.tgp_weight, WHAT)?.unwrap_or(0),
                    quantity: quantity.unwrap_or(self.defaults.profile_quantity),
                    quantity_stored: quantity.is_some(),
                })
            })
            .collect::<Result<_>>()?;
        Ok(TreasureGroupProfile { entries })
    }

    fn weighted_refs<A: PropertyAccess + ?Sized>(
        set: &A,
        array: Pid,
        target: Pid,
        weight: Pid,
        what: &'static str,
    ) -> Result<Vec<WeightedRef>> {
        struct_elements(set, array, what)?
            .into_iter()
            .map(|e| {
                Ok(WeightedRef {
                    target: did_or_null(e, target, what)?,
                    weight: opt_int(e, weight, what)?.unwrap_or(0),
                })
            })
            .collect()
    }

    /// Treasure list (§4.3).
    pub fn treasure_list<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<TreasureList> {
        let p = &self.pids.loot;
        Ok(TreasureList {
            entries: Self::weighted_refs(
                set,
                p.treasure_list,
                p.treasure_list_profile,
                p.treasure_list_weight,
                "treasure list",
            )?,
        })
    }

    /// Weighted treasure table (§4.4).
    pub fn weighted_treasure_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<WeightedTreasureTable> {
        let p = &self.pids.loot;
        Ok(WeightedTreasureTable {
            entries: Self::weighted_refs(
                set,
                p.weighted_table,
                p.weighted_trophy_list,
                p.weighted_weight,
                "weighted treasure table",
            )?,
        })
    }

    fn property_test(e: &PropertyEntry) -> PropertyTest {
        PropertyTest {
            pid: e.pid,
            value: e.value.clone(),
        }
    }

    fn world_event_condition(
        &self,
        s: &PropertyStruct,
        what: &'static str,
    ) -> Result<WorldEventCondition> {
        let p = &self.pids.loot;
        let operator = opt_enum(s, p.world_event_operator, what)?;
        // An int (type 15) wherever the label occurs (E1).
        let value = opt_int(s, p.world_event_value, what)?;
        Ok(WorldEventCondition {
            world_event: did_or_null(s, p.world_event, what)?,
            operator,
            value,
        })
    }

    /// Filtered treasure list (§4.5).
    pub fn filtered_treasure_list<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<FilteredTreasureList> {
        const WHAT: &str = "filtered treasure list";
        let p = &self.pids.loot;
        let mut entries = Vec::new();
        for e in struct_elements(set, p.filtered_array, WHAT)? {
            let mut filters = Vec::new();
            for f in elements(e, p.filter_array, WHAT)? {
                filters.push(self.entity_filter(f, WHAT)?);
            }
            entries.push(FilteredEntry {
                table: did_or_null(e, p.filtered_entry, WHAT)?,
                filters,
            });
        }
        Ok(FilteredTreasureList { entries })
    }

    fn entity_filter(&self, f: &PropertyEntry, what: &'static str) -> Result<EntityFilter> {
        let p = &self.pids.loot;
        if p.filter_property_set.is(f.pid) {
            let a = f
                .value
                .as_array()
                .ok_or_else(|| mismatch(what, p.filter_property_set, "array", &f.value))?;
            return Ok(EntityFilter::PropertySet(
                a.elements.iter().map(Self::property_test).collect(),
            ));
        }
        if p.filter_property_range.is(f.pid) {
            let s = f
                .value
                .as_struct()
                .ok_or_else(|| mismatch(what, p.filter_property_range, "struct", &f.value))?;
            // Each bound is a one-element array (§4.5); another length fails the record.
            let bound = |pid: Pid| -> Result<Option<PropertyTest>> {
                let Some(v) = s.get_pid(pid) else {
                    return Ok(None);
                };
                match v.as_array().map(|a| a.elements.as_slice()) {
                    Some([one]) => Ok(Some(Self::property_test(one))),
                    _ => Err(mismatch(what, pid, "one-element array", v)),
                }
            };
            return Ok(EntityFilter::PropertyRange {
                min: bound(p.filter_range_min)?,
                max: bound(p.filter_range_max)?,
            });
        }
        if p.filter_world_event.is(f.pid) {
            let s = f
                .value
                .as_struct()
                .ok_or_else(|| mismatch(what, p.filter_world_event, "struct", &f.value))?;
            let condition = opt_struct(s, p.world_event_condition, what)?
                .map(|c| self.world_event_condition(c, what))
                .transpose()?;
            return Ok(EntityFilter::WorldEvent(condition));
        }
        Ok(EntityFilter::Other { pid: f.pid })
    }

    /// Level-based loot table (§4.6).
    pub fn level_based_loot_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<LevelBasedLootTable> {
        const WHAT: &str = "level-based loot table";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.level_table, WHAT)?
            .into_iter()
            .map(|e| {
                let level = opt_int(e, p.level_table_level, WHAT)?;
                Ok(LevelLootEntry {
                    level: level.unwrap_or(self.defaults.level_table_level),
                    level_stored: level.is_some(),
                    trophy_list: nonnull_did(e, p.level_table_trophy_list, WHAT)?,
                    treasure_list: nonnull_did(e, p.level_table_treasure, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(LevelBasedLootTable { entries })
    }

    /// Species level loot table (§4.7).
    pub fn species_level_loot_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<SpeciesLevelLootTable> {
        const WHAT: &str = "species level loot table";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.species_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(SpeciesEntry {
                    species: opt_enum(e, p.agent_species, WHAT)?.unwrap_or(0),
                    subspecies: opt_enum(e, p.agent_subspecies, WHAT)?.unwrap_or(0),
                    examination_type: opt_enum(e, p.examination_type, WHAT)?.unwrap_or(0),
                    level_table: did_or_null(e, p.level_table_ref, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(SpeciesLevelLootTable { entries })
    }

    /// PVP level loot table (§4.7).
    pub fn pvp_level_loot_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<PvpLevelLootTable> {
        const WHAT: &str = "PVP level loot table";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.pvp_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(PvpEntry {
                    species: opt_enum(e, p.agent_species, WHAT)?.unwrap_or(0),
                    class: opt_enum(e, p.agent_class, WHAT)?.unwrap_or(0),
                    level_table: did_or_null(e, p.level_table_ref, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(PvpLevelLootTable { entries })
    }

    /// World-event level loot table (§4.8).
    pub fn world_event_level_loot_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<WorldEventLevelLootTable> {
        const WHAT: &str = "world-event level loot table";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.we_level_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(WorldEventLevelEntry {
                    level: opt_int(e, p.we_level, WHAT)?.or(self.defaults.world_event_level),
                    trophy_list: nonnull_did(e, p.we_trophy_template, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(WorldEventLevelLootTable { entries })
    }

    /// World-event loot table directory (§4.8).
    pub fn world_event_directory<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<WorldEventLootTableDirectory> {
        const WHAT: &str = "world-event loot table directory";
        let p = &self.pids.loot;
        let mut entries = Vec::new();
        for e in struct_elements(set, p.we_directory, WHAT)? {
            let mut conditions = Vec::new();
            for c in elements(e, p.world_event_all_conditions, WHAT)? {
                let s = c.value.as_struct().ok_or_else(|| {
                    mismatch(
                        WHAT,
                        p.world_event_all_conditions,
                        "struct elements",
                        &c.value,
                    )
                })?;
                conditions.push(self.world_event_condition(s, WHAT)?);
            }
            entries.push(WorldEventDirectoryEntry {
                level_table: did_or_null(e, p.we_directory_level_table, WHAT)?,
                conditions,
            });
        }
        Ok(WorldEventLootTableDirectory { entries })
    }

    /// Runic level loot table (§4.9): the stored `RunicLevelLootTable_Level` of each entry
    /// (E2).
    pub fn runic_level_loot_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<RunicLevelLootTable> {
        const WHAT: &str = "runic level loot table";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.runic_level_table, WHAT)?
            .into_iter()
            .map(|e| {
                let level = opt_int(e, p.runic_level_level, WHAT)?;
                Ok(RunicLevelEntry {
                    level: level.unwrap_or(self.defaults.runic_level),
                    level_stored: level.is_some(),
                    trophy_list: nonnull_did(e, p.runic_level_trophy, WHAT)?,
                    runic_list: nonnull_did(e, p.runic_level_runic, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(RunicLevelLootTable { entries })
    }

    /// Runic list (§4.9).
    pub fn runic_list<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<RunicList> {
        const WHAT: &str = "runic list";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.runic_list, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(RunicListEntry {
                    target: did_or_null(e, p.runic_list_target, WHAT)?,
                    drop_frequency: opt_enum(e, p.runic_list_drop_frequency, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(RunicList { entries })
    }

    /// Runic treasure group (§4.9).
    pub fn runic_treasure_group<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<RunicTreasureGroup> {
        const WHAT: &str = "runic treasure group";
        let p = &self.pids.loot;
        let relics = Self::weighted_refs(
            set,
            p.runic_group,
            p.runic_group_relic,
            p.runic_group_weight,
            WHAT,
        )?;
        let pull_counts = struct_elements(set, p.runic_pull_counts, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(PullCount {
                    count: opt_int(e, p.runic_pull_count, WHAT)?,
                    weight: opt_int(e, p.runic_group_weight, WHAT)?.unwrap_or(0),
                })
            })
            .collect::<Result<_>>()?;
        Ok(RunicTreasureGroup {
            relics,
            pull_counts,
        })
    }

    /// Skirmish treasure lookup (§4.10).
    pub fn skirmish_treasure_lookup<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<SkirmishTreasureLookup> {
        const WHAT: &str = "skirmish treasure lookup";
        let p = &self.pids.loot;
        Ok(SkirmishTreasureLookup {
            encounter: nonnull_did(set, p.skirmish_encounter, WHAT)?,
            match_parameters: nonnull_did(set, p.skirmish_match_parameters, WHAT)?,
            trophy_list: nonnull_did(set, p.skirmish_trophy, WHAT)?,
            treasure_list: nonnull_did(set, p.skirmish_treasure, WHAT)?,
            barter_trophy_list: nonnull_did(set, p.skirmish_barter, WHAT)?,
            reputation_trophy_list: nonnull_did(set, p.skirmish_reputation, WHAT)?,
        })
    }

    /// Skirmish custom loot lookup table (§4.10).
    pub fn skirmish_custom_table<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<SkirmishCustomLootLookupTable> {
        Ok(SkirmishCustomLootLookupTable {
            lookups: did_elements(
                set,
                self.pids.loot.skirmish_custom_table,
                "skirmish custom loot lookup table",
            )?,
        })
    }

    /// Hobby reward profile (§4.11).
    pub fn hobby_reward_profile<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<HobbyRewardProfile> {
        const WHAT: &str = "hobby reward profile";
        let p = &self.pids.loot;
        let entries = struct_elements(set, p.hobby_items, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(HobbyEntry {
                    item: did_or_null(e, p.hobby_item, WHAT)?,
                    weight: opt_int(e, p.hobby_weight, WHAT)?.unwrap_or(0),
                    min_proficiency: opt_int(e, p.hobby_min, WHAT)?,
                    max_proficiency: opt_int(e, p.hobby_max, WHAT)?,
                    meta_score: opt_int(e, p.hobby_meta_score, WHAT)?,
                    time_available: opt_enum(e, p.hobby_time_available, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(HobbyRewardProfile {
            progression: nonnull_did(set, p.hobby_progression, WHAT)?,
            entries,
        })
    }

    /// Loot control singleton (§4.12).
    pub fn loot_control<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<LootControl> {
        const WHAT: &str = "loot control";
        let p = &self.pids.loot;
        let drop_frequencies = struct_elements(set, p.control_drop_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(DropFrequency {
                    key: opt_enum(e, p.control_drop_label, WHAT)?,
                    fraction: opt_float(e, p.control_drop_fraction, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let money_drops = struct_elements(set, p.control_money_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(MoneyDrop {
                    maximum: opt_int(e, p.control_money_maximum, WHAT)?,
                    variance: opt_int(e, p.control_money_variance, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let money_modifiers = struct_elements(set, p.control_modifier_table, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(MoneyModifier {
                    key: opt_enum(e, p.control_modifier_label, WHAT)?,
                    multiplier: opt_float(e, p.control_modifier_multiplier, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(LootControl {
            drop_frequencies,
            money_drops,
            money_modifiers,
        })
    }

    /// Loot directory control singleton (§4.12).
    pub fn loot_directory_control<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<LootDirectoryControl> {
        const WHAT: &str = "loot directory control";
        let p = &self.pids.loot;
        Ok(LootDirectoryControl {
            species_tables: did_elements(set, p.directory_species_tables, WHAT)?,
            pvp_table: nonnull_did(set, p.directory_pvp_table, WHAT)?,
        })
    }

    // --- Crafting -----------------------------------------------------------------------

    fn ingredients<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
        array: Pid,
        what: &'static str,
    ) -> Result<Vec<Ingredient>> {
        let p = &self.pids.craft;
        struct_elements(set, array, what)?
            .into_iter()
            .map(|e| {
                Ok(Ingredient {
                    item: did_or_null(e, p.recipe_ingredient, what)?,
                    quantity: opt_int(e, p.recipe_ingredient_quantity, what)?,
                    consumed: opt_bool(e, p.recipe_ingredient_consumed, what)?
                        .unwrap_or(self.defaults.ingredient_consumed),
                    crit_bonus: opt_float(e, p.recipe_crit_bonus, what)?
                        .unwrap_or(self.defaults.crit_bonus),
                })
            })
            .collect()
    }

    /// Recipe (§5.1).
    pub fn recipe<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Recipe> {
        const WHAT: &str = "recipe";
        let p = &self.pids.craft;
        let tier_array = match set.get_pid(p.recipe_tier_array) {
            None => None,
            Some(_) => Some(enum_elements(set, p.recipe_tier_array, WHAT)?),
        };
        let multi_outputs = struct_elements(set, p.recipe_multi_output, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(MultiOutput {
                    ingredient: did_or_null(e, p.recipe_ingredient, WHAT)?,
                    result: did_or_null(e, p.recipe_result, WHAT)?,
                    critical_result: nonnull_did(e, p.recipe_critical_result, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let critical_effects = struct_elements(set, p.critical_effects, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(EffectRef {
                    effect: did_or_null(e, p.effect_id, WHAT)?,
                    spellcraft: opt_float(e, p.effect_spellcraft, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Recipe {
            profession: nonnull_did(set, p.recipe_profession, WHAT)?,
            tier: opt_enum(set, p.recipe_tier, WHAT)?,
            tier_array,
            cooldown_reset_cost: opt_int(set, p.recipe_cooldown_reset_cost, WHAT)?,
            xp_driver: opt_int(set, p.recipe_xp_driver, WHAT)?,
            level_munge_output: opt_bool(set, p.recipe_level_munge, WHAT)?,
            ingredients: self.ingredients(set, p.recipe_ingredients, WHAT)?,
            optional_ingredients: self.ingredients(set, p.recipe_optional_ingredients, WHAT)?,
            ingredient_pack: nonnull_did(set, p.recipe_pack, WHAT)?,
            ingredient_pack_quantity: opt_int(set, p.recipe_pack_quantity, WHAT)?,
            result: did_or_null(set, p.recipe_result, WHAT)?,
            result_quantity: opt_int(set, p.recipe_result_quantity, WHAT)?
                .unwrap_or(self.defaults.result_quantity),
            critical_result: nonnull_did(set, p.recipe_critical_result, WHAT)?,
            critical_quantity: opt_int(set, p.recipe_critical_quantity, WHAT)?
                .unwrap_or(self.defaults.critical_quantity),
            critical_chance: opt_float(set, p.recipe_critical_chance, WHAT)?,
            multi_outputs,
            xp_reward: opt_enum(set, p.recipe_xp_reward, WHAT)?,
            xp_progression: nonnull_did(set, p.recipe_xp_progression, WHAT)?,
            xp_driving_property: opt_pid_ref(set, p.recipe_xp_driving_property, WHAT)?,
            ui_category: opt_enum(set, p.recipe_ui_category, WHAT)?,
            execution_time: opt_float(set, p.recipe_execution_time, WHAT)?,
            cooldown: opt_enum(set, p.recipe_cooldown, WHAT)?,
            one_time: opt_bool(set, p.recipe_one_time, WHAT)?.unwrap_or(false),
            required_guild: nonnull_did(set, p.recipe_guild, WHAT)?,
            required_facility: nonnull_did(set, p.recipe_facility, WHAT)?,
            induction_action: nonnull_did(set, p.recipe_induction, WHAT)?,
            animation: opt_enum(set, p.recipe_animation, WHAT)?,
            name: opt_text(set, p.recipe_name, WHAT)?,
            name_item_on_normal: opt_bool(set, p.recipe_name_normal, WHAT)?.unwrap_or(false),
            name_item_on_critical: opt_bool(set, p.recipe_name_critical, WHAT)?.unwrap_or(false),
            result_icon: nonnull_did(set, p.recipe_result_icon, WHAT)?,
            critical_icon: nonnull_did(set, p.recipe_critical_icon, WHAT)?,
            critical_effects,
        })
    }

    /// Profession (§5.2).
    pub fn profession<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Profession> {
        const WHAT: &str = "profession";
        let p = &self.pids.craft;
        let tiers = struct_elements(set, p.profession_tiers, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(ProfessionTier {
                    tier: opt_enum(e, p.profession_tier, WHAT)?,
                    recipes: did_elements(e, p.profession_recipes, WHAT)?,
                    proficiency_xp_cap: opt_int(e, p.profession_proficiency_cap, WHAT)?,
                    mastery_xp_cap: opt_int(e, p.profession_mastery_cap, WHAT)?,
                    level_cap_gate: opt_int(e, p.profession_level_gate, WHAT)?,
                    proficiency_title: nonnull_did(e, p.profession_proficiency_title, WHAT)?,
                    mastery_title: nonnull_did(e, p.profession_mastery_title, WHAT)?,
                    completed_proficiency: nonnull_did(
                        e,
                        p.profession_completed_proficiency,
                        WHAT,
                    )?,
                    completed_mastery: nonnull_did(e, p.profession_completed_mastery, WHAT)?,
                    tier_opening: nonnull_did(e, p.profession_tier_opening, WHAT)?,
                    auto_complete_proficiency: opt_bool(e, p.profession_auto_proficiency, WHAT)?,
                    auto_complete_mastery: opt_bool(e, p.profession_auto_mastery, WHAT)?,
                    tutorial_hint: opt_enum(e, p.profession_tutorial_hint, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let pid = |label: Pid| opt_pid_ref(set, label, WHAT);
        let state_properties = ProfessionStateProperties {
            enabled: pid(p.profession_enabled)?,
            proficiency_level: pid(p.profession_proficiency_level)?,
            proficiency_xp: pid(p.profession_proficiency_xp)?,
            mastery_level: pid(p.profession_mastery_level)?,
            mastery_xp: pid(p.profession_mastery_xp)?,
            completed_proficiency_level: pid(p.profession_completed_proficiency_level)?,
            completed_mastery_level: pid(p.profession_completed_mastery_level)?,
            critical_chance_modifier: pid(p.profession_critical_chance_modifier)?,
            start_time: pid(p.profession_start_time)?,
            extra_recipes: pid(p.profession_extra_recipes)?,
            open_level: pid(p.profession_open_level)?,
            tier_open_level: pid(p.profession_tier_open_level)?,
        };
        Ok(Profession {
            tiers,
            item: nonnull_did(set, p.profession_item, WHAT)?,
            required_tool: opt_bits(set, p.profession_tool, WHAT)?,
            characteristic: nonnull_did(set, p.profession_characteristic, WHAT)?,
            state_properties,
            allows_xp_acceleration: opt_bool(set, p.profession_xp_acceleration, WHAT)?
                .unwrap_or(false),
            name: opt_text(set, p.profession_name, WHAT)?,
        })
    }

    /// Vocation (§5.3).
    pub fn vocation<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Vocation> {
        const WHAT: &str = "vocation";
        let p = &self.pids.craft;
        Ok(Vocation {
            professions: did_elements(set, p.vocation_professions, WHAT)?,
            items: did_elements(set, p.vocation_items, WHAT)?,
            name: opt_text(set, p.vocation_name, WHAT)?,
        })
    }

    /// Craft guild (§5.3).
    pub fn craft_guild<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<CraftGuild> {
        const WHAT: &str = "craft guild";
        let p = &self.pids.craft;
        Ok(CraftGuild {
            profession: nonnull_did(set, p.guild_profession, WHAT)?,
            faction: nonnull_did(set, p.guild_faction, WHAT)?,
            required_tier: opt_enum(set, p.guild_required_tier, WHAT)?,
            quest_category: opt_enum(set, p.guild_quest_category, WHAT)?,
        })
    }

    /// Craft control singleton (§5.4).
    pub fn craft_control<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<CraftControl> {
        const WHAT: &str = "craft control";
        let p = &self.pids.craft;
        let xp_rewards = struct_elements(set, p.control_xp_rewards, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(XpReward {
                    key: opt_enum(e, p.control_xp_key, WHAT)?,
                    value: opt_int(e, p.control_xp_value, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let tier_levels = struct_elements(set, p.control_tier_levels, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(TierLevel {
                    tier: opt_enum(e, p.profession_tier, WHAT)?,
                    min_level: opt_int(e, p.usage_min_level, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        let remove_quests = struct_elements(set, p.control_remove_quests, WHAT)?
            .into_iter()
            .map(|e| {
                Ok(RemoveQuests {
                    profession: nonnull_did(e, p.control_profession, WHAT)?,
                    quests: did_elements(e, p.control_quests, WHAT)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(CraftControl {
            xp_rewards,
            tier_levels,
            remove_quests,
        })
    }

    /// Craft directory singleton (§5.4).
    pub fn craft_directory<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<CraftDirectory> {
        const WHAT: &str = "craft directory";
        let p = &self.pids.craft;
        Ok(CraftDirectory {
            vocations: did_elements(set, p.directory_vocations, WHAT)?,
            professions: did_elements(set, p.directory_professions, WHAT)?,
            guilds: did_elements(set, p.directory_guilds, WHAT)?,
        })
    }

    // --- Vendors, barter, currency --------------------------------------------------------

    fn barter_element(&self, e: &PropertyEntry, what: &'static str) -> Result<BarterElement> {
        let p = &self.pids.vendor;
        if p.barter_cost.is(e.pid) {
            let s = e
                .value
                .as_struct()
                .ok_or_else(|| mismatch(what, p.barter_cost, "struct", &e.value))?;
            return Ok(BarterElement::Cost(BarterCost {
                item: did_or_null(s, p.barter_item, what)?,
                quantity: opt_int(s, p.barter_quantity, what)?,
                lookup_table: nonnull_did(s, p.barter_lookup, what)?,
            }));
        }
        if p.barter_reward.is(e.pid) {
            let s = e
                .value
                .as_struct()
                .ok_or_else(|| mismatch(what, p.barter_reward, "struct", &e.value))?;
            return Ok(BarterElement::Reward(BarterReward::Item {
                item: did_or_null(s, p.barter_item, what)?,
                quantity: opt_int(s, p.barter_quantity, what)?,
                uses_min_usage_level: opt_bool(s, p.barter_uses_min_level, what)?.unwrap_or(false),
            }));
        }
        if p.barter_reputation.is(e.pid) {
            let s = e
                .value
                .as_struct()
                .ok_or_else(|| mismatch(what, p.barter_reputation, "struct", &e.value))?;
            return Ok(BarterElement::Reward(BarterReward::Reputation {
                faction: did_or_null(s, p.barter_faction, what)?,
                tier: opt_enum(s, p.barter_reputation_tier, what)?,
            }));
        }
        if p.barter_sound.is(e.pid) {
            let k = e
                .value
                .enum_key()
                .ok_or_else(|| mismatch(what, p.barter_sound, "enum", &e.value))?;
            return Ok(BarterElement::SoundCategory(k));
        }
        Ok(BarterElement::Other(e.pid))
    }

    /// Barter profile (§6.1).
    pub fn barter_profile<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<BarterProfile> {
        const WHAT: &str = "barter profile";
        let p = &self.pids.vendor;
        let mut lists = Vec::new();
        for l in elements(set, p.barter_lists, WHAT)? {
            let a = l
                .value
                .as_array()
                .ok_or_else(|| mismatch(WHAT, p.barter_lists, "array elements", &l.value))?;
            let items = a
                .elements
                .iter()
                .map(|e| self.barter_element(e, WHAT))
                .collect::<Result<_>>()?;
            lists.push(BarterList { elements: items });
        }
        Ok(BarterProfile {
            lists,
            name: opt_text(set, p.barter_name, WHAT)?,
            munge_level_override: opt_int(set, p.barter_munge_level, WHAT)?
                .unwrap_or(self.defaults.munge_level_override),
            permissions: opt_struct(set, p.permissions, WHAT)?.cloned(),
        })
    }

    /// The level of a barter reward item for the look-up rule (§6.2 step 2), read from the
    /// reward item's property set: with `uses_min_usage_level`, its `Usage_MinLevel` (the
    /// top-level property first, else the field of `DefaultPermissionBlobStruct`);
    /// otherwise its `Item_Level`.
    pub fn reward_level<A: PropertyAccess + ?Sized>(
        &self,
        item: &A,
        uses_min_usage_level: bool,
    ) -> Result<Option<i64>> {
        const WHAT: &str = "barter reward item";
        let p = &self.pids.vendor;
        let int = |v: &PropertyValue| v.integer();
        if uses_min_usage_level {
            if let Some(l) = typed(item, p.usage_min_level, WHAT, "int", int)? {
                return Ok(Some(l));
            }
            return match opt_struct(item, p.permissions, WHAT)? {
                Some(s) => typed(s, p.usage_min_level, WHAT, "int", int),
                None => Ok(None),
            };
        }
        typed(item, p.item_level, WHAT, "int", int)
    }

    /// The float progression of a look-up table record (§6.2 step 3). The spec gives no
    /// value types here (OPEN in §6.2), so the minimum index is read from an int (15 or 19)
    /// and each element from a float or an int (9, 15 or 19), bare or as the
    /// `FloatProgression_FloatValue` field of a struct; other types fail.
    pub fn float_progression<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<FloatProgression> {
        const WHAT: &str = "float progression";
        let p = &self.pids.vendor;
        let minimum_index = typed(set, p.progression_minimum_index, WHAT, "int", |v| {
            v.integer()
        })?
        .unwrap_or(0);
        // Float (9) or integer (15, 19) values only.
        #[allow(clippy::cast_precision_loss)]
        let number = |v: &PropertyValue| match v {
            PropertyValue::Float(f) => Some(f64::from(*f)),
            other => other.integer().map(|i| i as f64),
        };
        let values = elements(set, p.float_progression, WHAT)?
            .iter()
            .map(|e| match &e.value {
                PropertyValue::Struct(s) => {
                    typed(s, p.float_progression_value, WHAT, "float", number)?.ok_or(
                        FormatError::MissingProperty {
                            what: WHAT,
                            label: p.float_progression_value.label(),
                        },
                    )
                }
                other => number(other)
                    .ok_or_else(|| mismatch(WHAT, p.float_progression, "float elements", other)),
            })
            .collect::<Result<_>>()?;
        Ok(FloatProgression {
            minimum_index,
            values,
        })
    }

    /// Vendor inventory (§6.3).
    pub fn vendor_inventory<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<VendorInventory> {
        Ok(VendorInventory {
            items: did_elements(set, self.pids.vendor.inventory_items, "vendor inventory")?,
        })
    }

    /// The vendor properties of an NPC (§6.3); `Ok(None)` when the set has none of them.
    pub fn npc_vendor<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Option<NpcVendor>> {
        const WHAT: &str = "NPC vendor";
        let p = &self.pids.vendor;
        let labels = [
            p.vendor_inventories,
            p.vendor_items,
            p.vendor_sell_modifier,
            p.vendor_sells_wearables,
            p.vendor_buys_items,
            p.vendor_discounts,
            p.barter_profiles,
            p.offered_professions,
            p.offered_guilds,
            p.offered_vocations,
        ];
        if !labels.iter().any(|&l| set.has(l)) {
            return Ok(None);
        }
        Ok(Some(NpcVendor {
            inventories: did_elements(set, p.vendor_inventories, WHAT)?,
            item_list: did_elements(set, p.vendor_items, WHAT)?,
            sell_modifier: opt_float(set, p.vendor_sell_modifier, WHAT)?
                .unwrap_or(self.defaults.sell_modifier),
            sells_wearable_items: opt_bool(set, p.vendor_sells_wearables, WHAT)?.unwrap_or(false),
            buys_items: opt_bool(set, p.vendor_buys_items, WHAT)?.unwrap_or(false),
            discounts: pid_elements(set, p.vendor_discounts, WHAT)?,
            barter_profiles: did_elements(set, p.barter_profiles, WHAT)?,
            barter_use_tabs: opt_bool(set, p.barter_use_tabs, WHAT)?.unwrap_or(false),
            offered_professions: did_elements(set, p.offered_professions, WHAT)?,
            offered_guilds: did_elements(set, p.offered_guilds, WHAT)?,
            offered_vocations: did_elements(set, p.offered_vocations, WHAT)?,
        }))
    }

    /// Perk vendor (§6.4).
    pub fn perk_vendor<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<PerkVendor> {
        Ok(PerkVendor {
            perks: did_elements(set, self.pids.vendor.perk_inventory, "perk vendor")?,
        })
    }

    /// Currency denomination (§7).
    pub fn currency<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Currency> {
        const WHAT: &str = "currency";
        let p = &self.pids.vendor;
        Ok(Currency {
            value: opt_int(set, p.currency_value, WHAT)?,
            object: nonnull_did(set, p.currency_object, WHAT)?,
            name: opt_text(set, p.currency_name, WHAT)?,
            singular: opt_text(set, p.currency_singular, WHAT)?,
            plural: opt_text(set, p.currency_plural, WHAT)?,
        })
    }

    /// Currency directory (§7).
    pub fn currency_directory<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
    ) -> Result<CurrencyDirectory> {
        Ok(CurrencyDirectory {
            currencies: did_elements(set, self.pids.vendor.currency_list, "currency directory")?,
        })
    }

    /// Currency control (§7).
    pub fn currency_control<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<CurrencyControl> {
        const WHAT: &str = "currency control";
        let p = &self.pids.vendor;
        Ok(CurrencyControl {
            garnishment: opt_float(set, p.currency_garnishment, WHAT)?,
            gold_cap_item: nonnull_did(set, p.currency_gold_cap_item, WHAT)?,
        })
    }

    /// The item links of a store record (§8).
    pub fn store_links<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<StoreLinks> {
        const WHAT: &str = "store record";
        let p = &self.pids.vendor;
        // §8: the bundle's elements are `WebStoreAccountItem_DataID` DIDs.
        let mut bundle = did_elements(set, p.store_bundle, WHAT)?;
        bundle.retain(|d| !d.is_null());
        Ok(StoreLinks {
            inventory_item: nonnull_did(set, p.store_inventory_item, WHAT)?,
            bundle,
        })
    }

    /// The recipe a recipe item teaches (`RecipeItem_Recipe`, §5.5).
    pub fn taught_recipe<A: PropertyAccess + ?Sized>(&self, set: &A) -> Result<Option<Did>> {
        nonnull_did(set, self.pids.craft.recipe_item_recipe, "recipe item")
    }
}

// ---------------------------------------------------------------------------------------------
// Invariants (§12)
// ---------------------------------------------------------------------------------------------

/// The invariants of §12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Invariant {
    /// E-I1: every record has a property set that decodes completely.
    PropertySet,
    /// E-I2: every non-zero DID names an existing record of a listed target class.
    Targets,
    /// E-I3: trophy entries have a non-zero target and a drop frequency with a control
    /// entry.
    TrophyEntries,
    /// E-I4: weights.
    Weights,
    /// E-I5: level tables have 160 (runic: 80) entries with levels 1 … n in order.
    LevelTables,
    /// E-I6: species-table keys are unique.
    SpeciesKeys,
    /// E-I7: recipe variant, critical chance and result.
    Recipe,
    /// E-I8: profession tiers 1 … 16, each once.
    ProfessionTiers,
    /// E-I9: barter list shapes.
    BarterShape,
    /// E-I10: a barter cost has a quantity or a look-up table, never both.
    BarterCostQuantity,
    /// E-I11: `Vendor_ItemList` is the union of the inventories.
    VendorItemList,
    /// E-I12: class-175 subtree sums and trailing word.
    SubtreeSums,
    /// E-I13: barter entries (class 1942) correspond to the barter lists.
    BarterEntries,
    /// E-I14: the loot-table graph is acyclic and shallow.
    LootGraph,
    /// E-I15: the VOCATIONS and PROFESSIONS DID maps.
    CraftMaps,
}

impl Invariant {
    /// The spec's identifier ("E-I1" …).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::PropertySet => "E-I1",
            Self::Targets => "E-I2",
            Self::TrophyEntries => "E-I3",
            Self::Weights => "E-I4",
            Self::LevelTables => "E-I5",
            Self::SpeciesKeys => "E-I6",
            Self::Recipe => "E-I7",
            Self::ProfessionTiers => "E-I8",
            Self::BarterShape => "E-I9",
            Self::BarterCostQuantity => "E-I10",
            Self::VendorItemList => "E-I11",
            Self::SubtreeSums => "E-I12",
            Self::BarterEntries => "E-I13",
            Self::LootGraph => "E-I14",
            Self::CraftMaps => "E-I15",
        }
    }
}

/// One broken invariant. The spec asks readers to warn, not fail, on real data (§12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Violation {
    /// Which invariant.
    pub invariant: Invariant,
    /// Index of the entry concerned, if any.
    pub entry: Option<usize>,
    /// What is wrong.
    pub detail: &'static str,
}

impl Violation {
    const fn new(invariant: Invariant, entry: Option<usize>, detail: &'static str) -> Self {
        Self {
            invariant,
            entry,
            detail,
        }
    }
}

/// Entries of a level-based and a world-event level table (E-I5).
pub const LEVEL_TABLE_LEVELS: usize = 160;
/// Entries of a runic level table (E-I5).
pub const RUNIC_TABLE_LEVELS: usize = 80;
/// Tier entries of a profession (E-I8).
pub const PROFESSION_TIERS: u32 = 16;

fn check_levels(
    out: &mut Vec<Violation>,
    levels: impl ExactSizeIterator<Item = Option<i32>>,
    expected: usize,
) {
    if levels.len() != expected {
        out.push(Violation::new(
            Invariant::LevelTables,
            None,
            "entry count differs from the table size",
        ));
    }
    for (i, level) in levels.enumerate() {
        if level.and_then(|l| usize::try_from(l).ok()) != Some(i + 1) {
            out.push(Violation::new(
                Invariant::LevelTables,
                Some(i),
                "level is not position + 1",
            ));
        }
    }
}

fn check_some_weight(out: &mut Vec<Violation>, weights: impl Iterator<Item = i32>) {
    let mut any = false;
    for w in weights {
        any |= w >= 1;
    }
    if !any {
        out.push(Violation::new(
            Invariant::Weights,
            None,
            "no entry with weight >= 1",
        ));
    }
}

impl EconomyRecord {
    /// Checks the per-record invariants of §12 (E-I3 without the control-table part, E-I4
    /// to E-I10). Cross-record invariants need the other records: see
    /// [`check_drop_frequencies`], [`NpcVendor::item_list_matches`], [`WeightedTable::check`],
    /// [`BarterList::expected_entry`] and [`CrossLinks::loot_graph`].
    #[must_use]
    pub fn check(&self) -> Vec<Violation> {
        let mut out = Vec::new();
        match self {
            Self::TrophyList(t) => {
                for (i, e) in t.entries.iter().enumerate() {
                    if e.target.is_null() {
                        out.push(Violation::new(
                            Invariant::TrophyEntries,
                            Some(i),
                            "no ItemOrProfile",
                        ));
                    }
                    if e.drop_frequency.is_none() {
                        out.push(Violation::new(
                            Invariant::TrophyEntries,
                            Some(i),
                            "no DropFrequency",
                        ));
                    }
                }
            }
            Self::TreasureGroupProfile(t) => {
                for (i, e) in t.entries.iter().enumerate() {
                    if e.weight < 1 {
                        out.push(Violation::new(Invariant::Weights, Some(i), "weight < 1"));
                    }
                }
            }
            Self::TreasureList(t) => {
                check_some_weight(&mut out, t.entries.iter().map(|e| e.weight))
            }
            Self::WeightedTreasureTable(t) => {
                check_some_weight(&mut out, t.entries.iter().map(|e| e.weight));
            }
            Self::HobbyRewardProfile(t) => {
                check_some_weight(&mut out, t.entries.iter().map(|e| e.weight));
            }
            Self::LevelBasedLootTable(t) => {
                check_levels(
                    &mut out,
                    t.entries.iter().map(|e| Some(e.level)),
                    LEVEL_TABLE_LEVELS,
                );
            }
            Self::WorldEventLevelLootTable(t) => {
                check_levels(
                    &mut out,
                    t.entries.iter().map(|e| e.level),
                    LEVEL_TABLE_LEVELS,
                );
            }
            Self::RunicLevelLootTable(t) => {
                check_levels(
                    &mut out,
                    t.entries.iter().map(|e| Some(e.level)),
                    RUNIC_TABLE_LEVELS,
                );
            }
            Self::SpeciesLevelLootTable(t) => {
                let mut seen = HashSet::new();
                for (i, e) in t.entries.iter().enumerate() {
                    match e.key() {
                        Some(k) if seen.insert(k) => {}
                        Some(_) => {
                            out.push(Violation::new(
                                Invariant::SpeciesKeys,
                                Some(i),
                                "repeated key",
                            ));
                        }
                        None => out.push(Violation::new(
                            Invariant::SpeciesKeys,
                            Some(i),
                            "key fields out of range",
                        )),
                    }
                }
            }
            Self::Recipe(r) => {
                if r.variant().is_none() {
                    out.push(Violation::new(
                        Invariant::Recipe,
                        None,
                        "not exactly one variant",
                    ));
                }
                if r.critical_chance.is_some() != r.critical_result.is_some() {
                    out.push(Violation::new(
                        Invariant::Recipe,
                        None,
                        "critical chance without critical result or the reverse",
                    ));
                }
                if r.result.is_null() {
                    out.push(Violation::new(Invariant::Recipe, None, "no result item"));
                }
            }
            Self::Profession(p) => {
                let keys: Vec<Option<u32>> = p.tiers.iter().map(|t| t.tier).collect();
                let distinct: BTreeSet<u32> = keys.iter().flatten().copied().collect();
                let expected: BTreeSet<u32> = (1..=PROFESSION_TIERS).collect();
                if keys.len() != PROFESSION_TIERS as usize || distinct != expected {
                    out.push(Violation::new(
                        Invariant::ProfessionTiers,
                        None,
                        "tier keys are not 1 to 16, each once",
                    ));
                }
            }
            Self::BarterProfile(b) => {
                for (i, l) in b.lists.iter().enumerate() {
                    if l.shape().is_none() {
                        out.push(Violation::new(
                            Invariant::BarterShape,
                            Some(i),
                            "list matches no shape",
                        ));
                    }
                    for c in l.costs() {
                        if c.quantity.is_some() == c.lookup_table.is_some() {
                            out.push(Violation::new(
                                Invariant::BarterCostQuantity,
                                Some(i),
                                "cost has both or neither of quantity and look-up table",
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// The drop-frequency keys the record uses (trophy and runic lists).
    #[must_use]
    pub fn drop_frequency_keys(&self) -> Vec<u32> {
        match self {
            Self::TrophyList(t) => t.entries.iter().filter_map(|e| e.drop_frequency).collect(),
            Self::RunicList(t) => t.entries.iter().filter_map(|e| e.drop_frequency).collect(),
            _ => Vec::new(),
        }
    }
}

/// E-I3, second part: every drop-frequency key `record` uses has an entry in the control
/// table.
#[must_use]
pub fn check_drop_frequencies(record: &EconomyRecord, control: &LootControl) -> Vec<Violation> {
    record
        .drop_frequency_keys()
        .into_iter()
        .enumerate()
        .filter(|&(_, k)| control.drop_fraction(k).is_none())
        .map(|(i, _)| {
            Violation::new(
                Invariant::TrophyEntries,
                Some(i),
                "drop frequency has no control entry",
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Native codecs of the object form (§9.2, §9.3)
// ---------------------------------------------------------------------------------------------

/// One element of a weighted table (class 175; §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WeightedElement {
    /// The element's own position (in the loot tables also the index of the sibling
    /// `refarray` element).
    pub index: u32,
    /// The weight.
    pub weight: u32,
    /// Subtree sum: weight + sums of children 2*i* + 1 and 2*i* + 2.
    pub sum: u32,
}

/// A weighted table (native class 175; §9.2, §13.2): an implicit binary tree of subtree
/// sums for logarithmic weighted picks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeightedTable {
    /// Elements in stored order.
    pub elements: Vec<WeightedElement>,
    /// The trailing word (0 in all observed).
    pub trailing: u32,
}

/// Bytes of one class-175 element.
const WEIGHTED_ELEMENT_LEN: usize = 12;

impl WeightedTable {
    /// Decodes a class-175 body: u32 count *n*, *n* × (index, weight, sum), u32 trailing
    /// word. The body must be consumed exactly.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let table = Self::read(&mut r)?;
        r.expect_end("weighted table")?;
        Ok(table)
    }

    /// Reads a class-175 body from `r`, leaving the reader after it.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let n = r.u32()?;
        let cap = r.ensure_elements(n, WEIGHTED_ELEMENT_LEN)?;
        let mut elements = Vec::with_capacity(cap);
        for _ in 0..n {
            elements.push(WeightedElement {
                index: r.u32()?,
                weight: r.u32()?,
                sum: r.u32()?,
            });
        }
        Ok(Self {
            elements,
            trailing: r.u32()?,
        })
    }

    /// Encodes the body (the inverse of [`WeightedTable::parse`]).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.elements.len() * WEIGHTED_ELEMENT_LEN);
        let n = u32::try_from(self.elements.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for e in &self.elements {
            out.extend_from_slice(&e.index.to_le_bytes());
            out.extend_from_slice(&e.weight.to_le_bytes());
            out.extend_from_slice(&e.sum.to_le_bytes());
        }
        out.extend_from_slice(&self.trailing.to_le_bytes());
        out
    }

    /// Builds the table for `weights` (§13.2): index words 0 … *n* − 1, subtree sums, trailing
    /// word 0. Sums saturate at `u32::MAX`.
    #[must_use]
    pub fn from_weights(weights: &[u32]) -> Self {
        let mut elements: Vec<WeightedElement> = weights
            .iter()
            .zip(0u32..)
            .map(|(&weight, index)| WeightedElement {
                index,
                weight,
                sum: weight,
            })
            .collect();
        for i in (0..elements.len()).rev() {
            let child = |c: usize| elements.get(c).map_or(0, |e| e.sum);
            let extra = child(2 * i + 1).saturating_add(child(2 * i + 2));
            if let Some(e) = elements.get_mut(i) {
                e.sum = e.sum.saturating_add(extra);
            }
        }
        Self {
            elements,
            trailing: 0,
        }
    }

    /// The weights in stored order.
    #[must_use]
    pub fn weights(&self) -> Vec<u32> {
        self.elements.iter().map(|e| e.weight).collect()
    }

    /// The total weight: the root's sum (0 for an empty table).
    #[must_use]
    pub fn total(&self) -> u32 {
        self.elements.first().map_or(0, |e| e.sum)
    }

    /// Whether every subtree sum holds (E-I12; arithmetic in u64, so no overflow).
    #[must_use]
    pub fn sums_hold(&self) -> bool {
        let sum = |i: usize| self.elements.get(i).map_or(0u64, |e| u64::from(e.sum));
        self.elements
            .iter()
            .enumerate()
            .all(|(i, e)| u64::from(e.sum) == u64::from(e.weight) + sum(2 * i + 1) + sum(2 * i + 2))
    }

    /// Whether every index word equals the element's position (§9.2; holds in the loot
    /// tables, not in every class-175 table of the install).
    #[must_use]
    pub fn indices_hold(&self) -> bool {
        self.elements
            .iter()
            .enumerate()
            .all(|(i, e)| usize::try_from(e.index).ok() == Some(i))
    }

    /// E-I12: subtree sums and the trailing word.
    #[must_use]
    pub fn check(&self) -> Vec<Violation> {
        let mut out = Vec::new();
        if !self.sums_hold() {
            out.push(Violation::new(
                Invariant::SubtreeSums,
                None,
                "a subtree sum is wrong",
            ));
        }
        if self.trailing != 0 {
            out.push(Violation::new(
                Invariant::SubtreeSums,
                None,
                "trailing word is not 0",
            ));
        }
        out
    }
}

/// A weighted pick by cumulative sums (§11.3): the entry *i* whose cumulative range
/// [*w*₀ + … + *w*ᵢ₋₁, *w*₀ + … + *w*ᵢ) holds `r`. `r` must lie in [0, Σ*w*); `None` otherwise.
/// Which entry a given `r` selects in the server's own tree descent is OPEN (§11.3); only the
/// probabilities agree.
#[must_use]
pub fn cumulative_pick(weights: &[u32], r: u64) -> Option<usize> {
    let mut acc = 0u64;
    for (i, &w) in weights.iter().enumerate() {
        acc += u64::from(w);
        if r < acc {
            return Some(i);
        }
    }
    None
}

fn clamp_weight(w: i32) -> u32 {
    u32::try_from(w).unwrap_or(0)
}

/// Length of a barter entry (class 1942; §9.3).
pub const BARTER_ENTRY_LEN: usize = 49;

/// A barter entry (native class 1942; §9.3, §13.3), one per barter list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BarterEntry {
    /// Offset 0: the barter profile that holds the entry.
    pub profile: Did,
    /// Offset 4: reward item; 0 for a reputation reward.
    pub reward_item: Did,
    /// Offset 8: reward quantity; 0 for a reputation reward.
    pub reward_quantity: u32,
    /// Offset 12: `Barter_ItemUsesMinUsageLevelForCost` of the reward.
    pub uses_min_usage_level: bool,
    /// Offset 13: reputation faction; 0 otherwise.
    pub reputation_faction: Did,
    /// Offset 17: reputation reward tier key; 0 otherwise.
    pub reputation_tier: u32,
    /// Offset 21: costs 1–3 (item, quantity); unused slots (0, 0).
    pub costs: [(Did, u32); 3],
    /// Offset 45: `Barter_SoundCategory` key; 0 when absent.
    pub sound_category: u32,
}

impl BarterEntry {
    /// Decodes the 49 bytes of an entry (exactly; offset 12 must be 0 or 1).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let e = Self::read(&mut r)?;
        r.expect_end("barter entry")?;
        Ok(e)
    }

    /// Reads one entry from `r`.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let profile = r.did()?;
        let reward_item = r.did()?;
        let reward_quantity = r.u32()?;
        let uses_min_usage_level = r.bool8()?;
        let reputation_faction = r.did()?;
        let reputation_tier = r.u32()?;
        let mut costs = [(Did(0), 0); 3];
        for c in &mut costs {
            *c = (r.did()?, r.u32()?);
        }
        let sound_category = r.u32()?;
        Ok(Self {
            profile,
            reward_item,
            reward_quantity,
            uses_min_usage_level,
            reputation_faction,
            reputation_tier,
            costs,
            sound_category,
        })
    }

    /// Encodes the 49 bytes (little-endian).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; BARTER_ENTRY_LEN] {
        let mut out = [0u8; BARTER_ENTRY_LEN];
        let mut words: Vec<u8> = Vec::with_capacity(BARTER_ENTRY_LEN);
        words.extend_from_slice(&self.profile.0.to_le_bytes());
        words.extend_from_slice(&self.reward_item.0.to_le_bytes());
        words.extend_from_slice(&self.reward_quantity.to_le_bytes());
        words.push(u8::from(self.uses_min_usage_level));
        words.extend_from_slice(&self.reputation_faction.0.to_le_bytes());
        words.extend_from_slice(&self.reputation_tier.to_le_bytes());
        for (d, q) in self.costs {
            words.extend_from_slice(&d.0.to_le_bytes());
            words.extend_from_slice(&q.to_le_bytes());
        }
        words.extend_from_slice(&self.sound_category.to_le_bytes());
        out.copy_from_slice(&words);
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Cross-links (§10)
// ---------------------------------------------------------------------------------------------

/// One source found by [`CrossLinks::drops_from`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropSource {
    /// The record that (indirectly) drops the item.
    pub source: Did,
    /// Its economy kind, if it is an economy record ([`EconomyKind::is_special_source`]
    /// kinds are reported as sources too, §10.2 step 5).
    pub kind: Option<EconomyKind>,
    /// Edges from the source to the item.
    pub depth: usize,
    /// The loot tables between the source and the item, nearest the source first.
    pub path: Vec<Did>,
    /// The PID of the source's reference.
    pub field: u32,
}

/// Statistics of the loot-table graph (E-I14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootGraphStats {
    /// Loot-table records with forward edges.
    pub tables: usize,
    /// Whether the graph of loot tables (items excluded) has no cycle.
    pub acyclic: bool,
    /// Longest path, in edges, from a loot table to a record that is not a loot table
    /// (meaningful only when acyclic).
    pub longest_path: usize,
}

/// Depth limit of a "drops from" walk that is safe for the observed data (§10.2 step 4).
pub const DROPS_FROM_DEPTH: usize = 8;

/// The explorer's cross-links (§10.2), built from the property form of every record.
///
/// Build it in two phases: first [`CrossLinks::note_kind`] for every economy record (so that
/// references into loot tables can be recognised), then [`CrossLinks::add`] for every record
/// with a property set.
#[derive(Debug, Clone, Default)]
pub struct CrossLinks {
    kinds: HashMap<Did, EconomyKind>,
    loot: ReverseDidIndex,
    forward: HashMap<Did, Vec<Did>>,
    item_inventories: HashMap<Did, BTreeSet<Did>>,
    inventory_vendors: HashMap<Did, BTreeSet<Did>>,
    reward_in: HashMap<Did, BTreeSet<Did>>,
    cost_in: HashMap<Did, BTreeSet<Did>>,
    traders: HashMap<Did, BTreeSet<Did>>,
    crafted_by: HashMap<Did, BTreeSet<Did>>,
    used_in: HashMap<Did, BTreeSet<Did>>,
    learned_from: HashMap<Did, BTreeSet<Did>>,
    store: HashMap<Did, BTreeSet<Did>>,
}

fn link(map: &mut HashMap<Did, BTreeSet<Did>>, key: Did, value: Did) {
    if !key.is_null() && !value.is_null() {
        map.entry(key).or_default().insert(value);
    }
}

fn linked(map: &HashMap<Did, BTreeSet<Did>>, key: Did) -> Vec<Did> {
    map.get(&key)
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default()
}

impl CrossLinks {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Phase 1: records the kind of economy record `did`.
    pub fn note_kind(&mut self, did: Did, kind: EconomyKind) {
        self.kinds.insert(did, kind);
    }

    /// The noted kind of `did`.
    #[must_use]
    pub fn kind(&self, did: Did) -> Option<EconomyKind> {
        self.kinds.get(&did).copied()
    }

    fn is_loot(&self, did: Did) -> bool {
        self.kind(did).is_some_and(EconomyKind::is_loot_table)
    }

    /// Phase 2: adds the links of record `did`. `record` is its decoded economy view, if it
    /// is an economy record (its kind is noted here too).
    ///
    /// - Loot tables: every non-zero DID field is a forward edge (§10.2 step 1).
    /// - Other records: references into noted loot tables (the attaching edges of §4.13,
    ///   step 3), vendor properties, recipe items and store links.
    /// - Economy records: vendor inventories, barter rewards and costs, recipe results and
    ///   ingredients, profession recipe lists.
    pub fn add(
        &mut self,
        reader: &EconomyReader,
        did: Did,
        set: &PropertySet,
        record: Option<&EconomyRecord>,
    ) -> Result<()> {
        if let Some(r) = record {
            self.note_kind(did, r.kind());
        }
        let source_is_loot = self.is_loot(did);
        let mut forward = Vec::new();
        let mut edges = Vec::new();
        for_each_did_reference(set, |r| {
            if source_is_loot || self.is_loot(r.target) {
                edges.push(r);
            }
        });
        for r in edges {
            self.loot.add_edge(
                r.target,
                Referrer {
                    source: did,
                    top: r.top,
                    pid: r.pid,
                },
            );
            if source_is_loot {
                forward.push(r.target);
            }
        }
        if source_is_loot && !forward.is_empty() {
            self.forward.entry(did).or_default().extend(forward);
        }

        match record {
            Some(EconomyRecord::VendorInventory(v)) => {
                for &item in &v.items {
                    link(&mut self.item_inventories, item, did);
                }
            }
            Some(EconomyRecord::BarterProfile(b)) => {
                for l in &b.lists {
                    for c in l.costs() {
                        link(&mut self.cost_in, c.item, did);
                    }
                    if let Some(BarterReward::Item { item, .. }) = l.reward() {
                        link(&mut self.reward_in, *item, did);
                    }
                }
            }
            Some(EconomyRecord::Recipe(r)) => {
                for item in r.produced_items() {
                    link(&mut self.crafted_by, item, did);
                }
                for item in r.used_items() {
                    link(&mut self.used_in, item, did);
                }
            }
            Some(EconomyRecord::Profession(p)) => {
                for recipe in p.recipes() {
                    link(&mut self.learned_from, recipe, did);
                }
            }
            Some(_) => {}
            None => {
                if let Some(v) = reader.npc_vendor(set)? {
                    for &inv in &v.inventories {
                        link(&mut self.inventory_vendors, inv, did);
                    }
                    for &profile in &v.barter_profiles {
                        link(&mut self.traders, profile, did);
                    }
                }
                if let Some(recipe) = reader.taught_recipe(set)? {
                    link(&mut self.learned_from, recipe, did);
                }
                let store = reader.store_links(set)?;
                if let Some(item) = store.inventory_item {
                    link(&mut self.store, item, did);
                }
            }
        }
        Ok(())
    }

    /// Number of edges in the loot reverse index.
    #[must_use]
    pub fn loot_edges(&self) -> usize {
        self.loot.len()
    }

    /// "Drops from" (§10.2 steps 4–5): the records that drop `item` through loot tables,
    /// found breadth first up to `max_depth` edges ([`DROPS_FROM_DEPTH`] is safe). The walk
    /// passes through loot tables only; every record that is not a loot table, and every
    /// special source kind, is reported with the path of tables in between.
    #[must_use]
    pub fn drops_from(&self, item: Did, max_depth: usize) -> Vec<DropSource> {
        let hits = self
            .loot
            .walk_referrers(item, max_depth, |d| self.is_loot(d));
        let via: HashMap<Did, Did> = hits.iter().map(|h| (h.source, h.via)).collect();
        hits.iter()
            .filter(|h| {
                self.kind(h.source)
                    .is_none_or(|k| !k.is_loot_table() || k.is_special_source())
            })
            .map(|h| {
                let mut path = Vec::new();
                let mut at = h.via;
                while at != item && path.len() < h.depth {
                    path.push(at);
                    match via.get(&at) {
                        Some(&next) => at = next,
                        None => break,
                    }
                }
                DropSource {
                    source: h.source,
                    kind: self.kind(h.source),
                    depth: h.depth,
                    path,
                    field: h.edge.pid,
                }
            })
            .collect()
    }

    /// "Sold by" (§10.2): (NPC, inventory) pairs whose inventory lists `item`.
    #[must_use]
    pub fn sold_by(&self, item: Did) -> Vec<(Did, Did)> {
        let mut out = Vec::new();
        for inv in linked(&self.item_inventories, item) {
            for npc in linked(&self.inventory_vendors, inv) {
                out.push((npc, inv));
            }
        }
        out
    }

    /// "Bartered for" (§10.2): the barter profiles whose rewards include `item`.
    #[must_use]
    pub fn bartered_for(&self, item: Did) -> Vec<Did> {
        linked(&self.reward_in, item)
    }

    /// "Used as barter currency in" (§10.2): the profiles whose costs include `item`.
    #[must_use]
    pub fn barter_currency_in(&self, item: Did) -> Vec<Did> {
        linked(&self.cost_in, item)
    }

    /// The NPCs (and mobs) whose `Barter_ProfileArray` names `profile`.
    #[must_use]
    pub fn traders(&self, profile: Did) -> Vec<Did> {
        linked(&self.traders, profile)
    }

    /// "Crafted by" (§10.2): recipes that produce `item`.
    #[must_use]
    pub fn crafted_by(&self, item: Did) -> Vec<Did> {
        linked(&self.crafted_by, item)
    }

    /// "Used in" (§10.2): recipes that take `item`.
    #[must_use]
    pub fn used_in(&self, item: Did) -> Vec<Did> {
        linked(&self.used_in, item)
    }

    /// "Learned from" (§10.2): recipe items and professions that teach `recipe`.
    #[must_use]
    pub fn learned_from(&self, recipe: Did) -> Vec<Did> {
        linked(&self.learned_from, recipe)
    }

    /// "Offered in the store" (§10.2, §8): store records naming `item`.
    #[must_use]
    pub fn offered_in_store(&self, item: Did) -> Vec<Did> {
        linked(&self.store, item)
    }

    /// E-I14: whether the loot-table graph is acyclic and its longest path to a record that
    /// is not a loot table. Iterative, so hostile graphs cannot exhaust the stack.
    #[must_use]
    pub fn loot_graph(&self) -> LootGraphStats {
        // 0 = unvisited, 1 = on the stack, 2 = done.
        let mut state: HashMap<Did, u8> = HashMap::new();
        let mut longest: HashMap<Did, usize> = HashMap::new();
        let mut acyclic = true;
        let empty = Vec::new();
        for &start in self.forward.keys() {
            if state.contains_key(&start) {
                continue;
            }
            let mut stack: Vec<(Did, usize)> = vec![(start, 0)];
            state.insert(start, 1);
            while let Some(top) = stack.last_mut() {
                let node = top.0;
                let targets = self.forward.get(&node).unwrap_or(&empty);
                if let Some(&t) = targets.get(top.1) {
                    top.1 += 1;
                    if !self.is_loot(t) || !self.forward.contains_key(&t) {
                        continue;
                    }
                    match state.get(&t).copied().unwrap_or(0) {
                        0 => {
                            state.insert(t, 1);
                            stack.push((t, 0));
                        }
                        1 => acyclic = false,
                        _ => {}
                    }
                } else {
                    let best = targets
                        .iter()
                        .map(|t| {
                            if self.is_loot(*t) && self.forward.contains_key(t) {
                                longest.get(t).copied().unwrap_or(0).saturating_add(1)
                            } else {
                                1
                            }
                        })
                        .max()
                        .unwrap_or(0);
                    longest.insert(node, best);
                    state.insert(node, 2);
                    stack.pop();
                }
            }
        }
        LootGraphStats {
            tables: self.forward.len(),
            acyclic,
            longest_path: longest.values().copied().max().unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtree_sums_of_the_worked_example() {
        // §13.2.
        let t = WeightedTable::from_weights(&[2, 5, 1, 4]);
        let words: Vec<[u32; 3]> = t
            .elements
            .iter()
            .map(|e| [e.index, e.weight, e.sum])
            .collect();
        assert_eq!(words, vec![[0, 2, 12], [1, 5, 9], [2, 1, 1], [3, 4, 4]]);
        assert_eq!(t.total(), 12);
        assert!(t.sums_hold() && t.indices_hold() && t.check().is_empty());
        assert_eq!(WeightedTable::parse(&t.to_bytes()).unwrap(), t);
    }

    #[test]
    fn species_key_packing() {
        // §13.5.
        assert_eq!(species_key(35, 5, 7), Some(0x0000_0007_0230_0050));
        assert_eq!(species_key(1 << 12, 0, 0), None);
    }

    #[test]
    fn cumulative_pick_bounds() {
        let w = [2, 5, 1, 4];
        assert_eq!(cumulative_pick(&w, 0), Some(0));
        assert_eq!(cumulative_pick(&w, 2), Some(1));
        assert_eq!(cumulative_pick(&w, 11), Some(3));
        assert_eq!(cumulative_pick(&w, 12), None);
        assert_eq!(cumulative_pick(&[], 0), None);
    }
}
