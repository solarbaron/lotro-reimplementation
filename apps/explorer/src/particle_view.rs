// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle preview for particle-system descriptors (0x39) in the gameplay view, and the
//! headless `--particles DID --sheet out.png` contact sheet.
//!
//! The record is simulated by `tessera-particles` (`docs/specs/particles.md` §8,
//! `docs/specs/particle-runtime.md` §3–§9) with its own seeded random stream
//! (`particle-runtime.md` §12 rule 1), at 30 steps per second, for an owner at the origin
//! whose attachment points are all at its origin (emitters with a bone or holding location
//! spawn there), and drawn through the renderer's particle pipeline with the textures,
//! blend state and program of the record's surface (`particle-runtime.md` §9.1, §9.1.1).
//! The preview renders
//! offscreen on its own thread (like the 3D view's viewport) and shows the frames as an
//! image; play, pause, restart and the seed control the simulation, dragging orbits the
//! camera and the wheel zooms.
//!
//! Fork effects (0x44) are not simulated (`particles.md` rule §10.2): the view says so.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Instant;

use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions, Ui};
use tessera_formats::gameplay::particles::ParticleSystem;
use tessera_formats::{Did, RgbaImage};
use tessera_material::ResolvedMaterial;
use tessera_material::records::{BlendFactor as MatFactor, BlendOp as MatOp};
use tessera_material::{BlendMode, CurveStates, TextureMatrix, TextureMatrixLayout};
use tessera_particles::glam::Vec3;
use tessera_particles::surface::{LayerCombine, ParticleSurface};
use tessera_particles::{
    CameraView, Environment, OriginAttachments, Placement, Ran2, SystemDraw, SystemSim,
};
use tessera_render::{
    AlphaMode, BlendFactor, BlendOp, Camera, ColorSpace, Filter, FixedBlend, Material, MeshData,
    MeshDraw, MeshId, OffscreenFormat, OffscreenTarget, ParticleDraw, ParticleLayers,
    ParticleShading, ParticleVertex, Renderer, RendererConfig, TextureId, TextureUpload,
    UvTransform, Wrap,
};

use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::sheet::{GUTTER, SheetReport, sheet_size, tile};
use crate::viewport::OrbitView;

/// Simulation steps per second (one placement per step).
pub const STEPS_PER_SECOND: f64 = 30.0;
/// Largest texture side loaded for the preview.
const TEXTURE_CAP: u32 = 512;
/// Largest simulated time a sheet column may ask for, seconds.
pub const MAX_SHEET_TIME: f32 = 600.0;
/// Background colour (linear).
const BACKGROUND: [f32; 4] = [0.045, 0.05, 0.06, 1.0];

/// How the preview draws a surface (`particle-runtime.md` §9.1, §9.1.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewLook {
    /// The surface's blend state.
    pub alpha: AlphaMode,
    /// What the surface's program computes.
    pub shading: ParticleShading,
    /// Material colour 2000.
    pub material: [f32; 4],
    /// `false` for the frame-buffer distortion program, which Tessera draws as its variant
    /// without a frame-buffer input (it leaves the image unchanged).
    pub drawn: bool,
    /// The shader record when it is one of the programs of runtime §9.1.1.
    pub program: Option<u32>,
    /// Texture matrices 0 and 1 of a two-layer program (identity otherwise; runtime
    /// §9.1.1, RT27).
    pub uv: [UvTransform; 2],
}

impl Default for PreviewLook {
    fn default() -> Self {
        Self {
            alpha: AlphaMode::Blend,
            shading: ParticleShading::STANDARD,
            material: [1.0; 4],
            drawn: true,
            program: None,
            uv: [UvTransform::IDENTITY; 2],
        }
    }
}

/// What re-evaluates a surface whose alpha-test reference or texture matrices follow the
/// clock ([`ParticleSurface::is_animated`]; runtime §9.1.1 answers (d), (e)).
#[derive(Debug)]
pub struct PreviewClock {
    look: ParticleSurface,
    material: Arc<ResolvedMaterial>,
    layout: Arc<TextureMatrixLayout>,
}

/// Texture matrices as the renderer takes them.
fn uv_transforms(m: [TextureMatrix; 2]) -> [UvTransform; 2] {
    m.map(|m| UvTransform { rows: m.rows }.sanitized())
}

/// What the preview needs from the install.
#[derive(Debug, Clone)]
pub struct PreviewSource {
    /// The record.
    pub did: u32,
    /// The decoded system.
    pub desc: Arc<ParticleSystem>,
    /// The surface's colour texture (slot 3000).
    pub texture: Option<Arc<RgbaImage>>,
    /// The second colour layer (slot 3001) of a two-layer program.
    pub second_texture: Option<Arc<RgbaImage>>,
    /// The surface's blend state and program at time 0.
    pub look: PreviewLook,
    /// For an animated surface: what [`PreviewSim::look`] re-evaluates at every step.
    pub clock: Option<Arc<PreviewClock>>,
    /// What could not be resolved.
    pub notes: Vec<String>,
}

/// The renderer's program description for a particle surface (`particle-runtime.md`
/// §9.1.1).
#[must_use]
pub fn particle_shading(s: &ParticleSurface) -> ParticleShading {
    let p = s.program;
    ParticleShading {
        tests_texture: p.tests_texture,
        texture_alpha: p.texture_alpha,
        vertex_colour: p.vertex_colour,
        material_colour: p.material_colour,
        doubled: p.doubled,
        fogged: p.fogged,
        layers: p.layers.map(|l| match l {
            LayerCombine::Product => ParticleLayers::Product,
            LayerCombine::Sum => ParticleLayers::Sum,
        }),
    }
}

fn factor(f: MatFactor) -> Option<BlendFactor> {
    Some(match f {
        MatFactor::Zero => BlendFactor::Zero,
        MatFactor::One => BlendFactor::One,
        MatFactor::SrcColor => BlendFactor::SrcColor,
        MatFactor::InvSrcColor => BlendFactor::OneMinusSrcColor,
        MatFactor::SrcAlpha => BlendFactor::SrcAlpha,
        MatFactor::InvSrcAlpha => BlendFactor::OneMinusSrcAlpha,
        MatFactor::DestAlpha => BlendFactor::DstAlpha,
        MatFactor::DestColor => BlendFactor::DstColor,
        MatFactor::Other(_) => return None,
    })
}

