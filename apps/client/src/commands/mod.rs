// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's one command dispatcher (decision 0020): the registry of
//! `chat-window.md` §7.1 and §8.2 ([`registry`]), the line pipeline of §7.2–§7.4
//! ([`line`](mod@line)), the words read from the install ([`text`]) and the client's built-in
//! commands. Every source of typed command text feeds the dispatcher: the walker's chat
//! overlay and the start-up lines of `--command` through [`Dispatcher::submit`] (the `Say`
//! mode), and the M5 chat window's entry line through [`Dispatcher::submit_in`] with the
//! window's channel mode (`chat-window.md` §6.2 submit, §6.5). The window browses the one
//! history of §6.3 through [`History`]'s [`EntryHistory`] implementation.
//!
//! Routes (decision 0020 item 2):
//!
//! | Line | Route |
//! |---|---|
//! | Plain text | The channel command of the entry's mode (the overlay: `Say`), §7.2 step 5 |
//! | Prefix + a registered name | That command's handler, on the client |
//! | Prefix + an unregistered name | The unknown-command line (type 1 `Error`); nothing is sent |
//! | Only the prefix | Nothing |
//!
//! Only channel commands build a chat message for the session; player music, help and the
//! plugin manager never need one. No handler creates movement input (item 8).
//!
//! The registry is the client's one command table (`chat-window.md` §10 item 3): the
//! built-ins are registered under their Tessera names at start-up, the install's names when
//! the tables have been read ([`Dispatcher::apply_text`]), and the plugins' commands through
//! the plugin host's `Shell.AddCommand` with their apartment as the owner
//! ([`SharedRegistry`]); plugins load only after the install's names are in.

pub mod line;
pub mod p5;
pub mod registry;
pub mod social;
pub mod text;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use tessera_net::ChatChannel;
use tessera_net::model::MAX_CHAT_LEN;

use tessera_ui::game::chat::EntryHistory;

use crate::audio::player_music::{MusicWord, PlayerMusic};
use crate::walker::WalkerSession;

use self::line::{History, Invocation, LineError};
use self::registry::{Builtin, Channel, Entry, Handler, Help, Owner, Registry, SocialWord};
use self::text::CommandText;

/// Tessera's unknown-command line when the install's is missing (decision 0020 item 4;
/// `{p}` is the command prefix).
pub const UNKNOWN_FALLBACK: &str = "Unknown command. Type {p}help for a list.";
/// The line of a channel command without a session (decision 0020 item 3).
pub const NOT_CONNECTED: &str = "Not connected: chat not sent";
/// The line of a channel command the session refused (decision 0020 item 3).
pub const NOT_IN_WORLD: &str = "Not in the world: chat not sent";
/// The line of a channel the Tessera protocol does not carry (decision 0020 item 2).
pub const CHANNEL_UNAVAILABLE: &str = "This channel is not available here.";
/// The line of a music command without player music.
pub const NO_MUSIC: &str = "Player music is not available.";
/// The line of `/plugins` without a plugin host.
pub const NO_PLUGINS: &str = "Plugins are not available.";
/// The line of an over-long command line.
pub const TOO_LONG: &str = "That line is too long.";

/// The chat type of a line the client posts itself (`chat-window.md` §10 items 2 and 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Type 1 `Error`: unknown commands, refusals.
    Error,
    /// Type 8 `Log`: help, listings, notices.
    Log,
}

/// A line the client posts itself (not received from the session).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalLine {
    /// Its chat type.
    pub kind: LineKind,
    /// Its text (plain).
    pub text: String,
    /// (P4) The line's 64-bit chat-type mask when it is not the kind's own (0x1 `Error`,
    /// 0x80 `Log`): the lines of the P4 built-ins take the original's masks
    /// (`chat-window.md` §15.10 CT2).
    pub mask: Option<u64>,
}

impl LocalLine {
    /// A type 1 `Error` line.
    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            kind: LineKind::Error,
            text: text.into(),
            mask: None,
        }
    }

    /// A type 8 `Log` line.
    #[must_use]
    pub fn log(text: impl Into<String>) -> Self {
        Self {
            kind: LineKind::Log,
            text: text.into(),
            mask: None,
        }
    }

    /// A line with the chat-type mask `mask` (`chat-window.md` §15.10 CT2): an `Error`
    /// line when bit 0 (type 1) is set, else a `Log`-kind line shown with the mask.
    #[must_use]
    pub fn masked(mask: u64, text: impl Into<String>) -> Self {
        Self {
            kind: if mask & 1 != 0 {
                LineKind::Error
            } else {
                LineKind::Log
            },
            text: text.into(),
            mask: Some(mask),
        }
    }

    /// The line's mask: [`Self::mask`], else 0x1 for `Error` and 0x80 for `Log`.
    #[must_use]
    pub fn chat_mask(&self) -> u64 {
        self.mask.unwrap_or(match self.kind {
            LineKind::Error => 0x1,
            LineKind::Log => 0x80,
        })
    }
}

/// Where a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Typed in the chat entry.
    Chat,
    /// A `--command` start-up line (decision 0020 item 7): channel commands are dropped.
    Startup,
}

/// What [`Dispatcher::submit`] did with a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to run (`chat-window.md` §7.4 result 5: empty, or only the prefix).
    Nothing,
    /// A handler ran (it may have posted its own error line).
    Ran,
    /// Unknown command (result 3): the unknown-command line was posted, nothing sent.
    Unknown,
    /// Refused before it ran (longer than [`line::MAX_LINE_LEN`]).
    TooLong,
}

/// The plugin side of the dispatcher (decision 0020 item 6; `chat-window.md` §10 item 3):
/// the plugin manager's entry points the built-in `plugins` command and the plugins'
/// registry entries call. The plugin host keeps no command table: its `Shell.AddCommand`,
/// `RemoveCommand` and `IsCommand` reach the dispatcher's registry ([`SharedRegistry`]).
pub trait PluginCommands {
    /// `/plugins <arguments>` (`lua-plugin-api.md` §6.4): the plugin-manager command, a
    /// client built-in that calls the plugin manager.
    fn plugins_command(&mut self, arguments: &str);
    /// Runs a plugin command of the registry: `cmd:Execute(word, arguments)` on the
    /// command object `handler` of `apartment` (`lua-plugin-api.md` §11.2, §9).
    fn run_command(&mut self, apartment: &str, handler: u64, word: &str, arguments: &str);
    /// A plugin command's help: `GetHelp` when `long`, else `GetShortHelp`.
    fn help(&mut self, apartment: &str, handler: u64, long: bool) -> Option<String> {
        let _ = (apartment, handler, long);
        None
    }
}

