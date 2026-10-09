// SPDX-License-Identifier: GPL-3.0-or-later
//! Player-character data: typed views of the avatar-template, race, nationality, class
//! creation, name, level, pip and account/social control records.
//!
//! Spec: `docs/specs/player-character.md`.
//!
//! None of these records needs a byte decoder of its own (§3): every view here reads an
//! already decoded property half ([`tessera_props::PropertySet`], the object's set at
//! DID + 0x09000000) or, for the experience and group modifier tables (§9.2, §9.3) and the
//! compiled phonetic-shift tables (§8.4), the compiled half ([`crate::ObjectState`]).
//! Property names are looked up in the install's dictionary at run time; no PID, class
//! index, enum key or DID is compiled in.
//!
//! | Item | Records | Spec |
//! |---|---|---|
//! | [`CharacterRecords`], [`Role`] | finding the records from their WEENIECONTENT labels and references | §2.2, §2.3 |
//! | [`mirror_base`], [`default_character_properties`] | mirror properties, a new character's default set | §4.3, §4.6 |
//! | [`RaceDirectory`], [`RaceRecord`], [`GenderEntry`], [`check_choice`] | race directory and tables, creation check | §5 |
//! | [`NationalityDirectory`], [`NationalityRecord`] | nationality tables | §6 |
//! | [`ClassCreationInfo`], [`ClassPriority`], [`LevelTableDirectory`] | class creation fields, class priority | §7 |
//! | [`NameRuleControl`], [`NameGenerator`], [`ShiftTable`], [`generate_name_once`], [`generate_name`] | names, the phonetic-shift rule and its text model, name generation | §8 |
//! | [`XpGlobals`], [`ExperienceTable`], [`GroupModifierTable`], [`RestedExperienceControl`] | experience and levels | §9 |
//! | [`PipControl`], [`Pip`], [`ThresholdPolicy`] | class resources | §10 |
//! | [`BankRepository`], [`BankUpgradeSchedule`], [`AllegianceControl`], [`Allegiance`] | bank schedules, allegiances | §11.3, §12.4 |
//!
//! # Hostile input
//!
//! The views copy scalars and DIDs out of sets that the property layer has already
//! decoded within its own bounds. By Decision D5 (§3.1) every list a view keeps is
//! checked against [`MAX_VIEW_ENTRIES`] before it is collected, and a present property
//! whose type differs from the spec's tables is an error ([`WStateError::BadProperty`]);
//! both are Tessera choices, not client behaviour. Nothing indexes by a data-derived
//! value.

mod controls;
mod levels;
mod names;
mod pips;
mod race;
mod records;

#[cfg(test)]
mod tests;

pub use controls::{
    Allegiance, AllegianceControl, AllegianceGroup, BankRepository, BankUpgradeSchedule,
    HousingUpgrade, UpgradeStep, allegiance_problems,
};
pub use levels::{ExperienceTable, GroupModifierTable, RestedExperienceControl, XpGlobals};
pub use names::{
    GeneratedName, MAX_NAME_ATTEMPTS, MAX_PART_DRAWS, NameCategory, NameGenerator, NameOption,
    NameRuleControl, NameText, RaceCategory, ShiftEntry, ShiftEntryInfo, ShiftGroup, ShiftTable,
    ShiftTableDefinition, ShiftTriple, SyllableTable, generate_name, generate_name_once,
    lower_text, lower_unit, text_eq_ci, upper_unit,
};
pub use pips::{AdjustmentKind, Pip, PipAdjustment, PipControl, ThresholdPolicy, ThresholdState};
pub use race::{
    AdvancedStart, ChoiceVerdict, ClassCreationInfo, ClassPriority, CreationChoice, DyeOverride,
    GenderEntry, LevelTableDirectory, NamingGuideline, NationalityDirectory, NationalityRecord,
    RaceDirectory, RaceRecord, RaceTrait, StartingItem, check_choice, default_character_properties,
};
pub use records::{CONTROL_LABELS, CharacterRecords, Role};

use tessera_formats::{Did, StringInfo};
use tessera_props::{Property, PropertyRegistry, PropertySet, Value};

use crate::error::{Result, WStateError};

/// Most entries a view keeps from one list: Decision D5 of `player-character.md` §3.1
/// (the largest observed list holds 463 elements; the client itself bounds lists only by
/// the record's bytes). `game-controls.md` §3.1 keeps the same cap as a Tessera Decision.
pub const MAX_VIEW_ENTRIES: usize = 1 << 16;

/// The base property name of a mirror property (`creatures.md` §3.2.1, used by
/// `player-character.md` §4.3 and §4.6), or `None` if `name` is not a mirror name.
///
/// The patterns are tried in the order of the spec's table: `Forward_Cached_`*X*,
/// *X*`_Cached`, `ForwardSource_`*X*, *X*`_ForwardSource`, *X*`_Shadow`, *X*`_Base`. A
/// pattern that would leave an empty base does not match.
#[must_use]
pub fn mirror_base(name: &str) -> Option<&str> {
    let non_empty = |s: &str| !s.is_empty();
    if let Some(b) = name
        .strip_prefix("Forward_Cached_")
        .filter(|s| non_empty(s))
    {
        return Some(b);
    }
    if let Some(b) = name.strip_suffix("_Cached").filter(|s| non_empty(s)) {
        return Some(b);
    }
    if let Some(b) = name.strip_prefix("ForwardSource_").filter(|s| non_empty(s)) {
        return Some(b);
    }
    for suffix in ["_ForwardSource", "_Shadow", "_Base"] {
        if let Some(b) = name.strip_suffix(suffix).filter(|s| non_empty(s)) {
            return Some(b);
        }
    }
    None
}

