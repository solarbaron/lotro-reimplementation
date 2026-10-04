// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless renderer bootstrap shared by golden-image and render tests.
//!
//! Tries the environment/default backends first, then GL and Vulkan explicitly, so a
//! software rasteriser (llvmpipe / lavapipe) is found even when the default pick fails.
//!
//! - **No adapter at all**: the caller skips, unless `TESSERA_REQUIRE_GPU=1` (CI sets it on
//!   Linux), which turns a missing software rasteriser into a failure.
//! - **An adapter exists but every device request failed** (for example "Parent device is
//!   lost"): the test fails, because skipping would report goldens as passed that never
//!   ran. `TESSERA_REQUIRE_GPU=0` skips anyway.
//!
//! Every renderer handed out holds a process-wide GPU lock for its whole life, so the tests
//! of one binary use the GPU one at a time (some drivers, e.g. NVIDIA's proprietary Vulkan
//! driver, lose or hang devices when many are created and used in parallel). The lock is
//! re-entrant per thread, so one test may hold several renderers at once.
//!
//! Included with `#[path]` by more than one test crate, so unused items are allowed.
#![allow(dead_code)]

use std::ops::{Deref, DerefMut};
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::ThreadId;

use tessera_render::{ContextOptions, RenderError, Renderer, RendererConfig, wgpu};

/// Environment variable that turns "no adapter, skipping" into a test failure (`1`), or
/// makes every adapter/device failure a skip (`0`).
pub const ENV_REQUIRE_GPU: &str = "TESSERA_REQUIRE_GPU";

/// Owner thread and hold count of the GPU lock.
static GPU_OWNER: Mutex<Option<(ThreadId, usize)>> = Mutex::new(None);
static GPU_FREE: Condvar = Condvar::new();

/// Holds the process-wide GPU lock; released on drop.
#[derive(Debug)]
pub struct GpuLock(());

impl GpuLock {
    /// Blocks until no other thread holds the lock (re-entrant for the current thread).
    #[must_use]
    pub fn acquire() -> Self {
        let me = std::thread::current().id();
        let mut owner = GPU_OWNER.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            match owner.as_mut() {
                None => {
                    *owner = Some((me, 1));
                    return Self(());
                }
                Some((id, n)) if *id == me => {
                    *n += 1;
                    return Self(());
                }
                Some(_) => {
                    owner = GPU_FREE.wait(owner).unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }
}

impl Drop for GpuLock {
    fn drop(&mut self) {
        let mut owner = GPU_OWNER.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, n)) = owner.as_mut() {
            *n -= 1;
            if *n == 0 {
                *owner = None;
                GPU_FREE.notify_all();
            }
        }
    }
}

/// A headless renderer that holds the GPU lock; derefs to [`Renderer`].
pub struct Headless {
    // Field order matters: the renderer is dropped before the lock is released.
    renderer: Renderer<'static>,
    _lock: GpuLock,
}

impl Deref for Headless {
    type Target = Renderer<'static>;
    fn deref(&self) -> &Self::Target {
        &self.renderer
    }
}

impl DerefMut for Headless {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.renderer
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tessera_render=info".into()),
        )
        .with_test_writer()
        .try_init();
}

fn require_gpu() -> Option<bool> {
    match std::env::var(ENV_REQUIRE_GPU).ok().as_deref() {
        None | Some("") => None,
        Some("0") => Some(false),
        Some(_) => Some(true),
    }
}

/// A headless renderer with `clear_color`, or `None` (with the reason printed) when no
/// adapter exists.
///
/// # Panics
///
/// See [`headless_renderer_with`].
#[must_use]
pub fn headless_renderer(clear_color: [f32; 4]) -> Option<Headless> {
    headless_renderer_with(|c| c.clear_color = clear_color)
}

/// A headless renderer whose configuration `configure` adjusts (the adapter search sets
/// `context.backends` and `allow_software` before calling it), or `None` when no adapter
/// exists.
///
/// # Panics
///
/// When no adapter exists and `TESSERA_REQUIRE_GPU=1`, or when an adapter exists but no
/// device could be created and `TESSERA_REQUIRE_GPU` is not `0`.
#[must_use]
pub fn headless_renderer_with(configure: impl Fn(&mut RendererConfig)) -> Option<Headless> {
    init_tracing();
    let lock = GpuLock::acquire();
    let attempts: [Option<wgpu::Backends>; 3] =
        [None, Some(wgpu::Backends::GL), Some(wgpu::Backends::VULKAN)];
    let mut reasons = Vec::new();
    let mut device_failed = false;
    for backends in attempts {
        let mut config = RendererConfig {
            context: ContextOptions {
                backends,
                allow_software: true,
                ..ContextOptions::default()
            },
            ..RendererConfig::default()
        };
        configure(&mut config);
        match Renderer::new_headless(&config) {
            Ok(r) => {
                let info = r.adapter_info();
                eprintln!(
                    "using adapter {:?} ({:?}, {:?}, driver {:?} {:?})",
                    info.name, info.backend, info.device_type, info.driver, info.driver_info
                );
                return Some(Headless {
                    renderer: r,
                    _lock: lock,
                });
            }
            Err(e) => {
                device_failed |= !matches!(e, RenderError::NoAdapter { .. });
                reasons.push(format!("{backends:?}: {e}"));
            }
        }
    }
    let reasons = reasons.join(" | ");
    match (require_gpu(), device_failed) {
        (Some(true), _) => {
            panic!("{ENV_REQUIRE_GPU} is set but no GPU device could be created: {reasons}")
        }
        (None, true) => panic!(
            "a GPU adapter exists but no device could be created ({reasons}); set \
             {ENV_REQUIRE_GPU}=0 to skip GPU tests on this machine"
        ),
        _ => {}
    }
    eprintln!("SKIPPED: no GPU adapter available: {reasons}");
    None
}
