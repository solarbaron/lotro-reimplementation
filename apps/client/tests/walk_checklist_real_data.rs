// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M4.5 exit criterion 4, scripted part: every walk of
//! `docs/checklists/m4-5-visual.md` §2.3 (at least 24) runs on the collision walk mode over
//! the user's own install, the way `tessera-client --walk-path` runs it (headless, no GPU),
//! and passes its expected outcome (§2.2) with no capsule penetration deeper than 0.01 m.
//! The per-category minimums of the criterion are checked on the table.
//!
//! Walks that fail for a reason recorded in the OPEN of `m4-5-visual.md` §2.4 (listed in
//! [`KNOWN_OPEN`]) are reported but do not fail the test; any other failure does, and a
//! listed walk that now passes is reported so the list gets shortened.
//! `TESSERA_WALKS_STRICT=1` fails on every failing walk (the criterion itself).
//!
//! Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-client --test walk_checklist_real_data -- --nocapture`.
//! `TESSERA_WALKS=WL1,ST2` runs only the named walks; `TESSERA_WALK_LOG=FILE` writes the
//! movement log (CSV) there (never inside the install). Only walk ids, measured distances
//! and states are printed; no game data leaves the install.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tessera_client::avatar::AvatarSpec;
use tessera_client::runtime::is_inside;
use tessera_client::walk_path::{Category, parse_walk_table};
use tessera_client::walker::movement::run::{
    LOG_HEADER, WalkSession, run_walk, summary_line, write_log,
};

/// Walks failing for the causes listed in the OPEN of `m4-5-visual.md` §2.4 (first run,
/// 2026-10-06; `walker.md` §13.8): blocked-walk end points after sliding round obstacles
/// (WL2, GN2–GN4), no generated body (GN1), steep starts and ends (CV1, CV2) and the
/// 0.05 m rule on a slope (FR1). ST2 and FR3 (controller faults) pass since the M4.5
/// verification, SL1–SL3 (state 6 at steep bands) since 2026-10-07. Shorten it as the
/// causes are settled.
const KNOWN_OPEN: [&str; 8] = ["WL2", "GN1", "GN2", "GN3", "GN4", "CV1", "CV2", "FR1"];

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn checklist() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/checklists/m4-5-visual.md"
    );
    std::fs::read_to_string(path).expect("the M4.5 checklist")
}

#[test]
fn table_meets_the_criterion_minimums() {
    let walks = parse_walk_table(&checklist()).expect("walk table");
    assert!(walks.len() >= 24, "{} walks", walks.len());
    let mut per: BTreeMap<Category, usize> = BTreeMap::new();
    for w in &walks {
        *per.entry(w.category).or_default() += 1;
    }
    let need = [
        (Category::Wall, 4),
        (Category::Undrawn, 2),
        (Category::Generated, 3),
        (Category::Step, 3),
        (Category::Steep, 2),
        (Category::Interior, 3),
        (Category::Cave, 2),
        (Category::Water, 3),
        (Category::Frill, 2),
    ];
    for (c, n) in need {
        assert!(per.get(&c).copied().unwrap_or(0) >= n, "{c:?}: {per:?}");
    }
}

#[test]
fn every_checklist_walk_passes() {
    let Some(dir) = install() else { return };
    let mut walks = parse_walk_table(&checklist()).expect("walk table");
    if let Ok(only) = std::env::var("TESSERA_WALKS") {
        let ids: Vec<&str> = only.split(',').map(str::trim).collect();
        walks.retain(|w| ids.contains(&w.id.as_str()));
    }
    let started = Instant::now();
    let session = WalkSession::open(&dir, &AvatarSpec::default(), &[1], 0).expect("session");
    eprintln!(
        "session ready in {:.1} s: capsule {:?} r {:.3} h {:.3}, float height {:.3}, notes {:?}",
        started.elapsed().as_secs_f64(),
        session.setup.shape_source,
        session.setup.shape.radius(),
        session.setup.shape.height(),
        session.setup.record.float_height(&session.setup.conditions),
        session.setup.notes
    );
    let mut world = session.world().expect("streamer");
    let mut log = std::env::var_os("TESSERA_WALK_LOG").map(|p| {
        let p = PathBuf::from(p);
        // Resolved (relative paths, `..`, symbolic links), as `--walk-log` is checked.
        assert!(
            !is_inside(&p, &dir),
            "the log must not be written into the install"
        );
        let mut f = std::io::BufWriter::new(std::fs::File::create(&p).expect("log file"));
        std::io::Write::write_all(&mut f, format!("{LOG_HEADER}\n").as_bytes()).unwrap();
        f
    });
    let mut failed = Vec::new();
    for walk in &walks {
        let t = Instant::now();
        match run_walk(walk, &mut world, &session.setup, Duration::from_secs(300)) {
            Ok(out) => {
                if let Some(f) = &mut log {
                    write_log(f, &out).expect("log");
                }
                eprintln!(
                    "{} ({:.1} s)",
                    summary_line(&out),
                    t.elapsed().as_secs_f64()
                );
                if !out.verdict.passed {
                    failed.push(walk.id.clone());
                }
            }
            Err(e) => {
                eprintln!("{} NOT RUN: {e}", walk.id);
                failed.push(walk.id.clone());
            }
        }
    }
    eprintln!(
        "{} walks, {} failed {:?}, {:.1} s",
        walks.len(),
        failed.len(),
        failed,
        started.elapsed().as_secs_f64()
    );
    let ran: Vec<&str> = walks.iter().map(|w| w.id.as_str()).collect();
    let now_passing: Vec<&str> = KNOWN_OPEN
        .iter()
        .copied()
        .filter(|id| ran.contains(id) && !failed.iter().any(|f| f == id))
        .collect();
    if !now_passing.is_empty() {
        eprintln!("listed in KNOWN_OPEN but passing now (shorten the list): {now_passing:?}");
    }
    let strict = std::env::var_os("TESSERA_WALKS_STRICT").is_some_and(|v| v == "1");
    let unexpected: Vec<&String> = failed
        .iter()
        .filter(|id| strict || !KNOWN_OPEN.contains(&id.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "failed walks not listed in KNOWN_OPEN (m4-5-visual.md §2.4): {unexpected:?}"
    );
}
