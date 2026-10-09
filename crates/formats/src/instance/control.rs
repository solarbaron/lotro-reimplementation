// SPDX-License-Identifier: GPL-3.0-or-later
//! Control records and small classes of `instances.md`: the private-encounter control
//! (§5.1) and entry filters (§5.2), skirmish match parameters (§6.1) and trainer trait
//! lists (§6.2), the world-join control (§7.1) and tickets (§7.2), battle statistics
//! (§8.7), the session-play control (§9), the fellowship-maker control (§10.1) and the
//! phasing flags (§11).

use std::collections::BTreeSet;

use super::labels as l;
use super::{Deviation, Fields, InstanceVocab, dids, enum_keys, first_of_type, structs};
use crate::did::Did;
use crate::property::{PropertyType, PropertyValue};
use crate::property_set::PropertySet;

// ------------------------------------------------------------------------ §5.1

/// One lock rule of `PrivateEncounter_Control_LockArray` (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LockRule {
    /// `PrivateEncounter_Control_LockSize` (lock kind 1–5).
    pub size: Option<u32>,
    /// `PrivateEncounter_Control_LockResetDays`: bit *k* − 1 is `DayOfWeek` key *k*.
    pub reset_days: Option<u32>,
}

impl LockRule {
    /// Whether the lock resets on `DayOfWeek` key `day` (1 Sunday … 7 Saturday).
    #[must_use]
    pub fn resets_on(&self, day: u32) -> bool {
        (1..=32).contains(&day)
            && self
                .reset_days
                .is_some_and(|bits| bits & (1u32 << (day - 1)) != 0)
    }

    /// The `DayOfWeek` keys of the reset days, ascending (keys 1–7 only).
    #[must_use]
    pub fn reset_day_keys(&self) -> Vec<u32> {
        (1..=7).filter(|&d| self.resets_on(d)).collect()
    }
}

/// The private-encounter control record (§5.1, label `PrivateEncounterControl`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EncounterControl {
    /// `PrivateEncounter_DefaultEncounterTimeout`.
    pub default_encounter_timeout: Option<f32>,
    /// `PrivateEncounter_DefaultLinkdeadTimeout`.
    pub default_linkdead_timeout: Option<f32>,
    /// `PrivateEncounter_LeaveInstanceInduction`.
    pub leave_induction: Option<Did>,
    /// Lock rules, in stored order.
    pub locks: Vec<LockRule>,
    /// `PrivateEncounter_Control_LockableEncounterArray`.
    pub lockable: Vec<Did>,
    /// `PrivateEncounter_Control_RestrictMaxPlayersExclusionArray`.
    pub restrict_max_players_exclusions: Vec<Did>,
    /// `PrivateEncounter_Control_PvMP_Challenge_Encounter_Array`.
    pub pvmp_challenge: Vec<Did>,
    /// `PrivateEncounter_EntryFilters`.
    pub entry_filters: Vec<Did>,
    /// `PrivateEncounter_EntryFilters_IgnoredEncounters`.
    pub filter_ignored: Vec<Did>,
}

impl EncounterControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            default_encounter_timeout: v.float(set, l::PE_DEFAULT_TIMEOUT),
            default_linkdead_timeout: v.float(set, l::PE_DEFAULT_LINKDEAD),
            leave_induction: v.did(set, l::PE_LEAVE_INDUCTION),
            locks: structs(v.elements(set, l::PE_LOCK_ARRAY))
                .map(|s| LockRule {
                    size: v.enum_key(s, l::PE_LOCK_SIZE),
                    reset_days: match v.value(s, l::PE_LOCK_RESET_DAYS) {
                        Some(PropertyValue::Bitfield32(b)) => Some(*b),
                        _ => None,
                    },
                })
                .collect(),
            lockable: dids(v.elements(set, l::PE_LOCKABLE)),
            restrict_max_players_exclusions: dids(v.elements(set, l::PE_RESTRICT_EXCLUSIONS)),
            pvmp_challenge: dids(v.elements(set, l::PE_PVMP)),
            entry_filters: dids(v.elements(set, l::PE_ENTRY_FILTERS)),
            filter_ignored: dids(v.elements(set, l::PE_FILTER_IGNORED)),
        }
    }

    /// The rule of lock kind `size`, if listed.
    #[must_use]
    pub fn lock(&self, size: u32) -> Option<&LockRule> {
        self.locks.iter().find(|r| r.size == Some(size))
    }

    /// N-I6: the lockable list equals the set of templates that carry `LockSize`.
    #[must_use]
    pub fn check_lockable(&self, templates_with_lock_size: &BTreeSet<Did>) -> Vec<Deviation> {
        let listed: BTreeSet<Did> = self.lockable.iter().copied().collect();
        let mut out = Vec::new();
        for d in listed.difference(templates_with_lock_size) {
            out.push(Deviation::new(
                "N-I6",
                format!("{:#010x} listed without LockSize", d.0),
            ));
        }
        for d in templates_with_lock_size.difference(&listed) {
            out.push(Deviation::new(
                "N-I6",
                format!("{:#010x} has LockSize but is not listed", d.0),
            ));
        }
        out
    }
}

