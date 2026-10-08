// SPDX-License-Identifier: GPL-3.0-or-later
//! Font records, DID prefix 0x42: line metrics, the glyph table and the two glyph-atlas DIDs.
//!
//! Spec: `docs/specs/fonts.md` §3 (record), §10 (census), §12 (invariants), §13.1 (worked
//! example). Of the §12 invariants, decoding enforces FN1 and FN2, [`Font::issues`] checks
//! FN3–FN7 and [`Font::atlas_issues`] checks FN9; FN8, FN10 and FN11 need the atlas surfaces
//! and are checked by the real-data test (`tests/font_real_data.rs`). Glyph lookup with `?` substitution, placement, line layout and
//! drawing (§5–§9) belong to `tessera-ui`; this module only decodes and checks the record.
//!
//! The record is a 16-byte header, *n* glyph entries of 11 bytes and a 20-byte trailer, so
//! its length is exactly 36 + 11·*n* (FN1). The decoder checks that length before it
//! allocates anything, and caps *n* at [`MAX_GLYPHS`] (§3.4 step 5).

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::Reader;
use crate::record::{Record, read_prologue};

/// Size of the header: DID, line height, widest glyph, glyph count (§3.2).
pub const HEADER_LEN: usize = 16;
/// Size of one glyph entry (§3.2).
pub const GLYPH_LEN: usize = 11;
/// Size of the trailer: two borders, baseline, two atlas DIDs (§3.2).
pub const TRAILER_LEN: usize = 20;
/// Largest glyph count the decoder accepts (§3.4 step 5): a code is 16 bits, so no valid
/// record holds more entries.
pub const MAX_GLYPHS: u32 = 65_536;
/// The code unit that substitutes for a missing glyph (§5 step 2); every observed font has it.
pub const QUESTION_MARK: u16 = 0x003F;
/// The first code of every observed font (§3.3).
pub const SPACE: u16 = 0x0020;

/// One glyph entry (§3.2). Field names follow the spec's letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Glyph {
    /// **W**: width of the glyph box in pixels.
    pub width: u8,
    /// **H**: height of the glyph box in pixels.
    pub height: u8,
    /// **A**: left bearing, from the pen position to the box's left edge; may be negative.
    pub left_bearing: i8,
    /// **B**: right bearing, from the box's right edge to the next pen position; may be
    /// negative.
    pub right_bearing: i8,
    /// **T**: distance from the top of the line box down to the top of the glyph box
    /// (read as signed; observed 0–31).
    pub top: i8,
    /// The UTF-16 code unit this glyph draws.
    pub code: u16,
    /// **X**: left column of the glyph box in both atlases.
    pub x: u16,
    /// **Y**: top row of the glyph box in both atlases.
    pub y: u16,
}

impl Glyph {
    /// Pen advance in pixels: *A* + *W* + *B* (§6.1, before kerning). May be negative for
    /// hostile data; observed 0–37.
    #[must_use]
    pub fn advance(&self) -> i32 {
        i32::from(self.left_bearing) + i32::from(self.width) + i32::from(self.right_bearing)
    }

    /// The glyph box in the atlas as (x, y, width, height).
    #[must_use]
    pub fn atlas_box(&self) -> (u32, u32, u32, u32) {
        (
            u32::from(self.x),
            u32::from(self.y),
            u32::from(self.width),
            u32::from(self.height),
        )
    }

    /// The bordered box (*X* − *bx*, *Y* − *by*, *W* + 2*bx*, *H* + 2*by*) that the outline
    /// (background) image of the glyph occupies (§4.1). Coordinates may be negative for
    /// hostile data.
    #[must_use]
    pub fn bordered_box(&self, border_x: u32, border_y: u32) -> (i64, i64, i64, i64) {
        let bx = i64::from(border_x);
        let by = i64::from(border_y);
        (
            i64::from(self.x) - bx,
            i64::from(self.y) - by,
            i64::from(self.width) + 2 * bx,
            i64::from(self.height) + 2 * by,
        )
    }
}

/// A decoded font record (§3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    /// The record's own DID (0x42…).
    pub did: Did,
    /// Height of the line box in pixels (header offset 4); equals the largest *H* (FN3).
    pub line_height: u32,
    /// Largest glyph width in pixels (header offset 8; FN3).
    pub widest_glyph: u32,
    /// Glyph entries in stored order (strictly ascending codes in valid data, FN4). Kept
    /// private so that the ascending-code flag used by [`Font::find`] stays true to it.
    glyphs: Vec<Glyph>,
    /// Horizontal outline border *bx* in pixels.
    pub border_x: u32,
    /// Vertical outline border *by* in pixels.
    pub border_y: u32,
    /// Distance from the top of the line box down to the baseline, in pixels.
    pub baseline: u32,
    /// Foreground (glyph coverage) atlas, a 0x41 A8 surface.
    pub foreground: Did,
    /// Background (outline coverage) atlas, or the null DID when the font has none.
    pub background: Did,
    /// Whether the codes are strictly ascending (FN4); [`Font::find`] uses a binary search
    /// only when they are.
    codes_ascending: bool,
}

