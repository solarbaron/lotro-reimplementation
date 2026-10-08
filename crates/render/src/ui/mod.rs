// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI pass (`docs/specs/render.md` §6, decision 0015 §8).
//!
//! The game UI is painted on the CPU into one canvas per panel surface (by
//! `tessera-ui`, which this crate does not depend on). The renderer only
//! stores those canvases in **UI textures** and composes them onto the frame:
//!
//! - [`crate::Renderer::create_ui_texture`] makes a linear `Bgra8Unorm`
//!   texture (a canvas pixel 0xAARRGGBB uploads unconverted, straight alpha);
//!   [`crate::Renderer::update_texture_rect`],
//!   [`crate::Renderer::copy_texture_rect`] and
//!   [`crate::Renderer::clear_texture_rect`] write it, in call order.
//! - [`crate::Frame::ui_pass`] records a [`UiPass`]: an ordered list of
//!   [`UiEntry`]s (one quad each, with a transform chain of
//!   [`UiRotation`]s and a clip chain of [`UiClip`]s) that the client fills
//!   from `tessera-ui`'s composition list.
//! - [`crate::Frame::software_cursor`] draws one more quad after every pass.
//!
//! The UI pass draws into a **non-sRGB view** of the frame's target, so the
//! premultiplied source-over blend ([`UI_BLEND_COLOR`], [`UI_BLEND_ALPHA`])
//! works on stored 8-bit values like the original. Where the backend cannot
//! create that view the shader linearises its output instead
//! ([`UiViewMode::LinearFallback`]).
//!
//! ```no_run
//! # fn main() -> Result<(), tessera_render::RenderError> {
//! use tessera_render::*;
//! let mut r = Renderer::new_headless(&RendererConfig::default())?;
//! // A 64 × 32 canvas, uploaded as B, G, R, A bytes.
//! let tex = r.create_ui_texture(64, 32, Some("panel"))?;
//! let canvas = vec![0x80u8; 64 * 32 * 4];
//! r.update_texture_rect(tex, 0, 0, 64, 32, &canvas, 64 * 4)?;
//! let target = r.create_offscreen_target(256, 256, OffscreenFormat::Rgba8Srgb)?;
//! let mut frame = r.begin_offscreen_frame(&target)?;
//! frame.pass("world", PassSpace::Screen).clear([0.0, 0.0, 0.0, 1.0]);
//! frame
//!     .ui_pass()
//!     .entry(&UiEntry::new(tex, [64, 32], UiRect::new(10, 10, 64, 32)));
//! let stats = r.end_frame(frame)?;
//! assert_eq!(stats.ui.quads, 1);
//! # Ok(()) }
//! ```

mod api;
mod geometry;
mod ops;
mod state;

pub(crate) use api::UiParts;
pub use geometry::{UiClipMap, UiGeometry};
pub(crate) use state::{FrameUi, UiState, offscreen_view_formats, surface_view_formats, ui_view};

use api::scoped;

use crate::texture::TextureId;

/// Colour blend of the UI pass: premultiplied source-over (source `One`,
/// destination `OneMinusSrcAlpha`, `Add`). Named so that a frame capture
/// settling `ui-layout.md` §9.6.1 changes one place (render.md §6.7 step 7).
pub const UI_BLEND_COLOR: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::One,
    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
    operation: wgpu::BlendOperation::Add,
};

/// Alpha blend of the UI pass (the same as [`UI_BLEND_COLOR`]).
pub const UI_BLEND_ALPHA: wgpu::BlendComponent = UI_BLEND_COLOR;

/// Most composition entries drawn per frame, counting every UI pass of the
/// frame (decision 0015 §7). Entries past it are dropped and counted as
/// refused. The software cursor is not an entry.
pub const MAX_UI_ENTRIES: usize = 8192;

/// Most rotations in an entry's transform chain (its own surface plus at
/// most 8 surfaced ancestors).
pub const MAX_UI_ROTATIONS: usize = 9;

/// Most items in an entry's clip chain.
pub const MAX_UI_CLIPS: usize = 8;

/// Largest UI texture and canvas side (decision 0015 §7); the device limit
/// `max_texture_dimension_2d` may be smaller.
pub const MAX_UI_TEXTURE_DIM: u32 = 16_384;

/// Largest displayed-box side and coordinate magnitude, in pixels (Tessera
/// bound so that every position stays exact in `f32`).
pub const MAX_UI_COORD: i64 = 1 << 24;

