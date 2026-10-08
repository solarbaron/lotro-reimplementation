// SPDX-License-Identifier: GPL-3.0-or-later
//! Quests, deeds and dialogue: quest records, objectives, roles, rewards, deeds, world
//! events, the quest-target index and quest map notes (0x0E400000).
//!
//! Spec: `docs/specs/quests.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`QuestLabels`] | Every property this module reads, resolved by dictionary label at run time | §1, §2.2 |
//! | [`QuestFamilyKind`], [`QuestLabels::classify`], [`QuestLabels::classify_record`], [`QuestSingletons`] | Record kinds of the family by their identifying properties; the singletons by their WEENIECONTENT labels | §2.1–§2.3 |
//! | [`QuestRecord`] | Quest or deed: identity, level, repetition, chaining, rewards, prerequisites | §4, §8, §9 |
//! | [`QuestRecord::effective_challenge_level`] | The level the client shows | §4.3 |
//! | [`Objective`], [`Condition`], [`CompoundEvent`], [`EventEntry`] | Source form of objectives, conditions and events | §5.1–§5.6 |
//! | [`EventNumbering`], [`QuestRecord::number_events`] | EventID, ObjectiveID, ConditionID, compound members, required counts | §5.2, §5.7, §6.1 |
//! | [`listen_list`] | Event types an event reacts to (compiled form) | §6.2 |
//! | [`Role`], [`RoleAction`], [`CompletionComment`], [`active_quest_roles`] | Roles, dialogue lines, NPC-side role lists | §7.1–§7.4 |
//! | [`NpcComment`], [`NpcCommentList`], [`ColourTextList`], [`ColourTextControl`], [`Scene`] | Free-standing dialogue, barks, scenes | §7.7–§7.9 |
//! | [`Permissions`], [`QuestRequirement`], [`QuestStatus`], [`Operator`], [`WorldEventCondition`] | Usage permissions and quest requirements | §8, §14.2 |
//! | [`RewardList`], [`RewardLevelTable`], [`choose_reward_table`], [`QuestControl`], [`QuestTreasure`] | Reward tiers, tables, treasure, medals | §9 |
//! | [`DeedInfo`], [`AccomplishmentDirectory`], [`AccomplishmentVersionList`] | Deeds and deed directories | §10 |
//! | [`QuestArc`], [`QuestArcDirectory`] | Quest arcs | §11 |
//! | [`Title`], [`Emote`], [`EmoteDirectory`] | Titles and emotes | §12 |
//! | [`Faction`], [`AccountModifier`], [`ReputationControl`] | Reputation | §13 |
//! | [`WorldEvent`], [`WorldEventList`] | World events | §14 |
//! | [`QuestItemRegionDirectory`], [`QuestItemDirectory`] | Quest-item directories | §15 |
//! | [`QuestMapNotes`] | The quest map-note record (byte layout) | §17 |
//!
//! The compiled quest object of the 0x70 record (§6) and the quest-target index (§16) are
//! object-state data and belong to `tessera-wstate`; this module reads only the property
//! sets (0x79) and the map-note record.
//!
//! Properties are found by label through the install's dictionary ([`QuestLabels`]); no
//! PID, class index or record DID of one install is hard-coded. Every view tolerates
//! absent and mistyped values (they read as `None`), so a hostile or unusual property tree
//! never makes a view fail; only [`QuestRecord::new`] (the three identifying properties)
//! and [`QuestRecord::number_events`] (an objective without an index; its lenient form
//! [`QuestRecord::number_events_lenient`] skips and reports such objectives) report errors.

use std::collections::{BTreeMap, BTreeSet};

use crate::did::Did;
use crate::did_map::DataIdMap;
use crate::error::{FormatError, Result};
use crate::gameplay::common::{Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::property::{
    Position, PropertyArray, PropertyDictionary, PropertyEntry, PropertyNames, PropertySchema,
    PropertyStruct, PropertyValue, ValueDecoder,
};
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// Properties of quest records, their objectives and conditions (§4, §5.1, §9.7).
    pub struct QuestPids {
        name = "Quest_Name",
        description = "Quest_Description",
        is_accomplishment = "Quest_IsAccomplishment",
        objective_array = "Quest_ObjectiveArray",
        role_array = "Quest_RoleArray",
        category = "Quest_Category",
        scope = "Quest_Scope",
        challenge_level = "Quest_ChallengeLevel",
        challenge_level_override = "Quest_ChallengeLevelOverrideProperty",
        ignore_default_challenge_level = "Quest_IgnoreDefaultChallengeLevel",
        hide_challenge_level = "Quest_HideChallengeLevel",
        max_times_completable = "Quest_MaxTimesCompletable",
        lock_type = "Quest_LockType",
        lock_reset_days = "Quest_Lock_ResetDays",
        daily_global_reset = "Quest_IsDailyGlobalReset",
        seconds_until_retry = "Quest_SecondsUntilRetry",
        abandon_retry_timer = "Quest_AbandonRetryTimer",
        failure_retry_timer = "Quest_FailureRetryTimer",
        time_limit = "Quest_TimeLimit",
        can_be_cancelled = "Quest_CanBeCancelled",
        is_shareable = "Quest_IsShareable",
        is_instance_quest = "Quest_IsInstanceQuest",
        is_session_quest = "Quest_IsSessionQuest",
        is_monster_play_quest = "Quest_IsMonsterPlayQuest",
        is_raid_quest = "Quest_IsRaidQuest",
        is_fellowship_recommended = "Quest_IsFellowshipRecommended",
        is_small_fellowship_recommended = "Quest_IsSmallFellowshipRecommended",
        next_quest = "Quest_NextQuest",
        precluded_quest = "Quest_PrecludedQuest",
        quests_to_complete = "Quest_QuestsToComplete",
        data_id = "Quest_DataID",
        bestowal_roles = "Quest_BestowalRoles",
        global_roles = "Quest_GlobalRoles",
        completion_comment_array = "Quest_CompletionCommentArray",
        failure_condition_array = "Quest_FailureConditionArray",
        tracked_conditions_array = "Quest_TrackedConditionsArray",
        includes_tracked_events = "Quest_IncludesTrackedEvents",
        points_start = "Quest_Points_Start",
        permissions = "DefaultPermissionBlobStruct",
        web_store_item = "WebStoreAccountItem_DataID",
        completion_objective = "Quest_CompletionObjective",
        objective_index = "Quest_ObjectiveIndex",
        completion_condition_array = "Quest_CompletionConditionArray",
        objective_description = "Quest_ObjectiveDescription",
        objective_progress_override = "Quest_ObjectiveProgressOverride",
        objective_billboard_override = "Quest_ObjectiveBillboardOverride",
        objective_lore_override = "Quest_ObjectiveLoreOverride",
        npc_obj_roles = "Quest_NPCObjRoles",
        completion_condition = "Quest_CompletionCondition",
        failure_condition = "Quest_FailureCondition",
        tracked_condition = "Quest_TrackedCondition",
        completion_condition_count = "Quest_CompletionConditionCount",
        condition_never_finish = "Quest_Condition_NeverFinish",
        event_entry = "QuestEvent_Entry",
        compound_event = "QuestEvent_CompoundEvent",
        event_entry_array = "QuestEvent_Entry_Array",
        compound_progress_override = "QuestEvent_CompoundProgressOverride",
        give_item_array = "Quest_GiveItemArray",
        item_did = "Quest_ItemDID",
        item_quantity = "Quest_ItemQuantity",
        loot_table = "Quest_LootTable",
        loot_item = "Quest_LootItemDID",
        loot_probability = "Quest_LootItemProbability",
        loot_probability_array = "Quest_LootItemProbabilityArray",
        loot_monster = "Quest_LootMonsterDID",
        loot_monster_selectors = "Quest_LootMonsterGenus_Array",
        loot_monster_exclusions = "Quest_LootMonsterGenus_ExclusionArray",
        effect_array = "Quest_EffectArray",
        effect_apply_role = "QuestEffect_ApplyRole",
        effect_spellcraft = "QuestEffect_Spellcraft",
    }
}

crate::property_ids! {
    /// Properties of event entries, locations and monster selectors (§5.3–§5.6).
    pub struct EventPids {
        id = "QuestEvent_ID",
        event_order = "QuestEvent_EventOrder",
        number = "QuestEvent_Number",
        progress_override = "QuestEvent_ProgressOverride",
        billboard_progress_override = "QuestEvent_BillboardProgressOverride",
        show_progress_text = "QuestEvent_ShowProgressText",
        show_billboard_text = "QuestEvent_ShowBillboardText",
        role_constraint = "QuestEvent_RoleConstraint",
        locations = "QuestEvent_Locations",
        location_position = "QuestEvent_Location_Position",
        location_radius = "QuestEvent_Location_Radius",
        location_entire_landblock = "QuestEvent_Location_EntireLandblock",
        location_role_constraint = "QuestEvent_Location_RoleConstraint",
        locations_are_overrides = "QuestEvent_LocationsAreOverrides",
        role = "Quest_Role",
        give_items_on_advance = "QuestEvent_GiveItemsOnAdvance",
        apply_effect_array = "QuestEvent_ApplyEffectArray",
        daily_maximum_increments = "QuestEvent_DailyMaximumIncrements",
        drama_name = "QuestEvent_DramaName",
        wait_for_drama = "QuestEvent_WaitForDrama",
        npc_talk = "QuestEvent_NPCTalk",
        item = "QuestEvent_ItemDID",
        destroy_inventory_items = "QuestEvent_DestroyInventoryItems",
        monster = "QuestEvent_MonsterDID",
        monster_selectors = "QuestEvent_MonsterGenus_Array",
        monster_exclusions = "QuestEvent_MonsterGenus_ExclusionArray",
        landmark = "QuestEvent_LandmarkDID",
        quest_complete = "QuestEvent_QuestComplete",
        quest_complete_category = "QuestEvent_QuestCompleteCategory",
        accomplishment_complete_category = "QuestEvent_AccomplishmentCompleteCategory",
        quest_complete_scope = "QuestEvent_QuestCompleteScope",
        bestowed_quest = "QuestEvent_BestowedQuestID",
        world_event = "QuestEvent_WorldEvent",
        world_event_operator = "QuestEvent_WorldEvent_Operator",
        world_event_value = "QuestEvent_WorldEvent_Value",
        emote = "QuestEvent_EmoteDID",
        emote_target = "QuestEvent_EmoteTargetDID",
        faction = "QuestEvent_FactionDID",
        reputation_tier = "QuestEvent_ReputationTier",
        skill = "QuestEvent_SkillDID",
        recipe = "QuestEvent_RecipeDID",
        hobby = "QuestEvent_HobbyDID",
        time_limit = "QuestEvent_TimeLimit",
        player_level = "QuestEvent_PlayerLevel",
        points = "QuestEvent_Points",
        script_callback_key = "QuestEvent_ScriptCallback_Key",
        status = "QuestEvent_Status",
        session = "QuestEvent_Session_DID",
        session_status = "QuestEvent_Session_Status",
        territory = "QuestEvent_TerritoryDID",
        detect = "QuestEvent_Detect",
        channeling_state = "QuestEvent_ChannelingState",
        channeling_role_constraint = "QuestEvent_ChannelingRoleConstraint",
        selector_genus = "Quest_MonsterGenus",
        selector_species = "Quest_MonsterSpecies",
        selector_subspecies = "Quest_MonsterSubspecies",
        selector_alignment = "Quest_MonsterAlignment",
        selector_class = "Quest_MonsterClass",
        selector_division = "Quest_MonsterDivision",
        selector_region = "Quest_MonsterRegion",
    }
}

crate::property_ids! {
    /// Properties of roles, dialogue lines and role lists (§7.1–§7.4).
    pub struct RolePids {
        action = "QuestDispenser_Action",
        npc = "QuestDispenser_NPC",
        role_name = "QuestDispenser_RoleName",
        role_constraint = "QuestDispenser_RoleConstraint",
        success_text = "QuestDispenser_RoleSuccessText",
        failure_text = "QuestDispenser_RoleFailureText",
        text_array = "QuestDispenser_TextArray",
        species = "QuestDispenser_Species",
        vo_sound = "QuestDispenser_VOSoundID",
        vo_script = "QuestDispenser_VOScriptID",
        start_script = "QuestDispenser_StartScriptID",
        is_remote = "QuestDispenser_IsRemote",
        detect_radius = "QuestDispenser_DetectRadius",
        private_encounter_template = "QuestDispenser_PrivateEncounterTemplate",
        session = "QuestDispenser_SessionDID",
        teleportation_scene = "QuestDispenser_TeleportationSceneID",
        active_quests = "QuestDispenser_ActiveQuests",
        quest_role = "QuestDispenser_QuestRole",
        quest = "QuestDispenser_Quest",
        npc_array = "QuestDispenser_NPCArray",
    }
}

crate::property_ids! {
    /// Usage-permission properties used by this family (§8.1, §8.2).
    pub struct UsagePids {
        min_level = "Usage_MinLevel",
        max_level = "Usage_MaxLevel",
        quest_requirements = "Usage_QuestRequirements",
        quest_requirements_or = "Usage_QuestRequirements_OrList",
        operator = "Usage_Operator",
        quest_id = "Usage_QuestID",
        quest_status = "Usage_QuestStatus",
        world_event_all = "Usage_WorldEvent_AllConditionList",
        world_event_any = "Usage_WorldEvent_AnyConditionList",
        required_classes = "Usage_RequiredClassList",
        required_races = "Usage_RequiredRaces",
        required_faction = "Usage_RequiredFaction",
        required_faction_did = "Usage_RequiredFaction_DataID",
        required_faction_tier = "Usage_RequiredFaction_Tier",
    }
}

crate::property_ids! {
    /// Reward properties of quests, reward-level records and the quest control (§9).
    pub struct RewardPids {
        reward_level = "Quest_RewardLevelDID",
        treasure = "Quest_QuestTreasureDID",
        exp_tier = "Quest_ExpTier",
        gold_tier = "Quest_GoldTier",
        item_exp_tier = "Quest_ItemExpTier",
        mount_exp_tier = "Quest_MountExpTier",
        craft_exp_tier = "Quest_CraftExpTier",
        craft_profession = "Quest_CraftProfessionDID",
        glory_tier = "Quest_GloryTier",
        store_points_tier = "Quest_TurbinePointTier",
        premium_coins_tier = "Quest_MithrilCoinsTier",
        session_points_tier = "Quest_SessionPointsTier",
        session_points_failure_tier = "Quest_SessionPointsFailureTier",
        positive_faction = "Quest_PositiveFaction",
        negative_faction = "Quest_NegativeFaction",
        faction = "Quest_FactionDID",
        rep_tier = "Quest_RepTier",
        pos_rep_to_give = "Quest_PosRepToGive",
        gold_to_give = "Quest_GoldToGive",
        exp_to_give = "Quest_ExpToGive",
        glory_to_give = "Quest_GloryToGive",
        medal_loot_array = "Quest_Medal_LootArray",
        medal = "Quest_Medal_LootArray_Medal",
        medal_treasure = "Quest_Medal_LootArray_TreasureDID",
        paper_item_awards = "Quest_PaperItemAwards",
        paper_item = "Quest_PaperItemDID",
        paper_item_tier = "Quest_PaperItemTier",
        tier = "QuestReward_Tier",
        failure_tier = "QuestReward_FailureTier",
        exp_list = "QuestReward_ExpEntryList",
        money_list = "QuestReward_MoneyEntryList",
        item_exp_list = "QuestReward_ItemExpEntryList",
        mount_exp_list = "QuestReward_MountExpEntryList",
        craft_exp_list = "QuestReward_CraftExpEntryList",
        glory_list = "QuestReward_GloryEntryList",
        store_points_list = "QuestReward_TurbinePointEntryList",
        premium_coins_list = "QuestReward_MithrilCoinEntryList",
        session_points_list = "QuestReward_SessionPointsEntryList",
        session_failure_points_list = "QuestReward_SessionFailurePointsEntryList",
        rep_list = "QuestReward_RepEntryList",
        virtue_exp_list = "QuestReward_VirtueExpEntryList",
        paper_items_list = "QuestReward_PaperItemsEntryList",
        deed_exp_list = "QuestReward_Accomplishment_ExpEntryList",
        deed_money_list = "QuestReward_Accomplishment_MoneyEntryList",
        deed_rep_list = "QuestReward_Accomplishment_RepEntryList",
        exp_amount = "QuestReward_ExpAmount",
        money_amount = "QuestReward_MoneyAmount",
        glory_amount = "QuestReward_GloryAmount",
        store_points_amount = "QuestReward_TurbinePointAmount",
        premium_coins_amount = "QuestReward_MithrilCoinAmount",
        session_points_amount = "QuestReward_SessionPointsAmount",
        session_failure_points_amount = "QuestReward_SessionFailurePointsAmount",
        rep_amount = "QuestReward_RepAmount",
        paper_items_amount = "QuestReward_PaperItems",
        reward_level_array = "QuestControl_RewardLevelDIDArray",
        medal_thresholds = "QuestControl_MedalThresholds",
        threshold_points = "Quest_Points",
        threshold_medal = "Quest_Medal",
        points_start_mapping = "QuestControl_Points_Start_Mapping",
        default_bestow_script = "Quest_DefaultBestowScriptID",
        default_comment_vo_script = "Quest_DefaultCommentVOScriptID",
    }
}

crate::property_ids! {
    /// Quest-treasure properties (§9.4).
    pub struct TreasurePids {
        fixed_items = "QuestTreasure_FixedItemArray",
        selectable_items = "QuestTreasure_SelectableItemArray",
        item = "QuestTreasure_Item",
        item_quantity = "QuestTreasure_ItemQuantity",
        fixed_titles = "QuestTreasure_FixedTitleArray",
        selectable_titles = "QuestTreasure_SelectableTitleArray",
        fixed_emotes = "QuestTreasure_FixedEmoteArray",
        selectable_emotes = "QuestTreasure_SelectableEmoteArray",
        fixed_traits = "QuestTreasure_FixedTraitArray",
        fixed_virtues = "QuestTreasure_FixedVirtueArray",
        virtue = "QuestTreasure_Virtue",
        virtue_increment = "QuestTreasure_Virtue_Increment",
        virtue_xp_tier = "QuestTreasure_Virtue_XP_Tier",
        fixed_billing_tokens = "QuestTreasure_FixedBillingTokenArray",
        fixed_runics = "QuestTreasure_FixedRunicArray",
        selectable_runics = "QuestTreasure_SelectableRunicArray",
    }
}

