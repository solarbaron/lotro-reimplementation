// SPDX-License-Identifier: GPL-3.0-or-later
//! Combat data: skills, effects, traits, progressions, modifier arrays and their tables.
//!
//! Spec: `docs/specs/combat-data.md`.
//!
//! Nothing here decodes bytes: every combat object is an ordinary object record (0x70) with
//! its property set (0x79), read by [`GameObjectRecord::decode`]. This module adds the
//! meaning, as typed views over the property half (`combat-data.md` §3.1):
//!
//! | Item | Spec |
//! |---|---|
//! | [`CombatClasses`], [`CombatFamily`], [`RelatedObject`], [`EffectRole`] | §2.1, §2.2, §6.1 |
//! | [`CombatPids`] and its groups | §3.1 (labels resolved at run time) |
//! | [`Progression`], [`ProgressionSource`], [`evaluate_progression`] | §4 |
//! | [`ModEntry`], [`ModOp`], [`LevelRange`], [`mod_entries`], [`mod_arrays`], [`ranged_groups`], [`sandbox_combine`] | §5 |
//! | [`Effect`], [`EffectDuration`], [`VitalAmount`], [`EffectReference`], [`EffectControl`] | §6 |
//! | [`Skill`], [`AttackHook`], [`VitalCost`], [`DetectionVolume`], [`Induction`], [`Channelling`] | §7 |
//! | [`Trait`], [`TraitTree`], [`TraitCell`], [`SparseDidProgression`], [`TraitControl`] | §4.6, §8 |
//! | [`ClassTable`], [`AdvancementLine`], [`MonsterPlayTable`] | §9 |
//! | [`CombatControl`], [`DpsTable`], [`ValueTable`], [`ThresholdPolicy`], [`BattleStatistic`] | §10 |
//! | [`CombatObject`], [`read_combat_object`] | all families |
//!
//! Class indices are build-specific: [`CombatClasses::observed`] holds the indices of the
//! install the spec was written against, in one table a new build can replace. Properties
//! are always found by dictionary label ([`CombatPids::resolve`]).
//!
//! Tessera decisions that are not facts about the original (`combat-data.md` D1–D4) are
//! marked where they apply: the clamp of out-of-range progression indices (D1,
//! [`Progression::value_at`]), the sandbox order of modifier entries (D2,
//! [`sandbox_combine`]), the stored value at a linear key (D3) and the progression-chain
//! rules (D4, [`evaluate_progression`]).

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use super::common::{ClassParents, ClassTest, GameObjectRecord, Pid, PropertyAccess, ValueExt};
use crate::did::Did;
use crate::enum_mapper::EnumMapper;
use crate::error::{FormatError, Result};
use crate::property::{PropertyArray, PropertyEntry, PropertyNames, PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

fn malformed(what: &'static str, reason: impl Into<Cow<'static, str>>) -> FormatError {
    FormatError::Malformed {
        what,
        offset: 0,
        reason: reason.into(),
    }
}

/// Value meaning "not given" in spellcraft, duration, level-range and other combat fields
/// (`combat-data.md` §1, §5.1, §6.3).
pub const NOT_GIVEN: f32 = -1.0;

fn given_f32(v: Option<f32>) -> Option<f32> {
    // Bit-exact −1 only: a value that merely rounds near it is data.
    v.filter(|x| x.to_bits() != NOT_GIVEN.to_bits())
}

fn given_i32(v: Option<i32>) -> Option<i32> {
    v.filter(|&x| x != -1)
}

fn nonzero_pid(v: Option<u32>) -> Option<u32> {
    v.filter(|&p| p != 0)
}

/// PIDs (type 10) stored in array `pid` of `holder`, in stored order, zeros (padding,
/// `combat-data.md` §3.3) removed.
fn pid_list<A: PropertyAccess + ?Sized>(holder: &A, pid: Pid) -> Vec<u32> {
    holder
        .elements(pid)
        .iter()
        .filter_map(ValueExt::property_ref)
        .filter(|&p| p != 0)
        .collect()
}

/// Every non-null DID anywhere inside `value` (the value itself, struct fields, array
/// elements), depth first in stored order. Depth is bounded by the decoder's nesting limit.
fn nested_dids(value: &PropertyValue, out: &mut Vec<Did>) {
    match value {
        PropertyValue::DidRef(d) if !d.is_null() => out.push(*d),
        PropertyValue::Struct(_) | PropertyValue::Array(_) => {
            for c in value.children() {
                nested_dids(&c.value, out);
            }
        }
        _ => {}
    }
}

fn dids_inside<A: PropertyAccess + ?Sized>(holder: &A, pid: Pid) -> Vec<Did> {
    let mut out = Vec::new();
    if let Some(v) = holder.get_pid(pid) {
        nested_dids(v, &mut out);
    }
    out
}

/// Every struct anywhere inside `value` (itself included) that carries field `marker`,
/// depth first. Used where the spec names the fields of a struct but not every
/// intermediate list label.
fn structs_with<'a>(value: &'a PropertyValue, marker: Pid, out: &mut Vec<&'a PropertyStruct>) {
    if let PropertyValue::Struct(s) = value
        && s.has(marker)
    {
        out.push(s);
        return;
    }
    for c in value.children() {
        structs_with(&c.value, marker, out);
    }
}

fn find_structs<A: PropertyAccess + ?Sized>(
    holder: &A,
    container: Pid,
    marker: Pid,
) -> Vec<&PropertyStruct> {
    let mut out = Vec::new();
    if let Some(v) = holder.get_pid(container) {
        structs_with(v, marker, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Families and classes (§2)
// ---------------------------------------------------------------------------------------------

/// A combat family (`combat-data.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CombatFamily {
    /// Rule 1: the effect base class or a descendant.
    Effect,
    /// Rule 2: the trait base class or a descendant.
    Trait,
    /// Rule 3: the progression base class or a descendant.
    Progression,
    /// Rule 4: the skill class exactly.
    Skill,
    /// Rule 5: monster-play tables and advancement lines.
    MonsterPlayOrAdvancementLine,
    /// Rule 6: per-class advancement tables.
    ClassTable,
    /// Rule 7: threshold policies.
    ThresholdPolicy,
    /// Rule 8: battle statistics.
    BattleStatistic,
}

impl CombatFamily {
    /// Every family, in rule order.
    pub const ALL: [Self; 8] = [
        Self::Effect,
        Self::Trait,
        Self::Progression,
        Self::Skill,
        Self::MonsterPlayOrAdvancementLine,
        Self::ClassTable,
        Self::ThresholdPolicy,
        Self::BattleStatistic,
    ];
}

/// Related objects outside the family subtrees that `combat-data.md` §2.1 also describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelatedObject {
    /// Trait trees (§8.5).
    TraitTree,
    /// Sparse DID progressions (§4.6).
    SparseDidProgression,
    /// The combat control object (§10.1).
    CombatControl,
    /// The effect control object (§6.6).
    EffectControl,
    /// The trait control object (§8.6).
    TraitControl,
    /// DPS lookup tables (§10.2).
    DpsTable,
    /// Item value lookup tables (§10.2).
    ValueTable,
    /// Induction objects named by skills (§7.9).
    Induction,
    /// Channelling objects named by toggle skills (§7.9).
    Channelling,
}

/// Kind of a progression (`combat-data.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProgressionKind {
    /// One property per element, all with the same PID.
    PropertyArray,
    /// f32 elements.
    FloatArray,
    /// Control points with linear interpolation.
    LinearInterpolating,
    /// StringInfo elements.
    TextArray,
    /// DID elements.
    DidArray,
    /// DID elements naming further progressions (two indices, §4.5).
    ProgressionOfProgressions,
}

/// Role of an effect class, this spec's description (`combat-data.md` §6.1). Not a fact
/// about the original's naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(missing_docs)]
pub enum EffectRole {
    PropertyModification,
    VitalOverTime,
    SummonObject,
    ApplyPeriodically,
    ConditionalCombination,
    InstantVital,
    Flag,
    SummonPet,
    Mount,
    CountDown,
    Travel,
    Aura,
    DispelByEffect,
    Tiered,
    GrantBirdType,
    SkillProc,
    Area,
    GroupWide,
    ReactiveVital,
    LootDiscovery,
    Recall,
    InduceCombatState,
    GrantTrait,
    InstantaneousNoData,
    RandomChoice,
    KillProc,
    GrantEmote,
    VitalTransfer,
    Protection,
    GrantSkill,
    SendMail,
    VitalThreshold,
    KnockAway,
    CooldownChange,
    AbsorbBubble,
    PipChange,
    DispelByResistCategory,
    LeaveChannelling,
    RedirectVital,
    ResetSkills,
    ThreatChange,
    Tutorial,
    Stealth,
    FearMovement,
    Tracking,
    GeneratorPropagation,
    EnterChannelling,
    Revive,
    PersistentCombination,
    EffectDataList,
    ThreatTransfer,
    PopulationControl,
    DistantBattleDamage,
    /// A class the role table does not list (the small classes of §6.1's last row, or a
    /// class of another build).
    Other,
}

/// Effect class → role, as observed (`combat-data.md` §6.1). Build-specific.
pub const OBSERVED_EFFECT_ROLES: &[(u32, EffectRole)] = &[
    (734, EffectRole::PropertyModification),
    (755, EffectRole::VitalOverTime),
    (719, EffectRole::SummonObject),
    (708, EffectRole::ApplyPeriodically),
    (767, EffectRole::ConditionalCombination),
    (725, EffectRole::InstantVital),
    (718, EffectRole::Flag),
    (763, EffectRole::SummonPet),
    (2459, EffectRole::Mount),
    (713, EffectRole::CountDown),
    (749, EffectRole::Travel),
    (709, EffectRole::Aura),
    (715, EffectRole::DispelByEffect),
    (3866, EffectRole::Tiered),
    (5724, EffectRole::GrantBirdType),
    (3686, EffectRole::SkillProc),
    (2762, EffectRole::Area),
    (724, EffectRole::GroupWide),
    (736, EffectRole::ReactiveVital),
    (776, EffectRole::LootDiscovery),
    (737, EffectRole::Recall),
    (769, EffectRole::InduceCombatState),
    (722, EffectRole::GrantTrait),
    (745, EffectRole::InstantaneousNoData),
    (2063, EffectRole::RandomChoice),
    (3842, EffectRole::KillProc),
    (720, EffectRole::GrantEmote),
    (756, EffectRole::VitalTransfer),
    (735, EffectRole::Protection),
    (721, EffectRole::GrantSkill),
    (2080, EffectRole::SendMail),
    (3218, EffectRole::VitalThreshold),
    (728, EffectRole::KnockAway),
    (3184, EffectRole::CooldownChange),
    (3222, EffectRole::AbsorbBubble),
    (731, EffectRole::PipChange),
    (714, EffectRole::DispelByResistCategory),
    (766, EffectRole::LeaveChannelling),
    (739, EffectRole::RedirectVital),
    (746, EffectRole::ResetSkills),
    (2281, EffectRole::ThreatChange),
    (2282, EffectRole::ThreatChange),
    (727, EffectRole::Tutorial),
    (780, EffectRole::Stealth),
    (2559, EffectRole::FearMovement),
    (784, EffectRole::Tracking),
    (3833, EffectRole::GeneratorPropagation),
    (3609, EffectRole::EnterChannelling),
    (744, EffectRole::Revive),
    (2441, EffectRole::Revive),
    (2607, EffectRole::DispelByEffect),
    (3124, EffectRole::PersistentCombination),
    (761, EffectRole::EffectDataList),
    (1865, EffectRole::ThreatTransfer),
    (2544, EffectRole::PopulationControl),
    (3839, EffectRole::DistantBattleDamage),
    (768, EffectRole::InstantaneousNoData),
    (726, EffectRole::InstantaneousNoData),
    (770, EffectRole::InstantaneousNoData),
    (779, EffectRole::InstantaneousNoData),
    (2751, EffectRole::InstantaneousNoData),
    (2796, EffectRole::InstantaneousNoData),
    (3099, EffectRole::InstantaneousNoData),
    (3667, EffectRole::InstantaneousNoData),
];

/// The class indices of one build (`combat-data.md` §2.1, §2.2, §4.1). Replace the table
/// for another build; nothing else in this module names a class index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatClasses {
    /// Effect base class (rule 1, "is *c* or an ancestor").
    pub effect: u32,
    /// Trait base class (rule 2).
    pub trait_base: u32,
    /// Progression base class (rule 3).
    pub progression: u32,
    /// Skill class (rule 4, exact).
    pub skill: u32,
    /// Monster-play table class (rule 5, exact).
    pub monster_play: u32,
    /// Advancement-line base class (rule 5, strict ancestor).
    pub advancement_line: u32,
    /// Class advancement table class (rule 6, exact).
    pub class_table: u32,
    /// Threshold-policy base class (rule 7, strict ancestor).
    pub threshold_policy: u32,
    /// Battle-statistic base class (rule 8, strict ancestor).
    pub battle_statistic: u32,
    /// Property-array progressions (§4.1).
    pub property_progression: u32,
    /// Float-array progressions.
    pub float_progression: u32,
    /// Linear-interpolating progressions.
    pub linear_progression: u32,
    /// Text-array progressions.
    pub text_progression: u32,
    /// DID-array progressions.
    pub did_progression: u32,
    /// Progressions of progressions (derives from the DID-array class).
    pub progression_of_progressions: u32,
    /// Sparse DID progressions (§4.6).
    pub sparse_did_progression: u32,
    /// Point-based traits, the cells of trait trees (§8.1).
    pub point_based_trait: u32,
    /// Trait trees (§8.5).
    pub trait_tree: u32,
    /// Combat control object (§10.1).
    pub combat_control: u32,
    /// Effect control object (§6.6).
    pub effect_control: u32,
    /// Trait control object (§8.6).
    pub trait_control: u32,
    /// DPS lookup tables (§10.2).
    pub dps_table: u32,
    /// Item value lookup tables (§10.2).
    pub value_table: u32,
    /// Induction objects (§7.9).
    pub induction: u32,
    /// Channelling objects (§7.9).
    pub channelling: u32,
    /// Effect class → role (§6.1).
    pub effect_roles: Cow<'static, [(u32, EffectRole)]>,
}

impl Default for CombatClasses {
    fn default() -> Self {
        Self::observed()
    }
}

impl CombatClasses {
    /// The indices observed in the install `combat-data.md` was written against.
    #[must_use]
    pub fn observed() -> Self {
        Self {
            effect: 698,
            trait_base: 1479,
            progression: 1386,
            skill: 827,
            monster_play: 1932,
            advancement_line: 2389,
            class_table: 853,
            threshold_policy: 3530,
            battle_statistic: 3584,
            property_progression: 1389,
            float_progression: 1385,
            linear_progression: 3918,
            text_progression: 2402,
            did_progression: 2519,
            progression_of_progressions: 3810,
            sparse_did_progression: 3800,
            point_based_trait: 3438,
            trait_tree: 3486,
            combat_control: 1659,
            effect_control: 1666,
            trait_control: 1486,
            dps_table: 1660,
            value_table: 1688,
            induction: 1195,
            channelling: 961,
            effect_roles: Cow::Borrowed(OBSERVED_EFFECT_ROLES),
        }
    }

    /// The ordered rule table of §2.1 (rule 5 is two tests).
    #[must_use]
    pub fn rules(&self) -> [(ClassTest, CombatFamily); 9] {
        [
            (ClassTest::IsA(self.effect), CombatFamily::Effect),
            (ClassTest::IsA(self.trait_base), CombatFamily::Trait),
            (ClassTest::IsA(self.progression), CombatFamily::Progression),
            (ClassTest::Is(self.skill), CombatFamily::Skill),
            (
                ClassTest::Is(self.monster_play),
                CombatFamily::MonsterPlayOrAdvancementLine,
            ),
            (
                ClassTest::Below(self.advancement_line),
                CombatFamily::MonsterPlayOrAdvancementLine,
            ),
            (ClassTest::Is(self.class_table), CombatFamily::ClassTable),
            (
                ClassTest::Below(self.threshold_policy),
                CombatFamily::ThresholdPolicy,
            ),
            (
                ClassTest::Below(self.battle_statistic),
                CombatFamily::BattleStatistic,
            ),
        ]
    }

    /// The family of root class `class` (§2.1), `None` for a class that is not a combat
    /// object.
    #[must_use]
    pub fn family<P: ClassParents + ?Sized>(
        &self,
        parents: &P,
        class: u32,
    ) -> Option<CombatFamily> {
        super::common::classify_class(parents, class, &self.rules())
    }

    /// The related-object kind of `class` (exact class match; §2.1 last paragraph, §7.9).
    #[must_use]
    pub fn related(&self, class: u32) -> Option<RelatedObject> {
        [
            (self.trait_tree, RelatedObject::TraitTree),
            (
                self.sparse_did_progression,
                RelatedObject::SparseDidProgression,
            ),
            (self.combat_control, RelatedObject::CombatControl),
            (self.effect_control, RelatedObject::EffectControl),
            (self.trait_control, RelatedObject::TraitControl),
            (self.dps_table, RelatedObject::DpsTable),
            (self.value_table, RelatedObject::ValueTable),
            (self.induction, RelatedObject::Induction),
            (self.channelling, RelatedObject::Channelling),
        ]
        .into_iter()
        .find(|&(c, _)| c == class)
        .map(|(_, k)| k)
    }

    /// The progression kind of `class` by ancestry (§4.1). The progression-of-progressions
    /// class is tested before the DID-array class it derives from.
    #[must_use]
    pub fn progression_kind<P: ClassParents + ?Sized>(
        &self,
        parents: &P,
        class: u32,
    ) -> Option<ProgressionKind> {
        let rules = [
            (
                ClassTest::IsA(self.progression_of_progressions),
                ProgressionKind::ProgressionOfProgressions,
            ),
            (
                ClassTest::IsA(self.did_progression),
                ProgressionKind::DidArray,
            ),
            (
                ClassTest::IsA(self.property_progression),
                ProgressionKind::PropertyArray,
            ),
            (
                ClassTest::IsA(self.float_progression),
                ProgressionKind::FloatArray,
            ),
            (
                ClassTest::IsA(self.linear_progression),
                ProgressionKind::LinearInterpolating,
            ),
            (
                ClassTest::IsA(self.text_progression),
                ProgressionKind::TextArray,
            ),
        ];
        super::common::classify_class(parents, class, &rules)
    }

