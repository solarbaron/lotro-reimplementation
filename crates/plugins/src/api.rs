// SPDX-License-Identifier: GPL-3.0-or-later
//! Extension API for layers that add native packages (`Turbine.UI`, `Turbine.UI.Lotro`)
//! to every apartment, and the runtime services they use from inside plugin calls.
//!
//! The embedding client implements [`HostApi::install_natives`](crate::HostApi::install_natives);
//! the host calls it with a [`NativeApi`] for each new apartment before any plugin code
//! runs. Classes created here behave like the built-in ones: `IsA`, a native `Constructor`,
//! instantiation by calling the class, Lua subclassing (§8). Functions created here raise
//! plain Lua 5.1 strings without a position prefix, as the original's API functions do
//! (`plugin-runtime.md` §11.2). Events are fired with [`fire_event`] (inside a plugin
//! call) or [`PluginManager::fire_event`](crate::PluginManager::fire_event).
//!
//! Spec: `lua-plugin-api.md` §7.4 (native packages), §8 (object model), §9 (events),
//! §13.8 (images); decision 0015 §4.4 (the call-time image check).

use std::any::Any;
use std::sync::Arc;

use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua, Table};
use tessera_script::{api_fn, host_error};

use crate::object::{Builder, NativeKind, Payload, bind_native, native_of};
use crate::root_ns::Handles;
use crate::shared::{ApartmentCtx, lock};
use crate::vfs::{CheckedImage, ImageCheckError};

/// Access to one new apartment while its native packages are installed.
pub struct NativeApi<'a> {
    lua: &'a Lua,
    ctx: &'a Arc<ApartmentCtx>,
    handles: &'a Handles,
}

impl std::fmt::Debug for NativeApi<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeApi")
            .field("apartment", &self.ctx.name)
            .finish_non_exhaustive()
    }
}

impl<'a> NativeApi<'a> {
    pub(crate) fn new(lua: &'a Lua, ctx: &'a Arc<ApartmentCtx>, handles: &'a Handles) -> Self {
        Self { lua, ctx, handles }
    }

    fn builder(&self) -> Builder<'a> {
        self.handles.builder(self.lua)
    }

    /// The apartment's Lua state.
    #[must_use]
    pub fn lua(&self) -> &Lua {
        self.lua
    }

    /// Name of the apartment being created.
    #[must_use]
    pub fn apartment(&self) -> &str {
        &self.ctx.name
    }

    /// The `Turbine` namespace table (it already holds the `UI`, `UI.Lotro`, `Gameplay`
    /// and `Gameplay.Attributes` tables).
    #[must_use]
    pub fn ns_root(&self) -> &Table {
        &self.handles.ns_root
    }

    /// `Turbine.Object`, the root of every native class.
    ///
    /// # Errors
    ///
    /// A Lua error if the table is missing.
    pub fn object_class(&self) -> mlua::Result<Table> {
        self.handles.ns_root.raw_get("Object")
    }

    /// A native function. Its own errors reach Lua as plain strings without a position
    /// prefix; argument conversion errors read `bad argument #n to '<name>' (...)`.
    ///
    /// # Errors
    ///
    /// A Lua error if the function cannot be created.
    pub fn function<A, R, F>(&self, name: &str, f: F) -> mlua::Result<Function>
    where
        A: FromLuaMulti,
        R: IntoLuaMulti,
        F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
    {
        api_fn(self.lua, name, f)
    }

    /// A static function stored as `owner[name]` and returned (§8.1: arguments are
    /// positional from the first; a colon call passes `owner` as the first argument).
    ///
    /// # Errors
    ///
    /// A Lua error if the function cannot be created or stored.
    pub fn static_function<A, R, F>(
        &self,
        owner: &Table,
        name: &str,
        f: F,
    ) -> mlua::Result<Function>
    where
        A: FromLuaMulti,
        R: IntoLuaMulti,
        F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
    {
        let func = api_fn(self.lua, name, f)?;
        owner.raw_set(name, func.clone())?;
        Ok(func)
    }

    /// A class table `namespace[name]` without a native constructor (abstract or static
    /// classes), inheriting from `base` through `__index`.
    ///
    /// # Errors
    ///
    /// A Lua error if the tables cannot be created.
    pub fn class(
        &self,
        namespace: &Table,
        name: &str,
        base: Option<&Table>,
    ) -> mlua::Result<Table> {
        self.builder().class(namespace, name, base, None)
    }

    /// An instantiable class `namespace[name]`: its native `Constructor` binds a fresh
    /// payload from `make` to the instance (read it back with [`object_payload`]), and
    /// calling the class creates an instance (§8.1–§8.3).
    ///
    /// # Errors
    ///
    /// A Lua error if the tables cannot be created.
    pub fn native_class<P, M>(
        &self,
        namespace: &Table,
        name: &str,
        base: Option<&Table>,
        make: M,
    ) -> mlua::Result<Table>
    where
        P: Any + Send + Sync,
        M: Fn() -> P + Send + Sync + 'static,
    {
        self.builder().class(
            namespace,
            name,
            base,
            Some(Arc::new(move || {
                NativeKind::External(Payload(Arc::new(make())))
            })),
        )
    }

    /// A static function table `namespace[name]` with `IsA`.
    ///
    /// # Errors
    ///
    /// A Lua error if the table cannot be created.
    pub fn statics(&self, namespace: &Table, name: &str) -> mlua::Result<Table> {
        self.builder().statics(namespace, name)
    }

    /// An enumeration table `namespace[name]` (§8.4).
    ///
    /// # Errors
    ///
    /// A Lua error if the table cannot be created.
    pub fn enumeration(
        &self,
        namespace: &Table,
        name: &str,
        values: &[(&str, u32)],
    ) -> mlua::Result<()> {
        self.builder()
            .enumeration(namespace, name, values)
            .map(|_| ())
    }

    /// Run `hook` once when this apartment closes, after the `Unload` handlers (§6.3
    /// step 4: the layer destroys its native objects).
    pub fn on_close(&self, hook: impl FnOnce() + Send + 'static) {
        lock(&self.ctx.inner).close_hooks.push(Box::new(hook));
    }
}

