// SPDX-License-Identifier: GPL-3.0-or-later
//! Playback state on the main thread: single clips, cross-fades, speed blends, groups of
//! clips and layers, and the plain-data snapshot that sampling jobs evaluate
//! (`skeleton-anim.md` §6, §10).
//!
//! The data decides *what* plays through script tables (`script-table.md`); this module
//! offers the mechanisms those tables need. Every playing clip carries **one overall
//! weight** of its own (`skeleton-anim.md` §6.5, K21): it rises linearly from 0 when the
//! clip starts, falls linearly to 0 from its current value when the clip stops, and jumps
//! when the node's intensity changes. Clips of one layer share the layer's remaining
//! budget (§6.6 step 2); nothing multiplies the weights of clips that fade in after each
//! other.
//!
//! - [`Playback`]: one clip with play time, rate, wrap and root-motion mode (§6.3, §6.4, §7);
//! - [`CrossFade`]: the clips of a layer stopped together while a new clip starts: each
//!   stopped clip falls linearly from the weight it had to 0 and the new clip rises
//!   linearly from 0 to 1, over one fade time (a Switch cross-fade, `script-table.md`
//!   §7.1; a clip's own `RampIn` and `RampOut` when they are equal);
//! - [`SpeedBlend`]: clips at parameter anchors weighted like a scalar blend
//!   (`script-table.md` §7.3: the weights jump with the parameter), phase-synchronised;
//! - [`Source::Group`]: several clips playing together on one layer, each with its own
//!   weight (`script-table.md` §6.2: starting a clip never stops another clip of the same
//!   layer);
//! - [`Layer`] and [`Animator`]: layers from low to high priority, combined bone by bone by
//!   the layered-weight rule of §6.6 (K18): higher layers take their weight first, lower
//!   layers get what remains (`script-table.md` §6.2 and §6.5);
//! - held clips ([`Playback::held`]): a clip that never advances, shown at a fraction of its
//!   duration (`script-table.md` §6.7).

use std::sync::Arc;

use glam::{Vec3, Vec4};
use tessera_havok::Transform;

use crate::blend::scalar_blend_weights;
use crate::clip::{BoundClip, SampleReport, Wrap, clip_time, scale_usable, translation_usable};
use crate::error::{AnimError, Result};
use crate::motion::{MotionDelta, RootMotionMode, motion_between};
use crate::skeleton::{Skeleton, check_len};

fn finite_or_zero(x: f32) -> f32 {
    if x.is_finite() { x } else { 0.0 }
}

/// One clip playing.
#[derive(Debug, Clone)]
pub struct Playback {
    /// The bound clip.
    pub clip: Arc<BoundClip>,
    /// Play time τ in seconds (kept within one cycle for looping clips).
    pub time: f32,
    /// Playback rate (clip seconds per real second, §6.4).
    pub rate: f32,
    /// Looping or one-shot (§6.3).
    pub wrap: Wrap,
    /// Whether extracted motion moves the entity (§7).
    pub root_motion: RootMotionMode,
    /// Whether the clip's track weights are its track opacities (its node's
    /// `UseBoneOpacity`, §6.6, `script-table.md` §6.5). [`Layer::use_track_weights`] sets
    /// the same for every clip of a layer.
    pub use_track_weights: bool,
}

impl Playback {
    /// Starts `clip` at time 0, rate 1, in place, without track opacities.
    #[must_use]
    pub fn new(clip: Arc<BoundClip>, wrap: Wrap) -> Self {
        Self {
            clip,
            time: 0.0,
            rate: 1.0,
            wrap,
            root_motion: RootMotionMode::InPlace,
            use_track_weights: false,
        }
    }

    /// A **held** clip (`script-table.md` §6.7): playback rate 0, one-shot wrap, play time
    /// `fraction` × duration, so the pose never changes with elapsed time. `fraction` is the
    /// fraction of the whole duration (§6.7 step 2: the held fraction × the played length,
    /// over the duration; [`crate::held_clip_fraction`]); it is limited to `0 ..= 1`
    /// here, and NaN reads 0 (the §6.7 step 1 Decision).
    #[must_use]
    pub fn held(clip: Arc<BoundClip>, fraction: f32) -> Self {
        let mut p = Self::new(clip, Wrap::Once);
        p.rate = 0.0;
        p.set_held_fraction(fraction);
        p
    }

    /// Moves a held clip to `fraction` of its duration at once (a changed driving
    /// property, `script-table.md` §6.7 step 3). Unlike [`Playback::set_phase`] the
    /// fraction 1 is kept as the clip's last pose. A non-finite fraction reads 0: the only
    /// one the §6.7 rules can produce is NaN on a looping node, whose Tessera Decision is 0
    /// (§6.7 step 1 and its answer).
    pub fn set_held_fraction(&mut self, fraction: f32) {
        let d = self.clip.duration();
        self.time = if d > 0.0 && d.is_finite() {
            finite_or_zero(fraction).clamp(0.0, 1.0) * d
        } else {
            0.0
        };
    }

