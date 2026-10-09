// SPDX-License-Identifier: GPL-3.0-or-later
//! Kind starts and stop hooks (`script-runtime.md` §8, §9, §12–§18; `sound.md` §9.2, §9.4).
//! Switch and When are in `switch.rs`, Sequence and Timer in `timing.rs`.

use std::sync::Arc;

use rand::Rng;
use tessera_formats::script_table::{NodeData, NodeKind, PortPayload, PortType, SoundNode};
use tessera_formats::{Did, PropertyValue};

use super::core::REASON_STOP;
use super::instance::{ParticleState, RunState, WhenRunningState};
use super::{DeferredRun, Event, INTENSITY_EPSILON, InstanceId, KindState, RunCtx, Runtime};
use crate::graph::{NodeRef, Unit};
use crate::sink::{
    AnimationOutcome, EffectSink, EntityId, EnvironmentRequest, ParticleStart, SoundPositioner,
    SoundStart, SoundStartOutcome,
};
use crate::values::{as_bool, as_f32, as_i32, as_u32, as_vec3};

/// A fire-and-forget Sound at or below this volume completes without a request
/// (`sound.md` §9.2 step 9).
pub const FIRE_AND_FORGET_MIN_VOLUME: f32 = 0.005;
/// Lowest pitch multiplier (`sound.md` §6.3).
pub const PITCH_FLOOR: f32 = 0.01;
/// Shortest random-offset direction before "straight up" is used (`sound.md` §9.2 step 5).
const DIRECTION_EPSILON: f32 = 1e-6;
/// Longest entity parent chain followed for the internal-object mute (§9.2 step 2).
const MAX_PARENT_CHAIN: usize = 64;
/// A finite effect is not started at or below this context intensity
/// (`particle-runtime.md` §5.4 step 3, BIN).
pub const PARTICLE_INTENSITY_GATE: f32 = 0.0002;