crate::property_ids! {
    /// Deed fields, deed directories, the version list and the deed control (§10).
    pub struct DeedPids {
        ui_tab = "Accomplishment_UITab",
        category = "Accomplishment_Category",
        ui_filter = "Accomplishment_UIFilter",
        min_level_to_start = "Accomplishment_MinLevelToStart",
        show_progress_bar = "Accomplishment_ShowProgressBar",
        acceleration_category = "Accomplishment_AccelerationCategory",
        can_be_accelerated = "Accomplishment_CanBeAccelerated",
        required_trait = "Accomplishment_RequiredTrait",
        is_hidden = "Quest_IsHidden",
        is_session_accomplishment = "Quest_IsSessionAccomplishment",
        list = "Accomplishment_List",
        file = "Accomplishment_File",
        set = "Accomplishment_Set",
        version_list = "Accomplishment_Version_List",
        version_number = "Accomplishment_Version_Number",
        ui_tab_elements = "Accomplishment_UITabElements_Array",
    }
}

crate::property_ids! {
    /// Arcs, titles, emotes, factions and account modifiers (§11–§13).
    pub struct LorePids {
        arc_name = "QuestArc_Name",
        arc_quests = "QuestArc_Quest_Array",
        arc_directory = "QuestArcDirectory_QuestArc_Array",
        title_string = "Title_String",
        title_description = "Title_Description",
        title_category = "Title_Category",
        title_exclusion_group = "Title_Exclusion_Group",
        title_priority = "Title_Priority",
        title_icon = "Title_Icon",
        emote_command = "Emote_CommandString",
        emote_description = "Emote_Description",
        emote_script = "Emote_ScriptID",
        emote_mounted_script = "Emote_Mounted_ScriptID",
        emote_icon = "Emote_IconImage",
        emote_allowed_while_mounted = "Emote_AllowedWhileMounted",
        emote_must_be_granted = "Emote_MustBeGranted",
        emote_available = "Emote_AvailableList",
        emote_afk = "Emote_AFK_Emote",
        faction_name = "Reputation_Faction_Name",
        faction_description = "Reputation_Faction_Description",
        faction_advancement = "Reputation_Faction_AdvancementTable",
        faction_tier_names = "Reputation_Faction_TierNameProgression",
        faction_current_tier_property = "Reputation_Faction_CurrentTier_PropertyName",
        faction_earned_property = "Reputation_Faction_EarnedReputation_PropertyName",
        faction_global_cap_property = "Reputation_Faction_GlobalCap_PropertyName",
        lowest_tier = "Reputation_LowestTier",
        highest_tier = "Reputation_HighestTier",
        default_tier = "Reputation_Faction_DefaultTier",
        account_modifier = "Reputation_AccountModifier",
        modifier_name = "AccountModifier_Name",
        modifier_list = "Reputation_AccountModifier_ModifierList",
        modifier_tier = "Reputation_AccountModifier_Tier",
        modifier_float = "Reputation_AccountModifier_FloatMod",
        modifier_high_water = "Reputation_AccountModifier_HighWater_PropertyName",
        faction_table = "Reputation_FactionTable",
    }
}

crate::property_ids! {
    /// World events, their conditions and lists (§14).
    pub struct WorldEventPids {
        world_property = "WorldEvent_WorldPropertyName",
        start_active = "WorldEvent_StartActive",
        default_bool = "WorldEvent_DefaultBoolValue",
        default_int = "WorldEvent_DefaultIntValue",
        min_int = "WorldEvent_MinIntValue",
        max_int = "WorldEvent_MaxIntValue",
        description = "WorldEvent_DescriptionString",
        progress = "WorldEvent_ProgressString",
        all_conditions = "WorldEvent_AllConditionList",
        any_conditions = "WorldEvent_AnyConditionList",
        count_conditions = "WorldEvent_CountConditionList",
        world_event = "WorldEvent_WorldEvent",
        operator = "WorldEvent_Operator",
        condition_value = "WorldEvent_ConditionValue",
        compare_with = "WorldEvent_Condition_WorldEventToCompareWith",
        event_list = "WorldEvent_WorldEventList",
        decay_epsilon = "WorldEvent_Control_DecayEpsilon",
        schedule_epsilon = "WorldEvent_Control_ScheduleEpsilon",
    }
}

crate::property_ids! {
    /// NPC comments, colour text, scenes, quest-item directories and map notes (§7.7–§7.9,
    /// §15, §17).
    pub struct NpcPids {
        comment_title = "NPCComment_Title",
        comment_window_label = "NPCComment_WindowLabel",
        comment_content = "NPCComment_Content",
        comment_close_button = "NPCComment_CloseButtonText",
        comment_vo_script = "NPCComment_VOScriptID",
        comment_array = "NPCComment_CommentArray",
        colour_text_array = "NPC_ColorTextArray",
        reputation_colour_text_array = "NPC_ReputationColorTextArray",
        colour_text = "NPC_ColorText",
        colour_text_min_tier = "NPC_ColorText_MinReputationTier",
        colour_text_max_tier = "NPC_ColorText_MaxReputationTier",
        text_duration = "NPC_TextDuration",
        colour_text_interval = "Quest_ColorTextInterval",
        colour_text_radius = "Quest_ColorTextDetectionRadius",
        scene_name = "Scene_Name",
        scene_keyframes = "Scene_KeyframeArray",
        keyframe_subtitle = "Keyframe_Subtitle",
        keyframe_video_track = "Keyframe_VideoTrack",
        keyframe_movie_track = "Keyframe_MovieTrack",
        keyframe_duration = "Keyframe_Duration",
        keyframe_random_image = "Keyframe_ShowRandomImageFromUIL",
        item_directory_array = "Quest_QuestItemDirectory_Array",
        item_region_directory_array = "Quest_QuestItemRegionDirectory_Array",
        item_directory = "Quest_QuestItemDirectory",
        agent_species = "Agent_Species",
        agent_subspecies = "Agent_Subspecies",
        drop_frequency = "Quest_DropFrequency",
        content_layers = "Entity_ContentLayers",
    }
}

/// Prefix of the labels of the quest-treasure kind (§2.1: "only `QuestTreasure_*`
/// properties").
pub const TREASURE_LABEL_PREFIX: &str = "QuestTreasure_";

/// Every property label this module reads, resolved against one install's dictionary.
///
/// Build it once per install with [`QuestLabels::from_dictionary`] (or
/// [`QuestLabels::resolve`] for any [`PropertyNames`]) and pass it to the views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestLabels {
    /// Quest records, objectives, conditions, items, loot, effects.
    pub quest: QuestPids,
    /// Event entries, locations, monster selectors.
    pub event: EventPids,
    /// Roles and role lists.
    pub role: RolePids,
    /// Usage permissions.
    pub usage: UsagePids,
    /// Rewards and the quest control.
    pub reward: RewardPids,
    /// Quest treasure.
    pub treasure: TreasurePids,
    /// Deeds and deed directories.
    pub deed: DeedPids,
    /// Arcs, titles, emotes, reputation.
    pub lore: LorePids,
    /// World events.
    pub world: WorldEventPids,
    /// NPC comments, colour text, scenes, quest-item directories, map notes.
    pub npc: NpcPids,
    /// PIDs whose label starts with [`TREASURE_LABEL_PREFIX`], sorted.
    treasure_pids: Vec<u32>,
}

impl QuestLabels {
    /// Resolves every label against `names`. The treasure kind is recognised by the
    /// treasure labels this module knows (§9.4); [`QuestLabels::from_dictionary`] also
    /// picks up treasure labels it does not know.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let treasure = TreasurePids::resolve(names);
        let mut treasure_pids: Vec<u32> = treasure.all().iter().filter_map(|p| p.id()).collect();
        treasure_pids.sort_unstable();
        treasure_pids.dedup();
        Self {
            quest: QuestPids::resolve(names),
            event: EventPids::resolve(names),
            role: RolePids::resolve(names),
            usage: UsagePids::resolve(names),
            reward: RewardPids::resolve(names),
            treasure,
            deed: DeedPids::resolve(names),
            lore: LorePids::resolve(names),
            world: WorldEventPids::resolve(names),
            npc: NpcPids::resolve(names),
            treasure_pids,
        }
    }

    /// Resolves every label against the install's dictionary; the treasure kind is every
    /// dictionary property whose label starts with [`TREASURE_LABEL_PREFIX`] (§2.1).
    #[must_use]
    pub fn from_dictionary(dict: &PropertyDictionary) -> Self {
        let mut labels = Self::resolve(dict);
        labels.treasure_pids.extend(
            dict.names()
                .iter()
                .filter(|(pid, name)| *pid != 0 && name.starts_with(TREASURE_LABEL_PREFIX))
                .map(|(pid, _)| *pid),
        );
        labels.treasure_pids.sort_unstable();
        labels.treasure_pids.dedup();
        labels
    }

    /// Every label unresolved (nothing matches).
    #[must_use]
    pub fn unresolved() -> Self {
        Self::resolve(&|_: &str| None::<u32>)
    }

    /// Every label this module reads, in declaration order.
    pub fn labels() -> impl Iterator<Item = &'static str> {
        QuestPids::LABELS
            .iter()
            .chain(EventPids::LABELS)
            .chain(RolePids::LABELS)
            .chain(UsagePids::LABELS)
            .chain(RewardPids::LABELS)
            .chain(TreasurePids::LABELS)
            .chain(DeedPids::LABELS)
            .chain(LorePids::LABELS)
            .chain(WorldEventPids::LABELS)
            .chain(NpcPids::LABELS)
            .copied()
    }

    /// The labels the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.quest.missing();
        out.extend(self.event.missing());
        out.extend(self.role.missing());
        out.extend(self.usage.missing());
        out.extend(self.reward.missing());
        out.extend(self.treasure.missing());
        out.extend(self.deed.missing());
        out.extend(self.lore.missing());
        out.extend(self.world.missing());
        out.extend(self.npc.missing());
        out
    }

    /// Whether `pid` is a quest-treasure property.
    #[must_use]
    pub fn is_treasure_pid(&self, pid: u32) -> bool {
        self.treasure_pids.binary_search(&pid).is_ok()
    }
}

// ---------------------------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------------------------

/// An enum key, bitfield or non-negative integer as an unsigned code. The dictionary
/// decides value types; small codes are read leniently so that a dictionary that types a
/// tier or action as Int still works.
fn code(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn code_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Option<u32> {
    code(a.get_pid(pid)?)
}

/// The elements of array `pid` that are structs, in stored order, whatever their PID.
fn structs_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Vec<&PropertyStruct> {
    a.structs(pid).collect()
}

/// The elements of array `pid` as unsigned codes, in stored order (others skipped).
fn codes_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Vec<u32> {
    a.elements(pid)
        .iter()
        .filter_map(|e| code(&e.value))
        .collect()
}

/// The DID elements of array `pid`, null DIDs included, in stored order.
fn raw_dids_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Vec<Did> {
    a.elements(pid)
        .iter()
        .filter_map(ValueExt::did_ref)
        .collect()
}

/// The StringInfo elements of array `pid` (or struct fields, for structs of one text).
fn texts_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Vec<&StringInfo> {
    a.elements(pid)
        .iter()
        .filter_map(ValueExt::string_info)
        .collect()
}

/// Whether element `e` is the array-valued property `pid`.
fn array_element(e: &PropertyEntry, pid: Pid) -> Option<&PropertyArray> {
    if pid.is(e.pid) {
        e.value.as_array()
    } else {
        None
    }
}

/// Whether element `e` is the struct-valued property `pid`.
fn struct_element(e: &PropertyEntry, pid: Pid) -> Option<&PropertyStruct> {
    if pid.is(e.pid) {
        e.value.as_struct()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Kinds (§2.1)
// ---------------------------------------------------------------------------------------------

/// A record kind of the quest family, recognised by its identifying properties (§2.1).
///
/// The quest-target index (§16) has an empty property set and is found by its
/// WEENIECONTENT label `QuestEventTargetLocationControl`, not by properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum QuestFamilyKind {
    /// Quest or deed (§4).
    Quest,
    /// Quest treasure (§9.4).
    QuestTreasure,
    /// Reward-level table (§9.2).
    RewardLevelTable,
    /// Quest arc (§11).
    QuestArc,
    /// Quest-arc directory (§11).
    QuestArcDirectory,
    /// Accomplishment directory (§10.2).
    AccomplishmentDirectory,
    /// Accomplishment version list (§10.3).
    AccomplishmentVersionList,
    /// Quest-item directory (§15).
    QuestItemDirectory,
    /// Quest-item region directory (§15).
    QuestItemRegionDirectory,
    /// Title (§12.1).
    Title,
    /// Emote (§12.2).
    Emote,
    /// Emote directory (§12.2).
    EmoteDirectory,
    /// Faction (§13.1).
    Faction,
    /// Reputation account modifier (§13.2).
    ReputationAccountModifier,
    /// Reputation control (§13.3).
    ReputationControl,
    /// Boolean world event (§14.1).
    WorldEventBool,
    /// Integer world event (§14.1).
    WorldEventInt,
    /// World-event list (§14.3).
    WorldEventList,
    /// NPC comment (§7.7).
    NpcComment,
    /// NPC comment list (§7.7).
    NpcCommentList,
    /// Colour-text list (§7.8).
    ColourTextList,
    /// Scene (§7.9).
    Scene,
    /// Quest control (§9.2, §9.8).
    QuestControl,
    /// Accomplishment control (§10.4).
    AccomplishmentControl,
    /// NPC colour-text control (§7.8).
    ColourTextControl,
}

impl QuestFamilyKind {
    /// Every kind, in classification order.
    pub const ALL: [Self; 25] = [
        Self::Quest,
        Self::QuestControl,
        Self::AccomplishmentControl,
        Self::ColourTextControl,
        Self::RewardLevelTable,
        Self::QuestArc,
        Self::QuestArcDirectory,
        Self::AccomplishmentDirectory,
        Self::AccomplishmentVersionList,
        Self::QuestItemDirectory,
        Self::QuestItemRegionDirectory,
        Self::Title,
        Self::Emote,
        Self::EmoteDirectory,
        Self::Faction,
        Self::ReputationAccountModifier,
        Self::ReputationControl,
        Self::WorldEventInt,
        Self::WorldEventBool,
        Self::WorldEventList,
        Self::NpcComment,
        Self::NpcCommentList,
        Self::ColourTextList,
        Self::Scene,
        Self::QuestTreasure,
    ];

    /// Whether the kind is a singleton of §2.3: its one record is found by its
    /// WEENIECONTENT label ([`QuestFamilyKind::weenie_content_label`]), not by properties
    /// (§2.2), so [`QuestLabels::classify`] never reports it.
    #[must_use]
    pub fn is_singleton(self) -> bool {
        self.weenie_content_label().is_some()
    }

    /// The WEENIECONTENT label of a singleton kind (§2.3); `None` for the other kinds.
    #[must_use]
    pub fn weenie_content_label(self) -> Option<&'static str> {
        use weenie_content_labels as l;
        Some(match self {
            Self::QuestControl => l::QUEST_CONTROL,
            Self::QuestArcDirectory => l::QUEST_ARC_DIRECTORY,
            Self::AccomplishmentVersionList => l::ACCOMPLISHMENT_VERSION_CONTROL,
            Self::AccomplishmentControl => l::ACCOMPLISHMENT_CONTROL,
            Self::QuestItemRegionDirectory => l::QUEST_ITEM_REGION_DIRECTORY,
            Self::EmoteDirectory => l::EMOTE_DIRECTORY,
            Self::ReputationControl => l::REPUTATION_CONTROL,
            Self::ColourTextControl => l::NPC_COLOUR_CONTROL,
            _ => return None,
        })
    }
}

/// WEENIECONTENT labels of the family's well-known records (§2.3). The DIDs are looked up
/// at run time ([`QuestSingletons`]) and never hard-coded.
pub mod weenie_content_labels {
    /// Quest control (§9.2, §9.8).
    pub const QUEST_CONTROL: &str = "QuestControl";
    /// Quest-arc directory (§11).
    pub const QUEST_ARC_DIRECTORY: &str = "QuestArcDirectory";
    /// Main accomplishment directory (§10.2).
    pub const ACCOMPLISHMENT_DIRECTORY: &str = "AccomplishmentDirectory";
    /// Auto-bestow directory, an accomplishment directory (§10.2).
    pub const AUTO_BESTOW_QUEST_DIRECTORY: &str = "AutoBestowQuestDirectory";
    /// Accomplishment version list (§10.3).
    pub const ACCOMPLISHMENT_VERSION_CONTROL: &str = "AccomplishmentVersionControl";
    /// Accomplishment control (§10.4).
    pub const ACCOMPLISHMENT_CONTROL: &str = "AccomplishmentControl";
    /// Quest-target index (§16).
    pub const QUEST_EVENT_TARGET_LOCATION_CONTROL: &str = "QuestEventTargetLocationControl";
    /// Quest-item region directory (§15).
    pub const QUEST_ITEM_REGION_DIRECTORY: &str = "QuestItemRegionDirectory";
    /// Quest-category limiter (§9.8; meaning OPEN).
    pub const QUEST_CATEGORY_LIMITER_CONTROL: &str = "QuestCategoryLimiterControl";
    /// Emote directory (§12.2).
    pub const EMOTE_DIRECTORY: &str = "EmoteDirectory";
    /// Reputation control (§13.3).
    pub const REPUTATION_CONTROL: &str = "ReputationControl";
    /// The main world-event list (§14.3).
    pub const WORLD_EVENT_CONTROL: &str = "WorldEventControl";
    /// NPC colour-text control (§7.8).
    pub const NPC_COLOUR_CONTROL: &str = "NPCColorControl";
    /// Monster-division control (§5.5).
    pub const MONSTER_DIVISION_CONTROL: &str = "MonsterDivisionControl";
    /// Version title grants (§12.1).
    pub const VERSION_ACCOUNT_TITLE_BESTOW: &str = "VersionAccountTitleBestow";
}

/// The well-known records of §2.3, resolved through the WEENIECONTENT DID map. A label
/// the map lacks, or that names the null DID, is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct QuestSingletons {
    /// `QuestControl`.
    pub quest_control: Option<Did>,
    /// `QuestArcDirectory`.
    pub quest_arc_directory: Option<Did>,
    /// `AccomplishmentDirectory` (the main deed directory).
    pub accomplishment_directory: Option<Did>,
    /// `AutoBestowQuestDirectory`.
    pub auto_bestow_directory: Option<Did>,
    /// `AccomplishmentVersionControl`.
    pub accomplishment_version_control: Option<Did>,
    /// `AccomplishmentControl`.
    pub accomplishment_control: Option<Did>,
    /// `QuestEventTargetLocationControl`.
    pub quest_target_index: Option<Did>,
    /// `QuestItemRegionDirectory`.
    pub quest_item_region_directory: Option<Did>,
    /// `QuestCategoryLimiterControl`.
    pub quest_category_limiter: Option<Did>,
    /// `EmoteDirectory`.
    pub emote_directory: Option<Did>,
    /// `ReputationControl`.
    pub reputation_control: Option<Did>,
    /// `WorldEventControl`.
    pub world_event_control: Option<Did>,
    /// `NPCColorControl`.
    pub colour_text_control: Option<Did>,
    /// `MonsterDivisionControl`.
    pub monster_division_control: Option<Did>,
    /// `VersionAccountTitleBestow`.
    pub version_title_bestow: Option<Did>,
}

