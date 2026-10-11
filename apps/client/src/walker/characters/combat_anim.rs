// SPDX-License-Identifier: GPL-3.0-or-later
//! Clips that scripts start on a figure (`script-table.md` §6.10, `combat-visuals.md` §3.3):
//! combat behaviours, hit reactions, effect and death clips played by the script-table
//! runtime's animation nodes on top of the figure's idle and locomotion layers.
//!
//! Each started clip gets its own animator layer, inserted by its data layer key
//! (`script-table.md` §6.2: a higher key overrides a lower one bone by bone, so an attack on
//! layer 40 plays over the idle (10, 15) and movement (20) layers and under the held
//! proportion layer 110; with `UseBoneOpacity` the clip's track weights let the locomotion
//! layer keep the legs, `skeleton-anim.md` §6.6). A newer clip of the same key goes above the
//! older ones. The clip's weight ramps in over `RampIn` to the node's intensity and out over
//! `RampOut` when the node stops or, for a one-shot clip, at its scheduled stop (§6.10 steps 4
//! and 8: τ_end − `RampOut`); the layer is removed once faded out.
//!
//! Tessera simplification (recorded in the module docs, not a spec rule): clips of one data
//! layer are kept on separate animator layers instead of sharing one layer's clips.

use std::sync::Arc;

use tessera_anim::{Animator, BoundClip, Playback, Source, Wrap};
use tessera_formats::script_table::AnimationNode;

use super::motion::Locomotion;

/// Below this a rate counts as zero (`script-table.md` §6.10 step 2).
pub const RATE_EPSILON: f32 = 0.0002;
/// Highest layer key the client plays (`script-table.md` §6.10 step 2).
pub const MAX_LAYER_KEY: u32 = 255;

/// How long a started clip runs before its node completes (`script-table.md` §6.10 steps
/// 3–4, 8: the node is told when the fade-out begins).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipRun {
    /// Not started (refused, or its end has passed already).
    Refused,
    /// A one-shot clip: seconds until its fade-out begins.
    For(f64),
    /// Looping or held: until its node stops it.
    UntilStopped,
}

/// A clip a node asks for, already bound to the figure's skeleton.
#[derive(Debug, Clone)]
pub struct ClipRequest {
    /// The sink's handle.
    pub handle: u64,
    /// The clip.
    pub clip: Arc<BoundClip>,
    /// The node's fields (driven values applied).
    pub node: AnimationNode,
    /// The node's intensity (the clip's target weight).
    pub intensity: f32,
    /// The start fraction *f*₀ (`StartTime`, or the random draw of `RandomizeStartTime`).
    pub start: f32,
}

impl ClipRequest {
    /// The playback rate: `Speed` × `SpeedMultiplier` (§6.10 step 1); a value that is not
    /// finite reads 0.
    #[must_use]
    pub fn rate(&self) -> f32 {
        let r = self.node.speed * self.node.speed_multiplier;
        if r.is_finite() { r } else { 0.0 }
    }

    /// How long the clip runs (§6.10 steps 2–4, 8).
    #[must_use]
    pub fn run(&self) -> ClipRun {
        let n = &self.node;
        if n.layer > MAX_LAYER_KEY {
            return ClipRun::Refused;
        }
        let rate = self.rate();
        if n.loop_forever || n.manual_update || rate < RATE_EPSILON {
            return ClipRun::UntilStopped;
        }
        let d = f64::from(self.clip.duration());
        if !(d.is_finite() && d > 0.0) {
            return ClipRun::Refused;
        }
        let count = f64::from(n.loop_count.max(1));
        let end = f64::from(n.end_time.clamp(0.0, 1.0));
        let start = f64::from(self.start.clamp(0.0, 1.0));
        let left = (count * d - (1.0 - end) * d - start * d) / f64::from(rate);
        if left.is_nan() || left <= 0.0 {
            return ClipRun::Refused;
        }
        ClipRun::For((left - f64::from(n.ramp_out.max(0.0))).max(0.0))
    }

    fn playback(&self) -> Playback {
        let n = &self.node;
        let d = self.clip.duration();
        if n.manual_update {
            let mut p = Playback::held(Arc::clone(&self.clip), n.manual_time);
            p.use_track_weights = n.use_bone_opacity;
            return p;
        }
        let looping = n.loop_forever || n.loop_count > 1;
        let mut p = Playback::new(
            Arc::clone(&self.clip),
            if looping { Wrap::Loop } else { Wrap::Once },
        );
        let rate = self.rate();
        p.rate = rate;
        p.use_track_weights = n.use_bone_opacity;
        p.time = self.initial_time(d);
        p
    }

