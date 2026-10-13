// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2 exit criterion 1 on the user's install (read-only; `combat-ui.md`
//! §16.1): the combat layouts and vocabulary resolve; the floating-text settings and the
//! avoidance tokens are found; the reference scenario of §17.1 plays headless with the
//! install's panels and, at every checkpoint, the §14 elements are shown, read from the UI
//! tree; every checkpoint is rendered and compared with a local-only golden (decision 0031
//! tier 2, `ui_common/goldens.rs`: `TESSERA_GOLDEN_DIR`, keyed by package, test binary,
//! function and install version, never inside a work tree, never committed). A missing
//! golden is written and reported `golden: blessed`; `TESSERA_GOLDEN_REQUIRE=1` makes it a
//! failure, `TESSERA_GOLDEN_BLESS_LOCAL=1` rewrites existing ones; `TESSERA_BLESS` is not
//! read. The GPU pictures are compared with the tolerance of the render crate's
//! `golden.rs` (tier 1 for the same painter).
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test combat_ui_real_data
//! -- --nocapture`. Prints labels and counts only, never game text.

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
#[path = "ui_common/goldens.rs"]
mod goldens;
#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::CharacterData;
use tessera_client::cli::Args;
use tessera_client::combat::CombatInstall;
use tessera_client::combat::headless::{CLEAR, CombatScreens, REFERENCE_CHECKPOINTS, play};
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::combat::Avoidance;

const SCREEN: (u32, u32) = (1024, 768);

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Compares a GPU picture with its golden under the render crate's default tolerance.
fn gpu_compare(want: &goldens::Rgba, got: &goldens::Rgba) -> (bool, String, goldens::Rgba) {
    let image = |g: &goldens::Rgba| tessera_render::RgbaImage {
        width: g.width,
        height: g.height,
        data: g.data.clone(),
    };
    let tol = golden::Tolerance::default();
    let (c, diff) = golden::compare(&image(want), &image(got), tol);
    (
        c.passes(tol),
        format!(
            "{} outliers, {} strict mismatches of {} pixels, max difference {}",
            c.outliers, c.strict_mismatches, c.pixels, c.max_diff
        ),
        goldens::Rgba {
            width: diff.width,
            height: diff.height,
            data: diff.data,
        },
    )
}

fn ms(t: f64) -> u64 {
    (t * 1000.0).round() as u64
}

