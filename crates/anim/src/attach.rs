// SPDX-License-Identifier: GPL-3.0-or-later
//! Attach points (`skeleton-anim.md` §8): the holding-location table (setup frame list A,
//! §8.2) and the semantic bone map (`attach_map`, §8.1), built from plain data the caller
//! passes in (for example the typed values `tessera-formats` decodes from a setup).

use glam::{Mat4, Quat, Vec3};

use crate::error::{AnimError, Result};
use crate::skeleton::Skeleton;

/// Most entries an attach table or bone map accepts (observed: 46 bone-map elements, a few
/// dozen holding locations per setup).
pub const MAX_ATTACH_ENTRIES: usize = 4096;

/// A holding-location frame as stored in a setup: position, scale and rotation, the
/// rotation in **(w, x, y, z)** order (`setup.md` §6.3; `skeleton-anim.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachFrame {
    /// Translation in the anchor's frame.
    pub position: [f32; 3],
    /// Per-axis scale.
    pub scale: [f32; 3],
    /// Rotation quaternion (w, x, y, z), active.
    pub rotation_wxyz: [f32; 4],
}

impl AttachFrame {
    /// Position 0, scale 1, identity rotation.
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        scale: [1.0; 3],
        rotation_wxyz: [1.0, 0.0, 0.0, 0.0],
    };

    /// The frame as a matrix T(position) · R(rotation) · S(scale) (§8.2, §5.3), or `None`
    /// when a value is not finite or the rotation cannot be normalised.
    #[must_use]
    pub fn matrix(&self) -> Option<Mat4> {
        let [w, x, y, z] = self.rotation_wxyz;
        let q = Quat::from_xyzw(x, y, z, w);
        let len = q.length();
        if !(len.is_finite() && len > 1e-6) {
            return None;
        }
        let p = Vec3::from_array(self.position);
        let s = Vec3::from_array(self.scale);
        if !(p.is_finite() && s.is_finite()) {
            return None;
        }
        let m = Mat4::from_scale_rotation_translation(s, q / len, p);
        m.is_finite().then_some(m)
    }
}

impl Default for AttachFrame {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One holding location as plain input (setup frame list A, §8.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachPointDesc {
    /// `HoldingLocation` enum key.
    pub key: u32,
    /// Bone index into the setup's skeleton, or −1 for the setup origin (object space).
    pub bone: i32,
    /// The frame in the bone's space.
    pub frame: AttachFrame,
}

/// A validated holding location.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachPoint {
    /// `HoldingLocation` enum key.
    pub key: u32,
    /// The bone it hangs on, `None` for the setup origin.
    pub bone: Option<u16>,
    /// The frame matrix in the bone's space (§8.2).
    pub frame: Mat4,
}

/// A setup's holding locations (§8.2), keyed by `HoldingLocation` enum key. A location's
/// model transform at a pose is **W**_bone · T(position) · R(rotation) · S(scale), or the
/// frame alone for bone −1; times the object's world placement it places a held item's
/// origin (§8.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttachTable {
    points: Vec<AttachPoint>,
}

impl AttachTable {
    /// Builds and validates the table for a skeleton of `bone_count` bones (KI4: keys
    /// unique, bone −1 or below the bone count; frames finite).
    ///
    /// # Errors
    ///
    /// [`AnimError::TooManyEntries`], [`AnimError::DuplicateKey`],
    /// [`AnimError::BadAttachBone`] or [`AnimError::BadAttachFrame`] for the first
    /// offending entry.
    pub fn new(entries: &[AttachPointDesc], bone_count: usize) -> Result<Self> {
        if entries.len() > MAX_ATTACH_ENTRIES {
            return Err(AnimError::TooManyEntries {
                table: "holding location",
                count: entries.len(),
                max: MAX_ATTACH_ENTRIES,
            });
        }
        let mut points = Vec::with_capacity(entries.len());
        for e in entries {
            points.push(validate_point(e, bone_count)?);
        }
        points.sort_by_key(|p| p.key);
        if let Some(w) = points.windows(2).find(|w| w[0].key == w[1].key) {
            return Err(AnimError::DuplicateKey {
                table: "holding location",
                key: w[0].key,
            });
        }
        Ok(Self { points })
    }

    /// Like [`AttachTable::new`], but skips offending entries (keeping the first of
    /// duplicate keys, and at most [`MAX_ATTACH_ENTRIES`] entries) and returns why each
    /// was skipped, so one bad location does not hide the others.
    #[must_use]
    pub fn new_skipping_invalid(
        entries: &[AttachPointDesc],
        bone_count: usize,
    ) -> (Self, Vec<AnimError>) {
        let mut issues = Vec::new();
        let mut points: Vec<AttachPoint> = Vec::new();
        for e in entries {
            if points.len() >= MAX_ATTACH_ENTRIES {
                issues.push(AnimError::TooManyEntries {
                    table: "holding location",
                    count: entries.len(),
                    max: MAX_ATTACH_ENTRIES,
                });
                break;
            }
            match validate_point(e, bone_count) {
                Ok(p) => points.push(p),
                Err(err) => issues.push(err),
            }
        }
        // Stable sort keeps the first of equal keys first.
        points.sort_by_key(|p| p.key);
        let mut out: Vec<AttachPoint> = Vec::with_capacity(points.len());
        for p in points {
            if out.last().is_some_and(|l| l.key == p.key) {
                issues.push(AnimError::DuplicateKey {
                    table: "holding location",
                    key: p.key,
                });
            } else {
                out.push(p);
            }
        }
        (Self { points: out }, issues)
    }

