// SPDX-License-Identifier: GPL-3.0-or-later
//! Sparse CPU canvases, the painter, damage tracking, paint jobs and the composition list
//! (decision 0015; `ui-layout.md` §9.6; `fonts.md` §8.4) and the cache of decoded images
//! (decision 0015 §4).
//!
//! The CPU side of the UI rendering decision:
//!
//! - [`Canvas`] / [`CanvasSet`]: sparse 32-bit canvases (`0xAARRGGBB`, straight alpha,
//!   256 × 256 tiles), one set per surface-owning element: the canvas below and the canvases
//!   above its nested entries (Decision 1.1, 1.2, 1.7).
//! - [`DisplayList`] / [`PaintOp`]: the paint primitives this module accepts: image layers
//!   tiled at native size, overlay layers, opaque fills and glyph coverage runs, plus
//!   nested-entry markers. [`paint_rect`] is the byte-exact painter (`ui-layout.md`
//!   §9.6.1–§9.6.3, `fonts.md` §8.4); the per-pixel arithmetic is public in [`ops`].
//! - [`DamageList`]: damage by rectangle and the per-frame merge (Decision 1.4).
//! - [`PaintJob`] and [`Scheduler`]: jobs in bands over frames, uploads under the cap,
//!   back textures, generations that swap once, first paints within the wait (Decision
//!   1.5, 1.8). The scheduler emits [`GpuOp`]s for the client.
//! - [`coverage_region`]: coverage regions of nested entries, with M5-2 option (a).
//! - [`build`]: the composition list (Decision 1.6, `render.md` §6.5–§6.8), with M5-1's
//!   clip chain.
//! - [`ImageCache`]: pinned images, an LRU and a hard cap (§4.5).
//! - [`PaintLimits`], [`SurfaceCounter`], [`canvas_extent`]: the caps of §7.

mod cache;
mod canvas;
mod caps;
mod compose;
mod coverage;
mod damage;
mod display;
mod geom;
mod job;
pub mod ops;
mod painter;
mod scheduler;

pub use cache::{
    CacheError, CacheStats, DEFAULT_LRU_BYTES, DEFAULT_PINNED_CAP_BYTES, ImageCache, ImageKey,
    MAX_PATH_KEY,
};
pub use canvas::{Canvas, MAX_CANVAS_DIM, TILE};
pub use caps::{
    CanvasExtent, CapCounters, CapRefusal, MemoryUse, PaintLimits, SurfaceCounter, canvas_extent,
};
pub use compose::{
    Addressing, Clip, CompNode, ComposeConfig, CompositionList, Entry, EntryTexture, Filter,
    MAX_DEPTH, MAX_NODES, MAX_TREE_DEPTH, NodeContent, OpacityRule, Rotation, Source,
    TextureLookup, build,
};
pub use coverage::{
    ANGLE_EPSILON, FootprintNode, MAX_FOOTPRINT_DEPTH, ParentCanvas, RectF, coverage_change_damage,
    coverage_region, footprint, holds_rotation,
};
pub use damage::{DamageList, MAX_MERGED, MAX_PENDING, merge};
pub use display::{
    CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun, ImageLayer, MAX_COVERAGE_DIM,
    MAX_GLYPHS_PER_RUN, MAX_NESTED, MAX_OPS, OverlayLayer, PaintOp,
};
pub use geom::Rect;
pub use job::{BAND_ROWS, BandBudget, PaintBudget, PaintJob, TimeSlice, Unlimited};
pub use ops::{CompositeOp, opacity_byte, pack_colour};
pub use painter::{CanvasSet, Layer, paint_rect};
pub use scheduler::{
    FrameOutput, GpuOp, GroupId, PaintFrameStats, Scheduler, Slot, Snapshot, SnapshotSource,
    SurfaceId, TexKey,
};

/// Errors of the paint module.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaintError {
    /// A canvas side is 0 or above [`MAX_CANVAS_DIM`].
    #[error("invalid canvas size {width} x {height}")]
    CanvasSize {
        /// Width asked for.
        width: u32,
        /// Height asked for.
        height: u32,
    },
    /// A coverage map whose size and data disagree or exceed [`MAX_COVERAGE_DIM`].
    #[error("invalid coverage map {width} x {height}")]
    CoverageSize {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
    },
    /// A display list over [`MAX_OPS`] operations or [`MAX_NESTED`] markers.
    #[error("display list over its operation cap")]
    TooManyOps,
    /// A glyph run over [`MAX_GLYPHS_PER_RUN`] boxes.
    #[error("glyph run over its cap")]
    TooManyGlyphs,
    /// A surface id registered twice.
    #[error("surface {0} already exists")]
    DuplicateSurface(u32),
    /// A malformed composition input.
    #[error("invalid composition tree: {0}")]
    InvalidTree(&'static str),
}
