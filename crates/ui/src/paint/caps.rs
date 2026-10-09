// SPDX-License-Identifier: GPL-3.0-or-later
//! Caps against hostile input (decision 0015 §7) and the U3–U5 budgets (§6).
//!
//! Every value is a Tessera setting with the decision's default. The caps on surfaces are
//! applied when an element would get a surface: a non-root element over a cap gets no
//! surface and is painted into its parent's canvas as if its mode were 0; a window over a
//! cap draws nothing. Each case gives one warning per plugin (or one `tracing` warning for
//! native panels) and a counter in [`CapCounters`].

use std::collections::HashMap;
use std::time::Duration;

use super::canvas::MAX_CANVAS_DIM;
use super::geom::Rect;

/// Paint and composition limits (Tessera settings; defaults from decision 0015 §6 and §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintLimits {
    /// Largest canvas side: the smaller of 16,384 and the device's
    /// `max_texture_dimension_2d`.
    pub max_surface_dim: u32,
    /// Surfaces per plugin apartment (windows and controls together).
    pub max_surfaces_per_apartment: u32,
    /// Surfaces in total, native and plugin, kept ones included.
    pub max_surfaces_total: u32,
    /// Composition entries per frame (nested entries, canvases above and external content);
    /// [`super::ComposeConfig::from_limits`] hands it to the builder.
    pub max_entries: usize,
    /// Levels of nested surfaces below a root (also handed to the builder).
    pub max_depth: u32,
    /// CPU canvases including canvases above (U5).
    pub canvas_bytes: u64,
    /// GPU surface textures, front and back (U5).
    pub texture_bytes: u64,
    /// UI upload bytes per frame, hard cap (U3).
    pub upload_cap_bytes: u64,
    /// UI paint time per frame on the main thread, the slice (U4 (b)).
    pub paint_slice: Duration,
    /// Longest wait of a first paint (U4 (c)).
    pub first_paint_wait: Duration,
}

impl Default for PaintLimits {
    fn default() -> Self {
        Self {
            max_surface_dim: MAX_CANVAS_DIM,
            max_surfaces_per_apartment: 512,
            max_surfaces_total: 4096,
            max_entries: 8192,
            max_depth: 8,
            canvas_bytes: 128 * 1024 * 1024,
            texture_bytes: 256 * 1024 * 1024,
            upload_cap_bytes: 8 * 1024 * 1024,
            paint_slice: Duration::from_millis(4),
            first_paint_wait: Duration::from_millis(100),
        }
    }
}

impl PaintLimits {
    /// The limits with the surface side capped by a device limit.
    #[must_use]
    pub fn with_device_max_dim(mut self, device_max: u32) -> Self {
        self.max_surface_dim = device_max.clamp(1, MAX_CANVAS_DIM);
        self
    }
}

/// The canvas of a surface after the size cap of §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasExtent {
    /// The stored canvas size.
    pub size: [u32; 2],
    /// Position of the stored part inside the element's layout box (0 unless cropped).
    pub origin: [i32; 2],
    /// Whether an axis was cropped (reported once as a categorised failure).
    pub cropped: bool,
}

/// The canvas of an element whose layout box is `layout` (in screen pixels, unscaled
/// canvas pixels and screen pixels coincide here) on a screen of `screen` size (§7):
///
/// - within `max_dim` on an axis, the canvas covers the whole box on that axis with origin
///   0, also when the element lies partly off the screen;
/// - over it, the canvas covers only the part of the box on the screen on that axis (at
///   most `max_dim`), and `origin` is that part's position inside the element.
///
/// `None` for an empty box or an over-size axis with no part on the screen (no canvas).
#[must_use]
pub fn canvas_extent(layout: &Rect, screen: [u32; 2], max_dim: u32) -> Option<CanvasExtent> {
    if layout.is_empty() {
        return None;
    }
    let max_dim = max_dim.clamp(1, MAX_CANVAS_DIM);
    let axis = |lo: i32, len: u32, screen_len: u32| -> Option<(u32, i32, bool)> {
        if len <= max_dim {
            return Some((len, 0, false));
        }
        let lo64 = i64::from(lo);
        let start = lo64.max(0);
        let end = (lo64 + i64::from(len)).min(i64::from(screen_len));
        if end <= start {
            return None;
        }
        let part = u32::try_from((end - start).min(i64::from(max_dim))).ok()?;
        let origin = i32::try_from(start - lo64).ok()?;
        Some((part, origin, true))
    };
    let (w, ox, cx) = axis(layout.x0, layout.width(), screen[0])?;
    let (h, oy, cy) = axis(layout.y0, layout.height(), screen[1])?;
    Some(CanvasExtent {
        size: [w, h],
        origin: [ox, oy],
        cropped: cx || cy,
    })
}

/// Why an element was refused a surface (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum CapRefusal {
    /// The total number of surfaces is reached.
    #[error("UI surface cap reached ({0} surfaces in total)")]
    TotalSurfaces(u32),
    /// The plugin apartment's number of surfaces is reached.
    #[error("plugin surface cap reached ({0} surfaces in the apartment)")]
    ApartmentSurfaces(u32),
    /// The element lies deeper than the nesting cap.
    #[error("nested surface deeper than {0} levels")]
    Depth(u32),
    /// The canvas or texture memory budget (U5) is reached.
    #[error("UI surface memory budget reached")]
    Memory,
}

