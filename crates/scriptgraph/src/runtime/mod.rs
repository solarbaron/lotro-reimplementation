// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime: entities, their runs and node instances, the scheduler, and the public API.

mod core;
mod instance;
mod kinds;
mod switch;
#[cfg(test)]
mod tests;
mod timing;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::SmallRng;
use tessera_formats::script_table::resolve::{EntityScripts, PropertyState};
use tessera_formats::{Did, PropertyValue, ScriptTable};

use crate::error::{RuntimeError, RuntimeIssue};
use crate::graph::{NodeRef, ScriptDef, Unit, UnitId, UnitKey};
use crate::sched::{EventKey, Scheduler};
use crate::sink::{EffectSink, EntityId, SoundHandle};
use crate::source::ScriptSource;
use crate::values::{as_f32, opt_equal};
use crate::vocab::{PropertyCatalog, RuntimeVocabulary};

pub(crate) use instance::{Instance, InstanceId, KindState, ParamSet};

/// Intensity changes at or below this are ignored, and nodes at or below it are silent
/// (`script-runtime.md` §4.3, §11.3).
pub const INTENSITY_EPSILON: f32 = 0.0002;
/// Shortest cycle of a looping Timer and shortest interval between Sequence passes, seconds
/// (§10.4, §11.2).
pub const MIN_LOOP_INTERVAL: f64 = 0.1;
/// Scheduler lag beyond which Timers skip whole cycles and Sequences resume at the frame
/// time, seconds (§10.4, §11.2).
pub const LAG_CATCH_UP: f64 = 1.111;
/// Longest time step one [`Runtime::advance`] takes, seconds; a longer step is shortened to
/// it (Tessera guard: the scheduler clock stays far from magnitudes where adding
/// [`MIN_LOOP_INTERVAL`] no longer changes it).
pub const MAX_ADVANCE: f64 = 3600.0;

/// Bounds for hostile or huge graphs. Every guard reports [`RuntimeIssue::Limit`] with its
/// field name and counts in [`RuntimeStats::limit_hits`]; the affected start completes at
/// once. The defaults are far above what the reference graphs need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimits {
    /// Most entities.
    pub max_entities: usize,
    /// Most live instances on one entity.
    pub max_instances_per_entity: usize,
    /// Most live instances overall.
    pub max_instances: usize,
    /// Deepest instance tree (a start below it completes at once). A run started by a Run
    /// node continues the depth of that Run instance, so chains of runs that start runs are
    /// bounded too. Also bounds the stop recursion.
    pub max_instance_depth: u32,
    /// Most nested synchronous node starts in one call chain (`script-runtime.md` §21
    /// OPEN-14 Decision: 256).
    pub max_nesting: u32,
    /// Most node starts in one public call (spawn, property change, …) or in the delivery
    /// of one scheduler event.
    pub max_starts_per_call: u64,
    /// Most node starts in one advance, all its events together; once reached, the
    /// remaining events wait for the next advance.
    pub max_starts_per_advance: u64,
    /// Most scheduler events delivered in one advance; the rest wait for the next one.
    pub max_events_per_advance: u64,
    /// Deepest nesting of Scriptlet invocations.
    pub max_scriptlet_depth: u32,
    /// Most tables visited by one script lookup.
    pub max_lookup_tables: usize,
    /// Deepest include chain followed by a lookup.
    pub max_include_depth: usize,
    /// Most nodes compiled overall (all cached tables and scriptlets).
    pub max_compiled_nodes: usize,
    /// Most cycles a non-looping Timer may begin without its cycle start moving forward (a
    /// zero cycle length, or one too small to change the scheduler time); the Timer then
    /// completes.
    pub max_same_time_cycles: u32,
    /// Most cycles one Timer may begin in one advance; further cycles wait for the next
    /// advance (the Timer then lags and catches up as in §11.2).
    pub max_timer_cycles_per_advance: u32,
    /// Most stack, in bytes, that one public call may use below its entry point for nested
    /// starts, property reactions, run watches and intensity propagation. Graphs that
    /// recurse synchronously (a Goto reaching its own Label, WhenRunning nodes that start
    /// and stop each other's scripts) recurse without bound in the original (§21); this
    /// guard ends them before the thread's stack does, whatever the build's frame sizes.
    /// The calling thread needs this much stack plus some margin (the default fits the
    /// 2 MiB of a spawned thread).
    pub max_stack_bytes: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_entities: 1 << 20,
            max_instances_per_entity: 16_384,
            max_instances: 4_000_000,
            max_instance_depth: 256,
            max_nesting: 256,
            max_starts_per_call: 200_000,
            max_starts_per_advance: 200_000,
            max_events_per_advance: 100_000,
            max_scriptlet_depth: 64,
            max_lookup_tables: 4096,
            max_include_depth: 64,
            max_compiled_nodes: 16_000_000,
            max_same_time_cycles: 256,
            max_timer_cycles_per_advance: 1024,
            max_stack_bytes: 1 << 20,
        }
    }
}

