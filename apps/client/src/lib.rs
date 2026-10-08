// SPDX-License-Identifier: GPL-3.0-or-later
//! The Tessera client: wires the install finder, the archive set, record decoders, the
//! renderer, the world session and the plugin host into one frame loop.
//!
//! - [`app`]: windowed client (winit).
//! - [`smoke`]: `--smoke`, the same start-up and frames rendered offscreen without a window.
//! - [`runtime`]: everything except presentation, shared by both.
//! - [`data`]: background install discovery, archive statistics and the showcase texture.
//! - [`net`], [`plugins`], [`scene`], [`input`]: subsystem glue.
//! - [`walker`]: the offline world walker (ROADMAP M3, M4 characters); [`walker_host`]
//!   wires it into the runtime, [`headless`] runs it offscreen (`--screenshot`,
//!   `--fly-path --headless`, `--duration --headless`); [`avatar`] parses `--avatar`.
//! - [`ui_host`]: the game UI (M5): `tessera-ui`, the native [`panels`], the plugin host and
//!   the input bridge in the frame loop (decisions 0014, 0015).

pub mod app;
pub mod avatar;
pub mod cli;
pub mod data;
pub mod headless;
pub mod input;
pub mod net;
pub mod panels;
pub mod plugins;
pub mod runtime;
pub mod scene;
pub mod smoke;
pub mod ui_host;
pub mod walker;
pub mod walker_host;

pub use cli::Args;

/// Run the client as configured by `args`.
pub fn run(args: Args) -> anyhow::Result<()> {
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
