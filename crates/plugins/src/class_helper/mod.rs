// SPDX-License-Identifier: GPL-3.0-or-later
//! The built-in, clean-room plugin `class` helper and the sample utilities plugins depend on
//! (`plugin-class-helper.md`, decision 0016). A user-installed original package takes
//! precedence over it.
//!
//! [`ClassHelper`] is a [`BuiltinPackages`] provider. Give it to the host through
//! [`ManagerConfig::builtins`](crate::ManagerConfig::builtins) (or
//! [`ClassHelper::provider`]); the host consults it only for apartments in **built-in
//! mode**, that is when the user's `Plugins/` folder has no `Turbine` directory and no
//! `Turbine.lua` file (§5.1). In user-copy mode the user's files run instead and this
//! module does nothing (§5.2).
//!
//! In built-in mode the provider serves the fifteen import names of §5.3. The root
//! (`Turbine`, imported at apartment creation, §5.4) brings in the global `class`,
//! `math.round`, `Turbine.Type`, `Turbine.Table`, `Turbine.Log`, `Turbine.Debug` and the
//! native `Turbine.UI`; `Turbine.UI.Extensions` and `Turbine.Animation` stay on demand.
//! Names the spec lists as not provided (§8 K1, K10) are never defined.
//!
//! The package itself is Lua source written for Tessera (`class.lua`, `utilities.lua`,
//! embedded in the crate). It is compiled once per apartment, on the first install, with
//! an empty environment and a table of primitives taken from the apartment's globals at
//! that moment (before any plugin code runs) plus three host functions: the unique-ID
//! text of a table (`plugin-runtime.md` §4.4), a byte-order string comparison (the table
//! dump's sort, §7.3) and the local time of day (the log line, §7.7, from
//! [`HostApi::engine_date`](crate::HostApi::engine_date)). Its class registry (§6.13)
//! lives in locals of that chunk, out of reach of plugin code.
//!
//! Spec: `plugin-class-helper.md` §5–§7; `lua-plugin-api.md` §7.2.

use std::sync::Arc;

use mlua::{Function, Lua, Table, Value};
use tessera_script::{call_protected, compile_source, host_error};

use crate::host::EngineDate;
use crate::packages::{BuiltinPackages, PackageContext, PackageShape};
use crate::shared::ApartmentCtx;

/// Registry name of an apartment's compiled package (its install functions).
const CORE_KEY: &str = "tessera.plugins.class_helper";

/// Part 1: `class`, classes, type objects (§6).
const CLASS_SOURCE: &str = include_str!("class.lua");
/// Part 2: the utilities and the install steps (§5.3, §7).
const UTILITIES_SOURCE: &str = include_str!("utilities.lua");

/// Base functions the package captures from the apartment's globals.
const BASE_FUNCTIONS: [&str; 9] = [
    "type",
    "select",
    "error",
    "rawequal",
    "rawset",
    "pairs",
    "setmetatable",
    "getfenv",
    "tostring",
];

/// One built-in import name (§5.3).
struct Entry {
    /// The name as plugins spell it (matched ASCII case-insensitively).
    name: &'static str,
    shape: PackageShape,
    /// Imports performed before the install.
    before: &'static [&'static str],
    /// The install function of `utilities.lua`, if the name installs anything.
    install: Option<&'static str>,
    /// Imports performed after the install.
    after: &'static [&'static str],
}

const fn entry(
    name: &'static str,
    shape: PackageShape,
    before: &'static [&'static str],
    install: Option<&'static str>,
    after: &'static [&'static str],
) -> Entry {
    Entry {
        name,
        shape,
        before,
        install,
        after,
    }
}

use PackageShape::{File, Folder};

