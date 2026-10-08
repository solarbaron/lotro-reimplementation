// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the bytecode chunk (type 1) and the message chunk (type 2) in a class-library
//! record (`wlib.md` §3, §4.1, §4.2), and decoding the messages (`wlib-bytecode.md` §8).
//!
//! The class table and the other chunks are decoded by `tessera_wstate::ClassLibrary`;
//! this module only walks the chunk stream again to get at the two chunks that decoder
//! skips.

use tessera_formats::{FormatError, Reader};

use crate::error::{Result, WvmError};

/// Chunk marker (`wlib.md` §3.2).
const CHUNK_MARKER: u32 = 0xFEDC_1234;
/// Chunk type of the bytecode (`wlib.md` §4.1).
pub const CHUNK_BYTECODE: u32 = 1;
/// Chunk type of the messages (`wlib.md` §4.2).
pub const CHUNK_MESSAGES: u32 = 2;

fn fmt_err(e: FormatError, at: usize) -> WvmError {
    WvmError::container(at, e.to_string())
}

/// The two chunks of a class-library record that hold code and messages, as borrowed
/// payloads (without the chunk header).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodeChunks<'a> {
    /// Payload of chunk type 1, if present.
    pub bytecode: Option<&'a [u8]>,
    /// Payload of chunk type 2, if present.
    pub messages: Option<&'a [u8]>,
}

impl<'a> CodeChunks<'a> {
    /// Walks the header and chunk stream of a class-library record (`wlib.md` §3.1, §3.2)
    /// and picks out chunks 1 and 2. Other chunks are skipped by their size; a duplicate
    /// chunk 1 or 2 is malformed.
    pub fn find(record: &'a [u8]) -> Result<Self> {
        let mut r = Reader::new(record);
        let head = |r: &mut Reader<'a>| -> Result<u32> {
            let at = r.position();
            r.u32().map_err(|e| fmt_err(e, at))
        };
        let _did = head(&mut r)?;
        let stop_code = head(&mut r)?;
        let version = head(&mut r)?;
        let extra = match version {
            30 => 0,
            36 => 16,
            38 => 4,
            v => {
                return Err(WvmError::container(8, format!("unsupported version {v}")));
            }
        };
        let at = r.position();
        r.skip(extra).map_err(|e| fmt_err(e, at))?;
        let mut out = Self::default();
        loop {
            let at = r.position();
            let m = head(&mut r)?;
            if m == stop_code {
                break;
            }
            if m != CHUNK_MARKER {
                return Err(WvmError::container(
                    at,
                    format!("marker {m:#010x}, expected a chunk or the stop code"),
                ));
            }
            let ty = head(&mut r)?;
            let size = head(&mut r)?;
            let p = r.position();
            let payload = r
                .bytes(usize::try_from(size).unwrap_or(usize::MAX))
                .map_err(|e| fmt_err(e, p))?;
            let slot = match ty {
                CHUNK_BYTECODE => &mut out.bytecode,
                CHUNK_MESSAGES => &mut out.messages,
                _ => continue,
            };
            if slot.is_some() {
                return Err(WvmError::container(
                    at,
                    format!("chunk type {ty} occurs twice"),
                ));
            }
            *slot = Some(payload);
        }
        Ok(out)
    }
}

/// Splits a bytecode chunk payload into the code block: u32 *L*, then *L* bytes, nothing
/// after (`wlib.md` §4.1).
pub fn code_block(payload: &[u8]) -> Result<&[u8]> {
    let mut r = Reader::new(payload);
    let len = r.u32().map_err(|e| fmt_err(e, 0))?;
    let code = r
        .bytes(usize::try_from(len).unwrap_or(usize::MAX))
        .map_err(|e| fmt_err(e, 4))?;
    if !r.is_empty() {
        return Err(WvmError::container(
            r.position(),
            format!("{} bytes after the code block", r.remaining()),
        ));
    }
    Ok(code)
}

/// The message list of chunk type 2 (`wlib.md` §4.2, `wlib-bytecode.md` §8): UTF-16 texts
/// kept as raw code units (unpaired surrogates are kept, `records.md` §1.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Messages {
    /// All code units, concatenated.
    units: Vec<u16>,
    /// (first unit, unit count) per message.
    spans: Vec<(u32, u32)>,
    /// Number of arrays after the messages (meaning OPEN, `wlib-bytecode.md` §15).
    pub array_count: u32,
    /// Total number of u32 values in those arrays.
    pub array_values: u64,
}

impl Messages {
    /// Decodes a message chunk payload. The payload must be consumed exactly.
    pub fn parse(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        let pos = |r: &Reader<'_>| r.position();
        let at = pos(&r);
        let count = r.u32().map_err(|e| fmt_err(e, at))?;
        // Every message takes at least its one-byte VLE count.
        let cap = r
            .ensure_elements(count, 1)
            .map_err(|e| fmt_err(e, pos(&r)))?;
        let mut spans = Vec::new();
        spans
            .try_reserve(cap)
            .map_err(|_| WvmError::Alloc { what: "messages" })?;
        let mut units = Vec::new();
        // All units fit in the remaining bytes.
        units
            .try_reserve_exact(r.remaining() / 2)
            .map_err(|_| WvmError::Alloc {
                what: "message text",
            })?;
        for _ in 0..count {
            let at = pos(&r);
            let k = r.vle().map_err(|e| fmt_err(e, at))?;
            let bytes = usize::try_from(k)
                .ok()
                .and_then(|k| k.checked_mul(2))
                .unwrap_or(usize::MAX);
            let at = pos(&r);
            let raw = r.bytes(bytes).map_err(|e| fmt_err(e, at))?;
            let first = u32::try_from(units.len())
                .map_err(|_| WvmError::TooLarge { len: payload.len() })?;
            units.extend(
                raw.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&c| u16::from_le_bytes(c)),
            );
            spans.push((first, k));
        }
        let at = pos(&r);
        let array_count = r.u32().map_err(|e| fmt_err(e, at))?;
        let mut array_values = 0u64;
        for _ in 0..array_count {
            let at = pos(&r);
            let c = r.u32().map_err(|e| fmt_err(e, at))?;
            let at = pos(&r);
            r.ensure_elements(c, 4).map_err(|e| fmt_err(e, at))?;
            r.skip(usize::try_from(c).unwrap_or(usize::MAX).saturating_mul(4))
                .map_err(|e| fmt_err(e, at))?;
            array_values += u64::from(c);
        }
        if !r.is_empty() {
            return Err(WvmError::container(
                pos(&r),
                format!("{} bytes after the message arrays", r.remaining()),
            ));
        }
        units.shrink_to_fit();
        Ok(Self {
            units,
            spans,
            array_count,
            array_values,
        })
    }

    /// Number of messages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// The code units of message `index`.
    #[must_use]
    pub fn units(&self, index: u32) -> Option<&[u16]> {
        let &(first, count) = self.spans.get(usize::try_from(index).ok()?)?;
        let first = first as usize;
        self.units.get(first..first.checked_add(count as usize)?)
    }

    /// Message `index` as text (lossy for unpaired surrogates).
    #[must_use]
    pub fn text(&self, index: u32) -> Option<String> {
        self.units(index).map(String::from_utf16_lossy)
    }

    /// Unit count of each message, in order.
    pub fn lengths(&self) -> impl Iterator<Item = u32> + '_ {
        self.spans.iter().map(|&(_, n)| n)
    }

    /// Total number of code units.
    #[must_use]
    pub fn total_units(&self) -> usize {
        self.units.len()
    }
}
