// SPDX-License-Identifier: GPL-3.0-or-later
//! Every `tessera-formats` record decoder on arbitrary bytes: none may panic, and accepted
//! records must agree with their own headers.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_records -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{
    AnyRecord, DataIdMap, Did, EnumMapper, Evidence, NoSchema, PropertyDictionary, Record,
    SoundRecord, StringInfo, StringTable, WaveInfo,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(t) = StringTable::parse(data) {
        assert_eq!(t.len() as u64, u64::from(t.size.count));
        for e in &t {
            let _ = e.plain_text();
            let _ = e.arg_names_match_ids();
        }
    }
    let _ = StringInfo::parse(data);
    if let Ok(d) = PropertyDictionary::parse(data) {
        assert_eq!(d.names().len() as u64, u64::from(d.names_size.count));
        assert_eq!(
            d.definitions().len() as u64,
            u64::from(d.definitions_size.count)
        );
    }
    if let Ok(m) = EnumMapper::parse(data) {
        assert_eq!(m.names().len() as u64, u64::from(m.names_size.count));
    }
    if let Ok(m) = DataIdMap::parse(data) {
        for e in &m {
            assert!(m.get(e.key).is_some());
        }
    }
    let _ = WaveInfo::parse(data);
    if let Ok(s) = SoundRecord::parse(data) {
        let _ = s.container();
        let _ = WaveInfo::parse(s.audio);
    }
    // Dispatch on the DID in the first four bytes, as an archive reader would. Records
    // without a DID of their own (0x07, 0x0C, 0x08, 0x66) are reached through the last
    // byte's prefix, so their decoders see the whole input as well.
    if let Some(did) = data.get(..4) {
        let did = u32::from_le_bytes([did[0], did[1], did[2], did[3]]);
        if let Ok(Some(rec)) = AnyRecord::decode(did, data) {
            assert_eq!(u32::from(rec.did()), did);
            // Whatever the dispatcher decodes is classified as decoded by this crate.
            assert!(AnyRecord::is_decodable(did));
            assert_eq!(Did(did).kind().evidence(), Evidence::Decoded);
        }
        // The schema and stamp dispatchers, with the stamp taken from byte 4: absent, an
        // observed value on either side of the build-byte rule, or an unobserved one.
        let stamp = match data.get(4).map(|b| b % 4) {
            Some(0) | None => None,
            Some(1) => Some(33),
            Some(2) => Some(36),
            Some(_) => Some(38),
        };
        if let Ok(Some(rec)) = AnyRecord::decode_with_stamp(did, data, stamp, &NoSchema) {
            assert_eq!(u32::from(rec.did()), did);
            assert!(AnyRecord::is_decodable_with_schema(did));
            assert_eq!(Did(did).kind().evidence(), Evidence::Decoded);
        }
    }
    if let Some(&last) = data.last() {
        let did = u32::from(last) << 24 | 1;
        if let Ok(Some(rec)) = AnyRecord::decode_with_stamp(did, data, None, &NoSchema) {
            assert_eq!(u32::from(rec.did()), did);
        }
    }
});
