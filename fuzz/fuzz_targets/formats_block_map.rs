// SPDX-License-Identifier: GPL-3.0-or-later
//! Block maps and block-map descriptors (`docs/specs/landblock.md` §5) on arbitrary bytes.
//! (Region descriptors, which name the block-map descriptor, have their own target,
//! `formats_region_descriptor`.)
//! Descriptors are decoded with a total synthetic schema; the input is also split in two
//! and the halves resolved against each other.
//!
//! Nothing may panic. Accepted block maps re-encode canonically; resolution yields one value
//! per block-map entry.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_block_map -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{
    BlockMap, BlockMapDescriptor, PropertyType, Record, RegionDescriptor, decode_block_map,
    decode_block_map_descriptor,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn map(data: &[u8]) -> Option<BlockMap> {
    let m = decode_block_map(data).ok()?;
    // Canonical re-encoding (shortest VLE count): a fixpoint, never longer than the input.
    let bytes = m.to_bytes();
    assert!(bytes.len() <= data.len());
    assert_eq!(
        BlockMap::parse(&bytes)
            .expect("re-encoded map decodes")
            .to_bytes(),
        bytes
    );
    let _ = m.first_unusual_field();
    Some(m)
}

fuzz_target!(|data: &[u8]| {
    let m = map(data);
    let d = decode_block_map_descriptor(data, &total).ok();
    if let Some(d) = &d {
        let _ = d.first_unusual_field();
    }
    if let Ok(r) = RegionDescriptor::parse(data) {
        // Also reached here so that block-map inputs exercise the lookup of their descriptor.
        let _ = r.block_map_descriptor();
    }
    // Resolve a map against a descriptor taken from the two halves of the input.
    if data.len() >= 2 {
        let cut = usize::from(data[0]).min(data.len() - 1) + 1;
        let (a, b) = data.split_at(cut);
        if let (Ok(desc), Some(m)) = (BlockMapDescriptor::parse(a, &total), map(b)) {
            let r = desc.resolve(&m);
            assert_eq!(r.len(), m.entries.len());
            for e in &desc.entries {
                let _ = desc.value_of(&m, e.pid);
            }
        }
    }
    let _ = m;
});
