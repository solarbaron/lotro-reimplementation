// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded Ogg page parser and packet assembly (`docs/specs/sound.md` §3.3, §12.2 step 4,
//! §13 SN3).
//!
//! The parser walks every page of a payload, verifies its CRC, and records the byte ranges
//! of the packets it carries together with the page on which each packet begins and
//! completes, which the played-range arithmetic of §3.3.2 needs. It never copies packet
//! data while parsing; [`OggStream::packet`] borrows a packet that lies inside one page and
//! copies only packets that span pages.
//!
//! Structural faults (bad capture pattern, version, truncated page, CRC mismatch, a second
//! logical stream, a continuation flag that disagrees with the lacing, an unterminated last
//! packet) are errors. Departures from the census invariants that do not stop decoding
//! (beginning- and end-of-stream flags, granule order, sequence gaps, trailing bytes) are
//! reported as [`OggIssue`]s.
//!
//! Allocation is linear in the input: at most one [`PageInfo`] per 27 input bytes and one
//! [`PacketInfo`] per lacing value.

use std::borrow::Cow;

use super::DecodeError;

/// Size of the fixed part of a page header (capture pattern to segment count).
pub const PAGE_HEADER_LEN: usize = 27;

/// Header-type flag: the first packet of the page continues a packet of the previous page.
pub const FLAG_CONTINUED: u8 = 0x01;
/// Header-type flag: beginning of stream.
pub const FLAG_BOS: u8 = 0x02;
/// Header-type flag: end of stream.
pub const FLAG_EOS: u8 = 0x04;

/// The granule value that means "no packet completes on this page".
pub const GRANULE_NONE: u64 = u64::MAX;

/// One parsed page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    /// Offset of the page's capture pattern in the stream.
    pub offset: usize,
    /// Header-type flags ([`FLAG_CONTINUED`], [`FLAG_BOS`], [`FLAG_EOS`]).
    pub flags: u8,
    /// Granule position as stored ([`GRANULE_NONE`] for −1).
    pub granule: u64,
    /// Bitstream serial number.
    pub serial: u32,
    /// Page sequence number.
    pub sequence: u32,
    /// Offset of the page body in the stream.
    pub body_offset: usize,
    /// Length of the page body.
    pub body_len: usize,
    /// Number of packets that complete on this page.
    pub packets_completed: u32,
}

impl PageInfo {
    /// Whether the granule position is set (not −1).
    #[must_use]
    pub fn has_granule(&self) -> bool {
        self.granule != GRANULE_NONE
    }
}

/// Where one packet lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketInfo {
    /// Index of the page on which the packet begins.
    pub start_page: u32,
    /// Index of the page on which the packet completes.
    pub end_page: u32,
    /// Offset of the packet's first byte in the stream.
    pub offset: usize,
    /// Packet length in bytes (0 for a zero-length packet).
    pub len: usize,
}

impl PacketInfo {
    /// Whether the packet begins and completes on page `page`.
    #[must_use]
    pub fn within(&self, page: u32) -> bool {
        self.start_page == page && self.end_page == page
    }
}

/// A census departure that does not stop decoding (`sound.md` §13 SN3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OggIssue {
    /// The first page lacks the beginning-of-stream flag.
    FirstPageNotBos,
    /// A page other than the first carries the beginning-of-stream flag.
    LateBos {
        /// Page index.
        page: u32,
    },
    /// The last page lacks the end-of-stream flag.
    LastPageNotEos,
    /// A page other than the last carries the end-of-stream flag.
    EarlyEos {
        /// Page index.
        page: u32,
    },
    /// A page's sequence number is not one more than the previous page's.
    SequenceGap {
        /// Page index.
        page: u32,
    },
    /// A page's granule is below an earlier page's granule.
    GranuleDecreasing {
        /// Page index.
        page: u32,
    },
    /// A page carries granule −1.
    GranuleUnset {
        /// Page index.
        page: u32,
    },
    /// Bytes after the last page that do not form a page.
    TrailingBytes {
        /// Number of bytes.
        len: usize,
    },
    /// The identification header is not 30 bytes (SN3 census).
    IdentHeaderLength {
        /// Its length.
        len: usize,
    },
    /// The first page that completes an audio packet is not the last page and its granule
    /// is below the samples decoded up to it (SN3a; samples to drop at the start).
    GranuleBelowDecoded {
        /// Page index.
        page: u32,
    },
    /// A zero-length packet that is not the sole packet of the last page (SN3a).
    ZeroLengthPacket {
        /// Page on which it completes.
        page: u32,
    },
    /// The setup header completes on the last page, which also completes every audio
    /// packet: page *a* of §3.3.2 does not exist, so the skipped lead *A* is all *T*
    /// samples and the original's played range is empty (*M* = 0). None in this install;
    /// what the original does is OPEN (§3.3.3, case 5).
    NoPageAfterSetup,
}

