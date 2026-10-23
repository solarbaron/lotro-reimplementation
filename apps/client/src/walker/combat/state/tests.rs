// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the combat-visual state machine: the worked examples of
//! `combat-visuals.md` §13 and the rules of §9 and §11. Every DID, key and PID is invented.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::{Did, PropertyType, PropertyValue};
use tessera_gamedata::combat::{
    ChannelVisuals, EffectVisuals, HookVisuals, InductionVisuals, MomentScripts, ScriptKey,
    SkillVisuals,
};
use tessera_net::domain::{CastKind, CastOutcome, HitResult, Vital};
use tessera_scriptgraph::{EntityId, RunId, StoryHookEvent};

use super::*;

const PLAYER: EntityId = EntityId(1);
const DUMMY: EntityId = EntityId(2);
const SKILL: Did = Did(0x7000_0001);
const SKILL_ONE_HOOK: Did = Did(0x7000_0002);
const INDUCED: Did = Did(0x7000_0003);
const CHANNELLED: Did = Did(0x7000_0004);
const INDUCTION: Did = Did(0x7000_1001);
const CHANNEL: Did = Did(0x7000_1002);
const EFFECT: Did = Did(0x7000_2001);
const EFFECT_TWIN: Did = Did(0x7000_2002);
const DOT: Did = Did(0x7000_2003);
const STATE_EFFECT: Did = Did(0x7000_2004);

const B: ScriptKey = 100;
const B_OTHER: ScriptKey = 101;
const R: ScriptKey = 200;
const F1: ScriptKey = 201;
const F2: ScriptKey = 202;
const A: ScriptKey = 300;
const X: ScriptKey = 301;
const Y: ScriptKey = 302;
const K: ScriptKey = 400;
const PULSE: ScriptKey = 401;
const STATE: ScriptKey = 402;
const DIE: ScriptKey = 500;
const REVIVE: ScriptKey = 501;

const P_STORY: u32 = 0x10;
const P_SPEED: u32 = 0x11;
const P_RESULT: u32 = 0x12;
const P_VIOLENT: u32 = 0x13;
const P_MODE: u32 = 0x14;
const P_LIFE: u32 = 0x15;
const P_DEAD: u32 = 0x16;
const P_ATTACKER: u32 = 0x17;
const P_INTENSITY: u32 = 0x18;
const P_ANGLE: u32 = 0x19;
const STANCE_1H: u32 = 0x1000_0005;

#[derive(Default)]
struct Data {
    skills: HashMap<Did, Arc<SkillVisuals>>,
    inductions: HashMap<Did, Arc<InductionVisuals>>,
    channels: HashMap<Did, Arc<ChannelVisuals>>,
    effects: HashMap<Did, Arc<EffectVisuals>>,
    states: HashMap<Did, ScriptKey>,
}

impl VisualData for Data {
    fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>> {
        self.skills.get(&did).cloned()
    }
    fn induction(&self, did: Did) -> Option<Arc<InductionVisuals>> {
        self.inductions.get(&did).cloned()
    }
    fn channel(&self, did: Did) -> Option<Arc<ChannelVisuals>> {
        self.channels.get(&did).cloned()
    }
    fn effect(&self, did: Did) -> Option<Arc<EffectVisuals>> {
        self.effects.get(&did).cloned()
    }
    fn induced_state_script(&self, did: Did) -> Option<ScriptKey> {
        self.states.get(&did).copied()
    }
}

fn hook(number: i64, fx: ScriptKey) -> HookVisuals {
    HookVisuals {
        number: Some(number),
        behaviour_target_hit: R,
        fx_target_hit: fx,
        ..HookVisuals::default()
    }
}