/// Smallest and largest accepted Tile node scale (Tessera bound for hostile
/// input; panel scales are 0.4–2 in practice).
pub const UI_NODE_SCALE_RANGE: (f32, f32) = (1.0e-4, 1.0e4);

/// Angles within this many degrees of 0 count as no rotation (decision 0015
/// §2.3).
pub const UI_ANGLE_EPSILON: f32 = 0.0002;

/// How texels of an entry's texture are to be read (render.md §6.5
/// `source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiSource {
    /// Straight alpha (UI textures): the shader premultiplies each texel.
    #[default]
    Straight,
    /// Already premultiplied (external content, §6.10): used as read.
    Premultiplied,
}

/// How the canvas covers the displayed box (render.md §6.5 `addressing`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum UiAddressing {
    /// The full canvas is scaled onto the box (clamp-to-edge filtering).
    #[default]
    Fit,
    /// The canvas repeats from the box's top-left corner at the display
    /// node's scale (each repeat covers `canvas × node_scale` pixels).
    Tile {
        /// Display-node scale `(sx, sy)`, finite and within
        /// [`UI_NODE_SCALE_RANGE`].
        node_scale: [f32; 2],
    },
}

/// Scale filter of a scaled or rotated entry (Tessera setting
/// `ui_scale_filter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiFilter {
    /// Bilinear on premultiplied texels.
    #[default]
    Linear,
    /// The texel under the position.
    Nearest,
}

/// An integer rectangle in physical pixels: `x`, `y` is the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width (≥ 1 for a displayed box).
    pub w: u32,
    /// Height (≥ 1 for a displayed box).
    pub h: u32,
}

impl UiRect {
    /// A rectangle from its top-left corner and size.
    #[must_use]
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// The same rectangle as half-open edges `[x0, y0, x1, y1]` (saturating).
    #[must_use]
    pub fn edges(&self) -> [i32; 4] {
        let x1 = i64::from(self.x) + i64::from(self.w);
        let y1 = i64::from(self.y) + i64::from(self.h);
        [
            self.x,
            self.y,
            i32::try_from(x1).unwrap_or(i32::MAX),
            i32::try_from(y1).unwrap_or(i32::MAX),
        ]
    }
}

/// One rotation of an entry's transform chain (render.md §6.6): three angles
/// in degrees, applied *x*, then *y*, then *z*, about `centre` (the centre of
/// the displayed box of the surface that holds it), with *x* right, *y* down
/// and *z* = 0 on the screen. A positive *z* turns counter-clockwise on
/// screen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiRotation {
    /// Centre in physical pixels.
    pub centre: [f32; 2],
    /// `[x, y, z]` angles in degrees. A non-finite angle counts as 0.
    pub angles: [f32; 3],
}

impl UiRotation {
    /// A rotation about `centre` by `z` degrees in the screen plane.
    #[must_use]
    pub const fn z(centre: [f32; 2], z: f32) -> Self {
        Self {
            centre,
            angles: [0.0, 0.0, z],
        }
    }
}

/// One item of an entry's clip chain (render.md §6.8): a half-open integer
/// rectangle `[x0, x1) × [y0, y1)` tested in space `space`, which is the
/// number of the entry's rotations applied before the test. `space` equal
/// to the number of rotations means screen space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiClip {
    /// `[x0, y0, x1, y1]`.
    pub rect: [i32; 4],
    /// Space index *k*, `0 ..= rotations.len()`.
    pub space: u32,
}

impl UiClip {
    /// A clip item from a rectangle and its space index.
    #[must_use]
    pub fn new(rect: UiRect, space: u32) -> Self {
        Self {
            rect: rect.edges(),
            space,
        }
    }
}

