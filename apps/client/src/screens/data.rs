// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the screens read from the install, loaded once: the creation lists of
//! `tessera-gamedata` ([`ChargenData`]), the name generator and check ([`NameData`]), and
//! each player avatar's control details ([`AvatarDetail`]).

use std::collections::BTreeMap;

use tessera_formats::{Did, StringInfo};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::chargen::{ChargenData, NameData, load_chargen, load_names};
use tessera_props::{EnumSource, GameData, PropertyRegistry, RecordSource};
use tessera_text::{RenderOptions, Strings, Values};

use super::controls::{AvatarDetail, ScreenPids, read_avatar_detail};
use super::model::Stage;

/// The `CharGen_Stage` keys the screens' figures carry (`character-screens.md` §2.2;
/// `appearance.md` §3.5: "the code of the character screens sets `CharGen_Stage` on the
/// figures it shows"). Read at run time from the property's enum mapper by the labels
/// `CharacterSelect`, `RaceClass` and `Appearance`; a label the install does not resolve
/// keeps the key §2.2 records (4, 7, 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageKeys {
    /// `CharacterSelect`.
    pub select: u32,
    /// `RaceClass`.
    pub race_class: u32,
    /// `Appearance`.
    pub appearance: u32,
}

impl Default for StageKeys {
    fn default() -> Self {
        Self {
            select: 4,
            race_class: 7,
            appearance: 6,
        }
    }
}

impl StageKeys {
    /// The keys of the mapper that labels `CharGen_Stage` in `gd`'s dictionary (module
    /// docs); the recorded keys where it does not resolve (logged).
    #[must_use]
    pub fn resolve<S: RecordSource>(gd: &GameData<S>, reg: &PropertyRegistry) -> Self {
        let mut keys = Self::default();
        let mapper = reg
            .pid("CharGen_Stage")
            .and_then(|p| reg.enum_source(p))
            .and_then(|s| match s {
                EnumSource::Mapper(m) => gd.enum_mapper(m).ok().flatten(),
                EnumSource::DidMap(_) => None,
            });
        let Some(mapper) = mapper else {
            tracing::debug!("CharGen_Stage has no enum mapper; the recorded stage keys are used");
            return keys;
        };
        for (label, slot) in [
            ("CharacterSelect", &mut keys.select),
            ("RaceClass", &mut keys.race_class),
            ("Appearance", &mut keys.appearance),
        ] {
            match mapper.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), label) {
                Some(k) => *slot = k,
                None => tracing::debug!(label, "CharGen_Stage label not found; recorded key used"),
            }
        }
        keys
    }

    /// The key of `stage`.
    #[must_use]
    pub fn of(&self, stage: Stage) -> u32 {
        match stage {
            Stage::Select => self.select,
            Stage::RaceClass => self.race_class,
            Stage::Appearance => self.appearance,
        }
    }
}

/// Why the screens' data could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ScreenDataError {
    /// The creation lists could not be read (no race directory, unreadable dictionary).
    #[error("creation data: {0}")]
    Chargen(#[from] tessera_gamedata::GameDataError),
}

/// The data of the selection and creation screens.
#[derive(Debug, Clone, Default)]
pub struct ScreenData {
    /// Races, genders, classes, nationalities and the control lists.
    pub chargen: ChargenData,
    /// The random-name generator and the name check; `None` without a generator (names are
    /// then only checked for being non-empty and valid for the session).
    pub names: Option<NameData>,
    /// Control details per player avatar (`RaceTable_Gender_PlayerAvatar`).
    pub avatars: BTreeMap<Did, AvatarDetail>,
    /// Property IDs by name.
    pub pids: ScreenPids,
    /// The `CharGen_Stage` keys of the screens' figures.
    pub stage_keys: StageKeys,
}

/// The plain text of `info` in the install's strings (`None` when it does not resolve
/// cleanly).
#[must_use]
pub fn resolve_text(strings: &Strings, info: &StringInfo) -> Option<String> {
    let r = strings.resolve(info, &Values::new(), &RenderOptions::default());
    r.is_clean().then(|| r.plain_text())
}

/// The screens' texts from an install: string references through its string tables, and
/// session texts naming a record (`Text::Name`) as the record's display name
/// (`GameDataStore::display_name`). Other text kinds resolve to nothing (the error line
/// shows Tessera's placeholder).
#[derive(Debug, Clone, Copy)]
pub struct InstallTexts<'a, S: RecordSource> {
    /// The install's records.
    pub store: &'a GameDataStore<S>,
    /// The install's string tables.
    pub strings: &'a Strings,
}

impl<S: RecordSource> super::ui::ScreenTexts for InstallTexts<'_, S> {
    fn text(&self, info: &StringInfo) -> Option<String> {
        resolve_text(self.strings, info)
    }

    fn reason(&self, text: &tessera_net::domain::Text) -> Option<String> {
        match text {
            tessera_net::domain::Text::Literal(s) => Some(s.clone()),
            tessera_net::domain::Text::Name(d) => self.store.display_name(Did(d.0), None),
            _ => None,
        }
    }
}

impl ScreenData {
    /// Loads the screens' data from `store`; `strings` resolves the name generator's
    /// syllables (without it, or without a generator, no random names are offered).
    ///
    /// # Errors
    ///
    /// [`ScreenDataError::Chargen`] when the creation lists cannot be read at all. A missing
    /// name generator, an unreadable avatar or control record is not an error (logged; the
    /// screens offer less).
    pub fn load<S: RecordSource>(
        store: &GameDataStore<S>,
        strings: Option<&Strings>,
    ) -> Result<Self, ScreenDataError> {
        let objects = store.object_store();
        let chargen = load_chargen(objects)?;
        let names = match strings {
            Some(st) => match load_names(objects, |i| resolve_text(st, i)) {
                Ok(n) => Some(n),
                Err(e) => {
                    tracing::warn!("random names unavailable: {e}");
                    None
                }
            },
            None => None,
        };
        let gd = objects.game_data();
        let pids = gd
            .registry()
            .map(|r| ScreenPids::resolve(r))
            .unwrap_or_default();
        let stage_keys = gd
            .registry()
            .map(|r| StageKeys::resolve(gd, r))
            .unwrap_or_default();
        let mut avatars = BTreeMap::new();
        for (avatar, controls) in &chargen.controls {
            if let Ok(c) = controls {
                let d = read_avatar_detail(gd, c);
                for n in &d.notes {
                    tracing::debug!(avatar = %avatar, "{n}");
                }
                avatars.insert(*avatar, d);
            }
        }
        Ok(Self {
            chargen,
            names,
            avatars,
            pids,
            stage_keys,
        })
    }

    /// The control details of (`species`, `sex`)'s player avatar.
    #[must_use]
    pub fn avatar(&self, species: u32, sex: u32) -> Option<&AvatarDetail> {
        let a = self
            .chargen
            .race_with_gender(species, sex)?
            .gender(sex)?
            .player_avatar?;
        self.avatars.get(&a)
    }
}