    /// The clip time to sample (§6.3).
    #[must_use]
    pub fn clip_time(&self) -> f32 {
        clip_time(self.time, self.clip.duration(), self.wrap)
    }

    /// Clip time as a fraction of the duration, in `0 ..= 1` (0 for a zero-length clip).
    #[must_use]
    pub fn phase(&self) -> f32 {
        let d = self.clip.duration();
        if d > 0.0 && d.is_finite() {
            (self.clip_time() / d).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Moves play time to `phase` (fraction of the duration) of the current cycle.
    pub fn set_phase(&mut self, phase: f32) {
        let d = self.clip.duration();
        if d > 0.0 && d.is_finite() {
            self.time = finite_or_zero(phase).clamp(0.0, 1.0) * d;
        }
    }

    /// Whether a one-shot clip has reached its end (always false when looping).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        match self.wrap {
            Wrap::Loop => false,
            Wrap::Once => {
                let d = self.clip.duration();
                if self.rate < 0.0 {
                    self.time <= 0.0
                } else {
                    !(d.is_finite() && d > 0.0) || self.time >= d
                }
            }
        }
    }

    /// Advances play time by `dt` real seconds at the playback rate and returns the
    /// entity's motion over the step (identity unless the mode is
    /// [`RootMotionMode::Translate`] and the clip has extracted motion).
    pub fn advance(&mut self, dt: f32) -> MotionDelta {
        let step = finite_or_zero(dt) * finite_or_zero(self.rate);
        let t0 = finite_or_zero(self.time);
        let t1 = t0 + step;
        let delta = match self.root_motion {
            RootMotionMode::Translate => motion_between(&self.clip, t0, t1, self.wrap),
            RootMotionMode::InPlace => MotionDelta::IDENTITY,
        };
        let d = self.clip.duration();
        self.time = if !(d.is_finite() && d > 0.0) {
            0.0
        } else {
            match self.wrap {
                Wrap::Loop => {
                    let t = t1.rem_euclid(d);
                    if t.is_finite() && t < d { t } else { 0.0 }
                }
                Wrap::Once => t1.clamp(0.0, d),
            }
        };
        delta
    }

    fn sample(&self) -> ClipSample {
        ClipSample {
            clip: Arc::clone(&self.clip),
            time: self.clip_time(),
        }
    }
}

/// Most clips one layer plays at once (a speed blend counts its clips with weight, a
/// cross-fade its stopped clips and the new one, a group its clips). A fade that would
/// exceed it drops its weakest stopped clips ([`CrossFade::from_source`]); a group refuses
/// further clips ([`Animator::add`]); both are logged. Tessera bound, not a client rule
/// (`skeleton-anim.md` §6.5: a layer has no limit, K29): the data starts at most two clips
/// on one layer of an avatar at a time.
pub const MAX_LAYER_CLIPS: usize = 4;

/// A weight limited to `0 ..= 1` (NaN reads 0).
fn unit(x: f32) -> f32 {
    if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }
}

/// The clips of a layer stopped together while a new clip starts, over one fade time
/// (`skeleton-anim.md` §6.5, K21): each stopped clip's overall weight falls linearly from
/// the value it had when the fade started to 0, and the new clip's rises linearly from 0
/// to 1. The stopped clips keep playing (their times advance) until the fade ends; their
/// own earlier ramps are abandoned.
#[derive(Debug, Clone)]
pub struct CrossFade {
    /// The stopped clips, each with its overall weight when the fade started (`0 ..= 1`).
    pub from: Vec<(Playback, f32)>,
    /// The incoming clip.
    pub to: Playback,
    /// Seconds since the fade started.
    pub elapsed: f32,
    /// Fade length in seconds (0 = immediate).
    pub duration: f32,
}

impl CrossFade {
    /// Starts fading from the clip `from` (at weight 1) to `to` over `duration` seconds.
    /// With `sync_phase` the incoming clip starts at the outgoing clip's phase.
    #[must_use]
    pub fn new(from: Playback, to: Playback, duration: f32, sync_phase: bool) -> Self {
        Self::from_source(Source::Clip(from), to, duration, sync_phase)
    }

    /// Stops every clip of `from` at the weight it has now and starts `to`, over
    /// `duration` seconds; with `sync_phase` the incoming clip starts at the phase of the
    /// source's newest clip. Stopped clips without weight are dropped, and beyond
    /// [`MAX_LAYER_CLIPS`] − 1 stopped clips the weakest are dropped.
    #[must_use]
    pub fn from_source(from: Source, mut to: Playback, duration: f32, sync_phase: bool) -> Self {
        if sync_phase {
            to.set_phase(from.phase());
        }
        let mut stopped = Vec::with_capacity(MAX_LAYER_CLIPS);
        from.into_weighted(&mut stopped);
        let mut f = Self {
            from: stopped,
            to,
            elapsed: 0.0,
            duration: finite_or_zero(duration).max(0.0),
        };
        f.limit_clips();
        f
    }

