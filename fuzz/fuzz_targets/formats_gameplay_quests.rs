// SPDX-License-Identifier: GPL-3.0-or-later
//! Quest-family views (`tessera_formats::gameplay::quests`, `docs/specs/quests.md`) on
//! arbitrary bytes.
//!
//! Byte 0 selects the mode: bit 0 clear → the map-note record (§17.1) with a total
//! synthetic schema; bit 0 set → an embedded property set decoded with the same schema and
//! read through every view of the module.
//!
//! Every label the module reads gets its own synthetic PID whose type (PID mod 24) is the
//! type the spec gives the label (§4.2, §5.1–§5.6, §7.1, §8, §9, §10–§15): the containers
//! of §5.1 (objective array, objectives, condition arrays, conditions, event entries,
//! compound events, entry arrays, quest-level failure and tracked lists), the role arrays
//! and role structs of §7, the permission struct and requirement lists of §8 and the
//! reward, treasure and loot containers of §9 are structs and arrays, so the inputs reach
//! the numbering of §5.7. Bits 1–7 of byte 0 (when non-zero) remap about one label in
//! eight to a wrong type, so mistyped trees are fuzzed as well.
//!
//! Nothing may panic. The event numbering (§5.7) must give EventIDs 1 … N in order,
//! symmetric sibling lists (§6.2), an objective index that covers every event once, and the
//! map-note decoder must consume the payload exactly.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_quests -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::quests::{
    AccomplishmentDirectory, AccomplishmentVersionList, AccountModifier, ColourTextControl,
    ColourTextList, Emote, EmoteDirectory, EventNumbering, Faction, NpcComment, NpcCommentList,
    QuestArc, QuestArcDirectory, QuestControl, QuestItemDirectory, QuestItemRegionDirectory,
    QuestLabels, QuestMapNotes, QuestRecord, QuestSingletons, QuestTreasure, ReputationControl,
    RewardLevelTable, RewardList, Scene, Title, WorldEvent, WorldEventList, active_quest_roles,
    choose_reward_table,
};
use tessera_formats::{Did, PropertySet, PropertyType};

/// Every PID has a type: PID mod 24.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

// Type codes (`property-system.md` §3).
const STRING: u32 = 1;
const ENUM: u32 = 8;
const FLOAT: u32 = 9;
const PID: u32 = 10;
const STRUCT: u32 = 11;
const ARRAY: u32 = 12;
const STRING_INFO: u32 = 13;
const INT: u32 = 15;
const POSITION: u32 = 17;
const BITFIELD32: u32 = 18;
const DID: u32 = 20;
const BOOL: u32 = 21;

/// Struct-valued labels (§4.2, §5.1, §5.3, §7.1, §7.4, §8.1, §9.5).
const STRUCTS: &[&str] = &[
    "Quest_CompletionObjective",
    "QuestEvent_Entry",
    "Quest_Role",
    "DefaultPermissionBlobStruct",
    "Quest_PositiveFaction",
    "Quest_NegativeFaction",
    "QuestDispenser_QuestRole",
    "Usage_RequiredFaction",
];

/// Array-valued labels that do not end in `Array` or `List` (§4.2, §5.1–§5.6, §7, §8, §9,
/// §10.2, §13.3, §17.1).
const ARRAYS: &[&str] = &[
    "Quest_QuestsToComplete",
    "Quest_BestowalRoles",
    "Quest_GlobalRoles",
    "Quest_NPCObjRoles",
    "Quest_CompletionCondition",
    "Quest_FailureCondition",
    "Quest_TrackedCondition",
    "QuestEvent_CompoundEvent",
    "Quest_LootTable",
    "QuestEvent_Locations",
    "QuestDispenser_ActiveQuests",
    "Usage_QuestRequirements",
    "Usage_RequiredRaces",
    "Quest_PaperItemAwards",
    "QuestControl_MedalThresholds",
    "QuestControl_Points_Start_Mapping",
    "Accomplishment_Set",
    "Reputation_FactionTable",
    "Entity_ContentLayers",
];

