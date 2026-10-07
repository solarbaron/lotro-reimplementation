// SPDX-License-Identifier: GPL-3.0-or-later
//! Playback state on the main thread: single clips, cross-fades, speed blends and layers,
//! and the plain-data snapshot that sampling jobs evaluate (`skeleton-anim.md` §6, §10).
//!
//! The data decides *what* plays through script tables (`script-table.md`); this module
//! offers the mechanisms those tables need:
//!
//! - [`Playback`]: one clip with play time, rate, wrap and root-motion mode (§6.3, §6.4, §7);
//! - [`CrossFade`]: a clip replacing whatever played before (a clip, a speed blend or
//!   another fade still in progress) over a fade time, optionally phase-matched
//!   (`script-table.md` §8.5: walk ↔ run are separate branches with a 0.2 s cross-fade and
//!   phase matching);
//! - [`SpeedBlend`]: clips at parameter anchors weighted like a scalar blend
//!   (`script-table.md` §7.3), phase-synchronised, e.g. idle at 0, walk at 2 m/s and run at
//!   6 m/s driven by the ground speed;
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
}

impl Playback {
    /// Starts `clip` at time 0, rate 1, in place.
    #[must_use]
    pub fn new(clip: Arc<BoundClip>, wrap: Wrap) -> Self {
        Self {
            clip,
            time: 0.0,
            rate: 1.0,
            wrap,
            root_motion: RootMotionMode::InPlace,
        }
    }

    /// A **held** clip (`script-table.md` §6.7): playback rate 0, one-shot wrap, play time
    /// `fraction` × duration, so the pose never changes with elapsed time. `fraction` is the
    /// held fraction the script node's rule gives (already wrapped or limited to `0 ..= 1`;
    /// it is clamped here, and a non-finite one reads 0).
    #[must_use]
    pub fn held(clip: Arc<BoundClip>, fraction: f32) -> Self {
        let mut p = Self::new(clip, Wrap::Once);
        p.rate = 0.0;
        p.set_held_fraction(fraction);
        p
    }

    /// Moves a held clip to `fraction` of its duration at once (a changed driving
    /// property, `script-table.md` §6.7 step 3). Unlike [`Playback::set_phase`] the
    /// fraction 1 is kept as the clip's last pose.
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

/// Most clips one layer blends at once (a speed blend counts two, every cross-fade in
/// progress one more). [`Animator::play`] and [`Animator::set_source`] keep sources within
/// it by settling the oldest fade still in progress (to whichever side dominates it).
pub const MAX_LAYER_CLIPS: usize = 4;

/// A clip replacing what played before over `duration` seconds with a linear weight ramp.
///
/// The outgoing side is a whole [`Source`] that keeps playing during the fade, so a fade
/// that interrupts another fade starts from the blend the layer showed at that instant
/// (the pose stays continuous) and the older fade finishes underneath.
#[derive(Debug, Clone)]
pub struct CrossFade {
    /// What plays out: a clip, a speed blend or an earlier fade still in progress.
    pub from: Box<Source>,
    /// The incoming clip.
    pub to: Playback,
    /// Seconds since the fade started.
    pub elapsed: f32,
    /// Fade length in seconds (0 = immediate).
    pub duration: f32,
}

impl CrossFade {
    /// Starts fading from the clip `from` to `to` over `duration` seconds. With
    /// `sync_phase` the incoming clip starts at the outgoing clip's phase (phase matching,
    /// as `SynchronizeTiming` asks for between locomotion branches, `script-table.md`
    /// §8.5).
    #[must_use]
    pub fn new(from: Playback, to: Playback, duration: f32, sync_phase: bool) -> Self {
        Self::from_source(Source::Clip(from), to, duration, sync_phase)
    }

    /// Starts fading from any source to `to` over `duration` seconds; with `sync_phase`
    /// the incoming clip starts at the phase of the source's newest clip.
    #[must_use]
    pub fn from_source(from: Source, mut to: Playback, duration: f32, sync_phase: bool) -> Self {
        if sync_phase {
            to.set_phase(from.phase());
        }
        Self {
            from: Box::new(from),
            to,
            elapsed: 0.0,
            duration: finite_or_zero(duration).max(0.0),
        }
    }

    /// Weight of the incoming clip, `0 ..= 1`.
    #[must_use]
    pub fn weight(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        }
    }

    /// Whether the fade has completed.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.weight() >= 1.0
    }

    /// Advances both sides and the fade; returns their motions blended by the weight.
    pub fn advance(&mut self, dt: f32) -> MotionDelta {
        let a = self.from.advance(dt);
        let b = self.to.advance(dt);
        self.elapsed += finite_or_zero(dt).max(0.0);
        let w = self.weight();
        a.scaled(1.0 - w).then(b.scaled(w))
    }
}

