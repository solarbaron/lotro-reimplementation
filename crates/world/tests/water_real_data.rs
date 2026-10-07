// SPDX-License-Identifier: GPL-3.0-or-later
//! The water module of `tessera-world` against a user's own install (read-only).
//!
//! - `landblock_water_census_and_sectors`: WA1 (every region's water table, named by slot 5
//!   of its descriptor, decodes; `WaterTables::load`), WA4 (every block map has the
//!   table's `type_key` and `level_key`), the `water.md` §6.2 rule over every landblock
//!   (wet and fully submerged counts, printed for comparison with §6.4), WA5 (every
//!   (type, *h*) pair of a wet landblock occurs, bit-identical, in the trailing section of
//!   its distant-sector record, §9) and WA6 (deciding twice gives bit-identical states).
//! - `volume_entities_cover_their_landblock`: WA3 for the volume entities and the §7.1
//!   bands: every volume entity a water table names has liquid boxes, they cover the whole
//!   160 × 160 block at the surface, and the column's surface is the water height.
//! - `placed_volumes_census`: the placed static entities whose 0x47 chain carries liquid
//!   monitors (§7.3; counts printed).
//!
//! Counts are printed, not asserted (they change with patches); the invariants are
//! asserted. A debug build visits a 1-in-23 sample of the landblocks unless
//! `TESSERA_FULL=1`; run `cargo test --release -p tessera-world --test water_real_data --
//! --nocapture` for the full census. Only counts and DIDs are printed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::landblock::{LandblockGroupRecord, SectorWaterPair, StaticEntityType};
use tessera_formats::{BlockMap, Did, LandblockData, LandblockInfo, Record};
use tessera_physics::WaterQuery;
use tessera_world::scenery::ObjectProperties;
use tessera_world::water::{
    BlockWater, DryReason, SectorWater, WaterSystem, decide_water, sector_origin,
};
use tessera_world::{BundleOptions, LandblockKey, WorldContext, WorldSource};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// A record through the world's source trait (`None` when absent).
fn rec(set: &ArchiveSet, did: Did) -> Option<Vec<u8>> {
    WorldSource::read(set, did).unwrap_or_else(|e| panic!("{did}: {e}"))
}

fn open(dir: &PathBuf) -> Arc<ArchiveSet> {
    Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("open archive set"))
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

/// Every block-map DID `0x801RXXYY` of every archive (first archive wins), optionally
/// sampled.
fn block_maps(set: &ArchiveSet) -> Vec<u32> {
    let mut out = BTreeSet::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x8010_0000..=0x801F_FFFF) {
            out.insert(e.expect("directory walk").did);
        }
    }
    let all: Vec<u32> = out.into_iter().collect();
    if full() {
        all
    } else {
        all.into_iter().step_by(23).collect()
    }
}

struct Outcome {
    key: LandblockKey,
    decision: Result<tessera_world::water::LandblockWater, DryReason>,
    has_type_key: bool,
    has_level_key: bool,
}

