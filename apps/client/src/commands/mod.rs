// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's one command dispatcher (decision 0020): the registry of
//! `chat-window.md` §7.1 and §8.2 ([`registry`]), the line pipeline of §7.2–§7.4
//! ([`line`](mod@line)), the words read from the install ([`text`]) and the client's built-in
//! commands. Every source of typed command text feeds [`Dispatcher::submit`]: the walker's
//! chat overlay, the start-up lines of `--command`, and (M5) the chat window's entry line.
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

pub mod line;
pub mod registry;
pub mod text;

use std::time::Instant;

use tessera_net::ChatChannel;
use tessera_net::model::MAX_CHAT_LEN;

use crate::audio::player_music::{MusicWord, PlayerMusic};
use crate::walker::WalkerSession;

use self::line::{History, Invocation, LineError};
use self::registry::{Builtin, Channel, Entry, Handler, Help, Owner, Registry};
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
/// The line of a tell in the sandbox (decision 0020 item 3).
pub const NO_TELL_PEERS: &str = "There is no one to tell in the sandbox.";
/// The line of a reply or tell-again word (no reply target in Tessera).
pub const NO_REPLY_TARGET: &str = "There is no one to reply to.";
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
}

impl LocalLine {
    /// A type 1 `Error` line.
    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            kind: LineKind::Error,
            text: text.into(),
        }
    }

    /// A type 8 `Log` line.
    #[must_use]
    pub fn log(text: impl Into<String>) -> Self {
        Self {
            kind: LineKind::Log,
            text: text.into(),
        }
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

/// The plugin side of the dispatcher (decision 0020 item 6). Until M5 connects
/// `tessera-script` to the registry, the client implements it on the plugin manager with
/// two transitional calls ([`Self::plugins_command`], [`Self::run_unregistered`]).
pub trait PluginCommands {
    /// `/plugins <arguments>` (`lua-plugin-api.md` §6.4).
    fn plugins_command(&mut self, arguments: &str);
    /// Transitional (until M5): runs a word the client registry does not hold through the
    /// plugin manager's own command table; `false` when no plugin has it.
    fn run_unregistered(&mut self, word: &str, arguments: &str) -> bool;
    /// Runs a plugin command of the registry (M5 `run_command(apartment, word, args)`).
    fn run_command(&mut self, apartment: &str, handler: u64, word: &str, arguments: &str) {
        let _ = (apartment, handler, word, arguments);
    }
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

/// The command dispatcher (module docs).
#[derive(Debug, Clone)]
pub struct Dispatcher {
    registry: Registry,
    text: CommandText,
    history: History,
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
        Channel::Say => text_help("say something nearby", "/say <text>: says the text on the local channel.".into()),
        Channel::Yell => text_help("yell something", "/yell <text>: yells the text, heard further away.".into()),
        Channel::Emote => text_help("act something out", "/emote <text>: shows the text as an emote.".into()),
        Channel::Tell => text_help(
            "send a private message",
            "/tell <name> <text>: sends the text to one character (quote a name with spaces).".into(),
        ),
        Channel::TellReply => text_help(
            "reply to the last private message",
            "Replies to the last private message; Tessera keeps no reply target, so this posts a notice.".into(),
        ),
        Channel::Unavailable => text_help(
            "a channel not available here",
            "A chat channel the Tessera protocol does not carry; using it posts a notice.".into(),
        ),
    }
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
    ] {
        v.push((name, Builtin::Channel(ch), channel_help(ch)));
    }
    v
}