/// DID-valued labels whose name does not end in `DID`, `_DID` or `_DataID`.
const DIDS: &[&str] = &[
    "Quest_NextQuest",
    "Quest_PrecludedQuest",
    "QuestDispenser_NPC",
    "QuestDispenser_VOSoundID",
    "QuestDispenser_PrivateEncounterTemplate",
    "QuestDispenser_TeleportationSceneID",
    "QuestDispenser_Quest",
    "QuestEvent_NPCTalk",
    "QuestEvent_QuestComplete",
    "QuestEvent_BestowedQuestID",
    "QuestEvent_WorldEvent",
    "QuestEvent_Detect",
    "QuestEvent_ChannelingState",
    "Quest_MonsterRegion",
    "Usage_QuestID",
    "QuestTreasure_Item",
    "Accomplishment_RequiredTrait",
    "Accomplishment_File",
    "Title_Icon",
    "Emote_IconImage",
    "Emote_AFK_Emote",
    "Reputation_Faction_AdvancementTable",
    "Reputation_Faction_TierNameProgression",
    "Reputation_AccountModifier",
    "WorldEvent_WorldEvent",
    "WorldEvent_Condition_WorldEventToCompareWith",
    "Keyframe_VideoTrack",
    "Keyframe_MovieTrack",
    "Quest_QuestItemDirectory",
];

/// StringInfo-valued labels.
const STRING_INFOS: &[&str] = &[
    "Quest_Name",
    "Quest_Description",
    "Quest_ObjectiveDescription",
    "Quest_ObjectiveProgressOverride",
    "Quest_ObjectiveBillboardOverride",
    "Quest_ObjectiveLoreOverride",
    "QuestEvent_CompoundProgressOverride",
    "QuestEvent_ProgressOverride",
    "QuestEvent_BillboardProgressOverride",
    "QuestDispenser_RoleSuccessText",
    "QuestDispenser_RoleFailureText",
    "QuestArc_Name",
    "Title_String",
    "Title_Description",
    "Emote_CommandString",
    "Emote_Description",
    "Reputation_Faction_Name",
    "Reputation_Faction_Description",
    "AccountModifier_Name",
    "WorldEvent_DescriptionString",
    "WorldEvent_ProgressString",
    "NPCComment_Title",
    "NPCComment_WindowLabel",
    "NPCComment_Content",
    "NPCComment_CloseButtonText",
    "NPC_ColorText",
    "Scene_Name",
    "Keyframe_Subtitle",
];

/// String-valued labels (role keys, drama names).
const STRINGS: &[&str] = &[
    "QuestEvent_RoleConstraint",
    "QuestEvent_Location_RoleConstraint",
    "QuestEvent_DramaName",
    "QuestEvent_ChannelingRoleConstraint",
    "QuestDispenser_RoleName",
    "QuestDispenser_RoleConstraint",
];

/// Bool-valued labels.
const BOOLS: &[&str] = &[
    "Quest_IsAccomplishment",
    "Quest_IgnoreDefaultChallengeLevel",
    "Quest_HideChallengeLevel",
    "Quest_IsDailyGlobalReset",
    "Quest_CanBeCancelled",
    "Quest_IsShareable",
    "Quest_IsInstanceQuest",
    "Quest_IsSessionQuest",
    "Quest_IsMonsterPlayQuest",
    "Quest_IsRaidQuest",
    "Quest_IsFellowshipRecommended",
    "Quest_IsSmallFellowshipRecommended",
    "Quest_IncludesTrackedEvents",
    "Quest_Condition_NeverFinish",
    "QuestEvent_ShowProgressText",
    "QuestEvent_ShowBillboardText",
    "QuestEvent_Location_EntireLandblock",
    "QuestEvent_LocationsAreOverrides",
    "QuestEvent_GiveItemsOnAdvance",
    "QuestEvent_WaitForDrama",
    "QuestEvent_DestroyInventoryItems",
    "QuestDispenser_IsRemote",
    "Accomplishment_ShowProgressBar",
    "Accomplishment_CanBeAccelerated",
    "Quest_IsHidden",
    "Quest_IsSessionAccomplishment",
    "Emote_AllowedWhileMounted",
    "Emote_MustBeGranted",
    "WorldEvent_StartActive",
    "WorldEvent_DefaultBoolValue",
    "Keyframe_ShowRandomImageFromUIL",
];

