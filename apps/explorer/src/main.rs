// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`: GUI asset browser. See the library documentation.

use std::time::Duration;

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
    let args = Args::try_parse_checked(std::env::args_os()).unwrap_or_else(|e| e.exit());
    if let Some(out) = args.sheet.clone() {
        return run_sheet(&args, out);
    }
    let size = match (args.width, args.height) {
        (Some(w), Some(h)) => Some([w.max(320.0), h.max(240.0)]),
        _ => None,
    };
    let screenshot = args.screenshot.clone().map(|path| ScreenshotPlan {
        path,
        timeout: Duration::from_secs(args.screenshot_timeout.max(1)),
        size,
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
    let character = args.character_args();
    // Character options open the character view; without --select the root DID map is
    // selected (the view does not need a particular record).
    let (select, view) = if character.any() {
        (
            args.select.or(Some(tessera_props::ROOT_DID_MAP.0)),
            args.view
                .or(Some(tessera_explorer::cli::ViewKind::Character)),
        )
    } else {
        (args.select, args.view)
    };
    let config = AppConfig {
        game_dir: args.game_dir.clone(),
        language: args.language.clone(),
        select,
        view,
        character,
        hide_panels: args.hide_panels,
        screenshot,
        browse: args.browse,
        search: args.search.clone(),
        font: args.font.clone(),
        mesh: tessera_explorer::mesh_view::MeshOptions {
            shading: args.mesh_shading,
            wireframe: args.mesh_wireframe,
            normals: args.mesh_normals,
            vertex_colors: args.mesh_vertex_colors,
            diffuse_tint: args.mesh_diffuse_tint,
            lod: args.mesh_lod,
            choice: args.mesh_choice,
            dye: args.dye,
            angles: args.mesh_angle,
            select: args.mesh_select.map(|[p, d]| (p as usize, d)),
        },
    };
    let code = tessera_explorer::run(config, persist, size)?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// `--sheet`: renders the character view headless and prints the report.
fn run_sheet(args: &Args, out: std::path::PathBuf) -> anyhow::Result<()> {
    let game_dir = args
        .game_dir
        .clone()
        .or_else(|| std::env::var_os(tessera_install::GAME_DIR_ENV).map(Into::into))
        .or_else(|| {
            tessera_settings::SettingsStore::open_default()
                .ok()
                .and_then(|s| s.load().settings.game.active_dir)
        })
        .ok_or_else(|| anyhow::anyhow!("no install: give --game-dir or set TESSERA_GAME_DIR"))?;
    let plan = tessera_explorer::sheet::SheetPlan {
        game_dir,
        out,
        args: args.character_args(),
        times: args
            .times
            .clone()
            .map_or_else(|| vec![args.time.unwrap_or(0.0)], |l| l.0),
        angles: args
            .angles
            .clone()
            .map_or_else(|| vec![args.mesh_angle.unwrap_or([160.0, 10.0])], |l| l.0),
        tile: args.tile.unwrap_or([480, 640]),
        wireframe: args.mesh_wireframe,
        focus: args.focus,
    };
    let report = tessera_explorer::sheet::run_sheet(&plan)?;
    for l in &report.lines {
        println!("{l}");
    }
    if let Some(w) = &report.written {
        println!("written {}", w.display());
    }
    Ok(())
}
