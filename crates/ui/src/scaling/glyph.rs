// SPDX-License-Identifier: GPL-3.0-or-later
//! Text at the scaled size (`ui-scaling.md` §6.4), the glyph cache (§6.5) and the cursor
//! size (§6.7).
//!
//! Text is laid out and rasterised at the height it is shown, from a glyph record chosen for
//! that height or derived from the nearest record in the same font *ladder*. This module holds
//! the pure decisions — the target height, the ladder choice, the derived glyph metrics — and a
//! bounded cache of derived glyphs. Downsampled coverage (`k < 1`) reuses the area filter of
//! [`super::image`] ([`downsample_coverage`]); the distance method for upscaled coverage
//! (`k > 1`, `ui-scaling.md` §6.4.4) is [`distance_coverage`], implemented in this module.

use std::collections::{BTreeMap, HashMap};

use super::image::area_weights;

/// The largest derived-glyph coverage this module allocates, in texels. A bound so a hostile
/// derived box cannot drive an unbounded allocation on the upscale path (§6.4.4); it is far
/// larger than any real glyph (a 2048 × 2048 cover).
const MAX_DERIVED_GLYPH_TEXELS: usize = 1 << 22;

/// The least target glyph height (`ui-scaling.md` §6.4.1): text is never rasterised below 6
/// physical pixels.
pub const MIN_TARGET_HEIGHT: i32 = 6;

/// The target glyph height `t` (`ui-scaling.md` §6.4.1): for a font record of line height
/// `line_height`, an element content scale `c` and a text factor `x` (1.0 unless the optional
/// text size applies), `t = ⌊line_height × c × x + ½⌋`, at least [`MIN_TARGET_HEIGHT`]. At
/// `c × x = 1` exactly, `t = line_height` (invariant I7). Non-finite or non-positive factors
/// clamp to the minimum.
#[must_use]
pub fn target_height(line_height: i32, c: f64, x: f64) -> i32 {
    if line_height <= 0 || !c.is_finite() || !x.is_finite() || c <= 0.0 || x <= 0.0 {
        return MIN_TARGET_HEIGHT;
    }
    let t = (f64::from(line_height) * c * x + 0.5).floor();
    (t as i32).max(MIN_TARGET_HEIGHT)
}

/// Which record of a font ladder a target height is served from (`ui-scaling.md` §6.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A record of the ladder has exactly the target line height: use it unchanged.
    Exact,
    /// The shortest record taller than the target: derive by the area filter (`k < 1`).
    Downsampled,
    /// The tallest record (target taller than every record): derive by the distance method
    /// (`k > 1`).
    Distance,
}

/// The chosen glyph source for a target height (`ui-scaling.md` §6.4.3): which record, how it
/// is served, and the scale `k` (1.0 for an exact record).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphSource {
    /// How the record is served.
    pub kind: SourceKind,
    /// The source record's line height.
    pub source_height: i32,
    /// The target height.
    pub target: i32,
    /// The scale from the source record to the target, `target ÷ source_height`.
    pub k: f64,
}

/// Chooses the glyph source for a target height from a font ladder (`ui-scaling.md` §6.4.3).
/// `ladder` is the set of line heights of the records that differ only in size (§6.4.2); it
/// need not be sorted and may hold duplicates. An empty ladder yields an [`SourceKind::Exact`]
/// source at the target (a font with no records derives from itself at the target, §13 OPEN).
#[must_use]
pub fn choose_glyph_source(ladder: &[i32], target: i32) -> GlyphSource {
    let target = target.max(1);
    // Exact.
    if ladder.contains(&target) {
        return GlyphSource {
            kind: SourceKind::Exact,
            source_height: target,
            target,
            k: 1.0,
        };
    }
    // Shortest record taller than the target.
    let taller = ladder.iter().copied().filter(|&h| h > target).min();
    if let Some(src) = taller {
        return GlyphSource {
            kind: SourceKind::Downsampled,
            source_height: src,
            target,
            k: f64::from(target) / f64::from(src),
        };
    }
    // Tallest record (target taller than all), or the target itself for an empty ladder.
    let tallest = ladder.iter().copied().filter(|&h| h > 0).max();
    match tallest {
        Some(src) => GlyphSource {
            kind: SourceKind::Distance,
            source_height: src,
            target,
            k: f64::from(target) / f64::from(src),
        },
        None => GlyphSource {
            kind: SourceKind::Exact,
            source_height: target,
            target,
            k: 1.0,
        },
    }
}

