// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests against a real, user-owned install. Skipped unless `TESSERA_GAME_DIR` is set.
//!
//! Everything here is read-only: the install is opened with positional reads and nothing
//! is ever written to it. No game data is copied into the repository or the test output
//! beyond counts and timings.

use std::path::PathBuf;
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet, METADATA_DID_BASE, SetOptions, ValidateOptions};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
            None
        }
    }
}

fn open_set() -> Option<ArchiveSet> {
    let dir = game_dir()?;
    let t = Instant::now();
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    eprintln!(
        "opened {} archives in {:?}",
        set.members().len(),
        t.elapsed()
    );
    for p in set.problems() {
        eprintln!("  set problem: {:?}: {}", p.path, p.message);
    }
    Some(set)
}

#[test]
fn general_iterate_read_validate() {
    let Some(set) = open_set() else { return };
    let general = set
        .find(&ArchiveKind::General, 0)
        .expect("client_general.dat present");
    let a = &general.archive;

    let t = Instant::now();
    let entries: Vec<_> = a
        .entries()
        .collect::<Result<_, _>>()
        .expect("iterate general");
    let iter_time = t.elapsed();
    assert!(!entries.is_empty());
    assert!(entries.windows(2).all(|w| w[0].did < w[1].did));
    eprintln!(
        "bench(real): iterated {} entries of client_general.dat in {iter_time:?}",
        entries.len()
    );

    // Range iteration agrees with the full walk for every DID class present.
    let t = Instant::now();
    let mut prefixes: Vec<u32> = entries.iter().map(|e| e.did >> 24).collect();
    prefixes.dedup();
    for p in prefixes {
        let lo = p << 24;
        let hi = lo.checked_add(1 << 24);
        let want = entries.iter().filter(|e| e.did >> 24 == p).map(|e| e.did);
        let got: Vec<u32> = match hi {
            Some(hi) => a.entries_range(lo..hi).map(|e| e.unwrap().did).collect(),
            None => a.entries_from(lo).map(|e| e.unwrap().did).collect(),
        };
        assert!(got.iter().copied().eq(want), "class {p:#04x}");
    }
    eprintln!("range-iterated every class of general in {:?}", t.elapsed());

    // Read a sample, plus every fragmented record.
    let t = Instant::now();
    let (mut read, mut bytes, mut fragmented) = (0u64, 0u64, 0u64);
    for (i, e) in entries.iter().enumerate() {
        let layout = a.layout(e).expect("layout");
        let frag = layout.extra_pieces() > 0;
        fragmented += u64::from(frag);
        if frag || i % 13 == 0 {
            let data = a.read_entry(e).expect("read general record");
            if !e.is_compressed() {
                assert_eq!(data.len() as u64, u64::from(e.length));
            }
            read += 1;
            bytes += data.len() as u64;
        }
    }
    eprintln!(
        "read {read} general records ({bytes} bytes, {fragmented} fragmented) in {:?}",
        t.elapsed()
    );

    let t = Instant::now();
    let r = a.validate_with(&ValidateOptions {
        inflate_compressed: true,
        ..ValidateOptions::default()
    });
    eprintln!(
        "validated general in {:?}: {} errors, {} warnings, stats {:?}",
        t.elapsed(),
        r.errors(),
        r.warnings(),
        r.stats
    );
    for i in &r.issues {
        eprintln!("  {i}");
    }
    assert!(r.is_ok());
    assert_eq!(r.stats.entries, entries.len() as u64);
}

#[test]
fn compressed_samples_inflate() {
    let Some(set) = open_set() else { return };
    let mut total = 0u64;
    for m in set.members() {
        let t = Instant::now();
        let mut n = 0u64;
        let mut inflated = 0u64;
        for e in m.archive.entries() {
            let e = e.expect("iterate");
            if !e.is_compressed() {
                continue;
            }
            n += 1;
            if n % 97 != 1 {
                continue;
            }
            let data = m.archive.read_entry(&e).expect("inflate");
            inflated += data.len() as u64;
            total += 1;
            if n > 97 * 200 {
                break;
            }
        }
        if n > 0 {
            eprintln!(
                "{}: inflated sample of compressed records ({inflated} bytes) in {:?}",
                m.id,
                t.elapsed()
            );
        }
    }
    eprintln!("inflated {total} compressed records in total");
}

#[test]
fn routing_finds_every_sampled_did_in_its_archive() {
    let Some(set) = open_set() else { return };
    let (mut checked, mut unrouted) = (0u64, 0u64);
    for (idx, m) in set.members().iter().enumerate() {
        let t = Instant::now();
        let mut count = 0u64;
        for (i, e) in m.archive.entries().enumerate() {
            let e = e.expect("iterate");
            count += 1;
            if i % 101 != 0 || e.did >= METADATA_DID_BASE {
                continue;
            }
            let l = set.locate(e.did).expect("locate").expect("found");
            assert_eq!(
                l.member, idx,
                "DID {:#010x} found in another archive",
                e.did
            );
            checked += 1;
            if !l.routed {
                unrouted += 1;
                eprintln!("  {:#010x} in {} found only by fallback", e.did, m.id);
            }
        }
        eprintln!(
            "bench(real): {} — {count} entries iterated+sampled in {:?}",
            m.id,
            t.elapsed()
        );
    }
    eprintln!("routing: {checked} DIDs checked, {unrouted} needed the fallback");
    assert_eq!(unrouted, 0);
}

#[test]
fn validate_whole_set() {
    let Some(set) = open_set() else { return };
    let t = Instant::now();
    let r = set.validate(&ValidateOptions::default());
    eprintln!(
        "validated {} archives in {:?}",
        r.archives.len(),
        t.elapsed()
    );
    let mut entries = 0;
    for (id, a) in &r.archives {
        entries += a.stats.entries;
        eprintln!(
            "  {id}: {} entries, {} nodes, depth {}, {} compressed, {} fragmented, {} errors, {} warnings",
            a.stats.entries,
            a.stats.nodes,
            a.stats.max_depth,
            a.stats.compressed,
            a.stats.fragmented,
            a.errors(),
            a.warnings()
        );
        for i in &a.issues {
            eprintln!("    {i}");
        }
    }
    for i in &r.issues {
        eprintln!("  set: {i}");
    }
    eprintln!(
        "total entries {entries}, duplicate DIDs {}",
        r.duplicate_dids
    );
    assert!(r.is_ok());
    assert_eq!(r.duplicate_dids, 0);
}
