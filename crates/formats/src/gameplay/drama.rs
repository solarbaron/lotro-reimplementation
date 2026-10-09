// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC dramas: drama lists, named dramas, their actions and facets, triggers, scripted
//! fights and starting idles, with Tessera's playback rules.
//!
//! Spec: `docs/specs/npc-drama.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`DramaLabels`] | Every label this module reads, resolved at run time | §3.1, §7, §8 |
//! | [`DramaList`], [`NamedDrama`], [`DramaKind`], [`effective_list`] | The list tree, its three element kinds, list precedence (Decision T4) | §2.2, §3.2, §3.3 |
//! | [`Action`], [`Facets`], [`Facet`], [`Dialog`], [`DramaSound`], [`Broadcast`], [`DirectActors`], [`AiHints`] | The action struct and its facets | §3.4, §4 |
//! | [`names_match`], [`script_link_port`], [`actor_number`], [`actors_of_mask`] | Name matching (Decision T3), script-link port from the label, actor selection | §4.6.1, §4.6.4, §10.4 |
//! | [`KungFu`], [`KungFuTransition`], [`transition_fires`] | Scripted fights | §7, §10.3 |
//! | [`StartingIdle`] | Starting-idle properties | §5.3, §8 |
//! | [`plan_drama`], [`DramaStep`], [`PerformDecision`], [`perform_decision`] | Decision T1: running one named drama | §10.1 |
//! | [`DialogPresentation`], [`dialog_presentation`], [`Facet::runs_offline`] | Decisions T2 and T5: what the offline client shows | §10.2 |
//!
//! Properties are found by label ([`DramaLabels::resolve`]). Enum labels (dialog types,
//! script-link labels, actor labels) are passed in by the caller, who reads them from the
//! install's enum mappers; no PID, key or label table of one install is written here. The
//! numbers that are written here are Tessera's own Decisions of §10 (waits, queue length,
//! durations), each marked where it is defined.
//!
//! Every view tolerates absent and mistyped values: they read as absent, and an element of
//! an unknown kind is kept as [`DramaKind::Other`] and skipped by name lookups.

use crate::did::Did;
use crate::gameplay::common::{Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::property::{PropertyEntry, PropertyNames, PropertyStruct, PropertyValue};
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// The `Drama_*` labels (§3.1).
    pub struct DramaPids {
        list = "Drama_DramaList",
        named = "Drama_NamedDrama",
        instant = "Drama_NamedDrama_Instant",
        idle = "Drama_NamedDrama_Idle",
        name = "Drama_Name",
        sequence = "Drama_ActionSequence",
        sequence_idle = "Drama_ActionSequence_Idle",
        action = "Drama_Action",
        action_instant = "Drama_Action_Instant",
        action_idle = "Drama_Action_Idle",
        interruptible = "Drama_Interruptible",
        wait_for_drama = "Drama_WaitForDrama",
        become_monster = "Drama_KungFu_BecomeMonster",
        become_npc = "Drama_KungFu_BecomeNPC",
        do_not_end_combat = "Drama_KungFu_BecomeNPC_DoNotEndCombat",
        comment = "Drama_Comment",
        action_comment = "Drama_ActionComment",
        wait_before = "Drama_Wait_Before",
        wait_after = "Drama_Wait_After",
        wait_variance = "Drama_Wait_Variance",
        wait_emote = "Drama_Wait_Emote",
        dialog = "Drama_Dialog",
        dialog_type = "Drama_Dialog_Type",
        say_channel = "Drama_Dialog_SayChannel",
        text_list = "Drama_Dialog_TextList",
        text = "Drama_Dialog_Text",
        emote = "Drama_Emote",
        emote_stop = "Drama_Emote_Stop",
        sound = "Drama_Sound",
        sound_id = "Drama_SoundID",
        sound_channel = "Drama_SoundChannel",
        sound_min_distance = "Drama_SoundMinRandomDistance",
        sound_max_distance = "Drama_SoundMaxRandomDistance",
        instance_sound = "Drama_InstanceSound",
        patrol_start = "Drama_PatrolStart",
        patrol_low_pri_start = "Drama_PatrolLowPriStart",
        patrol_change_speed = "Drama_PatrolChangeSpeed",
        patrol_change_speed_name = "Drama_PatrolChangeSpeed_Name",
        patrol_change_speed_speed = "Drama_PatrolChangeSpeed_Speed",
        script_link = "Drama_ScriptLink",
        broadcast = "Drama_BroadcastRadius",
        broadcast_property = "Drama_BroadcastRadius_PropertyToMatch",
        broadcast_name = "Drama_BroadcastRadius_Name",
        broadcast_range = "Drama_BroadcastRadius_Range",
        detector_enable = "Drama_RemoteDetector_Enable",
        detector_disable = "Drama_RemoteDetector_Disable",
        direct_actors = "Drama_DirectActors",
        actors = "Drama_DirectActors_Actors",
        actors_dialog = "Drama_DirectActors_Dialog",
        actors_dialog_type = "Drama_DirectActors_Dialog_Type",
        actors_text_list = "Drama_DirectActors_Dialog_TextList",
        actors_text = "Drama_DirectActors_Dialog_Text",
        actors_say_channel = "Drama_DirectActors_Dialog_SayChannel",
        actors_emote = "Drama_DirectActors_Emote",
        actors_perform = "Drama_DirectActors_PerformDrama",
        actors_patrol_start = "Drama_DirectActors_PatrolStart",
        actors_patrol_low_pri_start = "Drama_DirectActors_PatrolLowPriStart",
        add_effect = "Drama_AddEffect",
        add_effect_id = "Drama_AddEffectID",
        add_effect_spellcraft = "Drama_AddEffect_Spellcraft",
        remove_effect = "Drama_RemoveEffectID",
        scriptlet = "Drama_Scriptlet",
        replicate = "Drama_ReplicateScriptServerToClient",
        ai_hints = "Drama_AIHints",
        set_hints = "Drama_SetHints",
        hint_mask = "Drama_HintMask",
        billboard = "Drama_InstanceBillboard",
        debug_shout = "Drama_Debug_Shout",
        destroy_self = "Drama_DestroySelf",
        destroy_fade = "Drama_DestroySelf_FadeOut",
        stop_starting_idle = "Drama_StopStartingIdle",
        set_idle = "Drama_SetIdleDrama",
        clear_idle = "Drama_ClearIdleDrama",
        starting_idle = "Drama_StartingIdle",
        can_interact = "Drama_CanInteractDuringDrama",
        performing = "Drama_Performing",
        target = "Drama_Target",
    }
}

