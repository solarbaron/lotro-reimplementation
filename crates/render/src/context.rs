// SPDX-License-Identifier: GPL-3.0-or-later
//! Instance / adapter / device bootstrap.
//!
//! Adapter selection is done by Tessera itself (rather than
//! `wgpu::util::initialize_adapter_from_env`, which panics when no adapter
//! matches) so that a missing GPU is an ordinary [`RenderError::NoAdapter`].

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::error::{RenderError, Result};

/// Longest time any blocking wait on the GPU may take before it is reported as an error
/// (a stuck driver must not hang the caller, or a test run, forever).
pub const GPU_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// A device wait for the latest submission, bounded by [`GPU_WAIT_TIMEOUT`].
pub(crate) fn bounded_wait() -> wgpu::PollType {
    wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(GPU_WAIT_TIMEOUT),
    }
}

/// Environment variable naming the backends to search (`vulkan,gl,...`).
pub const ENV_BACKEND: &str = "WGPU_BACKEND";
/// Environment variable holding a case-insensitive adapter-name substring.
pub const ENV_ADAPTER_NAME: &str = "WGPU_ADAPTER_NAME";
/// Environment variable selecting the power preference (`low` / `high` / `none`).
pub const ENV_POWER_PREF: &str = "WGPU_POWER_PREF";

/// How to pick a GPU.
///
/// One precedence rule for every selectable setting: an explicit `Some(..)`
/// value wins; `None` defers to the matching environment variable
/// ([`ENV_BACKEND`], [`ENV_POWER_PREF`], [`ENV_ADAPTER_NAME`]); without that,
/// a built-in default applies.
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Backends to search. `None` means: `WGPU_BACKEND` if set, else
    /// `PRIMARY | GL` (all real backends; never the no-op backend).
    pub backends: Option<wgpu::Backends>,
    /// Preferred class of adapter. `None` means: `WGPU_POWER_PREF` if set,
    /// else [`wgpu::PowerPreference::HighPerformance`].
    pub power_preference: Option<wgpu::PowerPreference>,
    /// Case-insensitive substring an adapter's name must contain. `None`
    /// means: `WGPU_ADAPTER_NAME` if set, else no filter.
    pub adapter_name: Option<String>,
    /// Allow CPU (software) adapters such as llvmpipe/lavapipe/WARP.
    /// They are always ranked below hardware adapters.
    pub allow_software: bool,
    /// Device limits to request. `None` requests WebGL2-level downlevel
    /// limits raised to the adapter's texture resolution and alignment, which
    /// keeps the renderer honest about what it relies on.
    pub required_limits: Option<wgpu::Limits>,
    /// Request [`wgpu::Features::TIMESTAMP_QUERY`] when the adapter offers it (default
    /// `true`), so that the UI pass's GPU time is measured (`UiStats::gpu_ns`, render.md
    /// §6.16 U2). With `false`, or on an adapter without the feature, the device has no
    /// optional features and `gpu_ns` stays `None`.
    pub timestamp_queries: bool,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            backends: None,
            power_preference: None,
            adapter_name: None,
            allow_software: true,
            required_limits: None,
            timestamp_queries: true,
        }
    }
}

impl ContextOptions {
    /// Backends after applying the `WGPU_BACKEND` fallback.
    #[must_use]
    pub fn resolved_backends(&self) -> wgpu::Backends {
        if let Some(b) = self.backends {
            return b;
        }
        match wgpu::Backends::from_env() {
            Some(b) if !b.is_empty() => b,
            _ => wgpu::Backends::PRIMARY | wgpu::Backends::GL,
        }
    }

    /// Power preference after applying the `WGPU_POWER_PREF` fallback.
    #[must_use]
    pub fn resolved_power_preference(&self) -> wgpu::PowerPreference {
        self.power_preference
            .or_else(wgpu::PowerPreference::from_env)
            .unwrap_or(wgpu::PowerPreference::HighPerformance)
    }

