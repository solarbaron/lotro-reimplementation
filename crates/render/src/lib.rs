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
//! Adapter selection honours `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and
//! `WGPU_POWER_PREF` (see [`ContextOptions`]); asynchronous GPU errors are
//! captured and returned as [`RenderError::Gpu`] instead of panicking.

mod camera;
mod context;
mod error;
mod frame;
mod pipelines;
mod renderer;
mod texture;

pub use camera::{Camera, OrbitCamera, OrbitConfig, Perspective, UpAxis};
pub use context::{
    ContextOptions, ENV_ADAPTER_NAME, ENV_BACKEND, ENV_POWER_PREF, GpuContext, rank_adapter,
};
pub use error::{RenderError, Result};
pub use frame::{Frame, Pass, PassSpace, Quad, Rgba, screen_projection};
pub use pipelines::{DEPTH_FORMAT, LineVertex};
pub use renderer::{
    FrameStats, OffscreenFormat, OffscreenTarget, Renderer, RendererConfig, RgbaImage,
};
pub use texture::{ColorSpace, Filter, TextureId, TextureUpload, Wrap};

/// Re-export of the math library used in the public API.
pub use glam;
/// Re-export so callers can name wgpu types without a version mismatch.
pub use wgpu;