/// Float-valued labels.
const FLOATS: &[&str] = &[
    "Quest_TimeLimit",
    "QuestEvent_Location_Radius",
    "QuestEvent_TimeLimit",
    "QuestEffect_Spellcraft",
    "QuestDispenser_DetectRadius",
    "QuestReward_PaperItems",
    "Reputation_AccountModifier_FloatMod",
    "NPC_TextDuration",
    "Quest_ColorTextDetectionRadius",
    "Keyframe_Duration",
];

/// PID-valued labels.
const PIDS: &[&str] = &[
    "Quest_ChallengeLevelOverrideProperty",
    "QuestTreasure_Virtue",
    "Reputation_Faction_CurrentTier_PropertyName",
    "Reputation_Faction_EarnedReputation_PropertyName",
    "Reputation_Faction_GlobalCap_PropertyName",
    "Reputation_AccountModifier_HighWater_PropertyName",
    "WorldEvent_WorldPropertyName",
];

/// Int-valued labels (counts, levels, indices, amounts, timers).
const INTS: &[&str] = &[
    "Quest_ChallengeLevel",
    "Quest_MaxTimesCompletable",
    "Quest_SecondsUntilRetry",
    "Quest_AbandonRetryTimer",
    "Quest_FailureRetryTimer",
    "Quest_ObjectiveIndex",
    "Quest_CompletionConditionCount",
    "Quest_ItemQuantity",
    "Quest_LootItemProbability",
    "QuestEvent_EventOrder",
    "QuestEvent_Number",
    "QuestEvent_DailyMaximumIncrements",
    "QuestEvent_WorldEvent_Value",
    "QuestEvent_PlayerLevel",
    "QuestEvent_Points",
    "Usage_MinLevel",
    "Usage_MaxLevel",
    "Quest_PosRepToGive",
    "Quest_GoldToGive",
    "Quest_ExpToGive",
    "Quest_GloryToGive",
    "Quest_Points",
    "QuestTreasure_ItemQuantity",
    "QuestTreasure_Virtue_Increment",
    "Accomplishment_MinLevelToStart",
    "Title_Priority",
    "WorldEvent_DefaultIntValue",
    "WorldEvent_MinIntValue",
    "WorldEvent_MaxIntValue",
    "WorldEvent_ConditionValue",
    "WorldEvent_Control_DecayEpsilon",
    "WorldEvent_Control_ScheduleEpsilon",
    "Quest_ColorTextInterval",
];

/// The type the spec gives `label`; enum for the remaining labels (tiers, actions,
/// categories, operators, statuses, script keys, selector fields).
fn spec_type(label: &str) -> u32 {
    let is = |list: &[&str]| list.contains(&label);
    if is(STRUCTS) {
        STRUCT
    } else if is(ARRAYS) || label.ends_with("Array") || label.ends_with("List") {
        ARRAY
    } else if is(DIDS)
        || label.ends_with("DID")
        || label.ends_with("_DataID")
        || label.ends_with("_DID")
    {
        DID
    } else if is(STRING_INFOS) {
        STRING_INFO
    } else if is(STRINGS) {
        STRING
    } else if is(BOOLS) {
        BOOL
    } else if is(FLOATS) {
        FLOAT
    } else if is(PIDS) {
        PID
    } else if is(INTS) || label.starts_with("QuestReward_") && label.ends_with("Amount") {
        INT
    } else if label == "QuestEvent_Location_Position" {
        POSITION
    } else if label == "Quest_Lock_ResetDays" || label == "Quest_MonsterGenus" {
        BITFIELD32
    } else {
        ENUM
    }
}