    /// Adapter-name filter after applying `WGPU_ADAPTER_NAME` (lower-cased).
    #[must_use]
    pub fn resolved_adapter_name(&self) -> Option<String> {
        self.adapter_name
            .clone()
            .or_else(|| std::env::var(ENV_ADAPTER_NAME).ok())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
    }
}

/// Collects asynchronous GPU errors instead of letting wgpu panic.
#[derive(Clone, Default)]
pub(crate) struct ErrorSink {
    first: Arc<Mutex<Option<String>>>,
}

impl fmt::Debug for ErrorSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorSink").finish_non_exhaustive()
    }
}

impl ErrorSink {
    fn record(&self, msg: String) {
        let mut slot = self.first.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = Some(msg);
        }
    }

    /// Returns (and clears) the first error recorded since the last call.
    pub(crate) fn take(&self) -> Option<String> {
        self.first.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub(crate) fn check(&self) -> Result<()> {
        match self.take() {
            Some(msg) => Err(RenderError::Gpu(msg)),
            None => Ok(()),
        }
    }
}

/// A live GPU: instance, chosen adapter, device and queue.
#[derive(Debug)]
pub struct GpuContext {
    /// The wgpu instance (kept for surface (re)creation).
    pub instance: wgpu::Instance,
    /// The adapter that was selected.
    pub adapter: wgpu::Adapter,
    /// Logical device.
    pub device: wgpu::Device,
    /// Submission queue.
    pub queue: wgpu::Queue,
    /// Cached adapter description.
    pub info: wgpu::AdapterInfo,
    pub(crate) errors: ErrorSink,
}

/// Rank an adapter: `None` = rejected, larger = better.
///
/// `name_filter` is matched as a case-insensitive substring of the adapter
/// name (surrounding whitespace ignored; an empty filter matches everything).
///
/// Pure function so the policy is unit-testable without a GPU.
#[must_use]
pub fn rank_adapter(
    info: &wgpu::AdapterInfo,
    power: wgpu::PowerPreference,
    allow_software: bool,
    name_filter: Option<&str>,
) -> Option<u32> {
    if let Some(filter) = name_filter
        && !info
            .name
            .to_lowercase()
            .contains(&filter.trim().to_lowercase())
    {
        return None;
    }
    use wgpu::DeviceType as D;
    let class = match (info.device_type, power) {
        (D::Cpu, _) if !allow_software => return None,
        (D::Cpu, _) => 1,
        (D::Other, _) => 2,
        (D::VirtualGpu, _) => 3,
        (D::IntegratedGpu, wgpu::PowerPreference::LowPower) => 6,
        (D::IntegratedGpu, _) => 4,
        (D::DiscreteGpu, wgpu::PowerPreference::LowPower) => 4,
        (D::DiscreteGpu, _) => 6,
    };
    // Prefer native modern APIs over GL at equal class.
    let api = match info.backend {
        wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => 2,
        wgpu::Backend::Gl => 1,
        _ => 0,
    };
    Some(class * 4 + api)
}

fn log_adapter(info: &wgpu::AdapterInfo, chosen: bool) {
    if chosen {
        tracing::info!(
            name = %info.name,
            backend = ?info.backend,
            device_type = ?info.device_type,
            vendor = format_args!("{:#06x}", info.vendor),
            device = format_args!("{:#06x}", info.device),
            driver = %info.driver,
            driver_info = %info.driver_info,
            "selected GPU adapter"
        );
    } else {
        tracing::debug!(
            name = %info.name,
            backend = ?info.backend,
            device_type = ?info.device_type,
            driver = %info.driver,
            "candidate GPU adapter"
        );
    }
}

/// The error for an adapter search that opened no device. `rejected` lists
/// every candidate's reason (filtered out, cannot present, or a failed device
/// request); `last_device_error` is the last failed device request, if any
/// candidate got that far.
///
/// `NoAdapter` only when no candidate reached device creation: an adapter that
/// matched but could not open a device is a different failure (the GPU test
/// harness fails on it instead of skipping).
fn no_device_error(
    backends: String,
    rejected: &[String],
    last_device_error: Option<wgpu::RequestDeviceError>,
) -> RenderError {
    match last_device_error {
        Some(e) if rejected.len() == 1 => RenderError::RequestDevice(e),
        Some(_) => RenderError::NoDevice {
            backends,
            reason: rejected.join("; "),
        },
        None => RenderError::NoAdapter {
            backends,
            reason: if rejected.is_empty() {
                "no adapters enumerated".to_owned()
            } else {
                rejected.join("; ")
            },
        },
    }
}

