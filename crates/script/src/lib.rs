// SPDX-License-Identifier: GPL-3.0-or-later
//! Embedded Lua 5.1 plugin host.
//!
//! Runs user-interface plugins that target the plugin API of the original client, from a
//! read-only `Plugins/` folder, inside sandboxed Lua states ("apartments").
//! Implemented from `docs/specs/lua-plugin-api.md` only.
//!
//! - [`descriptor`]: `.plugin` XML descriptors (§5).
//! - [`PluginManager`]: discovery (§4.3), apartments and lifecycle (§6), `import` with
//!   per-package environments (§7), object model (§8), events (§9), `/plugins` and plugin
//!   chat commands (§6.4, §11), deferred work.
//! - [`HostApi`]: what the embedding client provides (chat output, time, plugin-data
//!   folders, error reporting).
//! - [`plugindata`]: the `.plugindata` value format and its no-execution reader (§10).
//!
//! Milestone-1 native surface: `Turbine.Object`, `Plugin`, `PluginData`, `PluginManager`,
//! `Shell`, `ShellCommand`, `Engine`, `Chat` and the `ChatType`, `DataScope`, `Language`
//! enumerations. The `Turbine.UI*` and `Turbine.Gameplay*` namespaces exist as empty tables
//! for other layers to fill from [`HostApi::install_natives`] (see [`api`]), which runs in
//! every new apartment before plugin code.
//!
//! All plugin code runs on the thread that owns the [`PluginManager`]; the manager is
//! `Send` so it can be moved to that thread.

pub mod api;
pub mod descriptor;
mod error;
mod host;
mod manager;
pub mod paths;
mod pattern;
pub mod plugindata;
mod runtime;
mod turbine;

pub use descriptor::{DescriptorError, PluginDescriptor, parse_descriptor};
pub use error::ScriptError;
pub use host::{
    DataScope, EngineDate, HostApi, LANGUAGE_ENGLISH, PluginDataLocation, ScriptErrorReport,
    sanitize_folder_name,
};
pub use manager::{
    AvailablePlugin, DEFAULT_APARTMENT, LoadedPluginInfo, ManagerConfig, PluginManager,
    RefreshReport,
};
pub use mlua;
pub use plugindata::{DataKey, DataValue, ParseLimits, PluginDataError};
pub use runtime::Limits;
pub use turbine::CHAT_TYPE_STANDARD;

#[cfg(test)]
mod send_check {
    fn assert_send<T: Send>() {}
    #[test]
    fn manager_is_send() {
        assert_send::<super::PluginManager>();
    }
}
