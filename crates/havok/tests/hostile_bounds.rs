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
