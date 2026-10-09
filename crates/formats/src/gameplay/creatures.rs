// SPDX-License-Identifier: GPL-3.0-or-later
//! Creatures, NPCs and spawning: templates, AI tables, generator profiles and the
//! encounter table (prefix 0x14).
//!
//! Spec: `docs/specs/creatures.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`LabelIndex`], [`CreaturePids`] | Dictionary labels both ways; every property this module reads, resolved at run time | §1a, §4–§6 |
//! | [`TemplateKind`], [`WeenieKinds`], [`TemplateFinder`], [`CreatureSubtree`] | Mob, NPC and container templates by `WeenieType` label; the subtree root as their nearest common ancestor | §2.2 |
//! | [`MirrorPattern`], [`mirror_base`], [`MirrorTable`] | Mirror properties and their bases | §3.2.1 |
//! | [`ClassConstantsBuilder`], [`ClassConstants`], [`TemplateStore`] | Class-constant and varying properties; the D1 template store and its four-step lookup | §3.2.2 |
//! | [`CreatureTemplate`] | Typed view of a template's identity, presentation, AI, combat, loot, roles and effects | §4 |
//! | [`displayed_level`], [`immunity_indicators`] | Values of the creature information panel | §4.8 |
//! | [`ActionTable`], [`ProbabilityTable`], [`AttitudeTable`], [`ClassificationFilterList`], [`Classification`], [`ColorEventTable`], [`Formation`], [`Conjunction`], [`PuppetPackage`] | AI records | §5 |
//! | [`CreatureRecordKind`], [`RecordKindIndex`] | AI and profile records recognised by signature, then by class | §5, §6.5 |
//! | [`GeneratorProfile`] and its tree | Generator-profile property sets, with the §6.5 shape checks | §6.5, CR9, CR10, CR14 |
//! | [`resolve_candidates`] | Static resolution: which templates can appear at a spawn point | §6.7 |
//! | [`evaluate_profile`] | Decision D3: one evaluation of a profile at a spawn point | §6.8 |
//! | [`LandblockSpawns`], [`PatrolGraph`] | Generators, spawn points, position sets, type-8 sources and patrol chains of one landblock | §6.2–§6.4, §6.9 |
//! | [`EncounterTable`] | The encounter table (prefix 0x14) | §7 |
//!
//! Class indices, PIDs, DIDs and enum keys are build-specific: nothing here hard-codes
//! one. Properties are found by dictionary label ([`CreaturePids::resolve`]); the
//! `WeenieType` labels come from the install's enum mapper ([`WeenieKinds`]); record kinds
//! are recognised by the properties they carry ([`CreatureRecordKind`]).

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::did::{Did, DidKind};
use crate::enum_mapper::EnumMapper;
use crate::error::{FormatError, Result};
use crate::gameplay::common::{ClassParents, Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::landblock::{InstanceId, LandblockInfo, StaticEntityType};
use crate::property::{
    PropertyArray, PropertyDictionary, PropertyEntry, PropertyNames, PropertyStruct, PropertyValue,
};
use crate::property_set::PropertySet;
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

/// The property dictionary's labels indexed both ways (`property-system.md` §2).
///
/// Besides exact lookups it answers [`LabelIndex::unique_match`] (a prefix and suffix
/// search, kept for tools; the module itself resolves every label in full, `creatures.md`
/// §5.3 answer, C32).
#[derive(Debug, Clone, Default)]
pub struct LabelIndex {
    by_name: HashMap<String, u32>,
    by_pid: HashMap<u32, String>,
}

impl LabelIndex {
    /// Builds the index from (PID, label) pairs. PID 0 is skipped; when a label or PID
    /// occurs twice, the first pair wins (as in the dictionary).
    #[must_use]
    pub fn new<I, S>(labels: I) -> Self
    where
        I: IntoIterator<Item = (u32, S)>,
        S: AsRef<str>,
    {
        let mut out = Self::default();
        for (pid, name) in labels {
            if pid == 0 {
                continue;
            }
            let name = name.as_ref();
            out.by_name.entry(name.to_owned()).or_insert(pid);
            out.by_pid.entry(pid).or_insert_with(|| name.to_owned());
        }
        out
    }

    /// The labels of the install's property dictionary.
    #[must_use]
    pub fn from_dictionary(dict: &PropertyDictionary) -> Self {
        Self::new(dict.names().iter().map(|(p, n)| (*p, n.as_str())))
    }

    /// The PID labelled `name`.
    #[must_use]
    pub fn pid(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// The label of `pid`.
    #[must_use]
    pub fn name(&self, pid: u32) -> Option<&str> {
        self.by_pid.get(&pid).map(String::as_str)
    }

    /// Number of labels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Every (PID, label) pair, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> + '_ {
        self.by_name.iter().map(|(n, &p)| (p, n.as_str()))
    }

    /// The one PID whose label starts with `prefix`, ends with `suffix` and is longer than
    /// `prefix`. `None` when no label or more than one PID matches (the second case is
    /// logged).
    #[must_use]
    pub fn unique_match(&self, prefix: &str, suffix: &str) -> Option<u32> {
        let mut found: Option<u32> = None;
        for (name, &pid) in &self.by_name {
            if name.len() > prefix.len() && name.starts_with(prefix) && name.ends_with(suffix) {
                if found.is_some_and(|f| f != pid) {
                    tracing::warn!(prefix, suffix, "abbreviated property label is ambiguous");
                    return None;
                }
                found = Some(pid);
            }
        }
        found
    }

    /// A [`Pid`] for a label the spec abbreviates: the unique label with `prefix` and
    /// `suffix` ([`LabelIndex::unique_match`]); `pattern` is kept for error messages.
    #[must_use]
    pub fn abbreviated(&self, pattern: &'static str, prefix: &str, suffix: &str) -> Pid {
        match self.unique_match(prefix, suffix) {
            Some(id) => Pid::with_id(pattern, id),
            None => Pid::unresolved(pattern),
        }
    }
}

impl PropertyNames for LabelIndex {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.pid(name)
    }
}

crate::property_ids! {
    /// Template properties (§4).
    pub struct TemplatePids {
        name = "Name",
        weenie_type = "WeenieType",
        species = "Agent_Species",
        subspecies = "Agent_Subspecies",
        show_subspecies = "Agent_ShowSubspecies",
        genus = "Agent_Genus",
        alignment = "Agent_Alignment",
        agent_class = "Agent_Class",
        classification = "Agent_Classification",
        classification_filter = "Agent_ClassificationFilter",
        monster_division = "Quest_MonsterDivision",
        examination_rank = "MonsterLevel_ExaminationModStatType",
        creation_type = "Entity_CreationType",
        not_alive_name = "Agent_NotAlive_Name",
        phys_obj = "PhysObj",
        appearance_class_list = "Appearance_ClassList",
        appearance_key = "Appearance_Key",
        appearance_apr_file = "Appearance_AprFile",
        appearance_modifier = "Appearance_Modifier",
        random_appearance_type = "Appearance_RandomAppearanceType",
        render_scale = "Render_AdjustableScale",
        physics_scale = "Physics_AdjustableScale",
        cosmetic_loadout_set = "Inventory_Cosmetic_LoadoutSet",
        torso_material = "Material_TorsoMaterial",
        boot_material = "Material_BootMaterial",
        off_hand_material = "Material_OffHandMaterial",
        weapon_material = "Material_WeaponMaterial",
        combat_animation_set = "Combat_AnimationSet",
        combat_fx_set = "Combat_FXSet",
        action_table_array = "AI_ActionTable_Array",
        action_table = "AI_ActionTable",
        action_table_min_level = "AI_ActionTable_MinLevel",
        action_table_max_level = "AI_ActionTable_MaxLevel",
        attitude_table = "Attitude_Table",
        perception_radius = "AI_PerceptionRadius",
        movement_type = "AI_MovementType",
        call_for_help_radius = "AI_CallForHelp_Radius",
        call_for_help_respond_types = "AI_CallForHelp_RespondTypes",
        call_for_help_filter = "AI_CallForHelpFilter",
        hints = "AI_Hints",
        color_event_table = "AI_ColorEventTable",
        formation_group = "Formation_Group_DataID",
        formation_layout = "Formation_Layout_Type",
        puppet_package = "AIPuppet_TargetEvalPackage",
        puppet_flags = "AIPuppet_TargetFlags",
        is_puppet = "AI_IsPuppetNPC",
        kung_fu_start_as_npc = "AI_KungFu_StartAsNPC",
        kung_fu_repeatable = "AI_KungFu_Repeatable",
        physically_wanders = "AI_PhysicallyWanders",
        wandering_range = "AI_WanderingRange",
        wandering_prob = "AI_WanderingProb",
        combat_event_table = "GameEvent_CombatEventTable",
        conjunction_event_table = "GameEvent_ConjunctionEventTable",
        can_block = "Combat_CanBlock",
        can_parry = "Combat_CanParry",
        can_evade = "Combat_CanEvade",
        state_immunities = "CombatState_CurrentStateImmunities",
        never_say_die = "Death_NeverSayDie",
        not_attackable = "Combat_NotAttackable",
        health_starting_level = "Health_Starting_Level",
        conjunction_immune = "Conjunction_Immune",
        advancement_level = "Advancement_Level",
        health_max = "Health_MaxLevel",
        power_max = "Power_MaxLevel",
        health_max_cached = "Health_MaxLevel_Cached",
        power_max_cached = "Power_MaxLevel_Cached",
        trophy_list = "LootGen_TrophyList_Override",
        treasure_list = "LootGen_TreasureList_Override",
        barter_trophy_list = "LootGen_BarterTrophyList",
        reputation_trophy_list = "LootGen_ReputationTrophyList",
        filtered_trophy_table = "LootGen_FilteredTrophyTable",
        generates_trophies = "LootGen_GeneratesTrophies",
        remote_lootable = "Loot_IsRemoteLootable",
        open_tapping = "Tapping_OpenTapping",
        active_quests = "QuestDispenser_ActiveQuests",
        quest = "QuestDispenser_Quest",
        role_name = "QuestDispenser_RoleName",
        role_constraint = "QuestDispenser_RoleConstraint",
        related_quest = "Quest_RelatedQuest",
        positive_faction = "Reputation_PositiveFaction",
        negative_faction = "Reputation_NegativeFaction",
        comment_array = "NPCComment_DIDCommentArray",
        occupation_title = "OccupationTitle",
        map_note_type = "MapNote_Type",
        startup_effects = "Effect_MonsterStartupEffect_Array",
        init_effects = "Effect_MonsterInitEffect_Array",
        startup_effect_id = "Effect_StartupEffectID",
        startup_effect_spellcraft = "Effect_StartupEffectSpellcraft",
        startup_effect_damage_qualifier = "Effect_StartupEffectDamageQualifier",
        script_tables = "Entity_AdditionalScriptTables",
        usage_action = "Usage_Action",
        csm_action_handler = "CSM_ActionHandler",
        induction_action = "Induction_ActionDID",
        movement_speed = "Movement_SpeedMultiplier",
    }
}

crate::property_ids! {
    /// AI-record properties (§5).
    pub struct AiRecordPids {
        actions = "AIActionTable_Actions",
        chain = "AIAction_Chain",
        skill = "AIAction_Skill",
        recovery = "AIAction_Recovery",
        probability = "AIAction_Probability",
        cooldown = "AIAction_Cooldown",
        initial_cooldown = "AIAction_InitialCooldown",
        target_cooldown = "AIAction_TargetCooldown",
        cooldown_channel = "AIAction_CooldownChannel",
        required_hints = "AIAction_RequiredHints",
        disallowed_hints = "AIAction_DisallowedHints",
        target_required_hints = "AIAction_TargetRequiredHints",
        target_disallowed_hints = "AIAction_TargetDisallowedHints",
        custom_target = "AIAction_CustomTarget",
        color_event_type = "AIAction_ColorEventType",
        probability_array = "AIActionProbability_Array",
        probability_type = "AIActionProbability_Type",
        probability_value = "AIActionProbability_Value",
        friendly_filters = "Attitude_FriendlyFilterList",
        aggressive_filters = "Attitude_AggressiveFilterList",
        filter_list = "ClassificationFilter_FilterList",
        filter_type = "ClassificationFilter_Filter_Type",
        filter_default = "ClassificationFilter_Filter_Default",
        exception_list = "ClassificationFilter_FilterExceptionList",
        classification_species = "Classification_Species",
        classification_genus = "Classification_Genus",
        classification_alignment = "Classification_Alignment",
        color_events = "AIColorEventTable_Events",
        color_event = "AIColorEventTable_Event",
        event_type = "AIColorEventTable_Event_Type",
        event_probability = "AIColorEventTable_Event_Probability",
        event_say_type = "AIColorEventTable_Event_SayType",
        event_sound_channel = "AIColorEventTable_Event_SoundChannel",
        event_data = "AIColorEventTable_Event_Data",
        event_string = "AIColorEventTable_Event_String",
        event_sound = "AIColorEventTable_Event_SoundID",
        formation_group = "Formation_Group",
        formation_follower = "Formation_Follower",
        conjunction_name = "Conjunction_Name",
        conjunction_description = "Conjunction_Description",
        conjunction_category = "Conjunction_Category",
        conjunction_flags = "Conjunction_Flags",
        icon_small = "Conjunction_Icon_Small",
        icon_medium = "Conjunction_Icon_Medium",
        icon_large = "Conjunction_Icon_Large",
        role_list = "Conjunction_RoleList",
        contribution = "Conjunction_Role_ContributionToActionMap",
        role_action_skill = "Conjunction_Role_ActionSkill",
        pvp_role_action_skill = "Conjunction_PvP_Role_ActionSkill",
        contribution_skill = "Conjunction_Role_ContributionSkill",
        conjunction_list = "Conjunction_MonsterPlay_ConjunctionList",
        puppet_queue_type = "AIPuppet_TargetRequestQueue_Type",
    }
}

crate::property_ids! {
    /// Generator-profile properties named in full (§6.5).
    pub struct ProfilePids {
        profile = "GeneratorProfile_Profile",
        regen_type = "GeneratorProfile_RegenType",
        entry = "GeneratorProfile_Entry",
        one_of = "GeneratorProfile_OneOf",
        position_set_entry = "GeneratorProfile_PositionSetEntry",
        position_set_id = "GeneratorProfile_PositionSetID",
        wsl_entity = "GeneratorProfile_WSLEntity",
        profile_definition = "GeneratorProfile_ProfileDefinition",
        weight = "GeneratorProfile_Weight",
        probability = "GeneratorProfile_Probability",
        repeat = "GeneratorProfile_Repeat",
        exact_placement = "GeneratorProfile_ExactPlacement",
        random_placement = "GeneratorProfile_RandomPlacement",
        properties = "GeneratorProfile_Properties",
    }
}

crate::property_ids! {
    /// Properties of generator instance sets, spawn points and links (§6.2–§6.4, §6.9).
    pub struct SpawnPids {
        profile_did = "Generator_ProfileDID",
        limit_array = "Generator_PositionSetLimitArray",
        position_set_id = "Generator_PositionSetID",
        limit = "Generator_PositionSetLimit",
        physically_wanders = "AI_PhysicallyWanders",
        wandering_range = "AI_WanderingRange",
        wandering_prob = "AI_WanderingProb",
        perception_radius = "GeneratorProfile_WBOnly_PerceptionRadius",
        link_name = "LinkName",
        patrol_speed = "PatrolLink_Speed",
    }
}

/// Fields of `GeneratorProfile_Repeat`, `…_ExactPlacement` and `…_RandomPlacement` (§6.5),
/// by their full dictionary labels (§5.3 answer, C32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileFieldPids {
    /// `GeneratorProfile_RepeatMinTimes` (int, dictionary default 1).
    pub repeat_min: Pid,
    /// `GeneratorProfile_RepeatMaxTimes` (int, dictionary default 1).
    pub repeat_max: Pid,
    /// `GeneratorProfile_ExactPlacementX`, `…Y`, `…Z` (float, default 0.0).
    pub exact: [Pid; 3],
    /// `GeneratorProfile_RandomPlacementMinDist` (float).
    pub random_min: Pid,
    /// `GeneratorProfile_RandomPlacementMaxDist` (float).
    pub random_max: Pid,
}

impl ProfileFieldPids {
    const REPEAT_MIN: &'static str = "GeneratorProfile_RepeatMinTimes";
    const REPEAT_MAX: &'static str = "GeneratorProfile_RepeatMaxTimes";
    const EXACT: [&'static str; 3] = [
        "GeneratorProfile_ExactPlacementX",
        "GeneratorProfile_ExactPlacementY",
        "GeneratorProfile_ExactPlacementZ",
    ];
    const RANDOM_MIN: &'static str = "GeneratorProfile_RandomPlacementMinDist";
    const RANDOM_MAX: &'static str = "GeneratorProfile_RandomPlacementMaxDist";

