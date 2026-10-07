// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for resource bounds on hostile input: work and memory must follow the
//! size of the input, not counts stored in it, and hand-built models with extreme numbers
//! must not panic.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BlockSpec, ClipRecordSpec, InterleavedClipSpec, MemberDef,
    SkeletonSpec, SplineClipSpec, TagfileWriter, TrackSpec, WValue, declare_base_classes,
    example_interleaved_12_6, example_spline_12_3, write_varint,
};
use tessera_havok::{
    Animation, AnimationData, Clip, CurveRef, InterleavedAnimation, ReferenceFrame,
    RotationChannel, Skeleton, SplineAnimation, SplineBlock, Tagfile, TrackChannels, Transform,
    VectorChannel, classify_clip_record,
};

/// Generous enough for an unoptimised build on a slow machine; the unbounded versions took
/// tens of seconds to hours.
const BUDGET: Duration = Duration::from_secs(10);

fn huge_frame_clip(tracks: usize) -> Vec<u8> {
    let n = i32::MAX as u32;
    let spec = SplineClipSpec {
        duration: 1.0,
        num_frames: n,
        max_frames_per_block: n,
        blocks: vec![BlockSpec {
            tracks: vec![TrackSpec::identity(); tracks],
        }],
    };
    ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes()
}

#[test]
fn a_huge_declared_frame_count_does_not_drive_classification_time() {
    for tracks in [1, 8] {
        let rec = huge_frame_clip(tracks);
        assert!(rec.len() < 1024, "record is {} bytes", rec.len());
        let clip = Clip::parse(&rec).expect("decodes");
        assert_eq!(clip.animation.frame_count(), i32::MAX as usize);
        let start = Instant::now();
        let mut scratch = Vec::new();
        let c = classify_clip_record(&rec, &mut scratch);
        let took = start.elapsed();
        assert!(c.is_ok(), "{:?}", c.error);
        assert!(took < BUDGET, "classification took {took:?}");
    }
}

#[test]
fn struct_class_lookup_is_independent_of_the_class_count() {
    let mut w = TagfileWriter::new();
    let prefix = "x".repeat(60);
    for i in 0..4000 {
        w.class(&format!("{prefix}{i:04}"), 0, 0, &[]);
    }
    let t = w.class("T", 0, 0, &[]);
    assert!(t > 4000);
    let members: Vec<_> = (0..1000)
        .map(|i| MemberDef::class(&format!("m{i}"), 25, "T"))
        .collect();
    let s = w.class("S", 0, 0, &members);
    // Objects of S with every member present as an empty struct array of T (1 byte each).
    let mut body = Vec::new();
    for _ in 0..400 {
        write_varint(&mut body, 4);
        write_varint(&mut body, s as i32);
        body.extend(std::iter::repeat_n(0xFF, 125));
        body.extend(std::iter::repeat_n(0, 1000));
    }
    w.raw(&body);
    let bytes = w.finish();
    let start = Instant::now();
    let tf = Tagfile::parse(&bytes).expect("parses");
    let took = start.elapsed();
    assert_eq!(tf.objects().len(), 400);
    assert!(took < BUDGET, "parse took {took:?}");
}

#[test]
fn struct_class_resolution_keeps_first_declared_and_forward_semantics() {
    // Two classes named "T": the first declared wins (one member, so the column decodes as
    // an int column of one element).
    let mut w = TagfileWriter::new();
    let base = declare_base_classes(&mut w);
    let holder = w.class("Holder", 0, base, &[MemberDef::class("items", 25, "T")]);
    let t1 = w.class("T", 0, 0, &[MemberDef::new("a", 2)]);
    let _t2 = w.class("T", 0, 0, &[MemberDef::new("b", 3)]);
    w.object(
        holder,
        &[(
            "items".into(),
            WValue::StructArray {
                len: 1,
                columns: vec![("a".into(), WValue::IntArray(vec![7]))],
            },
        )],
    );
    let bytes = w.finish();
    let tf = Tagfile::parse(&bytes).expect("parses");
    let root = tf.root().unwrap();
    let Some((_, tessera_havok::tagfile::Value::StructArray(sa))) = root.values.first() else {
        panic!("struct array expected: {:?}", root.values)
    };
    assert_eq!(sa.class as usize, t1);
}