crate::property_ids! {
    /// Scripted-fight labels (§7). The health-ratio field is written `…_HealthRatio` in the
    /// spec; both readings of the elision are resolved and the defined one is used.
    pub struct KungFuPids {
        start_as_npc = "AI_KungFu_StartAsNPC",
        repeatable = "AI_KungFu_Repeatable",
        enable_combat = "AI_KungFu_EnableCombat",
        is_npc = "AI_KungFu_IsNPC",
        transition_list = "AI_KungFu_TransitionList",
        drama_transition = "AI_KungFu_DramaTransition",
        quest_transition = "AI_KungFu_QuestTransition",
        transition_drama = "AI_KungFu_Transition_Drama",
        health_ratio = "AI_KungFu_HealthRatio",
        transition_health_ratio = "AI_KungFu_Transition_HealthRatio",
        should_end_combat = "AI_KungFu_ShouldEndCombat",
        always_active = "AI_KungFu_Transition_AlwaysActive",
        suppress_clamp = "AI_KungFu_Transition_SuppressHealthClamp",
    }
}

crate::property_ids! {
    /// Starting-idle and broadcast labels (§4.6.2, §8).
    pub struct StartingIdlePids {
        animation = "NPC_StartingIdleAnimation",
        playing = "NPC_PlayingStartingIdle",
        returns = "NPC_ReturnsToStartingIdle",
        offset = "NPC_StartingIdle_AnimStartOffset",
        accepts_broadcasts = "AI_AcceptsBroadcasts",
    }
}

/// Every label this module reads, resolved against one install's dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DramaLabels {
    /// `Drama_*`.
    pub drama: DramaPids,
    /// `AI_KungFu_*`.
    pub kung_fu: KungFuPids,
    /// Starting idles and broadcasts.
    pub idle: StartingIdlePids,
}

impl DramaLabels {
    /// Resolves every label against `names`.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            drama: DramaPids::resolve(names),
            kung_fu: KungFuPids::resolve(names),
            idle: StartingIdlePids::resolve(names),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub fn unresolved() -> Self {
        Self {
            drama: DramaPids::unresolved(),
            kung_fu: KungFuPids::unresolved(),
            idle: StartingIdlePids::unresolved(),
        }
    }

    /// Every label, in declaration order.
    pub fn labels() -> impl Iterator<Item = &'static str> {
        DramaPids::LABELS
            .iter()
            .chain(KungFuPids::LABELS)
            .chain(StartingIdlePids::LABELS)
            .copied()
    }

    /// The labels the dictionary does not define (one of the two health-ratio readings is
    /// always among them).
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.drama.missing();
        out.extend(self.kung_fu.missing());
        out.extend(self.idle.missing());
        out
    }

    /// The health-ratio label that resolves (§7).
    #[must_use]
    pub fn health_ratio(&self) -> Pid {
        if self.kung_fu.health_ratio.is_resolved() {
            self.kung_fu.health_ratio
        } else {
            self.kung_fu.transition_health_ratio
        }
    }
}

fn code(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) | PropertyValue::PropertyId(k) => {
            Some(*k)
        }
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn code_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Option<u32> {
    code(a.get_pid(pid)?)
}

/// Finite floats only.
fn finite(v: Option<f32>) -> Option<f32> {
    v.filter(|f| f.is_finite())
}

// ---------------------------------------------------------------------------------------------
// Lists and named dramas (§2, §3)
// ---------------------------------------------------------------------------------------------

/// The element kind of a drama list (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DramaKind {
    /// `Drama_NamedDrama`: a sequence of actions with flags.
    Named,
    /// `Drama_NamedDrama_Instant`: one action, all facets at once (§5.2).
    Instant,
    /// `Drama_NamedDrama_Idle`: a sequence of idle actions (§5.3).
    Idle,
    /// Any other element PID (none in the install).
    Other(u32),
}

impl DramaKind {
    /// A short name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Named => "named",
            Self::Instant => "instant",
            Self::Idle => "idle",
            Self::Other(_) => "other",
        }
    }
}

/// Decision T3 (§10.4): drama names are compared after ASCII case folding, nothing else.
#[must_use]
pub fn names_match(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// One element of a drama list.
#[derive(Debug, Clone, Copy)]
pub struct NamedDrama<'a> {
    pids: &'a DramaPids,
    /// The element kind.
    pub kind: DramaKind,
    /// The element struct (`None` for a non-struct element).
    pub raw: Option<&'a PropertyStruct>,
}

impl<'a> NamedDrama<'a> {
    /// A view over one list element.
    #[must_use]
    pub fn new(pids: &'a DramaPids, element: &'a PropertyEntry) -> Self {
        let kind = if pids.named.is(element.pid) {
            DramaKind::Named
        } else if pids.instant.is(element.pid) {
            DramaKind::Instant
        } else if pids.idle.is(element.pid) {
            DramaKind::Idle
        } else {
            DramaKind::Other(element.pid)
        };
        Self {
            pids,
            kind,
            raw: element.value.as_struct(),
        }
    }

