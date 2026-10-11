// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2 exit criterion 1 without an install (`combat-ui.md` §14, §16.2): the
//! reference scenario of §17.1 (`tests/scenarios/combat-ui.scenario`) plays headless on its
//! virtual clock with `--no-data`; at every checkpoint of §17.1 the elements §14 lists are
//! shown, read from the UI tree; at five checkpoints the UI is rendered on the headless
//! renderer and compared with the committed synthetic goldens (ARCHITECTURE §23.3,
//! per-pixel tolerance); the run writes the committed canonical log byte for byte; two
//! runs give the same element states (CUI9).
//!
//! Regenerate the goldens with `TESSERA_BLESS=1 cargo test -p tessera-client --test
//! combat_ui_golden`. The rendering part skips (reported as passed) without a GPU adapter.

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::combat::headless::{CombatScreens, Element, REFERENCE_CHECKPOINTS, play};
use tessera_client::scenario::{InstallChoice, headless_config, prepare};

const SCREEN: (u32, u32) = (1024, 768);

/// The checkpoints rendered and compared with goldens (ms, file).
const GOLDEN: [(u64, &str); 5] = [
    (1200, "combat_ui_1_2.png"),
    (3100, "combat_ui_3_1.png"),
    (3400, "combat_ui_3_4.png"),
    (4300, "combat_ui_4_3.png"),
    (8100, "combat_ui_8_1.png"),
];

fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/combat-ui.scenario")
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

/// What one run gives: the shown elements by checkpoint, the rendered checkpoints, the log.
type RunResult = (
    BTreeMap<u64, Vec<(Element, bool)>>,
    Vec<(u64, tessera_render::RgbaImage)>,
    Vec<u8>,
);

fn ms(t: f64) -> u64 {
    (t * 1000.0).round() as u64
}

/// Plays the reference scenario; returns the shown elements at each checkpoint, the
/// rendered checkpoints and the log.
fn run(mut renderer: Option<&mut tessera_render::Renderer<'static>>) -> RunResult {
    let path = scenario_path();
    let args = Args::try_parse_from([
        "tessera-client",
        "--headless",
        "--no-data",
        "--scenario",
        path.to_str().expect("utf-8"),
    ])
    .expect("args");
    let sc = Arc::new(prepare(&args, &InstallChoice::None).expect("prepares"));
    let config = headless_config(&args, Arc::clone(&sc)).expect("config");
    let mut screens = CombatScreens::without_install(SCREEN);
    let mut shown = BTreeMap::new();
    let mut images = Vec::new();
    let checkpoints: BTreeMap<u64, &[Element]> = REFERENCE_CHECKPOINTS
        .iter()
        .map(|(t, e)| (ms(*t), *e))
        .collect();
    let (outcome, log) = play(config, sc, Some(Vec::new()), &mut screens, |at, s| {
        let Some(elements) = checkpoints.get(&at) else {
            return;
        };
        s.settle(renderer.as_deref_mut());
        let states: Vec<(Element, bool)> = elements.iter().map(|e| (*e, s.is_shown(*e))).collect();
        shown.insert(at, states);
        if let Some(r) = renderer.as_deref_mut()
            && GOLDEN.iter().any(|(t, _)| *t == at)
        {
            images.push((at, s.render(r).expect("renders")));
        }
    })
    .expect("sandbox");
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    (shown, images, log.expect("log"))
}

#[test]
fn every_element_shows_at_its_checkpoint_and_matches_the_goldens() {
    let mut r = gpu::headless_renderer(tessera_client::combat::headless::CLEAR);
    let (shown, images, log) = run(r.as_deref_mut());
    // §14: every element at its checkpoint.
    let mut missing = Vec::new();
    for (t, states) in &shown {
        for (e, ok) in states {
            if !ok {
                missing.push(format!("{e:?} at {}", *t as f64 / 1000.0));
            }
        }
    }
    assert_eq!(
        shown.len(),
        REFERENCE_CHECKPOINTS.len(),
        "every checkpoint ran"
    );
    assert!(missing.is_empty(), "elements not shown: {missing:?}");
    // §16.2: the run writes the committed log.
    let committed = std::fs::read(scenario_path().with_extension("log")).expect("log");
    assert_eq!(
        String::from_utf8_lossy(&log),
        String::from_utf8_lossy(&committed),
        "canonical log"
    );
    // ARCHITECTURE §23.3: the synthetic goldens.
    if r.is_none() {
        return;
    }
    assert_eq!(images.len(), GOLDEN.len());
    for ((at, img), (t, name)) in images.iter().zip(GOLDEN) {
        assert_eq!(*at, t);
        golden::check(&golden_path(name), img, golden::Tolerance::default());
    }
}

#[test]
fn two_runs_give_the_same_element_states() {
    let (a, _, la) = run(None);
    let (b, _, lb) = run(None);
    assert_eq!(a, b, "CUI9");
    assert_eq!(la, lb);
}
