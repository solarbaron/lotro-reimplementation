// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat-visual state machine of `combat-visuals.md` §9 (stories, strikes and hooks,
//! inductions and channels, effects, stance, defeat and revive), over two narrow traits so
//! that it runs without a renderer or an install: [`VisualData`] answers the script
//! references of records (`tessera-gamedata`'s combat visuals), [`ScriptHost`] runs scripts on
//! entities (the walker's script-table runtime).
//!
//! Entities here are the runtime's [`EntityId`]s; the walker maps session entities to them.
//! Every rule cites the spec section it follows; Tessera rules are `combat-visuals.md` §11.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_formats::{Did, PropertyType, PropertyValue};
use tessera_gamedata::combat::{
    ChannelVisuals, EffectVisuals, InductionVisuals, MAX_STORIES_PER_PERFORMER,
    MAX_STRIKES_PER_STORY, MomentScripts, STORY_TIMEOUT, ScriptKey, SkillVisuals, StrikeResult,
};
use tessera_net::domain::{CastKind, CastOutcome, HitResult, Vital};
use tessera_particles::Ran2;
use tessera_scriptgraph::{EntityId, RunId, StoryHookEvent};

/// `StoryHookScript_Type` of an Attack hook (`combat-visuals.md` §1a).
pub const HOOK_ATTACK: u32 = 2;
/// `StoryHookScript_Type` of a Defend hook.
pub const HOOK_DEFEND: u32 = 1;
/// The strike kind every Tessera strike has (§11 rule 3).
pub const STRIKE_KIND: u32 = 1;
/// `ScriptSystem_Snd_StrikingImplementType` without a mode or value (§4.4).
pub const DEFAULT_STRIKING_IMPLEMENT: u32 = 25;
/// `Combat_SkillMode` keys (mapper 0x2300002F, §2.2): melee, ranged, thrown.
pub const SKILL_MODE_MELEE: u32 = 1;
/// Ranged.
pub const SKILL_MODE_RANGED: u32 = 2;
/// Thrown.
pub const SKILL_MODE_THROWN: u32 = 3;
/// Most story-hook events handled per call (a hook that starts scripts whose hooks fire at
/// once could otherwise loop).
pub const MAX_HOOK_ROUNDS: usize = 64;
/// Most entities whose combat state is kept (hostile-input bound).
pub const MAX_ENTITIES: usize = 4096;
/// Most effect instances kept per entity (hostile-input bound).
pub const MAX_EFFECTS_PER_ENTITY: usize = 256;

/// The script references of records (`tessera-gamedata` combat visuals).
pub trait VisualData {
    /// A skill's visuals.
    fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>>;
    /// An induction record's scripts.
    fn induction(&self, did: Did) -> Option<Arc<InductionVisuals>>;
    /// A channelling record's scripts.
    fn channel(&self, did: Did) -> Option<Arc<ChannelVisuals>>;
    /// An effect's visuals.
    fn effect(&self, did: Did) -> Option<Arc<EffectVisuals>>;
    /// The script of the combat state an effect induces (§9.4).
    fn induced_state_script(&self, did: Did) -> Option<ScriptKey>;
}

/// What the machine asks of the script runtime.
pub trait ScriptHost {
    /// Whether the entity is drawn as a figure and runs scripts (§9: runs on other entities
    /// are skipped).
    fn available(&mut self, e: EntityId) -> bool;
    /// Starts `key` on `e` with run parameters; the run when it stays live.
    fn run(
        &mut self,
        e: EntityId,
        key: ScriptKey,
        params: &[(u32, PropertyValue)],
    ) -> Option<RunId>;
    /// Stops a run.
    fn stop(&mut self, run: RunId);
    /// Whether a run is live.
    fn live(&mut self, run: RunId) -> bool;
    /// Sets an entity property.
    fn set_property(&mut self, e: EntityId, pid: u32, value: PropertyValue);
    /// Clears an entity property (back to its stored value or default).
    fn clear_property(&mut self, e: EntityId, pid: u32);
    /// An entity property's value.
    fn property(&mut self, e: EntityId, pid: u32) -> Option<PropertyValue>;
    /// The case values of the Switch on `pid` in the definition of `key` on `e`.
    fn switch_cases(&mut self, e: EntityId, key: ScriptKey, pid: u32) -> Vec<PropertyValue>;
    /// The entity's position and forward direction (horizontal, unit) in a shared frame.
    fn pose(&mut self, e: EntityId) -> Option<([f64; 3], [f32; 2])>;
    /// The `Entity_Mode` key of the entity's stance (§11 rule 6).
    fn stance(&mut self, e: EntityId) -> Option<u32>;
    /// The strikes of open stories changed: the Execute callbacks answer from this (§11
    /// rule 8).
    fn set_upcoming(&mut self, view: UpcomingView);
    /// Story-hook events the runtime produced since the last call, oldest first.
    fn take_hooks(&mut self) -> Vec<StoryHookEvent>;
}

