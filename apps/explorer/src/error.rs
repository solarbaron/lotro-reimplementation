// SPDX-License-Identifier: GPL-3.0-or-later
//! The explorer's error type.

use std::sync::Arc;

/// Errors of the explorer's data layer and decoders.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum ExplorerError {
    /// The archive layer failed.
    #[error("archive: {0}")]
    Dat(Arc<tessera_dat::Error>),
    /// A record did not decode.
    #[error("decode: {0}")]
    Format(#[from] tessera_formats::FormatError),
    /// Property access failed.
    #[error("properties: {0}")]
    Props(#[from] tessera_props::PropsError),
    /// String access failed.
    #[error("strings: {0}")]
    Text(Arc<tessera_text::TextError>),
    /// Icon composition or material resolution failed.
    #[error("material: {0}")]
    Material(Arc<tessera_material::MaterialError>),
    /// A mesh record did not decode or flatten.
    #[error("mesh: {0}")]
    Mesh(Arc<tessera_mesh::MeshError>),
    /// The renderer refused data.
    #[error("render: {0}")]
    Render(Arc<tessera_render::RenderError>),
    /// The record is not there.
    #[error("record {0:#010x} not found")]
    NotFound(u32),
    /// The view does not apply to this record.
    #[error("{0}")]
    NotApplicable(String),
    /// A background thread failed.
    #[error("background task: {0}")]
    Worker(String),
    /// The work was cancelled (the selection moved on).
    #[error("cancelled")]
    Cancelled,
    /// Writing a file (screenshot) failed.
    #[error("{0}")]
    Output(String),
}

impl From<tessera_dat::Error> for ExplorerError {
    fn from(e: tessera_dat::Error) -> Self {
        Self::Dat(Arc::new(e))
    }
}

impl From<tessera_text::TextError> for ExplorerError {
    fn from(e: tessera_text::TextError) -> Self {
        Self::Text(Arc::new(e))
    }
}

impl From<tessera_material::MaterialError> for ExplorerError {
    fn from(e: tessera_material::MaterialError) -> Self {
        Self::Material(Arc::new(e))
    }
}

impl From<tessera_mesh::MeshError> for ExplorerError {
    fn from(e: tessera_mesh::MeshError) -> Self {
        Self::Mesh(Arc::new(e))
    }
}

impl From<tessera_render::RenderError> for ExplorerError {
    fn from(e: tessera_render::RenderError) -> Self {
        Self::Render(Arc::new(e))
    }
}
