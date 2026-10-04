// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh viewer's 3D viewport: `tessera-render` running on its own thread, rendering
//! offscreen and handing finished frames to the UI as egui images.
//!
//! # Why a thread and an offscreen target
//!
//! eframe and `tessera-render` use the same wgpu major version, but the renderer owns its
//! device (it has no constructor over an existing device and no way to record into a
//! foreign render pass), so it cannot draw inside egui's render pass. Instead a headless
//! [`Renderer`] runs on the `explorer-render` thread: the UI sends the scene and a
//! [`DrawRequest`] (camera, debug toggles, dye) whenever they change, the thread uploads
//! the scene once, renders into an [`OffscreenTarget`], reads the pixels back and sends
//! them as a [`ColorImage`], which the UI shows as a texture. GPU work, uploads and
//! read-back never run on the UI thread, so its frames stay short whatever the mesh
//! costs to draw; the image lags the input by one render.
//!
//! [`OrbitView`] turns pointer input into an orbit camera (Z-up, `mesh.md` §11).
//!
//! # Lifetime
//!
//! The thread keeps the uploaded scene (and the [`SceneProduct`] it came from) only while
//! the 3D view shows it: [`Viewport::clear_scene`] releases both when another view is
//! shown. If the thread stops (it could not start, or it panicked), the next poll or
//! request reports "render thread stopped" as the error instead of waiting for ever.

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Instant;

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Filter, FrameStats, Lighting, Material, MeshDebug,
    MeshDraw, MeshId, MeshShading, NormalLines, OffscreenFormat, OffscreenTarget, OrbitCamera,
    OrbitConfig, Renderer, RendererConfig, Specular, TextureId, TextureUpload, UpAxis, Wrap,
};

use crate::geometry::{grid_cell, srgb_unit_to_linear, transform_aabb, transform_point};
use crate::scene::{AlphaKind, GpuMaterial, SceneMaterial, SceneProduct};

/// Largest viewport side rendered, in pixels.
pub const MAX_VIEWPORT_SIDE: u32 = 4096;
/// Smallest viewport side rendered, in pixels.
pub const MIN_VIEWPORT_SIDE: u32 = 16;

/// Background colour of the viewport (linear).
const BACKGROUND: [f32; 4] = [0.055, 0.058, 0.066, 1.0];

/// Request sequence number of errors that end the render thread (they answer every
/// request).
const FATAL: u64 = u64::MAX;

/// A dye channel of a material (`material.md` §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DyeChannel {
    /// The primary dye (colour 2003, the first dye mask).
    #[default]
    Primary,
    /// The second dye (colour 2013, the second dye-mask source).
    Secondary,
}

impl DyeChannel {
    /// Index into [`crate::scene::GpuMaterial::dye`].
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
        }
    }

    /// The channel at a [`crate::scene::GpuMaterial::dye`] index (0 primary, anything
    /// else second).
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        if i == 0 {
            Self::Primary
        } else {
            Self::Secondary
        }
    }
}

/// A dye colour chosen in the viewer, replacing a material's effective dye colour on one
/// channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyeOverride {
    /// sRGB colour, 0–1.
    pub color: [f32; 3],
    /// The channel the colour replaces.
    pub channel: DyeChannel,
}

/// Everything that decides one rendered image, apart from the scene itself.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawRequest {
    /// The scene, as numbered by [`Viewport::set_scene`].
    pub scene: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Camera.
    pub camera: Camera,
    /// Surface shading.
    pub shading: MeshShading,
    /// Wireframe overlay.
    pub wireframe: bool,
    /// Vertex-normal lines.
    pub normals: bool,
    /// Ground grid on the XY plane.
    pub grid: bool,
    /// Outline the selected draw's bounds.
    pub highlight: bool,
    /// Dim every draw but the selected one.
    pub isolate: bool,
    /// Hidden parts.
    pub hidden_parts: Vec<usize>,
    /// Hidden draws (part, draw).
    pub hidden: Vec<(usize, u32)>,
    /// Selected draw (part, draw).
    pub selected: Option<(usize, u32)>,
    /// Dye preview colour.
    pub dye: Option<DyeOverride>,
    /// Multiply the materials' diffuse colour 2000 into the base colour (its meaning is
    /// OPEN, `material.md` §19; off by default).
    pub diffuse_tint: bool,
    /// Parts moved apart from the scene centre by this factor of their offset (0 = as
    /// assembled).
    pub explode: f32,
}

/// Commands to the render thread.
enum Cmd {
    Scene {
        id: u64,
        scene: Arc<SceneProduct>,
    },
    Draw {
        seq: u64,
        request: DrawRequest,
    },
    /// Release the uploaded scene.
    Clear,
    Shutdown,
}

/// A rendered frame.
#[derive(Debug)]
pub struct RenderedFrame {
    /// The request's sequence number.
    pub seq: u64,
    /// The scene it shows.
    pub scene: u64,
    /// The pixels (opaque, sRGB).
    pub image: ColorImage,
    /// Renderer statistics.
    pub stats: FrameStats,
    /// Render plus read-back time in milliseconds.
    pub ms: f32,
}

/// Replies of the render thread.
#[derive(Debug)]
enum Reply {
    /// The renderer started on this adapter, whose largest texture side is `max_side`.
    Adapter { name: String, max_side: u32 },
    /// A scene was uploaded.
    Loaded {
        scene: u64,
        problems: Vec<String>,
        ms: f32,
    },
    /// A frame is ready.
    Frame(RenderedFrame),
    /// Rendering failed (the renderer could not start, or a frame failed).
    Error { seq: u64, message: String },
}