fn data() -> Data {
    let mut d = Data::default();
    d.skills.insert(
        SKILL,
        Arc::new(SkillVisuals {
            did: SKILL,
            self_list: vec![B],
            other_list: vec![B_OTHER],
            hooks: vec![hook(1, F1), hook(2, F2)],
            ..SkillVisuals::default()
        }),
    );
    d.skills.insert(
        SKILL_ONE_HOOK,
        Arc::new(SkillVisuals {
            did: SKILL_ONE_HOOK,
            self_list: vec![B],
            other_list: vec![B],
            hooks: vec![hook(1, F1)],
            ..SkillVisuals::default()
        }),
    );
    d.skills.insert(
        INDUCED,
        Arc::new(SkillVisuals {
            did: INDUCED,
            self_list: vec![B],
            other_list: vec![B],
            hooks: vec![hook(1, F1)],
            induction: Some(INDUCTION),
            ..SkillVisuals::default()
        }),
    );
    d.skills.insert(
        CHANNELLED,
        Arc::new(SkillVisuals {
            did: CHANNELLED,
            self_list: vec![B],
            other_list: vec![B],
            channelling: Some(CHANNEL),
            ..SkillVisuals::default()
        }),
    );
    d.inductions.insert(
        INDUCTION,
        Arc::new(InductionVisuals {
            animation: A,
            fx: X,
            failure_fx: Y,
            ..InductionVisuals::default()
        }),
    );
    let m = |caster, target| MomentScripts {
        caster,
        caster_fx: 0,
        target,
        target_fx: 0,
    };
    d.channels.insert(
        CHANNEL,
        Arc::new(ChannelVisuals {
            start: m(600, 0),
            running: m(601, 602),
            success: m(603, 0),
            failure: m(604, 0),
            abandoned: m(605, 606),
            ..ChannelVisuals::default()
        }),
    );
    for did in [EFFECT, EFFECT_TWIN] {
        d.effects.insert(
            did,
            Arc::new(EffectVisuals {
                fx: K,
                ..EffectVisuals::default()
            }),
        );
    }
    d.effects.insert(
        DOT,
        Arc::new(EffectVisuals {
            interval_fx: PULSE,
            ..EffectVisuals::default()
        }),
    );
    d.effects
        .insert(STATE_EFFECT, Arc::new(EffectVisuals::default()));
    d.states.insert(STATE_EFFECT, STATE);
    d
}

fn vocab() -> CombatVocab {
    let mut types = HashMap::new();
    types.insert(P_STORY, PropertyType::Int);
    types.insert(P_SPEED, PropertyType::Float);
    types.insert(P_VIOLENT, PropertyType::Bool);
    types.insert(P_INTENSITY, PropertyType::Float);
    types.insert(P_ANGLE, PropertyType::Float);
    types.insert(P_ATTACKER, PropertyType::InstanceId);
    CombatVocab {
        story_id: Some(P_STORY),
        speed_multiplier: Some(P_SPEED),
        attack_result: Some(P_RESULT),
        combat_violent: Some(P_VIOLENT),
        entity_mode: Some(P_MODE),
        life_state: Some(P_LIFE),
        dead_script: Some(P_DEAD),
        attacker_id: Some(P_ATTACKER),
        intensity: Some(P_INTENSITY),
        angle: Some(P_ANGLE),
        die_key: Some(DIE),
        revive_key: Some(REVIVE),
        alive: 4,
        dead: 5,
        types,
        ..CombatVocab::default()
    }
}

/// One recorded run: entity, key, parameters, live run.
type Run = (
    EntityId,
    ScriptKey,
    Vec<(u32, PropertyValue)>,
    Option<RunId>,
);

/// A script host that records runs; runs of `long` keys stay live until stopped.
#[derive(Default)]
struct Host {
    available: HashSet<EntityId>,
    long: HashSet<ScriptKey>,
    runs: Vec<Run>,
    live: HashSet<RunId>,
    stopped: Vec<RunId>,
    props: HashMap<(EntityId, u32), PropertyValue>,
    cases: Vec<PropertyValue>,
    hooks: Vec<StoryHookEvent>,
    upcoming: UpcomingView,
    poses: HashMap<EntityId, ([f64; 3], [f32; 2])>,
    next: u64,
    /// The stories the Execute view held when each run started.
    views_at_run: Vec<Vec<i64>>,
}

impl Host {
    fn new() -> Self {
        let mut h = Self::default();
        h.available.extend([PLAYER, DUMMY]);
        h.long.extend([B, B_OTHER, A, X, K, STATE, 600, 601, 602]);
        h
    }

