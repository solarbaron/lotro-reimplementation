// SPDX-License-Identifier: GPL-3.0-or-later
//! Text references and chat notices of the P4 social chat and panels (`chat-window.md` §15,
//! `social-panels.md`).
//!
//! The specs name every game string by its table (a STRINGTABLE **key**, `records.md` §8.2)
//! and token, with the argument IDs the entry declares; no game text is reproduced
//! (`chat-window.md` §15.10 CT12, `social-panels.md` §2). `tessera-ui` has no string tables:
//! the model and the widgets produce [`TextRef`]s and [`Notice`]s, and the client renders
//! them through its string resolver ([`TextSource`]). When a template is missing (no
//! install, another language without the token) [`fallback_text`] gives **Tessera's own
//! wording** with the same arguments, so the mask and the line still appear (CT12).
//!
//! Values that come from the session (names, channel names, chat text) are
//! [`Arg::Text`]: the renderer inserts them as **literal text**, interpreting neither the
//! message grammar nor markup inside them (CT12). The argument 0x4DD4 (an instance ID) is
//! never given (the session layer has none): the renderer removes the `Select` link markup
//! around the name and keeps its label (CT12).

/// STRINGTABLE keys of the tables the P4 specs name (looked up by key at run time).
pub mod tables {
    /// `PlayerActionHandler` (`social-panels.md` §2, PA).
    pub const PLAYER_ACTION: u32 = 0x1000_0001;
    /// `Commands` (CMD).
    pub const COMMANDS: u32 = 0x1000_0007;
    /// `Fellowship` (FT).
    pub const FELLOWSHIP: u32 = 0x1000_0017;
    /// `UI_Social` (US).
    pub const UI_SOCIAL: u32 = 0x1000_002F;
    /// `UI_SecureTrade` (UST).
    pub const UI_SECURE_TRADE: u32 = 0x1000_0031;
    /// The table of the "Weenie" lines of `chat-window.md` §15.6–§15.7.
    pub const WEENIE: u32 = 0x1000_0039;
    /// `CommandHelp` (CH).
    pub const COMMAND_HELP: u32 = 0x1000_004C;
    /// `Chat` (CT).
    pub const CHAT: u32 = 0x1000_004D;
    /// `UI_Friends` (UF).
    pub const UI_FRIENDS: u32 = 0x1000_005E;
    /// `Trade` (TT).
    pub const TRADE: u32 = 0x1000_0062;
    /// `UI`.
    pub const UI: u32 = 0x1000_0065;
    /// `UI_Guild` (UG).
    pub const UI_GUILD: u32 = 0x1000_0066;
}

/// Argument IDs the P4 entries declare (`chat-window.md` §15, `social-panels.md`).
pub mod args {
    /// A player name (the usual name argument).
    pub const NAME: u32 = 0x0005_2615;
    /// Chat text.
    pub const TEXT: u32 = 0x0000_5277;
    /// The recipient's name of an own tell and of the empty-tell line.
    pub const RECIPIENT: u32 = 0x0586_6BA4;
    /// The current time as text (instant-message lines).
    pub const TIME: u32 = 0x0005_8E15;
    /// The speaker of a fellowship or channel line.
    pub const SPEAKER: u32 = 0x0849_5FF2;
    /// A channel's name in a received channel line and in the join refusals.
    pub const CHANNEL: u32 = 0x07C6_32DC;
    /// A channel's name in the own-echo channel line.
    pub const CHANNEL_ECHO: u32 = 0x0AD9_9BC5;
    /// A channel's name in the joined, not-joined and list lines.
    pub const CHANNEL_NAME: u32 = 0x02E8_E425;
    /// A slot number (joined, left, list).
    pub const SLOT: u32 = 0x004E_28A8;
    /// A slot number or count (the full and free-slot lines).
    pub const COUNT: u32 = 0x0000_539D;
    /// A length (the name-too-long line).
    pub const LENGTH: u32 = 0x050A_2C88;
    /// A limit (the name-too-long line).
    pub const LIMIT: u32 = 0x0D0A_7DE8;
    /// The password part of a list line.
    pub const PASSWORD_PART: u32 = 0x012E_6A6F;
    /// A password.
    pub const PASSWORD: u32 = 0x0688_C124;
    /// The member's name of the fellowship out-of-range notices (`social-panels.md` §4.4).
    pub const MEMBER: u32 = 0x04AA_1147;
    /// A level (`social-panels.md` §3.3).
    pub const LEVEL: u32 = 0x0050_AA9C;
    /// An area name (`social-panels.md` §3.3).
    pub const AREA: u32 = 0x0376_892E;
    /// The online or away state text of the last-online column (`social-panels.md` §3.3).
    pub const ONLINE_STATE: u32 = 0x0948_0CA5;
    /// The friend's name of the note question (`social-panels.md` §3.5).
    pub const NOTE_NAME: u32 = 0x0A2D_94C5;
    /// The name of the ignore row menu's heading (`social-panels.md` §3.6).
    pub const IGNORE_NAME: u32 = 0x046E_FA25;
    /// The level of a fellowship row's level text (`social-panels.md` §4.2, SP28).
    pub const FELLOW_LEVEL: u32 = 0x007C_83B5;
    /// A coin element's count (`npc-windows.md` §9.2).
    pub const COIN_COUNT: u32 = 0x0052_42A9;
}