#[test]
fn bone_names_share_one_copy_per_pool_string() {
    let mut s = SkeletonSpec::chain(2000);
    let long = "n".repeat(50_000);
    for b in &mut s.bones {
        b.name = Some(long.clone());
    }
    let bytes = s.to_tagfile();
    assert!(bytes.len() < 200_000);
    let sk = Skeleton::parse(&bytes).expect("decodes");
    let first = sk.bones[0].name.clone().unwrap();
    assert_eq!(first.len(), 50_000);
    assert!(
        sk.bones
            .iter()
            .all(|b| Arc::ptr_eq(b.name.as_ref().unwrap(), &first))
    );
}

#[test]
fn annotation_names_share_one_copy_per_pool_string() {
    let interleaved = InterleavedClipSpec {
        duration: 1.0,
        tracks: 64,
        transforms: vec![Transform::IDENTITY; 64 * 2],
    };
    let mut anim = AnimationSpec::new(AnimationKindSpec::Interleaved(interleaved));
    anim.annotation_names = Some(vec!["a".repeat(10_000); 64]);
    let clip = Clip::parse(&ClipRecordSpec::new(anim).to_bytes()).expect("decodes");
    let tracks = &clip.animation.annotation_tracks;
    assert_eq!(tracks.len(), 64);
    let first = tracks[0].name.clone().unwrap();
    assert!(
        tracks
            .iter()
            .all(|a| Arc::ptr_eq(a.name.as_ref().unwrap(), &first))
    );
}

#[test]
fn annotation_tracks_are_bounded_by_the_transform_track_count() {
    // §12.3 has one transform track; a column-less array of 50 elements costs 2 bytes.
    let mut anim = AnimationSpec::new(AnimationKindSpec::Spline(example_spline_12_3()));
    anim.overrides.push((
        "annotationTracks".into(),
        WValue::StructArray {
            len: 50,
            columns: vec![],
        },
    ));
    let clip = Clip::parse(&ClipRecordSpec::new(anim).to_bytes()).expect("decodes");
    assert_eq!(clip.animation.annotation_tracks.len(), 1);
    assert_eq!(clip.animation.annotation_tracks[0].name, None);

    // Fewer names than tracks keeps every name.
    let mut anim = AnimationSpec::new(AnimationKindSpec::Interleaved(example_interleaved_12_6()));
    anim.annotation_names = Some(vec!["only".into()]);
    let clip = Clip::parse(&ClipRecordSpec::new(anim).to_bytes()).expect("decodes");
    let names: Vec<_> = clip
        .animation
        .annotation_tracks
        .iter()
        .map(|a| a.name.as_deref().map(<[u8]>::to_vec))
        .collect();
    assert_eq!(names, [Some(b"only".to_vec())]);
}

#[test]
fn hand_built_interleaved_with_overflowing_counts_does_not_panic() {
    let i = InterleavedAnimation {
        tracks: usize::MAX / 2 + 1,
        frames: 3,
        duration: 1.0,
        transforms: vec![],
    };
    let mut out = [Transform::IDENTITY; 2];
    assert_eq!(i.sample_frame(2, &mut out), 0);
    assert_eq!(i.sample_frame(1, &mut out), 0);
    assert_eq!(i.sample(0.7, &mut out), 0);
}

#[test]
fn check_samples_finite_sizes_scratch_from_the_data() {
    let mut scratch = Vec::new();
    let a = Animation {
        type_code: 1,
        duration: 1.0,
        tracks: usize::MAX / 8,
        data: AnimationData::Interleaved(InterleavedAnimation {
            tracks: 1,
            frames: 1,
            duration: 1.0,
            transforms: vec![Transform::IDENTITY],
        }),
        annotation_tracks: vec![],
        extracted_motion: None,
    };
    a.check_samples_finite(&mut scratch).unwrap();
    assert_eq!(scratch.len(), 1);

    // Declared frames far beyond the stored transforms.
    let a = Animation {
        data: AnimationData::Interleaved(InterleavedAnimation {
            tracks: 2,
            frames: usize::MAX,
            duration: 1.0,
            transforms: vec![Transform::IDENTITY; 4],
        }),
        ..a
    };
    let start = Instant::now();
    a.check_samples_finite(&mut scratch).unwrap();
    assert!(start.elapsed() < BUDGET);
}

