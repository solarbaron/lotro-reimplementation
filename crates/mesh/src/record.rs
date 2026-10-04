// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh record model and its decoder (`mesh.md` §3–§10, §13).

use tessera_formats::Did;

use crate::bounds::{Aabb, BoundingSphere, Bounds};
use crate::error::{MeshError, Result};
use crate::format::{VertexFormat, VertexLayout};
use crate::read::{Cursor, f32_at, vec2_at, vec3_at};

/// DID prefix (high byte) of mesh records (`mesh.md` §2).
pub const MESH_PREFIX: u8 = 0x06;

/// DID prefix of the surface records a mesh lists (`mesh.md` §12).
pub const SURFACE_PREFIX: u8 = 0x31;

/// Largest vertex count of one submesh: u16 indices cannot address more (`mesh.md` §13).
pub const MAX_SUBMESH_VERTICES: u32 = 65_536;

/// `flags` bit 28: set on meshes meant for skinning. It does **not** decide the vertex
/// layout (`mesh.md` §3.3).
pub const SKINNING_FLAG: u32 = 0x1000_0000;

/// Smallest directory `stamp` with a `build_byte` after the collision tree code (§10.4).
pub const BUILD_BYTE_MIN_STAMP: u16 = 38;

/// Largest directory `stamp` observed without `build_byte` (§10.4). Stamps strictly
/// between this and [`BUILD_BYTE_MIN_STAMP`] were never observed; they use the fallback.
pub const BUILD_BYTE_ABSENT_MAX_STAMP: u16 = 33;

/// The value of a draw entry's `primitive` byte: a triangle list (§8.1).
pub const PRIMITIVE_TRIANGLE_LIST: u8 = 4;

/// Most submeshes in one record. Not a spec limit: a hostile-input bound on the fixed
/// per-submesh overhead of a decoded mesh (observed maximum 78, `mesh.md` §15).
pub const MAX_SUBMESHES: u32 = 4096;

/// Most levels in one record. Not a spec limit: a hostile-input bound on the fixed
/// per-level overhead of a decoded mesh (observed records have a handful, `mesh.md` §8).
pub const MAX_LEVELS: u32 = 4096;

/// Smallest submesh block: format word, vertex count, AABB and influence count (§13).
const MIN_SUBMESH_BYTES: u64 = 36;
/// Smallest level: kind, draw count, one draw entry and the two presence bytes (§13).
const MIN_LEVEL_BYTES: u64 = 23;
/// One draw entry.
const DRAW_BYTES: u64 = 13;

/// How a decoder treats a record (`mesh.md` §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Keep the collision meshes of §9 in [`Level::collision`]. When false they are still
    /// parsed (to find the next level) and counted in [`Level::collision_counts`], but
    /// not stored. Default `true`.
    pub keep_collision: bool,
    /// Keep the opaque collision tree code bytes of §10 in [`TreeCode::code`]. Default
    /// `false`: the length is kept, the bytes are skipped.
    pub keep_tree_code: bool,
    /// Run [`Mesh::validate`] after decoding and fail with [`MeshError::Invalid`] on the
    /// first [`IssueClass::Structural`](crate::IssueClass::Structural) issue (§14: the
    /// decoder and the data disagree). Quality issues (MI4, MI5, MI7, MI10, non-finite
    /// values) never fail decoding, since a viewer may still draw such a mesh; among them
    /// is `CollisionIndexOverflow`, which real data has (§9.2). Callers that want those
    /// too run [`Mesh::validate`] themselves. Default `false`.
    pub strict: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            keep_collision: true,
            keep_tree_code: false,
            strict: false,
        }
    }
}

impl DecodeOptions {
    /// Keeps everything, so that [`Mesh::to_bytes`] reproduces the record exactly.
    #[must_use]
    pub fn lossless() -> Self {
        Self {
            keep_collision: true,
            keep_tree_code: true,
            strict: false,
        }
    }

    /// Only what rendering needs: collision meshes and tree code are skipped.
    #[must_use]
    pub fn render_only() -> Self {
        Self {
            keep_collision: false,
            keep_tree_code: false,
            strict: false,
        }
    }
}

/// Region A of a record, readable without decoding the rest (`mesh.md` §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshHeader {
    /// The record's own DID.
    pub did: Did,
    /// The `flags` word (§3.3). Unknown values are kept, never rejected.
    pub flags: u32,
    /// Surface record DIDs (0x31), one per submesh (§12).
    pub surfaces: Vec<Did>,
    /// Number of submesh blocks; equal to the surface count (MI2).
    pub submesh_count: u32,
}

