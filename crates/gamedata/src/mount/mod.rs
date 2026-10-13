// SPDX-License-Identifier: GPL-3.0-or-later
//! The mount presentation model (decision 0024 P5): a mount effect's name, icon, type,
//! war-steed flag and plugin class; the mounts a playable body is offered (MountDirectory
//! by body size, then its race's war-steeds); the gait multipliers of MountedCombatControl;
//! and the census of every mount effect by what the walker can draw.
//!
//! Spec: `docs/specs/mounts.md` §2.1–§2.8, §4.1, §14.10, §14.11; `game-controls.md` §5.1,
//! §5.3; `lua-plugin-api.md` §15.5 (names and classes of the plugin mount views);
//! `walker.md` §16.2 (resolving a source). The walker's states, gaits and controller are
//! the client's and `tessera-physics`'s; nothing here moves anything.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`MountPresenter`], [`MountInfo`], [`MountClass`], [`MountCategory`] | One mount effect (or a skill toggling one) | `mounts.md` §2, §14.11 |
//! | [`MountPresenter::offered`], [`OfferedMount`], [`OfferSource`], [`WarSteedTree`] | The mounts a body is offered | §14.10 |
//! | [`GaitTable`], [`MountPresenter::gaits`] | Speed and turn-rate multipliers by gait | `game-controls.md` §5.3.2 |
//! | [`MountPresenter::plugin_name`], [`MountPresenter::plugin_class`] | `GetName()` and the mount class of the plugin views | `lua-plugin-api.md` §15.5 |
//! | [`mount_census`], [`MountCensus`] | Census of every mount effect | §14.11 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes. Lists are bounded by [`MAX_OFFERED`].

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::gameplay::common::{PropertyAccess, ValueExt};
use tessera_formats::gameplay::mounts::{
    MountEffect, MountLabels, MountType, WAR_STEED_MASK, toggled_effects,
};
use tessera_formats::{Did, PropertySet};
use tessera_props::RecordSource;
use tessera_text::Values;

use crate::error::Result;
use crate::quest::{PresentationData, RecordRef};
use crate::store::GameDataStore;
use crate::tooltip::TextRef;

/// Most mounts one offered list holds (a Tessera bound; the directory has 676 skills).
pub const MAX_OFFERED: usize = 4_096;
/// WEENIECONTENT label of the mount directory (`game-controls.md` §5.1).
pub const MOUNT_DIRECTORY: &str = "MountDirectory";
/// WEENIECONTENT label of the mounted-combat control (`game-controls.md` §5.3).
pub const MOUNTED_COMBAT_CONTROL: &str = "MountedCombatControl";
/// The gaits whose multipliers the control stores: Walk, Trot, Canter, Gallop
/// (`game-controls.md` §5.3.2, mapper `MountedMovementSpeed`).
pub const GAITS: [u32; 4] = [4, 5, 6, 7];

tessera_formats::property_ids! {
    /// Properties of the mount directory and the mounted-combat control.
    pub(crate) struct ControlPids {
        skill_list = "Mount_SkillList",
        tall = "Mount_SkillToGrantTall",
        short = "Mount_SkillToGrantShort",
        tree_skill_hash = "Mount_Control_Tree_To_Skill_Hash",
        trait_tree = "Mount_Control_Trait_Tree",
        skill_hash = "Mount_Control_Mount_Skill_Hash",
        player_race = "Mount_Control_Player_Race",
        mount_skill = "Mount_Control_Mount_Skill",
        speed_walk = "Mount_Control_Speed_Multiplier_Walk",
        speed_trot = "Mount_Control_Speed_Multiplier_Trot",
        speed_canter = "Mount_Control_Speed_Multiplier_Canter",
        speed_gallop = "Mount_Control_Speed_Multiplier_Gallop",
        turn_walk = "Mount_Control_Turn_Rate_Multiplier_Walk",
        turn_trot = "Mount_Control_Turn_Rate_Multiplier_Trot",
        turn_canter = "Mount_Control_Turn_Rate_Multiplier_Canter",
        turn_gallop = "Mount_Control_Turn_Rate_Multiplier_Gallop",
    }
}

/// The class of the plugin mount object (`lua-plugin-api.md` §15.5): `CombatMount` for a
/// war-steed type, `BasicMount` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountClass {
    /// `BasicMount`.
    Basic,
    /// `CombatMount`.
    Combat,
}