#[test]
fn hand_built_spline_block_with_curves_and_huge_frames_is_bounded() {
    let track = TrackChannels {
        position: VectorChannel::Constant([0.0; 3]),
        rotation: RotationChannel::Dynamic(CurveRef {
            degree: 1,
            last: 1,
            knot_start: 0,
            point_start: 0,
            dims: 4,
        }),
        scale: VectorChannel::Constant([1.0; 3]),
        quantisation: 0x45,
    };
    let block = SplineBlock {
        frames: u32::MAX,
        tracks: vec![track; 4],
        knots: vec![0, 0, 255, 255],
        points: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    };
    let a = Animation {
        type_code: 3,
        duration: 1.0,
        tracks: 4,
        data: AnimationData::Spline(SplineAnimation {
            num_frames: u32::MAX,
            max_frames_per_block: u32::MAX,
            block_duration: 1.0,
            block_inverse_duration: 1.0,
            frame_duration: 1.0,
            duration: 1.0,
            tracks: 4,
            blocks: vec![block],
        }),
        annotation_tracks: vec![],
        extracted_motion: None,
    };
    let start = Instant::now();
    a.check_samples_finite(&mut Vec::new()).unwrap();
    assert!(start.elapsed() < BUDGET);
}

#[test]
fn short_reference_frames_hold_their_last_sample_on_the_clip_grid() {
    let rf = ReferenceFrame {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration: 4.0,
        components: 4,
        samples: vec![[0.0; 4], [1.0, 0.0, 0.0, 0.5], [2.0, 0.0, 0.0, 1.0]],
    };
    // Clip grid: 5 frames over 4 s, one frame per second; samples 0–2 exist.
    assert_eq!(rf.sample_on_grid(0.0, 5, 4.0), [0.0; 4]);
    assert_eq!(rf.sample_on_grid(1.5, 5, 4.0), [1.5, 0.0, 0.0, 0.75]);
    assert_eq!(rf.sample_on_grid(2.0, 5, 4.0), [2.0, 0.0, 0.0, 1.0]);
    assert_eq!(rf.sample_on_grid(3.5, 5, 4.0), [2.0, 0.0, 0.0, 1.0]);
    assert_eq!(rf.sample_on_grid(99.0, 5, 4.0), [2.0, 0.0, 0.0, 1.0]);
    // §7.8 as worded stretches the three samples over the duration instead.
    assert_eq!(rf.sample(2.0), [1.0, 0.0, 0.0, 0.5]);
    // Equal counts: both agree.
    assert_eq!(rf.sample_on_grid(1.0, 3, 4.0), rf.sample(1.0));
    // Degenerate inputs.
    assert_eq!(rf.sample_on_grid(f32::NAN, 0, f32::NAN), [0.0; 4]);
    let empty = ReferenceFrame {
        samples: vec![],
        ..rf
    };
    assert_eq!(empty.sample_on_grid(1.0, 5, 4.0), [0.0; 4]);
}

/// Objects of class `s`, each with `members` struct-array members all present as an empty
/// array whose element class has `element_members` members, all present (column bitmap of
/// all ones; empty byte columns take no bytes).
fn empty_struct_array_objects(
    s: usize,
    members: usize,
    element_members: usize,
    objects: usize,
) -> Vec<u8> {
    let mut body = Vec::new();
    for _ in 0..objects {
        write_varint(&mut body, 4);
        write_varint(&mut body, s as i32);
        body.extend(std::iter::repeat_n(0xFF, members.div_ceil(8)));
        for _ in 0..members {
            body.push(0); // count 0
            body.extend(std::iter::repeat_n(0xFF, element_members.div_ceil(8)));
        }
    }
    body
}

