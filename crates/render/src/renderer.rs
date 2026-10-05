// SPDX-License-Identifier: GPL-3.0-or-later
//! The [`Renderer`]: owns the GPU context, optional presentation surface,
//! built-in pipelines, textures and per-frame streaming buffers.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{Mat4, Vec3, Vec4};

use crate::arena::{ArenaStats, MeshArena, MeshConfig, MeshId};
use crate::context::{ContextOptions, GpuContext};
use crate::error::{RenderError, Result};
use crate::frame::{Frame, FrameTarget, Pass, PassSpace, Rgba, screen_projection, texture_runs};
use crate::material::{AlphaMode, DyeMask, MeshShading};
use crate::mesh::{MeshData, MeshInfo, PreparedMesh};
use crate::mesh_pipelines::{
    Cull, DrawContext, DrawUniform, Kind, MeshKey, MeshLayouts, MeshPipelineCache, is_mirrored,
};
use crate::pipelines::{
    DEPTH_CLEAR, DEPTH_FORMAT, Globals, LineVertex, PipelineCache, QuadInstance, SharedLayouts,
};
use crate::skin::{MeshSkin, PALETTE_BINDING_BYTES, PaletteData};
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
    /// Preferred present mode; falls back to `Fifo` when unsupported. `AutoVsync` and
    /// `AutoNoVsync` are resolved against the surface's modes (`FifoRelaxed`, else `Fifo`;
    /// `Immediate`, else `Mailbox`, else `Fifo`).
    pub present_mode: wgpu::PresentMode,
    /// Colour the first pass clears to when it does not set its own.
    pub clear_color: Rgba,
    /// Mesh arena sizes and per-mesh limits.
    pub meshes: MeshConfig,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            context: ContextOptions::default(),
            present_mode: wgpu::PresentMode::Fifo,
            clear_color: [0.0, 0.0, 0.0, 1.0],
            meshes: MeshConfig::default(),
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
    /// Mesh surface draws (one per submesh and face side; overlays not
    /// included).
    pub mesh_draws: u32,
    /// Triangles submitted by mesh surface draws.
    pub triangles: u64,
    /// Mesh surface draws that were skinned with a palette (included in
    /// `mesh_draws`).
    pub skinned_draws: u32,
    /// Skinning palettes uploaded with the frame.
    pub palettes: u32,
    /// Bytes of skinning-palette data uploaded with the frame.
    pub palette_bytes: u64,
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
        Self::with_size(device, label, usage, 4096)
    }

    fn with_size(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        size: u64,
    ) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        let buffer = Self::alloc(device, label, usage, size);
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
    mesh_layouts: MeshLayouts,
    mesh_pipelines: MeshPipelineCache,
    arena: MeshArena,
    draw_uniforms: StreamBuffer,
    /// Skinning palettes of the current frame (dynamic-offset uniform).
    palettes: StreamBuffer,
    /// Draw uniforms (binding 0) and palettes (binding 1).
    draw_bg: wgpu::BindGroup,
    draw_stride: u64,
    /// Start alignment of each palette (the dynamic uniform offset alignment).
    palette_align: u64,
    /// Per-frame cap on palette bytes.
    palette_budget: u64,
    /// Serial stamped into the next frame (and its palettes, together with
    /// `owner`).
    next_frame_serial: u64,
    /// Mesh material bind groups keyed by the four resolved textures, with
    /// the index of the frame that last used them.
    material_bgs: HashMap<[TextureId; 4], (wgpu::BindGroup, u64)>,
    /// Size at which `material_bgs` evicts entries not used this frame.
    material_bg_cap: usize,
    clear_color: wgpu::Color,
    frame_index: u64,
    /// Cloned into every surface [`Frame`]; strong count > 1 means a surface
    /// texture is still alive and the surface must not be reconfigured.
    frame_token: Arc<()>,
}

const MAX_DEPTH_CACHE: usize = 4;
/// Per-frame budget of skinning-palette bytes (about 5,000 palettes of 256
/// bones, or 25,000 of 89 bones).
const MAX_PALETTE_BYTES_PER_FRAME: u64 = 64 << 20;
/// Material bind groups kept before entries unused in the current frame are
/// evicted. A frame that alone uses more raises the threshold to twice its
/// working set, so a large scene never re-creates its bind groups every
/// frame and eviction stays amortised O(1) per insert.
const MAX_MATERIAL_BIND_GROUPS: usize = 1024;

/// Run `f` inside wgpu error scopes (validation, out-of-memory, internal)
/// and return its result together with the first GPU error it caused.
/// Errors raised by `f` are attributed to it instead of reaching the shared
/// [`crate::context`] error sink, where an unrelated later call would
/// report them.
fn with_error_scope<T>(device: &wgpu::Device, f: impl FnOnce() -> T) -> (T, Option<String>) {
    let oom = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let out = f();
    // Scopes pop in reverse order of pushing; on native the futures are
    // ready immediately.
    let v = pollster::block_on(validation.pop());
    let i = pollster::block_on(internal.pop());
    let o = pollster::block_on(oom.pop());
    (out, v.or(i).or(o).map(|e| e.to_string()))
}

