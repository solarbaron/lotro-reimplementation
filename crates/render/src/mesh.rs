// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine-neutral mesh input ([`MeshData`]), its validation and the CPU-side
//! preparation (vertex packing, generated normals, wireframe edges, bounds)
//! that runs before a mesh is copied into the shared GPU arena.
//!
//! The renderer does not depend on any decoder: callers (e.g. a mesh record
//! decoder) convert their output into these plain streams. Every stream is
//! treated as untrusted: lengths, index ranges and submesh ranges are checked
//! against each other and against [`MeshLimits`] before anything is
//! allocated, and non-finite attribute values are sanitised.
//!
//! Conventions (Tessera's own, see `docs/specs/render.md` §2.1):
//!
//! - Triangle lists only. A triangle is front-facing when its vertices appear
//!   **counter-clockwise** seen from the front (wgpu's default). Sources with
//!   the opposite convention call [`MeshData::flip_winding`] first.
//! - Positions are in the mesh's local space, in the caller's units.
//! - `uv` `(0, 0)` addresses the first texel of the first row of a texture
//!   uploaded with [`crate::TextureUpload`] (top row first).
//! - Tangents are `[tx, ty, tz, w]` with `w` the bitangent sign
//!   (`bitangent = w * cross(normal, tangent)`).
//! - Binormals (optional, only with tangents) are the stored `+v` directions of
//!   a source that has them (mesh records, `mesh.md` §5.6). The lit and
//!   instanced pipelines use them as **B** as given (`render.md` §4.1, R-D10);
//!   without them, or where one is zero, they derive `w * cross(normal,
//!   tangent)`.

use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec3};

use crate::arena::MeshConfig;
use crate::error::{RenderError, Result};
use crate::skin::{GpuSkin, MeshSkin, SkinInfo};

/// Index stream of a [`MeshData`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indices {
    /// 16-bit indices.
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

    /// Whether there are no indices.
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

    fn swap(&mut self, a: usize, b: usize) {
        match self {
            Self::U16(v) => v.swap(a, b),
            Self::U32(v) => v.swap(a, b),
        }
    }
}

impl Default for Indices {
    fn default() -> Self {
        Self::U32(Vec::new())
    }
}

/// A contiguous run of triangles in the index stream, drawn with one
/// material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Submesh {
    /// First index (a multiple of 3).
    pub first_index: u32,
    /// Number of indices (a multiple of 3).
    pub index_count: u32,
}

/// Engine-neutral mesh streams handed to [`crate::Renderer::upload_mesh`].
///
/// Only `positions` and `indices` are required. Optional streams must have
/// exactly one entry per position. Missing normals are generated
/// (area-weighted face normals); missing UVs are `(0, 0)`; missing colours
/// are opaque white; without tangents the mesh is drawn without normal
/// mapping. Binormals require tangents; without them the lit pipelines derive
/// the binormal from the normal, the tangent and its sign.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    /// Debug label for logs and GPU captures.
    pub label: Option<String>,
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (need not be normalised).
    pub normals: Option<Vec<[f32; 3]>>,
    /// Vertex tangents `[x, y, z, bitangent_sign]`.
    pub tangents: Option<Vec<[f32; 4]>>,
    /// Stored vertex binormals (the `+v` direction, not made orthogonal to
    /// the normal; `mesh.md` §5.6). Only together with `tangents`. The lit
    /// and instanced pipelines use them as the normal map's **B** instead of
    /// `w * cross(normal, tangent)` (`render.md` §4.1, departure R-D10 and
    /// the §4.8 OPEN); a zero binormal falls back to the derived one. The M2
    /// pipelines always derive it.
    pub binormals: Option<Vec<[f32; 3]>>,
    /// Texture coordinates.
    pub uvs: Option<Vec<[f32; 2]>>,
    /// Linear RGBA vertex colours, multiplied into the base colour.
    pub colors: Option<Vec<[f32; 4]>>,
    /// Triangle-list indices.
    pub indices: Indices,
    /// Material ranges. Empty means one submesh covering every index.
    pub submeshes: Vec<Submesh>,
}

/// Upper bounds applied to every [`MeshData`] before anything is allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshLimits {
    /// Largest vertex count of one mesh.
    pub max_vertices: u32,
    /// Largest index count of one mesh.
    pub max_indices: u32,
    /// Largest submesh count of one mesh.
    pub max_submeshes: u32,
}

impl Default for MeshLimits {
    fn default() -> Self {
        Self {
            max_vertices: 1 << 21,
            max_indices: 1 << 24,
            max_submeshes: 4096,
        }
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl Aabb {
    /// Centre point.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        ((self.min.as_dvec3() + self.max.as_dvec3()) * 0.5).as_vec3()
    }

    /// The axis-aligned box around this box's eight corners after `transform` (the
    /// world-space box of a placed object). `None` when a corner is not finite.
    #[must_use]
    pub fn transformed(&self, transform: Mat4) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for i in 0..8u8 {
            let corner = Vec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            );
            let p = transform.transform_point3(corner);
            if !p.is_finite() {
                return None;
            }
            min = min.min(p);
            max = max.max(p);
        }
        Some(Self { min, max })
    }
}

/// Bounding sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingSphere {
    /// Centre.
    pub center: Vec3,
    /// Radius (>= 0).
    pub radius: f32,
}

