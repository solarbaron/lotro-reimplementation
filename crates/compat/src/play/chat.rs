// SPDX-License-Identifier: GPL-3.0-or-later
//! World-server chat: text limits (`docs/specs/official-messages-play.md` §6.2), chat
//! categories (§6.7), outgoing chat and emote builders (§6.3.1, §6.4), the property-bag
//! chat event `0x1C` (§6.5.1) and the admin notice `0x30000045` (§6.5, ignored).

use tessera_net::ChatChannel;

use crate::play::rpc::{PlayRpc, StateRef};
use crate::play::{PlayError, check_units};
use crate::world::{
    EncodedPropertyTable, EncodedPropertyValue, LocalizedTokenText, LocalizedTokenTextContent,
    PropertySchemaLookup, WireReader, WireWriter,
};

/// Largest standard chat text (`/say`, `/tell`, channel chat), UTF-16 code units (§6.2).
pub const MAX_CHAT_UNITS: usize = 1925;
/// Largest custom emote text (`/emote <text>`), UTF-16 code units (§6.2).
pub const MAX_EMOTE_UNITS: usize = 1155;
/// Largest restricted or command-argument text, UTF-16 code units (§6.2). This layer
/// applies it to names, room names and other command arguments.
pub const MAX_ARGUMENT_UNITS: usize = 385;

/// Event ID of the property-bag UI event (`ui_property_event`, §6.5).
pub const OPCODE_UI_PROPERTY_EVENT: u32 = 0x0000_001C;
/// Event ID of the admin text notice (`system_admin_text_notice`, §6.5): out of scope
/// (decision 0042 item 2); never encoded, ignored and counted when received.
pub const OPCODE_ADMIN_TEXT_NOTICE: u32 = 0x3000_0045;

/// Chat category of spatial `/say` (§6.7).
pub const CATEGORY_SAY: u32 = 5;
/// Chat category of `/tell` (§6.7).
pub const CATEGORY_TELL: u32 = 6;
/// Chat category of `/emote` (§6.7).
pub const CATEGORY_EMOTE: u32 = 7;
/// Chat category of fellowship chat (§6.7).
pub const CATEGORY_FELLOWSHIP: u32 = 11;
/// Chat category of kinship chat (§6.7).
pub const CATEGORY_KINSHIP: u32 = 12;
/// Chat category of kinship officer chat (§6.7).
pub const CATEGORY_OFFICER: u32 = 13;
/// Chat category of spatial yell / shout (§6.7).
pub const CATEGORY_YELL: u32 = 19;
/// Chat category of raid chat (§6.7).
pub const CATEGORY_RAID: u32 = 23;
/// Chat category of the roleplay channel (§6.7).
pub const CATEGORY_ROLEPLAY: u32 = 27;
/// Chat category of the world channel (§6.7).
pub const CATEGORY_WORLD: u32 = 38;
/// Chat categories of user chat channel slots 1..8 (§6.1.3, §6.7): slot `k` is the `k`-th
/// user-created room joined.
pub const CATEGORY_USER_SLOTS: [u32; 8] = [0x1C, 0x1D, 0x1E, 0x1F, 0x27, 0x28, 0x29, 0x2A];

/// What a chat category number of §6.7 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CategoryKind {
    /// `5`.
    Say,
    /// `6`.
    Tell,
    /// `7`.
    Emote,
    /// `11`.
    Fellowship,
    /// `12`.
    Kinship,
    /// `13`.
    Officer,
    /// `19`.
    Yell,
    /// `23`.
    Raid,
    /// `27`.
    Roleplay,
    /// `38`.
    World,
    /// One of [`CATEGORY_USER_SLOTS`]: the 1-based user channel slot `1..=8`.
    UserSlot(u8),
}

