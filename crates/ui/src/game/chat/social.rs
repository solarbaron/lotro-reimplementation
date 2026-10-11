// SPDX-License-Identifier: GPL-3.0-or-later
//! The social chat of P4 (`chat-window.md` §15, Tessera rules §15.10 CT1–CT9): the line
//! forms of session chat on `Tell`, `Fellowship` and `Named` channels, the reply and retell
//! targets with the recent names, the tell, reply, retell, fellowship and user-channel
//! built-ins, the eight user-channel slots, the ignore filter and the friend online and
//! offline notices.
//!
//! This is a pure model. The client's command dispatcher (decision 0020) owns the registry
//! and runs the built-ins: it tokenizes a line (§7.3), then calls the matching method of
//! [`SocialChat`] with the tokens or the §7.4 arguments, posts the returned [`Notice`]s
//! (rendered by CT12, [`super::notice`]) and hands the returned [`ChatSend`] to the world
//! session as the matching command (`Chat`, `Social::JoinChannel`, `Social::LeaveChannel`).
//! Session events go through [`SocialChat::session_line`], [`SocialChat::channel_joined`],
//! [`SocialChat::channel_left`], [`SocialChat::set_ignore_list`],
//! [`SocialChat::friend_list`] and [`SocialChat::set_fellowship`]. The chat window keeps no
//! registry (decision 0020 item 1); it reads the tell targets through [`TellTargets`]
//! ([`super::window::Chat::set_tell_targets`]).

use std::collections::{HashMap, HashSet};

use super::notice::{
    self as n, MAX_TEXT_BYTES, Notice, TextRef, args as A, is_valid_name, name_equal, tables as T,
};

/// Slots of user chat channels (CW53).
pub const SLOTS: usize = 8;

/// The chat type of slot *k* = index + 1 (`chat-window.md` §15.6 table).
pub const SLOT_TYPES: [u32; SLOTS] = [28, 29, 30, 31, 39, 40, 41, 42];

/// The longest channel name in UTF-16 code units (§15.6 join step 3, CT5).
pub const MAX_CHANNEL_NAME_UNITS: usize = 50;

/// Names the recent-name lists keep (CT3).
pub const RECENT_NAMES: usize = 10;

/// Most ignored names and friend entries kept (Tessera cap; the feed allows 1,024 friends).
pub const MAX_LIST: usize = 4096;

/// Tessera's names of the channel commands, besides the install's names of the 26 fixed
/// mode-table rows (CT5 join: a channel may not be named like one of them).
pub const TESSERA_CHANNEL_COMMANDS: [&str; 15] = [
    "say",
    "yell",
    "emote",
    "tell",
    "reply",
    "retell",
    "fellowship",
    "ch1",
    "ch2",
    "ch3",
    "ch4",
    "ch5",
    "ch6",
    "ch7",
    "ch8",
];

/// The mask of slot `k` (1–8), or 0 outside.
#[must_use]
pub fn slot_mask(k: usize) -> u64 {
    match k.checked_sub(1).and_then(|i| SLOT_TYPES.get(i)) {
        Some(t) => 1u64 << (t - 1),
        None => 0,
    }
}

/// A session chat channel, as the session layer names it (`scenario-feed.md` §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel<'a> {
    /// `Say`.
    Say,
    /// `Yell`.
    Yell,
    /// `Emote`.
    Emote,
    /// `System`.
    System,
    /// `Tell { peer }`.
    Tell {
        /// The other party.
        peer: &'a str,
    },
    /// `Fellowship`.
    Fellowship,
    /// `Named { name }`.
    Named {
        /// The channel's name.
        name: &'a str,
    },
}

/// A line for the instant-message tab (CT6): the partner's session gets a line by the
/// speaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImLine {
    /// The session's partner (`peer` of the `Tell` line).
    pub partner: String,
    /// The speaker: *S* for an incoming line, the player for an own line.
    pub speaker: String,
    /// The text.
    pub text: String,
}

/// What became of a session chat line (CT1, CT8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionLine {
    /// Dropped (the ignore filter, CT8; a `Named` line for a channel in no slot, CT1):
    /// no log, no instant-message session, no sound, no plugin event.
    Dropped,
    /// Not a P4 channel (`Say`, `Yell`, `Emote`, `System`): shown as before (§10 item 2).
    Unchanged,
    /// A line for the logs, and for a `Tell` line its instant-message line.
    Line {
        /// The line.
        notice: Notice,
        /// The instant-message line (tells only).
        im: Option<ImLine>,
    },
}

