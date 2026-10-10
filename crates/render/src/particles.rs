// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle draws (`docs/specs/particle-runtime.md` §9.1; `docs/specs/particles.md`
//! §8.11, §8.15).
//!
//! [`Pass::particles`] queues one [`ParticleDraw`]: the vertices of one draw instance of a
//! particle system in world space, quads first (four vertices each) and then triangles
//! (three each), in the order the simulation wrote them; the renderer does **no** sorting
//! of particles or of draws (the order inside a system is the storage order, between
//! systems the caller's order, `particles.md` §8.15). Each draw is at most two draw calls,
//! as in the original: the quads as an indexed triangle list (0, 1, 2) and (0, 2, 3) through
//! a shared quad index buffer, then the triangles as a plain list. Blend, alpha test and
//! depth write come from the draw's [`AlphaMode`], the state of the system's surface
//! material (`particle-runtime.md` §9.1); the colour multiplier is
//! [`ParticleDraw::tint`], normally (1, 1, 1, instance opacity). Billboards are drawn from
//! both sides with the depth test on.
//!
//! What the surface program computes is [`ParticleDraw::shading`] (`particle-runtime.md`
//! §9.1.1): *M* = [`ParticleDraw::material`] (colour 2000) × the tint; colour = *T* × *V* ×
//! *M*, doubled where the program doubles it, then fogged (`lighting.md` §10, in world
//! passes with fog) where the program is fogged; output alpha = *T*.alpha × *V*.alpha ×
//! *M*.alpha without the factors the program leaves out. The alpha test reads the
//! texture's opacity alone (two layers: the product of both alphas), with the per-draw
//! reference of `material.md` §11.2 and *A* = *M*.alpha ([`draw_reference`]). In two-layer
//! programs the first layer is read at texture matrix 0 and the second at texture matrix 1
//! ([`ParticleDraw::uv`], `material.md` §8.4); one-layer programs read the vertex UV as it
//! is. Particle draws use the permutations without feature bit 1 (`particle-runtime.md`
//! §9.1.2, rule 11): no soft-particle factor (parameter 1174), and the heat-distortion
//! program is not drawn (the caller leaves it out).
//!
//! Opacity (`particle-runtime.md` §9.1.3): the blend's source alpha is the program's
//! output alpha above, not the mesh opacity source of `render.md` §4.3, so instance
//! opacity, distance fade and the particle colour's alpha fade particles through the
//! blend, also for [`AlphaMode::Fixed`] states; the alpha test comes from the draw's
//! [`AlphaMode`], which the caller builds from the particle surface. Colour and alpha are
//! clamped to [0, 1] before blending. In a premultiplied pass the alpha rows
//! (SRCALPHA / INVSRCALPHA, ADD) blend alpha source-over, the additive rows keep the
//! destination alpha and other fixed blends follow `render.md` §6.10 (R12).
//!
//! Draws are recorded in a world pass's translucent stage, after the opaque surfaces and
//! the render-world hooks, before the sorted blended meshes. They work in lit and unlit
//! world passes and are ignored by screen passes.

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::error::{RenderError, Result};
use crate::frame::{Pass, Rgba};
use crate::instanced::{uniform_entry, uniform_stride};
use crate::material::{AlphaMode, FixedAlpha, FixedBlend, UvTransform};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::pipelines::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::renderer::{FrameStats, ResolvedTexture, TextureResolver};
use crate::texture::TextureId;

const PARTICLES_WGSL: &str = include_str!("shaders/particles.wgsl");

/// Vertices one particle draw may hold: the original's draw buffer of 32,768 vertices
/// (8,192 quads, `particles.md` §8.11). Longer draws are cut.
pub const MAX_PARTICLE_DRAW_VERTICES: usize = 32_768;
/// Particle draws per pass and frame.
pub const MAX_PARTICLE_DRAWS_PER_PASS: usize = 65_536;
/// Particle vertices per pass and frame (all draws together, 36 MiB of vertex data; the
/// device's `max_buffer_size` lowers it further). Draws that would pass it are skipped
/// whole, with a warning: the data stays far below it (the largest live count of one system
/// is about 1,300 particles), so only hostile input meets it.
pub const MAX_PARTICLE_VERTICES_PER_PASS: usize = 1 << 20;
/// Quads the shared index buffer covers.
const MAX_QUADS: usize = MAX_PARTICLE_DRAW_VERTICES / 4;

