// SPDX-License-Identifier: GPL-3.0-or-later
//! The far-view pipelines (`docs/specs/far-view.md` §6, §9): far terrain from
//! distant-landscape sectors and distant sprites (imposters).
//!
//! **Far terrain.** A sector batch is uploaded once with
//! [`Renderer::upload_far_terrain`] (sector-local positions, the stored normals, texture
//! coordinates, a triangle list whose tail holds the stitching triangles) and drawn with
//! [`Pass::far_terrain`] in a lit pass. Shading is the plain template of §6.3 per vertex:
//! light *Σ* = sat(*A* + *n*_up *H*) + *k* sat(*n* · *L*) *C*, fog, far-draw fade *a*; per
//! pixel *c* = (colour + detail − 0.5) × *Σ*, output (1 − *f*) 2*c* + *f* × fog colour,
//! alpha *a*, discarded below 0.1. The colour list is sampled at (*u*, *v*) with clamp
//! addressing (parameters 4005 = 4006 = 0, no wrap), the detail texture at the draw's
//! repeat (4 × 4 per sector, §6.2) with wrap. With a normal map the draw follows the
//! normal-mapped template of §6.4 (normal strength, near-view mask, coverage, the sun per
//! pixel through the normal map in the tangent frame of FD5). Render state: SRCALPHA /
//! INVSRCALPHA, depth test "nearer or equal", depth write on, back faces culled for the grid
//! triangles; the stitching slivers are drawn without culling (implementer note in
//! `far-view.md` §12). No shadows reach the far terrain (§6.3).
//!
//! **Distant sprites.** A sector's placed sprites are uploaded once with
//! [`Renderer::upload_sprite_batch`] and drawn with [`Pass::distant_sprites`]: one
//! camera-facing quad per sprite built in the vertex shader (`misc-records.md` §5.2), the
//! atlas cell (mirrored *u* given by the caller, §5.4), the fade-in opacity of §9.3 times the
//! far-draw fade, an alpha test, lighting by the fake directional light, ambient and
//! hemispheric ambient through the normal atlas (§5.3), doubled, fogged (§9.6). Blended
//! SRCALPHA / INVSRCALPHA with depth writes; callers queue batches far to near (FD4).

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::error::{RenderError, Result};
use crate::frame::Pass;
use crate::instanced::{
    Frustum, lit_source, max_uniform_bytes, sampler_entry, texture_entry, uniform_entry,
    uniform_stride, world_depth,
};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::renderer::{FrameStats, Renderer, ResolvedTexture, WorldFrame};
use crate::texture::TextureId;

const FAR_TERRAIN_WGSL: &str = include_str!("shaders/far_terrain.wgsl");
const FAR_SPRITES_WGSL: &str = include_str!("shaders/far_sprites.wgsl");

/// Most vertices of one far-terrain batch (nine landblocks of 33 × 33 samples).
pub const MAX_FAR_TERRAIN_VERTICES: usize = 9 * 33 * 33;

/// Most indices of one far-terrain batch (nine LOD-0 landblocks and stitching).
pub const MAX_FAR_TERRAIN_INDICES: usize = 9 * (2_048 + 4 * 32 * 7) * 3;

/// Most sprites of one batch.
pub const MAX_SPRITES_PER_BATCH: usize = 1 << 16;

/// Most far-terrain and sprite draws per pass (each).
pub const MAX_FAR_DRAWS: usize = 4_096;

/// The alpha-test threshold of distant sprites when the caller sets none (the sprite
/// material's own value is not in the specs; Tessera's choice, `far-view.md` §12 note).
pub const DEFAULT_SPRITE_ALPHA_TEST: f32 = 0.5;

// ---------------------------------------------------------------------------------------
// Far terrain: public types
// ---------------------------------------------------------------------------------------

/// The vertices and triangles of one far-terrain batch, in its model space (sector-local
/// units; the draw's transform places it).
#[derive(Debug, Clone, Copy, Default)]
pub struct FarTerrainData<'a> {
    /// Debug label.
    pub label: &'a str,
    /// Positions.
    pub positions: &'a [[f32; 3]],
    /// Stored normals (not renormalised).
    pub normals: &'a [[f32; 3]],
    /// Texture coordinates of the colour list.
    pub uvs: &'a [[f32; 2]],
    /// Triangle list.
    pub indices: &'a [u32],
    /// First index of the stitching triangles (the rest are grid triangles).
    pub stitch_start: usize,
}

