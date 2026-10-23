// SPDX-License-Identifier: GPL-3.0-or-later
//! The renderer's UI state: pipelines, per-texture bind groups, the entry
//! uniform stream, and the planning and recording of a frame's UI passes
//! (render.md §6.4–§6.9, §6.11).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use bytemuck::{Pod, Zeroable};

use super::ops::PendingOps;
use super::timing::UiGpuTimer;
use super::{
    MAX_UI_CLIPS, MAX_UI_ENTRIES, RefusalCounts, UI_BLEND_ALPHA, UI_BLEND_COLOR, UiAddressing,
    UiEntry, UiFilter, UiPass, UiRect, UiRefusal, UiSource, UiStats, UiViewMode,
};
use crate::context::GpuContext;
use crate::error::{RenderError, Result};
use crate::texture::{GpuTexture, TextureId, TextureStore};

const UI_WGSL: &str = include_str!("../shaders/ui.wgsl");
const UI_CLEAR_WGSL: &str = include_str!("../shaders/ui_clear.wgsl");

/// Format of every UI texture (render.md §6.2).
pub(crate) const UI_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

const FLAG_TILE: i32 = 1;
const FLAG_NEAREST: i32 = 2;
const FLAG_PREMULTIPLIED: i32 = 4;
const FLAG_ONE_TO_ONE: i32 = 8;
const FLAG_LINEARIZE: i32 = 16;
const FLAG_UNROTATED: i32 = 32;
/// An untextured fill ([`super::UiFill`]): the premultiplied colour is in `tile_scale`.
const FLAG_FILL: i32 = 64;

/// Per-entry uniform block; must match `Entry` in `ui.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct EntryUniform {
    ndc01: [f32; 4],
    ndc23: [f32; 4],
    dest: [f32; 4],
    idest: [i32; 4],
    misc: [i32; 4],
    params: [f32; 4],
    tile_scale: [f32; 4],
    clip_rects: [[f32; 4]; MAX_UI_CLIPS],
    clip_maps: [[f32; 4]; 2 * MAX_UI_CLIPS],
}

/// UI state of one frame: its UI passes (each with the number of ordinary
/// passes recorded before it), the software cursor and the non-sRGB view.
#[derive(Debug, Default)]
pub(crate) struct FrameUi {
    pub(crate) passes: Vec<(usize, UiPass)>,
    pub(crate) cursor: Option<(TextureId, [i32; 2])>,
    /// Non-sRGB view of the frame's target, when the target is sRGB and has
    /// one.
    pub(crate) view: Option<wgpu::TextureView>,
}

impl FrameUi {
    pub(crate) fn new(view: Option<wgpu::TextureView>) -> Self {
        Self {
            view,
            ..Self::default()
        }
    }
}

/// The non-sRGB variant of an sRGB format (`None` for other formats).
fn unorm_variant(format: wgpu::TextureFormat) -> Option<wgpu::TextureFormat> {
    format.is_srgb().then(|| format.remove_srgb_suffix())
}

/// `view_formats` for a swap chain of `format` (render.md §6.4).
pub(crate) fn surface_view_formats(
    ctx: &GpuContext,
    format: wgpu::TextureFormat,
) -> Vec<wgpu::TextureFormat> {
    let flags = ctx.adapter.get_downlevel_capabilities().flags;
    match unorm_variant(format) {
        Some(f) if flags.contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS) => vec![f],
        _ => Vec::new(),
    }
}

/// `view_formats` for an offscreen target of `format` (render.md §6.4).
pub(crate) fn offscreen_view_formats(
    ctx: &GpuContext,
    format: wgpu::TextureFormat,
) -> &'static [wgpu::TextureFormat] {
    let flags = ctx.adapter.get_downlevel_capabilities().flags;
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb
            if flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS) =>
        {
            &[wgpu::TextureFormat::Rgba8Unorm]
        }
        _ => &[],
    }
}

/// The non-sRGB view of a target texture created with `view_formats`.
pub(crate) fn ui_view(
    texture: &wgpu::Texture,
    view_formats: &[wgpu::TextureFormat],
) -> Option<wgpu::TextureView> {
    let want = unorm_variant(texture.format())?;
    view_formats.contains(&want).then(|| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("tessera-ui-view"),
            format: Some(want),
            ..Default::default()
        })
    })
}

