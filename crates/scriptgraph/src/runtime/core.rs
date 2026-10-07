// SPDX-License-Identifier: GPL-3.0-or-later
//! The node life cycle of `script-runtime.md` §3 (start, registration, completion, stop),
//! intensity (§4), driven properties (§5), script runs (§12.1) and listeners.

use std::sync::Arc;

use rand::Rng;
use tessera_formats::PropertyValue;
use tessera_formats::script_table::{NodeData, NodeKind};

use super::instance::Child;
use super::{
    DeferredRun, Entity, INTENSITY_EPSILON, Instance, InstanceId, KindState, ParamSet, RunCtx,
    RunFail, Runtime,
};
use crate::error::RuntimeIssue;
use crate::graph::{CNode, NodeRef, Unit, UnitId};
use crate::sink::{EffectSink, EntityId};
use crate::values::as_f32;

/// Stop reason of ordinary stops and completion (§3.5).
pub(crate) const REASON_STOP: u8 = 0;

impl Runtime<'_> {
    // ----- intensity (§4) -----

    /// The intensity factor of §4.2: the multiplier unchanged when the variance is 0,
    /// otherwise multiplier + uniform(±variance) clamped to [0, 1]. Non-finite results
    /// count as 0.
    pub(crate) fn factor(&mut self, multiplier: f32, variance: f32) -> f32 {
        let f = if variance == 0.0 {
            multiplier
        } else {
            let u: f32 = self.rng.random::<f32>() * 2.0 - 1.0;
            (multiplier + u * variance).clamp(0.0, 1.0)
        };
        if f.is_finite() { f } else { 0.0 }
    }

    /// The (possibly driven) multiplier and the variance of an instance's node.
    pub(crate) fn multiplier_of(&self, id: InstanceId) -> Option<(f32, f32, bool)> {
        let i = self.insts.get(id)?;
        let Some(nref) = i.node else {
            return Some((1.0, 0.0, true));
        };
        let n = self.units.get(nref.unit as usize)?.node(nref.node)?;
        let m = i
            .override_of(self.vocab.intensity_multiplier)
            .and_then(as_f32)
            .unwrap_or(n.base.intensity_multiplier);
        Some((m, n.base.intensity_variance, n.base.use_parent_intensity))
    }

    /// Sets an instance's intensity and propagates it (§4.3): kind hooks (tracked Sound
    /// volume, environment intensity, Run forwarding), then every running child with
    /// `UseParentIntensity` = 1 gets factor × intensity (fresh draw) when that differs by
    /// more than [`INTENSITY_EPSILON`].
    pub(crate) fn set_intensity(&mut self, sink: &mut dyn EffectSink, id: InstanceId, value: f32) {
        if self.stack_exhausted() {
            return;
        }
        let value = if value.is_finite() { value } else { 0.0 };
        let (kids, forward) = {
            let Some(i) = self.insts.get_mut(id) else {
                return;
            };
            i.intensity = value;
            let forward = match &i.kind {
                KindState::Sound { handle, completed } if !*completed => {
                    sink.set_sound_volume(*handle, value.clamp(0.0, 1.0));
                    Vec::new()
                }
                KindState::Environment { handle } => {
                    sink.set_environment_intensity(*handle, value);
                    Vec::new()
                }
                KindState::Run(r) => r.started.clone(),
                _ => Vec::new(),
            };
            (i.children.iter().map(|c| c.id).collect::<Vec<_>>(), forward)
        };
        for root in forward {
            if self.alive(root) {
                self.set_intensity(sink, root, value);
            }
        }
        for c in kids {
            self.propagate_to(sink, c, value);
        }
    }

    /// §4.3 for one child.
    fn propagate_to(&mut self, sink: &mut dyn EffectSink, child: InstanceId, parent: f32) {
        let Some((m, var, upi)) = self.multiplier_of(child) else {
            return;
        };
        if !upi {
            return;
        }
        let f = self.factor(m, var);
        let new = f * parent;
        let Some(c) = self.insts.get_mut(child) else {
            return;
        };
        if c.stopped || (new - c.intensity).abs() <= INTENSITY_EPSILON {
            return;
        }
        c.factor = f;
        self.set_intensity(sink, child, new);
    }

    // ----- starting (§3.1, §3.2) -----

    fn take_start_budget(&mut self) -> bool {
        self.call.starts += 1;
        if self.call.starts > self.limits.max_starts_per_call {
            self.limit("max_starts_per_call");
            return false;
        }
        true
    }

    /// Starts `nref` under `parent` into port group `tag` (§3.1). Returns the instance it
    /// left running, or `None` when it was skipped, completed at once or failed.
    pub(crate) fn start_node(
        &mut self,
        sink: &mut dyn EffectSink,
        parent: InstanceId,
        nref: NodeRef,
        tag: u32,
    ) -> Option<InstanceId> {
        let unit = self.units.get(nref.unit as usize)?.clone();
        let n = unit.node(nref.node)?;
        if !self.take_start_budget() || self.stack_exhausted() {
            return None;
        }
        let (entity, p_int, params, permission, depth, sdepth) = {
            let p = self.insts.get(parent)?;
            (
                p.entity,
                p.intensity,
                p.params.clone(),
                p.permission,
                p.depth,
                p.scriptlet_depth,
            )
        };
        if depth >= self.limits.max_instance_depth {
            self.limit("max_instance_depth");
            return None;
        }
        // Teardown starts nothing (§19, OPEN-12 Decision): a node that a stop during
        // `despawn` would start (a WhenRunning flipping, a Switch re-evaluating) completes
        // at once.
        if self.entities.get(&entity).is_none_or(|e| e.destroying) {
            return None;
        }
        if self.call.nesting >= self.limits.max_nesting {
            self.limit("max_nesting");
            return None;
        }
        let per_entity = self.entities.get(&entity).map_or(0, |e| e.instances);
        if per_entity >= self.limits.max_instances_per_entity {
            self.limit("max_instances_per_entity");
            return None;
        }
        if self.insts.len() >= self.limits.max_instances {
            self.limit("max_instances");
            return None;
        }
        self.stats.node_starts += 1;
        // 1. Persistent-node gate.
        if n.is_persistent() && !permission {
            return None;
        }
        // 4. (computed first; reads only) The node copy's driven values (§5.2).
        let overrides = self.driven_values(n, nref, entity, &params, parent);
        // 2. Probability.
        let prob = n.base.probability;
        if prob != 1.0 {
            let u: f32 = self.rng.random();
            let passes = u <= prob;
            if !passes {
                self.stats.probability_skips += 1;
                return None;
            }
        }
        // 3. The same node already runs under this parent.
        if self.has_running_child_of(parent, nref) {
            return None;
        }
        // 5. Intensity (§4.1).
        let mult = overrides
            .iter()
            .find(|(p, _)| Some(*p) == self.vocab.intensity_multiplier)
            .and_then(|(_, v)| as_f32(v))
            .unwrap_or(n.base.intensity_multiplier);
        let factor = self.factor(mult, n.base.intensity_variance);
        let upi = n.base.use_parent_intensity;
        let intensity = factor * if upi { p_int } else { 1.0 };
        let intensity = if intensity.is_finite() {
            intensity
        } else {
            0.0
        };
        let mut inst = Instance::new(
            Some(nref),
            Some(parent),
            entity,
            depth + 1,
            intensity,
            factor,
            upi,
            params,
            permission,
            sdepth,
        );
        inst.overrides = overrides;
        let id = self.insts.insert(inst);
        if let Some(e) = self.entities.get_mut(&entity) {
            e.instances += 1;
        }
        // Driven fields follow their entity sources while the instance lives (§5.3).
        for &(_, src) in &n.base.driven {
            if !self.is_script_parameter(src) {
                self.listen(id, entity, src);
            }
        }
        // 6. Kind start.
        self.call.nesting += 1;
        let keep = self.kind_start(sink, id, &unit, nref);
        self.call.nesting -= 1;
        // 7. Registration.
        self.finish_start(sink, id, parent, tag, keep)
    }

    fn finish_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        parent: InstanceId,
        tag: u32,
        keep: bool,
    ) -> Option<InstanceId> {
        let (stopped, done) = {
            let i = self.insts.get(id)?;
            (i.stopped, !keep || (i.armed && i.children.is_empty()))
        };
        if stopped {
            return None;
        }
        if done {
            // Completed at once: release without telling the parent (it never knew it).
            self.stop(sink, id, REASON_STOP);
            return None;
        }
        let (parent_ok, group_ok) = match self.insts.get(parent) {
            Some(p) => (!p.stopped, p.group_live(tag)),
            None => (false, false),
        };
        if !parent_ok || !group_ok {
            self.stop(sink, id, REASON_STOP);
            return None;
        }
        if let Some(p) = self.insts.get_mut(parent) {
            p.children.push(Child { id, tag });
        }
        Some(id)
    }

    /// §3.1 step 3. A child that is being stopped (still listed while its stop runs) is
    /// not running: a nested re-evaluation during that stop may start its node again.
    fn has_running_child_of(&self, parent: InstanceId, nref: NodeRef) -> bool {
        self.insts.get(parent).is_some_and(|p| {
            p.children.iter().any(|c| {
                self.insts
                    .get(c.id)
                    .is_some_and(|i| !i.stopped && i.node == Some(nref))
            })
        })
    }

    /// Whether `id` or one of its ancestors was stopped (or is gone) (§3.2).
    pub(crate) fn stopped_or_ancestor(&self, id: InstanceId) -> bool {
        let mut cur = id;
        for _ in 0..=self.limits.max_instance_depth.saturating_add(2) {
            match self.insts.get(cur) {
                None => return true,
                Some(i) if i.stopped => return true,
                Some(i) => match i.parent {
                    None => return false,
                    Some(p) => cur = p,
                },
            }
        }
        false
    }

    /// Starts the children of port `port` of node `node` in `unit` under `parent`, into
    /// group `tag`, in stored order; stops early when `parent`, an ancestor or the group
    /// was stopped meanwhile (§3.2).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_port(
        &mut self,
        sink: &mut dyn EffectSink,
        unit: &Arc<Unit>,
        uid: UnitId,
        node: u32,
        port: usize,
        parent: InstanceId,
        tag: u32,
    ) {
        let Some(p) = unit.node(node).and_then(|n| n.ports.get(port)) else {
            return;
        };
        if let Some(i) = self.insts.get_mut(parent) {
            i.filling += 1;
        } else {
            return;
        }
        for &c in &p.children {
            self.start_node(sink, parent, NodeRef { unit: uid, node: c }, tag);
            if self.stopped_or_ancestor(parent)
                || !self.insts.get(parent).is_some_and(|i| i.group_live(tag))
            {
                break;
            }
        }
        if let Some(i) = self.insts.get_mut(parent) {
            i.filling = i.filling.saturating_sub(1);
        }
    }

    /// Marks an instance armed (§3.4 item 2).
    pub(crate) fn arm(&mut self, id: InstanceId) {
        if let Some(i) = self.insts.get_mut(id) {
            i.armed = true;
        }
    }

    // ----- completion and stopping (§3.4, §3.5) -----

    /// Completes `id` when it is armed, idle and childless.
    pub(crate) fn check_complete(&mut self, sink: &mut dyn EffectSink, id: InstanceId, reason: u8) {
        let done = self.insts.get(id).is_some_and(|i| {
            i.armed && !i.stopped && i.filling == 0 && i.children.is_empty() && !has_pending(i)
        });
        if done {
            self.stop(sink, id, reason);
        }
    }

    /// Stops an instance (§3.5): kind hook, children in list order, listeners, then the run
    /// and the parent are told; the instance is then freed.
    pub(crate) fn stop(&mut self, sink: &mut dyn EffectSink, id: InstanceId, reason: u8) {
        match self.insts.get_mut(id) {
            Some(i) if !i.stopped => i.stopped = true,
            _ => return,
        }
        self.stats.stops += 1;
        self.stop_hook(sink, id, reason);
        let kids: Vec<InstanceId> = self
            .insts
            .get(id)
            .map(|i| i.children.iter().map(|c| c.id).collect())
            .unwrap_or_default();
        for c in kids {
            self.stop(sink, c, reason);
        }
        self.unlisten_all(id);
        let Some(inst) = self.insts.get(id) else {
            return;
        };
        let parent = inst.parent;
        let entity = inst.entity;
        let run_key = match inst.kind {
            KindState::RunRoot { key } => Some(key),
            _ => None,
        };
        if let Some(key) = run_key {
            if let Some(e) = self.entities.get_mut(&entity)
                && let Some(v) = e.runs.get_mut(&key)
            {
                v.retain(|r| *r != id);
                if v.is_empty() {
                    e.runs.remove(&key);
                }
            }
            self.notify_watchers(sink, entity, key);
        }
        if let Some(p) = parent {
            self.child_stopped(sink, p, id, reason);
        }
        if self.insts.remove(id).is_some()
            && let Some(e) = self.entities.get_mut(&entity)
        {
            e.instances = e.instances.saturating_sub(1);
        }
    }

    /// The parent's side of a child stop (§3.4 item 1).
    fn child_stopped(
        &mut self,
        sink: &mut dyn EffectSink,
        parent: InstanceId,
        child: InstanceId,
        reason: u8,
    ) {
        let tag = {
            let Some(p) = self.insts.get_mut(parent) else {
                return;
            };
            let Some(pos) = p.children.iter().position(|c| c.id == child) else {
                return;
            };
            let tag = p.children.swap_remove(pos).tag;
            if p.stopped {
                return;
            }
            tag
        };
        self.child_stopped_hook(sink, parent, tag, reason);
        self.check_complete(sink, parent, reason);
    }

    /// Kind reactions to a child stop: a Sequence step finishing (§10.3), a Switch
    /// transition taking over (§6.9).
    fn child_stopped_hook(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        tag: u32,
        reason: u8,
    ) {
        let Some(i) = self.insts.get(id) else {
            return;
        };
        if i.filling > 0 || tag == 0 || i.group_len(tag) > 0 {
            return;
        }
        match &i.kind {
            KindState::Sequence(s) if s.step_tag == tag && reason != 2 => {
                if let Some(i) = self.insts.get_mut(id) {
                    i.close_group(tag);
                }
                self.sequence_run(sink, id);
            }
            KindState::Switch(s) if s.waiting && s.current_tag == tag => {
                self.switch_transition_done(sink, id);
            }
            _ => {}
        }
    }

    /// Stops every child of `id` in group `tag` (and closes the group).
    pub(crate) fn stop_group(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        tag: u32,
        reason: u8,
    ) {
        if tag == 0 {
            return;
        }
        let kids: Vec<InstanceId> = match self.insts.get_mut(id) {
            Some(i) => {
                i.close_group(tag);
                i.children
                    .iter()
                    .filter(|c| c.tag == tag)
                    .map(|c| c.id)
                    .collect()
            }
            None => return,
        };
        for c in kids {
            self.stop(sink, c, reason);
        }
    }

    /// Children of `id` in group `tag`.
    pub(crate) fn group_children(&self, id: InstanceId, tag: u32) -> Vec<InstanceId> {
        self.insts
            .get(id)
            .map(|i| {
                i.children
                    .iter()
                    .filter(|c| c.tag == tag)
                    .map(|c| c.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    // ----- listeners -----

    /// Registers `id` as a listener of `pid` on `entity` (once). The registration outlives
    /// the entity record (a respawned ID is heard again).
    pub(crate) fn listen(&mut self, id: InstanceId, entity: EntityId, pid: u32) {
        let v = self.listeners.entry((entity, pid)).or_default();
        if v.contains(&id) {
            return;
        }
        v.push(id);
        if let Some(i) = self.insts.get_mut(id) {
            i.listens.push((entity, pid));
        }
    }

    /// Registers `id` as a watcher of runs of `key` on `entity` (§13).
    pub(crate) fn watch(&mut self, id: InstanceId, entity: EntityId, key: u32) {
        let v = self.watchers.entry((entity, key)).or_default();
        if v.contains(&id) {
            return;
        }
        v.push(id);
        if let Some(i) = self.insts.get_mut(id) {
            i.watches.push((entity, key));
        }
    }

    fn unlisten_all(&mut self, id: InstanceId) {
        let (listens, watches) = match self.insts.get_mut(id) {
            Some(i) => (
                std::mem::take(&mut i.listens),
                std::mem::take(&mut i.watches),
            ),
            None => return,
        };
        for k in listens {
            remove_from(&mut self.listeners, k, id);
        }
        for k in watches {
            remove_from(&mut self.watchers, k, id);
        }
        self.frame_updates.retain(|&f| f != id);
    }

    /// Runs of `key` on `entity` started or stopped: watching WhenRunning nodes re-evaluate.
    fn notify_watchers(&mut self, sink: &mut dyn EffectSink, entity: EntityId, key: u32) {
        if self.stack_exhausted() {
            return;
        }
        let ws: Vec<InstanceId> = self
            .watchers
            .get(&(entity, key))
            .cloned()
            .unwrap_or_default();
        for w in ws {
            if self.alive(w) {
                self.when_running_evaluate(sink, w);
            }
        }
    }

    // ----- driven properties (§5) -----

    /// The driven values of a start (§5.2): each driven pair whose source has a value, then
    /// each scriptlet-driven pair whose parameter the invoking Scriptlet sets.
    fn driven_values(
        &self,
        n: &CNode,
        nref: NodeRef,
        entity: EntityId,
        params: &ParamSet,
        parent: InstanceId,
    ) -> Vec<(u32, PropertyValue)> {
        let mut out = Vec::new();
        for &(field, src) in &n.base.driven {
            if let Some(v) = self.read_value(entity, params, src) {
                set_override(&mut out, field, v);
            }
        }
        if !n.base.scriptlet_driven.is_empty()
            && let Some(sc) = self.enclosing_scriptlet(parent, nref.unit)
            && let Some(set) = self.scriptlet_parameters(sc)
        {
            for &(field, ppid) in &n.base.scriptlet_driven {
                if let Some(v) = set.get(ppid) {
                    set_override(&mut out, field, v.clone());
                }
            }
        }
        out
    }

    /// The nearest instance at or above `from` that is a Scriptlet invocation of `unit`
    /// (§17).
    pub(crate) fn enclosing_scriptlet(&self, from: InstanceId, unit: UnitId) -> Option<InstanceId> {
        let mut cur = from;
        for _ in 0..=self.limits.max_instance_depth.saturating_add(2) {
            let i = self.insts.get(cur)?;
            if let KindState::Scriptlet { unit: u } = i.kind
                && u == unit
            {
                return Some(cur);
            }
            cur = i.parent?;
        }
        None
    }

    /// The parameter set of a Scriptlet instance's node.
    fn scriptlet_parameters(&self, sc: InstanceId) -> Option<&tessera_formats::PropertySet> {
        let nref = self.insts.get(sc)?.node?;
        match &self.units.get(nref.unit as usize)?.node(nref.node)?.data {
            NodeData::Scriptlet { parameters, .. } => Some(parameters),
            _ => None,
        }
    }

    /// A property an instance listens to changed on `entity` (§5.3, §6.5, §7, §21).
    pub(crate) fn on_property_changed(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        entity: EntityId,
        pid: u32,
    ) {
        if self.stack_exhausted() {
            return;
        }
        // Re-entry guard: one property at most two levels deep per instance.
        {
            let Some(i) = self.insts.get_mut(id) else {
                return;
            };
            match i.handling.iter_mut().find(|(p, _)| *p == pid) {
                Some((_, d)) if *d >= 2 => {
                    self.issue(RuntimeIssue::PropertyReentry { pid });
                    return;
                }
                Some((_, d)) => *d += 1,
                None => i.handling.push((pid, 1)),
            }
        }
        self.property_changed_inner(sink, id, entity, pid);
        if let Some(i) = self.insts.get_mut(id)
            && let Some(pos) = i.handling.iter().position(|(p, _)| *p == pid)
        {
            if i.handling[pos].1 <= 1 {
                i.handling.swap_remove(pos);
            } else {
                i.handling[pos].1 -= 1;
            }
        }
    }

    fn property_changed_inner(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        entity: EntityId,
        pid: u32,
    ) {
        let Some(i) = self.insts.get(id) else {
            return;
        };
        let Some(nref) = i.node else {
            return;
        };
        let Some(unit) = self.units.get(nref.unit as usize).cloned() else {
            return;
        };
        let Some(n) = unit.node(nref.node) else {
            return;
        };
        // Driven fields follow the entity's source property (§5.3).
        let mut changed_fields: Vec<u32> = Vec::new();
        if i.entity == entity && !self.is_script_parameter(pid) {
            let pairs: Vec<u32> = n
                .base
                .driven
                .iter()
                .filter(|(_, s)| *s == pid)
                .map(|(f, _)| *f)
                .collect();
            if !pairs.is_empty() {
                let v = self.entity_value(entity, pid);
                if let Some(i) = self.insts.get_mut(id) {
                    for f in pairs {
                        match &v {
                            Some(v) => set_override(&mut i.overrides, f, v.clone()),
                            None => i.overrides.retain(|(p, _)| *p != f),
                        }
                        changed_fields.push(f);
                    }
                }
            }
        }
        if !changed_fields.is_empty() {
            if matches!(
                self.insts.get(id).map(|i| &i.kind),
                Some(KindState::Timer(_))
            ) {
                self.timer_reschedule(id);
            }
            if self.vocab.intensity_multiplier.is_some()
                && changed_fields
                    .iter()
                    .any(|f| Some(*f) == self.vocab.intensity_multiplier)
            {
                self.driven_multiplier_changed(sink, id);
            }
        }
        // Switch and When re-evaluation (§6.5, §7).
        let reevaluate = match self.insts.get(id).map(|i| &i.kind) {
            Some(KindState::Switch(s)) if s.persistent && s.target == entity => {
                pid == s.property || (!s.is_when && switch_int_pids(n).any(|p| p == pid))
            }
            _ => false,
        };
        if reevaluate && self.alive(id) {
            self.switch_reevaluate(sink, id);
        }
    }

    /// §4.4: a driven `Script_IntensityMultiplier` changed.
    fn driven_multiplier_changed(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some((m, var, upi)) = self.multiplier_of(id) else {
            return;
        };
        if !upi {
            return;
        }
        let Some(i) = self.insts.get(id) else {
            return;
        };
        let p = i
            .parent
            .and_then(|p| self.insts.get(p))
            .map_or(1.0, |p| p.intensity);
        let cur = i.intensity;
        let f = self.factor(m, var);
        let new = f * (f * p);
        if (new - cur).abs() > INTENSITY_EPSILON {
            if let Some(i) = self.insts.get_mut(id) {
                i.factor = f;
            }
            self.set_intensity(sink, id, new);
        }
    }

    // ----- runs (§12.1) -----

    /// Starts a run of `key` on `entity` (§12.1). `Ok(Some(root))` when it stays live,
    /// `Ok(None)` when it completed at once.
    pub(crate) fn start_run(
        &mut self,
        sink: &mut dyn EffectSink,
        entity: EntityId,
        key: u32,
        ctx: RunCtx,
    ) -> Result<Option<InstanceId>, RunFail> {
        match self.entities.get(&entity) {
            None => return Err(RunFail::Refused),
            Some(e) if e.destroying => return Err(RunFail::Refused),
            Some(_) => {}
        }
        let def = self.lookup(entity, key).ok_or(RunFail::NotDefined)?;
        let live = self
            .entities
            .get(&entity)
            .and_then(|e| e.runs.get(&key))
            .map_or(0, Vec::len);
        if live >= def.max_instances as usize {
            return Err(RunFail::Limit);
        }
        if self.insts.len() >= self.limits.max_instances {
            self.limit("max_instances");
            return Err(RunFail::Refused);
        }
        if ctx.depth >= self.limits.max_instance_depth {
            self.limit("max_instance_depth");
            return Err(RunFail::Refused);
        }
        let mut root = Instance::new(
            None,
            None,
            entity,
            ctx.depth,
            ctx.intensity,
            1.0,
            true,
            ctx.params,
            ctx.permission,
            0,
        );
        root.kind = KindState::RunRoot { key };
        let id = self.insts.insert(root);
        if let Some(e) = self.entities.get_mut(&entity) {
            e.instances += 1;
            e.runs.entry(key).or_default().push(id);
        }
        self.stats.runs_started += 1;
        self.notify_watchers(sink, entity, key);
        if let Some(i) = self.insts.get_mut(id) {
            i.filling += 1;
        }
        for &r in &def.roots {
            if self.stopped_or_ancestor(id) {
                break;
            }
            self.start_node(sink, id, r, 0);
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.filling = i.filling.saturating_sub(1);
            i.armed = true;
        }
        self.check_complete(sink, id, REASON_STOP);
        Ok(self.alive(id).then_some(id))
    }

    /// Stops every run of `key` on `entity`; returns how many there were.
    pub(crate) fn stop_runs(
        &mut self,
        sink: &mut dyn EffectSink,
        entity: EntityId,
        key: u32,
        reason: u8,
    ) -> usize {
        let runs: Vec<InstanceId> = self
            .entities
            .get(&entity)
            .and_then(|e| e.runs.get(&key))
            .cloned()
            .unwrap_or_default();
        let n = runs.len();
        for r in runs {
            self.stop(sink, r, reason);
        }
        n
    }

    /// A Run request to another entity, at the scheduler pass after it was made (§12.4).
    pub(crate) fn deferred_run(&mut self, sink: &mut dyn EffectSink, d: DeferredRun) {
        let result = self.start_run(sink, d.target, d.key, d.ctx);
        let Some(origin) = d.origin else {
            return;
        };
        let origin_live = self.alive(origin);
        match result {
            Ok(Some(root)) => {
                if origin_live {
                    if let Some(i) = self.insts.get_mut(origin)
                        && let KindState::Run(r) = &mut i.kind
                    {
                        r.started.push(root);
                    }
                } else if !d.unbind {
                    self.stop(sink, root, REASON_STOP);
                }
            }
            Ok(None) => {}
            Err(_) => {
                if !origin_live {
                    return;
                }
                let all_failed = match self.insts.get_mut(origin).map(|i| &mut i.kind) {
                    Some(KindState::Run(r)) => {
                        r.failed += 1;
                        r.failed >= r.targets && r.started.is_empty()
                    }
                    _ => false,
                };
                if all_failed {
                    self.stop(sink, origin, REASON_STOP);
                }
            }
        }
    }

    /// The entities a node acts on: its targets, else the running entity (§4.4 of
    /// `script-table.md`). `ThePlayer` (4) and `This` (13) resolve; other target types are
    /// reported and skipped. Empty when nothing resolves.
    pub(crate) fn resolve_targets(&mut self, n: &CNode, running: EntityId) -> Vec<EntityId> {
        if n.targets.is_empty() {
            return vec![running];
        }
        let mut out = Vec::new();
        for t in &n.targets {
            match t.target_type {
                13 => out.push(running),
                4 => {
                    if let Some(p) = self.player {
                        out.push(p);
                    }
                }
                other => self.issue(RuntimeIssue::UnsupportedTarget {
                    kind: n.kind,
                    target_type: other,
                }),
            }
        }
        out.dedup();
        out
    }

    /// Entity bookkeeping for a kind that needs the entity (used by sound starts).
    pub(crate) fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Whether a run of `key` is live on `entity`.
    pub(crate) fn runs_live(&self, entity: EntityId, key: u32) -> bool {
        self.entities
            .get(&entity)
            .and_then(|e| e.runs.get(&key))
            .is_some_and(|v| !v.is_empty())
    }

    /// The kind of a node, for issue reports.
    pub(crate) fn report_unimplemented(&mut self, kind: NodeKind) {
        self.issue(RuntimeIssue::Unimplemented(kind));
    }
}

/// Whether an instance waits for a scheduled event of its own.
fn has_pending(i: &Instance) -> bool {
    match &i.kind {
        KindState::Sequence(s) => s.pending.is_some(),
        KindState::Timer(t) => t.pending.is_some(),
        _ => false,
    }
}

/// Writes `field` = `v` into an override list (replacing an earlier value).
fn set_override(list: &mut Vec<(u32, PropertyValue)>, field: u32, v: PropertyValue) {
    match list.iter_mut().find(|(p, _)| *p == field) {
        Some(slot) => slot.1 = v,
        None => list.push((field, v)),
    }
}

/// Removes `id` from the list under `key`.
fn remove_from(map: &mut super::Registry, key: (EntityId, u32), id: InstanceId) {
    if let Some(v) = map.get_mut(&key) {
        v.retain(|x| *x != id);
        if v.is_empty() {
            map.remove(&key);
        }
    }
}

/// The property IDs of a Switch's case int lists (§6.3).
pub(crate) fn switch_int_pids(n: &CNode) -> impl Iterator<Item = u32> + '_ {
    n.ports.iter().flat_map(|p| match &p.payload {
        tessera_formats::script_table::PortPayload::SwitchCase(c) => c
            .ints
            .iter()
            .map(|&x| x.cast_unsigned())
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    })
}
