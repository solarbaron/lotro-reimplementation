// SPDX-License-Identifier: GPL-3.0-or-later
//! Paint primitives and the display list of one surface (decision 0015 Decision 1.3).
//!
//! The element tree turns the visible elements of a surface into a [`DisplayList`]: the
//! operations of `ui-layout.md` §9.3 in drawing order, already clipped to each element's
//! box and its ancestors' boxes, in **canvas coordinates** (the layout box of the surface's
//! element, less any canvas origin offset of §7). Text arrives from `ui::text` as glyph
//! coverage runs. A nested entry (Decision 1.7) is a marker at its place in the drawing
//! order. A display list plus the `Arc`ed images it names is an immutable **paint
//! snapshot**: painting is a pure function of it.

use std::sync::Arc;

use tessera_material::ui_image::{SourceTag, UiImage};

use super::PaintError;
use super::geom::Rect;
use super::ops::CompositeOp;

/// Most operations in one display list (Tessera cap, §7 hostile input).
pub const MAX_OPS: usize = 1 << 16;

/// Most nested-entry markers in one display list (Tessera cap: the 512 surfaces a plugin
/// apartment may own, §7).
pub const MAX_NESTED: usize = 512;

/// Most glyph boxes in one glyph run.
pub const MAX_GLYPHS_PER_RUN: usize = 1 << 16;

/// Largest side of a coverage map (a glyph atlas page or a rasterised fallback glyph set).
pub const MAX_COVERAGE_DIM: u32 = 8_192;

/// An image layer drawn at native size, tiled from `anchor` over `rect` (`ui-layout.md`
/// §9.6): the **base layer** of an element.
#[derive(Debug, Clone)]
pub struct ImageLayer {
    /// The element's box in canvas coordinates: the area the layer covers.
    pub rect: Rect,
    /// The element's clip (its ancestors' boxes intersected), in canvas coordinates.
    pub clip: Rect,
    /// Canvas position of image pixel (0, 0): the element's top-left corner plus the
    /// run-time image offset. Tiling is anchored here, never at a damage rectangle.
    pub anchor: [i32; 2],
    /// The decoded image (from `tessera-material::ui_image`).
    pub image: Arc<UiImage>,
    /// The operation of the item's draw mode.
    pub op: CompositeOp,
    /// The opacity byte *o* ([`super::opacity_byte`] of `UICore_Element_alphablendmod`).
    pub opacity: u8,
}

/// The **overlay layer** of an element (Coloured image items, `ui-layout.md` §9.6.3).
#[derive(Debug, Clone)]
pub struct OverlayLayer {
    /// The element's box in canvas coordinates.
    pub rect: Rect,
    /// The element's clip, in canvas coordinates.
    pub clip: Rect,
    /// The item's colour, `0xAARRGGBB`.
    pub colour: u32,
    /// The operation of the item's draw mode.
    pub op: CompositeOp,
    /// The item's image with its anchor, or `None` for a plain colour (item DID 0). An
    /// image draws only when its source tag is *ARGB32* or *alpha-only* (decision 0015
    /// §4.2); for any other tag the overlay draws nothing.
    pub image: Option<(Arc<UiImage>, [i32; 2])>,
}

/// A rectangle overwritten with one colour, alpha included, with no blending: underlines
/// (`fonts.md` §8.5), carets and selection highlights (decision 0015 §3).
#[derive(Debug, Clone, Copy)]
pub struct FillRect {
    /// The rectangle, in canvas coordinates.
    pub rect: Rect,
    /// The clip, in canvas coordinates.
    pub clip: Rect,
    /// The colour, `0xAARRGGBB`.
    pub colour: u32,
}

