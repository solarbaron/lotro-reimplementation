// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-havok`: a clean-room reader for exactly the subset of Havok-format binary data
//! found in the archives (skeletons inside setup records, animation clips: spline-compressed
//! with 40-bit quaternion tracks, uncompressed interleaved, annotation tracks).
//!
//! It never links against or calls Havok code, and no third-party implementation is ported.
//! Spec: `docs/specs/havok-data.md` (ROADMAP M4), "the spec" below.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`Tagfile::parse`] | Generic binary tagfile v3: varints, string pool, classes, objects, struct arrays | §3, §7.1 |
//! | [`Skeleton::parse`], [`Skeleton::from_setup_record`] | Bones, parents, names (bytes), reference pose | §4.2, §9.1 |
//! | [`Clip::parse`] | A 0x05 record: envelope, animation, annotations, reference frames, weights | §2.3, §7.2 |
//! | [`Envelope::parse`] | The 0x05 envelope alone | §2.3 |
//! | [`Animation`], [`SplineAnimation`], [`InterleavedAnimation`] | Clip data models | §4.3, §5, §6 |
//! | [`SplineBlock::parse`] | One spline block: masks, channels, quantisation, curves | §5.2–§5.7, §7.3 |
//! | [`decode_quat40`] | The 40-bit quaternion | §5.6 |
//! | [`Curve`], [`find_span`], [`basis_functions`] | B-spline evaluation | §7.5, §7.6 |
//! | [`Clip::sample`], [`Animation::sample`] | All tracks at time *t* into a caller buffer, no allocation | §7.4–§7.8 |
//! | [`Clip::binding`] | Positional track-to-bone binding (no binding object exists) | §4.4, §4.6 |
//! | [`classify_clip_record`], [`classify_setup_record`] | Structure kind and failure category | §9 |
//! | [`testkit`] | Synthetic builders for every structure kind | §12 |
//!
//! # Example
//!
//! ```
//! use tessera_havok::{Clip, Skeleton, Transform};
//!
//! # fn demo(setup_record: &[u8], clip_record: &[u8]) -> Result<(), tessera_havok::HavokError> {
//! let skeleton = Skeleton::from_setup_record(setup_record)?;
//! let clip = Clip::parse(clip_record)?;
//! if clip.binding().fits(&skeleton) {
//!     let mut pose = vec![Transform::IDENTITY; clip.track_count()];
//!     clip.sample(0.5, &mut pose); // Track j drives bone j.
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Hostile input
//!
//! Decoders never panic on malformed data and report a [`HavokError`] whose
//! [`HavokError::category`] is one of the spec's failure categories (§9.3). Every count is
//! checked against the bytes left before anything is allocated; class, member, inheritance,
//! object and decoded-value counts and the curve degree are capped
//! ([`tagfile::MAX_CLASSES`], [`tagfile::MAX_FULL_MEMBERS`],
//! [`tagfile::MAX_INHERITANCE_DEPTH`], [`tagfile::MAX_OBJECTS`], [`tagfile::MAX_VALUES`],
//! [`MAX_DEGREE`]). Work and memory follow the input's size, not the counts stored in it:
//! struct classes resolve through a name index whose keys are hashed once per pool string,
//! empty struct-array columns are not stored, pool strings named many times are copied once
//! and shared, annotation tracks stop
//! at the transform track count, and [`Animation::check_samples_finite`] samples a block
//! without curves once (any block with curves has at most 256 frames, its knots being
//! bytes). Sampling clamps every time and frame and uses checked arithmetic, so hand-built
//! models with odd numbers do not panic either; the invariants documented on model fields
//! are guaranteed only for parsed values.
//!
//! # Fuzz targets (ROADMAP M4 exit criterion 5)
//!
//! Each structure kind of spec §11 is reached by at least one target in `fuzz/`; every
//! target has a synthetic mode that builds its input with [`testkit`] and then damages it.
//!
//! | Kind (§11) | Fuzz target |
//! |---|---|
//! | K1 container header | `havok_tagfile` |
//! | K2 varint and string pool | `havok_tagfile` |
//! | K3 type table (metadata) | `havok_tagfile` |
//! | K4 object stream and references | `havok_tagfile` |
//! | K5 struct arrays | `havok_tagfile` |
//! | K6 skeleton | `havok_skeleton` |
//! | K7 0x05 envelope | `havok_clip_record` |
//! | K8 spline object fields and block table | `havok_clip_record` |
//! | K9 spline block and channels | `havok_spline_block` |
//! | K10 curve evaluation and sampling | `havok_clip_record`, `havok_spline_block` |
//! | K11 interleaved clip | `havok_clip_record` |
//! | K12 annotation tracks | `havok_clip_record` |
//! | K13 reference frame | `havok_clip_record` |

#![warn(missing_docs)]

mod classify;
mod clip;
mod error;
mod skeleton;
mod spline;
pub mod tagfile;
pub mod testkit;
mod transform;

pub use classify::{
    Classification, StructureKind, classify_clip_record, classify_setup_record,
    classify_skeleton_tagfile,
};
pub use clip::{
    Animation, AnimationData, Annotation, AnnotationTrack, Clip, Envelope, INTERLEAVED_CLASS,
    InterleavedAnimation, REFERENCE_FRAME_CLASS, ReferenceFrame, ReferenceFrameSource,
    SPLINE_CLASS, TrackBinding,
};
pub use error::{Channel, FailureCategory, HavokError, Result};
pub use skeleton::{Bone, SKELETON_CLASS, Skeleton, setup_tagfile};
pub use spline::{
    BlockParams, Curve, CurveRef, MAX_DEGREE, QUANT_8BIT, QUANT_16BIT, ROT_QUANT_40BIT,
    RotationChannel, SplineAnimation, SplineBlock, TrackChannels, VectorChannel, basis_functions,
    clamp_time, decode_quat40, find_span,
};
pub use tagfile::Tagfile;
pub use transform::{Transform, quat_dot, quat_length, quat_nlerp};

/// The 13 classes `havok-data.md` §4.1 describes. Object 1 of another class is
/// `object.unknown_class`; of one of these but the wrong one, `object.missing`.
pub const KNOWN_CLASSES: &[&str] = &[
    "hkBaseObject",
    "hkReferencedObject",
    "hkaSkeleton",
    "hkaBone",
    "hkaSkeletonLocalFrameOnBone",
    "hkLocalFrame",
    "hkaAnimation",
    "hkaAnimatedReferenceFrame",
    "hkaAnnotationTrack",
    "hkaAnnotationTrackAnnotation",
    "hkaSplineCompressedAnimation",
    "hkaInterleavedUncompressedAnimation",
    "hkaDefaultAnimatedReferenceFrame",
];

/// The error for an object 1 of class `found` where `expected` was needed.
pub(crate) fn class_error(found: &[u8], expected: &'static str, known: &[&str]) -> HavokError {
    let name = tagfile::latin1(found);
    if known.iter().any(|k| k.as_bytes() == found) {
        HavokError::ObjectMissing {
            expected,
            found: Some(name),
        }
    } else {
        HavokError::UnknownClass { name }
    }
}