/// A glyph's metrics (`fonts.md` §3.2, as `ui-scaling.md` §6.4.4 reads them). Integer pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphMetrics {
    /// Left side bearing `A` (pixels from the pen to the ink box's left).
    pub a: i32,
    /// Ink box width `W`.
    pub w: i32,
    /// Right side bearing `B` (pixels from the ink box's right to the next pen).
    pub b: i32,
    /// Top side bearing `T` (pixels from the ascent line to the ink box's top).
    pub t: i32,
    /// Ink box height `H`.
    pub h: i32,
    /// The advance width (`A + W + B`).
    pub advance: i32,
    /// The record's line height.
    pub line_height: i32,
    /// The baseline, pixels from the top of the line box.
    pub baseline: i32,
    /// The stamped-outline border radius in pixels (0 when the font has an outline atlas).
    pub outline_border: i32,
}

/// Derives a glyph's metrics at scale `k` for a target `line_height` (`ui-scaling.md` §6.4.4).
/// Edges, not sizes, are rounded with `r(v) = ⌊v + ½⌋`, so neighbouring metrics stay
/// consistent. A non-finite or non-positive `k` returns the source metrics unchanged with the
/// new line height.
#[must_use]
pub fn derive_metrics(src: GlyphMetrics, k: f64, line_height: i32) -> GlyphMetrics {
    if !k.is_finite() || k <= 0.0 {
        return GlyphMetrics { line_height, ..src };
    }
    let r = |v: f64| (v + 0.5).floor() as i32;
    let kf = k;
    let a = f64::from(src.a);
    let w = f64::from(src.w);
    let b = f64::from(src.b);
    let t = f64::from(src.t);
    let h = f64::from(src.h);
    let left = r(a * kf);
    let right = r((a + w) * kf);
    let advance = r((a + w + b) * kf);
    let top = r(t * kf);
    let bottom = r((t + h) * kf);
    let baseline = r(f64::from(src.baseline) * kf).min(line_height - 1);
    GlyphMetrics {
        a: left,
        w: right - left,
        b: advance - right,
        t: top,
        h: bottom - top,
        advance,
        line_height,
        baseline,
        outline_border: r(f64::from(src.outline_border) * kf),
    }
}