impl MeshHeader {
    /// Reads region A, and checks that the record is long enough for the *N* submesh
    /// blocks it announces (36 bytes each at least), without decoding them.
    ///
    /// # Errors
    ///
    /// [`MeshError::Truncated`] when region A, or the minimum `N × 36` bytes of submesh
    /// blocks after it, is cut short (so a buffer holding only region A fails);
    /// [`MeshError::CountMismatch`] when the submesh count differs from the surface count;
    /// [`MeshError::LimitExceeded`] for more than [`MAX_SUBMESHES`] surfaces.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::read(&mut Cursor::new(bytes))
    }

    fn read(c: &mut Cursor<'_>) -> Result<Self> {
        let did = Did(c.u32("DID")?);
        let flags = c.u32("flags")?;
        let s = c.count("surface list", 4)?;
        if s > MAX_SUBMESHES {
            return Err(MeshError::LimitExceeded {
                what: "submeshes in one record",
                value: u64::from(s),
                limit: u64::from(MAX_SUBMESHES),
            });
        }
        let mut surfaces = Vec::with_capacity(s as usize);
        for _ in 0..s {
            surfaces.push(Did(c.u32("surface DID")?));
        }
        let n = c.u32("submesh count")?;
        if n != s {
            return Err(MeshError::CountMismatch {
                what: "submesh count vs surface count",
                expected: u64::from(s),
                found: u64::from(n),
            });
        }
        c.need("submesh blocks", u64::from(n) * MIN_SUBMESH_BYTES)?;
        Ok(Self {
            did,
            flags,
            surfaces,
            submesh_count: n,
        })
    }

    /// Whether `flags` bit 28 is set. Informational only (§3.3).
    #[must_use]
    pub fn is_skinning_flagged(&self) -> bool {
        self.flags & SKINNING_FLAG != 0
    }
}

/// Bone indices and weights of one submesh (`mesh.md` §5.7), *K* per vertex.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Skin {
    /// Influences per vertex (*K* ≥ 1).
    pub influences: u8,
    /// `vertex_count × K` bone indices into the skeleton's bone array, vertex-major.
    pub joints: Vec<u8>,
    /// `vertex_count × K` weights, vertex-major. They sum to 1 in observed data.
    pub weights: Vec<f32>,
}

impl Skin {
    /// The slot range of vertex `v`, if it does not overflow.
    fn slots(&self, v: usize) -> Option<std::ops::Range<usize>> {
        let k = usize::from(self.influences);
        let start = v.checked_mul(k)?;
        Some(start..start.checked_add(k)?)
    }

    /// The bone indices of vertex `v` (empty if out of range).
    #[must_use]
    pub fn joints_of(&self, v: usize) -> &[u8] {
        self.slots(v)
            .and_then(|r| self.joints.get(r))
            .unwrap_or(&[])
    }

    /// The weights of vertex `v` (empty if out of range).
    #[must_use]
    pub fn weights_of(&self, v: usize) -> &[f32] {
        self.slots(v)
            .and_then(|r| self.weights.get(r))
            .unwrap_or(&[])
    }

    /// Vertex `v` as at most four influences, padded with index 0 / weight 0 (§5.7).
    ///
    /// With more than four influences (never observed) the four largest weights are kept
    /// (in the total order of `f32`; the earlier slot wins a tie) and renormalised to
    /// their original sum. Allocates nothing; cost is linear in *K*. If the joint and
    /// weight slices of `v` differ in length (only possible on a hand-built skin), only
    /// the slots both have are used.
    #[must_use]
    pub fn four(&self, v: usize) -> ([u8; 4], [f32; 4]) {
        let j = self.joints_of(v);
        let w = self.weights_of(v);
        let n = j.len().min(w.len());
        let (j, w) = (&j[..n], &w[..n]);
        let mut joints = [0u8; 4];
        let mut weights = [0f32; 4];
        if n <= 4 {
            joints[..n].copy_from_slice(j);
            weights[..n].copy_from_slice(w);
            return (joints, weights);
        }
        // Insertion into a fixed top-4, largest first; an equal weight goes after the
        // ones already kept, so earlier slots win ties.
        let mut top = [0usize; 4];
        let mut len = 0usize;
        for (i, &x) in w.iter().enumerate() {
            let pos = top[..len]
                .iter()
                .position(|&t| x.total_cmp(&w[t]).is_gt())
                .unwrap_or(len);
            if pos < 4 {
                for s in (pos..len.min(3)).rev() {
                    top[s + 1] = top[s];
                }
                top[pos] = i;
                len = (len + 1).min(4);
            }
        }
        let total: f32 = w.iter().filter(|x| x.is_finite()).sum();
        let kept: f32 = top.iter().map(|&i| w[i]).filter(|x| x.is_finite()).sum();
        let scale = if kept > 0.0 && total.is_finite() {
            total / kept
        } else {
            1.0
        };
        for (slot, &i) in top.iter().enumerate() {
            joints[slot] = j[i];
            weights[slot] = w[i] * scale;
        }
        (joints, weights)
    }
}