impl GpuContext {
    /// Create an instance with the resolved backends and optional display handle.
    pub(crate) fn create_instance(
        options: &ContextOptions,
        display: Option<Box<dyn wgpu::rwh::HasDisplayHandle + Send + Sync>>,
    ) -> wgpu::Instance {
        let backends = options.resolved_backends();
        let mut desc = match display {
            Some(d) => wgpu::InstanceDescriptor::new_with_display_handle(Box::new(DisplayBox(d))),
            None => wgpu::InstanceDescriptor::new_without_display_handle(),
        };
        desc.backends = backends;
        desc.flags = wgpu::InstanceFlags::from_build_config().with_env();
        tracing::debug!(?backends, flags = ?desc.flags, "creating wgpu instance");
        wgpu::Instance::new(desc)
    }

    /// Pick an adapter and open a device on it.
    pub(crate) fn new_with_instance(
        instance: wgpu::Instance,
        options: &ContextOptions,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self> {
        let backends = options.resolved_backends();
        let power = options.resolved_power_preference();
        let name_filter = options.resolved_adapter_name();

        let adapters = pollster::block_on(instance.enumerate_adapters(backends));
        let mut candidates: Vec<(u32, wgpu::Adapter)> = Vec::new();
        let mut rejected = Vec::new();
        for adapter in adapters {
            let info = adapter.get_info();
            log_adapter(&info, false);
            if let Some(surface) = compatible_surface
                && !adapter.is_surface_supported(surface)
            {
                rejected.push(format!(
                    "{} ({:?}): cannot present",
                    info.name, info.backend
                ));
                continue;
            }
            match rank_adapter(&info, power, options.allow_software, name_filter.as_deref()) {
                Some(rank) => candidates.push((rank, adapter)),
                None => rejected.push(format!(
                    "{} ({:?}, {:?}): filtered out",
                    info.name, info.backend, info.device_type
                )),
            }
        }
        // Best first; stable so enumeration order breaks ties.
        candidates.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));

        // Try each candidate until one yields a device: some drivers expose an
        // adapter that then fails device creation (e.g. GL via a vendor EGL
        // without a display).
        let mut last_device_error = None;
        let mut opened = None;
        for (_, adapter) in candidates {
            let info = adapter.get_info();
            let adapter_limits = adapter.limits();
            let required_limits = options.required_limits.clone().unwrap_or_else(|| {
                wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter_limits.clone())
                    .using_alignment(adapter_limits.clone())
            });
            // The only optional feature: timestamp queries for the UI pass timer, when the
            // adapter offers them (render.md §6.16 U2).
            let required_features = if options.timestamp_queries {
                adapter.features() & wgpu::Features::TIMESTAMP_QUERY
            } else {
                wgpu::Features::empty()
            };
            let request = adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("tessera-device"),
                required_features,
                required_limits,
                ..Default::default()
            });
            match pollster::block_on(request) {
                Ok((device, queue)) => {
                    log_adapter(&info, true);
                    opened = Some((adapter, info, device, queue));
                    break;
                }
                Err(e) => {
                    tracing::warn!(name = %info.name, backend = ?info.backend, error = %e,
                        "device creation failed; trying next adapter");
                    rejected.push(format!("{} ({:?}): {e}", info.name, info.backend));
                    last_device_error = Some(e);
                }
            }
        }

        let Some((adapter, info, device, queue)) = opened else {
            return Err(no_device_error(
                format!("{backends:?}"),
                &rejected,
                last_device_error,
            ));
        };

        let errors = ErrorSink::default();
        {
            let sink = errors.clone();
            device.on_uncaptured_error(Arc::new(move |err: wgpu::Error| {
                let msg = err.to_string();
                tracing::error!(error = %msg, "uncaptured wgpu error");
                sink.record(msg);
            }));
        }
        device.set_device_lost_callback(|reason, msg| {
            tracing::warn!(?reason, %msg, "GPU device lost");
        });

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            info,
            errors,
        })
    }

    /// Block until all submitted work is complete, or fail with
    /// [`RenderError::Poll`] after [`GPU_WAIT_TIMEOUT`].
    pub fn wait_idle(&self) -> Result<()> {
        self.device.poll(bounded_wait())?;
        Ok(())
    }
}