/// Counters (cumulative unless marked "live").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeStats {
    /// Live entities.
    pub entities: usize,
    /// Live instances (run roots included).
    pub instances: usize,
    /// Live runs.
    pub runs: usize,
    /// Pending scheduler events (some may belong to stopped instances).
    pub scheduled_events: usize,
    /// Live tracked sounds.
    pub tracked_sounds: usize,
    /// Runs started.
    pub runs_started: u64,
    /// Node starts attempted (past the budget checks).
    pub node_starts: u64,
    /// Starts skipped by the `Probability` draw.
    pub probability_skips: u64,
    /// Instances stopped (completion included).
    pub stops: u64,
    /// Sound requests sent to the sink.
    pub sounds_requested: u64,
    /// Sound requests the sink accepted.
    pub sounds_accepted: u64,
    /// Sound environments registered.
    pub environments_registered: u64,
    /// Scheduler events delivered.
    pub events_delivered: u64,
    /// Property changes that notified listeners.
    pub property_changes: u64,
    /// Guards of [`RuntimeLimits`] that fired.
    pub limit_hits: u64,
    /// Compiled tables and scriptlets.
    pub compiled_units: usize,
    /// Compiled nodes.
    pub compiled_nodes: usize,
}

/// What a script start did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptStart {
    /// The run is live.
    Running,
    /// The run started and completed at once (no instance stayed running).
    Completed,
    /// No table of the entity's closure defines the key (§12.1 step 2).
    NotDefined,
    /// `Script_MaxNumInstances` runs of the key are live already (§12.3).
    LimitReached,
    /// The entity is being destroyed (§12.1 step 1).
    Refused,
}

/// What an entity starts with. [`EntityInit::default`] has no tables and runs `Startup`
/// and the start-up scripts (`run_startup` = `true`).
#[derive(Debug, Clone)]
pub struct EntityInit {
    /// The entity's tables: class table, instance table, additional tables
    /// (`script-table.md` §8.1, §11.3 step 1). Their closures are searched in this order.
    pub tables: Vec<Did>,
    /// `ScriptTable_StartupScriptNames`, started after `Startup` (§8.1).
    pub startup_scripts: Vec<u32>,
    /// Stored property values (from the entity's property sets).
    pub properties: PropertyState,
    /// The entity holding or containing this one (`sound.md` §9.2 step 2).
    pub parent: Option<EntityId>,
    /// Whether to run `Startup` and the start-up scripts (`script-runtime.md` §22 step 3).
    /// `true` from [`EntityInit::default`] and [`EntityInit::from_scripts`].
    pub run_startup: bool,
}

impl Default for EntityInit {
    fn default() -> Self {
        Self {
            tables: Vec::new(),
            startup_scripts: Vec::new(),
            properties: PropertyState::default(),
            parent: None,
            run_startup: true,
        }
    }
}

impl EntityInit {
    /// The tables, start-up names and values gathered by
    /// [`tessera_formats::script_table::resolve::collect_entity_scripts`].
    #[must_use]
    pub fn from_scripts(scripts: &EntityScripts) -> Self {
        Self {
            tables: scripts.tables(),
            startup_scripts: scripts.startup_scripts.clone(),
            properties: scripts.properties.clone(),
            parent: None,
            run_startup: true,
        }
    }
}

/// The run context a script start gets (§2.1, §12.4).
#[derive(Debug, Clone)]
pub(crate) struct RunCtx {
    pub intensity: f32,
    pub params: Arc<ParamSet>,
    pub permission: bool,
    /// Instance depth of the run root: 0 for client starts, the starting Run instance's
    /// depth + 1 for runs started by a Run node (Tessera guard, [`RuntimeLimits`]).
    pub depth: u32,
}

impl RunCtx {
    fn client() -> Self {
        Self {
            intensity: 1.0,
            params: Arc::new(ParamSet::new()),
            permission: true,
            depth: 0,
        }
    }
}

/// Why a run did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunFail {
    NotDefined,
    Limit,
    Refused,
}

/// A Run request to another entity, handled at the next scheduler pass (§12.4 step 2).
#[derive(Debug, Clone)]
pub(crate) struct DeferredRun {
    pub target: EntityId,
    pub key: u32,
    pub ctx: RunCtx,
    /// The persistent Run instance waiting for the reply.
    pub origin: Option<InstanceId>,
    pub unbind: bool,
}

/// Which deferred Switch change is due (§6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchDelay {
    Transition(usize),
    Case,
}

