// SPDX-License-Identifier: GPL-3.0-or-later
//! Big and distant battles (`instances.md` §8): the big-battle control (§8.1),
//! distant-battle scripts (§8.3), distant-battle units with the client's display rules
//! (§8.4, §15.1 B5) and the formation control (§8.5).

use std::collections::BTreeMap;

use super::labels as l;
use super::{Deviation, InstanceVocab, dids, enum_keys, nonzero, structs};
use crate::did::Did;
use crate::property::{PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;

/// `DistantBattleUnit_CaptureQualityOverride` when absent: automatic (B5).
pub const CAPTURE_QUALITY_AUTOMATIC: u32 = 0xFFFF_FFFF;
/// `DistantBattleUnit_LaneType` when absent: spawn (§3.3).
pub const LANE_TYPE_DEFAULT: u32 = 2;

// ------------------------------------------------------------------------ §8.1

/// One `BigBattleControl_Medal_Info_Struct` (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MedalInfo {
    /// `BigBattleControl_Medal` (1 lowest … 5 highest).
    pub medal: Option<u32>,
    /// `BigBattleControl_TraitPointsToGrant`.
    pub trait_points: Option<i32>,
    /// `BigBattleControl_CashOut_PointsRequired` (−1 = none).
    pub cash_out_points_required: Option<i32>,
    /// `BigBattleControl_CashOut_CurrentPoints` (a PID).
    pub cash_out_current_points: Option<u32>,
    /// `BigBattleControl_Medal_BonusProp` (a PID).
    pub bonus_property: Option<u32>,
    /// `BigBattleControl_FTT_ForCashOut_Loot`.
    pub cash_out_loot: Option<Did>,
    /// `BigBattleControl_Medal_Alert`.
    pub alert: Option<u32>,
}

/// The big-battle control (§8.1, label `BigBattlesControl`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BigBattleControl {
    /// `BigBattleControl_InitialTraitPoints`.
    pub initial_trait_points: Option<i32>,
    /// `BigBattleControl_Traits_RefundCostByLevel_Progression`.
    pub refund_progression: Option<Did>,
    /// `BigBattleControl_Rewards_FirstTimeMedal_Bonus`.
    pub first_time_medal_bonus: Option<i32>,
    /// `BigBattleControl_Medal_Info_Hash`, stored order.
    pub medals: Vec<MedalInfo>,
    /// (`BigBattleControl_Quest`, `BigBattleControl_QuestPointValue`) of
    /// `BigBattleControl_Quest_RewardValue_Hash`.
    pub quest_points: Vec<(Option<Did>, Option<i32>)>,
    /// (`BigBattleControl_Quest`, `BigBattleControl_Encounter`) of
    /// `BigBattleControl_Quest_To_Encounter_Hash`.
    pub quest_encounters: Vec<(Option<Did>, Option<Did>)>,
    /// (`BigBattleControl_BigBattleWaveLocation`, `BigBattleControl_LocationZoneEvent`) of
    /// `BigBattleControl_LocationToZoneEvent_Array`.
    pub location_events: Vec<(Option<u32>, Option<Did>)>,
}

