// SPDX-License-Identifier: GPL-3.0-or-later
//! Skinning matrices and palettes (`skeleton-anim.md` §4), plus a CPU reference skinner.
//!
//! Mesh vertex bone indices address the skeleton's bone array directly; the per-submesh
//! influence list of `mesh.md` §6.2 is redundant and is **not** a remap (§4.1–§4.2). So the
//! palette of a skinned draw is the skeleton's skinning-matrix array indexed by bone index
//! (§4.3): [`write_palette_mat4`] or [`write_palette_rows3x4`] into a renderer buffer of
//! [`PALETTE_CAPACITY`] entries. A renderer that wants smaller per-draw palettes can use a
//! [`CompactPalette`] per submesh (sorted distinct bones with a vertex-index remap applied
//! at upload, an implementation choice §4.3 allows). Skinning is linear blend skinning
//! (§4.7).

use glam::{Mat4, Vec3, Vec4};

use crate::error::{AnimError, Result};
use crate::skeleton::{Skeleton, check_len};

/// Palette entries per skeleton instance: covers every observed skeleton and bone index
/// (`skeleton-anim.md` §4.3: at most 248 bones, largest index 247).
pub const PALETTE_CAPACITY: usize = 256;

/// Skinning matrices **P**_b · **B**_b⁻¹ for a model pose (§4.4): `out[b]` for every bone.
///
/// Tessera Decision (`skeleton-anim.md` implementer notes, broken sample values): a product
/// that is not finite (a hand-built model pose, or one whose scale overflows against the
/// inverse bind) is written as the identity, so the bone's vertices stay in the bind pose
/// and no infinity or NaN reaches a GPU palette.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`] when `model` or `out` is shorter than the bone count.
pub fn skinning_matrices(skeleton: &Skeleton, model: &[Mat4], out: &mut [Mat4]) -> Result<()> {
    let n = skeleton.bone_count();
    check_len("model pose", n, model.len())?;
    check_len("skinning matrices", n, out.len())?;
    for ((o, p), inv) in out.iter_mut().zip(model).zip(skeleton.inverse_bind()) {
        let m = *p * *inv;
        *o = if m.is_finite() { m } else { Mat4::IDENTITY };
    }
    Ok(())
}

/// Writes skinning matrices as column-major 4 × 4 f32 arrays (the layout of a WGSL
/// `mat4x4<f32>` array), entry *b* for bone *b*. Entries past `skin.len()` are set to the
/// identity, so a stray vertex index addresses a harmless matrix.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`] when `out` holds fewer entries than `skin`.
pub fn write_palette_mat4(skin: &[Mat4], out: &mut [[f32; 16]]) -> Result<()> {
    check_len("palette", skin.len(), out.len())?;
    for (o, m) in out.iter_mut().zip(skin) {
        *o = m.to_cols_array();
    }
    for o in out.iter_mut().skip(skin.len()) {
        *o = Mat4::IDENTITY.to_cols_array();
    }
    Ok(())
}

/// Writes skinning matrices as three rows of four floats each (12 floats, the affine part;
/// a shader computes `dot(row_i, vec4(p, 1))`). Entries past `skin.len()` are identity.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`] when `out` holds fewer entries than `skin`.
pub fn write_palette_rows3x4(skin: &[Mat4], out: &mut [[f32; 12]]) -> Result<()> {
    check_len("palette", skin.len(), out.len())?;
    let rows = |m: &Mat4| {
        let r0 = m.row(0).to_array();
        let r1 = m.row(1).to_array();
        let r2 = m.row(2).to_array();
        let mut a = [0.0; 12];
        a[..4].copy_from_slice(&r0);
        a[4..8].copy_from_slice(&r1);
        a[8..].copy_from_slice(&r2);
        a
    };
    for (o, m) in out.iter_mut().zip(skin) {
        *o = rows(m);
    }
    let id = rows(&Mat4::IDENTITY);
    for o in out.iter_mut().skip(skin.len()) {
        *o = id;
    }
    Ok(())
}

/// The blended skinning matrix of one vertex: Σ *w_k* · `skin[j_k]` over influences with a
/// non-zero, finite weight and an index inside `skin` (others are skipped). Zero matrix
/// when nothing contributes.
#[must_use]
pub fn blended_matrix(skin: &[Mat4], joints: &[u8], weights: &[f32]) -> Mat4 {
    let mut m = Mat4::ZERO;
    for (&j, &w) in joints.iter().zip(weights) {
        if w == 0.0 || !w.is_finite() {
            continue;
        }
        if let Some(s) = skin.get(usize::from(j)) {
            m += *s * w;
        }
    }
    m
}

/// Linear blend skinning of one position (§4.7). A vertex with no usable influence keeps
/// its position.
#[must_use]
pub fn skin_position(skin: &[Mat4], joints: &[u8], weights: &[f32], p: Vec3) -> Vec3 {
    let m = blended_matrix(skin, joints, weights);
    if m == Mat4::ZERO {
        return p;
    }
    (m * Vec4::new(p.x, p.y, p.z, 1.0)).truncate()
}

