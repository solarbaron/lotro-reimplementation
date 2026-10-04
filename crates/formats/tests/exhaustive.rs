// SPDX-License-Identifier: GPL-3.0-or-later
//! M0 exit criterion 4 against a user's own install (read-only): **every** string-table
//! record of the local archive decodes with no error and no trailing bytes, **every**
//! render-surface (texture) record header decodes, and the property dictionary decodes.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Unlike `real_data.rs` nothing is
//! sampled in a release build. The surface pass reads ~18 GiB of stored data, in parallel;
//! a debug build (slow zlib) samples every 97th surface unless `TESSERA_FULL=1` is set, so
//! run with `cargo test --release -p tessera-formats --test exhaustive`. Only counts are
//! printed; no game data leaves the install.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tessera_dat::{Archive, ArchiveSet, Entry, SetOptions};
use tessera_formats::{
    Did, FormatError, PropertyDictionary, PropertyType, Record, StringTable, Surface,
};

const STRING_TABLE: u8 = 0x25;
const RENDER_SURFACE: u8 = 0x41;
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

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

/// Whether every record is visited (release builds, or `TESSERA_FULL=1`).
fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Runs `f` over `items` on every core; returns the per-item results in no fixed order.
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

/// Every entry of `archive` with the given DID high byte.
fn entries_of(archive: &Archive, prefix: u8) -> Vec<Entry> {
    let lo = u32::from(prefix) << 24;
    archive
        .entries_range(lo..=(lo | 0x00FF_FFFF))
        .map(|e| e.expect("directory walk"))
        .collect()
}

#[test]
fn every_string_table_decodes_without_trailing_bytes() {
    let Some(set) = open_set() else { return };
    let t = Instant::now();
    let locals: Vec<_> = set
        .members()
        .iter()
        .filter(|m| m.id.to_string().starts_with("local_"))
        .collect();
    assert!(!locals.is_empty(), "no local archive");
    let mut tables = 0usize;
    let mut strings = 0usize;
    let mut trailing = Vec::new();
    let mut errors = Vec::new();
    for m in &locals {
        let entries = entries_of(&m.archive, STRING_TABLE);
        let results = par_each(&entries, |e| {
            let payload = m.archive.read_entry(e).map_err(|err| err.to_string())?;
            StringTable::parse_expecting(&payload, e.did)
                .map(|t| t.len())
                .map_err(|err| match err {
                    FormatError::TrailingBytes { count, .. } => format!("trailing:{count}"),
                    err => err.to_string(),
                })
                .map_err(|msg| format!("{:#010x}: {msg}", e.did))
        });
        for r in results {
            match r {
                Ok(n) => {
                    tables += 1;
                    strings += n;
                }
                Err(msg) if msg.contains("trailing:") => trailing.push(msg),
                Err(msg) => errors.push(msg),
            }
        }
        eprintln!("{}: {} string tables", m.id, entries.len());
    }
    for msg in trailing.iter().chain(&errors).take(20) {
        eprintln!("  {msg}");
    }
    eprintln!(
        "criterion 4a: {tables} string tables ({strings} strings) decoded; {} with trailing \
         bytes, {} other errors; {:?}",
        trailing.len(),
        errors.len(),
        t.elapsed()
    );
    assert!(tables > 0);
    assert!(trailing.is_empty(), "string tables with trailing bytes");
    assert!(errors.is_empty(), "string tables that fail to decode");
}

#[test]
fn every_render_surface_header_decodes() {
    let Some(set) = open_set() else { return };
    let t = Instant::now();
    let full = exhaustive();
    let mut work: Vec<(usize, Entry)> = Vec::new();
    let mut per_archive = Vec::new();
    for (idx, m) in set.members().iter().enumerate() {
        let entries = entries_of(&m.archive, RENDER_SURFACE);
        if entries.is_empty() {
            continue;
        }
        per_archive.push((m.id.to_string(), entries.len()));
        work.extend(
            entries
                .into_iter()
                .enumerate()
                .filter(|(i, _)| full || i % 97 == 0)
                .map(|(_, e)| (idx, e)),
        );
    }
    let results = par_each(&work, |(idx, e)| {
        let m = &set.members()[*idx];
        let payload = m
            .archive
            .read_entry(e)
            .map_err(|err| format!("{} {:#010x}: read: {err}", m.id, e.did))?;
        let s = Surface::parse_expecting(&payload, e.did)
            .map_err(|err| format!("{} {:#010x}: {err}", m.id, e.did))?;
        Ok::<_, String>((s.format, payload.len() as u64))
    });
    let mut formats: BTreeMap<String, usize> = BTreeMap::new();
    let mut bytes = 0u64;
    let mut errors = Vec::new();
    for r in results {
        match r {
            Ok((f, n)) => {
                *formats.entry(format!("{f} ({})", f.code())).or_default() += 1;
                bytes += n;
            }
            Err(msg) => errors.push(msg),
        }
    }
    for msg in errors.iter().take(20) {
        eprintln!("  {msg}");
    }
    eprintln!("surfaces per archive: {per_archive:?}");
    eprintln!("surface formats: {formats:?}");
    eprintln!(
        "criterion 4b: {} of {} render surfaces decoded ({}), {bytes} payload bytes, {} errors; {:?}",
        work.len() - errors.len(),
        per_archive.iter().map(|(_, n)| n).sum::<usize>(),
        if full { "every record" } else { "debug sample" },
        errors.len(),
        t.elapsed()
    );
    assert!(!work.is_empty());
    assert!(errors.is_empty(), "render surfaces that fail to decode");
}

#[test]
fn property_dictionary_decodes() {
    let Some(set) = open_set() else { return };
    let (located, payload) = set
        .read_with_entry(PROPERTY_DICTIONARY)
        .expect("read the property dictionary through routing");
    let dict = PropertyDictionary::parse_expecting(&payload, Did(PROPERTY_DICTIONARY))
        .expect("decode the property dictionary");
    let mut types: BTreeMap<String, usize> = BTreeMap::new();
    for d in dict.definitions() {
        *types.entry(format!("{:?}", d.ty)).or_default() += 1;
    }
    let unknown = dict
        .definitions()
        .iter()
        .filter(|d| matches!(d.ty, PropertyType::Unknown(_)))
        .count();
    eprintln!("property definition types: {types:?}");
    eprintln!(
        "criterion 4c: property dictionary ({} bytes from {}) decoded: {} names, {} definitions, \
         {unknown} of unknown type",
        payload.len(),
        set.member(located.member).id,
        dict.names().len(),
        dict.definitions().len()
    );
    assert!(!dict.names().is_empty());
    assert!(!dict.definitions().is_empty());
    assert_eq!(unknown, 0);
}
