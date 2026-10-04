// SPDX-License-Identifier: GPL-3.0-or-later
//! Allocation (extent) layer: modern first-extent + fragment table, and the optional
//! legacy chained layout (spec §4).

use crate::archive::Archive;
use crate::bytes::u32_at;
use crate::error::{Error, Result};
use crate::{HEADER_LEN, MAX_LEGACY_STEPS};

/// Size of the allocation header that starts nodes, record first extents and the free table.
pub(crate) const ALLOC_HEADER_LEN: u64 = 8;

/// One continuation fragment of a modern-layout allocation (spec §4.2).
///
/// A fragment is a raw byte range without a header of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fragment {
    /// Length of the fragment allocation in bytes.
    pub length: u32,
    /// Absolute offset of the fragment.
    pub offset: u32,
}

/// One block of a legacy chained allocation (spec §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyBlock {
    /// Size of the block including its 8-byte header.
    pub size: u32,
    /// Absolute offset of the block.
    pub offset: u32,
}

/// Physical layout of one allocation: where its payload bytes live (spec §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordLayout {
    /// Modern layout (header word +4 is 0): a first extent whose tail holds a table of
    /// `fragments.len()` continuation fragments (spec §4.2).
    Modern {
        /// Absolute offset of the first extent.
        offset: u32,
        /// Size of the first extent allocation.
        alloc_size: u32,
        /// Payload bytes the first extent can hold: `alloc_size − 8 − 8·F`.
        first_capacity: u32,
        /// Continuation fragments in payload order.
        fragments: Vec<Fragment>,
    },
    /// Legacy chained layout (header word +4 is not 0), stored tail-first (spec §4.3).
    /// `blocks[0]` is the allocation the pointer names; the last block supplies the start
    /// of the payload.
    Legacy {
        /// Chain blocks in walk order.
        blocks: Vec<LegacyBlock>,
    },
}

impl RecordLayout {
    /// Number of continuation fragments (modern) or extra blocks (legacy).
    pub fn extra_pieces(&self) -> usize {
        match self {
            Self::Modern { fragments, .. } => fragments.len(),
            Self::Legacy { blocks } => blocks.len().saturating_sub(1),
        }
    }

    /// Total payload bytes the layout can supply.
    pub fn capacity(&self) -> u64 {
        match self {
            Self::Modern {
                first_capacity,
                fragments,
                ..
            } => {
                u64::from(*first_capacity)
                    + fragments.iter().map(|f| u64::from(f.length)).sum::<u64>()
            }
            Self::Legacy { blocks } => blocks
                .iter()
                .map(|b| u64::from(b.size).saturating_sub(ALLOC_HEADER_LEN))
                .sum(),
        }
    }

    /// All byte ranges `(offset, len)` this allocation occupies in the file.
    pub fn extents(&self) -> Vec<(u64, u64)> {
        match self {
            Self::Modern {
                offset,
                alloc_size,
                fragments,
                ..
            } => std::iter::once((u64::from(*offset), u64::from(*alloc_size)))
                .chain(
                    fragments
                        .iter()
                        .map(|f| (u64::from(f.offset), u64::from(f.length))),
                )
                .collect(),
            Self::Legacy { blocks } => blocks
                .iter()
                .map(|b| (u64::from(b.offset), u64::from(b.size)))
                .collect(),
        }
    }
}

impl Archive {
    /// Reads the allocation header at `offset` and describes the allocation's layout.
    ///
    /// `alloc_size` comes from whoever points at the allocation (spec §4 table).
    pub(crate) fn layout_at(&self, offset: u32, alloc_size: u32) -> Result<RecordLayout> {
        self.layout_and_header(offset, alloc_size).map(|(l, _)| l)
    }

    /// Like [`Archive::layout_at`], also returning the allocation header words.
    pub(crate) fn layout_and_header(
        &self,
        offset: u32,
        alloc_size: u32,
    ) -> Result<(RecordLayout, [u32; 2])> {
        let off = u64::from(offset);
        let size = u64::from(alloc_size);
        self.check_range("allocation", off, size)?;
        if size < ALLOC_HEADER_LEN {
            return Err(Error::corrupt(off, format!("allocation size {size} < 8")));
        }
        let mut hdr = [0u8; 8];
        self.read_at(off, &mut hdr)?;
        let (w0, w1) = (u32_at(&hdr, 0), u32_at(&hdr, 4));
        if w1 != 0 {
            if !self.options().legacy_layout {
                return Err(Error::UnsupportedLegacyLayout { offset: off });
            }
            return Ok((self.legacy_chain(offset, alloc_size, w0, w1)?, [w0, w1]));
        }
        let frag_count = u64::from(w0);
        let table_len = frag_count * 8;
        if ALLOC_HEADER_LEN + table_len > size {
            return Err(Error::corrupt(
                off,
                format!("fragment count {frag_count} does not fit a {size}-byte allocation"),
            ));
        }
        let first_capacity = size - ALLOC_HEADER_LEN - table_len;
        let mut fragments = Vec::new();
        if frag_count > 0 {
            // The table size is otherwise bounded only by the allocation size, i.e. by the
            // file length (up to 4 GiB), so cap it before allocating anything.
            let max = self.options().max_fragments;
            if frag_count > max {
                return Err(Error::TooLarge {
                    what: "fragment table",
                    len: table_len,
                    limit: max.saturating_mul(8),
                });
            }
            if frag_count > 31 {
                tracing::debug!(offset = off, frag_count, "unusually many fragments");
            }
            // Bounded by `max_fragments`.
            let mut table = vec![0u8; table_len as usize];
            self.read_at(off + size - table_len, &mut table)?;
            fragments = table
                .chunks_exact(8)
                .map(|p| Fragment {
                    length: u32_at(p, 0),
                    offset: u32_at(p, 4),
                })
                .collect();
        }
        let layout = RecordLayout::Modern {
            offset,
            alloc_size,
            first_capacity: first_capacity as u32,
            fragments,
        };
        Ok((layout, [w0, w1]))
    }

