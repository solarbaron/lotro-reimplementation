// SPDX-License-Identifier: GPL-3.0-or-later
//! Placed decal draws (`docs/specs/decals.md` §6; `docs/specs/render.md` §5).
//!
//! [`Pass::decal`] queues one [`DecalDraw`]: the triangles of one decal batch (decals that
//! share the render phase, the depth-bias entry and the material, `decals.md` §6.6) as a
//! triangle list in world space, each vertex with the per-decal colour of §6.2 and the
//! texture coordinates of §4.5. The renderer draws every queued batch as one draw call, in
//! queue order and without sorting (§6.6: batch order, not sorted by depth).
//!
//! **Where (Tessera rule of §6.6):** after the opaque pass, in the world pass's opaque
//! stage after the opaque and alpha-tested meshes, the terrain layers and the opaque
//! instanced scenery (the receivers), before the sky; with the depth test on, depth write
//! as the material's [`AlphaMode`] says (mostly off), the material's blend, and the
//! batch's depth bias ([`DecalDraw::depth_bias`], §6.5; the table's sign convention,
//! negated for the reversed depth buffer as [`DrawDepthBias`] does for entities).
//!
//! **Shading:** colour = *T* × *V* × *M* and alpha = *T*.alpha × *V*.alpha × *M*.alpha,
//! with *T* the surface texture, *V* the vertex colour (left out when
//! [`DecalDraw::vertex_colour`] is off: the surface's program has no colour input,
//! `decals.md` §6.1) and *M* the surface's material colour 2000; fogged
//! (`lighting.md` §10) when [`DecalDraw::fogged`] is set and the pass has fog. Colours are
//! in the stored-value space of `render.md` §4.1, like particle draws. The alpha test, when
//! the [`AlphaMode`] has one, compares the texture's alpha with the per-draw reference of
//! `material.md` §11.2 (*A* = *M*.alpha, [`crate::draw_reference`]); a source-alpha blend
//! without a test discards texels of opacity 0. The lighting of a decal is in its vertex
//! colour (§6.2–§6.3, the caller's): decals are drawn unlit. Like the other world
//! queues they draw only in passes with [`Pass::frame_lighting`] (skipped with one warning
//! per frame in others); screen passes ignore them.

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::depth_bias::{BiasKey, DrawDepthBias};
use crate::error::{RenderError, Result};
use crate::frame::{Pass, Rgba};
use crate::instanced::{uniform_entry, uniform_stride};
use crate::material::{AlphaMode, BlendFactor, FixedAlpha, FixedBlend};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::pipelines::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::renderer::{FrameStats, ResolvedTexture, TextureResolver};
use crate::texture::TextureId;

const DECALS_WGSL: &str = include_str!("shaders/decals.wgsl");

/// Decal draws (batches) per pass and frame.
pub const MAX_DECAL_DRAWS_PER_PASS: usize = 65_536;
/// Decal vertices per pass and frame (all batches together, 36 MiB of vertex data; the
/// device's `max_buffer_size` lowers it further). A batch that would pass it is skipped
/// whole, with a warning.
pub const MAX_DECAL_VERTICES_PER_PASS: usize = 1 << 20;

/// One decal vertex (world space).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct DecalVertex {
    /// Position, metres (after the lift of `decals.md` §4.5 step 4).
    pub position: [f32; 3],
    /// The per-decal colour of `decals.md` §6.2 (the packed bytes / 255), RGBA.
    pub color: [f32; 4],
    /// Texture coordinates (§4.5 step 3).
    pub uv: [f32; 2],
}

/// One decal batch ([`Pass::decal`]): triangles drawn with one material and depth bias.
#[derive(Debug, Clone)]
pub struct DecalDraw {
    /// Triangle list (three vertices per triangle; a trailing partial triangle is
    /// ignored).
    pub vertices: Vec<DecalVertex>,
    /// The surface texture (white when `None` or unknown).
    pub texture: Option<TextureId>,
    /// Blend, alpha test and depth write of the surface (`decals.md` §6.1).
    pub alpha: AlphaMode,
    /// The surface's material colour 2000 (*M*), stored values, RGBA.
    pub material: Rgba,
    /// The vertex colour is a factor of colour and alpha (the surface's program has a
    /// colour input, `decals.md` §6.1).
    pub vertex_colour: bool,
    /// Distance fog applies (lit passes with fog only).
    pub fogged: bool,
    /// Draw both faces (else back faces are culled; counter-clockwise front faces).
    pub two_sided: bool,
    /// The depth-bias entry `DecalVolume_DepthBias` selects (`decals.md` §6.5), in the
    /// table's sign convention.
    pub depth_bias: DrawDepthBias,
}

