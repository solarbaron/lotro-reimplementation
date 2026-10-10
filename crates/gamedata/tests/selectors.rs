// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the record-family index and selector resolution
//! (`scenario-feed.md` §8) over a `MemorySource` install built by `common`.

mod common;

use common::*;
use tessera_formats::Did;
use tessera_gamedata::{
    Alt, Attribute, Candidate, CandidateOutcome, CandidateStatus, Count, FailureKind, Family,
    FilterValue, GameDataError, GameDataStore, IndexOptions, LabelContext, Location,
    MAX_RESOLVED_TOTAL, ResolveNote, SelectorQuery, SplitMix64, canonical_cmp, resolve_with,
    resolve_without_install, sample, sort_dedup,
};
use tessera_props::MemorySource;

// ---------------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------------

const C_VENDOR_INV: u32 = 301;
const C_QUEST: u32 = 300;
const C_RECIPE: u32 = 302;

fn s_struct(fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    v_struct(
        &fields
            .iter()
            .map(|(l, v)| (pid(l), v.clone()))
            .collect::<Vec<_>>(),
    )
}

fn structs(element: &str, items: &[Vec<u8>]) -> Vec<u8> {
    let e = pid(element);
    v_array(&items.iter().map(|v| (e, v.clone())).collect::<Vec<_>>())
}

/// The synthetic world of these tests (every DID invented).
#[allow(clippy::too_many_lines)]
fn world() -> Synth {
    let mut s = Synth::new();
    // Items (class 101 = I, 102 below it, 103 item-like under P = 100).
    s.object(
        0x7000_0101,
        C_ITEM_SUB,
        Some(&[
            ("WeenieType", v_u32(4)),
            ("Inventory_DefaultSlot", v_u32(0x11)),
            ("Inventory_CompatibleSlot", v_u32(0x03)),
            ("Item_Class", v_u32(1)),
            ("Item_Quality", v_u32(2)),
            ("Item_Level", v_i32(10)),
            ("Usage_MinLevel", v_i32(5)),
            (
                "Usage_RequiredClassList",
                v_array(&[(pid("Usage_ClassType"), v_u32(23))]),
            ),
            ("Inventory_MaxQuantity", v_i32(1)),
            ("Icon_Layer_ImageDID", v_u32(0x4100_0001)),
            ("Name", v_text("Blade")),
        ]),
    );
    s.object(
        0x7000_0102,
        C_ITEM_BASE,
        Some(&[
            ("Inventory_DefaultSlot", v_u32(0x20)),
            ("Item_Class", v_u32(2)),
            ("Item_Quality", v_u32(1)),
            ("Item_Level", v_i32(30)),
            ("Inventory_MaxQuantity", v_i32(50)),
        ]),
    );
    s.object(
        0x7000_0103,
        C_ITEM_BASE,
        Some(&[
            ("WeenieType", v_u32(5)),
            ("Inventory_DefaultSlot", v_u32(0x12)),
            ("Item_Level", v_i32(20)),
            ("Inventory_MaxQuantity", v_i32(-1)),
        ]),
    );
    s.object(0x7000_0104, C_ITEM_LIKE, Some(&[("Item_Level", v_i32(1))]));
    // Vendor inventories (their references find the item classes, items.md §2.4).
    s.object(
        0x7000_0401,
        C_VENDOR_INV,
        Some(&[(
            "VendorInventory_Items",
            v_dids("Vendor_Item", &[0x7000_0101, 0x7000_0102]),
        )]),
    );
    s.object(
        0x7000_0402,
        C_VENDOR_INV,
        Some(&[(
            "VendorInventory_Items",
            v_dids("Vendor_Item", &[0x7000_0104]),
        )]),
    );
    // Empty inventory: classified by its class (economy.md §3.4).
    s.object(0x7000_0403, C_VENDOR_INV, None);
    // Creatures (creature subtree 200).
    s.object(
        0x7000_0201,
        C_MOB,
        Some(&[
            ("WeenieType", v_u32(1)),
            ("Agent_Species", v_u32(1)),
            ("Agent_Genus", v_u32(0x1)),
            ("Agent_Alignment", v_u32(2)),
            ("Agent_Class", v_u32(1)),
            (
                "Appearance_ClassList",
                structs("Appearance_Entry", &[s_struct(&[])]),
            ),
        ]),
    );
    s.object(
        0x7000_0202,
        C_NPC,
        Some(&[
            ("WeenieType", v_u32(2)),
            ("Agent_Species", v_u32(2)),
            ("Agent_Genus", v_u32(0x2)),
            ("Agent_Alignment", v_u32(1)),
            (
                "Vendor_InventoryList",
                v_dids("Vendor_Inventory", &[0x7000_0401]),
            ),
            ("SkillTrainer_Class", v_u32(23)),
            (
                "QuestDispenser_ActiveQuests",
                v_dids("QuestDispenser_Quest", &[0x7000_0501]),
            ),
        ]),
    );
    s.object(0x7000_0203, C_AVATAR, Some(&[]));
    // Skills.
    s.object(
        0x7000_0301,
        C_SKILL,
        Some(&[
            ("Skill_SkillType", v_u32(0x1)),
            ("Skill_Harmful", v_bool(true)),
            ("Skill_InductionAction", v_u32(0x7000_0901)),
        ]),
    );
    s.object(
        0x7000_0302,
        C_SKILL,
        Some(&[
            ("Skill_SkillType", v_u32(0x4)),
            ("Skill_Harmful", v_bool(false)),
            ("Skill_IsMonsterSkill", v_bool(true)),
            ("Skill_Toggle_Hook_Number", v_i32(1)),
        ]),
    );
    s.object(0x7000_0303, C_SKILL, Some(&[]));
    // Class tables.
    s.object(
        0x7000_0311,
        C_CLASS_TABLE,
        Some(&[
            ("AdvTable_Class", v_u32(23)),
            (
                "AdvTable_AvailableSkillEntryList",
                structs(
                    "AdvTable_AvailableSkillEntry",
                    &[s_struct(&[("AdvTable_Skill", v_u32(0x7000_0302))])],
                ),
            ),
            (
                "AdvTable_AvailableMountSkillEntryList",
                structs(
                    "AdvTable_AvailableSkillEntry",
                    &[s_struct(&[("AdvTable_Skill", v_u32(0x7000_0303))])],
                ),
            ),
            (
                "AdvTable_ClassCharacteristic_List",
                structs(
                    "AdvTable_ClassCharacteristic",
                    &[s_struct(&[("AdvTable_Trait_WC", v_u32(0x7000_0321))])],
                ),
            ),
        ]),
    );
    s.object(
        0x7000_0312,
        C_CLASS_TABLE,
        Some(&[
            ("AdvTable_Class", v_u32(24)),
            (
                "AdvTable_AvailableSkillEntryList",
                structs(
                    "AdvTable_AvailableSkillEntry",
                    &[s_struct(&[("AdvTable_Skill", v_u32(0x7000_0301))])],
                ),
            ),
        ]),
    );
    // Traits, effects, trait tree, induction.
    s.object(
        0x7000_0321,
        C_TRAIT,
        Some(&[
            ("Trait_Nature", v_u32(1)),
            ("Trait_Minimum_Level", v_i32(10)),
        ]),
    );
    s.object(
        0x7000_0322,
        C_TRAIT,
        Some(&[
            ("Trait_Nature", v_u32(2)),
            ("Trait_Minimum_Level", v_i32(50)),
        ]),
    );
    s.object(0x7000_0331, C_EFFECT_INSTANT_VITAL, Some(&[]));
    s.object(0x7000_0332, C_EFFECT_MOUNT, Some(&[]));
    s.object(0x7000_0341, C_TRAIT_TREE, Some(&[]));
    s.object(0x7000_0901, C_INDUCTION, Some(&[]));
    // Quests and the other quest-family records.
    let objective = || s_struct(&[]);
    s.object(
        0x7000_0501,
        C_QUEST,
        Some(&[
            ("Quest_Name", v_text("q1")),
            (
                "Quest_ObjectiveArray",
                structs("Quest_CompletionObjective", &[objective(), objective()]),
            ),
            ("Quest_RoleArray", v_array(&[])),
            ("Quest_Category", v_u32(10)),
            ("Quest_ChallengeLevel", v_i32(20)),
            (
                "Usage_RequiredClassList",
                v_array(&[(pid("Usage_ClassType"), v_u32(24))]),
            ),
        ]),
    );
    s.object(
        0x7000_0502,
        C_QUEST,
        Some(&[
            ("Quest_Name", v_text("d1")),
            (
                "Quest_ObjectiveArray",
                structs("Quest_CompletionObjective", &[objective()]),
            ),
            ("Quest_RoleArray", v_array(&[])),
            ("Quest_Category", v_u32(11)),
            ("Quest_ChallengeLevel", v_i32(50)),
            ("Quest_IsAccomplishment", v_bool(true)),
        ]),
    );
    s.object(
        0x7000_0511,
        C_QUEST,
        Some(&[
            ("NPCComment_Title", v_text("t")),
            ("NPCComment_Content", v_text("c")),
        ]),
    );
    s.object(
        0x7000_0512,
        C_QUEST,
        Some(&[("Title_String", v_text("t")), ("Title_Category", v_u32(1))]),
    );
    s.object(
        0x7000_0513,
        C_QUEST,
        Some(&[
            ("Emote_CommandString", v_text("e")),
            ("Emote_ScriptID", v_u32(0x7000_0001)),
        ]),
    );
    s.object(
        0x7000_0514,
        C_QUEST,
        Some(&[
            ("Reputation_Faction_Name", v_text("f")),
            ("Reputation_Faction_AdvancementTable", v_u32(0x7000_0002)),
        ]),
    );
    // Economy.
    s.object(
        0x7000_0601,
        C_RECIPE,
        Some(&[("CraftProfession_TierArray", v_array(&[]))]),
    );
    s.object(
        0x7000_0602,
        C_RECIPE,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0103)),
            ("CraftRecipe_Profession", v_u32(0x7000_0601)),
            ("CraftRecipe_Tier", v_u32(2)),
        ]),
    );
    s.object(
        0x7000_0603,
        C_RECIPE,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0102)),
            ("CraftRecipe_Profession", v_u32(0x7000_0699)),
            ("CraftRecipe_Tier", v_u32(1)),
        ]),
    );
    s.object(
        0x7000_0611,
        C_RECIPE,
        Some(&[("LootGen_TrophyList", v_array(&[]))]),
    );
    // Map kinds.
    s.object(
        0x7000_0701,
        C_OTHER,
        Some(&[
            ("TravelDisplayName", v_text("x")),
            ("TravelDestinationIsSwiftTravel", v_bool(true)),
        ]),
    );
    s.object(
        0x7000_0711,
        C_OTHER,
        Some(&[("Area_Territory", v_u32(0x7000_0712))]),
    );
    s.object(
        0x7000_0712,
        C_OTHER,
        Some(&[("Area_Region", v_u32(0x7000_0713))]),
    );
    s.object(0x7000_0713, C_OTHER, Some(&[("Area_RegionID", v_i32(1))]));
    s.object(0x7000_0714, C_OTHER, Some(&[("Dungeon_Name", v_text("d"))]));
    // Instance template.
    s.object(
        0x7000_0801,
        C_OTHER,
        Some(&[("PrivateEncounterTemplate_ContentLayer", v_u32(3))]),
    );
    // Race tables (player-character.md §2.2, §5.2).
    s.object(
        0x7000_0A01,
        C_OTHER,
        Some(&[(
            "RaceTable_RaceTableList",
            v_dids("Vendor_Inventory", &[0x7000_0A02]),
        )]),
    );
    s.object(
        0x7000_0A02,
        C_OTHER,
        Some(&[
            ("RaceTable_Race", v_u32(23)),
            (
                "RaceTable_GenderList",
                structs(
                    "RaceTable_GenderStruct",
                    &[
                        s_struct(&[("RaceTable_GenderType", v_u32(0x1000))]),
                        s_struct(&[("RaceTable_GenderType", v_u32(0x2000))]),
                    ],
                ),
            ),
        ]),
    );
    s.weenie(1, 0x7000_0A01, "RaceTableDirectory");
    // Broken records.
    s.list_only(0x7000_0F01);
    let mut wrong = 0x7000_0F99u32.to_le_bytes().to_vec();
    wrong.extend_from_slice(&C_OTHER.to_le_bytes());
    s.raw(0x7000_0F02, wrong, true);
    s.object(0x7000_0F03, C_OTHER, None);
    s.raw(0x7900_0F03, vec![0x03, 0x0F, 0x00, 0x79, 0xFF, 0xFF], false);
    s.object(
        0x7000_0F04,
        C_VENDOR_INV,
        Some(&[(
            "VendorInventory_Items",
            v_array(&[(pid("Item_Level"), v_i32(3))]),
        )]),
    );
    // Tables.
    s.table(
        0x9E,
        0x0E00_0004,
        "PlacesTable",
        places_table(
            0x0E00_0004,
            &[
                ("Alpha", 1, (10, 20), 0, [5.0, 6.0, 7.0], 0),
                ("Beta", 2, (1, 1), 0x0105, [1.0, 2.0, 3.0], 0x7000_0714),
                ("Gamma", 1, (0, 0), 0, [0.0, 0.0, 0.0], 0),
            ],
        ),
    );
    s.table(
        0x1000_001F,
        0x0E00_0006,
        "MapNotes",
        map_notes_table(
            0x0E00_0006,
            &[
                (1, (3, 4), 0, [1.0, 1.0, 1.0], 0x7000_0202, 0x2),
                (2, (5, 5), 0, [2.0, 2.0, 2.0], 0x7000_0201, 0x4),
            ],
        ),
    );
    s
}

