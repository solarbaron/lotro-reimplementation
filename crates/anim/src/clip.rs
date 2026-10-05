// SPDX-License-Identifier: GPL-3.0-or-later
//! Clip-to-skeleton binding, clip time (looping and wrap) and per-track sampling
//! (`skeleton-anim.md` §3, §6).

use std::sync::Arc;

use tessera_havok::{AnimationData, Clip, Transform, quat_nlerp};

use crate::error::{AnimError, Result};
use crate::skeleton::{Skeleton, check_len};

/// A sampled translation component beyond this many metres marks the track as broken
/// (`havok-data.md` §7.7: two clips hold values above 10⁴ m and 10¹⁰ m, which "may be
/// treated as broken"); the track then takes the skeleton's reference transform.
pub const BROKEN_TRANSLATION: f32 = 1.0e4;

/// How play time maps to clip time (`skeleton-anim.md` §6.3). Clips carry no loop flag;
/// the playing system decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Wrap {
    /// Looping: *t* = τ mod `duration` (the last frame repeats the first, so no segment
    /// from the last frame back to the first is added).
    #[default]
    Loop,
    /// One-shot: *t* = min(τ, `duration`), holding the last pose (and 0 before the start).
    Once,
}

/// Clip time for play time `tau` (seconds) under `wrap`, in `0 ..= duration`
/// (`skeleton-anim.md` §6.3, worked example §14.5). A non-finite `tau` gives 0, a duration
/// that is not finite and positive gives 0 (a constant pose). Negative play times wrap
/// backwards when looping and clamp to 0 when playing once.
#[must_use]
pub fn clip_time(tau: f32, duration: f32, wrap: Wrap) -> f32 {
    if !tau.is_finite() || !(duration.is_finite() && duration > 0.0) {
        return 0.0;
    }
    match wrap {
        Wrap::Loop => {
            let t = tau.rem_euclid(duration);
            // rem_euclid can round up to `duration` for tiny negative inputs.
            if t >= duration { 0.0 } else { t }
        }
        Wrap::Once => tau.clamp(0.0, duration),
    }
}

/// Bytes a decoded clip holds on the heap (an estimate for cache budgets).
#[must_use]
pub fn clip_heap_size(clip: &Clip) -> usize {
    use std::mem::size_of;
    let a = &clip.animation;
    let data = match &a.data {
        AnimationData::Spline(s) => s
            .blocks
            .iter()
            .map(|b| {
                size_of::<tessera_havok::SplineBlock>()
                    + b.tracks.len() * size_of::<tessera_havok::TrackChannels>()
                    + b.knots.len()
                    + b.points.len() * size_of::<f32>()
            })
            .sum::<usize>(),
        AnimationData::Interleaved(i) => i.transforms.len() * size_of::<Transform>(),
    };
    let annotations: usize = a
        .annotation_tracks
        .iter()
        .map(|t| {
            size_of::<tessera_havok::AnnotationTrack>()
                + t.name.as_ref().map_or(0, |n| n.len())
                + t.annotations
                    .iter()
                    .map(|x| {
                        size_of::<tessera_havok::Annotation>() + x.text.as_ref().map_or(0, Vec::len)
                    })
                    .sum::<usize>()
        })
        .sum();
    let frames: usize = [
        a.extracted_motion.as_ref(),
        clip.leading_reference_frame.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|r| r.samples.len() * 16)
    .sum();
    size_of::<Clip>() + data + annotations + frames + clip.track_weights.len() * 4
}

/// A clip bound to a skeleton by position: track *j* drives bone *j*
/// (`skeleton-anim.md` §3.2). Binding succeeds exactly when the clip's track count equals
/// the bone count; otherwise the clip is incompatible (§3.4) and is never bound partially
/// or by name.
///
/// Holds the decoded clip and the skeleton (both shared), the clip's sanitised per-track
/// weights (§6.6) and optionally a [`BakedClip`] used instead of curve evaluation.
#[derive(Debug, Clone)]
pub struct BoundClip {
    clip: Arc<Clip>,
    skeleton: Arc<Skeleton>,
    weights: Option<Arc<[f32]>>,
    baked: Option<Arc<BakedClip>>,
}