/// Adapter so callers can hand us any boxed display-handle provider without
/// naming wgpu's internal `WgpuHasDisplayHandle` trait.
struct DisplayBox(Box<dyn wgpu::rwh::HasDisplayHandle + Send + Sync>);

impl fmt::Debug for DisplayBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DisplayBox(..)")
    }
}

impl wgpu::rwh::HasDisplayHandle for DisplayBox {
    fn display_handle(
        &self,
    ) -> std::result::Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        self.0.display_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, ty: wgpu::DeviceType, backend: wgpu::Backend) -> wgpu::AdapterInfo {
        let mut info = wgpu::AdapterInfo::new(ty, backend);
        info.name = name.to_owned();
        info
    }

    #[test]
    fn ranking_prefers_hardware_and_respects_filters() {
        use wgpu::{Backend as B, DeviceType as D, PowerPreference as P};
        let discrete = info("Big GPU", D::DiscreteGpu, B::Vulkan);
        let integrated = info("Small GPU", D::IntegratedGpu, B::Vulkan);
        let cpu = info("llvmpipe", D::Cpu, B::Vulkan);
        let gl = info("Big GPU", D::DiscreteGpu, B::Gl);

        let hp = |i| rank_adapter(i, P::HighPerformance, true, None).unwrap_or(0);
        assert!(hp(&discrete) > hp(&integrated));
        assert!(hp(&integrated) > hp(&cpu));
        assert!(hp(&discrete) > hp(&gl));

        let lp = |i| rank_adapter(i, P::LowPower, true, None).unwrap_or(0);
        assert!(lp(&integrated) > lp(&discrete));

        assert_eq!(rank_adapter(&cpu, P::None, false, None), None);
        assert_eq!(rank_adapter(&discrete, P::None, true, Some("small")), None);
        assert!(rank_adapter(&integrated, P::None, true, Some("small")).is_some());
        // The filter itself is case-insensitive, not just the adapter name.
        assert!(rank_adapter(&discrete, P::None, true, Some("Big")).is_some());
        assert!(rank_adapter(&discrete, P::None, true, Some(" BIG gpu ")).is_some());
        let nv = info("NVIDIA GeForce", D::DiscreteGpu, B::Vulkan);
        assert!(rank_adapter(&nv, P::None, true, Some("NVIDIA")).is_some());
        assert_eq!(rank_adapter(&nv, P::None, true, Some("Radeon")), None);
    }

    #[test]
    fn explicit_power_preference_wins() {
        // Explicit values never consult the environment.
        for p in [
            wgpu::PowerPreference::LowPower,
            wgpu::PowerPreference::HighPerformance,
            wgpu::PowerPreference::None,
        ] {
            let o = ContextOptions {
                power_preference: Some(p),
                ..ContextOptions::default()
            };
            assert_eq!(o.resolved_power_preference(), p);
        }
        let o = ContextOptions {
            backends: Some(wgpu::Backends::GL),
            adapter_name: Some("  Big ".into()),
            ..ContextOptions::default()
        };
        assert_eq!(o.resolved_backends(), wgpu::Backends::GL);
        assert_eq!(o.resolved_adapter_name().as_deref(), Some("big"));
    }

    #[test]
    fn error_sink_keeps_first_error() {
        let sink = ErrorSink::default();
        assert!(sink.check().is_ok());
        sink.record("a".into());
        sink.record("b".into());
        assert_eq!(sink.take().as_deref(), Some("a"));
        assert!(sink.take().is_none());
    }
}