/// Clips at parameter anchors, weighted by the parameter like a scalar blend
/// (`script-table.md` §7.3) and played phase-synchronised: every clip samples the same
/// fraction of its own duration, and the shared phase advances at the weighted average of
/// the clips' cycle rates, so feet stay in step while the weights change.
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

    /// The (up to two) clips with non-zero weight: the first one and, if any, the second
    /// with its share of the pair (the blend factor from the first to the second).
    fn active(&self) -> Option<(ClipSample, Option<(ClipSample, f32)>)> {
        let mut it = self
            .clips
            .iter()
            .zip(&self.weights)
            .filter(|(_, w)| **w > 0.0);
        let sample = |c: &Arc<BoundClip>| ClipSample {
            clip: Arc::clone(c),
            time: clip_time(self.phase * c.duration(), c.duration(), Wrap::Loop),
        };
        let (ca, wa) = it.next()?;
        let second = it.next().map(|(cb, wb)| (sample(cb), wb / (wa + wb)));
        Some((sample(ca), second))
    }
}

/// What a layer plays.
#[derive(Debug, Clone)]
pub enum Source {
    /// One clip.
    Clip(Playback),
    /// One clip replacing another.
    CrossFade(CrossFade),
    /// Phase-synchronised clips weighted by a parameter.
    Speed(SpeedBlend),
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
        }
    }

    /// Phase of the newest clip (what a phase-matched fade continues from).
    fn phase(&self) -> f32 {
        match self {
            Source::Clip(p) => p.phase(),
            Source::CrossFade(f) => f.to.phase(),
            Source::Speed(s) => s.phase(),
        }
    }

    /// Fades nested inside this source (0 for a clip or speed blend).
    fn fade_depth(&self) -> usize {
        let mut n = 0;
        let mut s = self;
        while let Source::CrossFade(f) = s {
            n += 1;
            s = &f.from;
        }
        n
    }

    /// Most clips the source blends at once (see [`MAX_LAYER_CLIPS`]).
    #[must_use]
    pub fn clip_count(&self) -> usize {
        let mut s = self;
        let mut n = 0;
        loop {
            match s {
                Source::Clip(_) => return n + 1,
                Source::Speed(b) => return n + b.clips.len().min(2),
                Source::CrossFade(f) => {
                    n += 1;
                    s = &f.from;
                }
            }
        }
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
        let mut s = self;
        loop {
            match s {
                Source::Clip(p) => return fits(&p.clip),
                Source::Speed(b) => return b.clips.iter().try_for_each(|c| fits(c)),
                Source::CrossFade(f) => {
                    fits(&f.to.clip)?;
                    s = &f.from;
                }
            }
        }
    }

    /// Settles the oldest fade in progress to the side that dominates it (the incoming
    /// clip from weight ½ on), removing at least one clip from the blend.
    fn settle_oldest_fade(&mut self) {
        let Some(depth) = self.fade_depth().checked_sub(1) else {
            return;
        };
        let mut s = self;
        for _ in 0..depth {
            match s {
                Source::CrossFade(f) => s = &mut f.from,
                _ => return,
            }
        }
        if let Source::CrossFade(f) = s {
            let settled = if f.weight() >= 0.5 {
                Source::Clip(f.to.clone())
            } else {
                std::mem::replace(&mut *f.from, Source::Clip(f.to.clone()))
            };
            *s = settled;
        }
    }

    /// Settles old fades until at most [`MAX_LAYER_CLIPS`] clips blend.
    fn limit_clips(&mut self) {
        while self.clip_count() > MAX_LAYER_CLIPS && self.fade_depth() > 0 {
            self.settle_oldest_fade();
        }
    }
}

/// One animation layer: a source, a weight that can ramp, and whether the clip's track
/// weights apply (`UseBoneOpacity`, `script-table.md` §6.5).
#[derive(Debug, Clone)]
pub struct Layer {
    /// What plays (`None` = nothing; the layer contributes nothing).
    pub source: Option<Source>,
    /// Current weight, `0 ..= 1`.
    pub weight: f32,
    /// Whether the clips' per-track weights are their track opacities (§6.6,
    /// `UseBoneOpacity`, `script-table.md` §6.5): on every layer, the lowest included.
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