    /// The role of effect class `class` (§6.1); [`EffectRole::Other`] when unlisted.
    #[must_use]
    pub fn effect_role(&self, class: u32) -> EffectRole {
        self.effect_roles
            .iter()
            .find(|&&(c, _)| c == class)
            .map_or(EffectRole::Other, |&(_, r)| r)
    }
}

// ---------------------------------------------------------------------------------------------
// Property IDs by label (§3.1)
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// Progression properties (`combat-data.md` §4.1, §4.2, §4.6).
    pub struct ProgressionPids {
        min_index = "Progression_MinimumIndexValue",
        property_array = "PropertyProgression_Array",
        float_array = "FloatProgression_Array",
        float_value = "FloatProgression_FloatValue",
        linear_array = "LinearInterpolatingProgression_Array",
        linear_point = "LinearInterpolatingProgression_ControlPoint",
        linear_key = "LinearInterpolatingProgression_Key",
        linear_value = "LinearInterpolatingProgression_Value",
        text_array = "StringInfoProgression_Array",
        text_value = "StringInfoProgression_Value",
        did_array = "DataIDProgression_Array",
        did_value = "DataIDProgression_DataIDValue",
        float_value_property = "FloatProgression_ValueProperty",
        linear_value_property = "LinearInterpolatingProgression_ValueProperty",
        inner_property = "ProgressionProgression_InnerRepresentedPropertyName",
        progression_type = "Progression_Type",
        point_tier = "Progression_PointTier",
        sparse_array = "SparseDIDProgression_Array",
        sparse_key = "SparseDIDProgressionEntry_Key",
        sparse_did = "SparseDIDProgressionEntry_DID",
    }
}

crate::property_ids! {
    /// Modifier-entry properties (`combat-data.md` §5.1).
    pub struct ModifierPids {
        array = "Mod_Array",
        entry = "Mod_Entry",
        modified = "Mod_Modified",
        op = "Mod_Op",
        progression = "Mod_Progression",
        floor = "Mod_ProgressionFloor",
        ceiling = "Mod_ProgressionCeiling",
        modifier_list = "Mod_ModifierList",
        modifier_property = "Mod_ModifierProperty",
        modifier_op = "Mod_ModifierOp",
        description_override = "Mod_DescriptionOverride",
        description_override_progression = "Mod_DescriptionOverrideProgression",
        next_rank_description_override = "Mod_NextRankDescriptionOverride",
        next_rank_description_override_progression = "Mod_NextRankDescriptionOverrideProgression",
    }
}

crate::property_ids! {
    /// Properties common to effects (`combat-data.md` §6.2, §6.4).
    pub struct EffectPids {
        name = "Effect_Name",
        definition_description = "Effect_Definition_Description",
        applied_description = "Effect_Applied_Description",
        icon = "Effect_Icon",
        harmful = "Effect_Harmful",
        debuff = "Effect_Debuff",
        application_probability = "Effect_ConstantApplicationProbability",
        probability_variance = "Effect_ApplicationProbabilityVariance",
        probability_additive_property = "Effect_ApplicationProbability_AdditiveModProp",
        probability_progression = "Effect_ApplicationProbabilityProgression",
        resist_category = "Effect_Resist_Category",
        resist_level = "Effect_Resist_Level",
        resist_chance_modifier = "Effect_Resist_Chance_Modifier",
        duration_interval = "Effect_Duration_ConstantInterval",
        duration_pulse_count = "Effect_Duration_ConstantPulseCount",
        duration_permanent = "Effect_Duration_Permanent",
        instantaneous = "Effect_Instantaneous",
        duration_progression_pulse_count = "Effect_Duration_ProgressionPulseCount",
        duration_progression_interval = "Effect_Duration_ProgressionInterval",
        expires_in_real_time = "Effect_Duration_ExpiresInRealTime",
        combat_only = "Effect_Duration_CombatOnly",
        interval_modifier_list = "Effect_Duration_ConstantInterval_ModifierList",
        equivalence_class = "Effect_EquivalenceClass",
        class_priority = "Effect_ClassPriority",
        cap = "Effect_Cap",
        critical_chance = "Effect_Critical_Chance",
        base_vital_type = "Effect_BaseVital_VitalType",
        over_time_vital_type = "Effect_VitalOverTime_VitalType",
        multiplicative = "Effect_InstantVital_Multiplicative",
        damage_type = "Effect_DamageType",
        random_min = "Effect_RandomValueMin",
        random_max = "Effect_RandomValueMax",
        variable_min = "Effect_VariableMin",
        variable_max = "Effect_VariableMax",
        variable_spellcraft_min = "Effect_VariableSpellcraftMin",
        variable_spellcraft_max = "Effect_VariableSpellcraftMax",
    }
}

crate::property_ids! {
    /// The three vital-amount stems of `combat-data.md` §6.4.
    pub struct VitalAmountPids {
        instant_constant = "Effect_InstantVital_InitialChangeConstant",
        instant_progression = "Effect_InstantVital_InitialChangeProgression",
        instant_random = "Effect_InstantVital_InitialChangeRandom",
        instant_variable = "Effect_InstantVital_InitialChangeVariable",
        instant_variance = "Effect_InstantVital_InitialChangeVariance",
        initial_constant = "Effect_VitalOverTime_InitialChangeConstant",
        initial_progression = "Effect_VitalOverTime_InitialChangeProgression",
        initial_random = "Effect_VitalOverTime_InitialChangeRandom",
        initial_variable = "Effect_VitalOverTime_InitialChangeVariable",
        initial_variance = "Effect_VitalOverTime_InitialChangeVariance",
        per_interval_constant = "Effect_VitalOverTime_ChangePerIntervalConstant",
        per_interval_progression = "Effect_VitalOverTime_ChangePerIntervalProgression",
        per_interval_random = "Effect_VitalOverTime_ChangePerIntervalRandom",
        per_interval_variable = "Effect_VitalOverTime_ChangePerIntervalVariable",
        per_interval_variance = "Effect_VitalOverTime_ChangePerIntervalVariance",
    }
}

crate::property_ids! {
    /// Effect-reference structs and their fields (`combat-data.md` §6.3).
    pub struct EffectRefPids {
        generator_struct = "EffectGenerator_EffectStruct",
        generator_id = "EffectGenerator_EffectID",
        generator_spellcraft = "EffectGenerator_EffectSpellcraft",
        generator_data_list = "EffectGenerator_EffectDataList",
        skill_effect_data = "Skill_EffectData",
        skill_effect = "Skill_Effect",
        skill_spellcraft = "Skill_EffectSpellcraft",
        skill_duration = "Skill_EffectDuration",
        skill_implement_usage = "Skill_EffectImplementUsage",
        toggle_data = "Skill_Toggle_Effect_Data",
        toggle_effect = "Skill_Toggle_Effect",
        toggle_spellcraft = "Skill_Toggle_Effect_Spellcraft",
        startup_struct = "Effect_MonsterStartupEffect_Struct",
        startup_id = "Effect_StartupEffectID",
        startup_spellcraft = "Effect_StartupEffectSpellcraft",
        startup_damage_qualifier = "Effect_StartupEffectDamageQualifier",
        bare_effect = "Effect_WSLEffect",
    }
}

crate::property_ids! {
    /// Class-specific effect values (`combat-data.md` §6.5) and the effect control object
    /// (§6.6).
    pub struct EffectExtraPids {
        aura_type = "Effect_Aura_Type",
        aura_radius = "Effect_Aura_Radius",
        aura_applied = "Effect_Aura_Applied_Effect_Array",
        aura_spellcraft_override = "Effect_Aura_Spellcraft_Override",
        aura_affects_caster = "Aura_ShouldAffectCaster",
        area_max_range = "Effect_Area_MaxRange",
        area_max_targets = "Effect_Area_MaxTargets",
        area_applied = "Effect_Area_Applied_Effect_Array",
        apply_over_time = "Effect_ApplyOverTime_Applied_Effect_Array",
        apply_over_time_initial = "Effect_ApplyOverTime_Initial_Applied_Effect_Array",
        proc_skill_types = "Effect_SkillProc_SkillTypes",
        proc_required_result = "Effect_SkillProc_RequiredCombatResult",
        proc_probability = "Effect_SkillProc_ProcProbability",
        proc_min_interval = "Effect_MinTimeBetweenProcs",
        bubble_vital_type = "Effect_Bubble_VitalType",
        bubble_value = "Effect_Bubble_Value",
        bubble_value_progression = "Effect_Bubble_Value_Progression",
        bubble_percentage = "Effect_Bubble_Percentage",
        pip_type = "Effect_Pip_Type",
        pip_amount = "Effect_Pip_AdjustmentAmount",
        pip_adjustment_type = "Effect_Pip_AdjustmentType",
        pip_reset = "Effect_Pip_Reset",
        eqv_array = "Effect_EqvClassData_Array",
        eqv_struct = "Effect_EqvClassData_Struct",
        eqv_max_depth = "Effect_EqvClassData_MaxDepth",
        eqv_max_expressed = "Effect_EqvClassData_MaxExpressedEffs",
        eqv_per_caster = "Effect_EqvClassData_IsPerCaster",
        eqv_sorter = "Effect_EqvClassData_SorterOverride",
        eqv_duration_source = "Effect_EqvClassData_DurationModifier_Source",
        eqv_duration_target = "Effect_EqvClassData_DurationModifier_Target",
        default_sorter = "Effect_Control_DefaultSorter",
        spellcraft_cap_progression = "Effect_Control_SpellcraftCapProgression",
    }
}

crate::property_ids! {
    /// Skill properties (`combat-data.md` §7).
    pub struct SkillPids {
        name = "Skill_Name",
        desc = "Skill_Desc",
        icon = "Skill_Icon",
        large_icon = "Skill_LargeIcon",
        small_icon = "Skill_SmallIcon",
        skill_type = "Skill_SkillType",
        category = "Skill_Category",
        display_skill_type = "Skill_DisplaySkillType",
        harmful = "Skill_Harmful",
        is_monster_skill = "Skill_IsMonsterSkill",
        target_selection_mode = "Skill_TargetSelectionMode",
        vital_cost_list = "Skill_VitalCostList",
        toggle_cost_list = "Skill_Toggle_VitalCostPerSecondList",
        vital_max_list = "Skill_VitalMaxList",
        vital_info = "Skill_VitalInfo",
        vital_type = "Skill_Vital_Type",
        vital_points = "Skill_Vital_Points",
        vital_points_progression = "Skill_Vital_Points_Progression",
        vital_percent = "Skill_Vital_Percent",
        vital_consumes_all = "Skill_Vital_Consumes_All",
        vital_mod_array = "Skill_Vital_Mod_Array",
        min_range = "Skill_MinRange",
        max_range = "Skill_MaxRange",
        max_range_modifiers = "Skill_MaxRange_ModifierArray",
        area_max_targets = "Skill_AreaEffectMaxTargets",
        area_anchor = "Skill_AreaEffectDetectionAnchor",
        sphere_radius = "Skill_AEDetectionVolume_SphereRadius",
        arc_radius = "Skill_AEDetectionVolume_ArcRadius",
        arc_degrees = "Skill_AEDetectionVolume_ArcDegrees",
        box_length = "Skill_AEDetectionVolume_BoxLength",
        box_width = "Skill_AEDetectionVolume_BoxWidth",
        heading_offset = "Skill_AEDetectionVolume_HeadingOffset",
        positional_heading = "Skill_PositionalHeading",
        positional_spread = "Skill_PositionalSpread",
        valid_target_player = "Skill_ValidTargetPlayer",
        valid_target_pet = "Skill_ValidTargetPet",
        valid_target_npc = "Skill_ValidTargetNPC",
        valid_target_monster = "Skill_ValidTargetMonster",
        requires_facing = "Skill_RequiresFacing",
        usable_while_moving = "Skill_UsableWhileMoving",
        required_trait = "Skill_RequiredTrait",
        required_effect_list = "Skill_RequiredEffectList",
        barring_effect_list = "Skill_BarringEffectList",
        can_miss = "Skill_CanMiss",
        miss_chance = "Skill_MissChanceModifier",
        can_be_evaded = "Skill_CanBeEvaded",
        evade_chance = "Skill_EvadeChanceModifier",
        can_be_parried = "Skill_CanBeParried",
        parry_chance = "Skill_ParryChanceModifier",
        can_be_blocked = "Skill_CanBeBlocked",
        block_chance = "Skill_BlockChanceModifier",
        resist_category = "Skill_Resist_Category",
        requires_line_of_sight = "Skill_RequiredLineOfSight",
        requires_area_line_of_sight = "Skill_RequiredAreaEffectLineOfSight",
        user_effects = "Skill_UserEffectList",
        user_effects_override = "Skill_UserEffectList_Override",
        critical_effects = "Skill_CriticalEffectList",
        toggle_effects = "Skill_Toggle_Effect_List",
        toggle_user_effects = "Skill_Toggle_User_Effect_List",
        action_duration = "Skill_ActionDurationContribution",
        recovery_time = "Skill_SkillRecoveryTime",
        recovery_time_modifiers = "Skill_SkillRecoveryTime_Mod_Array",
        recovery_channel = "Skill_RecoveryChannel",
        ignores_reset_time = "Skill_IgnoresResetTime",
        immediate = "Skill_Immediate",
        induction = "Skill_InductionAction",
        channelling_state = "Skill_Toggle_ChannelingState",
        channelling_duration_override = "Skill_Toggle_ChannelingDurationOverride",
        toggle_hook_number = "Skill_Toggle_Hook_Number",
    }
}

crate::property_ids! {
    /// Attack-hook fields (`combat-data.md` §7.7).
    pub struct AttackHookPids {
        list = "Skill_AttackHookList",
        info = "Skill_AttackHookInfo",
        animation_hook_index = "Skill_AttackHook_AnimationHookIndex",
        damage_max = "Skill_AttackHook_HookDamageMax",
        damage_max_progression = "Skill_AttackHook_HookDamageMax_Progression",
        damage_max_variance = "Skill_AttackHook_HookDamageMaxVariance",
        damage_modifier = "Skill_AttackHook_HookDamageModifier",
        dps_add_progression = "Skill_AttackHook_DPSAddMod_Progression",
        damage_add_multiplier = "Skill_AttackHook_DamageAddContributionMultiplier",
        implement_multiplier = "Skill_AttackHook_ImplementContributionMultiplier",
        damage_type = "Skill_AttackHook_DamageType",
        damage_type_override = "Skill_AttackHook_DamageTypeOverride",
        damage_qualifier = "Skill_AttackHook_DamageQualifier",
        critical_chance = "Skill_AttackHook_ChanceCriticalHit",
        critical_multiplier = "Skill_AttackHook_CriticalDamageMultiplier",
        super_critical_multiplier = "Skill_AttackHook_SuperCriticalDamageMultiplier",
        positional_multiplier = "Skill_AttackHook_PositionalDamageMultiplier",
        mitigation_modifier = "Skill_AttackHook_MitigationModifier",
        threat_value = "Skill_AttackHook_AdditionalThreatValue",
        threat_type = "Skill_AttackHook_AdditionalThreatType",
        uses_primary = "Skill_AttackHook_UsesPrimaryImplement",
        uses_secondary = "Skill_AttackHook_UsesSecondaryImplement",
        uses_ranged = "Skill_AttackHook_UsesRangedImplement",
        uses_natural = "Skill_AttackHook_UsesNatural",
        uses_tactical = "Skill_AttackHook_UsesTactical",
        can_critical = "Skill_CanCriticalHit",
        target_effects = "Skill_AttackHook_TargetEffectList",
        critical_target_effects = "Skill_AttackHook_CriticalTargetEffectList",
        super_critical_target_effects = "Skill_AttackHook_SuperCriticalTargetEffectList",
        positional_target_effects = "Skill_AttackHook_PositionalTargetEffectList",
    }
}

crate::property_ids! {
    /// Induction and channelling objects (`combat-data.md` §7.9).
    pub struct TimingObjectPids {
        induction_duration = "Induction_Duration",
        induction_interruptible = "Induction_Interruptable",
        induction_allowed_while_moving = "Induction_AllowedWhileMoving",
        induction_allowed_in_combat = "Induction_AllowedInCombat",
        induction_additive_modifiers = "Induction_Duration_AdditiveModifierList",
        induction_multiplier_modifiers = "Induction_Duration_MultiplierModifierList",
        channelling_duration = "Channeling_Duration",
        channelling_multiplier_property = "Channeling_Duration_Multiplier_PropertyName",
    }
}