/// Linear blend skinning of a direction (normal, tangent, binormal): the blended matrix
/// without translation, renormalised (§4.7). Zero-length results stay as they are.
#[must_use]
pub fn skin_direction(skin: &[Mat4], joints: &[u8], weights: &[f32], d: Vec3) -> Vec3 {
    let m = blended_matrix(skin, joints, weights);
    if m == Mat4::ZERO {
        return d;
    }
    m.transform_vector3(d).normalize_or(d)
}

/// The influence list of `mesh.md` §6.2 recomputed from vertex data (to validate a
/// record): for each vertex from the last to the first, its slot-0 bone index once per
/// non-zero weight. `k` is the influence count per vertex; `joints` and `weights` hold
/// `k` values per vertex (a trailing partial vertex is ignored).
#[must_use]
pub fn influence_list(joints: &[u8], weights: &[f32], k: usize) -> Vec<u32> {
    if k == 0 {
        return Vec::new();
    }
    let verts = (joints.len() / k).min(weights.len() / k);
    let mut out = Vec::new();
    for v in (0..verts).rev() {
        let first = u32::from(joints[v * k]);
        let nz = weights[v * k..(v + 1) * k]
            .iter()
            .filter(|w| **w != 0.0)
            .count();
        out.extend(std::iter::repeat_n(first, nz));
    }
    out
}

/// A per-submesh palette: the sorted distinct bones the submesh uses (by a non-zero
/// weight) and the remap from skeleton bone index to palette slot. Optional (§4.3): the
/// full skeleton palette is always correct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactPalette {
    bones: Vec<u8>,
    remap: [u8; 256],
}

impl CompactPalette {
    /// Builds the palette from per-vertex four-slot joints and weights (pairs beyond the
    /// shorter slice are ignored).
    #[must_use]
    pub fn from_vertices(joints: &[[u8; 4]], weights: &[[f32; 4]]) -> Self {
        let mut used = [false; 256];
        for (j, w) in joints.iter().zip(weights) {
            for k in 0..4 {
                if w[k] != 0.0 {
                    used[usize::from(j[k])] = true;
                }
            }
        }
        let bones: Vec<u8> = (0..=255u8).filter(|&b| used[usize::from(b)]).collect();
        let mut remap = [0u8; 256];
        for (slot, &b) in bones.iter().enumerate() {
            // At most 256 distinct bones, so the slot fits a byte.
            remap[usize::from(b)] = slot as u8;
        }
        Self { bones, remap }
    }

    /// The skeleton bones in palette order.
    #[must_use]
    pub fn bones(&self) -> &[u8] {
        &self.bones
    }

    /// Number of palette entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    /// Whether the submesh uses no bone.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// Palette slot of skeleton bone `bone` (0 for bones the submesh does not use).
    #[must_use]
    pub fn slot(&self, bone: u8) -> u8 {
        self.remap[usize::from(bone)]
    }

    /// Rewrites vertex joints from skeleton indices to palette slots (at upload time).
    pub fn remap_joints(&self, joints: &mut [[u8; 4]]) {
        for j in joints {
            for x in j.iter_mut() {
                *x = self.remap[usize::from(*x)];
            }
        }
    }

    /// Writes this submesh's palette (column-major 4 × 4) from the skeleton's skinning
    /// matrices; entries past [`CompactPalette::len`] are identity.
    ///
    /// # Errors
    ///
    /// [`AnimError::BufferTooSmall`] when `out` is shorter than the palette or `skin` lacks
    /// a bone the palette uses.
    pub fn write_mat4(&self, skin: &[Mat4], out: &mut [[f32; 16]]) -> Result<()> {
        check_len("palette", self.bones.len(), out.len())?;
        if let Some(&max) = self.bones.last() {
            check_len("skinning matrices", usize::from(max) + 1, skin.len())?;
        }
        for (o, &b) in out.iter_mut().zip(&self.bones) {
            *o = skin[usize::from(b)].to_cols_array();
        }
        for o in out.iter_mut().skip(self.bones.len()) {
            *o = Mat4::IDENTITY.to_cols_array();
        }
        Ok(())
    }
}