impl DecalDraw {
    /// An alpha-blended, white-textured, two-sided, unfogged batch of `vertices` without a
    /// depth bias, reading the vertex colour.
    #[must_use]
    pub fn new(vertices: Vec<DecalVertex>) -> Self {
        Self {
            vertices,
            texture: None,
            alpha: AlphaMode::Blend,
            material: [1.0; 4],
            vertex_colour: true,
            fogged: false,
            two_sided: true,
            depth_bias: DrawDepthBias::NONE,
        }
    }
}

impl Default for DecalDraw {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Pass {
    /// Queue a decal batch (see the module docs). Ignored by screen passes.
    pub fn decal(&mut self, draw: DecalDraw) -> &mut Self {
        self.world.decals.push(draw);
        self
    }
}

/// The fixed-function state a batch's [`AlphaMode`] selects.
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
    fn source_alpha(&self) -> bool {
        match self.blend {
            BlendKey::Alpha => true,
            BlendKey::Fixed(f) => f.src == BlendFactor::SrcAlpha,
            BlendKey::Opaque => false,
        }
    }
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
        AlphaMode::Fixed(f) => DrawState {
            blend: BlendKey::Fixed(f),
            depth_write: f.depth_write,
            cutoff: f.alpha_test.map(|t| t.value()),
        },
    }
}

/// The distance fog of a world pass (`lighting.md` §10), for fogged decals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DecalFog {
    pub eye: [f32; 3],
    pub near: f32,
    pub inv_range: f32,
    pub color: [f32; 3],
}

/// Per-batch uniform (must match `DecalDraw` in `decals.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct DecalUniform {
    view_proj: [[f32; 4]; 4],
    material: [f32; 4],
    params: [f32; 4],
    eye: [f32; 4],
    fog_color: [f32; 4],
    flags: [u32; 4],
}

type PipeKey = (
    wgpu::TextureFormat,
    BlendKey,
    bool,
    FixedAlpha,
    BiasKey,
    bool,
);

/// Pipelines of the decal path, built lazily.
#[derive(Debug, Default)]
pub(crate) struct DecalState {
    uniform_layout: Option<wgpu::BindGroupLayout>,
    texture_layout: Option<wgpu::BindGroupLayout>,
    layout: Option<wgpu::PipelineLayout>,
    shader: Option<wgpu::ShaderModule>,
    pipelines: Vec<(PipeKey, wgpu::RenderPipeline)>,
}

/// Most pipelines kept (keys differ by blend, depth write, bias and culling; the data uses
/// a handful). Beyond it the oldest is dropped.
const MAX_PIPELINES: usize = 256;

/// What [`build_hook`] needs from the frame.
#[derive(Debug)]
pub(crate) struct DecalFrame<'a> {
    pub device: &'a wgpu::Device,
    pub format: wgpu::TextureFormat,
    pub view_proj: Mat4,
    pub textures: &'a TextureResolver<'a>,
    pub premultiplied: bool,
    pub fog: Option<DecalFog>,
    /// The adapter supports a depth-bias clamp.
    pub bias_clamp: bool,
}

