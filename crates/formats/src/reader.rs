// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded cursor over a payload and the shared primitive encodings.
//!
//! Spec: `docs/specs/records.md` §1.

use std::fmt;

use crate::did::Did;
use crate::error::{FormatError, Result};

/// Largest initial capacity [`Reader::ensure_elements`] hands out for a count read from data.
pub const PREALLOC_CAP: usize = 4096;

/// Heap bytes a decoder may retain per byte of input: every [`Reader`] starts with a memory
/// budget of `MAX_DECODE_AMPLIFICATION` × input length + [`DECODE_BUDGET_SLACK`].
///
/// Decoders charge the decoded data they keep (collection storage, strings, boxed values,
/// lookup indexes) with [`Reader::charge`] before allocating it, and fail with
/// [`FormatError::MemoryLimit`] once the budget is spent. Real records stay far below the
/// limit; only hostile inputs made of many tiny elements reach it.
pub const MAX_DECODE_AMPLIFICATION: usize = 16;

/// Fixed part of every [`Reader`]'s memory budget, so that tiny payloads can still hold a
/// few boxed values and lookup tables.
pub const DECODE_BUDGET_SLACK: usize = 64 * 1024;

/// Bucket counts indexed by the TSize hint byte (spec §1.3). Every value is prime.
pub const TSIZE_BUCKET_COUNTS: [u32; 13] = [
    11, 23, 47, 89, 191, 383, 761, 1531, 3067, 6143, 12281, 24571, 49139,
];

/// Bucket count for a TSize hint, or `None` for hints the specification has not observed (≥ 13).
#[must_use]
pub fn tsize_bucket_count(hint: u8) -> Option<u32> {
    TSIZE_BUCKET_COUNTS.get(usize::from(hint)).copied()
}

/// Checks the element order of a TSize collection (spec §1.3.1): keys taken modulo the hint's
/// bucket count must be non-decreasing.
///
/// Returns `None` when the hint has no known bucket count (≥ 13), since no order can be
/// checked then.
#[must_use]
pub fn keys_in_bucket_order(hint: u8, keys: impl IntoIterator<Item = u32>) -> Option<bool> {
    let buckets = tsize_bucket_count(hint)?;
    let mut last = 0u32;
    for key in keys {
        let bucket = key % buckets;
        if bucket < last {
            return Some(false);
        }
        last = bucket;
    }
    Some(true)
}

/// Header of a serialized hash collection (spec §1.3): a bucket-count hint and an element count.
///
/// The hint is kept so that a byte-exact writer can reproduce the original layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
///
/// The result's capacity is exactly its UTF-8 length.
#[must_use]
pub fn latin1_to_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(latin1_utf8_len(bytes));
    s.extend(bytes.iter().map(|&b| char::from(b)));
    s
}

/// UTF-8 length of Latin-1 bytes: bytes 0x80 and above take two bytes.
fn latin1_utf8_len(bytes: &[u8]) -> usize {
    bytes.len() + bytes.iter().filter(|&&b| b >= 0x80).count()
}

/// A collection index as stored in lookup tables. Counts come from `u32` fields, so every
/// index fits; anything larger saturates (and then misses on lookup).
pub(crate) fn index_u32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Approximate heap bytes of a `HashMap<K, V>` created with capacity `len`: the bucket
/// array (a power of two at 7/8 load), one control byte per bucket and one group of padding.
pub(crate) fn hash_index_bytes<K, V>(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let buckets = if len < 4 {
        4
    } else if len < 8 {
        8
    } else {
        len.saturating_mul(8).div_ceil(7).next_power_of_two()
    };
    buckets
        .saturating_mul(std::mem::size_of::<(K, V)>() + 1)
        .saturating_add(16)
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
///
/// A reader also carries a memory budget (see [`MAX_DECODE_AMPLIFICATION`]). The string
/// readers charge what they allocate; decoders charge the rest with [`Reader::charge`].
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Heap bytes the decoder may still retain.
    budget: usize,
    /// The budget this reader started with.
    budget_limit: usize,
    /// TSize headers read so far whose hint has no known bucket count.
    unknown_hints: u32,
}

