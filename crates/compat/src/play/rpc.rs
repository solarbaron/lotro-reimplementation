// SPDX-License-Identifier: GPL-3.0-or-later
//! Script RPC envelope `0x30` (`script_rpc_message`) and `ScriptArgFrame`
//! (`docs/specs/official-messages-play.md` §5), with typed codecs of the play methods of
//! classes `0x04EC` (chat, §6.3), `0x04F9` (emotes, §6.4) and `0x051E` (logout, §7.1).
//!
//! The argument kinds of each method are those the spec tables give (§6.3.1, §6.3.2, §6.4,
//! §7.1). §5.1 asks implementations to read method signatures from the install's script
//! record at run time; the generic [`decode_args`] / [`encode_args`] take a signature as
//! data so a run-time table can drive them, while [`PlayRpc`] covers only the methods the
//! spec documents.

use crate::play::PlayError;
use crate::world::{WireLocalizedText, WireReader, WireWriter};

/// Event ID of the script RPC envelope (`script_rpc_message`, §5.2).
pub const OPCODE_SCRIPT_RPC: u32 = 0x0000_0030;

/// Script class of chat and social RPCs (§5.4).
pub const CLASS_CHAT: u16 = 0x04EC;
/// Script class of emote RPCs (§5.4).
pub const CLASS_EMOTE: u16 = 0x04F9;
/// Script class of session and logout RPCs (§5.4).
pub const CLASS_LOGOUT: u16 = 0x051E;

/// `node_flags` bit 28: a binary state class (§5.3, §9.1 rule 3).
pub const NODE_FLAG_BINARY_STATE: u32 = 0x1000_0000;
/// `ScriptStateObject` class of a UTF-16 text (`ref_utf16_text`, §5.3).
pub const STATE_CLASS_UTF16_TEXT: u16 = 0x00E1;
/// `ScriptStateObject` class of a localized text (`ref_localized_text`, §5.3).
pub const STATE_CLASS_LOCALIZED_TEXT: u16 = 0x00C7;

/// Function ID of the admin-only incoming tell (`0x84EC002C`): out of scope (decision 0042
/// item 2, §6.3.2); never encoded, ignored and counted when received.
pub const FN_ADMIN_TELL: u32 = 0x84EC_002C;

/// Largest node count accepted in one `ScriptStateObject` bundle. The bound of §9.1 rule 3
/// (`N <= remaining / 4`) already caps it; this smaller cap keeps one argument from holding
/// thousands of nodes (engineering choice).
pub const MAX_STATE_NODES: usize = 64;

/// A script RPC function identifier (§5.1): bit 31 set, the 15-bit class in bits 16..30,
/// the 16-bit method index in bits 0..15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub u32);

impl FunctionId {
    /// The function of `method` in script class `class_id`, or `None` when the class does
    /// not fit in 15 bits.
    #[must_use]
    pub const fn new(class_id: u16, method: u16) -> Option<Self> {
        if class_id > 0x7FFF {
            return None;
        }
        Some(Self(
            0x8000_0000 | ((class_id as u32) << 16) | method as u32,
        ))
    }

    /// Whether bit 31 is set (§5.1).
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 & 0x8000_0000 != 0
    }

    /// The 15-bit script class.
    #[must_use]
    pub const fn class_id(self) -> u16 {
        ((self.0 >> 16) & 0x7FFF) as u16
    }

    /// The 16-bit method index.
    #[must_use]
    pub const fn method(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }
}

/// Event `0x30` (`script_rpc_message`, C→S and S→C): the RPC envelope of §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRpcMessage {
    /// The method (§5.1).
    pub function_id: u32,
    /// The `ScriptArgFrame` bytes (§5.3), exactly `arg_frame_len` long.
    pub arg_frame: Vec<u8>,
}

