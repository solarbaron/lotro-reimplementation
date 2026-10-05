// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked examples of `havok-data.md` §12, built with the crate's own testkit,
//! plus round trips through every structure kind.

use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ClipRecordSpec, SkeletonSpec, SplineClipSpec, encode_block,
    encode_quat40, example_interleaved_12_6, example_record_12_8, example_reference_frame_12_7,
    example_rich_record, example_skeleton_12_1, example_spline_12_3, write_varint,
};
use tessera_havok::{
    AnimationData, Clip, Curve, Envelope, ReferenceFrame, ReferenceFrameSource, Skeleton,
    StructureKind, Transform, classify_clip_record, classify_setup_record, decode_quat40,
    quat_length,
};

const H: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn assert_vec(a: &[f32], b: &[f32], tol: f32) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert!(close(*x, *y, tol), "{a:?} vs {b:?}");
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[test]
fn varint_examples_3_2() {
    for (v, bytes) in [
        (0, "00"),
        (1, "02"),
        (-1, "03"),
        (-2, "05"),
        (36, "48"),
        (81, "A2 01"),
        (256, "80 04"),
        (13_200, "A0 CE 01"),
    ] {
        let mut out = Vec::new();
        write_varint(&mut out, v);
        assert_eq!(out, hex(bytes), "varint {v}");
    }
}

#[test]
fn minimal_skeleton_12_1() {
    let t = example_skeleton_12_1();
    assert_eq!(t.len(), 264);
    let mut expected = hex("1E 0D B0 CA CE FA 11 D0 02 06 04 16");
    expected.extend_from_slice(b"hkaSkeleton");
    expected.extend_from_slice(&hex("06 00 06 1A"));
    assert_eq!(&t[..expected.len()], &expected[..]);
    // The hkaBone metadata entry references the pool entry 5 created by the member.
    let pos = t
        .windows(5)
        .position(|w| w == hex("04 0B 00 00 04"))
        .unwrap();
    assert!(pos > 40);
    assert!(t.windows(3).any(|w| w == hex("08 02 07")));
    assert!(t.windows(5).any(|w| w == hex("06 08 03 00 02")));
    assert!(t.windows(4).any(|w| w == hex("F3 04 35 3F")));
    assert_eq!(*t.last().unwrap(), 0x0E);

    let s = Skeleton::parse(&t).unwrap();
    assert_eq!(s.bone_count(), 3);
    let names: Vec<_> = s
        .bones
        .iter()
        .map(|b| b.name.as_deref().unwrap().to_vec())
        .collect();
    assert_eq!(names, [b"b0".to_vec(), b"b1".to_vec(), b"b2".to_vec()]);
    assert_eq!(
        s.bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
        [None, Some(0), Some(1)]
    );
    assert_eq!(s.bones[1].reference.translation, [0.0, 0.0, 1.0]);
    assert_vec(&s.bones[2].reference.rotation, &[0.0, 0.0, H, H], 1e-7);
    assert!(
        s.bones
            .iter()
            .all(|b| !b.lock_translation && b.reference.scale == [1.0; 3])
    );
    assert_eq!(s.find_bone(b"b2"), Some(2));
}

#[test]
fn quat40_examples_12_2() {
    let cases: [([f32; 4], &str, [f32; 4]); 4] = [
        ([0.0, 0.0, 0.0, 1.0], "FF F7 7F FF 37", [0.0, 0.0, 0.0, 1.0]),
        ([0.0, 0.0, H, H], "FF F7 7F FE 2F", [0.0, 0.0, H, H]),
        (
            [0.2, -0.4, 0.1, -0.888819],
            "42 9A 37 20 79",
            [0.200007, -0.400014, 0.099831, -0.888830],
        ),
        (
            [0.1, -0.9, 0.3, 0.3],
            "20 39 B6 63 5B",
            [0.099831, -0.900127, 0.299838, 0.299838],
        ),
    ];
    for (q, bytes, decoded) in cases {
        let b = hex(bytes);
        let arr = [b[0], b[1], b[2], b[3], b[4]];
        assert_eq!(encode_quat40(q), arr, "encode {q:?}");
        let d = decode_quat40(arr);
        assert_vec(&d, &decoded, 2e-6);
        assert!(close(quat_length(d), 1.0, 1e-6));
    }
}

fn sample1(clip: &Clip, t: f32) -> Transform {
    let mut out = [Transform::IDENTITY; 1];
    assert_eq!(clip.sample(t, &mut out), 1);
    out[0]
}

