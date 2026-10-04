// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of every mesh record of a user's own install (read-only).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install directory. Archives are opened
//! read-only; nothing is written. Output is counts, DIDs and categories only, never
//! geometry. Run with `--release -- --nocapture`; `TESSERA_THREADS` sets the worker count
//! (default 8) and `TESSERA_MESH_LIMIT` decodes only the first N records.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_mesh::{
    CollisionTree, DecodeOptions, Did, FailureCategory, GeometryLimits, LevelKind, Mesh,
    MeshLibrary, MeshLibraryConfig, mesh_dids,
};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[derive(Default)]
struct Census {
    records: u64,
    ok: u64,
    failures: BTreeMap<FailureCategory, (u64, Vec<(u32, String)>)>,
    issues: BTreeMap<String, (u64, u64, Vec<u32>)>,
    records_with_issues: u64,
    fallback_disagrees: Vec<u32>,
    roundtrip_mismatch: Vec<u32>,
    geometry_fail: Vec<(u32, String)>,
    submeshes: u64,
    vertices: u64,
    triangles: u64,
    skinned_submeshes: u64,
    detail_levels: u64,
    proxy_levels: u64,
    collision_meshes: u64,
    tree_blocks: u64,
    tree_code: u64,
    build_bytes: u64,
    formats: BTreeMap<u32, u64>,
    stamps: BTreeMap<u16, u64>,
    flags: BTreeMap<u32, u64>,
    bytes: u64,
    /// Records that strict decoding (structural issues only) rejects.
    strict_fail: Vec<(u32, String)>,
    /// Largest per level: vertices, indices, draws, stream bytes; amplification ×1000.
    max_level: [u64; 5],
    /// Largest per record: submeshes, levels, and decoded heap bytes beyond 1.4 × the
    /// record bytes (the fixed overhead the crate documents).
    max_record: [u64; 3],
}

fn build_bytes(m: &Mesh) -> Vec<Option<u8>> {
    m.levels
        .iter()
        .map(|l| match &l.tree {
            Some(CollisionTree::Code(t)) => t.build_byte,
            _ => None,
        })
        .collect()
}

