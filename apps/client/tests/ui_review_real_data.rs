// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests of the client's game-UI glue on the user's install (read-only), for the
//! review findings that need the native panels: one `Received` per session line through
//! the chat window (`chat-window.md` §10 items 2 and 9), `[UI] ui_scale_global` on a panel
//! opened after the HUD (`ui-behaviour.md` §9.1), and the string tables the panels use
//! loaded by the start-up thread, not on the main thread (ARCHITECTURE §21). Only counts
//! are printed. Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! ui_review_real_data -- --nocapture`.

mod ui_common;

use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::net::ChatLine;
use tessera_client::panels::PanelId;
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
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

const LISTENER: &str = r#"
Turbine.Chat.Received = function(s, a)
  if string.sub(a.Message, 1, 7) == "review-" then
    Turbine.Shell.WriteLine("got " .. a.Message .. " " .. tostring(a.ChatType) .. " " .. tostring(a.Sender))
  end
end
"#;

#[test]
fn review_findings_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let mut booted = Some(booted);
    let mut rig = Rig::with_host(
        |p| write_plugin(p, "Review", "Listener", Some("L"), LISTENER),
        |o| {
            let mut o = o.clone();
            o.prefs.scale_global = 1.5;
            UiHost::from_boot(booted.take().expect("once"), &o)
        },
    );
    rig.attach(&["Listener"]);
    let failed = rig.host.open_hud(&mut rig.plugins);
    assert!(failed.is_empty(), "{failed:?}");
    rig.wait_loaded(1);
    for _ in 0..10 {
        rig.frame();
    }

    // One Received per session line, through the chat window.
    rig.host.post_session_lines([
        ChatLine {
            sender: String::new(),
            text: "review-welcome".into(),
            chat_type: 8,
        },
        ChatLine {
            sender: "Npc".into(),
            text: "review-say".into(),
            chat_type: 5,
        },
    ]);
    for _ in 0..5 {
        rig.frame();
    }
    let got = |m: &str| {
        rig.lines()
            .into_iter()
            .filter(|l| l.starts_with(&format!("got {m} ")))
            .collect::<Vec<_>>()
    };
    assert_eq!(got("review-welcome"), vec!["got review-welcome 8 nil"]);
    assert_eq!(got("review-say"), vec!["got review-say 5 Npc"]);

    // `ui_scale_global` reaches a panel opened after the HUD.
    let strings_before = rig
        .host
        .env()
        .map(|e| e.borrow().stats().late_string_tables)
        .unwrap_or_default();
    let p7 = PanelId::from_number(7).expect("P7");
    let root = rig.host.open_panel(&mut rig.plugins, p7).expect("P7 opens");
    for _ in 0..3 {
        rig.frame();
    }
    {
        let ui = rig.host.ui();
        let layout = ui.layout_box(root).expect("layout box");
        let shown = ui.displayed_box(root);
        println!(
            "P7 layout {}x{}, displayed {}x{}",
            layout.w(),
            layout.h(),
            shown.w(),
            shown.h()
        );
        assert!(
            shown.w() > layout.w() && shown.h() > layout.h(),
            "P7 takes the global scale"
        );
    }
    let late = rig
        .host
        .env()
        .map(|e| e.borrow().stats().late_string_tables)
        .unwrap_or_default();
    println!("string tables loaded on the main thread: HUD {strings_before}, after P7 {late}");
    assert_eq!(
        late, 0,
        "every string table of the HUD and P7 was preloaded by the start-up thread"
    );
    rig.shutdown();
}