impl BigBattleControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            initial_trait_points: v.int(set, l::BB_INITIAL_POINTS),
            refund_progression: v.did(set, l::BB_REFUND),
            first_time_medal_bonus: v.int(set, l::BB_FIRST_BONUS),
            medals: structs(v.elements(set, l::BB_MEDALS))
                .map(|s| MedalInfo {
                    medal: v.enum_key(s, l::BB_MEDAL),
                    trait_points: v.int(s, l::BB_TRAIT_POINTS),
                    cash_out_points_required: v.int(s, l::BB_CASHOUT_REQUIRED),
                    cash_out_current_points: v.pid_value(s, l::BB_CASHOUT_CURRENT),
                    bonus_property: v.pid_value(s, l::BB_BONUS_PROP),
                    cash_out_loot: v.did(s, l::BB_CASHOUT_LOOT),
                    alert: v.enum_key(s, l::BB_MEDAL_ALERT),
                })
                .collect(),
            quest_points: structs(v.elements(set, l::BB_QUEST_POINTS))
                .map(|s| (v.did(s, l::BB_QUEST), v.int(s, l::BB_QUEST_POINT_VALUE)))
                .collect(),
            quest_encounters: structs(v.elements(set, l::BB_QUEST_ENCOUNTERS))
                .map(|s| (v.did(s, l::BB_QUEST), v.did(s, l::BB_ENCOUNTER)))
                .collect(),
            location_events: structs(v.elements(set, l::BB_LOCATION_EVENTS))
                .map(|s| {
                    (
                        v.enum_key(s, l::BB_WAVE_LOCATION),
                        v.did(s, l::BB_LOCATION_EVENT),
                    )
                })
                .collect(),
        }
    }

    /// The medal entry of `medal`, if listed.
    #[must_use]
    pub fn medal(&self, medal: u32) -> Option<&MedalInfo> {
        self.medals.iter().find(|m| m.medal == Some(medal))
    }
}

// ------------------------------------------------------------------------ §8.3

/// One `DistantBattle_Wave` (§8.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wave {
    /// `DistantBattle_AllowedFlank_List` (`DistantBattleLocation` keys).
    pub flanks: Vec<u32>,
    /// `DistantBattle_DBU_Count`.
    pub unit_count: Option<i32>,
    /// `DistantBattle_DBU_Siege_Count`.
    pub siege_count: Option<i32>,
    /// Whether `DistantBattle_WaveRequirements` is stored.
    pub has_requirements: bool,
    /// `DistantBattle_ZoneEvent_FullClear`.
    pub full_clear_event: Option<Did>,
    /// `DistantBattle_NoSecondaryQuest`.
    pub no_secondary_quest: bool,
}

/// One `DistantBattle_Objective` (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Objective {
    /// `DistantBattle_Objective_Quest`.
    pub quest: Option<Did>,
    /// `DistantBattle_Objective_ZoneEvent`.
    pub zone_event: Option<Did>,
    /// `DistantBattle_AllowedFlank`.
    pub flank: Option<u32>,
    /// `DistantBattle_DBU_Count`.
    pub unit_count: Option<i32>,
    /// `DistantBattle_Wave_Count`.
    pub wave_count: Option<i32>,
}

/// A distant-battle script (§8.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DistantBattleScript {
    /// `DistantBattle_ZoneEvent_Required`.
    pub required_event: Option<Did>,
    /// `DistantBattle_ZoneEvent_Progress`.
    pub progress_event: Option<Did>,
    /// `DistantBattle_Wave_List`.
    pub waves: Vec<Wave>,
    /// `DistantBattle_Objective_List`.
    pub objectives: Vec<Objective>,
    /// (`DistantBattleUnit_Entry`, `DistantBattleUnit_Weight`) of
    /// `DistantBattleUnit_WeightedList`.
    pub weighted_units: Vec<(Option<Did>, Option<i32>)>,
    /// (unit, weight) of `DistantBattleUnit_Siege_WeightedList`.
    pub weighted_siege_units: Vec<(Option<Did>, Option<i32>)>,
}

