// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type for the renderer.

use crate::texture::TextureId;

/// Everything that can go wrong inside `tessera-render`.
///
/// GPU validation errors raised asynchronously by wgpu are captured by the
/// renderer (instead of wgpu's default panicking handler) and surfaced as
/// [`RenderError::Gpu`] from the next call that checks them.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// No adapter matched the requested backends / filters.
    #[error("no suitable GPU adapter found (backends: {backends}): {reason}")]
    NoAdapter {
        /// Human readable list of the backends that were searched.
        backends: String,
        /// Why every candidate was rejected.
        reason: String,
    },
    /// The adapter refused to create a device.
    #[error("failed to create GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    /// The platform surface could not be created.
    #[error("failed to create presentation surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    /// The selected adapter cannot present to the given surface.
    #[error("the selected adapter cannot present to this surface")]
    SurfaceUnsupported,
    /// A surface operation was requested on a headless renderer.
    #[error("renderer has no presentation surface (headless)")]
    NoSurface,
    /// The surface was lost and must be recreated with `Renderer::replace_surface`.
    #[error("presentation surface lost; it must be recreated")]
    SurfaceLost,
    /// Acquiring the next surface texture raised a validation error.
    #[error("validation error while acquiring the surface texture")]
    SurfaceValidation,
    /// A texture upload was rejected before reaching the GPU.
    #[error("invalid texture upload: {0}")]
    InvalidTexture(String),
    /// A texture handle is stale or was never issued by this renderer.
    #[error("unknown or destroyed texture {0:?}")]
    UnknownTexture(TextureId),
    /// An offscreen target was requested with unusable dimensions.
    #[error("invalid render target: {0}")]
    InvalidTarget(String),
    /// Reading pixels back from the GPU failed.
    #[error("pixel readback failed: {0}")]
    Readback(String),
    /// Waiting on the device failed.
    #[error("device poll failed: {0}")]
    Poll(#[from] wgpu::PollError),
    /// A handle (offscreen target, frame) created by a different
    /// [`crate::Renderer`] was passed to this one. Texture handles report
    /// [`RenderError::UnknownTexture`] instead.
    #[error("{0} belongs to another renderer")]
    ForeignHandle(&'static str),
    /// A frame or resource is larger than this device allows. This is a
    /// caller-side problem (split the work); the device itself is healthy.
    #[error("{what} needs {requested}, device/renderer maximum is {max}")]
    LimitExceeded {
        /// What hit the limit.
        what: &'static str,
        /// Requested amount (bytes or count, see `what`).
        requested: u64,
        /// Maximum allowed.
        max: u64,
    },
    /// An asynchronous GPU error (validation, OOM, internal) was captured.
    #[error("GPU error: {0}")]
    Gpu(String),
}

/// Convenience alias.
pub type Result<T, E = RenderError> = std::result::Result<T, E>;
