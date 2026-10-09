// SPDX-License-Identifier: GPL-3.0-or-later
//! The skirmish control record (`instances.md` §6.3, §6.4) and the client's statistics-panel
//! rules (§15.1 B4): derived statistics and the best-time aggregation.

use std::collections::BTreeSet;

use super::labels as l;
use super::{Deviation, InstanceVocab, enum_keys, structs};
use crate::did::Did;
use crate::property::{PropertyStruct, PropertyValue};
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

/// One `SkirmishStats_StatProperty_HashEntry` of the conversion hash (§6.3), its fields
/// read by their full labels (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConversionEntry {
    /// `SkirmishStats_StatProperty_FullPropertyName`: the statistic property
    /// (`SkirmishStats_<statistic>_<size>_<level class>`).
    pub property: Option<u32>,
    /// `SkirmishStats_StatProperty_SkirmishStat`.
    pub stat: Option<u32>,
    /// `SkirmishStats_StatProperty_SkirmishGroupSize`.
    pub group_size: Option<u32>,
    /// `SkirmishStats_StatProperty_OnLevel` (absent = false).
    pub on_level: bool,
}

/// One derived statistic (`Skirmish_Stat`, §6.3), read as the client reads it (§15.1 B4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedStat {
    /// `Skirmish_DerivedStat` (key 17–21; an enum or PID value).
    pub derived: Option<u32>,
    /// `Skirmish_Primary_Stat` (an enum or PID value).
    pub primary: Option<u32>,
    /// (`Skirmish_Secondary_Stat`, `Mod_Op`) of each operation, stored order. Reading
    /// stops at the first operation that is not a struct or whose secondary statistic or
    /// `Mod_Op` is missing or not an enum or PID value; the operations before it are kept.
    pub operations: Vec<(u32, u32)>,
    /// `Skirmish_Stat_Scale` (a 32-bit integer; absent → 1).
    pub scale: i32,
}

/// A statistic value: integer statistics (signed 32-bit) and float statistics (single
/// precision: best time, played time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatValue {
    /// An integer statistic.
    Int(i32),
    /// A float statistic.
    Float(f32),
}

impl StatValue {
    /// The value in single precision (an integer is converted, §15.1 B4).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f32(self) -> f32 {
        match self {
            Self::Int(v) => v as f32,
            Self::Float(v) => v,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }
}

impl DerivedStat {
    /// Reads one `Skirmish_Stat` struct (§15.1 B4). `None` when `Skirmish_Stat_Scale` is
    /// stored with a value type other than a 32-bit integer: the client stops reading such
    /// an entry before its operations, and Tessera ignores it as malformed, with a warning
    /// (Tessera Decision of B4).
    #[must_use]
    pub fn read(s: &PropertyStruct, v: &InstanceVocab) -> Option<Self> {
        let scale = match v.value(s, l::STAT_SCALE) {
            None => 1,
            Some(PropertyValue::Int(n)) => *n,
            Some(_) => {
                tracing::warn!("skirmish derived statistic with a non-integer scale ignored");
                return None;
            }
        };
        let mut operations = Vec::new();
        for e in v.elements(s, l::STAT_OPERATIONS) {
            let Some(o) = e.value.as_struct() else {
                break;
            };
            let (Some(secondary), Some(op)) = (v.key(o, l::STAT_SECONDARY), v.key(o, l::MOD_OP))
            else {
                break;
            };
            operations.push((secondary, op));
        }
        Some(Self {
            derived: v.key(s, l::STAT_DERIVED),
            primary: v.key(s, l::STAT_PRIMARY),
            operations,
            scale,
        })
    }