fn census_one(did: u32, set: &ArchiveSet, c: &mut Census) {
    c.records += 1;
    let (located, bytes) = match set.read_with_entry(did) {
        Ok(x) => x,
        Err(e) => {
            let f = c.failures.entry(FailureCategory::Source).or_default();
            f.0 += 1;
            if f.1.len() < 5 {
                f.1.push((did, e.to_string()));
            }
            return;
        }
    };
    let stamp = located.entry.stamp;
    *c.stamps.entry(stamp).or_default() += 1;
    c.bytes += bytes.len() as u64;
    let mesh =
        match Mesh::decode_expecting(&bytes, Did(did), Some(stamp), &DecodeOptions::lossless()) {
            Ok(m) => m,
            Err(e) => {
                let f = c.failures.entry(e.category()).or_default();
                f.0 += 1;
                if f.1.len() < 5 {
                    f.1.push((did, e.to_string()));
                }
                return;
            }
        };
    c.ok += 1;

    // Lossless: re-encoding gives the payload back.
    match mesh.to_bytes() {
        Ok(b) if b == bytes => {}
        _ => c.roundtrip_mismatch.push(did),
    }
    // The §10.4 fallback agrees with the stamp rule.
    match Mesh::decode(&bytes, None, &DecodeOptions::render_only()) {
        Ok(m) if build_bytes(&m) == build_bytes(&mesh) => {}
        _ => c.fallback_disagrees.push(did),
    }
    // Every level flattens within the default limits; record how close it comes.
    for li in 0..mesh.levels.len() {
        match mesh.level_geometry(li, &GeometryLimits::default()) {
            Ok(g) => {
                let lv = &mesh.levels[li];
                let mut blocks: Vec<u32> = lv.draws.iter().map(|d| d.vertex_block).collect();
                blocks.sort_unstable();
                blocks.dedup();
                let mut buffers: Vec<u32> = lv.draws.iter().map(|d| d.index_buffer).collect();
                buffers.sort_unstable();
                buffers.dedup();
                let stored: u64 = blocks
                    .iter()
                    .map(|&b| {
                        let sm = &mesh.submeshes[b as usize];
                        u64::from(sm.vertex_count) * sm.format.layout().stride as u64
                    })
                    .sum::<u64>()
                    + buffers
                        .iter()
                        .map(|&b| 2 * mesh.index_buffers[b as usize].indices.len() as u64)
                        .sum::<u64>();
                let amp = (g.stream_bytes() * 1000).checked_div(stored).unwrap_or(0);
                for (m, v) in c.max_level.iter_mut().zip([
                    g.vertex_count() as u64,
                    g.indices.len() as u64,
                    g.parts.len() as u64,
                    g.stream_bytes(),
                    amp,
                ]) {
                    *m = (*m).max(v);
                }
            }
            Err(e) => c.geometry_fail.push((did, format!("level {li}: {e}"))),
        }
    }
    for (m, v) in c.max_record.iter_mut().zip([
        mesh.submeshes.len() as u64,
        mesh.levels.len() as u64,
        (mesh.heap_size() as u64).saturating_sub(bytes.len() as u64 * 14 / 10),
    ]) {
        *m = (*m).max(v);
    }

    let report = mesh.validate();
    if !report.is_clean() {
        c.records_with_issues += 1;
        // Strict decoding fails only on structural issues.
        let strict = DecodeOptions {
            strict: true,
            ..DecodeOptions::lossless()
        };
        if let Err(e) = Mesh::decode(&bytes, Some(stamp), &strict) {
            c.strict_fail.push((did, e.to_string()));
        }
    }
    for i in &report.issues {
        let key = format!("{} {:?}", i.kind.invariant(), i.kind);
        let e = c.issues.entry(key).or_default();
        e.0 += 1;
        e.1 += i.count;
        if e.2.len() < 5 && !e.2.contains(&did) {
            e.2.push(did);
        }
    }

    *c.flags.entry(mesh.flags).or_default() += 1;
    for sm in &mesh.submeshes {
        c.submeshes += 1;
        c.vertices += u64::from(sm.vertex_count);
        c.skinned_submeshes += u64::from(sm.is_skinned());
        *c.formats.entry(sm.format.raw()).or_default() += 1;
    }
    c.triangles += mesh
        .index_buffers
        .iter()
        .map(|b| b.triangle_count() as u64)
        .sum::<u64>();
    for l in &mesh.levels {
        match l.kind {
            LevelKind::Detail => c.detail_levels += 1,
            LevelKind::Proxy => c.proxy_levels += 1,
            LevelKind::Other(_) => {}
        }
        c.collision_meshes += u64::from(l.collision_counts.is_some());
        if let Some(t) = &l.tree {
            c.tree_blocks += 1;
            if let CollisionTree::Code(code) = t {
                c.tree_code += 1;
                c.build_bytes += u64::from(code.build_byte.is_some());
            }
        }
    }
}