/// The property IDs and keys the machine uses, looked up by name in the data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatVocab {
    /// `StorySystem_StoryID`.
    pub story_id: Option<u32>,
    /// `Combat_AnimationSpeedMultiplier`.
    pub speed_multiplier: Option<u32>,
    /// `Combat_SkillMode`.
    pub skill_mode: Option<u32>,
    /// `Mount_Is_Mounted`.
    pub mounted: Option<u32>,
    /// `Entity_Mode`.
    pub entity_mode: Option<u32>,
    /// `Combat_Violent`.
    pub combat_violent: Option<u32>,
    /// `LifeState`.
    pub life_state: Option<u32>,
    /// `Death_DeadScript`.
    pub dead_script: Option<u32>,
    /// `Death_DiedWhileKnockedDown`.
    pub died_knocked_down: Option<u32>,
    /// `CombatStrikeData_AttackerID`.
    pub attacker_id: Option<u32>,
    /// `CombatStrikeData_AttackResult`.
    pub attack_result: Option<u32>,
    /// `CombatStrikeData_DamageType`.
    pub damage_type: Option<u32>,
    /// `CombatStrikeData_Vital`.
    pub vital: Option<u32>,
    /// `CombatStrikeData_TargetToAttackerAngle`.
    pub angle: Option<u32>,
    /// `CombatStrikeData_Intensity`.
    pub intensity: Option<u32>,
    /// `ScriptSystem_Snd_StrikingImplementType`.
    pub striking_implement: Option<u32>,
    /// `ScriptSystem_Snd_StruckSurfaceType`.
    pub struck_surface: Option<u32>,
    /// `Combat_DefaultStrikingImplementType_Melee`, `…_Ranged`, `…_Thrown`.
    pub implement_by_mode: [Option<u32>; 3],
    /// `Material_WeaponMaterial`, `Material_OffHandMaterial`, `Material_TorsoMaterial`,
    /// `Content_IAmMadeOf`.
    pub materials: [Option<u32>; 4],
    /// The `Death@Agent::*Die` key.
    pub die_key: Option<ScriptKey>,
    /// The `Death@Agent::*Revive` key.
    pub revive_key: Option<ScriptKey>,
    /// Attack-result keys by [`StrikeResult`] (the observed keys when the mapper is absent).
    pub results: BTreeMap<String, u32>,
    /// `LifeState` of an alive and a dead entity (§1a).
    pub alive: u32,
    /// Dead.
    pub dead: u32,
    /// Property types, for typed values.
    pub types: HashMap<u32, PropertyType>,
}

impl CombatVocab {
    /// The attack-result key of a feed result (§9.3 step 1).
    #[must_use]
    pub fn result_key(&self, r: StrikeResult) -> u32 {
        self.results
            .get(r.label())
            .copied()
            .unwrap_or_else(|| r.observed_key())
    }

    /// A number as a value of the property's type (`Enum` without a type).
    #[must_use]
    pub fn value(&self, pid: u32, x: f64) -> PropertyValue {
        number_value(self.types.get(&pid).copied(), x)
    }
}

/// A number as a property value of type `ty`.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn number_value(ty: Option<PropertyType>, x: f64) -> PropertyValue {
    let x = if x.is_finite() { x } else { 0.0 };
    match ty {
        Some(PropertyType::Float) => PropertyValue::Float(x as f32),
        Some(PropertyType::Int) => PropertyValue::Int(x as i32),
        Some(PropertyType::Int64) => PropertyValue::Int64(x as i64),
        Some(PropertyType::Bool) => PropertyValue::Bool(x != 0.0),
        Some(PropertyType::Bitfield32) => PropertyValue::Bitfield32(x as u32),
        Some(PropertyType::Bitfield64) => PropertyValue::Bitfield64(x as u64),
        Some(PropertyType::InstanceId) => PropertyValue::InstanceId(x as u64),
        Some(PropertyType::DidRef) => PropertyValue::DidRef(Did(x as u32)),
        _ => PropertyValue::Enum(x as u32),
    }
}

/// The feed's hit result as a strike result (§9.3 step 1).
#[must_use]
pub fn strike_result(r: HitResult) -> StrikeResult {
    match r {
        HitResult::Normal => StrikeResult::Normal,
        HitResult::Critical => StrikeResult::Critical,
        HitResult::Miss => StrikeResult::Miss,
        HitResult::Evade => StrikeResult::Evade,
        HitResult::Parry => StrikeResult::Parry,
        HitResult::Block => StrikeResult::Block,
        HitResult::Resist => StrikeResult::Resist,
        HitResult::Immune => StrikeResult::Immune,
    }
}

/// One strike (§4.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    /// The strike kind (always [`STRIKE_KIND`], rule 3).
    pub kind: u32,
    /// The hook number that releases it.
    pub hook: i64,
    /// The entity that shows it.
    pub target: EntityId,
    /// The attacker (`None` without a source).
    pub attacker: Option<EntityId>,
    /// The attack-result key.
    pub result: u32,
    /// The damage-type key (0 Undef).
    pub damage_type: u32,
    /// The vital key (1 health, 2 power, 0 other).
    pub vital: u32,
    /// The amount.
    pub amount: f32,
}

/// One story (§1a, §9.2).
#[derive(Debug, Clone)]
struct Story {
    skill: Arc<SkillVisuals>,
    performer: EntityId,
    target: Option<EntityId>,
    started: f64,
    run: Option<RunId>,
    strikes: Vec<Strike>,
    attack: u32,
    defend: u32,
    received: HashMap<EntityId, u32>,
}

/// One story in an [`UpcomingView`]: its strikes not yet shown (target, attack-result key),
/// in order, its target, and whether it has received any strike so far (shown or not).
pub type StoryView = (Vec<(EntityId, u32)>, Option<EntityId>, bool);

/// The Execute callbacks' view of the open stories (§11 rule 8): per story ID, the strikes
/// not yet shown (target, attack-result key) in order, and the story's target.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpcomingView {
    /// By story ID.
    pub stories: HashMap<i64, StoryView>,
    /// The `Hit` attack-result key.
    pub hit: u32,
}