/// One drawn quad of a plan.
#[derive(Debug, Clone, Copy)]
struct UiDraw {
    offset: u32,
    /// The texture read, or `None` for an untextured fill (the placeholder is bound).
    texture: Option<TextureId>,
    scissor: [u32; 4],
}

/// Everything `end_frame` needs to record the frame's UI passes.
#[derive(Debug, Default)]
pub(crate) struct UiPlan {
    view: Option<wgpu::TextureView>,
    format: Option<wgpu::TextureFormat>,
    passes: Vec<(usize, Range<usize>)>,
    draws: Vec<UiDraw>,
    cursor: Option<UiDraw>,
    /// Statistics of the frame's UI work.
    pub(crate) stats: UiStats,
}

/// The renderer's UI state.
#[derive(Debug)]
pub(crate) struct UiState {
    uniform_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    pub(crate) clear_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    stride: u64,
    bind_groups: HashMap<TextureId, wgpu::BindGroup>,
    /// A 1 × 1 UI texture bound for untextured fills, which the pipeline layout still needs
    /// a texture for (the fill branch of the shader never reads it).
    _fill_texture: wgpu::Texture,
    fill_bind_group: wgpu::BindGroup,
    /// Live UI textures and their bytes.
    pub(crate) textures: HashMap<TextureId, u64>,
    pub(crate) texture_bytes: u64,
    pub(crate) ops: PendingOps,
    /// Upload/copy/clear counters since the last `end_frame`.
    pub(crate) op_stats: UiStats,
    warned: [bool; UiRefusal::ALL.len()],
    pub(crate) force_fallback: bool,
    logged_fallback: bool,
    /// The UI pass timer (`UiStats::gpu_ns`); `None` without timestamp queries. Made at
    /// the first `prepare`, which has the queue (the timestamp period).
    timer: Option<UiGpuTimer>,
    timer_checked: bool,
}