/// One particle vertex (world space).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct ParticleVertex {
    /// Position, metres.
    pub position: [f32; 3],
    /// Particle colour in stored-value space, 0–1.
    pub color: [f32; 4],
    /// Texture coordinates.
    pub uv: [f32; 2],
}

/// How a two-layer surface program combines its layers' colours (the alphas multiply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticleLayers {
    /// Layer 1 × layer 2.
    Product,
    /// Layer 1 + layer 2.
    Sum,
}

/// What a particle surface's program computes (`particle-runtime.md` §9.1.1).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticleShading {
    /// The alpha test (when the draw's [`AlphaMode`] has one, and the per-draw rule keeps
    /// it) and the discard of transparent texels in source-alpha blends read the texture's
    /// opacity; `false`: the program discards nothing.
    pub tests_texture: bool,
    /// The texture's alpha is a factor of the output alpha.
    pub texture_alpha: bool,
    /// The vertex colour is a factor of colour and alpha.
    pub vertex_colour: bool,
    /// *M* (material colour × tint) is a factor of colour and alpha.
    pub material_colour: bool,
    /// The colour is doubled.
    pub doubled: bool,
    /// The colour is fogged (world passes with fog only).
    pub fogged: bool,
    /// A second colour layer ([`ParticleDraw::second_texture`]) and how it combines.
    pub layers: Option<ParticleLayers>,
}

impl ParticleShading {
    /// The common program: *T* × *V* × *M*, alpha *T*.alpha × *V*.alpha × *M*.alpha, the
    /// test on *T*.alpha, one layer, not doubled, not fogged.
    pub const STANDARD: Self = Self {
        tests_texture: true,
        texture_alpha: true,
        vertex_colour: true,
        material_colour: true,
        doubled: false,
        fogged: false,
        layers: None,
    };

    fn program_bits(self) -> u32 {
        u32::from(self.texture_alpha)
            | u32::from(self.vertex_colour) << 1
            | u32::from(self.doubled) << 2
    }
}

