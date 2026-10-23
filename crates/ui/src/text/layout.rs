// SPDX-License-Identifier: GPL-3.0-or-later
//! Placement and line layout (`fonts.md` §6–§7).
//!
//! [`layout`] runs the client's one pass over the characters: per-character metrics with the
//! `?` substitution (§5), pen advance without kerning (§6.1), line breaking with the break
//! opportunities and the one consumed space (§7.3), line metrics and leading (§7.1), indent
//! and line width with the trailing value *T* (§7.2), alignment (§7.4), the list-item extra
//! amount (§7.5) and truncation to fit (§7.6). The result is a [`TextLayout`] that
//! [`super::draw`] turns into paint operations.
//!
//! All coordinates are text coordinates of the element: (0, 0) is the element's top-left
//! corner, before any UI scale (§6.3) and before scrolling.

use std::sync::Arc;

use tessera_formats::font::Glyph;

use super::chars::{SLOT_NORMAL, TextChar};
use super::{FontFace, FontLadder, MAX_TEXT_UNITS, TextError, sat32};
use crate::scaling::{
    GlyphMetrics, SourceKind, choose_glyph_source, derive_metrics, target_height,
};

/// Justification value 1 (`Bottom` in enum mapper 0x23000177): the far side on either axis.
pub const JUSTIFY_BOTTOM: u32 = 1;
/// Justification value 2 (`Top`): the near side; the vertical default.
pub const JUSTIFY_TOP: u32 = 2;
/// Justification value 3 (`Center`).
pub const JUSTIFY_CENTER: u32 = 3;
/// Justification value 4 (`Right`): the far side on either axis.
pub const JUSTIFY_RIGHT: u32 = 4;
/// Justification value 5 (`Left`): the near side; the horizontal default.
pub const JUSTIFY_LEFT: u32 = 5;

/// Text margins of an element (`UICore_Text_left_margin` and relatives), in pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Margins {
    /// Left margin.
    pub left: i32,
    /// Right margin.
    pub right: i32,
    /// Top margin.
    pub top: i32,
    /// Bottom margin.
    pub bottom: i32,
}

/// The element properties that layout reads.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutParams {
    /// Element width (its layout box, unscaled; §6.3).
    pub width: i32,
    /// Element height.
    pub height: i32,
    /// Text margins.
    pub margins: Margins,
    /// `UICore_Text_one_line`: wrapping off (§7.3). Default false.
    pub one_line: bool,
    /// `UICore_Text_horizontal_justification` (default [`JUSTIFY_LEFT`]).
    pub horizontal: u32,
    /// `UICore_Text_vertical_justification` (default [`JUSTIFY_TOP`]).
    pub vertical: u32,
    /// Leading factor *f* (§7.1), see [`leading_factor`].
    pub leading: f32,
    /// `UICore_Text_last_line_leading` (default false).
    pub last_line_leading: bool,
    /// The list-item extra amount *X* (§7.5), see [`list_extra`].
    pub list_extra: i32,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            margins: Margins::default(),
            one_line: false,
            horizontal: JUSTIFY_LEFT,
            vertical: JUSTIFY_TOP,
            leading: 0.0,
            last_line_leading: false,
            list_extra: 0,
        }
    }
}

/// The leading factor *f* = *f*₀ + *E*, raised to 0 if negative (`fonts.md` §7.1): `locale`
/// is the locale value *f*₀, `extra` the element's `UICore_Text_extra_leading`. A NaN sum
/// counts as 0.
#[must_use]
pub fn leading_factor(locale: f32, extra: f32) -> f32 {
    let f = locale + extra;
    if f.is_nan() || f < 0.0 { 0.0 } else { f }
}

/// The list-item extra amount *X* (`fonts.md` §7.5): the width of the element's
/// `UICore_Text_bullet_image` (`None` without one) + `UICore_Text_bullet_spacing`.
/// Clamped to ±65,536 (Tessera bound against hostile values).
#[must_use]
pub fn list_extra(bullet_width: Option<u32>, spacing: i32) -> i32 {
    let x = i64::from(bullet_width.unwrap_or(0)) + i64::from(spacing);
    x.clamp(-65_536, 65_536) as i32
}

/// Underline thickness *t* = ⌊(line height + leading) / 20⌋ + 1, at least 1 (`fonts.md`
/// §8.5).
#[must_use]
pub fn underline_thickness(line_height: i32, leading: i32) -> i32 {
    let t = (i64::from(line_height) + i64::from(leading)).div_euclid(20) + 1;
    sat32(t.max(1))
}

