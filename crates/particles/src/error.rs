// SPDX-License-Identifier: GPL-3.0-or-later
//! The crate's error type.

use thiserror::Error;

/// Errors of `tessera-particles`.
#[derive(Debug, Error)]
pub enum ParticleError {
    /// The record did not decode.
    #[error("particle record: {0}")]
    Decode(#[from] tessera_formats::FormatError),
    /// The DID names neither a particle-system nor a fork-system descriptor.
    #[error("{0:#010x} is not a particle or fork system")]
    NotAnEffect(u32),
}

/// Results of this crate.
pub type Result<T> = std::result::Result<T, ParticleError>;
