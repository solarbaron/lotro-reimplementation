// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed skill and effect records over the record index (`combat-data.md` §6, §7;
//! `skill-tooltips.md` §3.1, §11).

use std::sync::Arc;

use tessera_formats::gameplay::combat::{CombatFamily, CombatObject, Effect, Skill};
use tessera_formats::gameplay::common::{GameObjectRecord, is_object_did};
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_props::RecordSource;

use crate::error::Result;
use crate::query::Family;
use crate::store::GameDataStore;
use crate::tooltip::{SkillIconSize, TemplateRef};

fn pair(set: &PropertySet, layout: Option<u32>, element: Option<u32>) -> Option<TemplateRef> {
    let layout = match set.get(layout?)? {
        PropertyValue::DidRef(d) if !d.is_null() => *d,
        _ => return None,
    };
    let element = match set.get(element?)? {
        PropertyValue::Enum(k) if *k != 0 => *k,
        _ => return None,
    };
    Some(TemplateRef { layout, element })
}

/// A skill record: the typed skill of `combat-data.md` §7, its property set and the
/// presentation facts of `skill-tooltips.md` (tooltip pair, icons).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillRecord {
    /// The record.
    pub did: Did,
    /// Root class.
    pub class: u32,
    /// The typed skill.
    pub skill: Skill,
    /// Its property set.
    pub properties: Arc<PropertySet>,
    /// The hover tooltip pair (`UI_Examination_Tooltip_0Click_*`, §3.1); `None` for monster
    /// skills.
    pub tooltip: Option<TemplateRef>,
}

impl SkillRecord {
    /// `Skill_Name`.
    #[must_use]
    pub fn name(&self) -> &StringInfo {
        &self.skill.name
    }

    /// The icon of a skill display of `size` (§11.2).
    #[must_use]
    pub fn icon(&self, size: SkillIconSize) -> Option<Did> {
        let [icon, large, small] = self.skill.icons;
        size.select(icon, large, small)
    }

    /// The examination icon, `Skill_SmallIcon` (§11.1).
    #[must_use]
    pub fn examination_icon(&self) -> Option<Did> {
        self.skill.icons[2]
    }

    /// `Skill_IsMonsterSkill`.
    #[must_use]
    pub fn is_monster_skill(&self) -> bool {
        self.skill.is_monster_skill == Some(true)
    }

    /// The skill is a toggle (`Skill_Toggle_Hook_Number` stored, §7.11).
    #[must_use]
    pub fn is_toggle(&self) -> bool {
        self.skill.timing.toggle_hook_number.is_some()
    }
}

/// An effect record: the typed effect of `combat-data.md` §6, its property set and its
/// presentation facts.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectRecord {
    /// The record.
    pub did: Did,
    /// Root class.
    pub class: u32,
    /// The typed effect.
    pub effect: Effect,
    /// Its property set.
    pub properties: Arc<PropertySet>,
    /// The hover tooltip pair (§3.1); `None` for effects without a tooltip.
    pub tooltip: Option<TemplateRef>,
}

impl EffectRecord {
    /// `Effect_Name`.
    #[must_use]
    pub fn name(&self) -> &StringInfo {
        &self.effect.name
    }

    /// `Effect_Icon` (§11.1, §11.3).
    #[must_use]
    pub fn icon(&self) -> Option<Did> {
        self.effect.icon
    }

    /// `Effect_Harmful`.
    #[must_use]
    pub fn harmful(&self) -> bool {
        self.effect.harmful == Some(true)
    }

    /// `Effect_Duration_CombatOnly`.
    #[must_use]
    pub fn combat_only(&self) -> bool {
        self.effect.duration.combat_only == Some(true)
    }
}

impl<S: RecordSource> GameDataStore<S> {
    /// [`GameDataStore::object`] with DIDs outside the object range as `Ok(None)`.
    fn object_or_none(&self, did: Did) -> Result<Option<GameObjectRecord>> {
        if !is_object_did(did) {
            return Ok(None);
        }
        self.object(did)
    }

    fn tooltip_pids(&self) -> (Option<u32>, Option<u32>) {
        let reg = self.object_store().game_data().registry().ok();
        (
            reg.and_then(|r| r.pid("UI_Examination_Tooltip_0Click_Layout")),
            reg.and_then(|r| r.pid("UI_Examination_Tooltip_0Click_Element")),
        )
    }

    /// A skill record (`combat-data.md` §7) by root class, without building the index;
    /// `Ok(None)` for other records, absent records and DIDs outside the object range. The
    /// record is decoded once.
    ///
    /// # Errors
    ///
    /// [`crate::GameDataError::View`] when the record does not decode.
    pub fn skill_record(&self, did: Did) -> Result<Option<SkillRecord>> {
        let Some(record) = self.object_or_none(did)? else {
            return Ok(None);
        };
        let class = record.root.root_class;
        let Some(CombatObject::Skill(skill)) =
            self.combat_of_record(CombatFamily::Skill, did, &record)?
        else {
            return Ok(None);
        };
        let skill = *skill;
        let (l, e) = self.tooltip_pids();
        let tooltip = pair(&record.properties, l, e);
        Ok(Some(SkillRecord {
            did,
            class,
            skill,
            properties: Arc::new(record.properties),
            tooltip,
        }))
    }

    /// An effect record (`combat-data.md` §6) by root class, without building the index;
    /// `Ok(None)` for other records, absent records and DIDs outside the object range. The
    /// record is decoded once.
    ///
    /// # Errors
    ///
    /// [`crate::GameDataError::View`] when the record does not decode.
    pub fn effect_record(&self, did: Did) -> Result<Option<EffectRecord>> {
        let Some(record) = self.object_or_none(did)? else {
            return Ok(None);
        };
        let class = record.root.root_class;
        let Some(CombatObject::Effect(effect)) =
            self.combat_of_record(CombatFamily::Effect, did, &record)?
        else {
            return Ok(None);
        };
        let effect = *effect;
        let (l, e) = self.tooltip_pids();
        let tooltip = pair(&record.properties, l, e);
        Ok(Some(EffectRecord {
            did,
            class,
            effect,
            properties: Arc::new(record.properties),
            tooltip,
        }))
    }

    /// Every skill record's DID in ascending order (builds the index).
    ///
    /// # Errors
    ///
    /// The index errors of [`GameDataStore::index`].
    pub fn skills(&self) -> Result<Vec<Did>> {
        self.family(Family::Skill)
    }

    /// Every effect record's DID in ascending order (builds the index).
    ///
    /// # Errors
    ///
    /// The index errors of [`GameDataStore::index`].
    pub fn effects(&self) -> Result<Vec<Did>> {
        self.family(Family::Effect)
    }
}
