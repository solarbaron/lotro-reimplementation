// SPDX-License-Identifier: GPL-3.0-or-later
//! The Tessera client: wires the install finder, the archive set, record decoders, the
//! renderer, the world session and the plugin host into one frame loop.
//!
//! - [`app`]: windowed client (winit).
//! - [`smoke`]: `--smoke`, the same start-up and frames rendered offscreen without a window.
//! - [`runtime`]: everything except presentation, shared by both.
//! - [`data`]: background install discovery, archive statistics and the showcase texture.
//! - [`net`], [`plugins`], [`scene`], [`input`]: subsystem glue.
//! - [`commands`]: the one command dispatcher (decision 0020): registry, line pipeline,
//!   client built-ins; the chat overlay, `--command` and plugins feed it.
//! - [`walker`]: the offline world walker (ROADMAP M3, M4 characters); [`walker_host`]
//!   wires it into the runtime, [`headless`] runs it offscreen (`--screenshot`,
//!   `--fly-path --headless`, `--duration --headless`, and `--walk-path`, which needs no
//!   GPU); [`avatar`] parses `--avatar`; [`walk_path`] parses the scripted walks of
//!   `--walk-path` (`docs/checklists/m4-5-visual.md` §2.1).
//! - [`audio`]: world audio for the walker (ROADMAP M6): the audio engine driven by the
//!   script graphs (area music, ambience, positional emitters).
//! - [`ui_host`]: the game UI (M5): `tessera-ui`, the native [`panels`], the plugin host and
//!   the input bridge in the frame loop (decisions 0014, 0015).
//! - [`scenario`]: `--scenario` (P0, `docs/specs/scenario-feed.md`): scripted world events
//!   played by the sandbox, selectors resolved against the install, the headless driver
//!   and its canonical log.
//! - [`screens`]: the character selection and creation screens before world entry (P1,
//!   `docs/specs/character-screens.md`); [`pre_world`] runs them at start-up
//!   (`--character-select`).
//! - [`character`]: characters, inventory and equipment (P1): the player's state from the
//!   session, the character panel and bag windows, item tooltips, the plugin inventory
//!   views.

pub mod app;
pub mod audio;
pub mod avatar;
pub mod character;
pub mod cli;
pub mod commands;
pub mod data;
pub mod headless;
pub mod input;
pub mod net;
pub mod panels;
pub mod plugins;
pub mod pre_world;
pub mod runtime;
pub mod scenario;
pub mod scene;
pub mod screens;
pub mod smoke;
pub mod social;
pub mod ui_host;
pub mod wait;
pub mod walk_path;
pub mod walker;
pub mod walker_host;

pub use cli::Args;

/// Runs the client as configured by `args` and returns the process exit code: 0 on
/// success, 1 on failure, and for `--scenario` the codes of `scenario-feed.md` §12.2
/// ([`scenario::run`]).
#[must_use]
pub fn run_to_exit_code(args: Args) -> u8 {
    if args.scenario.is_some() {
        return scenario::run(args);
    }
    match run(args) {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("{e:#}");
            1
        }
    }
}

/// Run the client as configured by `args` (`--scenario` runs go through
/// [`run_to_exit_code`], which keeps their exit codes).
pub fn run(args: Args) -> anyhow::Result<()> {
    if args.scenario.is_some() {
        return match scenario::run(args) {
            0 => Ok(()),
            code => Err(anyhow::anyhow!("scenario run failed (exit code {code})")),
        };
    }
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        smoke = args.smoke,
        "Tessera client starting"
    );
    if args.smoke {
        smoke::run(&args).map(|_| ())
    } else if args.walker_headless() && args.ui_enabled() {
        ui_host::run::run(&args).map(|_| ())
    } else if args.walker_headless() {
        headless::run(&args).map(|_| ())
    } else {
        app::run(args)
    }
}
