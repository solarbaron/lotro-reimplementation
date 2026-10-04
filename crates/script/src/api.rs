// SPDX-License-Identifier: GPL-3.0-or-later
//! Extension API for layers that add native packages (`Turbine.UI`, `Turbine.Gameplay`,
//! ...) to every apartment.
//!
//! The embedding client implements [`HostApi::install_natives`](crate::HostApi::install_natives);
//! the host calls it with a [`NativeApi`] for each new apartment before any plugin code
//! runs. Classes created here behave like the built-in ones: `IsA`, a native `Constructor`,
//! instantiation by calling the class, Lua subclassing (§8), and their instances count as
//! API objects (they cannot be saved with `PluginData.Save`, §10.5). Functions created here
//! raise plain Lua 5.1 string errors. Events are fired with
//! [`PluginManager::fire_event`](crate::PluginManager::fire_event).
//!
//! Spec: `docs/specs/lua-plugin-api.md` §7.4 (native packages), §8 (object model), §9
//! (events).

use std::any::Any;
use std::sync::Arc;

use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua, Table};

use crate::runtime::{
    ApartmentCtx, NativeKind, Payload, bind_native, host_error, native_fn, native_of,
};
use crate::turbine::{Builder, Handles, static_fn};

/// Access to one new apartment while its native packages are installed.
pub struct NativeApi<'a> {
    lua: &'a Lua,
    ctx: &'a ApartmentCtx,
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
    pub(crate) fn new(lua: &'a Lua, ctx: &'a ApartmentCtx, handles: &'a Handles) -> Self {
        Self { lua, ctx, handles }
    }

    fn builder(&self) -> Builder<'a> {
        Builder {
            lua: self.lua,
            isa: self.handles.isa.clone(),
            instantiate: self.handles.instantiate.clone(),
        }
    }

    /// The apartment's Lua state.
    pub fn lua(&self) -> &Lua {
        self.lua
    }

    /// Name of the apartment being created.
    pub fn apartment(&self) -> &str {
        &self.ctx.name
    }

    /// The `Turbine` namespace table (it already holds the empty `UI`, `UI.Lotro`,
    /// `Gameplay` and `Gameplay.Attributes` tables).
    pub fn turbine(&self) -> &Table {
        &self.handles.turbine
    }

    /// `Turbine.Object`, the root of every native class.
    pub fn object_class(&self) -> mlua::Result<Table> {
        self.handles.turbine.raw_get("Object")
    }

    /// A native function. Errors reach Lua as strings prefixed with the caller's
    /// `chunk:line:`, like Lua 5.1 C functions; argument conversion errors read
    /// `bad argument #n to '<name>' (...)`.
    pub fn function<A, R, F>(&self, name: &str, f: F) -> mlua::Result<Function>
    where
        A: FromLuaMulti,
        R: IntoLuaMulti,
        F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
    {
        native_fn(self.lua, name, f)
    }

    /// A static function of `owner` that also accepts colon calls (`owner:Name(...)`,
    /// §8.1): a leading argument equal to `owner` is dropped before conversion.
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
        static_fn(self.lua, owner, name, f)
    }

    /// A class table `namespace[name]` without a native constructor (abstract or static
    /// classes), inheriting from `base` through `__index`.
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
    pub fn statics(&self, namespace: &Table, name: &str) -> mlua::Result<Table> {
        self.builder().statics(namespace, name)
    }

    /// An enumeration table `namespace[name]` (§8.4).
    pub fn enumeration(
        &self,
        namespace: &Table,
        name: &str,
        values: &[(&str, u32)],
    ) -> mlua::Result<()> {
        self.builder().enumeration(namespace, name, values)
    }
}

/// An error for a native function to return: it reaches Lua as a string with the calling
/// script's `chunk:line:` prefix (use it rather than `mlua::Error::runtime`, whose text is
/// passed through as if Lua code had raised it).
pub fn script_error(msg: impl Into<String>) -> mlua::Error {
    host_error(msg)
}

/// Bind `payload` to the instance table `obj` of `class` without touching its metatable
/// (§8.3 step 3), making it an API object.
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
pub fn object_payload<P: Any + Send + Sync>(
    lua: &Lua,
    obj: &Table,
) -> mlua::Result<Option<Arc<P>>> {
    Ok(match native_of(lua, obj)? {
        Some(NativeKind::External(Payload(p))) => p.downcast::<P>().ok(),
        _ => None,
    })
}
