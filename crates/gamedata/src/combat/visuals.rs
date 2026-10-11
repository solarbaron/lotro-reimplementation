// SPDX-License-Identifier: GPL-3.0-or-later
//! Script references of combat visuals (`combat-visuals.md` §2, §3, §5, §6, §9, §11): which
//! script keys a skill, an attack hook, an induction, a channel, an effect and a combat state
//! name. The walker runs them; this module only reads them.
//!
//! A script key is a key of the script-key mapper (`combat-visuals.md` §2.1); 0 means none.

use tessera_formats::{Did, PropertySet, StringInfo};

use super::{
    CombatData, as_bits, as_bool, as_did, as_f32, as_info, as_int, as_key, field, get, structs,
};
use crate::tooltip::TooltipData;

/// A script key (0 = none).
pub type ScriptKey = u32;

/// `LifeState` of a living entity (`combat-visuals.md` §1a).
pub const LIFE_ALIVE: u32 = 4;
/// `LifeState` of a dead entity.
pub const LIFE_DEAD: u32 = 5;
/// Engineering limit: open stories per performer (`combat-visuals.md` §11 rule 11).
pub const MAX_STORIES_PER_PERFORMER: usize = 64;
/// Engineering limit: strikes per story.
pub const MAX_STRIKES_PER_STORY: usize = 256;
/// Engineering limit: seconds a story stays open.
pub const STORY_TIMEOUT: f32 = 30.0;
/// Label of the death script key (`combat-visuals.md` §7, §9.5).
pub const DIE_SCRIPT_LABEL: &str = "Death@Agent::*Die";
/// Label of the revive script key.
pub const REVIVE_SCRIPT_LABEL: &str = "Death@Agent::*Revive";

/// One `Skill_AttackHookInfo` as visuals use it (`combat-visuals.md` §2.3, §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HookVisuals {
    /// `Skill_AttackHook_AnimationHookIndex` (the hook number), when stored.
    pub number: Option<i64>,
    /// `Skill_AttackHook_DamageType` key.
    pub damage_type: Option<u32>,
    /// `Skill_AttackHook_BehaviorTargetHit`.
    pub behaviour_target_hit: ScriptKey,
    /// `Skill_AttackHook_fxTargetHit`.
    pub fx_target_hit: ScriptKey,
    /// `Skill_AttackHook_fxAttackerHit`.
    pub fx_attacker_hit: ScriptKey,
    /// `Skill_AttackHook_fxWeaponHit`.
    pub fx_weapon_hit: ScriptKey,
    /// `Skill_AttackHook_BehaviorWeaponHit`.
    pub behaviour_weapon_hit: ScriptKey,
    /// `Skill_AttackHook_Cosmetic`.
    pub cosmetic: bool,
}

/// The visual data of a skill (`combat-visuals.md` §2.2, §3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkillVisuals {
    /// The record.
    pub did: Did,
    /// `Skill_BehaviorSelfList` keys in stored order (zeros kept: a 0 entry plays nothing).
    pub self_list: Vec<ScriptKey>,
    /// `Skill_BehaviorOtherList`.
    pub other_list: Vec<ScriptKey>,
    /// `Skill_MountedBehaviorSelfList`.
    pub mounted_self_list: Vec<ScriptKey>,
    /// `Skill_MountedBehaviorOtherList`.
    pub mounted_other_list: Vec<ScriptKey>,
    /// `Skill_AnimationMode` key (also `Combat_SkillMode`, §9.2 step 3).
    pub animation_mode: Option<u32>,
    /// Attack hooks in stored order.
    pub hooks: Vec<HookVisuals>,
    /// `Skill_InductionAction`.
    pub induction: Option<Did>,
    /// `Skill_Toggle_ChannelingState`.
    pub channelling: Option<Did>,
    /// The skill is a toggle (`Skill_Toggle_Hook_Number` stored).
    pub toggle: bool,
    /// `Skill_ExecutionFailureScript` (not played in P2, rule 10).
    pub execution_failure: ScriptKey,
    /// `Skill_ConjunctionWindupScript` (not played in P2).
    pub conjunction_windup: ScriptKey,
}

impl SkillVisuals {
    /// The behaviour list (§3.1 steps 1–2): mounted lists when `mounted`, the Self list when
    /// the performer is the local player.
    #[must_use]
    pub fn behaviours(&self, mounted: bool, is_self: bool) -> &[ScriptKey] {
        match (mounted, is_self) {
            (true, true) => &self.mounted_self_list,
            (true, false) => &self.mounted_other_list,
            (false, true) => &self.self_list,
            (false, false) => &self.other_list,
        }
    }

