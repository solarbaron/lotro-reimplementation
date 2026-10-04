// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`: GUI asset browser. See the library documentation.

use std::time::Duration;

use clap::Parser;
use tessera_explorer::cli::Args;
use tessera_explorer::persist::Persist;
use tessera_explorer::{AppConfig, ScreenshotPlan};
use tessera_settings::Settings;

fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu=warn,naga=warn,eframe=warn,egui_wgpu=warn")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let screenshot = args.screenshot.clone().map(|path| ScreenshotPlan {
        path,
        timeout: Duration::from_secs(args.screenshot_timeout.max(1)),
    });
    let persist = if args.no_persist || screenshot.is_some() {
        // Still read the saved game directory, but never write.
        let loaded = tessera_settings::SettingsStore::open_default()
            .map(|s| s.load().settings)
            .unwrap_or_else(|_| Settings::default());
        Persist::in_memory(loaded)
    } else {
        Persist::load_default()
    };
    let size = match (args.width, args.height) {
        (Some(w), Some(h)) => Some([w.max(320.0), h.max(240.0)]),
        _ => None,
    };
    let config = AppConfig {
        game_dir: args.game_dir,
        language: args.language,
        select: args.select,
        view: args.view,
        screenshot,
        browse: args.browse,
        search: args.search,
        font: args.font,
    };
    let code = tessera_explorer::run(config, persist, size)?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