impl DistantBattleScript {
    /// Reads the script fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        let pairs = |label| {
            structs(v.elements(set, label))
                .map(|s| (v.did(s, l::DBU_ENTRY), v.int(s, l::DBU_WEIGHT)))
                .collect()
        };
        Self {
            required_event: v.did(set, l::DB_ZONE_REQUIRED),
            progress_event: v.did(set, l::DB_ZONE_PROGRESS),
            waves: structs(v.elements(set, l::DB_WAVES))
                .map(|s| Wave {
                    flanks: enum_keys(v.elements(s, l::DB_FLANKS)),
                    unit_count: v.int(s, l::DB_DBU_COUNT),
                    siege_count: v.int(s, l::DB_SIEGE_COUNT),
                    has_requirements: v.has(s, l::DB_REQUIREMENTS),
                    full_clear_event: v.did(s, l::DB_FULL_CLEAR),
                    no_secondary_quest: v.flag(s, l::DB_NO_SECONDARY),
                })
                .collect(),
            objectives: structs(v.elements(set, l::DB_OBJECTIVES))
                .map(|s| Objective {
                    quest: v.did(s, l::DB_OBJ_QUEST),
                    zone_event: v.did(s, l::DB_OBJ_EVENT),
                    flank: v.enum_key(s, l::DB_FLANK),
                    unit_count: v.int(s, l::DB_DBU_COUNT),
                    wave_count: v.int(s, l::DB_WAVE_COUNT),
                })
                .collect(),
            weighted_units: pairs(l::DBU_WEIGHTED),
            weighted_siege_units: pairs(l::DBU_SIEGE_WEIGHTED),
        }
    }

    /// Every unit named by the weighted lists (normal, then siege).
    pub fn units(&self) -> impl Iterator<Item = Did> + '_ {
        self.weighted_units
            .iter()
            .chain(&self.weighted_siege_units)
            .filter_map(|(d, _)| nonzero(*d))
    }
}

// ------------------------------------------------------------------------ §8.4

/// One `DistantBattleUnit_EntityStruct`: one kind of figure (§8.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityGroup {
    /// `DistantBattleUnit_Formation_Unit`.
    pub unit: Option<Did>,
    /// `DistantBattleUnit_CosmeticEntity` (an entity descriptor, prefix 0x47).
    pub cosmetic_entity: Option<Did>,
    /// `DistantBattleUnit_Entity_Offset_Array`: (lateral, 0, depth) per figure.
    pub offsets: Vec<[f32; 3]>,
    /// `DistantBattleUnit_VisualRadius`.
    pub visual_radius: Option<f32>,
    /// `DistantBattleUnit_Formation_Jitter`.
    pub jitter: Option<f32>,
    /// Number of `Appearance_ClassList` entries.
    pub appearance_entries: usize,
}

/// One `DistantBattleUnit_FormationGroup`: layout parameters (§8.4; the client does not
/// read them, B5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FormationGroup {
    /// `DistantBattleUnit_Formation_Unit`.
    pub unit: Option<Did>,
    /// `DistantBattleUnit_Formation_Jitter`.
    pub jitter: Option<f32>,
    /// `DistantBattleUnit_Formation_Priority`.
    pub priority: Option<i32>,
    /// `DistantBattleUnit_Formation_LateralDepth`.
    pub lateral_depth: Option<i32>,
    /// `DistantBattleUnit_Formation_LateralWidth_Ratio`.
    pub lateral_width_ratio: Option<f32>,
    /// `DistantBattleUnit_Formation_LayoutCalculator`.
    pub layout_calculator: Option<Did>,
}

/// One element of `DistantBattleUnit_Composition_Removed_List` (§15.1 B5 step 4, value
/// types §3.6). An element that is not a struct reads as the default (no unit), which the
/// client skips; it still counts as a position of the list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Removal {
    /// `DistantBattleUnit_Formation_Unit`: the groups to remove figures from. `None` when
    /// absent, not a DID or 0 (such an element is skipped).
    pub unit: Option<Did>,
    /// `DistantBattleUnit_Removal_Count`: a 32-bit integer read as unsigned; absent or of
    /// another type → 1.
    pub count: u32,
    /// `DistantBattleUnit_Removal_ScriptKey` (mapper `FX`); absent → 0, "no script".
    pub script_key: u32,
    /// `DistantBattleUnit_Removal_ScriptParameters`, handed to the removal script unchanged.
    pub script_parameters: Option<PropertyStruct>,
}