    /// The behaviour key (§3.1 steps 3–4): one entry of the list, `pick(n)` choosing an
    /// index in `0..n` when there are several (the shared random source, rule 2); `None` for
    /// an empty list or a 0 key. An out-of-range pick takes the last entry.
    pub fn choose(
        &self,
        mounted: bool,
        is_self: bool,
        pick: impl FnOnce(usize) -> usize,
    ) -> Option<ScriptKey> {
        let list = self.behaviours(mounted, is_self);
        let key = match list.len() {
            0 => return None,
            1 => list[0],
            n => list[pick(n).min(n - 1)],
        };
        (key != 0).then_some(key)
    }

    /// The hook data of hook number `number` (§4.4 step 2): the first hook with that
    /// number; a hook without a stored number counts by position (1-based).
    #[must_use]
    pub fn hook(&self, number: i64) -> Option<&HookVisuals> {
        self.hooks.iter().enumerate().find_map(|(i, h)| {
            let n = h
                .number
                .unwrap_or_else(|| i64::try_from(i + 1).unwrap_or(i64::MAX));
            (n == number).then_some(h)
        })
    }

    /// The largest hook number (§9.3 step 2: strikes past it use it).
    #[must_use]
    pub fn last_hook_number(&self) -> Option<i64> {
        self.hooks
            .iter()
            .enumerate()
            .map(|(i, h)| {
                h.number
                    .unwrap_or_else(|| i64::try_from(i + 1).unwrap_or(i64::MAX))
            })
            .max()
    }
}

/// The scripts of an induction record (`combat-visuals.md` §3.5; `interaction.md` §3.8).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InductionVisuals {
    /// `Induction_AnimationScriptID`.
    pub animation: ScriptKey,
    /// `Induction_MountedAnimationScriptID`.
    pub mounted_animation: ScriptKey,
    /// `Induction_FXID`.
    pub fx: ScriptKey,
    /// `Induction_FailureFXID`.
    pub failure_fx: ScriptKey,
    /// `Induction_TargetAnimationScriptID`.
    pub target_animation: ScriptKey,
    /// `Induction_TargetFXID`.
    pub target_fx: ScriptKey,
    /// `Induction_TargetFailureFXID`.
    pub target_failure_fx: ScriptKey,
    /// `Induction_Interruptable`.
    pub interruptible: bool,
    /// `Induction_Duration`.
    pub duration: Option<f32>,
    /// `Induction_TitleString`.
    pub title: Option<StringInfo>,
}

/// The scripts of one moment of a channel (`combat-visuals.md` §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MomentScripts {
    /// The performer's script.
    pub caster: ScriptKey,
    /// The performer's FX script.
    pub caster_fx: ScriptKey,
    /// The target's script.
    pub target: ScriptKey,
    /// The target's FX script.
    pub target_fx: ScriptKey,
}

/// The scripts of a channelling record (`combat-visuals.md` §3.5; `interaction.md` §3.9).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChannelVisuals {
    /// Channel started.
    pub start: MomentScripts,
    /// Channel running.
    pub running: MomentScripts,
    /// Ended, completed.
    pub success: MomentScripts,
    /// Ended, interrupted.
    pub failure: MomentScripts,
    /// Ended, cancelled.
    pub abandoned: MomentScripts,
    /// `Channeling_Interruptable`.
    pub interruptible: bool,
    /// `Channeling_Duration`.
    pub duration: Option<f32>,
}

/// The visuals of an effect (`combat-visuals.md` §5.1, §9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EffectVisuals {
    /// `Effect_FX`: run while the effect is applied.
    pub fx: ScriptKey,
    /// `Effect_VitalOverTime_IntervalFX`: run per pulse (§9.3 step 5).
    pub interval_fx: ScriptKey,
    /// `Effect_ThreatOverTime_IntervalFX`.
    pub threat_interval_fx: ScriptKey,
    /// `Effect_InduceCombatState_StateToInduce` (combat-state bits), for the state script.
    pub induced_state: Option<u32>,
}

/// One combat state of the combat-state control (`game-controls.md` §5.8,
/// `combat-visuals.md` §6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CombatStateVisual {
    /// `CombatState_StateData_State` (one bit).
    pub state: u32,
    /// `CombatState_StateData_ScriptID`: the state's visual.
    pub script: ScriptKey,
    /// `CombatState_StateData_ExaminationName`.
    pub name: Option<StringInfo>,
    /// `CombatState_StateData_FlagEffect`.
    pub flag_effect: Option<Did>,
    /// `CombatState_StateData_DefaultDuration`.
    pub default_duration: Option<f32>,
    /// From the mounted list.
    pub mounted: bool,
}