#[test]
fn struct_class_lookup_is_independent_of_the_class_name_length() {
    // A 64 KiB element class name named by 200,000 one-byte struct-array values: resolving
    // it by its bytes each time hashed 13 GB.
    let mut w = TagfileWriter::new();
    let long = "T".repeat(64 * 1024);
    let t = w.class(&long, 0, 0, &[]);
    let members: Vec<_> = (0..1000)
        .map(|i| MemberDef::class(&format!("m{i}"), 25, &long))
        .collect();
    let s = w.class("S", 0, 0, &members);
    w.raw(&empty_struct_array_objects(s, 1000, 0, 200));
    let bytes = w.finish();
    assert!(bytes.len() < 400_000, "input is {} bytes", bytes.len());
    let start = Instant::now();
    let tf = Tagfile::parse(&bytes).expect("parses");
    let took = start.elapsed();
    assert_eq!(tf.objects().len(), 200);
    let Some((_, tessera_havok::tagfile::Value::StructArray(sa))) =
        tf.root().unwrap().values.first()
    else {
        panic!("struct array expected")
    };
    assert_eq!(sa.class as usize, t);
    assert!(took < BUDGET, "parse took {took:?}");
}

#[test]
fn class_declarations_sharing_one_long_name_hash_it_once() {
    // 4,000 metadata entries whose names all refer to one 1 MiB pool string (pool index 2,
    // the first string written).
    let mut w = TagfileWriter::new();
    let long = "C".repeat(1 << 20);
    let first = w.class(&long, 0, 0, &[]);
    let mut body = Vec::new();
    for _ in 0..4000 {
        for v in [2, -2, 0, 0, 0] {
            write_varint(&mut body, v);
        }
    }
    w.raw(&body);
    let bytes = w.finish();
    let start = Instant::now();
    let tf = Tagfile::parse(&bytes).expect("parses");
    let took = start.elapsed();
    assert_eq!(tf.classes().len(), first + 4001);
    assert!(took < BUDGET, "parse took {took:?}");
}

#[test]
fn empty_struct_array_columns_are_not_stored() {
    // Every column of a count-0 struct array is present but empty: none is kept (each equals
    // the absent member's default, §3.10), so presence bits cost no memory.
    let mut w = TagfileWriter::new();
    let element: Vec<_> = (0..1024)
        .map(|i| MemberDef::new(&format!("e{i}"), 1))
        .collect();
    let e = w.class("E", 0, 0, &element);
    let members: Vec<_> = (0..1024)
        .map(|i| MemberDef::class(&format!("m{i}"), 25, "E"))
        .collect();
    let s = w.class("S", 0, 0, &members);
    w.raw(&empty_struct_array_objects(s, 1024, 1024, 4));
    let bytes = w.finish();
    let tf = Tagfile::parse(&bytes).expect("parses");
    assert_eq!(tf.objects().len(), 4);
    for o in tf.objects() {
        assert_eq!(o.values.len(), 1024);
        for (_, v) in &o.values {
            let tessera_havok::tagfile::Value::StructArray(sa) = v else {
                panic!("struct array expected")
            };
            assert_eq!((sa.class as usize, sa.len), (e, 0));
            assert!(sa.columns.is_empty());
        }
    }

    // Prefixed columns are still read and checked when empty: a vec4 column of a count-0
    // array with component count 9 is unsupported.
    let mut w = TagfileWriter::new();
    let v = w.class("V", 0, 0, &[MemberDef::new("v", 4)]);
    let h = w.class("H", 0, 0, &[MemberDef::class("a", 25, "V")]);
    let mut body = Vec::new();
    write_varint(&mut body, 4);
    write_varint(&mut body, h as i32);
    body.extend([0x01, 0x00, 0x01]); // bitmap, count 0, column bitmap
    write_varint(&mut body, 9);
    w.raw(&body);
    let bytes = w.finish();
    assert!(v > 0);
    let e = Tagfile::parse(&bytes).unwrap_err();
    assert_eq!(
        e.category(),
        tessera_havok::FailureCategory::TagfileUnsupportedForm
    );
}