/// The renderer's alpha mode for a particle surface's resolved material (`material.md`
/// §11.3: the blend mode, factors and depth write) with the particle alpha test `test`
/// ([`ParticleSurface::alpha_test`], runtime §9.1.3: the effective 4004 and 1043, not the
/// mesh opacity-source rule).
pub fn surface_alpha(m: &ResolvedMaterial, test: Option<f32>) -> AlphaMode {
    let b = m.features.blend;
    match b.mode {
        BlendMode::Opaque => test.map_or(AlphaMode::Opaque, |c| AlphaMode::Mask { cutoff: c }),
        BlendMode::Cutout => AlphaMode::Mask {
            cutoff: test.unwrap_or(0.5),
        },
        BlendMode::AlphaBlend if b.depth_write => AlphaMode::TestBlend {
            cutoff: test.unwrap_or(-1.0),
        },
        // `particle-runtime.md` §9.1.1 rule 1: alpha test on with depth write off keeps
        // the material's reference (the renderer switches the test off while *A* < 0.9),
        // so the draw carries the cut-off without depth write.
        BlendMode::AlphaBlend => test.map_or(AlphaMode::Blend, |c| {
            AlphaMode::Fixed(FixedBlend {
                src: BlendFactor::SrcAlpha,
                dst: BlendFactor::OneMinusSrcAlpha,
                op: BlendOp::Add,
                depth_write: false,
                alpha_test: Some(tessera_render::AlphaThreshold(c)),
            })
        }),
        BlendMode::Additive if b.depth_write => AlphaMode::Fixed(FixedBlend {
            depth_write: true,
            alpha_test: test.map(tessera_render::AlphaThreshold),
            ..FixedBlend::ADDITIVE
        }),
        BlendMode::Additive => test.map_or(AlphaMode::Additive, |c| {
            AlphaMode::Fixed(FixedBlend {
                alpha_test: Some(tessera_render::AlphaThreshold(c)),
                ..FixedBlend::ADDITIVE
            })
        }),
        BlendMode::Custom => {
            let op = match b.op {
                MatOp::Add => Some(BlendOp::Add),
                MatOp::RevSubtract => Some(BlendOp::ReverseSubtract),
                MatOp::Other(_) => None,
            };
            match (factor(b.src), factor(b.dst), op) {
                (Some(src), Some(dst), Some(op)) => AlphaMode::Fixed(FixedBlend {
                    src,
                    dst,
                    op,
                    depth_write: b.depth_write,
                    alpha_test: test.map(tessera_render::AlphaThreshold),
                }),
                _ => AlphaMode::Blend,
            }
        }
    }
}

impl PreviewClock {
    /// `base` with the alpha-test reference and the texture matrices evaluated at `t` with
    /// the run's curve states `states` and the run's stream `rng` (mode 3).
    fn look_at(
        &self,
        base: PreviewLook,
        t: f64,
        states: &mut CurveStates,
        rng: &mut Ran2,
    ) -> PreviewLook {
        let s = self.look.at(&self.material, &self.layout, t, states, rng);
        PreviewLook {
            alpha: surface_alpha(&self.material, s.alpha_test),
            uv: uv_transforms(s.matrices),
            ..base
        }
    }
}

/// Reads and decodes `did` and resolves its surface (`particles.md` §4.1: `surface`, else
/// `extra_surface`).
///
/// # Errors
///
/// When the record does not read or decode, or is not a 0x39 record (fork effects are not
/// simulated, `particles.md` rule §10.2).
pub fn load_source(ctx: &DataContext, did: u32) -> Result<PreviewSource, ExplorerError> {
    if did >> 24 == 0x44 {
        return Err(ExplorerError::NotApplicable(
            "fork effects (0x44) are not simulated: their effect image is opaque (particles.md \
             rule §10.2)"
                .into(),
        ));
    }
    if did >> 24 != 0x39 {
        return Err(ExplorerError::NotApplicable(format!(
            "{did:#010x} is not a particle system"
        )));
    }
    let raw = crate::decoders::read_raw(ctx, did, None)?;
    let desc = ParticleSystem::decode(&raw.bytes)?;
    let surface = if desc.surface.is_null() {
        desc.extra_surface
    } else {
        desc.surface
    };
    let mut notes = Vec::new();
    let mut texture = None;
    let mut second_texture = None;
    let mut look = PreviewLook::default();
    let mut clock = None;
    if surface.is_null() {
        notes.push("no surface: the client draws nothing for this system (§8.11)".into());
    } else {
        match ctx.materials.resolve_surface(surface) {
            Ok(m) => {
                let layout = ctx
                    .materials
                    .texture_matrices(m.shader.did)
                    .map_err(|e| notes.push(format!("texture matrices of {}: {e}", m.shader.did)))
                    .unwrap_or_default();
                let s = ParticleSurface::of(&m, &layout);
                look = PreviewLook {
                    alpha: surface_alpha(&m, s.alpha_test),
                    shading: particle_shading(&s),
                    material: s.material_colour,
                    drawn: s.is_drawn(),
                    program: s.program.shader,
                    uv: uv_transforms(s.matrices),
                };
                if s.is_animated() {
                    clock = Some(Arc::new(PreviewClock {
                        look: s,
                        material: Arc::clone(&m),
                        layout,
                    }));
                }
                if s.program.shader.is_none() {
                    notes.push(format!(
                        "shader {} is not one of the particle programs of runtime §9.1.1: \
                         drawn as the common program of its blend",
                        m.shader.did
                    ));
                }
                if !s.is_drawn() {
                    notes.push(
                        "heat-distortion program: drawn as its variant without feature bit 1, \
                         which leaves the image unchanged (runtime §9.1.2, rule 11)"
                            .into(),
                    );
                }
                let load = |list: Did, notes: &mut Vec<String>| match ctx
                    .materials
                    .texture(list, Some(TEXTURE_CAP))
                {
                    Ok(t) => Some(Arc::clone(&t.image)),
                    Err(e) => {
                        notes.push(format!("texture {list}: {e}"));
                        None
                    }
                };
                match s.texture {
                    Some(list) => texture = load(list, &mut notes),
                    None => notes.push(format!("surface {surface} has no colour texture")),
                }
                if let Some(list) = s.second_layer {
                    second_texture = load(list, &mut notes);
                }
            }
            Err(e) => notes.push(format!("surface {surface}: {e}")),
        }
    }
    Ok(PreviewSource {
        did,
        desc: Arc::new(desc),
        texture,
        second_texture,
        look,
        clock,
        notes,
    })
}