impl Default for ParticleShading {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// One draw instance of a particle system ([`Pass::particles`]). `Default` is an empty
/// [`ParticleDraw::new`] (white material, standard program).
#[derive(Debug, Clone)]
pub struct ParticleDraw {
    /// Quad vertices (four per quad) followed by triangle vertices (three per triangle).
    pub vertices: Vec<ParticleVertex>,
    /// How many of `vertices` are quad vertices (rounded down to whole quads).
    pub quad_vertices: usize,
    /// The surface texture (white when `None` or unknown).
    pub texture: Option<TextureId>,
    /// Blend, alpha test and depth write of the surface material.
    pub alpha: AlphaMode,
    /// Colour multiplier: (1, 1, 1, instance opacity).
    pub tint: Rgba,
    /// The surface's material colour 2000 (white when not declared); *M* = this × `tint`.
    pub material: Rgba,
    /// What the surface program computes.
    pub shading: ParticleShading,
    /// The second layer's texture (white when `None` or unknown), read only when
    /// [`ParticleShading::layers`] is set.
    pub second_texture: Option<TextureId>,
    /// Texture matrices 0 and 1 (`material.md` §8.4, evaluated at the draw's clock): the
    /// first layer is read at `uv[0]` and the second at `uv[1]` when
    /// [`ParticleShading::layers`] is set (`particle-runtime.md` §9.1.1, RT27); one-layer
    /// programs ignore both and read the vertex UV.
    pub uv: [UvTransform; 2],
}

impl ParticleDraw {
    /// An alpha-blended, white-textured draw of `vertices` with `quad_vertices` quad
    /// vertices first.
    #[must_use]
    pub fn new(vertices: Vec<ParticleVertex>, quad_vertices: usize) -> Self {
        Self {
            vertices,
            quad_vertices,
            texture: None,
            alpha: AlphaMode::Blend,
            tint: [1.0; 4],
            material: [1.0; 4],
            shading: ParticleShading::STANDARD,
            second_texture: None,
            uv: [UvTransform::IDENTITY; 2],
        }
    }
}

impl Default for ParticleDraw {
    fn default() -> Self {
        Self::new(Vec::new(), 0)
    }
}

impl Pass {
    /// Queue a particle draw (see the module docs). Ignored by screen passes.
    pub fn particles(&mut self, draw: ParticleDraw) -> &mut Self {
        self.particles.push(draw);
        self
    }
}

/// The fixed-function state a draw's [`AlphaMode`] selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlendKey {
    Opaque,
    Alpha,
    Fixed(FixedBlend),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DrawState {
    blend: BlendKey,
    depth_write: bool,
    cutoff: Option<f32>,
}

impl DrawState {
    /// Whether the blend's source factor is source alpha (`src_blend` SRCALPHA).
    fn source_alpha(&self) -> bool {
        match self.blend {
            BlendKey::Alpha => true,
            BlendKey::Fixed(f) => f.src == crate::material::BlendFactor::SrcAlpha,
            BlendKey::Opaque => false,
        }
    }
}

/// The alpha-test reference of one particle draw (`material.md` §11.2 "What the client
/// does per draw", applied by `particle-runtime.md` §9.1.1 rule 1 with *A* = *M*.alpha =
/// material alpha × tint alpha). `test` is the record's reference when its alpha test is
/// on, `depth_write` and `source_alpha` the permutation's state. Tessera has no
/// alpha-to-coverage option, so step 1 never applies.
///
/// - Alpha test on and depth write on: the reference becomes reference × *A*.
/// - Alpha test on and depth write off: the test is off for the draw while *A* < 0.9.
/// - On: ⌊reference × 255⌋ / 255. Off: 0 when the source factor is source alpha (texels
///   of opacity 0 are discarded), else nothing is discarded (`None`).
///
/// A fragment is discarded when its tested opacity is ≤ the returned reference.
#[must_use]
pub fn draw_reference(
    test: Option<f32>,
    depth_write: bool,
    source_alpha: bool,
    a: f32,
) -> Option<f32> {
    let off = source_alpha.then_some(0.0);
    let Some(r) = test.filter(|r| r.is_finite()) else {
        return off;
    };
    let a = if a.is_finite() { a } else { 0.0 };
    let r = if depth_write {
        r * a
    } else if a < 0.9 {
        return off;
    } else {
        r
    };
    Some((r * 255.0).floor() / 255.0)
}

/// The distance fog of a world pass (`lighting.md` §10), for fogged particle programs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ParticleFog {
    /// The eye position.
    pub eye: [f32; 3],
    /// Fog near distance.
    pub near: f32,
    /// 1 / (far − near).
    pub inv_range: f32,
    /// Fog colour (stored-value space, as the lit passes use it).
    pub color: [f32; 3],
}

fn cutoff(c: f32) -> f32 {
    if c.is_finite() { c } else { 0.5 }
}

fn draw_state(alpha: AlphaMode) -> DrawState {
    match alpha {
        AlphaMode::Opaque => DrawState {
            blend: BlendKey::Opaque,
            depth_write: true,
            cutoff: None,
        },
        AlphaMode::Mask { cutoff: c } => DrawState {
            blend: BlendKey::Opaque,
            depth_write: true,
            cutoff: Some(cutoff(c)),
        },
        AlphaMode::Blend => DrawState {
            blend: BlendKey::Alpha,
            depth_write: false,
            cutoff: None,
        },
        AlphaMode::TestBlend { cutoff: c } => DrawState {
            blend: BlendKey::Alpha,
            depth_write: true,
            cutoff: (cutoff(c) >= 0.0).then(|| cutoff(c)),
        },
        AlphaMode::Additive => DrawState {
            blend: BlendKey::Fixed(FixedBlend::ADDITIVE),
            depth_write: false,
            cutoff: None,
        },
        // The record's alpha test (`material.md` §11.2) uses the draw's cut-off.
        AlphaMode::Fixed(f) => DrawState {
            blend: BlendKey::Fixed(f),
            depth_write: f.depth_write,
            cutoff: f.alpha_test.map(|t| t.value()),
        },
    }
}

