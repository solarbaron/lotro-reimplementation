// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure helpers of the scaled-text path (`ui-scaling.md` §6.4.5, §6.4.6, §7.1, §7.5).
//!
//! The crisp text path lays text out **in physical pixels** with glyphs chosen or derived for
//! the size they are shown at ([`super::layout_scaled`], [`super::draw_scaled`]). This module
//! holds the parts of that path that do not need the element tree:
//!
//! - [`UiTextSource`]: the `ui_text_source` option (§6.4.3 item 4, §7.1);
//! - [`physical_params`]: an element's layout parameters converted from layout units to
//!   physical pixels by the element's own edge mapping (§6.2, §6.4.5);
//! - [`relayout_lines`]: a physical layout that keeps the line breaks of an existing layout,
//!   used when the layout parameters of an element are not known (see its docs);
//! - [`TextLayout::hit`] and [`mouse_to_text_physical`]: the physical text hit test (§6.4.6);
//! - [`ceil_div_scale`]: a physical text extent taken back to UI units (§6.4.5).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;

use super::chars::TextChar;
use super::layout::{JUSTIFY_LEFT, JUSTIFY_TOP, LayoutParams, Margins, ScaleCtx, TextLayout};
use super::{FontFace, TextError, layout_scaled};
use crate::paint::{CoverageMap, GlyphBox, GlyphRun, MAX_COVERAGE_DIM, PaintOp, Rect};

/// Reads an element's layout parameters (`fonts.md` §7) for the crisp scaled-text path
/// (`ui-scaling.md` §6.4.5): from the element's effective properties, its layout-box size and,
/// when measuring, a wrap width (as `TextRequest::wrap_width`). The client's shaper builds the
/// same parameters for `UiEnv::shape_text`; registering its reader with
/// `Ui::set_text_layout_params` lets the UI lay a text out again in physical pixels (and with
/// the text factor of §7.5) instead of keeping the shaper's line breaks ([`relayout_lines`]).
pub type TextParamsReader =
    Box<dyn Fn(&[(u32, PropertyValue)], (i32, i32), Option<i32>) -> LayoutParams>;

/// Default byte budget of the derived-glyph cache (`ui-scaling.md` §6.5, §8 S6: it shares the
/// decoded-image budget; the client sets its share with `Ui::set_glyph_cache_budget`).
pub const DEFAULT_GLYPH_CACHE_BYTES: usize = 8 << 20;

/// Smallest and largest text factor `ui_text_scale` (`ui-scaling.md` §7.1, §7.5).
pub const TEXT_SCALE_RANGE: (f64, f64) = (0.80, 1.50);

/// Most coverage bytes one stretched text may hold ([`stretch_text_ops`]; Tessera bound).
const MAX_STRETCH_BYTES: u64 = 16 << 20;

/// Width of the shelves the stretched glyphs are packed into (Tessera choice).
const STRETCH_SHELF: i64 = 2_048;

/// The `ui_text_source` option (`ui-scaling.md` §6.4.3 item 4, §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiTextSource {
    /// The game's fonts, chosen or derived for the target height (§6.4.3 items 1–3). The
    /// default.
    #[default]
    Game,
    /// Every game font replaced by Tessera's bundled font rasterised directly at the target
    /// height (decision 0022, `fonts.md` §11 item 4); layout then uses that font's metrics.
    Tessera,
}

impl UiTextSource {
    /// The option for a settings value (`"game"` or `"tessera"`, ASCII case ignored; `None`
    /// for anything else).
    #[must_use]
    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "game" => Some(Self::Game),
            "tessera" => Some(Self::Tessera),
            _ => None,
        }
    }

    /// The settings value.
    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Tessera => "tessera",
        }
    }
}