/// What the handlers act on, for one [`Dispatcher::submit`].
pub struct Context<'a> {
    /// The frame time.
    pub now: Instant,
    /// Where the line came from.
    pub origin: Origin,
    /// The world session (`None`: no session, headless and scripted runs).
    pub session: Option<&'a mut dyn WalkerSession>,
    /// Player music.
    pub music: Option<&'a mut PlayerMusic>,
    /// The plugin host.
    pub plugins: Option<&'a mut dyn PluginCommands>,
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("origin", &self.origin)
            .field("session", &self.session.is_some())
            .field("music", &self.music.is_some())
            .field("plugins", &self.plugins.is_some())
            .finish()
    }
}

/// The dispatcher's registry, shared with the plugin host's [`tessera_plugins::HostApi`]
/// (decision 0020 item 6): plugin code registers into it while the dispatcher is not
/// running, and the dispatcher never holds its lock while plugin code runs.
pub type SharedRegistry = Arc<Mutex<Registry>>;

/// Locks a [`SharedRegistry`] (a poisoned lock is used as it is: the registry stays
/// consistent after every single operation).
pub fn lock_registry(r: &SharedRegistry) -> MutexGuard<'_, Registry> {
    r.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The command dispatcher (module docs).
#[derive(Debug)]
pub struct Dispatcher {
    registry: SharedRegistry,
    text: CommandText,
    history: History,
    /// The requests of the P5 built-ins ([`p5`]).
    p5: p5::P5Queue,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

fn text_help(short: &str, long: String) -> Help {
    Help::Text {
        short: short.to_owned(),
        long,
    }
}

fn channel_help(ch: Channel) -> Help {
    match ch {
        Channel::Say => text_help(
            "say something nearby",
            "/say <text>: says the text on the local channel.".into(),
        ),
        Channel::Yell => text_help(
            "yell something",
            "/yell <text>: yells the text, heard further away.".into(),
        ),
        Channel::Emote => text_help(
            "act something out",
            "/emote <text>: shows the text as an emote.".into(),
        ),
        Channel::Tell => text_help(
            "send a private message",
            "/tell <name> <text>: sends the text to one character (quote a name with spaces)."
                .into(),
        ),
        Channel::Reply => text_help(
            "reply to the last private message",
            "/reply <text>: sends the text to the last character who sent you a tell.".into(),
        ),
        Channel::Retell => text_help(
            "tell the last character you told again",
            "/retell <text>: sends the text to the last character you sent a tell.".into(),
        ),
        Channel::Fellowship => text_help(
            "speak to your fellowship",
            "/fellowship <text>: says the text on your fellowship's channel.".into(),
        ),
        Channel::Slot(k) => text_help(
            "speak on a joined chat channel",
            format!("/ch{k} <text>: says the text on the chat channel in slot {k}."),
        ),
        Channel::Unavailable => text_help(
            "a channel not available here",
            "A chat channel the Tessera protocol does not carry; using it posts a notice.".into(),
        ),
    }
}

fn social_help(word: SocialWord) -> Help {
    let (short, long) = match word {
        SocialWord::Join => (
            "join a chat channel",
            "/join <channel>: joins the named chat channel (it takes the lowest free slot).",
        ),
        SocialWord::Part => (
            "leave a chat channel",
            "/part <channel>: leaves the chat channel.",
        ),
        SocialWord::Channels => (
            "list your chat channels",
            "/channels: lists the chat channels you joined, by slot.",
        ),
        SocialWord::ChSay => (
            "speak on a chat channel by name",
            "/chsay <channel> <text>: says the text on a joined chat channel.",
        ),
        SocialWord::Friend => (
            "manage your friends list",
            "/friend add <name> | remove <name> | list: adds, removes or lists friends.",
        ),
        SocialWord::Ignore => (
            "manage your ignore list",
            "/ignore add <name> | remove <name> | list: adds, removes or lists ignored names.",
        ),
        SocialWord::Invite => (
            "invite someone to your fellowship",
            "/invite <name>: invites the character to your fellowship.",
        ),
        SocialWord::Leave => ("leave your fellowship", "/leave: leaves your fellowship."),
        SocialWord::Dismiss => (
            "dismiss a member of your fellowship",
            "/dismiss <name>: dismisses the member from your fellowship (as its leader).",
        ),
        SocialWord::Promote => (
            "make someone the fellowship's leader",
            "/promote <name>: makes the member the leader of your fellowship.",
        ),
        SocialWord::Disband => (
            "disband your fellowship",
            "/disband: disbands your fellowship.",
        ),
    };
    text_help(short, long.to_owned())
}

fn music_help(word: MusicWord) -> Help {
    text_help(
        word.short_help(),
        format!("{}: {}.", word.usage(), word.short_help()),
    )
}

/// The Tessera names of the client built-ins, registered at start-up before any plugin
/// (decision 0020 item 6).
fn builtins() -> Vec<(&'static str, Builtin, Help)> {
    let mut v: Vec<(&'static str, Builtin, Help)> = MusicWord::ALL
        .into_iter()
        .map(|w| (w.name(), Builtin::Music(w), music_help(w)))
        .collect();
    v.push((
        "help",
        Builtin::Help,
        text_help(
            "list the commands, or show one command's help",
            "/help [<command>]: lists every command, or shows the help of one.".into(),
        ),
    ));
    v.push((
        "plugins",
        Builtin::Plugins,
        text_help(
            "load, unload and list plugins",
            "/plugins load <name> | unload [<state>] | list | refresh: manages plugins.".into(),
        ),
    ));
    for (name, ch) in [
        ("say", Channel::Say),
        ("yell", Channel::Yell),
        ("emote", Channel::Emote),
        ("tell", Channel::Tell),
        ("reply", Channel::Reply),
        ("retell", Channel::Retell),
        ("fellowship", Channel::Fellowship),
        ("ch1", Channel::Slot(1)),
        ("ch2", Channel::Slot(2)),
        ("ch3", Channel::Slot(3)),
        ("ch4", Channel::Slot(4)),
        ("ch5", Channel::Slot(5)),
        ("ch6", Channel::Slot(6)),
        ("ch7", Channel::Slot(7)),
        ("ch8", Channel::Slot(8)),
    ] {
        v.push((name, Builtin::Channel(ch), channel_help(ch)));
    }
    for w in SocialWord::ALL {
        v.push((w.name(), Builtin::Social(w), social_help(w)));
    }
    for w in p5::P5Word::ALL {
        v.push((w.name(), Builtin::P5(w), w.help()));
    }
    v
}

/// The usage line of a client built-in by its Tessera name (decision 0020 item 5): its
/// long help text, else `Usage: <prefix><name>`.
#[must_use]
pub fn usage_text(prefix: &str, name: &str) -> String {
    builtins()
        .into_iter()
        .find(|(n, _, _)| *n == name)
        .and_then(|(_, _, h)| match h {
            Help::Text { long, .. } => Some(format!("Usage: {long}")),
            Help::Plugin => None,
        })
        .unwrap_or_else(|| format!("Usage: {prefix}{name}"))
}

/// The protocol channel of a local-speech channel command (the P4 channels run through
/// [`social`]).
fn protocol_channel(ch: Channel) -> Option<ChatChannel> {
    match ch {
        Channel::Say => Some(ChatChannel::Say),
        Channel::Yell => Some(ChatChannel::Yell),
        Channel::Emote => Some(ChatChannel::Emote),
        _ => None,
    }
}

/// Splits `tell` arguments into the name (the first token, quoted when it has spaces) and
/// the text after it, trimmed.
#[must_use]
pub fn split_tell(arguments: &str) -> (&str, &str) {
    let a = line::trim(arguments);
    if let Some(rest) = a.strip_prefix('"')
        && let Some(end) = rest.find('"')
    {
        let (name, after) = rest.split_at(end);
        return (name, line::trim(after.get(1..).unwrap_or("")));
    }
    match a.find(line::is_whitespace) {
        Some(i) => {
            let (name, after) = a.split_at(i);
            (name, line::trim(after))
        }
        None => (a, ""),
    }
}

/// The same session borrowed for a shorter time (an `Option` of a trait object does not
/// coerce on its own).
pub fn shorten<'a>(
    session: Option<&'a mut (dyn WalkerSession + '_)>,
) -> Option<&'a mut (dyn WalkerSession + 'a)> {
    match session {
        Some(s) => Some(s),
        None => None,
    }
}