/// The text hit-test conversion of `fonts.md` §6.3 item 3: a mouse position relative to the
/// element's displayed box → text coordinates. Divides *x* by the scale's *x* and *y* by
/// its *y* (32-bit floats, no tolerance test; *z* unused), truncates each toward zero and
/// adds the scroll offsets. `scale` is the element's scale: that of the nearest display node
/// at or above it, (1, 1, 1) only when none exists (FN26).
#[must_use]
pub fn mouse_to_text(pos: [f32; 2], scale: [f32; 3], scroll: [i32; 2]) -> [i32; 2] {
    // `as` truncates toward zero and saturates (NaN → 0), which is what hostile scales need.
    let x = (pos[0] / scale[0]) as i32;
    let y = (pos[1] / scale[1]) as i32;
    [x.saturating_add(scroll[0]), y.saturating_add(scroll[1])]
}

/// One line of a [`TextLayout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineBox {
    /// First character index of the line.
    pub start: usize,
    /// One past the last character index (`start == end`: no characters).
    pub end: usize,
    /// The character consumed by the wrap that started this line (§7.3), if any; it is on
    /// neither line but counts towards this line's height and baseline. Exception: when
    /// this line is the empty line of a forced break (§7.1 case 4, every metric 0), the
    /// consumed character counts towards the next line instead (Tessera reading, OPEN in
    /// `fonts.md` §7.3).
    pub consumed: Option<usize>,
    /// Line width (§7.2): pen after the last character − its *T*; includes the indent.
    pub width: i32,
    /// Line height (§7.1).
    pub height: i32,
    /// Line baseline *L* (§7.1).
    pub baseline: i32,
    /// Leading (§7.1); 0 for the last line unless `last_line_leading`.
    pub leading: i32,
    /// Indent (§7.2): 1 − *A* of the first character when *A* < 0, else 0.
    pub indent: i32,
    /// Line start x: left margin + alignment offset (§7.4). The first character's pen is
    /// `x + indent`.
    pub x: i32,
    /// Line top y (§7.1, §7.4).
    pub top: i32,
}

/// Layout values of one character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharPlace {
    /// Index of the line holding the character (the next line for a consumed character).
    pub line: u32,
    /// Whether a wrap consumed the character (§7.3).
    pub consumed: bool,
    /// Layout pen before the character, line-relative, indent included (§7.2).
    pub pen: i32,
    /// Layout advance: *A* + *W* + *B*, plus *X* for a list-item character (§6.1, §7.5).
    pub advance: i32,
    /// The stored advance: *A* + *W* + *B* clamped to 0–255 (§6.1), for underline extents
    /// and the truncation measure.
    pub stored_advance: u8,
    /// The trailing value *T* (§7.2).
    pub trailing: i32,
}

/// Truncation to fit (`fonts.md` §7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Truncation {
    /// The line that is cut.
    pub line: usize,
    /// The cut index: characters from here on are not drawn; the suffix is drawn in their
    /// place. Equal to the text length when nothing is cut (the exception of step 6).
    pub cut: usize,
    /// The suffix width *S* (its line width laid out alone).
    pub suffix_width: i32,
    /// The suffix characters.
    pub suffix: Vec<TextChar>,
}

/// The result of [`layout`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextLayout {
    /// Lines in order; never empty (empty text has one all-zero line, §7.1 case 3).
    pub lines: Vec<LineBox>,
    /// One entry per character of the text.
    pub places: Vec<CharPlace>,
    /// Text height (§7.1).
    pub text_height: i32,
    /// Content width: widest line + left and right margins (§7.4), after truncation.
    pub content_width: i32,
    /// Content height: text height + top and bottom margins (§7.4), after truncation.
    pub content_height: i32,
    /// The list-item extra amount *X* used.
    pub list_extra: i32,
    /// Set when the text was truncated (§7.6 step 3 found it does not fit). The element
    /// marks itself "truncated" for the tooltip when `UICore_Text_auto_tooltip_truncated_text`
    /// is true (step 4).
    pub truncation: Option<Truncation>,
}

impl TextLayout {
    /// Whether the text was truncated.
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.truncation.is_some()
    }
}

/// Metrics of one character as layout sees them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Metrics {
    pub a: i64,
    pub w: i64,
    pub b: i64,
    /// Font line height clamped to 0–255 (§7.1); 0 without a font.
    pub line_height: i64,
    /// Font baseline (0 without a font).
    pub baseline: i64,
    /// The glyph drawn, if any.
    pub glyph: Option<Glyph>,
}