impl Removal {
    /// Reads one removal struct.
    #[must_use]
    pub fn read(s: &PropertyStruct, v: &InstanceVocab) -> Self {
        Self {
            unit: nonzero(v.did(s, l::DBU_UNIT)),
            count: v.int(s, l::DBU_REMOVAL_COUNT).map_or(1, i32::cast_unsigned),
            script_key: v.enum_key(s, l::DBU_REMOVAL_SCRIPT).unwrap_or(0),
            script_parameters: v
                .value(s, l::DBU_REMOVAL_PARAMS)
                .and_then(PropertyValue::as_struct)
                .cloned(),
        }
    }
}

/// A distant-battle unit (§8.4); also the unit fields carried by class 4026 (§8.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DistantBattleUnit {
    /// `DistantBattleUnit_EntityComposition`, stored order.
    pub groups: Vec<EntityGroup>,
    /// `DistantBattleUnit_Formation`, stored order.
    pub formation: Vec<FormationGroup>,
    /// `DistantBattleUnit_Formation_Dimensions`: (minimum lateral, maximum lateral, depth).
    pub dimensions: Option<[f32; 3]>,
    /// `DistantBattleUnit_Formation_Width` (width class 1–7).
    pub width_class: Option<u32>,
    /// `DistantBattleUnit_Composition`: one spawnable template per figure.
    pub composition: Vec<Did>,
    /// `DistantBattleUnit_SpawnRate`.
    pub spawn_rate: Option<f32>,
    /// `DistantBattleUnit_Min_Activator_Count`.
    pub min_activators: Option<i32>,
    /// `DistantBattleUnit_Activator_Respawn_Time`.
    pub activator_respawn_time: Option<f32>,
    /// `DistantBattleUnit_GroundClampEntities` as stored (a boolean value; another type
    /// reads as absent, §3.6).
    pub ground_clamp_stored: Option<bool>,
    /// `DistantBattleUnit_CaptureQualityOverride` as stored.
    pub capture_quality_stored: Option<u32>,
    /// `DistantBattleUnit_LaneType` as stored.
    pub lane_type_stored: Option<u32>,
    /// `DistantBattleUnit_Composition_Removed_List` (run-time state; empty in the data),
    /// one entry per stored element; a value that is not an array reads as empty.
    pub removals: Vec<Removal>,
}

