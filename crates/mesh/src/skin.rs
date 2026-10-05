// SPDX-License-Identifier: GPL-3.0-or-later
//! Skinning data in an engine-neutral form (`mesh.md` §5.7, §6.2; `skeleton-anim.md`
//! §4.1–§4.3).
//!
//! - Vertex bone indices address the **skeleton's bone array directly**: there is no
//!   per-submesh palette and no remap table (`skeleton-anim.md` §4.1). The palette of a
//!   skinned draw is the skeleton's skinning-matrix array indexed by bone (§4.3).
//! - The per-submesh influence list is redundant (`mesh.md` §6.2, settled by
//!   `skeleton-anim.md` §4.2): for each vertex from last to first, its slot-0 bone once per
//!   non-zero weight. [`Skin::influence_list`] recomputes it so that a record can be
//!   validated ([`crate::IssueKind::InfluenceListContent`]); rendering ignores it.
//! - [`Submesh::skin_streams`] reduces a submesh to four influences per vertex, and
//!   [`Mesh::skinned_level`] flattens a level together with the bones each part uses,
//!   checked against the skeleton's bone count (`setup.md` SI10).
//!
//! # Ownership of the skinning helpers
//!
//! This crate owns the **data-side** facts of `skeleton-anim.md` §4.1–§4.2: the
//! influence-list recomputation ([`Skin::influence_list`]), the bones a skin uses
//! ([`Skin::bones_used`], [`Skin::max_joint`]) and the joint-range check against a
//! skeleton ([`Mesh::skinned_level`]). The **GPU palette** of §4.3 (the 256-entry palette
//! capacity and the optional compact per-draw palette with its matrix upload) belongs to
//! `tessera-anim` (`PALETTE_CAPACITY`, `CompactPalette`); feed it the vertex streams of one
//! part with [`SkinnedLevel::part_streams`].

use crate::error::{MeshError, Result};
use crate::geometry::{GeometryLimits, LevelGeometry};
use crate::record::{Mesh, Skin, Submesh};

impl Skin {
    /// Number of whole vertices the joint and weight arrays hold.
    fn vertices(&self) -> usize {
        let k = usize::from(self.influences);
        if k == 0 {
            return 0;
        }
        self.joints.len().min(self.weights.len()) / k
    }

    /// The influence list this skin implies (`mesh.md` §6.2): for every vertex from the
    /// last to the first, its first bone index (slot 0) once per non-zero weight of that
    /// vertex. Its length is the number of non-zero weights (MI6). Allocates at most one
    /// `u32` per stored weight.
    #[must_use]
    pub fn influence_list(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.weights.iter().filter(|w| **w != 0.0).count());
        for v in (0..self.vertices()).rev() {
            let first = self.joints_of(v).first().copied().unwrap_or(0);
            let nonzero = self.weights_of(v).iter().filter(|w| **w != 0.0).count();
            out.extend(std::iter::repeat_n(u32::from(first), nonzero));
        }
        out
    }

    /// The largest bone index used by a non-zero weight, or `None` if no weight is
    /// non-zero. Compare it with the skeleton's bone count (`setup.md` SI10).
    #[must_use]
    pub fn max_joint(&self) -> Option<u8> {
        self.joints
            .iter()
            .zip(&self.weights)
            .filter(|(_, w)| **w != 0.0)
            .map(|(j, _)| *j)
            .max()
    }

    /// The distinct bone indices used by non-zero weights, ascending.
    #[must_use]
    pub fn bones_used(&self) -> Vec<u8> {
        let mut seen = [false; 256];
        for (j, w) in self.joints.iter().zip(&self.weights) {
            if *w != 0.0 {
                seen[usize::from(*j)] = true;
            }
        }
        (0..=255u8).filter(|&b| seen[usize::from(b)]).collect()
    }

    /// Every vertex as at most four influences ([`Skin::four`]).
    #[must_use]
    pub fn to_four(&self) -> SkinStreams {
        let n = self.vertices();
        let mut joints = Vec::with_capacity(n);
        let mut weights = Vec::with_capacity(n);
        for v in 0..n {
            let (j, w) = self.four(v);
            joints.push(j);
            weights.push(w);
        }
        SkinStreams { joints, weights }
    }
}