impl UpcomingView {
    /// `GetUpcomingTargets` (§11 rule 8): the targets of the story's strikes not yet shown
    /// whose result is in `filter` (an empty filter takes all), in strike order, without
    /// duplicates; when the story has received no strike yet and `Hit` is in the filter,
    /// the story's target alone (a story whose strikes were all shown answers nothing).
    #[must_use]
    pub fn targets(&self, story: Option<i64>, filter: &[u32]) -> Vec<EntityId> {
        let Some((strikes, target, received)) = story.and_then(|s| self.stories.get(&s)) else {
            return Vec::new();
        };
        if !received {
            return match target {
                Some(t) if filter.contains(&self.hit) => vec![*t],
                _ => Vec::new(),
            };
        }
        let mut out: Vec<EntityId> = Vec::new();
        for (t, r) in strikes {
            if (filter.is_empty() || filter.contains(r)) && !out.contains(t) {
                out.push(*t);
            }
        }
        out
    }

    /// `GetUpcomingHitResult` (§11 rule 8): the result of the first strike not yet shown,
    /// or `Hit`.
    #[must_use]
    pub fn hit_result(&self, story: Option<i64>) -> u32 {
        story
            .and_then(|s| self.stories.get(&s))
            .and_then(|(strikes, _, _)| strikes.first())
            .map_or(self.hit, |(_, r)| *r)
    }
}

/// An active cast with its runs (§9.1).
#[derive(Debug, Clone)]
enum ActiveCast {
    /// A plain cast: its story (if one opened).
    Cast { story: Option<i64> },
    /// An induction.
    Induction {
        skill: Arc<SkillVisuals>,
        visuals: Arc<InductionVisuals>,
        target: Option<EntityId>,
        runs: Vec<RunId>,
    },
    /// A channel.
    Channel {
        visuals: Arc<ChannelVisuals>,
        target: Option<EntityId>,
        runs: Vec<RunId>,
    },
}

/// What the machine keeps of one entity.
#[derive(Debug, Clone, Default)]
struct EntityState {
    in_combat: bool,
    defeated: bool,
    dead_script: Option<PropertyValue>,
    /// Effect instances: instance → keys started for it.
    effects: BTreeMap<u64, Vec<ScriptKey>>,
    /// Maximum of each vital (1 health, 2 power) from the last `VitalChanged`.
    vital_max: HashMap<u32, f32>,
}

/// One shared effect run (§9.4: instances running the same key on the same entity share it).
#[derive(Debug, Clone, Copy)]
struct SharedRun {
    run: Option<RunId>,
    count: u32,
}

/// Counters for the HUD, the frame log and tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CombatStats {
    /// Stories started (a behaviour ran).
    pub stories: u64,
    /// Open stories now.
    pub open_stories: usize,
    /// Strikes shown.
    pub strikes_shown: u64,
    /// Strikes waiting for their hook now.
    pub strikes_waiting: usize,
    /// Attack hooks received for an open story.
    pub hooks: u64,
    /// Script runs started (any outcome).
    pub runs: u64,
    /// Script runs that stayed live.
    pub live_runs: u64,
    /// Effect runs live now.
    pub effect_runs: usize,
}

/// The combat-visual state machine (module docs).
#[derive(Debug)]
pub struct CombatState {
    vocab: CombatVocab,
    player: Option<EntityId>,
    next_story: i64,
    stories: BTreeMap<i64, Story>,
    casts: HashMap<EntityId, ActiveCast>,
    entities: HashMap<EntityId, EntityState>,
    shared: HashMap<(EntityId, ScriptKey), SharedRun>,
    rng: Ran2,
    now: f64,
    stats: CombatStats,
}

fn vital_key(v: &Vital) -> u32 {
    match v {
        Vital::Health => 1,
        Vital::Power => 2,
        _ => 0,
    }
}

impl CombatState {
    /// A machine with `vocab`, its random draws seeded with `seed` (§11 rule 2).
    #[must_use]
    pub fn new(vocab: CombatVocab, seed: u32) -> Self {
        Self {
            vocab,
            player: None,
            next_story: 1,
            stories: BTreeMap::new(),
            casts: HashMap::new(),
            entities: HashMap::new(),
            shared: HashMap::new(),
            rng: Ran2::new(seed),
            now: 0.0,
            stats: CombatStats::default(),
        }
    }