    fn keys_on(&self, e: EntityId) -> Vec<ScriptKey> {
        self.runs.iter().filter(|r| r.0 == e).map(|r| r.1).collect()
    }

    fn run_of(&self, key: ScriptKey) -> Option<RunId> {
        self.runs
            .iter()
            .rev()
            .find(|r| r.1 == key)
            .and_then(|r| r.3)
    }

    fn param(&self, index: usize, pid: u32) -> Option<PropertyValue> {
        self.runs[index]
            .2
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, v)| v.clone())
    }

    fn end(&mut self, key: ScriptKey) {
        if let Some(r) = self.run_of(key) {
            self.live.remove(&r);
        }
    }
}

impl ScriptHost for Host {
    fn available(&mut self, e: EntityId) -> bool {
        self.available.contains(&e)
    }
    fn run(
        &mut self,
        e: EntityId,
        key: ScriptKey,
        params: &[(u32, PropertyValue)],
    ) -> Option<RunId> {
        let mut seen: Vec<i64> = self.upcoming.stories.keys().copied().collect();
        seen.sort_unstable();
        self.views_at_run.push(seen);
        let r = self.long.contains(&key).then(|| {
            self.next += 1;
            RunId(self.next)
        });
        if let Some(r) = r {
            self.live.insert(r);
        }
        self.runs.push((e, key, params.to_vec(), r));
        r
    }
    fn stop(&mut self, run: RunId) {
        if self.live.remove(&run) {
            self.stopped.push(run);
        }
    }
    fn live(&mut self, run: RunId) -> bool {
        self.live.contains(&run)
    }
    fn set_property(&mut self, e: EntityId, pid: u32, value: PropertyValue) {
        self.props.insert((e, pid), value);
    }
    fn clear_property(&mut self, e: EntityId, pid: u32) {
        self.props.remove(&(e, pid));
    }
    fn property(&mut self, e: EntityId, pid: u32) -> Option<PropertyValue> {
        self.props.get(&(e, pid)).cloned()
    }
    fn switch_cases(&mut self, _e: EntityId, key: ScriptKey, pid: u32) -> Vec<PropertyValue> {
        if key == DIE && pid == P_DEAD {
            self.cases.clone()
        } else {
            Vec::new()
        }
    }
    fn pose(&mut self, e: EntityId) -> Option<([f64; 3], [f32; 2])> {
        self.poses.get(&e).copied()
    }
    fn stance(&mut self, _e: EntityId) -> Option<u32> {
        Some(STANCE_1H)
    }
    fn set_upcoming(&mut self, view: UpcomingView) {
        self.upcoming = view;
    }
    fn take_hooks(&mut self) -> Vec<StoryHookEvent> {
        std::mem::take(&mut self.hooks)
    }
}

fn machine() -> CombatState {
    let mut m = CombatState::new(vocab(), 7);
    m.set_player(Some(PLAYER));
    m
}

fn attack(story: i64, h: &mut Host, m: &mut CombatState) {
    h.hooks.push(StoryHookEvent {
        entity: PLAYER,
        hook_type: HOOK_ATTACK,
        story: Some(story),
    });
    m.process_hooks(h);
}

fn damage(m: &mut CombatState, h: &mut Host, d: &Data, skill: Did, result: HitResult, amount: f32) {
    m.vital_delta(
        h,
        d,
        Some(PLAYER),
        DUMMY,
        &Vital::Health,
        amount,
        result,
        Some(skill),
        None,
    );
}