    fn legacy_chain(&self, offset: u32, alloc_size: u32, w0: u32, w1: u32) -> Result<RecordLayout> {
        let mut blocks = vec![LegacyBlock {
            size: alloc_size,
            offset,
        }];
        let (mut next_size, mut next_off) = (w0, w1);
        while next_size != 0 {
            if blocks.len() >= MAX_LEGACY_STEPS {
                return Err(Error::corrupt(
                    u64::from(offset),
                    format!("legacy chain longer than {MAX_LEGACY_STEPS} blocks"),
                ));
            }
            let (o, s) = (u64::from(next_off), u64::from(next_size));
            self.check_range("legacy block", o, s)?;
            if s < ALLOC_HEADER_LEN {
                return Err(Error::corrupt(o, format!("legacy block size {s} < 8")));
            }
            let mut hdr = [0u8; 8];
            self.read_at(o, &mut hdr)?;
            blocks.push(LegacyBlock {
                size: next_size,
                offset: next_off,
            });
            next_size = u32_at(&hdr, 0);
            next_off = u32_at(&hdr, 4);
        }
        Ok(RecordLayout::Legacy { blocks })
    }

    /// Reassembles `len` payload bytes described by `layout` (spec §4.2 / §4.3).
    pub(crate) fn read_payload(&self, layout: &RecordLayout, len: u64) -> Result<Vec<u8>> {
        let limit = self.options().max_record_len;
        if len > limit {
            return Err(Error::TooLarge {
                what: "stored record",
                len,
                limit,
            });
        }
        if len > self.file_len() {
            return Err(Error::TooLarge {
                what: "stored record",
                len,
                limit: self.file_len(),
            });
        }
        // `len` ≤ file length ≤ u64; on 32-bit hosts this may not fit usize.
        let n = usize::try_from(len).map_err(|_| Error::TooLarge {
            what: "stored record",
            len,
            limit: usize::MAX as u64,
        })?;
        let mut buf = vec![0u8; n];
        match layout {
            RecordLayout::Modern {
                offset,
                first_capacity,
                fragments,
                ..
            } => {
                let first = n.min(*first_capacity as usize);
                self.read_at(u64::from(*offset) + ALLOC_HEADER_LEN, &mut buf[..first])?;
                let mut filled = first;
                for frag in fragments {
                    if filled == n {
                        break;
                    }
                    let take = (n - filled).min(frag.length as usize);
                    self.check_range("fragment", u64::from(frag.offset), take as u64)?;
                    self.read_at(u64::from(frag.offset), &mut buf[filled..filled + take])?;
                    filled += take;
                }
                if filled < n {
                    return Err(Error::corrupt(
                        u64::from(*offset),
                        format!("allocation supplies only {filled} of {n} payload bytes"),
                    ));
                }
            }
            RecordLayout::Legacy { blocks } => {
                // Tail-first: each block fills the buffer backwards from the end.
                let mut remaining = n;
                for b in blocks {
                    if remaining == 0 {
                        break;
                    }
                    let cap = (b.size as usize).saturating_sub(ALLOC_HEADER_LEN as usize);
                    let take = remaining.min(cap);
                    self.read_at(
                        u64::from(b.offset) + ALLOC_HEADER_LEN,
                        &mut buf[remaining - take..remaining],
                    )?;
                    remaining -= take;
                }
                if remaining > 0 {
                    let first = blocks.first().map_or(0, |b| u64::from(b.offset));
                    return Err(Error::corrupt(
                        first,
                        format!("legacy chain supplies only {} of {n} bytes", n - remaining),
                    ));
                }
            }
        }
        Ok(buf)
    }

    /// Checks that `[offset, offset+len)` lies inside the data area `[0x400, file length)`.
    pub(crate) fn check_range(&self, what: &'static str, offset: u64, len: u64) -> Result<()> {
        match offset.checked_add(len) {
            Some(end) if offset >= HEADER_LEN && end <= self.file_len() => Ok(()),
            _ => Err(Error::OutOfBounds {
                what,
                offset,
                len,
                file_len: self.file_len(),
            }),
        }
    }
}
