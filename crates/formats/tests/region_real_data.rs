// SPDX-License-Identifier: GPL-3.0-or-later
//! Region descriptors (prefix 0x0F, `docs/specs/environment.md` §3.1–§3.2) against a user's
//! own install (read-only).
//!
//! Every 0x0F record of every archive is decoded with the grammar of §3.1 and must:
//! re-encode byte for byte, show the values observed in every record (one key-0 entry in
//! maps 1–7, maps 8–9 empty, every slot naming a DID with its table's prefix), name eight
//! tables that all exist in the install, and agree with the REGION DID map (root label
//! `REGION`, `records.md` §8) on its region number. Every region whose cell archive holds
//! landblock data has a descriptor. The block-map descriptor of slot 7 is the one the prefix
//! rule of `landblock.md` §9.1 finds. Counts are printed, not asserted (`landblock.md` §0:
//! counts are observations of one install; environment.md §3 observed seven descriptors).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test region_real_data -- --nocapture`.
//! Only counts, DIDs and region numbers are printed; no game data leaves the install.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::landblock::{REGION_DESCRIPTOR_PREFIX, RegionTable};
use tessera_formats::{DataIdMap, Did, Record, RegionDescriptor};

/// The root DID map (`records.md` §8).
const ROOT_DID_MAP: u32 = 0x2800_0000;

fn open_set() -> Option<ArchiveSet> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
}

/// Every region descriptor of the install with its stored bytes.
fn descriptors(set: &ArchiveSet) -> BTreeMap<u32, (RegionDescriptor, Vec<u8>)> {
    let prefix = u32::from(REGION_DESCRIPTOR_PREFIX) << 24;
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(prefix..=prefix | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            let payload = m.archive.read_entry(&e).expect("read region descriptor");
            let d = RegionDescriptor::parse_expecting(&payload, e.did)
                .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
            out.insert(e.did, (d, payload));
        }
    }
    out
}

/// The REGION DID map: region number → region-descriptor DID.
fn region_map(set: &ArchiveSet) -> BTreeMap<u32, Did> {
    let root = set.read(ROOT_DID_MAP).expect("read root DID map");
    let root = DataIdMap::parse_expecting(&root, ROOT_DID_MAP).expect("root DID map");
    let did = root
        .by_label("REGION")
        .expect("REGION entry in the root map");
    let bytes = set.read(did.0).expect("read REGION DID map");
    let map = DataIdMap::parse_expecting(&bytes, did).expect("REGION DID map");
    map.iter().map(|e| (e.key, e.value)).collect()
}

#[test]
fn region_descriptors_decode_with_all_eight_tables() {
    let Some(set) = open_set() else { return };
    let all = descriptors(&set);
    // environment.md §3 observed seven; the count is printed, not asserted.
    eprintln!("region descriptors: {} decoded", all.len());
    assert!(!all.is_empty(), "no region descriptor");
    for (did, (d, payload)) in &all {
        let did = Did(*did);
        assert_eq!(d.first_unusual_field(), None, "{did}");
        assert_eq!(&d.to_bytes(), payload, "{did}: re-encoding");
        let tables = d.tables();
        for (t, table) in tables {
            let table = table.unwrap_or_else(|| panic!("{did}: slot {} empty", t.slot()));
            assert_eq!(table.prefix(), t.prefix(), "{did}: slot {}", t.slot());
            assert!(
                set.read(table.0).is_ok(),
                "{did}: {} {table} missing",
                t.name()
            );
        }
        assert_eq!(
            d.block_map_descriptor(),
            d.table_with_prefix(RegionTable::BlockMapDescriptor.prefix()),
            "{did}: slot 7 and the prefix rule disagree"
        );
        assert!(d.map_scale.is_finite() && d.map_scale > 0.0);
        assert_eq!(d.map_image.prefix(), 0x41, "{did}: map image");
        eprintln!(
            "  {did}: region {:>2}, {} bytes, tables {:?}",
            d.region,
            payload.len(),
            tables.map(|(_, t)| format!("{:#010x}", t.map_or(0, |t| t.0)))
        );
    }
    // The REGION DID map names each descriptor under its own region number.
    let map = region_map(&set);
    let mut matched = 0usize;
    for (region, did) in &map {
        match all.get(&did.0) {
            Some((d, _)) => {
                assert_eq!(d.region, *region, "{did}: region number");
                matched += 1;
            }
            None => eprintln!("  REGION map: region {region} → {did} absent"),
        }
    }
    eprintln!(
        "REGION map: {} entries, {matched} with a descriptor",
        map.len()
    );
    assert_eq!(matched, all.len(), "every descriptor is named by the map");
    // Every region with landblock data (kind 0, `landblock.md` §2) in its cell archive has
    // a descriptor.
    let described: std::collections::BTreeSet<u32> = all.values().map(|(d, _)| d.region).collect();
    let mut with_data = std::collections::BTreeSet::new();
    for m in set.members() {
        let ArchiveKind::Cell(r) = m.id.kind else {
            continue;
        };
        let lo = 0x8000_0000 | ((r & 0xF) << 16);
        if let Some(e) = m.archive.entries_range(lo..=lo | 0xFFFF).next() {
            e.expect("directory walk");
            with_data.insert(r);
        }
    }
    eprintln!("regions with landblock data: {with_data:?}");
    assert!(
        !with_data.is_empty(),
        "no landblock data in any cell archive"
    );
    for r in &with_data {
        assert!(
            described.contains(r),
            "region {r} has landblock data but no descriptor"
        );
    }
}
