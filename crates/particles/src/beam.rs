// SPDX-License-Identifier: GPL-3.0-or-later
//! Beams (`particle-runtime.md` §11.1, rule 14): the envelope, the two ends with their random
//! offsets and detach times, the head and tail fractions, the head's arrival (which fires the
//! Beam node's port), the ribbon's segment layout and the spawn schedule of the beam and head
//! particle systems.
//!
//! [`Beam`] is plain state over one Beam node's fields ([`BeamNode`]). The caller resolves the
//! attachment points of the source and target entities each frame (as `particle-runtime.md`
//! §6.1 attaches an effect) and hands them to [`Beam::update`]; the update returns what to do
//! this frame: report the arrival, draw the ribbon, spawn particles. Simulating and drawing the
//! two particle systems is [`crate::SystemSim`]'s work ([`crate::SystemSim::spawn_at`]).
//!
//! Times are seconds of the global clock (§3.1), lengths metres. Everything is bounded: at most
//! [`MAX_BEAM_SPAWNS`] beam particles per update and [`MAX_RIBBON_SEGMENTS`] ribbon segments
//! (rule 14); non-finite inputs never produce non-finite outputs.

use glam::Vec3;
use tessera_formats::script_table::BeamNode;

use crate::rng::Ran2;

/// The threshold below which the beam treats a time, a radius or a length as zero (§11.1).
pub const BEAM_EPSILON: f32 = 0.0002;
/// A beam longer than this is neither moved nor drawn that frame (§11.1 step 4).
pub const MAX_BEAM_LENGTH: f32 = 1000.0;
/// The length used for the fractions of a beam whose ends meet (§11.1 step 4).
pub const DEGENERATE_LENGTH: f32 = 0.01;
/// The shortest drawn segment, and the least length for particles and the head entity
/// (§11.1 steps 6, 8, 9).
pub const MIN_DRAWN_LENGTH: f32 = 0.1;
/// The head fraction past which the head has arrived (§11.1 step 5).
pub const ARRIVAL_FRACTION: f32 = 0.9998;
/// Squared distance from the head to the target below which no head particle spawns
/// (§11.1 step 8).
pub const HEAD_SPAWN_MIN_DISTANCE_SQ: f32 = 0.1;
/// A spawn interval longer than this is cut (§11.1 step 8)…
pub const SPAWN_CATCH_UP_LIMIT: f64 = 1.0;
/// …to this.
pub const SPAWN_CATCH_UP: f64 = 0.1;
/// Most beam particles spawned per beam and update (rule 14).
pub const MAX_BEAM_SPAWNS: usize = 4096;
/// Most ribbon segments per beam; further body segments are merged into the last (rule 14).
pub const MAX_RIBBON_SEGMENTS: usize = 1024;

fn finite(x: f32) -> f32 {
    if x.is_finite() { x } else { 0.0 }
}

/// The envelope's phase (§11.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopePhase {
    /// Before the first update.
    Start,
    /// Ramping in.
    RampIn,
    /// Sustaining.
    Sustain,
    /// Ramping out.
    RampOut,
    /// Done (intensity 0).
    Done,
}

/// The intensity envelope of a beam (§11.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    /// The phase.
    pub phase: EnvelopePhase,
    /// Seconds since the first update.
    pub elapsed: f32,
    /// Seconds spent ramping out (preset by a stop during the ramp-in).
    pub ramp_out_clock: f32,
    /// The intensity *e*, `0 ..= 1`.
    pub intensity: f32,
    /// The beam was stopped.
    pub stopped: bool,
    ramp_in: f32,
    sustain: f32,
    ramp_out: f32,
    persistent: bool,
}

impl Envelope {
    /// The envelope of a node.
    #[must_use]
    pub fn new(node: &BeamNode) -> Self {
        Self {
            phase: EnvelopePhase::Start,
            elapsed: 0.0,
            ramp_out_clock: 0.0,
            intensity: 0.0,
            stopped: false,
            ramp_in: finite(node.ramp_in),
            sustain: finite(node.sustain),
            ramp_out: finite(node.ramp_out),
            persistent: node.persistent,
        }
    }

    fn begin_ramp_out(&mut self, preset: f32) {
        if self.ramp_out <= BEAM_EPSILON {
            self.phase = EnvelopePhase::Done;
            self.intensity = 0.0;
        } else {
            self.phase = EnvelopePhase::RampOut;
            self.ramp_out_clock = preset.max(0.0);
        }
    }

