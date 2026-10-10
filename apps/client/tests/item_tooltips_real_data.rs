// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P1 exit criterion 2 on the user's install (read-only): every item
//! record's hover tooltip is modelled (`tessera-gamedata`), converted by the client
//! (`character::tooltip`) and laid out headless in the game UI's examination tooltip panel
//! (`tessera-ui`), and each record ends in exactly one category of `item-tooltips.md` §12.
//!
//! A failure is a panic, an error escaping the categories, a tooltip whose model was built
//! but which the panel could not lay out (no frame, an entry template that could not be
//! created, an empty frame), or a `Rendered` count other than the number of records with a
//! valid hover pair. Prints counts only, never game text.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test item_tooltips_real_data
//! -- --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_client::character::CharacterData;
use tessera_client::character::tooltip::convert;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::Family;
use tessera_gamedata::tooltip::{Category, Census, Purpose, TooltipData, TooltipRequest, Viewer};
use tessera_ui::game::examine::{ExamKey, ExamTooltips};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Laid-out results of the panel over all records.
#[derive(Debug, Default)]
struct Layout {
    built: u64,
    failed: u64,
    template_failures: u64,
    rows_kept: u64,
    max_size: (i32, i32),
    examples: Vec<String>,
}

#[test]
fn every_item_tooltip_is_modelled_laid_out_and_categorised() {
    let Some(dir) = game_dir() else { return };
    let t0 = Instant::now();
    let data = CharacterData::open(&dir).expect("character data");
    let items = data.store().family(Family::Item).expect("item family");
    eprintln!(
        "{} item records ({:?} to open and index)",
        items.len(),
        t0.elapsed()
    );

    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let vocab = booted.char_vocab.clone();
    let pin_layout = booted.char_ids.examination_layout;
    let host = UiHost::from_boot(
        booted,
        &UiOptions {
            screen: (1920, 1080),
            hud: false,
            ..UiOptions::default()
        },
    );
    let mut tips = ExamTooltips::new(vocab);
    if let Some(l) = pin_layout {
        tips.set_pin_layout(l.0);
    }

    let engine = data.engine();
    let viewer = Viewer::default();
    let mut census = Census::default();
    let mut layout = Layout::default();
    let mut expected = 0u64;
    let t1 = Instant::now();
    for (n, &did) in items.iter().enumerate() {
        // The category of §12 (the engine's classification of the hover tooltip).
        let outcome = engine.classify(did, &viewer);
        census.add(did, &outcome);
        // The independent count of records with a valid hover pair (§12 failure rule).
        if let Ok(Some(set)) = engine.data().record(did)
            && let Some(p) = engine.pair(&set, Purpose::Hover)
            && engine
                .data()
                .layout(p.layout)
                .ok()
                .flatten()
                .is_some_and(|l| l.top_level(p.element).is_some())
        {
            expected += 1;
        }
        if outcome.category != Ok(Category::Rendered) {
            continue;
        }
        // Model → panel input → laid out headless.
        let model = engine
            .build(&TooltipRequest::hover(did), &viewer)
            .unwrap_or_else(|e| panic!("{did}: classified Rendered but did not build: {e}"));
        let Some(input) = convert(
            &data,
            &model,
            ExamKey {
                instance: 0,
                record: did.0,
            },
        ) else {
            layout.failed += 1;
            if layout.examples.len() < 16 {
                layout.examples.push(format!("{did}: not converted"));
            }
            continue;
        };
        let mut ui = host.ui_mut();
        match tips.build(&mut ui, &input) {
            Ok(b) => {
                layout.built += 1;
                layout.rows_kept += b.rows.len() as u64;
                layout.max_size.0 = layout.max_size.0.max(b.size.0);
                layout.max_size.1 = layout.max_size.1.max(b.size.1);
                if !b.failures.is_empty() || b.size.0 <= 0 || b.size.1 <= 0 {
                    layout.template_failures += 1;
                    if layout.examples.len() < 16 {
                        layout.examples.push(format!(
                            "{did}: size {:?}, {} template failures",
                            b.size,
                            b.failures.len()
                        ));
                    }
                }
                ui.destroy(b.frame);
            }
            Err(e) => {
                layout.failed += 1;
                if layout.examples.len() < 16 {
                    layout.examples.push(format!("{did}: {e}"));
                }
            }
        }
        if n % 20_000 == 0 && n > 0 {
            eprintln!("  {n} records, {:?}", t1.elapsed());
        }
    }
    eprintln!("modelled and laid out in {:?}", t1.elapsed());
    for (k, v) in &census.categories {
        eprintln!("  {k:?}: {v}");
    }
    eprintln!(
        "  escaped: {}, unnamed: {}, deferred rows: {}",
        census.escaped, census.unnamed, census.deferred
    );
    let mut rows: BTreeMap<&str, u64> = BTreeMap::new();
    for (label, n) in &census.non_empty {
        rows.insert(label, *n);
    }
    for (label, n) in rows {
        eprintln!("  non-empty {label}: {n}");
    }
    eprintln!(
        "  laid out: {} (rows kept {}, largest {:?}), not laid out: {}, with template failures: {}",
        layout.built, layout.rows_kept, layout.max_size, layout.failed, layout.template_failures
    );
    for e in census.examples.iter().chain(&layout.examples) {
        eprintln!("  example: {e}");
    }

    // Every record in exactly one category, none escaped.
    let total: u64 = [
        Category::Rendered,
        Category::NoTooltip,
        Category::LayoutUnreadable,
        Category::ElementMissing,
    ]
    .iter()
    .map(|c| census.count(*c))
    .sum::<u64>()
        + census.escaped;
    assert_eq!(
        total,
        items.len() as u64,
        "every record is categorised once"
    );
    assert_eq!(census.escaped, 0, "errors escaped the categories");
    assert_eq!(census.count(Category::LayoutUnreadable), 0);
    assert_eq!(census.count(Category::ElementMissing), 0);
    assert_eq!(census.count(Category::RowFailure), 0, "row failures");
    assert_eq!(
        census.count(Category::Rendered),
        expected,
        "Rendered equals the records with a valid hover pair"
    );
    // Every rendered tooltip was laid out by the panel.
    assert_eq!(layout.built, census.count(Category::Rendered));
    assert_eq!(layout.failed, 0, "tooltips the panel could not build");
    assert_eq!(
        layout.template_failures, 0,
        "tooltips with template failures"
    );
}
