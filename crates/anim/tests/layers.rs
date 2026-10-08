// SPDX-License-Identifier: GPL-3.0-or-later
//! The layered-weight rule and held clips on synthetic data: `skeleton-anim.md` §6.6,
//! §14.6, §14.8 and `script-table.md` §6.7, §13.2.

mod common;

use std::sync::Arc;

use common::*;
use glam::Mat4;
use tessera_anim::{Animator, EvalScratch, Playback, Skeleton, Wrap, evaluate};
use tessera_havok::Transform;

fn at_z(z: f32) -> Transform {
    Transform {
        translation: [0.0, 0.0, z],
        ..Transform::IDENTITY
    }
}

fn eval(anim: &Animator) -> Vec<Transform> {
    let s = anim.skeleton();
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    let mut out = s.reference_pose();
    evaluate(s, &snap, &mut EvalScratch::new(), &mut out).unwrap();
    out
}

/// `skeleton-anim.md` §14.8: three layers on one bone.
#[test]
fn three_layers_example_14_8() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let clip = |z: f32, w: Option<f32>| {
        let mut rec = interleaved_clip(2, 1, 1.0, |_, _| at_z(z));
        if let Some(w) = w {
            rec.weights = vec![w];
        }
        bind(&rec, &s)
    };
    let mut anim = Animator::new(Arc::clone(&s));
    // Animator layers in priority order: data layers 10, 18, 110.
    for (i, (z, w)) in [(0.0, None), (5.0, Some(0.0)), (-10.0, Some(0.01))]
        .into_iter()
        .enumerate()
    {
        anim.play(i, Playback::new(clip(z, w), Wrap::Loop), 0.0, false)
            .unwrap();
        anim.layer_mut(i).unwrap().use_track_weights = w.is_some();
    }
    let z = eval(&anim)[0].translation[2];
    assert!((z - (-0.07874)).abs() < 1e-5, "{z}");
}