/// A command for the world session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatSend {
    /// `Chat { Tell { peer }, text }`.
    Tell {
        /// The recipient.
        peer: String,
        /// The text.
        text: String,
    },
    /// `Chat { Fellowship, text }`.
    Fellowship {
        /// The text.
        text: String,
    },
    /// `Chat { Named { name }, text }`.
    Named {
        /// The channel (as the slot holds it).
        name: String,
        /// The text.
        text: String,
    },
    /// `Social::JoinChannel { name }`.
    Join {
        /// The channel.
        name: String,
    },
    /// `Social::LeaveChannel { name }`.
    Leave {
        /// The channel (as the slot holds it).
        name: String,
    },
}

/// What a built-in did: lines to post (in order) and at most one command to send.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatOutcome {
    /// Lines to post.
    pub lines: Vec<Notice>,
    /// The command to send.
    pub send: Option<ChatSend>,
}

impl ChatOutcome {
    fn line(n: Notice) -> Self {
        Self {
            lines: vec![n],
            send: None,
        }
    }

    fn send(s: ChatSend) -> Self {
        Self {
            lines: Vec::new(),
            send: Some(s),
        }
    }
}

/// A list of recent names, newest first, at most [`RECENT_NAMES`] distinct (CT3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentNames {
    names: Vec<String>,
}

impl RecentNames {
    /// The names, newest first.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Puts `name` first (a name-equal repeat moves to the front).
    pub fn push(&mut self, name: &str) {
        self.names.retain(|x| !name_equal(x, name));
        self.names.insert(0, name.to_string());
        self.names.truncate(RECENT_NAMES);
    }
}

/// The tell targets the chat window reads (CT3): the reply target R, the retell target T and
/// the recent incoming speakers and recipients.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TellTargets {
    /// R: the speaker of the most recent incoming tell that passed the ignore filter.
    pub reply: Option<String>,
    /// T: the recipient of the most recent `tell` handed to the session.
    pub retell: Option<String>,
    /// The last incoming tell speakers, newest first.
    pub senders: RecentNames,
    /// The last `tell` recipients, newest first.
    pub recipients: RecentNames,
}

/// The P4 social chat model (CT1, CT3–CT9).
#[derive(Debug, Clone, Default)]
pub struct SocialChat {
    player: String,
    targets: TellTargets,
    slots: [Option<String>; SLOTS],
    ignored: Vec<String>,
    /// `ignored`, lowercased: the filter's lookup (one hash lookup per line).
    ignored_lc: HashSet<String>,
    friends: Option<Vec<(String, bool)>>,
    in_fellowship: bool,
    user_chat: bool,
    channel_commands: Vec<String>,
}

impl SocialChat {
    /// A model for player `player` (*P*, the name the session entered the world with).
    /// `user_chat` says whether the backend offers user chat channels (CT11).
    #[must_use]
    pub fn new(player: &str, user_chat: bool) -> Self {
        Self {
            player: player.to_string(),
            user_chat,
            ..Self::default()
        }
    }

    /// The player's name *P*.
    #[must_use]
    pub fn player(&self) -> &str {
        &self.player
    }

    /// Whether the backend offers user chat channels (CT11: the scenario feed and the plain
    /// sandbox do; the test server does from protocol minor 2, `scenario-feed.md` §9.8).
    pub fn set_user_chat(&mut self, on: bool) {
        self.user_chat = on;
    }

    /// The install's names of the 26 fixed mode-table rows (§10 item 3), for the CT5
    /// channel-command test; the Tessera names of [`TESSERA_CHANNEL_COMMANDS`] are always
    /// included. At most [`MAX_LIST`] names are kept.
    pub fn set_channel_command_names<I: IntoIterator<Item = String>>(&mut self, names: I) {
        self.channel_commands = names.into_iter().take(MAX_LIST).collect();
    }

    fn is_channel_command(&self, name: &str) -> bool {
        TESSERA_CHANNEL_COMMANDS.iter().any(|c| name_equal(c, name))
            || self.channel_commands.iter().any(|c| name_equal(c, name))
    }

