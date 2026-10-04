// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoders for the housekeeping records used by split-set verification and
//! the content fingerprint.
//!
//! Spec-Ref: docs/specs/install-layout.md#92-housekeeping-records
//! Spec-Ref: docs/specs/install-layout.md#93-decoding-the-iteration-run-list
//!
//! This crate does not read archives beyond their header, so these functions
//! take the record bytes as input. The dat container layer looks the records
//! up by DID and hands the (decompressed) bytes over.

/// DID of the iteration record.
pub const DID_ITERATION: u32 = 0xFFFF_0001;
/// DID of the install version stamp record.
pub const DID_INSTALL_STAMP: u32 = 0xFFFF_0003;
/// DID of the record at 0xFFFF0004 (meaning unknown; not used).
pub const DID_FFFF0004: u32 = 0xFFFF_0004;
/// DID of the split-set membership record.
pub const DID_SET_MEMBERSHIP: u32 = 0xFFFF_0005;
/// DID of the record at 0xFFFF0008 (meaning unknown; not used).
pub const DID_FFFF0008: u32 = 0xFFFF_0008;

/// Maximum trailing entries accepted in an iteration record.
pub const MAX_TRAILING_ENTRIES: u32 = 64;

/// 16 opaque bytes, compared bytewise and never interpreted.
pub type Stamp = [u8; 16];

/// Format a stamp as 32 lower-case hex digits in byte order.
pub fn stamp_hex(s: &Stamp) -> String {
    hex(s)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Why a housekeeping record is malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecordError {
    /// The record ended before a field.
    #[error("record truncated at offset {offset}")]
    Truncated {
        /// Offset of the field that did not fit.
        offset: usize,
    },
    /// The iteration count is negative.
    #[error("negative iteration count {0}")]
    NegativeCount(i32),
    /// The run list covers more iterations than the count.
    #[error("run list exceeds the iteration count")]
    CountOverrun,
    /// More trailing entries than allowed.
    #[error("{0} trailing entries exceeds the limit")]
    TooManyTrailing(u32),
    /// Bytes remain after the last field.
    #[error("{0} bytes left over")]
    LeftOver(usize),
    /// The 0xFFFF0003 label length is out of range or not ASCII.
    #[error("bad label")]
    BadLabel,
}

struct Cursor<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RecordError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or(RecordError::Truncated { offset: self.pos })?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, RecordError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn i32(&mut self) -> Result<i32, RecordError> {
        self.u32().map(|v| v as i32)
    }

    fn stamp(&mut self) -> Result<Stamp, RecordError> {
        let s = self.take(16)?;
        let mut out = [0u8; 16];
        out.copy_from_slice(s);
        Ok(out)
    }

    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
}

/// A contiguous range of iterations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationRange {
    /// First iteration.
    pub start: i64,
    /// Number of iterations (at least 1).
    pub len: u32,
}

impl IterationRange {
    /// Last iteration in the range.
    pub fn last(&self) -> i64 {
        self.start + i64::from(self.len) - 1
    }
}

/// One trailing entry of the iteration record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailingEntry {
    /// Iteration number.
    pub iteration: u32,
    /// Eight bytes of unknown meaning.
    pub opaque: [u8; 8],
}

/// Decoded 0xFFFF0001 iteration record (§9.2.1, §9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IterationRecord {
    /// `N`: the number of iterations this archive holds.
    pub count: u32,
    /// Runs and singles (a single is a range of length 1), in record order.
    pub ranges: Vec<IterationRange>,
    /// The trailing entries.
    pub trailing: Vec<TrailingEntry>,
}

impl IterationRecord {
    /// Decode the record. Malformed input is an error, never a panic.
    pub fn parse(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut c = Cursor { b: bytes, pos: 0 };
        let n = c.i32()?;
        if n < 0 {
            return Err(RecordError::NegativeCount(n));
        }
        let mut remaining = i64::from(n);
        let mut ranges = Vec::new();
        while remaining > 0 {
            let t = c.i32()?;
            let range = if t < 0 {
                let len = t.unsigned_abs();
                let start = c.i32()?;
                IterationRange {
                    start: i64::from(start),
                    len,
                }
            } else {
                IterationRange {
                    start: i64::from(t),
                    len: 1,
                }
            };
            remaining -= i64::from(range.len);
            if remaining < 0 {
                return Err(RecordError::CountOverrun);
            }
            ranges.push(range);
        }
        let k = c.u32()?;
        if k > MAX_TRAILING_ENTRIES {
            return Err(RecordError::TooManyTrailing(k));
        }
        let mut trailing = Vec::with_capacity(k as usize);
        for _ in 0..k {
            let iteration = c.u32()?;
            let o = c.take(8)?;
            let mut opaque = [0u8; 8];
            opaque.copy_from_slice(o);
            trailing.push(TrailingEntry { iteration, opaque });
        }
        if c.remaining() != 0 {
            return Err(RecordError::LeftOver(c.remaining()));
        }
        Ok(Self {
            count: n.unsigned_abs(),
            ranges,
            trailing,
        })
    }

    /// `HI`: the highest iteration, or `None` when `N` = 0.
    pub fn highest(&self) -> Option<i64> {
        self.ranges.iter().map(IterationRange::last).max()
    }