/// The table of §5.3, with the root's import order of §5.4.
const ENTRIES: [Entry; 15] = [
    entry(
        "Turbine",
        Folder,
        &[
            "Turbine.Class",
            "Turbine.Debug",
            "Turbine.Log",
            "Turbine.MathUtils",
            "Turbine.Table",
            "Turbine.UI",
        ],
        None,
        &[],
    ),
    entry("Turbine.Class", File, &[], Some("class"), &["Turbine.Type"]),
    entry("Turbine.Type", File, &["Turbine.Class"], Some("type"), &[]),
    entry("Turbine.MathUtils", File, &[], Some("math_round"), &[]),
    entry("Turbine.Table", File, &[], Some("table"), &[]),
    entry("Turbine.Log", File, &["Turbine.Class"], Some("log"), &[]),
    entry(
        "Turbine.Debug",
        Folder,
        &["Turbine.Debug.Stack", "Turbine.Debug.Table"],
        None,
        &[],
    ),
    entry(
        "Turbine.Debug.Stack",
        File,
        &["Turbine"],
        Some("debug_stack"),
        &[],
    ),
    entry(
        "Turbine.Debug.Table",
        File,
        &["Turbine"],
        Some("debug_table"),
        &[],
    ),
    entry(
        "Turbine.Animation",
        Folder,
        &["Turbine.Animation.LerpValue"],
        None,
        &[],
    ),
    entry(
        "Turbine.Animation.LerpValue",
        File,
        &["Turbine"],
        Some("lerp_value"),
        &[],
    ),
    entry(
        "Turbine.UI.Extensions",
        Folder,
        &[
            "Turbine.UI.Extensions.SimpleWindow",
            "Turbine.UI.Extensions.Window",
            "Turbine.UI.Lotro.Quickslot",
        ],
        None,
        &[],
    ),
    entry(
        "Turbine.UI.Extensions.SimpleWindow",
        File,
        &["Turbine", "Turbine.UI", "Turbine.UI.Lotro"],
        Some("simple_window"),
        &[],
    ),
    entry(
        "Turbine.UI.Extensions.Window",
        File,
        &["Turbine", "Turbine.UI", "Turbine.UI.Lotro"],
        Some("window"),
        &[],
    ),
    entry(
        "Turbine.UI.Lotro.Quickslot",
        File,
        &["Turbine.UI.Lotro"],
        Some("quickslot"),
        &[],
    ),
];

fn find_entry(name: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.name.eq_ignore_ascii_case(name))
}

/// The built-in class helper package (decision 0016) as a [`BuiltinPackages`] provider.
///
/// Stateless: each apartment keeps its own compiled copy of the package and its own class
/// registry, so one provider serves every apartment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassHelper;

impl ClassHelper {
    /// The provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The provider, ready for [`ManagerConfig::builtins`](crate::ManagerConfig::builtins).
    #[must_use]
    pub fn provider() -> Arc<dyn BuiltinPackages> {
        Arc::new(Self)
    }

    /// The import names served in built-in mode, in the order of
    /// `plugin-class-helper.md` §5.3.
    pub fn names() -> impl Iterator<Item = &'static str> {
        ENTRIES.iter().map(|e| e.name)
    }
}

impl BuiltinPackages for ClassHelper {
    fn shape(&self, name: &str) -> Option<PackageShape> {
        find_entry(name).map(|e| e.shape)
    }

    fn install(&self, ctx: &PackageContext<'_>, name: &str) -> mlua::Result<()> {
        let Some(entry) = find_entry(name) else {
            return Err(host_error(format!(
                "import: '{name}' is not a built-in package"
            )));
        };
        // The imports a package performs use the fixed spellings of its own source, so a
        // case variant of a folder name still fills the usual namespaces (§5.3).
        for dep in entry.before {
            ctx.import(dep)?;
        }
        if let Some(key) = entry.install {
            let installs = package(ctx)?;
            let f: Function = installs.raw_get(key)?;
            call_protected::<_, ()>(ctx.lua(), &Value::Function(f), ctx.namespace().clone())?;
        }
        for dep in entry.after {
            ctx.import(dep)?;
        }
        Ok(())
    }
}

/// The apartment's compiled package, compiled on first use.
fn package(ctx: &PackageContext<'_>) -> mlua::Result<Table> {
    let lua = ctx.lua();
    if let Value::Table(t) = lua.named_registry_value::<Value>(CORE_KEY)? {
        return Ok(t);
    }
    let primitives = primitives(lua, ctx.globals())?;
    let class_part = run_chunk(lua, "=class helper", CLASS_SOURCE, primitives.clone())?;
    let installs = run_chunk(
        lua,
        "=class helper utilities",
        UTILITIES_SOURCE,
        (primitives, class_part),
    )?;
    lua.set_named_registry_value(CORE_KEY, installs.clone())?;
    Ok(installs)
}

