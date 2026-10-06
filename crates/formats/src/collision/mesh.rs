// SPDX-License-Identifier: GPL-3.0-or-later
//! Stand-alone collision meshes, prefix 0x66 (`collision.md` §4), and the physics mesh of a
//! cell-geometry record, which has the same fields (§5.3).

use crate::error::{FormatError, Result};
use crate::landblock::{LayoutRule, tree_build_byte_rule};
use crate::reader::{Reader, write_vle};

use super::tree::{TREE_HEADER_LEN, TreeBlock, TreeCode};

/// Type prefix of stand-alone collision meshes ([`crate::DidKind::CollisionMesh`], DB type
/// PHYSICS_MESH; `records.md` §2.1). The record has **no** DID prologue.
pub const COLLISION_MESH_PREFIX: u8 = 0x66;

/// Stored size of one vertex (3 × f32).
const VERTEX_LEN: usize = 12;
/// Stored size of one triangle (3 × u16).
const TRIANGLE_LEN: usize = 6;

/// What follows a collision mesh's tree code, for the stamp-less `build_byte` fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follower {
    /// The mesh ends the record (0x66): a `build_byte` is present exactly when one byte
    /// remains.
    EndOfRecord,
    /// A cell-volume plane list follows (0x01 physics mesh, `collision.md` §5.1): a
    /// `build_byte` is present unless the next four bytes are a plane tag.
    PlaneList,
}

/// A triangle mesh with a quantised tree: a 0x66 record (`collision.md` §4.1) or the
/// physics mesh of a 0x01 record (§5.3). Coordinates are in the host frame (setup frame for
/// 0x66, cell frame for 0x01), Z up, metres.
///
/// Triangles are counter-clockwise from outside in most closed meshes, but 51 closed meshes
/// are inside out and many are open (§4.2): collision tests must treat them as two-sided.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollisionMesh {
    /// Vertex positions.
    pub vertices: Vec<[f32; 3]>,
    /// Vertex indices, three per triangle; every index is below the vertex count (CO1).
    pub triangles: Vec<[u16; 3]>,
    /// The quantised tree over the triangles (§3); keys are triangle indices (§3.5).
    pub tree: TreeBlock,
    /// How the tree's `build_byte` was decided.
    pub layout_rule: LayoutRule,
}

impl CollisionMesh {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "collision mesh";

    /// Decodes a whole 0x66 payload (`collision.md` §4.1), consumed to the last byte.
    ///
    /// `stamp` is the record's directory stamp: the tree's `build_byte` is present iff it is
    /// ≥ 38 (§3.1). Without a stamp, or with an unobserved one (34–37), a `build_byte` is
    /// taken exactly when one byte is left after the code (a Tessera fallback).
    ///
    /// # Errors
    ///
    /// [`FormatError::Truncated`] or [`FormatError::TrailingBytes`] when the layout does not
    /// fit the payload, [`FormatError::IndexOutOfRange`] for a triangle index not below the
    /// vertex count, [`FormatError::Unsupported`] for `has_code` = 0 (never observed; the
    /// layout without code is unknown), [`FormatError::InvalidBool`] for `has_code` > 1,
    /// [`FormatError::MemoryLimit`] beyond the reader's budget.
    pub fn parse(data: &[u8], stamp: Option<u16>) -> Result<Self> {
        let mut r = Reader::new(data);
        let mesh = Self::read(&mut r, stamp, Follower::EndOfRecord)?;
        r.expect_end(Self::NAME)?;
        Ok(mesh)
    }

