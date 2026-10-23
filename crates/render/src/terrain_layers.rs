// SPDX-License-Identifier: GPL-3.0-or-later
//! The terrain layer pipeline (M4.5, `docs/specs/terrain-texturing.md` §5–§10;
//! `docs/specs/render.md` §5.2).
//!
//! A terrain *chunk* (the terrain or ceiling surface of one landblock) is uploaded once
//! with [`crate::Renderer::upload_terrain_chunk`] from render-owned input types: the
//! chunk's vertices with their coordinate sets 0–3 and tangent frame (§6.1, §6.4) and its
//! drawn layers in stored order, each with its triangles, texture transform, opacity rule,
//! alpha-test threshold, material parameters and textures (§7, §8.1, §8.2). The terrain
//! crate produces these values; this crate does not depend on it.
//!
//! [`Pass::terrain`] draws a chunk in a lit pass: the layers in order on the same
//! triangles, each lit per §9 (material colour *M* pre-multiplied into every light term,
//! colour = 2 × *C* × Σ with the light map added to Σ, specular, emissive, glow, fog) and
//! blended "over" by its opacity α after the discard test (§7, §8). Depth: "less or equal"
//! (reversed depth: `GreaterEqual`) with depth writes on every layer (§8 step 5, Tessera
//! Decision). Chunks cast the sun's shadow.

use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::error::{RenderError, Result};
use crate::frame::Pass;
use crate::instanced::{
    Frustum, light_words, lit_source, max_uniform_bytes, sampler_entry, shadow_depth,
    texture_entry, uniform_entry, uniform_stride, world_depth,
};
use crate::lighting::MAX_DRAW_LIGHTS;
use crate::lit_pipelines::LightTarget;
use crate::material::LightSelection;
use crate::mesh::{Aabb, BoundingSphere};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::renderer::{FrameStats, Renderer, ResolvedTexture, TextureResolver, WorldFrame};
use crate::texture::TextureId;

const TERRAIN_WGSL: &str = include_str!("shaders/terrain_layers.wgsl");

/// Most layers per chunk (the splat limit of the terrain crate; up to 80 are observed,
/// `terrain-texturing.md` §8).
pub const MAX_TERRAIN_LAYERS: usize = 256;

/// Most vertices per chunk (16-bit indices).
pub const MAX_TERRAIN_VERTICES: usize = 1 << 16;

/// Most triangles per chunk over all layers (256 layers of the full 2,048-triangle grid).
pub const MAX_TERRAIN_TRIANGLES: usize = MAX_TERRAIN_LAYERS * 2048;

/// The vertices of one terrain chunk (`terrain-texturing.md` §6.1, §6.4), one entry per
/// vertex in every non-empty stream. Positions are in the chunk's model space (the draw's
/// transform places them). Empty `tangents`/`binormals` mean a zero frame (normal maps
/// then have no effect, §6.4); an empty coordinate set reads as (0, 0).
#[derive(Debug, Clone, Copy, Default)]
pub struct TerrainVertices<'a> {
    /// Positions (x east, y north, z up).
    pub positions: &'a [[f32; 3]],
    /// Vertex normals (unit; non-finite or zero become +Z).
    pub normals: &'a [[f32; 3]],
    /// Tangents **T** (§6.4).
    pub tangents: &'a [[f32; 3]],
    /// Binormals **B** = **N** × **T** (§6.4).
    pub binormals: &'a [[f32; 3]],
    /// Coordinate sets 0–3 (§6.1): 0 top-down (x + oₓ, y + oᵧ), 1 (x + oₓ, h),
    /// 2 (y + oᵧ, h), 3 the mask and light-map coordinates (x / 160, y / 160).
    pub sets: [&'a [[f32; 2]]; 4],
}

/// The coordinate set a layer's textures read (`terrain-texturing.md` §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerrainProjection {
    /// `projection` = 3: set 0, top-down.
    #[default]
    Xy,
    /// `projection` = 1: set 1, faces looking north or south.
    Xz,
    /// `projection` = 2: set 2, faces looking east or west.
    Yz,
}

impl TerrainProjection {
    fn set(self) -> u32 {
        match self {
            Self::Xy => 0,
            Self::Xz => 1,
            Self::Yz => 2,
        }
    }
}

/// A layer's texture transform (`terrain-texturing.md` §6.3): scale, then a
/// counter-clockwise rotation, then the translation (the offset curves evaluated at the
/// material clock by the caller).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainTransform {
    /// `u_scale` (material parameter 1001).
    pub u_scale: f32,
    /// `v_scale` (1002).
    pub v_scale: f32,
    /// `rotation` in degrees (1033).
    pub rotation_degrees: f32,
    /// (`u_offset`, `v_offset`) at the current time (1017, 1018).
    pub offset: [f32; 2],
}

impl Default for TerrainTransform {
    fn default() -> Self {
        Self {
            u_scale: 1.0,
            v_scale: 1.0,
            rotation_degrees: 0.0,
            offset: [0.0; 2],
        }
    }
}

impl TerrainTransform {
    fn sane(self) -> (f32, f32, f32, [f32; 2]) {
        let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
        (
            f(self.u_scale, 0.0),
            f(self.v_scale, 0.0),
            f(self.rotation_degrees, 0.0).to_radians(),
            self.offset.map(|o| f(o, 0.0)),
        )
    }

    /// The rows of the affine map (p, q) → (u, v): u = r₀·(p, q, 1), v = r₁·(p, q, 1)
    /// (§6.3; non-finite values count as 0).
    #[must_use]
    pub fn rows(&self) -> [[f32; 3]; 2] {
        let (us, vs, th, off) = self.sane();
        let (s, c) = th.sin_cos();
        [[c * us, -s * vs, off[0]], [s * us, c * vs, off[1]]]
    }

    /// The first two columns of the texture matrix, each normalised (§6.4): *c*₀ ∝
    /// (`u_scale` cos θ, `u_scale` sin θ), *c*₁ ∝ (−`v_scale` sin θ, `v_scale` cos θ);
    /// a zero column stays zero.
    #[must_use]
    pub fn normal_columns(&self) -> [[f32; 2]; 2] {
        let (us, vs, th, _) = self.sane();
        let (s, c) = th.sin_cos();
        let unit = |x: f32, y: f32| {
            let l = (x * x + y * y).sqrt();
            if l > 0.0 && l.is_finite() {
                [x / l, y / l]
            } else {
                [0.0, 0.0]
            }
        };
        [unit(us * c, us * s), unit(-vs * s, vs * c)]
    }
}

/// How a layer's opacity α is formed (`terrain-texturing.md` §7); *m* is the mask value,
/// *a* the diffuse texture's alpha.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TerrainOpacity {
    /// The first drawn layer: α = 1, no alpha test.
    #[default]
    Opaque,
    /// A later layer with a mask and `use_opacity_mask`:
    /// α = clamp(*a* · *m* + *m*² · `alpha_blend_max`, 0, 1).
    MaskedDiffuse {
        /// `alpha_blend_max` (material parameter 1203).
        alpha_blend_max: f32,
    },
    /// A later layer with a mask, without `use_opacity_mask`: α = *m*.
    Mask,
    /// A later layer without a mask, with `use_opacity_mask`: α = *a*.
    Diffuse,
    /// A later layer without either: α = 1 (still blended and alpha-tested).
    One,
}

