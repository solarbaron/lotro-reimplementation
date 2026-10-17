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
    /// `(1, 1)` the painter resamples each image's and glyph's **native** source by `c` to fill
    /// the display list's already-scaled (physical-pixel) geometry (§6.1, §6.2); solid fills and
    /// plain-colour overlays need no resampling (their scaled rectangle is enough).
    scale: [f64; 2],
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
            draw_op(op, canvas, &piece, scale);
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
/// op takes its byte-exact native path (invariant I2); otherwise image and glyph sources are
/// resampled by `c` to fill the already-scaled geometry the display list carries. Solid fills
/// and plain-colour overlays are scale-independent (their geometry is already scaled).
fn draw_op(op: &PaintOp, canvas: &mut Canvas, region: &Rect, scale: [f64; 2]) {
    match op {
        PaintOp::Image(l) => draw_image_layer(l, canvas, region, scale),
        PaintOp::Overlay(l) => draw_overlay_layer(l, canvas, region, scale),
        PaintOp::Fill(f) => draw_fill(f, canvas, region),
        PaintOp::Text(r) => draw_text(r, canvas, region, scale),
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
/// axis (`ui-scaling.md` §6.3), with the default "sharp" filter:
///
/// - scale ≥ 1 (and the degenerate non-finite case): [`sharp_bilinear`] per destination pixel
///   (two taps, which collapse to one exact tap at an integer or unit scale);
/// - scale < 1: the area filter — each destination pixel averages the source texels its
///   footprint `[d ÷ s, (d + 1) ÷ s)` covers, weighted by the covered fraction and normalised,
///   as [`crate::scaling::area_weights`], but carrying the per-axis addressing below.
///
/// `origin` is the canvas coordinate of source texel 0 (the element's physical edge plus the
/// run-time image offset). A **tiled** axis wraps (`rem_euclid`); a **fitted** or **clipped**
/// axis clamps to `0 ..= native − 1`. The source texel count of a destination pixel is capped
/// at `native`, so an extreme scale cannot allocate without bound. `native` is ≥ 1 (the caller
/// has checked the image is non-empty).
fn axis_taps(lo: i32, hi: i32, origin: i32, kind: AxisKind, s: f64, native: u32) -> Vec<AxisTaps> {
    let nat = i64::from(native);
    let addr = |i: i64| -> usize {
        match kind {
            AxisKind::Tiled => i.rem_euclid(nat) as usize,
            AxisKind::Fitted | AxisKind::Clipped => i.clamp(0, nat - 1) as usize,
        }
    };
    let len = (hi - lo).max(0) as usize;
    let mut out = Vec::with_capacity(len);
    if !s.is_finite() || s >= 1.0 {
        for d in lo..hi {
            let (i, w0, w1) = sharp_bilinear(d - origin, s);
            let i = i64::from(i);
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
        }
    } else {
        for d in lo..hi {
            let dr = f64::from(d - origin);
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

/// Bilinearly samples an A8 coverage map at `(fx, fy)` (native coverage texels), clamped to
/// the map; texels outside read 0 (as [`super::display::CoverageMap::at`]).
#[inline]
fn sample_coverage_bilinear(cov: &super::display::CoverageMap, fx: f64, fy: f64) -> u8 {
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let (x0, y0) = (x0 as i32, y0 as i32);
    let a = f64::from(cov.at(x0, y0));
    let b = f64::from(cov.at(x0 + 1, y0));
    let c = f64::from(cov.at(x0, y0 + 1));
    let d = f64::from(cov.at(x0 + 1, y0 + 1));
    let top = a * (1.0 - tx) + b * tx;
    let bot = c * (1.0 - tx) + d * tx;
    (top * (1.0 - ty) + bot * ty).round().clamp(0.0, 255.0) as u8
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

fn draw_image_layer(l: &ImageLayer, canvas: &mut Canvas, region: &Rect, scale: [f64; 2]) {
    let o = l.opacity;
    if o == 0 || l.op == CompositeOp::Grayscale {
        return;
    }
    let (px, iw, ih) = (l.image.pixels(), l.image.width(), l.image.height());
    if iw == 0 || ih == 0 || px.len() != iw as usize * ih as usize {
        return;
    }
    if !is_identity_scale(scale) {
        draw_image_scaled(l, canvas, region, px, iw, ih, scale);
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

/// The resampled base-image path (`ui-scaling.md` §6.3): each axis is handled on its own —
/// its kind (fitted, tiled or clipped) from the native size against the element's layout
/// extent, its scale from [`axis_scale`], and its samples from the default "sharp" filter
/// (sharp bilinear above 1, the area filter below 1). One per-pixel path serves every draw
/// mode. At scale 1 on both axes every pixel is an exact native copy (invariant I2).
fn draw_image_scaled(
    l: &ImageLayer,
    canvas: &mut Canvas,
    region: &Rect,
    px: &[u32],
    iw: u32,
    ih: u32,
    scale: [f64; 2],
) {
    let o = l.opacity;
    let op = l.op;
    let (pw, ph) = (l.rect.width() as i32, l.rect.height() as i32);
    let kind_x = axis_kind(iw as i32, extent_from_physical(pw, scale[0]));
    let kind_y = axis_kind(ih as i32, extent_from_physical(ph, scale[1]));
    let sx = axis_scale(kind_x, pw, iw as i32, scale[0]);
    let sy = axis_scale(kind_y, ph, ih as i32, scale[1]);
    let x_taps = axis_taps(region.x0, region.x1, l.anchor[0], kind_x, sx, iw);
    let y_taps = axis_taps(region.y0, region.y1, l.anchor[1], kind_y, sy, ih);
    let iwu = iw as usize;
    for_spans(region, |y, x0, x1| {
        let yt = &y_taps[(y - region.y0) as usize];
        canvas.modify_span(y, x0, x1, |dst| {
            for (i, d) in dst.iter_mut().enumerate() {
                let xt = &x_taps[(x0 + i as i32 - region.x0) as usize];
                let s = resample_pixel(px, iwu, xt, yt);
                *d = match op {
                    CompositeOp::Replace => s,
                    _ => base_pixel(op, *d, s, o),
                };
            }
        });
    });
}

fn draw_overlay_layer(l: &OverlayLayer, canvas: &mut Canvas, region: &Rect, scale: [f64; 2]) {
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
            if !is_identity_scale(scale) {
                // The overlay uses only the image's alpha (§9.6.3), so the per-axis crisp
                // resample of §6.3 feeds that alpha to the colour `c`.
                let op = l.op;
                let (pw, ph) = (l.rect.width() as i32, l.rect.height() as i32);
                let kind_x = axis_kind(iw as i32, extent_from_physical(pw, scale[0]));
                let kind_y = axis_kind(ih as i32, extent_from_physical(ph, scale[1]));
                let sx = axis_scale(kind_x, pw, iw as i32, scale[0]);
                let sy = axis_scale(kind_y, ph, ih as i32, scale[1]);
                let x_taps = axis_taps(region.x0, region.x1, anchor[0], kind_x, sx, iw);
                let y_taps = axis_taps(region.y0, region.y1, anchor[1], kind_y, sy, ih);
                let iwu = iw as usize;
                for_spans(region, |y, x0, x1| {
                    let yt = &y_taps[(y - region.y0) as usize];
                    canvas.modify_span(y, x0, x1, |dst| {
                        for (i, d) in dst.iter_mut().enumerate() {
                            let xt = &x_taps[(x0 + i as i32 - region.x0) as usize];
                            let s = resample_pixel(px, iwu, xt, yt);
                            *d = overlay_image_pixel(op, *d, s, c);
                        }
                    });
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

fn draw_text(run: &GlyphRun, canvas: &mut Canvas, region: &Rect, scale: [f64; 2]) {
    if !is_identity_scale(scale) {
        draw_text_scaled(run, canvas, region, scale);
        return;
    }
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

/// The resampled glyph path (`ui-scaling.md` §6.1, §6.2): each glyph box carries its **scaled**
/// destination position and size, while `src` still addresses the font's **native** coverage
/// map; `scale` is the content scale `c`, so a canvas pixel reads the native coverage at
/// `src + (canvas − dest) ÷ c`, bilinearly. This keeps the glyph atlas native (never
/// re-rasterised) and the text correctly sized; it is soft, not yet crisp (crisp text needs the
/// derived-glyph source of `ui-scaling.md` §6.4, a later increment).
fn draw_text_scaled(run: &GlyphRun, canvas: &mut Canvas, region: &Rect, scale: [f64; 2]) {
    let cov = &*run.coverage;
    let colour = run.colour;
    let amul = text_amul(colour);
    let (inv_x, inv_y) = (1.0 / scale[0], 1.0 / scale[1]);
    for g in &run.glyphs {
        let b = Rect::from_xywh(g.dest[0], g.dest[1], g.size[0], g.size[1]).intersect(region);
        if b.is_empty() {
            continue;
        }
        for_spans(&b, |y, x0, x1| {
            let fy = f64::from(g.src[1]) + (f64::from(y) - f64::from(g.dest[1])) * inv_y;
            canvas.modify_span(y, x0, x1, |dst| {
                for (i, d) in dst.iter_mut().enumerate() {
                    let xc = x0 + i as i32;
                    let fx = f64::from(g.src[0]) + (f64::from(xc) - f64::from(g.dest[0])) * inv_x;
                    let c = sample_coverage_bilinear(cov, fx, fy);
                    if c == 0 {
                        continue;
                    }
                    *d = if *d >= 0xFF00_0000 {
                        text_opaque(*d, c, amul, colour)
                    } else {
                        text_pixel(*d, c, colour)
                    };
                }
            });
        });
    }
}
