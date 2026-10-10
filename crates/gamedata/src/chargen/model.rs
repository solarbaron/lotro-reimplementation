// SPDX-License-Identifier: GPL-3.0-or-later
//! The lists of the race-and-class and appearance-and-name screens
//! (`character-screens.md` §4, §5, §6), the creation check (`player-character.md` §5.4)
//! and the invariants CSI2–CSI5 (§10), over the `tessera-wstate` character views.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::{Did, StringInfo};
use tessera_wstate::character::{
    ChoiceVerdict, ClassPriority, CreationChoice, GenderEntry, NationalityRecord, RaceRecord,
    RaceTrait,
};

use super::appearance::{AppearanceControl, AvatarControls, ControlKind};
use super::names::{NameCheckFailure, NameData};

/// One gender entry of a race as the screens show it (`player-character.md` §5.3).
///
/// [`GenderChoice::from_entry`] builds it from the race table view's validated
/// [`GenderEntry`]; the loader then adds the display fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenderChoice {
    /// `RaceTable_GenderType`: the sex key (also the body's sex at creation).
    pub sex: u32,
    /// `RaceTable_Gender_PlayerAvatar`.
    pub player_avatar: Option<Did>,
    /// `RaceTable_RaceSelect_Entity`: the figure of the race-selection viewport.
    pub race_select_entity: Option<Did>,
    /// `RaceTable_RaceSelect_Icon`: the race button's icon.
    pub icon: Option<Did>,
    /// `RaceTable_Gender_Race_Name`: the race button's name.
    pub race_name: Option<StringInfo>,
    /// `RaceTable_Gender_Name`.
    pub name: Option<StringInfo>,
    /// `RaceTable_Gender_Desc`.
    pub description: Option<StringInfo>,
    /// `RaceTable_Chargen_Zoom_Camera`.
    pub chargen_zoom_camera: Option<u32>,
    /// `RaceTable_Character_Select_Zoom_Camera`.
    pub select_zoom_camera: Option<u32>,
}

impl GenderChoice {
    /// The gender choice of a race table view's gender entry: its sex, player avatar and
    /// race-selection figure; the display fields are empty.
    #[must_use]
    pub fn from_entry(entry: &GenderEntry) -> Self {
        Self {
            sex: entry.gender,
            player_avatar: entry.player_avatar,
            race_select_entity: entry.race_select_entity,
            ..Self::default()
        }
    }
}

/// One race table with its gender entries (`player-character.md` §5.2).
///
/// **One gender list.** The model (the creation check, the offered choices, the
/// defaults, CSI2, CSI4 and CSI5) reads only [`RaceChoice::genders`]; `record.genders` is
/// the view it was built from and is not consulted. [`RaceChoice::from_record`] (used by
/// [`super::load_chargen`]) builds `genders` one to one from `record.genders`, so
/// [`ChargenData::check`] agrees with `tessera_wstate::character::check_choice` over the
/// records. Data built directly must keep the two lists equal in sex for the same
/// agreement.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RaceChoice {
    /// The race table record.
    pub did: Did,
    /// The race table view.
    pub record: RaceRecord,
    /// Its gender entries, stored order (the model's gender list).
    pub genders: Vec<GenderChoice>,
}

impl RaceChoice {
    /// A race choice whose gender list is [`GenderChoice::from_entry`] of each entry of
    /// `record.genders`, in stored order.
    #[must_use]
    pub fn from_record(did: Did, record: RaceRecord) -> Self {
        Self {
            did,
            genders: record
                .genders
                .iter()
                .map(GenderChoice::from_entry)
                .collect(),
            record,
        }
    }

    /// The species key.
    #[must_use]
    pub fn species(&self) -> u32 {
        self.record.race
    }

    /// The first gender entry of `sex`.
    #[must_use]
    pub fn gender(&self, sex: u32) -> Option<&GenderChoice> {
        self.genders.iter().find(|g| g.sex == sex)
    }
}