/// The UI side of the render thread.
pub struct Viewport {
    tx: Sender<Cmd>,
    rx: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
    next_scene: u64,
    /// The scene sent last: (product pointer, id).
    scene: Option<(usize, u64)>,
    /// The scene the thread finished uploading, with its upload problems.
    loaded: Option<(u64, Vec<String>, f32)>,
    sent: Option<(u64, DrawRequest)>,
    shown: Option<(u64, u64)>,
    /// The texture showing the last frame.
    texture: Option<TextureHandle>,
    /// The adapter in use.
    pub adapter: Option<String>,
    /// The largest image side the renderer can render (the device's texture limit, at
    /// most [`MAX_VIEWPORT_SIDE`]); [`MAX_VIEWPORT_SIDE`] until the renderer started.
    max_side: u32,
    /// The last rendering error, with the request it belongs to.
    error: Option<(u64, String)>,
    /// Statistics of the last frame.
    pub stats: Option<(FrameStats, f32)>,
    /// The sequence number of the last request before the last [`Self::clear_scene`].
    cleared_seq: u64,
}

impl std::fmt::Debug for Viewport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewport")
            .field("scene", &self.scene)
            .field("sent", &self.sent.as_ref().map(|s| s.0))
            .field("shown", &self.shown)
            .field("adapter", &self.adapter)
            .finish_non_exhaustive()
    }
}

impl Viewport {
    /// Starts the render thread; `notify` runs (on that thread) whenever a reply is
    /// ready, to wake the UI.
    #[must_use]
    pub fn spawn(notify: impl Fn() + Send + 'static) -> Self {
        let (tx, crx) = mpsc::channel();
        let (ctx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("explorer-render".into())
            .spawn(move || render_thread(&crx, &ctx, &notify));
        let thread = match thread {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::error!(error = %e, "could not start the render thread");
                None
            }
        };
        Self::from_channels(tx, rx, thread)
    }

    /// The UI side over existing channels (without a thread, every request fails).
    fn from_channels(tx: Sender<Cmd>, rx: Receiver<Reply>, thread: Option<JoinHandle<()>>) -> Self {
        let error = thread
            .is_none()
            .then(|| (FATAL, "the render thread could not be started".to_owned()));
        Self {
            tx,
            rx,
            thread,
            next_scene: 1,
            scene: None,
            loaded: None,
            sent: None,
            shown: None,
            texture: None,
            adapter: None,
            max_side: MAX_VIEWPORT_SIDE,
            error,
            stats: None,
            cleared_seq: 0,
        }
    }

    /// Records that the render thread is gone (every request then counts as answered).
    fn stopped(&mut self) {
        if self.error.as_ref().is_none_or(|(s, _)| *s != FATAL) {
            tracing::error!("the viewport render thread stopped");
            self.error = Some((FATAL, "render thread stopped".into()));
        }
    }

    fn send(&mut self, cmd: Cmd) {
        if self.tx.send(cmd).is_err() {
            self.stopped();
        }
    }

    /// Makes `scene` the scene to draw (sends it only when it differs from the last one);
    /// returns its id for [`DrawRequest::scene`].
    pub fn set_scene(&mut self, scene: &Arc<SceneProduct>) -> u64 {
        let ptr = Arc::as_ptr(scene) as usize;
        if let Some((p, id)) = self.scene
            && p == ptr
        {
            return id;
        }
        let id = self.next_scene;
        self.next_scene += 1;
        self.scene = Some((ptr, id));
        self.send(Cmd::Scene {
            id,
            scene: Arc::clone(scene),
        });
        id
    }

    /// Releases the scene: the render thread drops its uploaded meshes and textures and
    /// its reference to the [`SceneProduct`], and the last frame is forgotten. Called
    /// when the 3D view is not shown; does nothing when there is no scene.
    pub fn clear_scene(&mut self) {
        if self.scene.is_none() && self.texture.is_none() && self.sent.is_none() {
            return;
        }
        self.send(Cmd::Clear);
        self.scene = None;
        self.loaded = None;
        self.shown = None;
        self.texture = None;
        self.stats = None;
        // Keep the sequence numbers increasing; a fatal error stays.
        if let Some((seq, _)) = self.sent.take() {
            self.cleared_seq = seq;
        }
        if self.error.as_ref().is_some_and(|(s, _)| *s != FATAL) {
            self.error = None;
        }
    }

    /// Whether a scene is held (sent to the render thread and not cleared).
    #[must_use]
    pub fn has_scene(&self) -> bool {
        self.scene.is_some()
    }

    /// The largest image side to request ([`DrawRequest::width`], `height`).
    #[must_use]
    pub fn max_side(&self) -> u32 {
        self.max_side
    }

    /// Asks for a frame of `request`, unless it equals the last request.
    pub fn request(&mut self, request: DrawRequest) {
        if self.sent.as_ref().is_some_and(|(_, r)| *r == request) {
            return;
        }
        let seq = self.sent.as_ref().map_or(self.cleared_seq, |(s, _)| *s) + 1;
        self.send(Cmd::Draw {
            seq,
            request: request.clone(),
        });
        self.sent = Some((seq, request));
    }