/// One composition entry (render.md §6.5): a quad that draws (part of) a
/// surface's canvas onto its displayed box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiEntry<'a> {
    /// A UI texture, or a registered `Rgba8Unorm` render target (external
    /// content).
    pub texture: TextureId,
    /// Straight (UI textures) or premultiplied (external content).
    pub source: UiSource,
    /// Size of the **full** canvas `(cw, ch)`, 1 ..= [`MAX_UI_TEXTURE_DIM`].
    pub canvas: [u32; 2],
    /// Canvas position of the texture's texel (0, 0) (tile-box offset plus a
    /// cropped canvas's origin offset).
    pub tex_offset: [i32; 2],
    /// Displayed box in physical pixels (the root's unrotated space).
    pub dest: UiRect,
    /// Fit or Tile.
    pub addressing: UiAddressing,
    /// Scale filter.
    pub filter: UiFilter,
    /// Opacity, clamped to 0–1 when drawn; NaN counts as 0.
    pub opacity: f32,
    /// Alpha threshold *t* (0–1): texels whose alpha is below it are dropped,
    /// compared **before** opacity. NaN counts as 0.
    pub alpha_threshold: f32,
    /// Rotations, nearest first (own surface, then surfaced ancestors up to
    /// the root); at most [`MAX_UI_ROTATIONS`].
    pub rotations: &'a [UiRotation],
    /// Clip chain; at most [`MAX_UI_CLIPS`] items.
    pub clips: &'a [UiClip],
}

impl<'a> UiEntry<'a> {
    /// An unrotated, unclipped Fit entry of a UI texture whose texel (0, 0)
    /// is canvas texel (0, 0), at opacity 1 with no threshold.
    #[must_use]
    pub fn new(texture: TextureId, canvas: [u32; 2], dest: UiRect) -> Self {
        Self {
            texture,
            source: UiSource::Straight,
            canvas,
            tex_offset: [0, 0],
            dest,
            addressing: UiAddressing::Fit,
            filter: UiFilter::Linear,
            opacity: 1.0,
            alpha_threshold: 0.0,
            rotations: &[],
            clips: &[],
        }
    }

    /// Set the source flag.
    #[must_use]
    pub fn with_source(mut self, source: UiSource) -> Self {
        self.source = source;
        self
    }

    /// Set the texture's offset in the canvas.
    #[must_use]
    pub fn with_tex_offset(mut self, offset: [i32; 2]) -> Self {
        self.tex_offset = offset;
        self
    }

    /// Set the addressing mode.
    #[must_use]
    pub fn with_addressing(mut self, addressing: UiAddressing) -> Self {
        self.addressing = addressing;
        self
    }

    /// Set the scale filter.
    #[must_use]
    pub fn with_filter(mut self, filter: UiFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Set the opacity.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Set the alpha threshold.
    #[must_use]
    pub fn with_alpha_threshold(mut self, threshold: f32) -> Self {
        self.alpha_threshold = threshold;
        self
    }

    /// Set the transform chain.
    #[must_use]
    pub fn with_rotations(mut self, rotations: &'a [UiRotation]) -> Self {
        self.rotations = rotations;
        self
    }

    /// Set the clip chain.
    #[must_use]
    pub fn with_clips(mut self, clips: &'a [UiClip]) -> Self {
        self.clips = clips;
        self
    }
}

/// Why an entry was refused (render.md §6.5). Each refusal adds 1 to
/// [`UiStats::entries_refused`] and gives one `tracing` warning per kind and
/// renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum UiRefusal {
    /// More than [`MAX_UI_ENTRIES`] entries in the frame.
    #[error("more than {MAX_UI_ENTRIES} UI entries in one frame")]
    EntryCap,
    /// A canvas side is 0 or above [`MAX_UI_TEXTURE_DIM`].
    #[error("canvas size outside 1..={MAX_UI_TEXTURE_DIM}")]
    Canvas,
    /// The displayed box is empty or beyond [`MAX_UI_COORD`].
    #[error("displayed box empty or out of range")]
    Dest,
    /// A Tile node scale that is not finite or outside
    /// [`UI_NODE_SCALE_RANGE`].
    #[error("Tile node scale not finite or out of range")]
    NodeScale,
    /// More than [`MAX_UI_ROTATIONS`] rotations, or a rotation centre that
    /// is not finite or beyond [`MAX_UI_COORD`].
    #[error("too many rotations or a bad rotation centre")]
    Rotations,
    /// More than [`MAX_UI_CLIPS`] clip items.
    #[error("more than {MAX_UI_CLIPS} clip items")]
    TooManyClips,
    /// A clip item's space index is larger than the number of rotations.
    #[error("clip space index out of range")]
    ClipSpace,
    /// The texture is unknown, destroyed or from another renderer.
    #[error("unknown, destroyed or foreign texture")]
    UnknownTexture,
    /// The texture is neither a UI texture nor a registered `Rgba8Unorm`
    /// target.
    #[error("texture is not UI content")]
    NotUiContent,
    /// The texture is a registered sRGB target (render.md §6.10).
    #[error("sRGB render target refused as UI content")]
    SrgbTarget,
    /// The texture is the frame's own colour target (feedback loop).
    #[error("entry reads the frame's own colour target")]
    OwnTarget,
}

