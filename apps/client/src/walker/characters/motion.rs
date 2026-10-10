// SPDX-License-Identifier: GPL-3.0-or-later
//! Idle, walk and run from a character's ground speed (`walker.md` §10 Decision).
//!
//! Each animated character plays these layers of a [`tessera_anim::Animator`], ordered by
//! the data's layer keys (`script-table.md` §6.2: higher layers override lower ones bone
//! by bone, weighted by their per-track weights where the node asks for them):
//!
//! - the looping idle clips the script tables chose, one per data layer;
//! - on a character-screen figure, its one-shot stage clips: each on its own layer from
//!   its start time, faded out over its `RampOut` ending at its end, then removed
//!   (`script-table.md` §6.10 steps 3–5 and 8, `character-screens.md` §2.1.4);
//! - the held proportion clips (`script-table.md` §6.7, §6.8: the River-hobbit and
//!   DwarfStoutAxe layer-110 `scale_blend` clips), shown at their held fraction with rate 0
//!   and never faded: they shape the figure in every motion state. Held clips of one data
//!   layer play together on one animator layer and share its budget (§6.2), each with its
//!   own track opacity;
//! - the locomotion layer (at the walk clip's data layer): the walk and run clips as a
//!   phase-synchronised speed blend with the clips' natural ground speeds as anchors (one
//!   clip when only one is known). Its weight fades to 1 over [`FADE`] seconds when the
//!   character moves faster than [`START_SPEED`] and back to 0 below [`STOP_SPEED`], so the
//!   idle shows through when standing; idle layers above it fade out while moving. The data
//!   switches walk and run as separate branches with a 0.2 s cross-fade
//!   (`script-table.md` §8.5); blending them by speed is the smooth equivalent for
//!   characters whose speed varies continuously.
//!
//! Layers combine by the layered-weight rule (`skeleton-anim.md` §6.6), applied by
//! [`tessera_anim::evaluate`].
//!
//! The blend's playback rate is scaled by speed / natural speed (the clip's anchors
//! clamp the natural speed), within [`MIN_RATE`]..[`MAX_RATE`], so the feet match the
//! ground (`script-table.md` §8.5 OPEN on the original's rule). Locomotion clips are
//! treadmill clips (`skeleton-anim.md` §7.2): they are played in place and the movement
//! comes from the controller or the network, never from root motion.

use std::sync::Arc;

use tessera_anim::{Animator, Playback, Skeleton, Source, SpeedBlend, Wrap};

use super::figure::FigureClips;

/// Speed (m/s) above which a standing character starts moving.
pub const START_SPEED: f32 = 0.3;
/// Speed (m/s) below which a moving character stops.
pub const STOP_SPEED: f32 = 0.15;
/// Fade between standing and moving, seconds (the data's walk/run cross-fade).
pub const FADE: f32 = 0.2;
/// Slowest locomotion playback rate.
pub const MIN_RATE: f32 = 0.3;
/// Fastest locomotion playback rate.
pub const MAX_RATE: f32 = 2.5;

/// Per-character locomotion state.
#[derive(Debug, Clone, PartialEq)]
pub struct Locomotion {
    /// Natural speed of the slowest locomotion clip (m/s).
    pub walk_speed: f32,
    /// Natural speed of the fastest locomotion clip (m/s).
    pub run_speed: f32,
    /// Base playback rate of the locomotion clips (from the script nodes).
    pub base_rate: f32,
    /// Whether the locomotion layer is (fading) in.
    pub moving: bool,
    /// Whether there is a locomotion layer at all.
    pub has_move: bool,
    /// Animator index of the locomotion layer.
    pub move_index: usize,
    /// Animator indices of the idle layers above the locomotion layer (faded out while
    /// moving).
    pub above: Vec<usize>,
    /// The one-shot clips still playing ([`OnceLayer`]).
    pub once: Vec<OnceLayer>,
}

/// A one-shot clip on its own animator layer (`script-table.md` §6.10 steps 4 and 8): it
/// fades out over its `RampOut` ending at its end and is then removed from the layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnceLayer {
    /// Animator layer index.
    pub index: usize,
    /// Seconds until its end.
    pub remaining: f32,
    /// Fade-out seconds ending at the end.
    pub ramp_out: f32,
    /// The fade-out has begun.
    pub fading: bool,
}