#[test]
fn spline_clip_12_3() {
    let spec = example_spline_12_3();
    let data = encode_block(&spec.blocks[0]);
    let expected = hex(
        "45 21 F0 00 01 00 01 00 00 04 04 00 00 00 00 3F 00 00 00 00 00 00 00 40 00 00 FF FF \
         01 00 01 00 00 04 04 FF F7 7F FF 37 FF F7 7F FE 2F 00 00 00",
    );
    assert_eq!(data, expected);

    let record =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
    let clip = Clip::parse(&record).unwrap();
    let AnimationData::Spline(sp) = &clip.animation.data else {
        panic!("spline expected")
    };
    assert_eq!(
        (sp.num_frames, sp.blocks.len(), sp.max_frames_per_block),
        (5, 1, 256)
    );
    assert_eq!(clip.track_count(), 1);
    let rows: [(f32, [f32; 3], [f32; 4]); 5] = [
        (0.0, [0.5, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        (1.0 / 30.0, [0.5, 0.5, 0.0], [0.0, 0.0, 0.187366, 0.982290]),
        (2.0 / 30.0, [0.5, 1.0, 0.0], [0.0, 0.0, 0.382683, 0.923880]),
        (4.0 / 30.0, [0.5, 2.0, 0.0], [0.0, 0.0, H, H]),
        (10.0, [0.5, 2.0, 0.0], [0.0, 0.0, H, H]),
    ];
    for (t, tr, rot) in rows {
        let s = sample1(&clip, t);
        assert_vec(&s.translation, &tr, 1e-5);
        assert_vec(&s.rotation, &rot, 1e-5);
        assert_eq!(s.scale, [1.0; 3]);
    }
    // Non-finite and negative times clamp to 0 (§7.4 step 1).
    for t in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -3.0] {
        assert_vec(&sample1(&clip, t).translation, &[0.5, 0.0, 0.0], 1e-6);
    }
    let one = clip.animation.sample_track(0, 2.0 / 30.0).unwrap();
    assert_vec(&one.translation, &[0.5, 1.0, 0.0], 1e-5);
    assert!(clip.animation.sample_track(1, 0.0).is_none());
}

#[test]
fn curve_evaluation_12_4() {
    type Case<'a> = (usize, &'a [u8], &'a [f32], [f32; 5]);
    let cases: [Case<'_>; 4] = [
        (1, &[0, 0, 4, 4], &[0.0, 2.0], [0.0, 0.5, 1.0, 1.5, 2.0]),
        (
            3,
            &[0, 0, 0, 0, 4, 4, 4, 4],
            &[0.0, 1.0, 1.0, 0.0],
            [0.0, 0.5625, 0.75, 0.5625, 0.0],
        ),
        (
            3,
            &[0, 0, 0, 0, 2, 4, 4, 4, 4],
            &[0.0, 1.0, 2.0, 3.0, 4.0],
            [0.0, 1.1875, 2.0, 2.8125, 4.0],
        ),
        (
            3,
            &[0, 0, 0, 0, 2, 2, 2, 4, 4, 4, 4],
            &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            [0.0, 1.5, 3.0, 4.5, 6.0],
        ),
    ];
    for (p, knots, points, values) in cases {
        let c = Curve::new(p, knots, points, 1).unwrap();
        for (u, v) in values.iter().enumerate() {
            let mut out = [0.0f32];
            c.evaluate(u as f32, &mut out);
            assert!(
                close(out[0], *v, 1e-6),
                "p {p} knots {knots:?} u {u}: {} vs {v}",
                out[0]
            );
        }
    }
    assert!(Curve::new(3, &[0, 0, 4, 4], &[0.0, 1.0], 1).is_none());
}

#[test]
fn block_selection_12_5() {
    let spec = SplineClipSpec::from_frames(300, 1, 1.0 / 60.0, 256, 1, |f, _| Transform {
        translation: [f as f32, 0.0, 0.0],
        ..Transform::IDENTITY
    });
    assert_eq!(spec.blocks.len(), 2);
    assert_eq!((spec.block_frames(0), spec.block_frames(1)), (256, 45));
    let record =
        ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
    let clip = Clip::parse(&record).unwrap();
    let AnimationData::Spline(sp) = &clip.animation.data else {
        panic!()
    };
    assert!(close(sp.block_duration, 4.25, 1e-5));
    let (b, u) = sp.locate(0.0);
    assert_eq!((b, u), (0, 0.0));
    let (b, u) = sp.locate(4.0);
    assert_eq!(b, 0);
    assert!(close(u, 240.0, 1e-3));
    // At the shared frame either block is acceptable; the pose must be frame 255.
    let (b, u) = sp.locate(4.25);
    assert!((b == 1 && close(u, 0.0, 1e-3)) || (b == 0 && close(u, 255.0, 1e-3)));
    assert!(close(sample1(&clip, 4.25).translation[0], 255.0, 0.01));
    let (b, u) = sp.locate(4.3);
    assert_eq!(b, 1);
    assert!(close(u, 3.0, 1e-2));
    for t in [299.0 / 60.0, 100.0] {
        let (b, u) = sp.locate(t);
        assert_eq!(b, 1);
        assert!(close(u, 44.0, 1e-3));
    }
    // Frame-exact sampling agrees with the source frames everywhere.
    let mut out = [Transform::IDENTITY];
    for f in [0u32, 100, 254, 255, 256, 299, 400] {
        clip.animation.sample_frame(f as usize, &mut out);
        assert!(
            close(out[0].translation[0], f.min(299) as f32, 0.01),
            "frame {f}"
        );
    }
}

#[test]
fn interleaved_12_6() {
    let record = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
        example_interleaved_12_6(),
    )))
    .to_bytes();
    let clip = Clip::parse(&record).unwrap();
    assert_eq!(clip.animation.frame_count(), 3);
    let mut out = [Transform::IDENTITY; 2];
    assert_eq!(clip.sample(0.25, &mut out), 2);
    assert_vec(&out[0].translation, &[0.5, 0.0, 0.0], 1e-6);
    assert_vec(&out[1].translation, &[0.0, 0.5, 0.0], 1e-6);
    for t in [1.0, 7.0] {
        clip.sample(t, &mut out);
        assert_vec(&out[0].translation, &[2.0, 0.0, 0.0], 1e-6);
    }
    // A shorter buffer receives a prefix.
    let mut one = [Transform::IDENTITY; 1];
    assert_eq!(clip.sample(0.75, &mut one), 1);
    assert_vec(&one[0].translation, &[1.5, 0.0, 0.0], 1e-6);
    let t1 = clip.animation.sample_track(1, 0.75).unwrap();
    assert_vec(&t1.translation, &[0.0, 1.5, 0.0], 1e-6);
}