/// What the walker can do with a mount effect (`mounts.md` §14.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MountCategory {
    /// A normal mount (no war-steed bit, not Clydesdale, hobby-horse or startup).
    Normal,
    /// Type labelled `Clydesdale`: war-steed riding set on the normal controller.
    Clydesdale,
    /// A war-steed type only (bits of 0x38): gaits 1–7 on the war-steed controller.
    WarSteed,
    /// A war-steed startup record with an entity (by DID; not offered).
    WarSteedStartup,
    /// A war-steed bit with another bit (by DID; reached by no skill).
    MixedWarSteed,
    /// A hobby-horse record: drawn as a held prop, on foot.
    HobbyHorse,
    /// No entity and no type (the NPC startup record): not drawable for a player.
    NoEntityNoType,
}

impl MountCategory {
    /// Whether the walker can draw it as a mount or a prop (§14.11).
    #[must_use]
    pub fn is_drawable(self) -> bool {
        self != Self::NoEntityNoType
    }
}

/// One mount effect as the walker, the plugin views and the UI present it.
#[derive(Debug, Clone, PartialEq)]
pub struct MountInfo {
    /// The mount effect.
    pub effect: Did,
    /// The skill it was reached through, if any.
    pub skill: Option<Did>,
    /// `Mount_Name_Initial`, rendered.
    pub name: Option<String>,
    /// `Effect_Icon`.
    pub icon: Option<Did>,
    /// `Mount_Type` bits.
    pub mount_type: Option<u32>,
    /// The labels of the type's keys (the install's `MountType` mapper).
    pub type_labels: Vec<String>,
    /// The type has a bit of 0x38 (`mounts.md` §2.8 war-steed rule).
    pub war_steed: bool,
    /// The plugin class.
    pub class: MountClass,
    /// The walker category.
    pub category: MountCategory,
    /// `Mounted_Player_Cosmetic_Slot_Entity`.
    pub entity: Option<Did>,
    /// `Mounted_Player_Cosmetic_Slot_Scale`.
    pub scale: Option<[f32; 3]>,
}

/// The war-steed trees, in the order of their keys (`mounts.md` §14.10: 1 Heavy,
/// 2 Light, 3 Medium here; the order is the keys', the labels are the install's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarSteedTree {
    /// `Mount_Control_Trait_Tree` key.
    pub tree: u32,
    /// Its label (`MountedCombat_Light`, …).
    pub label: Option<String>,
    /// (`Mount_Control_Player_Race`, `Mount_Control_Mount_Skill`) pairs, stored order.
    pub skills: Vec<(u32, Did)>,
}

/// Where an offered mount comes from (§14.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferSource {
    /// The directory group of the body's size.
    Directory,
    /// The race's war-steed of a tree (key).
    WarSteed(u32),
}

/// One mount a body is offered.
#[derive(Debug, Clone, PartialEq)]
pub struct OfferedMount {
    /// Position in the list (the walker's `dir:` index).
    pub index: usize,
    /// The skill.
    pub skill: Did,
    /// Where it comes from.
    pub source: OfferSource,
    /// The mount effect the skill toggles, when it resolves.
    pub mount: Option<MountInfo>,
}

/// The gait multipliers of MountedCombatControl (`game-controls.md` §5.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GaitTable {
    /// Speed multipliers of Walk, Trot, Canter, Gallop (absent: 1.0).
    pub speed: [Option<f32>; 4],
    /// Turn-rate multipliers of Walk, Trot, Canter, Gallop (absent: 1.0).
    pub turn: [Option<f32>; 4],
}

impl GaitTable {
    /// The speed multiplier of `gait` (step 1): gaits 4–7 their value (1.0 when absent),
    /// any other gait 1.0.
    #[must_use]
    pub fn speed_multiplier(&self, gait: u32) -> f32 {
        GAITS
            .iter()
            .position(|g| *g == gait)
            .and_then(|i| self.speed.get(i).copied().flatten())
            .unwrap_or(1.0)
    }