crate::property_ids! {
    /// Trait properties (`combat-data.md` §8.1–§8.6).
    pub struct TraitPids {
        name = "Trait_Name",
        description = "Trait_Description",
        tooltip = "Trait_Tooltip",
        nature = "Trait_Nature",
        category = "Trait_Category",
        tier = "Trait_Tier",
        cost_to_slot = "Trait_Cost_To_Slot",
        icon = "Trait_Icon",
        minimum_level = "Trait_Minimum_Level",
        maximum_rank = "Trait_Virtue_Maximum_Rank",
        rank_property = "Trait_Virtue_Rank_PropertyName",
        rank_overlay_progression = "Trait_Rank_Overlay_Progression",
        first_purchasable_rank = "Trait_First_Purchasable_Rank",
        point_cost_progression = "Trait_PointBasedTrait_PointCostProgression",
        required_character_level = "Trait_Prerequisite_CharacterLevel",
        effect_list = "EffectGenerator_TraitEffectList",
        skill_array = "Trait_Skill_Array",
        tree_type = "Trait_TraitTree_TreeType",
        tree_array = "Trait_TraitTree_TraitArray",
        tree_struct = "Trait_TraitTree_TraitStruct",
        tree_trait = "Trait_TraitTree_Trait",
        tree_branch = "Trait_TraitTree_Branch",
        tree_location = "Trait_TraitTree_TraitLocation",
        tree_dependencies = "Trait_TraitTree_TraitDependencyArray",
        tree_dependency = "Trait_TraitTree_TraitDependency",
        tree_dependency_rank = "Trait_TraitTree_TraitDependencyRank",
        tree_specializations = "Trait_TraitTree_SpecializationsArray",
        tree_specialization_progression = "Trait_TraitTree_SpecializationProgression",
        control_level_to_points = "Trait_Control_LevelToTotalClassTraitPointsProgression",
        control_points_to_refund = "Trait_Control_ClassTraitPointsToRefundCostProgression",
        control_rank_to_level = "Trait_Control_TraitRank_To_Level_Equivalent",
        control_level_to_virtue_rank_min = "Trait_Control_LevelScaling_LevelToVirtueRank_Min",
        control_level_to_virtue_rank_max = "Trait_Control_LevelScaling_LevelToVirtueRank_Max",
        control_virtue_tier_to_item_level = "Trait_Control_VirtueTierToItemLevelProgression",
        control_player_traits = "Trait_Control_FreepTraits",
        control_monster_traits = "Trait_Control_CreepTraits",
        control_skirmish_traits = "Trait_Control_SkirmishTraits",
        control_trait_did = "Trait_Control_TraitDID",
        control_virtue_xp_soft_cap = "Trait_Control_VirtueXP_Bonus_SoftCap",
        control_virtue_xp_hard_cap = "Trait_Control_VirtueXP_Bonus_HardCap",
        control_virtue_tier_to_experience = "Trait_Control_VirtueTierToExperience_AdvancementTable",
    }
}

crate::property_ids! {
    /// Class and advancement tables (`combat-data.md` §9).
    pub struct ClassTablePids {
        class = "AdvTable_Class",
        level_entry_list = "AdvTable_LevelEntryList",
        level_entry = "AdvTable_LevelEntry",
        base_vital_list = "AdvTable_BaseVitalEntryList",
        vital_type = "AdvTable_VitalType",
        base_vital_value = "AdvTable_BaseVitalValue",
        max_stat_list = "AdvTable_MaxStatEntryList",
        stat_type = "AdvTable_StatType",
        stat_value = "AdvTable_StatValue",
        derived_configuration = "AdvTable_DerivedStat_Configuration",
        derived_formula = "AdvTable_DerivedStat_Formula",
        derived_stat = "AdvTable_DerivedStat",
        derived_term_stat = "AdvTable_DerivedStat_Formula_Stat",
        derived_term_value = "AdvTable_DerivedStat_Formula_Value",
        armor_calc_type = "AdvTable_ArmorDefense_Points_CalcType",
        available_skills = "AdvTable_AvailableSkillEntryList",
        available_mount_skills = "AdvTable_AvailableMountSkillEntryList",
        characteristics = "AdvTable_ClassCharacteristic_List",
        characteristic_trait = "AdvTable_Trait_WC",
        line_entry_list = "AdvLine_EntryList",
        line_entry = "AdvLine_Entry",
        line_data_id = "AdvLine_DataID",
        monster_level = "MonsterPlay_MonsterLevel",
        monster_skill_list = "MonsterPlay_SkillList",
        monster_trait_list = "MonsterPlay_TraitList",
    }
}

crate::property_ids! {
    /// Combat control, lookup tables, thresholds and battle statistics
    /// (`combat-data.md` §10).
    pub struct CombatTablePids {
        calc_array = "Combat_Control_CalcControlInfoArray",
        calc_struct = "Combat_Control_CalcControlInfoStruct",
        calc_type = "Combat_Control_CalculationType",
        calc_rating = "Combat_Control_Rating",
        calc_target_cap = "Combat_Control_Target_Cap",
        calc_hard_cap = "Combat_Control_Hard_Cap",
        max_combat_level = "Combat_Control_MaxCombatLevel",
        base_dps_array = "Combat_BaseDPSArray",
        base_dps = "Combat_BaseDPS",
        dps_quality_array = "Combat_QualityModArray",
        dps_quality_data = "Combat_QualityModData",
        dps_quality = "Combat_Quality",
        dps_quality_mod = "Combat_DPSMod",
        base_value_array = "Item_BaseValArray",
        base_value_struct = "Item_BaseValStruct",
        base_value = "Item_BaseVal",
        value_quality_array = "Item_QualityArray",
        value_quality_struct = "Item_QualityStruct",
        value_quality = "Item_Quality",
        value_quality_mod = "Item_QualityModVal",
        threshold_policy = "Threshold_PercentageValue_Policy",
        threshold_index_property = "Threshold_Policy_CurrentStateIndexProperty",
        threshold_state_list = "Threshold_Policy_StateList",
        threshold_state = "Threshold_EffectScript_State",
        threshold_min = "Threshold_State_MinValue_Float",
        threshold_max = "Threshold_State_MaxValue_Float",
        threshold_effects = "Threshold_State_EffectList",
        battle_stat_name = "BattleStat_Name",
        battle_stat_description = "BattleStat_Description",
        battle_stat_scope = "BattleStat_Scope",
        battle_stat_update_policy = "BattleStat_UpdatePolicy",
        battle_stat_replicate = "BattleStat_ReplicateToClients",
        battle_stat_forward = "BattleStat_ForwardUpdatesToStat",
    }
}

/// Every combat property ID, resolved by label once per install (`combat-data.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct CombatPids {
    pub progression: ProgressionPids,
    pub modifier: ModifierPids,
    pub effect: EffectPids,
    pub vital: VitalAmountPids,
    pub effect_ref: EffectRefPids,
    pub effect_extra: EffectExtraPids,
    pub skill: SkillPids,
    pub hook: AttackHookPids,
    pub timing: TimingObjectPids,
    pub traits: TraitPids,
    pub class_table: ClassTablePids,
    pub tables: CombatTablePids,
}

impl CombatPids {
    /// Resolves every label against `names` (normally the property dictionary).
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            progression: ProgressionPids::resolve(names),
            modifier: ModifierPids::resolve(names),
            effect: EffectPids::resolve(names),
            vital: VitalAmountPids::resolve(names),
            effect_ref: EffectRefPids::resolve(names),
            effect_extra: EffectExtraPids::resolve(names),
            skill: SkillPids::resolve(names),
            hook: AttackHookPids::resolve(names),
            timing: TimingObjectPids::resolve(names),
            traits: TraitPids::resolve(names),
            class_table: ClassTablePids::resolve(names),
            tables: CombatTablePids::resolve(names),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            progression: ProgressionPids::unresolved(),
            modifier: ModifierPids::unresolved(),
            effect: EffectPids::unresolved(),
            vital: VitalAmountPids::unresolved(),
            effect_ref: EffectRefPids::unresolved(),
            effect_extra: EffectExtraPids::unresolved(),
            skill: SkillPids::unresolved(),
            hook: AttackHookPids::unresolved(),
            timing: TimingObjectPids::unresolved(),
            traits: TraitPids::unresolved(),
            class_table: ClassTablePids::unresolved(),
            tables: CombatTablePids::unresolved(),
        }
    }

    /// Every label, in declaration order of the groups.
    #[must_use]
    pub fn labels() -> Vec<&'static str> {
        [
            ProgressionPids::LABELS,
            ModifierPids::LABELS,
            EffectPids::LABELS,
            VitalAmountPids::LABELS,
            EffectRefPids::LABELS,
            EffectExtraPids::LABELS,
            SkillPids::LABELS,
            AttackHookPids::LABELS,
            TimingObjectPids::LABELS,
            TraitPids::LABELS,
            ClassTablePids::LABELS,
            CombatTablePids::LABELS,
        ]
        .concat()
    }

    /// The labels the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        [
            self.progression.missing(),
            self.modifier.missing(),
            self.effect.missing(),
            self.vital.missing(),
            self.effect_ref.missing(),
            self.effect_extra.missing(),
            self.skill.missing(),
            self.hook.missing(),
            self.timing.missing(),
            self.traits.missing(),
            self.class_table.missing(),
            self.tables.missing(),
        ]
        .concat()
    }

    /// Whether `set` carries the signature properties of `family` (`combat-data.md` §2.1
    /// last column; invariant F2). Skills are checked for the three named properties only.
    #[must_use]
    pub fn has_signature<A: PropertyAccess + ?Sized>(&self, family: CombatFamily, set: &A) -> bool {
        let e = &self.effect;
        let t = &self.traits;
        let p = &self.progression;
        let s = &self.skill;
        let c = &self.class_table;
        let x = &self.tables;
        match family {
            CombatFamily::Effect => [e.name, e.definition_description, e.applied_description]
                .iter()
                .all(|&q| set.has(q)),
            CombatFamily::Trait => [t.name, t.nature, t.category, t.tier, t.cost_to_slot]
                .iter()
                .all(|&q| set.has(q)),
            CombatFamily::Progression => value_arrays_present(set, p) > 0,
            CombatFamily::Skill => [s.name, s.skill_type, s.target_selection_mode]
                .iter()
                .all(|&q| set.has(q)),
            CombatFamily::MonsterPlayOrAdvancementLine => {
                set.has(c.armor_calc_type) || set.has(c.line_entry_list)
            }
            CombatFamily::ClassTable => {
                set.has(c.level_entry_list) && set.has(c.derived_configuration)
            }
            CombatFamily::ThresholdPolicy => set.has(x.threshold_policy),
            CombatFamily::BattleStatistic => {
                set.has(x.battle_stat_name) && set.has(x.battle_stat_scope)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Progressions (§4)
// ---------------------------------------------------------------------------------------------

/// Number of §4.1 value arrays `set` holds (invariant P1: exactly one in every progression).
#[must_use]
pub fn value_arrays_present<A: PropertyAccess + ?Sized>(set: &A, pids: &ProgressionPids) -> usize {
    [
        pids.property_array,
        pids.float_array,
        pids.linear_array,
        pids.text_array,
        pids.did_array,
    ]
    .iter()
    .filter(|&&p| set.has(p))
    .count()
}

/// One control point of a linear-interpolating progression (`combat-data.md` §4.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    /// The integer key *x*.
    pub key: i32,
    /// The value *y*.
    pub value: f32,
}

/// One segment between adjacent control points, with its slope (`combat-data.md` §4.4,
/// §4.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Start point.
    pub start: ControlPoint,
    /// End point.
    pub end: ControlPoint,
    /// (*y₁* − *y₀*) / f32(*x₁* − *x₀*), computed in single precision (P4).
    pub slope: f32,
}

/// The slope of the segment from `a` to `b` exactly as `combat-data.md` §4.4 describes it:
/// the f32 difference of the values, the integer difference of the keys converted to f32,
/// one f32 division.
#[must_use]
pub fn segment_slope(a: ControlPoint, b: ControlPoint) -> f32 {
    let dy = b.value - a.value;
    let dx = (i64::from(b.key) - i64::from(a.key)) as f32;
    dy / dx
}

/// The value array of a progression (`combat-data.md` §4.1).
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressionData {
    /// Property array: each element with its own PID and value (P2: one PID per record).
    Property(Vec<PropertyEntry>),
    /// Float array.
    Float(Vec<f32>),
    /// Linear interpolating: control points in stored order.
    Linear(Vec<ControlPoint>),
    /// Text array.
    Text(Vec<StringInfo>),
    /// DID array (null DIDs kept: element positions are indices).
    Did(Vec<Did>),
    /// Progression of progressions: DIDs of inner progressions.
    Progressions(Vec<Did>),
}

/// A value a progression yields (`combat-data.md` §4.3, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressionValue {
    /// From a float array or a linear progression.
    Float(f32),
    /// From a property array: the element's value, of its own type.
    Property(PropertyValue),
    /// From a text array.
    Text(StringInfo),
    /// From a DID array (and, for [`Progression::lookup`], the selected inner progression
    /// of a progression of progressions).
    Did(Did),
}

impl ProgressionValue {
    /// The value as a property value (floats as type 9, DIDs as type 20, text as type 13).
    #[must_use]
    pub fn into_property_value(self) -> PropertyValue {
        match self {
            Self::Float(v) => PropertyValue::Float(v),
            Self::Property(v) => v,
            Self::Text(s) => PropertyValue::StringInfo(Box::new(s)),
            Self::Did(d) => PropertyValue::DidRef(d),
        }
    }

    /// The value as f32: floats, and property-array numbers ([`PropertyValue::as_f64`]).
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Property(v) => v.as_f64().map(|x| x as f32),
            _ => None,
        }
    }
}

/// Result of a lookup without the D1 clamp (`combat-data.md` §4.8, §15).
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    /// The index is covered.
    Value(T),
    /// The index is below the first covered index.
    BelowRange,
    /// The index is above the last covered index.
    AboveRange,
    /// The progression has no elements.
    Empty,
    /// The progression's keys do not strictly increase (P3 broken).
    Malformed,
}

impl<T> Lookup<T> {
    /// The value, if the index was covered.
    pub fn value(self) -> Option<T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }
}

/// A progression (`combat-data.md` §4): a map from an integer index to a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Progression {
    /// `Progression_MinimumIndexValue` as stored (absent in linear progressions and in six
    /// float arrays).
    pub stored_min_index: Option<i32>,
    /// `FloatProgression_ValueProperty` or `LinearInterpolatingProgression_ValueProperty`:
    /// the property the curve stands for (informational, §3.3).
    pub value_property: Option<u32>,
    /// `ProgressionProgression_InnerRepresentedPropertyName` (§4.2).
    pub inner_property: Option<u32>,
    /// `Progression_Type` enum key (not needed to evaluate).
    pub progression_type: Option<u32>,
    /// `Progression_PointTier` enum key (not needed to evaluate).
    pub point_tier: Option<u32>,
    /// The value array.
    pub data: ProgressionData,
    /// Whether linear keys strictly increase (P3); always true for array kinds.
    keys_increasing: bool,
}

impl Progression {
    /// Reads a progression from its property set. `nested` says whether a DID array names
    /// inner progressions (the record's class is the progression-of-progressions class,
    /// [`CombatClasses::progression_kind`]).
    ///
    /// The value array is chosen as in §4.10 steps 1–2: the linear array first, then the
    /// first present of property, float, text and DID arrays. Fails with
    /// [`FormatError::MissingProperty`] when there is none and [`FormatError::Malformed`]
    /// when the array counts null elements (element positions would be lost) or an element
    /// has the wrong type.
    pub fn from_properties<A: PropertyAccess + ?Sized>(
        set: &A,
        nested: bool,
        pids: &ProgressionPids,
    ) -> Result<Self> {
        const WHAT: &str = "progression";
        let elements = |pid: Pid| -> Option<Result<&PropertyArray>> {
            let arr = set.array(pid)?;
            Some(if arr.null_entries() > 0 {
                Err(malformed(WHAT, "value array counts null elements"))
            } else {
                Ok(arr)
            })
        };
        let mut keys_increasing = true;
        let data = if let Some(arr) = elements(pids.linear_array) {
            let arr = arr?;
            let mut points = Vec::with_capacity(arr.elements.len());
            for e in &arr.elements {
                let s = e
                    .value
                    .as_struct()
                    .ok_or_else(|| malformed(WHAT, "control point is not a struct"))?;
                let key = s.require_int(pids.linear_key, WHAT)?;
                let value = s.require_float(pids.linear_value, WHAT)?;
                points.push(ControlPoint { key, value });
            }
            keys_increasing = points.windows(2).all(|w| match w {
                [a, b] => a.key < b.key,
                _ => true,
            });
            ProgressionData::Linear(points)
        } else if let Some(arr) = elements(pids.property_array) {
            ProgressionData::Property(arr?.elements.clone())
        } else if let Some(arr) = elements(pids.float_array) {
            let v = arr?
                .elements
                .iter()
                .map(|e| {
                    e.float()
                        .ok_or_else(|| malformed(WHAT, "float array element is not a float"))
                })
                .collect::<Result<Vec<f32>>>()?;
            ProgressionData::Float(v)
        } else if let Some(arr) = elements(pids.text_array) {
            let v = arr?
                .elements
                .iter()
                .map(|e| {
                    e.string_info()
                        .cloned()
                        .ok_or_else(|| malformed(WHAT, "text array element is not a StringInfo"))
                })
                .collect::<Result<Vec<StringInfo>>>()?;
            ProgressionData::Text(v)
        } else if let Some(arr) = elements(pids.did_array) {
            let v = arr?
                .elements
                .iter()
                .map(|e| {
                    e.did_ref()
                        .ok_or_else(|| malformed(WHAT, "DID array element is not a DID"))
                })
                .collect::<Result<Vec<Did>>>()?;
            if nested {
                ProgressionData::Progressions(v)
            } else {
                ProgressionData::Did(v)
            }
        } else {
            return Err(FormatError::MissingProperty {
                what: WHAT,
                label: "a value array of combat-data.md §4.1",
            });
        };
        Ok(Self {
            stored_min_index: set.int(pids.min_index),
            value_property: nonzero_pid(
                set.property_ref(pids.float_value_property)
                    .or_else(|| set.property_ref(pids.linear_value_property)),
            ),
            inner_property: nonzero_pid(set.property_ref(pids.inner_property)),
            progression_type: set.enum_key(pids.progression_type),
            point_tier: set.enum_key(pids.point_tier),
            data,
            keys_increasing,
        })
    }

    /// Reads the progression of `record`, deciding nesting by its root class.
    pub fn read<P: ClassParents + ?Sized>(
        record: &GameObjectRecord,
        classes: &CombatClasses,
        parents: &P,
        pids: &ProgressionPids,
    ) -> Result<Self> {
        let nested = classes.progression_kind(parents, record.root_class())
            == Some(ProgressionKind::ProgressionOfProgressions);
        Self::from_properties(&record.properties, nested, pids)
    }

