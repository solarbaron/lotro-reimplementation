// SPDX-License-Identifier: GPL-3.0-or-later
//! A bounded walk over the Vorbis I setup header (VORBIS §4.2.4) that extracts the mode
//! block flags and checks every count against the bits that remain.
//!
//! Two reasons for walking the header ourselves before the decoder library sees it:
//!
//! - the played-range arithmetic of `sound.md` §3.3.2 needs each audio packet's block size
//!   (its mode's block flag) without decoding the packet, and the library keeps the modes
//!   private;
//! - the library sizes some tables from header counts before reading them (codebook
//!   entries, lookup values, the expanded lookup table), so a hostile header could ask it
//!   for an unbounded allocation. This walk rejects such headers first.
//!
//! The walk accepts a strict subset of what a Vorbis I decoder accepts; the limits below are
//! far above anything a real encoder writes, and the real-data test confirms that every
//! stream of the install passes. Besides the counts it rejects headers that the decoder
//! library would accept but cannot decode without panicking or producing non-finite
//! samples:
//!
//! - a codebook with no entries (`entries` = 0);
//! - a codebook lookup whose minimum or delta value does not fit an `f32` (VORBIS §9.2.2
//!   `float32_unpack`, exponent field above 788 + 127 or a value above `f32::MAX`);
//! - a residue stage book (a cascade bit's book) without a lookup: VORBIS §3.3 makes using
//!   such a book for VQ an error;
//! - a residue cascade with bit 7 set: the library reads only the books of bits 0–6, so the
//!   rest of its header walk would disagree with ours;
//! - floor type 0. It is obsolete since libvorbis 1.0, no stream of the install uses it
//!   (the real-data test checks), and the library's floor-0 curve has unchecked arithmetic
//!   for small orders and wide amplitude fields. Rejected as
//!   [`DecodeError::Unsupported`].
//!
//! A sparse codebook whose entries are all unused is legal (VORBIS §3.2.1) and encoders
//! write such books as placeholders: two streams of the install name one as a residue
//! stage book of a classification their packets never choose. The decoder library builds
//! an empty Huffman tree for it and panics if a packet ever reads from it. The walk
//! therefore records where each such book's first entry flag sits, and
//! [`patch_unused_books`] gives the library a copy of the header in which that entry is
//! used with a 1-bit codeword. A stream that never reads the book decodes exactly as
//! before; a hostile packet that does reads entry 0 instead of panicking (a conforming
//! decoder would reject that packet).

use super::DecodeError;

/// Largest codebook (entries) accepted.
pub const MAX_CODEBOOK_ENTRIES: u32 = 1 << 16;
/// Largest sum of expanded lookup tables (entries × dimensions) over the whole header.
pub const MAX_LOOKUP_TABLE: u64 = 1 << 21;
/// Largest sum of codebook entries over the whole header.
pub const MAX_TOTAL_ENTRIES: u64 = 1 << 20;

/// What the walk keeps: one block flag per mode (true = long block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupModes {
    /// `mode_blockflag` of each mode, in mode-number order (1 to 64 modes).
    pub block_flags: Vec<bool>,
    /// Bit positions (after the 7-byte common header, LSB first) of the first entry flag
    /// of every sparse codebook whose entries are all unused, in header order.
    pub unused_books: Vec<u64>,
}

impl SetupModes {
    /// Number of bits of a mode number in an audio packet: ilog(modes − 1).
    #[must_use]
    pub fn mode_bits(&self) -> u32 {
        ilog(self.block_flags.len().saturating_sub(1) as u64)
    }
}

/// ilog of VORBIS §9.2.1: the position of the highest set bit, 0 for 0.
#[must_use]
pub fn ilog(x: u64) -> u32 {
    64 - x.leading_zeros()
}

/// LSB-first bit reader (VORBIS §2).
#[derive(Debug)]
pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: u64,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> u64 {
        (self.data.len() as u64 * 8).saturating_sub(self.pos)
    }

    /// Reads `n` ≤ 32 bits.
    pub(crate) fn read(&mut self, n: u32) -> Option<u32> {
        debug_assert!(n <= 32);
        if u64::from(n) > self.remaining() {
            return None;
        }
        let mut v: u64 = 0;
        for i in 0..u64::from(n) {
            let bit = self.pos + i;
            let byte = self.data[(bit / 8) as usize];
            v |= u64::from((byte >> (bit % 8)) & 1) << i;
        }
        self.pos += u64::from(n);
        Some(v as u32)
    }

    fn flag(&mut self) -> Option<bool> {
        self.read(1).map(|b| b == 1)
    }

    fn skip(&mut self, n: u64) -> Option<()> {
        if n > self.remaining() {
            return None;
        }
        self.pos += n;
        Some(())
    }
}

