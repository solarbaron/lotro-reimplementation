// SPDX-License-Identifier: GPL-3.0-or-later
//! Region descriptors (prefix 0x0F, `docs/specs/environment.md` §3.1–§3.2) on arbitrary
//! bytes. The first byte selects whether the rest is the payload as is, or follows a valid
//! synthetic 36-byte fixed part (own DID, region, map image, map parameters) so that short
//! inputs reach the nine DID maps and the trailing DID.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint, never longer
//! than the input), the typed accessors agree with the slot table, and the map-coordinate
//! helpers work on it.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_region_descriptor -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::RegionTable;
use tessera_formats::{Record, RegionDescriptor, decode_region_descriptor};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(36 + rest.len());
    for v in [0x0F00_0001u32, 2, 0x410D_9AE6, 2, 1468, 1244] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for f in [0.0f32, 0.0, 0.005] {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(d) = decode_region_descriptor(data) else {
        return;
    };
    let bytes = d.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = RegionDescriptor::parse(&bytes).expect("re-encoded descriptor decodes");
    // Compared as bytes: the f32 fields may be NaN.
    assert_eq!(again.to_bytes(), bytes);
    let typed = [
        d.scene_choice_table(),
        d.terrain_scene_table(),
        d.encounter_table(),
        d.sky_description(),
        d.water_table(),
        d.fog_zone_table(),
        d.block_map_descriptor(),
        d.terrain_type_table(),
    ];
    for ((t, did), typed) in d.tables().into_iter().zip(typed) {
        assert_eq!(did, typed, "slot {}", t.slot());
        assert_eq!(did, d.table(t));
        assert!(did.is_none_or(|x| !x.is_null()));
    }
    let _ = d.table_with_prefix(RegionTable::Water.prefix());
    let _ = d.first_unusual_field();
    let _ = d.map_origin_units();
    let _ = d.map_coordinates([254, 0], [160.0, f32::NAN]);
    for m in &d.maps {
        let _ = m.get(0);
    }
});