/// A scheduler event.
#[derive(Debug, Clone)]
pub(crate) enum Event {
    Timer(InstanceId),
    SequenceResume(InstanceId),
    SwitchDelay(InstanceId, SwitchDelay),
    SwitchFadeEnd(InstanceId),
    AnimationEnd(InstanceId),
    DeferredRun(Box<DeferredRun>),
}

/// An entity's run-time state.
#[derive(Debug, Clone)]
pub(crate) struct Entity {
    pub seq: u64,
    pub tables: Arc<[Did]>,
    pub stored: PropertyState,
    pub runtime: HashMap<u32, PropertyValue>,
    pub parent: Option<EntityId>,
    pub destroying: bool,
    pub runs: HashMap<u32, Vec<InstanceId>>,
    pub instances: usize,
}

/// Per-call bookkeeping.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CallState {
    pub starts: u64,
    pub nesting: u32,
    /// Stack position at the public call's entry.
    pub stack_base: usize,
}

/// An address on the current stack frame (the stack's current depth, as a position).
#[inline(never)]
fn stack_position() -> usize {
    let marker = 0u8;
    std::hint::black_box(std::ptr::addr_of!(marker)).addr()
}

/// Script definitions by (entity tables, key).
type DefCache = HashMap<(Arc<[Did]>, u32), Option<Arc<ScriptDef>>>;

/// Instances by (entity, property ID or script key). Kept outside the entity records, so
/// that instances of other entities keep listening across a despawn and respawn of the
/// same [`EntityId`].
pub(crate) type Registry = HashMap<(EntityId, u32), Vec<InstanceId>>;

/// Most distinct issues kept.
const MAX_ISSUES: usize = 4096;

/// The continuous runtime of script-table node graphs (`script-runtime.md`).
///
/// Entities are identified by the caller ([`EntityId`]). Each has its tables, a layered
/// property store (stored values from its property sets, then run-time values set by the
/// caller or by SetProperty nodes; unset properties read the dictionary default, and a
/// Switch or When starts from the type's zero value when there is none, §6.2) and its
/// runs. Every public call that can start or stop nodes takes the [`EffectSink`] that
/// receives sound and environment effects.
///
/// Per frame the client sets the game's property changes ([`Runtime::set_property`]),
/// then calls [`Runtime::advance`] once with the frame's time step, and feeds voice
/// completions back with [`Runtime::sound_completed`] (`script-runtime.md` §20.2, §22).
pub struct Runtime<'a> {
    source: Box<dyn ScriptSource + 'a>,
    catalog: Box<dyn PropertyCatalog + 'a>,
    pub(crate) vocab: RuntimeVocabulary,
    pub(crate) limits: RuntimeLimits,
    pub(crate) rng: SmallRng,
    tables: HashMap<Did, Option<Arc<ScriptTable>>>,
    scriptlet_units: HashMap<Did, Option<UnitId>>,
    pub(crate) units: Vec<Arc<Unit>>,
    unit_index: HashMap<UnitKey, UnitId>,
    compiled_nodes: usize,
    defs: DefCache,
    pub(crate) entities: HashMap<EntityId, Entity>,
    /// Property listeners by (entity, PID).
    pub(crate) listeners: Registry,
    /// Run watchers by (entity, key) (§13).
    pub(crate) watchers: Registry,
    /// Inside [`Runtime::advance`]: Run requests to other entities wait for the next one.
    pub(crate) in_advance: bool,
    /// Number of advances so far.
    pub(crate) advances: u64,
    entity_seq: u64,
    pub(crate) insts: instance::Slab,
    pub(crate) sched: Scheduler<Event>,
    pub(crate) random_memory: HashMap<NodeRef, u32>,
    pub(crate) sounds: HashMap<SoundHandle, InstanceId>,
    pub(crate) frame_updates: Vec<InstanceId>,
    pub(crate) player: Option<EntityId>,
    pub(crate) viewport: Option<EntityId>,
    issues: Vec<RuntimeIssue>,
    issue_set: HashSet<RuntimeIssue>,
    pub(crate) stats: RuntimeStats,
    pub(crate) call: CallState,
}

impl std::fmt::Debug for Runtime<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("time", &self.sched.now)
            .field("entities", &self.entities.len())
            .field("instances", &self.insts.len())
            .field("units", &self.units.len())
            .finish_non_exhaustive()
    }
}