// ------------------------------------------------------------------------ §5.2

/// The property form of an entry filter (§5.2). The window and limit numbers live only in
/// the object form and are not read here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryFilter {
    /// Whether `PrivateEncounter_EntryFilter_RejectMessage` is stored.
    pub has_reject_message: bool,
    /// `PrivateEncounter_EntryFilter_RejectErrorType` (mapper `ErrorType`).
    pub reject_error_type: Option<u32>,
    /// `PrivateEncounter_EntryFilter_Alert` (mapper `Alert`).
    pub alert: Option<u32>,
}

impl EntryFilter {
    /// Reads the filter fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            has_reject_message: v.has(set, l::EF_REJECT_MESSAGE),
            reject_error_type: v.enum_key(set, l::EF_REJECT_ERROR),
            alert: v.enum_key(set, l::EF_ALERT),
        }
    }
}

// ------------------------------------------------------------------------ §6.1

/// Skirmish match parameters (§6.1): inclusive level, group-size and tier bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct MatchParams {
    /// `SkirmishMatchParams_Level_Min` / `…_Level_Max`.
    pub level: (Option<i32>, Option<i32>),
    /// `SkirmishMatchParams_GroupSize_Min` / `…_Max`.
    pub group_size: (Option<u32>, Option<u32>),
    /// `SkirmishMatchParams_DifficultyTier_Min` / `…_Max`.
    pub tier: (Option<u32>, Option<u32>),
}

fn within<T: PartialOrd + Copy>(band: (Option<T>, Option<T>), x: T) -> bool {
    band.0.is_none_or(|lo| lo <= x) && band.1.is_none_or(|hi| x <= hi)
}

fn bands_overlap<T: PartialOrd + Copy>(
    a: (Option<T>, Option<T>),
    b: (Option<T>, Option<T>),
) -> bool {
    let lo_ok = match (a.0, b.1) {
        (Some(lo), Some(hi)) => lo <= hi,
        _ => true,
    };
    let hi_ok = match (b.0, a.1) {
        (Some(lo), Some(hi)) => lo <= hi,
        _ => true,
    };
    lo_ok && hi_ok
}

impl MatchParams {
    /// Reads the bands of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            level: (v.int(set, l::MP_LEVEL_MIN), v.int(set, l::MP_LEVEL_MAX)),
            group_size: (
                v.enum_key(set, l::MP_GROUP_MIN),
                v.enum_key(set, l::MP_GROUP_MAX),
            ),
            tier: (
                v.enum_key(set, l::MP_TIER_MIN),
                v.enum_key(set, l::MP_TIER_MAX),
            ),
        }
    }

    /// Whether a run of this level, group size and tier falls inside all three bands
    /// (bounds inclusive, Derived in §6.1). An absent bound does not limit.
    #[must_use]
    pub fn matches(&self, level: i32, group_size: u32, tier: u32) -> bool {
        within(self.level, level) && within(self.group_size, group_size) && within(self.tier, tier)
    }

    /// Whether the level bands of `self` and `other` overlap.
    #[must_use]
    pub fn level_overlaps(&self, other: &Self) -> bool {
        bands_overlap(self.level, other.level)
    }

    /// Whether the tier bands overlap.
    #[must_use]
    pub fn tier_overlaps(&self, other: &Self) -> bool {
        bands_overlap(self.tier, other.tier)
    }

    /// Whether the group-size bands overlap.
    #[must_use]
    pub fn group_overlaps(&self, other: &Self) -> bool {
        bands_overlap(self.group_size, other.group_size)
    }
}

// ------------------------------------------------------------------------ §6.2

/// A skirmish skill-trainer trait list (§6.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillTrainerTraits {
    /// `Skirmish_SkillTrainer_AllowedTraits` (trait DIDs, stored order).
    pub traits: Vec<Did>,
}

impl SkillTrainerTraits {
    /// Reads the list of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            traits: dids(v.elements(set, l::ST_ALLOWED_TRAITS)),
        }
    }
}

// ------------------------------------------------------------------------ §7.1

/// One `WorldJoinControl_Instance_Grouping` (§7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstanceGroup {
    /// `WorldJoinControl_Instance_Group_Label` (mapper `WJInstanceGroup`).
    pub label: Option<u32>,
    /// `WorldJoinControl_Grouped_Instance_List`, stored order.
    pub templates: Vec<Did>,
}