fn store() -> GameDataStore<MemorySource> {
    world().build()
}

fn dids(r: &[Candidate]) -> Vec<u32> {
    r.iter().map(|c| c.did.0).collect()
}

fn resolve(store: &GameDataStore<MemorySource>, q: &SelectorQuery) -> (Vec<u32>, Vec<ResolveNote>) {
    let r = store.resolve(q, &[]).expect("resolves");
    (dids(&r.records), r.notes)
}

fn q(family: Family) -> SelectorQuery {
    SelectorQuery::all(family)
}

fn vals(v: &[u32]) -> FilterValue {
    FilterValue::Alternatives(v.iter().map(|&x| Alt::Value(x)).collect())
}

fn labels(v: &[&str]) -> FilterValue {
    FilterValue::Alternatives(v.iter().map(|&x| Alt::Label(x.to_string())).collect())
}

fn kw(v: &[&str]) -> FilterValue {
    FilterValue::Keywords(v.iter().map(|s| (*s).to_string()).collect())
}

fn range(lo: Option<i64>, hi: Option<i64>) -> FilterValue {
    FilterValue::Range { lo, hi }
}

// ---------------------------------------------------------------------------------------------
// Sampling (§8.5, Vector C §14.3)
// ---------------------------------------------------------------------------------------------

