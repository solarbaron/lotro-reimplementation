// SPDX-License-Identifier: GPL-3.0-or-later
//! One detail level flattened into shared vertex and index streams for a renderer
//! (`mesh.md` §13, last paragraph).

use tessera_formats::Did;

use crate::bounds::Bounds;
use crate::error::{MeshError, Result};
use crate::record::Mesh;

/// Size limits applied before a level is flattened.
///
/// A draw list may reference the same vertex block many times, and every stream that any
/// one part has is filled for every vertex of the level, so a small hostile record could
/// otherwise ask for a very large flattened level. All limits are checked before anything
/// is allocated. Renderers must still check the result against their own upload limits
/// (for example a GPU's largest buffer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryLimits {
    /// Largest number of vertices in the flattened level.
    pub max_vertices: u64,
    /// Largest number of indices in the flattened level.
    pub max_indices: u64,
    /// Largest number of draw entries (parts) in the level.
    pub max_draws: u64,
    /// Largest size in bytes of the flattened vertex and index streams
    /// ([`LevelGeometry::stream_bytes`]).
    pub max_bytes: u64,
    /// Largest ratio between the flattened stream bytes and the bytes of vertex and
    /// index data the mesh stores for the level (each distinct vertex block and index
    /// buffer counted once). Drawing every block once, as observed data does, stays
    /// below about 16 whatever the formats; only repeated draws go further.
    pub max_amplification: u64,
}

impl Default for GeometryLimits {
    /// 1 Mi vertices, 4 Mi indices, 1,024 draws, 128 MiB of streams and an amplification
    /// of 64. The largest level of the observed install has 99,478 vertices, 287,712
    /// indices and 28 draws, and draws every block once (`mesh.md` §8.2, §18). The
    /// vertex and index counts stay within the renderer's default mesh limits, and the
    /// byte bound within the 256 MiB a GPU buffer may have by default.
    fn default() -> Self {
        Self {
            max_vertices: 1 << 20,
            max_indices: 1 << 22,
            max_draws: 1 << 10,
            max_bytes: 128 << 20,
            max_amplification: 64,
        }
    }
}

/// The index stream of a [`LevelGeometry`]: 16-bit while every index fits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indices {
    /// 16-bit indices (the level has at most 65,536 vertices).
    U16(Vec<u16>),
    /// 32-bit indices.
    U32(Vec<u32>),
}

impl Indices {
    /// Number of indices.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::U16(v) => v.len(),
            Self::U32(v) => v.len(),
        }
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Index `i` widened to `u32`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<u32> {
        match self {
            Self::U16(v) => v.get(i).map(|&x| u32::from(x)),
            Self::U32(v) => v.get(i).copied(),
        }
    }

    /// Every index widened to `u32`.
    #[must_use]
    pub fn to_u32(&self) -> Vec<u32> {
        match self {
            Self::U16(v) => v.iter().map(|&x| u32::from(x)).collect(),
            Self::U32(v) => v.clone(),
        }
    }
}

/// One draw entry of a flattened level: a contiguous index range drawn with one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    /// Position in the level's draw list (draw order, §8.2).
    pub draw: u32,
    /// Submesh whose vertices were copied.
    pub submesh: u32,
    /// Index buffer whose indices were copied.
    pub index_buffer: u32,
    /// The surface record (0x31) to bind, if the draw's surface index is valid.
    pub surface: Option<Did>,
    /// First index of the part in [`LevelGeometry::indices`] (a multiple of 3).
    pub first_index: u32,
    /// Number of indices.
    pub index_count: u32,
    /// First vertex of the part. Indices are already offset by it.
    pub base_vertex: u32,
    /// Number of vertices.
    pub vertex_count: u32,
    /// Whether the submesh has bone influences.
    pub skinned: bool,
}

