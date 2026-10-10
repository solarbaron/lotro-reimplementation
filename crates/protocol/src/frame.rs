// SPDX-License-Identifier: GPL-3.0-or-later
//! Stream framing (§5.4), stream kind bytes (§2.3) and datagram messages (§5.5).

use crate::error::{ProtocolError, Result};
use crate::limits::{
    FRAME_LENGTH_FIELD, MAX_DATAGRAM, MAX_FRAME_BODY, MAX_FRAME_BYTES, MAX_FRAME_LENGTH,
    MIN_FRAME_LENGTH, STREAM_KIND_CHAT, STREAM_KIND_WORLD, TYPE_FIELD,
};
use crate::message::{Channel, Message, MessageType, Side};

/// The kind of a server-initiated unidirectional stream (§2.3 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    /// `W` (0x57): the world stream.
    World,
    /// `C` (0x43): the chat stream.
    Chat,
}

impl StreamKind {
    /// Parses the first byte of a server uni stream. Any other byte is a protocol violation.
    pub fn from_byte(b: u8) -> Result<Self> {
        match b {
            STREAM_KIND_WORLD => Ok(Self::World),
            STREAM_KIND_CHAT => Ok(Self::Chat),
            other => Err(ProtocolError::UnknownStreamKind(other)),
        }
    }

    /// The kind byte.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        match self {
            Self::World => STREAM_KIND_WORLD,
            Self::Chat => STREAM_KIND_CHAT,
        }
    }

    /// The channel the stream carries.
    #[must_use]
    pub fn channel(self) -> Channel {
        match self {
            Self::World => Channel::World,
            Self::Chat => Channel::Chat,
        }
    }
}

/// Resolves a received type code: it must be defined, belong to `channel` and be sendable by
/// `sender` (§2.3 rule 3, §3).
pub fn check_type(code: u16, channel: Channel, sender: Side) -> Result<MessageType> {
    let ty = MessageType::from_code(code).ok_or(ProtocolError::UnknownMessageType(code))?;
    if ty.channel() != channel {
        return Err(ProtocolError::WrongChannel { ty: code, channel });
    }
    if !ty.direction().allows(sender) {
        return Err(ProtocolError::WrongDirection { ty: code, sender });
    }
    Ok(ty)
}

/// Appends one frame (`length`, `type`, body) holding `msg` to `out` in the v1.0 layout
/// (minor 0). `out` is unchanged on error. Datagram messages are refused.
pub fn encode_frame(msg: &Message, out: &mut Vec<u8>) -> Result<()> {
    encode_frame_minor(msg, out, 0)
}

/// [`encode_frame`] in the layout of negotiated minor `minor` (§3.1).
pub fn encode_frame_minor(msg: &Message, out: &mut Vec<u8>, minor: u16) -> Result<()> {
    let ty = msg.message_type();
    if !ty.channel().is_stream() {
        return Err(ProtocolError::WrongChannel {
            ty: ty.code(),
            channel: Channel::Datagram,
        });
    }
    let start = out.len();
    out.extend_from_slice(&[0; FRAME_LENGTH_FIELD]);
    out.extend_from_slice(&ty.code().to_le_bytes());
    if let Err(e) = msg.encode_body_minor(out, minor) {
        out.truncate(start);
        return Err(e);
    }
    let body = out.len() - start - FRAME_LENGTH_FIELD - TYPE_FIELD;
    if body > MAX_FRAME_BODY {
        out.truncate(start);
        return Err(ProtocolError::BodyTooLarge(body));
    }
    // body + 2 <= 65,536 fits a u32.
    let length = u32::try_from(body + TYPE_FIELD).unwrap_or(MAX_FRAME_LENGTH);
    if let Some(slot) = out.get_mut(start..start + FRAME_LENGTH_FIELD) {
        slot.copy_from_slice(&length.to_le_bytes());
    }
    Ok(())
}