    /// A new world entry as `player` (CT3, CT5, CT8, CT9): no reply target, all slots free
    /// without lines, an empty ignore list, no friend list yet (the next is the first),
    /// no fellowship. The retell target and recent names start empty too.
    pub fn world_entered(&mut self, player: &str) {
        let user_chat = self.user_chat;
        let names = std::mem::take(&mut self.channel_commands);
        *self = Self::new(player, user_chat);
        self.channel_commands = names;
    }

    /// `Disconnected` (CT3, CT5, CT8, CT9): R cleared, slots freed without lines, the
    /// ignore list and the friend list cleared.
    pub fn disconnected(&mut self) {
        self.targets.reply = None;
        self.slots = Default::default();
        self.ignored.clear();
        self.ignored_lc.clear();
        self.friends = None;
        self.in_fellowship = false;
    }

    /// The tell targets (CT3).
    #[must_use]
    pub fn targets(&self) -> &TellTargets {
        &self.targets
    }

    /// The slots: index *k* − 1 holds slot *k*'s channel name.
    #[must_use]
    pub fn slots(&self) -> &[Option<String>; SLOTS] {
        &self.slots
    }

    /// The slot (1–8) holding a channel name-equal to `name`.
    #[must_use]
    pub fn slot_of(&self, name: &str) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.as_deref().is_some_and(|x| name_equal(x, name)))
            .map(|i| i + 1)
    }

    /// The ignored names (CT8).
    #[must_use]
    pub fn ignored(&self) -> &[String] {
        &self.ignored
    }

    /// Whether `name` is ignored (name-equal).
    #[must_use]
    pub fn is_ignored(&self, name: &str) -> bool {
        !self.ignored_lc.is_empty() && self.ignored_lc.contains(&name.to_lowercase())
    }

    /// `Social::IgnoreList { names }` (CT8): the list is replaced.
    pub fn set_ignore_list(&mut self, names: &[String]) {
        self.ignored = names.iter().take(MAX_LIST).cloned().collect();
        self.ignored_lc = self.ignored.iter().map(|n| n.to_lowercase()).collect();
    }

    /// `Social::FellowshipChanged`: whether the member list is non-empty (CT7, SP §9 TR2).
    pub fn set_fellowship(&mut self, non_empty: bool) {
        self.in_fellowship = non_empty;
    }

    /// Whether a fellowship exists (the last `FellowshipChanged` list is non-empty).
    #[must_use]
    pub fn in_fellowship(&self) -> bool {
        self.in_fellowship
    }

    /// The ignore filter (CT8): whether a line of `channel` spoken by `speaker` is dropped.
    #[must_use]
    pub fn filtered(&self, channel: Channel<'_>, speaker: Option<&str>) -> bool {
        if matches!(channel, Channel::System) {
            return false;
        }
        let Some(s) = speaker else { return false };
        !name_equal(s, &self.player) && self.is_ignored(s)
    }

    /// A `ChatMessage` (CT8, then CT1). Updates R and the recent speakers for an incoming
    /// tell that passes the filter.
    pub fn session_line(
        &mut self,
        channel: Channel<'_>,
        sender: Option<&str>,
        text: &str,
    ) -> SessionLine {
        let speaker = match channel {
            Channel::Tell { peer } => Some(sender.unwrap_or(peer)),
            _ => sender,
        };
        if self.filtered(channel, speaker) {
            return SessionLine::Dropped;
        }
        match channel {
            Channel::Say | Channel::Yell | Channel::Emote | Channel::System => {
                SessionLine::Unchanged
            }
            Channel::Tell { peer } => {
                let s = speaker.unwrap_or(peer);
                let own = name_equal(s, &self.player) && !name_equal(peer, &self.player);
                if own {
                    SessionLine::Line {
                        notice: Notice::of(
                            TextRef::new(T::CHAT, 0x0AD7_4A23)
                                .text(A::RECIPIENT, peer)
                                .text(A::TEXT, text),
                            n::TELL,
                        ),
                        im: Some(ImLine {
                            partner: peer.to_string(),
                            speaker: self.player.clone(),
                            text: text.to_string(),
                        }),
                    }
                } else {
                    self.targets.reply = Some(s.to_string());
                    self.targets.senders.push(s);
                    SessionLine::Line {
                        notice: Notice::of(
                            TextRef::new(T::CHAT, 0x0BCA_09CC)
                                .text(A::NAME, s)
                                .text(A::TEXT, text),
                            n::TELL,
                        ),
                        im: Some(ImLine {
                            partner: peer.to_string(),
                            speaker: s.to_string(),
                            text: text.to_string(),
                        }),
                    }
                }
            }
            Channel::Fellowship => {
                let notice = match speaker {
                    None => Notice::own(text, n::LOG),
                    Some(s) if name_equal(s, &self.player) => Notice::of(
                        TextRef::new(T::CHAT, 0x04A3_E744).text(A::TEXT, text),
                        n::FELLOWSHIP,
                    ),
                    Some(s) => Notice::of(
                        TextRef::new(T::CHAT, 0x05D2_0D84)
                            .text(A::SPEAKER, s)
                            .text(A::TEXT, text),
                        n::FELLOWSHIP,
                    ),
                };
                SessionLine::Line { notice, im: None }
            }
            Channel::Named { name } => {
                let Some(k) = self.slot_of(name) else {
                    tracing::debug!("chat line for a channel in no slot dropped");
                    return SessionLine::Dropped;
                };
                let held = self.slots[k - 1].clone().unwrap_or_default();
                let mask = slot_mask(k);
                let notice = match speaker {
                    None => Notice::own(text, n::LOG),
                    Some(s) if name_equal(s, &self.player) => Notice::of(
                        TextRef::new(T::CHAT, 0x009D_15CC)
                            .text(A::CHANNEL_ECHO, held)
                            .text(A::TEXT, text),
                        mask,
                    ),
                    Some(s) => Notice::of(
                        TextRef::new(T::CHAT, 0x063C_924C)
                            .text(A::CHANNEL, held)
                            .text(A::SPEAKER, s)
                            .text(A::TEXT, text),
                        mask,
                    ),
                };
                SessionLine::Line { notice, im: None }
            }
        }
    }

    fn text_too_long(text: &str) -> Option<ChatOutcome> {
        (text.len() > MAX_TEXT_BYTES).then(|| {
            ChatOutcome::line(Notice::own(
                format!("The text is longer than {MAX_TEXT_BYTES} bytes and was not sent."),
                n::ERROR,
            ))
        })
    }

    /// `tell` and the names of `Commands` 0x0DA4AC6C (CT4): `tokens` are the §7.3 tokens
    /// after the command word.
    pub fn tell(&mut self, tokens: &[String]) -> ChatOutcome {
        if tokens.len() < 2 {
            return ChatOutcome::line(Notice::usage("tell"));
        }
        let recipient = tokens[0].clone();
        let text = tokens[1..].join(" ");
        if text.is_empty() {
            return ChatOutcome {
                lines: vec![
                    Notice::of(
                        TextRef::new(T::CHAT, 0x01A1_7E05).text(A::RECIPIENT, recipient),
                        n::TELL_ERROR,
                    ),
                    Notice::usage("tell"),
                ],
                send: None,
            };
        }
        if !is_valid_name(&recipient) {
            return ChatOutcome::line(Notice::own("That is not a valid character name.", n::ERROR));
        }
        if let Some(o) = Self::text_too_long(&text) {
            return o;
        }
        self.targets.retell = Some(recipient.clone());
        self.targets.recipients.push(&recipient);
        ChatOutcome::send(ChatSend::Tell {
            peer: recipient,
            text,
        })
    }

    /// `reply` (CT4): `args` are the §7.4 arguments (sent as typed).
    pub fn reply(&mut self, args: &str) -> ChatOutcome {
        let target = self.targets.reply.clone();
        Self::tell_to(target, args, "reply", 0x00CC_89CC)
    }

    /// `retell` (CT4): `args` are the §7.4 arguments.
    pub fn retell(&mut self, args: &str) -> ChatOutcome {
        let target = self.targets.retell.clone();
        Self::tell_to(target, args, "retell", 0x0811_EFD4)
    }

    fn tell_to(target: Option<String>, args: &str, cmd: &str, none: u32) -> ChatOutcome {
        if args.is_empty() {
            return ChatOutcome::line(Notice::usage(cmd));
        }
        let Some(peer) = target else {
            return ChatOutcome::line(Notice::of(TextRef::new(T::CHAT, none), n::TELL_ERROR));
        };
        if let Some(o) = Self::text_too_long(args) {
            return o;
        }
        ChatOutcome::send(ChatSend::Tell {
            peer,
            text: args.to_string(),
        })
    }

    /// `ChatModeReply` (CT3): `Ok(R)` opens the entry in the `Tell` mode with target R;
    /// without R the line *Chat*(0x00CC89CC) with mask 0x21.
    pub fn chat_mode_reply(&self) -> Result<String, Notice> {
        self.targets
            .reply
            .clone()
            .ok_or_else(|| Notice::of(TextRef::new(T::CHAT, 0x00CC_89CC), n::TELL_ERROR))
    }

    /// `fellowship` and the fixed row 0x038453D4 (CT7): `text` is the §7.4 arguments.
    pub fn fellowship(&mut self, text: &str) -> ChatOutcome {
        if text.is_empty() {
            return ChatOutcome::line(Notice::usage("fellowship"));
        }
        if !self.in_fellowship {
            return ChatOutcome::line(Notice::of(
                TextRef::new(T::FELLOWSHIP, 0x0635_8717),
                n::ERROR,
            ));
        }
        if let Some(o) = Self::text_too_long(text) {
            return o;
        }
        ChatOutcome::send(ChatSend::Fellowship {
            text: text.to_string(),
        })
    }

    /// `join` and the names of 0x03A7284C (CT5): `tokens` after the command word.
    pub fn join(&mut self, tokens: &[String]) -> ChatOutcome {
        let line = |r: TextRef| ChatOutcome::line(Notice::of(r, n::STANDARD));
        if tokens.is_empty() || tokens.len() > 2 {
            return ChatOutcome::line(Notice::usage("join"));
        }
        if !self.user_chat {
            return line(TextRef::new(T::CHAT, 0x0406_8445));
        }
        let name = &tokens[0];
        let units = name.encode_utf16().count();
        if units > MAX_CHANNEL_NAME_UNITS || name.len() > n::MAX_NAME_BYTES {
            return line(
                TextRef::new(T::CHAT, 0x0283_2AE7)
                    .text(A::CHANNEL, name.clone())
                    .int(A::LENGTH, i64::try_from(units).unwrap_or(i64::MAX))
                    .int(A::LIMIT, MAX_CHANNEL_NAME_UNITS as i64),
            );
        }
        if !is_valid_name(name) {
            return line(TextRef::new(T::CHAT, 0x0AC8_421C));
        }
        if self.is_channel_command(name) {
            return line(TextRef::new(T::CHAT, 0x0191_9744).text(A::CHANNEL, name.clone()));
        }
        if tokens.len() == 2 {
            return line(TextRef::new(T::CHAT, 0x08DC_4764));
        }
        if self.slot_of(name).is_some() {
            return line(TextRef::new(T::CHAT, 0x0754_8D0C).text(A::CHANNEL_NAME, name.clone()));
        }
        if self.slots.iter().all(Option::is_some) {
            return line(TextRef::new(T::CHAT, 0x002A_AA94).int(A::COUNT, SLOTS as i64));
        }
        ChatOutcome::send(ChatSend::Join { name: name.clone() })
    }

    /// `Social::ChannelJoined { channel }` (CT5): the lowest free slot takes it.
    pub fn channel_joined(&mut self, channel: &str) -> Option<Notice> {
        if self.slot_of(channel).is_some() {
            tracing::debug!("channel joined twice; ignored");
            return None;
        }
        let Some(i) = self.slots.iter().position(Option::is_none) else {
            tracing::warn!("channel joined with every slot held; dropped");
            return None;
        };
        self.slots[i] = Some(channel.to_string());
        Some(Notice::of(
            TextRef::new(T::CHAT, 0x09A9_9A08).int(A::SLOT, i as i64 + 1),
            n::STANDARD,
        ))
    }

    /// `part` and the names of 0x0DCD958C (CT5).
    pub fn part(&mut self, tokens: &[String]) -> ChatOutcome {
        if tokens.len() != 1 {
            return ChatOutcome::line(Notice::usage("part"));
        }
        let name = &tokens[0];
        if let Some(k) = self.slot_of(name) {
            let held = self.slots[k - 1].clone().unwrap_or_default();
            return ChatOutcome::send(ChatSend::Leave { name: held });
        }
        let r = if self.is_channel_command(name) {
            TextRef::new(T::CHAT, 0x03C0_43D4).text(A::CHANNEL, name.clone())
        } else {
            TextRef::new(T::CHAT, 0x0DF4_808C).text(A::CHANNEL_NAME, name.clone())
        };
        ChatOutcome::line(Notice::of(r, n::STANDARD))
    }

    /// `Social::ChannelLeft { channel }` (CT5).
    pub fn channel_left(&mut self, channel: &str) -> Option<Notice> {
        let Some(k) = self.slot_of(channel) else {
            tracing::debug!("left a channel in no slot; ignored");
            return None;
        };
        self.slots[k - 1] = None;
        Some(Notice::of(
            TextRef::new(T::CHAT, 0x03B0_95F8).int(A::SLOT, k as i64),
            n::STANDARD,
        ))
    }

    /// `channels` and the names of 0x0A55AF03 (CT5, §15.6 list).
    #[must_use]
    pub fn channels(&self) -> Vec<Notice> {
        if self.slots.iter().all(Option::is_none) {
            return vec![Notice::of(TextRef::new(T::CHAT, 0x0151_3613), n::STANDARD)];
        }
        let mut out = vec![Notice::of(TextRef::new(T::CHAT, 0x09AA_EAC2), n::STANDARD)];
        for (i, s) in self.slots.iter().enumerate() {
            let k = i + 1;
            let r = match s {
                Some(name) => TextRef::new(T::CHAT, 0x000E_E4F4)
                    .int(A::SLOT, k as i64)
                    .text(A::CHANNEL_NAME, name.clone())
                    .nested(A::PASSWORD_PART, TextRef::new(T::CHAT, 0x02E8_19D4)),
                None => TextRef::new(T::CHAT, 0x0EBE_4334).int(A::SLOT, k as i64),
            };
            out.push(Notice::of(r, slot_mask(k)));
        }
        out
    }

    /// `ch`*k* and the slot rows 0x08B3A4E1 … 0x08B3A4E8 (CT5): `text` is the §7.4
    /// arguments. A `k` outside 1–8 gives the usage.
    pub fn slot_say(&mut self, k: usize, text: &str) -> ChatOutcome {
        if !(1..=SLOTS).contains(&k) || text.is_empty() {
            return ChatOutcome::line(Notice::usage(&format!("ch{}", k.min(SLOTS))));
        }
        if !self.user_chat {
            return ChatOutcome::line(Notice::of(TextRef::new(T::CHAT, 0x0406_8445), n::ERROR));
        }
        let Some(name) = self.slots[k - 1].clone() else {
            return ChatOutcome::line(Notice::of(
                TextRef::new(T::CHAT, 0x0E2A_7CE8).int(A::COUNT, k as i64),
                n::STANDARD,
            ));
        };
        if let Some(o) = Self::text_too_long(text) {
            return o;
        }
        ChatOutcome::send(ChatSend::Named {
            name,
            text: text.to_string(),
        })
    }

    /// `chsay` and the names of 0x098B3A44 (CT5).
    pub fn chsay(&mut self, tokens: &[String]) -> ChatOutcome {
        let Some(name) = tokens.first() else {
            return ChatOutcome::line(Notice::usage("chsay"));
        };
        let Some(k) = self.slot_of(name) else {
            return ChatOutcome::line(Notice::of(
                TextRef::new(T::CHAT, 0x02E3_2555).text(A::CHANNEL_NAME, name.clone()),
                n::STANDARD,
            ));
        };
        let text = tokens[1..].join(" ");
        if text.is_empty() {
            return ChatOutcome::line(Notice::usage("chsay"));
        }
        self.slot_say(k, &text)
    }

    /// `Social::FriendList { entries }` (CT9): online and offline notices against the
    /// previous list; the first list after a world entry posts none.
    pub fn friend_list(&mut self, entries: &[(String, bool)]) -> Vec<Notice> {
        let new: Vec<(String, bool)> = entries.iter().take(MAX_LIST).cloned().collect();
        let Some(old) = self.friends.replace(new.clone()) else {
            return Vec::new();
        };
        let mut before: HashMap<String, bool> = HashMap::new();
        for (name, on) in &old {
            before.entry(name.to_lowercase()).or_insert(*on);
        }
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (name, on) in &new {
            let key = name.to_lowercase();
            if !seen.insert(key.clone()) {
                continue;
            }
            match before.get(&key) {
                Some(was) if was != on => {
                    let token = if *on { 0x0A29_AFC5 } else { 0x0105_38E5 };
                    out.push(Notice::of(
                        TextRef::new(T::CHAT, token).text(A::NAME, name.clone()),
                        n::STANDARD,
                    ));
                }
                _ => {}
            }
        }
        out
    }
}
