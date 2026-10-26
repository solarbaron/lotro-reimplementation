// SPDX-License-Identifier: GPL-3.0-or-later
//! Chat-server wire structures (`docs/specs/official-messages-play.md` §6.6.4):
//! `ChatRoomDescriptor`, the container encodings and `ChatMessagePayload`.

use crate::play::PlayError;
use crate::world::{EncodedPropertyTable, PropertySchemaLookup, WireReader, WireWriter};

/// `presence_tag` of a non-null descriptor (§6.6.4).
pub const DESCRIPTOR_PRESENT: u32 = 0x0000_0018;
/// `msg_type` of a chat message carrying a `ChatMessagePayload` (§6.6.4).
pub const MSG_TYPE_CHAT: u32 = 0x0802_0000;
/// `msg_type` of a direct string buffer (§6.6.4).
pub const MSG_TYPE_STRING: u32 = 0x0804_0000;
/// Smallest wire size of a descriptor (a null one), for count bounds (§9.1 rule 1).
pub const MIN_DESCRIPTOR_BYTES: usize = 4;
/// Most entries accepted in one container, on top of the byte bounds. Engineering choice,
/// spec silent.
pub const MAX_CONTAINER_ENTRIES: usize = 4096;

/// The kind of a non-null descriptor (`desc_type`, §6.6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorKind {
    /// `1`: a player or avatar.
    Entity,
    /// `2`: a user-created chat room.
    UserRoom,
    /// `3`: a server-managed chat room.
    ServerRoom,
}

impl DescriptorKind {
    /// The wire value.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Entity => 1,
            Self::UserRoom => 2,
            Self::ServerRoom => 3,
        }
    }

    /// The kind of a wire value.
    ///
    /// # Errors
    ///
    /// [`PlayError::InvalidDescriptorType`] outside `1..=3` (§9.1 rule 2).
    pub const fn from_code(code: u32) -> Result<Self, PlayError> {
        match code {
            1 => Ok(Self::Entity),
            2 => Ok(Self::UserRoom),
            3 => Ok(Self::ServerRoom),
            other => Err(PlayError::InvalidDescriptorType(other)),
        }
    }
}

/// A `ChatRoomDescriptor` (§6.6.4): a player, a user room or a server room.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChatRoomDescriptor {
    /// `presence_tag = 0`.
    Null,
    /// `presence_tag = 0x18`.
    Present {
        /// `desc_type`.
        kind: DescriptorKind,
        /// `descriptor_id`.
        id: u64,
        /// `name`.
        name: String,
    },
}

impl ChatRoomDescriptor {
    /// A descriptor of `kind`.
    #[must_use]
    pub fn new(kind: DescriptorKind, id: u64, name: impl Into<String>) -> Self {
        Self::Present {
            kind,
            id,
            name: name.into(),
        }
    }