impl FarTerrainData<'_> {
    /// Checks the streams.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] for streams of different lengths, an index out of range,
    /// a partial triangle, `stitch_start` not on a triangle boundary, or a non-finite
    /// position; [`RenderError::LimitExceeded`] above [`MAX_FAR_TERRAIN_VERTICES`] or
    /// [`MAX_FAR_TERRAIN_INDICES`].
    pub fn validate(&self) -> Result<()> {
        let n = self.positions.len();
        if n > MAX_FAR_TERRAIN_VERTICES {
            return Err(RenderError::LimitExceeded {
                what: "far-terrain vertices",
                requested: n as u64,
                max: MAX_FAR_TERRAIN_VERTICES as u64,
            });
        }
        if self.indices.len() > MAX_FAR_TERRAIN_INDICES {
            return Err(RenderError::LimitExceeded {
                what: "far-terrain indices",
                requested: self.indices.len() as u64,
                max: MAX_FAR_TERRAIN_INDICES as u64,
            });
        }
        let invalid =
            |m: &str| RenderError::InvalidMesh(format!("far terrain {}: {m}", self.label));
        if self.normals.len() != n || self.uvs.len() != n {
            return Err(invalid("stream lengths differ"));
        }
        if !self.indices.len().is_multiple_of(3) || !self.stitch_start.is_multiple_of(3) {
            return Err(invalid("not a triangle list"));
        }
        if self.stitch_start > self.indices.len() {
            return Err(invalid("stitch start past the indices"));
        }
        if self.indices.iter().any(|&i| i as usize >= n) {
            return Err(invalid("index out of range"));
        }
        if self.positions.iter().flatten().any(|v| !v.is_finite()) {
            return Err(invalid("non-finite position"));
        }
        Ok(())
    }
}

struct FarMeshGpu {
    label: String,
    owner: u64,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    grid: std::ops::Range<u32>,
    stitch: std::ops::Range<u32>,
    center: Vec3,
    radius: f32,
}

/// An uploaded far-terrain batch (shared handle).
#[derive(Clone)]
pub struct FarTerrainMesh(Arc<FarMeshGpu>);

impl fmt::Debug for FarTerrainMesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FarTerrainMesh")
            .field("label", &self.0.label)
            .field("triangles", &((self.0.stitch.end) / 3))
            .finish_non_exhaustive()
    }
}

impl FarTerrainMesh {
    /// Triangles (grid and stitching).
    #[must_use]
    pub fn triangles(&self) -> u32 {
        self.0.stitch.end / 3
    }

    /// Bounding sphere in model space (centre, radius).
    #[must_use]
    pub fn bounds(&self) -> (Vec3, f32) {
        (self.0.center, self.0.radius)
    }
}

/// The far-draw fade (`far-view.md` §6.3 step 3): *a* = 1 − sat((*d* − `start`) ×
/// `inv_range`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarFadeParams {
    /// Start distance *s*.
    pub start: f32,
    /// Inverse range *r* (0: no fade).
    pub inv_range: f32,
}

impl Default for FarFadeParams {
    fn default() -> Self {
        Self {
            start: f32::MAX,
            inv_range: 0.0,
        }
    }
}

/// One far-terrain batch drawn in a pass ([`Pass::far_terrain`]).
#[derive(Debug, Clone)]
pub struct FarTerrainDraw {
    /// The batch.
    pub mesh: FarTerrainMesh,
    /// Model → world (a translation to the sector's south-west corner).
    pub transform: Mat4,
    /// The colour list (parameter 3000); `None` or unknown: white.
    pub colour: Option<TextureId>,
    /// The detail texture (parameter 3001); `None` or unknown: mid grey (no change).
    pub detail: Option<TextureId>,
    /// Detail (and normal) repeats per unit of (*u*, *v*): 4 × 4 (§6.2).
    pub detail_repeats: [f32; 2],
    /// The far-draw fade.
    pub fade: FarFadeParams,
    /// The normal-mapped template's normal texture (3011); `None`: the plain template.
    pub normal_map: Option<TextureId>,
    /// `c_NearViewOrigin` in world (render) space for the near-view mask of §6.4
    /// (Inferred: the viewer landblock's south-west corner); `None`: mask 1 everywhere.
    pub near_view_origin: Option<[f32; 2]>,
}