impl<'a> Runtime<'a> {
    /// A runtime reading records from `source`, property facts from `catalog` (normally the
    /// property dictionary) and IDs from `vocab`, with the default limits and RNG seed 0.
    pub fn new(
        source: impl ScriptSource + 'a,
        catalog: impl PropertyCatalog + 'a,
        vocab: RuntimeVocabulary,
    ) -> Self {
        Self {
            source: Box::new(source),
            catalog: Box::new(catalog),
            vocab,
            limits: RuntimeLimits::default(),
            rng: SmallRng::seed_from_u64(0),
            tables: HashMap::new(),
            scriptlet_units: HashMap::new(),
            units: Vec::new(),
            unit_index: HashMap::new(),
            compiled_nodes: 0,
            defs: HashMap::new(),
            entities: HashMap::new(),
            listeners: HashMap::new(),
            watchers: HashMap::new(),
            in_advance: false,
            advances: 0,
            entity_seq: 0,
            insts: instance::Slab::default(),
            sched: Scheduler::default(),
            random_memory: HashMap::new(),
            sounds: HashMap::new(),
            frame_updates: Vec::new(),
            player: None,
            viewport: None,
            issues: Vec::new(),
            issue_set: HashSet::new(),
            stats: RuntimeStats::default(),
            call: CallState::default(),
        }
    }

