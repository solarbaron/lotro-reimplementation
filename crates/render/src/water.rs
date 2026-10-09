// SPDX-License-Identifier: GPL-3.0-or-later
//! The water pipeline (M4.5, `docs/specs/water.md` §8–§9; `docs/specs/render.md` §5.3).
//!
//! [`Pass::water`] draws landscape water as horizontal quads at the water height, one per
//! wet landblock (`water.md` §8.1 Tessera rule) or per distant landblock of a 3 × 3
//! sector (§9: the same (type, height) pairs, derived by the caller from the block maps),
//! each optionally subdivided into a grid for vertex waves. The geometry is generated in
//! the vertex shader from one instance per quad, so thousands of quads cost one draw.
//!
//! Shading follows the water permutations of shader record 0x2B0000A8 (§8.3–§8.4): three
//! scrolling normal-map layers, optional waves, a refraction of the scene drawn so far
//! (a copy of the colour target made just before the pass's translucent stage), the
//! refraction standing in for the reflection, Fresnel, specular with the sparkle term, the
//! water colour and overbright, then distance fog. Output alpha 1, written with source ONE
//! and destination ZERO, depth written, both faces drawn (§8.4 Tessera rule).
//!
//! A draw may carry the far view's far-draw fade (`far-view.md` §7.2, FD1; the far view
//! applies it to the sector water as a Tessera choice, `far-view.md` §8 OPEN): the fragment
//! is blended toward the undistorted scene copy by *a* = 1 − sat((*d* − *s*) × *r*), *d* the
//! distance to the eye, and discarded where *a* < 0.001 (without a scene copy: where
//! *a* < 0.5).

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::error::{RenderError, Result};
use crate::far_view::FarFadeParams;
use crate::frame::Pass;
use crate::instanced::{
    lit_source, max_uniform_bytes, sampler_entry, texture_entry, uniform_entry, uniform_stride,
    world_depth,
};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::renderer::{FrameStats, WorldFrame};
use crate::texture::TextureId;

const WATER_WGSL: &str = include_str!("shaders/water.wgsl");

/// Landblock edge in world units (`water.md` §8.1).
pub const WATER_BLOCK: f32 = 160.0;

/// Largest grid subdivision of one quad (per side).
pub const MAX_WATER_SUBDIVISIONS: u32 = 64;

/// Largest number of water quads per pass and frame.
pub const MAX_WATER_QUADS: usize = 1 << 16;

/// Largest number of vertices the water of one pass generates (all draws together). Grids
/// are coarsened to stay below; one cell per quad always fits (6 × [`MAX_WATER_QUADS`]).
pub const MAX_WATER_VERTICES: u64 = 1 << 24;

/// The water parameters of a surface (`water.md` §8.2: the `c_Water…` material
/// parameters the 0x2B0000A8 programs read). [`Default`] gives that record's shader
/// defaults; a surface's materials override them. Colours are stored-value RGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterParams {
    /// 1044 `c_WaterWaveSpeed`.
    pub wave_speed: f32,
    /// 1045 `c_WaterWaveFrequency`.
    pub wave_frequency: f32,
    /// 1046 `c_WaterWaveAmplitude` (0: no waves).
    pub wave_amplitude: f32,
    /// 1057–1062: (U, V) scroll speed of normal layers 0, 1, 2 (units per second).
    pub normal_speed: [[f32; 2]; 3],
    /// 1063–1065: tiling of normal layers 0, 1, 2 (repeats per landblock).
    pub normal_scale: [f32; 3],
    /// How many normal layers are used (1–3). Records that do not declare layers 1 and 2
    /// (`water.md` §8.2 OPEN) may set 1.
    pub normal_layers: u32,
    /// 1047 `c_WaterReflectionBumpDistortionAmount`.
    pub reflection_distortion: f32,
    /// 1048 `c_WaterRefractionBumpDistortionAmount`.
    pub refraction_distortion: f32,
    /// 1050 `c_WaterFresnelBias`.
    pub fresnel_bias: f32,
    /// 1051 `c_WaterFresnelExponent`.
    pub fresnel_exponent: f32,
    /// 1054 `c_WaterSpecularPower`.
    pub specular_power: f32,
    /// 1066 `c_WaterOverbrightAmount`.
    pub overbright: f32,
    /// 1067 `c_WaterFakeSpecularThreshold` (sparkle threshold).
    pub fake_specular_threshold: f32,
    /// 1068 `c_WaterFogAmount` (under-water tint of the refraction).
    pub fog_amount: f32,
    /// 2006 `c_WaterSpecularColor`.
    pub specular_color: [f32; 3],
    /// 2009 `c_WaterColor`.
    pub water_color: [f32; 3],
    /// 2010 `c_WaterFogColor`.
    pub fog_color: [f32; 3],
    /// 2011 `c_WaterShorelineColor`.
    pub shoreline_color: [f32; 3],
}

