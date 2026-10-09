// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Shell` and `Turbine.ShellCommand`.
//!
//! Spec: `lua-plugin-api.md` §11.1, §11.2; `plugin-runtime.md` §8.4 (queued `Received`
//! with skip sets).

use std::sync::Arc;

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::api_fn;

use crate::host::CHAT_TYPE_WRITE_LINE;
use crate::object::{Builder, NativeKind};
use crate::shared::{ApartmentCtx, ChatEvent, lock};

/// Longest `Shell.WriteLine` text kept, in bytes; longer text is cut at a character
/// boundary (a chat line, not a bulk transfer channel; Tessera bound).
pub(crate) const MAX_LINE_BYTES: usize = 4096;

/// Most chat commands one apartment may register; further names are not added (a Tessera
/// bound, `lua-plugin-api.md` §11.1 OPEN).
pub(crate) const MAX_COMMANDS_PER_APARTMENT: usize = 4096;

/// Budget units `AddCommand` charges per name it handles, on top of one unit per byte of
/// the names argument (`lua-plugin-api.md` §3.3).
const COMMAND_NAME_COST: u64 = 64;

/// Characters trimmed from command names (§11.1).
const NAME_TRIM: [char; 5] = [' ', '\t', '\r', '\n', '\u{0C}'];

/// The handler id of the command object `cmd` in this apartment: the id it already has,
/// or `None`.
fn handler_of(lua: &Lua, ctx: &ApartmentCtx, cmd: &Table) -> Option<u64> {
    lock(&ctx.inner)
        .commands
        .iter()
        .find(|(_, k)| {
            lua.registry_value::<Table>(k)
                .is_ok_and(|t| t.to_pointer() == cmd.to_pointer())
        })
        .map(|(h, _)| *h)
}

