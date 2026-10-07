// SPDX-License-Identifier: GPL-3.0-or-later
//! Version negotiation (`tessera-protocol.md` §3): majors by ALPN, minors in
//! `Hello`/`HelloAck`.

/// The ALPN protocol ID of a major version: `tessera/<major>` in decimal, no leading zeros.
#[must_use]
pub fn alpn_for_major(major: u16) -> Vec<u8> {
    format!("tessera/{major}").into_bytes()
}

/// The major version an ALPN protocol ID names, if it is a `tessera/<major>` ID in canonical
/// form (decimal, no leading zeros, no sign).
#[must_use]
pub fn major_from_alpn(alpn: &[u8]) -> Option<u16> {
    let digits = alpn.strip_prefix(b"tessera/")?;
    if digits.is_empty() || digits.len() > 5 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// The server's side of minor negotiation: min(client `minor_max`, `server_max`), or `None`
/// when that is below `server_min` (the server then sends
/// `Disconnect(UNSUPPORTED_VERSION)`).
#[must_use]
pub fn negotiate_minor(client_minor_max: u16, server_min: u16, server_max: u16) -> Option<u16> {
    let minor = client_minor_max.min(server_max);
    (minor >= server_min).then_some(minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpn() {
        assert_eq!(alpn_for_major(1), crate::limits::ALPN_V1);
        assert_eq!(major_from_alpn(b"tessera/1"), Some(1));
        assert_eq!(major_from_alpn(b"tessera/12"), Some(12));
        assert_eq!(major_from_alpn(b"tessera/01"), None);
        assert_eq!(major_from_alpn(b"tessera/"), None);
        assert_eq!(major_from_alpn(b"tessera/+1"), None);
        assert_eq!(major_from_alpn(b"tessera/70000"), None);
        assert_eq!(major_from_alpn(b"h3"), None);
    }

    #[test]
    fn minors() {
        assert_eq!(negotiate_minor(0, 0, 0), Some(0));
        assert_eq!(negotiate_minor(5, 0, 2), Some(2));
        assert_eq!(negotiate_minor(1, 2, 3), None);
    }
}
