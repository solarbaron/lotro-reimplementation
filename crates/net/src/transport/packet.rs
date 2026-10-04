// SPDX-License-Identifier: GPL-3.0-or-later
//! Wire codec for Tessera's own reliable-datagram framing (TRD v1).
//!
//! This is **our** design; see `docs/specs/transport-design.md`. All integers
//! are little-endian.
//!
//! ```text
//! offset  size  field
//! ------  ----  -----------------------------------------------
//!      0     2  magic  = b"TS"
//!      2     1  version = 1
//!      3     1  flags  (bit0 RELIABLE, bit1 UNRELIABLE, bit2 CLOSE,
//!                       bit3 PAUSED; others 0)
//!      4     4  conn_id
//!      8     4  ack_next   (receiver: next in-order seq it expects)
//!     12     4  ack_bits   (bit i set => seq ack_next+1+i was received)
//!     16     .. body, depending on flags:
//!               RELIABLE:   seq u32, msg_id u32, frag_index u16,
//!                           frag_count u16, len u16, payload[len]
//!               UNRELIABLE: len u16, payload[len]
//!               neither:    empty (ack-only / keepalive / close)
//!    end-4     4  CRC-32 (IEEE, reflected, poly 0xEDB88320) of all
//!                 preceding bytes
//! ```

use crate::NetError;

/// Packet magic.
pub const MAGIC: [u8; 2] = *b"TS";
/// Framing version this code speaks.
pub const VERSION: u8 = 1;
/// Fixed header length.
pub const HEADER_LEN: usize = 16;
/// Reliable segment sub-header length.
pub const SEGMENT_HEADER_LEN: usize = 14;
/// Unreliable sub-header length.
pub const UNRELIABLE_HEADER_LEN: usize = 2;
/// Trailer (checksum) length.
pub const TRAILER_LEN: usize = 4;
/// Smallest datagram size the transport can be configured with.
pub const MIN_DATAGRAM_SIZE: usize = 64;
/// Largest datagram size the transport can be configured with (the IPv4 UDP
/// payload limit).
pub const MAX_DATAGRAM_SIZE: usize = 65_507;

const FLAG_RELIABLE: u8 = 0x01;
const FLAG_UNRELIABLE: u8 = 0x02;
const FLAG_CLOSE: u8 = 0x04;
const FLAG_PAUSED: u8 = 0x08;
const FLAGS_KNOWN: u8 = FLAG_RELIABLE | FLAG_UNRELIABLE | FLAG_CLOSE | FLAG_PAUSED;

/// Acknowledgement state piggy-backed on every packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ack {
    /// Every sequence number before this one has been received.
    pub next: u32,
    /// Bit `i` set means `next + 1 + i` has been received (selective ack).
    pub bits: u32,
    /// The sender of this packet is back-pressured: its application is not
    /// draining received messages, so it has stopped consuming segments at
    /// `next`. The peer must not treat the missing acknowledgement as loss.
    pub paused: bool,
}

/// One fragment of a reliable message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    /// Per-direction segment sequence number.
    pub seq: u32,
    /// Message this fragment belongs to.
    pub msg_id: u32,
    /// Index of this fragment, `0..frag_count`.
    pub frag_index: u16,
    /// Number of fragments in the message, `>= 1`.
    pub frag_count: u16,
    /// Fragment bytes.
    pub payload: &'a [u8],
}

/// What a packet carries besides its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body<'a> {
    /// Nothing: an ack-only, keepalive, or close packet.
    Empty,
    /// A reliable segment.
    Reliable(Segment<'a>),
    /// An unreliable, unfragmented message.
    Unreliable(&'a [u8]),
}

/// A decoded (borrowing) packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet<'a> {
    /// Connection identifier both peers agreed on.
    pub conn_id: u32,
    /// Piggy-backed acknowledgement.
    pub ack: Ack,
    /// The sender is closing the connection.
    pub close: bool,
    /// Payload.
    pub body: Body<'a>,
}