impl QuestSingletons {
    /// Resolves every label with `lookup` (a WEENIECONTENT label → DID lookup).
    #[must_use]
    pub fn resolve(lookup: impl Fn(&str) -> Option<Did>) -> Self {
        use weenie_content_labels as l;
        let get = |label: &str| lookup(label).filter(|d| !d.is_null());
        Self {
            quest_control: get(l::QUEST_CONTROL),
            quest_arc_directory: get(l::QUEST_ARC_DIRECTORY),
            accomplishment_directory: get(l::ACCOMPLISHMENT_DIRECTORY),
            auto_bestow_directory: get(l::AUTO_BESTOW_QUEST_DIRECTORY),
            accomplishment_version_control: get(l::ACCOMPLISHMENT_VERSION_CONTROL),
            accomplishment_control: get(l::ACCOMPLISHMENT_CONTROL),
            quest_target_index: get(l::QUEST_EVENT_TARGET_LOCATION_CONTROL),
            quest_item_region_directory: get(l::QUEST_ITEM_REGION_DIRECTORY),
            quest_category_limiter: get(l::QUEST_CATEGORY_LIMITER_CONTROL),
            emote_directory: get(l::EMOTE_DIRECTORY),
            reputation_control: get(l::REPUTATION_CONTROL),
            world_event_control: get(l::WORLD_EVENT_CONTROL),
            colour_text_control: get(l::NPC_COLOUR_CONTROL),
            monster_division_control: get(l::MONSTER_DIVISION_CONTROL),
            version_title_bestow: get(l::VERSION_ACCOUNT_TITLE_BESTOW),
        }
    }

    /// Resolves every label in the WEENIECONTENT DID map (`property-system.md` §8.1).
    #[must_use]
    pub fn from_weenie_content(map: &DataIdMap) -> Self {
        Self::resolve(|label| map.by_label(label))
    }

    /// The kind §2.3 gives the record `did`, if it is one of the labelled records with a
    /// kind of [`QuestFamilyKind`] (the quest-target index, the limiter, the division
    /// control and the title grants have none).
    #[must_use]
    pub fn kind_of(&self, did: Did) -> Option<QuestFamilyKind> {
        let is = |d: Option<Did>| d == Some(did);
        if is(self.quest_control) {
            Some(QuestFamilyKind::QuestControl)
        } else if is(self.quest_arc_directory) {
            Some(QuestFamilyKind::QuestArcDirectory)
        } else if is(self.accomplishment_directory) || is(self.auto_bestow_directory) {
            Some(QuestFamilyKind::AccomplishmentDirectory)
        } else if is(self.accomplishment_version_control) {
            Some(QuestFamilyKind::AccomplishmentVersionList)
        } else if is(self.accomplishment_control) {
            Some(QuestFamilyKind::AccomplishmentControl)
        } else if is(self.quest_item_region_directory) {
            Some(QuestFamilyKind::QuestItemRegionDirectory)
        } else if is(self.emote_directory) {
            Some(QuestFamilyKind::EmoteDirectory)
        } else if is(self.reputation_control) {
            Some(QuestFamilyKind::ReputationControl)
        } else if is(self.world_event_control) {
            Some(QuestFamilyKind::WorldEventList)
        } else if is(self.colour_text_control) {
            Some(QuestFamilyKind::ColourTextControl)
        } else {
            None
        }
    }
}

impl QuestLabels {
    /// The kind of a property set by its identifying top-level properties (§2.1), or `None`
    /// when it belongs to no kind of this family.
    ///
    /// The singleton kinds of §2.3 ([`QuestFamilyKind::is_singleton`]) are never reported:
    /// §2.2 finds them by their WEENIECONTENT labels, and their identifying properties are
    /// not unique to them (other records carry `Reputation_FactionTable`, for example). Use
    /// [`QuestLabels::classify_record`] to include them. The treasure kind needs every
    /// entry and is decided by [`QuestLabels::classify_set`].
    #[must_use]
    pub fn classify<A: PropertyAccess + ?Sized>(&self, set: &A) -> Option<QuestFamilyKind> {
        QuestFamilyKind::ALL
            .into_iter()
            .filter(|&k| k != QuestFamilyKind::QuestTreasure && !k.is_singleton())
            .find(|&k| self.matches_kind(set, k))
    }

    /// [`QuestLabels::classify`] for a whole property set, including the treasure kind
    /// (a non-empty set of treasure properties only). Singleton kinds are not reported.
    #[must_use]
    pub fn classify_set(&self, set: &PropertySet) -> Option<QuestFamilyKind> {
        self.classify(set).or_else(|| {
            (!set.is_empty() && set.iter().all(|e| self.is_treasure_pid(e.pid)))
                .then_some(QuestFamilyKind::QuestTreasure)
        })
    }

    /// The kind of record `did` with property set `set`: the kind §2.3 gives a record named
    /// in the WEENIECONTENT map (`singletons`), otherwise [`QuestLabels::classify_set`]
    /// (§2.2). A labelled record whose set lacks the kind's identifying properties is
    /// still given the label's kind, with a warning.
    #[must_use]
    pub fn classify_record(
        &self,
        did: Did,
        set: &PropertySet,
        singletons: &QuestSingletons,
    ) -> Option<QuestFamilyKind> {
        if let Some(kind) = singletons.kind_of(did) {
            if !self.matches_kind(set, kind) {
                tracing::warn!(%did, ?kind, "labelled record lacks the identifying properties");
            }
            return Some(kind);
        }
        self.classify_set(set)
    }

    /// Whether `set` carries the identifying properties of `kind` (§2.1). For the treasure
    /// kind it checks only for any treasure property ([`QuestLabels::classify_set`] decides
    /// that kind); for the singleton kinds it is necessary but not sufficient (§2.2).
    #[must_use]
    pub fn matches_kind<A: PropertyAccess + ?Sized>(&self, set: &A, kind: QuestFamilyKind) -> bool {
        let q = &self.quest;
        let r = &self.reward;
        let l = &self.lore;
        let w = &self.world;
        let n = &self.npc;
        let d = &self.deed;
        match kind {
            QuestFamilyKind::Quest => {
                set.has(q.name) && set.has(q.objective_array) && set.has(q.role_array)
            }
            QuestFamilyKind::QuestTreasure => self.treasure.all().iter().any(|&p| set.has(p)),
            QuestFamilyKind::RewardLevelTable => RewardList::ALL
                .iter()
                .any(|&list| set.has(list.list_pid(r))),
            QuestFamilyKind::QuestArc => set.has(l.arc_name) && set.has(l.arc_quests),
            QuestFamilyKind::QuestArcDirectory => set.has(l.arc_directory),
            QuestFamilyKind::AccomplishmentDirectory => set.has(d.list),
            QuestFamilyKind::AccomplishmentVersionList => set.has(d.version_list),
            QuestFamilyKind::QuestItemDirectory => set.has(n.item_directory_array),
            QuestFamilyKind::QuestItemRegionDirectory => set.has(n.item_region_directory_array),
            QuestFamilyKind::Title => set.has(l.title_string) && set.has(l.title_category),
            QuestFamilyKind::Emote => set.has(l.emote_command) && set.has(l.emote_script),
            QuestFamilyKind::EmoteDirectory => set.has(l.emote_available),
            QuestFamilyKind::Faction => set.has(l.faction_name) && set.has(l.faction_advancement),
            QuestFamilyKind::ReputationAccountModifier => set.has(l.modifier_list),
            QuestFamilyKind::ReputationControl => set.has(l.faction_table),
            QuestFamilyKind::WorldEventBool => {
                set.has(w.world_property) && set.has(w.start_active) && !set.has(w.max_int)
            }
            QuestFamilyKind::WorldEventInt => set.has(w.world_property) && set.has(w.max_int),
            QuestFamilyKind::WorldEventList => {
                set.has(w.event_list) && (set.has(w.decay_epsilon) || set.has(w.schedule_epsilon))
            }
            QuestFamilyKind::NpcComment => set.has(n.comment_content) && set.has(n.comment_title),
            QuestFamilyKind::NpcCommentList => set.has(n.comment_array),
            QuestFamilyKind::ColourTextList => {
                set.has(n.colour_text_array) || set.has(n.reputation_colour_text_array)
            }
            QuestFamilyKind::Scene => set.has(n.scene_keyframes) && set.has(n.scene_name),
            QuestFamilyKind::QuestControl => set.has(r.reward_level_array),
            QuestFamilyKind::AccomplishmentControl => set.has(d.ui_tab_elements),
            QuestFamilyKind::ColourTextControl => {
                set.has(n.text_duration) && set.has(n.colour_text_interval)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Quest records (§4)
// ---------------------------------------------------------------------------------------------

/// A quest or deed record (§4): a typed view over its property set.
#[derive(Debug, Clone, Copy)]
pub struct QuestRecord<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

/// What [`QuestRecord::new`] names in its errors.
const QUEST: &str = "quest record";

impl<'a> QuestRecord<'a> {
    /// A view over `set`, which must carry the three identifying properties `Quest_Name`,
    /// `Quest_ObjectiveArray` and `Quest_RoleArray` (§2.1, V1); otherwise
    /// [`FormatError::MissingProperty`] (or [`FormatError::PropertyTypeMismatch`] when the
    /// two arrays are not arrays).
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Result<Self> {
        let q = &labels.quest;
        set.require(q.name, QUEST)?;
        set.require_array(q.objective_array, QUEST)?;
        set.require_array(q.role_array, QUEST)?;
        Ok(Self { labels, set })
    }

    /// A view over `set` without checking the identifying properties.
    #[must_use]
    pub fn new_unchecked(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The property set.
    #[must_use]
    pub fn set(&self) -> &'a PropertySet {
        self.set
    }

    /// The labels the view resolves with.
    #[must_use]
    pub fn labels(&self) -> &'a QuestLabels {
        self.labels
    }

    fn q(&self) -> &'a QuestPids {
        &self.labels.quest
    }

    /// `Quest_Name`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.q().name)
    }

    /// `Quest_Description`.
    #[must_use]
    pub fn description(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.q().description)
    }

    /// Whether the record is a deed: `Quest_IsAccomplishment` present and true (§4.1).
    #[must_use]
    pub fn is_deed(&self) -> bool {
        self.set.boolean(self.q().is_accomplishment) == Some(true)
    }

    /// The stored `Quest_IsAccomplishment`, if any (V1: never stored false).
    #[must_use]
    pub fn is_accomplishment_stored(&self) -> Option<bool> {
        self.set.boolean(self.q().is_accomplishment)
    }

    /// `Quest_Category` (journal category; no deed has one).
    #[must_use]
    pub fn category(&self) -> Option<u32> {
        code_of(self.set, self.q().category)
    }

    /// `Quest_Scope`.
    #[must_use]
    pub fn scope(&self) -> Option<u32> {
        code_of(self.set, self.q().scope)
    }

    /// The stored `Quest_ChallengeLevel`.
    #[must_use]
    pub fn challenge_level(&self) -> Option<i32> {
        self.set.int(self.q().challenge_level)
    }

    /// The PID named by `Quest_ChallengeLevelOverrideProperty` (0 = none).
    #[must_use]
    pub fn challenge_level_override(&self) -> Option<u32> {
        self.set.property_ref(self.q().challenge_level_override)
    }

    /// Whether `Quest_HideChallengeLevel` is stored and true.
    #[must_use]
    pub fn hides_challenge_level(&self) -> bool {
        self.set.boolean(self.q().hide_challenge_level) == Some(true)
    }

    /// Whether `Quest_IgnoreDefaultChallengeLevel` is stored and true (meaning OPEN, §4.3).
    #[must_use]
    pub fn ignores_default_challenge_level(&self) -> bool {
        self.set.boolean(self.q().ignore_default_challenge_level) == Some(true)
    }

    /// The level the client shows for the quest (§4.3, Confirmed): 0 when the level is
    /// hidden; else the player's value of the override property when one is named and
    /// `read_player` returns it; else the stored `Quest_ChallengeLevel`; else 0.
    ///
    /// `read_player` reads an integer property of the local player's object by PID.
    pub fn effective_challenge_level(&self, read_player: impl FnOnce(u32) -> Option<i32>) -> i32 {
        if self.hides_challenge_level() {
            return 0;
        }
        if let Some(pid) = self.challenge_level_override().filter(|&p| p != 0)
            && let Some(level) = read_player(pid)
        {
            return level;
        }
        self.challenge_level().unwrap_or(0)
    }

    /// `Quest_MaxTimesCompletable`, or its dictionary default when absent (−1 unlimited,
    /// Reported).
    #[must_use]
    pub fn max_times_completable<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> Option<i32> {
        self.set
            .value_or_default(self.q().max_times_completable, defaults)?
            .int()
    }

    /// `Quest_LockType`.
    #[must_use]
    pub fn lock_type(&self) -> Option<u32> {
        code_of(self.set, self.q().lock_type)
    }

    /// `Quest_Lock_ResetDays`: bit *i* names weekday key *i* + 1.
    #[must_use]
    pub fn lock_reset_days(&self) -> Option<u64> {
        self.set.bitfield(self.q().lock_reset_days)
    }

    /// `Quest_TimeLimit` in seconds.
    #[must_use]
    pub fn time_limit(&self) -> Option<f32> {
        self.set.float(self.q().time_limit)
    }

    /// `Quest_NextQuest` (non-null).
    #[must_use]
    pub fn next_quest(&self) -> Option<Did> {
        self.set.did_nonnull(self.q().next_quest)
    }

    /// `Quest_PrecludedQuest` (non-null).
    #[must_use]
    pub fn precluded_quest(&self) -> Option<Did> {
        self.set.did_nonnull(self.q().precluded_quest)
    }

    /// The `Quest_DataID` elements of `Quest_QuestsToComplete`.
    #[must_use]
    pub fn quests_to_complete(&self) -> Vec<Did> {
        self.set.dids(self.q().quests_to_complete).collect()
    }

    /// `WebStoreAccountItem_DataID`.
    #[must_use]
    pub fn web_store_item(&self) -> Option<Did> {
        self.set.did_nonnull(self.q().web_store_item)
    }

