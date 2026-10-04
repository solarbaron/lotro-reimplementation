// SPDX-License-Identifier: GPL-3.0-or-later
//! The [`Renderer`]: owns the GPU context, optional presentation surface,
//! built-in pipelines, textures and per-frame streaming buffers.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::{ContextOptions, GpuContext};
use crate::error::{RenderError, Result};
use crate::frame::{Frame, FrameTarget, PassSpace, Rgba, screen_projection, texture_runs};
use crate::pipelines::{
    DEPTH_FORMAT, Globals, LineVertex, PipelineCache, QuadInstance, SharedLayouts,
};
use crate::texture::{Filter, Samplers, TextureId, TextureStore, TextureUpload, Wrap};

/// Process-unique renderer ids; every handle a renderer issues carries its
/// id so handles are never mixed between renderers (whose wgpu instances
/// do not share resources).
fn next_owner_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Renderer-wide settings.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// GPU selection.
    pub context: ContextOptions,
    /// Preferred present mode; falls back to `Fifo` when unsupported.
    pub present_mode: wgpu::PresentMode,
    /// Colour the first pass clears to when it does not set its own.
    pub clear_color: Rgba,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            context: ContextOptions::default(),
            present_mode: wgpu::PresentMode::Fifo,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// Colour formats offered for offscreen targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffscreenFormat {
    /// `Rgba8UnormSrgb`: shader output is linear, stored sRGB-encoded.
    #[default]
    Rgba8Srgb,
    /// `Rgba8Unorm`: shader output stored as-is.
    Rgba8Unorm,
}

impl OffscreenFormat {
    fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        }
    }
}

/// A colour texture that frames can render into and that can be read back.
///
/// Only usable with the [`Renderer`] that created it; other renderers reject
/// it with [`RenderError::ForeignHandle`].
#[derive(Debug, Clone)]
pub struct OffscreenTarget {
    owner: u64,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl OffscreenTarget {
    /// Size in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Underlying wgpu texture, for code that drives wgpu directly. To draw
    /// the target with the built-in quads, register it with
    /// [`Renderer::register_target_texture`].
    #[must_use]
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

/// Tightly packed RGBA8 pixels read back from the GPU (top row first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub data: Vec<u8>,
}

impl RgbaImage {
    /// Pixel at `(x, y)` or `None` when out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        // Fields are public, so never trust width/height/data to agree.
        let i = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?
            .checked_mul(4)?;
        let p = self.data.get(i..i.checked_add(4)?)?;
        Some([p[0], p[1], p[2], p[3]])
    }
}

/// Statistics for a submitted frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Monotonic frame counter.
    pub frame_index: u64,
    /// Passes recorded.
    pub passes: u32,
    /// Quad instances drawn.
    pub quads: u32,
    /// Line segments drawn.
    pub lines: u32,
    /// Draw calls issued.
    pub draw_calls: u32,
}

struct SurfaceState<'w> {
    surface: wgpu::Surface<'w>,
    config: wgpu::SurfaceConfiguration,
    /// `true` when `config` must be (re)applied before the next acquire.
    dirty: bool,
}

impl std::fmt::Debug for SurfaceState<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceState")
            .field("config", &self.config)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

/// Small LRU cache keyed by `(width, height)` (used for depth buffers).
#[derive(Debug)]
struct SizeCache<V> {
    capacity: usize,
    /// Least recently used first.
    entries: Vec<((u32, u32), V)>,
}