impl Default for WaterParams {
    /// The shader defaults of 0x2B0000A8 (`water.md` §8.2 table; decision 0012).
    fn default() -> Self {
        Self {
            wave_speed: 1.0,
            wave_frequency: 0.2,
            wave_amplitude: 0.0,
            normal_speed: [[0.005, 0.0075], [0.015, 0.02], [-0.01, -0.0125]],
            normal_scale: [2.0, 4.0, 8.0],
            normal_layers: 3,
            reflection_distortion: 0.1,
            refraction_distortion: 0.04,
            fresnel_bias: 0.1,
            fresnel_exponent: 2.0,
            specular_power: 80.0,
            overbright: 1.0,
            fake_specular_threshold: 0.25,
            fog_amount: 1.0,
            specular_color: [1.0; 3],
            water_color: [1.0; 3],
            fog_color: [0.0; 3],
            shoreline_color: [1.0; 3],
        }
    }
}

/// One horizontal water quad (`water.md` §8.1): the rectangle from `origin` to
/// `origin + size` (world x east, y north) at height `height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterQuad {
    /// South-west corner (world x, y).
    pub origin: [f32; 2],
    /// Extent along x and y (positive).
    pub size: [f32; 2],
    /// Water height *h* (world z).
    pub height: f32,
}

impl WaterQuad {
    /// The 160 × 160 quad of a wet landblock whose south-west corner is at `origin`
    /// (`water.md` §8.1 Tessera rule).
    #[must_use]
    pub fn landblock(origin: [f32; 2], height: f32) -> Self {
        Self {
            origin,
            size: [WATER_BLOCK; 2],
            height,
        }
    }

    fn sane(&self) -> Option<Self> {
        let ok = self.origin.iter().chain(&self.size).all(|v| v.is_finite())
            && self.height.is_finite()
            && self.size.iter().all(|&s| s > 0.0);
        ok.then_some(*self)
    }
}

/// Water of one surface drawn in a pass ([`Pass::water`]).
#[derive(Debug, Clone)]
pub struct WaterDraw {
    /// The quads (shared, so a list can be queued every frame for free).
    pub quads: Arc<[WaterQuad]>,
    /// The surface's normal map (3011). `None`: a flat surface. A map uploaded
    /// [`crate::ColorSpace::Srgb`] is re-encoded after sampling (`docs/specs/render.md`
    /// §5.1), so either upload gives the stored texels.
    pub normal_map: Option<TextureId>,
    /// The surface's water parameters.
    pub params: WaterParams,
    /// Grid cells per quad side (1–[`MAX_WATER_SUBDIVISIONS`]); more than 1 only matters
    /// with waves. Halved until the pass's water fits in [`MAX_WATER_VERTICES`] (draws in
    /// queue order, each leaving one cell per quad for the draws after it).
    pub subdivisions: u32,
    /// The horizontal offset *O* of `water.md` §8.3 (`c_ContiguousWorldBlockOffset`):
    /// normal-layer coordinates are (position − *O*) / 160. Tessera rule: (0, 0) in the
    /// region frame.
    pub block_offset: [f32; 2],
    /// The far-draw fade (`far-view.md` §7.2; module docs). Default: no fade.
    pub fade: FarFadeParams,
}

