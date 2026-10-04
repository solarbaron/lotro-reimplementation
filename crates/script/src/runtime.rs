// SPDX-License-Identifier: GPL-3.0-or-later
//! Apartments: one sandboxed Lua 5.1 state each, with `import`, package environments,
//! the native object binding and event dispatch.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §3 (runtime and sandbox), §6.1 (apartments),
//! §7 (`import`), §8 (object model), §9.2 (dispatch).

use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use mlua::chunk::ChunkMode;
use mlua::{
    FromLuaMulti, Function, HookTriggers, IntoLuaMulti, Lua, LuaOptions, MultiValue, RegistryKey,
    StdLib, Table, UserData, Value, VmState,
};

use crate::descriptor::PluginDescriptor;
use crate::host::{HostApi, ScriptErrorReport};
use crate::plugindata::{DataKey, DataValue, ParseLimits, PluginDataError};

/// Registry name of the weak-keyed table binding instance tables to native objects.
const NATIVES_KEY: &str = "tessera.natives";

/// Registry name of the factory that wraps raw native functions (see [`native_fn`]).
const SHIM_KEY: &str = "tessera.shim";

/// Lua side of [`native_fn`]: the raw function returns `true, results...` or
/// `false, message`; the wrapper turns the latter into an ordinary string error, as a
/// Lua 5.1 C function raising with `luaL_error` would (§3.1).
const SHIM_SOURCE: &str = "local error = error
local function pass(ok, ...)
  if ok then return ... end
  error((...), 0)
end
return function(f)
  return function(...) return pass(f(...)) end
end
";

/// Replacements for `pcall`, `xpcall` and `coroutine.resume` that re-raise once the
/// instruction budget is spent, so a protected call cannot swallow the budget error and
/// keep the host thread busy forever (§3.3).
///
/// The `xpcall` message handler is skipped for the budget error: Lua runs a handler for
/// an error raised from inside a hook with hooks disabled, so a looping handler would
/// otherwise run unmetered.
const GUARD_SOURCE: &str = "local pcall, xpcall, resume, exhausted, error, msg = ...
local function after(ok, ...)
  if exhausted() then error(msg, 0) end
  return ok, ...
end
return function(f, ...) return after(pcall(f, ...)) end,
  function(f, h)
    return after(xpcall(f, function(e)
      if exhausted() then return e end
      return h(e)
    end))
  end,
  function(co, ...) return after(resume(co, ...)) end
";

/// `__index` of a package environment (§7.2 step 5): the namespace first, then globals.
const ENV_INDEX_SOURCE: &str = "local ns, g = ...
return function(_, k)
  local v = ns[k]
  if v == nil then return g[k] end
  return v
end
";

/// Message of the error raised when a call runs out of its instruction budget.
pub(crate) const BUDGET_MESSAGE: &str = "script exceeded its instruction budget";

/// Most plugin-data callbacks queued per apartment before further requests are refused.
pub(crate) const MAX_DEFERRED: usize = 1024;

/// Native package names (§7.4). Importing one of these is a no-op.
pub(crate) const NATIVE_PACKAGES: [&str; 5] = [
    "Turbine",
    "Turbine.Gameplay",
    "Turbine.Gameplay.Attributes",
    "Turbine.UI",
    "Turbine.UI.Lotro",
];

/// Lua's binary chunk signature (§3.2: binary chunks are rejected).
const BYTECODE_SIGNATURE: &[u8] = b"\x1bLua";

/// Resource limits applied to every apartment (§3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Memory cap per apartment, in bytes (0 = unlimited).
    pub memory_bytes: usize,
    /// Approximate VM instruction budget for one host-to-Lua call (load, event, command,
    /// callback, each element of a handler array). Pattern-matching work is charged to the
    /// same budget. Once spent, the call is aborted: `pcall`, `xpcall` and
    /// `coroutine.resume` cannot catch the budget error. `None` disables the hook.
    pub instructions_per_call: Option<u64>,
    /// Largest Lua source file `import` accepts, in bytes.
    pub max_source_bytes: u64,
    /// Bounds for reading and writing plugin data.
    pub plugin_data: ParseLimits,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(500_000_000),
            max_source_bytes: 16 * 1024 * 1024,
            plugin_data: ParseLimits::default(),
        }
    }
}

/// How often (in VM instructions) the budget hook runs.
const HOOK_GRANULARITY: u32 = 1000;

/// A request a script made that the manager carries out after the current call returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    /// `PluginManager.LoadPlugin(name)`.
    Load { name: String },
    /// `PluginManager.UnloadScriptState(name)`, issued from apartment `from` (lower case).
    Unload { apartment: String, from: String },
    /// `PluginManager.RefreshAvailablePlugins()`.
    Refresh,
}

/// A chat line waiting to be delivered to `Turbine.Chat.Received` handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatLine {
    pub sender: String,
    pub message: String,
    pub chat_type: u32,
}

