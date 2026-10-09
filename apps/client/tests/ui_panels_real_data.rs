// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's game UI host on the user's install (read-only): the UI start-up off the
//! main thread, the HUD set, Escape with a UI attached (`walker.md` §5, `game-panels.md`
//! §16: the game menu, never a quit), `/plugins manager` opening the plugin manager, a
//! position update reaching the minimap in step 7, and a synthetic plugin window next to
//! the native panels. Only counts are printed. Skipped (reported as passed) unless
//! `TESSERA_GAME_DIR` is set.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! ui_panels_real_data -- --nocapture`.

mod ui_common;

use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::panels::PanelId;
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_input::{KeyCode, RawEvent};
use ui_common::{Rig, write_plugin};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

const WINDOW: &str = r#"
local w = Turbine.UI.Window()
w:SetPosition(600, 300)
w:SetSize(200, 100)
w:SetBackColor(Turbine.UI.Color(1, 0.1, 0.4, 0.1))
w:SetText("Synthetic")
w:SetVisible(true)
Turbine.Shell.WriteLine("window: ready")
"#;

/// The most frames the UI may take to settle after the position update.
const SETTLE_FRAMES: usize = 20_000;

#[test]
fn game_ui_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let t = std::time::Instant::now();
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    println!(
        "UI data: {} layouts, {} panels in {} ms (indicative)",
        booted.layouts,
        booted.catalog.len(),
        t.elapsed().as_millis()
    );
    assert_eq!(booted.catalog.len(), 15);
    assert!(booted.layouts > 0);
    let mut booted = Some(booted);
    let mut rig = Rig::with_host(
        |p| write_plugin(p, "Suite", "Window", Some("Window"), WINDOW),
        |o| UiHost::from_boot(booted.take().expect("once"), o),
    );
    rig.attach(&["Window"]);
    let failed = rig.host.open_hud(&mut rig.plugins);
    assert!(failed.is_empty(), "{failed:?}");
    rig.wait_loaded(1);
    assert!(rig.said("window: ready"));
    assert_eq!(rig.host.plugin_ui().windows("Window").len(), 1);
    let panels = rig.host.panels().expect("native panels");
    assert_eq!(panels.opened().count(), 7, "the HUD set");

    // Escape: the UI declines `menu`, game code shows the main menu (no quit).
    let menu = rig.mapper.registry().action("menu").expect("menu action");
    rig.raw(RawEvent::Key {
        key: KeyCode::Escape,
        pressed: true,
        repeat: false,
    });
    rig.frame();
    assert!(rig.mapper.just_pressed(menu), "menu reached game code");
    let o = rig.host.escape(&mut rig.plugins, true, false);
    println!("Escape: {o:?}");
    rig.raw(RawEvent::Key {
        key: KeyCode::Escape,
        pressed: false,
        repeat: false,
    });
    rig.frame();
    let main_menu = PanelId::from_number(6).expect("P6");
    {
        let panels = rig.host.panels().expect("panels");
        let ui = rig.host.ui();
        assert!(panels.is_shown(&ui, main_menu), "P6 shown by Escape");
    }
    // A second Escape hides it again.
    let _ = rig.host.escape(&mut rig.plugins, true, false);
    {
        let panels = rig.host.panels().expect("panels");
        let ui = rig.host.ui();
        assert!(!panels.is_shown(&ui, main_menu), "P6 hidden by Escape");
    }

    // `/plugins manager` opens the plugin manager in step 7.
    assert!(rig.plugins.command("/plugins manager"));
    rig.frame();
    {
        let panels = rig.host.panels().expect("panels");
        let ui = rig.host.ui();
        assert!(panels.is_shown(&ui, PanelId::PluginManager), "P12 opened");
    }

    // A position update reaches the minimap in step 7 (game code, decision 0014 D8).
    let input = tessera_ui::game::radar::RadarInput {
        region: 1,
        x: 126,
        y: 140,
        cell: 0,
        global: (20_240.0, 22_480.0),
        heading: 90.0,
        has_player: true,
        ..tessera_ui::game::radar::RadarInput::default()
    };
    rig.host.update_position(input, None);
    // The paint step works in time slices, so the number of frames it needs depends on the
    // machine's load: wait for the UI to settle (at least one frame, so the position is
    // taken in step 7), bounded by a generous frame count.
    rig.frame();
    let mut frames = 1;
    while !rig.host.settled() && frames < SETTLE_FRAMES {
        rig.frame();
        std::thread::sleep(std::time::Duration::from_millis(1));
        frames += 1;
    }
    let settled = rig.host.settled();
    let s = rig.host.last_sample();
    println!(
        "after {frames} frames (settled: {settled}): {} composition entries, canvases {} KiB, \
         textures {} KiB",
        s.entries,
        s.canvas_bytes >> 10,
        s.texture_bytes >> 10
    );
    assert!(s.entries > 7);
    rig.shutdown();
    assert!(rig.plugins.manager().loaded_plugins().is_empty());
}
