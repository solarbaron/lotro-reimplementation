// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless renderer bootstrap shared by golden-image tests.
//!
//! Tries the environment/default backends first, then GL and Vulkan explicitly, so a
//! software rasteriser (llvmpipe / lavapipe) is found even when the default pick fails.
//! Without any adapter the caller skips — unless `TESSERA_REQUIRE_GPU=1`, which CI sets so
//! a missing software rasteriser fails loudly instead of silently skipping the goldens.
//!
//! Included with `#[path]` by more than one test crate, so unused items are allowed.
#![allow(dead_code)]

use tessera_render::{ContextOptions, Renderer, RendererConfig, wgpu};

/// Environment variable that turns "no adapter, skipping" into a test failure.
pub const ENV_REQUIRE_GPU: &str = "TESSERA_REQUIRE_GPU";

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tessera_render=info".into()),
        )
        .with_test_writer()
        .try_init();
}

/// A headless renderer with `clear_color`, or `None` (with the reason printed) when no
/// adapter exists.
///
/// # Panics
///
/// When no adapter exists and `TESSERA_REQUIRE_GPU` is set to a non-empty value other
/// than `0`.
#[must_use]
pub fn headless_renderer(clear_color: [f32; 4]) -> Option<Renderer<'static>> {
    init_tracing();
    let attempts: [Option<wgpu::Backends>; 3] =
        [None, Some(wgpu::Backends::GL), Some(wgpu::Backends::VULKAN)];
    let mut reasons = Vec::new();
    for backends in attempts {
        let config = RendererConfig {
            context: ContextOptions {
                backends,
                allow_software: true,
                ..ContextOptions::default()
            },
            clear_color,
            ..RendererConfig::default()
        };
        match Renderer::new_headless(&config) {
            Ok(r) => {
                let info = r.adapter_info();
                eprintln!(
                    "using adapter {:?} ({:?}, {:?}, driver {:?} {:?})",
                    info.name, info.backend, info.device_type, info.driver, info.driver_info
                );
                return Some(r);
            }
            Err(e) => reasons.push(format!("{backends:?}: {e}")),
        }
    }
    let reasons = reasons.join(" | ");
    if std::env::var(ENV_REQUIRE_GPU).is_ok_and(|v| !v.is_empty() && v != "0") {
        panic!("{ENV_REQUIRE_GPU} is set but no GPU adapter is available: {reasons}");
    }
    eprintln!("SKIPPED: no GPU adapter available: {reasons}");
    None
}