/// Per-character metrics (§5 step 3, §9.1 rule 4).
pub(crate) fn metrics(c: &TextChar, fonts: &[Option<Arc<FontFace>>]) -> Metrics {
    let Some(face) = fonts.get(usize::from(c.font)).and_then(Option::as_ref) else {
        // A slot without a font: the blank 1 × 1 glyph, line height 0, not drawn.
        return Metrics {
            a: 0,
            w: 1,
            b: 0,
            line_height: 0,
            baseline: 0,
            glyph: None,
        };
    };
    let line_height = i64::from(face.line_height().min(255));
    let baseline = i64::from(face.baseline().min(65_535));
    match face.lookup(c.unit).glyph {
        Some(g) => Metrics {
            a: i64::from(g.left_bearing),
            w: i64::from(g.width),
            b: i64::from(g.right_bearing),
            line_height,
            baseline,
            glyph: Some(g),
        },
        None => Metrics {
            a: 0,
            w: 1,
            b: 0,
            line_height,
            baseline,
            glyph: None,
        },
    }
}

/// The content scale and font ladders for the crisp scaled-text path (`ui-scaling.md` §6.4).
///
/// Passed to [`layout_scaled`] and [`super::draw_scaled`]; the ordinary [`layout`]/[`super::draw`]
/// entry points are the identity path (no scaling, byte-identical output, invariant I7). `c` is
/// the element's content scale *c* (§3.4, the screen's effective scale *Se* for an unstretched
/// surface) and `x` the optional text factor (§7.5, 1.0 unless it applies). `ladders` gives, per
/// font slot (index [`TextChar::font`]), the sibling records of that slot's font; a slot with no
/// ladder (empty or out of range) falls back to a one-face ladder of the slot's own face (a font
/// that stands alone, §6.4.3).
#[derive(Debug, Clone, Copy)]
pub struct ScaleCtx<'a> {
    /// Content scale *c* (§3.4).
    pub c: f64,
    /// Text factor *x* (§7.5), 1.0 unless the optional text size applies.
    pub x: f64,
    /// Per-slot font ladders (§6.4.2).
    pub ladders: &'a [FontLadder],
}

impl ScaleCtx<'_> {
    /// Whether this context is the identity (`c × x = 1`): the chosen size, metrics and layout
    /// then equal the unscaled path (invariant I7), so the scaled entry points may take the
    /// native path. `None` factors are treated as the identity.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        let c = if self.c.is_finite() && self.c > 0.0 {
            self.c
        } else {
            1.0
        };
        let x = if self.x.is_finite() && self.x > 0.0 {
            self.x
        } else {
            1.0
        };
        (c * x - 1.0).abs() < f64::EPSILON
    }
}

/// How one character is drawn in the scaled path (`ui-scaling.md` §6.4.3): resolved once and
/// shared by layout (metrics only) and drawing (metrics plus coverage).
#[derive(Debug, Clone)]
pub(crate) enum ResolvedDraw {
    /// No glyph drawn: either the slot has no font, or the font has neither the unit's glyph nor
    /// `?` (§5 step 3). Layout still reserves the blank 1 × 1; drawing advances 0.
    None,
    /// An exact ladder hit (§6.4.3 item 1): draw natively from this face's atlas and glyph.
    Native {
        /// The record serving the glyph (the slot's face, or a sibling at the exact height).
        face: Arc<FontFace>,
        /// The glyph.
        glyph: Glyph,
    },
    /// A derived glyph (§6.4.3 items 2–3, §6.4.4): coverage is downsampled (`kind`
    /// [`SourceKind::Downsampled`]) or distance-derived (`kind` [`SourceKind::Distance`]) from
    /// `face`'s glyph at derivation scale `k`, with the derived metrics `dm`.
    Derived {
        /// The source record the coverage is derived from.
        face: Arc<FontFace>,
        /// The source glyph.
        glyph: Glyph,
        /// The code unit (for the cache key; may be `?` after substitution, matching the glyph).
        unit: u16,
        /// Downsample or distance (the derivation scale `k` is recovered from the metrics).
        kind: SourceKind,
        /// The derived metrics (§6.4.4).
        dm: GlyphMetrics,
    },
}

/// One character resolved for the scaled path: its layout metrics and how it draws.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub a: i64,
    pub w: i64,
    pub b: i64,
    pub line_height: i64,
    pub baseline: i64,
    pub draw: ResolvedDraw,
}

impl Resolved {
    /// The layout metrics this resolution contributes (§6.4.5): layout never reads the glyph, so
    /// it is carried only for the native draw.
    fn to_metrics(&self) -> Metrics {
        let glyph = match &self.draw {
            ResolvedDraw::Native { glyph, .. } => Some(*glyph),
            _ => None,
        };
        Metrics {
            a: self.a,
            w: self.w,
            b: self.b,
            line_height: self.line_height,
            baseline: self.baseline,
            glyph,
        }
    }
}