    /// The objectives of `Quest_ObjectiveArray`, in **stored** order (§5.1).
    #[must_use]
    pub fn objectives(&self) -> Vec<Objective<'a>> {
        let q = self.q();
        self.set
            .elements(q.objective_array)
            .iter()
            .filter_map(|e| struct_element(e, q.completion_objective))
            .map(|raw| Objective::new(self.labels, raw))
            .collect()
    }

    /// The quest-level failure conditions: the `Quest_FailureCondition` arrays of
    /// `Quest_FailureConditionArray`, in stored order (§5.1). Elements with another PID are
    /// skipped (logged), as in an objective's condition array.
    #[must_use]
    pub fn failure_conditions(&self) -> Vec<Condition<'a>> {
        let q = self.q();
        conditions_of(
            self.labels,
            self.set.elements(q.failure_condition_array),
            q.failure_condition,
        )
    }

    /// The tracked conditions: the `Quest_TrackedCondition` arrays of
    /// `Quest_TrackedConditionsArray`, in stored order (§5.1). Elements with another PID are
    /// skipped (logged).
    #[must_use]
    pub fn tracked_conditions(&self) -> Vec<Condition<'a>> {
        let q = self.q();
        conditions_of(
            self.labels,
            self.set.elements(q.tracked_conditions_array),
            q.tracked_condition,
        )
    }

    /// The flattened role list `Quest_RoleArray`, the authority for roles (§7.3).
    #[must_use]
    pub fn roles(&self) -> Vec<Role<'a>> {
        roles_of(self.labels, self.set, self.q().role_array)
    }

    /// `Quest_BestowalRoles` (source form).
    #[must_use]
    pub fn bestowal_roles(&self) -> Vec<Role<'a>> {
        roles_of(self.labels, self.set, self.q().bestowal_roles)
    }

    /// `Quest_GlobalRoles` (source form; action 5).
    #[must_use]
    pub fn global_roles(&self) -> Vec<Role<'a>> {
        roles_of(self.labels, self.set, self.q().global_roles)
    }

    /// The roles of `Quest_RoleArray` with role name `name` (and, when given, NPC `npc`):
    /// the join of the NPC-side lists and map notes (§7.4, §17).
    #[must_use]
    pub fn roles_named(&self, name: &str, npc: Option<Did>) -> Vec<Role<'a>> {
        self.roles()
            .into_iter()
            .filter(|r| r.role_name() == Some(name) && npc.is_none_or(|n| r.npc() == Some(n)))
            .collect()
    }

    /// `Quest_CompletionCommentArray` (§7.3).
    #[must_use]
    pub fn completion_comments(&self) -> Vec<CompletionComment<'a>> {
        structs_of(self.set, self.q().completion_comment_array)
            .into_iter()
            .map(|raw| CompletionComment {
                labels: self.labels,
                raw,
            })
            .collect()
    }

    /// `DefaultPermissionBlobStruct` (§8.1).
    #[must_use]
    pub fn permissions(&self) -> Option<Permissions<'a>> {
        self.set
            .structure(self.q().permissions)
            .map(|raw| Permissions::new(self.labels, raw))
    }

    /// The top-level `Usage_*` properties outside the permission struct, read with the
    /// same view (whether they are evaluated like the struct's is OPEN, §4.2).
    #[must_use]
    pub fn top_level_usage(&self) -> TopLevelUsage<'a> {
        TopLevelUsage {
            labels: self.labels,
            set: self.set,
        }
    }

    /// Deed-only fields (§10.1), when the record is a deed.
    #[must_use]
    pub fn deed(&self) -> Option<DeedInfo> {
        self.is_deed()
            .then(|| DeedInfo::read(&self.labels.deed, self.set))
    }

    /// `Quest_QuestTreasureDID`.
    #[must_use]
    pub fn treasure(&self) -> Option<Did> {
        self.set.did_nonnull(self.labels.reward.treasure)
    }

    /// `Quest_RewardLevelDID`.
    #[must_use]
    pub fn reward_level(&self) -> Option<Did> {
        self.set.did_nonnull(self.labels.reward.reward_level)
    }

    /// The quest's tier value for a reward kind stored on the quest (§9.1): the top-level
    /// tier property, or `Quest_RepTier` of `Quest_PositiveFaction` for [`RewardList::Rep`].
    /// `None` for the lists whose tier lives elsewhere (virtue experience on the treasure,
    /// paper items per award, the deed lists).
    ///
    /// A quest can carry two reputation tiers (§9.1, §9.5): this reads only the positive
    /// one. The tier of `Quest_NegativeFaction` (which subtracts, Reported) comes from
    /// [`QuestRecord::negative_tier`]; a quest with only a negative faction gives `None`
    /// here.
    #[must_use]
    pub fn tier(&self, list: RewardList) -> Option<u32> {
        let r = &self.labels.reward;
        match list {
            RewardList::Rep => self.positive_faction()?.tier,
            other => code_of(self.set, other.quest_tier_pid(r)?),
        }
    }

    /// `Quest_RepTier` of `Quest_NegativeFaction` (§9.1, §9.5): looked up in
    /// [`RewardList::Rep`] like the positive tier; the amount is subtracted (Reported).
    #[must_use]
    pub fn negative_tier(&self) -> Option<u32> {
        self.negative_faction()?.tier
    }

    /// `Quest_CraftProfessionDID` (with `Quest_CraftExpTier`).
    #[must_use]
    pub fn craft_profession(&self) -> Option<Did> {
        self.set.did_nonnull(self.labels.reward.craft_profession)
    }

    /// `Quest_PositiveFaction` (§9.5).
    #[must_use]
    pub fn positive_faction(&self) -> Option<FactionReward> {
        let r = &self.labels.reward;
        self.set
            .structure(r.positive_faction)
            .map(|s| FactionReward::read(r, s))
    }

    /// `Quest_NegativeFaction` (§9.5).
    #[must_use]
    pub fn negative_faction(&self) -> Option<FactionReward> {
        let r = &self.labels.reward;
        self.set
            .structure(r.negative_faction)
            .map(|s| FactionReward::read(r, s))
    }

    /// The fixed amounts of §9.3: experience, gold, glory and positive reputation.
    #[must_use]
    pub fn fixed_amounts(&self) -> FixedAmounts {
        let r = &self.labels.reward;
        let get = |p: Pid| self.set.get_pid(p).and_then(PropertyValue::as_f64);
        FixedAmounts {
            exp: get(r.exp_to_give),
            gold: get(r.gold_to_give),
            glory: get(r.glory_to_give),
            positive_reputation: get(r.pos_rep_to_give),
        }
    }

    /// `Quest_Medal_LootArray` (§9.6).
    #[must_use]
    pub fn medal_loot(&self) -> Vec<MedalLoot> {
        let r = &self.labels.reward;
        structs_of(self.set, r.medal_loot_array)
            .into_iter()
            .map(|s| MedalLoot {
                medal: code_of(s, r.medal),
                treasure: s.did_nonnull(r.medal_treasure),
            })
            .collect()
    }

    /// `Quest_PaperItemAwards` (§9.1, §9.7).
    #[must_use]
    pub fn paper_items(&self) -> Vec<PaperItemAward> {
        let r = &self.labels.reward;
        structs_of(self.set, r.paper_item_awards)
            .into_iter()
            .map(|s| PaperItemAward {
                item: s.did_nonnull(r.paper_item),
                tier: code_of(s, r.paper_item_tier),
            })
            .collect()
    }

    /// `Quest_GiveItemArray` at quest level (§9.7).
    #[must_use]
    pub fn give_items(&self) -> Vec<GiveItem> {
        give_items_of(&self.labels.quest, self.set)
    }

    /// `Quest_LootTable` (§9.7).
    #[must_use]
    pub fn loot_table(&self) -> Vec<LootEntry<'a>> {
        let q = self.q();
        structs_of(self.set, q.loot_table)
            .into_iter()
            .map(|raw| LootEntry {
                labels: self.labels,
                raw,
            })
            .collect()
    }

    /// `Quest_EffectArray` (§9.7).
    #[must_use]
    pub fn effects(&self) -> Vec<QuestEffect> {
        effects_of(&self.labels.quest, self.set, self.q().effect_array)
    }

    /// Numbers every event entry of the quest per §5.7 (EventID, ObjectiveID,
    /// ConditionID), collects compound members (§5.2) and required counts (§6.1).
    ///
    /// Fails with [`FormatError::MissingProperty`] when an objective has no
    /// `Quest_ObjectiveIndex` (it is always present, §5.1), since its events could not be
    /// given an ObjectiveID. [`QuestRecord::number_events_lenient`] numbers the rest of
    /// such a quest instead.
    pub fn number_events(&self) -> Result<EventNumbering<'a>> {
        self.number(false)
    }

    /// [`QuestRecord::number_events`] that does not fail (§19: a reader may warn but must
    /// not fail): an objective without `Quest_ObjectiveIndex` is left out of the walk with
    /// a warning and its stored position (0-based in `Quest_ObjectiveArray`) is listed in
    /// [`EventNumbering::unindexed_objectives`]. Its events take no EventID, so in such a
    /// quest the EventIDs after it are lower than the compiled form's would be; with every
    /// objective indexed the result equals [`QuestRecord::number_events`].
    #[must_use]
    pub fn number_events_lenient(&self) -> EventNumbering<'a> {
        self.number(true).unwrap_or_default()
    }

    fn number(&self, lenient: bool) -> Result<EventNumbering<'a>> {
        let mut n = Numberer::default();
        for (position, objective) in self.objectives().into_iter().enumerate() {
            let Some(objective_id) = objective.index() else {
                if lenient {
                    tracing::warn!(
                        position,
                        "quest objective without an objective index skipped"
                    );
                    n.out.unindexed_objectives.push(position);
                    continue;
                }
                return Err(FormatError::MissingProperty {
                    what: "quest objective",
                    label: self.q().objective_index.label(),
                });
            };
            let conditions = objective
                .completion
                .iter()
                .map(|c| (c, EventOrigin::ObjectiveCompletion))
                .chain(
                    objective
                        .failure
                        .iter()
                        .map(|c| (c, EventOrigin::ObjectiveFailure)),
                );
            for (condition_id, (c, origin)) in (1u32..).zip(conditions) {
                n.condition(objective_id, condition_id, c, origin);
            }
        }
        for (condition_id, c) in (1u32..).zip(self.failure_conditions().iter()) {
            n.condition(0, condition_id, c, EventOrigin::QuestFailure);
        }
        for (condition_id, c) in (1u32..).zip(self.tracked_conditions().iter()) {
            n.condition(0, condition_id, c, EventOrigin::Tracked);
        }
        Ok(n.finish())
    }
}

/// Top-level `Usage_*` properties of a quest record (§4.2).
#[derive(Debug, Clone, Copy)]
pub struct TopLevelUsage<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl TopLevelUsage<'_> {
    /// Top-level `Usage_MinLevel`.
    #[must_use]
    pub fn min_level(&self) -> Option<i32> {
        self.set.int(self.labels.usage.min_level)
    }

    /// Top-level `Usage_RequiredClassList` codes.
    #[must_use]
    pub fn required_classes(&self) -> Vec<u32> {
        codes_of(self.set, self.labels.usage.required_classes)
    }
}

/// Deed-only fields of a quest record (§10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeedInfo {
    /// `Accomplishment_UITab`.
    pub ui_tab: Option<u32>,
    /// `Accomplishment_Category`.
    pub category: Option<u32>,
    /// `Accomplishment_UIFilter`.
    pub ui_filter: Option<u32>,
    /// `Accomplishment_MinLevelToStart` as stored (dictionary default 1).
    pub min_level_to_start: Option<i32>,
    /// `Accomplishment_ShowProgressBar`.
    pub show_progress_bar: Option<bool>,
    /// `Accomplishment_AccelerationCategory`.
    pub acceleration_category: Option<u32>,
    /// `Accomplishment_CanBeAccelerated` as stored (dictionary default true).
    pub can_be_accelerated: Option<bool>,
    /// `Accomplishment_RequiredTrait`.
    pub required_trait: Option<Did>,
    /// `Quest_IsHidden`.
    pub is_hidden: Option<bool>,
    /// `Quest_IsSessionAccomplishment`.
    pub is_session: Option<bool>,
}

impl DeedInfo {
    fn read<A: PropertyAccess + ?Sized>(d: &DeedPids, a: &A) -> Self {
        Self {
            ui_tab: code_of(a, d.ui_tab),
            category: code_of(a, d.category),
            ui_filter: code_of(a, d.ui_filter),
            min_level_to_start: a.int(d.min_level_to_start),
            show_progress_bar: a.boolean(d.show_progress_bar),
            acceleration_category: code_of(a, d.acceleration_category),
            can_be_accelerated: a.boolean(d.can_be_accelerated),
            required_trait: a.did_nonnull(d.required_trait),
            is_hidden: a.boolean(d.is_hidden),
            is_session: a.boolean(d.is_session_accomplishment),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Objectives, conditions, events (§5)
// ---------------------------------------------------------------------------------------------

/// One `Quest_CompletionObjective` (§5.1).
#[derive(Debug, Clone)]
pub struct Objective<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
    /// Completion conditions (`Quest_CompletionCondition` elements), in stored order.
    pub completion: Vec<Condition<'a>>,
    /// Failure conditions (`Quest_FailureCondition` elements), in stored order.
    pub failure: Vec<Condition<'a>>,
}

impl<'a> Objective<'a> {
    fn new(labels: &'a QuestLabels, raw: &'a PropertyStruct) -> Self {
        let q = &labels.quest;
        let mut completion = Vec::new();
        let mut failure = Vec::new();
        for e in raw.elements(q.completion_condition_array) {
            if let Some(a) = array_element(e, q.completion_condition) {
                completion.push(Condition::new(labels, a));
            } else if let Some(a) = array_element(e, q.failure_condition) {
                failure.push(Condition::new(labels, a));
            } else {
                tracing::debug!(pid = e.pid, "unexpected element in a condition array");
            }
        }
        Self {
            labels,
            raw,
            completion,
            failure,
        }
    }

    /// The objective struct.
    #[must_use]
    pub fn raw(&self) -> &'a PropertyStruct {
        self.raw
    }

    /// `Quest_ObjectiveIndex` (1 … *n*; V2).
    #[must_use]
    pub fn index(&self) -> Option<i32> {
        self.raw.int(self.labels.quest.objective_index)
    }

    /// `Quest_ObjectiveDescription`.
    #[must_use]
    pub fn description(&self) -> Option<&'a StringInfo> {
        self.raw
            .string_info(self.labels.quest.objective_description)
    }

    /// `Quest_ObjectiveProgressOverride`.
    #[must_use]
    pub fn progress_override(&self) -> Option<&'a StringInfo> {
        self.raw
            .string_info(self.labels.quest.objective_progress_override)
    }

    /// `Quest_ObjectiveBillboardOverride`.
    #[must_use]
    pub fn billboard_override(&self) -> Option<&'a StringInfo> {
        self.raw
            .string_info(self.labels.quest.objective_billboard_override)
    }

    /// `Quest_NPCObjRoles` (source form; the enclosing objective gives their objective).
    #[must_use]
    pub fn npc_roles(&self) -> Vec<Role<'a>> {
        roles_of(self.labels, self.raw, self.labels.quest.npc_obj_roles)
    }
}

/// One condition: an array of event entries, compound events and the optional count and
/// never-finish elements (§5.1, §5.2).
#[derive(Debug, Clone)]
pub struct Condition<'a> {
    /// The condition array.
    pub raw: &'a PropertyArray,
    /// Entries and compound events, in stored order.
    pub elements: Vec<ConditionElement<'a>>,
    /// `Quest_CompletionConditionCount`: how many of the entries must be done.
    pub required_count: Option<i32>,
    /// `Quest_Condition_NeverFinish` as stored.
    pub never_finish: Option<bool>,
}

/// An element of a condition that contributes events (§5.7 step 1.2).
#[derive(Debug, Clone)]
pub enum ConditionElement<'a> {
    /// A direct `QuestEvent_Entry`.
    Entry(EventEntry<'a>),
    /// A `QuestEvent_CompoundEvent`.
    Compound(CompoundEvent<'a>),
}

impl<'a> Condition<'a> {
    fn new(labels: &'a QuestLabels, raw: &'a PropertyArray) -> Self {
        let q = &labels.quest;
        let mut elements = Vec::new();
        let mut required_count = None;
        let mut never_finish = None;
        for e in &raw.elements {
            if let Some(s) = struct_element(e, q.event_entry) {
                elements.push(ConditionElement::Entry(EventEntry::new(labels, s)));
            } else if let Some(a) = array_element(e, q.compound_event) {
                elements.push(ConditionElement::Compound(CompoundEvent::new(labels, a)));
            } else if q.completion_condition_count.is(e.pid) {
                required_count = required_count.or(e.int());
            } else if q.condition_never_finish.is(e.pid) {
                never_finish = never_finish.or(e.boolean());
            } else {
                tracing::debug!(pid = e.pid, "unexpected element in a condition");
            }
        }
        Self {
            raw,
            elements,
            required_count,
            never_finish,
        }
    }

    /// Every event entry the condition contributes, in numbering order (§5.7 step 1.2).
    #[must_use]
    pub fn entries(&self) -> Vec<&EventEntry<'a>> {
        let mut out = Vec::new();
        for el in &self.elements {
            match el {
                ConditionElement::Entry(e) => out.push(e),
                ConditionElement::Compound(c) => out.extend(c.members()),
            }
        }
        out
    }

    /// Whether the condition contributes no event (it still takes a ConditionID, §5.7).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.iter().all(|el| match el {
            ConditionElement::Entry(_) => false,
            ConditionElement::Compound(c) => c.members().next().is_none(),
        })
    }
}

/// The conditions of a quest-level condition list: the elements that are arrays with PID
/// `element` (`Quest_FailureCondition` or `Quest_TrackedCondition`, §5.1), in stored order.
fn conditions_of<'a>(
    labels: &'a QuestLabels,
    elements: &'a [PropertyEntry],
    element: Pid,
) -> Vec<Condition<'a>> {
    elements
        .iter()
        .filter_map(|e| {
            let a = array_element(e, element);
            if a.is_none() {
                tracing::debug!(
                    pid = e.pid,
                    "unexpected element in a quest-level condition list"
                );
            }
            a
        })
        .map(|a| Condition::new(labels, a))
        .collect()
}

/// A `QuestEvent_CompoundEvent` (§5.2).
#[derive(Debug, Clone)]
pub struct CompoundEvent<'a> {
    /// Direct entries and entry arrays, in stored order.
    pub elements: Vec<CompoundElement<'a>>,
    /// `QuestEvent_CompoundProgressOverride`.
    pub progress_override: Option<&'a StringInfo>,
}

/// An element of a compound event.
#[derive(Debug, Clone)]
pub enum CompoundElement<'a> {
    /// A direct `QuestEvent_Entry`.
    Entry(EventEntry<'a>),
    /// A `QuestEvent_Entry_Array` (possibly empty).
    EntryArray(Vec<EventEntry<'a>>),
}

impl<'a> CompoundEvent<'a> {
    fn new(labels: &'a QuestLabels, raw: &'a PropertyArray) -> Self {
        let q = &labels.quest;
        let mut elements = Vec::new();
        let mut progress_override = None;
        for e in &raw.elements {
            if let Some(s) = struct_element(e, q.event_entry) {
                elements.push(CompoundElement::Entry(EventEntry::new(labels, s)));
            } else if let Some(a) = array_element(e, q.event_entry_array) {
                let entries = a
                    .elements
                    .iter()
                    .filter_map(|x| struct_element(x, q.event_entry))
                    .map(|s| EventEntry::new(labels, s))
                    .collect();
                elements.push(CompoundElement::EntryArray(entries));
            } else if q.compound_progress_override.is(e.pid) {
                progress_override = progress_override.or(e.string_info());
            } else {
                tracing::debug!(pid = e.pid, "unexpected element in a compound event");
            }
        }
        Self {
            elements,
            progress_override,
        }
    }

    /// The members (§5.2): each direct entry once and every entry of each entry array, in
    /// stored order.
    pub fn members(&self) -> impl Iterator<Item = &EventEntry<'a>> {
        self.elements.iter().flat_map(|el| match el {
            CompoundElement::Entry(e) => std::slice::from_ref(e).iter(),
            CompoundElement::EntryArray(v) => v.iter(),
        })
    }
}

/// One `QuestEvent_Entry` (§5.3, §5.4): a typed view over the entry struct.
#[derive(Debug, Clone, Copy)]
pub struct EventEntry<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
}

