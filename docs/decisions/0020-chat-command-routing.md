<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0020 — Chat command routing: one client-side dispatcher between the chat line, local commands, plugins and the server

**Status:** Accepted, 2026-10-08

## Context
Three features take commands typed as text, and each has its own path today:

- **The walker's chat line** (M7, `apps/client/src/walker/chat.rs`). Every submitted line goes
  to `WalkerSession::send_chat`, which sends it as a `Say` `ChatSend` (`tessera-protocol.md`
  §6.2.14) to the test server or the sandbox. A typed `/play tune` is said aloud.
- **Player music** (M6, `apps/client/src/audio/player_music.rs`, `abc-dialect.md` §11):
  `/play`, `/playstart`, `/playlist`, `/playstop` and `/instrument`. Only `--music-command`
  reaches them, plus `PluginRuntime::command` in `apps/client/src/plugins.rs`, a stop-gap
  router that the walker does not use. They have their own tokenizer.
- **Plugins** (`crates/script`, M5 later): `Shell.AddCommand`, `RemoveCommand` and `IsCommand`
  (`lua-plugin-api.md` §11.1). `PluginManager::execute_command` parses lines a third way and
  keeps its own name table. `HostApi::is_builtin_command` and `builtin_commands` exist but the
  client does not implement them.

`chat-window.md` §7–§10 (Accepted) already fixes the parts that plugins and players can see:
one registry, matched case-insensitively (§7.1, §8.2); the line pipeline with the prefix test,
tokenizer, command word and arguments (§7.2–§7.4); the unknown-command line (§7.6); the help
listing (§7.7); and the M5 Tessera rules (§10 items 3, 4, 6). What is still open is how the
client is built around those rules. Who owns the registry? Which commands run locally and
which go to the server? What happens without a server? How do plugins register before the M5
chat window exists?

## Decision
1. **One dispatcher.** The client has one command dispatcher, a module `apps/client/src/commands/`
   that owns the registry and the line pipeline. Every source of typed command text feeds it:
   the walker's chat overlay now, the M5 chat window's entry line later (it replaces the
   overlay), and the start-up lines of item 7. No other code parses command lines.
   - The parser implements `chat-window.md` §7.2–§7.4 exactly, quirks included. It depends
     only on `std`, so a fuzz target `client_command_line` can include it by path, as
     `client_player_music` does today. Input is bounded by the entry's cap.
   - The command prefix comes from the install (§10 item 4). When the string table cannot be
     read, the prefix is `/`.
   - The dispatcher runs where the line is submitted. In M5 that is frame-loop step 1 of
     decision 0014, so plugin command handlers stay inside steps 1 and 7.
2. **Registered commands run on the client. The server gets only channel lines.** There is
   no "send unknown commands to the server" path. The Tessera protocol has no command message,
   and a mistyped command must not leak into public chat. Every line goes one of three ways:

   | Line | Route |
   |---|---|
   | Plain text (no prefix) | Run as the entry's channel command with the text (§7.2 step 5). The walker overlay's mode is always `Say`; the M5 window uses its channel mode (§6.5). |
   | Prefix + a registered name | Run that command's handler, locally. |
   | Prefix + an unregistered name | Unknown command (item 4). Nothing is sent. |

   **Local built-ins** (owner: client):
   - **Player music:** `play`, `playstart`, `playlist`, `playstop`, `instrument`. They keep the
     M6 semantics and usage strings, but take their arguments from the §7.3 token list. The
     music module's own tokenizer is retired, and `abc-dialect.md` §11 OPEN (d) still applies
     to the argument rules.
   - **Help and plugins:** `help` (item 5) and `plugins` (`lua-plugin-api.md` §6.4, moved out of
     `PluginManager::execute_command` into a client built-in that calls the manager).
   - **Client options:** commands that change the client's own state, such as the walker's
     switches and clock, which keys reach today. Each one is a built-in registered under a
     Tessera name. None is added by this record.

   **Channel commands** (also client built-ins) are the only commands that build a `ChatSend`:
   - Tessera names: `say <text>`, `yell <text>`, `emote <text>`, and `tell <name> <text>`
     (the name is the first token and is quoted if it has spaces). They map to protocol
     channels 1–4.
   - Original names: the channel words of the install's mode table (`chat-window.md` §6.5,
     §10 item 3) are registered too. Words for `Say` and `Tell` send on those channels.
   - Other channels: words whose channel the Tessera protocol lacks (fellowship, raid,
     OOC, ...) stay registered, so `IsCommand` does not change between sessions. Their
     handler posts a local error line ("This channel is not available here.") and sends
     nothing.
   - To say a line that starts with the prefix, use `/say /text`.