    /// The initial clip time (§6.10 steps 3 and 5): with τ₀ = now − *f*₀ × duration / rate
    /// (rate above [`RATE_EPSILON`]), min(now − τ₀, duration), a wall-clock time not
    /// multiplied by the rate; 0 otherwise.
    #[must_use]
    pub fn initial_time(&self, duration: f32) -> f32 {
        let rate = self.rate();
        let f0 = if self.start.is_finite() {
            self.start.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if !(duration.is_finite() && duration > 0.0) || rate < RATE_EPSILON || f0 <= 0.0 {
            return 0.0;
        }
        let t = f0 * duration / rate;
        if t.is_finite() {
            t.clamp(0.0, duration)
        } else {
            0.0
        }
    }
}

/// A command for a figure's clips, queued by the script sink and applied by the character
/// layer.
#[derive(Debug, Clone)]
pub enum ClipCommand {
    /// Start a clip on the figure of a session entity (by its network ID).
    Play {
        /// The entity.
        entity: tessera_net::EntityId,
        /// The clip.
        request: ClipRequest,
    },
    /// Fade out and remove a started clip.
    Stop {
        /// The sink's handle.
        handle: u64,
    },
    /// A driven field of the clip's node changed while it plays (§6.10 step 7).
    Update {
        /// The sink's handle.
        handle: u64,
        /// The node's fields with the current driven values.
        node: AnimationNode,
    },
}

/// The script clips of one figure: the data layer key of every animator layer, and which
/// layers hold a script clip (by handle).
#[derive(Debug, Clone, Default)]
pub struct ScriptClips {
    /// Data layer key of each animator layer (base layers from `make_animator`).
    pub keys: Vec<u32>,
    /// The handle of the script clip each layer holds (`None` for base layers).
    pub handles: Vec<Option<u64>>,
    /// Layers fading out, removed at weight 0.
    pub fading: Vec<bool>,
    /// The scheduled stop of each one-shot script clip (§6.10 step 4): seconds left until
    /// its fade-out begins, and its `RampOut`.
    pub ends: Vec<Option<(f32, f32)>>,
}

impl ScriptClips {
    /// The layers of a fresh animator with base layer `keys`.
    #[must_use]
    pub fn new(keys: Vec<u32>) -> Self {
        let n = keys.len();
        Self {
            keys,
            handles: vec![None; n],
            fading: vec![false; n],
            ends: vec![None; n],
        }
    }

    fn in_step(&self, animator: &Animator) -> bool {
        let n = animator.layers().len();
        self.keys.len() == n
            && self.handles.len() == n
            && self.fading.len() == n
            && self.ends.len() == n
    }

    fn shift(loco: &mut Locomotion, at: usize, up: bool) {
        let f = |i: &mut usize| {
            if *i >= at {
                *i = if up { *i + 1 } else { i.saturating_sub(1) };
            }
        };
        f(&mut loco.move_index);
        for i in &mut loco.above {
            f(i);
        }
    }