impl<'a> Reader<'a> {
    /// Starts reading at offset 0, with a memory budget of [`MAX_DECODE_AMPLIFICATION`] ×
    /// `data.len()` + [`DECODE_BUDGET_SLACK`] bytes.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        let budget = data
            .len()
            .saturating_mul(MAX_DECODE_AMPLIFICATION)
            .saturating_add(DECODE_BUDGET_SLACK);
        Self {
            data,
            pos: 0,
            budget,
            budget_limit: budget,
            unknown_hints: 0,
        }
    }

    /// Heap bytes the decoder may still retain before [`Reader::charge`] fails.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Records that the decoder is about to retain `bytes` more heap bytes.
    ///
    /// Fails with [`FormatError::MemoryLimit`] (and charges nothing) if that would exceed
    /// the reader's budget. Call it before allocating, so a rejected record never allocates
    /// the memory it asked for.
    pub fn charge(&mut self, bytes: usize) -> Result<()> {
        match self.budget.checked_sub(bytes) {
            Some(left) => {
                self.budget = left;
                Ok(())
            }
            None => Err(FormatError::MemoryLimit {
                offset: self.pos,
                limit: self.budget_limit,
            }),
        }
    }

    /// Number of TSize headers read so far whose hint has no known bucket count (T1).
    ///
    /// [`Reader::tsize`] accepts such hints silently; decoders log them once per record,
    /// so hostile input cannot flood the log.
    #[must_use]
    pub fn unknown_tsize_hints(&self) -> u32 {
        self.unknown_hints
    }

    /// Emits one debug event if any TSize hint outside the observed range was read.
    pub(crate) fn log_unknown_hints(&self, what: &'static str) {
        if self.unknown_hints != 0 {
            tracing::debug!(
                what,
                count = self.unknown_hints,
                "TSize hints outside observed range"
            );
        }
    }

    /// Checks `count` like [`Reader::ensure_elements`] and returns an empty vector whose
    /// initial capacity (at most `cap_limit` and [`PREALLOC_CAP`]) has been charged.
    pub(crate) fn alloc_vec<T>(
        &mut self,
        count: u32,
        min_size: usize,
        cap_limit: usize,
    ) -> Result<Vec<T>> {
        let cap = self.ensure_elements(count, min_size)?.min(cap_limit);
        self.charge(cap.saturating_mul(std::mem::size_of::<T>()))?;
        let mut v = Vec::new();
        v.try_reserve_exact(cap).map_err(|_| self.memory_limit())?;
        Ok(v)
    }

    /// Appends `item` to a collection of `count` elements, growing it in charged steps that
    /// never take its capacity past `count`.
    pub(crate) fn push<T>(&mut self, v: &mut Vec<T>, item: T, count: u32) -> Result<()> {
        if v.len() == v.capacity() {
            let count = usize::try_from(count).unwrap_or(usize::MAX);
            let additional = count.saturating_sub(v.len()).min(v.len().max(4)).max(1);
            self.charge(additional.saturating_mul(std::mem::size_of::<T>()))?;
            v.try_reserve_exact(additional)
                .map_err(|_| self.memory_limit())?;
        }
        v.push(item);
        Ok(())
    }

    fn memory_limit(&self) -> FormatError {
        FormatError::MemoryLimit {
            offset: self.pos,
            limit: self.budget_limit,
        }
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

    /// Reads a TSize header (spec §1.3). Unknown hints are accepted and counted (T1); see
    /// [`Reader::unknown_tsize_hints`].
    pub fn tsize(&mut self) -> Result<TSize> {
        let hint = self.u8()?;
        let count = self.vle()?;
        if tsize_bucket_count(hint).is_none() {
            self.unknown_hints = self.unknown_hints.saturating_add(1);
        }
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

    /// Reads a Pascal string and decodes it as Latin-1. The string's heap bytes are
    /// charged to the budget.
    pub fn pascal_string(&mut self) -> Result<String> {
        let start = self.pos;
        let raw = self.pascal_bytes()?;
        self.charge_latin1(raw, start)
    }

    /// Reads a UTF-16 string (VLE count of code units, then UTF-16LE). The string's heap
    /// bytes are charged to the budget.
    pub fn utf16_string(&mut self) -> Result<Utf16String> {
        let start = self.pos;
        let count = self.vle()?;
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        let raw = self
            .bytes(count.saturating_mul(2))
            .inspect_err(|_| self.pos = start)?;
        self.charge(raw.len()).inspect_err(|_| self.pos = start)?;
        Ok(Utf16String(
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        ))
    }

    /// Reads a short ASCII string (`u8` length + bytes), decoding any high bytes as Latin-1.
    /// The string's heap bytes are charged to the budget.
    pub fn short_string(&mut self) -> Result<String> {
        let start = self.pos;
        let len = self.u8()?;
        let raw = self
            .bytes(usize::from(len))
            .inspect_err(|_| self.pos = start)?;
        self.charge_latin1(raw, start)
    }

    fn charge_latin1(&mut self, raw: &[u8], start: usize) -> Result<String> {
        self.charge(latin1_utf8_len(raw))
            .inspect_err(|_| self.pos = start)?;
        Ok(latin1_to_string(raw))
    }
}
