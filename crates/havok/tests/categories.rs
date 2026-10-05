// SPDX-License-Identifier: GPL-3.0-or-later
//! Every failure category of `havok-data.md` §9.3 is reachable from synthetic input, and the
//! decoder reports the right one.

use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BlockSpec, ClipRecordSpec, MemberDef, RotationSpec,
    SkeletonSpec, SplineClipSpec, TagfileWriter, TrackSpec, VectorSpec, WValue,
    declare_base_classes, example_interleaved_12_6, example_rich_record, example_skeleton_12_1,
    example_spline_12_3,
};
use tessera_havok::{
    BlockParams, Clip, FailureCategory as C, ReferenceFrame, Skeleton, SplineBlock, Tagfile,
    Transform, classify_clip_record, classify_skeleton_tagfile,
};

fn clip_cat(bytes: &[u8]) -> Option<C> {
    classify_clip_record(bytes, &mut Vec::new()).failure()
}

fn tag_cat(bytes: &[u8]) -> Option<C> {
    Tagfile::parse(bytes).err().map(|e| e.category())
}

fn spline_record(edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let spec = example_spline_12_3();
    let (mut data, _, _) = spec.data();
    edit(&mut data);
    let mut anim = AnimationSpec::new(AnimationKindSpec::Spline(spec));
    anim.overrides.push(("data".into(), WValue::Bytes(data)));
    ClipRecordSpec::new(anim).to_bytes()
}

fn with_override(name: &str, v: WValue) -> Vec<u8> {
    let mut anim = AnimationSpec::new(AnimationKindSpec::Spline(example_spline_12_3()));
    anim.overrides.push((name.into(), v));
    ClipRecordSpec::new(anim).to_bytes()
}

#[test]
fn envelope_categories() {
    let good = example_rich_record().to_bytes();
    assert_eq!(clip_cat(&good), None);
    assert_eq!(clip_cat(&good[..3]), Some(C::EnvelopeTruncated));
    assert_eq!(
        clip_cat(&good[..good.len() - 1]),
        Some(C::EnvelopeTruncated)
    );
    let mut b = good.clone();
    b[8] = 2;
    assert_eq!(clip_cat(&b), Some(C::EnvelopeBadFlag));
    let mut b = good.clone();
    b.push(0);
    assert_eq!(clip_cat(&b), Some(C::EnvelopeTrailingBytes));
    let mut r = example_rich_record();
    r.weights = vec![1.0];
    assert_eq!(clip_cat(&r.to_bytes()), Some(C::EnvelopeWeightCount));
    // A tagfile length past the end.
    let mut r = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(
        example_spline_12_3(),
    )))
    .to_bytes();
    r[9..13].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(clip_cat(&r), Some(C::EnvelopeTruncated));
}

#[test]
fn container_header_categories() {
    let good = example_skeleton_12_1();
    let mut b = good.clone();
    b[..8].copy_from_slice(&[0xCA, 0xB0, 0x0D, 0x1E, 0xD0, 0x11, 0xFA, 0xCE]);
    assert_eq!(tag_cat(&b), Some(C::TagfileBadMagic));
    assert_eq!(tag_cat(b"hello world"), Some(C::TagfileBadMagic));
    assert_eq!(tag_cat(&good[..5]), Some(C::TagfileTruncated));
    let mut b = good.clone();
    b[9] = 0x08; // version 4
    assert_eq!(tag_cat(&b), Some(C::TagfileBadVersion));
    let mut w = TagfileWriter::empty();
    w.raw(&tessera_havok::tagfile::MAGIC).varint(2);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadVersion));
    let mut w = TagfileWriter::new();
    w.varint(1).varint(3);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadVersion));
    assert_eq!(tag_cat(&good[..good.len() - 1]), Some(C::TagfileTruncated));
    let mut b = good.clone();
    b.push(0);
    assert_eq!(tag_cat(&b), Some(C::TagfileTrailingBytes));
    for t in [0, 3, 5, 6, 8, -1] {
        let mut w = TagfileWriter::new();
        w.varint(t);
        assert_eq!(tag_cat(&w.finish()), Some(C::TagfileUnknownTag), "tag {t}");
    }
}

#[test]
fn varint_and_pool_categories() {
    let mut w = TagfileWriter::new();
    w.raw(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadVarint));
    let mut w = TagfileWriter::new();
    w.raw(&[0x80, 0x80, 0x80, 0x80, 0x7F]); // magnitude ≥ 2^31
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadVarint));
    // Negative member count.
    let mut w = TagfileWriter::new();
    w.varint(2).string("A").varint(0).varint(0).varint(-3);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadVarint));
    // Huge member count.
    let mut w = TagfileWriter::new();
    w.varint(2)
        .string("A")
        .varint(0)
        .varint(0)
        .varint(1_000_000);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileTruncated));
    // String reference beyond the pool.
    let mut w = TagfileWriter::new();
    w.varint(2).varint(-50);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadStringRef));
}