/// `combat-visuals.md` §13.1: two strikes queued for the two Attack hooks of the behaviour.
#[test]
fn two_hook_melee_releases_each_strike_at_its_hook() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    assert_eq!(
        h.keys_on(PLAYER),
        vec![B],
        "the Self list for the local player"
    );
    let story = h
        .param(0, P_STORY)
        .and_then(|v| v.as_f64())
        .expect("story ID") as i64;
    assert_eq!(
        h.param(0, P_SPEED),
        Some(PropertyValue::Float(1.0)),
        "rule 4"
    );
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 100.0);
    damage(&mut m, &mut h, &d, SKILL, HitResult::Critical, 300.0);
    m.cast_ended(&mut h, PLAYER, CastOutcome::Completed);
    assert!(
        h.keys_on(DUMMY).is_empty(),
        "nothing shows before the hooks"
    );
    assert_eq!(m.stats().strikes_waiting, 2);
    assert_eq!(
        h.upcoming.targets(Some(story), &[9, 10]),
        vec![DUMMY],
        "GetUpcomingTargets sees the queued strikes"
    );
    attack(story, &mut h, &mut m);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1]);
    let i = h.runs.len() - 1;
    assert_eq!(h.param(i, P_RESULT), Some(PropertyValue::Enum(9)));
    attack(story, &mut h, &mut m);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1, R, F2]);
    let i = h.runs.len() - 1;
    assert_eq!(h.param(i, P_RESULT), Some(PropertyValue::Enum(10)));
    assert_eq!(
        h.param(i, P_ATTACKER),
        Some(PropertyValue::InstanceId(PLAYER.0))
    );
    // The behaviour ends: the story closes with nothing left.
    h.end(B);
    m.update(&mut h, 1.2);
    assert_eq!(m.stats().open_stories, 0);
    assert_eq!(m.stats().strikes_shown, 2);
}

/// §13.1 variant and §4.3: with a single hook the second strike waits for the end.
#[test]
fn strikes_whose_hook_never_fires_show_at_the_end() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    let story = h.param(0, P_STORY).and_then(|v| v.as_f64()).unwrap() as i64;
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 1.0);
    damage(&mut m, &mut h, &d, SKILL, HitResult::Critical, 1.0);
    attack(story, &mut h, &mut m);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1]);
    h.end(B);
    m.update(&mut h, 1.2);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1, R, F2]);
    // The 30 s limit ends a story whose behaviour loops.
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 1.0);
    m.update(&mut h, 10.0);
    assert_eq!(m.stats().open_stories, 1);
    m.update(&mut h, 31.3);
    assert_eq!(m.stats().open_stories, 0);
    assert_eq!(m.stats().strikes_shown, 3);
}

/// §9.3 steps 2–3: strikes past the skill's hooks, after the hook fired, or without a story
/// show at once.
#[test]
fn strikes_show_at_once_when_nothing_waits_for_them() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    // No story: hook 1 at once.
    damage(&mut m, &mut h, &d, SKILL, HitResult::Miss, 1.0);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1]);
    let i = h.runs.len() - 1;
    assert_eq!(h.param(i, P_RESULT), Some(PropertyValue::Enum(12)));
    // A one-hook skill: the second strike is past its hooks and shows with hook 1.
    h.runs.clear();
    m.cast_started(
        &mut h,
        &d,
        PLAYER,
        CastKind::Cast,
        Some(SKILL_ONE_HOOK),
        Some(DUMMY),
    );
    let story = h.param(0, P_STORY).and_then(|v| v.as_f64()).unwrap() as i64;
    damage(&mut m, &mut h, &d, SKILL_ONE_HOOK, HitResult::Normal, 1.0);
    damage(&mut m, &mut h, &d, SKILL_ONE_HOOK, HitResult::Normal, 1.0);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1], "the second shows at once");
    attack(story, &mut h, &mut m);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1, R, F1]);
    // After the hook fired, a late strike of that number shows at once.
    h.runs.clear();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    let story = h.param(0, P_STORY).and_then(|v| v.as_f64()).unwrap() as i64;
    attack(story, &mut h, &mut m);
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 1.0);
    assert_eq!(h.keys_on(DUMMY), vec![R, F1]);
    // A skill without hook data shows nothing.
    let mut bare = data();
    bare.skills.insert(
        SKILL,
        Arc::new(SkillVisuals {
            did: SKILL,
            self_list: vec![B],
            ..SkillVisuals::default()
        }),
    );
    h.runs.clear();
    m.vital_delta(
        &mut h,
        &bare,
        None,
        DUMMY,
        &Vital::Health,
        1.0,
        HitResult::Normal,
        Some(SKILL),
        None,
    );
    assert!(h.runs.is_empty());
}