impl WaterDraw {
    /// Water of `quads` with `params`, one cell per quad, *O* = (0, 0).
    #[must_use]
    pub fn new(
        quads: impl Into<Arc<[WaterQuad]>>,
        normal_map: Option<TextureId>,
        params: WaterParams,
    ) -> Self {
        Self {
            quads: quads.into(),
            normal_map,
            params,
            subdivisions: 1,
            block_offset: [0.0; 2],
            fade: FarFadeParams::default(),
        }
    }
}

impl Pass {
    /// Draw water (`docs/specs/render.md` §5.3). Needs a lit pass
    /// ([`Pass::frame_lighting`]); ignored by screen passes. The pass's colour target is
    /// copied before its translucent stage so the water refracts what was drawn before.
    pub fn water(&mut self, draw: WaterDraw) -> &mut Self {
        self.world.water.push(draw);
        self
    }
}

/// Water uniform (must match `Water` in `water.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct WaterUniform {
    /// speed, frequency, amplitude, normal layer count.
    waves: [f32; 4],
    /// layer 0 (u, v), layer 1 (u, v) speeds.
    speed01: [f32; 4],
    /// layer 2 (u, v) speed, scales 0 and 1.
    speed2_scale01: [f32; 4],
    /// scale 2, reflection distortion, refraction distortion, Fresnel bias.
    misc0: [f32; 4],
    /// Fresnel exponent, specular power, overbright, fake specular threshold.
    misc1: [f32; 4],
    /// fog amount, has scene copy, normal map (0 none, 1 linear, 2 sRGB: re-encode),
    /// subdivisions.
    misc2: [f32; 4],
    specular_color: [f32; 4],
    water_color: [f32; 4],
    fog_color: [f32; 4],
    shoreline_color: [f32; 4],
    /// 1 / width, 1 / height, Oₓ, Oᵧ.
    screen: [f32; 4],
    /// Far-draw fade start *s*, inverse range *r* (0: no fade), unused ×2.
    fade: [f32; 4],
}

/// Largest Fresnel exponent and specular power uploaded (the lit pipelines' cap); values
/// below 0 become 0 (`docs/specs/render.md` §5.3, implementer note).
const MAX_WATER_POWER: f32 = 4096.0;

/// Per-quad instance data (must match `QuadIn` in `water.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct QuadGpu {
    /// origin.xy, size.xy
    rect: [f32; 4],
    /// height, unused ×3
    height: [f32; 4],
}

const QUAD_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];

