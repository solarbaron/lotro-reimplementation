// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone sliders (`appearance.md` §6.3, A16): per-bone entries built from slider
//! definitions and positions, applied to a local pose.
//!
//! The entries shape the **deformation pose** of the bind-space deformation
//! ([`crate::BindDeformation`], `appearance.md` §6.4); they are never applied to the
//! animated pose (`skeleton-anim.md` §10 step 3).
//!
//! This module holds the entries and their application. Reading slider definitions and
//! value strings is `tessera-formats`' job; the caller maps a value byte to a position
//! *t* = byte / 255 (*t* = 0.5 for a slider without a byte, §6.3).

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

    /// Whether the sets share a channel.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
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

/// One bone's entry (`appearance.md` §6.3 "Per-bone entries"): the channel values the
/// sliders wrote for it and the channels written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderEntry {
    /// The bone (an index into the skeleton of the setup).
    pub bone: u16,
    /// Translation (metres), added to the local translation when any translate bit is set.
    pub translate: Vec3,
    /// Angles x, y, z (radians) of the `RotateX/Y/Z` channels.
    pub angles: Vec3,
    /// Scale, multiplied into the local scale per axis when any scale bit is set.
    pub scale: Vec3,
    /// Channels written by any slider.
    pub mask: SliderChannels,
}

impl SliderEntry {
    fn new(bone: u16) -> Self {
        Self {
            bone,
            translate: Vec3::ZERO,
            angles: Vec3::ZERO,
            scale: Vec3::ONE,
            mask: SliderChannels::NONE,
        }
    }

    /// The entry's rotation: *R*_Y(angle x) · *R*_Z(angle y) · *R*_X(angle z) (§6.3, BIN:
    /// the `RotateX` channel turns about local +Y, `RotateY` about +Z, `RotateZ` about +X).
    #[must_use]
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.angles.x)
            * Quat::from_rotation_z(self.angles.y)
            * Quat::from_rotation_x(self.angles.z)
    }
}

/// The bone sliders of one character, resolved to one entry per bone (`appearance.md`
/// §6.3).
///
/// Built for one skeleton rig (checked by [`BoneSliders::fits`] through its bone count and
/// content key). Sliders are added in definition order; a later slider that writes the
/// same channel of the same bone **replaces** the earlier value (nothing accumulates).
#[derive(Debug, Clone, PartialEq)]
pub struct BoneSliders {
    bones: usize,
    skeleton_key: u64,
    entries: Vec<SliderEntry>,
}

impl BoneSliders {
    /// Most bones one slider may name (the data has under 40 sliders of 1–2 bones each).
    pub const MAX_OPS: usize = 1024;

    /// An empty set for `skeleton`.
    #[must_use]
    pub fn new(skeleton: &Skeleton) -> Self {
        Self {
            bones: skeleton.bone_count(),
            skeleton_key: skeleton.content_key(),
            entries: Vec::new(),
        }
    }

