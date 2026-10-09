// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.PluginData` (static): synchronous `Load` while a plugin loads, asynchronous
//! `Load` and queued `Save` otherwise, synchronous `Save` while the apartment closes.
//!
//! Every physical write goes through the FIFO writer of [`crate::data_io`]: a queued
//! write's step-7 item hands its bytes to the writer (on a host worker), and its `onDone`
//! callbacks run as a later step-7 item once the write is done; writes of a closing
//! apartment are written on the calling thread after every earlier write.
//!
//! Spec: `lua-plugin-api.md` §10.1 (calls), §10.2 (validation), §10.6 (timing, the write
//! step); `plugin-runtime.md` §8.4 (deferred work).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use mlua::{Function, Lua, MultiValue, Table, Value};
use tessera_script::{api_fn, call_protected, format_number_g14, host_error};

use crate::apartment::report_ctx;
use crate::host::DataScope;
use crate::object::Builder;
use crate::plugindata::{self, PluginDataError, data_to_lua, serialize};
use crate::shared::{
    ApartmentCtx, Item, MAX_DATA_ITEMS, MAX_WRITE_CALLBACKS, Shared, lock, max_pending_write_bytes,
};

/// The message `onDone` receives when a write failed (Tessera's own wording; the
/// original's fixed text is not reproduced, §10.6).
pub(crate) const SAVE_FAILED: &str = "the plugin data could not be saved";

/// Budget units charged per byte a `Load` reads and parses or a `Save` serializes (about
/// the cost of a VM instruction, `lua-plugin-api.md` §3.3).
const BYTE_COST: u64 = 1;

/// Validate `(scope, key)` and find the file (§10.2, §10.3); errors are raised in Lua.
fn data_target(
    ctx: &ApartmentCtx,
    func: &str,
    scope: &Value,
    key: &Value,
) -> mlua::Result<PathBuf> {
    let raise = |e: PluginDataError| host_error(format!("PluginData.{func}: {e}"));
    let scope = match scope {
        Value::Integer(i) => DataScope::from_number(*i as f64),
        Value::Number(n) => DataScope::from_number(*n),
        Value::String(s) => s
            .to_str()
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .and_then(DataScope::from_number),
        _ => None,
    }
    .ok_or_else(|| raise(PluginDataError::InvalidScope))?;
    // The key as text: a number becomes its string form, anything else but a string the
    // empty key (§10.2).
    let key_bytes: Vec<u8> = match key {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Integer(i) => format_number_g14(*i as f64).into_bytes(),
        Value::Number(n) => format_number_g14(*n).into_bytes(),
        _ => Vec::new(),
    };
    let key = plugindata::validate_key(&key_bytes).map_err(raise)?;
    let dir = ctx
        .shared()?
        .host
        .plugin_data_dir(scope)
        .ok_or_else(|| raise(PluginDataError::ScopeUnavailable))?;
    Ok(plugindata::file_path(&dir, key))
}

/// Read the value stored at `path` now: the latest save of that file not yet on disk if
/// there is one, else the file. A missing or unreadable file is nil (an unreadable one is
/// logged; Tessera never runs the file, §10.4). The bytes read and parsed are charged to
/// the caller's instruction budget.
fn read_now(lua: &Lua, ctx: &ApartmentCtx, shared: &Shared, path: &Path) -> mlua::Result<Value> {
    let limits = &shared.limits.data;
    let bytes: Result<Option<Arc<[u8]>>, PluginDataError> = match shared.io.latest(path) {
        Some(b) => Ok(Some(b)),
        None => plugindata::read_path(path, limits).map(|b| b.map(Arc::from)),
    };
    if let Ok(Some(b)) = &bytes {
        ctx.budget.charge(b.len() as u64 * BYTE_COST)?;
    }
    let read = bytes.and_then(|b| match b {
        Some(b) => plugindata::parse(&b, limits)
            .map(Some)
            .map_err(PluginDataError::from),
        None => Ok(None),
    });
    match read {
        Ok(Some(v)) => data_to_lua(lua, &v),
        Ok(None) => Ok(Value::Nil),
        Err(e) => {
            tracing::warn!(apartment = %ctx.name, path = %path.display(), "plugin data unreadable: {e}");
            shared.host.script_log(
                &ctx.name,
                &format!("PluginData.Load({}): {e}", path.display()),
            );
            Ok(Value::Nil)
        }
    }
}

