// SPDX-License-Identifier: GPL-3.0-or-later
//! UI and system rules (`game-controls.md` §7) and the guild control (§8.1).

use tessera_formats::{Did, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet, Value};

use super::account::distinct;
use crate::character::Fields;
use crate::error::Result;

/// The keys named by the set bits of a bitfield, by the bit-to-key rule
/// (`property-system.md` §7.3): bit *i* stands for key *i* + 1 (§1, §11.3).
#[must_use]
pub fn bit_keys(bits: u64) -> Vec<u32> {
    (0..64u32)
        .filter(|&i| bits & (1u64 << i) != 0)
        .map(|i| i + 1)
        .collect()
}

/// The override key a `VitalDisplayOverride_UnlockedVDOs` value unlocks (§7.1, §11.4):
/// the value must be a bitfield with exactly one bit set, bit *b* giving key *b* + 1.
/// `None` for another type, no bit or several bits.
#[must_use]
pub fn unlocked_key(value: &Value) -> Option<u32> {
    if value.set_bit_count() != 1 {
        return None;
    }
    value.iter_set_bits().next().and_then(|b| b.checked_add(1))
}

/// The top-level array of an item's property set that holds its vital-display unlock
/// (§7.1, GCI17): a general list of property values, so the unlock is taken by label.
pub const USAGE_PROPERTY_ARRAY: &str = "Usage_SetPropertyUsageData_Array";

/// The label of the unlock value (§7.1).
pub const VDO_UNLOCK_LABEL: &str = "VitalDisplayOverride_UnlockedVDOs";

/// The vital-display unlocks of an item (§7.1): every element labelled
/// [`VDO_UNLOCK_LABEL`] of the item's top-level [`USAGE_PROPERTY_ARRAY`], mapped by
/// [`unlocked_key`] (`None` for an element that is not a one-bit bitfield). The unlock is
/// never a top-level property and never inside a struct; elements with other labels (a
/// quest property in one set of the install) are skipped. Empty when the item has no
/// such array or the dictionary lacks either label.
///
/// # Errors
///
/// [`crate::WStateError::BadProperty`] when the array property is not an array,
/// [`crate::WStateError::TooMany`] past the view limit.
pub fn item_vdo_unlocks(set: &PropertySet, reg: &PropertyRegistry) -> Result<Vec<Option<u32>>> {
    let Some(pid) = reg.pid(VDO_UNLOCK_LABEL) else {
        return Ok(Vec::new());
    };
    let f = Fields::new(set, reg, "item usage properties");
    Ok(f.list("Usage_SetPropertyUsageData_Array")?
        .iter()
        .filter(|p| p.pid == pid)
        .map(|p| unlocked_key(&p.value))
        .collect())
}

/// gmVitalDisplayOverrideControl (§7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VitalDisplayOverrideControl {
    /// `VitalDisplayOverride_Array` (non-null DIDs).
    pub overrides: Vec<Did>,
}

impl VitalDisplayOverrideControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "vital-display override control");
        Ok(Self {
            overrides: f.did_list("VitalDisplayOverride_Array")?,
        })
    }
}

/// A vital-display override (class 3463 here, §7.1). Where the chosen override is
/// stored and how its UI state changes the display is OPEN (Q13).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VitalDisplayOverride {
    /// `VitalDisplayOverride_RequiredEnum`: the override's key.
    pub key: Option<u32>,
    /// `VitalDisplayOverride_UIState`.
    pub ui_state: Option<u32>,
    /// `VitalDisplayOverride_CSM_Name`.
    pub name: Option<StringInfo>,
    /// `VitalDisplayOverride_IsForFreepCharacters`.
    pub for_free_peoples: Option<bool>,
    /// `VitalDisplayOverride_IsForMonsterCharacters`.
    pub for_monsters: Option<bool>,
    /// `VitalDisplayOverride_IsForPvPCharacters`.
    pub for_pvp: Option<bool>,
}

impl VitalDisplayOverride {
    /// Reads an override's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "vital-display override");
        Ok(Self {
            key: f.enum_("VitalDisplayOverride_RequiredEnum")?,
            ui_state: f.enum_("VitalDisplayOverride_UIState")?,
            name: f.string_info("VitalDisplayOverride_CSM_Name")?,
            for_free_peoples: f.boolean("VitalDisplayOverride_IsForFreepCharacters")?,
            for_monsters: f.boolean("VitalDisplayOverride_IsForMonsterCharacters")?,
            for_pvp: f.boolean("VitalDisplayOverride_IsForPvPCharacters")?,
        })
    }
}