impl WaterUniform {
    fn new(
        p: &WaterParams,
        scene: bool,
        normal: Option<bool>,
        subdivisions: u32,
        ctx: &WorldFrame<'_>,
        offset: [f32; 2],
        fade: FarFadeParams,
    ) -> Self {
        let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let d = WaterParams::default();
        let rgb = |c: [f32; 3], dc: [f32; 3]| [f(c[0], dc[0]), f(c[1], dc[1]), f(c[2], dc[2]), 1.0];
        let sp = |k: usize, i: usize| f(p.normal_speed[k][i], d.normal_speed[k][i]);
        let sc = |k: usize| f(p.normal_scale[k], d.normal_scale[k]);
        Self {
            waves: [
                f(p.wave_speed, d.wave_speed),
                f(p.wave_frequency, d.wave_frequency),
                f(p.wave_amplitude, 0.0),
                p.normal_layers.clamp(1, 3) as f32,
            ],
            speed01: [sp(0, 0), sp(0, 1), sp(1, 0), sp(1, 1)],
            speed2_scale01: [sp(2, 0), sp(2, 1), sc(0), sc(1)],
            misc0: [
                sc(2),
                f(p.reflection_distortion, d.reflection_distortion),
                f(p.refraction_distortion, d.refraction_distortion),
                f(p.fresnel_bias, d.fresnel_bias),
            ],
            // Exponents in 0..=4096 (as the lit and terrain powers): a negative one would
            // raise 0 to a negative power on the GPU (inf, then NaN).
            misc1: [
                f(p.fresnel_exponent, d.fresnel_exponent).clamp(0.0, MAX_WATER_POWER),
                f(p.specular_power, d.specular_power).clamp(0.0, MAX_WATER_POWER),
                f(p.overbright, d.overbright),
                f(p.fake_specular_threshold, d.fake_specular_threshold),
            ],
            misc2: [
                f(p.fog_amount, d.fog_amount),
                if scene { 1.0 } else { 0.0 },
                match normal {
                    None => 0.0,
                    Some(false) => 1.0,
                    Some(true) => 2.0,
                },
                subdivisions as f32,
            ],
            specular_color: rgb(p.specular_color, d.specular_color),
            water_color: rgb(p.water_color, d.water_color),
            fog_color: rgb(p.fog_color, d.fog_color),
            shoreline_color: rgb(p.shoreline_color, d.shoreline_color),
            screen: [
                1.0 / ctx.size.0.max(1) as f32,
                1.0 / ctx.size.1.max(1) as f32,
                f(offset[0], 0.0),
                f(offset[1], 0.0),
            ],
            fade: [
                f(fade.start, f32::MAX),
                f(fade.inv_range, 0.0).max(0.0),
                0.0,
                0.0,
            ],
        }
    }
}