#[test]
fn splitmix_vector_c() {
    let first3 = |seed| {
        let mut g = SplitMix64::new(seed);
        [g.next_u64(), g.next_u64(), g.next_u64()]
    };
    assert_eq!(
        first3(0),
        [
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F
        ]
    );
    assert_eq!(
        first3(1),
        [
            0x910A_2DEC_8902_5CC1,
            0xBEEB_8DA1_658E_EC67,
            0xF893_A2EE_FB32_555E
        ]
    );
    assert_eq!(
        first3(7),
        [
            0x63CB_E1E4_5932_0DD7,
            0x044C_3CD7_F43C_661C,
            0xE698_4080_BAB1_2A02
        ]
    );
    // The trace of (n = 3, seed 7): j = 7, 7, 4.
    let mut g = SplitMix64::new(7);
    assert_eq!(g.below(10), 7);
    assert_eq!(1 + g.below(9), 7);
    assert_eq!(2 + g.below(8), 4);
    assert_eq!(SplitMix64::new(3).below(0), 0);
    assert_eq!(SplitMix64::new(3).below(1), 0);
}

fn ten_shuffled() -> Vec<Candidate> {
    // "given by a synthetic resolver in any order"
    [7u32, 3, 10, 1, 9, 2, 8, 4, 6, 5, 3]
        .iter()
        .map(|&i| Candidate::record(Did(0x7000_0000 + i)))
        .collect()
}

fn vector_c(count: Count) -> (Vec<u32>, Vec<ResolveNote>) {
    let query = SelectorQuery::all(Family::Item).count(count);
    let r = resolve_with(&[query], |_, _| {
        (
            CandidateOutcome::new(CandidateStatus::Ready),
            ten_shuffled(),
        )
    })
    .expect("resolves")
    .remove(0);
    (
        r.records.iter().map(|c| c.did.0 - 0x7000_0000).collect(),
        r.notes,
    )
}

#[test]
fn sampling_vector_c() {
    assert_eq!(vector_c(Count::Sample { n: 3, seed: 7 }).0, [1, 5, 8]);
    assert_eq!(vector_c(Count::Sample { n: 3, seed: 0 }).0, [2, 6, 10]);
    assert_eq!(vector_c(Count::Sample { n: 5, seed: 1 }).0, [2, 4, 6, 8, 9]);
    assert_eq!(vector_c(Count::Sample { n: 1, seed: 7 }).0, [8]);
    let (all, notes) = vector_c(Count::Sample { n: 10, seed: 7 });
    assert_eq!(all, (1..=10).collect::<Vec<_>>());
    assert!(notes.is_empty());
    let (all, notes) = vector_c(Count::Sample { n: 12, seed: 7 });
    assert_eq!(all, (1..=10).collect::<Vec<_>>());
    assert_eq!(
        notes,
        [ResolveNote::Short {
            requested: 12,
            found: 10
        }]
    );
    assert_eq!(vector_c(Count::First(3)).0, [1, 2, 3]);
    assert_eq!(vector_c(Count::All).0.len(), 10);
}

#[test]
fn sample_swaps_match_the_array_form() {
    // The array after the swaps of (n = 3, seed 7) begins 8, 1, 5 (§14.3); the result
    // sorted is 1, 5, 8. Compare the sparse implementation with the array form for many
    // (m, n, seed).
    for m in [1usize, 2, 5, 10, 33, 100] {
        let cands: Vec<Candidate> = (0..m)
            .map(|i| Candidate::record(Did(u32::try_from(i).unwrap())))
            .collect();
        for n in [0usize, 1, 2, 3, m / 2, m.saturating_sub(1), m, m + 1] {
            for seed in [0u64, 1, 7, 0xDEAD_BEEF, u64::MAX] {
                let mut a: Vec<usize> = (0..m).collect();
                let mut g = SplitMix64::new(seed);
                let expected: Vec<usize> = if n >= m {
                    a.clone()
                } else {
                    for i in 0..n {
                        let j = i + usize::try_from(g.below((m - i) as u64)).unwrap();
                        a.swap(i, j);
                    }
                    let mut v = a[..n].to_vec();
                    v.sort_unstable();
                    v
                };
                let got: Vec<usize> = sample(&cands, n, seed)
                    .iter()
                    .map(|c| c.did.0 as usize)
                    .collect();
                assert_eq!(got, expected, "m={m} n={n} seed={seed}");
            }
        }
    }
}

#[test]
fn canonical_order_and_dedup() {
    let loc = |x: f64| {
        Some(Location {
            region: 1,
            x,
            y: 0.0,
            z: 0.0,
        })
    };
    let mut v = vec![
        Candidate {
            did: Did(2),
            row: 1,
            location: loc(5.0),
            anchor: None,
        },
        Candidate {
            did: Did(2),
            row: 1,
            location: None,
            anchor: None,
        },
        Candidate {
            did: Did(2),
            row: 0,
            location: loc(9.0),
            anchor: None,
        },
        Candidate {
            did: Did(0xF000_0000),
            row: 0,
            location: None,
            anchor: None,
        },
        Candidate {
            did: Did(1),
            row: 7,
            location: loc(1.0),
            anchor: None,
        },
    ];
    sort_dedup(&mut v);
    assert_eq!(
        v.iter().map(|c| (c.did.0, c.row)).collect::<Vec<_>>(),
        [(1, 7), (2, 0), (2, 1), (0xF000_0000, 0)]
    );
    assert!(
        v[2].location.is_none(),
        "no location sorts first and is kept"
    );
    assert!(canonical_cmp(&v[0], &v[1]).is_lt());
}

// ---------------------------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------------------------

#[test]
fn index_sorts_records_into_families() {
    let s = store();
    let ix = s.index().expect("index");
    let fam = |f| ix.dids(f).iter().map(|d| d.0).collect::<Vec<_>>();
    assert_eq!(fam(Family::Item), [0x7000_0101, 0x7000_0102, 0x7000_0103]);
    assert_eq!(
        fam(Family::Creature),
        [0x7000_0201, 0x7000_0202, 0x7000_0203]
    );
    assert_eq!(fam(Family::Skill), [0x7000_0301, 0x7000_0302, 0x7000_0303]);
    assert_eq!(fam(Family::Class), [0x7000_0311, 0x7000_0312]);
    assert_eq!(fam(Family::Trait), [0x7000_0321, 0x7000_0322]);
    assert_eq!(fam(Family::Effect), [0x7000_0331, 0x7000_0332]);
    assert_eq!(fam(Family::TraitTree), [0x7000_0341]);
    assert_eq!(fam(Family::Induction), [0x7000_0901]);
    assert_eq!(fam(Family::Quest), [0x7000_0501, 0x7000_0502]);
    assert_eq!(fam(Family::NpcComment), [0x7000_0511]);
    assert_eq!(fam(Family::Title), [0x7000_0512]);
    assert_eq!(fam(Family::Emote), [0x7000_0513]);
    assert_eq!(fam(Family::Faction), [0x7000_0514]);
    assert_eq!(fam(Family::Profession), [0x7000_0601]);
    assert_eq!(fam(Family::Recipe), [0x7000_0602, 0x7000_0603]);
    assert_eq!(fam(Family::TrophyList), [0x7000_0611]);
    assert_eq!(
        fam(Family::VendorInventory),
        [0x7000_0401, 0x7000_0402, 0x7000_0403]
    );
    assert_eq!(fam(Family::TravelDestination), [0x7000_0701]);
    assert_eq!(
        fam(Family::Area),
        [0x7000_0711, 0x7000_0712, 0x7000_0713, 0x7000_0714]
    );
    assert_eq!(fam(Family::Instance), [0x7000_0801]);
    assert_eq!(fam(Family::Race), [0x7000_0A02]);
    assert_eq!(
        ix.item_classes().map(|c| (c.item_parent, c.item_base)),
        Some((C_ITEM_PARENT, C_ITEM_BASE))
    );
    assert_eq!(ix.creature_subtree_root(), Some(C_CREATURE_ROOT));
    assert_eq!(ix.families_of(Did(0x7000_0202)), [Family::Creature]);
}

