// SPDX-License-Identifier: GPL-3.0-or-later
//! UI scaling on the user's install (decision 0040, `ui-scaling.md` §3.2, §4): the client's
//! UI host sizes the `tessera-ui` screen root in UI units (*F*), not physical pixels, and
//! applies a live scale change without rebuilding. This smoke test boots the real panels at
//! a window whose effective scale is below 1 (1280 × 720 → *Se* 0.9375, *F* 1365 × 768, the
//! client's default size, §9.2 C1), checks that the UI screen is *F*, opens the HUD, settles
//! the paint, then changes the scale live and settles again.
//!
//! Nothing in the install or the user's settings folder is touched; only counts are printed.
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! ui_scale_real_data -- --nocapture`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::plugins::Plugins;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_settings::{SafeAspect, UiPrefs, UiScale};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// The most frames the UI may take to settle (the paint works in time slices, so the count
/// depends on the machine's load; only the settled state ends the wait).
const SETTLE_FRAMES: usize = 20_000;

fn settle(host: &mut UiHost, plugins: &mut Plugins) {
    for _ in 0..SETTLE_FRAMES {
        host.step7(plugins, None);
        host.step8(None, plugins);
        host.end_frame(plugins, 0.0);
        if host.settled() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("UI did not settle in {SETTLE_FRAMES} frames");
}

#[test]
fn ui_screen_is_f_and_scale_changes_live() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let tmp = tempfile::tempdir().expect("tempdir");
    let empty_root = tmp.path().join("empty-skins");
    std::fs::create_dir_all(&empty_root).expect("mkdir");
    let plugins_dir = tmp.path().join("Plugins");
    std::fs::create_dir_all(&plugins_dir).expect("mkdir");
    let mut plugins = Plugins::start(&plugins_dir, tmp.path().join("data"), &[]);

    let booted = UiBoot::spawn(Arc::clone(&set), dir.clone(), Some(empty_root))
        .expect("spawn")
        .wait()
        .expect("UI data");

    // C1: the default 1280 × 720 window at S 1.0 → Se 0.9375, F 1365 × 768 (§9.2).
    let prefs = UiPrefs {
        ui_scale: UiScale::Fixed(1.0),
        safe_aspect: SafeAspect::R21_9,
        ..UiPrefs::default()
    };
    let options = UiOptions {
        screen: (1280, 720),
        prefs,
        hud: true,
        ..UiOptions::default()
    };
    let mut host = UiHost::from_boot(booted, &options);
    host.attach_plugins(&mut plugins, &[]);

    // The UI screen is F (UI units), not the physical window size (§3.2).
    assert_eq!(host.scale().ui_size(), (1365, 768), "F at C1");
    assert!(host.scale().is_limited(), "0.9375 < 1.0 is limited");
    assert_eq!(
        host.ui().screen_size(),
        (1365, 768),
        "screen root sized in F"
    );

    // The pointer at the window's last physical column maps inside F, not past it (§3.6).
    let (ux, _) = host.scale().physical_to_ui(1279.5, 0.0);
    assert!(ux < 1365.0, "last column inside F ({ux})");

    host.open_hud(&mut plugins);
    settle(&mut host, &mut plugins);
    let panels_at_c1 = host.panels().map_or(0, |p| p.opened().count());
    println!(
        "C1 (1280x720, Se 0.9375): F = {:?}, {panels_at_c1} panels",
        host.scale().ui_size()
    );

    // A live scale change to 150 % (no rebuild): F shrinks, the UI re-lays out and settles.
    let now = std::time::Instant::now();
    host.apply_uiscale_request(
        &tessera_client::commands::uiscale::UiScaleRequest::Set(1.5),
        now,
    );
    assert_eq!(
        host.scale().ui_size(),
        host.ui().screen_size(),
        "screen follows F"
    );
    // 1280 × 720 cannot reach S 1.5: Se is limited to 0.9375 by the window (§4.3), so F
    // stays 1365 × 768 and is never below the 1024 × 768 design floor.
    let (wu, hu) = host.scale().ui_size();
    assert!(wu >= 1024 && hu >= 768, "design floor kept: {wu}x{hu}");
    assert!(host.scale().is_limited(), "1.5 is limited at 1280x720");
    settle(&mut host, &mut plugins);
    println!("after /uiscale 1.5: F = {:?}", host.scale().ui_size());

    host.shutdown(&mut plugins, None);
    println!("passed");
}