    /// Builds the entries of `sliders` in definition order: each item is (channel bits as
    /// [`SliderChannels::from_bits`] takes them, `min`, `max`, position *t*, bones). A slider
    /// with no channel bit writes nothing and is skipped (`appearance.md` §6.3). The problems
    /// are returned for the log (at most 64):
    ///
    /// - a bone index outside the skeleton loses only that bone's entry, the slider's other
    ///   bones keep theirs (`appearance.md` §6.3, BIN4, A30): one
    ///   [`AnimError::BoneIndexOutOfRange`] per such bone;
    /// - a slider whose limits, position or channel values are not finite contributes no
    ///   entry (the §6.3 Tessera Decision) and one [`AnimError::BadSlider`] is returned;
    /// - beyond [`BoneSliders::MAX_OPS`] bones the slider is skipped.
    pub fn build<'a>(
        skeleton: &Skeleton,
        sliders: impl IntoIterator<Item = (u16, [f32; 3], [f32; 3], f32, &'a [(i64, bool)])>,
    ) -> (Self, Vec<AnimError>) {
        let mut out = Self::new(skeleton);
        let mut errors = Vec::new();
        for (bits, min, max, t, bones) in sliders {
            let channels = SliderChannels::from_bits(bits);
            if channels == SliderChannels::NONE {
                continue;
            }
            match out.add_slider(channels, min, max, t, bones) {
                Ok(skipped) => {
                    for &(bone, _) in skipped_bones(bones, out.bones).take(skipped) {
                        if errors.len() < 64 {
                            errors.push(AnimError::BoneIndexOutOfRange {
                                what: "bone slider",
                                bone: usize::try_from(bone).unwrap_or(usize::MAX),
                                bones: out.bones,
                            });
                        }
                    }
                }
                Err(e) => {
                    if errors.len() < 64 {
                        errors.push(e);
                    }
                }
            }
        }
        (out, errors)
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

    /// The per-bone entries, in the order their bones were first written.
    #[must_use]
    pub fn entries(&self) -> &[SliderEntry] {
        &self.entries
    }

    /// The entry of `bone`, if a slider wrote one.
    #[must_use]
    pub fn entry(&self, bone: usize) -> Option<&SliderEntry> {
        self.entries.iter().find(|e| usize::from(e.bone) == bone)
    }

    /// Whether there is nothing to apply.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds one slider at position `t` for its bones (`appearance.md` §6.3, BIN): channel
    /// vector *c* = `min` + *t* × (`max` − `min`) per component, with *t* used as given (the
    /// client does not limit it; a value piece with a minus sign gives a *t* far above 1);
    /// for every `(bone, reverse)`, with sign *σ* = −1 for a reversed bone, every channel of
    /// `channels` **writes** *σ* × the matching component of *c* into the bone's entry
    /// (`TranslateX/Y/Z` → translation, `RotateX/Y/Z` → angles, `ScaleX/Y/Z` → scale) and
    /// joins the entry's mask. Unwritten components keep 0 (scale 1).
    ///
    /// A bone index outside the skeleton (negative ones included) is passed over: the client
    /// walks the skeleton's bones and never uses such an entry, and the slider's other
    /// bones still get theirs (`appearance.md` §6.3, BIN4, A30). Returns how many bones were
    /// passed over.
    ///
    /// Nothing is changed when an error is returned.
    ///
    /// # Errors
    ///
    /// [`AnimError::BadSlider`] for a non-finite `t`, limit or channel value (the §6.3
    /// Tessera Decision: such a slider contributes no entry);
    /// [`AnimError::TooManyEntries`] for more than [`BoneSliders::MAX_OPS`] bones.
    pub fn add_slider(
        &mut self,
        channels: SliderChannels,
        min: [f32; 3],
        max: [f32; 3],
        t: f32,
        bones: &[(i64, bool)],
    ) -> Result<usize> {
        if !t.is_finite() {
            return Err(AnimError::BadSlider {
                what: "slider position",
            });
        }
        if !min.iter().chain(&max).all(|v| v.is_finite()) {
            return Err(AnimError::BadSlider { what: "limit" });
        }
        let value: [f32; 3] = std::array::from_fn(|k| min[k] + t * (max[k] - min[k]));
        if !value.iter().all(|v| v.is_finite()) {
            return Err(AnimError::BadSlider {
                what: "channel value",
            });
        }
        if bones.len() > Self::MAX_OPS {
            return Err(AnimError::TooManyEntries {
                table: "bone slider bones",
                count: bones.len(),
                max: Self::MAX_OPS,
            });
        }
        let mut skipped = 0;
        for &(bone, reverse) in bones {
            let Some(b) = bone_in(bone, self.bones) else {
                skipped += 1;
                continue;
            };
            let sign = if reverse { -1.0f32 } else { 1.0 };
            let i = match self.entries.iter().position(|e| e.bone == b) {
                Some(i) => i,
                None => {
                    self.entries.push(SliderEntry::new(b));
                    self.entries.len() - 1
                }
            };
            let e = &mut self.entries[i];
            for (k, v) in value.iter().enumerate() {
                if channels.axis(0, k) {
                    e.translate[k] = sign * v;
                }
                if channels.axis(3, k) {
                    e.angles[k] = sign * v;
                }
                if channels.axis(6, k) {
                    e.scale[k] = sign * v;
                }
            }
            e.mask = e.mask | channels;
        }
        Ok(skipped)
    }

    /// Applies every entry to `local` (a local pose of the skeleton, at least
    /// [`BoneSliders::bone_count`] entries; `appearance.md` §6.3 "Applying an entry"): with
    /// a translate bit the translation is added, with a rotate bit the local rotation is
    /// post-multiplied by [`SliderEntry::rotation`], with a scale bit the scale is
    /// multiplied per axis. Rotations stay normalised.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if `local` is too short.
    pub fn apply(&self, local: &mut [Transform]) -> Result<()> {
        check_len("local pose", self.bones, local.len())?;
        let translate =
            SliderChannels::TRANSLATE_X | SliderChannels::TRANSLATE_Y | SliderChannels::TRANSLATE_Z;
        let rotate = SliderChannels::ROTATE_X | SliderChannels::ROTATE_Y | SliderChannels::ROTATE_Z;
        let scale = SliderChannels::SCALE_X | SliderChannels::SCALE_Y | SliderChannels::SCALE_Z;
        for e in &self.entries {
            let Some(t) = local.get_mut(usize::from(e.bone)) else {
                continue;
            };
            if e.mask.intersects(translate) {
                for k in 0..3 {
                    t.translation[k] += e.translate[k];
                }
            }
            if e.mask.intersects(rotate) {
                let r = (Quat::from_array(t.rotation) * e.rotation()).normalize();
                if r.is_finite() {
                    t.rotation = r.to_array();
                }
            }
            if e.mask.intersects(scale) {
                for k in 0..3 {
                    t.scale[k] *= e.scale[k];
                }
            }
        }
        Ok(())
    }
}

/// `bone` as an index of a skeleton of `count` bones, if it is one.
fn bone_in(bone: i64, count: usize) -> Option<u16> {
    usize::try_from(bone)
        .ok()
        .filter(|&b| b < count)
        .and_then(|b| u16::try_from(b).ok())
}

/// The bones of `bones` outside a skeleton of `count` bones.
fn skipped_bones(bones: &[(i64, bool)], count: usize) -> impl Iterator<Item = &(i64, bool)> {
    bones
        .iter()
        .filter(move |(b, _)| bone_in(*b, count).is_none())
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
        let xyz = SliderChannels::SCALE_X | SliderChannels::SCALE_Y | SliderChannels::SCALE_Z;
        for (t, expect) in [
            (1.0f32, 1.1f32),
            (0.0, 0.9),
            (128.0 / 255.0, 0.9 + 0.2 * 128.0 / 255.0),
            (0.5, 1.0),
        ] {
            let mut sl = BoneSliders::new(&s);
            sl.add_slider(xyz, [0.9; 3], [1.1; 3], t, &[(7, false)])
                .unwrap();
            let mut local = s.reference_pose();
            sl.apply(&mut local).unwrap();
            for k in 0..3 {
                assert!((local[7].scale[k] - expect).abs() < 1e-6, "{t}");
            }
        }
        let mut sl = BoneSliders::new(&s);
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
        assert!((local[12].translation[0] + 0.003).abs() < 1e-7);
        assert!((local[13].translation[0] - 0.003).abs() < 1e-7);
        assert_eq!(local[12].translation[1], 0.0);
        assert_eq!(local[0], Transform::IDENTITY);
    }