#[test]
fn combat_screens_with_the_install() {
    let Some(dir) = game_dir() else { return };
    // Decision 0031 tier 2: refuse a golden folder in a work tree before any work.
    let golden_dir = goldens::golden_dir(
        env!("CARGO_PKG_NAME"),
        "combat_ui_real_data/combat_screens_with_the_install",
        &goldens::install_version(&dir),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut flags = goldens::Flags::from_env();
    flags.sheet = flags.sheet.map(|s| s.join("combat_ui_real_data"));
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    // §16.1 step 1: the layouts and the main element labels resolve.
    let refs = booted.combat;
    let placeholder = tessera_ui::game::combat::CombatVocab::default();
    let mut unresolved = Vec::new();
    for ((label, v), (_, p)) in refs
        .vocab
        .ids
        .entries()
        .into_iter()
        .zip(placeholder.ids.entries())
    {
        if v == p {
            unresolved.push(label);
        }
    }
    eprintln!(
        "element labels: {} resolved, {} unresolved {unresolved:?}",
        refs.vocab.ids.entries().len() - unresolved.len(),
        unresolved.len()
    );
    assert!(refs.layouts.floatytext.is_some(), "floatytext");
    assert!(refs.layouts.effects.is_some(), "effects");
    assert!(refs.layouts.dialog.is_some(), "Dialog");
    assert!(refs.activatables.is_some(), "gameplayactivatables");
    for must in [
        "AvatarPanel",
        "OpponentPanel",
        "GamePlay_SelectionTarget",
        "GamePlay_ProgressBar_Dragbox",
        "GamePlay_Opponent_ProgressBar_Dragbox",
        "TimedEffectTemplate",
        "DamageTextTemplate",
        "EntitySelectionIndicator",
        "SkillQuickslotDisplayTemplate",
    ] {
        assert!(!unresolved.contains(&must), "{must} unresolved");
    }

    // §16.1 step 2: the floating-text settings and the avoidance tokens.
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let install = Arc::new(CombatInstall::new(Arc::clone(&data)).expect("combat data"));
    let f = install.floaty_settings();
    assert!(
        (f.crit_initial_scale - 1.7).abs() < 1e-4,
        "UI_FloatyText_Properties read (§6.2 column 0x78000004)"
    );
    let mut words = 0;
    for a in [
        Avoidance::Miss,
        Avoidance::Evade,
        Avoidance::Parry,
        Avoidance::Block,
        Avoidance::Resist,
        Avoidance::Immune,
    ] {
        if install
            .combat()
            .avoidance_text(a)
            .is_some_and(|w| !w.trim().is_empty())
        {
            words += 1;
        }
    }
    eprintln!("avoidance tokens resolved: {words} of 6");
    assert_eq!(words, 6, "every §6.6 short-form token resolves");

    // §16.1 step 3: the scenario with the install's panels.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/combat-ui.scenario");
    let args = Args::try_parse_from([
        "tessera-client",
        "--headless",
        "--game-dir",
        dir.to_str().expect("utf-8"),
        "--scenario",
        path.to_str().expect("utf-8"),
    ])
    .expect("args");
    let sc = Arc::new(prepare(&args, &InstallChoice::Found(dir.clone())).expect("prepares"));
    let config = headless_config(&args, Arc::clone(&sc)).expect("config");
    let mut screens = CombatScreens::with_install(booted, install, SCREEN);
    let mut renderer = gpu::headless_renderer(CLEAR);
    let mut missing = Vec::new();
    let mut images = Vec::new();
    let mut passed = 0;
    let mut unsettled = Vec::new();
    let (outcome, _) = play(config, sc, None::<Vec<u8>>, &mut screens, |at, s| {
        let Some((t, elements)) = REFERENCE_CHECKPOINTS.iter().find(|(t, _)| ms(*t) == at) else {
            return;
        };
        // A capture after the settle limit ran out is a failure (decision 0031).
        let settled = s.settle(renderer.as_deref_mut());
        if !settled {
            unsettled.push(at);
            return;
        }
        for e in *elements {
            if s.is_shown(*e) {
                passed += 1;
            } else {
                missing.push(format!("{e:?} at {t}"));
            }
        }
        if let Some(r) = renderer.as_deref_mut() {
            images.push((at, s.render(r).expect("renders")));
        }
    })
    .expect("sandbox");
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    assert!(
        unsettled.is_empty(),
        "the UI did not settle at {unsettled:?} ms; no picture is taken (decision 0031)"
    );
    eprintln!(
        "checkpoints: {} elements shown, {} missing {missing:?}",
        passed,
        missing.len()
    );
    assert!(missing.is_empty(), "elements not shown: {missing:?}");

    // Local-only goldens (decision 0031 tier 2): blessed when missing, else compared.
    let mut blessed = 0;
    let mut matched = 0;
    for (at, img) in images {
        let name = format!("checkpoint-{at:05}ms");
        let pic = goldens::Rgba {
            width: img.width,
            height: img.height,
            data: img.data,
        };
        match goldens::check_with_compare(&golden_dir, &name, &pic, &flags, gpu_compare) {
            Ok(goldens::Outcome::Blessed(_)) => blessed += 1,
            Ok(goldens::Outcome::Passed(_)) => matched += 1,
            Err(e) => panic!("{e}"),
        }
    }
    eprintln!(
        "goldens in {}: {blessed} blessed, {matched} passed",
        golden_dir.display()
    );
}