    /// Reads the fields of §4.1 at the reader's position.
    pub(crate) fn read(r: &mut Reader<'_>, stamp: Option<u16>, follower: Follower) -> Result<Self> {
        let count = r.vle()?;
        let mut vertices = r.alloc_vec(count, VERTEX_LEN, usize::MAX)?;
        for _ in 0..count {
            let v = [r.f32()?, r.f32()?, r.f32()?];
            r.push(&mut vertices, v, count)?;
        }
        let tri_count = r.vle()?;
        let mut triangles = r.alloc_vec(tri_count, TRIANGLE_LEN, usize::MAX)?;
        for _ in 0..tri_count {
            let at = r.position();
            let t = [r.u16()?, r.u16()?, r.u16()?];
            if let Some(&bad) = t.iter().find(|&&i| u32::from(i) >= count) {
                return Err(FormatError::IndexOutOfRange {
                    what: "collision mesh vertex index",
                    offset: at,
                    index: u64::from(bad),
                    limit: u64::from(count),
                });
            }
            r.push(&mut triangles, t, tri_count)?;
        }
        let at = r.position();
        match r.u8()? {
            1 => {}
            0 => {
                return Err(FormatError::Unsupported {
                    what: "collision mesh has_code",
                    offset: at,
                    value: 0,
                });
            }
            value => return Err(FormatError::InvalidBool { offset: at, value }),
        }
        let mut tree = TreeBlock::read_core(r)?;
        let (rule, layout_rule) = tree_build_byte_rule(stamp);
        let present = match rule {
            Some(p) => p,
            None => match follower {
                Follower::EndOfRecord => r.remaining() == 1,
                Follower::PlaneList => r
                    .peek(4)
                    .ok()
                    .and_then(|b| b.first_chunk::<4>())
                    .is_none_or(|b| !super::cell::is_plane_tag(u32::from_le_bytes(*b))),
            },
        };
        if present {
            tree.build_byte = Some(r.u8()?);
        }
        Ok(Self {
            vertices,
            triangles,
            tree,
            layout_rule,
        })
    }