/// A class of the class list (`character-screens.md` §4.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassChoice {
    /// The class key.
    pub class: u32,
    /// Its class table (`combat-data.md` §9.1).
    pub table: Option<Did>,
    /// `AdvTable_ClassInfo` → `AdvTable_ClassIcon`.
    pub icon: Option<Did>,
    /// `AdvTable_ClassInfo` → `AdvTable_ClassName`.
    pub name: Option<StringInfo>,
    /// `AdvTable_ClassInfo` → `AdvTable_ClassDesc`.
    pub description: Option<StringInfo>,
    /// `AdvTable_CharGenBackground`.
    pub background: Option<Did>,
}

/// A broken install invariant of `character-screens.md` §10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChargenProblem {
    /// CSI2: a race has no gender entry.
    RaceWithoutGender {
        /// The species.
        species: u32,
    },
    /// CSI2: a race has no class.
    RaceWithoutClass {
        /// The species.
        species: u32,
    },
    /// CSI3: a class key of a race's list has no class table.
    ClassWithoutTable {
        /// The species.
        species: u32,
        /// The class.
        class: u32,
    },
    /// CSI3: a class table lacks the `AdvTable_ClassInfo` icon or name.
    ClassWithoutInfo {
        /// The class.
        class: u32,
    },
    /// CSI4: a gender entry has no player avatar.
    GenderWithoutAvatar {
        /// The species.
        species: u32,
        /// The sex.
        sex: u32,
    },
    /// CSI4: a player avatar's controls could not be read.
    ControlsUnreadable {
        /// The avatar.
        avatar: Did,
        /// Why.
        reason: String,
    },
    /// CSI4: a player avatar has no control available at creation.
    NoCreationControls {
        /// The avatar.
        avatar: Did,
    },
    /// A race, nationality or class table, or a directory or control the screens read, is
    /// missing or its view rejected it; the loader skipped it.
    RecordUnreadable {
        /// The record (null for a control named by label).
        did: Did,
        /// Why.
        reason: String,
    },
}

/// Why a creation request is refused before it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreationRefusal {
    /// `player-character.md` §5.4 rejects the choice.
    Choice(ChoiceVerdict),
    /// The name fails the name check.
    Name(NameCheckFailure),
}

impl std::fmt::Display for CreationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Choice(v) => write!(f, "the choice is not valid ({v:?})"),
            Self::Name(n) => write!(f, "the name is not valid: {n}"),
        }
    }
}

/// What the screen sends as `CreateCharacter` (`character-screens.md` §7.1): the name, the
/// body (`Body::Character { species, sex }`), the class and the nationality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationRequest {
    /// The name, as typed.
    pub name: String,
    /// The body's species (the race table's `RaceTable_Race`).
    pub species: u32,
    /// The body's sex (the gender entry's `RaceTable_GenderType`).
    pub sex: u32,
    /// The class key.
    pub class: u32,
    /// The nationality key.
    pub nationality: u32,
}

/// Everything the race-and-class and appearance-and-name screens list, read from the
/// install ([`super::load_chargen`]) or built directly (tests).
#[derive(Debug, Clone, Default)]
pub struct ChargenData {
    /// The races in the race directory's stored order (§4.1).
    pub races: Vec<RaceChoice>,
    /// Class display data by class key (from the class tables).
    pub classes: BTreeMap<u32, ClassChoice>,
    /// The class priority control (`player-character.md` §7.3); empty when absent.
    pub priority: ClassPriority,
    /// Nationality tables by key (the first record of a key).
    pub nationalities: BTreeMap<u32, (Did, NationalityRecord)>,
    /// Appearance controls by player avatar, or why they could not be read.
    pub controls: BTreeMap<Did, std::result::Result<AvatarControls, String>>,
    /// Labels of `AppearanceUI_Group` keys (the mapper's names).
    pub group_labels: BTreeMap<u32, String>,
    /// Records the loader skipped (missing, unreadable or rejected by their view), with
    /// the reason; reported by [`ChargenData::problems`].
    pub unreadable: Vec<(Did, String)>,
}