/// Converts an element's layout parameters (layout units) to physical pixels (`ui-scaling.md`
/// §6.4.5: "Inputs in layout units (the element's width, margins, indents, the bullet offset)
/// are converted to physical pixels by §6.2").
///
/// `fx(v)` and `fy(v)` give the physical offset of the layout offset `v` from the element's
/// left (top) edge, by the element's own edge mapping, so a margin or width maps exactly as
/// the element's box does: the width is `fx(width)`, the left margin `fx(left)`, the right
/// margin `fx(width) − fx(width − right)`, and likewise vertically. `c` is the content scale
/// used for the list-item extra amount (a bullet image width plus spacing, rounded). The
/// leading factor, justification and flags are unchanged.
#[must_use]
pub fn physical_params(
    p: &LayoutParams,
    fx: impl Fn(i32) -> i32,
    fy: impl Fn(i32) -> i32,
    c: f64,
) -> LayoutParams {
    let (w, h) = (p.width, p.height);
    let mg = p.margins;
    let c = if c.is_finite() && c > 0.0 { c } else { 1.0 };
    let extra = (f64::from(p.list_extra) * c).round();
    LayoutParams {
        width: fx(w),
        height: fy(h),
        margins: Margins {
            left: fx(mg.left),
            right: fx(w).saturating_sub(fx(w.saturating_sub(mg.right))),
            top: fy(mg.top),
            bottom: fy(h).saturating_sub(fy(h.saturating_sub(mg.bottom))),
        },
        list_extra: extra.clamp(-65_536.0, 65_536.0) as i32,
        ..p.clone()
    }
}

