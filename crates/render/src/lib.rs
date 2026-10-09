// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render` — the wgpu renderer foundation for Tessera.
//!
//! The crate is independent of any windowing library: hand it a window that
//! implements the `raw-window-handle` traits ([`Renderer::new_windowed`]), any
//! [`wgpu::SurfaceTarget`] ([`Renderer::new_with_surface_target`]), or run it
//! without a surface ([`Renderer::new_headless`]) and render into
//! [`OffscreenTarget`]s.
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! frame
//!     .pass("overlay", PassSpace::Screen)
//!     .clear([0.0, 0.0, 0.0, 1.0])
//!     .quad(Quad::screen_rect(16.0, 16.0, 32.0, 32.0).with_color([1.0, 0.0, 0.0, 1.0]));
//! r.end_frame(frame)?;
//! let image = r.read_pixels(&target)?;
//! assert_eq!(image.pixel(32, 32), Some([255, 0, 0, 255]));
//! # Ok(()) }
//! ```
//!
//! Meshes (M2): [`Renderer::upload_mesh`] copies engine-neutral
//! [`MeshData`] streams into shared, sub-allocated vertex/index buffers and
//! returns a [`MeshId`]; [`Pass::mesh`] draws it with a [`Material`]
//! (opaque, alpha-test, alpha-blend sorted back to front, two-sided, dye,
//! normal map, specular) under a pass's [`Lighting`], optionally with a
//! [`MeshDebug`] view (wireframe, normals, UV checker).
//! [`OrbitCamera::frame_sphere`] frames a mesh's [`BoundingSphere`].
//!
//! Skinned meshes (M4): [`Renderer::upload_skinned_mesh`] adds a
//! [`MeshSkin`] (four bone indices and weights per vertex) to the mesh
//! streams; [`Frame::skin_palette`] uploads one palette of skinning
//! matrices per character and frame, and [`MeshDraw::with_skin`] draws any
//! number of parts with it. Skinning runs in the vertex shader for every
//! material mode and debug view.
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::glam::Mat4;
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! let data = MeshData::plane(1.0, 2.0, 1, 4);
//! // Every vertex fully on bone 1 of a two-bone skeleton.
//! let skin = MeshSkin::rigid(data.positions.len(), 1, 2);
//! let mesh = r.upload_skinned_mesh(&data, &skin)?;
//! let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! let palette = frame.skin_palette(&[Mat4::IDENTITY, Mat4::from_rotation_z(0.5)])?;
//! frame
//!     .world_pass("skinned", &Camera::default())
//!     .mesh(MeshDraw::new(mesh, Mat4::IDENTITY, Material::default()).with_skin(palette));
//! r.end_frame(frame)?;
//! # Ok(()) }
//! ```
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::glam::Mat4;
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! let mesh = r.upload_mesh(&MeshData::uv_sphere(1.0, 32, 16))?;
//! let sphere = r.mesh_info(mesh).map(|i| i.sphere).expect("live mesh");
//! let mut orbit = OrbitCamera::default();
//! orbit.frame_sphere(sphere, 4.0 / 3.0);
//! let target = r.create_offscreen_target(320, 240, OffscreenFormat::Rgba8Srgb)?;
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! frame
//!     .world_pass("mesh", &orbit.camera())
//!     .clear([0.1, 0.1, 0.1, 1.0])
//!     .lighting(Lighting::default())
//!     .mesh(MeshDraw::new(mesh, Mat4::IDENTITY, Material {
//!         alpha: AlphaMode::Mask { cutoff: 0.5 },
//!         ..Material::color([0.8, 0.3, 0.2, 1.0])
//!     }));
//! r.end_frame(frame)?;
//! # Ok(()) }
//! ```
//!
//! Lit world passes (M4.5): [`Pass::frame_lighting`] makes a world pass draw its meshes
//! with the lighting of `docs/specs/lighting.md` §5 under a [`FrameLighting`] (sun or moon
//! with its [`SunShadow`], ambient and hemispheric terms, distance [`Fog`], a table of
//! point and directional [`Light`]s), in the original's stored-value colour space. The
//! inputs only lit passes use are a [`LitSurface`] on each [`MeshDraw`]
//! ([`MeshDraw::with_lit`]): glow and emissive, gloss and opacity sources, vertex colours,
//! fog, shadow casting and light selection. [`Renderer::upload_texture_mips`] uploads a
//! texture's stored mip chain ([`MipChainUpload`]). [`lighting::reference`] is a CPU twin of
//! the shader equations.
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::glam::{Mat4, Vec3};
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! let ground = r.upload_mesh(&MeshData::plane(40.0, 40.0, 1, 1))?;
//! let target = r.create_offscreen_target(320, 240, OffscreenFormat::Rgba8Srgb)?;
//! let toward_sun = Vec3::new(0.3, -0.2, 0.9);
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! frame
//!     .world_pass("world", &Camera::default())
//!     .frame_lighting(FrameLighting {
//!         sun: Some(SunLight { direction: toward_sun, color: [0.6; 3], specular: [1.0; 3] }),
//!         shadow: Some(SunShadow::fit(toward_sun, Vec3::ZERO, 20.0, 80.0, 2048)),
//!         fog: Some(Fog { near: 20.0, far: 200.0, color: [0.6, 0.65, 0.7] }),
//!         lights: vec![Light::point(Vec3::new(0.0, 0.0, 2.0), 8.0, 0.1, [1.0, 0.8, 0.5])],
//!         ..FrameLighting::default()
//!     })
//!     .mesh(
//!         MeshDraw::new(ground, Mat4::IDENTITY, Material::color([0.5, 0.45, 0.4, 1.0]))
//!             .with_lit(LitSurface { opacity: Opacity::One, ..LitSurface::default() }),
//!     );
//! r.end_frame(frame)?;
//! # Ok(()) }
//! ```
//!
//! World pipelines (M4.5, `docs/specs/render.md` §5) draw into lit passes:
//! [`Pass::terrain`] draws a [`TerrainChunk`] uploaded with
//! [`Renderer::upload_terrain_chunk`] (layers blended per `terrain-texturing.md` §7–§9),
//! [`Pass::water`] draws [`WaterQuad`]s refracting the scene drawn before them
//! (`water.md` §8), [`Pass::sky`] draws [`SkyObject`]s around the camera
//! (`environment.md` §12) and [`Pass::instanced`] draws thousands of instances of a
//! [`WorldModel`] per call, with the frill rules of `scenery.md` §8.3 for [`FrillParams`]
//! batches. [`Pass::far_terrain`] draws distant-landscape batches uploaded with
//! [`Renderer::upload_far_terrain`] and [`Pass::distant_sprites`] the sector sprites
//! uploaded with [`Renderer::upload_sprite_batch`] (`far-view.md` §6, §9).
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::glam::{Mat4, Vec3};
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! let rock = r.upload_world_model(&MeshData::cube(1.0), &[WorldSurface::default()])?;
//! let instances: Vec<Instance> = (0..1_000)
//!     .map(|i| Instance::new(Mat4::from_translation(Vec3::new(i as f32, 5.0, 0.0))))
//!     .collect();
//! let target = r.create_offscreen_target(320, 240, OffscreenFormat::Rgba8Srgb)?;
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! frame
//!     .world_pass("world", &Camera::default())
//!     .frame_lighting(FrameLighting::default())
//!     .instanced(InstanceBatch::new(rock, instances))
//!     .water(WaterDraw::new(
//!         [WaterQuad::landblock([0.0, 0.0], 2.0)],
//!         None,
//!         WaterParams::default(),
//!     ));
//! r.end_frame(frame)?;
//! # Ok(()) }
//! ```
//!
//! Particles (`docs/specs/particle-runtime.md` §9): [`Pass::particles`] draws one
//! [`ParticleDraw`] per draw instance of a particle system, world-space
//! [`ParticleVertex`]es with quads first and triangles after, in the caller's order (no
//! sorting), with the blend, alpha test and depth write of an [`AlphaMode`] and the
//! surface program's [`ParticleShading`] (§9.1.1: texture-only alpha test, output alpha,
//! doubling, fog, two layers), in lit and unlit world passes.
//! Game UI (M5): [`Renderer::create_ui_texture`] and
//! [`Renderer::update_texture_rect`] hold panel canvases, and
//! [`Frame::ui_pass`] composes them with transform and clip chains; see the
//! `ui` types ([`UiEntry`], [`UiPass`]) and `docs/specs/render.md` §6.
//!
//! Adapter selection honours `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and
//! `WGPU_POWER_PREF` (see [`ContextOptions`]); asynchronous GPU errors are
//! captured and returned as [`RenderError::Gpu`] instead of panicking.

