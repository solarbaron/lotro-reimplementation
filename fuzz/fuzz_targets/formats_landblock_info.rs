// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-info decoding (`docs/specs/landblock.md` §4) with static entity descriptors
//! (`docs/specs/entity-descriptor.md` §3, §10) on arbitrary bytes, with a total synthetic
//! schema (every PID has a type chosen from the PID). The first byte selects the directory
//! stamp (0xFF = none); the rest is the payload.
//!
//! Nothing may panic. An accepted record honours its flags, the entity limit and the
//! placement rules: finite, bounded positions, unit rotations, sane scales and finite
//! transforms.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_landblock_info -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{
    MAX_PLACEMENT_COORD, MAX_SCALE, MAX_STATIC_ENTITIES, MIN_SCALE, SED_FLAGS_KNOWN,
};
use tessera_formats::{LandblockInfo, PropertyType, decode_landblock_info};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fuzz_target!(|input: &[u8]| {
    let Some((&s, data)) = input.split_first() else {
        return;
    };
    let stamp = (s != 0xFF).then_some(u16::from(s));
    let Ok(info) = decode_landblock_info(data, stamp, &total) else {
        return;
    };
    assert_eq!(info.flags & !7, 0);
    assert_eq!(info.cells.is_some(), info.flags & 1 != 0);
    assert_eq!(info.cross_links.is_some(), info.flags & 2 != 0);
    assert_eq!(info.header_indices.is_some(), info.flags & 4 != 0);
    assert!(info.entities.len() <= MAX_STATIC_ENTITIES as usize);
    for e in &info.entities {
        assert_eq!(e.flags & !SED_FLAGS_KNOWN, 0);
        assert_eq!(e.placement.is_some(), e.flags & 0x10 != 0);
        assert_eq!(e.properties.is_some(), e.flags & 0x80 != 0);
        // Any PID: `scale` must be sane whatever value it finds.
        let scale = e.scale(7);
        assert!(scale.iter().all(|c| (MIN_SCALE..=MAX_SCALE).contains(c)));
        if let Some(p) = e.placement {
            assert!(p.position.iter().all(|c| c.abs() <= MAX_PLACEMENT_COORD));
            assert!((p.rotation.length() - 1.0).abs() < 1e-5);
            let w = p.transform([1.0e6, -2.0, 3.0], scale);
            assert!(w.iter().all(|c| c.is_finite()), "{w:?}");
            let n = p.transform_normal([0.0, 0.6, 0.8], scale);
            assert!(n.iter().all(|c| c.is_finite()), "{n:?}");
            let _ = (p.is_off_unit(), p.rotation.heading_degrees());
        }
        let _ = e.kind();
        let _ = e.iid.map(|i| (i.region(), i.landblock(), i.serial()));
    }
    for c in info.cells() {
        assert_eq!(c.flags & !0x3C, 0);
        assert_eq!(c.index_echo.is_some(), c.flags & 0x20 != 0);
        let _ = (c.rotation(), c.position.offset());
    }
    let _ = info.entities_by_iid();
    let _ = info.first_unusual_field();
    let _ = LandblockInfo::parse(data, None, &total);
});
