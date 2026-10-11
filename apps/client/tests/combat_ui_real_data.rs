// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2 exit criterion 1 on the user's install (read-only; `combat-ui.md`
//! §16.1): the combat layouts and vocabulary resolve; the floating-text settings and the
//! avoidance tokens are found; the reference scenario of §17.1 plays headless with the
//! install's panels and, at every checkpoint, the §14 elements are shown, read from the UI
//! tree; every checkpoint is rendered and compared with a local golden in the user's cache
//! (ARCHITECTURE §23.3: real-data goldens are never committed). The first run writes the
//! goldens; later runs compare.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test combat_ui_real_data
//! -- --nocapture`. Prints labels and counts only, never game text.

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
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

/// The local golden folder in the user's cache (never in the repository).
fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("tessera/goldens/combat_ui")
}

fn ms(t: f64) -> u64 {
    (t * 1000.0).round() as u64
}

#[test]
fn combat_screens_with_the_install() {
    let Some(dir) = game_dir() else { return };
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
    let (outcome, _) = play(config, sc, None::<Vec<u8>>, &mut screens, |at, s| {
        let Some((t, elements)) = REFERENCE_CHECKPOINTS.iter().find(|(t, _)| ms(*t) == at) else {
            return;
        };
        s.settle(renderer.as_deref_mut());
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
    eprintln!(
        "checkpoints: {} elements shown, {} missing {missing:?}",
        passed,
        missing.len()
    );
    assert!(missing.is_empty(), "elements not shown: {missing:?}");

    // Local goldens: written on the first run, compared on later runs.
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).expect("cache folder");
    for (at, img) in &images {
        let p = cache.join(format!("combat_ui_{at}.png"));
        if p.exists() && !golden::blessing() {
            golden::check(&p, img, golden::Tolerance::default());
        } else {
            golden::write_png(&p, img).expect("writes the local golden");
            eprintln!("local golden written: {}", p.display());
        }
    }
}
