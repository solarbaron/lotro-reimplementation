// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone sliders: per-instance modifiers of the sampled local pose (`appearance.md` §6.3,
//! §8 step 7), applied after clips are sampled and before model transforms are composed
//! (`skeleton-anim.md` §10).
//!
//! This module holds the **application** only. Reading slider definitions and value
//! strings is `tessera-formats`' job, and mapping a stored value byte to a slider position
//! is the caller's (the spec's Decision is *t* = byte / 255; the exact mapping is OPEN O9).

use glam::{Quat, Vec3};
use tessera_havok::Transform;

use crate::error::{AnimError, Result};
use crate::skeleton::{Skeleton, check_len};

/// The channels a slider drives (`BoneSlider_BoneTranformMaskArray`, `appearance.md`
/// §6.3), as Tessera's own bit set. The data's enum keys are mapped to these by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SliderChannels(u16);

impl SliderChannels {
    /// Translation along the bone's local x.
    pub const TRANSLATE_X: Self = Self(1 << 0);
    /// Translation along the bone's local y.
    pub const TRANSLATE_Y: Self = Self(1 << 1);
    /// Translation along the bone's local z.
    pub const TRANSLATE_Z: Self = Self(1 << 2);
    /// Rotation about the bone's local x.
    pub const ROTATE_X: Self = Self(1 << 3);
    /// Rotation about the bone's local y.
    pub const ROTATE_Y: Self = Self(1 << 4);
    /// Rotation about the bone's local z.
    pub const ROTATE_Z: Self = Self(1 << 5);
    /// Scale along the bone's local x.
    pub const SCALE_X: Self = Self(1 << 6);
    /// Scale along the bone's local y.
    pub const SCALE_Y: Self = Self(1 << 7);
    /// Scale along the bone's local z.
    pub const SCALE_Z: Self = Self(1 << 8);
    /// No channel.
    pub const NONE: Self = Self(0);

    /// The set with bits `bits` (bits above the nine channels are ignored).
    #[must_use]
    pub fn from_bits(bits: u16) -> Self {
        Self(bits & 0x1ff)
    }

    /// The raw bits.
    #[must_use]
    pub fn bits(self) -> u16 {
        self.0
    }

    /// Whether every channel of `other` is in the set.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two sets.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn axis(self, base: u16, k: usize) -> bool {
        self.0 & (1 << (base as usize + k)) != 0
    }
}

impl std::ops::BitOr for SliderChannels {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

/// One bone's change from one slider, in the bone's local frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderOp {
    /// The bone (an index into the skeleton of the setup).
    pub bone: u16,
    /// Added to the bone's local translation (metres).
    pub translate: Vec3,
    /// Post-multiplied onto the bone's local rotation (unit quaternion).
    pub rotate: Quat,
    /// Multiplies the bone's local scale per axis.
    pub scale: Vec3,
}

/// The bone sliders of one character, resolved to per-bone operations on a local pose.
///
/// Built for one skeleton rig (checked by [`Skeleton::same_rig`] through its bone count
/// and content key when paired with an instance). Operations apply in the order added:
/// translations add, rotations post-multiply, scales multiply (`appearance.md` §6.3
/// Decision "application").
#[derive(Debug, Clone, PartialEq)]
pub struct BoneSliders {
    bones: usize,
    skeleton_key: u64,
    ops: Vec<SliderOp>,
}

impl BoneSliders {
    /// Most operations one set holds (the data has under 40 sliders of 1–2 bones each).
    pub const MAX_OPS: usize = 1024;

    /// An empty set for `skeleton`.
    #[must_use]
    pub fn new(skeleton: &Skeleton) -> Self {
        Self {
            bones: skeleton.bone_count(),
            skeleton_key: skeleton.content_key(),
            ops: Vec::new(),
        }
    }

    /// Bone count of the skeleton the set was built for.
    #[must_use]
    pub fn bone_count(&self) -> usize {
        self.bones
    }

    /// Whether the set was built for a skeleton posing like `skeleton` (same bone count and
    /// content key).
    #[must_use]
    pub fn fits(&self, skeleton: &Skeleton) -> bool {
        self.bones == skeleton.bone_count() && self.skeleton_key == skeleton.content_key()
    }

    /// The operations in application order.
    #[must_use]
    pub fn ops(&self) -> &[SliderOp] {
        &self.ops
    }

