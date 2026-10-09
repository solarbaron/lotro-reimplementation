// SPDX-License-Identifier: GPL-3.0-or-later
//! A cheap summary of a mesh record: region A, the size of every submesh and index buffer,
//! without decoding a single vertex (`mesh.md` §3, §7, §13).
//!
//! Assembly (`appearance.md` §4.4) needs two facts about every part mesh it offers to a
//! slot: the mesh's slot (its `flags` word, a `MeshTypeID` value, `setup.md` §12.2) and
//! whether it is a **stub** (three vertices, one triangle: the slot is hidden). Decoding the
//! whole record for that would copy every vertex of a 20,000-vertex hair mesh; the summary
//! skips the vertex bytes, reading only counts.

use tessera_formats::Did;

use crate::error::{MeshError, Result};
use crate::format::VertexFormat;
use crate::read::Cursor;
use crate::record::{MAX_SUBMESH_VERTICES, MeshHeader};

/// Vertex count of the stub meshes of the reference install (`appearance.md` §4.4: three
/// vertices, one triangle; the smallest real part has four vertices). The stub test itself
/// counts triangles ([`MeshSummary::is_stub`]).
pub const STUB_VERTEX_COUNT: u64 = 3;

/// Size of one submesh block and its index buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmeshSummary {
    /// The vertex format word (§4).
    pub format: VertexFormat,
    /// Number of vertices.
    pub vertex_count: u32,
    /// Number of indices of the index buffer with the same number (§7).
    pub index_count: u32,
}

/// Region A plus the size of every submesh and index buffer of a mesh record.
///
/// Built by [`MeshSummary::parse`], which reads the record up to the end of region C
/// (`mesh.md` §13 steps 1–3) and checks every count against the bytes left, but does not
/// read region D (levels and collision data) and does not check the indices themselves.
/// A record that [`crate::Mesh::decode`] accepts always summarises to the same counts; a
/// record whose summary parses may still fail full decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSummary {
    /// The record's own DID (MI1).
    pub did: Did,
    /// The `flags` word: the part's `MeshTypeID` slot (`setup.md` §12.2).
    pub flags: u32,
    /// Surface records, one per submesh (§12).
    pub surfaces: Vec<Did>,
    /// One entry per submesh, in storage order.
    pub submeshes: Vec<SubmeshSummary>,
}

impl MeshSummary {
    /// Reads regions A to C of `bytes`, skipping the vertex data and the influence lists.
    ///
    /// # Errors
    ///
    /// [`MeshError::Truncated`] when a count runs past the end of the record;
    /// [`MeshError::CountMismatch`] for MI2 failures; [`MeshError::UnsupportedVertexFormat`];
    /// [`MeshError::LimitExceeded`] for more than [`crate::MAX_SUBMESHES`] submeshes or
    /// [`MAX_SUBMESH_VERTICES`] vertices in one submesh; [`MeshError::PartialTriangle`]
    /// for an index count that is not a multiple of 3.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(bytes);
        let header = MeshHeader::read(&mut c)?;
        let n = header.submesh_count;
        let mut submeshes = Vec::with_capacity(n as usize);
        for index in 0..n {
            let vfmt = c.u32("vertex format")?;
            let format =
                VertexFormat::new(vfmt).map_err(|e| MeshError::UnsupportedVertexFormat {
                    submesh: index,
                    vfmt,
                    reason: e.reason,
                })?;
            let v = c.u32("vertex count")?;
            if v > MAX_SUBMESH_VERTICES {
                return Err(MeshError::LimitExceeded {
                    what: "vertices in one submesh",
                    value: u64::from(v),
                    limit: u64::from(MAX_SUBMESH_VERTICES),
                });
            }
            let vbytes = u64::from(v) * format.stride() as u64;
            c.take("vertex block", vbytes)?;
            c.take("AABB", 24)?;
            let influences = c.count("influence list", 4)?;
            c.take("influence list", u64::from(influences) * 4)?;
            submeshes.push(SubmeshSummary {
                format,
                vertex_count: v,
                index_count: 0,
            });
        }
        let count = c.u32("index buffer count")?;
        if count != n {
            return Err(MeshError::CountMismatch {
                what: "index buffer count vs submesh count",
                expected: u64::from(n),
                found: u64::from(count),
            });
        }
        for (buffer, sm) in submeshes.iter_mut().enumerate() {
            let k = c.count("index buffer", 2)?;
            if k % 3 != 0 {
                return Err(MeshError::PartialTriangle {
                    buffer: buffer as u32,
                    count: k,
                });
            }
            c.take("index buffer", u64::from(k) * 2)?;
            sm.index_count = k;
        }
        Ok(Self {
            did: header.did,
            flags: header.flags,
            surfaces: header.surfaces,
            submeshes,
        })
    }

    /// Vertices over all submeshes (all levels).
    #[must_use]
    pub fn vertex_count(&self) -> u64 {
        self.submeshes
            .iter()
            .map(|s| u64::from(s.vertex_count))
            .sum()
    }

    /// Triangles over all index buffers (all levels).
    #[must_use]
    pub fn triangle_count(&self) -> u64 {
        self.submeshes
            .iter()
            .map(|s| u64::from(s.index_count / 3))
            .sum()
    }

    /// The part's slot: the `flags` word, a `MeshTypeID` value (`setup.md` §12.2).
    #[must_use]
    pub fn slot(&self) -> u32 {
        self.flags
    }

    /// Whether this is a stub mesh (`appearance.md` §4.4, §15; `mesh.md` §8.10): the client
    /// flags a mesh whose detail levels hold exactly **one triangle in total** and never
    /// draws it as one of several parts. The summary does not read the levels (region D),
    /// so it counts the triangles of the index buffers (the client's count whenever the
    /// detail levels draw each index buffer once); on the reference install the client's
    /// rule flags the same 258 records as the three-vertex test ([`STUB_VERTEX_COUNT`]), and a record with one triangle in its
    /// first level but more in others (`mesh.md` §8.10) is not a stub. A stub that wins a
    /// slot leaves it visually empty (the slot is hidden).
    #[must_use]
    pub fn is_stub(&self) -> bool {
        self.triangle_count() == 1
    }

    /// Whether any submesh carries bone influences (`mesh.md` §3.3).
    #[must_use]
    pub fn is_skinned(&self) -> bool {
        self.submeshes.iter().any(|s| s.format.is_skinned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::MeshSpec;

    #[test]
    fn summary_matches_full_decode() {
        for (format, vertices) in [(0x3002_0252, 3), (0x0000_0112, 40), (0x3004_0152, 9)] {
            let spec = MeshSpec::single(format, vertices);
            let mesh = spec.build().unwrap();
            let bytes = mesh.to_bytes().unwrap();
            let s = MeshSummary::parse(&bytes).unwrap();
            assert_eq!(s.did, mesh.did);
            assert_eq!(s.flags, mesh.flags);
            assert_eq!(s.surfaces, mesh.surfaces);
            assert_eq!(s.vertex_count(), u64::from(vertices));
            assert_eq!(s.triangle_count(), mesh.triangle_count(0) as u64);
            assert_eq!(s.is_stub(), vertices == 3);
            assert_eq!(s.is_skinned(), mesh.has_skinned_submesh());
            // Cutting the record anywhere before the end of region C fails cleanly.
            for cut in [0, 4, 11, bytes.len() / 3] {
                assert!(MeshSummary::parse(&bytes[..cut]).is_err());
            }
        }
    }
}
