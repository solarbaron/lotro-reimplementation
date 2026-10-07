// SPDX-License-Identifier: GPL-3.0-or-later
//! Switch (12) and When (63): `script-runtime.md` §6 and §7.

use std::sync::Arc;

use tessera_formats::PropertyValue;
use tessera_formats::script_table::{NodeData, NodeKind, PortPayload, PortType, SwitchTransition};

use super::core::{REASON_STOP, switch_int_pids};
use super::instance::{Fade, SwitchState};
use super::{Event, INTENSITY_EPSILON, InstanceId, KindState, Runtime, SwitchDelay};
use crate::graph::{CNode, NodeRef, Unit};
use crate::sink::EffectSink;
use crate::values::{case_matches, opt_equal, values_equal, when_holds};

/// (RampOut, Delay) of a value or Default port (§6.1); (0, 0) for other ports.
fn port_floats(n: &CNode, port: usize) -> (f32, f32) {
    let clean = |x: f32| if x.is_finite() { x } else { 0.0 };
    match n.ports.get(port).map(|p| &p.payload) {
        Some(PortPayload::SwitchCase(c)) => (clean(c.transition_time), clean(c.second_time)),
        Some(PortPayload::F32Pair(a, b)) => (clean(*a), clean(*b)),
        _ => (0.0, 0.0),
    }
}

/// The transition port fields of §6.1: (FromIsDefault, ToIsDefault, Delay, Wait, RampIn).
/// The second stored Delay is the one kept ("stored over the first").
fn transition_fields(t: &SwitchTransition) -> (bool, bool, f32, bool, f32) {
    let clean = |x: f32| if x.is_finite() { x } else { 0.0 };
    (
        t.flags[0],
        t.flags[1],
        clean(t.floats[3]),
        t.flags[2],
        clean(t.floats[1]),
    )
}

/// Whether an optional stored PV equals an optional value.
fn pv_equals(
    pv: Option<&tessera_formats::property::PropertyEntry>,
    v: Option<&PropertyValue>,
) -> bool {
    match (pv, v) {
        (Some(e), Some(v)) => values_equal(&e.value, v),
        _ => false,
    }
}