    /// Resolves the labels against `labels`.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(labels: &N) -> Self {
        let [x, y, z] = Self::EXACT;
        Self {
            repeat_min: Pid::resolve(labels, Self::REPEAT_MIN),
            repeat_max: Pid::resolve(labels, Self::REPEAT_MAX),
            exact: [
                Pid::resolve(labels, x),
                Pid::resolve(labels, y),
                Pid::resolve(labels, z),
            ],
            random_min: Pid::resolve(labels, Self::RANDOM_MIN),
            random_max: Pid::resolve(labels, Self::RANDOM_MAX),
        }
    }

    /// Every field unresolved.
    #[must_use]
    pub const fn unresolved() -> Self {
        let [x, y, z] = Self::EXACT;
        Self {
            repeat_min: Pid::unresolved(Self::REPEAT_MIN),
            repeat_max: Pid::unresolved(Self::REPEAT_MAX),
            exact: [Pid::unresolved(x), Pid::unresolved(y), Pid::unresolved(z)],
            random_min: Pid::unresolved(Self::RANDOM_MIN),
            random_max: Pid::unresolved(Self::RANDOM_MAX),
        }
    }

    /// Every field.
    #[must_use]
    pub fn all(&self) -> Vec<Pid> {
        let [x, y, z] = self.exact;
        vec![
            self.repeat_min,
            self.repeat_max,
            x,
            y,
            z,
            self.random_min,
            self.random_max,
        ]
    }
}

/// Fields of a classification-filter exception (§5.3), by their full dictionary labels
/// (C32): `ClassificationFilter_FilterException_Alignment` (enum), `…_Genus` (32-bit
/// bitfield), `…_Species` (enum) and `…_Subspecies` (enum), all with dictionary default 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterExceptionPids {
    /// `ClassificationFilter_FilterException_Alignment`.
    pub alignment: Pid,
    /// `ClassificationFilter_FilterException_Genus`.
    pub genus: Pid,
    /// `ClassificationFilter_FilterException_Species`.
    pub species: Pid,
    /// `ClassificationFilter_FilterException_Subspecies`.
    pub subspecies: Pid,
}

impl FilterExceptionPids {
    const ALIGNMENT: &'static str = "ClassificationFilter_FilterException_Alignment";
    const GENUS: &'static str = "ClassificationFilter_FilterException_Genus";
    const SPECIES: &'static str = "ClassificationFilter_FilterException_Species";
    const SUBSPECIES: &'static str = "ClassificationFilter_FilterException_Subspecies";

    /// Resolves the labels against `labels`.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(labels: &N) -> Self {
        Self {
            alignment: Pid::resolve(labels, Self::ALIGNMENT),
            genus: Pid::resolve(labels, Self::GENUS),
            species: Pid::resolve(labels, Self::SPECIES),
            subspecies: Pid::resolve(labels, Self::SUBSPECIES),
        }
    }

    /// Every field unresolved.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            alignment: Pid::unresolved(Self::ALIGNMENT),
            genus: Pid::unresolved(Self::GENUS),
            species: Pid::unresolved(Self::SPECIES),
            subspecies: Pid::unresolved(Self::SUBSPECIES),
        }
    }

    /// Every field.
    #[must_use]
    pub fn all(&self) -> Vec<Pid> {
        vec![self.alignment, self.genus, self.species, self.subspecies]
    }
}

/// Every property this module reads, resolved against one install's dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreaturePids {
    /// Template properties (§4).
    pub template: TemplatePids,
    /// AI-record properties (§5).
    pub ai: AiRecordPids,
    /// Filter-exception fields (§5.3).
    pub exceptions: FilterExceptionPids,
    /// Generator-profile properties (§6.5).
    pub profile: ProfilePids,
    /// Repeat and placement fields of the profile (§6.5).
    pub profile_fields: ProfileFieldPids,
    /// Generator, spawn-point and link properties (§6.2–§6.4, §6.9).
    pub spawn: SpawnPids,
}

impl CreaturePids {
    /// Resolves every label.
    #[must_use]
    pub fn resolve(labels: &LabelIndex) -> Self {
        Self {
            template: TemplatePids::resolve(labels),
            ai: AiRecordPids::resolve(labels),
            exceptions: FilterExceptionPids::resolve(labels),
            profile: ProfilePids::resolve(labels),
            profile_fields: ProfileFieldPids::resolve(labels),
            spawn: SpawnPids::resolve(labels),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            template: TemplatePids::unresolved(),
            ai: AiRecordPids::unresolved(),
            exceptions: FilterExceptionPids::unresolved(),
            profile: ProfilePids::unresolved(),
            profile_fields: ProfileFieldPids::unresolved(),
            spawn: SpawnPids::unresolved(),
        }
    }

    /// The labels that did not resolve.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.template.missing();
        out.extend(self.ai.missing());
        out.extend(
            self.exceptions
                .all()
                .into_iter()
                .filter(|p| !p.is_resolved())
                .map(Pid::label),
        );
        out.extend(self.profile.missing());
        out.extend(
            self.profile_fields
                .all()
                .into_iter()
                .filter(|p| !p.is_resolved())
                .map(Pid::label),
        );
        out.extend(self.spawn.missing());
        out
    }
}

/// WEENIECONTENT label of the probability table (§5.2).
pub const PROBABILITY_TABLE_LABEL: &str = "AIActionProbability";
/// WEENIECONTENT label of the monster level-range directory (§4.8).
pub const LEVEL_RANGE_DIRECTORY_LABEL: &str = "MonsterLevelRangeDirectory";
/// WEENIECONTENT label of the monster level directory (§4.8).
pub const LEVEL_DIRECTORY_LABEL: &str = "MonsterLevelDirectory";
/// WEENIECONTENT label of the conjunction list (§5.7).
pub const CONJUNCTION_TABLE_LABEL: &str = "ConjunctionTable";

// ---------------------------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------------------------

/// The set bits of a 32-bit, 64-bit or any-width bitfield value, ascending; `None` for other
/// types. Bit *i* is labelled by key *i* + 1 of the property's mapper (§8).
#[must_use]
pub fn bitfield_bits(value: &PropertyValue) -> Option<Vec<u32>> {
    match value {
        PropertyValue::Bitfield32(b) => Some((0..32).filter(|i| b & (1 << i) != 0).collect()),
        PropertyValue::Bitfield64(b) => Some((0..64).filter(|i| b & (1 << i) != 0).collect()),
        PropertyValue::BitfieldAny(b) => Some(b.set_bits().collect()),
        _ => None,
    }
}

fn bits_of<A: PropertyAccess + ?Sized>(set: &A, pid: Pid) -> Option<Vec<u32>> {
    bitfield_bits(set.get_pid(pid)?)
}

fn string_info_of<A: PropertyAccess + ?Sized>(set: &A, pid: Pid) -> Option<StringInfo> {
    set.string_info(pid).cloned()
}

/// Struct elements of an array value, in stored order.
fn struct_elements(array: &PropertyArray) -> impl Iterator<Item = (u32, &PropertyStruct)> {
    array
        .elements
        .iter()
        .filter_map(|e| e.value.as_struct().map(|s| (e.pid, s)))
}

// ---------------------------------------------------------------------------------------------
// §2.2 Finding templates
// ---------------------------------------------------------------------------------------------

/// `WeenieType` label of a mob template.
pub const WEENIE_MONSTER: &str = "Monster";
/// `WeenieType` label of an NPC template.
pub const WEENIE_REAL_NPC: &str = "RealNPC";
/// Second `WeenieType` label of an NPC template.
pub const WEENIE_NPC: &str = "NPC";
/// `WeenieType` label of a container template.
pub const WEENIE_GAMEPLAY_CONTAINER: &str = "GameplayContainer";

/// What a template is, by its `WeenieType` label (§1a, §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TemplateKind {
    /// `Monster`.
    Mob,
    /// `RealNPC` or `NPC`.
    Npc,
    /// `GameplayContainer` (chests and resource nodes).
    Container,
}

impl TemplateKind {
    /// The kind a `WeenieType` label marks, if any.
    #[must_use]
    pub fn from_weenie_label(label: &str) -> Option<Self> {
        match label {
            WEENIE_MONSTER => Some(Self::Mob),
            WEENIE_REAL_NPC | WEENIE_NPC => Some(Self::Npc),
            WEENIE_GAMEPLAY_CONTAINER => Some(Self::Container),
            _ => None,
        }
    }
}

/// `WeenieType` enum key → template kind, built from the install's enum mapper (§2.2 step 2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WeenieKinds {
    by_key: HashMap<u32, TemplateKind>,
}

impl WeenieKinds {
    /// Builds the map from (key, label) pairs; for a key listed twice the first label wins.
    #[must_use]
    pub fn from_labels<'a>(labels: impl IntoIterator<Item = (u32, &'a str)>) -> Self {
        let mut seen = HashSet::new();
        let mut by_key = HashMap::new();
        for (key, label) in labels {
            if !seen.insert(key) {
                continue;
            }
            if let Some(kind) = TemplateKind::from_weenie_label(label) {
                by_key.insert(key, kind);
            }
        }
        Self { by_key }
    }

    /// Builds the map from a mapper and its base chain, nearest first (the mapper the
    /// dictionary names for `WeenieType`, then its bases). A key's label is taken from the
    /// first mapper that names it.
    #[must_use]
    pub fn from_mapper_chain<M: Borrow<EnumMapper>>(chain: &[M]) -> Self {
        Self::from_labels(
            chain
                .iter()
                .flat_map(|m| m.borrow().names().iter().map(|(k, n)| (*k, n.as_str()))),
        )
    }

    /// The kind of `WeenieType` key `key`.
    #[must_use]
    pub fn kind(&self, key: u32) -> Option<TemplateKind> {
        self.by_key.get(&key).copied()
    }

    /// The kind of a property set by its `WeenieType`.
    #[must_use]
    pub fn of_set<A: PropertyAccess + ?Sized>(
        &self,
        set: &A,
        weenie_type: Pid,
    ) -> Option<TemplateKind> {
        self.kind(set.enum_key(weenie_type)?)
    }

    /// Number of keys that mark a template kind.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    /// Whether no key marks a template kind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

/// Template-kind counts per root class, gathered over object records (§2.2 steps 1–2).
#[derive(Debug, Clone, Default)]
pub struct TemplateFinder {
    classes: BTreeMap<u32, BTreeMap<TemplateKind, usize>>,
    records: usize,
}

impl TemplateFinder {
    /// An empty finder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one object record: its root class and the kind its `WeenieType` marks.
    pub fn add(&mut self, root_class: u32, kind: Option<TemplateKind>) {
        self.records += 1;
        if let Some(kind) = kind {
            *self
                .classes
                .entry(root_class)
                .or_default()
                .entry(kind)
                .or_default() += 1;
        }
    }

    /// Number of records added.
    #[must_use]
    pub fn records(&self) -> usize {
        self.records
    }

    /// (root class, kind, records) for every class where a kind was seen, ascending.
    pub fn kind_classes(&self) -> impl Iterator<Item = (u32, TemplateKind, usize)> + '_ {
        self.classes
            .iter()
            .flat_map(|(&c, m)| m.iter().map(move |(&k, &n)| (c, k, n)))
    }

    /// The creature subtree (§2.2 step 3): the nearest common ancestor of every class where
    /// a template kind was seen. `None` when no kind was seen or the classes share no
    /// ancestor.
    #[must_use]
    pub fn subtree<P: ClassParents + ?Sized>(&self, parents: &P) -> Option<CreatureSubtree> {
        parents
            .common_ancestor(self.classes.keys().copied())
            .map(|root| CreatureSubtree { root })
    }
}

/// The creature subtree of the class library (§2.1, §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreatureSubtree {
    /// The library class every template's root class descends from (or is).
    pub root: u32,
}

impl CreatureSubtree {
    /// Whether a record with root class `class` is a template.
    #[must_use]
    pub fn contains<P: ClassParents + ?Sized>(&self, parents: &P, class: u32) -> bool {
        parents.is_a(class, self.root)
    }
}

// ---------------------------------------------------------------------------------------------
// §3.2.1 Mirror properties
// ---------------------------------------------------------------------------------------------

/// One of the six mirror name patterns (§3.2.1), in table order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MirrorPattern {
    /// `Forward_Cached_`*X*.
    ForwardCachedPrefix,
    /// *X*`_Cached`.
    CachedSuffix,
    /// `ForwardSource_`*X*.
    ForwardSourcePrefix,
    /// *X*`_ForwardSource`.
    ForwardSourceSuffix,
    /// *X*`_Shadow`.
    ShadowSuffix,
    /// *X*`_Base`.
    BaseSuffix,
}

impl MirrorPattern {
    /// The six patterns in the order of the §3.2.1 table.
    pub const ALL: [Self; 6] = [
        Self::ForwardCachedPrefix,
        Self::CachedSuffix,
        Self::ForwardSourcePrefix,
        Self::ForwardSourceSuffix,
        Self::ShadowSuffix,
        Self::BaseSuffix,
    ];

    /// The pattern's affix and whether it is a prefix.
    #[must_use]
    pub const fn affix(self) -> (&'static str, bool) {
        match self {
            Self::ForwardCachedPrefix => ("Forward_Cached_", true),
            Self::CachedSuffix => ("_Cached", false),
            Self::ForwardSourcePrefix => ("ForwardSource_", true),
            Self::ForwardSourceSuffix => ("_ForwardSource", false),
            Self::ShadowSuffix => ("_Shadow", false),
            Self::BaseSuffix => ("_Base", false),
        }
    }

    /// The base name of `name` under this pattern: `name` without the affix. `None` when the
    /// name does not carry the affix or nothing would remain.
    #[must_use]
    pub fn base_of(self, name: &str) -> Option<&str> {
        let (affix, prefix) = self.affix();
        let base = if prefix {
            name.strip_prefix(affix)?
        } else {
            name.strip_suffix(affix)?
        };
        (!base.is_empty()).then_some(base)
    }

    /// The mirror name of `base` under this pattern.
    #[must_use]
    pub fn mirror_name(self, base: &str) -> String {
        let (affix, prefix) = self.affix();
        if prefix {
            format!("{affix}{base}")
        } else {
            format!("{base}{affix}")
        }
    }
}

/// Whether `name` is a mirror name, and its pattern and base (§3.2.1). A name matching
/// several patterns takes the first in table order ([`MirrorPattern::ALL`]; Decision D1, no
/// name of the observed install matches two, C31).
#[must_use]
pub fn mirror_base(name: &str) -> Option<(MirrorPattern, &str)> {
    MirrorPattern::ALL
        .into_iter()
        .find_map(|p| p.base_of(name).map(|b| (p, b)))
}

/// A mirror property: its pattern and the PID of its base (`None` when the base name has no
/// dictionary definition, as for seven of the eight base-less names of §3.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirrorInfo {
    /// The name pattern.
    pub pattern: MirrorPattern,
    /// The base property, if the dictionary defines it.
    pub base: Option<u32>,
}

/// Every mirror PID of a dictionary with its base (§3.2.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirrorTable {
    by_pid: HashMap<u32, MirrorInfo>,
}

impl MirrorTable {
    /// Finds every mirror label of `labels`.
    #[must_use]
    pub fn from_labels(labels: &LabelIndex) -> Self {
        let by_pid = labels
            .iter()
            .filter_map(|(pid, name)| {
                let (pattern, base) = mirror_base(name)?;
                let base = labels.pid(base).filter(|&b| b != pid);
                Some((pid, MirrorInfo { pattern, base }))
            })
            .collect();
        Self { by_pid }
    }

    /// The mirror entry of `pid`, if it is a mirror.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<MirrorInfo> {
        self.by_pid.get(&pid).copied()
    }

    /// Whether `pid` is a mirror.
    #[must_use]
    pub fn is_mirror(&self, pid: u32) -> bool {
        self.by_pid.contains_key(&pid)
    }

    /// Number of mirror PIDs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_pid.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_pid.is_empty()
    }
}

// ---------------------------------------------------------------------------------------------
// §3.2.2 Class-constant properties and the template store (Decision D1)
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
struct PidAccum {
    count: usize,
    first: PropertyValue,
    equal: bool,
}

#[derive(Debug, Default)]
struct ClassAccum {
    records: usize,
    mirror_pids: BTreeSet<u32>,
    pids: HashMap<u32, PidAccum>,
}

/// First pass of the D1 store (§3.2.2): finds, per root class, the non-mirror properties
/// present in every record with one value. Keeps one value per distinct PID and class, not
/// the sets.
#[derive(Debug)]
pub struct ClassConstantsBuilder<'m> {
    mirrors: &'m MirrorTable,
    classes: BTreeMap<u32, ClassAccum>,
}

impl<'m> ClassConstantsBuilder<'m> {
    /// An empty builder.
    #[must_use]
    pub fn new(mirrors: &'m MirrorTable) -> Self {
        Self {
            mirrors,
            classes: BTreeMap::new(),
        }
    }

    /// Adds one template's property set.
    pub fn add(&mut self, root_class: u32, set: &PropertySet) {
        let acc = self.classes.entry(root_class).or_default();
        acc.records += 1;
        for e in &set.entries {
            if self.mirrors.is_mirror(e.pid) {
                acc.mirror_pids.insert(e.pid);
                continue;
            }
            match acc.pids.get_mut(&e.pid) {
                Some(p) => {
                    p.count += 1;
                    if p.equal && p.first != e.value {
                        p.equal = false;
                    }
                }
                None => {
                    acc.pids.insert(
                        e.pid,
                        PidAccum {
                            count: 1,
                            first: e.value.clone(),
                            equal: true,
                        },
                    );
                }
            }
        }
    }

