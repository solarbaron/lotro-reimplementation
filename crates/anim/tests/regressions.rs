// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on `tessera-anim` (all data synthetic).

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use glam::{Quat, Vec3};
use tessera_anim::{
    AnimError, AnimationScheduler, Animator, BakeCache, BakedClip, BindDeformation, BoneSliders,
    BoundClip, CrossFade, EvalScratch, MAX_LAYER_CLIPS, MIN_BONE_WEIGHT, Playback, RootMotionMode,
    SchedulerConfig, SliderChannels, Source, SpeedBlend, SubmitError, Submitted, Wrap, evaluate,
    motion_at, motion_between, quantise_weight,
};
use tessera_assets::{CancelToken, PoolConfig, Priority, WorkerPool};
use tessera_havok::Transform;
use tessera_havok::testkit::{ReferenceFrameSpec, SkeletonSpec, quat_from_angles};

const WAIT: Duration = Duration::from_secs(10);

/// A clip of `frames` frames at `fps` whose extracted motion moves 1 m along +y per cycle,
/// linearly.
fn moving_clip(frames: u32, fps: f32) -> Arc<BoundClip> {
    let s = skeleton(&tree_spec(3));
    let samples: Vec<[f32; 4]> = (0..frames)
        .map(|f| [0.0, f as f32 / (frames - 1) as f32, 0.0, 0.0])
        .collect();
    let rec = with_motion(
        spline_clip(frames, 3, fps, |_, j| s.reference_local()[j]),
        samples,
    );
    bind(&rec, &s)
}

/// Findings 1 and 9: the cycle index and the in-cycle time came from different
/// computations and disagreed a hair below a cycle boundary, adding or losing a whole
/// cycle of extracted motion.
#[test]
fn loop_motion_counts_each_cycle_once() {
    // 2 s, 61 frames: a tiny negative start time.
    let c = moving_clip(61, 30.0);
    let d = c.duration();
    let m = motion_between(&c, -1e-9, 0.5, Wrap::Loop);
    assert!((m.translation.y - 0.25).abs() < 1e-4, "{}", m.translation);

    // A negative-rate step that lands just below 0.
    let mut p = Playback::new(c.clone(), Wrap::Loop);
    p.root_motion = RootMotionMode::Translate;
    p.time = 0.1;
    p.rate = -1.0;
    let m = p.advance(f32::from_bits(0.1f32.to_bits() + 1));
    assert!(
        m.translation.abs_diff_eq(Vec3::new(0.0, -0.05, 0.0), 1e-4),
        "{}",
        m.translation
    );

    // 5 frames at 30 fps (d = 4/30 s): tau / d rounds up to 3 in f32 at 0.4.
    let c = moving_clip(5, 30.0);
    let d5 = c.duration();
    let m = motion_between(&c, 0.0, 0.4, Wrap::Loop);
    assert!(
        (m.translation.y - 0.4 / d5).abs() < 1e-3,
        "{}",
        m.translation
    );
    let below = f32::from_bits((3.0 * d5).to_bits() - 1);
    let m = motion_between(&c, 0.0, below, Wrap::Loop);
    assert!((m.translation.y - 3.0).abs() < 1e-3, "{}", m.translation);

    // A sweep over frame counts, rates and times: the motion is always tau / d.
    for frames in [3u32, 5, 7, 11, 31, 61, 199] {
        for fps in [30.0f32, 60.0, 120.0] {
            let c = moving_clip(frames, fps);
            let d = c.duration();
            for k in 1..40 {
                let tau = k as f32 * 0.1;
                for tau in [tau, f32::from_bits(tau.to_bits() - 1), -tau] {
                    let m = motion_between(&c, 0.0, tau, Wrap::Loop);
                    let expect = f64::from(tau) / f64::from(d);
                    assert!(
                        (f64::from(m.translation.y) - expect).abs() < 1e-3 * (1.0 + expect.abs()),
                        "frames {frames} fps {fps} tau {tau}: {} vs {expect}",
                        m.translation.y
                    );
                }
            }
        }
    }
    let _ = d;
}