/// A running simulation of the preview.
///
/// With a surface ([`PreviewSim::with_surface`]) the run keeps one set of curve states for
/// that surface for its whole length (`particle-runtime.md` §12 rule 12) and evaluates the
/// look at every drawn frame, as the walker does: at the clock of the frame drawn, after
/// the system's draw, with mode-3 curves drawing from the shared stream next (§9.1.1 (d),
/// (e); RT33); a restart starts the states afresh.
#[derive(Debug, Clone)]
pub struct PreviewSim {
    desc: Arc<ParticleSystem>,
    sim: SystemSim,
    rng: Ran2,
    seed: u32,
    frame: u64,
    time: f64,
    pending: f64,
    base: PreviewLook,
    clock: Option<Arc<PreviewClock>>,
    states: CurveStates,
    look: PreviewLook,
}

impl PreviewSim {
    /// A fresh simulation with `seed`, at time 0, drawn with the default look.
    #[must_use]
    pub fn new(desc: Arc<ParticleSystem>, seed: u32) -> Self {
        Self {
            sim: SystemSim::new(Arc::clone(&desc), 0.0),
            desc,
            rng: Ran2::new(seed),
            seed,
            frame: 0,
            time: 0.0,
            pending: 0.0,
            base: PreviewLook::default(),
            clock: None,
            states: CurveStates::new(),
            look: PreviewLook::default(),
        }
    }

    /// This simulation drawn with the surface of `src`: its look at time 0, re-evaluated at
    /// every drawn frame when the surface is animated.
    #[must_use]
    pub fn with_surface(mut self, src: &PreviewSource) -> Self {
        self.base = src.look;
        self.look = src.look;
        self.clock = src.clock.clone();
        self.states = CurveStates::new();
        self
    }

    /// Starts again from time 0 with `seed` (the surface's curve states afresh).
    pub fn restart(&mut self, seed: u32) {
        let mut fresh = Self::new(Arc::clone(&self.desc), seed);
        fresh.base = self.base;
        fresh.look = self.base;
        fresh.clock = self.clock.take();
        *self = fresh;
    }

    /// The surface's look at the last drawn frame (the look at time 0 before any draw).
    #[must_use]
    pub fn look(&self) -> PreviewLook {
        self.look
    }

    /// The seed.
    #[must_use]
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Simulated time, seconds.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The system.
    #[must_use]
    pub fn sim(&self) -> &SystemSim {
        &self.sim
    }

    fn step(&mut self) {
        self.sim.place(
            self.frame,
            self.time,
            Placement::default(),
            &Environment::default(),
            &mut self.rng,
            &OriginAttachments,
        );
        self.frame += 1;
        self.time += 1.0 / STEPS_PER_SECOND;
    }

    /// Advances by `dt` seconds of simulated time in steps of 1/30 s (at most 10 s per
    /// call).
    pub fn advance(&mut self, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        self.pending = (self.pending + dt).min(10.0);
        if self.frame == 0 {
            self.step();
        }
        while self.pending >= 1.0 / STEPS_PER_SECOND {
            self.pending -= 1.0 / STEPS_PER_SECOND;
            self.step();
        }
    }

    /// Runs until simulated time `t` (from a restart when `t` is earlier than now).
    pub fn run_to(&mut self, t: f64) {
        if t < self.time {
            self.restart(self.seed);
        }
        if self.frame == 0 {
            self.step();
        }
        while self.time + 1e-9 < t {
            self.step();
        }
    }

    /// The current frame drawn from `camera`, and the surface's look ([`PreviewSim::look`])
    /// evaluated at that frame's clock after it, in the walker's order.
    pub fn draw(&mut self, camera: &Camera) -> Option<SystemDraw> {
        let now = self.drawn_clock();
        let view = CameraView::look_at(camera.eye, camera.target);
        self.sim.instance_frame()?;
        let drawn = self.sim.draw(now, &view, &mut self.rng)?;
        if let Some(c) = &self.clock {
            self.look = c.look_at(self.base, now, &mut self.states, &mut self.rng);
        }
        Some(drawn)
    }

    /// The clock of the last placed frame: the one [`PreviewSim::draw`] draws.
    fn drawn_clock(&self) -> f64 {
        (self.time - 1.0 / STEPS_PER_SECOND).max(0.0)
    }
}

/// The renderer's draws for a simulated frame (none for a surface that is not drawn);
/// `textures` are the colour texture and the second layer.
#[must_use]
pub fn render_draws(
    sys: &SystemDraw,
    textures: [Option<TextureId>; 2],
    look: PreviewLook,
) -> Vec<ParticleDraw> {
    if !look.drawn {
        return Vec::new();
    }
    sys.instances
        .iter()
        .map(|i| ParticleDraw {
            vertices: i
                .vertices
                .iter()
                .map(|v| ParticleVertex {
                    position: v.position,
                    color: v.colour,
                    uv: v.uv,
                })
                .collect(),
            quad_vertices: i.quad_vertices,
            texture: textures[0],
            alpha: look.alpha,
            tint: [1.0, 1.0, 1.0, i.opacity],
            material: look.material,
            shading: look.shading,
            second_texture: textures[1],
            uv: look.uv,
        })
        .collect()
}

/// A bounding sphere of the drawn vertices (centre, radius at least 0.5 m).
#[must_use]
pub fn draw_sphere(sys: Option<&SystemDraw>) -> ([f32; 3], f32) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for v in sys
        .iter()
        .flat_map(|s| &s.instances)
        .flat_map(|i| &i.vertices)
    {
        let p = Vec3::from_array(v.position);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if !lo.is_finite() || !hi.is_finite() {
        return ([0.0, 0.0, 1.0], 2.0);
    }
    let c = (lo + hi) * 0.5;
    (c.to_array(), ((hi - lo).length() * 0.5).clamp(0.5, 500.0))
}

