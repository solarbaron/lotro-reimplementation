// SPDX-License-Identifier: GPL-3.0-or-later
//! The skirmish control record (`instances.md` §6.3, §6.4) and the client's statistics-panel
//! rules (§15.1 B4): derived statistics and the best-time aggregation.

use std::collections::BTreeSet;

use super::labels as l;
use super::{Deviation, InstanceVocab, enum_keys, first_of_type, structs};
use crate::did::Did;
use crate::property::{PropertyStruct, PropertyType, PropertyValue};
use crate::property_set::PropertySet;

/// `Mod_Op` key: divide (§15.1 B4).
pub const OP_DIVIDE: u32 = 2;
/// `Mod_Op` key: subtract.
pub const OP_SUBTRACT: u32 = 6;
/// `Mod_Op` key: add.
pub const OP_ADD: u32 = 7;
/// `Mod_Op` key: multiply.
pub const OP_MULTIPLY: u32 = 8;

/// The `SkirmishStat` key of best time, which combines by minimum or maximum (B4).
pub const STAT_BEST_TIME: u32 = 16;

/// Statistics, group sizes and level classes of the conversion hash (N-I12).
pub const CONVERSION_STATS: usize = 19;
/// Group sizes of the conversion hash (N-I12).
pub const CONVERSION_SIZES: usize = 6;

/// One `SkirmishStats_StatProperty_Conversion_Hash` entry (§6.3).
///
/// The spec names its fields by suffix only; they are told apart by type and enum mapper:
/// the PID field is the statistic property, the boolean the on-level flag, and the two
/// enums by whether their definition uses the `SkirmishStat` or `SkirmishGroupSize` mapper
/// ([`InstanceVocab::is_stat_enum`], [`InstanceVocab::is_size_enum`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConversionEntry {
    /// The statistic property (`SkirmishStats_<statistic>_<size>_<level class>`).
    pub property: Option<u32>,
    /// The `SkirmishStat` key.
    pub stat: Option<u32>,
    /// The `SkirmishGroupSize` key.
    pub group_size: Option<u32>,
    /// On-level (absent = false).
    pub on_level: bool,
}

/// One derived statistic (`Skirmish_Stat`, §6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedStat {
    /// `Skirmish_DerivedStat` (key 17–21).
    pub derived: Option<u32>,
    /// `Skirmish_Primary_Stat`.
    pub primary: Option<u32>,
    /// (`Skirmish_Secondary_Stat`, `Mod_Op`) of each operation, stored order.
    pub operations: Vec<(Option<u32>, Option<u32>)>,
    /// `Skirmish_Stat_Scale` (default 1).
    pub scale: i32,
}

/// A statistic value: integer statistics and float statistics (best time, played time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatValue {
    /// An integer statistic.
    Int(i64),
    /// A float statistic.
    Float(f64),
}

impl StatValue {
    /// The value as f64.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(v) => v as f64,
            Self::Float(v) => v,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }
}

impl DerivedStat {
    fn read(s: &PropertyStruct, v: &InstanceVocab) -> Self {
        Self {
            derived: v.bits32(s, l::STAT_DERIVED),
            primary: v.enum_key(s, l::STAT_PRIMARY),
            operations: structs(v.elements(s, l::STAT_OPERATIONS))
                .map(|o| (v.enum_key(o, l::STAT_SECONDARY), v.enum_key(o, l::MOD_OP)))
                .collect(),
            scale: v.int(s, l::STAT_SCALE).unwrap_or(1),
        }
    }

    /// Evaluates the statistic as the panel does (§15.1 B4). `value_of` gives the current
    /// value of a stored statistic (its float-ness decides the arithmetic); an absent
    /// primary or secondary key reads as integer 0.
    ///
    /// Floating point is used when the primary or any secondary statistic is a float or any
    /// operation divides; integer arithmetic otherwise. Integer arithmetic saturates at the
    /// i64 range (an implementer choice; the spec does not give the width).
    pub fn evaluate(&self, value_of: impl Fn(u32) -> StatValue) -> StatValue {
        let get = |k: Option<u32>| k.map_or(StatValue::Int(0), &value_of);
        let primary = get(self.primary);
        let seconds: Vec<StatValue> = self.operations.iter().map(|(s, _)| get(*s)).collect();
        let float = primary.is_float()
            || seconds.iter().any(|s| s.is_float())
            || self.operations.iter().any(|(_, op)| *op == Some(OP_DIVIDE));
        if float {
            let mut acc = primary.as_f64();
            for ((_, op), sec) in self.operations.iter().zip(&seconds) {
                let x = sec.as_f64();
                acc = match *op {
                    Some(OP_DIVIDE) if x == 0.0 => 0.0,
                    Some(OP_DIVIDE) => acc / x,
                    Some(OP_SUBTRACT) => acc - x,
                    Some(OP_ADD) => acc + x,
                    Some(OP_MULTIPLY) => acc * x,
                    _ => acc,
                };
            }
            if self.scale != 1 {
                acc *= f64::from(self.scale);
            }
            StatValue::Float(acc)
        } else {
            let as_int = |s: StatValue| match s {
                StatValue::Int(v) => v,
                StatValue::Float(_) => 0,
            };
            let mut acc = as_int(primary);
            for ((_, op), sec) in self.operations.iter().zip(&seconds) {
                let x = as_int(*sec);
                acc = match *op {
                    Some(OP_SUBTRACT) => acc.saturating_sub(x),
                    Some(OP_ADD) => acc.saturating_add(x),
                    Some(OP_MULTIPLY) => acc.saturating_mul(x),
                    _ => acc,
                };
            }
            if self.scale != 1 {
                acc = acc.saturating_mul(i64::from(self.scale));
            }
            StatValue::Int(acc)
        }
    }
}