    /// The kind, from the value array.
    #[must_use]
    pub fn kind(&self) -> ProgressionKind {
        match self.data {
            ProgressionData::Property(_) => ProgressionKind::PropertyArray,
            ProgressionData::Float(_) => ProgressionKind::FloatArray,
            ProgressionData::Linear(_) => ProgressionKind::LinearInterpolating,
            ProgressionData::Text(_) => ProgressionKind::TextArray,
            ProgressionData::Did(_) => ProgressionKind::DidArray,
            ProgressionData::Progressions(_) => ProgressionKind::ProgressionOfProgressions,
        }
    }

    /// Number of elements (control points for linear progressions).
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.data {
            ProgressionData::Property(v) => v.len(),
            ProgressionData::Float(v) => v.len(),
            ProgressionData::Linear(v) => v.len(),
            ProgressionData::Text(v) => v.len(),
            ProgressionData::Did(v) | ProgressionData::Progressions(v) => v.len(),
        }
    }

    /// Whether the value array is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The minimum index *m* (§4.2, P5): the stored value, else 0; for linear progressions
    /// the first key (0 when there is no point).
    #[must_use]
    pub fn min_index(&self) -> i64 {
        match &self.data {
            ProgressionData::Linear(p) => p.first().map_or(0, |p| i64::from(p.key)),
            _ => i64::from(self.stored_min_index.unwrap_or(0)),
        }
    }

    /// The covered index range, both ends inclusive; `None` when empty or malformed.
    #[must_use]
    pub fn covered_range(&self) -> Option<(i64, i64)> {
        if !self.keys_increasing {
            return None;
        }
        match &self.data {
            ProgressionData::Linear(p) => {
                Some((i64::from(p.first()?.key), i64::from(p.last()?.key)))
            }
            _ => {
                let n = i64::try_from(self.len()).ok().filter(|&n| n > 0)?;
                let m = self.min_index();
                Some((m, m + n - 1))
            }
        }
    }

    /// Whether linear keys strictly increase (P3); true for array kinds.
    #[must_use]
    pub fn keys_increasing(&self) -> bool {
        self.keys_increasing
    }

    /// The PID every property-array element carries, when they all carry the same one
    /// (P2): the property the progression produces. `None` for other kinds, an empty array
    /// or mixed PIDs.
    #[must_use]
    pub fn produced_property(&self) -> Option<u32> {
        let ProgressionData::Property(v) = &self.data else {
            return None;
        };
        let first = v.first()?.pid;
        v.iter().all(|e| e.pid == first).then_some(first)
    }

    /// The segments of a linear progression with their slopes (§4.4, §4.7): *n* − 1 for
    /// *n* points. Empty for other kinds and for keys that do not strictly increase.
    #[must_use]
    pub fn segments(&self) -> Vec<Segment> {
        let ProgressionData::Linear(p) = &self.data else {
            return Vec::new();
        };
        if !self.keys_increasing {
            return Vec::new();
        }
        p.windows(2)
            .filter_map(|w| match w {
                [a, b] => Some(Segment {
                    start: *a,
                    end: *b,
                    slope: segment_slope(*a, *b),
                }),
                _ => None,
            })
            .collect()
    }

    fn element(&self, k: usize) -> Option<ProgressionValue> {
        Some(match &self.data {
            ProgressionData::Property(v) => ProgressionValue::Property(v.get(k)?.value.clone()),
            ProgressionData::Float(v) => ProgressionValue::Float(*v.get(k)?),
            ProgressionData::Text(v) => ProgressionValue::Text(v.get(k)?.clone()),
            ProgressionData::Did(v) | ProgressionData::Progressions(v) => {
                ProgressionValue::Did(*v.get(k)?)
            }
            ProgressionData::Linear(_) => return None,
        })
    }

    /// The value at index `x` without any clamp (`combat-data.md` §4.3, §4.4; what a viewer
    /// shows, §15). For a progression of progressions this is the selected inner
    /// progression's DID ([`ProgressionValue::Did`]); [`evaluate_progression`] follows it.
    #[must_use]
    pub fn lookup(&self, x: i64) -> Lookup<ProgressionValue> {
        if let ProgressionData::Linear(points) = &self.data {
            return match linear_lookup(points, self.keys_increasing, x) {
                Lookup::Value(v) => Lookup::Value(ProgressionValue::Float(v)),
                Lookup::BelowRange => Lookup::BelowRange,
                Lookup::AboveRange => Lookup::AboveRange,
                Lookup::Empty => Lookup::Empty,
                Lookup::Malformed => Lookup::Malformed,
            };
        }
        let n = self.len();
        if n == 0 {
            return Lookup::Empty;
        }
        let k = i128::from(x) - i128::from(self.min_index());
        if k < 0 {
            return Lookup::BelowRange;
        }
        match usize::try_from(k).ok().and_then(|k| self.element(k)) {
            Some(v) => Lookup::Value(v),
            None => Lookup::AboveRange,
        }
    }

    /// The value at index `x`, an index outside the covered range clamped to its nearest
    /// end and logged (**Decision D1**, not a fact about the original; §4.8). `None` when
    /// the progression is empty or its keys do not strictly increase. For a progression of
    /// progressions, the selected inner DID (see [`Progression::lookup`]).
    #[must_use]
    pub fn value_at(&self, x: i64) -> Option<ProgressionValue> {
        let (lo, hi) = self.covered_range()?;
        let clamped = x.clamp(lo, hi);
        if clamped != x {
            tracing::debug!(index = x, clamped, lo, hi, "progression index clamped (D1)");
        }
        self.lookup(clamped).value()
    }

    /// Evaluates this progression with the algorithm of §4.10 from step 2: `index` is the
    /// (outer) index, `inner` the inner index used only by a progression of progressions,
    /// whose elements are loaded from `source`.
    #[must_use]
    pub fn evaluate<S: ProgressionSource + ?Sized>(
        &self,
        source: &S,
        index: i64,
        inner: Option<i64>,
    ) -> Option<ProgressionValue> {
        self.evaluate_depth(source, index, inner, 0)
    }

    fn evaluate_depth<S: ProgressionSource + ?Sized>(
        &self,
        source: &S,
        index: i64,
        inner: Option<i64>,
        depth: usize,
    ) -> Option<ProgressionValue> {
        if !self.keys_increasing {
            tracing::debug!("linear progression keys do not strictly increase; no value");
            return None;
        }
        let selected = self.value_at(index)?;
        if !matches!(self.data, ProgressionData::Progressions(_)) {
            return Some(selected);
        }
        // §4.10 step 5 (Decision D4).
        let Some(u) = inner else {
            tracing::debug!("progression of progressions without an inner index; no value");
            return None;
        };
        let ProgressionValue::Did(did) = selected else {
            return None;
        };
        if did.is_null() {
            return None;
        }
        evaluate_at_depth(source, did, u, None, depth + 1)
    }
}

/// §4.4 evaluation over control points, without clamp.
fn linear_lookup(points: &[ControlPoint], increasing: bool, x: i64) -> Lookup<f32> {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return Lookup::Empty;
    };
    if !increasing {
        return Lookup::Malformed;
    }
    if x < i64::from(first.key) {
        return Lookup::BelowRange;
    }
    if x > i64::from(last.key) {
        return Lookup::AboveRange;
    }
    match points.binary_search_by(|p| i64::from(p.key).cmp(&x)) {
        // Step 1 (Decision D3): the stored value at any key, the last one included.
        Ok(i) => points
            .get(i)
            .map_or(Lookup::Malformed, |p| Lookup::Value(p.value)),
        // Step 2: the segment with x_i < x < x_{i+1}.
        Err(i) => {
            let (Some(a), Some(b)) = (i.checked_sub(1).and_then(|j| points.get(j)), points.get(i))
            else {
                return Lookup::Malformed;
            };
            let s = segment_slope(*a, *b);
            let dx = (x - i64::from(a.key)) as f32;
            Lookup::Value(a.value + s * dx)
        }
    }
}

/// Where [`evaluate_progression`] loads progressions from. `None` means "missing" (§4.10
/// step 1).
pub trait ProgressionSource {
    /// The progression with DID `did`.
    fn progression(&self, did: Did) -> Option<Cow<'_, Progression>>;
}

impl<S: std::hash::BuildHasher> ProgressionSource for HashMap<Did, Progression, S> {
    fn progression(&self, did: Did) -> Option<Cow<'_, Progression>> {
        self.get(&did).map(Cow::Borrowed)
    }
}

impl ProgressionSource for BTreeMap<Did, Progression> {
    fn progression(&self, did: Did) -> Option<Cow<'_, Progression>> {
        self.get(&did).map(Cow::Borrowed)
    }
}

impl<F: Fn(Did) -> Option<Progression>> ProgressionSource for F {
    fn progression(&self, did: Did) -> Option<Cow<'_, Progression>> {
        self(did).map(Cow::Owned)
    }
}

/// Longest chain of progressions followed (**Decision D4**; observed depth is 1).
pub const MAX_PROGRESSION_CHAIN: usize = 4;

/// The evaluation algorithm of `combat-data.md` §4.10: loads `did` from `source`
/// (missing → `None`), evaluates it at `index` with the D1 clamp, and for a progression of
/// progressions evaluates the selected inner progression at `inner` with no inner index of
/// its own (D4: no inner index → `None`).
#[must_use]
pub fn evaluate_progression<S: ProgressionSource + ?Sized>(
    source: &S,
    did: Did,
    index: i64,
    inner: Option<i64>,
) -> Option<ProgressionValue> {
    evaluate_at_depth(source, did, index, inner, 0)
}

fn evaluate_at_depth<S: ProgressionSource + ?Sized>(
    source: &S,
    did: Did,
    index: i64,
    inner: Option<i64>,
    depth: usize,
) -> Option<ProgressionValue> {
    if depth >= MAX_PROGRESSION_CHAIN {
        tracing::warn!(%did, "progression chain longer than {MAX_PROGRESSION_CHAIN}; cycle assumed");
        return None;
    }
    if did.is_null() {
        return None;
    }
    let p = source.progression(did)?;
    p.evaluate_depth(source, index, inner, depth)
}

/// A sparse DID progression (`combat-data.md` §4.6): key–DID pairs in stored order.
///
/// The lookup rule is OPEN (§18 Q4), so no lookup is offered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SparseDidProgression {
    /// (`SparseDIDProgressionEntry_Key`, `SparseDIDProgressionEntry_DID`) pairs.
    pub entries: Vec<(f32, Did)>,
}

impl SparseDidProgression {
    /// Reads the entries of `SparseDIDProgression_Array`; structs missing either field are
    /// skipped.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, pids: &ProgressionPids) -> Self {
        let entries = set
            .structs(pids.sparse_array)
            .filter_map(|s| Some((s.float(pids.sparse_key)?, s.did(pids.sparse_did)?)))
            .collect();
        Self { entries }
    }
}

// ---------------------------------------------------------------------------------------------
// Modifier entries (§5)
// ---------------------------------------------------------------------------------------------

/// An operator of enum mapper `PropertyOp` (`combat-data.md` §5.3). The arithmetic of each
/// is Reported only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModOp {
    /// Key 0 (empty entries only).
    Invalid,
    /// Key 1 (unused).
    Difference,
    /// Key 2 (unused).
    Divide,
    /// Key 3: bitwise or.
    Or,
    /// Key 4 (unused).
    Random,
    /// Key 5: replace.
    Set,
    /// Key 6: subtract.
    Subtract,
    /// Key 7: add.
    Add,
    /// Key 8: multiply.
    Multiply,
    /// Key 10: append array elements.
    ArrayAdd,
    /// Key 11 (unused).
    And,
    /// Key 12 (unused).
    Xor,
}

impl ModOp {
    /// Every operator, by observed key.
    pub const ALL: [Self; 12] = [
        Self::Invalid,
        Self::Difference,
        Self::Divide,
        Self::Or,
        Self::Random,
        Self::Set,
        Self::Subtract,
        Self::Add,
        Self::Multiply,
        Self::ArrayAdd,
        Self::And,
        Self::Xor,
    ];

    /// The operator's label in the mapper (an interoperability identifier).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Difference => "Difference",
            Self::Divide => "Divide",
            Self::Or => "Or",
            Self::Random => "Random",
            Self::Set => "Set",
            Self::Subtract => "Subtract",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::ArrayAdd => "ArrayAdd",
            Self::And => "And",
            Self::Xor => "Xor",
        }
    }

    /// The key observed for the operator (the fallback when no mapper is at hand).
    #[must_use]
    pub const fn observed_key(self) -> u32 {
        match self {
            Self::Invalid => 0,
            Self::Difference => 1,
            Self::Divide => 2,
            Self::Or => 3,
            Self::Random => 4,
            Self::Set => 5,
            Self::Subtract => 6,
            Self::Add => 7,
            Self::Multiply => 8,
            Self::ArrayAdd => 10,
            Self::And => 11,
            Self::Xor => 12,
        }
    }

    /// The operator with label `label`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.label() == label)
    }

    /// The operator with observed key `key`.
    #[must_use]
    pub fn from_observed_key(key: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.observed_key() == key)
    }

    /// Resolves `key` by its label in `mapper` (the install's `PropertyOp` mapper), falling
    /// back to the observed keys (§5.3).
    #[must_use]
    pub fn resolve(key: u32, mapper: Option<&EnumMapper>) -> Option<Self> {
        mapper
            .and_then(|m| m.name(key))
            .and_then(Self::from_label)
            .or_else(|| Self::from_observed_key(key))
    }
}

/// A level range of a modifier entry (`combat-data.md` §5.5). A missing end (absent or −1)
/// is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevelRange {
    /// `Mod_ProgressionFloor`, if given.
    pub floor: Option<i32>,
    /// `Mod_ProgressionCeiling`, if given.
    pub ceiling: Option<i32>,
}

impl LevelRange {
    /// Whether either end is given.
    #[must_use]
    pub fn is_ranged(&self) -> bool {
        self.floor.is_some() || self.ceiling.is_some()
    }

    /// Whether `level` lies in the range, both ends inclusive.
    #[must_use]
    pub fn contains(&self, level: i64) -> bool {
        self.floor.is_none_or(|f| level >= i64::from(f))
            && self.ceiling.is_none_or(|c| level <= i64::from(c))
    }

    fn lo(&self) -> i64 {
        self.floor.map_or(i64::MIN, i64::from)
    }

    fn hi(&self) -> i64 {
        self.ceiling.map_or(i64::MAX, i64::from)
    }
}

/// Kind of a modifier entry (`combat-data.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModEntryKind {
    /// A struct with no fields (skipped).
    FieldLess,
    /// `Mod_Modified` = 0 or absent (skipped).
    Empty,
    /// The value is stored inline.
    Inline,
    /// The value comes from `Mod_Progression`.
    ProgressionDriven,
    /// Both an inline value and a progression (M1 says never).
    Conflicting,
    /// Neither (M1 says never).
    Valueless,
}

/// One `Mod_Entry` (`combat-data.md` §5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ModEntry {
    /// The entry kind.
    pub kind: ModEntryKind,
    /// `Mod_Modified`: the property changed (0 for empty entries).
    pub modified: u32,
    /// `Mod_Op` key (resolve with [`ModOp::resolve`]).
    pub op: Option<u32>,
    /// The inline value: the field whose PID is `modified`.
    pub value: Option<PropertyValue>,
    /// `Mod_Progression`, when non-zero.
    pub progression: Option<Did>,
    /// `Mod_ProgressionFloor` / `Mod_ProgressionCeiling`.
    pub range: LevelRange,
    /// `Mod_ModifierList` PIDs, zeros removed.
    pub modifier_list: Vec<u32>,
    /// `Mod_ModifierOp` key.
    pub modifier_op: Option<u32>,
    /// `Mod_DescriptionOverride`.
    pub description_override: Option<StringInfo>,
    /// Fields with PIDs §5.1 does not name and that are not the value (anomalies; §5.2
    /// says ignore them).
    pub unrelated_fields: Vec<u32>,
}

impl ModEntry {
    /// Reads one `Mod_Entry` struct.
    #[must_use]
    pub fn from_struct(s: &PropertyStruct, pids: &ModifierPids) -> Self {
        let modified = s.property_ref(pids.modified).unwrap_or(0);
        let value = if modified == 0 {
            None
        } else {
            s.get(modified).cloned()
        };
        let progression = s.did_nonnull(pids.progression);
        let known = [
            pids.modified,
            pids.op,
            pids.progression,
            pids.floor,
            pids.ceiling,
            pids.modifier_list,
            pids.modifier_op,
            pids.description_override,
            pids.description_override_progression,
            pids.next_rank_description_override,
            pids.next_rank_description_override_progression,
        ];
        let unrelated_fields = s
            .fields
            .iter()
            .map(|f| f.pid)
            .filter(|&p| (modified == 0 || p != modified) && !known.iter().any(|k| k.is(p)))
            .collect();
        let kind = if s.fields.is_empty() {
            ModEntryKind::FieldLess
        } else if modified == 0 {
            ModEntryKind::Empty
        } else {
            match (value.is_some(), progression.is_some()) {
                (true, false) => ModEntryKind::Inline,
                (false, true) => ModEntryKind::ProgressionDriven,
                (true, true) => ModEntryKind::Conflicting,
                (false, false) => ModEntryKind::Valueless,
            }
        };
        Self {
            kind,
            modified,
            op: s.enum_key(pids.op),
            value,
            progression,
            range: LevelRange {
                floor: given_i32(s.int(pids.floor)),
                ceiling: given_i32(s.int(pids.ceiling)),
            },
            modifier_list: pid_list(s, pids.modifier_list),
            modifier_op: s.enum_key(pids.modifier_op),
            description_override: s.string_info(pids.description_override).cloned(),
            unrelated_fields,
        }
    }

    /// Whether readers skip the entry (empty or field-less, §5.2).
    #[must_use]
    pub fn is_skipped(&self) -> bool {
        matches!(self.kind, ModEntryKind::Empty | ModEntryKind::FieldLess)
    }

    /// The operator, resolved by label through `mapper` with the key fallback.
    #[must_use]
    pub fn operator(&self, mapper: Option<&EnumMapper>) -> Option<ModOp> {
        ModOp::resolve(self.op?, mapper)
    }