fn bad(reason: &'static str) -> DecodeError {
    DecodeError::VorbisHeader { reason }
}

fn eop() -> DecodeError {
    bad("setup header ends early")
}

/// Whether a codebook lookup value word (VORBIS §9.2.2 `float32_unpack`: 21-bit mantissa,
/// 10-bit exponent biased by 788, sign) unpacks to a value an `f32` holds, with a scale
/// factor 2^(exponent − 788) that is itself finite as an `f32`.
#[must_use]
pub fn float32_unpack_fits(word: u32) -> bool {
    let mantissa = word & 0x1F_FFFF;
    let exponent = i32::try_from((word >> 21) & 0x3FF).unwrap_or(0) - 788;
    if exponent > 127 {
        return false;
    }
    let value = f64::from(mantissa) * 2f64.powi(exponent);
    value <= f64::from(f32::MAX)
}

/// What the walk remembers of one codebook.
#[derive(Debug, Clone, Copy)]
struct Book {
    has_lookup: bool,
}

/// The greatest r with r^dims ≤ entries (VORBIS §9.2.3); `dims` ≥ 1.
#[must_use]
pub fn lookup1_values(entries: u32, dims: u32) -> u64 {
    if dims == 0 || entries == 0 {
        return 0;
    }
    let fits = |r: u64| -> bool {
        let mut acc: u64 = 1;
        for _ in 0..dims {
            acc = match acc.checked_mul(r) {
                Some(a) if a <= u64::from(entries) => a,
                _ => return false,
            };
        }
        true
    };
    let mut r = (f64::from(entries).powf(1.0 / f64::from(dims))).floor() as u64;
    while r > 0 && !fits(r) {
        r -= 1;
    }
    while fits(r + 1) {
        r += 1;
    }
    r
}