/// Cut `text` to at most [`MAX_LINE_BYTES`] bytes at a character boundary.
pub(crate) fn truncate_line(text: &mut String) {
    if text.len() > MAX_LINE_BYTES {
        let mut end = MAX_LINE_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}

pub(super) fn install(
    b: &Builder<'_>,
    ns_root: &Table,
    object: &Table,
    ctx: &Arc<ApartmentCtx>,
) -> mlua::Result<()> {
    let lua = b.lua;
    let cmd = b.class(
        ns_root,
        "ShellCommand",
        Some(object),
        Some(Arc::new(|| NativeKind::ShellCommand)),
    )?;
    cmd.raw_set(
        "Execute",
        api_fn(lua, "Execute", |_, _: MultiValue| Ok(()))?,
    )?;
    cmd.raw_set(
        "GetHelp",
        api_fn(lua, "GetHelp", |_, _: MultiValue| Ok(""))?,
    )?;
    cmd.raw_set(
        "GetShortHelp",
        api_fn(lua, "GetShortHelp", |_, _: MultiValue| Ok(""))?,
    )?;

    let shell = b.statics(ns_root, "Shell")?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "WriteLine",
        api_fn(lua, "WriteLine", move |lua, text: Value| {
            let mut text = super::tostring(lua, text)?;
            truncate_line(&mut text);
            let shared = c.shared()?;
            shared.host.write_chat(CHAT_TYPE_WRITE_LINE, &text);
            lock(&shared.state).queue_chat(ChatEvent {
                sender: None,
                message: text,
                chat_type: CHAT_TYPE_WRITE_LINE,
            });
            Ok(())
        })?,
    )?;

    // The shell commands live in the host's one command registry (`chat-window.md` §10
    // item 3, decision 0020 item 6): the apartment keeps only its command objects, by
    // handler id, and every name goes to the host with the apartment as its owner.
    let c = Arc::clone(ctx);
    shell.raw_set(
        "AddCommand",
        api_fn(
            lua,
            "AddCommand",
            move |lua, (names, cmd): (String, Option<Table>)| {
                let Some(cmd) = cmd else { return Ok(0u32) };
                let shared = c.shared()?;
                c.budget.charge(names.len() as u64)?;
                let known = handler_of(lua, &c, &cmd);
                let handler = known.unwrap_or_else(|| {
                    let mut inner = lock(&c.inner);
                    inner.next_handler = inner.next_handler.wrapping_add(1);
                    inner.next_handler
                });
                let mut added = 0u32;
                for raw in names.split([';', ':', ',']) {
                    c.budget.charge(COMMAND_NAME_COST)?;
                    let name = raw.trim_matches(NAME_TRIM);
                    if name.is_empty() {
                        continue;
                    }
                    if lock(&c.inner).command_names >= MAX_COMMANDS_PER_APARTMENT {
                        tracing::warn!(
                            apartment = %c.name,
                            "AddCommand: the apartment has {MAX_COMMANDS_PER_APARTMENT} commands; no more are added"
                        );
                        break;
                    }
                    if shared.host.register_command(&c.key, name, handler) {
                        lock(&c.inner).command_names += 1;
                        added += 1;
                    }
                }
                if added > 0 && known.is_none() {
                    let key = lua.create_registry_value(cmd)?;
                    lock(&c.inner).commands.insert(handler, key);
                }
                Ok(added)
            },
        )?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "RemoveCommand",
        api_fn(lua, "RemoveCommand", move |lua, cmd: Option<Table>| {
            let Some(cmd) = cmd else { return Ok(0u32) };
            let Some(handler) = handler_of(lua, &c, &cmd) else {
                return Ok(0u32);
            };
            let shared = c.shared()?;
            let removed = shared.host.unregister_command(&c.key, handler);
            let key = {
                let mut inner = lock(&c.inner);
                inner.command_names = inner.command_names.saturating_sub(removed);
                inner.commands.remove(&handler)
            };
            if let Some(k) = key {
                let _ = lua.remove_registry_value(k);
            }
            Ok(u32::try_from(removed).unwrap_or(u32::MAX))
        })?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "IsCommand",
        api_fn(lua, "IsCommand", move |_, name: String| {
            // `chat-window.md` §8.2: exactly as given, case-insensitive.
            let shared = c.shared()?;
            Ok(shared.host.is_command(&name))
        })?,
    )?;

    let c = Arc::clone(ctx);
    shell.raw_set(
        "GetCommands",
        api_fn(lua, "GetCommands", move |lua, ()| {
            let shared = c.shared()?;
            let mut names = shared.host.command_names();
            names.truncate(crate::commands::MAX_TABLE_NAMES);
            lua.create_sequence_from(names)
        })?,
    )?;
    Ok(())
}

/// Split a chat command line `/word rest` (`lua-plugin-api.md` §11.2): the word as typed
/// (case kept), and the rest after exactly one separating whitespace character,
/// everything else kept. `None` when the line does not start with `/` (after leading
/// whitespace) or has no word.
#[must_use]
pub fn split_command_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start().strip_prefix('/')?;
    let (word, rest) = match line.find(char::is_whitespace) {
        Some(i) => {
            let sep = line[i..].chars().next().map_or(1, char::len_utf8);
            (&line[..i], &line[i + sep..])
        }
        None => (line, ""),
    };
    (!word.is_empty()).then_some((word, rest))
}

/// Run `cmd:Execute(word, rest)` in `lua` (both always strings).
pub(crate) fn execute_shell_command(
    ap: &crate::apartment::Apartment,
    cmd: &Table,
    word: &str,
    rest: &str,
) -> mlua::Result<()> {
    let lua: &Lua = ap.lua();
    let args = MultiValue::from_vec(vec![
        Value::String(lua.create_string(word)?),
        Value::String(lua.create_string(rest)?),
    ]);
    ap.call_method(cmd, "Execute", args).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines() {
        assert_eq!(split_command_line("/greet"), Some(("greet", "")));
        assert_eq!(
            split_command_line("  /HI there  world "),
            Some(("HI", "there  world "))
        );
        assert_eq!(split_command_line("greet"), None);
        assert_eq!(split_command_line("/"), None);
        assert_eq!(split_command_line("/a\t\"q\""), Some(("a", "\"q\"")));
    }
}