impl<V> SizeCache<V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    /// Look up and mark as most recently used.
    fn get(&mut self, size: (u32, u32)) -> Option<&V> {
        let pos = self.entries.iter().position(|(k, _)| *k == size)?;
        let entry = self.entries.remove(pos);
        self.entries.push(entry);
        self.entries.last().map(|(_, v)| v)
    }

    /// Insert as most recently used, evicting the least recently used.
    fn insert(&mut self, size: (u32, u32), value: V) {
        self.entries.retain(|(k, _)| *k != size);
        while self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((size, value));
    }

    fn remove(&mut self, size: (u32, u32)) {
        self.entries.retain(|(k, _)| *k != size);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A GPU buffer that grows (power of two) as needed.
#[derive(Debug)]
struct StreamBuffer {
    label: &'static str,
    usage: wgpu::BufferUsages,
    buffer: wgpu::Buffer,
}

impl StreamBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        let buffer = Self::alloc(device, label, usage, 4096);
        Self {
            label,
            usage,
            buffer,
        }
    }

    fn alloc(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        size: u64,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Ensure capacity; returns `true` when the buffer was replaced.
    fn reserve(&mut self, device: &wgpu::Device, bytes: u64) -> Result<bool> {
        if bytes <= self.buffer.size() {
            return Ok(false);
        }
        let max = device.limits().max_buffer_size;
        let new_size = bytes.checked_next_power_of_two().unwrap_or(bytes).min(max);
        if new_size < bytes {
            return Err(RenderError::LimitExceeded {
                what: self.label,
                requested: bytes,
                max,
            });
        }
        self.buffer = Self::alloc(device, self.label, self.usage, new_size);
        Ok(true)
    }
}

/// The renderer. `'w` is the lifetime of the window the surface draws to
/// (`'static` for headless renderers and for owned window handles).
#[derive(Debug)]
pub struct Renderer<'w> {
    /// Process-unique id stamped into every handle this renderer issues.
    owner: u64,
    ctx: GpuContext,
    surface: Option<SurfaceState<'w>>,
    present_mode: wgpu::PresentMode,
    shared: SharedLayouts,
    pipelines: PipelineCache,
    samplers: Samplers,
    textures: TextureStore,
    white: TextureId,
    depth: SizeCache<wgpu::TextureView>,
    uniforms: StreamBuffer,
    globals_bg: wgpu::BindGroup,
    uniform_stride: u64,
    instances: StreamBuffer,
    lines: StreamBuffer,
    clear_color: wgpu::Color,
    frame_index: u64,
    /// Cloned into every surface [`Frame`]; strong count > 1 means a surface
    /// texture is still alive and the surface must not be reconfigured.
    frame_token: Arc<()>,
}

const MAX_DEPTH_CACHE: usize = 4;

fn align_up(v: u64, a: u64) -> u64 {
    if a <= 1 { v } else { v.div_ceil(a) * a }
}

impl Renderer<'static> {
    /// Renderer without a surface; render with offscreen targets.
    pub fn new_headless(config: &RendererConfig) -> Result<Self> {
        let instance = GpuContext::create_instance(&config.context, None);
        let ctx = GpuContext::new_with_instance(instance, &config.context, None)?;
        Self::from_parts(ctx, None, config)
    }

    /// Renderer presenting to a window. `window` is typically an
    /// `Arc<winit::window::Window>`; it is cloned so the instance also holds
    /// its display connection (needed by GL on Wayland).
    pub fn new_windowed<W>(
        window: W,
        width: u32,
        height: u32,
        config: &RendererConfig,
    ) -> Result<Self>
    where
        W: wgpu::rwh::HasWindowHandle + wgpu::rwh::HasDisplayHandle + Clone + Send + Sync + 'static,
    {
        let display: Box<dyn wgpu::rwh::HasDisplayHandle + Send + Sync> = Box::new(window.clone());
        Renderer::new_with_surface_target(window, Some(display), width, height, config)
    }
}