/// A level as engine-neutral streams with one entry per vertex.
///
/// Streams that only some parts have are filled for the others: generated area-weighted
/// face normals, zero UVs, white colours, zero tangents with sign +1, and joint 0 /
/// weight 0 skin slots (use [`Part::skinned`] to tell them apart).
#[derive(Debug, Clone, PartialEq)]
pub struct LevelGeometry {
    /// The level number.
    pub level: u32,
    /// Positions (object space, Z-up, metres).
    pub positions: Vec<[f32; 3]>,
    /// Normals.
    pub normals: Vec<[f32; 3]>,
    /// Tangents `[x, y, z, w]`, `w` = ±1 the bitangent sign: the stored binormal points
    /// along `w · (normal × tangent)`, with the normal of [`LevelGeometry::normals`]
    /// (generated when the format has none; §5.6: handedness varies per vertex). `None`
    /// if no part has a tangent frame.
    pub tangents: Option<Vec<[f32; 4]>>,
    /// UV sets; set 0 is the diffuse coordinate. Empty if no part has UVs.
    pub uv_sets: Vec<Vec<[f32; 2]>>,
    /// Vertex colours, R G B A bytes, if any part has them.
    pub colors: Option<Vec<[u8; 4]>>,
    /// Up to four bone indices per vertex, if any part is skinned.
    pub joints: Option<Vec<[u8; 4]>>,
    /// The matching weights.
    pub weights: Option<Vec<[f32; 4]>>,
    /// Triangle-list indices into the streams above; counter-clockwise front faces.
    pub indices: Indices,
    /// One part per draw entry, in draw order.
    pub parts: Vec<Part>,
    /// Bounds of the positions.
    pub bounds: Option<Bounds>,
}

/// Bytes per vertex of each optional stream of a [`LevelGeometry`].
const POSITION_BYTES: u64 = 12;
const NORMAL_BYTES: u64 = 12;
const TANGENT_BYTES: u64 = 16;
const UV_BYTES: u64 = 8;
const COLOR_BYTES: u64 = 4;
const SKIN_BYTES: u64 = 4 + 16;

impl LevelGeometry {
    /// Bytes of the vertex and index streams (by length): what an upload of them needs.
    #[must_use]
    pub fn stream_bytes(&self) -> u64 {
        let per_vertex = POSITION_BYTES
            + NORMAL_BYTES
            + self.tangents.as_ref().map_or(0, |_| TANGENT_BYTES)
            + UV_BYTES * self.uv_sets.len() as u64
            + self.colors.as_ref().map_or(0, |_| COLOR_BYTES)
            + self.joints.as_ref().map_or(0, |_| SKIN_BYTES);
        let index = match self.indices {
            Indices::U16(_) => 2,
            Indices::U32(_) => 4,
        };
        per_vertex * self.positions.len() as u64 + index * self.indices.len() as u64
    }

    /// Number of vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Area-weighted face normals for one part (fallback when the format has none).
fn face_normals(positions: &[[f32; 3]], indices: &[u16]) -> Vec<[f32; 3]> {
    let mut n = vec![[0f32; 3]; positions.len()];
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(usize::from);
        let (Some(&pa), Some(&pb), Some(&pc)) =
            (positions.get(a), positions.get(b), positions.get(c))
        else {
            continue;
        };
        let f = cross(sub(pb, pa), sub(pc, pa));
        for i in [a, b, c] {
            for k in 0..3 {
                n[i][k] += f[k];
            }
        }
    }
    for v in &mut n {
        let len = dot(*v, *v).sqrt();
        *v = if len > 0.0 && len.is_finite() {
            v.map(|c| c / len)
        } else {
            [0.0, 0.0, 1.0]
        };
    }
    n
}

/// Bytes of vertex data one vertex of `st` holds (the stride of its format, for a decoded
/// mesh).
fn stored_vertex_bytes(st: &crate::record::VertexStreams) -> u64 {
    let k = st.skin.as_ref().map_or(0, |s| u64::from(s.influences));
    12 + st.normals.as_ref().map_or(0, |_| 12)
        + st.scalars.as_ref().map_or(0, |_| 4)
        + st.colors.as_ref().map_or(0, |_| 4)
        + 8 * st.uv_sets.len() as u64
        + st.tangents.as_ref().map_or(0, |_| 12)
        + st.binormals.as_ref().map_or(0, |_| 12)
        + 5 * k
}