/// The vertex attributes of one submesh as separate streams (structure of arrays).
///
/// Every present stream has exactly `vertex_count` entries (per-vertex arrays) or
/// `vertex_count × K` entries ([`Skin`]). Values are as stored: object space, Z-up,
/// metres, right-handed; UVs unflipped (`mesh.md` §11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VertexStreams {
    /// Positions (§5.1).
    pub positions: Vec<[f32; 3]>,
    /// Normals (§5.2), unit length in observed data.
    pub normals: Option<Vec<[f32; 3]>>,
    /// The per-vertex scalar (§5.3; meaning OPEN).
    pub scalars: Option<Vec<f32>>,
    /// Vertex colours as **R, G, B, A** bytes (converted from the stored B, G, R, A, §5.4).
    pub colors: Option<Vec<[u8; 4]>>,
    /// UV sets (§5.5); set 0 samples the diffuse map.
    pub uv_sets: Vec<Vec<[f32; 2]>>,
    /// Tangents: the +U direction (§5.6). May be zero.
    pub tangents: Option<Vec<[f32; 3]>>,
    /// Binormals: the +V direction (§5.6). May be zero.
    pub binormals: Option<Vec<[f32; 3]>>,
    /// Bone influences (§5.7), when the format has any.
    pub skin: Option<Skin>,
}

impl VertexStreams {
    fn with_capacity(v: usize, l: &VertexLayout) -> Self {
        let k = usize::from(l.influences);
        Self {
            positions: Vec::with_capacity(v),
            normals: l.normal.map(|_| Vec::with_capacity(v)),
            scalars: l.scalar.map(|_| Vec::with_capacity(v)),
            colors: l.color.map(|_| Vec::with_capacity(v)),
            uv_sets: (0..l.uv_sets).map(|_| Vec::with_capacity(v)).collect(),
            tangents: l.tangent.map(|_| Vec::with_capacity(v)),
            binormals: l.binormal.map(|_| Vec::with_capacity(v)),
            skin: (k > 0).then(|| Skin {
                influences: l.influences,
                joints: Vec::with_capacity(v * k),
                weights: Vec::with_capacity(v * k),
            }),
        }
    }

    /// Decodes interleaved vertices (`bytes.len()` is a multiple of `l.stride`).
    pub(crate) fn decode(bytes: &[u8], l: &VertexLayout) -> Self {
        let v = bytes.len() / l.stride.max(1);
        let mut s = Self::with_capacity(v, l);
        let k = usize::from(l.influences);
        for b in bytes.chunks_exact(l.stride) {
            s.positions.push(vec3_at(b, 0));
            if let (Some(o), Some(n)) = (l.normal, s.normals.as_mut()) {
                n.push(vec3_at(b, o));
            }
            if let (Some(o), Some(x)) = (l.scalar, s.scalars.as_mut()) {
                x.push(f32_at(b, o));
            }
            if let (Some(o), Some(c)) = (l.color, s.colors.as_mut()) {
                c.push([b[o + 2], b[o + 1], b[o], b[o + 3]]);
            }
            if let Some(o) = l.uv {
                for (i, set) in s.uv_sets.iter_mut().enumerate() {
                    set.push(vec2_at(b, o + 8 * i));
                }
            }
            if let (Some(o), Some(t)) = (l.tangent, s.tangents.as_mut()) {
                t.push(vec3_at(b, o));
            }
            if let (Some(o), Some(t)) = (l.binormal, s.binormals.as_mut()) {
                t.push(vec3_at(b, o));
            }
            if let (Some(jo), Some(wo), Some(skin)) = (l.joints, l.weights, s.skin.as_mut()) {
                skin.joints.extend_from_slice(&b[jo..jo + k]);
                for i in 0..k {
                    skin.weights.push(f32_at(b, wo + 4 * i));
                }
            }
        }
        s
    }

    /// Heap bytes held (by capacity, including the outer vector of UV sets).
    #[must_use]
    pub fn heap_size(&self) -> usize {
        let v3 = std::mem::size_of::<[f32; 3]>();
        self.uv_sets.capacity() * std::mem::size_of::<Vec<[f32; 2]>>()
            + self.positions.capacity() * v3
            + self.normals.as_ref().map_or(0, |x| x.capacity() * v3)
            + self.scalars.as_ref().map_or(0, |x| x.capacity() * 4)
            + self.colors.as_ref().map_or(0, |x| x.capacity() * 4)
            + self.uv_sets.iter().map(|x| x.capacity() * 8).sum::<usize>()
            + self.tangents.as_ref().map_or(0, |x| x.capacity() * v3)
            + self.binormals.as_ref().map_or(0, |x| x.capacity() * v3)
            + self
                .skin
                .as_ref()
                .map_or(0, |s| s.joints.capacity() + s.weights.capacity() * 4)
    }
}

/// One submesh block (`mesh.md` §3.2): a vertex block and its trailer.
#[derive(Debug, Clone, PartialEq)]
pub struct Submesh {
    /// The vertex format word (§4).
    pub format: VertexFormat,
    /// Number of vertices.
    pub vertex_count: u32,
    /// Decoded vertex attributes.
    pub streams: VertexStreams,
    /// The AABB stored after the vertices (§6.1). Equal to the positions' bounds in
    /// observed data; [`Mesh::validate`] checks it.
    pub stored_aabb: Aabb,
    /// The influence list of §6.2, kept opaque (meaning OPEN).
    pub influence_list: Vec<u32>,
}