#[test]
fn an_interrupted_cast_drops_its_strikes_and_stops_its_behaviour() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 1.0);
    let b = h.run_of(B).unwrap();
    m.cast_ended(&mut h, PLAYER, CastOutcome::Interrupted);
    assert!(h.stopped.contains(&b));
    assert!(h.keys_on(DUMMY).is_empty(), "the strikes are dropped");
    assert_eq!(m.stats().open_stories, 0);
}

/// §13.3: an interrupted induction stops its runs and plays its failure, no behaviour.
#[test]
fn an_interrupted_induction_plays_its_failure() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(
        &mut h,
        &d,
        PLAYER,
        CastKind::Induction,
        Some(INDUCED),
        Some(DUMMY),
    );
    assert_eq!(h.keys_on(PLAYER), vec![A, X]);
    let (a, x) = (h.run_of(A).unwrap(), h.run_of(X).unwrap());
    m.cast_ended(&mut h, PLAYER, CastOutcome::Interrupted);
    assert!(h.stopped.contains(&a) && h.stopped.contains(&x));
    assert_eq!(h.keys_on(PLAYER), vec![A, X, Y]);
    // A completed induction starts the story.
    m.cast_started(
        &mut h,
        &d,
        PLAYER,
        CastKind::Cast,
        Some(INDUCED),
        Some(DUMMY),
    );
    m.cast_ended(&mut h, PLAYER, CastOutcome::Completed);
    assert_eq!(h.keys_on(PLAYER), vec![A, X, Y, A, X, B]);
    assert_eq!(m.stats().open_stories, 1);
}

#[test]
fn a_channel_runs_until_its_end_and_plays_the_matching_end() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(
        &mut h,
        &d,
        PLAYER,
        CastKind::Channel,
        Some(CHANNELLED),
        Some(DUMMY),
    );
    assert_eq!(h.keys_on(PLAYER), vec![600, 601]);
    assert_eq!(h.keys_on(DUMMY), vec![602]);
    // A new cast stops the channel's runs first.
    let running = h.run_of(601).unwrap();
    m.cast_ended(&mut h, PLAYER, CastOutcome::Cancelled);
    assert!(h.stopped.contains(&running));
    assert_eq!(h.keys_on(PLAYER), vec![600, 601, 605]);
    assert_eq!(h.keys_on(DUMMY), vec![602, 606]);
    // Crafting and items play nothing.
    let n = h.runs.len();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Craft, Some(SKILL), None);
    m.cast_started(
        &mut h,
        &d,
        PLAYER,
        CastKind::Cast,
        Some(Did(0x7099_0000)),
        None,
    );
    assert_eq!(h.runs.len(), n);
}

/// §9.4 and §13.4: instances share one run per key; the last removal stops it.
#[test]
fn effect_instances_share_their_run() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.effect_applied(&mut h, &d, DUMMY, 7, Some(EFFECT));
    m.effect_applied(&mut h, &d, DUMMY, 8, Some(EFFECT_TWIN));
    assert_eq!(h.keys_on(DUMMY), vec![K], "one run for both instances");
    let k = h.run_of(K).unwrap();
    m.effect_removed(&mut h, DUMMY, 7);
    assert!(h.live.contains(&k));
    m.effect_removed(&mut h, DUMMY, 8);
    assert!(h.stopped.contains(&k));
    // An induced combat state runs its state script too; a pulse runs at once.
    m.effect_applied(&mut h, &d, DUMMY, 9, Some(STATE_EFFECT));
    assert_eq!(h.keys_on(DUMMY), vec![K, STATE]);
    m.vital_delta(
        &mut h,
        &d,
        None,
        DUMMY,
        &Vital::Health,
        5.0,
        HitResult::Normal,
        Some(DOT),
        None,
    );
    assert_eq!(h.keys_on(DUMMY), vec![K, STATE, PULSE]);
    // Unknown instances are ignored; an unavailable entity runs nothing but keeps the
    // effect, which plays when its figure is ready.
    m.effect_removed(&mut h, DUMMY, 99);
    h.available.remove(&PLAYER);
    m.effect_applied(&mut h, &d, PLAYER, 1, Some(EFFECT));
    assert!(h.keys_on(PLAYER).is_empty());
    h.available.insert(PLAYER);
    m.entity_ready(&mut h, PLAYER);
    assert_eq!(h.keys_on(PLAYER), vec![K]);
}

