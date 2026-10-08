// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic round trips through `tessera-havok`'s testkit: skeletons and clips are written
//! as records by our own code, decoded, bound and played (`skeleton-anim.md` §3–§7, §14).

mod common;

use std::sync::Arc;

use common::*;
use glam::{Mat4, Quat, Vec3, Vec4};
use tessera_anim::{
    AnimError, Animator, BakeCache, BakedClip, BoundClip, CrossFade, EvalScratch, Layer,
    MotionDelta, PALETTE_CAPACITY, Playback, RootMotionMode, Skeleton, Source, SpeedBlend, Wrap,
    blend_poses, evaluate, motion_between, skin_position, skinning_matrices, write_palette_mat4,
};
use tessera_havok::testkit::{SkeletonSpec, quat_from_angles};
use tessera_havok::{Clip, Transform};

#[test]
fn skeleton_round_trip_through_setup_record() {
    let spec = tree_spec(40);
    let s = skeleton(&spec);
    assert_eq!(s.bone_count(), 40);
    for (i, b) in spec.bones.iter().enumerate() {
        assert_eq!(s.parent(i), usize::try_from(b.parent).ok());
        let r = s.reference_local()[i];
        assert_eq!(r.translation, b.reference.translation);
        assert_eq!(r.scale, b.reference.scale);
        assert!(quat_angle(r.rotation, b.reference.rotation) < 1e-6);
        assert_eq!(s.name(i), Some(format!("rig_bone{i}").as_bytes()));
    }
    assert_eq!(s.find_bone(b"rig_bone7"), Some(7));
    // B_b · B_b⁻¹ = I for every bone whose reference model scale is 1; the others have the
    // inverse reference triple of §5.4 (K26), which is not the exact inverse.
    let mut scaled = 0;
    for ((m, inv), r) in s
        .reference_model()
        .iter()
        .zip(s.inverse_bind())
        .zip(s.reference_triples())
    {
        if r.scale.iter().all(|v| (v - 1.0).abs() < 1e-6) {
            assert!((*m * *inv).abs_diff_eq(Mat4::IDENTITY, 1e-4));
        } else {
            scaled += 1;
            assert!(!(*m * *inv).abs_diff_eq(Mat4::IDENTITY, 1e-4));
        }
    }
    assert!(scaled > 0, "the tree has scaled reference bones");
    // Model transforms follow the triple rule of §5.4, built here independently.
    for i in 0..40 {
        let expect = independent_model(&s, s.reference_local(), i);
        assert!(s.reference_model()[i].abs_diff_eq(expect, 1e-4), "bone {i}");
    }
}

/// W_b by recursion with an explicit quaternion-to-matrix formula (not glam's TRS builder),
/// composing (translation, rotation, scale) triples by the rule of `skeleton-anim.md` §5.4:
/// t_b = t_p + R_p · t_local, R_b = R_p · R_local, s_b = s_p × s_local; W_b = T · R · S.
fn independent_model(s: &Skeleton, local: &[Transform], b: usize) -> Mat4 {
    let (t, r, sc) = independent_triple(s, local, b);
    Mat4::from_translation(t) * r * Mat4::from_scale(sc)
}

/// The skinned position of bind-space vertex `v` on bone `b` for pose `local` by the rule of
/// `skeleton-anim.md` §5.4 (K26), from independently built triples:
/// t − R (s ⊙ (R_rᵀ t_r)) + R R_rᵀ ((s ⊙ 1/s_r) ⊙ v).
fn independent_skin(s: &Skeleton, local: &[Transform], b: usize, v: Vec3) -> Vec3 {
    let (t, r, sc) = independent_triple(s, local, b);
    let (tr, rr, sr) = independent_triple(s, s.reference_local(), b);
    let rr_inv = rr.transpose();
    t - r.transform_vector3(sc * rr_inv.transform_vector3(tr))
        + (r * rr_inv).transform_vector3(sc / sr * v)
}

fn independent_triple(s: &Skeleton, local: &[Transform], b: usize) -> (Vec3, Mat4, Vec3) {
    let t = local[b];
    let [x, y, z, w] = t.rotation;
    let r = Mat4::from_cols(
        Vec4::new(
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
        ),
        Vec4::new(
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
        ),
        Vec4::new(
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
        ),
        Vec4::W,
    );
    let lt = Vec3::from_array(t.translation);
    let ls = Vec3::from_array(t.scale);
    match s.parent(b) {
        Some(p) => {
            let (pt, pr, ps) = independent_triple(s, local, p);
            (pt + pr.transform_vector3(lt), pr * r, ps * ls)
        }
        None => (lt, r, ls),
    }
}

