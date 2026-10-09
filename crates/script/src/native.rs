// SPDX-License-Identifier: GPL-3.0-or-later
//! Host functions whose errors reach Lua as plain Lua 5.1 strings, and error description.
//!
//! Spec: `plugin-runtime.md` §11.1–§11.3 (protected calls, API errors, message format).

use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua, MultiValue, Value};

use crate::budget::{BUDGET_MESSAGE, Budget};
use crate::host::ErrorKind;

/// Registry name of the factory that wraps raw native functions (see [`native_fn`]).
pub(crate) const SHIM_KEY: &str = "tessera.core.shim";

/// Lua side of [`native_fn`]: the raw function returns `true, results...` or
/// `false, message`; the wrapper turns the latter into an ordinary string error, as a
/// Lua 5.1 C function raising with `luaL_error`/`lua_error` would.
pub(crate) const SHIM_SOURCE: &str = "local error = error
local function pass(ok, ...)
  if ok then return ... end
  error((...), 0)
end
return function(f)
  return function(...) return pass(f(...)) end
end
";

/// How a host function's own error messages look in Lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorStyle {
    /// Prefixed with `chunk:line: ` of the Lua code that called the function, as the
    /// stock standard library does (`luaL_error` reports its caller's position).
    Positioned,
    /// No position prefix, as the original client's API functions raise them
    /// (`plugin-runtime.md` §11.2).
    Plain,
}

/// An error raised by host code (as opposed to one raised by Lua code and passed through).
#[derive(Debug)]
pub(crate) struct HostError(pub String);

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HostError {}

/// An error message raised by a host function. It reaches Lua as a plain string (with or
/// without a position prefix, by the function's [`ErrorStyle`]), never as a host error
/// object.
pub fn host_error(msg: impl Into<String>) -> mlua::Error {
    mlua::Error::external(HostError(msg.into()))
}

/// Was this error raised by Lua code (so it already carries its position)?
fn raised_by_lua(e: &mlua::Error) -> bool {
    match e {
        mlua::Error::CallbackError { cause, .. } | mlua::Error::WithContext { cause, .. } => {
            raised_by_lua(cause)
        }
        mlua::Error::RuntimeError(_)
        | mlua::Error::MemoryError(_)
        | mlua::Error::SyntaxError { .. } => true,
        _ => false,
    }
}

/// Lua's word for the Rust type an argument conversion wanted.
fn lua_type_word(rust: &str) -> String {
    match rust {
        "String" | "string" | "LuaString" | "BString" | "str" => "string".to_owned(),
        "Table" => "table".to_owned(),
        "Function" => "function".to_owned(),
        "bool" => "boolean".to_owned(),
        "f64" | "f32" | "i64" | "i32" | "u64" | "u32" | "usize" | "isize" | "u8" | "i8" | "u16"
        | "i16" | "Number" | "Integer" => "number".to_owned(),
        other => other.to_lowercase(),
    }
}

fn plain_message(e: &mlua::Error) -> String {
    match e {
        mlua::Error::BadArgument { to, pos, cause, .. } => {
            let what = match &**cause {
                mlua::Error::FromLuaConversionError { from, to, .. } => {
                    format!("{} expected, got {from}", lua_type_word(to))
                }
                other => plain_message(other),
            };
            format!(
                "bad argument #{pos} to '{}' ({what})",
                to.as_deref().unwrap_or("?")
            )
        }
        mlua::Error::CallbackError { cause, .. } => plain_message(cause),
        other => describe_error(other).0,
    }
}

/// `chunk:line: ` of the Lua code that called the wrapped native (`luaL_where(L, 1)` from
/// the script's point of view), or `None` when the caller is not Lua code.
fn where_prefix(lua: &Lua) -> Option<String> {
    // Level 0 is the raw native, level 1 the shim wrapper, level 2 the script.
    lua.inspect_stack(2, |d| {
        let line = d.current_line().filter(|&l| l > 0)?;
        let src = d.source().short_src?.into_owned();
        Some(format!("{src}:{line}: "))
    })
    .flatten()
}

/// The string a failed native raises in Lua.
fn native_error_message(lua: &Lua, e: &mlua::Error, style: ErrorStyle) -> String {
    if raised_by_lua(e) {
        return describe_error(e).0;
    }
    let msg = plain_message(e);
    match (style, where_prefix(lua)) {
        (ErrorStyle::Positioned, Some(w)) => format!("{w}{msg}"),
        _ => msg,
    }
}