impl TerrainOpacity {
    fn code(self) -> (u32, f32) {
        match self {
            Self::Opaque => (0, 0.0),
            Self::MaskedDiffuse { alpha_blend_max } => (
                1,
                if alpha_blend_max.is_finite() {
                    alpha_blend_max
                } else {
                    0.0
                },
            ),
            Self::Mask => (2, 0.0),
            Self::Diffuse => (3, 0.0),
            Self::One => (4, 0.0),
        }
    }

    /// α for mask value `m` and diffuse alpha `a` (both clamped to 0–1, NaN as 0): the
    /// CPU twin of the shader.
    #[must_use]
    pub fn opacity(self, m: f32, a: f32) -> f32 {
        let unit = |v: f32| if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        let (m, a) = (unit(m), unit(a));
        match self {
            Self::Opaque | Self::One => 1.0,
            Self::MaskedDiffuse { alpha_blend_max } => {
                let k = if alpha_blend_max.is_finite() {
                    alpha_blend_max
                } else {
                    0.0
                };
                unit(a * m + m * m * k)
            }
            Self::Mask => m,
            Self::Diffuse => a,
        }
    }
}

/// Where a layer's gloss *g* comes from (`terrain-texturing.md` §8.1, §9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerrainGloss {
    /// The diffuse texture's alpha (`ColorAndGlossTexSRT`).
    #[default]
    DiffuseAlpha,
    /// The normal texture's alpha (gloss-from-normal switch).
    NormalAlpha,
}

/// A layer's specular term (specular switch on, `terrain-texturing.md` §8.1, §9.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainSpecular {
    /// `specular_colour` (material 2001), stored-value RGB.
    pub color: [f32; 3],
    /// `specular_power` (material 1000); below 0.5 there is no highlight
    /// (`lighting.md` §5.8).
    pub power: f32,
    /// Gloss source.
    pub gloss: TerrainGloss,
}

/// The textures of one layer (`terrain-texturing.md` §8.2). `None`, an unknown or
/// destroyed id and a registered render target are not bound: a missing diffuse texture
/// draws white (§8, Tessera Decision), a missing normal, emissive or glow texture turns
/// its switch off (§8.1: gloss from the normal then falls back to the switches' rule), and
/// a layer whose mask is missing is drawn as if it had no mask (§5.1 Tessera Decision:
/// [`TerrainOpacity::MaskedDiffuse`] becomes [`TerrainOpacity::Diffuse`] and
/// [`TerrainOpacity::Mask`] becomes [`TerrainOpacity::One`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerrainLayerTextures {
    /// 3000 DiffuseMap (RGB albedo *C*, alpha: opacity or gloss).
    pub diffuse: Option<TextureId>,
    /// 3011 NormalMap (tangent space, RGB = XYZ · 0.5 + 0.5; alpha: gloss).
    pub normal: Option<TextureId>,
    /// 3018 EmissiveMap.
    pub emissive: Option<TextureId>,
    /// 3013 GlowMap.
    pub glow: Option<TextureId>,
    /// 3020 OpacityAlphaMap: the layer's mask, value in the **alpha** channel (upload an
    /// A8 surface as `tessera-formats` decodes it, (255, 255, 255, a), stored rows first:
    /// row 0 is the south edge, §5.2). Sampled at set 3 with clamp addressing.
    pub mask: Option<TextureId>,
}

/// One drawn layer of a chunk, bottom first (`terrain-texturing.md` §8).
#[derive(Debug, Clone, Copy, Default)]
pub struct TerrainLayer<'a> {
    /// The layer's triangles as vertex indices, counter-clockwise from the side that is
    /// drawn (back faces are culled).
    pub triangles: &'a [[u16; 3]],
    /// The coordinate set of the layer textures.
    pub projection: TerrainProjection,
    /// The texture transform.
    pub transform: TerrainTransform,
    /// The opacity rule (§7); [`TerrainOpacity::Opaque`] marks the first drawn layer.
    pub opacity: TerrainOpacity,
    /// The discard threshold τ: pixels with α ≤ τ are dropped (not used by
    /// [`TerrainOpacity::Opaque`]).
    pub alpha_test_ref: f32,
    /// `diffuse_scale`: material colour *M* (2000), stored-value RGB (alpha is 1).
    pub diffuse_color: [f32; 3],
    /// Specular term, `None` when the switch is off.
    pub specular: Option<TerrainSpecular>,
    /// `glow_colour` *G* (2014) for the emissive and glow terms.
    pub glow_color: [f32; 3],
    /// The layer's textures.
    pub textures: TerrainLayerTextures,
}

/// Input of [`crate::Renderer::upload_terrain_chunk`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TerrainChunkData<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// The chunk's vertices.
    pub vertices: TerrainVertices<'a>,
    /// The drawn layers in stored order (bottom first).
    pub layers: &'a [TerrainLayer<'a>],
    /// 3019 BakedLightingMap: the block's light map for this surface (RGB added to the
    /// light sum, §10), sampled at set 3 with clamp addressing; `None`: no baked light.
    pub light_map: Option<TextureId>,
}

/// Summary of a validated chunk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainChunkInfo {
    /// Vertex count.
    pub vertices: u32,
    /// Triangles over all layers.
    pub triangles: u32,
    /// Layer count.
    pub layers: u32,
    /// Sphere around the vertices (model space).
    pub sphere: BoundingSphere,
    /// Box around the vertices (model space; light selection, `lighting.md` §8.5).
    pub aabb: Aabb,
}

fn invalid(msg: impl Into<String>) -> RenderError {
    RenderError::InvalidMesh(format!("terrain chunk: {}", msg.into()))
}