#[test]
fn metadata_and_object_categories() {
    let mut w = TagfileWriter::new();
    w.varint(2).string("A").varint(0).varint(5).varint(0);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadClassIndex));
    for code in [11, 0x30, 0x40, -2] {
        let mut w = TagfileWriter::new();
        w.class("A", 0, 0, &[MemberDef::new("m", code)]);
        assert_eq!(
            tag_cat(&w.finish()),
            Some(C::TagfileBadMemberType),
            "code {code}"
        );
    }
    for class in [0, 7, -1] {
        let mut w = TagfileWriter::new();
        w.class("A", 0, 0, &[]);
        w.varint(4).varint(class);
        assert_eq!(
            tag_cat(&w.finish()),
            Some(C::TagfileBadClassIndex),
            "class {class}"
        );
    }
    // Struct array of an undeclared class.
    let mut w = TagfileWriter::new();
    let a = w.class("A", 0, 0, &[MemberDef::class("s", 25, "Missing")]);
    w.varint(4).varint(a as i32).raw(&[1]).varint(1);
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileBadClassIndex));
    // Late class-name resolution works: the element class is declared after its user.
    let mut w = TagfileWriter::new();
    let a = w.class("A", 0, 0, &[MemberDef::class("s", 25, "B")]);
    w.class("B", 0, 0, &[MemberDef::new("x", 2)]);
    w.object(
        a,
        &[(
            "s".into(),
            WValue::StructArray {
                len: 2,
                columns: vec![("x".into(), WValue::IntArray(vec![5, -5]))],
            },
        )],
    );
    assert_eq!(tag_cat(&w.finish()), None);
    // Unresolved and forward references.
    for (r, cat) in [
        (2, Some(C::TagfileUnresolvedRef)),
        (-1, Some(C::TagfileUnresolvedRef)),
        (1, None),
        (0, None),
    ] {
        let mut w = TagfileWriter::new();
        let a = w.class("A", 0, 0, &[MemberDef::class("o", 8, "A")]);
        w.object(a, &[("o".into(), WValue::Object(r))]);
        assert_eq!(tag_cat(&w.finish()), cat, "reference {r}");
    }
}

#[test]
fn unsupported_forms() {
    let cases: Vec<(MemberDef, WValue)> = vec![
        (MemberDef::new("v", 0), WValue::Int(0)),
        (
            MemberDef {
                tuple_size: 2,
                ..MemberDef::new("t", 0x22)
            },
            WValue::Int(0),
        ),
        (MemberDef::new("v8", 5), WValue::Int(0)),
        (MemberDef::new("v16a", 0x17), WValue::Int(0)),
        (MemberDef::new("oa", 0x18), WValue::Int(0)),
        (MemberDef::class("st", 9, "A"), WValue::Int(0)),
        (
            MemberDef::new("k0", 0x14),
            WValue::Vec4Array {
                components: 0,
                values: vec![[0.0; 4]],
            },
        ),
    ];
    for (m, v) in cases {
        let mut w = TagfileWriter::new();
        let a = w.class("A", 0, 0, std::slice::from_ref(&m));
        w.object(a, &[(m.name.clone(), v)]);
        assert_eq!(
            tag_cat(&w.finish()),
            Some(C::TagfileUnsupportedForm),
            "{m:?}"
        );
    }
    // Annotation tracks with an `annotations` column (§12.7).
    let mut anim = AnimationSpec::new(AnimationKindSpec::Interleaved(example_interleaved_12_6()));
    anim.overrides.push((
        "annotationTracks".into(),
        WValue::StructArray {
            len: 2,
            columns: vec![
                (
                    "trackName".into(),
                    WValue::CStringArray(vec!["t0".into(), "t1".into()]),
                ),
                ("annotations".into(), WValue::IntArray(vec![0, 0])),
            ],
        },
    ));
    assert_eq!(
        clip_cat(&ClipRecordSpec::new(anim).to_bytes()),
        Some(C::TagfileUnsupportedForm)
    );
    // Deep inheritance hits the resource limit.
    let mut w = TagfileWriter::new();
    let mut parent = 0;
    for i in 0..70 {
        parent = w.class(&format!("C{i}"), 0, parent, &[]);
    }
    assert_eq!(tag_cat(&w.finish()), Some(C::TagfileUnsupportedForm));
}

