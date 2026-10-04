// SPDX-License-Identifier: GPL-3.0-or-later
//! Setup-family decoders (`docs/specs/setup.md` §4–§6) on arbitrary bytes: entity
//! descriptors (0x47, header alone and with a total synthetic schema), visual templates
//! (0x1F) and setups (0x04).
//!
//! Nothing may panic. Accepted records must survive an encode → decode → encode round trip
//! byte for byte, and their derived values (bounds, links) must be computable.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_setup -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{
    EntityDescriptor, EntityHeader, PropertyType, Record, Setup, VisualTemplate,
};

/// Every PID has a type, so random property sets reach the value decoders.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = Setup::parse(data) {
        let _ = s.first_unusual_field();
        if let Some(b) = s.volume_bounds() {
            assert!(
                b.is_valid(),
                "volume bounds must be finite and ordered: {b:?}"
            );
        }
        for b in &s.boxes {
            assert!(b.aabb().is_none_or(|a| a.is_valid()));
        }
        let _ = s.tagfile_has_magic();
        if let Ok(bytes) = s.to_bytes() {
            let again = Setup::parse(&bytes).expect("re-encoded setup decodes");
            assert_eq!(again.to_bytes().expect("re-encodes"), bytes);
            assert_eq!(again.parts.len(), s.parts.len());
        }
    }
    if let Ok(t) = VisualTemplate::parse(data) {
        let _ = t.first_unusual_field();
        let _ = t.setup();
        let bytes = t.to_bytes();
        let again = VisualTemplate::parse(&bytes).expect("re-encoded template decodes");
        assert_eq!(again.to_bytes(), bytes);
        assert_eq!(again.flags, t.flags);
    }
    if let Ok(h) = EntityHeader::parse(data) {
        let _ = h.template_link();
        if let Ok((e, trailing)) = EntityDescriptor::parse_prefix(data, &total) {
            // Field by field: the block's floats may be NaN.
            assert_eq!(
                (
                    e.header.flags,
                    e.header.did,
                    e.header.kind,
                    e.header.template
                ),
                (h.flags, h.did, h.kind, h.template)
            );
            assert_eq!(e.header.block.is_some(), h.block.is_some());
            assert!(trailing <= data.len());
            let _ = e.first_unusual_field();
            if !h.has_extra_sets() {
                assert!(e.extra_sets.is_empty());
            }
            if trailing == 0 {
                let pset = e
                    .property_set_bytes(data)
                    .expect("span of a decoded record");
                assert_eq!(e.to_bytes_with(pset).expect("re-encodes"), data);
            }
        }
    }
});