/// A §3.3 / §12 rule that a decoded font breaks. Reported, never fatal (§3.4 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FontIssue {
    /// FN3: the line height is not the largest glyph height.
    LineHeightNotMaxHeight,
    /// FN3: the widest-glyph field is not the largest glyph width.
    WidestNotMaxWidth,
    /// FN4: glyph codes are not strictly ascending (first offending entry index).
    CodesNotAscending(usize),
    /// FN4: the first code is not U+0020.
    FirstCodeNotSpace,
    /// FN4: U+003F `?` is missing, so a missing glyph draws nothing (§5 step 3).
    NoQuestionMark,
    /// FN5: an entry has *H* = 0 (entry index).
    ZeroHeight(usize),
    /// FN5: an entry has *T* < 0 or *T* + *H* above the line height (entry index).
    GlyphOutsideLine(usize),
    /// FN5: *A* + *W* + *B* is negative (entry index).
    NegativeAdvance(usize),
    /// FN5: *A* + *W* + *B* is above 255 (entry index). The client's stored per-character
    /// advance is clamped to 0–255 (§6.1), so the underline extent and the truncation
    /// measure of such a glyph differ from its pen advance.
    AdvanceAbove255(usize),
    /// FN6: the baseline is not above the bottom of the line box.
    BaselineNotInLine,
    /// FN7: the two borders differ.
    BordersDiffer,
    /// FN7: the font has no background atlas but non-zero borders.
    NoBackgroundWithBorder,
    /// The record has no glyphs at all.
    Empty,
}

impl FontIssue {
    /// Short stable name, for censuses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::LineHeightNotMaxHeight => "line height is not max H",
            Self::WidestNotMaxWidth => "widest glyph is not max W",
            Self::CodesNotAscending(_) => "codes not ascending",
            Self::FirstCodeNotSpace => "first code not U+0020",
            Self::NoQuestionMark => "no U+003F",
            Self::ZeroHeight(_) => "glyph height 0",
            Self::GlyphOutsideLine(_) => "glyph outside the line box",
            Self::NegativeAdvance(_) => "negative advance",
            Self::AdvanceAbove255(_) => "advance above 255",
            Self::BaselineNotInLine => "baseline not inside the line box",
            Self::BordersDiffer => "borders differ",
            Self::NoBackgroundWithBorder => "no background atlas but a border",
            Self::Empty => "no glyphs",
        }
    }
}

/// An atlas rule (FN9) that a font breaks for a given atlas size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AtlasIssue {
    /// The bordered box of entry `index` does not lie inside the atlas.
    OutsideAtlas(usize),
    /// The bordered boxes of entries `first` and `second` overlap.
    Overlap {
        /// Index of the earlier entry.
        first: usize,
        /// Index of the later entry.
        second: usize,
    },
}

