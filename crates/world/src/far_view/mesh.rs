// SPDX-License-Identifier: GPL-3.0-or-later
//! Far-terrain and far-ceiling meshes from sector samples (`distant-landscape.md` §5.3,
//! `far-view.md` §5).
//!
//! A batch holds the present landblocks of one sector that are outside the near field (the
//! **far batch**, the only one the world view draws, FD8) or inside it (the **near
//! batch**, tools only). Each landblock is drawn at its LOD *l* with quads of 2^*l* samples,
//! the diagonal from south-west to north-east in every quad: terrain triangles (NW, SW, NE)
//! and (NE, SW, SE), counter-clockwise from above; ceiling triangles reversed. Positions are
//! sector-local (*x* east, *y* north, *z* absolute height): *x* = `block_size` × *x*ₗ +
//! *i* × `block_size` / 32; heights converted as `landblock.md` §3.4; the stored normals
//! decoded, not renormalised (§5.3 of `far-view.md`); texture coordinates (*u*, *v*) =
//! (*x* / 480, 1 − *y* / 480) for the standard 160-unit landblock.
//!
//! **Stitching** (`distant-landscape.md` §5.3, `far-view.md` §5.2): on an edge whose
//! descriptor stitches, every segment of 2^*l* samples gets the triangles that close the
//! crack against the neighbour's edge. With a neighbour one LOD finer (always the case
//! with *N* = 1, FV8) that is one triangle joining the segment's two ends with the edge
//! sample at its middle, as the client does. Against a neighbour more than one LOD finer
//! (possible only when the walker keeps a larger full-detail ring, FD9) Tessera adds a fan
//! from the segment's first end over every neighbour edge sample inside the segment
//! (Tessera Decision FD11). Stitching triangles are vertical slivers; they are stored after
//! the grid triangles ([`FarMesh::stitch_start`]) so the renderer can draw them without
//! face culling (Tessera Decision FD10; the client culls back faces, FV23).

use tessera_formats::landblock::{SectorMesh, SectorSample, block_sample_offset, chunk_index};
use tessera_formats::{Did, FormatError};

use super::grid::{LandblockDescriptor, SECTOR_SIDE};
use crate::error::{Result, WorldError};

/// Samples per landblock side.
pub const GRID: usize = 33;
/// Quads per landblock side at LOD 0.
pub const QUADS: usize = 32;

/// Largest accepted `block_size` (the data holds 160.0 in every record, DL11).
pub const MAX_BLOCK_SIZE: f32 = 10_000.0;

/// Which mesh of the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeshKind {
    /// The terrain mesh (counter-clockwise from above).
    Terrain,
    /// The ceiling mesh (clockwise from above, facing down); only landblocks whose mask
    /// bit is set.
    Ceiling,
}

/// Which batch of a sector mesh (`far-view.md` §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Batch {
    /// Present landblocks outside the near field.
    Far,
    /// Present landblocks inside the near field (never drawn in the world view, FD8).
    Near,
}

/// A built batch (module docs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FarMesh {
    /// Sector-local positions.
    pub positions: Vec<[f32; 3]>,
    /// Decoded stored normals (world x, y, z; not renormalised).
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates (*u*, *v*).
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list: the grid triangles, then the stitching triangles.
    pub indices: Vec<u32>,
    /// First index of the stitching triangles.
    pub stitch_start: usize,
    /// Landblocks in the batch.
    pub landblocks: u32,
    /// Lowest and highest *z* of the vertices (0, 0 when empty).
    pub z_range: [f32; 2],
}

impl FarMesh {
    /// Whether the batch has no triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Grid triangles.
    #[must_use]
    pub fn grid_triangles(&self) -> usize {
        self.stitch_start / 3
    }

    /// Stitching triangles.
    #[must_use]
    pub fn stitch_triangles(&self) -> usize {
        self.indices.len().saturating_sub(self.stitch_start) / 3
    }

    /// Bytes held.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.positions.capacity() * 12
            + self.normals.capacity() * 12
            + self.uvs.capacity() * 8
            + self.indices.capacity() * 4
    }
}