/// One frame as a `Vec` (v1.0 layout, minor 0).
pub fn frame_to_vec(msg: &Message) -> Result<Vec<u8>> {
    frame_to_vec_minor(msg, 0)
}

/// One frame as a `Vec` in the layout of negotiated minor `minor` (§3.1).
pub fn frame_to_vec_minor(msg: &Message, minor: u16) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encode_frame_minor(msg, &mut out, minor)?;
    Ok(out)
}

/// Checks a frame's `length` field and returns it (type plus body bytes, 2–65,536).
pub fn parse_frame_length(header: [u8; FRAME_LENGTH_FIELD]) -> Result<usize> {
    let length = u32::from_le_bytes(header);
    if !(MIN_FRAME_LENGTH..=MAX_FRAME_LENGTH).contains(&length) {
        return Err(ProtocolError::FrameLength(length));
    }
    usize::try_from(length).map_err(|_| ProtocolError::FrameLength(length))
}

/// A frame split from a byte buffer, not yet decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawFrame<'a> {
    /// The type code.
    pub ty: u16,
    /// The body bytes.
    pub body: &'a [u8],
}

/// Splits the first frame off `buf`. Returns `Ok(None)` when `buf` holds only part of a
/// frame, or the frame and the number of bytes it used.
pub fn split_frame(buf: &[u8]) -> Result<Option<(RawFrame<'_>, usize)>> {
    let Some(header) = buf.get(..FRAME_LENGTH_FIELD) else {
        return Ok(None);
    };
    let mut h = [0u8; FRAME_LENGTH_FIELD];
    h.copy_from_slice(header);
    let length = parse_frame_length(h)?;
    let total = FRAME_LENGTH_FIELD + length;
    let Some(frame) = buf.get(FRAME_LENGTH_FIELD..total) else {
        return Ok(None);
    };
    let (ty, body) = frame.split_at(TYPE_FIELD);
    let ty = u16::from_le_bytes([ty[0], ty[1]]);
    Ok(Some((RawFrame { ty, body }, total)))
}

/// Decodes a frame's message: the type must be defined, belong to `channel`, and be
/// sendable by `sender`; the body is decoded strictly in the v1.0 layout (minor 0).
pub fn decode_frame_message(
    channel: Channel,
    sender: Side,
    ty: u16,
    body: &[u8],
) -> Result<Message> {
    decode_frame_message_minor(channel, sender, ty, body, 0)
}

/// [`decode_frame_message`] in the layout of negotiated minor `minor` (§3.1).
pub fn decode_frame_message_minor(
    channel: Channel,
    sender: Side,
    ty: u16,
    body: &[u8],
    minor: u16,
) -> Result<Message> {
    if !channel.is_stream() {
        return Err(ProtocolError::WrongChannel { ty, channel });
    }
    let t = check_type(ty, channel, sender)?;
    Message::decode_body_minor(t, body, minor)
}

/// Incremental decoder for one stream: feed it the bytes as they arrive and it yields
/// messages. It never buffers more than one frame (at most 65,540 bytes, §5.4), and after
/// the first error it keeps returning that error.
#[derive(Debug)]
pub struct FrameDecoder {
    channel: Channel,
    sender: Side,
    minor: u16,
    buf: Vec<u8>,
    failed: Option<ProtocolError>,
}

impl FrameDecoder {
    /// A decoder for frames on `channel` sent by `sender`. `channel` must be a stream
    /// channel; for [`Channel::Datagram`] every frame is rejected.
    #[must_use]
    pub fn new(channel: Channel, sender: Side) -> Self {
        Self::with_minor(channel, sender, 0)
    }

    /// A decoder for frames in the layout of negotiated minor `minor` (§3.1).
    #[must_use]
    pub fn with_minor(channel: Channel, sender: Side, minor: u16) -> Self {
        Self {
            channel,
            sender,
            minor,
            buf: Vec::new(),
            failed: None,
        }
    }