impl FarTerrainDraw {
    /// A plain-template draw of `mesh` at `transform` with 4 × 4 detail repeats, no fade.
    #[must_use]
    pub fn new(mesh: FarTerrainMesh, transform: Mat4) -> Self {
        Self {
            mesh,
            transform,
            colour: None,
            detail: None,
            detail_repeats: [4.0, 4.0],
            fade: FarFadeParams::default(),
            normal_map: None,
            near_view_origin: None,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Sprites: public types
// ---------------------------------------------------------------------------------------

/// One distant sprite (`far-view.md` §9, `misc-records.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DistantSprite {
    /// Anchor in the batch's model space.
    pub position: [f32; 3],
    /// Quad width and height *S*.
    pub size: f32,
    /// Height of the lower edge above the anchor *B*.
    pub base: f32,
    /// (left, right) *u* (already mirrored for a mirrored sprite).
    pub u: [f32; 2],
    /// (top, bottom) *v*.
    pub v: [f32; 2],
    /// Fade-in distance parameter *D*.
    pub distance: f32,
    /// Mirrored (the normal atlas's R axis is negated).
    pub mirror: bool,
}

struct SpriteBatchGpu {
    label: String,
    owner: u64,
    instances: wgpu::Buffer,
    count: u32,
    center: Vec3,
    radius: f32,
}

/// An uploaded sprite batch (shared handle).
#[derive(Clone)]
pub struct SpriteBatch(Arc<SpriteBatchGpu>);

impl fmt::Debug for SpriteBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpriteBatch")
            .field("label", &self.0.label)
            .field("sprites", &self.0.count)
            .finish_non_exhaustive()
    }
}

impl SpriteBatch {
    /// Sprites in the batch.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.0.count
    }

    /// Whether the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.count == 0
    }
}

/// One sprite batch drawn in a pass ([`Pass::distant_sprites`]).
#[derive(Debug, Clone)]
pub struct SpriteDraw {
    /// The batch.
    pub batch: SpriteBatch,
    /// Model → world (a translation).
    pub transform: Mat4,
    /// The colour atlas (3000); `None` or unknown: nothing is drawn.
    pub colour_atlas: Option<TextureId>,
    /// The normal atlas (3011); `None`: flat sprites lit along *U*.
    pub normal_atlas: Option<TextureId>,
    /// The distance scale *c* of `c_DistantSpriteConfig` (crowd factor × *M_d*, §9.3).
    pub distance_scale: f32,
    /// The far-draw fade.
    pub fade: FarFadeParams,
    /// Alpha-test threshold.
    pub alpha_test: f32,
}

impl SpriteDraw {
    /// A draw of `batch` at `transform` with atlas `colour_atlas`, *c* = 1, no fade.
    #[must_use]
    pub fn new(batch: SpriteBatch, transform: Mat4, colour_atlas: Option<TextureId>) -> Self {
        Self {
            batch,
            transform,
            colour_atlas,
            normal_atlas: None,
            distance_scale: 1.0,
            fade: FarFadeParams::default(),
            alpha_test: DEFAULT_SPRITE_ALPHA_TEST,
        }
    }
}

impl Pass {
    /// Draw a far-terrain batch (`far-view.md` §6). Needs a lit pass; ignored by screen
    /// passes.
    pub fn far_terrain(&mut self, draw: FarTerrainDraw) -> &mut Self {
        self.world.far_terrain.push(draw);
        self
    }

    /// Draw a batch of distant sprites (`far-view.md` §9). Needs a lit pass; ignored by
    /// screen passes. Batches draw in queue order: queue them far to near (FD4).
    pub fn distant_sprites(&mut self, draw: SpriteDraw) -> &mut Self {
        self.world.sprites.push(draw);
        self
    }
}

// ---------------------------------------------------------------------------------------
// Uploads
// ---------------------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FarVertexGpu {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

const FAR_VERTEX_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SpriteGpu {
    pos_size: [f32; 4],
    base_dist: [f32; 4],
    uv: [f32; 4],
}

const SPRITE_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];

fn sphere_of(points: impl Iterator<Item = (Vec3, f32)> + Clone) -> (Vec3, f32) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for (p, _) in points.clone() {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if lo.x > hi.x {
        return (Vec3::ZERO, 0.0);
    }
    let c = (lo + hi) * 0.5;
    let r = points
        .map(|(p, extra)| (p - c).length() + extra)
        .fold(0.0f32, f32::max);
    (c, r)
}