/// Wrong types a remapped label may take.
const WRONG: [u32; 8] = [STRUCT, ARRAY, INT, ENUM, DID, STRING_INFO, STRING, BOOL];

/// The synthetic PID of label number `index` with type `ty`: one PID per label.
fn pid_for(index: usize, ty: u32) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX / 32) * 24 + ty
}

/// Label → PID with the spec's types (built once).
fn spec_pids() -> &'static HashMap<String, u32> {
    static PIDS: OnceLock<HashMap<String, u32>> = OnceLock::new();
    PIDS.get_or_init(|| {
        QuestLabels::labels()
            .enumerate()
            .map(|(i, label)| (label.to_owned(), pid_for(i, spec_type(label))))
            .collect()
    })
}

/// The labels for `seed`: the spec's types for seed 0, otherwise about one label in eight
/// moved to a wrong type chosen by the seed.
fn labels_for(seed: u8) -> QuestLabels {
    if seed == 0 {
        return QuestLabels::resolve(spec_pids());
    }
    let seed = u32::from(seed);
    let map: HashMap<String, u32> = QuestLabels::labels()
        .enumerate()
        .map(|(i, label)| {
            let ty = spec_type(label);
            let h = u32::try_from(i)
                .unwrap_or(0)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(seed.wrapping_mul(0x85EB_CA6B));
            let ty = if (h >> 7) % 8 == 0 {
                let wrong = WRONG[((h >> 11) % 8) as usize];
                if wrong == ty {
                    WRONG[((h >> 11) % 8 + 1) as usize % 8]
                } else {
                    wrong
                }
            } else {
                ty
            };
            (label.to_owned(), pid_for(i, ty))
        })
        .collect();
    QuestLabels::resolve(&map)
}

/// The §5.7 / §6.2 / §6.1 properties of a numbering.
fn check_numbering(numbering: &EventNumbering<'_>) {
    let n = numbering.len();
    for (i, e) in numbering.events.iter().enumerate() {
        assert_eq!(e.event_id as usize, i + 1);
        for s in numbering.siblings(e.event_id) {
            assert_ne!(s, e.event_id);
            assert!(numbering.siblings(s).contains(&e.event_id));
        }
    }
    let covered: usize = numbering.objective_index().values().map(Vec::len).sum();
    assert_eq!(covered, n);
}

