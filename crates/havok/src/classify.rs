// SPDX-License-Identifier: GPL-3.0-or-later
//! Record classification for census and exit-criterion reporting (`havok-data.md` §9).

use crate::clip::{
    Animation, Clip, Envelope, INTERLEAVED_CLASS, REFERENCE_FRAME_CLASS, SPLINE_CLASS,
};
use crate::error::{FailureCategory, HavokError};
use crate::skeleton::{SKELETON_CLASS, Skeleton, setup_tagfile};
use crate::tagfile::Tagfile;
use crate::transform::Transform;

/// The kind of structure a record holds, as far as it could be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StructureKind {
    /// An `hkaSkeleton` (0x04 field 3).
    Skeleton,
    /// A clip whose animation is an `hkaSplineCompressedAnimation`.
    SplineClip,
    /// A clip whose animation is an `hkaInterleavedUncompressedAnimation`.
    InterleavedClip,
    /// A tagfile whose object 1 is an `hkaDefaultAnimatedReferenceFrame`.
    ReferenceFrame,
    /// Anything else, or not determinable (the container did not parse).
    Unknown,
}

impl StructureKind {
    /// A short lowercase name.
    pub fn name(self) -> &'static str {
        match self {
            StructureKind::Skeleton => "skeleton",
            StructureKind::SplineClip => "spline_clip",
            StructureKind::InterleavedClip => "interleaved_clip",
            StructureKind::ReferenceFrame => "reference_frame",
            StructureKind::Unknown => "unknown",
        }
    }

    fn of_root(tagfile: &[u8]) -> Self {
        let Ok(tf) = Tagfile::parse(tagfile) else {
            return StructureKind::Unknown;
        };
        let Some(root) = tf.root() else {
            return StructureKind::Unknown;
        };
        let name = tf.class_name(root.class);
        if name == SKELETON_CLASS.as_bytes() {
            StructureKind::Skeleton
        } else if name == SPLINE_CLASS.as_bytes() {
            StructureKind::SplineClip
        } else if name == INTERLEAVED_CLASS.as_bytes() {
            StructureKind::InterleavedClip
        } else if name == REFERENCE_FRAME_CLASS.as_bytes() {
            StructureKind::ReferenceFrame
        } else {
            StructureKind::Unknown
        }
    }
}

/// The outcome of classifying one record: its structure kind and, if it failed, the error
/// (whose [`HavokError::category`] is the record's single failure category, §9.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    /// Structure kind.
    pub kind: StructureKind,
    /// The failure, `None` when the record decodes per §9.1 or §9.2.
    pub error: Option<HavokError>,
}

impl Classification {
    /// The failure category, `None` for a record that decodes.
    pub fn failure(&self) -> Option<FailureCategory> {
        self.error.as_ref().map(HavokError::category)
    }

    /// Whether the record decodes.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Classifies a skeleton tagfile (0x04 field 3) by the rule of §9.1.
pub fn classify_skeleton_tagfile(tagfile: &[u8]) -> Classification {
    match Skeleton::parse(tagfile) {
        Ok(_) => Classification {
            kind: StructureKind::Skeleton,
            error: None,
        },
        Err(e) => Classification {
            kind: StructureKind::of_root(tagfile),
            error: Some(e),
        },
    }
}

/// Classifies a 0x04 setup record (fields 1–3 via [`setup_tagfile`]) by the rule of §9.1.
pub fn classify_setup_record(record: &[u8]) -> Classification {
    match setup_tagfile(record) {
        Ok(t) => classify_skeleton_tagfile(t),
        Err(e) => Classification {
            kind: StructureKind::Unknown,
            error: Some(e),
        },
    }
}

/// Classifies a 0x05 clip record by the rule of §9.2, including sampling every frame of
/// every track. `scratch` is reused across calls.
pub fn classify_clip_record(record: &[u8], scratch: &mut Vec<Transform>) -> Classification {
    let kind = || match Envelope::parse(record) {
        Ok(env) => StructureKind::of_root(env.tagfile),
        Err(_) => StructureKind::Unknown,
    };
    match Clip::parse(record) {
        Ok(clip) => {
            let kind = clip_kind(&clip.animation);
            Classification {
                kind,
                error: clip.animation.check_samples_finite(scratch).err(),
            }
        }
        Err(e) => Classification {
            kind: kind(),
            error: Some(e),
        },
    }
}

fn clip_kind(a: &Animation) -> StructureKind {
    match a.data {
        crate::clip::AnimationData::Spline(_) => StructureKind::SplineClip,
        crate::clip::AnimationData::Interleaved(_) => StructureKind::InterleavedClip,
    }
}