#[test]
fn hand_built_havok_skeleton_with_cycle_is_rejected() {
    let mut h = tessera_havok::Skeleton::parse(&SkeletonSpec::chain(3).to_tagfile()).unwrap();
    // Hand-built values are not checked by tessera-havok: make bone 0's parent bone 2.
    h.bones[0].parent = Some(2);
    assert!(matches!(
        Skeleton::from_havok(&h),
        Err(AnimError::BadParent { bone: 0, parent: 2 })
    ));
    h.bones[0].parent = None;
    h.bones[1].parent = Some(1);
    assert!(matches!(
        Skeleton::from_havok(&h),
        Err(AnimError::BadParent { bone: 1, parent: 1 })
    ));
}

#[test]
fn binding_is_positional_and_exact_count() {
    let s = skeleton(&tree_spec(12));
    let rec = spline_clip(31, 11, 30.0, |_, _| Transform::IDENTITY);
    let clip = Arc::new(Clip::parse(&rec.to_bytes()).unwrap());
    assert_eq!(
        BoundClip::bind(clip, s.clone()).unwrap_err(),
        AnimError::TrackCountMismatch {
            tracks: 11,
            bones: 12
        }
    );
    let rec = spline_clip(31, 13, 30.0, |_, _| Transform::IDENTITY);
    let clip = Arc::new(Clip::parse(&rec.to_bytes()).unwrap());
    assert!(BoundClip::bind(clip, s.clone()).is_err());
    // Track j drives bone j: a clip moving only track 5 moves only bone 5's subtree.
    let rec = spline_clip(31, 12, 30.0, |f, j| {
        let mut t = s.reference_local()[j];
        if j == 5 {
            t.translation[2] += f as f32 * 0.01;
        }
        t
    });
    let c = bind(&rec, &s);
    let mut pose = vec![Transform::IDENTITY; 12];
    c.sample(1.0, &mut pose).unwrap();
    for (j, (t, r)) in pose.iter().zip(s.reference_local()).enumerate() {
        let dz = t.translation[2] - r.translation[2];
        if j == 5 {
            assert!((dz - 0.30).abs() < 1e-3, "{dz}");
        } else {
            assert!(dz.abs() < 1e-5);
        }
    }
}

#[test]
fn spline_and_interleaved_clip_round_trip() {
    let s = skeleton(&tree_spec(20));
    let pose = cyclic(&s, 61, 0.6);
    let c = bind(&spline_clip(61, 20, 60.0, &pose), &s);
    assert!((c.duration() - 1.0).abs() < 1e-6);
    assert_eq!(c.frame_count(), 61);
    let mut out = vec![Transform::IDENTITY; 20];
    for f in [0u32, 1, 15, 30, 59, 60] {
        c.sample(f as f32 / 60.0, &mut out).unwrap();
        let expect: Vec<Transform> = (0..20).map(|j| pose(f, j)).collect();
        let (dt, dr, ds) = pose_diff(&out, &expect);
        assert!(
            dt < 1e-4 && dr < 2e-3 && ds < 1e-4,
            "frame {f}: {dt} {dr} {ds}"
        );
    }
    let il = bind(
        &interleaved_clip(31, 20, 1.0, |f, j| pose(f as u32 * 2, j)),
        &s,
    );
    il.sample(0.5, &mut out).unwrap();
    let expect: Vec<Transform> = (0..20).map(|j| pose(30, j)).collect();
    let (dt, dr, _) = pose_diff(&out, &expect);
    assert!(dt < 1e-6 && dr < 1e-5);
}