/// Information about one plugin, as scripts see it through `PluginManager`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginRow {
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: Option<String>,
    pub apartment: String,
}

/// State shared between the manager and every apartment's callbacks.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    pub available: Vec<PluginRow>,
    pub loaded: Vec<PluginRow>,
    /// Lower-case plugin command name → lower-case apartment key.
    pub commands: HashMap<String, String>,
    pub requests: VecDeque<Request>,
    pub chat: VecDeque<ChatLine>,
    /// Set while the manager delivers a line to `Chat.Received` handlers; `WriteLine` calls
    /// made then are shown but not echoed again (no feedback loops between handlers).
    pub in_chat_dispatch: bool,
    /// Echo lines dropped because the queue was full, reported at the next pump.
    pub dropped_chat: usize,
    /// Script requests dropped because the queue was full.
    pub dropped_requests: usize,
}

/// Lock a mutex, recovering from poisoning (a panic elsewhere must not cascade).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Per-call work budget of one apartment (§3.3), shared by the VM instruction hook and the
/// pattern matcher.
#[derive(Debug)]
pub(crate) struct Budget {
    used: AtomicU64,
    limit: Option<u64>,
}

impl Budget {
    pub(crate) fn new(limit: Option<u64>) -> Self {
        Self {
            used: AtomicU64::new(0),
            limit,
        }
    }

    /// Account for `n` units of work; fails once the call is over its budget.
    pub(crate) fn charge(&self, n: u64) -> mlua::Result<()> {
        let used = self.used.fetch_add(n, Ordering::Relaxed).saturating_add(n);
        match self.limit {
            Some(limit) if used > limit => Err(mlua::Error::runtime(BUDGET_MESSAGE)),
            _ => Ok(()),
        }
    }

    /// Has the current call spent its budget?
    pub(crate) fn exhausted(&self) -> bool {
        self.limit
            .is_some_and(|limit| self.used.load(Ordering::Relaxed) > limit)
    }

    fn reset(&self) {
        self.used.store(0, Ordering::Relaxed);
    }
}

/// An error raised by host code (as opposed to one raised by Lua code and passed through).
#[derive(Debug)]
pub(crate) struct HostError(pub String);

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HostError {}

/// An error message raised by a native function.
pub(crate) fn host_error(msg: impl Into<String>) -> mlua::Error {
    mlua::Error::external(HostError(msg.into()))
}

/// Was this error raised by Lua code (so it already carries its position)?
fn raised_by_lua(e: &mlua::Error) -> bool {
    match e {
        mlua::Error::CallbackError { cause, .. } | mlua::Error::WithContext { cause, .. } => {
            raised_by_lua(cause)
        }
        mlua::Error::RuntimeError(_)
        | mlua::Error::MemoryError(_)
        | mlua::Error::SyntaxError { .. } => true,
        _ => false,
    }
}

/// Lua's word for the Rust type an argument conversion wanted.
fn lua_type_word(rust: &str) -> String {
    match rust {
        "String" | "string" | "LuaString" | "BString" | "str" => "string".to_owned(),
        "Table" => "table".to_owned(),
        "Function" => "function".to_owned(),
        "bool" => "boolean".to_owned(),
        "f64" | "f32" | "i64" | "i32" | "u64" | "u32" | "usize" | "isize" | "u8" | "i8" | "u16"
        | "i16" | "Number" | "Integer" => "number".to_owned(),
        other => other.to_lowercase(),
    }
}

fn plain_message(e: &mlua::Error) -> String {
    match e {
        mlua::Error::BadArgument { to, pos, cause, .. } => {
            let what = match &**cause {
                mlua::Error::FromLuaConversionError { from, to, .. } => {
                    format!("{} expected, got {from}", lua_type_word(to))
                }
                other => plain_message(other),
            };
            format!(
                "bad argument #{pos} to '{}' ({what})",
                to.as_deref().unwrap_or("?")
            )
        }
        mlua::Error::CallbackError { cause, .. } => plain_message(cause),
        other => describe_error(other).0,
    }
}

/// `chunk:line: ` of the Lua code that called the wrapped native (`luaL_where(L, 1)` from
/// the script's point of view), or `None` when the caller is not Lua code.
fn where_prefix(lua: &Lua) -> Option<String> {
    // Level 0 is the raw native, level 1 the shim wrapper, level 2 the script.
    lua.inspect_stack(2, |d| {
        let line = d.current_line().filter(|&l| l > 0)?;
        let src = d.source().short_src?.into_owned();
        Some(format!("{src}:{line}: "))
    })
    .flatten()
}

/// The string a failed native raises in Lua.
fn native_error_message(lua: &Lua, e: &mlua::Error) -> String {
    if raised_by_lua(e) {
        return describe_error(e).0;
    }
    let msg = plain_message(e);
    match where_prefix(lua) {
        Some(w) => format!("{w}{msg}"),
        None => msg,
    }
}