    /// The vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &CombatVocab {
        &self.vocab
    }

    /// The local player (the Self list of §3.1).
    pub fn set_player(&mut self, player: Option<EntityId>) {
        self.player = player;
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> CombatStats {
        let mut s = self.stats;
        s.open_stories = self.stories.len();
        s.strikes_waiting = self.stories.values().map(|s| s.strikes.len()).sum();
        s.effect_runs = self.shared.values().filter(|r| r.run.is_some()).count();
        s
    }

    /// The current time (seconds of the caller's clock).
    pub fn set_time(&mut self, now: f64) {
        if now.is_finite() {
            self.now = now;
        }
    }

    fn entity(&mut self, e: EntityId) -> Option<&mut EntityState> {
        if !self.entities.contains_key(&e) && self.entities.len() >= MAX_ENTITIES {
            return None;
        }
        Some(self.entities.entry(e).or_default())
    }

    fn run(
        &mut self,
        host: &mut dyn ScriptHost,
        e: Option<EntityId>,
        key: ScriptKey,
        params: &[(u32, PropertyValue)],
    ) -> Option<RunId> {
        let e = e?;
        if key == 0 || !host.available(e) {
            return None;
        }
        self.stats.runs += 1;
        let r = host.run(e, key, params);
        self.stats.live_runs += u64::from(r.is_some());
        r
    }

    fn publish(&self, host: &mut dyn ScriptHost) {
        let mut v = UpcomingView {
            hit: self.vocab.result_key(StrikeResult::Normal),
            ..UpcomingView::default()
        };
        for (id, s) in &self.stories {
            v.stories.insert(
                *id,
                (
                    s.strikes.iter().map(|k| (k.target, k.result)).collect(),
                    s.target,
                    !s.received.is_empty() || !s.strikes.is_empty(),
                ),
            );
        }
        host.set_upcoming(v);
    }

    // ----- casts (§9.1) -----

    /// `CastStarted` (§9.1): `what` is the cast's record when it is one.
    pub fn cast_started(
        &mut self,
        host: &mut dyn ScriptHost,
        data: &dyn VisualData,
        entity: EntityId,
        cast: CastKind,
        what: Option<Did>,
        target: Option<EntityId>,
    ) {
        // A new cast stops the runs of a running induction or channel first.
        self.stop_cast_runs(host, entity);
        let Some(skill) = what.and_then(|d| data.skill(d)) else {
            return;
        };
        if self.casts.len() >= MAX_ENTITIES {
            // Hostile-input bound: no further caster is tracked.
            return;
        }
        match cast {
            CastKind::Craft => {}
            CastKind::Cast if skill.induction.is_none() => {
                let story = self.start_story(host, entity, &skill, target);
                self.casts.insert(entity, ActiveCast::Cast { story });
            }
            CastKind::Cast | CastKind::Induction => {
                let Some(visuals) = skill.induction.and_then(|d| data.induction(d)) else {
                    // An induction without its record: the story starts at completion.
                    self.casts.insert(
                        entity,
                        ActiveCast::Induction {
                            skill,
                            visuals: Arc::new(InductionVisuals::default()),
                            target,
                            runs: Vec::new(),
                        },
                    );
                    return;
                };
                let mounted = self.mounted(host, entity);
                let anim = if mounted && visuals.mounted_animation != 0 {
                    visuals.mounted_animation
                } else {
                    visuals.animation
                };
                let mut runs = Vec::new();
                runs.extend(self.run(host, Some(entity), anim, &[]));
                runs.extend(self.run(host, Some(entity), visuals.fx, &[]));
                runs.extend(self.run(host, target, visuals.target_animation, &[]));
                runs.extend(self.run(host, target, visuals.target_fx, &[]));
                self.casts.insert(
                    entity,
                    ActiveCast::Induction {
                        skill,
                        visuals,
                        target,
                        runs,
                    },
                );
            }
            CastKind::Channel => {
                let Some(visuals) = skill.channelling.and_then(|d| data.channel(d)) else {
                    return;
                };
                let mut runs = Vec::new();
                for m in [visuals.start, visuals.running] {
                    runs.extend(self.moment(host, entity, target, m));
                }
                self.casts.insert(
                    entity,
                    ActiveCast::Channel {
                        visuals,
                        target,
                        runs,
                    },
                );
            }
        }
    }

    fn moment(
        &mut self,
        host: &mut dyn ScriptHost,
        entity: EntityId,
        target: Option<EntityId>,
        m: MomentScripts,
    ) -> Vec<RunId> {
        let mut runs = Vec::new();
        runs.extend(self.run(host, Some(entity), m.caster, &[]));
        runs.extend(self.run(host, Some(entity), m.caster_fx, &[]));
        runs.extend(self.run(host, target, m.target, &[]));
        runs.extend(self.run(host, target, m.target_fx, &[]));
        runs
    }

    fn stop_cast_runs(&mut self, host: &mut dyn ScriptHost, entity: EntityId) {
        match self.casts.remove(&entity) {
            Some(ActiveCast::Induction { runs, .. } | ActiveCast::Channel { runs, .. }) => {
                for r in runs {
                    host.stop(r);
                }
            }
            // A plain cast's story keeps running to its end.
            Some(ActiveCast::Cast { .. }) | None => {}
        }
    }

    /// `CastEnded` (§9.1).
    pub fn cast_ended(
        &mut self,
        host: &mut dyn ScriptHost,
        entity: EntityId,
        outcome: CastOutcome,
    ) {
        match self.casts.remove(&entity) {
            None => {}
            Some(ActiveCast::Cast { story }) => {
                if outcome != CastOutcome::Completed
                    && let Some(id) = story
                {
                    self.end_story(host, id, false);
                }
            }
            Some(ActiveCast::Induction {
                skill,
                visuals,
                target,
                runs,
            }) => {
                for r in runs {
                    host.stop(r);
                }
                if outcome == CastOutcome::Completed {
                    self.start_story(host, entity, &skill, target);
                } else {
                    self.run(host, Some(entity), visuals.failure_fx, &[]);
                    self.run(host, target, visuals.target_failure_fx, &[]);
                }
            }
            Some(ActiveCast::Channel {
                visuals,
                target,
                runs,
            }) => {
                for r in runs {
                    host.stop(r);
                }
                let m = match outcome {
                    CastOutcome::Completed => visuals.success,
                    CastOutcome::Interrupted => visuals.failure,
                    CastOutcome::Cancelled => visuals.abandoned,
                };
                self.moment(host, entity, target, m);
            }
        }
        self.publish(host);
    }

    fn mounted(&mut self, host: &mut dyn ScriptHost, e: EntityId) -> bool {
        self.vocab
            .mounted
            .and_then(|p| host.property(e, p))
            .and_then(|v| v.as_f64())
            .is_some_and(|x| x != 0.0)
    }

    // ----- stories (§9.2) -----

    /// Starts the story of `skill` on `performer` (§9.2); the story's ID when one opened.
    fn start_story(
        &mut self,
        host: &mut dyn ScriptHost,
        performer: EntityId,
        skill: &Arc<SkillVisuals>,
        target: Option<EntityId>,
    ) -> Option<i64> {
        // At most one story per (performer, skill).
        let previous: Vec<i64> = self
            .stories
            .iter()
            .filter(|(_, s)| s.performer == performer && s.skill.did == skill.did)
            .map(|(id, _)| *id)
            .collect();
        for id in previous {
            self.end_story(host, id, true);
        }
        let mounted = self.mounted(host, performer);
        let is_self = self.player == Some(performer);
        let rng = &mut self.rng;
        let key = skill.choose(mounted, is_self, |n| {
            usize::try_from(rng.range_int(0, i64::try_from(n).unwrap_or(1) - 1)).unwrap_or(0)
        })?;
        let open = self
            .stories
            .values()
            .filter(|s| s.performer == performer)
            .count();
        // Combat_SkillMode for the sound parameter of §4.4.
        if let (Some(pid), Some(mode)) = (self.vocab.skill_mode, skill.animation_mode)
            && host.available(performer)
        {
            host.set_property(performer, pid, self.vocab.value(pid, f64::from(mode)));
        }
        // Rule 4: 1.0, or the performer's own value.
        let mut params = Vec::new();
        if let Some(pid) = self.vocab.speed_multiplier {
            let v = host
                .property(performer, pid)
                .unwrap_or(PropertyValue::Float(1.0));
            params.push((pid, v));
        }
        if open >= MAX_STORIES_PER_PERFORMER {
            // Rule 11: no further story; the behaviour plays, its strikes show at once.
            self.run(host, Some(performer), key, &params);
            return None;
        }
        let id = self.next_story;
        self.next_story += 1;
        if let Some(pid) = self.vocab.story_id {
            params.push((pid, self.vocab.value(pid, id as f64)));
        }
        // The story exists and is published before its behaviour starts: Execute nodes at
        // the root of the behaviour ask for its upcoming targets synchronously (rule 8).
        self.stories.insert(
            id,
            Story {
                skill: Arc::clone(skill),
                performer,
                target,
                started: self.now,
                run: None,
                strikes: Vec::new(),
                attack: 0,
                defend: 0,
                received: HashMap::new(),
            },
        );
        self.publish(host);
        let run = self.run(host, Some(performer), key, &params);
        self.stats.stories += 1;
        self.process_hooks(host);
        match (run, self.stories.get_mut(&id)) {
            (Some(r), Some(story)) => {
                story.run = Some(r);
                Some(id)
            }
            (r, _) => {
                // Nothing stays running: the story ends at once.
                if let Some(r) = r {
                    host.stop(r);
                }
                self.end_story(host, id, true);
                None
            }
        }
    }

    /// Ends a story (§4.3): the remaining strikes are shown at once, or dropped for a
    /// discarding end; the run is stopped when it still plays and the end discards.
    fn end_story(&mut self, host: &mut dyn ScriptHost, id: i64, show: bool) {
        let Some(story) = self.stories.remove(&id) else {
            return;
        };
        if !show && let Some(r) = story.run {
            host.stop(r);
        }
        if show {
            for s in &story.strikes {
                self.show_strike(host, &story.skill, *s);
            }
        }
        self.publish(host);
    }

    /// Ends every story whose behaviour run ended or that is older than the limit (§9.2
    /// step 5), and handles the story hooks the runtime produced.
    pub fn update(&mut self, host: &mut dyn ScriptHost, now: f64) {
        self.set_time(now);
        self.process_hooks(host);
        let ended: Vec<i64> = self
            .stories
            .iter()
            .filter(|(_, s)| {
                s.run.is_none_or(|r| !host.live(r))
                    || self.now - s.started > f64::from(STORY_TIMEOUT)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ended {
            self.end_story(host, id, true);
        }
    }

    /// Handles the story hooks the runtime reported (§4.2).
    pub fn process_hooks(&mut self, host: &mut dyn ScriptHost) {
        for _ in 0..MAX_HOOK_ROUNDS {
            let hooks = host.take_hooks();
            if hooks.is_empty() {
                return;
            }
            for h in hooks {
                self.story_hook(host, h);
            }
        }
    }

    /// One StoryHook event (§4.2): the *n*-th hook of its type releases the story's strikes
    /// of hook number *n* (an Attack hook every kind, a Defend hook only kind 0x100, which
    /// no Tessera strike has, rule 3).
    pub fn story_hook(&mut self, host: &mut dyn ScriptHost, h: StoryHookEvent) {
        let Some(id) = h.story else {
            return;
        };
        let Some(story) = self.stories.get_mut(&id) else {
            return;
        };
        if story.performer != h.entity {
            return;
        }
        let n = match h.hook_type {
            HOOK_ATTACK => {
                story.attack += 1;
                story.attack
            }
            HOOK_DEFEND => {
                story.defend += 1;
                return;
            }
            _ => return,
        };
        self.stats.hooks += 1;
        let n = i64::from(n);
        let (take, keep): (Vec<Strike>, Vec<Strike>) =
            story.strikes.drain(..).partition(|s| s.hook == n);
        story.strikes = keep;
        let skill = Arc::clone(&story.skill);
        self.publish(host);
        for s in take {
            self.show_strike(host, &skill, s);
        }
    }

    // ----- strikes (§9.3) -----

    /// `VitalChanged`: keeps the maximum for the intensity of rule 5.
    pub fn vital_changed(&mut self, entity: EntityId, vital: &Vital, max: f32) {
        let k = vital_key(vital);
        if k != 0
            && max.is_finite()
            && let Some(e) = self.entity(entity)
        {
            e.vital_max.insert(k, max);
        }
    }

    /// `VitalDelta` (§9.3).
    #[allow(clippy::too_many_arguments)]
    pub fn vital_delta(
        &mut self,
        host: &mut dyn ScriptHost,
        data: &dyn VisualData,
        source: Option<EntityId>,
        target: EntityId,
        vital: &Vital,
        amount: f32,
        result: HitResult,
        what: Option<Did>,
        damage_type: Option<u32>,
    ) {
        let Some(what) = what else {
            return;
        };
        // Step 5: an effect pulse.
        if let Some(ev) = data.effect(what) {
            if ev.interval_fx != 0 {
                self.run(host, Some(target), ev.interval_fx, &[]);
            }
            return;
        }
        let Some(skill) = data.skill(what) else {
            return;
        };
        let rkey = self.vocab.result_key(strike_result(result));
        let mut strike = Strike {
            kind: STRIKE_KIND,
            hook: 1,
            target,
            attacker: source,
            result: rkey,
            damage_type: 0,
            vital: vital_key(vital),
            amount,
        };
        let story_id = source.and_then(|p| {
            self.stories
                .iter()
                .find(|(_, s)| s.performer == p && s.skill.did == skill.did)
                .map(|(id, _)| *id)
        });
        let last = skill.last_hook_number();
        let mut queue_in = None;
        if let Some(id) = story_id
            && let Some(story) = self.stories.get_mut(&id)
        {
            let r = story.received.entry(target).or_insert(0);
            *r = r.saturating_add(1);
            let i = i64::from(*r);
            match last {
                Some(l) if l < i => strike.hook = l,
                None => strike.hook = 1,
                Some(_) if i64::from(story.attack) >= i => strike.hook = i,
                Some(_) if story.strikes.len() >= MAX_STRIKES_PER_STORY => strike.hook = i,
                Some(_) => {
                    strike.hook = i;
                    queue_in = Some(id);
                }
            }
        }
        strike.damage_type = damage_type
            .or_else(|| skill.hook(strike.hook).and_then(|h| h.damage_type))
            .unwrap_or(0);
        match queue_in.and_then(|id| self.stories.get_mut(&id)) {
            Some(story) => {
                story.strikes.push(strike);
                self.publish(host);
            }
            None => self.show_strike(host, &skill, strike),
        }
    }

    /// Shows one strike (§4.4 steps 2–5).
    fn show_strike(&mut self, host: &mut dyn ScriptHost, skill: &SkillVisuals, s: Strike) {
        let Some(hook) = skill.hook(s.hook).copied() else {
            return;
        };
        self.stats.strikes_shown += 1;
        self.run(host, s.attacker, hook.fx_attacker_hit, &[]);
        // Step 4: no weapon objects in P2 (no wielded weapon entity exists).
        let params = self.strike_params(host, &s);
        self.run(host, Some(s.target), hook.behaviour_target_hit, &params);
        self.run(host, Some(s.target), hook.fx_target_hit, &params);
    }

    /// The strike run parameters (§4.4, rule 5).
    fn strike_params(
        &mut self,
        host: &mut dyn ScriptHost,
        s: &Strike,
    ) -> Vec<(u32, PropertyValue)> {
        let v = self.vocab.clone();
        let mut out = Vec::new();
        if let (Some(a), Some(p)) = (s.attacker, v.attacker_id) {
            // An instance ID, never through a float; the host maps it to the runtime's ID.
            out.push((p, PropertyValue::InstanceId(a.0)));
        }
        let mut put = |pid: Option<u32>, x: f64| {
            if let Some(p) = pid {
                out.push((p, v.value(p, x)));
            }
        };
        put(v.attack_result, f64::from(s.result));
        put(v.damage_type, f64::from(s.damage_type));
        put(v.vital, f64::from(s.vital));
        let angle = s
            .attacker
            .and_then(|a| Some((host.pose(s.target)?, host.pose(a)?)))
            .map_or(0.0, |((tp, tf), (ap, _))| attacker_angle(tp, tf, ap));
        put(v.angle, f64::from(angle));
        let max = self
            .entities
            .get(&s.target)
            .and_then(|e| e.vital_max.get(&s.vital).copied());
        put(v.intensity, f64::from(strike_intensity(s.amount, max)));
        // Sounds only (§4.4): the attacker's implement by its skill mode, the target's
        // struck surface by the result.
        let implement = s
            .attacker
            .and_then(|a| {
                let mode = v.skill_mode.and_then(|p| host.property(a, p))?.as_f64()?;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let pid = match mode as u32 {
                    SKILL_MODE_MELEE => v.implement_by_mode[0],
                    SKILL_MODE_RANGED => v.implement_by_mode[1],
                    SKILL_MODE_THROWN => v.implement_by_mode[2],
                    _ => None,
                }?;
                host.property(a, pid)?.as_f64().filter(|x| *x != 0.0)
            })
            .unwrap_or(f64::from(DEFAULT_STRIKING_IMPLEMENT));
        let parry = v.result_key(StrikeResult::Parry);
        let block = v.result_key(StrikeResult::Block);
        let first = if s.result == parry {
            v.materials[0]
        } else if s.result == block {
            v.materials[1]
        } else {
            None
        };
        let surface = [first, v.materials[2], v.materials[3]]
            .into_iter()
            .flatten()
            .find_map(|p| host.property(s.target, p).and_then(|x| x.as_f64()));
        let mut put = |pid: Option<u32>, x: f64| {
            if let Some(p) = pid {
                out.push((p, v.value(p, x)));
            }
        };
        put(v.striking_implement, implement);
        if let Some(x) = surface {
            put(v.struck_surface, x);
        }
        out
    }

    // ----- effects (§9.4) -----

    /// `EffectApplied` (§9.4): `instance` is the feed's effect instance.
    pub fn effect_applied(
        &mut self,
        host: &mut dyn ScriptHost,
        data: &dyn VisualData,
        entity: EntityId,
        instance: u64,
        what: Option<Did>,
    ) {
        let Some(what) = what else { return };
        let mut keys = Vec::new();
        if let Some(ev) = data.effect(what)
            && ev.fx != 0
        {
            keys.push(ev.fx);
        }
        if let Some(k) = data.induced_state_script(what).filter(|k| *k != 0) {
            keys.push(k);
        }
        let Some(st) = self.entity(entity) else {
            return;
        };
        if st.effects.len() >= MAX_EFFECTS_PER_ENTITY && !st.effects.contains_key(&instance) {
            return;
        }
        // A repeated instance replaces its keys.
        let old = st
            .effects
            .insert(instance, keys.clone())
            .unwrap_or_default();
        for k in old {
            self.release_shared(host, entity, k);
        }
        let defeated = self.entities.get(&entity).is_some_and(|e| e.defeated);
        for k in keys {
            let e = self.shared.entry((entity, k)).or_insert(SharedRun {
                run: None,
                count: 0,
            });
            e.count += 1;
            if e.count == 1 && !defeated {
                let r = self.run(host, Some(entity), k, &[]);
                if let Some(s) = self.shared.get_mut(&(entity, k)) {
                    s.run = r;
                }
            }
        }
    }

    fn release_shared(&mut self, host: &mut dyn ScriptHost, entity: EntityId, key: ScriptKey) {
        let Some(s) = self.shared.get_mut(&(entity, key)) else {
            return;
        };
        s.count = s.count.saturating_sub(1);
        if s.count == 0 {
            if let Some(r) = s.run {
                host.stop(r);
            }
            self.shared.remove(&(entity, key));
        }
    }

    /// `EffectRemoved` (§9.4).
    pub fn effect_removed(&mut self, host: &mut dyn ScriptHost, entity: EntityId, instance: u64) {
        let keys = self
            .entities
            .get_mut(&entity)
            .and_then(|e| e.effects.remove(&instance))
            .unwrap_or_default();
        for k in keys {
            self.release_shared(host, entity, k);
        }
    }

    // ----- combat state, defeat, revive (§9.5) -----

    /// `CombatState` (§9.5).
    pub fn combat_state(&mut self, host: &mut dyn ScriptHost, entity: EntityId, in_combat: bool) {
        if let Some(e) = self.entity(entity) {
            e.in_combat = in_combat;
        }
        self.apply_stance(host, entity, in_combat);
    }

    fn apply_stance(&mut self, host: &mut dyn ScriptHost, entity: EntityId, in_combat: bool) {
        if !host.available(entity) {
            return;
        }
        if let Some(p) = self.vocab.combat_violent {
            host.set_property(entity, p, PropertyValue::Bool(in_combat));
        }
        if let Some(p) = self.vocab.entity_mode {
            match host.stance(entity).filter(|_| in_combat) {
                Some(k) => host.set_property(entity, p, self.vocab.value(p, f64::from(k))),
                None => host.clear_property(entity, p),
            }
        }
    }

    /// `Defeated` (§9.5).
    pub fn defeated(&mut self, host: &mut dyn ScriptHost, entity: EntityId) {
        let open: Vec<i64> = self
            .stories
            .iter()
            .filter(|(_, s)| s.performer == entity)
            .map(|(id, _)| *id)
            .collect();
        for id in open {
            self.end_story(host, id, true);
        }
        self.stop_cast_runs(host, entity);
        self.casts.remove(&entity);
        // Effect runs stop (the effects stay applied).
        let keys: Vec<ScriptKey> = self
            .shared
            .keys()
            .filter(|(e, _)| *e == entity)
            .map(|(_, k)| *k)
            .collect();
        for k in keys {
            if let Some(s) = self.shared.get_mut(&(entity, k))
                && let Some(r) = s.run.take()
            {
                host.stop(r);
            }
        }
        if let Some(e) = self.entity(entity) {
            e.defeated = true;
            e.in_combat = false;
            e.dead_script = None;
        }
        self.apply_death(host, entity, true);
    }

    fn apply_death(&mut self, host: &mut dyn ScriptHost, entity: EntityId, choose: bool) {
        if !host.available(entity) {
            return;
        }
        let v = self.vocab.clone();
        if let Some(p) = v.life_state {
            host.set_property(entity, p, v.value(p, f64::from(v.dead)));
        }
        if choose && let (Some(p), Some(die)) = (v.dead_script, v.die_key) {
            let cases = host.switch_cases(entity, die, p);
            let pick = if cases.is_empty() {
                None
            } else {
                let i = self
                    .rng
                    .range_int(0, i64::try_from(cases.len()).unwrap_or(1) - 1);
                cases.get(usize::try_from(i).unwrap_or(0)).cloned()
            };
            if let Some(e) = self.entities.get_mut(&entity) {
                e.dead_script = pick;
            }
        }
        if let (Some(p), Some(val)) = (
            v.dead_script,
            self.entities
                .get(&entity)
                .and_then(|e| e.dead_script.clone()),
        ) {
            host.set_property(entity, p, val);
        }
        if let Some(die) = v.die_key {
            self.run(host, Some(entity), die, &[]);
        }
        if let Some(p) = v.combat_violent {
            host.clear_property(entity, p);
        }
        if let Some(p) = v.entity_mode {
            host.clear_property(entity, p);
        }
    }

    /// `Revived` (§9.5).
    pub fn revived(&mut self, host: &mut dyn ScriptHost, entity: EntityId) {
        if let Some(e) = self.entity(entity) {
            e.defeated = false;
            e.dead_script = None;
        }
        if !host.available(entity) {
            return;
        }
        let v = self.vocab.clone();
        if let Some(p) = v.life_state {
            host.set_property(entity, p, v.value(p, f64::from(v.alive)));
        }
        if let Some(k) = v.revive_key {
            self.run(host, Some(entity), k, &[]);
        }
        for p in [v.dead_script, v.died_knocked_down].into_iter().flatten() {
            host.clear_property(entity, p);
        }
    }

    // ----- entity life -----

    /// The entity's figure became available: the persistent state (stance, defeat, applied
    /// effects) is shown on it (one-shot visuals that happened before are not replayed).
    pub fn entity_ready(&mut self, host: &mut dyn ScriptHost, entity: EntityId) {
        let Some(st) = self.entities.get(&entity).cloned() else {
            return;
        };
        if st.in_combat {
            self.apply_stance(host, entity, true);
        }
        if st.defeated {
            self.apply_death(host, entity, st.dead_script.is_none());
        } else {
            let keys: Vec<ScriptKey> = self
                .shared
                .iter()
                .filter(|((e, _), s)| *e == entity && s.run.is_none())
                .map(|((_, k), _)| *k)
                .collect();
            for k in keys {
                let r = self.run(host, Some(entity), k, &[]);
                if let Some(s) = self.shared.get_mut(&(entity, k)) {
                    s.run = r;
                }
            }
        }
    }

    /// The entity is gone: its stories end (strikes on others are still shown), its state
    /// is dropped.
    pub fn entity_removed(&mut self, host: &mut dyn ScriptHost, entity: EntityId) {
        let open: Vec<i64> = self
            .stories
            .iter()
            .filter(|(_, s)| s.performer == entity)
            .map(|(id, _)| *id)
            .collect();
        for id in open {
            self.end_story(host, id, true);
        }
        self.casts.remove(&entity);
        self.entities.remove(&entity);
        self.shared.retain(|(e, _), _| *e != entity);
        for s in self.stories.values_mut() {
            s.strikes.retain(|k| k.target != entity);
        }
        self.publish(host);
    }

    /// The entity's figure left the script runtime while the entity stays (a region change):
    /// the runs started on it are gone; [`Self::entity_ready`] starts its persistent visuals
    /// again when the figure is back.
    pub fn entity_lost(&mut self, entity: EntityId) {
        for ((e, _), s) in &mut self.shared {
            if *e == entity {
                s.run = None;
            }
        }
        match self.casts.get_mut(&entity) {
            Some(ActiveCast::Induction { runs, .. } | ActiveCast::Channel { runs, .. }) => {
                runs.clear();
            }
            Some(ActiveCast::Cast { .. }) | None => {}
        }
    }

    /// The entities the machine keeps state for (combat state or a cast).
    #[must_use]
    pub fn known_entities(&self) -> Vec<EntityId> {
        let mut out: Vec<EntityId> = self.entities.keys().copied().collect();
        out.extend(self.casts.keys().filter(|e| !self.entities.contains_key(e)));
        out.extend(
            self.shared
                .keys()
                .map(|(e, _)| *e)
                .filter(|e| !self.entities.contains_key(e) && !self.casts.contains_key(e)),
        );
        out.sort_unstable_by_key(|e| e.0);
        out.dedup();
        out
    }

    /// Whether `entity` is defeated (for TR4's foe set and the picking rules).
    #[must_use]
    pub fn is_defeated(&self, entity: EntityId) -> bool {
        self.entities.get(&entity).is_some_and(|e| e.defeated)
    }

    /// Whether `entity` is in combat.
    #[must_use]
    pub fn in_combat(&self, entity: EntityId) -> bool {
        self.entities.get(&entity).is_some_and(|e| e.in_combat)
    }

    /// The open stories (performer, skill) for diagnostics.
    #[must_use]
    pub fn open_stories(&self) -> Vec<(i64, EntityId, Did)> {
        self.stories
            .iter()
            .map(|(id, s)| (*id, s.performer, s.skill.did))
            .collect()
    }
}

/// `CombatStrikeData_TargetToAttackerAngle` (§11 rule 5): the attacker's direction in the
/// target's horizontal frame, degrees in (−180, 180], 0 straight ahead, positive to the
/// target's left (counter-clockwise seen from above, Z up). 0 when it cannot be formed.
#[must_use]
pub fn attacker_angle(target: [f64; 3], forward: [f32; 2], attacker: [f64; 3]) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let d = [
        (attacker[0] - target[0]) as f32,
        (attacker[1] - target[1]) as f32,
    ];
    let [fx, fy] = forward;
    let cross = fx * d[1] - fy * d[0];
    let dot = fx * d[0] + fy * d[1];
    if !(cross.is_finite() && dot.is_finite()) || (cross == 0.0 && dot == 0.0) {
        return 0.0;
    }
    let a = cross.atan2(dot).to_degrees();
    if a <= -180.0 { 180.0 } else { a }
}

/// `CombatStrikeData_Intensity` (§11 rule 5): the amount over the vital's maximum, clamped
/// to [0, 1]; 1 when the maximum is unknown or not positive.
#[must_use]
pub fn strike_intensity(amount: f32, max: Option<f32>) -> f32 {
    match max {
        Some(m) if m > 0.0 && m.is_finite() && amount.is_finite() => (amount / m).clamp(0.0, 1.0),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests;
