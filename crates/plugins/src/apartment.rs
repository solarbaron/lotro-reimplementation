// SPDX-License-Identifier: GPL-3.0-or-later
//! Apartments: one sandboxed Lua state each, with `import`, the native object binding and
//! event dispatch.
//!
//! Spec: `plugin-runtime.md` §5 (creation, globals, isolation), §8 (lifecycle), §11
//! (errors); `lua-plugin-api.md` §6.1–§6.3, §7, §9.2.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mlua::{IntoLuaMulti, Lua, MultiValue, Table, Value};
use tessera_script::{Conversions, CoreHost, ErrorReport, Profile, RandomSource, ScriptState};

use crate::error::PluginError;
use crate::host::{CHAT_TYPE_ERROR, HostApi};
use crate::import::{GLOBALS_KEY, import_package};
use crate::object::{NativeKind, PluginRow, bind_native, install_natives_table};
use crate::root_ns::{Handles, truncate_line};
use crate::shared::{ApartmentCtx, ChatEvent, CtxInner, RunGuard, Shared, apartment_key, lock};

/// Adapter that hands the core's own reports (finalizer errors in a collector step, …) to
/// the plugin host's error path.
struct CoreAdapter {
    host: Arc<dyn HostApi>,
    shared: std::sync::Weak<Shared>,
}

impl CoreHost for CoreAdapter {
    fn script_log(&self, state: &str, message: &str) {
        self.host.script_log(state, message);
    }

    fn report_error(&self, report: &ErrorReport) {
        let mut report = report.clone();
        truncate_line(&mut report.message);
        self.host.report_error(&report);
        if let Some(shared) = self.shared.upgrade() {
            lock(&shared.state).queue_chat(ChatEvent {
                sender: None,
                message: report.message,
                chat_type: CHAT_TYPE_ERROR,
            });
        }
    }
}

/// One plugin of an apartment, in load order.
#[derive(Debug)]
pub(crate) struct LoadedPlugin {
    pub name: String,
    pub object: Table,
}

/// One Lua state (`lua-plugin-api.md` §6.1).
pub(crate) struct Apartment {
    pub state: ScriptState,
    pub ctx: Arc<ApartmentCtx>,
    /// Plugins in load order.
    pub plugins: Vec<LoadedPlugin>,
    pub handles: Handles,
    /// The apartment's real global table.
    pub globals: Table,
}