/// The attack result of a strike (`combat-visuals.md` §9.3 step 1), from the feed's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrikeResult {
    /// `Normal` → `Hit`.
    Normal,
    /// `Critical` → `CriticalHit`.
    Critical,
    /// `Miss`.
    Miss,
    /// `Evade`.
    Evade,
    /// `Parry`.
    Parry,
    /// `Block`.
    Block,
    /// `Resist`.
    Resist,
    /// `Immune`.
    Immune,
}

impl StrikeResult {
    /// The `CombatResultType` label (resolve the key by label at run time).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Hit",
            Self::Critical => "CriticalHit",
            Self::Miss => "Miss",
            Self::Evade => "Evade",
            Self::Parry => "Parry",
            Self::Block => "Block",
            Self::Resist => "Resist",
            Self::Immune => "Immune",
        }
    }

    /// The key §9.3 gives for the observed install (the fallback when no mapper is at hand).
    #[must_use]
    pub fn observed_key(self) -> u32 {
        match self {
            Self::Normal => 9,
            Self::Critical => 10,
            Self::Miss => 12,
            Self::Evade => 1,
            Self::Parry => 5,
            Self::Block => 3,
            Self::Resist => 7,
            Self::Immune => 8,
        }
    }
}

const ONE_HANDED: [&str; 6] = ["1HSword", "1HAxe", "1HClub", "1HMace", "1HHammer", "Dagger"];
const SHIELDS: [&str; 3] = ["Shield", "HeavyShield", "MediumShield"];

/// The stance (`Entity_Mode` label) of a figure (`combat-visuals.md` §11 rule 6) from its
/// main- and off-hand equipment-category labels (mapper of `Item_EquipmentCategory`). An
/// off-hand weapon is an off-hand item of a one-handed category (Tessera reading of "an
/// off-hand weapon").
#[must_use]
pub fn stance_for(main_hand: Option<&str>, off_hand: Option<&str>, player: bool) -> &'static str {
    if !player {
        return "combat_monster";
    }
    let shield = off_hand.is_some_and(|o| SHIELDS.contains(&o));
    let Some(main) = main_hand else {
        return "combat_ua";
    };
    if ONE_HANDED.contains(&main) {
        return if shield {
            "combat_1hshield"
        } else if off_hand.is_some_and(|o| ONE_HANDED.contains(&o)) {
            "combat_dualwield"
        } else {
            "combat_1h"
        };
    }
    match main {
        "2HSword" | "2HClub" | "2HMace" | "2HHammer" | "Halberd" => "combat_2h",
        "2HAxe" => "combat_2haxe",
        "Spear" if shield => "combat_spearshield",
        "Spear" => "combat_spear",
        "2HStaff" => "combat_staff",
        "Bow" => "combat_bow",
        "Crossbow" => "combat_xbow",
        "Javelin" | "Thrown" => "combat_throw",
        "BattleGauntlets" => "combat_gauntlets",
        _ => "combat_1h",
    }
}

/// PIDs of the channelling FX properties, found by name (`combat-visuals.md` §2.1 names the
/// pattern `Channeling_…Script_FXID` / `Channeling_Target…Script_FXID`): caster and target,
/// per moment (start, running, success, failure, abandoned).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ChannelFxPids {
    caster: [Option<u32>; 5],
    target: [Option<u32>; 5],
}

impl ChannelFxPids {
    fn find(data: &dyn TooltipData) -> Self {
        let mut out = Self::default();
        for (pid, name) in data.properties_with_prefix("Channeling_") {
            if !name.ends_with("_FXID") {
                continue;
            }
            let moment = if name.contains("Start") {
                0
            } else if name.contains("Animation") {
                1
            } else if name.contains("Success") {
                2
            } else if name.contains("Failure") {
                3
            } else if name.contains("Abandon") {
                4
            } else {
                continue;
            };
            let slot = if name.contains("Target") {
                &mut out.target[moment]
            } else {
                &mut out.caster[moment]
            };
            slot.get_or_insert(pid);
        }
        out
    }
}

fn key(set: &PropertySet, pid: Option<u32>) -> ScriptKey {
    as_key(get(set, pid)).unwrap_or(0)
}

impl<D: TooltipData> CombatData<D> {
    fn behaviour_list(&self, set: &PropertySet, list: Option<u32>) -> Vec<ScriptKey> {
        structs(get(set, list))
            .map(|s| as_key(field(s, self.p.skill_behaviour)).unwrap_or(0))
            .collect()
    }

