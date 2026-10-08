// SPDX-License-Identifier: GPL-3.0-or-later
//! Bind-space deformation of skinned meshes by body-shape deformer clips and bone sliders
//! (`appearance.md` §6.4, A17; `script-table.md` §6.9).
//!
//! The original applies neither the body-shape deformer nor the bone sliders to the
//! animated pose: it deforms the skinned vertices **once, in bind space**, and animates the
//! deformed vertices with the ordinary skeleton. [`BindDeformation::new`] builds the
//! deformation pose and its per-bone transforms *D_b*; [`BindDeformation::deform`] replaces
//! each vertex by Σ *w_k* · *D*_{j_k} · *v* over its own influences.

use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use tessera_havok::Transform;

use crate::clip::BoundClip;
use crate::error::{AnimError, Result};
use crate::player::{Accum, quantise_weight};
use crate::skeleton::Skeleton;
use crate::skin::{blended_matrix, skinning_matrices};
use crate::slider::BoneSliders;

/// One body-shape deformer clip (a MeshDeformer node, `script-table.md` §6.9).
#[derive(Debug, Clone)]
pub struct DeformClip {
    /// The clip, bound to the figure's skeleton.
    pub clip: Arc<BoundClip>,
    /// `…_AnimationTime`: a fraction of the clip (no wrap). The sample time is duration ×
    /// fraction limited to `0 ..= duration` ([`deformer_time`]), so +∞ gives the end and −∞
    /// gives 0; NaN reads 0 (the Tessera Decision of `appearance.md` §6.4 step 1).
    pub fraction: f32,
    /// `…_UseBoneWeights`: the clip's weight array gives the track weights.
    pub use_bone_weights: bool,
}

/// The sample time of a deformer clip of `duration` seconds at `fraction`
/// (`appearance.md` §6.4 step 1, `script-table.md` §6.9): duration × fraction, limited to
/// [0, duration] after the multiplication, so +∞ gives the end of the clip and −∞ gives 0.
/// A NaN fraction reads 0 (Tessera Decision); a duration that is not finite or not positive
/// gives 0 (hostile input).
#[must_use]
pub fn deformer_time(fraction: f32, duration: f32) -> f32 {
    if !(duration.is_finite() && duration > 0.0) || fraction.is_nan() {
        return 0.0;
    }
    (fraction * duration).clamp(0.0, duration)
}

/// The bind-space deformation of one figure: a transform per bone (`appearance.md` §6.4
/// steps 1–3).
#[derive(Debug, Clone, PartialEq)]
pub struct BindDeformation {
    skeleton_key: u64,
    local: Vec<Transform>,
    palette: Vec<Mat4>,
}

impl BindDeformation {
    /// Most deformer clips combined: a Tessera bound, not a client rule (`appearance.md`
    /// §6.4: the client keeps any number; the install has one per figure). Clips beyond it
    /// are dropped and logged.
    pub const MAX_CLIPS: usize = 16;