impl std::fmt::Debug for Apartment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apartment")
            .field("name", &self.ctx.name)
            .field(
                "plugins",
                &self.plugins.iter().map(|p| &p.name).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Apartment {
    /// Create an apartment (`plugin-runtime.md` §5.1): the sandboxed state of the plugin
    /// profile, `import`, the native packages, the host layers' natives, then
    /// `import "Turbine"` (native part, then the built-in root or the user's
    /// `Plugins/Turbine/` disk part).
    pub(crate) fn create(shared: &Arc<Shared>, name: &str) -> Result<Self, PluginError> {
        let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
        let profile = Profile {
            name: name.to_owned(),
            limits: shared.limits.clone(),
            base: tessera_script::BaseFunctions::default(),
            chunk_prefix: String::new(),
            random: RandomSource::Shared(Arc::clone(&shared.random)),
            conversions: Conversions::Compatible,
        };
        let core_host: Arc<dyn CoreHost> = Arc::new(CoreAdapter {
            host: Arc::clone(&shared.host),
            shared: Arc::downgrade(shared),
        });
        let state = ScriptState::new(profile, core_host)?;
        let builtin_mode = shared.builtins.is_some() && !user_copy_present(shared);
        let ctx = Arc::new(ApartmentCtx {
            name: name.to_owned(),
            key: apartment_key(name),
            id,
            shared: Arc::downgrade(shared),
            running: AtomicUsize::new(0),
            loading: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            budget: Arc::clone(state.budget()),
            builtin_mode,
            inner: Mutex::new(CtxInner::default()),
        });
        let lua = state.lua();
        let globals = lua.globals();
        lua.set_named_registry_value(GLOBALS_KEY, globals.clone())?;
        lua.set_app_data(Arc::clone(&ctx));
        install_natives_table(lua)?;
        crate::import::install(lua, &ctx)?;
        let handles = crate::root_ns::install(lua, &ctx)?;
        crate::gameplay::install(lua, &ctx, &handles)?;
        {
            let _run = RunGuard::new(&ctx);
            state.enter();
            shared
                .host
                .install_natives(&crate::api::NativeApi::new(lua, &ctx, &handles))?;
        }
        let ap = Self {
            state,
            ctx,
            plugins: Vec::new(),
            handles,
            globals,
        };
        // Step 5: `import "Turbine"`. A failing disk part is reported; the native part
        // stays installed and the apartment is usable.
        if let Err(e) = ap.run(|lua| import_package(lua, &ap.ctx, "Turbine")) {
            ap.report(None, "importing Turbine", &e);
        }
        Ok(ap)
    }

    pub(crate) fn lua(&self) -> &Lua {
        self.state.lua()
    }

    /// Run `f` as one host call: the apartment is marked running and the instruction
    /// budget is reset (unless a host call into it is already running).
    pub(crate) fn run<R>(&self, f: impl FnOnce(&Lua) -> mlua::Result<R>) -> mlua::Result<R> {
        if !self.ctx.is_running() {
            self.state.enter();
        }
        let _run = RunGuard::new(&self.ctx);
        f(self.lua())
    }

    /// Call a callable value as one host call (§11.1): protected, error value normalised.
    pub(crate) fn call(&self, f: &Value, args: impl IntoLuaMulti) -> mlua::Result<MultiValue> {
        let nested = self.ctx.is_running();
        let _run = RunGuard::new(&self.ctx);
        if nested {
            self.state.call_nested(f, args)
        } else {
            self.state.call(f, args)
        }
    }

    /// Report an error of this apartment (`lua-plugin-api.md` §6.6): to the host, and as
    /// a chat line of type 1 queued for `Received` handlers.
    pub(crate) fn report(&self, plugin: Option<&str>, context: &str, e: &mlua::Error) {
        report_ctx(&self.ctx, plugin, context, e);
    }

    /// Create the `Turbine.Plugin` object for `row` and publish it as `Plugins[name]` and
    /// `plugin` (§6.2 steps 2–3).
    pub(crate) fn create_plugin_object(&mut self, row: PluginRow) -> mlua::Result<Table> {
        let lua = self.state.lua();
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", self.handles.plugin_class.clone())?;
        obj.set_metatable(Some(mt))?;
        let name = row.name.clone();
        bind_native(
            lua,
            &obj,
            NativeKind::Plugin(Box::new(row)),
            self.handles.plugin_class.clone(),
        )?;
        let plugins = match self.globals.raw_get::<Value>("Plugins")? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                self.globals.raw_set("Plugins", t.clone())?;
                t
            }
        };
        plugins.raw_set(name.as_str(), obj.clone())?;
        self.globals.raw_set("plugin", obj.clone())?;
        Ok(obj)
    }

    /// Remove `Plugins[name]` if it is a table.
    pub(crate) fn clear_plugins_entry(&self, name: &str) {
        if let Ok(Value::Table(plugins)) = self.globals.raw_get::<Value>("Plugins") {
            let _ = plugins.raw_set(name, Value::Nil);
        }
    }

    /// Fire event `event` on `target` (`lua-plugin-api.md` §9.2). Errors are reported,
    /// never propagated. Each handler call is its own host call (own budget) unless a host
    /// call into the apartment is already running.
    pub(crate) fn fire_event(
        &self,
        target: &Table,
        event: &str,
        args: Table,
        plugin: Option<&str>,
    ) {
        dispatch(self, target, event, args, plugin);
    }

    /// Call `method` on `obj` as `obj:method(args...)`, honouring `__index`.
    pub(crate) fn call_method(
        &self,
        obj: &Table,
        method: &str,
        args: MultiValue,
    ) -> mlua::Result<MultiValue> {
        let f: Value = self.run(|_| obj.get(method))?;
        if f.is_nil() {
            return Ok(MultiValue::new());
        }
        let mut all = args;
        all.push_front(Value::Table(obj.clone()));
        self.call(&f, all)
    }

    /// The `ShellCommand` table of handler id `handler` (the key of its names in the
    /// host's command registry).
    pub(crate) fn command_table(&self, handler: u64) -> Option<Table> {
        let inner = lock(&self.ctx.inner);
        let key = inner.commands.get(&handler)?;
        self.lua().registry_value::<Table>(key).ok()
    }

    /// Run the per-tick collector step (`plugin-runtime.md` §9.2).
    pub(crate) fn gc_step(&self) -> bool {
        let _run = RunGuard::new(&self.ctx);
        self.state.gc_step()
    }
}

/// Report `e` for the apartment of `ctx` (`lua-plugin-api.md` §6.6): the host's report,
/// and the error line queued for `Received` handlers with chat type 1. The message is cut
/// to the chat-line bound (4096 bytes) at a character boundary (a Tessera bound, §6.6
/// OPEN), so an error value of any size costs the host and the chat queue at most that
/// much. The kind comes from the apartment's budget state, not the message text.
pub(crate) fn report_ctx(ctx: &ApartmentCtx, plugin: Option<&str>, context: &str, e: &mlua::Error) {
    let (mut message, traceback) = tessera_script::describe_error(e);
    truncate_line(&mut message);
    let report = ErrorReport {
        source: plugin.map(str::to_owned),
        state: ctx.name.clone(),
        context: context.to_owned(),
        message,
        traceback,
        kind: tessera_script::classify(e, &ctx.budget),
    };
    if let Ok(shared) = ctx.shared() {
        shared.host.report_error(&report);
        lock(&shared.state).queue_chat(ChatEvent {
            sender: None,
            message: report.message,
            chat_type: CHAT_TYPE_ERROR,
        });
    }
}