/// The height of the preview's ground: 0, or 5 cm below the lowest finite vertex of
/// `draws` (at most 500 m down).
#[must_use]
pub fn ground_height(draws: &[ParticleDraw]) -> f32 {
    let low = draws
        .iter()
        .flat_map(|d| &d.vertices)
        .map(|v| v.position[2])
        .filter(|z| z.is_finite())
        .fold(0.0f32, f32::min);
    if low < 0.0 {
        (low - 0.05).max(-500.0)
    } else {
        0.0
    }
}

/// The union of bounding spheres (centre, radius) as one sphere around their boxes.
fn union_sphere(spheres: &[([f32; 3], f32)]) -> ([f32; 3], f32) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for (c, r) in spheres {
        let c = Vec3::from_array(*c);
        lo = lo.min(c - Vec3::splat(*r));
        hi = hi.max(c + Vec3::splat(*r));
    }
    if !lo.is_finite() || !hi.is_finite() {
        return ([0.0, 0.0, 1.0], 2.0);
    }
    let c = (lo + hi) * 0.5;
    (c.to_array(), ((hi - lo).length() * 0.5).clamp(0.5, 500.0))
}

/// The preview's GPU side: a headless renderer, the surface textures and a ground plane.
pub struct PreviewGpu {
    renderer: Renderer<'static>,
    ground: MeshId,
    textures: [Option<TextureId>; 2],
    target: Option<(u32, u32, OffscreenTarget)>,
}

impl std::fmt::Debug for PreviewGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewGpu")
            .field("textures", &self.textures)
            .finish_non_exhaustive()
    }
}

impl PreviewGpu {
    /// A headless renderer.
    ///
    /// # Errors
    ///
    /// When no GPU adapter is available.
    pub fn new() -> Result<Self, String> {
        let mut renderer =
            Renderer::new_headless(&RendererConfig::default()).map_err(|e| e.to_string())?;
        let ground = renderer
            .upload_mesh(&MeshData::plane(40.0, 40.0, 1, 1))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            renderer,
            ground,
            textures: [None; 2],
            target: None,
        })
    }

    /// Replaces the surface texture of layer `layer` (0: colour, 1: second layer; `None`:
    /// white). Other layers are ignored.
    ///
    /// # Errors
    ///
    /// When the upload fails.
    pub fn set_texture(&mut self, layer: usize, image: Option<&RgbaImage>) -> Result<(), String> {
        let Some(slot) = self.textures.get_mut(layer) else {
            return Ok(());
        };
        if let Some(t) = slot.take() {
            self.renderer.destroy_texture(t);
        }
        if let Some(img) = image {
            let id = self
                .renderer
                .upload_texture(&TextureUpload {
                    color_space: ColorSpace::Srgb,
                    filter: Filter::Linear,
                    wrap: Wrap::Repeat,
                    ..TextureUpload::rgba8(img.width, img.height, &img.pixels)
                })
                .map_err(|e| e.to_string())?;
            if let Some(slot) = self.textures.get_mut(layer) {
                *slot = Some(id);
            }
        }
        Ok(())
    }

    /// The uploaded textures (colour, second layer).
    #[must_use]
    pub fn textures(&self) -> [Option<TextureId>; 2] {
        self.textures
    }

    /// Renders `draws` (texture ids already filled) from `camera` into a `w` × `h` image.
    ///
    /// # Errors
    ///
    /// When rendering or the read-back fails.
    pub fn render(
        &mut self,
        draws: Vec<ParticleDraw>,
        camera: &Camera,
        w: u32,
        h: u32,
    ) -> Result<ColorImage, String> {
        let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
        if !matches!(&self.target, Some((tw, th, _)) if *tw == w && *th == h) {
            let t = self
                .renderer
                .create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)
                .map_err(|e| e.to_string())?;
            self.target = Some((w, h, t));
        }
        let Some((_, _, target)) = &self.target else {
            return Err("no target".into());
        };
        let mut frame = self
            .renderer
            .begin_offscreen_frame(target)
            .map_err(|e| e.to_string())?;
        // The ground sits at z = 0, or just below the lowest drawn vertex, so particles
        // below the owner's origin stay visible.
        let ground_z = ground_height(&draws);
        let pass = frame.world_pass("particles", camera);
        pass.clear(BACKGROUND).mesh(MeshDraw::new(
            self.ground,
            tessera_render::glam::Mat4::from_translation(Vec3::new(0.0, 0.0, ground_z)),
            Material::color([0.16, 0.17, 0.17, 1.0]),
        ));
        let lz = ground_z + 0.002;
        for i in -10..=10 {
            let f = i as f32;
            let c = if i == 0 {
                [0.45, 0.45, 0.5, 1.0]
            } else {
                [0.25, 0.26, 0.27, 1.0]
            };
            pass.line(Vec3::new(f, -10.0, lz), Vec3::new(f, 10.0, lz), c)
                .line(Vec3::new(-10.0, f, lz), Vec3::new(10.0, f, lz), c);
        }
        for d in draws {
            pass.particles(d);
        }
        self.renderer.end_frame(frame).map_err(|e| e.to_string())?;
        let img = self
            .renderer
            .read_pixels(target)
            .map_err(|e| e.to_string())?;
        Ok(ColorImage::from_rgba_unmultiplied(
            [img.width as usize, img.height as usize],
            &img.data,
        ))
    }
}

enum Cmd {
    Texture(usize, Option<Arc<RgbaImage>>),
    Draw {
        seq: u64,
        draws: Vec<ParticleDraw>,
        camera: Camera,
        size: [u32; 2],
    },
}

enum Reply {
    Frame(u64, ColorImage),
    Error(String),
}

/// The preview's render thread.
struct PreviewThread {
    tx: Sender<Cmd>,
    rx: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
}