impl CategoryKind {
    /// The kind of category number `category` (§6.7), `None` for a number the spec does not
    /// name.
    #[must_use]
    pub fn of(category: u32) -> Option<Self> {
        Some(match category {
            CATEGORY_SAY => Self::Say,
            CATEGORY_TELL => Self::Tell,
            CATEGORY_EMOTE => Self::Emote,
            CATEGORY_FELLOWSHIP => Self::Fellowship,
            CATEGORY_KINSHIP => Self::Kinship,
            CATEGORY_OFFICER => Self::Officer,
            CATEGORY_YELL => Self::Yell,
            CATEGORY_RAID => Self::Raid,
            CATEGORY_ROLEPLAY => Self::Roleplay,
            CATEGORY_WORLD => Self::World,
            other => {
                let slot = CATEGORY_USER_SLOTS.iter().position(|&c| c == other)?;
                // At most 8 slots.
                #[allow(clippy::cast_possible_truncation)]
                Self::UserSlot(slot as u8 + 1)
            }
        })
    }

    /// The category number of this kind (§6.7).
    #[must_use]
    pub fn category(self) -> Option<u32> {
        Some(match self {
            Self::Say => CATEGORY_SAY,
            Self::Tell => CATEGORY_TELL,
            Self::Emote => CATEGORY_EMOTE,
            Self::Fellowship => CATEGORY_FELLOWSHIP,
            Self::Kinship => CATEGORY_KINSHIP,
            Self::Officer => CATEGORY_OFFICER,
            Self::Yell => CATEGORY_YELL,
            Self::Raid => CATEGORY_RAID,
            Self::Roleplay => CATEGORY_ROLEPLAY,
            Self::World => CATEGORY_WORLD,
            Self::UserSlot(k) => *CATEGORY_USER_SLOTS.get(usize::from(k).checked_sub(1)?)?,
        })
    }

    /// The neutral channel of this kind, where Tessera's model has one: Say, Tell (empty
    /// peer), Emote, Fellowship and Yell. Kinship, officer, raid, roleplay and world chat
    /// have no neutral channel (a caller may name one with [`ChatCategories::set`]); a user
    /// slot is the room bound to it ([`ChatCategories::set_user_rooms`]).
    #[must_use]
    pub fn neutral_channel(self) -> Option<ChatChannel> {
        Some(match self {
            Self::Say => ChatChannel::Say,
            Self::Tell => ChatChannel::Tell {
                peer: String::new(),
            },
            Self::Emote => ChatChannel::Emote,
            Self::Fellowship => ChatChannel::Fellowship,
            Self::Yell => ChatChannel::Yell,
            _ => return None,
        })
    }
}

/// `0x1C` event code of a standard chat line (§6.5.1).
pub const UI_EVENT_CHAT_LINE: u32 = 0x0102_000C;
/// `0x1C` event code of a tell (§6.5.1).
pub const UI_EVENT_TELL: u32 = 0x0102_000D;
/// `0x1C` property: chat text category.
pub const PID_CHAT_CATEGORY: u32 = 0x1000_00B1;
/// `0x1C` property: chat text.
pub const PID_CHAT_TEXT: u32 = 0x1000_15D7;
/// `0x1C` property: sender name (`0x0102000C`), or target/sender name (`0x0102000D`).
pub const PID_CHAT_SENDER_NAME: u32 = 0x1000_0914;
/// `0x1C` property: sender ID.
pub const PID_CHAT_SENDER_ID: u32 = 0x1000_215A;
/// `0x1C` property: tell target or sender name.
pub const PID_TELL_PEER_NAME_A: u32 = 0x1000_090E;
/// `0x1C` property: tell target or sender name, also the player name of room events.
pub const PID_TELL_PEER_NAME_B: u32 = 0x1000_0915;
/// `0x1C` property: a tell is the outgoing echo.
pub const PID_TELL_IS_OUTGOING: u32 = 0x1000_133D;

/// How chat categories (enum `0x230002A8`, §6.7) map to Tessera's channels: the numbers
/// the spec names with a neutral channel ([`CategoryKind::neutral_channel`]), kept only
/// when the install's enum has them ([`Self::from_install_keys`]), plus the user channel
/// slots bound to the user rooms in join order ([`Self::set_user_rooms`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCategories {
    entries: Vec<(u32, ChatChannel)>,
    /// The user rooms bound to slots 1..8, in join order.
    user_rooms: Vec<String>,
}