impl Submesh {
    /// Whether a renderer treats this submesh as skinned (influence count ≠ 0, §3.3).
    #[must_use]
    pub fn is_skinned(&self) -> bool {
        self.format.is_skinned()
    }

    /// Checks that every present stream holds one entry per vertex: `positions` and the
    /// other per-vertex streams `vertex_count` entries, skin joints and weights
    /// `vertex_count × K`. Always true for a decoded mesh; a hand-built or edited one may
    /// break it. Does not check that the streams match [`Submesh::format`] (the encoder
    /// does).
    ///
    /// # Errors
    ///
    /// [`MeshError::StreamLength`] for the first stream that disagrees; `index` is the
    /// submesh number reported in it.
    pub fn check_streams(&self, index: u32) -> Result<()> {
        let st = &self.streams;
        let n = u64::from(self.vertex_count);
        let opt = |v: Option<usize>| v.map(|x| x as u64);
        let k = st.skin.as_ref().map_or(0, |s| u64::from(s.influences));
        let checks = [
            ("positions", Some(st.positions.len() as u64), n),
            ("normals", opt(st.normals.as_ref().map(Vec::len)), n),
            ("scalars", opt(st.scalars.as_ref().map(Vec::len)), n),
            ("colors", opt(st.colors.as_ref().map(Vec::len)), n),
            ("tangents", opt(st.tangents.as_ref().map(Vec::len)), n),
            ("binormals", opt(st.binormals.as_ref().map(Vec::len)), n),
            (
                "skin joints",
                opt(st.skin.as_ref().map(|s| s.joints.len())),
                n * k,
            ),
            (
                "skin weights",
                opt(st.skin.as_ref().map(|s| s.weights.len())),
                n * k,
            ),
        ];
        let uv = st
            .uv_sets
            .iter()
            .map(|s| ("uv set", Some(s.len() as u64), n));
        for (stream, found, expected) in checks.into_iter().chain(uv) {
            if let Some(found) = found
                && found != expected
            {
                return Err(MeshError::StreamLength {
                    submesh: index,
                    stream,
                    found,
                    expected,
                });
            }
        }
        Ok(())
    }
}

/// One index buffer (`mesh.md` §7): a triangle list of submesh-local u16 indices.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexBuffer {
    /// The indices, three per triangle.
    pub indices: Vec<u16>,
}

impl IndexBuffer {
    /// Number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The triangles.
    pub fn triangles(&self) -> impl Iterator<Item = [u16; 3]> + '_ {
        self.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [t[0], t[1], t[2]])
    }
}

/// The kind of a level (`mesh.md` §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelKind {
    /// 1: a detail level. Level 0 is the full-detail geometry.
    Detail,
    /// 2: a proxy level (simple boxes; use OPEN). Renderers ignore it.
    Proxy,
    /// Any other value (never observed; reported by [`Mesh::validate`]).
    Other(u32),
}

impl LevelKind {
    /// Classifies a raw `kind` value.
    #[must_use]
    pub fn from_raw(v: u32) -> Self {
        match v {
            1 => Self::Detail,
            2 => Self::Proxy,
            o => Self::Other(o),
        }
    }

    /// The raw value.
    #[must_use]
    pub fn raw(self) -> u32 {
        match self {
            Self::Detail => 1,
            Self::Proxy => 2,
            Self::Other(o) => o,
        }
    }
}

/// One draw entry (`mesh.md` §8.1). In observed data the three indices are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Draw {
    /// Primitive type; always [`PRIMITIVE_TRIANGLE_LIST`].
    pub primitive: u8,
    /// Submesh whose vertices are drawn.
    pub vertex_block: u32,
    /// Index buffer drawn.
    pub index_buffer: u32,
    /// Index into [`Mesh::surfaces`].
    pub surface: u32,
}

/// A collision vertex: a reference to a render vertex of the same level (`mesh.md` §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CollisionVertex {
    /// Position in the level's draw list (not a submesh index).
    pub draw: u8,
    /// Vertex of that draw entry's vertex block.
    pub vertex: u16,
}

/// A collision triangle: three collision vertices and the edges between them (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CollisionTriangle {
    /// Collision-vertex indices v0, v1, v2.
    pub vertices: [u16; 3],
    /// Edge indices: `edges[k]` runs from `vertices[k]` to `vertices[(k + 1) % 3]`.
    pub edges: [u16; 3],
}

/// A collision edge: two collision vertices and up to two triangles (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CollisionEdge {
    /// vA, vB.
    pub vertices: [u16; 2],
    /// fA, fB; fB = [`CollisionEdge::NO_FACE`] on an open border.
    pub faces: [u16; 2],
}

impl CollisionEdge {
    /// "No second triangle".
    pub const NO_FACE: u16 = 0xFFFF;
}

/// The collision mesh of a level (`mesh.md` §9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollisionMesh {
    /// Vertex references, strictly ascending in observed data.
    pub vertices: Vec<CollisionVertex>,
    /// Triangles.
    pub triangles: Vec<CollisionTriangle>,
    /// Edges.
    pub edges: Vec<CollisionEdge>,
}

