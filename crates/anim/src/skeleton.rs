// SPDX-License-Identifier: GPL-3.0-or-later
//! The skeleton hierarchy: validated parents, reference pose, model-space reference
//! transforms and inverse reference triples (`skeleton-anim.md` §2, §4.4, §5).

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use tessera_havok::Transform;

use crate::error::{AnimError, Result};

/// Most bones a [`Skeleton`] may have: one GPU palette of 256 matrices covers every
/// observed skeleton (`skeleton-anim.md` §4.3; the largest has 248 bones).
pub const MAX_BONES: usize = 256;

/// The 4 × 4 matrix of a local transform: **L** = T(*t*) · R(*q*) · S(*s*) acting on
/// column vectors (`skeleton-anim.md` §5.3). The quaternion is (x, y, z, w) and rotates
/// actively (§5.2); it is normalised here, and a zero or non-finite one counts as identity.
#[must_use]
pub fn local_matrix(t: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(t.scale),
        quat_of(t.rotation),
        Vec3::from_array(t.translation),
    )
}

/// A (translation, rotation, per-axis scale) triple in glam types: the form in which the
/// client composes bone transforms (`skeleton-anim.md` §5.4, K17).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Qs {
    pub(crate) t: Vec3,
    pub(crate) q: Quat,
    pub(crate) s: Vec3,
}

impl Qs {
    pub(crate) const IDENTITY: Self = Self {
        t: Vec3::ZERO,
        q: Quat::IDENTITY,
        s: Vec3::ONE,
    };

    /// The triple of a local transform (rotation normalised, identity if it cannot be).
    pub(crate) fn of(t: &Transform) -> Self {
        Self {
            t: Vec3::from_array(t.translation),
            q: quat_of(t.rotation),
            s: Vec3::from_array(t.scale),
        }
    }

    /// The child's model triple from this (the parent's model triple) and the child's
    /// local triple (§5.4): translation *t*_p + *q*_p · *t*_local (the parent's scale does
    /// **not** move the child's joint), rotation *q*_p · *q*_local, scale *s*_p × *s*_local
    /// per axis.
    pub(crate) fn then(self, local: Self) -> Self {
        Self {
            t: self.t + self.q * local.t,
            q: self.q * local.q,
            s: self.s * local.s,
        }
    }

    /// The matrix T(*t*) · R(*q*) · S(*s*) (§5.3 order).
    pub(crate) fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.s, self.q, self.t)
    }

    /// The inverse reference triple *X* of this reference model triple (§5.4, K26):
    /// translation −(*q*⁻¹ applied to *t*), rotation *q*⁻¹, scale 1/*s* per axis. The
    /// translation is **not** divided by the scale, so *X* is the exact inverse only when
    /// the scale is (1, 1, 1).
    pub(crate) fn inverse_reference(self) -> Self {
        let q = self.q.inverse();
        Self {
            t: -(q * self.t),
            q,
            s: self.s.recip(),
        }
    }

    /// The skinning triple of a posed model triple `self` against the inverse reference
    /// triple `x` (§5.4, K26): translation *t* + *q* applied to (*s* ⊙ *x*.t), rotation
    /// *q* · *x*.q, scale *s* ⊙ *x*.s.
    pub(crate) fn skinning(self, x: Self) -> Self {
        Self {
            t: self.t + self.q * (self.s * x.t),
            q: self.q * x.q,
            s: self.s * x.s,
        }
    }

    /// Whether every component is finite.
    pub(crate) fn is_finite(self) -> bool {
        self.t.is_finite() && self.q.is_finite() && self.s.is_finite()
    }

    pub(crate) fn transform(self) -> Transform {
        Transform {
            translation: self.t.to_array(),
            rotation: self.q.to_array(),
            scale: self.s.to_array(),
        }
    }
}

/// A (x, y, z, w) quaternion as a normalised [`Quat`]; identity if it cannot be normalised.
#[must_use]
pub fn quat_of(q: [f32; 4]) -> Quat {
    let q = Quat::from_array(q);
    let len = q.length();
    if len.is_finite() && len > 1e-6 {
        q / len
    } else {
        Quat::IDENTITY
    }
}

