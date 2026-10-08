// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced and group content (`docs/specs/instances.md`): typed views of the property
//! sets (prefix 0x79) of private-encounter templates, their control record and entry
//! filters, skirmish match parameters, trainer trait lists and control record, the
//! world-join control and tickets, big-battle control, distant-battle scripts and units,
//! the formation control, session play, the fellowship maker, battle statistics and the
//! phasing flags.
//!
//! - [`InstanceVocab`] holds the PIDs of every label the views read, looked up by name at
//!   run time (§1: never hard-code a PID). [`labels`] lists the labels with the value type
//!   the spec gives for each.
//! - [`classify`] tells a record's kind from its characteristic property (§3.4);
//!   [`InstanceRecord::read`] builds the matching view; [`decode_instance_set`] parses a
//!   0x79 payload first.
//! - [`template`]: templates (§4), the landblock-list rule (§4.3), template kinds (§4.1)
//!   and the record-local invariants N-I3 to N-I5.
//! - [`control`]: the private-encounter control and entry filters (§5), match parameters
//!   (§6.1), trait lists (§6.2), world-join control and tickets (§7), session play (§9),
//!   fellowship maker (§10.1), battle statistics (§8.7) and phasing flags (§11).
//! - [`skirmish`]: the skirmish control (§6.3) and the statistics panel rules (§15.1 B4).
//! - [`battle`]: big-battle control (§8.1), distant-battle scripts (§8.3), units (§8.4,
//!   §15.1 B5) and the formation control (§8.5).
//! - [`testkit`]: a synthetic vocabulary and schema for tests and fuzzing.
//! - [`finder`]: the finder order (§13.3), featured instance (B1), level-scaling level
//!   (B2), instance groups (B3) and the fellowship-maker matching index (§10.2).
//!
//! The views never fail: a property that is absent or stored with an unexpected type reads
//! as absent, and an array element of the wrong kind is skipped. Every collection a view
//! builds is a copy of part of an already decoded (and budget-checked) property set, so a
//! view never allocates more than its input holds. The object-state forms of §13 are not
//! read here (they belong to `tessera-wstate`).

pub mod battle;
pub mod control;
pub mod finder;
pub mod skirmish;
pub mod template;
pub mod testkit;

use std::collections::{BTreeSet, HashMap};

use crate::did::Did;
use crate::error::Result;
use crate::property::{
    PropertyDictionary, PropertyEntry, PropertyNames, PropertySchema, PropertyStruct, PropertyType,
    PropertyValue,
};
use crate::property_set::{PropertySet, PropertySetRecord};
use crate::string_info::StringInfo;

pub use battle::{
    BigBattleControl, DistantBattleScript, DistantBattleUnit, FigureCounts, FormationControl,
};
pub use control::{
    BattleStat, EncounterControl, EntryFilter, FellowshipMakerControl, MatchParams, PhasingFlags,
    SessionPlayControl, SkillTrainerTraits, WorldJoinControl, WorldJoinTicket,
};
pub use finder::{MatchingIndex, MatchingRules, MatchingTemplate};
pub use skirmish::{DerivedStat, SkirmishControl, StatValue};
pub use template::{ContainedLandblock, Template, TemplateKind};

macro_rules! define_labels {
    ($($id:ident = $label:literal : $ty:expr;)*) => {
        $(
            #[doc = concat!("`", $label, "`")]
            pub const $id: &str = $label;
        )*
        /// Every label the views read, with the value type `instances.md` gives for it
        /// (`None` where the spec states no type; such fields are read leniently).
        pub const ALL: &[(&str, Option<PropertyType>)] = &[$(($label, $ty)),*];
    };
}

/// Property labels read by the views (`instances.md` §4–§12). They are interoperability
/// names: the dictionary is looked up by them.
pub mod labels {
    use crate::property::PropertyType;

    const S: Option<PropertyType> = Some(PropertyType::String);
    const V: Option<PropertyType> = Some(PropertyType::Vector3);
    const E: Option<PropertyType> = Some(PropertyType::Enum);
    const F: Option<PropertyType> = Some(PropertyType::Float);
    const P: Option<PropertyType> = Some(PropertyType::PropertyId);
    const ST: Option<PropertyType> = Some(PropertyType::Struct);
    const A: Option<PropertyType> = Some(PropertyType::Array);
    const SI: Option<PropertyType> = Some(PropertyType::StringInfo);
    const B64: Option<PropertyType> = Some(PropertyType::Bitfield64);
    const I: Option<PropertyType> = Some(PropertyType::Int);
    const B32: Option<PropertyType> = Some(PropertyType::Bitfield32);
    const D: Option<PropertyType> = Some(PropertyType::DidRef);
    const BO: Option<PropertyType> = Some(PropertyType::Bool);
    const ANY: Option<PropertyType> = None;