/// The world-join control (§7.1, label `WorldJoinControl`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldJoinControl {
    /// `WorldJoin_AvailablePETemplates_Array`, stored order.
    pub available: Vec<Did>,
    /// `WorldJoinControl_InstanceGroups`, stored order (not label order).
    pub groups: Vec<InstanceGroup>,
    /// `WorldJoinControl_FeaturedInstance_Array` elements; `None` for an element that is
    /// not a DID (§15.1 B1 treats it as "no featured instance").
    pub featured: Vec<Option<Did>>,
    /// `WorldJoinControl_FeaturedInstance_WorldEvent`.
    pub featured_world_event: Option<Did>,
    /// `WorldJoinControl_DefaultTicket`.
    pub default_ticket: Option<Did>,
    /// `WorldJoinControl_MinLevel`.
    pub min_level: Option<i32>,
    /// `WorldJoin_Seasonal_EncounterCategory_Array`.
    pub seasonal: Vec<u32>,
    /// `WorldJoin_Capped_EncounterCategory`.
    pub capped: Option<u32>,
}

impl WorldJoinControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            available: dids(v.elements(set, l::WJC_AVAILABLE)),
            groups: structs(v.elements(set, l::WJC_GROUPS))
                .map(|s| InstanceGroup {
                    label: v.enum_key(s, l::WJC_GROUP_LABEL),
                    templates: dids(v.elements(s, l::WJC_GROUP_LIST)),
                })
                .collect(),
            featured: v
                .elements(set, l::WJC_FEATURED)
                .iter()
                .map(|e| e.value.as_did())
                .collect(),
            featured_world_event: v.did(set, l::WJC_FEATURED_EVENT),
            default_ticket: v.did(set, l::WJC_DEFAULT_TICKET),
            min_level: v.int(set, l::WJC_MIN_LEVEL),
            seasonal: enum_keys(v.elements(set, l::WJC_SEASONAL)),
            capped: v.enum_key(set, l::WJC_CAPPED),
        }
    }

    /// Every grouped template, in stored group order.
    pub fn grouped(&self) -> impl Iterator<Item = Did> + '_ {
        self.groups.iter().flat_map(|g| g.templates.iter().copied())
    }

    /// N-I7 (featured ⊆ available, grouped ∩ available = ∅) and the §7.1 observation that
    /// the available list has no duplicates.
    #[must_use]
    pub fn deviations(&self) -> Vec<Deviation> {
        let available: BTreeSet<Did> = self.available.iter().copied().collect();
        let mut out = Vec::new();
        if available.len() != self.available.len() {
            out.push(Deviation::new("N-I7", "available list has duplicates"));
        }
        for f in &self.featured {
            match f {
                Some(d) if available.contains(d) => {}
                Some(d) => out.push(Deviation::new(
                    "N-I7",
                    format!("featured {:#010x} not available", d.0),
                )),
                None => out.push(Deviation::new("N-I7", "featured element is not a DID")),
            }
        }
        for g in self.grouped() {
            if available.contains(&g) {
                out.push(Deviation::new(
                    "N-I7",
                    format!("grouped {:#010x} is also available", g.0),
                ));
            }
        }
        out
    }
}

// ------------------------------------------------------------------------ §7.2

/// A world-join ticket (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorldJoinTicket {
    /// `WorldJoinTicket_MaxEncounters_RipCord`.
    pub max_encounters: Option<Did>,
    /// `WorldJoinTicket_MinTimeBetweenLaunches_RipCord`.
    pub min_time_between_launches: Option<Did>,
}

impl WorldJoinTicket {
    /// Reads the ticket fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            max_encounters: v.did(set, l::TK_MAX_ENCOUNTERS),
            min_time_between_launches: v.did(set, l::TK_MIN_TIME),
        }
    }
}

// ------------------------------------------------------------------------ §8.7

/// A battle statistic (§8.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleStat {
    /// Whether `BattleStat_Name` is stored.
    pub has_name: bool,
    /// Whether `BattleStat_Description` is stored.
    pub has_description: bool,
    /// `BattleStat_Scope` (mapper `BattleStatScopeType`).
    pub scope: Option<u32>,
    /// `BattleStat_UpdatePolicy` (1 set, 2 add, 3 minimum, 4 maximum).
    pub update_policy: Option<u32>,
    /// `BattleStat_ReplicateToClients` (stored true or false).
    pub replicate_to_clients: Option<bool>,
    /// `BattleStat_ForwardUpdatesToStat`.
    pub forward_to: Option<Did>,
}

impl BattleStat {
    /// Reads the statistic fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            has_name: v.has(set, l::BS_NAME),
            has_description: v.has(set, l::BS_DESCRIPTION),
            scope: v.enum_key(set, l::BS_SCOPE),
            update_policy: v.enum_key(set, l::BS_POLICY),
            replicate_to_clients: v.boolean(set, l::BS_REPLICATE),
            forward_to: v.did(set, l::BS_FORWARD),
        }
    }
}