    /// `Drama_Name` (a designer identifier, never shown to players).
    #[must_use]
    pub fn name(&self) -> Option<&'a str> {
        self.raw?.string(self.pids.name)
    }

    /// Whether the drama is called `name` (Decision T3).
    #[must_use]
    pub fn is_named(&self, name: &str) -> bool {
        self.name().is_some_and(|n| names_match(n, name))
    }

    /// The actions in stored order: the sequence of a named or idle drama (struct elements;
    /// other elements are kept as empty actions), the single action of an instant drama.
    #[must_use]
    pub fn actions(&self) -> Vec<Action<'a>> {
        let Some(raw) = self.raw else {
            return Vec::new();
        };
        let p = self.pids;
        match self.kind {
            DramaKind::Named | DramaKind::Idle => {
                let seq = if self.kind == DramaKind::Named {
                    p.sequence
                } else {
                    p.sequence_idle
                };
                raw.elements(seq)
                    .iter()
                    .map(|e| Action::new(p, e.value.as_struct(), Some(e.pid)))
                    .collect()
            }
            DramaKind::Instant => raw
                .structure(p.action_instant)
                .map(|s| vec![Action::new(p, Some(s), p.action_instant.id())])
                .unwrap_or_default(),
            DramaKind::Other(_) => Vec::new(),
        }
    }

    /// `Drama_Interruptible` as stored.
    #[must_use]
    pub fn interruptible(&self) -> Option<bool> {
        self.raw?.boolean(self.pids.interruptible)
    }

    /// `Drama_WaitForDrama` as stored (seconds presumably, §5.1).
    #[must_use]
    pub fn wait_for_drama(&self) -> Option<f32> {
        finite(self.raw?.float(self.pids.wait_for_drama))
    }

    /// `Drama_KungFu_BecomeMonster` as stored.
    #[must_use]
    pub fn become_monster(&self) -> Option<bool> {
        self.raw?.boolean(self.pids.become_monster)
    }

    /// `Drama_KungFu_BecomeNPC` as stored.
    #[must_use]
    pub fn become_npc(&self) -> Option<bool> {
        self.raw?.boolean(self.pids.become_npc)
    }

    /// `Drama_KungFu_BecomeNPC_DoNotEndCombat` as stored.
    #[must_use]
    pub fn do_not_end_combat(&self) -> Option<bool> {
        self.raw?.boolean(self.pids.do_not_end_combat)
    }

    /// `Drama_KungFu_BecomeNPC_DoNotEndCombat` with its dictionary default (true in the
    /// install).
    #[must_use]
    pub fn do_not_end_combat_effective<D: PropertyDefaults + ?Sized>(&self, defaults: &D) -> bool {
        self.do_not_end_combat().unwrap_or_else(|| {
            self.pids
                .do_not_end_combat
                .id()
                .and_then(|p| defaults.default_value(p))
                .and_then(ValueExt::boolean)
                .unwrap_or(true)
        })
    }
}

/// A drama list (§3.2): its elements in stored order.
#[derive(Debug, Clone)]
pub struct DramaList<'a> {
    /// The elements.
    pub dramas: Vec<NamedDrama<'a>>,
}

impl<'a> DramaList<'a> {
    /// The list of a `Drama_DramaList` value; `None` when it is not an array.
    #[must_use]
    pub fn from_value(pids: &'a DramaPids, value: &'a PropertyValue) -> Option<Self> {
        let arr = value.as_array()?;
        Some(Self {
            dramas: arr
                .elements
                .iter()
                .map(|e| NamedDrama::new(pids, e))
                .collect(),
        })
    }

    /// The list stored in `a` (a property set or struct), if any.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(pids: &'a DramaPids, a: &'a A) -> Option<Self> {
        Self::from_value(pids, a.get_pid(pids.list)?)
    }

    /// The first drama called `name` (Decision T3); elements of unknown kind are skipped.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&NamedDrama<'a>> {
        self.dramas
            .iter()
            .find(|d| !matches!(d.kind, DramaKind::Other(_)) && d.is_named(name))
    }

    /// Whether two elements have names that are equal after ASCII case folding (none in the
    /// install, §3.2).
    #[must_use]
    pub fn has_duplicate_names(&self) -> bool {
        let mut names: Vec<String> = self
            .dramas
            .iter()
            .filter_map(NamedDrama::name)
            .map(str::to_ascii_lowercase)
            .collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        names.len() != before
    }
}

/// Decision T4 (§2.2): the effective drama list of an entity is the first
/// `Drama_DramaList` found in its lookup order (instance set, then template, then the
/// template's parents); lists are never merged, and an empty list hides the lists below.
#[must_use]
pub fn effective_list<'a, A: PropertyAccess + ?Sized>(
    pids: &'a DramaPids,
    layers: &[&'a A],
) -> Option<DramaList<'a>> {
    layers.iter().find_map(|l| DramaList::read(pids, *l))
}

// ---------------------------------------------------------------------------------------------
// Actions and facets (§3.4, §4)
// ---------------------------------------------------------------------------------------------