impl<'w> Renderer<'w> {
    /// Renderer presenting to any [`wgpu::SurfaceTarget`]. `display` should be
    /// the same display connection the target uses (or `None`).
    pub fn new_with_surface_target(
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        display: Option<Box<dyn wgpu::rwh::HasDisplayHandle + Send + Sync>>,
        width: u32,
        height: u32,
        config: &RendererConfig,
    ) -> Result<Self> {
        let instance = GpuContext::create_instance(&config.context, display);
        let surface = instance.create_surface(target)?;
        let ctx = GpuContext::new_with_instance(instance, &config.context, Some(&surface))?;
        let state = Self::make_surface_state(&ctx, surface, width, height, config.present_mode)?;
        Self::from_parts(ctx, Some(state), config)
    }

    fn make_surface_state(
        ctx: &GpuContext,
        surface: wgpu::Surface<'w>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<SurfaceState<'w>> {
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or(RenderError::SurfaceUnsupported)?;
        let present_mode = if caps.present_modes.contains(&present_mode) {
            present_mode
        } else {
            tracing::warn!(?present_mode, "present mode unsupported, using Fifo");
            wgpu::PresentMode::Fifo
        };
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        tracing::info!(?format, ?present_mode, width, height, "surface configured");
        Ok(SurfaceState {
            surface,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width,
                height,
                desired_maximum_frame_latency: 2,
                present_mode,
                alpha_mode,
                view_formats: Vec::new(),
            },
            dirty: true,
        })
    }

    fn from_parts(
        ctx: GpuContext,
        surface: Option<SurfaceState<'w>>,
        config: &RendererConfig,
    ) -> Result<Self> {
        let device = &ctx.device;
        let shared = SharedLayouts::new(device);
        let samplers = Samplers::new(device);
        let owner = next_owner_id();
        let mut textures = TextureStore::new(owner);
        let white = textures.upload(
            device,
            &ctx.queue,
            &shared.texture,
            &samplers,
            &TextureUpload {
                label: Some("tessera-white"),
                ..TextureUpload::rgba8(1, 1, &[255; 4])
            },
        )?;
        let uniform_stride = align_up(
            size_of::<Globals>() as u64,
            u64::from(device.limits().min_uniform_buffer_offset_alignment),
        );
        let uniforms = StreamBuffer::new(device, "tessera-globals", wgpu::BufferUsages::UNIFORM);
        let globals_bg = Self::make_globals_bg(device, &shared, &uniforms.buffer);
        let instances =
            StreamBuffer::new(device, "tessera-quad-instances", wgpu::BufferUsages::VERTEX);
        let lines = StreamBuffer::new(device, "tessera-lines", wgpu::BufferUsages::VERTEX);
        let [r, g, b, a] = config.clear_color.map(f64::from);
        let mut renderer = Self {
            owner,
            present_mode: config.present_mode,
            ctx,
            surface,
            shared,
            pipelines: PipelineCache::default(),
            samplers,
            textures,
            white,
            depth: SizeCache::new(MAX_DEPTH_CACHE),
            uniforms,
            globals_bg,
            uniform_stride,
            instances,
            lines,
            clear_color: wgpu::Color { r, g, b, a },
            frame_index: 0,
            frame_token: Arc::new(()),
        };
        renderer.apply_surface_config();
        renderer.ctx.errors.check()?;
        Ok(renderer)
    }