impl PreviewThread {
    fn spawn(notify: impl Fn() + Send + 'static) -> Self {
        let (tx, crx) = mpsc::channel::<Cmd>();
        let (rtx, rx) = mpsc::channel::<Reply>();
        let thread = std::thread::Builder::new()
            .name("explorer-particles".into())
            .spawn(move || {
                let mut gpu = match PreviewGpu::new() {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = rtx.send(Reply::Error(e));
                        notify();
                        return;
                    }
                };
                while let Ok(first) = crx.recv() {
                    // Keep texture changes in order; of the draws only the newest.
                    let mut cmds = vec![first];
                    cmds.extend(crx.try_iter());
                    let mut draw = None;
                    for c in cmds {
                        match c {
                            Cmd::Texture(layer, t) => {
                                if let Err(e) = gpu.set_texture(layer, t.as_deref()) {
                                    let _ = rtx.send(Reply::Error(e));
                                }
                            }
                            d @ Cmd::Draw { .. } => draw = Some(d),
                        }
                    }
                    if let Some(Cmd::Draw {
                        seq,
                        mut draws,
                        camera,
                        size,
                    }) = draw
                    {
                        let [tex, tex2] = gpu.textures();
                        for d in &mut draws {
                            d.texture = tex;
                            d.second_texture = tex2;
                        }
                        let reply = match gpu.render(draws, &camera, size[0], size[1]) {
                            Ok(img) => Reply::Frame(seq, img),
                            Err(e) => Reply::Error(e),
                        };
                        if rtx.send(reply).is_err() {
                            break;
                        }
                        notify();
                    }
                }
            })
            .ok();
        Self { tx, rx, thread }
    }
}

impl Drop for PreviewThread {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, tx));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The particle preview of the selected record (UI state).
pub struct ParticlePreview {
    did: u32,
    loader: Option<Receiver<Result<PreviewSource, String>>>,
    source: Option<PreviewSource>,
    error: Option<String>,
    sim: Option<PreviewSim>,
    thread: Option<PreviewThread>,
    image: Option<TextureHandle>,
    playing: bool,
    seed: u32,
    speed: f32,
    orbit: OrbitView,
    framed: bool,
    last: Option<Instant>,
    seq: u64,
    shown: u64,
    stats: String,
}

impl std::fmt::Debug for ParticlePreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParticlePreview")
            .field("did", &format_args!("{:#010x}", self.did))
            .field("playing", &self.playing)
            .field("seed", &self.seed)
            .finish_non_exhaustive()
    }
}

impl ParticlePreview {
    /// A preview of `did`; loading starts on a background thread.
    #[must_use]
    pub fn new(ctx: &Arc<DataContext>, did: u32) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = Arc::clone(ctx);
        let started = std::thread::Builder::new()
            .name("explorer-particles-load".into())
            .spawn(move || {
                let _ = tx.send(load_source(&ctx, did).map_err(|e| e.to_string()));
            })
            .is_ok();
        Self {
            did,
            loader: started.then_some(rx),
            source: None,
            error: (!started).then(|| "could not start the loader thread".to_owned()),
            sim: None,
            thread: None,
            image: None,
            playing: true,
            seed: 1,
            speed: 1.0,
            orbit: OrbitView::new(),
            framed: false,
            last: None,
            seq: 0,
            shown: 0,
            stats: String::new(),
        }
    }

    /// The record shown.
    #[must_use]
    pub fn did(&self) -> u32 {
        self.did
    }

    fn poll_loader(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.loader else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(src)) => {
                let repaint = ctx.clone();
                let thread = PreviewThread::spawn(move || repaint.request_repaint());
                let _ = thread.tx.send(Cmd::Texture(0, src.texture.clone()));
                let _ = thread.tx.send(Cmd::Texture(1, src.second_texture.clone()));
                self.thread = Some(thread);
                self.sim =
                    Some(PreviewSim::new(Arc::clone(&src.desc), self.seed).with_surface(&src));
                self.source = Some(src);
                self.loader = None;
            }
            Ok(Err(e)) => {
                self.error = Some(e);
                self.loader = None;
            }
            Err(TryRecvError::Empty) => ctx.request_repaint(),
            Err(TryRecvError::Disconnected) => {
                self.error = Some("the loader stopped".into());
                self.loader = None;
            }
        }
    }

    /// Draws the preview: controls, the image, and the simulation's counts.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.poll_loader(&ctx);
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).color(Color32::from_rgb(220, 180, 90)));
            return;
        }
        let Some(src) = &self.source else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("loading the preview …");
            });
            return;
        };
        for n in &src.notes {
            ui.label(RichText::new(n).weak());
        }
        let mut restart = false;
        ui.horizontal(|ui| {
            if ui
                .button(if self.playing {
                    "⏸ Pause"
                } else {
                    "▶ Play"
                })
                .clicked()
            {
                self.playing = !self.playing;
            }
            if ui.button("⟲ Restart").clicked() {
                restart = true;
            }
            ui.label("Seed");
            if ui.add(egui::DragValue::new(&mut self.seed)).changed() {
                restart = true;
            }
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut self.speed, 0.1..=4.0).logarithmic(true));
            if ui.button("Frame").clicked() {
                self.framed = false;
            }
        });
        let Some(sim) = &mut self.sim else {
            return;
        };
        if restart {
            sim.restart(self.seed);
            self.framed = false;
        }
        let now = Instant::now();
        let dt = self
            .last
            .map_or(0.0, |l| now.duration_since(l).as_secs_f64());
        self.last = Some(now);
        if self.playing {
            sim.advance(dt.min(0.25) * f64::from(self.speed));
            ctx.request_repaint();
        } else if sim.time() == 0.0 {
            sim.advance(1.0 / STEPS_PER_SECOND);
        }
        let width = ui.available_width().clamp(160.0, 1600.0);
        let height = (width * 0.6).clamp(120.0, 900.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.orbit.rotate(d.x, d.y);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.orbit.zoom(scroll / 50.0);
            }
        }
        let camera = self.orbit.camera();
        let drawn = sim.draw(&camera);
        let look = sim.look();
        if !self.framed && drawn.is_some() {
            let (c, r) = draw_sphere(drawn.as_ref());
            self.orbit.frame(c, r * 1.2, width / height);
            self.framed = true;
        }
        let s = sim.sim();
        self.stats = format!(
            "t = {:.2} s · {} live particles · {} vertices · spawned {} · {}{}",
            sim.time(),
            s.live_particles(),
            drawn.as_ref().map_or(0, SystemDraw::vertex_count),
            s.stats().spawned,
            if s.has_ended(sim.time()) {
                "ended"
            } else {
                "running"
            },
            if s.stopped_at().is_some() {
                " · stopped"
            } else {
                ""
            }
        );
        if let Some(thread) = &self.thread {
            while let Ok(r) = thread.rx.try_recv() {
                match r {
                    Reply::Frame(seq, img) => {
                        self.shown = seq;
                        match &mut self.image {
                            Some(h) => h.set(img, TextureOptions::LINEAR),
                            None => {
                                self.image = Some(ctx.load_texture(
                                    "particle-preview",
                                    img,
                                    TextureOptions::LINEAR,
                                ));
                            }
                        }
                    }
                    Reply::Error(e) => self.error = Some(format!("render: {e}")),
                }
            }
            // One frame in flight at a time.
            if self.shown == self.seq {
                self.seq += 1;
                let draws = drawn
                    .as_ref()
                    .map_or_else(Vec::new, |d| render_draws(d, [None; 2], look));
                let ppp = ctx.pixels_per_point();
                let _ = thread.tx.send(Cmd::Draw {
                    seq: self.seq,
                    draws,
                    camera,
                    size: [(width * ppp) as u32, (height * ppp) as u32],
                });
            }
        }
        match &self.image {
            Some(img) => {
                ui.painter().image(
                    img.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().rect_filled(rect, 0.0, Color32::from_gray(20));
            }
        }
        ui.label(RichText::new(&self.stats).weak());
    }
}