/// An argument value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// Literal text from the session or the player (inserted as is, no markup).
    Text(String),
    /// A number.
    Int(i64),
    /// Another string-table entry, rendered first.
    Ref(Box<TextRef>),
}

/// A string-table entry with its argument values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRef {
    /// The table's STRINGTABLE key ([`tables`]).
    pub table: u32,
    /// The token.
    pub token: u32,
    /// (argument ID, value), in the order the spec names them.
    pub args: Vec<(u32, Arg)>,
}

impl TextRef {
    /// An entry without arguments.
    #[must_use]
    pub fn new(table: u32, token: u32) -> Self {
        Self {
            table,
            token,
            args: Vec::new(),
        }
    }

    /// Adds a literal text argument.
    #[must_use]
    pub fn text(mut self, id: u32, v: impl Into<String>) -> Self {
        self.args.push((id, Arg::Text(v.into())));
        self
    }

    /// Adds a number argument.
    #[must_use]
    pub fn int(mut self, id: u32, v: i64) -> Self {
        self.args.push((id, Arg::Int(v)));
        self
    }

    /// Adds a nested entry as an argument.
    #[must_use]
    pub fn nested(mut self, id: u32, r: TextRef) -> Self {
        self.args.push((id, Arg::Ref(Box::new(r))));
        self
    }

    /// The value of argument `id`.
    #[must_use]
    pub fn arg(&self, id: u32) -> Option<&Arg> {
        self.args.iter().find(|(a, _)| *a == id).map(|(_, v)| v)
    }

    /// Renders through `src`, else [`fallback_text`]; nested entries likewise.
    pub fn render(&self, src: &mut dyn TextSource) -> String {
        let mut depth = 0;
        render_depth(self, src, &mut depth)
    }
}

/// Most nesting [`TextRef::render`] follows (hostile input; the specs nest one level).
const MAX_NESTING: usize = 4;

fn render_depth(r: &TextRef, src: &mut dyn TextSource, depth: &mut usize) -> String {
    if *depth >= MAX_NESTING {
        return String::new();
    }
    *depth += 1;
    let flat: Vec<(u32, String)> = r
        .args
        .iter()
        .map(|(id, a)| {
            let s = match a {
                Arg::Text(t) => t.clone(),
                Arg::Int(n) => n.to_string(),
                Arg::Ref(inner) => render_depth(inner, src, depth),
            };
            (*id, s)
        })
        .collect();
    *depth -= 1;
    src.render(r.table, r.token, &flat)
        .unwrap_or_else(|| fallback_text(r.table, r.token, &flat))
}

/// The client's string resolver: renders a table entry with flat argument values (each
/// inserted as literal text, `chat-window.md` §15.10 CT12), or `None` when the entry is
/// missing.
pub trait TextSource {
    /// Renders entry (`table` key, `token`) with `args` ((argument ID, value) pairs).
    fn render(&mut self, table: u32, token: u32, args: &[(u32, String)]) -> Option<String>;
}

/// No string tables: every text is Tessera's own wording ([`fallback_text`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTables;