    /// The turn-rate multiplier of `gait` (step 2, without the actor's add-mod): gaits 5,
    /// 6, 7 their pair, any other gait the Walk pair; 1.0 when absent.
    #[must_use]
    pub fn turn_multiplier(&self, gait: u32) -> f32 {
        let i = match gait {
            5 => 1,
            6 => 2,
            7 => 3,
            _ => 0,
        };
        self.turn.get(i).copied().flatten().unwrap_or(1.0)
    }
}

/// The mount model over a [`PresentationData`].
pub struct MountPresenter<'d> {
    data: &'d dyn PresentationData,
    labels: MountLabels,
    pids: ControlPids,
    directory: Option<Arc<PropertySet>>,
    control: Option<Arc<PropertySet>>,
}

impl std::fmt::Debug for MountPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountPresenter")
            .field("directory", &self.directory.is_some())
            .field("control", &self.control.is_some())
            .finish_non_exhaustive()
    }
}

impl<'d> MountPresenter<'d> {
    /// Resolves labels and reads the directory and the control once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        let read = |label: &str| data.record(data.weenie(label)?).ok().flatten();
        Self {
            data,
            labels: MountLabels::resolve(&names),
            pids: ControlPids::resolve(&names),
            directory: read(MOUNT_DIRECTORY),
            control: read(MOUNTED_COMBAT_CONTROL),
        }
    }

    /// The labels in use.
    #[must_use]
    pub fn labels(&self) -> &MountLabels {
        &self.labels
    }

    fn set(&self, did: Did) -> Option<Arc<PropertySet>> {
        self.data.record(did).ok().flatten()
    }

    fn label(&self, pid: tessera_formats::gameplay::common::Pid, key: u32) -> Option<String> {
        self.data.enum_name(pid.id()?, key)
    }

    /// Whether `set` is a mount effect (`mounts.md` §2.1).
    fn is_mount(&self, set: &PropertySet) -> bool {
        MountEffect::recognise(&self.labels, set).is_some()
    }

    /// The mount effect `effect` (`None` when it is not one).
    #[must_use]
    pub fn effect(&self, effect: Did) -> Option<MountInfo> {
        let set = self.set(effect)?;
        let view = MountEffect::recognise(&self.labels, &*set)?;
        let p = &self.labels.effect;
        let ty = view.mount_type();
        let rider = view.rider();
        let type_labels: Vec<String> = ty
            .map(|t| {
                t.keys()
                    .map(|k| {
                        self.label(p.mount_type, k)
                            .unwrap_or_else(|| format!("key {k}"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let war_steed = ty.is_some_and(MountType::is_war_steed);
        let script = |k: u32| self.label(p.animation_script, k);
        let class_label = |k: u32| self.label(p.equivalence_class, k);
        let hobby = rider
            .animation_script
            .and_then(script)
            .is_some_and(|l| l == "Emote_Holding_HobbyHorse");
        let startup = set
            .get_pid(p.equivalence_class)
            .and_then(ValueExt::enum_key)
            .and_then(class_label)
            .is_some_and(|l| l == "Mounted_Monster_Startup")
            && !set.has(p.mod_array)
            && set.boolean(p.remove_on_defeat) == Some(false);
        let entity = rider.entity.filter(|d| !d.is_null());
        let category = categorise(ty, entity.is_some(), hobby, startup, &type_labels);
        Some(MountInfo {
            effect,
            skill: None,
            name: view
                .name()
                .and_then(|n| {
                    self.data
                        .render(TextRef::Info(n), &Values::new())
                        .ok()
                        .flatten()
                })
                .map(|t| t.plain_text()),
            icon: set.did_nonnull(p.icon),
            mount_type: ty.map(|t| t.0),
            type_labels,
            war_steed,
            class: if war_steed {
                MountClass::Combat
            } else {
                MountClass::Basic
            },
            category,
            entity,
            scale: rider.scale,
        })
    }

    /// The effects a skill toggles (`mounts.md` §4.1), in stored order.
    #[must_use]
    pub fn toggled(&self, skill: Did) -> Vec<Did> {
        self.set(skill)
            .map(|s| toggled_effects(&self.labels.stats, &*s))
            .unwrap_or_default()
    }

    /// A mount source (`walker.md` §16.2): a mount effect, or a skill whose
    /// `Skill_Toggle_Effect_List` names one (the first such element).
    #[must_use]
    pub fn resolve(&self, did: Did) -> Option<MountInfo> {
        let set = self.set(did)?;
        if self.is_mount(&set) {
            return self.effect(did);
        }
        self.toggled(did).into_iter().find_map(|e| {
            self.effect(e).map(|mut m| {
                m.skill = Some(did);
                m
            })
        })
    }

    /// The plugin class of a mount ref (`lua-plugin-api.md` §15.5): combat when the ref is
    /// a mount effect whose `Mount_Type` has a bit of 0x38, or a skill whose **first**
    /// `Skill_Toggle_Effect_List` element names such an effect; otherwise (literals
    /// included) basic.
    #[must_use]
    pub fn plugin_class(&self, r: &RecordRef) -> MountClass {
        match self.plugin_effect(r).and_then(|d| self.effect(d)) {
            Some(m) if m.war_steed => MountClass::Combat,
            _ => MountClass::Basic,
        }
    }

    /// `GetName()` of a plugin mount object (`lua-plugin-api.md` §15.5): the rendered
    /// `Mount_Name_Initial` of the effect (of a skill's first toggled effect); the empty
    /// string for another record or when absent; a literal's text.
    #[must_use]
    pub fn plugin_name(&self, r: &RecordRef) -> String {
        if let RecordRef::Literal(s) = r {
            return s.clone();
        }
        self.plugin_effect(r)
            .and_then(|d| self.effect(d))
            .and_then(|m| m.name)
            .unwrap_or_default()
    }

    fn plugin_effect(&self, r: &RecordRef) -> Option<Did> {
        let RecordRef::Record(did) = r else {
            return None;
        };
        let set = self.set(*did)?;
        if self.is_mount(&set) {
            return Some(*did);
        }
        self.toggled(*did).first().copied()
    }

    /// The directory's skills of one body size (`game-controls.md` §5.1): `tall` selects
    /// `Mount_SkillToGrantTall`, else `Mount_SkillToGrantShort`; stored order.
    #[must_use]
    pub fn directory(&self, tall: bool) -> Vec<Did> {
        let Some(dir) = &self.directory else {
            return Vec::new();
        };
        let want = if tall {
            self.pids.tall
        } else {
            self.pids.short
        };
        dir.elements(self.pids.skill_list)
            .iter()
            .filter(|e| want.is(e.pid))
            .filter_map(ValueExt::did_nonnull)
            .take(MAX_OFFERED)
            .collect()
    }

    /// The war-steed trees of MountedCombatControl, ascending tree key.
    #[must_use]
    pub fn war_steed_trees(&self) -> Vec<WarSteedTree> {
        let Some(c) = &self.control else {
            return Vec::new();
        };
        let mut out: BTreeMap<u32, WarSteedTree> = BTreeMap::new();
        for s in c.structs(self.pids.tree_skill_hash) {
            let Some(tree) = s.get_pid(self.pids.trait_tree).and_then(ValueExt::enum_key) else {
                continue;
            };
            let skills = s
                .structs(self.pids.skill_hash)
                .filter_map(|m| {
                    Some((
                        m.get_pid(self.pids.player_race)
                            .and_then(ValueExt::enum_key)?,
                        m.did_nonnull(self.pids.mount_skill)?,
                    ))
                })
                .collect();
            out.entry(tree).or_insert(WarSteedTree {
                tree,
                label: self.label(self.pids.trait_tree, tree),
                skills,
            });
        }
        out.into_values().collect()
    }

    /// The mounts a body is offered (`mounts.md` §14.10 Decision): the skills of its
    /// directory group (`tall` = the species' `RaceTable_IsTallRace`) in stored order, then
    /// the species' war-steed skill of each tree in ascending tree key.
    #[must_use]
    pub fn offered(&self, species: u32, tall: bool) -> Vec<OfferedMount> {
        let mut out = Vec::new();
        for skill in self.directory(tall) {
            out.push((skill, OfferSource::Directory));
        }
        for t in self.war_steed_trees() {
            if let Some((_, skill)) = t.skills.iter().find(|(r, _)| *r == species) {
                out.push((*skill, OfferSource::WarSteed(t.tree)));
            }
        }
        out.into_iter()
            .take(MAX_OFFERED)
            .enumerate()
            .map(|(index, (skill, source))| OfferedMount {
                index,
                skill,
                source,
                mount: self.resolve(skill),
            })
            .collect()
    }

    /// The gait multipliers (`game-controls.md` §5.3.1–§5.3.2).
    #[must_use]
    pub fn gaits(&self) -> GaitTable {
        let Some(c) = &self.control else {
            return GaitTable::default();
        };
        let f = |pid| c.float(pid).filter(|v: &f32| v.is_finite());
        let p = &self.pids;
        GaitTable {
            speed: [
                f(p.speed_walk),
                f(p.speed_trot),
                f(p.speed_canter),
                f(p.speed_gallop),
            ],
            turn: [
                f(p.turn_walk),
                f(p.turn_trot),
                f(p.turn_canter),
                f(p.turn_gallop),
            ],
        }
    }
}

/// The walker category of a mount effect (`mounts.md` §14.11).
#[must_use]
pub fn categorise(
    ty: Option<MountType>,
    has_entity: bool,
    hobby_horse: bool,
    startup: bool,
    type_labels: &[String],
) -> MountCategory {
    let bits = ty.map_or(0, |t| t.0);
    if !has_entity && bits == 0 {
        return MountCategory::NoEntityNoType;
    }
    if hobby_horse {
        return MountCategory::HobbyHorse;
    }
    if bits & WAR_STEED_MASK != 0 {
        if startup {
            return MountCategory::WarSteedStartup;
        }
        if bits & !WAR_STEED_MASK != 0 {
            return MountCategory::MixedWarSteed;
        }
        return MountCategory::WarSteed;
    }
    if type_labels.len() == 1 && type_labels.first().is_some_and(|l| l == "Clydesdale") {
        return MountCategory::Clydesdale;
    }
    MountCategory::Normal
}

/// The census of every mount effect (`mounts.md` §14.11) and of the offered lists.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MountCensus {
    /// Mount effects (effect role `mount`) per category.
    pub categories: BTreeMap<MountCategory, usize>,
    /// Mount effects per stored `Mount_Type` value (`None`: absent).
    pub types: BTreeMap<Option<u32>, usize>,
    /// Mount effects whose record is not recognised as a mount effect (none expected).
    pub not_recognised: Vec<Did>,
    /// Mount effects with a rendered name.
    pub named: usize,
    /// Mount effects with an icon.
    pub with_icon: usize,
    /// War-steed (class `Combat`) effects.
    pub combat: usize,
    /// Directory skills (tall, short) and how many resolve to a mount effect.
    pub directory: ((usize, usize), (usize, usize)),
    /// War-steed tree skills and how many resolve.
    pub war_steed_skills: (usize, usize),
}

impl MountCensus {
    /// Effects of `category`.
    #[must_use]
    pub fn count(&self, category: MountCategory) -> usize {
        self.categories.get(&category).copied().unwrap_or(0)
    }

    /// Effects in all (categorised).
    #[must_use]
    pub fn total(&self) -> usize {
        self.categories.values().sum()
    }
}

/// The census of every mount effect of the install (the effects of role `mount`,
/// [`GameDataStore::mount_types`]) through `presenter`.
///
/// # Errors
///
/// When the record index cannot be built.
pub fn mount_census<S: RecordSource>(
    store: &GameDataStore<S>,
    presenter: &MountPresenter<'_>,
) -> Result<MountCensus> {
    let mut c = MountCensus::default();
    for (&did, &ty) in store.mount_types()?.iter() {
        *c.types.entry(ty).or_default() += 1;
        let Some(m) = presenter.effect(Did(did)) else {
            c.not_recognised.push(Did(did));
            continue;
        };
        *c.categories.entry(m.category).or_default() += 1;
        c.named += usize::from(m.name.is_some());
        c.with_icon += usize::from(m.icon.is_some());
        c.combat += usize::from(m.class == MountClass::Combat);
    }
    let resolves = |skills: &[Did]| {
        skills
            .iter()
            .filter(|&&s| presenter.resolve(s).is_some())
            .count()
    };
    let tall = presenter.directory(true);
    let short = presenter.directory(false);
    c.directory = (
        (tall.len(), resolves(&tall)),
        (short.len(), resolves(&short)),
    );
    let steeds: Vec<Did> = presenter
        .war_steed_trees()
        .iter()
        .flat_map(|t| t.skills.iter().map(|(_, s)| *s))
        .collect();
    c.war_steed_skills = (steeds.len(), resolves(&steeds));
    Ok(c)
}