impl UiRefusal {
    /// Every kind, in index order.
    pub(crate) const ALL: [Self; 11] = [
        Self::EntryCap,
        Self::Canvas,
        Self::Dest,
        Self::NodeScale,
        Self::Rotations,
        Self::TooManyClips,
        Self::ClipSpace,
        Self::UnknownTexture,
        Self::NotUiContent,
        Self::SrgbTarget,
        Self::OwnTarget,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// Refusals counted per kind (bounded storage for any number of refusals).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RefusalCounts([u32; UiRefusal::ALL.len()]);

impl RefusalCounts {
    pub(crate) fn add(&mut self, kind: UiRefusal) {
        let i = kind.index();
        self.0[i] = self.0[i].saturating_add(1);
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a = a.saturating_add(b);
        }
    }

    pub(crate) fn total(&self) -> u32 {
        self.0.iter().fold(0u32, |a, b| a.saturating_add(*b))
    }

    /// Kinds with at least one refusal.
    pub(crate) fn kinds(&self) -> impl Iterator<Item = UiRefusal> + '_ {
        UiRefusal::ALL
            .iter()
            .zip(self.0)
            .filter(|(_, n)| *n > 0)
            .map(|(k, _)| *k)
    }
}

/// How the UI pass reaches stored values (render.md §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiViewMode {
    /// The UI pass renders into a non-sRGB view: blending works on stored
    /// bytes, and stored bytes reach the frame unconverted.
    #[default]
    Stored,
    /// The target has no non-sRGB view: the shader converts its output to
    /// linear light before the blend. Opaque pixels are exact; blended pixels
    /// are blended in linear light (a recorded known difference).
    LinearFallback,
}

/// The cap counters of decision 0015 §7, applied by `tessera-ui` and the
/// client and handed to the renderer with [`UiPass::cap_counters`] so that
/// they reach [`crate::FrameStats`] in one place. The renderer copies them
/// unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiCapCounters {
    /// Surfaces refused by the per-apartment cap (512).
    pub apartment_cap: u32,
    /// Surfaces refused by the total cap (4,096).
    pub total_cap: u32,
    /// Surfaces refused by the depth cap (8 levels).
    pub depth_cap: u32,
    /// Surfaces or back textures refused by the U5 memory caps.
    pub memory_cap: u32,
    /// Tile surfaces that fell back to CanvasResize (non-power-of-two).
    pub tile_fallbacks: u32,
    /// Canvases cropped to the screen by the size limit.
    pub cropped_canvases: u32,
    /// Entries the client's builder dropped past [`MAX_UI_ENTRIES`].
    pub entry_cap: u32,
}

/// UI statistics of one frame ([`crate::FrameStats::ui`]). Upload, copy and
/// clear counters cover the calls made since the previous `end_frame`; they
/// are kept apart from any asset-upload counter and never count toward the
/// asset budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiStats {
    /// Bytes written by `update_texture_rect` (4 × *w* × *h*; stride
    /// padding not counted).
    pub upload_bytes: u64,
    /// `update_texture_rect` calls that wrote at least one texel.
    pub upload_rects: u32,
    /// `copy_texture_rect` calls that copied at least one texel.
    pub gpu_copies: u32,
    /// Bytes copied by `copy_texture_rect`.
    pub copy_bytes: u64,
    /// `clear_texture_rect` calls that cleared at least one texel.
    pub clears: u32,
    /// Entries drawn (the software cursor not included).
    pub quads: u32,
    /// Distinct textures read by the drawn entries.
    pub surfaces: u32,
    /// Entries refused (see [`UiRefusal`]).
    pub entries_refused: u32,
    /// Entries accepted after sanitising a NaN opacity or threshold or a
    /// non-finite angle, drawn or not: a NaN opacity becomes 0, so such an
    /// entry is counted here but draws nothing (T15 "sanitised … and
    /// counted").
    pub entries_sanitised: u32,
    /// Sum of width × height × 4 over live UI textures (U5).
    pub texture_bytes: u64,
    /// The cap counters the client handed over.
    pub caps: UiCapCounters,
    /// GPU time of the UI passes in nanoseconds, when the device has
    /// timestamp queries (render.md §6.16; currently always `None`, see the
    /// OPEN there).
    pub gpu_ns: Option<u64>,
    /// Whether the software cursor was drawn.
    pub cursor_drawn: bool,
}