impl Runtime<'_> {
    fn switch_state(&self, id: InstanceId) -> Option<&SwitchState> {
        match &self.insts.get(id)?.kind {
            KindState::Switch(s) => Some(s),
            _ => None,
        }
    }

    fn switch_state_mut(&mut self, id: InstanceId) -> Option<&mut SwitchState> {
        match &mut self.insts.get_mut(id)?.kind {
            KindState::Switch(s) => Some(s),
            _ => None,
        }
    }

    /// The instance's unit and node index.
    fn switch_node(&self, id: InstanceId) -> Option<(Arc<Unit>, NodeRef)> {
        let nref = self.insts.get(id)?.node?;
        Some((self.units.get(nref.unit as usize)?.clone(), nref))
    }

    pub(crate) fn switch_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::Switch(sw) = &n.data else {
            return false;
        };
        let is_when = n.kind == NodeKind::WHEN;
        let Some(entity) = self.insts.get(id).map(|i| i.entity) else {
            return false;
        };
        let target = self
            .resolve_targets(n, entity)
            .first()
            .copied()
            .unwrap_or(entity);
        let transition_mode = !is_when
            && sw.persistent
            && (n.ports_of(PortType::SWITCH_TRANSITION).next().is_some()
                || n.ports.iter().enumerate().any(|(i, p)| {
                    matches!(p.port_type, PortType::SWITCH_VALUE | PortType::DEFAULT) && {
                        let (r, d) = port_floats(n, i);
                        r > INTENSITY_EPSILON || d > INTENSITY_EPSILON
                    }
                }));
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Switch(Box::new(SwitchState {
                is_when,
                property: sw.property,
                target,
                persistent: sw.persistent,
                transition_mode,
                value: None,
                current: None,
                current_tag: 0,
                fcv_tag: 0,
                outgoing_tag: 0,
                fade: None,
                fade_event: None,
                delay_event: None,
                waiting: false,
                eval: 0,
            }));
        }
        if sw.persistent {
            // Listen first: a child that changes the property cuts its port short (§3.2).
            self.listen(id, target, sw.property);
            if !is_when {
                let pids: Vec<u32> = switch_int_pids(n).collect();
                for p in pids {
                    self.listen(id, target, p);
                }
            }
        }
        let value = self.switch_value(id);
        if let Some(s) = self.switch_state_mut(id) {
            s.value = value;
        }
        let e = self.bump_eval(id);
        self.run_fcv(sink, id);
        // A ForCurrentValue child that changed the property re-evaluated already and
        // started the case for the newer value.
        if self.alive(id) && self.eval_is(id, e) {
            let port = self.select_port(id);
            self.start_case(sink, id, port);
        }
        if !sw.persistent {
            self.arm(id);
        }
        true
    }

    /// Bumps the evaluation counter and returns the new value.
    fn bump_eval(&mut self, id: InstanceId) -> u64 {
        self.switch_state_mut(id).map_or(0, |s| {
            s.eval = s.eval.wrapping_add(1);
            s.eval
        })
    }

    /// Whether no nested evaluation ran since `bump_eval` returned `e`.
    fn eval_is(&self, id: InstanceId, e: u64) -> bool {
        self.switch_state(id).is_some_and(|s| s.eval == e)
    }

    /// The Switch's current value (§6.2; OPEN-5 Decision through `read_value`).
    fn switch_value(&self, id: InstanceId) -> Option<PropertyValue> {
        let i = self.insts.get(id)?;
        let KindState::Switch(s) = &i.kind else {
            return None;
        };
        self.read_value(s.target, &i.params, s.property)
    }

    /// The selected port for the current value (§6.3, §7); `None` without a value.
    fn select_port(&self, id: InstanceId) -> Option<usize> {
        let i = self.insts.get(id)?;
        let KindState::Switch(s) = &i.kind else {
            return None;
        };
        let nref = i.node?;
        let n = self.units.get(nref.unit as usize)?.node(nref.node)?;
        let Some(v) = s.value.as_ref() else {
            return match self.unset_policy {
                super::UnsetPolicy::NoPort => None,
                super::UnsetPolicy::DefaultPort => {
                    n.ports_of(PortType::DEFAULT).map(|(i, _)| i).next()
                }
            };
        };
        for (idx, p) in n.ports.iter().enumerate() {
            match (&p.payload, p.port_type) {
                (PortPayload::SwitchCase(c), PortType::SWITCH_VALUE) if !s.is_when => {
                    if c.values
                        .iter()
                        .flatten()
                        .any(|cv| case_matches(v, &cv.value, c.operator))
                    {
                        return Some(idx);
                    }
                    for &pid in &c.ints {
                        if let Some(x) = self.read_value(s.target, &i.params, pid.cast_unsigned())
                            && case_matches(v, &x, c.operator)
                        {
                            return Some(idx);
                        }
                    }
                }
                (PortPayload::WhenTest(t), PortType::WHEN_TEST) if s.is_when => {
                    let c = if t.rhs_property != 0 {
                        self.read_value(s.target, &i.params, t.rhs_property.cast_unsigned())
                    } else {
                        t.constant.as_ref().map(|e| e.value.clone())
                    };
                    if let Some(c) = c
                        && when_holds(v, &c, t.operator)
                    {
                        return Some(idx);
                    }
                }
                _ => {}
            }
        }
        n.ports_of(PortType::DEFAULT).map(|(i, _)| i).next()
    }

    /// ForCurrentValue: restart the children of the first such port (§6.10).
    fn run_fcv(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some((unit, nref)) = self.switch_node(id) else {
            return;
        };
        let Some(port) = unit.node(nref.node).and_then(|n| {
            n.ports_of(PortType::FOR_CURRENT_VALUE)
                .map(|(i, _)| i)
                .next()
        }) else {
            return;
        };
        let old = self.switch_state(id).map_or(0, |s| s.fcv_tag);
        if old != 0 {
            self.stop_group(sink, id, old, REASON_STOP);
        }
        let Some(tag) = self.insts.get_mut(id).map(|i| i.open_group()) else {
            return;
        };
        if let Some(s) = self.switch_state_mut(id) {
            s.fcv_tag = tag;
        }
        self.run_port(sink, &unit, nref.unit, nref.node, port, id, tag);
    }

    /// Makes `port` the current case and starts its children.
    fn start_case(&mut self, sink: &mut dyn EffectSink, id: InstanceId, port: Option<usize>) {
        let tag = match port {
            Some(_) => match self.insts.get_mut(id) {
                Some(i) => i.open_group(),
                None => return,
            },
            None => 0,
        };
        if let Some(s) = self.switch_state_mut(id) {
            s.current = port;
            s.current_tag = tag;
            s.waiting = false;
        }
        if let (Some(p), Some((unit, nref))) = (port, self.switch_node(id)) {
            self.run_port(sink, &unit, nref.unit, nref.node, p, id, tag);
        }
    }

    /// Re-evaluation of a persistent Switch or When after a change (§6.6–§6.9).
    pub(crate) fn switch_reevaluate(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let v = self.switch_value(id);
        let Some(s) = self.switch_state_mut(id) else {
            return;
        };
        let old = s.value.take();
        let changed = !opt_equal(old.as_ref(), v.as_ref());
        s.value = v.clone();
        let transition_mode = s.transition_mode;
        let e = self.bump_eval(id);
        if changed {
            self.run_fcv(sink, id);
            if !self.alive(id) || !self.eval_is(id, e) {
                return;
            }
        }
        if !transition_mode {
            let port = self.select_port(id);
            if self.switch_state(id).is_some_and(|s| s.current == port) {
                return;
            }
            self.do_switch(sink, id, port);
            return;
        }
        // §6.7 steps 1–3 apply to a new value only. A re-evaluation for a changed case int
        // list (§6.3, §6.5) with the same value selects the case (step 4) without a
        // transition (Decision, `script-runtime.md` §29).
        if !changed {
            self.select_and_switch(sink, id);
            return;
        }
        if let Some(k) = self.switch_state_mut(id).and_then(|s| s.delay_event.take()) {
            self.sched.cancel(k);
        }
        if let Some(tp) = self.find_transition(id, old.as_ref(), v.as_ref()) {
            let delay = self.transition_of(id, tp).map_or(0.0, |t| t.2);
            if delay > 0.0 {
                let due = self.sched.now + f64::from(delay);
                let k = self.schedule(due, Event::SwitchDelay(id, SwitchDelay::Transition(tp)));
                if let Some(s) = self.switch_state_mut(id) {
                    s.delay_event = Some(k);
                }
                return;
            }
            if self.apply_transition(sink, id, tp) {
                return;
            }
        }
        self.select_and_switch(sink, id);
    }

    fn transition_of(&self, id: InstanceId, port: usize) -> Option<(bool, bool, f32, bool, f32)> {
        let (unit, nref) = self.switch_node(id)?;
        match &unit.node(nref.node)?.ports.get(port)?.payload {
            PortPayload::SwitchTransition(t) => Some(transition_fields(t)),
            _ => None,
        }
    }

    /// The transition port for (old → new), by the four tiers of §6.9.
    fn find_transition(
        &self,
        id: InstanceId,
        old: Option<&PropertyValue>,
        new: Option<&PropertyValue>,
    ) -> Option<usize> {
        let (unit, nref) = self.switch_node(id)?;
        let n = unit.node(nref.node)?;
        let ports: Vec<(usize, &SwitchTransition)> = n
            .ports_of(PortType::SWITCH_TRANSITION)
            .filter_map(|(i, p)| match &p.payload {
                PortPayload::SwitchTransition(t) => Some((i, &**t)),
                _ => None,
            })
            .collect();
        for tier in 1..=4 {
            for &(i, t) in &ports {
                let (fd, td, ..) = transition_fields(t);
                let hit = match tier {
                    1 => {
                        !fd && !td
                            && pv_equals(t.from.as_ref(), old)
                            && pv_equals(t.to.as_ref(), new)
                    }
                    2 => fd && !td && pv_equals(t.to.as_ref(), new),
                    3 => !fd && td && pv_equals(t.from.as_ref(), old),
                    _ => fd && td,
                };
                if hit {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Applies a transition port (§6.9); returns whether the evaluation ends here (the
    /// transition took over with Wait, or a nested evaluation overtook this one).
    fn apply_transition(&mut self, sink: &mut dyn EffectSink, id: InstanceId, port: usize) -> bool {
        let Some((unit, nref)) = self.switch_node(id) else {
            return true;
        };
        let Some((_, _, _, wait, ramp_in)) = self.transition_of(id, port) else {
            return false;
        };
        let e = self.bump_eval(id);
        let Some(tag) = self.insts.get_mut(id).map(|i| i.open_group()) else {
            return true;
        };
        self.run_port(sink, &unit, nref.unit, nref.node, port, id, tag);
        if !self.alive(id) {
            return true;
        }
        let running = !self.group_children(id, tag).is_empty();
        if wait && running && self.eval_is(id, e) {
            self.begin_outgoing(sink, id, ramp_in, true);
            if self.alive(id) && self.eval_is(id, e) {
                if let Some(s) = self.switch_state_mut(id) {
                    s.current = None;
                    s.current_tag = tag;
                    s.waiting = true;
                }
                return true;
            }
        }
        // Not taking over (or overtaken): the group takes no further children; children
        // still running finish on their own, untracked by the case logic.
        if let Some(i) = self.insts.get_mut(id) {
            i.close_group(tag);
        }
        !self.alive(id) || !self.eval_is(id, e)
    }

    /// §6.7 step 4.
    fn select_and_switch(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let port = self.select_port(id);
        let Some(s) = self.switch_state(id) else {
            return;
        };
        if port == s.current && !s.waiting {
            // Same case: a half-done cross-fade is dropped; the case runs at full intensity.
            if s.outgoing_tag != 0 {
                self.end_fade(sink, id, false);
            }
            return;
        }
        let delay = match (port, self.switch_node(id)) {
            (Some(p), Some((unit, nref))) => {
                unit.node(nref.node).map_or(0.0, |n| port_floats(n, p).1)
            }
            _ => 0.0,
        };
        if delay > 0.0 {
            if let Some(k) = self.switch_state_mut(id).and_then(|s| s.delay_event.take()) {
                self.sched.cancel(k);
            }
            let due = self.sched.now + f64::from(delay);
            let k = self.schedule(due, Event::SwitchDelay(id, SwitchDelay::Case));
            if let Some(s) = self.switch_state_mut(id) {
                s.delay_event = Some(k);
            }
        } else {
            self.do_switch(sink, id, port);
        }
    }

    pub(crate) fn switch_delay_event(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        d: SwitchDelay,
    ) {
        if let Some(s) = self.switch_state_mut(id) {
            s.delay_event = None;
        }
        match d {
            SwitchDelay::Transition(tp) => {
                if !self.apply_transition(sink, id, tp) {
                    self.select_and_switch(sink, id);
                }
            }
            SwitchDelay::Case => {
                let port = self.select_port(id);
                if self
                    .switch_state(id)
                    .is_some_and(|s| s.current != port || s.waiting)
                {
                    self.do_switch(sink, id, port);
                }
            }
        }
    }

    /// Switches from the current case to `port` (§6.6, §6.8).
    fn do_switch(&mut self, sink: &mut dyn EffectSink, id: InstanceId, port: Option<usize>) {
        let ramp = match (
            self.switch_state(id).and_then(|s| s.current),
            self.switch_node(id),
        ) {
            (Some(p), Some((unit, nref))) => {
                unit.node(nref.node).map_or(0.0, |n| port_floats(n, p).0)
            }
            _ => 0.0,
        };
        let e = self.bump_eval(id);
        self.begin_outgoing(sink, id, ramp, false);
        // Stopping the old case can change the property (an OnStop's SetProperty); the
        // nested re-evaluation then started the case for the newer value (§6.6: one branch).
        if self.alive(id) && self.eval_is(id, e) {
            self.start_case(sink, id, port);
        }
    }

    /// §6.8 steps 1–3: an earlier outgoing branch stops; the current branch becomes the
    /// outgoing branch with a window of `ramp` seconds, or stops now. With `defer` (a
    /// transition taking over with RampIn 0, §6.9) a branch without a window drops to
    /// intensity 0 now and stops at the next scheduler pass.
    fn begin_outgoing(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        ramp: f32,
        defer: bool,
    ) {
        let now = self.sched.now;
        let mut level = 1.0f64;
        if let Some(s) = self.switch_state(id)
            && s.outgoing_tag != 0
        {
            if let Some(f) = s.fade
                && f.dur > 0.0
            {
                level = 1.0 - ((f.end - now) / f.dur).clamp(0.0, 1.0);
            }
            let e = s.eval;
            self.end_fade(sink, id, true);
            if !self.alive(id) || !self.eval_is(id, e) {
                return; // A nested evaluation took over during that stop.
            }
        }
        // The current case stops being current here: a nested re-evaluation during the
        // stops below must not take it for the running case.
        let Some(old) = self.switch_state_mut(id).map(|s| {
            s.current = None;
            std::mem::take(&mut s.current_tag)
        }) else {
            return;
        };
        if old == 0 {
            return;
        }
        let running = !self.group_children(id, old).is_empty();
        if !running {
            if let Some(i) = self.insts.get_mut(id) {
                i.close_group(old);
            }
            return;
        }
        let ramp = f64::from(ramp);
        if ramp >= f64::from(INTENSITY_EPSILON) || defer {
            let (end, dur) = if ramp >= f64::from(INTENSITY_EPSILON) {
                (now + level * ramp, ramp)
            } else {
                for c in self.group_children(id, old) {
                    self.set_intensity(sink, c, 0.0);
                }
                (now, 0.0)
            };
            let k = self.schedule(end, Event::SwitchFadeEnd(id));
            if let Some(s) = self.switch_state_mut(id) {
                s.outgoing_tag = old;
                s.fade = Some(Fade { end, dur });
                s.fade_event = Some(k);
            }
            if dur > 0.0 && !self.frame_updates.contains(&id) {
                self.frame_updates.push(id);
            }
        } else {
            self.stop_group(sink, id, old, REASON_STOP);
        }
    }

    /// Ends a cross-fade: the outgoing branch stops (intensity 0 first when the window
    /// ran out) and the current branch returns to full intensity unless `keep_current`.
    fn end_fade(&mut self, sink: &mut dyn EffectSink, id: InstanceId, keep_current: bool) {
        let Some(s) = self.switch_state_mut(id) else {
            return;
        };
        let out = std::mem::take(&mut s.outgoing_tag);
        s.fade = None;
        let ev = s.fade_event.take();
        let current = s.current_tag;
        if let Some(k) = ev {
            self.sched.cancel(k);
        }
        self.frame_updates.retain(|&f| f != id);
        if out != 0 {
            self.stop_group(sink, id, out, REASON_STOP);
        }
        if !keep_current && current != 0 {
            self.restore_full(sink, id, current);
        }
    }

    /// Sets the children of group `tag` back to factor × the Switch's intensity.
    fn restore_full(&mut self, sink: &mut dyn EffectSink, id: InstanceId, tag: u32) {
        let Some(base) = self.insts.get(id).map(|i| i.intensity) else {
            return;
        };
        for c in self.group_children(id, tag) {
            if let Some((f, upi)) = self
                .insts
                .get(c)
                .map(|i| (i.factor, i.use_parent_intensity))
                && upi
            {
                self.set_intensity(sink, c, f * base);
            }
        }
    }

    /// The window end of a cross-fade (§6.8 step 6).
    pub(crate) fn switch_fade_end(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some(out) = self.switch_state_mut(id).map(|s| {
            s.fade_event = None;
            s.outgoing_tag
        }) else {
            return;
        };
        for c in self.group_children(id, out) {
            self.set_intensity(sink, c, 0.0);
        }
        self.end_fade(sink, id, false);
    }

    /// One frame of a cross-fade (§6.8 step 5).
    pub(crate) fn switch_frame(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some((out, current, fade, base)) = self.insts.get(id).and_then(|i| match &i.kind {
            KindState::Switch(s) => Some((s.outgoing_tag, s.current_tag, s.fade?, i.intensity)),
            _ => None,
        }) else {
            self.frame_updates.retain(|&f| f != id);
            return;
        };
        if fade.dur <= 0.0 {
            return;
        }
        #[allow(clippy::cast_possible_truncation)]
        let f = ((fade.end - self.sched.now) / fade.dur).clamp(0.0, 1.0) as f32;
        for (tag, w) in [(out, f), (current, 1.0 - f)] {
            if tag == 0 {
                continue;
            }
            for c in self.group_children(id, tag) {
                if let Some((factor, upi)) = self
                    .insts
                    .get(c)
                    .map(|i| (i.factor, i.use_parent_intensity))
                    && upi
                {
                    self.set_intensity(sink, c, factor * w * base);
                }
            }
        }
    }

    /// The last child of a transition that took over finished (§6.9): the outgoing branch
    /// stops and the case for the current value starts.
    pub(crate) fn switch_transition_done(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some(tag) = self.switch_state_mut(id).map(|s| {
            s.waiting = false;
            std::mem::take(&mut s.current_tag)
        }) else {
            return;
        };
        if let Some(i) = self.insts.get_mut(id) {
            i.close_group(tag);
        }
        let e = self.bump_eval(id);
        self.end_fade(sink, id, true);
        if !self.alive(id) || !self.eval_is(id, e) {
            return;
        }
        let port = self.select_port(id);
        self.start_case(sink, id, port);
    }
}
