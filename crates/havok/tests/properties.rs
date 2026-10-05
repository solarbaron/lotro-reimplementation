// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests over synthetic clips: sampled quaternions are unit length, sampling is
//! continuous across block boundaries, arbitrary times never panic, and damaged inputs
//! never panic.

use proptest::prelude::*;
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ClipRecordSpec, InterleavedClipSpec, SkeletonSpec,
    SplineClipSpec, encode_quat40, example_rich_record, example_skeleton_12_1,
};
use tessera_havok::{
    AnimationData, Clip, Curve, Skeleton, Tagfile, Transform, classify_clip_record, decode_quat40,
    quat_length,
};

fn quat(a: f32, b: f32, c: f32) -> [f32; 4] {
    // A rotation from three angles, as a product-free closed form (unit by construction).
    let (s1, c1) = (a * 0.5).sin_cos();
    let (s2, c2) = (b * 0.5).sin_cos();
    let (s3, c3) = (c * 0.5).sin_cos();
    [
        s1 * c2 * c3 - c1 * s2 * s3,
        c1 * s2 * c3 + s1 * c2 * s3,
        c1 * c2 * s3 - s1 * s2 * c3,
        c1 * c2 * c3 + s1 * s2 * s3,
    ]
}

/// A smooth pose function with per-track frequencies.
fn pose(seed: [f32; 3]) -> impl Fn(u32, usize) -> Transform {
    move |f, j| {
        let t = f as f32 / 60.0;
        let k = 1.0 + j as f32 * 0.37;
        Transform {
            translation: [(t * seed[0] * k).sin(), t * seed[1], 0.1 * j as f32],
            rotation: quat(t * seed[0] * k, t * seed[1], t * seed[2] * k),
            scale: [1.0 + 0.2 * (t * seed[2]).sin(), 1.0, 1.0],
        }
    }
}