    /// Stops the beam: the next update ramps out from the current level.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// One update over `dt` seconds (§11.1 step 2).
    pub fn update(&mut self, dt: f32) {
        let dt = finite(dt).max(0.0);
        if self.phase == EnvelopePhase::Start {
            self.phase = if self.stopped {
                EnvelopePhase::Done
            } else if self.ramp_in > BEAM_EPSILON {
                EnvelopePhase::RampIn
            } else {
                EnvelopePhase::Sustain
            };
        }
        match self.phase {
            EnvelopePhase::RampIn => {
                let e = (self.elapsed / self.ramp_in).clamp(0.0, 1.0);
                self.intensity = e;
                if self.stopped {
                    self.begin_ramp_out((1.0 - e) * self.ramp_out);
                } else if e >= 1.0 {
                    self.phase = EnvelopePhase::Sustain;
                    self.intensity = 1.0;
                }
            }
            EnvelopePhase::Sustain => {
                self.intensity = 1.0;
                if self.stopped || (!self.persistent && self.elapsed > self.ramp_in + self.sustain)
                {
                    self.begin_ramp_out(0.0);
                }
            }
            EnvelopePhase::Start | EnvelopePhase::RampOut | EnvelopePhase::Done => {}
        }
        if self.phase == EnvelopePhase::RampOut {
            if self.ramp_out_clock > self.ramp_out {
                self.phase = EnvelopePhase::Done;
                self.intensity = 0.0;
            } else {
                self.intensity = (1.0 - self.ramp_out_clock / self.ramp_out).clamp(0.0, 1.0);
            }
        }
        if self.phase == EnvelopePhase::Done {
            self.intensity = 0.0;
        }
        self.elapsed += dt;
        if self.phase == EnvelopePhase::RampOut {
            self.ramp_out_clock += dt;
        }
    }
}

/// Which part of the ribbon a segment belongs to (§11.1 step 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonPart {
    /// `TailMaterial`.
    Tail,
    /// `BodyMaterial`.
    Body,
    /// `HeadMaterial`.
    Head,
}

/// One ribbon segment: its part, its start along the drawn segment (from the tail end) and
/// its length, metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RibbonSegment {
    /// Material part.
    pub part: RibbonPart,
    /// Distance of its start from the tail point.
    pub start: f32,
    /// Its length.
    pub length: f32,
}

/// The ribbon's segments over a drawn length `len` (§11.1 step 6): tail segments, body
/// segments, head segments, each `min(SegmentSize, len)` long; when the head and tail
/// segments together are longer than `len` they shrink to `len / n` each and there is no
/// body. Parts with a count of 0 are left out (a missing material is the caller's test).
/// Empty when `len` is below [`MIN_DRAWN_LENGTH`] or not finite.
#[must_use]
pub fn ribbon_segments(node: &BeamNode, len: f32) -> Vec<RibbonSegment> {
    let mut out = Vec::new();
    if !(len >= MIN_DRAWN_LENGTH) || !len.is_finite() {
        return out;
    }
    let tails = usize::try_from(node.tail_segments.max(0)).unwrap_or(0);
    let heads = usize::try_from(node.head_segments.max(0)).unwrap_or(0);
    // Head and tail segments leave room for at least one body segment (rule 14).
    let tails = tails.min((MAX_RIBBON_SEGMENTS - 1) / 2);
    let heads = heads.min((MAX_RIBBON_SEGMENTS - 1) / 2);
    let size = finite(node.segment_size);
    let g = if size > BEAM_EPSILON {
        size.min(len)
    } else {
        len
    };
    let n = tails + heads;
    #[allow(clippy::cast_precision_loss)]
    let (g, body) = if n > 0 && n as f32 * g > len {
        (len / n as f32, 0.0)
    } else {
        #[allow(clippy::cast_precision_loss)]
        let rest = len - n as f32 * g;
        (g, rest)
    };
    let mut at = 0.0f32;
    for _ in 0..tails {
        out.push(RibbonSegment {
            part: RibbonPart::Tail,
            start: at,
            length: g,
        });
        at += g;
    }
    if body > BEAM_EPSILON {
        let room = MAX_RIBBON_SEGMENTS.saturating_sub(n).max(1);
        let mut left = body;
        let mut count = 0usize;
        while left > BEAM_EPSILON {
            count += 1;
            // The last allowed segment takes the rest (rule 14).
            let piece = if count >= room || g <= BEAM_EPSILON {
                left
            } else {
                g.min(left)
            };
            out.push(RibbonSegment {
                part: RibbonPart::Body,
                start: at,
                length: piece,
            });
            at += piece;
            left -= piece;
        }
    }
    for _ in 0..heads {
        out.push(RibbonSegment {
            part: RibbonPart::Head,
            start: at,
            length: g,
        });
        at += g;
    }
    out
}