/// A skeleton ready for posing: parents validated so that every parent precedes its child
/// (one forward pass composes model transforms, and no cycle can exist), reference
/// rotations normalised, and the model-space reference transforms **B**_b and the inverse
/// reference triples *X*_b (§5.4, K26) precomputed.
///
/// Bones are in stored order: the order clip tracks (§3.2) and mesh bone indices (§4.1)
/// address. Build one per distinct skeleton and share it (`Arc`); identical skeletons can
/// be found by [`Skeleton::content_key`] (§2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    parents: Vec<Option<u16>>,
    reference: Vec<Transform>,
    reference_triples: Vec<Transform>,
    reference_model: Vec<Mat4>,
    inverse_reference: Vec<Transform>,
    inverse_bind: Vec<Mat4>,
    names: Vec<Option<Arc<[u8]>>>,
    key: u64,
}

impl Skeleton {
    /// Builds a skeleton from a decoded Havok skeleton.
    ///
    /// # Errors
    ///
    /// See [`Skeleton::from_parts`]. Parsed skeletons always pass the parent rule; a
    /// hand-built one may not.
    pub fn from_havok(skeleton: &tessera_havok::Skeleton) -> Result<Self> {
        let parents: Vec<i64> = skeleton
            .bones
            .iter()
            .map(|b| {
                b.parent
                    .map_or(-1, |p| i64::try_from(p).unwrap_or(i64::MAX))
            })
            .collect();
        let reference: Vec<Transform> = skeleton.bones.iter().map(|b| b.reference).collect();
        let names: Vec<Option<Arc<[u8]>>> = skeleton.bones.iter().map(|b| b.name.clone()).collect();
        Self::build(&parents, &reference, names)
    }

    /// Decodes the skeleton of a 0x04 setup record (fields 1–3) and builds it.
    ///
    /// # Errors
    ///
    /// [`AnimError::Havok`] when the tagfile does not decode, else as
    /// [`Skeleton::from_parts`].
    pub fn from_setup_record(record: &[u8]) -> Result<Self> {
        let s = tessera_havok::Skeleton::from_setup_record(record)?;
        Self::from_havok(&s)
    }

    /// Builds a skeleton from plain data: one parent index per bone (−1 for a root) and
    /// one reference transform per bone, relative to the parent. Bones have no names.
    ///
    /// # Errors
    ///
    /// [`AnimError::EmptySkeleton`], [`AnimError::TooManyBones`] (more than
    /// [`MAX_BONES`]), [`AnimError::BadParent`] (a parent that is not −1 and not smaller
    /// than the bone's own index, or lengths that differ), [`AnimError::BadReference`]
    /// (non-finite values, a rotation that cannot be normalised) and
    /// [`AnimError::SingularBind`] (a reference transform that cannot be inverted).
    pub fn from_parts(parents: &[i32], reference: &[Transform]) -> Result<Self> {
        let parents: Vec<i64> = parents.iter().map(|&p| i64::from(p)).collect();
        Self::build(&parents, reference, vec![None; reference.len()])
    }