/// Create a native function whose errors reach Lua as plain strings with a `chunk:line:`
/// prefix (Lua 5.1 semantics, §3.1), never as host error objects. Arguments are converted
/// to `A`; a failed conversion raises `bad argument #n to '<name>' (...)`.
pub(crate) fn native_fn<A, R, F>(lua: &Lua, name: &str, f: F) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
{
    let name = name.to_owned();
    let raw = lua.create_function(move |lua, args: MultiValue| {
        let result = A::from_lua_args(args, 1, Some(&name), lua)
            .and_then(|a| f(lua, a))
            .and_then(|r| r.into_lua_multi(lua));
        match result {
            Ok(mut out) => {
                out.push_front(Value::Boolean(true));
                Ok(out)
            }
            Err(e) => {
                let msg = lua.create_string(native_error_message(lua, &e))?;
                Ok(MultiValue::from_vec(vec![
                    Value::Boolean(false),
                    Value::String(msg),
                ]))
            }
        }
    })?;
    let make: Function = lua.named_registry_value(SHIM_KEY)?;
    make.call(raw)
}

/// A callback deferred to the next pump (plugin-data completion callbacks, §10.1).
#[derive(Debug)]
struct Deferred {
    func: Function,
    args: Vec<Value>,
    context: &'static str,
}

#[derive(Debug, Default)]
pub(crate) struct CtxInner {
    /// Canonical paths of files already run by `import` (§7.3).
    imported: HashSet<PathBuf>,
    deferred: Vec<Deferred>,
    /// Lower-case command name → registry key of the `ShellCommand` table.
    pub commands: HashMap<String, RegistryKey>,
}

/// Per-apartment context captured by the native callbacks.
pub(crate) struct ApartmentCtx {
    /// Display name.
    pub name: String,
    /// Lower-case name used for comparisons (§6.1).
    pub key: String,
    pub plugins_root: PathBuf,
    pub host: Arc<dyn HostApi>,
    pub shared: Arc<Mutex<Shared>>,
    pub limits: Limits,
    pub inner: Mutex<CtxInner>,
    pub budget: Arc<Budget>,
}

impl std::fmt::Debug for ApartmentCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApartmentCtx")
            .field("name", &self.name)
            .field("plugins_root", &self.plugins_root)
            .finish_non_exhaustive()
    }
}

impl ApartmentCtx {
    /// Is there room for another deferred callback?
    pub(crate) fn can_defer(&self) -> bool {
        lock(&self.inner).deferred.len() < MAX_DEFERRED
    }

    /// Queue a callback for the next pump; dropped (with a warning) when the queue is full.
    pub(crate) fn defer(&self, func: Function, args: Vec<Value>, context: &'static str) {
        let mut inner = lock(&self.inner);
        if inner.deferred.len() >= MAX_DEFERRED {
            tracing::warn!(apartment = %self.name, "deferred callback queue full; dropping {context}");
            return;
        }
        inner.deferred.push(Deferred {
            func,
            args,
            context,
        });
    }
}

/// Host data attached to a native object created by an embedding layer.
#[derive(Clone)]
pub(crate) struct Payload(pub Arc<dyn Any + Send + Sync>);

impl std::fmt::Debug for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Payload(..)")
    }
}

/// What a native object bound to a Lua table is (§8.2).
#[derive(Debug, Clone)]
pub(crate) enum NativeKind {
    Object,
    Plugin(Box<PluginRow>),
    ShellCommand,
    /// An object of a class installed by the embedding client (§7.4 extension point).
    External(Payload),
}

/// The opaque userdata stored in the hidden natives table.
#[derive(Debug)]
pub(crate) struct Native {
    pub kind: NativeKind,
    /// The class table the object was constructed as (for `IsA`).
    pub class: Table,
}

impl UserData for Native {}

/// The native object bound to `obj`, if any.
pub(crate) fn native_of(lua: &Lua, obj: &Table) -> mlua::Result<Option<NativeKind>> {
    let natives: Table = lua.named_registry_value(NATIVES_KEY)?;
    match natives.raw_get::<Value>(obj)? {
        Value::UserData(ud) => Ok(Some(ud.borrow::<Native>()?.kind.clone())),
        _ => Ok(None),
    }
}

/// The class table of the native object bound to `obj`, if any.
pub(crate) fn native_class_of(lua: &Lua, obj: &Table) -> mlua::Result<Option<Table>> {
    let natives: Table = lua.named_registry_value(NATIVES_KEY)?;
    match natives.raw_get::<Value>(obj)? {
        Value::UserData(ud) => Ok(Some(ud.borrow::<Native>()?.class.clone())),
        _ => Ok(None),
    }
}