/// Checks that every bone a submesh uses (by a non-zero weight) exists in a skeleton of
/// `bones` bones (`skeleton-anim.md` KI1); returns the largest used index.
///
/// # Errors
///
/// [`AnimError::BoneIndexOutOfRange`] when an index is out of range.
pub fn check_joints(joints: &[[u8; 4]], weights: &[[f32; 4]], bones: usize) -> Result<Option<u8>> {
    let mut max = None;
    for (j, w) in joints.iter().zip(weights) {
        for k in 0..4 {
            if w[k] != 0.0 {
                max = max.max(Some(j[k]));
            }
        }
    }
    if let Some(m) = max
        && usize::from(m) >= bones
    {
        return Err(AnimError::BoneIndexOutOfRange {
            what: "vertex bone index",
            bone: usize::from(m),
            bones,
        });
    }
    Ok(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_havok::Transform;

    fn example() -> Skeleton {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let t = |tr: [f32; 3], rot: [f32; 4]| Transform {
            translation: tr,
            rotation: rot,
            scale: [1.0; 3],
        };
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

    /// `skeleton-anim.md` §14.2.
    #[test]
    fn skinning_example_14_2() {
        let s = example();
        let mut local = s.reference_pose();
        local[1].rotation = [0.0, 0.0, 0.0, 1.0];
        let mut model = vec![Mat4::IDENTITY; 3];
        s.compose(&local, &mut model).unwrap();
        let mut skin = vec![Mat4::IDENTITY; 3];
        skinning_matrices(&s, &model, &mut skin).unwrap();
        let v = Vec3::new(-1.2, 0.5, 1.0);
        let p = skin_position(&skin, &[2], &[1.0], v);
        assert!(p.abs_diff_eq(Vec3::new(0.0, 1.7, 1.0), 1e-5), "{p}");
        let p = skin_position(&skin, &[1], &[1.0], v);
        assert!(p.abs_diff_eq(Vec3::new(0.0, 1.7, 1.0), 1e-5), "{p}");
        let p = skin_position(&skin, &[0, 2], &[0.5, 0.5], v);
        assert!(p.abs_diff_eq(Vec3::new(-0.6, 1.1, 1.0), 1e-5), "{p}");
        // Reference pose: identity skinning.
        let mut skin_ref = vec![Mat4::IDENTITY; 3];
        skinning_matrices(&s, s.reference_model(), &mut skin_ref).unwrap();
        for m in &skin_ref {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-5));
        }
        let q = skin_position(&skin_ref, &[0, 1, 2, 0], &[0.1, 0.2, 0.3, 0.4], v);
        assert!(q.abs_diff_eq(v, 1e-5));
        let n = skin_direction(&skin, &[2], &[1.0], Vec3::X);
        assert!((n.length() - 1.0).abs() < 1e-5);
    }

    /// `skeleton-anim.md` §14.3 / `mesh.md` §6.2.
    #[test]
    fn influence_list_example_14_3() {
        let joints = [4, 7, 2, 0, 9, 4];
        let weights = [0.75, 0.25, 1.0, 0.0, 0.5, 0.5];
        assert_eq!(influence_list(&joints, &weights, 2), vec![9, 9, 2, 4, 4]);
        assert!(influence_list(&joints, &weights, 0).is_empty());
        assert!(influence_list(&joints[..1], &weights, 2).is_empty());
    }

    #[test]
    fn palettes_pad_with_identity() {
        let skin = vec![Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)); 2];
        let mut out = vec![[0.0; 16]; 4];
        write_palette_mat4(&skin, &mut out).unwrap();
        assert_eq!(out[0][12..15], [1.0, 2.0, 3.0]);
        assert_eq!(out[3], Mat4::IDENTITY.to_cols_array());
        let mut short = vec![[0.0; 16]; 1];
        assert!(write_palette_mat4(&skin, &mut short).is_err());
        let mut rows = vec![[0.0; 12]; 3];
        write_palette_rows3x4(&skin, &mut rows).unwrap();
        assert_eq!(
            rows[0],
            [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0]
        );
        assert_eq!(
            rows[2],
            [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn compact_palette_matches_full() {
        let joints = [[5, 2, 0, 0], [2, 9, 0, 0], [9, 0, 0, 0]];
        let weights = [
            [0.5, 0.5, 0.0, 0.0],
            [0.7, 0.3, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ];
        let cp = CompactPalette::from_vertices(&joints, &weights);
        assert_eq!(cp.bones(), &[2, 5, 9]);
        let skin: Vec<Mat4> = (0..10)
            .map(|i| Mat4::from_translation(Vec3::splat(i as f32)))
            .collect();
        let mut pal = vec![[0.0; 16]; 4];
        cp.write_mat4(&skin, &mut pal).unwrap();
        let pal_m: Vec<Mat4> = pal.iter().map(Mat4::from_cols_array).collect();
        let mut remapped = joints;
        cp.remap_joints(&mut remapped);
        for (v, (j, w)) in joints.iter().zip(&weights).enumerate() {
            let p = Vec3::new(0.1, 0.2, 0.3);
            let full = skin_position(&skin, j, w, p);
            let compact = skin_position(&pal_m, &remapped[v], w, p);
            assert!(full.abs_diff_eq(compact, 1e-6));
        }
        assert!(cp.write_mat4(&skin[..5], &mut pal).is_err());
        assert_eq!(check_joints(&joints, &weights, 10).unwrap(), Some(9));
        // A data invariant (KI1), not a caller buffer problem.
        let e = check_joints(&joints, &weights, 9).unwrap_err();
        assert_eq!(
            e,
            AnimError::BoneIndexOutOfRange {
                what: "vertex bone index",
                bone: 9,
                bones: 9
            }
        );
        assert_eq!(e.category(), "skin.bone_out_of_range");
    }

    #[test]
    fn hostile_skinning_inputs() {
        let skin = vec![Mat4::IDENTITY; 2];
        let p = Vec3::ONE;
        assert_eq!(skin_position(&skin, &[200], &[1.0], p), p);
        assert_eq!(skin_position(&skin, &[0], &[f32::NAN], p), p);
        assert_eq!(skin_position(&skin, &[], &[], p), p);
    }
}