/// Pipelines of the water, built lazily.
#[derive(Debug, Default)]
pub(crate) struct WaterState {
    layout: Option<wgpu::BindGroupLayout>,
    samplers: Option<(wgpu::Sampler, wgpu::Sampler)>,
    shader: Option<wgpu::ShaderModule>,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

impl WaterState {
    fn pipeline(
        &mut self,
        ctx: &WorldFrame<'_>,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::BindGroupLayout,
        (wgpu::Sampler, wgpu::Sampler),
    ) {
        let device = ctx.device;
        let layout = self
            .layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-water-bgl"),
                    entries: &[
                        uniform_entry(
                            0,
                            wgpu::ShaderStages::VERTEX_FRAGMENT,
                            true,
                            size_of::<WaterUniform>(),
                        ),
                        texture_entry(1),
                        texture_entry(2),
                        sampler_entry(3),
                        sampler_entry(4),
                    ],
                })
            })
            .clone();
        let samplers = self
            .samplers
            .get_or_insert_with(|| {
                let make = |a: wgpu::AddressMode| {
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("tessera-water-sampler"),
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
                    make(wgpu::AddressMode::Repeat),
                    make(wgpu::AddressMode::ClampToEdge),
                )
            })
            .clone();
        let shader = self
            .shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-water.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(water_source().into()),
                })
            })
            .clone();
        if let Some((_, p)) = self.pipelines.iter().find(|(f, _)| *f == ctx.format) {
            return (p.clone(), layout, samplers);
        }
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-water-layout"),
            bind_group_layouts: &[Some(ctx.frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-water"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_water"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<QuadGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &QUAD_ATTRS,
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
                entry_point: Some("fs_water"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                // ONE / ZERO: the water composites the scene copy itself (§8.4).
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((ctx.format, p.clone()));
        (p, layout, samplers)
    }
}

pub(crate) fn water_source() -> String {
    lit_source(&[WATER_WGSL])
}

struct WaterItem {
    /// Index into the hook's bind groups.
    bind_group: usize,
    offset: u32,
    quads: std::ops::Range<u32>,
    vertices: u32,
}

struct WaterHook {
    pipeline: wgpu::RenderPipeline,
    quads: wgpu::Buffer,
    bind_groups: Vec<wgpu::BindGroup>,
    items: Vec<WaterItem>,
}

impl fmt::Debug for WaterHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaterHook")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

/// Grid cells per quad side for each draw of a pass, given its valid quad counts and
/// requested subdivisions: each request clamped to 1–[`MAX_WATER_SUBDIVISIONS`] and halved
/// until the draw fits in what [`MAX_WATER_VERTICES`] leaves after the earlier draws and one
/// cell per quad of the later ones. The total never exceeds the budget (with at most
/// [`MAX_WATER_QUADS`] quads, one cell each always fits).
fn grid_cells(draws: &[(u64, u32)]) -> Vec<u32> {
    let mut later: u64 = draws.iter().map(|&(q, _)| q * 6).sum();
    let mut used: u64 = 0;
    draws
        .iter()
        .map(|&(quads, requested)| {
            later -= quads * 6;
            let budget = MAX_WATER_VERTICES.saturating_sub(used.saturating_add(later));
            let mut n = requested.clamp(1, MAX_WATER_SUBDIVISIONS);
            while n > 1 && quads * 6 * u64::from(n) * u64::from(n) > budget {
                n /= 2;
            }
            used += quads * 6 * u64::from(n) * u64::from(n);
            n
        })
        .collect()
}

pub(crate) fn build_hook(
    state: &mut WaterState,
    draws: Vec<WaterDraw>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    let total: usize = draws.iter().map(|d| d.quads.len()).sum();
    if total == 0 {
        return Ok(None);
    }
    if total > MAX_WATER_QUADS {
        return Err(RenderError::LimitExceeded {
            what: "water quads per pass",
            requested: total as u64,
            max: MAX_WATER_QUADS as u64,
        });
    }
    let (pipeline, layout, (repeat, clamp)) = state.pipeline(ctx);
    let white = ctx.textures.white();
    let scene = ctx.scene_copy;
    // The valid quads of every draw, back to back.
    let mut quads: Vec<QuadGpu> = Vec::with_capacity(total);
    let mut ranges = Vec::with_capacity(draws.len());
    for draw in &draws {
        let start = quads.len() as u32;
        quads.extend(
            draw.quads
                .iter()
                .filter_map(WaterQuad::sane)
                .map(|q| QuadGpu {
                    rect: [q.origin[0], q.origin[1], q.size[0], q.size[1]],
                    height: [q.height, 0.0, 0.0, 0.0],
                }),
        );
        ranges.push(start..quads.len() as u32);
    }
    let cells = grid_cells(
        &draws
            .iter()
            .zip(&ranges)
            .map(|(d, r)| (u64::from(r.end - r.start), d.subdivisions))
            .collect::<Vec<_>>(),
    );
    // One uniform slot per draw in one buffer (dynamic offsets); one bind group per
    // distinct normal map.
    let stride = uniform_stride(ctx.device, size_of::<WaterUniform>());
    let mut uniforms: Vec<u8> = Vec::new();
    let mut maps: Vec<Option<TextureId>> = Vec::new();
    let mut views: Vec<Option<crate::renderer::ResolvedTexture>> = Vec::new();
    let mut items = Vec::with_capacity(draws.len());
    let mut warned = false;
    for ((draw, range), n) in draws.iter().zip(ranges).zip(cells) {
        if range.is_empty() {
            continue;
        }
        let normal = ctx.textures.get(draw.normal_map);
        if draw.normal_map.is_some() && normal.is_none() && !warned {
            tracing::warn!(id = ?draw.normal_map, "water normal map unknown; flat water");
            warned = true;
        }
        let key = draw.normal_map.filter(|_| normal.is_some());
        let u = WaterUniform::new(
            &draw.params,
            scene.is_some(),
            normal.as_ref().map(|t| t.srgb),
            n,
            ctx,
            draw.block_offset,
            draw.fade,
        );
        let bind_group = match maps.iter().position(|m| *m == key) {
            Some(i) => i,
            None => {
                maps.push(key);
                views.push(normal);
                maps.len() - 1
            }
        };
        let offset = uniforms.len() as u64;
        if offset + stride > max_uniform_bytes(ctx.device) {
            return Err(RenderError::LimitExceeded {
                what: "water uniform bytes per pass",
                requested: offset + stride,
                max: max_uniform_bytes(ctx.device),
            });
        }
        uniforms.extend_from_slice(bytemuck::bytes_of(&u));
        uniforms.resize((offset + stride) as usize, 0);
        items.push(WaterItem {
            bind_group,
            offset: offset as u32,
            quads: range,
            vertices: 6 * n * n,
        });
    }
    if items.is_empty() {
        return Ok(None);
    }
    let uniform_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-water"),
            contents: &uniforms,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let scene_view = scene.unwrap_or(&white.view);
    let bind_groups = views
        .iter()
        .map(|normal| {
            let normal_view = normal.as_ref().map_or(&white.view, |t| &t.view);
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tessera-water"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform_buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(size_of::<WaterUniform>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(scene_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&repeat),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&clamp),
                    },
                ],
            })
        })
        .collect();
    let buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-water-quads"),
            contents: bytemuck::cast_slice(&quads),
            usage: wgpu::BufferUsages::VERTEX,
        });
    Ok(Some(Arc::new(WaterHook {
        pipeline,
        quads: buffer,
        bind_groups,
        items,
    })))
}

