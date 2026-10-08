// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the plugin host.

use crate::descriptor::DescriptorError;
use crate::plugindata::PluginDataError;

/// Errors returned by [`PluginManager`](crate::PluginManager) operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// A descriptor could not be parsed.
    #[error("plugin descriptor: {0}")]
    Descriptor(#[from] DescriptorError),
    /// No available plugin has this name.
    #[error("no available plugin named {0:?}")]
    UnknownPlugin(String),
    /// The plugin is loaded already (§6.2).
    #[error("plugin {0:?} is already loaded")]
    AlreadyLoaded(String),
    /// No apartment has this name.
    #[error("no script state named {0:?}")]
    UnknownApartment(String),
    /// The apartment's own code is running, so it cannot be unloaded now (§6.3 step 2).
    #[error("script state {0:?} is running")]
    ApartmentRunning(String),
    /// The plugin's main package raised an error while loading (§6.2 step 5).
    #[error("plugin {plugin:?} failed to load: {message}")]
    LoadFailed {
        /// Plugin name.
        plugin: String,
        /// Lua error message.
        message: String,
    },
    /// The Lua runtime failed (e.g. state creation, out of memory).
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    /// The Lua core failed.
    #[error("{0}")]
    Core(#[from] tessera_script::CoreError),
    /// Plugin-data error.
    #[error("plugin data: {0}")]
    PluginData(#[from] PluginDataError),
}

/// The earlier name of [`PluginError`], kept for M5 callers; use [`PluginError`].
pub type ScriptError = PluginError;