/// The `(success, message)` of a write's `onDone` (§10.6).
pub(crate) fn done_message(ok: bool) -> &'static str {
    if ok { "" } else { SAVE_FAILED }
}

/// Call `onDone(success, message)` callbacks of a write in the apartment of `ctx`.
fn call_done(lua: &Lua, ctx: &ApartmentCtx, callbacks: &[Function], ok: bool) {
    for cb in callbacks {
        let args = match lua.create_string(done_message(ok)) {
            Ok(m) => MultiValue::from_vec(vec![Value::Boolean(ok), Value::String(m)]),
            Err(e) => {
                report_ctx(ctx, None, "plugin data save callback", &e);
                continue;
            }
        };
        let f = Value::Function(cb.clone());
        if let Err(e) = call_protected::<_, ()>(lua, &f, args) {
            report_ctx(ctx, None, "plugin data save callback", &e);
        }
    }
}

/// Move the results of finished worker writes into the deferred-work queue as
/// [`Item::WriteDone`] items, in write order.
pub(crate) fn collect_write_results(shared: &Shared) {
    let done = shared.io.take_done();
    if done.is_empty() {
        return;
    }
    let mut st = lock(&shared.state);
    for (seq, ok) in done {
        if let Some((apartment, callbacks)) = st.awaiting.remove(&seq) {
            st.queue.push_back(Item::WriteDone {
                apartment,
                callbacks,
                ok,
            });
        }
    }
}

/// Write every plugin-data write of the apartment before it closes (§10.6: data saved in
/// `Unload` handlers must reach disk), calling the `onDone` callbacks while the apartment
/// still exists: first the writes a worker already has (waited for), then the queued
/// ones, synchronously and in queue order. Queued `Load` handlers are dropped.
pub(crate) fn flush_writes(ap: &crate::apartment::Apartment) {
    let Ok(shared) = ap.ctx.shared() else { return };
    let mine = {
        let st = lock(&shared.state);
        st.awaiting
            .iter()
            .filter(|(_, (a, _))| *a == ap.ctx.id)
            .map(|(seq, _)| *seq)
            .max()
    };
    if let Some(seq) = mine {
        shared.io.run_until(seq);
    }
    collect_write_results(&shared);
    let io = Arc::clone(&shared.io);
    let items = lock(&shared.state).take_data_items(ap.ctx.id, &|p, n| io.drop_save(p, n));
    // Results of worker writes are older than every write still queued.
    let (done, queued): (Vec<Item>, Vec<Item>) = items
        .into_iter()
        .partition(|i| matches!(i, Item::WriteDone { .. }));
    for item in done.into_iter().chain(queued) {
        match item {
            Item::WriteDone { callbacks, ok, .. } => run_write_callbacks(ap, &callbacks, ok),
            Item::DataWrite {
                path,
                bytes,
                callbacks,
                ..
            } => {
                let ok = match bytes {
                    Some(bytes) => shared.io.write_sync(path, bytes),
                    None => true,
                };
                run_write_callbacks(ap, &callbacks, ok);
            }
            _ => {}
        }
    }
}