#[test]
fn decoded_values_are_capped() {
    // Present byte members cost one input byte each but decode to a much larger value; the
    // total is capped at MAX_VALUES.
    let mut w = TagfileWriter::new();
    let members: Vec<_> = (0..1024)
        .map(|i| MemberDef::new(&format!("b{i}"), 1))
        .collect();
    let c = w.class("B", 0, 0, &members);
    let objects = tessera_havok::tagfile::MAX_VALUES / 1024 + 1;
    let mut body = Vec::new();
    for _ in 0..objects {
        write_varint(&mut body, 4);
        write_varint(&mut body, c as i32);
        body.extend(std::iter::repeat_n(0xFF, 128));
        body.extend(std::iter::repeat_n(7, 1024));
    }
    w.raw(&body);
    let bytes = w.finish();
    let e = Tagfile::parse(&bytes).unwrap_err();
    assert_eq!(
        e.category(),
        tessera_havok::FailureCategory::TagfileUnsupportedForm
    );
}

#[test]
fn interleaved_blends_of_extreme_finite_frames_stay_finite() {
    // §7.8 blends a + α(b − a); b − a overflows here although both frames are finite.
    let frame = |x: f32, s: f32| Transform {
        translation: [x, 0.0, 0.0],
        scale: [s; 3],
        ..Transform::IDENTITY
    };
    let spec = InterleavedClipSpec {
        duration: 1.0,
        tracks: 1,
        transforms: vec![frame(-2e38, 2e38), frame(2e38, -2e38), frame(2e38, 2e38)],
    };
    let rec =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec))).to_bytes();
    let c = classify_clip_record(&rec, &mut Vec::new());
    assert!(c.is_ok(), "{:?}", c.error);
    let clip = Clip::parse(&rec).expect("decodes");
    let mut out = [Transform::IDENTITY];
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(clip.sample(t, &mut out), 1);
        assert!(out[0].is_finite(), "t = {t}: {:?}", out[0]);
        let one = clip.animation.sample_track(0, t).unwrap();
        assert!(one.is_finite(), "t = {t}: {one:?}");
    }
    // Frame times give the frames exactly.
    clip.sample(0.0, &mut out);
    assert_eq!(out[0].translation[0], -2e38);
    clip.sample(0.5, &mut out);
    assert_eq!((out[0].translation[0], out[0].scale[0]), (2e38, -2e38));

    let rf = ReferenceFrame {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration: 1.0,
        components: 4,
        samples: vec![[-3e38; 4], [3e38; 4]],
    };
    for t in [0.0, 0.5, 1.0] {
        assert!(rf.sample(t).iter().all(|v| v.is_finite()));
        assert!(rf.sample_on_grid(t, 2, 1.0).iter().all(|v| v.is_finite()));
    }
}

#[test]
fn spline_helpers_reject_overflowing_arguments() {
    // Documented: a too-short knot vector clamps the span; bad arguments return false.
    assert_eq!(tessera_havok::find_span(&[0, 1], usize::MAX, 0, 0.0), 0);
    assert_eq!(tessera_havok::find_span(&[0, 1], usize::MAX, 3, 0.0), 3);
    let mut out = [0.0f32; 2];
    assert!(!tessera_havok::basis_functions(
        &[0, 0, 1, 1],
        usize::MAX,
        1,
        0.0,
        &mut out
    ));
    assert!(tessera_havok::basis_functions(
        &[0, 0, 1, 1],
        1,
        1,
        0.5,
        &mut out
    ));
    assert_eq!(out, [0.5, 0.5]);
}