/// Typed access to the properties of one set by dictionary name.
#[derive(Clone, Copy)]
pub(crate) struct Fields<'a> {
    set: &'a PropertySet,
    reg: &'a PropertyRegistry,
    record: &'static str,
}

impl<'a> Fields<'a> {
    pub(crate) fn new(
        set: &'a PropertySet,
        reg: &'a PropertyRegistry,
        record: &'static str,
    ) -> Self {
        Self { set, reg, record }
    }

    /// The same accessors on a struct value's fields.
    pub(crate) fn nested(&self, set: &'a PropertySet) -> Self {
        Self { set, ..*self }
    }

    pub(crate) fn get(&self, name: &str) -> Option<&'a Value> {
        self.set.get_by_name(self.reg, name)
    }

    fn bad(&self, property: &'static str, problem: &'static str) -> WStateError {
        WStateError::BadProperty {
            record: self.record,
            property,
            problem,
        }
    }

    /// The value of `name` converted by `f`; absent is `None`, another type an error.
    pub(crate) fn typed<T>(
        &self,
        name: &'static str,
        f: impl FnOnce(&'a Value) -> Option<T>,
    ) -> Result<Option<T>> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => f(v)
                .map(Some)
                .ok_or_else(|| self.bad(name, "has an unexpected type")),
        }
    }

    pub(crate) fn require<T>(&self, name: &'static str, v: Option<T>) -> Result<T> {
        v.ok_or_else(|| self.bad(name, "is absent"))
    }

    pub(crate) fn enum_(&self, name: &'static str) -> Result<Option<u32>> {
        self.typed(name, Value::as_enum)
    }

    pub(crate) fn int(&self, name: &'static str) -> Result<Option<i32>> {
        self.typed(name, Value::as_i32)
    }

    pub(crate) fn float(&self, name: &'static str) -> Result<Option<f32>> {
        self.typed(name, Value::as_f32)
    }

    pub(crate) fn boolean(&self, name: &'static str) -> Result<Option<bool>> {
        self.typed(name, Value::as_bool)
    }

    pub(crate) fn pid(&self, name: &'static str) -> Result<Option<u32>> {
        self.typed(name, Value::as_pid)
    }

    /// A DID reference; the null DID reads as absent.
    pub(crate) fn did(&self, name: &'static str) -> Result<Option<Did>> {
        Ok(self.typed(name, Value::as_did)?.filter(|d| !d.is_null()))
    }

    pub(crate) fn string_info(&self, name: &'static str) -> Result<Option<StringInfo>> {
        self.typed(name, |v| v.as_string_info().cloned())
    }

    /// Bits of a fixed-width or variable-width bitfield (first 64 bits).
    pub(crate) fn bits(&self, name: &'static str) -> Result<Option<u64>> {
        self.typed(name, |v| {
            v.as_bits().or_else(|| {
                v.as_bitfield_any().map(|b| {
                    let mut out = [0u8; 8];
                    for (o, i) in out.iter_mut().zip(&b.bytes) {
                        *o = *i;
                    }
                    u64::from_le_bytes(out)
                })
            })
        })
    }

    /// The elements of array `name` (absent: empty), checked against
    /// [`MAX_VIEW_ENTRIES`].
    pub(crate) fn list(&self, name: &'static str) -> Result<&'a [Property]> {
        let Some(items) = self.typed(name, Value::as_array)? else {
            return Ok(&[]);
        };
        check_len(name, items.len())?;
        Ok(items)
    }

    /// The fields of struct `name`, if present.
    pub(crate) fn structure(&self, name: &'static str) -> Result<Option<Fields<'a>>> {
        Ok(self.typed(name, Value::as_struct)?.map(|s| self.nested(s)))
    }

    /// Maps every element of array `name` with `f`.
    pub(crate) fn map_list<T>(
        &self,
        name: &'static str,
        mut f: impl FnMut(&'a Value) -> Option<T>,
    ) -> Result<Vec<T>> {
        let items = self.list(name)?;
        let mut out = Vec::with_capacity(items.len());
        for p in items {
            out.push(
                f(&p.value)
                    .ok_or_else(|| self.bad(name, "has an element of an unexpected type"))?,
            );
        }
        Ok(out)
    }

    /// The enum keys of array `name`.
    pub(crate) fn enum_list(&self, name: &'static str) -> Result<Vec<u32>> {
        self.map_list(name, Value::as_enum)
    }

    /// The non-null DIDs of array `name` (null elements are skipped).
    pub(crate) fn did_list(&self, name: &'static str) -> Result<Vec<Did>> {
        Ok(self
            .map_list(name, Value::as_did)?
            .into_iter()
            .filter(|d| !d.is_null())
            .collect())
    }

    /// Maps every struct element of array `name` with `f`.
    pub(crate) fn struct_list<T>(
        &self,
        name: &'static str,
        mut f: impl FnMut(Fields<'a>) -> Result<T>,
    ) -> Result<Vec<T>> {
        let items = self.list(name)?;
        let mut out = Vec::with_capacity(items.len());
        for p in items {
            let s = p
                .value
                .as_struct()
                .ok_or_else(|| self.bad(name, "has an element that is not a struct"))?;
            out.push(f(self.nested(s))?);
        }
        Ok(out)
    }
}

pub(crate) fn check_len(what: &'static str, count: usize) -> Result<()> {
    if count > MAX_VIEW_ENTRIES {
        return Err(WStateError::TooMany {
            what,
            count,
            limit: MAX_VIEW_ENTRIES,
        });
    }
    Ok(())
}