/// Per-draw uniform (must match `ParticleDraw` in `particles.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ParticleUniform {
    view_proj: [[f32; 4]; 4],
    material: [f32; 4],
    params: [f32; 4],
    eye: [f32; 4],
    fog_color: [f32; 4],
    flags: [u32; 4],
    layers: [u32; 4],
    /// Texture matrices 0 and 1 packed by [`UvTransform::pack`].
    uv: [[f32; 4]; 3],
}

type PipeKey = (wgpu::TextureFormat, BlendKey, bool, FixedAlpha);

/// Pipelines and shared buffers of the particle path, built lazily.
#[derive(Debug, Default)]
pub(crate) struct ParticleState {
    uniform_layout: Option<wgpu::BindGroupLayout>,
    texture_layout: Option<wgpu::BindGroupLayout>,
    layout: Option<wgpu::PipelineLayout>,
    shader: Option<wgpu::ShaderModule>,
    quad_indices: Option<wgpu::Buffer>,
    pipelines: Vec<(PipeKey, wgpu::RenderPipeline)>,
}

/// What [`build_hook`] needs from the frame.
#[derive(Debug)]
pub(crate) struct ParticleFrame<'a> {
    pub device: &'a wgpu::Device,
    pub format: wgpu::TextureFormat,
    pub view_proj: Mat4,
    pub textures: TextureResolver<'a>,
    /// The pass has the premultiplied option (`Pass::premultiplied_alpha`): fixed blends
    /// write alpha by [`FixedBlend::alpha_rule`] (`docs/specs/render.md` §6.10, R12).
    pub premultiplied: bool,
    /// The pass's distance fog (lit world passes with fog on).
    pub fog: Option<ParticleFog>,
}

impl ParticleState {
    fn layouts(&mut self, device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let uniform = self
            .uniform_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-particles-draw-bgl"),
                    entries: &[uniform_entry(
                        0,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        true,
                        size_of::<ParticleUniform>(),
                    )],
                })
            })
            .clone();
        let texture = self
            .texture_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-particles-texture-bgl"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                })
            })
            .clone();
        (uniform, texture)
    }

    fn quad_indices(&mut self, device: &wgpu::Device) -> wgpu::Buffer {
        self.quad_indices
            .get_or_insert_with(|| {
                let indices: Vec<u32> = (0..MAX_QUADS as u32)
                    .flat_map(|q| {
                        let b = q * 4;
                        [b, b + 1, b + 2, b, b + 2, b + 3]
                    })
                    .collect();
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tessera-particles-quad-indices"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                })
            })
            .clone()
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        state: DrawState,
        premultiplied: bool,
    ) -> wgpu::RenderPipeline {
        let alpha = match state.blend {
            BlendKey::Fixed(f) => f.alpha_rule(premultiplied),
            BlendKey::Opaque | BlendKey::Alpha => FixedAlpha::Keep,
        };
        let key = (format, state.blend, state.depth_write, alpha);
        if let Some((_, p)) = self.pipelines.iter().find(|(k, _)| *k == key) {
            return p.clone();
        }
        let (uniform, texture) = self.layouts(device);
        let shader = self
            .shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-particles.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(PARTICLES_WGSL.into()),
                })
            })
            .clone();
        let layout = self
            .layout
            .get_or_insert_with(|| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-particles-layout"),
                    bind_group_layouts: &[Some(&uniform), Some(&texture)],
                    immediate_size: 0,
                })
            })
            .clone();
        // A fixed blend that draws opaque in a premultiplied pass (render.md §6.10, R12)
        // does not blend; the fragment applies `FIXED_OPAQUE`.
        let (blend, constants) = match state.blend {
            BlendKey::Opaque => (None, &[][..]),
            BlendKey::Alpha => (Some(wgpu::BlendState::ALPHA_BLENDING), &[][..]),
            BlendKey::Fixed(f) => {
                let p = f.pipeline(alpha);
                (p.blend, p.constants())
            }
        };
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32x2];
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-particles"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_particle"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ParticleVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(state.depth_write),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_particle"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((key, p.clone()));
        p
    }
}

