// SPDX-License-Identifier: GPL-3.0-or-later
//! Compressed record payloads (spec §6.1).

use flate2::{Decompress, FlushDecompress, Status};

use crate::bytes::u32_at;
use crate::error::{Error, Result};

/// Inflates a compressed record's stored payload.
///
/// `stored` is the reassembled payload of a record whose flag bit 0 is set: a `u32`
/// inflated length followed by one zlib stream (RFC 1950). The stream must produce exactly
/// the prefixed number of bytes, end (valid Adler-32 trailer), and be followed by no
/// further bytes (invariant I-23). The prefix must not exceed `max_inflated`, which bounds
/// the allocation; there is no limit on the compression ratio.
///
/// Errors are [`Error::Decompress`] (with `did` 0; [`crate::Archive`] fills in the DID) or
/// [`Error::TooLarge`].
pub fn inflate_record(stored: &[u8], max_inflated: u64) -> Result<Vec<u8>> {
    let fail = |reason: String| Error::Decompress { did: 0, reason };
    let Some(input) = stored.get(4..) else {
        return Err(fail(format!(
            "stored length {} is shorter than the 4-byte length prefix",
            stored.len()
        )));
    };
    let expected = u64::from(u32_at(stored, 0));
    if expected > max_inflated {
        return Err(Error::TooLarge {
            what: "inflated record",
            len: expected,
            limit: max_inflated,
        });
    }
    let expected = usize::try_from(expected).map_err(|_| Error::TooLarge {
        what: "inflated record",
        len: expected,
        limit: usize::MAX as u64,
    })?;

    let mut out = vec![0u8; expected];
    let mut probe = [0u8; 256];
    let mut z = Decompress::new(true);
    loop {
        let in_pos = usize::try_from(z.total_in()).unwrap_or(usize::MAX);
        let out_pos = usize::try_from(z.total_out()).unwrap_or(usize::MAX);
        let src = input.get(in_pos..).unwrap_or(&[]);
        let full = out_pos >= expected;
        let dst: &mut [u8] = if full {
            &mut probe
        } else {
            out.get_mut(out_pos..).unwrap_or(&mut [])
        };
        let status = z
            .decompress(src, dst, FlushDecompress::Finish)
            .map_err(|e| fail(format!("zlib error: {e}")))?;
        let new_out = usize::try_from(z.total_out()).unwrap_or(usize::MAX);
        if full && new_out > out_pos {
            return Err(fail(format!(
                "stream inflates to more than the prefixed {expected} bytes"
            )));
        }
        if status == Status::StreamEnd {
            break;
        }
        let new_in = usize::try_from(z.total_in()).unwrap_or(usize::MAX);
        if new_in == in_pos && new_out == out_pos {
            return Err(fail(format!(
                "stream is truncated: {new_out} of {expected} bytes produced, input exhausted \
                 after {new_in} of {} bytes",
                input.len()
            )));
        }
    }
    let produced = usize::try_from(z.total_out()).unwrap_or(usize::MAX);
    if produced != expected {
        return Err(fail(format!(
            "stream inflated to {produced} bytes but the prefix says {expected}"
        )));
    }
    let consumed = usize::try_from(z.total_in()).unwrap_or(usize::MAX);
    if consumed != input.len() {
        return Err(fail(format!(
            "{} trailing bytes after the end of the zlib stream",
            input.len().saturating_sub(consumed)
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn pack(prefix: u32, raw: &[u8], level: Compression) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), level);
        enc.write_all(raw).unwrap();
        let z = enc.finish().unwrap();
        let mut out = prefix.to_le_bytes().to_vec();
        out.extend_from_slice(&z);
        out
    }

    #[test]
    fn roundtrip_both_levels() {
        let raw: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
        for level in [Compression::default(), Compression::best()] {
            let s = pack(raw.len() as u32, &raw, level);
            assert_eq!(inflate_record(&s, 1 << 20).unwrap(), raw);
        }
    }

    #[test]
    fn empty_payload() {
        let s = pack(0, &[], Compression::default());
        assert_eq!(inflate_record(&s, 16).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn high_ratio_is_allowed() {
        let raw = vec![0u8; 4 << 20];
        let s = pack(raw.len() as u32, &raw, Compression::best());
        assert!(raw.len() / s.len() > 1000);
        assert_eq!(inflate_record(&s, 64 << 20).unwrap().len(), raw.len());
    }

    #[test]
    fn rejects_mismatches() {
        let raw = vec![7u8; 1000];
        // Prefix too large: stream ends early.
        assert!(inflate_record(&pack(1001, &raw, Compression::default()), 1 << 20).is_err());
        // Prefix too small: stream wants to produce more.
        assert!(inflate_record(&pack(999, &raw, Compression::default()), 1 << 20).is_err());
        // Trailing garbage.
        let mut s = pack(1000, &raw, Compression::default());
        s.push(0);
        assert!(inflate_record(&s, 1 << 20).is_err());
        // Truncated stream.
        let s = pack(1000, &raw, Compression::default());
        assert!(inflate_record(&s[..s.len() - 3], 1 << 20).is_err());
        // Cap.
        assert!(matches!(
            inflate_record(&pack(1000, &raw, Compression::default()), 999),
            Err(Error::TooLarge { .. })
        ));
        // Garbage and short input.
        assert!(inflate_record(&[0x10, 0, 0, 0, 1, 2, 3, 4, 5], 1 << 20).is_err());
        assert!(inflate_record(&[1, 2], 1 << 20).is_err());
    }
}