impl ScriptRpcMessage {
    /// Encodes the inner payload: `base_flags`, `function_id`, `arg_frame_len`, `arg_frame`.
    ///
    /// # Errors
    ///
    /// [`PlayError::InvalidFunctionId`] when bit 31 of the function ID is clear, or
    /// [`PlayError::TooMany`] when the frame does not fit a `u32` length.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        if !FunctionId(self.function_id).is_valid() {
            return Err(PlayError::InvalidFunctionId(self.function_id));
        }
        let len = u32::try_from(self.arg_frame.len()).map_err(|_| PlayError::TooMany {
            what: "script argument frame bytes",
            max: u32::MAX as usize,
        })?;
        writer.write_base_flags();
        writer.write_u32_le(self.function_id);
        writer.write_u32_le(len);
        writer.write_bytes(&self.arg_frame);
        Ok(())
    }

    /// Decodes the inner payload; the argument frame is sliced to exactly `arg_frame_len`
    /// bytes (§9.1 rule 5).
    ///
    /// # Errors
    ///
    /// [`PlayError::World`] on truncation or a bad `base_flags` byte, and
    /// [`PlayError::InvalidFunctionId`] when bit 31 is clear.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, PlayError> {
        reader.read_base_flags()?;
        let function_id = reader.read_u32_le()?;
        if !FunctionId(function_id).is_valid() {
            return Err(PlayError::InvalidFunctionId(function_id));
        }
        let len = reader.read_u32_le()? as usize;
        reader.ensure_count(len, 1)?;
        let arg_frame = reader.read_bytes(len)?.to_vec();
        Ok(Self {
            function_id,
            arg_frame,
        })
    }

    /// Encodes to a standalone inner payload.
    ///
    /// # Errors
    ///
    /// As [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::with_capacity(9 + self.arg_frame.len());
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes a standalone inner payload; trailing bytes are an error.
    ///
    /// # Errors
    ///
    /// As [`Self::decode`], plus trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// The wire kind of one `ScriptArgFrame` argument (§5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgKind {
    /// `i32 LE` (`0x008E0002` / `0x008E0007`).
    I32,
    /// `u32 LE` (`0x008E0002` / `0x008E0007`).
    U32,
    /// `i64 LE` (`0x008E0003` / `0x008E0008`).
    I64,
    /// `u64 LE` (`0x008E0003` / `0x008E0008`).
    U64,
    /// `f32 LE` (`0x008E0004` / `0x008E0009`).
    F32,
    /// `f64 LE` (`0x008E0005` / `0x008E000A`).
    F64,
    /// A `ScriptStateObject` reference of text classes `0xE1` or `0xC7`
    /// (`0x008E0001` / `0x008E0006`).
    Ref,
    /// A `ScriptStateObject` reference of classes this layer cannot size, kept as raw
    /// bytes; only valid as the last argument of a frame (it takes the rest of the frame).
    OpaqueRef,
}

/// The value of one node of a `ScriptStateObject` bundle (§5.3).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    /// Class `0xE1`: a prefixed UTF-16LE string.
    Text(String),
    /// Class `0xC7`: a wire `LocalizedTokenText` (`official-messages-world.md` §2.6).
    Localized(WireLocalizedText),
}

/// One node of a `ScriptStateObject` bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct StateNode {
    /// The node label ID (`node_ids[i]`).
    pub label: u32,
    /// `node_flags` (bit 28 always set).
    pub flags: u32,
    /// The class payload.
    pub value: NodeValue,
}

/// A `ScriptStateObject` reference argument (§5.3).
#[derive(Debug, Clone, PartialEq)]
pub enum StateRef {
    /// The null reference (node count `0`).
    Null,
    /// Decoded text nodes.
    Nodes(Vec<StateNode>),
    /// The raw bytes of a bundle of other classes ([`ArgKind::OpaqueRef`]).
    Opaque(Vec<u8>),
}