/// One UI pass of a frame: an ordered list of composition entries. Obtain it
/// from [`crate::Frame::ui_pass`]. Entries are drawn in the given order, with
/// no depth, sorting or batching that reorders.
#[derive(Debug, Default)]
pub struct UiPass {
    pub(crate) entries: Vec<StoredEntry>,
    pub(crate) rotations: Vec<UiRotation>,
    pub(crate) clips: Vec<UiClip>,
    pub(crate) caps: Option<UiCapCounters>,
    /// Entries refused while recording, per kind.
    pub(crate) refused: RefusalCounts,
    /// Entries accepted by the frame's earlier UI passes (which can no
    /// longer grow), so that [`MAX_UI_ENTRIES`] holds for the whole frame.
    pub(crate) earlier: usize,
}

/// An entry as stored by [`UiPass`] (slices replaced by ranges).
#[derive(Debug, Clone, Copy)]
pub(crate) struct StoredEntry {
    pub texture: TextureId,
    pub source: UiSource,
    pub canvas: [u32; 2],
    pub tex_offset: [i32; 2],
    pub dest: UiRect,
    pub addressing: UiAddressing,
    pub filter: UiFilter,
    pub opacity: f32,
    pub alpha_threshold: f32,
    pub rotations: (u32, u32),
    pub clips: (u32, u32),
}

impl UiPass {
    /// Append an entry. An entry that fails a structural check of
    /// render.md §6.5 (or is past [`MAX_UI_ENTRIES`], counting the entries
    /// of the frame's earlier UI passes) is refused here, never stored, and
    /// counted; texture checks run when the frame ends.
    pub fn entry(&mut self, entry: &UiEntry<'_>) -> &mut Self {
        if self.earlier.saturating_add(self.entries.len()) >= MAX_UI_ENTRIES {
            self.refused.add(UiRefusal::EntryCap);
            return self;
        }
        if let Err(kind) = geometry::check_structure(entry) {
            self.refused.add(kind);
            return self;
        }
        let r0 = u32::try_from(self.rotations.len()).unwrap_or(u32::MAX);
        let c0 = u32::try_from(self.clips.len()).unwrap_or(u32::MAX);
        self.rotations.extend_from_slice(entry.rotations);
        self.clips.extend_from_slice(entry.clips);
        let r1 = u32::try_from(self.rotations.len()).unwrap_or(u32::MAX);
        let c1 = u32::try_from(self.clips.len()).unwrap_or(u32::MAX);
        self.entries.push(StoredEntry {
            texture: entry.texture,
            source: entry.source,
            canvas: entry.canvas,
            tex_offset: entry.tex_offset,
            dest: entry.dest,
            addressing: entry.addressing,
            filter: entry.filter,
            opacity: entry.opacity,
            alpha_threshold: entry.alpha_threshold,
            rotations: (r0, r1),
            clips: (c0, c1),
        });
        self
    }

    /// Hand over the cap counters of decision 0015 §7; `end_frame` copies
    /// them into [`UiStats::caps`] (the last call of the frame wins).
    pub fn cap_counters(&mut self, caps: UiCapCounters) -> &mut Self {
        self.caps = Some(caps);
        self
    }

    /// Entries accepted so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entry was accepted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries refused while recording (structural checks and the cap).
    #[must_use]
    pub fn refused(&self) -> u32 {
        self.refused.total()
    }

    /// The stored entry `i` with its slices restored.
    pub(crate) fn get(&self, i: usize) -> Option<UiEntry<'_>> {
        let e = self.entries.get(i)?;
        let rot = self
            .rotations
            .get(e.rotations.0 as usize..e.rotations.1 as usize)?;
        let clips = self.clips.get(e.clips.0 as usize..e.clips.1 as usize)?;
        Some(UiEntry {
            texture: e.texture,
            source: e.source,
            canvas: e.canvas,
            tex_offset: e.tex_offset,
            dest: e.dest,
            addressing: e.addressing,
            filter: e.filter,
            opacity: e.opacity,
            alpha_threshold: e.alpha_threshold,
            rotations: rot,
            clips,
        })
    }
}
