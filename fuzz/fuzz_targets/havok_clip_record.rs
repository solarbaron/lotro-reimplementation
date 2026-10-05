// SPDX-License-Identifier: GPL-3.0-or-later
//! 0x05 clip records (`docs/specs/havok-data.md` §2.3, §4.3–§4.5, §5, §6, §7; structure
//! kinds K7, K8, K10, K11, K12 and K13 of §11) on arbitrary bytes.
//!
//! The first byte picks a mode:
//!
//! - 0: raw record bytes after the mode byte;
//! - 1: a record described by the input and written by `tessera_havok::testkit`: the rich
//!   two-block spline record, the §12.3 spline clip, an interleaved clip of any shape
//!   (zero tracks, empty or ragged arrays, non-finite values), a multi-block spline clip
//!   built from a pose function, or the §12.8 record; with annotation names (any count),
//!   leading and embedded reference frames (component counts 0–5, any sample count),
//!   weights of any count, envelope overrides and member overrides of the spline object
//!   (counts, durations including 0, negative and NaN, block tables); then damaged.
//!
//! Every decoded clip is sampled at times chosen by the input (NaN, ±∞, huge, negative)
//! and at chosen frames. Nothing may panic. An undamaged description that the builder
//! keeps consistent must decode.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ByteSource, ClipRecordSpec, InterleavedClipSpec,
    ReferenceFrameSpec, SplineClipSpec, WValue, damage, example_record_12_8, example_rich_record,
    example_spline_12_3, quat_from_angles,
};
use tessera_havok::{AnimationData, Clip, Envelope, Transform, classify_clip_record};

fn sample_all(clip: &Clip, src: &mut ByteSource<'_>) {
    let tracks = clip.track_count();
    assert_eq!(tracks, clip.animation.tracks);
    let mut out = vec![Transform::IDENTITY; tracks.min(512)];
    let d = clip.duration();
    for _ in 0..4 {
        let t = src.time(d);
        let n = clip.sample(t, &mut out);
        assert_eq!(n, out.len().min(tracks));
        let f = usize::from(src.u16());
        let _ = clip.animation.sample_frame(f, &mut out);
        let _ = clip.animation.sample_track(src.below(tracks + 2), t);
        if let Some((rf, _)) = clip.reference_frame() {
            let _ = rf.sample(t);
        }
    }
    for t in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        -0.0,
        1e30,
    ] {
        clip.sample(t, &mut out);
    }
    if let AnimationData::Spline(sp) = &clip.animation.data {
        for b in &sp.blocks {
            for ch in &b.tracks {
                for c in [&ch.position, &ch.scale] {
                    if let tessera_havok::VectorChannel::Spline { curve, .. } = c {
                        let cv = b.curve(curve).expect("parsed curve fits its block");
                        let mut v = [0.0f32; 3];
                        cv.evaluate(src.time(b.frames as f32), &mut v);
                    }
                }
                if let tessera_havok::RotationChannel::Dynamic(curve) = &ch.rotation {
                    let cv = b.curve(curve).expect("parsed curve fits its block");
                    let _ = cv.evaluate_rotation(src.time(b.frames as f32));
                }
            }
        }
    }
    let binding = clip.binding();
    assert_eq!(binding.bone(tracks), None);
    let _ = binding.track_name(0);
}

fn check(bytes: &[u8], src: &mut ByteSource<'_>) {
    let _ = Envelope::parse(bytes);
    let c = classify_clip_record(bytes, &mut Vec::new());
    match Clip::parse(bytes) {
        Ok(clip) => {
            if let Some(cat) = c.failure() {
                assert_eq!(cat, tessera_havok::FailureCategory::SampleNonFinite);
            }
            sample_all(&clip, src);
        }
        Err(e) => assert_eq!(c.failure(), Some(e.category())),
    }
}

fn reference_frame(src: &mut ByteSource<'_>, duration: f32) -> ReferenceFrameSpec {
    let n = src.below(12);
    ReferenceFrameSpec {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration: if src.u8().is_multiple_of(8) {
            src.f32()
        } else {
            duration
        },
        components: if src.u8().is_multiple_of(8) {
            src.below(6) as u8
        } else {
            4
        },
        samples: (0..n)
            .map(|_| [src.f32(), src.f32(), src.f32(), src.f32()])
            .collect(),
    }
}

