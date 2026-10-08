// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-script`: the plugin-free, sandboxed Lua 5.1 core (`docs/ARCHITECTURE.md` §18.1,
//! decision 0023).
//!
//! A [`ScriptState`] is one sandboxed Lua 5.1 state built from a [`Profile`]: which base
//! functions exist, the [`Limits`] (memory cap, per-call instruction budget, source and
//! data bounds), how host-compiled chunks are named, which random generator `math.random`
//! uses, and whether the C-runtime number and string conversions of the original client
//! apply. The core provides:
//!
//! - the library allow-list (base, `coroutine`, `table`, `string`, `math`; no `io`, `os`,
//!   `package`, `debug`, `dofile`, `loadfile`, `require`, `module`) and text-only
//!   compilation (binary chunks are refused);
//! - the per-call instruction [`Budget`] with its count hook, and the `pcall` guard that
//!   keeps protected calls from swallowing a spent budget;
//! - protected calls with error description ([`describe_error`]) and [`native_fn`] /
//!   [`api_fn`] for host functions whose errors reach Lua as plain strings;
//! - the bounded Lua pattern matcher behind `string.find`, `match`, `gmatch` and `gsub`;
//! - the metered finalizer proxy behind `newproxy(true)`;
//! - collector stepping ([`ScriptState::gc_step`]);
//! - the neutral, no-execution Lua-table data reader ([`data`]).
//!
//! The core names no plugin concept: it has no plugin API namespace, no `import`, no
//! plugins root, no shared plugin state, no command table and no chat queue. The plugin
//! host (`tessera-plugins`) builds on it. Its host trait, [`CoreHost`], covers the script
//! log and error reports only.
//!
//! Spec: `docs/specs/plugin-runtime.md` §4 (interpreter, conversions, random numbers), §5
//! (state creation), §9 (memory, collector, finalizers), §10 (limits), §11 (errors);
//! `docs/specs/lua-plugin-api.md` §3.1–§3.3 (sandbox and limits).

mod budget;
mod compat;
pub mod data;
mod error;
mod host;
mod limits;
mod native;
mod pattern;
mod sandbox;
mod state;

pub use budget::{BUDGET_MESSAGE, Budget};
pub use compat::{CrtRandom, SharedRandom, format_number_g14};
pub use error::CoreError;
pub use host::{CoreHost, ErrorKind, ErrorReport, LogHost};
pub use limits::{DEFAULT_MEMORY_BYTES, Limits};
pub use mlua;
pub use native::{
    ErrorStyle, api_fn, classify, describe_error, host_error, is_budget_error, is_memory_error,
    native_fn, native_fn_styled,
};
pub use state::{
    BaseFunctions, Conversions, GC_STEP_KBYTES, Profile, RandomSource, ScriptState, call_protected,
    compile_source,
};

#[cfg(test)]
mod send_check {
    fn assert_send<T: Send>() {}
    #[test]
    fn state_is_send() {
        assert_send::<super::ScriptState>();
    }
}