/// What one sampling call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleReport {
    /// Tracks whose sampled value was not finite or beyond [`BROKEN_TRANSLATION`] and was
    /// replaced by the bone's reference transform.
    pub replaced: u32,
}

impl BoundClip {
    /// Binds `clip` to `skeleton` (§3.2, §3.4).
    ///
    /// # Errors
    ///
    /// [`AnimError::TrackCountMismatch`] when the clip's track count differs from the bone
    /// count.
    pub fn bind(clip: Arc<Clip>, skeleton: Arc<Skeleton>) -> Result<Self> {
        let tracks = clip.track_count();
        let bones = skeleton.bone_count();
        if tracks != bones {
            return Err(AnimError::TrackCountMismatch { tracks, bones });
        }
        let weights = if clip.track_weights.is_empty() {
            None
        } else {
            // Parsed arrays have one value per track in [0, 1]; anything else is
            // sanitised: missing values and NaN read 1, others are clamped.
            Some(
                (0..tracks)
                    .map(|j| {
                        let w = clip.track_weights.get(j).copied().unwrap_or(1.0);
                        if w.is_nan() { 1.0 } else { w.clamp(0.0, 1.0) }
                    })
                    .collect::<Arc<[f32]>>(),
            )
        };
        Ok(BoundClip {
            clip,
            skeleton,
            weights,
            baked: None,
        })
    }

    /// Uses `baked` for sampling instead of evaluating curves.
    ///
    /// The baked clip must come from this clip: besides the track count, its frame count,
    /// duration and its first and last frames must match this clip's bit for bit (a cheap
    /// identity check against a cache key mix-up; sampling the two frames costs two frame
    /// evaluations).
    ///
    /// # Errors
    ///
    /// [`AnimError::TrackCountMismatch`] if the baked clip has a different track count,
    /// [`AnimError::Mismatch`] if it was baked from another clip.
    pub fn with_baked(mut self, baked: Arc<BakedClip>) -> Result<Self> {
        if baked.tracks != self.track_count() {
            return Err(AnimError::TrackCountMismatch {
                tracks: baked.tracks,
                bones: self.track_count(),
            });
        }
        if !baked.matches(&self.clip) {
            return Err(AnimError::Mismatch { what: "baked clip" });
        }
        self.baked = Some(baked);
        Ok(self)
    }

    /// The decoded clip.
    #[must_use]
    pub fn clip(&self) -> &Arc<Clip> {
        &self.clip
    }

    /// The skeleton the clip is bound to.
    #[must_use]
    pub fn skeleton(&self) -> &Arc<Skeleton> {
        &self.skeleton
    }

    /// The baked samples in use, if any.
    #[must_use]
    pub fn baked(&self) -> Option<&Arc<BakedClip>> {
        self.baked.as_ref()
    }

