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
use crate::material::{AlphaMode, FixedAlpha, FixedBlend};
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

/// One draw instance of a particle system ([`Pass::particles`]).
#[derive(Debug, Clone, Default)]
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
        }
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
    tint: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
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
        let tint = d.tint.map(|c| if c.is_finite() { c } else { 1.0 });
        let st = draw_state(d.alpha);
        let resolved: ResolvedTexture = ctx
            .textures
            .get(d.texture)
            .unwrap_or_else(|| ctx.textures.white());
        let uniform = ParticleUniform {
            view_proj: ctx.view_proj.to_cols_array_2d(),
            tint,
            params: [st.cutoff.unwrap_or(0.0), 0.0, 0.0, 0.0],
            flags: [
                u32::from(resolved.srgb),
                u32::from(srgb_target),
                u32::from(st.cutoff.is_some()),
                0,
            ],
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
        assert_eq!(size_of::<ParticleUniform>(), 112);
        assert_eq!(size_of::<ParticleVertex>(), 36);
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