    fn build(
        parents: &[i64],
        reference: &[Transform],
        names: Vec<Option<Arc<[u8]>>>,
    ) -> Result<Self> {
        let n = reference.len();
        if n == 0 {
            return Err(AnimError::EmptySkeleton);
        }
        if n > MAX_BONES {
            return Err(AnimError::TooManyBones {
                count: n,
                max: MAX_BONES,
            });
        }
        if parents.len() != n {
            return Err(AnimError::BadParent {
                bone: parents.len().min(n),
                parent: -2,
            });
        }
        let mut ps = Vec::with_capacity(n);
        for (i, &p) in parents.iter().enumerate() {
            if p == -1 {
                ps.push(None);
            } else if p >= 0 && (p as u64) < i as u64 {
                // i < MAX_BONES, so the parent fits u16.
                ps.push(Some(p as u16));
            } else {
                return Err(AnimError::BadParent { bone: i, parent: p });
            }
        }
        let mut refs = Vec::with_capacity(n);
        for (i, t) in reference.iter().enumerate() {
            if !t.is_finite() {
                return Err(AnimError::BadReference {
                    bone: i,
                    what: "non-finite value",
                });
            }
            let q = Quat::from_array(t.rotation);
            let len = q.length();
            if !(len.is_finite() && len > 1e-6) {
                return Err(AnimError::BadReference {
                    bone: i,
                    what: "rotation cannot be normalised",
                });
            }
            refs.push(Transform {
                rotation: (q / len).to_array(),
                ..*t
            });
        }
        // Model triples by the composition rule of §5.4 (K17).
        let mut triples: Vec<Qs> = Vec::with_capacity(n);
        let mut model: Vec<Mat4> = Vec::with_capacity(n);
        for (i, t) in refs.iter().enumerate() {
            let l = Qs::of(t);
            let m = match ps[i] {
                Some(p) => triples[usize::from(p)].then(l),
                None => l,
            };
            let mat = m.matrix();
            if !mat.is_finite() || !m.q.is_finite() {
                return Err(AnimError::BadReference {
                    bone: i,
                    what: "model transform overflows",
                });
            }
            triples.push(m);
            model.push(mat);
        }
        // Tessera rule (`skeleton-anim.md` §15): a reference that cannot be inverted (a zero
        // or vanishing model scale) is rejected.
        let mut inverse = Vec::with_capacity(n);
        let mut inverse_matrices = Vec::with_capacity(n);
        for (i, (m, r)) in model.iter().zip(&triples).enumerate() {
            let det = m.determinant();
            if !det.is_finite() || det.abs() < 1e-12 {
                return Err(AnimError::SingularBind { bone: i });
            }
            let x = r.inverse_reference();
            let xm = x.matrix();
            if !x.is_finite() || !xm.is_finite() {
                return Err(AnimError::SingularBind { bone: i });
            }
            inverse.push(x.transform());
            inverse_matrices.push(xm);
        }
        let key = content_key(&ps, &refs);
        Ok(Skeleton {
            parents: ps,
            reference: refs,
            reference_triples: triples.iter().map(|q| q.transform()).collect(),
            reference_model: model,
            inverse_reference: inverse,
            inverse_bind: inverse_matrices,
            names,
            key,
        })
    }