    /// Sampling duration in seconds (the tagfile's, `havok-data.md` §2.3).
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.clip.duration()
    }

    /// Number of frames.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.clip.animation.frame_count()
    }

    /// Number of tracks (= bones).
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.clip.track_count()
    }

    /// Per-track weights (§6.6), `None` when the clip has none (weight 1 on every track).
    #[must_use]
    pub fn track_weights(&self) -> Option<&[f32]> {
        self.weights.as_deref()
    }

    /// Weight of track `j` (1 when the clip has no weights or `j` is out of range).
    #[must_use]
    pub fn track_weight(&self, j: usize) -> f32 {
        self.weights
            .as_ref()
            .and_then(|w| w.get(j).copied())
            .unwrap_or(1.0)
    }

    /// Whether the clip carries an extracted-motion reference frame (§7.3).
    #[must_use]
    pub fn has_extracted_motion(&self) -> bool {
        self.clip
            .reference_frame()
            .is_some_and(|(r, _)| !r.samples.is_empty())
    }

    /// Samples every track at clip time `t` (seconds; clamped to `0 ..= duration` by the
    /// decoder) into `out`, one local transform per bone.
    ///
    /// Rotations are normalised. A track whose value is not finite, or whose translation
    /// exceeds [`BROKEN_TRANSLATION`], takes the bone's reference transform and is counted
    /// in the report. Does not allocate.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if `out` holds fewer entries than the bone count.
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> Result<SampleReport> {
        let n = self.track_count();
        check_len("local pose", n, out.len())?;
        let out = &mut out[..n];
        let written = match &self.baked {
            Some(b) => b.sample(t, out),
            None => self.clip.sample(t, out),
        };
        let reference = self.skeleton.reference_local();
        let mut report = SampleReport::default();
        for (j, o) in out.iter_mut().enumerate() {
            if j >= written {
                *o = reference[j];
                report.replaced += 1;
                continue;
            }
            if !usable(o) {
                *o = reference[j];
                report.replaced += 1;
                continue;
            }
            o.rotation = normalise(o.rotation);
        }
        Ok(report)
    }

    /// Samples at play time `tau` after applying `wrap` ([`clip_time`]).
    ///
    /// # Errors
    ///
    /// As [`BoundClip::sample`].
    pub fn sample_at(&self, tau: f32, wrap: Wrap, out: &mut [Transform]) -> Result<SampleReport> {
        self.sample(clip_time(tau, self.duration(), wrap), out)
    }

    /// Largest horizontal (x, y) distance, in metres, between the first and last frame of
    /// the clip's first four tracks (§7.2: a clip is in place when every one of them
    /// returns within 10 cm). Uses the decoder directly; 0 for clips with no frames.
    #[must_use]
    pub fn root_displacement(&self) -> f32 {
        let n = self.track_count().min(4);
        let last = self.frame_count().saturating_sub(1);
        let mut a = [Transform::IDENTITY; 4];
        let mut b = [Transform::IDENTITY; 4];
        // `sample_frame` writes as many tracks as fit, so four-entry buffers suffice.
        self.clip.animation.sample_frame(0, &mut a);
        self.clip.animation.sample_frame(last, &mut b);
        (0..n)
            .map(|j| {
                let dx = b[j].translation[0] - a[j].translation[0];
                let dy = b[j].translation[1] - a[j].translation[1];
                (dx * dx + dy * dy).sqrt()
            })
            .filter(|d| d.is_finite())
            .fold(0.0, f32::max)
    }
}

fn usable(t: &Transform) -> bool {
    t.is_finite() && t.translation.iter().all(|v| v.abs() <= BROKEN_TRANSLATION)
}