/// Bind a new native object of `kind`/`class` to `obj` without touching its metatable
/// (§8.3 step 3).
pub(crate) fn bind_native(
    lua: &Lua,
    obj: &Table,
    kind: NativeKind,
    class: Table,
) -> mlua::Result<()> {
    let natives: Table = lua.named_registry_value(NATIVES_KEY)?;
    let ud = lua.create_userdata(Native { kind, class })?;
    natives.raw_set(obj, ud)
}

/// Split an mlua error into (message, traceback) for reporting (§6.6).
pub(crate) fn describe_error(e: &mlua::Error) -> (String, Option<String>) {
    match e {
        mlua::Error::CallbackError { traceback, cause } => {
            let (msg, inner_tb) = describe_error(cause);
            (msg, inner_tb.or_else(|| Some(traceback.clone())))
        }
        mlua::Error::WithContext { context, cause } => {
            let (msg, tb) = describe_error(cause);
            (format!("{context}: {msg}"), tb)
        }
        mlua::Error::RuntimeError(m) | mlua::Error::MemoryError(m) => split_traceback(m),
        mlua::Error::SyntaxError { message, .. } => split_traceback(message),
        other => (other.to_string(), None),
    }
}

fn split_traceback(m: &str) -> (String, Option<String>) {
    match m.find("\nstack traceback:") {
        Some(i) => (m[..i].to_owned(), Some(m[i + 1..].to_owned())),
        None => (m.to_owned(), None),
    }
}

/// State recorded before a plugin's main package runs, for cleanup if it fails.
#[derive(Debug)]
pub(crate) struct LoadSnapshot {
    commands: HashSet<String>,
    deferred: usize,
}

/// One Lua state (§6.1).
pub(crate) struct Apartment {
    pub lua: Lua,
    pub ctx: Arc<ApartmentCtx>,
    /// Plugin objects in load order: (plugin name, object table).
    pub plugins: Vec<(String, Table)>,
    pub handles: crate::turbine::Handles,
    caller: Function,
}

