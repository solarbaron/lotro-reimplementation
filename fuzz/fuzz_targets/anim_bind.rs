// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-anim` on untrusted skeletons and clips (`docs/specs/skeleton-anim.md` §3–§7,
//! bone sliders of `docs/specs/appearance.md` §6.3):
//! skeleton building, positional binding, sampling with wrap, layered evaluation,
//! composition, skinning matrices, root motion and baking.
//!
//! The first byte picks a mode:
//!
//! - 0: raw bytes: a u16 length, that many bytes of skeleton tagfile, then a 0x05 clip
//!   record;
//! - 1: a skeleton (any parent indices, transforms including NaN, zero scale) and a clip
//!   (track count equal to the bone count or off by one, optional extracted motion with any
//!   samples, any weights) described by the input, written by `tessera_havok::testkit`,
//!   then damaged.
//!
//! Whatever decodes is built, bound and played at times chosen by the input (NaN, ±∞,
//! huge, negative). Nothing may panic, and every posed matrix of a successfully built
//! skeleton and bound clip is finite unless the clip's values are hostile (checked only
//! for the absence of panics).

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_anim::glam::Mat4;
use tessera_anim::{
    Animator, BakedClip, BindDeformation, BoneSliders, BoundClip, CrossFade, DeformClip,
    EvalScratch, MAX_LAYER_CLIPS, Playback, RootMotionMode, Skeleton, SliderChannels, Source,
    SpeedBlend, Wrap, evaluate, motion_between, skinning_matrices,
};
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BoneSpec, ByteSource, ClipRecordSpec, ReferenceFrameSpec,
    SkeletonSpec, SplineClipSpec, damage, quat_from_angles,
};
use tessera_havok::{Clip, Transform};

fn play(skeleton: Skeleton, clip: Clip, src: &mut ByteSource<'_>) {
    let skeleton = Arc::new(skeleton);
    let n = skeleton.bone_count();
    let clip = Arc::new(clip);
    let bound = match BoundClip::bind(clip.clone(), skeleton.clone()) {
        Ok(b) => b,
        Err(_) => {
            assert_ne!(
                clip.track_count(),
                n,
                "binding fails only on a count mismatch"
            );
            return;
        }
    };
    assert_eq!(bound.track_count(), n);
    let bound = if src.u8().is_multiple_of(4) {
        match BakedClip::bake(&clip, 1 << 20) {
            Some(b) => bound.with_baked(Arc::new(b)).expect("baked from this clip"),
            None => bound,
        }
    } else {
        bound
    };
    let bound = Arc::new(bound);
    let d = bound.duration();
    let mut local = vec![Transform::IDENTITY; n];
    let mut model = vec![Mat4::IDENTITY; n];
    let mut skin = vec![Mat4::IDENTITY; n];
    for _ in 0..4 {
        let tau = src.time(d);
        let wrap = if src.u8() & 1 == 0 {
            Wrap::Loop
        } else {
            Wrap::Once
        };
        bound
            .sample_at(tau, wrap, &mut local)
            .expect("sized buffer");
        for t in &local {
            assert!(t.is_finite(), "sampled transforms are sanitised");
        }
        skeleton.compose(&local, &mut model).expect("sized buffer");
        skinning_matrices(&skeleton, &model, &mut skin).expect("sized buffer");
        let _ = motion_between(&bound, src.time(d), src.time(d), wrap);
    }
    let _ = bound.root_displacement();
    // Layered playback through an animator.
    let mut anim = Animator::new(skeleton.clone());
    let mut p = Playback::new(bound.clone(), Wrap::Loop);
    p.rate = src.f32();
    p.root_motion = RootMotionMode::Translate;
    let _ = anim.play(0, p.clone(), src.f32(), src.u8() & 1 == 0);
    let _ = anim.play(0, Playback::new(bound.clone(), Wrap::Once), src.f32(), true);
    if let Ok(sb) = SpeedBlend::new(vec![(src.f32(), bound.clone()), (src.f32(), bound.clone())]) {
        let _ = anim.set_source(1, Source::Speed(sb));
    }
    let _ = anim.set_source(
        2,
        Source::CrossFade(CrossFade::new(p.clone(), p, src.f32(), true)),
    );
    if let Some(l) = anim.layer_mut(1) {
        l.use_track_weights = true;
        l.fade_to(src.f32(), src.f32());
    }
    let mut snap = Vec::new();
    let mut scratch = EvalScratch::new();
    for _ in 0..3 {
        let m = anim.advance(src.f32());
        let _ = m.is_finite();
        // Interrupting fades (on a speed blend or a fade in progress) nests sources; the
        // clip count stays bounded.
        let layer = usize::from(src.u8() % 3);
        let _ = anim.play(
            layer,
            Playback::new(bound.clone(), Wrap::Loop),
            src.f32(),
            true,
        );
        for l in anim.layers() {
            if let Some(s) = &l.source {
                assert!(s.clip_count() <= MAX_LAYER_CLIPS);
            }
        }
        anim.snapshot(&mut snap);
        for l in &snap {
            assert!(l.clips().count() <= MAX_LAYER_CLIPS);
        }
        evaluate(&skeleton, &snap, &mut scratch, &mut local).expect("bound clips fit");
        // Bone sliders on the sampled pose.
        let mut sliders = BoneSliders::new(&skeleton);
        for _ in 0..src.below(4) {
            let targets: Vec<(i64, bool)> = (0..src.below(3))
                .map(|_| (i64::from(src.u8()) - 2, src.u8() & 1 == 1))
                .collect();
            let _ = sliders.add_slider(
                SliderChannels::from_bits((u16::from(src.u8()) << 1) | u16::from(src.u8() & 1)),
                [src.f32(), src.f32(), src.f32()],
                [src.f32(), src.f32(), src.f32()],
                src.f32(),
                &targets,
            );
        }
        for e in sliders.entries() {
            assert!(usize::from(e.bone) < n, "slider bones are validated");
        }
        sliders.apply(&mut local).expect("sized buffer");
        let _ = skeleton.compose(&local, &mut model);
        // Held playback and the bind-space deformation (appearance.md §6.4).
        let held = Playback::held(bound.clone(), src.f32());
        assert!(held.clip_time().is_finite());
        let deform = [DeformClip {
            clip: bound.clone(),
            fraction: src.f32(),
            use_bone_weights: src.u8() & 1 == 1,
        }];
        if let Ok(d) = BindDeformation::new(&skeleton, &deform, Some(&sliders)) {
            let mut pos = [[src.f32(), src.f32(), src.f32()]; 2];
            let mut nrm = [[src.f32(), src.f32(), src.f32()]; 2];
            let joints = [[src.u8(), src.u8(), 0, 0], [src.u8(), 0, 0, 0]];
            let weights = [[src.f32(), src.f32(), 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]];
            d.deform(&mut pos, Some(&mut nrm), None, &joints, &weights);
        }
    }
}