pub(super) fn install(
    b: &Builder<'_>,
    ns_root: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let pd = b.statics(ns_root, "PluginData")?;

    let c = Arc::clone(ctx);
    pd.raw_set(
        "Load",
        api_fn(
            lua,
            "Load",
            move |lua, (scope, key, handler): (Value, Value, Value)| {
                let path = data_target(&c, "Load", &scope, &key)?;
                let shared = c.shared()?;
                if c.loading.load(Ordering::SeqCst) {
                    // Synchronous while a plugin loads (§10.1 step 1).
                    let value = read_now(lua, &c, &shared, &path)?;
                    if let Value::Function(_) = handler
                        && let Err(e) = call_protected::<_, ()>(lua, &handler, value.clone())
                    {
                        report_ctx(&c, None, "plugin data load handler", &e);
                    }
                    return Ok(value);
                }
                // Asynchronous (§10.1 step 2): the handler is required.
                let Value::Function(handler) = handler else {
                    return Err(host_error(
                        "PluginData.Load: a load handler function is required",
                    ));
                };
                if lock(&shared.state).data_items(c.id) >= MAX_DATA_ITEMS {
                    return Err(host_error(
                        "PluginData.Load: too many pending plugin data operations",
                    ));
                }
                let value = read_now(lua, &c, &shared, &path)?;
                lock(&shared.state).queue.push_back(Item::DataLoad {
                    apartment: c.id,
                    handler,
                    value,
                });
                Ok(Value::Nil)
            },
        )?,
    )?;

    let c = Arc::clone(ctx);
    pd.raw_set(
        "Save",
        api_fn(
            lua,
            "Save",
            move |lua, (scope, key, data, on_done): (Value, Value, Value, Value)| {
                let path = data_target(&c, "Save", &scope, &key)?;
                let shared = c.shared()?;
                let limits = &shared.limits.data;
                let raise = |e: PluginDataError| host_error(format!("PluginData.Save: {e}"));
                let value =
                    plugindata::lua_to_data_metered(&data, limits, &c.budget).map_err(raise)?;
                let bytes = serialize(&value);
                c.budget.charge(bytes.len() as u64 * BYTE_COST)?;
                if bytes.len() as u64 > limits.max_bytes {
                    return Err(raise(PluginDataError::ValueTooLarge(limits.max_bytes)));
                }
                let bytes: Arc<[u8]> = Arc::from(bytes);
                let callback = match on_done {
                    Value::Function(f) => Some(f),
                    _ => None,
                };
                let io = Arc::clone(&shared.io);
                let drop_save = |p: &Path, n: usize| io.drop_save(p, n);
                if c.closing.load(Ordering::SeqCst) {
                    // The state is closing: write now (§10.1 step 3, §10.6), after every
                    // earlier write of the file; writes of it still queued are older and
                    // are superseded.
                    lock(&shared.state).supersede_writes(&path, &drop_save);
                    shared.io.register_save(&path, &bytes);
                    let ok = shared.io.write_sync(path, bytes);
                    if let Some(cb) = callback {
                        call_done(lua, &c, &[cb], ok);
                    }
                    return Ok(());
                }
                let refuse =
                    || host_error("PluginData.Save: too many pending plugin data operations");
                let pending = shared.io.pending_bytes();
                let cap = max_pending_write_bytes(&shared.limits);
                let mut state = lock(&shared.state);
                // The latest queued write of this file, by any apartment: when it is this
                // apartment's, it takes the new bytes and its callbacks run in call order
                // (per-key order, §10.6). Otherwise a new write is queued behind it.
                let latest = state.queue.iter_mut().rev().find_map(|i| match i {
                    Item::DataWrite {
                        apartment,
                        path: p,
                        bytes: b,
                        callbacks,
                    } if *p == path => Some((*apartment, b, callbacks)),
                    _ => None,
                });
                if let Some((apartment, Some(b), callbacks)) = latest
                    && apartment == c.id
                {
                    let old = b.len();
                    if pending.saturating_sub(old).saturating_add(bytes.len()) > cap
                        || (callback.is_some() && callbacks.len() >= MAX_WRITE_CALLBACKS)
                    {
                        return Err(refuse());
                    }
                    *b = Arc::clone(&bytes);
                    callbacks.extend(callback);
                    drop(state);
                    shared.io.replace_save(&path, old, &bytes);
                    return Ok(());
                }
                if state.data_items(c.id) >= MAX_DATA_ITEMS
                    || pending.saturating_add(bytes.len()) > cap
                {
                    return Err(refuse());
                }
                shared.io.register_save(&path, &bytes);
                state.queue.push_back(Item::DataWrite {
                    apartment: c.id,
                    path,
                    bytes: Some(bytes),
                    callbacks: callback.into_iter().collect(),
                });
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

/// Call the `onDone` callbacks of a write that ran (§10.6 step 4), each as its own host
/// call into the apartment.
pub(crate) fn run_write_callbacks(
    ap: &crate::apartment::Apartment,
    callbacks: &[Function],
    ok: bool,
) {
    for cb in callbacks {
        let args = ap
            .lua()
            .create_string(done_message(ok))
            .map(|m| MultiValue::from_vec(vec![Value::Boolean(ok), Value::String(m)]));
        let result = args.and_then(|a| ap.call(&Value::Function(cb.clone()), a));
        if let Err(e) = result {
            ap.report(None, "plugin data save callback", &e);
        }
    }
}