/// Combines best times over several group sizes or level classes (§15.1 B4): a time at or
/// below `fastest_possible` is "no time"; among real times the smallest wins, or the
/// largest when `highest_is_best` (the template's
/// `Skirmish_Template_Highest_Time_Is_Best_Time`). `None` when no real time exists.
#[must_use]
pub fn combine_best_time(
    times: &[f64],
    fastest_possible: f64,
    highest_is_best: bool,
) -> Option<f64> {
    times
        .iter()
        .copied()
        .filter(|t| *t > fastest_possible)
        .reduce(|a, b| if highest_is_best { a.max(b) } else { a.min(b) })
}

/// The skirmish control record (§6.3, label `SkirmishControl`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkirmishControl {
    /// (tier, point multiplier) of `SkirmishControl_DifficultyMultiplier_Array`; fields told
    /// apart by type (one enum, one float).
    pub difficulty_multipliers: Vec<(u32, f32)>,
    /// (group size, point multiplier) of `SkirmishControl_GroupSizeMultiplier_Array`.
    pub group_size_multipliers: Vec<(u32, f32)>,
    /// Whether `SkirmishControl_ZoneEvents` is stored.
    pub has_zone_events: bool,
    /// `SkirmishControl_SkirmishPointToken`.
    pub point_token: Option<Did>,
    /// `SkirmishControl_SkirmishPanel_RequiredTrait`.
    pub panel_required_trait: Option<Did>,
    /// (`…_MaxRank`, `…_MaxRank_WebstoreItem`) of each trait gate, told apart by type.
    pub trait_gates: Vec<(Option<i32>, Option<Did>)>,
    /// `SkirmishControl_SkirmishTraitGate_DEFAULT_MaxRank`.
    pub default_max_rank: Option<i32>,
    /// `SkirmishControl_SkirmishTraitPools` (trait-nature keys).
    pub trait_pools: Vec<u32>,
    /// `SkirmishControl_SkirmishGating_DEFAULT_MaxSkirmishesPerWeek` (−1 = no limit).
    pub max_skirmishes_per_week: Option<i32>,
    /// `SkirmishControl_BlockedStats_List` (statistic keys).
    pub blocked_stats: Vec<u32>,
    /// `SkirmishControl_SkirmishGRSEnum_PropertyList` (PIDs).
    pub grs_properties: Vec<u32>,
    /// (statistic key, `GRS_…` PID) of the statistic-to-GRS hash, told apart by type.
    pub stat_to_grs: Vec<(Option<u32>, Option<u32>)>,
    /// The conversion hash.
    pub conversion: Vec<ConversionEntry>,
    /// `SkirmishStats_CONSTANT_FASTEST_POSSIBLE_SKIRMISH_TIME`.
    pub fastest_possible_time: Option<f32>,
    /// `Skirmish_Stats`.
    pub derived_stats: Vec<DerivedStat>,
}

fn enum_float_pairs(s: &PropertyStruct) -> Option<(u32, f32)> {
    let key = match first_of_type(s, PropertyType::Enum, None)? {
        PropertyValue::Enum(k) => *k,
        _ => return None,
    };
    let x = first_of_type(s, PropertyType::Float, None)?.as_f32()?;
    Some((key, x))
}