/// One mesh surface draw, resolved during the gather phase of a frame.
#[derive(Debug, Clone)]
struct MeshItem {
    key: MeshKey,
    page: usize,
    indices: Range<u32>,
    uniform_offset: u32,
    /// Dynamic offset of the palette (0 for static draws).
    palette_offset: u32,
    /// Index into the frame's material bind group list.
    material: usize,
    /// NDC depth of the submesh's bounding-sphere centre (blend sorting).
    depth: f32,
}

#[derive(Debug, Clone)]
enum OverlayDraw {
    /// Line-list edge indices.
    Wire(Range<u32>),
    /// Mesh vertices drawn as instances.
    Normals(Range<u32>),
}

#[derive(Debug, Clone)]
struct OverlayItem {
    key: MeshKey,
    page: usize,
    uniform_offset: u32,
    palette_offset: u32,
    draw: OverlayDraw,
}

/// Every mesh draw of one pass, in drawing order per bucket.
#[derive(Debug, Default)]
struct PassMeshes {
    /// Opaque, then alpha-tested.
    surfaces: Vec<MeshItem>,
    overlays: Vec<OverlayItem>,
    /// Sorted back to front.
    blended: Vec<MeshItem>,
}

impl PassMeshes {
    fn is_empty(&self) -> bool {
        self.surfaces.is_empty() && self.overlays.is_empty() && self.blended.is_empty()
    }
}

/// Warn-once flags for one frame.
#[derive(Debug, Default)]
struct FrameWarnings {
    texture: bool,
    mesh: bool,
    submesh: bool,
    wire: bool,
    screen_mesh: bool,
    palette: bool,
}

/// Camera position for a world pass: the explicit eye, else the centre of
/// the near plane (NDC depth 1 under reversed depth) recovered from
/// `view_proj`.
fn pass_eye(pass: &Pass, view_proj: Mat4) -> Vec3 {
    if let Some(e) = pass.eye
        && e.is_finite()
    {
        return e;
    }
    let p = view_proj.inverse() * Vec4::new(0.0, 0.0, 1.0, 1.0);
    let e = p.truncate() / p.w;
    if e.is_finite() { e } else { Vec3::ZERO }
}