/// The categories with a neutral channel, in §6.7 order.
const NEUTRAL_KINDS: [CategoryKind; 5] = [
    CategoryKind::Say,
    CategoryKind::Tell,
    CategoryKind::Emote,
    CategoryKind::Fellowship,
    CategoryKind::Yell,
];

impl Default for ChatCategories {
    /// The spec's numbers with a neutral channel: Say `5`, Tell `6`, Emote `7`, Fellowship
    /// `11`, Yell `19` (§6.7); no user room bound.
    fn default() -> Self {
        Self::from_install_keys(None)
    }
}

/// The key of a channel for [`ChatCategories::set`]: one entry per channel kind, a named
/// channel per name.
fn channel_key(channel: &ChatChannel) -> (u8, Option<&str>) {
    match channel {
        ChatChannel::Say => (0, None),
        ChatChannel::Yell => (1, None),
        ChatChannel::Emote => (2, None),
        ChatChannel::Tell { .. } => (3, None),
        ChatChannel::System => (4, None),
        ChatChannel::Fellowship => (5, None),
        ChatChannel::Named { name } => (6, Some(name.as_str())),
        _ => (7, None),
    }
}

impl ChatCategories {
    /// A table with no entries.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            user_rooms: Vec::new(),
        }
    }

    /// The spec's categories with a neutral channel ([`Self::default`]), each kept only when
    /// `keys` (the keys of the install's enum `0x230002A8` and its base chain) has it; all
    /// of them without the install's enum (`None`).
    #[must_use]
    pub fn from_install_keys(keys: Option<&[u32]>) -> Self {
        let mut c = Self::empty();
        for kind in NEUTRAL_KINDS {
            let (Some(category), Some(channel)) = (kind.category(), kind.neutral_channel()) else {
                continue;
            };
            if keys.is_none_or(|k| k.contains(&category)) {
                c.entries.push((category, channel));
            }
        }
        c
    }

    /// Maps `category` to `channel` (for a tell, the peer is ignored), replacing any
    /// mapping of that category or of that channel (a named channel by its name).
    pub fn set(&mut self, category: u32, channel: ChatChannel) {
        self.entries
            .retain(|(c, ch)| *c != category && channel_key(ch) != channel_key(&channel));
        self.entries.push((category, channel));
    }

    /// Binds the user channel slots 1..8 to `rooms` (§6.7: the user-created rooms in the
    /// order they were joined); rooms past the eighth have no slot.
    pub fn set_user_rooms<I: IntoIterator<Item = String>>(&mut self, rooms: I) {
        self.user_rooms = rooms.into_iter().take(CATEGORY_USER_SLOTS.len()).collect();
    }

    /// The channel of `category`, with an empty peer for a tell; a user slot with a bound
    /// room is that room's named channel.
    #[must_use]
    pub fn channel_for(&self, category: u32) -> Option<ChatChannel> {
        if let Some((_, ch)) = self.entries.iter().find(|(c, _)| *c == category) {
            return Some(ch.clone());
        }
        match CategoryKind::of(category)? {
            CategoryKind::UserSlot(k) => self
                .user_rooms
                .get(usize::from(k).checked_sub(1)?)
                .map(|name| ChatChannel::Named { name: name.clone() }),
            _ => None,
        }
    }

    /// The category of `channel`'s kind; for a named channel, the category it was set to or
    /// its user slot.
    #[must_use]
    pub fn category_for(&self, channel: &ChatChannel) -> Option<u32> {
        let key = channel_key(channel);
        if let Some((c, _)) = self.entries.iter().find(|(_, ch)| channel_key(ch) == key) {
            return Some(*c);
        }
        let ChatChannel::Named { name } = channel else {
            return None;
        };
        let slot = self.user_rooms.iter().position(|r| r == name)?;
        CATEGORY_USER_SLOTS.get(slot).copied()
    }
}