impl StateRef {
    /// A `ref_utf16_text` holding `text`: one node, label `0`, flags `0x10000000`.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Nodes(vec![StateNode {
            label: 0,
            flags: NODE_FLAG_BINARY_STATE,
            value: NodeValue::Text(text.into()),
        }])
    }

    /// A `ref_localized_text` holding `text`.
    #[must_use]
    pub fn localized(text: WireLocalizedText) -> Self {
        Self::Nodes(vec![StateNode {
            label: 0,
            flags: NODE_FLAG_BINARY_STATE,
            value: NodeValue::Localized(text),
        }])
    }

    /// The value of a single-node reference.
    #[must_use]
    pub fn single(&self) -> Option<&NodeValue> {
        match self {
            Self::Nodes(nodes) if nodes.len() == 1 => nodes.first().map(|n| &n.value),
            _ => None,
        }
    }

    /// The plain text of a single `0xE1` node.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self.single() {
            Some(NodeValue::Text(s)) => Some(s),
            _ => None,
        }
    }

    fn encode(&self, writer: &mut WireWriter) -> Result<(), PlayError> {
        match self {
            Self::Null => writer.write_vle4(0),
            Self::Opaque(bytes) => writer.write_bytes(bytes),
            Self::Nodes(nodes) => {
                if nodes.len() > MAX_STATE_NODES {
                    return Err(PlayError::TooMany {
                        what: "script state nodes",
                        max: MAX_STATE_NODES,
                    });
                }
                // Bounded by MAX_STATE_NODES above.
                writer.write_vle4(nodes.len() as u32);
                for node in nodes {
                    writer.write_u32_le(node.label);
                }
                for node in nodes {
                    if node.flags & NODE_FLAG_BINARY_STATE == 0 {
                        return Err(PlayError::InvalidStateNodeFlags(node.flags));
                    }
                    writer.write_u32_le(node.flags);
                    match &node.value {
                        NodeValue::Text(s) => {
                            writer.write_u16_le(STATE_CLASS_UTF16_TEXT);
                            writer.write_utf16le_string(s)?;
                        }
                        NodeValue::Localized(t) => {
                            writer.write_u16_le(STATE_CLASS_LOCALIZED_TEXT);
                            t.encode(writer)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn decode(reader: &mut WireReader<'_>) -> Result<Self, PlayError> {
        let count = reader.read_vle4()? as usize;
        if count == 0 {
            return Ok(Self::Null);
        }
        // §9.1 rule 3: N <= remaining / 4 before reading the label array.
        reader.ensure_count(count, 4)?;
        if count > MAX_STATE_NODES {
            return Err(PlayError::TooMany {
                what: "script state nodes",
                max: MAX_STATE_NODES,
            });
        }
        let mut labels = Vec::with_capacity(count);
        for _ in 0..count {
            labels.push(reader.read_u32_le()?);
        }
        let mut nodes = Vec::with_capacity(count);
        for label in labels {
            let flags = reader.read_u32_le()?;
            if flags & NODE_FLAG_BINARY_STATE == 0 {
                return Err(PlayError::InvalidStateNodeFlags(flags));
            }
            let class_id = reader.read_u16_le()?;
            let value = match class_id {
                STATE_CLASS_UTF16_TEXT => NodeValue::Text(reader.read_utf16le_string()?),
                STATE_CLASS_LOCALIZED_TEXT => {
                    NodeValue::Localized(WireLocalizedText::decode(reader)?)
                }
                other => return Err(PlayError::UnsupportedStateClass(other)),
            };
            nodes.push(StateNode {
                label,
                flags,
                value,
            });
        }
        Ok(Self::Nodes(nodes))
    }
}

/// One decoded `ScriptArgFrame` argument.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptArg {
    /// `i32`.
    I32(i32),
    /// `u32`.
    U32(u32),
    /// `i64`.
    I64(i64),
    /// `u64`.
    U64(u64),
    /// `f32`.
    F32(f32),
    /// `f64`.
    F64(f64),
    /// A `ScriptStateObject` reference.
    Ref(StateRef),
}

impl ScriptArg {
    fn kind_matches(&self, kind: ArgKind) -> bool {
        matches!(
            (self, kind),
            (Self::I32(_), ArgKind::I32)
                | (Self::U32(_), ArgKind::U32)
                | (Self::I64(_), ArgKind::I64)
                | (Self::U64(_), ArgKind::U64)
                | (Self::F32(_), ArgKind::F32)
                | (Self::F64(_), ArgKind::F64)
                | (Self::Ref(_), ArgKind::Ref | ArgKind::OpaqueRef)
        )
    }
}

/// Decodes `frame` as the arguments of `signature`, in order (§5.3). The frame must be
/// consumed exactly (engineering choice: a frame longer than its signature is refused).
///
/// # Errors
///
/// [`PlayError`] on truncation, a bad reference bundle, an [`ArgKind::OpaqueRef`] that is
/// not last, or trailing bytes.
pub fn decode_args(frame: &[u8], signature: &[ArgKind]) -> Result<Vec<ScriptArg>, PlayError> {
    let mut reader = WireReader::new(frame);
    let mut out = Vec::with_capacity(signature.len());
    for (index, kind) in signature.iter().enumerate() {
        let arg = match kind {
            ArgKind::I32 => ScriptArg::I32(reader.read_i32_le()?),
            ArgKind::U32 => ScriptArg::U32(reader.read_u32_le()?),
            ArgKind::I64 => ScriptArg::I64(reader.read_i64_le()?),
            ArgKind::U64 => ScriptArg::U64(reader.read_u64_le()?),
            ArgKind::F32 => ScriptArg::F32(reader.read_f32_le()?),
            ArgKind::F64 => ScriptArg::F64(reader.read_f64_le()?),
            ArgKind::Ref => ScriptArg::Ref(StateRef::decode(&mut reader)?),
            ArgKind::OpaqueRef => {
                if index + 1 != signature.len() {
                    return Err(PlayError::ArgumentMismatch {
                        function_id: 0,
                        index,
                    });
                }
                // The node count still bounds the label array (§9.1 rule 3); the class
                // payloads stay raw.
                let mut probe = reader.clone();
                let count = probe.read_vle4()? as usize;
                probe.ensure_count(count, 4)?;
                let rest = reader.remaining_slice().to_vec();
                let _ = reader.read_bytes(rest.len())?;
                if count == 0 && rest.len() == 1 {
                    ScriptArg::Ref(StateRef::Null)
                } else {
                    ScriptArg::Ref(StateRef::Opaque(rest))
                }
            }
        };
        out.push(arg);
    }
    reader.finish()?;
    Ok(out)
}

/// Encodes `args` as a `ScriptArgFrame` after checking them against `signature`.
///
/// # Errors
///
/// [`PlayError::ArgumentMismatch`] when an argument does not match its kind or the counts
/// differ, or an encoding error of a reference.
pub fn encode_args(
    function_id: u32,
    signature: &[ArgKind],
    args: &[ScriptArg],
) -> Result<Vec<u8>, PlayError> {
    if signature.len() != args.len() {
        return Err(PlayError::ArgumentMismatch {
            function_id,
            index: signature.len().min(args.len()),
        });
    }
    let mut w = WireWriter::new();
    for (index, (kind, arg)) in signature.iter().zip(args).enumerate() {
        if !arg.kind_matches(*kind) {
            return Err(PlayError::ArgumentMismatch { function_id, index });
        }
        match arg {
            ScriptArg::I32(v) => w.write_i32_le(*v),
            ScriptArg::U32(v) => w.write_u32_le(*v),
            ScriptArg::I64(v) => w.write_i64_le(*v),
            ScriptArg::U64(v) => w.write_u64_le(*v),
            ScriptArg::F32(v) => w.write_f32_le(*v),
            ScriptArg::F64(v) => w.write_f64_le(*v),
            ScriptArg::Ref(r) => r.encode(&mut w)?,
        }
    }
    Ok(w.into_vec())
}

// ---------------------------------------------------------------------------
// Documented play methods (§6.3, §6.4, §7.1)
// ---------------------------------------------------------------------------

use ArgKind::{I32, OpaqueRef, Ref, U32, U64};

/// Direction of a documented RPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RpcDirection {
    /// Client to server.
    ClientToServer,
    /// Server to client.
    ServerToClient,
}

/// The argument kinds and direction of a method the play spec documents, by function ID
/// (§6.3.1, §6.3.2, §6.4, §7.1). `None` for any other function, including the admin-only
/// `0x84EC002C`, which this layer never decodes.
#[must_use]
pub fn documented_signature(function_id: u32) -> Option<(RpcDirection, &'static [ArgKind])> {
    use RpcDirection::{ClientToServer as C, ServerToClient as S};
    let sig: (RpcDirection, &'static [ArgKind]) = match function_id {
        // §6.3.1 client → server chat.
        0x84EC_001B => (C, &[U32, Ref, U32]),
        0x84EC_000E => (C, &[Ref, Ref]),
        0x84EC_0021 => (C, &[Ref]),
        0x84EC_002A => (C, &[U32]),
        0x84EC_0042 => (C, &[Ref, Ref]),
        0x84EC_003F => (C, &[Ref]),
        0x84EC_004D => (C, &[U64]),
        0x84EC_004F => (C, &[U64, Ref]),
        0x84EC_005A | 0x84EC_0088 => (C, &[]),
        0x84EC_005C => (C, &[U32, U64]),
        // §6.3.2 server → client chat.
        0x84EC_0005 => (S, &[U32, Ref, U32, Ref, U64]),
        0x84EC_000A | 0x84EC_001E => (S, &[Ref, U64]),
        0x84EC_001A => (S, &[U32, U64, Ref, Ref]),
        0x84EC_001C => (S, &[U32, Ref, U32]),
        0x84EC_001D | 0x84EC_0040 => (S, &[OpaqueRef]),
        0x84EC_003B => (S, &[Ref, U64, U64]),
        0x84EC_0044 => (S, &[U32, Ref, Ref]),
        0x84EC_0049 => (S, &[Ref, Ref, U64]),
        // §6.4 emotes.
        0x84F9_000A => (C, &[Ref]),
        0x84F9_0008 => (C, &[U64, U32]),
        0x84F9_0005 => (C, &[Ref, U32]),
        0x84F9_0007 => (C, &[U32]),
        0x84F9_0002 => (S, &[Ref, Ref, U64, Ref, U32]),
        0x84F9_0003 => (S, &[Ref, Ref, U64, U32]),
        0x84F9_0006 => (S, &[Ref, Ref, U64]),
        0x84F9_0010 => (S, &[U32, U64]),
        // §7.1 logout.
        0x851E_002F | 0x851E_0006 | 0x851E_0037 | 0x851E_000B | 0x851E_0003 => {
            let dir = if function_id == 0x851E_0003 { S } else { C };
            (dir, &[])
        }
        0x851E_0007 => (S, &[I32, I32, I32]),
        0x851E_000F => (S, &[I32, Ref]),
        0x851E_0012 => (S, &[U64]),
        0x851E_0017 => (S, &[I32]),
        _ => return None,
    };
    Some(sig)
}

/// A documented play RPC, typed (§6.3, §6.4, §7.1). Text arguments are
/// [`StateRef`]s: `ref_utf16_text` and `ref_localized_text` share the wire form and are
/// told apart by their class.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlayRpc {
    // ---- §6.3.1, client → server ----
    /// `0x84EC001B`: `/say` or typed chat.
    Say {
        /// `0` for normal player input.
        flags: u32,
        /// The text.
        text: StateRef,
        /// The chat category (§6.7).
        chat_type: u32,
    },
    /// `0x84EC000E`: `/tell` by name.
    Tell {
        /// The text.
        message: StateRef,
        /// The recipient's name.
        target: StateRef,
    },
    /// `0x84EC0021`: add to the ignore list.
    IgnoreAdd {
        /// The name.
        name: StateRef,
    },
    /// `0x84EC002A`: communication filter or channel opt-in flag.
    SetChatFilter {
        /// The flag value.
        flag: u32,
    },
    /// `0x84EC0042`: two-string social command (annotation).
    SocialPair {
        /// First string.
        a: StateRef,
        /// Second string.
        b: StateRef,
    },
    /// `0x84EC003F`: single-string social or friend command.
    SocialSingle {
        /// The string.
        target: StateRef,
    },
    /// `0x84EC004D`: an incoming tell was rejected by the client's filter.
    TellRejected {
        /// The sender.
        sender: u64,
    },
    /// `0x84EC004F`: remove from the ignore list.
    IgnoreRemove {
        /// The entity.
        entity: u64,
        /// The name.
        name: StateRef,
    },
    /// `0x84EC005A`: auto-gag state acknowledgement (no arguments).
    AutoGagSync,
    /// `0x84EC005C`: report a player for chat spam.
    ReportSpam {
        /// Reason code.
        reason: u32,
        /// The reported entity.
        spammer: u64,
    },
    /// `0x84EC0088`: communication session sync (no arguments).
    CommSessionSync,
    // ---- §6.3.2, server → client ----
    /// `0x84EC0005`: spatial or typed chat line.
    ChatLine {
        /// Flags.
        flags: u32,
        /// The sender's name.
        sender_name: StateRef,
        /// The chat category (§6.7).
        chat_type: u32,
        /// The text.
        text: StateRef,
        /// The sender.
        sender: u64,
    },
    /// `0x84EC000A`: entity-associated text notification.
    EntityText {
        /// The text.
        text: StateRef,
        /// The entity.
        entity: u64,
    },
    /// `0x84EC001A`: outgoing tell echo or delivery status.
    TellStatus {
        /// Status code.
        status: u32,
        /// The recipient.
        target: u64,
        /// The recipient's name.
        target_name: StateRef,
        /// The text.
        text: StateRef,
    },
    /// `0x84EC001C`: friends or ignore list update status.
    SocialListStatus {
        /// Action code.
        action: u32,
        /// The name.
        name: StateRef,
        /// Status code.
        status: u32,
    },
    /// `0x84EC001D`: friends and ignore table (raw bundle).
    SocialTable {
        /// The bundle.
        table: StateRef,
    },
    /// `0x84EC001E`: entity communication status notice.
    EntityCommNotice {
        /// The text.
        text: StateRef,
        /// The entity.
        entity: u64,
    },
    /// `0x84EC003B`: system or channel notice with two entity IDs.
    ChannelNotice {
        /// The text.
        text: StateRef,
        /// First entity.
        entity_a: u64,
        /// Second entity.
        entity_b: u64,
    },
    /// `0x84EC0040`: social annotation state (raw bundle).
    SocialState {
        /// The bundle.
        state: StateRef,
    },
    /// `0x84EC0044`: social command status with two texts.
    SocialCommandStatus {
        /// Status code.
        status: u32,
        /// First text.
        text_a: StateRef,
        /// Second text.
        text_b: StateRef,
    },
    /// `0x84EC0049`: an incoming player tell.
    TellReceived {
        /// The text.
        text: StateRef,
        /// The sender's name.
        sender_name: StateRef,
        /// The sender.
        sender: u64,
    },
    // ---- §6.4 ----
    /// `0x84F9000A`: custom text emote.
    EmoteText {
        /// The text (at most 1155 code units).
        text: StateRef,
    },
    /// `0x84F90008`: predefined emote on a target entity.
    EmoteAtEntity {
        /// The target.
        target: u64,
        /// The emote definition DID.
        emote_did: u32,
    },
    /// `0x84F90005`: predefined emote, untargeted (null name) or by target name.
    EmoteByName {
        /// The target's name, or [`StateRef::Null`].
        target_name: StateRef,
        /// The emote definition DID.
        emote_did: u32,
    },
    /// `0x84F90007`: predefined emote by ID.
    EmoteById {
        /// The emote definition DID.
        emote_did: u32,
    },
    /// `0x84F90002`: targeted predefined emote broadcast.
    EmoteTargeted {
        /// Source name.
        source_name: StateRef,
        /// Target name.
        target_name: StateRef,
        /// The source.
        source: u64,
        /// The emote line.
        message: StateRef,
        /// The emote definition DID.
        emote_did: u32,
    },
    /// `0x84F90003`: untargeted predefined emote broadcast.
    EmoteUntargeted {
        /// Source name.
        source_name: StateRef,
        /// The emote line.
        message: StateRef,
        /// The source.
        source: u64,
        /// The emote definition DID.
        emote_did: u32,
    },
    /// `0x84F90006`: custom text emote broadcast.
    EmoteCustom {
        /// Source name.
        source_name: StateRef,
        /// The text.
        text: StateRef,
        /// The source.
        source: u64,
    },
    /// `0x84F90010`: an emote was granted.
    EmoteGranted {
        /// The emote definition DID.
        emote_did: u32,
        /// The player.
        player: u64,
    },
    // ---- §7.1 ----
    /// `0x851E002F`: request logout to character selection.
    LogoutRequest,
    /// `0x851E0006`: cancel the logout countdown.
    LogoutCancel,
    /// `0x851E0037`: immediate or secondary world exit request.
    WorldExitRequest,
    /// `0x851E000B`: auxiliary session state acknowledgement.
    SessionAuxAck,
    /// `0x851E0007`: logout countdown started or status.
    LogoutCountdown {
        /// Countdown duration or status code.
        arg0: i32,
        /// Reason or mode code.
        arg1: i32,
        /// Flags.
        arg2: i32,
    },
    /// `0x851E0003`: logout cancelled.
    LogoutCancelled,
    /// `0x851E000F`: localized session notice.
    SessionNotice {
        /// Error or modal flag.
        is_error_or_modal: i32,
        /// The text.
        text: StateRef,
    },
    /// `0x851E0012`: session timestamp or token update.
    SessionValue {
        /// The value.
        value: u64,
    },
    /// `0x851E0017`: session state code.
    SessionStateCode {
        /// The code.
        code: i32,
    },
}

fn take_u32(args: &mut std::vec::IntoIter<ScriptArg>, f: u32, i: usize) -> Result<u32, PlayError> {
    match args.next() {
        Some(ScriptArg::U32(v)) => Ok(v),
        _ => Err(PlayError::ArgumentMismatch {
            function_id: f,
            index: i,
        }),
    }
}

fn take_i32(args: &mut std::vec::IntoIter<ScriptArg>, f: u32, i: usize) -> Result<i32, PlayError> {
    match args.next() {
        Some(ScriptArg::I32(v)) => Ok(v),
        _ => Err(PlayError::ArgumentMismatch {
            function_id: f,
            index: i,
        }),
    }
}

fn take_u64(args: &mut std::vec::IntoIter<ScriptArg>, f: u32, i: usize) -> Result<u64, PlayError> {
    match args.next() {
        Some(ScriptArg::U64(v)) => Ok(v),
        _ => Err(PlayError::ArgumentMismatch {
            function_id: f,
            index: i,
        }),
    }
}

fn take_ref(
    args: &mut std::vec::IntoIter<ScriptArg>,
    f: u32,
    i: usize,
) -> Result<StateRef, PlayError> {
    match args.next() {
        Some(ScriptArg::Ref(v)) => Ok(v),
        _ => Err(PlayError::ArgumentMismatch {
            function_id: f,
            index: i,
        }),
    }
}

impl PlayRpc {
    /// The function ID of this RPC.
    #[must_use]
    pub fn function_id(&self) -> u32 {
        match self {
            Self::Say { .. } => 0x84EC_001B,
            Self::Tell { .. } => 0x84EC_000E,
            Self::IgnoreAdd { .. } => 0x84EC_0021,
            Self::SetChatFilter { .. } => 0x84EC_002A,
            Self::SocialPair { .. } => 0x84EC_0042,
            Self::SocialSingle { .. } => 0x84EC_003F,
            Self::TellRejected { .. } => 0x84EC_004D,
            Self::IgnoreRemove { .. } => 0x84EC_004F,
            Self::AutoGagSync => 0x84EC_005A,
            Self::ReportSpam { .. } => 0x84EC_005C,
            Self::CommSessionSync => 0x84EC_0088,
            Self::ChatLine { .. } => 0x84EC_0005,
            Self::EntityText { .. } => 0x84EC_000A,
            Self::TellStatus { .. } => 0x84EC_001A,
            Self::SocialListStatus { .. } => 0x84EC_001C,
            Self::SocialTable { .. } => 0x84EC_001D,
            Self::EntityCommNotice { .. } => 0x84EC_001E,
            Self::ChannelNotice { .. } => 0x84EC_003B,
            Self::SocialState { .. } => 0x84EC_0040,
            Self::SocialCommandStatus { .. } => 0x84EC_0044,
            Self::TellReceived { .. } => 0x84EC_0049,
            Self::EmoteText { .. } => 0x84F9_000A,
            Self::EmoteAtEntity { .. } => 0x84F9_0008,
            Self::EmoteByName { .. } => 0x84F9_0005,
            Self::EmoteById { .. } => 0x84F9_0007,
            Self::EmoteTargeted { .. } => 0x84F9_0002,
            Self::EmoteUntargeted { .. } => 0x84F9_0003,
            Self::EmoteCustom { .. } => 0x84F9_0006,
            Self::EmoteGranted { .. } => 0x84F9_0010,
            Self::LogoutRequest => 0x851E_002F,
            Self::LogoutCancel => 0x851E_0006,
            Self::WorldExitRequest => 0x851E_0037,
            Self::SessionAuxAck => 0x851E_000B,
            Self::LogoutCountdown { .. } => 0x851E_0007,
            Self::LogoutCancelled => 0x851E_0003,
            Self::SessionNotice { .. } => 0x851E_000F,
            Self::SessionValue { .. } => 0x851E_0012,
            Self::SessionStateCode { .. } => 0x851E_0017,
        }
    }

    /// The direction the spec gives this RPC.
    #[must_use]
    pub fn direction(&self) -> RpcDirection {
        documented_signature(self.function_id()).map_or(RpcDirection::ServerToClient, |(d, _)| d)
    }

    fn args(&self) -> Vec<ScriptArg> {
        use ScriptArg as A;
        match self.clone() {
            Self::Say {
                flags,
                text,
                chat_type,
            } => vec![A::U32(flags), A::Ref(text), A::U32(chat_type)],
            Self::Tell { message, target } => vec![A::Ref(message), A::Ref(target)],
            Self::IgnoreAdd { name } => vec![A::Ref(name)],
            Self::SetChatFilter { flag } => vec![A::U32(flag)],
            Self::SocialPair { a, b } => vec![A::Ref(a), A::Ref(b)],
            Self::SocialSingle { target } => vec![A::Ref(target)],
            Self::TellRejected { sender } => vec![A::U64(sender)],
            Self::IgnoreRemove { entity, name } => vec![A::U64(entity), A::Ref(name)],
            Self::ReportSpam { reason, spammer } => vec![A::U32(reason), A::U64(spammer)],
            Self::ChatLine {
                flags,
                sender_name,
                chat_type,
                text,
                sender,
            } => vec![
                A::U32(flags),
                A::Ref(sender_name),
                A::U32(chat_type),
                A::Ref(text),
                A::U64(sender),
            ],
            Self::EntityText { text, entity } | Self::EntityCommNotice { text, entity } => {
                vec![A::Ref(text), A::U64(entity)]
            }
            Self::TellStatus {
                status,
                target,
                target_name,
                text,
            } => vec![
                A::U32(status),
                A::U64(target),
                A::Ref(target_name),
                A::Ref(text),
            ],
            Self::SocialListStatus {
                action,
                name,
                status,
            } => vec![A::U32(action), A::Ref(name), A::U32(status)],
            Self::SocialTable { table: r } | Self::SocialState { state: r } => vec![A::Ref(r)],
            Self::ChannelNotice {
                text,
                entity_a,
                entity_b,
            } => vec![A::Ref(text), A::U64(entity_a), A::U64(entity_b)],
            Self::SocialCommandStatus {
                status,
                text_a,
                text_b,
            } => vec![A::U32(status), A::Ref(text_a), A::Ref(text_b)],
            Self::TellReceived {
                text,
                sender_name,
                sender,
            } => vec![A::Ref(text), A::Ref(sender_name), A::U64(sender)],
            Self::EmoteText { text } => vec![A::Ref(text)],
            Self::EmoteAtEntity { target, emote_did } => vec![A::U64(target), A::U32(emote_did)],
            Self::EmoteByName {
                target_name,
                emote_did,
            } => vec![A::Ref(target_name), A::U32(emote_did)],
            Self::EmoteById { emote_did } => vec![A::U32(emote_did)],
            Self::EmoteTargeted {
                source_name,
                target_name,
                source,
                message,
                emote_did,
            } => vec![
                A::Ref(source_name),
                A::Ref(target_name),
                A::U64(source),
                A::Ref(message),
                A::U32(emote_did),
            ],
            Self::EmoteUntargeted {
                source_name,
                message,
                source,
                emote_did,
            } => vec![
                A::Ref(source_name),
                A::Ref(message),
                A::U64(source),
                A::U32(emote_did),
            ],
            Self::EmoteCustom {
                source_name,
                text,
                source,
            } => vec![A::Ref(source_name), A::Ref(text), A::U64(source)],
            Self::EmoteGranted { emote_did, player } => vec![A::U32(emote_did), A::U64(player)],
            Self::LogoutCountdown { arg0, arg1, arg2 } => {
                vec![A::I32(arg0), A::I32(arg1), A::I32(arg2)]
            }
            Self::SessionNotice {
                is_error_or_modal,
                text,
            } => vec![A::I32(is_error_or_modal), A::Ref(text)],
            Self::SessionValue { value } => vec![A::U64(value)],
            Self::SessionStateCode { code } => vec![A::I32(code)],
            Self::AutoGagSync
            | Self::CommSessionSync
            | Self::LogoutRequest
            | Self::LogoutCancel
            | Self::WorldExitRequest
            | Self::SessionAuxAck
            | Self::LogoutCancelled => Vec::new(),
        }
    }

    /// The envelope of this RPC.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when an argument cannot be encoded.
    pub fn to_message(&self) -> Result<ScriptRpcMessage, PlayError> {
        let function_id = self.function_id();
        let (_, sig) =
            documented_signature(function_id).ok_or(PlayError::InvalidFunctionId(function_id))?;
        let arg_frame = encode_args(function_id, sig, &self.args())?;
        Ok(ScriptRpcMessage {
            function_id,
            arg_frame,
        })
    }

    /// The `0x30` inner payload of this RPC.
    ///
    /// # Errors
    ///
    /// As [`Self::to_message`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlayError> {
        self.to_message()?.to_bytes()
    }

    /// Decodes a documented RPC from its envelope; `Ok(None)` for any other function.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when the argument frame does not match the documented signature.
    pub fn from_message(msg: &ScriptRpcMessage) -> Result<Option<Self>, PlayError> {
        let f = msg.function_id;
        let Some((_, sig)) = documented_signature(f) else {
            return Ok(None);
        };
        let mut a = decode_args(&msg.arg_frame, sig)
            .map_err(|e| match e {
                PlayError::ArgumentMismatch { index, .. } => PlayError::ArgumentMismatch {
                    function_id: f,
                    index,
                },
                other => other,
            })?
            .into_iter();
        let a = &mut a;
        let rpc = match f {
            0x84EC_001B => Self::Say {
                flags: take_u32(a, f, 0)?,
                text: take_ref(a, f, 1)?,
                chat_type: take_u32(a, f, 2)?,
            },
            0x84EC_000E => Self::Tell {
                message: take_ref(a, f, 0)?,
                target: take_ref(a, f, 1)?,
            },
            0x84EC_0021 => Self::IgnoreAdd {
                name: take_ref(a, f, 0)?,
            },
            0x84EC_002A => Self::SetChatFilter {
                flag: take_u32(a, f, 0)?,
            },
            0x84EC_0042 => Self::SocialPair {
                a: take_ref(a, f, 0)?,
                b: take_ref(a, f, 1)?,
            },
            0x84EC_003F => Self::SocialSingle {
                target: take_ref(a, f, 0)?,
            },
            0x84EC_004D => Self::TellRejected {
                sender: take_u64(a, f, 0)?,
            },
            0x84EC_004F => Self::IgnoreRemove {
                entity: take_u64(a, f, 0)?,
                name: take_ref(a, f, 1)?,
            },
            0x84EC_005A => Self::AutoGagSync,
            0x84EC_005C => Self::ReportSpam {
                reason: take_u32(a, f, 0)?,
                spammer: take_u64(a, f, 1)?,
            },
            0x84EC_0088 => Self::CommSessionSync,
            0x84EC_0005 => Self::ChatLine {
                flags: take_u32(a, f, 0)?,
                sender_name: take_ref(a, f, 1)?,
                chat_type: take_u32(a, f, 2)?,
                text: take_ref(a, f, 3)?,
                sender: take_u64(a, f, 4)?,
            },
            0x84EC_000A => Self::EntityText {
                text: take_ref(a, f, 0)?,
                entity: take_u64(a, f, 1)?,
            },
            0x84EC_001E => Self::EntityCommNotice {
                text: take_ref(a, f, 0)?,
                entity: take_u64(a, f, 1)?,
            },
            0x84EC_001A => Self::TellStatus {
                status: take_u32(a, f, 0)?,
                target: take_u64(a, f, 1)?,
                target_name: take_ref(a, f, 2)?,
                text: take_ref(a, f, 3)?,
            },
            0x84EC_001C => Self::SocialListStatus {
                action: take_u32(a, f, 0)?,
                name: take_ref(a, f, 1)?,
                status: take_u32(a, f, 2)?,
            },
            0x84EC_001D => Self::SocialTable {
                table: take_ref(a, f, 0)?,
            },
            0x84EC_0040 => Self::SocialState {
                state: take_ref(a, f, 0)?,
            },
            0x84EC_003B => Self::ChannelNotice {
                text: take_ref(a, f, 0)?,
                entity_a: take_u64(a, f, 1)?,
                entity_b: take_u64(a, f, 2)?,
            },
            0x84EC_0044 => Self::SocialCommandStatus {
                status: take_u32(a, f, 0)?,
                text_a: take_ref(a, f, 1)?,
                text_b: take_ref(a, f, 2)?,
            },
            0x84EC_0049 => Self::TellReceived {
                text: take_ref(a, f, 0)?,
                sender_name: take_ref(a, f, 1)?,
                sender: take_u64(a, f, 2)?,
            },
            0x84F9_000A => Self::EmoteText {
                text: take_ref(a, f, 0)?,
            },
            0x84F9_0008 => Self::EmoteAtEntity {
                target: take_u64(a, f, 0)?,
                emote_did: take_u32(a, f, 1)?,
            },
            0x84F9_0005 => Self::EmoteByName {
                target_name: take_ref(a, f, 0)?,
                emote_did: take_u32(a, f, 1)?,
            },
            0x84F9_0007 => Self::EmoteById {
                emote_did: take_u32(a, f, 0)?,
            },
            0x84F9_0002 => Self::EmoteTargeted {
                source_name: take_ref(a, f, 0)?,
                target_name: take_ref(a, f, 1)?,
                source: take_u64(a, f, 2)?,
                message: take_ref(a, f, 3)?,
                emote_did: take_u32(a, f, 4)?,
            },
            0x84F9_0003 => Self::EmoteUntargeted {
                source_name: take_ref(a, f, 0)?,
                message: take_ref(a, f, 1)?,
                source: take_u64(a, f, 2)?,
                emote_did: take_u32(a, f, 3)?,
            },
            0x84F9_0006 => Self::EmoteCustom {
                source_name: take_ref(a, f, 0)?,
                text: take_ref(a, f, 1)?,
                source: take_u64(a, f, 2)?,
            },
            0x84F9_0010 => Self::EmoteGranted {
                emote_did: take_u32(a, f, 0)?,
                player: take_u64(a, f, 1)?,
            },
            0x851E_002F => Self::LogoutRequest,
            0x851E_0006 => Self::LogoutCancel,
            0x851E_0037 => Self::WorldExitRequest,
            0x851E_000B => Self::SessionAuxAck,
            0x851E_0003 => Self::LogoutCancelled,
            0x851E_0007 => Self::LogoutCountdown {
                arg0: take_i32(a, f, 0)?,
                arg1: take_i32(a, f, 1)?,
                arg2: take_i32(a, f, 2)?,
            },
            0x851E_000F => Self::SessionNotice {
                is_error_or_modal: take_i32(a, f, 0)?,
                text: take_ref(a, f, 1)?,
            },
            0x851E_0012 => Self::SessionValue {
                value: take_u64(a, f, 0)?,
            },
            0x851E_0017 => Self::SessionStateCode {
                code: take_i32(a, f, 0)?,
            },
            _ => return Ok(None),
        };
        Ok(Some(rpc))
    }
}
