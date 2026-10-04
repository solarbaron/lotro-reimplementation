// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data throughput check against a user's own install (read-only).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install directory. The archives are
//! opened read-only through `tessera-dat`; nothing is written. Results are printed as
//! counts and timings, never as game content. `TESSERA_SURFACES=<n>` changes the sample
//! size (default 2000); run with `--release --nocapture` for meaningful numbers.

#![cfg(feature = "formats")]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tessera_assets::decoders::SurfaceRgba;
use tessera_assets::{
    AssetState, AssetStore, Decoder, JobContext, Outcome, PoolConfig, Priority, StoreConfig,
    WorkerPool,
};
use tessera_dat::{ArchiveSet, SetOptions};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn sample_size() -> usize {
    std::env::var("TESSERA_SURFACES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000)
}

/// Deterministic sample of `n` items (xorshift-driven partial Fisher–Yates).
fn sample<T: Copy>(items: &[T], n: usize, seed: u64) -> Vec<T> {
    let mut v = items.to_vec();
    let mut x = seed | 1;
    let n = n.min(v.len());
    for i in 0..n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = i + (x % (v.len() - i) as u64) as usize;
        v.swap(i, j);
    }
    v.truncate(n);
    v
}

#[test]
fn decode_random_surfaces_through_the_pool() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let mut all = BTreeSet::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x4100_0000..0x4200_0000) {
            all.insert(e.expect("directory walk").did);
        }
    }
    let all: Vec<u32> = all.into_iter().collect();
    assert!(!all.is_empty(), "no 0x41 records in the install");
    let picked = sample(&all, sample_size(), 0x5EED_0041);
    eprintln!(
        "surfaces in install: {}; sampled: {}",
        all.len(),
        picked.len()
    );

    let pool = WorkerPool::new(PoolConfig::default());
    let store = AssetStore::new(
        &pool,
        set,
        SurfaceRgba,
        StoreConfig {
            cache_budget: 512 << 20,
            ..StoreConfig::default()
        },
    );

    // Feed requests in a window kept well below `PoolConfig::max_queued` and drain
    // completions the way a frame loop would, without blocking. Only requests that got a
    // job post a completion (a refused or already-final request does not), so progress is
    // counted per job, not per request.
    let window = PoolConfig::default().max_queued / 4;
    let start = Instant::now();
    let mut submit_time = Duration::ZERO;
    let mut next = 0usize;
    let mut expected = 0usize;
    let mut done = 0usize;
    let mut pixels_bytes = 0u64;
    let mut failures = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(600);
    while next < picked.len() || done < expected {
        assert!(
            Instant::now() < deadline,
            "timed out with {done}/{expected} done"
        );
        let t = Instant::now();
        while next < picked.len() && expected - done < window {
            drop(store.request(picked[next], Priority::Visible));
            next += 1;
            // Every new job posts exactly one completion; refused requests post none.
            let st = store.stats();
            expected = (st.misses - st.rejected) as usize;
        }
        submit_time += t.elapsed();
        let batch = store.poll_completions(256);
        if batch.is_empty() {
            std::thread::sleep(Duration::from_micros(200));
            continue;
        }
        for c in batch {
            done += 1;
            match c.outcome {
                Outcome::Ready(img) => pixels_bytes += img.pixels.len() as u64,
                Outcome::Failed(e) => failures.push((c.key, e.to_string())),
                Outcome::Cancelled => panic!("{} cancelled", c.key),
            }
        }
    }
    let wall = start.elapsed();
    assert_eq!(store.stats().rejected, 0);

    let s = store.stats();
    let p = pool.stats();
    let secs = wall.as_secs_f64();
    eprintln!(
        "workers: {}; submit: {:?}; wall: {:?}; {:.0} surfaces/s; {:.1} MiB/s RGBA8 out",
        p.workers,
        submit_time,
        wall,
        picked.len() as f64 / secs,
        pixels_bytes as f64 / secs / (1 << 20) as f64
    );
    eprintln!(
        "read total: {:?}; decode total: {:?}; mean decode: {:?}; cache: {} entries, {} MiB; evictions: {}",
        s.read_time,
        s.decode_time,
        s.mean_decode_time(),
        s.cache_entries,
        s.cache_bytes >> 20,
        s.evictions
    );
    eprintln!("failed: {}", failures.len());
    for (k, e) in failures.iter().take(10) {
        eprintln!("  {k}: {e}");
    }
    assert!(failures.is_empty(), "{} surfaces failed", failures.len());
    assert_eq!(s.completed as usize, picked.len());

    // Second pass over the cached part: every request should be a hit.
    let cached: Vec<u32> = picked
        .iter()
        .copied()
        .filter(|&d| store.contains(d))
        .collect();
    let before = store.stats().hits;
    for &d in &cached {
        assert!(matches!(
            store.request(d, Priority::Visible).state(),
            AssetState::Ready(_)
        ));
    }
    assert_eq!(store.stats().hits - before, cached.len() as u64);

    // Single-threaded baseline on a slice of the sample, for the speed-up figure.
    let n = picked.len().min(200);
    let t0 = Instant::now();
    for &did in &picked[..n] {
        let bytes = store.source().read(did).expect("read");
        SurfaceRgba
            .decode(did.into(), bytes, &JobContext::detached())
            .expect("decode");
    }
    let per = t0.elapsed().as_secs_f64() / n as f64;
    eprintln!(
        "single-thread baseline: {:.0} surfaces/s; pool speed-up ≈ {:.1}×",
        1.0 / per,
        (picked.len() as f64 / secs) * per
    );
    store.log_stats();
}
