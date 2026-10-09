// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.PluginManager` (static) and the immediate unload of an apartment.
//!
//! Spec: `lua-plugin-api.md` §6.2 (`LoadPlugin` queues), §6.3 (`UnloadScriptState` acts
//! at once, refused while the apartment runs), §6.5, §6.7 (rows); `plugin-runtime.md`
//! §8.3, §8.4 (unload timing).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::api_fn;

use crate::apartment::Apartment;
use crate::object::{Builder, NativeKind, PluginRow, native_of};
use crate::shared::{
    ApartmentCell, ApartmentCtx, Item, LoadOrigin, MAX_REQUEST_NAME, Shared, lock,
};

fn rows_table(lua: &Lua, rows: &[PluginRow], available: bool) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    for (i, r) in rows.iter().enumerate() {
        let t = lua.create_table()?;
        t.raw_set("Name", r.name.as_str())?;
        t.raw_set("Author", r.author.as_str())?;
        t.raw_set("Version", r.version.as_str())?;
        t.raw_set("Package", r.package.as_str())?;
        t.raw_set("ScriptState", r.apartment.as_str())?;
        if available {
            t.raw_set("Description", r.description.as_str())?;
            t.raw_set("Image", r.image.as_str())?;
        }
        out.raw_set(i + 1, t)?;
    }
    Ok(out)
}

pub(super) fn install(
    b: &Builder<'_>,
    ns_root: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let pm = b.statics(ns_root, "PluginManager")?;

    let c = Arc::clone(ctx);
    pm.raw_set(
        "GetAvailablePlugins",
        api_fn(lua, "GetAvailablePlugins", move |lua, ()| {
            let shared = c.shared()?;
            let rows: Vec<PluginRow> = {
                let st = lock(&shared.state);
                st.available
                    .iter()
                    .map(|a| crate::manager::row_of(&a.descriptor, &st.scaling))
                    .collect()
            };
            rows_table(lua, &rows, true)
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "GetLoadedPlugins",
        api_fn(lua, "GetLoadedPlugins", move |lua, ()| {
            let rows = lock(&c.shared()?.state).loaded.clone();
            rows_table(lua, &rows, false)
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "LoadPlugin",
        api_fn(lua, "LoadPlugin", move |_, name: Value| {
            let Value::String(name) = name else {
                return Ok(());
            };
            let name = name.to_string_lossy();
            if name.len() > MAX_REQUEST_NAME {
                return Ok(());
            }
            let shared = c.shared()?;
            let mut state = lock(&shared.state);
            // Only queued when an available plugin has that name and none of that name is
            // loaded (§6.2); otherwise nothing happens.
            let available = state
                .available
                .iter()
                .any(|a| a.descriptor.name.eq_ignore_ascii_case(&name));
            let loaded = state
                .loaded
                .iter()
                .any(|r| r.name.eq_ignore_ascii_case(&name));
            if available && !loaded {
                state.queue_request(Item::Load {
                    name,
                    origin: LoadOrigin::Script,
                });
            }
            Ok(())
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "UnloadScriptState",
        api_fn(lua, "UnloadScriptState", move |_, name: Value| {
            let Value::String(name) = name else { return Ok(()) };
            let name = name.to_string_lossy();
            if name.len() > MAX_REQUEST_NAME {
                return Ok(());
            }
            let shared = c.shared()?;
            match unload_now(&shared, &name) {
                Ok(()) => {}
                Err(UnloadRefused::Running) => {
                    tracing::debug!(apartment = %name, "UnloadScriptState refused: the script state is running");
                }
                Err(UnloadRefused::Unknown) => {}
            }
            Ok(())
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "RefreshAvailablePlugins",
        api_fn(lua, "RefreshAvailablePlugins", move |_, ()| {
            lock(&c.shared()?.state).queue_request(Item::Refresh);
            Ok(())
        })?,
    )?;
    let c = Arc::clone(ctx);
    pm.raw_set(
        "ShowOptions",
        api_fn(lua, "ShowOptions", move |lua, args: MultiValue| {
            if let Some(Value::Table(obj)) = args.front()
                && let Some(NativeKind::Plugin(row)) = native_of(lua, obj)?
            {
                c.shared()?.host.show_plugin_options(&row.name);
            }
            Ok(())
        })?,
    )?;
    Ok(())
}

/// Why an immediate unload did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnloadRefused {
    /// No apartment of that name.
    Unknown,
    /// Lua code of that apartment is running (§6.3 step 2).
    Running,
}

/// Unload apartment `name` now (§6.3 steps 1–5): refused while its code runs.
pub(crate) fn unload_now(shared: &Arc<Shared>, name: &str) -> Result<(), UnloadRefused> {
    let cell = lock(&shared.state)
        .cell_by_name(name)
        .ok_or(UnloadRefused::Unknown)?;
    unload_cell(shared, &cell)
}

/// Unload the apartment of `cell` now (§6.3 steps 1–5): refused while its code runs.
pub(crate) fn unload_cell(
    shared: &Arc<Shared>,
    cell: &Arc<ApartmentCell>,
) -> Result<(), UnloadRefused> {
    if cell.ctx.is_running() {
        return Err(UnloadRefused::Running);
    }
    let ap = match cell.slot.try_lock() {
        Ok(mut slot) => slot.take(),
        Err(_) => return Err(UnloadRefused::Running),
    };
    let Some(ap) = ap else {
        return Err(UnloadRefused::Unknown);
    };
    close_apartment(shared, cell, ap);
    Ok(())
}

/// Steps 3–5 of §6.3 for an apartment already taken out of its cell.
pub(crate) fn close_apartment(shared: &Arc<Shared>, cell: &Arc<ApartmentCell>, mut ap: Apartment) {
    // Step 3: `Unload` on each plugin in load order, then `Plugins[name] = nil`.
    let plugins = std::mem::take(&mut ap.plugins);
    for p in &plugins {
        match ap.lua().create_table() {
            Ok(args) => ap.fire_event(&p.object, "Unload", args, Some(&p.name)),
            Err(e) => ap.report(Some(&p.name), "unloading", &e),
        }
        ap.clear_plugins_entry(&p.name);
    }
    // Step 4: native objects: shell commands, and the embedding layers' objects.
    {
        let mut state = lock(&shared.state);
        state
            .loaded
            .retain(|r| crate::shared::apartment_key(&r.apartment) != ap.ctx.key);
    }
    // The apartment's names leave the host's command registry (`lua-plugin-api.md` §11.1:
    // registrations are removed when it unloads).
    shared.host.unregister_apartment_commands(&ap.ctx.key);
    let hooks = {
        let mut inner = lock(&ap.ctx.inner);
        inner.commands.clear();
        inner.command_names = 0;
        std::mem::take(&mut inner.close_hooks)
    };
    for hook in hooks {
        hook();
    }
    // Step 5: the state closes; `Save` writes synchronously from now on, and every write
    // still queued is flushed first.
    ap.ctx.closing.store(true, Ordering::SeqCst);
    super::plugin_data::flush_writes(&ap);
    lock(&shared.state)
        .apartments
        .retain(|c| !Arc::ptr_eq(c, cell));
    tracing::info!(
        apartment = %ap.ctx.name,
        memory_bytes = ap.state.used_memory(),
        peak_memory_bytes = ap.state.peak_memory(),
        "apartment closed"
    );
    drop(plugins);
    drop(ap);
}