fn spline_clip(frames: u32, tracks: usize, degree: u8, seed: [f32; 3]) -> Clip {
    let spec = SplineClipSpec::from_frames(frames, tracks, 1.0 / 60.0, 256, degree, pose(seed));
    Clip::parse(
        &ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes(),
    )
    .expect("synthetic clip decodes")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn sampled_quaternions_are_unit(
        frames in 2u32..700,
        tracks in 1usize..5,
        degree in prop_oneof![Just(1u8), Just(3u8)],
        seed in prop::array::uniform3(0.1f32..6.0),
        times in prop::collection::vec(-1.0f32..15.0, 1..20),
    ) {
        let clip = spline_clip(frames, tracks, degree, seed);
        let mut out = vec![Transform::IDENTITY; tracks];
        for t in times {
            prop_assert_eq!(clip.sample(t, &mut out), tracks);
            for x in &out {
                prop_assert!(x.is_finite());
                prop_assert!((quat_length(x.rotation) - 1.0).abs() < 1e-5, "{:?}", x.rotation);
            }
        }
        prop_assert!(clip.animation.check_samples_finite(&mut Vec::new()).is_ok());
    }

    #[test]
    fn continuous_across_block_boundaries(
        blocks in 2u32..5,
        extra in 1u32..200,
        tracks in 1usize..4,
        degree in prop_oneof![Just(1u8), Just(3u8)],
        seed in prop::array::uniform3(0.1f32..3.0),
    ) {
        let frames = (blocks - 1) * 255 + 1 + extra.min(254);
        let clip = spline_clip(frames, tracks, degree, seed);
        let AnimationData::Spline(sp) = &clip.animation.data else { unreachable!() };
        prop_assert!(sp.blocks.len() >= 2);
        let mut a = vec![Transform::IDENTITY; tracks];
        let mut b = vec![Transform::IDENTITY; tracks];
        for i in 0..sp.blocks.len() - 1 {
            // The last frame of block i and the first frame of block i + 1 are one frame.
            let end = &sp.blocks[i];
            end.sample((end.frames - 1) as f32, &mut a);
            sp.blocks[i + 1].sample(0.0, &mut b);
            for (x, y) in a.iter().zip(&b) {
                for k in 0..3 {
                    prop_assert!((x.translation[k] - y.translation[k]).abs() < 1e-3);
                    prop_assert!((x.scale[k] - y.scale[k]).abs() < 1e-3);
                }
                let dot: f32 = (0..4).map(|k| x.rotation[k] * y.rotation[k]).sum();
                prop_assert!(dot.abs() > 1.0 - 1e-5, "{:?} vs {:?}", x.rotation, y.rotation);
            }
            // Times just either side of the boundary give nearly the same pose.
            let tb = (i as f32 + 1.0) * 255.0 / 60.0;
            clip.sample(tb - 1e-4, &mut a);
            clip.sample(tb + 1e-4, &mut b);
            for (x, y) in a.iter().zip(&b) {
                for k in 0..3 {
                    prop_assert!((x.translation[k] - y.translation[k]).abs() < 2e-2);
                }
                let dot: f32 = (0..4).map(|k| x.rotation[k] * y.rotation[k]).sum();
                prop_assert!(dot.abs() > 0.999);
            }
        }
    }

    #[test]
    fn any_time_is_safe(t in any::<f32>(), f in any::<usize>()) {
        let clip = Clip::parse(&example_rich_record().to_bytes()).unwrap();
        let mut out = [Transform::IDENTITY; 3];
        prop_assert_eq!(clip.sample(t, &mut out), 3);
        prop_assert!(out.iter().all(Transform::is_finite));
        clip.animation.sample_frame(f, &mut out);
        prop_assert!(out.iter().all(Transform::is_finite));
        let (rf, _) = clip.reference_frame().unwrap();
        prop_assert!(rf.sample(t).iter().all(|v| v.is_finite()));
        let _ = clip.animation.sample_track(f % 5, t);
    }

    #[test]
    fn quat40_finite_and_bounded(bytes in any::<[u8; 5]>()) {
        // Unit whenever the stored components leave room for the dropped one; at worst the
        // three stored components alone (length up to √1.5).
        let q = decode_quat40(bytes);
        let len = quat_length(q);
        prop_assert!(len.is_finite() && len > 1.0 - 1e-5 && len < 1.2248, "{len}");
    }

    #[test]
    fn quat40_round_trip(a in -3.2f32..3.2, b in -3.2f32..3.2, c in -3.2f32..3.2) {
        let q = quat(a, b, c);
        let d = decode_quat40(encode_quat40(q));
        prop_assert!((quat_length(d) - 1.0).abs() < 1e-5);
        let dot: f32 = (0..4).map(|k| q[k] * d[k]).sum();
        prop_assert!(dot > 0.99999, "{q:?} -> {d:?}");
    }

    #[test]
    fn curve_evaluation_never_panics(
        degree in 0usize..6,
        knots in prop::collection::vec(any::<u8>(), 2..20),
        points in prop::collection::vec(-10.0f32..10.0, 1..16),
        dims in 1usize..5,
        u in any::<f32>(),
    ) {
        if let Some(c) = Curve::new(degree, &knots, &points, dims) {
            let mut out = [0.0f32; 4];
            c.evaluate(u, &mut out);
            let _ = c.evaluate_rotation(u);
        }
    }

    #[test]
    fn interleaved_quaternions_are_unit(
        frames in 1usize..20,
        tracks in 1usize..4,
        seed in prop::array::uniform3(0.1f32..6.0),
        t in -1.0f32..3.0,
    ) {
        let p = pose(seed);
        let transforms = (0..frames).flat_map(|f| (0..tracks).map(move |j| (f, j))).map(|(f, j)| p(f as u32, j)).collect();
        let spec = InterleavedClipSpec { duration: 1.0, tracks, transforms };
        let clip = Clip::parse(&ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec))).to_bytes()).unwrap();
        let mut out = vec![Transform::IDENTITY; tracks];
        clip.sample(t, &mut out);
        for x in &out {
            prop_assert!((quat_length(x.rotation) - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn damaged_records_never_panic(edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8), cut in any::<prop::sample::Index>()) {
        let mut b = example_rich_record().to_bytes();
        for (i, v) in edits {
            let i = i.index(b.len());
            b[i] = v;
        }
        let _ = classify_clip_record(&b, &mut Vec::new());
        let c = cut.index(b.len());
        let _ = classify_clip_record(&b[..c], &mut Vec::new());
    }

    #[test]
    fn damaged_skeletons_never_panic(edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8)) {
        for mut b in [example_skeleton_12_1(), SkeletonSpec::chain(7).to_tagfile()] {
            for (i, v) in &edits {
                let i = i.index(b.len());
                b[i] = *v;
            }
            let _ = Skeleton::parse(&b);
            let _ = Tagfile::parse(&b);
        }
    }
}

#[test]
fn every_truncation_is_an_error() {
    let rec = example_rich_record().to_bytes();
    for cut in 0..rec.len() {
        assert!(Clip::parse(&rec[..cut]).is_err(), "cut {cut}");
    }
    let t = example_skeleton_12_1();
    for cut in 0..t.len() {
        assert!(Skeleton::parse(&t[..cut]).is_err(), "cut {cut}");
    }
}