    /// Takes finished replies; the newest frame replaces the texture.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut newest: Option<RenderedFrame> = None;
        loop {
            let r = match self.rx.try_recv() {
                Ok(r) => r,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stopped();
                    break;
                }
            };
            match r {
                Reply::Adapter { name, max_side } => {
                    self.adapter = Some(name);
                    self.max_side = max_side.clamp(MIN_VIEWPORT_SIDE, MAX_VIEWPORT_SIDE);
                }
                Reply::Loaded {
                    scene,
                    problems,
                    ms,
                } => {
                    for p in &problems {
                        tracing::info!(scene, problem = %p, "scene upload");
                    }
                    self.loaded = Some((scene, problems, ms));
                }
                Reply::Frame(f) => newest = Some(f),
                Reply::Error { seq, message } => {
                    tracing::warn!(seq, error = %message, "viewport render failed");
                    self.error = Some((seq, message));
                }
            }
        }
        if let Some(f) = newest {
            self.shown = Some((f.seq, f.scene));
            self.stats = Some((f.stats, f.ms));
            if self.error.as_ref().is_some_and(|(s, _)| *s <= f.seq) {
                self.error = None;
            }
            let options = TextureOptions::LINEAR;
            match &mut self.texture {
                Some(t) => t.set(f.image, options),
                None => self.texture = Some(ctx.load_texture("mesh-viewport", f.image, options)),
            }
        }
    }

    /// The texture showing the last frame.
    #[must_use]
    pub fn texture(&self) -> Option<&TextureHandle> {
        self.texture.as_ref()
    }

    /// Whether the last request has not been answered yet.
    #[must_use]
    pub fn waiting(&self) -> bool {
        let sent = self.sent.as_ref().map(|s| s.0);
        let answered = self
            .shown
            .map(|s| s.0)
            .max(self.error.as_ref().map(|e| e.0));
        sent.is_some() && sent > answered
    }

    /// Whether the frame on screen answers the last request, which shows scene `scene`.
    #[must_use]
    pub fn shows(&self, scene: u64) -> bool {
        match (&self.sent, self.shown) {
            (Some((seq, req)), Some((shown_seq, shown_scene))) => {
                *seq == shown_seq && shown_scene == scene && req.scene == scene
            }
            _ => false,
        }
    }

    /// The rendering error of the last request, if it failed (or the thread stopped).
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        let (seq, msg) = self.error.as_ref()?;
        let last = self.sent.as_ref().map_or(0, |s| s.0);
        (*seq >= last).then_some(msg.as_str())
    }

    /// Upload problems of scene `scene` and the upload time, once it is loaded.
    #[must_use]
    pub fn loaded(&self, scene: u64) -> Option<(&[String], f32)> {
        self.loaded
            .as_ref()
            .filter(|l| l.0 == scene)
            .map(|l| (l.1.as_slice(), l.2))
    }
}

impl Drop for Viewport {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------------------------------
// Render thread
// ---------------------------------------------------------------------------------------

/// A scene uploaded to the renderer.
struct GpuScene {
    id: u64,
    scene: Arc<SceneProduct>,
    meshes: Vec<Option<MeshId>>,
    textures: Vec<Option<TextureId>>,
}

impl GpuScene {
    fn release(&self, r: &mut Renderer<'static>) {
        // Parts that share a prepared mesh share its upload: destroy each once.
        let mut seen = HashSet::new();
        for m in self.meshes.iter().flatten() {
            if seen.insert(*m) {
                r.destroy_mesh(*m);
            }
        }
        for t in self.textures.iter().flatten() {
            r.destroy_texture(*t);
        }
    }
}

/// The render thread: [`render_loop`], with a panic turned into a final error reply
/// (the UI then stops waiting for frames).
fn render_thread(rx: &Receiver<Cmd>, tx: &Sender<Reply>, notify: &dyn Fn()) {
    let run = std::panic::catch_unwind(AssertUnwindSafe(|| render_loop(rx, tx, notify)));
    if let Err(panic) = run {
        let what = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        tracing::error!(error = %what, "the viewport render thread panicked");
        let _ = tx.send(Reply::Error {
            seq: FATAL,
            message: format!("render thread stopped: {what}"),
        });
        notify();
    }
}

fn render_loop(rx: &Receiver<Cmd>, tx: &Sender<Reply>, notify: &dyn Fn()) {
    let mut renderer: Option<Renderer<'static>> = None;
    let mut start_error: Option<String> = None;
    let mut target: Option<OffscreenTarget> = None;
    let mut gpu: Option<GpuScene> = None;
    let send = |r: Reply| {
        let _ = tx.send(r);
        notify();
    };
    while let Ok(first) = rx.recv() {
        let mut cmds = vec![first];
        while let Ok(c) = rx.try_recv() {
            cmds.push(c);
        }
        let mut scene = None;
        let mut draw = None;
        let mut quit = false;
        let mut clear = false;
        for c in cmds {
            match c {
                Cmd::Scene { id, scene: s } => {
                    scene = Some((id, s));
                    clear = false;
                }
                Cmd::Draw { seq, request } => draw = Some((seq, request)),
                Cmd::Clear => {
                    // A clear drops a scene and a draw that came before it.
                    scene = None;
                    draw = None;
                    clear = true;
                }
                Cmd::Shutdown => quit = true,
            }
        }
        if quit {
            break;
        }
        if renderer.is_none() && start_error.is_none() {
            let config = RendererConfig {
                clear_color: BACKGROUND,
                ..RendererConfig::default()
            };
            match Renderer::new_headless(&config) {
                Ok(r) => {
                    let info = r.adapter_info();
                    let name = format!("{} ({:?})", info.name, info.backend);
                    let max_side = r.context().device.limits().max_texture_dimension_2d;
                    tracing::info!(adapter = %name, max_side, "viewport renderer started");
                    send(Reply::Adapter { name, max_side });
                    renderer = Some(r);
                }
                Err(e) => {
                    tracing::error!(error = %e, "viewport renderer could not start");
                    start_error = Some(format!("no GPU renderer: {e}"));
                }
            }
        }
        let Some(r) = renderer.as_mut() else {
            if clear {
                gpu = None;
            }
            if let Some((seq, _)) = draw {
                send(Reply::Error {
                    seq,
                    message: start_error.clone().unwrap_or_default(),
                });
            }
            continue;
        };
        if clear && let Some(old) = gpu.take() {
            old.release(r);
            // The scene's product is dropped with it.
        }
        if let Some((id, s)) = scene {
            if let Some(old) = gpu.take() {
                old.release(r);
            }
            let started = Instant::now();
            let (g, problems) = upload_scene(r, id, s);
            gpu = Some(g);
            send(Reply::Loaded {
                scene: id,
                problems,
                ms: started.elapsed().as_secs_f32() * 1000.0,
            });
        }
        if let Some((seq, request)) = draw {
            let started = Instant::now();
            match render(r, &mut target, gpu.as_ref(), &request) {
                Ok((image, stats)) => send(Reply::Frame(RenderedFrame {
                    seq,
                    scene: request.scene,
                    image,
                    stats,
                    ms: started.elapsed().as_secs_f32() * 1000.0,
                })),
                Err(message) => send(Reply::Error { seq, message }),
            }
        }
    }
    if let (Some(r), Some(g)) = (renderer.as_mut(), gpu.take()) {
        g.release(r);
    }
}

fn upload_scene(
    r: &mut Renderer<'static>,
    id: u64,
    scene: Arc<SceneProduct>,
) -> (GpuScene, Vec<String>) {
    let mut problems = Vec::new();
    // Parts that share a prepared mesh (a repeated part mesh) share one upload.
    let mut uploaded: HashMap<*const tessera_render::PreparedMesh, Option<MeshId>> = HashMap::new();
    let meshes = scene
        .parts
        .iter()
        .map(|p| {
            let prepared = p.prepared.as_ref()?;
            *uploaded.entry(Arc::as_ptr(prepared)).or_insert_with(|| {
                match r.upload_prepared((**prepared).clone()) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        problems.push(format!("part {} ({:#010x}): {e}", p.caption, p.mesh));
                        None
                    }
                }
            })
        })
        .collect();
    let textures = scene
        .textures
        .iter()
        .map(|t| {
            let upload = TextureUpload {
                label: Some(&t.label),
                width: t.width,
                height: t.height,
                rgba: &t.rgba,
                color_space: if t.linear {
                    ColorSpace::Linear
                } else {
                    ColorSpace::Srgb
                },
                filter: if t.nearest {
                    Filter::Nearest
                } else {
                    Filter::Linear
                },
                wrap: if t.clamp { Wrap::Clamp } else { Wrap::Repeat },
            };
            match r.upload_texture(&upload) {
                Ok(id) => Some(id),
                Err(e) => {
                    problems.push(format!("texture {}: {e}", t.label));
                    None
                }
            }
        })
        .collect();
    (
        GpuScene {
            id,
            scene,
            meshes,
            textures,
        },
        problems,
    )
}