    /// Overall weight of the incoming clip, `0 ..= 1`: elapsed / duration.
    #[must_use]
    pub fn weight(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            unit(self.elapsed / self.duration)
        }
    }

    /// Overall weight now of a stopped clip whose weight was `start` when the fade began:
    /// `start` × (1 − [`CrossFade::weight`]), linear from `start` to 0.
    #[must_use]
    pub fn stopped_weight(&self, start: f32) -> f32 {
        unit(start) * (1.0 - self.weight())
    }

    /// Whether the fade has completed.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.weight() >= 1.0
    }

    /// Advances every clip and the fade; returns their motions weighted by their weights.
    pub fn advance(&mut self, dt: f32) -> MotionDelta {
        self.elapsed += finite_or_zero(dt).max(0.0);
        let incoming = self.weight();
        let mut m = MotionDelta::IDENTITY;
        for (p, w) in &mut self.from {
            m = m.then(p.advance(dt).scaled(unit(*w) * (1.0 - incoming)));
        }
        m.then(self.to.advance(dt).scaled(incoming))
    }

    /// Drops the weakest stopped clips until at most [`MAX_LAYER_CLIPS`] clips play (logged:
    /// a deviation from the client, which has no limit).
    fn limit_clips(&mut self) {
        self.from.retain(|(_, w)| unit(*w) > 0.0);
        if self.from.len() + 1 > MAX_LAYER_CLIPS {
            tracing::warn!(
                clips = self.from.len() + 1,
                max = MAX_LAYER_CLIPS,
                "cross-fade drops its weakest stopped clips (Tessera bound)"
            );
        }
        while self.from.len() + 1 > MAX_LAYER_CLIPS {
            let weakest = self
                .from
                .iter()
                .enumerate()
                .min_by(|a, b| unit(a.1.1).total_cmp(&unit(b.1.1)))
                .map(|(i, _)| i);
            match weakest {
                Some(i) => {
                    self.from.remove(i);
                }
                None => break,
            }
        }
    }
}

/// Clips at parameter anchors, weighted by the parameter like a scalar blend
/// (`script-table.md` §7.3: each clip's weight follows the parameter at once) and played
/// phase-synchronised: every clip samples the same fraction of its own duration, and the
/// shared phase advances at the weighted average of the clips' cycle rates, so feet stay in
/// step while the weights change.
///
/// Typical use: anchors (0, idle), (2, walk), (6, run) and the ground speed in m/s as the
/// parameter. The data itself switches walk and run as separate branches with a cross-fade
/// (`script-table.md` §8.5, use [`CrossFade`] for that); a speed blend is the smooth
/// alternative for crowds and tools. Speed blends do not move the entity (treadmill
/// clips, §7.2).
#[derive(Debug, Clone)]
pub struct SpeedBlend {
    anchors: Vec<f32>,
    clips: Vec<Arc<BoundClip>>,
    weights: Vec<f32>,
    /// The blend parameter (for example ground speed).
    pub param: f32,
    /// Playback rate multiplier.
    pub rate: f32,
    phase: f32,
}

impl SpeedBlend {
    /// Most anchors a speed blend accepts.
    pub const MAX_ANCHORS: usize = 16;

    /// A speed blend over `(anchor, clip)` entries (at most [`SpeedBlend::MAX_ANCHORS`]),
    /// all bound to one skeleton.
    ///
    /// # Errors
    ///
    /// [`AnimError::TooManyEntries`] for no entries or too many, and
    /// [`AnimError::TrackCountMismatch`] when the clips are bound to skeletons of
    /// different bone counts.
    pub fn new(entries: Vec<(f32, Arc<BoundClip>)>) -> Result<Self> {
        if entries.is_empty() || entries.len() > Self::MAX_ANCHORS {
            return Err(AnimError::TooManyEntries {
                table: "speed blend",
                count: entries.len(),
                max: Self::MAX_ANCHORS,
            });
        }
        let bones = entries[0].1.track_count();
        if let Some((_, c)) = entries.iter().find(|(_, c)| c.track_count() != bones) {
            return Err(AnimError::TrackCountMismatch {
                tracks: c.track_count(),
                bones,
            });
        }
        let (anchors, clips): (Vec<f32>, Vec<Arc<BoundClip>>) = entries.into_iter().unzip();
        let n = anchors.len();
        let mut s = Self {
            anchors,
            clips,
            weights: vec![0.0; n],
            param: 0.0,
            rate: 1.0,
            phase: 0.0,
        };
        s.update_weights();
        Ok(s)
    }

    fn update_weights(&mut self) {
        scalar_blend_weights(&self.anchors, self.param, &mut self.weights);
    }

    /// Sets the parameter and recomputes the weights.
    pub fn set_param(&mut self, param: f32) {
        self.param = param;
        self.update_weights();
    }