/// The protocol channel of a sending channel command.
fn protocol_channel(ch: Channel, peer: &str) -> Option<ChatChannel> {
    match ch {
        Channel::Say => Some(ChatChannel::Say),
        Channel::Yell => Some(ChatChannel::Yell),
        Channel::Emote => Some(ChatChannel::Emote),
        Channel::Tell => Some(ChatChannel::Tell {
            peer: peer.to_owned(),
        }),
        Channel::TellReply | Channel::Unavailable => None,
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

/// Whether the session is a remote one (`--server`), which carries tells.
fn is_remote(session: &dyn WalkerSession) -> bool {
    session.net_status().is_some_and(|s| s.link.is_some())
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
            registry,
            text: CommandText::default(),
            history: History::new(),
        }
    }

    /// Takes the install's words (`chat-window.md` §10 items 3, 4, 6): the prefix, the
    /// unknown-command line, and the original names of help, the plugin manager and the
    /// channels, registered for the same built-ins (a name already held is skipped).
    pub fn apply_text(&mut self, text: CommandText) {
        let client = Owner::Client;
        let mut added = 0;
        let mut add = |reg: &mut Registry, names: &str, b: Builtin| {
            let help = match b {
                Builtin::Channel(ch) => channel_help(ch),
                _ => builtins()
                    .into_iter()
                    .find(|(_, x, _)| *x == b)
                    .map_or(Help::Plugin, |(_, _, h)| h),
            };
            added += reg.register(names, &client, &Handler::Builtin(b), &help);
        };
        if let Some(n) = &text.help_names {
            add(&mut self.registry, n, Builtin::Help);
        }
        if let Some(n) = &text.plugins_names {
            add(&mut self.registry, n, Builtin::Plugins);
        }
        // The default channel word runs plain text (§7.2 step 5), so it must be registered.
        if let Some(w) = &text.default_word {
            add(&mut self.registry, w, Builtin::Channel(Channel::Say));
        }
        for (names, ch) in &text.channel_words {
            add(&mut self.registry, names, Builtin::Channel(*ch));
        }
        tracing::info!(added, prefix = %text.prefix, "install command names registered");
        self.text = text;
    }

    /// The registry.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The registry, for plugin registrations (M5, decision 0020 item 6).
    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
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

    /// The channel word plain text runs (§7.2: the default word when the mode has none).
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
    /// overlay's `Say` mode; lines the client posts go to `out`.
    pub fn submit(
        &mut self,
        text: &str,
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
        let prepared = line::prepare(text, &self.text.prefix, self.channel_word(), "");
        let inv = match prepared {
            Ok(Some(inv)) => inv,
            Ok(None) => return Outcome::Nothing,
            Err(LineError::TooLong) => {
                out.push(LocalLine::error(TOO_LONG));
                return Outcome::TooLong;
            }
        };
        let Some(entry) = self.registry.lookup(&inv.word).cloned() else {
            if !inv.word.contains(line::is_whitespace)
                && let Some(p) = ctx.plugins.as_deref_mut()
                && p.run_unregistered(&inv.word, &inv.arguments)
            {
                return Outcome::Ran;
            }
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
        }
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
        let (peer, text) = match ch {
            Channel::Unavailable => return out.push(LocalLine::error(CHANNEL_UNAVAILABLE)),
            Channel::TellReply => return out.push(LocalLine::error(NO_REPLY_TARGET)),
            Channel::Tell => split_tell(&inv.arguments),
            _ => ("", inv.arguments.as_str()),
        };
        let usage = || match channel_help(ch) {
            Help::Text { long, .. } => long,
            Help::Plugin => String::new(),
        };
        if text.is_empty() || (ch == Channel::Tell && peer.is_empty()) {
            return out.push(LocalLine::error(format!("Usage: {}", usage())));
        }
        if text.len() > MAX_CHAT_LEN {
            return out.push(LocalLine::error(TOO_LONG));
        }
        let Some(channel) = protocol_channel(ch, peer) else {
            return out.push(LocalLine::error(CHANNEL_UNAVAILABLE));
        };
        let Some(session) = ctx.session.as_deref_mut() else {
            return out.push(LocalLine::error(NOT_CONNECTED));
        };
        if ch == Channel::Tell && !is_remote(session) {
            return out.push(LocalLine::error(NO_TELL_PEERS));
        }
        if !session.send_chat(ctx.now, &channel, text) {
            out.push(LocalLine::error(NOT_IN_WORLD));
        }
    }

    /// The help groups: entries of one owner and handler, names sorted, groups ordered by
    /// their first name.
    fn groups(&self) -> Vec<Vec<&Entry>> {
        let mut groups: Vec<Vec<&Entry>> = Vec::new();
        for e in self.registry.entries() {
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
            for g in self.groups() {
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
        match self.registry.lookup(name) {
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

    /// Every name the client owns (lower case), for the plugin host's `IsCommand` and
    /// `GetCommands` until M5 (`HostApi::is_builtin_command`, `builtin_commands`).
    #[must_use]
    pub fn client_names(&self) -> Vec<String> {
        self.registry
            .entries()
            .filter(|e| e.owner == Owner::Client)
            .map(|e| registry::key(&e.name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::player_music::MusicConfig;
    use crate::net::PlayerLocation;

    /// A session that records what it is asked to send.
    #[derive(Default)]
    struct Recorder {
        sent: Vec<(ChatChannel, String)>,
        refuse: bool,
        remote: bool,
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
    }

    #[derive(Default)]
    struct FakePlugins {
        plugins: Vec<String>,
        known: Vec<String>,
        ran: Vec<(String, String)>,
    }

    impl PluginCommands for FakePlugins {
        fn plugins_command(&mut self, arguments: &str) {
            self.plugins.push(arguments.to_owned());
        }
        fn run_unregistered(&mut self, word: &str, arguments: &str) -> bool {
            if self.known.iter().any(|k| k.eq_ignore_ascii_case(word)) {
                self.ran.push((word.to_owned(), arguments.to_owned()));
                true
            } else {
                false
            }
        }
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
        // Tells: not in the sandbox; on a server, to the first token.
        let (_, out) = run(&mut d, "/tell Bram hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NO_TELL_PEERS)]);
        s.remote = true;
        s.sent.clear();
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
                d.registry_mut()
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
            channel_words: vec![
                ("say;s".into(), Channel::Say),
                ("fellowship;f".into(), Channel::Unavailable),
                ("reply;r".into(), Channel::TellReply),
            ],
        });
        assert!(d.registry().is_command("AIDE"));
        assert!(d.registry().is_command("plugin"));
        assert!(!d.registry().is_command("#aide"));
        let mut s = Recorder::default();
        let (_, out) = run(&mut d, "#f hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(CHANNEL_UNAVAILABLE)]);
        let (_, out) = run(&mut d, "#r hi", Some(&mut s), None, Origin::Chat);
        assert_eq!(out, [LocalLine::error(NO_REPLY_TARGET)]);
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

    #[test]
    fn plugins_bridge() {
        let mut d = Dispatcher::new();
        let mut fp = FakePlugins {
            known: vec!["hello".into()],
            ..FakePlugins::default()
        };
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
        assert_eq!(fp.plugins, ["list"]);
        assert_eq!(fp.ran, [("Hello".to_owned(), "a \"b c\"".to_owned())]);
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

    #[test]
    fn split_tell_forms() {
        assert_eq!(split_tell("Bram hi there"), ("Bram", "hi there"));
        assert_eq!(split_tell("\"Old Bram\"  hi"), ("Old Bram", "hi"));
        assert_eq!(split_tell("\"open quote hi"), ("\"open", "quote hi"));
        assert_eq!(split_tell("Bram"), ("Bram", ""));
        assert_eq!(split_tell(""), ("", ""));
    }
}