/// Rounds `v × c` to the nearest integer, saturating (a hostile `c` gives `v`).
fn scale_round(v: i32, c: f64) -> i32 {
    if !c.is_finite() || c <= 0.0 {
        return v;
    }
    (f64::from(v) * c)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// A physical layout that keeps the **line breaks** of `unit` (a layout in layout units of the
/// same characters), for elements whose layout parameters are not known to the UI.
///
/// The UI receives a text element's text already laid out by the client's shaper
/// ([`crate::PreparedText`]), without the margins, justification and leading it was laid out
/// with. When no layout-parameter reader is registered ([`crate::Ui::set_text_layout_params`])
/// the crisp path cannot lay the text out again from scratch, so it keeps each line of `unit`:
/// the line's characters are laid out alone in physical pixels with the chosen or derived
/// metrics of `scale` (§6.4.3–§6.4.5), and the line is placed where `unit` placed it, mapped by
/// `fx` and `fy` (the element's edge mapping, as in [`physical_params`]). Glyphs are crisp and
/// whole-pixel; the breaks are those at the text factor 1 (so the text factor of §7.5, which
/// needs a new layout, is not applied on this path) and a centred or right-aligned line may sit
/// up to a pixel off the place a full layout would give it. `c` scales the leading and the
/// list-item extra amount.
///
/// # Errors
/// [`TextError::TextTooLong`] as [`super::layout`].
#[allow(clippy::too_many_arguments)]
pub fn relayout_lines(
    unit: &TextLayout,
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    scale: &ScaleCtx<'_>,
    fx: impl Fn(i32) -> i32,
    fy: impl Fn(i32) -> i32,
    c: f64,
) -> Result<TextLayout, TextError> {
    let list_extra = scale_round(unit.list_extra, c);
    let line_params = LayoutParams {
        width: i32::MAX / 4,
        height: i32::MAX / 4,
        margins: Margins::default(),
        one_line: true,
        horizontal: JUSTIFY_LEFT,
        vertical: JUSTIFY_TOP,
        leading: 0.0,
        last_line_leading: false,
        list_extra,
    };
    let mut out = TextLayout {
        lines: Vec::with_capacity(unit.lines.len()),
        places: vec![super::CharPlace::default(); chars.len()],
        text_height: fy(unit.text_height),
        content_width: 0,
        content_height: 0,
        list_extra,
        truncation: None,
    };
    let mut widest_unit = 0i32;
    let mut widest = 0i32;
    for (li, line) in unit.lines.iter().enumerate() {
        let end = line.end.min(chars.len());
        let start = line.start.min(end);
        let slice = chars.get(start..end).unwrap_or(&[]);
        // A trailing newline adds the extra line of §7.1 case 2 in one-line mode; the unit
        // layout already holds that line, so it is left out here.
        let body = match slice.split_last() {
            Some((last, rest)) if last.is_newline() && !rest.is_empty() => rest,
            _ => slice,
        };
        let mut lb = *line;
        lb.x = fx(line.x);
        lb.top = fy(line.top);
        lb.leading = scale_round(line.leading, c);
        if body.is_empty() || line.height == 0 {
            lb.height = scale_round(line.height, c);
            lb.baseline = scale_round(line.baseline, c);
            lb.width = scale_round(line.width, c);
            lb.indent = scale_round(line.indent, c);
        } else {
            let sub = layout_scaled(body, fonts, &line_params, None, scale)?;
            if let Some(first) = sub.lines.first() {
                lb.height = first.height;
                lb.baseline = first.baseline;
                lb.width = first.width;
                lb.indent = first.indent;
            }
            let li32 = u32::try_from(li).unwrap_or(u32::MAX);
            for (k, p) in sub.places.iter().enumerate() {
                if let Some(slot) = out.places.get_mut(start + k) {
                    *slot = super::CharPlace { line: li32, ..*p };
                }
            }
        }
        widest_unit = widest_unit.max(line.width);
        widest = widest.max(lb.width);
        out.lines.push(lb);
    }
    for l in &unit.lines {
        if let Some(ci) = l.consumed
            && let Some(slot) = out.places.get_mut(ci)
        {
            slot.consumed = true;
        }
    }
    // Margins are what the unit extent holds beyond its widest line and its text height.
    let margins_w = unit.content_width.saturating_sub(widest_unit).max(0);
    let margins_h = unit.content_height.saturating_sub(unit.text_height).max(0);
    out.content_width = fx(margins_w).saturating_add(widest);
    out.content_height = fy(margins_h).saturating_add(out.text_height);
    if let Some(t) = &unit.truncation {
        let sw = layout_scaled(&t.suffix, fonts, &line_params, None, scale)?
            .lines
            .first()
            .map_or(0, |l| l.width);
        out.truncation = Some(super::Truncation {
            suffix_width: sw,
            ..t.clone()
        });
    }
    Ok(out)
}

impl TextLayout {
    /// The character at a point in the layout's coordinates (`ui-scaling.md` §6.4.6: for the
    /// physical layout of the crisp path, the pointer's physical pixel minus the element's
    /// physical origin plus the scroll offsets in physical pixels, see
    /// [`mouse_to_text_physical`]; for a layout-unit layout, `fonts.md` §6.3 item 3).
    ///
    /// The line is the one whose rows `[top, top + height + leading)` hold `y`; the character
    /// the one whose cell `[x + pen, x + pen + advance)` holds `x`. `None` when the point is on
    /// no line or no character (a consumed character, §7.3, is never hit).
    #[must_use]
    pub fn hit(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (i64::from(x), i64::from(y));
        let li = self.lines.iter().position(|l| {
            let top = i64::from(l.top);
            let bottom = top + i64::from(l.height).max(0) + i64::from(l.leading).max(0);
            y >= top && y < bottom
        })?;
        let line = self.lines.get(li)?;
        let lx = i64::from(line.x);
        (line.start..line.end.min(self.places.len())).find(|&k| {
            self.places.get(k).is_some_and(|p| {
                let left = lx + i64::from(p.pen);
                !p.consumed && x >= left && x < left + i64::from(p.advance).max(0)
            })
        })
    }
}

/// The physical text hit-test conversion (`ui-scaling.md` §6.4.6): the pointer's physical
/// pixel minus the element's physical box origin plus the scroll offsets converted to physical
/// pixels. No division by a scale is needed. Saturating.
#[must_use]
pub fn mouse_to_text_physical(pointer: [i32; 2], origin: [i32; 2], scroll: [i32; 2]) -> [i32; 2] {
    [
        pointer[0]
            .saturating_sub(origin[0])
            .saturating_add(scroll[0]),
        pointer[1]
            .saturating_sub(origin[1])
            .saturating_add(scroll[1]),
    ]
}

/// A physical text extent taken back to UI units (`ui-scaling.md` §6.4.5: "the extent in UI
/// units is the physical extent ÷ c, rounded up"). A hostile `c` gives the extent unchanged.
#[must_use]
pub fn ceil_div_scale(physical: i32, c: f64) -> i32 {
    if !c.is_finite() || c <= 0.0 {
        return physical;
    }
    let v = f64::from(physical) / c;
    // A tiny tolerance keeps an exact quotient (for example 30 / 1.5) from rounding up past
    // itself through floating error.
    (v - 1e-9)
        .ceil()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// The text of a stretched surface (`ui-scaling.md` §6.1, §6.2, §5.5 last paragraph): a
/// stretch vector *σ* = (`sigma`) is a picture stretch, so text is laid out and drawn at the
/// uniform content scale and the drawn result is stretched by *σ* around the text origin
/// `origin` (canvas pixels): every glyph box, underline and coverage is resampled per axis
/// (bilinear, clamped to its glyph box) into a new coverage map, and every operation gets the
/// clip `clip` (the clip in stretched canvas pixels). This is the picture stretch §6.1 applies
/// at composition, done here because Tessera's canvas has the stretched size. `None` when the
/// stretched glyphs would not fit one coverage map (the caller keeps the unstretched text).
#[must_use]
pub(crate) fn stretch_text_ops(
    ops: &[PaintOp],
    origin: [i32; 2],
    sigma: [f64; 2],
    clip: Rect,
) -> Option<Vec<PaintOp>> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    if !ok(sigma[0]) || !ok(sigma[1]) {
        return None;
    }
    let map = |v: i32, o: i32, s: f64| -> i32 {
        let d = (f64::from(v) - f64::from(o)) * s;
        (f64::from(o) + d)
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    };
    let map_rect = |r: Rect| -> Rect {
        Rect::new(
            map(r.x0, origin[0], sigma[0]),
            map(r.y0, origin[1], sigma[1]),
            map(r.x1, origin[0], sigma[0]),
            map(r.y1, origin[1], sigma[1]),
        )
    };
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            PaintOp::Text(run) => out.push(PaintOp::Text(Arc::new(stretch_run(
                run, &map, origin, sigma, clip,
            )?))),
            PaintOp::Fill(f) => {
                let mut f = *f;
                f.rect = map_rect(f.rect);
                f.clip = clip;
                out.push(PaintOp::Fill(f));
            }
            other => out.push(other.clone()),
        }
    }
    Some(out)
}

