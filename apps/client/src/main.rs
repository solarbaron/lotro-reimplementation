// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-client` binary entry point.

use std::process::ExitCode;

use clap::Parser as _;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let args = tessera_client::Args::parse();
    if let Err(e) = args.check() {
        e.exit();
    }
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn,winit=warn")
    });
    // A scenario log on standard output keeps standard output to itself.
    if args
        .scenario_log
        .as_deref()
        .is_some_and(|p| p.as_os_str() == "-")
    {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
    ExitCode::from(tessera_client::run_to_exit_code(args))
}
