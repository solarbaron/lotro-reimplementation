// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the screens read from the install, loaded once: the creation lists of
//! `tessera-gamedata` ([`ChargenData`]), the name generator and check ([`NameData`]), and
//! each player avatar's control details ([`AvatarDetail`]).

use std::collections::BTreeMap;

use tessera_formats::{Did, StringInfo};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::chargen::{ChargenData, NameData, load_chargen, load_names};
use tessera_props::RecordSource;
use tessera_text::{RenderOptions, Strings, Values};

use super::controls::{AvatarDetail, ScreenPids, read_avatar_detail};

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
