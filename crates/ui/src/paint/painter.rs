// SPDX-License-Identifier: GPL-3.0-or-later
//! The byte-exact painter (decision 0015 Decision 1.3, 1.7).
//!
//! [`paint_rect`] repaints one damaged rectangle of a surface: it erases the rectangle in
//! the canvas below and in every canvas above, then walks the display list in drawing
//! order. Every operation is a per-pixel function of the destination pixel and of a source
//! pixel addressed from the element's anchor, so painting any partition of a region gives
//! the same bytes as one paint of the whole region.
//!
//! **Nested entries.** A pixel drawn after a nested-entry marker goes to the canvas above of
//! the **last** marker before it in drawing order whose coverage region contains the pixel;
//! a pixel with no such marker goes to the canvas below (Decision 1.7).

use std::collections::BTreeMap;

use super::PaintError;
use super::canvas::{Canvas, TILE};
use super::display::{DisplayList, FillRect, GlyphRun, ImageLayer, OverlayLayer, PaintOp};
use super::geom::Rect;
use super::ops::{
    CompositeOp, base_pixel, over_opaque, overlay_fill_pixel, overlay_image_pixel, text_amul,
    text_opaque, text_pixel,
};

/// Which canvas of a surface: the ordinary canvas, or the canvas above one of its direct
/// nested entries (Decision 1.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    /// The surface's ordinary canvas.
    Below,
    /// The canvas above the nested entry with this id.
    Above(u32),
}

/// The canvases of one surface: the canvas below and the canvases above its direct nested
/// entries, all of the surface's canvas size and sparse.
#[derive(Debug, Clone)]
pub struct CanvasSet {
    below: Canvas,
    above: BTreeMap<u32, Canvas>,
    byte_limit: usize,
    refused: u64,
}

impl CanvasSet {
    /// Transparent-black canvases of `width × height`.
    ///
    /// # Errors
    /// As [`Canvas::new`].
    pub fn new(width: u32, height: u32) -> Result<Self, PaintError> {
        Ok(Self {
            below: Canvas::new(width, height)?,
            above: BTreeMap::new(),
            byte_limit: usize::MAX,
            refused: 0,
        })
    }

    /// Sets the most bytes of tiles the set's canvases may hold together (the set's share of
    /// the U5 canvas budget, decision 0015 §6, §7). [`paint_rect`] does not allocate a tile
    /// past it: the pixels that tile would hold are dropped and counted
    /// ([`CanvasSet::take_refused_tiles`]). Tiles already allocated are kept. The default is
    /// no limit.
    pub fn set_byte_limit(&mut self, limit: usize) {
        self.byte_limit = limit;
    }

    /// The byte limit of [`CanvasSet::set_byte_limit`].
    #[must_use]
    pub fn byte_limit(&self) -> usize {
        self.byte_limit
    }

    /// Tiles refused by the byte limit since the last call; resets the count.
    pub fn take_refused_tiles(&mut self) -> u64 {
        std::mem::take(&mut self.refused)
    }

    /// Releases every tile of every canvas and every canvas above (the surface has nothing
    /// to paint any more, Decision 1.1).
    pub fn clear(&mut self) {
        self.below.clear();
        self.above.clear();
    }