    /// Approximate bytes held: the struct plus every per-bone vector (by capacity) and the
    /// heap bytes of the bone names (an `Arc<[u8]>` holds two reference counts and the
    /// bytes). Used to charge skeletons against cache budgets.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        use std::mem::size_of;
        let names: usize = self
            .names
            .iter()
            .flatten()
            .map(|n| 2 * size_of::<usize>() + n.len())
            .sum();
        size_of::<Self>()
            + self.parents.capacity() * size_of::<Option<u16>>()
            + self.reference.capacity() * size_of::<Transform>()
            + self.reference_triples.capacity() * size_of::<Transform>()
            + self.reference_model.capacity() * size_of::<Mat4>()
            + self.inverse_reference.capacity() * size_of::<Transform>()
            + self.inverse_bind.capacity() * size_of::<Mat4>()
            + self.names.capacity() * size_of::<Option<Arc<[u8]>>>()
            + names
    }

    /// Number of bones.
    #[must_use]
    pub fn bone_count(&self) -> usize {
        self.parents.len()
    }

    /// Parent of bone `i` (`None` for a root or an index out of range).
    #[must_use]
    pub fn parent(&self, i: usize) -> Option<usize> {
        self.parents.get(i).copied().flatten().map(usize::from)
    }

    /// Parent of every bone, in bone order.
    #[must_use]
    pub fn parents(&self) -> &[Option<u16>] {
        &self.parents
    }

    /// Indices of the root bones (no parent).
    pub fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        self.parents
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_none())
            .map(|(i, _)| i)
    }

    /// Children of bone `i`, in bone order.
    pub fn children(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.parents
            .iter()
            .enumerate()
            .filter(move |(_, p)| p.map(usize::from) == Some(i))
            .map(|(c, _)| c)
    }

    /// Reference (bind) pose, local to each parent, rotations normalised.
    #[must_use]
    pub fn reference_local(&self) -> &[Transform] {
        &self.reference
    }

    /// Model-space reference transforms **B**_b (§4.4): the matrices T · R · S of the
    /// reference model triples ([`Skeleton::reference_triples`]).
    #[must_use]
    pub fn reference_model(&self) -> &[Mat4] {
        &self.reference_model
    }

    /// Model-space reference triples (translation, rotation, per-axis scale), composed from
    /// the reference pose by the rule of §5.4.
    #[must_use]
    pub fn reference_triples(&self) -> &[Transform] {
        &self.reference_triples
    }

    /// Inverse reference triples *X*_b (§5.4, K26): translation −(*q*_r⁻¹ applied to
    /// *t*_r), rotation *q*_r⁻¹, scale 1/*s*_r per axis, from each bone's reference model
    /// triple (*t*_r, *q*_r, *s*_r). The client does not divide the translation by the
    /// reference scale, so *X*_b is the exact inverse of **B**_b only when *s*_r = (1, 1, 1);
    /// [`crate::skinning_matrices`] uses these triples.
    #[must_use]
    pub fn inverse_reference(&self) -> &[Transform] {
        &self.inverse_reference
    }

    /// The matrices T · R · S of [`Skeleton::inverse_reference`]: the inverse-bind matrices
    /// **B**_b⁻¹ of §4.4 for every bone whose reference model scale is 1 (every
    /// player-skeleton bone), and **not** the inverse of **B**_b for the other bones (§5.4).
    /// Skinning goes through the triples ([`crate::skinning_matrices`]); these matrices are
    /// for inspection and tests.
    #[must_use]
    pub fn inverse_bind(&self) -> &[Mat4] {
        &self.inverse_bind
    }

    /// Stored name of bone `i`, if the skeleton has names.
    #[must_use]
    pub fn name(&self, i: usize) -> Option<&[u8]> {
        self.names.get(i)?.as_deref()
    }

    /// Index of the bone whose stored name equals `name`.
    #[must_use]
    pub fn find_bone(&self, name: &[u8]) -> Option<usize> {
        self.names.iter().position(|n| n.as_deref() == Some(name))
    }

    /// A 64-bit hash of the parents and the reference pose (bit patterns): the skeleton's
    /// identity **for posing**. Skeletons stored in different setups with the same rig have
    /// equal keys, so callers can share bound clips, poses and palettes between them (§2.1,
    /// an optimisation only).
    ///
    /// Bone **names are not part of the key** (nor of [`Skeleton::same_rig`]): skeletons
    /// with equal keys may be named differently, by index or not at all (§3.5). A cache
    /// that shares one `Skeleton` per key must not answer [`Skeleton::name`] or
    /// [`Skeleton::find_bone`] for another setup from it; keep the setup's own skeleton (or
    /// its names) for name lookups. The derived `PartialEq` does compare names.
    #[must_use]
    pub fn content_key(&self) -> u64 {
        self.key
    }

    /// Whether `other` poses identically: the same parents and bit-identical reference
    /// transforms (names ignored, see [`Skeleton::content_key`]). Poses, model transforms
    /// and skinning matrices computed for one apply to the other.
    #[must_use]
    pub fn same_rig(&self, other: &Skeleton) -> bool {
        std::ptr::eq(self, other)
            || (self.key == other.key
                && self.parents == other.parents
                && self.reference.len() == other.reference.len()
                && self.reference.iter().zip(&other.reference).all(|(a, b)| {
                    a.translation
                        .iter()
                        .chain(&a.rotation)
                        .chain(&a.scale)
                        .zip(b.translation.iter().chain(&b.rotation).chain(&b.scale))
                        .all(|(x, y)| x.to_bits() == y.to_bits())
                }))
    }

    /// Composes local transforms into model transforms in one parent-to-child pass by the
    /// triple rule of §5.4 (K17): the parent's scale never moves a child's joint, scales
    /// multiply per axis down the chain, and `model[b]` is the matrix T · R · S of bone
    /// *b*'s model triple. With every scale 1 this is **W**_b = **W**_parent · **L**_b
    /// (§5.3, KI8).
    ///
    /// `local` and `model` must hold at least [`Skeleton::bone_count`] entries; extra
    /// entries are left alone. Every written matrix is finite: see
    /// [`Skeleton::compose_counted`], which this calls.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if either slice is too short.
    pub fn compose(&self, local: &[Transform], model: &mut [Mat4]) -> Result<()> {
        self.compose_counted(local, model).map(|_| ())
    }

    /// [`Skeleton::compose`] that also writes the model **triples** (the input of
    /// [`crate::skinning_matrices`]) in the same pass, returning the fallback count of
    /// [`Skeleton::compose_counted`].
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if a slice is too short.
    pub fn compose_with_triples(
        &self,
        local: &[Transform],
        model: &mut [Mat4],
        triples: &mut [Transform],
    ) -> Result<u32> {
        let n = self.bone_count();
        check_len("local pose", n, local.len())?;
        check_len("model pose", n, model.len())?;
        check_len("model triples", n, triples.len())?;
        let mut fallbacks = 0;
        for i in 0..n {
            let parent = self.parents[i].map(|p| Qs::of(&triples[usize::from(p)]));
            let (m, mat) = self.model_triple(i, parent, Qs::of(&local[i]), &mut fallbacks);
            triples[i] = m.transform();
            model[i] = mat;
        }
        Ok(fallbacks)
    }

    /// [`Skeleton::compose`], returning how many bones fell back because their model
    /// triple or matrix was not finite.
    ///
    /// Tessera Decision (`skeleton-anim.md` implementer notes, broken sample values): a
    /// bone whose composed model triple (or its matrix) is not finite, for example because
    /// finite but huge scales multiplied down the chain overflow, takes its parent's model
    /// triple (it collapses onto the parent joint), and a root takes its reference model
    /// triple. Sampled clips never get there ([`crate::BROKEN_SCALE`]); hand-built poses
    /// can.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if either slice is too short.
    pub fn compose_counted(&self, local: &[Transform], model: &mut [Mat4]) -> Result<u32> {
        let n = self.bone_count();
        check_len("local pose", n, local.len())?;
        check_len("model pose", n, model.len())?;
        let mut triples = [Qs::IDENTITY; MAX_BONES];
        let mut fallbacks = 0;
        for i in 0..n {
            let parent = self.parents[i].map(|p| triples[usize::from(p)]);
            let (m, mat) = self.model_triple(i, parent, Qs::of(&local[i]), &mut fallbacks);
            triples[i] = m;
            model[i] = mat;
        }
        Ok(fallbacks)
    }

    /// Composes local transforms into model **triples** (translation, rotation, per-axis
    /// scale; §5.4), the form [`Skeleton::compose`] turns into matrices, with the same
    /// fallback for triples that are not finite ([`Skeleton::compose_counted`]).
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] if either slice is too short.
    pub fn compose_triples(&self, local: &[Transform], model: &mut [Transform]) -> Result<()> {
        let n = self.bone_count();
        check_len("local pose", n, local.len())?;
        check_len("model pose", n, model.len())?;
        let mut fallbacks = 0;
        for i in 0..n {
            let parent = self.parents[i].map(|p| Qs::of(&model[usize::from(p)]));
            let (m, _) = self.model_triple(i, parent, Qs::of(&local[i]), &mut fallbacks);
            model[i] = m.transform();
        }
        Ok(())
    }

    /// Bone `i`'s model triple and matrix from its parent's model triple and its local
    /// triple, with the fallback of [`Skeleton::compose_counted`]. The parent triple is
    /// finite by induction; the reference triples were checked when the skeleton was built.
    fn model_triple(
        &self,
        i: usize,
        parent: Option<Qs>,
        local: Qs,
        fallbacks: &mut u32,
    ) -> (Qs, Mat4) {
        let m = match parent {
            Some(p) => p.then(local),
            None => local,
        };
        let mat = m.matrix();
        if m.is_finite() && mat.is_finite() {
            return (m, mat);
        }
        *fallbacks += 1;
        match (parent, self.reference_triples.get(i)) {
            (Some(p), _) => (p, p.matrix()),
            (None, Some(r)) => {
                let r = Qs::of(r);
                (r, r.matrix())
            }
            (None, None) => (Qs::IDENTITY, Mat4::IDENTITY),
        }
    }

    /// The reference pose as a fresh local pose vector.
    #[must_use]
    pub fn reference_pose(&self) -> Vec<Transform> {
        self.reference.clone()
    }
}

