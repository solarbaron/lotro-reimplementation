// SPDX-License-Identifier: GPL-3.0-or-later
//! Skeletons: the `hkaSkeleton` object of a 0x04 setup's tagfile (`havok-data.md` §4.2,
//! decode rule §9.1).

use std::borrow::Cow;
use std::sync::Arc;

use crate::error::{HavokError, Result};
use crate::tagfile::{SharedStrings, Tagfile, Value, latin1};
use crate::transform::{Transform, quat_length};
use crate::{KNOWN_CLASSES, class_error};

/// Class name of a skeleton object.
pub const SKELETON_CLASS: &str = "hkaSkeleton";

/// One bone of a skeleton.
#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    /// Bone name as stored (`None` when the skeleton has no name column). Names are bytes;
    /// use [`Bone::name_lossy`] for display. Bones naming the same pool string share one
    /// allocation.
    pub name: Option<Arc<[u8]>>,
    /// The `lockTranslation` flag (0 = false when the column is absent).
    pub lock_translation: bool,
    /// Parent bone index, `None` for a root. Smaller than the bone's own index in every
    /// skeleton the parsers return (hand-built values are not checked).
    pub parent: Option<usize>,
    /// Reference (bind) pose relative to the parent.
    pub reference: Transform,
}

impl Bone {
    /// The name decoded as Latin-1, if present.
    pub fn name_lossy(&self) -> Option<String> {
        self.name.as_deref().map(latin1)
    }
}

/// A decoded skeleton. Bones are in stored order: the order mesh bone indices and clip
/// tracks address, with parents before their children.
///
/// The fields are public so tests and tools can build skeletons by hand. The invariants
/// stated here (at least one bone, every parent before its child) hold for every value the
/// parsers return; code that accepts hand-built skeletons must check them itself, for
/// example before a one-pass parent-to-child walk.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    /// The bones; never empty in parsed values.
    pub bones: Vec<Bone>,
}

/// Fields 1–3 of a 0x04 setup record (`setup.md` §6.1): returns the skeleton tagfile slice.
///
/// Only the DID and the tagfile length are read; the rest of the setup is left to
/// `tessera-formats`. A length past the record end is reported as
/// [`HavokError::Truncated`].
pub fn setup_tagfile(record: &[u8]) -> Result<&[u8]> {
    if record.len() < 8 {
        return Err(HavokError::Truncated {
            offset: 0,
            what: "setup DID and tagfile length",
        });
    }
    let t = u32::from_le_bytes([record[4], record[5], record[6], record[7]]) as usize;
    record
        .get(8..8usize.saturating_add(t))
        .ok_or(HavokError::Truncated {
            offset: 8,
            what: "setup tagfile",
        })
}

impl Skeleton {
    /// Parses a skeleton tagfile (0x04 field 3) and checks the decode rule of §9.1.
    pub fn parse(tagfile: &[u8]) -> Result<Self> {
        let tf = Tagfile::parse(tagfile)?;
        Self::from_tagfile(&tf)
    }

    /// Parses the skeleton of a 0x04 setup record (fields 1–3, [`setup_tagfile`]).
    pub fn from_setup_record(record: &[u8]) -> Result<Self> {
        Self::parse(setup_tagfile(record)?)
    }

    /// Builds the skeleton from object 1 of a parsed tagfile.
    pub fn from_tagfile(tf: &Tagfile<'_>) -> Result<Self> {
        let Some(root) = tf.root() else {
            return Err(HavokError::ObjectMissing {
                expected: SKELETON_CLASS,
                found: None,
            });
        };
        let cname = tf.class_name(root.class);
        if cname != SKELETON_CLASS.as_bytes() {
            return Err(class_error(cname, SKELETON_CLASS, KNOWN_CLASSES));
        }
        let c = root.class;
        let v = &root.values;
        let parents: &[i32] = match tf.typed(c, v, SKELETON_CLASS, "parentIndices", 0x12)? {
            Some(Value::IntArray { values, .. }) => values,
            _ => &[],
        };
        let pose: &[[f32; 12]] = match tf.typed(c, v, SKELETON_CLASS, "referencePose", 0x16)? {
            Some(Value::Vec12Array(values)) => values,
            _ => &[],
        };
        let (bone_count, names, locks) = match tf.typed(c, v, SKELETON_CLASS, "bones", 0x19)? {
            Some(Value::StructArray(sa)) => {
                let names = match tf.typed(sa.class, &sa.columns, "hkaBone", "name", 0x0A)? {
                    Some(Value::CStringArray(ids)) => Some(ids),
                    _ => None,
                };
                let locks =
                    match tf.typed(sa.class, &sa.columns, "hkaBone", "lockTranslation", 0x01)? {
                        Some(Value::Bytes(b)) => Some(*b),
                        _ => None,
                    };
                (sa.len, names, locks)
            }
            _ => (0, None, None),
        };
        if bone_count == 0 || parents.len() != bone_count || pose.len() != bone_count {
            return Err(HavokError::inconsistent(format!(
                "skeleton lengths: {} bones, {} parents, {} pose elements",
                bone_count,
                parents.len(),
                pose.len()
            )));
        }
        let mut bones = Vec::with_capacity(bone_count);
        let mut shared = SharedStrings::default();
        for i in 0..bone_count {
            let p = parents[i];
            let parent = if p == -1 {
                None
            } else if p >= 0 && (p as usize) < i {
                Some(p as usize)
            } else {
                return Err(HavokError::inconsistent(format!(
                    "bone {i} has parent index {p}"
                )));
            };
            let reference = Transform::from_vec12(&pose[i]);
            if !reference.is_finite() {
                return Err(HavokError::inconsistent(Cow::Owned(format!(
                    "bone {i} reference pose is not finite"
                ))));
            }
            let len = quat_length(reference.rotation);
            if (len - 1.0).abs() > 1e-3 {
                return Err(HavokError::inconsistent(format!(
                    "bone {i} reference rotation has length {len}"
                )));
            }
            bones.push(Bone {
                name: names.map(|ids| shared.get(tf, ids[i])),
                lock_translation: locks.is_some_and(|b| b[i] != 0),
                parent,
                reference,
            });
        }
        Ok(Skeleton { bones })
    }

    /// Number of bones.
    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    /// Parent of bone `i`, `None` for a root or an out-of-range index.
    pub fn parent(&self, i: usize) -> Option<usize> {
        self.bones.get(i).and_then(|b| b.parent)
    }

    /// Index of the bone whose stored name equals `name`, if any.
    pub fn find_bone(&self, name: &[u8]) -> Option<usize> {
        self.bones
            .iter()
            .position(|b| b.name.as_deref() == Some(name))
    }

    /// The reference pose as one transform per bone.
    pub fn reference_pose(&self) -> impl ExactSizeIterator<Item = Transform> + '_ {
        self.bones.iter().map(|b| b.reference)
    }
}