/// The facets of an action, in the order Decision T1 applies them (§10.1 step 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Facet {
    /// `Drama_StopStartingIdle`.
    StopStartingIdle,
    /// `Drama_Emote_Stop`.
    EmoteStop,
    /// `Drama_Emote`.
    Emote,
    /// `Drama_Dialog`.
    Dialog,
    /// `Drama_Sound`.
    Sound,
    /// `Drama_DirectActors`.
    DirectActors,
    /// `Drama_PatrolChangeSpeed`.
    PatrolChangeSpeed,
    /// `Drama_PatrolStart`.
    PatrolStart,
    /// `Drama_PatrolLowPriStart`.
    PatrolLowPriStart,
    /// `Drama_ScriptLink`.
    ScriptLink,
    /// `Drama_BroadcastRadius`.
    BroadcastRadius,
    /// `Drama_RemoteDetector_Enable`.
    RemoteDetectorEnable,
    /// `Drama_RemoteDetector_Disable`.
    RemoteDetectorDisable,
    /// `Drama_AIHints`.
    AiHints,
    /// `Drama_AddEffect`.
    AddEffect,
    /// `Drama_RemoveEffectID`.
    RemoveEffect,
    /// `Drama_Scriptlet`.
    Scriptlet,
    /// `Drama_SetIdleDrama`.
    SetIdleDrama,
    /// `Drama_ClearIdleDrama`.
    ClearIdleDrama,
    /// `Drama_InstanceBillboard`.
    InstanceBillboard,
    /// `Drama_DestroySelf_FadeOut`.
    DestroySelfFadeOut,
    /// `Drama_DestroySelf`.
    DestroySelf,
}

impl Facet {
    /// Every facet in Decision T1's order.
    pub const T1_ORDER: [Self; 22] = [
        Self::StopStartingIdle,
        Self::EmoteStop,
        Self::Emote,
        Self::Dialog,
        Self::Sound,
        Self::DirectActors,
        Self::PatrolChangeSpeed,
        Self::PatrolStart,
        Self::PatrolLowPriStart,
        Self::ScriptLink,
        Self::BroadcastRadius,
        Self::RemoteDetectorEnable,
        Self::RemoteDetectorDisable,
        Self::AiHints,
        Self::AddEffect,
        Self::RemoveEffect,
        Self::Scriptlet,
        Self::SetIdleDrama,
        Self::ClearIdleDrama,
        Self::InstanceBillboard,
        Self::DestroySelfFadeOut,
        Self::DestroySelf,
    ];

    /// Decision T5 (§10.2): whether the facet does something in the offline client. The
    /// facets that need server state do nothing (patrols, script links, broadcasts,
    /// detectors, AI hints, effects); `DirectActors` runs only its dialog and emote
    /// (see [`DirectActors`]); a scriptlet runs only with the replicate flag (checked by the
    /// caller); `DestroySelf` and its fade hide the entity until its landblock reloads.
    #[must_use]
    pub fn runs_offline(self) -> bool {
        !matches!(
            self,
            Self::PatrolChangeSpeed
                | Self::PatrolStart
                | Self::PatrolLowPriStart
                | Self::ScriptLink
                | Self::BroadcastRadius
                | Self::RemoteDetectorEnable
                | Self::RemoteDetectorDisable
                | Self::AiHints
                | Self::AddEffect
                | Self::RemoveEffect
        )
    }
}

/// A dialog facet (`Drama_Dialog` or `Drama_DirectActors_Dialog`, §4.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dialog {
    /// Dialog type key (mapper 0x23000158 here; labels give the presentation, T2).
    pub dialog_type: Option<u32>,
    /// Say-channel key (mapper 0x23000113 here; ignored by T2).
    pub say_channel: Option<u32>,
    /// The text-list elements that are StringInfos, in stored order.
    pub lines: Vec<StringInfo>,
    /// Text-list elements of another type (none in the install).
    pub other_elements: usize,
}

impl Dialog {
    fn read(s: &PropertyStruct, ty: Pid, channel: Pid, list: Pid) -> Self {
        let mut lines = Vec::new();
        let mut other_elements = 0;
        for e in s.elements(list) {
            match e.value.string_info() {
                Some(si) => lines.push(si.clone()),
                None => other_elements += 1,
            }
        }
        Self {
            dialog_type: code_of(s, ty),
            say_channel: code_of(s, channel),
            lines,
            other_elements,
        }
    }
}

/// A sound facet (`Drama_Sound`, §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DramaSound {
    /// `Drama_SoundID` (a sound info, prefix 0x2A).
    pub sound: Option<Did>,
    /// `Drama_SoundChannel` as stored (dictionary default 4 `VO`).
    pub channel: Option<u32>,
    /// `Drama_SoundMinRandomDistance`.
    pub min_random_distance: Option<f32>,
    /// `Drama_SoundMaxRandomDistance`.
    pub max_random_distance: Option<f32>,
    /// `Drama_InstanceSound`.
    pub instance_sound: Option<bool>,
}

/// A broadcast facet (`Drama_BroadcastRadius`, §4.6.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Broadcast {
    /// `…_PropertyToMatch`: a PID.
    pub property_to_match: Option<u32>,
    /// `…_Name`: the drama name to perform.
    pub name: Option<String>,
    /// `…_Range` (metres presumably).
    pub range: Option<f32>,
}

/// A director facet (`Drama_DirectActors`, §4.6.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DirectActors {
    /// `…_Actors`: the actor bitfield (bit *i* is key *i* + 1 of mapper 0x230002CF here).
    pub actors: Option<u64>,
    /// `…_Dialog`.
    pub dialog: Option<Dialog>,
    /// `…_Emote` (a script key).
    pub emote: Option<u32>,
    /// `…_PerformDrama`: a drama of the actor.
    pub perform: Option<String>,
    /// `…_PatrolStart`: a patrol link of the actor.
    pub patrol_start: Option<String>,
    /// `…_PatrolLowPriStart`.
    pub patrol_low_pri_start: Option<String>,
}

/// An AI-hint facet (`Drama_AIHints`, §4.8): set bits of the two any-width bitfields
/// (bit *i* stands for key *i* + 1 of mapper 0x2300005C here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AiHints {
    /// `Drama_SetHints` set bits.
    pub set: Vec<u32>,
    /// `Drama_HintMask` set bits.
    pub mask: Vec<u32>,
    /// Bit widths of the two fields as stored ((set, mask); 0 for another value type).
    pub widths: (u32, u32),
}