    /// Current weight of each entry.
    #[must_use]
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// The shared phase, `0 .. 1`.
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Advances the shared phase by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.update_weights();
        let mut cycles_per_second = 0.0f32;
        let mut total = 0.0f32;
        for (c, &w) in self.clips.iter().zip(&self.weights) {
            let d = c.duration();
            if w > 0.0 && d.is_finite() && d > 0.0 {
                cycles_per_second += w / d;
                total += w;
            }
        }
        if total > 0.0 {
            let step = finite_or_zero(dt) * finite_or_zero(self.rate) * cycles_per_second / total;
            let p = (self.phase + step).rem_euclid(1.0);
            self.phase = if p.is_finite() && p < 1.0 { p } else { 0.0 };
        }
    }

    /// The clips with a weight, each with its weight (at most two: scalar-blend weights
    /// fall on the two anchors around the parameter).
    fn active(&self) -> impl Iterator<Item = (&Arc<BoundClip>, f32)> {
        self.clips
            .iter()
            .zip(&self.weights)
            .filter(|(_, w)| unit(**w) > 0.0)
            .map(|(c, w)| (c, unit(*w)))
            .take(2)
    }

    /// Clip time of `c` at the shared phase.
    fn time_of(&self, c: &BoundClip) -> f32 {
        clip_time(self.phase * c.duration(), c.duration(), Wrap::Loop)
    }
}

/// What a layer plays.
#[derive(Debug, Clone)]
pub enum Source {
    /// One clip at weight 1.
    Clip(Playback),
    /// Clips stopped while a new clip starts.
    CrossFade(CrossFade),
    /// Phase-synchronised clips weighted by a parameter.
    Speed(SpeedBlend),
    /// Several clips playing together, each with its own overall weight (`0 ..= 1`;
    /// `script-table.md` §6.2: they share the layer's budget).
    Group(Vec<(Playback, f32)>),
}

impl Source {
    fn advance(&mut self, dt: f32) -> MotionDelta {
        match self {
            Source::Clip(p) => p.advance(dt),
            Source::CrossFade(f) => {
                let m = f.advance(dt);
                if f.is_done() {
                    *self = Source::Clip(f.to.clone());
                }
                m
            }
            Source::Speed(s) => {
                s.advance(dt);
                MotionDelta::IDENTITY
            }
            Source::Group(clips) => {
                let mut m = MotionDelta::IDENTITY;
                for (p, w) in clips.iter_mut() {
                    m = m.then(p.advance(dt).scaled(unit(*w)));
                }
                m
            }
        }
    }

    /// Phase of the newest clip (what a phase-matched fade continues from).
    fn phase(&self) -> f32 {
        match self {
            Source::Clip(p) => p.phase(),
            Source::CrossFade(f) => f.to.phase(),
            Source::Speed(s) => s.phase(),
            Source::Group(clips) => clips.last().map_or(0.0, |(p, _)| p.phase()),
        }
    }

    /// Visits every clip with a weight: its sample, its overall weight and whether its
    /// own track opacities apply.
    fn visit(&self, f: &mut impl FnMut(ClipSample, f32, bool)) {
        match self {
            Source::Clip(p) => f(p.sample(), 1.0, p.use_track_weights),
            Source::CrossFade(x) => {
                for (p, w) in &x.from {
                    let w = x.stopped_weight(*w);
                    if w > 0.0 {
                        f(p.sample(), w, p.use_track_weights);
                    }
                }
                let w = x.weight();
                if w > 0.0 {
                    f(x.to.sample(), w, x.to.use_track_weights);
                }
            }
            Source::Speed(s) => {
                for (c, w) in s.active() {
                    let sample = ClipSample {
                        clip: Arc::clone(c),
                        time: s.time_of(c),
                    };
                    f(sample, w, false);
                }
            }
            Source::Group(clips) => {
                for (p, w) in clips {
                    let w = unit(*w);
                    if w > 0.0 {
                        f(p.sample(), w, p.use_track_weights);
                    }
                }
            }
        }
    }

    /// Moves every clip with a weight into `out` with its current overall weight (a speed
    /// blend's clips become plain looping playbacks at the shared phase and rate).
    fn into_weighted(self, out: &mut Vec<(Playback, f32)>) {
        match self {
            Source::Clip(p) => out.push((p, 1.0)),
            Source::CrossFade(x) => {
                let incoming = x.weight();
                for (p, w) in x.from {
                    let w = unit(w) * (1.0 - incoming);
                    if w > 0.0 {
                        out.push((p, w));
                    }
                }
                if incoming > 0.0 {
                    out.push((x.to, incoming));
                }
            }
            Source::Speed(s) => {
                for (c, w) in s.active() {
                    let mut p = Playback::new(Arc::clone(c), Wrap::Loop);
                    p.time = s.time_of(c);
                    p.rate = s.rate;
                    out.push((p, w));
                }
            }
            Source::Group(clips) => {
                out.extend(clips.into_iter().filter(|(_, w)| unit(*w) > 0.0));
            }
        }
    }

    /// Clips the source plays with a weight (see [`MAX_LAYER_CLIPS`]).
    #[must_use]
    pub fn clip_count(&self) -> usize {
        let mut n = 0;
        self.visit(&mut |_, _, _| n += 1);
        n
    }

