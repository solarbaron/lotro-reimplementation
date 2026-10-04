// SPDX-License-Identifier: GPL-3.0-or-later
//! The Tessera client: wires the install finder, the archive set, record decoders, the
//! renderer, the world session and the plugin host into one frame loop.
//!
//! - [`app`]: windowed client (winit).
//! - [`smoke`]: `--smoke`, the same start-up and frames rendered offscreen without a window.
//! - [`runtime`]: everything except presentation, shared by both.
//! - [`data`]: background install discovery, archive statistics and the showcase texture.
//! - [`net`], [`plugins`], [`scene`], [`input`]: subsystem glue.

pub mod app;
pub mod cli;
pub mod data;
pub mod input;
pub mod net;
pub mod plugins;
pub mod runtime;
pub mod scene;
pub mod smoke;

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
    } else {
        app::run(args)
    }
}