impl TerrainChunkData<'_> {
    /// Check the chunk without a GPU: stream lengths, vertex and layer limits, finite
    /// positions, triangle indices in range, and the triangle total.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] for inconsistent data, [`RenderError::LimitExceeded`]
    /// beyond [`MAX_TERRAIN_VERTICES`], [`MAX_TERRAIN_LAYERS`] or
    /// [`MAX_TERRAIN_TRIANGLES`].
    pub fn validate(&self) -> Result<TerrainChunkInfo> {
        let v = &self.vertices;
        let n = v.positions.len();
        if n == 0 {
            return Err(invalid("no vertices"));
        }
        for (what, requested, max) in [
            ("terrain chunk vertices", n, MAX_TERRAIN_VERTICES),
            (
                "terrain chunk layers",
                self.layers.len(),
                MAX_TERRAIN_LAYERS,
            ),
        ] {
            if requested > max {
                return Err(RenderError::LimitExceeded {
                    what,
                    requested: requested as u64,
                    max: max as u64,
                });
            }
        }
        for (what, len) in [
            ("normals", v.normals.len()),
            ("tangents", v.tangents.len()),
            ("binormals", v.binormals.len()),
            ("set 0", v.sets[0].len()),
            ("set 1", v.sets[1].len()),
            ("set 2", v.sets[2].len()),
            ("set 3", v.sets[3].len()),
        ] {
            if len != 0 && len != n {
                return Err(invalid(format!("{what}: {len} entries for {n} vertices")));
            }
        }
        if v.normals.is_empty() {
            return Err(invalid("no normals"));
        }
        if v.tangents.len() != v.binormals.len() {
            return Err(invalid(
                "tangents and binormals must both be given or both empty",
            ));
        }
        if let Some(i) = v
            .positions
            .iter()
            .position(|p| !p.iter().all(|c| c.is_finite()))
        {
            return Err(invalid(format!("vertex {i}: non-finite position")));
        }
        let mut triangles = 0usize;
        for (li, layer) in self.layers.iter().enumerate() {
            triangles = triangles.saturating_add(layer.triangles.len());
            if triangles > MAX_TERRAIN_TRIANGLES {
                return Err(RenderError::LimitExceeded {
                    what: "terrain chunk triangles",
                    requested: triangles as u64,
                    max: MAX_TERRAIN_TRIANGLES as u64,
                });
            }
            if let Some(t) = layer
                .triangles
                .iter()
                .position(|t| t.iter().any(|&i| usize::from(i) >= n))
            {
                return Err(invalid(format!(
                    "layer {li}, triangle {t}: vertex index out of range ({n} vertices)"
                )));
            }
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for p in v.positions {
            let p = Vec3::from_array(*p);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let center = (lo + hi) * 0.5;
        let radius = v
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p).distance(center))
            .fold(0.0f32, f32::max);
        Ok(TerrainChunkInfo {
            vertices: n as u32,
            triangles: triangles as u32,
            layers: self.layers.len() as u32,
            sphere: BoundingSphere { center, radius },
            aabb: Aabb { min: lo, max: hi },
        })
    }
}

/// A terrain chunk on the GPU ([`crate::Renderer::upload_terrain_chunk`]). Cheap to clone
/// (shared handle). A texture it was built with that is later destroyed
/// ([`crate::Renderer::destroy_texture`]) is dropped from its layers when the chunk is next
/// drawn in colour: the layer then draws as if that texture were missing.
#[derive(Clone)]
pub struct TerrainChunk(Arc<ChunkGpu>);

impl fmt::Debug for TerrainChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerrainChunk")
            .field("label", &self.0.label)
            .field("info", &self.0.info)
            .finish_non_exhaustive()
    }
}

impl TerrainChunk {
    /// Summary of the chunk.
    #[must_use]
    pub fn info(&self) -> TerrainChunkInfo {
        self.0.info
    }
}

/// One terrain chunk drawn in a pass ([`Pass::terrain`]).
#[derive(Debug, Clone)]
pub struct TerrainDraw {
    /// The chunk.
    pub chunk: TerrainChunk,
    /// Model → world (normally a translation to the landblock's origin).
    pub transform: Mat4,
    /// Lights from the pass's table. [`LightSelection::Auto`] selects by the chunk's world
    /// box and leaves out lights flagged as baked into the landscape
    /// ([`crate::LightReach::baked_landscape`], `lighting.md` §8.3, §8.5).
    pub lights: LightSelection,
    /// The chunk casts the sun's shadow.
    pub cast_shadow: bool,
    /// New layer translations (`u_offset`, `v_offset` at the current time, §6.3): one per
    /// layer of the chunk, in stored order (a layer that does not scroll repeats its
    /// offset). They replace the stored offsets for this and later frames (the last value
    /// written in a frame applies to every pass of it); only changed layers are written. A
    /// list of another length is applied to the first layers it covers (logged). `None`:
    /// unchanged.
    pub layer_offsets: Option<Arc<[[f32; 2]]>>,
}

impl TerrainDraw {
    /// Draw `chunk` at `transform` with automatic lights, casting shadows.
    #[must_use]
    pub fn new(chunk: TerrainChunk, transform: Mat4) -> Self {
        Self {
            chunk,
            transform,
            lights: LightSelection::Auto,
            cast_shadow: true,
            layer_offsets: None,
        }
    }
}

impl Pass {
    /// Draw a terrain chunk (`docs/specs/render.md` §5.2). Needs a lit pass
    /// ([`Pass::frame_lighting`]); ignored by screen passes.
    pub fn terrain(&mut self, draw: TerrainDraw) -> &mut Self {
        self.world.terrain.push(draw);
        self
    }
}

impl Renderer<'_> {
    /// Upload a terrain chunk: its vertices, the triangles of all its layers and one bind
    /// group per layer (`docs/specs/render.md` §5.2). Textures are resolved now (unknown
    /// ids and registered render targets are treated as absent and logged).
    ///
    /// # Errors
    ///
    /// As [`TerrainChunkData::validate`], and [`RenderError::Gpu`] when the GPU rejects
    /// the buffers.
    pub fn upload_terrain_chunk(&mut self, data: &TerrainChunkData<'_>) -> Result<TerrainChunk> {
        let info = data.validate()?;
        let up = self.world_upload();
        let device = up.device;
        let layouts = up.terrain.layouts(device);
        let (chunk, gpu_error) = crate::renderer::with_error_scope(device, || {
            build_chunk(
                device,
                &layouts,
                data,
                info,
                up.owner,
                &up.textures.white(),
                |id| up.textures.get(id),
            )
        });
        if let Some(e) = gpu_error {
            return Err(RenderError::Gpu(e));
        }
        tracing::trace!(label = data.label, ?info, "uploaded terrain chunk");
        Ok(TerrainChunk(Arc::new(chunk?)))
    }
}

// ---------------------------------------------------------------------------------------
// GPU side
// ---------------------------------------------------------------------------------------

/// Terrain vertex (must match `TerrainIn` in `terrain_layers.wgsl`). 80 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct TerrainVertexGpu {
    position: [f32; 3],
    normal: [f32; 3],
    tangent: [f32; 3],
    binormal: [f32; 3],
    sets: [[f32; 2]; 4],
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3,
    4 => Float32x2, 5 => Float32x2, 6 => Float32x2, 7 => Float32x2
];

/// Layer uniform (must match `TerrainLayer` in `terrain_layers.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LayerUniform {
    /// rgb = M, w = alpha_blend_max.
    diffuse: [f32; 4],
    /// rgb = Ms, w = power (< 0: no specular).
    specular: [f32; 4],
    /// rgb = G, w = τ.
    glow: [f32; 4],
    /// u = x·p + y·q + z.
    uv_u: [f32; 4],
    /// v = x·p + y·q + z.
    uv_v: [f32; 4],
    /// c₀.xy, c₁.xy.
    normal_cols: [f32; 4],
    /// x = coordinate set, y = opacity rule, z = flags (`TL_*`), w = sRGB bits.
    params: [u32; 4],
}

