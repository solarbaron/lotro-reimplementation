// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-anim`: skeleton hierarchy, pose sampling (decompression on worker threads,
//! cached per clip), blending, root motion, attach points (held-item bones) and skinning
//! palette output for the renderer.
//!
//! Specs: `docs/specs/skeleton-anim.md` ("the spec" below) and `docs/specs/havok-data.md`
//! (ROADMAP M4, ARCHITECTURE §11.2, §20.3, §21). Decoding is `tessera-havok`'s; this crate
//! starts from decoded skeletons and clips.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`Skeleton`] | Validated hierarchy (parents precede children), reference pose, model-space reference transforms, inverse reference triples | §2, §4.4, §5 |
//! | [`Skeleton::compose`] | Local → model transforms by the triple rule (a parent's scale never moves a child joint) | §5.3, §5.4 |
//! | [`BoundClip`] | Positional clip-to-skeleton binding (track *j* → bone *j*, track count = bone count) and sampling | §3.2, §3.4 |
//! | [`clip_time`], [`Wrap`] | Looping (τ mod duration) and one-shot clamping | §6.3 |
//! | [`evaluate`] | Layered-weight combination of every playing clip (remaining weight per layer, 1/255 steps, reference fill) | §6.6 |
//! | [`blend_poses`], [`layer_over`], [`scalar_blend_weights`] | Two-pose blends, two-pose partial blends, scalar-blend weights | §6.5 |
//! | [`Playback`], [`CrossFade`], [`SpeedBlend`], [`Animator`] | Playback state: rate, one overall weight per clip with linear ramps, cross-fades, speed-parameter blends (idle ↔ walk ↔ run), several clips per layer, held clips, layers | §6, `script-table.md` §6–§8 |
//! | [`motion_between`], [`RootMotionMode`], [`MotionDelta`] | Root motion: in place, or translate by the extracted-motion reference frame | §7 |
//! | [`AttachTable`], [`BoneMap`] | Holding locations (list A) and the `BoneType` bone map from plain data | §8 |
//! | [`skinning_matrices`], [`write_palette_mat4`], [`write_palette_rows3x4`], [`CompactPalette`] | Skinning transforms from model triples (§5.4, K26); palettes indexed by bone index; optional per-submesh compact palettes | §4, §5.4 |
//! | [`skin_position`], [`influence_list`] | CPU reference skinning; influence-list recomputation | §4.2, §4.7 |
//! | [`BoneSliders`] | Per-bone slider entries (written in definition order, later ones replace earlier ones) | `appearance.md` §6.3 |
//! | [`BindDeformation`] | Body-shape deformer clips and sliders as a bind-space deformation of skinned vertices | `appearance.md` §6.4 |
//! | [`bind_shape`], [`BoundShape`] | A figure's shape inputs (`tessera-mesh`) bound to its skeleton: held clips and the deformation, shared by the client and the explorer | `appearance.md` §6.2–§6.4, `script-table.md` §6.7–§6.9 |
//! | [`ClipDecoder`], [`SkeletonDecoder`], [`BakeCache`] | Per-clip caches: decoded clips in an asset store, baked frames, both byte-bounded | ARCH §11.2 |
//! | [`AnimationScheduler`] | One sampling job per instance per frame on the asset workers, double-buffered | ARCH §20.3, §21 |
//!
//! # Conventions
//!
//! Z up, metres, right-handed, characters face +Y (§5.1). Quaternions from Havok data are
//! (x, y, z, w); setup frames are (w, x, y, z) ([`AttachFrame`]). Rotations are active.
//! A local transform is T · R · S on column vectors, and model = parent model · local
//! (§5.3), composed as (translation, rotation, scale) triples so that a parent's scale never
//! moves a child joint (§5.4, K17). Matrices are [`glam::Mat4`], column-major.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use glam::Mat4;
//! use tessera_anim::{Animator, BoundClip, Playback, Skeleton, Wrap, skinning_matrices};
//! use tessera_havok::testkit::{AnimationKindSpec, AnimationSpec, ClipRecordSpec, SkeletonSpec, SplineClipSpec};
//! use tessera_havok::{Clip, Transform};
//!
//! // A synthetic three-bone skeleton and a one-second clip that lifts the root.
//! let skeleton = Arc::new(Skeleton::from_setup_record(&SkeletonSpec::chain(3).to_setup_prefix(0x0400_0001)).unwrap());
//! let spec = SplineClipSpec::from_frames(61, 3, 1.0 / 60.0, 256, 1, |f, j| Transform {
//!     translation: [0.0, 0.0, if j == 0 { f as f32 / 60.0 } else { 1.0 }],
//!     ..Transform::IDENTITY
//! });
//! let record = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
//! let clip = Arc::new(BoundClip::bind(Arc::new(Clip::parse(&record).unwrap()), skeleton.clone()).unwrap());
//!
//! let mut animator = Animator::new(skeleton.clone());
//! animator.play(0, Playback::new(clip, Wrap::Loop), 0.0, false).unwrap();
//! animator.advance(0.5);
//!
//! // Synchronous evaluation (the scheduler does the same on workers).
//! let mut layers = Vec::new();
//! animator.snapshot(&mut layers);
//! let mut local = skeleton.reference_pose();
//! tessera_anim::evaluate(&skeleton, &layers, &mut Default::default(), &mut local).unwrap();
//! let mut model = vec![Mat4::IDENTITY; 3];
//! let mut triples = vec![Transform::IDENTITY; 3];
//! skeleton.compose_with_triples(&local, &mut model, &mut triples).unwrap();
//! let mut skin = vec![Mat4::IDENTITY; 3];
//! skinning_matrices(&skeleton, &triples, &mut skin).unwrap();
//! assert!((model[0].w_axis.z - 0.5).abs() < 1e-3);
//! ```
//!
//! # Hostile input
//!
//! Nothing here panics on malformed data. Skeletons are validated once (parents precede
//! children, at most [`MAX_BONES`] bones, finite invertible reference transforms), clips
//! bind only when their track count equals the bone count, sampled values that are not
//! finite or beyond [`BROKEN_TRANSLATION`] / [`BROKEN_SCALE`] fall back to the reference
//! transform, model transforms that still overflow fall back to their parent's (see
//! [`Skeleton::compose_counted`]), non-finite skinning matrices become the identity, attach
//! tables and bone maps are validated against the bone count, and every caller buffer is
//! length-checked. Allocation is bounded by the bone count, the number of layers
//! ([`Animator::MAX_LAYERS`]) and the cache budgets.