/// Finding 4: §7.3 turns the entity about the world up axis, whatever `up` the frame
/// stores.
#[test]
fn extracted_yaw_turns_about_world_up() {
    let s = skeleton(&tree_spec(3));
    let frames = 11u32;
    let mut rec = spline_clip(frames, 3, 10.0, |_, j| s.reference_local()[j]);
    let duration = rec.animation.duration();
    rec.animation.extracted_motion = Some(ReferenceFrameSpec {
        up: [0.0, 1.0, 0.0, 0.0],
        forward: [0.0, 0.0, 1.0, 0.0],
        duration,
        components: 4,
        samples: (0..frames)
            .map(|f| [0.0, 0.0, 0.0, 0.04 * f as f32 / (frames - 1) as f32])
            .collect(),
    });
    let c = bind(&rec, &s);
    let m = motion_at(&c, c.duration()).unwrap();
    assert!(
        m.rotation.abs_diff_eq(Quat::from_rotation_z(0.04), 1e-6),
        "{:?}",
        m.rotation
    );
    assert!((m.yaw() - 0.04).abs() < 1e-5);
}

/// Three clips that set bone 1 to 0, 1 and 2 rad about z.
fn fade_clips() -> (Arc<tessera_anim::Skeleton>, [Arc<BoundClip>; 3]) {
    let s = skeleton(&SkeletonSpec::chain(3));
    let clip = |angle: f32| {
        let s2 = s.clone();
        bind(
            &spline_clip(11, 3, 10.0, move |_, j| {
                if j == 1 {
                    Transform {
                        rotation: quat_from_angles(0.0, 0.0, angle),
                        ..s2.reference_local()[j]
                    }
                } else {
                    s2.reference_local()[j]
                }
            }),
            &s,
        )
    };
    let clips = [clip(0.0), clip(1.0), clip(2.0)];
    (s, clips)
}

fn bone1_angle(anim: &Animator) -> f32 {
    let s = anim.skeleton();
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    let mut out = s.reference_pose();
    evaluate(s, &snap, &mut EvalScratch::new(), &mut out).unwrap();
    let q = out[1].rotation;
    2.0 * q[2].atan2(q[3])
}

/// Findings 3 and 10, revised by `script-table.md` §7.1 step 5 and `skeleton-anim.md`
/// §14.9: interrupting a fade stops the clip still fading out at once (the pose loses its
/// share), while the clip that was coming in continues from its weight without a jump and
/// the new clip rises from the complement.
#[test]
fn interrupted_fade_is_continuous() {
    let (s, [a, b, c]) = fade_clips();
    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(a.clone(), Wrap::Loop), 0.0, false)
        .unwrap();
    anim.play(0, Playback::new(b.clone(), Wrap::Loop), 1.0, false)
        .unwrap();
    anim.advance(0.5);
    let before = bone1_angle(&anim);
    assert!((before - 0.5).abs() < 0.02, "{before}");
    anim.play(0, Playback::new(c.clone(), Wrap::Loop), 1.0, false)
        .unwrap();
    // a (0 rad) is gone; b (1 rad) at 0.5 and c (2 rad) at 0.5.
    let after = bone1_angle(&anim);
    assert!((after - 1.5).abs() < 0.02, "{before} -> {after}");
    // The fade is placed: b is carried at start weight 1, half of the fade already past.
    match &anim.layers()[0].source {
        Some(Source::CrossFade(f)) => {
            let w: Vec<f32> = f.from.iter().map(|(_, w)| *w).collect();
            assert_eq!(w, vec![1.0]);
            assert!((f.elapsed - 0.5).abs() < 1e-6, "{}", f.elapsed);
            assert!((f.weight() - 0.5).abs() < 1e-6);
        }
        other => panic!("{other:?}"),
    }
    anim.advance(0.25);
    let mid = bone1_angle(&anim);
    assert!(mid > after && mid < 2.0, "{mid}");
    anim.advance(0.25);
    assert!((bone1_angle(&anim) - 2.0).abs() < 1e-3);
    assert!(matches!(anim.layers()[0].source, Some(Source::Clip(_))));

    // A fade from a speed blend starts from the blend, not with a switch.
    let mut sb = SpeedBlend::new(vec![(0.0, a.clone()), (1.0, b.clone())]).unwrap();
    sb.set_param(0.5);
    let mut anim = Animator::new(s.clone());
    anim.set_source(0, Source::Speed(sb)).unwrap();
    let before = bone1_angle(&anim);
    anim.play(0, Playback::new(c.clone(), Wrap::Loop), 0.3, true)
        .unwrap();
    assert!((bone1_angle(&anim) - before).abs() < 1e-4);

    // Rapid interruptions stay bounded and continuous.
    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(a.clone(), Wrap::Loop), 0.0, false)
        .unwrap();
    for i in 0..50 {
        let next = [&a, &b, &c][i % 3].clone();
        let before = bone1_angle(&anim);
        anim.play(0, Playback::new(next, Wrap::Loop), 0.25, false)
            .unwrap();
        let src = anim.layers()[0].source.as_ref().unwrap();
        assert!(src.clip_count() <= MAX_LAYER_CLIPS);
        let after = bone1_angle(&anim);
        // Interrupting a fade stops its outgoing clip at once (§7.1 step 5), so only the
        // first fade, started from a plain clip, keeps the pose.
        assert!(after.is_finite());
        if i == 0 {
            assert!((after - before).abs() < 1e-4, "{i}: {before} -> {after}");
        } else {
            assert!(src.clip_count() <= 2, "{i}: {}", src.clip_count());
        }
        anim.advance(0.03);
    }
}