impl Renderer<'_> {
    /// Upload a far-terrain batch (`far-view.md` §5).
    ///
    /// # Errors
    ///
    /// As [`FarTerrainData::validate`]; [`RenderError::Gpu`] when the GPU rejects the
    /// buffers.
    pub fn upload_far_terrain(&mut self, data: &FarTerrainData<'_>) -> Result<FarTerrainMesh> {
        data.validate()?;
        let up = self.world_upload();
        let device = up.device;
        let owner = up.owner;
        let vertices: Vec<FarVertexGpu> = data
            .positions
            .iter()
            .zip(data.normals)
            .zip(data.uvs)
            .map(|((p, n), uv)| FarVertexGpu {
                position: *p,
                normal: n.map(|v| if v.is_finite() { v } else { 0.0 }),
                uv: uv.map(|v| if v.is_finite() { v } else { 0.0 }),
            })
            .collect();
        let (center, radius) = sphere_of(data.positions.iter().map(|p| (Vec3::from(*p), 0.0)));
        let total = u32::try_from(data.indices.len()).unwrap_or(u32::MAX);
        let stitch = u32::try_from(data.stitch_start).unwrap_or(total);
        // An empty index buffer is not allowed; keep one degenerate triangle.
        let indices: &[u32] = if data.indices.is_empty() {
            &[0, 0, 0]
        } else {
            data.indices
        };
        let zero = FarVertexGpu::zeroed();
        let (bufs, err) = crate::renderer::with_error_scope(device, || {
            let v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tessera-far-terrain-vertices"),
                contents: if vertices.is_empty() {
                    bytemuck::bytes_of(&zero)
                } else {
                    bytemuck::cast_slice(&vertices)
                },
                usage: wgpu::BufferUsages::VERTEX,
            });
            let i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tessera-far-terrain-indices"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (v, i)
        });
        if let Some(e) = err {
            return Err(RenderError::Gpu(e));
        }
        tracing::trace!(
            label = data.label,
            vertices = vertices.len(),
            "uploaded far terrain"
        );
        Ok(FarTerrainMesh(Arc::new(FarMeshGpu {
            label: data.label.to_owned(),
            owner,
            vertices: bufs.0,
            indices: bufs.1,
            grid: 0..stitch,
            stitch: stitch..total,
            center,
            radius,
        })))
    }

    /// Upload a batch of distant sprites (`far-view.md` §9). Sprites with a non-finite
    /// value or a non-positive size are dropped.
    ///
    /// # Errors
    ///
    /// [`RenderError::LimitExceeded`] above [`MAX_SPRITES_PER_BATCH`]; [`RenderError::Gpu`]
    /// when the GPU rejects the buffer.
    pub fn upload_sprite_batch(
        &mut self,
        label: &str,
        sprites: &[DistantSprite],
    ) -> Result<SpriteBatch> {
        if sprites.len() > MAX_SPRITES_PER_BATCH {
            return Err(RenderError::LimitExceeded {
                what: "distant sprites per batch",
                requested: sprites.len() as u64,
                max: MAX_SPRITES_PER_BATCH as u64,
            });
        }
        let gpu: Vec<SpriteGpu> = sprites
            .iter()
            .filter(|s| {
                s.position.iter().all(|v| v.is_finite())
                    && s.size.is_finite()
                    && s.size > 0.0
                    && s.base.is_finite()
                    && s.distance.is_finite()
                    && s.u.iter().chain(&s.v).all(|v| v.is_finite())
            })
            .map(|s| SpriteGpu {
                pos_size: [s.position[0], s.position[1], s.position[2], s.size],
                base_dist: [s.base, s.distance, if s.mirror { 1.0 } else { 0.0 }, 0.0],
                uv: [s.u[0], s.u[1], s.v[0], s.v[1]],
            })
            .collect();
        let (center, radius) = sphere_of(gpu.iter().map(|s| {
            (
                Vec3::new(s.pos_size[0], s.pos_size[1], s.pos_size[2]),
                s.pos_size[3] + s.base_dist[0].abs(),
            )
        }));
        let up = self.world_upload();
        let device = up.device;
        let owner = up.owner;
        let zero = SpriteGpu::zeroed();
        let (buf, err) = crate::renderer::with_error_scope(device, || {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tessera-distant-sprites"),
                contents: if gpu.is_empty() {
                    bytemuck::bytes_of(&zero)
                } else {
                    bytemuck::cast_slice(&gpu)
                },
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        if let Some(e) = err {
            return Err(RenderError::Gpu(e));
        }
        Ok(SpriteBatch(Arc::new(SpriteBatchGpu {
            label: label.to_owned(),
            owner,
            instances: buf,
            count: u32::try_from(gpu.len()).unwrap_or(u32::MAX),
            center,
            radius,
        })))
    }
}

// ---------------------------------------------------------------------------------------
// Pipelines
// ---------------------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FarDrawUniform {
    model: [[f32; 4]; 4],
    fade: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SpriteDrawUniform {
    model: [[f32; 4]; 4],
    params: [f32; 4],
    flags: [u32; 4],
}

/// Pipelines of the far view, built lazily.
#[derive(Debug, Default)]
pub(crate) struct FarViewState {
    terrain_layout: Option<wgpu::BindGroupLayout>,
    sprite_layout: Option<wgpu::BindGroupLayout>,
    samplers: Option<(wgpu::Sampler, wgpu::Sampler)>,
    terrain_shader: Option<wgpu::ShaderModule>,
    sprite_shader: Option<wgpu::ShaderModule>,
    /// (format, culled grid pipeline, unculled stitch pipeline).
    terrain: Vec<(
        wgpu::TextureFormat,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    )>,
    sprites: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    grey: Option<(wgpu::Texture, wgpu::TextureView)>,
}

pub(crate) fn far_terrain_source() -> String {
    lit_source(&[FAR_TERRAIN_WGSL])
}

pub(crate) fn far_sprites_source() -> String {
    lit_source(&[FAR_SPRITES_WGSL])
}

fn blend_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

impl FarViewState {
    fn samplers(&mut self, device: &wgpu::Device) -> (wgpu::Sampler, wgpu::Sampler) {
        self.samplers
            .get_or_insert_with(|| {
                let make = |a: wgpu::AddressMode| {
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("tessera-far-view-sampler"),
                        address_mode_u: a,
                        address_mode_v: a,
                        address_mode_w: a,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::MipmapFilterMode::Linear,
                        ..Default::default()
                    })
                };
                (
                    make(wgpu::AddressMode::ClampToEdge),
                    make(wgpu::AddressMode::Repeat),
                )
            })
            .clone()
    }

    /// A 1 × 1 mid-grey texture (128): the detail texture's neutral value, since
    /// colour + detail − 0.5 leaves the colour unchanged.
    fn grey(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        self.grey
            .get_or_insert_with(|| {
                let t = device.create_texture_with_data(
                    queue,
                    &wgpu::TextureDescriptor {
                        label: Some("tessera-far-view-grey"),
                        size: wgpu::Extent3d {
                            width: 1,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &[128, 128, 128, 255],
                );
                let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                (t, v)
            })
            .1
            .clone()
    }

    fn terrain_pipelines(
        &mut self,
        ctx: &WorldFrame<'_>,
    ) -> (
        wgpu::BindGroupLayout,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    ) {
        let device = ctx.device;
        let layout = self
            .terrain_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-far-terrain-bgl"),
                    entries: &[
                        uniform_entry(
                            0,
                            wgpu::ShaderStages::VERTEX_FRAGMENT,
                            true,
                            size_of::<FarDrawUniform>(),
                        ),
                        texture_entry(1),
                        texture_entry(2),
                        texture_entry(3),
                        sampler_entry(4),
                        sampler_entry(5),
                    ],
                })
            })
            .clone();
        if let Some((_, a, b)) = self.terrain.iter().find(|(f, _, _)| *f == ctx.format) {
            return (layout, a.clone(), b.clone());
        }
        let shader = self
            .terrain_shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-far-terrain.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(far_terrain_source().into()),
                })
            })
            .clone();
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-far-terrain-layout"),
            bind_group_layouts: &[Some(ctx.frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        let make = |cull: Option<wgpu::Face>, label| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_far"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<FarVertexGpu>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &FAR_VERTEX_ATTRS,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: cull,
                    ..Default::default()
                },
                depth_stencil: Some(world_depth(true)),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_far"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(blend_target(ctx.format))],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let grid = make(Some(wgpu::Face::Back), "tessera-far-terrain");
        let stitch = make(None, "tessera-far-terrain-stitch");
        self.terrain
            .push((ctx.format, grid.clone(), stitch.clone()));
        (layout, grid, stitch)
    }

    fn sprite_pipeline(
        &mut self,
        ctx: &WorldFrame<'_>,
    ) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
        let device = ctx.device;
        let layout = self
            .sprite_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-distant-sprites-bgl"),
                    entries: &[
                        uniform_entry(
                            0,
                            wgpu::ShaderStages::VERTEX_FRAGMENT,
                            true,
                            size_of::<SpriteDrawUniform>(),
                        ),
                        texture_entry(1),
                        texture_entry(2),
                        sampler_entry(3),
                    ],
                })
            })
            .clone();
        if let Some((_, p)) = self.sprites.iter().find(|(f, _)| *f == ctx.format) {
            return (layout, p.clone());
        }
        let shader = self
            .sprite_shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-far-sprites.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(far_sprites_source().into()),
                })
            })
            .clone();
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-distant-sprites-layout"),
            bind_group_layouts: &[Some(ctx.frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-distant-sprites"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sprite"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<SpriteGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &SPRITE_ATTRS,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(world_depth(true)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sprite"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(blend_target(ctx.format))],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.sprites.push((ctx.format, p.clone()));
        (layout, p)
    }
}

// ---------------------------------------------------------------------------------------
// Per-frame hook
// ---------------------------------------------------------------------------------------

struct TerrainItem {
    mesh: FarTerrainMesh,
    bind_group: wgpu::BindGroup,
    offset: u32,
}

struct SpriteItem {
    batch: SpriteBatch,
    bind_group: wgpu::BindGroup,
    offset: u32,
}

struct FarViewHook {
    grid: Option<wgpu::RenderPipeline>,
    stitch: Option<wgpu::RenderPipeline>,
    sprite: Option<wgpu::RenderPipeline>,
    terrain: Vec<TerrainItem>,
    sprites: Vec<SpriteItem>,
}

impl fmt::Debug for FarViewHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FarViewHook")
            .field("terrain", &self.terrain.len())
            .field("sprites", &self.sprites.len())
            .finish_non_exhaustive()
    }
}

