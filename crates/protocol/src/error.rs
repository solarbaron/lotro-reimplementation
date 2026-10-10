// SPDX-License-Identifier: GPL-3.0-or-later
//! The codec's error type.

use crate::codes::AppErrorCode;
use crate::message::{Channel, Side};

/// A broken wire rule, found while decoding received bytes or while encoding a value that
/// would break the spec.
///
/// Every decode error is a protocol violation (`tessera-protocol.md` §0, §13.1): the
/// receiver ends the connection with [`AppErrorCode::ProtocolViolation`]. The encoder returns
/// the same errors for values it refuses to put on the wire, so whatever it writes decodes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// The input ended before the message did.
    #[error("message ends early")]
    Truncated,
    /// Bytes were left after the last field of a message (§3).
    #[error("{0} bytes left after the message body")]
    TrailingBytes(usize),
    /// A frame `length` outside 2–65,536 (§5.4).
    #[error("frame length {0} outside 2..=65536")]
    FrameLength(u32),
    /// A message body too large for one frame (encoder only).
    #[error("message body of {0} bytes does not fit in a frame")]
    BodyTooLarge(usize),
    /// A datagram over 1,200 bytes (§5.5).
    #[error("datagram of {0} bytes exceeds 1200")]
    DatagramTooLarge(usize),
    /// A message type this version does not define (§3).
    #[error("unknown message type {0:#06x}")]
    UnknownMessageType(u16),
    /// A message type outside the range of the channel it arrived on (§2.3 rule 3).
    #[error("message type {ty:#06x} not allowed on the {channel} channel")]
    WrongChannel {
        /// The message type.
        ty: u16,
        /// The channel it arrived on.
        channel: Channel,
    },
    /// A message sent in the wrong direction (§2.3 rule 3, §6.1).
    #[error("message type {ty:#06x} may not be sent by the {sender}")]
    WrongDirection {
        /// The message type.
        ty: u16,
        /// Who sent it.
        sender: Side,
    },
    /// A server uni stream started with an unknown kind byte (§2.3 rule 1).
    #[error("unknown stream kind byte {0:#04x}")]
    UnknownStreamKind(u8),
    /// Invalid UTF-8 in a `str` field.
    #[error("{0}: invalid UTF-8")]
    InvalidUtf8(&'static str),
    /// A `str` length outside its text class's range (§5.2).
    #[error("{field}: {len} bytes outside {min}..={max}")]
    StringLength {
        /// The field.
        field: &'static str,
        /// Its length in bytes.
        len: usize,
        /// The smallest allowed length.
        min: usize,
        /// The largest allowed length.
        max: usize,
    },
    /// The encoder refuses a control character in a text field the receiver would sanitise.
    #[error("{0}: control character in text")]
    ControlCharacter(&'static str),
    /// A NaN or infinite float (§5.1).
    #[error("{0}: non-finite float")]
    NonFinite(&'static str),
    /// A `bool` byte other than 0 or 1 (§5.1).
    #[error("{field}: bool byte {value}")]
    InvalidBool {
        /// The field.
        field: &'static str,
        /// The byte read.
        value: u8,
    },
    /// An `opt` tag other than 0 or 1 (§5.1).
    #[error("{field}: option tag {value}")]
    InvalidOption {
        /// The field.
        field: &'static str,
        /// The byte read.
        value: u8,
    },
    /// A value outside the listed range of an enumerated field (§3).
    #[error("{field}: value {value} is not defined")]
    InvalidEnum {
        /// The field.
        field: &'static str,
        /// The value read.
        value: u64,
    },
    /// A number outside its field's range, or a field rule broken.
    #[error("{0}: value out of range")]
    OutOfRange(&'static str),
    /// Reserved bits that must be 0 were set.
    #[error("{0}: reserved bits set")]
    ReservedBits(&'static str),
    /// A count above its maximum, or below its minimum.
    #[error("{field}: count {count} outside {min}..={max}")]
    Count {
        /// The field.
        field: &'static str,
        /// The count read.
        count: usize,
        /// The smallest allowed count.
        min: usize,
        /// The largest allowed count.
        max: usize,
    },
    /// An Anchor that does not own the Location it accompanies (§5.3).
    #[error("anchor does not own the location")]
    AnchorNotOwning,
    /// A list holds the same name or ID twice where the spec forbids it.
    #[error("{0}: duplicate entry")]
    Duplicate(&'static str),
    /// Property IDs or struct field IDs not strictly ascending, or zero (§7.2).
    #[error("{0}: IDs not strictly ascending and non-zero")]
    IdOrder(&'static str),
    /// A property value tag this version does not define (§7.2).
    #[error("unknown property value tag {0:#04x}")]
    UnknownValueTag(u8),
    /// A None value where the spec does not allow one (§7.2).
    #[error("None value not allowed here")]
    NoneNotAllowed,
    /// Array elements with different tags (§7.2).
    #[error("array elements with different tags")]
    MixedArray,
    /// A property value nested deeper than 4 (§7.2).
    #[error("property value nested deeper than 4")]
    ValueTooDeep,
    /// One top-level property value larger than 8,192 bytes (§7.2).
    #[error("property value larger than 8192 bytes")]
    ValueTooLarge,
    /// A value of the wrong kind for a property ID this version defines (§7.2).
    #[error("property {pid:#06x}: wrong value kind")]
    WrongPropertyKind {
        /// The property ID.
        pid: u16,
    },
    /// A look entry with a value tag a look does not allow (minor ≥ 1, §5.3).
    #[error("look entry: value tag {0:#04x} not allowed")]
    LookTag(u8),
    /// A look string with a byte outside 0x21–0x7E (§5.3).
    #[error("look entry: string byte outside 0x21..=0x7e")]
    LookString,
    /// A look whose encoding exceeds 768 bytes (§5.3).
    #[error("look of {0} bytes exceeds 768")]
    LookTooLarge(usize),
    /// A field this negotiated minor version does not carry (§3.1).
    #[error("{0}: not allowed at the negotiated minor version")]
    NotInMinor(&'static str),
}

impl ProtocolError {
    /// The application error code a receiver closes with: always
    /// [`AppErrorCode::ProtocolViolation`].
    #[must_use]
    pub fn app_code(&self) -> AppErrorCode {
        AppErrorCode::ProtocolViolation
    }
}

/// Shorthand for codec results.
pub type Result<T, E = ProtocolError> = std::result::Result<T, E>;