impl std::fmt::Debug for Apartment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apartment")
            .field("name", &self.ctx.name)
            .field(
                "plugins",
                &self.plugins.iter().map(|p| &p.0).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Apartment {
    /// Create a state with the §3.2 libraries, `import`, and the native packages (§6.2 step 3).
    pub(crate) fn new(
        name: &str,
        plugins_root: &Path,
        host: Arc<dyn HostApi>,
        shared: Arc<Mutex<Shared>>,
        limits: Limits,
    ) -> mlua::Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH,
            LuaOptions::default(),
        )?;
        if limits.memory_bytes > 0 {
            lua.set_memory_limit(limits.memory_bytes)?;
        }
        let budget = Arc::new(Budget::new(limits.instructions_per_call));
        if limits.instructions_per_call.is_some() {
            let b = Arc::clone(&budget);
            lua.set_global_hook(
                HookTriggers::new().every_nth_instruction(HOOK_GRANULARITY),
                move |_, _| {
                    b.charge(u64::from(HOOK_GRANULARITY))
                        .map(|()| VmState::Continue)
                },
            )?;
        }
        let shim: Function = lua
            .load(SHIM_SOURCE)
            .set_name("=tessera")
            .set_mode(ChunkMode::Text)
            .call(())?;
        lua.set_named_registry_value(SHIM_KEY, shim)?;

        let ctx = Arc::new(ApartmentCtx {
            name: name.to_owned(),
            key: name.to_lowercase(),
            plugins_root: plugins_root.to_path_buf(),
            host,
            shared,
            limits,
            inner: Mutex::new(CtxInner::default()),
            budget,
        });

        let natives = lua.create_table()?;
        let weak = lua.create_table()?;
        weak.raw_set("__mode", "k")?;
        natives.set_metatable(Some(weak))?;
        lua.set_named_registry_value(NATIVES_KEY, natives)?;

        install_sandbox(&lua, &ctx)?;
        install_import(&lua, &ctx)?;
        let handles = crate::turbine::install(&lua, &ctx)?;
        // Embedding layers add their native packages before any plugin code runs.
        ctx.host
            .install_natives(&crate::api::NativeApi::new(&lua, &ctx, &handles))?;
        lua.globals().raw_set("Plugins", lua.create_table()?)?;

        let caller = lua
            .load("local h, o, a = ...; return h(o, a)")
            .set_name("=tessera")
            .set_mode(ChunkMode::Text)
            .into_function()?;

        Ok(Self {
            lua,
            ctx,
            plugins: Vec::new(),
            handles,
            caller,
        })
    }

    /// Reset the per-call instruction budget (every host-to-Lua entry point).
    pub(crate) fn enter(&self) {
        self.ctx.budget.reset();
    }

    pub(crate) fn report(&self, plugin: Option<&str>, context: String, e: &mlua::Error) {
        let (message, traceback) = describe_error(e);
        self.ctx.host.report_error(&ScriptErrorReport {
            plugin: plugin.map(str::to_owned),
            apartment: self.ctx.name.clone(),
            context,
            message,
            traceback,
        });
    }

    /// Create the `Turbine.Plugin` object for `desc` and publish it as `Plugins[name]` and
    /// `plugin` (§6.2 step 4).
    pub(crate) fn create_plugin_object(&mut self, desc: &PluginDescriptor) -> mlua::Result<Table> {
        let lua = &self.lua;
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", self.handles.plugin_class.clone())?;
        obj.set_metatable(Some(mt))?;
        bind_native(
            lua,
            &obj,
            NativeKind::Plugin(Box::new(PluginRow {
                name: desc.name.clone(),
                author: desc.author.clone(),
                version: desc.version.clone(),
                description: desc.description.clone(),
                apartment: self.ctx.name.clone(),
            })),
            self.handles.plugin_class.clone(),
        )?;
        let plugins: Table = lua.globals().raw_get("Plugins")?;
        plugins.raw_set(desc.name.as_str(), obj.clone())?;
        lua.globals().raw_set("plugin", obj.clone())?;
        self.plugins.push((desc.name.clone(), obj.clone()));
        Ok(obj)
    }

    /// What a failed load must undo: commands and deferred callbacks present before it.
    pub(crate) fn snapshot(&self) -> LoadSnapshot {
        let inner = lock(&self.ctx.inner);
        LoadSnapshot {
            commands: inner.commands.keys().cloned().collect(),
            deferred: inner.deferred.len(),
        }
    }

    /// Undo [`Self::create_plugin_object`] and the failed plugin's top-level registrations
    /// after a failed load (§6.2 step 6): remove it from `Plugins`, unregister shell
    /// commands it added, drop callbacks it queued, and point `plugin` back at the most
    /// recently loaded plugin of the apartment (§6.2 OPEN recommendation).
    pub(crate) fn forget_plugin(&mut self, name: &str, before: &LoadSnapshot) {
        self.plugins.retain(|(n, _)| n != name);
        let globals = self.lua.globals();
        if let Ok(plugins) = globals.raw_get::<Table>("Plugins") {
            let _ = plugins.raw_set(name, Value::Nil);
        }
        let previous = self
            .plugins
            .last()
            .map_or(Value::Nil, |(_, obj)| Value::Table(obj.clone()));
        let _ = globals.raw_set("plugin", previous);

        let mut shared = lock(&self.ctx.shared);
        let mut inner = lock(&self.ctx.inner);
        let added: Vec<String> = inner
            .commands
            .keys()
            .filter(|k| !before.commands.contains(*k))
            .cloned()
            .collect();
        for n in added {
            if shared.commands.get(&n) == Some(&self.ctx.key) {
                shared.commands.remove(&n);
            }
            if let Some(k) = inner.commands.remove(&n) {
                let _ = self.lua.remove_registry_value(k);
            }
        }
        inner.deferred.truncate(before.deferred);
    }

    /// Run `import(package)` from the host (§6.2 step 5).
    pub(crate) fn import(&self, package: &str) -> mlua::Result<()> {
        self.enter();
        import_package(&self.lua, &self.ctx, package)
    }

    /// Fire event `event` on `target` (§9.2). Errors are reported, never propagated.
    pub(crate) fn fire_event(
        &self,
        target: &Table,
        event: &str,
        args: Table,
        plugin: Option<&str>,
    ) {
        self.enter();
        let context = || format!("handling event {event}");
        let handler: Value = match target.get(event) {
            Ok(v) => v,
            Err(e) => return self.report(plugin, context(), &e),
        };
        let call = |h: Value| {
            // Each handler (and each element of a handler array) gets its own budget, so a
            // runaway handler cannot starve the next one (§9.2 step 5, §3.3).
            self.enter();
            let result = match &h {
                Value::Function(f) => f.call::<()>((target.clone(), args.clone())),
                _ => self.caller.call::<()>((h, target.clone(), args.clone())),
            };
            if let Err(e) = result {
                self.report(plugin, context(), &e);
            }
        };
        match handler {
            Value::Nil | Value::Boolean(false) => {}
            Value::Function(_) => call(handler),
            Value::Table(ref t) if has_call(t) => call(handler),
            Value::Table(t) => {
                for i in 1..=t.raw_len() {
                    match t.raw_get::<Value>(i) {
                        Ok(v @ Value::Function(_)) => call(v),
                        Ok(Value::Table(et)) if has_call(&et) => call(Value::Table(et)),
                        Ok(_) => {}
                        Err(e) => self.report(plugin, context(), &e),
                    }
                }
            }
            Value::UserData(_) => call(handler),
            _ => {}
        }
    }

    /// Call `method` on `obj` as `obj:method(args...)`, honouring `__index` (§11.2).
    pub(crate) fn call_method(
        &self,
        obj: &Table,
        method: &str,
        args: MultiValue,
    ) -> mlua::Result<MultiValue> {
        self.enter();
        let f: Value = obj.get(method)?;
        let mut all = args;
        all.push_front(Value::Table(obj.clone()));
        match f {
            Value::Function(f) => f.call(all),
            Value::Nil => Ok(MultiValue::new()),
            other => {
                let mut with_handler = all;
                with_handler.push_front(other);
                self.lua
                    .load("local h = ...; return h(select(2, ...))")
                    .set_mode(ChunkMode::Text)
                    .call(with_handler)
            }
        }
    }

    /// Run callbacks deferred by plugin-data calls (§10.1, §10.6).
    pub(crate) fn run_deferred(&self) {
        // Bounded: callbacks queued by these callbacks run on the next pump.
        let batch = std::mem::take(&mut lock(&self.ctx.inner).deferred);
        for d in batch {
            self.enter();
            if let Err(e) = d.func.call::<()>(MultiValue::from_vec(d.args)) {
                self.report(None, d.context.to_owned(), &e);
            }
        }
    }

    /// Look up the `ShellCommand` table registered under `name` (lower case).
    pub(crate) fn command_table(&self, name: &str) -> Option<Table> {
        let inner = lock(&self.ctx.inner);
        let key = inner.commands.get(name)?;
        self.lua.registry_value::<Table>(key).ok()
    }
}