/// Should a failure of the handler-table probe (§9.2 step 4.1) be reported? Its ordinary
/// failures are silent, but a spent budget ends the dispatch (the call is aborted,
/// `lua-plugin-api.md` §3.3) and a refused allocation is an out-of-memory report
/// (`plugin-runtime.md` §9.1). Returns `(report, stop)`.
fn probe_failure(ctx: &ApartmentCtx, e: &mlua::Error) -> (bool, bool) {
    match tessera_script::classify(e, &ctx.budget) {
        tessera_script::ErrorKind::Budget => (true, true),
        tessera_script::ErrorKind::Memory => (true, false),
        tessera_script::ErrorKind::Runtime => (false, false),
    }
}

/// Is a user copy of the sample package installed (`plugin-class-helper.md` §5.1)? From
/// the plugin file index when one is ready, else from a listing of `Plugins/`.
fn user_copy_present(shared: &Shared) -> bool {
    if let Some(idx) = lock(&shared.index).current() {
        return idx.has_user_api_copy();
    }
    let Ok(rd) = std::fs::read_dir(&shared.plugins_dir) else {
        return false;
    };
    rd.take(crate::paths::MAX_DIR_ENTRIES).flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        let path = e.path();
        (name == "turbine" && path.is_dir()) || (name == "turbine.lua" && path.is_file())
    })
}

/// The dispatch of §9.2.
fn dispatch(ap: &Apartment, target: &Table, event: &str, args: Table, plugin: Option<&str>) {
    let context = format!("handling event {event}");
    let handler: Value = match ap.run(|_| target.get(event)) {
        Ok(v) => v,
        Err(e) => return ap.report(plugin, &context, &e),
    };
    let call = |h: &Value| {
        if let Err(e) = ap.call(h, (target.clone(), args.clone())) {
            ap.report(plugin, &context, &e);
        }
    };
    match &handler {
        Value::Function(_) => call(&handler),
        Value::Table(t) => {
            // Step 4.1: the table itself, silently (but see `probe_failure`).
            match ap.call(&handler, (target.clone(), args.clone())) {
                Ok(_) => return,
                Err(e) => {
                    let (report, stop) = probe_failure(&ap.ctx, &e);
                    if report {
                        ap.report(plugin, &context, &e);
                    }
                    if stop {
                        return;
                    }
                }
            }
            // Step 4.2: every entry visited by `next`; functions are called.
            let mut values = Vec::new();
            for pair in t.pairs::<Value, Value>() {
                match pair {
                    Ok((_, v @ Value::Function(_))) => values.push(v),
                    Ok(_) => {}
                    Err(e) => {
                        ap.report(plugin, &context, &e);
                        break;
                    }
                }
            }
            for v in &values {
                call(v);
            }
        }
        _ => {}
    }
}

/// Fire an event from inside a host call of the apartment that owns `lua` (UI layers).
pub(crate) fn dispatch_in(lua: &Lua, target: &Table, event: &str, args: Table) -> mlua::Result<()> {
    let Some(ctx) = lua
        .app_data_ref::<Arc<ApartmentCtx>>()
        .map(|c| Arc::clone(&c))
    else {
        return Err(tessera_script::host_error("not a plugin apartment"));
    };
    let context = format!("handling event {event}");
    let handler: Value = target.get(event)?;
    let report = |e: &mlua::Error| report_ctx(&ctx, None, &context, e);
    let pcall = |h: &Value| -> mlua::Result<()> {
        let _run = RunGuard::new(&ctx);
        tessera_script::call_protected::<_, ()>(lua, h, (target.clone(), args.clone()))
    };
    match &handler {
        Value::Function(_) => {
            if let Err(e) = pcall(&handler) {
                report(&e);
            }
        }
        Value::Table(t) => {
            if let Err(e) = pcall(&handler) {
                let (rep, stop) = probe_failure(&ctx, &e);
                if rep {
                    report(&e);
                }
                if stop {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
            let values: Vec<Value> = t
                .pairs::<Value, Value>()
                .filter_map(Result::ok)
                .filter_map(|(_, v)| matches!(v, Value::Function(_)).then_some(v))
                .collect();
            for v in &values {
                if let Err(e) = pcall(v) {
                    report(&e);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