/// Create a host function in the style of the stock standard library: its own errors are
/// plain Lua strings prefixed with the calling script's `chunk:line:`; errors raised by
/// Lua code it called pass through unchanged. Arguments are converted to `A`; a failed
/// conversion raises `bad argument #n to '<name>' (...)`.
///
/// # Errors
///
/// A Lua error if the function cannot be created (memory) or the state was not built by
/// [`ScriptState`](crate::ScriptState).
pub fn native_fn<A, R, F>(lua: &Lua, name: &str, f: F) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
{
    native_fn_styled(lua, name, ErrorStyle::Positioned, f)
}

/// Create a host function in the style of the original client's API functions: its own
/// errors are plain Lua strings **without** a position prefix (`plugin-runtime.md` §11.2);
/// errors raised by Lua code it called pass through unchanged.
///
/// # Errors
///
/// As [`native_fn`].
pub fn api_fn<A, R, F>(lua: &Lua, name: &str, f: F) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
{
    native_fn_styled(lua, name, ErrorStyle::Plain, f)
}

/// [`native_fn`] or [`api_fn`] by `style`.
///
/// # Errors
///
/// As [`native_fn`].
pub fn native_fn_styled<A, R, F>(
    lua: &Lua,
    name: &str,
    style: ErrorStyle,
    f: F,
) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
{
    let name = name.to_owned();
    let raw = lua.create_function(move |lua, args: MultiValue| {
        let result = A::from_lua_args(args, 1, Some(&name), lua)
            .and_then(|a| f(lua, a))
            .and_then(|r| r.into_lua_multi(lua));
        match result {
            Ok(mut out) => {
                out.push_front(Value::Boolean(true));
                Ok(out)
            }
            // A spent budget also becomes a string here; the `pcall` guard re-raises it
            // after any protected call that caught it (`sandbox::GUARD_SOURCE`).
            Err(e) => {
                let msg = lua.create_string(native_error_message(lua, &e, style))?;
                Ok(MultiValue::from_vec(vec![
                    Value::Boolean(false),
                    Value::String(msg),
                ]))
            }
        }
    })?;
    let make: Function = lua.named_registry_value(SHIM_KEY)?;
    make.call(raw)
}

/// Split an mlua error into (message, traceback) for reporting.
#[must_use]
pub fn describe_error(e: &mlua::Error) -> (String, Option<String>) {
    match e {
        mlua::Error::CallbackError { traceback, cause } => {
            let (msg, inner_tb) = describe_error(cause);
            (msg, inner_tb.or_else(|| Some(traceback.clone())))
        }
        mlua::Error::WithContext { context, cause } => {
            let (msg, tb) = describe_error(cause);
            (format!("{context}: {msg}"), tb)
        }
        mlua::Error::RuntimeError(m) | mlua::Error::MemoryError(m) => split_traceback(m),
        mlua::Error::SyntaxError { message, .. } => split_traceback(message),
        mlua::Error::ExternalError(inner) => (inner.to_string(), None),
        other => (other.to_string(), None),
    }
}

fn split_traceback(m: &str) -> (String, Option<String>) {
    match m.find("\nstack traceback:") {
        Some(i) => (m[..i].to_owned(), Some(m[i + 1..].to_owned())),
        None => (m.to_owned(), None),
    }
}

/// Does this error carry the message of a spent instruction budget? A text test only: a
/// script can raise the same text itself. To decide whether a call really ran out of its
/// budget, use [`classify`] with the state's [`Budget`].
#[must_use]
pub fn is_budget_error(e: &mlua::Error) -> bool {
    describe_error(e).0.ends_with(BUDGET_MESSAGE)
}

/// Classify an error raised by a call of a state whose per-call budget is `budget`
/// (checked before the budget is reset for the next call): a refused allocation is
/// [`ErrorKind::Memory`]; a spent budget, known from the budget's own state and not from
/// the message text, is [`ErrorKind::Budget`]; anything else is [`ErrorKind::Runtime`].
#[must_use]
pub fn classify(e: &mlua::Error, budget: &Budget) -> ErrorKind {
    if is_memory_error(e) {
        ErrorKind::Memory
    } else if budget.exhausted() && is_budget_error(e) {
        ErrorKind::Budget
    } else {
        ErrorKind::Runtime
    }
}

/// Is this an allocation refused by the memory cap?
#[must_use]
pub fn is_memory_error(e: &mlua::Error) -> bool {
    match e {
        mlua::Error::MemoryError(_) => true,
        mlua::Error::CallbackError { cause, .. } | mlua::Error::WithContext { cause, .. } => {
            is_memory_error(cause)
        }
        mlua::Error::RuntimeError(m) => m == "not enough memory",
        _ => false,
    }
}
