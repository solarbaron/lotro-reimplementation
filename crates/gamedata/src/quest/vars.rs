// SPDX-License-Identifier: GPL-3.0-or-later
//! The local player's values that texts receive: the 20 player variables of
//! `npc-windows.md` §5.5 (Tessera rule: values from the local character, surname, rank and
//! titles empty) and the player's reported properties (`quest-journal.md` §4.4).

use std::collections::BTreeMap;

use tessera_text::{Value, Values};

use super::data::PresentationData;
use super::ids;

/// What the client knows of the local character (`npc-windows.md` §5.5 Tessera rule;
/// `quest-journal.md` §3, §4.4). Every field is optional: an unknown value leaves its
/// variables unset, and a text that needs one does not render (`string-table.md` §8.3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerContext {
    /// The entered character's name.
    pub name: Option<String>,
    /// `Agent_Species` key (from `CharacterInfo`).
    pub species: Option<u32>,
    /// `Sex` key (from `CharacterInfo`).
    pub sex: Option<u32>,
    /// `Agent_Class` key (from `CharacterInfo`).
    pub class: Option<u32>,
    /// Level (latest `LevelChanged` or `CharacterInfo`).
    pub level: Option<i32>,
    /// `Character_SpeciesNameOverride`, when the client knows one.
    pub species_name_override: Option<String>,
    /// Integer properties of the player entity reported by `PropertyChanged`, by PID
    /// (challenge-level overrides, `Quest_MaxActiveQuests`, category limits).
    pub properties: BTreeMap<u32, i64>,
}

impl PlayerContext {
    /// The player's value of an integer property: a reported value, or for
    /// `Advancement_Level` (`level_pid`) the reported level (`quest-journal.md` §4.4).
    #[must_use]
    pub fn int_property(&self, pid: u32, level_pid: Option<u32>) -> Option<i64> {
        self.properties.get(&pid).copied().or_else(|| {
            (Some(pid) == level_pid)
                .then_some(self.level.map(i64::from))
                .flatten()
        })
    }
}

/// The player variables for one [`PlayerContext`], ready to merge into a text's values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerVars {
    /// The context they were built from.
    pub context: PlayerContext,
    /// The variable values (`npc-windows.md` §5.5 table).
    pub values: Values,
}

impl PlayerVars {
    /// Builds the variables. Species and class names are the enum display texts of
    /// `Agent_Species` and `Agent_Class`; the two enum arguments (species, sex) are given as
    /// their display texts (`tessera-text` has no enum value; see the OPEN in
    /// `npc-windows.md` §5.5).
    #[must_use]
    pub fn new(data: &dyn PresentationData, context: PlayerContext) -> Self {
        let mut values = Values::new();
        if let Some(name) = &context.name {
            for id in ids::PLAYER_NAME {
                values.insert(id, name.as_str());
            }
        }
        let species_pid = data.pid("Agent_Species");
        let species_text = context
            .species
            .and_then(|k| species_pid.and_then(|p| data.enum_text(p, k)));
        let species_name = context
            .species_name_override
            .clone()
            .or_else(|| species_text.clone());
        if let Some(s) = species_name {
            values.insert(ids::PLAYER_SPECIES_NAME, s);
        }
        if let Some(s) = species_text {
            values.insert(ids::PLAYER_SPECIES_ENUM, s);
        }
        if let Some(c) = context
            .class
            .and_then(|k| data.pid("Agent_Class").and_then(|p| data.enum_text(p, k)))
        {
            for id in ids::PLAYER_CLASS_NAME {
                values.insert(id, c.as_str());
            }
        }
        if let Some(s) = context
            .sex
            .and_then(|k| data.pid("Sex").and_then(|p| data.enum_text(p, k)))
        {
            for id in ids::PLAYER_SEX_ENUM {
                values.insert(id, s.as_str());
            }
        }
        if let Some(l) = context.level {
            values.insert(ids::PLAYER_LEVEL, Value::Integer(i64::from(l)));
        }
        for id in ids::PLAYER_EMPTY {
            values.insert(id, "");
        }
        Self { context, values }
    }

    /// `values` plus the player variables (`values` win).
    #[must_use]
    pub fn with(&self, values: &Values) -> Values {
        let mut out = values.clone();
        for (id, v) in self.values.iter() {
            if out.get(id).is_none() {
                out.insert(id, v.clone());
            }
        }
        out
    }
}

/// Whether `id` is one of the 20 player variables of `npc-windows.md` §5.5.
#[must_use]
pub fn is_player_variable(id: u32) -> bool {
    ids::PLAYER_NAME.contains(&id)
        || ids::PLAYER_CLASS_NAME.contains(&id)
        || ids::PLAYER_SEX_ENUM.contains(&id)
        || ids::PLAYER_EMPTY.contains(&id)
        || [
            ids::PLAYER_SPECIES_NAME,
            ids::PLAYER_SPECIES_ENUM,
            ids::PLAYER_LEVEL,
        ]
        .contains(&id)
}

/// Whether `id` is a counter ID of `quest-journal.md` §5.4.
#[must_use]
pub fn is_counter(id: u32) -> bool {
    ids::COUNTERS.contains(&id)
}