    /// Checks that every clip of the source has `bones` tracks.
    fn check_tracks(&self, bones: usize) -> Result<()> {
        let fits = |c: &BoundClip| {
            if c.track_count() == bones {
                Ok(())
            } else {
                Err(AnimError::TrackCountMismatch {
                    tracks: c.track_count(),
                    bones,
                })
            }
        };
        match self {
            Source::Clip(p) => fits(&p.clip),
            Source::Speed(b) => b.clips.iter().try_for_each(|c| fits(c)),
            Source::CrossFade(f) => {
                fits(&f.to.clip)?;
                f.from.iter().try_for_each(|(p, _)| fits(&p.clip))
            }
            Source::Group(clips) => clips.iter().try_for_each(|(p, _)| fits(&p.clip)),
        }
    }

    /// Keeps at most [`MAX_LAYER_CLIPS`] clips (sources set through public fields may hold
    /// more): a fade drops its weakest stopped clips, a group its weakest clips.
    fn limit_clips(&mut self) {
        match self {
            Source::CrossFade(f) => f.limit_clips(),
            Source::Group(clips) => {
                if clips.len() > MAX_LAYER_CLIPS {
                    tracing::warn!(
                        clips = clips.len(),
                        max = MAX_LAYER_CLIPS,
                        "layer drops its weakest clips (Tessera bound)"
                    );
                }
                while clips.len() > MAX_LAYER_CLIPS {
                    let weakest = clips
                        .iter()
                        .enumerate()
                        .min_by(|a, b| unit(a.1.1).total_cmp(&unit(b.1.1)))
                        .map(|(i, _)| i);
                    match weakest {
                        Some(i) => {
                            clips.remove(i);
                        }
                        None => break,
                    }
                }
            }
            Source::Clip(_) | Source::Speed(_) => {}
        }
    }
}

/// One animation layer: a source, a weight that can ramp, and whether the clips' track
/// weights apply (`UseBoneOpacity`, `script-table.md` §6.5).
#[derive(Debug, Clone)]
pub struct Layer {
    /// What plays (`None` = nothing; the layer contributes nothing).
    pub source: Option<Source>,
    /// Current weight, `0 ..= 1`: a factor on the overall weight of every clip of the layer
    /// (the node intensity of its clips, ramped linearly by [`Layer::fade_to`]).
    pub weight: f32,
    /// Whether the clips' per-track weights are their track opacities (§6.6,
    /// `UseBoneOpacity`, `script-table.md` §6.5) for every clip of the layer, on every
    /// layer, the lowest included. A clip's own [`Playback::use_track_weights`] sets it for
    /// that clip alone.
    pub use_track_weights: bool,
    target: f32,
    ramp_rate: f32,
}

impl Layer {
    /// A layer playing `source` at full weight, without track weights.
    #[must_use]
    pub fn new(source: Option<Source>) -> Self {
        Self {
            source,
            weight: 1.0,
            use_track_weights: false,
            target: 1.0,
            ramp_rate: 0.0,
        }
    }

    /// Ramps the weight linearly to `target` over `seconds` (ramp-in / ramp-out times,
    /// `script-table.md` §6.1, `skeleton-anim.md` §6.5); 0 seconds sets it at once.
    pub fn fade_to(&mut self, target: f32, seconds: f32) {
        let target = unit(target);
        self.target = target;
        let s = finite_or_zero(seconds);
        if s <= 0.0 {
            self.weight = target;
            self.ramp_rate = 0.0;
        } else {
            self.ramp_rate = (target - self.weight).abs() / s;
        }
    }

    fn advance(&mut self, dt: f32) -> MotionDelta {
        let dt = finite_or_zero(dt).max(0.0);
        if self.weight != self.target {
            let step = self.ramp_rate * dt;
            self.weight = if self.weight < self.target {
                (self.weight + step).min(self.target)
            } else {
                (self.weight - step).max(self.target)
            };
        }
        match &mut self.source {
            Some(s) => {
                // Sources set through the public field may hold more clips than the API.
                s.limit_clips();
                s.advance(dt).scaled(self.weight)
            }
            None => MotionDelta::IDENTITY,
        }
    }
}

/// One clip at one clip time, as a sampling job needs it.
#[derive(Debug, Clone)]
pub struct ClipSample {
    /// The bound clip.
    pub clip: Arc<BoundClip>,
    /// Clip time in `0 ..= duration` (wrap already applied).
    pub time: f32,
}

/// One clip of a [`LayerSnapshot`].
#[derive(Debug, Clone)]
pub struct LayerClip {
    /// The clip and its time.
    pub sample: ClipSample,
    /// The clip's own overall weight, `0 ..= 1` (its ramp; the layer weight multiplies it).
    pub weight: f32,
    /// Whether its track weights are its track opacities.
    pub use_track_weights: bool,
}

/// One layer flattened to plain data: up to [`MAX_LAYER_CLIPS`] clips, each with its own
/// overall weight.
#[derive(Debug, Clone)]
pub struct LayerSnapshot {
    /// Layer weight, `0 ..= 1`.
    pub weight: f32,
    /// The clips; the used entries come first.
    pub clips: [Option<LayerClip>; MAX_LAYER_CLIPS],
}