/// Walks a setup header packet (including its 7-byte common header) for a stream with
/// `channels` channels.
///
/// # Errors
/// [`DecodeError::VorbisHeader`] for a malformed header and [`DecodeError::LimitExceeded`]
/// for one whose tables exceed the limits of this module.
pub fn walk_setup(packet: &[u8], channels: u8) -> Result<SetupModes, DecodeError> {
    if packet.len() < 7 || packet[0] != 5 || &packet[1..7] != b"vorbis" {
        return Err(bad("not a setup header"));
    }
    let mut b = Bits::new(&packet[7..]);
    let r = |b: &mut Bits<'_>, n: u32| b.read(n).ok_or_else(eop);

    // Codebooks.
    let codebooks = r(&mut b, 8)? + 1;
    let mut books: Vec<Book> = Vec::with_capacity(codebooks as usize);
    let mut total_entries: u64 = 0;
    let mut total_lookup: u64 = 0;
    let mut unused_books = Vec::new();
    for _ in 0..codebooks {
        if r(&mut b, 24)? != 0x56_4342 {
            return Err(bad("codebook sync pattern"));
        }
        let dims = r(&mut b, 16)?;
        let entries = r(&mut b, 24)?;
        if dims == 0 {
            return Err(bad("codebook with zero dimensions"));
        }
        if entries == 0 {
            return Err(bad("codebook with no entries"));
        }
        if entries > MAX_CODEBOOK_ENTRIES {
            return Err(DecodeError::LimitExceeded {
                what: "codebook entries",
                value: u64::from(entries),
                limit: u64::from(MAX_CODEBOOK_ENTRIES),
            });
        }
        total_entries += u64::from(entries);
        if total_entries > MAX_TOTAL_ENTRIES {
            return Err(DecodeError::LimitExceeded {
                what: "codebook entries in the setup header",
                value: total_entries,
                limit: MAX_TOTAL_ENTRIES,
            });
        }
        let ordered = b.flag().ok_or_else(eop)?;
        if ordered {
            let mut length = r(&mut b, 5)? + 1;
            let mut current = 0u32;
            while current < entries {
                if length > 32 {
                    return Err(bad("ordered codeword length above 32"));
                }
                let number = r(&mut b, ilog(u64::from(entries - current)))?;
                current = current
                    .checked_add(number)
                    .filter(|&c| c <= entries)
                    .ok_or_else(|| bad("ordered codeword lengths overrun the entries"))?;
                length += 1;
            }
        } else {
            let sparse = b.flag().ok_or_else(eop)?;
            // Each entry takes at least one bit (sparse) or exactly five.
            let min_bits = u64::from(entries) * if sparse { 1 } else { 5 };
            if min_bits > b.remaining() {
                return Err(eop());
            }
            let first_flag = b.pos;
            let mut used = 0u32;
            for _ in 0..entries {
                if !sparse || b.flag().ok_or_else(eop)? {
                    r(&mut b, 5)?;
                    used += 1;
                }
            }
            if used == 0 {
                // `used` = 0 needs the sparse flag: a dense book uses every entry.
                unused_books.push(first_flag);
            }
        }
        let lookup = r(&mut b, 4)?;
        match lookup {
            0 => {}
            1 | 2 => {
                let table = u64::from(entries) * u64::from(dims);
                total_lookup += table;
                if total_lookup > MAX_LOOKUP_TABLE {
                    return Err(DecodeError::LimitExceeded {
                        what: "codebook lookup tables",
                        value: total_lookup,
                        limit: MAX_LOOKUP_TABLE,
                    });
                }
                let minimum = r(&mut b, 32)?;
                let delta = r(&mut b, 32)?;
                if !float32_unpack_fits(minimum) || !float32_unpack_fits(delta) {
                    return Err(bad("codebook lookup value does not fit an f32"));
                }
                let value_bits = r(&mut b, 4)? + 1;
                r(&mut b, 1)?;
                let values = if lookup == 1 {
                    lookup1_values(entries, dims)
                } else {
                    table
                };
                b.skip(values * u64::from(value_bits)).ok_or_else(eop)?;
            }
            _ => return Err(bad("codebook lookup type above 2")),
        }
        books.push(Book {
            has_lookup: lookup != 0,
        });
    }

    // Time-domain transforms: placeholders, all zero.
    let times = r(&mut b, 6)? + 1;
    for _ in 0..times {
        if r(&mut b, 16)? != 0 {
            return Err(bad("non-zero time-domain transform"));
        }
    }

    // Floors.
    let floors = r(&mut b, 6)? + 1;
    for _ in 0..floors {
        match r(&mut b, 16)? {
            0 => {
                return Err(DecodeError::Unsupported {
                    what: "Vorbis floor type",
                    value: 0,
                });
            }
            1 => {
                let partitions = r(&mut b, 5)?;
                let mut classes = Vec::with_capacity(partitions as usize);
                let mut max_class: i32 = -1;
                for _ in 0..partitions {
                    let c = r(&mut b, 4)?;
                    max_class = max_class.max(c as i32);
                    classes.push(c);
                }
                let mut class_dims = Vec::with_capacity(16);
                for _ in 0..=max_class {
                    class_dims.push(r(&mut b, 3)? + 1);
                    let subclasses = r(&mut b, 2)?;
                    if subclasses != 0 && r(&mut b, 8)? >= codebooks {
                        return Err(bad("floor 1 master book out of range"));
                    }
                    for _ in 0..(1u32 << subclasses) {
                        // Stored as book + 1; 0 means "no book".
                        let book = r(&mut b, 8)?;
                        if book > codebooks {
                            return Err(bad("floor 1 subclass book out of range"));
                        }
                    }
                }
                r(&mut b, 2)?;
                let range_bits = r(&mut b, 4)?;
                let mut xs: Vec<u32> = vec![0, 1 << range_bits];
                for &c in &classes {
                    for _ in 0..class_dims[c as usize] {
                        xs.push(r(&mut b, range_bits)?);
                    }
                }
                if xs.len() > 65 {
                    return Err(bad("floor 1 with more than 65 values"));
                }
                xs.sort_unstable();
                if xs.windows(2).any(|w| w[0] == w[1]) {
                    return Err(bad("floor 1 with duplicate X values"));
                }
            }
            _ => return Err(bad("floor type above 1")),
        }
    }

    // Residues.
    let residues = r(&mut b, 6)? + 1;
    for _ in 0..residues {
        if r(&mut b, 16)? > 2 {
            return Err(bad("residue type above 2"));
        }
        let begin = r(&mut b, 24)?;
        let end = r(&mut b, 24)?;
        if begin > end {
            return Err(bad("residue begins after its end"));
        }
        r(&mut b, 24)?;
        let classifications = r(&mut b, 6)? + 1;
        if r(&mut b, 8)? >= codebooks {
            return Err(bad("residue class book out of range"));
        }
        let mut cascade = Vec::with_capacity(classifications as usize);
        for _ in 0..classifications {
            let low = r(&mut b, 3)?;
            let high = if b.flag().ok_or_else(eop)? {
                r(&mut b, 5)?
            } else {
                0
            };
            if high & 0x10 != 0 {
                return Err(bad(
                    "residue cascade bit 7 is not supported by the decoder library",
                ));
            }
            cascade.push((high << 3) | low);
        }
        for c in cascade {
            for _ in 0..c.count_ones() {
                let book = r(&mut b, 8)?;
                match books.get(book as usize) {
                    None => return Err(bad("residue book out of range")),
                    Some(bk) if !bk.has_lookup => {
                        return Err(bad("residue book without a lookup"));
                    }
                    Some(_) => {}
                }
            }
        }
    }

    // Mappings.
    let mappings = r(&mut b, 6)? + 1;
    let chan_bits = ilog(u64::from(channels.saturating_sub(1)));
    for _ in 0..mappings {
        if r(&mut b, 16)? != 0 {
            return Err(bad("mapping type is not 0"));
        }
        let submaps = if b.flag().ok_or_else(eop)? {
            r(&mut b, 4)? + 1
        } else {
            1
        };
        if b.flag().ok_or_else(eop)? {
            let steps = r(&mut b, 8)? + 1;
            for _ in 0..steps {
                let mag = r(&mut b, chan_bits)?;
                let ang = r(&mut b, chan_bits)?;
                if mag == ang || mag >= u32::from(channels) || ang >= u32::from(channels) {
                    return Err(bad("invalid channel coupling"));
                }
            }
        }
        if r(&mut b, 2)? != 0 {
            return Err(bad("mapping reserved bits"));
        }
        if submaps > 1 {
            for _ in 0..channels {
                if r(&mut b, 4)? >= submaps {
                    return Err(bad("mapping mux out of range"));
                }
            }
        }
        for _ in 0..submaps {
            r(&mut b, 8)?;
            if r(&mut b, 8)? >= floors || r(&mut b, 8)? >= residues {
                return Err(bad("submap floor or residue out of range"));
            }
        }
    }

    // Modes.
    let modes = r(&mut b, 6)? + 1;
    let mut block_flags = Vec::with_capacity(modes as usize);
    for _ in 0..modes {
        let flag = b.flag().ok_or_else(eop)?;
        if r(&mut b, 16)? != 0 || r(&mut b, 16)? != 0 {
            return Err(bad("mode window or transform type is not 0"));
        }
        if r(&mut b, 8)? >= mappings {
            return Err(bad("mode mapping out of range"));
        }
        block_flags.push(flag);
    }
    if !b.flag().ok_or_else(eop)? {
        return Err(bad("setup header framing bit not set"));
    }
    Ok(SetupModes {
        block_flags,
        unused_books,
    })
}