#[test]
fn stance_defeat_and_revive_set_their_properties() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.combat_state(&mut h, DUMMY, true);
    assert_eq!(
        h.props.get(&(DUMMY, P_VIOLENT)),
        Some(&PropertyValue::Bool(true))
    );
    assert_eq!(
        h.props.get(&(DUMMY, P_MODE)).and_then(|v| v.as_f64()),
        Some(f64::from(STANCE_1H))
    );
    m.effect_applied(&mut h, &d, DUMMY, 3, Some(EFFECT));
    let k = h.run_of(K).unwrap();
    h.cases = vec![
        PropertyValue::Enum(11),
        PropertyValue::Enum(12),
        PropertyValue::Enum(13),
    ];
    m.defeated(&mut h, DUMMY);
    assert!(h.stopped.contains(&k), "effect runs stop at defeat");
    assert!(m.is_defeated(DUMMY));
    assert_eq!(
        h.props.get(&(DUMMY, P_LIFE)).and_then(|v| v.as_f64()),
        Some(5.0)
    );
    let dead = h
        .props
        .get(&(DUMMY, P_DEAD))
        .cloned()
        .expect("a dead script");
    assert!(h.cases.contains(&dead));
    assert_eq!(h.keys_on(DUMMY).last(), Some(&DIE));
    assert!(!h.props.contains_key(&(DUMMY, P_VIOLENT)));
    assert!(!h.props.contains_key(&(DUMMY, P_MODE)));
    m.revived(&mut h, DUMMY);
    assert_eq!(
        h.props.get(&(DUMMY, P_LIFE)).and_then(|v| v.as_f64()),
        Some(4.0)
    );
    assert!(!h.props.contains_key(&(DUMMY, P_DEAD)));
    assert_eq!(h.keys_on(DUMMY).last(), Some(&REVIVE));
    m.combat_state(&mut h, DUMMY, false);
    assert_eq!(
        h.props.get(&(DUMMY, P_VIOLENT)),
        Some(&PropertyValue::Bool(false))
    );
    assert!(!h.props.contains_key(&(DUMMY, P_MODE)));
}

/// The Self list for the local player, the Other list for others (§3.1).
#[test]
fn others_use_the_other_list() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, DUMMY, CastKind::Cast, Some(SKILL), Some(PLAYER));
    assert_eq!(h.keys_on(DUMMY), vec![B_OTHER]);
    // A second story of the same skill ends the first one.
    m.cast_started(&mut h, &d, DUMMY, CastKind::Cast, Some(SKILL), Some(PLAYER));
    assert_eq!(m.stats().open_stories, 1);
}

#[test]
fn rule_8_upcoming_targets_and_results() {
    let mut v = UpcomingView {
        hit: 9,
        ..UpcomingView::default()
    };
    v.stories.insert(1, (Vec::new(), Some(DUMMY), false));
    v.stories.insert(3, (Vec::new(), Some(DUMMY), true));
    v.stories
        .insert(2, (vec![(DUMMY, 12), (PLAYER, 9), (DUMMY, 9)], None, true));
    assert_eq!(v.targets(Some(1), &[9, 10]), vec![DUMMY]);
    assert!(v.targets(Some(1), &[12]).is_empty());
    // Every received strike shown already: no target fallback.
    assert!(v.targets(Some(3), &[9, 10]).is_empty());
    assert_eq!(v.targets(Some(2), &[9]), vec![PLAYER, DUMMY]);
    assert_eq!(v.targets(Some(2), &[]), vec![DUMMY, PLAYER]);
    assert!(v.targets(None, &[9]).is_empty());
    assert_eq!(v.hit_result(Some(2)), 12);
    assert_eq!(v.hit_result(Some(1)), 9);
}