/// A parsed Ogg stream: its pages and packets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OggStream {
    /// Every page, in stream order.
    pub pages: Vec<PageInfo>,
    /// Every packet, in stream order, including zero-length packets.
    pub packets: Vec<PacketInfo>,
    /// Census departures that did not stop parsing.
    pub issues: Vec<OggIssue>,
}

/// The Ogg page CRC: polynomial 0x04C11DB7, initial value 0, no reflection, no final XOR.
/// Computed over the whole page with the CRC field set to zero.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    crc32_update(0, bytes)
}

/// Slicing-by-8 tables: `CRC_TABLES[0]` is the byte-wise table; `CRC_TABLES[k][i]` is the
/// CRC register after byte `i` followed by `k` zero bytes.
const CRC_TABLES: [[u32; 256]; 8] = {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut r = (i as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            r = if r & 0x8000_0000 != 0 {
                (r << 1) ^ 0x04C1_1DB7
            } else {
                r << 1
            };
            bit += 1;
        }
        t[0][i] = r;
        i += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = t[k - 1][i];
            t[k][i] = (prev << 8) ^ t[0][(prev >> 24) as usize];
            i += 1;
        }
        k += 1;
    }
    t
};

/// Byte-at-a-time update (the reference form; also the tail of [`crc32_update`]).
fn crc32_update_bytewise(mut crc: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        crc = (crc << 8) ^ CRC_TABLES[0][usize::from((crc >> 24) as u8 ^ b)];
    }
    crc
}

/// CRC update, eight bytes per step (page CRCs are checked over every byte of a payload,
/// up to about 41 MB for the longest music tracks).
fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    let t = &CRC_TABLES;
    let (chunks, rest) = bytes.as_chunks::<8>();
    for c in chunks {
        let one = crc ^ u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        crc = t[7][(one >> 24) as usize]
            ^ t[6][((one >> 16) & 0xFF) as usize]
            ^ t[5][((one >> 8) & 0xFF) as usize]
            ^ t[4][(one & 0xFF) as usize]
            ^ t[3][usize::from(c[4])]
            ^ t[2][usize::from(c[5])]
            ^ t[1][usize::from(c[6])]
            ^ t[0][usize::from(c[7])];
    }
    crc32_update_bytewise(crc, rest)
}

/// Builds one page with a correct CRC. For tests, tools and fuzzing; the decoder never
/// writes pages. `lacing` is the segment table (at most 255 values) and `body` must be
/// exactly as long as the lacing values add up to; otherwise `None`.
#[must_use]
pub fn build_page(
    flags: u8,
    granule: u64,
    serial: u32,
    sequence: u32,
    lacing: &[u8],
    body: &[u8],
) -> Option<Vec<u8>> {
    let nsegs = u8::try_from(lacing.len()).ok()?;
    let total: usize = lacing.iter().map(|&v| usize::from(v)).sum();
    if total != body.len() {
        return None;
    }
    let mut page = Vec::with_capacity(PAGE_HEADER_LEN + lacing.len() + body.len());
    page.extend_from_slice(b"OggS");
    page.push(0);
    page.push(flags);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&serial.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&[0; 4]);
    page.push(nsegs);
    page.extend_from_slice(lacing);
    page.extend_from_slice(body);
    let crc = crc32(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    Some(page)
}

/// Splits a packet length into lacing values (255 … 255, remainder). A packet whose length
/// is a multiple of 255 ends with a 0 value, so a zero-length packet is the single value 0.
#[must_use]
pub fn lacing_for(len: usize) -> Vec<u8> {
    let mut v = vec![255u8; len / 255];
    v.push((len % 255) as u8);
    v
}

fn page_err(offset: usize, reason: &'static str) -> DecodeError {
    DecodeError::OggPage { offset, reason }
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&data[at..at + 4]);
    u32::from_le_bytes(b)
}