#[test]
fn undecodable_records_are_listed_with_a_reason() {
    let s = store();
    let ix = s.index().expect("index");
    let reasons: Vec<(u32, FailureKind, Option<Family>)> = ix
        .failures()
        .iter()
        .map(|f| (f.did.0, f.kind, f.family))
        .collect();
    assert!(
        reasons.contains(&(0x7000_0F01, FailureKind::Missing, None)),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&(0x7000_0F02, FailureKind::Header, None)),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&(0x7000_0F03, FailureKind::PropertySet, None)),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&(
            0x7000_0F04,
            FailureKind::View,
            Some(Family::VendorInventory)
        )),
        "{reasons:?}"
    );
    assert_eq!(ix.failures_total(), ix.failures().len());
    assert!(
        !ix.contains(Family::VendorInventory, Did(0x7000_0F04)),
        "a failed view is no candidate"
    );
    for f in ix.failures() {
        assert!(f.detail.len() <= 160);
    }
}

#[test]
fn thread_count_does_not_change_the_index() {
    let mut w = world();
    // Enough records for the parallel path.
    for i in 0..700u32 {
        w.object(
            0x7100_0000 + i,
            C_ITEM_BASE,
            Some(&[("Item_Level", v_i32(i32::try_from(i % 90).unwrap()))]),
        );
    }
    let one = w
        .build()
        .with_options(IndexOptions::default().with_threads(1));
    let four = w
        .build()
        .with_options(IndexOptions::default().with_threads(4));
    for f in Family::RECORD_FAMILIES {
        assert_eq!(one.family(f).unwrap(), four.family(f).unwrap(), "{f:?}");
    }
    let qq = q(Family::Item)
        .filter(Attribute::Level, range(Some(10), Some(20)))
        .count(Count::Sample { n: 17, seed: 99 });
    assert_eq!(
        one.resolve(&qq, &[]).unwrap(),
        four.resolve(&qq, &[]).unwrap()
    );
}

// ---------------------------------------------------------------------------------------------
// Selector kinds (§8.2 type table) per family (§8.3)
// ---------------------------------------------------------------------------------------------

#[test]
fn item_attributes() {
    let s = store();
    let f = |a, v| resolve(&s, &q(Family::Item).filter(a, v)).0;
    assert_eq!(f(Attribute::Kind, kw(&["weapon"])), [0x7000_0101]);
    assert_eq!(
        f(Attribute::Kind, kw(&["wearable", "general"])),
        [0x7000_0102, 0x7000_0103]
    );
    // Slots: bit of key v (key k ↔ bit k − 1).
    assert_eq!(f(Attribute::Slot, labels(&["@Head"])), [0x7000_0101]);
    assert_eq!(f(Attribute::Slot, vals(&[2])), [0x7000_0103]);
    assert_eq!(
        f(Attribute::Slot, labels(&["@Backpack"])),
        [0x7000_0102],
        "label from the base mapper"
    );
    assert_eq!(f(Attribute::Fits, labels(&["@Chest"])), [0x7000_0101]);
    assert_eq!(
        f(Attribute::Equippable, FilterValue::Bool(true)),
        [0x7000_0101, 0x7000_0103]
    );
    assert_eq!(
        f(Attribute::Equippable, FilterValue::Bool(false)),
        [0x7000_0102]
    );
    assert_eq!(f(Attribute::Category, labels(&["@Sword"])), [0x7000_0101]);
    assert_eq!(
        f(Attribute::Category, vals(&[1, 2])),
        [0x7000_0101, 0x7000_0102]
    );
    assert_eq!(f(Attribute::Quality, labels(&["@Common"])), [0x7000_0102]);
    assert_eq!(
        f(Attribute::Level, range(Some(10), Some(20))),
        [0x7000_0101, 0x7000_0103]
    );
    assert_eq!(f(Attribute::Level, range(None, Some(10))), [0x7000_0101]);
    assert_eq!(f(Attribute::Level, range(Some(30), None)), [0x7000_0102]);
    assert_eq!(
        f(Attribute::Level, range(Some(20), Some(20))),
        [0x7000_0103]
    );
    assert_eq!(
        f(Attribute::MinLevel, range(Some(0), None)),
        [0x7000_0101],
        "absent matches no range"
    );
    assert_eq!(f(Attribute::Class, labels(&["@Minstrel"])), [0x7000_0101]);
    // Stackable: max quantity > 1 compared unsigned, so −1 counts as stackable.
    assert_eq!(
        f(Attribute::Stackable, FilterValue::Bool(true)),
        [0x7000_0102, 0x7000_0103]
    );
    assert_eq!(
        f(Attribute::Stackable, FilterValue::Bool(false)),
        [0x7000_0101]
    );
    assert_eq!(
        f(Attribute::HasIcon, FilterValue::Bool(true)),
        [0x7000_0101]
    );
    assert_eq!(
        f(Attribute::HasIcon, FilterValue::Bool(false)),
        [0x7000_0102, 0x7000_0103]
    );
}

#[test]
fn filters_are_anded() {
    let s = store();
    let r = resolve(
        &s,
        &q(Family::Item)
            .filter(Attribute::Equippable, FilterValue::Bool(true))
            .filter(Attribute::Level, range(Some(15), None)),
    );
    assert_eq!(r.0, [0x7000_0103]);
}

#[test]
fn skill_trait_class_attributes() {
    let s = store();
    let sk = |a, v| resolve(&s, &q(Family::Skill).filter(a, v)).0;
    assert_eq!(sk(Attribute::Class, labels(&["@Minstrel"])), [0x7000_0302]);
    assert_eq!(
        sk(Attribute::Class, vals(&[23, 24])),
        [0x7000_0301, 0x7000_0302]
    );
    assert_eq!(sk(Attribute::Mount, FilterValue::Bool(true)), [0x7000_0303]);
    assert_eq!(
        sk(Attribute::Mount, FilterValue::Bool(false)),
        [0x7000_0301, 0x7000_0302]
    );
    assert_eq!(sk(Attribute::Type, labels(&["@Song"])), [0x7000_0302]);
    assert_eq!(
        sk(Attribute::Type, labels(&["@Melee", "@Ranged"])),
        [0x7000_0301]
    );
    // Harmful has no absence rule: the record without it matches neither value.
    assert_eq!(
        sk(Attribute::Harmful, FilterValue::Bool(true)),
        [0x7000_0301]
    );
    assert_eq!(
        sk(Attribute::Harmful, FilterValue::Bool(false)),
        [0x7000_0302]
    );
    // Monster: absent = no.
    assert_eq!(
        sk(Attribute::Monster, FilterValue::Bool(false)),
        [0x7000_0301, 0x7000_0303]
    );
    assert_eq!(
        sk(Attribute::Induction, FilterValue::Bool(true)),
        [0x7000_0301]
    );
    assert_eq!(
        sk(Attribute::Toggle, FilterValue::Bool(true)),
        [0x7000_0302]
    );

    let tr = |a, v| resolve(&s, &q(Family::Trait).filter(a, v)).0;
    assert_eq!(tr(Attribute::Nature, labels(&["@Racial"])), [0x7000_0322]);
    assert_eq!(tr(Attribute::Class, labels(&["@Minstrel"])), [0x7000_0321]);
    assert_eq!(
        tr(Attribute::Class, labels(&["@Hunter"])),
        Vec::<u32>::new()
    );
    assert_eq!(
        tr(Attribute::MinLevel, range(Some(11), None)),
        [0x7000_0322]
    );

    let cl = |v| resolve(&s, &q(Family::Class).filter(Attribute::Class, v)).0;
    assert_eq!(cl(labels(&["@Hunter"])), [0x7000_0312]);

    let ef = |v| resolve(&s, &q(Family::Effect).filter(Attribute::Role, v)).0;
    assert_eq!(ef(kw(&["instant-vital"])), [0x7000_0331]);
    assert_eq!(ef(kw(&["mount", "other"])), [0x7000_0332]);
}