    /// The visual data of a skill record (`combat-visuals.md` §2.2, §2.3); `None` for a
    /// record without `Skill_Name`.
    pub fn skill_visuals(&self, skill: Did) -> Option<std::sync::Arc<SkillVisuals>> {
        let p = self.p;
        self.cached(&self.skills, skill, |s| {
            get(s, p.skill_name)?;
            let hooks = structs(get(s, p.hook_list))
                .map(|h| HookVisuals {
                    number: as_int(field(h, p.hook_index)),
                    damage_type: as_key(field(h, p.hook_damage_type)),
                    behaviour_target_hit: as_key(field(h, p.hook_behaviour_target)).unwrap_or(0),
                    fx_target_hit: as_key(field(h, p.hook_fx_target)).unwrap_or(0),
                    fx_attacker_hit: as_key(field(h, p.hook_fx_attacker)).unwrap_or(0),
                    fx_weapon_hit: as_key(field(h, p.hook_fx_weapon)).unwrap_or(0),
                    behaviour_weapon_hit: as_key(field(h, p.hook_behaviour_weapon)).unwrap_or(0),
                    cosmetic: as_bool(field(h, p.hook_cosmetic)) == Some(true),
                })
                .collect();
            Some(SkillVisuals {
                did: skill,
                self_list: self.behaviour_list(s, p.skill_self_list),
                other_list: self.behaviour_list(s, p.skill_other_list),
                mounted_self_list: self.behaviour_list(s, p.skill_mounted_self_list),
                mounted_other_list: self.behaviour_list(s, p.skill_mounted_other_list),
                animation_mode: as_key(get(s, p.skill_animation_mode)),
                hooks,
                induction: as_did(get(s, p.skill_induction)),
                channelling: as_did(get(s, p.skill_channelling)),
                toggle: get(s, p.skill_toggle_hook).is_some(),
                execution_failure: key(s, p.skill_execution_failure),
                conjunction_windup: key(s, p.skill_conjunction_windup),
            })
        })
    }

    /// The scripts of an induction record (§3.5); `None` without `Induction_Duration`.
    pub fn induction_visuals(&self, induction: Did) -> Option<std::sync::Arc<InductionVisuals>> {
        let p = self.p;
        self.cached(&self.inductions, induction, |s| {
            let duration = as_f32(get(s, p.induction_duration));
            duration?;
            Some(InductionVisuals {
                animation: key(s, p.induction_animation),
                mounted_animation: key(s, p.induction_mounted_animation),
                fx: key(s, p.induction_fx),
                failure_fx: key(s, p.induction_failure_fx),
                target_animation: key(s, p.induction_target_animation),
                target_fx: key(s, p.induction_target_fx),
                target_failure_fx: key(s, p.induction_target_failure_fx),
                interruptible: as_bool(get(s, p.induction_interruptible)) == Some(true),
                duration,
                title: as_info(get(s, p.induction_title)),
            })
        })
    }

    /// The scripts of a channelling record (§3.5); `None` without `Channeling_Duration`.
    pub fn channel_visuals(&self, channel: Did) -> Option<std::sync::Arc<ChannelVisuals>> {
        let p = self.p;
        let fx = *self
            .channel_fx
            .get_or_init(|| ChannelFxPids::find(&self.data));
        self.cached(&self.channels, channel, |s| {
            let duration = as_f32(get(s, p.channelling_duration));
            duration?;
            let moment = |i: usize, caster: Option<u32>, target: Option<u32>| MomentScripts {
                caster: key(s, caster),
                caster_fx: key(s, fx.caster[i]),
                target: key(s, target),
                target_fx: key(s, fx.target[i]),
            };
            Some(ChannelVisuals {
                start: moment(0, p.channelling_start, p.channelling_target_start),
                running: moment(1, p.channelling_animation, p.channelling_target_animation),
                success: moment(2, p.channelling_success, p.channelling_target_success),
                failure: moment(3, p.channelling_failure, p.channelling_target_failure),
                abandoned: moment(4, p.channelling_abandoned, p.channelling_target_abandoned),
                interruptible: as_bool(get(s, p.channelling_interruptible)) == Some(true),
                duration,
            })
        })
    }

