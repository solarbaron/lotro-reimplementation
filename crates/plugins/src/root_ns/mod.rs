// SPDX-License-Identifier: GPL-3.0-or-later
//! The core `Turbine` namespace: `Object`, `Plugin`, `Chat`, `PluginData`,
//! `PluginManager`, `Shell`, `ShellCommand`, `Engine` and the `ChatType`, `DataScope`,
//! `Language` enumerations.
//!
//! The identifiers are the interoperability names that existing plugins call
//! (`docs/LEGAL.md` §17); they appear only inside the Lua API surface.
//!
//! Spec: `lua-plugin-api.md` §7.4, §8, §10, §11, §12, §16, §17.1, Appendix A.1.

mod engine;
mod plugin_data;
mod plugin_manager;
mod shell;

use std::sync::Arc;

use mlua::{Function, Lua, Table};
use tessera_script::{api_fn, host_error};

use crate::object::{Builder, NativeKind, PluginRow, native_of};
use crate::shared::ApartmentCtx;

pub(crate) use plugin_data::{collect_write_results, run_write_callbacks};
pub(crate) use plugin_manager::{UnloadRefused, unload_cell, unload_now};
pub use shell::split_command_line;
pub(crate) use shell::{execute_shell_command, truncate_line};

/// Tables and functions the host needs after installation.
#[derive(Debug, Clone)]
pub(crate) struct Handles {
    pub plugin_class: Table,
    pub chat: Table,
    pub ns_root: Table,
    pub isa: Function,
    pub instantiate: Function,
}

impl Handles {
    pub(crate) fn builder<'a>(&self, lua: &'a Lua) -> Builder<'a> {
        Builder {
            lua,
            isa: self.isa.clone(),
            instantiate: self.instantiate.clone(),
        }
    }
}

/// `Turbine.ChatType` (Appendix A.1).
pub(crate) const CHAT_TYPES: &[(&str, u32)] = &[
    ("Undef", 0),
    ("Error", 1),
    ("Admin", 3),
    ("Standard", 4),
    ("Say", 5),
    ("Tell", 6),
    ("Emote", 7),
    ("Fellowship", 11),
    ("Kinship", 12),
    ("Officer", 13),
    ("Advancement", 14),
    ("Trade", 15),
    ("LFF", 16),
    ("OOC", 18),
    ("Regional", 19),
    ("Death", 20),
    ("Quest", 21),
    ("Raid", 23),
    ("Unfiltered", 25),
    ("Roleplay", 27),
    ("UserChat1", 28),
    ("UserChat2", 29),
    ("UserChat3", 30),
    ("UserChat4", 31),
    ("Tribe", 32),
    ("PlayerCombat", 34),
    ("EnemyCombat", 35),
    ("SelfLoot", 36),
    ("FellowLoot", 37),
    ("World", 38),
    ("UserChat5", 39),
    ("UserChat6", 40),
    ("UserChat7", 41),
    ("UserChat8", 42),
];

const DATA_SCOPES: &[(&str, u32)] = &[("Account", 1), ("Server", 2), ("Character", 3)];

const LANGUAGES: &[(&str, u32)] = &[
    ("Invalid", 0),
    ("English", 2),
    ("EnglishGB", 0x1000_0001),
    ("French", 0x1000_0003),
    ("German", 0x1000_0004),
];

/// Registry name of the apartment's own `tostring` (captured at creation).
const TOSTRING_KEY: &str = "tessera.plugins.tostring";

/// Install the native packages into a fresh apartment (§7.4: all at creation).
pub(crate) fn install(lua: &Lua, ctx: &Arc<ApartmentCtx>) -> mlua::Result<Handles> {
    let g = lua.globals();
    lua.set_named_registry_value(TOSTRING_KEY, g.raw_get::<Function>("tostring")?)?;
    let ns_root = lua.create_table()?;
    g.raw_set("Turbine", ns_root.clone())?;
    // Sub-namespaces exist so `import` and lookups work; their classes come from the UI
    // layer (`Turbine.UI*`) and the gameplay views.
    let gameplay = lua.create_table()?;
    gameplay.raw_set("Attributes", lua.create_table()?)?;
    ns_root.raw_set("Gameplay", gameplay)?;
    let ui = lua.create_table()?;
    ui.raw_set("Lotro", lua.create_table()?)?;
    ns_root.raw_set("UI", ui)?;

    let b = Builder::new(lua)?;
    let object = b.class(
        &ns_root,
        "Object",
        None,
        Some(Arc::new(|| NativeKind::Object)),
    )?;
    let plugin_class = install_plugin(&b, &ns_root, &object)?;
    shell::install(&b, &ns_root, &object, ctx)?;
    engine::install(&b, &ns_root, ctx)?;
    plugin_data::install(&b, &ns_root, ctx)?;
    plugin_manager::install(&b, &ns_root, ctx)?;
    let chat = b.class(&ns_root, "Chat", Some(&object), None)?;

    b.enumeration(&ns_root, "ChatType", CHAT_TYPES)?;
    b.enumeration(&ns_root, "DataScope", DATA_SCOPES)?;
    b.enumeration(&ns_root, "Language", LANGUAGES)?;

    Ok(Handles {
        plugin_class,
        chat,
        ns_root,
        isa: b.isa,
        instantiate: b.instantiate,
    })
}

fn plugin_row(lua: &Lua, this: &Table, method: &str) -> mlua::Result<PluginRow> {
    match native_of(lua, this)? {
        Some(NativeKind::Plugin(row)) => Ok(*row),
        _ => Err(host_error(format!(
            "{method}: object is not a Turbine.Plugin"
        ))),
    }
}

fn install_plugin(b: &Builder<'_>, ns_root: &Table, object: &Table) -> mlua::Result<Table> {
    let lua = b.lua;
    let plugin = b.class(ns_root, "Plugin", Some(object), None)?;
    plugin.raw_set(
        "GetName",
        api_fn(lua, "GetName", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetName")?.name)
        })?,
    )?;
    plugin.raw_set(
        "GetAuthor",
        api_fn(lua, "GetAuthor", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetAuthor")?.author)
        })?,
    )?;
    plugin.raw_set(
        "GetVersion",
        api_fn(lua, "GetVersion", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetVersion")?.version)
        })?,
    )?;
    plugin.raw_set(
        "GetUsesGlobalScaling",
        api_fn(lua, "GetUsesGlobalScaling", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetUsesGlobalScaling")?.uses_scaling)
        })?,
    )?;
    Ok(plugin)
}

/// `tostring` through the apartment's own (compatible) function.
pub(crate) fn tostring(lua: &Lua, v: mlua::Value) -> mlua::Result<String> {
    let f: Function = lua.named_registry_value(TOSTRING_KEY)?;
    Ok(f.call::<mlua::LuaString>(v)?.to_string_lossy())
}