#[test]
fn creature_attributes() {
    let s = store();
    let cr = |a, v| resolve(&s, &q(Family::Creature).filter(a, v)).0;
    assert_eq!(cr(Attribute::Kind, kw(&["mob"])), [0x7000_0201]);
    assert_eq!(
        cr(Attribute::Kind, kw(&["npc", "container"])),
        [0x7000_0202]
    );
    assert_eq!(cr(Attribute::Species, labels(&["@Elf"])), [0x7000_0202]);
    assert_eq!(cr(Attribute::Genus, labels(&["@Beast"])), [0x7000_0201]);
    assert_eq!(cr(Attribute::Alignment, vals(&[1])), [0x7000_0202]);
    assert_eq!(
        cr(Attribute::AgentClass, labels(&["@Soldier"])),
        [0x7000_0201]
    );
    assert_eq!(
        cr(Attribute::HasAppearance, FilterValue::Bool(true)),
        [0x7000_0201]
    );
    assert_eq!(
        cr(Attribute::Vendor, FilterValue::Bool(true)),
        [0x7000_0202]
    );
    assert_eq!(
        cr(Attribute::Vendor, FilterValue::Bool(false)),
        [0x7000_0201, 0x7000_0203]
    );
    assert_eq!(
        cr(Attribute::Trainer, FilterValue::Bool(true)),
        [0x7000_0202]
    );
    assert_eq!(
        cr(Attribute::QuestGiver, FilterValue::Bool(true)),
        [0x7000_0202]
    );
}

#[test]
fn quest_attributes() {
    let s = store();
    let qu = |a, v| resolve(&s, &q(Family::Quest).filter(a, v)).0;
    assert_eq!(qu(Attribute::Deed, FilterValue::Bool(true)), [0x7000_0502]);
    assert_eq!(qu(Attribute::Deed, FilterValue::Bool(false)), [0x7000_0501]);
    assert_eq!(qu(Attribute::Category, labels(&["@Epic"])), [0x7000_0501]);
    assert_eq!(qu(Attribute::Level, range(Some(21), None)), [0x7000_0502]);
    assert_eq!(
        qu(Attribute::Objectives, range(Some(2), Some(2))),
        [0x7000_0501]
    );
    assert_eq!(qu(Attribute::Class, labels(&["@Hunter"])), [0x7000_0501]);
}

#[test]
fn economy_and_area_attributes() {
    let s = store();
    let re = resolve(
        &s,
        &q(Family::Recipe).filter(Attribute::Tier, labels(&["@Journeyman"])),
    )
    .0;
    assert_eq!(re, [0x7000_0602]);
    let ar = |v| resolve(&s, &q(Family::Area).filter(Attribute::Level, v)).0;
    assert_eq!(ar(kw(&["territory"])), [0x7000_0712]);
    assert_eq!(ar(kw(&["area", "dungeon"])), [0x7000_0711, 0x7000_0714]);
    assert_eq!(ar(kw(&["region"])), [0x7000_0713]);
}

#[test]
fn binding_attributes_in_declaration_order() {
    let s = store();
    let queries = [
        // 0: the NPC vendor.
        q(Family::Creature).filter(Attribute::Vendor, FilterValue::Bool(true)),
        // 1: its inventories.
        q(Family::VendorInventory).filter(Attribute::Of, FilterValue::Binding(0)),
        // 2: the items they list.
        q(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(1)),
        // 3: the profession.
        q(Family::Profession),
        // 4: recipes of it.
        q(Family::Recipe).filter(Attribute::Profession, FilterValue::Binding(3)),
        // 5: items those recipes make.
        q(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(4)),
        // 6: a binding index that is not an earlier binding matches nothing.
        q(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(9)),
    ];
    let r = s.resolve_all(&queries).expect("resolves");
    let d: Vec<Vec<u32>> = r.iter().map(|x| dids(&x.records)).collect();
    assert_eq!(d[0], [0x7000_0202]);
    assert_eq!(d[1], [0x7000_0401]);
    assert_eq!(d[2], [0x7000_0101, 0x7000_0102]);
    assert_eq!(d[3], [0x7000_0601]);
    assert_eq!(d[4], [0x7000_0602]);
    assert_eq!(d[5], [0x7000_0103]);
    assert!(d[6].is_empty());
}

#[test]
fn binding_uses_the_counted_list() {
    let s = store();
    let queries = [
        q(Family::VendorInventory).count(Count::First(1)),
        q(Family::Item).filter(Attribute::ListedBy, FilterValue::Binding(0)),
    ];
    let r = s.resolve_all(&queries).expect("resolves");
    assert_eq!(dids(&r[0].records), [0x7000_0401]);
    assert_eq!(dids(&r[1].records), [0x7000_0101, 0x7000_0102]);
}

#[test]
fn unknown_labels_are_dropped_and_reported() {
    let s = store();
    let (d, notes) = resolve(
        &s,
        &q(Family::Item)
            .filter(
                Attribute::Category,
                FilterValue::Alternatives(vec![
                    Alt::Label("@Nonesuch".into()),
                    Alt::Label("@Sword".into()),
                    Alt::Label("@display Food".into()),
                ]),
            )
            .filter(Attribute::Quality, labels(&["@Shiny"])),
    );
    assert!(d.is_empty(), "the quality filter has no alternative left");
    assert_eq!(
        notes,
        [
            ResolveNote::UnmatchedLabel {
                attribute: Attribute::Category,
                label: "@Nonesuch".into()
            },
            ResolveNote::UnmatchedLabel {
                attribute: Attribute::Category,
                label: "@display Food".into()
            },
            ResolveNote::UnmatchedLabel {
                attribute: Attribute::Quality,
                label: "@Shiny".into()
            },
        ]
    );
    let (d, _) = resolve(
        &s,
        &q(Family::Item).filter(Attribute::Category, labels(&["@Nonesuch", "@Food"])),
    );
    assert_eq!(d, [0x7000_0102]);
}

#[test]
fn counts_and_partial_results() {
    let s = store();
    let (d, notes) = resolve(&s, &q(Family::Skill).count(Count::First(2)));
    assert_eq!(d, [0x7000_0301, 0x7000_0302]);
    assert!(notes.is_empty());
    let (d, notes) = resolve(&s, &q(Family::Skill).count(Count::First(5)));
    assert_eq!(d.len(), 3);
    assert_eq!(
        notes,
        [ResolveNote::Short {
            requested: 5,
            found: 3
        }]
    );
    let (d, notes) = resolve(&s, &q(Family::Skill).count(Count::Sample { n: 2, seed: 7 }));
    assert_eq!(d.len(), 2);
    assert!(notes.is_empty());
    let (d, notes) = resolve(
        &s,
        &q(Family::Item)
            .filter(Attribute::Level, range(Some(1000), None))
            .count(Count::First(1)),
    );
    assert!(d.is_empty());
    assert_eq!(
        notes,
        [ResolveNote::Short {
            requested: 1,
            found: 0
        }]
    );
}