impl BoundingSphere {
    /// A sphere enclosing this one after `transform` (centre transformed,
    /// radius scaled by the largest axis scale). Non-finite results fall
    /// back to the untransformed sphere.
    #[must_use]
    pub fn transformed(&self, transform: Mat4) -> Self {
        let center = transform.transform_point3(self.center);
        let scale = [transform.x_axis, transform.y_axis, transform.z_axis]
            .iter()
            .map(|a| a.truncate().length())
            .fold(0.0f32, f32::max);
        let radius = self.radius * scale;
        if center.is_finite() && radius.is_finite() {
            Self { center, radius }
        } else {
            *self
        }
    }
}

/// Bounds of one submesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubmeshInfo {
    /// The submesh's index range.
    pub range: Submesh,
    /// Box around the vertices its triangles reference.
    pub aabb: Aabb,
    /// Sphere around the same vertices.
    pub sphere: BoundingSphere,
}

/// Summary of a validated mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshInfo {
    /// Vertex count.
    pub vertex_count: u32,
    /// Index count (triangles × 3).
    pub index_count: u32,
    /// Line-list indices generated for wireframe display (0 when disabled).
    pub edge_index_count: u32,
    /// Box around all vertices.
    pub aabb: Aabb,
    /// Sphere around all vertices (centred on the box centre).
    pub sphere: BoundingSphere,
    /// Per-submesh bounds (at least one entry).
    pub submeshes: Vec<SubmeshInfo>,
    /// Normals were supplied (`false`: generated).
    pub has_normals: bool,
    /// Tangents were supplied (normal mapping possible).
    pub has_tangents: bool,
    /// Stored binormals were supplied (the lit pipelines use them as **B**).
    pub has_binormals: bool,
    /// UVs were supplied.
    pub has_uvs: bool,
    /// Vertex colours were supplied.
    pub has_colors: bool,
    /// Bone influences of a skinned mesh (`None`: static mesh).
    pub skin: Option<SkinInfo>,
}

/// GPU vertex (must match `VsIn` in `mesh.wgsl`, `lit_common.wgsl` and
/// `ModelIn` in `instanced_common.wgsl`). 48 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuVertex {
    pub position: [f32; 3],
    /// snorm16 xyz, w = 0.
    pub normal: [i16; 4],
    /// snorm16 xyz, w = ±1 (bitangent sign).
    pub tangent: [i16; 4],
    pub uv: [f32; 2],
    /// unorm8 RGBA (linear).
    pub color: [u8; 4],
    /// snorm16 xyz of the stored binormal, normalised, w = 0; all zero when
    /// the mesh has none (the shaders then derive it). Shader location 13.
    pub binormal: [i16; 4],
}

/// Per-submesh data kept with an uploaded mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedSubmesh {
    /// Range in the triangle indices.
    pub first_index: u32,
    pub index_count: u32,
    /// Range in the edge indices (relative to the start of the edge list).
    pub first_edge: u32,
    pub edge_count: u32,
    pub sphere: BoundingSphere,
}

/// A [`MeshData`] validated and converted to the renderer's GPU layout:
/// packed vertices, widened indices, generated normals, wireframe edges and
/// bounds.
///
/// Made by [`MeshData::prepare`], which does all the CPU work of an upload
/// and needs no GPU, so it can run on a worker thread (`PreparedMesh` is
/// `Send + Sync`); [`crate::Renderer::upload_prepared`] then only copies it
/// into the shared buffers. The contents are private, so a prepared mesh
/// always satisfies the checks it was prepared under.
#[derive(Debug, Clone)]
pub struct PreparedMesh {
    pub(crate) vertices: Vec<GpuVertex>,
    /// Triangle indices followed by edge (line-list) indices, local to the mesh.
    pub(crate) indices: Vec<u32>,
    pub(crate) submeshes: Vec<PreparedSubmesh>,
    pub(crate) info: MeshInfo,
    /// Packed bone influences, one per vertex (skinned meshes only).
    pub(crate) skin: Option<Vec<GpuSkin>>,
}

impl PreparedMesh {
    /// Bounds and stream summary (what [`crate::Renderer::mesh_info`]
    /// reports after the upload, unless the renderer drops the edges).
    #[must_use]
    pub fn info(&self) -> &MeshInfo {
        &self.info
    }

    /// Bytes this mesh occupies in the shared buffers (vertices, triangle
    /// indices, edge indices and, for a skinned mesh, bone influences).
    #[must_use]
    pub fn gpu_bytes(&self) -> u64 {
        (self.vertices.len() as u64) * size_of::<GpuVertex>() as u64
            + (self.indices.len() as u64) * 4
            + self
                .skin
                .as_ref()
                .map_or(0, |s| (s.len() as u64) * size_of::<GpuSkin>() as u64)
    }

    /// Drop the wireframe edge list (for a renderer that does not keep
    /// edges).
    pub(crate) fn strip_edges(&mut self) {
        self.indices.truncate(self.info.index_count as usize);
        for s in &mut self.submeshes {
            s.first_edge = 0;
            s.edge_count = 0;
        }
        self.info.edge_index_count = 0;
    }
}

fn invalid(msg: impl Into<String>) -> RenderError {
    RenderError::InvalidMesh(msg.into())
}

