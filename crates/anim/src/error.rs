// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type.

use tessera_havok::HavokError;

/// Result alias of this crate.
pub type Result<T, E = AnimError> = std::result::Result<T, E>;

/// Why a skeleton, binding, attach table or output could not be built.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum AnimError {
    /// A skeleton with no bones.
    #[error("skeleton has no bones")]
    EmptySkeleton,
    /// More bones than [`MAX_BONES`](crate::MAX_BONES).
    #[error("skeleton has {count} bones, more than the supported {max}")]
    TooManyBones {
        /// Bone count of the input.
        count: usize,
        /// The limit.
        max: usize,
    },
    /// A parent index that does not precede its child (which also rules out cycles and
    /// self-parenting), or is out of range.
    #[error("bone {bone} has parent {parent}, which does not precede it")]
    BadParent {
        /// The bone.
        bone: usize,
        /// Its stored parent index.
        parent: i64,
    },
    /// A reference transform with a non-finite value, or a rotation that cannot be
    /// normalised.
    #[error("bone {bone} reference transform is not usable ({what})")]
    BadReference {
        /// The bone.
        bone: usize,
        /// What is wrong.
        what: &'static str,
    },
    /// The model-space reference transform of a bone cannot be inverted (zero scale).
    #[error("bone {bone} reference transform is singular; no inverse-bind matrix")]
    SingularBind {
        /// The bone.
        bone: usize,
    },
    /// A clip whose track count differs from the skeleton's bone count
    /// (`skeleton-anim.md` §3.4, category "track count mismatch").
    #[error("track count mismatch: clip has {tracks} tracks, skeleton has {bones} bones")]
    TrackCountMismatch {
        /// Transform tracks of the clip.
        tracks: usize,
        /// Bones of the skeleton.
        bones: usize,
    },
    /// A caller buffer smaller than needed.
    #[error("{what} buffer holds {got} entries, {needed} needed")]
    BufferTooSmall {
        /// Which buffer.
        what: &'static str,
        /// Entries needed.
        needed: usize,
        /// Entries given.
        got: usize,
    },
    /// An attach or bone-map entry naming a bone the skeleton does not have.
    #[error("{table} entry {key:#x} names bone {bone}, skeleton has {bones} bones")]
    BadAttachBone {
        /// `"holding location"` or `"bone map"`.
        table: &'static str,
        /// The entry's enum key.
        key: u32,
        /// The stored bone index.
        bone: i64,
        /// Bones of the skeleton.
        bones: usize,
    },
    /// Two entries with the same key (`skeleton-anim.md` KI3, KI4).
    #[error("{table} has key {key:#x} more than once")]
    DuplicateKey {
        /// `"holding location"` or `"bone map"`.
        table: &'static str,
        /// The key.
        key: u32,
    },
    /// Two bone-map entries naming the same bone (KI3: values unique).
    #[error("bone map names bone {bone} more than once")]
    DuplicateBone {
        /// The bone.
        bone: u32,
    },
    /// An attach frame with a non-finite value.
    #[error("holding location {key:#x} has a non-finite frame")]
    BadAttachFrame {
        /// The key.
        key: u32,
    },
    /// More entries than a table accepts.
    #[error("{table} has {count} entries, more than {max}")]
    TooManyEntries {
        /// Which table.
        table: &'static str,
        /// Entries given.
        count: usize,
        /// The limit.
        max: usize,
    },
    /// An instance id the scheduler does not know (removed, or from another scheduler).
    #[error("unknown animation instance {id}")]
    UnknownInstance {
        /// The raw id.
        id: u64,
    },
    /// A bone index at or beyond the skeleton's bone count: a vertex bone index of a
    /// submesh (`skeleton-anim.md` KI1) or a bone-slider target.
    #[error("{what} names bone {bone}, skeleton has {bones} bones")]
    BoneIndexOutOfRange {
        /// What holds the index (`"vertex bone index"`, `"bone slider"`).
        what: &'static str,
        /// The index.
        bone: usize,
        /// Bones of the skeleton.
        bones: usize,
    },
    /// An animator, modifier or baked clip built for a different skeleton (or clip) than
    /// the one it is paired with.
    #[error("{what} does not belong to this skeleton or clip")]
    Mismatch {
        /// What was paired wrongly (`"animator skeleton"`, `"bone sliders"`,
        /// `"baked clip"`).
        what: &'static str,
    },
    /// A bone-slider channel value or slider position that is not finite.
    #[error("bone slider has a non-finite {what}")]
    BadSlider {
        /// Which value.
        what: &'static str,
    },
    /// The Havok reader rejected a record.
    #[error(transparent)]
    Havok(#[from] HavokError),
}

impl AnimError {
    /// A short stable category name for counting failures in reports.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::EmptySkeleton => "skeleton.empty",
            Self::TooManyBones { .. } => "skeleton.too_many_bones",
            Self::BadParent { .. } => "skeleton.bad_parent",
            Self::BadReference { .. } => "skeleton.bad_reference",
            Self::SingularBind { .. } => "skeleton.singular_bind",
            Self::TrackCountMismatch { .. } => "bind.track_count_mismatch",
            Self::BufferTooSmall { .. } => "output.buffer_too_small",
            Self::BadAttachBone { .. } => "attach.bad_bone",
            Self::DuplicateKey { .. } => "attach.duplicate_key",
            Self::DuplicateBone { .. } => "attach.duplicate_bone",
            Self::BadAttachFrame { .. } => "attach.bad_frame",
            Self::TooManyEntries { .. } => "attach.too_many_entries",
            Self::UnknownInstance { .. } => "scheduler.unknown_instance",
            Self::BoneIndexOutOfRange { .. } => "skin.bone_out_of_range",
            Self::Mismatch { .. } => "bind.mismatch",
            Self::BadSlider { .. } => "slider.bad_value",
            Self::Havok(e) => e.category().name(),
        }
    }
}