    /// The classification of every class added.
    #[must_use]
    pub fn finish(self) -> ClassConstants {
        let classes = self
            .classes
            .into_iter()
            .map(|(class, acc)| {
                let mut constants = Vec::new();
                let mut varying = Vec::new();
                for (pid, p) in acc.pids {
                    if p.equal && p.count == acc.records {
                        constants.push(PropertyEntry {
                            pid,
                            value: p.first,
                        });
                    } else {
                        varying.push(pid);
                    }
                }
                constants.sort_by_key(|e| e.pid);
                varying.sort_unstable();
                let info = ClassInfo {
                    records: acc.records,
                    mirror_pids: acc.mirror_pids.into_iter().collect(),
                    constants,
                    varying,
                };
                (class, info)
            })
            .collect();
        ClassConstants { classes }
    }
}

/// One root class of the D1 classification (§3.2.2 table).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    /// Records of the class.
    pub records: usize,
    /// Mirror PIDs that occur in the class, ascending.
    pub mirror_pids: Vec<u32>,
    /// Class-constant properties with their value, by PID.
    pub constants: Vec<PropertyEntry>,
    /// Varying (authored) PIDs, ascending.
    pub varying: Vec<u32>,
}

impl ClassInfo {
    /// The class-constant value of `pid`.
    #[must_use]
    pub fn constant(&self, pid: u32) -> Option<&PropertyValue> {
        self.constants
            .binary_search_by_key(&pid, |e| e.pid)
            .ok()
            .and_then(|i| self.constants.get(i))
            .map(|e| &e.value)
    }

    /// Distinct PIDs of the class (mirrors, constants and varying).
    #[must_use]
    pub fn distinct_pids(&self) -> usize {
        self.mirror_pids.len() + self.constants.len() + self.varying.len()
    }
}

/// The per-class result of [`ClassConstantsBuilder`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassConstants {
    classes: BTreeMap<u32, ClassInfo>,
}

impl ClassConstants {
    /// The classification of `class`.
    #[must_use]
    pub fn class(&self, class: u32) -> Option<&ClassInfo> {
        self.classes.get(&class)
    }

    /// Every class, ascending.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &ClassInfo)> {
        self.classes.iter().map(|(&c, i)| (c, i))
    }
}

/// Limit on mirror-to-base steps of one lookup (a mirror of a mirror); names get shorter at
/// every step, so real data never comes near it.
pub const MAX_MIRROR_STEPS: usize = 8;

/// What the D1 store keeps for one template (§3.2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTemplate {
    /// Root class.
    pub class: u32,
    /// Varying properties and stored mirrors, by PID.
    stored: Vec<PropertyEntry>,
    /// Mirrors the set holds that equal their base (derived, not stored), ascending.
    derived: Vec<u32>,
    /// Class-constant PIDs this set lacks (empty for every set the classification was
    /// built from), ascending.
    missing_constants: Vec<u32>,
    /// How many of `stored` are mirrors.
    stored_mirrors: usize,
}

impl StoredTemplate {
    /// The stored values (varying properties and stored mirrors), by PID.
    #[must_use]
    pub fn stored(&self) -> &[PropertyEntry] {
        &self.stored
    }

    /// The derived mirrors the set holds.
    #[must_use]
    pub fn derived_mirrors(&self) -> &[u32] {
        &self.derived
    }

    /// Number of stored mirrors.
    #[must_use]
    pub fn stored_mirrors(&self) -> usize {
        self.stored_mirrors
    }

    /// Number of stored non-mirror values: the template's authored subset.
    #[must_use]
    pub fn varying(&self) -> usize {
        self.stored.len() - self.stored_mirrors
    }
}

/// Totals of a [`TemplateStore`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoreStats {
    /// Templates stored.
    pub templates: usize,
    /// Stored values (varying properties and stored mirrors).
    pub stored_values: usize,
    /// Stored mirrors.
    pub stored_mirrors: usize,
    /// Derived mirror occurrences.
    pub derived_mirrors: usize,
}

/// The template store of Decision D1 (§3.2.2): class-constant properties once per class,
/// per template only its varying properties and the mirrors that cannot be derived.
///
/// Build it in two passes: [`ClassConstantsBuilder`] over every template of the classes,
/// then [`TemplateStore::insert`] for each ([`TemplateStore::build`] does both over an
/// iterator that can be walked twice).
#[derive(Debug, Clone)]
pub struct TemplateStore {
    mirrors: MirrorTable,
    constants: ClassConstants,
    templates: HashMap<Did, StoredTemplate>,
}

impl TemplateStore {
    /// An empty store over a classification.
    #[must_use]
    pub fn new(mirrors: MirrorTable, constants: ClassConstants) -> Self {
        Self {
            mirrors,
            constants,
            templates: HashMap::new(),
        }
    }

    /// Both passes over `records` (DID, root class, set).
    #[must_use]
    pub fn build<'a, I>(mirrors: MirrorTable, records: I) -> Self
    where
        I: IntoIterator<Item = (Did, u32, &'a PropertySet)> + Clone,
    {
        let mut b = ClassConstantsBuilder::new(&mirrors);
        for (_, class, set) in records.clone() {
            b.add(class, set);
        }
        let constants = b.finish();
        let mut store = Self::new(mirrors, constants);
        for (did, class, set) in records {
            store.insert(did, class, set);
        }
        store
    }

    /// Stores template `did` (a later insert of the same DID replaces it). A mirror is
    /// derived only when its base is in the same set with an equal value; every other
    /// mirror is stored. A class-constant property whose value differs from the constant is
    /// stored too, and a missing one is recorded, so the store gives back any set exactly.
    pub fn insert(&mut self, did: Did, root_class: u32, set: &PropertySet) {
        let values: HashMap<u32, &PropertyValue> =
            set.entries.iter().map(|e| (e.pid, &e.value)).collect();
        let info = self.constants.class(root_class);
        let mut stored = Vec::new();
        let mut derived = Vec::new();
        let mut stored_mirrors = 0;
        for e in &set.entries {
            if let Some(m) = self.mirrors.get(e.pid) {
                let equal = m
                    .base
                    .and_then(|b| values.get(&b))
                    .is_some_and(|&b| *b == e.value);
                if equal {
                    derived.push(e.pid);
                } else {
                    stored.push(e.clone());
                    stored_mirrors += 1;
                }
            } else if info
                .and_then(|i| i.constant(e.pid))
                .is_some_and(|c| *c == e.value)
            {
                // Class-constant: kept once for the class.
            } else {
                stored.push(e.clone());
            }
        }
        let mut missing_constants: Vec<u32> = info
            .map(|i| {
                i.constants
                    .iter()
                    .filter(|c| !values.contains_key(&c.pid))
                    .map(|c| c.pid)
                    .collect()
            })
            .unwrap_or_default();
        stored.sort_by_key(|e| e.pid);
        derived.sort_unstable();
        missing_constants.sort_unstable();
        self.templates.insert(
            did,
            StoredTemplate {
                class: root_class,
                stored,
                derived,
                missing_constants,
                stored_mirrors,
            },
        );
    }

    /// The classification the store was built over.
    #[must_use]
    pub fn constants(&self) -> &ClassConstants {
        &self.constants
    }

    /// The mirror table.
    #[must_use]
    pub fn mirrors(&self) -> &MirrorTable {
        &self.mirrors
    }

    /// What is stored for `did`.
    #[must_use]
    pub fn stored(&self, did: Did) -> Option<&StoredTemplate> {
        self.templates.get(&did)
    }

    /// Number of templates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Every stored DID, in no particular order.
    pub fn dids(&self) -> impl Iterator<Item = Did> + '_ {
        self.templates.keys().copied()
    }

    /// Totals over every template.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let mut s = StoreStats {
            templates: self.templates.len(),
            ..StoreStats::default()
        };
        for t in self.templates.values() {
            s.stored_values += t.stored.len();
            s.stored_mirrors += t.stored_mirrors;
            s.derived_mirrors += t.derived.len();
        }
        s
    }

    /// The D1 lookup of `pid` on template `did` (§3.2.2): the template's own stored value;
    /// else the class-constant value; else, for a derived mirror the set holds, the base's
    /// value by the same lookup; else absent. A base is never answered from a mirror.
    #[must_use]
    pub fn lookup(&self, did: Did, pid: u32) -> Option<&PropertyValue> {
        let t = self.templates.get(&did)?;
        self.lookup_in(t, pid, 0)
    }

    /// [`TemplateStore::lookup`], else the dictionary default (`property-system.md` §2.2).
    #[must_use]
    pub fn lookup_or_default<'a, D: PropertyDefaults + ?Sized>(
        &'a self,
        did: Did,
        pid: u32,
        defaults: &'a D,
    ) -> Option<&'a PropertyValue> {
        self.lookup(did, pid)
            .or_else(|| defaults.default_value(pid))
    }

    fn lookup_in<'s>(
        &'s self,
        t: &'s StoredTemplate,
        pid: u32,
        steps: usize,
    ) -> Option<&'s PropertyValue> {
        if let Ok(i) = t.stored.binary_search_by_key(&pid, |e| e.pid) {
            return t.stored.get(i).map(|e| &e.value);
        }
        if t.missing_constants.binary_search(&pid).is_err()
            && let Some(v) = self.constants.class(t.class).and_then(|c| c.constant(pid))
        {
            return Some(v);
        }
        if t.derived.binary_search(&pid).is_ok() {
            if steps >= MAX_MIRROR_STEPS {
                tracing::warn!(pid, "mirror chain longer than the step limit");
                return None;
            }
            let base = self.mirrors.get(pid)?.base?;
            return self.lookup_in(t, base, steps + 1);
        }
        None
    }

    /// A [`PropertyAccess`] view of template `did` answering by [`TemplateStore::lookup`].
    #[must_use]
    pub fn template(&self, did: Did) -> Option<StoredTemplateView<'_>> {
        let template = self.templates.get(&did)?;
        Some(StoredTemplateView {
            store: self,
            template,
        })
    }

    /// Every property and value of template `did` as its set held them, by PID (CR13).
    #[must_use]
    pub fn reconstruct(&self, did: Did) -> Option<Vec<PropertyEntry>> {
        let t = self.templates.get(&did)?;
        let mut out: Vec<PropertyEntry> = t.stored.clone();
        if let Some(info) = self.constants.class(t.class) {
            out.extend(
                info.constants
                    .iter()
                    .filter(|c| {
                        t.missing_constants.binary_search(&c.pid).is_err()
                            && t.stored.binary_search_by_key(&c.pid, |e| e.pid).is_err()
                    })
                    .cloned(),
            );
        }
        for &pid in &t.derived {
            if let Some(v) = self.lookup_in(t, pid, 0) {
                out.push(PropertyEntry {
                    pid,
                    value: v.clone(),
                });
            }
        }
        out.sort_by_key(|e| e.pid);
        Some(out)
    }
}

/// A template of a [`TemplateStore`], readable like a property set.
#[derive(Debug, Clone, Copy)]
pub struct StoredTemplateView<'s> {
    store: &'s TemplateStore,
    template: &'s StoredTemplate,
}

impl StoredTemplateView<'_> {
    /// What the store keeps for this template.
    #[must_use]
    pub fn stored(&self) -> &StoredTemplate {
        self.template
    }
}

impl PropertyAccess for StoredTemplateView<'_> {
    fn value_of(&self, pid: u32) -> Option<&PropertyValue> {
        self.store.lookup_in(self.template, pid, 0)
    }
}

// ---------------------------------------------------------------------------------------------
// §4 Template view
// ---------------------------------------------------------------------------------------------

/// §4.1 identity and classification.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Identity {
    /// `Name` (unresolved StringInfo).
    pub name: Option<StringInfo>,
    /// `WeenieType` key.
    pub weenie_type: Option<u32>,
    /// `Agent_Species` key.
    pub species: Option<u32>,
    /// `Agent_Subspecies` key.
    pub subspecies: Option<u32>,
    /// `Agent_ShowSubspecies`.
    pub show_subspecies: Option<bool>,
    /// `Agent_Genus` bits.
    pub genus: Option<u64>,
    /// `Agent_Alignment` key.
    pub alignment: Option<u32>,
    /// `Agent_Class` key.
    pub agent_class: Option<u32>,
    /// `Agent_Classification` (classification record, §5.4).
    pub classification: Option<Did>,
    /// `Agent_ClassificationFilter` key.
    pub classification_filter: Option<u32>,
    /// `Quest_MonsterDivision` key.
    pub monster_division: Option<u32>,
    /// `MonsterLevel_ExaminationModStatType` key (examination rank).
    pub examination_rank: Option<u32>,
    /// `Entity_CreationType` key.
    pub creation_type: Option<u32>,
    /// `Agent_NotAlive_Name`.
    pub not_alive_name: Option<StringInfo>,
}

impl Identity {
    /// The one set bit of `Agent_Genus` (CR3); `None` when absent, zero or more than one
    /// bit is set. Its label is key bit + 1 of the genus mapper.
    #[must_use]
    pub fn genus_bit(&self) -> Option<u32> {
        single_bit(self.genus?)
    }
}

/// The index of the only set bit of `bits`.
#[must_use]
pub fn single_bit(bits: u64) -> Option<u32> {
    (bits.count_ones() == 1).then(|| bits.trailing_zeros())
}

/// One `Appearance_KeyInfo` (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceKey {
    /// `Appearance_Key`, raw.
    pub key: Option<u32>,
    /// `Appearance_AprFile` (0x20 record).
    pub apr_file: Option<Did>,
    /// `Appearance_Modifier`, raw.
    pub modifier: Option<u32>,
}

/// §4.2 presentation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Presentation {
    /// `PhysObj` (0x47).
    pub phys_obj: Option<Did>,
    /// `Appearance_ClassList`.
    pub appearances: Vec<AppearanceKey>,
    /// `Appearance_RandomAppearanceType` key.
    pub random_appearance_type: Option<u32>,
    /// `Render_AdjustableScale`.
    pub render_scale: Option<[f32; 3]>,
    /// `Physics_AdjustableScale`.
    pub physics_scale: Option<[f32; 3]>,
    /// `Inventory_Cosmetic_LoadoutSet` (non-null DIDs).
    pub cosmetic_loadouts: Vec<Did>,
    /// `Material_TorsoMaterial`, `_BootMaterial`, `_OffHandMaterial`, `_WeaponMaterial`.
    pub materials: [Option<u32>; 4],
    /// `Combat_AnimationSet` key.
    pub combat_animation_set: Option<u32>,
    /// `Combat_FXSet` key.
    pub combat_fx_set: Option<u32>,
}

/// One `AI_ActionTable_Array_Element` (§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionTableRef {
    /// `AI_ActionTable` (may be the null DID).
    pub table: Did,
    /// `AI_ActionTable_MinLevel` as stored.
    pub min_level: Option<i32>,
    /// `AI_ActionTable_MaxLevel` as stored.
    pub max_level: Option<i32>,
}

impl ActionTableRef {
    /// The lower level bound; stored values ≤ 0 mean "no bound" (§4.3, Reported).
    #[must_use]
    pub fn min_bound(&self) -> Option<i32> {
        self.min_level.filter(|&v| v > 0)
    }

    /// The upper level bound; stored values ≤ 0 mean "no bound".
    #[must_use]
    pub fn max_bound(&self) -> Option<i32> {
        self.max_level.filter(|&v| v > 0)
    }

    /// Whether `level` lies within the bounds.
    #[must_use]
    pub fn contains_level(&self, level: i32) -> bool {
        self.min_bound().is_none_or(|m| level >= m) && self.max_bound().is_none_or(|m| level <= m)
    }
}

/// §4.3 AI settings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiSettings {
    /// `AI_ActionTable_Array`, in stored order.
    pub action_tables: Vec<ActionTableRef>,
    /// `Attitude_Table`.
    pub attitude_table: Option<Did>,
    /// `AI_PerceptionRadius` (world units).
    pub perception_radius: Option<f32>,
    /// `AI_MovementType` key.
    pub movement_type: Option<u32>,
    /// `AI_CallForHelp_Radius`.
    pub call_for_help_radius: Option<f32>,
    /// `AI_CallForHelp_RespondTypes` bits.
    pub call_for_help_respond_types: Option<u64>,
    /// `AI_CallForHelpFilter` (non-null).
    pub call_for_help_filter: Option<Did>,
    /// `AI_Hints` set bits.
    pub hints: Option<Vec<u32>>,
    /// `AI_ColorEventTable` (non-null).
    pub color_event_table: Option<Did>,
    /// `Formation_Group_DataID` (non-null).
    pub formation_group: Option<Did>,
    /// `Formation_Layout_Type` key.
    pub formation_layout: Option<u32>,
    /// `AIPuppet_TargetEvalPackage` (non-null).
    pub puppet_package: Option<Did>,
    /// `AIPuppet_TargetFlags` bits.
    pub puppet_flags: Option<u64>,
    /// `AI_IsPuppetNPC`.
    pub is_puppet: Option<bool>,
    /// `AI_KungFu_StartAsNPC`.
    pub kung_fu_start_as_npc: Option<bool>,
    /// `AI_KungFu_Repeatable`.
    pub kung_fu_repeatable: Option<bool>,
    /// `AI_PhysicallyWanders`.
    pub physically_wanders: Option<bool>,
    /// `AI_WanderingRange`.
    pub wandering_range: Option<f32>,
    /// `AI_WanderingProb`.
    pub wandering_prob: Option<f32>,
    /// `GameEvent_CombatEventTable` (non-null).
    pub combat_event_table: Option<Did>,
    /// `GameEvent_ConjunctionEventTable` (non-null).
    pub conjunction_event_table: Option<Did>,
}