/// AlertsControl (§7.3). The overflow behaviour is OPEN (Q14).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlertsControl {
    /// `AlertControl_MaxNumberOfBigAlerts`.
    pub max_big_alerts: Option<i32>,
    /// `AlertControl_TypeToOverflow_Array`: (`Alert_Type`, `Alert_IsOverflow`).
    pub overflow: Vec<(Option<u32>, Option<bool>)>,
}

impl AlertsControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "alerts control");
        Ok(Self {
            max_big_alerts: f.int("AlertControl_MaxNumberOfBigAlerts")?,
            overflow: f.struct_list("AlertControl_TypeToOverflow_Array", |s| {
                Ok((s.enum_("Alert_Type")?, s.boolean("Alert_IsOverflow")?))
            })?,
        })
    }

    /// GCI11: the alert types are pairwise distinct and non-zero.
    #[must_use]
    pub fn types_are_distinct_and_non_zero(&self) -> bool {
        distinct(self.overflow.iter().map(|(t, _)| t.filter(|&k| k != 0)))
    }
}

/// One `ResetScheduleControl_ResetScheduleEntry` (§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResetSchedule {
    /// `ResetScheduleControl_ResetSchedule`.
    pub schedule: Option<u32>,
    /// `ResetScheduleControl_ResetDays` (bitfield over the DayOfWeek mapper).
    pub days: Option<u64>,
}

impl ResetSchedule {
    /// The DayOfWeek keys of the reset days, by the bit-to-key rule (§11.3). The time
    /// of day and time zone are OPEN (Q15).
    #[must_use]
    pub fn day_keys(&self) -> Vec<u32> {
        self.days.map(bit_keys).unwrap_or_default()
    }
}

/// ResetScheduleControl (§7.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResetScheduleControl {
    /// `ResetScheduleControl_ResetScheduleArray`.
    pub schedules: Vec<ResetSchedule>,
}

impl ResetScheduleControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "reset-schedule control");
        Ok(Self {
            schedules: f.struct_list("ResetScheduleControl_ResetScheduleArray", |s| {
                Ok(ResetSchedule {
                    schedule: s.enum_("ResetScheduleControl_ResetSchedule")?,
                    days: s.bits("ResetScheduleControl_ResetDays")?,
                })
            })?,
        })
    }

    /// The entry of schedule key `schedule`.
    #[must_use]
    pub fn get(&self, schedule: u32) -> Option<&ResetSchedule> {
        self.schedules.iter().find(|s| s.schedule == Some(schedule))
    }
}

/// ConsiderationControl (§7.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConsiderationControl {
    /// `ConsiderationControl_RankToUIState`: (`ConsiderationControl_ConsiderRank`,
    /// `ConsiderationControl_UIStateID`).
    pub rank_to_ui_state: Vec<(Option<u32>, Option<u32>)>,
    /// `ConsiderationControl_CombatTableData`: (`ConsiderationControl_ConsiderType`,
    /// `ConsiderationControl_CombatTableList`, null elements kept).
    pub combat_tables: Vec<(Option<u32>, Vec<Did>)>,
    /// `ConsiderationControl_QuestTableList` (null elements kept).
    pub quest_tables: Vec<Did>,
}

impl ConsiderationControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "consideration control");
        Ok(Self {
            rank_to_ui_state: f.struct_list("ConsiderationControl_RankToUIState", |s| {
                Ok((
                    s.enum_("ConsiderationControl_ConsiderRank")?,
                    s.enum_("ConsiderationControl_UIStateID")?,
                ))
            })?,
            combat_tables: f.struct_list("ConsiderationControl_CombatTableData", |s| {
                Ok((
                    s.enum_("ConsiderationControl_ConsiderType")?,
                    s.map_list("ConsiderationControl_CombatTableList", Value::as_did)?,
                ))
            })?,
            quest_tables: f.map_list("ConsiderationControl_QuestTableList", Value::as_did)?,
        })
    }

    /// The UI state of rank `rank` through `RankToUIState`.
    #[must_use]
    pub fn ui_state(&self, rank: u32) -> Option<u32> {
        self.rank_to_ui_state
            .iter()
            .find(|(r, _)| *r == Some(rank))
            .and_then(|&(_, s)| s)
    }

    /// The table list of consider type `consider_type`.
    #[must_use]
    pub fn combat_list(&self, consider_type: u32) -> Option<&[Did]> {
        self.combat_tables
            .iter()
            .find(|(t, _)| *t == Some(consider_type))
            .map(|(_, l)| l.as_slice())
    }

    /// Element `level` (counted from 1) of a table list: the table for that level by
    /// the data-consistent reading of §7.5 (which level selects it is OPEN, Q16).
    #[must_use]
    pub fn table_for_level(list: &[Did], level: u32) -> Option<Did> {
        let i = usize::try_from(level.checked_sub(1)?).ok()?;
        list.get(i).copied()
    }
}