impl Font {
    /// Builds a font from its parts (for fixtures and tools).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        did: Did,
        line_height: u32,
        widest_glyph: u32,
        glyphs: Vec<Glyph>,
        border_x: u32,
        border_y: u32,
        baseline: u32,
        foreground: Did,
        background: Did,
    ) -> Self {
        let codes_ascending = glyphs.windows(2).all(|w| w[0].code < w[1].code);
        Self {
            did,
            line_height,
            widest_glyph,
            glyphs,
            border_x,
            border_y,
            baseline,
            foreground,
            background,
            codes_ascending,
        }
    }

    /// Glyph entries in stored order.
    #[must_use]
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// Number of glyph entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the record has no glyph entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Whether the font names a background (outline) atlas.
    #[must_use]
    pub fn has_background(&self) -> bool {
        !self.background.is_null()
    }

    /// Whether the codes are strictly ascending (FN4).
    #[must_use]
    pub fn codes_ascending(&self) -> bool {
        self.codes_ascending
    }

    /// Index of the entry whose code is exactly `code` (§5 step 1), without the `?`
    /// substitution of §5 step 2. If codes repeat (invalid data), the first match wins.
    #[must_use]
    pub fn find(&self, code: u16) -> Option<usize> {
        if self.codes_ascending {
            self.glyphs.binary_search_by_key(&code, |g| g.code).ok()
        } else {
            self.glyphs.iter().position(|g| g.code == code)
        }
    }

    /// The entry whose code is exactly `code`, if any (see [`Font::find`]).
    #[must_use]
    pub fn glyph(&self, code: u16) -> Option<&Glyph> {
        self.find(code).and_then(|i| self.glyphs.get(i))
    }

    /// Checks the record-level rules of §3.3 and §12 (FN3–FN7). An empty list means the
    /// font satisfies all of them.
    #[must_use]
    pub fn issues(&self) -> Vec<FontIssue> {
        let mut out = Vec::new();
        if self.glyphs.is_empty() {
            out.push(FontIssue::Empty);
        }
        let max_h = self.glyphs.iter().map(|g| u32::from(g.height)).max();
        let max_w = self.glyphs.iter().map(|g| u32::from(g.width)).max();
        if max_h.is_some_and(|h| h != self.line_height) {
            out.push(FontIssue::LineHeightNotMaxHeight);
        }
        if max_w.is_some_and(|w| w != self.widest_glyph) {
            out.push(FontIssue::WidestNotMaxWidth);
        }
        if let Some(i) = self.glyphs.windows(2).position(|w| w[0].code >= w[1].code) {
            out.push(FontIssue::CodesNotAscending(i + 1));
        }
        if self.glyphs.first().is_some_and(|g| g.code != SPACE) {
            out.push(FontIssue::FirstCodeNotSpace);
        }
        if !self.glyphs.is_empty() && self.find(QUESTION_MARK).is_none() {
            out.push(FontIssue::NoQuestionMark);
        }
        for (i, g) in self.glyphs.iter().enumerate() {
            if g.height == 0 {
                out.push(FontIssue::ZeroHeight(i));
            }
            let bottom = i64::from(g.top) + i64::from(g.height);
            if g.top < 0 || bottom > i64::from(self.line_height) {
                out.push(FontIssue::GlyphOutsideLine(i));
            }
            if g.advance() < 0 {
                out.push(FontIssue::NegativeAdvance(i));
            } else if g.advance() > 255 {
                out.push(FontIssue::AdvanceAbove255(i));
            }
        }
        if self.baseline >= self.line_height {
            out.push(FontIssue::BaselineNotInLine);
        }
        if self.border_x != self.border_y {
            out.push(FontIssue::BordersDiffer);
        }
        if !self.has_background() && (self.border_x != 0 || self.border_y != 0) {
            out.push(FontIssue::NoBackgroundWithBorder);
        }
        out
    }

    /// Checks FN9 against an atlas of `width` × `height` pixels: every bordered box lies
    /// inside the atlas and no two overlap. At most `limit` issues are returned.
    ///
    /// Overlaps are found by a sweep over the rows: boxes enter an active set ordered by
    /// left edge at their top row and leave it at their bottom row, and each entering box is
    /// compared only with active boxes whose left edge lies within one box width of it. While
    /// no overlap has been found the active boxes are disjoint, so the search stops at the
    /// first neighbour to the left that ends before the entering box. The cost is
    /// O(*n* log *n*) plus the overlaps reported, whatever the record holds (§3.4).
    #[must_use]
    pub fn atlas_issues(&self, width: u32, height: u32, limit: usize) -> Vec<AtlasIssue> {
        use std::cmp::Reverse;
        use std::collections::{BTreeMap, BinaryHeap};

        let mut out = Vec::new();
        if limit == 0 {
            return out;
        }
        let boxes: Vec<(i64, i64, i64, i64)> = self
            .glyphs
            .iter()
            .map(|g| g.bordered_box(self.border_x, self.border_y))
            .collect();
        for (i, &(x, y, w, h)) in boxes.iter().enumerate() {
            if x < 0 || y < 0 || x + w > i64::from(width) || y + h > i64::from(height) {
                out.push(AtlasIssue::OutsideAtlas(i));
                if out.len() >= limit {
                    return out;
                }
            }
        }
        // Boxes of zero area never overlap anything.
        let mut order: Vec<usize> = (0..boxes.len())
            .filter(|&i| boxes[i].2 > 0 && boxes[i].3 > 0)
            .collect();
        order.sort_by_key(|&i| (boxes[i].1, boxes[i].0, i));
        let widest = order.iter().map(|&i| boxes[i].2).max().unwrap_or(0);
        // Active boxes: (left, index) -> right edge; and a queue of (bottom, left, index).
        let mut active: BTreeMap<(i64, usize), i64> = BTreeMap::new();
        let mut leaving: BinaryHeap<Reverse<(i64, i64, usize)>> = BinaryHeap::new();
        let mut disjoint = true;
        for &b in &order {
            let (bx, by, bw, bh) = boxes[b];
            while let Some(&Reverse((bottom, left, i))) = leaving.peek() {
                if bottom > by {
                    break;
                }
                leaving.pop();
                active.remove(&(left, i));
            }
            // An active box overlaps b when its left edge is below b's right edge and its
            // right edge is above b's left edge; no box is wider than `widest`.
            let lowest = (bx - widest + 1, 0);
            for (&(_, a), &right) in active.range(lowest..(bx + bw, 0)).rev() {
                if right <= bx {
                    if disjoint {
                        break;
                    }
                    continue;
                }
                out.push(AtlasIssue::Overlap {
                    first: a.min(b),
                    second: a.max(b),
                });
                if out.len() >= limit {
                    return out;
                }
                disjoint = false;
            }
            active.insert((bx, b), bx + bw);
            leaving.push(Reverse((by + bh, bx, b)));
        }
        out
    }

    /// Decodes a font record and returns it together with the §3.3 issues it has, which
    /// [`Record::parse`] logs as one warning per record and otherwise ignores (§3.4 step 3).
    pub fn parse_with_issues(data: &[u8]) -> Result<(Self, Vec<FontIssue>)> {
        let font = Self::parse(data)?;
        let issues = font.issues();
        Ok((font, issues))
    }
}