3. **Sessions, and offline play without `--server`.** Local commands behave the same in every
   session state. Only channel commands depend on it:
   - **`--server`:** the line is sent as `ChatSend`. A send that fails gives the existing
     local line "Not in the world: chat not sent".
   - **Sandbox (no `--server`):** the line goes to the sandbox backend, which echoes it as a
     line of that channel (`chat-window.md` §10 item 2). `Tell` has no peers in the sandbox and
     posts a local error line.
   - **No session** (headless and scripted runs where the walker has no `WalkerSession`): the
     existing local line "Not connected: chat not sent".

   Music never needs a session. In a `--server` session it plays for the local player only,
   because protocol v1 does not replicate music.
4. **Unknown commands.** Result 3 of §7.4 posts the `chat-window.md` §7.6 line locally as a
   type 1 `Error` line (§10 item 6). If the string table is missing, Tessera's own wording is
   used: "Unknown command. Type /help for a list." A line that is only the prefix (result 5)
   does nothing. A rejected line is not sent and is not kept in the overlay's outgoing queue.
5. **Help.** Every registration carries a short help (one line) and a long help (usage plus
   a sentence), in Tessera's own wording. For the music commands these are the existing
   `USAGE_*` strings. `/help` with no argument lists every registered name in alphabetical
   order, with its short help, one line each, aliases grouped, as type 8 `Log` lines
   (`chat-window.md` §7.7, §10). `/help <name>` shows that command's long help. For plugins,
   short and long help are `GetShortHelp` and `GetHelp` (`lua-plugin-api.md` §11.2), called
   when `/help` runs. `help` is also registered under the install's help names (§10 item 3).
6. **The registry interface for M5 plugins** (specified now, built with M5). The registry is
   owned by the client and keyed by the lower-cased name without the prefix. Each entry
   records an **owner**: `Client`, or `Plugin(apartment key)`. The registry has four
   operations:
   - **register(names, owner, handler) → count.** Splits `names` per §7.1. The first
     registration of a name wins; a taken name is skipped and not counted.
   - **unregister(owner, handler) → count**, and **unregister_owner(owner) → count**
     (apartment unload, `lua-plugin-api.md` §6.3).
   - **is_command(name) → bool.** Matches the exact name, case-insensitively (§8.2).
   - **entries() → names, owners and help**, for `/help` and `Shell.GetCommands`.

   How the two crates connect:
   - `tessera-script` keeps no name table of its own. `Shell.AddCommand`, `RemoveCommand`
     and `IsCommand` reach the registry through new `HostApi` methods (`register_command`,
     `unregister_command`, `is_command`). Those methods replace `is_builtin_command` and
     `builtin_commands`.
   - `PluginManager::execute_command(line)` becomes `run_command(apartment, word, args)`. The
     dispatcher calls it after parsing and lookup, so plugins get §9's `command` and
     `arguments`.
   - Client built-ins are registered at start-up, before any plugin loads, so a plugin cannot
     take `play`, `help` or a channel word.
7. **Start-up command lines.** `--music-command LINE` becomes `--command LINE` (repeatable),
   and the old name stays as an alias. The lines are queued in order and dispatched in step 1
   of the first frame after the archives are open. A start-up line that resolves to a channel
   command is logged and dropped, never sent, so a scripted run does not talk on a server
   unless someone asked it to.
8. **Typing in chat never moves the character.**
   - While the entry is open, every key and text event goes to the entry. Opening it releases
     everything the controls hold (`chat.rs` `route`, test
     `opening_the_chat_releases_held_movement_keys`).
   - In M5 the `text_entry` context in the UI band (decision 0014 D2, D5) carries the same
     guarantee.
   - No command handler creates movement input or key events. A future command that changes
     the character (for example an emote) sends a session command or calls a client API.
     It never acts as if a key were pressed.
   - Closing the entry does not re-press keys that are still held. A movement key acts only on
     its next press.

## Consequences
- **Walker:** `chat.rs` keeps its input handling. `walker/mod.rs` passes each submitted line
  to the dispatcher instead of to `send_chat`. `WalkerSession::send_chat` gains a channel (and
  peer) parameter, used only by channel commands. `PluginRuntime::command` in `plugins.rs`
  becomes a call to the dispatcher.
- **Player music:** `/play` and the other music commands work from the walker's chat line in
  every session. `--music-command` scripts keep working through the alias.
- **New tests:**
  - A music command, an unknown command and a prefix-only line never reach `send_chat`.
  - `/say /x` sends `/x`.
  - Built-in names cannot be taken by `register`.
  - `is_command` follows §8.2.
  - The existing movement test keeps passing.
- **New fuzz target:** `client_command_line`.
- **Spec follow-ups (spec writers, not this record):** `walker.md` §10.8 should say that the
  overlay feeds the dispatcher, and `lua-plugin-api.md` §11.1 should name the `HostApi` hooks
  of item 6 as Tessera's implementation note. `tessera-protocol.md` does not change. A
  server-side command set (for example administration on the test server) would need a
  protocol message and a later record. It would register the server's commands with an owner
  `Server`, and is not part of this record.
- **Known differences from the original:**
  - Channel words the Tessera server cannot carry answer locally instead of reaching game
    scripts.
  - Only the original command names listed in `chat-window.md` §10 item 3 are registered
    (§13 Q15).