mod tl {
    pub const NORMAL_MAP: u32 = 1;
    pub const SPECULAR: u32 = 2;
    pub const GLOSS_NORMAL: u32 = 4;
    pub const BAKED: u32 = 8;
    pub const EMISSIVE: u32 = 16;
    pub const GLOW: u32 = 32;
    pub const FIRST: u32 = 64;
    pub const MASK: u32 = 128;
}

/// Per-frame draw uniform (must match `TerrainDraw` in `terrain_layers.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct DrawUniform {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
    /// x, y = light indices, z = count.
    lights: [u32; 4],
}

impl LayerUniform {
    fn new(layer: &TerrainLayer<'_>, present: Present, srgb: [bool; 6], baked: bool) -> Self {
        let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let rgb = |c: [f32; 3], d: f32| c.map(|v| f(v, d));
        // A mask that did not resolve: drawn as if the layer had no mask (§5.1 Tessera
        // Decision, §7).
        let opacity = match layer.opacity {
            TerrainOpacity::MaskedDiffuse { .. } if !present.mask => TerrainOpacity::Diffuse,
            TerrainOpacity::Mask if !present.mask => TerrainOpacity::One,
            o => o,
        };
        let (rule, abm) = opacity.code();
        let first = opacity == TerrainOpacity::Opaque;
        // §8.1: the opacity switch is on when the diffuse alpha is the opacity.
        let opacity_switch = matches!(
            opacity,
            TerrainOpacity::MaskedDiffuse { .. } | TerrainOpacity::Diffuse
        );
        let mut flags = 0;
        if first {
            flags |= tl::FIRST;
        }
        if present.mask {
            flags |= tl::MASK;
        }
        if present.normal {
            flags |= tl::NORMAL_MAP;
        }
        if present.emissive {
            flags |= tl::EMISSIVE;
        }
        if present.glow {
            flags |= tl::GLOW;
        }
        if baked {
            flags |= tl::BAKED;
        }
        // §8.1: gloss from the normal needs the normal-map switch (the texture loaded; the
        // tangent frame plays no part), and the specular switch is on only when gloss comes
        // from the normal or the opacity switch is off.
        let (spec, power) = match layer.specular {
            Some(s) if f(s.power, 0.0) >= 0.5 => {
                let gloss_normal = s.gloss == TerrainGloss::NormalAlpha && present.normal_texture;
                if gloss_normal || !opacity_switch {
                    flags |= tl::SPECULAR;
                    if gloss_normal {
                        flags |= tl::GLOSS_NORMAL;
                    }
                    (rgb(s.color, 0.0), s.power.min(4096.0))
                } else {
                    ([0.0; 3], -1.0)
                }
            }
            _ => ([0.0; 3], -1.0),
        };
        let m = rgb(layer.diffuse_color, 1.0);
        let g = rgb(layer.glow_color, 0.0);
        let tau = if first {
            0.0
        } else {
            f(layer.alpha_test_ref, 0.0).max(0.0)
        };
        let [r0, r1] = layer.transform.rows();
        let [c0, c1] = layer.transform.normal_columns();
        let srgb_bits = srgb
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &s)| acc | (u32::from(s) << i));
        Self {
            diffuse: [m[0], m[1], m[2], abm],
            specular: [spec[0], spec[1], spec[2], power],
            glow: [g[0], g[1], g[2], tau],
            uv_u: [r0[0], r0[1], r0[2], 0.0],
            uv_v: [r1[0], r1[1], r1[2], 0.0],
            normal_cols: [c0[0], c0[1], c1[0], c1[1]],
            params: [layer.projection.set(), rule, flags, srgb_bits],
        }
    }

    fn set_transform(&mut self, t: TerrainTransform) {
        let [r0, r1] = t.rows();
        self.uv_u = [r0[0], r0[1], r0[2], 0.0];
        self.uv_v = [r1[0], r1[1], r1[2], 0.0];
    }
}

/// Which optional textures of a layer resolved.
#[derive(Debug, Clone, Copy, Default)]
struct Present {
    /// The normal map perturbs the normal: its texture resolved and the chunk has a
    /// tangent frame.
    normal: bool,
    /// The normal texture resolved (gloss from its alpha, §8.1).
    normal_texture: bool,
    emissive: bool,
    glow: bool,
    mask: bool,
}

/// The mutable state of a layer: its uniform as last written (scrolled offsets included),
/// its bind group, and the textures that bind group holds (diffuse, normal, emissive, glow,
/// mask, light map; only ids that resolved).
struct LayerLive {
    uniform: LayerUniform,
    bind_group: wgpu::BindGroup,
    textures: [Option<TextureId>; 6],
}

struct LayerGpu {
    indices: Range<u32>,
    /// The layer as uploaded, without its triangles (for rebuilding its uniform).
    spec: TerrainLayer<'static>,
    live: Mutex<LayerLive>,
}

impl LayerGpu {
    fn live(&self) -> std::sync::MutexGuard<'_, LayerLive> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The distinct triangles of a chunk's layer index list (`indices`: whole triangles, every
/// layer's back to back), each once in first-seen order, or `None` when no triangle repeats.
/// Two triangles are the same when they use the same three vertices in any order (the
/// shadow pass culls no faces).
fn shadow_triangles(indices: &[u16]) -> Option<Vec<u16>> {
    let triangles = indices.len() / 3;
    let mut seen = std::collections::HashSet::with_capacity(triangles);
    let mut unique = Vec::with_capacity(indices.len());
    let mut repeats = false;
    for t in indices.as_chunks::<3>().0 {
        let mut key = [t[0], t[1], t[2]];
        key.sort_unstable();
        if seen.insert(key) {
            unique.extend_from_slice(t);
        } else {
            repeats = true;
        }
    }
    repeats.then_some(unique)
}

/// What a terrain sampler samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplerUse {
    /// The layer textures (diffuse, normal, emissive, glow): wrap addressing, filtered
    /// between mip levels (`terrain-texturing.md` §6.3).
    Layer,
    /// Masks and the light map: clamp-to-edge addressing and bilinear filtering
    /// (`terrain-texturing.md` §5.2, §10): linear within a level, the nearest mip level
    /// (no blend between levels).
    BlockMap,
}

fn sampler_descriptor(kind: SamplerUse) -> wgpu::SamplerDescriptor<'static> {
    let (label, address, mipmap_filter) = match kind {
        SamplerUse::Layer => (
            "tessera-terrain-layer-sampler",
            wgpu::AddressMode::Repeat,
            wgpu::MipmapFilterMode::Linear,
        ),
        SamplerUse::BlockMap => (
            "tessera-terrain-block-sampler",
            wgpu::AddressMode::ClampToEdge,
            wgpu::MipmapFilterMode::Nearest,
        ),
    };
    wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter,
        ..Default::default()
    }
}

struct ChunkGpu {
    owner: u64,
    label: Option<String>,
    info: TerrainChunkInfo,
    /// The chunk has tangent streams.
    has_frame: bool,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// The index range the shadow pass draws: every covered triangle once (render.md §5.2
    /// Shadows; see [`shadow_triangles`]).
    shadow_indices: Range<u32>,
    layer_buffer: wgpu::Buffer,
    layer_stride: u64,
    layers: Vec<LayerGpu>,
}