/// An 8-bit coverage map (A8): a glyph atlas or a set of rasterised glyphs.
#[derive(Clone, PartialEq, Eq)]
pub struct CoverageMap {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for CoverageMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoverageMap")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl CoverageMap {
    /// A coverage map from row-major bytes, top row first.
    ///
    /// # Errors
    /// [`PaintError::CoverageSize`] unless `data.len()` is `width × height` and both sides
    /// are 1..=[`MAX_COVERAGE_DIM`].
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, PaintError> {
        let ok = (1..=MAX_COVERAGE_DIM).contains(&width)
            && (1..=MAX_COVERAGE_DIM).contains(&height)
            && data.len() as u64 == u64::from(width) * u64::from(height);
        if !ok {
            return Err(PaintError::CoverageSize { width, height });
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Width.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The coverage at (`x`, `y`); 0 outside the map (an outline box of `fonts.md` §8.2
    /// that reaches past the atlas edge reads no coverage there).
    #[inline]
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            0
        } else {
            self.data[y as usize * self.width as usize + x as usize]
        }
    }

    /// Row `y` (inside the map).
    pub(crate) fn row(&self, y: u32) -> &[u8] {
        let w = self.width as usize;
        &self.data[y as usize * w..(y as usize + 1) * w]
    }

    /// The bytes, row major.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// One glyph box of a run: `w × h` pixels at canvas position `dest`, read from the coverage
/// map at `src` (`fonts.md` §6.2, §8.2, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphBox {
    /// Canvas position of the box's top-left pixel.
    pub dest: [i32; 2],
    /// Box size.
    pub size: [u32; 2],
    /// Coverage-map position of the box's top-left pixel.
    pub src: [i32; 2],
}

/// One text pass over a set of glyphs in one colour (`fonts.md` §8.1): the outline pass,
/// or a run of the fill pass. Boxes are composited in their order with the rule of
/// `fonts.md` §8.4.
#[derive(Debug, Clone)]
pub struct GlyphRun {
    /// The text element's clip, in canvas coordinates.
    pub clip: Rect,
    /// The pass colour, `0xAARRGGBB`.
    pub colour: u32,
    /// The coverage source.
    pub coverage: Arc<CoverageMap>,
    /// Glyph boxes in drawing order.
    pub glyphs: Vec<GlyphBox>,
}

impl GlyphRun {
    /// The bounding box of the run's glyph boxes, clipped by `clip`.
    #[must_use]
    pub fn area(&self) -> Rect {
        let mut b = Rect::EMPTY;
        for g in &self.glyphs {
            b = b.union(&Rect::from_xywh(g.dest[0], g.dest[1], g.size[0], g.size[1]));
        }
        b.intersect(&self.clip)
    }
}

/// One paint operation.
#[derive(Debug, Clone)]
pub enum PaintOp {
    /// A base image layer.
    Image(ImageLayer),
    /// An overlay layer.
    Overlay(OverlayLayer),
    /// An opaque overwrite of a rectangle.
    Fill(FillRect),
    /// A text pass.
    Text(Arc<GlyphRun>),
    /// A direct nested entry of this surface at its place in drawing order (Decision 1.7):
    /// every pixel drawn after it inside `coverage` (and not inside the coverage of a later
    /// nested entry) goes to the entry's canvas above.
    Nested {
        /// The nested entry's id (its surface or external-content id).
        entry: u32,
        /// Its coverage region in this surface's canvas coordinates (see
        /// [`super::coverage_region`]).
        coverage: Rect,
    },
}

impl PaintOp {
    /// The canvas area the operation may change (empty for a marker or a layer that draws
    /// nothing).
    #[must_use]
    pub fn area(&self) -> Rect {
        match self {
            Self::Image(l) => {
                if l.opacity == 0 || l.op == CompositeOp::Grayscale {
                    Rect::EMPTY
                } else {
                    l.rect.intersect(&l.clip)
                }
            }
            Self::Overlay(l) => {
                let draws = match &l.image {
                    None => true,
                    Some((img, _)) => {
                        l.colour >> 24 != 0
                            && l.op != CompositeOp::Grayscale
                            && matches!(img.tag(), SourceTag::Argb32 | SourceTag::AlphaOnly)
                    }
                };
                if draws {
                    l.rect.intersect(&l.clip)
                } else {
                    Rect::EMPTY
                }
            }
            Self::Fill(f) => f.rect.intersect(&f.clip),
            Self::Text(r) => r.area(),
            Self::Nested { .. } => Rect::EMPTY,
        }
    }
}

/// The paint operations of one surface in drawing order.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    ops: Vec<PaintOp>,
    nested: usize,
}

impl DisplayList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an operation.
    ///
    /// # Errors
    /// [`PaintError::TooManyOps`] past [`MAX_OPS`] operations or [`MAX_NESTED`] markers,
    /// [`PaintError::TooManyGlyphs`] for a run of more than [`MAX_GLYPHS_PER_RUN`] boxes.
    pub fn push(&mut self, op: PaintOp) -> Result<(), PaintError> {
        if self.ops.len() >= MAX_OPS {
            return Err(PaintError::TooManyOps);
        }
        match &op {
            PaintOp::Nested { .. } => {
                if self.nested >= MAX_NESTED {
                    return Err(PaintError::TooManyOps);
                }
                self.nested += 1;
            }
            PaintOp::Text(r) if r.glyphs.len() > MAX_GLYPHS_PER_RUN => {
                return Err(PaintError::TooManyGlyphs);
            }
            _ => {}
        }
        self.ops.push(op);
        Ok(())
    }

    /// The operations in drawing order.
    #[must_use]
    pub fn ops(&self) -> &[PaintOp] {
        &self.ops
    }

    /// The nested-entry markers in drawing order, as `(entry, coverage)`.
    pub fn nested(&self) -> impl Iterator<Item = (u32, Rect)> + '_ {
        self.ops.iter().filter_map(|o| match o {
            PaintOp::Nested { entry, coverage } => Some((*entry, *coverage)),
            _ => None,
        })
    }

    /// Number of operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}