impl SkirmishControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        let pid_of = |s: &PropertyStruct| match first_of_type(s, PropertyType::PropertyId, None) {
            Some(PropertyValue::PropertyId(p)) => Some(*p),
            _ => None,
        };
        Self {
            difficulty_multipliers: structs(v.elements(set, l::SC_DIFFICULTY_MULT))
                .filter_map(enum_float_pairs)
                .collect(),
            group_size_multipliers: structs(v.elements(set, l::SC_GROUP_SIZE_MULT))
                .filter_map(enum_float_pairs)
                .collect(),
            has_zone_events: v.has(set, l::SC_ZONE_EVENTS),
            point_token: v.did(set, l::SC_POINT_TOKEN),
            panel_required_trait: v.did(set, l::SC_PANEL_TRAIT),
            trait_gates: structs(v.elements(set, l::SC_TRAIT_GATES))
                .map(|s| {
                    (
                        match first_of_type(s, PropertyType::Int, None) {
                            Some(PropertyValue::Int(n)) => Some(*n),
                            _ => None,
                        },
                        first_of_type(s, PropertyType::DidRef, None)
                            .and_then(PropertyValue::as_did),
                    )
                })
                .collect(),
            default_max_rank: v.int(set, l::SC_GATE_DEFAULT_RANK),
            trait_pools: enum_keys(v.elements(set, l::SC_TRAIT_POOLS)),
            max_skirmishes_per_week: v.int(set, l::SC_MAX_PER_WEEK),
            blocked_stats: enum_keys(v.elements(set, l::SC_BLOCKED_STATS)),
            grs_properties: v
                .elements(set, l::SC_GRS_LIST)
                .iter()
                .filter_map(|e| match e.value {
                    PropertyValue::PropertyId(p) => Some(p),
                    _ => None,
                })
                .collect(),
            stat_to_grs: structs(v.elements(set, l::SC_STAT_TO_GRS))
                .map(|s| {
                    let stat = match first_of_type(s, PropertyType::Enum, None) {
                        Some(PropertyValue::Enum(k)) => Some(*k),
                        _ => None,
                    };
                    (stat, pid_of(s))
                })
                .collect(),
            conversion: structs(v.elements(set, l::SC_CONVERSION))
                .map(|s| {
                    let mut e = ConversionEntry {
                        property: pid_of(s),
                        ..ConversionEntry::default()
                    };
                    for f in &s.fields {
                        match f.value {
                            PropertyValue::Enum(k) if v.is_stat_enum(f.pid) => e.stat = Some(k),
                            PropertyValue::Enum(k) if v.is_size_enum(f.pid) => {
                                e.group_size = Some(k);
                            }
                            PropertyValue::Bool(b) => e.on_level = b,
                            _ => {}
                        }
                    }
                    e
                })
                .collect(),
            fastest_possible_time: v.float(set, l::SC_FASTEST),
            derived_stats: structs(v.elements(set, l::SC_STATS))
                .map(|s| DerivedStat::read(s, v))
                .collect(),
        }
    }

    /// The point multiplier of difficulty tier `tier`, if listed.
    #[must_use]
    pub fn difficulty_multiplier(&self, tier: u32) -> Option<f32> {
        self.difficulty_multipliers
            .iter()
            .find(|(k, _)| *k == tier)
            .map(|(_, m)| *m)
    }

    /// The point multiplier of group size `size`, if listed.
    #[must_use]
    pub fn group_size_multiplier(&self, size: u32) -> Option<f32> {
        self.group_size_multipliers
            .iter()
            .find(|(k, _)| *k == size)
            .map(|(_, m)| *m)
    }

    /// The statistic property of (statistic, group size, on-level) from the conversion
    /// hash, the authoritative map of §6.4.
    #[must_use]
    pub fn stat_property(&self, stat: u32, group_size: u32, on_level: bool) -> Option<u32> {
        self.conversion
            .iter()
            .find(|e| {
                e.stat == Some(stat) && e.group_size == Some(group_size) && e.on_level == on_level
            })
            .and_then(|e| e.property)
    }

    /// N-I12: one conversion entry per (statistic, size, on-level) for 19 statistics, 6
    /// sizes and both level classes, each with a statistic property.
    #[must_use]
    pub fn check_conversion(&self) -> Vec<Deviation> {
        let mut out = Vec::new();
        let mut keys = BTreeSet::new();
        let mut stats = BTreeSet::new();
        let mut sizes = BTreeSet::new();
        for e in &self.conversion {
            let (Some(stat), Some(size), Some(_)) = (e.stat, e.group_size, e.property) else {
                out.push(Deviation::new("N-I12", format!("incomplete entry {e:?}")));
                continue;
            };
            stats.insert(stat);
            sizes.insert(size);
            if !keys.insert((stat, size, e.on_level)) {
                out.push(Deviation::new(
                    "N-I12",
                    format!("duplicate ({stat}, {size}, {})", e.on_level),
                ));
            }
        }
        if stats.len() != CONVERSION_STATS
            || sizes.len() != CONVERSION_SIZES
            || keys.len() != CONVERSION_STATS * CONVERSION_SIZES * 2
        {
            out.push(Deviation::new(
                "N-I12",
                format!(
                    "{} statistics × {} sizes, {} distinct keys",
                    stats.len(),
                    sizes.len(),
                    keys.len()
                ),
            ));
        }
        out
    }
}