    /// Whether the entry applies at governing level `level` (§5.6 step 2); `None` means no
    /// level is known, which passes only unranged entries.
    #[must_use]
    pub fn applies_at(&self, level: Option<i64>) -> bool {
        match level {
            Some(l) => self.range.contains(l),
            None => !self.range.is_ranged(),
        }
    }

    /// The entry's value (§5.6 steps 1 and 3): the inline value, else the progression
    /// evaluated at `index` (and `inner` for a progression of progressions). `None` for
    /// skipped entries and when the progression gives none. Level ranges and modifier lists
    /// are not applied here.
    #[must_use]
    pub fn resolve_value<S: ProgressionSource + ?Sized>(
        &self,
        source: &S,
        index: i64,
        inner: Option<i64>,
    ) -> Option<PropertyValue> {
        if self.is_skipped() {
            return None;
        }
        if let Some(v) = &self.value {
            return Some(v.clone());
        }
        evaluate_progression(source, self.progression?, index, inner)
            .map(ProgressionValue::into_property_value)
    }
}

/// The entries of the `Mod_Array` of `holder`, in stored order; non-struct elements are
/// skipped.
#[must_use]
pub fn mod_entries<A: PropertyAccess + ?Sized>(holder: &A, pids: &ModifierPids) -> Vec<ModEntry> {
    holder
        .structs(pids.array)
        .map(|s| ModEntry::from_struct(s, pids))
        .collect()
}

/// One `Mod_Array` found anywhere in a property tree ([`mod_arrays`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ModArray {
    /// The top-level property that holds it (`Mod_Array` itself at top level).
    pub top: u32,
    /// Nesting depth: 0 at top level.
    pub depth: usize,
    /// Its entries, in stored order.
    pub entries: Vec<ModEntry>,
}

/// Every `Mod_Array` value of `set`, at any depth, depth first in stored order
/// (`combat-data.md` §5.2 counts them all; items also nest them in structs). The entries of
/// a `Mod_Array` are not searched for further arrays. Depth is bounded by the decoder's
/// nesting limit.
#[must_use]
pub fn mod_arrays(set: &PropertySet, pids: &ModifierPids) -> Vec<ModArray> {
    fn visit(
        e: &PropertyEntry,
        top: u32,
        depth: usize,
        pids: &ModifierPids,
        out: &mut Vec<ModArray>,
    ) {
        if pids.array.is(e.pid)
            && let PropertyValue::Array(a) = &e.value
        {
            out.push(ModArray {
                top,
                depth,
                entries: a
                    .elements
                    .iter()
                    .filter_map(|x| x.value.as_struct())
                    .map(|s| ModEntry::from_struct(s, pids))
                    .collect(),
            });
            return;
        }
        for c in e.value.children() {
            visit(c, top, depth + 1, pids, out);
        }
    }
    let mut out = Vec::new();
    for e in &set.entries {
        visit(e, e.pid, 0, pids, &mut out);
    }
    out
}

/// A ranged group (`combat-data.md` §5.5): the non-empty entries of one array that share
/// `Mod_Modified`, at least one of them ranged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangedGroup {
    /// The modified property.
    pub modified: u32,
    /// (index into the entry list, range), sorted by floor (stable; open floors first).
    pub members: Vec<(usize, LevelRange)>,
    /// Number of unranged entries for the same property (M3 says 0).
    pub unranged: usize,
    /// Whether the stored order was already ascending by floor.
    pub stored_ascending: bool,
}

impl RangedGroup {
    /// Whether the ranges are pairwise disjoint (M3).
    #[must_use]
    pub fn is_disjoint(&self) -> bool {
        self.members.windows(2).all(|w| match w {
            [(_, a), (_, b)] => a.hi() < b.lo(),
            _ => true,
        })
    }

    /// Whether consecutive ranges touch once sorted (ceiling + 1 = next floor).
    #[must_use]
    pub fn is_contiguous(&self) -> bool {
        self.members.windows(2).all(|w| match w {
            [(_, a), (_, b)] => a.hi().checked_add(1) == Some(b.lo()),
            _ => true,
        })
    }

    /// The entry index that applies at `level`, if any (at most one when disjoint).
    #[must_use]
    pub fn select(&self, level: i64) -> Option<usize> {
        self.members
            .iter()
            .find(|(_, r)| r.contains(level))
            .map(|&(i, _)| i)
    }
}

/// The ranged groups of `entries` (one array's entries), by first occurrence of the
/// modified property.
#[must_use]
pub fn ranged_groups(entries: &[ModEntry]) -> Vec<RangedGroup> {
    let mut order: Vec<u32> = Vec::new();
    let mut by_pid: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.is_skipped() {
            continue;
        }
        let slot = by_pid.entry(e.modified).or_default();
        if slot.is_empty() {
            order.push(e.modified);
        }
        slot.push(i);
    }
    let mut out = Vec::new();
    for pid in order {
        let Some(idx) = by_pid.get(&pid) else {
            continue;
        };
        let mut members: Vec<(usize, LevelRange)> = idx
            .iter()
            .filter_map(|&i| entries.get(i).map(|e| (i, e.range)))
            .filter(|(_, r)| r.is_ranged())
            .collect();
        if members.is_empty() {
            continue;
        }
        let unranged = idx.len() - members.len();
        let stored_ascending = members.windows(2).all(|w| match w {
            [(_, a), (_, b)] => a.lo() <= b.lo(),
            _ => true,
        });
        members.sort_by_key(|(_, r)| r.lo());
        out.push(RangedGroup {
            modified: pid,
            members,
            unranged,
            stored_ascending,
        });
    }
    out
}

/// Combines modifier values into a property value in **Tessera's sandbox order**
/// (**Decision D2**, not a fact about the original, whose order is OPEN, §5.3 / Q6):
/// start from `base`; `Set` entries (last wins); then `Add` and `Subtract` in the given
/// order; then `Multiply`; then the bitwise operators; then `ArrayAdd`.
///
/// Arithmetic needs the value's type to equal the base's (§5.2 M2); a mismatching value is
/// skipped and logged. Integers saturate. Operators with no Reported meaning (`Invalid`,
/// `Difference`, `Divide`, `Random`) are skipped and logged.
#[must_use]
pub fn sandbox_combine(base: &PropertyValue, applied: &[(ModOp, PropertyValue)]) -> PropertyValue {
    let mut out = applied
        .iter()
        .rev()
        .find(|(op, _)| *op == ModOp::Set)
        .map_or_else(|| base.clone(), |(_, v)| v.clone());
    let phases: [&[ModOp]; 4] = [
        &[ModOp::Add, ModOp::Subtract],
        &[ModOp::Multiply],
        &[ModOp::Or, ModOp::And, ModOp::Xor],
        &[ModOp::ArrayAdd],
    ];
    for phase in phases {
        for (op, v) in applied.iter().filter(|(op, _)| phase.contains(op)) {
            match apply_op(&out, *op, v) {
                Some(next) => out = next,
                None => tracing::debug!(?op, "modifier value type does not fit; skipped"),
            }
        }
    }
    for (op, _) in applied {
        if matches!(
            op,
            ModOp::Invalid | ModOp::Difference | ModOp::Divide | ModOp::Random
        ) {
            tracing::debug!(?op, "operator without a sandbox meaning; skipped");
        }
    }
    out
}

fn apply_op(cur: &PropertyValue, op: ModOp, v: &PropertyValue) -> Option<PropertyValue> {
    use PropertyValue as V;
    Some(match (op, cur, v) {
        (ModOp::Add, V::Float(a), V::Float(b)) => V::Float(a + b),
        (ModOp::Subtract, V::Float(a), V::Float(b)) => V::Float(a - b),
        (ModOp::Multiply, V::Float(a), V::Float(b)) => V::Float(a * b),
        (ModOp::Add, V::Int(a), V::Int(b)) => V::Int(a.saturating_add(*b)),
        (ModOp::Subtract, V::Int(a), V::Int(b)) => V::Int(a.saturating_sub(*b)),
        (ModOp::Multiply, V::Int(a), V::Int(b)) => V::Int(a.saturating_mul(*b)),
        (ModOp::Add, V::Int64(a), V::Int64(b)) => V::Int64(a.saturating_add(*b)),
        (ModOp::Subtract, V::Int64(a), V::Int64(b)) => V::Int64(a.saturating_sub(*b)),
        (ModOp::Multiply, V::Int64(a), V::Int64(b)) => V::Int64(a.saturating_mul(*b)),
        (ModOp::Or, V::Bool(a), V::Bool(b)) => V::Bool(*a || *b),
        (ModOp::And, V::Bool(a), V::Bool(b)) => V::Bool(*a && *b),
        (ModOp::Xor, V::Bool(a), V::Bool(b)) => V::Bool(a ^ b),
        (ModOp::Or, V::Bitfield32(a), V::Bitfield32(b)) => V::Bitfield32(a | b),
        (ModOp::And, V::Bitfield32(a), V::Bitfield32(b)) => V::Bitfield32(a & b),
        (ModOp::Xor, V::Bitfield32(a), V::Bitfield32(b)) => V::Bitfield32(a ^ b),
        (ModOp::Or, V::Bitfield64(a), V::Bitfield64(b)) => V::Bitfield64(a | b),
        (ModOp::And, V::Bitfield64(a), V::Bitfield64(b)) => V::Bitfield64(a & b),
        (ModOp::Xor, V::Bitfield64(a), V::Bitfield64(b)) => V::Bitfield64(a ^ b),
        (ModOp::Or | ModOp::And | ModOp::Xor, V::BitfieldAny(a), V::BitfieldAny(b)) => {
            let mut out = a.clone();
            for (i, byte) in out.bytes.iter_mut().enumerate() {
                let other = b.bytes.get(i).copied().unwrap_or(0);
                *byte = match op {
                    ModOp::Or => *byte | other,
                    ModOp::And => *byte & other,
                    _ => *byte ^ other,
                };
            }
            V::BitfieldAny(out)
        }
        (ModOp::ArrayAdd, V::Array(a), V::Array(b)) => {
            let mut out = a.clone();
            out.elements.extend(b.elements.iter().cloned());
            out.count = out.count.saturating_add(b.count);
            V::Array(out)
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Effects (§6)
// ---------------------------------------------------------------------------------------------

/// Kind of an effect reference (`combat-data.md` §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectRefKind {
    /// A struct with `EffectGenerator_EffectID` (`EffectGenerator_EffectStruct` and the
    /// other structs §6.3 lists).
    Generator,
    /// A struct with `Skill_Effect` (`Skill_EffectData`).
    SkillEffectData,
    /// A struct with `Skill_Toggle_Effect`.
    Toggle,
    /// A struct with `Effect_StartupEffectID`.
    Startup,
    /// A DID entry outside such a struct: `Effect_WSLEffect` or a bare `Skill_Effect` (and
    /// the other ID labels when they occur alone).
    Bare,
}

/// One effect reference (`combat-data.md` §6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectReference {
    /// How the reference is stored.
    pub kind: EffectRefKind,
    /// The top-level property that holds it (the list it belongs to).
    pub top: u32,
    /// PID of the struct carrying the fields, or of the entry itself for bare references.
    pub container: u32,
    /// The effect (null for a placeholder, §3.4).
    pub effect: Did,
    /// The stored spellcraft (−1 = not given; see [`EffectReference::given_spellcraft`]).
    pub spellcraft: Option<f32>,
    /// `Skill_EffectDuration` (−1 = the effect's own).
    pub duration: Option<f32>,
    /// `Skill_EffectImplementUsage` enum key.
    pub implement_usage: Option<u32>,
    /// `Effect_StartupEffectDamageQualifier` enum key.
    pub damage_qualifier: Option<u32>,
}

impl EffectReference {
    /// Whether this is a placeholder (effect DID 0).
    #[must_use]
    pub fn is_placeholder(&self) -> bool {
        self.effect.is_null()
    }

    /// The spellcraft, `None` when absent or −1 ("not given").
    #[must_use]
    pub fn given_spellcraft(&self) -> Option<f32> {
        given_f32(self.spellcraft)
    }

    /// The duration override, `None` when absent or −1 (the effect's own).
    #[must_use]
    pub fn given_duration(&self) -> Option<f32> {
        given_f32(self.duration)
    }
}

/// Every effect reference in `set`, depth first in stored order (`combat-data.md` §6.3).
/// Structs are recognised by their ID field, so the same struct kinds are found in any
/// holder; references nested inside a recognised struct (for example in its data list) are
/// found too. Placeholders are included; filter with [`EffectReference::is_placeholder`].
#[must_use]
pub fn effect_references(set: &PropertySet, pids: &EffectRefPids) -> Vec<EffectReference> {
    let mut out = Vec::new();
    for e in &set.entries {
        visit_effect_refs(e, e.pid, pids, &mut out);
    }
    out
}

fn visit_effect_refs(
    e: &PropertyEntry,
    top: u32,
    pids: &EffectRefPids,
    out: &mut Vec<EffectReference>,
) {
    match &e.value {
        PropertyValue::Struct(s) => {
            let id_fields = [
                (pids.generator_id, EffectRefKind::Generator),
                (pids.skill_effect, EffectRefKind::SkillEffectData),
                (pids.toggle_effect, EffectRefKind::Toggle),
                (pids.startup_id, EffectRefKind::Startup),
            ];
            let found = id_fields
                .iter()
                .find_map(|&(p, k)| s.did(p).map(|d| (p, k, d)));
            if let Some((id_pid, kind, effect)) = found {
                let spellcraft = match kind {
                    EffectRefKind::Generator => s.float(pids.generator_spellcraft),
                    EffectRefKind::SkillEffectData => s.float(pids.skill_spellcraft),
                    EffectRefKind::Toggle => s.float(pids.toggle_spellcraft),
                    EffectRefKind::Startup => s.float(pids.startup_spellcraft),
                    EffectRefKind::Bare => None,
                };
                out.push(EffectReference {
                    kind,
                    top,
                    container: e.pid,
                    effect,
                    spellcraft,
                    duration: s.float(pids.skill_duration),
                    implement_usage: s.enum_key(pids.skill_implement_usage),
                    damage_qualifier: s.enum_key(pids.startup_damage_qualifier),
                });
                for f in s.fields.iter().filter(|f| !id_pid.is(f.pid)) {
                    visit_effect_refs(f, top, pids, out);
                }
            } else {
                for f in &s.fields {
                    visit_effect_refs(f, top, pids, out);
                }
            }
        }
        PropertyValue::Array(a) => {
            for c in &a.elements {
                visit_effect_refs(c, top, pids, out);
            }
        }
        PropertyValue::DidRef(d) => {
            let bare = [
                pids.bare_effect,
                pids.skill_effect,
                pids.generator_id,
                pids.startup_id,
                pids.toggle_effect,
            ];
            if bare.iter().any(|p| p.is(e.pid)) {
                out.push(EffectReference {
                    kind: EffectRefKind::Bare,
                    top,
                    container: e.pid,
                    effect: *d,
                    spellcraft: None,
                    duration: None,
                    implement_usage: None,
                    damage_qualifier: None,
                });
            }
        }
        _ => {}
    }
}

/// Duration group of an effect (`combat-data.md` §6.2), taken by stored value: interval and
/// pulse count by presence, permanent by a stored true, instantaneous by a stored true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DurationGroup {
    /// No duration property.
    Unspecified,
    /// `Effect_Instantaneous` true.
    Instantaneous,
    /// Permanent only.
    Permanent,
    /// Interval only.
    Interval,
    /// Interval and pulse count.
    Pulsed,
    /// Interval and permanent.
    IntervalPermanent,
    /// A combination no record of the observed install has (E1 says never).
    Other,
}

/// The duration properties of an effect (`combat-data.md` §6.2). Their semantics are OPEN
/// (Q7).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EffectDuration {
    /// `Effect_Duration_ConstantInterval` (seconds).
    pub interval: Option<f32>,
    /// `Effect_Duration_ConstantPulseCount`.
    pub pulse_count: Option<i64>,
    /// `Effect_Duration_Permanent` as stored (E2: always true when present).
    pub permanent: Option<bool>,
    /// `Effect_Instantaneous` as stored (a stored false means "not instantaneous").
    pub instantaneous: Option<bool>,
    /// `Effect_Duration_ProgressionPulseCount`, when non-zero.
    pub pulse_count_progression: Option<Did>,
    /// `Effect_Duration_ProgressionInterval`, when non-zero.
    pub interval_progression: Option<Did>,
    /// `Effect_Duration_ExpiresInRealTime`.
    pub expires_in_real_time: Option<bool>,
    /// `Effect_Duration_CombatOnly`.
    pub combat_only: Option<bool>,
}

impl EffectDuration {
    /// The duration group (E1).
    #[must_use]
    pub fn group(&self) -> DurationGroup {
        let interval = self.interval.is_some();
        let pulses = self.pulse_count.is_some();
        let permanent = self.permanent == Some(true);
        let instant = self.instantaneous == Some(true);
        match (interval, pulses, permanent, instant) {
            (false, false, false, false) => DurationGroup::Unspecified,
            (false, false, false, true) => DurationGroup::Instantaneous,
            (false, false, true, false) => DurationGroup::Permanent,
            (true, false, false, false) => DurationGroup::Interval,
            (true, true, false, false) => DurationGroup::Pulsed,
            (true, false, true, false) => DurationGroup::IntervalPermanent,
            _ => DurationGroup::Other,
        }
    }
}

/// A uniform random amount (`*Random` struct, §6.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RandomAmount {
    /// `Effect_RandomValueMin`.
    pub min: Option<f32>,
    /// `Effect_RandomValueMax`.
    pub max: Option<f32>,
}

/// An amount interpolated by spellcraft (`*Variable` struct, §6.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariableAmount {
    /// `Effect_VariableMin`.
    pub min: Option<f32>,
    /// `Effect_VariableMax`.
    pub max: Option<f32>,
    /// `Effect_VariableSpellcraftMin`.
    pub spellcraft_min: Option<f32>,
    /// `Effect_VariableSpellcraftMax`.
    pub spellcraft_max: Option<f32>,
}