impl CollisionMesh {
    /// Resolves the vertex references of this collision mesh against `level` of `mesh`.
    /// `None` for a reference that does not resolve.
    #[must_use]
    pub fn positions(&self, mesh: &Mesh, level: &Level) -> Vec<Option<[f32; 3]>> {
        self.vertices
            .iter()
            .map(|cv| {
                let draw = level.draws.get(usize::from(cv.draw))?;
                let sm = mesh.submeshes.get(draw.vertex_block as usize)?;
                sm.streams.positions.get(usize::from(cv.vertex)).copied()
            })
            .collect()
    }
}

/// Element counts of a collision mesh, kept even when the mesh itself is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollisionCounts {
    /// Collision vertices.
    pub vertices: u32,
    /// Triangles.
    pub triangles: u32,
    /// Edges.
    pub edges: u32,
}

/// The quantised collision tree code of a level (`mesh.md` §10), not interpreted.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeCode {
    /// Quantisation origin.
    pub origin: [f32; 3],
    /// Quantisation scale.
    pub scale: f32,
    /// Length of the code in bytes.
    pub code_len: u32,
    /// The code bytes, when [`DecodeOptions::keep_tree_code`] was set.
    pub code: Option<Vec<u8>>,
    /// The trailing `build_byte` (§10.4), when present.
    pub build_byte: Option<u8>,
}

/// The collision tree block of a level (`mesh.md` §10.1).
#[derive(Debug, Clone, PartialEq)]
pub enum CollisionTree {
    /// `has_code` = 0: the block is one byte.
    NoCode,
    /// `has_code` = 1.
    Code(TreeCode),
}

/// One level of the record (`mesh.md` §8): a draw list and optional collision data.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    /// Detail or proxy level.
    pub kind: LevelKind,
    /// Draw entries, in draw order (which matters for blending, §8.2).
    pub draws: Vec<Draw>,
    /// Element counts of the collision mesh, when the level has one.
    pub collision_counts: Option<CollisionCounts>,
    /// The collision mesh, when the level has one and it was kept.
    pub collision: Option<CollisionMesh>,
    /// The collision tree block, when the level has one.
    pub tree: Option<CollisionTree>,
}

/// Which rule decided whether `build_byte`s were present (`mesh.md` §10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildByteRule {
    /// The directory `stamp` (normative).
    Stamp(u16),
    /// The look-ahead fallback (no stamp, or a stamp in the unobserved gap 34–37).
    Fallback,
}

/// A decoded mesh record (`mesh.md` §3).
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// The record's own DID.
    pub did: Did,
    /// The `flags` word (§3.3; low bits OPEN).
    pub flags: u32,
    /// Surface records (0x31) referenced by draw entries (§12).
    pub surfaces: Vec<Did>,
    /// Submesh blocks, in storage order.
    pub submeshes: Vec<Submesh>,
    /// Index buffers; buffer *i* belongs to submesh *i* in observed data.
    pub index_buffers: Vec<IndexBuffer>,
    /// Levels: detail levels first, then proxy levels (§8).
    pub levels: Vec<Level>,
    /// Bounds of level 0 (the mesh's bound, §6.1), computed from the positions it draws.
    /// `None` when level 0 draws no finite position.
    pub bounds: Option<Bounds>,
    /// How `build_byte` presence was decided.
    pub build_byte_rule: BuildByteRule,
}