#[test]
fn rule_5_angle_and_intensity() {
    // Facing +Y: an attacker straight ahead is 0°, to the left (−X) +90°, behind 180°.
    let f = [0.0, 1.0];
    assert!(attacker_angle([0.0; 3], f, [0.0, 5.0, 0.0]).abs() < 1e-4);
    assert!((attacker_angle([0.0; 3], f, [-5.0, 0.0, 0.0]) - 90.0).abs() < 1e-4);
    assert!((attacker_angle([0.0; 3], f, [5.0, 0.0, 0.0]) + 90.0).abs() < 1e-4);
    assert!((attacker_angle([0.0; 3], f, [0.0, -5.0, 0.0]) - 180.0).abs() < 1e-4);
    assert_eq!(attacker_angle([0.0; 3], f, [0.0; 3]), 0.0);
    assert_eq!(strike_intensity(50.0, Some(200.0)), 0.25);
    assert_eq!(strike_intensity(500.0, Some(200.0)), 1.0);
    assert_eq!(strike_intensity(50.0, None), 1.0);
    assert_eq!(strike_intensity(f32::NAN, Some(200.0)), 1.0);
    // The machine passes them as run parameters.
    let d = data();
    let mut h = Host::new();
    h.poses.insert(DUMMY, ([0.0; 3], f));
    h.poses.insert(PLAYER, ([-3.0, 0.0, 0.0], [1.0, 0.0]));
    let mut m = machine();
    m.vital_changed(DUMMY, &Vital::Health, 400.0);
    damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 100.0);
    let i = h.runs.len() - 1;
    assert_eq!(h.param(i, P_INTENSITY), Some(PropertyValue::Float(0.25)));
    assert_eq!(h.param(i, P_ANGLE), Some(PropertyValue::Float(90.0)));
}

#[test]
fn hostile_volumes_stay_bounded() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    for _ in 0..2_000 {
        damage(&mut m, &mut h, &d, SKILL, HitResult::Normal, 1.0);
    }
    assert!(m.stats().strikes_waiting <= MAX_STRIKES_PER_STORY);
    for i in 0..5_000u64 {
        m.effect_applied(&mut h, &d, EntityId(10 + i), i, Some(EFFECT));
    }
    assert!(m.entities.len() <= MAX_ENTITIES);
    // Hooks of unknown stories, of other performers and of unknown types do nothing.
    for (story, e, ty) in [
        (Some(999), PLAYER, 2),
        (Some(1), DUMMY, 2),
        (Some(1), PLAYER, 7),
        (None, PLAYER, 2),
    ] {
        h.hooks.push(StoryHookEvent {
            entity: e,
            hook_type: ty,
            story,
        });
    }
    m.process_hooks(&mut h);
    m.entity_removed(&mut h, PLAYER);
    assert_eq!(m.stats().open_stories, 0);
}

/// Rule 8 with §9.2: the story is published before its behaviour starts, so an Execute node
/// at the root of the behaviour sees it (its target when no strike arrived yet).
#[test]
fn the_story_is_visible_to_its_behaviour_from_the_start() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.cast_started(&mut h, &d, PLAYER, CastKind::Cast, Some(SKILL), Some(DUMMY));
    let i = h.runs.iter().position(|r| r.1 == B).expect("behaviour ran");
    assert_eq!(h.views_at_run[i], vec![1]);
    // A behaviour that does not stay live ends its story at once.
    let mut h2 = Host::new();
    h2.long.remove(&B);
    let mut m2 = machine();
    m2.cast_started(
        &mut h2,
        &d,
        PLAYER,
        CastKind::Cast,
        Some(SKILL),
        Some(DUMMY),
    );
    assert_eq!(m2.stats().open_stories, 0);
    assert!(h2.upcoming.stories.is_empty());
}