#[test]
fn object_categories() {
    let clip_tagfile =
        AnimationSpec::new(AnimationKindSpec::Spline(example_spline_12_3())).to_tagfile();
    let e = Skeleton::parse(&clip_tagfile).unwrap_err();
    assert_eq!(e.category(), C::ObjectMissing);
    let mut w = TagfileWriter::new();
    declare_base_classes(&mut w);
    assert_eq!(
        classify_skeleton_tagfile(&w.finish()).failure(),
        Some(C::ObjectMissing)
    );
    let mut w = TagfileWriter::new();
    let a = w.class("someOtherClass", 0, 0, &[]);
    w.object(a, &[]);
    let t = w.finish();
    assert_eq!(
        classify_skeleton_tagfile(&t).failure(),
        Some(C::ObjectUnknownClass)
    );
    // A clip whose animation slot holds a skeleton.
    let mut r = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
        example_interleaved_12_6(),
    )))
    .to_bytes();
    let skel = SkeletonSpec::chain(2).to_tagfile();
    let tlen = u32::from_le_bytes(r[9..13].try_into().unwrap()) as usize;
    r.splice(
        9..13 + tlen,
        (skel.len() as u32)
            .to_le_bytes()
            .into_iter()
            .chain(skel.iter().copied()),
    );
    assert_eq!(clip_cat(&r), Some(C::ObjectMissing));
    // Leading reference frame tagfile that is not one.
    assert_eq!(
        ReferenceFrame::parse(&skel).unwrap_err().category(),
        C::ObjectMissing
    );

    // Type mismatch: numFrames declared real.
    let mut w = TagfileWriter::new();
    let a = w.class(
        "hkaSplineCompressedAnimation",
        0,
        0,
        &[MemberDef::new("numFrames", 3)],
    );
    w.object(a, &[("numFrames".into(), WValue::Real(5.0))]);
    assert_eq!(
        tessera_havok::Animation::parse(&w.finish())
            .unwrap_err()
            .category(),
        C::ObjectTypeMismatch
    );
    let mut s = SkeletonSpec::chain(3);
    s.overrides.push((
        "referencePose".into(),
        WValue::Vec12Array(vec![[0.0; 12]; 3]),
    ));
    let mut w = TagfileWriter::new();
    let k = w.class("hkaSkeleton", 3, 0, &[MemberDef::new("parentIndices", 19)]);
    w.object(k, &[("parentIndices".into(), WValue::RealArray(vec![0.0]))]);
    assert_eq!(
        classify_skeleton_tagfile(&w.finish()).failure(),
        Some(C::ObjectTypeMismatch)
    );

    // Inconsistent skeletons.
    assert_eq!(
        classify_skeleton_tagfile(&s.to_tagfile()).failure(),
        Some(C::ObjectInconsistent)
    );
    let mut s = SkeletonSpec::chain(3);
    s.bones[1].parent = 1;
    assert_eq!(
        classify_skeleton_tagfile(&s.to_tagfile()).failure(),
        Some(C::ObjectInconsistent)
    );
    let mut s = SkeletonSpec::chain(3);
    s.overrides
        .push(("parentIndices".into(), WValue::IntArray(vec![-1, 0])));
    assert_eq!(
        classify_skeleton_tagfile(&s.to_tagfile()).failure(),
        Some(C::ObjectInconsistent)
    );
    let mut s = SkeletonSpec::chain(3);
    s.bones[2].reference.translation[0] = f32::NAN;
    assert_eq!(
        classify_skeleton_tagfile(&s.to_tagfile()).failure(),
        Some(C::ObjectInconsistent)
    );
    assert_eq!(
        classify_skeleton_tagfile(&SkeletonSpec::chain(0).to_tagfile()).failure(),
        Some(C::ObjectInconsistent)
    );

    // Inconsistent clips.
    assert_eq!(
        clip_cat(&with_override("numBlocks", WValue::Int(2))),
        Some(C::ObjectInconsistent)
    );
    assert_eq!(
        clip_cat(&with_override("maskAndQuantizationSize", WValue::Int(8))),
        Some(C::ObjectInconsistent)
    );
    assert_eq!(
        clip_cat(&with_override("blockOffsets", WValue::IntArray(vec![16]))),
        Some(C::ObjectInconsistent)
    );
    assert_eq!(
        clip_cat(&with_override(
            "floatBlockOffsets",
            WValue::IntArray(vec![])
        )),
        Some(C::ObjectInconsistent)
    );
    assert_eq!(
        clip_cat(&with_override("maxFramesPerBlock", WValue::Int(1))),
        Some(C::ObjectInconsistent)
    );
    let mut r = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(
        example_spline_12_3(),
    )));
    r.envelope_track_count = Some(2);
    assert_eq!(clip_cat(&r.to_bytes()), Some(C::ObjectInconsistent));
    let mut a = AnimationSpec::new(AnimationKindSpec::Spline(example_spline_12_3()));
    a.overrides
        .push(("extractedMotion".into(), WValue::Object(1)));
    assert_eq!(
        clip_cat(&ClipRecordSpec::new(a).to_bytes()),
        Some(C::ObjectInconsistent)
    );
}