/// The dye channel the renderer draws for `g` (it takes one): the override's channel
/// with the override colour when the material has that channel; otherwise the
/// material's own effective dye, preferring the first channel unless only the second
/// has a colour other than white. Returns the mask texture index and the sRGB colour.
///
/// A material without the overridden channel keeps its own dye: a "dye 2" preview never
/// paints the first channel's mask (`material.md` §12.3: 2013 applies only through the
/// second dye-mask source).
fn drawn_dye(g: &GpuMaterial, over: Option<DyeOverride>) -> Option<(usize, [f32; 3])> {
    if let Some(o) = over
        && let Some(d) = g.dye[o.channel.index()]
    {
        return Some((d.mask, o.color));
    }
    let own = |d: crate::scene::GpuDye| (d.mask, [d.color[0], d.color[1], d.color[2]]);
    let white = |d: &crate::scene::GpuDye| d.color[..3].iter().all(|c| *c >= 0.999);
    match g.dye {
        [Some(first), Some(second)] if white(&first) && !white(&second) => Some(own(second)),
        [first, second] => first.or(second).map(own),
    }
}

/// The renderer material of a scene material under the request's dye and isolation.
fn material(
    m: &SceneMaterial,
    textures: &[Option<TextureId>],
    request: &DrawRequest,
    dim: bool,
) -> Material {
    let tex = |i: Option<usize>| i.and_then(|i| textures.get(i).copied().flatten());
    let g = &m.gpu;
    let mut base_color = if request.diffuse_tint {
        g.base_color
    } else {
        [1.0; 4]
    };
    if dim {
        for c in &mut base_color[..3] {
            *c *= 0.2;
        }
    }
    let dye = drawn_dye(g, request.dye).and_then(|(mask, color)| {
        tex(Some(mask)).map(|t| Dye {
            color: color.map(srgb_unit_to_linear),
            mask: DyeMask::Texture(t),
        })
    });
    Material {
        base_color,
        base_texture: tex(g.base),
        alpha: match g.alpha {
            AlphaKind::Opaque => AlphaMode::Opaque,
            AlphaKind::Mask(cutoff) => AlphaMode::Mask { cutoff },
            AlphaKind::Blend => AlphaMode::Blend,
        },
        two_sided: g.two_sided,
        normal_texture: tex(g.normal),
        normal_green_up: false,
        dye,
        specular: g.specular.map(|(color, shininess)| Specular {
            color,
            shininess,
            texture: tex(g.gloss),
        }),
        unlit: g.unlit,
    }
}

fn vec3(v: [f32; 3]) -> Vec3 {
    Vec3::from_array(v)
}