/// Derives a downsampled glyph's coverage (`ui-scaling.md` §6.4.4 "Coverage, downsampled"):
/// the area filter of §6.3 on the coverage values, mapping the source box onto the rounded
/// derived box. `src` is a `src_w` × `src_h` coverage bitmap (row-major, one byte per texel);
/// returns the `dst_w` × `dst_h` derived bitmap. Used for `k < 1`.
///
/// Downsampling never enlarges, so the derived box must be no larger than the source box
/// (`dst_w ≤ src_w`, `dst_h ≤ src_h`); the returned buffer is then always within the source
/// slice, so the allocation is bounded by the input. Returns an empty vector when the inputs
/// are not a valid downsample — any zero extent, a derived box larger than the source box, or a
/// source slice shorter than `src_w` × `src_h` — treating every such case as hostile rather
/// than panicking (a debug overflow check) or wrapping (a release overflow that then indexes
/// out of bounds) on an unchecked dimension product.
#[must_use]
pub fn downsample_coverage(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    // `checked_mul` guards the source area so a hostile `src_w`/`src_h` can neither overflow nor
    // wrap. With `dst ≤ src` the output `dst_w × dst_h ≤ src_w × src_h ≤ src.len()`, so the
    // allocation below is bounded by the input and the products cannot overflow.
    let valid = src_w != 0
        && src_h != 0
        && dst_w != 0
        && dst_h != 0
        && dst_w <= src_w
        && dst_h <= src_h
        && src_w
            .checked_mul(src_h)
            .is_some_and(|area| src.len() >= area);
    if !valid {
        return Vec::new();
    }
    let sx = dst_w as f64 / src_w as f64;
    let sy = dst_h as f64 / src_h as f64;
    let wx = area_weights(dst_w, src_w, sx);
    let wy = area_weights(dst_h, src_h, sy);
    let mut out = vec![0u8; dst_w * dst_h];
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let mut acc = 0.0f64;
            for &(syi, wyf) in &wy[dy] {
                let row = syi * src_w;
                for &(sxi, wxf) in &wx[dx] {
                    acc += f64::from(src[row + sxi]) * wxf * wyf;
                }
            }
            out[dy * dst_w + dx] = acc.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Derives an upscaled glyph's coverage by the distance method (`ui-scaling.md` §6.4.4
/// "Coverage, distance method"), for `k > 1`. `src` is a `src_w` × `src_h` coverage bitmap
/// (row-major, one byte per texel: 0 = empty, 255 = full); returns the `dst_w` × `dst_h`
/// derived bitmap.
///
/// The source box is padded with 4 texels of zero coverage on each side; a signed distance in
/// source pixels is built for every padded texel (positive inside where coverage ≥ ½, negative
/// outside, a partial texel carrying its own `coverage − ½`, every magnitude clamped to ±4);
/// then each derived pixel samples that field bilinearly at its centre taken back into source
/// coordinates, multiplies by the derivation scale `k`, and its coverage is
/// `clamp(distance × k + ½, 0, 1)`. A single blended pixel spans an edge. `k` is read from the
/// box ratio (`dst ÷ src`) per axis and averaged, which equals the glyph's `k` for a true
/// derivation (`dst ≈ src × k` on both axes).
///
/// Returns an empty vector when the inputs are not a valid upscale — any zero extent, a derived
/// box smaller than the source box, a source slice shorter than `src_w` × `src_h`, or a padded
/// or derived box so large it would overflow or exceed `MAX_DERIVED_GLYPH_TEXELS` (4,194,304
/// texels) — treating every such case as hostile rather than panicking or allocating from an
/// unchecked dimension.
#[must_use]
pub fn distance_coverage(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    const PAD: usize = 4;
    // Padded and derived areas must not overflow; the source slice must hold the source box; the
    // derived box must be no smaller than the source box (an upscale) and within the allocation
    // bound. `checked_*` keeps a hostile dimension from overflowing (a debug panic) or wrapping.
    let padded_ok = src_w
        .checked_add(2 * PAD)
        .zip(src_h.checked_add(2 * PAD))
        .and_then(|(pw, ph)| pw.checked_mul(ph))
        .is_some();
    let valid = src_w != 0
        && src_h != 0
        && dst_w != 0
        && dst_h != 0
        && dst_w >= src_w
        && dst_h >= src_h
        && padded_ok
        && src_w
            .checked_mul(src_h)
            .is_some_and(|area| src.len() >= area)
        && dst_w
            .checked_mul(dst_h)
            .is_some_and(|area| area <= MAX_DERIVED_GLYPH_TEXELS);
    if !valid {
        return Vec::new();
    }
    let pw = src_w + 2 * PAD;
    let ph = src_h + 2 * PAD;
    // Coverage in [0, 1] of a padded texel; the padding ring is empty.
    let cov = |px: usize, py: usize| -> f64 {
        if px < PAD || py < PAD || px >= PAD + src_w || py >= PAD + src_h {
            0.0
        } else {
            f64::from(src[(py - PAD) * src_w + (px - PAD)]) / 255.0
        }
    };
    // Signed distance field over the padded grid, in source pixels, clamped to ±4.
    let mut field = vec![0.0f64; pw * ph];
    for py in 0..ph {
        for px in 0..pw {
            let c = cov(px, py);
            let d = if c > 0.0 && c < 1.0 {
                // A partial texel carries its own sub-pixel distance.
                c - 0.5
            } else {
                let inside = c >= 0.5;
                // Euclidean distance to the nearest texel of the other class, searched within
                // the ±4 clamp (one past 4 so the subtracted ½ still resolves); past it it is ±4.
                let r = PAD + 1;
                let x0 = px.saturating_sub(r);
                let y0 = py.saturating_sub(r);
                let x1 = (px + r).min(pw - 1);
                let y1 = (py + r).min(ph - 1);
                let mut best = f64::INFINITY;
                for qy in y0..=y1 {
                    for qx in x0..=x1 {
                        if (cov(qx, qy) >= 0.5) != inside {
                            let dx = qx as f64 - px as f64;
                            let dy = qy as f64 - py as f64;
                            let e = (dx * dx + dy * dy).sqrt();
                            if e < best {
                                best = e;
                            }
                        }
                    }
                }
                let mag = if best.is_finite() { best - 0.5 } else { 4.0 };
                let mag = mag.clamp(0.0, 4.0);
                if inside { mag } else { -mag }
            };
            field[py * pw + px] = d.clamp(-4.0, 4.0);
        }
    }
    // Sample the field for each derived pixel and map the distance to coverage.
    let kx = dst_w as f64 / src_w as f64;
    let ky = dst_h as f64 / src_h as f64;
    let k = (kx + ky) / 2.0;
    let sample = |fx: f64, fy: f64| -> f64 {
        let cx = fx.clamp(0.0, (pw - 1) as f64);
        let cy = fy.clamp(0.0, (ph - 1) as f64);
        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(pw - 1);
        let y1 = (y0 + 1).min(ph - 1);
        let tx = cx - x0 as f64;
        let ty = cy - y0 as f64;
        let top = field[y0 * pw + x0] + (field[y0 * pw + x1] - field[y0 * pw + x0]) * tx;
        let bot = field[y1 * pw + x0] + (field[y1 * pw + x1] - field[y1 * pw + x0]) * tx;
        top + (bot - top) * ty
    };
    let mut out = vec![0u8; dst_w * dst_h];
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Derived pixel centre back into source index space, then into the padded grid.
            let sxp = (dx as f64 + 0.5) / kx - 0.5 + PAD as f64;
            let syp = (dy as f64 + 0.5) / ky - 0.5 + PAD as f64;
            let c = (sample(sxp, syp) * k + 0.5).clamp(0.0, 1.0);
            out[dy * dst_w + dx] = (c * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// The cursor image side in physical pixels (`ui-scaling.md` §6.7):
/// `trunc(32 × Se × cursor_scale)`, at least 1. `cursor_scale` is the `ui_scale_cursor`
/// factor (now a factor on top of `Se`).
#[must_use]
pub fn cursor_size(se: f64, cursor_scale: f64) -> i32 {
    let f = cursor_factor(se, cursor_scale);
    ((32.0 * f).trunc() as i32).max(1)
}

/// The factor a cursor's size and hot spot are scaled by (`ui-scaling.md` §6.7):
/// `Se × cursor_scale`, clamped to a finite non-negative value.
#[must_use]
pub fn cursor_factor(se: f64, cursor_scale: f64) -> f64 {
    let se = if se.is_finite() && se > 0.0 { se } else { 1.0 };
    let cs = if cursor_scale.is_finite() && cursor_scale > 0.0 {
        cursor_scale
    } else {
        1.0
    };
    se * cs
}

/// The cursor hot spot scaled and rounded (`ui-scaling.md` §6.7).
#[must_use]
pub fn cursor_hotspot(coord: i32, se: f64, cursor_scale: f64) -> i32 {
    (f64::from(coord) * cursor_factor(se, cursor_scale) + 0.5).floor() as i32
}

/// The key of a cached derived glyph (`ui-scaling.md` §6.5): font DID, target height, glyph
/// (codepoint or glyph index) and whether it is the outline or the fill. The source kind
/// (§6.4.3) is a function of the font DID and target height through the ladder choice, so it is
/// not part of the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// The font record's DID.
    pub font_did: u32,
    /// The target height.
    pub target_height: u16,
    /// Which codepoint (or glyph index) this is.
    pub glyph: u32,
    /// `true` for the outline coverage, `false` for the fill.
    pub outline: bool,
}

/// A derived glyph held in the cache: its metrics and coverage bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedGlyph {
    /// The derived metrics.
    pub metrics: GlyphMetrics,
    /// The coverage bitmap, row-major, `metrics.w × metrics.h` bytes.
    pub coverage: Vec<u8>,
}

impl DerivedGlyph {
    fn bytes(&self) -> usize {
        self.coverage.len() + std::mem::size_of::<GlyphMetrics>()
    }
}

/// A bounded cache of derived glyphs (`ui-scaling.md` §6.5), keyed by [`GlyphKey`] and evicting
/// least-recently-used entries once its byte budget is exceeded. It counts toward the decoded
/// image budget of §8; the client sets the budget from `image_lru_mib`.
#[derive(Debug)]
pub struct GlyphCache {
    budget: usize,
    bytes: usize,
    clock: u64,
    map: HashMap<GlyphKey, (u64, DerivedGlyph)>,
    /// Recency index: cache clock → key, ascending, so its first entry is the least-recently-used
    /// glyph. Kept in exact step with `map` (one entry per cached glyph) so eviction finds its
    /// victim in `O(log n)` instead of scanning the whole map on every removal.
    lru: BTreeMap<u64, GlyphKey>,
    /// Glyphs inserted since the cache was made (each one a derivation; budget S7 counts them).
    inserted: u64,
}

impl GlyphCache {
    /// A cache with a byte budget (`ui-scaling.md` §6.5, §8).
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget: budget_bytes,
            bytes: 0,
            clock: 0,
            map: HashMap::new(),
            lru: BTreeMap::new(),
            inserted: 0,
        }
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Changes the byte budget, evicting least-recently-used glyphs until the cache fits it.
    pub fn set_budget(&mut self, budget_bytes: usize) {
        self.budget = budget_bytes;
        self.evict();
    }

    /// Removes every glyph (the budget and the insertion count are kept).
    pub fn clear(&mut self) {
        self.map.clear();
        self.lru.clear();
        self.bytes = 0;
    }

    /// How many glyphs were inserted since the cache was made: every insertion is one
    /// derivation (`ui-scaling.md` §6.5; the paint slice counts them, budget S7).
    #[must_use]
    pub fn insertions(&self) -> u64 {
        self.inserted
    }

    /// The current total byte size of the cached glyphs.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The number of cached glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache holds no glyph.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Looks a glyph up, marking it most-recently-used.
    pub fn get(&mut self, key: &GlyphKey) -> Option<&DerivedGlyph> {
        self.clock += 1;
        let clock = self.clock;
        let old = self.map.get(key).map(|slot| slot.0)?;
        // Move the recency index entry from the old clock to the new one.
        self.lru.remove(&old);
        self.lru.insert(clock, *key);
        let slot = self.map.get_mut(key)?;
        slot.0 = clock;
        Some(&slot.1)
    }

    /// Inserts a derived glyph, evicting least-recently-used entries until the cache is within
    /// its budget. A single glyph larger than the whole budget is stored and then immediately
    /// eligible for eviction (so `get` right after may return `None`), never retained over the
    /// budget.
    pub fn insert(&mut self, key: GlyphKey, glyph: DerivedGlyph) {
        self.clock += 1;
        let clock = self.clock;
        let new_bytes = glyph.bytes();
        if let Some((old_clock, old)) = self.map.insert(key, (clock, glyph)) {
            self.bytes = self.bytes.saturating_sub(old.bytes());
            self.lru.remove(&old_clock);
        }
        self.lru.insert(clock, key);
        self.bytes = self.bytes.saturating_add(new_bytes);
        self.inserted = self.inserted.saturating_add(1);
        self.evict();
    }

    fn evict(&mut self) {
        // The least-recently-used glyph is the lowest clock in the recency index; `pop_first` is
        // `O(log n)`, so evicting the whole cache (e.g. for one oversized glyph) is `O(n log n)`,
        // not the `O(n²)` of a per-removal linear scan.
        while self.bytes > self.budget {
            let Some((_, lru_key)) = self.lru.pop_first() else {
                break;
            };
            if let Some((_, g)) = self.map.remove(&lru_key) {
                self.bytes = self.bytes.saturating_sub(g.bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_heights_from_examples() {
        // §10.1 C1 loading label: verdana12 at c = 0.667 → 8.
        assert_eq!(target_height(12, 0.667, 1.0), 8);
        // §10.2 bold 16 at c = 0.8 / 1.6 / 2.4.
        assert_eq!(target_height(16, 0.8, 1.0), 13);
        assert_eq!(target_height(16, 1.6, 1.0), 26);
        assert_eq!(target_height(16, 2.4, 1.0), 38);
        // Identity at c × x = 1.
        assert_eq!(target_height(20, 1.0, 1.0), 20);
        // Floor.
        assert_eq!(target_height(10, 0.1, 1.0), MIN_TARGET_HEIGHT);
    }

    #[test]
    fn glyph_source_choice() {
        let verdana = [10, 12, 14, 16, 18, 20, 22, 23];
        assert_eq!(choose_glyph_source(&verdana, 16).kind, SourceKind::Exact);
        let s = choose_glyph_source(&verdana, 8);
        assert_eq!(s.kind, SourceKind::Downsampled);
        assert_eq!(s.source_height, 10);
        assert!((s.k - 0.8).abs() < 1e-9);
        let s = choose_glyph_source(&verdana, 24);
        assert_eq!(s.kind, SourceKind::Distance);
        assert_eq!(s.source_height, 23);
        // A ladder with only one record (the §10.2 bold font).
        let bold = [16];
        assert_eq!(choose_glyph_source(&bold, 13).kind, SourceKind::Downsampled);
        assert_eq!(choose_glyph_source(&bold, 26).kind, SourceKind::Distance);
        // Empty ladder derives from itself.
        assert_eq!(choose_glyph_source(&[], 20).kind, SourceKind::Exact);
    }

    /// Derived metrics keep the ink box within one pixel of the source scaled by k (T2).
    #[test]
    fn derived_metrics_ink_box() {
        let src = GlyphMetrics {
            a: 2,
            w: 10,
            b: 3,
            t: 1,
            h: 14,
            advance: 15,
            line_height: 16,
            baseline: 13,
            outline_border: 0,
        };
        for &k in &[0.5, 0.8, 1.25, 2.0, 2.6667] {
            let t = (16.0_f64 * k).round() as i32;
            let d = derive_metrics(src, k, t);
            // Ink box within one pixel of source scaled by k.
            assert!(
                (f64::from(d.w) - f64::from(src.w) * k).abs() <= 1.0,
                "w k={k}"
            );
            assert!(
                (f64::from(d.h) - f64::from(src.h) * k).abs() <= 1.0,
                "h k={k}"
            );
            // Advance = A' + W' + B'.
            assert_eq!(d.advance, d.a + d.w + d.b);
            // Baseline below the line height.
            assert!(d.baseline < t);
            // No negative extents.
            assert!(d.w >= 0 && d.h >= 0);
        }
    }

    /// A solid source rectangle stays solid when downsampled (T1).
    #[test]
    fn downsample_solid() {
        let src = vec![255u8; 8 * 8];
        let out = downsample_coverage(&src, 8, 8, 4, 4);
        assert!(out.iter().all(|&v| v == 255), "interior not solid: {out:?}");
    }

    #[test]
    fn downsample_total_preserved() {
        // Area filter preserves total coverage up to rounding (T3 spirit).
        let mut src = vec![0u8; 10 * 10];
        for (i, v) in src.iter_mut().enumerate() {
            *v = ((i * 37) % 256) as u8;
        }
        let total_src: u64 = src.iter().map(|&v| u64::from(v)).sum();
        let out = downsample_coverage(&src, 10, 10, 5, 5);
        let total_out: u64 = out.iter().map(|&v| u64::from(v)).sum();
        // Each derived pixel averages 4 source texels; total scales by (5*5)/(10*10) = 1/4.
        let expected = total_src as f64 / 4.0;
        assert!((total_out as f64 - expected).abs() < expected * 0.05 + 25.0);
    }

    #[test]
    fn coverage_hostile_dims_no_panic() {
        // Overflowing or invalid downsample dims return empty and never panic (the dev profile
        // keeps overflow checks on). `usize::MAX × usize::MAX` would overflow the area product.
        assert!(downsample_coverage(&[], usize::MAX, usize::MAX, 1, 1).is_empty());
        assert!(downsample_coverage(&[255; 4], 2, 2, 8, 8).is_empty()); // dst > src: not a downsample
        assert!(downsample_coverage(&[255; 1], 2, 2, 1, 1).is_empty()); // source slice too small
        assert!(downsample_coverage(&[255; 4], 2, 2, 0, 1).is_empty()); // zero extent
        // Upscale guards.
        assert!(distance_coverage(&[], usize::MAX, usize::MAX, usize::MAX, usize::MAX).is_empty());
        assert!(distance_coverage(&[255; 4], 2, 2, 1, 1).is_empty()); // dst < src: not an upscale
        assert!(distance_coverage(&[255; 1], 1, 1, 1 << 30, 1 << 30).is_empty()); // output too large
    }

    /// The distance method keeps a solid source solid in the interior with sharp edges (T1).
    #[test]
    fn distance_upscale_solid() {
        let src = vec![255u8; 4 * 4];
        let out = distance_coverage(&src, 4, 4, 8, 8);
        assert_eq!(out.len(), 8 * 8);
        // The 2×2 centre is fully covered.
        for &(x, y) in &[(3, 3), (3, 4), (4, 3), (4, 4)] {
            assert_eq!(out[y * 8 + x], 255, "centre not solid at ({x},{y})");
        }
        // At most one partial pixel per edge: the second column in from each side is already solid.
        for y in 2..6 {
            assert_eq!(out[y * 8 + 1], 255, "left-but-one not solid at row {y}");
            assert_eq!(out[y * 8 + 6], 255, "right-but-one not solid at row {y}");
        }
    }

    #[test]
    fn cursor_sizes() {
        assert_eq!(cursor_size(1.0, 1.0), 32);
        assert_eq!(cursor_size(1.5, 1.0), 48);
        assert_eq!(cursor_size(2.0, 1.0), 64);
        assert_eq!(cursor_size(0.9375, 1.0), 30); // trunc(30.0)
        assert_eq!(cursor_hotspot(4, 2.0, 1.0), 8);
    }

    #[test]
    fn glyph_cache_lru() {
        let one = DerivedGlyph {
            metrics: GlyphMetrics::default(),
            coverage: vec![0u8; 100],
        };
        let budget = one.bytes() * 2 + 1;
        let mut cache = GlyphCache::new(budget);
        let k = |g: u32| GlyphKey {
            font_did: 1,
            target_height: 16,
            glyph: g,
            outline: false,
        };
        cache.insert(k(1), one.clone());
        cache.insert(k(2), one.clone());
        // Touch glyph 1 so glyph 2 is the LRU.
        assert!(cache.get(&k(1)).is_some());
        cache.insert(k(3), one.clone());
        assert!(cache.bytes() <= budget);
        assert!(
            cache.get(&k(2)).is_none(),
            "LRU (glyph 2) should be evicted"
        );
        assert!(cache.get(&k(1)).is_some());
        assert!(cache.get(&k(3)).is_some());
    }

    #[test]
    fn glyph_cache_zero_budget() {
        let mut cache = GlyphCache::new(0);
        cache.insert(
            GlyphKey {
                font_did: 1,
                target_height: 16,
                glyph: 1,
                outline: false,
            },
            DerivedGlyph {
                metrics: GlyphMetrics::default(),
                coverage: vec![0u8; 10],
            },
        );
        assert!(cache.is_empty());
    }
}