/// §4.4 combat flags and the panel values of §4.8.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CombatFlags {
    /// `Combat_CanBlock`.
    pub can_block: Option<bool>,
    /// `Combat_CanParry`.
    pub can_parry: Option<bool>,
    /// `Combat_CanEvade`.
    pub can_evade: Option<bool>,
    /// `CombatState_CurrentStateImmunities` bits.
    pub state_immunities: Option<u64>,
    /// `Death_NeverSayDie`.
    pub never_say_die: Option<bool>,
    /// `Combat_NotAttackable`.
    pub not_attackable: Option<bool>,
    /// `Health_Starting_Level` (fraction).
    pub health_starting_level: Option<f32>,
    /// `Conjunction_Immune`.
    pub conjunction_immune: Option<bool>,
    /// `Advancement_Level` (absent in mob and NPC templates, §4.8).
    pub advancement_level: Option<i32>,
}

/// §4.5 loot references (non-null DIDs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LootRefs {
    /// `LootGen_TrophyList_Override`.
    pub trophy_list: Option<Did>,
    /// `LootGen_TreasureList_Override`.
    pub treasure_list: Option<Did>,
    /// `LootGen_BarterTrophyList`.
    pub barter_trophy_list: Option<Did>,
    /// `LootGen_ReputationTrophyList`.
    pub reputation_trophy_list: Option<Did>,
    /// `LootGen_FilteredTrophyTable`.
    pub filtered_trophy_table: Option<Did>,
    /// `LootGen_GeneratesTrophies`.
    pub generates_trophies: Option<bool>,
    /// `Loot_IsRemoteLootable`.
    pub remote_lootable: Option<bool>,
    /// `Tapping_OpenTapping`.
    pub open_tapping: Option<bool>,
}

/// One `QuestDispenser_QuestRole` (§4.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestRole {
    /// `QuestDispenser_Quest` (may be null).
    pub quest: Option<Did>,
    /// `QuestDispenser_RoleName`.
    pub role_name: Option<String>,
}

/// §4.6 quest, faction and service roles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Roles {
    /// `QuestDispenser_ActiveQuests`, in stored order.
    pub active_quests: Vec<QuestRole>,
    /// `QuestDispenser_RoleConstraint`.
    pub role_constraint: Option<String>,
    /// `Quest_RelatedQuest` (non-null).
    pub related_quest: Option<Did>,
    /// `Reputation_PositiveFaction` (non-null).
    pub positive_faction: Option<Did>,
    /// `Reputation_NegativeFaction` (non-null).
    pub negative_faction: Option<Did>,
    /// `NPCComment_DIDCommentArray` (non-null DIDs).
    pub comments: Vec<Did>,
    /// `OccupationTitle`.
    pub occupation_title: Option<StringInfo>,
    /// `MapNote_Type` bits.
    pub map_note_type: Option<u64>,
}

/// One `Effect_MonsterStartupEffect_Struct` (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StartupEffect {
    /// `Effect_StartupEffectID`.
    pub effect: Option<Did>,
    /// `Effect_StartupEffectSpellcraft`.
    pub spellcraft: Option<f32>,
    /// `Effect_StartupEffectDamageQualifier`, raw bits or key.
    pub damage_qualifier: Option<u64>,
}

/// §4.7 effects, scripts and use.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectsAndUse {
    /// `Effect_MonsterStartupEffect_Array`.
    pub startup_effects: Vec<StartupEffect>,
    /// `Effect_MonsterInitEffect_Array`.
    pub init_effects: Vec<StartupEffect>,
    /// `Entity_AdditionalScriptTables` (non-null DIDs).
    pub script_tables: Vec<Did>,
    /// `Usage_Action` (non-null).
    pub usage_action: Option<Did>,
    /// `CSM_ActionHandler` (non-null).
    pub csm_action_handler: Option<Did>,
    /// `Induction_ActionDID` (non-null).
    pub induction_action: Option<Did>,
}

/// A typed view of a template's properties (§4). Every field is optional; reading never
/// fails. Read it from a decoded set or from a [`StoredTemplateView`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CreatureTemplate {
    /// §4.1.
    pub identity: Identity,
    /// §4.2.
    pub presentation: Presentation,
    /// §4.3.
    pub ai: AiSettings,
    /// §4.4.
    pub combat: CombatFlags,
    /// §4.5.
    pub loot: LootRefs,
    /// §4.6.
    pub roles: Roles,
    /// §4.7.
    pub effects: EffectsAndUse,
}

fn startup_effects<A: PropertyAccess + ?Sized>(
    set: &A,
    array: Pid,
    p: &TemplatePids,
) -> Vec<StartupEffect> {
    set.structs(array)
        .map(|s| StartupEffect {
            effect: s.did_nonnull(p.startup_effect_id),
            spellcraft: s.float(p.startup_effect_spellcraft),
            damage_qualifier: s
                .get_pid(p.startup_effect_damage_qualifier)
                .and_then(PropertyValue::as_bits),
        })
        .collect()
}

impl CreatureTemplate {
    /// Reads the view from any property source.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &TemplatePids) -> Self {
        let identity = Identity {
            name: string_info_of(set, p.name),
            weenie_type: set.enum_key(p.weenie_type),
            species: set.enum_key(p.species),
            subspecies: set.enum_key(p.subspecies),
            show_subspecies: set.boolean(p.show_subspecies),
            genus: set.bitfield(p.genus),
            alignment: set.enum_key(p.alignment),
            agent_class: set.enum_key(p.agent_class),
            classification: set.did_nonnull(p.classification),
            classification_filter: set.enum_key(p.classification_filter),
            monster_division: set.enum_key(p.monster_division),
            examination_rank: set.enum_key(p.examination_rank),
            creation_type: set.enum_key(p.creation_type),
            not_alive_name: string_info_of(set, p.not_alive_name),
        };
        let presentation = Presentation {
            phys_obj: set.did_nonnull(p.phys_obj),
            appearances: set
                .structs(p.appearance_class_list)
                .map(|s| AppearanceKey {
                    key: s.get_pid(p.appearance_key).and_then(enum_or_int),
                    apr_file: s.did_nonnull(p.appearance_apr_file),
                    modifier: s.get_pid(p.appearance_modifier).and_then(enum_or_int),
                })
                .collect(),
            random_appearance_type: set.enum_key(p.random_appearance_type),
            render_scale: set.vector3(p.render_scale),
            physics_scale: set.vector3(p.physics_scale),
            cosmetic_loadouts: set.dids(p.cosmetic_loadout_set).collect(),
            materials: [
                set.enum_key(p.torso_material),
                set.enum_key(p.boot_material),
                set.enum_key(p.off_hand_material),
                set.enum_key(p.weapon_material),
            ],
            combat_animation_set: set.enum_key(p.combat_animation_set),
            combat_fx_set: set.enum_key(p.combat_fx_set),
        };
        let ai = AiSettings {
            action_tables: set
                .structs(p.action_table_array)
                .map(|s| ActionTableRef {
                    table: s.did(p.action_table).unwrap_or_default(),
                    min_level: s.int(p.action_table_min_level),
                    max_level: s.int(p.action_table_max_level),
                })
                .collect(),
            attitude_table: set.did_nonnull(p.attitude_table),
            perception_radius: set.float(p.perception_radius),
            movement_type: set.enum_key(p.movement_type),
            call_for_help_radius: set.float(p.call_for_help_radius),
            call_for_help_respond_types: set.bitfield(p.call_for_help_respond_types),
            call_for_help_filter: set.did_nonnull(p.call_for_help_filter),
            hints: bits_of(set, p.hints),
            color_event_table: set.did_nonnull(p.color_event_table),
            formation_group: set.did_nonnull(p.formation_group),
            formation_layout: set.enum_key(p.formation_layout),
            puppet_package: set.did_nonnull(p.puppet_package),
            puppet_flags: set.bitfield(p.puppet_flags),
            is_puppet: set.boolean(p.is_puppet),
            kung_fu_start_as_npc: set.boolean(p.kung_fu_start_as_npc),
            kung_fu_repeatable: set.boolean(p.kung_fu_repeatable),
            physically_wanders: set.boolean(p.physically_wanders),
            wandering_range: set.float(p.wandering_range),
            wandering_prob: set.float(p.wandering_prob),
            combat_event_table: set.did_nonnull(p.combat_event_table),
            conjunction_event_table: set.did_nonnull(p.conjunction_event_table),
        };
        let combat = CombatFlags {
            can_block: set.boolean(p.can_block),
            can_parry: set.boolean(p.can_parry),
            can_evade: set.boolean(p.can_evade),
            state_immunities: set.bitfield(p.state_immunities),
            never_say_die: set.boolean(p.never_say_die),
            not_attackable: set.boolean(p.not_attackable),
            health_starting_level: set.float(p.health_starting_level),
            conjunction_immune: set.boolean(p.conjunction_immune),
            advancement_level: set.int(p.advancement_level),
        };
        let loot = LootRefs {
            trophy_list: set.did_nonnull(p.trophy_list),
            treasure_list: set.did_nonnull(p.treasure_list),
            barter_trophy_list: set.did_nonnull(p.barter_trophy_list),
            reputation_trophy_list: set.did_nonnull(p.reputation_trophy_list),
            filtered_trophy_table: set.did_nonnull(p.filtered_trophy_table),
            generates_trophies: set.boolean(p.generates_trophies),
            remote_lootable: set.boolean(p.remote_lootable),
            open_tapping: set.boolean(p.open_tapping),
        };
        let roles = Roles {
            active_quests: set
                .structs(p.active_quests)
                .map(|s| QuestRole {
                    quest: s.did(p.quest),
                    role_name: s.string(p.role_name).map(str::to_owned),
                })
                .collect(),
            role_constraint: set.string(p.role_constraint).map(str::to_owned),
            related_quest: set.did_nonnull(p.related_quest),
            positive_faction: set.did_nonnull(p.positive_faction),
            negative_faction: set.did_nonnull(p.negative_faction),
            comments: set.dids(p.comment_array).collect(),
            occupation_title: string_info_of(set, p.occupation_title),
            map_note_type: set.bitfield(p.map_note_type),
        };
        let effects = EffectsAndUse {
            startup_effects: startup_effects(set, p.startup_effects, p),
            init_effects: startup_effects(set, p.init_effects, p),
            script_tables: set.dids(p.script_tables).collect(),
            usage_action: set.did_nonnull(p.usage_action),
            csm_action_handler: set.did_nonnull(p.csm_action_handler),
            induction_action: set.did_nonnull(p.induction_action),
        };
        Self {
            identity,
            presentation,
            ai,
            combat,
            loot,
            roles,
            effects,
        }
    }

    /// The action tables whose target is in `valid` (§4.3: entries naming a record of
    /// another kind are authoring errors and are ignored; null tables are skipped).
    pub fn valid_action_tables(
        &self,
        mut valid: impl FnMut(Did) -> bool,
    ) -> impl Iterator<Item = &ActionTableRef> {
        self.ai
            .action_tables
            .iter()
            .filter(move |t| !t.table.is_null() && valid(t.table))
    }
}

fn enum_or_int(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => v.as_bits().and_then(|b| u32::try_from(b).ok()),
    }
}

// ---------------------------------------------------------------------------------------------
// §4.8 Panel values
// ---------------------------------------------------------------------------------------------

/// The level the client's creature panel shows: `Advancement_Level` when it is an integer,
/// else 1 (§4.8).
#[must_use]
pub fn displayed_level<A: PropertyAccess + ?Sized>(set: &A, advancement_level: Pid) -> i32 {
    set.int(advancement_level).unwrap_or(1)
}

/// The three immunity indicators of the creature panel (§4.8) from
/// `CombatState_CurrentStateImmunities` and `Conjunction_Immune`: the first when bits 2 and
/// 14 are both set; the second when bit 8 is set, else the value of `Conjunction_Immune`;
/// the third when bit 9 is set.
#[must_use]
pub fn immunity_indicators(immunities: u64, conjunction_immune: bool) -> [bool; 3] {
    let bit = |i: u32| immunities & (1u64 << i) != 0;
    [bit(2) && bit(14), bit(8) || conjunction_immune, bit(9)]
}

// ---------------------------------------------------------------------------------------------
// §5 AI records
// ---------------------------------------------------------------------------------------------

/// One `AIAction_ChainEntry` (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChainEntry {
    /// `AIAction_Skill` (may be null).
    pub skill: Option<Did>,
    /// `AIAction_Recovery` (time; unit OPEN).
    pub recovery: Option<f32>,
}

/// One `AIActionTable_Action` (§5.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiAction {
    /// `AIAction_Chain`.
    pub chain: Vec<ChainEntry>,
    /// `AIAction_Probability` key (§5.2 converts it).
    pub probability: Option<u32>,
    /// `AIAction_Cooldown`.
    pub cooldown: Option<f32>,
    /// `AIAction_InitialCooldown`.
    pub initial_cooldown: Option<f32>,
    /// `AIAction_TargetCooldown`.
    pub target_cooldown: Option<f32>,
    /// `AIAction_CooldownChannel` key.
    pub cooldown_channel: Option<u32>,
    /// `AIAction_RequiredHints` set bits.
    pub required_hints: Option<Vec<u32>>,
    /// `AIAction_DisallowedHints` set bits.
    pub disallowed_hints: Option<Vec<u32>>,
    /// `AIAction_TargetRequiredHints` set bits.
    pub target_required_hints: Option<Vec<u32>>,
    /// `AIAction_TargetDisallowedHints` set bits.
    pub target_disallowed_hints: Option<Vec<u32>>,
    /// `AIAction_CustomTarget` key.
    pub custom_target: Option<u32>,
    /// `AIAction_ColorEventType` key.
    pub color_event_type: Option<u32>,
}

impl AiAction {
    /// Reads one action struct.
    #[must_use]
    pub fn read(s: &PropertyStruct, p: &AiRecordPids) -> Self {
        Self {
            chain: s
                .structs(p.chain)
                .map(|c| ChainEntry {
                    skill: c.did(p.skill),
                    recovery: c.float(p.recovery),
                })
                .collect(),
            probability: s.enum_key(p.probability),
            cooldown: s.float(p.cooldown),
            initial_cooldown: s.float(p.initial_cooldown),
            target_cooldown: s.float(p.target_cooldown),
            cooldown_channel: s.enum_key(p.cooldown_channel),
            required_hints: bits_of(s, p.required_hints),
            disallowed_hints: bits_of(s, p.disallowed_hints),
            target_required_hints: bits_of(s, p.target_required_hints),
            target_disallowed_hints: bits_of(s, p.target_disallowed_hints),
            custom_target: s.enum_key(p.custom_target),
            color_event_type: s.enum_key(p.color_event_type),
        }
    }
}

/// An action table (§5.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActionTable {
    /// `AIActionTable_Actions`, in stored order.
    pub actions: Vec<AiAction>,
}

impl ActionTable {
    /// Reads the table; `AIActionTable_Actions` is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Result<Self> {
        let array = set.require_array(p.actions, "action table")?;
        Ok(Self {
            actions: struct_elements(array)
                .map(|(_, s)| AiAction::read(s, p))
                .collect(),
        })
    }
}

/// The probability table (§5.2): `AIAction_Probability` key → percentage, read from the
/// install (the key labelled `Infinite` maps to −1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProbabilityTable {
    entries: Vec<(u32, f32)>,
}

impl ProbabilityTable {
    /// Reads `AIActionProbability_Array` (required). Elements without both fields are
    /// skipped; for a key listed twice the first value wins.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Result<Self> {
        let array = set.require_array(p.probability_array, "probability table")?;
        let mut entries: Vec<(u32, f32)> = Vec::new();
        for (_, s) in struct_elements(array) {
            if let (Some(k), Some(v)) =
                (s.enum_key(p.probability_type), s.float(p.probability_value))
                && !entries.iter().any(|(e, _)| *e == k)
            {
                entries.push((k, v));
            }
        }
        Ok(Self { entries })
    }

    /// The percentage of key `key`.
    #[must_use]
    pub fn percentage(&self, key: u32) -> Option<f32> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    /// (key, percentage) in stored order.
    #[must_use]
    pub fn entries(&self) -> &[(u32, f32)] {
        &self.entries
    }
}

/// One `ClassificationFilter_FilterException` (§5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterException {
    /// Alignment key.
    pub alignment: Option<u32>,
    /// Genus bits.
    pub genus: Option<u64>,
    /// Species key.
    pub species: Option<u32>,
    /// Subspecies key.
    pub subspecies: Option<u32>,
}