fn srgb_bit(t: Option<&ResolvedTexture>, bit: u32) -> u32 {
    if t.is_some_and(|t| t.srgb) { bit } else { 0 }
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

fn uniform_buffer(device: &wgpu::Device, bytes: &[u8], label: &str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build_hook(
    state: &mut FarViewState,
    terrain: Vec<FarTerrainDraw>,
    sprites: Vec<SpriteDraw>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    if terrain.is_empty() && sprites.is_empty() {
        return Ok(None);
    }
    for (what, n) in [
        ("far-terrain draws per pass", terrain.len()),
        ("sprite draws per pass", sprites.len()),
    ] {
        if n > MAX_FAR_DRAWS {
            return Err(RenderError::LimitExceeded {
                what,
                requested: n as u64,
                max: MAX_FAR_DRAWS as u64,
            });
        }
    }
    let frustum = Frustum::new(ctx.view_proj);
    let (clamp, wrap) = state.samplers(ctx.device);
    let white = ctx.textures.white();
    let grey = state.grey(ctx.device, ctx.queue);
    let max_bytes = max_uniform_bytes(ctx.device);
    let mut hook = FarViewHook {
        grid: None,
        stitch: None,
        sprite: None,
        terrain: Vec::new(),
        sprites: Vec::new(),
    };
    let mut warned = false;

    // Far terrain.
    if !terrain.is_empty() {
        let (layout, grid, stitch) = state.terrain_pipelines(ctx);
        let stride = uniform_stride(ctx.device, size_of::<FarDrawUniform>());
        let requested = (terrain.len() as u64).saturating_mul(stride);
        if requested > max_bytes {
            return Err(RenderError::LimitExceeded {
                what: "far-terrain uniform bytes per pass",
                requested,
                max: max_bytes,
            });
        }
        let mut data: Vec<u8> = Vec::new();
        let mut chosen = Vec::new();
        for d in &terrain {
            let gpu = &d.mesh.0;
            if gpu.owner != ctx.owner {
                if !warned {
                    tracing::warn!("far terrain of another renderer skipped");
                    warned = true;
                }
                continue;
            }
            let tf = if d.transform.is_finite() {
                d.transform
            } else {
                Mat4::IDENTITY
            };
            let c = tf.transform_point3(gpu.center);
            let scale = tf.to_scale_rotation_translation().0.max_element().abs();
            if !frustum.sphere_visible(c, gpu.radius * scale.max(1e-6)) {
                continue;
            }
            let colour = ctx.textures.get(d.colour);
            let detail = ctx.textures.get(d.detail);
            let normal = ctx.textures.get(d.normal_map);
            let mut flags = 0u32;
            if normal.is_some() {
                flags |= 1;
                if d.near_view_origin.is_some() {
                    flags |= 2;
                }
            }
            let srgb = srgb_bit(colour.as_ref(), 1)
                | srgb_bit(detail.as_ref(), 2)
                | srgb_bit(normal.as_ref(), 4);
            let origin = d.near_view_origin.unwrap_or([0.0; 2]);
            #[allow(clippy::cast_precision_loss)]
            let u = FarDrawUniform {
                model: tf.to_cols_array_2d(),
                fade: [
                    finite_or(d.fade.start, f32::MAX),
                    finite_or(d.fade.inv_range, 0.0).max(0.0),
                    finite_or(origin[0], 0.0),
                    finite_or(origin[1], 0.0),
                ],
                params: [
                    finite_or(d.detail_repeats[0], 4.0),
                    finite_or(d.detail_repeats[1], 4.0),
                    flags as f32,
                    srgb as f32,
                ],
            };
            let offset = u32::try_from(data.len()).unwrap_or(u32::MAX);
            data.extend_from_slice(bytemuck::bytes_of(&u));
            data.resize(offset as usize + stride as usize, 0);
            chosen.push((d.mesh.clone(), offset, colour, detail, normal));
        }
        if !chosen.is_empty() {
            let buffer = uniform_buffer(ctx.device, &data, "tessera-far-terrain-draws");
            for (mesh, offset, colour, detail, normal) in chosen {
                let view = |t: &Option<ResolvedTexture>, fallback: &wgpu::TextureView| {
                    t.as_ref()
                        .map_or_else(|| fallback.clone(), |t| t.view.clone())
                };
                let cv = view(&colour, &white.view);
                let dv = view(&detail, &grey);
                let nv = view(&normal, &white.view);
                let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("tessera-far-terrain"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &buffer,
                                offset: 0,
                                size: wgpu::BufferSize::new(size_of::<FarDrawUniform>() as u64),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&cv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&dv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&nv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&clamp),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&wrap),
                        },
                    ],
                });
                hook.terrain.push(TerrainItem {
                    mesh,
                    bind_group,
                    offset,
                });
            }
            hook.grid = Some(grid);
            hook.stitch = Some(stitch);
        }
    }

    // Sprites.
    if !sprites.is_empty() {
        let (layout, pipeline) = state.sprite_pipeline(ctx);
        let stride = uniform_stride(ctx.device, size_of::<SpriteDrawUniform>());
        let requested = (sprites.len() as u64).saturating_mul(stride);
        if requested > max_bytes {
            return Err(RenderError::LimitExceeded {
                what: "sprite uniform bytes per pass",
                requested,
                max: max_bytes,
            });
        }
        let mut data: Vec<u8> = Vec::new();
        let mut chosen = Vec::new();
        for d in &sprites {
            let gpu = &d.batch.0;
            if gpu.owner != ctx.owner || gpu.count == 0 {
                continue;
            }
            let Some(atlas) = ctx.textures.get(d.colour_atlas) else {
                continue;
            };
            let tf = if d.transform.is_finite() {
                d.transform
            } else {
                Mat4::IDENTITY
            };
            let c = tf.transform_point3(gpu.center);
            if !frustum.sphere_visible(c, gpu.radius) {
                continue;
            }
            let normal = ctx.textures.get(d.normal_atlas);
            let u = SpriteDrawUniform {
                model: tf.to_cols_array_2d(),
                params: [
                    finite_or(d.distance_scale, 1.0),
                    finite_or(d.alpha_test, DEFAULT_SPRITE_ALPHA_TEST).clamp(0.0, 1.0),
                    finite_or(d.fade.start, f32::MAX),
                    finite_or(d.fade.inv_range, 0.0).max(0.0),
                ],
                flags: [
                    u32::from(normal.is_some()),
                    srgb_bit(Some(&atlas), 1) | srgb_bit(normal.as_ref(), 2),
                    0,
                    0,
                ],
            };
            let offset = u32::try_from(data.len()).unwrap_or(u32::MAX);
            data.extend_from_slice(bytemuck::bytes_of(&u));
            data.resize(offset as usize + stride as usize, 0);
            chosen.push((d.batch.clone(), offset, atlas, normal));
        }
        if !chosen.is_empty() {
            let buffer = uniform_buffer(ctx.device, &data, "tessera-distant-sprite-draws");
            for (batch, offset, atlas, normal) in chosen {
                let nv = normal
                    .as_ref()
                    .map_or_else(|| white.view.clone(), |t| t.view.clone());
                let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("tessera-distant-sprites"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &buffer,
                                offset: 0,
                                size: wgpu::BufferSize::new(size_of::<SpriteDrawUniform>() as u64),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&atlas.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&nv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&wrap),
                        },
                    ],
                });
                hook.sprites.push(SpriteItem {
                    batch,
                    bind_group,
                    offset,
                });
            }
            hook.sprite = Some(pipeline);
        }
    }
    if hook.terrain.is_empty() && hook.sprites.is_empty() {
        return Ok(None);
    }
    Ok(Some(Arc::new(hook)))
}