/// `skeleton-anim.md` §14.6 with no base: the reference pose fills the weak track.
#[test]
fn lone_partial_clip_is_filled_with_the_reference_pose() {
    let s = Arc::new(Skeleton::from_parts(&[-1, 0], &[Transform::IDENTITY; 2]).unwrap());
    let mut rec = interleaved_clip(2, 2, 1.0, |_, _| at_z(1.0));
    rec.weights = vec![1.0, 0.01];
    let mut anim = Animator::new(Arc::clone(&s));
    anim.play(0, Playback::new(bind(&rec, &s), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.layer_mut(0).unwrap().use_track_weights = true;
    let out = eval(&anim);
    assert!((out[0].translation[2] - 1.0).abs() < 1e-6);
    // 2/255 of the clip, 0.1 − 2/255 of the reference: 0.0784 of the way.
    let share = (2.0 / 255.0) / 0.1;
    assert!((out[1].translation[2] - share).abs() < 1e-5, "{out:?}");
}

/// `script-table.md` §13.2: a held proportion clip keeps the foot on the ground for every
/// value of its driving property and never changes with elapsed time.
#[test]
fn held_proportion_clip_example_13_2() {
    let s = Arc::new(Skeleton::from_parts(&[-1, 0], &[at_z(0.0), at_z(-0.5)]).unwrap());
    let idle = bind(
        &interleaved_clip(2, 2, 1.0, |_, j| s.reference_local()[j]),
        &s,
    );
    // 2.0 s, three frames (linear in time, so interpolation is exact).
    let mut rec = interleaved_clip(3, 2, 2.0, |f, j| {
        let t = f as f32; // seconds: frames at 0, 1, 2
        if j == 0 {
            at_z(-10.0 + 10.0 * (t / 2.0))
        } else {
            at_z(-0.5 + 10.0 - 10.0 * (t / 2.0))
        }
    });
    rec.weights = vec![0.01, 0.01];
    let held = bind(&rec, &s);
    let pose_at = |fraction: f32, elapsed: f32| {
        let mut anim = Animator::new(Arc::clone(&s));
        anim.play(0, Playback::new(Arc::clone(&idle), Wrap::Loop), 0.0, false)
            .unwrap();
        anim.play(1, Playback::held(Arc::clone(&held), fraction), 0.0, false)
            .unwrap();
        anim.layer_mut(1).unwrap().use_track_weights = true;
        anim.advance(elapsed);
        let local = eval(&anim);
        let mut model = vec![Mat4::IDENTITY; 2];
        s.compose(&local, &mut model).unwrap();
        (local, model)
    };
    // (P, held fraction by the looping rule of §6.7, bone 0 z, bone 1 local z).
    for (p, fraction, b0, b1) in [
        (0.0f32, 0.0f32, -0.07874f32, -0.42126f32),
        (0.5, 0.5, -0.03937, -0.46063),
        (0.95, 0.95, -0.00394, -0.49606),
        (1.0, 0.0, -0.07874, -0.42126),
    ] {
        for elapsed in [0.0f32, 0.7, 1.9, 5.3] {
            let (local, model) = pose_at(fraction, elapsed);
            assert!(
                (local[0].translation[2] - b0).abs() < 1e-4,
                "P {p}: {local:?}"
            );
            assert!(
                (local[1].translation[2] - b1).abs() < 1e-4,
                "P {p}: {local:?}"
            );
            assert!(
                (model[1].w_axis.z + 0.5).abs() < 1e-4,
                "P {p}: foot on the ground"
            );
        }
    }
    // Played over time instead (rate 1, looping) the root moves: the wrong behaviour.
    let mut anim = Animator::new(Arc::clone(&s));
    anim.play(0, Playback::new(Arc::clone(&idle), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(1, Playback::new(Arc::clone(&held), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.layer_mut(1).unwrap().use_track_weights = true;
    let z0 = eval(&anim)[0].translation[2];
    anim.advance(1.0);
    let z1 = eval(&anim)[0].translation[2];
    assert!((z0 - z1).abs() > 0.03, "{z0} {z1}");
    // A held playback can be moved at once.
    let mut p = Playback::held(Arc::clone(&held), 0.25);
    assert_eq!(p.rate, 0.0);
    assert!((p.clip_time() - 0.5).abs() < 1e-6);
    p.set_held_fraction(1.0);
    assert!(
        (p.clip_time() - 2.0).abs() < 1e-6,
        "fraction 1 is the last pose"
    );
    p.advance(3.0);
    assert!((p.clip_time() - 2.0).abs() < 1e-6);
    p.set_held_fraction(f32::NAN);
    assert_eq!(p.clip_time(), 0.0);
}

/// One bone, clips that set its z to `z`, without track weights.
fn z_clip(s: &Arc<Skeleton>, z: f32) -> Arc<tessera_anim::BoundClip> {
    bind(&interleaved_clip(2, 1, 1.0, |_, _| at_z(z)), s)
}

/// The clips of layer 0 with their own overall weights.
fn clip_weights(anim: &Animator) -> Vec<f32> {
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    snap.first()
        .map(|l| l.clips().map(|c| c.weight).collect())
        .unwrap_or_default()
}

/// `skeleton-anim.md` §14.9: a Switch cross-fade with `RampOut` 0.2. At 0.05 s into it the
/// outgoing clip has weight 0.75 and the incoming one 0.25; at 0.2 s the outgoing clip is
/// removed. Both ramps are linear.
#[test]
fn switch_cross_fade_example_14_9() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut anim = Animator::new(Arc::clone(&s));
    anim.play(0, Playback::new(z_clip(&s, 0.0), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(0, Playback::new(z_clip(&s, 4.0), Wrap::Loop), 0.2, false)
        .unwrap();
    anim.advance(0.05);
    let w = clip_weights(&anim);
    assert!(
        (w[0] - 0.75).abs() < 1e-6 && (w[1] - 0.25).abs() < 1e-6,
        "{w:?}"
    );
    // Quantised 191/255 and 63/255 (§6.6 step 3).
    let z = eval(&anim)[0].translation[2];
    assert!((z - 4.0 * 63.0 / 254.0).abs() < 1e-5, "{z}");
    anim.advance(0.15);
    assert_eq!(clip_weights(&anim), vec![1.0]);
}

/// `skeleton-anim.md` §14.9, second paragraph, and `script-table.md` §7.1 step 5: the
/// Switch property changes back at 0.05 s into a 0.2 s cross-fade. The clip still fading
/// out (at 0.75) is stopped at once; the clip that was coming in (at 0.25) becomes the
/// outgoing one and continues from 0.25 down to 0, so the placed fade ends 0.25 × 0.2 =
/// 0.05 s later; the first case's clip is started again and rises as the complement, from
/// 0.75.
#[test]
fn interrupted_fade_example_14_9() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut anim = Animator::new(Arc::clone(&s));
    anim.play(0, Playback::new(z_clip(&s, 0.0), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(0, Playback::new(z_clip(&s, 4.0), Wrap::Loop), 0.2, false)
        .unwrap();
    anim.advance(0.05);
    anim.play(0, Playback::new(z_clip(&s, 0.0), Wrap::Loop), 0.2, false)
        .unwrap();
    let close = |w: &[f32], want: &[f32]| {
        w.len() == want.len() && w.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-5)
    };
    // Outgoing (z = 4) at 0.25, the restarted first case at 0.75: the old z = 0 clip that
    // was fading out at 0.75 is gone.
    let w = clip_weights(&anim);
    assert!(close(&w, &[0.25, 0.75]), "{w:?}");
    let z = eval(&anim)[0].translation[2];
    // Quantised 63/255 and 191/255 (§6.6 step 3).
    assert!((z - 4.0 * 63.0 / 254.0).abs() < 1e-5, "{z}");
    anim.advance(0.025);
    let w = clip_weights(&anim);
    assert!(close(&w, &[0.125, 0.875]), "{w:?}");
    anim.advance(0.025);
    assert_eq!(clip_weights(&anim), vec![1.0]);
    assert!(eval(&anim)[0].translation[2].abs() < 1e-6);
}

/// The same rule when the new fade is longer than the old one and when the interrupted
/// fade had several stopped clips: only the incoming clip is carried, over its weight ×
/// the new duration.
#[test]
fn interrupted_fade_carries_only_the_incoming_clip() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut anim = Animator::new(Arc::clone(&s));
    anim.add(0, Playback::new(z_clip(&s, 1.0), Wrap::Loop))
        .unwrap();
    anim.add(0, Playback::new(z_clip(&s, 2.0), Wrap::Loop))
        .unwrap();
    anim.play(0, Playback::new(z_clip(&s, 3.0), Wrap::Loop), 1.0, false)
        .unwrap();
    anim.advance(0.4);
    assert_eq!(clip_weights(&anim).len(), 3);
    anim.play(0, Playback::new(z_clip(&s, 5.0), Wrap::Loop), 2.0, false)
        .unwrap();
    let w = clip_weights(&anim);
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(
        (w[0] - 0.4).abs() < 1e-5 && (w[1] - 0.6).abs() < 1e-5,
        "{w:?}"
    );
    // The carried clip reaches 0 after 0.4 × 2 = 0.8 s.
    anim.advance(0.4);
    let w = clip_weights(&anim);
    assert!(
        (w[0] - 0.2).abs() < 1e-5 && (w[1] - 0.8).abs() < 1e-5,
        "{w:?}"
    );
    anim.advance(0.4);
    assert_eq!(clip_weights(&anim), vec![1.0]);
}

/// `script-table.md` §6.2: starting a clip never stops another clip of the same layer;
/// both play at full weight and share the layer's remaining budget, so a lower layer gets
/// nothing (`skeleton-anim.md` §6.6 step 2).
#[test]
fn clips_added_to_a_layer_share_its_budget() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut anim = Animator::new(Arc::clone(&s));
    anim.play(0, Playback::new(z_clip(&s, 9.0), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.add(1, Playback::new(z_clip(&s, 1.0), Wrap::Loop))
        .unwrap();
    anim.add(1, Playback::new(z_clip(&s, 3.0), Wrap::Loop))
        .unwrap();
    // Each clip of layer 1 gets c × r = 1; the layer then leaves nothing for layer 0.
    let z = eval(&anim)[0].translation[2];
    assert!((z - 2.0).abs() < 1e-6, "{z}");
    // A layer full of clips refuses one more and stays as it was.
    anim.add(1, Playback::new(z_clip(&s, 5.0), Wrap::Loop))
        .unwrap();
    anim.add(1, Playback::new(z_clip(&s, 7.0), Wrap::Loop))
        .unwrap();
    assert!(
        anim.add(1, Playback::new(z_clip(&s, 11.0), Wrap::Loop))
            .is_err()
    );
    let z = eval(&anim)[0].translation[2];
    assert!((z - 4.0).abs() < 1e-5, "{z}");
}

/// The opacity of a clip comes from its own node (`script-table.md` §6.5): two clips on
/// one layer, only one using its track weights.
#[test]
fn track_opacity_is_per_clip() {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut weighted = interleaved_clip(2, 1, 1.0, |_, _| at_z(10.0));
    weighted.weights = vec![0.01];
    let mut anim = Animator::new(Arc::clone(&s));
    let mut p = Playback::new(bind(&weighted, &s), Wrap::Loop);
    p.use_track_weights = true;
    anim.add(0, p).unwrap();
    anim.add(0, Playback::new(z_clip(&s, 0.0), Wrap::Loop))
        .unwrap();
    // Weights 0.01 → 2/255 and 1 → 255/255.
    let z = eval(&anim)[0].translation[2];
    assert!((z - 10.0 * 2.0 / 257.0).abs() < 1e-5, "{z}");
}

/// `appearance.md` §6.4 step 1 with `skeleton-anim.md` §6.6 step 3: a deformer clip's
/// track weight becomes ⌊255 × weight⌋ / 255 with no tolerance. The single-precision value
/// 1 − 0.6 = 0.39999998 keeps 101 steps (a tolerance would give 102).
#[test]
fn deformer_track_weights_are_truncated_without_tolerance() {
    use tessera_anim::{BindDeformation, DeformClip};
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let mut weighted = interleaved_clip(2, 1, 1.0, |_, _| at_z(1.0));
    weighted.weights = vec![1.0f32 - 0.6];
    let clips = [
        DeformClip {
            clip: z_clip(&s, 0.0),
            fraction: 0.0,
            use_bone_weights: true,
        },
        DeformClip {
            clip: bind(&weighted, &s),
            fraction: 0.0,
            use_bone_weights: true,
        },
    ];
    let d = BindDeformation::new(&s, &clips, None).unwrap();
    let z = d.local_pose()[0].translation[2];
    let expect = (101.0 / 255.0) / (1.0 + 101.0 / 255.0);
    assert!((z - expect).abs() < 1e-6, "{z} vs {expect}");
}