    /// Sets the negotiated minor version for the frames that follow (the control stream
    /// learns it from `Hello`/`HelloAck`, which decode the same at every minor).
    pub fn set_minor(&mut self, minor: u16) {
        self.minor = minor;
    }

    /// The minor version frames are decoded under.
    #[must_use]
    pub fn minor(&self) -> u16 {
        self.minor
    }

    /// Bytes buffered for an incomplete frame.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// Checks the end of the stream (FIN): an error when the decoder already failed, and
    /// [`ProtocolError::Truncated`] when part of a frame is still buffered. The decoder is
    /// left unchanged.
    pub fn finish(&self) -> Result<()> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::Truncated)
        }
    }

    /// How many more bytes complete the current frame header or frame. Lets an async reader
    /// read exactly that much.
    #[must_use]
    pub fn wanted(&self) -> usize {
        if self.buf.len() < FRAME_LENGTH_FIELD {
            return FRAME_LENGTH_FIELD - self.buf.len();
        }
        let mut h = [0u8; FRAME_LENGTH_FIELD];
        h.copy_from_slice(&self.buf[..FRAME_LENGTH_FIELD]);
        match parse_frame_length(h) {
            Ok(len) => (FRAME_LENGTH_FIELD + len).saturating_sub(self.buf.len()),
            Err(_) => 0,
        }
    }

    /// Takes bytes from the front of `input` until one frame is complete, and returns its
    /// message. Returns `Ok(None)` when `input` ran out first (it is then empty). Call again
    /// with the rest of `input` until it is empty.
    pub fn push(&mut self, input: &mut &[u8]) -> Result<Option<Message>> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        match self.push_inner(input) {
            Ok(m) => Ok(m),
            Err(e) => {
                self.failed = Some(e.clone());
                self.buf = Vec::new();
                Err(e)
            }
        }
    }

    fn push_inner(&mut self, input: &mut &[u8]) -> Result<Option<Message>> {
        loop {
            let want = self.wanted();
            if want == 0 {
                // Either the header is invalid or the frame is complete.
                if self.buf.len() >= FRAME_LENGTH_FIELD {
                    let mut h = [0u8; FRAME_LENGTH_FIELD];
                    h.copy_from_slice(&self.buf[..FRAME_LENGTH_FIELD]);
                    parse_frame_length(h)?;
                }
                let frame = std::mem::take(&mut self.buf);
                let Some((raw, used)) = split_frame(&frame)? else {
                    return Ok(None);
                };
                debug_assert_eq!(used, frame.len());
                let msg = decode_frame_message_minor(
                    self.channel,
                    self.sender,
                    raw.ty,
                    raw.body,
                    self.minor,
                )?;
                return Ok(Some(msg));
            }
            if input.is_empty() {
                return Ok(None);
            }
            let take = want.min(input.len());
            let (head, tail) = input.split_at(take);
            if self.buf.len() + take > MAX_FRAME_BYTES {
                // Unreachable: wanted() never asks beyond one frame.
                return Err(ProtocolError::FrameLength(u32::MAX));
            }
            self.buf.extend_from_slice(head);
            *input = tail;
            if self.buf.len() == FRAME_LENGTH_FIELD {
                let mut h = [0u8; FRAME_LENGTH_FIELD];
                h.copy_from_slice(&self.buf);
                // Reject a bad length before buffering any body byte.
                parse_frame_length(h)?;
            }
        }
    }
}

/// Encodes a datagram message (`type`, then the body, §5.5). Stream messages and datagrams
/// over 1,200 bytes are refused.
pub fn encode_datagram(msg: &Message) -> Result<Vec<u8>> {
    let ty = msg.message_type();
    if ty.channel() != Channel::Datagram {
        return Err(ProtocolError::WrongChannel {
            ty: ty.code(),
            channel: ty.channel(),
        });
    }
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&ty.code().to_le_bytes());
    msg.encode_body(&mut out)?;
    if out.len() > MAX_DATAGRAM {
        return Err(ProtocolError::DatagramTooLarge(out.len()));
    }
    Ok(out)
}