#[test]
fn annotations_and_reference_frame_12_7() {
    let mut anim = AnimationSpec::new(AnimationKindSpec::Interleaved(example_interleaved_12_6()));
    anim.annotation_names = Some(vec!["t0".into(), "t1".into()]);
    let clip = Clip::parse(&ClipRecordSpec::new(anim.clone()).to_bytes()).unwrap();
    let names: Vec<_> = clip
        .animation
        .annotation_tracks
        .iter()
        .map(|a| (a.name.as_deref().unwrap().to_vec(), a.annotations.len()))
        .collect();
    assert_eq!(names, [(b"t0".to_vec(), 0), (b"t1".to_vec(), 0)]);
    assert_eq!(clip.binding().track_name(1), Some(&b"t1"[..]));
    assert_eq!(clip.binding().bone(1), Some(1));
    assert_eq!(clip.binding().bone(2), None);

    for components in [4u8, 3] {
        let rf =
            ReferenceFrame::parse(&example_reference_frame_12_7(components).to_tagfile()).unwrap();
        assert_eq!(rf.components, components);
        let w = if components == 4 { 0.05 } else { 0.0 };
        assert_vec(&rf.sample(0.25), &[0.25, 0.0, 0.0, w], 1e-6);
        assert_vec(&rf.up, &[0.0, 0.0, 1.0, 0.0], 0.0);
    }

    // Embedded and leading reference frames; the embedded one is preferred.
    anim.extracted_motion = Some(example_reference_frame_12_7(4));
    let mut rec = ClipRecordSpec::new(anim);
    let mut leading = example_reference_frame_12_7(3);
    leading.samples.pop();
    rec.leading_reference_frame = Some(leading);
    let clip = Clip::parse(&rec.to_bytes()).unwrap();
    let (rf, src) = clip.reference_frame().unwrap();
    assert_eq!(src, ReferenceFrameSource::Embedded);
    assert_eq!(rf.samples.len(), 3);
    assert_eq!(
        clip.leading_reference_frame.as_ref().unwrap().samples.len(),
        2
    );
}