/// Audit finding (fix2 2): a curve whose control points are all finite but near `f32::MAX`
/// summed to infinity between frames (basis weights summing a hair above 1), while the
/// §9.2 check, which samples whole frames only, passed.
#[test]
fn spline_blends_of_extreme_finite_points_stay_finite_between_frames() {
    use tessera_havok::testkit::{RotationSpec, VectorSpec};
    let big = |v: f32| {
        VectorSpec::spline(
            3,
            vec![0, 0, 0, 0, 4, 4, 4, 4],
            &[[v, -v, v]; 4],
            0b111,
            [None; 3],
        )
    };
    let spec = SplineClipSpec {
        duration: 4.0 / 30.0,
        num_frames: 5,
        max_frames_per_block: 256,
        blocks: vec![BlockSpec {
            tracks: vec![TrackSpec::new(
                big(f32::MAX),
                RotationSpec::Identity,
                big(f32::MAX),
            )],
        }],
    };
    let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
    let clip = Clip::parse(&rec).expect("decodes");
    assert!(clip.animation.check_samples_finite(&mut Vec::new()).is_ok());
    let mut out = [Transform::IDENTITY];
    let d = clip.animation.duration;
    for i in 0..=4000 {
        let t = d * i as f32 / 4000.0;
        assert_eq!(clip.sample(t, &mut out), 1);
        assert!(out[0].is_finite(), "t = {t}: {:?}", out[0]);
        // Within the window's range (all points equal): f32::MAX up to rounding.
        for v in out[0].translation.iter().chain(&out[0].scale) {
            assert!((v.abs() / f32::MAX - 1.0).abs() < 1e-5, "t = {t}: {v}");
        }
    }
}

/// Audit finding (fix2 3): §7.8 step 1 and §4.5 step 1, t ≥ D gives the last frame exactly;
/// t / (D / (N − 1)) rounded just below N − 1 for many (D, N), and a duration of 0 gave the
/// first frame.
#[test]
fn sampling_at_the_duration_gives_the_last_frame_exactly() {
    let frame = |f: usize| Transform {
        translation: [0.3 + 1.1 * f as f32, -0.7 * f as f32, 0.1],
        rotation: tessera_havok::testkit::quat_from_angles(0.37 * f as f32, 0.1, -0.2),
        scale: [1.0 + 0.013 * f as f32; 3],
    };
    let normalised = |q: [f32; 4]| tessera_havok::quat_nlerp(q, q, 0.0);
    for n in [8usize, 14, 31] {
        for d in [1.0f32, 0.7, 2.3, 1.0 / 3.0] {
            let anim = InterleavedAnimation {
                tracks: 1,
                frames: n,
                duration: d,
                transforms: (0..n).map(frame).collect(),
            };
            let last = Transform {
                rotation: normalised(frame(n - 1).rotation),
                ..frame(n - 1)
            };
            let mut out = [Transform::IDENTITY];
            for t in [d, d * 2.0, d + 1e3] {
                assert_eq!(anim.sample(t, &mut out), 1);
                assert_eq!(out[0], last, "n = {n}, d = {d}, t = {t}");
            }
            let whole = Animation {
                type_code: 1,
                duration: d,
                tracks: 1,
                data: AnimationData::Interleaved(anim.clone()),
                annotation_tracks: Vec::new(),
                extracted_motion: None,
            };
            assert_eq!(whole.sample_track(0, d), Some(last), "n = {n}, d = {d}");

            let samples: Vec<[f32; 4]> = (0..n)
                .map(|f| [0.2 + 0.31 * f as f32, 0.0, 1.0, 0.11 * f as f32])
                .collect();
            let rf = ReferenceFrame {
                up: [0.0, 0.0, 1.0, 0.0],
                forward: [1.0, 0.0, 0.0, 0.0],
                duration: d,
                components: 4,
                samples: samples.clone(),
            };
            assert_eq!(rf.sample(d), samples[n - 1], "n = {n}, d = {d}");
            assert_eq!(
                rf.sample_on_grid(d, n, d),
                samples[n - 1],
                "n = {n}, d = {d}"
            );
        }
    }
    // A duration of 0 (or not finite): every t is at or past the end.
    let anim = InterleavedAnimation {
        tracks: 1,
        frames: 3,
        duration: 0.0,
        transforms: (0..3).map(frame).collect(),
    };
    let mut out = [Transform::IDENTITY];
    anim.sample(0.0, &mut out);
    assert_eq!(out[0].translation, frame(2).translation);
}