fn merge(into: &mut Census, from: Census) {
    into.records += from.records;
    into.ok += from.ok;
    for (k, (n, s)) in from.failures {
        let e = into.failures.entry(k).or_default();
        e.0 += n;
        for x in s {
            if e.1.len() < 5 {
                e.1.push(x);
            }
        }
    }
    for (k, (n, cnt, s)) in from.issues {
        let e = into.issues.entry(k).or_default();
        e.0 += n;
        e.1 += cnt;
        for x in s {
            if e.2.len() < 5 {
                e.2.push(x);
            }
        }
    }
    into.records_with_issues += from.records_with_issues;
    into.fallback_disagrees.extend(from.fallback_disagrees);
    into.roundtrip_mismatch.extend(from.roundtrip_mismatch);
    into.geometry_fail.extend(from.geometry_fail);
    into.submeshes += from.submeshes;
    into.vertices += from.vertices;
    into.triangles += from.triangles;
    into.skinned_submeshes += from.skinned_submeshes;
    into.detail_levels += from.detail_levels;
    into.proxy_levels += from.proxy_levels;
    into.collision_meshes += from.collision_meshes;
    into.tree_blocks += from.tree_blocks;
    into.tree_code += from.tree_code;
    into.build_bytes += from.build_bytes;
    into.bytes += from.bytes;
    into.strict_fail.extend(from.strict_fail);
    for (a, b) in into.max_level.iter_mut().zip(from.max_level) {
        *a = (*a).max(b);
    }
    for (a, b) in into.max_record.iter_mut().zip(from.max_record) {
        *a = (*a).max(b);
    }
    for (k, v) in from.formats {
        *into.formats.entry(k).or_default() += v;
    }
    for (k, v) in from.stamps {
        *into.stamps.entry(k).or_default() += v;
    }
    for (k, v) in from.flags {
        *into.flags.entry(k).or_default() += v;
    }
}

#[test]
fn decode_every_mesh_record() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let mut dids = mesh_dids(&set).expect("list mesh DIDs");
    let limit = env_usize("TESSERA_MESH_LIMIT", usize::MAX);
    dids.truncate(limit);
    assert!(!dids.is_empty(), "no mesh records found");
    let threads = env_usize("TESSERA_THREADS", 8).max(1);
    let next = AtomicUsize::new(0);
    let total = Mutex::new(Census::default());
    let start = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut c = Census::default();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(did) = dids.get(i) else { break };
                    census_one(did.0, &set, &mut c);
                }
                merge(&mut total.lock().unwrap(), c);
            });
        }
    });
    let c = total.into_inner().unwrap();
    let elapsed = start.elapsed();

    println!(
        "mesh census: {} records in {:.1?} ({} threads)",
        c.records, elapsed, threads
    );
    println!(
        "  decoded {} / {} ({:.3} %), {} MiB of payload",
        c.ok,
        c.records,
        100.0 * c.ok as f64 / c.records as f64,
        c.bytes >> 20
    );
    for (cat, (n, samples)) in &c.failures {
        println!("  FAIL {cat}: {n}");
        for (did, msg) in samples {
            println!("    {did:#010x}: {msg}");
        }
    }
    println!(
        "  submeshes {} (skinned {}), vertices {}, triangles {}",
        c.submeshes, c.skinned_submeshes, c.vertices, c.triangles
    );
    println!(
        "  levels: detail {}, proxy {}; collision meshes {}; tree blocks {} (with code {}, build_byte {})",
        c.detail_levels,
        c.proxy_levels,
        c.collision_meshes,
        c.tree_blocks,
        c.tree_code,
        c.build_bytes
    );
    println!("  vertex formats: {}", c.formats.len());
    for (f, n) in &c.formats {
        println!("    {f:#010x}: {n}");
    }
    println!("  stamps: {:?}", c.stamps);
    println!("  distinct flags values: {}", c.flags.len());
    println!(
        "  records with validation issues: {} ; issue kinds: {}",
        c.records_with_issues,
        c.issues.len()
    );
    for (k, (records, count, samples)) in &c.issues {
        let s: Vec<String> = samples.iter().map(|d| format!("{d:#010x}")).collect();
        println!(
            "    {k}: {records} issues, {count} occurrences, e.g. {}",
            s.join(" ")
        );
    }
    println!(
        "  lossless round trip mismatches: {}; fallback rule disagreements: {}; geometry failures (all levels): {}",
        c.roundtrip_mismatch.len(),
        c.fallback_disagrees.len(),
        c.geometry_fail.len()
    );
    for d in c.roundtrip_mismatch.iter().take(5) {
        println!("    round trip {d:#010x}");
    }
    for d in c.fallback_disagrees.iter().take(5) {
        println!("    fallback {d:#010x}");
    }
    for (d, e) in c.geometry_fail.iter().take(5) {
        println!("    geometry {d:#010x}: {e}");
    }

    let [lv, li, ld, lb, la] = c.max_level;
    println!(
        "  largest level: {lv} vertices, {li} indices, {ld} draws, {} KiB of streams; largest amplification {:.3}",
        lb >> 10,
        la as f64 / 1000.0
    );
    let [rs, rl, rh] = c.max_record;
    println!(
        "  largest record: {rs} submeshes, {rl} levels; decoded heap beyond 1.4 × the record at most {rh} bytes"
    );
    println!("  strict decoding failures: {}", c.strict_fail.len());
    for (d, e) in c.strict_fail.iter().take(5) {
        println!("    strict {d:#010x}: {e}");
    }

    let rate = c.ok as f64 / c.records as f64;
    assert!(rate >= 0.99, "decode rate {rate:.4} is below 99 %");
    if limit == usize::MAX {
        // mesh.md §15 / §18: every record decodes.
        assert!(c.failures.is_empty(), "decode failures on a full census");
    }
    // mesh.md §18: the only invariant real data breaks is the documented MI10 overflow.
    let allowed = "MI10 CollisionIndexOverflow";
    assert!(
        c.issues.keys().all(|k| k == allowed),
        "unexpected validation issues: {:?}",
        c.issues.keys().collect::<Vec<_>>()
    );
    assert!(c.strict_fail.is_empty(), "strict decoding rejected records");
    // The default limits keep headroom over everything observed.
    let d = GeometryLimits::default();
    assert!(lv * 4 <= d.max_vertices && li * 4 <= d.max_indices && ld * 4 <= d.max_draws);
    assert!(lb * 4 <= d.max_bytes && la * 4 <= d.max_amplification * 1000);
    assert!(rs * 4 <= u64::from(tessera_mesh::MAX_SUBMESHES));
    assert!(rl * 4 <= u64::from(tessera_mesh::MAX_LEVELS));
    assert!(
        rh <= 1 << 20,
        "decoded heap exceeds 1.4 × the record + 1 MiB"
    );
    assert!(
        c.roundtrip_mismatch.is_empty(),
        "lossless round trip failed"
    );
    assert!(c.geometry_fail.is_empty(), "a level did not flatten");
    assert!(
        c.fallback_disagrees.is_empty(),
        "the §10.4 fallback disagrees with the stamp rule"
    );
}