impl PassHook for WaterHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Translucent {
            return;
        }
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, group0, &[offset]);
        rp.set_vertex_buffer(0, self.quads.slice(..));
        for item in &self.items {
            let Some(bg) = self.bind_groups.get(item.bind_group) else {
                continue;
            };
            rp.set_bind_group(1, bg, &[item.offset]);
            rp.draw(0..item.vertices, item.quads.clone());
            stats.draw_calls = stats.draw_calls.saturating_add(1);
            stats.water_quads = stats
                .water_quads
                .saturating_add(item.quads.end - item.quads.start);
        }
    }
}

/// CPU twin of the water's height and slope (`water.md` §8.3): the wave height offset at
/// model position (`x`, `y`) and time `t`, and its derivatives along x and y.
#[must_use]
pub fn wave_height(params: &WaterParams, x: f32, y: f32, t: f32) -> (f32, f32, f32) {
    let (mut h, mut dx, mut dy) = (0.0f32, 0.0f32, 0.0f32);
    for k in 0..2 {
        let kf = k as f32;
        let a = params.wave_amplitude * (1.0 - 0.5 * kf);
        let fx = (-1.0 + 0.3 * kf) * params.wave_frequency * (kf + 1.0);
        let fy = 0.7 * kf * params.wave_frequency * (kf + 1.0);
        let s = fx * x + fy * y + params.wave_speed * t * (0.5 + 0.8 * kf);
        let s = wrap_pi(s);
        h += a * s.sin();
        dx += a * s.cos() * fx;
        dy += a * s.cos() * fy;
    }
    (h, dx, dy)
}

/// CPU twin of the far-draw fade of a water fragment at distance `d` from the eye (module
/// docs): *a* = 1 − sat((*d* − *s*) × *r*) with the values as the pipeline uploads them
/// (a non-finite start means no fade, a non-finite or negative inverse range 0).
#[must_use]
pub fn water_fade(fade: FarFadeParams, d: f32) -> f32 {
    let s = if fade.start.is_finite() {
        fade.start
    } else {
        f32::MAX
    };
    let r = if fade.inv_range.is_finite() {
        fade.inv_range.max(0.0)
    } else {
        0.0
    };
    1.0 - ((d - s) * r).clamp(0.0, 1.0)
}

/// `s` reduced to [−π, π).
fn wrap_pi(s: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    s - tau * ((s + std::f32::consts::PI) / tau).floor()
}