#[test]
fn landblock_water_census_and_sectors() {
    let Some(dir) = install() else { return };
    let set = open(&dir);
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let system = WaterSystem::load(&ctx).expect("water system");
    let tables = system.tables();
    // WA1: every region with a descriptor has a decoded water table.
    for (r, info) in ctx.regions() {
        if let Some(d) = info.descriptor.as_ref().and_then(|d| d.water_table()) {
            assert!(
                tables.water_table(*r).is_some(),
                "region {r}: water table {d} did not load"
            );
        }
    }
    let dids = block_maps(&set);
    assert!(!dids.is_empty(), "no block maps");
    let outcomes: Vec<Outcome> = par_each(&dids, |&did| {
        let key = LandblockKey::from_did(Did(did)).expect("block-map DID");
        let map = BlockMap::parse(&rec(&set, Did(did)).expect("block map"))
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let data_bytes = rec(&set, key.data_did())
            .unwrap_or_else(|| panic!("{}: no landblock data", key.data_did()));
        let (_, _, heights) =
            LandblockData::parse_heights(&data_bytes).unwrap_or_else(|e| panic!("{key}: {e}"));
        let table = tables.water_table(key.region());
        let decision = tables.decide(key, Some(&map), &heights);
        // WA6: deciding again gives a bit-identical state.
        let again = decide_water(table.map(|t| &**t), &map, &heights);
        match (&decision, &again) {
            (Ok(a), Ok(b)) => assert_eq!(a.height.to_bits(), b.height.to_bits()),
            (a, b) => assert_eq!(a, b, "{key}: WA6"),
        }
        Outcome {
            key,
            decision,
            has_type_key: table.is_some_and(|t| map.get(t.type_key).is_some()),
            has_level_key: table.is_some_and(|t| map.get(t.level_key).is_some()),
        }
    });

    // WA4 and the §6.4 census.
    let mut per_region: BTreeMap<u8, [usize; 5]> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut sectors: HashMap<LandblockKey, Vec<SectorWaterPair>> = HashMap::new();
    let mut by_key: HashMap<LandblockKey, tessera_world::water::LandblockWater> = HashMap::new();
    for o in &outcomes {
        assert!(o.has_type_key && o.has_level_key, "{}: WA4", o.key);
        let row = per_region.entry(o.key.region()).or_default();
        row[0] += 1;
        match &o.decision {
            Ok(w) => {
                row[1] += 1;
                if w.is_fully_submerged() {
                    row[2] += 1;
                }
                assert!(w.height > w.lowest_terrain);
                by_key.insert(o.key, *w);
                let pairs = sectors.entry(sector_origin(o.key)).or_default();
                let p = w.sector_pair();
                if !pairs.iter().any(|q| {
                    q.water_type == p.water_type && q.height.to_bits() == p.height.to_bits()
                }) {
                    pairs.push(p);
                }
            }
            Err(DryReason::DryType) => row[3] += 1,
            Err(DryReason::BelowTerrain) => row[4] += 1,
            Err(r) => *reasons.entry(format!("{r:?}")).or_default() += 1,
        }
    }
    let mut total = [0usize; 5];
    for (r, row) in &per_region {
        eprintln!(
            "  region {r}: {} landblocks, dry type {}, typed but not above the lowest sample {}, \
             wet {}, fully submerged {}",
            row[0], row[3], row[4], row[1], row[2]
        );
        for (t, v) in total.iter_mut().zip(row) {
            *t += v;
        }
    }
    eprintln!(
        "landblock water (water.md §6.2{}): {} landblocks, {} wet, {} fully submerged; other \
         dry reasons {reasons:?}",
        if full() { "" } else { ", 1-in-23 sample" },
        total[0],
        total[1],
        total[2],
    );
    assert!(total[1] > 0, "no wet landblock");

    // WA5: the distant-sector record of every sector with water lists every pair.
    let sector_list: Vec<(LandblockKey, Vec<SectorWaterPair>)> = sectors.into_iter().collect();
    let results: Vec<(LandblockKey, usize, usize)> = par_each(&sector_list, |(origin, pairs)| {
        let did = 0x8050_0000
            | (u32::from(origin.region()) << 16)
            | (u32::from(origin.x()) << 8)
            | u32::from(origin.y());
        let Some(bytes) = rec(&set, Did(did)) else {
            return (*origin, 0, pairs.len());
        };
        let rec =
            LandblockGroupRecord::parse(&bytes).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let found = pairs
            .iter()
            .filter(|p| rec.tail_contains_water_pair(**p))
            .count();
        (*origin, found, pairs.len())
    });
    let agree = results.iter().filter(|(_, f, n)| f == n).count();
    let disagree: Vec<_> = results.iter().filter(|(_, f, n)| f != n).take(10).collect();
    eprintln!(
        "distant sectors with water: {}, agreeing with §6.2 (WA5): {agree}",
        results.len()
    );
    assert!(disagree.is_empty(), "WA5 fails for {disagree:?}");

    // The sector builder gives the same pairs as the census grouping (full runs only:
    // a sample does not hold whole sectors).
    if full() {
        for (origin, _, n) in results.iter().take(500) {
            let sw = SectorWater::build(*origin, |k| by_key.get(&k).copied());
            assert_eq!(sw.pairs.len(), *n, "{origin}");
        }
    }
}

#[test]
fn volume_entities_cover_their_landblock() {
    let Some(dir) = install() else { return };
    let set = open(&dir);
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let system = WaterSystem::load(&ctx).expect("water system");
    let mut seen = BTreeSet::new();
    for r in system.tables().water_regions().collect::<Vec<_>>() {
        let table = system.tables().water_table(r).unwrap();
        for e in table.entries() {
            let Some(w) = e.wet else { continue };
            if !seen.insert(w.volume_entity) {
                continue;
            }
            assert!(
                ctx.source().contains(w.volume_entity).unwrap(),
                "WA3: {} missing",
                w.volume_entity
            );
            let t = system
                .volumes()
                .template(&**ctx.source(), ctx.schema(), w.volume_entity)
                .unwrap()
                .unwrap_or_else(|| panic!("{}: no liquid monitors", w.volume_entity));
            let h = 149.019_6_f32;
            let bw = BlockWater {
                key: LandblockKey::new(r, 0, 0).unwrap(),
                decision: Err(DryReason::DryType),
                boxes: t.place_landblock(h),
                placed_volume_entities: 0,
                boxes_left_out: 0,
            };
            let lava = t.boxes.iter().any(|b| b.flags.lava);
            let names: Vec<_> = t.boxes.iter().map(|b| b.name).collect();
            eprintln!(
                "  volume entity {}: {} liquid boxes {names:?}, lava {lava}",
                w.volume_entity,
                t.boxes.len()
            );
            for (x, y) in [
                (0.5, 0.5),
                (159.5, 0.5),
                (0.5, 159.5),
                (159.5, 159.5),
                (80.0, 80.0),
            ] {
                for depth in [0.25_f32, 0.75, 50.0, 1500.0] {
                    let s = bw.water_at([x, y, h - depth]).unwrap_or_else(|| {
                        panic!("{}: dry at ({x}, {y}) depth {depth}", w.volume_entity)
                    });
                    assert!(
                        (s.surface - h).abs() < 1e-3,
                        "{}: surface {} at depth {depth}",
                        w.volume_entity,
                        s.surface
                    );
                    assert!(s.flags.liquid);
                }
                assert!(bw.water_at([x, y, h + 0.01]).is_none());
            }
        }
    }
    assert!(!seen.is_empty(), "no volume entity");
}