impl Mesh {
    /// Decodes a mesh record with no directory stamp and default options.
    ///
    /// # Errors
    ///
    /// See [`Mesh::decode`].
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes, None, &DecodeOptions::default())
    }

    /// Decodes a mesh record that must carry `did` (MI1).
    ///
    /// # Errors
    ///
    /// [`MeshError::DidMismatch`], or anything [`Mesh::decode`] returns.
    pub fn decode_expecting(
        bytes: &[u8],
        did: Did,
        stamp: Option<u16>,
        options: &DecodeOptions,
    ) -> Result<Self> {
        let found = Did(Cursor::new(bytes).u32("DID")?);
        if found != did {
            return Err(MeshError::DidMismatch {
                expected: did,
                found,
            });
        }
        Self::decode(bytes, stamp, options)
    }

    /// Decodes a whole mesh record (`mesh.md` §13).
    ///
    /// `stamp` is the directory entry's `stamp` of the record (`dat-container.md` §5.2);
    /// it decides whether a `build_byte` follows each collision tree code (§10.4). With
    /// `None`, or a stamp in the unobserved range 34–37, the look-ahead fallback of §10.4
    /// is used.
    ///
    /// Every count is checked against the bytes left before anything is allocated. The
    /// decoded mesh holds at most about 1.4 × `bytes.len()` of vertex, index and
    /// collision data, plus a fixed per-submesh and per-level overhead of a few hundred
    /// bytes each, bounded by [`MAX_SUBMESHES`] and [`MAX_LEVELS`] (a few MB in all).
    /// [`Mesh::heap_size`] reports it.
    ///
    /// # Errors
    ///
    /// Structural problems ([`MeshError::Truncated`], [`MeshError::CountMismatch`],
    /// [`MeshError::UnsupportedVertexFormat`], [`MeshError::IndexOutOfRange`],
    /// [`MeshError::DrawOutOfRange`], [`MeshError::BadPrimitive`],
    /// [`MeshError::BadFlagByte`], [`MeshError::TrailingBytes`], …); in strict mode also
    /// [`MeshError::Invalid`]. Use [`MeshError::category`] to classify them.
    pub fn decode(bytes: &[u8], stamp: Option<u16>, options: &DecodeOptions) -> Result<Self> {
        let mut c = Cursor::new(bytes);
        let header = MeshHeader::read(&mut c)?;
        let n = header.submesh_count;

        // Region B.
        let mut submeshes = Vec::with_capacity(n as usize);
        for i in 0..n {
            submeshes.push(read_submesh(&mut c, i)?);
        }

        // Region C.
        let count = c.u32("index buffer count")?;
        if count != n {
            return Err(MeshError::CountMismatch {
                what: "index buffer count vs submesh count",
                expected: u64::from(n),
                found: u64::from(count),
            });
        }
        let mut index_buffers = Vec::with_capacity(n as usize);
        for (i, sm) in submeshes.iter().enumerate() {
            index_buffers.push(read_index_buffer(&mut c, i as u32, sm.vertex_count)?);
        }
        let vertex_counts: Vec<u32> = submeshes.iter().map(|s| s.vertex_count).collect();
        let buffer_max: Vec<Option<u16>> = index_buffers
            .iter()
            .map(|b| b.indices.iter().copied().max())
            .collect();

        // Region D.
        let rule = match stamp {
            Some(s) if s >= BUILD_BYTE_MIN_STAMP || s <= BUILD_BYTE_ABSENT_MAX_STAMP => {
                BuildByteRule::Stamp(s)
            }
            _ => BuildByteRule::Fallback,
        };
        let l = c.u32("level count")?;
        if l == 0 {
            return Err(MeshError::Empty {
                what: "level list".into(),
            });
        }
        c.need("levels", u64::from(l) * MIN_LEVEL_BYTES)?;
        if l > MAX_LEVELS {
            return Err(MeshError::LimitExceeded {
                what: "levels in one record",
                value: u64::from(l),
                limit: u64::from(MAX_LEVELS),
            });
        }
        let mut levels = Vec::with_capacity(l as usize);
        for li in 0..l {
            let last = li + 1 == l;
            levels.push(read_level(
                &mut c,
                li,
                &vertex_counts,
                &buffer_max,
                rule,
                last,
                options,
            )?);
        }
        if c.remaining() != 0 {
            return Err(MeshError::TrailingBytes {
                offset: c.pos(),
                count: c.remaining(),
            });
        }

        let mut mesh = Self {
            did: header.did,
            flags: header.flags,
            surfaces: header.surfaces,
            submeshes,
            index_buffers,
            levels,
            bounds: None,
            build_byte_rule: rule,
        };
        mesh.bounds = mesh.level_bounds(0);
        tracing::trace!(
            did = %mesh.did,
            submeshes = mesh.submeshes.len(),
            levels = mesh.levels.len(),
            ?rule,
            "mesh decoded"
        );
        if options.strict
            && let Some(issue) = mesh
                .validate()
                .issues
                .into_iter()
                .find(|i| i.kind.class() == crate::validate::IssueClass::Structural)
        {
            return Err(MeshError::Invalid(issue));
        }
        Ok(mesh)
    }

    /// Region A as a [`MeshHeader`].
    #[must_use]
    pub fn header(&self) -> MeshHeader {
        MeshHeader {
            did: self.did,
            flags: self.flags,
            surfaces: self.surfaces.clone(),
            submesh_count: self.submeshes.len() as u32,
        }
    }

    /// Whether `flags` bit 28 is set (informational, §3.3).
    #[must_use]
    pub fn is_skinning_flagged(&self) -> bool {
        self.flags & SKINNING_FLAG != 0
    }

    /// Whether any submesh carries bone influences.
    #[must_use]
    pub fn has_skinned_submesh(&self) -> bool {
        self.submeshes.iter().any(Submesh::is_skinned)
    }

    /// Indices of the detail levels (kind 1), in order. A viewer draws the first.
    pub fn detail_levels(&self) -> impl Iterator<Item = usize> + '_ {
        self.levels
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == LevelKind::Detail)
            .map(|(i, _)| i)
    }

    /// The surface DID bound by `draw` (§12), if its index is in range.
    #[must_use]
    pub fn surface_of(&self, draw: &Draw) -> Option<Did> {
        self.surfaces.get(draw.surface as usize).copied()
    }

    /// Triangles drawn by `level` (sum over its draw entries, §8.2).
    #[must_use]
    pub fn triangle_count(&self, level: usize) -> usize {
        self.levels.get(level).map_or(0, |l| {
            l.draws
                .iter()
                .filter_map(|d| self.index_buffers.get(d.index_buffer as usize))
                .map(IndexBuffer::triangle_count)
                .sum()
        })
    }

    /// Vertices drawn by `level`.
    #[must_use]
    pub fn vertex_count(&self, level: usize) -> usize {
        self.levels.get(level).map_or(0, |l| {
            l.draws
                .iter()
                .filter_map(|d| self.submeshes.get(d.vertex_block as usize))
                .map(|s| s.streams.positions.len())
                .sum()
        })
    }

    /// Bounds of the positions drawn by `level`, or `None` if the level does not exist or
    /// draws no finite position.
    #[must_use]
    pub fn level_bounds(&self, level: usize) -> Option<Bounds> {
        let level = self.levels.get(level)?;
        // Each vertex block once, however many draws use it (linear in the record size).
        let mut used = vec![false; self.submeshes.len()];
        for d in &level.draws {
            if let Some(u) = used.get_mut(d.vertex_block as usize) {
                *u = true;
            }
        }
        let blocks = || {
            self.submeshes
                .iter()
                .zip(&used)
                .filter(|(_, u)| **u)
                .flat_map(|(s, _)| s.streams.positions.iter())
        };
        let aabb = Aabb::from_points(blocks())?;
        let sphere = BoundingSphere::around(&aabb, blocks());
        Some(Bounds { aabb, sphere })
    }

    /// Bytes held: the `Mesh` value itself and every heap allocation it owns, by
    /// capacity (for cache budgets). Allocator bookkeeping is not counted.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        use std::mem::size_of;
        let sub: usize = self
            .submeshes
            .iter()
            .map(|s| s.streams.heap_size() + s.influence_list.capacity() * size_of::<u32>())
            .sum();
        let ib: usize = self
            .index_buffers
            .iter()
            .map(|b| b.indices.capacity() * size_of::<u16>())
            .sum();
        let lv: usize = self
            .levels
            .iter()
            .map(|l| {
                l.draws.capacity() * size_of::<Draw>()
                    + l.collision.as_ref().map_or(0, |c| {
                        c.vertices.capacity() * size_of::<CollisionVertex>()
                            + c.triangles.capacity() * size_of::<CollisionTriangle>()
                            + c.edges.capacity() * size_of::<CollisionEdge>()
                    })
                    + match &l.tree {
                        Some(CollisionTree::Code(t)) => t.code.as_ref().map_or(0, Vec::capacity),
                        _ => 0,
                    }
            })
            .sum();
        size_of::<Self>()
            + self.surfaces.capacity() * size_of::<Did>()
            + self.submeshes.capacity() * size_of::<Submesh>()
            + self.index_buffers.capacity() * size_of::<IndexBuffer>()
            + self.levels.capacity() * size_of::<Level>()
            + sub
            + ib
            + lv
    }
}

