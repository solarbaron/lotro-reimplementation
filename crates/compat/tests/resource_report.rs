// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic structural vectors of the resource-set report `0x13D` (`official-session.md`
//! §9.3.12 "Synthetic structural vectors"): invented keys and masks, never captured ones.

use tessera_compat::resource_report::{ReportOmission, check_mounted};
use tessera_compat::world::{
    RESOURCE_SET_MAX_BITS, ResourceSetMask, ResourceSetReport, WireBitfield, WireReader, WorldError,
};

fn report(available: &[u32], loaded: &[u32]) -> ResourceSetReport {
    ResourceSetReport {
        available: ResourceSetMask::from_keys(available).expect("in range"),
        loaded: ResourceSetMask::from_keys(loaded).expect("in range"),
    }
}

/// Row 1: available keys 6 and 10, loaded key 6.
#[test]
fn keys_six_and_ten_then_six() {
    let data = [0x00, 0x0A, 0x20, 0x02, 0x06, 0x20];
    let r = report(&[6, 10], &[6]);
    assert_eq!(r.to_bytes().expect("encodes"), data);
    let back = ResourceSetReport::from_bytes(&data).expect("strict");
    assert_eq!(back, r);
    assert_eq!(back.available.bit_count(), 10);
    assert_eq!(back.loaded.bit_count(), 6);
    assert_eq!(back.available.keys(), [6, 10]);
    assert_eq!(back.loaded.keys(), [6]);
}

/// Row 2: available key 130 (a two-byte count, 17 packed bytes, bit 129 only), loaded empty.
#[test]
fn key_one_hundred_thirty_then_empty() {
    let mut data = vec![0x00, 0x80, 0x82];
    data.extend_from_slice(&[0u8; 16]);
    data.push(0x02);
    data.push(0x00);
    assert_eq!(data.len(), 21);
    let r = report(&[130], &[]);
    assert_eq!(r.to_bytes().expect("encodes"), data);
    let back = ResourceSetReport::from_bytes(&data).expect("strict");
    assert_eq!(back.available.keys(), [130]);
    assert_eq!(back.available.bytes().len(), 17);
    assert_eq!(back.loaded.bit_count(), 0);
    assert!(back.loaded.bytes().is_empty());
}

/// Row 3: both fields empty (a codec case only, never the item-13 candidate).
#[test]
fn both_empty() {
    let data = [0x00, 0x00, 0x00];
    assert_eq!(report(&[], &[]).to_bytes().expect("encodes"), data);
    let back = ResourceSetReport::from_bytes(&data).expect("strict");
    assert_eq!(back, ResourceSetReport::default());
}

/// Row 4: a stray high bit in the last byte above count 6 must fail; stale bits are never forwarded.
#[test]
fn a_stray_bit_above_the_count_fails() {
    assert!(matches!(
        ResourceSetReport::from_bytes(&[0x00, 0x06, 0xA0, 0x00]),
        Err(WorldError::ResourceSetField(_))
    ));
}

/// Remediation §31 fix 11: the strict unused-bit check is the `0x13D` codec's own (a self-check of
/// Tessera's encoding); the shared Type 22 decoder that reads the server's tables ignores unused
/// bits (`property-system.md`), so the same bytes decode there.
#[test]
fn the_shared_type_22_decoder_keeps_stray_bits() {
    let field = [0x06, 0xA0];
    let mut reader = WireReader::new(&field);
    let shared = WireBitfield::decode(&mut reader).expect("readers ignore unused bits");
    assert_eq!(shared.bit_count, 6);
    assert_eq!(shared.bytes, [0xA0]);
    let mut reader = WireReader::new(&field);
    assert!(matches!(
        ResourceSetMask::decode(&mut reader),
        Err(WorldError::ResourceSetField(_))
    ));
}

/// Row 5: key 4097 or 0 gives no report: never clamped.
#[test]
fn keys_outside_the_policy_range_give_no_report() {
    for key in [0, RESOURCE_SET_MAX_BITS + 1] {
        assert!(matches!(
            ResourceSetMask::singleton(key),
            Err(WorldError::ResourceSetField(_))
        ));
        assert_eq!(
            check_mounted(key, &[]),
            Err(ReportOmission::KeyOutOfRange),
            "{key}"
        );
    }
    let top = ResourceSetMask::singleton(RESOURCE_SET_MAX_BITS).expect("the cap itself");
    assert_eq!(top.bytes().len(), 512);
    let both = ResourceSetReport {
        available: top.clone(),
        loaded: top,
    };
    let data = both.to_bytes().expect("encodes");
    // §9.3.12: two maximum-sized fields plus the prefix are at most 1029 data bytes.
    assert!(data.len() <= 1029, "{}", data.len());
    assert_eq!(ResourceSetReport::from_bytes(&data).expect("strict"), both);
}

/// The strict decoder's other refusals: a count above the highest set bit, a count past the cap, a
/// count in a longer compact form than it needs, truncation and trailing bytes.
#[test]
fn the_decoder_refuses_every_non_minimal_form() {
    for data in [
        // Count 8 with only bit 5 set: not minimal.
        &[0x00, 0x08, 0x20, 0x00][..],
        // Count 4097.
        &[0x00, 0x90, 0x01][..],
        // Count 6 in the two-byte form.
        &[0x00, 0x80, 0x06, 0x20, 0x00][..],
        // Truncated.
        &[0x00, 0x0A, 0x20][..],
        // A trailing byte.
        &[0x00, 0x00, 0x00, 0x00][..],
        // A nonzero shared byte.
        &[0x01, 0x00, 0x00][..],
    ] {
        assert!(ResourceSetReport::from_bytes(data).is_err(), "{data:x?}");
    }
    // A single field read on its own.
    let mut r = WireReader::new(&[0x06, 0x20]);
    assert_eq!(ResourceSetMask::decode(&mut r).expect("strict").keys(), [6]);
}

/// The singleton candidate's form: the count is the key, one bit at key − 1.
#[test]
fn the_singleton_candidate_is_minimal() {
    for key in [1u32, 8, 9, 127, 128, 4096] {
        let mask = ResourceSetMask::singleton(key).expect("in range");
        assert_eq!(mask.bit_count(), key);
        assert_eq!(mask.bytes().len(), key.div_ceil(8) as usize);
        let set: u32 = mask.bytes().iter().map(|b| b.count_ones()).sum();
        assert_eq!(set, 1);
        assert_eq!(mask.keys(), [key]);
    }
}
