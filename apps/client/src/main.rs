// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-client` binary entry point.

use std::process::ExitCode;

use clap::Parser as _;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn,winit=warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let args = tessera_client::Args::parse();
    match tessera_client::run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}