impl<'a> EventEntry<'a> {
    /// A view over an event-entry struct.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, raw: &'a PropertyStruct) -> Self {
        Self { labels, raw }
    }

    /// The entry struct.
    #[must_use]
    pub fn raw(&self) -> &'a PropertyStruct {
        self.raw
    }

    fn e(&self) -> &'a EventPids {
        &self.labels.event
    }

    /// `QuestEvent_ID`, the event type (§5.4).
    #[must_use]
    pub fn event_type(&self) -> Option<u32> {
        code_of(self.raw, self.e().id)
    }

    /// `QuestEvent_EventOrder` (completion entries only).
    #[must_use]
    pub fn event_order(&self) -> Option<i32> {
        self.raw.int(self.e().event_order)
    }

    /// The order value of the compiled event: `QuestEvent_EventOrder`, or −1 (§5.7, §6.2).
    #[must_use]
    pub fn compiled_order(&self) -> i32 {
        self.event_order().unwrap_or(-1)
    }

    /// `QuestEvent_Number` as stored (dictionary default 1).
    #[must_use]
    pub fn number(&self) -> Option<i32> {
        self.raw.int(self.e().number)
    }

    /// `QuestEvent_ProgressOverride`.
    #[must_use]
    pub fn progress_override(&self) -> Option<&'a StringInfo> {
        self.raw.string_info(self.e().progress_override)
    }

    /// `QuestEvent_BillboardProgressOverride`.
    #[must_use]
    pub fn billboard_progress_override(&self) -> Option<&'a StringInfo> {
        self.raw.string_info(self.e().billboard_progress_override)
    }

    /// `QuestEvent_RoleConstraint`: the role key naming which world object counts.
    #[must_use]
    pub fn role_constraint(&self) -> Option<&'a str> {
        self.raw.string(self.e().role_constraint)
    }

    /// `QuestEvent_NPCTalk`.
    #[must_use]
    pub fn npc_talk(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().npc_talk)
    }

    /// `QuestEvent_ItemDID`.
    #[must_use]
    pub fn item(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().item)
    }

    /// `QuestEvent_MonsterDID`.
    #[must_use]
    pub fn monster(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().monster)
    }

    /// `QuestEvent_LandmarkDID`.
    #[must_use]
    pub fn landmark(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().landmark)
    }

    /// `QuestEvent_QuestComplete`.
    #[must_use]
    pub fn quest_complete(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().quest_complete)
    }

    /// `QuestEvent_QuestCompleteCategory`.
    #[must_use]
    pub fn quest_complete_category(&self) -> Option<u32> {
        code_of(self.raw, self.e().quest_complete_category)
    }

    /// `QuestEvent_BestowedQuestID`.
    #[must_use]
    pub fn bestowed_quest(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().bestowed_quest)
    }

    /// The world-event condition of a type-34 entry (§5.4, §14.2), when it names an event.
    #[must_use]
    pub fn world_event_condition(&self) -> Option<WorldEventCondition> {
        let e = self.e();
        let world_event = self.raw.did(e.world_event)?;
        Some(WorldEventCondition {
            world_event,
            operator: code_of(self.raw, e.world_event_operator).unwrap_or(0),
            value: self.raw.int(e.world_event_value),
            compare_with: None,
        })
    }

    /// `QuestEvent_EmoteDID`.
    #[must_use]
    pub fn emote(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().emote)
    }

    /// `QuestEvent_EmoteTargetDID`.
    #[must_use]
    pub fn emote_target(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().emote_target)
    }

    /// `QuestEvent_FactionDID`.
    #[must_use]
    pub fn faction(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().faction)
    }

    /// `QuestEvent_ReputationTier`.
    #[must_use]
    pub fn reputation_tier(&self) -> Option<u32> {
        code_of(self.raw, self.e().reputation_tier)
    }

    /// `QuestEvent_SkillDID`.
    #[must_use]
    pub fn skill(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().skill)
    }

    /// `QuestEvent_TimeLimit` in seconds (numeric types read leniently).
    #[must_use]
    pub fn time_limit(&self) -> Option<f64> {
        self.raw.get_pid(self.e().time_limit)?.as_f64()
    }

    /// `QuestEvent_PlayerLevel`.
    #[must_use]
    pub fn player_level(&self) -> Option<i32> {
        self.raw.int(self.e().player_level)
    }

    /// `QuestEvent_Points` (tracked events, §9.6).
    #[must_use]
    pub fn points(&self) -> Option<i32> {
        self.raw.int(self.e().points)
    }

    /// `QuestEvent_ScriptCallback_Key`.
    #[must_use]
    pub fn script_callback_key(&self) -> Option<u32> {
        code_of(self.raw, self.e().script_callback_key)
    }

    /// `QuestEvent_Status` (§8.3).
    #[must_use]
    pub fn status(&self) -> Option<QuestStatus> {
        code_of(self.raw, self.e().status).map(QuestStatus)
    }

    /// `QuestEvent_Session_DID`.
    #[must_use]
    pub fn session(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().session)
    }

    /// `QuestEvent_Session_Status` (§8.3).
    #[must_use]
    pub fn session_status(&self) -> Option<QuestStatus> {
        code_of(self.raw, self.e().session_status).map(QuestStatus)
    }

    /// `QuestEvent_TerritoryDID`.
    #[must_use]
    pub fn territory(&self) -> Option<Did> {
        self.raw.did_nonnull(self.e().territory)
    }

    /// `QuestEvent_DramaName`.
    #[must_use]
    pub fn drama_name(&self) -> Option<&'a str> {
        self.raw.string(self.e().drama_name)
    }

    /// `QuestEvent_WaitForDrama` as stored.
    #[must_use]
    pub fn wait_for_drama(&self) -> Option<bool> {
        self.raw.boolean(self.e().wait_for_drama)
    }

    /// Whether the drama rule of the listen lists holds: `QuestEvent_WaitForDrama` true and
    /// a `QuestEvent_DramaName` present (§6.2).
    #[must_use]
    pub fn waits_for_named_drama(&self) -> bool {
        self.wait_for_drama() == Some(true) && self.raw.has(self.e().drama_name)
    }

    /// The listen list of the compiled event (§6.2); see [`listen_list`].
    #[must_use]
    pub fn listen_list(&self) -> Option<Vec<u32>> {
        listen_list(self.event_type()?, self.waits_for_named_drama())
    }

    /// `QuestEvent_DailyMaximumIncrements` as stored (the compiled form stores 0 when
    /// absent, §6.2).
    #[must_use]
    pub fn daily_maximum_increments(&self) -> Option<i32> {
        self.raw.int(self.e().daily_maximum_increments)
    }

    /// `QuestEvent_Locations` (§5.6).
    #[must_use]
    pub fn locations(&self) -> Vec<EventLocation<'a>> {
        let e = self.e();
        self.raw
            .structs(e.locations)
            .map(|s| EventLocation {
                position: s.position(e.location_position),
                radius: s.float(e.location_radius),
                entire_landblock: s.boolean(e.location_entire_landblock),
                role_constraint: s.string(e.location_role_constraint),
            })
            .collect()
    }

    /// `QuestEvent_MonsterGenus_Array` (§5.5).
    #[must_use]
    pub fn monster_selectors(&self) -> Vec<MonsterSelector> {
        selectors_of(self.e(), self.raw, self.e().monster_selectors)
    }

    /// `QuestEvent_MonsterGenus_ExclusionArray` (§5.5).
    #[must_use]
    pub fn monster_exclusions(&self) -> Vec<MonsterSelector> {
        selectors_of(self.e(), self.raw, self.e().monster_exclusions)
    }

    /// The `Quest_Role` bound to this event (§7), unless absent.
    #[must_use]
    pub fn role(&self) -> Option<Role<'a>> {
        self.raw.structure(self.e().role).map(|raw| Role {
            labels: self.labels,
            raw,
        })
    }

    /// `Quest_GiveItemArray` of the event.
    #[must_use]
    pub fn give_items(&self) -> Vec<GiveItem> {
        give_items_of(&self.labels.quest, self.raw)
    }

    /// `QuestEvent_GiveItemsOnAdvance`.
    #[must_use]
    pub fn gives_items_on_advance(&self) -> Option<bool> {
        self.raw.boolean(self.e().give_items_on_advance)
    }

    /// `QuestEvent_ApplyEffectArray` (§9.7).
    #[must_use]
    pub fn effects(&self) -> Vec<QuestEffect> {
        effects_of(&self.labels.quest, self.raw, self.e().apply_effect_array)
    }
}

/// One `QuestEvent_Location` (§5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventLocation<'a> {
    /// `QuestEvent_Location_Position`.
    pub position: Option<&'a Position>,
    /// `QuestEvent_Location_Radius`.
    pub radius: Option<f32>,
    /// `QuestEvent_Location_EntireLandblock`.
    pub entire_landblock: Option<bool>,
    /// `QuestEvent_Location_RoleConstraint`.
    pub role_constraint: Option<&'a str>,
}

/// One `Quest_MonsterGenus_Struct` (§5.5). Each field is optional; a monster matches when
/// it matches every field present (Reported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct MonsterSelector {
    /// `Quest_MonsterGenus` (bitfield).
    pub genus: Option<u32>,
    /// `Quest_MonsterSpecies`.
    pub species: Option<u32>,
    /// `Quest_MonsterSubspecies`.
    pub subspecies: Option<u32>,
    /// `Quest_MonsterAlignment`.
    pub alignment: Option<u32>,
    /// `Quest_MonsterClass`.
    pub class: Option<u32>,
    /// `Quest_MonsterDivision`.
    pub division: Option<u32>,
    /// `Quest_MonsterRegion` (territory DID).
    pub region: Option<Did>,
    /// `QuestEvent_LandmarkDID` (meaning OPEN, §5.5).
    pub landmark: Option<Did>,
}

impl MonsterSelector {
    fn read<A: PropertyAccess + ?Sized>(e: &EventPids, a: &A) -> Self {
        Self {
            genus: code_of(a, e.selector_genus),
            species: code_of(a, e.selector_species),
            subspecies: code_of(a, e.selector_subspecies),
            alignment: code_of(a, e.selector_alignment),
            class: code_of(a, e.selector_class),
            division: code_of(a, e.selector_division),
            region: a.did(e.selector_region),
            landmark: a.did(e.landmark),
        }
    }
}

fn selectors_of<A: PropertyAccess + ?Sized>(
    e: &EventPids,
    a: &A,
    pid: Pid,
) -> Vec<MonsterSelector> {
    a.structs(pid)
        .map(|s| MonsterSelector::read(e, s))
        .collect()
}

/// The event types a compiled event reacts to (§6.2): for most types the type itself;
/// fixed lists for types 9, 10, 13, 16 and 27; (*t*, 17, 12) for types 1, 5, 11 and 24
/// when `waits_for_drama` (the entry waits for a drama it names).
///
/// `None` for types 19 and 29, whose list holds a type (1 or 11) chosen by a rule that is
/// OPEN (§6.2).
#[must_use]
pub fn listen_list(event_type: u32, waits_for_drama: bool) -> Option<Vec<u32>> {
    Some(match event_type {
        9 => vec![1, 21, 2],
        10 => vec![10, 31, 7],
        13 => vec![1, 7, 28, 15, 20],
        16 => vec![7, 16],
        27 => vec![27, 11, 22, 28, 8],
        19 | 29 => return None,
        1 | 5 | 11 | 24 if waits_for_drama => vec![event_type, 17, 12],
        t => vec![t],
    })
}

// ---------------------------------------------------------------------------------------------
// Event numbering (§5.7)
// ---------------------------------------------------------------------------------------------

/// Where a numbered event comes from (§5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOrigin {
    /// A completion condition of an objective.
    ObjectiveCompletion,
    /// A failure condition of an objective.
    ObjectiveFailure,
    /// `Quest_FailureConditionArray` (ObjectiveID 0).
    QuestFailure,
    /// `Quest_TrackedConditionsArray` (ObjectiveID 0).
    Tracked,
}

impl EventOrigin {
    /// Whether events of this origin are failure events (`QuestEvent_IsFailureEvent`, §6.2).
    #[must_use]
    pub fn is_failure(self) -> bool {
        matches!(self, Self::ObjectiveFailure | Self::QuestFailure)
    }
}

/// One event entry with the numbers the compiled form gives it (§5.7).
#[derive(Debug, Clone, Copy)]
pub struct NumberedEvent<'a> {
    /// EventID: position in the walk, from 1.
    pub event_id: u32,
    /// ObjectiveID: the objective's `Quest_ObjectiveIndex`, or 0 at quest level.
    pub objective_id: i32,
    /// ConditionID: 1-based within the objective (completion conditions, then failure
    /// conditions), or within the quest-level failure or tracked list.
    pub condition_id: u32,
    /// Where the entry sits.
    pub origin: EventOrigin,
    /// Index into [`EventNumbering::compounds`] when the entry is a compound member.
    pub compound: Option<usize>,
    /// The compiled `Quest_Condition_NeverFinish`: true for events of a condition whose own
    /// flag is true, and for every tracked event (§6.2).
    pub never_finish: bool,
    /// The entry.
    pub entry: EventEntry<'a>,
}

impl NumberedEvent<'_> {
    /// The compiled `QuestEvent_IsFailureEvent` (§6.2).
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.origin.is_failure()
    }
}

/// The key of the required-counts attribute (§5.2, §6.1): ObjectiveID × 65,536 +
/// ConditionID, computed in 32 bits.
#[must_use]
pub fn required_count_key(objective_id: i32, condition_id: u32) -> u32 {
    (objective_id as u32)
        .wrapping_mul(65_536)
        .wrapping_add(condition_id)
}

/// The numbered events of one quest (§5.7) with compound membership (§5.2) and required
/// counts (§6.1).
#[derive(Debug, Clone, Default)]
pub struct EventNumbering<'a> {
    /// Events in EventID order (EventID *i* at index *i* − 1).
    pub events: Vec<NumberedEvent<'a>>,
    /// The EventIDs of each compound event's members, ascending.
    pub compounds: Vec<Vec<u32>>,
    /// (required-count key, `Quest_CompletionConditionCount`) per condition with a count, in
    /// walk order.
    pub required_counts: Vec<(u32, i32)>,
    /// Stored positions (0-based) of the objectives that [`QuestRecord::number_events_lenient`]
    /// left out because they have no `Quest_ObjectiveIndex`; always empty from
    /// [`QuestRecord::number_events`].
    pub unindexed_objectives: Vec<usize>,
}

impl<'a> EventNumbering<'a> {
    /// Number of events (*N*).
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the quest has no event.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The event with EventID `event_id`.
    #[must_use]
    pub fn get(&self, event_id: u32) -> Option<&NumberedEvent<'a>> {
        let i = usize::try_from(event_id.checked_sub(1)?).ok()?;
        self.events.get(i)
    }

    /// The sibling list of an event (§6.2): the EventIDs of the other members of its
    /// compound event, ascending; empty outside a compound (V5).
    #[must_use]
    pub fn siblings(&self, event_id: u32) -> Vec<u32> {
        self.get(event_id)
            .and_then(|e| e.compound)
            .and_then(|c| self.compounds.get(c))
            .map(|members| members.iter().copied().filter(|&m| m != event_id).collect())
            .unwrap_or_default()
    }

    /// The objective index of §6.1: ObjectiveID → EventIDs, ascending.
    #[must_use]
    pub fn objective_index(&self) -> BTreeMap<i32, Vec<u32>> {
        let mut out: BTreeMap<i32, Vec<u32>> = BTreeMap::new();
        for e in &self.events {
            out.entry(e.objective_id).or_default().push(e.event_id);
        }
        out
    }
}

#[derive(Default)]
struct Numberer<'a> {
    out: EventNumbering<'a>,
}

impl<'a> Numberer<'a> {
    fn condition(
        &mut self,
        objective_id: i32,
        condition_id: u32,
        c: &Condition<'a>,
        origin: EventOrigin,
    ) {
        let never_finish = origin == EventOrigin::Tracked || c.never_finish == Some(true);
        if let Some(count) = c.required_count {
            self.out
                .required_counts
                .push((required_count_key(objective_id, condition_id), count));
        }
        for el in &c.elements {
            match el {
                ConditionElement::Entry(entry) => {
                    self.push(
                        *entry,
                        objective_id,
                        condition_id,
                        origin,
                        never_finish,
                        None,
                    );
                }
                ConditionElement::Compound(compound) => {
                    let index = self.out.compounds.len();
                    let mut members = Vec::new();
                    for entry in compound.members() {
                        let id = self.push(
                            *entry,
                            objective_id,
                            condition_id,
                            origin,
                            never_finish,
                            Some(index),
                        );
                        members.push(id);
                    }
                    self.out.compounds.push(members);
                }
            }
        }
    }

    fn push(
        &mut self,
        entry: EventEntry<'a>,
        objective_id: i32,
        condition_id: u32,
        origin: EventOrigin,
        never_finish: bool,
        compound: Option<usize>,
    ) -> u32 {
        let event_id = u32::try_from(self.out.events.len())
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        self.out.events.push(NumberedEvent {
            event_id,
            objective_id,
            condition_id,
            origin,
            compound,
            never_finish,
            entry,
        });
        event_id
    }

    fn finish(self) -> EventNumbering<'a> {
        self.out
    }
}

// ---------------------------------------------------------------------------------------------
// Roles and dialogue (§7)
// ---------------------------------------------------------------------------------------------

/// `QuestDispenser_Action` values (§7.2). Labels come from the mapper at run time; these
/// are the meanings the spec gives the observed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleAction {
    /// 1: a comment line during an objective (no advance).
    Comment,
    /// 2: talk to an NPC; advances the objective.
    Talk,
    /// 3: teleport.
    Teleport,
    /// 4: talk through an item.
    TalkThroughItem,
    /// 5: leave an instance (belongs to the quest as a whole).
    LeaveInstance,
    /// 6: bestow (offer) the quest.
    Bestow,
    /// 7: detection.
    Detection,
    /// 9: session.
    Session,
    /// 10: wait for a resource set.
    WaitForResourceSet,
    /// 13: open an instance-selection UI.
    OpenInstanceUi,
    /// 14: mission teleport.
    MissionTeleport,
    /// Any other value.
    Other(u32),
}

impl RoleAction {
    /// The action of a stored value.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Comment,
            2 => Self::Talk,
            3 => Self::Teleport,
            4 => Self::TalkThroughItem,
            5 => Self::LeaveInstance,
            6 => Self::Bestow,
            7 => Self::Detection,
            9 => Self::Session,
            10 => Self::WaitForResourceSet,
            13 => Self::OpenInstanceUi,
            14 => Self::MissionTeleport,
            other => Self::Other(other),
        }
    }

    /// The stored value.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::Comment => 1,
            Self::Talk => 2,
            Self::Teleport => 3,
            Self::TalkThroughItem => 4,
            Self::LeaveInstance => 5,
            Self::Bestow => 6,
            Self::Detection => 7,
            Self::Session => 9,
            Self::WaitForResourceSet => 10,
            Self::OpenInstanceUi => 13,
            Self::MissionTeleport => 14,
            Self::Other(c) => c,
        }
    }
}

/// A `Quest_Role` struct (§7.1).
#[derive(Debug, Clone, Copy)]
pub struct Role<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
}

