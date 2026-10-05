// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic builders shared by the integration tests (all values invented).

#![allow(dead_code)]

use std::sync::Arc;

use tessera_anim::{BoundClip, Skeleton};
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BoneSpec, ClipRecordSpec, InterleavedClipSpec,
    ReferenceFrameSpec, SkeletonSpec, SplineClipSpec, quat_from_angles,
};
use tessera_havok::{Clip, Transform};

/// A humanoid-ish tree of `n` bones (n ≥ 3): bone 0 root at pelvis height, then a few
/// branches, with some rotated and scaled reference bones.
pub fn tree_spec(n: usize) -> SkeletonSpec {
    let mut bones = Vec::with_capacity(n);
    for i in 0..n {
        let parent = if i == 0 { -1 } else { ((i - 1) / 2) as i32 };
        let a = i as f32 * 0.37;
        bones.push(BoneSpec {
            name: Some(format!("rig_bone{i}")),
            parent,
            reference: Transform {
                translation: if i == 0 {
                    [0.0, 0.02, 0.95]
                } else {
                    [0.1 * a.sin(), 0.15 + 0.01 * i as f32, 0.05 * a.cos()]
                },
                rotation: if i % 3 == 1 {
                    quat_from_angles(0.3 * a.sin(), 0.2, 0.1 * a.cos())
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                },
                scale: if i % 7 == 5 {
                    [1.1, 0.9, 1.0]
                } else {
                    [1.0; 3]
                },
            },
            lock_translation: false,
        });
    }
    SkeletonSpec {
        bones,
        overrides: Vec::new(),
    }
}

/// Decodes a skeleton spec through the 0x04 setup prefix and builds the anim skeleton.
pub fn skeleton(spec: &SkeletonSpec) -> Arc<Skeleton> {
    Arc::new(Skeleton::from_setup_record(&spec.to_setup_prefix(0x0400_0001)).expect("skeleton"))
}

/// Writes and decodes a spline clip of `frames` frames at `fps` from `pose(frame, track)`.
pub fn spline_clip(
    frames: u32,
    tracks: usize,
    fps: f32,
    pose: impl Fn(u32, usize) -> Transform,
) -> ClipRecordSpec {
    let spec = SplineClipSpec::from_frames(frames, tracks, 1.0 / fps, 256, 1, pose);
    ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec)))
}

/// An interleaved clip of `frames` frames over `duration` seconds.
pub fn interleaved_clip(
    frames: usize,
    tracks: usize,
    duration: f32,
    pose: impl Fn(usize, usize) -> Transform,
) -> ClipRecordSpec {
    let mut transforms = Vec::with_capacity(frames * tracks);
    for f in 0..frames {
        for j in 0..tracks {
            transforms.push(pose(f, j));
        }
    }
    ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
        InterleavedClipSpec {
            duration,
            tracks,
            transforms,
        },
    )))
}

/// Adds an embedded extracted-motion reference frame.
pub fn with_motion(mut rec: ClipRecordSpec, samples: Vec<[f32; 4]>) -> ClipRecordSpec {
    let duration = rec.animation.duration();
    rec.animation.extracted_motion = Some(ReferenceFrameSpec {
        up: [0.0, 0.0, 1.0, 0.0],
        forward: [1.0, 0.0, 0.0, 0.0],
        duration,
        components: 4,
        samples,
    });
    rec
}

/// Decodes a record and binds it.
pub fn bind(rec: &ClipRecordSpec, skeleton: &Arc<Skeleton>) -> Arc<BoundClip> {
    let clip = Clip::parse(&rec.to_bytes()).expect("clip decodes");
    Arc::new(BoundClip::bind(Arc::new(clip), Arc::clone(skeleton)).expect("binds"))
}

/// A cyclic pose: the reference pose with every track rotated by a sine of the phase,
/// ending on its first pose (frame `frames − 1` equals frame 0).
pub fn cyclic(
    skeleton: &Skeleton,
    frames: u32,
    amplitude: f32,
) -> impl Fn(u32, usize) -> Transform + '_ {
    move |f, j| {
        let phase = f as f32 / (frames - 1) as f32 * std::f32::consts::TAU;
        let r = skeleton.reference_local()[j];
        let d = quat_from_angles(amplitude * phase.sin(), 0.0, amplitude * 0.5 * phase.cos());
        Transform {
            rotation: quat_mul(r.rotation, d),
            translation: [
                r.translation[0],
                r.translation[1],
                r.translation[2] + 0.01 * phase.sin(),
            ],
            ..r
        }
    }
}

/// Hamilton product of (x, y, z, w) quaternions.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// Angle between two rotations in radians (sign-insensitive).
pub fn quat_angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs();
    2.0 * d.min(1.0).acos()
}

/// Largest difference between two local poses: (translation metres, rotation radians,
/// scale).
pub fn pose_diff(a: &[Transform], b: &[Transform]) -> (f32, f32, f32) {
    let mut d = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        for k in 0..3 {
            d.0 = d.0.max((x.translation[k] - y.translation[k]).abs());
            d.2 = d.2.max((x.scale[k] - y.scale[k]).abs());
        }
        d.1 = d.1.max(quat_angle(x.rotation, y.rotation));
    }
    d
}