/// One glyph run of [`stretch_text_ops`].
fn stretch_run(
    run: &GlyphRun,
    map: &impl Fn(i32, i32, f64) -> i32,
    origin: [i32; 2],
    sigma: [f64; 2],
    clip: Rect,
) -> Option<GlyphRun> {
    struct Placed {
        src: GlyphBox,
        dest: [i32; 2],
        size: [i64; 2],
        at: [i64; 2],
    }
    let mut placed = Vec::with_capacity(run.glyphs.len());
    let (mut x, mut y, mut shelf_h, mut width) = (0i64, 0i64, 0i64, 1i64);
    for g in &run.glyphs {
        let x0 = map(g.dest[0], origin[0], sigma[0]);
        let y0 = map(g.dest[1], origin[1], sigma[1]);
        let x1 = map(
            g.dest[0].saturating_add(i32::try_from(g.size[0]).unwrap_or(i32::MAX)),
            origin[0],
            sigma[0],
        );
        let y1 = map(
            g.dest[1].saturating_add(i32::try_from(g.size[1]).unwrap_or(i32::MAX)),
            origin[1],
            sigma[1],
        );
        let (w, h) = (
            (i64::from(x1) - i64::from(x0)).max(0),
            (i64::from(y1) - i64::from(y0)).max(0),
        );
        if w == 0 || h == 0 || g.size[0] == 0 || g.size[1] == 0 {
            continue;
        }
        if x > 0 && x + w > STRETCH_SHELF {
            x = 0;
            y += shelf_h;
            shelf_h = 0;
        }
        placed.push(Placed {
            src: *g,
            dest: [x0, y0],
            size: [w, h],
            at: [x, y],
        });
        x += w;
        width = width.max(x);
        shelf_h = shelf_h.max(h);
    }
    let height = (y + shelf_h).max(1);
    let max = i64::from(MAX_COVERAGE_DIM);
    if width > max || height > max || (width * height) as u64 > MAX_STRETCH_BYTES {
        tracing::debug!(
            width,
            height,
            "stretched text does not fit one coverage map"
        );
        return None;
    }
    let (wu, hu) = (width as usize, height as usize);
    let mut data = vec![0u8; wu * hu];
    let cov = &*run.coverage;
    let mut glyphs = Vec::with_capacity(placed.len());
    for p in &placed {
        let (sw, sh) = (i64::from(p.src.size[0]), i64::from(p.src.size[1]));
        let (rx, ry) = (sw as f64 / p.size[0] as f64, sh as f64 / p.size[1] as f64);
        let tap = |v: f64, n: i64| -> (i64, i64, f64) {
            let v = v.clamp(0.0, (n - 1) as f64);
            let i = v.floor() as i64;
            (i, (i + 1).min(n - 1), v - i as f64)
        };
        for j in 0..p.size[1] {
            let (y0, y1, ty) = tap((j as f64 + 0.5) * ry - 0.5, sh);
            for i in 0..p.size[0] {
                let (x0, x1, tx) = tap((i as f64 + 0.5) * rx - 0.5, sw);
                let at = |xx: i64, yy: i64| -> f64 {
                    f64::from(cov.at(
                        p.src.src[0].saturating_add(xx as i32),
                        p.src.src[1].saturating_add(yy as i32),
                    ))
                };
                let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
                let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
                let v = (top * (1.0 - ty) + bot * ty).round().clamp(0.0, 255.0) as u8;
                let idx = (p.at[1] + j) as usize * wu + (p.at[0] + i) as usize;
                if let Some(d) = data.get_mut(idx) {
                    *d = v;
                }
            }
        }
        glyphs.push(GlyphBox {
            dest: p.dest,
            size: [p.size[0] as u32, p.size[1] as u32],
            src: [p.at[0] as i32, p.at[1] as i32],
        });
    }
    let coverage = CoverageMap::new(width as u32, height as u32, data).ok()?;
    Some(GlyphRun {
        clip,
        colour: run.colour,
        coverage: Arc::new(coverage),
        glyphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6.1: a stretched text is the unstretched one stretched per axis around its origin.
    #[test]
    fn stretch_doubles_glyph_boxes() {
        // One 2 × 2 glyph at (10, 4) from a 2 × 2 map, origin (6, 4).
        let cov = Arc::new(CoverageMap::new(2, 2, vec![255, 0, 255, 0]).unwrap());
        let run = GlyphRun {
            clip: Rect::new(0, 0, 100, 100),
            colour: 0xFFFF_FFFF,
            coverage: cov,
            glyphs: vec![GlyphBox {
                dest: [10, 4],
                size: [2, 2],
                src: [0, 0],
            }],
        };
        let clip = Rect::new(0, 0, 200, 100);
        let ops = stretch_text_ops(&[PaintOp::Text(Arc::new(run))], [6, 4], [2.0, 1.0], clip)
            .expect("fits");
        let PaintOp::Text(r) = &ops[0] else {
            panic!("a glyph run")
        };
        assert_eq!(r.clip, clip);
        assert_eq!(r.glyphs[0].dest, [14, 4], "6 + (10 − 6) × 2");
        assert_eq!(r.glyphs[0].size, [4, 2]);
        // The left texel column is covered, the right one is not: the stretched left half
        // is mostly covered and the right end is clear.
        let m = &*r.coverage;
        assert_eq!(m.at(0, 0), 255);
        assert_eq!(m.at(3, 0), 0);
        // A hostile stretch is refused.
        assert!(stretch_text_ops(&[], [0, 0], [f64::NAN, 1.0], clip).is_none());
    }

    #[test]
    fn text_source_settings() {
        assert_eq!(UiTextSource::default(), UiTextSource::Game);
        assert_eq!(
            UiTextSource::from_setting("Tessera"),
            Some(UiTextSource::Tessera)
        );
        assert_eq!(UiTextSource::from_setting("game"), Some(UiTextSource::Game));
        assert_eq!(UiTextSource::from_setting("x"), None);
        assert_eq!(UiTextSource::Tessera.as_setting(), "tessera");
    }

    #[test]
    fn params_convert_by_the_edge_map() {
        let p = LayoutParams {
            width: 100,
            height: 40,
            margins: Margins {
                left: 4,
                right: 6,
                top: 2,
                bottom: 3,
            },
            list_extra: 10,
            ..LayoutParams::default()
        };
        let q = physical_params(&p, |v| v * 3 / 2, |v| v * 3 / 2, 1.5);
        assert_eq!(q.width, 150);
        assert_eq!(q.height, 60);
        assert_eq!(q.margins.left, 6);
        assert_eq!(q.margins.right, 150 - 141);
        assert_eq!(q.margins.top, 3);
        assert_eq!(q.margins.bottom, 60 - 55);
        assert_eq!(q.list_extra, 15);
        // The identity map leaves every value unchanged.
        let id = physical_params(&p, |v| v, |v| v, 1.0);
        assert_eq!(id, p);
    }

    #[test]
    fn ceil_div() {
        assert_eq!(ceil_div_scale(30, 1.5), 20);
        assert_eq!(ceil_div_scale(31, 1.5), 21);
        assert_eq!(ceil_div_scale(10, 1.0), 10);
        assert_eq!(ceil_div_scale(10, f64::NAN), 10);
    }

    #[test]
    fn physical_hit_conversion() {
        assert_eq!(
            mouse_to_text_physical([105, 60], [100, 50], [0, 12]),
            [5, 22]
        );
        assert_eq!(
            mouse_to_text_physical([i32::MIN, 0], [i32::MAX, 0], [0, 0])[0],
            i32::MIN
        );
    }
}