/// Builds `0x84EC001B` (`/say` or typed chat, §6.3.1) with `flags = 0`.
///
/// # Errors
///
/// [`PlayError::EmptyText`], [`PlayError::TextTooLong`] (over [`MAX_CHAT_UNITS`]) or
/// [`PlayError::ChatTypeRefused`] for categories `1` and `2`.
pub fn say(text: &str, chat_type: u32) -> Result<PlayRpc, PlayError> {
    if text.is_empty() {
        return Err(PlayError::EmptyText);
    }
    check_units(text, MAX_CHAT_UNITS)?;
    if chat_type == 1 || chat_type == 2 {
        return Err(PlayError::ChatTypeRefused(chat_type));
    }
    Ok(PlayRpc::Say {
        flags: 0,
        text: StateRef::text(text),
        chat_type,
    })
}

/// Builds `0x84EC000E` (`/tell` by name, §6.3.1).
///
/// # Errors
///
/// [`PlayError::EmptyText`] or [`PlayError::TextTooLong`] (text over [`MAX_CHAT_UNITS`],
/// name over [`MAX_ARGUMENT_UNITS`]).
pub fn tell(target: &str, text: &str) -> Result<PlayRpc, PlayError> {
    if text.is_empty() || target.is_empty() {
        return Err(PlayError::EmptyText);
    }
    check_units(text, MAX_CHAT_UNITS)?;
    check_units(target, MAX_ARGUMENT_UNITS)?;
    Ok(PlayRpc::Tell {
        message: StateRef::text(text),
        target: StateRef::text(target),
    })
}

/// Builds `0x84F9000A` (custom text emote, §6.4).
///
/// # Errors
///
/// [`PlayError::EmptyText`] or [`PlayError::TextTooLong`] (over [`MAX_EMOTE_UNITS`]).
pub fn emote_text(text: &str) -> Result<PlayRpc, PlayError> {
    if text.is_empty() {
        return Err(PlayError::EmptyText);
    }
    check_units(text, MAX_EMOTE_UNITS)?;
    Ok(PlayRpc::EmoteText {
        text: StateRef::text(text),
    })
}

/// Builds `0x84F90005` (predefined emote, untargeted when `target` is `None`, §6.4).
///
/// # Errors
///
/// [`PlayError::TextTooLong`] when the name exceeds [`MAX_ARGUMENT_UNITS`].
pub fn emote_by_name(target: Option<&str>, emote_did: u32) -> Result<PlayRpc, PlayError> {
    let target_name = match target {
        Some(name) if !name.is_empty() => {
            check_units(name, MAX_ARGUMENT_UNITS)?;
            StateRef::text(name)
        }
        _ => StateRef::Null,
    };
    Ok(PlayRpc::EmoteByName {
        target_name,
        emote_did,
    })
}

/// Turns a wire text into display text (`string-table.md`): literal texts resolve
/// without data; references need the install's string tables.
pub trait TextResolver {
    /// The display text of `text`, or `None` when it cannot be resolved.
    fn resolve(&self, text: &LocalizedTokenText) -> Option<String>;
}

/// A resolver that knows only literal texts (mode `1`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LiteralTexts;

impl TextResolver for LiteralTexts {
    fn resolve(&self, text: &LocalizedTokenText) -> Option<String> {
        match &text.content {
            LocalizedTokenTextContent::Literal(s) => Some(s.clone()),
            LocalizedTokenTextContent::Reference { .. } => None,
        }
    }
}

/// The display text of a reference argument: its UTF-16 text, its resolved localized
/// text, or the empty string for a null reference. `None` when a localized text cannot be
/// resolved or the bundle is not a single text node.
#[must_use]
pub fn ref_text(r: &StateRef, resolver: &dyn TextResolver) -> Option<String> {
    match r {
        StateRef::Null => Some(String::new()),
        StateRef::Opaque(_) => None,
        StateRef::Nodes(_) => match r.single()? {
            crate::play::rpc::NodeValue::Text(s) => Some(s.clone()),
            crate::play::rpc::NodeValue::Localized(t) => resolver.resolve(t),
        },
    }
}

/// Event `0x1C` (`ui_property_event`, S→C, §6.5–§6.5.1): an event code and its property
/// table.
#[derive(Debug, Clone, PartialEq)]
pub struct UiPropertyEvent {
    /// The event code (`0x01020001`–`0x01020022`).
    pub event_code: u32,
    /// The properties.
    pub properties: EncodedPropertyTable,
}