impl Packet<'_> {
    /// Encoded size in bytes.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + TRAILER_LEN
            + match self.body {
                Body::Empty => 0,
                Body::Reliable(s) => SEGMENT_HEADER_LEN + s.payload.len(),
                Body::Unreliable(p) => UNRELIABLE_HEADER_LEN + p.len(),
            }
    }

    /// Appends the encoded packet to `out`.
    ///
    /// # Errors
    ///
    /// [`NetError::MessageTooLarge`] if a payload exceeds `u16::MAX` bytes.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), NetError> {
        let start = out.len();
        let mut flags = 0;
        if self.close {
            flags |= FLAG_CLOSE;
        }
        if self.ack.paused {
            flags |= FLAG_PAUSED;
        }
        match self.body {
            Body::Empty => {}
            Body::Reliable(_) => flags |= FLAG_RELIABLE,
            Body::Unreliable(_) => flags |= FLAG_UNRELIABLE,
        }
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.push(flags);
        out.extend_from_slice(&self.conn_id.to_le_bytes());
        out.extend_from_slice(&self.ack.next.to_le_bytes());
        out.extend_from_slice(&self.ack.bits.to_le_bytes());
        match self.body {
            Body::Empty => {}
            Body::Reliable(s) => {
                let len = payload_len(s.payload)?;
                out.extend_from_slice(&s.seq.to_le_bytes());
                out.extend_from_slice(&s.msg_id.to_le_bytes());
                out.extend_from_slice(&s.frag_index.to_le_bytes());
                out.extend_from_slice(&s.frag_count.to_le_bytes());
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(s.payload);
            }
            Body::Unreliable(p) => {
                let len = payload_len(p)?;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(p);
            }
        }
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_le_bytes());
        Ok(())
    }

    /// Decodes and validates one datagram. Never panics; rejects anything
    /// not exactly matching the layout (including trailing bytes).
    ///
    /// # Errors
    ///
    /// [`NetError::Malformed`] describing the first problem found.
    pub fn decode(buf: &[u8]) -> Result<Packet<'_>, NetError> {
        if buf.len() < HEADER_LEN + TRAILER_LEN {
            return Err(NetError::Malformed("shorter than header"));
        }
        let (content, trailer) = buf.split_at(buf.len() - TRAILER_LEN);
        if crc32(content) != u32::from_le_bytes(arr(trailer)?) {
            return Err(NetError::Malformed("checksum mismatch"));
        }
        let mut r = Reader { buf: content };
        if r.take(2)? != MAGIC {
            return Err(NetError::Malformed("bad magic"));
        }
        if r.u8()? != VERSION {
            return Err(NetError::Malformed("unsupported version"));
        }
        let flags = r.u8()?;
        if flags & !FLAGS_KNOWN != 0 {
            return Err(NetError::Malformed("reserved flag bits set"));
        }
        let conn_id = r.u32()?;
        let ack = Ack {
            next: r.u32()?,
            bits: r.u32()?,
            paused: flags & FLAG_PAUSED != 0,
        };
        let body = match (flags & FLAG_RELIABLE != 0, flags & FLAG_UNRELIABLE != 0) {
            (true, true) => return Err(NetError::Malformed("both body flags set")),
            (false, false) => Body::Empty,
            (true, false) => {
                let seq = r.u32()?;
                let msg_id = r.u32()?;
                let frag_index = r.u16()?;
                let frag_count = r.u16()?;
                let len = usize::from(r.u16()?);
                let payload = r.take(len)?;
                if frag_count == 0 || frag_index >= frag_count {
                    return Err(NetError::Malformed("fragment index out of range"));
                }
                Body::Reliable(Segment {
                    seq,
                    msg_id,
                    frag_index,
                    frag_count,
                    payload,
                })
            }
            (false, true) => {
                let len = usize::from(r.u16()?);
                Body::Unreliable(r.take(len)?)
            }
        };
        if !r.buf.is_empty() {
            return Err(NetError::Malformed("trailing bytes"));
        }
        Ok(Packet {
            conn_id,
            ack,
            close: flags & FLAG_CLOSE != 0,
            body,
        })
    }
}

fn payload_len(p: &[u8]) -> Result<u16, NetError> {
    u16::try_from(p.len()).map_err(|_| NetError::MessageTooLarge {
        len: p.len(),
        max: usize::from(u16::MAX),
    })
}

fn arr<const N: usize>(s: &[u8]) -> Result<[u8; N], NetError> {
    s.try_into().map_err(|_| NetError::Malformed("truncated"))
}

struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], NetError> {
        if self.buf.len() < n {
            return Err(NetError::Malformed("truncated"));
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, NetError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, NetError> {
        Ok(u16::from_le_bytes(arr(self.take(2)?)?))
    }
    fn u32(&mut self) -> Result<u32, NetError> {
        Ok(u32::from_le_bytes(arr(self.take(4)?)?))
    }
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        #[allow(clippy::cast_possible_truncation)]
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// CRC-32 (IEEE 802.3, reflected, init and xor-out `0xFFFF_FFFF`).
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn crc_check_value() {
        // The standard CRC-32 check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    fn roundtrip(p: Packet<'_>) {
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), p.encoded_len());
        assert_eq!(Packet::decode(&buf).unwrap(), p);
    }

    #[test]
    fn roundtrips() {
        let ack = Ack {
            next: 7,
            bits: 0b101,
            paused: false,
        };
        roundtrip(Packet {
            conn_id: 1,
            ack,
            close: false,
            body: Body::Empty,
        });
        roundtrip(Packet {
            conn_id: 1,
            ack,
            close: true,
            body: Body::Empty,
        });
        roundtrip(Packet {
            conn_id: 1,
            ack: Ack {
                paused: true,
                ..ack
            },
            close: false,
            body: Body::Empty,
        });
        roundtrip(Packet {
            conn_id: 9,
            ack,
            close: false,
            body: Body::Unreliable(b"hi"),
        });
        roundtrip(Packet {
            conn_id: u32::MAX,
            ack,
            close: false,
            body: Body::Reliable(Segment {
                seq: 3,
                msg_id: 4,
                frag_index: 1,
                frag_count: 2,
                payload: b"payload",
            }),
        });
    }

    #[test]
    fn rejects_corruption_and_trailing_bytes() {
        let mut buf = Vec::new();
        Packet {
            conn_id: 1,
            ack: Ack::default(),
            close: false,
            body: Body::Unreliable(b"x"),
        }
        .encode(&mut buf)
        .unwrap();
        for i in 0..buf.len() {
            let mut bad = buf.clone();
            bad[i] ^= 0x10;
            assert!(Packet::decode(&bad).is_err(), "flip at {i} accepted");
        }
        let mut longer = buf.clone();
        longer.push(0);
        assert!(Packet::decode(&longer).is_err());
        assert!(Packet::decode(&buf[..buf.len() - 1]).is_err());
    }

    /// Re-seal a hand-built packet with a valid checksum.
    fn seal(mut content: Vec<u8>) -> Vec<u8> {
        let crc = crc32(&content);
        content.extend_from_slice(&crc.to_le_bytes());
        content
    }

    #[test]
    fn rejects_bad_fields_with_valid_checksum() {
        let mut base = Vec::new();
        base.extend_from_slice(b"TS");
        base.extend_from_slice(&[VERSION, FLAG_RELIABLE]);
        base.extend_from_slice(&[0; 12]);
        // seq, msg_id, frag_index=2, frag_count=2, len=0
        base.extend_from_slice(&[0; 8]);
        base.extend_from_slice(&2u16.to_le_bytes());
        base.extend_from_slice(&2u16.to_le_bytes());
        base.extend_from_slice(&0u16.to_le_bytes());
        assert!(
            Packet::decode(&seal(base.clone())).is_err(),
            "index >= count"
        );

        let mut both = base.clone();
        both[3] = FLAG_RELIABLE | FLAG_UNRELIABLE;
        assert!(Packet::decode(&seal(both)).is_err());

        let mut reserved = base.clone();
        reserved[3] = FLAG_RELIABLE | 0x10;
        assert!(Packet::decode(&seal(reserved)).is_err());

        let mut long_len = base;
        long_len[28] = 0xFF; // len larger than the remaining bytes
        assert!(Packet::decode(&seal(long_len)).is_err());
    }

    proptest! {
        #[test]
        fn decode_never_panics(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let _ = Packet::decode(&data);
        }

        #[test]
        fn decode_never_panics_with_valid_checksum(
            data in proptest::collection::vec(any::<u8>(), 0..2048)
        ) {
            let mut sealed = b"TS\x01".to_vec();
            sealed.extend_from_slice(&data);
            let _ = Packet::decode(&seal(sealed));
        }
    }
}