    /// Builds the deformation (`appearance.md` §6.4):
    ///
    /// 1. **Deformation local pose.** With deformer clips: each is sampled at
    ///    [`deformer_time`], its track weights are ⌊255 × clip weight⌋ / 255 (truncation,
    ///    no tolerance) when `use_bone_weights` is set and the clip has a weight array, else
    ///    1; the clips are combined per bone as a normalised weighted average, the reference
    ///    pose filling a total of at most 0.1 (`skeleton-anim.md` §6.6 steps 4–5). A clip
    ///    whose track count differs from the bone count contributes nothing and is logged
    ///    as an error; clips beyond [`BindDeformation::MAX_CLIPS`] are dropped and logged.
    ///    With no contributing clip: the reference pose.
    /// 2. **Sliders**: every entry of `sliders` is applied to that pose.
    /// 3. **Transforms**: model triples composed by `skeleton-anim.md` §5.4 and each bone's
    ///    skinning transform *D_b* against the ordinary reference pose, by the inverse
    ///    reference triple and skinning rule of §5.4 (K26, [`crate::skinning_matrices`]).
    ///
    /// # Errors
    ///
    /// [`AnimError::Mismatch`] for sliders built for another rig.
    pub fn new(
        skeleton: &Skeleton,
        clips: &[DeformClip],
        sliders: Option<&BoneSliders>,
    ) -> Result<Self> {
        let n = skeleton.bone_count();
        if clips.len() > Self::MAX_CLIPS {
            tracing::warn!(
                dropped = clips.len() - Self::MAX_CLIPS,
                kept = Self::MAX_CLIPS,
                "deformer clips beyond the Tessera bound dropped"
            );
        }
        let clips = clips.get(..Self::MAX_CLIPS).unwrap_or(clips);
        for c in clips.iter().filter(|c| c.clip.track_count() != n) {
            tracing::error!(
                tracks = c.clip.track_count(),
                bones = n,
                "deformer clip does not fit the skeleton; it contributes nothing"
            );
        }
        if let Some(s) = sliders
            && !s.fits(skeleton)
        {
            return Err(AnimError::Mismatch {
                what: "bone sliders",
            });
        }
        let reference = skeleton.reference_local();
        let mut local = skeleton.reference_pose();
        let mut fitting = clips
            .iter()
            .filter(|c| c.clip.track_count() == n)
            .peekable();
        if fitting.peek().is_some() {
            let mut acc = vec![Accum::default(); n];
            let mut sample = skeleton.reference_pose();
            for c in fitting {
                let t = deformer_time(c.fraction, c.clip.duration());
                c.clip.sample(t, &mut sample)?;
                let weighted = c.use_bone_weights && c.clip.track_weights().is_some();
                for (j, (a, s)) in acc.iter_mut().zip(&sample).enumerate() {
                    let w = if weighted {
                        quantise_weight(c.clip.track_weight(j))
                    } else {
                        1.0
                    };
                    a.add(s, w);
                }
            }
            for ((l, a), r) in local.iter_mut().zip(&mut acc).zip(reference) {
                a.fill(r);
                *l = a.result(r);
            }
        }
        if let Some(s) = sliders {
            s.apply(&mut local)?;
        }
        let mut model = vec![Transform::IDENTITY; n];
        skeleton.compose_triples(&local, &mut model)?;
        let mut palette = vec![Mat4::IDENTITY; n];
        skinning_matrices(skeleton, &model, &mut palette)?;
        Ok(Self {
            skeleton_key: skeleton.content_key(),
            local,
            palette,
        })
    }

    /// The deformation local pose (after the sliders).
    #[must_use]
    pub fn local_pose(&self) -> &[Transform] {
        &self.local
    }

    /// The per-bone deformation transforms *D_b*, indexed by bone.
    #[must_use]
    pub fn palette(&self) -> &[Mat4] {
        &self.palette
    }

    /// Whether the deformation was built for a skeleton posing like `skeleton`.
    #[must_use]
    pub fn fits(&self, skeleton: &Skeleton) -> bool {
        self.palette.len() == skeleton.bone_count() && self.skeleton_key == skeleton.content_key()
    }

    /// Whether every transform is the identity within `eps` (nothing to deform).
    #[must_use]
    pub fn is_identity(&self, eps: f32) -> bool {
        self.palette
            .iter()
            .all(|m| m.abs_diff_eq(Mat4::IDENTITY, eps))
    }