fn raw(bytes: &[u8]) {
    let mut src = ByteSource::new(bytes);
    let len = usize::from(src.u16());
    let rest = src.rest();
    let (skel, clip) = rest.split_at(len.min(rest.len()));
    let Ok(h) = tessera_havok::Skeleton::parse(skel) else {
        return;
    };
    let Ok(s) = Skeleton::from_havok(&h) else {
        return;
    };
    let Ok(c) = Clip::parse(clip) else { return };
    let mut times = ByteSource::new(clip);
    play(s, c, &mut times);
}

fn synthetic(src: &mut ByteSource<'_>) {
    let n = 1 + src.below(40);
    let bones: Vec<BoneSpec> = (0..n)
        .map(|i| BoneSpec {
            name: None,
            parent: match src.u8() % 8 {
                0 => src.u8() as i32 - 128,
                _ if i == 0 => -1,
                _ => src.below(i) as i32,
            },
            reference: Transform {
                translation: [src.f32(), src.f32(), src.f32()],
                rotation: if src.u8().is_multiple_of(16) {
                    [src.f32(), src.f32(), src.f32(), src.f32()]
                } else {
                    quat_from_angles(src.small_f32(), src.small_f32(), src.small_f32())
                },
                scale: if src.u8().is_multiple_of(16) {
                    [src.f32(), src.f32(), src.f32()]
                } else {
                    [1.0; 3]
                },
            },
            lock_translation: false,
        })
        .collect();
    let skel_spec = SkeletonSpec {
        bones,
        overrides: Vec::new(),
    };
    let mut skel_bytes = skel_spec.to_tagfile();
    if src.u8().is_multiple_of(4) {
        damage(&mut skel_bytes, src);
    }
    let tracks = match src.u8() % 8 {
        0 => n + 1,
        1 => n.saturating_sub(1).max(1),
        _ => n,
    };
    let frames = 1 + u32::from(src.u16() % 400);
    let degree = [1u8, 2, 3][src.below(3)];
    let w = [src.small_f32(), src.small_f32(), src.small_f32()];
    let sp = SplineClipSpec::from_frames(frames, tracks, 1.0 / 60.0, 256, degree, move |f, j| {
        let t = f as f32 / 60.0 * (1.0 + j as f32);
        Transform {
            translation: [(t * w[0]).sin(), t * w[1], 0.0],
            rotation: quat_from_angles(t * w[0], t * w[2], 0.3),
            scale: [1.0 + 0.1 * (t * w[2]).cos(), 1.0, 1.0],
        }
    });
    let mut rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(sp)));
    if src.u8().is_multiple_of(4) {
        let k = src.below(80);
        rec.animation.extracted_motion = Some(ReferenceFrameSpec {
            up: [src.small_f32(), src.small_f32(), 1.0, 0.0],
            forward: [1.0, 0.0, 0.0, 0.0],
            duration: rec.animation.duration(),
            components: 4,
            samples: (0..k)
                .map(|_| [src.f32(), src.f32(), src.f32(), src.f32()])
                .collect(),
        });
    }
    if src.u8().is_multiple_of(4) {
        rec.weights = (0..tracks).map(|_| src.f32()).collect();
    }
    let mut clip_bytes = rec.to_bytes();
    if src.u8().is_multiple_of(4) {
        damage(&mut clip_bytes, src);
    }
    let Ok(h) = tessera_havok::Skeleton::parse(&skel_bytes) else {
        return;
    };
    let Ok(s) = Skeleton::from_havok(&h) else {
        return;
    };
    let Ok(c) = Clip::parse(&clip_bytes) else {
        return;
    };
    play(s, c, src);
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 2 {
        0 => raw(rest),
        _ => synthetic(&mut ByteSource::new(rest)),
    }
});