/// Up to four bone influences per vertex, padded with index 0 / weight 0 (`mesh.md` §5.7):
/// the form GPU skinning paths take. Indices address the skeleton's bone array directly.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkinStreams {
    /// Bone indices per vertex.
    pub joints: Vec<[u8; 4]>,
    /// The matching weights.
    pub weights: Vec<[f32; 4]>,
}

impl Submesh {
    /// The submesh's influences as four per vertex, or `None` if it is not skinned.
    #[must_use]
    pub fn skin_streams(&self) -> Option<SkinStreams> {
        self.streams.skin.as_ref().map(Skin::to_four)
    }

    /// The influence list the vertex data implies (`mesh.md` §6.2); empty for an
    /// unskinned submesh.
    #[must_use]
    pub fn computed_influence_list(&self) -> Vec<u32> {
        self.streams
            .skin
            .as_ref()
            .map(Skin::influence_list)
            .unwrap_or_default()
    }

    /// Whether the stored influence list equals [`Submesh::computed_influence_list`]
    /// (true for all 43,059 skinned submeshes of the observed install).
    #[must_use]
    pub fn influence_list_matches(&self) -> bool {
        match &self.streams.skin {
            Some(skin) => {
                // Compare without allocating when the lengths already differ.
                let n = skin.weights.iter().filter(|w| **w != 0.0).count();
                n == self.influence_list.len() && skin.influence_list() == self.influence_list
            }
            None => self.influence_list.is_empty(),
        }
    }
}

/// The four-influence vertex streams of one part of a [`SkinnedLevel`]
/// ([`SkinnedLevel::part_streams`]); equal lengths, one entry per vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartStreams<'a> {
    /// Bone indices per vertex (skeleton bones directly).
    pub joints: &'a [[u8; 4]],
    /// The matching weights.
    pub weights: &'a [[f32; 4]],
}

/// The skinning facts of one part (draw) of a [`SkinnedLevel`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartSkin {
    /// Whether the part's submesh has bone influences.
    pub skinned: bool,
    /// The largest bone index its non-zero weights use.
    pub max_joint: Option<u8>,
    /// The distinct bones its non-zero weights use, ascending.
    pub bones_used: Vec<u8>,
}

/// A flattened level with its skinning facts: what a renderer needs to build a skinned
/// mesh (positions, normals, tangents, UVs, four joints and weights per vertex, indices
/// and parts) plus the bones each part uses.
///
/// Joints address the skeleton's bone array directly, so the draw's palette is the
/// skeleton's skinning-matrix array (`skeleton-anim.md` §4.3; `tessera-anim` holds the
/// palette types). Unskinned parts carry joint 0 / weight 0 and are rigid
/// (`skeleton-anim.md` §9.1); no observed setup mixes them with skinned ones.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinnedLevel {
    /// The flattened streams; `joints` and `weights` are `Some` when any part is skinned.
    pub geometry: LevelGeometry,
    /// One entry per [`LevelGeometry::parts`] element, in the same order.
    pub parts: Vec<PartSkin>,
    /// The largest bone index of the whole level.
    pub max_joint: Option<u8>,
}

impl SkinnedLevel {
    /// Whether any part is skinned.
    #[must_use]
    pub fn is_skinned(&self) -> bool {
        self.parts.iter().any(|p| p.skinned)
    }

    /// Palette entries the level needs with the identity remap: `max_joint + 1`, or 0.
    #[must_use]
    pub fn palette_len(&self) -> usize {
        self.max_joint.map_or(0, |m| usize::from(m) + 1)
    }