fn build_chunk(
    device: &wgpu::Device,
    layouts: &TerrainLayouts,
    data: &TerrainChunkData<'_>,
    info: TerrainChunkInfo,
    owner: u64,
    white: &ResolvedTexture,
    resolve: impl Fn(Option<TextureId>) -> Option<ResolvedTexture>,
) -> Result<ChunkGpu> {
    let v = &data.vertices;
    let n = v.positions.len();
    let get3 = |s: &[[f32; 3]], i: usize, d: [f32; 3]| {
        s.get(i)
            .copied()
            .filter(|c| c.iter().all(|x| x.is_finite()))
            .unwrap_or(d)
    };
    let vertices: Vec<TerrainVertexGpu> = (0..n)
        .map(|i| {
            let normal = get3(v.normals, i, [0.0, 0.0, 1.0]);
            let normal = if normal == [0.0; 3] {
                [0.0, 0.0, 1.0]
            } else {
                normal
            };
            TerrainVertexGpu {
                position: v.positions[i],
                normal,
                tangent: get3(v.tangents, i, [0.0; 3]),
                binormal: get3(v.binormals, i, [0.0; 3]),
                sets: std::array::from_fn(|s| {
                    v.sets[s]
                        .get(i)
                        .copied()
                        .filter(|c| c.iter().all(|x| x.is_finite()))
                        .unwrap_or([0.0; 2])
                }),
            }
        })
        .collect();
    let mut indices: Vec<u16> = Vec::with_capacity(info.triangles as usize * 3);
    let mut ranges = Vec::with_capacity(data.layers.len());
    for layer in data.layers {
        let start = indices.len() as u32;
        indices.extend(layer.triangles.iter().flatten());
        ranges.push(start..indices.len() as u32);
    }
    let index_count = indices.len() as u32;
    // The shadow pass draws each covered triangle once: the layers' triangles overlap
    // (every layer that covers a triangle lists it), so their whole list would rasterise a
    // triangle once per layer. The distinct triangles follow the layers' lists when there
    // are repeats; otherwise the shadow pass draws the layers' list itself.
    let shadow_indices = match shadow_triangles(&indices) {
        Some(unique) => {
            let start = indices.len() as u32;
            indices.extend(unique);
            start..indices.len() as u32
        }
        None => 0..index_count,
    };
    // A 4-byte multiple for the copy.
    if indices.len() % 2 == 1 {
        indices.push(0);
    }
    if indices.is_empty() {
        indices.extend([0, 0]);
    }
    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-terrain-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-terrain-indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let light = resolve(data.light_map);
    if data.light_map.is_some() && light.is_none() {
        tracing::warn!(
            label = data.label,
            "terrain light map unknown; no baked light"
        );
    }
    let has_frame = !v.tangents.is_empty();
    let mut warned = false;
    let mut sets = Vec::with_capacity(data.layers.len());
    for layer in data.layers {
        let t = &layer.textures;
        let mut get = |id: Option<TextureId>| {
            let r = resolve(id);
            if id.is_some() && r.is_none() && !warned {
                tracing::warn!(
                    ?id,
                    label = data.label,
                    "terrain layer texture unknown; unbound"
                );
                warned = true;
            }
            r
        };
        sets.push([
            get(t.diffuse),
            get(t.normal),
            get(t.emissive),
            get(t.glow),
            get(t.mask),
            light.clone(),
        ]);
    }
    let uniforms: Vec<LayerUniform> = data
        .layers
        .iter()
        .zip(&sets)
        .map(|(layer, set)| layer_uniform(layer, set, has_frame))
        .collect();
    let stride = uniform_stride(device, size_of::<LayerUniform>());
    let mut bytes = vec![0u8; (stride as usize) * uniforms.len().max(1)];
    for (i, u) in uniforms.iter().enumerate() {
        let off = i * stride as usize;
        bytes[off..off + size_of::<LayerUniform>()].copy_from_slice(bytemuck::bytes_of(u));
    }
    let layer_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-terrain-layers"),
        contents: &bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let mut layers = Vec::with_capacity(data.layers.len());
    for (i, ((layer, set), uniform)) in data.layers.iter().zip(&sets).zip(uniforms).enumerate() {
        let bind_group = layer_bind_group(
            device,
            layouts,
            &layer_buffer,
            i as u64 * stride,
            set,
            white,
        );
        let t = &layer.textures;
        let ids = [
            t.diffuse,
            t.normal,
            t.emissive,
            t.glow,
            t.mask,
            data.light_map,
        ];
        layers.push(LayerGpu {
            indices: ranges[i].clone(),
            spec: TerrainLayer {
                triangles: &[],
                ..*layer
            },
            live: Mutex::new(LayerLive {
                uniform,
                bind_group,
                textures: std::array::from_fn(|k| ids[k].filter(|_| set[k].is_some())),
            }),
        });
    }
    Ok(ChunkGpu {
        owner,
        label: data.label.map(str::to_owned),
        info,
        has_frame,
        vertices: vbuf,
        indices: ibuf,
        shadow_indices,
        layer_buffer,
        layer_stride: stride,
        layers,
    })
}

/// The uniform of a layer whose textures resolved as `set` (diffuse, normal, emissive,
/// glow, mask, light map).
fn layer_uniform(
    layer: &TerrainLayer<'_>,
    set: &[Option<ResolvedTexture>; 6],
    has_frame: bool,
) -> LayerUniform {
    let present = Present {
        normal: set[1].is_some() && has_frame,
        normal_texture: set[1].is_some(),
        emissive: set[2].is_some(),
        glow: set[3].is_some(),
        mask: set[4].is_some(),
    };
    let srgb = std::array::from_fn(|k| set[k].as_ref().is_some_and(|r| r.srgb));
    LayerUniform::new(layer, present, srgb, set[5].is_some())
}

/// A layer's bind group: its uniform slot at `offset` and its textures (white where
/// missing).
fn layer_bind_group(
    device: &wgpu::Device,
    layouts: &TerrainLayouts,
    buffer: &wgpu::Buffer,
    offset: u64,
    set: &[Option<ResolvedTexture>; 6],
    white: &ResolvedTexture,
) -> wgpu::BindGroup {
    let view = |k: usize| set[k].as_ref().map_or(&white.view, |r| &r.view);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-terrain-layer"),
        layout: &layouts.layer,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset,
                    size: wgpu::BufferSize::new(size_of::<LayerUniform>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view(0)),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(view(1)),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(view(2)),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(view(3)),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(view(4)),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(view(5)),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(&layouts.repeat),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&layouts.clamp),
            },
        ],
    })
}

