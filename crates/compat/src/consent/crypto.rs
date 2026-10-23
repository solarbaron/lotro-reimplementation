// SPDX-License-Identifier: GPL-3.0-or-later
//! The byte strings of `docs/specs/official-entry.md` §5.3 and §5.6: length-prefixed items,
//! SHA-256, HMAC-SHA-256 (RFC 2104 over SHA-256), lowercase hexadecimal and a constant-time
//! comparison.

use sha2::{Digest as _, Sha256};

/// SHA-256 block size in bytes (RFC 2104 *B*).
const BLOCK: usize = 64;

/// A byte string of length-prefixed items (§5.3): each item is preceded by its length in
/// bytes as a 4-byte little-endian unsigned number.
#[derive(Debug, Default)]
pub(crate) struct Items(Vec<u8>);

impl Items {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Appends one item. Items longer than `u32::MAX` bytes cannot occur (every input is
    /// bounded far below it); such an item would be refused by every check, since its
    /// prefix saturates and no stored value can match it.
    pub(crate) fn push(&mut self, item: &[u8]) -> &mut Self {
        let len = u32::try_from(item.len()).unwrap_or(u32::MAX);
        self.0.extend_from_slice(&len.to_le_bytes());
        self.0.extend_from_slice(item);
        self
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// SHA-256 of `data`.
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// HMAC-SHA-256 of `message` under `key` (RFC 2104).
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let h = sha256(key);
        k[..32].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    let mut outer = Sha256::new();
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for (i, b) in k.iter().enumerate() {
        ipad[i] = b ^ 0x36;
        opad[i] = b ^ 0x5c;
    }
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();
    outer.update(opad);
    outer.update(inner_hash);
    // Best effort: the padded key copies do not outlive the call.
    k.fill(0);
    ipad.fill(0);
    opad.fill(0);
    std::hint::black_box(&k);
    outer.finalize().into()
}

/// Lowercase hexadecimal of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from(DIGITS[usize::from(b >> 4)]));
        s.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    s
}

/// The 32 bytes of exactly 64 lowercase hexadecimal digits; `None` for anything else.
pub(crate) fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        let hi = digit(*b.get(2 * i)?)?;
        let lo = digit(*b.get(2 * i + 1)?)?;
        *o = (hi << 4) | lo;
    }
    Some(out)
}

/// Equality of two 32-byte values in time independent of where they differ.
pub(crate) fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        // RFC 4231 test case 2: key "Jefe", data "what do ya want for nothing?".
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_matches_rfc4231_case_6_long_key() {
        let key = [0xaa_u8; 131];
        let mac = hmac_sha256(
            &key,
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex(&mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn hex_round_trips_and_rejects_upper_case() {
        let v = sha256(b"abc");
        let h = hex(&v);
        assert_eq!(parse_hex32(&h), Some(v));
        assert_eq!(parse_hex32(&h.to_uppercase()), None);
        assert_eq!(parse_hex32(&h[..63]), None);
        assert!(ct_eq(&v, &v));
        let mut w = v;
        w[31] ^= 1;
        assert!(!ct_eq(&v, &w));
    }

    #[test]
    fn items_are_length_prefixed() {
        let mut i = Items::new();
        i.push(b"ab").push(b"");
        assert_eq!(i.bytes(), &[2, 0, 0, 0, b'a', b'b', 0, 0, 0, 0]);
    }
}