impl Runtime<'_> {
    /// The kind's start routine (§3.1 step 6). Returns whether the instance may stay; the
    /// caller releases it when it is armed without children (completion at once).
    pub(crate) fn kind_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        match n.kind {
            NodeKind::COLLECTION | NodeKind::LABEL => {
                for &p in &n.ordered {
                    self.run_port(sink, unit, nref.unit, nref.node, p as usize, id, 0);
                    if self.stopped_or_ancestor(id) {
                        break;
                    }
                }
                self.arm(id);
                true
            }
            NodeKind::RANDOM => self.random_start(sink, id, unit, nref),
            NodeKind::SEQUENCE => self.sequence_start(sink, id, unit, nref),
            NodeKind::TIMER => self.timer_start(id, unit, nref),
            NodeKind::SWITCH | NodeKind::WHEN => self.switch_start(sink, id, unit, nref),
            NodeKind::WHEN_RUNNING => self.when_running_start(sink, id, unit, nref),
            NodeKind::RUN => self.run_start(sink, id, unit, nref),
            NodeKind::SET_PROPERTY => {
                self.set_property_start(sink, id, unit, nref);
                false
            }
            NodeKind::GOTO => self.goto_start(sink, id, unit, nref),
            NodeKind::ON_STOP => {
                if let Some(i) = self.insts.get_mut(id) {
                    i.kind = KindState::OnStop;
                }
                true
            }
            NodeKind::SCRIPTLET => self.scriptlet_start(sink, id, unit, nref),
            NodeKind::SCRIPTLET_PARAMETER => self.scriptlet_parameter_start(sink, id, unit, nref),
            NodeKind::SOUND => self.sound_start(sink, id, unit, nref),
            NodeKind::SOUND_ENVIRONMENT => self.environment_start(sink, id, unit, nref),
            NodeKind::ANIMATION => self.animation_start(sink, id, unit, nref),
            NodeKind::PARTICLE => self.particle_start(sink, id, unit, nref),
            NodeKind::SCALAR_BLEND => self.scalar_blend_start(sink, id, unit, nref),
            NodeKind::GAME_CALLBACK => {
                self.issue(crate::RuntimeIssue::GameCallbackOpaque);
                false
            }
            // Native callbacks and gameplay hooks: complete at once (§18).
            NodeKind::EXECUTE | NodeKind::STORY_HOOK => false,
            other => {
                self.report_unimplemented(other);
                false
            }
        }
    }

    /// The kind's stop hook (§3.5 step 2).
    pub(crate) fn stop_hook(&mut self, sink: &mut dyn EffectSink, id: InstanceId, reason: u8) {
        let Some(i) = self.insts.get_mut(id) else {
            return;
        };
        let node = i.node;
        let hook = match &mut i.kind {
            KindState::Sound { handle, completed } => Hook::Sound(*handle, *completed),
            KindState::Environment { handle } => Hook::Environment(*handle),
            KindState::Timer(t) => Hook::Cancel(vec![t.pending.take()]),
            KindState::Sequence(s) => Hook::Cancel(vec![s.pending.take()]),
            KindState::Switch(s) => Hook::Cancel(vec![s.delay_event.take(), s.fade_event.take()]),
            KindState::Animation { handle, event } => Hook::Animation(handle.take(), event.take()),
            KindState::Particle(p) => Hook::Particle(p.handle.take(), p.immediate),
            KindState::Run(r) if !r.unbind => Hook::Run(std::mem::take(&mut r.started)),
            KindState::OnStop if reason != 2 => Hook::OnStop,
            _ => Hook::None,
        };
        match hook {
            Hook::None => {}
            Hook::Sound(h, completed) => {
                self.sounds.remove(&h);
                if !completed {
                    sink.stop_sound(h);
                }
            }
            Hook::Environment(h) => sink.unregister_environment(h),
            Hook::Cancel(keys) => {
                for k in keys.into_iter().flatten() {
                    self.sched.cancel(k);
                }
            }
            Hook::Animation(h, ev) => {
                if let Some(k) = ev {
                    self.sched.cancel(k);
                }
                if let Some(h) = h {
                    sink.stop_animation(h);
                }
            }
            Hook::Particle(h, immediate) => {
                if let Some(h) = h {
                    sink.stop_particle(h, immediate);
                }
            }
            Hook::Run(started) => {
                for root in started {
                    self.stop(sink, root, reason);
                }
            }
            Hook::OnStop => {
                // §16: run the Default children during the stop, not on teardown. Each one
                // that would stay running is stopped at once (the OnStop is stopped).
                let Some(nref) = node else { return };
                let Some(unit) = self.units.get(nref.unit as usize).cloned() else {
                    return;
                };
                let Some(n) = unit.node(nref.node) else {
                    return;
                };
                for (_, port) in n.ports_of(PortType::DEFAULT) {
                    for &c in &port.children {
                        self.start_node(
                            sink,
                            id,
                            NodeRef {
                                unit: nref.unit,
                                node: c,
                            },
                            0,
                        );
                    }
                }
            }
        }
    }

    // ----- Random (§8) -----

    fn random_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let no_repeats = matches!(
            n.data,
            NodeData::Random {
                no_repeats: true,
                ..
            }
        );
        let ports: Vec<usize> = n.ports_of(PortType::DEFAULT).map(|(i, _)| i).collect();
        // A node with driven fields runs on a fresh copy: no memory (§8, Inferred).
        let copy = !n.base.driven.is_empty() || !n.base.scriptlet_driven.is_empty();
        let weight = |i: usize| -> u64 {
            match n.ports.get(i).map(|p| &p.payload) {
                Some(PortPayload::U32(w)) => u64::from(*w),
                _ => 0,
            }
        };
        let pick = match ports.as_slice() {
            [] => None,
            [only] => Some(*only),
            _ => {
                let last = if no_repeats && !copy {
                    self.random_memory.get(&nref).map(|&x| x as usize)
                } else {
                    None
                };
                let eligible = |i: &&usize| Some(**i) != last;
                let sum: u64 = ports.iter().filter(eligible).map(|&i| weight(i)).sum();
                if sum == 0 {
                    self.random_memory.remove(&nref);
                    None
                } else {
                    let r = self.rng.random_range(0..sum);
                    let mut acc = 0u64;
                    let mut chosen = None;
                    for &i in ports.iter().filter(eligible) {
                        acc += weight(i);
                        if acc > r {
                            chosen = Some(i);
                            break;
                        }
                    }
                    if let Some(c) = chosen
                        && !copy
                    {
                        self.random_memory
                            .insert(nref, u32::try_from(c).unwrap_or(u32::MAX));
                    }
                    chosen
                }
            }
        };
        if let Some(p) = pick {
            self.run_port(sink, unit, nref.unit, nref.node, p, id, 0);
        }
        self.arm(id);
        true
    }

    // ----- Run (§12.4) -----

    fn run_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::Run(r) = &n.data else {
            return false;
        };
        let Some((entity, intensity, params, permission, depth)) = self.insts.get(id).map(|i| {
            (
                i.entity,
                i.intensity,
                i.params.clone(),
                i.permission,
                i.depth,
            )
        }) else {
            return false;
        };
        let targets = self.resolve_targets(n, entity);
        if r.stop {
            for t in targets {
                self.stop_runs(sink, t, r.script, REASON_STOP);
            }
            return false;
        }
        if targets.is_empty() {
            return false;
        }
        // The started run inherits the Run's context, plus the parameter entries and the
        // starting entity (§12.4 step 2).
        let mut run_params = (*params).clone();
        for p in &r.parameters {
            let v = match &p.constant {
                Some(e) => Some(e.value.clone()),
                None if p.source != 0 => self.read_value(entity, &params, p.source),
                None => None,
            };
            if let Some(v) = v {
                run_params.insert(p.target, v);
            }
        }
        if let Some(src) = self.vocab.run_output_source {
            run_params.insert(src, PropertyValue::InstanceId(entity.0));
        }
        let ctx = RunCtx {
            intensity,
            params: Arc::new(run_params),
            permission: if r.persistent { permission } else { true },
            // A started run continues this Run's depth, so run chains stay bounded.
            depth: depth.saturating_add(1),
        };
        let mut started = Vec::new();
        let mut failed = 0u32;
        for &t in &targets {
            if t == entity {
                match self.start_run(sink, t, r.script, ctx.clone()) {
                    Ok(Some(root)) => started.push(root),
                    Ok(None) => {}
                    Err(_) => failed += 1,
                }
            } else {
                // "Deferred to the next scheduler pass" (§12.4 step 2): inside an advance,
                // the next advance (Decision, see `script-runtime.md` §29).
                let due = if self.in_advance {
                    self.sched.frame_time.next_up()
                } else {
                    self.sched.now
                };
                self.schedule(
                    due,
                    Event::DeferredRun(Box::new(DeferredRun {
                        target: t,
                        key: r.script,
                        ctx: ctx.clone(),
                        origin: r.persistent.then_some(id),
                        unbind: r.unbind_on_death,
                    })),
                );
            }
        }
        if !r.persistent {
            return false;
        }
        let total = u32::try_from(targets.len()).unwrap_or(u32::MAX);
        if failed >= total {
            return false;
        }
        if !self.alive(id) {
            return false;
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Run(Box::new(RunState {
                started,
                unbind: r.unbind_on_death,
                targets: total,
                failed,
            }));
        }
        true
    }

    // ----- WhenRunning (§13) -----

    fn when_running_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::WhenRunning {
            scripts,
            names,
            persistent,
        } = &n.data
        else {
            return false;
        };
        let Some((entity, params)) = self.insts.get(id).map(|i| (i.entity, i.params.clone()))
        else {
            return false;
        };
        let target = self
            .resolve_targets(n, entity)
            .first()
            .copied()
            .unwrap_or(entity);
        let mut keys = scripts.clone();
        for &pid in names {
            if let Some(k) = self
                .read_value(target, &params, pid)
                .as_ref()
                .and_then(as_u32)
            {
                keys.push(k);
            }
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::WhenRunning(Box::new(WhenRunningState {
                target,
                keys: keys.clone(),
                running_tag: 0,
                not_running_tag: 0,
            }));
        }
        if *persistent {
            for k in keys {
                self.watch(id, target, k);
            }
        }
        self.when_running_evaluate(sink, id);
        if !*persistent {
            self.arm(id);
        }
        true
    }

    /// One evaluation of a WhenRunning instance (§13).
    pub(crate) fn when_running_evaluate(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some((nref, target, keys, run_tag, not_tag)) = self.insts.get(id).and_then(|i| {
            let KindState::WhenRunning(w) = &i.kind else {
                return None;
            };
            Some((
                i.node?,
                w.target,
                w.keys.clone(),
                w.running_tag,
                w.not_running_tag,
            ))
        }) else {
            return;
        };
        let running = keys.iter().any(|&k| self.runs_live(target, k));
        let (stop_tag, start_tag, port_type) = if running {
            (not_tag, run_tag, PortType::RUNNING)
        } else {
            (run_tag, not_tag, PortType::NOT_RUNNING)
        };
        if stop_tag != 0 {
            self.stop_group(sink, id, stop_tag, REASON_STOP);
            self.set_when_running_tag(id, !running, 0);
        }
        let busy = start_tag != 0 && !self.group_children(id, start_tag).is_empty();
        if busy {
            return;
        }
        let tag = match self.insts.get_mut(id) {
            Some(i) => {
                if start_tag != 0 {
                    i.close_group(start_tag);
                }
                i.open_group()
            }
            None => return,
        };
        self.set_when_running_tag(id, running, tag);
        let Some(unit) = self.units.get(nref.unit as usize).cloned() else {
            return;
        };
        let ports: Vec<usize> = unit
            .node(nref.node)
            .map(|n| n.ports_of(port_type).map(|(i, _)| i).collect())
            .unwrap_or_default();
        for p in ports {
            self.run_port(sink, &unit, nref.unit, nref.node, p, id, tag);
            if !self.alive(id) || !self.insts.get(id).is_some_and(|i| i.group_live(tag)) {
                break;
            }
        }
    }

    fn set_when_running_tag(&mut self, id: InstanceId, running: bool, tag: u32) {
        if let Some(i) = self.insts.get_mut(id)
            && let KindState::WhenRunning(w) = &mut i.kind
        {
            if running {
                w.running_tag = tag;
            } else {
                w.not_running_tag = tag;
            }
        }
    }

    // ----- SetProperty (§14) -----

    fn set_property_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) {
        let Some(n) = unit.node(nref.node) else {
            return;
        };
        let NodeData::SetProperty {
            property,
            value,
            server_only,
        } = &n.data
        else {
            return;
        };
        if *server_only {
            return;
        }
        let Some(entity) = self.insts.get(id).map(|i| i.entity) else {
            return;
        };
        // A driven pair on the property itself replaces the constant (`script-table.md` §16
        // OPEN: the field PID of the value is not named; Decision: the property's own PID).
        let v = self
            .insts
            .get(id)
            .and_then(|i| i.override_of(Some(*property)).cloned())
            .or_else(|| value.as_ref().map(|e| e.value.clone()));
        let Some(v) = v else {
            return;
        };
        for t in self.resolve_targets(n, entity) {
            self.write_property(sink, t, *property, v.clone());
        }
    }

    // ----- Goto (§15) -----

    fn goto_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(NodeData::Goto { name }) = unit.node(nref.node).map(|n| &n.data) else {
            return false;
        };
        let Some(&label) = unit.labels.get(name) else {
            self.issue(crate::RuntimeIssue::LabelNotFound(name.clone()));
            return false;
        };
        self.start_node(
            sink,
            id,
            NodeRef {
                unit: nref.unit,
                node: label,
            },
            0,
        );
        self.arm(id);
        true
    }

    // ----- Scriptlet and ScriptletParameter (§17) -----

    fn scriptlet_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(NodeData::Scriptlet { file, parameters }) = unit.node(nref.node).map(|n| &n.data)
        else {
            return false;
        };
        if file.is_null() {
            return false;
        }
        let depth = self.insts.get(id).map_or(0, |i| i.scriptlet_depth);
        if depth >= self.limits.max_scriptlet_depth {
            self.limit("max_scriptlet_depth");
            return false;
        }
        let Some(sid) = self.scriptlet_unit(*file) else {
            return false;
        };
        let Some(sunit) = self.units.get(sid as usize).cloned() else {
            return false;
        };
        if let Some(i) = self.insts.get_mut(id) {
            let mut merged = (*i.params).clone();
            for e in parameters {
                merged.insert(e.pid, e.value.clone());
            }
            i.params = Arc::new(merged);
            i.kind = KindState::Scriptlet { unit: sid };
            i.scriptlet_depth = depth + 1;
            i.filling += 1;
        }
        for &r in &sunit.roots {
            self.start_node(sink, id, NodeRef { unit: sid, node: r }, 0);
            if self.stopped_or_ancestor(id) {
                break;
            }
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.filling = i.filling.saturating_sub(1);
            i.armed = true;
        }
        true
    }

    fn scriptlet_parameter_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(NodeData::ScriptletParameter { name }) = unit.node(nref.node).map(|n| &n.data)
        else {
            return false;
        };
        let Some(parent) = self.insts.get(id).and_then(|i| i.parent) else {
            return false;
        };
        // The nearest Scriptlet instance that invoked this parameter's record.
        let Some(sc) = self.enclosing_scriptlet(parent, nref.unit) else {
            return false;
        };
        let Some(snode) = self.insts.get(sc).and_then(|i| i.node) else {
            return false;
        };
        let Some(sunit) = self.units.get(snode.unit as usize).cloned() else {
            return false;
        };
        let port = sunit.node(snode.node).and_then(|n| {
            n.ports_of(PortType::DEFAULT)
                .find_map(|(i, p)| match &p.payload {
                    PortPayload::Name(s) if s.eq_ignore_ascii_case(name) => Some(i),
                    _ => None,
                })
        });
        let Some(port) = port else {
            return false;
        };
        self.run_port(sink, &sunit, snode.unit, snode.node, port, id, 0);
        self.arm(id);
        true
    }

    // ----- Sound (sound.md §9.2) -----

    /// The Sound node's fields with the driven values of this start applied.
    fn effective_sound(&self, id: InstanceId, stored: &SoundNode) -> SoundNode {
        let mut s = *stored;
        let Some(i) = self.insts.get(id) else {
            return s;
        };
        let f = &self.vocab.sound;
        if let Some(v) = i.override_of(f.sound_id).and_then(as_u32) {
            s.sound = Did(v);
        }
        if let Some(v) = i.override_of(f.fire_and_forget).and_then(as_bool) {
            s.fire_and_forget = v;
        }
        if let Some(v) = i.override_of(f.move_with_entity).and_then(as_bool) {
            s.move_with_entity = v;
        }
        if let Some(v) = i.override_of(f.use_camera_position).and_then(as_bool) {
            s.use_camera_position = v;
        }
        if let Some(v) = i.override_of(f.min_random_distance).and_then(as_f32) {
            s.min_random_distance = v;
        }
        if let Some(v) = i.override_of(f.max_random_distance).and_then(as_f32) {
            s.max_random_distance = v;
        }
        if let Some(v) = i.override_of(f.offset).and_then(as_vec3) {
            s.offset = v;
        }
        if let Some(v) = i.override_of(f.channel).and_then(as_u32) {
            s.channel = v;
        }
        if let Some(v) = i.override_of(f.ramp_out_time).and_then(as_f32) {
            s.ramp_out_time = v;
        }
        if let Some(v) = i.override_of(f.pitch).and_then(as_f32) {
            s.pitch = v;
        }
        if let Some(v) = i.override_of(f.pitch_variance).and_then(as_f32) {
            s.pitch_variance = v;
        }
        s
    }

    /// `sound.md` §9.2 steps 1–12.
    #[allow(clippy::too_many_lines)]
    fn sound_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::Sound(stored) = &n.data else {
            return false;
        };
        let s = self.effective_sound(id, stored);
        let Some((entity, intensity)) = self.insts.get(id).map(|i| (i.entity, i.intensity)) else {
            return false;
        };
        // 1. A null sound info fails (completes at once).
        if s.sound.is_null() || self.entity(entity).is_none() {
            return false;
        }
        // 2. Internal-object mute; 3. the topmost entity of the chain.
        let mut cur = entity;
        for _ in 0..MAX_PARENT_CHAIN {
            let parent = self.entity(cur).and_then(|e| e.parent);
            if self.entity_bool(cur, self.vocab.mute_when_internal) {
                if parent.is_some() {
                    return false;
                }
                break;
            }
            match parent {
                Some(p) if self.entity(p).is_some() => cur = p,
                _ => break,
            }
        }
        let sound_entity = match self.viewport {
            Some(v) if self.entity_bool(cur, self.vocab.run_in_viewport) => v,
            _ => entity,
        };
        // 4. "No effects" is never set (Decision).
        // 5. Random offset.
        let (min, max) = s.random_distance_range();
        let mut offset = s.offset.map(|c| if c.is_finite() { c } else { 0.0 });
        if max > 0.0 {
            let d: [f32; 3] = [
                self.rng.random_range(-1.0..=1.0),
                self.rng.random_range(-1.0..=1.0),
                self.rng.random_range(-1.0..=1.0),
            ];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let dir = if len <= DIRECTION_EPSILON {
                [0.0, 0.0, 1.0]
            } else {
                [d[0] / len, d[1] / len, d[2] / len]
            };
            let dist = if max > min {
                self.rng.random_range(min..=max)
            } else {
                min
            };
            for (o, c) in offset.iter_mut().zip(dir) {
                *o += c * dist;
            }
        }
        // 6. Positioner.
        let positioner = if s.use_camera_position {
            SoundPositioner::Camera {
                entity: sound_entity,
                offset,
            }
        } else if s.move_with_entity {
            SoundPositioner::FollowEntity {
                entity: sound_entity,
                offset,
            }
        } else {
            SoundPositioner::Frozen {
                entity: sound_entity,
                offset,
            }
        };
        // 7. Volume.
        let volume = if n.base.use_parent_intensity {
            intensity
        } else {
            let mult = self
                .insts
                .get(id)
                .and_then(|i| i.override_of(self.vocab.intensity_multiplier))
                .and_then(as_f32)
                .unwrap_or(n.base.intensity_multiplier);
            let var = n.base.intensity_variance;
            if var == 0.0 {
                mult
            } else {
                mult + (self.rng.random::<f32>() * 2.0 - 1.0) * var
            }
        };
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.0
        };
        // 8. Pitch (§6.3; a multiplier ≤ 0 becomes the floor, §6.3 OPEN Decision).
        let mut pitch = (s.pitch + 2.0) * 0.5;
        if s.pitch_variance > 0.0 {
            pitch += (self.rng.random::<f32>() * 2.0 - 1.0) * s.pitch_variance;
            pitch = pitch.max(PITCH_FLOOR);
        }
        if !(pitch.is_finite() && pitch > 0.0) {
            pitch = PITCH_FLOOR;
        }
        // 9. Inaudible fire-and-forget sounds are not requested.
        if s.fire_and_forget && volume <= FIRE_AND_FORGET_MIN_VOLUME {
            return false;
        }
        // 10. The request.
        let request = SoundStart {
            entity,
            sound: s.sound,
            volume,
            pitch,
            channel: s.channel,
            ramp_out: s.ramp_out_time,
            tracked: !s.fire_and_forget,
            positioner,
            node_id: n.base.node_id,
        };
        self.stats.sounds_requested += 1;
        let outcome = sink.start_sound(&request);
        let handle = match outcome {
            SoundStartOutcome::Accepted(h) => {
                self.stats.sounds_accepted += 1;
                h
            }
            SoundStartOutcome::AcceptedSilent => {
                self.stats.sounds_accepted += 1;
                return false;
            }
            SoundStartOutcome::Refused => return false,
        };
        // 11. Fire-and-forget completes now; 12. tracked stays running.
        if s.fire_and_forget {
            return false;
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Sound {
                handle,
                completed: false,
            };
        }
        self.sounds.insert(handle, id);
        true
    }

    // ----- SoundEnvironment (sound.md §8, §9.4) -----

    fn environment_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(NodeData::SoundEnvironment { name, priority }) =
            unit.node(nref.node).map(|n| &n.data)
        else {
            return false;
        };
        let Some(i) = self.insts.get(id) else {
            return false;
        };
        let name = i
            .override_of(self.vocab.environment_name)
            .and_then(as_u32)
            .unwrap_or(*name);
        let priority = i
            .override_of(self.vocab.environment_priority)
            .and_then(as_i32)
            .unwrap_or(*priority);
        let request = EnvironmentRequest {
            entity: i.entity,
            name,
            priority,
            intensity: i.intensity,
        };
        let handle = sink.register_environment(&request);
        self.stats.environments_registered += 1;
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Environment { handle };
        }
        true
    }

    // ----- Animation (§18) -----

    fn animation_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(NodeData::Animation(a)) = unit.node(nref.node).map(|n| &n.data) else {
            return false;
        };
        let Some((entity, intensity)) = self.insts.get(id).map(|i| (i.entity, i.intensity)) else {
            return false;
        };
        match sink.start_animation(entity, a, intensity) {
            AnimationOutcome::CompleteAtOnce => false,
            AnimationOutcome::RunFor(h, secs) => {
                let secs = if secs.is_finite() && secs > 0.0 {
                    secs
                } else {
                    0.0
                };
                let due = self.sched.now + secs;
                let key = self.schedule(due, Event::AnimationEnd(id));
                if let Some(i) = self.insts.get_mut(id) {
                    i.kind = KindState::Animation {
                        handle: Some(h),
                        event: Some(key),
                    };
                }
                true
            }
            AnimationOutcome::RunUntilStopped(h) => {
                if let Some(i) = self.insts.get_mut(id) {
                    i.kind = KindState::Animation {
                        handle: Some(h),
                        event: None,
                    };
                }
                true
            }
        }
    }

    // ----- Particle (`particle-runtime.md` §5.4–§5.8) -----

    /// §5.4: the effect DID, the classification (the stored byte, rule §12.2), the
    /// intensity gate of finite effects (context flag 0x40 treated as clear, Q-RT2), then
    /// the sink takes the dropped or the attached path. A finite effect, and every effect on
    /// the dropped path, completes at once; a never-ending attached effect leaves an
    /// instance (with an invalid handle when nothing started) that runs until it is stopped
    /// and listens to the redirect property (§5.5 step 5, §5.8).
    fn particle_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::Particle(p) = &n.data else {
            return false;
        };
        let p = **p;
        // 1. No effect DID: the start fails.
        let Some(effect) = p.effect_did() else {
            return false;
        };
        let Some((entity, intensity)) = self.insts.get(id).map(|i| (i.entity, i.intensity)) else {
            return false;
        };
        // 2. Classification.
        let never_ending = n
            .particle_flag
            .or_else(|| sink.classify_particle(effect))
            .unwrap_or(false);
        // 3. Intensity gate (finite effects only).
        // NaN counts as at or below the gate (the start needs intensity > 0.0002).
        let above_gate =
            intensity.partial_cmp(&PARTICLE_INTENSITY_GATE) == Some(std::cmp::Ordering::Greater);
        if !never_ending && !above_gate {
            self.stats.particles_gated += 1;
            return false;
        }
        // 4–5. Dropped or attached path.
        let target = if p.drop_effect {
            entity
        } else {
            self.particle_target(sink, entity, p.redirect_property)
        };
        let request = ParticleStart {
            entity,
            target,
            effect,
            node: p,
            never_ending,
            node_id: n.base.node_id,
        };
        self.stats.particles_requested += 1;
        let handle = sink.start_particle(&request);
        if !never_ending || p.drop_effect {
            return false;
        }
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Particle(Box::new(ParticleState {
                handle,
                target,
                immediate: p.immediate_disposal,
                redirect: p.redirect_property,
            }));
        }
        if p.redirect_property != 0 {
            self.listen(id, entity, p.redirect_property);
        }
        true
    }

    /// §5.5 step 1: the script's entity, unless `redirect` ≠ 0 and the entity's value of
    /// that property names an existing object (an entity of this runtime, or one the sink
    /// knows).
    fn particle_target(
        &mut self,
        sink: &mut dyn EffectSink,
        entity: EntityId,
        redirect: u32,
    ) -> EntityId {
        if redirect == 0 {
            return entity;
        }
        let Some(v) = self
            .entity_value(entity, redirect)
            .and_then(|v| v.as_bits())
            .filter(|&v| v != 0)
        else {
            return entity;
        };
        if self.entities.contains_key(&EntityId(v)) || sink.object_exists(v) {
            EntityId(v)
        } else {
            entity
        }
    }

    /// §5.8: the redirect property of a running never-ending Particle node changed: stop
    /// the effect as in §7.1 (with the node's `ImmediateDisposal`) and start it again on
    /// the newly named object by §5.5 steps 1–4, replacing the handle and the target.
    pub(crate) fn particle_redirect(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        node: &crate::graph::CNode,
    ) {
        let NodeData::Particle(p) = &node.data else {
            return;
        };
        let p = **p;
        let Some(effect) = p.effect_did() else {
            return;
        };
        let Some((entity, old, immediate)) = self.insts.get_mut(id).and_then(|i| {
            let entity = i.entity;
            match &mut i.kind {
                KindState::Particle(s) => Some((entity, s.handle.take(), s.immediate)),
                _ => None,
            }
        }) else {
            return;
        };
        if let Some(h) = old {
            sink.stop_particle(h, immediate);
        }
        let target = self.particle_target(sink, entity, p.redirect_property);
        let request = ParticleStart {
            entity,
            target,
            effect,
            node: p,
            never_ending: true,
            node_id: node.base.node_id,
        };
        self.stats.particles_requested += 1;
        let handle = sink.start_particle(&request);
        if let Some(KindState::Particle(s)) = self.insts.get_mut(id).map(|i| &mut i.kind) {
            s.handle = handle;
            s.target = target;
        }
    }

    // ----- ScalarBlend (`script-table.md` §7.3) -----

    /// Starts the children of the ports whose blend weight for the property's current
    /// value is above 0 (the one or two anchors bracketing it, or the nearest one outside
    /// the range), once, as a container. Decision: the weights are not applied to the
    /// children's intensity and the node does not follow later changes
    /// (`script-runtime.md` §18 OPEN-17).
    fn scalar_blend_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::ScalarBlend { property, .. } = &n.data else {
            return false;
        };
        let Some((entity, params)) = self.insts.get(id).map(|i| (i.entity, i.params.clone()))
        else {
            return false;
        };
        let p = self
            .read_value(entity, &params, *property)
            .as_ref()
            .and_then(as_f32)
            .unwrap_or(0.0);
        let mut anchors: Vec<(f32, usize)> = n
            .ports_of(PortType::DEFAULT)
            .filter_map(|(i, port)| match port.payload {
                PortPayload::F32(a) if a.is_finite() => Some((a, i)),
                _ => None,
            })
            .collect();
        anchors.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut picked: Vec<usize> = Vec::new();
        if let (Some(first), Some(last)) = (anchors.first(), anchors.last()) {
            if p <= first.0 {
                picked.push(first.1);
            } else if p >= last.0 {
                picked.push(last.1);
            } else {
                for w in anchors.windows(2) {
                    let (lo, hi) = (w[0], w[1]);
                    if p >= lo.0 && p <= hi.0 {
                        if p < hi.0 || hi.0 == lo.0 {
                            picked.push(lo.1);
                        }
                        if p > lo.0 {
                            picked.push(hi.1);
                        }
                        break;
                    }
                }
            }
        }
        for port in picked {
            self.run_port(sink, unit, nref.unit, nref.node, port, id, 0);
            if self.stopped_or_ancestor(id) {
                break;
            }
        }
        self.arm(id);
        true
    }
}

/// Whether an intensity counts as silent (§11.3).
pub(crate) fn silent(intensity: f32) -> bool {
    intensity <= INTENSITY_EPSILON
}

/// What a stop hook does, taken out of the instance first.
enum Hook {
    None,
    Sound(crate::sink::SoundHandle, bool),
    Environment(crate::sink::EnvironmentHandle),
    Cancel(Vec<Option<crate::sched::EventKey>>),
    Animation(
        Option<crate::sink::AnimationHandle>,
        Option<crate::sched::EventKey>,
    ),
    Particle(Option<crate::sink::ParticleHandle>, bool),
    Run(Vec<InstanceId>),
    OnStop,
}