#[test]
fn reference_pose_reproduces_bind_pose() {
    let s = skeleton(&tree_spec(30));
    let reference = s.reference_local().to_vec();
    // Spline clips quantise rotations to 40 bits: close to the bind pose.
    let sp = bind(&spline_clip(10, 30, 30.0, |_, j| reference[j]), &s);
    let mut local = vec![Transform::IDENTITY; 30];
    sp.sample(0.13, &mut local).unwrap();
    let mut model = vec![Mat4::IDENTITY; 30];
    s.compose(&local, &mut model).unwrap();
    for (a, b) in model.iter().zip(s.reference_model()) {
        assert!(a.abs_diff_eq(*b, 5e-3));
    }
    // Interleaved clips store floats: the bind pose to float precision.
    let c = bind(&interleaved_clip(4, 30, 0.1, |_, j| reference[j]), &s);
    c.sample(0.05, &mut local).unwrap();
    s.compose(&local, &mut model).unwrap();
    for (a, b) in model.iter().zip(s.reference_model()) {
        assert!(a.abs_diff_eq(*b, 2e-4));
    }
    let mut triples = vec![Transform::IDENTITY; 30];
    s.compose_triples(&local, &mut triples).unwrap();
    let mut skin = vec![Mat4::IDENTITY; 30];
    skinning_matrices(&s, &triples, &mut skin).unwrap();
    for (m, r) in skin.iter().zip(s.reference_triples()) {
        if r.scale.iter().all(|v| (v - 1.0).abs() < 1e-6) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 2e-4), "{m}");
        }
    }
    // Skinning stored vertices at the reference pose leaves them in place (§4.4) on bones
    // whose reference model scale is 1, and moves them by the client's offset
    // t_r − q_r (s_r ⊙ (q_r⁻¹ t_r)) on the others (§5.4, K26).
    let reference = s.reference_local().to_vec();
    for i in 0..50 {
        let v = Vec3::new((i as f32).sin(), (i as f32 * 0.7).cos(), 0.02 * i as f32);
        let j = [(i % 30) as u8, ((i * 7) % 30) as u8, 0, 0];
        let w = [0.6, 0.4, 0.0, 0.0];
        let p = skin_position(&skin, &j, &w, v);
        let expect = independent_skin(&s, &reference, usize::from(j[0]), v) * w[0]
            + independent_skin(&s, &reference, usize::from(j[1]), v) * w[1];
        assert!(p.abs_diff_eq(expect, 5e-4), "{i}: {p} vs {expect}");
    }
}

#[test]
fn looping_continuity_and_example_14_5() {
    let s = skeleton(&tree_spec(8));
    let pose = cyclic(&s, 61, 0.8);
    let c = bind(&spline_clip(61, 8, 60.0, &pose), &s);
    let mut a = vec![Transform::IDENTITY; 8];
    let mut b = vec![Transform::IDENTITY; 8];
    // §14.5: 2.25 s → t = 0.25 (frame 15); 3.0 s → t = 0; once: 2.25 → the last frame.
    c.sample_at(2.25, Wrap::Loop, &mut a).unwrap();
    let f15: Vec<Transform> = (0..8).map(|j| pose(15, j)).collect();
    assert!(pose_diff(&a, &f15).1 < 2e-3);
    c.sample_at(3.0, Wrap::Loop, &mut a).unwrap();
    c.sample(c.duration(), &mut b).unwrap();
    assert!(pose_diff(&a, &b).1 < 2e-3, "first pose equals last pose");
    c.sample_at(2.25, Wrap::Once, &mut a).unwrap();
    let last: Vec<Transform> = (0..8).map(|j| pose(60, j)).collect();
    assert!(pose_diff(&a, &last).1 < 2e-3);
    // Continuity across the wrap: poses just before and after a cycle boundary are close,
    // as close as neighbouring frames inside the cycle.
    let eps = 1e-3;
    c.sample_at(2.0 - eps, Wrap::Loop, &mut a).unwrap();
    c.sample_at(2.0 + eps, Wrap::Loop, &mut b).unwrap();
    let across = pose_diff(&a, &b);
    c.sample_at(0.5 - eps, Wrap::Loop, &mut a).unwrap();
    c.sample_at(0.5 + eps, Wrap::Loop, &mut b).unwrap();
    let inside = pose_diff(&a, &b);
    assert!(across.1 < inside.1 * 3.0 + 2e-3, "{across:?} vs {inside:?}");
    // A playback advanced in small steps never jumps.
    let mut p = Playback::new(c.clone(), Wrap::Loop);
    let mut prev = vec![Transform::IDENTITY; 8];
    c.sample(p.clip_time(), &mut prev).unwrap();
    for _ in 0..500 {
        p.advance(1.0 / 120.0);
        assert!(p.time >= 0.0 && p.time < c.duration());
        c.sample(p.clip_time(), &mut a).unwrap();
        assert!(pose_diff(&a, &prev).1 < 0.1);
        prev.copy_from_slice(&a);
    }
    let mut once = Playback::new(c, Wrap::Once);
    once.advance(5.0);
    assert!(once.is_finished());
    assert_eq!(once.clip_time(), 1.0);
}