    /// The name, for a non-null descriptor.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Null => None,
            Self::Present { name, .. } => Some(name),
        }
    }

    /// The kind, for a non-null descriptor.
    #[must_use]
    pub fn kind(&self) -> Option<DescriptorKind> {
        match self {
            Self::Null => None,
            Self::Present { kind, .. } => Some(*kind),
        }
    }

    /// The ID, for a non-null descriptor.
    #[must_use]
    pub fn id(&self) -> Option<u64> {
        match self {
            Self::Null => None,
            Self::Present { id, .. } => Some(*id),
        }
    }

    /// For a server room: `(channel_type, secondary_type)`, the low 32 bits and bits
    /// 48..63 of the composite ID (§6.6.4, §6.7).
    #[must_use]
    pub fn server_room_types(&self) -> Option<(u32, u32)> {
        match self {
            Self::Present {
                kind: DescriptorKind::ServerRoom,
                id,
                ..
            } => Some(((*id & 0xFFFF_FFFF) as u32, (*id >> 48) as u32)),
            _ => None,
        }
    }

    /// The composite ID of a server room of `channel_type` and `secondary_type` (§6.6.4).
    #[must_use]
    pub fn server_room_id(channel_type: u32, secondary_type: u16) -> u64 {
        (u64::from(secondary_type) << 48) | u64::from(channel_type)
    }

    /// Encodes the descriptor.
    ///
    /// # Errors
    ///
    /// [`PlayError::World`] when the name does not fit a 3-tier VLE count.
    pub fn encode(&self, w: &mut WireWriter) -> Result<(), PlayError> {
        match self {
            Self::Null => w.write_u32_le(0),
            Self::Present { kind, id, name } => {
                w.write_u32_le(DESCRIPTOR_PRESENT);
                w.write_u32_le(kind.code());
                w.write_u64_le(*id);
                w.write_utf16le_string(name)?;
            }
        }
        Ok(())
    }

    /// Decodes a descriptor (§9.1 rule 2).
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation, a bad tag or a bad kind.
    pub fn decode(r: &mut WireReader<'_>) -> Result<Self, PlayError> {
        match r.read_u32_le()? {
            0 => Ok(Self::Null),
            DESCRIPTOR_PRESENT => {
                let kind = DescriptorKind::from_code(r.read_u32_le()?)?;
                let id = r.read_u64_le()?;
                let name = r.read_utf16le_string()?;
                Ok(Self::Present { kind, id, name })
            }
            other => Err(PlayError::InvalidDescriptorTag(other)),
        }
    }
}

fn read_count(r: &mut WireReader<'_>, min_entry: usize) -> Result<usize, PlayError> {
    let count = r.read_u32_le()? as usize;
    r.ensure_count(count, min_entry)?;
    if count > MAX_CONTAINER_ENTRIES {
        return Err(PlayError::TooMany {
            what: "chat container entries",
            max: MAX_CONTAINER_ENTRIES,
        });
    }
    Ok(count)
}

fn write_count(w: &mut WireWriter, n: usize) -> Result<(), PlayError> {
    if n > MAX_CONTAINER_ENTRIES {
        return Err(PlayError::TooMany {
            what: "chat container entries",
            max: MAX_CONTAINER_ENTRIES,
        });
    }
    // Bounded by MAX_CONTAINER_ENTRIES.
    w.write_u32_le(n as u32);
    Ok(())
}

/// `LengthPrefixedBytes`: `u32 LE` count + bytes.
///
/// # Errors
///
/// [`PlayError`] on truncation.
pub fn read_bytes_prefixed(r: &mut WireReader<'_>) -> Result<Vec<u8>, PlayError> {
    let n = r.read_u32_le()? as usize;
    r.ensure_count(n, 1)?;
    Ok(r.read_bytes(n)?.to_vec())
}

/// Writes `LengthPrefixedBytes`.
///
/// # Errors
///
/// [`PlayError::TooMany`] when the length does not fit a `u32`.
pub fn write_bytes_prefixed(w: &mut WireWriter, bytes: &[u8]) -> Result<(), PlayError> {
    let n = u32::try_from(bytes.len()).map_err(|_| PlayError::TooMany {
        what: "length-prefixed bytes",
        max: u32::MAX as usize,
    })?;
    w.write_u32_le(n);
    w.write_bytes(bytes);
    Ok(())
}

/// `DescriptorList`: `u32 LE` count + descriptors.
///
/// # Errors
///
/// [`PlayError`] on truncation or a bad descriptor.
pub fn read_descriptor_list(r: &mut WireReader<'_>) -> Result<Vec<ChatRoomDescriptor>, PlayError> {
    let n = read_count(r, MIN_DESCRIPTOR_BYTES)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(ChatRoomDescriptor::decode(r)?);
    }
    Ok(out)
}

/// Writes a `DescriptorList`.
///
/// # Errors
///
/// [`PlayError`] over [`MAX_CONTAINER_ENTRIES`] or on a descriptor error.
pub fn write_descriptor_list(
    w: &mut WireWriter,
    list: &[ChatRoomDescriptor],
) -> Result<(), PlayError> {
    write_count(w, list.len())?;
    for d in list {
        d.encode(w)?;
    }
    Ok(())
}