#[test]
fn truncation_keeps_the_first_in_canonical_order() {
    let s = store();
    let mut out = Vec::new();
    let o = s.candidates(&q(Family::Area), &[], 2, &mut out);
    assert_eq!(o.status, CandidateStatus::Truncated);
    assert_eq!(dids(&out), [0x7000_0711, 0x7000_0712]);
    let mut out = Vec::new();
    let o = s.candidates(&q(Family::Place), &[], 1, &mut out);
    assert_eq!(o.status, CandidateStatus::Truncated);
    assert_eq!(out[0].row, 0);
}

#[test]
fn unsupported_did_and_no_install() {
    let s = store();
    let (d, notes) = resolve(&s, &q(Family::Hotspot));
    assert!(d.is_empty());
    assert_eq!(notes, [ResolveNote::Unsupported]);
    assert_eq!(resolve(&s, &q(Family::Pet)).1, [ResolveNote::Unsupported]);
    let (d, _) = resolve(
        &s,
        &SelectorQuery::dids([
            Did(0x7000_0302),
            Did(0x7000_0101),
            Did(0x7000_0F01),
            Did(0x1234),
            Did(0x7000_0101),
        ]),
    );
    assert_eq!(d, [0x7000_0101, 0x7000_0302]);
    let r = resolve_without_install(&[q(Family::Item), q(Family::Place)]);
    assert!(
        r.iter()
            .all(|x| x.records.is_empty() && x.notes == [ResolveNote::NoInstall])
    );
}

#[test]
fn table_families() {
    let s = store();
    let r = s.resolve(&q(Family::Place), &[]).unwrap();
    assert_eq!(
        r.records
            .iter()
            .map(|c| (c.did.0, c.row))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 2), (0x7000_0714, 1)]
    );
    let alpha = r.records[0];
    assert_eq!(
        alpha.location,
        Some(Location {
            region: 1,
            x: 1605.0,
            y: 3206.0,
            z: 7.0
        })
    );
    assert!(alpha.anchor.is_none());
    let beta = r.records[2];
    let a = beta.anchor.expect("interior anchor");
    assert_eq!((a.region, a.x, a.y, a.cell), (2, 1, 1, 0x0105));
    let reg = |f, v: &[i64]| {
        dids(
            &s.resolve(
                &q(f).filter(Attribute::Region, FilterValue::Integers(v.to_vec())),
                &[],
            )
            .unwrap()
            .records,
        )
    };
    assert_eq!(reg(Family::Place, &[2]), [0x7000_0714]);
    assert_eq!(reg(Family::MapNote, &[1]), [0x7000_0202]);
    assert_eq!(reg(Family::MapNote, &[1, 2]), [0x7000_0201, 0x7000_0202]);
    let ty = dids(
        &s.resolve(
            &q(Family::MapNote).filter(Attribute::Type, labels(&["@Trainer"])),
            &[],
        )
        .unwrap()
        .records,
    );
    assert_eq!(ty, [0x7000_0201]);
    assert_eq!(
        s.display_name(Did(0x7000_0714), Some(1)).as_deref(),
        Some("Beta")
    );
    assert_eq!(
        s.display_name(Did(0x7000_0714), Some(0)),
        None,
        "row and DID must agree"
    );
    assert!(s.table_failures().is_empty());
}

#[test]
fn enum_values_outside_selectors() {
    let s = store();
    assert_eq!(s.enum_value(LabelContext::Class, "@Hunter"), Some(24));
    assert_eq!(s.enum_value(LabelContext::Species, "@Halfling"), Some(81));
    assert_eq!(s.enum_value(LabelContext::Sex, "m"), Some(0x1000));
    assert_eq!(s.enum_value(LabelContext::Sex, "f"), Some(0x2000));
    assert_eq!(s.enum_value(LabelContext::Sex, "@Female"), Some(0x2000));
    assert_eq!(s.enum_value(LabelContext::EquipSlot, "@Equipment"), Some(5));
    assert_eq!(s.enum_value(LabelContext::DamageType, "@Fire"), Some(2));
    assert_eq!(s.enum_value(LabelContext::Class, "@Nobody"), None);
    assert_eq!(
        s.enum_value(LabelContext::Class, "@display Hunter"),
        None,
        "display texts never match"
    );
}

#[test]
fn typed_entry_points() {
    let s = store();
    let item = s.item(Did(0x7000_0101)).unwrap().expect("an item");
    assert_eq!(item.view().level(), Some(10));
    assert_eq!(
        item.kind(),
        tessera_formats::gameplay::items::ItemKind::Weapon
    );
    assert!(
        s.item(Did(0x7000_0104)).unwrap().is_none(),
        "item-like is no item"
    );
    let npc = s.creature(Did(0x7000_0202)).unwrap().expect("a creature");
    assert_eq!(npc.identity.species, Some(2));
    let quest = s.quest(Did(0x7000_0501)).unwrap().expect("a quest");
    assert_eq!(quest.record().unwrap().challenge_level(), Some(20));
    let t = s
        .class_table(Did(0x7000_0311))
        .unwrap()
        .expect("a class table");
    assert_eq!(t.class, Some(23));
    assert!(s.economy(Did(0x7000_0602)).unwrap().is_some());
    assert!(s.map_kind(Did(0x7000_0713)).unwrap().is_some());
    assert!(s.places().is_some() && s.map_notes().is_some());
    assert!(matches!(
        s.object(Did(0x1234)),
        Err(GameDataError::NotObject(_))
    ));
    assert!(s.object(Did(0x7000_0F01)).unwrap().is_none());
    assert!(matches!(
        s.object(Did(0x7000_0F03)),
        Err(GameDataError::View { .. })
    ));
}

#[test]
fn resolve_limit_is_an_error() {
    let big: Vec<Candidate> = (0..600_000u32).map(|i| Candidate::record(Did(i))).collect();
    let queries = [q(Family::Item), q(Family::Item)];
    let r = resolve_with(&queries, |_, _| {
        (CandidateOutcome::new(CandidateStatus::Ready), big.clone())
    });
    assert!(
        matches!(r, Err(GameDataError::ResolveLimit { index: 1, limit }) if limit == MAX_RESOLVED_TOTAL)
    );
    let failed = resolve_with(&queries[..1], |_, _| {
        (
            CandidateOutcome::new(CandidateStatus::Failed("broken".into())),
            Vec::new(),
        )
    });
    assert!(failed.is_err());
}

#[test]
fn hostile_installs_never_panic() {
    // An install with nothing: no dictionary, no library.
    let empty = GameDataStore::new(
        tessera_wstate::ObjectStore::new(MemorySource::new()),
        vec![Did(0x7000_0001)],
    );
    let mut out = Vec::new();
    let o = empty.candidates(&q(Family::Item), &[], 10, &mut out);
    assert!(matches!(o.status, CandidateStatus::Failed(_)));
    assert!(empty.resolve(&q(Family::Item), &[]).is_err());
    assert_eq!(empty.enum_value(LabelContext::Class, "@X"), None);
    // Garbage in every record of a real-looking world.
    let mut w = world();
    for i in 0..64u32 {
        let did = 0x7000_0E00 + i;
        let mut root = did.to_le_bytes().to_vec();
        root.extend_from_slice(&(i * 97).to_le_bytes());
        w.raw(did, root, true);
        let junk: Vec<u8> = (0..i)
            .map(|k| u8::try_from((k * 31 + i) % 251).unwrap())
            .collect();
        let mut set = (did + 0x0900_0000).to_le_bytes().to_vec();
        set.extend_from_slice(&junk);
        w.raw(did + 0x0900_0000, set, false);
    }
    let s = w.build();
    for f in tessera_gamedata::Family::ALL {
        let _ = s.resolve(&q(f), &[]);
    }
    let ix = s.index().unwrap();
    assert!(ix.failures_total() >= 4);
}

