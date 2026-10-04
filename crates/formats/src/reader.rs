// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded cursor over a payload and the shared primitive encodings.
//!
//! Spec: `docs/specs/records.md` §1.

use std::fmt;

use crate::did::Did;
use crate::error::{FormatError, Result};

/// Largest initial capacity [`Reader::ensure_elements`] hands out for a count read from data.
pub const PREALLOC_CAP: usize = 4096;

/// Bucket counts indexed by the TSize hint byte (spec §1.3). Every value is prime.
pub const TSIZE_BUCKET_COUNTS: [u32; 13] = [
    11, 23, 47, 89, 191, 383, 761, 1531, 3067, 6143, 12281, 24571, 49139,
];

/// Bucket count for a TSize hint, or `None` for hints the specification has not observed (≥ 13).
#[must_use]
pub fn tsize_bucket_count(hint: u8) -> Option<u32> {
    TSIZE_BUCKET_COUNTS.get(usize::from(hint)).copied()
}

/// Header of a serialized hash collection (spec §1.3): a bucket-count hint and an element count.
///
/// The hint is kept so that a byte-exact writer can reproduce the original layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TSize {
    /// Bucket-count index; see [`TSIZE_BUCKET_COUNTS`].
    pub hint: u8,
    /// Number of elements that follow.
    pub count: u32,
}

impl TSize {
    /// Bucket count this hint selects, if known.
    #[must_use]
    pub fn bucket_count(self) -> Option<u32> {
        tsize_bucket_count(self.hint)
    }
}

/// A UTF-16 string kept as raw code units, so unpaired surrogates survive (spec §1.4).
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Utf16String(pub Vec<u16>);

impl Utf16String {
    /// Raw UTF-16 code units.
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.0
    }

    /// Number of code units.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the string has no code units.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Converts to a Rust string, replacing unpaired surrogates with U+FFFD.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }
}

impl From<&str> for Utf16String {
    fn from(s: &str) -> Self {
        Self(s.encode_utf16().collect())
    }
}

impl fmt::Display for Utf16String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in char::decode_utf16(self.0.iter().copied()) {
            fmt::Write::write_char(f, c.unwrap_or(char::REPLACEMENT_CHARACTER))?;
        }
        Ok(())
    }
}

impl fmt::Debug for Utf16String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.to_string_lossy(), f)
    }
}