impl OggStream {
    /// Parses every page of `data` and assembles packets.
    ///
    /// # Errors
    /// [`DecodeError::OggPage`] for a structural fault (see the module documentation).
    pub fn parse(data: &[u8]) -> Result<Self, DecodeError> {
        let mut s = Self::default();
        let mut pos = 0usize;
        // The packet being assembled: (start page, offset, length so far).
        let mut open: Option<(u32, usize, usize)> = None;
        let mut serial = None;
        let mut max_granule: Option<u64> = None;
        while pos < data.len() {
            let rest = &data[pos..];
            if rest.len() < PAGE_HEADER_LEN || !rest.starts_with(b"OggS") {
                if s.pages.is_empty() {
                    return Err(page_err(pos, "missing capture pattern"));
                }
                // A tail that is not a page (SN3: none in this install).
                if rest.starts_with(b"OggS") {
                    return Err(page_err(pos, "truncated page header"));
                }
                s.issues.push(OggIssue::TrailingBytes { len: rest.len() });
                break;
            }
            if rest[4] != 0 {
                return Err(page_err(pos, "unsupported stream structure version"));
            }
            let flags = rest[5];
            if flags & !(FLAG_CONTINUED | FLAG_BOS | FLAG_EOS) != 0 {
                return Err(page_err(pos, "unknown header-type flags"));
            }
            let mut g = [0u8; 8];
            g.copy_from_slice(&rest[6..14]);
            let granule = u64::from_le_bytes(g);
            let page_serial = u32_at(rest, 14);
            let sequence = u32_at(rest, 18);
            let stored_crc = u32_at(rest, 22);
            let nsegs = usize::from(rest[26]);
            let lacing_end = PAGE_HEADER_LEN + nsegs;
            if rest.len() < lacing_end {
                return Err(page_err(pos, "truncated segment table"));
            }
            let lacing = &rest[PAGE_HEADER_LEN..lacing_end];
            let body_len: usize = lacing.iter().map(|&v| usize::from(v)).sum();
            let page_len = lacing_end + body_len;
            if rest.len() < page_len {
                return Err(page_err(pos, "truncated page body"));
            }
            let crc = crc32_update(
                crc32_update(crc32_update(0, &rest[..22]), &[0; 4]),
                &rest[26..page_len],
            );
            if crc != stored_crc {
                return Err(page_err(pos, "page CRC mismatch"));
            }
            match serial {
                None => serial = Some(page_serial),
                Some(sn) if sn != page_serial => {
                    return Err(page_err(pos, "more than one logical stream"));
                }
                Some(_) => {}
            }
            let index =
                u32::try_from(s.pages.len()).map_err(|_| page_err(pos, "too many pages"))?;
            if flags & FLAG_CONTINUED != 0 && open.is_none() {
                return Err(page_err(pos, "continuation flag without an open packet"));
            }
            if flags & FLAG_CONTINUED == 0 && open.is_some() {
                return Err(page_err(pos, "open packet not continued"));
            }
            // Page-level census checks.
            if index == 0 {
                if flags & FLAG_BOS == 0 {
                    s.issues.push(OggIssue::FirstPageNotBos);
                }
            } else {
                if flags & FLAG_BOS != 0 {
                    s.issues.push(OggIssue::LateBos { page: index });
                }
                if let Some(prev) = s.pages.last() {
                    if prev.flags & FLAG_EOS != 0 {
                        s.issues.push(OggIssue::EarlyEos { page: index - 1 });
                    }
                    if sequence != prev.sequence.wrapping_add(1) {
                        s.issues.push(OggIssue::SequenceGap { page: index });
                    }
                }
            }
            if granule == GRANULE_NONE {
                s.issues.push(OggIssue::GranuleUnset { page: index });
            } else {
                if max_granule.is_some_and(|m| granule < m) {
                    s.issues.push(OggIssue::GranuleDecreasing { page: index });
                }
                max_granule = Some(max_granule.map_or(granule, |m| m.max(granule)));
            }
            // Packets.
            let body_offset = pos + lacing_end;
            let mut at = body_offset;
            let mut completed = 0u32;
            for &v in lacing {
                let v = usize::from(v);
                let (start_page, offset, len) = open.take().unwrap_or((index, at, 0));
                let len = len + v;
                at += v;
                if v < 255 {
                    s.packets.push(PacketInfo {
                        start_page,
                        end_page: index,
                        offset,
                        len,
                    });
                    completed += 1;
                } else {
                    open = Some((start_page, offset, len));
                }
            }
            s.pages.push(PageInfo {
                offset: pos,
                flags,
                granule,
                serial: page_serial,
                sequence,
                body_offset,
                body_len,
                packets_completed: completed,
            });
            pos += page_len;
        }
        if s.pages.is_empty() {
            return Err(page_err(0, "no pages"));
        }
        if open.is_some() {
            return Err(page_err(pos, "last packet never completes"));
        }
        if s.pages.last().is_some_and(|p| p.flags & FLAG_EOS == 0) {
            s.issues.push(OggIssue::LastPageNotEos);
        }
        Ok(s)
    }