impl<'a> Role<'a> {
    /// A view over a role struct.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, raw: &'a PropertyStruct) -> Self {
        Self { labels, raw }
    }

    /// The role struct.
    #[must_use]
    pub fn raw(&self) -> &'a PropertyStruct {
        self.raw
    }

    fn r(&self) -> &'a RolePids {
        &self.labels.role
    }

    /// Whether the struct has no field (166 such structs sit in event entries, §7.1).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.fields.is_empty()
    }

    /// `QuestDispenser_Action` as stored.
    #[must_use]
    pub fn action_code(&self) -> Option<u32> {
        code_of(self.raw, self.r().action)
    }

    /// `QuestDispenser_Action` (§7.2).
    #[must_use]
    pub fn action(&self) -> Option<RoleAction> {
        self.action_code().map(RoleAction::from_code)
    }

    /// `QuestDispenser_NPC` (non-null).
    #[must_use]
    pub fn npc(&self) -> Option<Did> {
        self.raw.did_nonnull(self.r().npc)
    }

    /// `QuestDispenser_RoleName`: the role key inside the quest.
    #[must_use]
    pub fn role_name(&self) -> Option<&'a str> {
        self.raw.string(self.r().role_name)
    }

    /// `QuestDispenser_RoleConstraint`.
    #[must_use]
    pub fn role_constraint(&self) -> Option<&'a str> {
        self.raw.string(self.r().role_constraint)
    }

    /// `Quest_ObjectiveIndex` as stored. The property has **no dictionary default**:
    /// `None` means "not bound to an objective" and must stay distinct from 0, the
    /// bestowal stage (§7.1).
    #[must_use]
    pub fn objective_index(&self) -> Option<i32> {
        self.raw.int(self.labels.quest.objective_index)
    }

    /// Whether the role satisfies the invariant of §7.2 (V6) for the roles of
    /// `Quest_RoleArray`: action 6 ⇔ objective stored as 0; action 5 ⇔ objective absent;
    /// any other action ⇔ a stored objective ≥ 1.
    #[must_use]
    pub fn objective_rule_holds(&self) -> bool {
        match (self.action(), self.objective_index()) {
            (Some(RoleAction::Bestow), Some(0)) | (Some(RoleAction::LeaveInstance), None) => true,
            (Some(RoleAction::Bestow | RoleAction::LeaveInstance), _) => false,
            (_, Some(i)) => i >= 1,
            (_, None) => false,
        }
    }

    /// `QuestDispenser_RoleSuccessText`: the dialogue line.
    #[must_use]
    pub fn success_text(&self) -> Option<&'a StringInfo> {
        self.raw.string_info(self.r().success_text)
    }

    /// `QuestDispenser_RoleFailureText`.
    #[must_use]
    pub fn failure_text(&self) -> Option<&'a StringInfo> {
        self.raw.string_info(self.r().failure_text)
    }

    /// `QuestDispenser_TextArray`: variant lines per player species (choice OPEN, §7.1).
    #[must_use]
    pub fn text_variants(&self) -> Vec<TextVariant<'a>> {
        let r = self.r();
        self.raw
            .structs(r.text_array)
            .map(|s| TextVariant {
                species: code_of(s, r.species),
                success_text: s.string_info(r.success_text),
                failure_text: s.string_info(r.failure_text),
            })
            .collect()
    }

    /// `QuestDispenser_VOSoundID` (a sound-info DID).
    #[must_use]
    pub fn vo_sound(&self) -> Option<Did> {
        self.raw.did_nonnull(self.r().vo_sound)
    }

    /// `QuestDispenser_VOScriptID`.
    #[must_use]
    pub fn vo_script(&self) -> Option<u32> {
        code_of(self.raw, self.r().vo_script)
    }

    /// `QuestDispenser_StartScriptID`.
    #[must_use]
    pub fn start_script(&self) -> Option<u32> {
        code_of(self.raw, self.r().start_script)
    }

    /// `QuestDispenser_IsRemote`.
    #[must_use]
    pub fn is_remote(&self) -> Option<bool> {
        self.raw.boolean(self.r().is_remote)
    }

    /// `QuestDispenser_DetectRadius` as stored (dictionary default 5.0).
    #[must_use]
    pub fn detect_radius(&self) -> Option<f32> {
        self.raw.float(self.r().detect_radius)
    }

    /// `QuestDispenser_PrivateEncounterTemplate`.
    #[must_use]
    pub fn private_encounter_template(&self) -> Option<Did> {
        self.raw.did_nonnull(self.r().private_encounter_template)
    }

    /// `QuestDispenser_SessionDID`.
    #[must_use]
    pub fn session(&self) -> Option<Did> {
        self.raw.did_nonnull(self.r().session)
    }

    /// `QuestDispenser_TeleportationSceneID`.
    #[must_use]
    pub fn teleportation_scene(&self) -> Option<Did> {
        self.raw.did_nonnull(self.r().teleportation_scene)
    }
}

fn roles_of<'a, A: PropertyAccess + ?Sized>(
    labels: &'a QuestLabels,
    a: &'a A,
    pid: Pid,
) -> Vec<Role<'a>> {
    a.structs(pid).map(|raw| Role { labels, raw }).collect()
}

/// One `QuestDispenser_TextStruct` (§7.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextVariant<'a> {
    /// `QuestDispenser_Species`.
    pub species: Option<u32>,
    /// `QuestDispenser_RoleSuccessText`.
    pub success_text: Option<&'a StringInfo>,
    /// `QuestDispenser_RoleFailureText`.
    pub failure_text: Option<&'a StringInfo>,
}

/// One `Quest_CompletionCommentStruct` (§7.3): lines the listed NPCs say once the quest is
/// done (Reported).
#[derive(Debug, Clone, Copy)]
pub struct CompletionComment<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
}

impl<'a> CompletionComment<'a> {
    /// The NPC DIDs of `QuestDispenser_NPCArray`.
    #[must_use]
    pub fn npcs(&self) -> Vec<Did> {
        self.raw.dids(self.labels.role.npc_array).collect()
    }

    /// The role keys (`QuestDispenser_RoleConstraint` strings) some structs list instead of
    /// NPCs, from either the NPC array or the struct itself.
    #[must_use]
    pub fn role_constraints(&self) -> Vec<&'a str> {
        let r = &self.labels.role;
        let mut out: Vec<&str> = self
            .raw
            .elements(r.npc_array)
            .iter()
            .filter(|e| r.role_constraint.is(e.pid))
            .filter_map(ValueExt::string)
            .collect();
        out.extend(self.raw.string(r.role_constraint));
        out
    }

    /// The lines of `QuestDispenser_TextArray` (`QuestDispenser_RoleSuccessText` values).
    #[must_use]
    pub fn lines(&self) -> Vec<&'a StringInfo> {
        texts_of(self.raw, self.labels.role.text_array)
    }
}

/// One entry of an NPC-side role list (§7.4): a quest and the role name it plays there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuestRoleRef<'a> {
    /// `QuestDispenser_Quest`.
    pub quest: Option<Did>,
    /// `QuestDispenser_RoleName`.
    pub role_name: Option<&'a str>,
}

/// The `QuestDispenser_QuestRole` entries of a set (NPC, monster, item or map-note info,
/// §7.4, §17): the elements of `QuestDispenser_ActiveQuests`, then a top-level
/// `QuestDispenser_QuestRole` struct if one is stored.
#[must_use]
pub fn active_quest_roles<'a, A: PropertyAccess + ?Sized>(
    labels: &QuestLabels,
    set: &'a A,
) -> Vec<QuestRoleRef<'a>> {
    let r = &labels.role;
    let read = |s: &'a PropertyStruct| QuestRoleRef {
        quest: s.did(r.quest),
        role_name: s.string(r.role_name),
    };
    let mut out: Vec<QuestRoleRef<'a>> = set.structs(r.active_quests).map(read).collect();
    out.extend(set.structure(r.quest_role).map(read));
    out
}

// ---------------------------------------------------------------------------------------------
// Prerequisites (§8)
// ---------------------------------------------------------------------------------------------

/// A 32-bit quest status code (§8.3): the raw status in bits 28–31, an objective number in
/// bits 0–27.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuestStatus(pub u32);

/// The raw status of a [`QuestStatus`] (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusKind {
    /// 0x1: underway (with an objective number, 0 = any).
    Underway,
    /// 0x2: failed.
    Failed,
    /// 0x3: completed.
    Completed,
    /// 0x4: abandoned.
    Abandoned,
    /// 0x5: tapped.
    Tapped,
    /// Any other top nibble.
    Other(u32),
}

impl QuestStatus {
    /// Mask of the raw status.
    pub const STATUS_MASK: u32 = 0xF000_0000;
    /// Mask of the objective number.
    pub const OBJECTIVE_MASK: u32 = 0x0FFF_FFFF;
    /// Failed.
    pub const FAILED: Self = Self(0x2000_0000);
    /// Completed.
    pub const COMPLETED: Self = Self(0x3000_0000);
    /// Abandoned.
    pub const ABANDONED: Self = Self(0x4000_0000);
    /// Tapped.
    pub const TAPPED: Self = Self(0x5000_0000);

    /// Underway at `objective` (0 = any); the objective is masked to 28 bits.
    #[must_use]
    pub fn underway(objective: u32) -> Self {
        Self(0x1000_0000 | (objective & Self::OBJECTIVE_MASK))
    }

    /// The raw status: code AND 0xF0000000.
    #[must_use]
    pub fn raw_status(self) -> u32 {
        self.0 & Self::STATUS_MASK
    }

    /// The objective number: code AND 0x0FFFFFFF.
    #[must_use]
    pub fn objective(self) -> u32 {
        self.0 & Self::OBJECTIVE_MASK
    }

    /// The raw status as a kind.
    #[must_use]
    pub fn kind(self) -> StatusKind {
        match self.raw_status() >> 28 {
            1 => StatusKind::Underway,
            2 => StatusKind::Failed,
            3 => StatusKind::Completed,
            4 => StatusKind::Abandoned,
            5 => StatusKind::Tapped,
            other => StatusKind::Other(other),
        }
    }
}

/// Comparison operators of mapper "Util" (§8.2), used by quest requirements and world-event
/// conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    /// 1: ≥.
    GreaterOrEqual,
    /// 2: ≠.
    NotEqual,
    /// 3: =.
    Equal,
    /// 4: <.
    Less,
    /// 5: ≤.
    LessOrEqual,
    /// 6: >.
    Greater,
}

impl Operator {
    /// The operator of a stored value; `None` for 0 (undefined) and unknown values.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::GreaterOrEqual,
            2 => Self::NotEqual,
            3 => Self::Equal,
            4 => Self::Less,
            5 => Self::LessOrEqual,
            6 => Self::Greater,
            _ => return None,
        })
    }

    /// `lhs` *op* `rhs` on integers. How quest statuses are compared is OPEN (§8.2);
    /// this is plain integer comparison.
    #[must_use]
    pub fn compare(self, lhs: i64, rhs: i64) -> bool {
        match self {
            Self::GreaterOrEqual => lhs >= rhs,
            Self::NotEqual => lhs != rhs,
            Self::Equal => lhs == rhs,
            Self::Less => lhs < rhs,
            Self::LessOrEqual => lhs <= rhs,
            Self::Greater => lhs > rhs,
        }
    }
}

/// One `Usage_QuestRequirement` (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuestRequirement {
    /// `Usage_Operator` as stored (0 = undefined).
    pub operator: u32,
    /// `Usage_QuestID` (null DIDs kept).
    pub quest: Option<Did>,
    /// `Usage_QuestStatus`.
    pub status: Option<QuestStatus>,
}

impl QuestRequirement {
    /// Whether the entry is an empty slot (operator 0 or quest DID 0), which a reader skips
    /// (§7.7).
    #[must_use]
    pub fn is_empty_slot(&self) -> bool {
        self.operator == 0 || self.quest.is_none_or(Did::is_null)
    }

    /// The operator, if defined.
    #[must_use]
    pub fn operator(&self) -> Option<Operator> {
        Operator::from_code(self.operator)
    }
}

/// One `WorldEvent_Condition` (§14.2), or the three type-34 fields of an event entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldEventCondition {
    /// `WorldEvent_WorldEvent` (null kept).
    pub world_event: Did,
    /// `WorldEvent_Operator` as stored (0 = undefined).
    pub operator: u32,
    /// `WorldEvent_ConditionValue` (0 or 1 for a boolean event).
    pub value: Option<i32>,
    /// `WorldEvent_Condition_WorldEventToCompareWith`: compare with this integer event
    /// instead of the constant.
    pub compare_with: Option<Did>,
}

impl WorldEventCondition {
    fn read<A: PropertyAccess + ?Sized>(w: &WorldEventPids, a: &A) -> Self {
        Self {
            world_event: a.did(w.world_event).unwrap_or(Did(0)),
            operator: code_of(a, w.operator).unwrap_or(0),
            value: a.int(w.condition_value),
            compare_with: a.did_nonnull(w.compare_with),
        }
    }

    /// Whether the entry is an empty slot (operator 0 or world event 0, §7.7).
    #[must_use]
    pub fn is_empty_slot(&self) -> bool {
        self.operator == 0 || self.world_event.is_null()
    }
}

fn world_conditions_of<A: PropertyAccess + ?Sized>(
    w: &WorldEventPids,
    a: &A,
    pid: Pid,
) -> Vec<WorldEventCondition> {
    a.structs(pid)
        .map(|s| WorldEventCondition::read(w, s))
        .collect()
}

/// A `DefaultPermissionBlobStruct` (§8.1) read for the fields this family uses.
#[derive(Debug, Clone, Copy)]
pub struct Permissions<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
}

impl<'a> Permissions<'a> {
    /// A view over a permission struct.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, raw: &'a PropertyStruct) -> Self {
        Self { labels, raw }
    }

    /// The struct.
    #[must_use]
    pub fn raw(&self) -> &'a PropertyStruct {
        self.raw
    }

    fn u(&self) -> &'a UsagePids {
        &self.labels.usage
    }

    /// `Usage_MinLevel` as stored (dictionary default 1; −1 = no bound).
    #[must_use]
    pub fn min_level(&self) -> Option<i32> {
        self.raw.int(self.u().min_level)
    }

    /// `Usage_MaxLevel` as stored (−1 = no bound).
    #[must_use]
    pub fn max_level(&self) -> Option<i32> {
        self.raw.int(self.u().max_level)
    }

    fn requirements_of(&self, pid: Pid) -> Vec<QuestRequirement> {
        let u = self.u();
        self.raw
            .structs(pid)
            .map(|s| QuestRequirement {
                operator: code_of(s, u.operator).unwrap_or(0),
                quest: s.did(u.quest_id),
                status: code_of(s, u.quest_status).map(QuestStatus),
            })
            .collect()
    }

    /// `Usage_QuestRequirements` (all must hold, Reported), empty slots included.
    #[must_use]
    pub fn requirements(&self) -> Vec<QuestRequirement> {
        self.requirements_of(self.u().quest_requirements)
    }

    /// `Usage_QuestRequirements_OrList` (one must hold, Reported), empty slots included.
    #[must_use]
    pub fn or_requirements(&self) -> Vec<QuestRequirement> {
        self.requirements_of(self.u().quest_requirements_or)
    }

    /// `Usage_WorldEvent_AllConditionList`.
    #[must_use]
    pub fn world_event_all(&self) -> Vec<WorldEventCondition> {
        world_conditions_of(&self.labels.world, self.raw, self.u().world_event_all)
    }

    /// `Usage_WorldEvent_AnyConditionList`.
    #[must_use]
    pub fn world_event_any(&self) -> Vec<WorldEventCondition> {
        world_conditions_of(&self.labels.world, self.raw, self.u().world_event_any)
    }

    /// `Usage_RequiredClassList` codes (0 = empty slot).
    #[must_use]
    pub fn required_classes(&self) -> Vec<u32> {
        codes_of(self.raw, self.u().required_classes)
    }

    /// `Usage_RequiredRaces` codes (0 = empty slot).
    #[must_use]
    pub fn required_races(&self) -> Vec<u32> {
        codes_of(self.raw, self.u().required_races)
    }

    /// `Usage_RequiredFaction`: (faction DID, tier).
    #[must_use]
    pub fn required_faction(&self) -> Option<(Option<Did>, Option<u32>)> {
        let u = self.u();
        let s = self.raw.structure(u.required_faction)?;
        Some((
            s.did_nonnull(u.required_faction_did),
            code_of(s, u.required_faction_tier),
        ))
    }
}

// ---------------------------------------------------------------------------------------------
// Rewards (§9)
// ---------------------------------------------------------------------------------------------

/// The lists of a reward-level record (§9.1, §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RewardList {
    /// `QuestReward_ExpEntryList` (tier `Quest_ExpTier`).
    Exp,
    /// `QuestReward_MoneyEntryList` (tier `Quest_GoldTier`).
    Money,
    /// `QuestReward_ItemExpEntryList` (tier `Quest_ItemExpTier`).
    ItemExp,
    /// `QuestReward_MountExpEntryList` (tier `Quest_MountExpTier`).
    MountExp,
    /// `QuestReward_CraftExpEntryList` (tier `Quest_CraftExpTier`).
    CraftExp,
    /// `QuestReward_GloryEntryList` (tier `Quest_GloryTier`).
    Glory,
    /// The store-point list (tier: the store-point tier property).
    StorePoints,
    /// The premium-coin list (tier: the premium-coin tier property).
    PremiumCoins,
    /// `QuestReward_SessionPointsEntryList` (tier `Quest_SessionPointsTier`).
    SessionPoints,
    /// `QuestReward_SessionFailurePointsEntryList` (tier `Quest_SessionPointsFailureTier`,
    /// key `QuestReward_FailureTier`).
    SessionFailurePoints,
    /// `QuestReward_RepEntryList` (tier `Quest_RepTier` of the faction struct).
    Rep,
    /// `QuestReward_VirtueExpEntryList` (tier `QuestTreasure_Virtue_XP_Tier`).
    VirtueExp,
    /// `QuestReward_PaperItemsEntryList` (tier `Quest_PaperItemTier`).
    PaperItems,
    /// `QuestReward_Accomplishment_ExpEntryList` (selection OPEN, §9.1).
    DeedExp,
    /// `QuestReward_Accomplishment_MoneyEntryList` (selection OPEN).
    DeedMoney,
    /// `QuestReward_Accomplishment_RepEntryList` (selection OPEN).
    DeedRep,
}

impl RewardList {
    /// Every list.
    pub const ALL: [Self; 16] = [
        Self::Exp,
        Self::Money,
        Self::ItemExp,
        Self::MountExp,
        Self::CraftExp,
        Self::Glory,
        Self::StorePoints,
        Self::PremiumCoins,
        Self::SessionPoints,
        Self::SessionFailurePoints,
        Self::Rep,
        Self::VirtueExp,
        Self::PaperItems,
        Self::DeedExp,
        Self::DeedMoney,
        Self::DeedRep,
    ];

    /// The list property.
    #[must_use]
    pub fn list_pid(self, r: &RewardPids) -> Pid {
        match self {
            Self::Exp => r.exp_list,
            Self::Money => r.money_list,
            Self::ItemExp => r.item_exp_list,
            Self::MountExp => r.mount_exp_list,
            Self::CraftExp => r.craft_exp_list,
            Self::Glory => r.glory_list,
            Self::StorePoints => r.store_points_list,
            Self::PremiumCoins => r.premium_coins_list,
            Self::SessionPoints => r.session_points_list,
            Self::SessionFailurePoints => r.session_failure_points_list,
            Self::Rep => r.rep_list,
            Self::VirtueExp => r.virtue_exp_list,
            Self::PaperItems => r.paper_items_list,
            Self::DeedExp => r.deed_exp_list,
            Self::DeedMoney => r.deed_money_list,
            Self::DeedRep => r.deed_rep_list,
        }
    }

    /// The amount property of the list's elements; `None` for the deed lists, whose
    /// amount property the spec does not name.
    #[must_use]
    pub fn amount_pid(self, r: &RewardPids) -> Option<Pid> {
        Some(match self {
            Self::Exp | Self::ItemExp | Self::MountExp | Self::CraftExp | Self::VirtueExp => {
                r.exp_amount
            }
            Self::Money => r.money_amount,
            Self::Glory => r.glory_amount,
            Self::StorePoints => r.store_points_amount,
            Self::PremiumCoins => r.premium_coins_amount,
            Self::SessionPoints => r.session_points_amount,
            Self::SessionFailurePoints => r.session_failure_points_amount,
            Self::Rep => r.rep_amount,
            Self::PaperItems => r.paper_items_amount,
            Self::DeedExp | Self::DeedMoney | Self::DeedRep => return None,
        })
    }