/// `skeleton-anim.md` §6.6 step 2 and the §15 answer (3): each clip's weight is its own
/// ramp, with no cross-fade chain product. A stopped clip at weight 0.1 (b, from a group)
/// falls linearly while c rises; with nothing below, the layered-weight rule quantises
/// them and fills a total of at most 0.1 with the reference pose. A fade from an empty
/// speed blend stops nothing.
#[test]
fn fade_over_empty_source_keeps_later_weights() {
    let (s, [_, b, c]) = fade_clips();
    let reference = bone1_angle(&Animator::new(s.clone()));
    let state = |w2: f32| {
        let mut sb = SpeedBlend::new(vec![(0.0, b.clone()), (1.0, c.clone())]).unwrap();
        sb.set_param(f32::NAN);
        // A fade from an empty speed blend to b, 0.1 of the way in: b has weight 0.1.
        let empty = CrossFade::from_source(
            Source::Speed(sb),
            Playback::new(b.clone(), Wrap::Loop),
            1.0,
            false,
        );
        assert!(empty.from.is_empty(), "nothing to stop");
        // b plays at weight 0.1 beside nothing else (not a fade in progress).
        let inner = Source::Group(vec![(Playback::new(b.clone(), Wrap::Loop), 0.1)]);
        let outer = CrossFade {
            elapsed: w2,
            duration: 1.0,
            ..CrossFade::from_source(inner, Playback::new(c.clone(), Wrap::Loop), 1.0, false)
        };
        let mut anim = Animator::new(s.clone());
        anim.set_source(0, Source::CrossFade(outer)).unwrap();
        bone1_angle(&anim)
    };
    // b (1 rad) has weight 0.1 (1 − w2), c (2 rad) weight w2. Up to nlerp error.
    let expect = |w2: f32| {
        let wb = quantise_weight(0.1 * (1.0 - w2));
        let wc = quantise_weight(w2);
        let fill = (MIN_BONE_WEIGHT - wb - wc).max(0.0);
        (wb + 2.0 * wc + fill * reference) / (wb + wc + fill)
    };
    for w2 in [0.0f32, 0.5, 0.9, 0.999] {
        let got = state(w2);
        assert!(
            (got - expect(w2)).abs() < 0.05,
            "w2 {w2}: {got} vs {}",
            expect(w2)
        );
    }
    let near = state(0.999);
    let done = state(1.0);
    assert!((done - 2.0).abs() < 1e-3, "{done}");
    assert!((done - near).abs() < 0.01, "{near} -> {done}");
}

/// Finding 13: a fade whose outgoing clip did not fit the skeleton was accepted.
#[test]
fn fade_sources_are_validated_on_both_sides() {
    let (s, [a, ..]) = fade_clips();
    let other = skeleton(&tree_spec(5));
    let oc = bind(
        &spline_clip(3, 5, 10.0, |_, j| other.reference_local()[j]),
        &other,
    );
    let mut anim = Animator::new(s);
    let bad = CrossFade::new(
        Playback::new(oc.clone(), Wrap::Loop),
        Playback::new(a.clone(), Wrap::Loop),
        0.5,
        false,
    );
    assert!(matches!(
        anim.set_source(0, Source::CrossFade(bad)),
        Err(AnimError::TrackCountMismatch { tracks: 5, .. })
    ));
    let good = CrossFade::new(
        Playback::new(a.clone(), Wrap::Loop),
        Playback::new(a, Wrap::Loop),
        0.5,
        false,
    );
    anim.set_source(0, Source::CrossFade(good)).unwrap();
}