    /// Deforms skinned vertices in place (`appearance.md` §6.4 step 4): each position *v*
    /// becomes Σ *w_k* · *D*_{j_k} · *v* over the vertex's own influences; normals and the
    /// xyz of tangents use the same blended transform without translation and are
    /// renormalised (Reported by analogy in the spec; the tangent sign is kept). Vertices
    /// beyond the shortest of the given streams, and vertices without a usable influence,
    /// are left as they are.
    pub fn deform(
        &self,
        positions: &mut [[f32; 3]],
        mut normals: Option<&mut [[f32; 3]]>,
        mut tangents: Option<&mut [[f32; 4]]>,
        joints: &[[u8; 4]],
        weights: &[[f32; 4]],
    ) {
        for (i, ((p, j), w)) in positions.iter_mut().zip(joints).zip(weights).enumerate() {
            let m = blended_matrix(&self.palette, j, w);
            if m == Mat4::ZERO || !m.is_finite() {
                continue;
            }
            let v = m * Vec4::new(p[0], p[1], p[2], 1.0);
            if v.is_finite() {
                *p = v.truncate().to_array();
            }
            if let Some(n) = normals.as_deref_mut().and_then(|n| n.get_mut(i)) {
                let d = Vec3::from_array(*n);
                *n = m.transform_vector3(d).normalize_or(d).to_array();
            }
            if let Some(t) = tangents.as_deref_mut().and_then(|t| t.get_mut(i)) {
                let d = Vec3::new(t[0], t[1], t[2]);
                let r = m.transform_vector3(d).normalize_or(d);
                *t = [r.x, r.y, r.z, t[3]];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slider::SliderChannels;

    fn leg() -> Skeleton {
        let t = |z: f32| Transform {
            translation: [0.0, 0.0, z],
            ..Transform::IDENTITY
        };
        Skeleton::from_parts(&[-1, 0, 1], &[t(1.0), t(-0.45), t(-0.41)]).unwrap()
    }

    /// `appearance.md` §12.5: bind-space deformation of a leg by two fixed sliders.
    #[test]
    fn worked_example_12_5() {
        let s = leg();
        let mut sl = BoneSliders::new(&s);
        sl.add_slider(
            SliderChannels::SCALE_Z,
            [1.0, 1.0, 0.8],
            [1.0, 1.0, 0.8],
            0.5,
            &[(0, false)],
        )
        .unwrap();
        sl.add_slider(
            SliderChannels::SCALE_Z,
            [1.0, 1.0, 1.15],
            [1.0, 1.0, 1.15],
            0.5,
            &[(1, false)],
        )
        .unwrap();
        let d = BindDeformation::new(&s, &[], Some(&sl)).unwrap();
        assert!(d.fits(&s));
        assert!(!d.is_identity(1e-6));
        let mut pos = [[0.0, 0.0, 0.775], [0.0, 0.0, 0.345], [0.0, 0.0, 0.0]];
        let mut nrm = [[1.0, 0.0, 0.0]; 3];
        let joints = [[0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0]];
        let weights = [[1.0, 0.0, 0.0, 0.0]; 3];
        d.deform(&mut pos, Some(&mut nrm), None, &joints, &weights);
        assert!((pos[0][2] - 0.82).abs() < 1e-5, "{:?}", pos[0]);
        assert!((pos[1][2] - 0.3614).abs() < 1e-5, "{:?}", pos[1]);
        assert!((pos[2][2] - 0.0112).abs() < 1e-5, "{:?}", pos[2]);
        assert!((Vec3::from_array(nrm[2]).length() - 1.0).abs() < 1e-6);

        // Animating the deformed mesh with the reference pose leaves it unchanged.
        let mut skin = vec![Mat4::IDENTITY; 3];
        skinning_matrices(&s, s.reference_triples(), &mut skin).unwrap();
        let p = crate::skin::skin_position(&skin, &[2], &[1.0], Vec3::from_array(pos[2]));
        assert!((p.z - 0.0112).abs() < 1e-5);
    }

    #[test]
    fn no_inputs_is_the_identity() {
        let s = leg();
        let d = BindDeformation::new(&s, &[], None).unwrap();
        assert!(d.is_identity(1e-5));
        let other = Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap();
        let sl = BoneSliders::new(&other);
        assert!(BindDeformation::new(&s, &[], Some(&sl)).is_err());
        let mut pos = [[0.0, 0.0, 0.5]];
        d.deform(
            &mut pos,
            None,
            None,
            &[[9, 0, 0, 0]],
            &[[1.0, 0.0, 0.0, 0.0]],
        );
        assert_eq!(pos, [[0.0, 0.0, 0.5]]);
    }
}