impl UiState {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let stride = (size_of::<EntryUniform>() as u64).next_multiple_of(u64::from(
            device.limits().min_uniform_buffer_offset_alignment.max(16),
        ));
        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-ui-entry-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(size_of::<EntryUniform>() as u64),
                },
                count: None,
            }],
        });
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-ui-texture-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-ui-layout"),
            bind_group_layouts: &[Some(&uniform_bgl), Some(&texture_bgl)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-ui.wgsl"),
            source: wgpu::ShaderSource::Wgsl(UI_WGSL.into()),
        });
        let clear_pipeline = build_clear_pipeline(device);
        let uniforms = alloc_uniforms(device, stride * 64);
        let uniform_bg = make_uniform_bg(device, &uniform_bgl, &uniforms);
        let fill_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tessera-ui-fill-placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: UI_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fill_view = fill_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let fill_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessera-ui-fill-placeholder"),
            layout: &texture_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&fill_view),
            }],
        });
        Self {
            uniform_bgl,
            texture_bgl,
            layout,
            shader,
            pipelines: Vec::new(),
            clear_pipeline,
            uniforms,
            uniform_bg,
            stride,
            bind_groups: HashMap::new(),
            _fill_texture: fill_texture,
            fill_bind_group,
            textures: HashMap::new(),
            texture_bytes: 0,
            ops: PendingOps::default(),
            op_stats: UiStats::default(),
            warned: [false; UiRefusal::ALL.len()],
            force_fallback: false,
            logged_fallback: false,
            timer: None,
            timer_checked: false,
        }
    }

    /// Drop everything cached for a texture that is being destroyed.
    pub(crate) fn forget(&mut self, id: TextureId) {
        self.bind_groups.remove(&id);
        if let Some(bytes) = self.textures.remove(&id) {
            self.texture_bytes = self.texture_bytes.saturating_sub(bytes);
        }
    }

    fn pipeline_index(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) -> usize {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return i;
        }
        tracing::debug!(?format, "building UI pipeline");
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-ui"),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                // A tilt past 90 degrees flips the winding.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: UI_BLEND_COLOR,
                        alpha: UI_BLEND_ALPHA,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, pipeline));
        self.pipelines.len() - 1
    }

    /// Validate the frame's UI work, upload its entry uniforms and resolve
    /// bind groups (render.md §6.5). Never fails on bad entries: they are
    /// refused and counted.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &mut self,
        ctx: &GpuContext,
        textures: &TextureStore,
        frame_ui: &FrameUi,
        frame_view: &wgpu::TextureView,
        frame_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<UiPlan> {
        let mut plan = UiPlan {
            stats: std::mem::take(&mut self.op_stats),
            ..UiPlan::default()
        };
        plan.stats.texture_bytes = self.texture_bytes;
        if !self.timer_checked {
            self.timer = UiGpuTimer::new(&ctx.device, &ctx.queue);
            self.timer_checked = true;
        }
        plan.stats.gpu_ns = self
            .timer
            .as_ref()
            .and_then(|t| t.begin_frame(&ctx.device, self.ops.failed_flushes()));
        let mut refused = RefusalCounts::default();
        for (_, pass) in &frame_ui.passes {
            refused.merge(&pass.refused);
            if let Some(caps) = pass.caps {
                plan.stats.caps = caps;
            }
        }
        let has_work =
            frame_ui.passes.iter().any(|(_, p)| !p.is_empty()) || frame_ui.cursor.is_some();
        if !has_work {
            self.finish_refusals(&refused, &mut plan.stats);
            return Ok(plan);
        }

        // Where the UI draws, and whether the shader must linearise.
        let (view, format, linearize) = match (&frame_ui.view, self.force_fallback) {
            (Some(v), false) => (v.clone(), frame_format.remove_srgb_suffix(), false),
            _ => (frame_view.clone(), frame_format, frame_format.is_srgb()),
        };
        if linearize && !self.logged_fallback {
            tracing::info!(
                ?frame_format,
                "UI pass: no non-sRGB view of the target; blending in linear light (fallback)"
            );
            self.logged_fallback = true;
        }

        let mut data: Vec<u8> = Vec::new();
        let mut surfaces: HashSet<TextureId> = HashSet::new();
        let mut used: Vec<TextureId> = Vec::new();
        let mut count = 0usize;
        for (position, pass) in &frame_ui.passes {
            let start = plan.draws.len();
            for i in 0..pass.len() {
                let Some((entry, fill, fill_sanitised)) = pass.get(i) else {
                    continue;
                };
                if count >= MAX_UI_ENTRIES {
                    refused.add(UiRefusal::EntryCap);
                    continue;
                }
                count += 1;
                match self.plan_entry(
                    &entry,
                    fill,
                    textures,
                    frame_view,
                    (width, height),
                    linearize,
                    &mut data,
                ) {
                    Ok((draw, sanitised)) => {
                        if sanitised || fill_sanitised {
                            plan.stats.entries_sanitised =
                                plan.stats.entries_sanitised.saturating_add(1);
                        }
                        if let Some(draw) = draw {
                            if let Some(t) = draw.texture
                                && surfaces.insert(t)
                            {
                                used.push(t);
                            }
                            plan.draws.push(draw);
                        }
                    }
                    Err(kind) => refused.add(kind),
                }
            }
            plan.passes.push((*position, start..plan.draws.len()));
        }
        if let Some((tex, [x, y])) = frame_ui.cursor {
            // The cursor is a UI texture (straight alpha, §6.11): a registered
            // target would be premultiplied twice, so it is refused too.
            let planned = match textures.get(tex) {
                None => Err(UiRefusal::UnknownTexture),
                Some(_) if !self.textures.contains_key(&tex) => Err(UiRefusal::NotUiContent),
                Some(t) => {
                    let (w, h) = (t.width, t.height);
                    let c = UiEntry::new(tex, [w, h], UiRect::new(x, y, w, h));
                    self.plan_entry(
                        &c,
                        None,
                        textures,
                        frame_view,
                        (width, height),
                        linearize,
                        &mut data,
                    )
                }
            };
            match planned {
                Ok((Some(draw), _)) => {
                    if let Some(t) = draw.texture
                        && !used.contains(&t)
                    {
                        used.push(t);
                    }
                    plan.cursor = Some(draw);
                    plan.stats.cursor_drawn = true;
                }
                Ok((None, _)) => {}
                Err(kind) => self.warn_once(kind, "software cursor"),
            }
        }
        plan.stats.quads = u32::try_from(plan.draws.len()).unwrap_or(u32::MAX);
        plan.stats.surfaces = u32::try_from(surfaces.len()).unwrap_or(u32::MAX);
        self.finish_refusals(&refused, &mut plan.stats);
        if plan.draws.is_empty() && plan.cursor.is_none() {
            return Ok(plan);
        }

        // Upload the entry uniforms (one dynamic-offset block per draw).
        let device = &ctx.device;
        let bytes = data.len() as u64;
        if bytes > self.uniforms.size() {
            let max = device.limits().max_buffer_size;
            let size = bytes.next_power_of_two().min(max);
            if size < bytes {
                return Err(RenderError::LimitExceeded {
                    what: "UI entry uniforms",
                    requested: bytes,
                    max,
                });
            }
            let (buffer, gpu_error) = super::scoped(device, || alloc_uniforms(device, size));
            if let Some(e) = gpu_error {
                // An invalid buffer still reports its size: keep the old one,
                // so the next frame allocates again.
                buffer.destroy();
                return Err(RenderError::Gpu(e));
            }
            self.uniforms = buffer;
            self.uniform_bg = make_uniform_bg(device, &self.uniform_bgl, &self.uniforms);
        }
        ctx.queue.write_buffer(&self.uniforms, 0, &data);
        for id in used {
            if self.bind_groups.contains_key(&id) {
                continue;
            }
            if let Some(t) = textures.get(id) {
                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("tessera-ui-texture"),
                    layout: &self.texture_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&t.view),
                    }],
                });
                self.bind_groups.insert(id, bg);
            }
        }
        self.pipeline_index(device, format);
        plan.view = Some(view);
        plan.format = Some(format);
        Ok(plan)
    }

    fn finish_refusals(&mut self, refused: &RefusalCounts, stats: &mut UiStats) {
        stats.entries_refused = refused.total();
        for kind in refused.kinds() {
            self.warn_once(kind, "composition entry");
        }
    }

    fn warn_once(&mut self, kind: UiRefusal, what: &str) {
        let i = UiRefusal::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        if !self.warned[i] {
            self.warned[i] = true;
            tracing::warn!(%kind, what, "UI pass refused an entry (logged once per kind)");
        }
    }

    /// Check one entry against the renderer and write its uniform block.
    /// `fill` is the premultiplied colour of an untextured fill, whose
    /// texture is not resolved. `Ok((None, _))` = valid but draws nothing;
    /// the flag says whether a value was sanitised.
    #[allow(clippy::too_many_arguments)]
    fn plan_entry(
        &self,
        entry: &UiEntry<'_>,
        fill: Option<[f32; 4]>,
        textures: &TextureStore,
        frame_view: &wgpu::TextureView,
        (width, height): (u32, u32),
        linearize: bool,
        data: &mut Vec<u8>,
    ) -> std::result::Result<(Option<UiDraw>, bool), UiRefusal> {
        if fill.is_none() {
            let tex = textures
                .get(entry.texture)
                .ok_or(UiRefusal::UnknownTexture)?;
            classify(tex, frame_view)?;
        }
        let g = entry.geometry()?;
        if !g.drawable {
            return Ok((None, g.sanitised));
        }
        // Scissor: the screen-space clip items, within the target.
        let mut sc = [0i64, 0, i64::from(width), i64::from(height)];
        if let Some(s) = g.scissor {
            sc = [
                sc[0].max(i64::from(s[0])),
                sc[1].max(i64::from(s[1])),
                sc[2].min(i64::from(s[2])),
                sc[3].min(i64::from(s[3])),
            ];
        }
        // Bounding box of the projected quad, for an early out.
        let (mut bx0, mut by0, mut bx1, mut by1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for c in &g.corners {
            bx0 = bx0.min(c[0]);
            by0 = by0.min(c[1]);
            bx1 = bx1.max(c[0]);
            by1 = by1.max(c[1]);
        }
        let clamp_i = |v: f64| v.clamp(-1.0e12, 1.0e12) as i64;
        sc = [
            sc[0].max(clamp_i(bx0.floor())),
            sc[1].max(clamp_i(by0.floor())),
            sc[2].min(clamp_i(bx1.ceil())),
            sc[3].min(clamp_i(by1.ceil())),
        ];
        if sc[0] >= sc[2] || sc[1] >= sc[3] {
            return Ok((None, g.sanitised));
        }
        let to_u32 = |v: i64| u32::try_from(v).unwrap_or(0);
        let scissor = [
            to_u32(sc[0]),
            to_u32(sc[1]),
            to_u32(sc[2] - sc[0]),
            to_u32(sc[3] - sc[1]),
        ];

        let (w, h) = (f64::from(width.max(1)), f64::from(height.max(1)));
        let ndc = g
            .corners
            .map(|c| [(c[0] / w * 2.0 - 1.0) as f32, (1.0 - c[1] / h * 2.0) as f32]);
        let d = entry.dest;
        let mut flags = 0;
        let mut tile_scale = [0.0f32; 4];
        let ratio = match entry.addressing {
            UiAddressing::Fit => [
                (f64::from(entry.canvas[0]) / f64::from(d.w)) as f32,
                (f64::from(entry.canvas[1]) / f64::from(d.h)) as f32,
            ],
            UiAddressing::Tile { node_scale } => {
                flags |= FLAG_TILE;
                tile_scale = [node_scale[0], node_scale[1], 0.0, 0.0];
                [
                    (1.0 / f64::from(node_scale[0])) as f32,
                    (1.0 / f64::from(node_scale[1])) as f32,
                ]
            }
        };
        if entry.filter == UiFilter::Nearest {
            flags |= FLAG_NEAREST;
        }
        if entry.source == UiSource::Premultiplied {
            flags |= FLAG_PREMULTIPLIED;
        }
        if g.one_to_one {
            flags |= FLAG_ONE_TO_ONE;
        }
        if linearize {
            flags |= FLAG_LINEARIZE;
        }
        if !g.rotated {
            // Screen space is the root's unrotated space: the shader takes the
            // exact pixel centre instead of the interpolated position.
            flags |= FLAG_UNROTATED;
        }
        if let Some(colour) = fill {
            flags |= FLAG_FILL;
            tile_scale = colour;
        }
        let mut u = EntryUniform::zeroed();
        u.ndc01 = [ndc[0][0], ndc[0][1], ndc[1][0], ndc[1][1]];
        u.ndc23 = [ndc[2][0], ndc[2][1], ndc[3][0], ndc[3][1]];
        u.dest = [d.x as f32, d.y as f32, d.w as f32, d.h as f32];
        // Canvas sides are ≤ 16,384 (checked), so they fit i32.
        let cw = i32::try_from(entry.canvas[0]).unwrap_or(i32::MAX);
        let ch = i32::try_from(entry.canvas[1]).unwrap_or(i32::MAX);
        u.idest = [d.x, d.y, cw, ch];
        let n = i32::try_from(g.clips.len().min(MAX_UI_CLIPS)).unwrap_or(0);
        u.misc = [entry.tex_offset[0], entry.tex_offset[1], flags, n];
        u.params = [ratio[0], ratio[1], g.opacity, g.alpha_threshold];
        u.tile_scale = tile_scale;
        for (i, c) in g.clips.iter().take(MAX_UI_CLIPS).enumerate() {
            u.clip_rects[i] = c.rect.map(|v| v as f32);
            u.clip_maps[2 * i] = [
                c.map[0][0] as f32,
                c.map[0][1] as f32,
                c.map[0][2] as f32,
                0.0,
            ];
            u.clip_maps[2 * i + 1] = [
                c.map[1][0] as f32,
                c.map[1][1] as f32,
                c.map[1][2] as f32,
                0.0,
            ];
        }
        let offset = data.len();
        let Ok(offset32) = u32::try_from(offset) else {
            return Ok((None, g.sanitised));
        };
        data.extend_from_slice(bytemuck::bytes_of(&u));
        data.resize(offset + usize::try_from(self.stride).unwrap_or(512), 0);
        Ok((
            Some(UiDraw {
                offset: offset32,
                texture: fill.is_none().then_some(entry.texture),
                scissor,
            }),
            g.sanitised,
        ))
    }

    /// Record the UI passes placed before ordinary pass `position` (or after
    /// the last one, for `position` = the number of passes).
    pub(crate) fn record(
        &self,
        plan: &UiPlan,
        position: usize,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        for (pos, range) in &plan.passes {
            if *pos != position || range.is_empty() {
                continue;
            }
            if let Some(draws) = plan.draws.get(range.clone()) {
                self.record_draws(plan, draws, "tessera-ui-pass", encoder);
            }
        }
    }

    /// Record the software cursor (the last draw of the frame), then resolve the frame's
    /// UI timestamp queries when the device has them (`timing.rs`).
    pub(crate) fn record_cursor(&self, plan: &UiPlan, encoder: &mut wgpu::CommandEncoder) {
        if let Some(draw) = plan.cursor {
            self.record_draws(plan, &[draw], "tessera-ui-cursor", encoder);
        }
        if let Some(t) = &self.timer {
            t.finish_frame(encoder, self.ops.failed_flushes());
        }
    }

    fn record_draws(
        &self,
        plan: &UiPlan,
        draws: &[UiDraw],
        label: &str,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let (Some(view), Some(format)) = (&plan.view, plan.format) else {
            return;
        };
        let Some((_, pipeline)) = self.pipelines.iter().find(|(f, _)| *f == format) else {
            return;
        };
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: self.timer.as_ref().and_then(UiGpuTimer::pass_writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(pipeline);
        // `Some(None)`: the fill placeholder is bound.
        let mut bound: Option<Option<TextureId>> = None;
        for d in draws {
            let bg = match d.texture {
                Some(t) => match self.bind_groups.get(&t) {
                    Some(bg) => bg,
                    None => continue,
                },
                None => &self.fill_bind_group,
            };
            if bound != Some(d.texture) {
                rp.set_bind_group(1, bg, &[]);
                bound = Some(d.texture);
            }
            rp.set_bind_group(0, &self.uniform_bg, &[d.offset]);
            let [x, y, w, h] = d.scissor;
            rp.set_scissor_rect(x, y, w, h);
            rp.draw(0..4, 0..1);
        }
    }

    /// The view mode a frame of `format` gets (render.md §6.4).
    pub(crate) fn view_mode(&self, format: wgpu::TextureFormat, has_view: bool) -> UiViewMode {
        if format.is_srgb() && (self.force_fallback || !has_view) {
            UiViewMode::LinearFallback
        } else {
            UiViewMode::Stored
        }
    }
}

/// Whether a texture may be composed (render.md §6.5, §6.10).
fn classify(t: &GpuTexture, frame_view: &wgpu::TextureView) -> std::result::Result<(), UiRefusal> {
    let format = t.texture.format();
    match &t.target_view {
        Some(v) => {
            if v == frame_view {
                Err(UiRefusal::OwnTarget)
            } else if format.is_srgb() {
                Err(UiRefusal::SrgbTarget)
            } else if format == wgpu::TextureFormat::Rgba8Unorm {
                Ok(())
            } else {
                Err(UiRefusal::NotUiContent)
            }
        }
        None if format == UI_TEXTURE_FORMAT => Ok(()),
        None => Err(UiRefusal::NotUiContent),
    }
}

fn alloc_uniforms(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tessera-ui-entries"),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_uniform_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-ui-entries"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<EntryUniform>() as u64),
            }),
        }],
    })
}