/// A consideration table (classes 1694 and 1697 here, §7.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConsiderationTable {
    /// `ConsiderationTable_RankPerDifferentialArray`: ConsiderRank keys.
    pub ranks: Vec<u32>,
}

impl ConsiderationTable {
    /// The index of the level difference 0 in the observed 31-element tables.
    pub const EVEN_INDEX: usize = 15;
    /// The observed table length.
    pub const LEN: usize = 31;

    /// Reads a table's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "consideration table");
        Ok(Self {
            ranks: f.enum_list("ConsiderationTable_RankPerDifferentialArray")?,
        })
    }

    /// GCI12: 31 ranks, non-decreasing, with `even` (the Even key of the ConsiderRank
    /// mapper) at index 15.
    #[must_use]
    pub fn is_well_formed(&self, even: u32) -> bool {
        self.ranks.len() == Self::LEN
            && self.ranks.windows(2).all(|w| matches!(w, [a, b] if a <= b))
            && self.ranks.get(Self::EVEN_INDEX) == Some(&even)
    }

    /// The rank for level difference `d` = target − viewer by the data-consistent
    /// reading of §7.5: index *d* + 15, clamped to 0–30 (the index rule is OPEN, Q16).
    #[must_use]
    pub fn rank_for_difference(&self, d: i64) -> Option<u32> {
        let i = d.saturating_add(15).clamp(0, 30);
        self.ranks.get(usize::try_from(i).ok()?).copied()
    }
}

/// TrackingStateControl (§7.10). What a tracking state changes is OPEN (Q26).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackingStateControl {
    /// `TrackingStateControl_MapNoteToState_Array`: (`TrackingStateControl_MapNote`
    /// bits, `TrackingStateControl_TrackingState`).
    pub entries: Vec<(Option<u64>, Option<u32>)>,
}

impl TrackingStateControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "tracking-state control");
        Ok(Self {
            entries: f.struct_list("TrackingStateControl_MapNoteToState_Array", |s| {
                Ok((
                    s.bits("TrackingStateControl_MapNote")?,
                    s.enum_("TrackingStateControl_TrackingState")?,
                ))
            })?,
        })
    }

    /// GCI13: the tracking states are pairwise distinct.
    #[must_use]
    pub fn states_are_distinct(&self) -> bool {
        distinct(self.entries.iter().map(|&(_, s)| s))
    }
}

/// DiscoveryDistanceControl (§7.9). What is classified is OPEN (Q20).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiscoveryDistanceControl {
    /// (`DiscoveryDistanceControl_DistanceEnum`,
    /// `DiscoveryDistanceControl_DistanceThreshold`).
    pub thresholds: Vec<(Option<u32>, Option<f32>)>,
}

impl DiscoveryDistanceControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "discovery-distance control");
        Ok(Self {
            thresholds: f.struct_list("DiscoveryDistanceControl_DistanceEnum_Array", |s| {
                Ok((
                    s.enum_("DiscoveryDistanceControl_DistanceEnum")?,
                    s.float("DiscoveryDistanceControl_DistanceThreshold")?,
                ))
            })?,
        })
    }
}

/// CombatMusicControl (§7.8). The intensity formula is OPEN (Q19).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CombatMusicControl {
    /// `CombatMusicControl_Floor`.
    pub floor: Option<i32>,
    /// `CombatMusicControl_Ceiling`.
    pub ceiling: Option<i32>,
    /// `CombatMusicControl_ModArray`: (`CombatMusicControl_ModStatEnum`,
    /// `CombatMusicControl_ModStatAddModifier`).
    pub modifiers: Vec<(Option<u32>, Option<i32>)>,
}