/// The offset of part `i` under the request's explode factor.
fn explode_offset(scene: &SceneProduct, i: usize, explode: f32) -> Vec3 {
    if explode <= 0.0 || !explode.is_finite() {
        return Vec3::ZERO;
    }
    let Some(p) = scene.parts.get(i) else {
        return Vec3::ZERO;
    };
    let Some((c, _)) = p.sphere else {
        return Vec3::ZERO;
    };
    let d = vec3(transform_point(&p.transform, c)) - vec3(scene.sphere.0);
    let d = if d.length() > scene.sphere.1 * 1e-3 {
        d
    } else {
        // Parts centred on the scene centre move along +X by their index.
        Vec3::X * (i as f32) * scene.sphere.1 * 0.25
    };
    let o = d * explode;
    if o.is_finite() { o } else { Vec3::ZERO }
}

/// The colour of a draw without a material, dimmed like materials in isolate mode.
fn fallback_color(dim: bool) -> [f32; 4] {
    let k = if dim { 0.2 } else { 1.0 };
    [0.6 * k, 0.6 * k, 0.62 * k, 1.0]
}

/// The rendered image size for a requested `width` × `height`: both sides scaled by one
/// factor so the larger fits `max` (the device's texture limit, at most
/// [`MAX_VIEWPORT_SIDE`]), keeping the aspect of the rectangle the image is shown in;
/// each side at least [`MIN_VIEWPORT_SIDE`].
#[must_use]
pub fn fit_viewport(width: u32, height: u32, max: u32) -> (u32, u32) {
    let max = max.clamp(MIN_VIEWPORT_SIDE, MAX_VIEWPORT_SIDE);
    let (w, h) = (width.max(1), height.max(1));
    let scale = (f64::from(max) / f64::from(w.max(h))).min(1.0);
    let side = |v: u32| ((f64::from(v) * scale).round() as u32).clamp(MIN_VIEWPORT_SIDE, max);
    (side(w), side(h))
}

fn render(
    r: &mut Renderer<'static>,
    target: &mut Option<OffscreenTarget>,
    gpu: Option<&GpuScene>,
    request: &DrawRequest,
) -> Result<(ColorImage, FrameStats), String> {
    let max = r.context().device.limits().max_texture_dimension_2d;
    let (w, h) = fit_viewport(request.width, request.height, max);
    if target.as_ref().is_none_or(|t| t.size() != (w, h)) {
        *target = None;
        *target = Some(
            r.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)
                .map_err(|e| e.to_string())?,
        );
    }
    let Some(t) = target.as_ref() else {
        return Err("no render target".into());
    };
    let mut frame = r.begin_offscreen_frame(t).map_err(|e| e.to_string())?;
    let scene = gpu.filter(|g| g.id == request.scene);
    let radius = scene.map_or(1.0, |g| g.scene.sphere.1);
    let pass = frame.world_pass("mesh-view", &request.camera);
    pass.clear(BACKGROUND).lighting(Lighting::default());
    pass.mesh_debug(MeshDebug {
        shading: request.shading,
        wireframe: request.wireframe.then_some([0.95, 0.75, 0.25, 1.0]),
        normals: request.normals.then_some(NormalLines {
            length: radius * 0.03,
            color: [0.3, 0.7, 1.0, 1.0],
        }),
    });
    if request.grid {
        let cell = grid_cell(radius);
        pass.grid(
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
            cell,
            10,
            [0.32, 0.33, 0.36, 1.0],
        );
        pass.axes(Vec3::ZERO, cell * 2.0);
    }
    if let Some(g) = scene {
        let hidden_parts: HashSet<usize> = request.hidden_parts.iter().copied().collect();
        let hidden: HashSet<(usize, u32)> = request.hidden.iter().copied().collect();
        for (pi, part) in g.scene.parts.iter().enumerate() {
            if hidden_parts.contains(&pi) {
                continue;
            }
            let Some(mesh) = g.meshes.get(pi).copied().flatten() else {
                continue;
            };
            let offset = explode_offset(&g.scene, pi, request.explode);
            let transform = Mat4::from_translation(offset) * Mat4::from_cols_array(&part.transform);
            for s in &part.submeshes {
                if hidden.contains(&(pi, s.draw)) {
                    continue;
                }
                let selected = request.selected == Some((pi, s.draw));
                let dim = request.isolate && request.selected.is_some() && !selected;
                let mat = s
                    .material
                    .and_then(|i| g.scene.materials.get(i))
                    .map_or_else(
                        || Material::color(fallback_color(dim)),
                        |m| material(m, &g.textures, request, dim),
                    );
                pass.mesh(MeshDraw::new(mesh, transform, mat).with_submesh(s.draw));
                if selected
                    && request.highlight
                    && let Some((lo, hi)) = s
                        .aabb
                        .and_then(|(lo, hi)| transform_aabb(&part.transform, lo, hi))
                {
                    pass.aabb(vec3(lo) + offset, vec3(hi) + offset, [1.0, 0.55, 0.1, 1.0]);
                }
            }
        }
    }
    let stats = r.end_frame(frame).map_err(|e| e.to_string())?;
    let img = r.read_pixels(t).map_err(|e| e.to_string())?;
    let (w, h) = (img.width as usize, img.height as usize);
    let pixels: Vec<Color32> = img
        .data
        .chunks_exact(4)
        .map(|p| Color32::from_rgb(p[0], p[1], p[2]))
        .collect();
    if pixels.len() != w * h {
        return Err("read-back size mismatch".into());
    }
    Ok((ColorImage::new([w, h], pixels), stats))
}