    /// `[width, height]` of every canvas of the set.
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        self.below.size()
    }

    /// The canvas below.
    #[must_use]
    pub fn below(&self) -> &Canvas {
        &self.below
    }

    /// The canvas above nested entry `entry`, if one exists.
    #[must_use]
    pub fn above(&self, entry: u32) -> Option<&Canvas> {
        self.above.get(&entry)
    }

    /// The canvas of `layer`, if it exists.
    #[must_use]
    pub fn layer(&self, layer: Layer) -> Option<&Canvas> {
        match layer {
            Layer::Below => Some(&self.below),
            Layer::Above(e) => self.above.get(&e),
        }
    }

    /// Every existing layer with its canvas, the canvas below first.
    pub fn layers(&self) -> impl Iterator<Item = (Layer, &Canvas)> {
        std::iter::once((Layer::Below, &self.below))
            .chain(self.above.iter().map(|(e, c)| (Layer::Above(*e), c)))
    }

    /// Bytes held by the allocated tiles of every canvas of the set.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.layers().map(|(_, c)| c.allocated_bytes()).sum()
    }

    /// Changes the size of every canvas; all content is released (a size change repaints
    /// the canvas in full, Decision 1.4).
    ///
    /// # Errors
    /// As [`Canvas::new`]; the set is unchanged on error.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), PaintError> {
        if self.size() == [width, height] {
            return Ok(());
        }
        self.below = Canvas::new(width, height)?;
        self.above.clear();
        Ok(())
    }

    /// Keeps the canvases above only for the nested entries of `list`; the others are
    /// released (the split exists only while a nested entry is displayed).
    pub fn retain_nested(&mut self, list: &DisplayList) {
        let keep: std::collections::BTreeSet<u32> = list.nested().map(|(e, _)| e).collect();
        self.above.retain(|e, _| keep.contains(e));
    }

    fn target(&mut self, owner: Option<u32>) -> &mut Canvas {
        use std::collections::btree_map::Entry;
        match owner {
            None => &mut self.below,
            Some(e) => {
                let [w, h] = self.below.size();
                match self.above.entry(e) {
                    Entry::Occupied(o) => o.into_mut(),
                    // The below canvas has this size, so creation cannot fail; if it ever
                    // did, the pixels would go to the canvas below.
                    Entry::Vacant(v) => match Canvas::new(w, h) {
                        Ok(c) => v.insert(c),
                        Err(_) => &mut self.below,
                    },
                }
            }
        }
    }
}

/// Repaints `rect` of a surface from its display list (Decision 1.3): erase, then every
/// operation in drawing order, clipped to `rect` and to the canvas. Tiles are allocated
/// only within the set's byte limit ([`CanvasSet::set_byte_limit`]).
pub fn paint_rect(list: &DisplayList, set: &mut CanvasSet, rect: Rect) {
    let rect = rect.intersect(&set.below.bounds());
    if rect.is_empty() {
        return;
    }
    set.below.erase(&rect);
    for c in set.above.values_mut() {
        c.erase(&rect);
    }
    let limit = set.byte_limit;
    let mut used = set.allocated_bytes();
    let mut refused = 0u64;
    // Which canvas owns each pixel of `rect`, updated at each marker.
    let mut owners = OwnerMap::new(rect);
    let mut pieces: Vec<(Option<u32>, Rect)> = Vec::new();
    for op in list.ops() {
        if let PaintOp::Nested { entry, coverage } = op {
            let c = coverage.intersect(&rect);
            if !c.is_empty() {
                owners.assign(&c, *entry);
            }
            continue;
        }
        let area = op.area().intersect(&rect);
        if area.is_empty() {
            continue;
        }
        pieces.clear();
        owners.split(&area, &mut pieces);
        for &(owner, piece) in &pieces {
            let canvas = set.target(owner);
            let before = canvas.allocated_bytes();
            canvas.limit = before.saturating_add(limit.saturating_sub(used));
            draw_op(op, canvas, &piece);
            canvas.limit = usize::MAX;
            refused += canvas.take_refused();
            used = used - before + canvas.allocated_bytes();
        }
    }
    set.refused += refused;
}

/// The owner of every pixel of a rectangle: the last marker whose coverage region (clipped
/// to the rectangle) contains it, or `None` (the canvas below). Kept as horizontal strips,
/// each a sorted list of x intervals, so that adding a marker costs O(strips × intervals)
/// and splitting an operation costs O(pieces) instead of a scan of every marker.
struct OwnerMap {
    rect: Rect,
    /// Strip boundaries, ascending: strip `i` is rows `ys[i]..ys[i + 1]`.
    ys: Vec<i32>,
    /// Per strip, the intervals as `(start, owner)` sorted by start; interval `j` ends where
    /// interval `j + 1` starts, the last at `rect.x1`. Adjacent intervals differ in owner.
    strips: Vec<Vec<(i32, Option<u32>)>>,
}