#[test]
fn spline_categories() {
    assert_eq!(
        clip_cat(&with_override("numberOfFloatTracks", WValue::Int(1))),
        Some(C::SplineFloatTracks)
    );
    // Rotation quantisation 2 (48-bit, not implemented).
    assert_eq!(
        clip_cat(&spline_record(|d| d[0] = 0x49)),
        Some(C::SplineUnsupportedQuantisation)
    );
    // Position quantisation 2.
    assert_eq!(
        clip_cat(&spline_record(|d| d[0] = 0x46)),
        Some(C::SplineUnsupportedQuantisation)
    );
    // Position x both static and dynamic; unused bit 3.
    assert_eq!(
        clip_cat(&spline_record(|d| d[1] = 0x31)),
        Some(C::SplineBadMask)
    );
    assert_eq!(
        clip_cat(&spline_record(|d| d[1] = 0x29)),
        Some(C::SplineBadMask)
    );
    // Degree 0; degree above n; knots decreasing; not clamped; beyond the last frame.
    assert_eq!(
        clip_cat(&spline_record(|d| d[6] = 0)),
        Some(C::SplineBadCurve)
    );
    assert_eq!(
        clip_cat(&spline_record(|d| d[6] = 2)),
        Some(C::SplineBadCurve)
    );
    assert_eq!(
        clip_cat(&spline_record(|d| d[8] = 3)),
        Some(C::SplineBadCurve)
    );
    assert_eq!(
        clip_cat(&spline_record(|d| d[7] = 1)),
        Some(C::SplineBadCurve)
    );
    assert_eq!(
        clip_cat(&spline_record(|d| d[9..11].copy_from_slice(&[9, 9]))),
        Some(C::SplineBadCurve)
    );
    // n + 1 > F_b.
    assert_eq!(
        clip_cat(&spline_record(|d| d[4] = 9)),
        Some(C::SplineBadCurve)
    );
    // Rotation channel turned static: parsing ends early.
    assert_eq!(
        clip_cat(&spline_record(|d| d[2] = 0x0F)),
        Some(C::SplineBlockLength)
    );

    let spec = example_spline_12_3();
    let (data, _, lens) = spec.data();
    let params = BlockParams {
        block: 0,
        tracks: 1,
        float_tracks: 0,
        frames: 5,
        transform_length: lens[0] as usize,
    };
    assert!(SplineBlock::parse(&data, &params).is_ok());
    for cut in [0, 2, 4, 11, 20, 30, 44] {
        let e = SplineBlock::parse(&data[..cut], &params).unwrap_err();
        assert_eq!(e.category(), C::SplineBlockOverrun, "cut {cut}");
    }
    let e = SplineBlock::parse(
        &data,
        &BlockParams {
            transform_length: 52,
            ..params
        },
    )
    .unwrap_err();
    assert_eq!(e.category(), C::SplineBlockLength);
    let e = SplineBlock::parse(
        &data,
        &BlockParams {
            tracks: usize::MAX,
            ..params
        },
    )
    .unwrap_err();
    assert_eq!(e.category(), C::SplineBlockOverrun);
}

/// A one-block, one-track spline clip of `frames` frames.
fn one_track_clip(frames: u32, track: TrackSpec) -> Vec<u8> {
    let spec = SplineClipSpec {
        duration: (frames - 1) as f32 / 30.0,
        num_frames: frames,
        max_frames_per_block: 256,
        blocks: vec![BlockSpec {
            tracks: vec![track],
        }],
    };
    ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes()
}

fn rotation_curve(degree: u8, knots: Vec<u8>) -> TrackSpec {
    let points = knots.len() - usize::from(degree) - 1;
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let points = (0..points)
        .map(|k| {
            if k % 2 == 0 {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [0.0, 0.0, h, h]
            }
        })
        .collect();
    TrackSpec::new(
        VectorSpec::DEFAULT,
        RotationSpec::Dynamic {
            degree,
            knots,
            points,
        },
        VectorSpec::DEFAULT,
    )
}