    fn make_globals_bg(
        device: &wgpu::Device,
        shared: &SharedLayouts,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessera-globals"),
            layout: &shared.globals,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<Globals>() as u64),
                }),
            }],
        })
    }

    /// The GPU context (device, queue, adapter info).
    #[must_use]
    pub fn context(&self) -> &GpuContext {
        &self.ctx
    }

    /// Information about the selected adapter.
    #[must_use]
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.ctx.info
    }

    /// Whether this renderer presents to a surface.
    #[must_use]
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    /// Current surface size, or `None` when headless.
    #[must_use]
    pub fn surface_size(&self) -> Option<(u32, u32)> {
        self.surface
            .as_ref()
            .map(|s| (s.config.width, s.config.height))
    }

    /// Set the default clear colour.
    pub fn set_clear_color(&mut self, color: Rgba) {
        let [r, g, b, a] = color.map(f64::from);
        self.clear_color = wgpu::Color { r, g, b, a };
    }

    fn frame_alive(&self) -> bool {
        Arc::strong_count(&self.frame_token) > 1
    }

    fn apply_surface_config(&mut self) {
        let alive = self.frame_alive();
        let Some(state) = self.surface.as_mut() else {
            return;
        };
        if !state.dirty || alive || state.config.width == 0 || state.config.height == 0 {
            return;
        }
        state.surface.configure(&self.ctx.device, &state.config);
        state.dirty = false;
    }

    /// Handle a window resize. A zero dimension (minimised window) pauses
    /// rendering until a non-zero size arrives. Headless: no-op.
    pub fn resize(&mut self, width: u32, height: u32) {
        let max = self.ctx.device.limits().max_texture_dimension_2d;
        if let Some(state) = self.surface.as_mut() {
            let (w, h) = (width.min(max), height.min(max));
            let old = (state.config.width, state.config.height);
            if (w, h) != old {
                tracing::debug!(w, h, "surface resize");
                state.config.width = w;
                state.config.height = h;
                state.dirty = true;
                // The old window-sized depth buffer is dead weight now
                // (interactive resizing would otherwise keep several
                // full-window buffers alive). An offscreen target of the
                // same size simply re-creates it on its next frame.
                self.depth.remove(old);
            }
        }
        self.apply_surface_config();
    }

    /// Replace a lost surface (after [`RenderError::SurfaceLost`]).
    pub fn replace_surface(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let surface = self.ctx.instance.create_surface(target)?;
        if !self.ctx.adapter.is_surface_supported(&surface) {
            return Err(RenderError::SurfaceUnsupported);
        }
        self.surface = None;
        let state = Self::make_surface_state(&self.ctx, surface, width, height, self.present_mode)?;
        self.surface = Some(state);
        self.apply_surface_config();
        Ok(())
    }

    /// Upload an RGBA8 texture.
    pub fn upload_texture(&mut self, desc: &TextureUpload<'_>) -> Result<TextureId> {
        self.textures.upload(
            &self.ctx.device,
            &self.ctx.queue,
            &self.shared.texture,
            &self.samplers,
            desc,
        )
    }

    /// Overwrite all texels of an existing texture (same size).
    pub fn update_texture(&mut self, id: TextureId, rgba: &[u8]) -> Result<()> {
        self.textures.update(&self.ctx.queue, id, rgba)
    }

    /// Size of a texture.
    #[must_use]
    pub fn texture_size(&self, id: TextureId) -> Option<(u32, u32)> {
        self.textures.get(id).map(|t| (t.width, t.height))
    }

    /// Destroy a texture. Returns `false` for stale/unknown handles (and for
    /// handles issued by another renderer). The built-in white texture
    /// cannot be destroyed. Destroying a registered render target only
    /// removes the sampling handle; the target stays usable.
    pub fn destroy_texture(&mut self, id: TextureId) -> bool {
        if id == self.white {
            return false;
        }
        self.textures.remove(id)
    }

    /// Make an offscreen target drawable as a texture (render-to-texture:
    /// UI composition, minimaps, previews). The returned handle samples the
    /// target's current contents whenever a frame is drawn; it cannot be
    /// written with [`Self::update_texture`]. A quad that samples the very
    /// target its frame renders into is drawn white instead (a GPU feedback
    /// loop is not allowed).
    pub fn register_target_texture(
        &mut self,
        target: &OffscreenTarget,
        filter: Filter,
        wrap: Wrap,
    ) -> Result<TextureId> {
        self.check_target(target)?;
        self.textures.register_target(
            &self.ctx.device,
            &self.shared.texture,
            &self.samplers,
            &target.texture,
            &target.view,
            filter,
            wrap,
        )
    }

    fn check_target(&self, target: &OffscreenTarget) -> Result<()> {
        if target.owner == self.owner {
            Ok(())
        } else {
            Err(RenderError::ForeignHandle("offscreen target"))
        }
    }

    /// Number of live textures (including the built-in white texture).
    #[must_use]
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Create an offscreen colour target usable with
    /// [`Self::begin_offscreen_frame`] and [`Self::read_pixels`].
    pub fn create_offscreen_target(
        &self,
        width: u32,
        height: u32,
        format: OffscreenFormat,
    ) -> Result<OffscreenTarget> {
        let max = self.ctx.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(RenderError::InvalidTarget(format!(
                "{width}x{height} (limit {max})"
            )));
        }
        let format = format.wgpu();
        let texture = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tessera-offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(OffscreenTarget {
            owner: self.owner,
            texture,
            view,
            width,
            height,
            format,
        })
    }

    /// Begin a frame on the window surface.
    ///
    /// Returns `Ok(None)` when no frame should be drawn right now (minimised,
    /// occluded, timeout, or a previous surface frame is still alive).
    pub fn begin_frame(&mut self) -> Result<Option<Frame>> {
        if self.surface.is_none() {
            return Err(RenderError::NoSurface);
        }
        if self.frame_alive() {
            tracing::warn!("begin_frame called while a surface frame is still alive");
            return Ok(None);
        }
        self.apply_surface_config();
        let Some(state) = self.surface.as_mut() else {
            return Err(RenderError::NoSurface);
        };
        if state.dirty {
            // Zero-sized: nothing to draw until resized.
            return Ok(None);
        }
        let texture = match state.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                state.dirty = true;
                t
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                state.dirty = true;
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err(RenderError::SurfaceLost),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::SurfaceValidation);
            }
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("tessera-surface-view"),
            ..Default::default()
        });
        let (width, height, format) =
            (state.config.width, state.config.height, state.config.format);
        Ok(Some(Frame {
            owner: self.owner,
            target: FrameTarget::Surface(texture, Arc::clone(&self.frame_token)),
            view,
            format,
            width,
            height,
            passes: Vec::new(),
        }))
    }

    /// Begin a frame rendering into an offscreen target.
    ///
    /// Fails with [`RenderError::ForeignHandle`] when `target` was created
    /// by a different renderer.
    pub fn begin_offscreen_frame(&mut self, target: &OffscreenTarget) -> Result<Frame> {
        self.check_target(target)?;
        Ok(Frame {
            owner: self.owner,
            target: FrameTarget::Offscreen,
            view: target.view.clone(),
            format: target.format,
            width: target.width,
            height: target.height,
            passes: Vec::new(),
        })
    }

    fn depth_view(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        if let Some(view) = self.depth.get((width, height)) {
            return view.clone();
        }
        let texture = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tessera-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth.insert((width, height), view.clone());
        view
    }

    /// Record, submit and (for surface frames) present a frame.
    ///
    /// A frame begun on a different renderer is rejected with
    /// [`RenderError::ForeignHandle`] before anything is recorded (a surface
    /// texture in it is dropped without being presented).
    pub fn end_frame(&mut self, frame: Frame) -> Result<FrameStats> {
        if frame.owner != self.owner {
            return Err(RenderError::ForeignHandle("frame"));
        }
        let Frame {
            owner: _,
            target,
            view,
            format,
            width,
            height,
            mut passes,
        } = frame;

        if passes.is_empty() {
            // Always produce defined contents.
            passes.push(crate::frame::Pass::new("clear".into(), PassSpace::Screen));
        }

        // ---- gather streaming data -------------------------------------------------
        let stride = self.uniform_stride;
        let pass_count = u64::try_from(passes.len()).unwrap_or(u64::MAX);
        // Dynamic uniform offsets are u32.
        let max_offset = u64::from(u32::MAX);
        let max_passes = max_offset / stride.max(1);
        let too_many = RenderError::LimitExceeded {
            what: "passes per frame",
            requested: pass_count,
            max: max_passes,
        };
        let uniform_bytes = match pass_count.checked_mul(stride) {
            Some(b) if b <= max_offset => b,
            _ => return Err(too_many),
        };
        let mut uniform_data = vec![0u8; usize::try_from(uniform_bytes).unwrap_or(0)];
        let mut instance_data: Vec<QuadInstance> = Vec::new();
        let mut line_data: Vec<LineVertex> = Vec::new();
        let mut instance_base = Vec::with_capacity(passes.len());
        let mut line_base = Vec::with_capacity(passes.len());
        let mut needs_depth = false;
        let mut needs_flat = false;

        for (i, pass) in passes.iter().enumerate() {
            let view_proj = match pass.space {
                PassSpace::World { view_proj } => {
                    needs_depth = true;
                    view_proj
                }
                PassSpace::Screen => {
                    needs_flat = true;
                    screen_projection(width, height)
                }
            };
            let globals = Globals {
                view_proj: view_proj.to_cols_array_2d(),
            };
            let off = i * stride as usize;
            if let Some(dst) = uniform_data.get_mut(off..off + size_of::<Globals>()) {
                dst.copy_from_slice(bytemuck::bytes_of(&globals));
            }
            instance_base.push(instance_data.len());
            instance_data.extend(pass.quads.iter().map(|q| QuadInstance {
                model: q.transform.to_cols_array_2d(),
                color: q.color,
                uv_rect: q.uv_rect,
            }));
            line_base.push(line_data.len());
            // A dangling odd vertex is ignored (line list needs pairs).
            let even = pass.lines.len() & !1;
            line_data.extend_from_slice(&pass.lines[..even]);
        }

        let device = &self.ctx.device;
        if self.uniforms.reserve(device, uniform_bytes)? {
            self.globals_bg = Self::make_globals_bg(device, &self.shared, &self.uniforms.buffer);
        }
        let inst_bytes: &[u8] = bytemuck::cast_slice(&instance_data);
        let line_bytes: &[u8] = bytemuck::cast_slice(&line_data);
        self.instances.reserve(device, inst_bytes.len() as u64)?;
        self.lines.reserve(device, line_bytes.len() as u64)?;
        let queue = &self.ctx.queue;
        queue.write_buffer(&self.uniforms.buffer, 0, &uniform_data);
        if !inst_bytes.is_empty() {
            queue.write_buffer(&self.instances.buffer, 0, inst_bytes);
        }
        if !line_bytes.is_empty() {
            queue.write_buffer(&self.lines.buffer, 0, line_bytes);
        }

        // ---- pipelines & attachments ----------------------------------------------
        if needs_depth {
            self.pipelines
                .get(&self.ctx.device, &self.shared, format, true);
        }
        if needs_flat {
            self.pipelines
                .get(&self.ctx.device, &self.shared, format, false);
        }
        let depth_view = needs_depth.then(|| self.depth_view(width, height));

        // ---- record -----------------------------------------------------------------
        let mut stats = FrameStats {
            frame_index: self.frame_index,
            passes: u32::try_from(passes.len()).unwrap_or(u32::MAX),
            ..FrameStats::default()
        };
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tessera-frame"),
            });
        let mut depth_cleared = false;
        let mut warned_missing = false;
        for (i, pass) in passes.iter().enumerate() {
            let is_world = matches!(pass.space, PassSpace::World { .. });
            let load = match (pass.clear_color, i) {
                (Some(c), _) => wgpu::LoadOp::Clear(c),
                (None, 0) => wgpu::LoadOp::Clear(self.clear_color),
                (None, _) => wgpu::LoadOp::Load,
            };
            let depth_attachment = match (&depth_view, is_world) {
                (Some(dv), true) => {
                    let depth_load = if depth_cleared && !pass.clear_depth {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    };
                    depth_cleared = true;
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: dv,
                        depth_ops: Some(wgpu::Operations {
                            load: depth_load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    })
                }
                _ => None,
            };
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&pass.label),
                color_attachments: &color_attachments,
                depth_stencil_attachment: depth_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if pass.quads.is_empty() && pass.lines.len() < 2 {
                continue;
            }
            let Some(set) = self.pipelines.lookup(format, is_world) else {
                continue;
            };
            let uniform_offset = u32::try_from(i as u64 * stride).unwrap_or(0);
            rp.set_bind_group(0, &self.globals_bg, &[uniform_offset]);

            if !pass.quads.is_empty() {
                rp.set_pipeline(&set.quad);
                let base = instance_base.get(i).copied().unwrap_or(0);
                for (tex, range) in texture_runs(&pass.quads) {
                    let gpu_tex = match tex {
                        None => self.textures.get(self.white),
                        Some(id) => match self.textures.get(id) {
                            Some(t) if t.target_view.as_ref() != Some(&view) => Some(t),
                            found => {
                                if !warned_missing {
                                    let reason = if found.is_some() {
                                        "texture is this frame's own render target"
                                    } else {
                                        "unknown texture"
                                    };
                                    tracing::warn!(?id, reason, "drawing white instead");
                                    warned_missing = true;
                                }
                                self.textures.get(self.white)
                            }
                        },
                    };
                    let Some(gpu_tex) = gpu_tex else { continue };
                    rp.set_bind_group(1, &gpu_tex.bind_group, &[]);
                    let start =
                        (base + range.start as usize) as u64 * size_of::<QuadInstance>() as u64;
                    let end = (base + range.end as usize) as u64 * size_of::<QuadInstance>() as u64;
                    rp.set_vertex_buffer(0, self.instances.buffer.slice(start..end));
                    rp.draw(0..4, 0..(range.end - range.start));
                    stats.draw_calls += 1;
                    stats.quads += range.end - range.start;
                }
            }

            let line_vertices = pass.lines.len() & !1;
            if line_vertices > 0 {
                rp.set_pipeline(&set.line);
                let base = line_base.get(i).copied().unwrap_or(0);
                let vsize = size_of::<LineVertex>() as u64;
                let start = base as u64 * vsize;
                let end = (base + line_vertices) as u64 * vsize;
                rp.set_vertex_buffer(0, self.lines.buffer.slice(start..end));
                let n = u32::try_from(line_vertices).unwrap_or(u32::MAX);
                rp.draw(0..n, 0..1);
                stats.draw_calls += 1;
                stats.lines += n / 2;
            }
        }

        self.ctx.queue.submit(Some(encoder.finish()));
        drop(view);
        if let FrameTarget::Surface(texture, token) = target {
            self.ctx.queue.present(texture);
            drop(token);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        // A Suboptimal acquire marks the surface dirty; reconfigure now that
        // the frame is gone.
        self.apply_surface_config();
        self.ctx.errors.check()?;
        tracing::trace!(?stats, "frame submitted");
        Ok(stats)
    }

    /// Copy an offscreen target back to the CPU (blocks until the GPU is done).
    ///
    /// Fails with [`RenderError::ForeignHandle`] when `target` was created
    /// by a different renderer.
    pub fn read_pixels(&self, target: &OffscreenTarget) -> Result<RgbaImage> {
        self.check_target(target)?;
        let (w, h) = (target.width, target.height);
        let unpadded = u64::from(w) * 4;
        let padded = align_up(unpadded, u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let size = padded
            .checked_mul(u64::from(h))
            .ok_or_else(|| RenderError::Readback("size overflow".into()))?;
        let padded_u32 =
            u32::try_from(padded).map_err(|_| RenderError::Readback("row too wide".into()))?;
        let device = &self.ctx.device;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessera-readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tessera-readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.ctx.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            // Receiver only disappears if read_pixels already returned.
            let _ = tx.send(r);
        });
        self.ctx.device.poll(wgpu::PollType::wait_indefinitely())?;
        match rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(RenderError::Readback(e.to_string())),
            Err(_) => return Err(RenderError::Readback("map callback dropped".into())),
        }
        let data = {
            let mapped = slice
                .get_mapped_range()
                .map_err(|e| RenderError::Readback(e.to_string()))?;
            unpad_rows(&mapped, w, h, padded)
        };
        buffer.unmap();
        self.ctx.errors.check()?;
        let data = data.ok_or_else(|| RenderError::Readback("short buffer".into()))?;
        Ok(RgbaImage {
            width: w,
            height: h,
            data,
        })
    }
}