    /// `RUNS`: the number of runs plus singles.
    pub fn runs(&self) -> usize {
        self.ranges.len()
    }
}

/// Decoded 0xFFFF0003 install version stamp (§9.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallStamp {
    /// Stamp S1.
    pub s1: Stamp,
    /// Stamp S2.
    pub s2: Stamp,
    /// Short ASCII label (a decimal string in the reference install).
    pub label: String,
}

impl InstallStamp {
    /// Decode the record.
    pub fn parse(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut c = Cursor { b: bytes, pos: 0 };
        let s1 = c.stamp()?;
        let s2 = c.stamp()?;
        let l = c.take(1)?[0];
        if l >= 0x80 {
            return Err(RecordError::BadLabel);
        }
        let label = c.take(usize::from(l))?;
        if !label.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            return Err(RecordError::BadLabel);
        }
        if c.remaining() != 0 {
            return Err(RecordError::LeftOver(c.remaining()));
        }
        Ok(Self {
            s1,
            s2,
            label: String::from_utf8_lossy(label).into_owned(),
        })
    }
}

/// Decoded 0xFFFF0005 split-set membership record (§9.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMembership {
    /// Stamp shared by all members of one split set.
    pub stamp: Stamp,
    /// Member index (0 for the base).
    pub index: u32,
    /// Number of members in the set.
    pub count: u32,
}

impl SetMembership {
    /// Decode the record. The spec gives a 24-byte layout; any extra bytes
    /// are ignored with a debug log.
    pub fn parse(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut c = Cursor { b: bytes, pos: 0 };
        let stamp = c.stamp()?;
        let index = c.u32()?;
        let count = c.u32()?;
        if c.remaining() != 0 {
            tracing::debug!(
                extra = c.remaining(),
                "set-membership record longer than expected"
            );
        }
        Ok(Self {
            stamp,
            index,
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &[i32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn iteration_two_runs() {
        // N = 10168: run 1..3630 (3630), run 13630..20167 (6538).
        let mut b = le(&[10168, -3630, 1, -6538, 13630]);
        b.extend(le(&[2]));
        b.extend(le(&[20166]));
        b.extend([1u8; 8]);
        b.extend(le(&[20167]));
        b.extend([2u8; 8]);
        let r = IterationRecord::parse(&b).unwrap();
        assert_eq!(r.count, 10168);
        assert_eq!(r.highest(), Some(20167));
        assert_eq!(r.runs(), 2);
        assert_eq!(r.trailing.len(), 2);
        assert_eq!(r.trailing[1].iteration, 20167);
    }

    #[test]
    fn iteration_singles_and_empty() {
        let b = le(&[3, 5, -2, 9, 0]);
        let r = IterationRecord::parse(&b).unwrap();
        assert_eq!(r.highest(), Some(10));
        assert_eq!(r.runs(), 2);
        let empty = IterationRecord::parse(&le(&[0, 0])).unwrap();
        assert_eq!(empty.highest(), None);
        assert_eq!(empty.count, 0);
    }

    #[test]
    fn iteration_malformed() {
        assert_eq!(
            IterationRecord::parse(&le(&[-1, 0])),
            Err(RecordError::NegativeCount(-1))
        );
        assert_eq!(
            IterationRecord::parse(&le(&[2, -3, 1, 0])),
            Err(RecordError::CountOverrun)
        );
        assert!(matches!(
            IterationRecord::parse(&le(&[2, 1])),
            Err(RecordError::Truncated { .. })
        ));
        assert_eq!(
            IterationRecord::parse(&le(&[0, 65])),
            Err(RecordError::TooManyTrailing(65))
        );
        assert_eq!(
            IterationRecord::parse(&le(&[0, 0, 0])),
            Err(RecordError::LeftOver(4))
        );
        // i32::MIN run length must not overflow.
        assert_eq!(
            IterationRecord::parse(&le(&[5, i32::MIN, 0, 0])),
            Err(RecordError::CountOverrun)
        );
    }

    #[test]
    fn install_stamp() {
        let mut b = vec![1u8; 16];
        b.extend([2u8; 16]);
        b.push(4);
        b.extend(b"5758");
        let s = InstallStamp::parse(&b).unwrap();
        assert_eq!(s.label, "5758");
        assert_eq!(stamp_hex(&s.s1), "01".repeat(16));
        b.push(0);
        assert_eq!(InstallStamp::parse(&b), Err(RecordError::LeftOver(1)));
        b.pop();
        b[32] = 0x80;
        assert_eq!(InstallStamp::parse(&b), Err(RecordError::BadLabel));
    }

    #[test]
    fn set_membership() {
        let mut b = vec![7u8; 16];
        b.extend(1u32.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        let m = SetMembership::parse(&b).unwrap();
        assert_eq!((m.index, m.count), (1, 3));
        assert!(SetMembership::parse(&b[..23]).is_err());
    }

    proptest::proptest! {
        #[test]
        fn parsers_never_panic(b in proptest::collection::vec(proptest::num::u8::ANY, 0..400)) {
            let _ = IterationRecord::parse(&b);
            let _ = InstallStamp::parse(&b);
            let _ = SetMembership::parse(&b);
        }
    }
}