/// §11 rule 5 / §4.4: the attacker travels as an instance ID, exactly (no float on the way).
#[test]
fn the_attacker_id_is_an_exact_instance_id() {
    let d = data();
    let mut h = Host::new();
    let big = EntityId((1 << 60) + 3);
    h.available.insert(big);
    let mut m = machine();
    m.vital_delta(
        &mut h,
        &d,
        Some(big),
        DUMMY,
        &Vital::Health,
        1.0,
        HitResult::Normal,
        Some(SKILL),
        None,
    );
    let i = h.runs.iter().position(|r| r.1 == R).expect("hit reaction");
    assert_eq!(
        h.param(i, P_ATTACKER),
        Some(PropertyValue::InstanceId(big.0))
    );
}

/// A figure that left the runtime (a region change) gets its persistent visuals again when
/// it is spawned again; entities with state are listed for the walker's clean-up.
#[test]
fn a_lost_figure_gets_its_effects_again() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    m.effect_applied(&mut h, &d, DUMMY, 7, Some(EFFECT));
    assert_eq!(h.keys_on(DUMMY), vec![K]);
    m.entity_lost(DUMMY);
    m.entity_ready(&mut h, DUMMY);
    assert_eq!(h.keys_on(DUMMY), vec![K, K], "restarted on the new figure");
    assert_eq!(m.known_entities(), vec![DUMMY]);
    m.entity_removed(&mut h, DUMMY);
    assert!(m.known_entities().is_empty());
}

/// `ready_keys` names exactly the scripts `entity_ready` runs: nothing for an entity with no
/// state or only a stance (so its figure is spawned without waiting for any clip), the
/// effect runs a lost figure gets again, and the death script of a defeated entity.
#[test]
fn ready_keys_are_the_scripts_entity_ready_runs() {
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    assert!(
        m.ready_keys(DUMMY).is_empty(),
        "no state, nothing to wait for"
    );
    m.combat_state(&mut h, DUMMY, true);
    assert!(m.ready_keys(DUMMY).is_empty(), "a stance runs no script");
    m.effect_applied(&mut h, &d, DUMMY, 7, Some(EFFECT));
    assert!(m.ready_keys(DUMMY).is_empty(), "the effect run is live");
    m.entity_lost(DUMMY);
    assert_eq!(m.ready_keys(DUMMY), vec![K]);
    let before = h.keys_on(DUMMY).len();
    m.entity_ready(&mut h, DUMMY);
    assert_eq!(h.keys_on(DUMMY)[before..], [K]);
    assert!(m.ready_keys(DUMMY).is_empty());
    m.defeated(&mut h, DUMMY);
    m.entity_lost(DUMMY);
    assert_eq!(m.ready_keys(DUMMY), vec![DIE]);
    let before = h.keys_on(DUMMY).len();
    m.entity_ready(&mut h, DUMMY);
    assert_eq!(h.keys_on(DUMMY)[before..], [DIE]);
}

/// The spawn gate of the combat layer (`super::super::spawn_waits_for_clips`): a figure
/// whose persistent state runs nothing is spawned at once, without asking whether any clip
/// is decoded (its start-up and looping clips are not waited for); one whose state runs
/// scripts waits exactly for those scripts' clips.
#[test]
fn a_figure_with_nothing_to_run_waits_for_no_clip() {
    use super::super::{ClipPrefetch, spawn_waits_for_clips};
    let d = data();
    let mut h = Host::new();
    let mut m = machine();
    let never = |_: &ClipPrefetch| -> bool { panic!("no clip is asked for") };
    assert!(!spawn_waits_for_clips(&m, DUMMY, never), "no state");
    m.combat_state(&mut h, DUMMY, true);
    assert!(!spawn_waits_for_clips(&m, DUMMY, never), "a stance only");
    m.effect_applied(&mut h, &d, DUMMY, 7, Some(EFFECT));
    m.entity_lost(DUMMY);
    let mut asked = Vec::new();
    let waits = spawn_waits_for_clips(&m, DUMMY, |r| {
        asked.push(r.clone());
        false
    });
    assert!(waits, "the effect run's clips are not decoded");
    assert_eq!(
        asked,
        vec![ClipPrefetch {
            keys: vec![K],
            overrides: Vec::new(),
        }]
    );
    assert!(!spawn_waits_for_clips(&m, DUMMY, |_| true), "decoded");
}