fn read_submesh(c: &mut Cursor<'_>, index: u32) -> Result<Submesh> {
    let vfmt = c.u32("vertex format")?;
    let format = VertexFormat::new(vfmt).map_err(|e| MeshError::UnsupportedVertexFormat {
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
    let layout = format.layout();
    let vbytes = u64::from(v) * layout.stride as u64;
    c.need("vertex block and trailer", vbytes + 28)?;
    let raw = c.take("vertex block", vbytes)?;
    let streams = VertexStreams::decode(raw, &layout);
    let min = c.vec3("AABB")?;
    let max = c.vec3("AABB")?;
    let i = c.count("influence list", 4)?;
    let mut influence_list = Vec::with_capacity(i as usize);
    for _ in 0..i {
        influence_list.push(c.u32("influence")?);
    }
    Ok(Submesh {
        format,
        vertex_count: v,
        streams,
        stored_aabb: Aabb { min, max },
        influence_list,
    })
}

fn read_index_buffer(c: &mut Cursor<'_>, buffer: u32, vertex_count: u32) -> Result<IndexBuffer> {
    let n = c.count("index buffer", 2)?;
    if n % 3 != 0 {
        return Err(MeshError::PartialTriangle { buffer, count: n });
    }
    let raw = c.take("index buffer", u64::from(n) * 2)?;
    let mut indices = Vec::with_capacity(n as usize);
    for (position, b) in raw.as_chunks::<2>().0.iter().enumerate() {
        let value = u16::from_le_bytes([b[0], b[1]]);
        if u32::from(value) >= vertex_count {
            return Err(MeshError::IndexOutOfRange {
                buffer,
                position,
                value: u32::from(value),
                vertex_count,
            });
        }
        indices.push(value);
    }
    Ok(IndexBuffer { indices })
}

fn flag_byte(c: &mut Cursor<'_>, what: &'static str) -> Result<bool> {
    let offset = c.pos();
    match c.u8(what)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(MeshError::BadFlagByte {
            what,
            offset,
            value,
        }),
    }
}

fn read_level(
    c: &mut Cursor<'_>,
    level: u32,
    vertex_counts: &[u32],
    buffer_max: &[Option<u16>],
    rule: BuildByteRule,
    last: bool,
    options: &DecodeOptions,
) -> Result<Level> {
    let kind = LevelKind::from_raw(c.u32("level kind")?);
    let d = c.count("draw list", DRAW_BYTES)?;
    if d == 0 {
        return Err(MeshError::Empty {
            what: format!("draw list of level {level}"),
        });
    }
    let n = vertex_counts.len() as u32;
    let mut draws = Vec::with_capacity(d as usize);
    for draw in 0..d {
        let primitive = c.u8("primitive")?;
        if primitive != PRIMITIVE_TRIANGLE_LIST {
            return Err(MeshError::BadPrimitive {
                level,
                draw,
                value: primitive,
            });
        }
        let entry = Draw {
            primitive,
            vertex_block: c.u32("draw vertex block")?,
            index_buffer: c.u32("draw index buffer")?,
            surface: c.u32("draw surface")?,
        };
        for (field, value) in [
            ("vertex_block", entry.vertex_block),
            ("index_buffer", entry.index_buffer),
            ("surface", entry.surface),
        ] {
            if value >= n {
                return Err(MeshError::DrawOutOfRange {
                    level,
                    draw,
                    field,
                    value,
                    count: n,
                });
            }
        }
        // A draw may pair any vertex block with any index buffer (§8.1); the indices must
        // still address the vertex block actually bound.
        if entry.vertex_block != entry.index_buffer {
            let vc = vertex_counts[entry.vertex_block as usize];
            if let Some(max) = buffer_max[entry.index_buffer as usize]
                && u32::from(max) >= vc
            {
                return Err(MeshError::DrawOutOfRange {
                    level,
                    draw,
                    field: "index",
                    value: u32::from(max),
                    count: vc,
                });
            }
        }
        draws.push(entry);
    }

    let (collision_counts, collision) = if flag_byte(c, "has_collision_mesh")? {
        let (counts, mesh) = read_collision(c, options.keep_collision)?;
        (Some(counts), mesh)
    } else {
        (None, None)
    };

    let tree = if flag_byte(c, "has_tree")? {
        Some(read_tree(c, rule, last, options.keep_tree_code)?)
    } else {
        None
    };

    Ok(Level {
        kind,
        draws,
        collision_counts,
        collision,
        tree,
    })
}

fn read_collision(
    c: &mut Cursor<'_>,
    keep: bool,
) -> Result<(CollisionCounts, Option<CollisionMesh>)> {
    let mut mesh = CollisionMesh::default();
    let cv = c.count("collision vertices", 3)?;
    let raw = c.take("collision vertices", u64::from(cv) * 3)?;
    if keep {
        mesh.vertices = raw
            .as_chunks::<3>()
            .0
            .iter()
            .map(|b| CollisionVertex {
                draw: b[0],
                vertex: u16::from_le_bytes([b[1], b[2]]),
            })
            .collect();
    }
    let u16s = |b: &[u8], i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
    let ct = c.count("collision triangles", 12)?;
    let raw = c.take("collision triangles", u64::from(ct) * 12)?;
    if keep {
        mesh.triangles = raw
            .as_chunks::<12>()
            .0
            .iter()
            .map(|b| CollisionTriangle {
                vertices: [u16s(b, 0), u16s(b, 2), u16s(b, 4)],
                edges: [u16s(b, 1), u16s(b, 3), u16s(b, 5)],
            })
            .collect();
    }
    let ce = c.count("collision edges", 8)?;
    let raw = c.take("collision edges", u64::from(ce) * 8)?;
    if keep {
        mesh.edges = raw
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| CollisionEdge {
                vertices: [u16s(b, 0), u16s(b, 2)],
                faces: [u16s(b, 1), u16s(b, 3)],
            })
            .collect();
    }
    let counts = CollisionCounts {
        vertices: cv,
        triangles: ct,
        edges: ce,
    };
    Ok((counts, keep.then_some(mesh)))
}