impl OwnerMap {
    fn new(rect: Rect) -> Self {
        Self {
            rect,
            ys: vec![rect.y0, rect.y1],
            strips: vec![vec![(rect.x0, None)]],
        }
    }

    /// Splits the strip containing row `y` so that a strip starts at `y` (no-op at an edge).
    fn cut(&mut self, y: i32) {
        if y <= self.rect.y0 || y >= self.rect.y1 {
            return;
        }
        let i = self.ys.partition_point(|&b| b <= y);
        // ys[i - 1] <= y < ys[i]
        if self.ys[i - 1] == y {
            return;
        }
        let copy = self.strips[i - 1].clone();
        self.ys.insert(i, y);
        self.strips.insert(i, copy);
    }

    /// Gives `c` (inside the rectangle, not empty) to marker `entry`.
    fn assign(&mut self, c: &Rect, entry: u32) {
        self.cut(c.y0);
        self.cut(c.y1);
        let s0 = self.ys.partition_point(|&b| b < c.y0);
        let s1 = self.ys.partition_point(|&b| b < c.y1);
        let x_end = self.rect.x1;
        for strip in &mut self.strips[s0..s1] {
            assign_interval(strip, c.x0, c.x1, x_end, Some(entry));
        }
    }

    /// Appends the pieces of `area` (inside the rectangle) with their owners.
    fn split(&self, area: &Rect, out: &mut Vec<(Option<u32>, Rect)>) {
        if self.strips.len() == 1 && self.strips[0].len() == 1 {
            out.push((self.strips[0][0].1, *area));
            return;
        }
        let s0 = self.ys.partition_point(|&b| b <= area.y0) - 1;
        for s in s0..self.strips.len() {
            let (ya, yb) = (self.ys[s].max(area.y0), self.ys[s + 1].min(area.y1));
            if ya >= yb {
                break;
            }
            let iv = &self.strips[s];
            let mut j = iv.partition_point(|&(x, _)| x <= area.x0) - 1;
            while j < iv.len() && iv[j].0 < area.x1 {
                let xa = iv[j].0.max(area.x0);
                let xb = iv.get(j + 1).map_or(self.rect.x1, |n| n.0).min(area.x1);
                if xa < xb {
                    out.push((iv[j].1, Rect::new(xa, ya, xb, yb)));
                }
                j += 1;
            }
        }
    }
}

/// Sets the owner of `[a, b)` to `v` in a strip's interval list covering `[x0, x_end)`.
fn assign_interval(iv: &mut Vec<(i32, Option<u32>)>, a: i32, b: i32, x_end: i32, v: Option<u32>) {
    // Owner at `b`, restored after the new interval.
    let after = if b < x_end {
        let k = iv.partition_point(|&(x, _)| x <= b) - 1;
        Some(iv[k].1)
    } else {
        None
    };
    let lo = iv.partition_point(|&(x, _)| x < a);
    let hi = iv.partition_point(|&(x, _)| x <= b);
    let mut repl: Vec<(i32, Option<u32>)> = Vec::with_capacity(2);
    if lo == 0 || iv[lo - 1].1 != v {
        repl.push((a, v));
    }
    if let Some(o) = after
        && o != v
    {
        repl.push((b, o));
    }
    // Adjacent intervals keep different owners: the predecessor is extended instead of
    // repeated, and the owner at `b` differed from the interval after it already.
    iv.splice(lo..hi, repl);
}

/// Draws one operation into `canvas` over `region` (inside the canvas and the op's area).
fn draw_op(op: &PaintOp, canvas: &mut Canvas, region: &Rect) {
    match op {
        PaintOp::Image(l) => draw_image_layer(l, canvas, region),
        PaintOp::Overlay(l) => draw_overlay_layer(l, canvas, region),
        PaintOp::Fill(f) => draw_fill(f, canvas, region),
        PaintOp::Text(r) => draw_text(r, canvas, region),
        PaintOp::Nested { .. } => {}
    }
}

