// SPDX-License-Identifier: GPL-3.0-or-later
//! Server key fingerprints (`tessera-protocol.md` §4.2) and the certificate check of §4.3
//! steps 1–3.
//!
//! The certificate walk here is a minimal, bounds-checked DER reader that only finds the
//! `subjectPublicKeyInfo` of an X.509 v3 certificate. Names, validity dates and extensions are
//! ignored on purpose (§4.3 step 5); the TLS layer verifies the handshake signature.

use std::fmt;

use sha2::{Digest, Sha256};

/// The 12-byte DER prefix of an Ed25519 SubjectPublicKeyInfo (RFC 8410): SEQUENCE(42) {
/// SEQUENCE(5) { OID 1.3.101.112 }, BIT STRING(33) with 0 unused bits }.
pub const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];
/// Length of an Ed25519 SubjectPublicKeyInfo.
pub const ED25519_SPKI_LEN: usize = 44;
/// The text prefix of a fingerprint.
pub const FINGERPRINT_PREFIX: &str = "sha256:";
/// Length of a fingerprint's text form.
pub const FINGERPRINT_TEXT_LEN: usize = 71;

/// A SHA-256 fingerprint of a server's SubjectPublicKeyInfo (§4.2). `Display` writes the
/// canonical text `sha256:` + 64 lowercase hex digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint(pub [u8; 32]);

/// Why a fingerprint text is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FingerprintError {
    /// Not exactly 71 characters.
    #[error("fingerprint must be 71 characters, found {0}")]
    Length(usize),
    /// The prefix is not exactly `sha256:`.
    #[error("fingerprint must start with \"sha256:\"")]
    Prefix,
    /// A character that is not a hexadecimal digit.
    #[error("fingerprint holds a character that is not a hex digit")]
    Digit,
}

impl Fingerprint {
    /// The fingerprint of SubjectPublicKeyInfo DER bytes: their SHA-256 digest.
    #[must_use]
    pub fn of_spki(spki_der: &[u8]) -> Self {
        Self(Sha256::digest(spki_der).into())
    }

    /// The fingerprint of a raw Ed25519 public key (via its 44-byte SPKI).
    #[must_use]
    pub fn of_ed25519_key(public_key: &[u8; 32]) -> Self {
        Self::of_spki(&ed25519_spki(public_key))
    }

    /// Parses the text form strictly (§4.2): exactly `sha256:` then 64 hex digits of either
    /// case. No surrounding whitespace or separators.
    pub fn parse(s: &str) -> Result<Self, FingerprintError> {
        if s.len() != FINGERPRINT_TEXT_LEN {
            return Err(FingerprintError::Length(s.len()));
        }
        let Some(hex) = s.strip_prefix(FINGERPRINT_PREFIX) else {
            return Err(FingerprintError::Prefix);
        };
        let hex = hex.as_bytes();
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            let hi = hex_value(hex.get(2 * i).copied())?;
            let lo = hex_value(hex.get(2 * i + 1).copied())?;
            *byte = hi << 4 | lo;
        }
        Ok(Self(out))
    }

    /// The digest bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// A form for people to read: the canonical text with the hex digits in groups of four
    /// separated by spaces. Never valid in connect strings or the known-servers file.
    #[must_use]
    pub fn to_grouped(&self) -> String {
        let text = self.to_string();
        let hex = text.get(FINGERPRINT_PREFIX.len()..).unwrap_or_default();
        let groups: Vec<&str> = hex
            .as_bytes()
            .chunks(4)
            .filter_map(|c| std::str::from_utf8(c).ok())
            .collect();
        format!("{FINGERPRINT_PREFIX}{}", groups.join(" "))
    }
}