impl Dispatcher {
    /// A dispatcher with the client built-ins under their Tessera names and the default
    /// prefix (the install's words come with [`Self::apply_text`]).
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Registry::new();
        for (name, b, help) in builtins() {
            registry.register(name, &Owner::Client, &Handler::Builtin(b), &help);
        }
        Self {
            registry: Arc::new(Mutex::new(registry)),
            text: CommandText::default(),
            history: History::new(),
            p5: p5::P5Queue::default(),
        }
    }

    /// The requests the P5 built-ins queued ([`p5`]): the frame loop takes them.
    #[must_use]
    pub fn p5_queue(&self) -> &p5::P5Queue {
        &self.p5
    }

    /// Takes the install's words (`chat-window.md` §10 items 3, 4, 6): the prefix, the
    /// unknown-command line, and the original names of help, the plugin manager, the
    /// `CommandSetup` default channel word (a `Say` word, so plain text always finds its
    /// command; in this install the same name as the `Say` row's, CW42, and then a no-op)
    /// and the 26 fixed mode-table rows, registered for the same built-ins (a name already
    /// held is skipped, §7.1). Call before any plugin loads (§10 item 3).
    pub fn apply_text(&mut self, text: CommandText) {
        let client = Owner::Client;
        let mut added = 0;
        let mut reg = lock_registry(&self.registry);
        let mut add = |reg: &mut Registry, names: &str, b: Builtin| {
            let help = match b {
                Builtin::Channel(ch) => channel_help(ch),
                Builtin::Social(w) => social_help(w),
                _ => builtins()
                    .into_iter()
                    .find(|(_, x, _)| *x == b)
                    .map_or(Help::Plugin, |(_, _, h)| h),
            };
            added += reg.register(names, &client, &Handler::Builtin(b), &help);
        };
        if let Some(n) = &text.help_names {
            add(&mut reg, n, Builtin::Help);
        }
        if let Some(n) = &text.plugins_names {
            add(&mut reg, n, Builtin::Plugins);
        }
        // The default channel word runs plain text (§7.2 step 5), so it must be registered.
        if let Some(w) = &text.default_word {
            add(&mut reg, w, Builtin::Channel(Channel::Say));
        }
        for m in &text.modes {
            add(&mut reg, &m.names, Builtin::Channel(m.channel));
        }
        // The P4 built-ins (`chat-window.md` §15.10 CT2, `social-panels.md` §9 TR11).
        for (w, names) in &text.social {
            add(&mut reg, names, Builtin::Social(*w));
        }
        drop(reg);
        tracing::info!(added, prefix = %text.prefix, "install command names registered");
        self.text = text;
    }

    /// The registry (locked for the guard's lifetime; never hold it across plugin code).
    pub fn registry(&self) -> MutexGuard<'_, Registry> {
        lock_registry(&self.registry)
    }

    /// The registry for the plugin host (decision 0020 item 6).
    #[must_use]
    pub fn shared_registry(&self) -> SharedRegistry {
        Arc::clone(&self.registry)
    }

    /// The command prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.text.prefix
    }

    /// The install's words in use.
    #[must_use]
    pub fn text(&self) -> &CommandText {
        &self.text
    }

    /// The history (`chat-window.md` §6.3).
    #[must_use]
    pub fn history(&self) -> &History {
        &self.history
    }

    /// The history, for line up and down in an entry.
    pub fn history_mut(&mut self) -> &mut History {
        &mut self.history
    }

    /// The default channel word (§7.2: the word plain text runs when the mode has none).
    fn channel_word(&self) -> &str {
        self.text.default_word.as_deref().unwrap_or("say")
    }

    /// The unknown-command line.
    #[must_use]
    pub fn unknown_line(&self) -> String {
        self.text
            .unknown_line
            .clone()
            .unwrap_or_else(|| UNKNOWN_FALLBACK.replace("{p}", self.prefix()))
    }

    /// Runs one submitted line (`chat-window.md` §6.2 submit steps 3–4, §7.2) in the
    /// overlay's `Say` mode (the default channel word, no target); lines the client posts
    /// go to `out`.
    pub fn submit(
        &mut self,
        text: &str,
        ctx: &mut Context<'_>,
        out: &mut Vec<LocalLine>,
    ) -> Outcome {
        self.submit_in(text, "", "", ctx, out)
    }

    /// Runs one submitted line in a channel mode (`chat-window.md` §6.2 submit steps 3–4,
    /// §6.5, §7.2): plain text runs as `word` (the mode's command word *C*; empty: the
    /// default channel word) with the tell target `target` (*G*, empty outside `Tell`).
    /// Lines the client posts go to `out` (the unknown-command line among them, decision
    /// 0020 item 4).
    pub fn submit_in(
        &mut self,
        text: &str,
        word: &str,
        target: &str,
        ctx: &mut Context<'_>,
        out: &mut Vec<LocalLine>,
    ) -> Outcome {
        if text.is_empty() {
            return Outcome::Nothing;
        }
        if text.len() > line::MAX_LINE_LEN {
            out.push(LocalLine::error(TOO_LONG));
            return Outcome::TooLong;
        }
        if ctx.origin == Origin::Chat {
            self.history.add(text);
        }
        let word = if word.is_empty() {
            self.channel_word()
        } else {
            word
        };
        let prepared = line::prepare(text, &self.text.prefix, word, target);
        let inv = match prepared {
            Ok(Some(inv)) => inv,
            Ok(None) => return Outcome::Nothing,
            Err(LineError::TooLong) => {
                out.push(LocalLine::error(TOO_LONG));
                return Outcome::TooLong;
            }
        };
        // The guard is a temporary: the lock is released before any handler runs.
        let entry = self.registry().lookup(&inv.word).cloned();
        let Some(entry) = entry else {
            tracing::debug!(word = %inv.word, "unknown command");
            out.push(LocalLine::error(self.unknown_line()));
            return Outcome::Unknown;
        };
        match &entry.handler {
            Handler::Builtin(b) => self.run_builtin(*b, &inv, ctx, out),
            Handler::Plugin(id) => {
                if let (Owner::Plugin(ap), Some(p)) = (&entry.owner, ctx.plugins.as_deref_mut()) {
                    p.run_command(ap, *id, &inv.word, &inv.arguments);
                }
            }
        }
        Outcome::Ran
    }

    fn run_builtin(
        &self,
        b: Builtin,
        inv: &Invocation,
        ctx: &mut Context<'_>,
        out: &mut Vec<LocalLine>,
    ) {
        match b {
            Builtin::Music(word) => match ctx.music.as_deref_mut() {
                Some(m) => m.run_tokens(word, inv.args_tokens()),
                None => out.push(LocalLine::error(NO_MUSIC)),
            },
            Builtin::Help => self.help(inv, ctx, out),
            Builtin::Plugins => match ctx.plugins.as_deref_mut() {
                Some(p) => p.plugins_command(&inv.arguments),
                None => out.push(LocalLine::error(NO_PLUGINS)),
            },
            Builtin::Channel(ch) => self.channel(ch, inv, ctx, out),
            Builtin::Social(w) => social::run_word(self, w, inv, ctx, out),
            Builtin::P5(word) => p5::run(word, &inv.arguments, &self.p5, out),
        }
    }

    /// The usage line of a built-in by its Tessera name, with this dispatcher's prefix.
    #[must_use]
    pub fn usage(&self, name: &str) -> String {
        usage_text(self.prefix(), name)
    }

    fn channel(
        &self,
        ch: Channel,
        inv: &Invocation,
        ctx: &mut Context<'_>,
        out: &mut Vec<LocalLine>,
    ) {
        if ctx.origin == Origin::Startup {
            tracing::warn!(word = %inv.word, "start-up command line is a chat line; dropped, not sent");
            out.push(LocalLine::log(format!(
                "Start-up line not sent (chat): {}{}",
                self.prefix(),
                inv.word
            )));
            return;
        }
        if matches!(
            ch,
            Channel::Tell
                | Channel::Reply
                | Channel::Retell
                | Channel::Fellowship
                | Channel::Slot(_)
        ) {
            return social::run_channel(self, ch, inv, ctx, out);
        }
        let text = inv.arguments.as_str();
        let usage = || match channel_help(ch) {
            Help::Text { long, .. } => long,
            Help::Plugin => String::new(),
        };
        let Some(channel) = protocol_channel(ch) else {
            return out.push(LocalLine::error(CHANNEL_UNAVAILABLE));
        };
        if text.is_empty() {
            return out.push(LocalLine::error(format!("Usage: {}", usage())));
        }
        if text.len() > MAX_CHAT_LEN {
            return out.push(LocalLine::error(TOO_LONG));
        }
        let Some(session) = ctx.session.as_deref_mut() else {
            return out.push(LocalLine::error(NOT_CONNECTED));
        };
        if !session.send_chat(ctx.now, &channel, text) {
            out.push(LocalLine::error(NOT_IN_WORLD));
        }
    }

    /// The help groups: entries of one owner and handler, names sorted, groups ordered by
    /// their first name (of a snapshot, so that plugin help runs without the lock).
    fn groups(entries: &[Entry]) -> Vec<Vec<&Entry>> {
        let mut groups: Vec<Vec<&Entry>> = Vec::new();
        for e in entries {
            match groups.iter_mut().find(|g| {
                g.first()
                    .is_some_and(|f| f.owner == e.owner && f.handler == e.handler)
            }) {
                Some(g) => g.push(e),
                None => groups.push(vec![e]),
            }
        }
        groups
    }

    fn help(&self, inv: &Invocation, ctx: &mut Context<'_>, out: &mut Vec<LocalLine>) {
        let p = self.prefix();
        let plugin_help = |ctx: &mut Context<'_>, e: &Entry, long: bool| -> Option<String> {
            match (&e.owner, &e.handler) {
                (Owner::Plugin(ap), Handler::Plugin(id)) => ctx
                    .plugins
                    .as_deref_mut()
                    .and_then(|pl| pl.help(ap, *id, long)),
                _ => None,
            }
        };
        let Some(arg) = inv.args_tokens().first() else {
            out.push(LocalLine::log(format!(
                "Commands (type {p}help <command> for one command's help):"
            )));
            let entries: Vec<Entry> = self.registry().entries().cloned().collect();
            for g in Self::groups(&entries) {
                let Some(first) = g.first() else { continue };
                let names: Vec<String> = g.iter().map(|e| format!("{p}{}", e.name)).collect();
                let short = match &first.help {
                    Help::Text { short, .. } => Some(short.clone()),
                    Help::Plugin => plugin_help(ctx, first, false),
                };
                let line = match short {
                    Some(s) if !s.is_empty() => format!("  {} - {s}", names.join(", ")),
                    _ => format!("  {}", names.join(", ")),
                };
                out.push(LocalLine::log(line));
            }
            return;
        };
        let name = arg.trim_start_matches(|c: char| p.contains(c));
        let entry = self.registry().lookup(name).cloned();
        match entry.as_ref() {
            Some(e) => {
                let long = match &e.help {
                    Help::Text { long, .. } => Some(long.clone()),
                    Help::Plugin => plugin_help(ctx, e, true),
                };
                out.push(LocalLine::log(
                    long.unwrap_or_else(|| format!("{p}{}: no help.", e.name)),
                ));
            }
            None => out.push(LocalLine::error(format!("There is no command {p}{name}."))),
        }
    }
}

