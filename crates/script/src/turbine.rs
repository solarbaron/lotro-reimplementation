// SPDX-License-Identifier: GPL-3.0-or-later
//! The core native namespace for Milestone 1: object model helpers, `Object`, `Plugin`,
//! `PluginData`, `PluginManager`, `Shell`, `ShellCommand`, `Engine`, `Chat` and the
//! `ChatType`, `DataScope`, `Language` enumerations.
//!
//! The identifiers below are the interoperability names that existing plugins call
//! (spec naming note); they appear only inside the Lua API surface.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §7.4, §8, §10, §11, §12, §16, §17.1, Appendix A.1.

use std::sync::Arc;

use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

use crate::host::DataScope;
use crate::plugindata::{self, PluginDataError};
use crate::runtime::{
    ApartmentCtx, ChatLine, NativeKind, PluginRow, Request, Shared, bind_native, data_to_lua,
    host_error, lock, lua_to_data, native_class_of, native_fn, native_of,
};

/// `Turbine.ChatType.Standard` (Appendix A.1).
pub const CHAT_TYPE_STANDARD: u32 = 4;

/// Longest `IsA` chain walked before giving up (guards against `__index` cycles).
const MAX_CHAIN: usize = 256;

/// Longest `Shell.WriteLine` text kept, in bytes; longer text is cut at a character
/// boundary (a chat line, not a bulk transfer channel).
pub(crate) const MAX_LINE_BYTES: usize = 4096;

/// Most `WriteLine` echoes waiting for the next pump; further echoes are dropped.
pub(crate) const MAX_CHAT_QUEUE: usize = 256;

/// Most script requests (`LoadPlugin`, ...) waiting for the next pump.
pub(crate) const MAX_REQUESTS: usize = 64;

/// Longest plugin or script-state name accepted by `PluginManager` requests.
const MAX_REQUEST_NAME: usize = 256;

/// Tables and functions the host needs after installation.
#[derive(Debug, Clone)]
pub(crate) struct Handles {
    pub plugin_class: Table,
    pub chat: Table,
    pub turbine: Table,
    pub isa: Function,
    pub instantiate: Function,
}

/// Creates the native object bound by a class `Constructor`.
pub(crate) type KindFactory = Arc<dyn Fn() -> NativeKind + Send + Sync>;

const CHAT_TYPES: &[(&str, u32)] = &[
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

/// Does `start`'s `__index` chain (starting at `start` itself) reach `target`?
fn chain_contains(start: &Table, target: &Table) -> bool {
    let mut cur = start.clone();
    for _ in 0..MAX_CHAIN {
        if cur.to_pointer() == target.to_pointer() {
            return true;
        }
        match cur.metatable().map(|mt| mt.raw_get::<Value>("__index")) {
            Some(Ok(Value::Table(next))) => cur = next,
            _ => return false,
        }
    }
    false
}

/// `X:IsA(C)` for classes, enums and instances, native or Lua (§8.1, §8.2).
fn is_a(lua: &Lua, (this, class): (Value, Value)) -> mlua::Result<bool> {
    let (Value::Table(this), Value::Table(class)) = (this, class) else {
        return Ok(false);
    };
    if chain_contains(&this, &class) {
        return Ok(true);
    }
    Ok(native_class_of(lua, &this)?.is_some_and(|nc| chain_contains(&nc, &class)))
}

/// A function that ignores a leading argument equal to `owner` (colon-called statics,
/// §8.1), then converts the remaining arguments to `A`.
pub(crate) fn static_fn<A, R, F>(
    lua: &Lua,
    owner: &Table,
    name: &str,
    f: F,
) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
{
    let owner_ptr = owner.to_pointer() as usize;
    let fname = name.to_owned();
    native_fn(lua, name, move |lua, mut args: MultiValue| {
        if let Some(Value::Table(t)) = args.front()
            && t.to_pointer() as usize == owner_ptr
        {
            args.pop_front();
        }
        f(lua, A::from_lua_args(args, 1, Some(&fname), lua)?)
    })
}

pub(crate) struct Builder<'a> {
    pub lua: &'a Lua,
    pub isa: Function,
    pub instantiate: Function,
}