#[test]
fn placed_volumes_census() {
    let Some(dir) = install() else { return };
    let set = open(&dir);
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let system = WaterSystem::load(&ctx).expect("water system");
    let mut infos = BTreeSet::new();
    for m in set.members() {
        let ArchiveKind::Cell(r) = m.id.kind else {
            continue;
        };
        let lo = 0x8020_0000 | (r << 16);
        for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
            infos.insert(e.expect("directory walk").did);
        }
    }
    let infos: Vec<u32> = if full() {
        infos.into_iter().collect()
    } else {
        infos.into_iter().step_by(23).collect()
    };
    let found: Vec<Vec<(Did, usize)>> = par_each(&infos, |&did| {
        let (bytes, stamp) = WorldSource::read_stamped(&*set, Did(did))
            .unwrap()
            .expect("info");
        let Ok(info) = LandblockInfo::parse(&bytes, stamp, ctx.schema()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in &info.entities {
            if e.kind() != Some(StaticEntityType::Physics) {
                continue;
            }
            let Some(object) = e.object else { continue };
            if let Some(t) = system
                .volumes()
                .template(&**ctx.source(), ctx.schema(), object)
                .unwrap()
            {
                out.push((object, t.boxes.len()));
            }
        }
        out
    });
    let mut per_object: BTreeMap<Did, usize> = BTreeMap::new();
    for (o, n) in found.iter().flatten() {
        assert!(*n > 0);
        *per_object.entry(*o).or_default() += 1;
    }
    let entities: usize = per_object.values().sum();
    let mut top: Vec<_> = per_object.iter().map(|(d, n)| (*n, *d)).collect();
    top.sort_unstable_by(|a, b| b.cmp(a));
    eprintln!(
        "placed volumes (§7.3{}): {entities} Physics entities over {} landblock-info records \
         name {} distinct 0x47 objects with liquid monitors; most used {:?}",
        if full() { "" } else { ", 1-in-23 sample" },
        infos.len(),
        per_object.len(),
        &top[..top.len().min(3)],
    );
    assert!(entities > 0, "no placed water volume");
    // Placements per applied property set, with the set's flags.
    let mut per_set: BTreeMap<Did, (usize, usize, String)> = BTreeMap::new();
    for (o, n) in &per_object {
        let t = system
            .volumes()
            .template(&**ctx.source(), ctx.schema(), *o)
            .unwrap()
            .unwrap();
        let sets: BTreeSet<Did> = t.boxes.iter().map(|b| b.properties).collect();
        for set in sets {
            let flags = t.boxes.iter().find(|b| b.properties == set).unwrap().flags;
            let e = per_set.entry(set).or_insert((0, 0, format!("{flags:?}")));
            e.0 += 1;
            e.1 += n;
        }
    }
    let dict = tessera_world::scenery::property_dictionary(&ctx).expect("dictionary");
    let coin = dict.pid("Volume_IsCoin");
    for (set, (objects, placements, flags)) in &per_set {
        let props = tessera_world::scenery::read_set(&**ctx.source(), ctx.schema(), *set)
            .expect("monitor set");
        let is_coin = coin.and_then(|p| props.get(p)).is_some();
        eprintln!(
            "  set {set}: {objects} objects, {placements} placements, {flags}, \
             Volume_IsCoin {is_coin}"
        );
    }
    // Objects whose monitors come from a parent 0x47 record rather than their own.
    let monitors = dict.pid("Entity_Monitors").expect("Entity_Monitors");
    let inherited: Vec<(Did, usize)> = per_object
        .iter()
        .filter(|(d, _)| {
            let p = ObjectProperties::load(&**ctx.source(), ctx.schema(), **d).expect("chain");
            p.levels.first().is_none_or(|l| l.get(monitors).is_none())
        })
        .map(|(d, n)| (*d, *n))
        .collect();
    eprintln!(
        "  of them, objects whose liquid monitors are inherited from a parent 0x47 record: \
         {inherited:?}"
    );
}