/// An axis-aligned box (B5 step 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitBox {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl DistantBattleUnit {
    /// Reads the unit fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            groups: structs(v.elements(set, l::DBU_ENTITY_COMPOSITION))
                .map(|s| EntityGroup {
                    unit: v.did(s, l::DBU_UNIT),
                    cosmetic_entity: v.did(s, l::DBU_COSMETIC),
                    offsets: v
                        .elements(s, l::DBU_OFFSETS)
                        .iter()
                        .filter_map(|e| match e.value {
                            PropertyValue::Vector3(o) => Some(o),
                            _ => None,
                        })
                        .collect(),
                    visual_radius: v.float(s, l::DBU_VISUAL_RADIUS),
                    jitter: v.float(s, l::DBU_JITTER),
                    appearance_entries: v.elements(s, l::APPEARANCE_CLASS_LIST).len(),
                })
                .collect(),
            formation: structs(v.elements(set, l::DBU_FORMATION))
                .map(|s| FormationGroup {
                    unit: v.did(s, l::DBU_UNIT),
                    jitter: v.float(s, l::DBU_JITTER),
                    priority: v.int(s, l::DBU_PRIORITY),
                    lateral_depth: v.int(s, l::DBU_LATERAL_DEPTH),
                    lateral_width_ratio: v.float(s, l::DBU_WIDTH_RATIO),
                    layout_calculator: v.did(s, l::DBU_LAYOUT),
                })
                .collect(),
            dimensions: match v.value(set, l::DBU_DIMENSIONS) {
                Some(PropertyValue::Vector3(d)) => Some(*d),
                _ => None,
            },
            width_class: v.enum_key(set, l::DBU_WIDTH),
            composition: dids(v.elements(set, l::DBU_COMPOSITION)),
            spawn_rate: v.float(set, l::DBU_SPAWN_RATE),
            min_activators: v.int(set, l::DBU_MIN_ACTIVATORS),
            activator_respawn_time: v.float(set, l::DBU_RESPAWN),
            ground_clamp_stored: v.boolean(set, l::DBU_GROUND_CLAMP),
            capture_quality_stored: v.enum_key(set, l::DBU_CAPTURE),
            lane_type_stored: v.enum_key(set, l::DBU_LANE),
            removals: Self::read_removals(set, v),
        }
    }

    /// The removal list of a property set or struct as the client reads it (§15.1 B5
    /// step 4): one [`Removal`] per element, a non-struct element as the default (skipped
    /// when applied), a value that is not an array as the empty list.
    #[must_use]
    pub fn read_removals<F: super::Fields + ?Sized>(f: &F, v: &InstanceVocab) -> Vec<Removal> {
        v.elements(f, l::DBU_REMOVED)
            .iter()
            .map(|e| {
                e.value
                    .as_struct()
                    .map_or_else(Removal::default, |s| Removal::read(s, v))
            })
            .collect()
    }

    /// Ground clamping of the figures (B5 step 3: absent or not a boolean → true).
    #[must_use]
    pub fn ground_clamp(&self) -> bool {
        self.ground_clamp_stored.unwrap_or(true)
    }

    /// Capture quality (B5 step 2: absent → automatic, 0xFFFFFFFF).
    #[must_use]
    pub fn capture_quality(&self) -> u32 {
        self.capture_quality_stored
            .unwrap_or(CAPTURE_QUALITY_AUTOMATIC)
    }

    /// Lane type (§3.3: absent → 2, spawn).
    #[must_use]
    pub fn lane_type(&self) -> u32 {
        self.lane_type_stored.unwrap_or(LANE_TYPE_DEFAULT)
    }

    /// Number of cosmetic figures: one per offset of every group (B5 step 1).
    #[must_use]
    pub fn figure_count(&self) -> usize {
        self.groups.iter().map(|g| g.offsets.len()).sum()
    }

    /// The box of B5 step 5 around a unit at `position`: with dimensions (a, b, c) and
    /// h = (b − a) × 0.5 it spans x − h … x + h, y … y + c and z … z + 1; without
    /// dimensions it is the point.
    #[must_use]
    pub fn bounding_box(&self, position: [f32; 3]) -> UnitBox {
        let [x, y, z] = position;
        match self.dimensions {
            Some([a, b, c]) => {
                let h = (b - a) * 0.5;
                UnitBox {
                    min: [x - h, y, z],
                    max: [x + h, y + c, z + 1.0],
                }
            }
            None => UnitBox {
                min: position,
                max: position,
            },
        }
    }

    /// N-I10 on one unit: composition multiset = each group's unit repeated per offset; the
    /// formation and the entity composition name the same units with equal jitter; offsets
    /// have second component 0 and lie inside the dimensions; and, when `actual_width` (the
    /// formation control's width for this unit's width class) is given, dimension width ≤
    /// actual width.
    #[must_use]
    pub fn deviations(&self, actual_width: Option<f32>) -> Vec<Deviation> {
        let mut out = Vec::new();
        let mut want: BTreeMap<u32, usize> = BTreeMap::new();
        for g in &self.groups {
            let unit = g.unit.map_or(0, |d| d.0);
            *want.entry(unit).or_default() += g.offsets.len();
        }
        let mut have: BTreeMap<u32, usize> = BTreeMap::new();
        for d in &self.composition {
            *have.entry(d.0).or_default() += 1;
        }
        if want != have {
            out.push(Deviation::new(
                "N-I10",
                format!(
                    "composition {} entries in {} units vs {} offsets in {} groups",
                    self.composition.len(),
                    have.len(),
                    self.figure_count(),
                    self.groups.len()
                ),
            ));
        }
        let mut entity: BTreeMap<u32, Option<u32>> = BTreeMap::new();
        for g in &self.groups {
            entity.insert(g.unit.map_or(0, |d| d.0), g.jitter.map(f32::to_bits));
        }
        let mut formation: BTreeMap<u32, Option<u32>> = BTreeMap::new();
        for f in &self.formation {
            formation.insert(f.unit.map_or(0, |d| d.0), f.jitter.map(f32::to_bits));
        }
        if entity != formation {
            out.push(Deviation::new(
                "N-I10",
                "formation units or jitter differ from the entity composition",
            ));
        }
        match self.dimensions {
            Some([lo, hi, depth]) => {
                let outside = self
                    .groups
                    .iter()
                    .flat_map(|g| &g.offsets)
                    .filter(|[a, b, c]| *b != 0.0 || *a < lo || *a > hi || *c < 0.0 || *c > depth)
                    .count();
                if outside > 0 {
                    out.push(Deviation::new(
                        "N-I10",
                        format!("{outside} offsets outside the dimensions"),
                    ));
                }
                if let Some(w) = actual_width
                    && hi - lo > w
                {
                    out.push(Deviation::new(
                        "N-I10",
                        format!("dimension width {} > actual width {w}", hi - lo),
                    ));
                }
            }
            None if self.figure_count() > 0 => {
                out.push(Deviation::new("N-I10", "offsets without dimensions"));
            }
            None => {}
        }
        out
    }
}