/// The ends of a beam for one frame, as the caller resolved them (§11.1 step 3), without the
/// random offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamEnds {
    /// The source entity's attachment point (`None` when the source is gone).
    pub source: Option<Vec3>,
    /// The source frame's forward axis (for an untargeted beam).
    pub source_forward: Vec3,
    /// The target entity's attachment point, or `TargetPosition` with
    /// `UseTargetPosition` = 1 (`None` for an untargeted beam or a target that is gone).
    pub target: Option<Vec3>,
}

/// The drawn part of a beam this frame (§11.1 steps 6, 8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamSpan {
    /// The tail point.
    pub tail: Vec3,
    /// The head point.
    pub head: Vec3,
    /// The source point.
    pub source: Vec3,
    /// The target point.
    pub target: Vec3,
    /// The envelope intensity *e*.
    pub intensity: f32,
}

impl BeamSpan {
    /// The drawn length ℓ (tail to head).
    #[must_use]
    pub fn length(&self) -> f32 {
        let l = (self.head - self.tail).length();
        if l.is_finite() { l } else { 0.0 }
    }
}

/// What one update asks for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BeamUpdate {
    /// The head arrived in this update: fire the node's Default ports (§11.1 step 7).
    pub arrived_now: bool,
    /// The span this frame (`None` before the start, without ends, or beyond
    /// [`MAX_BEAM_LENGTH`]).
    pub span: Option<BeamSpan>,
    /// The ribbon is drawn this frame (tail before head and ℓ ≥ [`MIN_DRAWN_LENGTH`]).
    pub ribbon: bool,
    /// Beam-system particles: (spawn time, world point), oldest first.
    pub beam_spawns: Vec<(f64, Vec3)>,
    /// A head-system particle: (spawn time, world point).
    pub head_spawn: Option<(f64, Vec3)>,
    /// The head entity is shown (§11.1 step 9).
    pub head_entity_shown: bool,
}

/// One beam (§11.1).
#[derive(Debug, Clone)]
pub struct Beam {
    node: BeamNode,
    created: f64,
    /// The envelope.
    pub envelope: Envelope,
    start_offset: Vec3,
    impact_offset: Vec3,
    source_point: Option<Vec3>,
    target_point: Option<Vec3>,
    source_detached: bool,
    target_detached: bool,
    arrived: bool,
    fired: bool,
    last_beam_spawn: f64,
    last_head_spawn: f64,
    stopped_at: Option<f64>,
}

/// A random offset of length up to `radius` (§11.1 step 3); zero when `radius` ≤ ε.
fn random_offset(radius: f32, rng: &mut Ran2) -> Vec3 {
    let r = finite(radius);
    if r <= BEAM_EPSILON {
        return Vec3::ZERO;
    }
    let d = Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
    );
    let d = if d.length() > 0.0 && d.is_finite() {
        d.normalize()
    } else {
        Vec3::ZERO
    };
    d * rng.range(0.0, r)
}

impl Beam {
    /// A beam of `node` created at `now` (§11.1 step 1; the random offsets of step 3 are drawn
    /// here from `rng`).
    #[must_use]
    pub fn new(node: &BeamNode, now: f64, rng: &mut Ran2) -> Self {
        let now = if now.is_finite() { now } else { 0.0 };
        let start_offset = random_offset(node.random_start_radius, rng);
        let impact_offset = random_offset(node.random_impact_radius, rng);
        Self {
            node: node.clone(),
            created: now,
            envelope: Envelope::new(node),
            start_offset,
            impact_offset,
            source_point: None,
            target_point: None,
            source_detached: false,
            target_detached: false,
            arrived: false,
            fired: false,
            last_beam_spawn: now,
            last_head_spawn: now,
            stopped_at: None,
        }
    }

    /// The node's fields.
    #[must_use]
    pub fn node(&self) -> &BeamNode {
        &self.node
    }

    /// Whether the head has arrived.
    #[must_use]
    pub fn arrived(&self) -> bool {
        self.arrived
    }