impl ChunkGpu {
    /// Rebuild the layers that hold a texture destroyed since they were built: that texture
    /// is then treated as missing (its switch off), the layer's current offsets kept. A
    /// destroyed texture may not be bound: the GPU would reject the whole submission.
    fn refresh(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layouts: &TerrainLayouts,
        textures: &TextureResolver<'_>,
    ) {
        for (i, layer) in self.layers.iter().enumerate() {
            let mut live = layer.live();
            let alive = |id: &Option<TextureId>| id.is_none() || textures.get(*id).is_some();
            if live.textures.iter().all(alive) {
                continue;
            }
            tracing::debug!(
                label = self.label,
                layer = i,
                "terrain texture destroyed; layer rebuilt without it"
            );
            let set: [Option<ResolvedTexture>; 6] =
                std::array::from_fn(|k| live.textures[k].and_then(|id| textures.get(Some(id))));
            let mut uniform = layer_uniform(&layer.spec, &set, self.has_frame);
            uniform.uv_u = live.uniform.uv_u;
            uniform.uv_v = live.uniform.uv_v;
            let offset = i as u64 * self.layer_stride;
            queue.write_buffer(&self.layer_buffer, offset, bytemuck::bytes_of(&uniform));
            live.uniform = uniform;
            live.bind_group = layer_bind_group(
                device,
                layouts,
                &self.layer_buffer,
                offset,
                &set,
                &textures.white(),
            );
            live.textures = std::array::from_fn(|k| live.textures[k].filter(|_| set[k].is_some()));
        }
    }

    /// Apply new layer offsets (one per layer, stored order): the changed layers' uniforms
    /// are written with one buffer write covering them. Returns whether the list's length
    /// matched the layer count.
    fn set_offsets(&self, queue: &wgpu::Queue, offsets: &[[f32; 2]]) -> bool {
        let mut changed: Option<(usize, usize)> = None;
        for (i, (layer, off)) in self.layers.iter().zip(offsets).enumerate() {
            let mut live = layer.live();
            let mut u = live.uniform;
            u.set_transform(TerrainTransform {
                offset: *off,
                ..layer.spec.transform
            });
            if u.uv_u != live.uniform.uv_u || u.uv_v != live.uniform.uv_v {
                live.uniform = u;
                changed = Some(changed.map_or((i, i), |(a, _)| (a, i)));
            }
        }
        if let Some((first, last)) = changed {
            let stride = self.layer_stride as usize;
            let mut bytes = vec![0u8; (last - first) * stride + size_of::<LayerUniform>()];
            for (k, layer) in self.layers[first..=last].iter().enumerate() {
                let u = layer.live().uniform;
                bytes[k * stride..k * stride + size_of::<LayerUniform>()]
                    .copy_from_slice(bytemuck::bytes_of(&u));
            }
            queue.write_buffer(&self.layer_buffer, (first * stride) as u64, &bytes);
        }
        offsets.len() == self.layers.len()
    }
}

/// Layouts, samplers and the white fallback texture of the terrain pipeline.
#[derive(Debug)]
pub(crate) struct TerrainLayouts {
    draw: wgpu::BindGroupLayout,
    layer: wgpu::BindGroupLayout,
    /// Layer textures: bilinear with mipmaps, wrap (§6.3).
    repeat: wgpu::Sampler,
    /// Masks and light maps: bilinear, clamp to edge (§5.2, §10).
    clamp: wgpu::Sampler,
}

/// Pipelines of the terrain layers, built lazily.
#[derive(Debug, Default)]
pub(crate) struct TerrainState {
    layouts: Option<Arc<TerrainLayouts>>,
    shader: Option<wgpu::ShaderModule>,
    colour: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    shadow: Option<wgpu::RenderPipeline>,
}

impl TerrainState {
    fn layouts(&mut self, device: &wgpu::Device) -> Arc<TerrainLayouts> {
        if let Some(l) = &self.layouts {
            return l.clone();
        }
        let draw = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-terrain-draw-bgl"),
            entries: &[uniform_entry(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                true,
                size_of::<DrawUniform>(),
            )],
        });
        let mut entries = vec![uniform_entry(
            0,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            false,
            size_of::<LayerUniform>(),
        )];
        entries.extend((1..=6).map(texture_entry));
        entries.push(sampler_entry(7));
        entries.push(sampler_entry(8));
        let layer = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-terrain-layer-bgl"),
            entries: &entries,
        });
        let repeat = device.create_sampler(&sampler_descriptor(SamplerUse::Layer));
        let clamp = device.create_sampler(&sampler_descriptor(SamplerUse::BlockMap));
        let l = Arc::new(TerrainLayouts {
            draw,
            layer,
            repeat,
            clamp,
        });
        self.layouts = Some(l.clone());
        l
    }

    fn pipelines(
        &mut self,
        ctx: &WorldFrame<'_>,
    ) -> (
        Arc<TerrainLayouts>,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    ) {
        let device = ctx.device;
        let layouts = self.layouts(device);
        let shader = self
            .shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-terrain-layers.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(terrain_source().into()),
                })
            })
            .clone();
        let vertex = wgpu::VertexBufferLayout {
            array_stride: size_of::<TerrainVertexGpu>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRS,
        };
        let colour = match self.colour.iter().find(|(f, _)| *f == ctx.format) {
            Some((_, p)) => p.clone(),
            None => {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-terrain-layout"),
                    bind_group_layouts: &[
                        Some(ctx.frame_layout),
                        Some(&layouts.draw),
                        Some(&layouts.layer),
                    ],
                    immediate_size: 0,
                });
                let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("tessera-terrain-layers"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_terrain"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[Some(vertex.clone())],
                    },
                    primitive: wgpu::PrimitiveState {
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(world_depth(true)),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_terrain"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        // "Over": source α, destination 1 − α (§8 step 4).
                        targets: &[Some(wgpu::ColorTargetState {
                            format: ctx.format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.colour.push((ctx.format, p.clone()));
                p
            }
        };
        let shadow = self
            .shadow
            .get_or_insert_with(|| {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-terrain-shadow-layout"),
                    bind_group_layouts: &[Some(ctx.shadow_frame_layout), Some(&layouts.draw)],
                    immediate_size: 0,
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("tessera-terrain-shadow"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_terrain_shadow"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[Some(vertex.clone())],
                    },
                    primitive: wgpu::PrimitiveState {
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(shadow_depth()),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: None,
                    multiview_mask: None,
                    cache: None,
                })
            })
            .clone();
        (layouts, colour, shadow)
    }
}

pub(crate) fn terrain_source() -> String {
    lit_source(&[TERRAIN_WGSL])
}

// ---------------------------------------------------------------------------------------
// Per-frame hook
// ---------------------------------------------------------------------------------------

struct Item {
    chunk: TerrainChunk,
    offset: u32,
    visible: bool,
    cast: bool,
}

struct TerrainHook {
    items: Vec<Item>,
    draw_bg: wgpu::BindGroup,
    colour: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    culled: u32,
}

impl fmt::Debug for TerrainHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerrainHook")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