impl TextSource for NoTables {
    fn render(&mut self, _: u32, _: u32, _: &[(u32, String)]) -> Option<String> {
        None
    }
}

/// What a notice line says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeText {
    /// A string-table entry (CT12).
    Ref(TextRef),
    /// The usage text of a client built-in, by its Tessera name (D20 item 5: the
    /// dispatcher's own usage text).
    Usage(String),
    /// Tessera's own words (refusals the specs leave to Tessera, `chat-window.md` §10
    /// item 12) or the session's own text (a `Fellowship` or `Named` line without a
    /// sender, CT1 last rows).
    Own(String),
}

/// A line for the chat logs: what it says and its 64-bit chat-type mask
/// (`chat-window.md` §2, §15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The text.
    pub text: NoticeText,
    /// The mask (CW-I30: the one the §15 tables give, whatever the text's source).
    pub mask: u64,
}

impl Notice {
    /// A string-table line.
    #[must_use]
    pub fn of(r: TextRef, mask: u64) -> Self {
        Self {
            text: NoticeText::Ref(r),
            mask,
        }
    }

    /// A usage line (mask 0x8, `chat-window.md` §15.10 CT2).
    #[must_use]
    pub fn usage(command: &str) -> Self {
        Self {
            text: NoticeText::Usage(command.to_string()),
            mask: STANDARD,
        }
    }

    /// A line in Tessera's own words.
    #[must_use]
    pub fn own(text: impl Into<String>, mask: u64) -> Self {
        Self {
            text: NoticeText::Own(text.into()),
            mask,
        }
    }

    /// The string-table entry, if the line has one.
    #[must_use]
    pub fn text_ref(&self) -> Option<&TextRef> {
        match &self.text {
            NoticeText::Ref(r) => Some(r),
            _ => None,
        }
    }

    /// Renders the line: a table entry through `src` (else Tessera's wording), a usage
    /// line through `usage` (the dispatcher's usage text of that command), own text as is.
    pub fn render(&self, src: &mut dyn TextSource, usage: &dyn Fn(&str) -> String) -> String {
        match &self.text {
            NoticeText::Ref(r) => r.render(src),
            NoticeText::Usage(c) => usage(c),
            NoticeText::Own(t) => t.clone(),
        }
    }
}

/// Mask 0x1: type 1 `Error`.
pub const ERROR: u64 = 0x1;
/// Mask 0x8: type 4 `Standard`.
pub const STANDARD: u64 = 0x8;
/// Mask 0x20: type 6 `Tell`.
pub const TELL: u64 = 0x20;
/// Mask 0x21: types 1 and 6.
pub const TELL_ERROR: u64 = 0x21;
/// Mask 0x80: type 8 `Log` (Tessera's informational lines, `chat-window.md` §10 item 2).
pub const LOG: u64 = 0x80;
/// Mask 0x400: type 11 `Fellowship`.
pub const FELLOWSHIP: u64 = 0x400;