/// Calls `f(y, x0, x1)` for every tile-local span of `region`.
#[inline]
fn for_spans(region: &Rect, mut f: impl FnMut(i32, i32, i32)) {
    for y in region.y0..region.y1 {
        let mut x = region.x0;
        while x < region.x1 {
            let end = ((x / TILE as i32) + 1) * TILE as i32;
            let end = end.min(region.x1);
            f(y, x, end);
            x = end;
        }
    }
}

/// Tiles an image over `region` from `anchor` and combines each source pixel with `f`.
/// Each span is processed in runs that end at the image's right edge, so the inner loop
/// has no wrap test and can be vectorised.
#[inline]
fn tile_image(
    canvas: &mut Canvas,
    region: &Rect,
    pixels: &[u32],
    iw: u32,
    ih: u32,
    anchor: [i32; 2],
    f: impl Fn(u32, u32) -> u32,
) {
    tile_image_with(
        canvas,
        region,
        pixels,
        iw,
        ih,
        anchor,
        &f,
        None::<&fn(u32, u32) -> u32>,
    );
}

/// [`tile_image`] with an optional second function used on runs whose destination pixels
/// are all opaque (it must give the same result as `f` there).
#[inline]
#[allow(clippy::too_many_arguments)]
fn tile_image_with<F: Fn(u32, u32) -> u32, G: Fn(u32, u32) -> u32>(
    canvas: &mut Canvas,
    region: &Rect,
    pixels: &[u32],
    iw: u32,
    ih: u32,
    anchor: [i32; 2],
    f: &F,
    opaque: Option<&G>,
) {
    let (iw_i, ih_i) = (i64::from(iw), i64::from(ih));
    let iw = iw as usize;
    for_spans(region, |y, x0, x1| {
        let sy = (i64::from(y) - i64::from(anchor[1])).rem_euclid(ih_i) as usize;
        let row = &pixels[sy * iw..(sy + 1) * iw];
        let sx0 = (i64::from(x0) - i64::from(anchor[0])).rem_euclid(iw_i) as usize;
        canvas.modify_span(y, x0, x1, |dst| {
            let mut sx = sx0;
            let mut rest = dst;
            while !rest.is_empty() {
                let n = rest.len().min(iw - sx);
                let (head, tail) = rest.split_at_mut(n);
                let src = &row[sx..sx + n];
                match opaque {
                    Some(g) if head.iter().all(|p| *p >= 0xFF00_0000) => {
                        for (d, s) in head.iter_mut().zip(src) {
                            *d = g(*d, *s);
                        }
                    }
                    _ => {
                        for (d, s) in head.iter_mut().zip(src) {
                            *d = f(*d, *s);
                        }
                    }
                }
                rest = tail;
                sx = 0;
            }
        });
    });
}

/// Applies `f` to every pixel of `region`.
#[inline]
fn map_region(canvas: &mut Canvas, region: &Rect, f: impl Fn(u32) -> u32) {
    for_spans(region, |y, x0, x1| {
        canvas.modify_span(y, x0, x1, |dst| {
            for d in dst {
                *d = f(*d);
            }
        });
    });
}

macro_rules! dispatch_op {
    ($op:expr, $mac:ident) => {
        match $op {
            CompositeOp::Replace => $mac!(CompositeOp::Replace),
            CompositeOp::MixColour => $mac!(CompositeOp::MixColour),
            CompositeOp::Over => $mac!(CompositeOp::Over),
            CompositeOp::Multiply => $mac!(CompositeOp::Multiply),
            CompositeOp::Screen => $mac!(CompositeOp::Screen),
            CompositeOp::Colorize => $mac!(CompositeOp::Colorize),
            CompositeOp::Grayscale => $mac!(CompositeOp::Grayscale),
        }
    };
}

