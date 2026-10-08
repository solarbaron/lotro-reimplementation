// SPDX-License-Identifier: GPL-3.0-or-later
//! Bank repositories and upgrade schedules (§11.3), allegiances (§12.4) of
//! `player-character.md`.

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet};

use super::Fields;
use crate::error::Result;

/// One `BankRepository_HousingUpgradeEntry` (§11.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HousingUpgrade {
    /// `BankRepository_UpgradeProgression`.
    pub schedule: Option<Did>,
    /// `BankRepository_HousingUpgradeTypeArray`: house-type keys.
    pub house_types: Vec<u32>,
}

/// A bank repository (classes of §11.3: account, character and housing storage).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BankRepository {
    /// `BankRepository_BaseCapacity`.
    pub base_capacity: Option<i32>,
    /// `BankRepository_MaxCapacity`.
    pub max_capacity: Option<i32>,
    /// `Bank_PolicyProhibitionProperty`.
    pub prohibition_property: Option<u32>,
    /// `Bank_PolicyProhibitionError`.
    pub prohibition_error: Option<u32>,
    /// `BankRepository_UpgradeProgression` (character bank).
    pub schedule: Option<Did>,
    /// `BankRepository_HousingUpgradeArray` (shared housing).
    pub housing_upgrades: Vec<HousingUpgrade>,
}

impl BankRepository {
    /// Reads a repository's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type;
    /// [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "bank repository");
        Ok(Self {
            base_capacity: f.int("BankRepository_BaseCapacity")?,
            max_capacity: f.int("BankRepository_MaxCapacity")?,
            prohibition_property: f.pid("Bank_PolicyProhibitionProperty")?,
            prohibition_error: f.enum_("Bank_PolicyProhibitionError")?,
            schedule: f.did("BankRepository_UpgradeProgression")?,
            housing_upgrades: f.struct_list("BankRepository_HousingUpgradeArray", |e| {
                Ok(HousingUpgrade {
                    schedule: e.did("BankRepository_UpgradeProgression")?,
                    house_types: e.enum_list("BankRepository_HousingUpgradeTypeArray")?,
                })
            })?,
        })
    }

    /// Every upgrade schedule the repository names, stored order.
    #[must_use]
    pub fn schedules(&self) -> Vec<Did> {
        self.schedule
            .into_iter()
            .chain(self.housing_upgrades.iter().filter_map(|h| h.schedule))
            .collect()
    }
}

/// One `BankRepository_Upgrade_Struct` (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpgradeStep {
    /// `BankRepository_Upgrade_RequiredCapacity`.
    pub required_capacity: Option<i32>,
    /// `BankRepository_Upgrade_IncreasedCapacity`.
    pub increase: Option<i32>,
    /// `BankRepository_Upgrade_Cost`.
    pub cost: Option<i32>,
}

/// A bank upgrade schedule (§11.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BankUpgradeSchedule {
    /// `BankRepository_UpgradeProgression_Array`, stored order.
    pub steps: Vec<UpgradeStep>,
}

impl BankUpgradeSchedule {
    /// Reads a schedule's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type;
    /// [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "bank upgrade schedule");
        Ok(Self {
            steps: f.struct_list("BankRepository_UpgradeProgression_Array", |s| {
                Ok(UpgradeStep {
                    required_capacity: s.int("BankRepository_Upgrade_RequiredCapacity")?,
                    increase: s.int("BankRepository_Upgrade_IncreasedCapacity")?,
                    cost: s.int("BankRepository_Upgrade_Cost")?,
                })
            })?,
        })
    }

    /// Whether every step's required capacity is the previous step's required capacity
    /// plus the previous step's increase (PC15). Absent values fail the check.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.steps.windows(2).all(|w| match w {
            [a, b] => match (a.required_capacity, a.increase, b.required_capacity) {
                (Some(r), Some(i), Some(next)) => i64::from(r) + i64::from(i) == i64::from(next),
                _ => false,
            },
            _ => true,
        })
    }

    /// The capacity reached after the last step (required capacity + increase).
    #[must_use]
    pub fn final_capacity(&self) -> Option<i64> {
        let last = self.steps.last()?;
        Some(i64::from(last.required_capacity?) + i64::from(last.increase?))
    }
}

