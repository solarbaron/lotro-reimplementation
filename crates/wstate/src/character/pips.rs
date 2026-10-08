// SPDX-License-Identifier: GPL-3.0-or-later
//! Class resources ("pips", `player-character.md` §10).

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet, Value};

use super::Fields;
use crate::error::Result;

/// Pip control (§10.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PipControl {
    /// `PipControl_Directory`: every pip record.
    pub pips: Vec<Did>,
}

impl PipControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type;
    /// [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "pip control");
        Ok(Self {
            pips: f.did_list("PipControl_Directory")?,
        })
    }
}

/// When an adjustment applies (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdjustmentKind {
    /// `Pip_Adjustment_OutOfCombat`.
    OutOfCombat,
    /// `Pip_Adjustment_InCombat`.
    InCombat,
    /// `Pip_Adjustment_WhileMoving`.
    WhileMoving,
    /// `Pip_Adjustment_StandingStill`.
    StandingStill,
}

impl AdjustmentKind {
    /// All kinds with their property names.
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::OutOfCombat, "Pip_Adjustment_OutOfCombat"),
        (Self::InCombat, "Pip_Adjustment_InCombat"),
        (Self::WhileMoving, "Pip_Adjustment_WhileMoving"),
        (Self::StandingStill, "Pip_Adjustment_StandingStill"),
    ];
}

/// An adjustment struct (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PipAdjustment {
    /// `Pip_Adjustment_Amount` (dictionary default 0 when absent).
    pub amount: Option<i32>,
    /// `Pip_Adjustment_Interval` (seconds by its name; default 0.0 when absent).
    pub interval: Option<f32>,
    /// `Pip_Adjustment_NextTriggerTimestamp`.
    pub next_trigger_property: Option<u32>,
}

/// One `Threshold_EffectScript_State` (§10.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThresholdState {
    /// `Threshold_State_MinValue_Integer`.
    pub min: Option<i32>,
    /// `Threshold_State_MaxValue_Integer`.
    pub max: Option<i32>,
    /// Number of `Threshold_State_EffectStruct` entries.
    pub effect_count: usize,
}

impl ThresholdState {
    /// The band [min, max] with absent bounds read as 0 (§10.3).
    #[must_use]
    pub fn band(&self) -> (i32, i32) {
        (self.min.unwrap_or(0), self.max.unwrap_or(0))
    }
}

/// `Threshold_Pip_IntegerValue_Policy` (§10.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThresholdPolicy {
    /// `Threshold_Policy_CurrentStateIndexProperty`.
    pub state_index_property: Option<u32>,
    /// `Threshold_Policy_StateList`, stored order.
    pub states: Vec<ThresholdState>,
}

impl ThresholdPolicy {
    /// The position (0-based, stored order) of the first state whose band holds `value`.
    ///
    /// This is a position in [`ThresholdPolicy::states`], not the value the original
    /// writes to the state-index property, whose base is OPEN (Q15, §10.3).
    #[must_use]
    pub fn state_position(&self, value: i32) -> Option<usize> {
        self.states.iter().position(|s| {
            let (lo, hi) = s.band();
            lo <= value && value <= hi
        })
    }

    /// Whether the bands are ascending, each with min ≤ max, touching (min = previous max
    /// + 1), the first starting at 0 or 1 and the last reaching `pip_max` (PC13).
    #[must_use]
    pub fn bands_are_consistent(&self, pip_max: i32) -> bool {
        let bands: Vec<(i32, i32)> = self.states.iter().map(ThresholdState::band).collect();
        let (Some(first), Some(last)) = (bands.first(), bands.last()) else {
            return false;
        };
        matches!(first.0, 0 | 1)
            && last.1 >= pip_max
            && bands.iter().all(|(lo, hi)| lo <= hi)
            && bands
                .windows(2)
                .all(|w| matches!(w, [a, b] if i64::from(b.0) == i64::from(a.1) + 1))
    }
}

