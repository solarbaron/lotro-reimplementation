// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Engine` (static).
//!
//! Spec: `lua-plugin-api.md` §12.

use std::sync::Arc;

use mlua::{MultiValue, Table, Value};
use tessera_script::api_fn;

use crate::object::Builder;
use crate::shared::ApartmentCtx;

pub(super) fn install(
    b: &Builder<'_>,
    ns_root: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let engine = b.statics(ns_root, "Engine")?;

    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetDate",
        api_fn(lua, "GetDate", move |lua, ()| {
            let d = c.shared()?.host.engine_date();
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
        api_fn(lua, "GetGameTime", move |_, ()| {
            Ok(c.shared()?.host.game_time())
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLocalTime",
        api_fn(lua, "GetLocalTime", move |_, ()| {
            // A 32-bit signed integer: wraps after 2038-01-19 (§12).
            let t = c.shared()?.host.local_time();
            Ok(f64::from((t as i64) as i32))
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLanguage",
        api_fn(lua, "GetLanguage", move |_, ()| {
            Ok(f64::from(c.shared()?.host.language()))
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetLocale",
        api_fn(lua, "GetLocale", move |_, ()| Ok(c.shared()?.host.locale()))?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "GetScriptVersion",
        api_fn(lua, "GetScriptVersion", move |_, ()| {
            Ok(c.shared()?.host.script_version())
        })?,
    )?;
    // `debug.traceback(...)` with up to three of the given arguments: an optional thread
    // (ignored: the trace is of the running thread), a message, a level.
    engine.raw_set(
        "GetCallStack",
        api_fn(lua, "GetCallStack", |lua, args: MultiValue| {
            let mut it = args.into_iter().take(3).peekable();
            if matches!(it.peek(), Some(Value::Thread(_))) {
                it.next();
            }
            let msg = it.next().unwrap_or(Value::Nil);
            let level = match it.next() {
                Some(Value::Integer(i)) => usize::try_from(i).unwrap_or(1),
                Some(Value::Number(n)) if n >= 0.0 => n as usize,
                _ => 1,
            };
            match msg {
                Value::Nil => Ok(Value::String(lua.traceback(None, level)?)),
                Value::String(s) => Ok(Value::String(
                    lua.traceback(Some(&s.to_string_lossy()), level)?,
                )),
                Value::Integer(_) | Value::Number(_) => {
                    let s = super::tostring(lua, msg)?;
                    Ok(Value::String(lua.traceback(Some(&s), level)?))
                }
                // Lua 5.1: a message that is not a string is returned untouched.
                other => Ok(other),
            }
        })?,
    )?;
    let c = Arc::clone(ctx);
    engine.raw_set(
        "ScriptLog",
        api_fn(lua, "ScriptLog", move |lua, msg: Value| {
            let text = super::tostring(lua, msg)?;
            c.shared()?.host.script_log(&c.name, &text);
            Ok(())
        })?,
    )?;
    Ok(())
}