fn has_call(t: &Table) -> bool {
    t.metatable()
        .and_then(|mt| mt.raw_get::<Value>("__call").ok())
        .is_some_and(|v| !v.is_nil())
}

/// Remove or replace globals so the state matches §3.2.
fn install_sandbox(lua: &Lua, ctx: &Arc<ApartmentCtx>) -> mlua::Result<()> {
    let g = lua.globals();
    for name in [
        "io", "os", "package", "debug", "dofile", "require", "module", "loadfile",
    ] {
        g.raw_set(name, Value::Nil)?;
    }

    // `loadstring`/`load` for text chunks only, compiled with the apartment's globals.
    let loadstring = native_fn(
        lua,
        "loadstring",
        |lua, (src, chunkname): (mlua::LuaString, Option<String>)| {
            compile_text(lua, &src.as_bytes(), chunkname)
        },
    )?;
    g.raw_set("loadstring", loadstring)?;
    let max_source = ctx.limits.max_source_bytes;
    let load = native_fn(
        lua,
        "load",
        move |lua, (reader, chunkname): (Value, Option<String>)| {
            let src: Vec<u8> = match reader {
                Value::String(s) => s.as_bytes().to_vec(),
                Value::Function(f) => {
                    let mut buf = Vec::new();
                    loop {
                        match f.call::<Value>(())? {
                            Value::String(piece) if !piece.as_bytes().is_empty() => {
                                buf.extend_from_slice(&piece.as_bytes());
                                if buf.len() as u64 > max_source {
                                    return Err(host_error("load: chunk too large"));
                                }
                            }
                            Value::Nil | Value::String(_) => break,
                            _ => {
                                return Err(host_error("reader function must return a string"));
                            }
                        }
                    }
                    buf
                }
                other => {
                    return Err(host_error(format!(
                        "bad argument #1 to 'load' (function expected, got {})",
                        other.type_name()
                    )));
                }
            };
            compile_text(lua, &src, chunkname)
        },
    )?;
    g.raw_set("load", load)?;

    // `print` goes to the script log (§3.2 recommendation).
    let tostring: Function = g.raw_get("tostring")?;
    let c = Arc::clone(ctx);
    let print = native_fn(lua, "print", move |_, args: MultiValue| {
        let mut parts = Vec::with_capacity(args.len());
        for v in args {
            parts.push(tostring.call::<mlua::LuaString>(v)?.to_string_lossy());
        }
        c.host.script_log(&c.name, &parts.join("\t"));
        Ok(())
    })?;
    g.raw_set("print", print)?;

    // Budget errors cannot be swallowed by protected calls (§3.3).
    let budget = Arc::clone(&ctx.budget);
    let exhausted = lua.create_function(move |_, ()| Ok(budget.exhausted()))?;
    let coroutine: Table = g.raw_get("coroutine")?;
    let (pcall, xpcall, resume): (Function, Function, Function) = lua
        .load(GUARD_SOURCE)
        .set_name("=tessera")
        .set_mode(ChunkMode::Text)
        .call((
            g.raw_get::<Function>("pcall")?,
            g.raw_get::<Function>("xpcall")?,
            coroutine.raw_get::<Function>("resume")?,
            exhausted,
            g.raw_get::<Function>("error")?,
            BUDGET_MESSAGE,
        ))?;
    g.raw_set("pcall", pcall)?;
    g.raw_set("xpcall", xpcall)?;
    coroutine.raw_set("resume", resume)?;

    // Bounded pattern matching (no native-stack overflow, budgeted backtracking).
    let string: Table = g.raw_get("string")?;
    crate::pattern::install(lua, &string, ctx)?;
    Ok(())
}