    /// Starts `req`: a new animator layer above every layer of a lower or equal key, its
    /// weight ramping from 0 to the intensity over `RampIn`. `false` when refused (the
    /// animator is full or the clip does not fit the skeleton).
    pub fn play(
        &mut self,
        animator: &mut Animator,
        loco: &mut Locomotion,
        req: &ClipRequest,
    ) -> bool {
        if !self.in_step(animator) {
            // Out of step (should not happen): resynchronise as base layers.
            *self = Self::new(vec![0; animator.layers().len()]);
        }
        let key = req.node.layer;
        let at = self
            .keys
            .iter()
            .position(|k| *k > key)
            .unwrap_or(self.keys.len());
        if animator.insert_layer(at).is_err() {
            return false;
        }
        if animator.play(at, req.playback(), 0.0, false).is_err() {
            animator.remove_layer(at);
            return false;
        }
        let target = if req.intensity.is_finite() {
            req.intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(l) = animator.layer_mut(at) {
            l.weight = 0.0;
            l.fade_to(target, req.node.ramp_in.max(0.0));
        }
        self.keys.insert(at, key);
        self.handles.insert(at, Some(req.handle));
        self.fading.insert(at, false);
        let end = match req.run() {
            ClipRun::For(t) => {
                #[allow(clippy::cast_possible_truncation)]
                let t = t as f32;
                Some((t.max(0.0), req.node.ramp_out.max(0.0)))
            }
            ClipRun::Refused | ClipRun::UntilStopped => None,
        };
        self.ends.insert(at, end);
        Self::shift(loco, at, true);
        true
    }

    /// Fades out the clip of `handle` over its `RampOut` (0: removed at once). `false` when
    /// the figure holds no such clip.
    pub fn stop(
        &mut self,
        animator: &mut Animator,
        loco: &mut Locomotion,
        handle: u64,
        ramp_out: f32,
    ) -> bool {
        let Some(i) = self.handles.iter().position(|h| *h == Some(handle)) else {
            return false;
        };
        self.fade(animator, i, ramp_out);
        self.sweep(animator, loco);
        true
    }

    fn fade(&mut self, animator: &mut Animator, i: usize, ramp_out: f32) {
        if self.fading.get(i).copied().unwrap_or(true) {
            return;
        }
        if let Some(l) = animator.layer_mut(i) {
            l.fade_to(
                0.0,
                if ramp_out.is_finite() {
                    ramp_out.max(0.0)
                } else {
                    0.0
                },
            );
        }
        self.fading[i] = true;
        self.ends[i] = None;
    }

    /// Advances the scheduled stops by `dt` seconds (one-shot clips begin their fade-out at
    /// τ_end − `RampOut`, §6.10 step 4), then removes the faded-out layers.
    pub fn tick(&mut self, animator: &mut Animator, loco: &mut Locomotion, dt: f32) {
        if !self.in_step(animator) {
            return;
        }
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        for i in 0..self.ends.len() {
            let Some((left, ramp_out)) = self.ends[i] else {
                continue;
            };
            let left = left - dt;
            if left <= 0.0 {
                self.fade(animator, i, ramp_out);
            } else {
                self.ends[i] = Some((left, ramp_out));
            }
        }
        self.sweep(animator, loco);
    }

    /// A driven field of the node of `handle` changed while its clip plays (§6.10 step 7):
    /// the new rate (`Speed` × `SpeedMultiplier`) applies at once, a held clip takes its
    /// time from `ManualUpdateTime` again, and the `UseBoneOpacity` flag blends over 0.2 s
    /// (`skeleton-anim.md` §6.6). `false` when the figure holds no such clip.
    pub fn update(&mut self, animator: &mut Animator, handle: u64, node: &AnimationNode) -> bool {
        let Some(i) = self.handles.iter().position(|h| *h == Some(handle)) else {
            return false;
        };
        let Some(layer) = animator.layer_mut(i) else {
            return false;
        };
        if let Some(Source::Clip(p)) = &mut layer.source {
            if node.manual_update {
                p.set_held_fraction(node.manual_time);
            } else {
                let r = node.speed * node.speed_multiplier;
                p.rate = if r.is_finite() { r } else { 0.0 };
            }
        }
        layer.blend_bone_opacity(node.use_bone_opacity);
        true
    }

    /// Removes the faded-out layers.
    pub fn sweep(&mut self, animator: &mut Animator, loco: &mut Locomotion) {
        if !self.in_step(animator) {
            return;
        }
        let mut i = 0;
        while i < self.handles.len() {
            let gone = self.fading[i] && animator.layers().get(i).is_none_or(|l| l.weight <= 0.0);
            if gone {
                animator.remove_layer(i);
                self.keys.remove(i);
                self.handles.remove(i);
                self.fading.remove(i);
                self.ends.remove(i);
                Self::shift(loco, i + 1, false);
            } else {
                i += 1;
            }
        }
    }

    /// The ramp-out of a clip by handle (kept by the caller with the request).
    #[must_use]
    pub fn holds(&self, handle: u64) -> bool {
        self.handles.contains(&Some(handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_anim::Skeleton;
    use tessera_havok::Transform;
    use tessera_havok::testkit::{
        AnimationKindSpec, AnimationSpec, ClipRecordSpec, InterleavedClipSpec,
    };

    fn skeleton() -> Arc<Skeleton> {
        Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap())
    }

    fn clip(s: &Arc<Skeleton>, seconds: f32) -> Arc<BoundClip> {
        let spec = InterleavedClipSpec {
            duration: seconds,
            tracks: 1,
            transforms: vec![Transform::IDENTITY; 2],
        };
        let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec)))
            .to_bytes();
        let c = tessera_havok::Clip::parse(&rec).unwrap();
        Arc::new(BoundClip::bind(Arc::new(c), Arc::clone(s)).unwrap())
    }