#[test]
fn two_stores_give_identical_lists() {
    let a = store();
    let b = store();
    for f in Family::ALL {
        let qq = q(f).count(Count::Sample { n: 2, seed: 0x5EED });
        assert_eq!(
            a.resolve(&qq, &[]).unwrap(),
            b.resolve(&qq, &[]).unwrap(),
            "{f:?}"
        );
    }
}

#[test]
fn store_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<GameDataStore<MemorySource>>();
    check::<tessera_gamedata::RecordIndex>();
}

// ---------------------------------------------------------------------------------------------
// Review regressions
// ---------------------------------------------------------------------------------------------

/// String access with no tables: literal StringInfos render, table references miss.
fn no_tables() -> tessera_text::Strings {
    tessera_text::Strings::new(tessera_text::MemorySource::new())
}

#[test]
fn recipe_tier_matches_tier_array_elements() {
    // economy.md §5.1 rule 1: a tier-array recipe has no top-level tier; §8.2 "for a list,
    // one of its elements".
    let mut w = world();
    w.object(
        0x7000_0604,
        C_RECIPE,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0101)),
            ("CraftRecipe_Profession", v_u32(0x7000_0601)),
            (
                "CraftRecipe_TierArray",
                v_array(&[(pid("CraftRecipe_Tier"), v_u32(2))]),
            ),
        ]),
    );
    let s = w.build();
    let f = |v| resolve(&s, &q(Family::Recipe).filter(Attribute::Tier, v)).0;
    assert_eq!(f(labels(&["@Journeyman"])), [0x7000_0602, 0x7000_0604]);
    assert_eq!(f(vals(&[2])), [0x7000_0602, 0x7000_0604]);
    assert_eq!(f(vals(&[1])), [0x7000_0603]);
}

#[test]
fn class_lists_count_only_entry_skills() {
    // An entry with a prerequisite skill but no `AdvTable_Skill` lists no skill
    // (combat-data.md §9.1; scenario-feed.md §8.3 "listed in").
    let mut w = world();
    w.object(
        0x7000_0313,
        C_CLASS_TABLE,
        Some(&[
            ("AdvTable_Class", v_u32(25)),
            (
                "AdvTable_AvailableSkillEntryList",
                structs(
                    "AdvTable_AvailableSkillEntry",
                    &[s_struct(&[(
                        "AdvTable_PrerequisiteSkill",
                        v_u32(0x7000_0301),
                    )])],
                ),
            ),
            (
                "AdvTable_AvailableMountSkillEntryList",
                structs(
                    "AdvTable_AvailableSkillEntry",
                    &[s_struct(&[(
                        "AdvTable_PrerequisiteSkill",
                        v_u32(0x7000_0302),
                    )])],
                ),
            ),
        ]),
    );
    let s = w.build();
    let sk = |a, v| resolve(&s, &q(Family::Skill).filter(a, v)).0;
    assert!(sk(Attribute::Class, vals(&[25])).is_empty());
    assert_eq!(
        sk(Attribute::Mount, FilterValue::Bool(true)),
        [0x7000_0303],
        "a prerequisite in the mount list is no mount skill"
    );
}

#[test]
fn did_family_honours_the_limit() {
    let s = store();
    let query = SelectorQuery::dids([Did(0x7000_0302), Did(0x7000_0101)]);
    let mut out = Vec::new();
    let o = s.candidates(&query, &[], 1, &mut out);
    assert_eq!(o.status, CandidateStatus::Truncated);
    assert_eq!(dids(&out), [0x7000_0101], "the first in canonical order");
    out.clear();
    let o = s.candidates(&query, &[], 2, &mut out);
    assert_eq!(o.status, CandidateStatus::Ready);
    assert_eq!(dids(&out), [0x7000_0101, 0x7000_0302]);
}

#[test]
fn limit_zero_appends_nothing() {
    let s = store();
    for family in [Family::Place, Family::Item, Family::Did] {
        let query = if family == Family::Did {
            SelectorQuery::dids([Did(0x7000_0101)])
        } else {
            q(family)
        };
        let mut out = Vec::new();
        let o = s.candidates(&query, &[], 0, &mut out);
        assert!(out.is_empty(), "{family:?}");
        assert_eq!(o.status, CandidateStatus::Truncated, "{family:?}");
    }
    let mut out = Vec::new();
    let o = s.candidates(
        &q(Family::Item).filter(Attribute::Level, range(Some(9000), None)),
        &[],
        0,
        &mut out,
    );
    assert_eq!(o.status, CandidateStatus::Ready, "nothing to cut");
}

#[test]
fn index_errors_keep_their_variant() {
    // No dictionary: the property layer's error, the same on every call.
    let empty = GameDataStore::new(
        tessera_wstate::ObjectStore::new(MemorySource::new()),
        vec![Did(0x7000_0001)],
    );
    assert!(matches!(empty.index(), Err(GameDataError::Props(_))));
    assert!(matches!(empty.index(), Err(GameDataError::Props(_))));
    assert!(empty.is_index_built());
    // A record bound.
    let s = world()
        .build()
        .with_options(IndexOptions::default().with_max_records(5));
    assert!(matches!(
        s.index(),
        Err(GameDataError::TooManyRecords { limit: 5, found }) if found > 5
    ));
    let mut out = Vec::new();
    let o = s.candidates(&q(Family::Item), &[], 10, &mut out);
    assert!(matches!(o.status, CandidateStatus::Failed(_)));
    assert!(matches!(
        s.resolve(&q(Family::Item), &[]),
        Err(GameDataError::IndexUnavailable(_))
    ));
}

#[test]
fn index_options_are_bounded() {
    let o = IndexOptions::default().with_threads(usize::MAX);
    assert_eq!(o.threads, tessera_gamedata::MAX_INDEX_THREADS);
    assert_eq!(IndexOptions::default().with_threads(0).threads, 1);
    assert_eq!(
        IndexOptions::default()
            .with_max_records(usize::MAX)
            .max_records,
        tessera_gamedata::MAX_OBJECT_RECORDS
    );
    // A huge thread count on a small install still resolves (and never spawns one thread
    // per record).
    let mut w = world();
    for i in 0..600u32 {
        w.object(
            0x7100_0000 + i,
            C_ITEM_BASE,
            Some(&[("Item_Level", v_i32(1))]),
        );
    }
    let mut s = w.build();
    s = s.with_options(IndexOptions::default().with_threads(usize::MAX));
    assert!(s.family(Family::Item).unwrap().len() > 600);
}

/// Bindings given by an adapter's own lookup (DIDs only).
struct ProfessionBinding;

impl tessera_gamedata::Bindings for ProfessionBinding {
    fn for_each_did(&self, index: usize, f: &mut dyn FnMut(Did)) {
        if index == 0 {
            f(Did(0x7000_0601));
        }
    }
}

#[test]
fn bindings_through_a_lookup() {
    let s = store();
    let query = q(Family::Recipe).filter(Attribute::Profession, FilterValue::Binding(0));
    let mut out = Vec::new();
    let o = s.candidates_with(&query, &ProfessionBinding, 10, &mut out);
    assert_eq!(o.status, CandidateStatus::Ready);
    assert_eq!(dids(&out), [0x7000_0602]);
    // The same through the slice form.
    let lists = vec![vec![Candidate::record(Did(0x7000_0601))]];
    let mut out2 = Vec::new();
    s.candidates(&query, &lists, 10, &mut out2);
    assert_eq!(out, out2);
    // An index that names no binding matches nothing.
    let none = q(Family::Recipe).filter(Attribute::Profession, FilterValue::Binding(3));
    out.clear();
    s.candidates_with(&none, &ProfessionBinding, 10, &mut out);
    assert!(out.is_empty());
}