/// A pip record (§10.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pip {
    /// `Pip_Type`.
    pub pip_type: u32,
    /// `Pip_ValueProperty`: the actor property holding the current value.
    pub value_property: Option<u32>,
    /// `Pip_Min` (absent: −1).
    pub min: Option<i32>,
    /// `Pip_Max` (absent: −1).
    pub max: Option<i32>,
    /// `Pip_Home`.
    pub home: Option<i32>,
    /// `Pip_ErrorType`.
    pub error_type: Option<u32>,
    /// `Pip_MaxModifier`.
    pub max_modifier: Option<u32>,
    /// `Pip_Home_ModProperty`.
    pub home_modifier: Option<u32>,
    /// `Pip_Cashout_Conversion_ModProperty`.
    pub cashout_modifier: Option<u32>,
    /// `Pip_LastValidSelectionProperty`.
    pub last_valid_selection_property: Option<u32>,
    /// `Pip_ResetOnSelectedEnemyChange`.
    pub reset_on_enemy_change: Option<bool>,
    /// Adjustment structs present, in [`AdjustmentKind::ALL`] order.
    pub adjustments: Vec<(AdjustmentKind, PipAdjustment)>,
    /// `Threshold_Pip_IntegerValue_Policy`.
    pub threshold: Option<ThresholdPolicy>,
}

impl Pip {
    /// Reads a pip's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] when `Pip_Type` is absent or a property has an
    /// unexpected type; [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "pip");
        let mut adjustments = Vec::new();
        for (kind, name) in AdjustmentKind::ALL {
            if let Some(a) = f.structure(name)? {
                adjustments.push((
                    kind,
                    PipAdjustment {
                        amount: a.int("Pip_Adjustment_Amount")?,
                        interval: a.float("Pip_Adjustment_Interval")?,
                        next_trigger_property: a.pid("Pip_Adjustment_NextTriggerTimestamp")?,
                    },
                ));
            }
        }
        let threshold = match f.structure("Threshold_Pip_IntegerValue_Policy")? {
            None => None,
            Some(p) => Some(ThresholdPolicy {
                state_index_property: p.pid("Threshold_Policy_CurrentStateIndexProperty")?,
                states: p.struct_list("Threshold_Policy_StateList", |s| {
                    Ok(ThresholdState {
                        min: s.int("Threshold_State_MinValue_Integer")?,
                        max: s.int("Threshold_State_MaxValue_Integer")?,
                        effect_count: s.list("Threshold_State_EffectList")?.len(),
                    })
                })?,
            }),
        };
        Ok(Self {
            pip_type: f.require("Pip_Type", f.enum_("Pip_Type")?)?,
            value_property: f.pid("Pip_ValueProperty")?,
            min: f.int("Pip_Min")?,
            max: f.int("Pip_Max")?,
            home: f.int("Pip_Home")?,
            error_type: f.enum_("Pip_ErrorType")?,
            max_modifier: f.pid("Pip_MaxModifier")?,
            home_modifier: f.pid("Pip_Home_ModProperty")?,
            cashout_modifier: f.pid("Pip_Cashout_Conversion_ModProperty")?,
            last_valid_selection_property: f.pid("Pip_LastValidSelectionProperty")?,
            reset_on_enemy_change: f.boolean("Pip_ResetOnSelectedEnemyChange")?,
            adjustments,
            threshold,
        })
    }

    /// The adjustment of `kind`, if the pip has one.
    #[must_use]
    pub fn adjustment(&self, kind: AdjustmentKind) -> Option<&PipAdjustment> {
        self.adjustments
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, a)| a)
    }

    /// The effective maximum for an actor (§10.4): `Pip_Max` (−1 if absent); if
    /// `Pip_MaxModifier` names a property (not 0), lowered by the actor's value of it when
    /// that value is an `int` (otherwise 0), never raised, and raised back to `Pip_Min`
    /// (−1 if absent) if it falls below.
    ///
    /// `actor` returns the actor's value of a property ID.
    #[must_use]
    pub fn effective_max<'v>(&self, actor: impl FnOnce(u32) -> Option<&'v Value>) -> i32 {
        let m = self.max.unwrap_or(-1);
        let Some(pid) = self.max_modifier.filter(|&p| p != 0) else {
            return m;
        };
        let v = actor(pid).and_then(Value::as_i32).unwrap_or(0);
        m.min(m.saturating_add(v)).max(self.min.unwrap_or(-1))
    }
}