/// A vital-change amount (`combat-data.md` §6.4). Which source wins when several are
/// present is OPEN (Q8), so all are kept.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VitalAmount {
    /// *S*`Constant`.
    pub constant: Option<f32>,
    /// *S*`Progression`, when non-zero.
    pub progression: Option<Did>,
    /// *S*`Random`.
    pub random: Option<RandomAmount>,
    /// *S*`Variable`.
    pub variable: Option<VariableAmount>,
    /// *S*`Variance`.
    pub variance: Option<f32>,
}

impl VitalAmount {
    fn read<A: PropertyAccess + ?Sized>(set: &A, stem: [Pid; 5], e: &EffectPids) -> Option<Self> {
        let [constant, progression, random, variable, variance] = stem;
        if !stem.iter().any(|&p| set.has(p)) {
            return None;
        }
        Some(Self {
            constant: set.float(constant),
            progression: set.did_nonnull(progression),
            random: set.structure(random).map(|s| RandomAmount {
                min: s.float(e.random_min),
                max: s.float(e.random_max),
            }),
            variable: set.structure(variable).map(|s| VariableAmount {
                min: s.float(e.variable_min),
                max: s.float(e.variable_max),
                spellcraft_min: s.float(e.variable_spellcraft_min),
                spellcraft_max: s.float(e.variable_spellcraft_max),
            }),
            variance: set.float(variance),
        })
    }

    /// Whether the amount has a constant and a progression (the case Q8 leaves open).
    #[must_use]
    pub fn has_constant_and_progression(&self) -> bool {
        self.constant.is_some() && self.progression.is_some()
    }
}

/// Aura data (`combat-data.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub struct AuraData {
    pub aura_type: Option<u32>,
    pub radius: Option<f32>,
    pub spellcraft_override: Option<f32>,
    pub affects_caster: Option<bool>,
}

/// Area data (`combat-data.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub struct AreaData {
    pub max_range: Option<f32>,
    pub max_targets: Option<i64>,
}

/// Skill-proc data (`combat-data.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub struct SkillProcData {
    pub skill_types: Option<u64>,
    pub required_result: Option<u64>,
    pub probability: Option<f32>,
    pub min_interval: Option<f32>,
}

/// Absorb-bubble data (`combat-data.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub struct BubbleData {
    pub vital_type: Option<u32>,
    pub value: Option<f32>,
    pub value_progression: Option<Did>,
    pub percentage: Option<f32>,
}

/// Pip data (`combat-data.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub struct PipData {
    pub pip_type: Option<u32>,
    pub amount: Option<f64>,
    pub adjustment_type: Option<u32>,
    pub reset: Option<bool>,
}

/// An effect (`combat-data.md` §6), from its property half.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// The record.
    pub did: Did,
    /// Root class.
    pub class: u32,
    /// `Effect_Name`.
    pub name: StringInfo,
    /// `Effect_Definition_Description`.
    pub definition_description: StringInfo,
    /// `Effect_Applied_Description`.
    pub applied_description: StringInfo,
    /// `Effect_Icon`, when non-zero.
    pub icon: Option<Did>,
    /// `Effect_Harmful`.
    pub harmful: Option<bool>,
    /// `Effect_Debuff`.
    pub debuff: Option<bool>,
    /// `Effect_ConstantApplicationProbability` (Reported: absent ⇒ 1, see
    /// [`Effect::application_probability_or_default`]).
    pub application_probability: Option<f32>,
    /// `Effect_ApplicationProbabilityVariance`.
    pub probability_variance: Option<f32>,
    /// `Effect_ApplicationProbability_AdditiveModProp`, when non-zero.
    pub probability_additive_property: Option<u32>,
    /// `Effect_ApplicationProbabilityProgression`, when non-zero.
    pub probability_progression: Option<Did>,
    /// `Effect_Resist_Category` bits.
    pub resist_category: Option<u64>,
    /// `Effect_Resist_Level`.
    pub resist_level: Option<i64>,
    /// Duration properties.
    pub duration: EffectDuration,
    /// `Effect_Duration_ConstantInterval_ModifierList` PIDs.
    pub interval_modifiers: Vec<u32>,
    /// `Effect_EquivalenceClass` key.
    pub equivalence_class: Option<u32>,
    /// `Effect_ClassPriority`.
    pub class_priority: Option<i64>,
    /// `Effect_Cap`.
    pub cap: Option<i64>,
    /// `Effect_Critical_Chance`.
    pub critical_chance: Option<f32>,
    /// `Mod_Array` entries (§5).
    pub modifiers: Vec<ModEntry>,
    /// Instant vital amount (`Effect_InstantVital_InitialChange*`).
    pub instant_vital: Option<VitalAmount>,
    /// Initial amount of a vital change over time.
    pub over_time_initial: Option<VitalAmount>,
    /// Per-interval amount of a vital change over time.
    pub over_time_per_interval: Option<VitalAmount>,
    /// `Effect_BaseVital_VitalType` or `Effect_VitalOverTime_VitalType`.
    pub vital_type: Option<u32>,
    /// `Effect_DamageType` key.
    pub damage_type: Option<u32>,
    /// `Effect_InstantVital_Multiplicative`.
    pub multiplicative: Option<bool>,
    /// Aura data, when `Effect_Aura_Type` or `Effect_Aura_Radius` is present.
    pub aura: Option<AuraData>,
    /// Area data, when `Effect_Area_MaxRange` or `_MaxTargets` is present.
    pub area: Option<AreaData>,
    /// Skill-proc data, when any of its properties is present.
    pub skill_proc: Option<SkillProcData>,
    /// Absorb-bubble data, when `Effect_Bubble_VitalType` is present.
    pub bubble: Option<BubbleData>,
    /// Pip data, when `Effect_Pip_Type` is present.
    pub pips: Option<PipData>,
    /// Every effect reference of the record (§6.3).
    pub references: Vec<EffectReference>,
}

impl Effect {
    /// Reads an effect. The three text properties of the family signature are required.
    pub fn read(record: &GameObjectRecord, pids: &CombatPids) -> Result<Self> {
        const WHAT: &str = "effect";
        let set = &record.properties;
        let e = &pids.effect;
        let v = &pids.vital;
        let x = &pids.effect_extra;
        let text = |p: Pid| {
            set.require_as(
                p,
                WHAT,
                crate::property::PropertyType::StringInfo,
                ValueExt::string_info,
            )
            .cloned()
        };
        let any = |ps: &[Pid]| ps.iter().any(|&p| set.has(p));
        Ok(Self {
            did: record.did(),
            class: record.root_class(),
            name: text(e.name)?,
            definition_description: text(e.definition_description)?,
            applied_description: text(e.applied_description)?,
            icon: set.did_nonnull(e.icon),
            harmful: set.boolean(e.harmful),
            debuff: set.boolean(e.debuff),
            application_probability: set.float(e.application_probability),
            probability_variance: set.float(e.probability_variance),
            probability_additive_property: nonzero_pid(
                set.property_ref(e.probability_additive_property),
            ),
            probability_progression: set.did_nonnull(e.probability_progression),
            resist_category: set
                .get_pid(e.resist_category)
                .and_then(PropertyValue::as_bits),
            resist_level: set.integer(e.resist_level),
            duration: EffectDuration {
                interval: set.float(e.duration_interval),
                pulse_count: set.integer(e.duration_pulse_count),
                permanent: set.boolean(e.duration_permanent),
                instantaneous: set.boolean(e.instantaneous),
                pulse_count_progression: set.did_nonnull(e.duration_progression_pulse_count),
                interval_progression: set.did_nonnull(e.duration_progression_interval),
                expires_in_real_time: set.boolean(e.expires_in_real_time),
                combat_only: set.boolean(e.combat_only),
            },
            interval_modifiers: pid_list(set, e.interval_modifier_list),
            equivalence_class: set.enum_key(e.equivalence_class),
            class_priority: set.integer(e.class_priority),
            cap: set.integer(e.cap),
            critical_chance: set.float(e.critical_chance),
            modifiers: mod_entries(set, &pids.modifier),
            instant_vital: VitalAmount::read(
                set,
                [
                    v.instant_constant,
                    v.instant_progression,
                    v.instant_random,
                    v.instant_variable,
                    v.instant_variance,
                ],
                e,
            ),
            over_time_initial: VitalAmount::read(
                set,
                [
                    v.initial_constant,
                    v.initial_progression,
                    v.initial_random,
                    v.initial_variable,
                    v.initial_variance,
                ],
                e,
            ),
            over_time_per_interval: VitalAmount::read(
                set,
                [
                    v.per_interval_constant,
                    v.per_interval_progression,
                    v.per_interval_random,
                    v.per_interval_variable,
                    v.per_interval_variance,
                ],
                e,
            ),
            vital_type: set
                .enum_key(e.base_vital_type)
                .or_else(|| set.enum_key(e.over_time_vital_type)),
            damage_type: set.enum_key(e.damage_type),
            multiplicative: set.boolean(e.multiplicative),
            aura: any(&[x.aura_type, x.aura_radius]).then(|| AuraData {
                aura_type: set.enum_key(x.aura_type),
                radius: set.float(x.aura_radius),
                spellcraft_override: set.float(x.aura_spellcraft_override),
                affects_caster: set.boolean(x.aura_affects_caster),
            }),
            area: any(&[x.area_max_range, x.area_max_targets]).then(|| AreaData {
                max_range: set.float(x.area_max_range),
                max_targets: set.integer(x.area_max_targets),
            }),
            skill_proc: any(&[
                x.proc_skill_types,
                x.proc_required_result,
                x.proc_probability,
                x.proc_min_interval,
            ])
            .then(|| SkillProcData {
                skill_types: set
                    .get_pid(x.proc_skill_types)
                    .and_then(PropertyValue::as_bits),
                required_result: set
                    .get_pid(x.proc_required_result)
                    .and_then(PropertyValue::as_bits),
                probability: set.float(x.proc_probability),
                min_interval: set.float(x.proc_min_interval),
            }),
            bubble: set.has(x.bubble_vital_type).then(|| BubbleData {
                vital_type: set.enum_key(x.bubble_vital_type),
                value: set.float(x.bubble_value),
                value_progression: set.did_nonnull(x.bubble_value_progression),
                percentage: set.float(x.bubble_percentage),
            }),
            pips: set.has(x.pip_type).then(|| PipData {
                pip_type: set.enum_key(x.pip_type),
                amount: set.get_pid(x.pip_amount).and_then(PropertyValue::as_f64),
                adjustment_type: set.enum_key(x.pip_adjustment_type),
                reset: set.boolean(x.pip_reset),
            }),
            references: effect_references(set, &pids.effect_ref),
        })
    }

    /// The application probability, 1 when absent (Reported, §6.2).
    #[must_use]
    pub fn application_probability_or_default(&self) -> f32 {
        self.application_probability.unwrap_or(1.0)
    }
}

/// One `Effect_EqvClassData_Struct` (`combat-data.md` §6.6). The stacking rule is OPEN
/// (Q10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct EquivalenceClassData {
    pub class: u32,
    pub max_depth: Option<i64>,
    pub max_expressed: Option<i64>,
    pub per_caster: Option<bool>,
    pub sorter_override: Option<Did>,
    pub duration_modifier_source: Option<u32>,
    pub duration_modifier_target: Option<u32>,
}

/// The effect control object (`combat-data.md` §6.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectControl {
    /// `Effect_EqvClassData_Array` structs that name a class.
    pub classes: Vec<EquivalenceClassData>,
    /// `Effect_Control_DefaultSorter`.
    pub default_sorter: Option<Did>,
    /// `Effect_Control_SpellcraftCapProgression`.
    pub spellcraft_cap_progression: Option<Did>,
}

impl EffectControl {
    /// Reads the effect control object's property set.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, pids: &CombatPids) -> Self {
        let x = &pids.effect_extra;
        let classes = set
            .structs(x.eqv_array)
            .filter_map(|s| {
                Some(EquivalenceClassData {
                    class: s.enum_key(pids.effect.equivalence_class)?,
                    max_depth: s.integer(x.eqv_max_depth),
                    max_expressed: s.integer(x.eqv_max_expressed),
                    per_caster: s.boolean(x.eqv_per_caster),
                    sorter_override: s.did_nonnull(x.eqv_sorter),
                    duration_modifier_source: nonzero_pid(s.property_ref(x.eqv_duration_source)),
                    duration_modifier_target: nonzero_pid(s.property_ref(x.eqv_duration_target)),
                })
            })
            .collect();
        Self {
            classes,
            default_sorter: set.did_nonnull(x.default_sorter),
            spellcraft_cap_progression: set.did_nonnull(x.spellcraft_cap_progression),
        }
    }

    /// The data for equivalence class `class`.
    #[must_use]
    pub fn class(&self, class: u32) -> Option<&EquivalenceClassData> {
        self.classes.iter().find(|c| c.class == class)
    }
}

/// Label of the COMBATEFFECTS DID map in the root DID map (`combat-data.md` §6.7).
pub const COMBAT_EFFECTS_MAP_LABEL: &str = "COMBATEFFECTS";

/// Label of the BATTLESTATS DID map in the root DID map (`combat-data.md` §10.4).
pub const BATTLE_STATS_MAP_LABEL: &str = "BATTLESTATS";

// ---------------------------------------------------------------------------------------------
// Skills (§7)
// ---------------------------------------------------------------------------------------------

/// One `Skill_VitalInfo` (`combat-data.md` §7.3).
#[derive(Debug, Clone, PartialEq)]
pub struct VitalCost {
    /// `Skill_Vital_Type` key.
    pub vital_type: Option<u32>,
    /// `Skill_Vital_Points`.
    pub points: Option<f32>,
    /// `Skill_Vital_Points_Progression`, when non-zero.
    pub points_progression: Option<Did>,
    /// `Skill_Vital_Percent`.
    pub percent: Option<f32>,
    /// `Skill_Vital_Consumes_All`.
    pub consumes_all: Option<bool>,
    /// `Skill_Vital_Mod_Array` PIDs (every PID-typed element inside, zeros removed).
    pub modifiers: Vec<u32>,
}

impl VitalCost {
    fn from_struct(s: &PropertyStruct, p: &SkillPids) -> Self {
        let mut modifiers = Vec::new();
        if let Some(v) = s.get_pid(p.vital_mod_array) {
            nested_pids(v, &mut modifiers);
        }
        Self {
            vital_type: s.enum_key(p.vital_type),
            points: s.float(p.vital_points),
            points_progression: s.did_nonnull(p.vital_points_progression),
            percent: s.float(p.vital_percent),
            consumes_all: s.boolean(p.vital_consumes_all),
            modifiers,
        }
    }
}

fn nested_pids(value: &PropertyValue, out: &mut Vec<u32>) {
    match value {
        PropertyValue::PropertyId(p) if *p != 0 => out.push(*p),
        PropertyValue::Struct(_) | PropertyValue::Array(_) => {
            for c in value.children() {
                nested_pids(&c.value, out);
            }
        }
        _ => {}
    }
}

fn vital_costs<A: PropertyAccess + ?Sized>(set: &A, list: Pid, p: &SkillPids) -> Vec<VitalCost> {
    set.structs(list)
        .map(|s| VitalCost::from_struct(s, p))
        .collect()
}

/// An area detection volume (`combat-data.md` §7.4). S2: a skill has at most one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionVolume {
    /// `Skill_AEDetectionVolume_SphereRadius`.
    Sphere {
        /// Radius (world units).
        radius: f32,
    },
    /// `Skill_AEDetectionVolume_ArcRadius` and `_ArcDegrees`.
    Arc {
        /// Radius (world units).
        radius: f32,
        /// Arc (degrees).
        degrees: Option<f32>,
    },
    /// `Skill_AEDetectionVolume_BoxLength` and `_BoxWidth`.
    Box {
        /// Length (world units).
        length: f32,
        /// Width (world units).
        width: Option<f32>,
    },
}

/// One `Skill_AttackHookInfo` (`combat-data.md` §7.7).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct AttackHook {
    pub animation_hook_index: Option<i64>,
    pub damage_max: Option<f32>,
    pub damage_max_progression: Option<Did>,
    pub damage_max_variance: Option<f32>,
    pub damage_modifier: Option<f32>,
    pub dps_add_progression: Option<Did>,
    pub damage_add_multiplier: Option<f32>,
    pub implement_multiplier: Option<f32>,
    pub damage_type: Option<u32>,
    pub damage_type_override: Option<u32>,
    pub damage_qualifier: Option<u32>,
    pub critical_chance: Option<f32>,
    pub critical_multiplier: Option<f32>,
    pub super_critical_multiplier: Option<f32>,
    pub positional_multiplier: Option<f32>,
    pub mitigation_modifier: Option<f32>,
    pub threat_value: Option<f32>,
    pub threat_type: Option<u32>,
    pub uses_primary: Option<bool>,
    pub uses_secondary: Option<bool>,
    pub uses_ranged: Option<bool>,
    pub uses_natural: Option<bool>,
    pub uses_tactical: Option<bool>,
    pub can_critical: Option<bool>,
}

impl AttackHook {
    fn from_struct(s: &PropertyStruct, h: &AttackHookPids) -> Self {
        Self {
            animation_hook_index: s.integer(h.animation_hook_index),
            damage_max: s.float(h.damage_max),
            damage_max_progression: s.did_nonnull(h.damage_max_progression),
            damage_max_variance: s.float(h.damage_max_variance),
            damage_modifier: s.float(h.damage_modifier),
            dps_add_progression: s.did_nonnull(h.dps_add_progression),
            damage_add_multiplier: s.float(h.damage_add_multiplier),
            implement_multiplier: s.float(h.implement_multiplier),
            damage_type: s.enum_key(h.damage_type),
            damage_type_override: nonzero_pid(s.property_ref(h.damage_type_override)),
            damage_qualifier: s.enum_key(h.damage_qualifier),
            critical_chance: s.float(h.critical_chance),
            critical_multiplier: s.float(h.critical_multiplier),
            super_critical_multiplier: s.float(h.super_critical_multiplier),
            positional_multiplier: s.float(h.positional_multiplier),
            mitigation_modifier: s.float(h.mitigation_modifier),
            threat_value: s.float(h.threat_value),
            threat_type: s.enum_key(h.threat_type),
            uses_primary: s.boolean(h.uses_primary),
            uses_secondary: s.boolean(h.uses_secondary),
            uses_ranged: s.boolean(h.uses_ranged),
            uses_natural: s.boolean(h.uses_natural),
            uses_tactical: s.boolean(h.uses_tactical),
            can_critical: s.boolean(h.can_critical),
        }
    }
}