impl LayerSnapshot {
    /// The used clips.
    pub fn clips(&self) -> impl Iterator<Item = &LayerClip> {
        self.clips.iter().map_while(Option::as_ref)
    }
}

/// Per-instance animation state on the main thread: a skeleton and layers from low to
/// high priority.
#[derive(Debug, Clone)]
pub struct Animator {
    skeleton: Arc<Skeleton>,
    layers: Vec<Layer>,
}

impl Animator {
    /// Most layers an animator holds (the data's layer enum has 27 values).
    pub const MAX_LAYERS: usize = 32;

    /// An animator with no layers (it evaluates to the reference pose).
    #[must_use]
    pub fn new(skeleton: Arc<Skeleton>) -> Self {
        Self {
            skeleton,
            layers: Vec::new(),
        }
    }

    /// The skeleton.
    #[must_use]
    pub fn skeleton(&self) -> &Arc<Skeleton> {
        &self.skeleton
    }

    /// The layers, lowest priority first.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Mutable access to layer `i`.
    pub fn layer_mut(&mut self, i: usize) -> Option<&mut Layer> {
        self.layers.get_mut(i)
    }

    /// Makes sure layers `0 ..= i` exist (empty ones are added) and returns layer `i`.
    ///
    /// # Errors
    ///
    /// [`AnimError::TooManyEntries`] when `i` ≥ [`Animator::MAX_LAYERS`].
    pub fn ensure_layer(&mut self, i: usize) -> Result<&mut Layer> {
        if i >= Self::MAX_LAYERS {
            return Err(AnimError::TooManyEntries {
                table: "animator layers",
                count: i + 1,
                max: Self::MAX_LAYERS,
            });
        }
        while self.layers.len() <= i {
            self.layers.push(Layer::new(None));
        }
        Ok(&mut self.layers[i])
    }

    fn check_source(&self, source: &Source) -> Result<()> {
        source.check_tracks(self.skeleton.bone_count())
    }

    fn check(&self, bones: usize) -> Result<()> {
        if bones == self.skeleton.bone_count() {
            Ok(())
        } else {
            Err(AnimError::TrackCountMismatch {
                tracks: bones,
                bones: self.skeleton.bone_count(),
            })
        }
    }

    /// Sets the source of layer `layer` (created if needed). A source holding more than
    /// [`MAX_LAYER_CLIPS`] clips drops its weakest ones first.
    ///
    /// # Errors
    ///
    /// [`AnimError::TrackCountMismatch`] when any clip of the source (every stopped clip
    /// and the new clip of a fade, every clip of a speed blend or group) does not fit the
    /// skeleton, [`AnimError::TooManyEntries`] for a layer index out of range.
    pub fn set_source(&mut self, layer: usize, mut source: Source) -> Result<()> {
        self.check_source(&source)?;
        source.limit_clips();
        self.ensure_layer(layer)?.source = Some(source);
        Ok(())
    }

    /// Stops every clip of layer `layer` and starts `next` there, over `fade` seconds
    /// (`skeleton-anim.md` §6.5): each stopped clip's weight falls linearly from its
    /// current value to 0 and `next` rises linearly from 0 to 1 ([`CrossFade`]). An
    /// immediate switch when the layer is empty or `fade` ≤ 0.
    ///
    /// # Errors
    ///
    /// As [`Animator::set_source`].
    pub fn play(
        &mut self,
        layer: usize,
        next: Playback,
        fade: f32,
        sync_phase: bool,
    ) -> Result<()> {
        self.check(next.clip.track_count())?;
        let l = self.ensure_layer(layer)?;
        let source = match l.source.take() {
            Some(cur) if fade > 0.0 => {
                Source::CrossFade(CrossFade::from_source(cur, next, fade, sync_phase))
            }
            _ => Source::Clip(next),
        };
        l.source = Some(source);
        Ok(())
    }

    /// Starts `next` on layer `layer` at full weight **without stopping** the clips already
    /// there (`script-table.md` §6.2: several clips share a layer). The clips playing keep
    /// their current weights (a fade in progress or a speed blend is frozen at its weights)
    /// and all share the layer's budget (`skeleton-anim.md` §6.6 step 2).
    ///
    /// # Errors
    ///
    /// As [`Animator::set_source`], and [`AnimError::TooManyEntries`] when the layer
    /// already plays [`MAX_LAYER_CLIPS`] clips (the layer is then unchanged).
    pub fn add(&mut self, layer: usize, next: Playback) -> Result<()> {
        self.check(next.clip.track_count())?;
        let l = self.ensure_layer(layer)?;
        let Some(cur) = l.source.take() else {
            l.source = Some(Source::Clip(next));
            return Ok(());
        };
        let count = cur.clip_count();
        if count >= MAX_LAYER_CLIPS {
            tracing::warn!(
                layer,
                clips = count,
                max = MAX_LAYER_CLIPS,
                "clip refused: the layer is full (Tessera bound)"
            );
            l.source = Some(cur);
            return Err(AnimError::TooManyEntries {
                table: "layer clips",
                count: count + 1,
                max: MAX_LAYER_CLIPS,
            });
        }
        let mut clips = Vec::with_capacity(MAX_LAYER_CLIPS);
        cur.into_weighted(&mut clips);
        clips.push((next, 1.0));
        l.source = Some(Source::Group(clips));
        Ok(())
    }