fn normal_matrix(m: Mat4) -> [[f32; 4]; 3] {
    let m3 = Mat3::from_mat4(m);
    let det = m3.determinant();
    let n = if det.is_finite() && det.abs() > 1e-20 {
        // Scaled to determinant ±1, so a uniform scale keeps the stored normal's length,
        // which the shading uses as it is (`terrain-texturing.md` §4.10).
        m3.inverse().transpose() * det.abs().cbrt()
    } else {
        Mat3::IDENTITY
    };
    [
        n.x_axis.extend(0.0).to_array(),
        n.y_axis.extend(0.0).to_array(),
        n.z_axis.extend(0.0).to_array(),
    ]
}

pub(crate) fn build_hook(
    state: &mut TerrainState,
    draws: Vec<TerrainDraw>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    if draws.is_empty() {
        return Ok(None);
    }
    let (layouts, colour, shadow) = state.pipelines(ctx);
    let frustum = Frustum::new(ctx.view_proj);
    let stride = uniform_stride(ctx.device, size_of::<DrawUniform>());
    // One draw-uniform slot per queued draw at most: checked up front, so the outcome does
    // not depend on the view (the device's buffer limit, render.md §5.2).
    let max_bytes = max_uniform_bytes(ctx.device);
    let requested = (draws.len() as u64).saturating_mul(stride);
    if requested > max_bytes {
        return Err(RenderError::LimitExceeded {
            what: "terrain draw uniform bytes per pass",
            requested,
            max: max_bytes,
        });
    }
    let casts = ctx.frame.has_shadow();
    let mut data: Vec<u8> = Vec::with_capacity(draws.len() * stride as usize);
    let mut items = Vec::with_capacity(draws.len());
    let mut chosen = Vec::with_capacity(MAX_DRAW_LIGHTS);
    let mut culled = 0u32;
    let mut warned = false;
    let mut warned_offsets = false;
    for draw in draws {
        let chunk = &draw.chunk.0;
        if chunk.owner != ctx.owner {
            if !warned {
                tracing::warn!("terrain chunk of another renderer skipped");
                warned = true;
            }
            continue;
        }
        let transform = if draw.transform.is_finite() {
            draw.transform
        } else {
            Mat4::IDENTITY
        };
        if let Some(offsets) = &draw.layer_offsets
            && !chunk.set_offsets(ctx.queue, offsets)
            && !warned_offsets
        {
            tracing::warn!(
                label = chunk.label,
                offsets = offsets.len(),
                layers = chunk.layers.len(),
                "terrain layer offsets do not match the layer count; applied to the first layers"
            );
            warned_offsets = true;
        }
        let sphere = chunk.info.sphere.transformed(transform);
        let visible = frustum.sphere_visible(sphere.center, sphere.radius);
        let cast = draw.cast_shadow && casts;
        if !visible {
            culled = culled.saturating_add(1);
            if !cast {
                continue;
            }
        } else {
            chunk.refresh(ctx.device, ctx.queue, &layouts, &ctx.textures);
        }
        // Terrain and ceiling chunks are landscape objects: baked lights skip them
        // (`lighting.md` §8.3, §8.5 step 3, L30).
        let target = LightTarget::placed(chunk.info.aabb, transform, None).landscape();
        ctx.lights.select(draw.lights, &target, &mut chosen);
        let lw = light_words(&chosen);
        let u = DrawUniform {
            model: transform.to_cols_array_2d(),
            normal: normal_matrix(transform),
            lights: [lw[0], lw[1], lw[2], 0],
        };
        let offset = u32::try_from(data.len()).map_err(|_| RenderError::LimitExceeded {
            what: "terrain draw uniform bytes per pass",
            requested: data.len() as u64,
            max: u64::from(u32::MAX),
        })?;
        data.extend_from_slice(bytemuck::bytes_of(&u));
        data.resize(offset as usize + stride as usize, 0);
        items.push(Item {
            chunk: draw.chunk.clone(),
            offset,
            visible,
            cast,
        });
    }
    if items.is_empty() {
        return Ok(None);
    }
    let buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-terrain-draws"),
            contents: &data,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let draw_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-terrain-draws"),
        layout: &layouts.draw,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<DrawUniform>() as u64),
            }),
        }],
    });
    Ok(Some(Arc::new(TerrainHook {
        items,
        draw_bg,
        colour,
        shadow,
        culled,
    })))
}

