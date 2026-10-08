// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the crate.

use std::borrow::Cow;

use tessera_wstate::WStateError;

/// Result alias using the crate's [`WvmError`].
pub type Result<T, E = WvmError> = std::result::Result<T, E>;

/// Everything that can go wrong while extracting, decoding or validating a code block.
///
/// Offsets are byte offsets into the code block (`wlib-bytecode.md` §1) unless the variant
/// says otherwise. The decoder never panics on malformed input; every failure is reported
/// through this type.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum WvmError {
    /// The class-library container around the code block is malformed (`wlib.md` §3).
    #[error("malformed class-library container at record offset {offset}: {reason}")]
    Container {
        /// Offset within the library record.
        offset: usize,
        /// Why it was rejected.
        reason: Cow<'static, str>,
    },

    /// The class library itself failed to decode.
    #[error("class library: {0}")]
    Library(#[from] WStateError),

    /// The library has no bytecode chunk (type 1, `wlib.md` §4.1).
    #[error("class library has no bytecode chunk")]
    NoCode,

    /// An instruction runs past the end of the code block (`wlib-bytecode.md` §3.2).
    #[error(
        "instruction at {offset:#x} (opcode {opcode:#04x}) runs past the end of the code block"
    )]
    Truncated {
        /// Start of the instruction.
        offset: u32,
        /// Its opcode.
        opcode: u8,
    },

    /// An offset that is not a valid instruction position was asked for.
    #[error("offset {offset:#x} is not an instruction start")]
    NotAnInstruction {
        /// The offset.
        offset: u32,
    },

    /// An opcode outside 0x00–0xBF and 0xC4–0xDF (`wlib-bytecode.md` §3.1).
    #[error("invalid opcode {opcode:#04x} at {offset:#x}")]
    InvalidOpcode {
        /// Start of the instruction.
        offset: u32,
        /// The opcode byte.
        opcode: u8,
    },

    /// Byte 1 of an instruction word is not allowed for its opcode (`wlib-bytecode.md`
    /// §3.1, §13 V4).
    #[error("instruction at {offset:#x} (opcode {opcode:#04x}) has type byte {byte}")]
    TypeByte {
        /// Start of the instruction.
        offset: u32,
        /// Its opcode.
        opcode: u8,
        /// The byte found.
        byte: u8,
    },

    /// A body-structure rule of `wlib-bytecode.md` §2 / §12 does not hold.
    #[error("body at {body:#x}: {reason}")]
    Body {
        /// Start of the body (or the offending method-table offset).
        body: u32,
        /// Why it was rejected.
        reason: Cow<'static, str>,
    },

    /// A branch or switch target is not an instruction start inside the same body
    /// (`wlib-bytecode.md` §3.4, §12).
    #[error("instruction at {offset:#x} targets {target:#x}, not an instruction start in its body")]
    Target {
        /// Start of the branching instruction.
        offset: u32,
        /// The target (may be negative or past the block).
        target: i64,
    },

    /// A static-call operand is not a body start (`wlib-bytecode.md` §6.1, §12).
    #[error("static call at {offset:#x} names {target:#x}, which is not a body start")]
    StaticCall {
        /// Start of the call.
        offset: u32,
        /// The operand.
        target: u32,
    },

    /// A frame-cell index lies outside the frame of its body (`wlib-bytecode.md` §12).
    #[error(
        "instruction at {offset:#x} addresses frame cell {index} ({cells} cell(s)) outside the frame"
    )]
    FrameIndex {
        /// Start of the instruction.
        offset: u32,
        /// The cell index relative to FP.
        index: i32,
        /// Width of the access in cells.
        cells: u8,
    },

    /// The code block is larger than this crate supports (offsets must fit in a `u32`).
    #[error("code block of {len} bytes is too large")]
    TooLarge {
        /// The length.
        len: usize,
    },

    /// A memory reservation failed.
    #[error("allocation of {what} failed")]
    Alloc {
        /// What was being allocated.
        what: &'static str,
    },
}

impl WvmError {
    pub(crate) fn container(offset: usize, reason: impl Into<Cow<'static, str>>) -> Self {
        Self::Container {
            offset,
            reason: reason.into(),
        }
    }

    pub(crate) fn body(body: u32, reason: impl Into<Cow<'static, str>>) -> Self {
        Self::Body {
            body,
            reason: reason.into(),
        }
    }
}