/// What `--particles DID --sheet` renders.
#[derive(Debug, Clone)]
pub struct ParticleSheetPlan {
    /// The install.
    pub game_dir: PathBuf,
    /// The output PNG.
    pub out: PathBuf,
    /// The 0x39 record.
    pub did: u32,
    /// The random seed.
    pub seed: u32,
    /// Simulated times in seconds (columns).
    pub times: Vec<f32>,
    /// Camera (yaw, pitch) in degrees (rows).
    pub angles: Vec<[f32; 2]>,
    /// Tile size in pixels.
    pub tile: [u32; 2],
}

/// Renders the particle sheet of `plan`: one tile per (angle, time); each column is a run
/// from time 0 with the plan's seed, framed on the drawn particles of the last column.
///
/// # Errors
///
/// When the install does not open, the record does not load, no GPU is available, or the
/// PNG cannot be written (never inside the install).
pub fn run_particle_sheet(plan: &ParticleSheetPlan) -> Result<SheetReport, ExplorerError> {
    let cols = plan.times.len().max(1);
    let rows = plan.angles.len().max(1);
    let [tw, th] = plan.tile;
    sheet_size(cols, rows, tw as usize, th as usize)?;
    if let Some(t) = plan
        .times
        .iter()
        .find(|t| !(t.is_finite() && (0.0..=MAX_SHEET_TIME).contains(*t)))
    {
        return Err(ExplorerError::NotApplicable(format!(
            "sheet time {t} s is outside 0–{MAX_SHEET_TIME} s"
        )));
    }
    let ctx = DataContext::open(&plan.game_dir, None)?;
    let src = load_source(&ctx, plan.did)?;
    let mut report = SheetReport {
        title: format!("particle system {:#010x}", plan.did),
        ..SheetReport::default()
    };
    let d = &src.desc;
    report.lines.push(format!(
        "particle system {:#010x}: {} emitters, time step {} s, fast forward {} s, world space {}, \
         blend {:?}, seed {}",
        plan.did,
        d.emitters.len(),
        d.time_step,
        d.fast_forward,
        d.world_space,
        src.look.alpha,
        plan.seed
    ));
    for n in &src.notes {
        report.lines.push(format!("  note {n}"));
    }
    let mut gpu = PreviewGpu::new().map_err(ExplorerError::Worker)?;
    gpu.set_texture(0, src.texture.as_deref())
        .map_err(ExplorerError::Worker)?;
    gpu.set_texture(1, src.second_texture.as_deref())
        .map_err(ExplorerError::Worker)?;
    let times: Vec<f64> = if plan.times.is_empty() {
        vec![1.0]
    } else {
        plan.times.iter().map(|&t| f64::from(t)).collect()
    };
    let angles = if plan.angles.is_empty() {
        vec![[160.0, 15.0]]
    } else {
        plan.angles.clone()
    };
    // Framing: the drawn particles of every column that draws any (a column whose effect
    // has ended adds nothing; with none drawn, the default sphere).
    let mut frame_orbit = OrbitView::new();
    let mut spheres = Vec::new();
    for &t in &times {
        let mut probe = PreviewSim::new(Arc::clone(&src.desc), plan.seed).with_surface(&src);
        probe.run_to(t);
        let drawn = probe.draw(&frame_orbit.camera());
        if drawn.as_ref().is_some_and(|d| d.vertex_count() > 0) {
            spheres.push(draw_sphere(drawn.as_ref()));
        }
    }
    let (centre, radius) = union_sphere(&spheres);
    let aspect = tw as f32 / th as f32;
    let blank = ColorImage::new(
        [tw as usize, th as usize],
        vec![Color32::BLACK; tw as usize * th as usize],
    );
    let mut images = vec![blank; cols * rows];
    for (c, &t) in times.iter().enumerate() {
        let mut sim = PreviewSim::new(Arc::clone(&src.desc), plan.seed).with_surface(&src);
        sim.run_to(t);
        let mut counts = Vec::new();
        for (r, angle) in angles.iter().enumerate() {
            frame_orbit = OrbitView::new();
            frame_orbit.orbit.yaw = angle[0].to_radians();
            frame_orbit.orbit.pitch = angle[1].to_radians();
            frame_orbit.frame(centre, radius * 1.2, aspect);
            let camera = frame_orbit.camera();
            let drawn = sim.draw(&camera);
            counts.push(drawn.as_ref().map_or(0, SystemDraw::vertex_count));
            let draws = drawn
                .as_ref()
                .map_or_else(Vec::new, |s| render_draws(s, gpu.textures(), sim.look()));
            let img = gpu
                .render(draws, &camera, tw, th)
                .map_err(ExplorerError::Worker)?;
            if let Some(slot) = images.get_mut(r * cols + c) {
                *slot = img;
            }
        }
        let s = sim.sim();
        report.lines.push(format!(
            "  column {c}: t = {t:.2} s, {} live particles, {} spawned, vertices per row {counts:?}, {}",
            s.live_particles(),
            s.stats().spawned,
            if s.has_ended(sim.time()) {
                "ended"
            } else {
                "running"
            }
        ));
    }
    let sheet = tile(&images, cols, rows, tw as usize, th as usize);
    drop(images);
    let guards = vec![plan.game_dir.clone()];
    let written = crate::screenshot::save_png(&plan.out, &sheet, &guards)?;
    report.written = Some(written);
    report.lines.push(format!(
        "sheet: {rows} rows (angles {angles:?}) × {cols} columns (times {times:?}), tiles \
         {tw}×{th}, gutter {GUTTER} px; framed on centre {centre:?}, radius {radius:.2} m"
    ));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::Did;
    use tessera_formats::gameplay::particles::{Curve, Emitter, Keyframe};

    fn system() -> Arc<ParticleSystem> {
        let e = Emitter {
            emit_interval: 0.05,
            lifespan: Curve::constant(1.0),
            keyframes: vec![Keyframe::default()],
            ..Emitter::default()
        };
        Arc::new(ParticleSystem {
            time_step: 1.0 / 30.0,
            fast_forward: 0.0,
            physics_duration: 0.0,
            world_space: true,
            force_draw: false,
            uses_video_post: false,
            draw_reflection: false,
            use_lighting: false,
            inherit_opacity: false,
            far_fade_start: 40.0,
            far_fade_end: 60.0,
            near_fade_start: 0.0,
            near_fade_end: 0.0,
            scale_mode: 1,
            emitters: vec![e],
            has_bounds: false,
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
            sorting_priority: 5,
            shareable: false,
            surface: Did(0x3100_0001),
            extra_surface: Did(0),
        })
    }

    /// A resolved unlit particle material with the given blend and no mesh alpha test
    /// (the particle test comes from the surface, runtime §9.1.3).
    fn particle_material(mode: BlendMode, dst: MatFactor, depth_write: bool) -> ResolvedMaterial {
        use tessera_material::{BlendState, FeatureFlags, ParameterSet, ShaderInterface};
        let shader = Arc::new(ShaderInterface {
            did: Did(0x2B00_0001),
            declarations: Vec::new(),
            slots: Vec::new(),
            permutation_count: 1,
            lit: false,
            common_blend: None,
            cull_none: false,
            default_alpha_ref: None,
            family: Default::default(),
            unbound_defaults: Vec::new(),
        });
        let params = ParameterSet::build(&shader, &[]);
        ResolvedMaterial {
            surface: Some(Did(0x3100_0001)),
            binding_key: Some(1),
            shader,
            materials: Vec::new(),
            skipped: Vec::new(),
            params,
            slots: Vec::new(),
            features: FeatureFlags {
                lit: false,
                alpha_test: None,
                opacity_slot: Some(3000),
                blend: BlendState {
                    mode,
                    src: MatFactor::SrcAlpha,
                    dst,
                    op: MatOp::Add,
                    depth_write,
                    opacity_slot: Some(3000),
                },
                two_sided: true,
                normal_mapped: false,
                specular: false,
                dyeable: false,
                dyeable_2: false,
                emissive: false,
                glow: false,
                environment: false,
                second_layer: false,
                baked_lighting: false,
                uv_transform: false,
                animated: false,
            },
            dye: None,
        }
    }

    /// The alpha-test reference the renderer applies to a draw of `mode` at *A* = `a`
    /// (`particle-runtime.md` §9.1.1 rule 1 through `material.md` §11.2).
    fn reference(mode: AlphaMode, a: f32) -> Option<f32> {
        let (test, depth_write) = match mode {
            AlphaMode::Fixed(f) => (f.alpha_test.map(|t| t.value()), f.depth_write),
            AlphaMode::Blend | AlphaMode::Additive => (None, false),
            other => panic!("unexpected particle mode {other:?}"),
        };
        tessera_render::draw_reference(test, depth_write, true, a)
    }

    /// `particle-runtime.md` §9.1.1 rule 1: a surface with alpha test on and depth write
    /// off keeps its material's reference while *A* ≥ 0.9 and tests nothing but opacity 0
    /// below; without alpha test the reference is 0.
    #[test]
    fn alpha_tested_surfaces_without_depth_write_keep_their_reference() {
        let r = (0.5_f32 * 255.0).floor() / 255.0;
        for (mode, dst, factor) in [
            (
                BlendMode::AlphaBlend,
                MatFactor::InvSrcAlpha,
                BlendFactor::OneMinusSrcAlpha,
            ),
            (BlendMode::Additive, MatFactor::One, BlendFactor::One),
        ] {
            let alpha = surface_alpha(&particle_material(mode, dst, false), Some(0.5));
            let AlphaMode::Fixed(f) = alpha else {
                panic!("{mode:?}: {alpha:?}");
            };
            assert_eq!(f.src, BlendFactor::SrcAlpha);
            assert_eq!(f.dst, factor);
            assert_eq!(f.op, BlendOp::Add);
            assert!(!f.depth_write);
            assert_eq!(reference(alpha, 1.0), Some(r), "{mode:?}");
            assert_eq!(reference(alpha, 0.9), Some(r), "{mode:?}");
            assert_eq!(reference(alpha, 0.85), Some(0.0), "{mode:?}");

            let untested = surface_alpha(&particle_material(mode, dst, false), None);
            assert!(matches!(untested, AlphaMode::Blend | AlphaMode::Additive));
            assert_eq!(reference(untested, 1.0), Some(0.0), "{mode:?}");
        }
        // Depth write on: the reference is scaled by *A*.
        let written = surface_alpha(
            &particle_material(BlendMode::Additive, MatFactor::One, true),
            Some(0.5),
        );
        assert_eq!(
            reference(written, 0.5),
            Some((0.25_f32 * 255.0).floor() / 255.0)
        );
    }

    #[test]
    fn preview_simulation_is_seeded_and_restartable() {
        let mut a = PreviewSim::new(system(), 3);
        a.run_to(1.0);
        let n = a.sim().live_particles();
        assert!(n > 10, "{n}");
        let cam = OrbitView::new().camera();
        let draw = a.draw(&cam).expect("drawn");
        let additive = PreviewLook {
            alpha: AlphaMode::Additive,
            ..PreviewLook::default()
        };
        assert_eq!(render_draws(&draw, [None; 2], additive).len(), 1);
        // The heat-distortion program is not drawn (runtime §9.1.2, rule 11).
        let hidden = PreviewLook {
            drawn: false,
            ..additive
        };
        assert!(render_draws(&draw, [None; 2], hidden).is_empty());
        let mut b = PreviewSim::new(system(), 3);
        b.advance(0.5);
        b.advance(0.5);
        assert_eq!(a.sim().live_particles(), b.sim().live_particles());
        a.run_to(0.2);
        assert!(a.time() < 0.3);
        a.restart(9);
        assert_eq!((a.seed(), a.time()), (9, 0.0));
        a.advance(f64::NAN);
        assert_eq!(a.time(), 0.0);
        let (c, r) = draw_sphere(None);
        assert!(r >= 0.5 && c[2].is_finite());
    }

    /// An animated particle surface: program 0x2B00_070F (it tests the texture) with alpha
    /// test 4004 on and reference 1043 = `reference`, on a blend without depth write.
    fn animated_clock(reference: tessera_material::FloatCurve) -> Arc<PreviewClock> {
        use tessera_material::records::{
            PERMUTATION_KEY_WORDS, ParamEntry, ParamValue, Permutation, RENDER_STATE_SIZE,
            RenderState,
        };
        use tessera_material::{ShaderInterface, ShaderRecord};
        let mut raw = [0u8; RENDER_STATE_SIZE];
        // SRCALPHA / INVSRCALPHA, no depth write.
        raw[..7].copy_from_slice(&[5, 6, 1, 7, 0, 2, 0]);
        let record = ShaderRecord {
            did: Did(0x2B00_070F),
            reserved: 0,
            params: vec![
                ParamEntry {
                    id: 1043,
                    group: 1000,
                    value: ParamValue::Float(reference),
                    flag: 0,
                    bindings: Vec::new(),
                },
                ParamEntry {
                    id: 4004,
                    group: 4000,
                    value: ParamValue::Flag(1),
                    flag: 0,
                    bindings: Vec::new(),
                },
            ],
            permutations: vec![Permutation {
                key: [0; PERMUTATION_KEY_WORDS],
                programs: None,
                samplers: Vec::new(),
                extra_items: Vec::new(),
                render_state: RenderState { raw },
            }],
        };
        let layout = Arc::new(TextureMatrixLayout::from_record(&record));
        let shader = Arc::new(ShaderInterface::from_record(&record));
        let material = Arc::new(ResolvedMaterial::resolve(shader, &[]));
        let look = ParticleSurface::of(&material, &layout);
        assert!(look.is_animated());
        Arc::new(PreviewClock {
            look,
            material,
            layout,
        })
    }

    /// The look is evaluated at every drawn frame, at that frame's clock and after the
    /// system's draw, as the walker does (`particle-runtime.md` §9.1.1 (e), rule 12, RT33).
    #[test]
    fn surface_look_follows_the_drawn_frame() {
        let cam = Camera {
            eye: Vec3::new(0.0, -4.0, 1.0),
            target: Vec3::new(0.0, 0.0, 0.5),
            ..Camera::default()
        };
        let source = |clock: Arc<PreviewClock>| PreviewSource {
            did: 0x3900_0001,
            desc: system(),
            texture: None,
            second_texture: None,
            look: PreviewLook::default(),
            clock: Some(clock),
            notes: Vec::new(),
        };
        let threshold = |look: PreviewLook| match look.alpha {
            AlphaMode::Fixed(f) => f.alpha_test.map(|t| t.value()),
            _ => None,
        };
        // A ramp 0.5 + 0.1 t: the drawn frame's clock, not the next step's.
        let mut ramp = tessera_material::FloatCurve::constant(0.5);
        ramp.mode = 2;
        ramp.params[1] = 0.1;
        ramp.param_count = 10;
        let mut sim = PreviewSim::new(system(), 1).with_surface(&source(animated_clock(ramp)));
        sim.run_to(1.0);
        let now = sim.drawn_clock();
        assert!(sim.draw(&cam).is_some());
        let got = threshold(sim.look()).expect("tested");
        assert!(
            (f64::from(got) - (0.5 + 0.1 * now)).abs() < 1e-5,
            "{got} at {now}"
        );
        // A mode-3 flicker 0.5 + 0.25 r takes its r from the shared stream after the
        // system's draw.
        let mut flicker = tessera_material::FloatCurve::constant(0.5);
        flicker.mode = 3;
        flicker.params[2] = 0.25;
        flicker.param_count = 10;
        let clock = animated_clock(flicker);
        let mut sim = PreviewSim::new(system(), 5).with_surface(&source(Arc::clone(&clock)));
        sim.run_to(0.5);
        let mut probe = sim.clone();
        let view = CameraView::look_at(cam.eye, cam.target);
        let now = probe.drawn_clock();
        assert!(probe.sim.draw(now, &view, &mut probe.rng).is_some());
        let want = clock.look_at(
            PreviewLook::default(),
            now,
            &mut CurveStates::new(),
            &mut probe.rng,
        );
        assert!(sim.draw(&cam).is_some());
        assert_eq!(sim.look(), want);
        assert_eq!(sim.rng, probe.rng, "the same draws from the shared stream");
    }

    #[test]
    fn renders_a_synthetic_system_when_a_gpu_exists() {
        let Ok(mut gpu) = PreviewGpu::new() else {
            return;
        };
        let mut sim = PreviewSim::new(system(), 1);
        sim.run_to(1.0);
        let cam = Camera {
            eye: Vec3::new(0.0, -4.0, 1.0),
            target: Vec3::new(0.0, 0.0, 0.5),
            ..Camera::default()
        };
        let drawn = sim.draw(&cam).expect("drawn");
        let img = gpu
            .render(
                render_draws(&drawn, [None; 2], PreviewLook::default()),
                &cam,
                64,
                48,
            )
            .expect("rendered");
        assert_eq!(img.size, [64, 48]);
    }

    #[test]
    fn ground_drops_below_low_particles_and_spheres_unite() {
        let v = |z: f32| ParticleVertex {
            position: [0.0, 0.0, z],
            color: [1.0; 4],
            uv: [0.0; 2],
        };
        assert_eq!(ground_height(&[]), 0.0);
        let low = ParticleDraw::new(vec![v(0.5), v(-0.1), v(f32::NAN)], 0);
        assert!((ground_height(&[low]) + 0.15).abs() < 1e-6);
        let (c, r) = union_sphere(&[([0.0, 0.0, 0.0], 1.0), ([4.0, 0.0, 0.0], 1.0)]);
        assert!((c[0] - 2.0).abs() < 1e-6 && r >= 3.0);
        assert_eq!(union_sphere(&[]), ([0.0, 0.0, 1.0], 2.0));
    }
}
