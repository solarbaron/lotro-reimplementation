// SPDX-License-Identifier: GPL-3.0-or-later
//! Character selection and creation data (decision 0024 P1): race, gender, class and
//! nationality lists, appearance-control lists, name generation and name checks read from
//! the install for the screens before world entry.
//!
//! Spec: `docs/specs/character-screens.md` §3–§5, §9; `docs/specs/player-character.md`
//! §5, §8; `docs/specs/appearance.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ChargenData`], [`RaceChoice`], [`GenderChoice`], [`ClassChoice`], [`load_chargen`] | Race, gender, class and nationality lists in screen order; defaults of the flow | `character-screens.md` §4, §5.1, §6; `player-character.md` §5–§7 |
//! | [`ChargenData::check`], [`ChargenData::validate_creation`], [`CreationRequest`], [`ChargenData::problems`] | The creation check, the request the screen sends, CSI2–CSI5 | `player-character.md` §5.4; `character-screens.md` §7.1, §10 |
//! | [`AvatarControls`], [`AppearanceControl`], [`ControlKind`], [`group_controls`] | Appearance controls per (race, gender) | §5.2; `appearance.md` §3.4 |
//! | [`NameData`], [`NamePolicy`], [`check_characters`], [`check_name_rules`], [`load_names`] | Random names and name checks | `player-character.md` §8.5, §8.6; `misc-records.md` §9 |
//!
//! # Hostile input
//!
//! No byte parser lives here; records are decoded by `tessera-formats`,
//! `tessera-props` and `tessera-wstate` within their bounds. Each view list is bounded by
//! `tessera_wstate::character::MAX_VIEW_ENTRIES`; [`load_chargen`] reads each listed table
//! once and keeps at most [`MAX_RACES`] races with [`MAX_GENDERS_PER_RACE`] gender entries
//! and [`MAX_RACE_LIST_ENTRIES`] class and nationality keys each, which bounds the derived
//! products ([`ChargenData::offered`]); controls are bounded by [`MAX_CONTROLS`]; name
//! checks are linear in the name times the rule set's sizes. One broken record is skipped
//! and reported ([`ChargenProblem::RecordUnreadable`]), never an error of the whole load.

mod appearance;
mod load;
mod model;
mod names;

#[cfg(test)]
mod tests;

pub use appearance::{
    AVATAR_UPDATE_TOGGLE_SPECIES, AppearanceControl, AppearancePids, AvatarControls, ControlGroup,
    ControlKind, MAX_CONTROLS, creation_controls, group_controls, read_controls,
    shows_avatar_update_toggle,
};
pub use load::{
    MAX_GENDERS_PER_RACE, MAX_RACE_LIST_ENTRIES, MAX_RACES, load_chargen, load_name_filter,
    load_names,
};
pub use model::{
    ChargenData, ChargenProblem, ClassChoice, CreationRefusal, CreationRequest, GenderChoice,
    RaceChoice, keeps_appearance,
};
pub use names::{
    NameCheckFailure, NameData, NamePolicy, VOWELS, char_allowed, check_characters,
    check_name_rules,
};

/// Types of `tessera-wstate` and `tessera-formats` that appear in this module's API.
pub use tessera_formats::gameplay::misc::{CharacterClass, NameFilterTable, NameRuleSet};
pub use tessera_wstate::character::{
    ChoiceVerdict, ClassPriority, CreationChoice, DyeOverride, GenderEntry, GeneratedName,
    NationalityRecord, RaceRecord, RaceTrait,
};