/// The chat window's line up and down (`chat-window.md` §6.3) browse the dispatcher's one
/// history.
impl EntryHistory for History {
    fn older(&mut self) -> Option<String> {
        History::older(self).map(str::to_owned)
    }

    fn newer(&mut self) -> Option<String> {
        History::newer(self).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::text::ModeText;
    use super::*;
    use crate::audio::player_music::MusicConfig;
    use crate::net::PlayerLocation;

    /// A session that records what it is asked to send.
    #[derive(Default)]
    struct Recorder {
        sent: Vec<(ChatChannel, String)>,
        commands: Vec<tessera_net::Command>,
        refuse: bool,
        remote: bool,
        social: Option<crate::social::SocialState>,
    }

    impl Recorder {
        /// A recorder with a social state for player "Tam" (the P4 built-ins).
        fn social() -> Self {
            let mut st = crate::social::SocialState::new(true);
            let _ = st.apply(
                &tessera_net::Event::WorldEntered {
                    player: tessera_net::EntityId(1),
                    name: "Tam".into(),
                    transform: tessera_net::Transform::new(tessera_net::Vec3::ZERO, 0.0),
                },
                &|_| None,
            );
            Self {
                social: Some(st),
                ..Self::default()
            }
        }
    }

    impl WalkerSession for Recorder {
        fn player_location(&self) -> Option<PlayerLocation> {
            None
        }
        fn send_move(
            &mut self,
            _: Instant,
            _: tessera_net::Location,
            _: f32,
            _: tessera_net::Vec3,
        ) {
        }
        fn net_status(&self) -> Option<crate::net::NetStatus> {
            Some(crate::net::NetStatus {
                backend: "test".into(),
                state: tessera_net::SessionState::InWorld,
                link: self.remote.then_some("open"),
                rtt: None,
                clock_offset_us: None,
                delay: None,
                entities: 0,
            })
        }
        fn send_chat(&mut self, _: Instant, channel: &ChatChannel, text: &str) -> bool {
            if self.refuse {
                return false;
            }
            self.sent.push((channel.clone(), text.to_owned()));
            true
        }
        fn social_mut(&mut self) -> Option<&mut crate::social::SocialState> {
            self.social.as_mut()
        }
        fn send_command(
            &mut self,
            _: Instant,
            command: tessera_net::Command,
        ) -> Result<(), String> {
            if self.refuse {
                return Err("not in the world".into());
            }
            match command {
                tessera_net::Command::Chat { channel, text } => self.sent.push((channel, text)),
                c => self.commands.push(c),
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakePlugins {
        plugins: Vec<String>,
        ran: Vec<(String, u64, String, String)>,
    }

    impl PluginCommands for FakePlugins {
        fn plugins_command(&mut self, arguments: &str) {
            self.plugins.push(arguments.to_owned());
        }
        fn run_command(&mut self, apartment: &str, handler: u64, word: &str, arguments: &str) {
            self.ran.push((
                apartment.to_owned(),
                handler,
                word.to_owned(),
                arguments.to_owned(),
            ));
        }
        fn help(&mut self, apartment: &str, handler: u64, long: bool) -> Option<String> {
            Some(format!(
                "{apartment}#{handler} {}",
                if long { "long" } else { "short" }
            ))
        }
    }

    fn modes(rows: &[(&str, Channel)]) -> Vec<ModeText> {
        rows.iter()
            .map(|(names, channel)| ModeText {
                names: (*names).to_owned(),
                chat_type: 5,
                kind: tessera_ui::game::chat::ModeKind::Plain,
                channel: *channel,
            })
            .collect()
    }

    fn run(
        d: &mut Dispatcher,
        line: &str,
        session: Option<&mut Recorder>,
        music: Option<&mut PlayerMusic>,
        origin: Origin,
    ) -> (Outcome, Vec<LocalLine>) {
        let mut out = Vec::new();
        let mut ctx = Context {
            now: Instant::now(),
            origin,
            session: session.map(|s| s as &mut dyn WalkerSession),
            music,
            plugins: None,
        };
        let o = d.submit(line, &mut ctx, &mut out);
        (o, out)
    }

    #[test]
    fn music_unknown_and_prefix_only_never_reach_the_session() {
        let mut d = Dispatcher::new();
        let mut s = Recorder::default();
        let mut m = PlayerMusic::new(MusicConfig::default());
        for line in ["/play tune", "/PlayStop", "/playlist", "/instrument lute"] {
            let (o, out) = run(&mut d, line, Some(&mut s), Some(&mut m), Origin::Chat);
            assert_eq!(o, Outcome::Ran, "{line}");
            assert!(out.is_empty(), "{line}: {out:?}");
        }
        assert_eq!(m.stats().commands, 4);
        let (o, out) = run(&mut d, "/nosuch thing", Some(&mut s), None, Origin::Chat);
        assert_eq!(o, Outcome::Unknown);
        assert_eq!(
            out,
            [LocalLine::error("Unknown command. Type /help for a list.")]
        );
        for line in ["/", " // ", "", "   "] {
            let (o, out) = run(&mut d, line, Some(&mut s), None, Origin::Chat);
            assert_eq!(o, Outcome::Nothing, "{line:?}");
            assert!(out.is_empty());
        }
        assert!(s.sent.is_empty(), "{:?}", s.sent);
        // Music without player music says so.
        let (_, out) = run(&mut d, "/play tune", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NO_MUSIC)]);
        assert!(s.sent.is_empty());
    }

    #[test]
    fn plain_text_and_channel_commands() {
        let mut d = Dispatcher::new();
        let mut s = Recorder::default();
        let (o, out) = run(&mut d, "  hello there ", Some(&mut s), None, Origin::Chat);
        assert_eq!((o, out.len()), (Outcome::Ran, 0));
        let (_, out) = run(&mut d, "/say /x", Some(&mut s), None, Origin::Chat);
        assert!(out.is_empty());
        run(&mut d, "/YELL hey", Some(&mut s), None, Origin::Chat);
        run(&mut d, "/emote waves", Some(&mut s), None, Origin::Chat);
        assert_eq!(
            s.sent,
            [
                (ChatChannel::Say, "hello there".to_owned()),
                (ChatChannel::Say, "/x".to_owned()),
                (ChatChannel::Yell, "hey".to_owned()),
                (ChatChannel::Emote, "waves".to_owned()),
            ]
        );
        // Tells need the session's social state (CT4); without one, not connected.
        let (_, out) = run(&mut d, "/tell Bram hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NOT_CONNECTED)]);
        let mut s = Recorder::social();
        run(
            &mut d,
            "/tell Bram  hi there",
            Some(&mut s),
            None,
            Origin::Chat,
        );
        run(
            &mut d,
            "/tell \"Old Bram\" hi",
            Some(&mut s),
            None,
            Origin::Chat,
        );
        assert_eq!(
            s.sent,
            [
                (
                    ChatChannel::Tell {
                        peer: "Bram".into()
                    },
                    "hi there".to_owned()
                ),
                (
                    ChatChannel::Tell {
                        peer: "Old Bram".into()
                    },
                    "hi".to_owned()
                ),
            ]
        );
        // Usage, no session, refused.
        let (_, out) = run(&mut d, "/tell Bram", Some(&mut s), None, Origin::Chat);
        assert!(out[0].text.starts_with("Usage: /tell"), "{out:?}");
        let (_, out) = run(&mut d, "/say", Some(&mut s), None, Origin::Chat);
        assert!(out[0].text.starts_with("Usage: /say"), "{out:?}");
        let (_, out) = run(&mut d, "hello", None, None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NOT_CONNECTED)]);
        s.refuse = true;
        let (_, out) = run(&mut d, "hello", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NOT_IN_WORLD)]);
    }

    #[test]
    fn startup_lines_never_chat() {
        let mut d = Dispatcher::new();
        let mut s = Recorder::default();
        for line in ["hello", "/say hi", "/tell a b"] {
            let (o, out) = run(&mut d, line, Some(&mut s), None, Origin::Startup);
            assert_eq!(o, Outcome::Ran);
            assert_eq!(out.len(), 1);
        }
        assert!(s.sent.is_empty());
        assert_eq!(
            d.history().lines().count(),
            0,
            "start-up lines are not history"
        );
    }

    #[test]
    fn install_words_and_builtins_cannot_be_taken() {
        let mut d = Dispatcher::new();
        let p = Owner::Plugin("ap".into());
        for name in ["play", "HELP", "say", "plugins"] {
            assert_eq!(
                d.registry()
                    .register(name, &p, &Handler::Plugin(1), &Help::Plugin),
                0,
                "{name}"
            );
        }
        d.apply_text(CommandText {
            prefix: "#".into(),
            default_word: Some("talk".into()),
            unknown_line: Some("Nope.".into()),
            help_names: Some("help; aide, hilfe".into()),
            plugins_names: Some("plugins;plugin".into()),
            marker: None,
            modes: modes(&[
                ("say;s", Channel::Say),
                ("fellowship;f", Channel::Unavailable),
                ("reply;r", Channel::Reply),
            ]),
            ..CommandText::default()
        });
        assert!(d.registry().is_command("AIDE"));
        assert!(d.registry().is_command("plugin"));
        assert!(!d.registry().is_command("#aide"));
        let mut s = Recorder::default();
        let (_, out) = run(&mut d, "#f hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(CHANNEL_UNAVAILABLE)]);
        // The reply word needs the session's social state (CT4).
        let (_, out) = run(&mut d, "#r hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NOT_CONNECTED)]);
        let (_, out) = run(&mut d, "#zzz", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error("Nope.")]);
        run(&mut d, "plain", Some(&mut s), None, Origin::Chat);
        run(&mut d, "#s short", Some(&mut s), None, Origin::Chat);
        // With `#` as the prefix, `/x` is plain text.
        run(&mut d, "/x", Some(&mut s), None, Origin::Chat);
        let texts: Vec<&str> = s.sent.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(texts, ["plain", "short", "/x"]);
        // Help lists aliases on one line, alphabetically, as Log lines.
        let (_, out) = run(&mut d, "#hilfe", Some(&mut s), None, Origin::Chat);
        assert!(out.iter().all(|l| l.kind == LineKind::Log));
        assert!(
            out.iter()
                .any(|l| l.text.contains("#aide, #help, #hilfe - list the commands")),
            "{out:?}"
        );
        let firsts: Vec<String> = out
            .iter()
            .skip(1)
            .map(|l| {
                l.text
                    .trim()
                    .split([',', ' '])
                    .next()
                    .unwrap_or("")
                    .to_lowercase()
            })
            .collect();
        let mut sorted = firsts.clone();
        sorted.sort();
        assert_eq!(firsts, sorted);
        let (_, out) = run(&mut d, "#help #play", Some(&mut s), None, Origin::Chat);
        assert_eq!(out.len(), 1);
        assert!(out[0].text.starts_with("/play <name>"), "{out:?}");
        let (_, out) = run(&mut d, "#help nothing", Some(&mut s), None, Origin::Chat);
        assert_eq!(out[0].kind, LineKind::Error);
    }

    /// `chat-window.md` §10 item 3, decision 0020 item 6: plugin commands live in the
    /// dispatcher's registry with their apartment as the owner; the dispatcher runs them
    /// through `run_command` with the word as typed and the §7.4 arguments; `/plugins` is
    /// a client built-in that calls the plugin manager; a word nobody registered is the
    /// unknown-command line (there is no second table to fall back to).
    #[test]
    fn plugins_bridge() {
        let mut d = Dispatcher::new();
        let owner = Owner::Plugin("ap".into());
        assert_eq!(
            d.registry()
                .register("hello;hi", &owner, &Handler::Plugin(7), &Help::Plugin),
            2
        );
        let mut fp = FakePlugins::default();
        let mut out = Vec::new();
        let mut ctx = Context {
            now: Instant::now(),
            origin: Origin::Chat,
            session: None,
            music: None,
            plugins: Some(&mut fp),
        };
        assert_eq!(
            d.submit("/plugins  list ", &mut ctx, &mut out),
            Outcome::Ran
        );
        assert_eq!(
            d.submit("/Hello  a \"b c\" ", &mut ctx, &mut out),
            Outcome::Ran
        );
        assert_eq!(d.submit("/other", &mut ctx, &mut out), Outcome::Unknown);
        assert_eq!(out.len(), 1);
        // Plugin help comes from the plugin when `/help` runs.
        out.clear();
        assert_eq!(d.submit("/help hi", &mut ctx, &mut out), Outcome::Ran);
        assert_eq!(out, [LocalLine::log("ap#7 long")]);
        assert_eq!(fp.plugins, ["list"]);
        assert_eq!(
            fp.ran,
            [(
                "ap".to_owned(),
                7,
                "Hello".to_owned(),
                "a \"b c\"".to_owned()
            )]
        );
        // The apartment's unload removes its names.
        assert_eq!(d.registry().unregister_owner(&owner), 2);
        assert!(!d.registry().is_command("hello"));
        // No plugin host: the plugin manager command says so.
        let (_, out) = run(&mut d, "/plugins list", None, None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NO_PLUGINS)]);
    }

    #[test]
    fn history_and_bounds() {
        let mut d = Dispatcher::new();
        run(&mut d, "/play a", None, None, Origin::Chat);
        run(&mut d, "hello", None, None, Origin::Chat);
        assert_eq!(
            d.history().lines().collect::<Vec<_>>(),
            ["hello", "/play a"]
        );
        let long = "x".repeat(line::MAX_LINE_LEN + 1);
        let (o, out) = run(&mut d, &long, None, None, Origin::Chat);
        assert_eq!(
            (o, out),
            (Outcome::TooLong, vec![LocalLine::error(TOO_LONG)])
        );
        assert_eq!(d.history().lines().count(), 2);
        let mut s = Recorder::default();
        let (_, out) = run(
            &mut d,
            &format!("/say {}", "y".repeat(MAX_CHAT_LEN + 1)),
            Some(&mut s),
            None,
            Origin::Chat,
        );
        assert_eq!(out, [LocalLine::error(TOO_LONG)]);
        assert!(s.sent.is_empty());
    }

    /// The M5 chat window's lines run in its channel mode (`chat-window.md` §6.2 submit,
    /// §6.5, §7.2 step 5; decision 0020 item 2): plain text goes to the mode's word with the
    /// tell target, an empty word is the default word, commands ignore the mode.
    #[test]
    fn entry_lines_run_in_the_windows_channel_mode() {
        let mut d = Dispatcher::new();
        d.apply_text(CommandText {
            prefix: "/".into(),
            default_word: Some("say".into()),
            unknown_line: None,
            help_names: None,
            plugins_names: None,
            marker: None,
            modes: modes(&[
                ("s;say", Channel::Say),
                ("t;tell", Channel::Tell),
                ("f", Channel::Unavailable),
            ]),
            ..CommandText::default()
        });
        let mut s = Recorder::social();
        let mut m = PlayerMusic::new(MusicConfig::default());
        let sub = |d: &mut Dispatcher,
                   text: &str,
                   word: &str,
                   target: &str,
                   s: &mut Recorder,
                   m: &mut PlayerMusic| {
            let mut out = Vec::new();
            let mut ctx = Context {
                now: Instant::now(),
                origin: Origin::Chat,
                session: Some(s),
                music: Some(m),
                plugins: None,
            };
            let o = d.submit_in(text, word, target, &mut ctx, &mut out);
            (o, out)
        };
        assert_eq!(sub(&mut d, "hi", "", "", &mut s, &mut m).0, Outcome::Ran);
        assert_eq!(
            sub(&mut d, "hi all", "s", "", &mut s, &mut m).0,
            Outcome::Ran
        );
        assert_eq!(
            sub(&mut d, " psst ", "t", "Bram", &mut s, &mut m).0,
            Outcome::Ran
        );
        // Fellowship-like modes the Tessera protocol lacks: a local error, nothing sent.
        let (o, out) = sub(&mut d, "hi", "f", "", &mut s, &mut m);
        assert_eq!(
            (o, out),
            (Outcome::Ran, vec![LocalLine::error(CHANNEL_UNAVAILABLE)])
        );
        // Commands ignore the mode: `/play` in the Tell mode is music, not a tell.
        let (o, out) = sub(&mut d, "/play tune", "t", "Bram", &mut s, &mut m);
        assert_eq!((o, out.len()), (Outcome::Ran, 0));
        assert_eq!(m.stats().commands, 1);
        // A mode word the registry does not hold: the unknown-command line, nothing sent.
        let (o, _) = sub(&mut d, "hi", "nosuch", "", &mut s, &mut m);
        assert_eq!(o, Outcome::Unknown);
        assert_eq!(
            s.sent,
            [
                (ChatChannel::Say, "hi".to_owned()),
                (ChatChannel::Say, "hi all".to_owned()),
                (
                    ChatChannel::Tell {
                        peer: "Bram".into()
                    },
                    "psst".to_owned()
                ),
            ]
        );
        // Every entry line is history, typed text only (§6.3).
        assert_eq!(
            d.history().lines().collect::<Vec<_>>(),
            ["hi", "/play tune", " psst ", "hi all"]
        );
        // The window's line up and down read it through `EntryHistory`.
        let h: &mut dyn EntryHistory = d.history_mut();
        assert_eq!(h.older().as_deref(), Some("hi"));
        assert_eq!(h.older().as_deref(), Some("/play tune"));
        assert_eq!(h.newer().as_deref(), Some("hi"));
        assert_eq!(h.newer(), None);
    }

    /// Hostile entry lines (any text, word or target) never panic and never send more
    /// than the protocol's chat length.
    #[test]
    fn hostile_entry_lines() {
        let mut d = Dispatcher::new();
        let mut s = Recorder::default();
        let long = "x".repeat(200_000);
        let words = ["", "say", "\u{0}", "\"", long.as_str(), "tell"];
        for text in [
            "",
            "/",
            "//",
            "/\"",
            "/plugins load \"",
            ";;;;",
            long.as_str(),
            "/\u{FFFF}\u{10FFFF}",
            "\"a\"\u{1F600}",
        ] {
            for word in words {
                for target in ["", "\"", long.as_str()] {
                    let mut out = Vec::new();
                    let mut ctx = Context {
                        now: Instant::now(),
                        origin: Origin::Chat,
                        session: Some(&mut s),
                        music: None,
                        plugins: None,
                    };
                    let _ = d.submit_in(text, word, target, &mut ctx, &mut out);
                }
            }
        }
        assert!(s.sent.iter().all(|(_, t)| t.len() <= MAX_CHAT_LEN));
        assert!(d.history().lines().count() < line::HISTORY_CAPACITY);
    }

    /// `chat-window.md` §15.10 CT5, CT7 and `social-panels.md` §9 TR11 through the
    /// dispatcher: channels join, speak and part; the fellowship refusal without one; list
    /// and fellowship commands as session commands; the lists printed; start-up lines never
    /// chat.
    #[test]
    fn social_built_ins() {
        use tessera_net::domain::{SocialCommand, SocialEvent};
        let mut d = Dispatcher::new();
        let mut s = Recorder::social();
        let lines = |d: &mut Dispatcher, s: &mut Recorder, line: &str| -> Vec<LocalLine> {
            run(d, line, Some(s), None, Origin::Chat).1
        };
        // CT5: join sends `JoinChannel`; nothing is shown until `ChannelJoined`.
        assert!(lines(&mut d, &mut s, "/join Lore").is_empty());
        assert_eq!(
            s.commands.pop(),
            Some(tessera_net::Command::Social(SocialCommand::JoinChannel {
                name: "Lore".into()
            }))
        );
        let st = s.social.as_mut().expect("social");
        let l = st.apply(
            &tessera_net::Event::Social(SocialEvent::ChannelJoined {
                channel: "Lore".into(),
            }),
            &|_| None,
        );
        assert_eq!(l.len(), 1);
        // A second join of the same name, a reserved name, a password.
        let out = lines(&mut d, &mut s, "/join lore");
        assert_eq!(out[0].mask, Some(0x8));
        assert!(s.commands.is_empty());
        let out = lines(&mut d, &mut s, "/join tell");
        assert_eq!(out.len(), 1);
        let out = lines(&mut d, &mut s, "/join Other secret");
        assert_eq!(out.len(), 1);
        assert!(s.commands.is_empty());
        // Speak by slot and by name.
        assert!(lines(&mut d, &mut s, "/ch1 any maps?").is_empty());
        assert!(lines(&mut d, &mut s, "/chsay Lore old maps").is_empty());
        assert_eq!(
            s.sent,
            [
                (
                    ChatChannel::Named {
                        name: "Lore".into()
                    },
                    "any maps?".to_owned()
                ),
                (
                    ChatChannel::Named {
                        name: "Lore".into()
                    },
                    "old maps".to_owned()
                ),
            ]
        );
        // An empty slot, the channel list, leaving.
        let out = lines(&mut d, &mut s, "/ch2 hello");
        assert_eq!(out[0].mask, Some(0x8));
        let out = lines(&mut d, &mut s, "/channels");
        assert!(out.iter().any(|l| l.text.contains("Lore")), "{out:?}");
        assert!(lines(&mut d, &mut s, "/part Lore").is_empty());
        assert_eq!(
            s.commands.pop(),
            Some(tessera_net::Command::Social(SocialCommand::LeaveChannel {
                name: "Lore".into()
            }))
        );
        // CT7: no fellowship, the refusal (mask 0x1), nothing sent.
        let before = s.sent.len();
        let out = lines(&mut d, &mut s, "/fellowship hello all");
        assert_eq!(out[0].mask, Some(0x1));
        assert_eq!(s.sent.len(), before);
        // TR11: list and fellowship commands.
        for (line, want) in [
            ("/friend add Brin", "AddFriend"),
            ("/friend REMOVE Brin", "RemoveFriend"),
            ("/ignore add Hask", "Ignore"),
            ("/ignore remove Hask", "Unignore"),
            ("/invite Brin", "InviteToFellowship"),
            ("/dismiss Brin", "KickFromFellowship"),
            ("/promote Brin", "SetFellowshipLeader"),
            ("/leave", "LeaveFellowship"),
            ("/disband", "DisbandFellowship"),
        ] {
            let out = lines(&mut d, &mut s, line);
            assert!(out.is_empty(), "{line}: {out:?}");
            assert_eq!(s.commands.pop().map(|c| c.name()), Some(want), "{line}");
        }
        // A missing name, an unknown sub-word: lines, nothing sent.
        assert_eq!(lines(&mut d, &mut s, "/invite")[0].kind, LineKind::Error);
        for line in ["/friend hug Brin", "/friend", "/ignore"] {
            let out = lines(&mut d, &mut s, line);
            assert_eq!(out.len(), 1, "{line}");
            // TR11: type 8 (mask 0x80) or type 1, never the standard type.
            assert_eq!(out[0].mask, Some(0x80), "{line}: {out:?}");
        }
        assert!(s.commands.is_empty());
        // Lists: empty lines (Tessera's wording without an install, CT12), then the model.
        let out = lines(&mut d, &mut s, "/friend list");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].mask, Some(0x80));
        assert!(!out[0].text.trim().is_empty(), "{out:?}");
        let out = lines(&mut d, &mut s, "/ignore list");
        assert_eq!(out.len(), 1);
        assert!(!out[0].text.trim().is_empty(), "{out:?}");
        let st = s.social.as_mut().expect("social");
        let _ = st.apply(
            &tessera_net::Event::Social(SocialEvent::FriendList {
                entries: vec![
                    tessera_net::domain::FriendEntry {
                        name: "Brin".into(),
                        online: true,
                    },
                    tessera_net::domain::FriendEntry {
                        name: "Cael".into(),
                        online: false,
                    },
                ],
            }),
            &|_| None,
        );
        let out = lines(&mut d, &mut s, "/friend list");
        let texts: Vec<&str> = out.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Brin online", "Cael offline"]);
        let st = s.social.as_mut().expect("social");
        let _ = st.apply(
            &tessera_net::Event::Social(SocialEvent::IgnoreList {
                names: vec!["Zed".into(), "Ann".into()],
            }),
            &|_| None,
        );
        let out = lines(&mut d, &mut s, "/ignore list");
        let texts: Vec<&str> = out.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts.len(), 3, "{texts:?}");
        assert!(!texts[0].trim().is_empty(), "the header (CT12 fallback)");
        assert_eq!(&texts[1..], ["Ann", "Zed"]);
        // Start-up lines never chat or act.
        let (_, out) = run(&mut d, "/join Lore", Some(&mut s), None, Origin::Startup);
        assert_eq!(out.len(), 1);
        assert!(s.commands.is_empty());
        // D20 item 7 drops only channel commands: the lists run from start-up lines.
        let (_, out) = run(&mut d, "/friend list", Some(&mut s), None, Origin::Startup);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out.iter().all(|l| !l.text.starts_with("Start-up line")));
        let (_, out) = run(&mut d, "/channels", Some(&mut s), None, Origin::Startup);
        assert!(
            out.iter().all(|l| !l.text.starts_with("Start-up line")),
            "{out:?}"
        );
        // Every P4 Tessera name is registered.
        for n in [
            "reply",
            "retell",
            "fellowship",
            "join",
            "part",
            "channels",
            "chsay",
            "ch8",
        ] {
            assert!(d.registry().is_command(n), "{n}");
        }
    }

    /// The install's P4 names (CT2, TR11) are registered for the same built-ins; the
    /// corrected reply and retell rows (CW43) run the reply and retell handlers.
    #[test]
    fn install_social_names() {
        let mut d = Dispatcher::new();
        d.apply_text(CommandText {
            modes: modes(&[("r", Channel::Reply), ("rt", Channel::Retell)]),
            social: vec![
                (SocialWord::Join, "joinchannel".into()),
                (SocialWord::Friend, "buddy".into()),
            ],
            sub_add: vec!["plus".into()],
            ..CommandText::default()
        });
        let mut s = Recorder::social();
        let (_, out) = run(&mut d, "/r hi", Some(&mut s), None, Origin::Chat);
        // No reply target yet (CT4): the 0x00CC89CC line with mask 0x21.
        assert_eq!(out[0].mask, Some(0x21));
        let _ = run(&mut d, "/tell Bo first", Some(&mut s), None, Origin::Chat);
        let _ = run(&mut d, "/rt again", Some(&mut s), None, Origin::Chat);
        assert_eq!(
            s.sent.last(),
            Some(&(ChatChannel::Tell { peer: "Bo".into() }, "again".to_owned()))
        );
        let _ = run(
            &mut d,
            "/joinchannel Lore",
            Some(&mut s),
            None,
            Origin::Chat,
        );
        let _ = run(&mut d, "/buddy plus Brin", Some(&mut s), None, Origin::Chat);
        let names: Vec<&str> = s.commands.iter().map(|c| c.name()).collect();
        assert_eq!(names, ["JoinChannel", "AddFriend"]);
    }

    #[test]
    fn split_tell_forms() {
        assert_eq!(split_tell("Bram hi there"), ("Bram", "hi there"));
        assert_eq!(split_tell("\"Old Bram\"  hi"), ("Old Bram", "hi"));
        assert_eq!(split_tell("\"open quote hi"), ("\"open", "quote hi"));
        assert_eq!(split_tell("Bram"), ("Bram", ""));
        assert_eq!(split_tell(""), ("", ""));
    }
}