    /// Advances every layer by `dt` seconds; returns the entity's root motion over the step
    /// (layer motions in order, each scaled by its layer weight).
    pub fn advance(&mut self, dt: f32) -> MotionDelta {
        let mut m = MotionDelta::IDENTITY;
        for l in &mut self.layers {
            m = m.then(l.advance(dt));
        }
        m
    }

    /// Writes the current state as plain data for a sampling job (`out` is cleared first).
    pub fn snapshot(&self, out: &mut Vec<LayerSnapshot>) {
        out.clear();
        for l in &self.layers {
            if l.weight <= 0.0 || l.weight.is_nan() {
                continue;
            }
            let Some(src) = &l.source else {
                continue;
            };
            let mut clips: [Option<LayerClip>; MAX_LAYER_CLIPS] = Default::default();
            let mut n = 0;
            src.visit(&mut |sample, weight, own| {
                if let Some(slot) = clips.get_mut(n) {
                    *slot = Some(LayerClip {
                        sample,
                        weight,
                        use_track_weights: own || l.use_track_weights,
                    });
                    n += 1;
                }
            });
            if n == 0 {
                continue;
            }
            out.push(LayerSnapshot {
                weight: l.weight,
                clips,
            });
        }
    }
}

/// Scratch buffers for [`evaluate`]; reuse one per worker or instance to avoid allocation.
#[derive(Debug, Clone, Default)]
pub struct EvalScratch {
    a: Vec<Transform>,
    acc: Vec<Accum>,
    remaining: Vec<f32>,
    layer_sum: Vec<f32>,
}

impl EvalScratch {
    /// Empty scratch; grows on first use.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Weighted sums of one bone's contributions (§6.6 step 5).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Accum {
    t: Vec3,
    s: Vec3,
    q: Vec4,
    w: f32,
}

impl Accum {
    /// Adds a contribution of weight `w`. The quaternion is negated first when its dot
    /// product with the weighted quaternion sum accumulated so far is negative (shortest
    /// path, `skeleton-anim.md` §6.5: the client's test for the reference fill, K23, and
    /// Reported for clip contributions); the first contribution is taken as it is.
    pub(crate) fn add(&mut self, t: &Transform, w: f32) {
        if w <= 0.0 || !w.is_finite() {
            return;
        }
        let q = Vec4::from_array(t.rotation);
        let q = if self.q.dot(q) < 0.0 { -q } else { q };
        self.t += Vec3::from_array(t.translation) * w;
        self.s += Vec3::from_array(t.scale) * w;
        self.q += q * w;
        self.w += w;
    }

    /// Step 4 of §6.6: a total of at most [`MIN_BONE_WEIGHT`] is filled with `reference` at
    /// weight 0.1 − total (0 at exactly 0.1).
    pub(crate) fn fill(&mut self, reference: &Transform) {
        if self.w < MIN_BONE_WEIGHT {
            let fill = MIN_BONE_WEIGHT - self.w;
            self.add(reference, fill);
        }
    }

    /// The weighted average; `fallback` where it cannot be formed or is not finite or
    /// beyond the broken-sample bounds ([`crate::BROKEN_TRANSLATION`], [`crate::BROKEN_SCALE`]).
    pub(crate) fn result(&self, fallback: &Transform) -> Transform {
        if !(self.w > 0.0 && self.w.is_finite()) {
            return *fallback;
        }
        let inv = 1.0 / self.w;
        let t = self.t * inv;
        let s = self.s * inv;
        let len = self.q.length();
        let rotation = if len.is_finite() && len > 1e-6 {
            (self.q / len).to_array()
        } else {
            fallback.rotation
        };
        let (t, s) = (t.to_array(), s.to_array());
        Transform {
            translation: if translation_usable(&t) {
                t
            } else {
                fallback.translation
            },
            rotation,
            scale: if scale_usable(&s) { s } else { fallback.scale },
        }
    }
}

/// Total weight of a bone up to which the reference pose fills the rest (§6.6 step 4).
pub const MIN_BONE_WEIGHT: f32 = 0.1;

/// A weight truncated to whole 1/255 steps (§6.6 step 3, BIN3): the single-precision
/// weight times 255, converted to an integer toward zero, over 255. No tolerance is added,
/// so a weight that single precision computes a hair below a whole step loses that step,
/// as in the client. A weight that is not finite or not positive gives 0 (hostile input).
#[must_use]
pub fn quantise_weight(w: f32) -> f32 {
    if !w.is_finite() || w <= 0.0 {
        return 0.0;
    }
    (255.0 * w).trunc() / 255.0
}