    /// Reseeds the random draws (probabilities, intensity variances, Random picks, sound
    /// offsets and pitch spreads), for deterministic tests.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng = SmallRng::seed_from_u64(seed);
        self
    }

    /// Replaces the limits.
    #[must_use]
    pub fn with_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The limits in use.
    #[must_use]
    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    /// The vocabulary in use.
    #[must_use]
    pub fn vocabulary(&self) -> &RuntimeVocabulary {
        &self.vocab
    }

    /// Scheduler time, seconds since the runtime was created (the sum of the advances).
    #[must_use]
    pub fn time(&self) -> f64 {
        self.sched.now
    }

    /// Sets the local player's entity (the target type `ThePlayer`).
    pub fn set_player(&mut self, player: Option<EntityId>) {
        self.player = player;
    }

    /// Sets the viewport entity of `sound.md` §9.2 step 3 (the locally controlled
    /// character).
    pub fn set_viewport_entity(&mut self, viewport: Option<EntityId>) {
        self.viewport = viewport;
    }

    /// Sets the entity holding or containing `id` (for `Sound_MuteWhenInternal`).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn set_parent(
        &mut self,
        id: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), RuntimeError> {
        let e = self
            .entities
            .get_mut(&id)
            .ok_or(RuntimeError::NoSuchEntity(id))?;
        e.parent = parent;
        Ok(())
    }

    /// Creates an entity and runs `Startup` and its start-up scripts
    /// (`script-table.md` §8.1–§8.2, `script-runtime.md` §22 step 3).
    ///
    /// An [`EntityId`] may be reused after [`Runtime::despawn`]: nodes of other entities
    /// that target it (a Switch on `ThePlayer`, a WhenRunning) keep listening and
    /// re-evaluate against the new entity's values.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::EntityExists`], [`RuntimeError::TooManyEntities`].
    pub fn spawn_entity(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        init: EntityInit,
    ) -> Result<(), RuntimeError> {
        if self.entities.contains_key(&id) {
            return Err(RuntimeError::EntityExists(id));
        }
        if self.entities.len() >= self.limits.max_entities {
            return Err(RuntimeError::TooManyEntities(self.limits.max_entities));
        }
        self.begin_call();
        self.entity_seq += 1;
        self.entities.insert(
            id,
            Entity {
                seq: self.entity_seq,
                tables: init.tables.into(),
                stored: init.properties,
                runtime: HashMap::new(),
                parent: init.parent,
                destroying: false,
                runs: HashMap::new(),
                instances: 0,
            },
        );
        // Instances of other entities that listen to this ID (a respawn) read its values
        // again.
        self.notify_all_properties(sink, id);
        if init.run_startup {
            if let Some(k) = self.vocab.startup_key {
                let _ = self.start_run(sink, id, k, RunCtx::client());
            }
            for k in init.startup_scripts {
                if let Err(RunFail::NotDefined) = self.start_run(sink, id, k, RunCtx::client()) {
                    self.issue(RuntimeIssue::ScriptNotDefined { key: k });
                }
            }
        }
        Ok(())
    }

    /// Removes an entity: every run stops with the teardown reason (tracked sounds ramp
    /// out, environments unregister, persistent Runs stop the scripts they started; OnStop
    /// nodes do nothing; no node of the entity starts during the teardown), then the sink is
    /// told to end the entity's fire-and-forget voices (`script-runtime.md` §19 OPEN-12
    /// Decision). Nodes of other entities that listen to the entity's properties then read
    /// no entity value (the dictionary default) and re-evaluate.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn despawn(&mut self, sink: &mut dyn EffectSink, id: EntityId) -> Result<(), RuntimeError> {
        let e = self
            .entities
            .get_mut(&id)
            .ok_or(RuntimeError::NoSuchEntity(id))?;
        e.destroying = true;
        let mut runs: Vec<(u32, Vec<InstanceId>)> =
            e.runs.iter().map(|(k, v)| (*k, v.clone())).collect();
        runs.sort_by_key(|(k, _)| *k);
        let roots: Vec<InstanceId> = runs.into_iter().flat_map(|(_, v)| v).collect();
        self.begin_call();
        for r in roots {
            self.stop(sink, r, 2);
        }
        sink.entity_removed(id);
        self.entities.remove(&id);
        if self.player == Some(id) {
            self.player = None;
        }
        if self.viewport == Some(id) {
            self.viewport = None;
        }
        self.notify_all_properties(sink, id);
        Ok(())
    }

    /// Whether the entity exists.
    #[must_use]
    pub fn has_entity(&self, id: EntityId) -> bool {
        self.entities.contains_key(&id)
    }

    /// Sets a run-time property value on an entity. Listeners (persistent Switches and
    /// Whens, driven fields) react synchronously when the value changes (§20.3).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn set_property(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        pid: u32,
        value: PropertyValue,
    ) -> Result<(), RuntimeError> {
        if !self.entities.contains_key(&id) {
            return Err(RuntimeError::NoSuchEntity(id));
        }
        self.begin_call();
        self.write_property(sink, id, pid, value);
        Ok(())
    }

    /// Removes a run-time value, so the entity reads its stored value or the default
    /// again (listeners react when that differs).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn clear_property(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        pid: u32,
    ) -> Result<(), RuntimeError> {
        let old = self.entity_value(id, pid);
        let e = self
            .entities
            .get_mut(&id)
            .ok_or(RuntimeError::NoSuchEntity(id))?;
        if e.runtime.remove(&pid).is_none() {
            return Ok(());
        }
        self.begin_call();
        if !opt_equal(old.as_ref(), self.entity_value(id, pid).as_ref()) {
            self.notify_property(sink, id, pid);
        }
        Ok(())
    }

    /// The value an entity reads for `pid`: run-time value, else stored value, else the
    /// default.
    #[must_use]
    pub fn property(&self, id: EntityId, pid: u32) -> Option<PropertyValue> {
        self.entity_value(id, pid)
    }

    /// Starts a script on an entity with the client's default context (intensity 1, no
    /// parameters), as the client does for scripts it starts itself (§12.1).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn start_script(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        key: u32,
    ) -> Result<ScriptStart, RuntimeError> {
        if !self.entities.contains_key(&id) {
            return Err(RuntimeError::NoSuchEntity(id));
        }
        self.begin_call();
        Ok(match self.start_run(sink, id, key, RunCtx::client()) {
            Ok(Some(_)) => ScriptStart::Running,
            Ok(None) => ScriptStart::Completed,
            Err(RunFail::NotDefined) => ScriptStart::NotDefined,
            Err(RunFail::Limit) => ScriptStart::LimitReached,
            Err(RunFail::Refused) => ScriptStart::Refused,
        })
    }

    /// Stops every run of `key` on an entity (as a Run node with `Stop` = 1 does) and
    /// returns how many were live.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NoSuchEntity`].
    pub fn stop_script(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        key: u32,
    ) -> Result<usize, RuntimeError> {
        if !self.entities.contains_key(&id) {
            return Err(RuntimeError::NoSuchEntity(id));
        }
        self.begin_call();
        Ok(self.stop_runs(sink, id, key, 0))
    }

    /// Whether a run of `key` is live on the entity.
    #[must_use]
    pub fn is_running(&self, id: EntityId, key: u32) -> bool {
        self.entities
            .get(&id)
            .and_then(|e| e.runs.get(&key))
            .is_some_and(|v| !v.is_empty())
    }

    /// The keys with a live run on the entity, ascending.
    #[must_use]
    pub fn running_scripts(&self, id: EntityId) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .entities
            .get(&id)
            .map(|e| {
                e.runs
                    .iter()
                    .filter(|(_, r)| !r.is_empty())
                    .map(|(k, _)| *k)
                    .collect()
            })
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    /// Live instances on an entity.
    #[must_use]
    pub fn entity_instances(&self, id: EntityId) -> Option<usize> {
        self.entities.get(&id).map(|e| e.instances)
    }

    /// One frame: advances the scheduler by `dt` seconds (due events fire in time order,
    /// §20.1), then runs the per-frame updates (Switch cross-fades, §20.2). A negative or
    /// non-finite `dt` counts as 0; a `dt` above [`MAX_ADVANCE`] counts as that.
    ///
    /// Run requests that nodes make to other entities during the advance are handled in
    /// the next advance (§12.4 step 2, Decision), so Runs that bounce between entities take
    /// one step per frame.
    pub fn advance(&mut self, sink: &mut dyn EffectSink, dt: f64) {
        self.begin_call();
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(MAX_ADVANCE)
        } else {
            0.0
        };
        let target = self.sched.now + dt;
        self.sched.frame_time = target;
        self.in_advance = true;
        self.advances = self.advances.wrapping_add(1);
        let mut delivered = 0u64;
        let mut starts = 0u64;
        loop {
            if delivered >= self.limits.max_events_per_advance {
                self.limit("max_events_per_advance");
                break;
            }
            if starts >= self.limits.max_starts_per_advance {
                self.limit("max_starts_per_advance");
                break;
            }
            let Some((due, ev)) = self.sched.pop_due(target) else {
                break;
            };
            self.sched.now = self.sched.now.max(due);
            delivered += 1;
            self.stats.events_delivered += 1;
            // Each event is its own synchronous chain: the per-call budget restarts, the
            // per-advance budget does not.
            self.call.starts = 0;
            self.deliver(sink, ev);
            starts = starts.saturating_add(self.call.starts);
        }
        self.in_advance = false;
        self.sched.now = self.sched.now.max(target);
        self.frame_update(sink);
    }

    /// A voice that a tracked Sound node started has completed (`sound.md` §6.4): the node
    /// completes, and completion propagates synchronously (§3.4). Returns whether the
    /// handle belonged to a running node.
    pub fn sound_completed(&mut self, sink: &mut dyn EffectSink, handle: SoundHandle) -> bool {
        let Some(id) = self.sounds.remove(&handle) else {
            return false;
        };
        self.begin_call();
        if let Some(i) = self.insts.get_mut(id)
            && let KindState::Sound { completed, .. } = &mut i.kind
        {
            *completed = true;
        } else {
            return false;
        }
        self.stop(sink, id, 0);
        true
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        let mut s = self.stats;
        s.entities = self.entities.len();
        s.instances = self.insts.len();
        s.runs = self
            .entities
            .values()
            .map(|e| e.runs.values().map(Vec::len).sum::<usize>())
            .sum();
        s.scheduled_events = self.sched.len();
        s.tracked_sounds = self.sounds.len();
        s.compiled_units = self.units.len();
        s.compiled_nodes = self.compiled_nodes;
        s
    }

    /// The distinct issues met so far, oldest first.
    #[must_use]
    pub fn issues(&self) -> &[RuntimeIssue] {
        &self.issues
    }

    /// Takes the issue list (a taken issue is reported again when it recurs).
    pub fn take_issues(&mut self) -> Vec<RuntimeIssue> {
        self.issue_set.clear();
        std::mem::take(&mut self.issues)
    }

    // ----- internal: bookkeeping -----

    fn begin_call(&mut self) {
        self.call.starts = 0;
        self.call.stack_base = stack_position();
    }

    /// Whether the current call went deeper than [`RuntimeLimits::max_stack_bytes`]
    /// (reported as a limit). Stacks that grow upwards never trip it.
    pub(crate) fn stack_exhausted(&mut self) -> bool {
        let used = self.call.stack_base.saturating_sub(stack_position());
        if used > self.limits.max_stack_bytes {
            self.limit("max_stack_bytes");
            return true;
        }
        false
    }

    pub(crate) fn issue(&mut self, i: RuntimeIssue) {
        if self.issue_set.len() < MAX_ISSUES && self.issue_set.insert(i.clone()) {
            tracing::warn!(issue = ?i, "script runtime issue");
            self.issues.push(i);
        }
    }

    pub(crate) fn limit(&mut self, name: &'static str) {
        self.stats.limit_hits += 1;
        self.issue(RuntimeIssue::Limit(name));
    }

    pub(crate) fn schedule(&mut self, due: f64, ev: Event) -> EventKey {
        self.sched.schedule(due, ev)
    }

    // ----- internal: properties -----

    /// The entity's value of `pid` (run-time, stored, default).
    pub(crate) fn entity_value(&self, id: EntityId, pid: u32) -> Option<PropertyValue> {
        if let Some(e) = self.entities.get(&id) {
            if let Some(v) = e.runtime.get(&pid) {
                return Some(v.clone());
            }
            if let Some(v) = e.stored.get(pid) {
                return Some(v.clone());
            }
        }
        self.catalog.default_value(pid)
    }

    /// The value a node reads for `pid`: from the run parameters for script-parameter
    /// properties (§5.1), else from the entity; the default when neither has one.
    pub(crate) fn read_value(
        &self,
        entity: EntityId,
        params: &ParamSet,
        pid: u32,
    ) -> Option<PropertyValue> {
        if self.catalog.is_script_parameter(pid) {
            params
                .get(&pid)
                .cloned()
                .or_else(|| self.catalog.default_value(pid))
        } else {
            self.entity_value(entity, pid)
        }
    }

    /// The value actually set for `pid`, with no dictionary default: in the run parameters
    /// for script-parameter properties (§5.1), else a run-time write or a stored property
    /// set of the entity. The int-list properties of a Switch case read this way (§6.2:
    /// "those without a value are skipped, with no default").
    pub(crate) fn read_set_value(
        &self,
        entity: EntityId,
        params: &ParamSet,
        pid: u32,
    ) -> Option<PropertyValue> {
        if self.catalog.is_script_parameter(pid) {
            params.get(&pid).cloned()
        } else {
            let e = self.entities.get(&entity)?;
            e.runtime
                .get(&pid)
                .cloned()
                .or_else(|| e.stored.get(pid).cloned())
        }
    }

    /// The value a Switch or When starts from for `pid` (§6.2, R27): the dictionary
    /// default, else the zero value of the definition's type; `None` without a definition.
    pub(crate) fn unset_value(&self, pid: u32) -> Option<PropertyValue> {
        self.catalog.unset_value(pid)
    }

    pub(crate) fn is_script_parameter(&self, pid: u32) -> bool {
        self.catalog.is_script_parameter(pid)
    }

    /// Writes a run-time value and notifies the listeners when the value changed.
    pub(crate) fn write_property(
        &mut self,
        sink: &mut dyn EffectSink,
        id: EntityId,
        pid: u32,
        value: PropertyValue,
    ) {
        let old = self.entity_value(id, pid);
        let changed = !opt_equal(old.as_ref(), Some(&value));
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        e.runtime.insert(pid, value);
        if changed {
            self.notify_property(sink, id, pid);
        }
    }

    fn notify_property(&mut self, sink: &mut dyn EffectSink, id: EntityId, pid: u32) {
        self.stats.property_changes += 1;
        let listeners: Vec<InstanceId> =
            self.listeners.get(&(id, pid)).cloned().unwrap_or_default();
        for l in listeners {
            if self.insts.get(l).is_some_and(|i| !i.stopped) {
                self.on_property_changed(sink, l, id, pid);
            }
        }
    }

    /// Every property of `id` that instances listen to may have changed (the entity was
    /// despawned or spawned again): its listeners re-evaluate, in PID order.
    fn notify_all_properties(&mut self, sink: &mut dyn EffectSink, id: EntityId) {
        let mut pids: Vec<u32> = self
            .listeners
            .keys()
            .filter(|(e, _)| *e == id)
            .map(|(_, p)| *p)
            .collect();
        pids.sort_unstable();
        for pid in pids {
            self.notify_property(sink, id, pid);
        }
    }

    /// A number property read as `f32`.
    pub(crate) fn entity_bool(&self, id: EntityId, pid: Option<u32>) -> bool {
        pid.and_then(|p| self.entity_value(id, p))
            .and_then(|v| as_f32(&v))
            .is_some_and(|x| x != 0.0)
    }

    // ----- internal: records and lookup -----

    fn load_table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        if let Some(t) = self.tables.get(&did) {
            return t.clone();
        }
        let t = self.source.table(did);
        if t.is_none() {
            self.issue(RuntimeIssue::MissingRecord(did));
        }
        self.tables.insert(did, t.clone());
        t
    }

    /// Registers a compiled unit, unless the node budget is spent.
    fn add_unit(&mut self, unit: Unit) -> Option<UnitId> {
        if self.compiled_nodes.saturating_add(unit.nodes.len()) > self.limits.max_compiled_nodes {
            self.limit("max_compiled_nodes");
            return None;
        }
        self.compiled_nodes += unit.nodes.len();
        let id = UnitId::try_from(self.units.len()).ok()?;
        self.unit_index.insert(unit.key, id);
        self.units.push(Arc::new(unit));
        Some(id)
    }

    /// The compiled definition of `key` in `table`.
    fn table_unit(&mut self, table: &ScriptTable, key: u32) -> Option<UnitId> {
        let k = UnitKey::Table(table.did, key);
        if let Some(&u) = self.unit_index.get(&k) {
            return Some(u);
        }
        let script = table.script(key)?;
        self.add_unit(Unit::compile(k, &script.roots))
    }

    /// The compiled scriptlet `did` (`None` when missing; reported once).
    pub(crate) fn scriptlet_unit(&mut self, did: Did) -> Option<UnitId> {
        if let Some(u) = self.scriptlet_units.get(&did) {
            return *u;
        }
        let u = match self.source.scriptlet(did) {
            Some(s) => self.add_unit(Unit::compile(UnitKey::Scriptlet(did), &s.roots)),
            None => {
                self.issue(RuntimeIssue::MissingRecord(did));
                None
            }
        };
        self.scriptlet_units.insert(did, u);
        u
    }

    /// The definition of `key` on an entity (§12.2), cached per (tables, key).
    pub(crate) fn lookup(&mut self, entity: EntityId, key: u32) -> Option<Arc<ScriptDef>> {
        let tables = self.entities.get(&entity)?.tables.clone();
        let cache_key = (tables.clone(), key);
        if let Some(d) = self.defs.get(&cache_key) {
            return d.clone();
        }
        let d = self.compute_def(&tables, key).map(Arc::new);
        self.defs.insert(cache_key, d.clone());
        d
    }

    /// Walks the closure of `roots` for `key`: pre-order, includes depth first in stored
    /// order, every table once; the roots of every defining table, kind 1 first, then 2,
    /// then 4; `Script_CombinationType` = `Overriding` stops the descent into that table's
    /// includes (§12.2). The merged `Script_MaxNumInstances` is the first value met in that
    /// order (Decision; the merge order is not stated).
    fn compute_def(&mut self, roots: &[Did], key: u32) -> Option<ScriptDef> {
        let mut seen: HashSet<Did> = HashSet::new();
        let mut by_kind: [Vec<NodeRef>; 3] = Default::default();
        let mut max_instances: Option<i64> = None;
        let mut defined = false;
        let mut stack: Vec<(Did, usize)> = roots.iter().rev().map(|&d| (d, 0)).collect();
        while let Some((did, depth)) = stack.pop() {
            if !seen.insert(did) {
                continue;
            }
            if seen.len() > self.limits.max_lookup_tables {
                self.limit("max_lookup_tables");
                break;
            }
            let Some(table) = self.load_table(did) else {
                continue;
            };
            let mut descend = true;
            if table.script(key).is_some()
                && let Some(unit) = self.table_unit(&table, key)
            {
                defined = true;
                let slot = match table.kind {
                    1 => 0,
                    4 => 2,
                    _ => 1,
                };
                if let Some(u) = self.units.get(unit as usize) {
                    by_kind[slot].extend(u.roots.iter().map(|&n| NodeRef { unit, node: n }));
                }
                if let Some(props) = table.script_properties(key) {
                    if max_instances.is_none()
                        && let Some(v) = self.vocab.max_num_instances.and_then(|p| props.get(p))
                    {
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            max_instances = v.as_f64().map(|x| x as i64);
                        }
                    }
                    if let (Some(ct), Some(over)) = (
                        self.vocab.combination_type,
                        self.vocab.combination_overriding,
                    ) && props
                        .get(ct)
                        .and_then(PropertyValue::as_bits)
                        .is_some_and(|b| b == u64::from(over))
                    {
                        descend = false;
                    }
                }
            }
            if descend {
                if depth >= self.limits.max_include_depth {
                    if !table.includes.is_empty() {
                        self.limit("max_include_depth");
                    }
                } else {
                    for &inc in table.includes.iter().rev() {
                        if !seen.contains(&inc) {
                            stack.push((inc, depth + 1));
                        }
                    }
                }
            }
        }
        if !defined {
            return None;
        }
        let [a, b, c] = by_kind;
        let mut roots = a;
        roots.extend(b);
        roots.extend(c);
        let max = match max_instances {
            Some(n) if n >= 2 => u32::try_from(n).unwrap_or(u32::MAX),
            _ => 1,
        };
        Some(ScriptDef {
            roots,
            max_instances: max,
        })
    }

    // ----- internal: events and frames -----

    fn deliver(&mut self, sink: &mut dyn EffectSink, ev: Event) {
        match ev {
            Event::Timer(id) => {
                if self.alive(id) {
                    self.timer_event(sink, id);
                }
            }
            Event::SequenceResume(id) => {
                if self.alive(id) {
                    if let Some(i) = self.insts.get_mut(id)
                        && let KindState::Sequence(s) = &mut i.kind
                    {
                        s.pending = None;
                    }
                    self.sequence_run(sink, id);
                    self.check_complete(sink, id, 0);
                }
            }
            Event::SwitchDelay(id, d) => {
                if self.alive(id) {
                    self.switch_delay_event(sink, id, d);
                }
            }
            Event::SwitchFadeEnd(id) => {
                if self.alive(id) {
                    self.switch_fade_end(sink, id);
                }
            }
            Event::AnimationEnd(id) => {
                if self.alive(id) {
                    if let Some(i) = self.insts.get_mut(id)
                        && let KindState::Animation { handle, event } = &mut i.kind
                    {
                        *handle = None;
                        *event = None;
                    }
                    self.stop(sink, id, 0);
                }
            }
            Event::DeferredRun(d) => self.deferred_run(sink, *d),
        }
    }

    /// Whether `id` is a live, running instance.
    pub(crate) fn alive(&self, id: InstanceId) -> bool {
        self.insts.get(id).is_some_and(|i| !i.stopped)
    }

    fn frame_update(&mut self, sink: &mut dyn EffectSink) {
        if self.frame_updates.is_empty() {
            return;
        }
        let mut list: Vec<(u64, InstanceId)> = self
            .frame_updates
            .iter()
            .filter_map(|&id| {
                let i = self.insts.get(id)?;
                let seq = self.entities.get(&i.entity).map_or(u64::MAX, |e| e.seq);
                Some((seq, id))
            })
            .collect();
        list.sort_by_key(|(s, _)| *s);
        for (_, id) in list {
            if self.alive(id) {
                self.switch_frame(sink, id);
            }
        }
    }
}