impl PassHook for FarViewHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Opaque {
            return;
        }
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        for (pipeline, stitch) in [(&self.grid, false), (&self.stitch, true)] {
            let Some(p) = pipeline else {
                continue;
            };
            rp.set_pipeline(p);
            rp.set_bind_group(0, group0, &[offset]);
            for item in &self.terrain {
                let gpu = &item.mesh.0;
                let range = if stitch {
                    gpu.stitch.clone()
                } else {
                    gpu.grid.clone()
                };
                if range.is_empty() {
                    continue;
                }
                rp.set_bind_group(1, &item.bind_group, &[item.offset]);
                rp.set_vertex_buffer(0, gpu.vertices.slice(..));
                rp.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(range, 0, 0..1);
                stats.draw_calls = stats.draw_calls.saturating_add(1);
            }
        }
        if let Some(p) = &self.sprite {
            rp.set_pipeline(p);
            rp.set_bind_group(0, group0, &[offset]);
            for item in &self.sprites {
                let gpu = &item.batch.0;
                rp.set_bind_group(1, &item.bind_group, &[item.offset]);
                rp.set_vertex_buffer(0, gpu.instances.slice(..));
                rp.draw(0..6, 0..gpu.count);
                stats.draw_calls = stats.draw_calls.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaders_validate() {
        for src in [far_terrain_source(), far_sprites_source()] {
            let module = wgpu::naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
            let mut v = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            );
            if let Err(e) = v.validate(&module) {
                panic!("{}", e.emit_to_string(&src));
            }
        }
    }

    #[test]
    fn uniforms_match_the_shader_sizes() {
        assert_eq!(size_of::<FarDrawUniform>(), 96);
        assert_eq!(size_of::<SpriteDrawUniform>(), 96);
        assert_eq!(size_of::<FarVertexGpu>(), 32);
        assert_eq!(size_of::<SpriteGpu>(), 48);
    }

    #[test]
    fn far_data_validation() {
        let p = [[0.0f32; 3]; 3];
        let n = [[0.0f32, 0.0, 1.0]; 3];
        let uv = [[0.0f32; 2]; 3];
        let ok = FarTerrainData {
            label: "t",
            positions: &p,
            normals: &n,
            uvs: &uv,
            indices: &[0, 1, 2],
            stitch_start: 3,
        };
        assert!(ok.validate().is_ok());
        assert!(
            FarTerrainData {
                indices: &[0, 1, 3],
                ..ok
            }
            .validate()
            .is_err()
        );
        assert!(
            FarTerrainData {
                stitch_start: 1,
                ..ok
            }
            .validate()
            .is_err()
        );
        assert!(FarTerrainData { uvs: &[], ..ok }.validate().is_err());
        let bad = [[f32::NAN, 0.0, 0.0]; 3];
        assert!(
            FarTerrainData {
                positions: &bad,
                ..ok
            }
            .validate()
            .is_err()
        );
    }
}
