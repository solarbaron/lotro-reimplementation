// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery determinism through the streaming path against a user's own install (read-only).
//! Skipped unless `TESSERA_GAME_DIR` points at an install.
//!
//! `scenery_is_deterministic_on_the_workers` is ROADMAP M4.5 exit criterion 3 (SC8) as the
//! world streamer runs it: every landblock with a non-zero coarse grid (78,114 in the
//! owner's install) is built as a landblock bundle by the bundle store on the asset
//! workers, with its real water plane wired into scenery and its collision set built on the
//! same worker (exit criterion 5). It is done twice: once with 1 worker in ascending DID
//! order, once with the default worker count in descending order, each with fresh caches.
//! The generated object lists (DID, position, rotation, scale) must be identical bit for
//! bit. Frill thinning below density 1 is random by design and not part of the lists.
//!
//! Debug builds sample every 97th landblock unless `TESSERA_FULL=1`; run
//! `cargo test --release -p tessera-world --test determinism_real_data -- --nocapture` for
//! the full criterion. `TESSERA_DETERMINISM_STRIDE` overrides the sampling stride. Only
//! counts, digests and timings are printed; no game data leaves the install.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{AssetState, AssetStore, PoolConfig, Priority, StoreConfig, WorkerPool};
use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::{Did, LandblockData};
use tessera_world::{
    BundleDecoder, BundleOptions, BundleSource, LandblockBundle, LandblockKey, WorldContext,
    WorldSource,
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn stride() -> usize {
    if let Some(s) = std::env::var("TESSERA_DETERMINISM_STRIDE")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return s;
    }
    let full =
        !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1");
    if full { 1 } else { 97 }
}

/// The landblock-data DIDs of the cell archives whose coarse grid is not all zero.
fn scenery_blocks(set: &ArchiveSet) -> Vec<u32> {
    let mut dids: Vec<u32> = Vec::new();
    for m in set.members() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8000_0000..=0x800F_FFFF) {
            dids.push(e.expect("directory walk").did);
        }
    }
    dids.sort_unstable();
    dids.dedup();
    dids.into_iter()
        .filter(|&did| {
            let Some((bytes, stamp)) = WorldSource::read_stamped(set, Did(did)).expect("read")
            else {
                return false;
            };
            let data = LandblockData::parse_expecting(&bytes, did, stamp).expect("data decodes");
            data.coarse.iter().any(|w| *w != 0)
        })
        .collect()
}

/// FNV-1a over the bit patterns of a bundle's generated objects.
fn digest(b: &LandblockBundle) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    let mut eat = |v: u32| {
        for byte in v.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    for e in &b.generated {
        eat(e.object.map_or(0, |d| d.0));
        e.placement.position.iter().for_each(|c| eat(c.to_bits()));
        e.placement
            .rotation
            .to_wxyz()
            .iter()
            .for_each(|c| eat(c.to_bits()));
        e.scale.iter().for_each(|c| eat(c.to_bits()));
    }
    h
}

/// What one run records per landblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    digest: u64,
    objects: usize,
    complete: bool,
}

#[derive(Debug, Default)]
struct Totals {
    objects: usize,
    blocking: u64,
    wet: usize,
    with_water_objects: usize,
    triangles: u64,
    max_bundle: usize,
    bundle_bytes: u128,
}