    /// Ramps the weight to `target` over `seconds` (ramp-in / ramp-out times,
    /// `script-table.md` §6.1); 0 seconds sets it at once.
    pub fn fade_to(&mut self, target: f32, seconds: f32) {
        let target = if target.is_nan() {
            0.0
        } else {
            target.clamp(0.0, 1.0)
        };
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
                // Sources set through the public field may nest deeper than the API allows.
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

/// One layer flattened to plain data: a clip, then up to [`MAX_LAYER_CLIPS`] − 1 further
/// clips each blended over the result so far by its factor (a speed blend's pair, then one
/// per cross-fade in progress, oldest first).
#[derive(Debug, Clone)]
pub struct LayerSnapshot {
    /// Layer weight, `0 ..= 1`.
    pub weight: f32,
    /// Whether track weights apply (see [`Layer::use_track_weights`]).
    pub use_track_weights: bool,
    /// The first clip.
    pub a: ClipSample,
    /// Further clips in blend order with the blend factor towards each; the used entries
    /// come first.
    pub blends: [Option<(ClipSample, f32)>; MAX_LAYER_CLIPS - 1],
}

impl LayerSnapshot {
    /// The clips with their blend factors: `a` (factor 1), then the used `blends`.
    pub fn clips(&self) -> impl Iterator<Item = (&ClipSample, f32)> {
        std::iter::once((&self.a, 1.0)).chain(
            self.blends
                .iter()
                .map_while(|b| b.as_ref().map(|(c, x)| (c, *x))),
        )
    }
}

/// Builds the clip chain of one layer.
#[derive(Default)]
struct Chain {
    base: Option<ClipSample>,
    /// Share of the layer weight the chain gets: below 1 only when a fade starts from a
    /// source that contributed nothing (a speed blend with no weight). The rest of the
    /// layer weight shows the pose below the layer, which every later fade also fades out.
    base_weight: f32,
    blends: [Option<(ClipSample, f32)>; MAX_LAYER_CLIPS - 1],
    len: usize,
}

impl Chain {
    fn push(&mut self, c: ClipSample, alpha: f32) {
        let alpha = if alpha.is_nan() {
            0.0
        } else {
            alpha.clamp(0.0, 1.0)
        };
        if alpha <= 0.0 {
            return;
        }
        if self.base.is_none() {
            self.base_weight = 0.0;
        }
        // The chain so far shows at `base_weight` of the layer (the pose below the layer
        // fills the rest). Fading `c` in by `alpha` fades out both, so the chain's share
        // grows to `base_weight + (1 - base_weight) * alpha` and `c` is blended over the
        // chain by `alpha / share`, which keeps `c` at exactly `alpha` of the layer.
        let base_weight = if self.base_weight.is_nan() {
            0.0
        } else {
            self.base_weight.clamp(0.0, 1.0)
        };
        let share = (base_weight + (1.0 - base_weight) * alpha).clamp(0.0, 1.0);
        self.base_weight = share;
        let alpha = if share > 0.0 {
            (alpha / share).min(1.0)
        } else {
            1.0
        };
        if self.base.is_none() || alpha >= 1.0 {
            // A clip at full weight hides everything below it.
            self.base = Some(c);
            self.blends = Default::default();
            self.len = 0;
            return;
        }
        if self.len == self.blends.len() {
            // Full (only for sources nested past the limit): settle the oldest blend.
            if let Some((oldest, x)) = self.blends[0].take()
                && x >= 0.5
            {
                self.base = Some(oldest);
            }
            self.blends.rotate_left(1);
            self.len -= 1;
        }
        self.blends[self.len] = Some((c, alpha));
        self.len += 1;
    }

    /// Adds `src`; fades deeper than [`MAX_LAYER_CLIPS`] are read as their incoming clip.
    fn add(&mut self, src: &Source, depth: usize) {
        match src {
            Source::Clip(p) => self.push(p.sample(), 1.0),
            Source::Speed(s) => {
                if let Some((a, b)) = s.active() {
                    self.push(a, 1.0);
                    if let Some((b, x)) = b {
                        self.push(b, x);
                    }
                }
            }
            Source::CrossFade(f) => {
                let w = f.weight();
                if w >= 1.0 || depth >= MAX_LAYER_CLIPS {
                    self.push(f.to.sample(), 1.0);
                } else {
                    self.add(&f.from, depth + 1);
                    self.push(f.to.sample(), w);
                }
            }
        }
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

    /// Sets the source of layer `layer` (created if needed). A source blending more than
    /// [`MAX_LAYER_CLIPS`] clips has its oldest fades settled first.
    ///
    /// # Errors
    ///
    /// [`AnimError::TrackCountMismatch`] when any clip of the source (both sides of every
    /// fade, every clip of a speed blend) does not fit the skeleton,
    /// [`AnimError::TooManyEntries`] for a layer index out of range.
    pub fn set_source(&mut self, layer: usize, mut source: Source) -> Result<()> {
        self.check_source(&source)?;
        source.limit_clips();
        self.ensure_layer(layer)?.source = Some(source);
        Ok(())
    }

    /// Plays `next` on layer `layer`, cross-fading over `fade` seconds from whatever the
    /// layer shows: a clip, a speed blend, or a fade still in progress, which keeps
    /// running underneath so the pose stays continuous (at most [`MAX_LAYER_CLIPS`] clips
    /// blend; beyond that the oldest fade settles to its dominant side). An immediate
    /// switch when the layer is empty or `fade` ≤ 0.
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
                let mut s = Source::CrossFade(CrossFade::from_source(cur, next, fade, sync_phase));
                s.limit_clips();
                s
            }
            _ => Source::Clip(next),
        };
        l.source = Some(source);
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
            let mut chain = Chain::default();
            chain.add(src, 0);
            let Some(a) = chain.base else {
                continue;
            };
            out.push(LayerSnapshot {
                weight: l.weight * chain.base_weight,
                use_track_weights: l.use_track_weights,
                a,
                blends: chain.blends,
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
    /// Adds a contribution of weight `w`. The quaternion is negated first when it points
    /// away from the sum so far (shortest path, §6.5). Tessera Decision: the reference for
    /// that test is the running weighted sum (`skeleton-anim.md` §6.5 reports the exact
    /// reference as not traced); the first contribution is taken as it is.
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

    /// Step 4 of §6.6: a total below [`MIN_BONE_WEIGHT`] is filled with `reference`.
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

/// Minimum total weight of a bone; below it the reference pose fills the rest (§6.6 step 4).
pub const MIN_BONE_WEIGHT: f32 = 0.1;

/// A weight truncated to whole 1/255 steps (§6.6 step 3). Tessera Decision: 10⁻⁴ of a
/// step is added before truncating, so that weights such as 1 − 0.5 that floating point
/// computes a hair below a whole step keep that step.
#[must_use]
pub fn quantise_weight(w: f32) -> f32 {
    if !w.is_finite() || w <= 0.0 {
        return 0.0;
    }
    (255.0 * w.min(1.0) + 1e-4).floor() / 255.0
}

/// The blend factor of a chain entry, limited to `0 ..= 1` (NaN reads 0).
fn factor(x: f32) -> f32 {
    if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }
}

/// Evaluates layer snapshots into a local pose (`skeleton-anim.md` §10 steps 2–3) by the
/// layered-weight rule of §6.6 (K18):
///
/// 1. every clip *i* gets an overall weight *c_i*: its layer's weight times its share of the
///    layer's chain (a cross-fade's incoming clip its fade factor, the clips it fades out
///    the rest);
/// 2. per bone *j*, layers are visited from the **last snapshot to the first** (highest
///    priority first) with a remaining weight *r* = 1; each clip gets *w_ij* = *c_i* × *r* ×
///    *o_ij*, where the opacity *o_ij* is the clip's track weight *j* when the layer uses
///    track weights and the clip has them, else 1; clips of one layer share *r*, and after
///    the layer *r* drops by the layer's total (never below 0);
/// 3. every weight is truncated to whole 1/255 steps ([`quantise_weight`]);
/// 4. a bone whose total weight is below [`MIN_BONE_WEIGHT`] gets the reference transform
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
        for (c, _) in l.clips() {
            if c.clip.track_count() != n {
                return Err(AnimError::TrackCountMismatch {
                    tracks: c.clip.track_count(),
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
        let lw = factor(l.weight);
        if lw == 0.0 {
            continue;
        }
        // Shares of the chain: clip k keeps its factor times (1 − factor) of every later
        // entry ([`LayerSnapshot::clips`] gives the first entry factor 1).
        let mut clips: [Option<(&ClipSample, f32)>; MAX_LAYER_CLIPS] = Default::default();
        for (slot, (c, x)) in clips.iter_mut().zip(l.clips()) {
            *slot = Some((c, factor(x)));
        }
        let mut later = 1.0f32;
        for slot in clips.iter_mut().rev().flatten() {
            let x = slot.1;
            slot.1 = x * later;
            later *= 1.0 - x;
        }
        scratch.layer_sum.iter_mut().for_each(|x| *x = 0.0);
        for (c, share) in clips.iter().flatten() {
            let ci = lw * share;
            if ci.is_nan() || ci <= 0.0 {
                continue;
            }
            report.replaced += c.clip.sample(c.time, &mut scratch.a)?.replaced;
            let opacity = l.use_track_weights && c.clip.track_weights().is_some();
            for j in 0..n {
                let o = if opacity {
                    factor(c.clip.track_weight(j))
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
        assert_eq!(quantise_weight(7.0), 1.0);
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