#[derive(Debug)]
struct ParticleItem {
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::BindGroup,
    uniform_offset: u32,
    /// First vertex in the frame's vertex buffer.
    first: u64,
    quads: u32,
    triangles_vertices: u32,
}

struct ParticleHook {
    items: Vec<ParticleItem>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    uniforms: wgpu::BindGroup,
    vertex_count: u32,
}

impl fmt::Debug for ParticleHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParticleHook")
            .field("items", &self.items.len())
            .field("vertex_count", &self.vertex_count)
            .finish_non_exhaustive()
    }
}

fn finite_vertex(v: &ParticleVertex) -> ParticleVertex {
    let f = |x: f32| if x.is_finite() { x } else { 0.0 };
    ParticleVertex {
        position: v.position.map(f),
        color: v.color.map(f),
        uv: v.uv.map(f),
    }
}

/// Builds the hook of one world pass's particle draws.
pub(crate) fn build_hook(
    state: &mut ParticleState,
    draws: Vec<ParticleDraw>,
    ctx: &ParticleFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    if draws.is_empty() {
        return Ok(None);
    }
    if draws.len() > MAX_PARTICLE_DRAWS_PER_PASS {
        return Err(RenderError::LimitExceeded {
            what: "particle draws per pass",
            requested: draws.len() as u64,
            max: MAX_PARTICLE_DRAWS_PER_PASS as u64,
        });
    }
    let device = ctx.device;
    let (_, texture_layout) = state.layouts(device);
    let indices = state.quad_indices(device);
    let stride = uniform_stride(device, size_of::<ParticleUniform>());
    let mut uniform_data: Vec<u8> = Vec::new();
    let mut vertices: Vec<ParticleVertex> = Vec::new();
    let mut items = Vec::with_capacity(draws.len());
    let mut warned = false;
    let srgb_target = ctx.format.is_srgb();
    let vsize = size_of::<ParticleVertex>() as u64;
    let budget = usize::try_from(device.limits().max_buffer_size / vsize)
        .unwrap_or(usize::MAX)
        .min(MAX_PARTICLE_VERTICES_PER_PASS);
    let mut skipped = 0usize;
    for d in &draws {
        let n = d.vertices.len().min(MAX_PARTICLE_DRAW_VERTICES);
        if n < d.vertices.len() && !warned {
            tracing::warn!(
                vertices = d.vertices.len(),
                max = MAX_PARTICLE_DRAW_VERTICES,
                "particle draw cut to the draw-buffer size"
            );
            warned = true;
        }
        let quad_vertices = (d.quad_vertices.min(n) / 4) * 4;
        let quads = (quad_vertices / 4) as u32;
        let tri_vertices = ((n - quad_vertices) / 3 * 3) as u32;
        if quads == 0 && tri_vertices == 0 {
            continue;
        }
        if vertices.len() + quad_vertices + tri_vertices as usize > budget {
            skipped += 1;
            continue;
        }
        let finite = |c: f32| if c.is_finite() { c } else { 1.0 };
        let tint = d.tint.map(finite);
        let material = d.material.map(finite);
        let sh = d.shading;
        let st = draw_state(d.alpha);
        let reference = if sh.tests_texture {
            draw_reference(
                st.cutoff,
                st.depth_write,
                st.source_alpha(),
                material[3] * tint[3],
            )
        } else {
            None
        };
        let m = if sh.material_colour {
            [0, 1, 2, 3].map(|i| material[i] * tint[i])
        } else {
            [1.0; 4]
        };
        let resolved: ResolvedTexture = ctx
            .textures
            .get(d.texture)
            .unwrap_or_else(|| ctx.textures.white());
        let second: ResolvedTexture = match sh.layers {
            Some(_) => ctx.textures.get(d.second_texture),
            None => None,
        }
        .unwrap_or_else(|| ctx.textures.white());
        let fog = ctx.fog.filter(|_| sh.fogged);
        let uniform = ParticleUniform {
            view_proj: ctx.view_proj.to_cols_array_2d(),
            material: m,
            params: [
                reference.unwrap_or(0.0),
                fog.map_or(0.0, |f| f.near),
                fog.map_or(0.0, |f| f.inv_range),
                0.0,
            ],
            eye: fog.map_or([0.0; 4], |f| [f.eye[0], f.eye[1], f.eye[2], 1.0]),
            fog_color: fog.map_or([0.0; 4], |f| [f.color[0], f.color[1], f.color[2], 1.0]),
            flags: [
                u32::from(resolved.srgb),
                u32::from(srgb_target),
                u32::from(reference.is_some()),
                sh.program_bits(),
            ],
            layers: [
                u32::from(second.srgb),
                match sh.layers {
                    None => 0,
                    Some(ParticleLayers::Product) => 1,
                    Some(ParticleLayers::Sum) => 2,
                },
                0,
                0,
            ],
            uv: UvTransform::pack(match sh.layers {
                Some(_) => d.uv,
                None => [UvTransform::IDENTITY; 2],
            }),
        };
        let offset = u32::try_from(uniform_data.len()).map_err(|_| RenderError::LimitExceeded {
            what: "particle uniform bytes per pass",
            requested: uniform_data.len() as u64,
            max: u64::from(u32::MAX),
        })?;
        uniform_data.extend_from_slice(bytemuck::bytes_of(&uniform));
        uniform_data.resize(
            uniform_data.len() + (stride as usize - size_of::<ParticleUniform>()),
            0,
        );
        let texture = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessera-particles-texture"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolved.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&resolved.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&second.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&second.sampler),
                },
            ],
        });
        let first = vertices.len() as u64;
        vertices.extend(
            d.vertices
                .iter()
                .take(quad_vertices + tri_vertices as usize)
                .map(finite_vertex),
        );
        items.push(ParticleItem {
            pipeline: state.pipeline(device, ctx.format, st, ctx.premultiplied),
            texture,
            uniform_offset: offset,
            first,
            quads,
            triangles_vertices: tri_vertices,
        });
    }
    if skipped > 0 {
        tracing::warn!(
            skipped,
            budget,
            "particle draws skipped: the pass's vertex budget is spent"
        );
    }
    if items.is_empty() {
        return Ok(None);
    }
    let (uniform_layout, _) = state.layouts(device);
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-particles-uniforms"),
        contents: &uniform_data,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let uniforms = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-particles-uniforms"),
        layout: &uniform_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<ParticleUniform>() as u64),
            }),
        }],
    });
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-particles-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    Ok(Some(Arc::new(ParticleHook {
        items,
        vertices: vertex_buffer,
        indices,
        uniforms,
        vertex_count: u32::try_from(vertices.len()).unwrap_or(u32::MAX),
    })))
}