    /// The bytes of packet `index`: borrowed when the packet lies in one page, copied when
    /// it spans pages. `None` for an index out of range.
    #[must_use]
    pub fn packet<'d>(&self, data: &'d [u8], index: usize) -> Option<Cow<'d, [u8]>> {
        let p = self.packets.get(index)?;
        if p.start_page == p.end_page {
            return data.get(p.offset..p.offset + p.len).map(Cow::Borrowed);
        }
        let mut out = Vec::with_capacity(p.len);
        let mut page = usize::try_from(p.start_page).ok()?;
        let mut from = p.offset;
        while out.len() < p.len {
            let info = self.pages.get(page)?;
            let body_end = info.body_offset + info.body_len;
            let take = (body_end.checked_sub(from)?).min(p.len - out.len());
            out.extend_from_slice(data.get(from..from + take)?);
            page += 1;
            from = self.pages.get(page).map_or(body_end, |n| n.body_offset);
        }
        Some(Cow::Owned(out))
    }

    /// Copies packet `index` into `buf` (cleared first). Returns false for an index out of
    /// range.
    pub fn packet_into(&self, data: &[u8], index: usize, buf: &mut Vec<u8>) -> bool {
        buf.clear();
        match self.packet(data, index) {
            Some(p) => {
                buf.extend_from_slice(&p);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_check_value() {
        // CRC-32 with polynomial 0x04C11DB7, init 0, no reflection, no final XOR.
        assert_eq!(crc32(b"123456789"), 0x89A1_897F);
    }

    #[test]
    fn crc_slicing_matches_bytewise() {
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        let data: Vec<u8> = (0..1000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect();
        for len in 0..80 {
            for start in [0usize, 1, 3, 7] {
                let d = &data[start..start + len];
                for init in [0u32, 0xDEAD_BEEF] {
                    assert_eq!(crc32_update(init, d), crc32_update_bytewise(init, d));
                }
            }
        }
        assert_eq!(crc32_update(0, &data), crc32_update_bytewise(0, &data));
    }

    #[test]
    fn packet_spanning_pages() {
        let body: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        let p0 = build_page(FLAG_BOS, GRANULE_NONE, 7, 0, &[255], &body[..255]).unwrap();
        let p1 = build_page(FLAG_CONTINUED | FLAG_EOS, 10, 7, 1, &[45, 0], &body[255..]).unwrap();
        let data = [p0, p1].concat();
        let s = OggStream::parse(&data).unwrap();
        assert_eq!(s.pages.len(), 2);
        assert_eq!(s.packets.len(), 2);
        assert_eq!(s.packets[0].start_page, 0);
        assert_eq!(s.packets[0].end_page, 1);
        assert_eq!(&*s.packet(&data, 0).unwrap(), &body[..]);
        assert_eq!(s.packets[1].len, 0);
        assert_eq!(s.issues, vec![OggIssue::GranuleUnset { page: 0 }]);
    }

    #[test]
    fn crc_mismatch_rejected() {
        let mut p = build_page(FLAG_BOS | FLAG_EOS, 0, 1, 0, &[1], &[9]).unwrap();
        p[27] ^= 1;
        assert!(matches!(
            OggStream::parse(&p),
            Err(DecodeError::OggPage { reason, .. }) if reason.contains("CRC")
        ));
    }

    #[test]
    fn lacing_values() {
        assert_eq!(lacing_for(0), vec![0]);
        assert_eq!(lacing_for(255), vec![255, 0]);
        assert_eq!(lacing_for(256), vec![255, 1]);
    }
}