/// Advances the one-shot clips of `loco` by `dt` seconds: a clip whose fade-out time has
/// come starts fading (over the time left), and a clip at its end is removed (its layer
/// plays nothing). Call before the animator advances by the same `dt`.
fn tick_once(a: &mut Animator, loco: &mut Locomotion, dt: f32) {
    let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
    loco.once.retain_mut(|o| {
        o.remaining -= dt;
        let Some(layer) = a.layer_mut(o.index) else {
            return false;
        };
        if o.remaining <= 0.0 {
            layer.source = None;
            return false;
        }
        if !o.fading && o.ramp_out >= MIN_RAMP && o.remaining <= o.ramp_out {
            layer.fade_to(0.0, o.remaining);
            o.fading = true;
        }
        true
    });
}

/// Ramps shorter than this end at once (`script-table.md` §6.10 step 8).
const MIN_RAMP: f32 = 0.0002;

impl Locomotion {
    /// The natural speed the blend shows at ground speed `speed`.
    #[must_use]
    pub fn natural_at(&self, speed: f32) -> f32 {
        if speed.is_finite() {
            speed.clamp(self.walk_speed, self.run_speed.max(self.walk_speed))
        } else {
            self.walk_speed
        }
    }

    /// The locomotion playback rate at ground speed `speed`.
    #[must_use]
    pub fn rate_at(&self, speed: f32) -> f32 {
        let n = self.natural_at(speed);
        let r = if n > 0.0 && speed.is_finite() {
            speed / n
        } else {
            1.0
        };
        (r * self.base_rate).clamp(MIN_RATE, MAX_RATE)
    }

    /// Whether a character at `speed` moves (with hysteresis).
    #[must_use]
    pub fn wants_move(&self, speed: f32) -> bool {
        if !speed.is_finite() {
            return false;
        }
        if self.moving {
            speed > STOP_SPEED
        } else {
            speed > START_SPEED
        }
    }
}

/// An animator for `skeleton` playing `clips`, advanced by `phase` seconds so characters
/// do not move in step. `None` when there is no clip at all.
#[must_use]
pub fn make_animator(
    skeleton: &Arc<Skeleton>,
    clips: &FigureClips,
    phase: f32,
) -> Option<(Animator, Locomotion)> {
    let mut a = Animator::new(Arc::clone(skeleton));
    let mut any = false;
    let moves: Vec<_> = [&clips.walk, &clips.run]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    // Animator layer order: the data's layer keys (idle and held layers, then locomotion
    // on ties).
    #[derive(Clone, Copy, PartialEq)]
    enum Entry {
        Idle(usize),
        Held(usize),
        Move,
    }
    let move_key = moves.first().map(|m| m.layer);
    let mut order: Vec<(u32, Entry)> = clips
        .idle
        .iter()
        .enumerate()
        .map(|(i, c)| (c.layer, Entry::Idle(i)))
        .chain(
            clips
                .held
                .iter()
                .enumerate()
                .map(|(i, c)| (c.layer, Entry::Held(i))),
        )
        .collect();
    if let Some(k) = move_key {
        order.push((k, Entry::Move));
    }
    order.sort_by_key(|(k, e)| (*k, *e == Entry::Move));
    // Animator layer of each entry: held clips of one data layer (adjacent after the sort,
    // idle entries of the same key come first) share one animator layer.
    let mut slots: Vec<(usize, bool)> = Vec::with_capacity(order.len());
    let mut next = 0;
    let mut last_held: Option<(u32, usize)> = None;
    for (key, entry) in &order {
        if matches!(entry, Entry::Held(_))
            && let Some((k, i)) = last_held
            && k == *key
        {
            slots.push((i, true));
            continue;
        }
        last_held = matches!(entry, Entry::Held(_)).then_some((*key, next));
        slots.push((next, false));
        next += 1;
    }
    let mut loco = Locomotion {
        walk_speed: 0.0,
        run_speed: 0.0,
        base_rate: 1.0,
        moving: false,
        has_move: false,
        move_index: order
            .iter()
            .zip(&slots)
            .find(|((_, e), _)| *e == Entry::Move)
            .map_or(0, |(_, (i, _))| *i),
        above: Vec::new(),
        once: Vec::new(),
    };
    for ((key, entry), &(index, shared)) in order.iter().zip(&slots) {
        let started = match *entry {
            Entry::Idle(i) => {
                let idle = &clips.idle[i];
                let wrap = match idle.once {
                    Some(o) if !o.wraps => Wrap::Once,
                    _ => Wrap::Loop,
                };
                let mut p = Playback::new(Arc::clone(&idle.clip), wrap);
                p.rate = idle.rate;
                if let Some(o) = idle.once {
                    p.time = o.start;
                }
                let ok = a.play(index, p, 0.0, false).is_ok();
                if ok && let Some(l) = a.layer_mut(index) {
                    l.use_track_weights = idle.track_weights;
                }
                if ok && let Some(o) = idle.once {
                    loco.once.push(OnceLayer {
                        index,
                        remaining: o.length,
                        ramp_out: o.ramp_out,
                        fading: false,
                    });
                }
                ok
            }
            // Held (script-table.md §6.7): rate 0 at the held fraction, whatever the time,
            // with its own node's track opacity; a further held clip of the same data layer
            // starts beside the first without stopping it (§6.2).
            Entry::Held(i) => {
                let held = &clips.held[i];
                let mut p = Playback::held(Arc::clone(&held.clip), held.held.unwrap_or(0.0));
                p.use_track_weights = held.track_weights;
                if shared {
                    a.add(index, p).is_ok()
                } else {
                    a.play(index, p, 0.0, false).is_ok()
                }
            }
            Entry::Move => continue,
        };
        if started {
            any = true;
            if let Entry::Idle(i) = entry
                && clips.idle[*i].once.is_none()
                && move_key.is_some_and(|m| *key > m)
            {
                loco.above.push(index);
            }
        }
    }
    if let (Some(first), Some(last)) = (moves.first(), moves.last()) {
        loco.walk_speed = first.speed;
        loco.run_speed = last.speed.max(first.speed);
        loco.base_rate = first.rate;
        let source = if moves.len() >= 2 {
            SpeedBlend::new(
                moves
                    .iter()
                    .map(|m| (m.speed, Arc::clone(&m.clip)))
                    .collect(),
            )
            .ok()
            .map(Source::Speed)
        } else {
            None
        };
        let source = source.unwrap_or_else(|| {
            let mut p = Playback::new(Arc::clone(&first.clip), Wrap::Loop);
            p.rate = first.rate;
            Source::Clip(p)
        });
        if a.set_source(loco.move_index, source).is_ok() {
            if let Some(l) = a.layer_mut(loco.move_index) {
                l.fade_to(0.0, 0.0);
                l.use_track_weights = first.track_weights;
            }
            loco.has_move = true;
            any = true;
        }
    }
    if !any {
        return None;
    }
    if phase.is_finite() && phase > 0.0 {
        let phase = phase.min(600.0);
        tick_once(&mut a, &mut loco, phase);
        let _ = a.advance(phase);
    }
    Some((a, loco))
}