/// One `ClassificationFilter_Filter` (§5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassificationFilter {
    /// `ClassificationFilter_Filter_Type` key (compared with `Agent_ClassificationFilter`).
    pub filter_type: Option<u32>,
    /// `ClassificationFilter_Filter_Default`.
    pub default: Option<bool>,
    /// `ClassificationFilter_FilterExceptionList`.
    pub exceptions: Vec<FilterException>,
}

fn read_filters(
    array: &PropertyArray,
    p: &AiRecordPids,
    x: &FilterExceptionPids,
) -> Vec<ClassificationFilter> {
    struct_elements(array)
        .map(|(_, s)| ClassificationFilter {
            filter_type: s.enum_key(p.filter_type),
            default: s.boolean(p.filter_default),
            exceptions: s
                .structs(p.exception_list)
                .map(|e| FilterException {
                    alignment: e.enum_key(x.alignment),
                    genus: e.bitfield(x.genus),
                    species: e.enum_key(x.species),
                    subspecies: e.enum_key(x.subspecies),
                })
                .collect(),
        })
        .collect()
}

/// An attitude table (§5.3). Either list may be absent; two records hold neither.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttitudeTable {
    /// `Attitude_FriendlyFilterList`.
    pub friendly: Option<Vec<ClassificationFilter>>,
    /// `Attitude_AggressiveFilterList`.
    pub aggressive: Option<Vec<ClassificationFilter>>,
}

impl AttitudeTable {
    /// Reads the table (never fails: an empty set gives an empty table).
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(
        set: &A,
        p: &AiRecordPids,
        x: &FilterExceptionPids,
    ) -> Self {
        Self {
            friendly: set.array(p.friendly_filters).map(|a| read_filters(a, p, x)),
            aggressive: set
                .array(p.aggressive_filters)
                .map(|a| read_filters(a, p, x)),
        }
    }
}

/// A classification-filter list record (§5.3, `ClassificationFilter_FilterList`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassificationFilterList {
    /// The filters.
    pub filters: Vec<ClassificationFilter>,
}

impl ClassificationFilterList {
    /// Reads the list; `ClassificationFilter_FilterList` is required.
    pub fn read<A: PropertyAccess + ?Sized>(
        set: &A,
        p: &AiRecordPids,
        x: &FilterExceptionPids,
    ) -> Result<Self> {
        let array = set.require_array(p.filter_list, "classification filter list")?;
        Ok(Self {
            filters: read_filters(array, p, x),
        })
    }
}

/// A classification record (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Classification {
    /// `Classification_Species` key.
    pub species: Option<u32>,
    /// `Classification_Genus` bits.
    pub genus: Option<u64>,
    /// `Classification_Alignment` key.
    pub alignment: Option<u32>,
}

impl Classification {
    /// Reads the record.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Self {
        Self {
            species: set.enum_key(p.classification_species),
            genus: set.bitfield(p.classification_genus),
            alignment: set.enum_key(p.classification_alignment),
        }
    }

    /// The fields on which a template disagrees with this record (CR2; a disagreement is
    /// a data error to report, §5.4): any of `"species"`, `"genus"`, `"alignment"`.
    #[must_use]
    pub fn disagreements(&self, identity: &Identity) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.species != identity.species {
            out.push("species");
        }
        if self.genus != identity.genus {
            out.push("genus");
        }
        if self.alignment != identity.alignment {
            out.push("alignment");
        }
        out
    }
}

/// One `AIColorEventTable_Event_DataEntry` (§5.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColorEventLine {
    /// `AIColorEventTable_Event_String`.
    pub text: Option<StringInfo>,
    /// `AIColorEventTable_Event_SoundID` (non-null, 0x2A).
    pub sound: Option<Did>,
}

/// One `AIColorEventTable_Event` (§5.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColorEvent {
    /// `AIColorEventTable_Event_Type` key.
    pub event_type: Option<u32>,
    /// `AIColorEventTable_Event_Probability`.
    pub probability: Option<f32>,
    /// `AIColorEventTable_Event_SayType` key.
    pub say_type: Option<u32>,
    /// `AIColorEventTable_Event_SoundChannel`.
    pub sound_channel: Option<i32>,
    /// `AIColorEventTable_Event_Data`.
    pub lines: Vec<ColorEventLine>,
}

impl ColorEvent {
    fn read(s: &PropertyStruct, p: &AiRecordPids) -> Self {
        Self {
            event_type: s.enum_key(p.event_type),
            probability: s.float(p.event_probability),
            say_type: s.enum_key(p.event_say_type),
            sound_channel: s.int(p.event_sound_channel),
            lines: s
                .structs(p.event_data)
                .map(|d| ColorEventLine {
                    text: string_info_of(d, p.event_string),
                    sound: d.did_nonnull(p.event_sound),
                })
                .collect(),
        }
    }
}

/// A colour-event table (§5.5) with Decision D7: a top-level `AIColorEventTable_Event` is
/// kept as one more event, besides the array.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColorEventTable {
    /// `AIColorEventTable_Events`, in stored order.
    pub events: Vec<ColorEvent>,
    /// A top-level `AIColorEventTable_Event`, if the set holds one (D7).
    pub top_level: Option<ColorEvent>,
}

impl ColorEventTable {
    /// Reads the table; `AIColorEventTable_Events` is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Result<Self> {
        let array = set.require_array(p.color_events, "colour-event table")?;
        Ok(Self {
            events: struct_elements(array)
                .map(|(_, s)| ColorEvent::read(s, p))
                .collect(),
            top_level: set.structure(p.color_event).map(|s| ColorEvent::read(s, p)),
        })
    }

    /// Every event: the array events, then the top-level one (D7).
    pub fn all_events(&self) -> impl Iterator<Item = &ColorEvent> {
        self.events.iter().chain(self.top_level.as_ref())
    }

    /// Whether the array holds exactly one event per key in `keys` and nothing else (CR15
    /// with keys 1–22 of the event-type mapper).
    #[must_use]
    pub fn covers_exactly(&self, keys: impl IntoIterator<Item = u32>) -> bool {
        let want: BTreeSet<u32> = keys.into_iter().collect();
        let mut have = BTreeSet::new();
        for e in &self.events {
            match e.event_type {
                Some(k) if have.insert(k) => {}
                _ => return false,
            }
        }
        have == want
    }
}

/// A formation (§5.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formation {
    /// Every `Formation_Follower` DID under `Formation_Group`, in stored order (null DIDs
    /// skipped).
    pub followers: Vec<Did>,
}

impl Formation {
    /// Reads the formation; `Formation_Group` is required and is an array whose elements are
    /// bare `Formation_Follower` DIDs (§5.6, C33). Null DIDs and elements of another label or
    /// type are skipped.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Result<Self> {
        let group = set.require_array(p.formation_group, "formation")?;
        let followers = group
            .elements
            .iter()
            .filter(|e| p.formation_follower.is(e.pid))
            .filter_map(ValueExt::did_nonnull)
            .collect();
        Ok(Self { followers })
    }
}

/// One `Conjunction_Role_ContributionToActionMap` entry (§5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoleContribution {
    /// `Conjunction_Role_ActionSkill`.
    pub action_skill: Option<Did>,
    /// `Conjunction_PvP_Role_ActionSkill`.
    pub pvp_action_skill: Option<Did>,
    /// `Conjunction_Role_ContributionSkill`.
    pub contribution_skill: Option<Did>,
}

/// One `Conjunction_Role` (§5.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConjunctionRole {
    /// Every contribution entry under the role, in stored order.
    pub contributions: Vec<RoleContribution>,
}

/// A conjunction record (§5.7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Conjunction {
    /// `Conjunction_Name`.
    pub name: Option<StringInfo>,
    /// `Conjunction_Description`.
    pub description: Option<StringInfo>,
    /// `Conjunction_Category` key.
    pub category: Option<u32>,
    /// `Conjunction_Flags` bits.
    pub flags: Option<u64>,
    /// `Conjunction_Icon_Small`, `_Medium`, `_Large` (non-null).
    pub icons: [Option<Did>; 3],
    /// `Conjunction_RoleList` elements, in stored order.
    pub roles: Vec<ConjunctionRole>,
}

impl Conjunction {
    /// Reads the record (never fails). `Conjunction_RoleList` is an array of
    /// `Conjunction_Role` elements, each itself an array whose elements are the
    /// `Conjunction_Role_ContributionToActionMap` structs (§5.7, C33); elements of another
    /// label or type are skipped.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Self {
        let roles = set
            .elements(p.role_list)
            .iter()
            .map(|role| ConjunctionRole {
                contributions: role
                    .value
                    .children()
                    .iter()
                    .filter(|c| p.contribution.is(c.pid))
                    .filter_map(|c| c.value.as_struct())
                    .map(|s| RoleContribution {
                        action_skill: s.did(p.role_action_skill),
                        pvp_action_skill: s.did(p.pvp_role_action_skill),
                        contribution_skill: s.did(p.contribution_skill),
                    })
                    .collect(),
            })
            .collect();
        Self {
            name: string_info_of(set, p.conjunction_name),
            description: string_info_of(set, p.conjunction_description),
            category: set.enum_key(p.conjunction_category),
            flags: set.bitfield(p.conjunction_flags),
            icons: [
                set.did_nonnull(p.icon_small),
                set.did_nonnull(p.icon_medium),
                set.did_nonnull(p.icon_large),
            ],
            roles,
        }
    }
}

/// A puppet target package (§5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PuppetPackage {
    /// `AIPuppet_TargetRequestQueue_Type` key.
    pub queue_type: Option<u32>,
}

impl PuppetPackage {
    /// Reads the record.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &AiRecordPids) -> Self {
        Self {
            queue_type: set.enum_key(p.puppet_queue_type),
        }
    }
}

/// Kinds of records this module decodes besides templates, recognised by the property that
/// characterises each (§5, §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CreatureRecordKind {
    /// `AIActionTable_Actions` (§5.1).
    ActionTable,
    /// `AIActionProbability_Array` (§5.2).
    ProbabilityTable,
    /// `Attitude_FriendlyFilterList` or `Attitude_AggressiveFilterList` (§5.3).
    AttitudeTable,
    /// `ClassificationFilter_FilterList` (§5.3).
    FilterList,
    /// `Classification_Species`, `_Genus` and `_Alignment` (§5.4).
    Classification,
    /// `AIColorEventTable_Events` (§5.5).
    ColorEventTable,
    /// `Formation_Group` (§5.6).
    Formation,
    /// `Conjunction_RoleList` (§5.7).
    Conjunction,
    /// `AIPuppet_TargetRequestQueue_Type` (§5.8).
    PuppetPackage,
    /// `GeneratorProfile_Profile` (§6.5).
    GeneratorProfile,
}

impl CreatureRecordKind {
    /// Every kind.
    pub const ALL: [Self; 10] = [
        Self::ActionTable,
        Self::ProbabilityTable,
        Self::AttitudeTable,
        Self::FilterList,
        Self::Classification,
        Self::ColorEventTable,
        Self::Formation,
        Self::Conjunction,
        Self::PuppetPackage,
        Self::GeneratorProfile,
    ];

    /// The kind whose characteristic properties `set` holds (the first in [`Self::ALL`]
    /// order). Records with an empty set (two attitude tables) are not recognised here; see
    /// [`RecordKindIndex`].
    #[must_use]
    pub fn of_set<A: PropertyAccess + ?Sized>(set: &A, pids: &CreaturePids) -> Option<Self> {
        let a = &pids.ai;
        Self::ALL.into_iter().find(|k| match k {
            Self::ActionTable => set.has(a.actions),
            Self::ProbabilityTable => set.has(a.probability_array),
            Self::AttitudeTable => set.has(a.friendly_filters) || set.has(a.aggressive_filters),
            Self::FilterList => set.has(a.filter_list),
            Self::Classification => {
                set.has(a.classification_species)
                    && set.has(a.classification_genus)
                    && set.has(a.classification_alignment)
            }
            Self::ColorEventTable => set.has(a.color_events),
            Self::Formation => set.has(a.formation_group),
            Self::Conjunction => set.has(a.role_list),
            Self::PuppetPackage => set.has(a.puppet_queue_type),
            Self::GeneratorProfile => set.has(pids.profile.profile),
        })
    }
}

/// Root classes per record kind, learned from the records [`CreatureRecordKind::of_set`]
/// recognises. A record whose own set is not recognised (for example an empty attitude
/// table) takes the kind of its class when exactly one kind was seen in that class.
#[derive(Debug, Clone, Default)]
pub struct RecordKindIndex {
    by_class: BTreeMap<u32, BTreeMap<CreatureRecordKind, usize>>,
}

impl RecordKindIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one record of `root_class` recognised as `kind`.
    pub fn add(&mut self, root_class: u32, kind: CreatureRecordKind) {
        *self
            .by_class
            .entry(root_class)
            .or_default()
            .entry(kind)
            .or_default() += 1;
    }

    /// The single kind seen in `root_class`.
    #[must_use]
    pub fn kind_of_class(&self, root_class: u32) -> Option<CreatureRecordKind> {
        let m = self.by_class.get(&root_class)?;
        if m.len() == 1 {
            m.keys().next().copied()
        } else {
            None
        }
    }

    /// The classes where `kind` was seen, ascending, with their counts.
    #[must_use]
    pub fn classes_of(&self, kind: CreatureRecordKind) -> Vec<(u32, usize)> {
        self.by_class
            .iter()
            .filter_map(|(&c, m)| m.get(&kind).map(|&n| (c, n)))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// §6.5 Generator profiles
// ---------------------------------------------------------------------------------------------

/// Deepest nesting of `OneOf` and `PositionSetEntry` values the profile reader follows
/// (a resource limit; real profiles nest 3 levels below the profile array).
pub const MAX_PROFILE_TREE_DEPTH: usize = 16;

/// A `GeneratorProfile_Repeat` (§6.5). Absent bounds take the dictionary default (1 in
/// this install) through [`EvalOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Repeat {
    /// `GeneratorProfile_RepeatMinTimes`.
    pub min: Option<i32>,
    /// `GeneratorProfile_RepeatMaxTimes`.
    pub max: Option<i32>,
}

impl Repeat {
    /// (min, max) with `default` for absent bounds.
    #[must_use]
    pub fn bounds(self, default: i32) -> (i32, i32) {
        (self.min.unwrap_or(default), self.max.unwrap_or(default))
    }
}

/// A placement (§6.5): an exact offset or a random annulus, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// `GeneratorProfile_ExactPlacement` (absent components 0).
    Exact {
        /// X, Y, Z offset.
        offset: [f32; 3],
    },
    /// `GeneratorProfile_RandomPlacement` (absent distances 0).
    Random {
        /// `…MinDist`.
        min_dist: f32,
        /// `…MaxDist`.
        max_dist: f32,
    },
}

/// A modifier at element 0 of a `OneOf` (§6.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Modifier {
    /// `GeneratorProfile_Repeat`.
    Repeat(Repeat),
    /// `GeneratorProfile_ExactPlacement` or `GeneratorProfile_RandomPlacement`.
    Placement(Placement),
}

/// What an entry produces (§6.5: exactly one target per entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryTarget {
    /// `GeneratorProfile_WSLEntity`: a template or other object.
    Object(Did),
    /// `GeneratorProfile_ProfileDefinition`: another profile.
    Profile(Did),
}

/// A `GeneratorProfile_Entry` (§6.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProfileEntry {
    /// `GeneratorProfile_WSLEntity` (non-null).
    pub object: Option<Did>,
    /// `GeneratorProfile_ProfileDefinition` (non-null).
    pub profile: Option<Did>,
    /// `GeneratorProfile_Weight`.
    pub weight: Option<i32>,
    /// `GeneratorProfile_Probability` (per mille under D3).
    pub probability: Option<i32>,
    /// `GeneratorProfile_Repeat`.
    pub repeat: Option<Repeat>,
    /// `GeneratorProfile_ExactPlacement`, else `GeneratorProfile_RandomPlacement`.
    pub placement: Option<Placement>,
    /// Whether the entry holds both placement kinds (only the exact one is used).
    pub both_placements: bool,
    /// `GeneratorProfile_Properties` elements, in stored order.
    pub properties: Vec<PropertyEntry>,
}

impl ProfileEntry {
    /// The entry's target: the object when present, else the profile.
    #[must_use]
    pub fn target(&self) -> Option<EntryTarget> {
        self.object
            .map(EntryTarget::Object)
            .or(self.profile.map(EntryTarget::Profile))
    }

    /// Number of targets (1 in every entry of this install).
    #[must_use]
    pub fn target_count(&self) -> usize {
        usize::from(self.object.is_some()) + usize::from(self.profile.is_some())
    }

    /// Whether C17 removes the entry: a weight or probability of 0 or less.
    #[must_use]
    pub fn is_removed(&self) -> bool {
        self.weight.is_some_and(|w| w <= 0) || self.probability.is_some_and(|p| p <= 0)
    }
}

/// A `GeneratorProfile_PositionSetEntry` (§6.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PositionSetEntry {
    /// `GeneratorProfile_PositionSetID` key.
    pub position_set: Option<u32>,
    /// `GeneratorProfile_Weight` (when it is a choice).
    pub weight: Option<i32>,
    /// Its `GeneratorProfile_Entry`.
    pub entry: Option<ProfileEntry>,
    /// Its `GeneratorProfile_OneOf`.
    pub one_of: Option<OneOf>,
}