    /// The program view of the tree.
    #[must_use]
    pub fn tree_code(&self) -> TreeCode<'_> {
        self.tree.code()
    }

    /// The three corner positions of triangle `t`, or `None` when `t` is out of range.
    #[must_use]
    pub fn triangle_positions(&self, t: usize) -> Option<[[f32; 3]; 3]> {
        let tri = self.triangles.get(t)?;
        let mut out = [[0.0; 3]; 3];
        for (dst, &i) in out.iter_mut().zip(tri) {
            *dst = *self.vertices.get(usize::from(i))?;
        }
        Some(out)
    }

    /// Axis-aligned bounds of the vertices as (minimum, maximum), or `None` without
    /// vertices. For 0x66 meshes this equals the setup's `physics_bounds` (§4.3).
    #[must_use]
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let first = *self.vertices.first()?;
        Some(self.vertices.iter().fold((first, first), |(lo, hi), v| {
            (
                [lo[0].min(v[0]), lo[1].min(v[1]), lo[2].min(v[2])],
                [hi[0].max(v[0]), hi[1].max(v[1]), hi[2].max(v[2])],
            )
        }))
    }

    /// Stored size in bytes with shortest VLE counts.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        vle_len(self.vertices.len())
            + VERTEX_LEN * self.vertices.len()
            + vle_len(self.triangles.len())
            + TRIANGLE_LEN * self.triangles.len()
            + 1
            + TREE_HEADER_LEN
            + self.tree.code.len()
            + usize::from(self.tree.build_byte.is_some())
    }

    /// Appends the stored form (§4.1) with shortest VLE counts.
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        write_vle(out, u32::try_from(self.vertices.len()).unwrap_or(u32::MAX));
        for v in &self.vertices {
            for c in v {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        write_vle(out, u32::try_from(self.triangles.len()).unwrap_or(u32::MAX));
        for t in &self.triangles {
            for i in t {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        out.push(1);
        self.tree.write(out);
    }

    /// The stored 0x66 form with shortest VLE counts. A decoded record re-encodes to its
    /// input when the input used shortest VLEs.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.encoded_len());
        self.write(&mut out);
        out
    }
}

/// Bytes of the shortest VLE encoding of `n` (`records.md` §1.2).
fn vle_len(n: usize) -> usize {
    match n {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        // The four-byte form cannot start with 0xE0, the five-byte escape.
        0x4000..=0x3FFF_FFFF if n >> 24 != 0x20 => 4,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CollisionMesh {
        CollisionMesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.5]],
            triangles: vec![[0, 1, 2]],
            tree: TreeBlock {
                origin: [-0.05; 3],
                scale: 100.0,
                code: vec![0x30],
                build_byte: None,
            },
            layout_rule: LayoutRule::Stamp,
        }
    }

    #[test]
    fn round_trip_with_and_without_build_byte() {
        let mut m = sample();
        let bytes = m.to_bytes();
        assert_eq!(bytes.len(), m.encoded_len());
        assert_eq!(CollisionMesh::parse(&bytes, Some(33)).unwrap(), m);
        // Without a stamp: nothing left after the code, so no build byte.
        let back = CollisionMesh::parse(&bytes, None).unwrap();
        assert_eq!(back.tree.build_byte, None);
        assert_eq!(back.layout_rule, LayoutRule::Fallback);
        // A stamp >= 38 needs the byte.
        assert!(matches!(
            CollisionMesh::parse(&bytes, Some(38)),
            Err(FormatError::Truncated { .. })
        ));
        m.tree.build_byte = Some(1);
        let bytes = m.to_bytes();
        assert_eq!(CollisionMesh::parse(&bytes, Some(39)).unwrap(), m);
        assert_eq!(
            CollisionMesh::parse(&bytes, None).unwrap().tree.build_byte,
            Some(1)
        );
        assert_eq!(
            CollisionMesh::parse(&bytes, Some(36)).unwrap().layout_rule,
            LayoutRule::UnobservedStamp
        );
        assert!(matches!(
            CollisionMesh::parse(&bytes, Some(20)),
            Err(FormatError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn rejects_bad_index_and_has_code() {
        let m = sample();
        let mut bytes = m.to_bytes();
        // Triangle indices start after the VLE count (1), 36 vertex bytes and the VLE (1).
        bytes[38] = 3;
        assert!(matches!(
            CollisionMesh::parse(&bytes, Some(7)),
            Err(FormatError::IndexOutOfRange { index: 3, .. })
        ));
        let mut bytes = m.to_bytes();
        bytes[44] = 0;
        assert!(matches!(
            CollisionMesh::parse(&bytes, Some(7)),
            Err(FormatError::Unsupported { .. })
        ));
        bytes[44] = 2;
        assert!(matches!(
            CollisionMesh::parse(&bytes, Some(7)),
            Err(FormatError::InvalidBool { value: 2, .. })
        ));
    }

    #[test]
    fn hostile_counts_do_not_allocate() {
        // VLE 0xE0 escape: a count of 2^32 - 1 vertices in a tiny payload.
        let bytes = [0xE0, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0];
        assert!(matches!(
            CollisionMesh::parse(&bytes, None),
            Err(FormatError::Truncated { .. })
        ));
    }

    #[test]
    fn helpers() {
        let m = sample();
        assert_eq!(m.triangle_positions(0).unwrap()[2], [0.0, 1.0, 0.5]);
        assert!(m.triangle_positions(1).is_none());
        assert_eq!(m.bounds(), Some(([0.0; 3], [1.0, 1.0, 0.5])));
        assert_eq!(CollisionMesh::default().bounds(), None);
        assert_eq!(m.tree_code().point_keys([0.2, 0.2, 0.1]).unwrap(), vec![0]);
        for n in [
            0usize,
            0x7F,
            0x80,
            0x3FFF,
            0x4000,
            0x1FFF_FFFF,
            0x2000_0000,
            0x2100_0000,
            0x4000_0000,
        ] {
            let mut v = Vec::new();
            write_vle(&mut v, u32::try_from(n).unwrap());
            assert_eq!(v.len(), vle_len(n), "{n:#x}");
        }
    }
}