/// Sets the layers for ground speed `speed` and advances by `dt` seconds.
pub fn drive(a: &mut Animator, loco: &mut Locomotion, speed: f32, dt: f32) {
    let speed = if speed.is_finite() {
        speed.max(0.0)
    } else {
        0.0
    };
    if loco.has_move {
        let moving = loco.wants_move(speed);
        if moving != loco.moving {
            for &i in &loco.above {
                if let Some(l) = a.layer_mut(i) {
                    l.fade_to(if moving { 0.0 } else { 1.0 }, FADE);
                }
            }
        }
        if let Some(layer) = a.layer_mut(loco.move_index) {
            if moving != loco.moving {
                layer.fade_to(if moving { 1.0 } else { 0.0 }, FADE);
                loco.moving = moving;
            }
            // While standing the blend keeps its walking end, so it fades in walking.
            let p = if moving { speed } else { loco.walk_speed };
            let rate = loco.rate_at(p);
            match &mut layer.source {
                Some(Source::Speed(b)) => {
                    b.set_param(p);
                    b.rate = rate;
                }
                Some(Source::Clip(c)) => c.rate = rate,
                _ => {}
            }
        }
    }
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 1.0)
    } else {
        0.0
    };
    tick_once(a, loco, dt);
    let _ = a.advance(dt);
}

#[cfg(test)]
mod tests {
    use tessera_anim::BoundClip;
    use tessera_havok::testkit::{
        AnimationKindSpec, AnimationSpec, BoneSpec, ClipRecordSpec, InterleavedClipSpec,
        SkeletonSpec,
    };
    use tessera_havok::{Clip, Transform};

    use super::super::figure::ClipPlay;
    use super::*;

    const BONES: usize = 3;

