// SPDX-License-Identifier: GPL-3.0-or-later
//! Private-encounter templates (`instances.md` §4): identity, capacity, timers, places, the
//! landblock list (§4.3), content layer (§4.4), quests (§4.6), permission blobs (§4.7),
//! scenes and sound (§4.8), awards (§4.9) and the skirmish and world-join fields (§4.10).

use std::collections::BTreeSet;

use super::labels as l;
use super::{Deviation, InstanceVocab, dids, enum_keys, nonzero, structs};
use crate::did::Did;
use crate::property::{PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::string_info::StringInfo;

/// Largest landblock index (exclusive) of N-I4.
pub const LANDBLOCK_INDEX_LIMIT: u32 = 256;

/// The divisor of the landblock-list encoding (§4.3).
pub const LANDBLOCK_LIST_SCALE: u32 = 8;

/// One entry of `PrivateEncounterTemplate_ContainedLandBlockArray` (§4.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainedLandblock {
    /// Region number (first component).
    pub region: u32,
    /// Landblock X = ⌊X ÷ 8⌋.
    pub x: u8,
    /// Landblock Y = ⌊Y ÷ 8⌋.
    pub y: u8,
    /// The stored vector (region, 8 × X, 8 × Y).
    pub raw: [f32; 3],
}

impl ContainedLandblock {
    /// Applies the §4.3 rule to a stored vector. `None` when a component is not a
    /// non-negative integer or a landblock index reaches 256 (N-I4).
    #[must_use]
    pub fn from_vector(raw: [f32; 3]) -> Option<Self> {
        let [r, x, y] = raw.map(non_negative_integer);
        let (region, x, y) = (r?, x?, y?);
        let x = u8::try_from(x / LANDBLOCK_LIST_SCALE).ok()?;
        let y = u8::try_from(y / LANDBLOCK_LIST_SCALE).ok()?;
        Some(Self { region, x, y, raw })
    }

    /// (region, X, Y).
    #[must_use]
    pub fn key(&self) -> (u32, u8, u8) {
        (self.region, self.x, self.y)
    }

    /// Whether both stored coordinates are multiples of 8 (4,410 of 4,628 observed).
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        let [_, x, y] = self.raw;
        [x, y].iter().all(|v| {
            non_negative_integer(*v).is_some_and(|n| n.is_multiple_of(LANDBLOCK_LIST_SCALE))
        })
    }
}

/// A float that holds a non-negative integer below 2³², as u32.
fn non_negative_integer(v: f32) -> Option<u32> {
    if !v.is_finite() || v < 0.0 || v.fract() != 0.0 || v >= 4_294_967_296.0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(v as u32)
}

/// Template kind (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TemplateKind {
    /// Not a skirmish template (root class 1368 here).
    Ordinary,
    /// Skirmish template with `PrivateEncounterTemplate_EncounterType` 1.
    Mission,
    /// Skirmish template with `WorldJoin_EncounterCategory`.
    WorldJoinEncounter,
    /// Skirmish template with neither.
    OtherSkirmish,
}

/// One paper-item award entry (§4.9).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AwardEntry {
    /// `PrivateEncounterTemplate_PaperItemAwardType` (mapper `PEAwardType`).
    pub award_type: Option<u32>,
    /// (`…_AwardCurrency`, `…_AwardRatio`) of each ratio element.
    pub ratios: Vec<(Option<Did>, Option<f32>)>,
}

/// One `Skirmish_Template_TierMin_ConfigerationArray` entry (§4.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TierConfiguration {
    /// `Skirmish_DifficultyTier`.
    pub tier: Option<u32>,
    /// `Skirmish_GroupSize` (0 in most entries).
    pub group_size: Option<u32>,
}

/// Place-key fields of a template (§4.3); each names a position of `map-data.md` §3.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplatePlaces {
    /// `PrivateEncounterTemplate_StartingPosition`.
    pub starting_position: Option<String>,
    /// `PrivateEncounterTemplate_SuccessTeleportLocation`.
    pub success_location: Option<String>,
    /// `PrivateEncounterTemplate_InstanceMilestone`.
    pub instance_milestone: Option<String>,
    /// `PrivateEncounterTemplate_ExternalInstanceMilestone`.
    pub external_milestone: Option<String>,
    /// `PrivateEncounterTemplate_DefaultExitLocation`.
    pub default_exit: Option<String>,
    /// `PrivateEncounterTemplate_ExitLocationIfEnteredThroughDoor`.
    pub exit_if_door: Option<String>,
}