impl UiPropertyEvent {
    /// Encodes the inner payload.
    ///
    /// # Errors
    ///
    /// [`PlayError::World`] when a property value cannot be encoded.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        writer.write_base_flags();
        writer.write_u32_le(self.event_code);
        self.properties.encode(writer)?;
        Ok(())
    }

    /// Decodes the inner payload; property types come from `schema` (the install's
    /// property dictionary, `property-system.md` §10).
    ///
    /// # Errors
    ///
    /// [`PlayError::World`] on truncation or an unknown property.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, PlayError> {
        reader.read_base_flags()?;
        let event_code = reader.read_u32_le()?;
        let properties = EncodedPropertyTable::decode(reader, schema)?;
        Ok(Self {
            event_code,
            properties,
        })
    }

    /// Decodes a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }

    /// Encodes to a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// The chat line of a `0x0102000C` or `0x0102000D` event (§6.5.1), `None` for other
    /// codes or when a required property is missing.
    #[must_use]
    pub fn chat_line(&self, resolver: &dyn TextResolver) -> Option<UiChatLine> {
        let text = self
            .properties
            .get(PID_CHAT_TEXT)
            .and_then(|v| value_text(v, resolver))?;
        let category = self.properties.get(PID_CHAT_CATEGORY).and_then(value_u32);
        match self.event_code {
            UI_EVENT_CHAT_LINE => Some(UiChatLine {
                category,
                text,
                sender_name: self
                    .properties
                    .get(PID_CHAT_SENDER_NAME)
                    .and_then(|v| value_text(v, resolver)),
                sender_id: self.properties.get(PID_CHAT_SENDER_ID).and_then(value_u64),
                tell_outgoing: None,
            }),
            UI_EVENT_TELL => Some(UiChatLine {
                category: category.or(Some(CATEGORY_TELL)),
                text,
                sender_name: self
                    .properties
                    .get(PID_TELL_PEER_NAME_A)
                    .or_else(|| self.properties.get(PID_TELL_PEER_NAME_B))
                    .and_then(|v| value_text(v, resolver)),
                sender_id: None,
                tell_outgoing: self.properties.get(PID_TELL_IS_OUTGOING).and_then(|v| {
                    if let EncodedPropertyValue::Boolean(b) = v {
                        Some(*b)
                    } else {
                        None
                    }
                }),
            }),
            _ => None,
        }
    }
}

/// A chat line carried by a `0x1C` event (§6.5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiChatLine {
    /// The chat category, when present.
    pub category: Option<u32>,
    /// The text.
    pub text: String,
    /// The sender's (or for a tell, the peer's) name.
    pub sender_name: Option<String>,
    /// The sender's ID.
    pub sender_id: Option<u64>,
    /// For a tell: whether it is the outgoing echo.
    pub tell_outgoing: Option<bool>,
}

fn value_u32(v: &EncodedPropertyValue) -> Option<u32> {
    match v {
        EncodedPropertyValue::Enum(x)
        | EncodedPropertyValue::DidRef(x)
        | EncodedPropertyValue::Bitfield32(x)
        | EncodedPropertyValue::StringToken(x)
        | EncodedPropertyValue::PropertyId(x) => Some(*x),
        EncodedPropertyValue::Int(x) => u32::try_from(*x).ok(),
        _ => None,
    }
}

fn value_u64(v: &EncodedPropertyValue) -> Option<u64> {
    match v {
        EncodedPropertyValue::InstanceId(x) | EncodedPropertyValue::Bitfield64(x) => Some(*x),
        EncodedPropertyValue::Int64(x) => u64::try_from(*x).ok(),
        _ => None,
    }
}

fn value_text(v: &EncodedPropertyValue, resolver: &dyn TextResolver) -> Option<String> {
    match v {
        EncodedPropertyValue::String(s) => Some(s.clone()),
        EncodedPropertyValue::LocalizedTokenText(t) => resolver.resolve(t),
        _ => None,
    }
}