    /// The tier key of the list's elements: `QuestReward_FailureTier` for the session
    /// failure list, `QuestReward_Tier` otherwise.
    #[must_use]
    pub fn tier_key_pid(self, r: &RewardPids) -> Pid {
        match self {
            Self::SessionFailurePoints => r.failure_tier,
            _ => r.tier,
        }
    }

    /// The top-level tier property of a quest that selects from this list, for the lists
    /// whose tier is stored at the top level of the quest.
    #[must_use]
    pub fn quest_tier_pid(self, r: &RewardPids) -> Option<Pid> {
        Some(match self {
            Self::Exp => r.exp_tier,
            Self::Money => r.gold_tier,
            Self::ItemExp => r.item_exp_tier,
            Self::MountExp => r.mount_exp_tier,
            Self::CraftExp => r.craft_exp_tier,
            Self::Glory => r.glory_tier,
            Self::StorePoints => r.store_points_tier,
            Self::PremiumCoins => r.premium_coins_tier,
            Self::SessionPoints => r.session_points_tier,
            Self::SessionFailurePoints => r.session_points_failure_tier,
            _ => return None,
        })
    }
}

/// One element of a reward list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RewardTierAmount {
    /// The tier key.
    pub tier: Option<u32>,
    /// The amount (numeric types widened to f64).
    pub amount: Option<f64>,
}

/// A reward-level record (§9.2): a typed view over its property set.
#[derive(Debug, Clone, Copy)]
pub struct RewardLevelTable<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> RewardLevelTable<'a> {
    /// A view over a reward-level record's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The element structs of `list`, in stored order.
    #[must_use]
    pub fn elements(&self, list: RewardList) -> Vec<&'a PropertyStruct> {
        structs_of(self.set, list.list_pid(&self.labels.reward))
    }

    /// The element of `list` whose tier key equals `tier`.
    #[must_use]
    pub fn entry(&self, list: RewardList, tier: u32) -> Option<&'a PropertyStruct> {
        let key = list.tier_key_pid(&self.labels.reward);
        self.elements(list)
            .into_iter()
            .find(|s| code_of(*s, key) == Some(tier))
    }

    /// (tier, amount) for every element of `list`.
    #[must_use]
    pub fn entries(&self, list: RewardList) -> Vec<RewardTierAmount> {
        let r = &self.labels.reward;
        let key = list.tier_key_pid(r);
        let amount = list.amount_pid(r);
        self.elements(list)
            .into_iter()
            .map(|s| RewardTierAmount {
                tier: code_of(s, key),
                amount: amount.and_then(|p| s.get_pid(p)?.as_f64()),
            })
            .collect()
    }

    /// The amount of `list` for `tier` (§9.1): tier 0 means none; a tier without an
    /// element gives no reward.
    #[must_use]
    pub fn amount(&self, list: RewardList, tier: u32) -> Option<f64> {
        if tier == 0 {
            return None;
        }
        let pid = list.amount_pid(&self.labels.reward)?;
        self.entry(list, tier)?.get_pid(pid)?.as_f64()
    }
}

/// Chooses the reward-level table of a quest (§9.2).
///
/// 1. With `Quest_RewardLevelDID` (`quest_table`), that record when `is_table` accepts it,
///    else no table (two quests name a quest record).
/// 2. Otherwise (Reported) element *level* (1-based) of `QuestControl_RewardLevelDIDArray`
///    (`control`), and the last element when *level* is 0. `level` is the quest's level
///    (§4.3). Levels outside 0 … `control.len()` give `None` (the rule for levels above the
///    table is OPEN, §9.2), as does a null element.
#[must_use]
pub fn choose_reward_table(
    quest_table: Option<Did>,
    is_table: impl FnOnce(Did) -> bool,
    control: &[Did],
    level: i32,
) -> Option<Did> {
    if let Some(did) = quest_table.filter(|d| !d.is_null()) {
        return is_table(did).then_some(did);
    }
    let did = if level == 0 {
        *control.last()?
    } else {
        let i = usize::try_from(level).ok()?.checked_sub(1)?;
        *control.get(i)?
    };
    (!did.is_null()).then_some(did)
}

/// `Quest_PositiveFaction` / `Quest_NegativeFaction` (§9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactionReward {
    /// `Quest_FactionDID`.
    pub faction: Option<Did>,
    /// `Quest_RepTier`.
    pub tier: Option<u32>,
}

impl FactionReward {
    fn read(r: &RewardPids, s: &PropertyStruct) -> Self {
        Self {
            faction: s.did_nonnull(r.faction),
            tier: code_of(s, r.rep_tier),
        }
    }
}

/// Fixed reward amounts of a quest (§9.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FixedAmounts {
    /// `Quest_ExpToGive`.
    pub exp: Option<f64>,
    /// `Quest_GoldToGive` (unit OPEN).
    pub gold: Option<f64>,
    /// `Quest_GloryToGive`.
    pub glory: Option<f64>,
    /// `Quest_PosRepToGive`.
    pub positive_reputation: Option<f64>,
}

/// One `Quest_Medal_LootArray` entry (§9.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MedalLoot {
    /// `Quest_Medal_LootArray_Medal`.
    pub medal: Option<u32>,
    /// `Quest_Medal_LootArray_TreasureDID`.
    pub treasure: Option<Did>,
}

/// One `Quest_PaperItemAwards` entry (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaperItemAward {
    /// `Quest_PaperItemDID`.
    pub item: Option<Did>,
    /// `Quest_PaperItemTier`.
    pub tier: Option<u32>,
}

/// One `Quest_ItemStruct` of a `Quest_GiveItemArray` (§9.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GiveItem {
    /// `Quest_ItemDID`.
    pub item: Option<Did>,
    /// `Quest_ItemQuantity`, 1 when absent (§9.7).
    pub quantity: i32,
}

fn give_items_of<A: PropertyAccess + ?Sized>(q: &QuestPids, a: &A) -> Vec<GiveItem> {
    a.structs(q.give_item_array)
        .map(|s| GiveItem {
            item: s.did_nonnull(q.item_did),
            quantity: s.int(q.item_quantity).unwrap_or(1),
        })
        .collect()
}

/// One `Quest_Effect` (§9.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestEffect {
    /// `Quest_DataID` (the effect).
    pub effect: Option<Did>,
    /// `QuestEffect_ApplyRole` (2 completion, 4 bestowal, 7 failure here).
    pub apply_role: Option<u32>,
    /// `QuestEffect_Spellcraft`.
    pub spellcraft: Option<f64>,
}

fn effects_of<A: PropertyAccess + ?Sized>(q: &QuestPids, a: &A, pid: Pid) -> Vec<QuestEffect> {
    a.structs(pid)
        .map(|s| QuestEffect {
            effect: s.did_nonnull(q.data_id),
            apply_role: code_of(s, q.effect_apply_role),
            spellcraft: s
                .get_pid(q.effect_spellcraft)
                .and_then(PropertyValue::as_f64),
        })
        .collect()
}

/// One `Quest_LootItemStruct` (§9.7).
#[derive(Debug, Clone, Copy)]
pub struct LootEntry<'a> {
    labels: &'a QuestLabels,
    raw: &'a PropertyStruct,
}

impl<'a> LootEntry<'a> {
    /// The struct.
    #[must_use]
    pub fn raw(&self) -> &'a PropertyStruct {
        self.raw
    }

    /// `Quest_LootItemDID`.
    #[must_use]
    pub fn item(&self) -> Option<Did> {
        self.raw.did_nonnull(self.labels.quest.loot_item)
    }

    /// `Quest_LootItemProbability`.
    #[must_use]
    pub fn probability(&self) -> Option<f64> {
        self.raw
            .get_pid(self.labels.quest.loot_probability)?
            .as_f64()
    }

    /// `Quest_LootItemProbabilityArray`.
    #[must_use]
    pub fn probabilities(&self) -> Vec<f64> {
        self.raw
            .elements(self.labels.quest.loot_probability_array)
            .iter()
            .filter_map(|e| e.value.as_f64())
            .collect()
    }

    /// `Quest_LootMonsterDID`.
    #[must_use]
    pub fn monster(&self) -> Option<Did> {
        self.raw.did_nonnull(self.labels.quest.loot_monster)
    }

    /// `Quest_LootMonsterGenus_Array` (§5.5).
    #[must_use]
    pub fn monster_selectors(&self) -> Vec<MonsterSelector> {
        selectors_of(
            &self.labels.event,
            self.raw,
            self.labels.quest.loot_monster_selectors,
        )
    }

    /// `Quest_LootMonsterGenus_ExclusionArray` (§5.5).
    #[must_use]
    pub fn monster_exclusions(&self) -> Vec<MonsterSelector> {
        selectors_of(
            &self.labels.event,
            self.raw,
            self.labels.quest.loot_monster_exclusions,
        )
    }
}

/// One `QuestTreasure_ItemStruct` (§9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreasureItem {
    /// `QuestTreasure_Item`.
    pub item: Option<Did>,
    /// `QuestTreasure_ItemQuantity` (always stored, V9).
    pub quantity: Option<i32>,
}

/// One `QuestTreasure_Virtue_Struct` (§9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreasureVirtue {
    /// `QuestTreasure_Virtue`: PID of the virtue's rank property.
    pub virtue: Option<u32>,
    /// `QuestTreasure_Virtue_Increment`.
    pub increment: Option<i32>,
}

/// A quest-treasure record (§9.4).
#[derive(Debug, Clone, Copy)]
pub struct QuestTreasure<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> QuestTreasure<'a> {
    /// A view over a treasure record's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    fn t(&self) -> &'a TreasurePids {
        &self.labels.treasure
    }

    fn items(&self, pid: Pid) -> Vec<TreasureItem> {
        let t = self.t();
        self.set
            .structs(pid)
            .map(|s| TreasureItem {
                item: s.did_nonnull(t.item),
                quantity: s.int(t.item_quantity),
            })
            .collect()
    }

    /// `QuestTreasure_FixedItemArray`: items always given.
    #[must_use]
    pub fn fixed_items(&self) -> Vec<TreasureItem> {
        self.items(self.t().fixed_items)
    }

    /// `QuestTreasure_SelectableItemArray`: items of which the player chooses one.
    #[must_use]
    pub fn selectable_items(&self) -> Vec<TreasureItem> {
        self.items(self.t().selectable_items)
    }

    /// `QuestTreasure_FixedTitleArray`.
    #[must_use]
    pub fn fixed_titles(&self) -> Vec<Did> {
        self.set.dids(self.t().fixed_titles).collect()
    }

    /// `QuestTreasure_SelectableTitleArray`.
    #[must_use]
    pub fn selectable_titles(&self) -> Vec<Did> {
        self.set.dids(self.t().selectable_titles).collect()
    }

    /// `QuestTreasure_FixedEmoteArray`.
    #[must_use]
    pub fn fixed_emotes(&self) -> Vec<Did> {
        self.set.dids(self.t().fixed_emotes).collect()
    }

    /// `QuestTreasure_SelectableEmoteArray`.
    #[must_use]
    pub fn selectable_emotes(&self) -> Vec<Did> {
        self.set.dids(self.t().selectable_emotes).collect()
    }

    /// `QuestTreasure_FixedTraitArray`.
    #[must_use]
    pub fn fixed_traits(&self) -> Vec<Did> {
        self.set.dids(self.t().fixed_traits).collect()
    }

    /// `QuestTreasure_FixedVirtueArray`.
    #[must_use]
    pub fn fixed_virtues(&self) -> Vec<TreasureVirtue> {
        let t = self.t();
        self.set
            .structs(t.fixed_virtues)
            .map(|s| TreasureVirtue {
                virtue: s.property_ref(t.virtue),
                increment: s.int(t.virtue_increment),
            })
            .collect()
    }

    /// `QuestTreasure_Virtue_XP_Tier` (list [`RewardList::VirtueExp`]).
    #[must_use]
    pub fn virtue_xp_tier(&self) -> Option<u32> {
        code_of(self.set, self.t().virtue_xp_tier)
    }

    /// `QuestTreasure_FixedBillingTokenArray` codes.
    #[must_use]
    pub fn billing_tokens(&self) -> Vec<u32> {
        codes_of(self.set, self.t().fixed_billing_tokens)
    }
}

/// The quest control record (`QuestControl`, §9.2, §9.6, §9.8).
#[derive(Debug, Clone, Copy)]
pub struct QuestControl<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> QuestControl<'a> {
    /// A view over the quest control's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The property set.
    #[must_use]
    pub fn set(&self) -> &'a PropertySet {
        self.set
    }

    /// `QuestControl_RewardLevelDIDArray`: element *i* (1-based) is the table for level
    /// *i*. Null elements are kept so that positions stay levels.
    #[must_use]
    pub fn reward_level_tables(&self) -> Vec<Did> {
        raw_dids_of(self.set, self.labels.reward.reward_level_array)
    }

    /// `QuestControl_MedalThresholds`: (`Quest_Medal`, `Quest_Points`) pairs.
    #[must_use]
    pub fn medal_thresholds(&self) -> Vec<(Option<u32>, Option<i32>)> {
        let r = &self.labels.reward;
        self.set
            .structs(r.medal_thresholds)
            .map(|s| (code_of(s, r.threshold_medal), s.int(r.threshold_points)))
            .collect()
    }

    /// `Quest_DefaultBestowScriptID`.
    #[must_use]
    pub fn default_bestow_script(&self) -> Option<u32> {
        code_of(self.set, self.labels.reward.default_bestow_script)
    }

    /// `Quest_DefaultCommentVOScriptID`.
    #[must_use]
    pub fn default_comment_vo_script(&self) -> Option<u32> {
        code_of(self.set, self.labels.reward.default_comment_vo_script)
    }
}

// ---------------------------------------------------------------------------------------------
// Deed directories (§10.2, §10.3)
// ---------------------------------------------------------------------------------------------

/// One element of an `Accomplishment_List` (§10.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DirectoryEntry {
    /// `Accomplishment_File`: one quest DID.
    File(Did),
    /// `Accomplishment_Set`: the `Accomplishment_File` DIDs of a set.
    Set(Vec<Did>),
}

impl DirectoryEntry {
    /// The DIDs the entry names.
    #[must_use]
    pub fn dids(&self) -> &[Did] {
        match self {
            Self::File(d) => std::slice::from_ref(d),
            Self::Set(v) => v,
        }
    }
}

fn directory_entries<A: PropertyAccess + ?Sized>(d: &DeedPids, a: &A) -> Vec<DirectoryEntry> {
    let mut out = Vec::new();
    for e in a.elements(d.list) {
        if d.file.is(e.pid) {
            if let Some(did) = e.did_ref() {
                out.push(DirectoryEntry::File(did));
            }
        } else if let Some(set) = array_element(e, d.set) {
            out.push(DirectoryEntry::Set(
                set.elements
                    .iter()
                    .filter(|x| d.file.is(x.pid))
                    .filter_map(ValueExt::did_ref)
                    .collect(),
            ));
        }
    }
    out
}

/// An accomplishment directory (§10.2).
#[derive(Debug, Clone, Copy)]
pub struct AccomplishmentDirectory<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> AccomplishmentDirectory<'a> {
    /// A view over a directory's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The elements of `Accomplishment_List`, in stored order.
    #[must_use]
    pub fn entries(&self) -> Vec<DirectoryEntry> {
        directory_entries(&self.labels.deed, self.set)
    }

    /// Every DID the list names, in stored order (sets flattened).
    #[must_use]
    pub fn dids(&self) -> Vec<Did> {
        self.entries()
            .iter()
            .flat_map(|e| e.dids().to_vec())
            .collect()
    }
}

/// One `Accomplishment_Version_Entry` (§10.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionEntry {
    /// `Accomplishment_Version_Number`.
    pub number: Option<u32>,
    /// Its `Accomplishment_List`.
    pub entries: Vec<DirectoryEntry>,
}

/// The accomplishment version list (§10.3).
#[derive(Debug, Clone, Copy)]
pub struct AccomplishmentVersionList<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> AccomplishmentVersionList<'a> {
    /// A view over the version list's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The version entries, in stored order.
    #[must_use]
    pub fn versions(&self) -> Vec<VersionEntry> {
        let d = &self.labels.deed;
        self.set
            .structs(d.version_list)
            .map(|s| VersionEntry {
                number: code_of(s, d.version_number),
                entries: directory_entries(d, s),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Arcs, titles, emotes, reputation (§11–§13)
// ---------------------------------------------------------------------------------------------

/// A quest arc (§11).
#[derive(Debug, Clone, Copy)]
pub struct QuestArc<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> QuestArc<'a> {
    /// A view over an arc's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// `QuestArc_Name`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.labels.lore.arc_name)
    }

    /// The quests of `QuestArc_Quest_Array`, in stored order.
    #[must_use]
    pub fn quests(&self) -> Vec<Did> {
        self.set.dids(self.labels.lore.arc_quests).collect()
    }
}

/// The quest-arc directory (§11).
#[derive(Debug, Clone, Copy)]
pub struct QuestArcDirectory<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> QuestArcDirectory<'a> {
    /// A view over the arc directory's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// The arcs of `QuestArcDirectory_QuestArc_Array`.
    #[must_use]
    pub fn arcs(&self) -> Vec<Did> {
        self.set.dids(self.labels.lore.arc_directory).collect()
    }
}

/// A title record (§12.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Title<'a> {
    /// `Title_String`.
    pub string: Option<&'a StringInfo>,
    /// `Title_Description`.
    pub description: Option<&'a StringInfo>,
    /// `Title_Category`.
    pub category: Option<u32>,
    /// `Title_Exclusion_Group` (0 = none).
    pub exclusion_group: Option<u32>,
    /// `Title_Priority`.
    pub priority: Option<i32>,
    /// `Title_Icon`.
    pub icon: Option<Did>,
}

impl<'a> Title<'a> {
    /// Reads a title record's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let l = &labels.lore;
        Self {
            string: set.string_info(l.title_string),
            description: set.string_info(l.title_description),
            category: code_of(set, l.title_category),
            exclusion_group: code_of(set, l.title_exclusion_group),
            priority: set.int(l.title_priority),
            icon: set.did_nonnull(l.title_icon),
        }
    }
}

/// An emote record (§12.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emote<'a> {
    /// `Emote_CommandString`.
    pub command: Option<&'a StringInfo>,
    /// `Emote_Description`.
    pub description: Option<&'a StringInfo>,
    /// `Emote_ScriptID`.
    pub script: Option<u32>,
    /// `Emote_Mounted_ScriptID` (0 = none).
    pub mounted_script: Option<u32>,
    /// `Emote_IconImage`.
    pub icon: Option<Did>,
    /// `Emote_AllowedWhileMounted`.
    pub allowed_while_mounted: Option<bool>,
    /// `Emote_MustBeGranted`.
    pub must_be_granted: Option<bool>,
}