    /// The visuals of an effect record (§5.1, the `Effect_FX` mapping of §9.4); `None` for a
    /// record without `Effect_Name`.
    pub fn effect_visuals(&self, effect: Did) -> Option<std::sync::Arc<EffectVisuals>> {
        let p = self.p;
        self.cached(&self.effects, effect, |s| {
            get(s, p.effect_name)?;
            let induced = as_bits(get(s, p.effect_induced_state));
            Some(EffectVisuals {
                fx: key(s, p.effect_fx),
                interval_fx: key(s, p.effect_interval_fx),
                threat_interval_fx: key(s, p.effect_threat_interval_fx),
                induced_state: (!induced.is_empty()).then(|| {
                    induced
                        .iter()
                        .filter(|&&b| b < 32)
                        .fold(0u32, |acc, b| acc | (1 << b))
                }),
            })
        })
    }

    /// The combat states of the combat-state control (`game-controls.md` §5.8), read once.
    pub fn combat_states(&self) -> &[CombatStateVisual] {
        self.states.get_or_init(|| {
            let p = self.p;
            let Some(control) = self
                .data
                .weenie("CombatStateControl")
                .and_then(|d| self.set(d))
            else {
                return Vec::new();
            };
            let mut out = Vec::new();
            for (list, mounted) in [(p.state_list, false), (p.state_mounted_list, true)] {
                for s in structs(get(&control, list)) {
                    let bits = as_bits(field(s, p.state_bits));
                    let state = bits
                        .iter()
                        .filter(|&&b| b < 32)
                        .fold(0u32, |acc, b| acc | (1 << b));
                    out.push(CombatStateVisual {
                        state,
                        script: as_key(field(s, p.state_script)).unwrap_or(0),
                        name: as_info(field(s, p.state_name)),
                        flag_effect: as_did(field(s, p.state_flag_effect)),
                        default_duration: as_f32(field(s, p.state_default_duration)),
                        mounted,
                    });
                }
            }
            out
        })
    }

    /// The script of the combat state an effect induces (`combat-visuals.md` §9.4: effects of
    /// class 769 also run the induced state's `CombatState_StateData_ScriptID`); `None`
    /// without one. The unmounted list is searched first.
    pub fn induced_state_script(&self, effect: Did) -> Option<ScriptKey> {
        let bits = self.effect_visuals(effect)?.induced_state?;
        self.combat_states()
            .iter()
            .find(|s| s.state & bits != 0 && s.script != 0)
            .map(|s| s.script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviour_choice_3_1() {
        let v = SkillVisuals {
            self_list: vec![10, 11, 12],
            other_list: vec![20],
            mounted_self_list: vec![0],
            ..SkillVisuals::default()
        };
        assert_eq!(v.choose(false, true, |n| n - 1), Some(12));
        assert_eq!(v.choose(false, true, |_| 99), Some(12));
        assert_eq!(v.choose(false, false, |_| 0), Some(20));
        assert_eq!(v.choose(true, true, |_| 0), None);
        assert_eq!(v.choose(true, false, |_| 0), None);
    }

    #[test]
    fn hooks_by_number() {
        let v = SkillVisuals {
            hooks: vec![
                HookVisuals {
                    number: Some(1),
                    fx_target_hit: 5,
                    ..HookVisuals::default()
                },
                HookVisuals {
                    number: None,
                    fx_target_hit: 6,
                    ..HookVisuals::default()
                },
            ],
            ..SkillVisuals::default()
        };
        assert_eq!(v.hook(1).map(|h| h.fx_target_hit), Some(5));
        assert_eq!(v.hook(2).map(|h| h.fx_target_hit), Some(6));
        assert!(v.hook(3).is_none());
        assert_eq!(v.last_hook_number(), Some(2));
    }

    #[test]
    fn stances_rule_6() {
        assert_eq!(stance_for(None, None, true), "combat_ua");
        assert_eq!(
            stance_for(Some("1HSword"), Some("Shield"), true),
            "combat_1hshield"
        );
        assert_eq!(
            stance_for(Some("Dagger"), Some("1HAxe"), true),
            "combat_dualwield"
        );
        assert_eq!(stance_for(Some("1HMace"), None, true), "combat_1h");
        assert_eq!(stance_for(Some("Halberd"), None, true), "combat_2h");
        assert_eq!(
            stance_for(Some("Spear"), Some("HeavyShield"), true),
            "combat_spearshield"
        );
        assert_eq!(stance_for(Some("Crossbow"), None, true), "combat_xbow");
        assert_eq!(stance_for(Some("Lute"), None, true), "combat_1h");
        assert_eq!(stance_for(Some("Bow"), None, false), "combat_monster");
        assert_eq!(StrikeResult::Critical.observed_key(), 10);
        assert_eq!(StrikeResult::Normal.label(), "Hit");
    }
}
