// SPDX-License-Identifier: GPL-3.0-or-later
//! The protocol's small byte readers that no other target reaches on their own
//! (`tessera-protocol.md` §2.1, §3, §4.3, §5.4; `test-server.md` §3.3): the server key file
//! (`ServerIdentity::from_pkcs8_pem`, `from_pkcs8_der`; an operator can hand-edit it), the
//! Ed25519 SPKI reader (`ed25519_key_from_spki`), the ALPN major reader
//! (`major_from_alpn`) and the frame length check (`parse_frame_length`).
//!
//! Byte 0 selects the mode, the rest is the input:
//! - 0: the rest as PEM text (lossy UTF-8);
//! - 1: the rest as PKCS#8 DER;
//! - 2: a valid key file (a fixed synthetic throwaway key, the same in every process so a
//!   saved input reproduces) with bytes of the input spliced in at input-chosen places, so
//!   runs reach the base64 and DER checks past the frame;
//! - 3: the rest as an SPKI, and as an ALPN protocol ID;
//! - otherwise: the first four bytes as a frame length field.
//!
//! Nothing may panic. A key that loads gives a certificate whose fingerprint (§4.3 steps
//! 2–3) is the identity's own and whose SPKI holds the identity's public key, and its key
//! file reads back to the same identity. A major read from an ALPN ID prints back to the
//! same ID; an accepted frame length lies in 2–65,536.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_protocol::fingerprint::{
    certificate_fingerprint, certificate_spki, ed25519_key_from_spki,
};
use tessera_protocol::frame::parse_frame_length;
use tessera_protocol::limits::{FRAME_LENGTH_FIELD, MAX_FRAME_LENGTH, MIN_FRAME_LENGTH};
use tessera_protocol::transport::ServerIdentity;
use tessera_protocol::version::{alpn_for_major, major_from_alpn};

/// The PKCS#8 v1 prefix of an Ed25519 private key (RFC 8410 §7: SEQUENCE, version 0, the
/// id-Ed25519 algorithm identifier, then the OCTET STRING that wraps the 32-byte seed).
const PKCS8_V1_ED25519_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// A valid key file, built once from a fixed synthetic seed (bytes 0..32), so the base of
/// mode 2 is the same in every process and a saved input reproduces. This is a throwaway
/// test key made by this harness, never a server's key; nothing is written.
fn valid_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| {
        let mut der = PKCS8_V1_ED25519_PREFIX.to_vec();
        der.extend(0u8..32);
        ServerIdentity::from_pkcs8_der(&der)
            .map(|id| id.to_pkcs8_pem())
            .unwrap_or_default()
    })
}

/// The invariants of an identity that loaded.
fn check_identity(id: &ServerIdentity) {
    let cert = id.certificate_der();
    let fp = certificate_fingerprint(cert).expect("own certificate has a fingerprint");
    assert_eq!(fp, id.fingerprint(), "certificate fingerprint differs");
    let spki = certificate_spki(cert).expect("own certificate has an SPKI");
    let key = ed25519_key_from_spki(spki).expect("own SPKI is Ed25519");
    assert_eq!(
        &key,
        id.public_key(),
        "SPKI key differs from the identity's"
    );
    let again = ServerIdentity::from_pkcs8_pem(&id.to_pkcs8_pem()).expect("own key file reloads");
    assert_eq!(
        again.public_key(),
        id.public_key(),
        "key file round trip changed the key"
    );
}

fn pem(text: &str) {
    if let Ok(id) = ServerIdentity::from_pkcs8_pem(text) {
        check_identity(&id);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode {
        0 => pem(&String::from_utf8_lossy(rest)),
        1 => {
            if let Ok(id) = ServerIdentity::from_pkcs8_der(rest) {
                check_identity(&id);
            }
        }
        2 => {
            let base = valid_pem().as_bytes();
            // The fixed synthetic key must load, or mode 2 would silently test nothing.
            assert!(!base.is_empty(), "the fixed synthetic key does not load");
            let mut bytes = base.to_vec();
            for &[lo, hi, b] in rest.as_chunks::<3>().0.iter().take(8) {
                let at = usize::from(u16::from_le_bytes([lo, hi])) % bytes.len();
                bytes[at] = b;
            }
            pem(&String::from_utf8_lossy(&bytes));
        }
        3 => {
            if let Ok(key) = ed25519_key_from_spki(rest) {
                assert_eq!(rest.len(), 44, "an Ed25519 SPKI is 44 bytes");
                assert_eq!(rest.get(12..), Some(&key[..]), "the key is the SPKI's tail");
            }
            if let Some(major) = major_from_alpn(rest) {
                assert_eq!(alpn_for_major(major), rest, "ALPN ID not in canonical form");
            }
        }
        _ => {
            let mut field = [0u8; FRAME_LENGTH_FIELD];
            for (d, s) in field.iter_mut().zip(rest) {
                *d = *s;
            }
            if let Ok(n) = parse_frame_length(field) {
                let range = MIN_FRAME_LENGTH as usize..=MAX_FRAME_LENGTH as usize;
                assert!(range.contains(&n), "frame length {n} accepted");
                assert_eq!(u32::from_le_bytes(field) as usize, n);
            }
        }
    }
});