/// Whether a `build_byte` follows the tree code (§10.4). `rest` is what follows the code.
pub(crate) fn build_byte_present(rule: BuildByteRule, last: bool, rest: &[u8]) -> bool {
    match rule {
        BuildByteRule::Stamp(s) => s >= BUILD_BYTE_MIN_STAMP,
        BuildByteRule::Fallback => {
            if last {
                rest.len() == 1
            } else if rest.len() < 8 {
                true
            } else {
                let kind = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
                let draws = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]);
                !((kind == 1 || kind == 2) && draws >= 1)
            }
        }
    }
}

fn read_tree(
    c: &mut Cursor<'_>,
    rule: BuildByteRule,
    last: bool,
    keep_code: bool,
) -> Result<CollisionTree> {
    if !flag_byte(c, "has_code")? {
        return Ok(CollisionTree::NoCode);
    }
    let origin = c.vec3("tree origin")?;
    let scale = c.f32("tree scale")?;
    let code_len = c.count("tree code", 1)?;
    let raw = c.take("tree code", u64::from(code_len))?;
    let build_byte = if build_byte_present(rule, last, c.rest()) {
        Some(c.u8("build_byte")?)
    } else {
        None
    };
    Ok(CollisionTree::Code(TreeCode {
        origin,
        scale,
        code_len,
        code: keep_code.then(|| raw.to_vec()),
        build_byte,
    }))
}