impl CombatMusicControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "combat-music control");
        Ok(Self {
            floor: f.int("CombatMusicControl_Floor")?,
            ceiling: f.int("CombatMusicControl_Ceiling")?,
            modifiers: f.struct_list("CombatMusicControl_ModArray", |s| {
                Ok((
                    s.enum_("CombatMusicControl_ModStatEnum")?,
                    s.int("CombatMusicControl_ModStatAddModifier")?,
                ))
            })?,
        })
    }
}

/// AggroPingControl (§7.7). Meaning and units are OPEN (Q18).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AggroPingControl {
    /// `AggroPingControl_AggroPingFrequency`.
    pub frequency: Option<f32>,
    /// `AggroPingControl_ChallengeLevelWindow`.
    pub challenge_level_window: Option<i32>,
}

impl AggroPingControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "aggro-ping control");
        Ok(Self {
            frequency: f.float("AggroPingControl_AggroPingFrequency")?,
            challenge_level_window: f.int("AggroPingControl_ChallengeLevelWindow")?,
        })
    }
}

/// OpenTappingControl (§7.15; server rule).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OpenTappingControl {
    /// `OpenTapping_MaxCreditDistance`.
    pub max_credit_distance: Option<f32>,
}

impl OpenTappingControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "open-tapping control");
        Ok(Self {
            max_credit_distance: f.float("OpenTapping_MaxCreditDistance")?,
        })
    }
}

/// The player-note length the client uses when GuildControl does not hold
/// `Guild_PlayerNoteMaxLength` as an int: absent, or present with another type (§8.1,
/// BIN q5).
pub const PLAYER_NOTE_DEFAULT: i32 = 48;

/// GuildControl (§8.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuildControl {
    /// `Guild_MaxMembers`.
    pub max_members: Option<i32>,
    /// `Guild_GuildNameMaxLength`.
    pub name_max_length: Option<i32>,
    /// `Guild_RankNameMaxLength`.
    pub rank_name_max_length: Option<i32>,
    /// `Guild_MotDMaxLength`.
    pub motd_max_length: Option<i32>,
    /// `Guild_MaxGuildCalendarEvents`.
    pub max_calendar_events: Option<i32>,
    /// `Guild_RecruitmentTimeOut`.
    pub recruitment_timeout: Option<f32>,
    /// `Guild_RankList`: (`Guild_RankID`, `Guild_RankLevel`, `Guild_RankPermissions`).
    pub ranks: Vec<(Option<i32>, Option<i32>, Option<u64>)>,
    /// `Guild_DefaultTheme`.
    pub default_theme: Option<Did>,
    /// `Guild_RecreateGuildCharterDID`.
    pub charter: Option<Did>,
    /// `Guild_RecreateMPGuildCharterDID`.
    pub monster_play_charter: Option<Did>,
    /// `Guild_PlayerNoteMaxLength` as stored (an int; absent in the spec's install). A
    /// value of another type counts as absent (an implementer choice, §8.1 OPEN note).
    pub player_note_max_length: Option<i32>,
}

impl GuildControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "guild control");
        Ok(Self {
            max_members: f.int("Guild_MaxMembers")?,
            name_max_length: f.int("Guild_GuildNameMaxLength")?,
            rank_name_max_length: f.int("Guild_RankNameMaxLength")?,
            motd_max_length: f.int("Guild_MotDMaxLength")?,
            max_calendar_events: f.int("Guild_MaxGuildCalendarEvents")?,
            recruitment_timeout: f.float("Guild_RecruitmentTimeOut")?,
            ranks: f.struct_list("Guild_RankList", |s| {
                Ok((
                    s.int("Guild_RankID")?,
                    s.int("Guild_RankLevel")?,
                    s.bits("Guild_RankPermissions")?,
                ))
            })?,
            default_theme: f.did("Guild_DefaultTheme")?,
            charter: f.did("Guild_RecreateGuildCharterDID")?,
            monster_play_charter: f.did("Guild_RecreateMPGuildCharterDID")?,
            player_note_max_length: f.get("Guild_PlayerNoteMaxLength").and_then(Value::as_i32),
        })
    }

    /// The player-note length limit the client uses: the stored value when it is an int
    /// (property type 15), else [`PLAYER_NOTE_DEFAULT`] (§8.1).
    #[must_use]
    pub fn player_note_limit(&self) -> i32 {
        self.player_note_max_length.unwrap_or(PLAYER_NOTE_DEFAULT)
    }
}
