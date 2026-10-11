// SPDX-License-Identifier: GPL-3.0-or-later
//! Combat presentation data (decision 0024 P2): typed skill and effect records, the data the
//! combat panels show, and the script references of combat visuals. Neutral types: the client
//! fills its widgets and the walker runs its scripts from them; nothing here decides a
//! gameplay outcome (decision 0024 §2.4).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`SkillRecord`], [`EffectRecord`] | Typed skill and effect records over the record index | `combat-data.md` §6, §7; `skill-tooltips.md` §3.1, §11 |
//! | [`CombatData`] | The panel and visual data of one install, read on demand and cached | below |
//! | [`Consideration`], [`TargetKind`], [`name_colour_state`], [`health_meter_state`] | Consideration colours by level difference, name colours, health-meter colours | `game-controls.md` §7.5; `combat-ui.md` §5.3, §5.4 (TR14, TR15) |
//! | [`VitalDisplayOverrides`], [`PLAIN_FRAME_STATE`] | Vital display overrides | `game-controls.md` §7.1; `combat-ui.md` TR9 |
//! | [`TargetFrame`], [`LevelDisplay`], [`decoration_state`] | Target-frame decoration and level display | `combat-ui.md` §5.2 (TR13) |
//! | [`EffectIcon`], [`EffectRow`], [`BORDER_STATES`] | Effect icons, rows and borders | `combat-ui.md` §8.1 (TR29, TR30) |
//! | [`CastBar`] | Cast-bar labels and style | `combat-ui.md` §7.2 (TR25); `interaction.md` §3.8.4 |
//! | [`DefeatRecord`] | Defeat records | `interaction.md` §8 |
//! | [`FloatyTextSettings`], [`Avoidance`] | Floating-text settings and avoidance words | `combat-ui.md` §6.2, §6.6 (TR19) |
//! | [`SkillVisuals`], [`HookVisuals`], [`InductionVisuals`], [`ChannelVisuals`], [`EffectVisuals`], [`CombatStateVisual`], [`stance_for`] | Script references of combat visuals | `combat-visuals.md` §2, §3, §5, §6, §11 |
//!
//! # Hostile input
//!
//! Everything is read from decoded property sets; lists are bounded ([`MAX_LIST`] elements,
//! [`MAX_CACHED_RECORDS`] cached records per kind, cleared when full) and nothing panics on a
//! missing or mistyped property.

mod panels;
mod records;
mod visuals;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};

use crate::tooltip::TooltipData;

pub use panels::{
    Avoidance, BORDER_STATES, CON_EVEN_STATE, CONSIDER_MONSTER, CORPSE_STATE, CastBar,
    Consideration, DefeatRecord, EffectIcon, EffectRow, FloatyTextSettings, LevelDisplay,
    NPC_STATE, OBJECT_STATE, PLAIN_FRAME_STATE, PLAYER_STATE, TargetFrame, TargetKind,
    VitalDisplayOverride, VitalDisplayOverrides, decoration_state, health_meter_state,
    name_colour_state,
};
pub use records::{EffectRecord, SkillRecord};
pub use visuals::{
    ChannelVisuals, CombatStateVisual, DIE_SCRIPT_LABEL, EffectVisuals, HookVisuals,
    InductionVisuals, LIFE_ALIVE, LIFE_DEAD, MAX_STORIES_PER_PERFORMER, MAX_STRIKES_PER_STORY,
    MomentScripts, REVIVE_SCRIPT_LABEL, STORY_TIMEOUT, ScriptKey, SkillVisuals, StrikeResult,
    stance_for,
};

/// Most elements read from one list property (hostile-input bound; the data's longest combat
/// list here is 120 elements).
pub const MAX_LIST: usize = 1024;

/// Most distinct consideration tables [`CombatData::consideration`] reads, and most
/// consideration lists it keeps (hostile-input bound; the data has six tables of class 1694
/// in six lists, `game-controls.md` §7.5).
pub const MAX_CONSIDERATION_TABLES: usize = 64;