/// Evaluates layer snapshots into a local pose (`skeleton-anim.md` §10 steps 2–3) by the
/// layered-weight rule of §6.6 (K18):
///
/// 1. every clip *i* has an overall weight *c_i*: its layer's weight times the clip's own
///    weight ([`LayerClip::weight`], its linear ramp, §6.5);
/// 2. per bone *j*, layers are visited from the **last snapshot to the first** (highest
///    priority first) with a remaining weight *r* = 1; each clip gets *w_ij* = *c_i* × *r* ×
///    *o_ij* in single precision, where the opacity *o_ij* is the clip's track weight *j*
///    when its track weights apply and the clip has them, else 1; clips of one layer share
///    *r*, and after the layer *r* drops by the layer's total (never below 0);
/// 3. every weight is truncated to whole 1/255 steps ([`quantise_weight`]);
/// 4. a bone whose total weight is at most [`MIN_BONE_WEIGHT`] gets the reference transform
///    with weight 0.1 − total;
/// 5. the bone's local transform is the normalised weighted average of the contributions.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`] if `out` is shorter than the bone count, and
/// [`AnimError::TrackCountMismatch`] if a snapshot's clip has a different track count.
pub fn evaluate(
    skeleton: &Skeleton,
    layers: &[LayerSnapshot],
    scratch: &mut EvalScratch,
    out: &mut [Transform],
) -> Result<SampleReport> {
    let n = skeleton.bone_count();
    check_len("local pose", n, out.len())?;
    for l in layers {
        for c in l.clips() {
            if c.sample.clip.track_count() != n {
                return Err(AnimError::TrackCountMismatch {
                    tracks: c.sample.clip.track_count(),
                    bones: n,
                });
            }
        }
    }
    let out = &mut out[..n];
    let reference = skeleton.reference_local();
    scratch.a.resize(n, Transform::IDENTITY);
    scratch.acc.clear();
    scratch.acc.resize(n, Accum::default());
    scratch.remaining.clear();
    scratch.remaining.resize(n, 1.0);
    scratch.layer_sum.resize(n, 0.0);
    let mut report = SampleReport::default();
    for l in layers.iter().rev() {
        let lw = unit(l.weight);
        if lw == 0.0 {
            continue;
        }
        scratch.layer_sum.iter_mut().for_each(|x| *x = 0.0);
        for c in l.clips() {
            let ci = lw * unit(c.weight);
            if ci <= 0.0 {
                continue;
            }
            let clip = &c.sample.clip;
            report.replaced += clip.sample(c.sample.time, &mut scratch.a)?.replaced;
            let opacity = c.use_track_weights && clip.track_weights().is_some();
            for j in 0..n {
                let o = if opacity {
                    unit(clip.track_weight(j))
                } else {
                    1.0
                };
                let w = ci * scratch.remaining[j] * o;
                scratch.layer_sum[j] += w;
                scratch.acc[j].add(&scratch.a[j], quantise_weight(w));
            }
        }
        for (r, s) in scratch.remaining.iter_mut().zip(&scratch.layer_sum) {
            *r = (*r - s).max(0.0);
        }
    }
    for ((o, acc), r) in out.iter_mut().zip(&mut scratch.acc).zip(reference) {
        acc.fill(r);
        *o = acc.result(r);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantisation_truncates_to_steps() {
        assert_eq!(quantise_weight(1.0), 1.0);
        assert_eq!(quantise_weight(0.01), 2.0 / 255.0);
        assert_eq!(quantise_weight(0.99), 252.0 / 255.0);
        assert_eq!(quantise_weight(0.5), 127.0 / 255.0);
        assert_eq!(quantise_weight(1.0 - 0.5), 127.0 / 255.0);
        assert_eq!(quantise_weight(-1.0), 0.0);
        assert_eq!(quantise_weight(f32::NAN), 0.0);
        assert_eq!(quantise_weight(f32::INFINITY), 0.0);
    }

    /// `skeleton-anim.md` §6.6 step 3 (BIN3): no tolerance is added before truncating. In
    /// single precision 1 − 0.6 is 0.39999998, and 255 times that is 101.99999, so the
    /// weight keeps 101 steps, not 102.
    #[test]
    fn quantisation_adds_no_tolerance() {
        let r = 1.0f32 - 0.6;
        assert!(255.0 * r < 102.0);
        assert_eq!(quantise_weight(r), 101.0 / 255.0);
        assert_eq!(quantise_weight(1.0 - 0.99), 2.0 / 255.0);
    }

    #[test]
    fn accumulation_aligns_signs() {
        let mut a = Accum::default();
        let id = Transform::IDENTITY;
        let neg = Transform {
            rotation: [0.0, 0.0, 0.0, -1.0],
            ..id
        };
        a.add(&id, 0.5);
        a.add(&neg, 0.5);
        let r = a.result(&id);
        assert!((r.rotation[3].abs() - 1.0).abs() < 1e-6);
    }
}
