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

/// M0 exit criterion 3: every DID of every archive is found **through routing** in the same
/// archive the directory walk found it in, and reading it through the set succeeds.
///
/// Every non-metadata entry of every archive is located (metadata DIDs ≥ 0xFFFF0000 exist
/// once per archive and are not routable by design, spec §8). Reading every record would
/// move ~37 GiB, so reads through the set cover a sample per archive: the first and last
/// entry plus every `READ_STRIDE`-th one. Set `TESSERA_FULL=1` to read every record too.
///
/// Archives are checked in parallel; `--release` takes ~1 s, a debug build ~30 s.
#[test]
fn routing_resolves_every_entry_to_its_listing_archive() {
    const READ_STRIDE: usize = 997;
    let Some(set) = open_set() else { return };
    let full = std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1");
    let t = Instant::now();

    #[derive(Default)]
    struct Tally {
        listed: u64,
        metadata: u64,
        located: u64,
        unrouted: Vec<u32>,
        elsewhere: Vec<(u32, usize)>,
        missing: Vec<u32>,
        read: u64,
        read_bytes: u64,
        read_errors: Vec<(u32, String)>,
    }

    let members = set.members();
    let tallies: Vec<Tally> = std::thread::scope(|s| {
        let handles: Vec<_> = members
            .iter()
            .enumerate()
            .map(|(idx, m)| {
                let set = &set;
                s.spawn(move || {
                    let mut t = Tally::default();
                    let entries: Vec<_> = m
                        .archive
                        .entries()
                        .collect::<Result<_, _>>()
                        .unwrap_or_else(|e| panic!("{}: directory walk: {e}", m.id));
                    let last = entries.len().saturating_sub(1);
                    for (i, e) in entries.iter().enumerate() {
                        t.listed += 1;
                        if e.did >= METADATA_DID_BASE {
                            t.metadata += 1;
                            continue;
                        }
                        let located = match set.locate(e.did) {
                            Ok(Some(l)) => l,
                            Ok(None) => {
                                t.missing.push(e.did);
                                continue;
                            }
                            Err(err) => {
                                t.read_errors.push((e.did, format!("locate: {err}")));
                                continue;
                            }
                        };
                        t.located += 1;
                        if !located.routed {
                            t.unrouted.push(e.did);
                        }
                        if located.member != idx {
                            t.elsewhere.push((e.did, located.member));
                            continue;
                        }
                        assert_eq!(located.entry, *e, "{}: {:#010x}", m.id, e.did);
                        if full || i == 0 || i == last || i % READ_STRIDE == 0 {
                            match set.read_with_entry(e.did) {
                                Ok((l, bytes)) => {
                                    assert_eq!(l.member, idx);
                                    if !e.is_compressed() {
                                        assert_eq!(bytes.len() as u64, u64::from(e.length));
                                    }
                                    t.read += 1;
                                    t.read_bytes += bytes.len() as u64;
                                }
                                Err(err) => t.read_errors.push((e.did, err.to_string())),
                            }
                        }
                    }
                    t
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect()
    });

    let mut total = Tally::default();
    for (m, t) in members.iter().zip(&tallies) {
        eprintln!(
            "  {:<14} listed {:>7}, metadata {:>2}, routed+same archive {:>7}, read via set {:>4} ({} bytes)",
            m.id.to_string(),
            t.listed,
            t.metadata,
            t.located - t.elsewhere.len() as u64 - t.unrouted.len() as u64,
            t.read,
            t.read_bytes
        );
        for d in t.unrouted.iter().take(5) {
            eprintln!("    {d:#010x}: found only by the fallback search");
        }
        for (d, other) in t.elsewhere.iter().take(5) {
            eprintln!(
                "    {d:#010x}: routing answered from {}",
                members[*other].id
            );
        }
        for d in t.missing.iter().take(5) {
            eprintln!("    {d:#010x}: not found through the set");
        }
        for (d, err) in t.read_errors.iter().take(5) {
            eprintln!("    {d:#010x}: {err}");
        }
        total.listed += t.listed;
        total.metadata += t.metadata;
        total.located += t.located;
        total.read += t.read;
        total.read_bytes += t.read_bytes;
        total.unrouted.extend(&t.unrouted);
        total.elsewhere.extend(&t.elsewhere);
        total.missing.extend(&t.missing);
        total.read_errors.extend(t.read_errors.iter().cloned());
    }
    eprintln!(
        "criterion 3: {} archives, {} entries listed ({} metadata skipped), {} located; \
         {} unrouted, {} answered by another archive, {} missing; {} read via the set \
         ({} bytes), {} errors; {:?}",
        members.len(),
        total.listed,
        total.metadata,
        total.located,
        total.unrouted.len(),
        total.elsewhere.len(),
        total.missing.len(),
        total.read,
        total.read_bytes,
        total.read_errors.len(),
        t.elapsed()
    );
    assert!(total.missing.is_empty(), "DIDs not found through the set");
    assert!(total.read_errors.is_empty(), "locate/read errors");
    assert!(
        total.elsewhere.is_empty(),
        "routing answered from a different archive"
    );
    assert!(total.unrouted.is_empty(), "DIDs found only by fallback");
    assert_eq!(total.located, total.listed - total.metadata);
}