/// The derived metrics (`ui-scaling.md` §6.4.4) of glyph `g` of `source_face` at derivation
/// scale `k` for target height `t`.
pub(crate) fn derived_metrics_of(
    source_face: &FontFace,
    g: &Glyph,
    k: f64,
    t: i32,
) -> GlyphMetrics {
    let sm = GlyphMetrics {
        a: i32::from(g.left_bearing),
        w: i32::from(g.width),
        b: i32::from(g.right_bearing),
        t: i32::from(g.top),
        h: i32::from(g.height),
        advance: i32::from(g.left_bearing) + i32::from(g.width) + i32::from(g.right_bearing),
        line_height: source_face.line_height_i32(),
        baseline: i32::try_from(source_face.baseline()).unwrap_or(i32::MAX),
        outline_border: i32::try_from(source_face.borders()[0]).unwrap_or(i32::MAX),
    };
    derive_metrics(sm, k, t)
}

/// Resolves one character for the scaled path (`ui-scaling.md` §6.4.1–§6.4.4): picks the target
/// height, the ladder source and, for a derived glyph, the derived metrics. Pure (no cache, no
/// allocation of coverage); [`super::draw_scaled`] derives and caches the coverage.
pub(crate) fn resolve_scaled(
    c: &TextChar,
    fonts: &[Option<Arc<FontFace>>],
    ctx: &ScaleCtx<'_>,
) -> Resolved {
    let slot = usize::from(c.font);
    let Some(slot_face) = fonts.get(slot).and_then(Option::as_ref) else {
        // A slot without a font: the blank 1 × 1 glyph, line height 0, not drawn.
        return Resolved {
            a: 0,
            w: 1,
            b: 0,
            line_height: 0,
            baseline: 0,
            draw: ResolvedDraw::None,
        };
    };
    // The ladder: the supplied siblings, or the face standing alone (§6.4.3).
    let single;
    let ladder: &FontLadder = match ctx.ladders.get(slot) {
        Some(l) if !l.is_empty() => l,
        _ => {
            single = FontLadder::single(Arc::clone(slot_face));
            &single
        }
    };
    let base_lh = slot_face.line_height_i32();
    let t = target_height(base_lh, ctx.c, ctx.x);
    let (src, source_face) = match ladder.direct_face(t) {
        // A face made for this height (§6.4.3 item 4): used as it is, like an exact record.
        Some(f) => (
            crate::scaling::GlyphSource {
                kind: SourceKind::Exact,
                source_height: f.line_height_i32(),
                target: t,
                k: 1.0,
            },
            Arc::clone(f),
        ),
        None => {
            let src = choose_glyph_source(&ladder.heights(), t);
            let face = ladder
                .face_at(src.source_height)
                .cloned()
                .unwrap_or_else(|| Arc::clone(slot_face));
            (src, face)
        }
    };
    let line_height = i64::from(source_face.line_height().min(255));
    let baseline_face = i64::from(source_face.baseline().min(65_535));
    match source_face.lookup(c.unit).glyph {
        None => Resolved {
            a: 0,
            w: 1,
            b: 0,
            line_height,
            baseline: baseline_face,
            draw: ResolvedDraw::None,
        },
        Some(g) if src.kind == SourceKind::Exact => Resolved {
            a: i64::from(g.left_bearing),
            w: i64::from(g.width),
            b: i64::from(g.right_bearing),
            line_height,
            baseline: baseline_face,
            draw: ResolvedDraw::Native {
                face: source_face,
                glyph: g,
            },
        },
        Some(g) => {
            let dm = derived_metrics_of(&source_face, &g, src.k, t);
            Resolved {
                a: i64::from(dm.a),
                w: i64::from(dm.w),
                b: i64::from(dm.b),
                line_height: i64::from(dm.line_height),
                baseline: i64::from(dm.baseline),
                draw: ResolvedDraw::Derived {
                    face: source_face,
                    glyph: g,
                    unit: c.unit,
                    kind: src.kind,
                    dm,
                },
            }
        }
    }
}

const fn is_break_anywhere(u: u16) -> bool {
    matches!(u, 0x3040..=0x312F | 0x3190..=0x319F | 0x4E00..=0x9FBF | 0xF900..=0xFAFF)
}

const fn is_consumable(u: u16) -> bool {
    matches!(u, 0x0020 | 0x0009 | 0x000D)
}