impl AiHints {
    /// Whether every set bit lies inside the mask (674 of 679 in the install).
    #[must_use]
    pub fn set_within_mask(&self) -> bool {
        self.set.iter().all(|b| self.mask.contains(b))
    }
}

fn bits_of(v: Option<&PropertyValue>) -> (Vec<u32>, u32) {
    match v {
        Some(PropertyValue::BitfieldAny(b)) => (b.set_bits().collect(), b.bits),
        Some(v) => match v.as_bits() {
            Some(b) => ((0..64).filter(|i| b & (1u64 << i) != 0).collect(), 0),
            None => (Vec::new(), 0),
        },
        None => (Vec::new(), 0),
    }
}

/// Every facet field of one action struct, as stored (§3.4). Absent fields are `None`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Facets {
    /// `Drama_Wait_Before`.
    pub wait_before: Option<f32>,
    /// `Drama_Wait_After`.
    pub wait_after: Option<f32>,
    /// `Drama_Wait_Variance`.
    pub wait_variance: Option<f32>,
    /// `Drama_Wait_Emote`.
    pub wait_emote: Option<bool>,
    /// `Drama_Dialog`.
    pub dialog: Option<Dialog>,
    /// `Drama_Emote` (a script key; 0 means none).
    pub emote: Option<u32>,
    /// `Drama_Emote_Stop`.
    pub emote_stop: Option<u32>,
    /// `Drama_Sound`.
    pub sound: Option<DramaSound>,
    /// `Drama_PatrolStart`.
    pub patrol_start: Option<String>,
    /// `Drama_PatrolLowPriStart`.
    pub patrol_low_pri_start: Option<String>,
    /// `Drama_PatrolChangeSpeed`: (name, speed).
    pub patrol_change_speed: Option<(Option<String>, Option<f32>)>,
    /// `Drama_ScriptLink` key (its port number comes from the label, [`script_link_port`]).
    pub script_link: Option<u32>,
    /// `Drama_BroadcastRadius`.
    pub broadcast: Option<Broadcast>,
    /// `Drama_RemoteDetector_Enable` key.
    pub detector_enable: Option<u32>,
    /// `Drama_RemoteDetector_Disable` key.
    pub detector_disable: Option<u32>,
    /// `Drama_DirectActors`.
    pub direct_actors: Option<DirectActors>,
    /// `Drama_AddEffect`: (effect, spellcraft; −1 keeps the effect's own).
    pub add_effect: Option<(Option<Did>, Option<f32>)>,
    /// `Drama_RemoveEffectID`.
    pub remove_effect: Option<Did>,
    /// `Drama_Scriptlet`.
    pub scriptlet: Option<Did>,
    /// `Drama_ReplicateScriptServerToClient`.
    pub replicate: Option<bool>,
    /// `Drama_AIHints`.
    pub ai_hints: Option<AiHints>,
    /// `Drama_StopStartingIdle`.
    pub stop_starting_idle: Option<bool>,
    /// `Drama_SetIdleDrama`.
    pub set_idle: Option<String>,
    /// `Drama_ClearIdleDrama`.
    pub clear_idle: Option<bool>,
    /// `Drama_InstanceBillboard`.
    pub billboard: Option<StringInfo>,
    /// `Drama_Debug_Shout` (designer text; never shown).
    pub debug_shout: Option<String>,
    /// `Drama_DestroySelf`.
    pub destroy_self: Option<bool>,
    /// `Drama_DestroySelf_FadeOut`.
    pub destroy_fade: Option<bool>,
    /// Fields whose PID is none of the above, in stored order.
    pub unknown_fields: Vec<u32>,
}

impl Facets {
    /// The facets present, in Decision T1's order. A facet is present when its field is
    /// stored, whatever its value (a stored `false` flag is present; T1 applies its meaning).
    #[must_use]
    pub fn present(&self) -> Vec<Facet> {
        Facet::T1_ORDER
            .into_iter()
            .filter(|f| match f {
                Facet::StopStartingIdle => self.stop_starting_idle.is_some(),
                Facet::EmoteStop => self.emote_stop.is_some(),
                Facet::Emote => self.emote.is_some(),
                Facet::Dialog => self.dialog.is_some(),
                Facet::Sound => self.sound.is_some(),
                Facet::DirectActors => self.direct_actors.is_some(),
                Facet::PatrolChangeSpeed => self.patrol_change_speed.is_some(),
                Facet::PatrolStart => self.patrol_start.is_some(),
                Facet::PatrolLowPriStart => self.patrol_low_pri_start.is_some(),
                Facet::ScriptLink => self.script_link.is_some(),
                Facet::BroadcastRadius => self.broadcast.is_some(),
                Facet::RemoteDetectorEnable => self.detector_enable.is_some(),
                Facet::RemoteDetectorDisable => self.detector_disable.is_some(),
                Facet::AiHints => self.ai_hints.is_some(),
                Facet::AddEffect => self.add_effect.is_some(),
                Facet::RemoveEffect => self.remove_effect.is_some(),
                Facet::Scriptlet => self.scriptlet.is_some(),
                Facet::SetIdleDrama => self.set_idle.is_some(),
                Facet::ClearIdleDrama => self.clear_idle.is_some(),
                Facet::InstanceBillboard => self.billboard.is_some(),
                Facet::DestroySelfFadeOut => self.destroy_fade.is_some(),
                Facet::DestroySelf => self.destroy_self.is_some(),
            })
            .collect()
    }

    /// Whether the action starts an emote: a non-zero `Drama_Emote`.
    #[must_use]
    pub fn starts_emote(&self) -> bool {
        self.emote.is_some_and(|k| k != 0)
    }
}