fn exercise(labels: &QuestLabels, set: &PropertySet) {
    let _ = labels.classify_set(set);
    let singletons =
        QuestSingletons::resolve(|label| (label.len() % 3 == 0).then_some(Did(0x7000_0001)));
    let _ = labels.classify_record(Did(0x7000_0001), set, &singletons);
    let quest = QuestRecord::new_unchecked(labels, set);
    let _ = (
        quest.name(),
        quest.is_deed(),
        quest.deed(),
        quest.category(),
    );
    let _ = quest.effective_challenge_level(|p| Some(p as i32));
    let _ = (
        quest.quests_to_complete(),
        quest.next_quest(),
        quest.treasure(),
    );
    for list in RewardList::ALL {
        let _ = quest.tier(list);
    }
    let _ = (
        quest.fixed_amounts(),
        quest.medal_loot(),
        quest.paper_items(),
    );
    let _ = (quest.give_items(), quest.effects());
    for loot in quest.loot_table() {
        let _ = (loot.item(), loot.probabilities(), loot.monster_selectors());
    }
    if let Some(p) = quest.permissions() {
        let _ = (p.requirements(), p.or_requirements(), p.world_event_all());
        let _ = (
            p.required_classes(),
            p.required_races(),
            p.required_faction(),
        );
    }
    for role in quest
        .roles()
        .into_iter()
        .chain(quest.bestowal_roles())
        .chain(quest.global_roles())
    {
        let _ = (
            role.action(),
            role.objective_index(),
            role.objective_rule_holds(),
        );
        let _ = (role.text_variants(), role.success_text(), role.npc());
    }
    for c in quest.completion_comments() {
        let _ = (c.npcs(), c.role_constraints(), c.lines());
    }
    for o in quest.objectives() {
        let _ = (o.index(), o.description(), o.npc_roles());
        for c in o.completion.iter().chain(&o.failure) {
            for e in c.entries() {
                let _ = (e.event_type(), e.locations(), e.monster_selectors());
                let _ = (
                    e.world_event_condition(),
                    e.status(),
                    e.role(),
                    e.listen_list(),
                );
                let _ = (e.give_items(), e.effects(), e.time_limit());
            }
        }
    }
    let strict = quest.number_events();
    let lenient = quest.number_events_lenient();
    check_numbering(&lenient);
    if let Ok(numbering) = strict {
        check_numbering(&numbering);
        assert!(numbering.unindexed_objectives.is_empty());
        assert!(lenient.unindexed_objectives.is_empty());
        assert_eq!(numbering.len(), lenient.len());
    } else {
        assert!(!lenient.unindexed_objectives.is_empty());
    }
    let _ = (quest.failure_conditions(), quest.tracked_conditions());
    let _ = quest.negative_tier();
    let _ = active_quest_roles(labels, set);

    let table = RewardLevelTable::new(labels, set);
    for list in RewardList::ALL {
        let _ = (table.entries(list), table.amount(list, 4));
    }
    let control = QuestControl::new(labels, set);
    let tables = control.reward_level_tables();
    let _ = choose_reward_table(None, |_| true, &tables, 0);
    let _ = control.medal_thresholds();
    let t = QuestTreasure::new(labels, set);
    let _ = (
        t.fixed_items(),
        t.selectable_items(),
        t.fixed_virtues(),
        t.billing_tokens(),
    );
    let _ = AccomplishmentDirectory::new(labels, set).dids();
    let _ = AccomplishmentVersionList::new(labels, set).versions();
    let _ = QuestArc::new(labels, set).quests();
    let _ = QuestArcDirectory::new(labels, set).arcs();
    let _ = (Title::read(labels, set), Emote::read(labels, set));
    let _ = (
        EmoteDirectory::read(labels, set),
        Faction::read(labels, set),
    );
    let _ = (
        AccountModifier::read(labels, set),
        ReputationControl::read(labels, set),
    );
    let we = WorldEvent::new(labels, set);
    let _ = (
        we.all_conditions(),
        we.any_conditions(),
        we.count_conditions(),
    );
    let _ = WorldEventList::read(labels, set);
    if let Some(p) = NpcComment::new(labels, set).permissions() {
        let _ = p.requirements();
    }
    let _ = (
        NpcCommentList::read(labels, set),
        ColourTextList::read(labels, set),
    );
    let _ = (
        ColourTextControl::read(labels, set),
        Scene::read(labels, set),
    );
    let _ = QuestItemRegionDirectory::read(labels, set);
    let _ = QuestItemDirectory::read(labels, set);
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let seed = mode >> 1;
    if mode % 2 == 0 {
        if let Ok(notes) = QuestMapNotes::parse(rest, &total) {
            let labels = labels_for(seed);
            for note in &notes.notes {
                let _ = (note.content_layers(), note.role_constraint(&labels));
                let _ = note.quest_roles(&labels);
            }
            assert!(notes.for_dungeon(notes.did).count() <= notes.notes.len());
        }
    } else if let Ok(set) = PropertySet::parse(rest, &total) {
        let labels = labels_for(seed);
        exercise(&labels, &set);
    }
});