fn build_clear_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tessera-ui-clear.wgsl"),
        source: wgpu::ShaderSource::Wgsl(UI_CLEAR_WGSL.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tessera-ui-clear-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tessera-ui-clear"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: UI_TEXTURE_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaders_parse_and_validate() {
        for (name, src) in [("ui", UI_WGSL), ("ui_clear", UI_CLEAR_WGSL)] {
            let module = wgpu::naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
            let mut v = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            );
            if let Err(e) = v.validate(&module) {
                panic!("{name}: {}", e.emit_to_string(src));
            }
        }
    }

    #[test]
    fn uniform_layout_matches_the_shader() {
        assert_eq!(size_of::<EntryUniform>(), 496);
        let module = wgpu::naga::front::wgsl::parse_str(UI_WGSL).expect("parses");
        let entry = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Entry"))
            .map(|(_, t)| t.inner.size(module.to_ctx()));
        assert_eq!(entry, Some(496));
    }

    #[test]
    fn inter_stage_variables_fit_downlevel_limits() {
        // Position plus five user locations: well inside 15.
        let module = wgpu::naga::front::wgsl::parse_str(UI_WGSL).expect("parses");
        let out = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("VsOut"))
            .map(|(_, t)| match &t.inner {
                wgpu::naga::TypeInner::Struct { members, .. } => members.len(),
                _ => 0,
            });
        assert_eq!(out, Some(6));
        assert!(6 <= wgpu::Limits::downlevel_webgl2_defaults().max_inter_stage_shader_variables);
    }

    #[test]
    fn unorm_variants() {
        assert_eq!(
            unorm_variant(wgpu::TextureFormat::Bgra8UnormSrgb),
            Some(wgpu::TextureFormat::Bgra8Unorm)
        );
        assert_eq!(
            unorm_variant(wgpu::TextureFormat::Rgba8UnormSrgb),
            Some(wgpu::TextureFormat::Rgba8Unorm)
        );
        assert_eq!(unorm_variant(wgpu::TextureFormat::Rgba8Unorm), None);
    }
}
