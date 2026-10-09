// SPDX-License-Identifier: GPL-3.0-or-later
//! The small host trait of the core: script log and error reports.
//!
//! Spec: `plugin-runtime.md` §11 (errors), `lua-plugin-api.md` §3.2 (`print`).

/// What kind of failure a report describes (`plugin-runtime.md` §11.1, §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// A run-time or syntax error raised by Lua code or a host function.
    Runtime,
    /// The state's memory cap refused an allocation (an out-of-memory report).
    Memory,
    /// The call ran out of its instruction budget.
    Budget,
}

/// A caught Lua error, as reported to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorReport {
    /// The unit the host attributes the error to, when it knows one (opaque to the core).
    pub source: Option<String>,
    /// Name of the script state.
    pub state: String,
    /// What the host was doing, e.g. `"loading"`, `"handling event Load"`.
    pub context: String,
    /// The Lua error message as Lua produced it (for a non-string error value: its text,
    /// or empty, `plugin-runtime.md` §11.1).
    pub message: String,
    /// Lua traceback when available (for the log only).
    pub traceback: Option<String>,
    /// Classification.
    pub kind: ErrorKind,
}

/// Services every embedding of the core provides.
///
/// Methods are called on the thread that drives the state; implementations must not call
/// back into it.
pub trait CoreHost: Send + Sync + 'static {
    /// Text from `print` and other script-log writes of state `state`.
    fn script_log(&self, state: &str, message: &str) {
        tracing::debug!(target: "tessera_script::script_log", state, "{message}");
    }

    /// A Lua error the core caught itself (a finalizer error during a collector step, for
    /// example). Default: a `tracing` warning with the traceback.
    fn report_error(&self, report: &ErrorReport) {
        tracing::warn!(
            target: "tessera_script::error",
            source = report.source.as_deref().unwrap_or("-"),
            state = %report.state,
            context = %report.context,
            traceback = report.traceback.as_deref().unwrap_or(""),
            "{}",
            report.message
        );
    }
}

/// A [`CoreHost`] that only logs through `tracing`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LogHost;

impl CoreHost for LogHost {}