fn position_curve(degree: u8, knots: Vec<u8>) -> TrackSpec {
    let points: Vec<[f32; 3]> = (0..knots.len() - usize::from(degree) - 1)
        .map(|k| [k as f32, 0.0, 0.0])
        .collect();
    TrackSpec::new(
        VectorSpec::spline(degree, knots, &points, 0b001, [None; 3]),
        RotationSpec::Identity,
        VectorSpec::DEFAULT,
    )
}

#[test]
fn interior_knot_multiplicity_above_the_degree_is_a_bad_curve() {
    // §5.4 / HV11: interior knots repeat at most p times. Degree 1, knot 2 twice: the
    // rotation would jump at frame 2.
    let rec = one_track_clip(5, rotation_curve(1, vec![0, 0, 2, 2, 4, 4]));
    assert_eq!(clip_cat(&rec), Some(C::SplineBadCurve));
    // Degree 3, knot 2 four times.
    let rec = one_track_clip(
        8,
        position_curve(3, vec![0, 0, 0, 0, 2, 2, 2, 2, 7, 7, 7, 7]),
    );
    assert_eq!(clip_cat(&rec), Some(C::SplineBadCurve));
    // At most p repeats is fine: degree 1 with distinct interior knots, degree 3 with a
    // triple interior knot.
    let rec = one_track_clip(5, rotation_curve(1, vec![0, 0, 2, 3, 4, 4]));
    assert_eq!(clip_cat(&rec), None);
    let rec = one_track_clip(8, position_curve(3, vec![0, 0, 0, 0, 2, 2, 2, 7, 7, 7, 7]));
    assert_eq!(clip_cat(&rec), None);
}

#[test]
fn degrees_above_the_resource_limit_are_a_bad_curve() {
    let max = tessera_havok::MAX_DEGREE;
    assert!(max < 15, "the limit is below the 16-frame block used here");
    let (max, above) = (max as u8, (max + 1) as u8);
    let clamped = |p: u8, last: u8| {
        let mut k = vec![0; usize::from(p) + 1];
        k.extend(std::iter::repeat_n(last, usize::from(p) + 1));
        k
    };
    // Degree MAX_DEGREE with n = p control points - 1: accepted.
    let rec = one_track_clip(16, rotation_curve(max, clamped(max, 15)));
    assert_eq!(clip_cat(&rec), None);
    let rec = one_track_clip(16, position_curve(max, clamped(max, 15)));
    assert_eq!(clip_cat(&rec), None);
    // One more: rejected before control points are read.
    let rec = one_track_clip(16, rotation_curve(above, clamped(above, 15)));
    assert_eq!(clip_cat(&rec), Some(C::SplineBadCurve));
    let rec = one_track_clip(16, position_curve(above, clamped(above, 15)));
    assert_eq!(clip_cat(&rec), Some(C::SplineBadCurve));
    // The u8 maximum, 255 (256 control points over a 256-frame block).
    let rec = one_track_clip(256, rotation_curve(255, clamped(255, 255)));
    assert_eq!(clip_cat(&rec), Some(C::SplineBadCurve));
}

#[test]
fn interleaved_and_sampling_categories() {
    let mut spec = example_interleaved_12_6();
    spec.tracks = 4;
    let rec =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec))).to_bytes();
    assert_eq!(clip_cat(&rec), Some(C::InterleavedBadShape));
    let mut spec = example_interleaved_12_6();
    spec.transforms.clear();
    let rec =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec))).to_bytes();
    assert_eq!(clip_cat(&rec), Some(C::InterleavedBadShape));
    let mut spec = example_interleaved_12_6();
    spec.transforms[3] = Transform {
        translation: [f32::INFINITY, 0.0, 0.0],
        ..Transform::IDENTITY
    };
    let rec =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec))).to_bytes();
    assert!(Clip::parse(&rec).is_ok());
    assert_eq!(clip_cat(&rec), Some(C::SampleNonFinite));
    // A NaN dequantisation range in a spline channel.
    let rec = spline_record(|d| d[20..24].copy_from_slice(&f32::NAN.to_le_bytes()));
    assert_eq!(clip_cat(&rec), Some(C::SampleNonFinite));
}

#[test]
fn category_names_match_the_spec() {
    let names: Vec<&str> = C::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(names.len(), 27);
    for (c, n) in C::ALL.iter().zip(&names) {
        assert_eq!(C::from_name(n), Some(*c));
        assert!(n.contains('.'));
    }
}
