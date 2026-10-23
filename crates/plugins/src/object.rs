// SPDX-License-Identifier: GPL-3.0-or-later
//! The object model: class tables, instances bound to native objects, enumerations.
//!
//! Spec: `lua-plugin-api.md` §8 (class tables, instances, subclassing from Lua,
//! enumerations, writable class tables).

use std::any::Any;
use std::sync::Arc;

use mlua::{Function, Lua, MultiValue, Table, UserData, Value};
use tessera_script::{api_fn, host_error};

/// Registry name of the weak-keyed table binding instance tables to native objects.
pub(crate) const NATIVES_KEY: &str = "tessera.plugins.natives";

/// Registry name of the weak-keyed table giving the class table each native object was
/// constructed as (for `IsA`). The class lives on the Lua side, so a native object holds
/// no host-side handle to a Lua value: the number of live objects is bounded by memory,
/// not by the host's handle stack (about 8,000 slots in Lua 5.1).
pub(crate) const NATIVE_CLASSES_KEY: &str = "tessera.plugins.native_classes";

/// Longest `IsA` chain walked before giving up (guards against `__index` cycles).
const MAX_CHAIN: usize = 256;

/// Information about one plugin, as scripts see it through `Turbine.Plugin` and
/// `PluginManager` rows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PluginRow {
    pub name: String,
    pub author: String,
    pub version: String,
    pub package: String,
    pub description: String,
    pub image: String,
    pub apartment: String,
    pub uses_scaling: bool,
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
    /// An object of a class installed by an embedding layer (§7.4 extension point).
    External(Payload),
}

/// The opaque userdata stored in the hidden natives table. Its class table is kept in
/// the apartment's [`NATIVE_CLASSES_KEY`] table, never here.
#[derive(Debug)]
pub(crate) struct Native {
    pub kind: NativeKind,
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
    let classes: Table = lua.named_registry_value(NATIVE_CLASSES_KEY)?;
    match classes.raw_get::<Value>(obj)? {
        Value::Table(class) => Ok(Some(class)),
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
    let classes: Table = lua.named_registry_value(NATIVE_CLASSES_KEY)?;
    let ud = lua.create_userdata(Native { kind })?;
    classes.raw_set(obj, class)?;
    natives.raw_set(obj, ud)
}

/// Create the hidden natives and native-classes tables (weak keys) of a new apartment.
pub(crate) fn install_natives_table(lua: &Lua) -> mlua::Result<()> {
    for key in [NATIVES_KEY, NATIVE_CLASSES_KEY] {
        let t = lua.create_table()?;
        let weak = lua.create_table()?;
        weak.raw_set("__mode", "k")?;
        t.set_metatable(Some(weak))?;
        lua.set_named_registry_value(key, t)?;
    }
    Ok(())
}

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

/// Creates the native object bound by a class `Constructor`.
pub(crate) type KindFactory = Arc<dyn Fn() -> NativeKind + Send + Sync>;

/// Builds class, static and enumeration tables of one apartment.
#[derive(Debug, Clone)]
pub(crate) struct Builder<'a> {
    pub lua: &'a Lua,
    pub isa: Function,
    pub instantiate: Function,
}

impl<'a> Builder<'a> {
    /// The shared `IsA` and class-call functions of a new apartment.
    pub(crate) fn new(lua: &'a Lua) -> mlua::Result<Self> {
        let instantiate = api_fn(lua, "__call", |lua, mut args: MultiValue| {
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
        Ok(Self {
            lua,
            isa: api_fn(lua, "IsA", is_a)?,
            instantiate,
        })
    }

    /// A class table (§8.1): `IsA`, metatable `__index` = base; instantiable classes also
    /// get a native `Constructor` binding `kind` and a `__call` that creates instances.
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
            let ctor = api_fn(self.lua, "Constructor", move |lua, args: MultiValue| {
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
    ) -> mlua::Result<Table> {
        let t = self.lua.create_table()?;
        for (k, v) in values {
            t.raw_set(*k, f64::from(*v))?;
        }
        t.raw_set("IsA", self.isa.clone())?;
        ns.raw_set(name, t.clone())?;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twenty thousand live native objects, each bound with its own fresh handle of the
    /// class table (as the gameplay views do), keep no host handle alive: binding never
    /// runs out of the host's handle stack, and `IsA` still answers from the class.
    #[test]
    fn many_live_natives_hold_no_host_handles() {
        let lua = Lua::new();
        install_natives_table(&lua).unwrap();
        let base = lua.create_table().unwrap();
        let cls = lua.create_table().unwrap();
        let mt = lua.create_table().unwrap();
        mt.raw_set("__index", base.clone()).unwrap();
        cls.set_metatable(Some(mt)).unwrap();
        let other = lua.create_table().unwrap();
        lua.set_named_registry_value("probe.class", cls).unwrap();
        let mut last = None;
        for i in 1..=20_000 {
            // A fresh handle each time, dropped at the end of the iteration.
            let class: Table = lua.named_registry_value("probe.class").unwrap();
            let obj = lua.create_table().unwrap();
            bind_native(&lua, &obj, NativeKind::Object, class).unwrap();
            lua.globals().raw_set(format!("o{i}"), obj.clone()).unwrap();
            last = Some(obj);
        }
        let last = last.unwrap();
        let isa = |class: &Table| {
            is_a(
                &lua,
                (Value::Table(last.clone()), Value::Table(class.clone())),
            )
            .unwrap()
        };
        let r = (isa(&base), isa(&other));
        assert_eq!(r, (true, false));
    }
}