/// An error for a native function to return: it reaches Lua as a plain string without a
/// position prefix (`plugin-runtime.md` §11.2).
#[must_use]
pub fn script_error(msg: impl Into<String>) -> mlua::Error {
    host_error(msg)
}

/// Bind `payload` to the instance table `obj` of `class` without touching its metatable
/// (§8.3 step 3), making it an API object.
///
/// # Errors
///
/// A Lua error if the binding cannot be stored.
pub fn bind_object<P: Any + Send + Sync>(
    lua: &Lua,
    obj: &Table,
    class: &Table,
    payload: P,
) -> mlua::Result<()> {
    bind_native(
        lua,
        obj,
        NativeKind::External(Payload(Arc::new(payload))),
        class.clone(),
    )
}

/// The payload of type `P` bound to `obj` by a [`NativeApi::native_class`] constructor or
/// [`bind_object`], if any.
///
/// # Errors
///
/// A Lua error if the binding table is missing.
pub fn object_payload<P: Any + Send + Sync>(
    lua: &Lua,
    obj: &Table,
) -> mlua::Result<Option<Arc<P>>> {
    Ok(match native_of(lua, obj)? {
        Some(NativeKind::External(Payload(p))) => p.downcast::<P>().ok(),
        _ => None,
    })
}

fn ctx_of(lua: &Lua) -> mlua::Result<Arc<ApartmentCtx>> {
    lua.app_data_ref::<Arc<ApartmentCtx>>()
        .map(|c| Arc::clone(&c))
        .ok_or_else(|| host_error("not a plugin apartment"))
}

/// Name of the apartment that owns `lua`, if it is one.
#[must_use]
pub fn apartment_name(lua: &Lua) -> Option<String> {
    ctx_of(lua).ok().map(|c| c.name.clone())
}

/// Fire `event` on `target` from inside a plugin call of the apartment that owns `lua`
/// (a UI notification delivered synchronously, `lua-plugin-api.md` §9.7): the dispatch of
/// §9.2, each handler protected, errors reported for the apartment. It runs inside the
/// current host call and shares its instruction budget.
///
/// # Errors
///
/// A Lua error when `lua` is not a plugin apartment or the handler field cannot be read.
pub fn fire_event(lua: &Lua, target: &Table, event: &str, args: Table) -> mlua::Result<()> {
    crate::apartment::dispatch_in(lua, target, event, args)
}

/// Report `error` for the apartment that owns `lua`, as a plugin error
/// (`lua-plugin-api.md` §6.6), with `context` saying what the layer was doing.
pub fn report_error(lua: &Lua, context: &str, error: &mlua::Error) {
    if let Ok(ctx) = ctx_of(lua) {
        crate::apartment::report_ctx(&ctx, None, context, error);
    }
}

/// The call-time check of `SetBackground(path)` (decision 0015 §4.4): the path must name
/// a file of the `Plugins/` tree (whole-tree index, case-insensitive, confined), and its
/// first 64 bytes must be a supported image header (`.png` and `.bmp` are refused). The
/// result is cached per path until the index is rebuilt.
///
/// # Errors
///
/// [`ImageCheckError`] for the plugin call to raise; a Lua error is never returned for a
/// missing file.
pub fn check_image(lua: &Lua, path: &str) -> Result<CheckedImage, ImageCheckError> {
    let Ok(ctx) = ctx_of(lua) else {
        return Err(ImageCheckError::NotFound(path.to_owned()));
    };
    let Ok(shared) = ctx.shared() else {
        return Err(ImageCheckError::NotFound(path.to_owned()));
    };
    lock(&shared.index).check_image(&shared.plugins_dir, path)
}

/// The game time (`Turbine.Engine.GetGameTime()`) as the host reports it, for layers that
/// time UI behaviour on the same frame clock.
#[must_use]
pub fn game_time(lua: &Lua) -> Option<f64> {
    let ctx = ctx_of(lua).ok()?;
    Some(ctx.shared().ok()?.host.game_time())
}