fn bad(did: Did, reason: &'static str) -> WorldError {
    WorldError::Format {
        did: did.0,
        what: "distant-landscape sector",
        source: FormatError::Malformed {
            what: "sector mesh",
            offset: 0,
            reason: reason.into(),
        },
    }
}

/// One landblock's vertex table: sample (i, j) → vertex index, emitted on first use.
struct Block<'a> {
    samples: &'a [SectorSample],
    map: [u32; GRID * GRID],
    origin: [f32; 2],
    spacing: f32,
    uv_origin: [f32; 2],
}

struct Builder {
    out: FarMesh,
    kind: MeshKind,
}

impl Builder {
    fn vertex(&mut self, b: &mut Block<'_>, i: usize, j: usize) -> Option<u32> {
        let slot = GRID * j + i;
        let cur = *b.map.get(slot)?;
        if cur != u32::MAX {
            return Some(cur);
        }
        let s = *b.samples.get(block_sample_offset(i, j)?)?;
        let idx = u32::try_from(self.out.positions.len()).ok()?;
        #[allow(clippy::cast_precision_loss)]
        let (fi, fj) = (i as f32, j as f32);
        let z = s.height_units();
        self.out.positions.push([
            b.origin[0] + fi * b.spacing,
            b.origin[1] + fj * b.spacing,
            z,
        ]);
        self.out.normals.push(s.normal_xyz());
        self.out.uvs.push([
            (b.uv_origin[0] + fi / 32.0) / 3.0,
            1.0 - (b.uv_origin[1] + fj / 32.0) / 3.0,
        ]);
        if self.out.positions.len() == 1 {
            self.out.z_range = [z, z];
        } else {
            self.out.z_range = [self.out.z_range[0].min(z), self.out.z_range[1].max(z)];
        }
        if let Some(m) = b.map.get_mut(slot) {
            *m = idx;
        }
        Some(idx)
    }

    /// A triangle given counter-clockwise from above (terrain winding); reversed for the
    /// ceiling.
    fn tri(&mut self, a: u32, b: u32, c: u32) {
        match self.kind {
            MeshKind::Terrain => self.out.indices.extend_from_slice(&[a, b, c]),
            MeshKind::Ceiling => self.out.indices.extend_from_slice(&[c, b, a]),
        }
    }

    fn grid(&mut self, b: &mut Block<'_>, step: usize) -> Option<()> {
        for j in (0..QUADS).step_by(step) {
            for i in (0..QUADS).step_by(step) {
                let sw = self.vertex(b, i, j)?;
                let se = self.vertex(b, i + step, j)?;
                let nw = self.vertex(b, i, j + step)?;
                let ne = self.vertex(b, i + step, j + step)?;
                self.tri(nw, sw, ne);
                self.tri(ne, sw, se);
            }
        }
        Some(())
    }

    /// Stitching triangles on edge `e` (0 west, 1 south, 2 east, 3 north) from own step
    /// `step` to neighbour step `t`.
    fn stitch(&mut self, b: &mut Block<'_>, e: usize, step: usize, t: usize) -> Option<()> {
        if t == 0 || t >= step || !step.is_multiple_of(t) {
            return Some(());
        }
        let at = |k: usize| match e {
            0 => (0, k),
            1 => (k, 0),
            2 => (QUADS, k),
            _ => (k, QUADS),
        };
        for k in (0..QUADS).step_by(step) {
            let (ai, aj) = at(k);
            let first = self.vertex(b, ai, aj)?;
            let mut prev = None;
            for m in (t..=step).step_by(t) {
                let (pi, pj) = at(k + m);
                let p = self.vertex(b, pi, pj)?;
                if let Some(q) = prev {
                    self.tri(first, q, p);
                }
                prev = Some(p);
            }
        }
        Some(())
    }
}