fn snorm16(v: f32) -> i16 {
    // Saturating float → int cast; NaN maps to 0.
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn unorm8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Normalise in f64, `None` for zero/non-finite input.
fn normalize3(v: DVec3) -> Option<DVec3> {
    if !v.is_finite() {
        return None;
    }
    let m = v.abs().max_element();
    if m <= 0.0 || !m.is_normal() {
        return None;
    }
    (v / m).try_normalize()
}

/// Fallback normal for vertices without usable normal data.
const FALLBACK_NORMAL: DVec3 = DVec3::Z;

fn check_len<T>(name: &str, s: Option<&Vec<T>>, n: usize) -> Result<()> {
    match s {
        Some(v) if v.len() != n => Err(invalid(format!(
            "{name} has {} entries, positions has {n}",
            v.len()
        ))),
        _ => Ok(()),
    }
}

impl MeshData {
    /// Mesh from positions and 32-bit triangle indices.
    #[must_use]
    pub fn new(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Self {
        Self {
            positions,
            indices: Indices::U32(indices),
            ..Self::default()
        }
    }

    /// Reverse the winding of every triangle (for sources whose front faces
    /// are clockwise). A trailing partial triangle is left untouched.
    pub fn flip_winding(&mut self) {
        let n = self.indices.len() / 3;
        for t in 0..n {
            self.indices.swap(t * 3 + 1, t * 3 + 2);
        }
    }

    /// Fill `normals` with area-weighted face normals (the same rule
    /// [`crate::Renderer::upload_mesh`] applies when normals are missing).
    /// Out-of-range indices and a trailing partial triangle are ignored.
    pub fn compute_normals(&mut self) {
        let positions = to_dvec(&self.positions);
        let tris = self.triangles();
        let normals = smooth_normals(&positions, &tris);
        self.normals = Some(normals.iter().map(|n| n.as_vec3().to_array()).collect());
    }

    /// Fill `tangents` from the positions, normals and UVs: per-vertex
    /// sums of each triangle's UV-space derivatives, made orthogonal to the
    /// normal, with `w` the handedness so that `w * cross(normal, tangent)`
    /// points along increasing `v`. Computes normals first when missing.
    /// Returns `false` (and leaves `tangents` unchanged) without UVs.
    /// Replacing the tangents drops any stored `binormals` (they belonged to
    /// the old frame), so the lit pipelines derive the binormal.
    pub fn compute_tangents(&mut self) -> bool {
        if self
            .uvs
            .as_ref()
            .is_none_or(|u| u.len() != self.positions.len())
        {
            return false;
        }
        if self
            .normals
            .as_ref()
            .is_none_or(|n| n.len() != self.positions.len())
        {
            self.compute_normals();
        }
        let uvs = self.uvs.as_deref().unwrap_or(&[]);
        let positions = to_dvec(&self.positions);
        let normals: Vec<DVec3> = self
            .normals
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|n| DVec3::new(f64::from(n[0]), f64::from(n[1]), f64::from(n[2])))
            .collect();
        let n = positions.len();
        let mut sdir = vec![DVec3::ZERO; n];
        let mut tdir = vec![DVec3::ZERO; n];
        for t in self.triangles() {
            let [a, b, c] = t.map(|i| i as usize);
            let uv = |i: usize| (f64::from(uvs[i][0]), f64::from(uvs[i][1]));
            let (e1, e2) = (positions[b] - positions[a], positions[c] - positions[a]);
            let ((u0, v0), (u1, v1), (u2, v2)) = (uv(a), uv(b), uv(c));
            let (du1, dv1, du2, dv2) = (u1 - u0, v1 - v0, u2 - u0, v2 - v0);
            let r = du1 * dv2 - du2 * dv1;
            if !r.is_finite() || r.abs() < 1e-20 {
                continue;
            }
            let s = (e1 * dv2 - e2 * dv1) / r;
            let t = (e2 * du1 - e1 * du2) / r;
            if !(s.is_finite() && t.is_finite()) {
                continue;
            }
            for v in [a, b, c] {
                sdir[v] += s;
                tdir[v] += t;
            }
        }
        let tangents = (0..n)
            .map(|i| {
                let nrm = normalize3(normals[i]).unwrap_or(FALLBACK_NORMAL);
                let t = normalize3(sdir[i] - nrm * nrm.dot(sdir[i]))
                    .or_else(|| normalize3(nrm.any_orthogonal_vector()))
                    .unwrap_or(DVec3::X);
                let w = if nrm.cross(t).dot(tdir[i]) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                [t.x as f32, t.y as f32, t.z as f32, w]
            })
            .collect();
        self.tangents = Some(tangents);
        self.binormals = None;
        true
    }

    /// Complete, in-range triangles.
    fn triangles(&self) -> Vec<[u32; 3]> {
        let n = self.positions.len();
        (0..self.indices.len() / 3)
            .filter_map(|t| {
                let tri = [
                    self.indices.get(t * 3)?,
                    self.indices.get(t * 3 + 1)?,
                    self.indices.get(t * 3 + 2)?,
                ];
                tri.iter().all(|&i| (i as usize) < n).then_some(tri)
            })
            .collect()
    }

    /// Validate the streams against `config.limits` and convert them to the
    /// GPU layout, building wireframe edges when `config.wireframe_edges` is
    /// set. This is all the CPU work of [`crate::Renderer::upload_mesh`]; it
    /// needs no GPU, so callers can run it on a worker thread and hand the
    /// result to [`crate::Renderer::upload_prepared`].
    ///
    /// Work and memory are linear in the stream lengths, which `limits`
    /// bound: submesh ranges together may not cover more indices than the
    /// mesh has (overlapping ranges are only accepted while that holds).
    pub fn prepare(&self, config: &MeshConfig) -> Result<PreparedMesh> {
        prepare_with(self, &config.limits, config.wireframe_edges)
    }

    /// Validate the streams and compute the summary that
    /// [`crate::Renderer::upload_mesh`] would produce on a renderer with
    /// this `config` (including `edge_index_count`, which depends on
    /// [`MeshConfig::wireframe_edges`]), without a GPU. The same work as
    /// [`Self::prepare`]; keep the prepared mesh instead when it is going
    /// to be uploaded.
    pub fn validate(&self, config: &MeshConfig) -> Result<MeshInfo> {
        self.prepare(config).map(|p| p.info)
    }

    /// [`Self::prepare`] for a skinned mesh: the streams plus `skin`'s bone
    /// influences (validated with [`MeshSkin::validate`], then packed with
    /// weights renormalised to sum exactly 1). The result reports
    /// [`MeshInfo::skin`] and is drawn skinned when a
    /// [`crate::MeshDraw`] carries a palette ([`crate::MeshDraw::with_skin`]),
    /// in its bind pose otherwise.
    ///
    /// # Errors
    ///
    /// Those of [`Self::prepare`], then [`RenderError::InvalidSkin`].
    pub fn prepare_skinned(&self, skin: &MeshSkin, config: &MeshConfig) -> Result<PreparedMesh> {
        let mut prepared = self.prepare(config)?;
        let (info, data) = skin.pack(self.positions.len(), true)?;
        prepared.info.skin = Some(info);
        prepared.skin = Some(data);
        Ok(prepared)
    }

    /// [`Self::validate`] for a skinned mesh (the summary
    /// [`crate::Renderer::upload_skinned_mesh`] would report).
    ///
    /// # Errors
    ///
    /// As [`Self::prepare_skinned`].
    pub fn validate_skinned(&self, skin: &MeshSkin, config: &MeshConfig) -> Result<MeshInfo> {
        let mut info = self.validate(config)?;
        info.skin = Some(skin.validate(self.positions.len())?);
        Ok(info)
    }
}

/// Validate and convert `data` into GPU-ready vectors.
pub(crate) fn prepare_with(
    data: &MeshData,
    limits: &MeshLimits,
    edges: bool,
) -> Result<PreparedMesh> {
    let n = data.positions.len();
    if n == 0 {
        return Err(invalid("mesh has no vertices"));
    }
    if n > limits.max_vertices as usize {
        return Err(RenderError::LimitExceeded {
            what: "mesh vertices",
            requested: n as u64,
            max: u64::from(limits.max_vertices),
        });
    }
    let index_count = data.indices.len();
    if index_count > limits.max_indices as usize {
        return Err(RenderError::LimitExceeded {
            what: "mesh indices",
            requested: index_count as u64,
            max: u64::from(limits.max_indices),
        });
    }
    if index_count == 0 || !index_count.is_multiple_of(3) {
        return Err(invalid(format!(
            "index count {index_count} is not a positive multiple of 3"
        )));
    }
    if data.submeshes.len() > limits.max_submeshes as usize {
        return Err(RenderError::LimitExceeded {
            what: "mesh submeshes",
            requested: data.submeshes.len() as u64,
            max: u64::from(limits.max_submeshes),
        });
    }
    check_len("normals", data.normals.as_ref(), n)?;
    check_len("tangents", data.tangents.as_ref(), n)?;
    check_len("binormals", data.binormals.as_ref(), n)?;
    if data.binormals.is_some() && data.tangents.is_none() {
        return Err(invalid("binormals without tangents"));
    }
    check_len("uvs", data.uvs.as_ref(), n)?;
    check_len("colors", data.colors.as_ref(), n)?;

    // Positions must be finite: there is no sensible substitute.
    if let Some(i) = data
        .positions
        .iter()
        .position(|p| !p.iter().all(|c| c.is_finite()))
    {
        return Err(invalid(format!("non-finite position at vertex {i}")));
    }

    // Indices (index_count fits in u32: bounded by max_indices).
    let mut indices: Vec<u32> = Vec::with_capacity(index_count);
    for i in 0..index_count {
        let v = data.indices.get(i).unwrap_or(0);
        if v as usize >= n {
            return Err(invalid(format!(
                "index {i} = {v} is out of range for {n} vertices"
            )));
        }
        indices.push(v);
    }

    // Submeshes.
    let ranges: Vec<Submesh> = if data.submeshes.is_empty() {
        vec![Submesh {
            first_index: 0,
            index_count: index_count as u32,
        }]
    } else {
        data.submeshes.clone()
    };
    for (i, s) in ranges.iter().enumerate() {
        let end = u64::from(s.first_index) + u64::from(s.index_count);
        if !s.first_index.is_multiple_of(3)
            || !s.index_count.is_multiple_of(3)
            || end > index_count as u64
        {
            return Err(invalid(format!(
                "submesh {i} ({}..{end}) is not a whole-triangle range of {index_count} indices",
                s.first_index
            )));
        }
    }
    // Every per-submesh pass (bounds, edges, draws) walks its range, so
    // duplicated or overlapping ranges would multiply the work and the edge
    // memory by up to `max_submeshes`. Requiring the ranges to cover at most
    // `index_count` indices in total keeps everything linear.
    let covered: u64 = ranges.iter().map(|s| u64::from(s.index_count)).sum();
    if covered > index_count as u64 {
        return Err(invalid(format!(
            "submesh ranges cover {covered} indices but the mesh has {index_count} \
             (overlapping or duplicated ranges)"
        )));
    }

    let positions = to_dvec(&data.positions);

    // Normals: supplied (sanitised) or generated.
    let normals: Vec<DVec3> = match &data.normals {
        Some(ns) => ns
            .iter()
            .map(|v| {
                normalize3(DVec3::new(
                    f64::from(v[0]),
                    f64::from(v[1]),
                    f64::from(v[2]),
                ))
                .unwrap_or(FALLBACK_NORMAL)
            })
            .collect(),
        None => {
            let tris: Vec<[u32; 3]> = indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [t[0], t[1], t[2]])
                .collect();
            smooth_normals(&positions, &tris)
        }
    };

    let vertices: Vec<GpuVertex> = (0..n)
        .map(|i| {
            let p = data.positions[i];
            let nr = normals[i];
            let normal = [
                snorm16(nr.x as f32),
                snorm16(nr.y as f32),
                snorm16(nr.z as f32),
                0,
            ];
            let tangent = match data.tangents.as_ref().and_then(|t| t.get(i)) {
                Some(t) => {
                    let dir = normalize3(DVec3::new(
                        f64::from(t[0]),
                        f64::from(t[1]),
                        f64::from(t[2]),
                    ))
                    .unwrap_or(DVec3::ZERO);
                    let sign = if t[3] < 0.0 { -32767 } else { 32767 };
                    [
                        snorm16(dir.x as f32),
                        snorm16(dir.y as f32),
                        snorm16(dir.z as f32),
                        sign,
                    ]
                }
                None => [0, 0, 0, 32767],
            };
            let binormal = match data.binormals.as_ref().and_then(|b| b.get(i)) {
                Some(b) => {
                    let dir = normalize3(DVec3::new(
                        f64::from(b[0]),
                        f64::from(b[1]),
                        f64::from(b[2]),
                    ))
                    .unwrap_or(DVec3::ZERO);
                    [
                        snorm16(dir.x as f32),
                        snorm16(dir.y as f32),
                        snorm16(dir.z as f32),
                        0,
                    ]
                }
                None => [0; 4],
            };
            let uv = match data.uvs.as_ref().and_then(|u| u.get(i)) {
                Some(&[u, v]) => [
                    if u.is_finite() { u } else { 0.0 },
                    if v.is_finite() { v } else { 0.0 },
                ],
                None => [0.0, 0.0],
            };
            let color = match data.colors.as_ref().and_then(|c| c.get(i)) {
                Some(c) => c.map(unorm8),
                None => [255; 4],
            };
            GpuVertex {
                position: p,
                normal,
                tangent,
                uv,
                color,
                binormal,
            }
        })
        .collect();

    // Bounds.
    let all = bounds(&positions, None);
    let mut submeshes = Vec::with_capacity(ranges.len());
    let mut infos = Vec::with_capacity(ranges.len());
    let mut edge_list: Vec<u32> = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    for s in &ranges {
        let tri = &indices[s.first_index as usize..(s.first_index + s.index_count) as usize];
        let (aabb, sphere) = if tri.is_empty() {
            all
        } else {
            bounds(&positions, Some(tri))
        };
        let first_edge = edge_len(&edge_list)?;
        if edges {
            seen.clear();
            for t in tri.as_chunks::<3>().0 {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if a != b && seen.insert((a.min(b), a.max(b))) {
                        edge_list.push(a);
                        edge_list.push(b);
                    }
                }
            }
        }
        submeshes.push(PreparedSubmesh {
            first_index: s.first_index,
            index_count: s.index_count,
            first_edge,
            edge_count: edge_len(&edge_list)?.saturating_sub(first_edge),
            sphere,
        });
        infos.push(SubmeshInfo {
            range: *s,
            aabb,
            sphere,
        });
    }
    let edge_index_count = edge_len(&edge_list)?;
    indices.extend_from_slice(&edge_list);

    Ok(PreparedMesh {
        vertices,
        indices,
        submeshes,
        info: MeshInfo {
            vertex_count: n as u32,
            index_count: index_count as u32,
            edge_index_count,
            aabb: all.0,
            sphere: all.1,
            submeshes: infos,
            has_normals: data.normals.is_some(),
            has_tangents: data.tangents.is_some(),
            has_binormals: data.binormals.is_some(),
            has_uvs: data.uvs.is_some(),
            has_colors: data.colors.is_some(),
            skin: None,
        },
        skin: None,
    })
}