/// The setup header to hand to the decoder library: `packet` itself when the walk found no
/// sparse codebook without a used entry, otherwise a copy in which the first entry of each
/// such book is used with a 1-bit codeword (its flag set, then a 5-bit length field of 0).
/// `modes` must come from [`walk_setup`] on the same packet.
#[must_use]
pub fn patch_unused_books(packet: &[u8], modes: &SetupModes) -> Option<Vec<u8>> {
    if modes.unused_books.is_empty() || packet.len() < 7 {
        return None;
    }
    let body = &packet[7..];
    let total = body.len() as u64 * 8;
    let mut out = BitSink::default();
    let mut from = 0u64;
    for &at in &modes.unused_books {
        if at < from || at >= total {
            return None;
        }
        out.copy(body, from, at);
        out.push(true);
        for _ in 0..5 {
            out.push(false);
        }
        // The original flag bit (0, unused) is replaced by the flag and length just written.
        from = at + 1;
    }
    out.copy(body, from, total);
    let mut patched = packet[..7].to_vec();
    patched.extend_from_slice(&out.bytes);
    Some(patched)
}

/// LSB-first bit writer for [`patch_unused_books`].
#[derive(Default)]
struct BitSink {
    bytes: Vec<u8>,
    len: u64,
}

impl BitSink {
    fn push(&mut self, bit: bool) {
        if self.len.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if bit && let Some(last) = self.bytes.last_mut() {
            *last |= 1 << (self.len % 8);
        }
        self.len += 1;
    }