/// Finding 5: an animator of another rig with the same bone count was accepted.
#[test]
fn scheduler_rejects_other_rig_with_same_bone_count() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
    let s = skeleton(&tree_spec(6));
    let id = sched.add(s.clone(), None).unwrap();
    let mut spec = tree_spec(6);
    spec.bones[3].reference.translation[0] += 0.01;
    let other = skeleton(&spec);
    assert_eq!(other.bone_count(), s.bone_count());
    assert!(matches!(
        sched.submit(id, &Animator::new(other), 1),
        Err(AnimError::Mismatch { .. })
    ));
    // The same rig decoded separately (another Arc) is fine.
    let same = skeleton(&tree_spec(6));
    assert!(!Arc::ptr_eq(&same, &s));
    assert_eq!(
        sched.submit(id, &Animator::new(same), 1).unwrap(),
        Submitted::Queued
    );
    assert_eq!(sched.collect_all(WAIT).completed, 1);
}

/// Occupies one worker until the returned sender is dropped (or 20 s pass).
fn block_worker(pool: &WorkerPool) -> std::sync::mpsc::Sender<()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    pool.spawn(Priority::Immediate, CancelToken::new(), move |_| {
        let _ = started_tx.send(());
        let _ = rx.recv_timeout(Duration::from_secs(20));
    })
    .unwrap();
    started_rx.recv_timeout(WAIT).unwrap();
    tx
}

/// Finding 14 (second part): pool refusals were counted as submitted and cancelled.
#[test]
fn pool_refusals_are_counted_separately() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued: 1,
        ..PoolConfig::default()
    });
    let s = skeleton(&tree_spec(4));
    let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
    let ids: Vec<_> = (0..4)
        .map(|_| sched.add(s.clone(), None).unwrap())
        .collect();
    let a = Animator::new(s.clone());
    // Block the single worker so the queue fills.
    let tx = block_worker(&pool);
    let mut queued = 0;
    let mut refused = 0;
    for &id in &ids {
        match sched.try_submit(id, &a, 1) {
            Ok(Submitted::Queued) => queued += 1,
            Err(SubmitError::Pool(_)) => refused += 1,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!((queued, refused), (1, 3));
    let st = sched.stats();
    assert_eq!(st.submitted, 1);
    assert_eq!(st.refused, 3);
    drop(tx);
    let c = sched.collect_all(WAIT);
    assert_eq!(c.completed, 1);
    assert_eq!(c.cancelled, 0, "refusals are not cancellations");
    let st = sched.stats();
    assert_eq!((st.cancelled, st.in_flight), (0, 0));
    // The refused instances got their buffers back and can submit again.
    for &id in &ids {
        assert!(!sched.is_busy(id));
    }
    // `submit` reports refusals as Busy (and logs only the first as a warning).
    let release = block_worker(&pool);
    let outcomes: Vec<_> = ids
        .iter()
        .map(|&id| sched.submit(id, &a, 2).unwrap())
        .collect();
    assert_eq!(
        outcomes.iter().filter(|o| **o == Submitted::Busy).count(),
        3
    );
    drop(release);
    sched.collect_all(WAIT);
    assert_eq!(sched.stats().refused, 6);
}

/// Finding 11, revised (`appearance.md` §6.4, `skeleton-anim.md` §10 step 3): bone sliders
/// never touch the animated pose; they shape the deformation pose of the bind-space
/// deformation instead.
#[test]
fn sliders_deform_in_bind_space_not_the_animated_pose() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        ..PoolConfig::default()
    });
    let s = skeleton(&tree_spec(16));
    let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
    let id = sched.add(s.clone(), None).unwrap();
    let mut sliders = BoneSliders::new(&s);
    let xyz = SliderChannels::SCALE_X | SliderChannels::SCALE_Y | SliderChannels::SCALE_Z;
    sliders
        .add_slider(xyz, [0.9; 3], [1.1; 3], 1.0, &[(7, false)])
        .unwrap();
    sliders
        .add_slider(
            SliderChannels::TRANSLATE_X,
            [-0.003, 0.0, 0.0],
            [0.003, 0.0, 0.0],
            1.0,
            &[(12, true), (13, false)],
        )
        .unwrap();
    let clip = bind(&spline_clip(11, 16, 10.0, cyclic(&s, 11, 0.2)), &s);
    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(clip, Wrap::Loop), 0.0, false)
        .unwrap();
    anim.advance(0.3);
    sched.submit(id, &anim, 1).unwrap();
    sched.collect_all(WAIT);
    let pose = sched.pose(id).unwrap();
    assert!(pose.error.is_none());

    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    let mut plain = s.reference_pose();
    evaluate(&s, &snap, &mut EvalScratch::new(), &mut plain).unwrap();
    let (dt, dr, ds) = pose_diff(&pose.local, &plain);
    assert!(dt < 1e-6 && dr < 1e-5 && ds < 1e-6, "{dt} {dr} {ds}");

    // The deformation pose carries the sliders.
    let d = BindDeformation::new(&s, &[], Some(&sliders)).unwrap();
    let mut expect = s.reference_pose();
    sliders.apply(&mut expect).unwrap();
    assert_eq!(d.local_pose(), expect.as_slice());
    for k in 0..3 {
        assert!((d.local_pose()[7].scale[k] - 1.1).abs() < 1e-6);
    }
    // Sliders of another rig are refused.
    let other = skeleton(&tree_spec(17));
    assert!(matches!(
        BindDeformation::new(&s, &[], Some(&BoneSliders::new(&other))),
        Err(AnimError::Mismatch { .. })
    ));
}