    define_labels! {
        // §4.2 identity, capacity and timers
        TPL_NAME = "PrivateEncounterTemplate_Name": SI;
        TPL_DESCRIPTION = "PrivateEncounterTemplate_Description": SI;
        TPL_MAX_PLAYERS = "PrivateEncounterTemplate_MaxPlayers": I;
        TPL_PUBLIC_INSTANCE = "PrivateEncounterTemplate_PublicInstance": BO;
        TPL_PUBLIC_MAX_PLAYER_PROP = "PrivateEncounterTemplate_PublicInstance_MaxPlayerWorldProp": P;
        TPL_LOCK_SIZE = "PrivateEncounterTemplate_LockSize": E;
        TPL_TIME_DORMANT = "PrivateEncounterTemplate_TimeDormantLimit": F;
        TPL_TIME_GRACE = "PrivateEncounterTemplate_TimeGraceLimit": F;
        TPL_LINKDEAD_OVERRIDE = "PrivateEncounterTemplate_LinkdeadTimeoutOverride": F;
        TPL_FAILURE_OVERRIDE = "PrivateEncounterTemplate_FailureTimeoutOverride": F;
        TPL_LENGTH_DEFAULT = "PrivateEncounterTemplate_LengthDefault": I;
        TPL_LENGTH_RIPCORD = "PrivateEncounterTemplate_LengthRipcord": D;
        TPL_CAN_SELF_REVIVE = "PrivateEncounterTemplate_CanSelfRevive": BO;
        TPL_IS_CINEMATIC = "PrivateEncounterTemplate_IsCinematicEncounter": BO;
        TPL_DYNAMIC_AREA = "PrivateEncounterTemplate_DynamicAreaInstance": BO;
        TPL_WHITE_LIST = "PrivateEncounterTemplate_WhiteList": D;
        // §4.3 places and landblocks
        TPL_STARTING_POSITION = "PrivateEncounterTemplate_StartingPosition": S;
        TPL_SUCCESS_LOCATION = "PrivateEncounterTemplate_SuccessTeleportLocation": S;
        TPL_INSTANCE_MILESTONE = "PrivateEncounterTemplate_InstanceMilestone": S;
        TPL_EXTERNAL_MILESTONE = "PrivateEncounterTemplate_ExternalInstanceMilestone": S;
        TPL_DEFAULT_EXIT = "PrivateEncounterTemplate_DefaultExitLocation": S;
        TPL_EXIT_IF_DOOR = "PrivateEncounterTemplate_ExitLocationIfEnteredThroughDoor": S;
        TPL_LANDBLOCKS = "PrivateEncounterTemplate_ContainedLandBlockArray": A;
        TPL_LANDBLOCK = "PrivateEncounterTemplate_ContainedLandBlock": V;
        TPL_LOAD_ALL = "PrivateEncounterTemplate_ShouldLoadAllLandblocks": BO;
        // §4.4, §4.5
        TPL_CONTENT_LAYER = "PrivateEncounterTemplate_ContentLayer": E;
        TPL_INSTANCE_PROPERTIES = "PrivateEncounterTemplate_InstanceProperties": ST;
        // §4.6 quests
        TPL_INSTANCE_QUEST = "PrivateEncounterTemplate_InstanceQuest": D;
        TPL_TRACKS_QUEST = "PrivateEncounterTemplate_TracksInstanceQuestCompletion": BO;
        TPL_QUESTS_TO_BESTOW = "PrivateEncounterTemplate_QuestToBestow_Array": A;
        TPL_EVENT_CONTROL = "PrivateEncounterTemplate_EventControlDID": D;
        TPL_MEMBER_COUNT_EVENT = "PrivateEncounterTemplate_MemberCountZoneEvent": D;
        TPL_RESPONSE_MAP = "PrivateEncounterTemplate_PropertyResponseMapDID": D;
        // §4.7 permission blobs
        TPL_DEFAULT_PERMISSION = "DefaultPermissionBlobStruct": ST;
        TPL_CREATION_PERMISSION = "PrivateEncounterTemplate_CreationPermissionBlobStruct": ST;
        // §4.8 scenes, sound, sky and map
        TPL_TELEPORT_SCENE = "PrivateEncounterTemplate_TeleportationSceneID": D;
        TPL_SUCCESS_SCENE = "PrivateEncounterTemplate_SuccessTeleportSceneDID": D;
        TPL_INTRO_VO = "PrivateEncounterTemplate_IntroVO": D;
        TPL_SKY_OVERRIDE = "PrivateEncounterTemplate_SkyLocalNameOverride": E;
        TPL_DISALLOWED_MAP_NOTES = "PrivateEncounterTemplate_DisallowedMapNoteType": B64;
        // §4.9 loot and awards
        TPL_SPECIES_LOOT = "PrivateEncounterTemplate_SpeciesLevelLootTable": D;
        TPL_TREASURE_LEVEL = "PrivateEncounterTemplate_TreasureLevel": I;
        TPL_DEFEAT_INFO = "PrivateEncounterTemplate_DefeatInfoOverride": D;
        TPL_AWARD_ARRAY = "PrivateEncounterTemplate_PaperItemAwardArray": A;
        TPL_AWARD_ENTRY = "PrivateEncounterTemplate_PaperItemAwardEntry": ST;
        TPL_AWARD_TYPE = "PrivateEncounterTemplate_PaperItemAwardType": E;
        TPL_RATIO_ARRAY = "PrivateEncounterTemplate_RatioArray": A;
        TPL_AWARD_CURRENCY = "PrivateEncounterTemplate_AwardCurrency": D;
        TPL_AWARD_RATIO = "PrivateEncounterTemplate_AwardRatio": F;
        // §4.10 skirmish and world-join fields
        SK_TIER_ARRAY = "Skirmish_Template_DifficultyTierArray": A;
        SK_GROUP_SIZE_ARRAY = "Skirmish_Template_GroupSizeArray": A;
        SK_DIFFICULTY_TIER = "Skirmish_DifficultyTier": E;
        SK_GROUP_SIZE = "Skirmish_GroupSize": E;
        SK_GROUP_SIZE_OVERRIDE = "Skirmish_Template_GroupSize_Override": E;
        SK_TIER_MIN = "Skirmish_Template_TierMin_ConfigerationArray": A;
        SK_MIN_LEVEL = "Skirmish_Template_MinLevelScale": I;
        SK_MAX_LEVEL = "Skirmish_Template_MaxLevelScale": I;
        SK_LEVEL_SCALING = "Skirmish_Template_LevelScalingLevel": I;
        SK_TIER_GATE = "Skirmish_Template_DifficultyTierGate_PropertyName": P;
        SK_HIGHEST_TIME_BEST = "Skirmish_Template_Highest_Time_Is_Best_Time": BO;
        WJ_ENCOUNTER_TYPE = "WorldJoin_EncounterType": E;
        WJ_CATEGORY = "WorldJoin_EncounterCategory": E;
        WJ_TICKET = "WorldJoin_EncounterTicket": D;
        TPL_ENCOUNTER_TYPE = "PrivateEncounterTemplate_EncounterType": E;
        TPL_DISTANT_BATTLE_SCRIPT = "PrivateEncounterTemplate_DistantBattleScript": D;
        // §5.1 control record
        PE_DEFAULT_TIMEOUT = "PrivateEncounter_DefaultEncounterTimeout": F;
        PE_DEFAULT_LINKDEAD = "PrivateEncounter_DefaultLinkdeadTimeout": F;
        PE_LEAVE_INDUCTION = "PrivateEncounter_LeaveInstanceInduction": D;
        PE_LOCK_ARRAY = "PrivateEncounter_Control_LockArray": A;
        PE_LOCK_SIZE = "PrivateEncounter_Control_LockSize": E;
        PE_LOCK_RESET_DAYS = "PrivateEncounter_Control_LockResetDays": B32;
        PE_LOCKABLE = "PrivateEncounter_Control_LockableEncounterArray": A;
        PE_RESTRICT_EXCLUSIONS = "PrivateEncounter_Control_RestrictMaxPlayersExclusionArray": A;
        PE_PVMP = "PrivateEncounter_Control_PvMP_Challenge_Encounter_Array": A;
        PE_ENTRY_FILTERS = "PrivateEncounter_EntryFilters": A;
        PE_FILTER_IGNORED = "PrivateEncounter_EntryFilters_IgnoredEncounters": A;
        // §5.2 entry filters
        EF_REJECT_MESSAGE = "PrivateEncounter_EntryFilter_RejectMessage": SI;
        EF_REJECT_ERROR = "PrivateEncounter_EntryFilter_RejectErrorType": E;
        EF_ALERT = "PrivateEncounter_EntryFilter_Alert": E;
        // §6.1 match parameters
        MP_LEVEL_MIN = "SkirmishMatchParams_Level_Min": I;
        MP_LEVEL_MAX = "SkirmishMatchParams_Level_Max": I;
        MP_GROUP_MIN = "SkirmishMatchParams_GroupSize_Min": E;
        MP_GROUP_MAX = "SkirmishMatchParams_GroupSize_Max": E;
        MP_TIER_MIN = "SkirmishMatchParams_DifficultyTier_Min": E;
        MP_TIER_MAX = "SkirmishMatchParams_DifficultyTier_Max": E;
        // §6.2 trait lists
        ST_ALLOWED_TRAITS = "Skirmish_SkillTrainer_AllowedTraits": A;
        // §6.3 skirmish control
        SC_DIFFICULTY_MULT = "SkirmishControl_DifficultyMultiplier_Array": A;
        SC_GROUP_SIZE_MULT = "SkirmishControl_GroupSizeMultiplier_Array": A;
        SC_ZONE_EVENTS = "SkirmishControl_ZoneEvents": ST;
        SC_POINT_TOKEN = "SkirmishControl_SkirmishPointToken": D;
        SC_PANEL_TRAIT = "SkirmishControl_SkirmishPanel_RequiredTrait": D;
        SC_TRAIT_GATES = "SkirmishControl_SkirmishTraitGates": A;
        SC_GATE_DEFAULT_RANK = "SkirmishControl_SkirmishTraitGate_DEFAULT_MaxRank": I;
        SC_TRAIT_POOLS = "SkirmishControl_SkirmishTraitPools": A;
        SC_MAX_PER_WEEK = "SkirmishControl_SkirmishGating_DEFAULT_MaxSkirmishesPerWeek": I;
        SC_BLOCKED_STATS = "SkirmishControl_BlockedStats_List": A;
        SC_GRS_LIST = "SkirmishControl_SkirmishGRSEnum_PropertyList": A;
        SC_STAT_TO_GRS = "SkirmishControl_SkirmishStat_To_GRSEnum_PropertyName_Hash": A;
        SC_STAT = "SkirmishControl_SkirmishStat": E;
        SC_CONVERSION = "SkirmishStats_StatProperty_Conversion_Hash": A;
        SC_FASTEST = "SkirmishStats_CONSTANT_FASTEST_POSSIBLE_SKIRMISH_TIME": F;
        SC_STATS = "Skirmish_Stats": A;
        STAT_DERIVED = "Skirmish_DerivedStat": ANY;
        STAT_PRIMARY = "Skirmish_Primary_Stat": E;
        STAT_OPERATIONS = "Skirmish_Stat_Operations": A;
        STAT_SECONDARY = "Skirmish_Secondary_Stat": E;
        MOD_OP = "Mod_Op": E;
        STAT_SCALE = "Skirmish_Stat_Scale": I;
        // §7 world join
        WJC_AVAILABLE = "WorldJoin_AvailablePETemplates_Array": A;
        WJC_GROUPS = "WorldJoinControl_InstanceGroups": A;
        WJC_GROUP_LABEL = "WorldJoinControl_Instance_Group_Label": E;
        WJC_GROUP_LIST = "WorldJoinControl_Grouped_Instance_List": A;
        WJC_FEATURED = "WorldJoinControl_FeaturedInstance_Array": A;
        WJC_FEATURED_EVENT = "WorldJoinControl_FeaturedInstance_WorldEvent": D;
        WJC_DEFAULT_TICKET = "WorldJoinControl_DefaultTicket": D;
        WJC_MIN_LEVEL = "WorldJoinControl_MinLevel": I;
        WJC_SEASONAL = "WorldJoin_Seasonal_EncounterCategory_Array": A;
        WJC_CAPPED = "WorldJoin_Capped_EncounterCategory": E;
        TK_MAX_ENCOUNTERS = "WorldJoinTicket_MaxEncounters_RipCord": D;
        TK_MIN_TIME = "WorldJoinTicket_MinTimeBetweenLaunches_RipCord": D;
        // §8.1 big-battle control
        BB_INITIAL_POINTS = "BigBattleControl_InitialTraitPoints": I;
        BB_REFUND = "BigBattleControl_Traits_RefundCostByLevel_Progression": D;
        BB_FIRST_BONUS = "BigBattleControl_Rewards_FirstTimeMedal_Bonus": I;
        BB_MEDALS = "BigBattleControl_Medal_Info_Hash": A;
        BB_MEDAL = "BigBattleControl_Medal": E;
        BB_TRAIT_POINTS = "BigBattleControl_TraitPointsToGrant": I;
        BB_CASHOUT_REQUIRED = "BigBattleControl_CashOut_PointsRequired": I;
        BB_CASHOUT_CURRENT = "BigBattleControl_CashOut_CurrentPoints": P;
        BB_BONUS_PROP = "BigBattleControl_Medal_BonusProp": P;
        BB_CASHOUT_LOOT = "BigBattleControl_FTT_ForCashOut_Loot": D;
        BB_MEDAL_ALERT = "BigBattleControl_Medal_Alert": E;
        BB_QUEST_POINTS = "BigBattleControl_Quest_RewardValue_Hash": A;
        BB_QUEST = "BigBattleControl_Quest": D;
        BB_QUEST_ENCOUNTERS = "BigBattleControl_Quest_To_Encounter_Hash": A;
        BB_LOCATION_EVENTS = "BigBattleControl_LocationToZoneEvent_Array": A;
        BB_WAVE_LOCATION = "BigBattleControl_BigBattleWaveLocation": E;
        // §8.3 distant-battle scripts
        DB_ZONE_REQUIRED = "DistantBattle_ZoneEvent_Required": D;
        DB_ZONE_PROGRESS = "DistantBattle_ZoneEvent_Progress": D;
        DB_WAVES = "DistantBattle_Wave_List": A;
        DB_FLANKS = "DistantBattle_AllowedFlank_List": A;
        DB_FLANK = "DistantBattle_AllowedFlank": E;
        DB_DBU_COUNT = "DistantBattle_DBU_Count": I;
        DB_SIEGE_COUNT = "DistantBattle_DBU_Siege_Count": I;
        DB_REQUIREMENTS = "DistantBattle_WaveRequirements": ST;
        DB_FULL_CLEAR = "DistantBattle_ZoneEvent_FullClear": D;
        DB_NO_SECONDARY = "DistantBattle_NoSecondaryQuest": BO;
        DB_OBJECTIVES = "DistantBattle_Objective_List": A;
        DB_OBJ_QUEST = "DistantBattle_Objective_Quest": D;
        DB_OBJ_EVENT = "DistantBattle_Objective_ZoneEvent": D;
        DB_WAVE_COUNT = "DistantBattle_Wave_Count": I;
        DBU_WEIGHTED = "DistantBattleUnit_WeightedList": A;
        DBU_SIEGE_WEIGHTED = "DistantBattleUnit_Siege_WeightedList": A;
        DBU_ENTRY = "DistantBattleUnit_Entry": D;
        // §8.4 distant-battle units, §15.1 B5
        DBU_ENTITY_COMPOSITION = "DistantBattleUnit_EntityComposition": A;
        DBU_UNIT = "DistantBattleUnit_Formation_Unit": D;
        DBU_COSMETIC = "DistantBattleUnit_CosmeticEntity": D;
        DBU_OFFSETS = "DistantBattleUnit_Entity_Offset_Array": A;
        DBU_VISUAL_RADIUS = "DistantBattleUnit_VisualRadius": F;
        DBU_JITTER = "DistantBattleUnit_Formation_Jitter": F;
        APPEARANCE_CLASS_LIST = "Appearance_ClassList": A;
        DBU_FORMATION = "DistantBattleUnit_Formation": A;
        DBU_PRIORITY = "DistantBattleUnit_Formation_Priority": I;
        DBU_LATERAL_DEPTH = "DistantBattleUnit_Formation_LateralDepth": I;
        DBU_WIDTH_RATIO = "DistantBattleUnit_Formation_LateralWidth_Ratio": F;
        DBU_LAYOUT = "DistantBattleUnit_Formation_LayoutCalculator": D;
        DBU_DIMENSIONS = "DistantBattleUnit_Formation_Dimensions": V;
        DBU_WIDTH = "DistantBattleUnit_Formation_Width": E;
        DBU_COMPOSITION = "DistantBattleUnit_Composition": A;
        DBU_SPAWN_RATE = "DistantBattleUnit_SpawnRate": F;
        DBU_MIN_ACTIVATORS = "DistantBattleUnit_Min_Activator_Count": I;
        DBU_RESPAWN = "DistantBattleUnit_Activator_Respawn_Time": F;
        DBU_GROUND_CLAMP = "DistantBattleUnit_GroundClampEntities": BO;
        DBU_CAPTURE = "DistantBattleUnit_CaptureQualityOverride": E;
        DBU_LANE = "DistantBattleUnit_LaneType": E;
        DBU_REMOVED = "DistantBattleUnit_Composition_Removed_List": ANY;
        DBU_REMOVAL_COUNT = "DistantBattleUnit_Removal_Count": ANY;
        DBU_REMOVAL_SCRIPT = "DistantBattleUnit_Removal_ScriptKey": ANY;
        // §8.5 formation control
        FC_SPACING = "DistantBattleUnit_FormationControl_SpacingBetweenGroups": ANY;
        FC_WIDTHS = "DistantBattleUnit_FormationControl_Width_Lookup": A;
        FC_ACTUAL_WIDTH = "DistantBattleUnit_FormationControl_ActualWidth": ANY;
        // §8.7 battle statistics
        BS_NAME = "BattleStat_Name": SI;
        BS_DESCRIPTION = "BattleStat_Description": SI;
        BS_SCOPE = "BattleStat_Scope": E;
        BS_POLICY = "BattleStat_UpdatePolicy": E;
        BS_REPLICATE = "BattleStat_ReplicateToClients": BO;
        BS_FORWARD = "BattleStat_ForwardUpdatesToStat": D;
        // §9 session play
        SP_LIST = "SessionPlay_AllowedSlotCount_List": A;
        SP_DID = "SessionPlay_DID": D;
        SP_COUNT = "SessionPlay_AllowedSlotCount": I;
        // §10.1 fellowship maker
        FM_MIN_LEVEL = "FellowshipMakerControl_MinLevel": I;
        FM_SIZES = "FellowshipMakerControl_ValidEncounterSizeArray": A;
        FM_MATCH_TIME = "FellowshipMakerControl_MatchTimeWorldPropertyBySizeArray": A;
        FM_BONUS = "FellowshipMakerControl_BonusEffectThreshold_Array": A;
        FM_RANDOM_BONUS = "FellowshipMakerControl_RandomBonusEffect": D;
        FM_DESERTER = "FellowshipMakerControl_DeserterEffect": D;
        FM_EXCLUDED = "FellowshipMakerControl_ExcludedEncounterArray": A;
        // §11 phasing
        PB_UPDATE_ONLY = "PhasingBox_UpdateOnlyOnCreation": ANY;
        PB_DELAY = "PhasingBox_DelayRefreshOfLinkedEntities": ANY;
        PH_FORCE_REFRESH = "Phasing_ForcePhasingRefresh_OnEnterWorld": ANY;
    }
}

