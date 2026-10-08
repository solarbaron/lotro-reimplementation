// SPDX-License-Identifier: GPL-3.0-or-later
//! Sequence (6) and Timer (65): `script-runtime.md` §10 and §11.

use std::sync::Arc;

use tessera_formats::script_table::{NodeData, TimerNode};

use super::instance::{SequenceState, TimerState};
use super::kinds::silent;
use super::{
    Event, INTENSITY_EPSILON, InstanceId, KindState, LAG_CATCH_UP, MIN_LOOP_INTERVAL, Runtime,
};
use crate::graph::{NodeRef, Unit, port_time};
use crate::sink::EffectSink;
use crate::values::{as_bool, as_f32, as_i32};

impl Runtime<'_> {
    // ----- Sequence (§10) -----

    pub(crate) fn sequence_start(
        &mut self,
        sink: &mut dyn EffectSink,
        id: InstanceId,
        unit: &Arc<Unit>,
        nref: NodeRef,
    ) -> bool {
        let Some(n) = unit.node(nref.node) else {
            return false;
        };
        let NodeData::Sequence {
            loop_forever,
            loop_count,
        } = n.data
        else {
            return false;
        };
        let steps = u32::try_from(n.ordered.len()).unwrap_or(u32::MAX);
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Sequence(Box::new(SequenceState {
                forever: loop_forever,
                limit: steps.wrapping_mul(loop_count.cast_unsigned()),
                started: 0,
                step_tag: 0,
                last_pass: None,
                pending: None,
            }));
        }
        self.sequence_run(sink, id);
        true
    }

    /// Starts steps until one waits, the pass pacing defers, or the limit is reached
    /// (§10.2–§10.4).
    pub(crate) fn sequence_run(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some(nref) = self.insts.get(id).and_then(|i| i.node) else {
            return;
        };
        let Some(unit) = self.units.get(nref.unit as usize).cloned() else {
            return;
        };
        let Some(n) = unit.node(nref.node) else {
            return;
        };
        let steps = &n.ordered;
        loop {
            let Some(i) = self.insts.get_mut(id) else {
                return;
            };
            if i.stopped {
                return;
            }
            let KindState::Sequence(s) = &mut i.kind else {
                return;
            };
            if s.pending.is_some() {
                return;
            }
            if steps.is_empty() {
                if !s.forever {
                    i.armed = true;
                }
                return;
            }
            if !s.forever && s.started >= u64::from(s.limit) {
                i.armed = true;
                return;
            }
            let len = steps.len() as u64;
            let idx = usize::try_from(s.started % len).unwrap_or(0);
            if idx == 0 {
                let now = self.sched.now;
                if let Some(lp) = s.last_pass
                    && now < lp + MIN_LOOP_INTERVAL
                {
                    // The same expression as the resume time, so a resume is never early.
                    let mut due = lp + MIN_LOOP_INTERVAL;
                    if self.sched.frame_time - due > LAG_CATCH_UP {
                        due = self.sched.frame_time;
                    }
                    let k = self.sched.schedule(due, Event::SequenceResume(id));
                    if let Some(i) = self.insts.get_mut(id)
                        && let KindState::Sequence(s) = &mut i.kind
                    {
                        s.pending = Some(k);
                    }
                    return;
                }
                if s.last_pass.is_some_and(|lp| now <= lp) {
                    // Only when the clock is so large that 0.1 s no longer changes it: the
                    // pacing cannot hold the loop, so the Sequence completes (guard).
                    i.armed = true;
                    self.limit("max_same_time_cycles");
                    return;
                }
                s.last_pass = Some(now);
            }
            s.started += 1;
            let tag = i.open_group();
            if let KindState::Sequence(s) = &mut i.kind {
                s.step_tag = tag;
            }
            let Some(&port) = steps.get(idx) else {
                return;
            };
            self.run_port(sink, &unit, nref.unit, nref.node, port as usize, id, tag);
            match self.insts.get_mut(id) {
                Some(i) if !i.stopped => {
                    if i.group_len(tag) > 0 {
                        return;
                    }
                    i.close_group(tag);
                }
                _ => return,
            }
        }
    }

    // ----- Timer (§11) -----

    /// The Timer's fields with this start's driven values applied.
    fn timer_fields(&self, id: InstanceId) -> Option<TimerNode> {
        let i = self.insts.get(id)?;
        let nref = i.node?;
        let NodeData::Timer(t) = &self.units.get(nref.unit as usize)?.node(nref.node)?.data else {
            return None;
        };
        let mut t = *t;
        let f = &self.vocab.timer;
        if let Some(v) = i.override_of(f.loop_forever).and_then(as_bool) {
            t.loop_forever = v;
        }
        if let Some(v) = i.override_of(f.duration).and_then(as_f32) {
            t.duration = v;
        }
        if let Some(v) = i.override_of(f.loop_count).and_then(as_i32) {
            t.loop_count = v;
        }
        if let Some(v) = i.override_of(f.speed).and_then(as_f32) {
            t.speed = v;
        }
        if let Some(v) = i.override_of(f.speed_multiplier).and_then(as_f32) {
            t.speed_multiplier = v;
        }
        Some(t)
    }

    /// (port index, time) of the Timer's ports in time order.
    fn timer_ports(&self, id: InstanceId) -> Vec<(u32, f32)> {
        let Some(nref) = self.insts.get(id).and_then(|i| i.node) else {
            return Vec::new();
        };
        let Some(n) = self
            .units
            .get(nref.unit as usize)
            .and_then(|u| u.node(nref.node))
        else {
            return Vec::new();
        };
        n.ordered
            .iter()
            .filter_map(|&p| n.ports.get(p as usize).map(|port| (p, port_time(port))))
            .collect()
    }

    /// Speed *s* and cycle length *C* (§11.2).
    fn timer_cycle(t: &TimerNode, ports: &[(u32, f32)]) -> (f64, f64) {
        let s = f64::from(t.speed) * f64::from(t.speed_multiplier);
        let latest = ports.iter().map(|p| p.1).fold(0.0f32, f32::max);
        let dur = if t.duration.is_finite() {
            t.duration
        } else {
            0.0
        };
        let mut c = f64::from(dur.max(latest)) / s;
        if !c.is_finite() {
            c = 0.0;
        }
        c = c.max(0.0);
        if t.loop_forever {
            c = c.max(MIN_LOOP_INTERVAL);
        }
        (s, c)
    }

    pub(crate) fn timer_start(
        &mut self,
        id: InstanceId,
        _unit: &Arc<Unit>,
        _nref: NodeRef,
    ) -> bool {
        let now = self.sched.now;
        if let Some(i) = self.insts.get_mut(id) {
            i.kind = KindState::Timer(Box::new(TimerState {
                t0: now,
                count: 0,
                next: 0,
                pending: None,
                same_time_cycles: 0,
                advance: 0,
                advance_cycles: 0,
                not_before: f64::NEG_INFINITY,
            }));
        }
        self.timer_begin_cycle(id);
        true
    }

    /// Starts a cycle at the state's `t0` (§11.2–§11.3).
    fn timer_begin_cycle(&mut self, id: InstanceId) {
        let Some(t) = self.timer_fields(id) else {
            return;
        };
        let ports = self.timer_ports(id);
        let (s, _) = Self::timer_cycle(&t, &ports);
        let intensity = self.insts.get(id).map_or(0.0, |i| i.intensity);
        let fast = s >= f64::from(INTENSITY_EPSILON);
        if !fast {
            return; // Too slow: nothing is scheduled; the Timer stays running.
        }
        let ports_on = !silent(intensity) && !ports.is_empty();
        if t.loop_forever && !ports_on {
            return; // A looping Timer without audible ports schedules nothing.
        }
        if let Some(i) = self.insts.get_mut(id)
            && let KindState::Timer(st) = &mut i.kind
        {
            st.next = if ports_on { 0 } else { ports.len() };
        }
        self.timer_schedule_next(id, &t, &ports);
    }

    /// Schedules the pending port or the cycle end.
    fn timer_schedule_next(&mut self, id: InstanceId, t: &TimerNode, ports: &[(u32, f32)]) {
        let (s, c) = Self::timer_cycle(t, ports);
        let Some(i) = self.insts.get_mut(id) else {
            return;
        };
        let KindState::Timer(st) = &mut i.kind else {
            return;
        };
        let due = match ports.get(st.next) {
            Some(&(_, time)) => st.t0 + (f64::from(time) / s).max(0.0),
            None => st.t0 + c,
        };
        let due = due.max(st.not_before);
        if let Some(k) = st.pending.take() {
            self.sched.cancel(k);
        }
        let k = self.sched.schedule(due, Event::Timer(id));
        if let Some(i) = self.insts.get_mut(id)
            && let KindState::Timer(st) = &mut i.kind
        {
            st.pending = Some(k);
        }
    }

    /// A Timer event: a port fires or the cycle ends (§11.2–§11.4).
    pub(crate) fn timer_event(&mut self, sink: &mut dyn EffectSink, id: InstanceId) {
        let Some(t) = self.timer_fields(id) else {
            return;
        };
        let ports = self.timer_ports(id);
        let Some((next, intensity)) = self.insts.get_mut(id).and_then(|i| {
            let intensity = i.intensity;
            match &mut i.kind {
                KindState::Timer(st) => {
                    st.pending = None;
                    Some((st.next, intensity))
                }
                _ => None,
            }
        }) else {
            return;
        };
        if let Some(&(port, _)) = ports.get(next) {
            if let Some(i) = self.insts.get_mut(id)
                && let KindState::Timer(st) = &mut i.kind
            {
                st.next += 1;
            }
            if !silent(intensity)
                && let Some(nref) = self.insts.get(id).and_then(|i| i.node)
                && let Some(unit) = self.units.get(nref.unit as usize).cloned()
            {
                self.run_port(sink, &unit, nref.unit, nref.node, port as usize, id, 0);
            }
            if self.alive(id) {
                self.timer_schedule_next(id, &t, &ports);
            }
            return;
        }
        // Cycle end.
        let (_, c) = Self::timer_cycle(&t, &ports);
        let frame = self.sched.frame_time;
        let advance = self.advances;
        let max_same = self.limits.max_same_time_cycles;
        let max_per_advance = self.limits.max_timer_cycles_per_advance;
        let mut arm = false;
        let mut runaway = false;
        let mut deferred = false;
        if let Some(i) = self.insts.get_mut(id)
            && let KindState::Timer(st) = &mut i.kind
        {
            st.count = st.count.saturating_add(1);
            let loops = t.loop_count.cast_unsigned();
            if !t.loop_forever && loops <= st.count {
                arm = true;
            } else {
                let prev = st.t0;
                st.t0 += c;
                if frame - st.t0 > LAG_CATCH_UP && c > 0.0 {
                    let k = ((frame - st.t0) / c).floor();
                    st.t0 += k * c;
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let k = k.min(f64::from(u32::MAX)) as u32;
                    st.count = st.count.saturating_add(k);
                    if !t.loop_forever && loops <= st.count {
                        arm = true;
                    }
                }
                // A cycle that does not move the start forward (length 0, or too short to
                // change the scheduler time) counts toward the zero-length guard.
                if st.t0 <= prev {
                    st.same_time_cycles = st.same_time_cycles.saturating_add(1);
                    if st.same_time_cycles > max_same {
                        runaway = true;
                        arm = true;
                    }
                } else {
                    st.same_time_cycles = 0;
                }
                // At most so many cycles per advance; the next waits for the next advance.
                if st.advance == advance {
                    st.advance_cycles = st.advance_cycles.saturating_add(1);
                } else {
                    st.advance = advance;
                    st.advance_cycles = 1;
                }
                if !arm && st.advance_cycles >= max_per_advance {
                    st.not_before = frame.next_up();
                    deferred = true;
                }
            }
        }
        if runaway {
            self.limit("max_same_time_cycles");
        }
        if deferred {
            self.limit("max_timer_cycles_per_advance");
        }
        if arm {
            self.arm(id);
            self.check_complete(sink, id, 0);
        } else {
            self.timer_begin_cycle(id);
        }
    }

    /// A driven Timer field changed: the pending event is rescheduled from the current
    /// cycle's start with the new speed (§11.4).
    pub(crate) fn timer_reschedule(&mut self, id: InstanceId) {
        let Some(t) = self.timer_fields(id) else {
            return;
        };
        let ports = self.timer_ports(id);
        let pending = self.insts.get(id).is_some_and(|i| match &i.kind {
            KindState::Timer(st) => st.pending.is_some(),
            _ => false,
        });
        let (s, _) = Self::timer_cycle(&t, &ports);
        if !pending {
            return;
        }
        let fast = s >= f64::from(INTENSITY_EPSILON);
        if !fast {
            if let Some(i) = self.insts.get_mut(id)
                && let KindState::Timer(st) = &mut i.kind
                && let Some(k) = st.pending.take()
            {
                self.sched.cancel(k);
            }
            return;
        }
        self.timer_schedule_next(id, &t, &ports);
    }
}