#[test]
fn library_loads_and_caches() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let set = std::sync::Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open"));
    let pool = WorkerPool::new(PoolConfig {
        workers: 4,
        ..PoolConfig::default()
    });
    let lib = MeshLibrary::new(set, &pool, MeshLibraryConfig::default());
    let dids = lib.mesh_dids().expect("list");
    let step = (dids.len() / 200).max(1);
    let sample: Vec<Did> = dids.iter().step_by(step).copied().collect();
    let handles: Vec<_> = sample
        .iter()
        .map(|&d| lib.request(d, tessera_assets::Priority::Visible))
        .collect();
    let mut ok = 0;
    for h in &handles {
        if h.wait(Duration::from_secs(120)).ready().is_some() {
            ok += 1;
        }
    }
    println!("library: {ok} / {} sampled meshes ready", sample.len());
    assert_eq!(ok, sample.len());
    // A second load is a cache hit and returns the same product.
    let first = lib.load(sample[0]).expect("load");
    let again = lib.load(sample[0]).expect("load again");
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    assert!(lib.cached(sample[0]).is_some());
    let stats = lib.store().stats();
    println!(
        "library stats: hits {}, misses {}, cached {} meshes / {} MiB",
        stats.hits,
        stats.misses,
        stats.cache_entries,
        stats.cache_bytes >> 20
    );
    // A missing DID is a categorised source error.
    let missing = lib.load(Did(0x06FF_FFFF)).unwrap_err();
    assert_eq!(missing.category(), FailureCategory::Source);
}