/// Most records kept per cache of [`CombatData`] before the cache is cleared.
pub const MAX_CACHED_RECORDS: usize = 16_384;

macro_rules! data_pids {
    ($(#[$m:meta])* $name:ident { $($field:ident = $label:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name {
            $(pub(crate) $field: Option<u32>,)*
        }

        impl $name {
            pub(crate) fn resolve(data: &dyn TooltipData) -> Self {
                Self { $($field: data.pid($label),)* }
            }
        }
    };
}

data_pids! {
    /// Properties the combat data reads, by dictionary name.
    Pids {
        skill_name = "Skill_Name",
        skill_icon = "Skill_Icon",
        skill_large_icon = "Skill_LargeIcon",
        skill_small_icon = "Skill_SmallIcon",
        skill_induction = "Skill_InductionAction",
        skill_channelling = "Skill_Toggle_ChannelingState",
        skill_toggle_hook = "Skill_Toggle_Hook_Number",
        skill_self_list = "Skill_BehaviorSelfList",
        skill_other_list = "Skill_BehaviorOtherList",
        skill_mounted_self_list = "Skill_MountedBehaviorSelfList",
        skill_mounted_other_list = "Skill_MountedBehaviorOtherList",
        skill_behaviour = "Skill_Behavior",
        skill_animation_mode = "Skill_AnimationMode",
        skill_execution_failure = "Skill_ExecutionFailureScript",
        skill_conjunction_windup = "Skill_ConjunctionWindupScript",
        hook_list = "Skill_AttackHookList",
        hook_index = "Skill_AttackHook_AnimationHookIndex",
        hook_damage_type = "Skill_AttackHook_DamageType",
        hook_fx_target = "Skill_AttackHook_fxTargetHit",
        hook_fx_attacker = "Skill_AttackHook_fxAttackerHit",
        hook_fx_weapon = "Skill_AttackHook_fxWeaponHit",
        hook_behaviour_target = "Skill_AttackHook_BehaviorTargetHit",
        hook_behaviour_weapon = "Skill_AttackHook_BehaviorWeaponHit",
        hook_cosmetic = "Skill_AttackHook_Cosmetic",
        effect_name = "Effect_Name",
        effect_icon = "Effect_Icon",
        effect_harmful = "Effect_Harmful",
        effect_resist_category = "Effect_Resist_Category",
        effect_combat_only = "Effect_Duration_CombatOnly",
        effect_fx = "Effect_FX",
        effect_interval_fx = "Effect_VitalOverTime_IntervalFX",
        effect_threat_interval_fx = "Effect_ThreatOverTime_IntervalFX",
        effect_induced_state = "Effect_InduceCombatState_StateToInduce",
        hover_layout = "UI_Examination_Tooltip_0Click_Layout",
        hover_element = "UI_Examination_Tooltip_0Click_Element",
        induction_duration = "Induction_Duration",
        induction_interruptible = "Induction_Interruptable",
        induction_title = "Induction_TitleString",
        induction_animation = "Induction_AnimationScriptID",
        induction_mounted_animation = "Induction_MountedAnimationScriptID",
        induction_fx = "Induction_FXID",
        induction_failure_fx = "Induction_FailureFXID",
        induction_target_animation = "Induction_TargetAnimationScriptID",
        induction_target_fx = "Induction_TargetFXID",
        induction_target_failure_fx = "Induction_TargetFailureFXID",
        channelling_duration = "Channeling_Duration",
        channelling_interruptible = "Channeling_Interruptable",
        channelling_title = "Channeling_TitleString",
        channelling_start = "Channeling_StartScriptID",
        channelling_animation = "Channeling_AnimationScriptID",
        channelling_success = "Channeling_Success_Script",
        channelling_failure = "Channeling_Failure_Script",
        channelling_abandoned = "Channeling_Abandoned_Script",
        channelling_target_start = "Channeling_Target_Start_Script",
        channelling_target_animation = "Channeling_Target_Animation_Script",
        channelling_target_success = "Channeling_Target_Success_Script",
        channelling_target_failure = "Channeling_Target_Failure_Script",
        channelling_target_abandoned = "Channeling_Target_Abandoned_Script",
        state_list = "CombatState_Control_StateDataList",
        state_mounted_list = "CombatState_Control_Mounted_StateDataList",
        state_bits = "CombatState_StateData_State",
        state_script = "CombatState_StateData_ScriptID",
        state_name = "CombatState_StateData_ExaminationName",
        state_flag_effect = "CombatState_StateData_FlagEffect",
        state_default_duration = "CombatState_StateData_DefaultDuration",
        rank_to_state = "ConsiderationControl_RankToUIState",
        consider_rank = "ConsiderationControl_ConsiderRank",
        consider_state = "ConsiderationControl_UIStateID",
        combat_tables = "ConsiderationControl_CombatTableData",
        consider_type = "ConsiderationControl_ConsiderType",
        combat_table_list = "ConsiderationControl_CombatTableList",
        table_ranks = "ConsiderationTable_RankPerDifferentialArray",
        vdo_array = "VitalDisplayOverride_Array",
        vdo_key = "VitalDisplayOverride_RequiredEnum",
        vdo_state = "VitalDisplayOverride_UIState",
        vdo_name = "VitalDisplayOverride_CSM_Name",
        vdo_freep = "VitalDisplayOverride_IsForFreepCharacters",
        vdo_monster = "VitalDisplayOverride_IsForMonsterCharacters",
        vdo_pvp = "VitalDisplayOverride_IsForPvPCharacters",
        decoration = "MonsterLevel_ExaminationModStatType",
        hide_level = "NPC_HideLevelInDisplay",
        hide_level_and_vitals = "NPC_HideLevelAndVitalText",
        rally_point = "Death_RallyPoint",
        defeat_effect_progression = "Death_DefeatEffectProgression",
        defeat_conditions = "WorldEvent_AllConditionList",
        floaty_los_fade = "UI_FloatyText_LOSFailureFadeTime",
        floaty_fade = "UI_FloatyText_FadeTime",
        floaty_near = "UI_FloatyText_NearRange",
        floaty_far = "UI_FloatyText_FarRange",
        floaty_near_scale = "UI_FloatyText_NearRangeScale",
        floaty_far_scale = "UI_FloatyText_FarRangeScale",
        floaty_dialog_near_scale = "UI_FloatyText_DialogNearRangeScale",
        floaty_dialog_far_scale = "UI_FloatyText_DialogFarRangeScale",
        floaty_columns = "UI_FloatyText_DamageColumns",
        floaty_column_separation = "UI_FloatyText_DamageColumnSeparation",
        floaty_critical_speed = "UI_FloatyText_CriticalDamage_SpeedMod",
        floaty_critical_scale = "UI_FloatyText_CriticalDamage_InitialScale",
        floaty_critical_shrink = "UI_FloatyText_CriticalDamage_ShrinkTime",
        floaty_critical_horizontal = "UI_FloatyText_CriticalDamage_HorizontalOffset",
        floaty_critical_vertical = "UI_FloatyText_CriticalDamage_VerticalOffset",
        floaty_check_distance = "UI_FloatyText_CheckDistance",
    }
}