/// Whether a break is possible before `c` when `p` precedes it (§7.3 rule 3).
const fn is_opportunity(p: u16, c: u16) -> bool {
    let allowed = is_consumable(c) || p == 0x002D || is_break_anywhere(c) || is_break_anywhere(p);
    let forbidden = matches!(
        c,
        0x0021
            | 0x0029
            | 0x002C
            | 0x002E
            | 0x003F
            | 0x3001
            | 0x3002
            | 0x30FC
            | 0xFF01
            | 0xFF09
            | 0xFF1F
            | 0xFF70
            | 0xFF9E
            | 0xFF9F
    ) || matches!(p, 0x0028 | 0xFF08);
    allowed && !forbidden
}

/// A line before metrics: a character range and the consumed character before it.
#[derive(Debug, Clone, Copy)]
struct RawLine {
    start: usize,
    end: usize,
    consumed: Option<usize>,
    /// The extra line after a final newline (§7.1 case 2): its font's line height.
    extra: Option<i64>,
    /// First character index whose metrics count towards the line (§7.1, §7.3): the
    /// consumed character when there is one, else `start`.
    metrics_from: usize,
    /// The empty line of a forced break (§7.1 case 4): every metric 0.
    zero: bool,
}

impl RawLine {
    fn new(start: usize, end: usize, consumed: Option<usize>) -> Self {
        Self {
            start,
            end,
            consumed,
            extra: None,
            metrics_from: consumed.unwrap_or(start),
            zero: false,
        }
    }

    /// Takes the metrics of a character consumed before an empty forced-break line.
    fn carrying(mut self, carry: &mut Option<usize>) -> Self {
        if let Some(c) = carry.take() {
            self.metrics_from = c;
        }
        self
    }
}

fn indent_of(m: &Metrics) -> i64 {
    if m.a < 0 { 1 - m.a } else { 0 }
}

/// Breaks the text into raw lines (§7.3).
fn break_lines(
    chars: &[TextChar],
    m: &[Metrics],
    adv: &[i64],
    extra: &[i64],
    avail: i64,
    one_line: bool,
) -> Vec<RawLine> {
    let n = chars.len();
    let mut lines = Vec::new();
    if one_line {
        lines.push(RawLine::new(0, n, None));
    } else {
        let mut start = 0usize;
        let mut consumed: Option<usize> = None;
        let mut pen = 0i64;
        let mut last_opp: Option<usize> = None;
        // A character consumed before an empty forced-break line: it counts towards the
        // metrics of the line after it (see the forced break below).
        let mut carry: Option<usize> = None;
        for i in 0..n {
            // Rule 1: a newline ends its line before the next character.
            if i > 0 && chars[i - 1].is_newline() {
                lines.push(RawLine::new(start, i, consumed.take()).carrying(&mut carry));
                start = i;
                last_opp = None;
            }
            if i == start {
                pen = indent_of(&m[i]);
            }
            // Rule 3: an opportunity before c is recorded before c's overflow is acted on.
            // Tessera reading (OPEN in `fonts.md` §7.3): an opportunity before the line's own
            // first character is not recorded, as for the first character of the text.
            // Breaking there would move nothing, give an empty line and consume a space that
            // §7.3 says starts the line ("spaces at the start of the next line stay").
            if i > start && is_opportunity(chars[i - 1].unit, chars[i].unit) {
                last_opp = Some(i);
            }
            // Rule 2.
            let overflows = pen + extra[i] + m[i].a + m[i].w > avail;
            if !overflows {
                pen += adv[i];
                continue;
            }
            // Rule 4; rule 5: moved characters are not tested again.
            if let Some(o) = last_opp.take() {
                lines.push(RawLine::new(start, o, consumed.take()).carrying(&mut carry));
                if is_consumable(chars[o].unit) {
                    consumed = Some(o);
                    start = o + 1;
                } else {
                    start = o;
                }
                pen = if start <= i {
                    indent_of(&m[start]) + adv[start..=i].iter().sum::<i64>()
                } else {
                    0
                };
            } else {
                // Forced break. When c is the line's first character the line is empty
                // (§7.1 case 4: height, baseline and leading 0). A character consumed by
                // the wrap that started that empty line stays its `consumed` (the order
                // of FN17), but its metrics count towards the next line, the one holding
                // c (§7.3; Tessera reading, OPEN in §7.3).
                let mut line = RawLine::new(start, i, consumed.take());
                if start == i {
                    line.zero = true;
                    carry = line.consumed;
                } else {
                    line = line.carrying(&mut carry);
                }
                lines.push(line);
                start = i;
                pen = indent_of(&m[i]) + adv[i];
            }
        }
        lines.push(RawLine::new(start, n, consumed).carrying(&mut carry));
    }
    // §7.1 case 2: the extra line after a final newline, in both modes.
    if chars.last().is_some_and(TextChar::is_newline) {
        lines.push(RawLine {
            extra: Some(m[n - 1].line_height),
            ..RawLine::new(n, n, None)
        });
    }
    lines
}