/// A `DescriptorHashSet`: the bucket hint and the descriptors. The hint is kept but never
/// used to allocate (§9.1 rule 1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DescriptorSet {
    /// `num_buckets`, as sent.
    pub num_buckets: u16,
    /// The descriptors.
    pub entries: Vec<ChatRoomDescriptor>,
}

impl DescriptorSet {
    /// Decodes a `DescriptorHashSet`.
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation or a bad descriptor.
    pub fn decode(r: &mut WireReader<'_>) -> Result<Self, PlayError> {
        let num_buckets = r.read_u16_le()?;
        let entries = read_descriptor_list(r)?;
        Ok(Self {
            num_buckets,
            entries,
        })
    }

    /// Encodes a `DescriptorHashSet`.
    ///
    /// # Errors
    ///
    /// As [`write_descriptor_list`].
    pub fn encode(&self, w: &mut WireWriter) -> Result<(), PlayError> {
        w.write_u16_le(self.num_buckets);
        write_descriptor_list(w, &self.entries)
    }
}

/// `U64List`: `u32 LE` count + `u64 LE` values.
///
/// # Errors
///
/// [`PlayError`] on truncation.
pub fn read_u64_list(r: &mut WireReader<'_>) -> Result<Vec<u64>, PlayError> {
    let n = read_count(r, 8)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.read_u64_le()?);
    }
    Ok(out)
}

/// Writes a `U64List`.
///
/// # Errors
///
/// [`PlayError::TooMany`] over [`MAX_CONTAINER_ENTRIES`].
pub fn write_u64_list(w: &mut WireWriter, list: &[u64]) -> Result<(), PlayError> {
    write_count(w, list.len())?;
    for v in list {
        w.write_u64_le(*v);
    }
    Ok(())
}

/// A `U64ToU32HashTable`: the bucket hint and the `(key, value)` entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct U64ToU32Table {
    /// `num_buckets`, as sent (never used to allocate).
    pub num_buckets: u16,
    /// The entries.
    pub entries: Vec<(u64, u32)>,
}

impl U64ToU32Table {
    /// Decodes the table (12 bytes per entry, §9.1 rule 1).
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation.
    pub fn decode(r: &mut WireReader<'_>) -> Result<Self, PlayError> {
        let num_buckets = r.read_u16_le()?;
        let n = read_count(r, 12)?;
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push((r.read_u64_le()?, r.read_u32_le()?));
        }
        Ok(Self {
            num_buckets,
            entries,
        })
    }

    /// Encodes the table.
    ///
    /// # Errors
    ///
    /// [`PlayError::TooMany`] over [`MAX_CONTAINER_ENTRIES`].
    pub fn encode(&self, w: &mut WireWriter) -> Result<(), PlayError> {
        w.write_u16_le(self.num_buckets);
        write_count(w, self.entries.len())?;
        for (k, v) in &self.entries {
            w.write_u64_le(*k);
            w.write_u32_le(*v);
        }
        Ok(())
    }
}

/// The optional extra payload of a `ChatMessagePayload` (`extra_type`, §6.6.4).
#[derive(Debug, Clone, PartialEq)]
pub enum ChatExtra {
    /// `0`: none.
    None,
    /// `1`: `(key, descriptor, flags)` entries.
    Descriptors {
        /// `num_buckets`, as sent.
        num_buckets: u16,
        /// The entries.
        entries: Vec<(u64, ChatRoomDescriptor, u32)>,
    },
    /// `2`: a property table and an optional `(key, descriptor)` table.
    Properties {
        /// The property table.
        properties: EncodedPropertyTable,
        /// `has_table` and its entries: `(num_buckets, entries)`.
        table: Option<(u16, Vec<(u64, ChatRoomDescriptor)>)>,
    },
}

