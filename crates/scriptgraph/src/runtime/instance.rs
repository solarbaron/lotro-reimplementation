// SPDX-License-Identifier: GPL-3.0-or-later
//! Node instances (`script-runtime.md` §2.1) and their store.

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::PropertyValue;

use crate::graph::{NodeRef, UnitId};
use crate::sched::EventKey;
use crate::sink::{AnimationHandle, EntityId, EnvironmentHandle, SoundHandle};

/// A run-parameter set (`script-runtime.md` §5.1).
pub(crate) type ParamSet = BTreeMap<u32, PropertyValue>;

/// An instance, by slot and generation (a stale ID never reaches a newer instance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InstanceId {
    idx: u32,
    generation: u32,
}

/// A running child and the port group it was started into (0: none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Child {
    pub id: InstanceId,
    pub tag: u32,
}

/// A Switch cross-fade window (`script-runtime.md` §6.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fade {
    pub end: f64,
    pub dur: f64,
}

/// State of a Switch or When instance (§6, §7).
#[derive(Debug, Clone)]
pub(crate) struct SwitchState {
    pub is_when: bool,
    pub property: u32,
    pub target: EntityId,
    pub persistent: bool,
    /// Delay, RampOut or transition ports in use (§6.7).
    pub transition_mode: bool,
    pub value: Option<PropertyValue>,
    pub current: Option<usize>,
    pub current_tag: u32,
    pub fcv_tag: u32,
    pub outgoing_tag: u32,
    pub fade: Option<Fade>,
    pub fade_event: Option<EventKey>,
    pub delay_event: Option<EventKey>,
    /// A transition with `Wait` = 1 took over; the case starts when its children finish.
    pub waiting: bool,
    /// Evaluation counter: bumped by every re-evaluation, case switch and transition. An
    /// evaluation that finds it changed after a stop or a port run was overtaken by a
    /// nested one (a child or an OnStop changed the property) and ends there.
    pub eval: u64,
}

/// State of a WhenRunning instance (§13).
#[derive(Debug, Clone)]
pub(crate) struct WhenRunningState {
    pub target: EntityId,
    pub keys: Vec<u32>,
    pub running_tag: u32,
    pub not_running_tag: u32,
}

/// State of a Sequence instance (§10).
#[derive(Debug, Clone)]
pub(crate) struct SequenceState {
    pub forever: bool,
    pub limit: u32,
    pub started: u64,
    pub step_tag: u32,
    pub last_pass: Option<f64>,
    pub pending: Option<EventKey>,
}

/// State of a Timer instance (§11).
#[derive(Debug, Clone)]
pub(crate) struct TimerState {
    pub t0: f64,
    pub count: u32,
    pub next: usize,
    pub pending: Option<EventKey>,
    /// Cycles begun in a row without `t0` moving forward.
    pub same_time_cycles: u32,
    /// The advance (`Runtime::advances`) of the last cycle end, and the cycles ended in it.
    pub advance: u64,
    pub advance_cycles: u32,
    /// No event of this Timer is due before this time (set when a cycle had to wait for
    /// the next advance).
    pub not_before: f64,
}

/// State of a While instance (`script-runtime.md` §30.3–§30.4).
#[derive(Debug, Clone)]
pub(crate) struct WhileState {
    /// `WhileScript_PropertyName`.
    pub property: u32,
    /// The entity whose property is watched (the running entity unless a target names
    /// another, §30.3 step 1).
    pub target: EntityId,
    /// The `Names` properties, watched on the running entity (§30.1 OPEN-21 Decision).
    pub names: Vec<u32>,
    /// The port group of the branch (0 before it starts).
    pub tag: u32,
    /// The test failed once: the branch was stopped and the node armed; later changes do
    /// nothing (§30.4).
    pub ended: bool,
}

/// State of a Run instance with `Persistent` = 1 (§12.4).
#[derive(Debug, Clone)]
pub(crate) struct RunState {
    pub started: Vec<InstanceId>,
    pub unbind: bool,
    pub targets: u32,
    pub failed: u32,
}

/// Kind-specific state.
#[derive(Debug, Clone)]
pub(crate) enum KindState {
    /// Collection, Label, Goto, Random, ScriptletParameter, ScalarBlend: a pure container.
    Container,
    /// The run of a script key (the parent of its root instances).
    RunRoot {
        key: u32,
    },
    /// A Scriptlet invocation of `unit` (the invoked scriptlet).
    Scriptlet {
        unit: UnitId,
    },
    /// OnStop: idle until stopped.
    OnStop,
    Switch(Box<SwitchState>),
    WhenRunning(Box<WhenRunningState>),
    Sequence(Box<SequenceState>),
    Timer(Box<TimerState>),
    /// A tracked Sound.
    Sound {
        handle: SoundHandle,
        completed: bool,
    },
    Environment {
        handle: EnvironmentHandle,
    },
    Run(Box<RunState>),
    Animation {
        handle: Option<AnimationHandle>,
        event: Option<EventKey>,
    },
    /// A While node (§30).
    While(Box<WhileState>),
    /// A never-ending Particle node (`particle-runtime.md` §5.5 step 5): the effect's handle
    /// (none when nothing started), its target, the `ImmediateDisposal` byte and the
    /// redirect PID it listens to (0 for none, §5.8).
    Particle(Box<ParticleState>),
}