    /// Copies bits `from..to` of `data`.
    fn copy(&mut self, data: &[u8], from: u64, to: u64) {
        for bit in from..to {
            self.push((data[(bit / 8) as usize] >> (bit % 8)) & 1 == 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ilog_values() {
        assert_eq!(ilog(0), 0);
        assert_eq!(ilog(1), 1);
        assert_eq!(ilog(2), 2);
        assert_eq!(ilog(3), 2);
        assert_eq!(ilog(4), 3);
        assert_eq!(ilog(7), 3);
    }

    #[test]
    fn lookup1() {
        assert_eq!(lookup1_values(1024, 10), 2);
        assert_eq!(lookup1_values(1023, 10), 1);
        assert_eq!(lookup1_values(3125, 5), 5);
        assert_eq!(lookup1_values(3124, 5), 4);
        assert_eq!(lookup1_values(81, 4), 3);
        assert_eq!(lookup1_values(65536, 1), 65536);
        assert_eq!(lookup1_values(0, 3), 0);
    }

    #[test]
    fn float32_unpack_range() {
        // 1.0: mantissa 2^20, exponent 768 (2^20 × 2^−20).
        assert!(float32_unpack_fits((768 << 21) | (1 << 20)));
        assert!(float32_unpack_fits(0));
        // (2^21 − 1) × 2^107 = 2^128 − 2^107 is still below f32::MAX = 2^128 − 2^104 ...
        assert!(float32_unpack_fits(((788 + 107) << 21) | 0x1F_FFFF));
        // ... (2^21 − 1) × 2^108 is above it.
        assert!(!float32_unpack_fits(((788 + 108) << 21) | 0x1F_FFFF));
        // The scale itself overflows, even with a zero mantissa (0 × inf = NaN).
        assert!(!float32_unpack_fits(0x7FE0_0000));
        assert!(!float32_unpack_fits(0xFFFF_FFFF));
        assert!(!float32_unpack_fits(((788 + 128) << 21) | 1));
    }

    #[test]
    fn patch_inserts_a_codeword_at_each_unused_book() {
        let mut packet = vec![5];
        packet.extend_from_slice(b"vorbis");
        packet.extend_from_slice(&[0b1111_0000, 0xAB]);
        let none = SetupModes {
            block_flags: vec![false],
            unused_books: vec![],
        };
        assert_eq!(patch_unused_books(&packet, &none), None);
        // Flags at bits 0 and 9 (both 0) become 1 followed by five 0 bits.
        let modes = SetupModes {
            block_flags: vec![false],
            unused_books: vec![0, 9],
        };
        let patched = patch_unused_books(&packet, &modes).unwrap();
        assert_eq!(&patched[..7], &packet[..7]);
        let mut b = Bits::new(&patched[7..]);
        assert_eq!(b.read(6), Some(1));
        assert_eq!(b.read(8), Some(0b1111_0000 >> 1 | (0xAB & 1) << 7));
        assert_eq!(b.read(6), Some(1));
        assert_eq!(b.read(6), Some(0xAB >> 2));
        // Positions out of order or outside the body are refused.
        let bad = SetupModes {
            block_flags: vec![false],
            unused_books: vec![9, 0],
        };
        assert_eq!(patch_unused_books(&packet, &bad), None);
    }

    #[test]
    fn bits_lsb_first() {
        let mut b = Bits::new(&[0b1010_0110, 0xFF]);
        assert_eq!(b.read(1), Some(0));
        assert_eq!(b.read(2), Some(0b11));
        assert_eq!(b.read(5), Some(0b10100));
        assert_eq!(b.read(8), Some(0xFF));
        assert_eq!(b.read(1), None);
    }
}