/// Builds every block of `dids` through a fresh bundle store on a pool of `workers`
/// workers (0 = default), in the given order, a chunk at a time.
fn run(
    set: &Arc<ArchiveSet>,
    workers: usize,
    dids: &[u32],
) -> (BTreeMap<u32, Outcome>, Totals, Duration, usize) {
    let ctx = Arc::new(
        WorldContext::from_install(
            Arc::clone(set),
            BundleOptions {
                resolve_area_names: false,
                ..BundleOptions::default()
            },
        )
        .expect("world context"),
    );
    assert!(
        ctx.regions().values().any(|r| r.scenery.is_some()),
        "no region scenery tables"
    );
    let pool = WorkerPool::new(PoolConfig {
        workers,
        ..PoolConfig::default()
    });
    let used = pool.workers();
    let store = AssetStore::new(
        &pool,
        BundleSource::new(Arc::clone(ctx.source())),
        BundleDecoder::new(Arc::clone(&ctx)),
        StoreConfig {
            cache_budget: 0,
            ..StoreConfig::default()
        },
    );
    let mut out = BTreeMap::new();
    let mut totals = Totals::default();
    let start = Instant::now();
    for chunk in dids.chunks(2_048) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|&did| (did, store.request(did, Priority::Visible)))
            .collect();
        for (did, h) in handles {
            let AssetState::Ready(b) = h.wait(Duration::from_secs(600)) else {
                panic!("{did:#010x}: bundle did not build: {:?}", h.state().name());
            };
            let complete = b.scenery.as_ref().is_some_and(|s| s.complete);
            totals.objects += b.generated.len();
            if let Some(c) = &b.collision {
                totals.blocking += u64::from(c.report().generated_blocking);
                totals.triangles += c.set.triangle_count() as u64;
            }
            if b.water.as_ref().is_some_and(|w| w.water().is_some()) {
                totals.wet += 1;
                if b.scenery.as_ref().is_some_and(|s| s.water.is_some()) {
                    totals.with_water_objects += 1;
                }
            }
            totals.max_bundle = totals.max_bundle.max(b.heap_bytes);
            totals.bundle_bytes += b.heap_bytes as u128;
            out.insert(
                did,
                Outcome {
                    digest: digest(&b),
                    objects: b.generated.len(),
                    complete,
                },
            );
        }
    }
    (out, totals, start.elapsed(), used)
}

#[test]
fn scenery_is_deterministic_on_the_workers() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let t = Instant::now();
    let all = scenery_blocks(&set);
    let n = all.len();
    let every = stride();
    let dids: Vec<u32> = all.into_iter().step_by(every).collect();
    eprintln!(
        "{n} landblocks with a non-zero coarse grid (listed in {:.1} s); building every {every}th: {}",
        t.elapsed().as_secs_f64(),
        dids.len()
    );

    let (a, ta, t1, _) = run(&set, 1, &dids);
    let reversed: Vec<u32> = dids.iter().rev().copied().collect();
    let (b, tb, tn, workers) = run(&set, 0, &reversed);

    let differ: Vec<u32> = a
        .iter()
        .filter(|(k, v)| b.get(k) != Some(v))
        .map(|(k, _)| *k)
        .collect();
    let incomplete = a.values().filter(|o| !o.complete).count();
    eprintln!(
        "{} landblocks: {} generated objects ({} blocking), {} wet ({} with a water plane in \
         scenery), {} collision triangles",
        a.len(),
        ta.objects,
        ta.blocking,
        ta.wet,
        ta.with_water_objects,
        ta.triangles
    );
    eprintln!(
        "  1 worker {:.1} s ({:.2} ms per landblock); {workers} workers {:.1} s; bundle mean \
         {:.0} KiB, max {:.0} KiB",
        t1.as_secs_f64(),
        t1.as_secs_f64() * 1e3 / a.len().max(1) as f64,
        tn.as_secs_f64(),
        ta.bundle_bytes as f64 / a.len().max(1) as f64 / 1024.0,
        ta.max_bundle as f64 / 1024.0
    );
    eprintln!("  {} differ, {incomplete} incomplete", differ.len());
    assert_eq!(a.len(), b.len());
    assert_eq!(ta.objects, tb.objects);
    assert!(
        differ.is_empty(),
        "SC8: lists differ, first {:#010x}",
        differ[0]
    );
    assert_eq!(incomplete, 0, "a scenery input could not be read");
    assert_eq!(
        ta.wet, ta.with_water_objects,
        "water plane not wired into scenery"
    );
    if every == 1 {
        assert_eq!(a.len(), 78_114, "landblocks with a non-zero coarse grid");
    }
    // A key that is not a scenery block is never in the lists.
    assert!(a.keys().all(|&d| LandblockKey::from_did(d).is_some()));
}
