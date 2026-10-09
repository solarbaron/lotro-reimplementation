// SPDX-License-Identifier: GPL-3.0-or-later
//! The sandboxed global environment of a script state.
//!
//! Spec: `lua-plugin-api.md` §3.2 (libraries and removed globals), §3.3 (instruction
//! budget, finalizers); `plugin-runtime.md` §5.2 (globals), §9.3 (finalizers).

use std::sync::Arc;

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, MultiValue, Table, Value};

use crate::budget::{BUDGET_MESSAGE, Budget};
use crate::host::CoreHost;
use crate::native::{describe_error, host_error, native_fn};

/// Replacements for `pcall`, `xpcall` and `coroutine.resume` that re-raise once the
/// instruction budget is spent, so a protected call cannot swallow the budget error and
/// keep the host thread busy forever (`lua-plugin-api.md` §3.3).
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

/// Replacement for `newproxy` (`plugin-runtime.md` §9.3). Lua 5.1 runs `__gc` metamethods
/// with debug hooks disabled, so a finalizer written by a script would run outside the
/// instruction budget, and a looping one would hang the host thread (also when the state
/// closes). Proxies made by `newproxy(true)` therefore hide their real metatable behind
/// `__metatable`: the script sees and edits a view whose writes are copied to the real
/// metatable, except `__gc`, which becomes a fixed dispatcher. The dispatcher runs the
/// script's finalizer in a fresh coroutine, where the count hook works again, and re-raises
/// its error as Lua 5.1 does for an error in a finalizer. Reading a field through the view
/// returns what the script stored.
const PROXY_SOURCE: &str = "local newproxy, getmetatable, setmetatable, rawset, pcall, create, resume, error, type, find = ...
local function wrap(f)
  return function(...) return f(...) end
end
return function(arg)
  if arg ~= true then
    local ok, p = pcall(newproxy, arg)
    if ok then return p end
    if type(p) ~= 'string' or not find(p, 'proxy expected', 1, true) then error(p, 0) end
    error(\"bad argument #1 to 'newproxy' (boolean or proxy expected)\", 2)
  end
  local p = newproxy(true)
  local real = getmetatable(p)
  local shadow, view = {}, {}
  local function gc(u)
    local f = shadow.__gc
    if f == nil then return end
    local ok, e = resume(create(wrap(f)), u)
    if not ok then error(e, 0) end
  end
  setmetatable(view, {
    __index = shadow,
    __newindex = function(_, k, v)
      rawset(shadow, k, v)
      if k == '__gc' then
        if v == nil then rawset(real, k, nil) else rawset(real, k, gc) end
      elseif k == '__metatable' then
        if v == nil then rawset(real, k, view) else rawset(real, k, v) end
      else
        rawset(real, k, v)
      end
    end,
    __metatable = false,
  })
  rawset(real, '__metatable', view)
  return p
end
";

/// Every host call goes through this trampoline (`plugin-runtime.md` §11.1): the stock
/// `pcall` catches the error, and the error value is re-raised as a string as it was, a
/// number as its text through the state's `tostring` (the `%.14g` form of §4.4 when the
/// profile asks for the compatible conversions), and any other value (nil, boolean,
/// table, function, script userdata) as the **empty** message. Only the host's own error
/// objects (raised by host functions and the budget hook, recognised host-side) pass
/// through unchanged. No message handler and no `__tostring` of script values run while an
/// error is described.
const TRAMPOLINE_SOURCE: &str = "local pcall, type, error, is_host, tostring = ...
local function finish(ok, ...)
  if ok then return ... end
  local e = ...
  local t = type(e)
  if t == 'string' then error(e, 0) end
  if t == 'number' then error(tostring(e), 0) end
  if t == 'userdata' and is_host(e) then error(e, 0) end
  error('', 0)
end
return function(f, ...) return finish(pcall(f, ...)) end
";

/// Lua's binary chunk signature (binary chunks are refused, `lua-plugin-api.md` §3.2).
pub(crate) const BYTECODE_SIGNATURE: &[u8] = b"\x1bLua";

/// Globals no state ever has (`lua-plugin-api.md` §3.2).
const ALWAYS_REMOVED: [&str; 8] = [
    "io", "os", "package", "debug", "dofile", "require", "module", "loadfile",
];

/// Longest prefix of a `loadstring` source used as its chunk name (Lua shows at most the
/// first line, shortened to fit its 60-byte buffer).
const MAX_STRING_CHUNK_NAME: usize = 256;

pub(crate) fn load_internal(lua: &Lua, src: &str) -> mlua::Result<Function> {
    lua.load(src)
        .set_name("=tessera")
        .set_mode(ChunkMode::Text)
        .into_function()
}

/// Compile text Lua source with the calling thread's current global table as environment;
/// returns `(f)` or `(nil, message)` like Lua 5.1's `loadstring`.
fn compile_text(lua: &Lua, src: &[u8], chunkname: String) -> mlua::Result<MultiValue> {
    if src.starts_with(BYTECODE_SIGNATURE) {
        return lua.pack_multi((Value::Nil, "binary chunks are not allowed"));
    }
    match lua
        .load(src)
        .set_name(chunkname)
        .set_mode(ChunkMode::Text)
        .set_environment(lua.globals())
        .into_function()
    {
        Ok(f) => lua.pack_multi(f),
        Err(e) => lua.pack_multi((Value::Nil, describe_error(&e).0)),
    }
}

/// What [`install`] needs from the state being built.
pub(crate) struct SandboxSetup<'a> {
    pub name: &'a str,
    pub host: &'a Arc<dyn CoreHost>,
    pub budget: &'a Arc<Budget>,
    pub max_source: u64,
    pub max_output: usize,
    pub hidden: &'a [String],
    /// The `tostring` that `print` uses (the compatible one, when installed).
    pub tostring: Function,
}