/// Finding 7: after the budget shrank below the per-clip limit, a clip between the two was
/// baked again on every call and never kept.
#[test]
fn bake_cache_limit_follows_budget() {
    let s = skeleton(&tree_spec(4));
    let rec = spline_clip(61, 4, 30.0, |_, j| s.reference_local()[j]);
    let clip = tessera_havok::Clip::parse(&rec.to_bytes()).unwrap();
    let size = BakedClip::bake(&clip, usize::MAX).unwrap().heap_size();
    assert!(size > 1000);
    let cache = BakeCache::new(1 << 20, 1 << 20);
    cache.set_budget(1000);
    for _ in 0..5 {
        assert!(cache.get_or_bake(7, &clip).is_none());
    }
    let st = cache.stats();
    assert_eq!((st.bakes, st.refused, st.entries), (0, 5, 0));
    cache.set_budget(1 << 20);
    for _ in 0..5 {
        assert!(cache.get_or_bake(7, &clip).is_some());
    }
    let st = cache.stats();
    assert_eq!((st.bakes, st.hits, st.entries), (1, 4, 1));
}

/// Finding 16: a baked clip from another clip with the same track count was accepted.
#[test]
fn baked_clip_must_come_from_the_bound_clip() {
    let (s, [a, b, _]) = fade_clips();
    let baked_b = Arc::new(BakedClip::bake(b.clip(), usize::MAX).unwrap());
    assert!(matches!(
        BoundClip::clone(&a).with_baked(baked_b.clone()),
        Err(AnimError::Mismatch { .. })
    ));
    assert!(BoundClip::clone(&b).with_baked(baked_b).is_ok());
    // Same content, other frame count.
    let longer = bind(&spline_clip(21, 3, 10.0, |_, j| s.reference_local()[j]), &s);
    let baked_long = Arc::new(BakedClip::bake(longer.clip(), usize::MAX).unwrap());
    assert!(BoundClip::clone(&a).with_baked(baked_long).is_err());
}

/// Finding 15: the content key ignores names (documented), and `same_rig` says so.
#[test]
fn content_key_ignores_names() {
    let named = skeleton(&tree_spec(5));
    let mut spec = tree_spec(5);
    for (i, b) in spec.bones.iter_mut().enumerate() {
        b.name = Some(i.to_string());
    }
    let numbered = skeleton(&spec);
    assert_eq!(named.content_key(), numbered.content_key());
    assert!(named.same_rig(&numbered));
    assert_ne!(*named, *numbered, "PartialEq compares names");
    assert_eq!(named.find_bone(b"rig_bone2"), Some(2));
    assert_eq!(numbered.find_bone(b"rig_bone2"), None);
}

