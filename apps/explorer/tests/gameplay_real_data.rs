// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data smoke test of the gameplay view's worker side: cards for a sample of game
//! objects, every encounter table, and samples of particle and fork systems and of the
//! 0x0E tables, built headlessly against the owner's install.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. The install is only
//! read. Output is counts per family and kind (Tessera's own names), never game content.
//! Run in release mode: `cargo test --release -p tessera-explorer --test gameplay_real_data
//! -- --nocapture`. `TESSERA_EXPLORER_FULL=1` builds every object card instead of a sample.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_assets::JobContext;
use tessera_explorer::ExplorerError;
use tessera_explorer::data::DataContext;
use tessera_explorer::gameplay::{Family, GameplayDecoder};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn full() -> bool {
    std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1")
}

/// Every DID with `prefix` in the set (sorted, deduplicated).
fn dids(ctx: &DataContext, prefix: u8) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo + (1 << 24);
    let mut out = Vec::new();
    for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..hi) {
            out.push(e.expect("directory walk").did);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every `n`-th element, so that about `want` remain.
fn sample(v: &[u32], want: usize) -> Vec<u32> {
    let step = (v.len() / want.max(1)).max(1);
    v.iter().copied().step_by(step).collect()
}

#[derive(Debug, Default)]
struct Tally {
    built: usize,
    rows: usize,
    by_kind: BTreeMap<String, usize>,
    by_family: BTreeMap<&'static str, usize>,
    failed: BTreeMap<String, usize>,
    /// Card notes by their first words (Tessera's own messages and error kinds).
    notes: BTreeMap<String, usize>,
}

impl Tally {
    fn add(&mut self, r: Result<tessera_explorer::gameplay::GameplayProduct, ExplorerError>) {
        match r {
            Ok(p) => {
                self.built += 1;
                self.rows += p.sections.iter().map(|s| s.rows.len()).sum::<usize>();
                *self.by_family.entry(p.family.name()).or_default() += 1;
                *self.by_kind.entry(p.kind.clone()).or_default() += 1;
                for n in &p.notes {
                    let key: String = n
                        .split(|c: char| c == ':' || c == '(' || c.is_ascii_digit())
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(40)
                        .collect();
                    *self.notes.entry(key).or_default() += 1;
                }
            }
            Err(e) => {
                let key: String = e.to_string().chars().take(60).collect();
                *self.failed.entry(key).or_default() += 1;
            }
        }
    }
}

#[test]
fn gameplay_cards() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open the install read-only"));
    let decoder = GameplayDecoder::new(Arc::clone(&ctx));
    let job = JobContext::detached();

    let t = Instant::now();
    let objects = dids(&ctx, 0x70);
    let picked = if full() {
        objects.clone()
    } else {
        sample(&objects, 3_000)
    };
    let mut tally = Tally::default();
    for &did in &picked {
        tally.add(decoder.card(did, &job));
    }
    eprintln!(
        "objects: {} of {} in {} ms: built {}, rows {}",
        picked.len(),
        objects.len(),
        t.elapsed().as_millis(),
        tally.built,
        tally.rows,
    );
    eprintln!("  notes: {:?}", tally.notes);
    eprintln!("  by family: {:?}", tally.by_family);
    eprintln!("  by kind: {:?}", tally.by_kind);
    eprintln!("  failed: {:?}", tally.failed);
    assert_eq!(
        tally.built,
        picked.len(),
        "every object gets a card (unrecognised ones a property card)"
    );
    for f in [Family::Item, Family::Combat, Family::Creature] {
        assert!(
            tally.by_family.contains_key(f.name()),
            "the sample holds {} cards",
            f.name()
        );
    }

    // Property-set DIDs give the card of their object.
    if let Some(&first) = objects.first() {
        let via_set = decoder
            .card(first + 0x0900_0000, &job)
            .expect("card via the set");
        assert_eq!(via_set.object, Some(first));
    }

    for (prefix, want) in [
        (0x14u8, usize::MAX),
        (0x39, 2_000),
        (0x44, 500),
        (0x0E, 400),
    ] {
        let all = dids(&ctx, prefix);
        let picked = if full() || want == usize::MAX {
            all.clone()
        } else {
            sample(&all, want)
        };
        let t = Instant::now();
        let mut tally = Tally::default();
        for &did in &picked {
            tally.add(decoder.card(did, &job));
        }
        eprintln!(
            "{prefix:#04x}: {} of {} in {} ms: built {}, rows {}, kinds {:?}, failed {:?}",
            picked.len(),
            all.len(),
            t.elapsed().as_millis(),
            tally.built,
            tally.rows,
            tally.by_kind,
            tally.failed
        );
        if prefix != 0x0E {
            // Every encounter table, particle and fork system decodes (the decoders'
            // own real-data tests hold the same bar).
            assert_eq!(tally.built, picked.len(), "{prefix:#04x} cards");
        }
    }
}