/// Remove or replace globals so the state matches `lua-plugin-api.md` §3.2. The names of
/// [`SandboxSetup::hidden`] are removed last, after everything the sandbox builds from
/// the stock functions has captured them.
pub(crate) fn install(lua: &Lua, setup: SandboxSetup<'_>) -> mlua::Result<()> {
    let g = lua.globals();
    for name in ALWAYS_REMOVED {
        g.raw_set(name, Value::Nil)?;
    }

    // `loadstring`/`load` for text chunks only, compiled with the thread's globals.
    let loadstring = native_fn(
        lua,
        "loadstring",
        |lua, (src, chunkname): (mlua::LuaString, Option<String>)| {
            let bytes = src.as_bytes();
            let name = chunkname.unwrap_or_else(|| {
                String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_STRING_CHUNK_NAME)])
                    .into_owned()
            });
            compile_text(lua, &bytes, name)
        },
    )?;
    g.raw_set("loadstring", loadstring)?;
    let max_source = setup.max_source;
    let load = native_fn(
        lua,
        "load",
        move |lua, (reader, chunkname): (Value, Option<String>)| {
            let src: Vec<u8> = match reader {
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
            compile_text(lua, &src, chunkname.unwrap_or_else(|| "=(load)".to_owned()))
        },
    )?;
    g.raw_set("load", load)?;

    // `print` goes to the script log (`lua-plugin-api.md` §3.2).
    let tostring = setup.tostring;
    let host = Arc::clone(setup.host);
    let name = setup.name.to_owned();
    let print = native_fn(lua, "print", move |_, args: MultiValue| {
        let mut parts = Vec::with_capacity(args.len());
        for v in args {
            parts.push(tostring.call::<mlua::LuaString>(v)?.to_string_lossy());
        }
        host.script_log(&name, &parts.join("\t"));
        Ok(())
    })?;
    g.raw_set("print", print)?;

    // Finalizers run metered; see [`PROXY_SOURCE`]. Installed before `pcall` and
    // `coroutine.resume` are replaced, so it captures the stock functions.
    let coroutine: Table = g.raw_get("coroutine")?;
    {
        let newproxy: Function = load_internal(lua, PROXY_SOURCE)?.call((
            g.raw_get::<Function>("newproxy")?,
            g.raw_get::<Function>("getmetatable")?,
            g.raw_get::<Function>("setmetatable")?,
            g.raw_get::<Function>("rawset")?,
            g.raw_get::<Function>("pcall")?,
            coroutine.raw_get::<Function>("create")?,
            coroutine.raw_get::<Function>("resume")?,
            g.raw_get::<Function>("error")?,
            g.raw_get::<Function>("type")?,
            g.raw_get::<Table>("string")?.raw_get::<Function>("find")?,
        ))?;
        g.raw_set("newproxy", newproxy)?;
    }

    // Budget errors cannot be swallowed by protected calls.
    let budget = Arc::clone(setup.budget);
    let exhausted = lua.create_function(move |_, ()| Ok(budget.exhausted()))?;
    let (pcall, xpcall, resume): (Function, Function, Function) = load_internal(lua, GUARD_SOURCE)?
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
    crate::pattern::install(lua, &string, setup.budget, setup.max_output)?;

    // Last: the profile's hidden names (everything above has captured what it needs).
    for name in setup.hidden {
        g.raw_set(name.as_str(), Value::Nil)?;
    }
    Ok(())
}

/// The stock functions the trampoline is built from, captured before the sandbox
/// replaces or hides any global.
pub(crate) struct TrampolineParts {
    pub pcall: Function,
    pub type_fn: Function,
    pub error: Function,
    /// The state's `tostring` (the compatible one when installed).
    pub tostring: Function,
}

/// Build the host-call trampoline (see [`TRAMPOLINE_SOURCE`]).
pub(crate) fn trampoline(lua: &Lua, parts: TrampolineParts) -> mlua::Result<Function> {
    // A host error object is mlua's wrapped error userdata; any other userdata is the
    // script's own and is reported as the empty message.
    let is_host = lua.create_function(|_, v: Value| Ok(matches!(v, Value::Error(_))))?;
    load_internal(lua, TRAMPOLINE_SOURCE)?.call((
        parts.pcall,
        parts.type_fn,
        parts.error,
        is_host,
        parts.tostring,
    ))
}
