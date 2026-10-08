// SPDX-License-Identifier: GPL-3.0-or-later
//! Drawing text (`fonts.md` §6.2, §7.5, §7.6 step 8, §8.1–§8.5).
//!
//! [`draw`] walks the lines of a [`TextLayout`] twice, as the client does: first the outline
//! of every glyph (when the outline flag is set), then the fill of every glyph with its
//! underline before it. The result is a list of [`TextOp`]s in canvas coordinates: one-byte
//! glyph coverage runs ([`GlyphRun`]: one colour, one coverage map, boxes copied 1 : 1),
//! underline fills ([`FillRect`], overwriting) and bullet positions. [`crate::paint`] does
//! the per-pixel compositing of §8.4.

use std::sync::Arc;

use tessera_material::ui_image::UiImage;

use super::chars::TextChar;
use super::layout::{LineBox, Metrics, TextLayout, metrics, underline_thickness};
use super::{FontFace, sat32};
use crate::paint::{
    CompositeOp, CoverageMap, DisplayList, FillRect, GlyphBox, GlyphRun, ImageLayer,
    MAX_GLYPHS_PER_RUN, MAX_OPS, PaintError, PaintOp, Rect,
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
        pen += a + w + b;
    }
    pen
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