impl DecalState {
    fn layouts(&mut self, device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let uniform = self
            .uniform_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-decals-draw-bgl"),
                    entries: &[uniform_entry(
                        0,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        true,
                        size_of::<DecalUniform>(),
                    )],
                })
            })
            .clone();
        let texture = self
            .texture_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-decals-texture-bgl"),
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

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        state: DrawState,
        premultiplied: bool,
        bias: BiasKey,
        two_sided: bool,
    ) -> wgpu::RenderPipeline {
        let alpha = match state.blend {
            BlendKey::Fixed(f) => f.alpha_rule(premultiplied),
            BlendKey::Opaque | BlendKey::Alpha => FixedAlpha::Keep,
        };
        let key = (
            format,
            state.blend,
            state.depth_write,
            alpha,
            bias,
            two_sided,
        );
        if let Some((_, p)) = self.pipelines.iter().find(|(k, _)| *k == key) {
            return p.clone();
        }
        let (uniform, texture) = self.layouts(device);
        let shader = self
            .shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-decals.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(DECALS_WGSL.into()),
                })
            })
            .clone();
        let layout = self
            .layout
            .get_or_insert_with(|| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-decals-layout"),
                    bind_group_layouts: &[Some(&uniform), Some(&texture)],
                    immediate_size: 0,
                })
            })
            .clone();
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
            label: Some("tessera-decals"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_decal"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<DecalVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: (!two_sided).then_some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(state.depth_write),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: bias.state(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_decal"),
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
        if self.pipelines.len() >= MAX_PIPELINES {
            self.pipelines.remove(0);
        }
        self.pipelines.push((key, p.clone()));
        p
    }
}

#[derive(Debug)]
struct DecalItem {
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::BindGroup,
    uniform_offset: u32,
    first: u32,
    count: u32,
}

struct DecalHook {
    items: Vec<DecalItem>,
    vertices: wgpu::Buffer,
    uniforms: wgpu::BindGroup,
    triangles: u32,
}

impl fmt::Debug for DecalHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecalHook")
            .field("items", &self.items.len())
            .field("triangles", &self.triangles)
            .finish_non_exhaustive()
    }
}

fn finite_vertex(v: &DecalVertex) -> DecalVertex {
    let f = |x: f32| if x.is_finite() { x } else { 0.0 };
    DecalVertex {
        position: v.position.map(f),
        color: v.color.map(|c| {
            if c.is_finite() {
                c.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }),
        uv: v.uv.map(f),
    }
}

/// Builds the hook of one world pass's decal batches.
pub(crate) fn build_hook(
    state: &mut DecalState,
    mut draws: Vec<DecalDraw>,
    ctx: &DecalFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    if draws.is_empty() {
        return Ok(None);
    }
    if draws.len() > MAX_DECAL_DRAWS_PER_PASS {
        // Too many batches never fail the frame: the draws beyond the bound are left out.
        tracing::warn!(
            queued = draws.len(),
            max = MAX_DECAL_DRAWS_PER_PASS,
            "decal draws beyond the per-pass bound skipped"
        );
        draws.truncate(MAX_DECAL_DRAWS_PER_PASS);
    }
    let device = ctx.device;
    let (_, texture_layout) = state.layouts(device);
    let stride = uniform_stride(device, size_of::<DecalUniform>());
    let mut uniform_data: Vec<u8> = Vec::new();
    let mut vertices: Vec<DecalVertex> = Vec::new();
    let mut items = Vec::with_capacity(draws.len());
    let srgb_target = ctx.format.is_srgb();
    let vsize = size_of::<DecalVertex>() as u64;
    let budget = usize::try_from(device.limits().max_buffer_size / vsize)
        .unwrap_or(usize::MAX)
        .min(MAX_DECAL_VERTICES_PER_PASS);
    let mut skipped = 0usize;
    for d in &draws {
        let n = d.vertices.len() / 3 * 3;
        if n == 0 {
            continue;
        }
        if vertices.len() + n > budget {
            skipped += 1;
            continue;
        }
        // Non-finite colour channels are 1; a NaN alpha counts as 0 (`material.md` §18).
        let finite = |c: f32| if c.is_finite() { c } else { 1.0 };
        let alpha = |c: f32| {
            if c.is_nan() {
                0.0
            } else if c.is_finite() {
                c
            } else {
                c.clamp(0.0, 1.0)
            }
        };
        let material = [
            finite(d.material[0]),
            finite(d.material[1]),
            finite(d.material[2]),
            alpha(d.material[3]),
        ];
        let st = draw_state(d.alpha);
        let reference = crate::particles::draw_reference(
            st.cutoff,
            st.depth_write,
            st.source_alpha(),
            material[3],
        );
        let resolved: ResolvedTexture = ctx
            .textures
            .get(d.texture)
            .unwrap_or_else(|| ctx.textures.white());
        let fog = ctx.fog.filter(|_| d.fogged);
        let uniform = DecalUniform {
            view_proj: ctx.view_proj.to_cols_array_2d(),
            material,
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
                u32::from(d.vertex_colour),
            ],
        };
        let offset = u32::try_from(uniform_data.len()).map_err(|_| RenderError::LimitExceeded {
            what: "decal uniform bytes per pass",
            requested: uniform_data.len() as u64,
            max: u64::from(u32::MAX),
        })?;
        uniform_data.extend_from_slice(bytemuck::bytes_of(&uniform));
        uniform_data.resize(
            uniform_data.len() + (stride as usize - size_of::<DecalUniform>()),
            0,
        );
        let texture = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessera-decals-texture"),
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
        let (Ok(first), Ok(count)) = (u32::try_from(vertices.len()), u32::try_from(n)) else {
            skipped += 1;
            continue;
        };
        vertices.extend(d.vertices.iter().take(n).map(finite_vertex));
        let bias = d.depth_bias.reversed(ctx.bias_clamp);
        items.push(DecalItem {
            pipeline: state.pipeline(device, ctx.format, st, ctx.premultiplied, bias, d.two_sided),
            texture,
            uniform_offset: offset,
            first,
            count,
        });
    }
    if skipped > 0 {
        tracing::warn!(
            skipped,
            budget,
            "decal draws skipped: the pass's vertex budget is spent"
        );
    }
    if items.is_empty() {
        return Ok(None);
    }
    let (uniform_layout, _) = state.layouts(device);
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-decals-uniforms"),
        contents: &uniform_data,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let uniforms = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-decals-uniforms"),
        layout: &uniform_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<DecalUniform>() as u64),
            }),
        }],
    });
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-decals-vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    Ok(Some(Arc::new(DecalHook {
        items,
        vertices: vertex_buffer,
        uniforms,
        triangles: u32::try_from(vertices.len() / 3).unwrap_or(u32::MAX),
    })))
}

