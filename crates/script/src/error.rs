// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the Lua core.

/// Errors returned by [`ScriptState`](crate::ScriptState) operations.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The Lua runtime failed (state creation, out of memory, a host call that raised).
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    /// Source text was a precompiled (binary) chunk; only source text is compiled.
    #[error("{0} is a binary chunk; only source text is allowed")]
    BinaryChunk(String),
    /// Source text was larger than [`Limits::max_source_bytes`](crate::Limits).
    #[error("{name} is larger than {limit} bytes")]
    SourceTooLarge {
        /// Chunk name.
        name: String,
        /// The limit.
        limit: u64,
    },
}