impl<'a> Emote<'a> {
    /// Reads an emote record's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let l = &labels.lore;
        Self {
            command: set.string_info(l.emote_command),
            description: set.string_info(l.emote_description),
            script: code_of(set, l.emote_script),
            mounted_script: code_of(set, l.emote_mounted_script),
            icon: set.did_nonnull(l.emote_icon),
            allowed_while_mounted: set.boolean(l.emote_allowed_while_mounted),
            must_be_granted: set.boolean(l.emote_must_be_granted),
        }
    }
}

/// The emote directory (§12.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmoteDirectory {
    /// The `Emote_ID` elements of `Emote_AvailableList`.
    pub emotes: Vec<Did>,
    /// `Emote_AFK_Emote`.
    pub afk_emote: Option<Did>,
}

impl EmoteDirectory {
    /// Reads the emote directory's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        let l = &labels.lore;
        Self {
            emotes: set.dids(l.emote_available).collect(),
            afk_emote: set.did_nonnull(l.emote_afk),
        }
    }
}

/// A faction record (§13.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Faction<'a> {
    /// `Reputation_Faction_Name`.
    pub name: Option<&'a StringInfo>,
    /// `Reputation_Faction_Description`.
    pub description: Option<&'a StringInfo>,
    /// `Reputation_Faction_AdvancementTable`.
    pub advancement_table: Option<Did>,
    /// `Reputation_Faction_TierNameProgression`.
    pub tier_names: Option<Did>,
    /// `Reputation_Faction_CurrentTier_PropertyName` (PID).
    pub current_tier_property: Option<u32>,
    /// `Reputation_Faction_EarnedReputation_PropertyName` (PID).
    pub earned_property: Option<u32>,
    /// `Reputation_Faction_GlobalCap_PropertyName` (PID).
    pub global_cap_property: Option<u32>,
    /// `Reputation_LowestTier`.
    pub lowest_tier: Option<u32>,
    /// `Reputation_HighestTier`.
    pub highest_tier: Option<u32>,
    /// `Reputation_Faction_DefaultTier`.
    pub default_tier: Option<u32>,
    /// `Reputation_AccountModifier`.
    pub account_modifier: Option<Did>,
}

impl<'a> Faction<'a> {
    /// Reads a faction record's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let l = &labels.lore;
        Self {
            name: set.string_info(l.faction_name),
            description: set.string_info(l.faction_description),
            advancement_table: set.did_nonnull(l.faction_advancement),
            tier_names: set.did_nonnull(l.faction_tier_names),
            current_tier_property: set.property_ref(l.faction_current_tier_property),
            earned_property: set.property_ref(l.faction_earned_property),
            global_cap_property: set.property_ref(l.faction_global_cap_property),
            lowest_tier: code_of(set, l.lowest_tier),
            highest_tier: code_of(set, l.highest_tier),
            default_tier: code_of(set, l.default_tier),
            account_modifier: set.did_nonnull(l.account_modifier),
        }
    }
}

/// A reputation account modifier (§13.2).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountModifier<'a> {
    /// `AccountModifier_Name`.
    pub name: Option<&'a StringInfo>,
    /// `Reputation_AccountModifier_HighWater_PropertyName` (PID).
    pub high_water_property: Option<u32>,
    /// (`Reputation_AccountModifier_Tier`, `Reputation_AccountModifier_FloatMod`) pairs.
    pub modifiers: Vec<(Option<u32>, Option<f32>)>,
}

impl<'a> AccountModifier<'a> {
    /// Reads a modifier record's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let l = &labels.lore;
        Self {
            name: set.string_info(l.modifier_name),
            high_water_property: set.property_ref(l.modifier_high_water),
            modifiers: set
                .structs(l.modifier_list)
                .map(|s| (code_of(s, l.modifier_tier), s.float(l.modifier_float)))
                .collect(),
        }
    }
}

/// The reputation control (§13.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationControl {
    /// `Reputation_FactionTable`.
    pub factions: Vec<Did>,
}

impl ReputationControl {
    /// Reads the reputation control's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        Self {
            factions: set.dids(labels.lore.faction_table).collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// World events (§14)
// ---------------------------------------------------------------------------------------------

/// A world event (§14.1).
#[derive(Debug, Clone, Copy)]
pub struct WorldEvent<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> WorldEvent<'a> {
    /// A view over a world event's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    fn w(&self) -> &'a WorldEventPids {
        &self.labels.world
    }

    /// Whether the event is an integer event (it has `WorldEvent_MaxIntValue`, §2.1).
    #[must_use]
    pub fn is_integer(&self) -> bool {
        self.set.has(self.w().max_int)
    }

    /// `WorldEvent_WorldPropertyName`: the PID of the world property holding the state.
    #[must_use]
    pub fn world_property(&self) -> Option<u32> {
        self.set.property_ref(self.w().world_property)
    }

    /// `WorldEvent_StartActive`.
    #[must_use]
    pub fn start_active(&self) -> Option<bool> {
        self.set.boolean(self.w().start_active)
    }

    /// `WorldEvent_DefaultBoolValue`.
    #[must_use]
    pub fn default_bool(&self) -> Option<bool> {
        self.set.boolean(self.w().default_bool)
    }

    /// `WorldEvent_DefaultIntValue`.
    #[must_use]
    pub fn default_int(&self) -> Option<i32> {
        self.set.int(self.w().default_int)
    }

    /// (`WorldEvent_MinIntValue`, `WorldEvent_MaxIntValue`).
    #[must_use]
    pub fn int_range(&self) -> (Option<i32>, Option<i32>) {
        (
            self.set.int(self.w().min_int),
            self.set.int(self.w().max_int),
        )
    }

    /// `WorldEvent_DescriptionString`.
    #[must_use]
    pub fn description(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.w().description)
    }

    /// `WorldEvent_AllConditionList`.
    #[must_use]
    pub fn all_conditions(&self) -> Vec<WorldEventCondition> {
        world_conditions_of(self.w(), self.set, self.w().all_conditions)
    }

    /// `WorldEvent_AnyConditionList`.
    #[must_use]
    pub fn any_conditions(&self) -> Vec<WorldEventCondition> {
        world_conditions_of(self.w(), self.set, self.w().any_conditions)
    }

    /// `WorldEvent_CountConditionList`.
    #[must_use]
    pub fn count_conditions(&self) -> Vec<WorldEventCondition> {
        world_conditions_of(self.w(), self.set, self.w().count_conditions)
    }
}

/// A world-event list (§14.3).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldEventList {
    /// The DIDs of `WorldEvent_WorldEventList`, in stored order.
    pub events: Vec<Did>,
    /// `WorldEvent_Control_DecayEpsilon` (unit OPEN).
    pub decay_epsilon: Option<f64>,
    /// `WorldEvent_Control_ScheduleEpsilon` (unit OPEN).
    pub schedule_epsilon: Option<f64>,
}

impl WorldEventList {
    /// Reads a world-event list's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        let w = &labels.world;
        Self {
            events: set.dids(w.event_list).collect(),
            decay_epsilon: set.get_pid(w.decay_epsilon).and_then(PropertyValue::as_f64),
            schedule_epsilon: set
                .get_pid(w.schedule_epsilon)
                .and_then(PropertyValue::as_f64),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// NPC comments, colour text, scenes (§7.7–§7.9)
// ---------------------------------------------------------------------------------------------

/// An NPC comment record (§7.7).
#[derive(Debug, Clone, Copy)]
pub struct NpcComment<'a> {
    labels: &'a QuestLabels,
    set: &'a PropertySet,
}

impl<'a> NpcComment<'a> {
    /// A view over a comment record's property set.
    #[must_use]
    pub fn new(labels: &'a QuestLabels, set: &'a PropertySet) -> Self {
        Self { labels, set }
    }

    /// `NPCComment_Title`.
    #[must_use]
    pub fn title(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.labels.npc.comment_title)
    }

    /// `NPCComment_WindowLabel`.
    #[must_use]
    pub fn window_label(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.labels.npc.comment_window_label)
    }

    /// `NPCComment_Content`.
    #[must_use]
    pub fn content(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.labels.npc.comment_content)
    }

    /// `NPCComment_CloseButtonText`.
    #[must_use]
    pub fn close_button(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.labels.npc.comment_close_button)
    }

    /// `NPCComment_VOScriptID`.
    #[must_use]
    pub fn vo_script(&self) -> Option<u32> {
        code_of(self.set, self.labels.npc.comment_vo_script)
    }

    /// `DefaultPermissionBlobStruct` (fixed shape with empty slots, §7.7).
    #[must_use]
    pub fn permissions(&self) -> Option<Permissions<'a>> {
        self.set
            .structure(self.labels.quest.permissions)
            .map(|raw| Permissions::new(self.labels, raw))
    }
}

/// An NPC comment list (§7.7): the comment DIDs of `NPCComment_CommentArray`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcCommentList {
    /// Comment records, in stored order.
    pub comments: Vec<Did>,
}

impl NpcCommentList {
    /// Reads a comment list's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        Self {
            comments: set.dids(labels.npc.comment_array).collect(),
        }
    }
}

/// One `NPC_ReputationColorTextArray` element (§7.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReputationLine<'a> {
    /// `NPC_ColorText`.
    pub text: Option<&'a StringInfo>,
    /// `NPC_ColorText_MinReputationTier` (0 = no bound).
    pub min_tier: Option<u32>,
    /// `NPC_ColorText_MaxReputationTier` (0 = no bound).
    pub max_tier: Option<u32>,
}

/// A colour-text list (§7.8): ambient lines ("barks").
#[derive(Debug, Clone, PartialEq)]
pub struct ColourTextList<'a> {
    /// `NPC_ColorTextArray` lines.
    pub lines: Vec<&'a StringInfo>,
    /// `NPC_ReputationColorTextArray` lines.
    pub reputation_lines: Vec<ReputationLine<'a>>,
}

impl<'a> ColourTextList<'a> {
    /// Reads a colour-text list's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let n = &labels.npc;
        Self {
            lines: texts_of(set, n.colour_text_array),
            reputation_lines: set
                .structs(n.reputation_colour_text_array)
                .map(|s| ReputationLine {
                    text: s.string_info(n.colour_text),
                    min_tier: code_of(s, n.colour_text_min_tier),
                    max_tier: code_of(s, n.colour_text_max_tier),
                })
                .collect(),
        }
    }
}

/// The NPC colour-text control (§7.8).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColourTextControl {
    /// `Quest_ColorTextDetectionRadius`.
    pub detection_radius: Option<f64>,
    /// `NPC_TextDuration` (seconds).
    pub text_duration: Option<f64>,
    /// `Quest_ColorTextInterval` (unit OPEN).
    pub interval: Option<f64>,
}

impl ColourTextControl {
    /// Reads the colour-text control's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        let n = &labels.npc;
        let get = |p: Pid| set.get_pid(p).and_then(PropertyValue::as_f64);
        Self {
            detection_radius: get(n.colour_text_radius),
            text_duration: get(n.text_duration),
            interval: get(n.colour_text_interval),
        }
    }
}

/// One `Keyframe_Struct` of a scene (§7.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<'a> {
    /// `Keyframe_Subtitle`.
    pub subtitle: Option<&'a StringInfo>,
    /// `Keyframe_VideoTrack` (a surface DID).
    pub video_track: Option<Did>,
    /// `Keyframe_MovieTrack` (a movie-library record).
    pub movie_track: Option<Did>,
    /// `Keyframe_Duration`.
    pub duration: Option<f32>,
    /// `Keyframe_ShowRandomImageFromUIL`.
    pub show_random_image: Option<bool>,
}

/// A scene record (§7.9).
#[derive(Debug, Clone, PartialEq)]
pub struct Scene<'a> {
    /// `Scene_Name`.
    pub name: Option<&'a StringInfo>,
    /// `Scene_KeyframeArray`, in stored order.
    pub keyframes: Vec<Keyframe<'a>>,
}

impl<'a> Scene<'a> {
    /// Reads a scene record's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &'a PropertySet) -> Self {
        let n = &labels.npc;
        Self {
            name: set.string_info(n.scene_name),
            keyframes: set
                .structs(n.scene_keyframes)
                .map(|s| Keyframe {
                    subtitle: s.string_info(n.keyframe_subtitle),
                    video_track: s.did_nonnull(n.keyframe_video_track),
                    movie_track: s.did_nonnull(n.keyframe_movie_track),
                    duration: s.float(n.keyframe_duration),
                    show_random_image: s.boolean(n.keyframe_random_image),
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Quest-item directories (§15)
// ---------------------------------------------------------------------------------------------

/// The quest-item region directory (§15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemRegionDirectory {
    /// (`Quest_MonsterRegion` territory, `Quest_QuestItemDirectory`) pairs.
    pub entries: Vec<(Option<Did>, Option<Did>)>,
}

impl QuestItemRegionDirectory {
    /// Reads the region directory's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        let n = &labels.npc;
        Self {
            entries: set
                .structs(n.item_region_directory_array)
                .map(|s| {
                    (
                        s.did_nonnull(labels.event.selector_region),
                        s.did_nonnull(n.item_directory),
                    )
                })
                .collect(),
        }
    }
}

/// One `Quest_QuestItemDirectory_Entry` (§15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuestItemDrop {
    /// `Quest_ItemDID`.
    pub item: Option<Did>,
    /// `Agent_Species` (0 = any).
    pub species: Option<u32>,
    /// `Agent_Subspecies` (0 = any).
    pub subspecies: Option<u32>,
    /// `Quest_MonsterDivision`.
    pub division: Option<u32>,
    /// `Quest_DropFrequency`.
    pub drop_frequency: Option<u32>,
}

/// A quest-item directory (§15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDirectory {
    /// Its entries, in stored order.
    pub entries: Vec<QuestItemDrop>,
}

impl QuestItemDirectory {
    /// Reads a quest-item directory's property set.
    #[must_use]
    pub fn read(labels: &QuestLabels, set: &PropertySet) -> Self {
        let n = &labels.npc;
        Self {
            entries: set
                .structs(n.item_directory_array)
                .map(|s| QuestItemDrop {
                    item: s.did_nonnull(labels.quest.item_did),
                    species: code_of(s, n.agent_species),
                    subspecies: code_of(s, n.agent_subspecies),
                    division: code_of(s, labels.event.selector_division),
                    drop_frequency: code_of(s, n.drop_frequency),
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Quest map notes (§17)
// ---------------------------------------------------------------------------------------------

/// DID of the quest map-note record (§17.1).
pub const QUEST_MAP_NOTES_DID: Did = Did(0x0E40_0000);

/// Smallest possible note: a position flag byte, three DIDs, a null layers PID and an
/// empty info set (TSize hint and count).
const MIN_NOTE_LEN: usize = 1 + 12 + 4 + 2;

/// Name used in errors.
const MAP_NOTES: &str = "quest map notes";

/// One map note (§17.1).
#[derive(Debug, Clone, PartialEq)]
pub struct MapNote {
    /// Where the note sits (always inside a cell here).
    pub position: Position,
    /// Area or dungeon record, or the null DID.
    pub area: Did,
    /// The dungeon whose map shows the note.
    pub dungeon: Did,
    /// The noted object.
    pub target: Did,
    /// The content-layer property in the list form (PID and value); `None` for a PID of 0.
    pub layers: Option<PropertyEntry>,
    /// The embedded info set (`QuestDispenser_RoleConstraint`, `QuestDispenser_ActiveQuests`).
    pub info: PropertySet,
}

impl MapNote {
    /// The content-layer codes of the layers property, when it is an array (an empty
    /// vector for no layers or a null PID).
    #[must_use]
    pub fn content_layers(&self) -> Vec<u32> {
        self.layers
            .as_ref()
            .and_then(|e| e.value.as_array())
            .map(|a| a.elements.iter().filter_map(|e| code(&e.value)).collect())
            .unwrap_or_default()
    }

    /// The info's `QuestDispenser_RoleConstraint`.
    #[must_use]
    pub fn role_constraint<'a>(&'a self, labels: &QuestLabels) -> Option<&'a str> {
        self.info.string(labels.role.role_constraint)
    }

    /// The info's quest roles (`QuestDispenser_ActiveQuests`, §7.4).
    #[must_use]
    pub fn quest_roles<'a>(&'a self, labels: &QuestLabels) -> Vec<QuestRoleRef<'a>> {
        active_quest_roles(labels, &self.info)
    }
}

/// The quest map-note record (§17).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuestMapNotes {
    /// The record's DID.
    pub did: Did,
    /// The notes, in stored order.
    pub notes: Vec<MapNote>,
}

impl QuestMapNotes {
    /// Decodes a complete payload; it must end exactly after the last note (V10).
    ///
    /// Property types come from `schema` (the dictionary). Fails with
    /// [`FormatError::Truncated`] or [`FormatError::MemoryLimit`] when the note count
    /// cannot fit the payload, with [`FormatError::UnknownProperty`] on a PID the schema
    /// does not define, with the property-set errors of the info set (wrapped in
    /// [`FormatError::InPropertySet`]), and with [`FormatError::TrailingBytes`].
    pub fn parse<S: PropertySchema + ?Sized>(payload: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(payload);
        let did = r.did()?;
        let count = r.u32()?;
        let mut notes = r.alloc_vec(count, MIN_NOTE_LEN, usize::MAX)?;
        let mut decoder = ValueDecoder::new(schema);
        for _ in 0..count {
            let position = Position::read(&mut r)?;
            let area = r.did()?;
            let dungeon = r.did()?;
            let target = r.did()?;
            let layers = decoder.entry(&mut r, false, 0)?;
            let info_at = r.position();
            let info =
                PropertySet::read(&mut r, schema).map_err(|e| FormatError::InPropertySet {
                    offset: info_at,
                    source: Box::new(e),
                })?;
            let note = MapNote {
                position,
                area,
                dungeon,
                target,
                layers,
                info,
            };
            r.push(&mut notes, note, count)?;
        }
        r.expect_end(MAP_NOTES)?;
        r.log_unknown_hints(MAP_NOTES);
        Ok(Self { did, notes })
    }

    /// [`QuestMapNotes::parse`], then checks the record's DID
    /// ([`FormatError::DidMismatch`]).
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        payload: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let notes = Self::parse(payload, schema)?;
        if notes.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: notes.did.0,
            });
        }
        Ok(notes)
    }

    /// The notes shown on `dungeon`'s map (§17.2), before content-layer filtering.
    pub fn for_dungeon(&self, dungeon: Did) -> impl Iterator<Item = &MapNote> {
        self.notes.iter().filter(move |n| n.dungeon == dungeon)
    }

    /// The distinct dungeons that have notes.
    #[must_use]
    pub fn dungeons(&self) -> BTreeSet<Did> {
        self.notes.iter().map(|n| n.dungeon).collect()
    }
}