/// A `ChatMessagePayload` (§6.6.4), the `raw_data` of a `msg_type = 0x08020000` message.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessagePayload {
    /// The sender.
    pub sender: ChatRoomDescriptor,
    /// The text.
    pub text: String,
    /// Item links or formatting metadata (empty when none).
    pub metadata: Vec<u8>,
    /// The extra payload.
    pub extra: ChatExtra,
    /// Primary message flags.
    pub msg_flags: u32,
    /// The chat mode / sub-flags word (`0` on standard room messages; OPEN otherwise).
    pub chat_mode_word: u32,
}

impl ChatMessagePayload {
    /// A plain text message: no metadata, no extra payload, flags `0`.
    #[must_use]
    pub fn text(sender: ChatRoomDescriptor, text: impl Into<String>) -> Self {
        Self {
            sender,
            text: text.into(),
            metadata: Vec::new(),
            extra: ChatExtra::None,
            msg_flags: 0,
            chat_mode_word: 0,
        }
    }

    /// Encodes the payload.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when a part cannot be encoded.
    pub fn encode(&self, w: &mut WireWriter) -> Result<(), PlayError> {
        self.sender.encode(w)?;
        w.write_utf16le_string(&self.text)?;
        write_bytes_prefixed(w, &self.metadata)?;
        match &self.extra {
            ChatExtra::None => w.write_u32_le(0),
            ChatExtra::Descriptors {
                num_buckets,
                entries,
            } => {
                w.write_u32_le(1);
                w.write_u16_le(*num_buckets);
                write_count(w, entries.len())?;
                for (k, d, f) in entries {
                    w.write_u64_le(*k);
                    d.encode(w)?;
                    w.write_u32_le(*f);
                }
            }
            ChatExtra::Properties { properties, table } => {
                w.write_u32_le(2);
                properties.encode(w)?;
                w.write_bool8(table.is_some());
                if let Some((num_buckets, entries)) = table {
                    w.write_u16_le(*num_buckets);
                    write_count(w, entries.len())?;
                    for (k, d) in entries {
                        w.write_u64_le(*k);
                        d.encode(w)?;
                    }
                }
            }
        }
        w.write_u32_le(self.msg_flags);
        w.write_u32_le(self.chat_mode_word);
        Ok(())
    }

    /// Decodes a payload; the property table of `extra_type = 2` uses `schema`.
    ///
    /// # Errors
    ///
    /// [`PlayError`] on truncation, a bad descriptor or `bool8`, or an unknown
    /// `extra_type`.
    pub fn decode(
        r: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, PlayError> {
        let sender = ChatRoomDescriptor::decode(r)?;
        let text = r.read_utf16le_string()?;
        let metadata = read_bytes_prefixed(r)?;
        let extra = match r.read_u32_le()? {
            0 => ChatExtra::None,
            1 => {
                let num_buckets = r.read_u16_le()?;
                let n = read_count(r, 16)?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = r.read_u64_le()?;
                    let d = ChatRoomDescriptor::decode(r)?;
                    let f = r.read_u32_le()?;
                    entries.push((k, d, f));
                }
                ChatExtra::Descriptors {
                    num_buckets,
                    entries,
                }
            }
            2 => {
                let properties = EncodedPropertyTable::decode(r, schema)?;
                let table = if r.read_bool8()? {
                    let num_buckets = r.read_u16_le()?;
                    let n = read_count(r, 12)?;
                    let mut entries = Vec::with_capacity(n);
                    for _ in 0..n {
                        let k = r.read_u64_le()?;
                        entries.push((k, ChatRoomDescriptor::decode(r)?));
                    }
                    Some((num_buckets, entries))
                } else {
                    None
                };
                ChatExtra::Properties { properties, table }
            }
            other => return Err(PlayError::UnknownExtraType(other)),
        };
        let msg_flags = r.read_u32_le()?;
        let chat_mode_word = r.read_u32_le()?;
        Ok(Self {
            sender,
            text,
            metadata,
            extra,
            msg_flags,
            chat_mode_word,
        })
    }

    /// Decodes a whole `raw_data` buffer; trailing bytes are an error.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let p = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(p)
    }

    /// Encodes to a standalone `raw_data` buffer.
    ///
    /// # Errors
    ///
    /// As [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }
}