impl PassHook for DecalHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Opaque {
            return;
        }
        rp.set_vertex_buffer(0, self.vertices.slice(..));
        for item in &self.items {
            rp.set_pipeline(&item.pipeline);
            rp.set_bind_group(0, &self.uniforms, &[item.uniform_offset]);
            rp.set_bind_group(1, &item.texture, &[]);
            rp.draw(item.first..item.first + item.count, 0..1);
            stats.draw_calls = stats.draw_calls.saturating_add(1);
            stats.decal_draws = stats.decal_draws.saturating_add(1);
        }
        stats.decal_triangles = stats.decal_triangles.saturating_add(self.triangles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decal_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(DECALS_WGSL)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(DECALS_WGSL)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(DECALS_WGSL)));
    }

    #[test]
    fn layouts() {
        assert_eq!(size_of::<DecalUniform>(), 144);
        assert_eq!(size_of::<DecalVertex>(), 36);
    }

    #[test]
    fn alpha_modes_map_to_states() {
        let b = draw_state(AlphaMode::Blend);
        assert_eq!(b.blend, BlendKey::Alpha);
        assert!(!b.depth_write && b.source_alpha());
        assert!(draw_state(AlphaMode::Opaque).depth_write);
        assert_eq!(
            draw_state(AlphaMode::Mask { cutoff: f32::NAN }).cutoff,
            Some(0.5)
        );
        let add = draw_state(AlphaMode::Additive);
        assert_eq!(add.blend, BlendKey::Fixed(FixedBlend::ADDITIVE));
        assert!(!add.depth_write);
    }

    #[test]
    fn hostile_vertices_are_made_finite() {
        let v = finite_vertex(&DecalVertex {
            position: [f32::NAN, 1.0, f32::INFINITY],
            color: [2.0, -1.0, f32::NAN, 0.5],
            uv: [f32::NEG_INFINITY, 0.25],
        });
        assert_eq!(v.position, [0.0, 1.0, 0.0]);
        assert_eq!(v.color, [1.0, 0.0, 0.0, 0.5]);
        assert_eq!(v.uv, [0.0, 0.25]);
    }
}