/// Trailing values *T* in text order (§7.2).
fn trailing_values(m: &[Metrics]) -> Vec<i64> {
    let mut prev = 0i64;
    m.iter()
        .map(|x| {
            let t = if prev >= 0 {
                x.b
            } else {
                x.b + (prev + x.w).min(0)
            };
            prev = t;
            t
        })
        .collect()
}

/// Laid-out lines without alignment, in 64-bit values.
struct Lines {
    raw: Vec<RawLine>,
    width: Vec<i64>,
    height: Vec<i64>,
    baseline: Vec<i64>,
    leading: Vec<i64>,
    indent: Vec<i64>,
    pens: Vec<i64>,
    line_of: Vec<u32>,
    text_height: i64,
}

/// Largest leading of one line (Tessera bound against hostile leading factors; a line is
/// at most 255 pixels high, so any real factor stays far below it).
const MAX_LEADING: i64 = 1 << 24;

fn leading_of(height: i64, f: f32) -> i64 {
    // Single precision, truncated toward zero (`as` saturates).
    ((height as f32 * f) as i64).clamp(0, MAX_LEADING)
}

#[allow(clippy::too_many_arguments)]
fn lay_lines(
    chars: &[TextChar],
    m: &[Metrics],
    adv: &[i64],
    extra: &[i64],
    t: &[i64],
    avail: i64,
    one_line: bool,
    f: f32,
    last_line_leading: bool,
) -> Lines {
    let raw = break_lines(chars, m, adv, extra, avail, one_line);
    let n = chars.len();
    let mut out = Lines {
        width: Vec::with_capacity(raw.len()),
        height: Vec::with_capacity(raw.len()),
        baseline: Vec::with_capacity(raw.len()),
        leading: Vec::with_capacity(raw.len()),
        indent: Vec::with_capacity(raw.len()),
        pens: vec![0; n],
        line_of: vec![0; n],
        text_height: 0,
        raw: Vec::new(),
    };
    for (li, r) in raw.iter().enumerate() {
        let li32 = u32::try_from(li).unwrap_or(u32::MAX);
        let (height, baseline) = if let Some(h) = r.extra {
            (h, 0)
        } else if r.zero {
            (0, 0)
        } else {
            let range = &m[r.metrics_from.min(r.end)..r.end];
            (
                range.iter().map(|x| x.line_height).max().unwrap_or(0),
                range.iter().map(|x| x.baseline).max().unwrap_or(0),
            )
        };
        let leading = if r.extra.is_some() || r.zero {
            0
        } else {
            leading_of(height, f)
        };
        let indent = if r.start < r.end {
            indent_of(&m[r.start])
        } else {
            0
        };
        let mut pen = indent;
        for ((p, l), a) in out.pens[r.start..r.end]
            .iter_mut()
            .zip(&mut out.line_of[r.start..r.end])
            .zip(&adv[r.start..r.end])
        {
            *p = pen;
            *l = li32;
            pen += a;
        }
        if let Some(c) = r.consumed {
            out.line_of[c] = li32;
        }
        let width = if r.start < r.end {
            pen - t[r.end - 1]
        } else {
            0
        };
        out.width.push(width);
        out.height.push(height);
        out.baseline.push(baseline);
        out.leading.push(leading);
        out.indent.push(indent);
    }
    if !last_line_leading && let Some(l) = out.leading.last_mut() {
        *l = 0;
    }
    out.text_height = out
        .height
        .iter()
        .zip(&out.leading)
        .map(|(h, l)| h + l)
        .sum();
    out.raw = raw;
    out
}

/// The offset of §7.4 on one axis.
fn justify(value: u32, available: i64, used: i64) -> i64 {
    let available = available.max(0);
    match value {
        JUSTIFY_BOTTOM | JUSTIFY_RIGHT => available - used,
        JUSTIFY_CENTER => available / 2 - used.max(0) / 2,
        _ => 0,
    }
}

/// Lays out a character list (`fonts.md` §6–§7).
///
/// `fonts` maps each character's [`TextChar::font`] to its face (`None` or out of range: a
/// slot without a font). `suffix` is the resolved text of `UICore_Text_truncate_text_to_fit`
/// when the element truncates (§7.6): pass it only when the property is present, resolves
/// to at least one character and the element is neither editable nor selectable (the
/// Tessera rule of OPEN Q13).
///
/// # Errors
/// [`TextError::TextTooLong`] when text and suffix together exceed
/// [`super::MAX_TEXT_UNITS`].
pub fn layout(
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &LayoutParams,
    suffix: Option<&[TextChar]>,
) -> Result<TextLayout, TextError> {
    layout_impl(chars, fonts, params, suffix, None)
}