impl PassHook for TerrainHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        match ctx.stage {
            HookStage::Translucent | HookStage::Post => {}
            HookStage::Shadow => {
                let mut bound = false;
                for item in self.items.iter().filter(|i| i.cast) {
                    let chunk = &item.chunk.0;
                    if chunk.shadow_indices.is_empty() {
                        continue;
                    }
                    if !bound {
                        rp.set_pipeline(&self.shadow);
                        rp.set_bind_group(0, group0, &[offset]);
                        bound = true;
                    }
                    rp.set_bind_group(1, &self.draw_bg, &[item.offset]);
                    rp.set_vertex_buffer(0, chunk.vertices.slice(..));
                    rp.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint16);
                    rp.draw_indexed(chunk.shadow_indices.clone(), 0, 0..1);
                    stats.draw_calls = stats.draw_calls.saturating_add(1);
                    stats.shadow_draws = stats.shadow_draws.saturating_add(1);
                }
            }
            HookStage::Opaque => {
                stats.terrain_chunks_culled =
                    stats.terrain_chunks_culled.saturating_add(self.culled);
                rp.set_pipeline(&self.colour);
                rp.set_bind_group(0, group0, &[offset]);
                for item in self.items.iter().filter(|i| i.visible) {
                    let chunk = &item.chunk.0;
                    rp.set_bind_group(1, &self.draw_bg, &[item.offset]);
                    rp.set_vertex_buffer(0, chunk.vertices.slice(..));
                    rp.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint16);
                    for layer in &chunk.layers {
                        if layer.indices.is_empty() {
                            continue;
                        }
                        rp.set_bind_group(2, &layer.live().bind_group, &[]);
                        rp.draw_indexed(layer.indices.clone(), 0, 0..1);
                        stats.draw_calls = stats.draw_calls.saturating_add(1);
                        stats.terrain_layers = stats.terrain_layers.saturating_add(1);
                        stats.triangles = stats
                            .triangles
                            .saturating_add(u64::from(layer.indices.len() as u32 / 3));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_shader_validates() {
        let src = terrain_source();
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    }

    /// The shadow pass draws each covered triangle once, not once per layer covering it.
    #[test]
    fn shadow_draws_each_triangle_once() {
        // Three layers: the first covers both triangles, the second one of them (in another
        // vertex order), the third the other one.
        let layers: [&[u16]; 3] = [&[0, 1, 2, 2, 1, 3], &[1, 2, 0], &[2, 1, 3]];
        let all: Vec<u16> = layers.concat();
        assert_eq!(shadow_triangles(&all), Some(vec![0, 1, 2, 2, 1, 3]));
        // No repeat: the layers' own list serves.
        assert_eq!(shadow_triangles(&[0, 1, 2, 2, 1, 3]), None);
        assert_eq!(shadow_triangles(&[]), None);
        // A large chunk where 256 layers each list every triangle: 1 copy, not 256.
        let one: Vec<u16> = (0..1000u16).flat_map(|i| [i, i + 1, i + 2]).collect();
        let many: Vec<u16> = (0..256).flat_map(|_| one.iter().copied()).collect();
        assert_eq!(shadow_triangles(&many), Some(one));
    }

    /// Masks and light maps: clamp-to-edge and bilinear, no blend between mip levels
    /// (`terrain-texturing.md` §5.2, §10); layer textures: wrap, between mip levels (§6.3).
    #[test]
    fn samplers_follow_the_spec() {
        let b = sampler_descriptor(SamplerUse::BlockMap);
        assert_eq!(b.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(b.address_mode_v, wgpu::AddressMode::ClampToEdge);
        assert_eq!(b.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(b.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(b.mipmap_filter, wgpu::MipmapFilterMode::Nearest);
        let l = sampler_descriptor(SamplerUse::Layer);
        assert_eq!(l.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(l.mipmap_filter, wgpu::MipmapFilterMode::Linear);
    }

    #[test]
    fn layouts_match_the_wgsl() {
        assert_eq!(size_of::<TerrainVertexGpu>(), 80);
        assert_eq!(size_of::<LayerUniform>(), 112);
        assert_eq!(size_of::<DrawUniform>(), 128);
    }

    #[test]
    fn transform_follows_section_6_3() {
        // Worked example §14, key 1: scale 0.2, rotation 90°, (p, q) = (492.5, 1123).
        let t = TerrainTransform {
            u_scale: 0.2,
            v_scale: 0.2,
            rotation_degrees: 90.0,
            offset: [0.0; 2],
        };
        let [r0, r1] = t.rows();
        let (p, q) = (492.5f32, 1123.0f32);
        let u = r0[0] * p + r0[1] * q + r0[2];
        let v = r1[0] * p + r1[1] * q + r1[2];
        assert!(
            (u + 224.6).abs() < 1e-2 && (v - 98.5).abs() < 1e-2,
            "{u} {v}"
        );
        // Translation is added last.
        let t2 = TerrainTransform {
            offset: [0.5, -0.25],
            ..t
        };
        let [s0, s1] = t2.rows();
        assert_eq!(s0[2], 0.5);
        assert_eq!(s1[2], -0.25);
        // Hostile values become 0.
        let bad = TerrainTransform {
            u_scale: f32::NAN,
            v_scale: f32::INFINITY,
            rotation_degrees: f32::NAN,
            offset: [f32::NAN; 2],
        };
        assert!(bad.rows().iter().flatten().all(|v| *v == 0.0 || *v == -0.0));
    }

    #[test]
    fn normal_columns_follow_section_6_4() {
        let t = TerrainTransform {
            u_scale: 0.1,
            v_scale: -0.3,
            rotation_degrees: 90.0,
            offset: [5.0, 5.0],
        };
        let [c0, c1] = t.normal_columns();
        // c0 ∝ (us cos θ, us sin θ) = (0, 0.1) → (0, 1); c1 ∝ (−vs sin θ, vs cos θ) =
        // (0.3, 0) → (1, 0): the negative scale flips the matching axis.
        assert!((c0[0]).abs() < 1e-6 && (c0[1] - 1.0).abs() < 1e-6, "{c0:?}");
        assert!((c1[0] - 1.0).abs() < 1e-6 && c1[1].abs() < 1e-6, "{c1:?}");
        let zero = TerrainTransform { u_scale: 0.0, ..t };
        assert_eq!(zero.normal_columns()[0], [0.0, 0.0]);
    }

    #[test]
    fn opacity_follows_section_7() {
        // Worked example §14: m = 0.8, a = 0.5, alpha_blend_max 0.25 → 0.56.
        let r = TerrainOpacity::MaskedDiffuse {
            alpha_blend_max: 0.25,
        };
        assert!((r.opacity(0.8, 0.5) - 0.56).abs() < 1e-6);
        assert_eq!(TerrainOpacity::Mask.opacity(0.8, 0.5), 0.8);
        assert_eq!(TerrainOpacity::Diffuse.opacity(0.8, 0.5), 0.5);
        assert_eq!(TerrainOpacity::One.opacity(0.0, 0.0), 1.0);
        assert_eq!(TerrainOpacity::Opaque.opacity(0.0, 0.0), 1.0);
        assert_eq!(TerrainOpacity::Mask.opacity(f32::NAN, 0.0), 0.0);
        assert_eq!(
            TerrainOpacity::MaskedDiffuse {
                alpha_blend_max: 9.0
            }
            .opacity(1.0, 1.0),
            1.0
        );
    }

    type Grid = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[u16; 3]>);

    fn grid(n: usize) -> Grid {
        let mut p = Vec::new();
        for j in 0..n {
            for i in 0..n {
                p.push([i as f32, j as f32, 0.0]);
            }
        }
        let normals = vec![[0.0, 0.0, 1.0]; p.len()];
        let mut t = Vec::new();
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                let a = (j * n + i) as u16;
                t.push([a, a + 1, a + n as u16 + 1]);
                t.push([a, a + n as u16 + 1, a + n as u16]);
            }
        }
        (p, normals, t)
    }

    #[test]
    fn validation_rejects_hostile_chunks() {
        let (p, n, t) = grid(3);
        let layer = TerrainLayer {
            triangles: &t,
            ..TerrainLayer::default()
        };
        let layers = [layer];
        let ok = TerrainChunkData {
            vertices: TerrainVertices {
                positions: &p,
                normals: &n,
                ..TerrainVertices::default()
            },
            layers: &layers,
            ..TerrainChunkData::default()
        };
        let info = ok.validate().expect("valid");
        assert_eq!((info.vertices, info.triangles, info.layers), (9, 8, 1));
        assert!((info.sphere.center - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-6);

        let bad_index = [[0u16, 1, 9]];
        let layers = [TerrainLayer {
            triangles: &bad_index,
            ..TerrainLayer::default()
        }];
        assert!(matches!(
            TerrainChunkData {
                layers: &layers,
                ..ok
            }
            .validate(),
            Err(RenderError::InvalidMesh(_))
        ));
        let short = &n[..4];
        let v = TerrainVertices {
            normals: short,
            ..ok.vertices
        };
        assert!(TerrainChunkData { vertices: v, ..ok }.validate().is_err());
        let mut nan = p.clone();
        nan[3][1] = f32::NAN;
        let v = TerrainVertices {
            positions: &nan,
            ..ok.vertices
        };
        assert!(TerrainChunkData { vertices: v, ..ok }.validate().is_err());
        let tangents = vec![[1.0, 0.0, 0.0]; 9];
        let v = TerrainVertices {
            tangents: &tangents,
            ..ok.vertices
        };
        assert!(TerrainChunkData { vertices: v, ..ok }.validate().is_err());
        let too_many = vec![TerrainLayer::default(); MAX_TERRAIN_LAYERS + 1];
        assert!(matches!(
            TerrainChunkData {
                layers: &too_many,
                ..ok
            }
            .validate(),
            Err(RenderError::LimitExceeded { .. })
        ));
        assert!(TerrainChunkData::default().validate().is_err());
    }
}