impl PassHook for ParticleHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Translucent {
            return;
        }
        let vsize = size_of::<ParticleVertex>() as u64;
        rp.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for item in &self.items {
            rp.set_pipeline(&item.pipeline);
            rp.set_bind_group(0, &self.uniforms, &[item.uniform_offset]);
            rp.set_bind_group(1, &item.texture, &[]);
            let start = item.first * vsize;
            let count = u64::from(item.quads) * 4 + u64::from(item.triangles_vertices);
            rp.set_vertex_buffer(0, self.vertices.slice(start..start + count * vsize));
            if item.quads > 0 {
                rp.draw_indexed(0..item.quads * 6, 0, 0..1);
                stats.draw_calls = stats.draw_calls.saturating_add(1);
            }
            if item.triangles_vertices > 0 {
                let base = item.quads * 4;
                rp.draw(base..base + item.triangles_vertices, 0..1);
                stats.draw_calls = stats.draw_calls.saturating_add(1);
            }
            stats.particle_draws = stats.particle_draws.saturating_add(1);
        }
        stats.particle_vertices = stats.particle_vertices.saturating_add(self.vertex_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(PARTICLES_WGSL)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(PARTICLES_WGSL)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(PARTICLES_WGSL)));
    }

    #[test]
    fn uniform_layout() {
        assert_eq!(size_of::<ParticleUniform>(), 208);
        assert_eq!(size_of::<ParticleVertex>(), 36);
    }

    /// `material.md` §11.2 per draw with *A* = *M*.alpha (`particle-runtime.md` §9.1.1
    /// rule 1).
    #[test]
    fn per_draw_alpha_reference() {
        // Alpha test off: 0 with a source-alpha blend (opacity-0 texels go), else nothing.
        assert_eq!(draw_reference(None, false, true, 1.0), Some(0.0));
        assert_eq!(draw_reference(None, false, true, 0.2), Some(0.0));
        assert_eq!(draw_reference(None, true, false, 1.0), None);
        // On, depth write off: kept while A >= 0.9, switched off below.
        assert_eq!(draw_reference(Some(0.8), false, true, 1.0), Some(0.8));
        assert_eq!(draw_reference(Some(0.8), false, true, 0.9), Some(0.8));
        assert_eq!(draw_reference(Some(0.8), false, true, 0.89), Some(0.0));
        assert_eq!(draw_reference(Some(0.8), false, false, 0.5), None);
        // On, depth write on: scaled by A, then rounded down to 8 bits.
        let r = draw_reference(Some(0.8), true, true, 0.5).unwrap();
        assert!((r - (102.0 / 255.0)).abs() < 1e-6, "{r}");
        let r = draw_reference(Some(0.65), true, true, 1.0).unwrap();
        assert!((r - 165.0 / 255.0).abs() < 1e-6, "{r}");
        // Hostile values: a non-finite A counts as 0, a non-finite reference as no test.
        assert_eq!(draw_reference(Some(0.8), true, true, f32::NAN), Some(0.0));
        assert_eq!(draw_reference(Some(f32::NAN), true, true, 1.0), Some(0.0));
        // The source factor of the draw states.
        assert!(draw_state(AlphaMode::Blend).source_alpha());
        assert!(draw_state(AlphaMode::Additive).source_alpha());
        assert!(!draw_state(AlphaMode::Opaque).source_alpha());
    }

    #[test]
    fn program_bits() {
        assert_eq!(ParticleShading::STANDARD.program_bits(), 0b011);
        let p = ParticleShading {
            texture_alpha: false,
            doubled: true,
            ..ParticleShading::STANDARD
        };
        assert_eq!(p.program_bits(), 0b110);
    }

    #[test]
    fn alpha_modes_map_to_states() {
        assert_eq!(draw_state(AlphaMode::Blend).blend, BlendKey::Alpha);
        assert!(!draw_state(AlphaMode::Blend).depth_write);
        let add = draw_state(AlphaMode::Additive);
        assert_eq!(add.blend, BlendKey::Fixed(FixedBlend::ADDITIVE));
        assert_eq!(
            draw_state(AlphaMode::Mask { cutoff: f32::NAN }).cutoff,
            Some(0.5)
        );
        assert_eq!(
            draw_state(AlphaMode::TestBlend { cutoff: -1.0 }).cutoff,
            None
        );
        assert!(draw_state(AlphaMode::TestBlend { cutoff: 0.2 }).depth_write);
        let fixed = FixedBlend {
            depth_write: true,
            ..FixedBlend::ADDITIVE
        };
        assert!(draw_state(AlphaMode::Fixed(fixed)).depth_write);
        assert_eq!(draw_state(AlphaMode::Fixed(fixed)).cutoff, None);
        let tested = FixedBlend {
            alpha_test: Some(crate::material::AlphaThreshold(0.25)),
            ..fixed
        };
        assert_eq!(draw_state(AlphaMode::Fixed(tested)).cutoff, Some(0.25));
    }
}