/// Tessera's own wording for a table entry (CT12: used when the template is missing).
/// The words are Tessera's; only the arguments are the entry's. Unknown entries give
/// their argument values joined with spaces.
#[must_use]
pub fn fallback_text(table: u32, token: u32, vals: &[(u32, String)]) -> String {
    let a = |id: u32| {
        vals.iter()
            .find(|(i, _)| *i == id)
            .map_or("", |(_, v)| v.as_str())
            .to_string()
    };
    use args as A;
    use tables as T;
    match (table, token) {
        (T::CHAT, 0x0BCA_09CC) => format!("{} tells you: {}", a(A::NAME), a(A::TEXT)),
        (T::CHAT, 0x0AD7_4A23) => format!("To {}: {}", a(A::RECIPIENT), a(A::TEXT)),
        (T::CHAT, 0x0F2C_F919) => format!("[{}] {}: {}", a(A::TIME), a(A::NAME), a(A::TEXT)),
        (T::CHAT, 0x04A3_E744) => format!("[Group] You: {}", a(A::TEXT)),
        (T::CHAT, 0x05D2_0D84) => format!("[Group] {}: {}", a(A::SPEAKER), a(A::TEXT)),
        (T::CHAT, 0x009D_15CC) => format!("[{}] You: {}", a(A::CHANNEL_ECHO), a(A::TEXT)),
        (T::CHAT, 0x063C_924C) => {
            format!("[{}] {}: {}", a(A::CHANNEL), a(A::SPEAKER), a(A::TEXT))
        }
        (T::CHAT, 0x00CC_89CC) => "Nobody has sent you a tell yet.".into(),
        (T::CHAT, 0x0811_EFD4) => "You have not sent a tell yet.".into(),
        (T::CHAT, 0x01A1_7E05) => format!("No text to send to {}.", a(A::RECIPIENT)),
        (T::CHAT, 0x0D93_B0C5) => "Everyone".into(),
        (T::CHAT, 0x0E3E_83D5) => "Tells".into(),
        (T::CHAT, 0x0780_711E) => "New conversation".into(),
        (T::CHAT, 0x0E6C_6E0D) => "End conversation".into(),
        (T::CHAT, 0x093D_1E8D) => "Conversation tab".into(),
        (T::CHAT, 0x0D3D_2E04) => "Talk to whom?".into(),
        (T::CHAT, 0x0EA6_8129) => "No name given.".into(),
        (T::CHAT, 0x0406_8445) => "Chat channels are not available here.".into(),
        (T::CHAT, 0x0283_2AE7) => format!(
            "The channel name {} has {} characters; at most {} are allowed.",
            a(A::CHANNEL),
            a(A::LENGTH),
            a(A::LIMIT)
        ),
        (T::CHAT, 0x0AC8_421C) => "Give a channel name.".into(),
        (T::CHAT, 0x0191_9744) => format!("{} cannot be used as a channel name.", a(A::CHANNEL)),
        (T::CHAT, 0x08DC_4764) => "Channel passwords are not supported.".into(),
        (T::CHAT, 0x0754_8D0C) => format!("You are already in {}.", a(A::CHANNEL_NAME)),
        (T::CHAT, 0x002A_AA94) => format!("You are already in {} channels.", a(A::COUNT)),
        (T::CHAT, 0x09A9_9A08) => format!("Channel {} joined.", a(A::SLOT)),
        (T::CHAT, 0x03B0_95F8) => format!("Channel {} left.", a(A::SLOT)),
        (T::CHAT, 0x03C0_43D4) => format!("{} is not a channel you joined.", a(A::CHANNEL)),
        (T::CHAT, 0x0DF4_808C) | (T::CHAT, 0x02E3_2555) => {
            format!("You are not in {}.", a(A::CHANNEL_NAME))
        }
        (T::CHAT, 0x0151_3613) => "You have joined no channels.".into(),
        (T::CHAT, 0x09AA_EAC2) => "Channels:".into(),
        (T::CHAT, 0x000E_E4F4) => format!(
            "{}. {}{}",
            a(A::SLOT),
            a(A::CHANNEL_NAME),
            a(A::PASSWORD_PART)
        ),
        (T::CHAT, 0x02E8_19D4) => String::new(),
        (T::CHAT, 0x066B_746F) => format!(" ({})", a(A::PASSWORD)),
        (T::CHAT, 0x0EBE_4334) => format!("{}. -", a(A::SLOT)),
        (T::CHAT, 0x0E2A_7CE8) => format!("Channel {} is not joined.", a(A::COUNT)),
        (T::CHAT, 0x0A29_AFC5) => format!("{} has come online.", a(A::NAME)),
        (T::CHAT, 0x0105_38E5) => format!("{} has gone offline.", a(A::NAME)),
        (T::FELLOWSHIP, 0x0635_8717) => "You are not in a group.".into(),
        (T::FELLOWSHIP, 0x0307_3954) => "You left the group.".into(),
        (T::FELLOWSHIP, 0x018C_CD74) => "The group has ended.".into(),
        (T::FELLOWSHIP, 0x0245_83E7) => "You formed a group.".into(),
        (T::FELLOWSHIP, 0x0CDF_FC77) => "You are now in a group.".into(),
        (T::FELLOWSHIP, 0x0928_4BE7) => format!("{} was removed from the group.", a(A::NAME)),
        (T::FELLOWSHIP, 0x0C75_F607) => format!("{} left the group.", a(A::NAME)),
        (T::FELLOWSHIP, 0x047E_6917) => format!("{} joined the group.", a(A::NAME)),
        (T::FELLOWSHIP, 0x043F_25A6) => "You lead the group now.".into(),
        (T::FELLOWSHIP, 0x03FD_5682) => format!("{} leads the group now.", a(A::NAME)),
        (T::FELLOWSHIP, 0x0571_E4C5) => format!("{} is too far away.", a(A::MEMBER)),
        (T::FELLOWSHIP, 0x04CF_6294) => format!("{} is not connected.", a(A::MEMBER)),
        (T::FELLOWSHIP, 0x0897_28CE) => format!("{} invites you to a group.", a(A::NAME)),
        (T::UI, 0x0304_5B45) => "Group invitation".into(),
        (T::UI_FRIENDS, 0x0A82_90DF) => "?".into(),
        (T::UI_FRIENDS, 0x06CD_DF0C) => a(A::LEVEL),
        (T::UI_FRIENDS, 0x0FDC_19AC) => a(A::AREA),
        (T::UI_FRIENDS, 0x0FE8_45CC) => a(A::ONLINE_STATE),
        (T::UI_FRIENDS, 0x0FF0_52F5) => "Online".into(),
        (T::UI_FRIENDS, 0x0C3E_9831) => "Away".into(),
        (T::UI_FRIENDS, 0x0BC2_5DB4) => "Name of the friend to add:".into(),
        (T::UI_FRIENDS, 0x0803_A974) => format!("Note for {}:", a(A::NOTE_NAME)),
        (T::UI_FRIENDS, 0x0903_7E74) => "Name to ignore:".into(),
        (T::UI_FRIENDS, 0x0193_53F5) => a(A::NOTE_NAME),
        (T::UI, 0x070F_9F3C) => a(A::FELLOW_LEVEL),
        (T::UI, 0x038B_9CDC) => a(A::LEVEL),
        (T::UI, 0x0748_0C44) => a(A::COIN_COUNT),
        (T::UI_FRIENDS, 0x0743_846D) => "Send a tell".into(),
        (T::UI_FRIENDS, 0x0452_9325) => "Invite to group".into(),
        (T::UI_FRIENDS, 0x033D_8545) => "Remove friend".into(),
        (T::UI_FRIENDS, 0x044B_1505) => "Ignore".into(),
        (T::UI_FRIENDS, 0x0386_01E5) | (T::UI_FRIENDS, 0x0F57_CB45) => "Edit note".into(),
        (T::UI_FRIENDS, 0x065B_E227) => "Join groups automatically".into(),
        (T::UI_FRIENDS, 0x00CB_5BA7) => "Do not join groups automatically".into(),
        (T::UI_FRIENDS, 0x0B60_7755) => a(A::IGNORE_NAME),
        (T::UI_FRIENDS, 0x0177_E995) => "Stop ignoring".into(),
        (T::UI_SECURE_TRADE, 0x0CFA_21C2) => format!("{} offers", a(A::NAME)),
        (T::UI_SECURE_TRADE, 0x085F_80B9) => "Enter a whole number of coins.".into(),
        (T::UI_SECURE_TRADE, 0x0619_5A09) => "You do not have that much money.".into(),
        _ => vals
            .iter()
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Name-equal (`chat-window.md` §15.10, §10 item 12): equal after Unicode default
/// lower-casing, without a locale.
#[must_use]
pub fn name_equal(a: &str, b: &str) -> bool {
    a == b || a.to_lowercase() == b.to_lowercase()
}

/// Longest name Tessera accepts, in bytes of UTF-8 (the session layer's `NAME` bound,
/// `tessera-protocol.md` §5.2).
pub const MAX_NAME_BYTES: usize = 64;

/// Longest chat text Tessera sends, in bytes of UTF-8 (`chat-window.md` §10 item 12, the
/// protocol's `TEXT` bound).
pub const MAX_TEXT_BYTES: usize = 1024;

/// The session layer's name rule (`chat-window.md` §15.10 `is_valid_name`; the same rule as
/// the session layer's): 1–[`MAX_NAME_BYTES`] bytes and no control characters.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME_BYTES && !name.chars().any(char::is_control)
}
