// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the screens read from the install, loaded once: the creation lists of
//! `tessera-gamedata` ([`ChargenData`]), the name generator and check ([`NameData`]), and
//! each player avatar's control details ([`AvatarDetail`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::{Did, StringInfo};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::chargen::{ChargenData, NameData, load_chargen, load_names};
use tessera_props::{EnumSource, GameData, PropertyRegistry, RecordSource};
use tessera_text::{RenderOptions, Strings, Values};

use super::controls::{AvatarDetail, ScreenPids, read_avatar_detail};
use super::model::Stage;
use super::scene::{SceneKeys, StageScene, load_scene};

/// Most group display texts kept (the data has 9).
pub const MAX_GROUPS: usize = 256;

/// The `CharGen_Stage` and `CharGen_Action` keys the screens' figures carry
/// (`character-screens.md` §2.1.4: the selection figure 4 `CharacterSelect`, the
/// race-and-class figure 1 `ClassSelect`, the appearance-stage figure 6 `Appearance`;
/// `CharGen_Action` 2 `Idle` and 3 `Hover`). Read at run time from the properties' enum
/// mappers by label; a label the install does not resolve keeps the key §2.1.4 records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageKeys {
    /// `CharacterSelect`.
    pub select: u32,
    /// `ClassSelect`: the race-and-class figure's key (not `RaceClass`, which names a
    /// layout stage only).
    pub class_select: u32,
    /// `Appearance`.
    pub appearance: u32,
    /// `CharGen_Action` `Idle`.
    pub idle: u32,
    /// `CharGen_Action` `Hover`.
    pub hover: u32,
}

impl Default for StageKeys {
    fn default() -> Self {
        Self {
            select: 4,
            class_select: 1,
            appearance: 6,
            idle: 2,
            hover: 3,
        }
    }
}

fn mapper_of<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    property: &str,
) -> Option<std::sync::Arc<tessera_formats::EnumMapper>> {
    let pid = reg.pid(property)?;
    match reg.enum_source(pid)? {
        EnumSource::Mapper(m) => gd.enum_mapper(m).ok().flatten(),
        EnumSource::DidMap(_) => None,
    }
}

impl StageKeys {
    /// The keys of the mappers that label `CharGen_Stage` and `CharGen_Action` in `gd`'s
    /// dictionary (module docs); the recorded keys where they do not resolve (logged).
    #[must_use]
    pub fn resolve<S: RecordSource>(gd: &GameData<S>, reg: &PropertyRegistry) -> Self {
        let d = Self::default();
        let stage = mapper_of(gd, reg, "CharGen_Stage");
        let action = mapper_of(gd, reg, "CharGen_Action");
        let key = |m: &Option<std::sync::Arc<tessera_formats::EnumMapper>>,
                   label: &str,
                   recorded: u32| {
            match m
                .as_ref()
                .and_then(|m| m.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), label))
            {
                Some(k) => k,
                None => {
                    tracing::debug!(label, "stage or action label not found; recorded key used");
                    recorded
                }
            }
        };
        Self {
            select: key(&stage, "CharacterSelect", d.select),
            class_select: key(&stage, "ClassSelect", d.class_select),
            appearance: key(&stage, "Appearance", d.appearance),
            idle: key(&action, "Idle", d.idle),
            hover: key(&action, "Hover", d.hover),
        }
    }

    /// The `CharGen_Stage` key of the figure of `stage` (§2.1.4).
    #[must_use]
    pub fn of(&self, stage: Stage) -> u32 {
        match stage {
            Stage::Select => self.select,
            Stage::RaceClass => self.class_select,
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
    /// The `CharGen_Stage` and `CharGen_Action` keys of the screens' figures.
    pub stage_keys: StageKeys,
    /// The race scenes by DID (`character-screens.md` §2.1.1–§2.1.3), those that load.
    pub scenes: BTreeMap<Did, Arc<StageScene>>,
    /// The display text of each `AppearanceUI_Group` key (§5.2.2 rule 3).
    pub group_names: BTreeMap<u32, StringInfo>,
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

    fn format(&self, info: &StringInfo, args: &[(u32, i64)]) -> Option<String> {
        let mut values = Values::new();
        for (id, v) in args {
            values.insert(*id, *v);
        }
        let r = self
            .strings
            .resolve(info, &values, &RenderOptions::default());
        r.is_clean().then(|| r.plain_text())
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
        let mut scenes = BTreeMap::new();
        let mut group_names = BTreeMap::new();
        if let Ok(reg) = gd.registry() {
            let keys = SceneKeys::resolve(gd, reg);
            for r in &chargen.races {
                let Some(did) = r.record.ui_scene else {
                    continue;
                };
                if scenes.contains_key(&did) {
                    continue;
                }
                match load_scene(gd, reg, &keys, did) {
                    Ok(sc) => {
                        tracing::debug!(
                            scene = %did,
                            objects = sc.objects.len(),
                            lights = sc.lights.len(),
                            marker = sc.marker.is_some(),
                            "race scene read"
                        );
                        scenes.insert(did, Arc::new(sc));
                    }
                    Err(e) => tracing::warn!(scene = %did, "race scene unreadable: {e}"),
                }
            }
            if let Some(m) = mapper_of(gd, reg, "AppearanceUI_Group") {
                for (k, info) in m.displays().iter().take(MAX_GROUPS) {
                    group_names.insert(*k, info.clone());
                }
            }
        }
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
            scenes,
            group_names,
        })
    }

    /// The scene of race `species` (`RaceTable_UIScene`), when it loaded.
    #[must_use]
    pub fn scene(&self, species: u32) -> Option<&Arc<StageScene>> {
        self.scenes
            .get(&self.chargen.race(species)?.record.ui_scene?)
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
