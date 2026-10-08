// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the player-character records without class indices (`player-character.md`
//! §2.2, §2.3): the controls through their WEENIECONTENT labels, the other records by
//! following the references of §16 from them.

use std::collections::BTreeMap;

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet, RecordSource};

use super::{
    Allegiance, AllegianceControl, BankRepository, ClassCreationInfo, Fields, LevelTableDirectory,
    NationalityDirectory, PipControl, RaceDirectory, RaceRecord, XpGlobals,
};
use crate::error::Result;
use crate::store::ObjectStore;

/// The WEENIECONTENT labels of §2.2, in the spec's order. They are DID-map labels of the
/// install (interoperability identifiers); the DIDs are read at run time.
pub const CONTROL_LABELS: [&str; 34] = [
    "RaceTableDirectory",
    "NationalityTableDirectory",
    "LevelTableDirectory",
    "StatDirectory",
    "ChargenClassPriorityControl",
    "AdvancedCharacterStartControl",
    "NameRulePropertyControl",
    "RandomNameGenerationDirectory",
    "XPGlobals",
    "XPBonusControl",
    "LevelScalingControl",
    "CappedGrowthPropertyControl",
    "ProgressionGatingControl",
    "PipControl",
    "CharacterSlotControl",
    "AccountTypeRestrictionsControl",
    "Account",
    "Character",
    "SharedHousing",
    "StandardHousing",
    "BankRepositoryControl",
    "WardrobeControl",
    "OutfitControl",
    "ColorPackControl",
    "AppearanceColorControl",
    "GuildControl",
    "PedigreeControl",
    "AllegianceControl",
    "TraitConfigurationControl",
    "TraitGroupToTraitNature",
    "ClassToTraitControl",
    "TraitControl",
    "RepositoryManager",
    "CharacterSheetManager",
];

/// The kind of a record reached from a control (§2.3, §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    /// A control named by a WEENIECONTENT label (§2.2).
    Control,
    /// A race table (§5.2).
    RaceTable,
    /// A race avatar, player template (§4.4).
    PlayerAvatar,
    /// A race avatar's administrator twin (§4.2).
    AdminAvatar,
    /// A nationality table (§6.2).
    NationalityTable,
    /// A class table (`combat-data.md` §9.1; creation fields §7).
    ClassTable,
    /// An experience table (§9.2).
    ExperienceTable,
    /// An advanced-start kit (§7.2).
    AdvancedStartKit,
    /// A stat object (§7.6).
    StatObject,
    /// A pip (§10.2).
    Pip,
    /// A trait configuration (§13.2).
    TraitConfiguration,
    /// An allegiance (§12.4).
    Allegiance,
    /// A bank upgrade schedule (§11.3).
    BankSchedule,
    /// A kinship theme (§12.2).
    KinshipTheme,
    /// A group modifier table (§9.3).
    GroupModifierTable,
}

/// The records of the group that can be reached from the controls (§2.3).
///
/// Avatar templates are reached only from the race tables here; the monster-play tables
/// and the PvP control that name the other pairs (§4.1) belong to other specs. Kinship
/// themes are reached from the guild control only (charter items are items).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharacterRecords {
    /// Label → control DID, for the labels of [`CONTROL_LABELS`] the install has.
    pub controls: BTreeMap<&'static str, Did>,
    /// Every record found, with the role it was first reached in.
    pub records: BTreeMap<Did, Role>,
}