    /// The locations, sorted by key.
    #[must_use]
    pub fn points(&self) -> &[AttachPoint] {
        &self.points
    }

    /// Number of locations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The location with `key`, if the setup has it (absent: do not draw the item, §8.4).
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&AttachPoint> {
        self.points
            .binary_search_by_key(&key, |p| p.key)
            .ok()
            .map(|i| &self.points[i])
    }

    /// The location's model transform at a pose given by model-space bone transforms
    /// (§8.2). `None` when the key is absent or the bone lies beyond `model`.
    #[must_use]
    pub fn model_transform(&self, key: u32, model: &[Mat4]) -> Option<Mat4> {
        self.get(key).and_then(|p| point_model(p, model))
    }

    /// The location's world transform: `object_world` · model transform (§5.3, §8.4).
    #[must_use]
    pub fn world_transform(&self, key: u32, model: &[Mat4], object_world: Mat4) -> Option<Mat4> {
        self.model_transform(key, model).map(|m| object_world * m)
    }

    /// Model transforms of every location at a pose into `out` (cleared first), as
    /// (key, transform) pairs in key order; locations whose bone lies beyond `model` are
    /// left out.
    pub fn model_transforms(&self, model: &[Mat4], out: &mut Vec<(u32, Mat4)>) {
        out.clear();
        out.extend(
            self.points
                .iter()
                .filter_map(|p| point_model(p, model).map(|m| (p.key, m))),
        );
    }
}

fn point_model(p: &AttachPoint, model: &[Mat4]) -> Option<Mat4> {
    match p.bone {
        None => Some(p.frame),
        Some(b) => model.get(usize::from(b)).map(|w| *w * p.frame),
    }
}

fn validate_point(e: &AttachPointDesc, bone_count: usize) -> Result<AttachPoint> {
    let bone = match e.bone {
        -1 => None,
        b if b >= 0 && (b as usize) < bone_count.min(usize::from(u16::MAX)) => Some(b as u16),
        b => {
            return Err(AnimError::BadAttachBone {
                table: "holding location",
                key: e.key,
                bone: i64::from(b),
                bones: bone_count,
            });
        }
    };
    let frame = e
        .frame
        .matrix()
        .ok_or(AnimError::BadAttachFrame { key: e.key })?;
    Ok(AttachPoint {
        key: e.key,
        bone,
        frame,
    })
}

/// A setup's `attach_map` (§8.1): `BoneType` enum key → bone index. Effect and script
/// properties name bones by `BoneType`; an absent key means the skeleton has no such bone
/// (fall back to the object origin).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoneMap {
    entries: Vec<(u32, u16)>,
}

impl BoneMap {
    /// Builds and validates the map for a skeleton of `bone_count` bones (KI3: keys unique,
    /// values unique, values below the bone count).
    ///
    /// # Errors
    ///
    /// [`AnimError::TooManyEntries`], [`AnimError::BadAttachBone`],
    /// [`AnimError::DuplicateKey`] or [`AnimError::DuplicateBone`].
    pub fn new(pairs: &[(u32, u32)], bone_count: usize) -> Result<Self> {
        if pairs.len() > MAX_ATTACH_ENTRIES {
            return Err(AnimError::TooManyEntries {
                table: "bone map",
                count: pairs.len(),
                max: MAX_ATTACH_ENTRIES,
            });
        }
        let mut entries = Vec::with_capacity(pairs.len());
        for &(key, bone) in pairs {
            if (bone as usize) >= bone_count || bone > u32::from(u16::MAX) {
                return Err(AnimError::BadAttachBone {
                    table: "bone map",
                    key,
                    bone: i64::from(bone),
                    bones: bone_count,
                });
            }
            entries.push((key, bone as u16));
        }
        let mut bones: Vec<u16> = entries.iter().map(|e| e.1).collect();
        bones.sort_unstable();
        if let Some(w) = bones.windows(2).find(|w| w[0] == w[1]) {
            return Err(AnimError::DuplicateBone {
                bone: u32::from(w[0]),
            });
        }
        entries.sort_by_key(|e| e.0);
        if let Some(w) = entries.windows(2).find(|w| w[0].0 == w[1].0) {
            return Err(AnimError::DuplicateKey {
                table: "bone map",
                key: w[0].0,
            });
        }
        Ok(Self { entries })
    }