// ---------------------------------------------------------------------------------------
// Orbit camera
// ---------------------------------------------------------------------------------------

/// The viewer's orbit camera: drag to orbit, drag with the secondary button (or Shift)
/// to pan, wheel to zoom, framing on a bounding sphere. Z is up (`mesh.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitView {
    /// The camera.
    pub orbit: OrbitCamera,
}

impl Default for OrbitView {
    fn default() -> Self {
        Self::new()
    }
}

impl OrbitView {
    /// Initial yaw: the camera in front of a character (characters face +Y, `mesh.md`
    /// §11.1) and a little to its left.
    pub const YAW: f32 = std::f32::consts::PI * 0.82;
    /// Initial pitch: 20° above the horizon.
    pub const PITCH: f32 = 0.35;

    /// A camera at the default angles.
    #[must_use]
    pub fn new() -> Self {
        let mut orbit = OrbitCamera::new(OrbitConfig {
            up: UpAxis::Z,
            pivot_height: 0.0,
            pitch: Self::PITCH,
            min_pitch: (-89f32).to_radians(),
            max_pitch: 89f32.to_radians(),
            zoom_smoothing: 0.0,
            ..OrbitConfig::default()
        });
        orbit.yaw = Self::YAW;
        Self { orbit }
    }

    /// Frames a sphere for a viewport of `aspect`, keeping the angles.
    pub fn frame(&mut self, center: [f32; 3], radius: f32, aspect: f32) {
        self.orbit.frame_sphere(
            tessera_render::BoundingSphere {
                center: vec3(center),
                radius,
            },
            aspect,
        );
    }

    /// Back to the default angles.
    pub fn reset_angles(&mut self) {
        self.orbit.yaw = Self::YAW;
        self.orbit.pitch = Self::PITCH;
        self.orbit.orbit(0.0, 0.0);
    }