impl CharacterRecords {
    /// Resolves the labels and follows the references (property halves only).
    ///
    /// # Errors
    ///
    /// A failed read or DID-map lookup, an undecodable property half, or a view error
    /// ([`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`]).
    pub fn collect<S: RecordSource>(store: &ObjectStore<S>) -> Result<Self> {
        let reg = store.game_data().registry()?.clone();
        let mut out = Self::default();
        for label in CONTROL_LABELS {
            match store.weenie(label)? {
                Some(did) => {
                    out.controls.insert(label, did);
                    out.add(did, Role::Control);
                }
                None => tracing::debug!(label, "WEENIECONTENT label not in this install"),
            }
        }
        let props = |did: Did| -> Result<Option<PropertySet>> { store.properties(did) };
        let control = |label: &str| out.controls.get(label).copied();

        let mut found: Vec<(Did, Role)> = Vec::new();
        if let Some(set) = control("RaceTableDirectory")
            .map(props)
            .transpose()?
            .flatten()
        {
            for t in RaceDirectory::from_properties(&set, &reg)?.tables {
                found.push((t, Role::RaceTable));
                if let Some(rs) = props(t)? {
                    for g in RaceRecord::from_properties(&rs, &reg)?.genders {
                        found.extend(g.player_avatar.map(|d| (d, Role::PlayerAvatar)));
                        found.extend(g.admin_avatar.map(|d| (d, Role::AdminAvatar)));
                    }
                }
            }
        }
        if let Some(set) = control("NationalityTableDirectory")
            .map(props)
            .transpose()?
            .flatten()
        {
            found.extend(
                NationalityDirectory::from_properties(&set, &reg)?
                    .tables
                    .into_iter()
                    .map(|d| (d, Role::NationalityTable)),
            );
        }
        if let Some(set) = control("LevelTableDirectory")
            .map(props)
            .transpose()?
            .flatten()
        {
            for t in LevelTableDirectory::from_properties(&set, &reg)?.class_tables {
                found.push((t, Role::ClassTable));
                if let Some(cs) = props(t)? {
                    let info = ClassCreationInfo::from_properties(&cs, &reg)?;
                    found.extend(info.experience_table.map(|d| (d, Role::ExperienceTable)));
                    found.extend(
                        info.advanced_starts
                            .iter()
                            .filter_map(|a| a.kit)
                            .map(|d| (d, Role::AdvancedStartKit)),
                    );
                }
            }
        }
        list_refs(
            &mut found,
            &reg,
            control("StatDirectory").map(props).transpose()?.flatten(),
            "stat directory",
            "Stat_StatList",
            Role::StatObject,
        )?;
        if let Some(set) = control("PipControl").map(props).transpose()?.flatten() {
            found.extend(
                PipControl::from_properties(&set, &reg)?
                    .pips
                    .into_iter()
                    .map(|d| (d, Role::Pip)),
            );
        }
        list_refs(
            &mut found,
            &reg,
            control("TraitConfigurationControl")
                .map(props)
                .transpose()?
                .flatten(),
            "trait-configuration control",
            "TraitConfiguration_TraitConfigurationList",
            Role::TraitConfiguration,
        )?;
        if let Some(set) = control("AllegianceControl")
            .map(props)
            .transpose()?
            .flatten()
        {
            let c = AllegianceControl::from_properties(&set, &reg)?;
            let mut allegiances: Vec<Did> = c.types.clone();
            allegiances.extend(c.groups.iter().flat_map(|g| g.allegiances.iter().copied()));
            found.extend(c.progressions.iter().map(|&d| (d, Role::ExperienceTable)));
            for a in allegiances {
                found.push((a, Role::Allegiance));
                if let Some(s) = props(a)? {
                    found.extend(
                        Allegiance::from_properties(&s, &reg)?
                            .progression
                            .map(|d| (d, Role::ExperienceTable)),
                    );
                }
            }
        }
        for label in ["Account", "Character", "SharedHousing", "StandardHousing"] {
            if let Some(set) = control(label).map(props).transpose()?.flatten() {
                found.extend(
                    BankRepository::from_properties(&set, &reg)?
                        .schedules()
                        .into_iter()
                        .map(|d| (d, Role::BankSchedule)),
                );
            }
        }
        if let Some(set) = control("GuildControl").map(props).transpose()?.flatten() {
            let f = Fields::new(&set, &reg, "guild control");
            found.extend(
                f.did("Guild_DefaultTheme")?
                    .map(|d| (d, Role::KinshipTheme)),
            );
        }
        if let Some(set) = control("XPGlobals").map(props).transpose()?.flatten() {
            found.extend(
                XpGlobals::from_properties(&set, &reg)?
                    .group_modifier_table
                    .map(|d| (d, Role::GroupModifierTable)),
            );
        }
        for (did, role) in found {
            out.add(did, role);
        }
        Ok(out)
    }

    fn add(&mut self, did: Did, role: Role) {
        self.records.entry(did).or_insert(role);
    }

    /// The records of `role`, ascending DID.
    pub fn with_role(&self, role: Role) -> impl Iterator<Item = Did> + '_ {
        self.records
            .iter()
            .filter(move |&(_, &r)| r == role)
            .map(|(&d, _)| d)
    }

    /// The control named `label`, if the install has it.
    #[must_use]
    pub fn control(&self, label: &str) -> Option<Did> {
        self.controls.get(label).copied()
    }
}

fn list_refs(
    found: &mut Vec<(Did, Role)>,
    reg: &PropertyRegistry,
    set: Option<PropertySet>,
    record: &'static str,
    list: &'static str,
    role: Role,
) -> Result<()> {
    if let Some(set) = set {
        let f = Fields::new(&set, reg, record);
        found.extend(f.did_list(list)?.into_iter().map(|d| (d, role)));
    }
    Ok(())
}