/// One action struct (§3.4).
#[derive(Debug, Clone, Copy)]
pub struct Action<'a> {
    pids: &'a DramaPids,
    /// The struct (`None` for a non-struct element).
    pub raw: Option<&'a PropertyStruct>,
    /// The element PID (`Drama_Action`, `Drama_Action_Idle` or `Drama_Action_Instant`).
    pub element_pid: Option<u32>,
}

impl<'a> Action<'a> {
    /// A view over an action struct.
    #[must_use]
    pub fn new(
        pids: &'a DramaPids,
        raw: Option<&'a PropertyStruct>,
        element_pid: Option<u32>,
    ) -> Self {
        Self {
            pids,
            raw,
            element_pid,
        }
    }

    /// Every facet field of the action.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn facets(&self) -> Facets {
        let Some(s) = self.raw else {
            return Facets::default();
        };
        let p = self.pids;
        let string = |pid: Pid| s.string(pid).map(str::to_owned);
        let known = [
            p.wait_before,
            p.wait_after,
            p.wait_variance,
            p.wait_emote,
            p.dialog,
            p.emote,
            p.emote_stop,
            p.sound,
            p.patrol_start,
            p.patrol_low_pri_start,
            p.patrol_change_speed,
            p.script_link,
            p.broadcast,
            p.detector_enable,
            p.detector_disable,
            p.direct_actors,
            p.add_effect,
            p.remove_effect,
            p.scriptlet,
            p.replicate,
            p.ai_hints,
            p.stop_starting_idle,
            p.set_idle,
            p.clear_idle,
            p.billboard,
            p.debug_shout,
            p.destroy_self,
            p.destroy_fade,
            p.action_comment,
        ];
        Facets {
            wait_before: finite(s.float(p.wait_before)),
            wait_after: finite(s.float(p.wait_after)),
            wait_variance: finite(s.float(p.wait_variance)),
            wait_emote: s.boolean(p.wait_emote),
            dialog: s
                .structure(p.dialog)
                .map(|d| Dialog::read(d, p.dialog_type, p.say_channel, p.text_list)),
            emote: code_of(s, p.emote),
            emote_stop: code_of(s, p.emote_stop),
            sound: s.structure(p.sound).map(|d| DramaSound {
                sound: d.did(p.sound_id),
                channel: code_of(d, p.sound_channel),
                min_random_distance: finite(d.float(p.sound_min_distance)),
                max_random_distance: finite(d.float(p.sound_max_distance)),
                instance_sound: d.boolean(p.instance_sound),
            }),
            patrol_start: string(p.patrol_start),
            patrol_low_pri_start: string(p.patrol_low_pri_start),
            patrol_change_speed: s.structure(p.patrol_change_speed).map(|d| {
                (
                    d.string(p.patrol_change_speed_name).map(str::to_owned),
                    finite(d.float(p.patrol_change_speed_speed)),
                )
            }),
            script_link: code_of(s, p.script_link),
            broadcast: s.structure(p.broadcast).map(|d| Broadcast {
                property_to_match: d.property_ref(p.broadcast_property),
                name: d.string(p.broadcast_name).map(str::to_owned),
                range: finite(d.float(p.broadcast_range)),
            }),
            detector_enable: code_of(s, p.detector_enable),
            detector_disable: code_of(s, p.detector_disable),
            direct_actors: s.structure(p.direct_actors).map(|d| DirectActors {
                actors: d.get_pid(p.actors).and_then(PropertyValue::as_bits),
                dialog: d.structure(p.actors_dialog).map(|x| {
                    Dialog::read(
                        x,
                        p.actors_dialog_type,
                        p.actors_say_channel,
                        p.actors_text_list,
                    )
                }),
                emote: code_of(d, p.actors_emote),
                perform: d.string(p.actors_perform).map(str::to_owned),
                patrol_start: d.string(p.actors_patrol_start).map(str::to_owned),
                patrol_low_pri_start: d.string(p.actors_patrol_low_pri_start).map(str::to_owned),
            }),
            add_effect: s.structure(p.add_effect).map(|d| {
                (
                    d.did(p.add_effect_id),
                    finite(d.float(p.add_effect_spellcraft)),
                )
            }),
            remove_effect: s.did(p.remove_effect),
            scriptlet: s.did(p.scriptlet),
            replicate: s.boolean(p.replicate),
            ai_hints: s.structure(p.ai_hints).map(|d| {
                let (set, ws) = bits_of(d.get_pid(p.set_hints));
                let (mask, wm) = bits_of(d.get_pid(p.hint_mask));
                AiHints {
                    set,
                    mask,
                    widths: (ws, wm),
                }
            }),
            stop_starting_idle: s.boolean(p.stop_starting_idle),
            set_idle: string(p.set_idle),
            clear_idle: s.boolean(p.clear_idle),
            billboard: s.string_info(p.billboard).cloned(),
            debug_shout: string(p.debug_shout),
            destroy_self: s.boolean(p.destroy_self),
            destroy_fade: s.boolean(p.destroy_fade),
            unknown_fields: s
                .fields
                .iter()
                .map(|f| f.pid)
                .filter(|&pid| !known.iter().any(|k| k.is(pid)))
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Labels of signals and actors (§4.6)
// ---------------------------------------------------------------------------------------------

/// The port number *N* of a script-link facet, parsed from the key's **label** `Custom<N>`
/// (§4.6.1; the labels are not in key order), 1 ≤ *N* ≤ 20. `None` for any other label.
#[must_use]
pub fn script_link_port(label: &str) -> Option<u32> {
    let n: u32 = label.strip_prefix("Custom")?.parse().ok()?;
    (1..=20).contains(&n).then_some(n)
}

/// The actor number *NN* of an actor label `Actor<NN>` (§4.6.4).
#[must_use]
pub fn actor_number(label: &str) -> Option<u32> {
    label.strip_prefix("Actor")?.parse().ok()
}

/// The actors a director selects (§4.6.4): bit *i* of `mask` is key *i* + 1, whose label
/// `Actor<NN>` gives the actor number. `label_of_key` reads the actor mapper. Bits whose key
/// has no actor label are skipped. The result is sorted and without repeats.
#[must_use]
pub fn actors_of_mask<'l>(mask: u64, label_of_key: impl Fn(u32) -> Option<&'l str>) -> Vec<u32> {
    let mut out: Vec<u32> = (0..64u32)
        .filter(|i| mask & (1u64 << i) != 0)
        .filter_map(|i| label_of_key(i + 1).and_then(actor_number))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

// ---------------------------------------------------------------------------------------------
// Scripted fights and starting idles (§7, §8)
// ---------------------------------------------------------------------------------------------

/// The kind of a scripted-fight transition (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    /// `AI_KungFu_DramaTransition`.
    Drama,
    /// `AI_KungFu_QuestTransition`.
    Quest,
    /// Any other element.
    Other(u32),
}

/// One scripted-fight transition (§7).
#[derive(Debug, Clone, PartialEq)]
pub struct KungFuTransition {
    /// The element kind.
    pub kind: TransitionKind,
    /// `AI_KungFu_Transition_Drama`: a drama of the entity's own list.
    pub drama: Option<String>,
    /// The health ratio.
    pub health_ratio: Option<f32>,
    /// `AI_KungFu_ShouldEndCombat`.
    pub should_end_combat: Option<bool>,
    /// `AI_KungFu_Transition_AlwaysActive`.
    pub always_active: Option<bool>,
    /// `AI_KungFu_Transition_SuppressHealthClamp`.
    pub suppress_clamp: Option<bool>,
}

/// The scripted-fight properties of an entity (§7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KungFu {
    /// `AI_KungFu_StartAsNPC`.
    pub start_as_npc: Option<bool>,
    /// `AI_KungFu_Repeatable` (dictionary default true).
    pub repeatable: Option<bool>,
    /// `AI_KungFu_EnableCombat`.
    pub enable_combat: Option<bool>,
    /// `AI_KungFu_TransitionList`, `None` when absent.
    pub transitions: Option<Vec<KungFuTransition>>,
}