/// The figures left per entity group after removals (§15.1 B5 step 4, §17.6).
///
/// Figures go from the **end** of a group's offset array, last stored first, so the figures
/// left in group *g* are always its first [`FigureCounts::remaining`] offsets
/// ([`FigureCounts::write_back`] shortens the unit's arrays accordingly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureCounts {
    units: Vec<Option<Did>>,
    remaining: Vec<usize>,
    applied: usize,
}

/// What the client does with the figures one element removed (§15.1 B5 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalEffect {
    /// Nothing to play: first display, or no figure was removed.
    Silent,
    /// No script key (0): the removed figures are dropped at once.
    DropAtOnce,
    /// The removal script with this key plays once for the element, on the removed figures
    /// (with the element's parameters).
    PlayScript(u32),
}

/// The figures one group lost to one element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedFigures {
    /// Index of the group in [`DistantBattleUnit::groups`].
    pub group: usize,
    /// Indices into the group's offset array of the removed figures, last stored first.
    pub offsets: Vec<usize>,
}

/// One applied element of the removal list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedRemoval {
    /// Position of the element in the list.
    pub element: usize,
    /// The figures removed, per matching group (every group whose unit matches).
    pub removed: Vec<RemovedFigures>,
    /// What happens to the removed figures.
    pub effect: RemovalEffect,
}

impl AppliedRemoval {
    /// Figures removed in all groups.
    #[must_use]
    pub fn figure_count(&self) -> usize {
        self.removed.iter().map(|r| r.offsets.len()).sum()
    }
}

impl FigureCounts {
    /// All figures of `unit`, none removed, no element applied yet.
    #[must_use]
    pub fn new(unit: &DistantBattleUnit) -> Self {
        Self {
            units: unit.groups.iter().map(|g| g.unit).collect(),
            remaining: unit.groups.iter().map(|g| g.offsets.len()).collect(),
            applied: 0,
        }
    }

    /// The first display of the unit's figures (B5 step 4): the elements already in `list`
    /// are applied (positions 0 … length − 1) before the figures are created, without
    /// removal scripts ([`RemovalEffect::Silent`]).
    #[must_use]
    pub fn first_display(
        unit: &DistantBattleUnit,
        list: &[Removal],
    ) -> (Self, Vec<AppliedRemoval>) {
        let mut c = Self::new(unit);
        let out = list
            .iter()
            .enumerate()
            .filter_map(|(i, r)| c.apply_element(i, r, true))
            .collect();
        c.applied = list.len();
        (c, out)
    }