    /// `appearance.md` §12.4: overwrite, sign and rotation axes.
    #[test]
    fn worked_example_12_4() {
        let s = skel(24);
        let mut sl = BoneSliders::new(&s);
        sl.add_slider(
            SliderChannels::SCALE_X,
            [1.0; 3],
            [1.2; 3],
            1.0,
            &[(5, false)],
        )
        .unwrap();
        sl.add_slider(
            SliderChannels::SCALE_X | SliderChannels::SCALE_Z,
            [0.9; 3],
            [0.9; 3],
            0.5,
            &[(5, false)],
        )
        .unwrap();
        let e = sl.entry(5).unwrap();
        assert_eq!(e.scale, Vec3::new(0.9, 1.0, 0.9));
        assert_eq!(e.mask, SliderChannels::SCALE_X | SliderChannels::SCALE_Z);

        sl.add_slider(
            SliderChannels::ROTATE_X,
            [-0.1, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            1.0,
            &[(19, true), (22, false)],
        )
        .unwrap();
        assert_eq!(sl.entry(19).unwrap().angles, Vec3::new(-0.1, 0.0, 0.0));
        assert_eq!(sl.entry(22).unwrap().angles, Vec3::new(0.1, 0.0, 0.0));
        let mut local = s.reference_pose();
        sl.apply(&mut local).unwrap();
        // A turn about local +Y.
        let q = Quat::from_array(local[19].rotation);
        let expect = Quat::from_rotation_y(-0.1);
        assert!(
            q.abs_diff_eq(expect, 1e-6) || q.abs_diff_eq(-expect, 1e-6),
            "{q}"
        );
        let q = Quat::from_array(local[22].rotation);
        assert!(q.abs_diff_eq(Quat::from_rotation_y(0.1), 1e-6), "{q}");

        let e = SliderEntry {
            angles: Vec3::new(0.3, -0.7, 1.1),
            ..SliderEntry::new(0)
        };
        let q = e.rotation();
        let expect = Quat::from_xyzw(0.44180, -0.05754, -0.36242, 0.81863);
        assert!(q.abs_diff_eq(expect, 1e-4), "{q}");
    }

    #[test]
    fn fixed_sliders_apply_at_any_position() {
        let s = skel(4);
        let mut sl = BoneSliders::new(&s);
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
        assert_eq!(local[3].scale, [1.0, 1.0, 0.8]);
        assert_eq!(local[3].translation, [0.0; 3]);
        assert_eq!(local[3].rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn hostile_inputs_are_rejected() {
        let s = skel(3);
        let mut sl = BoneSliders::new(&s);
        let ch = SliderChannels::TRANSLATE_Z;
        // `appearance.md` §6.3 (BIN4, A30): a bone outside the skeleton loses only its own
        // entry; the slider's other bones keep theirs.
        assert_eq!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(3, false)])
                .unwrap(),
            1
        );
        assert_eq!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(-1, false)])
                .unwrap(),
            1
        );
        assert!(sl.is_empty());
        assert_eq!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &[(0, false), (9, false)])
                .unwrap(),
            1
        );
        assert_eq!(sl.entries().len(), 1);
        assert_eq!(sl.entry(0).unwrap().translate, Vec3::new(0.0, 0.0, 1.0));
        // Values that are not finite: the slider contributes no entry (§6.3 Decision).
        let mut sl = BoneSliders::new(&s);
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
        assert!(sl.is_empty());
        let too_many = vec![(0i64, false); BoneSliders::MAX_OPS + 1];
        assert!(matches!(
            sl.add_slider(ch, [0.0; 3], [1.0; 3], 1.0, &too_many),
            Err(AnimError::TooManyEntries { .. })
        ));
        // Positions are used as given (§6.3: the client does not limit t).
        sl.add_slider(ch, [0.0; 3], [1.0; 3], 7.0, &[(0, false)])
            .unwrap();
        let mut short = vec![Transform::IDENTITY; 2];
        assert!(sl.apply(&mut short).is_err());
        let mut local = s.reference_pose();
        sl.apply(&mut local).unwrap();
        assert_eq!(local[0].translation, [0.0, 0.0, 7.0]);
        assert!(sl.fits(&s));
        assert!(!sl.fits(&skel(4)));
        assert_eq!(SliderChannels::from_bits(0xffff).bits(), 0x1ff);
        assert!(SliderChannels::from_bits(0x1ff).contains(SliderChannels::SCALE_Z));
    }

    /// `BoneSliders::build` keeps the in-range bones of a slider that names a bone outside
    /// the skeleton and reports each such bone; a slider without channels is skipped.
    #[test]
    fn build_keeps_the_other_bones() {
        let s = skel(4);
        let bones: [(i64, bool); 3] = [(1, false), (7, false), (2, true)];
        let none: [(i64, bool); 1] = [(9, false)];
        let (sl, errors) = BoneSliders::build(
            &s,
            [
                (
                    SliderChannels::SCALE_Z.bits(),
                    [1.0, 1.0, 0.8],
                    [1.0, 1.0, 0.8],
                    0.5,
                    &bones[..],
                ),
                (0, [0.0; 3], [0.0; 3], 0.5, &none[..]),
            ],
        );
        assert_eq!(sl.entries().len(), 2);
        assert_eq!(sl.entry(1).unwrap().scale, Vec3::new(1.0, 1.0, 0.8));
        assert_eq!(sl.entry(2).unwrap().scale, Vec3::new(1.0, 1.0, -0.8));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(matches!(
            errors[0],
            AnimError::BoneIndexOutOfRange { bone: 7, .. }
        ));
    }
}
