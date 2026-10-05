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
//! Adapter selection honours `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and
//! `WGPU_POWER_PREF` (see [`ContextOptions`]); asynchronous GPU errors are
//! captured and returned as [`RenderError::Gpu`] instead of panicking.

mod arena;
mod camera;
mod context;
mod error;
mod frame;
mod material;
mod mesh;
mod mesh_pipelines;
mod pipelines;
mod renderer;
mod shapes;
mod skin;
mod texture;

pub use arena::{ArenaStats, MeshConfig, MeshId, RangeAllocator};
pub use camera::{Camera, OrbitCamera, OrbitConfig, Perspective, UpAxis};
pub use context::{
    ContextOptions, ENV_ADAPTER_NAME, ENV_BACKEND, ENV_POWER_PREF, GPU_WAIT_TIMEOUT, GpuContext,
    rank_adapter,
};
pub use error::{RenderError, Result};
pub use frame::{Frame, Pass, PassSpace, Quad, Rgba, screen_projection};
pub use material::{
    AlphaMode, Dye, DyeMask, Lighting, Material, MeshDebug, MeshDraw, MeshShading, NormalLines,
    Specular,
};
pub use mesh::{
    Aabb, BoundingSphere, Indices, MeshData, MeshInfo, MeshLimits, PreparedMesh, Submesh,
    SubmeshInfo,
};
pub use pipelines::{DEPTH_CLEAR, DEPTH_COMPARE, DEPTH_FORMAT, LineVertex};
pub use renderer::{
    FrameStats, OffscreenFormat, OffscreenTarget, Renderer, RendererConfig, RgbaImage,
};
pub use shapes::MAX_SEGMENTS as MAX_SHAPE_SEGMENTS;
pub use skin::{MAX_PALETTE_BONES, MeshSkin, SkinInfo, SkinPalette, WEIGHT_SUM_TOLERANCE};
pub use texture::{ColorSpace, Filter, TextureId, TextureUpload, Wrap};

/// Re-export of the math library used in the public API.
pub use glam;
/// Re-export so callers can name wgpu types without a version mismatch.
pub use wgpu;