/// The PIDs of [`labels::ALL`], looked up in a dictionary at run time.
///
/// A label the dictionary does not define reads as "absent" everywhere. Two sets of enum
/// PIDs are kept for fields the spec names only by suffix and that share a value type with
/// a sibling field: the PIDs whose definition uses the `SkirmishStat` mapper and those that
/// use the `SkirmishGroupSize` mapper (§3.3: "every other group-size field").
#[derive(Debug, Clone, Default)]
pub struct InstanceVocab {
    pids: HashMap<&'static str, u32>,
    stat_enums: BTreeSet<u32>,
    size_enums: BTreeSet<u32>,
}

impl InstanceVocab {
    /// Looks every label up in `names`. The statistic and group-size enum sets hold only
    /// the PIDs of `SkirmishControl_SkirmishStat` and `Skirmish_GroupSize`.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let mut pids = HashMap::with_capacity(labels::ALL.len());
        for (label, _) in labels::ALL {
            if let Some(pid) = names.pid_of(label) {
                pids.insert(*label, pid);
            }
        }
        let mut v = Self {
            pids,
            ..Self::default()
        };
        v.stat_enums.extend(v.pid(labels::SC_STAT));
        v.size_enums.extend(v.pid(labels::SK_GROUP_SIZE));
        v
    }

    /// Looks every label up in the dictionary and fills the statistic and group-size enum
    /// sets with every enum PID whose definition names the same mapper as
    /// `SkirmishControl_SkirmishStat` and `Skirmish_GroupSize` respectively.
    #[must_use]
    pub fn from_dictionary(dict: &PropertyDictionary) -> Self {
        let mut v = Self::from_names(dict);
        let source = |label: &str| {
            v.pid(label)
                .and_then(|p| dict.definition(p))
                .filter(|d| d.ty == PropertyType::Enum && d.data.0 != 0)
                .map(|d| d.data)
        };
        let stat = source(labels::SC_STAT);
        let size = source(labels::SK_GROUP_SIZE);
        let (mut stats, mut sizes) = (BTreeSet::new(), BTreeSet::new());
        for d in dict.definitions() {
            if d.ty != PropertyType::Enum {
                continue;
            }
            if Some(d.data) == stat {
                stats.insert(d.pid);
            }
            if Some(d.data) == size {
                sizes.insert(d.pid);
            }
        }
        v.stat_enums.extend(stats);
        v.size_enums.extend(sizes);
        v
    }

    /// The PID of `label`, when the dictionary defines it.
    #[must_use]
    pub fn pid(&self, label: &str) -> Option<u32> {
        self.pids.get(label).copied()
    }

    /// The labels of [`labels::ALL`] the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        labels::ALL
            .iter()
            .map(|(l, _)| *l)
            .filter(|l| !self.pids.contains_key(l))
            .collect()
    }

    /// Labels whose dictionary type differs from the type the spec gives
    /// (label, expected, found).
    pub fn type_mismatches<S: PropertySchema + ?Sized>(
        &self,
        schema: &S,
    ) -> Vec<(&'static str, PropertyType, Option<PropertyType>)> {
        let mut out = Vec::new();
        for (label, ty) in labels::ALL {
            let (Some(want), Some(pid)) = (ty, self.pid(label)) else {
                continue;
            };
            let found = schema.property_type(pid);
            if found != Some(*want) {
                out.push((*label, *want, found));
            }
        }
        out
    }

    /// Whether `pid` is an enum field of the `SkirmishStat` mapper.
    #[must_use]
    pub fn is_stat_enum(&self, pid: u32) -> bool {
        self.stat_enums.contains(&pid)
    }

    /// Whether `pid` is an enum field of the `SkirmishGroupSize` mapper.
    #[must_use]
    pub fn is_size_enum(&self, pid: u32) -> bool {
        self.size_enums.contains(&pid)
    }

    // ---------------------------------------------------------------- typed access

    pub(crate) fn value<'a, F: Fields + ?Sized>(
        &self,
        f: &'a F,
        label: &str,
    ) -> Option<&'a PropertyValue> {
        self.pid(label).and_then(|p| f.field(p))
    }

    pub(crate) fn has<F: Fields + ?Sized>(&self, f: &F, label: &str) -> bool {
        self.value(f, label).is_some()
    }

    pub(crate) fn int<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<i32> {
        match self.value(f, label)? {
            PropertyValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub(crate) fn float<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<f32> {
        self.value(f, label).and_then(PropertyValue::as_f32)
    }

    /// Any numeric value as f64 (fields whose type the spec does not state).
    pub(crate) fn number<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<f64> {
        self.value(f, label).and_then(PropertyValue::as_f64)
    }

    pub(crate) fn enum_key<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<u32> {
        match self.value(f, label)? {
            PropertyValue::Enum(v) => Some(*v),
            _ => None,
        }
    }

    /// An unsigned integer of any integral type (fields whose type the spec does not state).
    pub(crate) fn bits32<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<u32> {
        self.value(f, label)
            .and_then(PropertyValue::as_bits)
            .and_then(|b| u32::try_from(b).ok())
    }

    pub(crate) fn did<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<Did> {
        self.value(f, label).and_then(PropertyValue::as_did)
    }

    pub(crate) fn pid_value<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<u32> {
        match self.value(f, label)? {
            PropertyValue::PropertyId(v) => Some(*v),
            _ => None,
        }
    }

    pub(crate) fn boolean<F: Fields + ?Sized>(&self, f: &F, label: &str) -> Option<bool> {
        match self.value(f, label)? {
            PropertyValue::Bool(b) => Some(*b),
            other => other.as_bits().map(|b| b != 0),
        }
    }

    /// A stored boolean; absent reads as `false` (§3.1).
    pub(crate) fn flag<F: Fields + ?Sized>(&self, f: &F, label: &str) -> bool {
        self.boolean(f, label).unwrap_or(false)
    }

    pub(crate) fn text<'a, F: Fields + ?Sized>(&self, f: &'a F, label: &str) -> Option<&'a str> {
        match self.value(f, label)? {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub(crate) fn string_info<'a, F: Fields + ?Sized>(
        &self,
        f: &'a F,
        label: &str,
    ) -> Option<&'a StringInfo> {
        match self.value(f, label)? {
            PropertyValue::StringInfo(s) => Some(s),
            _ => None,
        }
    }

    /// The elements of an array property; empty when absent or not an array.
    pub(crate) fn elements<'a, F: Fields + ?Sized>(
        &self,
        f: &'a F,
        label: &str,
    ) -> &'a [PropertyEntry] {
        self.value(f, label)
            .and_then(PropertyValue::as_array)
            .map_or(&[], |a| a.elements.as_slice())
    }
}