pub(crate) fn check_len(what: &'static str, needed: usize, got: usize) -> Result<()> {
    if got < needed {
        Err(AnimError::BufferTooSmall { what, needed, got })
    } else {
        Ok(())
    }
}

fn content_key(parents: &[Option<u16>], reference: &[Transform]) -> u64 {
    // FNV-1a, 64 bit: deterministic across runs and machines.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u32| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(parents.len() as u32);
    for (p, t) in parents.iter().zip(reference) {
        eat(p.map_or(u32::MAX, u32::from));
        for v in t.translation.iter().chain(&t.rotation).chain(&t.scale) {
            eat(v.to_bits());
        }
    }
    h
}

/// Whether an annotation track name matches a bone name by the diagnostic rule of
/// `skeleton-anim.md` §3.5: use the last `|`-separated segment of the track name, compare
/// case-insensitively, and drop the bone name's rig-family prefix (up to and including the
/// first `_`); the track name matches when it equals the rest or ends with `_` followed by
/// it. A bone name without `_` is compared whole.
///
/// This is a diagnostic only: binding is positional and never uses names (§3.2).
#[must_use]
pub fn track_name_matches(track_name: &[u8], bone_name: &[u8]) -> bool {
    let track = track_name
        .rsplit(|&b| b == b'|')
        .next()
        .unwrap_or(track_name);
    let bone = match bone_name.iter().position(|&b| b == b'_') {
        Some(i) => &bone_name[i + 1..],
        None => bone_name,
    };
    if bone.is_empty() || track.len() < bone.len() {
        return false;
    }
    if track.eq_ignore_ascii_case(bone) {
        return true;
    }
    let cut = track.len() - bone.len();
    cut >= 1 && track[cut - 1] == b'_' && track[cut..].eq_ignore_ascii_case(bone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(tr: [f32; 3], rot: [f32; 4]) -> Transform {
        Transform {
            translation: tr,
            rotation: rot,
            scale: [1.0; 3],
        }
    }

    /// `skeleton-anim.md` §14.1.
    fn example() -> Skeleton {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        Skeleton::from_parts(
            &[-1, 0, 1],
            &[
                t([0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
                t([0.0, 0.5, 0.0], [0.0, 0.0, h, h]),
                t([0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
            ],
        )
        .unwrap()
    }

    #[test]
    fn composition_example_14_1() {
        let s = example();
        let m = s.reference_model();
        let pos = |i: usize| m[i].w_axis.truncate();
        assert!(pos(0).abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-6));
        assert!(pos(1).abs_diff_eq(Vec3::new(0.0, 0.5, 1.0), 1e-6));
        assert!(
            pos(2).abs_diff_eq(Vec3::new(-1.0, 0.5, 1.0), 1e-6),
            "{}",
            pos(2)
        );
        // Bone 2 inherits the 90° turn about z: its +x axis points along model +y.
        assert!(
            m[2].x_axis
                .truncate()
                .abs_diff_eq(Vec3::new(0.0, 1.0, 0.0), 1e-6)
        );
        for (b, inv) in m.iter().zip(s.inverse_bind()) {
            assert!((*b * *inv).abs_diff_eq(Mat4::IDENTITY, 1e-5));
        }
    }

    /// `skeleton-anim.md` §14.7: scale composition and skinning by the triple rule.
    #[test]
    fn scale_composition_example_14_7() {
        let s = Skeleton::from_parts(
            &[-1, 0, 1],
            &[
                t([0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
                t([0.0, 0.0, -0.45], [0.0, 0.0, 0.0, 1.0]),
                t([0.0, 0.0, -0.41], [0.0, 0.0, 0.0, 1.0]),
            ],
        )
        .unwrap();
        let mut local = s.reference_pose();
        local[0].scale = [1.0, 1.0, 0.8];
        local[1].scale = [1.0, 1.0, 1.15];
        let mut model = vec![Mat4::IDENTITY; 3];
        s.compose(&local, &mut model).unwrap();
        for (b, z) in [(0, 1.0), (1, 0.55), (2, 0.14)] {
            assert!((model[b].w_axis.z - z).abs() < 1e-6, "joint {b}");
        }
        let mut triples = vec![Transform::IDENTITY; 3];
        s.compose_triples(&local, &mut triples).unwrap();
        for (b, sz) in [(0, 0.8), (1, 0.92), (2, 0.92)] {
            assert!((triples[b].scale[2] - sz).abs() < 1e-6, "scale {b}");
        }
        let mut skin = vec![Mat4::IDENTITY; 3];
        crate::skin::skinning_matrices(&s, &triples, &mut skin).unwrap();
        let z = |b: u8, v: f32| {
            crate::skin::skin_position(&skin, &[b], &[1.0], Vec3::new(0.0, 0.0, v)).z
        };
        assert!((z(0, 0.775) - 0.82).abs() < 1e-5);
        assert!((z(1, 0.345) - 0.3614).abs() < 1e-5);
        assert!((z(2, 0.0) - 0.0112).abs() < 1e-5, "{}", z(2, 0.0));
    }

    #[test]
    fn rejects_bad_parents_and_cycles() {
        let r = [Transform::IDENTITY; 3];
        assert!(matches!(
            Skeleton::from_parts(&[-1, 2, 1], &r),
            Err(AnimError::BadParent { bone: 1, parent: 2 })
        ));
        assert!(matches!(
            Skeleton::from_parts(&[0, -1, -1], &r),
            Err(AnimError::BadParent { bone: 0, parent: 0 })
        ));
        assert!(matches!(
            Skeleton::from_parts(&[-1, -7, 0], &r),
            Err(AnimError::BadParent { bone: 1, .. })
        ));
        assert!(matches!(
            Skeleton::from_parts(&[-1, 0], &r),
            Err(AnimError::BadParent { .. })
        ));
        assert_eq!(
            Skeleton::from_parts(&[], &[]),
            Err(AnimError::EmptySkeleton)
        );
        let many = vec![Transform::IDENTITY; MAX_BONES + 1];
        let parents = vec![-1; MAX_BONES + 1];
        assert!(matches!(
            Skeleton::from_parts(&parents, &many),
            Err(AnimError::TooManyBones { .. })
        ));
    }

    #[test]
    fn rejects_bad_reference_values() {
        let mut r = [Transform::IDENTITY; 2];
        r[1].translation[0] = f32::NAN;
        assert!(matches!(
            Skeleton::from_parts(&[-1, 0], &r),
            Err(AnimError::BadReference { bone: 1, .. })
        ));
        let mut r = [Transform::IDENTITY; 2];
        r[0].rotation = [0.0; 4];
        assert!(matches!(
            Skeleton::from_parts(&[-1, 0], &r),
            Err(AnimError::BadReference { bone: 0, .. })
        ));
        let mut r = [Transform::IDENTITY; 2];
        r[1].scale = [0.0, 1.0, 1.0];
        assert_eq!(
            Skeleton::from_parts(&[-1, 0], &r),
            Err(AnimError::SingularBind { bone: 1 })
        );
        let mut r = [Transform::IDENTITY; 2];
        r[0].scale = [1e30; 3];
        r[1].scale = [1e30; 3];
        assert!(Skeleton::from_parts(&[-1, 0], &r).is_err());
    }

    #[test]
    fn unnormalised_reference_rotation_is_normalised() {
        let s = Skeleton::from_parts(&[-1], &[t([0.0; 3], [0.0, 0.0, 0.0, 2.0])]).unwrap();
        assert_eq!(s.reference_local()[0].rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn hierarchy_queries() {
        let s = Skeleton::from_parts(&[-1, 0, 0, 1, -1], &[Transform::IDENTITY; 5]).unwrap();
        assert_eq!(s.roots().collect::<Vec<_>>(), vec![0, 4]);
        assert_eq!(s.children(0).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(s.parent(3), Some(1));
        assert_eq!(s.parent(4), None);
        assert_eq!(s.parent(99), None);
        assert_eq!(s.name(0), None);
    }

    #[test]
    fn content_key_tracks_content() {
        let a = example();
        let b = example();
        assert_eq!(a.content_key(), b.content_key());
        let c = Skeleton::from_parts(&[-1, 0, 0], a.reference_local()).unwrap();
        assert_ne!(a.content_key(), c.content_key());
    }

    #[test]
    fn compose_checks_lengths() {
        let s = example();
        let mut out = vec![Mat4::IDENTITY; 2];
        assert!(matches!(
            s.compose(s.reference_local(), &mut out),
            Err(AnimError::BufferTooSmall { .. })
        ));
        let mut out = vec![Mat4::IDENTITY; 3];
        s.compose(s.reference_local(), &mut out).unwrap();
        for (a, b) in out.iter().zip(s.reference_model()) {
            assert!(a.abs_diff_eq(*b, 1e-6));
        }
    }

    #[test]
    fn track_name_rule_3_5() {
        assert!(track_name_matches(b"Head", b"rig_head"));
        assert!(track_name_matches(b"root|spine|Head", b"rig_head"));
        assert!(track_name_matches(b"sword_head", b"rig_head"));
        assert!(!track_name_matches(b"swordhead", b"rig_head"));
        assert!(!track_name_matches(b"hea", b"rig_head"));
        assert!(track_name_matches(b"head", b"head"));
        assert!(!track_name_matches(b"x", b"rig_"));
    }
}
