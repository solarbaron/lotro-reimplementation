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
use super::display::{
    CoverFit, DisplayList, FillRect, GlyphRun, ImageLayer, OverlayLayer, PaintOp,
};
use super::geom::Rect;
use super::ops::{
    CompositeOp, base_pixel, over_opaque, overlay_fill_pixel, overlay_image_pixel, text_amul,
    text_opaque, text_pixel,
};
use crate::scaling::image::{ArtFilter, integer_scale};
use crate::scaling::{AxisKind, axis_kind, axis_scale, sharp_bilinear};

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
    /// The surface's **content scale** `c` = (cx, cy) (`ui-scaling.md` §3.4, §6.1): the number
    /// of canvas pixels per the element's own (owner-relative, UI-unit) coordinate on each
    /// axis. `(1.0, 1.0)` at the identity (`Se` = 1, no node scale) and for every surface built
    /// without a UI screen, so painting is then byte-identical (invariant I2). When it is not
    /// `(1, 1)` the painter resamples each image's **native** source by `c` to fill the display
    /// list's already-scaled (physical-pixel) geometry (§6.1, §6.2); solid fills and
    /// plain-colour overlays need no resampling (their scaled rectangle is enough), and glyph
    /// runs are already laid out and derived at the physical size (§6.4), so they are drawn
    /// 1 : 1 at every scale.
    scale: [f64; 2],
    /// The art filter of `ui-scaling.md` §6.3 / §7.6 used to resample images.
    filter: ArtFilter,
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
            scale: [1.0, 1.0],
            filter: ArtFilter::Sharp,
        })
    }

    /// Sets the content scale `c` used to resample image and glyph sources (`ui-scaling.md`
    /// §6.1). `(1.0, 1.0)` (the default) is the crisp identity path. A non-finite or
    /// non-positive component is treated as 1.0.
    pub fn set_scale(&mut self, scale: [f64; 2]) {
        let fix = |v: f64| if v.is_finite() && v > 0.0 { v } else { 1.0 };
        self.scale = [fix(scale[0]), fix(scale[1])];
    }

    /// The content scale `c` of [`CanvasSet::set_scale`].
    #[must_use]
    pub fn scale(&self) -> [f64; 2] {
        self.scale
    }

    /// Sets the art filter images are resampled with (`ui-scaling.md` §6.3, §7.6). At a scale
    /// of exactly 1 every filter is the exact copy.
    pub fn set_art_filter(&mut self, filter: ArtFilter) {
        self.filter = filter;
    }

    /// The art filter of [`CanvasSet::set_art_filter`].
    #[must_use]
    pub fn art_filter(&self) -> ArtFilter {
        self.filter
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
    paint_rect_ops(list, None, set, rect);
}

/// The operations of `list` (indices, ascending) whose area or coverage region meets
/// `region`: the only ones [`paint_rect`] can draw or split by inside it.
pub(crate) fn ops_meeting(list: &DisplayList, region: &Rect) -> Vec<u32> {
    list.areas()
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.intersect(region).is_empty())
        .filter_map(|(i, _)| u32::try_from(i).ok())
        .collect()
}