    fn skeleton() -> Arc<Skeleton> {
        let reference = Transform {
            translation: [0.0, 0.1, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        };
        let spec = SkeletonSpec {
            bones: (0..BONES)
                .map(|i| BoneSpec {
                    name: Some(format!("bone{i}")),
                    parent: i as i32 - 1,
                    reference,
                    lock_translation: false,
                })
                .collect(),
            overrides: Vec::new(),
        };
        Arc::new(Skeleton::from_setup_record(&spec.to_setup_prefix(0x0400_0001)).unwrap())
    }

    fn play(skel: &Arc<Skeleton>, layer: u32, speed: f32) -> ClipPlay {
        let t = Transform {
            translation: [0.0, 0.1, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        };
        let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
            InterleavedClipSpec {
                duration: 1.0,
                tracks: BONES,
                transforms: vec![t; 2 * BONES],
            },
        )));
        let clip = Clip::parse(&rec.to_bytes()).unwrap();
        ClipPlay {
            did: 0x0300_0000 | layer,
            clip: Arc::new(BoundClip::bind(Arc::new(clip), Arc::clone(skel)).unwrap()),
            rate: 1.0,
            speed,
            layer,
            track_weights: layer == 30,
            held: None,
            once: None,
        }
    }

    #[test]
    fn animator_layers_follow_the_data_layer_order() {
        let skel = skeleton();
        let clips = FigureClips {
            idle: vec![play(&skel, 10, 0.0), play(&skel, 30, 0.0)],
            walk: Some(play(&skel, 20, 2.0)),
            run: Some(play(&skel, 20, 6.0)),
            held: Vec::new(),
        };
        let (a, loco) = make_animator(&skel, &clips, 0.5).expect("animator");
        // Idle 10, locomotion 20, idle 30 (faded out while moving).
        assert_eq!(loco.move_index, 1);
        assert_eq!(loco.above, vec![2]);
        assert!(loco.has_move && !loco.moving);
        assert_eq!((loco.walk_speed, loco.run_speed), (2.0, 6.0));
        let layers = a.layers();
        assert!(matches!(layers[1].source, Some(Source::Speed(_))));
        assert_eq!(layers[1].weight, 0.0, "locomotion starts faded out");
        assert!(matches!(layers[0].source, Some(Source::Clip(_))));
        assert!(layers[2].use_track_weights && !layers[0].use_track_weights);

        // An idle on the locomotion's layer goes below it; one clip is not a blend.
        let clips = FigureClips {
            idle: vec![play(&skel, 20, 0.0)],
            walk: Some(play(&skel, 20, 2.0)),
            run: None,
            held: Vec::new(),
        };
        let (a, loco) = make_animator(&skel, &clips, 0.0).expect("animator");
        assert_eq!(loco.move_index, 1);
        assert!(loco.above.is_empty());
        assert_eq!((loco.walk_speed, loco.run_speed), (2.0, 2.0));
        assert!(matches!(a.layers()[1].source, Some(Source::Clip(_))));

        // Idle only: no locomotion layer.
        let clips = FigureClips {
            idle: vec![play(&skel, 10, 0.0)],
            walk: None,
            run: None,
            held: Vec::new(),
        };
        let (_, loco) = make_animator(&skel, &clips, f32::NAN).expect("animator");
        assert!(!loco.has_move);

        assert!(make_animator(&skel, &FigureClips::default(), 0.0).is_none());
    }

    /// `script-table.md` §6.7: a held clip sits above locomotion at its fraction, rate 0,
    /// and is never faded out while moving.
    #[test]
    fn held_clips_are_held_and_never_faded() {
        let skel = skeleton();
        let mut held = play(&skel, 110, 0.0);
        held.held = Some(0.25);
        held.track_weights = true;
        let clips = FigureClips {
            idle: vec![play(&skel, 10, 0.0)],
            walk: Some(play(&skel, 20, 2.0)),
            run: Some(play(&skel, 20, 6.0)),
            held: vec![held],
        };
        let (mut a, mut loco) = make_animator(&skel, &clips, 3.7).expect("animator");
        assert_eq!(loco.move_index, 1);
        assert!(loco.above.is_empty(), "held layers are not faded");
        let time = |a: &Animator| match &a.layers()[2].source {
            Some(Source::Clip(p)) => (p.rate, p.clip_time()),
            _ => panic!("held layer"),
        };
        assert_eq!(time(&a), (0.0, 0.25));
        assert!(matches!(&a.layers()[2].source, Some(Source::Clip(p)) if p.use_track_weights));
        for speed in [0.0, 5.0, 5.0, 0.0] {
            drive(&mut a, &mut loco, speed, 0.4);
            assert_eq!(time(&a), (0.0, 0.25));
            assert_eq!(a.layers()[2].weight, 1.0);
        }
    }

    /// `script-table.md` §6.2: two held clips of one data layer share one animator layer
    /// (and its budget), each with its own track opacity; the next layer follows.
    #[test]
    fn held_clips_of_one_layer_share_an_animator_layer() {
        let skel = skeleton();
        let mut first = play(&skel, 110, 0.0);
        first.held = Some(0.25);
        first.track_weights = true;
        let mut second = play(&skel, 110, 0.0);
        second.held = Some(0.5);
        let clips = FigureClips {
            idle: vec![play(&skel, 10, 0.0)],
            walk: Some(play(&skel, 20, 2.0)),
            run: None,
            held: vec![first, second],
        };
        let (a, loco) = make_animator(&skel, &clips, 0.0).expect("animator");
        assert_eq!(loco.move_index, 1);
        assert_eq!(a.layers().len(), 3);
        match &a.layers()[2].source {
            Some(Source::Group(g)) => {
                let v: Vec<(f32, bool, f32)> = g
                    .iter()
                    .map(|(p, w)| (p.clip_time(), p.use_track_weights, *w))
                    .collect();
                assert_eq!(v, vec![(0.25, true, 1.0), (0.5, false, 1.0)]);
            }
            other => panic!("{other:?}"),
        }
        assert!(!a.layers()[2].use_track_weights);
    }

    /// `script-table.md` §6.10 steps 4 and 8: a one-shot clip plays from its start, fades
    /// out over its ramp ending at its end and is then removed; the looping layer stays.
    #[test]
    fn one_shot_clips_play_once_and_are_removed() {
        use super::super::figure::OneShot;
        let skel = skeleton();
        let mut gesture = play(&skel, 90, 0.0);
        gesture.once = Some(OneShot {
            start: 0.25,
            length: 1.0,
            ramp_out: 0.4,
            wraps: false,
        });
        let clips = FigureClips {
            idle: vec![play(&skel, 80, 0.0), gesture],
            walk: None,
            run: None,
            held: Vec::new(),
        };
        let (mut a, mut loco) = make_animator(&skel, &clips, 0.0).expect("animator");
        assert_eq!(loco.once.len(), 1);
        let time = |a: &Animator| match &a.layers()[1].source {
            Some(Source::Clip(p)) => Some(p.clip_time()),
            _ => None,
        };
        assert_eq!(time(&a), Some(0.25), "starts at its start time");
        drive(&mut a, &mut loco, 0.0, 0.5);
        assert_eq!(a.layers()[1].weight, 1.0, "not fading yet");
        drive(&mut a, &mut loco, 0.0, 0.2);
        assert!(loco.once[0].fading, "the fade-out began");
        drive(&mut a, &mut loco, 0.0, 0.2);
        assert!(a.layers()[1].weight < 0.6);
        drive(&mut a, &mut loco, 0.0, 0.2);
        assert!(time(&a).is_none(), "removed at its end");
        assert!(loco.once.is_empty());
        assert!(matches!(a.layers()[0].source, Some(Source::Clip(_))));
    }

    fn loco() -> Locomotion {
        Locomotion {
            walk_speed: 2.0,
            run_speed: 6.0,
            base_rate: 1.0,
            moving: false,
            has_move: true,
            move_index: 1,
            above: Vec::new(),
            once: Vec::new(),
        }
    }

    #[test]
    fn rates_match_the_ground() {
        let l = loco();
        assert_eq!(l.natural_at(1.0), 2.0);
        assert_eq!(l.natural_at(4.0), 4.0);
        assert_eq!(l.natural_at(9.0), 6.0);
        assert!((l.rate_at(1.0) - 0.5).abs() < 1e-6);
        assert!((l.rate_at(4.0) - 1.0).abs() < 1e-6);
        assert!((l.rate_at(9.0) - 1.5).abs() < 1e-6);
        assert_eq!(l.rate_at(0.0), MIN_RATE);
        assert_eq!(l.rate_at(100.0), MAX_RATE);
        assert_eq!(l.rate_at(f32::NAN), 1.0_f32.clamp(MIN_RATE, MAX_RATE));
    }

    #[test]
    fn moving_has_hysteresis() {
        let mut l = loco();
        assert!(!l.wants_move(0.2));
        assert!(l.wants_move(0.31));
        l.moving = true;
        assert!(l.wants_move(0.2));
        assert!(!l.wants_move(0.1));
        assert!(!l.wants_move(f32::NAN));
    }
}