#[test]
fn envelope_12_8() {
    let rec = example_record_12_8();
    let bytes = rec.to_bytes();
    let tagfile = rec.animation.to_tagfile();
    let mut expected = hex("00 00 80 3F 02 00 00 00 00");
    expected.extend_from_slice(&(tagfile.len() as u32).to_le_bytes());
    expected.extend_from_slice(&tagfile);
    expected.extend_from_slice(&hex("02 00 00 80 3F 00 00 00 3F"));
    assert_eq!(bytes, expected);
    let env = Envelope::parse(&bytes).unwrap();
    assert_eq!((env.duration, env.track_count), (1.0, 2));
    assert_eq!(env.track_weights, [1.0, 0.5]);
    assert!(env.reference_frame.is_none());

    let mut none = rec.clone();
    none.weights.clear();
    let b = none.to_bytes();
    assert_eq!(*b.last().unwrap(), 0);
    assert!(Clip::parse(&b).unwrap().track_weights.is_empty());

    let mut lead = rec;
    lead.leading_reference_frame = Some(example_reference_frame_12_7(4));
    let b = lead.to_bytes();
    assert_eq!(b[8], 1);
    let clip = Clip::parse(&b).unwrap();
    assert_eq!(
        clip.reference_frame().unwrap().1,
        ReferenceFrameSource::Leading
    );
}

#[test]
fn skeleton_round_trip_full_classes() {
    let mut spec = SkeletonSpec::chain(5);
    spec.bones[3].parent = 0;
    spec.bones[4].reference.scale = [2.0, 1.0, 0.5];
    spec.bones[2].reference.rotation = [0.0, H, 0.0, H];
    let t = spec.to_tagfile();
    let s = Skeleton::parse(&t).unwrap();
    for (b, bs) in s.bones.iter().zip(&spec.bones) {
        assert_eq!(b.name.as_deref(), bs.name.as_deref().map(str::as_bytes));
        assert_eq!(b.parent, usize::try_from(bs.parent).ok());
        assert_eq!(b.reference, bs.reference);
    }
    // No names, locked translations.
    for b in &mut spec.bones {
        b.name = None;
        b.lock_translation = true;
    }
    let s = Skeleton::parse(&spec.to_tagfile()).unwrap();
    assert!(
        s.bones
            .iter()
            .all(|b| b.name.is_none() && b.lock_translation)
    );
    // Through fields 1–3 of a setup record.
    let rec = spec.to_setup_prefix(0x0400_1234);
    assert_eq!(Skeleton::from_setup_record(&rec).unwrap(), s);
    let c = classify_setup_record(&rec);
    assert_eq!((c.kind, c.failure()), (StructureKind::Skeleton, None));
}

#[test]
fn rich_record_round_trip_and_classification() {
    let rec = example_rich_record();
    let bytes = rec.to_bytes();
    let clip = Clip::parse(&bytes).unwrap();
    assert_eq!(clip.track_count(), 3);
    assert_eq!(clip.track_weights, [1.0, 0.5, 0.0]);
    assert_eq!(clip.animation.annotation_tracks.len(), 3);
    assert!(clip.animation.extracted_motion.is_some());
    assert!(clip.leading_reference_frame.is_some());
    assert!(!clip.duration_mismatch());
    let AnimationData::Spline(sp) = &clip.animation.data else {
        panic!()
    };
    assert_eq!(sp.blocks.len(), 2);
    let mut scratch = Vec::new();
    let c = classify_clip_record(&bytes, &mut scratch);
    assert_eq!((c.kind, c.failure()), (StructureKind::SplineClip, None));

    // Sampled frames follow the source poses (degree-3 curves through per-frame control
    // points approximate within a fraction of a frame near the block ends).
    let mut out = [Transform::IDENTITY; 3];
    for f in [0u32, 17, 255, 256, 299] {
        clip.animation.sample_frame(f as usize, &mut out);
        let t = f as f32 / 60.0;
        assert!(
            close(out[2].scale[0], 1.0 + t * 0.1, 2e-3),
            "frame {f}: {:?}",
            out[2]
        );
        assert!(
            close(out[0].translation[1], (t * 3.0).sin(), 0.04),
            "frame {f}"
        );
        let angle = t * 2.0;
        let (sn, cs) = (angle * 0.5).sin_cos();
        let r = out[1].rotation;
        let dot = (r[2] * sn + r[3] * cs).abs();
        assert!(dot > 0.9999, "frame {f}: {r:?}");
    }
}