/// One choice of a `OneOf` (§6.5).
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// A `GeneratorProfile_Entry`.
    Entry(ProfileEntry),
    /// A `GeneratorProfile_PositionSetEntry`.
    PositionSet(PositionSetEntry),
}

impl Choice {
    /// The choice's `GeneratorProfile_Weight`.
    #[must_use]
    pub fn weight(&self) -> Option<i32> {
        match self {
            Self::Entry(e) => e.weight,
            Self::PositionSet(p) => p.weight,
        }
    }
}

/// A `GeneratorProfile_OneOf` (§6.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OneOf {
    /// The modifier at element 0, if any.
    pub modifier: Option<Modifier>,
    /// The choices, in stored order.
    pub choices: Vec<Choice>,
    /// Modifiers found after element 0 or after another modifier (CR9 violations; ignored).
    pub stray_modifiers: usize,
    /// Elements of any other kind (ignored).
    pub other_elements: usize,
}

/// One element of `GeneratorProfile_Profile` (§6.5).
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileElement {
    /// `GeneratorProfile_RegenType` key.
    RegenType(u32),
    /// An unconditional `GeneratorProfile_Entry`.
    Entry(ProfileEntry),
    /// A `GeneratorProfile_OneOf`.
    OneOf(OneOf),
    /// A `GeneratorProfile_PositionSetEntry`.
    PositionSet(PositionSetEntry),
    /// An element of another kind, kept by PID.
    Other {
        /// The element's PID.
        pid: u32,
    },
}

/// A generator profile (§6.5): the elements of `GeneratorProfile_Profile`, in stored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeneratorProfile {
    /// The elements.
    pub elements: Vec<ProfileElement>,
}

/// A deviation from the §6.5 grammar (CR9, CR10, CR14), found by
/// [`GeneratorProfile::issues`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileIssue {
    /// An entry with no target or two (§6.5).
    EntryTargets(usize),
    /// An entry with both placement kinds.
    EntryPlacements,
    /// A modifier after element 0 or a second modifier in a `OneOf` (CR9).
    StrayModifier,
    /// An `Entry` in a `OneOf` without `GeneratorProfile_Weight` ≥ 1 (CR9).
    ChoiceWeight,
    /// `RepeatMinTimes` > `RepeatMaxTimes` (CR10).
    RepeatBounds,
    /// `RandomPlacementMinDist` > `…MaxDist` (CR10).
    PlacementBounds,
    /// A `OneOf` directly in a profile that mixes `Entry` and `PositionSetEntry` choices or
    /// has a modifier with `PositionSetEntry` choices (CR14).
    MixedOneOf,
    /// A `OneOf` of `PositionSetEntry` choices directly in a profile that names more than one
    /// position set (CR14).
    SeveralPositionSets,
    /// A `PositionSetEntry` choice whose `OneOf` lacks a `Repeat` with minimum = maximum at
    /// element 0 (CR14).
    ChoiceRepeat,
    /// A `PositionSetEntry` with neither an `Entry` nor a `OneOf`, or with both.
    PositionSetBody,
}

struct ProfileReader<'p> {
    p: &'p ProfilePids,
    f: &'p ProfileFieldPids,
}

const PROFILE: &str = "generator profile";

impl ProfileReader<'_> {
    fn depth(depth: usize) -> Result<usize> {
        let next = depth + 1;
        if next > MAX_PROFILE_TREE_DEPTH {
            return Err(FormatError::DepthLimit {
                offset: 0,
                limit: MAX_PROFILE_TREE_DEPTH,
            });
        }
        Ok(next)
    }

    fn as_struct<'v>(
        &self,
        e: &'v PropertyEntry,
        label: &'static str,
    ) -> Result<&'v PropertyStruct> {
        e.value
            .as_struct()
            .ok_or(FormatError::PropertyTypeMismatch {
                what: PROFILE,
                label,
                expected: "struct",
                found: e.value.property_type(),
            })
    }

    fn as_array<'v>(&self, e: &'v PropertyEntry, label: &'static str) -> Result<&'v PropertyArray> {
        e.value.as_array().ok_or(FormatError::PropertyTypeMismatch {
            what: PROFILE,
            label,
            expected: "array",
            found: e.value.property_type(),
        })
    }

    fn repeat(&self, s: &PropertyStruct) -> Repeat {
        Repeat {
            min: s.int(self.f.repeat_min),
            max: s.int(self.f.repeat_max),
        }
    }

    fn exact(&self, s: &PropertyStruct) -> Placement {
        let [x, y, z] = self.f.exact;
        Placement::Exact {
            offset: [
                s.float(x).unwrap_or(0.0),
                s.float(y).unwrap_or(0.0),
                s.float(z).unwrap_or(0.0),
            ],
        }
    }

    fn random(&self, s: &PropertyStruct) -> Placement {
        Placement::Random {
            min_dist: s.float(self.f.random_min).unwrap_or(0.0),
            max_dist: s.float(self.f.random_max).unwrap_or(0.0),
        }
    }

    /// The modifier an element is, if it is one.
    fn modifier(&self, e: &PropertyEntry) -> Result<Option<Modifier>> {
        let p = self.p;
        Ok(if p.repeat.is(e.pid) {
            Some(Modifier::Repeat(
                self.repeat(self.as_struct(e, p.repeat.label())?),
            ))
        } else if p.exact_placement.is(e.pid) {
            Some(Modifier::Placement(
                self.exact(self.as_struct(e, p.exact_placement.label())?),
            ))
        } else if p.random_placement.is(e.pid) {
            Some(Modifier::Placement(
                self.random(self.as_struct(e, p.random_placement.label())?),
            ))
        } else {
            None
        })
    }

    fn entry(&self, s: &PropertyStruct) -> ProfileEntry {
        let p = self.p;
        let exact = s.structure(p.exact_placement).map(|x| self.exact(x));
        let random = s.structure(p.random_placement).map(|x| self.random(x));
        ProfileEntry {
            object: s.did_nonnull(p.wsl_entity),
            profile: s.did_nonnull(p.profile_definition),
            weight: s.int(p.weight),
            probability: s.int(p.probability),
            repeat: s.structure(p.repeat).map(|r| self.repeat(r)),
            placement: exact.or(random),
            both_placements: exact.is_some() && random.is_some(),
            properties: s.elements(p.properties).to_vec(),
        }
    }

    fn position_set_entry(&self, s: &PropertyStruct, depth: usize) -> Result<PositionSetEntry> {
        let p = self.p;
        let depth = Self::depth(depth)?;
        let one_of = match s.fields.iter().find(|f| p.one_of.is(f.pid)) {
            Some(f) => Some(self.one_of(self.as_array(f, p.one_of.label())?, depth)?),
            None => None,
        };
        Ok(PositionSetEntry {
            position_set: s.enum_key(p.position_set_id),
            weight: s.int(p.weight),
            entry: s.structure(p.entry).map(|e| self.entry(e)),
            one_of,
        })
    }

    fn one_of(&self, a: &PropertyArray, depth: usize) -> Result<OneOf> {
        let p = self.p;
        let depth = Self::depth(depth)?;
        let mut out = OneOf::default();
        for (i, e) in a.elements.iter().enumerate() {
            if let Some(m) = self.modifier(e)? {
                if i == 0 {
                    out.modifier = Some(m);
                } else {
                    out.stray_modifiers += 1;
                }
            } else if p.entry.is(e.pid) {
                let s = self.as_struct(e, p.entry.label())?;
                out.choices.push(Choice::Entry(self.entry(s)));
            } else if p.position_set_entry.is(e.pid) {
                let s = self.as_struct(e, p.position_set_entry.label())?;
                out.choices
                    .push(Choice::PositionSet(self.position_set_entry(s, depth)?));
            } else {
                out.other_elements += 1;
            }
        }
        Ok(out)
    }
}

impl GeneratorProfile {
    /// Reads a profile set; `GeneratorProfile_Profile` is required. Fails with
    /// [`FormatError::PropertyTypeMismatch`] when a known element has another type than
    /// the grammar gives it, and with [`FormatError::DepthLimit`] past
    /// [`MAX_PROFILE_TREE_DEPTH`].
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, pids: &CreaturePids) -> Result<Self> {
        let p = &pids.profile;
        let r = ProfileReader {
            p,
            f: &pids.profile_fields,
        };
        let array = set.require_array(p.profile, PROFILE)?;
        let mut elements = Vec::new();
        for e in &array.elements {
            let el = if p.regen_type.is(e.pid) {
                match e.value.enum_key() {
                    Some(k) => ProfileElement::RegenType(k),
                    None => ProfileElement::Other { pid: e.pid },
                }
            } else if p.entry.is(e.pid) {
                ProfileElement::Entry(r.entry(r.as_struct(e, p.entry.label())?))
            } else if p.one_of.is(e.pid) {
                ProfileElement::OneOf(r.one_of(r.as_array(e, p.one_of.label())?, 0)?)
            } else if p.position_set_entry.is(e.pid) {
                let s = r.as_struct(e, p.position_set_entry.label())?;
                ProfileElement::PositionSet(r.position_set_entry(s, 0)?)
            } else {
                ProfileElement::Other { pid: e.pid }
            };
            elements.push(el);
        }
        Ok(Self { elements })
    }

    /// The `GeneratorProfile_RegenType` key, if the profile has one.
    #[must_use]
    pub fn regen_type(&self) -> Option<u32> {
        self.elements.iter().find_map(|e| match e {
            ProfileElement::RegenType(k) => Some(*k),
            _ => None,
        })
    }

    /// Every entry of the tree, depth first in stored order, with whether it sits inside a
    /// `OneOf` (as a choice).
    #[must_use]
    pub fn entries(&self) -> Vec<(&ProfileEntry, bool)> {
        fn one_of<'a>(o: &'a OneOf, out: &mut Vec<(&'a ProfileEntry, bool)>) {
            for c in &o.choices {
                match c {
                    Choice::Entry(e) => out.push((e, true)),
                    Choice::PositionSet(p) => pse(p, out),
                }
            }
        }
        fn pse<'a>(p: &'a PositionSetEntry, out: &mut Vec<(&'a ProfileEntry, bool)>) {
            if let Some(e) = &p.entry {
                out.push((e, false));
            }
            if let Some(o) = &p.one_of {
                one_of(o, out);
            }
        }
        let mut out = Vec::new();
        for el in &self.elements {
            match el {
                ProfileElement::Entry(e) => out.push((e, false)),
                ProfileElement::OneOf(o) => one_of(o, &mut out),
                ProfileElement::PositionSet(p) => pse(p, &mut out),
                ProfileElement::RegenType(_) | ProfileElement::Other { .. } => {}
            }
        }
        out
    }

    /// Every deviation from the §6.5 grammar (CR9, CR10, CR14), depth first.
    #[must_use]
    pub fn issues(&self) -> Vec<ProfileIssue> {
        let mut out = Vec::new();
        for el in &self.elements {
            match el {
                ProfileElement::Entry(e) => entry_issues(e, false, &mut out),
                ProfileElement::OneOf(o) => {
                    one_of_issues(o, &mut out);
                    top_one_of_issues(o, &mut out);
                }
                ProfileElement::PositionSet(p) => pse_issues(p, &mut out),
                ProfileElement::RegenType(_) | ProfileElement::Other { .. } => {}
            }
        }
        out
    }
}

fn repeat_issue(r: Repeat, out: &mut Vec<ProfileIssue>) {
    if let (Some(a), Some(b)) = (r.min, r.max)
        && a > b
    {
        out.push(ProfileIssue::RepeatBounds);
    }
}

fn placement_issue(p: Placement, out: &mut Vec<ProfileIssue>) {
    if let Placement::Random { min_dist, max_dist } = p
        && min_dist > max_dist
    {
        out.push(ProfileIssue::PlacementBounds);
    }
}

fn entry_issues(e: &ProfileEntry, in_one_of: bool, out: &mut Vec<ProfileIssue>) {
    if e.target_count() != 1 {
        out.push(ProfileIssue::EntryTargets(e.target_count()));
    }
    if e.both_placements {
        out.push(ProfileIssue::EntryPlacements);
    }
    if in_one_of && !e.weight.is_some_and(|w| w >= 1) {
        out.push(ProfileIssue::ChoiceWeight);
    }
    if let Some(r) = e.repeat {
        repeat_issue(r, out);
    }
    if let Some(p) = e.placement {
        placement_issue(p, out);
    }
}

fn pse_issues(p: &PositionSetEntry, out: &mut Vec<ProfileIssue>) {
    if p.entry.is_some() == p.one_of.is_some() {
        out.push(ProfileIssue::PositionSetBody);
    }
    if let Some(e) = &p.entry {
        entry_issues(e, false, out);
    }
    if let Some(o) = &p.one_of {
        one_of_issues(o, out);
    }
}

fn one_of_issues(o: &OneOf, out: &mut Vec<ProfileIssue>) {
    for _ in 0..o.stray_modifiers {
        out.push(ProfileIssue::StrayModifier);
    }
    match o.modifier {
        Some(Modifier::Repeat(r)) => repeat_issue(r, out),
        Some(Modifier::Placement(p)) => placement_issue(p, out),
        None => {}
    }
    for c in &o.choices {
        match c {
            Choice::Entry(e) => entry_issues(e, true, out),
            Choice::PositionSet(p) => pse_issues(p, out),
        }
    }
}

/// CR14 for a `OneOf` directly in a profile.
fn top_one_of_issues(o: &OneOf, out: &mut Vec<ProfileIssue>) {
    let entries = o
        .choices
        .iter()
        .filter(|c| matches!(c, Choice::Entry(_)))
        .count();
    let sets: Vec<&PositionSetEntry> = o
        .choices
        .iter()
        .filter_map(|c| match c {
            Choice::PositionSet(p) => Some(p),
            Choice::Entry(_) => None,
        })
        .collect();
    if sets.is_empty() {
        return;
    }
    if entries > 0 || o.modifier.is_some() {
        out.push(ProfileIssue::MixedOneOf);
    }
    let named: BTreeSet<Option<u32>> = sets.iter().map(|p| p.position_set).collect();
    if named.len() > 1 {
        out.push(ProfileIssue::SeveralPositionSets);
    }
    for p in sets {
        let fixed = p.one_of.as_ref().is_some_and(|o| {
            matches!(o.modifier, Some(Modifier::Repeat(r)) if r.min.is_some() && r.min == r.max)
        });
        if !fixed {
            out.push(ProfileIssue::ChoiceRepeat);
        }
    }
}

/// Access to decoded profiles by DID, for [`resolve_candidates`] and [`evaluate_profile`].
pub trait ProfileSource {
    /// The profile `did`, if known.
    fn profile(&self, did: Did) -> Option<&GeneratorProfile>;
}

impl<S: std::hash::BuildHasher> ProfileSource for HashMap<Did, GeneratorProfile, S> {
    fn profile(&self, did: Did) -> Option<&GeneratorProfile> {
        self.get(&did)
    }
}

impl ProfileSource for BTreeMap<Did, GeneratorProfile> {
    fn profile(&self, did: Did) -> Option<&GeneratorProfile> {
        self.get(&did)
    }
}

impl<P: ProfileSource + ?Sized> ProfileSource for &P {
    fn profile(&self, did: Did) -> Option<&GeneratorProfile> {
        (**self).profile(did)
    }
}

// ---------------------------------------------------------------------------------------------
// §6.7 Static resolution
// ---------------------------------------------------------------------------------------------

/// Deepest profile nesting followed through `ProfileDefinition` (§6.7 step 6).
pub const MAX_PROFILE_DEPTH: usize = 8;

/// Upper bound on the elements one resolution or evaluation visits; protects against
/// hostile profile graphs whose expansion grows exponentially (a resource limit, not a
/// format fact).
pub const MAX_PROFILE_WORK: usize = 1 << 16;

/// A template that can appear at a spawn point (§6.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The `GeneratorProfile_WSLEntity` DID.
    pub object: Did,
    /// The weight of the nearest `OneOf` choice on its path, if any.
    pub weight: Option<i32>,
    /// The nearest `GeneratorProfile_Probability` on its path, if any (per mille).
    pub probability: Option<i32>,
    /// The innermost placement on its path (the entry's own, else the nearest `OneOf`
    /// modifier).
    pub placement: Option<Placement>,
    /// The innermost `Repeat` on its path (the entry's own, else the nearest enclosing one).
    /// A display value of Decision D4a only: it is not the number of objects spawned, which
    /// follows from D3 (rules 2.2 and 2.4).
    pub repeat: Option<Repeat>,
    /// Whether it lies inside a `PositionSetEntry` (scoped to the point's position set).
    pub scoped: bool,
    /// Profile nesting depth (0 = the generator's own profile).
    pub depth: usize,
    /// The profile whose entry names it.
    pub profile: Did,
}