impl Builder<'_> {
    /// A class table (§8.1): `IsA`, metatable `__index` = base; instantiable classes also get
    /// a native `Constructor` binding `kind` and a `__call` that creates instances.
    pub(crate) fn class(
        &self,
        ns: &Table,
        name: &str,
        base: Option<&Table>,
        kind: Option<KindFactory>,
    ) -> mlua::Result<Table> {
        let cls = self.lua.create_table()?;
        cls.raw_set("IsA", self.isa.clone())?;
        let mt = self.lua.create_table()?;
        if let Some(base) = base {
            mt.raw_set("__index", base.clone())?;
        }
        if let Some(make) = kind {
            let cls_ref = cls.clone();
            let class_name = name.to_owned();
            let ctor = native_fn(self.lua, "Constructor", move |lua, args: MultiValue| {
                let Some(Value::Table(this)) = args.front() else {
                    return Err(host_error(format!(
                        "{class_name}.Constructor: expected an object table as first argument"
                    )));
                };
                bind_native(lua, this, make(), cls_ref.clone())
            })?;
            cls.raw_set("Constructor", ctor)?;
            mt.raw_set("__call", self.instantiate.clone())?;
        }
        cls.set_metatable(Some(mt))?;
        ns.raw_set(name, cls.clone())?;
        Ok(cls)
    }

    /// A static function table (`Engine`, `Shell`, ...) with `IsA`.
    pub(crate) fn statics(&self, ns: &Table, name: &str) -> mlua::Result<Table> {
        let t = self.lua.create_table()?;
        t.raw_set("IsA", self.isa.clone())?;
        ns.raw_set(name, t.clone())?;
        Ok(t)
    }

    /// An enumeration table (§8.4).
    pub(crate) fn enumeration(
        &self,
        ns: &Table,
        name: &str,
        values: &[(&str, u32)],
    ) -> mlua::Result<()> {
        let t = self.lua.create_table()?;
        for (k, v) in values {
            t.raw_set(*k, f64::from(*v))?;
        }
        t.raw_set("IsA", self.isa.clone())?;
        ns.raw_set(name, t)
    }
}

