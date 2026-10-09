// SPDX-License-Identifier: GPL-3.0-or-later
//! Economy views (`tessera_formats::gameplay::economy`, `docs/specs/economy.md`) on arbitrary
//! bytes: the native codecs of the object form (class-175 weighted tables, §9.2, and the
//! 49-byte class-1942 barter entries, §9.3) and every typed view of §4–§8 over an arbitrary
//! property set, with the invariant checks of §12 and the cross-links of §10.
//!
//! Input: the whole input is tried as a class-175 body and its first 49 bytes as a barter
//! entry. Byte 0 then selects a mode, and the rest feeds two property sets:
//!
//! 1. **Decoded**: the rest parsed as a bare property set. The synthetic dictionary gives
//!    every label the module reads, and the element labels of its arrays, a small PID (1, 2,
//!    …) and the value type the spec gives it ([`TYPED`]); a PID outside the dictionary gets a
//!    type from its number.
//! 2. **Built**: the rest read as a stream of choices that builds a set directly: the
//!    characteristic property of a kind (§3.4) and neighbouring labels, each with its spec
//!    type, structs and arrays filled from the labels that follow them in [`TYPED`] (where the
//!    spec's tables list an array's element and fields), DIDs from a small pool so that the
//!    links loop back. Random bytes rarely parse into deep well-typed sets, so this path is
//!    what drives the entry decoders of every kind, the barter shapes, the filters and the
//!    graph walks with well-typed data.
//!
//! With a selector below 0x80 every label has its spec type. From 0x80 up, about a quarter
//! of the labels of the decoded set and about one built value in eight get another family
//! type, which drives the type-mismatch paths deep inside otherwise well-typed records.
//!
//! Each set is read as every kind, checked, and linked under its own DID and under the DIDs
//! it names (so the loot graph gets self-references and cycles), then walked.
//!
//! Nothing may panic; parse/encode must round-trip; subtree sums built by `from_weights`
//! must hold whenever the total weight fits a u32; walks must end.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_economy -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::common::build::{array, did, entry, set, structure};
use tessera_formats::gameplay::common::did_references;
use tessera_formats::gameplay::economy::{
    BARTER_ENTRY_LEN, BarterEntry, CraftPids, CrossLinks, DROPS_FROM_DEPTH, EconomyDefaults,
    EconomyKind, EconomyPids, EconomyReader, EconomyRecord, KindByClass, LootPids, VendorPids,
    WeightedTable, check_drop_frequencies, cumulative_pick, lookup_quantity,
};
use tessera_formats::{
    Did, PropertyEntry, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource,
};