impl ChargenData {
    /// The first race of `species`.
    #[must_use]
    pub fn race(&self, species: u32) -> Option<&RaceChoice> {
        self.races.iter().find(|r| r.species() == species)
    }

    /// The first race of `species` that has a gender entry of `sex` (§5.4 rule 1).
    #[must_use]
    pub fn race_with_gender(&self, species: u32, sex: u32) -> Option<&RaceChoice> {
        self.races
            .iter()
            .find(|r| r.species() == species && r.gender(sex).is_some())
    }

    /// The class keys of `species` in display order (§4.2, Decision D3 of
    /// `player-character.md` §7.3): ascending `CharGen_ClassPriority_PriorityValue`;
    /// classes without a priority entry follow in stored order (Tessera rule); a repeated
    /// key is listed once.
    #[must_use]
    pub fn class_order(&self, species: u32) -> Vec<u32> {
        match self.race(species) {
            Some(race) => class_order_of(race, &self.priority_map()),
            None => Vec::new(),
        }
    }

    /// The priority of each class key (the first entry of a key wins).
    fn priority_map(&self) -> BTreeMap<u32, i32> {
        self.priority
            .entries
            .iter()
            .rev()
            .map(|&(c, p)| (c, p))
            .collect()
    }

    /// The class list of `species` with display data, in [`ChargenData::class_order`].
    #[must_use]
    pub fn classes_for(&self, species: u32) -> Vec<ClassChoice> {
        self.class_order(species)
            .into_iter()
            .map(|c| {
                self.classes.get(&c).cloned().unwrap_or(ClassChoice {
                    class: c,
                    ..ClassChoice::default()
                })
            })
            .collect()
    }