    /// Stops the beam at `now` (its node stopped, §11.1 step 2).
    pub fn stop(&mut self, now: f64) {
        if self.stopped_at.is_none() {
            self.stopped_at = Some(if now.is_finite() { now } else { self.created });
        }
        self.envelope.stop();
    }

    /// Whether the envelope has finished at `now` (§11.1 step 2): done, or a non-persistent
    /// beam past ramp-in + sustain + ramp-out since its start, or a stopped persistent beam
    /// past ramp-out since its stop.
    #[must_use]
    pub fn finished(&self, now: f64) -> bool {
        let e = &self.envelope;
        if e.phase == EnvelopePhase::Done {
            return true;
        }
        let ro = f64::from(e.ramp_out.max(0.0));
        if !e.persistent {
            let total = f64::from(e.ramp_in.max(0.0) + e.sustain.max(0.0)) + ro;
            return now - self.created > total;
        }
        self.stopped_at.is_some_and(|s| now - s > ro)
    }

    /// The fraction of `start` + `speed` × (t − `start_time`) / `len`, clamped (§11.1 step 5).
    fn fraction(start: f32, start_time: f32, speed: f32, t: f32, len: f32) -> f32 {
        let mut h = finite(start);
        if t > finite(start_time) {
            h = finite(start) + finite(speed) * (t - finite(start_time)) / len;
        }
        if h.is_finite() {
            h.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// One update at `now`, `dt` seconds after the last (§11.1 steps 2–9).
    /// `beam_system` / `head_system`: whether the beam's systems exist (loaded) with a rate.
    pub fn update(
        &mut self,
        now: f64,
        dt: f32,
        ends: BeamEnds,
        beam_system: bool,
        head_system: bool,
        rng: &mut Ran2,
    ) -> BeamUpdate {
        let mut out = BeamUpdate::default();
        if !now.is_finite() {
            return out;
        }
        self.envelope.update(dt);
        #[allow(clippy::cast_possible_truncation)]
        let t = (now - self.created) as f32;
        if t < 0.0 {
            return out;
        }
        let n = &self.node;
        // Step 3: detach times.
        let ramping_out = matches!(
            self.envelope.phase,
            EnvelopePhase::RampOut | EnvelopePhase::Done
        );
        if (finite(n.source_detach_time) > BEAM_EPSILON && t >= n.source_detach_time)
            || (n.detach_before_ramping_out && ramping_out)
        {
            self.source_detached |= self.source_point.is_some();
        }
        if (finite(n.target_detach_time) > BEAM_EPSILON && t >= n.target_detach_time)
            || (n.detach_before_ramping_out && ramping_out)
        {
            self.target_detached |= self.target_point.is_some();
        }
        if !self.source_detached
            && let Some(s) = ends.source.filter(|v| v.is_finite())
        {
            self.source_point = Some(s + self.start_offset);
        }
        let Some(source) = self.source_point else {
            return out;
        };
        if !self.target_detached {
            let target = match ends.target.filter(|v| v.is_finite()) {
                Some(p) => p + self.impact_offset,
                None => {
                    let f = if ends.source_forward.is_finite()
                        && ends.source_forward.length() > BEAM_EPSILON
                    {
                        ends.source_forward.normalize()
                    } else {
                        Vec3::Y
                    };
                    source + f * finite(n.untargeted_length)
                }
            };
            self.target_point = Some(target);
        }
        let Some(target) = self.target_point else {
            return out;
        };
        // Step 4.
        let full = (target - source).length();
        if !full.is_finite() || full > MAX_BEAM_LENGTH {
            return out;
        }
        let len = if full <= BEAM_EPSILON {
            DEGENERATE_LENGTH
        } else {
            full
        };
        // Step 5.
        let h = Self::fraction(n.head_start, n.head_start_time, n.head_speed, t, len);
        let tau = Self::fraction(n.tail_start, n.tail_start_time, n.tail_speed, t, len);
        let head = source + (target - source) * h;
        let tail = source + (target - source) * tau;
        // Step 7.
        if finite(n.head_speed) > BEAM_EPSILON && h > ARRIVAL_FRACTION {
            self.arrived = true;
        }
        if self.arrived && !self.fired {
            self.fired = true;
            out.arrived_now = true;
        }
        let span = BeamSpan {
            tail,
            head,
            source,
            target,
            intensity: self.envelope.intensity,
        };
        let ell = span.length();
        out.span = Some(span);
        // Step 6.
        out.ribbon = tau <= h && ell >= MIN_DRAWN_LENGTH;
        // Steps 8 and 9.
        let active = ell > MIN_DRAWN_LENGTH
            && !self.envelope.stopped
            && self.envelope.intensity > BEAM_EPSILON;
        out.head_entity_shown = active && !n.head_entity.is_null();
        if !active {
            return out;
        }
        let rate = n.beam_particles_per_second;
        if beam_system && rate > 0 {
            let mut delta = now - self.last_beam_spawn;
            if delta > SPAWN_CATCH_UP_LIMIT {
                self.last_beam_spawn = now - SPAWN_CATCH_UP;
                delta = SPAWN_CATCH_UP;
            }
            let size = finite(n.segment_size);
            let count = if size > BEAM_EPSILON && delta > 0.0 {
                (f64::from(ell / size) * f64::from(rate) * delta).floor()
            } else {
                0.0
            };
            if count >= 1.0 && count.is_finite() {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let k = (count as usize).min(MAX_BEAM_SPAWNS);
                #[allow(clippy::cast_precision_loss)]
                let step = delta / k as f64;
                let mut time = self.last_beam_spawn;
                for _ in 0..k {
                    time += step;
                    let u = rng.range(0.0, 1.0);
                    out.beam_spawns.push((time, tail + (head - tail) * u));
                }
                self.last_beam_spawn = time;
            }
        }
        let head_rate = n.head_particles_per_second;
        if head_system
            && beam_system
            && head_rate > 0
            && (head - target).length_squared() > HEAD_SPAWN_MIN_DISTANCE_SQ
        {
            let mut delta = now - self.last_head_spawn;
            if delta > SPAWN_CATCH_UP_LIMIT {
                self.last_head_spawn = now - SPAWN_CATCH_UP;
                delta = SPAWN_CATCH_UP;
            }
            if (f64::from(head_rate) * delta).floor() >= 1.0 {
                out.head_spawn = Some((now, head));
                self.last_head_spawn = now;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> BeamNode {
        BeamNode {
            ramp_in: 0.0,
            sustain: 1.0,
            ramp_out: 0.25,
            head_speed: 40.0,
            segment_size: 1.0,
            beam_particles_per_second: 10,
            head_particles_per_second: 10,
            ..BeamNode::default()
        }
    }

    fn ends(target: Vec3) -> BeamEnds {
        BeamEnds {
            source: Some(Vec3::ZERO),
            source_forward: Vec3::Y,
            target: Some(target),
        }
    }

    /// `combat-visuals.md` §13.2: 12 m at 40 m/s arrives about 0.300 s after the start.
    #[test]
    fn the_head_arrives_once_at_distance_over_speed() {
        let mut rng = Ran2::new(1);
        let mut b = Beam::new(&node(), 10.0, &mut rng);
        let target = Vec3::new(0.0, 12.0, 0.0);
        let mut fired = Vec::new();
        for f in 0..=60u32 {
            let now = 10.0 + f64::from(f) / 120.0;
            let u = b.update(now, 1.0 / 120.0, ends(target), false, false, &mut rng);
            if u.arrived_now {
                fired.push(now);
            }
        }
        assert_eq!(fired.len(), 1, "{fired:?}");
        let t = fired[0] - 10.0;
        assert!((0.29..=0.31).contains(&t), "arrived at {t}");
    }

    #[test]
    fn a_beam_without_head_speed_never_arrives() {
        let mut rng = Ran2::new(1);
        let n = BeamNode {
            head_speed: 0.0,
            head_start: 1.0,
            ..node()
        };
        let mut b = Beam::new(&n, 0.0, &mut rng);
        for f in 0..200u32 {
            let u = b.update(
                f64::from(f) / 60.0,
                1.0 / 60.0,
                ends(Vec3::X * 5.0),
                false,
                false,
                &mut rng,
            );
            assert!(!u.arrived_now);
        }
    }

    #[test]
    fn the_envelope_ramps_and_finishes() {
        let n = BeamNode {
            ramp_in: 0.5,
            sustain: 1.0,
            ramp_out: 0.5,
            ..node()
        };
        let mut e = Envelope::new(&n);
        e.update(0.25);
        assert_eq!(e.phase, EnvelopePhase::RampIn);
        assert_eq!(e.intensity, 0.0);
        // A stop during the ramp-in continues from the current level (0.5 here).
        e.stop();
        e.update(0.25);
        assert_eq!(e.phase, EnvelopePhase::RampOut);
        assert!((e.intensity - 0.5).abs() < 1e-6, "{}", e.intensity);
        for _ in 0..10 {
            e.update(0.1);
        }
        assert_eq!(e.phase, EnvelopePhase::Done);
        assert_eq!(e.intensity, 0.0);
        // A non-persistent beam finishes after ramp-in + sustain + ramp-out.
        let mut rng = Ran2::new(3);
        let b = Beam::new(&n, 0.0, &mut rng);
        assert!(!b.finished(1.9));
        assert!(b.finished(2.01));
        // A persistent one only after its stop and ramp-out.
        let p = BeamNode {
            persistent: true,
            ..n
        };
        let mut b = Beam::new(&p, 0.0, &mut rng);
        assert!(!b.finished(100.0));
        b.stop(100.0);
        assert!(!b.finished(100.4));
        assert!(b.finished(100.6));
    }

    #[test]
    fn beam_particles_follow_length_rate_and_time() {
        let mut rng = Ran2::new(9);
        let mut b = Beam::new(&node(), 0.0, &mut rng);
        let target = Vec3::new(10.0, 0.0, 0.0);
        let mut total = 0usize;
        let mut heads = 0usize;
        for f in 1..=60u32 {
            let u = b.update(
                f64::from(f) / 60.0,
                1.0 / 60.0,
                ends(target),
                true,
                true,
                &mut rng,
            );
            for (time, p) in &u.beam_spawns {
                assert!(*time <= f64::from(f) / 60.0 + 1e-9);
                assert!(p.y.abs() < 1e-5 && p.x >= -1e-5 && p.x <= 10.0 + 1e-5);
            }
            total += u.beam_spawns.len();
            heads += usize::from(u.head_spawn.is_some());
        }
        // ℓ grows to 10 m within 0.25 s; afterwards ⌊10 segments × 10 /s × Δ⌋ per update
        // (the fraction of an update is not carried, step 8): one per 60 Hz frame.
        assert!((40..=100).contains(&total), "{total}");
        assert!(heads <= 3, "the head arrives within 0.25 s: {heads}");
    }

    #[test]
    fn hostile_beams_stay_bounded_and_finite() {
        let mut rng = Ran2::new(5);
        let n = BeamNode {
            segment_size: 1e-30,
            beam_particles_per_second: i32::MAX,
            head_speed: f32::NAN,
            random_start_radius: f32::INFINITY,
            ..node()
        };
        let mut b = Beam::new(&n, 0.0, &mut rng);
        let u = b.update(
            0.5,
            0.5,
            ends(Vec3::new(5.0, 0.0, 0.0)),
            true,
            false,
            &mut rng,
        );
        assert!(u.beam_spawns.len() <= MAX_BEAM_SPAWNS);
        assert!(
            u.beam_spawns
                .iter()
                .all(|(t, p)| t.is_finite() && p.is_finite())
        );
        let far = b.update(
            1.0,
            0.5,
            ends(Vec3::new(5000.0, 0.0, 0.0)),
            true,
            false,
            &mut rng,
        );
        assert!(far.span.is_none(), "a beam over 1,000 m is not moved");
        let segs = ribbon_segments(
            &BeamNode {
                segment_size: 1e-6,
                head_segments: i32::MAX,
                tail_segments: 3,
                ..node()
            },
            50.0,
        );
        assert!(segs.len() <= MAX_RIBBON_SEGMENTS);
    }

    #[test]
    fn ribbon_layout_follows_the_segment_rule() {
        let n = BeamNode {
            segment_size: 2.0,
            head_segments: 1,
            tail_segments: 1,
            ..node()
        };
        let s = ribbon_segments(&n, 7.0);
        let parts: Vec<RibbonPart> = s.iter().map(|x| x.part).collect();
        assert_eq!(
            parts,
            vec![
                RibbonPart::Tail,
                RibbonPart::Body,
                RibbonPart::Body,
                RibbonPart::Head
            ]
        );
        assert!(
            (s[2].length - 1.0).abs() < 1e-6,
            "the last body piece is shorter"
        );
        let total: f32 = s.iter().map(|x| x.length).sum();
        assert!((total - 7.0).abs() < 1e-5);
        // Head and tail longer than the beam: they shrink, no body.
        let s = ribbon_segments(&n, 3.0);
        assert_eq!(s.len(), 2);
        assert!((s[0].length - 1.5).abs() < 1e-6);
        assert!(ribbon_segments(&n, 0.05).is_empty());
    }
}