fn draw_image_layer(l: &ImageLayer, canvas: &mut Canvas, region: &Rect) {
    let o = l.opacity;
    if o == 0 || l.op == CompositeOp::Grayscale {
        return;
    }
    let (px, iw, ih) = (l.image.pixels(), l.image.width(), l.image.height());
    if iw == 0 || ih == 0 || px.len() != iw as usize * ih as usize {
        return;
    }
    if l.op == CompositeOp::Over {
        let f = |d, s| base_pixel(CompositeOp::Over, d, s, o);
        let g = |d, s| over_opaque(d, s, o);
        tile_image_with(canvas, region, px, iw, ih, l.anchor, &f, Some(&g));
        return;
    }
    macro_rules! go {
        (CompositeOp::Replace) => {
            tile_image(canvas, region, px, iw, ih, l.anchor, |_, s| s)
        };
        ($k:expr) => {
            if o == 255 {
                tile_image(canvas, region, px, iw, ih, l.anchor, |d, s| {
                    base_pixel($k, d, s, 255)
                })
            } else {
                tile_image(canvas, region, px, iw, ih, l.anchor, |d, s| {
                    base_pixel($k, d, s, o)
                })
            }
        };
    }
    dispatch_op!(l.op, go);
}

fn draw_overlay_layer(l: &OverlayLayer, canvas: &mut Canvas, region: &Rect) {
    let c = l.colour;
    match &l.image {
        None => {
            macro_rules! go {
                (CompositeOp::Replace) => {
                    map_region(canvas, region, |_| c)
                };
                ($k:expr) => {
                    map_region(canvas, region, |d| overlay_fill_pixel($k, d, c))
                };
            }
            dispatch_op!(l.op, go);
        }
        Some((img, anchor)) => {
            // `PaintOp::area` is empty for tags that draw nothing, so the tag is checked
            // there; the pixels' alpha is *I*.A for both *ARGB32* and *alpha-only*.
            let (px, iw, ih) = (img.pixels(), img.width(), img.height());
            if iw == 0 || ih == 0 || px.len() != iw as usize * ih as usize {
                return;
            }
            macro_rules! go {
                ($k:expr) => {
                    tile_image(canvas, region, px, iw, ih, *anchor, |d, i| {
                        overlay_image_pixel($k, d, i, c)
                    })
                };
            }
            dispatch_op!(l.op, go);
        }
    }
}

fn draw_fill(f: &FillRect, canvas: &mut Canvas, region: &Rect) {
    let c = f.colour;
    map_region(canvas, region, |_| c);
}

fn draw_text(run: &GlyphRun, canvas: &mut Canvas, region: &Rect) {
    let cov = &*run.coverage;
    let colour = run.colour;
    let amul = text_amul(colour);
    let (cw, ch) = (i64::from(cov.width()), i64::from(cov.height()));
    for g in &run.glyphs {
        let b = Rect::from_xywh(g.dest[0], g.dest[1], g.size[0], g.size[1]).intersect(region);
        if b.is_empty() {
            continue;
        }
        // Offsets from the glyph box to the coverage map.
        let dx = i64::from(g.src[0]) - i64::from(g.dest[0]);
        let dy = i64::from(g.src[1]) - i64::from(g.dest[1]);
        for_spans(&b, |y, x0, x1| {
            // Pixels that read outside the map have coverage 0 and stay unchanged.
            let sy = i64::from(y) + dy;
            if !(0..ch).contains(&sy) {
                return;
            }
            let (sx0, sx1) = ((i64::from(x0) + dx).max(0), (i64::from(x1) + dx).min(cw));
            if sx0 >= sx1 {
                return;
            }
            let row = &cov.row(sy as u32)[sx0 as usize..sx1 as usize];
            let xa = (sx0 - dx) as i32;
            let xb = (sx1 - dx) as i32;
            canvas.modify_span(y, xa, xb, |dst| {
                if dst.iter().all(|p| *p >= 0xFF00_0000) {
                    for (d, c) in dst.iter_mut().zip(row) {
                        *d = text_opaque(*d, *c, amul, colour);
                    }
                } else {
                    for (d, c) in dst.iter_mut().zip(row) {
                        *d = text_pixel(*d, *c, colour);
                    }
                }
            });
        });
    }
}