/// Sort key of blended draws (larger = farther): the negated NDC depth,
/// since world passes use reversed depth (near = 1, far = 0). Points behind
/// the camera sort first.
fn sort_depth(view_proj: Mat4, point: Vec3) -> f32 {
    let clip = view_proj * point.extend(1.0);
    if clip.w > 1e-6 {
        -(clip.z / clip.w)
    } else {
        f32::INFINITY
    }
}

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
        let present_mode = {
            let chosen = choose_present_mode(present_mode, &caps.present_modes);
            if !matches!(
                present_mode,
                wgpu::PresentMode::AutoVsync | wgpu::PresentMode::AutoNoVsync
            ) && chosen != present_mode
            {
                tracing::warn!(?present_mode, "present mode unsupported, using Fifo");
            }
            chosen
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
        let mesh_layouts = MeshLayouts::new(device, &shared);
        let draw_stride = align_up(
            size_of::<DrawUniform>() as u64,
            u64::from(device.limits().min_uniform_buffer_offset_alignment),
        );
        let draw_uniforms =
            StreamBuffer::new(device, "tessera-mesh-draws", wgpu::BufferUsages::UNIFORM);
        let palettes = StreamBuffer::with_size(
            device,
            "tessera-skin-palettes",
            wgpu::BufferUsages::UNIFORM,
            PALETTE_BINDING_BYTES,
        );
        let draw_bg = Self::make_draw_bg(
            device,
            &mesh_layouts,
            &draw_uniforms.buffer,
            &palettes.buffer,
        );
        let palette_align = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(16);
        let palette_budget = MAX_PALETTE_BYTES_PER_FRAME
            .min(device.limits().max_buffer_size)
            .min(u64::from(u32::MAX));
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
            mesh_layouts,
            mesh_pipelines: MeshPipelineCache::default(),
            arena: MeshArena::new(owner, config.meshes),
            draw_uniforms,
            palettes,
            draw_bg,
            draw_stride,
            palette_align,
            palette_budget,
            next_frame_serial: 0,
            material_bgs: HashMap::new(),
            material_bg_cap: MAX_MATERIAL_BIND_GROUPS,
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

    fn make_draw_bg(
        device: &wgpu::Device,
        layouts: &MeshLayouts,
        buffer: &wgpu::Buffer,
        palettes: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessera-mesh-draws"),
            layout: &layouts.draw,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<DrawUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: palettes,
                        offset: 0,
                        size: wgpu::BufferSize::new(PALETTE_BINDING_BYTES),
                    }),
                },
            ],
        })
    }

    fn new_frame_parts(&mut self) -> (u64, PaletteData) {
        let serial = self.next_frame_serial;
        self.next_frame_serial = self.next_frame_serial.wrapping_add(1);
        (
            serial,
            PaletteData::new(self.palette_align, self.palette_budget),
        )
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
    ///
    /// A GPU error raised by the upload itself (e.g. out of memory) is
    /// returned as [`RenderError::Gpu`] and nothing is kept.
    pub fn upload_texture(&mut self, desc: &TextureUpload<'_>) -> Result<TextureId> {
        let (result, gpu_error) = with_error_scope(&self.ctx.device, || {
            self.textures.upload(
                &self.ctx.device,
                &self.ctx.queue,
                &self.shared.texture,
                &self.samplers,
                desc,
            )
        });
        match (result, gpu_error) {
            (result, None) => result,
            (Ok(id), Some(e)) => {
                self.textures.remove(id);
                Err(RenderError::Gpu(e))
            }
            (Err(_), Some(e)) => Err(RenderError::Gpu(e)),
        }
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
        self.material_bgs.retain(|key, _| !key.contains(&id));
        self.textures.remove(id)
    }

    /// Validate `data` and copy it into the shared mesh buffers.
    ///
    /// Fails with [`RenderError::InvalidMesh`] for inconsistent streams,
    /// [`RenderError::LimitExceeded`] when a [`crate::MeshLimits`] bound or
    /// the arena's byte cap ([`MeshConfig::max_total_bytes`]) would be
    /// exceeded, and [`RenderError::Gpu`] when the GPU rejects the copy.
    /// Nothing is kept on failure.
    ///
    /// The CPU part (validation, packing, normals, edges, bounds) runs on
    /// the calling thread; to keep it off the render thread, call
    /// [`MeshData::prepare`] elsewhere and pass the result to
    /// [`Self::upload_prepared`].
    pub fn upload_mesh(&mut self, data: &MeshData) -> Result<MeshId> {
        let prepared = data.prepare(self.arena.config())?;
        let id = self.upload_prepared(prepared)?;
        tracing::trace!(?id, label = data.label.as_deref(), "uploaded mesh");
        Ok(id)
    }

    /// Copy a mesh prepared with [`MeshData::prepare`] (possibly on another
    /// thread) into the shared mesh buffers.
    ///
    /// The mesh's counts are re-checked against this renderer's
    /// [`MeshConfig::limits`] (it may have been prepared under a different
    /// configuration). Its wireframe edges are dropped when this renderer's
    /// [`MeshConfig::wireframe_edges`] is off; a mesh prepared without edges
    /// uploads normally but shows no wireframe. Errors and cleanup as for
    /// [`Self::upload_mesh`].
    pub fn upload_prepared(&mut self, mut prepared: PreparedMesh) -> Result<MeshId> {
        let config = *self.arena.config();
        let info = prepared.info();
        let limits = config.limits;
        for (what, requested, max) in [
            ("mesh vertices", info.vertex_count, limits.max_vertices),
            ("mesh indices", info.index_count, limits.max_indices),
            (
                "mesh submeshes",
                u32::try_from(info.submeshes.len()).unwrap_or(u32::MAX),
                // The implicit whole-mesh submesh is always allowed.
                limits.max_submeshes.max(1),
            ),
        ] {
            if requested > max {
                return Err(RenderError::LimitExceeded {
                    what,
                    requested: u64::from(requested),
                    max: u64::from(max),
                });
            }
        }
        if !config.wireframe_edges && info.edge_index_count > 0 {
            prepared.strip_edges();
        }
        let (result, gpu_error) = with_error_scope(&self.ctx.device, || {
            self.arena
                .insert(&self.ctx.device, &self.ctx.queue, prepared)
        });
        match (result, gpu_error) {
            (result, None) => result,
            (Ok(id), Some(e)) => {
                // Also releases a page created for this mesh.
                self.arena.discard(id);
                Err(RenderError::Gpu(e))
            }
            (Err(_), Some(e)) => Err(RenderError::Gpu(e)),
        }
    }

    /// Validate a skinned mesh (the streams of `data` plus the bone
    /// influences of `skin`) and copy it into the shared mesh buffers. The
    /// influences go to a skin buffer parallel to the vertex buffer of the
    /// mesh's page.
    ///
    /// Draw it with a palette ([`crate::MeshDraw::with_skin`]) to skin it,
    /// or without one to show its bind pose. Errors as for
    /// [`Self::upload_mesh`], plus [`RenderError::InvalidSkin`] for bad
    /// skin streams (see [`MeshSkin::validate`]). To do the CPU work
    /// elsewhere, call [`MeshData::prepare_skinned`] and
    /// [`Self::upload_prepared`].
    pub fn upload_skinned_mesh(&mut self, data: &MeshData, skin: &MeshSkin) -> Result<MeshId> {
        let prepared = data.prepare_skinned(skin, self.arena.config())?;
        let id = self.upload_prepared(prepared)?;
        tracing::trace!(?id, label = data.label.as_deref(), "uploaded skinned mesh");
        Ok(id)
    }

    /// Free a mesh's buffer space. Returns `false` for stale or foreign
    /// handles. Draws of a destroyed mesh are skipped (with a warning).
    pub fn destroy_mesh(&mut self, id: MeshId) -> bool {
        self.arena.remove(id)
    }

    /// Bounds and stream summary of a live mesh.
    #[must_use]
    pub fn mesh_info(&self, id: MeshId) -> Option<&MeshInfo> {
        self.arena.get(id).map(|m| &m.info)
    }

    /// Memory use of the shared mesh buffers.
    #[must_use]
    pub fn mesh_stats(&self) -> ArenaStats {
        self.arena.stats()
    }

    /// Resolve an optional material texture for a frame rendering into
    /// `view`: `None` when absent, stale, foreign or the frame's own target.
    fn resolve_texture(
        &self,
        id: Option<TextureId>,
        view: &wgpu::TextureView,
        warnings: &mut FrameWarnings,
    ) -> Option<TextureId> {
        let id = id?;
        match self.textures.get(id) {
            Some(t) if t.target_view.as_ref() != Some(view) => Some(id),
            found => {
                if !warnings.texture {
                    let reason = if found.is_some() {
                        "texture is this frame's own render target"
                    } else {
                        "unknown texture"
                    };
                    tracing::warn!(?id, reason, "mesh material texture ignored");
                    warnings.texture = true;
                }
                None
            }
        }
    }

    /// Bind group for four resolved textures (cached across frames).
    fn material_bind_group(&mut self, key: [TextureId; 4]) -> Option<wgpu::BindGroup> {
        let frame = self.frame_index;
        if let Some((bg, used)) = self.material_bgs.get_mut(&key) {
            *used = frame;
            return Some(bg.clone());
        }
        let texs: Vec<_> = key.iter().filter_map(|&id| self.textures.get(id)).collect();
        let [base, normal, dye, spec] = texs.as_slice() else {
            return None;
        };
        let bg = self
            .ctx
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tessera-mesh-material"),
                layout: &self.mesh_layouts.material,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&base.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&normal.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&dye.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&spec.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&base.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&normal.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&dye.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&spec.sampler),
                    },
                ],
            });
        if self.material_bgs.len() >= self.material_bg_cap {
            // Evict what this frame has not used; keep the working set.
            self.material_bgs.retain(|_, (_, used)| *used == frame);
            self.material_bg_cap = MAX_MATERIAL_BIND_GROUPS.max(self.material_bgs.len() * 2);
        }
        self.material_bgs.insert(key, (bg.clone(), frame));
        Some(bg)
    }

    /// Resolve one pass's mesh draws: append their uniforms to `uniforms`,
    /// their material bind groups to `bind_groups`, build the pipelines
    /// they need and return them bucketed in drawing order.
    #[allow(clippy::too_many_arguments)]
    fn plan_meshes(
        &mut self,
        pass: &Pass,
        view_proj: Mat4,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        uniforms: &mut Vec<u8>,
        bind_groups: &mut Vec<wgpu::BindGroup>,
        bind_group_index: &mut HashMap<[TextureId; 4], usize>,
        frame_serial: u64,
        warnings: &mut FrameWarnings,
    ) -> Result<PassMeshes> {
        let mut out = PassMeshes::default();
        if pass.meshes.is_empty() {
            return Ok(out);
        }
        let debug = pass.mesh_debug;
        let shading = debug.shading;
        for draw in &pass.meshes {
            let Some(mesh) = self.arena.get(draw.mesh) else {
                if !warnings.mesh {
                    tracing::warn!(mesh = ?draw.mesh, "unknown or destroyed mesh skipped");
                    warnings.mesh = true;
                }
                continue;
            };
            let page = mesh.page;
            let has_tangents = mesh.info.has_tangents;
            let vertex_range = mesh.vertex_range.clone();
            // Skinned when the mesh has influences and the draw a usable
            // palette; otherwise the bind pose (static path).
            // A palette is usable only in the frame (of this renderer) that
            // issued it: its offset addresses that frame's palette buffer.
            let owner = self.owner;
            let this_frame =
                |p: &crate::skin::SkinPalette| p.owner == owner && p.frame == frame_serial;
            let palette_offset = match (mesh.skinned, mesh.info.skin, draw.skin) {
                (true, Some(info), Some(p))
                    if this_frame(&p) && p.len() >= info.required_palette_len() =>
                {
                    Some(p.offset)
                }
                (true, Some(info), Some(p)) => {
                    if !warnings.palette {
                        tracing::warn!(
                            mesh = ?draw.mesh,
                            palette_len = p.len(),
                            needed = info.required_palette_len(),
                            same_frame = this_frame(&p),
                            "skin palette from another frame or too short; drawing the bind pose"
                        );
                        warnings.palette = true;
                    }
                    None
                }
                _ => None,
            };
            let skinned = palette_offset.is_some();
            let palette_offset = palette_offset.unwrap_or(0);
            // Indices into `mesh.submeshes` (the mesh is looked up again
            // below, after the material borrows `self` mutably).
            let submesh_range = match draw.submesh {
                None => 0..mesh.submeshes.len(),
                Some(i) if (i as usize) < mesh.submeshes.len() => i as usize..i as usize + 1,
                Some(i) => {
                    if !warnings.submesh {
                        tracing::warn!(mesh = ?draw.mesh, submesh = i, "no such submesh");
                        warnings.submesh = true;
                    }
                    continue;
                }
            };

            let m = &draw.material;
            let white = self.white;
            let base = self.resolve_texture(m.base_texture, view, warnings);
            let normal = self.resolve_texture(m.normal_texture, view, warnings);
            let dye_mask = match m.dye.map(|d| d.mask) {
                Some(DyeMask::Texture(id)) => self.resolve_texture(Some(id), view, warnings),
                _ => None,
            };
            let spec = self.resolve_texture(m.specular.and_then(|s| s.texture), view, warnings);
            let key = [
                base.unwrap_or(white),
                normal.unwrap_or(white),
                dye_mask.unwrap_or(white),
                spec.unwrap_or(white),
            ];
            let material = match bind_group_index.get(&key) {
                Some(&i) => i,
                None => {
                    let Some(bg) = self.material_bind_group(key) else {
                        continue;
                    };
                    bind_groups.push(bg);
                    bind_group_index.insert(key, bind_groups.len() - 1);
                    bind_groups.len() - 1
                }
            };

            let (overlay_color, normal_length) = (
                debug.wireframe.unwrap_or([1.0; 4]),
                debug.normals.map_or(0.0, |n| n.length),
            );
            let uniform = DrawUniform::new(
                draw.transform,
                m,
                &DrawContext {
                    shading,
                    normal_map: has_tangents && normal.is_some(),
                    dye_mask: dye_mask.is_some(),
                    specular_tex: spec.is_some(),
                    overlay_color,
                    normal_length,
                },
            );
            let offset = uniforms.len() as u64;
            let uniform_offset = u32::try_from(offset).map_err(|_| RenderError::LimitExceeded {
                what: "mesh draw uniform bytes per frame",
                requested: offset,
                max: u64::from(u32::MAX),
            })?;
            uniforms.extend_from_slice(bytemuck::bytes_of(&uniform));
            uniforms.resize(
                usize::try_from(offset + self.draw_stride).unwrap_or(usize::MAX),
                0,
            );

            let Some(mesh) = self.arena.get(draw.mesh) else {
                continue;
            };
            let submeshes = mesh.submeshes.get(submesh_range).unwrap_or(&[]);

            // Surfaces.
            if shading != MeshShading::Hidden {
                let lit = shading == MeshShading::Lit;
                let kind = match m.alpha {
                    AlphaMode::Opaque => Kind::Opaque,
                    AlphaMode::Mask { .. } => Kind::Mask,
                    AlphaMode::Blend if lit => Kind::Blend,
                    AlphaMode::Blend => Kind::Opaque,
                };
                let mirrored = is_mirrored(draw.transform);
                let (back, front) = if mirrored {
                    (Cull::Front, Cull::Back)
                } else {
                    (Cull::Back, Cull::Front)
                };
                // Two-sided blending draws back faces first, then front faces.
                let culls: &[Cull] = match (m.two_sided, kind) {
                    (true, Kind::Blend) => &[front, back],
                    (true, _) => &[Cull::None],
                    (false, _) => &[back],
                };
                for s in submeshes {
                    let tris = mesh.triangles(s);
                    if tris.is_empty() {
                        continue;
                    }
                    let depth = if kind == Kind::Blend {
                        sort_depth(view_proj, s.sphere.transformed(draw.transform).center)
                    } else {
                        0.0
                    };
                    for &cull in culls {
                        let item = MeshItem {
                            key: MeshKey {
                                format,
                                kind,
                                cull,
                                skinned,
                            },
                            page,
                            indices: tris.clone(),
                            uniform_offset,
                            palette_offset,
                            material,
                            depth,
                        };
                        if kind == Kind::Blend {
                            out.blended.push(item);
                        } else {
                            out.surfaces.push(item);
                        }
                    }
                }
            }

            // Overlays.
            if debug.wireframe.is_some() {
                let mut any = false;
                for s in submeshes {
                    let edges = mesh.edges(s);
                    if edges.is_empty() {
                        continue;
                    }
                    any = true;
                    out.overlays.push(OverlayItem {
                        key: MeshKey {
                            format,
                            kind: Kind::Wire,
                            cull: Cull::None,
                            skinned,
                        },
                        page,
                        uniform_offset,
                        palette_offset,
                        draw: OverlayDraw::Wire(edges),
                    });
                }
                if !any && !warnings.wire {
                    tracing::warn!(
                        "wireframe requested but the mesh has no edge list \
                         (MeshConfig::wireframe_edges is off)"
                    );
                    warnings.wire = true;
                }
            }
            if debug.normals.is_some() {
                out.overlays.push(OverlayItem {
                    key: MeshKey {
                        format,
                        kind: Kind::NormalLines,
                        cull: Cull::None,
                        skinned,
                    },
                    page,
                    uniform_offset,
                    palette_offset,
                    draw: OverlayDraw::Normals(vertex_range),
                });
            }
        }
        // Opaque first, then alpha-tested; within each, grouped by pipeline,
        // material and page so state changes are minimal (stable: submission
        // order among equal keys).
        out.surfaces.sort_by_key(|item| {
            (
                item.key.kind == Kind::Mask,
                item.key.skinned,
                item.key.cull,
                item.material,
                item.page,
            )
        });
        // Back to front; stable, so two-sided pairs stay back-faces-first.
        out.blended.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        let device = &self.ctx.device;
        for key in out
            .surfaces
            .iter()
            .chain(&out.blended)
            .map(|i| i.key)
            .chain(out.overlays.iter().map(|o| o.key))
        {
            self.mesh_pipelines.ensure(device, &self.mesh_layouts, key);
        }
        Ok(out)
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
        let (texture, gpu_error) = with_error_scope(&self.ctx.device, || {
            self.ctx.device.create_texture(&wgpu::TextureDescriptor {
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
            })
        });
        if let Some(e) = gpu_error {
            return Err(RenderError::Gpu(e));
        }
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
        let token = Arc::clone(&self.frame_token);
        let (serial, palettes) = self.new_frame_parts();
        Ok(Some(Frame {
            owner: self.owner,
            target: FrameTarget::Surface(texture, token),
            view,
            format,
            width,
            height,
            passes: Vec::new(),
            serial,
            palettes,
        }))
    }

    /// Begin a frame rendering into an offscreen target.
    ///
    /// Fails with [`RenderError::ForeignHandle`] when `target` was created
    /// by a different renderer.
    pub fn begin_offscreen_frame(&mut self, target: &OffscreenTarget) -> Result<Frame> {
        self.check_target(target)?;
        let (serial, palettes) = self.new_frame_parts();
        Ok(Frame {
            owner: self.owner,
            target: FrameTarget::Offscreen,
            view: target.view.clone(),
            format: target.format,
            width: target.width,
            height: target.height,
            passes: Vec::new(),
            serial,
            palettes,
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
            serial,
            palettes,
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
        let mut mesh_uniforms: Vec<u8> = Vec::new();
        let mut material_groups: Vec<wgpu::BindGroup> = Vec::new();
        let mut material_index: HashMap<[TextureId; 4], usize> = HashMap::new();
        let mut warnings = FrameWarnings::default();
        let mut pass_meshes: Vec<PassMeshes> = Vec::with_capacity(passes.len());

        for (i, pass) in passes.iter().enumerate() {
            let is_world = matches!(pass.space, PassSpace::World { .. });
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
            let light = &pass.lighting;
            let eye = if is_world {
                pass_eye(pass, view_proj)
            } else {
                Vec3::ZERO
            };
            let rgb = |c: [f32; 3]| {
                let f = |v: f32| if v.is_finite() { v } else { 0.0 };
                [f(c[0]), f(c[1]), f(c[2]), 1.0]
            };
            let globals = Globals {
                view_proj: view_proj.to_cols_array_2d(),
                eye: eye.extend(1.0).to_array(),
                light_dir: light.direction_normalized().extend(0.0).to_array(),
                light_color: rgb(light.color),
                ambient: rgb(light.ambient),
            };
            if is_world {
                pass_meshes.push(self.plan_meshes(
                    pass,
                    view_proj,
                    format,
                    &view,
                    &mut mesh_uniforms,
                    &mut material_groups,
                    &mut material_index,
                    serial,
                    &mut warnings,
                )?);
            } else {
                if !pass.meshes.is_empty() && !warnings.screen_mesh {
                    tracing::warn!(pass = %pass.label, "meshes in a screen pass are ignored");
                    warnings.screen_mesh = true;
                }
                pass_meshes.push(PassMeshes::default());
            }
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
        // Both buffers of the draw bind group: run both reserves and rebuild
        // the bind group when either was replaced *before* propagating an
        // error, so a failed palette reserve cannot leave `draw_bg` pointing
        // at a draw-uniform buffer that has since been replaced.
        let draws_grew = self
            .draw_uniforms
            .reserve(device, mesh_uniforms.len() as u64);
        let palettes_grew = self.palettes.reserve(device, palettes.buffer_bytes());
        if matches!(draws_grew, Ok(true)) || matches!(palettes_grew, Ok(true)) {
            self.draw_bg = Self::make_draw_bg(
                device,
                &self.mesh_layouts,
                &self.draw_uniforms.buffer,
                &self.palettes.buffer,
            );
        }
        draws_grew?;
        palettes_grew?;
        let queue = &self.ctx.queue;
        queue.write_buffer(&self.uniforms.buffer, 0, &uniform_data);
        if !mesh_uniforms.is_empty() {
            queue.write_buffer(&self.draw_uniforms.buffer, 0, &mesh_uniforms);
        }
        let palette_bytes: &[u8] = bytemuck::cast_slice(&palettes.rows);
        if !palette_bytes.is_empty() {
            queue.write_buffer(&self.palettes.buffer, 0, palette_bytes);
        }
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
            palettes: palettes.count,
            palette_bytes: palette_bytes.len() as u64,
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
                        wgpu::LoadOp::Clear(DEPTH_CLEAR)
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
            let meshes = pass_meshes.get(i);
            let has_meshes = meshes.is_some_and(|m| !m.is_empty());
            if pass.quads.is_empty() && pass.lines.len() < 2 && !has_meshes {
                continue;
            }
            let uniform_offset = u32::try_from(i as u64 * stride).unwrap_or(0);
            rp.set_bind_group(0, &self.globals_bg, &[uniform_offset]);

            let mut bound_page = None;
            if let Some(m) = meshes {
                self.draw_mesh_items(
                    &mut rp,
                    &m.surfaces,
                    &material_groups,
                    &mut bound_page,
                    &mut stats,
                );
            }

            let set = self.pipelines.lookup(format, is_world);
            if let Some(set) = set
                && !pass.quads.is_empty()
            {
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
                    stats.draw_calls = stats.draw_calls.saturating_add(1);
                    stats.quads = stats.quads.saturating_add(range.end - range.start);
                }
            }

            let line_vertices = pass.lines.len() & !1;
            if let Some(set) = set
                && line_vertices > 0
            {
                rp.set_pipeline(&set.line);
                let base = line_base.get(i).copied().unwrap_or(0);
                let vsize = size_of::<LineVertex>() as u64;
                let start = base as u64 * vsize;
                let end = (base + line_vertices) as u64 * vsize;
                rp.set_vertex_buffer(0, self.lines.buffer.slice(start..end));
                let n = u32::try_from(line_vertices).unwrap_or(u32::MAX);
                rp.draw(0..n, 0..1);
                stats.draw_calls = stats.draw_calls.saturating_add(1);
                stats.lines = stats.lines.saturating_add(n / 2);
            }

            if let Some(m) = meshes {
                // Quads/lines replaced the vertex buffer binding.
                bound_page = None;
                self.draw_overlays(&mut rp, &m.overlays, &mut bound_page, &mut stats);
                self.draw_mesh_items(
                    &mut rp,
                    &m.blended,
                    &material_groups,
                    &mut bound_page,
                    &mut stats,
                );
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

    fn bind_page(
        &self,
        rp: &mut wgpu::RenderPass<'_>,
        page: usize,
        bound: &mut Option<usize>,
    ) -> bool {
        if *bound == Some(page) {
            return true;
        }
        let Some(p) = self.arena.page(page) else {
            return false;
        };
        rp.set_vertex_buffer(0, p.vertex_buffer.slice(..));
        if let Some(skin) = &p.skin_buffer {
            rp.set_vertex_buffer(1, skin.slice(..));
        }
        rp.set_index_buffer(p.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        *bound = Some(page);
        true
    }

    fn draw_mesh_items(
        &self,
        rp: &mut wgpu::RenderPass<'_>,
        items: &[MeshItem],
        materials: &[wgpu::BindGroup],
        bound_page: &mut Option<usize>,
        stats: &mut FrameStats,
    ) {
        let mut current_key = None;
        let mut current_material = None;
        for item in items {
            let Some(pipeline) = self.mesh_pipelines.lookup(item.key) else {
                continue;
            };
            let Some(material) = materials.get(item.material) else {
                continue;
            };
            if !self.bind_page(rp, item.page, bound_page) {
                continue;
            }
            if current_key != Some(item.key) {
                rp.set_pipeline(pipeline);
                current_key = Some(item.key);
            }
            if current_material != Some(item.material) {
                rp.set_bind_group(2, material, &[]);
                current_material = Some(item.material);
            }
            rp.set_bind_group(
                1,
                &self.draw_bg,
                &[item.uniform_offset, item.palette_offset],
            );
            rp.draw_indexed(item.indices.clone(), 0, 0..1);
            stats.draw_calls = stats.draw_calls.saturating_add(1);
            stats.mesh_draws = stats.mesh_draws.saturating_add(1);
            if item.key.skinned {
                stats.skinned_draws = stats.skinned_draws.saturating_add(1);
            }
            stats.triangles = stats.triangles.saturating_add(u64::from(
                item.indices.end.saturating_sub(item.indices.start) / 3,
            ));
        }
    }

    fn draw_overlays(
        &self,
        rp: &mut wgpu::RenderPass<'_>,
        items: &[OverlayItem],
        bound_page: &mut Option<usize>,
        stats: &mut FrameStats,
    ) {
        let mut current_key = None;
        for item in items {
            let Some(pipeline) = self.mesh_pipelines.lookup(item.key) else {
                continue;
            };
            if current_key != Some(item.key) {
                rp.set_pipeline(pipeline);
                current_key = Some(item.key);
            }
            rp.set_bind_group(
                1,
                &self.draw_bg,
                &[item.uniform_offset, item.palette_offset],
            );
            match &item.draw {
                OverlayDraw::Wire(range) => {
                    if !self.bind_page(rp, item.page, bound_page) {
                        continue;
                    }
                    rp.draw_indexed(range.clone(), 0, 0..1);
                }
                OverlayDraw::Normals(range) => {
                    let Some(p) = self.arena.page(item.page) else {
                        continue;
                    };
                    // Instance-stepped view of just this mesh's vertices.
                    let size = size_of::<crate::mesh::GpuVertex>() as u64;
                    let start = u64::from(range.start) * size;
                    let end = u64::from(range.end) * size;
                    rp.set_vertex_buffer(0, p.vertex_buffer.slice(start..end));
                    *bound_page = None;
                    if item.key.skinned {
                        let Some(skin) = &p.skin_buffer else {
                            continue;
                        };
                        let size = size_of::<crate::skin::GpuSkin>() as u64;
                        let (start, end) =
                            (u64::from(range.start) * size, u64::from(range.end) * size);
                        rp.set_vertex_buffer(1, skin.slice(start..end));
                    }
                    rp.draw(0..2, 0..range.end.saturating_sub(range.start));
                }
            }
            stats.draw_calls = stats.draw_calls.saturating_add(1);
        }
    }

    /// Copy an offscreen target back to the CPU (blocks until the GPU is done).
    ///
    /// Fails with [`RenderError::ForeignHandle`] when `target` was created
    /// by a different renderer, with [`RenderError::LimitExceeded`] when the
    /// readback buffer (rows padded to 256 bytes) would exceed the device's
    /// `max_buffer_size` (targets larger than about 8192 × 8192 on the
    /// default limits), and with [`RenderError::Gpu`] for a GPU error raised
    /// by the readback itself.
    pub fn read_pixels(&self, target: &OffscreenTarget) -> Result<RgbaImage> {
        self.check_target(target)?;
        let (w, h) = (target.width, target.height);
        let unpadded = u64::from(w) * 4;
        let padded = align_up(unpadded, u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let size = padded
            .checked_mul(u64::from(h))
            .ok_or_else(|| RenderError::Readback("size overflow".into()))?;
        let max = self.ctx.device.limits().max_buffer_size;
        if size > max {
            return Err(RenderError::LimitExceeded {
                what: "readback buffer bytes",
                requested: size,
                max,
            });
        }
        let padded_u32 =
            u32::try_from(padded).map_err(|_| RenderError::Readback("row too wide".into()))?;
        let (result, gpu_error) = with_error_scope(&self.ctx.device, || {
            self.read_pixels_scoped(target, size, padded, padded_u32)
        });
        if let Some(e) = gpu_error {
            return Err(RenderError::Gpu(e));
        }
        let data = result?;
        Ok(RgbaImage {
            width: w,
            height: h,
            data,
        })
    }

    /// GPU part of [`Self::read_pixels`] (runs inside an error scope).
    fn read_pixels_scoped(
        &self,
        target: &OffscreenTarget,
        size: u64,
        padded: u64,
        padded_u32: u32,
    ) -> Result<Vec<u8>> {
        let (w, h) = (target.width, target.height);
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
        // Bounded: a stuck driver yields `RenderError::Poll` (timeout) instead of a hang.
        self.ctx.device.poll(crate::context::bounded_wait())?;
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
        data.ok_or_else(|| RenderError::Readback("short buffer".into()))
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

/// The present mode to configure for `requested` on a surface that supports `supported`.
///
/// Surfaces never list the `Auto*` aliases, so they are resolved here: `AutoVsync` to
/// `FifoRelaxed` when supported, else `Fifo`; `AutoNoVsync` to `Immediate`, else `Mailbox`,
/// else `Fifo`. A concrete mode the surface lacks becomes `Fifo`, which every surface
/// supports.
pub(crate) fn choose_present_mode(
    requested: wgpu::PresentMode,
    supported: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    use wgpu::PresentMode as P;
    let first = |modes: &[P]| {
        modes
            .iter()
            .copied()
            .find(|m| supported.contains(m))
            .unwrap_or(P::Fifo)
    };
    match requested {
        P::AutoVsync => first(&[P::FifoRelaxed, P::Fifo]),
        P::AutoNoVsync => first(&[P::Immediate, P::Mailbox, P::Fifo]),
        m if supported.contains(&m) => m,
        _ => P::Fifo,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn auto_present_modes_resolve_against_the_surface() {
        use super::choose_present_mode as choose;
        use wgpu::PresentMode as P;
        let all = [P::Fifo, P::FifoRelaxed, P::Immediate, P::Mailbox];
        assert_eq!(choose(P::AutoNoVsync, &all), P::Immediate);
        assert_eq!(choose(P::AutoNoVsync, &[P::Fifo, P::Mailbox]), P::Mailbox);
        assert_eq!(choose(P::AutoNoVsync, &[P::Fifo]), P::Fifo);
        assert_eq!(choose(P::AutoVsync, &all), P::FifoRelaxed);
        assert_eq!(choose(P::AutoVsync, &[P::Fifo]), P::Fifo);
        assert_eq!(choose(P::Mailbox, &[P::Fifo]), P::Fifo);
        assert_eq!(choose(P::Mailbox, &all), P::Mailbox);
        assert_eq!(choose(P::Immediate, &[]), P::Fifo);
    }
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