/// Compile text Lua source with the apartment globals; returns `(f)` or `(nil, message)`
/// like Lua 5.1's `loadstring`.
fn compile_text(lua: &Lua, src: &[u8], chunkname: Option<String>) -> mlua::Result<MultiValue> {
    if src.starts_with(BYTECODE_SIGNATURE) {
        return lua.pack_multi((Value::Nil, "binary chunks are not allowed"));
    }
    let name = chunkname.unwrap_or_else(|| {
        let first = src.split(|&b| b == b'\n').next().unwrap_or_default();
        format!(
            "={}",
            String::from_utf8_lossy(&first[..first.len().min(60)])
        )
    });
    match lua
        .load(src)
        .set_name(name)
        .set_mode(ChunkMode::Text)
        .set_environment(lua.globals())
        .into_function()
    {
        Ok(f) => lua.pack_multi(f),
        Err(e) => lua.pack_multi((Value::Nil, describe_error(&e).0)),
    }
}

fn install_import(lua: &Lua, ctx: &Arc<ApartmentCtx>) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    let import = native_fn(lua, "import", move |lua, name: Option<Value>| {
        let name = match name {
            Some(Value::String(s)) => s
                .to_str()
                .map_err(|_| host_error("import: package name is not valid UTF-8"))?
                .to_owned(),
            Some(Value::Integer(i)) => i.to_string(),
            Some(Value::Number(n)) => n.to_string(),
            _ => return Err(host_error("import: no package name given")),
        };
        import_package(lua, &c, &name)
    })?;
    lua.globals().raw_set("import", import)
}

/// The `import` algorithm of §7.2.
pub(crate) fn import_package(lua: &Lua, ctx: &ApartmentCtx, name: &str) -> mlua::Result<()> {
    if NATIVE_PACKAGES.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Ok(());
    }
    let comps: Vec<&str> = name.split('.').collect();
    let Some((path, ns_len)) = crate::paths::resolve_package(&ctx.plugins_root, name) else {
        return Err(host_error(format!(
            "import: cannot resolve package '{name}'"
        )));
    };
    let ns_comps = &comps[..ns_len];

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !lock(&ctx.inner).imported.insert(canonical.clone()) {
        return Ok(()); // §7.3: each file runs at most once per apartment
    }
    let result = run_package_file(lua, ctx, name, &path, ns_comps);
    if result.is_err() {
        lock(&ctx.inner).imported.remove(&canonical);
    }
    result
}