impl Mesh {
    /// Flattens `level` into one vertex/index stream set (`mesh.md` §13): each draw entry,
    /// in order, appends its vertex block and its indices offset by the running vertex
    /// count, and becomes a [`Part`] bound to its surface.
    ///
    /// Every limit of `limits` is checked, and every stream length verified
    /// ([`crate::Submesh::check_streams`]), before the output is allocated. Work done per
    /// vertex block (skin reduction to four influences) is done once per block, however
    /// often the level draws it.
    ///
    /// # Errors
    ///
    /// [`MeshError::NoSuchLevel`] when the level does not exist;
    /// [`MeshError::LimitExceeded`] when the level is larger than `limits`;
    /// [`MeshError::DrawOutOfRange`] when a draw names a missing block or an index buffer
    /// that does not fit its block, and [`MeshError::StreamLength`] when a drawn submesh's
    /// streams disagree with its vertex count (both impossible for a decoded mesh).
    pub fn level_geometry(&self, level: usize, limits: &GeometryLimits) -> Result<LevelGeometry> {
        let lv = self.levels.get(level).ok_or(MeshError::NoSuchLevel {
            level,
            count: self.levels.len(),
        })?;
        let draws = lv.draws.len() as u64;
        if draws > limits.max_draws {
            return Err(MeshError::LimitExceeded {
                what: "draws in one level",
                value: draws,
                limit: limits.max_draws,
            });
        }
        // Resolve every draw, check each distinct block once, and sum the totals before
        // allocating any output or scanning indices.
        let mut resolved = Vec::with_capacity(lv.draws.len());
        let mut block_seen = vec![false; self.submeshes.len()];
        let mut buffer_seen = vec![false; self.index_buffers.len()];
        let (mut nv, mut ni, mut stored) = (0u64, 0u64, 0u64);
        for (di, d) in lv.draws.iter().enumerate() {
            let oob = |field, value, count: usize| MeshError::DrawOutOfRange {
                level: level as u32,
                draw: di as u32,
                field,
                value,
                count: count as u32,
            };
            let sm = self
                .submeshes
                .get(d.vertex_block as usize)
                .ok_or_else(|| oob("vertex_block", d.vertex_block, self.submeshes.len()))?;
            let ib = self
                .index_buffers
                .get(d.index_buffer as usize)
                .ok_or_else(|| oob("index_buffer", d.index_buffer, self.index_buffers.len()))?;
            let vc = sm.streams.positions.len() as u64;
            if !std::mem::replace(&mut block_seen[d.vertex_block as usize], true) {
                sm.check_streams(d.vertex_block)?;
                stored += vc * stored_vertex_bytes(&sm.streams);
            }
            if !std::mem::replace(&mut buffer_seen[d.index_buffer as usize], true) {
                stored += 2 * ib.indices.len() as u64;
            }
            nv += vc;
            ni += ib.indices.len() as u64;
            resolved.push((di, d, sm, ib));
        }
        drop((block_seen, buffer_seen));
        let any_tangents = resolved.iter().any(|r| r.2.streams.tangents.is_some());
        let uv_sets = resolved
            .iter()
            .map(|r| r.2.streams.uv_sets.len())
            .max()
            .unwrap_or(0);
        let any_colors = resolved.iter().any(|r| r.2.streams.colors.is_some());
        let any_skin = resolved.iter().any(|r| r.2.streams.skin.is_some());
        let wide = nv > 65_536;
        let per_vertex = POSITION_BYTES
            + NORMAL_BYTES
            + if any_tangents { TANGENT_BYTES } else { 0 }
            + UV_BYTES * uv_sets as u64
            + if any_colors { COLOR_BYTES } else { 0 }
            + if any_skin { SKIN_BYTES } else { 0 };
        let bytes = per_vertex
            .saturating_mul(nv)
            .saturating_add(ni.saturating_mul(if wide { 4 } else { 2 }));
        for (what, value, limit) in [
            ("flattened vertices", nv, limits.max_vertices),
            ("flattened indices", ni, limits.max_indices),
            ("flattened stream bytes", bytes, limits.max_bytes),
            (
                "flattened stream bytes for the stored bytes (amplification)",
                bytes,
                stored.saturating_mul(limits.max_amplification),
            ),
        ] {
            if value > limit {
                return Err(MeshError::LimitExceeded { what, value, limit });
            }
        }
        // Largest index per buffer, computed once per buffer however often it is drawn.
        let mut buffer_max: Vec<Option<Option<u16>>> = vec![None; self.index_buffers.len()];
        for &(di, d, sm, ib) in &resolved {
            let max = *buffer_max[d.index_buffer as usize]
                .get_or_insert_with(|| ib.indices.iter().copied().max());
            let vc = sm.streams.positions.len();
            if let Some(m) = max
                && usize::from(m) >= vc
            {
                return Err(MeshError::DrawOutOfRange {
                    level: level as u32,
                    draw: di as u32,
                    field: "index",
                    value: u32::from(m),
                    count: vc as u32,
                });
            }
        }
        drop(buffer_max);
        let nv = nv as usize;
        let ni = ni as usize;

        let mut g = LevelGeometry {
            level: level as u32,
            positions: Vec::with_capacity(nv),
            normals: Vec::with_capacity(nv),
            tangents: any_tangents.then(|| Vec::with_capacity(nv)),
            uv_sets: (0..uv_sets).map(|_| Vec::with_capacity(nv)).collect(),
            colors: any_colors.then(|| Vec::with_capacity(nv)),
            joints: any_skin.then(|| Vec::with_capacity(nv)),
            weights: any_skin.then(|| Vec::with_capacity(nv)),
            indices: if wide {
                Indices::U32(Vec::with_capacity(ni))
            } else {
                Indices::U16(Vec::with_capacity(ni))
            },
            parts: Vec::with_capacity(resolved.len()),
            bounds: None,
        };
        // Where each skinned block's reduced influences were first written, so that a
        // block drawn again is copied instead of reduced again.
        let mut skin_at: Vec<Option<usize>> = vec![None; self.submeshes.len()];

        for (di, d, sm, ib) in resolved {
            let st = &sm.streams;
            let base = g.positions.len();
            let first = g.indices.len() as u32;
            let vc = st.positions.len();
            g.positions.extend_from_slice(&st.positions);
            match &st.normals {
                Some(n) => g.normals.extend_from_slice(n),
                None => g.normals.extend(face_normals(&st.positions, &ib.indices)),
            }
            if let Some(out) = g.tangents.as_mut() {
                match (&st.tangents, &st.binormals) {
                    (Some(t), Some(b)) => {
                        // The sign is taken against the normals just written, which are
                        // the generated ones when the format has none.
                        let normals = &g.normals[base..base + vc];
                        for ((t, b), &n) in t.iter().zip(b).zip(normals) {
                            let w = if dot(cross(n, *t), *b) < 0.0 {
                                -1.0
                            } else {
                                1.0
                            };
                            out.push([t[0], t[1], t[2], w]);
                        }
                    }
                    _ => out.extend(std::iter::repeat_n([0.0, 0.0, 0.0, 1.0], vc)),
                }
            }
            for (i, set) in g.uv_sets.iter_mut().enumerate() {
                match st.uv_sets.get(i) {
                    Some(s) => set.extend_from_slice(s),
                    None => set.extend(std::iter::repeat_n([0.0, 0.0], vc)),
                }
            }
            if let Some(out) = g.colors.as_mut() {
                match &st.colors {
                    Some(c) => out.extend_from_slice(c),
                    None => out.extend(std::iter::repeat_n([255; 4], vc)),
                }
            }
            if let (Some(jo), Some(wo)) = (g.joints.as_mut(), g.weights.as_mut()) {
                match (&st.skin, &mut skin_at[d.vertex_block as usize]) {
                    (Some(_), Some(at)) => {
                        let at = *at;
                        jo.extend_from_within(at..at + vc);
                        wo.extend_from_within(at..at + vc);
                    }
                    (Some(skin), slot @ None) => {
                        *slot = Some(base);
                        for v in 0..vc {
                            let (j, w) = skin.four(v);
                            jo.push(j);
                            wo.push(w);
                        }
                    }
                    (None, _) => {
                        jo.extend(std::iter::repeat_n([0; 4], vc));
                        wo.extend(std::iter::repeat_n([0.0; 4], vc));
                    }
                }
            }
            match &mut g.indices {
                Indices::U16(out) => out.extend(ib.indices.iter().map(|&x| x + base as u16)),
                Indices::U32(out) => {
                    out.extend(ib.indices.iter().map(|&x| u32::from(x) + base as u32));
                }
            }
            g.parts.push(Part {
                draw: di as u32,
                submesh: d.vertex_block,
                index_buffer: d.index_buffer,
                surface: self.surface_of(d),
                first_index: first,
                index_count: ib.indices.len() as u32,
                base_vertex: base as u32,
                vertex_count: vc as u32,
                skinned: st.skin.is_some(),
            });
        }
        g.bounds = self.level_bounds(level);
        Ok(g)
    }
}