    /// Whether there is nothing to apply.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Adds one slider at position `t` (`0 ..= 1`, clamped) for its bones, following
    /// `appearance.md` §6.3: the channel value is `min + t · (max − min)` per component
    /// (so a slider with `min` = `max` always applies that value), and only the components
    /// of `channels` are used. For each `(bone, reverse)`:
    ///
    /// - translation channels add the value, negated on x for a reversed bone;
    /// - rotation channels rotate about the bone's local axis by the value in radians,
    ///   negated for a reversed bone (several rotation channels compose as x, then y,
    ///   then z; the data drives one at a time);
    /// - scale channels multiply the scale on that axis.
    ///
    /// Nothing is added when an error is returned.
    ///
    /// # Errors
    ///
    /// [`AnimError::BadSlider`] for a non-finite `t`, limit or rotation;
    /// [`AnimError::BoneIndexOutOfRange`] for a bone index outside the skeleton;
    /// [`AnimError::TooManyEntries`] beyond [`BoneSliders::MAX_OPS`] operations.
    pub fn add_slider(
        &mut self,
        channels: SliderChannels,
        min: [f32; 3],
        max: [f32; 3],
        t: f32,
        bones: &[(i64, bool)],
    ) -> Result<()> {
        if !t.is_finite() {
            return Err(AnimError::BadSlider {
                what: "slider position",
            });
        }
        if !min.iter().chain(&max).all(|v| v.is_finite()) {
            return Err(AnimError::BadSlider { what: "limit" });
        }
        let t = t.clamp(0.0, 1.0);
        let value: [f32; 3] = std::array::from_fn(|k| min[k] + t * (max[k] - min[k]));
        if !value.iter().all(|v| v.is_finite()) {
            return Err(AnimError::BadSlider {
                what: "channel value",
            });
        }
        let count = self.ops.len().saturating_add(bones.len());
        if count > Self::MAX_OPS {
            return Err(AnimError::TooManyEntries {
                table: "bone slider operations",
                count,
                max: Self::MAX_OPS,
            });
        }
        let mut new = Vec::with_capacity(bones.len());
        for &(bone, reverse) in bones {
            let b = usize::try_from(bone)
                .ok()
                .filter(|&b| b < self.bones)
                .and_then(|b| u16::try_from(b).ok())
                .ok_or(AnimError::BoneIndexOutOfRange {
                    what: "bone slider",
                    bone: usize::try_from(bone).unwrap_or(usize::MAX),
                    bones: self.bones,
                })?;
            let pick = |base: u16, k: usize, unset: f32| {
                if channels.axis(base, k) {
                    value[k]
                } else {
                    unset
                }
            };
            let mut translate = Vec3::from_array(std::array::from_fn(|k| pick(0, k, 0.0)));
            if reverse {
                translate.x = -translate.x;
            }
            let sign = if reverse { -1.0 } else { 1.0 };
            let angles: [f32; 3] = std::array::from_fn(|k| sign * pick(3, k, 0.0));
            let rotate = (Quat::from_rotation_x(angles[0])
                * Quat::from_rotation_y(angles[1])
                * Quat::from_rotation_z(angles[2]))
            .normalize();
            if !rotate.is_finite() {
                return Err(AnimError::BadSlider { what: "rotation" });
            }
            let scale = Vec3::from_array(std::array::from_fn(|k| pick(6, k, 1.0)));
            new.push(SliderOp {
                bone: b,
                translate,
                rotate,
                scale,
            });
        }
        self.ops.extend(new);
        Ok(())
    }