/// Something with fields addressed by PID: a property set or a struct value.
pub trait Fields {
    /// The value of `pid`, if present.
    fn field(&self, pid: u32) -> Option<&PropertyValue>;
}

impl Fields for PropertySet {
    fn field(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(pid)
    }
}

impl Fields for PropertyStruct {
    fn field(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(pid)
    }
}

/// The DIDs among `elements`, in stored order (other element kinds are skipped).
pub(crate) fn dids(elements: &[PropertyEntry]) -> Vec<Did> {
    elements.iter().filter_map(|e| e.value.as_did()).collect()
}

/// The enum keys among `elements`, in stored order.
pub(crate) fn enum_keys(elements: &[PropertyEntry]) -> Vec<u32> {
    elements
        .iter()
        .filter_map(|e| match e.value {
            PropertyValue::Enum(k) => Some(k),
            _ => None,
        })
        .collect()
}

/// The struct values among `elements`, in stored order.
pub(crate) fn structs(elements: &[PropertyEntry]) -> impl Iterator<Item = &PropertyStruct> {
    elements.iter().filter_map(|e| e.value.as_struct())
}

/// The first field of `s` whose value has type `ty` and whose PID is not `except`.
pub(crate) fn first_of_type(
    s: &PropertyStruct,
    ty: PropertyType,
    except: Option<u32>,
) -> Option<&PropertyValue> {
    s.fields
        .iter()
        .find(|e| e.value.property_type() == ty && Some(e.pid) != except)
        .map(|e| &e.value)
}

