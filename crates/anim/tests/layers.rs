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