/// [`paint_rect`] over a subset of the operations: `candidates` (from [`ops_meeting`] for
/// a region holding `rect`) skips operations that cannot reach the rectangle. The result
/// is the same as with every operation, because an operation whose area (or marker whose
/// coverage region) misses the rectangle changes nothing in it.
pub(crate) fn paint_rect_ops(
    list: &DisplayList,
    candidates: Option<&[u32]>,
    set: &mut CanvasSet,
    rect: Rect,
) {
    let rect = rect.intersect(&set.below.bounds());
    if rect.is_empty() {
        return;
    }
    set.below.erase(&rect);
    for c in set.above.values_mut() {
        c.erase(&rect);
    }
    let limit = set.byte_limit;
    let scale = set.scale;
    let filter = set.filter;
    let mut used = set.allocated_bytes();
    let mut refused = 0u64;
    // Which canvas owns each pixel of `rect`, updated at each marker.
    let mut owners = OwnerMap::new(rect);
    let mut pieces: Vec<(Option<u32>, Rect)> = Vec::new();
    let (ops, areas) = (list.ops(), list.areas());
    let mut all = 0..u32::try_from(ops.len()).unwrap_or(u32::MAX);
    let mut picked = candidates.map(|c| c.iter().copied());
    loop {
        let next = match &mut picked {
            Some(p) => p.next(),
            None => all.next(),
        };
        let Some(i) = next else { break };
        let (Some(op), Some(op_area)) = (ops.get(i as usize), areas.get(i as usize)) else {
            continue;
        };
        if let PaintOp::Nested { entry, coverage } = op {
            let c = coverage.intersect(&rect);
            if !c.is_empty() {
                owners.assign(&c, *entry);
            }
            continue;
        }
        let area = op_area.intersect(&rect);
        if area.is_empty() {
            continue;
        }
        pieces.clear();
        owners.split(&area, &mut pieces);
        for &(owner, piece) in &pieces {
            let canvas = set.target(owner);
            let before = canvas.allocated_bytes();
            canvas.limit = before.saturating_add(limit.saturating_sub(used));
            draw_op(op, canvas, &piece, scale, filter, list.cover_of(i));
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
///
/// `scale` is the surface's content scale `c` (`ui-scaling.md` §6.1): at `(1.0, 1.0)` every
/// op takes its byte-exact native path (invariant I2); otherwise image sources are resampled by
/// `c` with `filter` (§6.3) to fill the already-scaled geometry the display list carries. Solid
/// fills and plain-colour overlays are scale-independent (their geometry is already scaled), and
/// glyph runs carry physical-size coverage (§6.4), so they are drawn 1 : 1 at every scale. An
/// image layer with a `cover` fit (§5.5) is resampled by the fit's own scale at any `scale`.
fn draw_op(
    op: &PaintOp,
    canvas: &mut Canvas,
    region: &Rect,
    scale: [f64; 2],
    filter: ArtFilter,
    cover: Option<CoverFit>,
) {
    match op {
        PaintOp::Image(l) => draw_image_layer(l, canvas, region, scale, filter, cover),
        PaintOp::Overlay(l) => draw_overlay_layer(l, canvas, region, scale, filter, cover),
        PaintOp::Fill(f) => draw_fill(f, canvas, region),
        PaintOp::Text(r) => draw_text(r, canvas, region),
        PaintOp::Nested { .. } => {}
    }
}

/// Whether a content scale is the crisp identity (the byte-exact native paths run).
#[inline]
fn is_identity_scale(scale: [f64; 2]) -> bool {
    scale == [1.0, 1.0]
}

/// The source texels and their weights for one destination coordinate of a resampled image
/// axis (`ui-scaling.md` §6.3). Two taps for the default "sharp" filter above 1 (sharp
/// bilinear, which also gives the exact copy at scale 1 and the nearest blocks at an integer
/// scale), or the covered source texels for the area filter below 1.
type AxisTaps = Vec<(usize, f64)>;

/// Reconstructs an element's layout extent in UI units from its physical box extent and the
/// content scale `c` (the display list carries only the already-scaled physical boxes). Used
/// only to pick the per-axis image kind of `ui-scaling.md` §6.3 (fitted / tiled / clipped);
/// a one-pixel rounding difference only moves the fitted/tiled/clipped boundary, where the
/// three kinds agree. A non-finite or non-positive `c` falls back to the physical extent.
#[inline]
fn extent_from_physical(physical: i32, c: f64) -> i32 {
    if c.is_finite() && c > 0.0 {
        (f64::from(physical) / c).round() as i32
    } else {
        physical
    }
}

/// The per-axis source taps for the destination coordinates `lo..hi` of a resampled image
/// axis (`ui-scaling.md` §6.3), with the art filter `filter` (the columns of the §6.3 table):
///
/// - scale exactly 1: one exact tap per pixel with every filter (the 1 : 1 copy);
/// - an integer scale ≥ 2 (within 10⁻⁶): `sharp` and `nearest` take the nearest texel (each
///   texel becomes an *s* × *s* block), `smooth` is bilinear;
/// - above 1, not an integer: `sharp` is [`sharp_bilinear`] (each texel boundary turns into at
///   most one blended pixel), `smooth` bilinear (weight *φ*), `nearest` the nearest texel;
/// - below 1: `sharp` and `smooth` use the area filter — each destination pixel averages the
///   source texels its footprint `[d ÷ s, (d + 1) ÷ s)` covers, weighted by the covered fraction
///   and normalised, as [`crate::scaling::area_weights`] — and `nearest` the nearest texel.
///
/// `origin` is the canvas coordinate of source texel 0 (the element's physical edge plus the
/// run-time image offset). A **tiled** axis wraps (`rem_euclid`); a **fitted** or **clipped**
/// axis clamps to `0 ..= native − 1`. The source texel count of a destination pixel is capped
/// at `native`, so an extreme scale cannot allocate without bound. `native` is ≥ 1 (the caller
/// has checked the image is non-empty). A non-finite or non-positive scale takes the texel at
/// the destination offset.
fn axis_taps(
    lo: i32,
    hi: i32,
    origin: i32,
    kind: AxisKind,
    s: f64,
    native: u32,
    filter: ArtFilter,
) -> Vec<AxisTaps> {
    let nat = i64::from(native);
    let addr = |i: i64| -> usize {
        match kind {
            AxisKind::Tiled => i.rem_euclid(nat) as usize,
            AxisKind::Fitted | AxisKind::Clipped => i.clamp(0, nat - 1) as usize,
        }
    };
    let len = (hi - lo).max(0) as usize;
    let mut out = Vec::with_capacity(len);
    let degenerate = !s.is_finite() || s <= 0.0;
    let nearest = !degenerate
        && (filter == ArtFilter::Nearest
            || (filter == ArtFilter::Sharp && integer_scale(s).is_some()));
    for d in lo..hi {
        let off = i64::from(d) - i64::from(origin);
        if degenerate {
            out.push(vec![(addr(off), 1.0)]);
            continue;
        }
        if nearest {
            let i = ((off as f64 + 0.5) / s).floor() as i64;
            out.push(vec![(addr(i), 1.0)]);
            continue;
        }
        if s >= 1.0 {
            let (i, w0, w1) = if filter == ArtFilter::Smooth {
                let t = (off as f64 + 0.5) / s - 0.5;
                let i = t.floor();
                let phi = t - i;
                (i as i64, 1.0 - phi, phi)
            } else {
                let (i, w0, w1) = sharp_bilinear(i32::try_from(off).unwrap_or(0), s);
                (i64::from(i), w0, w1)
            };
            let mut taps = Vec::with_capacity(2);
            if w0 > 0.0 {
                taps.push((addr(i), w0));
            }
            if w1 > 0.0 {
                taps.push((addr(i + 1), w1));
            }
            if taps.is_empty() {
                taps.push((addr(i), 1.0));
            }
            out.push(taps);
        } else {
            let dr = off as f64;
            let flo = dr / s;
            let fhi = (dr + 1.0) / s;
            let first = flo.floor() as i64;
            // Cover at most `native` source texels: a tiny scale would otherwise span an
            // unbounded count (the footprint wraps or clamps over the whole image anyway).
            let count = (fhi.ceil() as i64 - first).clamp(1, nat);
            let mut taps = Vec::with_capacity(count as usize);
            let mut wsum = 0.0;
            for k in 0..count {
                let i = first + k;
                let overlap = (fhi.min((i + 1) as f64) - flo.max(i as f64)).max(0.0);
                if overlap > 0.0 {
                    taps.push((addr(i), overlap));
                    wsum += overlap;
                }
            }
            if taps.is_empty() || wsum <= 0.0 {
                taps.clear();
                taps.push((addr(first), 1.0));
            } else {
                for t in &mut taps {
                    t.1 /= wsum;
                }
            }
            out.push(taps);
        }
    }
    out
}

/// One resampled source pixel from the separable per-axis taps (`ui-scaling.md` §6.3): the
/// filter is applied on **premultiplied** texels (so transparent neighbours never darken an
/// edge) and the result is converted back to straight alpha. A single tap on each axis (an
/// exact copy or a nearest block) returns the native texel unchanged, so the byte-exact
/// native output is kept wherever the scale is 1 on both axes (invariant I2).
#[inline]
fn resample_pixel(px: &[u32], iwu: usize, xt: &[(usize, f64)], yt: &[(usize, f64)]) -> u32 {
    if let ([(sx, _)], [(sy, _)]) = (xt, yt) {
        return px[sy * iwu + sx];
    }
    let (mut ar, mut ag, mut ab, mut aa) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &(sy, wy) in yt {
        let row = sy * iwu;
        for &(sx, wx) in xt {
            let p = px[row + sx];
            let w = wx * wy;
            let a = f64::from((p >> 24) & 0xFF);
            let af = a / 255.0;
            ar += f64::from((p >> 16) & 0xFF) * af * w;
            ag += f64::from((p >> 8) & 0xFF) * af * w;
            ab += f64::from(p & 0xFF) * af * w;
            aa += a * w;
        }
    }
    if aa <= 0.0 {
        return 0;
    }
    let inv = 255.0 / aa;
    let r = (ar * inv).round().clamp(0.0, 255.0) as u32;
    let g = (ag * inv).round().clamp(0.0, 255.0) as u32;
    let b = (ab * inv).round().clamp(0.0, 255.0) as u32;
    let a = aa.round().clamp(0.0, 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
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

fn draw_image_layer(
    l: &ImageLayer,
    canvas: &mut Canvas,
    region: &Rect,
    scale: [f64; 2],
    filter: ArtFilter,
    cover: Option<CoverFit>,
) {
    let o = l.opacity;
    if o == 0 || l.op == CompositeOp::Grayscale {
        return;
    }
    let (px, iw, ih) = (l.image.pixels(), l.image.width(), l.image.height());
    if iw == 0 || ih == 0 || px.len() != iw as usize * ih as usize {
        return;
    }
    if let Some(fit) = cover {
        let taps = cover_taps(region, l.anchor, fit, [iw, ih], filter);
        resample_into(canvas, region, px, iw, &taps, |d, s| match l.op {
            CompositeOp::Replace => s,
            op => base_pixel(op, d, s, o),
        });
        return;
    }
    if !is_identity_scale(scale) {
        draw_image_scaled(l, canvas, region, px, iw, ih, scale, filter);
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

/// The per-axis taps of a resampled image layer over `region` (`ui-scaling.md` §6.3): each
/// axis's kind (fitted, tiled or clipped) from the native size against the element's layout
/// extent, its scale from [`axis_scale`], its samples from [`axis_taps`] with `filter`.
fn layer_taps(
    rect: &Rect,
    region: &Rect,
    anchor: [i32; 2],
    native: [u32; 2],
    scale: [f64; 2],
    filter: ArtFilter,
) -> [Vec<AxisTaps>; 2] {
    let [iw, ih] = native;
    let (pw, ph) = (rect.width() as i32, rect.height() as i32);
    let kind_x = axis_kind(iw as i32, extent_from_physical(pw, scale[0]));
    let kind_y = axis_kind(ih as i32, extent_from_physical(ph, scale[1]));
    let sx = axis_scale(kind_x, pw, iw as i32, scale[0]);
    let sy = axis_scale(kind_y, ph, ih as i32, scale[1]);
    [
        axis_taps(region.x0, region.x1, anchor[0], kind_x, sx, iw, filter),
        axis_taps(region.y0, region.y1, anchor[1], kind_y, sy, ih, filter),
    ]
}

/// The per-axis taps of a backdrop **cover fit** over `region` (`ui-scaling.md` §5.5): one
/// copy from `anchor` at the fit's scale, clamped addressing, resampled with `filter` (§6.3).
fn cover_taps(
    region: &Rect,
    anchor: [i32; 2],
    fit: CoverFit,
    native: [u32; 2],
    filter: ArtFilter,
) -> [Vec<AxisTaps>; 2] {
    [
        axis_taps(
            region.x0,
            region.x1,
            anchor[0],
            AxisKind::Clipped,
            fit.scale[0],
            native[0],
            filter,
        ),
        axis_taps(
            region.y0,
            region.y1,
            anchor[1],
            AxisKind::Clipped,
            fit.scale[1],
            native[1],
            filter,
        ),
    ]
}

/// Composites resampled source pixels over `region` with `f(destination, source)`: the one
/// per-pixel path of every resampled image (`ui-scaling.md` §6.3), for every draw mode.
fn resample_into(
    canvas: &mut Canvas,
    region: &Rect,
    px: &[u32],
    iw: u32,
    taps: &[Vec<AxisTaps>; 2],
    f: impl Fn(u32, u32) -> u32,
) {
    let [x_taps, y_taps] = taps;
    let iwu = iw as usize;
    for_spans(region, |y, x0, x1| {
        let Some(yt) = y_taps.get((y - region.y0) as usize) else {
            return;
        };
        canvas.modify_span(y, x0, x1, |dst| {
            for (i, d) in dst.iter_mut().enumerate() {
                let Some(xt) = x_taps.get((x0 + i as i32 - region.x0) as usize) else {
                    continue;
                };
                let s = resample_pixel(px, iwu, xt, yt);
                *d = f(*d, s);
            }
        });
    });
}

/// The resampled base-image path (`ui-scaling.md` §6.3): each axis is handled on its own
/// ([`layer_taps`]) with the art filter. One per-pixel path serves every draw mode. At scale 1
/// on both axes every pixel is an exact native copy (invariant I2).
#[allow(clippy::too_many_arguments)]
fn draw_image_scaled(
    l: &ImageLayer,
    canvas: &mut Canvas,
    region: &Rect,
    px: &[u32],
    iw: u32,
    ih: u32,
    scale: [f64; 2],
    filter: ArtFilter,
) {
    let o = l.opacity;
    let op = l.op;
    let taps = layer_taps(&l.rect, region, l.anchor, [iw, ih], scale, filter);
    resample_into(canvas, region, px, iw, &taps, |d, s| match op {
        CompositeOp::Replace => s,
        _ => base_pixel(op, d, s, o),
    });
}

fn draw_overlay_layer(
    l: &OverlayLayer,
    canvas: &mut Canvas,
    region: &Rect,
    scale: [f64; 2],
    filter: ArtFilter,
    cover: Option<CoverFit>,
) {
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
            if cover.is_some() || !is_identity_scale(scale) {
                // The overlay uses only the image's alpha (§9.6.3), so the per-axis crisp
                // resample of §6.3 (or the cover fit of §5.5) feeds that alpha to the colour.
                let op = l.op;
                let taps = match cover {
                    Some(fit) => cover_taps(region, *anchor, fit, [iw, ih], filter),
                    None => layer_taps(&l.rect, region, *anchor, [iw, ih], scale, filter),
                };
                resample_into(canvas, region, px, iw, &taps, |d, s| {
                    overlay_image_pixel(op, d, s, c)
                });
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

/// Draws a glyph run 1 : 1 (`fonts.md` §8.4): every box copies its coverage cell unchanged.
/// At a content scale other than 1 the runs come from the derived-glyph path (`ui-scaling.md`
/// §6.4), already laid out and rasterised at the physical size, so no resampling is involved
/// at any scale (the soft bilinear resample of native coverage is retired).
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

#[cfg(test)]
mod filter_tests {
    //! The art-filter columns of `ui-scaling.md` §6.3 (sharp, smooth, nearest) at a scale of 1,
    //! an integer scale, a non-integer scale above 1 and a scale below 1, and the backdrop
    //! cover fit of §5.5.
    use std::sync::Arc;

    use tessera_material::ui_image::{PixelLayout, UiImage};

    use super::*;

    const FILTERS: [ArtFilter; 3] = [ArtFilter::Sharp, ArtFilter::Smooth, ArtFilter::Nearest];

    /// An opaque image whose texels all differ: red = 40 × x + 1, green = 40 × y + 1.
    fn image(w: u32, h: u32) -> Arc<UiImage> {
        let px = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| 0xFF00_0000 | ((40 * x + 1) & 0xFF) << 16 | ((40 * y + 1) & 0xFF) << 8)
            .collect();
        Arc::new(UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).unwrap())
    }

    /// Paints `img` (Replace) over a `pw × ph` element at the surface scale `scale` with
    /// `filter`, and returns the canvas pixels row by row.
    fn paint(img: &Arc<UiImage>, pw: u32, ph: u32, scale: [f64; 2], filter: ArtFilter) -> Vec<u32> {
        let mut set = CanvasSet::new(pw, ph).unwrap();
        set.set_scale(scale);
        set.set_art_filter(filter);
        let rect = Rect::of_size(pw, ph);
        let mut list = DisplayList::new();
        list.push(PaintOp::Image(ImageLayer {
            rect,
            clip: rect,
            anchor: [0, 0],
            image: Arc::clone(img),
            op: CompositeOp::Replace,
            opacity: 255,
        }))
        .unwrap();
        paint_rect(&list, &mut set, rect);
        let mut out = Vec::new();
        for y in 0..ph as i32 {
            for x in 0..pw as i32 {
                out.push(set.below().pixel(x, y));
            }
        }
        out
    }

    fn texel(img: &UiImage, x: u32, y: u32) -> u32 {
        img.pixels()[(y * img.width() + x) as usize]
    }

    fn red(p: u32) -> u32 {
        (p >> 16) & 0xFF
    }

    /// Scale 1 on both axes: every filter is the exact 1 : 1 copy (byte-identical, I2).
    #[test]
    fn scale_one_is_exact_for_every_filter() {
        let img = image(4, 3);
        let want: Vec<u32> = (0..3)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .map(|(x, y)| texel(&img, x, y))
            .collect();
        for f in FILTERS {
            assert_eq!(paint(&img, 4, 3, [1.0, 1.0], f), want, "{f:?}");
            // The taps at exactly 1 (also off the identity fast path) are one exact texel.
            for taps in axis_taps(0, 8, 0, AxisKind::Fitted, 1.0, 8, f) {
                assert_eq!(taps.len(), 1, "{f:?}");
            }
        }
    }

    /// An integer scale (2): sharp and nearest make 2 × 2 blocks, smooth blends.
    #[test]
    fn integer_scale() {
        let img = image(4, 4);
        for f in FILTERS {
            let out = paint(&img, 8, 8, [2.0, 2.0], f);
            let row: Vec<u32> = out[..8].iter().map(|p| red(*p)).collect();
            match f {
                ArtFilter::Sharp | ArtFilter::Nearest => {
                    assert_eq!(row, [1, 1, 41, 41, 81, 81, 121, 121], "{f:?}");
                }
                ArtFilter::Smooth => {
                    // Pixel 1's centre is a quarter texel past texel 0's: 0.75·1 + 0.25·41.
                    assert_eq!(row[1], 11, "{f:?}");
                    assert!(row.iter().any(|r| (r - 1) % 40 != 0), "{f:?} blends");
                }
            }
        }
    }

    /// A non-integer scale above 1 (1.5): sharp blends at most one pixel per texel boundary,
    /// smooth blends more, nearest never blends.
    #[test]
    fn non_integer_scale() {
        let img = image(6, 1);
        let exact = |r: u32| (r + 39).is_multiple_of(40);
        let mut blended = Vec::new();
        for f in FILTERS {
            let out = paint(&img, 9, 1, [1.5, 1.5], f);
            let row: Vec<u32> = out.iter().map(|p| red(*p)).collect();
            let n = row.iter().filter(|r| !exact(**r)).count();
            blended.push(n);
            if f == ArtFilter::Nearest {
                assert_eq!(n, 0, "nearest never blends: {row:?}");
            }
            if f == ArtFilter::Sharp {
                // Five texel boundaries in six texels: at most one blended pixel each.
                assert!(n <= 5, "{f:?}: {row:?}");
            }
        }
        assert!(
            blended[1] >= blended[0],
            "smooth blends at least as much as sharp"
        );
    }

    /// A scale below 1 (0.5): sharp and smooth average the footprint (area), nearest picks.
    #[test]
    fn scale_below_one() {
        let img = image(8, 8);
        for f in FILTERS {
            let out = paint(&img, 4, 4, [0.5, 0.5], f);
            let r0 = red(out[0]);
            match f {
                ArtFilter::Sharp | ArtFilter::Smooth => {
                    // Texels 0 and 1 (red 1 and 41) averaged.
                    assert_eq!(r0, 21, "{f:?}");
                }
                ArtFilter::Nearest => assert_eq!(r0, 41, "{f:?}: floor((0 + ½) ÷ ½) = 1"),
            }
        }
    }

    /// `ui-scaling.md` §5.5: a 1024 × 768 backdrop made for its Stretch element, which the
    /// layout resized to 1365 × 768, is drawn once as a cover fit (scaled by 1365 ÷ 1024,
    /// centred vertically, clipped), never tiled.
    #[test]
    fn backdrop_cover_is_one_scaled_copy() {
        let (iw, ih) = (1024u32, 768u32);
        let px = (0..ih)
            .flat_map(|y| (0..iw).map(move |x| (x, y)))
            .map(|(x, y)| 0xFF00_0000 | ((x * 255 / 1023) << 16) | ((y * 255 / 767) << 8))
            .collect();
        let img = Arc::new(UiImage::new(iw, ih, iw, ih, PixelLayout::A8R8G8B8, px).unwrap());
        let (pw, ph) = (1365u32, 768u32);
        let f =
            crate::scaling::image::backdrop_cover([1024, 768], [1024, 768], [1365, 768]).unwrap();
        let off_y = ((f64::from(ph) - 768.0 * f) / 2.0).round() as i32;
        assert_eq!(off_y, -128);
        let rect = Rect::of_size(pw, ph);
        let mut list = DisplayList::new();
        list.push_cover(
            PaintOp::Image(ImageLayer {
                rect,
                clip: rect,
                anchor: [0, off_y],
                image: Arc::clone(&img),
                op: CompositeOp::Replace,
                opacity: 255,
            }),
            CoverFit { scale: [f, f] },
        )
        .unwrap();
        assert!(list.cover_of(0).is_some());
        let mut set = CanvasSet::new(pw, ph).unwrap();
        paint_rect(&list, &mut set, rect);
        let at = |x: i32, y: i32| set.below().pixel(x, y);
        let g = |p: u32| (p >> 8) & 0xFF;
        // The right end shows the image's last column (a tiled copy would restart at 0).
        assert!(red(at(1364, 384)) >= 254, "{:#x}", at(1364, 384));
        assert!(red(at(1100, 384)) > 200, "not tiled: {:#x}", at(1100, 384));
        // Centred vertically: the top row is texel row ≈ 96, the bottom one ≈ 671.
        assert!(
            (i64::from(g(at(10, 0))) - 32).abs() <= 1,
            "{:#x}",
            at(10, 0)
        );
        assert!(
            (i64::from(g(at(10, 767))) - 223).abs() <= 1,
            "{:#x}",
            at(10, 767)
        );
        // Without the fit the same layer tiles at native size (UL §9.6).
        let mut plain = DisplayList::new();
        plain
            .push(PaintOp::Image(ImageLayer {
                rect,
                clip: rect,
                anchor: [0, 0],
                image: img,
                op: CompositeOp::Replace,
                opacity: 255,
            }))
            .unwrap();
        let mut set2 = CanvasSet::new(pw, ph).unwrap();
        paint_rect(&plain, &mut set2, rect);
        assert_eq!(red(set2.below().pixel(1100, 384)), 76 * 255 / 1023);
    }
}