    /// The nationality keys of `species` in stored order (§5.1) with their records.
    #[must_use]
    pub fn nationalities_for(&self, species: u32) -> Vec<(u32, Option<&NationalityRecord>)> {
        self.race(species)
            .map(|r| {
                r.record
                    .nationalities
                    .iter()
                    .map(|k| (*k, self.nationalities.get(k).map(|(_, n)| n)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The naming guideline of `nationality` for `sex` (§5.1;
    /// `player-character.md` §6.2, §8.3).
    #[must_use]
    pub fn naming_guideline(&self, nationality: u32, sex: u32) -> Option<&StringInfo> {
        self.nationalities
            .get(&nationality)?
            .1
            .naming_guidelines
            .iter()
            .find(|g| g.sex == Some(sex))
            .and_then(|g| g.text.as_ref())
    }

    /// The racial traits of `species` (§4.1 Tessera rule): all, by ascending
    /// `AdvTable_Trait_Level` (absent last), then stored order.
    #[must_use]
    pub fn racial_traits(&self, species: u32) -> Vec<RaceTrait> {
        let mut t: Vec<RaceTrait> = self
            .race(species)
            .map(|r| r.record.characteristics.clone())
            .unwrap_or_default();
        t.sort_by_key(|x| x.level.map_or((1, 0), |l| (0, l)));
        t
    }

    /// The appearance controls of (`species`, `sex`): those of the gender entry's player
    /// avatar (§5.2).
    #[must_use]
    pub fn controls_for(
        &self,
        species: u32,
        sex: u32,
    ) -> Option<&std::result::Result<AvatarControls, String>> {
        let avatar = self
            .race_with_gender(species, sex)?
            .gender(sex)?
            .player_avatar?;
        self.controls.get(&avatar)
    }

    /// `player-character.md` §5.4 for a (species, sex, nationality, class) choice (the
    /// rules of `tessera_wstate::character::check_choice`, over the gender entries of
    /// [`RaceChoice::genders`], the model's one gender list; equal to `check_choice` over
    /// the records for data built by [`RaceChoice::from_record`]).
    #[must_use]
    pub fn check(&self, choice: &CreationChoice) -> ChoiceVerdict {
        let Some(race) = self.race_with_gender(choice.species, choice.sex) else {
            return ChoiceVerdict::NoSuchRaceAndSex;
        };
        if !race.record.nationalities.contains(&choice.nationality) {
            return ChoiceVerdict::NationalityNotAllowed;
        }
        if !race.record.classes.contains(&choice.class) {
            return ChoiceVerdict::ClassNotAllowed;
        }
        ChoiceVerdict::Valid
    }

    /// Every (species, sex, class) the race-and-class screen offers (§9 step 1), in
    /// screen order: races, their genders, their classes in display order. Its length is
    /// at most races × genders × classes, each bounded by the loader
    /// ([`super::MAX_RACES`], [`super::MAX_GENDERS_PER_RACE`],
    /// [`super::MAX_RACE_LIST_ENTRIES`]); data built directly is the caller's bound.
    #[must_use]
    pub fn offered(&self) -> Vec<(u32, u32, u32)> {
        let priority = self.priority_map();
        let mut out = Vec::new();
        for r in &self.races {
            // The first race of a species decides its class order (as `class_order`).
            let classes = match self.race(r.species()) {
                Some(first) => class_order_of(first, &priority),
                None => Vec::new(),
            };
            for g in &r.genders {
                for c in &classes {
                    out.push((r.species(), g.sex, *c));
                }
            }
        }
        out
    }

    /// CSI5: whether (`species`, `sex`) is the body of a gender entry, so a created
    /// character's `Body::Character { species, sex }` is valid.
    #[must_use]
    pub fn is_creation_body(&self, species: u32, sex: u32) -> bool {
        self.race_with_gender(species, sex).is_some()
    }

    /// Validates a creation before it is sent (§7.1): the choice by §5.4 and the name by
    /// `names` (when given), and returns the command's content.
    ///
    /// # Errors
    ///
    /// The first refusal.
    pub fn validate_creation(
        &self,
        choice: &CreationChoice,
        name: &str,
        names: Option<&NameData>,
    ) -> std::result::Result<CreationRequest, CreationRefusal> {
        match self.check(choice) {
            ChoiceVerdict::Valid => {}
            v => return Err(CreationRefusal::Choice(v)),
        }
        match names {
            Some(n) => n.check(name).map_err(CreationRefusal::Name)?,
            None if name.is_empty() => {
                return Err(CreationRefusal::Name(NameCheckFailure::Empty));
            }
            None => {}
        }
        Ok(CreationRequest {
            name: name.to_owned(),
            species: choice.species,
            sex: choice.sex,
            class: choice.class,
            nationality: choice.nationality,
        })
    }

    /// The palette image of colour control `control` while nationality `nationality` is
    /// selected (§5.1, Tessera rule): the `AppearanceUI_DyeMapImage` of the nationality's
    /// first `AppearanceUI_DyeMapControlOverrides` entry whose
    /// `AppearanceUI_ControlPropertyName` equals the control's and that names an image;
    /// else the control's own palette. `None` for a control that is not a colour control
    /// or has no palette at all.
    #[must_use]
    pub fn palette_for(&self, control: &AppearanceControl, nationality: u32) -> Option<Did> {
        let ControlKind::Colour { property, palette } = control.kind else {
            return None;
        };
        property
            .and_then(|p| {
                self.nationalities
                    .get(&nationality)?
                    .1
                    .dye_overrides
                    .iter()
                    .filter(|o| o.control_property == Some(p))
                    .find_map(|o| o.image)
            })
            .or(palette)
    }

    /// The broken invariants CSI2–CSI4 of the install, and the records the loader skipped
    /// (empty when sound). CSI4 is checked only for avatars whose controls were loaded.
    #[must_use]
    pub fn problems(&self) -> Vec<ChargenProblem> {
        let mut out: Vec<ChargenProblem> = self
            .unreadable
            .iter()
            .map(|(did, reason)| ChargenProblem::RecordUnreadable {
                did: *did,
                reason: reason.clone(),
            })
            .collect();
        let mut info_checked = BTreeSet::new();
        for r in &self.races {
            let species = r.species();
            if r.genders.is_empty() {
                out.push(ChargenProblem::RaceWithoutGender { species });
            }
            if r.record.classes.is_empty() {
                out.push(ChargenProblem::RaceWithoutClass { species });
            }
            for &class in &r.record.classes {
                match self.classes.get(&class) {
                    Some(c) if c.table.is_some() => {
                        if info_checked.insert(class) && (c.icon.is_none() || c.name.is_none()) {
                            out.push(ChargenProblem::ClassWithoutInfo { class });
                        }
                    }
                    _ => out.push(ChargenProblem::ClassWithoutTable { species, class }),
                }
            }
            for g in &r.genders {
                let Some(avatar) = g.player_avatar else {
                    out.push(ChargenProblem::GenderWithoutAvatar {
                        species,
                        sex: g.sex,
                    });
                    continue;
                };
                match self.controls.get(&avatar) {
                    Some(Ok(c)) if c.creation.is_empty() => {
                        out.push(ChargenProblem::NoCreationControls { avatar });
                    }
                    Some(Err(reason)) => out.push(ChargenProblem::ControlsUnreadable {
                        avatar,
                        reason: reason.clone(),
                    }),
                    _ => {}
                }
            }
        }
        out.dedup();
        out
    }

    // -------------------------------------------------------------------------------------
    // Screen defaults (Tessera rules of `character-screens.md` §4.2 and §6)
    // -------------------------------------------------------------------------------------

    /// The race and gender the race-and-class screen opens with (§6 step 2): those of the
    /// selected character when the install offers them, else the first race and its first
    /// gender.
    #[must_use]
    pub fn default_race_and_gender(&self, selected: Option<(u32, u32)>) -> Option<(u32, u32)> {
        if let Some((s, x)) = selected
            && self.race_with_gender(s, x).is_some()
        {
            return Some((s, x));
        }
        let r = self.races.first()?;
        Some((r.species(), r.genders.first()?.sex))
    }

    /// The class after a race change (§4.2): `current` when the new race allows it, else
    /// the first class of its list.
    #[must_use]
    pub fn class_after_race_change(&self, current: Option<u32>, species: u32) -> Option<u32> {
        let order = self.class_order(species);
        match current {
            Some(c) if order.contains(&c) => Some(c),
            _ => order.first().copied(),
        }
    }

    /// The gender after a race change: `current` when the new race has it, else its first
    /// gender (Tessera rule, as for classes; named in the implementer OPEN of
    /// `character-screens.md` §5.2).
    #[must_use]
    pub fn gender_after_race_change(&self, current: Option<u32>, species: u32) -> Option<u32> {
        let r = self.race(species)?;
        match current {
            Some(x) if r.gender(x).is_some() => Some(x),
            _ => r.genders.first().map(|g| g.sex),
        }
    }

    /// The nationality the appearance screen opens with (§6 step 3): the first of the
    /// race's list.
    #[must_use]
    pub fn default_nationality(&self, species: u32) -> Option<u32> {
        self.race(species)?.record.nationalities.first().copied()
    }
}

/// The class keys of `race` in display order under `priority` (see
/// [`ChargenData::class_order`]).
fn class_order_of(race: &RaceChoice, priority: &BTreeMap<u32, i32>) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    let mut keys: Vec<(usize, u32)> = race
        .record
        .classes
        .iter()
        .copied()
        .filter(|c| seen.insert(*c))
        .enumerate()
        .collect();
    keys.sort_by_key(|&(i, c)| (priority.get(&c).map_or((1, 0), |p| (0, *p)), i));
    keys.into_iter().map(|(_, c)| c).collect()
}

/// Whether the appearance choices are kept on going back from the appearance screen (§6
/// step 3): only while race and gender stay the same.
#[must_use]
pub fn keeps_appearance(before: (u32, u32), after: (u32, u32)) -> bool {
    before == after
}