/// A DID with value 0 means "none" (`property-system.md` §4).
pub(crate) fn nonzero(d: Option<Did>) -> Option<Did> {
    d.filter(|d| d.0 != 0)
}

/// The kind of a record of this family, told by its characteristic property (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceKind {
    /// Private-encounter template (root classes 1368 and 2651 here).
    Template,
    /// Skirmish match parameters (2727).
    MatchParams,
    /// Skirmish skill-trainer trait list (2669).
    SkillTrainerTraits,
    /// Private-encounter control (1366).
    EncounterControl,
    /// Entry filter (2573, 2576).
    EntryFilter,
    /// Skirmish control (2515).
    SkirmishControl,
    /// World-join control (2606).
    WorldJoinControl,
    /// World-join ticket (3894).
    WorldJoinTicket,
    /// Big-battle control (3787).
    BigBattleControl,
    /// Distant-battle script (3808).
    DistantBattleScript,
    /// Distant-battle unit (3725).
    DistantBattleUnit,
    /// Distant-battle unit formation control (3745).
    FormationControl,
    /// Session-play control (1925).
    SessionPlayControl,
    /// Fellowship-maker control (3276).
    FellowshipMakerControl,
    /// Phasing box (3369).
    PhasingBox,
    /// Battle statistic (3590, 3622).
    BattleStat,
}