impl KungFu {
    /// Reads the scripted-fight properties of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(labels: &DramaLabels, a: &A) -> Self {
        let k = &labels.kung_fu;
        let ratio = labels.health_ratio();
        let transitions = a.array(k.transition_list).map(|arr| {
            arr.elements
                .iter()
                .map(|e| {
                    let kind = if k.drama_transition.is(e.pid) {
                        TransitionKind::Drama
                    } else if k.quest_transition.is(e.pid) {
                        TransitionKind::Quest
                    } else {
                        TransitionKind::Other(e.pid)
                    };
                    let s = e.value.as_struct();
                    KungFuTransition {
                        kind,
                        drama: s
                            .and_then(|s| s.string(k.transition_drama))
                            .map(str::to_owned),
                        health_ratio: finite(s.and_then(|s| s.float(ratio))),
                        should_end_combat: s.and_then(|s| s.boolean(k.should_end_combat)),
                        always_active: s.and_then(|s| s.boolean(k.always_active)),
                        suppress_clamp: s.and_then(|s| s.boolean(k.suppress_clamp)),
                    }
                })
                .collect()
        });
        Self {
            start_as_npc: a.boolean(k.start_as_npc),
            repeatable: a.boolean(k.repeatable),
            enable_combat: a.boolean(k.enable_combat),
            transitions,
        }
    }

    /// Whether the entity carries any scripted-fight property.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Decision T6 (§10.3 item 5): a drama transition fires once when the entity's health falls
/// to or below `ratio` × `max_health`; `already_fired` re-arms only on respawn.
#[must_use]
pub fn transition_fires(ratio: f32, health: f32, max_health: f32, already_fired: bool) -> bool {
    !already_fired
        && ratio.is_finite()
        && health.is_finite()
        && max_health.is_finite()
        && health <= ratio * max_health
}

/// The starting-idle properties of an entity (§5.3, §8).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StartingIdle {
    /// `Drama_StartingIdle`: the idle drama of the entity's list.
    pub drama: Option<String>,
    /// `NPC_StartingIdleAnimation` (enum 0x23000110 here).
    pub animation: Option<u32>,
    /// `NPC_PlayingStartingIdle`.
    pub playing: Option<bool>,
    /// `NPC_ReturnsToStartingIdle`.
    pub returns: Option<bool>,
    /// `NPC_StartingIdle_AnimStartOffset`.
    pub offset: Option<f32>,
}