/// Install the native packages into a fresh apartment (§7.4: all at creation).
pub(crate) fn install(lua: &Lua, ctx: &Arc<ApartmentCtx>) -> mlua::Result<Handles> {
    let g = lua.globals();
    let turbine = lua.create_table()?;
    g.raw_set("Turbine", turbine.clone())?;
    // Sub-namespaces exist (empty for now) so `import` and lookups work; their classes are
    // provided by the UI and gameplay layers.
    let gameplay = lua.create_table()?;
    gameplay.raw_set("Attributes", lua.create_table()?)?;
    turbine.raw_set("Gameplay", gameplay)?;
    let ui = lua.create_table()?;
    ui.raw_set("Lotro", lua.create_table()?)?;
    turbine.raw_set("UI", ui)?;

    let instantiate = native_fn(lua, "__call", |lua, mut args: MultiValue| {
        let Some(Value::Table(cls)) = args.pop_front() else {
            return Err(host_error("class call without a class table"));
        };
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", cls.clone())?;
        obj.set_metatable(Some(mt))?;
        if let Value::Function(ctor) = cls.get::<Value>("Constructor")? {
            args.push_front(Value::Table(obj.clone()));
            ctor.call::<()>(args)?;
        }
        Ok(obj)
    })?;
    let b = Builder {
        lua,
        isa: native_fn(lua, "IsA", is_a)?,
        instantiate,
    };

    let object = b.class(
        &turbine,
        "Object",
        None,
        Some(Arc::new(|| NativeKind::Object)),
    )?;
    let plugin_class = install_plugin(&b, &turbine, &object)?;
    install_shell(&b, &turbine, &object, ctx)?;
    install_engine(&b, &turbine, ctx)?;
    install_plugin_data(&b, &turbine, ctx)?;
    install_plugin_manager(&b, &turbine, ctx)?;
    let chat = b.class(&turbine, "Chat", Some(&object), None)?;

    b.enumeration(&turbine, "ChatType", CHAT_TYPES)?;
    b.enumeration(&turbine, "DataScope", DATA_SCOPES)?;
    b.enumeration(&turbine, "Language", LANGUAGES)?;

    Ok(Handles {
        plugin_class,
        chat,
        turbine,
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

fn install_plugin(b: &Builder<'_>, turbine: &Table, object: &Table) -> mlua::Result<Table> {
    let lua = b.lua;
    let plugin = b.class(turbine, "Plugin", Some(object), None)?;
    plugin.raw_set(
        "GetName",
        native_fn(lua, "GetName", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetName")?.name)
        })?,
    )?;
    plugin.raw_set(
        "GetAuthor",
        native_fn(lua, "GetAuthor", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetAuthor")?.author)
        })?,
    )?;
    plugin.raw_set(
        "GetVersion",
        native_fn(lua, "GetVersion", |lua, this: Table| {
            Ok(plugin_row(lua, &this, "GetVersion")?.version)
        })?,
    )?;
    Ok(plugin)
}

fn tostring(lua: &Lua, v: Value) -> mlua::Result<String> {
    let f: Function = lua.globals().raw_get("tostring")?;
    Ok(f.call::<mlua::LuaString>(v)?.to_string_lossy())
}

fn normalize_command(name: &str) -> String {
    name.trim().trim_start_matches('/').to_lowercase()
}

/// Built-in command names the host always owns.
pub(crate) const HOST_COMMANDS: [&str; 1] = ["plugins"];

fn is_taken(ctx: &ApartmentCtx, shared: &Shared, name: &str) -> bool {
    HOST_COMMANDS.contains(&name)
        || shared.commands.contains_key(name)
        || ctx.host.is_builtin_command(name)
}

fn install_shell(
    b: &Builder<'_>,
    turbine: &Table,
    object: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let cmd = b.class(
        turbine,
        "ShellCommand",
        Some(object),
        Some(Arc::new(|| NativeKind::ShellCommand)),
    )?;
    cmd.raw_set(
        "Execute",
        native_fn(lua, "Execute", |_, _: MultiValue| Ok(()))?,
    )?;
    cmd.raw_set(
        "GetHelp",
        native_fn(lua, "GetHelp", |_, _: MultiValue| Ok(Value::Nil))?,
    )?;
    cmd.raw_set(
        "GetShortHelp",
        native_fn(lua, "GetShortHelp", |_, _: MultiValue| Ok(Value::Nil))?,
    )?;

    let shell = b.statics(turbine, "Shell")?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "WriteLine",
        static_fn(lua, &shell, "WriteLine", move |lua, text: Value| {
            let mut text = tostring(lua, text)?;
            truncate_line(&mut text);
            c.host.write_line(&text);
            let mut shared = lock(&c.shared);
            if shared.in_chat_dispatch {
                // Written by a `Chat.Received` handler: shown, not echoed again (§11.1).
            } else if shared.chat.len() >= MAX_CHAT_QUEUE {
                shared.dropped_chat += 1;
            } else {
                shared.chat.push_back(ChatLine {
                    sender: String::new(),
                    message: text,
                    chat_type: CHAT_TYPE_STANDARD,
                });
            }
            Ok(())
        })?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "AddCommand",
        static_fn(
            lua,
            &shell,
            "AddCommand",
            move |lua, (names, cmd): (String, Table)| {
                let mut added = 0u32;
                for raw in names.split(';') {
                    let name = normalize_command(raw);
                    if name.is_empty() || name.contains(char::is_whitespace) {
                        continue;
                    }
                    let key = lua.create_registry_value(cmd.clone())?;
                    let mut shared = lock(&c.shared);
                    if is_taken(&c, &shared, &name) {
                        continue;
                    }
                    shared.commands.insert(name.clone(), c.key.clone());
                    lock(&c.inner).commands.insert(name, key);
                    added += 1;
                }
                Ok(added)
            },
        )?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "RemoveCommand",
        static_fn(lua, &shell, "RemoveCommand", move |lua, cmd: Table| {
            let names: Vec<String> = {
                let inner = lock(&c.inner);
                inner
                    .commands
                    .iter()
                    .filter(|(_, k)| {
                        lua.registry_value::<Table>(k)
                            .is_ok_and(|t| t.to_pointer() == cmd.to_pointer())
                    })
                    .map(|(n, _)| n.clone())
                    .collect()
            };
            let mut shared = lock(&c.shared);
            let mut inner = lock(&c.inner);
            for n in &names {
                shared.commands.remove(n);
                if let Some(k) = inner.commands.remove(n) {
                    let _ = lua.remove_registry_value(k);
                }
            }
            Ok(names.len() as u32)
        })?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "IsCommand",
        static_fn(lua, &shell, "IsCommand", move |_, name: String| {
            let name = normalize_command(&name);
            let shared = lock(&c.shared);
            Ok(!name.is_empty() && is_taken(&c, &shared, &name))
        })?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "GetCommands",
        static_fn(lua, &shell, "GetCommands", move |lua, ()| {
            let mut names: Vec<String> = c.host.builtin_commands();
            names.extend(HOST_COMMANDS.iter().map(|s| (*s).to_owned()));
            names.extend(lock(&c.shared).commands.keys().cloned());
            names.sort();
            names.dedup();
            lua.create_sequence_from(names)
        })?,
    )?;
    Ok(())
}

