// SPDX-License-Identifier: GPL-3.0-or-later
//! `import` and package environments.
//!
//! Spec: `lua-plugin-api.md` §7 (call form, resolution, namespaces, run once, running a
//! disk chunk, native packages); `plugin-descriptor.md` §7 (grammar, disk resolution and
//! precedence); `plugin-runtime.md` §6 (environments, `getfenv(0)`); `plugin-class-helper.md`
//! §5 (built-in packages).

use std::sync::Arc;

use mlua::{Lua, Table, Value};
use tessera_script::{api_fn, call_protected, compile_source, format_number_g14, host_error};

use crate::packages::{PackageContext, PackageShape};
use crate::paths::{is_valid_package_name, resolve_package};
use crate::shared::{ApartmentCtx, lock};

/// Registry name of the apartment's real global table.
pub(crate) const GLOBALS_KEY: &str = "tessera.plugins.globals";

/// Native package names (§7.4), matched exactly.
pub(crate) const NATIVE_PACKAGES: [&str; 5] = [
    "Turbine",
    "Turbine.Gameplay",
    "Turbine.Gameplay.Attributes",
    "Turbine.UI",
    "Turbine.UI.Lotro",
];

/// The real global table of the apartment.
pub(crate) fn real_globals(lua: &Lua) -> mlua::Result<Table> {
    lua.named_registry_value(GLOBALS_KEY)
}

/// Register the global `import` (§7.1): no argument raises; a string is used as is; a
/// number becomes its `%.14g` text; any other value is the empty name.
pub(crate) fn install(lua: &Lua, ctx: &Arc<ApartmentCtx>) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    let import = api_fn(lua, "import", move |lua, args: mlua::MultiValue| {
        let name = match args.front() {
            None => return Err(host_error("import: no package name given")),
            Some(Value::String(s)) => s.to_string_lossy(),
            Some(Value::Integer(i)) => format_number_g14(*i as f64),
            Some(Value::Number(n)) => format_number_g14(*n),
            Some(_) => String::new(),
        };
        import_package(lua, &c, &name)
    })?;
    real_globals(lua)?.raw_set("import", import)
}

/// The disk part of a package: a file of the plugin tree or a built-in install.
enum DiskPart {
    File(std::path::PathBuf),
    Builtin,
}