type Cache<T> = Mutex<HashMap<Did, Option<Arc<T>>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The combat panel and visual data of one install (`combat-ui.md`, `combat-visuals.md`),
/// read through a [`TooltipData`] on demand. Singletons (controls, settings) are read once;
/// per-record data is cached ([`MAX_CACHED_RECORDS`] per kind). `Sync` when the data is.
pub struct CombatData<D: TooltipData> {
    data: D,
    p: Pids,
    channel_fx: OnceLock<visuals::ChannelFxPids>,
    consideration: OnceLock<Option<Consideration>>,
    overrides: OnceLock<VitalDisplayOverrides>,
    states: OnceLock<Vec<CombatStateVisual>>,
    floaty: OnceLock<[FloatyTextSettings; 2]>,
    skills: Cache<SkillVisuals>,
    effects: Cache<EffectVisuals>,
    inductions: Cache<InductionVisuals>,
    channels: Cache<ChannelVisuals>,
}

impl<D: TooltipData> std::fmt::Debug for CombatData<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatData")
            .field("skills", &lock(&self.skills).len())
            .field("effects", &lock(&self.effects).len())
            .finish_non_exhaustive()
    }
}

impl<D: TooltipData> CombatData<D> {
    /// Combat data over `data`; property IDs are looked up by name once here.
    pub fn new(data: D) -> Self {
        let p = Pids::resolve(&data);
        Self {
            data,
            p,
            channel_fx: OnceLock::new(),
            consideration: OnceLock::new(),
            overrides: OnceLock::new(),
            states: OnceLock::new(),
            floaty: OnceLock::new(),
            skills: Mutex::new(HashMap::new()),
            effects: Mutex::new(HashMap::new()),
            inductions: Mutex::new(HashMap::new()),
            channels: Mutex::new(HashMap::new()),
        }
    }