    fn node(layer: u32) -> AnimationNode {
        AnimationNode {
            layer,
            loop_count: 1,
            ramp_in: 0.2,
            ramp_out: 0.2,
            speed: 1.0,
            speed_multiplier: 1.0,
            end_time: 1.0,
            ..AnimationNode::default()
        }
    }

    fn loco() -> Locomotion {
        Locomotion {
            walk_speed: 1.0,
            run_speed: 4.0,
            base_rate: 1.0,
            moving: false,
            has_move: true,
            move_index: 1,
            above: vec![2],
        }
    }

    #[test]
    fn one_shot_clips_end_their_node_at_the_fade_out() {
        let s = skeleton();
        let r = ClipRequest {
            handle: 1,
            clip: clip(&s, 1.2),
            node: node(40),
            intensity: 1.0,
            start: 0.0,
        };
        let ClipRun::For(t) = r.run() else {
            panic!("{:?}", r.run())
        };
        assert!((t - 1.0).abs() < 0.05, "1.2 s clip, 0.2 s ramp-out: {t}");
        // Twice the rate: half as long; a start fraction shortens it; looping never ends.
        let fast = ClipRequest {
            node: AnimationNode {
                speed: 2.0,
                ..node(40)
            },
            ..r.clone()
        };
        let ClipRun::For(tf) = fast.run() else {
            panic!()
        };
        assert!((tf - 0.4).abs() < 0.05, "{tf}");
        let late = ClipRequest {
            start: 0.99,
            ..r.clone()
        };
        assert!(matches!(late.run(), ClipRun::For(x) if x == 0.0));
        let looping = ClipRequest {
            node: AnimationNode {
                loop_forever: true,
                ..node(40)
            },
            ..r.clone()
        };
        assert_eq!(looping.run(), ClipRun::UntilStopped);
        let high = ClipRequest {
            node: node(300),
            ..r.clone()
        };
        assert_eq!(high.run(), ClipRun::Refused);
        let nan = ClipRequest {
            node: AnimationNode {
                speed: f32::NAN,
                ..node(40)
            },
            ..r
        };
        assert_eq!(nan.run(), ClipRun::UntilStopped, "rate 0 holds");
    }

    /// §6.10 step 5: the initial time is *f*₀ × duration / rate, not multiplied by the rate.
    #[test]
    fn the_initial_time_follows_the_start_fraction_over_the_rate() {
        let s = skeleton();
        let r = ClipRequest {
            handle: 1,
            clip: clip(&s, 2.0),
            node: AnimationNode {
                speed: 2.0,
                ..node(40)
            },
            intensity: 1.0,
            start: 0.5,
        };
        assert!((r.initial_time(2.0) - 0.5).abs() < 1e-6);
        assert!((r.playback().time - 0.5).abs() < 1e-6);
        let slow = ClipRequest {
            node: AnimationNode {
                speed: 0.25,
                ..node(40)
            },
            ..r.clone()
        };
        assert_eq!(slow.initial_time(2.0), 2.0, "clamped to the duration");
        let held = ClipRequest {
            node: AnimationNode {
                speed: 0.0,
                ..node(40)
            },
            ..r
        };
        assert_eq!(held.initial_time(2.0), 0.0);
    }

    /// §6.10 steps 4 and 8: a one-shot clip fades out at its scheduled stop and its layer is
    /// removed without a stop from its node; a looping clip stays.
    #[test]
    fn one_shot_clip_layers_are_removed_after_their_duration() {
        let s = skeleton();
        let mut a = Animator::new(Arc::clone(&s));
        a.play(0, Playback::new(clip(&s, 1.0), Wrap::Loop), 0.0, false)
            .unwrap();
        let mut c = ScriptClips::new(vec![10]);
        let mut l = Locomotion {
            move_index: 0,
            above: Vec::new(),
            ..loco()
        };
        let once = ClipRequest {
            handle: 1,
            clip: clip(&s, 1.2),
            node: node(40),
            intensity: 1.0,
            start: 0.0,
        };
        let looping = ClipRequest {
            handle: 2,
            node: AnimationNode {
                loop_forever: true,
                ..node(40)
            },
            ..once.clone()
        };
        assert!(c.play(&mut a, &mut l, &once));
        assert!(c.play(&mut a, &mut l, &looping));
        let mut t = 0.0;
        while t < 1.5 {
            a.advance(0.05);
            c.tick(&mut a, &mut l, 0.05);
            t += 0.05;
            if t < 0.95 {
                assert!(c.holds(1), "still playing at {t}");
            }
        }
        assert!(!c.holds(1), "removed after 1.2 s");
        assert!(c.holds(2), "a looping clip waits for its stop");
        assert_eq!(c.keys.len(), a.layers().len());
        assert_eq!(c.ends.len(), a.layers().len());
    }