    /// Orbits by a pointer movement in points.
    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.orbit.orbit(dx, dy);
    }

    /// Zooms by `steps` wheel notches (positive = closer).
    pub fn zoom(&mut self, steps: f32) {
        self.orbit.zoom(steps);
        self.orbit.update(0.0);
    }

    /// Pans by a pointer movement of (`dx`, `dy`) points in a viewport `height` points
    /// high: the point under the pointer stays under it (at the pivot's depth).
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let cam = self.orbit.camera();
        let forward = (cam.target - cam.eye).normalize_or_zero();
        let right = forward.cross(cam.up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let fov = cam.projection.fov_y;
        let per_point = 2.0 * self.orbit.distance * (fov * 0.5).tan() / height.max(1.0);
        let delta = (-right * dx + up * dy) * per_point;
        if delta.is_finite() {
            self.orbit.target += delta;
        }
    }

    /// The camera to render with.
    #[must_use]
    pub fn camera(&self) -> Camera {
        self.orbit.camera()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_starts_in_front_and_frames() {
        let mut v = OrbitView::new();
        v.frame([1.0, 2.0, 3.0], 2.0, 1.5);
        let cam = v.camera();
        assert_eq!(cam.target, Vec3::new(1.0, 2.0, 3.0));
        // In front of a character facing +Y, above the horizon.
        assert!(cam.eye.y > 2.0 && cam.eye.z > 3.0, "{cam:?}");
        let d = (cam.eye - cam.target).length();
        assert!(d > 2.0 && d < 10.0, "{d}");
    }

    #[test]
    fn pan_moves_the_target_in_the_view_plane() {
        let mut v = OrbitView::new();
        v.frame([0.0; 3], 1.0, 1.0);
        let before = v.camera();
        v.pan(100.0, 0.0, 500.0);
        let after = v.camera();
        let moved = after.target - before.target;
        assert!(moved.length() > 0.0);
        // Perpendicular to the view direction.
        let fwd = (before.target - before.eye).normalize();
        assert!(moved.normalize().dot(fwd).abs() < 1e-4);
        // Garbage input never poisons the camera.
        v.pan(f32::NAN, f32::INFINITY, 0.0);
        assert!(v.camera().target.is_finite());
    }

    #[test]
    fn zoom_and_rotate_stay_finite() {
        let mut v = OrbitView::new();
        v.frame([0.0; 3], 1.0, 1.0);
        let d0 = v.orbit.distance;
        v.zoom(2.0);
        assert!(v.orbit.distance < d0);
        v.rotate(1e9, -1e9);
        v.zoom(f32::NAN);
        assert!(v.camera().eye.is_finite());
        v.reset_angles();
        assert!((v.orbit.yaw - OrbitView::YAW).abs() < 1e-5);
    }

    #[test]
    fn explode_offsets() {
        let s = crate::scene::tests_support::two_part_scene();
        assert_eq!(explode_offset(&s, 0, 0.0), Vec3::ZERO);
        let o = explode_offset(&s, 1, 1.0);
        assert!(o.x > 0.0, "{o:?}");
        assert_eq!(explode_offset(&s, 9, 1.0), Vec3::ZERO);
        assert_eq!(explode_offset(&s, 1, f32::NAN), Vec3::ZERO);
    }

    fn request_for(scene: &SceneProduct, id: u64) -> DrawRequest {
        let mut cam = OrbitView::new();
        cam.frame(scene.sphere.0, scene.sphere.1, 1.0);
        DrawRequest {
            scene: id,
            width: 96,
            height: 96,
            camera: cam.camera(),
            shading: MeshShading::BaseColor,
            wireframe: false,
            normals: false,
            grid: false,
            highlight: false,
            isolate: false,
            hidden_parts: Vec::new(),
            hidden: Vec::new(),
            selected: None,
            dye: None,
            diffuse_tint: false,
            explode: 0.0,
        }
    }

    /// Draws the synthetic scene with the render thread's own functions: something is
    /// drawn, hiding every part leaves the background, and a dye colour changes pixels.
    #[test]
    fn render_function_draws_hides_and_dyes() {
        let mut r = match Renderer::new_headless(&RendererConfig::default()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("no GPU renderer in this environment, skipping: {e}");
                return;
            }
        };
        let mut shared = crate::scene::tests_support::two_part_scene();
        // A third part repeating the first part's mesh shares its upload.
        let mut again = shared.parts[0].clone();
        again.caption = "again".into();
        shared.parts.push(again);
        let (g3, problems) = upload_scene(&mut r, 6, Arc::new(shared));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(g3.meshes[0], g3.meshes[2]);
        assert_ne!(g3.meshes[0], g3.meshes[1]);
        g3.release(&mut r);
        assert_eq!(r.mesh_stats().meshes, 0);
        let scene = Arc::new(crate::scene::tests_support::two_part_scene());
        let (gpu, problems) = upload_scene(&mut r, 7, Arc::clone(&scene));
        assert!(problems.is_empty(), "{problems:?}");
        let mut target = None;
        let req = request_for(&scene, 7);
        let (img, stats) = render(&mut r, &mut target, Some(&gpu), &req).unwrap();
        assert_eq!(img.size, [96, 96]);
        assert!(stats.mesh_draws >= 2, "{stats:?}");
        let hidden = DrawRequest {
            hidden_parts: vec![0, 1],
            ..req.clone()
        };
        let (empty, _) = render(&mut r, &mut target, Some(&gpu), &hidden).unwrap();
        // With every part hidden only the clear colour remains.
        let bg = empty.pixels[0];
        assert!(empty.pixels.iter().all(|c| *c == bg));
        let drawn = img.pixels.iter().filter(|c| **c != bg).count();
        assert!(drawn > 50, "only {drawn} non-background pixels");
        let dyed = DrawRequest {
            dye: Some(DyeOverride {
                color: [0.0, 1.0, 0.0],
                channel: DyeChannel::Primary,
            }),
            ..req.clone()
        };
        let (green, _) = render(&mut r, &mut target, Some(&gpu), &dyed).unwrap();
        assert_ne!(green.pixels, img.pixels, "dye changed nothing");
        // A request for another scene draws no meshes.
        let other = DrawRequest { scene: 8, ..req };
        let (_, stats) = render(&mut r, &mut target, Some(&gpu), &other).unwrap();
        assert_eq!(stats.mesh_draws, 0);
        gpu.release(&mut r);
    }

    fn dye(mask: usize, color: [f32; 4]) -> crate::scene::GpuDye {
        crate::scene::GpuDye { mask, color }
    }

    #[test]
    fn dye_channel_selection() {
        let white = [1.0; 4];
        let red = [1.0, 0.0, 0.0, 1.0];
        let one = GpuMaterial {
            dye: [Some(dye(3, red)), None],
            ..GpuMaterial::default()
        };
        let two = GpuMaterial {
            dye: [Some(dye(3, red)), Some(dye(4, [0.0, 1.0, 0.0, 1.0]))],
            ..GpuMaterial::default()
        };
        let green = |channel| {
            Some(DyeOverride {
                color: [0.0, 1.0, 0.0],
                channel,
            })
        };
        // No override: the material's own first dye.
        assert_eq!(drawn_dye(&one, None), Some((3, [1.0, 0.0, 0.0])));
        // Overriding dye 1 recolours the first mask.
        assert_eq!(
            drawn_dye(&one, green(DyeChannel::Primary)),
            Some((3, [0.0, 1.0, 0.0]))
        );
        // Regression: overriding dye 2 painted the first mask of single-channel
        // materials; now they keep their own dye.
        assert_eq!(
            drawn_dye(&one, green(DyeChannel::Secondary)),
            Some((3, [1.0, 0.0, 0.0]))
        );
        assert_eq!(
            drawn_dye(&two, green(DyeChannel::Secondary)),
            Some((4, [0.0, 1.0, 0.0]))
        );
        // Without an override, a white first channel gives way to a coloured second.
        let second_only = GpuMaterial {
            dye: [Some(dye(3, white)), Some(dye(4, red))],
            ..GpuMaterial::default()
        };
        assert_eq!(drawn_dye(&second_only, None), Some((4, [1.0, 0.0, 0.0])));
        let lone_second = GpuMaterial {
            dye: [None, Some(dye(4, red))],
            ..GpuMaterial::default()
        };
        assert_eq!(drawn_dye(&lone_second, None), Some((4, [1.0, 0.0, 0.0])));
        assert_eq!(
            drawn_dye(&GpuMaterial::default(), green(DyeChannel::Primary)),
            None
        );
        assert_eq!(DyeChannel::from_index(1), DyeChannel::Secondary);
        assert_eq!(DyeChannel::Secondary.index(), 1);
    }

    #[test]
    fn untextured_draws_dim_in_isolate_mode() {
        assert_eq!(fallback_color(false), [0.6, 0.6, 0.62, 1.0]);
        let d = fallback_color(true);
        assert!(d[..3].iter().all(|c| *c < 0.13) && d[3] == 1.0);
    }

    #[test]
    fn viewport_sizes_keep_the_aspect() {
        assert_eq!(fit_viewport(800, 600, 4096), (800, 600));
        // Regression: each side was clamped on its own, stretching the image.
        assert_eq!(fit_viewport(5400, 2600, 4096), (4096, 1972));
        // The device's limit wins when it is smaller.
        assert_eq!(fit_viewport(3000, 1500, 2048), (2048, 1024));
        assert_eq!(
            fit_viewport(0, 0, 4096),
            (MIN_VIEWPORT_SIDE, MIN_VIEWPORT_SIDE)
        );
        assert_eq!(
            fit_viewport(10_000, 1, 0),
            (MIN_VIEWPORT_SIDE, MIN_VIEWPORT_SIDE)
        );
        let (w, h) = fit_viewport(u32::MAX, u32::MAX, u32::MAX);
        assert_eq!((w, h), (MAX_VIEWPORT_SIDE, MAX_VIEWPORT_SIDE));
    }

    #[test]
    fn a_dead_render_thread_is_reported() {
        let scene = Arc::new(crate::scene::tests_support::two_part_scene());
        let ctx = egui::Context::default();
        // The thread is gone before the first request: sending fails.
        let (tx, crx) = mpsc::channel();
        let (rtx, rx) = mpsc::channel();
        drop((crx, rtx));
        let mut vp = Viewport::from_channels(tx, rx, None);
        let id = vp.set_scene(&scene);
        vp.request(request_for(&scene, id));
        assert!(!vp.waiting());
        assert!(vp.error().is_some());
        // The thread dies after taking the request: the poll notices.
        let (tx, crx) = mpsc::channel::<Cmd>();
        let (rtx, rx) = mpsc::channel::<Reply>();
        let handle = std::thread::spawn(move || {
            let _ = crx.recv();
            drop(rtx);
        });
        let mut vp = Viewport::from_channels(tx, rx, None);
        vp.error = None;
        let id = vp.set_scene(&scene);
        vp.request(request_for(&scene, id));
        assert!(vp.waiting());
        handle.join().unwrap();
        vp.poll(&ctx);
        assert!(!vp.waiting(), "still waiting for a dead thread");
        assert_eq!(vp.error(), Some("render thread stopped"));
    }

    #[test]
    fn a_panicking_render_loop_answers_with_an_error() {
        let (tx, rx) = mpsc::channel::<Reply>();
        let (ctx, crx) = mpsc::channel::<Cmd>();
        // The loop calls `notify` after its first reply (adapter or start error); a
        // panic there stands for any panic inside the loop.
        ctx.send(Cmd::Draw {
            seq: 1,
            request: request_for(&crate::scene::tests_support::two_part_scene(), 1),
        })
        .unwrap();
        drop(ctx);
        let calls = std::cell::Cell::new(0);
        render_thread(&crx, &tx, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                panic!("boom");
            }
        });
        let replies: Vec<Reply> = rx.try_iter().collect();
        assert!(
            replies.iter().any(|r| matches!(
                r,
                Reply::Error { seq: FATAL, message } if message.contains("boom")
            )),
            "{replies:?}"
        );
    }

    #[test]
    fn clearing_releases_the_scene() {
        let scene = Arc::new(crate::scene::tests_support::two_part_scene());
        let (tx, crx) = mpsc::channel();
        let (_rtx, rx) = mpsc::channel();
        let mut vp = Viewport::from_channels(tx, rx, None);
        vp.error = None;
        let id = vp.set_scene(&scene);
        vp.request(request_for(&scene, id));
        assert!(vp.has_scene() && vp.waiting());
        vp.clear_scene();
        assert!(!vp.has_scene() && !vp.waiting());
        let cmds: Vec<Cmd> = crx.try_iter().collect();
        assert!(matches!(cmds.last(), Some(Cmd::Clear)));
        // Dropping the commands drops the thread's references to the scene.
        drop(cmds);
        assert_eq!(Arc::strong_count(&scene), 1);
        // A second clear sends nothing; showing the scene again re-sends it, and request
        // numbers keep increasing.
        vp.clear_scene();
        assert!(crx.try_recv().is_err());
        let id2 = vp.set_scene(&scene);
        assert_ne!(id, id2);
        vp.request(request_for(&scene, id2));
        let cmds: Vec<Cmd> = crx.try_iter().collect();
        assert!(matches!(cmds.first(), Some(Cmd::Scene { .. })));
        assert!(matches!(cmds.last(), Some(Cmd::Draw { seq: 2, .. })));
    }

    /// Renders a two-part synthetic scene headless, when a GPU adapter exists.
    #[test]
    fn renders_a_synthetic_scene_when_a_gpu_exists() {
        let scene = Arc::new(crate::scene::tests_support::two_part_scene());
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut vp = Viewport::spawn(move || {
            let _ = tx.send(());
        });
        let id = vp.set_scene(&scene);
        let mut cam = OrbitView::new();
        cam.frame(scene.sphere.0, scene.sphere.1, 1.0);
        vp.request(DrawRequest {
            scene: id,
            width: 64,
            height: 64,
            camera: cam.camera(),
            shading: MeshShading::Lit,
            wireframe: true,
            normals: true,
            grid: true,
            highlight: true,
            isolate: true,
            hidden_parts: Vec::new(),
            hidden: Vec::new(),
            selected: Some((0, 0)),
            dye: None,
            diffuse_tint: true,
            explode: 0.5,
        });
        let ctx = egui::Context::default();
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        while vp.waiting() && Instant::now() < deadline {
            let _ = rx.recv_timeout(std::time::Duration::from_millis(200));
            vp.poll(&ctx);
        }
        if let Some(e) = vp.error() {
            eprintln!("no GPU renderer in this environment, skipping: {e}");
            return;
        }
        assert!(vp.shows(id), "frame not shown");
        let t = vp.texture().expect("texture");
        assert_eq!(t.size(), [64, 64]);
    }
}
