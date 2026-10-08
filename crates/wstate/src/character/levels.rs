// SPDX-License-Identifier: GPL-3.0-or-later
//! Experience, levels and level controls (`player-character.md` §9).

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet};

use super::{Fields, check_len};
use crate::error::Result;
use crate::state::ObjectState;

/// XP globals (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct XpGlobals {
    /// `XPGlobals_AbsoluteMaxLevel`.
    pub absolute_max_level: Option<i32>,
    /// `XPGlobals_AbsoluteMaxItemLevel`.
    pub absolute_max_item_level: Option<i32>,
    /// `XPGlobals_GroupXPModifierTable` (§9.3).
    pub group_modifier_table: Option<Did>,
    /// `XPGlobals_EffectiveContribution_MaxLevelDistance`.
    pub contribution_max_level_distance: Option<i32>,
    /// `XPGlobals_Contribution_AllowedLevelDifference`.
    pub contribution_allowed_level_difference: Option<i32>,
}

impl XpGlobals {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "XP globals");
        Ok(Self {
            absolute_max_level: f.int("XPGlobals_AbsoluteMaxLevel")?,
            absolute_max_item_level: f.int("XPGlobals_AbsoluteMaxItemLevel")?,
            group_modifier_table: f.did("XPGlobals_GroupXPModifierTable")?,
            contribution_max_level_distance: f
                .int("XPGlobals_EffectiveContribution_MaxLevelDistance")?,
            contribution_allowed_level_difference: f
                .int("XPGlobals_Contribution_AllowedLevelDifference")?,
        })
    }
}

/// Rested-experience control (§9.4; units and rule OPEN).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RestedExperienceControl {
    /// `XPBonusControl_DailyGlobalResetTime`.
    pub daily_reset_time: Option<i32>,
    /// `XPBonusControl_DailyRestPercentage`.
    pub daily_rest_percentage: Option<f32>,
    /// `XPBonusControl_MaxRestPercentage`.
    pub max_rest_percentage: Option<f32>,
    /// `XPBonusControl_Multiplier`.
    pub multiplier: Option<f32>,
}

impl RestedExperienceControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "rested-experience control");
        Ok(Self {
            daily_reset_time: f.int("XPBonusControl_DailyGlobalResetTime")?,
            daily_rest_percentage: f.float("XPBonusControl_DailyRestPercentage")?,
            max_rest_percentage: f.float("XPBonusControl_MaxRestPercentage")?,
            multiplier: f.float("XPBonusControl_Multiplier")?,
        })
    }
}

/// An experience table (§9.2): the `array_u64` of the compiled half.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExperienceTable {
    /// Element *k*: total experience needed to be at level (or rank) *k* (Reported rule).
    pub totals: Vec<u64>,
}

impl ExperienceTable {
    /// Reads the first `array_u64` item of the compiled half; `None` if it has none.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Option<Self>> {
        let Some(v) = state
            .items()
            .iter()
            .find_map(|i| i.as_native().and_then(|n| n.as_array_u64()))
        else {
            return Ok(None);
        };
        check_len("experience table", v.len())?;
        Ok(Some(Self { totals: v.to_vec() }))
    }

    /// Whether element 0 is 0 and the elements never decrease (PC12).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.totals.first() == Some(&0)
            && self
                .totals
                .windows(2)
                .all(|w| matches!(w, [a, b] if a <= b))
    }

    /// The level of a character with `xp` total experience by the Reported index rule
    /// (§9.2): the highest *k* ≥ 1 whose element is ≤ `xp`. `None` when no such *k*
    /// exists (a table shorter than two elements, or element 1 above `xp`; what the
    /// original shows then is OPEN, §9.2).
    #[must_use]
    pub fn level_for(&self, xp: u64) -> Option<usize> {
        self.totals
            .iter()
            .enumerate()
            .skip(1)
            .filter(|&(_, &t)| t <= xp)
            .map(|(k, _)| k)
            .next_back()
    }

    /// The highest level the table defines (its length minus one), if any.
    #[must_use]
    pub fn max_level(&self) -> Option<usize> {
        self.totals.len().checked_sub(1).filter(|&k| k >= 1)
    }
}

/// A group modifier table (§9.3): key → factor, from the compiled half's `map_u32` (the
/// value is a 32-bit float stored as its bit pattern).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupModifierTable {
    /// (key, factor), stored order.
    pub factors: Vec<(u32, f32)>,
}

impl GroupModifierTable {
    /// Reads the first `map_u32` item of the compiled half; `None` if it has none.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Option<Self>> {
        let Some(v) = state
            .items()
            .iter()
            .find_map(|i| i.as_native().and_then(|n| n.as_map_u32()))
        else {
            return Ok(None);
        };
        check_len("group modifier table", v.len())?;
        Ok(Some(Self {
            factors: v
                .iter()
                .map(|&(k, bits)| (k, f32::from_bits(bits)))
                .collect(),
        }))
    }

    /// The factor for `key`.
    #[must_use]
    pub fn factor(&self, key: u32) -> Option<f32> {
        self.factors
            .iter()
            .find(|(k, _)| *k == key)
            .map(|&(_, f)| f)
    }
}