/// The characteristic property of each kind, in the order [`classify`] tests them (§3.4).
pub const CHARACTERISTIC: &[(&str, InstanceKind)] = &[
    (labels::TPL_CONTENT_LAYER, InstanceKind::Template),
    (labels::MP_LEVEL_MIN, InstanceKind::MatchParams),
    (labels::ST_ALLOWED_TRAITS, InstanceKind::SkillTrainerTraits),
    (labels::PE_LOCK_ARRAY, InstanceKind::EncounterControl),
    (labels::EF_REJECT_ERROR, InstanceKind::EntryFilter),
    (labels::SC_TRAIT_GATES, InstanceKind::SkirmishControl),
    (labels::WJC_GROUPS, InstanceKind::WorldJoinControl),
    (labels::TK_MAX_ENCOUNTERS, InstanceKind::WorldJoinTicket),
    (labels::BB_MEDALS, InstanceKind::BigBattleControl),
    (labels::DB_WAVES, InstanceKind::DistantBattleScript),
    (labels::DBU_RESPAWN, InstanceKind::DistantBattleUnit),
    (labels::FC_WIDTHS, InstanceKind::FormationControl),
    (labels::SP_LIST, InstanceKind::SessionPlayControl),
    (labels::FM_SIZES, InstanceKind::FellowshipMakerControl),
    (labels::PB_UPDATE_ONLY, InstanceKind::PhasingBox),
    (labels::BS_NAME, InstanceKind::BattleStat),
];