/// Audit finding (fix2 0): finite but huge clip scales passed the broken-sample check, and
/// the scales multiplied down the chain to inf, giving a non-finite skinning palette.
#[test]
fn huge_clip_scales_never_reach_the_palette() {
    let s = skeleton(&SkeletonSpec::chain(3));
    let rec = spline_clip(5, 3, 30.0, |_, j| Transform {
        scale: if j < 2 { [1e20; 3] } else { [1.0; 3] },
        ..s.reference_local()[j]
    });
    let c = bind(&rec, &s);
    let mut local = vec![Transform::IDENTITY; 3];
    let report = c.sample(0.05, &mut local).unwrap();
    assert_eq!(report.replaced, 2, "huge scales are broken samples");
    assert_eq!(&local[..2], &s.reference_local()[..2]);

    // A hand-built pose with the same values (compose is public): model and skinning
    // matrices stay finite.
    let local: Vec<Transform> = (0..3)
        .map(|j| Transform {
            scale: [1e20; 3],
            ..s.reference_local()[j]
        })
        .collect();
    let mut model = vec![glam::Mat4::IDENTITY; 3];
    let fallbacks = s.compose_counted(&local, &mut model).unwrap();
    assert!(fallbacks > 0);
    assert!(model.iter().all(glam::Mat4::is_finite), "{model:?}");
    let mut triples = vec![Transform::IDENTITY; 3];
    s.compose_triples(&local, &mut triples).unwrap();
    assert!(triples.iter().all(Transform::is_finite), "{triples:?}");
    let mut skin = vec![glam::Mat4::IDENTITY; 3];
    tessera_anim::skinning_matrices(&s, &triples, &mut skin).unwrap();
    assert!(skin.iter().all(glam::Mat4::is_finite), "{skin:?}");

    // Through the layered evaluator and the averaged result as well.
    let mut out = vec![Transform::IDENTITY; 3];
    let mut anim = Animator::new(s.clone());
    anim.play(0, Playback::new(c, Wrap::Loop), 0.0, false)
        .unwrap();
    let mut snap = Vec::new();
    anim.snapshot(&mut snap);
    let r = evaluate(&s, &snap, &mut EvalScratch::new(), &mut out).unwrap();
    assert_eq!(r.replaced, 2);
    let mut model = vec![glam::Mat4::IDENTITY; 3];
    s.compose(&out, &mut model).unwrap();
    assert!(model.iter().all(glam::Mat4::is_finite));
}

/// Audit finding (fix2 1): the skeleton decoder's cache size missed the reference triples,
/// counted `Option<u16>` parents as 2 bytes and ignored the name bytes.
#[test]
fn skeleton_cache_size_counts_every_vector() {
    use tessera_assets::Decoder;
    let s = skeleton(&tree_spec(248));
    let names: usize = (0..248).map(|i| s.name(i).map_or(0, <[u8]>::len)).sum();
    assert!(names > 0);
    // parents, reference, reference triples, two matrices, name handles, name bytes.
    let floor = 248 * (4 + 40 + 40 + 64 + 64 + 16) + names;
    let charged = tessera_anim::SkeletonDecoder.size_of(&s);
    assert!(charged >= floor, "{charged} < {floor}");
    assert_eq!(charged, s.heap_size());
}

/// Audit finding (fix2 3), the baked path: t = duration gives the last frame exactly
/// (`havok-data.md` §7.8 step 1), not a blend a hair short of it.
#[test]
fn baked_clip_at_the_duration_is_the_last_frame() {
    let s = skeleton(&SkeletonSpec::chain(3));
    let rec = interleaved_clip(8, 3, 1.0, |f, j| Transform {
        translation: [0.3 + 1.1 * f as f32, -0.7 * f as f32, 0.1 * j as f32],
        ..s.reference_local()[j]
    });
    let c = bind(&rec, &s);
    let baked = BakedClip::bake(c.clip(), usize::MAX).unwrap();
    let mut last = vec![Transform::IDENTITY; 3];
    c.clip().animation.sample_frame(7, &mut last);
    let mut out = vec![Transform::IDENTITY; 3];
    for t in [1.0, 1.5] {
        assert_eq!(baked.sample(t, &mut out), 3);
        assert_eq!(out, last, "t = {t}");
        assert_eq!(c.clip().sample(t, &mut out), 3);
        assert_eq!(out, last, "t = {t}");
    }
}
