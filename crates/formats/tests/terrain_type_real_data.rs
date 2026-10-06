// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain type tables (prefix 0x19, `docs/specs/terrain-texturing.md` §3) against a user's
//! own install (read-only).
//!
//! Every 0x19 record of every archive must decode with §3.2–§3.3 to its last byte, which
//! enforces the decoder invariants of §3.5 (exact consumption, keys exactly 0 … n − 1, curve
//! modes 1–10, bool8 fields 0 or 1, projection 1–3, label length within the record), and
//! re-encode byte for byte. Every non-zero texture DID must be an existing 0x40 texture list
//! and every non-zero physics material an existing 0x78 property set (§3.3). Every region
//! descriptor's slot 8 must name a decodable table. Counts and the census of §3.3, §3.4 and
//! `scenery.md` §4.4 are printed, not asserted (observations of one install).
//!
//! SC7 (`scenery.md` §13, §4.4; `terrain-texturing.md` §4.1): every terrain-chunk layer
//! `material` and every terrain quad-grid value of every landblock-data record (kind 0 of
//! each `client_cell_R` archive) is a key of the slot-8 table of region R's descriptor.
//! Ceiling chunks are counted and printed, not asserted (the spec states SC7 for terrain
//! chunks). A release build visits every record; a debug build a 1-in-23 sample unless
//! `TESSERA_FULL=1`.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test terrain_type_real_data -- --nocapture`.
//! Only counts and DIDs are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::landblock::{QUAD_SIDE, REGION_DESCRIPTOR_PREFIX, SurfaceChunk};
use tessera_formats::terrain_type::{TERRAIN_TYPE_TABLE_PREFIX, TerrainTypeTable};
use tessera_formats::{Did, LandblockData, Record, RegionDescriptor};

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

#[test]
fn terrain_type_tables_decode_exactly() {
    let Some(set) = open_set() else { return };
    let all = records(&set, TERRAIN_TYPE_TABLE_PREFIX);
    assert!(!all.is_empty(), "no terrain type table");
    let mut decoded = BTreeMap::new();
    let mut entries = 0usize;
    for (&did, payload) in &all {
        let t = TerrainTypeTable::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&t.to_bytes(), payload, "{did:#010x}: re-encoding");
        entries += t.len();
        decoded.insert(did, t);
    }
    eprintln!(
        "terrain type tables: {} / {} decoded, {entries} entries, 0 leftover bytes",
        decoded.len(),
        all.len()
    );

    // §3.3: every texture DID resolves to a 0x40 list; physics materials are 0x78 sets.
    let mut textures = BTreeSet::new();
    let mut physics = BTreeSet::new();
    for t in decoded.values() {
        for e in t.entries() {
            for d in [e.diffuse, e.normal, e.emissive, e.glow] {
                if !d.is_null() {
                    textures.insert(d);
                }
            }
            if !e.physics_material.is_null() {
                physics.insert(e.physics_material);
            }
        }
    }
    for d in &textures {
        assert_eq!(d.prefix(), 0x40, "{d}: texture DID prefix");
        assert!(exists(&set, *d), "{d}: texture list missing");
    }
    for d in &physics {
        assert_eq!(d.prefix(), 0x78, "{d}: physics material prefix");
        assert!(exists(&set, *d), "{d}: physics material missing");
    }
    eprintln!(
        "  {} distinct texture lists, {} distinct physics materials, all resolve",
        textures.len(),
        physics.len()
    );

    // Census (§3.3, §3.4, §3.6, scenery.md §4.4), printed only.
    for (did, t) in &decoded {
        let min_order = t.entries().iter().map(|e| e.draw_order).min();
        let key0 = t.get(0).map(|e| e.draw_order);
        let no_proc = t.entries().iter().filter(|e| !e.procedural).count();
        let curves: BTreeMap<u32, usize> = t
            .entries()
            .iter()
            .flat_map(|e| [e.u_offset.mode(), e.v_offset.mode()])
            .fold(BTreeMap::new(), |mut m, k| {
                *m.entry(k).or_default() += 1;
                m
            });
        eprintln!(
            "  {did:#010x}: {} bytes, {} entries, key 0 has the smallest draw order: {}, \
             procedural off in {no_proc}, curve modes {curves:?}",
            all[did].len(),
            t.len(),
            key0.is_some() && key0 == min_order,
        );
    }
    let flat = || decoded.values().flat_map(|t| t.entries().iter());
    let projections: BTreeMap<u32, usize> = flat().fold(BTreeMap::new(), |mut m, e| {
        *m.entry(e.projection.raw()).or_default() += 1;
        m
    });
    eprintln!(
        "  projection {projections:?}; opacity mask {}; specular both {}; diffuse 0 in {}; \
         normal 0 in {}; physics 0 in {}",
        flat().filter(|e| e.use_opacity_mask).count(),
        flat().filter(|e| e.specular_switches()).count(),
        flat().filter(|e| e.diffuse.is_null()).count(),
        flat().filter(|e| e.normal.is_null()).count(),
        flat().filter(|e| e.physics_material.is_null()).count(),
    );

    // Every region descriptor's slot 8 names one of these tables.
    let regions = records(&set, REGION_DESCRIPTOR_PREFIX);
    for (&rdid, payload) in &regions {
        let d = RegionDescriptor::parse_expecting(payload, rdid)
            .unwrap_or_else(|e| panic!("{rdid:#010x}: {e}"));
        let tt = d.terrain_type_table().expect("slot 8 set");
        assert!(
            decoded.contains_key(&tt.0),
            "{rdid:#010x}: slot 8 {tt} not a decoded table"
        );
        eprintln!("  region {:>2} → {tt}", d.region);
    }
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