    /// The bone a `BoneType` key names, if the skeleton has it.
    #[must_use]
    pub fn bone_for(&self, key: u32) -> Option<usize> {
        self.entries
            .binary_search_by_key(&key, |e| e.0)
            .ok()
            .map(|i| usize::from(self.entries[i].1))
    }

    /// The (key, bone) pairs, sorted by key.
    #[must_use]
    pub fn entries(&self) -> &[(u32, u16)] {
        &self.entries
    }

    /// The model transform of the bone a `BoneType` key names at a pose, or `None` (use
    /// the object origin then).
    #[must_use]
    pub fn model_transform(&self, key: u32, model: &[Mat4]) -> Option<Mat4> {
        model.get(self.bone_for(key)?).copied()
    }
}

/// Convenience: a table for `skeleton` (see [`AttachTable::new`]).
///
/// # Errors
///
/// As [`AttachTable::new`].
pub fn attach_table_for(skeleton: &Skeleton, entries: &[AttachPointDesc]) -> Result<AttachTable> {
    AttachTable::new(entries, skeleton.bone_count())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `skeleton-anim.md` §14.4.
    #[test]
    fn attach_example_14_4() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let table = AttachTable::new(
            &[AttachPointDesc {
                key: 7,
                bone: 0,
                frame: AttachFrame {
                    position: [0.0, -0.2, 0.1],
                    scale: [1.0; 3],
                    rotation_wxyz: [h, -h, 0.0, 0.0],
                },
            }],
            1,
        )
        .unwrap();
        let model = [Mat4::from_translation(Vec3::new(0.8, 0.0, 1.36))];
        let m = table.model_transform(7, &model).unwrap();
        assert!(
            m.w_axis
                .truncate()
                .abs_diff_eq(Vec3::new(0.8, -0.2, 1.46), 1e-6)
        );
        assert!(m.transform_vector3(Vec3::Y).abs_diff_eq(-Vec3::Z, 1e-6));
        assert!(m.transform_vector3(Vec3::Z).abs_diff_eq(Vec3::Y, 1e-6));
        let world = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let w = table.world_transform(7, &model, world).unwrap();
        assert!(
            w.w_axis
                .truncate()
                .abs_diff_eq(Vec3::new(10.8, -0.2, 1.46), 1e-5)
        );
        assert!(table.model_transform(8, &model).is_none());
        assert!(table.model_transform(7, &[]).is_none());
    }

    #[test]
    fn origin_anchor_and_validation() {
        let e = |key, bone| AttachPointDesc {
            key,
            bone,
            frame: AttachFrame {
                position: [1.0, 2.0, 3.0],
                ..AttachFrame::IDENTITY
            },
        };
        let t = AttachTable::new(&[e(1, -1)], 0).unwrap();
        let m = t.model_transform(1, &[]).unwrap();
        assert_eq!(m.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));
        assert!(matches!(
            AttachTable::new(&[e(1, 3)], 3),
            Err(AnimError::BadAttachBone { bone: 3, .. })
        ));
        assert!(matches!(
            AttachTable::new(&[e(1, -2)], 3),
            Err(AnimError::BadAttachBone { .. })
        ));
        assert!(matches!(
            AttachTable::new(&[e(1, 0), e(1, 1)], 3),
            Err(AnimError::DuplicateKey { key: 1, .. })
        ));
        let mut bad = e(2, 0);
        bad.frame.rotation_wxyz = [0.0; 4];
        assert!(matches!(
            AttachTable::new(&[bad], 3),
            Err(AnimError::BadAttachFrame { key: 2 })
        ));
        let (t, issues) = AttachTable::new_skipping_invalid(&[e(5, 0), e(5, 1), bad, e(3, 9)], 3);
        assert_eq!(t.len(), 1);
        assert_eq!(issues.len(), 3);
        assert_eq!(t.get(5).unwrap().bone, Some(0));
        let mut out = Vec::new();
        t.model_transforms(&[Mat4::IDENTITY], &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn bone_map_ki3() {
        let m = BoneMap::new(&[(3, 2), (1, 1)], 4).unwrap();
        assert_eq!(m.bone_for(3), Some(2));
        assert_eq!(m.bone_for(9), None);
        assert!(matches!(
            BoneMap::new(&[(3, 2), (3, 1)], 4),
            Err(AnimError::DuplicateKey { .. })
        ));
        assert!(matches!(
            BoneMap::new(&[(3, 2), (4, 2)], 4),
            Err(AnimError::DuplicateBone { bone: 2 })
        ));
        assert!(matches!(
            BoneMap::new(&[(3, 4)], 4),
            Err(AnimError::BadAttachBone { .. })
        ));
        let model = [Mat4::IDENTITY, Mat4::from_translation(Vec3::X)];
        assert_eq!(m.model_transform(1, &model), Some(model[1]));
        assert_eq!(m.model_transform(3, &model), None);
    }
}
