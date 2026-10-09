// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 exit criterion 4 on the user's own plugins, read-only: every available plugin of the
//! settings folder's `Plugins/` is loaded with the `Turbine.UI` bindings, its windows are
//! counted, a click lands on each shown window and a few frames run; the per-plugin result
//! (loads, windows, errors) is printed as the incompatibility list that
//! `plugin-api-usage.md` §9.5 asks for. Plugin errors are reported, not failures.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` names an install; when the
//! settings folder has no `Plugins/` (the owner's case on 2026-10-08) it prints that the
//! criterion waits for the owner's install. Nothing is written into the install or the
//! settings folder: plugin data goes to a temporary folder.

#[path = "fixtures/harness.rs"]
mod harness;

use std::path::Path;

use harness::Fx;
use tessera_install::{GameInstall, SearchEnv, SettingsQuery};

#[test]
fn users_plugins_load_and_respond() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("TESSERA_GAME_DIR is not set; skipping");
        return;
    };
    let inst = GameInstall::open(Path::new(&dir)).expect("install opens");
    let settings = inst.settings_dir(&SettingsQuery::default(), &SearchEnv::from_process());
    let plugins = settings.path.join("Plugins");
    if !plugins.is_dir() {
        eprintln!(
            "no Plugins folder in {}; exit criterion 4 waits for the owner's install",
            settings.path.display()
        );
        return;
    }
    let mut probe = Fx::over(&plugins);
    probe.refresh();
    let names: Vec<String> = probe
        .manager
        .available_plugins()
        .iter()
        .map(|p| p.descriptor.name.clone())
        .collect();
    drop(probe);
    eprintln!("| Plugin | Loads | Windows (shown) | Errors |");
    eprintln!("|---|---|---|---|");
    for name in names {
        let mut fx = Fx::over(&plugins);
        fx.refresh();
        let loaded = fx.manager.load_plugin(&name).is_ok();
        fx.deliver();
        fx.manager.run_deferred_until_idle(20);
        fx.deliver();
        let apartment = fx
            .manager
            .loaded_plugins()
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.apartment.clone());
        let (windows, shown) = apartment.as_deref().map_or((0, 0), |ap| {
            let ui = fx.pui.ui().borrow();
            let w = fx.pui.windows(ap);
            let shown = w.iter().filter(|k| ui.is_visible(**k)).count();
            (w.len(), shown)
        });
        if let Some(ap) = &apartment {
            let boxes: Vec<_> = {
                let ui = fx.pui.ui().borrow();
                fx.pui
                    .windows(ap)
                    .into_iter()
                    .filter(|k| ui.is_visible(*k))
                    .map(|k| ui.displayed_box(k))
                    .collect()
            };
            for b in boxes {
                fx.click(b.l + b.w() / 2, b.t + b.h() / 2, 1);
            }
        }
        for _ in 0..3 {
            fx.frame(0.016);
        }
        fx.manager.unload_all();
        fx.deliver();
        let errors = fx.errors();
        eprintln!(
            "| {name} | {} | {windows} ({shown}) | {} |",
            if loaded { "yes" } else { "no" },
            if errors.is_empty() {
                "none".to_owned()
            } else {
                errors.join("; ").replace('|', "/")
            }
        );
    }
}