/// CPU twin of the Fresnel factor of `water.md` §8.4 step 5 for |*v* · *n*| = `v_dot_n`,
/// with the parameters as the pipeline uploads them (non-finite values replaced by the
/// defaults, the exponent clamped to 0–4096) and 0⁰ = 1.
#[must_use]
pub fn fresnel(params: &WaterParams, v_dot_n: f32) -> f32 {
    let d = WaterParams::default();
    let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
    let bias = f(params.fresnel_bias, d.fresnel_bias);
    let e = f(params.fresnel_exponent, d.fresnel_exponent).clamp(0.0, MAX_WATER_POWER);
    let x = f(1.0 - v_dot_n.abs(), 0.0).clamp(0.0, 1.0);
    let p = if e <= 0.0 {
        1.0
    } else if x <= 0.0 {
        0.0
    } else {
        x.powf(e)
    };
    bias + (1.0 - bias) * p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Negative exponents are clamped to 0 (x⁰ = 1, also for x = 0), never inf or NaN.
    #[test]
    fn fresnel_with_hostile_exponents() {
        let p = |e: f32| WaterParams {
            fresnel_bias: 0.1,
            fresnel_exponent: e,
            ..WaterParams::default()
        };
        assert!((fresnel(&p(2.0), 0.5) - (0.1 + 0.9 * 0.25)).abs() < 1e-6);
        for v in [0.0, 0.5, 1.0] {
            assert_eq!(fresnel(&p(-3.0), v), 1.0, "v . n = {v}");
            assert_eq!(fresnel(&p(0.0), v), 1.0, "v . n = {v}");
        }
        assert_eq!(fresnel(&p(2.0), 1.0), 0.1);
        assert!(fresnel(&p(f32::NAN), f32::NAN).is_finite());
    }

    #[test]
    fn water_shader_validates() {
        let src = water_source();
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    }

    #[test]
    fn layouts_match_the_wgsl() {
        assert_eq!(size_of::<WaterUniform>(), 192);
        assert_eq!(size_of::<QuadGpu>(), 32);
    }

    #[test]
    fn far_draw_fade_matches_the_far_terrain() {
        let f = FarFadeParams {
            start: 1_920.0,
            inv_range: 1.0 / 160.0,
        };
        assert_eq!(water_fade(f, 0.0), 1.0);
        assert_eq!(water_fade(f, 1_920.0), 1.0);
        assert!((water_fade(f, 2_000.0) - 0.5).abs() < 1e-6);
        assert_eq!(water_fade(f, 2_080.0), 0.0);
        assert_eq!(water_fade(f, 9_000.0), 0.0);
        // No fade by default, and hostile values never fade or produce NaN.
        assert_eq!(water_fade(FarFadeParams::default(), 1.0e9), 1.0);
        for (start, inv_range) in [(f32::NAN, 1.0), (0.0, f32::NAN), (0.0, -1.0)] {
            let a = water_fade(FarFadeParams { start, inv_range }, 100.0);
            assert_eq!(a, 1.0, "{start} {inv_range}");
        }
    }

    #[test]
    fn defaults_are_the_record_defaults() {
        let p = WaterParams::default();
        assert_eq!(p.wave_amplitude, 0.0);
        assert_eq!(p.normal_scale, [2.0, 4.0, 8.0]);
        assert_eq!(p.specular_power, 80.0);
        assert_eq!(p.fog_color, [0.0; 3]);
    }

    #[test]
    fn no_waves_with_zero_amplitude() {
        let p = WaterParams::default();
        assert_eq!(wave_height(&p, 123.0, 456.0, 7.0), (0.0, 0.0, 0.0));
    }

    #[test]
    fn waves_follow_section_8_3() {
        let p = WaterParams {
            wave_amplitude: 2.0,
            wave_frequency: 0.2,
            wave_speed: 1.0,
            ..WaterParams::default()
        };
        // At the origin and t = 0 both phases are 0.
        let (h, dx, dy) = wave_height(&p, 0.0, 0.0, 0.0);
        assert!(h.abs() < 1e-6);
        // dh/dx = A0 * (-1) * f + A1 * (-0.7) * 2f = -2 * 0.2 - 1 * 0.7 * 0.4.
        assert!((dx - (-0.4 - 0.28)).abs() < 1e-5, "{dx}");
        // dh/dy = A1 * 0.7 * 2f = 0.28.
        assert!((dy - 0.28).abs() < 1e-5, "{dy}");
        // The derivatives match central differences at the same point.
        let e = 1e-2;
        let (_, dx, dy) = wave_height(&p, 3.0, 5.0, 2.0);
        let (hxp, ..) = wave_height(&p, 3.0 + e, 5.0, 2.0);
        let (hxm, ..) = wave_height(&p, 3.0 - e, 5.0, 2.0);
        let (hyp, ..) = wave_height(&p, 3.0, 5.0 + e, 2.0);
        let (hym, ..) = wave_height(&p, 3.0, 5.0 - e, 2.0);
        let fdx = (hxp - hxm) / (2.0 * e);
        let fdy = (hyp - hym) / (2.0 * e);
        assert!((fdx - dx).abs() < 1e-3, "{fdx} {dx}");
        assert!((fdy - dy).abs() < 1e-3, "{fdy} {dy}");
    }

    #[test]
    fn grids_fit_the_pass_budget() {
        let verts = |d: &[(u64, u32)], c: &[u32]| -> u64 {
            d.iter()
                .zip(c)
                .map(|(&(q, _), &n)| q * 6 * u64::from(n) * u64::from(n))
                .sum()
        };
        // One small draw keeps its request; the request is clamped to 1-64.
        assert_eq!(grid_cells(&[(4, 16)]), vec![16]);
        assert_eq!(grid_cells(&[(4, 0), (4, 999)]), vec![1, 64]);
        // One draw per quad at the largest grid: the per-pass total stays bounded and
        // every draw keeps at least one cell.
        let many = vec![(1u64, 64u32); MAX_WATER_QUADS];
        let cells = grid_cells(&many);
        assert!(verts(&many, &cells) <= MAX_WATER_VERTICES);
        assert!(cells.iter().all(|&n| n >= 1));
        assert_eq!(cells[0], 64);
        assert_eq!(*cells.last().unwrap_or(&0), 1);
        // A single huge draw is coarsened as before.
        let one = [(MAX_WATER_QUADS as u64, 64)];
        let c = grid_cells(&one);
        assert!(verts(&one, &c) <= MAX_WATER_VERTICES && c[0] >= 1, "{c:?}");
        assert_eq!(grid_cells(&[]), Vec::<u32>::new());
    }

    #[test]
    fn wrap_pi_stays_in_range() {
        for s in [
            -100.0f32,
            -3.2,
            -std::f32::consts::PI,
            0.0,
            3.1,
            3.2,
            1000.0,
        ] {
            let w = wrap_pi(s);
            assert!(
                (-std::f32::consts::PI..std::f32::consts::PI + 1e-5).contains(&w),
                "{s} {w}"
            );
            assert!((w.sin() - s.sin()).abs() < 1e-3);
        }
    }

    #[test]
    fn fresnel_follows_step_5() {
        let p = WaterParams::default();
        // Looking straight down: the bias.
        assert!((fresnel(&p, 1.0) - 0.1).abs() < 1e-6);
        // Grazing: 1.
        assert!((fresnel(&p, 0.0) - 1.0).abs() < 1e-6);
        assert!((fresnel(&p, 0.5) - (0.1 + 0.9 * 0.25)).abs() < 1e-6);
    }

    #[test]
    fn hostile_quads_are_dropped() {
        assert!(WaterQuad::landblock([0.0, 0.0], 1.0).sane().is_some());
        assert!(
            WaterQuad {
                origin: [f32::NAN, 0.0],
                size: [1.0; 2],
                height: 0.0
            }
            .sane()
            .is_none()
        );
        assert!(
            WaterQuad {
                origin: [0.0; 2],
                size: [-1.0, 1.0],
                height: 0.0
            }
            .sane()
            .is_none()
        );
    }
}