fn normalise(q: [f32; 4]) -> [f32; 4] {
    let len = tessera_havok::quat_length(q);
    if len.is_finite() && len > 1e-6 {
        q.map(|c| c / len)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// A clip sampled at every frame into a dense frame-major array ("sampled keys"), so that
/// playback interpolates two stored frames instead of evaluating B-spline curves.
///
/// At frame times it reproduces the decoder exactly; between frames it interpolates
/// translation and scale linearly and rotation by normalised linear interpolation with
/// sign alignment, the interleaved-clip rule of `havok-data.md` §7.8. For spline clips this
/// differs slightly from curve evaluation between frames (curves of degree above 1 are not
/// straight between knots); use it where that difference is acceptable (crowds, distant
/// characters) and the curve path for close-ups.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedClip {
    tracks: usize,
    frames: usize,
    duration: f32,
    samples: Vec<Transform>,
}

impl BakedClip {
    /// Bakes `clip`, or returns `None` when the result would exceed `max_bytes` (or the
    /// clip has no tracks or frames).
    #[must_use]
    pub fn bake(clip: &Clip, max_bytes: usize) -> Option<Self> {
        let tracks = clip.track_count();
        let frames = clip.animation.frame_count();
        if tracks == 0 || frames == 0 {
            return None;
        }
        let bytes = tracks
            .checked_mul(frames)?
            .checked_mul(std::mem::size_of::<Transform>())?;
        if bytes > max_bytes {
            return None;
        }
        let mut samples = vec![Transform::IDENTITY; tracks * frames];
        for (f, chunk) in samples.chunks_exact_mut(tracks).enumerate() {
            clip.animation.sample_frame(f, chunk);
        }
        Some(BakedClip {
            tracks,
            frames,
            duration: clip.duration(),
            samples,
        })
    }

    /// Whether this was baked from `clip` (same shape and duration, bit-identical first
    /// and last frames).
    #[must_use]
    pub fn matches(&self, clip: &Clip) -> bool {
        if self.tracks != clip.track_count()
            || self.frames != clip.animation.frame_count()
            || self.duration.to_bits() != clip.duration().to_bits()
            || self.frames == 0
        {
            return false;
        }
        let mut frame = vec![Transform::IDENTITY; self.tracks];
        [0, self.frames - 1].into_iter().all(|f| {
            // As in `bake`: entries the decoder does not write stay at identity.
            frame.fill(Transform::IDENTITY);
            clip.animation.sample_frame(f, &mut frame);
            self.samples
                .get(f * self.tracks..(f + 1) * self.tracks)
                .is_some_and(|stored| stored.iter().zip(&frame).all(|(a, b)| same_bits(a, b)))
        })
    }

    /// Bytes held on the heap.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.samples.len() * std::mem::size_of::<Transform>()
    }

    /// Number of tracks.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks
    }

    /// Number of frames.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames
    }

    /// Samples every track at clip time `t` into `out` (as many as fit); returns the
    /// number written. `t` is clamped as in `havok-data.md` §7.4.
    pub fn sample(&self, t: f32, out: &mut [Transform]) -> usize {
        let n = self.tracks.min(out.len());
        if self.frames < 2 {
            out[..n].copy_from_slice(&self.samples[..n]);
            return n;
        }
        let t = tessera_havok::clamp_time(t, self.duration);
        let delta = self.duration / (self.frames - 1) as f32;
        let x = t / delta;
        let x = if x.is_finite() { x.max(0.0) } else { 0.0 };
        let f = (x.floor() as usize).min(self.frames - 2);
        let alpha = (x - f as f32).clamp(0.0, 1.0);
        let a = &self.samples[f * self.tracks..(f + 1) * self.tracks];
        let b = &self.samples[(f + 1) * self.tracks..(f + 2) * self.tracks];
        if alpha == 0.0 {
            out[..n].copy_from_slice(&a[..n]);
            return n;
        }
        for ((o, x), y) in out[..n].iter_mut().zip(a).zip(b) {
            *o = Transform {
                translation: std::array::from_fn(|k| {
                    x.translation[k] + (y.translation[k] - x.translation[k]) * alpha
                }),
                rotation: quat_nlerp(x.rotation, y.rotation, alpha),
                scale: std::array::from_fn(|k| x.scale[k] + (y.scale[k] - x.scale[k]) * alpha),
            };
        }
        n
    }
}

fn same_bits(a: &Transform, b: &Transform) -> bool {
    a.translation
        .iter()
        .chain(&a.rotation)
        .chain(&a.scale)
        .zip(b.translation.iter().chain(&b.rotation).chain(&b.scale))
        .all(|(x, y)| x.to_bits() == y.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_example_14_5() {
        // Duration 1.0 s, 61 frames: 2.25 s loops to 0.25, 3.0 s to 0; once clamps to 1.0.
        assert!((clip_time(2.25, 1.0, Wrap::Loop) - 0.25).abs() < 1e-6);
        assert_eq!(clip_time(3.0, 1.0, Wrap::Loop), 0.0);
        assert_eq!(clip_time(2.25, 1.0, Wrap::Once), 1.0);
        assert_eq!(clip_time(-0.5, 1.0, Wrap::Once), 0.0);
        assert!((clip_time(-0.25, 1.0, Wrap::Loop) - 0.75).abs() < 1e-6);
        assert_eq!(clip_time(f32::NAN, 1.0, Wrap::Loop), 0.0);
        assert_eq!(clip_time(f32::INFINITY, 1.0, Wrap::Once), 0.0);
        assert_eq!(clip_time(5.0, 0.0, Wrap::Loop), 0.0);
        assert_eq!(clip_time(5.0, f32::NAN, Wrap::Once), 0.0);
        let t = clip_time(-1e-9, 1.0, Wrap::Loop);
        assert!((0.0..1.0).contains(&t));
    }
}