/// The kind of the record whose property set is `set`, or `None` when the set holds no
/// characteristic property of this family. The first match of [`CHARACTERISTIC`] wins; in
/// the observed data no set holds two.
#[must_use]
pub fn classify(set: &PropertySet, vocab: &InstanceVocab) -> Option<InstanceKind> {
    CHARACTERISTIC
        .iter()
        .find(|(label, _)| vocab.has(set, label))
        .map(|(_, kind)| *kind)
}

/// A typed view of one record of the family.
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceRecord {
    /// §4.
    Template(Box<Template>),
    /// §6.1.
    MatchParams(MatchParams),
    /// §6.2.
    SkillTrainerTraits(SkillTrainerTraits),
    /// §5.1.
    EncounterControl(Box<EncounterControl>),
    /// §5.2.
    EntryFilter(EntryFilter),
    /// §6.3.
    SkirmishControl(Box<SkirmishControl>),
    /// §7.1.
    WorldJoinControl(Box<WorldJoinControl>),
    /// §7.2.
    WorldJoinTicket(WorldJoinTicket),
    /// §8.1.
    BigBattleControl(Box<BigBattleControl>),
    /// §8.3.
    DistantBattleScript(Box<DistantBattleScript>),
    /// §8.4.
    DistantBattleUnit(Box<DistantBattleUnit>),
    /// §8.5.
    FormationControl(FormationControl),
    /// §9.
    SessionPlayControl(SessionPlayControl),
    /// §10.1.
    FellowshipMakerControl(Box<FellowshipMakerControl>),
    /// §11.
    PhasingBox(PhasingFlags),
    /// §8.7.
    BattleStat(BattleStat),
}

