// SPDX-License-Identifier: GPL-3.0-or-later
//! Resource limits of a script state (`plugin-runtime.md` §9.1, §9.4, §10.1).

use crate::data::ParseLimits;

/// Default memory cap per state: 512 MiB (`plugin-runtime.md` §9.1 and §9.4, which replace
/// the 64 MiB named earlier by `lua-plugin-api.md` §3.3; the largest measured apartment of
/// the M5 plugin set peaks at 160.2 MiB).
pub const DEFAULT_MEMORY_BYTES: usize = 512 * 1024 * 1024;

/// Default instruction budget per host call (`plugin-runtime.md` §10.1).
pub const DEFAULT_INSTRUCTIONS_PER_CALL: u64 = 500_000_000;

/// Resource limits applied to a script state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Memory cap of the state, in bytes (0 = unlimited). Counts the bytes the Lua state
    /// requests from its allocator (block sizes, new minus old size on a resize), checked
    /// against the peak including garbage not yet collected (`plugin-runtime.md` §9.1).
    pub memory_bytes: usize,
    /// Approximate VM instruction budget for one host-to-Lua call. Pattern-matching work
    /// is charged to the same budget. Once spent, the call is aborted: `pcall`, `xpcall`
    /// and `coroutine.resume` cannot catch the budget error. `None` disables the hook.
    pub instructions_per_call: Option<u64>,
    /// Largest Lua source text compiled from a file or a `load` reader, in bytes.
    pub max_source_bytes: u64,
    /// Bounds for the no-execution data reader and for data written by hosts.
    pub data: ParseLimits,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: DEFAULT_MEMORY_BYTES,
            instructions_per_call: Some(DEFAULT_INSTRUCTIONS_PER_CALL),
            max_source_bytes: 64 * 1024 * 1024,
            data: ParseLimits::default(),
        }
    }
}