fn run_chunk(
    lua: &Lua,
    name: &str,
    source: &str,
    args: impl mlua::IntoLuaMulti,
) -> mlua::Result<Table> {
    let env = lua.create_table()?;
    let f =
        compile_source(lua, name, source.as_bytes(), u64::MAX, Some(env)).map_err(|e| match e {
            tessera_script::CoreError::Lua(e) => e,
            other => host_error(other.to_string()),
        })?;
    call_protected(lua, &Value::Function(f), args)
}

/// The primitives table both chunks receive.
fn primitives(lua: &Lua, globals: &Table) -> mlua::Result<Table> {
    let p = lua.create_table()?;
    for name in BASE_FUNCTIONS {
        p.raw_set(name, globals.raw_get::<Value>(name)?)?;
    }
    let library = |lib: &str, name: &str, key: &str| -> mlua::Result<()> {
        if let Value::Table(t) = globals.raw_get::<Value>(lib)? {
            p.raw_set(key, t.raw_get::<Value>(name)?)?;
        }
        Ok(())
    };
    library("math", "floor", "floor")?;
    library("string", "find", "find")?;
    library("string", "sub", "sub")?;
    library("table", "sort", "sort")?;
    p.raw_set("G", globals.clone())?;
    p.raw_set(
        "uid",
        lua.create_function(|_, t: Table| Ok(unique_id(t.to_pointer())))?,
    )?;
    p.raw_set(
        "byte_less",
        lua.create_function(|_, (a, b): (mlua::LuaString, mlua::LuaString)| {
            Ok(a.as_bytes()[..] < b.as_bytes()[..])
        })?,
    )?;
    p.raw_set(
        "clock",
        lua.create_function(|lua, ()| Ok(time_of_day(&local_date(lua))))?,
    )?;
    Ok(p)
}

/// The text a table's address gives in `tostring` without the `table: ` prefix: 16
/// upper-case hexadecimal digits (`plugin-runtime.md` §4.4, row R5).
fn unique_id(ptr: *const std::ffi::c_void) -> String {
    format!("{:016X}", ptr as usize)
}

/// The host's local date and time (`HostApi::engine_date`), or UTC outside a plugin host.
fn local_date(lua: &Lua) -> EngineDate {
    let shared = lua
        .app_data_ref::<Arc<ApartmentCtx>>()
        .and_then(|c| c.shared().ok());
    shared.map_or_else(EngineDate::utc_now, |s| s.host.engine_date())
}

/// `hh:mm:ss` on a 24-hour clock (§7.7).
fn time_of_day(d: &EngineDate) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        d.hour.min(23),
        d.minute.min(59),
        d.second.min(60)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_shapes() {
        let h = ClassHelper::new();
        assert_eq!(ClassHelper::names().count(), 15);
        assert_eq!(h.shape("Turbine"), Some(PackageShape::Folder));
        assert_eq!(h.shape("tUrBiNe.ui.EXTENSIONS"), Some(PackageShape::Folder));
        assert_eq!(
            h.shape("Turbine.UI.Lotro.Quickslot"),
            Some(PackageShape::File)
        );
        assert_eq!(h.shape("Turbine.Debug"), Some(PackageShape::Folder));
        for missing in [
            "Turbine.UI",
            "Turbine.UI.Lotro",
            "Turbine.Utils",
            "Turbine.Event",
            "Turbine.Timer",
            "Turbine.Testing",
            "Turbine.Mixin",
            "Turbine.String",
            "Turbine.Class.",
            "",
        ] {
            assert_eq!(h.shape(missing), None, "{missing}");
        }
    }

    #[test]
    fn every_import_names_a_served_or_native_package() {
        for e in &ENTRIES {
            for dep in e.before.iter().chain(e.after) {
                assert!(
                    find_entry(dep).is_some() || crate::import::NATIVE_PACKAGES.contains(dep),
                    "{dep}"
                );
            }
            if e.shape == PackageShape::Folder {
                assert!(e.install.is_none(), "{}", e.name);
            }
        }
    }

    #[test]
    fn install_keys_exist_in_the_source() {
        for e in &ENTRIES {
            if let Some(key) = e.install {
                assert!(
                    UTILITIES_SOURCE.contains(&format!("function installs.{key}(")),
                    "{key}"
                );
            }
        }
    }

    #[test]
    fn text_forms() {
        assert_eq!(unique_id(0x1234usize as *const _), "0000000000001234");
        let d = EngineDate {
            hour: 7,
            minute: 5,
            second: 9,
            ..EngineDate::default()
        };
        assert_eq!(time_of_day(&d), "07:05:09");
    }
}