    /// Evaluates the statistic as the panel does (§15.1 B4). `value_of` gives the current
    /// value of a stored statistic (its float-ness decides the arithmetic); an absent
    /// primary key reads as integer 0.
    ///
    /// Floating point (IEEE single precision, integer statistics converted before each
    /// operation) is used when the primary or any secondary statistic is a float or any
    /// operation divides; a divisor of 0 sets the running value to 0. Otherwise the
    /// arithmetic is signed 32-bit two's complement: add, subtract and multiply wrap on
    /// overflow. The scale, when not 1, multiplies the result (converted to single
    /// precision in floating-point mode).
    pub fn evaluate(&self, value_of: impl Fn(u32) -> StatValue) -> StatValue {
        let primary = self.primary.map_or(StatValue::Int(0), &value_of);
        let seconds: Vec<StatValue> = self.operations.iter().map(|(s, _)| value_of(*s)).collect();
        let float = primary.is_float()
            || seconds.iter().any(|s| s.is_float())
            || self.operations.iter().any(|(_, op)| *op == OP_DIVIDE);
        if float {
            let mut acc = primary.as_f32();
            for ((_, op), sec) in self.operations.iter().zip(&seconds) {
                let x = sec.as_f32();
                acc = match *op {
                    OP_DIVIDE if x == 0.0 => 0.0,
                    OP_DIVIDE => acc / x,
                    OP_SUBTRACT => acc - x,
                    OP_ADD => acc + x,
                    OP_MULTIPLY => acc * x,
                    _ => acc,
                };
            }
            if self.scale != 1 {
                acc *= StatValue::Int(self.scale).as_f32();
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
                    OP_SUBTRACT => acc.wrapping_sub(x),
                    OP_ADD => acc.wrapping_add(x),
                    OP_MULTIPLY => acc.wrapping_mul(x),
                    _ => acc,
                };
            }
            if self.scale != 1 {
                acc = acc.wrapping_mul(self.scale);
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
    /// (`SkirmishControl_DifficultyTier`, `SkirmishControl_SkirmishPoint_Multiplier`) of
    /// `SkirmishControl_DifficultyMultiplier_Array`.
    pub difficulty_multipliers: Vec<(u32, f32)>,
    /// (`SkirmishControl_GroupSize`, `SkirmishControl_SkirmishPoint_Multiplier`) of
    /// `SkirmishControl_GroupSizeMultiplier_Array`.
    pub group_size_multipliers: Vec<(u32, f32)>,
    /// (`SkirmishControl_GroupSize_StructVal`, `SkirmishControl_GroupTier_StructVal`) of
    /// `SkirmishControl_GroupTier_Array`: group size → size band used for scaling.
    pub group_tiers: Vec<(Option<u32>, Option<u32>)>,
    /// Whether `SkirmishControl_ZoneEvents` is stored.
    pub has_zone_events: bool,
    /// `SkirmishControl_SkirmishPointToken`.
    pub point_token: Option<Did>,
    /// `SkirmishControl_SkirmishPanel_RequiredTrait`.
    pub panel_required_trait: Option<Did>,
    /// (`SkirmishControl_SkirmishTraitGate_MaxRank`,
    /// `SkirmishControl_SkirmishTraitGate_MaxRank_WebstoreItem`) of each trait gate.
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
    /// (`SkirmishControl_SkirmishStat`, `SkirmishControl_GRSEnum_PropertyName`) of the
    /// statistic-to-GRS hash.
    pub stat_to_grs: Vec<(Option<u32>, Option<u32>)>,
    /// The conversion hash.
    pub conversion: Vec<ConversionEntry>,
    /// `SkirmishStats_CONSTANT_FASTEST_POSSIBLE_SKIRMISH_TIME`.
    pub fastest_possible_time: Option<f32>,
    /// `Skirmish_Stats` (entries with a malformed scale left out, [`DerivedStat::read`]).
    pub derived_stats: Vec<DerivedStat>,
}

impl SkirmishControl {
    /// Reads the control fields of `set`.
    #[must_use]
    pub fn read(set: &PropertySet, v: &InstanceVocab) -> Self {
        let pairs = |array: &str, key: &str| -> Vec<(u32, f32)> {
            structs(v.elements(set, array))
                .filter_map(|s| Some((v.enum_key(s, key)?, v.float(s, l::SC_POINT_MULT)?)))
                .collect()
        };
        Self {
            difficulty_multipliers: pairs(l::SC_DIFFICULTY_MULT, l::SC_DIFF_TIER),
            group_size_multipliers: pairs(l::SC_GROUP_SIZE_MULT, l::SC_GROUP_SIZE),
            group_tiers: structs(v.elements(set, l::SC_GROUP_TIERS))
                .map(|s| (v.enum_key(s, l::SC_GT_SIZE), v.enum_key(s, l::SC_GT_TIER)))
                .collect(),
            has_zone_events: v.has(set, l::SC_ZONE_EVENTS),
            point_token: v.did(set, l::SC_POINT_TOKEN),
            panel_required_trait: v.did(set, l::SC_PANEL_TRAIT),
            trait_gates: structs(v.elements(set, l::SC_TRAIT_GATES))
                .map(|s| (v.int(s, l::SC_GATE_MAX_RANK), v.did(s, l::SC_GATE_WEBSTORE)))
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
                    (
                        v.enum_key(s, l::SC_STAT),
                        v.pid_value(s, l::SC_GRS_PROPERTY),
                    )
                })
                .collect(),
            conversion: structs(v.elements(set, l::SC_CONVERSION))
                .map(|s| ConversionEntry {
                    property: v.pid_value(s, l::CV_PROPERTY),
                    stat: v.enum_key(s, l::CV_STAT),
                    group_size: v.enum_key(s, l::CV_SIZE),
                    on_level: v.flag(s, l::CV_ON_LEVEL),
                })
                .collect(),
            fastest_possible_time: v.float(set, l::SC_FASTEST),
            derived_stats: structs(v.elements(set, l::SC_STATS))
                .filter_map(|s| DerivedStat::read(s, v))
                .collect(),
        }
    }

    /// The size band of group size `size` (`SkirmishControl_GroupTier_Array`), if listed.
    #[must_use]
    pub fn group_tier(&self, size: u32) -> Option<u32> {
        self.group_tiers
            .iter()
            .find(|(s, _)| *s == Some(size))
            .and_then(|(_, t)| *t)
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