/// Lays out a character list at a content scale (`ui-scaling.md` §6.4.5), choosing a crisp glyph
/// size per font from the ladders in `scale`. At `scale.is_identity()` the result is byte-identical
/// to [`layout`] (invariant I7). Pair it with [`super::draw_scaled`] using the same `scale`.
///
/// # Errors
/// [`TextError::TextTooLong`] when text and suffix together exceed [`super::MAX_TEXT_UNITS`].
pub fn layout_scaled(
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &LayoutParams,
    suffix: Option<&[TextChar]>,
    scale: &ScaleCtx<'_>,
) -> Result<TextLayout, TextError> {
    let scale = if scale.is_identity() {
        None
    } else {
        Some(scale)
    };
    layout_impl(chars, fonts, params, suffix, scale)
}

fn layout_impl(
    chars: &[TextChar],
    fonts: &[Option<Arc<FontFace>>],
    params: &LayoutParams,
    suffix: Option<&[TextChar]>,
    scale: Option<&ScaleCtx<'_>>,
) -> Result<TextLayout, TextError> {
    let total = chars.len() + suffix.map_or(0, <[TextChar]>::len);
    if total > MAX_TEXT_UNITS {
        return Err(TextError::TextTooLong(total));
    }
    let x_extra = i64::from(params.list_extra.clamp(-65_536, 65_536));
    let f = if params.leading.is_nan() || params.leading < 0.0 {
        0.0
    } else {
        params.leading
    };
    let mg = params.margins;
    let (ml, mr, mt, mb) = (
        i64::from(mg.left),
        i64::from(mg.right),
        i64::from(mg.top),
        i64::from(mg.bottom),
    );
    let (ew, eh) = (i64::from(params.width), i64::from(params.height));

    let prepare = |cs: &[TextChar]| {
        let m: Vec<Metrics> = cs
            .iter()
            .map(|c| match scale {
                None => metrics(c, fonts),
                Some(s) => resolve_scaled(c, fonts, s).to_metrics(),
            })
            .collect();
        let extra: Vec<i64> = cs
            .iter()
            .map(|c| if c.list_item { x_extra } else { 0 })
            .collect();
        let adv: Vec<i64> = m
            .iter()
            .zip(&extra)
            .map(|(x, e)| x.a + x.w + x.b + e)
            .collect();
        let t = trailing_values(&m);
        (m, extra, adv, t)
    };

    let (m, extra, adv, t) = prepare(chars);
    let avail = ew - ml - mr;
    let mut lines = lay_lines(
        chars,
        &m,
        &adv,
        &extra,
        &t,
        avail,
        params.one_line,
        f,
        params.last_line_leading,
    );
    let mut widths = lines.width.clone();
    let mut content_w = widths.iter().copied().max().unwrap_or(0) + ml + mr;
    let mut content_h = lines.text_height + mt + mb;

    // §7.6 truncation.
    let mut truncation = None;
    if let Some(sfx) = suffix {
        let (sm, sextra, sadv, st) = prepare(sfx);
        let s_lines = lay_lines(sfx, &sm, &sadv, &sextra, &st, i64::MAX, true, f, false);
        let s = s_lines.width.first().copied().unwrap_or(0);
        let does_not_fit = if params.one_line {
            content_w > ew
        } else {
            content_h > eh
        };
        if does_not_fit {
            let cut_line = if params.one_line {
                0
            } else {
                let ha = eh - mt - mb - 1;
                let mut sum = 0i64;
                let mut cut = 0usize;
                for (i, (h, l)) in lines.height.iter().zip(&lines.leading).enumerate() {
                    sum += h + l;
                    if sum <= ha {
                        cut = i;
                    } else {
                        break;
                    }
                }
                cut
            };
            let r = lines.raw[cut_line];
            let indent = lines.indent[cut_line];
            let stored = |k: usize| (m[k].a + m[k].w + m[k].b).clamp(0, 255);
            let mut budget = ew - ml - mr - s - indent;
            let mut cut = r.start;
            let mut stopped_on: Option<usize> = None;
            for k in r.start..r.end {
                let a = stored(k);
                if budget > a && k + 1 < r.end {
                    budget -= a;
                } else {
                    budget -= a;
                    cut = k;
                    stopped_on = Some(k);
                    break;
                }
            }
            let is_last_line = cut_line + 1 == lines.raw.len();
            if is_last_line && budget > 0 && !stopped_on.is_some_and(|k| chars[k].is_newline()) {
                cut = chars.len();
            }
            let cut_width = indent + (r.start..cut.min(r.end)).map(stored).sum::<i64>() + s;
            widths[cut_line] = cut_width;
            if params.one_line {
                content_w = cut_width + ml + mr;
            } else {
                content_w = widths[..=cut_line].iter().copied().max().unwrap_or(0) + ml + mr;
                content_h = lines.height[..=cut_line]
                    .iter()
                    .zip(&lines.leading[..=cut_line])
                    .map(|(h, l)| h + l)
                    .sum::<i64>()
                    + mt
                    + mb;
            }
            truncation = Some(Truncation {
                line: cut_line,
                cut,
                suffix_width: sat32(s),
                suffix: sfx.to_vec(),
            });
        }
    }

    // §7.4 alignment.
    let h_avail = ew.max(content_w) - ml - mr;
    let normal_lh = fonts
        .get(usize::from(SLOT_NORMAL))
        .and_then(Option::as_ref)
        .map_or(0, |f| i64::from(f.line_height().min(255)));
    let v_used = content_h.max(normal_lh);
    let v_avail = eh.max(content_h) - mt - mb;
    let mut top = mt + justify(params.vertical, v_avail, v_used);

    // Per-character values first, so the per-character temporaries can be freed before the
    // line boxes are built (bounds the peak of a long text).
    let mut places = Vec::with_capacity(chars.len());
    for k in 0..chars.len() {
        places.push(CharPlace {
            line: lines.line_of[k],
            consumed: false,
            pen: sat32(lines.pens[k]),
            advance: sat32(adv[k]),
            stored_advance: (m[k].a + m[k].w + m[k].b).clamp(0, 255) as u8,
            trailing: sat32(t[k]),
        });
    }
    for r in &lines.raw {
        if let Some(c) = r.consumed {
            places[c].consumed = true;
        }
    }
    drop((m, extra, adv, t));
    lines.pens = Vec::new();
    lines.line_of = Vec::new();

    let mut out_lines = Vec::with_capacity(lines.raw.len());
    for (i, r) in lines.raw.iter().enumerate() {
        let x = ml + justify(params.horizontal, h_avail, widths[i]);
        out_lines.push(LineBox {
            start: r.start,
            end: r.end,
            consumed: r.consumed,
            width: sat32(widths[i]),
            height: sat32(lines.height[i]),
            baseline: sat32(lines.baseline[i]),
            leading: sat32(lines.leading[i]),
            indent: sat32(lines.indent[i]),
            x: sat32(x),
            top: sat32(top),
        });
        top += lines.height[i] + lines.leading[i];
    }

    Ok(TextLayout {
        lines: out_lines,
        places,
        text_height: sat32(lines.text_height),
        content_width: sat32(content_w),
        content_height: sat32(content_h),
        list_extra: sat32(x_extra),
        truncation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opportunities() {
        // §13.4: "well-known", "( x", "x, y", CJK.
        assert!(is_opportunity(u16::from(b'-'), u16::from(b'k')));
        assert!(!is_opportunity(u16::from(b'('), u16::from(b' ')));
        assert!(!is_opportunity(u16::from(b'x'), u16::from(b',')));
        assert!(is_opportunity(u16::from(b','), u16::from(b' ')));
        assert!(is_opportunity(0x4E00, 0x4E01));
        assert!(!is_opportunity(0x4E00, 0x3002));
        assert!(!is_opportunity(u16::from(b'a'), u16::from(b'b')));
    }

    #[test]
    fn leading_factor_is_never_negative() {
        assert_eq!(leading_factor(0.0, -1.0), 0.0);
        assert_eq!(leading_factor(f32::NAN, 0.2), 0.0);
        assert_eq!(leading_factor(0.0, 0.2), 0.2);
    }

    #[test]
    fn mouse_conversion_of_13_12() {
        assert_eq!(
            mouse_to_text([31.0, 16.0], [1.5, 1.5, 1.0], [0, 5]),
            [20, 15]
        );
        assert_eq!(
            mouse_to_text([31.0, 16.0], [1.0, 1.0, 1.0], [0, 5]),
            [31, 21]
        );
        assert_eq!(
            mouse_to_text([3.0, 3.0], [0.0, f32::NAN, 1.0], [0, 0])[1],
            0
        );
    }

    #[test]
    fn underline_thickness_of_13_9() {
        assert_eq!(underline_thickness(10, 0), 1);
        assert_eq!(underline_thickness(10, 2), 1);
        assert_eq!(underline_thickness(20, 2), 2);
        assert_eq!(underline_thickness(-100, 0), 1);
    }
}
