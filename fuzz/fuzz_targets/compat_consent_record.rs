// SPDX-License-Identifier: GPL-3.0-or-later
//! The acceptance record's check (`docs/specs/official-entry.md` §5.6, §10.3): arbitrary
//! bytes as the record document and the key file, with arbitrary binding values. Nothing may
//! panic; allocation stays bounded by the input; a record is never accepted unless its
//! authentication code matches the key, the values and the binding. Half of the inputs build
//! a record with a correct code first (fuzz-chosen values), so the accepting path is reached
//! too.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_compat::consent::test_support::{
    RecordValues, binding, evaluate, record_mac, render_document,
};
use tessera_compat::consent::{KEY_BYTES, NotAcceptedReason, WarningText};

/// Splits `data` at the first `0xFF` bytes into up to `n` parts.
fn parts(data: &[u8], n: usize) -> Vec<&[u8]> {
    let mut out = Vec::with_capacity(n);
    let mut rest = data;
    while out.len() + 1 < n {
        match rest.iter().position(|b| *b == 0xFF) {
            Some(i) => {
                out.push(&rest[..i]);
                rest = &rest[i + 1..];
            }
            None => break,
        }
    }
    out.push(rest);
    while out.len() < n {
        out.push(&[]);
    }
    out
}

fn key_of(bytes: &[u8]) -> [u8; KEY_BYTES] {
    let mut k = [0u8; KEY_BYTES];
    for (i, b) in bytes.iter().take(KEY_BYTES).enumerate() {
        k[i] = *b;
    }
    k
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, data)) = data.split_first() else {
        return;
    };
    let digest = WarningText::shipped().digest();
    let p = parts(data, 6);
    let bound = binding(p[2], p[3], p[4]);
    if mode & 1 == 0 {
        // Arbitrary record and key.
        if let Ok(values) = evaluate(p[0], p[1], &bound, &digest) {
            let key: &[u8; KEY_BYTES] = p[1].try_into().expect("an accepted key has 32 bytes");
            let text = std::str::from_utf8(p[0]).expect("an accepted record is UTF-8");
            let mac = record_mac(key, &values, &bound);
            assert!(
                text.contains(&format!("mac = \"{mac}\"")),
                "accepted without the matching code"
            );
            assert_eq!(values.text_sha256(), digest.hex());
            assert_eq!(
                render_document(&values, &mac).as_bytes(),
                p[0],
                "only the exact document"
            );
        }
    } else {
        // A record with a correct code for fuzz-chosen values.
        let s = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        let revision = i64::from(u16::from_le_bytes([
            p[5].first().copied().unwrap_or(1),
            p[5].get(1).copied().unwrap_or(0),
        ]));
        let text_sha256 = if mode & 4 == 0 { digest.hex() } else { s(p[0]) };
        let accepted_at = if mode & 8 == 0 {
            "2026-10-23T12:00:00Z".to_owned()
        } else {
            s(p[1])
        };
        let values = RecordValues::new(
            i64::from((mode >> 1) & 1),
            revision,
            &s(p[5].get(2..6).unwrap_or(b"en")),
            &text_sha256,
            &accepted_at,
        );
        let key = key_of(p[1]);
        let mac = record_mac(&key, &values, &bound);
        let doc = render_document(&values, &mac);
        match evaluate(doc.as_bytes(), &key, &bound, &digest) {
            Ok(v) => {
                assert!(values.is_valid() && v == values);
                assert_eq!(values.text_sha256(), digest.hex());
            }
            Err(NotAcceptedReason::TextChanged) => {
                assert!(values.is_valid());
                assert_ne!(values.text_sha256(), digest.hex());
            }
            Err(_) => {}
        }
        // Another binding never accepts it.
        let other = binding(p[2], p[3], b"/elsewhere-not-the-same");
        if p[4] != b"/elsewhere-not-the-same" {
            assert!(evaluate(doc.as_bytes(), &key, &other, &digest).is_err());
        }
        // Another key never accepts it.
        let mut k2 = key;
        k2[0] ^= 1;
        assert!(evaluate(doc.as_bytes(), &k2, &bound, &digest).is_err());
    }
});
