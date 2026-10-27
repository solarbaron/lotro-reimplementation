// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the economy views (`tessera_formats::gameplay::economy`) against a
//! user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Walks **every** object record:
//! reads its root class and property set, classifies it by characteristic property
//! (`docs/specs/economy.md` §3.4), decodes every economy record and checks
//!
//! - the census of §2 and §3.4 (records per kind and root class) and the entry counts of
//!   §4–§7;
//! - the invariants E-I1 to E-I15 of §12, including the object-form mirrors of §9.2 and §9.3
//!   (class-175 tables and class-1942 barter entries, with the look-up quantities of §6.2);
//! - the cross-links of §10.
//!
//! The census figures are observations of one install (§1): here the owner's install as
//! patched on 2026-10-27 (`economy.md` states the figures of the install before that patch
//! until it is re-measured). After a game update they may
//! change: `TESSERA_ECONOMY_CENSUS=report` prints census differences instead of failing

//! (invariants still fail). Only counts and error kinds are printed; no game data leaves the
//! install. Run with
//! `cargo test --release -p tessera-formats --test gameplay_economy_real_data -- --nocapture`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use tessera_formats::gameplay::common::{
    ObjectRoot, PropertyAccess, decode_object_properties, did_references,
};
use tessera_formats::gameplay::economy::{
    BarterElement, BarterEntry, BarterReward, BarterShape, CrossLinks, DROPS_FROM_DEPTH,
    EconomyKind, EconomyReader, EconomyRecord, EntityFilter, FloatProgression, Invariant,
    KindByClass, NpcVendor, Violation, WeightedElement, WeightedTable, check_drop_frequencies,
    lookup_quantity,
};
use tessera_formats::{Did, PropertyDictionary, PropertySet, PropertySetRecord};
use tessera_wstate::{Native, ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn census_is_strict() -> bool {
    std::env::var_os("TESSERA_ECONOMY_CENSUS").is_none_or(|v| v != "report")
}

/// Census differences, collected so that one run reports all of them.
#[derive(Default)]
struct Census {
    diffs: Vec<String>,
    checked: usize,
}

impl Census {
    fn eq(&mut self, what: &str, got: usize, expected: usize) {
        self.checked += 1;
        if got != expected {
            self.diffs.push(format!("{what}: {got} (spec {expected})"));
        }
    }

    fn finish(self) {
        eprintln!(
            "economy census: {} figures checked, {} differ",
            self.checked,
            self.diffs.len()
        );
        for d in &self.diffs {
            eprintln!("  census: {d}");
        }
        if census_is_strict() {
            assert!(self.diffs.is_empty(), "census differs from economy.md");
        }
    }
}

/// One record after the first pass.
struct Rec {
    did: Did,
    class: u32,
    kind: Option<EconomyKind>,
    record: Option<EconomyRecord>,
    set: Option<PropertySet>,
    /// Non-economy records: distinct 0x70 targets of their DID values.
    targets: Vec<Did>,
    /// Every non-null 0x70 DID value of the set, repeats kept (§4.13 counts references).
    refs: Vec<Did>,
    /// Non-economy records with vendor, recipe-item or store links.
    linked: bool,
    /// How many characteristic properties the set holds (§3.4: at most one).
    characteristic: usize,
}

fn read_set(store: &ObjectStore, dict: &PropertyDictionary, did: Did) -> Option<PropertySet> {
    let root = ObjectRoot { did, root_class: 0 };
    let set_did = root.property_set_did()?;
    let bytes = store
        .game_data()
        .read(set_did)
        .expect("read property set")?;
    Some(decode_object_properties(did, &bytes, dict).unwrap_or_else(|e| panic!("{did}: {e}")))
}

fn first_pass(
    store: &ObjectStore,
    dict: &PropertyDictionary,
    reader: &EconomyReader,
    dids: &[Did],
) -> Vec<Rec> {
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .clamp(1, 8);
    let chunk = dids.len().div_ceil(threads).max(1);
    let out = Mutex::new(Vec::with_capacity(dids.len()));
    std::thread::scope(|scope| {
        for part in dids.chunks(chunk) {
            let out = &out;
            scope.spawn(move || {
                let mut local = Vec::with_capacity(part.len());
                for &did in part {
                    let payload = store
                        .game_data()
                        .read(did)
                        .expect("read object record")
                        .expect("listed record exists");
                    let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
                    let set = read_set(store, dict, did);
                    let empty = PropertySet::default();
                    let props = set.as_ref().unwrap_or(&empty);
                    let characteristic = EconomyKind::ALL
                        .iter()
                        .filter(|k| props.has(k.characteristic(&reader.pids)))
                        .count();
                    let kind = reader.kind(props);
                    // E-I1: every economy record decodes.
                    let record = kind.map(|k| {
                        reader
                            .decode_as(k, props)
                            .unwrap_or_else(|e| panic!("E-I1 {did}: {e}"))
                    });
                    let refs: Vec<Did> = did_references(props)
                        .into_iter()
                        .map(|r| r.target)
                        .filter(|t| t.prefix() == 0x70)
                        .collect();
                    let mut targets = Vec::new();
                    let mut linked = false;
                    if kind.is_none() && set.is_some() {
                        let mut seen = HashSet::new();
                        for &t in &refs {
                            if seen.insert(t) {
                                targets.push(t);
                            }
                        }
                        linked = reader.npc_vendor(props).expect("NPC vendor").is_some()
                            || reader.taught_recipe(props).expect("recipe item").is_some()
                            || reader
                                .store_links(props)
                                .expect("store links")
                                .inventory_item
                                .is_some();
                    }
                    local.push(Rec {
                        did,
                        class: root.root_class,
                        kind,
                        record,
                        set: if kind.is_some() { set } else { None },
                        targets,
                        refs,
                        linked,
                        characteristic,
                    });
                }
                out.lock().expect("lock").extend(local);
            });
        }
    });
    let mut v = out.into_inner().expect("lock");
    v.sort_by_key(|r| r.did);
    v
}

/// §2 / §3.4: kind → (root class, records).
const CENSUS: &[(EconomyKind, &[(u32, usize)])] = &[
    (
        EconomyKind::TrophyList,
        &[(1237, 15_227), (1917, 179), (1918, 47)],
    ),
    (
        EconomyKind::TreasureGroupProfile,
        &[(1235, 6_418), (2372, 16)],
    ),
    (EconomyKind::TreasureList, &[(1236, 894)]),
    (EconomyKind::WeightedTreasureTable, &[(3307, 1_007)]),
    (EconomyKind::FilteredTreasureList, &[(3666, 927)]),
    (EconomyKind::LevelBasedLootTable, &[(3669, 121)]),
    (EconomyKind::SpeciesLevelLootTable, &[(2001, 23)]),
    (EconomyKind::PvpLevelLootTable, &[(3671, 1)]),
    (EconomyKind::WorldEventLevelLootTable, &[(2127, 11)]),
    (EconomyKind::WorldEventLootTableDirectory, &[(2128, 1)]),
    (EconomyKind::RunicLevelLootTable, &[(2227, 15)]),
    (EconomyKind::RunicList, &[(2229, 169)]),
    (EconomyKind::RunicTreasureGroup, &[(2239, 34)]),
    (EconomyKind::SkirmishTreasureLookup, &[(2728, 8_326)]),
    (EconomyKind::SkirmishCustomLootLookupTable, &[(2729, 7)]),
    (EconomyKind::HobbyRewardProfile, &[(2153, 127)]),
    (EconomyKind::LootControl, &[(1673, 1)]),
    (EconomyKind::LootDirectoryControl, &[(1674, 1)]),
    (EconomyKind::Recipe, &[(1024, 7_791)]),
    (EconomyKind::Profession, &[(1023, 10)]),
    (EconomyKind::Vocation, &[(1026, 7)]),
    (EconomyKind::CraftGuild, &[(2278, 7)]),
    (EconomyKind::CraftControl, &[(1665, 1)]),
    (EconomyKind::CraftDirectory, &[(1021, 1)]),
    (EconomyKind::BarterProfile, &[(1943, 3_449)]),
    (EconomyKind::VendorInventory, &[(1619, 400)]),
    (EconomyKind::PerkVendor, &[(1796, 2)]),
    (EconomyKind::Currency, &[(1028, 3)]),
    (EconomyKind::CurrencyDirectory, &[(1029, 1)]),
    (EconomyKind::CurrencyControl, &[(1996, 1)]),
];

/// Root classes of the item records the economy tables name (§1).
const ITEMS: &[u32] = &[795, 796, 797, 798, 799, 802, 803, 805, 2814, 4178];
/// Items and the class-794 records that recipes name as results (§1).
const ITEMS_AND_794: &[u32] = &[794, 795, 796, 797, 798, 799, 802, 803, 805, 2814, 4178];
/// Trophy-list targets (§4.1): items, treasure group profiles and treasure lists.
const TROPHY_TARGETS: &[u32] = &[
    795, 796, 797, 798, 799, 802, 803, 805, 2814, 4178, 1235, 1236, 2372,
];

/// E-I2: the target classes of each DID field of §4–§8 in the economy records. A reference
/// is matched by its own label, else by the label of the array or struct that holds it (for
/// arrays whose element label the spec does not name). Fields whose targets the spec gives
/// as 0 in all have no class. References inside `DefaultPermissionBlobStruct` (usage
/// requirements, whose targets the spec does not list field by field) and references of
/// unlisted fields are checked for existence only.
const ECONOMY_TARGETS: &[(&str, &[u32])] = &[
    ("LootGen_TrophyList_ItemOrProfile", TROPHY_TARGETS),
    ("TreasureGroupProfile_ItemTable_Item", ITEMS),
    ("LootGen_TreasureList_GroupProfile", &[1235]),
    ("LootGen_WeightedTreasureTable_TrophyList", &[1237]),
    ("LootGen_FilteredTrophyTable_EntryDID", &[1237, 3307]),
    ("WorldEvent_WorldEvent", &[1815, 1887]),
    ("LevelBasedLootTable_TrophyList", &[1237]),
    ("LevelBasedLootTable_TreasureProfile", &[1236]),
    ("LevelBasedLootTable_LevelTable", &[3669]),
    ("WorldEventLevelLootTable_TrophyListTemplate", &[1237, 1917]),
    ("WorldEventLootTableDirectory_LevelTable", &[2127]),
    ("RunicLevelLootTable_TrophyList", &[1237]),
    ("RunicLevelLootTable_RunicTrophyList", &[2229]),
    ("RunicList_RunicOrTreasureGroup", &[2219, 2239]),
    ("RunicTreasureGroup_Runic", &[2219]),
    ("SkirmishTreasureLookup_EncounterDID", &[2651]),
    ("SkirmishTreasureLookup_SkirmishMatchParametersDID", &[2727]),
    ("SkirmishTreasureLookup_TrophyListTemplateDID", &[1237]),
    ("SkirmishTreasureLookup_TreasureListTemplateDID", &[]),
    ("SkirmishTreasureLookup_BarterTrophyListDID", &[]),
    ("SkirmishTreasureLookup_ReputationTrophyListDID", &[]),
    ("SkirmishCustomLootLookupTable_Array", &[2728]),
    ("HobbyRewardProfile_AdvancementProgression", &[1385]),
    ("HobbyRewardProfile_Item", ITEMS),
    ("LootGenControl_SpeciesLevelLootTableArray", &[2001]),
    ("LootGenControl_PVPLevelLootTable", &[3671]),
    ("CraftRecipe_Profession", &[1023]),
    ("CraftRecipe_Ingredient", ITEMS),
    ("CraftRecipe_IngredientPack", ITEMS),
    ("CraftRecipe_ResultItem", ITEMS_AND_794),
    ("CraftRecipe_CriticalResultItem", ITEMS_AND_794),
    ("CraftRecipe_XPAward_Progression", &[1385]),
    ("CraftRecipe_RequiredCraftGuild", &[2278]),
    ("CraftRecipe_RequiredFacilityEffect", &[718]),
    ("CraftRecipe_InductionAction", &[1195]),
    ("EffectGenerator_EffectID", &[715, 745]),
    ("CraftProfession_RecipeArray", &[1024]),
    ("CraftProfession_ProficiencyTitle", &[1470]),
    ("CraftProfession_MasteryTitle", &[1470]),
    (
        "CraftProfession_CompletedProficiencyLevelCharacteristic",
        &[1477],
    ),
    (
        "CraftProfession_CompletedMasteryLevelCharacteristic",
        &[1477],
    ),
    ("CraftProfession_TierOpeningCharacteristic", &[1477]),
    ("CraftProfession_Item", &[796]),
    ("CraftProfession_Characteristic", &[1483]),
    ("WebStoreAccountItem_DataID", &[]),
    ("CraftVocation_ProfessionArray", &[1023]),
    ("CraftVocation_ItemArray", &[796, 798]),
    ("CraftGuild_Profession", &[1023]),
    ("CraftGuild_Faction", &[1889]),
    ("CraftControl_Profession", &[1023]),
    ("CraftControl_RemoveQuestsArray", &[1398]),
    ("CraftDirectory_VocationArray", &[1026]),
    ("CraftDirectory_ProfessionArray", &[1023]),
    ("CraftDirectory_CraftGuildArray", &[2278]),
    ("Barter_Item", ITEMS),
    ("Barter_ItemQuantity_LookupTable", &[1385]),
    ("Barter_ReputationFaction", &[1889]),
    ("VendorInventory_Items", ITEMS),
    ("PerkVendor_InventoryList", &[1377]),
    ("Currency_CurrencyList", &[1028]),
    ("Currency_ObjectDID", &[796]),
    ("Currency_Control_GoldCapWebstoreItem", &[2784]),
];

/// E-I2 for the vendor fields of NPC records (§6.3), matched like [`ECONOMY_TARGETS`].
const VENDOR_TARGETS: &[(&str, &[u32])] = &[
    ("Vendor_InventoryList", &[1619]),
    ("Vendor_ItemList", ITEMS),
    ("Barter_ProfileArray", &[1943]),
    ("Craft_OfferedProfessionArray", &[1023]),
    ("Craft_OfferedCraftGuildArray", &[2278]),
    ("Craft_OfferedVocationArray", &[1026]),
];

/// §4.13: references into the loot family by (target class, holder class), each the sum of
/// the figures the spec gives for that holder; the row totals are the References column.
/// (target class, [(holder class, references)], row total).
type AttachedRow = (u32, &'static [(u32, usize)], usize);

const ATTACHED: &[AttachedRow] = &[
    (
        1237,
        &[
            (1723, 1_949),
            (794, 1_300),
            (2814, 3_393),
            (795, 600),
            (802, 246),
            (799, 126),
            (797, 16),
            (796, 4),
            (2219, 706),
            (2301, 9),
            (3307, 4_335),
            (3666, 10_302),
            (3669, 16_185),
            (2127, 1_063),
            (2227, 1_200),
            (2728, 8_326),
        ],
        49_760,
    ),
    (
        1917,
        &[(1723, 333), (794, 109), (2814, 1), (2127, 160)],
        603,
    ),
    (1918, &[(1723, 1_088), (794, 1)], 1_089),
    (
        1236,
        &[
            (794, 668),
            (1723, 45),
            (776, 327),
            (1389, 160),
            (2426, 5),
            (1237, 7_568),
            (3669, 3_825),
        ],
        12_598,
    ),
    (1235, &[(1237, 14_546), (1236, 2_299)], 16_845),
    (
        3666,
        &[(794, 2_618), (1723, 157), (2814, 275), (3787, 4)],
        3_054,
    ),
    (3307, &[(3666, 1_316), (2814, 57), (3731, 6)], 1_379),
    (2001, &[(1674, 4), (1368, 21), (2651, 12)], 37),
    (2729, &[(794, 99)], 99),
    (2153, &[(2131, 120), (734, 52)], 172),
    (2227, &[(2321, 453)], 453),
    (2229, &[(2227, 1_050), (796, 136), (2301, 18)], 1_204),
];

/// A field of the target-class census: holder class, top-level label, label of the struct
/// directly holding the value (`None`: any), label of the value (`None`: any), and the
/// expected references per target class (§4–§7 "Observed" columns).
type FieldCensus = (
    u32,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static [(u32, usize)],
);

const FIELD_TARGETS: &[FieldCensus] = &[
    (
        1237,
        "LootGen_TrophyList",
        None,
        Some("LootGen_TrophyList_ItemOrProfile"),
        &[
            (796, 41_767),
            (795, 5_663),
            (802, 3_088),
            (2814, 2_175),
            (799, 1_727),
            (797, 299),
            (805, 273),
            (4178, 62),
            (798, 20),
            (803, 12),
            (1235, 14_546),
            (1236, 7_568),
        ],
    ),
    (
        1917,
        "LootGen_TrophyList",
        None,
        Some("LootGen_TrophyList_ItemOrProfile"),
        &[(796, 415), (2372, 21)],
    ),
    (
        1918,
        "LootGen_TrophyList",
        None,
        Some("LootGen_TrophyList_ItemOrProfile"),
        &[(796, 121)],
    ),
    (
        1235,
        "TreasureGroupProfile_ItemTable",
        None,
        Some("TreasureGroupProfile_ItemTable_Item"),
        &[
            (795, 35_653),
            (802, 28_448),
            (796, 25_101),
            (799, 18_780),
            (2814, 3_433),
            (805, 2_742),
            (797, 2_496),
            (798, 50),
            (803, 3),
        ],
    ),
    (
        2372,
        "TreasureGroupProfile_ItemTable",
        None,
        Some("TreasureGroupProfile_ItemTable_Item"),
        &[(796, 51), (805, 7)],
    ),
    (
        1236,
        "LootGen_TreasureList",
        None,
        Some("LootGen_TreasureList_GroupProfile"),
        &[(1235, 2_299)],
    ),
    (
        3307,
        "LootGen_WeightedTreasureTable",
        None,
        Some("LootGen_WeightedTreasureTable_TrophyList"),
        &[(1237, 4_335)],
    ),
    (
        3666,
        "LootGen_FilteredTreasureListArray",
        None,
        Some("LootGen_FilteredTrophyTable_EntryDID"),
        &[(1237, 10_302), (3307, 1_316)],
    ),
    (
        3669,
        "LevelBasedLootTable_LootTable",
        None,
        Some("LevelBasedLootTable_TrophyList"),
        &[(1237, 16_185)],
    ),
    (
        3669,
        "LevelBasedLootTable_LootTable",
        None,
        Some("LevelBasedLootTable_TreasureProfile"),
        &[(1236, 3_825)],
    ),
    (
        2001,
        "SpeciesLevelLootTable",
        None,
        Some("LevelBasedLootTable_LevelTable"),
        &[(3669, 264)],
    ),
    (
        3671,
        "PVPLevelLootTable",
        None,
        Some("LevelBasedLootTable_LevelTable"),
        &[(3669, 15)],
    ),
    (
        2127,
        "WorldEventLevelLootTable_LootTable",
        None,
        Some("WorldEventLevelLootTable_TrophyListTemplate"),
        &[(1237, 1_063), (1917, 160)],
    ),
    (
        2128,
        "WorldEventLootTableDirectory_Array",
        None,
        Some("WorldEventLootTableDirectory_LevelTable"),
        &[(2127, 11)],
    ),
    (
        2128,
        "WorldEventLootTableDirectory_Array",
        None,
        Some("WorldEvent_WorldEvent"),
        &[(1887, 9), (1815, 3)],
    ),
    (
        2227,
        "RunicLevelLootTable_LootTable",
        None,
        Some("RunicLevelLootTable_TrophyList"),
        &[(1237, 1_200)],
    ),
    (
        2227,
        "RunicLevelLootTable_LootTable",
        None,
        Some("RunicLevelLootTable_RunicTrophyList"),
        &[(2229, 1_050)],
    ),
    (
        2229,
        "RunicList_Array",
        None,
        Some("RunicList_RunicOrTreasureGroup"),
        &[(2219, 469), (2239, 373)],
    ),
    (
        2239,
        "RunicTreasureGroup_Array",
        None,
        Some("RunicTreasureGroup_Runic"),
        &[(2219, 450)],
    ),
    (
        2728,
        "SkirmishTreasureLookup_EncounterDID",
        None,
        None,
        &[(2651, 8_326)],
    ),
    (
        2728,
        "SkirmishTreasureLookup_SkirmishMatchParametersDID",
        None,
        None,
        &[(2727, 8_326)],
    ),
    (
        2728,
        "SkirmishTreasureLookup_TrophyListTemplateDID",
        None,
        None,
        &[(1237, 8_326)],
    ),
    (
        2729,
        "SkirmishCustomLootLookupTable_Array",
        None,
        None,
        &[(2728, 8_326)],
    ),
    (
        2153,
        "HobbyRewardProfile_ItemArray",
        None,
        Some("HobbyRewardProfile_Item"),
        &[(796, 2_688), (2814, 1)],
    ),
    (
        2153,
        "HobbyRewardProfile_AdvancementProgression",
        None,
        None,
        &[(1385, 127)],
    ),
    (
        1674,
        "LootGenControl_SpeciesLevelLootTableArray",
        None,
        None,
        &[(2001, 4)],
    ),
    (
        1674,
        "LootGenControl_PVPLevelLootTable",
        None,
        None,
        &[(3671, 1)],
    ),
    (
        1024,
        "CraftRecipe_IngredientList",
        None,
        Some("CraftRecipe_Ingredient"),
        &[(796, 20_335), (795, 122), (802, 82), (799, 4), (797, 2)],
    ),
    (
        1024,
        "CraftRecipe_OptionalIngredientList",
        None,
        Some("CraftRecipe_Ingredient"),
        &[(796, 8_017)],
    ),
    (
        1024,
        "CraftRecipe_ResultItem",
        None,
        None,
        &[
            (796, 3_607),
            (799, 1_926),
            (795, 1_435),
            (802, 413),
            (797, 171),
            (2814, 157),
            (794, 79),
            (803, 3),
        ],
    ),
    (1024, "CraftRecipe_Profession", None, None, &[(1023, 7_791)]),
    (
        1024,
        "CraftRecipe_RequiredCraftGuild",
        None,
        None,
        &[(2278, 2_184)],
    ),
    (
        1024,
        "CraftRecipe_IngredientPack",
        None,
        None,
        &[(796, 4_520)],
    ),
    (
        1024,
        "CraftRecipe_XPAward_Progression",
        None,
        None,
        &[(1385, 7_791)],
    ),
    (
        1023,
        "CraftProfession_TierArray",
        None,
        Some("CraftProfession_Recipe"),
        &[(1024, 1_733)],
    ),
    (1023, "CraftProfession_Item", None, None, &[(796, 10)]),
    (
        1023,
        "CraftProfession_Characteristic",
        None,
        None,
        &[(1483, 4)],
    ),
    (
        1026,
        "CraftVocation_ProfessionArray",
        None,
        None,
        &[(1023, 21)],
    ),
    (
        1026,
        "CraftVocation_ItemArray",
        None,
        None,
        &[(796, 21), (798, 7)],
    ),
    (2278, "CraftGuild_Profession", None, None, &[(1023, 7)]),
    (
        1943,
        "Barter_ItemListArray",
        Some("Barter_ItemStruct"),
        Some("Barter_Item"),
        &[
            (796, 54_862),
            (795, 3_589),
            (802, 123),
            (799, 8),
            (803, 4),
            (797, 3),
        ],
    ),
    (
        1943,
        "Barter_ItemListArray",
        Some("Barter_RewardItemStruct"),
        Some("Barter_Item"),
        &[
            (796, 20_844),
            (795, 13_589),
            (802, 3_742),
            (805, 1_524),
            (799, 1_307),
            (2814, 841),
            (797, 354),
            (4178, 21),
            (803, 20),
        ],
    ),
    (
        1943,
        "Barter_ItemListArray",
        Some("Barter_ItemStruct"),
        Some("Barter_ItemQuantity_LookupTable"),
        &[(1385, 5_565)],
    ),
    (
        1619,
        "VendorInventory_Items",
        None,
        None,
        &[
            (796, 5_605),
            (805, 4_078),
            (795, 2_407),
            (799, 1_030),
            (2814, 473),
            (802, 246),
            (797, 174),
            (798, 1),
        ],
    ),
    (1796, "PerkVendor_InventoryList", None, None, &[(1377, 72)]),
    (1029, "Currency_CurrencyList", None, None, &[(1028, 3)]),
    (1028, "Currency_ObjectDID", None, None, &[(796, 3)]),
    (
        1996,
        "Currency_Control_GoldCapWebstoreItem",
        None,
        None,
        &[(2784, 1)],
    ),
];

/// Resolves the labels of a target table to PIDs; every label must be in the dictionary.
fn target_table(
    dict: &PropertyDictionary,
    table: &[(&'static str, &'static [u32])],
) -> HashMap<u32, (&'static str, &'static [u32])> {
    table
        .iter()
        .map(|&(label, classes)| {
            let pid = dict
                .pid(label)
                .unwrap_or_else(|| panic!("`{label}` not in the dictionary"));
            (pid, (label, classes))
        })
        .collect()
}

/// E-I2 over one set: dangling references and references to a class the table does not
/// list for the field (by label). `skip_top` subtrees are checked for existence only.
#[allow(clippy::too_many_arguments)]
fn check_targets(
    set: &PropertySet,
    table: &HashMap<u32, (&'static str, &'static [u32])>,
    skip_top: Option<u32>,
    class_of: &HashMap<Did, u32>,
    dangling: &mut usize,
    wrong: &mut BTreeMap<&'static str, usize>,
    classified: &mut usize,
) {
    for r in did_references(set) {
        if r.target.prefix() != 0x70 {
            continue;
        }
        let Some(&class) = class_of.get(&r.target) else {
            *dangling += 1;
            continue;
        };
        if skip_top == Some(r.top) {
            continue;
        }
        let rule = table
            .get(&r.pid)
            .or_else(|| r.parent.and_then(|p| table.get(&p)));
        if let Some(&(label, classes)) = rule {
            *classified += 1;
            if !classes.contains(&class) {
                *wrong.entry(label).or_default() += 1;
            }
        }
    }
}

fn violations(all: &mut BTreeMap<&'static str, usize>, v: &[Violation]) {
    for v in v {
        *all.entry(v.invariant.code()).or_default() += 1;
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn economy_family_census_and_invariants() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let started = std::time::Instant::now();
    let store = ObjectStore::open(&dir).expect("open install");
    let data = store.game_data();
    let dict = data.registry().expect("dictionary").dictionary().clone();
    let reader = EconomyReader::new(&dict);
    assert!(
        reader.pids.missing().is_empty(),
        "labels missing from the dictionary: {:?}",
        reader.pids.missing()
    );
    let dids = object_dids(data.source()).expect("object DIDs");
    let all_dids: HashSet<Did> = dids.iter().copied().collect();
    let mut recs = first_pass(&store, &dict, &reader, &dids);
    eprintln!(
        "economy: {} object records read in {:.1?}",
        recs.len(),
        started.elapsed()
    );
    let mut census = Census::default();
    let mut broken: BTreeMap<&'static str, usize> = BTreeMap::new();

    // §3.4: a set holds at most one characteristic property.
    assert_eq!(recs.iter().filter(|r| r.characteristic > 1).count(), 0);

    // Kinds known only by class (the empty vendor inventories).
    let mut by_class = KindByClass::new();
    for r in &recs {
        if let Some(k) = r.kind {
            by_class.learn(r.class, k);
        }
    }
    let mut by_class_only = 0usize;
    for r in &mut recs {
        if r.kind.is_none()
            && let Some(k) = by_class.kind(r.class)
        {
            let set = read_set(&store, &dict, r.did).unwrap_or_default();
            r.record = Some(reader.decode_as(k, &set).expect("E-I1 by class"));
            r.kind = Some(k);
            r.targets.clear();
            r.set = Some(set);
            by_class_only += 1;
        }
    }
    census.eq("records known only by class", by_class_only, 7);

    // §2 census: records per kind and class, and every record of each class has the kind.
    let mut class_total: HashMap<u32, usize> = HashMap::new();
    let mut per_kind_class: HashMap<(EconomyKind, u32), usize> = HashMap::new();
    for r in &recs {
        *class_total.entry(r.class).or_default() += 1;
        if let Some(k) = r.kind {
            *per_kind_class.entry((k, r.class)).or_default() += 1;
        }
    }
    for &(kind, classes) in CENSUS {
        assert_eq!(
            by_class.classes_of(kind),
            classes.iter().map(|c| c.0).collect::<Vec<_>>(),
            "{kind:?} root classes"
        );
        for &(class, expected) in classes {
            let got = per_kind_class.get(&(kind, class)).copied().unwrap_or(0);
            census.eq(&format!("{kind:?} class {class}"), got, expected);
            assert_eq!(class_total.get(&class).copied(), Some(got), "class {class}");
        }
    }

    // Index the decoded records.
    let economy: Vec<&Rec> = recs.iter().filter(|r| r.record.is_some()).collect();
    let by_did: HashMap<Did, &Rec> = economy.iter().map(|r| (r.did, *r)).collect();
    let of_kind = |k: EconomyKind| economy.iter().filter(move |r| r.kind == Some(k));
    let control = of_kind(EconomyKind::LootControl)
        .find_map(|r| match &r.record {
            Some(EconomyRecord::LootControl(c)) => Some(c.clone()),
            _ => None,
        })
        .expect("loot control");
    // The singletons are reached through their WEENIECONTENT labels (§4.12, §5.4, §7).
    for (label, kind) in [
        ("LootGenControl", EconomyKind::LootControl),
        ("LootGenDirectoryControl", EconomyKind::LootDirectoryControl),
        (
            "WorldEventLootTableDirectory",
            EconomyKind::WorldEventLootTableDirectory,
        ),
        ("CraftControl", EconomyKind::CraftControl),
        ("CraftDirectory", EconomyKind::CraftDirectory),
        ("CurrencyDirectory", EconomyKind::CurrencyDirectory),
        ("CurrencyControl", EconomyKind::CurrencyControl),
        ("GoldCurrency", EconomyKind::Currency),
    ] {
        let did = store.weenie(label).expect("weenie map").expect(label);
        assert_eq!(by_did.get(&did).and_then(|r| r.kind), Some(kind), "{label}");
    }

    // Per-record invariants (E-I3 to E-I10), E-I2 targets, E-I3 control part.
    let class_of: HashMap<Did, u32> = recs.iter().map(|r| (r.did, r.class)).collect();
    assert_eq!(class_of.len(), all_dids.len());
    let economy_targets = target_table(&dict, ECONOMY_TARGETS);
    let vendor_targets = target_table(&dict, VENDOR_TARGETS);
    let permissions = dict.pid("DefaultPermissionBlobStruct");
    let (mut dangling, mut classified) = (0usize, 0usize);
    let mut wrong_class: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut drop_keys = BTreeSet::new();
    for r in &economy {
        let rec = r.record.as_ref().expect("decoded");
        violations(&mut broken, &rec.check());
        violations(&mut broken, &check_drop_frequencies(rec, &control));
        drop_keys.extend(rec.drop_frequency_keys());
        if let Some(set) = &r.set {
            check_targets(
                set,
                &economy_targets,
                permissions,
                &class_of,
                &mut dangling,
                &mut wrong_class,
                &mut classified,
            );
        }
    }
    census.eq("drop-frequency keys used", drop_keys.len(), 47);

    // §4–§7 "Observed": references per field and target class.
    let label_pid = |l: &str| {
        dict.pid(l)
            .unwrap_or_else(|| panic!("`{l}` not in the dictionary"))
    };
    let mut field_refs: HashMap<(u32, u32, Option<u32>, u32, u32), usize> = HashMap::new();
    for r in &economy {
        if let Some(set) = &r.set {
            for t in did_references(set) {
                if let Some(&class) = class_of.get(&t.target) {
                    *field_refs
                        .entry((r.class, t.top, t.parent, t.pid, class))
                        .or_default() += 1;
                }
            }
        }
    }
    for &(holder, top, parent, leaf, expected) in FIELD_TARGETS {
        let (top_pid, parent_pid, leaf_pid) =
            (label_pid(top), parent.map(label_pid), leaf.map(label_pid));
        let mut per_class: BTreeMap<u32, usize> = BTreeMap::new();
        for (&(h, t, p, l, class), &n) in &field_refs {
            if h == holder
                && t == top_pid
                && parent_pid.is_none_or(|x| p == Some(x))
                && leaf_pid.is_none_or(|x| l == x)
            {
                *per_class.entry(class).or_default() += n;
            }
        }
        let what = format!(
            "{holder} {top}{}{} targets",
            parent.map_or(String::new(), |p| format!("/{p}")),
            leaf.map_or(String::new(), |l| format!("/{l}"))
        );
        for &(class, n) in expected {
            census.eq(
                &format!("{what} of class {class}"),
                per_class.get(&class).copied().unwrap_or(0),
                n,
            );
        }
        census.eq(
            &format!("{what}, all classes"),
            per_class.values().sum(),
            expected.iter().map(|e| e.1).sum(),
        );
    }

    // §4.13: references into the loot family by target and holder class, over every
    // object property set.
    let mut attached: HashMap<(u32, u32), usize> = HashMap::new();
    for r in &recs {
        for t in &r.refs {
            if let Some(&class) = class_of.get(t) {
                *attached.entry((class, r.class)).or_default() += 1;
            }
        }
    }
    for &(target, holders, total) in ATTACHED {
        for &(holder, n) in holders {
            census.eq(
                &format!("§4.13 references to {target} from {holder}"),
                attached.get(&(target, holder)).copied().unwrap_or(0),
                n,
            );
        }
        census.eq(
            &format!("§4.13 references to {target}"),
            attached
                .iter()
                .filter(|((t, _), _)| *t == target)
                .map(|(_, n)| n)
                .sum(),
            total,
        );
    }

    // §3.4, §4.13: no stand-alone 0x78 set holds a characteristic property or references the
    // loot family.
    let loot_classes: HashSet<u32> = ATTACHED.iter().map(|a| a.0).collect();
    let (mut bare_sets, mut bare_characteristic, mut bare_loot_refs) = (0usize, 0usize, 0usize);
    let mut seen_bare = HashSet::new();
    for m in data.source().members() {
        for e in m.archive.entries_range(0x7800_0000..0x7900_0000) {
            let e = e.expect("directory entry");
            if !seen_bare.insert(e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read 0x78 set");
            let set = PropertySetRecord::parse_expecting(&payload, e.did, &dict)
                .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did))
                .set;
            bare_sets += 1;
            bare_characteristic += usize::from(
                EconomyKind::ALL
                    .iter()
                    .any(|k| set.has(k.characteristic(&reader.pids))),
            );
            bare_loot_refs += did_references(&set)
                .iter()
                .filter(|r| {
                    class_of
                        .get(&r.target)
                        .is_some_and(|c| loot_classes.contains(c))
                })
                .count();
        }
    }
    census.eq(
        "0x78 sets with a characteristic property",
        bare_characteristic,
        0,
    );
    census.eq("0x78 references into the loot family", bare_loot_refs, 0);

    // --- Entry census (§4) -------------------------------------------------------------
    let mut trophy_entries: BTreeMap<u32, (usize, usize, usize)> = BTreeMap::new();
    let (mut tgp_entries, mut tgp_quantity) = (BTreeMap::<u32, usize>::new(), 0usize);
    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut add = |k: &'static str, n: usize| *sizes.entry(k).or_default() += n;
    let mut filter_kinds = [0usize; 4];
    let mut empty_property_sets = 0usize;
    for r in &economy {
        match r.record.as_ref().expect("decoded") {
            EconomyRecord::TrophyList(t) => {
                let e = trophy_entries.entry(r.class).or_default();
                e.0 += t.entries.len();
                e.1 += t.entries.iter().filter(|e| e.quantity_stored).count();
                e.2 += t.entries.iter().filter(|e| e.group_drop).count();
            }
            EconomyRecord::TreasureGroupProfile(t) => {
                *tgp_entries.entry(r.class).or_default() += t.entries.len();
                tgp_quantity += t.entries.iter().filter(|e| e.quantity_stored).count();
            }
            EconomyRecord::TreasureList(t) => add("treasure list entries", t.entries.len()),
            EconomyRecord::WeightedTreasureTable(t) => {
                add("weighted table entries", t.entries.len());
            }
            EconomyRecord::FilteredTreasureList(t) => {
                add("filtered entries", t.entries.len());
                for f in t.entries.iter().flat_map(|e| &e.filters) {
                    match f {
                        EntityFilter::PropertySet(v) => {
                            filter_kinds[0] += 1;
                            empty_property_sets += usize::from(v.is_empty());
                        }
                        EntityFilter::PropertyRange { .. } => filter_kinds[1] += 1,
                        EntityFilter::WorldEvent(_) => filter_kinds[2] += 1,
                        EntityFilter::Other { .. } => filter_kinds[3] += 1,
                    }
                }
            }
            EconomyRecord::LevelBasedLootTable(t) => {
                add("level table entries", t.entries.len());
                add(
                    "level table trophy lists",
                    t.entries.iter().filter(|e| e.trophy_list.is_some()).count(),
                );
                add(
                    "level table treasure lists",
                    t.entries
                        .iter()
                        .filter(|e| e.treasure_list.is_some())
                        .count(),
                );
            }
            EconomyRecord::SpeciesLevelLootTable(t) => add("species entries", t.entries.len()),
            EconomyRecord::PvpLevelLootTable(t) => add("PVP entries", t.entries.len()),
            EconomyRecord::WorldEventLevelLootTable(t) => {
                add("world-event level entries", t.entries.len());
                add(
                    "world-event trophy lists",
                    t.entries.iter().filter(|e| e.trophy_list.is_some()).count(),
                );
            }
            EconomyRecord::WorldEventLootTableDirectory(t) => {
                add("world-event directory entries", t.entries.len());
                add(
                    "world-event directory conditions",
                    t.entries.iter().map(|e| e.conditions.len()).sum(),
                );
            }
            EconomyRecord::RunicLevelLootTable(t) => {
                add("runic level entries", t.entries.len());
                add(
                    "runic level runic lists",
                    t.entries.iter().filter(|e| e.runic_list.is_some()).count(),
                );
                // E2: every entry stores its level (E-I5 checks the value).
                add(
                    "runic level stored levels",
                    t.entries.iter().filter(|e| e.level_stored).count(),
                );
            }
            EconomyRecord::RunicList(t) => add("runic list entries", t.entries.len()),
            EconomyRecord::RunicTreasureGroup(t) => {
                add("runic group relics", t.relics.len());
                add("runic group pull counts", t.pull_counts.len());
                let counts: Vec<Option<i32>> = t.pull_counts.iter().map(|p| p.count).collect();
                assert_eq!(counts, vec![Some(1), Some(2), Some(3), Some(4)], "§4.9");
            }
            EconomyRecord::SkirmishCustomLootLookupTable(t) => {
                add("skirmish custom lookups", t.lookups.len());
            }
            EconomyRecord::HobbyRewardProfile(t) => {
                add("hobby entries", t.entries.len());
                add(
                    "hobby meta scores",
                    t.entries.iter().filter(|e| e.meta_score.is_some()).count(),
                );
            }
            _ => {}
        }
    }
    for (class, (entries, quantity, group)) in &trophy_entries {
        let expected = match class {
            1237 => (77_200, 72_016, 170),
            1917 => (436, 121, 73),
            _ => (121, 0, 0),
        };
        census.eq(&format!("trophy entries {class}"), *entries, expected.0);
        census.eq(&format!("trophy quantities {class}"), *quantity, expected.1);
        census.eq(&format!("trophy group drops {class}"), *group, expected.2);
    }
    census.eq(
        "profile entries 1235",
        tgp_entries.get(&1235).copied().unwrap_or(0),
        116_706,
    );
    census.eq(
        "profile entries 2372",
        tgp_entries.get(&2372).copied().unwrap_or(0),
        58,
    );
    census.eq("profile quantities", tgp_quantity, 15_468);
    for (what, expected) in [
        ("treasure list entries", 2_299),
        ("weighted table entries", 4_335),
        ("filtered entries", 11_618),
        ("level table entries", 19_360),
        ("level table trophy lists", 16_185),
        ("level table treasure lists", 3_825),
        ("species entries", 264),
        ("PVP entries", 15),
        ("world-event level entries", 1_760),
        ("world-event trophy lists", 1_223),
        ("world-event directory entries", 11),
        ("world-event directory conditions", 12),
        ("runic level entries", 1_200),
        ("runic level runic lists", 1_050),
        ("runic level stored levels", 1_200),
        ("runic list entries", 842),
        ("runic group relics", 450),
        ("runic group pull counts", 136),
        ("skirmish custom lookups", 8_326),
        ("hobby entries", 2_689),
        ("hobby meta scores", 2_562),
    ] {
        census.eq(what, sizes.get(what).copied().unwrap_or(0), expected);
    }
    census.eq("property-set filters", filter_kinds[0], 18_525);
    census.eq("empty property-set filters", empty_property_sets, 7_356);
    census.eq("range filters", filter_kinds[1], 7_426);
    census.eq("world-event filters", filter_kinds[2], 326);
    census.eq("other filters", filter_kinds[3], 0);
    census.eq("drop-frequency rows", control.drop_frequencies.len(), 56);
    census.eq("money-drop rows", control.money_drops.len(), 160);
    census.eq("money-modifier rows", control.money_modifiers.len(), 27);

    // Every skirmish lookup is listed exactly once over the custom tables (§4.10).
    let mut listed: HashMap<Did, usize> = HashMap::new();
    for r in of_kind(EconomyKind::SkirmishCustomLootLookupTable) {
        if let Some(EconomyRecord::SkirmishCustomLootLookupTable(t)) = &r.record {
            for &d in &t.lookups {
                *listed.entry(d).or_default() += 1;
            }
        }
    }
    for r in of_kind(EconomyKind::SkirmishTreasureLookup) {
        assert_eq!(listed.get(&r.did), Some(&1), "skirmish lookup listed once");
    }

    // E-I6 for the directory union (§9.1, 1674: no key in two tables).
    let mut union_keys = HashSet::new();
    let mut union_conflicts = 0usize;
    for r in of_kind(EconomyKind::LootDirectoryControl) {
        if let Some(EconomyRecord::LootDirectoryControl(d)) = &r.record {
            census.eq("directory species tables", d.species_tables.len(), 4);
            assert!(d.pvp_table.is_some());
            for t in &d.species_tables {
                if let Some(EconomyRecord::SpeciesLevelLootTable(s)) =
                    by_did.get(t).and_then(|r| r.record.as_ref())
                {
                    for e in &s.entries {
                        union_conflicts += usize::from(!union_keys.insert(e.key()));
                    }
                }
            }
        }
    }
    census.eq("directory union keys", union_keys.len(), 135);
    assert_eq!(union_conflicts, 0, "§9.1 union without conflicts");

    // --- Crafting (§5) -------------------------------------------------------------------
    let pc = &reader.pids.craft;
    let mut c = [0usize; 12];
    let mut xp_keys = BTreeSet::new();
    for r in of_kind(EconomyKind::Recipe) {
        let (Some(EconomyRecord::Recipe(rec)), Some(set)) = (&r.record, &r.set) else {
            continue;
        };
        c[0] += usize::from(rec.tier.is_some());
        c[1] += usize::from(rec.tier_array.is_some());
        c[2] += rec.ingredients.len();
        c[3] += usize::from(!rec.optional_ingredients.is_empty());
        c[4] += rec.optional_ingredients.len();
        c[5] += usize::from(rec.ingredient_pack.is_some());
        c[6] += usize::from(set.has(pc.recipe_result_quantity));
        c[7] += usize::from(rec.critical_result.is_some());
        c[8] += usize::from(set.has(pc.recipe_critical_quantity));
        c[9] += usize::from(!rec.multi_outputs.is_empty());
        c[10] += rec.multi_outputs.len();
        c[11] += usize::from(rec.name.is_some());
        xp_keys.extend(rec.xp_reward);
    }
    for (i, (what, expected)) in [
        ("recipes with a tier", 7_682),
        ("recipes with a tier array", 109),
        ("ingredients", 20_545),
        ("recipes with optional ingredients", 4_012),
        ("optional ingredients", 8_017),
        ("ingredient packs", 4_520),
        ("result quantities", 843),
        ("critical results", 4_461),
        ("critical quantities", 4_067),
        ("recipes with multi-outputs", 2_414),
        ("multi-output entries", 4_772),
        ("recipe names", 3_092),
    ]
    .into_iter()
    .enumerate()
    {
        census.eq(what, c[i], expected);
    }
    let craft_control = of_kind(EconomyKind::CraftControl)
        .find_map(|r| match &r.record {
            Some(EconomyRecord::CraftControl(c)) => Some(c.clone()),
            _ => None,
        })
        .expect("craft control");
    for k in &xp_keys {
        assert!(craft_control.xp_value(*k).is_some(), "§5.4 XP key {k}");
    }
    let (mut tiers, mut tier_recipes) = (0usize, 0usize);
    for r in of_kind(EconomyKind::Profession) {
        if let Some(EconomyRecord::Profession(p)) = &r.record {
            tiers += p.tiers.len();
            tier_recipes += p.recipes().count();
            assert_eq!(p.required_tool_keys().len(), 1, "§5.2 one tool bit");
        }
    }
    census.eq("profession tiers", tiers, 160);
    census.eq("recipes in tier arrays", tier_recipes, 1_733);
    for r in of_kind(EconomyKind::Vocation) {
        if let Some(EconomyRecord::Vocation(v)) = &r.record {
            assert_eq!((v.professions.len(), v.items.len()), (3, 4), "§5.3");
        }
    }

    // E-I15: VOCATIONS and PROFESSIONS DID maps.
    for (label, kind, n) in [
        ("VOCATIONS", EconomyKind::Vocation, 7usize),
        ("PROFESSIONS", EconomyKind::Profession, 10),
    ] {
        let map = data
            .did_map_by_label(label)
            .expect("DID map")
            .unwrap_or_else(|| panic!("{label} map"));
        let mut targets = BTreeSet::new();
        let mut ok = map.len() == n + 1;
        for e in map.iter() {
            if e.key == 0 {
                ok &= e.value.is_null();
            } else {
                ok &= (1..=n as u32).contains(&e.key);
                ok &= by_did.get(&e.value).and_then(|r| r.kind) == Some(kind);
                ok &= targets.insert(e.value);
            }
        }
        if !ok {
            *broken.entry(Invariant::CraftMaps.code()).or_default() += 1;
        }
    }

    // --- Barter and vendors (§6) ----------------------------------------------------------
    let mut shapes: BTreeMap<BarterShape, usize> = BTreeMap::new();
    let (mut costs, mut cost_items, mut lookups, mut rewards, mut reputation, mut sounds) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut uses_min, mut lookup_profiles) = (0usize, 0usize);
    for r in of_kind(EconomyKind::BarterProfile) {
        let Some(EconomyRecord::BarterProfile(b)) = &r.record else {
            continue;
        };
        let mut has_lookup = false;
        for l in &b.lists {
            if let Some(s) = l.shape() {
                *shapes.entry(s).or_default() += 1;
            }
            for e in &l.elements {
                match e {
                    BarterElement::Cost(c) => {
                        costs += 1;
                        cost_items += usize::from(!c.item.is_null());
                        lookups += usize::from(c.lookup_table.is_some());
                        has_lookup |= c.lookup_table.is_some();
                    }
                    BarterElement::Reward(BarterReward::Item {
                        uses_min_usage_level,
                        ..
                    }) => {
                        rewards += 1;
                        uses_min += usize::from(*uses_min_usage_level);
                    }
                    BarterElement::Reward(BarterReward::Reputation { .. }) => reputation += 1,
                    BarterElement::SoundCategory(_) => sounds += 1,
                    BarterElement::Other(_) => {}
                }
            }
        }
        lookup_profiles += usize::from(has_lookup);
    }
    for (shape, expected) in [
        (BarterShape::Item { costs: 1 }, 27_711),
        (BarterShape::Item { costs: 2 }, 12_720),
        (BarterShape::Item { costs: 3 }, 1_811),
        (
            BarterShape::Reputation {
                costs: 1,
                sound: false,
            },
            4,
        ),
        (
            BarterShape::Reputation {
                costs: 1,
                sound: true,
            },
            3,
        ),
    ] {
        census.eq(
            &format!("barter shape {shape:?}"),
            shapes.get(&shape).copied().unwrap_or(0),
            expected,
        );
    }
    census.eq("barter lists", shapes.values().sum(), 42_249);
    census.eq("barter costs", costs, 58_591);
    census.eq("barter cost items", cost_items, 58_589);
    census.eq("barter look-up costs", lookups, 5_565);
    census.eq("barter look-up profiles", lookup_profiles, 342);
    census.eq("barter reward items", rewards, 42_242);
    census.eq("barter reputation rewards", reputation, 7);
    census.eq("barter sound categories", sounds, 3);
    census.eq("barter min-usage-level rewards", uses_min, 1_649);

    let inventories: HashMap<Did, &tessera_formats::gameplay::economy::VendorInventory> =
        of_kind(EconomyKind::VendorInventory)
            .filter_map(|r| match &r.record {
                Some(EconomyRecord::VendorInventory(v)) => Some((r.did, v)),
                _ => None,
            })
            .collect();
    census.eq(
        "vendor inventory items",
        inventories.values().map(|v| v.items.len()).sum(),
        14_014,
    );

    // --- Second pass: non-economy records with links (NPC vendors, attachments) -----------
    let loot: HashSet<Did> = economy
        .iter()
        .filter(|r| r.kind.is_some_and(EconomyKind::is_loot_table))
        .map(|r| r.did)
        .collect();
    let mut links = CrossLinks::new();
    for r in &economy {
        links.note_kind(r.did, r.kind.expect("kind"));
    }
    for r in &economy {
        let set = r.set.clone().unwrap_or_default();
        links
            .add(&reader, r.did, &set, r.record.as_ref())
            .expect("links");
    }
    let (mut vendors, mut vendor_items, mut vendor_ok, mut attaching) =
        (0usize, 0usize, 0usize, 0usize);
    let mut named_inventories: HashSet<Did> = HashSet::new();
    let mut vendor_lists: Vec<(Did, Vec<Did>)> = Vec::new();
    for r in recs.iter().filter(|r| r.kind.is_none()) {
        let attaches = r.targets.iter().any(|t| loot.contains(t));
        if !attaches && !r.linked {
            continue;
        }
        attaching += usize::from(attaches);
        let set = read_set(&store, &dict, r.did).expect("set of a linked record");
        if let Some(v) = reader.npc_vendor(&set).expect("NPC vendor") {
            let v: NpcVendor = v;
            check_targets(
                &set,
                &vendor_targets,
                None,
                &class_of,
                &mut dangling,
                &mut wrong_class,
                &mut classified,
            );
            named_inventories.extend(v.inventories.iter().copied());
            if !v.item_list.is_empty() {
                vendor_lists.push((r.did, v.item_list.clone()));
            }
            if !v.inventories.is_empty() {
                vendors += 1;
                vendor_items += v.item_list.len();
                if v.item_list_matches(|d| inventories.get(&d).copied()) {
                    vendor_ok += 1;
                } else {
                    *broken.entry(Invariant::VendorItemList.code()).or_default() += 1;
                }
            }
        }
        links.add(&reader, r.did, &set, None).expect("links");
    }
    census.eq("NPCs with inventories", vendors, 1_439);
    census.eq("NPC vendor item lists", vendor_items, 91_274);
    census.eq("NPCs whose item list is the union", vendor_ok, 1_439);

    // E-I2 summary.
    eprintln!(
        "economy E-I2: {classified} references checked by class, {dangling} dangling, wrong \
         classes {wrong_class:?}"
    );
    let wrong: usize = wrong_class.values().sum();
    if dangling + wrong > 0 {
        *broken.entry(Invariant::Targets.code()).or_default() += dangling + wrong;
    }

    // E-I14.
    let graph = links.loot_graph();
    assert!(graph.acyclic, "E-I14: loot-table graph has a cycle");
    assert!(
        graph.longest_path <= 6,
        "E-I14: longest path {}",
        graph.longest_path
    );

    // Cross-links: every item of a treasure group profile has a "drops from" source or is
    // reached only by unattached tables; count both.
    let mut items: BTreeSet<Did> = BTreeSet::new();
    for r in of_kind(EconomyKind::TreasureGroupProfile) {
        if let Some(EconomyRecord::TreasureGroupProfile(t)) = &r.record {
            items.extend(t.entries.iter().map(|e| e.item));
        }
    }
    // Oracle for "drops from" (§10.2): a breadth-first walk forward from every source (a
    // record that is not a loot table and names one, or a species, world-event or skirmish
    // table), through loot tables only. An item has a source when the walk reaches it within
    // the depth limit.
    let kind_of: HashMap<Did, EconomyKind> = economy
        .iter()
        .filter_map(|r| Some((r.did, r.kind?)))
        .collect();
    let is_loot = |d: &Did| kind_of.get(d).is_some_and(|k| k.is_loot_table());
    let forward: HashMap<Did, Vec<Did>> = economy
        .iter()
        .filter(|r| is_loot(&r.did))
        .map(|r| {
            let set = r.set.as_ref().expect("set");
            (
                r.did,
                did_references(set).iter().map(|t| t.target).collect(),
            )
        })
        .collect();
    let mut distance: HashMap<Did, usize> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for r in &recs {
        let special = r.kind.is_some_and(EconomyKind::is_special_source);
        if special {
            distance.insert(r.did, 0);
            queue.push_back(r.did);
        } else if !is_loot(&r.did) {
            for t in r.refs.iter().filter(|t| is_loot(t)) {
                if !distance.contains_key(t) {
                    distance.insert(*t, 1);
                    queue.push_back(*t);
                }
            }
        }
    }
    while let Some(node) = queue.pop_front() {
        let d = distance[&node];
        if d >= DROPS_FROM_DEPTH {
            continue;
        }
        for &t in forward.get(&node).into_iter().flatten() {
            if let std::collections::hash_map::Entry::Vacant(e) = distance.entry(t) {
                e.insert(d + 1);
                if is_loot(&t) {
                    queue.push_back(t);
                }
            }
        }
    }
    let (mut with_source, mut deepest, mut oracle_differs) = (0usize, 0usize, 0usize);
    for &item in items.iter().step_by(25) {
        let sources = links.drops_from(item, DROPS_FROM_DEPTH);
        with_source += usize::from(!sources.is_empty());
        oracle_differs += usize::from(sources.is_empty() == distance.contains_key(&item));
        for s in &sources {
            deepest = deepest.max(s.depth);
            assert_eq!(s.path.len() + 1, s.depth, "{item}: path length");
            assert!(s.path.iter().all(&is_loot), "{item}: path of loot tables");
            assert!(
                s.kind
                    .is_none_or(|k| !k.is_loot_table() || k.is_special_source()),
                "{item}: source kind"
            );
        }
    }
    assert_eq!(
        oracle_differs, 0,
        "drops-from differs from the forward walk"
    );
    // §10.1: the longest path from an attaching record to an item has 6 edges.
    assert!(deepest <= 6, "drops-from depth {deepest}");
    let (mut crafted, mut sold, mut bartered) = (0usize, 0usize, 0usize);
    for r in of_kind(EconomyKind::Recipe) {
        if let Some(EconomyRecord::Recipe(rec)) = &r.record {
            crafted += usize::from(links.crafted_by(rec.result).contains(&r.did));
        }
    }
    // "Sold by": an inventory item is sold exactly when an NPC names an inventory with it.
    let mut stocked: HashSet<Did> = HashSet::new();
    for (did, v) in &inventories {
        if named_inventories.contains(did) {
            stocked.extend(v.items.iter().copied());
        }
    }
    let mut sold_expected = 0usize;
    for v in inventories.values() {
        sold += v
            .items
            .iter()
            .filter(|&&i| !links.sold_by(i).is_empty())
            .count();
        sold_expected += v.items.iter().filter(|i| stocked.contains(i)).count();
    }
    assert_eq!(sold, sold_expected, "sold-by against the named inventories");
    // §10.2: `Vendor_ItemList` gives the same NPCs.
    let mut sold_by_misses = 0usize;
    for (npc, list) in &vendor_lists {
        for &item in list {
            sold_by_misses += usize::from(!links.sold_by(item).iter().any(|(n, _)| n == npc));
        }
    }
    assert_eq!(
        sold_by_misses, 0,
        "Vendor_ItemList items not found by sold-by"
    );
    for r in of_kind(EconomyKind::BarterProfile) {
        if let Some(EconomyRecord::BarterProfile(b)) = &r.record {
            for l in &b.lists {
                if let Some(BarterReward::Item { item, .. }) = l.reward() {
                    bartered += usize::from(links.bartered_for(*item).contains(&r.did));
                }
            }
        }
    }
    census.eq("recipes found by crafted-by", crafted, 7_791);
    // §10: every item reward of a barter list is found by bartered-for. An invariant over
    // this run's own reward count, not a census figure.
    assert_eq!(
        bartered, rewards,
        "barter reward items not found by bartered-for"
    );
    eprintln!(
        "economy links: {} loot edges, {attaching} attaching records, {} loot tables, longest \
         path {}; drops-from sample {with_source} of {} items with sources (deepest {deepest}); \
         {sold} inventory items sold by an NPC; {bare_sets} 0x78 sets",
        links.loot_edges(),
        graph.tables,
        graph.longest_path,
        items.len().div_ceil(25),
    );

    // --- Object form (§9.2, §9.3): E-I12 and E-I13 ----------------------------------------
    let (mut tables, mut tables_indexed, mut tables_matching) = (0usize, 0usize, 0usize);
    for r in economy.iter().filter(|r| {
        matches!(
            r.kind,
            Some(EconomyKind::TreasureGroupProfile | EconomyKind::WeightedTreasureTable)
        )
    }) {
        let payload = data.read(r.did).expect("read").expect("record");
        let state = store.decode(r.did, &payload).expect("object state");
        let found: Vec<WeightedTable> = state
            .items()
            .iter()
            .filter_map(|i| match i.as_native() {
                Some(Native::WeightedTable(v, trailing)) => Some(WeightedTable {
                    elements: v
                        .iter()
                        .map(|e| WeightedElement {
                            index: e.value,
                            weight: e.weight,
                            sum: e.unknown,
                        })
                        .collect(),
                    trailing: *trailing,
                }),
                _ => None,
            })
            .collect();
        assert_eq!(found.len(), 1, "{}: one class-175 table", r.did);
        let t = &found[0];
        tables += 1;
        violations(&mut broken, &t.check());
        tables_indexed += usize::from(t.indices_hold());
        let weights: Vec<u32> = match r.record.as_ref().expect("decoded") {
            EconomyRecord::TreasureGroupProfile(p) => p.weights(),
            EconomyRecord::WeightedTreasureTable(w) => w
                .sorted_by_weight()
                .iter()
                .map(|e| u32::try_from(e.weight).unwrap_or(0))
                .collect(),
            _ => Vec::new(),
        };
        tables_matching += usize::from(t.weights() == weights);
    }
    census.eq("class-175 tables of the loot classes", tables, 7_441);
    census.eq("class-175 tables with index words", tables_indexed, 7_441);
    census.eq(
        "class-175 weights equal the property form",
        tables_matching,
        7_441,
    );

    let mut progressions: HashMap<Did, FloatProgression> = HashMap::new();
    let mut levels: HashMap<(Did, bool), Option<i64>> = HashMap::new();
    let (mut entries, mut entries_ok, mut by_min_level, mut by_item_level, mut clamped) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for r in of_kind(EconomyKind::BarterProfile) {
        let Some(EconomyRecord::BarterProfile(b)) = &r.record else {
            continue;
        };
        let payload = data.read(r.did).expect("read").expect("record");
        let state = store.decode(r.did, &payload).expect("object state");
        let stored: Vec<BarterEntry> = state
            .items()
            .iter()
            .filter_map(|i| match i.as_native() {
                Some(Native::BarterEntry(e)) => Some(BarterEntry {
                    profile: e.owner,
                    reward_item: e.target,
                    reward_quantity: e.count,
                    uses_min_usage_level: e.flag != 0,
                    reputation_faction: Did(e.alt_target),
                    reputation_tier: e.alt_count,
                    costs: e.pairs,
                    sound_category: e.tail,
                }),
                _ => None,
            })
            .collect();
        if stored.len() != b.lists.len() {
            *broken.entry(Invariant::BarterEntries.code()).or_default() += 1;
            continue;
        }
        for (l, s) in b.lists.iter().zip(&stored) {
            entries += 1;
            let reward = l.reward().copied();
            let expected = l.expected_entry(r.did, |c| {
                let Some(table) = c.lookup_table else {
                    return c.quantity.and_then(|q| u32::try_from(q).ok());
                };
                let Some(BarterReward::Item {
                    item,
                    uses_min_usage_level,
                    ..
                }) = reward
                else {
                    return None;
                };
                let level = *levels
                    .entry((item, uses_min_usage_level))
                    .or_insert_with(|| {
                        let set = read_set(&store, &dict, item)?;
                        reader
                            .reward_level(&set, uses_min_usage_level)
                            .expect("reward level")
                    });
                let level = level?;
                let prog = progressions.entry(table).or_insert_with(|| {
                    let set = read_set(&store, &dict, table).unwrap_or_default();
                    reader.float_progression(&set).expect("progression")
                });
                if uses_min_usage_level {
                    by_min_level += 1;
                } else {
                    by_item_level += 1;
                }
                let last = i64::try_from(prog.values.len()).unwrap_or(0) - 1;
                clamped += usize::from(level - prog.minimum_index > last);
                lookup_quantity(prog, level)
            });
            if expected.as_ref() == Some(s) {
                entries_ok += 1;
            } else {
                *broken.entry(Invariant::BarterEntries.code()).or_default() += 1;
            }
        }
    }
    census.eq("barter entries", entries, 42_249);
    // §9.3: every barter entry equals the property form. An invariant over this run's own
    // entry count, not a census figure.
    assert_eq!(
        entries_ok, entries,
        "barter entries equal to the property form"
    );

    census.eq("look-up costs by minimum usage level", by_min_level, 2_446);
    census.eq("look-up costs by item level", by_item_level, 3_119);
    census.eq("look-up costs clamped at the high end", clamped, 220);

    eprintln!(
        "economy: {} economy records checked in {:.1?}; invariant violations {broken:?}",
        economy.len(),
        started.elapsed()
    );
    census.finish();
    assert!(broken.is_empty(), "economy invariants broken: {broken:?}");
}