fn run_package_file(
    lua: &Lua,
    ctx: &ApartmentCtx,
    name: &str,
    path: &Path,
    ns_comps: &[&str],
) -> mlua::Result<()> {
    let globals = lua.globals();
    // Namespace tables (§7.2 step 3).
    let mut ns = globals.clone();
    for c in ns_comps {
        ns = match ns.get::<Value>(*c)? {
            Value::Table(t) => t,
            Value::Nil => {
                let t = lua.create_table()?;
                ns.set(*c, t.clone())?;
                t
            }
            other => {
                return Err(host_error(format!(
                    "import: '{c}' in '{name}' is a {}, not a table",
                    other.type_name()
                )));
            }
        };
    }

    // Source (§7.2 step 4): bounded, text only.
    let len = std::fs::metadata(path)
        .map_err(|e| host_error(format!("import: cannot open '{name}': {e}")))?
        .len();
    if len > ctx.limits.max_source_bytes {
        return Err(host_error(format!(
            "import: '{name}' is larger than {} bytes",
            ctx.limits.max_source_bytes
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| host_error(format!("import: cannot read '{name}': {e}")))?;
    let src = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    if src.starts_with(BYTECODE_SIGNATURE) {
        return Err(host_error(format!(
            "import: '{name}' is a binary chunk; only source files are allowed"
        )));
    }
    let rel = path
        .strip_prefix(&ctx.plugins_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    // Environment (§7.2 step 5): namespace first, then the real globals.
    let env = if ns_comps.is_empty() {
        globals
    } else {
        let env = lua.create_table()?;
        let mt = lua.create_table()?;
        // Plain Lua, so errors from the tables' own metamethods stay ordinary Lua errors.
        let index: Function = lua
            .load(ENV_INDEX_SOURCE)
            .set_name("=tessera")
            .set_mode(ChunkMode::Text)
            .call((ns.clone(), globals.clone()))?;
        mt.raw_set("__index", index)?;
        mt.raw_set("__newindex", ns)?;
        env.set_metatable(Some(mt))?;
        env
    };

    tracing::debug!(apartment = %ctx.name, package = name, file = %rel, "import");
    lua.load(src)
        .set_name(format!("@{rel}"))
        .set_mode(ChunkMode::Text)
        .set_environment(env)
        .exec()
}

// ---------------------------------------------------------------------------------------
// Lua <-> plugin data
// ---------------------------------------------------------------------------------------

/// Convert a Lua value to a saveable [`DataValue`] (§10.5).
pub(crate) fn lua_to_data(
    lua: &Lua,
    v: &Value,
    limits: &ParseLimits,
) -> Result<DataValue, PluginDataError> {
    let natives: Option<Table> = lua.named_registry_value(NATIVES_KEY).ok();
    let mut walk = Walk {
        natives: natives.as_ref(),
        limits,
        path: Vec::new(),
        count: 0,
        bytes: 0,
    };
    walk.convert(v)
}

/// Conversion state: open tables (cycle check), entry count, and a running lower bound
/// of the serialized size, so a value that references one large string many times is
/// rejected before its copies are made (the file would be refused by `Load` anyway).
struct Walk<'a> {
    natives: Option<&'a Table>,
    limits: &'a ParseLimits,
    path: Vec<usize>,
    count: usize,
    bytes: u64,
}

impl Walk<'_> {
    fn add_bytes(&mut self, n: u64) -> Result<(), PluginDataError> {
        self.bytes = self.bytes.saturating_add(n);
        if self.bytes > self.limits.max_bytes {
            Err(PluginDataError::ValueTooLarge(self.limits.max_bytes))
        } else {
            Ok(())
        }
    }

    fn convert(&mut self, v: &Value) -> Result<DataValue, PluginDataError> {
        Ok(match v {
            Value::Nil => DataValue::Nil,
            Value::Boolean(b) => DataValue::Boolean(*b),
            Value::Integer(i) => DataValue::Number(*i as f64),
            Value::Number(n) if n.is_finite() => DataValue::Number(*n),
            Value::Number(_) => return Err(PluginDataError::NonFinite),
            Value::String(s) => {
                let bytes = s.as_bytes();
                // Quotes plus every byte at least once.
                self.add_bytes(bytes.len() as u64 + 2)?;
                DataValue::String(bytes.to_vec())
            }
            Value::Table(t) => {
                let ptr = t.to_pointer() as usize;
                if self.path.contains(&ptr) {
                    return Err(PluginDataError::Cycle);
                }
                if self.path.len() >= self.limits.max_depth {
                    return Err(PluginDataError::TooDeep(self.limits.max_depth));
                }
                if self
                    .natives
                    .is_some_and(|n| n.raw_get::<Value>(t).is_ok_and(|b| !b.is_nil()))
                {
                    return Err(PluginDataError::Unsupported("API object"));
                }
                self.path.push(ptr);
                // Braces and line breaks of the table itself.
                self.add_bytes(4 + 2 * self.path.len() as u64)?;
                let mut map = std::collections::BTreeMap::new();
                for pair in t.pairs::<Value, Value>() {
                    let (k, val) = pair.map_err(|_| PluginDataError::Unsupported("table"))?;
                    self.count += 1;
                    if self.count > self.limits.max_entries {
                        return Err(PluginDataError::TooManyEntries(self.limits.max_entries));
                    }
                    // Indentation, brackets, ` = `, separator and line break.
                    self.add_bytes(self.path.len() as u64 + 8)?;
                    let key = match &k {
                        Value::Integer(i) => DataKey::number(*i as f64)?,
                        Value::Number(n) => DataKey::number(*n)?,
                        Value::String(s) => {
                            let bytes = s.as_bytes();
                            self.add_bytes(bytes.len() as u64 + 2)?;
                            DataKey::String(bytes.to_vec())
                        }
                        other => return Err(PluginDataError::UnsupportedKey(other.type_name())),
                    };
                    let dv = self.convert(&val)?;
                    map.insert(key, dv);
                }
                self.path.pop();
                DataValue::Table(map)
            }
            other => return Err(PluginDataError::Unsupported(other.type_name())),
        })
    }
}

/// Build a fresh Lua value from a [`DataValue`].
pub(crate) fn data_to_lua(lua: &Lua, v: &DataValue) -> mlua::Result<Value> {
    Ok(match v {
        DataValue::Nil => Value::Nil,
        DataValue::Boolean(b) => Value::Boolean(*b),
        DataValue::Number(n) => Value::Number(*n),
        DataValue::String(s) => Value::String(lua.create_string(s)?),
        DataValue::Table(map) => {
            let t = lua.create_table()?;
            for (k, val) in map {
                let key = match k {
                    DataKey::Number(n) => Value::Number(*n),
                    DataKey::String(s) => Value::String(lua.create_string(s)?),
                };
                t.raw_set(key, data_to_lua(lua, val)?)?;
            }
            Value::Table(t)
        }
    })
}