impl Record<'_> for Font {
    const NAME: &'static str = "font";

    /// Decodes a complete payload (§3.4). The length must be exactly 36 + 11·*n*.
    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        if data.len() < HEADER_LEN + TRAILER_LEN {
            return Err(FormatError::Truncated {
                offset: 0,
                needed: HEADER_LEN + TRAILER_LEN,
                available: data.len(),
            });
        }
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::Font)?;
        let line_height = r.u32()?;
        let widest_glyph = r.u32()?;
        let count_offset = r.position();
        let count = r.u32()?;
        if count > MAX_GLYPHS {
            return Err(FormatError::CountLimit {
                what: "font glyph count",
                offset: count_offset,
                count: u64::from(count),
                limit: u64::from(MAX_GLYPHS),
            });
        }
        let expected = (HEADER_LEN + TRAILER_LEN) as u64 + GLYPH_LEN as u64 * u64::from(count);
        if data.len() as u64 != expected {
            return Err(FormatError::malformed(
                "font record",
                count_offset,
                format!(
                    "payload is {} bytes but {count} glyphs need exactly {expected}",
                    data.len()
                ),
            ));
        }
        let mut glyphs = r.alloc_vec::<Glyph>(count, GLYPH_LEN, usize::MAX)?;
        for _ in 0..count {
            let b = r.array::<GLYPH_LEN>()?;
            let g = Glyph {
                width: b[0],
                height: b[1],
                left_bearing: b[2] as i8,
                right_bearing: b[3] as i8,
                top: b[4] as i8,
                code: u16::from_le_bytes([b[5], b[6]]),
                x: u16::from_le_bytes([b[7], b[8]]),
                y: u16::from_le_bytes([b[9], b[10]]),
            };
            r.push(&mut glyphs, g, count)?;
        }
        let border_x = r.u32()?;
        let border_y = r.u32()?;
        let baseline = r.u32()?;
        let foreground = r.did()?;
        let background = r.did()?;
        r.expect_end(Self::NAME)?;
        let font = Self::new(
            did,
            line_height,
            widest_glyph,
            glyphs,
            border_x,
            border_y,
            baseline,
            foreground,
            background,
        );
        // §3.4 step 3: report broken entries as a warning, still decode the record.
        if tracing::enabled!(tracing::Level::WARN) {
            let issues = font.issues();
            if let Some(first) = issues.first() {
                tracing::warn!(did = %font.did, count = issues.len(), first = first.name(),
                    "font record breaks fonts.md §3.3");
            }
        }
        Ok(font)
    }

    fn did(&self) -> Did {
        self.did
    }
}

/// Encodes a font record (§3.2) byte-exactly; the inverse of [`Record::parse`]. For
/// synthetic fixtures and round-trip tests.
#[must_use]
pub fn encode_font(font: &Font) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + TRAILER_LEN + GLYPH_LEN * font.glyphs.len());
    out.extend_from_slice(&font.did.0.to_le_bytes());
    out.extend_from_slice(&font.line_height.to_le_bytes());
    out.extend_from_slice(&font.widest_glyph.to_le_bytes());
    let n = u32::try_from(font.glyphs.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&n.to_le_bytes());
    for g in &font.glyphs {
        out.extend_from_slice(&[
            g.width,
            g.height,
            g.left_bearing as u8,
            g.right_bearing as u8,
            g.top as u8,
        ]);
        out.extend_from_slice(&g.code.to_le_bytes());
        out.extend_from_slice(&g.x.to_le_bytes());
        out.extend_from_slice(&g.y.to_le_bytes());
    }
    for v in [
        font.border_x,
        font.border_y,
        font.baseline,
        font.foreground.0,
        font.background.0,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}