fn install_engine(b: &Builder<'_>, turbine: &Table, ctx: &Arc<ApartmentCtx>) -> mlua::Result<()> {
    let lua = b.lua;
    let engine = b.statics(turbine, "Engine")?;

    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetDate",
        static_fn(lua, &engine, "GetDate", move |lua, ()| {
            let d = c.host.engine_date();
            let t = lua.create_table()?;
            t.raw_set("Year", d.year)?;
            t.raw_set("Month", d.month)?;
            t.raw_set("Day", d.day)?;
            t.raw_set("Hour", d.hour)?;
            t.raw_set("Minute", d.minute)?;
            t.raw_set("Second", d.second)?;
            t.raw_set("DayOfWeek", d.day_of_week)?;
            t.raw_set("DayOfYear", d.day_of_year)?;
            t.raw_set("IsDST", d.is_dst)?;
            Ok(t)
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetGameTime",
        static_fn(lua, &engine, "GetGameTime", move |_, ()| {
            Ok(c.host.game_time())
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLocalTime",
        static_fn(lua, &engine, "GetLocalTime", move |_, ()| {
            Ok(c.host.local_time())
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLanguage",
        static_fn(lua, &engine, "GetLanguage", move |_, ()| {
            Ok(f64::from(c.host.language()))
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLocale",
        static_fn(lua, &engine, "GetLocale", move |_, ()| Ok(c.host.locale()))?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetScriptVersion",
        static_fn(lua, &engine, "GetScriptVersion", move |_, ()| {
            Ok(c.host.script_version())
        })?,
    )?;
    engine.raw_set(
        "GetCallStack",
        static_fn(
            lua,
            &engine,
            "GetCallStack",
            |lua, (msg, level): (Option<String>, Option<usize>)| {
                lua.traceback(msg.as_deref(), level.unwrap_or(1))
            },
        )?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "ScriptLog",
        static_fn(lua, &engine, "ScriptLog", move |lua, msg: Value| {
            c.host.script_log(&c.name, &tostring(lua, msg)?);
            Ok(())
        })?,
    )?;
    Ok(())
}

fn is_callable(v: &Value) -> bool {
    match v {
        Value::Function(_) => true,
        Value::Table(t) => t
            .metatable()
            .and_then(|mt| mt.raw_get::<Value>("__call").ok())
            .is_some_and(|c| !c.is_nil()),
        _ => false,
    }
}

/// Validate `(scope, key)` and find the folder (§10.2); errors are raised in Lua.
fn data_target(
    ctx: &ApartmentCtx,
    func: &str,
    scope: &Value,
    key: &Value,
) -> mlua::Result<(std::path::PathBuf, String)> {
    let raise = |e: PluginDataError| host_error(format!("PluginData.{func}: {e}"));
    let scope = match scope {
        Value::Integer(i) => DataScope::from_number(*i as f64),
        Value::Number(n) => DataScope::from_number(*n),
        _ => None,
    }
    .ok_or_else(|| raise(PluginDataError::InvalidScope))?;
    let Value::String(key) = key else {
        return Err(raise(PluginDataError::InvalidKey("not a string")));
    };
    let key = plugindata::validate_key(&key.as_bytes())
        .map_err(raise)?
        .to_owned();
    let dir = ctx
        .host
        .plugin_data_dir(scope)
        .ok_or_else(|| raise(PluginDataError::ScopeUnavailable))?;
    Ok((dir, key))
}

fn install_plugin_data(
    b: &Builder<'_>,
    turbine: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let pd = b.statics(turbine, "PluginData")?;

    let c = Arc::clone(ctx);
    pd.raw_set(
        "Save",
        static_fn(
            lua,
            &pd,
            "Save",
            move |lua, (scope, key, data, on_done): (Value, Value, Value, Value)| {
                let (dir, key) = data_target(&c, "Save", &scope, &key)?;
                let callback = match on_done {
                    Value::Function(f) => Some(f),
                    v if is_callable(&v) => {
                        let wrap: Function = lua
                            .load("local h = ...; return function(...) return h(...) end")
                            .call(v)?;
                        Some(wrap)
                    }
                    Value::Nil => None,
                    other => {
                        tracing::debug!(
                            apartment = %c.name,
                            "PluginData.Save: ignoring non-callable completion argument of type {}",
                            other.type_name()
                        );
                        None
                    }
                };
                if callback.is_some() && !c.can_defer() {
                    return Err(host_error(
                        "PluginData.Save: too many pending plugin data callbacks",
                    ));
                }
                let limits = &c.limits.plugin_data;
                let result = lua_to_data(lua, &data, limits)
                    .and_then(|v| plugindata::save_file(&dir, &key, &v, limits));
                match (result, callback) {
                    (Ok(()), Some(cb)) => {
                        c.defer(cb, vec![Value::Boolean(true)], "plugin data save callback")
                    }
                    (Ok(()), None) => {}
                    (Err(e), Some(cb)) => {
                        let msg = Value::String(lua.create_string(e.to_string())?);
                        c.defer(
                            cb,
                            vec![Value::Boolean(false), msg],
                            "plugin data save callback",
                        );
                    }
                    (Err(e), None) => {
                        return Err(host_error(format!("PluginData.Save: {e}")));
                    }
                }
                Ok(())
            },
        )?,
    )?;

    let c = Arc::clone(ctx);
    pd.raw_set(
        "Load",
        static_fn(
            lua,
            &pd,
            "Load",
            move |lua, (scope, key, on_loaded): (Value, Value, Value)| {
                let (dir, key) = data_target(&c, "Load", &scope, &key)?;
                if !on_loaded.is_nil() && !is_callable(&on_loaded) {
                    return Err(host_error(
                        "PluginData.Load: load handler must be a valid function",
                    ));
                }
                if !on_loaded.is_nil() && !c.can_defer() {
                    return Err(host_error(
                        "PluginData.Load: too many pending plugin data callbacks",
                    ));
                }
                let value = match plugindata::load_file(&dir, &key, &c.limits.plugin_data) {
                    Ok(Some(v)) => data_to_lua(lua, &v)?,
                    Ok(None) => Value::Nil,
                    Err(e) => {
                        tracing::warn!(apartment = %c.name, key, "plugin data unreadable: {e}");
                        c.host
                            .script_log(&c.name, &format!("PluginData.Load({key}): {e}"));
                        Value::Nil
                    }
                };
                match on_loaded {
                    Value::Function(f) => {
                        c.defer(f, vec![value.clone()], "plugin data load callback")
                    }
                    Value::Nil => {}
                    v => {
                        let wrap: Function = lua
                            .load("local h = ...; return function(...) return h(...) end")
                            .call(v)?;
                        c.defer(wrap, vec![value.clone()], "plugin data load callback");
                    }
                }
                Ok(value)
            },
        )?,
    )?;
    Ok(())
}

fn plugin_rows_table(lua: &Lua, rows: &[PluginRow], with_apartment: bool) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    for (i, r) in rows.iter().enumerate() {
        let t = lua.create_table()?;
        t.raw_set("Name", r.name.as_str())?;
        t.raw_set("Author", r.author.as_str())?;
        t.raw_set("Version", r.version.as_str())?;
        if let Some(d) = &r.description {
            t.raw_set("Description", d.as_str())?;
        }
        if with_apartment {
            t.raw_set("ScriptState", r.apartment.as_str())?;
        }
        out.raw_set(i + 1, t)?;
    }
    Ok(out)
}

fn install_plugin_manager(
    b: &Builder<'_>,
    turbine: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let pm = b.statics(turbine, "PluginManager")?;

    let c = Arc::clone(ctx);
    pm.raw_set(
        "GetAvailablePlugins",
        static_fn(lua, &pm, "GetAvailablePlugins", move |lua, ()| {
            let rows = lock(&c.shared).available.clone();
            plugin_rows_table(lua, &rows, false)
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "GetLoadedPlugins",
        static_fn(lua, &pm, "GetLoadedPlugins", move |lua, ()| {
            let rows = lock(&c.shared).loaded.clone();
            plugin_rows_table(lua, &rows, true)
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "LoadPlugin",
        static_fn(lua, &pm, "LoadPlugin", move |_, name: String| {
            push_request(&c, Request::Load { name });
            Ok(())
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "UnloadScriptState",
        static_fn(lua, &pm, "UnloadScriptState", move |_, name: String| {
            push_request(
                &c,
                Request::Unload {
                    apartment: name,
                    from: c.key.clone(),
                },
            );
            Ok(())
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "RefreshAvailablePlugins",
        static_fn(lua, &pm, "RefreshAvailablePlugins", move |_, ()| {
            push_request(&c, Request::Refresh);
            Ok(())
        })?,
    )?;
    pm.raw_set(
        "ShowOptions",
        static_fn(lua, &pm, "ShowOptions", |_, _: MultiValue| Ok(()))?,
    )?;
    Ok(())
}

/// Cut `text` to at most [`MAX_LINE_BYTES`] bytes at a character boundary.
fn truncate_line(text: &mut String) {
    if text.len() > MAX_LINE_BYTES {
        let mut end = MAX_LINE_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}

/// Queue a script request for the next pump: duplicates collapse (one refresh, one load
/// per plugin name, one unload per state), oversized names are ignored and the queue is
/// capped, so a script loop cannot make the pump do unbounded work.
fn push_request(ctx: &ApartmentCtx, req: Request) {
    let name_len = match &req {
        Request::Load { name } => name.len(),
        Request::Unload { apartment, .. } => apartment.len(),
        Request::Refresh => 0,
    };
    if name_len > MAX_REQUEST_NAME {
        tracing::debug!(apartment = %ctx.name, "ignoring plugin manager request with an oversized name");
        return;
    }
    let mut shared = lock(&ctx.shared);
    let duplicate = shared.requests.iter().any(|r| match (r, &req) {
        (Request::Refresh, Request::Refresh) => true,
        (Request::Load { name: a }, Request::Load { name: b }) => a.eq_ignore_ascii_case(b),
        (
            Request::Unload {
                apartment: a,
                from: fa,
            },
            Request::Unload {
                apartment: b,
                from: fb,
            },
        ) => a.eq_ignore_ascii_case(b) && fa == fb,
        _ => false,
    });
    if duplicate {
        return;
    }
    if shared.requests.len() >= MAX_REQUESTS {
        shared.dropped_requests += 1;
        return;
    }
    shared.requests.push_back(req);
}