#![warn(missing_docs)]

mod attach;
mod blend;
mod cache;
mod clip;
mod deform;
mod error;
mod figure;
mod motion;
mod player;
mod scheduler;
mod skeleton;
mod skin;
mod slider;

pub use attach::{
    AttachFrame, AttachPoint, AttachPointDesc, AttachTable, BoneMap, MAX_ATTACH_ENTRIES,
    attach_table_for,
};
pub use blend::{blend_poses, blend_transform, layer_over, scalar_blend_weights};
pub use cache::{BakeCache, BakeStats, ClipDecoder, SkeletonDecoder};
pub use clip::{
    BROKEN_SCALE, BROKEN_TRANSLATION, BakedClip, BoundClip, SampleReport, Wrap, clip_heap_size,
    clip_time,
};
pub use deform::{BindDeformation, DeformClip, deformer_time};
pub use error::{AnimError, Result};
pub use figure::{BoundHeldClip, BoundShape, MAX_HELD_CLIPS, bind_shape, held_clip_fraction};
pub use motion::{MotionDelta, RootMotionMode, motion_at, motion_between};
pub use player::{
    Animator, ClipSample, CrossFade, EvalScratch, Layer, LayerClip, LayerSnapshot, MAX_LAYER_CLIPS,
    MIN_BONE_WEIGHT, Playback, Source, SpeedBlend, evaluate, quantise_weight,
};
pub use scheduler::{
    AnimationScheduler, Collected, InstanceId, PoseOutput, SchedulerConfig, SchedulerStats,
    SubmitError, Submitted,
};
pub use skeleton::{MAX_BONES, Skeleton, local_matrix, quat_of, track_name_matches};
pub use skin::{
    CompactPalette, PALETTE_CAPACITY, blended_matrix, check_joints, influence_list, skin_direction,
    skin_position, skinning_matrices, write_palette_mat4, write_palette_rows3x4,
};

pub use slider::{BoneSliders, SliderChannels, SliderEntry};

pub use glam;
