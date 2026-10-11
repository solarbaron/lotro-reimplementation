// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-plugins`: the plugin host (`docs/ARCHITECTURE.md` §18.2, decision 0023).
//!
//! Runs user-interface plugins written for the original client's plugin API, from a
//! read-only `Plugins/` folder, in sandboxed Lua states ("apartments") built on the
//! plugin-free core `tessera-script`.
//!
//! - [`descriptor`], [`discovery`]: `.plugin` descriptors and the available list
//!   (`plugin-descriptor.md` §4–§5); [`options`]: the read-only `PluginOptions.xml`
//!   importer (§10).
//! - [`PluginManager`]: apartments and their lifecycle, `import` with package resolution
//!   (`lua-plugin-api.md` §6–§7; [`packages`] is the hook for built-in packages), the
//!   object model and events (§8–§9), `/plugins` and plugin chat commands (§6.4, §11), the
//!   step-7 deferred work and the collector step (`plugin-runtime.md` §8.4, §9.2).
//! - The core `Turbine` namespace (`Engine`, `Shell`, `Chat`, `PluginData`,
//!   `PluginManager`, …) and `Turbine.Gameplay`'s read-only local-player views fed by a
//!   [`LocalPlayerSource`] the client implements ([`gameplay`]); the inventory views
//!   (`Backpack`, `Equipment`, `Item`, `ItemInfo`) fed by an [`InventorySource`] and
//!   queued [`InventoryViewEvent`]s ([`inventory`], `lua-plugin-api.md` §15.1); the
//!   combat views (actor vitals, level and target, `EffectList`, `Effect`, `IsInCombat`,
//!   the trained `SkillList`) fed by a [`CombatSource`] and queued [`CombatViewEvent`]s,
//!   with [`CombatModel`] as a ready-made source ([`combat`], §15.2).
//! - [`HostApi`]: what the embedding client provides; it extends the core's
//!   [`CoreHost`]. Plugin shell commands live in the host's one command registry
//!   (`chat-window.md` §10 item 3); [`CommandTable`] is a registry for hosts without one.
//! - [`plugindata`]: `.plugindata` naming, keys, writer and the comma rule over the
//!   core's neutral reader.
//! - [`vfs`]: the index of the whole `Plugins/` tree and the call-time header probe of
//!   `SetBackground(path)` (decision 0015 §4.4).
//! - [`api`]: the extension API for the `Turbine.UI` layers.
//!
//! Module ownership during M5 (parallel implementation):
//!
//! - [`class_helper`]: the built-in class helper package (decision 0016).
//! - [`ui`]: the `Turbine.UI` and `Turbine.UI.Lotro` bindings over `tessera-ui` elements.
//! - the rest of the crate: everything else above.
//!
//! All plugin code runs on the thread that owns the [`PluginManager`]; the manager is
//! `Send` so it can be moved to that thread.

mod apartment;
pub mod api;
pub mod class_helper;
pub mod combat;
pub mod commands;
mod data_io;
pub mod descriptor;
pub mod discovery;
mod error;
pub mod gameplay;
mod gameplay_enums;
mod host;
mod import;
pub mod inventory;
mod manager;
mod object;
pub mod options;
pub mod packages;
pub mod paths;
pub mod plugindata;
mod root_ns;
mod shared;
pub mod ui;
pub mod vfs;

pub use combat::{
    ActorEvent, ActorKind, ActorValues, CombatModel, CombatSource, CombatViewEvent, CombatVital,
    EffectData, EffectEvent, SkillData, SkillKey, ViewEntity,
};
pub use commands::{CommandOwner, CommandTable};
pub use descriptor::{DescriptorError, PluginDescriptor, parse_descriptor};
pub use discovery::{AvailablePlugin, RefreshReport};
pub use error::{PluginError, ScriptError};
pub use gameplay::{GameplayEvent, LocalPlayerEvent, LocalPlayerInfo, LocalPlayerSource};
pub use host::{
    CHAT_TYPE_ERROR, CHAT_TYPE_STANDARD, CHAT_TYPE_WRITE_LINE, CoreHost, DataScope, EngineDate,
    HostApi, LANGUAGE_ENGLISH, PluginDataLocation, ScriptErrorReport, WorkerJob,
    sanitize_folder_name,
};
pub use inventory::{InventoryItem, InventorySource, InventoryViewEvent, ItemInfoData};
pub use manager::{
    DEFAULT_APARTMENT, DEFAULT_DEFERRED_SLICE, LoadedPluginInfo, ManagerConfig, PluginManager,
};
pub use mlua;
pub use plugindata::{DataKey, DataValue, ParseLimits, PluginDataError};
pub use root_ns::split_command_line;
pub use tessera_script::{ErrorKind, Limits};

#[cfg(test)]
mod send_check {
    fn assert_send<T: Send>() {}
    #[test]
    fn manager_is_send() {
        assert_send::<super::PluginManager>();
    }
}