/// A private-encounter template (§4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Template {
    /// Whether the set carries `Skirmish_Template_TierMin_ConfigerationArray`, which tells
    /// the skirmish subclass (2651 here) from the base class (§3.4).
    pub skirmish_class: bool,
    /// `PrivateEncounterTemplate_Name`.
    pub name: Option<StringInfo>,
    /// `PrivateEncounterTemplate_Description`.
    pub description: Option<StringInfo>,
    /// `PrivateEncounterTemplate_ContentLayer` (key of the `ContentLayer` DID map, §4.4).
    pub content_layer: Option<u32>,
    /// `PrivateEncounterTemplate_MaxPlayers`.
    pub max_players: Option<i32>,
    /// `PrivateEncounterTemplate_PublicInstance`.
    pub public_instance: bool,
    /// `PrivateEncounterTemplate_PublicInstance_MaxPlayerWorldProp` (a world-property PID).
    pub public_max_player_property: Option<u32>,
    /// `PrivateEncounterTemplate_LockSize` (§5.1).
    pub lock_size: Option<u32>,
    /// `PrivateEncounterTemplate_TimeDormantLimit`.
    pub time_dormant_limit: Option<f32>,
    /// `PrivateEncounterTemplate_TimeGraceLimit`.
    pub time_grace_limit: Option<f32>,
    /// `PrivateEncounterTemplate_LinkdeadTimeoutOverride`.
    pub linkdead_timeout_override: Option<f32>,
    /// `PrivateEncounterTemplate_FailureTimeoutOverride`.
    pub failure_timeout_override: Option<f32>,
    /// `PrivateEncounterTemplate_LengthDefault` (−1 = none).
    pub length_default: Option<i32>,
    /// `PrivateEncounterTemplate_LengthRipcord` (0 = none).
    pub length_ripcord: Option<Did>,
    /// `PrivateEncounterTemplate_CanSelfRevive`.
    pub can_self_revive: bool,
    /// `PrivateEncounterTemplate_IsCinematicEncounter`.
    pub is_cinematic: bool,
    /// `PrivateEncounterTemplate_DynamicAreaInstance`.
    pub dynamic_area_instance: bool,
    /// `PrivateEncounterTemplate_WhiteList`.
    pub white_list: Option<Did>,
    /// Place keys.
    pub places: TemplatePlaces,
    /// Whether `PrivateEncounterTemplate_ContainedLandBlockArray` is stored.
    pub has_landblock_list: bool,
    /// The landblock list after the §4.3 rule, in stored order (duplicates kept).
    pub landblocks: Vec<ContainedLandblock>,
    /// Stored list elements that break N-I4 (not a vector, or not an encodable landblock).
    pub bad_landblocks: Vec<Option<[f32; 3]>>,
    /// `PrivateEncounterTemplate_ShouldLoadAllLandblocks`.
    pub load_all_landblocks: bool,
    /// Number of fields of `PrivateEncounterTemplate_InstanceProperties` (§4.5), if stored.
    pub instance_property_count: Option<usize>,
    /// `PrivateEncounterTemplate_InstanceQuest`.
    pub instance_quest: Option<Did>,
    /// `PrivateEncounterTemplate_TracksInstanceQuestCompletion`.
    pub tracks_instance_quest: bool,
    /// `PrivateEncounterTemplate_QuestToBestow_Array`.
    pub quests_to_bestow: Vec<Did>,
    /// `PrivateEncounterTemplate_EventControlDID`.
    pub event_control: Option<Did>,
    /// `PrivateEncounterTemplate_MemberCountZoneEvent`.
    pub member_count_event: Option<Did>,
    /// `PrivateEncounterTemplate_PropertyResponseMapDID`.
    pub property_response_map: Option<Did>,
    /// Whether `DefaultPermissionBlobStruct` is stored (§4.7).
    pub has_default_permission: bool,
    /// Whether `PrivateEncounterTemplate_CreationPermissionBlobStruct` is stored.
    pub has_creation_permission: bool,
    /// `PrivateEncounterTemplate_TeleportationSceneID`.
    pub teleport_scene: Option<Did>,
    /// `PrivateEncounterTemplate_SuccessTeleportSceneDID`.
    pub success_scene: Option<Did>,
    /// `PrivateEncounterTemplate_IntroVO` (a sound-info DID, prefix 0x2A, §3.2).
    pub intro_vo: Option<Did>,
    /// `PrivateEncounterTemplate_SkyLocalNameOverride`.
    pub sky_override: Option<u32>,
    /// `PrivateEncounterTemplate_DisallowedMapNoteType` (64-bit bitfield).
    pub disallowed_map_notes: Option<u64>,
    /// `PrivateEncounterTemplate_SpeciesLevelLootTable`.
    pub species_loot_table: Option<Did>,
    /// `PrivateEncounterTemplate_TreasureLevel`.
    pub treasure_level: Option<i32>,
    /// `PrivateEncounterTemplate_DefeatInfoOverride`.
    pub defeat_info: Option<Did>,
    /// Award entries: the array's structs, then the single `…_PaperItemAwardEntry`.
    pub awards: Vec<AwardEntry>,
    /// `Skirmish_Template_DifficultyTierArray` keys in stored order.
    pub tiers: Vec<u32>,
    /// `Skirmish_Template_GroupSizeArray` keys in stored order.
    pub group_sizes: Vec<u32>,
    /// `Skirmish_Template_GroupSize_Override`.
    pub group_size_override: Option<u32>,
    /// `Skirmish_Template_TierMin_ConfigerationArray`.
    pub tier_minimums: Vec<TierConfiguration>,
    /// `Skirmish_Template_MinLevelScale`.
    pub min_level_scale: Option<i32>,
    /// `Skirmish_Template_MaxLevelScale`.
    pub max_level_scale: Option<i32>,
    /// `Skirmish_Template_LevelScalingLevel` (§15.1 B2).
    pub level_scaling_level: Option<i32>,
    /// `Skirmish_Template_DifficultyTierGate_PropertyName` (0 in most).
    pub tier_gate_property: Option<u32>,
    /// `Skirmish_Template_Highest_Time_Is_Best_Time` (§15.1 B4).
    pub highest_time_is_best: bool,
    /// `WorldJoin_EncounterType` (3 skirmish, 4 classic instance, 6 team).
    pub world_join_type: Option<u32>,
    /// `WorldJoin_EncounterCategory`.
    pub world_join_category: Option<u32>,
    /// `WorldJoin_EncounterTicket`.
    pub world_join_ticket: Option<Did>,
    /// `PrivateEncounterTemplate_EncounterType` (1 mission).
    pub encounter_type: Option<u32>,
    /// `PrivateEncounterTemplate_DistantBattleScript`.
    pub distant_battle_script: Option<Did>,
}