impl StartingIdle {
    /// Reads the starting-idle properties of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(labels: &DramaLabels, a: &A) -> Self {
        let i = &labels.idle;
        Self {
            drama: a.string(labels.drama.starting_idle).map(str::to_owned),
            animation: code_of(a, i.animation),
            playing: a.boolean(i.playing),
            returns: a.boolean(i.returns),
            offset: finite(a.float(i.offset)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Decision T1: running one drama (§10.1)
// ---------------------------------------------------------------------------------------------

/// Decision T1 step 3.4: an emote wait lasts at most this many seconds.
pub const EMOTE_WAIT_LIMIT: f32 = 30.0;
/// Decision T1 step 3.5: a wait of this many seconds or more means "until stopped".
pub const UNTIL_STOPPED: f32 = 9_999.0;
/// Decision T1: at most this many perform requests wait behind a running drama.
pub const PERFORM_QUEUE_LEN: usize = 4;
/// Decision T5: pause between two passes of an idle drama, seconds.
pub const IDLE_PAUSE: f32 = 5.0;
/// Decision T2: overhead speech text stays this many seconds.
pub const OVERHEAD_SECONDS: f32 = 10.0;
/// Decision T2: an instance billboard stays this many seconds.
pub const BILLBOARD_SECONDS: f32 = 5.0;
/// Decision T5: the drama an entity performs when it is created (matched by T3).
pub const ENTER_WORLD_DRAMA: &str = "Signal_EnterWorld";
/// Decision T2: dialog lines go to the chat window as this chat type (`Say`).
pub const DIALOG_CHAT_TYPE: u32 = 5;

/// One step of a planned drama (Decision T1).
#[derive(Debug, Clone, PartialEq)]
pub enum DramaStep {
    /// Wait this many seconds.
    Wait(f32),
    /// Apply these facets of action `action` (an index into [`NamedDrama::actions`]), in
    /// this order.
    Apply {
        /// The action index.
        action: usize,
        /// The facets present, in T1 order.
        facets: Vec<Facet>,
    },
    /// Wait until the emote the action started has ended, at most [`EMOTE_WAIT_LIMIT`]
    /// seconds.
    WaitForEmote {
        /// The action index.
        action: usize,
    },
    /// Stay here until the drama is interrupted or the entity removed (a wait of
    /// [`UNTIL_STOPPED`] seconds or more). Nothing follows.
    UntilStopped,
}

/// The steps of performing `drama` under Decision T1 (steps 1–4; setting and clearing
/// `Drama_Performing` and resuming idles, steps 1 and 5, are the runner's). `variance`
/// draws a uniform random number in [0, *v*] for a `Wait_Variance` of *v*; it is called once
/// per action that has a variance.
///
/// An instant drama gives one `Apply`; an empty sequence gives nothing.
pub fn plan_drama(drama: &NamedDrama<'_>, mut variance: impl FnMut(f32) -> f32) -> Vec<DramaStep> {
    let mut steps = Vec::new();
    let actions = drama.actions();
    if drama.kind == DramaKind::Instant {
        if let Some(a) = actions.first() {
            steps.push(DramaStep::Apply {
                action: 0,
                facets: a.facets().present(),
            });
        }
        return steps;
    }
    for (i, a) in actions.iter().enumerate() {
        let f = a.facets();
        let extra = f.wait_variance.filter(|v| *v > 0.0).map_or(0.0, |v| {
            let x = variance(v);
            if x.is_finite() { x.clamp(0.0, v) } else { 0.0 }
        });
        let mut before = f.wait_before.unwrap_or(0.0).max(0.0);
        let mut after = f.wait_after.map(|w| w.max(0.0));
        match after.as_mut() {
            Some(w) => *w += extra,
            None => before += extra,
        }
        if push_wait(&mut steps, before) {
            return steps;
        }
        steps.push(DramaStep::Apply {
            action: i,
            facets: f.present(),
        });
        if push_wait(&mut steps, after.unwrap_or(0.0)) {
            return steps;
        }
        if f.wait_emote == Some(true) && f.starts_emote() {
            steps.push(DramaStep::WaitForEmote { action: i });
        }
    }
    steps
}

/// Pushes a wait; returns true when it is "until stopped" (the plan ends).
fn push_wait(steps: &mut Vec<DramaStep>, seconds: f32) -> bool {
    if seconds >= UNTIL_STOPPED {
        steps.push(DramaStep::UntilStopped);
        return true;
    }
    if seconds > 0.0 {
        steps.push(DramaStep::Wait(seconds));
    }
    false
}

/// What happens to a perform request (Decision T1, last paragraph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerformDecision {
    /// No drama runs: start it.
    Start,
    /// The running drama is interruptible: replace it.
    Replace,
    /// Queue it behind the running drama.
    Queue,
    /// The queue is full: drop it (and log).
    Drop,
    /// The name is not in the entity's list: ignore it (and log).
    Unknown,
}

/// Decision T1's rule for a perform request: `known` whether the name is in the entity's
/// list, `running_interruptible` the running drama's `Drama_Interruptible` (`None` when no
/// drama runs; absent reads false), `queued` the requests already waiting.
#[must_use]
pub fn perform_decision(
    known: bool,
    running_interruptible: Option<Option<bool>>,
    queued: usize,
) -> PerformDecision {
    if !known {
        return PerformDecision::Unknown;
    }
    match running_interruptible {
        None => PerformDecision::Start,
        Some(Some(true)) => PerformDecision::Replace,
        Some(_) if queued < PERFORM_QUEUE_LEN => PerformDecision::Queue,
        Some(_) => PerformDecision::Drop,
    }
}

/// How the offline client shows a dialog line (Decision T2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DialogPresentation {
    /// Show overhead speech text for [`OVERHEAD_SECONDS`].
    pub overhead: bool,
    /// Log to chat as [`DIALOG_CHAT_TYPE`], prefixed by the speaker's name.
    pub chat: bool,
}

/// Decision T2 by the dialog type's mapper label: overhead text for the labels beginning
/// with `Color`; chat for every type except `ColorOnly`. An unknown label (or none) is
/// logged to chat only.
#[must_use]
pub fn dialog_presentation(type_label: Option<&str>) -> DialogPresentation {
    let label = type_label.unwrap_or("");
    DialogPresentation {
        overhead: label.starts_with("Color"),
        chat: label != "ColorOnly",
    }
}

/// Decision T2: the line to speak, uniformly at random. `pick` returns a number in
/// [0, `count`); the result is clamped into range. `None` for an empty list.
#[must_use]
pub fn pick_line(count: usize, pick: impl FnOnce(usize) -> usize) -> Option<usize> {
    (count > 0).then(|| pick(count).min(count - 1))
}

#[cfg(test)]
mod tests;