/// The state of a never-ending Particle node's instance.
#[derive(Debug, Clone)]
pub(crate) struct ParticleState {
    pub handle: Option<crate::sink::ParticleHandle>,
    pub target: EntityId,
    pub immediate: bool,
    pub redirect: u32,
}

/// A node instance.
#[derive(Debug, Clone)]
pub(crate) struct Instance {
    /// The node (`None` for a run root).
    pub node: Option<NodeRef>,
    pub parent: Option<InstanceId>,
    pub entity: EntityId,
    pub depth: u32,
    pub intensity: f32,
    /// The intensity factor last applied (§4.2), for cross-fades.
    pub factor: f32,
    pub use_parent_intensity: bool,
    pub params: Arc<ParamSet>,
    /// The persistent-node permission of the context (§2.1).
    pub permission: bool,
    /// Driven values written into this start's node copy: (field PID, value) (§2.2, §5).
    pub overrides: Vec<(u32, PropertyValue)>,
    pub children: Vec<Child>,
    /// The parent's port group this instance was started into (0: none).
    pub group: u32,
    pub live_tags: Vec<u32>,
    pub next_tag: u32,
    pub armed: bool,
    pub stopped: bool,
    /// Ports being started: completion waits until it is 0.
    pub filling: u32,
    pub listens: Vec<(EntityId, u32)>,
    pub watches: Vec<(EntityId, u32)>,
    /// Property changes being handled: (PID, nesting) (§21).
    pub handling: Vec<(u32, u8)>,
    pub scriptlet_depth: u32,
    pub kind: KindState,
}

impl Instance {
    /// A new instance with no children.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node: Option<NodeRef>,
        parent: Option<InstanceId>,
        entity: EntityId,
        depth: u32,
        intensity: f32,
        factor: f32,
        use_parent_intensity: bool,
        params: Arc<ParamSet>,
        permission: bool,
        scriptlet_depth: u32,
    ) -> Self {
        Self {
            node,
            parent,
            entity,
            depth,
            intensity,
            factor,
            use_parent_intensity,
            params,
            permission,
            overrides: Vec::new(),
            children: Vec::new(),
            group: 0,
            live_tags: Vec::new(),
            next_tag: 0,
            armed: false,
            stopped: false,
            filling: 0,
            listens: Vec::new(),
            watches: Vec::new(),
            handling: Vec::new(),
            scriptlet_depth,
            kind: KindState::Container,
        }
    }

    /// The override of `field`, if a driven pair wrote one.
    pub fn override_of(&self, field: Option<u32>) -> Option<&PropertyValue> {
        let f = field?;
        self.overrides.iter().find(|(p, _)| *p == f).map(|(_, v)| v)
    }

    /// Opens a new port group and returns its tag (never 0).
    pub fn open_group(&mut self) -> u32 {
        self.next_tag = self.next_tag.wrapping_add(1);
        if self.next_tag == 0 {
            self.next_tag = 1;
        }
        let t = self.next_tag;
        self.live_tags.push(t);
        t
    }

    /// Closes a port group (its tag no longer accepts children).
    pub fn close_group(&mut self, tag: u32) {
        self.live_tags.retain(|&t| t != tag);
    }

    /// Whether children may still join group `tag`.
    pub fn group_live(&self, tag: u32) -> bool {
        tag == 0 || self.live_tags.contains(&tag)
    }

    /// Number of running children in group `tag`.
    pub fn group_len(&self, tag: u32) -> usize {
        self.children.iter().filter(|c| c.tag == tag).count()
    }
}

#[derive(Debug, Clone)]
struct Slot {
    generation: u32,
    inst: Option<Instance>,
}

/// The instance store: slots with generations and a free list.
#[derive(Debug, Clone, Default)]
pub(crate) struct Slab {
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
}

impl Slab {
    pub fn insert(&mut self, inst: Instance) -> InstanceId {
        self.live += 1;
        if let Some(idx) = self.free.pop()
            && let Some(slot) = self.slots.get_mut(idx as usize)
        {
            slot.inst = Some(inst);
            return InstanceId {
                idx,
                generation: slot.generation,
            };
        }
        let idx = u32::try_from(self.slots.len()).unwrap_or(u32::MAX);
        self.slots.push(Slot {
            generation: 0,
            inst: Some(inst),
        });
        InstanceId { idx, generation: 0 }
    }

    pub fn get(&self, id: InstanceId) -> Option<&Instance> {
        let s = self.slots.get(id.idx as usize)?;
        if s.generation == id.generation {
            s.inst.as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, id: InstanceId) -> Option<&mut Instance> {
        let s = self.slots.get_mut(id.idx as usize)?;
        if s.generation == id.generation {
            s.inst.as_mut()
        } else {
            None
        }
    }

    pub fn remove(&mut self, id: InstanceId) -> Option<Instance> {
        let s = self.slots.get_mut(id.idx as usize)?;
        if s.generation != id.generation {
            return None;
        }
        let inst = s.inst.take()?;
        s.generation = s.generation.wrapping_add(1);
        self.free.push(id.idx);
        self.live -= 1;
        Some(inst)
    }

    pub fn len(&self) -> usize {
        self.live
    }

    /// The live instances.
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.slots.iter().filter_map(|s| s.inst.as_ref())
    }
}
