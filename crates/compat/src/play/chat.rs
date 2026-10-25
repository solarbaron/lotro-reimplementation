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
/// Chat categories of user chat channel slots 1..8 (§6.1.3, §6.7).
pub const CATEGORY_USER_SLOTS: [u32; 8] = [0x1C, 0x1D, 0x1E, 0x1F, 0x27, 0x28, 0x29, 0x2A];

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

/// How chat categories (enum `0x230002A8`, §6.7) map to Tessera's channels. Only the
/// numbers the spec names are built in ([`Self::default`]); the rest come from the
/// install's enum record at run time, through [`Self::set`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCategories {
    entries: Vec<(u32, ChatChannel)>,
}

impl Default for ChatCategories {
    /// Say = 5 and Tell = 6 (§6.7). User channel slots have no channel name here; the
    /// chat-server rooms carry their names.
    fn default() -> Self {
        Self {
            entries: vec![
                (CATEGORY_SAY, ChatChannel::Say),
                (
                    CATEGORY_TELL,
                    ChatChannel::Tell {
                        peer: String::new(),
                    },
                ),
            ],
        }
    }
}

fn channel_key(channel: &ChatChannel) -> u8 {
    match channel {
        ChatChannel::Say => 0,
        ChatChannel::Yell => 1,
        ChatChannel::Emote => 2,
        ChatChannel::Tell { .. } => 3,
        ChatChannel::System => 4,
        ChatChannel::Fellowship => 5,
        ChatChannel::Named { .. } => 6,
        _ => 7,
    }
}

impl ChatCategories {
    /// A table with no entries.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Maps `category` to `channel` (for a tell, the peer is ignored), replacing any
    /// mapping of that category or of that channel kind.
    pub fn set(&mut self, category: u32, channel: ChatChannel) {
        let key = channel_key(&channel);
        self.entries
            .retain(|(c, ch)| *c != category && channel_key(ch) != key);
        self.entries.push((category, channel));
    }

    /// The channel of `category`, with an empty peer for a tell.
    #[must_use]
    pub fn channel_for(&self, category: u32) -> Option<ChatChannel> {
        self.entries
            .iter()
            .find(|(c, _)| *c == category)
            .map(|(_, ch)| ch.clone())
    }

    /// The category of `channel`'s kind.
    #[must_use]
    pub fn category_for(&self, channel: &ChatChannel) -> Option<u32> {
        let key = channel_key(channel);
        self.entries
            .iter()
            .find(|(_, ch)| channel_key(ch) == key)
            .map(|(c, _)| *c)
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
