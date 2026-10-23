// SPDX-License-Identifier: GPL-3.0-or-later
//! Drawing text (`fonts.md` §6.2, §7.5, §7.6 step 8, §8.1–§8.5).
//!
//! [`draw`] walks the lines of a [`TextLayout`] twice, as the client does: first the outline
//! of every glyph (when the outline flag is set), then the fill of every glyph with its
//! underline before it. The result is a list of [`TextOp`]s in canvas coordinates: one-byte
//! glyph coverage runs ([`GlyphRun`]: one colour, one coverage map, boxes copied 1 : 1),
//! underline fills ([`FillRect`], overwriting) and bullet positions. [`crate::paint`] does
//! the per-pixel compositing of §8.4.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::font::Glyph;
use tessera_material::ui_image::UiImage;

use super::chars::TextChar;
use super::layout::{
    LineBox, Metrics, ResolvedDraw, ScaleCtx, TextLayout, metrics, resolve_scaled,
    underline_thickness,
};
use super::{FontFace, sat32};
use crate::paint::{
    CompositeOp, CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun, ImageLayer,
    MAX_GLYPHS_PER_RUN, MAX_OPS, PaintError, PaintOp, Rect,
};
use crate::scaling::{
    DerivedGlyph, GlyphCache, GlyphKey, GlyphMetrics, SourceKind, distance_coverage,
    downsample_coverage,
};

/// The default outline colour, opaque black (`fonts.md` §8.1).
const DEFAULT_OUTLINE: u32 = 0xFF00_0000;

/// What [`draw`] needs besides the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawParams {
    /// Canvas position of text coordinate (0, 0): the element's top-left corner in the
    /// canvas, minus its scroll offsets.
    pub origin: [i32; 2],
    /// The element's clip in canvas coordinates (its box and its ancestors' boxes).
    pub clip: Rect,
    /// `UICore_Text_outline` (default false).
    pub outline: bool,
    /// The outline colour, `0xAARRGGBB` (`UICore_Text_font_outline_color`; default opaque
    /// black).
    pub outline_colour: u32,
    /// Size of the element's `UICore_Text_bullet_image`, when it has one (§7.5).
    pub bullet: Option<[u32; 2]>,
}

impl Default for DrawParams {
    fn default() -> Self {
        Self {
            origin: [0, 0],
            clip: Rect::new(i32::MIN, i32::MIN, i32::MAX, i32::MAX),
            outline: false,
            outline_colour: DEFAULT_OUTLINE,
            bullet: None,
        }
    }
}

/// One text paint operation, in drawing order.
#[derive(Debug, Clone)]
pub enum TextOp {
    /// Glyph boxes in one colour from one coverage map, composited with `fonts.md` §8.4.
    Glyphs(GlyphRun),
    /// An underline rectangle overwritten with its colour (§8.5).
    Underline(FillRect),
    /// The list-item bullet image drawn at its own size into this rectangle (§7.5), clipped
    /// by the element's clip.
    Bullet(Rect),
}

/// The bullet image and how it is blended (like other UI art of the element, §7.5).
#[derive(Debug, Clone)]
pub struct BulletArt {
    /// The decoded image.
    pub image: Arc<UiImage>,
    /// The operation of the element's draw mode.
    pub op: CompositeOp,
    /// The opacity byte.
    pub opacity: u8,
}

/// The paint operations of one text element.
#[derive(Debug, Clone, Default)]
pub struct TextPaint {
    /// Operations in drawing order.
    pub ops: Vec<TextOp>,
    /// The clip every operation carries.
    pub clip: Rect,
}

impl TextPaint {
    /// The canvas area the operations may change.
    #[must_use]
    pub fn area(&self) -> Rect {
        let mut a = Rect::EMPTY;
        for op in &self.ops {
            let r = match op {
                TextOp::Glyphs(r) => r.area(),
                TextOp::Underline(f) => f.rect.intersect(&f.clip),
                TextOp::Bullet(r) => r.intersect(&self.clip),
            };
            a = a.union(&r);
        }
        a
    }

