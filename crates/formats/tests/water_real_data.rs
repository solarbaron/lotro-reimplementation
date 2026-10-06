// SPDX-License-Identifier: GPL-3.0-or-later
//! Water tables (prefix 0x16, `docs/specs/water.md` §4) and fog-zone tables (prefix 0x17,
//! §5) against a user's own install (read-only).
//!
//! Every record of each prefix must decode to its last byte and re-encode byte for byte.
//! Record-level invariants of `water.md` §11 asserted: WA1 (every 0x16 record parses
//! exactly; `has_default` and `dry` 0 or 1, enforced by the decoder), WA2 (every 0x17 record
//! parses exactly with the newest layout), WA3 (every non-dry entry's `surface` is an
//! existing 0x31 record and `volume_entity` an existing 0x47 record), and WA4 (every block
//! map has the region's water-table `type_key` and `level_key` and fog-table `zone_key`;
//! a release build visits every block map, a debug build a 1-in-23 sample unless
//! `TESSERA_FULL=1`). Every region descriptor's slots 5 and 6 must name decoded tables.
//! Counts of this install are printed, not asserted.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test water_real_data -- --nocapture`.
//! The output (counts, DIDs and a few census values for comparison with the spec, such as
//! the water `value` and fog-zone near and far distances) stays on the local terminal; do
//! not paste it into committed files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::landblock::REGION_DESCRIPTOR_PREFIX;
use tessera_formats::water::{FOG_ZONE_TABLE_PREFIX, FogZoneTable, WATER_TABLE_PREFIX, WaterTable};
use tessera_formats::{BlockMap, Did, Record, RegionDescriptor};

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

/// Every record with type prefix `prefix` (first archive wins), with its stored bytes.
fn records(set: &ArchiveSet, prefix: u8) -> BTreeMap<u32, Vec<u8>> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if out.contains_key(&e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read record");
            out.insert(e.did, payload);
        }
    }
    out
}

fn exists(set: &ArchiveSet, did: Did) -> bool {
    matches!(set.locate(did.0), Ok(Some(_)))
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Runs `f` over `items` on every core; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(64, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 64).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

#[test]
fn wa1_wa3_water_tables() {
    let Some(set) = open_set() else { return };
    let all = records(&set, WATER_TABLE_PREFIX);
    assert!(!all.is_empty(), "no water table");
    let mut listed = 0usize;
    let mut surfaces = BTreeSet::new();
    let mut volumes: BTreeMap<Did, usize> = BTreeMap::new();
    let mut values = BTreeSet::new();
    for (&did, payload) in &all {
        let t = WaterTable::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&t.to_bytes(), payload, "{did:#010x}: re-encoding");
        listed += t.entries().len();
        for e in t.entries().iter().chain(t.default.as_ref()) {
            assert_eq!(t.select(e.key).map(|x| x.key), Some(e.key));
            let Some(w) = &e.wet else { continue };
            assert_eq!(
                w.surface.prefix(),
                0x31,
                "{did:#010x}: key {} surface",
                e.key
            );
            assert!(
                exists(&set, w.surface),
                "{did:#010x}: WA3 {} missing",
                w.surface
            );
            assert_eq!(
                w.volume_entity.prefix(),
                0x47,
                "{did:#010x}: key {} volume entity",
                e.key
            );
            assert!(
                exists(&set, w.volume_entity),
                "{did:#010x}: WA3 {} missing",
                w.volume_entity
            );
            surfaces.insert(w.surface);
            *volumes.entry(w.volume_entity).or_default() += 1;
            values.insert(w.value.to_bits());
        }
        let keys: Vec<u32> = t.entries().iter().map(|e| e.key).collect();
        let dense = keys.iter().enumerate().all(|(i, &k)| k as usize == i + 1);
        eprintln!(
            "  {did:#010x}: {} bytes, level key {}, type key {}, default {:?} (dry {}), {} \
             entries, keys 1 … n in order: {dense}, dry listed {}",
            payload.len(),
            t.level_key,
            t.type_key,
            t.default.map(|d| d.key),
            t.default.is_some_and(|d| d.is_dry()),
            t.entries().len(),
            t.entries().iter().filter(|e| e.is_dry()).count(),
        );
    }
    let values: Vec<f32> = values.into_iter().map(f32::from_bits).collect();
    eprintln!(
        "water tables: {} / {} decoded ({listed} listed entries), 0 leftover bytes; {} \
         distinct surfaces, volume entities {volumes:?}, values {values:?}",
        all.len(),
        all.len(),
        surfaces.len(),
    );
}