/// One `Allegiance_Group_Entry` (§12.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AllegianceGroup {
    /// `Allegiance_Group_Name`.
    pub name: Option<u32>,
    /// `Allegiance_Group_MinLevelToStart`.
    pub min_level: Option<i32>,
    /// `Allegiance_Group_Array`: the group's allegiances.
    pub allegiances: Vec<Did>,
}

/// Allegiance control (§12.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AllegianceControl {
    /// `Allegiance_Control_UIVisibilityLevelRequirement`.
    pub visibility_level: Option<i32>,
    /// `Allegiance_Type_Array`.
    pub types: Vec<Did>,
    /// `Allegiance_Advancement_Progressions_Array`.
    pub progressions: Vec<Did>,
    /// `Allegiance_Group_Table`.
    pub groups: Vec<AllegianceGroup>,
}

impl AllegianceControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type;
    /// [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "allegiance control");
        Ok(Self {
            visibility_level: f.int("Allegiance_Control_UIVisibilityLevelRequirement")?,
            types: f.did_list("Allegiance_Type_Array")?,
            progressions: f.did_list("Allegiance_Advancement_Progressions_Array")?,
            groups: f.struct_list("Allegiance_Group_Table", |g| {
                Ok(AllegianceGroup {
                    name: g.enum_("Allegiance_Group_Name")?,
                    min_level: g.int("Allegiance_Group_MinLevelToStart")?,
                    allegiances: g.did_list("Allegiance_Group_Array")?,
                })
            })?,
        })
    }
}

/// An allegiance (§12.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Allegiance {
    /// `Allegiance_Groups`.
    pub group: Option<u32>,
    /// `Allegiance_MinimumLeveltoStart`.
    pub min_level: Option<i32>,
    /// `Allegiance_Advancement_Progression`: an experience table.
    pub progression: Option<Did>,
    /// `Allegiance_DeedList`.
    pub deeds: Vec<Did>,
    /// `Allegiance_TravelSkill`.
    pub travel_skill: Option<Did>,
}

impl Allegiance {
    /// Reads an allegiance's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type;
    /// [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "allegiance");
        Ok(Self {
            group: f.enum_("Allegiance_Groups")?,
            min_level: f.int("Allegiance_MinimumLeveltoStart")?,
            progression: f.did("Allegiance_Advancement_Progression")?,
            deeds: f.did_list("Allegiance_DeedList")?,
            travel_skill: f.did("Allegiance_TravelSkill")?,
        })
    }
}

/// The ways the control and the allegiances disagree (PC16, §12.4): the type array must
/// equal the union of the group arrays, and each allegiance's group and minimum level
/// must equal its group's name and minimum level. Empty when they agree.
#[must_use]
pub fn allegiance_problems(
    control: &AllegianceControl,
    allegiances: &[(Did, Allegiance)],
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut union: Vec<Did> = control
        .groups
        .iter()
        .flat_map(|g| g.allegiances.iter().copied())
        .collect();
    union.sort_unstable();
    union.dedup();
    let mut types = control.types.clone();
    types.sort_unstable();
    types.dedup();
    if union != types {
        problems.push("type array differs from the union of the group arrays".to_owned());
    }
    for (did, a) in allegiances {
        let Some(group) = control.groups.iter().find(|g| g.allegiances.contains(did)) else {
            problems.push(format!("{did}: in no group"));
            continue;
        };
        if a.group != group.name {
            problems.push(format!("{did}: group key differs from its group"));
        }
        if a.min_level != group.min_level {
            problems.push(format!("{did}: minimum level differs from its group"));
        }
    }
    problems
}