/// The result of [`resolve_candidates`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolution {
    /// The candidates, in walk order (duplicates kept: one per path).
    pub candidates: Vec<Candidate>,
    /// Profiles named but not found in the source.
    pub missing_profiles: Vec<Did>,
    /// Expansions stopped because the profile was already on the path.
    pub cycles: usize,
    /// Expansions stopped at [`MAX_PROFILE_DEPTH`].
    pub depth_limited: usize,
    /// Entries removed by C17 (weight or probability ≤ 0).
    pub removed: usize,
    /// Deepest profile nesting reached.
    pub max_depth: usize,
    /// Whether [`MAX_PROFILE_WORK`] cut the walk short.
    pub truncated: bool,
}

impl Resolution {
    /// The distinct candidate objects, ascending.
    #[must_use]
    pub fn objects(&self) -> BTreeSet<Did> {
        self.candidates.iter().map(|c| c.object).collect()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PathCtx {
    weight: Option<i32>,
    probability: Option<i32>,
    placement: Option<Placement>,
    repeat: Option<Repeat>,
    scoped: bool,
}

struct Resolver<'s, S: ?Sized> {
    src: &'s S,
    set: Option<u32>,
    out: Resolution,
    path: Vec<Did>,
    work: usize,
}

impl<S: ProfileSource + ?Sized> Resolver<'_, S> {
    fn tick(&mut self) -> bool {
        self.work += 1;
        if self.work > MAX_PROFILE_WORK {
            self.out.truncated = true;
            return false;
        }
        true
    }

    fn matches(&self, p: &PositionSetEntry) -> bool {
        match self.set {
            None => true,
            Some(s) => p.position_set == Some(s),
        }
    }

    fn profile(&mut self, did: Did, ctx: PathCtx) {
        if self.path.contains(&did) {
            self.out.cycles += 1;
            return;
        }
        let depth = self.path.len();
        if depth > MAX_PROFILE_DEPTH {
            self.out.depth_limited += 1;
            return;
        }
        let src = self.src;
        let Some(profile) = src.profile(did) else {
            if !self.out.missing_profiles.contains(&did) {
                self.out.missing_profiles.push(did);
            }
            return;
        };
        self.out.max_depth = self.out.max_depth.max(depth);
        self.path.push(did);
        for el in &profile.elements {
            if !self.tick() {
                break;
            }
            match el {
                ProfileElement::Entry(e) => self.entry(e, ctx),
                ProfileElement::OneOf(o) => self.one_of(o, ctx),
                ProfileElement::PositionSet(p) => self.position_set(p, ctx),
                ProfileElement::RegenType(_) | ProfileElement::Other { .. } => {}
            }
        }
        self.path.pop();
    }

    fn entry(&mut self, e: &ProfileEntry, mut ctx: PathCtx) {
        if e.is_removed() {
            self.out.removed += 1;
            return;
        }
        if e.probability.is_some() {
            ctx.probability = e.probability;
        }
        if e.placement.is_some() {
            ctx.placement = e.placement;
        }
        if e.repeat.is_some() {
            ctx.repeat = e.repeat;
        }
        if let Some(object) = e.object {
            let (Some(&profile), depth) = (self.path.last(), self.path.len().saturating_sub(1))
            else {
                return;
            };
            self.out.candidates.push(Candidate {
                object,
                weight: ctx.weight,
                probability: ctx.probability,
                placement: ctx.placement,
                repeat: ctx.repeat,
                scoped: ctx.scoped,
                depth,
                profile,
            });
        } else if let Some(p) = e.profile {
            self.profile(p, ctx);
        }
    }

    fn position_set(&mut self, p: &PositionSetEntry, mut ctx: PathCtx) {
        if !self.matches(p) {
            return;
        }
        ctx.scoped = true;
        if let Some(e) = &p.entry {
            self.entry(e, ctx);
        }
        if let Some(o) = &p.one_of {
            self.one_of(o, ctx);
        }
    }

    fn one_of(&mut self, o: &OneOf, mut ctx: PathCtx) {
        match o.modifier {
            Some(Modifier::Repeat(r)) => ctx.repeat = Some(r),
            Some(Modifier::Placement(p)) => ctx.placement = Some(p),
            None => {}
        }
        for c in &o.choices {
            if !self.tick() {
                break;
            }
            let mut c_ctx = ctx;
            c_ctx.weight = c.weight();
            match c {
                Choice::Entry(e) => self.entry(e, c_ctx),
                Choice::PositionSet(p) => {
                    if p.weight.is_some_and(|w| w <= 0) {
                        self.out.removed += 1;
                        continue;
                    }
                    self.position_set(p, c_ctx);
                }
            }
        }
    }
}

/// Static resolution (§6.7): the templates that can appear at a spawn point of position set
/// `position_set` of a generator whose profile is `profile`, with their weights,
/// probabilities and modifiers. Entries outside any `PositionSetEntry` apply to every point
/// (Decision D4). With `position_set` = `None` every `PositionSetEntry` is followed (all
/// templates the profile can reach anywhere). Entries removed by C17 are skipped.
#[must_use]
pub fn resolve_candidates<S: ProfileSource + ?Sized>(
    src: &S,
    profile: Did,
    position_set: Option<u32>,
) -> Resolution {
    let mut r = Resolver {
        src,
        set: position_set,
        out: Resolution::default(),
        path: Vec::new(),
        work: 0,
    };
    r.profile(profile, PathCtx::default());
    r.out
}

// ---------------------------------------------------------------------------------------------
// §6.8 Decision D3: evaluation
// ---------------------------------------------------------------------------------------------

/// Uniform random numbers for [`evaluate_profile`].
pub trait SpawnRandom {
    /// A whole number uniformly distributed in `0..n` (`n` ≥ 1).
    fn below(&mut self, n: u32) -> u32;
}

impl<F: FnMut(u32) -> u32> SpawnRandom for F {
    fn below(&mut self, n: u32) -> u32 {
        self(n) % n.max(1)
    }
}

/// Parameters of [`evaluate_profile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalOptions {
    /// Weight of a choice that has none (Decision D3 (a), §6.8; never needed on the observed
    /// install, C34).
    pub default_weight: i32,
    /// `GeneratorProfile_RepeatMinTimes` / `…MaxTimes` when absent (the dictionary default,
    /// 1; Decision D3 (b)).
    pub default_repeat: i32,
    /// Most objects produced per spawn point and evaluation (rule 2.5).
    pub max_objects: usize,
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self {
            default_weight: 1,
            default_repeat: 1,
            max_objects: 256,
        }
    }
}

impl EvalOptions {
    /// The defaults with the repeat default taken from the dictionary when it defines one
    /// for `GeneratorProfile_RepeatMinTimes`.
    #[must_use]
    pub fn from_defaults<D: PropertyDefaults + ?Sized>(defaults: &D, f: &ProfileFieldPids) -> Self {
        let mut o = Self::default();
        if let Some(v) = f
            .repeat_min
            .id()
            .and_then(|id| defaults.default_value(id))
            .and_then(ValueExt::int)
        {
            o.default_repeat = v;
        }
        o
    }
}

/// One object an evaluation produces (§6.8 rule 2).
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedObject {
    /// The `GeneratorProfile_WSLEntity` DID.
    pub object: Did,
    /// The innermost placement that applies (rule 2.4), if any.
    pub placement: Option<Placement>,
    /// The producing entry's `GeneratorProfile_Properties` (applied after the template's,
    /// rule 5).
    pub properties: Vec<PropertyEntry>,
}

/// The result of [`evaluate_profile`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluation {
    /// The produced objects, in production order.
    pub objects: Vec<SpawnedObject>,
    /// Objects dropped past [`EvalOptions::max_objects`].
    pub dropped: usize,
    /// Profiles named but not found.
    pub missing_profiles: Vec<Did>,
    /// Expansions stopped because the profile was already on the path.
    pub cycles: usize,
    /// Expansions stopped at [`MAX_PROFILE_DEPTH`].
    pub depth_limited: usize,
    /// Whether [`MAX_PROFILE_WORK`] cut the evaluation short.
    pub truncated: bool,
}

struct Evaluator<'s, S: ?Sized, R: ?Sized> {
    src: &'s S,
    set: u32,
    rng: &'s mut R,
    opts: EvalOptions,
    out: Evaluation,
    path: Vec<Did>,
    work: usize,
}

impl<S: ProfileSource + ?Sized, R: SpawnRandom + ?Sized> Evaluator<'_, S, R> {
    fn tick(&mut self) -> bool {
        self.work += 1;
        if self.work > MAX_PROFILE_WORK {
            self.out.truncated = true;
            return false;
        }
        true
    }

    /// A whole number uniformly in `lo..=hi` (bounds ordered and clamped at 0); no random
    /// draw when the range holds one value.
    fn count(&mut self, (a, b): (i32, i32)) -> u32 {
        let lo = a.min(b).max(0).unsigned_abs();
        let hi = a.max(b).max(0).unsigned_abs();
        if hi <= lo {
            return lo;
        }
        // hi − lo ≤ i32::MAX, so the range size fits in u32.
        lo + self.rng.below(hi - lo + 1)
    }

    fn profile(&mut self, did: Did, placement: Option<Placement>) {
        if self.path.contains(&did) {
            self.out.cycles += 1;
            return;
        }
        if self.path.len() > MAX_PROFILE_DEPTH {
            self.out.depth_limited += 1;
            return;
        }
        let src = self.src;
        let Some(profile) = src.profile(did) else {
            if !self.out.missing_profiles.contains(&did) {
                self.out.missing_profiles.push(did);
            }
            return;
        };
        self.path.push(did);
        for el in &profile.elements {
            if !self.tick() {
                break;
            }
            match el {
                ProfileElement::Entry(e) => self.entry(e, placement),
                ProfileElement::OneOf(o) => self.one_of(o, placement),
                ProfileElement::PositionSet(p) => self.position_set(p, placement),
                ProfileElement::RegenType(_) | ProfileElement::Other { .. } => {}
            }
        }
        self.path.pop();
    }

    fn entry(&mut self, e: &ProfileEntry, inherited: Option<Placement>) {
        if e.is_removed() {
            return;
        }
        if let Some(p) = e.probability
            && p < 1000
            && self.rng.below(1000) >= u32::try_from(p).unwrap_or(0)
        {
            return;
        }
        let k = match e.repeat {
            Some(r) => self.count(r.bounds(self.opts.default_repeat)),
            None => 1,
        };
        let placement = e.placement.or(inherited);
        for _ in 0..k {
            if !self.tick() {
                break;
            }
            if let Some(object) = e.object {
                if self.out.objects.len() >= self.opts.max_objects {
                    if self.out.dropped == 0 {
                        tracing::warn!(
                            limit = self.opts.max_objects,
                            "spawn evaluation exceeds the object limit; further objects dropped"
                        );
                    }
                    self.out.dropped += 1;
                } else {
                    self.out.objects.push(SpawnedObject {
                        object,
                        placement,
                        properties: e.properties.clone(),
                    });
                }
            } else if let Some(p) = e.profile {
                self.profile(p, placement);
            }
        }
    }

    fn position_set(&mut self, p: &PositionSetEntry, placement: Option<Placement>) {
        if p.position_set != Some(self.set) {
            return;
        }
        if let Some(e) = &p.entry {
            self.entry(e, placement);
        }
        if let Some(o) = &p.one_of {
            self.one_of(o, placement);
        }
    }

    fn eligible(&self, c: &Choice) -> Option<u32> {
        let w = c.weight().unwrap_or(self.opts.default_weight);
        if w <= 0 {
            return None;
        }
        match c {
            Choice::Entry(e) if !e.is_removed() => u32::try_from(w).ok(),
            Choice::PositionSet(p) if p.position_set == Some(self.set) => u32::try_from(w).ok(),
            _ => None,
        }
    }

    fn one_of(&mut self, o: &OneOf, inherited: Option<Placement>) {
        let mut placement = inherited;
        let mut draws = 1;
        match o.modifier {
            Some(Modifier::Repeat(r)) => draws = self.count(r.bounds(self.opts.default_repeat)),
            Some(Modifier::Placement(p)) => placement = Some(p),
            None => {}
        }
        let weighted: Vec<(u32, &Choice)> = o
            .choices
            .iter()
            .filter_map(|c| self.eligible(c).map(|w| (w, c)))
            .collect();
        let total: u64 = weighted.iter().map(|(w, _)| u64::from(*w)).sum();
        if total == 0 {
            return;
        }
        let total = u32::try_from(total).unwrap_or(u32::MAX);
        for _ in 0..draws {
            if !self.tick() {
                break;
            }
            // One eligible choice: picked without a draw.
            let mut r = if weighted.len() == 1 {
                0
            } else {
                self.rng.below(total)
            };
            let mut picked = None;
            for &(w, c) in &weighted {
                if r < w {
                    picked = Some(c);
                    break;
                }
                r -= w;
            }
            match picked {
                Some(Choice::Entry(e)) => self.entry(e, placement),
                Some(Choice::PositionSet(p)) => self.position_set(p, placement),
                None => {}
            }
        }
    }
}

/// One evaluation of profile `profile` at a spawn point of position set `position_set`
/// under Decision D3 (§6.8 rule 2). Tessera's sandbox rule, not the original's.
///
/// Random draws, in order: an entry with a probability *p* < 1000 draws `below(1000)` and
/// succeeds when the draw is < *p*; a repeat whose bounds differ draws its count; a `OneOf`
/// with two or more eligible choices draws `below(sum of eligible weights)` per draw. A
/// range of one value and a single eligible choice draw nothing.
pub fn evaluate_profile<S: ProfileSource + ?Sized, R: SpawnRandom + ?Sized>(
    src: &S,
    profile: Did,
    position_set: u32,
    rng: &mut R,
    opts: EvalOptions,
) -> Evaluation {
    let mut e = Evaluator {
        src,
        set: position_set,
        rng,
        opts,
        out: Evaluation::default(),
        path: Vec::new(),
        work: 0,
    };
    e.profile(profile, None);
    e.out
}

// ---------------------------------------------------------------------------------------------
// §6.2–§6.4, §6.9 Landblock spawn data
// ---------------------------------------------------------------------------------------------

/// Link type codes used by spawning (`landblock.md` §4.6, §6.3 here).
pub mod link_type {
    /// PatrolOnce.
    pub const PATROL_ONCE: u32 = 1;
    /// Patrol.
    pub const PATROL: u32 = 2;
    /// Generator_PositionSet: generator (source) → GeneratorPoint (target).
    pub const GENERATOR_POSITION_SET: u32 = 7;
    /// Generator: spawned object (source) → generator (target).
    pub const GENERATOR: u32 = 8;
    /// FlyingPatrol.
    pub const FLYING_PATROL: u32 = 11;
    /// Path: patrol point → next patrol point.
    pub const PATH: u32 = 16;

    /// Whether `t` starts a patrol (types 2, 1 and 11).
    #[must_use]
    pub fn starts_patrol(t: u32) -> bool {
        matches!(t, PATROL | PATROL_ONCE | FLYING_PATROL)
    }
}

/// One `Generator_PositionSetLimitRecord` (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PositionSetLimit {
    /// `Generator_PositionSetID` key.
    pub position_set: Option<u32>,
    /// `Generator_PositionSetLimit`.
    pub limit: Option<i32>,
}

/// A generator of one landblock (§1a, §6.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeneratorInfo {
    /// Instance ID.
    pub iid: InstanceId,
    /// `Generator_ProfileDID` (non-null).
    pub profile: Option<Did>,
    /// `Generator_PositionSetLimitArray`.
    pub limits: Vec<PositionSetLimit>,
    /// Targets of its type-7 links (spawn points), in link order.
    pub points: Vec<InstanceId>,
    /// Type-7 links from it whose target is in another landblock (target 0).
    pub cross_point_links: usize,
    /// Sources of type-8 links to it (individually placed objects), in link order.
    pub spawned: Vec<InstanceId>,
    /// Type-8 links to it whose source is in another landblock (source 0).
    pub cross_spawn_links: usize,
    /// Whether the landblock's weenie list holds an instance set for it.
    pub has_instance_set: bool,
    /// Whether a static entity places it.
    pub has_static_entity: bool,
}

/// Where a spawn point's position set came from (§6.4 precedence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionSetSource {
    /// The point's instance set.
    InstanceSet,
    /// The static entity's own set.
    EntitySet,
    /// The 0x47 marker (or its parents).
    Marker,
}

/// A GeneratorPoint static entity (§6.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnPointInfo {
    /// Instance ID.
    pub iid: InstanceId,
    /// Index into [`LandblockInfo::entities`].
    pub entity: usize,
    /// The 0x47 marker it names.
    pub marker: Option<Did>,
    /// Its position set and where it came from.
    pub position_set: Option<(u32, PositionSetSource)>,
}

/// A patrol start (§6.9): a type-2, -1 or -11 link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatrolStart {
    /// Link type.
    pub link_type: u32,
    /// The patrolling object or GeneratorPoint (0 in a cross-landblock half).
    pub source: InstanceId,
    /// The first PatrolPoint (0 in a cross-landblock half).
    pub start: InstanceId,
    /// `PatrolLink_Speed` on the link (meaning OPEN, §6.9).
    pub speed: Option<f32>,
}