    /// The four-influence joints and weights of part `part`'s vertices (the input of a
    /// per-draw compact palette, `tessera-anim`'s `CompactPalette::from_vertices`). `None`
    /// for an unskinned or missing part, or when the part's vertex range lies outside the
    /// streams (a hand-edited value; nothing overflows).
    #[must_use]
    pub fn part_streams(&self, part: usize) -> Option<PartStreams<'_>> {
        let p = self.geometry.parts.get(part)?;
        if !p.skinned {
            return None;
        }
        let start = p.base_vertex as usize;
        let end = start.checked_add(p.vertex_count as usize)?;
        let joints = self.geometry.joints.as_ref()?.get(start..end)?;
        let weights = self.geometry.weights.as_ref()?.get(start..end)?;
        Some(PartStreams { joints, weights })
    }
}

impl Mesh {
    /// Flattens `level` ([`Mesh::level_geometry`]) and adds the bones each part uses.
    ///
    /// With `bone_count` (the skeleton's bone count), every bone index used by a non-zero
    /// weight must be below it (`setup.md` SI10, `skeleton-anim.md` §4.1).
    ///
    /// # Errors
    ///
    /// Everything [`Mesh::level_geometry`] returns, and [`MeshError::JointOutOfRange`] when
    /// a part uses a bone the skeleton does not have.
    pub fn skinned_level(
        &self,
        level: usize,
        limits: &GeometryLimits,
        bone_count: Option<usize>,
    ) -> Result<SkinnedLevel> {
        let geometry = self.level_geometry(level, limits)?;
        // Per vertex block once, however often the level draws it.
        let mut per_block: Vec<Option<PartSkin>> = vec![None; self.submeshes.len()];
        let mut parts = Vec::with_capacity(geometry.parts.len());
        let mut max_joint: Option<u8> = None;
        for p in &geometry.parts {
            let block = p.submesh as usize;
            let info = match per_block.get_mut(block) {
                Some(Some(info)) => info.clone(),
                Some(slot @ None) => {
                    let skin = self.submeshes[block].streams.skin.as_ref();
                    let info = PartSkin {
                        skinned: skin.is_some(),
                        max_joint: skin.and_then(Skin::max_joint),
                        bones_used: skin.map(Skin::bones_used).unwrap_or_default(),
                    };
                    *slot = Some(info.clone());
                    info
                }
                None => PartSkin::default(),
            };
            if let (Some(count), Some(j)) = (bone_count, info.max_joint)
                && usize::from(j) >= count
            {
                return Err(MeshError::JointOutOfRange {
                    submesh: p.submesh,
                    joint: j,
                    bone_count: count,
                });
            }
            max_joint = max_joint.max(info.max_joint);
            parts.push(info);
        }
        Ok(SkinnedLevel {
            geometry,
            parts,
            max_joint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin(k: u8, joints: &[u8], weights: &[f32]) -> Skin {
        Skin {
            influences: k,
            joints: joints.to_vec(),
            weights: weights.to_vec(),
        }
    }

    #[test]
    fn influence_list_follows_the_spec_example() {
        // mesh.md §6.2 / skeleton-anim.md §14.3.
        let s = skin(2, &[4, 7, 2, 0, 9, 4], &[0.75, 0.25, 1.0, 0.0, 0.5, 0.5]);
        assert_eq!(s.influence_list(), vec![9, 9, 2, 4, 4]);
        assert_eq!(s.max_joint(), Some(9));
        assert_eq!(s.bones_used(), vec![2, 4, 7, 9]);
        let four = s.to_four();
        assert_eq!(four.joints[0], [4, 7, 0, 0]);
        assert_eq!(four.weights[2], [0.5, 0.5, 0.0, 0.0]);
        // A ragged hand-built skin uses whole vertices only.
        let ragged = skin(2, &[1, 2, 3], &[1.0, 0.0, 1.0, 0.0]);
        assert_eq!(ragged.influence_list(), vec![1]);
        assert_eq!(skin(0, &[], &[]).influence_list(), Vec::<u32>::new());
    }
}