#[test]
fn blending_weights() {
    let s = skeleton(&tree_spec(6));
    let ca = bind(
        &spline_clip(11, 6, 10.0, |_, j| Transform {
            translation: [0.0, 0.0, 0.0],
            ..s.reference_local()[j]
        }),
        &s,
    );
    let cb = bind(
        &spline_clip(11, 6, 10.0, |_, j| Transform {
            translation: [2.0, 0.0, 0.0],
            rotation: quat_from_angles(0.0, 0.0, 1.0),
            ..s.reference_local()[j]
        }),
        &s,
    );
    let mut a = vec![Transform::IDENTITY; 6];
    let mut b = vec![Transform::IDENTITY; 6];
    let mut out = vec![Transform::IDENTITY; 6];
    ca.sample(0.0, &mut a).unwrap();
    cb.sample(0.0, &mut b).unwrap();
    for alpha in [0.0f32, 0.25, 0.5, 1.0] {
        blend_poses(&a, &b, alpha, &mut out).unwrap();
        assert!((out[3].translation[0] - 2.0 * alpha).abs() < 1e-3);
        let angle = 2.0 * out[3].rotation[2].atan2(out[3].rotation[3]);
        // nlerp is exact at 0, 0.5 and 1 and close in between.
        assert!((angle - alpha).abs() < 0.02, "{alpha}: {angle}");
    }

    // Cross-fade: the incoming weight ramps linearly over the fade.
    let mut f = CrossFade::new(
        Playback::new(ca.clone(), Wrap::Loop),
        Playback::new(cb.clone(), Wrap::Loop),
        0.2,
        true,
    );
    assert_eq!(f.weight(), 0.0);
    f.advance(0.05);
    assert!((f.weight() - 0.25).abs() < 1e-6);
    f.advance(0.2);
    assert!(f.is_done());

    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(ca.clone(), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(0, Playback::new(cb.clone(), Wrap::Loop), 0.2, true)
        .unwrap();
    anim.advance(0.1);
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    assert_eq!(snap.len(), 1);
    let mut scratch = EvalScratch::new();
    evaluate(&s, &snap, &mut scratch, &mut out).unwrap();
    assert!(
        (out[3].translation[0] - 1.0).abs() < 1e-3,
        "half-way through the fade"
    );
    anim.advance(0.2);
    anim.snapshot(&mut snap);
    assert_eq!(snap[0].clips().count(), 1, "fade finished");
    evaluate(&s, &snap, &mut scratch, &mut out).unwrap();
    assert!((out[3].translation[0] - 2.0).abs() < 1e-3);

    // Speed blend idle (0) ↔ walk (2) ↔ run (6): weights bracket the speed.
    let mut sb = SpeedBlend::new(vec![
        (0.0, ca.clone()),
        (2.0, cb.clone()),
        (6.0, ca.clone()),
    ])
    .unwrap();
    sb.set_param(1.0);
    assert_eq!(sb.weights(), &[0.5, 0.5, 0.0]);
    sb.set_param(4.0);
    assert_eq!(sb.weights(), &[0.0, 0.5, 0.5]);
    sb.set_param(9.0);
    assert_eq!(sb.weights(), &[0.0, 0.0, 1.0]);
    sb.set_param(0.5);
    let mut anim = Animator::new(s.clone());
    anim.set_source(0, Source::Speed(sb)).unwrap();
    anim.snapshot(&mut snap);
    evaluate(&s, &snap, &mut scratch, &mut out).unwrap();
    // Weights 0.75 and 0.25, truncated to 191/255 and 63/255 (§6.6 step 3).
    let expect = 2.0 * 63.0 / (191.0 + 63.0);
    assert!(
        (out[3].translation[0] - expect).abs() < 1e-3,
        "{}",
        out[3].translation[0]
    );
    assert!(SpeedBlend::new(Vec::new()).is_err());
    let other = skeleton(&tree_spec(7));
    let oc = bind(&spline_clip(3, 7, 10.0, |_, _| Transform::IDENTITY), &other);
    assert!(SpeedBlend::new(vec![(0.0, ca.clone()), (1.0, oc.clone())]).is_err());
    assert!(
        anim.play(1, Playback::new(oc, Wrap::Loop), 0.0, false)
            .is_err()
    );
}

#[test]
fn speed_blend_phase_stays_synchronised() {
    let s = skeleton(&tree_spec(4));
    // Walk cycle 1.0 s, run cycle 0.5 s.
    let walk = bind(&spline_clip(61, 4, 60.0, cyclic(&s, 61, 0.3)), &s);
    let run = bind(&spline_clip(31, 4, 60.0, cyclic(&s, 31, 0.6)), &s);
    let mut sb = SpeedBlend::new(vec![(2.0, walk), (6.0, run)]).unwrap();
    sb.set_param(4.0);
    // Weighted cycle rate: 0.5 · 1/1.0 + 0.5 · 1/0.5 = 1.5 cycles per second.
    sb.advance(0.2);
    assert!((sb.phase() - 0.3).abs() < 1e-5, "{}", sb.phase());
    sb.advance(1.0);
    assert!((sb.phase() - 0.8).abs() < 1e-4);
}

#[test]
fn partial_layer_with_track_weights() {
    let s = skeleton(&tree_spec(4));
    let base = bind(&spline_clip(3, 4, 10.0, |_, j| s.reference_local()[j]), &s);
    let mut rec = spline_clip(3, 4, 10.0, |_, j| Transform {
        rotation: quat_from_angles(0.0, 0.0, std::f32::consts::FRAC_PI_2),
        ..s.reference_local()[j]
    });
    rec.weights = vec![1.0, 1.0, 0.01, 0.5];
    let layer = bind(&rec, &s);
    assert_eq!(layer.track_weights(), Some(&[1.0, 1.0, 0.01, 0.5][..]));
    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(base, Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(1, Playback::new(layer.clone(), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.layer_mut(1).unwrap().use_track_weights = true;
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    let mut out = vec![Transform::IDENTITY; 4];
    evaluate(&s, &snap, &mut EvalScratch::new(), &mut out).unwrap();
    let rel_of = |out: &[Transform], j: usize| {
        let r = Quat::from_array(s.reference_local()[j].rotation);
        let q = Quat::from_array(out[j].rotation);
        (r.inverse() * q).to_axis_angle().1.to_degrees()
    };
    let rel = |j: usize| rel_of(&out, j);
    // §14.6 with a reference-pose base: tracks 0, 1 full, track 3 half (127/255 each),
    // track 2 2/255 against 252/255: 0.639°.
    assert!((rel(0) - 90.0).abs() < 0.1);
    assert!((rel(3) - 45.0).abs() < 0.1, "{}", rel(3));
    assert!((rel(2) - 0.639).abs() < 0.05, "{}", rel(2));
    // Played alone, track 2 gets 2/255 and the reference pose fills 0.1 − 2/255 (§14.6):
    // 0.0784 of the way, 6.5° after normalisation.
    let mut solo = Animator::new(s.clone());
    solo.play(0, Playback::new(layer, Wrap::Loop), 0.0, false)
        .unwrap();
    solo.layer_mut(0).unwrap().use_track_weights = true;
    solo.snapshot(&mut snap);
    evaluate(&s, &snap, &mut EvalScratch::new(), &mut out).unwrap();
    assert!((rel_of(&out, 2) - 6.495).abs() < 0.1, "{}", rel_of(&out, 2));
    assert!((rel_of(&out, 0) - 90.0).abs() < 0.1);
    // Layer weight ramps.
    let l = anim.layer_mut(1).unwrap();
    l.fade_to(0.0, 0.5);
    anim.advance(0.25);
    assert!((anim.layers()[1].weight - 0.5).abs() < 1e-5);
    anim.advance(1.0);
    assert_eq!(anim.layers()[1].weight, 0.0);
    anim.snapshot(&mut snap);
    assert_eq!(snap.len(), 1, "zero-weight layers are skipped");
    let _ = Layer::new(None);
}

#[test]
fn root_motion_totals() {
    let s = skeleton(&tree_spec(3));
    // 2 s clip, 61 frames: moves 1 m forward (+y) and turns +π/4 over one cycle.
    let frames = 61;
    let samples: Vec<[f32; 4]> = (0..frames)
        .map(|f| {
            let u = f as f32 / (frames - 1) as f32;
            [0.0, u, 0.0, u * std::f32::consts::FRAC_PI_4]
        })
        .collect();
    let rec = with_motion(
        spline_clip(frames as u32, 3, 30.0, |_, j| s.reference_local()[j]),
        samples,
    );
    let c = bind(&rec, &s);
    assert!(c.has_extracted_motion());
    let d = c.duration();
    assert!((d - 2.0).abs() < 1e-5);

    // One full cycle: 1 m forward, π/4 turn.
    let one = motion_between(&c, 0.0, d, Wrap::Once);
    assert!(one.translation.abs_diff_eq(Vec3::new(0.0, 1.0, 0.0), 1e-4));
    assert!((one.yaw() - std::f32::consts::FRAC_PI_4).abs() < 1e-4);

    // Played in place: nothing.
    let mut p = Playback::new(c.clone(), Wrap::Loop);
    assert_eq!(p.advance(1.0), MotionDelta::IDENTITY);

    // Translated, looping, in uneven steps over 3.5 cycles: the composed motion equals the
    // cycle motion repeated three times, then half a cycle.
    let mut p = Playback::new(c.clone(), Wrap::Loop);
    p.root_motion = RootMotionMode::Translate;
    let mut total = MotionDelta::IDENTITY;
    let mut t = 0.0;
    let steps = [0.016f32, 0.033, 0.4, 1.7, 0.016, 2.5];
    let mut i = 0;
    while t < 3.5 * d - 1e-6 {
        let step = steps[i % steps.len()].min(3.5 * d - t);
        total = total.then(p.advance(step));
        t += step;
        i += 1;
    }
    let half = motion_between(&c, 0.0, d * 0.5, Wrap::Once);
    let expect = one.repeat(3).then(half);
    assert!(
        total.translation.abs_diff_eq(expect.translation, 1e-3),
        "{} vs {}",
        total.translation,
        expect.translation
    );
    assert!((total.yaw() - expect.yaw()).abs() < 1e-3);
    // 3.5 turns of π/4 = 7π/8.
    assert!((total.yaw() - 7.0 * std::f32::consts::PI / 8.0).abs() < 1e-3);

    // Backwards undoes forwards; once clamps at the end.
    let back = motion_between(&c, 1.5, 0.5, Wrap::Loop);
    let fwd = motion_between(&c, 0.5, 1.5, Wrap::Loop);
    let id = fwd.then(back);
    assert!(id.translation.abs_diff_eq(Vec3::ZERO, 1e-5));
    let past = motion_between(&c, 0.0, 10.0, Wrap::Once);
    assert!(past.translation.abs_diff_eq(one.translation, 1e-5));
    // Short sample arrays hold the last sample (§7.3).
    let short = with_motion(
        spline_clip(61, 3, 30.0, |_, j| s.reference_local()[j]),
        vec![[0.0; 4], [0.0, 0.5, 0.0, 0.0]],
    );
    let cs = bind(&short, &s);
    let m = motion_between(&cs, 0.0, 2.0, Wrap::Once);
    assert!(m.translation.abs_diff_eq(Vec3::new(0.0, 0.5, 0.0), 1e-5));
    // A clip without extracted motion never moves.
    let still = bind(&spline_clip(3, 3, 30.0, |_, j| s.reference_local()[j]), &s);
    assert_eq!(
        motion_between(&still, 0.0, 9.0, Wrap::Loop),
        MotionDelta::IDENTITY
    );
}

#[test]
fn palette_matches_cpu_skinned_reference() {
    let s = skeleton(&tree_spec(64));
    let c = bind(&spline_clip(61, 64, 60.0, cyclic(&s, 61, 0.9)), &s);
    let mut local = vec![Transform::IDENTITY; 64];
    c.sample(0.37, &mut local).unwrap();
    let mut model = vec![Mat4::IDENTITY; 64];
    let mut triples = vec![Transform::IDENTITY; 64];
    s.compose_with_triples(&local, &mut model, &mut triples)
        .unwrap();
    let mut skin = vec![Mat4::IDENTITY; 64];
    skinning_matrices(&s, &triples, &mut skin).unwrap();
    let mut palette = vec![[0.0f32; 16]; PALETTE_CAPACITY];
    write_palette_mat4(&skin, &mut palette).unwrap();
    for i in 0..400u32 {
        let v = Vec3::new(
            (i as f32 * 0.13).sin(),
            (i as f32 * 0.29).cos(),
            (i % 17) as f32 * 0.1,
        );
        let joints = [
            (i % 64) as u8,
            ((i * 5 + 1) % 64) as u8,
            ((i * 11 + 3) % 64) as u8,
            ((i * 3 + 7) % 64) as u8,
        ];
        let w = [0.4, 0.3, 0.2, 0.1];
        // The "GPU" path: matrices read back from the palette buffer.
        let mut gpu = Vec4::ZERO;
        for k in 0..4 {
            let m = Mat4::from_cols_array(&palette[usize::from(joints[k])]);
            gpu += m * v.extend(1.0) * w[k];
        }
        // The CPU reference: Σ w · (the §5.4 skinning of v on bone b), from triples built
        // independently.
        let mut cpu = Vec3::ZERO;
        for k in 0..4 {
            cpu += independent_skin(&s, &local, usize::from(joints[k]), v) * w[k];
        }
        assert!(gpu.truncate().abs_diff_eq(cpu, 1e-3), "{i}: {gpu} vs {cpu}");
        let lib = skin_position(&skin, &joints, &w, v);
        assert!(lib.abs_diff_eq(cpu, 1e-3));
    }
    // Unused palette entries are identity.
    assert_eq!(palette[200], Mat4::IDENTITY.to_cols_array());
}

#[test]
fn baked_clip_matches_curves_at_frames() {
    let s = skeleton(&tree_spec(10));
    let c = bind(&spline_clip(121, 10, 120.0, cyclic(&s, 121, 0.7)), &s);
    let baked = Arc::new(BakedClip::bake(c.clip(), 1 << 20).unwrap());
    assert_eq!(baked.frame_count(), 121);
    let cb = Arc::new(BoundClip::clone(&c).with_baked(baked.clone()).unwrap());
    let mut a = vec![Transform::IDENTITY; 10];
    let mut b = vec![Transform::IDENTITY; 10];
    for f in 0..121 {
        let t = f as f32 / 120.0;
        c.sample(t, &mut a).unwrap();
        cb.sample(t, &mut b).unwrap();
        let (dt, dr, ds) = pose_diff(&a, &b);
        assert!(dt < 1e-5 && dr < 1e-4 && ds < 1e-5, "frame {f}");
    }
    // Between frames the baked clip stays close to the curves.
    c.sample(0.5 + 1.0 / 240.0, &mut a).unwrap();
    cb.sample(0.5 + 1.0 / 240.0, &mut b).unwrap();
    assert!(pose_diff(&a, &b).1 < 1e-2);
    assert!(BakedClip::bake(c.clip(), 100).is_none());

    let cache = BakeCache::new(2 * baked.heap_size() + 10, 1 << 20);
    for key in 0..4u64 {
        assert!(cache.get_or_bake(key, c.clip()).is_some());
    }
    let st = cache.stats();
    assert_eq!(st.bakes, 4);
    assert!(st.bytes <= 2 * baked.heap_size() + 10);
    assert_eq!(st.entries, 2);
    assert!(cache.get(3).is_some());
    assert!(cache.get(0).is_none());
    let small = BakeCache::new(1000, 1000);
    assert!(small.get_or_bake(9, c.clip()).is_none());
    assert_eq!(small.stats().refused, 1);
}

#[test]
fn hostile_sample_values_fall_back_to_reference() {
    let s = skeleton(&tree_spec(3));
    let rec = interleaved_clip(2, 3, 1.0, |f, j| {
        let mut t = s.reference_local()[j];
        if j == 1 {
            t.translation[0] = if f == 0 { 2.0e4 } else { 1.0e10 };
        }
        if j == 2 {
            t.rotation = [0.0, 0.0, 0.0, 3.0];
        }
        t
    });
    let c = bind(&rec, &s);
    let mut out = vec![Transform::IDENTITY; 3];
    let r = c.sample(0.5, &mut out).unwrap();
    assert_eq!(r.replaced, 1);
    assert_eq!(out[1], s.reference_local()[1]);
    assert!((tessera_havok::quat_length(out[2].rotation) - 1.0).abs() < 1e-5);
    let mut short = vec![Transform::IDENTITY; 2];
    assert!(c.sample(0.5, &mut short).is_err());
    assert!(c.root_displacement().is_finite());
}