    /// The data read.
    pub fn data(&self) -> &D {
        &self.data
    }

    fn set(&self, did: Did) -> Option<Arc<PropertySet>> {
        self.data.property_set(did).ok().flatten()
    }

    fn cached<T>(
        &self,
        cache: &Cache<T>,
        did: Did,
        read: impl FnOnce(&PropertySet) -> Option<T>,
    ) -> Option<Arc<T>> {
        if let Some(v) = lock(cache).get(&did) {
            return v.clone();
        }
        let v = self.set(did).and_then(|s| read(&s)).map(Arc::new);
        let mut c = lock(cache);
        if c.len() >= MAX_CACHED_RECORDS {
            c.clear();
        }
        c.insert(did, v.clone());
        v
    }
}

// ---------------------------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------------------------

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&PropertyValue> {
    set.get(pid?)
}

fn field(
    s: &tessera_formats::property::PropertyStruct,
    pid: Option<u32>,
) -> Option<&PropertyValue> {
    s.get(pid?)
}

fn as_bool(v: Option<&PropertyValue>) -> Option<bool> {
    match v? {
        PropertyValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn as_f32(v: Option<&PropertyValue>) -> Option<f32> {
    match v? {
        PropertyValue::Float(x) if x.is_finite() => Some(*x),
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn as_int(v: Option<&PropertyValue>) -> Option<i64> {
    match v? {
        PropertyValue::Int(i) => Some(i64::from(*i)),
        PropertyValue::Int64(i) => Some(*i),
        _ => None,
    }
}

fn as_key(v: Option<&PropertyValue>) -> Option<u32> {
    match v? {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn as_did(v: Option<&PropertyValue>) -> Option<Did> {
    match v? {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

fn as_bits(v: Option<&PropertyValue>) -> Vec<u32> {
    match v {
        Some(PropertyValue::Bitfield32(b)) => (0..32).filter(|i| b >> i & 1 == 1).collect(),
        Some(PropertyValue::Bitfield64(b)) => (0..64).filter(|i| b >> i & 1 == 1).collect(),
        Some(PropertyValue::BitfieldAny(b)) => b.set_bits().take(MAX_LIST).collect(),
        _ => Vec::new(),
    }
}

fn as_info(v: Option<&PropertyValue>) -> Option<StringInfo> {
    match v? {
        PropertyValue::StringInfo(s) => Some((**s).clone()),
        _ => None,
    }
}

/// The elements of an array value, bounded.
fn elements(v: Option<&PropertyValue>) -> &[tessera_formats::property::PropertyEntry] {
    match v {
        Some(PropertyValue::Array(a)) => {
            let n = a.elements.len().min(MAX_LIST);
            &a.elements[..n]
        }
        _ => &[],
    }
}

fn structs(
    v: Option<&PropertyValue>,
) -> impl Iterator<Item = &tessera_formats::property::PropertyStruct> {
    elements(v).iter().filter_map(|e| e.value.as_struct())
}