/// Hit-rule switches and modifiers of a skill (`combat-data.md` §7.6).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(missing_docs)]
pub struct HitRules {
    pub can_miss: Option<bool>,
    pub miss_chance_modifier: Option<f32>,
    pub can_be_evaded: Option<bool>,
    pub evade_chance_modifier: Option<f32>,
    pub can_be_parried: Option<bool>,
    pub parry_chance_modifier: Option<f32>,
    pub can_be_blocked: Option<bool>,
    pub block_chance_modifier: Option<f32>,
    pub resist_category: Option<u64>,
    pub requires_line_of_sight: Option<bool>,
    pub requires_area_line_of_sight: Option<bool>,
}

/// Timing of a skill (`combat-data.md` §7.9).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(missing_docs)]
pub struct SkillTiming {
    pub action_duration: Option<f32>,
    pub recovery_time: Option<f32>,
    pub recovery_channel: Option<u32>,
    pub ignores_reset_time: Option<bool>,
    pub immediate: Option<bool>,
    /// `Skill_InductionAction` (an induction object, [`Induction`]).
    pub induction: Option<Did>,
    /// `Skill_Toggle_ChannelingState` (a channelling object, [`Channelling`]).
    pub channelling_state: Option<Did>,
    pub channelling_duration_override: Option<f32>,
    /// `Skill_Toggle_Hook_Number` (Reported: marks a toggle skill).
    pub toggle_hook_number: Option<i64>,
}

/// A skill (`combat-data.md` §7), from its property half.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    /// The record.
    pub did: Did,
    /// `Skill_Name`.
    pub name: StringInfo,
    /// `Skill_Desc`.
    pub desc: Option<StringInfo>,
    /// `Skill_Icon`, `Skill_LargeIcon`, `Skill_SmallIcon` (non-zero).
    pub icons: [Option<Did>; 3],
    /// `Skill_SkillType` bits.
    pub skill_type: u64,
    /// `Skill_Category` key.
    pub category: Option<u32>,
    /// `Skill_Harmful`.
    pub harmful: Option<bool>,
    /// `Skill_IsMonsterSkill`.
    pub is_monster_skill: Option<bool>,
    /// `Skill_TargetSelectionMode` key.
    pub target_selection_mode: u32,
    /// Valid target kinds: player, pet, NPC, monster.
    pub valid_targets: [Option<bool>; 4],
    /// `Skill_VitalCostList`.
    pub vital_costs: Vec<VitalCost>,
    /// `Skill_Toggle_VitalCostPerSecondList`.
    pub toggle_costs: Vec<VitalCost>,
    /// `Skill_MinRange` (−1 = none).
    pub min_range: Option<f32>,
    /// `Skill_MaxRange` (−1 = none).
    pub max_range: Option<f32>,
    /// `Skill_AreaEffectMaxTargets` (−1 = none).
    pub area_max_targets: Option<i64>,
    /// `Skill_AreaEffectDetectionAnchor` key.
    pub area_anchor: Option<u32>,
    /// Detection volumes present (S2: at most one).
    pub detection_volumes: Vec<DetectionVolume>,
    /// `Skill_AEDetectionVolume_HeadingOffset` (degrees).
    pub heading_offset: Option<f32>,
    /// Hit rules.
    pub hit: HitRules,
    /// `Skill_AttackHookList`.
    pub attack_hooks: Vec<AttackHook>,
    /// Timing.
    pub timing: SkillTiming,
    /// `Skill_RequiredTrait`, when non-zero.
    pub required_trait: Option<Did>,
    /// Every effect reference of the record (§6.3, §7.8), placeholders included.
    pub references: Vec<EffectReference>,
}

impl Skill {
    /// Reads a skill. `Skill_Name`, `Skill_SkillType` and `Skill_TargetSelectionMode` (the
    /// family signature) are required.
    pub fn read(record: &GameObjectRecord, pids: &CombatPids) -> Result<Self> {
        const WHAT: &str = "skill";
        let set = &record.properties;
        let p = &pids.skill;
        let name = set
            .require_as(
                p.name,
                WHAT,
                crate::property::PropertyType::StringInfo,
                ValueExt::string_info,
            )?
            .clone();
        let skill_type = set.require_as(
            p.skill_type,
            WHAT,
            crate::property::PropertyType::Bitfield64,
            PropertyValue::as_bits,
        )?;
        let target_selection_mode = set.require_enum(p.target_selection_mode, WHAT)?;
        let mut detection_volumes = Vec::new();
        if let Some(radius) = set.float(p.sphere_radius) {
            detection_volumes.push(DetectionVolume::Sphere { radius });
        }
        if let Some(radius) = set.float(p.arc_radius) {
            detection_volumes.push(DetectionVolume::Arc {
                radius,
                degrees: set.float(p.arc_degrees),
            });
        }
        if let Some(length) = set.float(p.box_length) {
            detection_volumes.push(DetectionVolume::Box {
                length,
                width: set.float(p.box_width),
            });
        }
        let h = &pids.hook;
        Ok(Self {
            did: record.did(),
            name,
            desc: set.string_info(p.desc).cloned(),
            icons: [
                set.did_nonnull(p.icon),
                set.did_nonnull(p.large_icon),
                set.did_nonnull(p.small_icon),
            ],
            skill_type,
            category: set.enum_key(p.category),
            harmful: set.boolean(p.harmful),
            is_monster_skill: set.boolean(p.is_monster_skill),
            target_selection_mode,
            valid_targets: [
                set.boolean(p.valid_target_player),
                set.boolean(p.valid_target_pet),
                set.boolean(p.valid_target_npc),
                set.boolean(p.valid_target_monster),
            ],
            vital_costs: vital_costs(set, p.vital_cost_list, p),
            toggle_costs: vital_costs(set, p.toggle_cost_list, p),
            min_range: given_f32(set.float(p.min_range)),
            max_range: given_f32(set.float(p.max_range)),
            area_max_targets: set.integer(p.area_max_targets).filter(|&n| n != -1),
            area_anchor: set.enum_key(p.area_anchor),
            detection_volumes,
            heading_offset: set.float(p.heading_offset),
            hit: HitRules {
                can_miss: set.boolean(p.can_miss),
                miss_chance_modifier: set.float(p.miss_chance),
                can_be_evaded: set.boolean(p.can_be_evaded),
                evade_chance_modifier: set.float(p.evade_chance),
                can_be_parried: set.boolean(p.can_be_parried),
                parry_chance_modifier: set.float(p.parry_chance),
                can_be_blocked: set.boolean(p.can_be_blocked),
                block_chance_modifier: set.float(p.block_chance),
                resist_category: set
                    .get_pid(p.resist_category)
                    .and_then(PropertyValue::as_bits),
                requires_line_of_sight: set.boolean(p.requires_line_of_sight),
                requires_area_line_of_sight: set.boolean(p.requires_area_line_of_sight),
            },
            attack_hooks: set
                .structs(h.list)
                .map(|s| AttackHook::from_struct(s, h))
                .collect(),
            timing: SkillTiming {
                action_duration: set.float(p.action_duration),
                recovery_time: set.float(p.recovery_time),
                recovery_channel: set.enum_key(p.recovery_channel),
                ignores_reset_time: set.boolean(p.ignores_reset_time),
                immediate: set.boolean(p.immediate),
                induction: set.did_nonnull(p.induction),
                channelling_state: set.did_nonnull(p.channelling_state),
                channelling_duration_override: set.float(p.channelling_duration_override),
                toggle_hook_number: set.integer(p.toggle_hook_number),
            },
            required_trait: set.did_nonnull(p.required_trait),
            references: effect_references(set, &pids.effect_ref),
        })
    }

    /// The references of top-level list `list` (for example `Skill_UserEffectList`),
    /// placeholders skipped.
    pub fn references_in(&self, list: Pid) -> impl Iterator<Item = &EffectReference> {
        self.references
            .iter()
            .filter(move |r| list.is(r.top) && !r.is_placeholder())
    }
}

/// An induction object (`combat-data.md` §7.9).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct Induction {
    /// `Induction_Duration` (seconds; Reported: the cast time).
    pub duration: f32,
    pub interruptible: Option<bool>,
    pub allowed_while_moving: Option<bool>,
    pub allowed_in_combat: Option<bool>,
    pub additive_modifiers: Vec<u32>,
    pub multiplier_modifiers: Vec<u32>,
}

impl Induction {
    /// Reads an induction object; `Induction_Duration` is required.
    pub fn from_properties<A: PropertyAccess + ?Sized>(
        set: &A,
        p: &TimingObjectPids,
    ) -> Result<Self> {
        Ok(Self {
            duration: set.require_float(p.induction_duration, "induction object")?,
            interruptible: set.boolean(p.induction_interruptible),
            allowed_while_moving: set.boolean(p.induction_allowed_while_moving),
            allowed_in_combat: set.boolean(p.induction_allowed_in_combat),
            additive_modifiers: pid_list(set, p.induction_additive_modifiers),
            multiplier_modifiers: pid_list(set, p.induction_multiplier_modifiers),
        })
    }
}

/// A channelling object (`combat-data.md` §7.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Channelling {
    /// `Channeling_Duration` (seconds).
    pub duration: f32,
    /// `Channeling_Duration_Multiplier_PropertyName`, when non-zero.
    pub multiplier_property: Option<u32>,
}

impl Channelling {
    /// Reads a channelling object; `Channeling_Duration` is required.
    pub fn from_properties<A: PropertyAccess + ?Sized>(
        set: &A,
        p: &TimingObjectPids,
    ) -> Result<Self> {
        Ok(Self {
            duration: set.require_float(p.channelling_duration, "channelling object")?,
            multiplier_property: nonzero_pid(set.property_ref(p.channelling_multiplier_property)),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Traits (§8)
// ---------------------------------------------------------------------------------------------

/// A trait (`combat-data.md` §8.1–§8.4), from its property half.
#[derive(Debug, Clone, PartialEq)]
pub struct Trait {
    /// The record.
    pub did: Did,
    /// Root class.
    pub class: u32,
    /// `Trait_Name`.
    pub name: StringInfo,
    /// `Trait_Description`.
    pub description: Option<StringInfo>,
    /// `Trait_Tooltip`.
    pub tooltip: Option<StringInfo>,
    /// `Trait_Nature` key.
    pub nature: u32,
    /// `Trait_Category` key.
    pub category: u32,
    /// `Trait_Tier`.
    pub tier: i32,
    /// `Trait_Cost_To_Slot`.
    pub cost_to_slot: i32,
    /// `Trait_Icon`, when non-zero.
    pub icon: Option<Did>,
    /// `Trait_Minimum_Level`.
    pub minimum_level: Option<i64>,
    /// `Trait_Virtue_Maximum_Rank`.
    pub maximum_rank: Option<i64>,
    /// `Trait_Virtue_Rank_PropertyName`: the actor property that stores the rank.
    pub rank_property: Option<u32>,
    /// `Trait_Rank_Overlay_Progression`, when non-zero.
    pub rank_overlay_progression: Option<Did>,
    /// `Trait_PointBasedTrait_PointCostProgression`, when non-zero.
    pub point_cost_progression: Option<Did>,
    /// `Trait_Prerequisite_CharacterLevel`.
    pub required_character_level: Option<i64>,
    /// `Mod_Array` entries (§5).
    pub modifiers: Vec<ModEntry>,
    /// Every DID inside `Trait_Skill_Array` (granted skills).
    pub skills: Vec<Did>,
    /// Every effect reference of the record (§6.3).
    pub references: Vec<EffectReference>,
}

impl Trait {
    /// Reads a trait. `Trait_Name`, `Trait_Nature`, `Trait_Category`, `Trait_Tier` and
    /// `Trait_Cost_To_Slot` (the family signature) are required.
    pub fn read(record: &GameObjectRecord, pids: &CombatPids) -> Result<Self> {
        const WHAT: &str = "trait";
        let set = &record.properties;
        let t = &pids.traits;
        Ok(Self {
            did: record.did(),
            class: record.root_class(),
            name: set
                .require_as(
                    t.name,
                    WHAT,
                    crate::property::PropertyType::StringInfo,
                    ValueExt::string_info,
                )?
                .clone(),
            description: set.string_info(t.description).cloned(),
            tooltip: set.string_info(t.tooltip).cloned(),
            nature: set.require_enum(t.nature, WHAT)?,
            category: set.require_enum(t.category, WHAT)?,
            tier: set.require_int(t.tier, WHAT)?,
            cost_to_slot: set.require_int(t.cost_to_slot, WHAT)?,
            icon: set.did_nonnull(t.icon),
            minimum_level: set.integer(t.minimum_level),
            maximum_rank: set.integer(t.maximum_rank),
            rank_property: nonzero_pid(set.property_ref(t.rank_property)),
            rank_overlay_progression: set.did_nonnull(t.rank_overlay_progression),
            point_cost_progression: set.did_nonnull(t.point_cost_progression),
            required_character_level: set.integer(t.required_character_level),
            modifiers: mod_entries(set, &pids.modifier),
            skills: dids_inside(set, t.skill_array),
            references: effect_references(set, &pids.effect_ref),
        })
    }
}

/// A trait-tree cell key (`combat-data.md` §8.5). Which of the two numbers is the row and
/// which the column, and what the special group means, are OPEN (Q11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TraitCell {
    /// Key 0.
    None,
    /// *a* × 65,536 + *b* (observed *a* 1–8, *b* 1–12); the mapper label is "a_b".
    Grid {
        /// The high number *a*.
        a: u16,
        /// The low number *b*.
        b: u16,
    },
    /// 0x10000000 + *n* (observed *n* 0–8).
    Special(u32),
    /// Any other key.
    Unknown(u32),
}

/// Offset of the special cell group (`combat-data.md` §8.5).
pub const TRAIT_CELL_SPECIAL_BASE: u32 = 0x1000_0000;

impl TraitCell {
    /// Decodes a cell key.
    #[must_use]
    pub fn from_key(key: u32) -> Self {
        if key == 0 {
            return Self::None;
        }
        if key >= TRAIT_CELL_SPECIAL_BASE {
            return Self::Special(key - TRAIT_CELL_SPECIAL_BASE);
        }
        let a = (key >> 16) as u16;
        let b = (key & 0xFFFF) as u16;
        if a > 0 && b > 0 {
            Self::Grid { a, b }
        } else {
            Self::Unknown(key)
        }
    }

    /// The key again.
    #[must_use]
    pub fn key(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Grid { a, b } => (u32::from(a) << 16) | u32::from(b),
            Self::Special(n) => TRAIT_CELL_SPECIAL_BASE.wrapping_add(n),
            Self::Unknown(k) => k,
        }
    }

    /// Whether the key has one of the observed forms (T2): 0, *a* 1–8 with *b* 1–12, or
    /// the special group with *n* 0–8.
    #[must_use]
    pub fn has_observed_form(self) -> bool {
        match self {
            Self::None => true,
            Self::Grid { a, b } => (1..=8).contains(&a) && (1..=12).contains(&b),
            Self::Special(n) => n <= 8,
            Self::Unknown(_) => false,
        }
    }
}

/// One `Trait_TraitTree_TraitStruct` (`combat-data.md` §8.5).
#[derive(Debug, Clone, PartialEq)]
pub struct TraitTreeEntry {
    /// `Trait_TraitTree_Trait` (T1: a point-based trait).
    pub trait_did: Did,
    /// `Trait_TraitTree_Branch` key.
    pub branch: Option<u32>,
    /// `Trait_TraitTree_TraitLocation` key.
    pub location: Option<u32>,
    /// (`Trait_TraitTree_TraitDependency` cell key, `_TraitDependencyRank`).
    pub dependencies: Vec<(u32, Option<i64>)>,
}

impl TraitTreeEntry {
    /// The decoded cell, if a location is stored.
    #[must_use]
    pub fn cell(&self) -> Option<TraitCell> {
        self.location.map(TraitCell::from_key)
    }
}

/// A trait tree (`combat-data.md` §8.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TraitTree {
    /// `Trait_TraitTree_TreeType` key.
    pub tree_type: Option<u32>,
    /// `Trait_TraitTree_TraitArray` entries that name a trait.
    pub entries: Vec<TraitTreeEntry>,
    /// Specialisations: (branch key, sparse DID progression).
    pub specializations: Vec<(Option<u32>, Did)>,
}

impl TraitTree {
    /// Reads a trait tree.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, t: &TraitPids) -> Self {
        let entries = set
            .structs(t.tree_array)
            .filter_map(|s| {
                Some(TraitTreeEntry {
                    trait_did: s.did(t.tree_trait)?,
                    branch: s.enum_key(t.tree_branch),
                    location: s.enum_key(t.tree_location),
                    dependencies: s
                        .structs(t.tree_dependencies)
                        .filter_map(|d| {
                            Some((
                                d.enum_key(t.tree_dependency)?,
                                d.integer(t.tree_dependency_rank),
                            ))
                        })
                        .collect(),
                })
            })
            .collect();
        let specializations = set
            .structs(t.tree_specializations)
            .filter_map(|s| {
                Some((
                    s.enum_key(t.tree_branch),
                    s.did_nonnull(t.tree_specialization_progression)?,
                ))
            })
            .collect();
        Self {
            tree_type: set.enum_key(t.tree_type),
            entries,
            specializations,
        }
    }
}