/// Builds batch `batch` of mesh `kind` of a sector record from its mesh block `mesh`,
/// `block_size` and the nine descriptors (chunk order, [`super::LandblockGrid::descriptors`]).
/// Landblocks are visited X outer, Y inner (§5.3). Stitching follows each descriptor's
/// edge flags.
///
/// # Errors
///
/// [`WorldError::Format`] (for `did`) when `block_size` is not a positive finite value up
/// to [`MAX_BLOCK_SIZE`], or a selected landblock's chunk has no 1,089-sample block (a
/// decoded record never has the latter).
pub fn build_batch(
    did: Did,
    mesh: &SectorMesh,
    block_size: f32,
    descriptors: &[LandblockDescriptor; 9],
    kind: MeshKind,
    batch: Batch,
) -> Result<FarMesh> {
    if !(block_size.is_finite() && block_size > 0.0 && block_size <= MAX_BLOCK_SIZE) {
        return Err(bad(did, "block size is not a positive finite value"));
    }
    let mut builder = Builder {
        out: FarMesh::default(),
        kind,
    };
    let mut blocks: Vec<(Block<'_>, &LandblockDescriptor)> = Vec::new();
    for x in 0..usize::from(SECTOR_SIDE) {
        for y in 0..usize::from(SECTOR_SIDE) {
            let Some(d) = descriptors.get(chunk_index(x, y)) else {
                continue;
            };
            let wanted = d.present
                && (d.far == (batch == Batch::Far))
                && (kind == MeshKind::Terrain || d.ceiling);
            if !wanted {
                continue;
            }
            let Some(samples) = mesh.block(x, y) else {
                return Err(bad(did, "a selected landblock has no sample block"));
            };
            if samples.len() != GRID * GRID {
                return Err(bad(did, "a sample block does not hold 1,089 samples"));
            }
            #[allow(clippy::cast_precision_loss)]
            let (fx, fy) = (x as f32, y as f32);
            blocks.push((
                Block {
                    samples,
                    map: [u32::MAX; GRID * GRID],
                    origin: [fx * block_size, fy * block_size],
                    spacing: block_size / 32.0,
                    uv_origin: [fx, fy],
                },
                d,
            ));
        }
    }
    for (b, d) in &mut blocks {
        builder
            .grid(b, d.step())
            .ok_or_else(|| bad(did, "sample outside the block"))?;
    }
    builder.out.stitch_start = builder.out.indices.len();
    for (b, d) in &mut blocks {
        for (e, edge) in d.edges.iter().enumerate() {
            if edge.stitch {
                let t = 1usize << edge.neighbour_lod.min(3);
                builder
                    .stitch(b, e, d.step(), t)
                    .ok_or_else(|| bad(did, "sample outside the block"))?;
            }
        }
    }
    builder.out.landblocks = u32::try_from(blocks.len()).unwrap_or(u32::MAX);
    Ok(builder.out)
}

/// The index list the client generates when a mesh block stores none
/// (`distant-landscape.md` §5.3 step 1–3): for chunk positions *r* = 0…8 (base vertex
/// 1,089·*r*) and LOD *l* = 0…3, quads of 2^*l* samples, row by row; 73,440 indices. The
/// stored `lods` ranges (§4.5) point into it.
#[must_use]
pub fn generated_index_list(kind: MeshKind) -> Vec<u32> {
    let mut out = Vec::with_capacity(9 * 8_160);
    for r in 0..9u32 {
        let base = 1_089 * r;
        for l in 0..4 {
            let s = 1usize << l;
            for j in (0..QUADS).step_by(s) {
                for i in (0..QUADS).step_by(s) {
                    #[allow(clippy::cast_possible_truncation)]
                    let at = |c: usize, row: usize| base + (GRID * row + c) as u32;
                    let (sw, se, nw, ne) = (at(i, j), at(i + s, j), at(i, j + s), at(i + s, j + s));
                    match kind {
                        MeshKind::Terrain => out.extend_from_slice(&[nw, sw, ne, ne, sw, se]),
                        MeshKind::Ceiling => out.extend_from_slice(&[ne, sw, nw, se, sw, ne]),
                    }
                }
            }
        }
    }
    out
}

/// Stitching triangles one edge adds for a landblock of LOD `lod` joined to a neighbour of
/// LOD `neighbour` (FV-I6: 32 / 2^*l* for a neighbour one finer).
#[must_use]
pub fn stitch_triangle_count(lod: u8, neighbour: u8) -> usize {
    let s = 1usize << lod.min(3);
    let t = 1usize << neighbour.min(3);
    if t >= s {
        return 0;
    }
    (QUADS / s) * (s / t - 1)
}