    /// Applies every operation to `local` (a local pose of the skeleton, at least
    /// [`BoneSliders::bone_count`] entries). Rotations stay normalised.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if `local` is too short.
    pub fn apply(&self, local: &mut [Transform]) -> Result<()> {
        check_len("local pose", self.bones, local.len())?;
        for op in &self.ops {
            let Some(t) = local.get_mut(usize::from(op.bone)) else {
                continue;
            };
            for k in 0..3 {
                t.translation[k] += op.translate[k];
                t.scale[k] *= op.scale[k];
            }
            let r = (Quat::from_array(t.rotation) * op.rotate).normalize();
            if r.is_finite() {
                t.rotation = r.to_array();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skel(n: usize) -> Skeleton {
        let parents: Vec<i32> = (0..n as i32).map(|i| i - 1).collect();
        Skeleton::from_parts(&parents, &vec![Transform::IDENTITY; n]).unwrap()
    }

    /// `appearance.md` §12.3.
    #[test]
    fn worked_example_12_3() {
        let s = skel(16);
        let mut sl = BoneSliders::new(&s);
        let xyz = SliderChannels::SCALE_X | SliderChannels::SCALE_Y | SliderChannels::SCALE_Z;
        sl.add_slider(xyz, [0.9; 3], [1.1; 3], 255.0 / 255.0, &[(7, false)])
            .unwrap();
        sl.add_slider(
            SliderChannels::TRANSLATE_X,
            [-0.003, 0.0, 0.0],
            [0.003, 0.0, 0.0],
            1.0,
            &[(12, true), (13, false)],
        )
        .unwrap();
        let mut local = s.reference_pose();
        sl.apply(&mut local).unwrap();
        for k in 0..3 {
            assert!((local[7].scale[k] - 1.1).abs() < 1e-6);
        }
        assert!((local[12].translation[0] + 0.003).abs() < 1e-7);
        assert!((local[13].translation[0] - 0.003).abs() < 1e-7);
        assert_eq!(local[12].translation[1], 0.0);
        assert_eq!(local[0], Transform::IDENTITY);

        // Byte 0x00 → 0.9; byte 0x80 → 1.00039 (the spec's Decision mapping).
        for (byte, expect) in [(0u8, 0.9f32), (0x80, 0.9 + 0.2 * 128.0 / 255.0)] {
            let mut sl = BoneSliders::new(&s);
            sl.add_slider(
                xyz,
                [0.9; 3],
                [1.1; 3],
                f32::from(byte) / 255.0,
                &[(7, false)],
            )
            .unwrap();
            let mut local = s.reference_pose();
            sl.apply(&mut local).unwrap();
            assert!((local[7].scale[1] - expect).abs() < 1e-6, "{byte}");
        }
    }

    #[test]
    fn rotation_mirror_and_fixed_sliders() {
        let s = skel(4);
        let mut sl = BoneSliders::new(&s);
        // Eye rotation about local x, mirrored on bone 1.
        sl.add_slider(
            SliderChannels::ROTATE_X,
            [-0.1, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            1.0,
            &[(1, true), (2, false)],
        )
        .unwrap();
        // Hidden slider with min = max: applies regardless of the position.
        sl.add_slider(
            SliderChannels::SCALE_Z,
            [1.0, 1.0, 0.8],
            [1.0, 1.0, 0.8],
            0.0,
            &[(3, false)],
        )
        .unwrap();
        let mut local = s.reference_pose();
        sl.apply(&mut local).unwrap();
        let angle = |q: [f32; 4]| 2.0 * q[0].atan2(q[3]);
        assert!((angle(local[1].rotation) + 0.1).abs() < 1e-6);
        assert!((angle(local[2].rotation) - 0.1).abs() < 1e-6);
        assert_eq!(local[3].scale, [1.0, 1.0, 0.8]);
        // Unmasked components are untouched.
        assert_eq!(local[1].translation, [0.0; 3]);
        assert_eq!(local[1].scale, [1.0; 3]);
    }

    #[test]
    fn hostile_inputs_are_rejected() {
        let s = skel(3);
        let mut sl = BoneSliders::new(&s);
        let ch = SliderChannels::TRANSLATE_Z;
        assert!(matches!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(3, false)]),
            Err(AnimError::BoneIndexOutOfRange { bone: 3, .. })
        ));
        assert!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(-1, false)])
                .is_err()
        );
        assert!(
            sl.add_slider(ch, [f32::NAN; 3], [1.0; 3], 1.0, &[(0, false)])
                .is_err()
        );
        assert!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], f32::INFINITY, &[(0, false)])
                .is_err()
        );
        assert!(
            sl.add_slider(ch, [-f32::MAX; 3], [f32::MAX; 3], 0.5, &[(0, false)])
                .is_err()
        );
        // A failed slider adds nothing, even when its first bones were valid.
        assert!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(0, false), (9, false)])
                .is_err()
        );
        assert!(sl.is_empty());
        let too_many = vec![(0i64, false); BoneSliders::MAX_OPS + 1];
        assert!(matches!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &too_many),
            Err(AnimError::TooManyEntries { .. })
        ));
        // Positions are clamped.
        sl.add_slider(ch, [0.0; 3], [1.0; 3], 7.0, &[(0, false)])
            .unwrap();
        let mut short = vec![Transform::IDENTITY; 2];
        assert!(sl.apply(&mut short).is_err());
        let mut local = s.reference_pose();
        sl.apply(&mut local).unwrap();
        assert_eq!(local[0].translation, [0.0, 0.0, 1.0]);
        assert!(sl.fits(&s));
        assert!(!sl.fits(&skel(4)));
        assert_eq!(SliderChannels::from_bits(0xffff).bits(), 0x1ff);
        assert!(SliderChannels::from_bits(0x1ff).contains(SliderChannels::SCALE_Z));
    }
}