/// The `import` algorithm of §7.2.
pub(crate) fn import_package(lua: &Lua, ctx: &ApartmentCtx, name: &str) -> mlua::Result<()> {
    if lock(&ctx.inner).packages_run.contains(name) {
        return Ok(());
    }
    let shared = ctx.shared()?;
    let native = NATIVE_PACKAGES.contains(&name);
    let comps: Vec<&str> = name.split('.').collect();
    let mut disk: Option<(DiskPart, usize)> = None;
    if is_valid_package_name(name) {
        let builtin = if ctx.builtin_mode {
            shared.builtins.as_ref().and_then(|b| b.shape(name))
        } else {
            None
        };
        if let Some(shape) = builtin {
            let ns_len = match shape {
                PackageShape::Folder => comps.len(),
                PackageShape::File => comps.len() - 1,
            };
            disk = Some((DiskPart::Builtin, ns_len));
        } else if let Some(p) = resolve_package(&shared.plugins_dir, name) {
            let ns_len = p.namespace_len(comps.len());
            disk = Some((DiskPart::File(p.file), ns_len));
        }
    }
    if !native && disk.is_none() {
        return Err(host_error(format!(
            "import: cannot resolve package '{name}'"
        )));
    }
    // Namespace: the whole name for a native or folder package, all but the last
    // component for a file package (merged packages use the native rule).
    let ns_len = if native {
        comps.len()
    } else {
        disk.as_ref().map_or(comps.len(), |(_, n)| *n)
    };
    let globals = real_globals(lua)?;
    let mut ns = globals.clone();
    for c in &comps[..ns_len] {
        ns = match ns.raw_get::<Value>(*c)? {
            Value::Table(t) => t,
            Value::Nil => {
                let t = lua.create_table()?;
                ns.raw_set(*c, t.clone())?;
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

    // Run once: record first, so a package that imports itself does not recurse.
    lock(&ctx.inner).packages_run.insert(name.to_owned());
    let result = (|| {
        let marker = lua.create_table()?;
        marker.raw_set("Name", name)?;
        ns.raw_set("_", marker)?;
        match disk {
            None => Ok(()),
            Some((DiskPart::File(path), _)) => {
                run_disk_chunk(lua, ctx, &shared.plugins_dir, &path, &ns, &globals, &shared)
            }
            Some((DiskPart::Builtin, _)) => run_builtin(lua, ctx, name, &ns, &globals, &shared),
        }
    })();
    if result.is_err() {
        lock(&ctx.inner).packages_run.remove(name);
    }
    result
}

/// Steps 1, 2, 4 and 5 of "Running a disk chunk" around `body` (step 3).
fn with_namespace<R>(
    lua: &Lua,
    ns: &Table,
    globals: &Table,
    body: impl FnOnce() -> mlua::Result<R>,
) -> mlua::Result<R> {
    let distinct = ns.to_pointer() != globals.to_pointer();
    let previous = if distinct {
        let previous = ns.metatable();
        let mt = lua.create_table()?;
        mt.raw_set("__index", globals.clone())?;
        ns.set_metatable(Some(mt))?;
        previous
    } else {
        None
    };
    lua.set_globals(ns.clone())?;
    let result = body();
    // Step 4: the real global table again, whatever happened.
    let restored = lua.set_globals(globals.clone());
    if let Some(prev) = previous {
        ns.set_metatable(Some(prev))?;
    }
    restored?;
    result
}

fn run_disk_chunk(
    lua: &Lua,
    ctx: &ApartmentCtx,
    root: &std::path::Path,
    path: &std::path::Path,
    ns: &Table,
    globals: &Table,
    shared: &crate::shared::Shared,
) -> mlua::Result<()> {
    let len = std::fs::metadata(path)
        .map_err(|e| host_error(format!("import: cannot open '{}': {e}", path.display())))?
        .len();
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if len > shared.limits.max_source_bytes {
        return Err(host_error(format!(
            "import: '{rel}' is larger than {} bytes",
            shared.limits.max_source_bytes
        )));
    }
    let bytes =
        std::fs::read(path).map_err(|e| host_error(format!("import: cannot read '{rel}': {e}")))?;
    tracing::debug!(apartment = %ctx.name, file = %rel, "import");
    with_namespace(lua, ns, globals, || {
        let f = compile_source(
            lua,
            &format!("@{rel}"),
            &bytes,
            shared.limits.max_source_bytes,
            Some(ns.clone()),
        )
        .map_err(|e| match e {
            tessera_script::CoreError::Lua(e) => e,
            tessera_script::CoreError::BinaryChunk(n) => host_error(format!(
                "import: '{n}' is a binary chunk; only source files are allowed"
            )),
            other => host_error(format!("import: {other}")),
        })?;
        // Through the host-call trampoline (without resetting the budget): the error value
        // of a failing chunk is normalised there (a string or number as text, anything
        // else the empty message), and no `__tostring` of a script value runs
        // (`plugin-runtime.md` §11.1).
        call_protected::<_, ()>(lua, &Value::Function(f), ())
    })
}

fn run_builtin(
    lua: &Lua,
    ctx: &ApartmentCtx,
    name: &str,
    ns: &Table,
    globals: &Table,
    shared: &crate::shared::Shared,
) -> mlua::Result<()> {
    let Some(provider) = shared.builtins.clone() else {
        return Ok(());
    };
    let import = |n: &str| import_package(lua, ctx, n);
    with_namespace(lua, ns, globals, || {
        let pctx = PackageContext {
            lua,
            namespace: ns.clone(),
            globals: globals.clone(),
            import: &import,
        };
        provider.install(&pctx, name)
    })
}