/// The trait control object (`combat-data.md` §8.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TraitControl {
    /// The six progression DIDs of §8.6, in the order listed there (non-zero ones).
    pub progressions: Vec<(&'static str, Did)>,
    /// `Trait_Control_FreepTraits` elements.
    pub player_traits: Vec<Did>,
    /// `Trait_Control_CreepTraits` elements.
    pub monster_traits: Vec<Did>,
    /// `Trait_Control_SkirmishTraits` elements.
    pub skirmish_traits: Vec<Did>,
    /// `Trait_Control_VirtueXP_Bonus_SoftCap`.
    pub virtue_xp_soft_cap: Option<i64>,
    /// `Trait_Control_VirtueXP_Bonus_HardCap`.
    pub virtue_xp_hard_cap: Option<i64>,
    /// `Trait_Control_VirtueTierToExperience_AdvancementTable`.
    pub virtue_tier_to_experience: Option<Did>,
}

impl TraitControl {
    /// Reads the trait control object.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, t: &TraitPids) -> Self {
        let progressions = [
            t.control_level_to_points,
            t.control_points_to_refund,
            t.control_rank_to_level,
            t.control_level_to_virtue_rank_min,
            t.control_level_to_virtue_rank_max,
            t.control_virtue_tier_to_item_level,
        ]
        .into_iter()
        .filter_map(|p| Some((p.label(), set.did_nonnull(p)?)))
        .collect();
        Self {
            progressions,
            player_traits: dids_inside(set, t.control_player_traits),
            monster_traits: dids_inside(set, t.control_monster_traits),
            skirmish_traits: dids_inside(set, t.control_skirmish_traits),
            virtue_xp_soft_cap: set.integer(t.control_virtue_xp_soft_cap),
            virtue_xp_hard_cap: set.integer(t.control_virtue_xp_hard_cap),
            virtue_tier_to_experience: set.did_nonnull(t.control_virtue_tier_to_experience),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Class and advancement tables (§9)
// ---------------------------------------------------------------------------------------------

/// One level of a class advancement table (`combat-data.md` §9.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelEntry {
    /// (`AdvTable_VitalType` key, `AdvTable_BaseVitalValue`).
    pub base_vitals: Vec<(u32, Option<f64>)>,
    /// (`AdvTable_StatType` key, `AdvTable_StatValue`).
    pub max_stats: Vec<(u32, Option<f64>)>,
}

/// One derived-stat formula (`combat-data.md` §9.1). Its reading (sum of coefficient ×
/// primary stat) is Reported; completeness is OPEN (Q13).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DerivedStatFormula {
    /// `AdvTable_DerivedStat` key.
    pub stat: Option<u32>,
    /// (`AdvTable_DerivedStat_Formula_Stat` key, `AdvTable_DerivedStat_Formula_Value`).
    pub terms: Vec<(u32, f32)>,
}

/// A class advancement table (`combat-data.md` §9.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassTable {
    /// `AdvTable_Class` key.
    pub class: Option<u32>,
    /// `AdvTable_LevelEntryList`, one per level in stored order (level 1 first).
    pub levels: Vec<LevelEntry>,
    /// `AdvTable_DerivedStat_Configuration` formulas.
    pub derived_stats: Vec<DerivedStatFormula>,
    /// Every DID inside `AdvTable_AvailableSkillEntryList`.
    pub available_skills: Vec<Did>,
    /// Every DID inside `AdvTable_AvailableMountSkillEntryList`.
    pub available_mount_skills: Vec<Did>,
    /// `AdvTable_Trait_WC` of each `AdvTable_ClassCharacteristic_List` entry.
    pub characteristic_traits: Vec<Did>,
}

fn pairs<A: PropertyAccess + ?Sized>(
    holder: &A,
    list: Pid,
    key: Pid,
    value: Pid,
) -> Vec<(u32, Option<f64>)> {
    find_structs(holder, list, key)
        .into_iter()
        .filter_map(|s| {
            Some((
                s.enum_key(key)?,
                s.get_pid(value).and_then(PropertyValue::as_f64),
            ))
        })
        .collect()
}

impl ClassTable {
    /// Reads a class advancement table.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, c: &ClassTablePids) -> Self {
        let levels = set
            .structs(c.level_entry_list)
            .map(|s| LevelEntry {
                base_vitals: pairs(s, c.base_vital_list, c.vital_type, c.base_vital_value),
                max_stats: pairs(s, c.max_stat_list, c.stat_type, c.stat_value),
            })
            .collect();
        let derived_stats = set
            .structs(c.derived_configuration)
            .map(|s| {
                let mut terms = Vec::new();
                for f in &s.fields {
                    let mut found = Vec::new();
                    structs_with(&f.value, c.derived_term_stat, &mut found);
                    terms.extend(found.into_iter().filter_map(|t| {
                        Some((
                            t.enum_key(c.derived_term_stat)?,
                            t.float(c.derived_term_value)?,
                        ))
                    }));
                }
                DerivedStatFormula {
                    stat: s.enum_key(c.derived_stat),
                    terms,
                }
            })
            .collect();
        Self {
            class: set.enum_key(c.class),
            levels,
            derived_stats,
            available_skills: dids_inside(set, c.available_skills),
            available_mount_skills: dids_inside(set, c.available_mount_skills),
            characteristic_traits: find_structs(set, c.characteristics, c.characteristic_trait)
                .into_iter()
                .filter_map(|s| s.did_nonnull(c.characteristic_trait))
                .collect(),
        }
    }
}

/// An advancement line (`combat-data.md` §9.3): DIDs in stored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdvancementLine {
    /// `AdvLine_DataID` of every `AdvLine_Entry`, null DIDs skipped.
    pub entries: Vec<Did>,
}

impl AdvancementLine {
    /// Reads an advancement line.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, c: &ClassTablePids) -> Self {
        Self {
            entries: find_structs(set, c.line_entry_list, c.line_data_id)
                .into_iter()
                .filter_map(|s| s.did_nonnull(c.line_data_id))
                .collect(),
        }
    }
}

/// A monster-play table (`combat-data.md` §9.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonsterPlayTable {
    /// `MonsterPlay_MonsterLevel`.
    pub level: Option<i64>,
    /// Every DID inside `MonsterPlay_SkillList`.
    pub skills: Vec<Did>,
    /// Every DID inside `MonsterPlay_TraitList`.
    pub traits: Vec<Did>,
}

impl MonsterPlayTable {
    /// Reads a monster-play table.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, c: &ClassTablePids) -> Self {
        Self {
            level: set.integer(c.monster_level),
            skills: dids_inside(set, c.monster_skill_list),
            traits: dids_inside(set, c.monster_trait_list),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Combat control, lookup tables, thresholds, battle statistics (§10)
// ---------------------------------------------------------------------------------------------

/// One `Combat_Control_CalcControlInfoStruct` (`combat-data.md` §10.1). The rating-curve
/// formula is Reported only (Q12), so only the data is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalcControl {
    /// `Combat_Control_CalculationType` key.
    pub calc_type: u32,
    /// `Combat_Control_Rating` progression.
    pub rating: Option<Did>,
    /// `Combat_Control_Target_Cap` progression.
    pub target_cap: Option<Did>,
    /// `Combat_Control_Hard_Cap` progression.
    pub hard_cap: Option<Did>,
}

/// The combat control object (`combat-data.md` §10.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CombatControl {
    /// Calculation entries that name a type.
    pub calc_controls: Vec<CalcControl>,
    /// `Combat_Control_MaxCombatLevel`.
    pub max_combat_level: Option<i64>,
}

impl CombatControl {
    /// Reads the combat control object.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, x: &CombatTablePids) -> Self {
        Self {
            calc_controls: set
                .structs(x.calc_array)
                .filter_map(|s| {
                    Some(CalcControl {
                        calc_type: s.enum_key(x.calc_type)?,
                        rating: s.did_nonnull(x.calc_rating),
                        target_cap: s.did_nonnull(x.calc_target_cap),
                        hard_cap: s.did_nonnull(x.calc_hard_cap),
                    })
                })
                .collect(),
            max_combat_level: set.integer(x.max_combat_level),
        }
    }

    /// The entry for calculation type `key`.
    #[must_use]
    pub fn calc(&self, key: u32) -> Option<&CalcControl> {
        self.calc_controls.iter().find(|c| c.calc_type == key)
    }
}

/// A lookup table of base values with five quality factors (`combat-data.md` §10.2): DPS
/// tables and item value tables share this shape.
///
/// Elements are offered by position only: the index base of the array (whether element 0
/// is level 1) is OPEN (Q12).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QualityTable {
    /// Base values in stored order.
    pub base: Vec<f32>,
    /// (quality key, factor) in stored order (observed order 5, 4, 2, 3, 1).
    pub qualities: Vec<(u32, f32)>,
}

impl QualityTable {
    /// The factor of quality `key`, looked up by key, never by position.
    #[must_use]
    pub fn quality(&self, key: u32) -> Option<f32> {
        self.qualities
            .iter()
            .find(|&&(k, _)| k == key)
            .map(|&(_, f)| f)
    }
}

/// A DPS table (class 1660 here, `combat-data.md` §10.2).
pub type DpsTable = QualityTable;

/// An item value table (class 1688 here, `combat-data.md` §10.2).
pub type ValueTable = QualityTable;

fn float_elements(value: &PropertyValue, element: Pid, out: &mut Vec<f32>) {
    for c in value.children() {
        match &c.value {
            PropertyValue::Float(f) if element.is(c.pid) => out.push(*f),
            PropertyValue::Struct(_) | PropertyValue::Array(_) => {
                float_elements(&c.value, element, out);
            }
            _ => {}
        }
    }
}

impl QualityTable {
    fn read<A: PropertyAccess + ?Sized>(
        set: &A,
        [base_array, base, quality_array, quality, factor]: [Pid; 5],
    ) -> Self {
        let mut values = Vec::new();
        if let Some(v) = set.get_pid(base_array) {
            float_elements(v, base, &mut values);
        }
        Self {
            base: values,
            qualities: find_structs(set, quality_array, quality)
                .into_iter()
                .filter_map(|s| Some((s.enum_key(quality)?, s.float(factor)?)))
                .collect(),
        }
    }

    /// Reads a DPS table (`Combat_BaseDPSArray`, `Combat_QualityModArray`).
    #[must_use]
    pub fn dps_from_properties<A: PropertyAccess + ?Sized>(set: &A, x: &CombatTablePids) -> Self {
        Self::read(
            set,
            [
                x.base_dps_array,
                x.base_dps,
                x.dps_quality_array,
                x.dps_quality,
                x.dps_quality_mod,
            ],
        )
    }

    /// Reads an item value table (`Item_BaseValArray`, `Item_QualityArray`).
    #[must_use]
    pub fn value_from_properties<A: PropertyAccess + ?Sized>(set: &A, x: &CombatTablePids) -> Self {
        Self::read(
            set,
            [
                x.base_value_array,
                x.base_value,
                x.value_quality_array,
                x.value_quality,
                x.value_quality_mod,
            ],
        )
    }
}

/// One state of a threshold policy (`combat-data.md` §10.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThresholdState {
    /// `Threshold_State_MinValue_Float`.
    pub min: Option<f32>,
    /// `Threshold_State_MaxValue_Float`.
    pub max: Option<f32>,
    /// Every non-null DID inside `Threshold_State_EffectList` (effects).
    pub effects: Vec<Did>,
}

/// A threshold policy (`combat-data.md` §10.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThresholdPolicy {
    /// `Threshold_Policy_CurrentStateIndexProperty`.
    pub index_property: Option<u32>,
    /// States in stored order.
    pub states: Vec<ThresholdState>,
}

impl ThresholdPolicy {
    /// Reads a threshold policy (the `Threshold_PercentageValue_Policy` struct).
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, x: &CombatTablePids) -> Self {
        let Some(policy) = set.structure(x.threshold_policy) else {
            return Self::default();
        };
        Self {
            index_property: nonzero_pid(policy.property_ref(x.threshold_index_property)),
            states: policy
                .structs(x.threshold_state_list)
                .map(|s| ThresholdState {
                    min: s.float(x.threshold_min),
                    max: s.float(x.threshold_max),
                    effects: dids_inside(s, x.threshold_effects),
                })
                .collect(),
        }
    }
}

/// A battle statistic (`combat-data.md` §10.4).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct BattleStatistic {
    pub name: Option<StringInfo>,
    pub description: Option<StringInfo>,
    pub scope: Option<u32>,
    pub update_policy: Option<u32>,
    pub replicate_to_clients: Option<bool>,
    pub forward_to: Option<Did>,
}

impl BattleStatistic {
    /// Reads a battle statistic.
    #[must_use]
    pub fn from_properties<A: PropertyAccess + ?Sized>(set: &A, x: &CombatTablePids) -> Self {
        Self {
            name: set.string_info(x.battle_stat_name).cloned(),
            description: set.string_info(x.battle_stat_description).cloned(),
            scope: set.enum_key(x.battle_stat_scope),
            update_policy: set.enum_key(x.battle_stat_update_policy),
            replicate_to_clients: set.boolean(x.battle_stat_replicate),
            forward_to: set.did_nonnull(x.battle_stat_forward),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Any combat object
// ---------------------------------------------------------------------------------------------

/// A combat object or related object, typed by its family (`combat-data.md` §2).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum CombatObject {
    Effect(Box<Effect>),
    Trait(Box<Trait>),
    Progression(Box<Progression>),
    Skill(Box<Skill>),
    MonsterPlayTable(MonsterPlayTable),
    AdvancementLine(AdvancementLine),
    ClassTable(Box<ClassTable>),
    ThresholdPolicy(ThresholdPolicy),
    BattleStatistic(Box<BattleStatistic>),
    TraitTree(TraitTree),
    SparseDidProgression(SparseDidProgression),
    CombatControl(CombatControl),
    EffectControl(EffectControl),
    TraitControl(TraitControl),
    DpsTable(QualityTable),
    ValueTable(QualityTable),
    Induction(Induction),
    Channelling(Channelling),
}

/// Reads `record` as the combat object its root class makes it: a family of §2.1 first,
/// then a related object. `Ok(None)` for any other class.
pub fn read_combat_object<P: ClassParents + ?Sized>(
    record: &GameObjectRecord,
    classes: &CombatClasses,
    parents: &P,
    pids: &CombatPids,
) -> Result<Option<CombatObject>> {
    let set = &record.properties;
    let class = record.root_class();
    if let Some(family) = classes.family(parents, class) {
        return Ok(Some(match family {
            CombatFamily::Effect => CombatObject::Effect(Box::new(Effect::read(record, pids)?)),
            CombatFamily::Trait => CombatObject::Trait(Box::new(Trait::read(record, pids)?)),
            CombatFamily::Progression => CombatObject::Progression(Box::new(Progression::read(
                record,
                classes,
                parents,
                &pids.progression,
            )?)),
            CombatFamily::Skill => CombatObject::Skill(Box::new(Skill::read(record, pids)?)),
            CombatFamily::MonsterPlayOrAdvancementLine => {
                if class == classes.monster_play {
                    CombatObject::MonsterPlayTable(MonsterPlayTable::from_properties(
                        set,
                        &pids.class_table,
                    ))
                } else {
                    CombatObject::AdvancementLine(AdvancementLine::from_properties(
                        set,
                        &pids.class_table,
                    ))
                }
            }
            CombatFamily::ClassTable => CombatObject::ClassTable(Box::new(
                ClassTable::from_properties(set, &pids.class_table),
            )),
            CombatFamily::ThresholdPolicy => {
                CombatObject::ThresholdPolicy(ThresholdPolicy::from_properties(set, &pids.tables))
            }
            CombatFamily::BattleStatistic => CombatObject::BattleStatistic(Box::new(
                BattleStatistic::from_properties(set, &pids.tables),
            )),
        }));
    }
    let Some(related) = classes.related(class) else {
        return Ok(None);
    };
    Ok(Some(match related {
        RelatedObject::TraitTree => {
            CombatObject::TraitTree(TraitTree::from_properties(set, &pids.traits))
        }
        RelatedObject::SparseDidProgression => CombatObject::SparseDidProgression(
            SparseDidProgression::from_properties(set, &pids.progression),
        ),
        RelatedObject::CombatControl => {
            CombatObject::CombatControl(CombatControl::from_properties(set, &pids.tables))
        }
        RelatedObject::EffectControl => {
            CombatObject::EffectControl(EffectControl::from_properties(set, pids))
        }
        RelatedObject::TraitControl => {
            CombatObject::TraitControl(TraitControl::from_properties(set, &pids.traits))
        }
        RelatedObject::DpsTable => {
            CombatObject::DpsTable(QualityTable::dps_from_properties(set, &pids.tables))
        }
        RelatedObject::ValueTable => {
            CombatObject::ValueTable(QualityTable::value_from_properties(set, &pids.tables))
        }
        RelatedObject::Induction => {
            CombatObject::Induction(Induction::from_properties(set, &pids.timing)?)
        }
        RelatedObject::Channelling => {
            CombatObject::Channelling(Channelling::from_properties(set, &pids.timing)?)
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(key: i32, value: f32) -> ControlPoint {
        ControlPoint { key, value }
    }

    #[test]
    fn slope_is_single_precision() {
        let s = segment_slope(pt(1, 10.0), pt(11, 30.0));
        assert_eq!(s.to_bits(), 2.0f32.to_bits());
        let a = pt(3, 0.1);
        let b = pt(10, 0.7);
        assert_eq!(
            segment_slope(a, b).to_bits(),
            ((0.7f32 - 0.1f32) / 7.0f32).to_bits()
        );
    }

    #[test]
    fn cell_keys_round_trip() {
        for key in [
            0,
            0x0001_0001,
            0x0008_000C,
            0x1000_0000,
            0x1000_0008,
            0x0000_0005,
        ] {
            assert_eq!(TraitCell::from_key(key).key(), key);
        }
        assert_eq!(
            TraitCell::from_key(0x0003_0007),
            TraitCell::Grid { a: 3, b: 7 }
        );
        assert!(!TraitCell::from_key(0x0009_0001).has_observed_form());
        assert!(TraitCell::from_key(0x1000_0004).has_observed_form());
    }

    #[test]
    fn ops_resolve_by_key_fallback() {
        assert_eq!(ModOp::resolve(7, None), Some(ModOp::Add));
        assert_eq!(ModOp::resolve(9, None), None);
        for op in ModOp::ALL {
            assert_eq!(ModOp::from_label(op.label()), Some(op));
        }
    }
}