/// Decodes a received datagram sent by `sender`. Every failure is a protocol violation
/// (§5.5); whether the current state allows the message is the state machine's question.
pub fn decode_datagram(sender: Side, bytes: &[u8]) -> Result<Message> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(ProtocolError::DatagramTooLarge(bytes.len()));
    }
    let Some((ty, body)) = bytes.split_first_chunk::<TYPE_FIELD>() else {
        return Err(ProtocolError::Truncated);
    };
    let code = u16::from_le_bytes(*ty);
    let t = check_type(code, Channel::Datagram, sender)?;
    Message::decode_body(t, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_byte_by_byte() {
        let msg = Message::Ping { nonce: 7 };
        let mut bytes = frame_to_vec(&msg).unwrap();
        bytes.extend(frame_to_vec(&Message::Pong { nonce: 8 }).unwrap());
        let mut d = FrameDecoder::new(Channel::Control, Side::Client);
        let mut got = Vec::new();
        for b in &bytes {
            let mut input: &[u8] = std::slice::from_ref(b);
            while let Some(m) = d.push(&mut input).unwrap() {
                got.push(m);
            }
            assert!(input.is_empty());
        }
        assert_eq!(got, vec![msg, Message::Pong { nonce: 8 }]);
        assert_eq!(d.buffered(), 0);
        assert_eq!(d.finish(), Ok(()));
    }

    #[test]
    fn finish_reports_a_truncated_frame() {
        let bytes = frame_to_vec(&Message::Ping { nonce: 7 }).unwrap();
        let mut d = FrameDecoder::new(Channel::Control, Side::Client);
        let mut input: &[u8] = &bytes[..bytes.len() - 1];
        assert_eq!(d.push(&mut input), Ok(None));
        assert_eq!(d.finish(), Err(ProtocolError::Truncated));
        // A failed decoder reports its error at the end too.
        let mut d = FrameDecoder::new(Channel::Control, Side::Client);
        let mut bad: &[u8] = &[0, 0, 0, 0];
        let err = d.push(&mut bad).unwrap_err();
        assert_eq!(d.finish(), Err(err));
    }

    #[test]
    fn bad_length_rejected_before_body() {
        let mut d = FrameDecoder::new(Channel::Control, Side::Client);
        let mut input: &[u8] = &[0x01, 0x00, 0x01, 0x00, 0xAA];
        assert_eq!(d.push(&mut input), Err(ProtocolError::FrameLength(65_537)));
        assert_eq!(d.buffered(), 0);
        let mut more: &[u8] = &[0];
        assert!(d.push(&mut more).is_err());
    }

    #[test]
    fn direction_and_channel() {
        let bytes = frame_to_vec(&Message::Ping { nonce: 1 }).unwrap();
        let mut d = FrameDecoder::new(Channel::World, Side::Server);
        let mut input: &[u8] = &bytes;
        assert!(matches!(
            d.push(&mut input),
            Err(ProtocolError::WrongChannel { .. })
        ));
        let bytes = frame_to_vec(&Message::InterestReset).unwrap();
        let mut d = FrameDecoder::new(Channel::World, Side::Client);
        let mut input: &[u8] = &bytes;
        assert!(matches!(
            d.push(&mut input),
            Err(ProtocolError::WrongDirection { .. })
        ));
        assert!(encode_datagram(&Message::Ping { nonce: 1 }).is_err());
        assert!(frame_to_vec(&Message::TimeRequest { client_time: 1 }).is_err());
        assert_eq!(
            StreamKind::from_byte(0x58),
            Err(ProtocolError::UnknownStreamKind(0x58))
        );
    }
}