mod arena;
mod camera;
mod context;
mod error;
mod far_view;
mod frame;
mod instanced;
pub mod lighting;
mod lit_pipelines;
mod material;
mod mesh;
mod mesh_pipelines;
mod particles;
mod pass_hook;
mod pipelines;
mod renderer;
mod shapes;
mod skin;
mod sky;
mod terrain_layers;
mod texture;
mod ui;
mod water;

pub use arena::{ArenaStats, MeshConfig, MeshId, RangeAllocator};
pub use camera::{Camera, OrbitCamera, OrbitConfig, Perspective, UpAxis};
pub use context::{
    ContextOptions, ENV_ADAPTER_NAME, ENV_BACKEND, ENV_POWER_PREF, GPU_WAIT_TIMEOUT, GpuContext,
    rank_adapter,
};
pub use error::{RenderError, Result};
pub use far_view::{
    DEFAULT_SPRITE_ALPHA_TEST, DistantSprite, FAR_GRID_CULL, FAR_STITCH_CULL, FarFadeParams,
    FarTerrainData, FarTerrainDraw, FarTerrainMesh, MAX_FAR_TERRAIN_INDICES,
    MAX_FAR_TERRAIN_VERTICES, MAX_SPRITES_PER_BATCH, SPRITE_ALPHA_MARGIN, SPRITE_CULL, SpriteBatch,
    SpriteDraw, sprite_texel_kept,
};
pub use frame::{Frame, Pass, PassSpace, Quad, Rgba, screen_projection};
pub use instanced::{
    FRILL_FADE_IN_SECONDS, FRILL_SECTOR, FRILL_SWAY_MIN, FrillParams, FrillReduction, Instance,
    InstanceBatch, MAX_INSTANCES_PER_PASS, MAX_MODEL_SURFACES, WorldModel, WorldSurface,
    WorldSurfaceOptions, frill_distances, frill_fade_in, frill_radius_sectors, frill_reduced_count,
};
pub use lighting::{
    FOG_FAR_CAP, FRAME_SHADING_WGSL, FRAME_WGSL, Fog, FrameLighting, Light, LightFields, LightKind,
    LightReach, LightRecord, MAX_DRAW_LIGHTS, MAX_FRAME_LIGHTS, SELECTION_FRACTION, SunLight,
    SunShadow,
};
pub use material::{
    AlphaMode, AlphaThreshold, BlendFactor, BlendOp, Dye, DyeMask, FixedBlend, Gloss,
    LightSelection, Lighting, LitSurface, Material, MeshDebug, MeshDraw, MeshShading, NormalLines,
    Opacity, Specular,
};
pub use mesh::{
    Aabb, BoundingSphere, Indices, MeshData, MeshInfo, MeshLimits, PreparedMesh, Submesh,
    SubmeshInfo,
};
pub use particles::{
    MAX_PARTICLE_DRAW_VERTICES, MAX_PARTICLE_DRAWS_PER_PASS, MAX_PARTICLE_VERTICES_PER_PASS,
    ParticleDraw, ParticleLayers, ParticleShading, ParticleVertex, draw_reference,
};
pub use pipelines::{DEPTH_CLEAR, DEPTH_COMPARE, DEPTH_FORMAT, LineVertex};
pub use renderer::{
    FrameStats, OffscreenFormat, OffscreenTarget, Renderer, RendererConfig, RgbaImage,
};
pub use shapes::MAX_SEGMENTS as MAX_SHAPE_SEGMENTS;
pub use skin::{MAX_PALETTE_BONES, MeshSkin, SkinInfo, SkinPalette, WEIGHT_SUM_TOLERANCE};
pub use sky::{MAX_SKY_OBJECTS, SkyDraw, SkyObject};
pub use terrain_layers::{
    MAX_TERRAIN_LAYERS, MAX_TERRAIN_TRIANGLES, MAX_TERRAIN_VERTICES, TerrainChunk,
    TerrainChunkData, TerrainChunkInfo, TerrainDraw, TerrainGloss, TerrainLayer,
    TerrainLayerTextures, TerrainOpacity, TerrainProjection, TerrainSpecular, TerrainTransform,
    TerrainVertices,
};
pub use texture::{ColorSpace, Filter, MipChainUpload, MipLevel, TextureId, TextureUpload, Wrap};
pub use ui::{
    MAX_UI_CLIPS, MAX_UI_COORD, MAX_UI_ENTRIES, MAX_UI_ROTATIONS, MAX_UI_TEXTURE_DIM,
    UI_ANGLE_EPSILON, UI_BLEND_ALPHA, UI_BLEND_COLOR, UI_NODE_SCALE_RANGE, UiAddressing,
    UiCapCounters, UiClip, UiClipMap, UiEntry, UiFilter, UiGeometry, UiPass, UiRect, UiRefusal,
    UiRotation, UiSource, UiStats, UiViewMode,
};
pub use water::{
    MAX_WATER_QUADS, MAX_WATER_SUBDIVISIONS, MAX_WATER_VERTICES, WATER_BLOCK, WaterDraw,
    WaterParams, WaterQuad, fresnel as water_fresnel, water_fade, wave_height as water_wave_height,
};

/// Re-export of the math library used in the public API.
pub use glam;
/// Re-export so callers can name wgpu types without a version mismatch.
pub use wgpu;