/// Counters of the §7 fallbacks, handed to the renderer's `FrameStats` (render.md §6.5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapCounters {
    /// Surfaces refused by the total cap.
    pub total_surfaces: u64,
    /// Surfaces refused by an apartment cap.
    pub apartment_surfaces: u64,
    /// Surfaces refused by the depth cap.
    pub depth: u64,
    /// Surfaces or textures refused by the memory budget.
    pub memory: u64,
    /// Elements below a Tile surface painted into its canvas (Decision 1.7).
    pub tile_parent: u64,
    /// Composition entries dropped past the entry cap.
    pub entries: u64,
    /// Canvases cropped to the screen (size cap).
    pub cropped: u64,
    /// Back textures not created for lack of texture memory (the job waits).
    pub back_texture_waits: u64,
    /// Snapshots whose canvas size could not be stored (0, or over the surface size cap);
    /// the surface was treated as having nothing to paint (a categorised failure).
    pub invalid_snapshots: u64,
}

impl CapCounters {
    /// Adds a refusal.
    pub fn count(&mut self, r: CapRefusal) {
        match r {
            CapRefusal::TotalSurfaces(_) => self.total_surfaces += 1,
            CapRefusal::ApartmentSurfaces(_) => self.apartment_surfaces += 1,
            CapRefusal::Depth(_) => self.depth += 1,
            CapRefusal::Memory => self.memory += 1,
        }
    }
}

/// Memory figures for admitting one surface against U5 (§7: a surface whose canvas or
/// texture **would push past** the budget is refused).
///
/// The `*_new` figures are the caller's estimate of what the new surface will hold. Canvases
/// are sparse, so its full size is only an upper bound: [`MemoryUse::full_size`] gives
/// that bound. Painting enforces the canvas budget again tile by tile
/// ([`super::CanvasSet::set_byte_limit`]), and the scheduler refuses textures that do not
/// fit, so an estimate below the bound is safe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryUse {
    /// Bytes of CPU canvases allocated now.
    pub canvas_now: u64,
    /// Bytes of UI textures allocated now.
    pub texture_now: u64,
    /// Canvas bytes the new surface is expected to need.
    pub canvas_new: u64,
    /// Texture bytes the new surface is expected to need.
    pub texture_new: u64,
}

impl MemoryUse {
    /// The current figures with the upper bound for a new canvas of `size`: every tile
    /// allocated (`width × height × 4` bytes, for the canvas and for its texture).
    #[must_use]
    pub fn full_size(canvas_now: u64, texture_now: u64, size: [u32; 2]) -> Self {
        let bytes = u64::from(size[0]) * u64::from(size[1]) * 4;
        Self {
            canvas_now,
            texture_now,
            canvas_new: bytes,
            texture_new: bytes,
        }
    }

    /// Whether the new surface fits both budgets of `limits`.
    #[must_use]
    pub fn fits(&self, limits: &PaintLimits) -> bool {
        let fit = |now: u64, new: u64, limit: u64| now < limit && now.saturating_add(new) <= limit;
        fit(self.canvas_now, self.canvas_new, limits.canvas_bytes)
            && fit(self.texture_now, self.texture_new, limits.texture_bytes)
    }
}

/// Counts surfaces against the caps of §7.
#[derive(Debug, Clone, Default)]
pub struct SurfaceCounter {
    limits: PaintLimits,
    total: u32,
    per_apartment: HashMap<u32, u32>,
    counters: CapCounters,
    warned: [bool; 4],
}