impl InstanceRecord {
    /// Classifies `set` and reads the matching view; `None` when the set is not a record of
    /// this family.
    #[must_use]
    pub fn read(set: &PropertySet, vocab: &InstanceVocab) -> Option<Self> {
        Some(match classify(set, vocab)? {
            InstanceKind::Template => Self::Template(Box::new(Template::read(set, vocab))),
            InstanceKind::MatchParams => Self::MatchParams(MatchParams::read(set, vocab)),
            InstanceKind::SkillTrainerTraits => {
                Self::SkillTrainerTraits(SkillTrainerTraits::read(set, vocab))
            }
            InstanceKind::EncounterControl => {
                Self::EncounterControl(Box::new(EncounterControl::read(set, vocab)))
            }
            InstanceKind::EntryFilter => Self::EntryFilter(EntryFilter::read(set, vocab)),
            InstanceKind::SkirmishControl => {
                Self::SkirmishControl(Box::new(SkirmishControl::read(set, vocab)))
            }
            InstanceKind::WorldJoinControl => {
                Self::WorldJoinControl(Box::new(WorldJoinControl::read(set, vocab)))
            }
            InstanceKind::WorldJoinTicket => {
                Self::WorldJoinTicket(WorldJoinTicket::read(set, vocab))
            }
            InstanceKind::BigBattleControl => {
                Self::BigBattleControl(Box::new(BigBattleControl::read(set, vocab)))
            }
            InstanceKind::DistantBattleScript => {
                Self::DistantBattleScript(Box::new(DistantBattleScript::read(set, vocab)))
            }
            InstanceKind::DistantBattleUnit => {
                Self::DistantBattleUnit(Box::new(DistantBattleUnit::read(set, vocab)))
            }
            InstanceKind::FormationControl => {
                Self::FormationControl(FormationControl::read(set, vocab))
            }
            InstanceKind::SessionPlayControl => {
                Self::SessionPlayControl(SessionPlayControl::read(set, vocab))
            }
            InstanceKind::FellowshipMakerControl => {
                Self::FellowshipMakerControl(Box::new(FellowshipMakerControl::read(set, vocab)))
            }
            InstanceKind::PhasingBox => Self::PhasingBox(PhasingFlags::read(set, vocab)),
            InstanceKind::BattleStat => Self::BattleStat(BattleStat::read(set, vocab)),
        })
    }

    /// The kind of this view.
    #[must_use]
    pub fn kind(&self) -> InstanceKind {
        match self {
            Self::Template(_) => InstanceKind::Template,
            Self::MatchParams(_) => InstanceKind::MatchParams,
            Self::SkillTrainerTraits(_) => InstanceKind::SkillTrainerTraits,
            Self::EncounterControl(_) => InstanceKind::EncounterControl,
            Self::EntryFilter(_) => InstanceKind::EntryFilter,
            Self::SkirmishControl(_) => InstanceKind::SkirmishControl,
            Self::WorldJoinControl(_) => InstanceKind::WorldJoinControl,
            Self::WorldJoinTicket(_) => InstanceKind::WorldJoinTicket,
            Self::BigBattleControl(_) => InstanceKind::BigBattleControl,
            Self::DistantBattleScript(_) => InstanceKind::DistantBattleScript,
            Self::DistantBattleUnit(_) => InstanceKind::DistantBattleUnit,
            Self::FormationControl(_) => InstanceKind::FormationControl,
            Self::SessionPlayControl(_) => InstanceKind::SessionPlayControl,
            Self::FellowshipMakerControl(_) => InstanceKind::FellowshipMakerControl,
            Self::PhasingBox(_) => InstanceKind::PhasingBox,
            Self::BattleStat(_) => InstanceKind::BattleStat,
        }
    }
}

/// Parses a property-set payload (prefix 0x79, `property-system.md` §5–§6) with `schema`
/// and reads the family view of it. `expected` checks the record's own DID when given.
/// Returns the parsed record and the view (`None` when the set belongs to no record of this
/// family).
///
/// # Errors
///
/// Any [`crate::FormatError`] of [`PropertySetRecord::parse`] (truncation, unknown PID,
/// depth or memory limit, DID mismatch).
pub fn decode_instance_set<S: PropertySchema + ?Sized>(
    payload: &[u8],
    expected: Option<Did>,
    schema: &S,
    vocab: &InstanceVocab,
) -> Result<(PropertySetRecord, Option<InstanceRecord>)> {
    let rec = match expected {
        Some(did) => PropertySetRecord::parse_expecting(payload, did, schema)?,
        None => PropertySetRecord::parse(payload, schema)?,
    };
    let view = InstanceRecord::read(&rec.set, vocab);
    if let Some(v) = &view {
        tracing::trace!(did = rec.did().0, kind = ?v.kind(), "instance record");
    }
    Ok((rec, view))
}

/// One broken invariant of §16, found on a record or between records. Readers warn, they
/// do not fail (§16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    /// The invariant (`"N-I4"`, ...).
    pub invariant: &'static str,
    /// What was found (numbers and DIDs only).
    pub detail: String,
}

impl Deviation {
    pub(crate) fn new(invariant: &'static str, detail: impl Into<String>) -> Self {
        Self {
            invariant,
            detail: detail.into(),
        }
    }
}