/// Layers, quads, and the first offending (did, value, what) of one surface chunk checked
/// against `table`.
#[derive(Default)]
struct KeyCheck {
    layers: usize,
    quads: usize,
    bad: Vec<(u32, u32, &'static str)>,
}

impl KeyCheck {
    fn chunk(&mut self, did: u32, chunk: &SurfaceChunk, table: &TerrainTypeTable) {
        for layer in &chunk.layers {
            self.layers += 1;
            if table.get(layer.material).is_none() {
                self.bad.push((did, layer.material, "layer material"));
            }
        }
        if chunk.quads.is_some() {
            for qy in 0..QUAD_SIDE {
                for qx in 0..QUAD_SIDE {
                    let Some(q) = chunk.quad_material(qx, qy) else {
                        continue;
                    };
                    self.quads += 1;
                    if table.get(u32::from(q)).is_none() {
                        self.bad.push((did, u32::from(q), "quad-grid value"));
                    }
                }
            }
        }
    }

    fn merge(&mut self, o: Self) {
        self.layers += o.layers;
        self.quads += o.quads;
        self.bad.extend(o.bad);
    }
}

#[test]
fn sc7_every_layer_and_quad_is_a_key_of_the_region_table() {
    let Some(set) = open_set() else { return };
    // Region number → its slot-8 table, decoded.
    let mut tables: BTreeMap<u32, TerrainTypeTable> = BTreeMap::new();
    for (&rdid, payload) in &records(&set, REGION_DESCRIPTOR_PREFIX) {
        let d = RegionDescriptor::parse_expecting(payload, rdid)
            .unwrap_or_else(|e| panic!("{rdid:#010x}: {e}"));
        let tt = d.terrain_type_table().expect("slot 8 set");
        let payload = set.read(tt.0).unwrap_or_else(|e| panic!("{tt}: {e}"));
        let t = TerrainTypeTable::parse_expecting(&payload, tt.0)
            .unwrap_or_else(|e| panic!("{tt}: {e}"));
        tables.insert(d.region, t);
    }

    let full = exhaustive();
    let (mut terrain, mut ceiling) = (KeyCheck::default(), KeyCheck::default());
    let mut blocks = 0usize;
    for (i, m) in set.members().iter().enumerate() {
        let &ArchiveKind::Cell(region) = &m.id.kind else {
            continue;
        };
        let entries: Vec<Entry> = m
            .archive
            .entries_range(0x8000_0000..=0x800F_FFFF)
            .map(|e| e.expect("directory walk"))
            .enumerate()
            .filter(|(n, _)| full || n % 23 == 0)
            .map(|(_, e)| e)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let table = tables
            .get(&region)
            .unwrap_or_else(|| panic!("region {region}: landblock data but no descriptor"));
        let results = par_each(&entries, |e| {
            let payload = set.member(i).archive.read_entry(e).expect("read record");
            let d = LandblockData::parse_expecting(&payload, e.did, Some(e.stamp))
                .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
            let mut t = KeyCheck::default();
            t.chunk(e.did, &d.terrain, table);
            let mut c = KeyCheck::default();
            if let Some(ceil) = &d.ceiling {
                c.chunk(e.did, &ceil.surface, table);
            }
            (t, c)
        });
        let (mut rt, mut rc) = (KeyCheck::default(), KeyCheck::default());
        for (t, c) in results {
            rt.merge(t);
            rc.merge(c);
        }
        eprintln!(
            "  region {region:>2} ({}, {} keys): {} blocks, {} layers, {} quads; ceiling {} \
             layers, {} quads, {} not keys",
            table.did,
            table.len(),
            entries.len(),
            rt.layers,
            rt.quads,
            rc.layers,
            rc.quads,
            rc.bad.len()
        );
        blocks += entries.len();
        terrain.merge(rt);
        ceiling.merge(rc);
    }
    eprintln!(
        "SC7: {blocks} landblock-data records{}; {} terrain layers and {} quads all keys of \
         their region's table: {}; ceiling {} layers, {} quads, {} not keys",
        if full { "" } else { " (1-in-23 sample)" },
        terrain.layers,
        terrain.quads,
        terrain.bad.is_empty(),
        ceiling.layers,
        ceiling.quads,
        ceiling.bad.len()
    );
    assert!(blocks > 0, "no landblock-data record");
    assert!(
        terrain.bad.is_empty(),
        "SC7: {} terrain values are not keys, first {:#010x}: {} {}",
        terrain.bad.len(),
        terrain.bad[0].0,
        terrain.bad[0].2,
        terrain.bad[0].1
    );
}