/// How a patrol chain ends (§6.9 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatrolEnd {
    /// A point without a next point.
    NoNext,
    /// The chain comes back to its first point.
    ReturnsToStart,
    /// The chain comes back to a point other than the first.
    EntersLoop,
    /// The next link is a cross-landblock half (next point 0).
    LeavesLandblock,
}

/// A chain followed by [`PatrolGraph::follow`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatrolChain {
    /// The points visited, starting with the start point, each once.
    pub points: Vec<InstanceId>,
    /// How the chain ends.
    pub end: PatrolEnd,
}

/// The type-16 path graph of one landblock (§6.9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatrolGraph {
    next: HashMap<InstanceId, Vec<InstanceId>>,
    links: usize,
}

impl PatrolGraph {
    /// Adds a type-16 link `from` → `to` (`to` 0 in a cross-landblock half).
    pub fn add(&mut self, from: InstanceId, to: InstanceId) {
        if from.0 == 0 {
            return;
        }
        self.next.entry(from).or_default().push(to);
        self.links += 1;
    }

    /// The next points of `point`, in link order.
    #[must_use]
    pub fn next(&self, point: InstanceId) -> &[InstanceId] {
        self.next.get(&point).map_or(&[], Vec::as_slice)
    }

    /// Number of links with a source in the landblock.
    #[must_use]
    pub fn len(&self) -> usize {
        self.links
    }

    /// Whether the graph has no link.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links == 0
    }

    /// Follows the chain from `start` through the last-stored next link of each point
    /// (§6.9), stopping at the end, on a revisit or at a cross-landblock link.
    #[must_use]
    pub fn follow(&self, start: InstanceId) -> PatrolChain {
        let mut points = vec![start];
        let mut seen: HashSet<InstanceId> = HashSet::from([start]);
        let mut at = start;
        // Every step visits a new point, so the chain ends within links + 1 steps.
        for _ in 0..=self.links {
            let Some(&next) = self.next(at).last() else {
                return PatrolChain {
                    points,
                    end: PatrolEnd::NoNext,
                };
            };
            if next.0 == 0 {
                return PatrolChain {
                    points,
                    end: PatrolEnd::LeavesLandblock,
                };
            }
            if next == start {
                return PatrolChain {
                    points,
                    end: PatrolEnd::ReturnsToStart,
                };
            }
            if !seen.insert(next) {
                return PatrolChain {
                    points,
                    end: PatrolEnd::EntersLoop,
                };
            }
            points.push(next);
            at = next;
        }
        PatrolChain {
            points,
            end: PatrolEnd::EntersLoop,
        }
    }
}

/// The spawn data of one landblock-info record (§6.2–§6.4, §6.9).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LandblockSpawns {
    /// Generators: instance sets with `Generator_ProfileDID`, sources of type-7 links and
    /// targets of type-8 links; ordered by instance ID.
    pub generators: Vec<GeneratorInfo>,
    /// GeneratorPoint static entities, in entity order.
    pub points: Vec<SpawnPointInfo>,
    /// Patrol starts, in link order.
    pub patrol_starts: Vec<PatrolStart>,
    /// The path graph.
    pub paths: PatrolGraph,
}

impl LandblockSpawns {
    /// Collects the spawn data of `info`. `marker_position_set` gives the position set of a
    /// 0x47 marker (its own set, else through its parents, §6.4 step 2; see
    /// [`marker_position_set`]).
    pub fn build(
        info: &LandblockInfo,
        p: &SpawnPids,
        mut marker_position_set: impl FnMut(Did) -> Option<u32>,
    ) -> Self {
        let weenies: HashMap<InstanceId, &PropertySet> = info
            .weenies
            .iter()
            .map(|w| (w.iid, &w.properties))
            .collect();
        let entity_iids: HashSet<InstanceId> = info.entities.iter().filter_map(|e| e.iid).collect();

        let mut generators: BTreeMap<InstanceId, GeneratorInfo> = BTreeMap::new();
        let gen_entry = |gens: &mut BTreeMap<InstanceId, GeneratorInfo>, iid: InstanceId| {
            gens.entry(iid).or_insert_with(|| GeneratorInfo {
                iid,
                ..GeneratorInfo::default()
            });
        };
        for w in info.weenies.iter() {
            if w.properties.did_nonnull(p.profile_did).is_some() {
                gen_entry(&mut generators, w.iid);
            }
        }
        let mut patrol_starts = Vec::new();
        let mut paths = PatrolGraph::default();
        for l in &info.links {
            match l.link_type {
                link_type::GENERATOR_POSITION_SET if l.from_iid.0 != 0 => {
                    gen_entry(&mut generators, l.from_iid);
                    if let Some(g) = generators.get_mut(&l.from_iid) {
                        if l.to_iid.0 == 0 {
                            g.cross_point_links += 1;
                        } else {
                            g.points.push(l.to_iid);
                        }
                    }
                }
                link_type::GENERATOR if l.to_iid.0 != 0 => {
                    gen_entry(&mut generators, l.to_iid);
                    if let Some(g) = generators.get_mut(&l.to_iid) {
                        if l.from_iid.0 == 0 {
                            g.cross_spawn_links += 1;
                        } else {
                            g.spawned.push(l.from_iid);
                        }
                    }
                }
                link_type::PATH => paths.add(l.from_iid, l.to_iid),
                t if link_type::starts_patrol(t) => patrol_starts.push(PatrolStart {
                    link_type: t,
                    source: l.from_iid,
                    start: l.to_iid,
                    speed: l.properties.float(p.patrol_speed),
                }),
                _ => {}
            }
        }
        for g in generators.values_mut() {
            g.has_instance_set = weenies.contains_key(&g.iid);
            g.has_static_entity = entity_iids.contains(&g.iid);
            if let Some(set) = weenies.get(&g.iid) {
                g.profile = set.did_nonnull(p.profile_did);
                g.limits = set
                    .structs(p.limit_array)
                    .map(|s| PositionSetLimit {
                        position_set: s.enum_key(p.position_set_id),
                        limit: s.int(p.limit),
                    })
                    .collect();
            }
        }

        let mut points = Vec::new();
        for (i, e) in info.entities.iter().enumerate() {
            if e.kind() != Some(StaticEntityType::GeneratorPoint) {
                continue;
            }
            let Some(iid) = e.iid else { continue };
            let own = weenies
                .get(&iid)
                .and_then(|s| s.enum_key(p.position_set_id))
                .map(|k| (k, PositionSetSource::InstanceSet));
            let entity = || {
                e.properties
                    .as_ref()
                    .and_then(|s| s.enum_key(p.position_set_id))
                    .map(|k| (k, PositionSetSource::EntitySet))
            };
            let position_set = own.or_else(entity).or_else(|| {
                e.object
                    .and_then(&mut marker_position_set)
                    .map(|k| (k, PositionSetSource::Marker))
            });
            points.push(SpawnPointInfo {
                iid,
                entity: i,
                marker: e.object,
                position_set,
            });
        }
        Self {
            generators: generators.into_values().collect(),
            points,
            patrol_starts,
            paths,
        }
    }

    /// The point with instance ID `iid`.
    #[must_use]
    pub fn point(&self, iid: InstanceId) -> Option<&SpawnPointInfo> {
        self.points.iter().find(|p| p.iid == iid)
    }

    /// The generator with instance ID `iid`.
    #[must_use]
    pub fn generator(&self, iid: InstanceId) -> Option<&GeneratorInfo> {
        self.generators
            .binary_search_by_key(&iid, |g| g.iid)
            .ok()
            .and_then(|i| self.generators.get(i))
    }
}

/// Longest 0x47 parent chain [`marker_position_set`] follows.
pub const MAX_MARKER_CHAIN: usize = 16;

/// The position set of a 0x47 marker (§6.4 step 2): `Generator_PositionSetID` in its own
/// set, else in its parents'. `fetch` loads a marker and returns its position set (if its
/// set holds one) and its parent 0x47 DID (if any). Cycles and chains longer than
/// [`MAX_MARKER_CHAIN`] end the search.
pub fn marker_position_set(
    marker: Did,
    mut fetch: impl FnMut(Did) -> Option<(Option<u32>, Option<Did>)>,
) -> Option<u32> {
    let mut seen = HashSet::new();
    let mut at = marker;
    for _ in 0..MAX_MARKER_CHAIN {
        if !seen.insert(at) {
            return None;
        }
        let (set, parent) = fetch(at)?;
        if set.is_some() {
            return set;
        }
        at = parent?;
    }
    None
}

// ---------------------------------------------------------------------------------------------
// §7 Encounter table
// ---------------------------------------------------------------------------------------------

/// Size of one encounter-table slot (§7).
pub const ENCOUNTER_SLOT_LEN: usize = 21;
/// Smallest encoding of a zone entry: key, empty label, slot count.
const MIN_ZONE_LEN: usize = 9;

/// One slot of an encounter zone (§7; field meanings OPEN).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EncounterSlot {
    /// Offset 0: slot index (0 … *m* − 1 observed).
    pub index: u32,
    /// Offset 4 (0 observed).
    pub field_4: u32,
    /// Offset 8 (1.0 observed).
    pub field_8: f32,
    /// Offset 12 (1 observed).
    pub field_12: u32,
    /// Offset 16 (0 observed).
    pub field_16: u32,
    /// Offset 20 (0 observed).
    pub field_20: u8,
}

/// One zone of an encounter table (§7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EncounterZone {
    /// Zone key (equals the entry's index in every observed record).
    pub key: u32,
    /// Editor label (Latin-1).
    pub label: String,
    /// The slots.
    pub slots: Vec<EncounterSlot>,
}

/// An encounter table, prefix 0x14 (§7; ENCOUNTER_DESC).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EncounterTable {
    /// Own DID.
    pub did: Did,
    /// Offset 4 (0 observed).
    pub reserved: u32,
    /// Offset 8: the block-map key the table is indexed by (6 observed).
    pub block_map_key: u32,
    /// The zones.
    pub zones: Vec<EncounterZone>,
}

impl EncounterTable {
    /// The zone with key `key`.
    #[must_use]
    pub fn zone(&self, key: u32) -> Option<&EncounterZone> {
        self.zones.iter().find(|z| z.key == key)
    }

    /// Whether every zone key equals its index (CR11).
    #[must_use]
    pub fn keys_are_indices(&self) -> bool {
        self.zones
            .iter()
            .enumerate()
            .all(|(i, z)| usize::try_from(z.key).is_ok_and(|k| k == i))
    }

    /// Encodes the table (§7). The label is written as Latin-1 (characters above U+00FF
    /// become `?`) with a shortest VLE length; a decoded table re-encodes byte for byte when
    /// its payload used shortest VLEs.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.extend_from_slice(&self.reserved.to_le_bytes());
        out.extend_from_slice(&self.block_map_key.to_le_bytes());
        out.extend_from_slice(&count_u32(self.zones.len()).to_le_bytes());
        for z in &self.zones {
            out.extend_from_slice(&z.key.to_le_bytes());
            let label: Vec<u8> = z
                .label
                .chars()
                .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                .collect();
            crate::reader::write_vle(&mut out, count_u32(label.len()));
            out.extend_from_slice(&label);
            out.extend_from_slice(&count_u32(z.slots.len()).to_le_bytes());
            for s in &z.slots {
                out.extend_from_slice(&s.index.to_le_bytes());
                out.extend_from_slice(&s.field_4.to_le_bytes());
                out.extend_from_slice(&s.field_8.to_le_bytes());
                out.extend_from_slice(&s.field_12.to_le_bytes());
                out.extend_from_slice(&s.field_16.to_le_bytes());
                out.push(s.field_20);
            }
        }
        out
    }
}

fn count_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Record<'_> for EncounterTable {
    const NAME: &'static str = "encounter table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::EncounterTable)?;
        let reserved = r.u32()?;
        let block_map_key = r.u32()?;
        let n = r.u32()?;
        let mut zones = r.alloc_vec(n, MIN_ZONE_LEN, PREALLOC_CAP)?;
        for _ in 0..n {
            let key = r.u32()?;
            let label = r.pascal_string()?;
            let m = r.u32()?;
            let mut slots = r.alloc_vec(m, ENCOUNTER_SLOT_LEN, PREALLOC_CAP)?;
            for _ in 0..m {
                let slot = EncounterSlot {
                    index: r.u32()?,
                    field_4: r.u32()?,
                    field_8: r.f32()?,
                    field_12: r.u32()?,
                    field_16: r.u32()?,
                    field_20: r.u8()?,
                };
                r.push(&mut slots, slot, m)?;
            }
            r.push(&mut zones, EncounterZone { key, label, slots }, n)?;
        }
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            reserved,
            block_map_key,
            zones,
        })
    }
}

/// Decodes an encounter-table payload (for fuzz targets and tools).
pub fn decode_encounter_table(data: &[u8]) -> Result<EncounterTable> {
    EncounterTable::parse(data)
}

// ---------------------------------------------------------------------------------------------
// Unit tests (the synthetic worked examples live in tests/gameplay_creatures.rs)
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_patterns_in_table_order() {
        assert_eq!(
            mirror_base("Forward_Cached_A"),
            Some((MirrorPattern::ForwardCachedPrefix, "A"))
        );
        assert_eq!(
            mirror_base("A_B_Cached"),
            Some((MirrorPattern::CachedSuffix, "A_B"))
        );
        assert_eq!(
            mirror_base("ForwardSource_A"),
            Some((MirrorPattern::ForwardSourcePrefix, "A"))
        );
        assert_eq!(
            mirror_base("A_ForwardSource"),
            Some((MirrorPattern::ForwardSourceSuffix, "A"))
        );
        assert_eq!(
            mirror_base("A_Shadow"),
            Some((MirrorPattern::ShadowSuffix, "A"))
        );
        assert_eq!(
            mirror_base("A_Base"),
            Some((MirrorPattern::BaseSuffix, "A"))
        );
        assert_eq!(mirror_base("_Base"), None);
        assert_eq!(mirror_base("Plain"), None);
        // Two patterns: the first in table order wins.
        assert_eq!(
            mirror_base("ForwardSource_A_Cached"),
            Some((MirrorPattern::CachedSuffix, "ForwardSource_A"))
        );
        for p in MirrorPattern::ALL {
            assert_eq!(p.base_of(&p.mirror_name("X")), Some("X"));
        }
    }

    #[test]
    fn unique_match_rejects_ambiguity() {
        let l = LabelIndex::new([(1, "Fam_A_Alpha"), (2, "Fam_B_Alpha"), (3, "Fam_Beta")]);
        assert_eq!(l.unique_match("Fam", "_Alpha"), None);
        assert_eq!(l.unique_match("Fam", "_Beta"), Some(3));
        assert_eq!(l.unique_match("Fam_Beta", "_Beta"), None);
    }

    #[test]
    fn immunity_bits() {
        assert_eq!(immunity_indicators(0, false), [false, false, false]);
        assert_eq!(immunity_indicators(1 << 2, false), [false, false, false]);
        assert_eq!(
            immunity_indicators((1 << 2) | (1 << 14), false),
            [true, false, false]
        );
        assert_eq!(immunity_indicators(0, true), [false, true, false]);
        assert_eq!(immunity_indicators(1 << 8, false), [false, true, false]);
        assert_eq!(immunity_indicators(1 << 9, false), [false, false, true]);
    }

    #[test]
    fn patrol_chain_ends() {
        let i = InstanceId;
        let mut g = PatrolGraph::default();
        g.add(i(1), i(2));
        g.add(i(2), i(3));
        g.add(i(3), i(1));
        assert_eq!(g.follow(i(1)).end, PatrolEnd::ReturnsToStart);
        assert_eq!(g.follow(i(1)).points, vec![i(1), i(2), i(3)]);
        g.add(i(5), i(6));
        g.add(i(6), i(2));
        assert_eq!(g.follow(i(5)).end, PatrolEnd::EntersLoop);
        g.add(i(7), i(0));
        assert_eq!(g.follow(i(7)).end, PatrolEnd::LeavesLandblock);
        assert_eq!(g.follow(i(9)).end, PatrolEnd::NoNext);
        // The last-stored next link is followed.
        g.add(i(10), i(11));
        g.add(i(10), i(0));
        assert_eq!(g.follow(i(10)).end, PatrolEnd::LeavesLandblock);
        g.add(i(12), i(12));
        assert_eq!(g.follow(i(12)).end, PatrolEnd::ReturnsToStart);
    }

    #[test]
    fn marker_chain() {
        let m = |d: Did| match d.0 {
            1 => Some((None, Some(Did(2)))),
            2 => Some((Some(7), None)),
            3 => Some((None, Some(Did(3)))),
            _ => None,
        };
        assert_eq!(marker_position_set(Did(1), m), Some(7));
        assert_eq!(marker_position_set(Did(3), m), None);
        assert_eq!(marker_position_set(Did(9), m), None);
    }

    #[test]
    fn action_table_level_bounds() {
        let t = ActionTableRef {
            table: Did(1),
            min_level: Some(-1),
            max_level: Some(0),
        };
        assert!(t.contains_level(1) && t.contains_level(500));
        let t = ActionTableRef {
            table: Did(1),
            min_level: Some(10),
            max_level: Some(20),
        };
        assert!(!t.contains_level(9) && t.contains_level(10) && !t.contains_level(21));
    }
}