/// Decodes ISO-8859-1 bytes: every byte maps to the code point of the same value.
#[must_use]
pub fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Appends `value` as a VLE (spec §1.2) in the shortest form.
pub fn write_vle(out: &mut Vec<u8>, value: u32) {
    if value < 0x80 {
        out.push(value as u8);
    } else if value < 0x4000 {
        out.push(0x80 | (value >> 8) as u8);
        out.push(value as u8);
    } else if value < 0x4000_0000 && (value >> 24) != 0x20 {
        out.push(0xC0 | (value >> 24) as u8);
        out.push((value >> 16) as u8);
        out.extend_from_slice(&(value as u16).to_le_bytes());
    } else {
        out.push(0xE0);
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Bounded little-endian cursor over a payload.
///
/// Every read checks the remaining length first and never panics. Callers that allocate
/// for a count read from the data should first call [`Reader::ensure_elements`] so that a
/// hostile count cannot cause an allocation larger than the input justifies.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Starts reading at offset 0.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current offset from the start of the payload.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Bytes left to read.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Whether every byte has been consumed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// The unread tail of the payload.
    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or(&[])
    }

    /// Fails with [`FormatError::TrailingBytes`] unless the payload has been consumed exactly.
    pub fn expect_end(&self, what: &'static str) -> Result<()> {
        match self.remaining() {
            0 => Ok(()),
            count => Err(FormatError::TrailingBytes {
                what,
                offset: self.pos,
                count,
            }),
        }
    }

    fn truncated(&self, needed: usize) -> FormatError {
        FormatError::Truncated {
            offset: self.pos,
            needed,
            available: self.remaining(),
        }
    }

    /// Reads `n` bytes as a borrowed slice.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| self.truncated(n))?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| self.truncated(n))?;
        self.pos = end;
        Ok(slice)
    }

    /// Looks at the next `n` bytes without consuming them.
    pub fn peek(&self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| self.truncated(n))?;
        self.data
            .get(self.pos..end)
            .ok_or_else(|| self.truncated(n))
    }

    /// Skips `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    /// Reads a fixed-size array.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    /// Reads a `u8`.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    /// Reads a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16> {
        self.array().map(u16::from_le_bytes)
    }

    /// Reads a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32> {
        self.array().map(u32::from_le_bytes)
    }

    /// Reads a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64> {
        self.array().map(u64::from_le_bytes)
    }

    /// Reads a little-endian `i32`.
    pub fn i32(&mut self) -> Result<i32> {
        self.array().map(i32::from_le_bytes)
    }

    /// Reads a little-endian `i64`.
    pub fn i64(&mut self) -> Result<i64> {
        self.array().map(i64::from_le_bytes)
    }

    /// Reads a little-endian IEEE-754 `f32`.
    pub fn f32(&mut self) -> Result<f32> {
        self.array().map(f32::from_le_bytes)
    }

    /// Reads a little-endian IEEE-754 `f64`.
    pub fn f64(&mut self) -> Result<f64> {
        self.array().map(f64::from_le_bytes)
    }

    /// Reads a `bool8`; values other than 0 and 1 are rejected (spec §1.1).
    pub fn bool8(&mut self) -> Result<bool> {
        let offset = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(FormatError::InvalidBool { offset, value }),
        }
    }

    /// Reads a DID (`u32`).
    pub fn did(&mut self) -> Result<Did> {
        self.u32().map(Did)
    }

    /// Reads a VLE (spec §1.2). Accepts every form, including the `0xE0` escape and
    /// non-shortest encodings. Truncation mid-value is an error (invariant V2).
    pub fn vle(&mut self) -> Result<u32> {
        let start = self.pos;
        let a = self.u8()?;
        let value = match a {
            0x00..=0x7F => u32::from(a),
            0x80..=0xBF => {
                let b = self.u8().inspect_err(|_| self.pos = start)?;
                (u32::from(a & 0x3F) << 8) | u32::from(b)
            }
            0xE0 => self.u32().inspect_err(|_| self.pos = start)?,
            _ => {
                let rest = self.array::<3>().inspect_err(|_| self.pos = start)?;
                (u32::from(a & 0x3F) << 24)
                    | (u32::from(rest[0]) << 16)
                    | u32::from(u16::from_le_bytes([rest[1], rest[2]]))
            }
        };
        Ok(value)
    }

    /// Reads a TSize header (spec §1.3). Unknown hints are accepted with a debug log (T1).
    pub fn tsize(&mut self) -> Result<TSize> {
        let hint = self.u8()?;
        if tsize_bucket_count(hint).is_none() {
            tracing::debug!(
                hint,
                offset = self.pos - 1,
                "TSize hint outside observed range"
            );
        }
        let count = self.vle()?;
        Ok(TSize { hint, count })
    }

    /// Checks that `count` elements of at least `min_size` bytes each could still fit in the
    /// remaining input, and returns a safe initial capacity for a collection of them.
    ///
    /// The capacity is clamped to [`PREALLOC_CAP`]: an in-memory element is usually much
    /// larger than its smallest encoding, so reserving the full count up front would let one
    /// hostile header reserve many times the input size before any element is checked.
    /// Collections grow as elements actually decode. Callers must loop over `count`, not
    /// over the returned capacity.
    pub fn ensure_elements(&self, count: u32, min_size: usize) -> Result<usize> {
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        let needed = count.saturating_mul(min_size.max(1));
        if needed > self.remaining() {
            return Err(self.truncated(needed));
        }
        Ok(count.min(PREALLOC_CAP))
    }

    /// Reads a Pascal string (VLE byte length + Latin-1 bytes) as raw bytes.
    pub fn pascal_bytes(&mut self) -> Result<&'a [u8]> {
        let start = self.pos;
        let len = self.vle()?;
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        self.bytes(len).inspect_err(|_| self.pos = start)
    }

    /// Reads a Pascal string and decodes it as Latin-1.
    pub fn pascal_string(&mut self) -> Result<String> {
        self.pascal_bytes().map(latin1_to_string)
    }

    /// Reads a UTF-16 string (VLE count of code units, then UTF-16LE).
    pub fn utf16_string(&mut self) -> Result<Utf16String> {
        let start = self.pos;
        let count = self.vle()?;
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        let raw = self
            .bytes(count.saturating_mul(2))
            .inspect_err(|_| self.pos = start)?;
        Ok(Utf16String(
            raw.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        ))
    }

    /// Reads a short ASCII string (`u8` length + bytes), decoding any high bytes as Latin-1.
    pub fn short_string(&mut self) -> Result<String> {
        let start = self.pos;
        let len = self.u8()?;
        let raw = self
            .bytes(usize::from(len))
            .inspect_err(|_| self.pos = start)?;
        Ok(latin1_to_string(raw))
    }
}