    /// §6.10 step 7: a driven change reaches the playing clip (rate at once, the
    /// `UseBoneOpacity` blend through the layer's mixing factor).
    #[test]
    fn driven_changes_reach_the_playing_clip() {
        let s = skeleton();
        let mut a = Animator::new(Arc::clone(&s));
        let mut c = ScriptClips::new(Vec::new());
        let mut l = Locomotion {
            move_index: 0,
            above: Vec::new(),
            ..loco()
        };
        let req = ClipRequest {
            handle: 5,
            clip: clip(&s, 1.0),
            node: AnimationNode {
                use_bone_opacity: true,
                ..node(40)
            },
            intensity: 1.0,
            start: 0.0,
        };
        assert!(c.play(&mut a, &mut l, &req));
        let moved = AnimationNode {
            speed: 2.0,
            use_bone_opacity: false,
            ..req.node
        };
        assert!(c.update(&mut a, 5, &moved));
        let layer = &a.layers()[0];
        let Some(Source::Clip(p)) = &layer.source else {
            panic!("a clip layer")
        };
        assert_eq!(p.rate, 2.0);
        assert_eq!(
            layer.opacity_mix(),
            0.0,
            "the blend starts from the opacities"
        );
        a.advance(0.2);
        assert_eq!(a.layers()[0].opacity_mix(), 1.0);
        assert!(!c.update(&mut a, 6, &moved));
    }

    #[test]
    fn script_clips_take_their_layer_order_and_shift_locomotion() {
        let s = skeleton();
        let mut a = Animator::new(Arc::clone(&s));
        for i in 0..3 {
            a.play(i, Playback::new(clip(&s, 1.0), Wrap::Loop), 0.0, false)
                .unwrap();
        }
        // Base layers: idle 10, movement 20, idle emote 30 (above the movement layer).
        let mut c = ScriptClips::new(vec![10, 20, 30]);
        let mut l = loco();
        let req = |h, layer| ClipRequest {
            handle: h,
            clip: clip(&s, 1.0),
            node: node(layer),
            intensity: 1.0,
            start: 0.0,
        };
        assert!(c.play(&mut a, &mut l, &req(1, 40)));
        assert_eq!(c.keys, vec![10, 20, 30, 40]);
        assert!(c.play(&mut a, &mut l, &req(2, 15)));
        assert_eq!(c.keys, vec![10, 15, 20, 30, 40]);
        assert_eq!((l.move_index, l.above.clone()), (2, vec![3]));
        assert!(c.play(&mut a, &mut l, &req(3, 40)));
        assert_eq!(
            c.handles.last(),
            Some(&Some(3)),
            "the newer clip goes above"
        );
        // A stop with no ramp-out removes the layer at once and shifts back.
        assert!(c.stop(&mut a, &mut l, 2, 0.0));
        assert_eq!(c.keys, vec![10, 20, 30, 40, 40]);
        assert_eq!((l.move_index, l.above.clone()), (1, vec![2]));
        // With a ramp-out it fades first (once its ramp-in has brought it up).
        a.advance(0.3);
        assert!(c.stop(&mut a, &mut l, 1, 0.2));
        assert!(c.holds(1));
        a.advance(0.25);
        c.sweep(&mut a, &mut l);
        assert!(!c.holds(1));
        assert_eq!(c.keys.len(), a.layers().len());
        assert!(!c.stop(&mut a, &mut l, 99, 0.0));
        // A full animator refuses.
        while a.layers().len() < Animator::MAX_LAYERS {
            let n = a.layers().len();
            a.insert_layer(n).unwrap();
            c.keys.push(255);
            c.handles.push(None);
            c.fading.push(false);
            c.ends.push(None);
        }
        assert!(!c.play(&mut a, &mut l, &req(9, 40)));
    }
}
