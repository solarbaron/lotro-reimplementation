// SPDX-License-Identifier: GPL-3.0-or-later
//! Idle, walk and run from a character's ground speed (`walker.md` §10 Decision).
//!
//! Each animated character plays these layers of a [`tessera_anim::Animator`], ordered by
//! the data's layer keys (`script-table.md` §6.2: higher layers override lower ones bone
//! by bone, weighted by their per-track weights where the node asks for them):
//!
//! - the looping idle clips the script tables chose, one per data layer;
//! - the locomotion layer (at the walk clip's data layer): the walk and run clips as a
//!   phase-synchronised speed blend with the clips' natural ground speeds as anchors (one
//!   clip when only one is known). Its weight fades to 1 over [`FADE`] seconds when the
//!   character moves faster than [`START_SPEED`] and back to 0 below [`STOP_SPEED`], so the
//!   idle shows through when standing; idle layers above it fade out while moving. The data
//!   switches walk and run as separate branches with a 0.2 s cross-fade
//!   (`script-table.md` §8.5); blending them by speed is the smooth equivalent for
//!   characters whose speed varies continuously.
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
}

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
    // Animator layer order: the data's layer keys (idle layers, then locomotion on ties).
    let move_key = moves.first().map(|m| m.layer);
    let mut order: Vec<(u32, Option<usize>)> = clips
        .idle
        .iter()
        .enumerate()
        .map(|(i, c)| (c.layer, Some(i)))
        .collect();
    if let Some(k) = move_key {
        order.push((k, None));
    }
    order.sort_by_key(|(k, i)| (*k, i.is_none()));
    let mut loco = Locomotion {
        walk_speed: 0.0,
        run_speed: 0.0,
        base_rate: 1.0,
        moving: false,
        has_move: false,
        move_index: order.iter().position(|(_, i)| i.is_none()).unwrap_or(0),
        above: Vec::new(),
    };
    for (index, (key, idle)) in order.iter().enumerate() {
        let Some(i) = idle else { continue };
        let idle = &clips.idle[*i];
        let mut p = Playback::new(Arc::clone(&idle.clip), Wrap::Loop);
        p.rate = idle.rate;
        if a.play(index, p, 0.0, false).is_ok() {
            any = true;
            if let Some(l) = a.layer_mut(index) {
                l.use_track_weights = idle.track_weights;
            }
            if move_key.is_some_and(|m| *key > m) {
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
        let _ = a.advance(phase.min(600.0));
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
    let _ = a.advance(if dt.is_finite() {
        dt.clamp(0.0, 1.0)
    } else {
        0.0
    });
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
        }
    }

    #[test]
    fn animator_layers_follow_the_data_layer_order() {
        let skel = skeleton();
        let clips = FigureClips {
            idle: vec![play(&skel, 10, 0.0), play(&skel, 30, 0.0)],
            walk: Some(play(&skel, 20, 2.0)),
            run: Some(play(&skel, 20, 6.0)),
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
        };
        let (_, loco) = make_animator(&skel, &clips, f32::NAN).expect("animator");
        assert!(!loco.has_move);

        assert!(make_animator(&skel, &FigureClips::default(), 0.0).is_none());
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