    /// Figures left in group `group`.
    #[must_use]
    pub fn remaining(&self, group: usize) -> Option<usize> {
        self.remaining.get(group).copied()
    }

    /// A change of the property (B5 step 4): applies the elements at positions *old
    /// length* … *new length* − 1, *old length* being the list's length just before this
    /// change. A list that shrinks or keeps its length applies nothing; either way the
    /// length is remembered for the next change.
    pub fn apply(&mut self, list: &[Removal]) -> Vec<AppliedRemoval> {
        let out = list
            .iter()
            .enumerate()
            .skip(self.applied)
            .filter_map(|(i, r)| self.apply_element(i, r, false))
            .collect();
        self.applied = list.len();
        out
    }

    /// One element: skipped without a unit; otherwise every group whose unit equals it
    /// loses up to `count` figures from the end of its offsets. `None` when no group
    /// matched (nothing happens).
    fn apply_element(&mut self, i: usize, r: &Removal, silent: bool) -> Option<AppliedRemoval> {
        let unit = r.unit?;
        let want = usize::try_from(r.count).unwrap_or(usize::MAX);
        let mut removed = Vec::new();
        for (g, (u, left)) in self.units.iter().zip(self.remaining.iter_mut()).enumerate() {
            if *u != Some(unit) {
                continue;
            }
            let n = want.min(*left);
            let offsets = (*left - n..*left).rev().collect();
            *left -= n;
            removed.push(RemovedFigures { group: g, offsets });
        }
        if removed.is_empty() {
            tracing::debug!(unit = unit.0, "removal names no group of the unit");
            return None;
        }
        let any = removed.iter().any(|r| !r.offsets.is_empty());
        let effect = match (silent, any, r.script_key) {
            (true, _, _) | (false, false, _) => RemovalEffect::Silent,
            (false, true, 0) => RemovalEffect::DropAtOnce,
            (false, true, k) => RemovalEffect::PlayScript(k),
        };
        Some(AppliedRemoval {
            element: i,
            removed,
            effect,
        })
    }

    /// Writes the shortened offset arrays back into `unit`'s entity composition, so a later
    /// rebuild of the figures does not bring removed ones back (B5 step 4).
    pub fn write_back(&self, unit: &mut DistantBattleUnit) {
        for (g, left) in unit.groups.iter_mut().zip(&self.remaining) {
            g.offsets.truncate(*left);
        }
    }
}

// ------------------------------------------------------------------------ §8.5

/// The formation control (§8.5, label `DistantBattleUnitFormationControl`). The client
/// reads none of it (B7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormationControl {
    /// `DistantBattleUnit_FormationControl_SpacingBetweenGroups` (float).
    pub spacing_between_groups: Option<f32>,
    /// (`DistantBattleUnit_Formation_Width`, `DistantBattleUnit_FormationControl_ActualWidth`)
    /// of each `DistantBattleUnit_FormationControl_Width_Struct`.
    pub widths: Vec<(Option<u32>, Option<f32>)>,
}

impl FormationControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        Self {
            spacing_between_groups: v.float(set, l::FC_SPACING),
            widths: structs(v.elements(set, l::FC_WIDTHS))
                .map(|s| (v.enum_key(s, l::DBU_WIDTH), v.float(s, l::FC_ACTUAL_WIDTH)))
                .collect(),
        }
    }

    /// The actual width of width class `class`, if listed.
    #[must_use]
    pub fn actual_width(&self, class: u32) -> Option<f32> {
        self.widths
            .iter()
            .find(|(c, _)| *c == Some(class))
            .and_then(|(_, w)| *w)
    }
}