fn hex_value(c: Option<u8>) -> Result<u8, FingerprintError> {
    match c {
        Some(c @ b'0'..=b'9') => Ok(c - b'0'),
        Some(c @ b'a'..=b'f') => Ok(c - b'a' + 10),
        Some(c @ b'A'..=b'F') => Ok(c - b'A' + 10),
        _ => Err(FingerprintError::Digit),
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(FINGERPRINT_PREFIX)?;
        for b in &self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

impl std::str::FromStr for Fingerprint {
    type Err = FingerprintError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// The 44-byte SubjectPublicKeyInfo DER of an Ed25519 public key.
#[must_use]
pub fn ed25519_spki(public_key: &[u8; 32]) -> [u8; ED25519_SPKI_LEN] {
    let mut out = [0u8; ED25519_SPKI_LEN];
    out[..12].copy_from_slice(&ED25519_SPKI_PREFIX);
    out[12..].copy_from_slice(public_key);
    out
}

/// Why a server certificate is refused (§4.3 step 2; the client reports `TLS_FAILURE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CertificateError {
    /// The DER structure is malformed or truncated.
    #[error("certificate DER is malformed: {0}")]
    Malformed(&'static str),
    /// The certificate is not X.509 version 3.
    #[error("certificate is not X.509 v3")]
    NotV3,
    /// The key is not Ed25519 (OID 1.3.101.112 with a 32-byte key).
    #[error("server key is not Ed25519")]
    NotEd25519,
}

/// One DER element: tag, full encoding (header and content), content.
struct Tlv<'a> {
    tag: u8,
    full: &'a [u8],
    content: &'a [u8],
}

/// Reads one DER element from the front of `buf`; returns it and the rest.
fn read_tlv(buf: &[u8]) -> Result<(Tlv<'_>, &[u8]), CertificateError> {
    let malformed = CertificateError::Malformed;
    let (&tag, rest) = buf.split_first().ok_or(malformed("truncated tag"))?;
    if tag & 0x1f == 0x1f {
        return Err(malformed("multi-byte tag"));
    }
    let (&first, mut rest) = rest.split_first().ok_or(malformed("truncated length"))?;
    let len = if first < 0x80 {
        usize::from(first)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 {
            return Err(malformed("unsupported length form"));
        }
        let bytes = rest.get(..n).ok_or(malformed("truncated length"))?;
        rest = rest.get(n..).unwrap_or_default();
        bytes
            .iter()
            .fold(0usize, |acc, b| acc << 8 | usize::from(*b))
    };
    let header = buf.len() - rest.len();
    let content = rest.get(..len).ok_or(malformed("truncated content"))?;
    let full = buf
        .get(..header + len)
        .ok_or(malformed("truncated content"))?;
    let after = rest.get(len..).unwrap_or_default();
    Ok((Tlv { tag, full, content }, after))
}

const SEQUENCE: u8 = 0x30;
const INTEGER: u8 = 0x02;
const VERSION_TAG: u8 = 0xa0;

/// The DER bytes of the `subjectPublicKeyInfo` element of an X.509 v3 certificate, exactly as
/// they occur in it (§4.2 input). Works on hostile input without panicking.
pub fn certificate_spki(cert_der: &[u8]) -> Result<&[u8], CertificateError> {
    let malformed = CertificateError::Malformed;
    let (cert, trailing) = read_tlv(cert_der)?;
    if cert.tag != SEQUENCE || !trailing.is_empty() {
        return Err(malformed("certificate is not one SEQUENCE"));
    }
    let (tbs, _) = read_tlv(cert.content)?;
    if tbs.tag != SEQUENCE {
        return Err(malformed("tbsCertificate is not a SEQUENCE"));
    }
    let mut rest = tbs.content;
    // version [0] EXPLICIT INTEGER: must be present and 2 (v3).
    let (version, r) = read_tlv(rest)?;
    if version.tag != VERSION_TAG {
        return Err(CertificateError::NotV3);
    }
    let (v, _) = read_tlv(version.content)?;
    if v.tag != INTEGER || v.content != [2] {
        return Err(CertificateError::NotV3);
    }
    rest = r;
    // serialNumber, signature, issuer, validity, subject.
    let (serial, r) = read_tlv(rest)?;
    if serial.tag != INTEGER {
        return Err(malformed("serialNumber is not an INTEGER"));
    }
    rest = r;
    for what in ["signature", "issuer", "validity", "subject"] {
        let (el, r) = read_tlv(rest)?;
        if el.tag != SEQUENCE {
            return Err(malformed(match what {
                "signature" => "signature is not a SEQUENCE",
                "issuer" => "issuer is not a SEQUENCE",
                "validity" => "validity is not a SEQUENCE",
                _ => "subject is not a SEQUENCE",
            }));
        }
        rest = r;
    }
    let (spki, _) = read_tlv(rest)?;
    if spki.tag != SEQUENCE {
        return Err(malformed("subjectPublicKeyInfo is not a SEQUENCE"));
    }
    Ok(spki.full)
}

/// The raw Ed25519 public key of an SPKI, or `NotEd25519`.
pub fn ed25519_key_from_spki(spki_der: &[u8]) -> Result<[u8; 32], CertificateError> {
    if spki_der.len() != ED25519_SPKI_LEN || spki_der[..12] != ED25519_SPKI_PREFIX {
        return Err(CertificateError::NotEd25519);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&spki_der[12..]);
    Ok(key)
}

/// §4.3 steps 2–3 without the comparison: the fingerprint of an end-entity certificate whose
/// key must be Ed25519.
pub fn certificate_fingerprint(cert_der: &[u8]) -> Result<Fingerprint, CertificateError> {
    let spki = certificate_spki(cert_der)?;
    ed25519_key_from_spki(spki)?;
    Ok(Fingerprint::of_spki(spki))
}

/// Outcome of comparing a presented fingerprint with the expected one (§4.3 step 3, §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCheck {
    /// The fingerprint matches.
    Match,
    /// It differs: `KEY_MISMATCH`.
    Mismatch,
    /// Nothing was expected: trust on first use, or `KEY_UNKNOWN`.
    Unknown,
}

/// Compares `presented` with `expected`.
#[must_use]
pub fn check_pin(expected: Option<&Fingerprint>, presented: &Fingerprint) -> PinCheck {
    match expected {
        None => PinCheck::Unknown,
        Some(e) if e == presented => PinCheck::Match,
        Some(_) => PinCheck::Mismatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ];
    const TEXT: &str = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";

    #[test]
    fn vector_16_1() {
        let fp = Fingerprint::of_ed25519_key(&KEY);
        assert_eq!(fp.to_string(), TEXT);
        assert_eq!(Fingerprint::parse(TEXT).unwrap(), fp);
        assert_eq!(
            Fingerprint::parse(&TEXT.to_uppercase().replace("SHA256", "sha256")).unwrap(),
            fp
        );
    }

    #[test]
    fn parse_rejects() {
        assert_eq!(
            Fingerprint::parse(&TEXT.replace("sha256", "SHA256")),
            Err(FingerprintError::Prefix)
        );
        assert_eq!(
            Fingerprint::parse(&format!(" {}", &TEXT[1..])),
            Err(FingerprintError::Prefix)
        );
        assert_eq!(
            Fingerprint::parse(&format!("{TEXT} ")),
            Err(FingerprintError::Length(72))
        );
        assert_eq!(
            Fingerprint::parse(&TEXT.replace('f', "g")),
            Err(FingerprintError::Digit)
        );
        // Multi-byte characters keep the byte length but are not hex digits.
        let mut s = TEXT[..69].to_owned();
        s.push('é');
        assert_eq!(Fingerprint::parse(&s), Err(FingerprintError::Digit));
        assert!(Fingerprint::parse("").is_err());
    }

    #[test]
    fn grouped() {
        let fp = Fingerprint::parse(TEXT).unwrap();
        assert!(fp.to_grouped().starts_with("sha256:06e3 fd8f "));
        assert!(Fingerprint::parse(&fp.to_grouped()).is_err());
    }

    #[test]
    fn der_walker_rejects_garbage() {
        assert!(certificate_spki(&[]).is_err());
        assert!(certificate_spki(&[0x30, 0x84, 0xff, 0xff, 0xff, 0xff]).is_err());
        assert!(certificate_spki(&[0x30, 0x00]).is_err());
        assert_eq!(
            ed25519_key_from_spki(&[0u8; 44]),
            Err(CertificateError::NotEd25519)
        );
        assert_eq!(ed25519_key_from_spki(&ed25519_spki(&KEY)).unwrap(), KEY);
    }
}