/// Returns the record description and whether the builder kept it consistent.
fn spec(src: &mut ByteSource<'_>) -> (ClipRecordSpec, bool) {
    let mut consistent = true;
    let mut rec = match src.below(5) {
        0 => example_rich_record(),
        1 => ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(
            example_spline_12_3(),
        ))),
        2 => {
            let tracks = src.below(5);
            let frames = src.below(6);
            let mut transforms = Vec::new();
            for _ in 0..tracks * frames {
                transforms.push(Transform {
                    translation: [src.f32(), src.f32(), src.f32()],
                    rotation: quat_from_angles(src.small_f32(), src.small_f32(), src.small_f32()),
                    scale: [1.0; 3],
                });
            }
            if src.u8().is_multiple_of(8) {
                transforms.truncate(transforms.len().saturating_sub(1));
            }
            consistent &=
                tracks > 0 && !transforms.is_empty() && transforms.len().is_multiple_of(tracks);
            ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
                InterleavedClipSpec {
                    duration: src.small_f32().abs(),
                    tracks,
                    transforms,
                },
            )))
        }
        3 => {
            let frames = 1 + u32::from(src.u16() % 900);
            let tracks = 1 + src.below(4);
            let degree = [1u8, 2, 3][src.below(3)];
            let f = [256u32, 256, 256, 2, 3, 17][src.below(6)];
            let w = [src.small_f32(), src.small_f32(), src.small_f32()];
            let sp =
                SplineClipSpec::from_frames(frames, tracks, 1.0 / 60.0, f, degree, move |fr, j| {
                    let t = fr as f32 / 60.0 * (1.0 + j as f32);
                    Transform {
                        translation: [(t * w[0]).sin(), t * w[1], 0.0],
                        rotation: quat_from_angles(t * w[0], t * w[2], 0.3),
                        scale: [1.0 + 0.1 * (t * w[2]).cos(), 1.0, 1.0],
                    }
                });
            ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(sp)))
        }
        _ => example_record_12_8(),
    };
    let tracks = rec.animation.track_count();
    let duration = rec.animation.duration();
    if src.u8().is_multiple_of(4) {
        let n = if src.u8().is_multiple_of(4) {
            src.below(6)
        } else {
            tracks
        };
        rec.animation.annotation_names = Some((0..n).map(|i| format!("t{i}")).collect());
    }
    if src.u8().is_multiple_of(4) {
        let rf = reference_frame(src, duration);
        // Component counts outside 1–4 are an unsupported form (§3.8).
        consistent &= (1..=4).contains(&rf.components);
        rec.animation.extracted_motion = Some(rf);
    }
    if src.u8().is_multiple_of(4) {
        let rf = reference_frame(src, duration);
        consistent &= (1..=4).contains(&rf.components);
        rec.leading_reference_frame = Some(rf);
    }
    match src.u8() % 6 {
        0 => rec.weights = (0..tracks).map(|_| src.f32()).collect(),
        1 => {
            let n = src.below(300);
            consistent &= n == 0 || n == tracks;
            rec.weights = vec![0.5; n];
        }
        _ => {}
    }
    match src.u8() % 8 {
        0 => rec.envelope_duration = Some(src.f32()),
        1 => {
            let t = src.below(6) as u32;
            consistent &= t as usize == tracks;
            rec.envelope_track_count = Some(t);
        }
        _ => {}
    }
    if matches!(rec.animation.kind, AnimationKindSpec::Spline(_)) && src.u8().is_multiple_of(3) {
        const NAMES: [&str; 12] = [
            "numFrames",
            "numBlocks",
            "maxFramesPerBlock",
            "maskAndQuantizationSize",
            "blockDuration",
            "blockInverseDuration",
            "frameDuration",
            "duration",
            "numberOfFloatTracks",
            "endian",
            "type",
            "numberOfTransformTracks",
        ];
        let name = NAMES[src.below(NAMES.len())];
        let v = match name {
            "blockDuration" | "blockInverseDuration" | "frameDuration" | "duration" => {
                WValue::Real([0.0, -1.0, f32::NAN, f32::INFINITY, src.raw_f32()][src.below(5)])
            }
            _ => WValue::Int(i32::from(src.u16() as i16)),
        };
        // Durations do not take part in the decode rule; the others may break it.
        if !matches!(
            name,
            "blockDuration" | "blockInverseDuration" | "frameDuration" | "duration" | "type"
        ) {
            consistent = false;
        }
        rec.animation.overrides.push((name.into(), v));
    }
    if matches!(rec.animation.kind, AnimationKindSpec::Spline(_)) && src.u8().is_multiple_of(6) {
        let name = ["blockOffsets", "floatBlockOffsets"][src.below(2)];
        let n = src.below(4);
        rec.animation.overrides.push((
            name.into(),
            WValue::IntArray((0..n).map(|_| i32::from(src.u8()) * 16).collect()),
        ));
        consistent = false;
    }
    (rec, consistent)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut src = ByteSource::new(rest);
    if mode.is_multiple_of(2) {
        let tail = rest.len().saturating_sub(16);
        let mut times = ByteSource::new(&rest[tail..]);
        check(rest, &mut times);
        return;
    }
    let (rec, consistent) = spec(&mut src);
    let mut bytes = rec.to_bytes();
    match Clip::parse(&bytes) {
        Ok(clip) => {
            sample_all(&clip, &mut src);
            assert_eq!(clip.track_weights.len(), rec.weights.len());
        }
        Err(e) => assert!(!consistent, "consistent description rejected: {e}"),
    }
    damage(&mut bytes, &mut src);
    check(&bytes, &mut src);
});