    /// Appends the operations to a display list: glyph runs as [`PaintOp::Text`],
    /// underlines as [`PaintOp::Fill`], bullets as [`PaintOp::Image`] when `bullet` is given
    /// (dropped otherwise). All or nothing: on an error the list is unchanged.
    ///
    /// # Errors
    /// The list's caps ([`PaintError::TooManyOps`], [`PaintError::TooManyGlyphs`]).
    pub fn push_to(
        self,
        list: &mut DisplayList,
        bullet: Option<&BulletArt>,
    ) -> Result<(), PaintError> {
        let mut count = 0usize;
        for op in &self.ops {
            match op {
                TextOp::Glyphs(r) if r.glyphs.len() > MAX_GLYPHS_PER_RUN => {
                    return Err(PaintError::TooManyGlyphs);
                }
                TextOp::Bullet(_) if bullet.is_none() => {}
                _ => count += 1,
            }
        }
        if list.len().saturating_add(count) > MAX_OPS {
            return Err(PaintError::TooManyOps);
        }
        let clip = self.clip;
        for op in self.ops {
            match op {
                TextOp::Glyphs(r) => list.push(PaintOp::Text(Arc::new(r)))?,
                TextOp::Underline(f) => list.push(PaintOp::Fill(f))?,
                TextOp::Bullet(rect) => {
                    if let Some(b) = bullet {
                        list.push(PaintOp::Image(ImageLayer {
                            rect,
                            clip,
                            anchor: [rect.x0, rect.y0],
                            image: Arc::clone(&b.image),
                            op: b.op,
                            opacity: b.opacity,
                        }))?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Collects glyph boxes into runs of one colour and one coverage map, and underlines into
/// one fill per contiguous stretch where that cannot change a pixel.
struct Runs {
    ops: Vec<TextOp>,
    cur: Option<GlyphRun>,
    clip: Rect,
    /// The latest operation, when it is an underline still open for merging: its index in
    /// `ops` and the clipped bounding box of the glyphs drawn after it.
    open_underline: Option<(usize, Rect)>,
}

impl Runs {
    fn flush(&mut self) {
        if let Some(r) = self.cur.take()
            && !r.glyphs.is_empty()
        {
            self.ops.push(TextOp::Glyphs(r));
        }
    }

    fn glyph(&mut self, colour: u32, map: &Arc<CoverageMap>, b: GlyphBox) {
        let r = Rect::from_xywh(b.dest[0], b.dest[1], b.size[0], b.size[1]);
        if r.intersect(&self.clip).is_empty() {
            return;
        }
        let same = self.cur.as_ref().is_some_and(|c| {
            c.colour == colour
                && Arc::ptr_eq(&c.coverage, map)
                && c.glyphs.len() < MAX_GLYPHS_PER_RUN
        });
        if !same {
            self.flush();
            self.cur = Some(GlyphRun {
                clip: self.clip,
                colour,
                coverage: Arc::clone(map),
                glyphs: Vec::new(),
            });
        }
        if let Some(c) = self.cur.as_mut() {
            c.glyphs.push(b);
        }
        if let Some((_, since)) = self.open_underline.as_mut() {
            *since = since.union(&r.intersect(&self.clip));
        }
    }

    /// A bullet (or any other operation that ends a glyph run). Culled outside the clip.
    fn other(&mut self, op: TextOp, rect: Rect) {
        if rect.intersect(&self.clip).is_empty() {
            return;
        }
        self.flush();
        self.ops.push(op);
        self.open_underline = None;
    }

    /// An underline (§8.5), culled outside the clip. It joins the open underline when both
    /// have one colour and one row span, they touch or overlap, and no glyph drawn since the
    /// open one meets it: the fill overwrites and glyphs composite over it, so drawing the
    /// joined fill earlier changes no pixel then. A visible underlined word thus costs one
    /// fill and does not split its glyph run.
    fn underline(&mut self, f: FillRect) {
        let vis = f.rect.intersect(&self.clip);
        if vis.is_empty() {
            return;
        }
        if let Some((idx, since)) = self.open_underline
            && let Some(TextOp::Underline(prev)) = self.ops.get_mut(idx)
            && prev.colour == f.colour
            && prev.clip == f.clip
            && prev.rect.y0 == f.rect.y0
            && prev.rect.y1 == f.rect.y1
            && f.rect.x0 <= prev.rect.x1
            && prev.rect.x0 <= f.rect.x1
            && vis.intersect(&since).is_empty()
        {
            prev.rect = Rect::new(
                prev.rect.x0.min(f.rect.x0),
                prev.rect.y0,
                prev.rect.x1.max(f.rect.x1),
                prev.rect.y1,
            );
            return;
        }
        self.flush();
        self.ops.push(TextOp::Underline(f));
        self.open_underline = Some((self.ops.len() - 1, Rect::EMPTY));
    }
}

fn clamp_size(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Outline,
    Fill,
}

/// Draws one character list continuing on `line` from drawing pen `pen` (line-relative).
/// Returns the pen after the characters.
#[allow(clippy::too_many_arguments)]
fn draw_chars<'a>(
    runs: &mut Runs,
    pass: Pass,
    chars: impl Iterator<Item = &'a TextChar>,
    fonts: &[Option<Arc<FontFace>>],
    line: &LineBox,
    origin: [i64; 2],
    mut pen: i64,
    params: &DrawParams,
    list_extra: i64,
    mut first: bool,
    scale: Option<&ScaleDraw<'_>>,
) -> i64 {
    let line_x = origin[0] + i64::from(line.x);
    let line_top = origin[1] + i64::from(line.top);
    let base_y = line_top + i64::from(line.baseline);
    for c in chars {
        // §7.5 step 3: the first character of a drawn line, when it has the list flag (a
        // newline first character has none, so a later `<li>` character on the same line,
        // with wrapping off, gets no bullet).
        if std::mem::take(&mut first) && c.list_item {
            if let Some([bw, bh]) = params.bullet {
                let rect = Rect::from_xywh(
                    sat32(line_x + i64::from(line.indent)),
                    sat32(base_y - i64::from(bh)),
                    bw,
                    bh,
                );
                runs.other(TextOp::Bullet(rect), rect);
            }
            pen += list_extra;
        }
        if c.is_newline() {
            // Not drawn and does not move the drawing pen (§5).
            continue;
        }
        if let Some(sd) = scale {
            // Crisp scaled path (`ui-scaling.md` §6.4): resolve the glyph at the content scale.
            pen = draw_char_scaled(runs, pass, c, fonts, line, line_x, base_y, pen, params, sd);
            continue;
        }
        let Metrics {
            glyph, baseline, ..
        } = metrics(c, fonts);
        let (Some(g), Some(face)) = (
            glyph,
            fonts.get(usize::from(c.font)).and_then(Option::as_ref),
        ) else {
            // No glyph (§5 step 3) or no font (§9.1 rule 4): nothing drawn, advance 0.
            continue;
        };
        draw_atlas_glyph(
            runs, pass, face, &g, baseline, line, line_x, base_y, pen, c, params,
        );
        pen += i64::from(g.left_bearing) + i64::from(g.width) + i64::from(g.right_bearing);
    }
    pen
}

/// Draws one glyph straight from a face's atlas (`fonts.md` §8.2, §8.3, §8.5): the outline pass
/// (background box or eight foreground stamps) or the fill pass (underline then foreground box).
/// Shared by the unscaled path and the exact-ladder case of the scaled path (`ui-scaling.md`
/// §6.4.3 item 1), so both draw identically. `baseline` is the glyph's own baseline.
#[allow(clippy::too_many_arguments)]
fn draw_atlas_glyph(
    runs: &mut Runs,
    pass: Pass,
    face: &FontFace,
    g: &Glyph,
    baseline: i64,
    line: &LineBox,
    line_x: i64,
    base_y: i64,
    pen: i64,
    c: &TextChar,
    params: &DrawParams,
) {
    let (a, w, h, b) = (
        i64::from(g.left_bearing),
        i64::from(g.width),
        i64::from(g.height),
        i64::from(g.right_bearing),
    );
    let (gx, gy) = (i64::from(g.x), i64::from(g.y));
    let left = line_x + pen + a;
    let top = base_y - baseline + i64::from(g.top);
    match pass {
        Pass::Outline => {
            let colour = params.outline_colour;
            if let Some(bg) = face.background() {
                // §8.2: the bordered box from the background atlas.
                let [bx, by] = face.borders().map(i64::from);
                runs.glyph(
                    colour,
                    bg,
                    GlyphBox {
                        dest: [sat32(left - bx), sat32(top - by)],
                        size: [clamp_size(w + 2 * bx), clamp_size(h + 2 * by)],
                        src: [sat32(gx - bx), sat32(gy - by)],
                    },
                );
            } else if w > 0 && h > 0 {
                // §8.3: eight one-pixel stamps of the foreground glyph.
                for dy in -1..=1i64 {
                    for dx in -1..=1i64 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        runs.glyph(
                            colour,
                            face.foreground(),
                            GlyphBox {
                                dest: [sat32(left + dx), sat32(top + dy)],
                                size: [clamp_size(w), clamp_size(h)],
                                src: [sat32(gx), sat32(gy)],
                            },
                        );
                    }
                }
            }
        }
        Pass::Fill => {
            if let Some(ucol) = c.underline {
                // §8.5: before the glyph, overwrite the underline rectangle.
                let t = i64::from(underline_thickness(line.height, line.leading));
                let stored = (a + w + b).clamp(0, 255);
                let right = line_x + pen + stored + if b < 0 { -b } else { 0 };
                let rect = Rect::new(
                    sat32(line_x + pen),
                    sat32(base_y + t),
                    sat32(right + 1),
                    sat32(base_y + 2 * t),
                );
                runs.underline(FillRect {
                    rect,
                    clip: params.clip,
                    colour: ucol,
                });
            }
            if w > 0 && h > 0 {
                runs.glyph(
                    c.colour,
                    face.foreground(),
                    GlyphBox {
                        dest: [sat32(left), sat32(top)],
                        size: [clamp_size(w), clamp_size(h)],
                        src: [sat32(gx), sat32(gy)],
                    },
                );
            }
        }
    }
}

/// State for the crisp scaled-text path (`ui-scaling.md` §6.4, §6.5): the content scale and
/// ladders, the derived-glyph cache (shared across frames), and a per-draw map from glyph key to a
/// built coverage map so repeated glyphs of one text share one `Arc` (and so merge into one run).
struct ScaleDraw<'a> {
    ctx: &'a ScaleCtx<'a>,
    cache: RefCell<&'a mut GlyphCache>,
    arcs: RefCell<HashMap<GlyphKey, Arc<CoverageMap>>>,
}

/// The stamped-outline radius for a content scale (`ui-scaling.md` §6.4.4 "Outline"):
/// `max(1, round(c))`. At `c = 1` this is 1, the native eight one-pixel stamps (§8.3).
fn stamp_radius(c: f64) -> i64 {
    if !c.is_finite() || c <= 0.0 {
        1
    } else {
        ((c + 0.5).floor() as i64).max(1)
    }
}

/// The derived box size (width, height) for a glyph (`ui-scaling.md` §6.4.4): the derived ink box
/// `W′ × H′` for the fill, or that padded by the derived border for a background outline box.
fn derived_box(dm: &GlyphMetrics, outline: bool) -> (i64, i64) {
    let (w, h) = (i64::from(dm.w).max(0), i64::from(dm.h).max(0));
    if outline {
        let bd = i64::from(dm.outline_border).max(0);
        ((w + 2 * bd).max(1), (h + 2 * bd).max(1))
    } else {
        (w.max(1), h.max(1))
    }
}

/// Derives (or fetches from the cache) one glyph's coverage at the target height
/// (`ui-scaling.md` §6.4.4, §6.5) and returns it as a coverage map. `outline` selects the
/// background (outline) coverage; the fill is `false`. Returns `None` when the source has no such
/// coverage, the derivation is degenerate, or the derived box is outside the coverage-map limits —
/// the glyph (or its outline) is then simply not drawn.
fn derived_cover(
    sd: &ScaleDraw<'_>,
    face: &FontFace,
    g: &Glyph,
    unit: u16,
    kind: SourceKind,
    dm: &GlyphMetrics,
    outline: bool,
) -> Option<Arc<CoverageMap>> {
    let target = u16::try_from(dm.line_height).unwrap_or(u16::MAX);
    let key = GlyphKey {
        face: face.id(),
        target_height: target,
        source: kind,
        glyph: u32::from(unit),
        outline,
    };
    if let Some(arc) = sd.arcs.borrow().get(&key) {
        return Some(Arc::clone(arc));
    }
    // The cross-frame cache holds the derived metrics (whose `w`/`h` are the coverage box) and the
    // coverage bytes; a hit avoids re-deriving. `get` marks the entry most-recently-used.
    let cached = sd.cache.borrow_mut().get(&key).cloned();
    let dg = match cached {
        Some(d) => d,
        None => {
            // S7: the derivation alone is timed (it counts toward the paint slice).
            let t0 = std::time::Instant::now();
            let d = derive_glyph(face, g, kind, dm, outline)?;
            let mut cache = sd.cache.borrow_mut();
            cache.insert(key, d.clone());
            cache.record_derivation(t0.elapsed());
            d
        }
    };
    let w = u32::try_from(dg.metrics.w).ok()?;
    let h = u32::try_from(dg.metrics.h).ok()?;
    let arc = Arc::new(CoverageMap::new(w, h, dg.coverage).ok()?);
    sd.arcs.borrow_mut().insert(key, Arc::clone(&arc));
    Some(arc)
}

/// Derives one glyph's coverage box (`ui-scaling.md` §6.4.4): the fill (`outline` false) or the
/// outline coverage of `g` of `face`, downsampled or by the distance method (`kind`), with the
/// derived metrics `dm`. `None` when the source has no such coverage, the derivation is
/// degenerate or the box is outside the coverage limits; an exact source derives nothing.
pub(crate) fn derive_glyph(
    face: &FontFace,
    g: &Glyph,
    kind: SourceKind,
    dm: &GlyphMetrics,
    outline: bool,
) -> Option<DerivedGlyph> {
    let (src_cov, sw, sh) = face.cover_box(g, outline)?;
    let (dbw, dbh) = derived_box(dm, outline);
    let dbw = usize::try_from(dbw).ok()?;
    let dbh = usize::try_from(dbh).ok()?;
    // The area filter needs `dst ≤ src`; the distance method needs `dst ≥ src` (its guards
    // reject otherwise). Clamp the derived box to that side of the source box so a rounding
    // wobble never loses the glyph.
    let (dw, dh, cov) = match kind {
        SourceKind::Downsampled => {
            let dw = dbw.min(sw).max(1);
            let dh = dbh.min(sh).max(1);
            (dw, dh, downsample_coverage(&src_cov, sw, sh, dw, dh))
        }
        SourceKind::Distance => {
            let dw = dbw.max(sw);
            let dh = dbh.max(sh);
            (dw, dh, distance_coverage(&src_cov, sw, sh, dw, dh))
        }
        // An exact hit never derives coverage (it draws from the atlas).
        SourceKind::Exact => return None,
    };
    if cov.len() != dw.checked_mul(dh)? {
        return None;
    }
    let metrics = GlyphMetrics {
        w: i32::try_from(dw).unwrap_or(i32::MAX),
        h: i32::try_from(dh).unwrap_or(i32::MAX),
        ..*dm
    };
    Some(DerivedGlyph {
        metrics,
        coverage: cov,
    })
}

/// The printable ASCII range a ladder step covers (`ui-scaling.md` §6.5 last sentence, S7).
pub const LADDER_STEP_UNITS: std::ops::RangeInclusive<u16> = 0x20..=0x7E;

/// The glyphs of one ladder step derived ahead of drawing (`ui-scaling.md` §6.5: "a client may
/// derive the printable ASCII range of the fonts on screen on the asset workers ahead of a
/// settle"), with the worker time it took (budget S7: at most 5 ms). Made by
/// [`derive_ladder_step`] on a worker, handed to `Ui::accept_ladder_step` on the frame thread.
#[derive(Debug, Clone, Default)]
pub struct LadderStep {
    /// The derived glyphs with their cache keys.
    pub glyphs: Vec<(GlyphKey, DerivedGlyph)>,
    /// How long the derivation took on the worker.
    pub elapsed: std::time::Duration,
}

/// Derives the printable ASCII range ([`LADDER_STEP_UNITS`]) of `ladder` at target height
/// `target` (`ui-scaling.md` §6.4.3, §6.4.4), fill and, for a face with an outline atlas,
/// outline, keyed as the drawing path keys them; times the whole step (S7). A pure function of
/// its inputs, meant for the asset workers. A target the ladder serves exactly (or directly)
/// derives nothing.
#[must_use]
pub fn derive_ladder_step(ladder: &super::FontLadder, target: i32) -> LadderStep {
    let t0 = std::time::Instant::now();
    let mut glyphs = Vec::new();
    let target = target.max(crate::scaling::glyph::MIN_TARGET_HEIGHT);
    if ladder.is_empty() || ladder.direct_face(target).is_some() {
        return LadderStep {
            glyphs,
            elapsed: t0.elapsed(),
        };
    }
    let src = crate::scaling::choose_glyph_source(&ladder.heights(), target);
    let Some(face) = ladder.face_at(src.source_height) else {
        return LadderStep {
            glyphs,
            elapsed: t0.elapsed(),
        };
    };
    if src.kind != SourceKind::Exact {
        let Ok(th) = u16::try_from(target) else {
            return LadderStep::default();
        };
        for unit in LADDER_STEP_UNITS {
            let Some(g) = face.lookup(unit).glyph else {
                continue;
            };
            let dm = super::layout::derived_metrics_of(face, &g, src.k, target);
            for outline in [false, true] {
                if outline && face.background().is_none() {
                    continue;
                }
                if let Some(d) = derive_glyph(face, &g, src.kind, &dm, outline) {
                    let key = GlyphKey {
                        face: face.id(),
                        target_height: th,
                        source: src.kind,
                        glyph: u32::from(unit),
                        outline,
                    };
                    glyphs.push((key, d));
                }
            }
        }
    }
    LadderStep {
        glyphs,
        elapsed: t0.elapsed(),
    }
}

/// Draws one character on the crisp scaled path (`ui-scaling.md` §6.4). Returns the drawing pen
/// after the character. An exact ladder hit draws from the atlas (identical to the unscaled path);
/// a downsampled or distance-derived glyph draws its derived coverage box.
#[allow(clippy::too_many_arguments)]
fn draw_char_scaled(
    runs: &mut Runs,
    pass: Pass,
    c: &TextChar,
    fonts: &[Option<Arc<FontFace>>],
    line: &LineBox,
    line_x: i64,
    base_y: i64,
    pen: i64,
    params: &DrawParams,
    sd: &ScaleDraw<'_>,
) -> i64 {
    let res = resolve_scaled(c, fonts, sd.ctx);
    let advance = res.a + res.w + res.b;
    match res.draw {
        // No glyph and no font: nothing drawn, advance 0 (as the unscaled path, §5 step 3).
        ResolvedDraw::None => pen,
        ResolvedDraw::Native { face, glyph } => {
            draw_atlas_glyph(
                runs,
                pass,
                &face,
                &glyph,
                res.baseline,
                line,
                line_x,
                base_y,
                pen,
                c,
                params,
            );
            pen + advance
        }
        ResolvedDraw::Derived {
            face,
            glyph,
            unit,
            kind,
            dm,
        } => {
            let left = line_x + pen + i64::from(dm.a);
            let top = base_y - i64::from(dm.baseline) + i64::from(dm.t);
            match pass {
                Pass::Outline => {
                    let colour = params.outline_colour;
                    if face.background().is_some() {
                        // §8.2 + §6.4.4: the derived bordered box from the background atlas.
                        if let Some(cov) = derived_cover(sd, &face, &glyph, unit, kind, &dm, true) {
                            let bd = i64::from(dm.outline_border).max(0);
                            runs.glyph(
                                colour,
                                &cov,
                                GlyphBox {
                                    dest: [sat32(left - bd), sat32(top - bd)],
                                    size: [cov.width(), cov.height()],
                                    src: [0, 0],
                                },
                            );
                        }
                    } else if dm.w > 0 && dm.h > 0 {
                        // §8.3 + §6.4.4: eight stamps of the derived fill at radius max(1, round(c)).
                        if let Some(cov) = derived_cover(sd, &face, &glyph, unit, kind, &dm, false)
                        {
                            let r = stamp_radius(sd.ctx.c);
                            for dy in -1..=1i64 {
                                for dx in -1..=1i64 {
                                    if dx == 0 && dy == 0 {
                                        continue;
                                    }
                                    runs.glyph(
                                        colour,
                                        &cov,
                                        GlyphBox {
                                            dest: [sat32(left + dx * r), sat32(top + dy * r)],
                                            size: [cov.width(), cov.height()],
                                            src: [0, 0],
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                Pass::Fill => {
                    if let Some(ucol) = c.underline {
                        // §8.5: the underline uses the derived advance, like the unscaled path.
                        let t = i64::from(underline_thickness(line.height, line.leading));
                        let stored = advance.clamp(0, 255);
                        let right = line_x + pen + stored + if res.b < 0 { -res.b } else { 0 };
                        let rect = Rect::new(
                            sat32(line_x + pen),
                            sat32(base_y + t),
                            sat32(right + 1),
                            sat32(base_y + 2 * t),
                        );
                        runs.underline(FillRect {
                            rect,
                            clip: params.clip,
                            colour: ucol,
                        });
                    }
                    if dm.w > 0
                        && dm.h > 0
                        && let Some(cov) = derived_cover(sd, &face, &glyph, unit, kind, &dm, false)
                    {
                        runs.glyph(
                            c.colour,
                            &cov,
                            GlyphBox {
                                dest: [sat32(left), sat32(top)],
                                size: [cov.width(), cov.height()],
                                src: [0, 0],
                            },
                        );
                    }
                }
            }
            pen + advance
        }
    }
}

/// Draws a laid-out text (`fonts.md` §8): the outline pass (when `params.outline`), then
/// the fill pass with underlines; truncated text switches to the suffix at the cut index
/// and stops there (§7.6 step 8).
///
/// `chars` and `fonts` must be those given to [`super::layout`]; characters the layout does
/// not cover are ignored.
#[must_use]
pub fn draw(
    layout: &TextLayout,
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &DrawParams,
) -> TextPaint {
    draw_impl(layout, chars, fonts, params, None)
}

/// Draws a laid-out text at a content scale (`ui-scaling.md` §6.4), rasterising each glyph at its
/// physical height from the font ladders in `scale` and caching derived glyphs in `cache` (§6.5).
/// At `scale.is_identity()` the result is byte-identical to [`draw`] (invariant I7). `layout`
/// should be the matching [`super::layout_scaled`] output. Deriving a glyph counts toward the
/// frame's paint slice (§6.5).
#[must_use]
pub fn draw_scaled(
    layout: &TextLayout,
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &DrawParams,
    scale: &ScaleCtx<'_>,
    cache: &mut GlyphCache,
) -> TextPaint {
    if scale.is_identity() {
        return draw(layout, chars, fonts, params);
    }
    let sd = ScaleDraw {
        ctx: scale,
        cache: RefCell::new(cache),
        arcs: RefCell::new(HashMap::new()),
    };
    draw_impl(layout, chars, fonts, params, Some(&sd))
}

fn draw_impl(
    layout: &TextLayout,
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &DrawParams,
    scale: Option<&ScaleDraw<'_>>,
) -> TextPaint {
    let mut runs = Runs {
        ops: Vec::new(),
        cur: None,
        clip: params.clip,
        open_underline: None,
    };
    // A bound on how far anything a line draws reaches above its top and below its
    // baseline (glyph tops and heights are within ±128 and 255, outline borders and the
    // stamp add at most 2 × border + 1, the bullet its height, the underline 2 t below the
    // baseline): lines outside the clip by more than this are skipped without walking
    // their characters. Culling inside makes the result the same either way.
    let (max_baseline, max_border) = fonts.iter().flatten().fold((0i64, 0i64), |(b, r), f| {
        (
            b.max(i64::from(f.baseline().min(65_535))),
            r.max(i64::from(f.borders()[1])),
        )
    });
    let reach = 512 + max_baseline + 2 * max_border + params.bullet.map_or(0, |b| i64::from(b[1]));
    // Derived glyphs at a content scale > 1 reach further above and below the line than the
    // native ±128 top; widen the cull bound by the scale so no visible glyph is skipped (a larger
    // bound only culls less, never wrongly). The unscaled path keeps the exact native bound (I7).
    let reach = match scale {
        None => reach,
        Some(sd) => {
            let f = (sd.ctx.c * sd.ctx.x).max(1.0);
            ((reach as f64) * f).ceil() as i64
        }
    };
    let (clip_y0, clip_y1) = (i64::from(params.clip.y0), i64::from(params.clip.y1));
    let origin = params.origin.map(i64::from);
    let cut = layout.truncation.as_ref().map(|t| (t.line, t.cut));
    let list_extra = i64::from(layout.list_extra);
    let passes: &[Pass] = if params.outline {
        &[Pass::Outline, Pass::Fill]
    } else {
        &[Pass::Fill]
    };
    for &pass in passes {
        runs.open_underline = None;
        for (li, line) in layout.lines.iter().enumerate() {
            if cut.is_some_and(|(cl, _)| li > cl) {
                break;
            }
            let top = origin[1] + i64::from(line.top);
            let t = i64::from(underline_thickness(line.height, line.leading));
            if top + i64::from(line.baseline) + reach + 2 * t <= clip_y0 || top - reach >= clip_y1 {
                continue;
            }
            let end = match cut {
                Some((cl, ci)) if cl == li => ci.clamp(line.start, line.end.max(line.start)),
                _ => line.end,
            };
            let slice = chars.get(line.start..end.min(chars.len())).unwrap_or(&[]);
            let pen = draw_chars(
                &mut runs,
                pass,
                slice.iter(),
                fonts,
                line,
                origin,
                i64::from(line.indent),
                params,
                list_extra,
                true,
                scale,
            );
            if let Some(t) = &layout.truncation
                && t.line == li
                && t.cut < chars.len()
            {
                // §7.6 step 8: the suffix continues on the same line from the pen; its first
                // character is the line's first drawn character only when nothing is cut
                // before it.
                let first = slice.is_empty();
                draw_chars(
                    &mut runs,
                    pass,
                    t.suffix.iter(),
                    fonts,
                    line,
                    origin,
                    pen,
                    params,
                    list_extra,
                    first,
                    scale,
                );
            }
        }
    }
    runs.flush();
    TextPaint {
        ops: runs.ops,
        clip: params.clip,
    }
}

#[cfg(test)]
mod scaled_tests {
    use super::*;
    use crate::text::{FontId, FontLadder, LayoutParams, ScaleCtx, layout, layout_scaled};

    fn glyph(code: u16, x: u16, y: u16, w: u8, h: u8) -> Glyph {
        Glyph {
            width: w,
            height: h,
            left_bearing: 1,
            right_bearing: 1,
            top: 2,
            code,
            x,
            y,
        }
    }

    /// A face with a uniform-coverage foreground atlas and one glyph.
    fn face(did: u32, line_height: u32, atlas: u32, g: Glyph) -> Arc<FontFace> {
        let fg = Arc::new(
            CoverageMap::new(atlas, atlas, vec![255u8; (atlas * atlas) as usize]).unwrap(),
        );
        Arc::new(
            FontFace::from_parts(
                FontId::Data(did),
                line_height,
                line_height.saturating_sub(2),
                [0, 0],
                vec![g],
                fg,
                None,
            )
            .unwrap(),
        )
    }

    fn fill_run(tp: &TextPaint) -> Option<&GlyphRun> {
        tp.ops.iter().find_map(|op| match op {
            TextOp::Glyphs(r) => Some(r),
            _ => None,
        })
    }

    fn text(unit: u16) -> Vec<TextChar> {
        vec![TextChar::plain(unit, 0, 0xFFFF_FFFF)]
    }

    fn params() -> (LayoutParams, DrawParams) {
        let lp = LayoutParams {
            width: 1 << 20,
            height: 1 << 20,
            ..LayoutParams::default()
        };
        (lp, DrawParams::default())
    }

    /// Invariant I7: the scaled path at `c × x = 1` is byte-identical to the unscaled path.
    #[test]
    fn identity_matches_unscaled() {
        let f = face(1, 10, 8, glyph(0x41, 0, 0, 4, 6));
        let fonts = vec![Some(Arc::clone(&f))];
        let cs = text(0x41);
        let (lp, dp) = params();
        let base = layout(&cs, &fonts, &lp, None).unwrap();
        let ladders = [FontLadder::single(Arc::clone(&f))];
        let ctx = ScaleCtx {
            c: 1.0,
            x: 1.0,
            ladders: &ladders,
        };
        let scaled = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
        assert_eq!(base, scaled, "layout differs at identity");
        let base_tp = draw(&base, &cs, &fonts, &dp);
        let mut cache = GlyphCache::new(1 << 20);
        let scaled_tp = draw_scaled(&scaled, &cs, &fonts, &dp, &ctx, &mut cache);
        assert_eq!(
            format!("{base_tp:?}"),
            format!("{scaled_tp:?}"),
            "draw differs at identity"
        );
    }

    /// An exact ladder hit (§6.4.3 item 1) draws natively from the sibling record's own atlas.
    #[test]
    fn exact_ladder_hit_uses_sibling_atlas() {
        let small = face(1, 10, 8, glyph(0x41, 0, 0, 4, 6));
        let big = face(2, 20, 32, glyph(0x41, 3, 5, 9, 13));
        let fonts = vec![Some(Arc::clone(&small))];
        let ladders = [FontLadder::new(vec![Arc::clone(&small), Arc::clone(&big)])];
        let ctx = ScaleCtx {
            c: 2.0,
            x: 1.0,
            ladders: &ladders,
        };
        let cs = text(0x41);
        let (lp, dp) = params();
        let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
        // The advance is the 20-record glyph's own (1 + 9 + 1), not the 10-record's.
        assert_eq!(lay.places[0].advance, 11);
        let mut cache = GlyphCache::new(1 << 20);
        let tp = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
        let run = fill_run(&tp).expect("a fill run");
        // The whole 32×32 sibling atlas is the coverage, read at the sibling glyph's (3, 5).
        assert_eq!(run.coverage.width(), 32);
        assert_eq!(run.glyphs[0].size, [9, 13]);
        assert_eq!(run.glyphs[0].src, [3, 5]);
        // Nothing derived: the exact hit never touches the cache.
        assert!(cache.is_empty());
    }

    /// A target above every ladder record derives by the distance method (§6.4.3 item 3), with a
    /// derived coverage box larger than the source and caches the result (§6.5).
    #[test]
    fn distance_derivation_enlarges_and_caches() {
        let f = face(1, 10, 8, glyph(0x41, 0, 0, 4, 6));
        let fonts = vec![Some(Arc::clone(&f))];
        let ladders = [FontLadder::single(Arc::clone(&f))];
        let ctx = ScaleCtx {
            c: 2.0,
            x: 1.0,
            ladders: &ladders,
        };
        let cs = text(0x41);
        let (lp, dp) = params();
        let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
        assert!(
            lay.places[0].advance > 11,
            "advance should grow with the scale"
        );
        let mut cache = GlyphCache::new(1 << 20);
        let tp = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
        let run = fill_run(&tp).expect("a fill run");
        // Derived coverage is its own box drawn 1 : 1 (src [0,0]), larger than the 4×6 source.
        assert_eq!(run.glyphs[0].src, [0, 0]);
        assert!(run.coverage.width() > 4 && run.coverage.height() > 6);
        assert_eq!(
            [run.coverage.width(), run.coverage.height()],
            run.glyphs[0].size
        );
        assert!(!cache.is_empty(), "the derived glyph should be cached");
    }

    /// A target below the smallest ladder record downsamples (§6.4.3 item 2): a smaller box.
    #[test]
    fn downsample_derivation_shrinks() {
        let f = face(1, 20, 32, glyph(0x41, 0, 0, 10, 14));
        let fonts = vec![Some(Arc::clone(&f))];
        let ladders = [FontLadder::single(Arc::clone(&f))];
        let ctx = ScaleCtx {
            c: 0.5,
            x: 1.0,
            ladders: &ladders,
        };
        let cs = text(0x41);
        let (lp, dp) = params();
        let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
        let mut cache = GlyphCache::new(1 << 20);
        let tp = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
        let run = fill_run(&tp).expect("a fill run");
        assert!(run.coverage.width() < 10 && run.coverage.height() < 14);
        assert_eq!(run.glyphs[0].src, [0, 0]);
    }

    /// A face whose glyph `A` is a solid `w × h` rectangle of full coverage at (0, 0) of an
    /// atlas just large enough, for the T1–T3 checks on the wired path.
    fn solid_face(did: u32, line_height: u32, w: u8, h: u8) -> Arc<FontFace> {
        pattern_face(did, line_height, w, h, |_, _| 255)
    }

    /// As [`solid_face`] with the coverage of texel (x, y) given by `cov`.
    fn pattern_face(
        did: u32,
        line_height: u32,
        w: u8,
        h: u8,
        cov: impl Fn(u32, u32) -> u8,
    ) -> Arc<FontFace> {
        let (aw, ah) = (u32::from(w), u32::from(h));
        let data = (0..ah)
            .flat_map(|y| (0..aw).map(move |x| (x, y)))
            .map(|(x, y)| cov(x, y))
            .collect();
        let fg = Arc::new(CoverageMap::new(aw, ah, data).unwrap());
        let g = Glyph {
            width: w,
            height: h,
            left_bearing: 0,
            right_bearing: 0,
            top: 0,
            code: 0x41,
            x: 0,
            y: 0,
        };
        Arc::new(
            FontFace::from_parts(
                FontId::Data(did),
                line_height,
                line_height.saturating_sub(1),
                [0, 0],
                vec![g],
                fg,
                None,
            )
            .unwrap(),
        )
    }

    /// The fill coverage `draw_scaled` produces for one `A` at content scale `c`.
    fn derived_fill(f: &Arc<FontFace>, c: f64) -> (Vec<u8>, usize, usize) {
        let fonts = vec![Some(Arc::clone(f))];
        let ladders = [FontLadder::single(Arc::clone(f))];
        let ctx = ScaleCtx {
            c,
            x: 1.0,
            ladders: &ladders,
        };
        let cs = text(0x41);
        let (lp, dp) = params();
        let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
        let mut cache = GlyphCache::new(1 << 20);
        let tp = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
        let run = fill_run(&tp).expect("a fill run");
        let cov = &run.coverage;
        (
            cov.data().to_vec(),
            cov.width() as usize,
            cov.height() as usize,
        )
    }

    /// Invariant I7 on the wired path: at `c × x = 1` every glyph comes from the font's own
    /// record (its own atlas, nothing derived); at a height with an exact ladder record, from
    /// that record's atlas.
    #[test]
    fn i7_glyphs_come_from_the_records() {
        let small = face(1, 10, 8, glyph(0x41, 0, 0, 4, 6));
        let big = face(2, 15, 16, glyph(0x41, 1, 1, 6, 9));
        let fonts = vec![Some(Arc::clone(&small))];
        let ladders = [FontLadder::new(vec![Arc::clone(&small), Arc::clone(&big)])];
        let cs = text(0x41);
        let (lp, dp) = params();
        for (c, x, want) in [(1.0, 1.0, &small), (0.8, 1.25, &small), (1.5, 1.0, &big)] {
            let ctx = ScaleCtx {
                c,
                x,
                ladders: &ladders,
            };
            let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
            let mut cache = GlyphCache::new(1 << 20);
            let tp = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
            let run = fill_run(&tp).expect("a fill run");
            assert!(
                Arc::ptr_eq(&run.coverage, want.foreground()),
                "c {c} x {x}: the glyph is not the record's own"
            );
            assert!(cache.is_empty(), "c {c} x {x}: nothing is derived");
        }
    }

    /// Ink box of a coverage bitmap: (left, top, right, bottom), exclusive ends.
    fn ink(cov: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
        let mut b: Option<(usize, usize, usize, usize)> = None;
        for y in 0..h {
            for x in 0..w {
                if cov[y * w + x] > 0 {
                    b = Some(match b {
                        None => (x, y, x + 1, y + 1),
                        Some((l, t, r, bt)) => (l.min(x), t.min(y), r.max(x + 1), bt.max(y + 1)),
                    });
                }
            }
        }
        b
    }

    /// T1: a solid source rectangle stays a solid rectangle with at most one partial row or
    /// column per edge, downsampled and distance-derived, through `draw_scaled`.
    #[test]
    fn t1_solid_rectangles_stay_solid() {
        let f = solid_face(1, 20, 9, 13);
        for c in [0.55, 0.75, 1.3, 1.75, 2.5] {
            let (cov, w, h) = derived_fill(&f, c);
            let (l, t, r, b) = ink(&cov, w, h).expect("some ink");
            for y in t..b {
                for x in l..r {
                    let edge = x == l || x + 1 == r || y == t || y + 1 == b;
                    if !edge {
                        assert_eq!(cov[y * w + x], 255, "c {c}: interior ({x}, {y}) not full");
                    }
                }
            }
            // At most one partial column on the left and right, one partial row top and bottom.
            // A column is partial when one of its pixels between the edge rows is; a row
            // when one of its pixels between the edge columns is.
            let inner_rows = (t + 1)..b.saturating_sub(1);
            let inner_cols = (l + 1)..r.saturating_sub(1);
            let partial_col = |x: usize| inner_rows.clone().any(|y| cov[y * w + x] < 255);
            let partial_row = |y: usize| inner_cols.clone().any(|x| cov[y * w + x] < 255);
            let cols = (l..r).filter(|&x| partial_col(x)).count();
            let rows = (t..b).filter(|&y| partial_row(y)).count();
            assert!(
                cols <= 2 && rows <= 2,
                "c {c}: {cols} partial columns, {rows} rows"
            );
        }
    }

    /// T2: the derived ink box is within one pixel of the source ink box scaled by `k`.
    #[test]
    fn t2_ink_box_follows_k() {
        let f = solid_face(1, 20, 9, 13);
        for c in [0.55, 0.75, 1.3, 1.75, 2.5] {
            let (cov, w, h) = derived_fill(&f, c);
            let (l, t, r, b) = ink(&cov, w, h).expect("some ink");
            let k = f64::from(crate::scaling::target_height(20, c, 1.0)) / 20.0;
            let (iw, ih) = ((r - l) as f64, (b - t) as f64);
            assert!(
                (iw - 9.0 * k).abs() <= 1.0 + 1e-9,
                "c {c}: width {iw} vs {}",
                9.0 * k
            );
            assert!(
                (ih - 13.0 * k).abs() <= 1.0 + 1e-9,
                "c {c}: height {ih} vs {}",
                13.0 * k
            );
        }
    }

    /// T3: a downsampled glyph's coverage sum is the source sum times the box ratio, within
    /// half an 8-bit step per derived pixel plus 1 %.
    #[test]
    fn t3_downsampled_coverage_sum() {
        let pat = |x: u32, y: u32| ((x * 29 + y * 53 + x * y * 7) % 256) as u8;
        let f = pattern_face(1, 20, 9, 13, pat);
        let src: f64 = (0..13u32)
            .flat_map(|y| (0..9u32).map(move |x| (x, y)))
            .map(|(x, y)| f64::from(pat(x, y)) / 255.0)
            .sum();
        for c in [0.5, 0.55, 0.75, 0.9] {
            let (cov, w, h) = derived_fill(&f, c);
            let sum: f64 = cov.iter().map(|&v| f64::from(v) / 255.0).sum();
            let reference = src * (w * h) as f64 / (9.0 * 13.0);
            let tol = (w * h) as f64 / 510.0 + 0.01 * reference;
            assert!(
                (sum - reference).abs() <= tol,
                "c {c}: sum {sum} vs {reference} (tolerance {tol})"
            );
        }
    }

    /// A hostile content scale cannot panic or allocate unboundedly: a target above the coverage
    /// limits simply draws nothing.
    #[test]
    fn hostile_scale_no_panic() {
        let f = face(1, 10, 8, glyph(0x41, 0, 0, 4, 6));
        let fonts = vec![Some(Arc::clone(&f))];
        let ladders = [FontLadder::single(Arc::clone(&f))];
        let cs = text(0x41);
        let (lp, dp) = params();
        for c in [1e9, f64::NAN, -2.0, 1e-9] {
            let ctx = ScaleCtx {
                c,
                x: 1.0,
                ladders: &ladders,
            };
            let lay = layout_scaled(&cs, &fonts, &lp, None, &ctx).unwrap();
            let mut cache = GlyphCache::new(1 << 16);
            let _ = draw_scaled(&lay, &cs, &fonts, &dp, &ctx, &mut cache);
        }
    }
}