impl Template {
    /// Reads the template fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        let mut landblocks = Vec::new();
        let mut bad_landblocks = Vec::new();
        for e in v.elements(set, l::TPL_LANDBLOCKS) {
            match &e.value {
                PropertyValue::Vector3(raw) => match ContainedLandblock::from_vector(*raw) {
                    Some(b) => landblocks.push(b),
                    None => bad_landblocks.push(Some(*raw)),
                },
                _ => bad_landblocks.push(None),
            }
        }
        let mut awards: Vec<AwardEntry> = structs(v.elements(set, l::TPL_AWARD_ARRAY))
            .map(|s| award_entry(s, v))
            .collect();
        if let Some(s) = v
            .value(set, l::TPL_AWARD_ENTRY)
            .and_then(PropertyValue::as_struct)
        {
            awards.push(award_entry(s, v));
        }
        let text = |label| v.text(set, label).map(str::to_owned);
        Self {
            skirmish_class: v.has(set, l::SK_TIER_MIN),
            name: v.string_info(set, l::TPL_NAME).cloned(),
            description: v.string_info(set, l::TPL_DESCRIPTION).cloned(),
            content_layer: v.enum_key(set, l::TPL_CONTENT_LAYER),
            max_players: v.int(set, l::TPL_MAX_PLAYERS),
            public_instance: v.flag(set, l::TPL_PUBLIC_INSTANCE),
            public_max_player_property: v.pid_value(set, l::TPL_PUBLIC_MAX_PLAYER_PROP),
            lock_size: v.enum_key(set, l::TPL_LOCK_SIZE),
            time_dormant_limit: v.float(set, l::TPL_TIME_DORMANT),
            time_grace_limit: v.float(set, l::TPL_TIME_GRACE),
            linkdead_timeout_override: v.float(set, l::TPL_LINKDEAD_OVERRIDE),
            failure_timeout_override: v.float(set, l::TPL_FAILURE_OVERRIDE),
            length_default: v.int(set, l::TPL_LENGTH_DEFAULT),
            length_ripcord: v.did(set, l::TPL_LENGTH_RIPCORD),
            can_self_revive: v.flag(set, l::TPL_CAN_SELF_REVIVE),
            is_cinematic: v.flag(set, l::TPL_IS_CINEMATIC),
            dynamic_area_instance: v.flag(set, l::TPL_DYNAMIC_AREA),
            white_list: v.did(set, l::TPL_WHITE_LIST),
            places: TemplatePlaces {
                starting_position: text(l::TPL_STARTING_POSITION),
                success_location: text(l::TPL_SUCCESS_LOCATION),
                instance_milestone: text(l::TPL_INSTANCE_MILESTONE),
                external_milestone: text(l::TPL_EXTERNAL_MILESTONE),
                default_exit: text(l::TPL_DEFAULT_EXIT),
                exit_if_door: text(l::TPL_EXIT_IF_DOOR),
            },
            has_landblock_list: v.has(set, l::TPL_LANDBLOCKS),
            landblocks,
            bad_landblocks,
            load_all_landblocks: v.flag(set, l::TPL_LOAD_ALL),
            instance_property_count: v
                .value(set, l::TPL_INSTANCE_PROPERTIES)
                .and_then(PropertyValue::as_struct)
                .map(|s| s.fields.len()),
            instance_quest: v.did(set, l::TPL_INSTANCE_QUEST),
            tracks_instance_quest: v.flag(set, l::TPL_TRACKS_QUEST),
            quests_to_bestow: dids(v.elements(set, l::TPL_QUESTS_TO_BESTOW)),
            event_control: v.did(set, l::TPL_EVENT_CONTROL),
            member_count_event: v.did(set, l::TPL_MEMBER_COUNT_EVENT),
            property_response_map: v.did(set, l::TPL_RESPONSE_MAP),
            has_default_permission: v.has(set, l::TPL_DEFAULT_PERMISSION),
            has_creation_permission: v.has(set, l::TPL_CREATION_PERMISSION),
            teleport_scene: v.did(set, l::TPL_TELEPORT_SCENE),
            success_scene: v.did(set, l::TPL_SUCCESS_SCENE),
            intro_vo: v.did(set, l::TPL_INTRO_VO),
            sky_override: v.enum_key(set, l::TPL_SKY_OVERRIDE),
            disallowed_map_notes: match v.value(set, l::TPL_DISALLOWED_MAP_NOTES) {
                Some(PropertyValue::Bitfield64(b)) => Some(*b),
                _ => None,
            },
            species_loot_table: v.did(set, l::TPL_SPECIES_LOOT),
            treasure_level: v.int(set, l::TPL_TREASURE_LEVEL),
            defeat_info: v.did(set, l::TPL_DEFEAT_INFO),
            awards,
            tiers: enum_keys(v.elements(set, l::SK_TIER_ARRAY)),
            group_sizes: enum_keys(v.elements(set, l::SK_GROUP_SIZE_ARRAY)),
            group_size_override: v.enum_key(set, l::SK_GROUP_SIZE_OVERRIDE),
            tier_minimums: structs(v.elements(set, l::SK_TIER_MIN))
                .map(|s| TierConfiguration {
                    tier: v.enum_key(s, l::SK_DIFFICULTY_TIER),
                    group_size: v.enum_key(s, l::SK_GROUP_SIZE),
                })
                .collect(),
            min_level_scale: v.int(set, l::SK_MIN_LEVEL),
            max_level_scale: v.int(set, l::SK_MAX_LEVEL),
            level_scaling_level: v.int(set, l::SK_LEVEL_SCALING),
            tier_gate_property: v.pid_value(set, l::SK_TIER_GATE),
            highest_time_is_best: v.flag(set, l::SK_HIGHEST_TIME_BEST),
            world_join_type: v.enum_key(set, l::WJ_ENCOUNTER_TYPE),
            world_join_category: v.enum_key(set, l::WJ_CATEGORY),
            world_join_ticket: v.did(set, l::WJ_TICKET),
            encounter_type: v.enum_key(set, l::TPL_ENCOUNTER_TYPE),
            distant_battle_script: v.did(set, l::TPL_DISTANT_BATTLE_SCRIPT),
        }
    }

    /// The template kind of §4.1.
    #[must_use]
    pub fn kind(&self) -> TemplateKind {
        if !self.skirmish_class {
            TemplateKind::Ordinary
        } else if self.encounter_type == Some(1) {
            TemplateKind::Mission
        } else if self.world_join_category.is_some() {
            TemplateKind::WorldJoinEncounter
        } else {
            TemplateKind::OtherSkirmish
        }
    }

    /// Whether the template names a distant-battle script (a big battle, §4.1).
    #[must_use]
    pub fn is_big_battle(&self) -> bool {
        nonzero(self.distant_battle_script).is_some()
    }

    /// The landblocks without duplicates, first occurrence kept (§14.4 step 2).
    #[must_use]
    pub fn unique_landblocks(&self) -> Vec<(u32, u8, u8)> {
        let mut seen = BTreeSet::new();
        self.landblocks
            .iter()
            .map(ContainedLandblock::key)
            .filter(|k| seen.insert(*k))
            .collect()
    }

    /// Whether the list names a landblock twice (25 templates observed).
    #[must_use]
    pub fn has_duplicate_landblock(&self) -> bool {
        self.unique_landblocks().len() != self.landblocks.len()
    }

    /// Largest stored group size, if any.
    #[must_use]
    pub fn largest_group_size(&self) -> Option<u32> {
        self.group_sizes.iter().copied().max()
    }

    /// Record-local invariants N-I3 (without the DID-map check, see
    /// [`Template::content_layer_known`]), N-I4 and N-I5.
    #[must_use]
    pub fn deviations(&self) -> Vec<Deviation> {
        let mut out = Vec::new();
        if self.name.is_none() {
            out.push(Deviation::new("N-I3", "no name"));
        }
        if self.description.is_none() {
            out.push(Deviation::new("N-I3", "no description"));
        }
        if self.content_layer.is_none() {
            out.push(Deviation::new("N-I3", "no content layer"));
        }
        for b in &self.bad_landblocks {
            out.push(Deviation::new(
                "N-I4",
                match b {
                    Some(v) => format!("landblock vector {v:?}"),
                    None => "landblock list element is not a vector".to_owned(),
                },
            ));
        }
        if self.skirmish_class {
            let expected: Vec<u32> = (1..).take(self.tiers.len()).collect();
            if self.tiers != expected {
                out.push(Deviation::new(
                    "N-I5",
                    format!("tiers {:?} are not 1..n", self.tiers),
                ));
            }
            if let (Some(lo), Some(hi)) = (self.min_level_scale, self.max_level_scale)
                && lo > hi
            {
                out.push(Deviation::new("N-I5", format!("level scale {lo} > {hi}")));
            }
            for (k, c) in (1u32..).zip(&self.tier_minimums) {
                if c.tier != Some(k) {
                    out.push(Deviation::new(
                        "N-I5",
                        format!("tier-minimum entry {k} has tier {:?}", c.tier),
                    ));
                }
            }
            if let Some(max) = self.max_players {
                let largest = self.largest_group_size().map(i64::from);
                if largest != Some(i64::from(max)) {
                    out.push(Deviation::new(
                        "N-I5",
                        format!("MaxPlayers {max} vs largest group size {largest:?}"),
                    ));
                }
            }
        }
        out
    }

    /// N-I3's DID-map part: whether the content-layer key is a key of the `ContentLayer`
    /// map, given a membership test.
    pub fn content_layer_known(&self, is_key: impl Fn(u32) -> bool) -> bool {
        self.content_layer.is_some_and(is_key)
    }
}

fn award_entry(s: &PropertyStruct, v: &InstanceVocab) -> AwardEntry {
    AwardEntry {
        award_type: v.enum_key(s, l::TPL_AWARD_TYPE),
        ratios: structs(v.elements(s, l::TPL_RATIO_ARRAY))
            .map(|r| {
                (
                    v.did(r, l::TPL_AWARD_CURRENCY),
                    v.float(r, l::TPL_AWARD_RATIO),
                )
            })
            .collect(),
    }
}