/// Value types (codes of `property-system.md` §3) of every label the module reads, as the
/// spec's tables give them (economy.md §4–§8), and of the element labels of its arrays.
/// Labels whose type the spec does not state (the progression fields of §6.2) get the type
/// the reader accepts first; `WorldEvent_ConditionValue` is an int (E1).
///
/// The list is also the grammar of the set builder: every struct or array is followed by
/// its element and fields (repeated where several compounds share them; where the spec names
/// no element label, a struct label of the same section stands in, since the readers do not
/// look at element labels).
const TYPED: &[(&str, u32)] = &[
    // §4.1–§4.4
    ("LootGen_TrophyList", 12),
    ("LootGen_TrophyList_Entry", 11),
    ("LootGen_TrophyList_ItemOrProfile", 20),
    ("LootGen_TrophyList_DropFrequency", 8),
    ("LootGen_TrophyList_ItemQuantity", 15),
    ("LootGen_TrophyList_GroupDrop", 21),
    ("Description", 13),
    ("Icon_Layer_ImageOverrideDID", 20),
    ("Inventory_MaxQuantity", 15),
    ("TreasureGroupProfile_ItemTable", 12),
    ("TreasureGroupProfile_ItemTable_Entry", 11),
    ("TreasureGroupProfile_ItemTable_Item", 20),
    ("TreasureGroupProfile_ItemTable_Weight", 15),
    ("TreasureGroupProfile_ItemTable_Quantity", 15),
    ("LootGen_TreasureList", 12),
    ("LootGen_TreasureList_Entry", 11),
    ("LootGen_TreasureList_GroupProfile", 20),
    ("LootGen_TreasureList_Weight", 15),
    ("LootGen_WeightedTreasureTable", 12),
    ("LootGen_WeightedTreasureTable_Entry", 11),
    ("LootGen_WeightedTreasureTable_TrophyList", 20),
    ("LootGen_WeightedTreasureTable_TrophyList_Weight", 15),
    // §4.5
    ("LootGen_FilteredTreasureListArray", 12),
    ("LootGen_FilteredTreasureListArray_Entry", 11),
    ("LootGen_FilteredTrophyTable_EntryDID", 20),
    ("EntityFilter_Array", 12),
    ("EntityFilter_PropertySet", 12),
    ("EntityFilter_PropertyRange", 11),
    ("EntityFilter_PropertyRange_Min", 12),
    ("EntityFilter_PropertyRange_Max", 12),
    ("EntityFilter_WorldEvent", 11),
    ("WorldEvent_Condition", 11),
    ("WorldEvent_WorldEvent", 20),
    ("WorldEvent_Operator", 8),
    ("WorldEvent_ConditionValue", 15),
    ("WorldEvent_AllConditionList", 12),
    // §4.6–§4.8
    ("LevelBasedLootTable_LootTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("LevelBasedLootTable_Level", 15),
    ("LevelBasedLootTable_TrophyList", 20),
    ("LevelBasedLootTable_TreasureProfile", 20),
    ("SpeciesLevelLootTable", 12),
    ("SpeciesLevelLootTable_Entry", 11),
    ("Agent_Species", 8),
    ("Agent_Subspecies", 8),
    ("Agent_Class", 8),
    ("MonsterLevel_ExaminationModStatType", 8),
    ("LevelBasedLootTable_LevelTable", 20),
    ("PVPLevelLootTable", 12),
    ("SpeciesLevelLootTable_Entry", 11),
    ("Agent_Species", 8),
    ("Agent_Class", 8),
    ("LevelBasedLootTable_LevelTable", 20),
    ("WorldEventLevelLootTable_LootTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("WorldEventLevelLootTable_Level", 15),
    ("WorldEventLevelLootTable_TrophyListTemplate", 20),
    ("WorldEventLootTableDirectory_Array", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("WorldEventLootTableDirectory_LevelTable", 20),
    ("WorldEvent_AllConditionList", 12),
    ("WorldEvent_Condition", 11),
    ("WorldEvent_WorldEvent", 20),
    ("WorldEvent_Operator", 8),
    ("WorldEvent_ConditionValue", 15),
    // §4.9–§4.11
    ("RunicLevelLootTable_LootTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("RunicLevelLootTable_Level", 15),
    ("RunicLevelLootTable_TrophyList", 20),
    ("RunicLevelLootTable_RunicTrophyList", 20),
    ("RunicList_Array", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("RunicList_RunicOrTreasureGroup", 20),
    ("RunicList_DropFrequency", 8),
    ("RunicTreasureGroup_Array", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("RunicTreasureGroup_Runic", 20),
    ("RunicTreasureGroup_Weight", 15),
    ("RunicTreasureGroup_PullCount_Array", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("RunicTreasureGroup_PullCount", 15),
    ("SkirmishTreasureLookup_EncounterDID", 20),
    ("SkirmishTreasureLookup_SkirmishMatchParametersDID", 20),
    ("SkirmishTreasureLookup_TrophyListTemplateDID", 20),
    ("SkirmishTreasureLookup_TreasureListTemplateDID", 20),
    ("SkirmishTreasureLookup_BarterTrophyListDID", 20),
    ("SkirmishTreasureLookup_ReputationTrophyListDID", 20),
    ("SkirmishCustomLootLookupTable_Array", 12),
    ("SkirmishCustomLootLookupTable_Entry", 20),
    ("HobbyRewardProfile_AdvancementProgression", 20),
    ("HobbyRewardProfile_ItemArray", 12),
    ("HobbyRewardProfile_ItemStruct", 11),
    ("HobbyRewardProfile_Item", 20),
    ("HobbyRewardProfile_Weight", 15),
    ("HobbyRewardProfile_MinProficiency", 15),
    ("HobbyRewardProfile_MaxProficiency", 15),
    ("HobbyRewardProfile_MetaScore", 15),
    ("HobbyRewardProfile_TimeAvailable", 8),
    // §4.12
    ("LootGenControl_DropFrequencyTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("LootGenControl_DropFrequency_Label", 8),
    ("LootGenControl_DropFrequency_Percentage", 9),
    ("LootGenControl_MoneyDropTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("LootGenControl_MoneyDropTable_DropAmountMaximum", 15),
    ("LootGenControl_MoneyDropTable_DropAmountVariance", 15),
    ("LootGenControl_MoneyDropModifierTable", 12),
    ("LevelBasedLootTable_LootTable_Entry", 11),
    ("LootGenControl_MoneyDropModifier_Label", 8),
    ("LootGenControl_MoneyDropModifier_Multiplier", 9),
    ("LootGenControl_SpeciesLevelLootTableArray", 12),
    ("LootGenControl_SpeciesLevelLootTable", 20),
    ("LootGenControl_PVPLevelLootTable", 20),
    // §5.1
    ("CraftRecipe_Profession", 20),
    ("CraftRecipe_Tier", 8),
    ("CraftRecipe_TierArray", 12),
    ("CraftRecipe_Tier", 8),
    ("CraftRecipe_IngredientList", 12),
    ("CraftRecipe_IngredientEntry", 11),
    ("CraftRecipe_Ingredient", 20),
    ("CraftRecipe_IngredientQuantity", 15),
    ("CraftRecipe_IngredientIsConsumed", 21),
    ("CraftRecipe_OptionalIngredientList", 12),
    ("CraftRecipe_IngredientEntry", 11),
    ("CraftRecipe_Ingredient", 20),
    ("CraftRecipe_IngredientQuantity", 15),
    ("CraftRecipe_IngredientIsConsumed", 21),
    ("CraftRecipe_IngredientCritBonus", 9),
    ("CraftRecipe_IngredientPack", 20),
    ("CraftRecipe_IngredientPackQuantity", 15),
    ("CraftRecipe_ResultItem", 20),
    ("CraftRecipe_ResultItemQuantity", 15),
    ("CraftRecipe_CriticalResultItem", 20),
    ("CraftRecipe_CriticalResultItemQuantity", 15),
    ("CraftRecipe_CriticalSuccessChance", 9),
    ("CraftRecipe_MultiOutputArray", 12),
    ("CraftRecipe_MultiOutputEntry", 11),
    ("CraftRecipe_Ingredient", 20),
    ("CraftRecipe_ResultItem", 20),
    ("CraftRecipe_CriticalResultItem", 20),
    ("CraftRecipe_XPReward", 8),
    ("CraftRecipe_XPAward_Progression", 20),
    ("CraftRecipe_XPAward_Progression_DrivingProperty", 10),
    ("CraftRecipe_XPAward_Driver", 15),
    ("CraftRecipe_LevelMungeOutput", 21),
    ("CraftRecipe_UICategory", 8),
    ("CraftRecipe_ExecutionTime", 9),
    ("CraftRecipe_CooldownDuration", 8),
    ("CraftRecipe_Cooldown_MithrilResetCost", 15),
    ("CraftRecipe_OneTimeRecipe", 21),
    ("CraftRecipe_RequiredCraftGuild", 20),
    ("CraftRecipe_RequiredFacilityEffect", 20),
    ("CraftRecipe_InductionAction", 20),
    ("CraftRecipe_Animation", 8),
    ("CraftRecipe_Name", 13),
    ("CraftRecipe_NameItemOnNormalSuccess", 21),
    ("CraftRecipe_NameItemOnCriticalSuccess", 21),
    ("CraftRecipe_Field_ResultIcon", 20),
    ("CraftRecipe_Field_CritResultIcon", 20),
    ("EffectGenerator_CraftingCriticalSuccessEffectList", 12),
    ("EffectGenerator_EffectStruct", 11),
    ("EffectGenerator_EffectID", 20),
    ("EffectGenerator_EffectSpellcraft", 9),
    ("Item_iLevel_Crafting_Offset", 15),
    // §5.2
    ("CraftProfession_TierArray", 12),
    ("CraftProfession_TierEntry", 11),
    ("CraftProfession_Tier", 8),
    ("CraftProfession_RecipeArray", 12),
    ("CraftProfession_Recipe", 20),
    ("CraftProfession_ProficiencyXPCap", 15),
    ("CraftProfession_MasteryXPCap", 15),
    ("CraftProfession_TierLevelCapGate", 15),
    ("CraftProfession_ProficiencyTitle", 20),
    ("CraftProfession_MasteryTitle", 20),
    (
        "CraftProfession_CompletedProficiencyLevelCharacteristic",
        20,
    ),
    ("CraftProfession_CompletedMasteryLevelCharacteristic", 20),
    ("CraftProfession_TierOpeningCharacteristic", 20),
    ("CraftProfession_AutoCompleteProficiency", 21),
    ("CraftProfession_AutoCompleteMastery", 21),
    ("CraftProfession_ProficiencyTutorialHint", 8),
    ("CraftProfession_Item", 20),
    ("CraftProfession_RequiredTool", 18),
    ("CraftProfession_Characteristic", 20),
    ("CraftProfession_Enabled_PropertyName", 10),
    ("CraftProfession_ProficiencyLevel_PropertyName", 10),
    ("CraftProfession_ProficiencyXP_PropertyName", 10),
    ("CraftProfession_MasteryLevel_PropertyName", 10),
    ("CraftProfession_MasteryXP_PropertyName", 10),
    ("CraftProfession_CompletedProficiencyLevel_PropertyName", 10),
    ("CraftProfession_CompletedMasteryLevel_PropertyName", 10),
    ("CraftProfession_CriticalChanceAddModifier_PropertyName", 10),
    ("CraftProfession_StartTime_PropertyName", 10),
    ("CraftProfession_ExtraRecipeArray_PropertyName", 10),
    ("CraftProfession_OpenLevel_PropertyName", 10),
    ("CraftProfession_TierOpenLevel_PropertyName", 10),
    ("CraftProfession_AllowsXPAcceleration", 21),
    ("CraftProfession_Name", 13),
    // §5.3, §5.4
    ("CraftVocation_ProfessionArray", 12),
    ("CraftVocation_Profession", 20),
    ("CraftVocation_ItemArray", 12),
    ("CraftVocation_Item", 20),
    ("CraftVocation_Name", 13),
    ("CraftGuild_Profession", 20),
    ("CraftGuild_Faction", 20),
    ("CraftGuild_RequiredCraftTier", 8),
    ("CraftGuild_QuestCategory", 8),
    ("CraftControl_XPRewardArray", 12),
    ("CraftControl_XPRewardEntry", 11),
    ("CraftControl_XPRewardEnum", 8),
    ("CraftControl_XPRewardValue", 15),
    ("CraftControl_TierToMungeLevelArray", 12),
    ("CraftControl_XPRewardEntry", 11),
    ("CraftProfession_Tier", 8),
    ("Usage_MinLevel", 15),
    ("CraftControl_ChangeVocation_RemoveQuestsArray", 12),
    ("CraftControl_XPRewardEntry", 11),
    ("CraftControl_Profession", 20),
    ("CraftControl_RemoveQuestsArray", 12),
    ("CraftControl_QuestToRemove", 20),
    ("CraftDirectory_VocationArray", 12),
    ("CraftVocation_Profession", 20),
    ("CraftDirectory_ProfessionArray", 12),
    ("CraftVocation_Profession", 20),
    ("CraftDirectory_CraftGuildArray", 12),
    ("CraftVocation_Profession", 20),
    ("RecipeItem_Recipe", 20),
    // §6.1, §6.2
    ("Barter_ItemListArray", 12),
    ("Barter_ItemList", 12),
    ("Barter_ItemStruct", 11),
    ("Barter_Item", 20),
    ("Barter_ItemQuantity", 15),
    ("Barter_ItemQuantity_LookupTable", 20),
    ("Barter_RewardItemStruct", 11),
    ("Barter_Item", 20),
    ("Barter_ItemQuantity", 15),
    ("Barter_ItemUsesMinUsageLevelForCost", 21),
    ("Barter_RewardReputationStruct", 11),
    ("Barter_ReputationFaction", 20),
    ("Barter_ReputationRewardTier", 8),
    ("Barter_SoundCategory", 8),
    ("Barter_Profile_Name", 13),
    ("Barter_ItemMungeLevelOverride", 15),
    ("DefaultPermissionBlobStruct", 11),
    ("Usage_MinLevel", 15),
    ("Progression_MinimumIndexValue", 15),
    ("FloatProgression_Array", 12),
    ("FloatProgression_FloatValue", 9),
    ("Item_Level", 15),
    // §6.3, §6.4
    ("VendorInventory_Items", 12),
    ("VendorInventory_Item", 20),
    ("Vendor_InventoryList", 12),
    ("Vendor_Inventory", 20),
    ("Vendor_ItemList", 12),
    ("Vendor_Item", 20),
    ("Vendor_SellModifier", 9),
    ("Vendor_SellsWearableItems", 21),
    ("Vendor_BuysItems", 21),
    ("Vendor_DiscountList", 12),
    ("Vendor_Discount", 10),
    ("Barter_ProfileArray", 12),
    ("Barter_Profile", 20),
    ("Barter_Profile_UseTabs", 21),
    ("Craft_OfferedProfessionArray", 12),
    ("CraftVocation_Profession", 20),
    ("Craft_OfferedCraftGuildArray", 12),
    ("CraftVocation_Profession", 20),
    ("Craft_OfferedVocationArray", 12),
    ("CraftVocation_Profession", 20),
    ("PerkVendor_InventoryList", 12),
    ("PerkVendor_PerkDID", 20),
    // §7, §8
    ("Currency_Value", 15),
    ("Currency_ObjectDID", 20),
    ("Currency_Name", 13),
    ("Currency_Name_Singular", 13),
    ("Currency_Name_Plural", 13),
    ("Currency_CurrencyList", 12),
    ("Currency_Currency", 20),
    ("Currency_Control_GarnishmentPercentage", 9),
    ("Currency_Control_GoldCapWebstoreItem", 20),
    ("WebStoreInventoryItem_DataID", 20),
    ("WebStoreAccountItem_BundleItemArray", 12),
    ("WebStoreAccountItem_DataID", 20),
];

/// Type codes of the family (economy.md §3.1), for PIDs outside the dictionary and for
/// mismatches.
const TYPES: [u32; 10] = [8, 9, 10, 11, 12, 13, 15, 18, 20, 21];

struct Fixture {
    reader: EconomyReader,
    /// PID → spec type code.
    types: HashMap<u32, u32>,
    /// PID of each entry of [`TYPED`], same order.
    typed_pids: Vec<u32>,
    /// Index in [`TYPED`] of the characteristic property of each kind (§3.4).
    characteristic: Vec<usize>,
}

fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let spec: HashMap<&str, u32> = TYPED.iter().copied().collect();
        let mut names: HashMap<String, u32> = HashMap::new();
        let mut types = HashMap::new();
        let labels = LootPids::LABELS
            .iter()
            .chain(CraftPids::LABELS)
            .chain(VendorPids::LABELS)
            .chain(TYPED.iter().map(|(l, _)| l));
        let mut next = 1u32;
        for label in labels {
            if names.contains_key(*label) {
                continue;
            }
            // A label without a spec type is a harness bug: fail at once, on any input.
            let code = *spec
                .get(label)
                .unwrap_or_else(|| panic!("TYPED lacks `{label}`"));
            names.insert((*label).to_owned(), next);
            types.insert(next, code);
            next += 1;
        }
        let reader = EconomyReader::from_parts(EconomyPids::resolve(&names), EconomyDefaults::SPEC);
        let typed_pids = TYPED.iter().map(|(l, _)| names[*l]).collect();
        let characteristic = EconomyKind::ALL
            .iter()
            .map(|k| {
                let label = k.characteristic(&reader.pids).label();
                TYPED
                    .iter()
                    .position(|(l, _)| *l == label)
                    .unwrap_or_else(|| panic!("TYPED lacks `{label}`"))
            })
            .collect();
        Fixture {
            reader,
            types,
            typed_pids,
            characteristic,
        }
    })
}

fn type_of(pid: u32) -> u32 {
    TYPES[(pid as usize) % TYPES.len()]
}

/// A family type other than `code`, chosen by `n`.
fn other_type(code: u32, n: u32) -> u32 {
    let at = TYPES.iter().position(|&c| c == code).unwrap_or(0);
    let step = 1 + (n % 9) as usize;
    TYPES[(at + step) % TYPES.len()]
}

/// The input bytes as a stream of choices; 0 once exhausted.
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
const BUILD_BUDGET: usize = 256;
/// Labels after a struct or array in [`TYPED`] from which its fields or elements are drawn:
/// the spec's tables, and so [`TYPED`], list an array's element and fields right after it.
const CHILD_WINDOW: usize = 12;

/// Builds an entry of label `TYPED[i]` with its spec type (with `mismatch`, about one value
/// in eight gets another type), from the choices in `c`.
fn build_entry(
    f: &Fixture,
    c: &mut Choices<'_>,
    i: usize,
    depth: usize,
    budget: &mut usize,
    mismatch: bool,
) -> PropertyEntry {
    let pid = f.typed_pids.get(i).copied().unwrap_or(1);
    let mut code = TYPED.get(i).map_or(15, |t| t.1);
    if mismatch && c.byte().is_multiple_of(8) {
        code = other_type(code, u32::from(c.byte()));
    }
    let b = c.byte();
    let value = match code {
        11 | 12 => {
            let mut children = Vec::new();
            if depth < 4 {
                for _ in 0..(b % 7) {
                    if *budget == 0 {
                        break;
                    }
                    *budget -= 1;
                    // Array elements are mostly the label that follows the array (its element);
                    // struct fields, and the other elements, come from the window after it.
                    let pick = c.byte();
                    let j = if code == 12 && !pick.is_multiple_of(4) {
                        i + 1
                    } else {
                        i + 1 + usize::from(pick) % CHILD_WINDOW
                    };
                    if j < TYPED.len() {
                        children.push(build_entry(f, c, j, depth + 1, budget, mismatch));
                    }
                }
            }
            if code == 11 {
                structure(children)
            } else {
                array(children)
            }
        }
        8 => PropertyValue::Enum(u32::from(b % 24)),
        9 => PropertyValue::Float(f32::from(b) / 16.0 - 2.0),
        10 => PropertyValue::PropertyId(u32::from(b % 64) + 1),
        13 => PropertyValue::StringInfo(Box::new(StringInfo {
            source: StringSource::Table {
                token: u32::from(b),
                table: Did(0x2500_0001),
            },
            args: None,
        })),
        18 => PropertyValue::Bitfield32(1u32 << (b % 32)),
        20 => did(if b.is_multiple_of(9) {
            0
        } else {
            0x7000_0000 | u32::from(b % 9)
        }),
        21 => PropertyValue::Bool(b & 1 == 1),
        // 15 and anything else: a small signed int, 160 and 161 included.
        _ => PropertyValue::Int(match b {
            250 => 160,
            251 => 161,
            252 => i32::MAX,
            253 => i32::MIN,
            _ => i32::from(b as i8),
        }),
    };
    entry(pid, value)
}

/// A set built from the choices: the characteristic property of a kind, then up to seven
/// neighbouring labels of [`TYPED`].
fn build_set(f: &Fixture, c: &mut Choices<'_>, mismatch: bool) -> PropertySet {
    let mut budget = BUILD_BUDGET;
    let pick = usize::from(c.byte()) % f.characteristic.len().max(1);
    let main = f.characteristic.get(pick).copied().unwrap_or(0);
    let mut entries = vec![build_entry(f, c, main, 0, &mut budget, mismatch)];
    for _ in 0..(c.byte() % 8) {
        let j = (main + usize::from(c.byte()) % 33).saturating_sub(16);
        if j < TYPED.len() {
            entries.push(build_entry(f, c, j, 0, &mut budget, mismatch));
        }
    }
    set(entries)
}

fuzz_target!(|input: &[u8]| {
    // Native codecs.
    if let Ok(t) = WeightedTable::parse(input) {
        assert_eq!(WeightedTable::parse(&t.to_bytes()).as_ref(), Ok(&t));
        let _ = t.check();
        let _ = t.indices_hold();
        let rebuilt = WeightedTable::from_weights(&t.weights());
        assert!(rebuilt.indices_hold());
        // Sums saturate at u32::MAX; below that they must hold exactly.
        let sum: u64 = t.weights().iter().map(|&w| u64::from(w)).sum();
        if sum <= u64::from(u32::MAX) {
            assert!(rebuilt.sums_hold());
            assert_eq!(u64::from(rebuilt.total()), sum);
        }
        let total = u64::from(rebuilt.total());
        if total > 0 {
            assert!(cumulative_pick(&t.weights(), total - 1).is_some());
        }
    }
    if let Some(head) = input.get(..BARTER_ENTRY_LEN)
        && let Ok(e) = BarterEntry::parse(head)
    {
        assert_eq!(BarterEntry::parse(&e.to_bytes()), Ok(e));
    }

    let [selector, rest @ ..] = input else {
        return;
    };
    let selector = u32::from(*selector);
    let mismatch = selector >= 0x80;
    let f = fixture();

    // 1. The rest decoded as a property set with the selected schema.
    let schema = |pid: u32| {
        let code = match f.types.get(&pid) {
            Some(&spec) if !mismatch || !(pid ^ selector).is_multiple_of(4) => spec,
            Some(&spec) => other_type(spec, pid.wrapping_add(selector)),
            None => type_of(pid),
        };
        Some(PropertyType::from_code(code))
    };
    if let Ok(set) = PropertySet::parse(rest, &schema) {
        exercise(f, &set, selector);
    }

    // 2. A set built from the rest: well-typed entries of the kinds' own labels.
    let mut choices = Choices { data: rest, at: 0 };
    let built = build_set(f, &mut choices, mismatch);
    exercise(f, &built, selector);
});

/// Reads `set` as every kind, checks it, and links it under its own DID and under the DIDs
/// it names (so the loot graph gets self-references and cycles), then walks the links.
fn exercise(f: &Fixture, set: &PropertySet, selector: u32) {
    let reader = &f.reader;
    let mut decoded = Vec::new();
    for kind in EconomyKind::ALL {
        let Ok(rec) = reader.decode_as(kind, set) else {
            continue;
        };
        let _ = rec.check();
        let _ = rec.drop_frequency_keys();
        match &rec {
            EconomyRecord::TrophyList(t) => {
                for e in &t.entries {
                    let _ = e.mirror();
                }
            }
            EconomyRecord::TreasureGroupProfile(t) => {
                for e in &t.entries {
                    let _ = e.mirror();
                }
                let w = t.weights();
                let table = WeightedTable::from_weights(&w);
                assert!(table.indices_hold());
                let _ = cumulative_pick(&w, u64::from(table.total()) / 2);
            }
            EconomyRecord::WeightedTreasureTable(t) => {
                let sorted = t.sorted_by_weight();
                assert_eq!(sorted.len(), t.entries.len());
            }
            EconomyRecord::FilteredTreasureList(t) => {
                for e in &t.entries {
                    for filter in &e.filters {
                        let _ = filter.tested_property();
                    }
                }
            }
            EconomyRecord::LevelBasedLootTable(t) => {
                let (a, b) = t.mirror_maps();
                assert!(a.len() <= t.entries.len() && b.len() <= t.entries.len());
                let _ = t.at_level(1);
            }
            EconomyRecord::SpeciesLevelLootTable(t) => {
                assert!(t.mirror_map().len() <= t.entries.len());
            }
            EconomyRecord::PvpLevelLootTable(t) => {
                for e in &t.entries {
                    let _ = e.key();
                }
            }
            EconomyRecord::HobbyRewardProfile(t) => {
                for e in &t.entries {
                    let _ = e.mirror();
                }
            }
            EconomyRecord::LootControl(c) => {
                let _ = c.drop_fraction(1);
            }
            EconomyRecord::Recipe(r) => {
                let _ = r.variant();
                let _ = r.tiers();
                let _ = r.produced_items();
                let _ = r.used_items();
            }
            EconomyRecord::Profession(p) => {
                let _ = p.required_tool_keys();
                let _ = p.recipes().count();
            }
            EconomyRecord::CraftControl(c) => {
                let _ = c.xp_value(1);
            }
            EconomyRecord::BarterProfile(b) => {
                for l in &b.lists {
                    let _ = l.shape();
                    let _ = l.sound_category();
                    if let Some(e) =
                        l.expected_entry(Did(0x7000_0001), |c| c.quantity.map(|q| q as u32))
                    {
                        assert_eq!(BarterEntry::parse(&e.to_bytes()), Ok(e));
                    }
                }
            }
            _ => {}
        }
        decoded.push(rec);
    }
    if let Some(EconomyRecord::LootControl(control)) = decoded
        .iter()
        .find(|r| matches!(r, EconomyRecord::LootControl(_)))
    {
        for rec in &decoded {
            let _ = check_drop_frequencies(rec, control);
        }
    }
    let inventory = decoded.iter().find_map(|r| match r {
        EconomyRecord::VendorInventory(v) => Some(v),
        _ => None,
    });
    if let Ok(Some(v)) = reader.npc_vendor(set) {
        let _ = v.item_list_matches(|_| inventory);
    }
    let _ = reader.store_links(set);
    let _ = reader.taught_recipe(set);
    let _ = reader.reward_level(set, true);
    let _ = reader.reward_level(set, false);
    if let Ok(p) = reader.float_progression(set) {
        for level in [i64::MIN, -1, 0, 1, 160, i64::MAX] {
            let _ = lookup_quantity(&p, level);
        }
    }

    // Kinds by class and cross-links.
    let kind = reader.kind(set);
    let mut by_class = KindByClass::new();
    if let Some(k) = kind {
        by_class.learn(selector, k);
        let _ = by_class.kind(selector);
        let _ = by_class.classes_of(k);
    }
    let record = kind.and_then(|k| reader.decode_as(k, set).ok());
    let own = Did(0x7000_0001);
    let mut dids = vec![own];
    dids.extend(did_references(set).iter().map(|r| r.target).take(8));
    let mut links = CrossLinks::new();
    if let Some(rec) = &record {
        for &d in &dids {
            links.note_kind(d, rec.kind());
        }
    }
    for &d in &dids {
        let _ = links.add(reader, d, set, record.as_ref());
    }
    // The same set again under another DID, as a record that is not an economy record.
    let _ = links.add(reader, Did(0x7000_00FF), set, None);
    let stats = links.loot_graph();
    assert!(stats.tables <= dids.len());
    for &d in &dids {
        for s in links.drops_from(d, DROPS_FROM_DEPTH) {
            assert!(s.depth <= DROPS_FROM_DEPTH);
            assert!(s.path.len() <= s.depth);
        }
        let _ = links.sold_by(d);
        let _ = links.bartered_for(d);
        let _ = links.barter_currency_in(d);
        let _ = links.traders(d);
        let _ = links.crafted_by(d);
        let _ = links.used_in(d);
        let _ = links.learned_from(d);
        let _ = links.offered_in_store(d);
    }
}