impl SurfaceCounter {
    /// A counter with `limits`.
    #[must_use]
    pub fn new(limits: PaintLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// Asks for one surface for an element `depth` levels of nested surfaces below its root
    /// (0 for a root), in plugin apartment `apartment` (`None` for native panels), with the
    /// memory figures `memory`: it is refused when the budget is full already or when the
    /// new surface would push canvases or textures past it ([`MemoryUse::fits`]).
    ///
    /// # Errors
    /// The cap that refuses it; the refusal is counted, and the first refusal of each kind
    /// gives one `tracing` warning. The per-plugin warning to the error listeners is the
    /// plugin host's (it sees the refusal).
    pub fn admit(
        &mut self,
        apartment: Option<u32>,
        depth: u32,
        memory: MemoryUse,
    ) -> Result<(), CapRefusal> {
        let r = if depth > self.limits.max_depth {
            Err(CapRefusal::Depth(self.limits.max_depth))
        } else if self.total >= self.limits.max_surfaces_total {
            Err(CapRefusal::TotalSurfaces(self.limits.max_surfaces_total))
        } else if apartment.is_some_and(|a| {
            self.per_apartment.get(&a).copied().unwrap_or(0)
                >= self.limits.max_surfaces_per_apartment
        }) {
            Err(CapRefusal::ApartmentSurfaces(
                self.limits.max_surfaces_per_apartment,
            ))
        } else if !memory.fits(&self.limits) {
            Err(CapRefusal::Memory)
        } else {
            Ok(())
        };
        match r {
            Ok(()) => {
                self.total += 1;
                if let Some(a) = apartment {
                    *self.per_apartment.entry(a).or_insert(0) += 1;
                }
                Ok(())
            }
            Err(e) => {
                self.counters.count(e);
                let k = match e {
                    CapRefusal::TotalSurfaces(_) => 0,
                    CapRefusal::ApartmentSurfaces(_) => 1,
                    CapRefusal::Depth(_) => 2,
                    CapRefusal::Memory => 3,
                };
                if !self.warned[k] {
                    self.warned[k] = true;
                    tracing::warn!(
                        "{e}: the element is drawn into its parent's canvas (a window draws nothing)"
                    );
                }
                Err(e)
            }
        }
    }

    /// Returns one surface.
    pub fn release(&mut self, apartment: Option<u32>) {
        self.total = self.total.saturating_sub(1);
        if let Some(a) = apartment
            && let Some(n) = self.per_apartment.get_mut(&a)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.per_apartment.remove(&a);
            }
        }
    }

    /// Surfaces counted now.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.total
    }

    /// The refusal counters.
    #[must_use]
    pub fn counters(&self) -> CapCounters {
        self.counters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_within_limits_is_whole() {
        let e = canvas_extent(&Rect::from_xywh(-50, -10, 300, 200), [1920, 1080], 16_384).unwrap();
        assert_eq!(e.size, [300, 200]);
        assert_eq!(e.origin, [0, 0]);
        assert!(!e.cropped);
    }

    #[test]
    fn extent_over_limit_is_cropped_to_screen() {
        let e = canvas_extent(
            &Rect::from_xywh(-1000, 0, 20_000, 100),
            [3840, 2160],
            16_384,
        )
        .unwrap();
        assert_eq!(e.size, [3840, 100]);
        assert_eq!(e.origin, [1000, 0]);
        assert!(e.cropped);
        assert!(
            canvas_extent(&Rect::from_xywh(5000, 0, 20_000, 10), [3840, 2160], 16_384).is_none()
        );
        assert!(canvas_extent(&Rect::EMPTY, [10, 10], 16).is_none());
    }

    #[test]
    fn surface_caps() {
        let limits = PaintLimits {
            max_surfaces_total: 3,
            max_surfaces_per_apartment: 2,
            ..PaintLimits::default()
        };
        let mut c = SurfaceCounter::new(limits);
        let none = MemoryUse::default();
        c.admit(Some(1), 0, none).unwrap();
        c.admit(Some(1), 1, none).unwrap();
        assert_eq!(
            c.admit(Some(1), 1, none),
            Err(CapRefusal::ApartmentSurfaces(2))
        );
        c.admit(None, 0, none).unwrap();
        assert_eq!(c.admit(Some(2), 0, none), Err(CapRefusal::TotalSurfaces(3)));
        c.release(Some(1));
        assert_eq!(c.admit(Some(2), 9, none), Err(CapRefusal::Depth(8)));
        let full = MemoryUse {
            canvas_now: u64::MAX,
            ..none
        };
        assert_eq!(c.admit(Some(2), 0, full), Err(CapRefusal::Memory));
        c.admit(Some(2), 8, none).unwrap();
        let k = c.counters();
        assert_eq!(
            (k.apartment_surfaces, k.total_surfaces, k.depth, k.memory),
            (1, 1, 1, 1)
        );
    }

    #[test]
    fn a_surface_that_would_push_past_u5_is_refused() {
        let lim = PaintLimits::default();
        let mut c = SurfaceCounter::new(lim);
        let mib = 1024 * 1024;
        // 127 MiB of canvases in use: a 16,384 × 16,384 surface (1 GiB) would push past.
        let big = MemoryUse::full_size(127 * mib, 0, [16_384, 16_384]);
        assert_eq!(c.admit(None, 0, big), Err(CapRefusal::Memory));
        // A 16 × 16 control still fits; so does a sparse estimate of a large window.
        assert!(
            c.admit(None, 0, MemoryUse::full_size(127 * mib, 0, [16, 16]))
                .is_ok()
        );
        let sparse = MemoryUse {
            canvas_now: 127 * mib,
            canvas_new: 256 * 1024,
            ..MemoryUse::default()
        };
        assert!(c.admit(None, 0, sparse).is_ok());
        // The texture budget is checked too.
        let tex = MemoryUse {
            texture_now: 200 * mib,
            texture_new: 64 * mib,
            ..MemoryUse::default()
        };
        assert_eq!(c.admit(None, 0, tex), Err(CapRefusal::Memory));
        // Exactly at the budget fits; one byte over does not.
        let exact = MemoryUse {
            canvas_now: 100 * mib,
            canvas_new: 28 * mib,
            ..MemoryUse::default()
        };
        assert!(exact.fits(&lim));
        assert!(
            !MemoryUse {
                canvas_new: 28 * mib + 1,
                ..exact
            }
            .fits(&lim)
        );
    }
}