/// Strip per-row padding from a readback buffer.
fn unpad_rows(src: &[u8], width: u32, height: u32, padded_row: u64) -> Option<Vec<u8>> {
    let row = width as usize * 4;
    let padded_row = usize::try_from(padded_row).ok()?;
    let mut out = Vec::with_capacity(row.checked_mul(height as usize)?);
    for y in 0..height as usize {
        let start = y.checked_mul(padded_row)?;
        out.extend_from_slice(src.get(start..start + row)?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpad() {
        // 2x2 image, rows padded to 12 bytes.
        let src = [
            1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, //
            9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0,
        ];
        assert_eq!(
            unpad_rows(&src, 2, 2, 12),
            Some((1..=16).collect::<Vec<u8>>())
        );
        assert_eq!(unpad_rows(&src[..19], 2, 2, 12), None);
    }

    #[test]
    fn alignment() {
        assert_eq!(align_up(64, 256), 256);
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(257, 256), 512);
        assert_eq!(align_up(5, 0), 5);
    }

    #[test]
    fn image_pixel_bounds() {
        let img = RgbaImage {
            width: 1,
            height: 1,
            data: vec![1, 2, 3, 4],
        };
        assert_eq!(img.pixel(0, 0), Some([1, 2, 3, 4]));
        assert_eq!(img.pixel(1, 0), None);
    }

    #[test]
    fn image_pixel_never_overflows() {
        // Public fields can disagree with `data`; huge dimensions used to
        // overflow the index arithmetic and panic.
        let img = RgbaImage {
            width: u32::MAX,
            height: u32::MAX,
            data: vec![],
        };
        assert_eq!(img.pixel(u32::MAX - 1, u32::MAX - 1), None);
        assert_eq!(img.pixel(0, 0), None);
        let short = RgbaImage {
            width: 2,
            height: 2,
            data: vec![0; 15],
        };
        assert_eq!(short.pixel(1, 1), None);
        assert_eq!(short.pixel(0, 1), Some([0; 4]));
    }

    #[test]
    fn size_cache_is_lru_and_removable() {
        let mut c: SizeCache<u32> = SizeCache::new(2);
        c.insert((1, 1), 1);
        c.insert((2, 2), 2);
        // Touch (1,1) so (2,2) becomes least recently used.
        assert_eq!(c.get((1, 1)), Some(&1));
        c.insert((3, 3), 3);
        assert_eq!(c.get((2, 2)), None, "LRU entry evicted");
        assert_eq!(c.get((1, 1)), Some(&1), "recently used entry kept");
        assert_eq!(c.get((3, 3)), Some(&3));
        // Re-inserting a size replaces it instead of duplicating.
        c.insert((3, 3), 33);
        assert_eq!(c.len(), 2);
        assert_eq!(c.get((3, 3)), Some(&33));
        // Removing the old surface size frees it immediately.
        c.remove((1, 1));
        assert_eq!(c.len(), 1);
        assert_eq!(c.get((1, 1)), None);
        let mut zero: SizeCache<u8> = SizeCache::new(0);
        zero.insert((1, 1), 1);
        assert_eq!(zero.len(), 1);
    }
}