#[test]
fn combat_lookups_do_not_build_the_index() {
    let s = store();
    // The synthetic skills lack the properties the skill view requires: a member whose
    // view fails is an error (as `combat` reports it), a non-member is `None`.
    assert!(
        matches!(s.skill(Did(0x7000_0301)), Err(GameDataError::View { .. })),
        "a skill by its class"
    );
    assert!(
        s.skill(Did(0x7000_0311)).unwrap().is_none(),
        "a class table"
    );
    assert!(s.skill(Did(0x1234)).unwrap().is_none(), "not an object");
    assert!(
        !matches!(s.effect(Did(0x7000_0331)), Ok(None)),
        "an effect by its class"
    );
    assert!(s.effect(Did(0x7000_0301)).unwrap().is_none());
    assert!(s.class_table(Did(0x7000_0312)).unwrap().is_some());
    assert!(s.class_table(Did(0x7000_0101)).unwrap().is_none());
    assert!(!s.is_index_built());
    // The same answers as the index gives.
    let ix = s.index().unwrap();
    assert!(ix.contains(Family::Skill, Did(0x7000_0301)));
    assert!(ix.contains(Family::Effect, Did(0x7000_0331)));
    assert!(ix.contains(Family::Class, Did(0x7000_0312)));
}

#[test]
fn sex_values_come_from_the_race_tables() {
    // Only the male entry: `f` stays unresolved (§5.1: unresolved means absent).
    let mut w = world();
    w.object(
        0x7000_0A02,
        C_OTHER,
        Some(&[
            ("RaceTable_Race", v_u32(23)),
            (
                "RaceTable_GenderList",
                structs(
                    "RaceTable_GenderStruct",
                    &[s_struct(&[("RaceTable_GenderType", v_u32(0x1000))])],
                ),
            ),
        ]),
    );
    let s = w.build();
    assert_eq!(s.enum_value(LabelContext::Sex, "m"), Some(0x1000));
    assert_eq!(s.enum_value(LabelContext::Sex, "f"), None);
    assert_eq!(
        s.enum_value(LabelContext::Sex, "@Female"),
        Some(0x2000),
        "labels go through the mapper"
    );
    // No race tables at all.
    let mut bare = Synth::new();
    bare.object(0x7000_0001, C_OTHER, Some(&[]));
    let b = bare.build();
    assert_eq!(b.enum_value(LabelContext::Sex, "m"), None);
    assert_eq!(b.enum_value(LabelContext::Sex, "f"), None);
}

#[test]
fn display_names_by_family() {
    let mut w = world();
    // A skill with both names: the family's own label wins.
    w.object(
        0x7000_0304,
        C_SKILL,
        Some(&[
            ("Name", v_text("Generic")),
            ("Skill_Name", v_text("Own name")),
        ]),
    );
    // An empty name is no name.
    w.object(0x7000_0305, C_SKILL, Some(&[("Skill_Name", v_text(""))]));
    w.object(
        0x7000_0323,
        C_TRAIT,
        Some(&[("Trait_Name", v_text("Trait"))]),
    );
    w.object(
        0x7000_0604,
        C_RECIPE,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0101)),
            ("CraftRecipe_Profession", v_u32(0x7000_0601)),
            ("CraftRecipe_Tier", v_u32(1)),
            ("CraftRecipe_Name", v_text("Recipe")),
        ]),
    );
    w.object(
        0x7000_0605,
        C_RECIPE,
        Some(&[
            ("CraftProfession_TierArray", v_array(&[])),
            ("CraftProfession_Name", v_text("Profession")),
        ]),
    );
    w.object(
        0x7000_0802,
        C_OTHER,
        Some(&[
            ("PrivateEncounterTemplate_ContentLayer", v_u32(3)),
            ("PrivateEncounterTemplate_Name", v_text("Instance")),
        ]),
    );
    w.object(
        0x7000_0715,
        C_OTHER,
        Some(&[
            ("Area_Territory", v_u32(0x7000_0712)),
            ("Area_Name", v_text("Area")),
        ]),
    );
    // The mob of map-note row 1, now named (a tab inside).
    w.object(
        0x7000_0201,
        C_MOB,
        Some(&[("WeenieType", v_u32(1)), ("Name", v_text("Mob\tone"))]),
    );
    let bare = w.build();
    assert_eq!(
        bare.display_name(Did(0x7000_0101), None),
        None,
        "record names need string access"
    );
    assert_eq!(
        bare.display_name(Did(0x7000_0714), Some(1)).as_deref(),
        Some("Beta"),
        "place names do not"
    );
    let s = w.build().with_strings(std::sync::Arc::new(no_tables()));
    let name = |d: u32| s.display_name(Did(d), None);
    assert_eq!(name(0x7000_0101).as_deref(), Some("Blade"));
    assert_eq!(name(0x7000_0304).as_deref(), Some("Own name"));
    assert_eq!(name(0x7000_0305), None, "empty text");
    assert_eq!(name(0x7000_0323).as_deref(), Some("Trait"));
    assert_eq!(name(0x7000_0501).as_deref(), Some("q1"));
    assert_eq!(name(0x7000_0514).as_deref(), Some("f"), "faction");
    assert_eq!(
        name(0x7000_0701).as_deref(),
        Some("x"),
        "travel destination"
    );
    assert_eq!(name(0x7000_0714).as_deref(), Some("d"), "dungeon");
    assert_eq!(name(0x7000_0715).as_deref(), Some("Area"));
    assert_eq!(name(0x7000_0604).as_deref(), Some("Recipe"));
    assert_eq!(name(0x7000_0605).as_deref(), Some("Profession"));
    assert_eq!(name(0x7000_0802).as_deref(), Some("Instance"));
    assert_eq!(name(0x7000_0331), None, "no name property");
    assert_eq!(name(0x1234), None, "not an object");
    // A map-note element is named by its entity (§8.6), not by the row's text.
    assert_eq!(
        s.display_name(Did(0x7000_0201), Some(1)).as_deref(),
        Some("Mob one")
    );
    assert_eq!(
        s.display_name(Did(0x7000_0202), Some(0)),
        None,
        "an unnamed entity"
    );
    assert_eq!(
        s.display_name(Did(0x7000_0201), Some(0)),
        None,
        "row and DID must agree"
    );
}

#[test]
fn display_names_follow_sanitize_text() {
    // 63 bytes before a two-byte character: the cut falls before it. Control characters
    // become spaces; nothing is trimmed.
    let long = format!(" Ab\ncd{}\u{e9}zz", "x".repeat(57));
    assert_eq!(long.find('\u{e9}'), Some(63));
    let mut w = Synth::new();
    w.table(
        0x9E,
        0x0E00_0004,
        "PlacesTable",
        places_table(
            0x0E00_0004,
            &[
                (&long, 1, (1, 1), 0, [0.0; 3], 0),
                ("tab\there ", 1, (1, 1), 0, [0.0; 3], 0),
            ],
        ),
    );
    let s = w.build();
    let first = s.display_name(Did(0), Some(0)).expect("a place name");
    assert_eq!(first, format!(" Ab cd{}", "x".repeat(57)));
    assert_eq!(first.len(), 63);
    assert_eq!(
        s.display_name(Did(0), Some(1)).as_deref(),
        Some("tab here ")
    );
}