// ------------------------------------------------------------------------ §9

/// The session-play control (§9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionPlayControl {
    /// (`SessionPlay_DID`, `SessionPlay_AllowedSlotCount`) of each list entry.
    pub slots: Vec<(Option<Did>, Option<i32>)>,
}

impl SessionPlayControl {
    /// Reads the slot list of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            slots: structs(v.elements(set, l::SP_LIST))
                .map(|s| (v.did(s, l::SP_DID), v.int(s, l::SP_COUNT)))
                .collect(),
        }
    }
}

// ------------------------------------------------------------------------ §10.1

/// The fellowship-maker control, property form (§10.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FellowshipMakerControl {
    /// `FellowshipMakerControl_MinLevel`.
    pub min_level: Option<i32>,
    /// `FellowshipMakerControl_ValidEncounterSizeArray` (group-size keys, stored order).
    pub valid_sizes: Vec<u32>,
    /// Match-time entries: (encounter-size key, world-property PID). The struct fields are
    /// told apart by type (one enum, one PID).
    pub match_time: Vec<(Option<u32>, Option<u32>)>,
    /// Bonus thresholds: (effect, threshold), told apart by type (one DID, one float).
    pub bonus_thresholds: Vec<(Option<Did>, Option<f32>)>,
    /// `FellowshipMakerControl_RandomBonusEffect`.
    pub random_bonus_effect: Option<Did>,
    /// `FellowshipMakerControl_DeserterEffect`.
    pub deserter_effect: Option<Did>,
    /// `FellowshipMakerControl_ExcludedEncounterArray`.
    pub excluded: Vec<Did>,
}

impl FellowshipMakerControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            min_level: v.int(set, l::FM_MIN_LEVEL),
            valid_sizes: enum_keys(v.elements(set, l::FM_SIZES)),
            match_time: structs(v.elements(set, l::FM_MATCH_TIME))
                .map(|s| {
                    (
                        match first_of_type(s, PropertyType::Enum, None) {
                            Some(PropertyValue::Enum(k)) => Some(*k),
                            _ => None,
                        },
                        match first_of_type(s, PropertyType::PropertyId, None) {
                            Some(PropertyValue::PropertyId(p)) => Some(*p),
                            _ => None,
                        },
                    )
                })
                .collect(),
            bonus_thresholds: structs(v.elements(set, l::FM_BONUS))
                .map(|s| {
                    (
                        first_of_type(s, PropertyType::DidRef, None)
                            .and_then(PropertyValue::as_did),
                        first_of_type(s, PropertyType::Float, None).and_then(PropertyValue::as_f32),
                    )
                })
                .collect(),
            random_bonus_effect: v.did(set, l::FM_RANDOM_BONUS),
            deserter_effect: v.did(set, l::FM_DESERTER),
            excluded: dids(v.elements(set, l::FM_EXCLUDED)),
        }
    }
}

// ------------------------------------------------------------------------ §11

/// The phasing flags of a box or entity (§11, §15.1 B6). They apply to placed content and
/// NPCs as well as to the phasing-box record; [`PhasingFlags::read_fields`] reads them
/// from any property set or struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhasingFlags {
    /// `PhasingBox_UpdateOnlyOnCreation`: evaluated once, not registered for updates.
    pub update_only_on_creation: bool,
    /// `PhasingBox_DelayRefreshOfLinkedEntities`: linked entities are not refreshed at once.
    pub delay_refresh: bool,
    /// `Phasing_ForcePhasingRefresh_OnEnterWorld`: entering the world runs a refresh.
    pub force_refresh_on_enter: bool,
}

impl PhasingFlags {
    /// Reads the flags of a record's property set.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self::read_fields(set, v)
    }

    /// Reads the flags of any property set or struct (absent = false).
    #[must_use]
    pub fn read_fields<F: Fields + ?Sized>(f: &F, v: &InstanceVocab) -> Self {
        Self {
            update_only_on_creation: v.flag(f, l::PB_UPDATE_ONLY),
            delay_refresh: v.flag(f, l::PB_DELAY),
            force_refresh_on_enter: v.flag(f, l::PH_FORCE_REFRESH),
        }
    }

    /// Whether the box is registered for later updates (B6: not when it updates only on
    /// creation).
    #[must_use]
    pub fn registers_for_updates(&self) -> bool {
        !self.update_only_on_creation
    }

    /// Whether a change of the box's state refreshes the linked entities at once (B6).
    #[must_use]
    pub fn refreshes_immediately(&self) -> bool {
        !self.delay_refresh
    }
}