#[test]
fn wa2_fog_zone_tables() {
    let Some(set) = open_set() else { return };
    let all = records(&set, FOG_ZONE_TABLE_PREFIX);
    assert!(!all.is_empty(), "no fog-zone table");
    for (&did, payload) in &all {
        let t = FogZoneTable::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&t.to_bytes(), payload, "{did:#010x}: re-encoding");
        let modes: BTreeSet<u8> = t
            .entries()
            .iter()
            .chain(t.default.as_ref())
            .flat_map(|z| z.overrides.iter().map(|o| o.mode))
            .collect();
        eprintln!(
            "  {did:#010x}: {} bytes, zone key {}, default active {:?} near {:?} far {:?}, {} \
             entries ({} active), override modes {modes:?}",
            payload.len(),
            t.zone_key,
            t.default.map(|d| d.is_active()),
            t.default.map(|d| d.near),
            t.default.map(|d| d.far),
            t.entries().len(),
            t.entries().iter().filter(|z| z.is_active()).count(),
        );
    }
    eprintln!(
        "fog-zone tables: {} / {} decoded, 0 leftover bytes",
        all.len(),
        all.len()
    );
}

#[test]
fn wa4_block_maps_have_the_table_keys() {
    let Some(set) = open_set() else { return };
    // Region number → (water table, fog-zone table).
    let mut by_region: BTreeMap<u32, (WaterTable, FogZoneTable)> = BTreeMap::new();
    for (&rdid, payload) in &records(&set, REGION_DESCRIPTOR_PREFIX) {
        let d = RegionDescriptor::parse_expecting(payload, rdid)
            .unwrap_or_else(|e| panic!("{rdid:#010x}: {e}"));
        let w = d.water_table().expect("slot 5 set");
        let f = d.fog_zone_table().expect("slot 6 set");
        let wt = WaterTable::parse_expecting(&set.read(w.0).expect("read water table"), w.0)
            .unwrap_or_else(|e| panic!("{w}: {e}"));
        let ft = FogZoneTable::parse_expecting(&set.read(f.0).expect("read fog table"), f.0)
            .unwrap_or_else(|e| panic!("{f}: {e}"));
        by_region.insert(d.region, (wt, ft));
    }
    let full = exhaustive();
    let mut maps = 0usize;
    let mut zone_nonzero = 0usize;
    for (i, m) in set.members().iter().enumerate() {
        let &ArchiveKind::Map(region) = &m.id.kind else {
            continue;
        };
        let Some((wt, ft)) = by_region.get(&region) else {
            eprintln!("  map archive of region {region}: no region descriptor; skipped");
            continue;
        };
        let entries: Vec<Entry> = m
            .archive
            .entries_range(0x8010_0000..=0x801F_FFFF)
            .map(|e| e.expect("directory walk"))
            .enumerate()
            .filter(|(n, _)| full || n % 23 == 0)
            .map(|(_, e)| e)
            .collect();
        let results = par_each(&entries, |e| {
            let payload = set.member(i).archive.read_entry(e).expect("read block map");
            let b = BlockMap::parse_expecting(&payload, e.did)
                .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
            for key in [wt.type_key, wt.level_key, ft.zone_key] {
                assert!(
                    b.get(key).is_some(),
                    "{:#010x}: WA4 block-map key {key} missing",
                    e.did
                );
            }
            b.get(ft.zone_key) != Some(0)
        });
        maps += results.len();
        zone_nonzero += results.iter().filter(|&&nz| nz).count();
    }
    assert!(maps > 0, "no block map visited");
    eprintln!(
        "WA4: {maps} block maps{} have the water and fog-zone keys; zone value non-zero in \
         {zone_nonzero}",
        if full { "" } else { " (1-in-23 sample)" }
    );
}