/// Length of the edge list as `u32`. At most `2 × index_count` by the
/// submesh coverage check, so this only fails if that invariant breaks.
fn edge_len(edges: &[u32]) -> Result<u32> {
    u32::try_from(edges.len()).map_err(|_| RenderError::LimitExceeded {
        what: "mesh edge indices",
        requested: edges.len() as u64,
        max: u64::from(u32::MAX),
    })
}

fn to_dvec(p: &[[f32; 3]]) -> Vec<DVec3> {
    p.iter()
        .map(|p| DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
        .collect()
}

/// Area-weighted vertex normals (the cross product's length is twice the
/// triangle area). Triangles must be in range.
fn smooth_normals(positions: &[DVec3], tris: &[[u32; 3]]) -> Vec<DVec3> {
    let mut acc = vec![DVec3::ZERO; positions.len()];
    for t in tris {
        let [a, b, c] = t.map(|i| i as usize);
        let (Some(&pa), Some(&pb), Some(&pc)) =
            (positions.get(a), positions.get(b), positions.get(c))
        else {
            continue;
        };
        let face = (pb - pa).cross(pc - pa);
        if face.is_finite() {
            for v in [a, b, c] {
                acc[v] += face;
            }
        }
    }
    acc.into_iter()
        .map(|v| normalize3(v).unwrap_or(FALLBACK_NORMAL))
        .collect()
}

/// Box and sphere around all `positions`, or only the ones referenced by
/// `refs`. Computed in f64 so huge coordinates cannot overflow.
fn bounds(positions: &[DVec3], refs: Option<&[u32]>) -> (Aabb, BoundingSphere) {
    let iter = || -> Box<dyn Iterator<Item = DVec3> + '_> {
        match refs {
            Some(r) => Box::new(r.iter().filter_map(|&i| positions.get(i as usize).copied())),
            None => Box::new(positions.iter().copied()),
        }
    };
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    for p in iter() {
        min = min.min(p);
        max = max.max(p);
    }
    if !(min.is_finite() && max.is_finite()) {
        min = DVec3::ZERO;
        max = DVec3::ZERO;
    }
    let center = (min + max) * 0.5;
    let radius = iter()
        .map(|p| p.distance(center))
        .fold(0.0f64, f64::max)
        .min(f64::from(f32::MAX));
    (
        Aabb {
            min: min.as_vec3(),
            max: max.as_vec3(),
        },
        BoundingSphere {
            center: center.as_vec3(),
            radius: radius as f32,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> MeshData {
        MeshData::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![0, 1, 2],
        )
    }

    #[test]
    fn vertex_layout_is_48_bytes() {
        assert_eq!(size_of::<GpuVertex>(), 48);
    }

    #[test]
    fn generates_ccw_normals_and_bounds() {
        let p = prepare_with(&tri(), &MeshLimits::default(), true).expect("valid");
        // CCW in the XY plane faces +Z.
        assert!(p.vertices.iter().all(|v| v.normal == [0, 0, 32767, 0]));
        assert_eq!(p.info.aabb.min, Vec3::ZERO);
        assert_eq!(p.info.aabb.max, Vec3::new(1.0, 1.0, 0.0));
        assert!((p.info.sphere.radius - 0.5f32.sqrt()).abs() < 1e-6);
        assert_eq!(p.info.edge_index_count, 6);
        assert_eq!(p.indices, vec![0, 1, 2, 0, 1, 1, 2, 2, 0]);
        assert!(!p.info.has_normals);
        assert_eq!(p.vertices[0].color, [255; 4]);
    }

    #[test]
    fn flip_winding_flips_generated_normals() {
        let mut m = tri();
        m.flip_winding();
        let p = prepare_with(&m, &MeshLimits::default(), false).expect("valid");
        assert!(p.vertices.iter().all(|v| v.normal[2] == -32767));
        assert_eq!(p.info.edge_index_count, 0);
    }

    #[test]
    fn shared_edges_are_deduplicated() {
        // Quad from two triangles: 5 unique edges.
        let m = MeshData::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            vec![0, 1, 2, 0, 2, 3],
        );
        let p = prepare_with(&m, &MeshLimits::default(), true).expect("valid");
        assert_eq!(p.info.edge_index_count, 10);
    }

    #[test]
    fn rejects_hostile_streams() {
        let lim = MeshLimits::default();
        let mut m = tri();
        m.indices = Indices::U16(vec![0, 1, 3]);
        assert!(matches!(
            prepare_with(&m, &lim, true),
            Err(RenderError::InvalidMesh(_))
        ));
        let mut m = tri();
        m.indices = Indices::U32(vec![0, 1]);
        assert!(prepare_with(&m, &lim, true).is_err());
        let mut m = tri();
        m.normals = Some(vec![[0.0; 3]; 2]);
        assert!(prepare_with(&m, &lim, true).is_err());
        let mut m = tri();
        m.positions[1][0] = f32::NAN;
        assert!(prepare_with(&m, &lim, true).is_err());
        let mut m = tri();
        m.submeshes = vec![Submesh {
            first_index: 3,
            index_count: 3,
        }];
        assert!(prepare_with(&m, &lim, true).is_err());
        m.submeshes = vec![Submesh {
            first_index: u32::MAX,
            index_count: u32::MAX,
        }];
        assert!(prepare_with(&m, &lim, true).is_err());
        let small = MeshLimits {
            max_vertices: 2,
            ..lim
        };
        assert!(matches!(
            prepare_with(&tri(), &small, true),
            Err(RenderError::LimitExceeded { .. })
        ));
        assert!(prepare_with(&MeshData::default(), &lim, true).is_err());
    }

    #[test]
    fn sanitises_attributes() {
        let mut m = tri();
        m.normals = Some(vec![
            [f32::NAN, 0.0, 0.0],
            [0.0; 3],
            [f32::MAX, f32::MAX, 0.0],
        ]);
        m.uvs = Some(vec![[f32::INFINITY, 0.5], [0.0, 0.0], [1.0, 1.0]]);
        m.colors = Some(vec![[2.0, -1.0, f32::NAN, 0.5]; 3]);
        m.tangents = Some(vec![[1.0, 0.0, 0.0, -1.0]; 3]);
        let p = prepare_with(&m, &MeshLimits::default(), true).expect("valid");
        assert_eq!(p.vertices[0].normal, [0, 0, 32767, 0]);
        assert_eq!(p.vertices[1].normal, [0, 0, 32767, 0]);
        assert_eq!(p.vertices[2].normal, [23170, 23170, 0, 0]);
        assert_eq!(p.vertices[0].uv, [0.0, 0.5]);
        assert_eq!(p.vertices[0].color, [255, 0, 0, 128]);
        assert_eq!(p.vertices[0].tangent, [32767, 0, 0, -32767]);
        assert!(p.info.has_tangents);
        assert!(!p.info.has_binormals);
        assert!(p.vertices.iter().all(|v| v.binormal == [0; 4]));
    }

    /// render.md §4.8 (R-D10): stored binormals are packed as given (normalised, not made
    /// orthogonal to the normal, either handedness); zero or non-finite ones pack as zero,
    /// which makes the lit shaders derive B for that vertex.
    #[test]
    fn packs_stored_binormals() {
        let mut m = tri();
        m.tangents = Some(vec![[1.0, 0.0, 0.0, 1.0]; 3]);
        m.binormals = Some(vec![[0.0, -2.0, 2.0], [0.0; 3], [f32::NAN, 1.0, 0.0]]);
        let p = prepare_with(&m, &MeshLimits::default(), false).expect("valid");
        assert!(p.info.has_binormals);
        assert_eq!(p.vertices[0].binormal, [0, -23170, 23170, 0]);
        assert_eq!(p.vertices[1].binormal, [0; 4]);
        assert_eq!(p.vertices[2].binormal, [0; 4]);
        // The tangent's sign is kept for the derived fallback.
        assert_eq!(p.vertices[0].tangent, [32767, 0, 0, 32767]);
    }

    #[test]
    fn rejects_binormals_without_tangents_or_of_the_wrong_length() {
        let lim = MeshLimits::default();
        let mut m = tri();
        m.binormals = Some(vec![[0.0, 1.0, 0.0]; 3]);
        assert!(matches!(
            prepare_with(&m, &lim, false),
            Err(RenderError::InvalidMesh(_))
        ));
        m.tangents = Some(vec![[1.0, 0.0, 0.0, 1.0]; 3]);
        assert!(prepare_with(&m, &lim, false).is_ok());
        m.binormals = Some(vec![[0.0, 1.0, 0.0]; 2]);
        assert!(matches!(
            prepare_with(&m, &lim, false),
            Err(RenderError::InvalidMesh(_))
        ));
    }

    #[test]
    fn computed_tangents_drop_stored_binormals() {
        let mut m = MeshData {
            uvs: Some(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]),
            tangents: Some(vec![[0.0, 1.0, 0.0, 1.0]; 3]),
            binormals: Some(vec![[1.0, 0.0, 0.0]; 3]),
            ..tri()
        };
        assert!(m.compute_tangents());
        assert!(m.binormals.is_none());
    }

    #[test]
    fn huge_coordinates_do_not_overflow_bounds() {
        let m = MeshData::new(
            vec![
                [f32::MAX, 0.0, 0.0],
                [-f32::MAX, 0.0, 0.0],
                [0.0, f32::MAX, 0.0],
            ],
            vec![0, 1, 2],
        );
        let info = m.validate(&MeshConfig::default()).expect("valid");
        assert!(info.sphere.center.is_finite());
        assert!(info.sphere.radius.is_finite());
    }

    #[test]
    fn submesh_bounds_cover_only_their_vertices() {
        let m = MeshData {
            submeshes: vec![
                Submesh {
                    first_index: 0,
                    index_count: 3,
                },
                Submesh {
                    first_index: 3,
                    index_count: 3,
                },
            ],
            ..MeshData::new(
                vec![
                    [0.0; 3],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [10.0, 0.0, 0.0],
                    [11.0, 0.0, 0.0],
                    [10.0, 1.0, 0.0],
                ],
                vec![0, 1, 2, 3, 4, 5],
            )
        };
        let info = m.validate(&MeshConfig::default()).expect("valid");
        assert_eq!(info.submeshes.len(), 2);
        assert!(info.submeshes[0].aabb.max.x <= 1.0);
        assert!(info.submeshes[1].aabb.min.x >= 10.0);
    }

    #[test]
    fn tangents_follow_uv_directions() {
        // Quad in XY facing +Z, u along +X, v along -Y (v = 0 at the top).
        let mut m = MeshData {
            uvs: Some(vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]),
            ..MeshData::new(
                vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                ],
                vec![0, 1, 2, 0, 2, 3],
            )
        };
        assert!(m.compute_tangents());
        let t = m.tangents.as_ref().expect("tangents");
        for v in t {
            assert!((v[0] - 1.0).abs() < 1e-6 && v[1].abs() < 1e-6, "{v:?}");
            // cross(+Z, +X) = +Y, but v grows towards -Y.
            assert_eq!(v[3], -1.0);
        }
        assert!(m.normals.is_some(), "normals computed on the way");
        let mut no_uv = tri();
        assert!(!no_uv.compute_tangents());
        assert!(no_uv.tangents.is_none());
        // Degenerate UVs fall back to some tangent orthogonal to the normal.
        let mut flat = tri();
        flat.uvs = Some(vec![[0.0; 2]; 3]);
        assert!(flat.compute_tangents());
        let t = flat.tangents.expect("tangents")[0];
        assert!((t[2]).abs() < 1e-6);
    }

    #[test]
    fn compute_normals_tolerates_bad_indices() {
        let mut m = tri();
        m.indices = Indices::U32(vec![0, 1, 2, 0, 1, 99, 5]);
        m.compute_normals();
        assert_eq!(m.normals.as_ref().map(Vec::len), Some(3));
        assert_eq!(m.normals.expect("normals")[0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn transformed_sphere() {
        let s = BoundingSphere {
            center: Vec3::X,
            radius: 1.0,
        };
        let t = s.transformed(Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 3.0, 2.0),
            glam::Quat::IDENTITY,
            Vec3::Y,
        ));
        assert_eq!(t.center, Vec3::new(1.0, 1.0, 0.0));
        assert!((t.radius - 3.0).abs() < 1e-6);
        let nan = s.transformed(Mat4::from_cols_array(&[f32::NAN; 16]));
        assert_eq!(nan, s);
    }

    #[test]
    fn rejects_overlapping_submesh_amplification() {
        // Regression: 4096 copies of one full range used to be accepted and
        // multiplied the bounds/edge work and the edge memory by 4096.
        let base = MeshData::plane(1.0, 1.0, 10, 10);
        let n = base.indices.len() as u32;
        let lim = MeshLimits::default();
        let mut m = base.clone();
        m.submeshes = vec![
            Submesh {
                first_index: 0,
                index_count: n,
            };
            lim.max_submeshes as usize
        ];
        assert!(matches!(
            prepare_with(&m, &lim, true),
            Err(RenderError::InvalidMesh(_))
        ));
        // Two halves that overlap by one triangle cover too much as well.
        m.submeshes = vec![
            Submesh {
                first_index: 0,
                index_count: n / 2 + 3 - (n / 2) % 3,
            },
            Submesh {
                first_index: n / 2 - (n / 2) % 3,
                index_count: n - (n / 2 - (n / 2) % 3),
            },
        ];
        assert!(prepare_with(&m, &lim, true).is_err());
        // Disjoint ranges covering everything, plus empty ones, are fine; the
        // edge list then stays within 2 × index_count.
        let half = (n / 6) * 3;
        m.submeshes = vec![
            Submesh {
                first_index: 0,
                index_count: half,
            },
            Submesh {
                first_index: half,
                index_count: n - half,
            },
            Submesh {
                first_index: 0,
                index_count: 0,
            },
        ];
        let p = prepare_with(&m, &lim, true).expect("disjoint ranges");
        assert_eq!(p.submeshes.len(), 3);
        assert!(p.info.edge_index_count <= 2 * n);
        assert_eq!(p.indices.len() as u32, n + p.info.edge_index_count);
    }

    #[test]
    fn limits_report_limit_exceeded() {
        let lim = MeshLimits::default();
        let small_indices = MeshLimits {
            max_indices: 2,
            ..lim
        };
        assert!(matches!(
            prepare_with(&tri(), &small_indices, true),
            Err(RenderError::LimitExceeded {
                what: "mesh indices",
                requested: 3,
                max: 2
            })
        ));
        let no_submeshes = MeshLimits {
            max_submeshes: 0,
            ..lim
        };
        let mut m = tri();
        // No explicit submeshes: the implicit full range is always allowed.
        assert!(prepare_with(&m, &no_submeshes, true).is_ok());
        m.submeshes = vec![Submesh {
            first_index: 0,
            index_count: 3,
        }];
        assert!(matches!(
            prepare_with(&m, &no_submeshes, true),
            Err(RenderError::LimitExceeded {
                what: "mesh submeshes",
                requested: 1,
                max: 0
            })
        ));
    }

    #[test]
    fn u16_indices_match_u32() {
        let quad = |indices| MeshData {
            indices,
            ..MeshData::new(
                vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                Vec::new(),
            )
        };
        let lim = MeshLimits::default();
        let a = prepare_with(&quad(Indices::U16(vec![0, 1, 2, 0, 2, 3])), &lim, true).expect("u16");
        let b = prepare_with(&quad(Indices::U32(vec![0, 1, 2, 0, 2, 3])), &lim, true).expect("u32");
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.vertices, b.vertices);
        assert_eq!(a.info, b.info);
    }

    #[test]
    fn validate_follows_the_wireframe_setting() {
        let cube = MeshData::cube(1.0);
        let with = MeshConfig::default();
        let without = MeshConfig {
            wireframe_edges: false,
            ..with
        };
        assert_eq!(cube.validate(&with).expect("valid").edge_index_count, 60);
        assert_eq!(cube.validate(&without).expect("valid").edge_index_count, 0);
        let mut p = cube.prepare(&with).expect("valid");
        assert_eq!(p.gpu_bytes(), 24 * 48 + (36 + 60) * 4);
        p.strip_edges();
        let q = cube.prepare(&without).expect("valid");
        assert_eq!(p.info(), q.info());
        assert_eq!(p.indices, q.indices);
        assert_eq!(p.submeshes, q.submeshes);
    }

    #[test]
    fn prepared_mesh_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<PreparedMesh>();
    }
}
