// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own font (`fonts.md` §11).
//!
//! Text that no game layout draws (start-up and install-detection screens, error dialogs,
//! Tessera's settings, the plugin manager without a game layout, debug overlays) needs a
//! font that works without a game install (§11 item 2). Tessera bundles **Noto Sans
//! Regular** under the SIL Open Font License 1.1 (`crates/ui/assets/fonts/`, §11 item 5);
//! it is not a game font.
//!
//! [`TesseraFont::rasterise`] turns its outlines into a [`FontFace`] at one pixel size, on
//! the CPU with a pure-Rust rasteriser (`ab_glyph`), at 1 : 1 into the same one-byte
//! coverage form as the game atlases (§11 item 4): the glyphs are packed with the rule of
//! §4.2 (borders 0) and the face has no background atlas, so the outline style uses the
//! stamped outline of §8.3. From there layout and drawing are exactly those of game fonts,
//! including the `?` substitution of §5 (the face never falls back to another font).
//! Rasterising is a pure function; the client runs it on a worker.

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _, point};
use tessera_formats::font::{Glyph, QUESTION_MARK, SPACE};

use super::{FontFace, FontId, TextError};
use crate::paint::{CoverageMap, MAX_COVERAGE_DIM};

static BUNDLED: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

/// The bundled font file (Noto Sans Regular 2.015, SIL OFL 1.1; licence in
/// `crates/ui/assets/fonts/OFL.txt`).
#[must_use]
pub fn bundled_font_bytes() -> &'static [u8] {
    BUNDLED
}

/// Smallest pixel size [`TesseraFont::rasterise`] accepts.
pub const MIN_FALLBACK_PX: u32 = 6;
/// Largest pixel size [`TesseraFont::rasterise`] accepts. For the bundled font it keeps every
/// metric within the glyph-entry ranges of `fonts.md` §3.2 (tested); a user's font may still
/// have glyphs that do not fit, and those are left out (see [`TesseraFont::rasterise`]).
pub const MAX_FALLBACK_PX: u32 = 64;

/// The code ranges rasterised by default: Basic Latin, Latin-1, Latin Extended-A, Greek,
/// Cyrillic, general punctuation, the euro and trade mark signs (the scripts the game fonts
/// cover, `fonts.md` §10). Codes the font lacks are left out and show as `?` (§5).
pub const DEFAULT_RANGES: &[(u16, u16)] = &[
    (0x0020, 0x007E),
    (0x00A0, 0x017F),
    (0x0370, 0x03FF),
    (0x0400, 0x04FF),
    (0x2010, 0x2027),
    (0x2030, 0x203A),
    (0x20AC, 0x20AC),
    (0x2122, 0x2122),
];

/// Most coverage bytes one rasterisation may produce before packing (Tessera bound against
/// hostile fonts and ranges).
const MAX_RASTER_PIXELS: u64 = 16 * 1024 * 1024;

/// Atlas widths tried in order.
const ATLAS_WIDTHS: [u32; 6] = [128, 256, 512, 1024, 2048, 4096];

/// A Tessera font: an outline font that can be rasterised at any supported pixel size.
#[derive(Clone)]
pub struct TesseraFont {
    font: FontArc,
    /// FNV-1a 64 of the font file (see [`TesseraFont::key`]).
    key: u64,
}

impl std::fmt::Debug for TesseraFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TesseraFont").finish_non_exhaustive()
    }
}

/// One rasterised glyph before packing.
struct Raster {
    code: u16,
    w: u32,
    h: u32,
    a: i64,
    b: i64,
    top: i64,
    pixels: Vec<u8>,
}

fn clamp_i8(v: i64) -> i8 {
    v.clamp(i64::from(i8::MIN), i64::from(i8::MAX)) as i8
}

fn fits_i8(v: i64) -> bool {
    (i64::from(i8::MIN)..=i64::from(i8::MAX)).contains(&v)
}

/// Smallest ascent − descent, in font units, [`TesseraFont::rasterise`] accepts (the
/// TrueType minimum units-per-em; real fonts use 1,000–2,048). Below it the scale factor is
/// out of range and the font is refused.
const MIN_FONT_HEIGHT_UNITS: f32 = 16.0;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a (64-bit) over `bytes`, continuing from `h`: a stable, deterministic key.
fn fnv1a(h: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(h, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A packing: atlas width, atlas height and the top-left corner of every glyph.
type Packing = (u32, u32, Vec<(u32, u32)>);

/// Packs `rasters` into an atlas `aw` wide (§4.2 with borders 0), or `None` when a glyph is
/// wider than the atlas or the atlas would be too tall.
fn pack(rasters: &[Raster], aw: u32) -> Option<Packing> {
    let mut x = 0u32;
    let mut y = 0u32;
    let mut row_h = 0u32;
    let mut places = Vec::with_capacity(rasters.len());
    for (i, r) in rasters.iter().enumerate() {
        if r.w > aw {
            return None;
        }
        if i > 0 && x + r.w > aw {
            y += row_h;
            x = 0;
            row_h = 0;
        }
        places.push((x, y));
        row_h = row_h.max(r.h);
        x += r.w;
    }
    let h = (y + row_h).max(1);
    (h <= MAX_COVERAGE_DIM).then_some((aw, h, places))
}

impl TesseraFont {
    /// The bundled font.
    ///
    /// # Errors
    /// [`TextError::FontFile`] if the embedded file does not parse (never for the shipped
    /// file; tested).
    pub fn bundled() -> Result<Self, TextError> {
        static KEY: OnceLock<u64> = OnceLock::new();
        let key = *KEY.get_or_init(|| fnv1a(FNV_OFFSET, BUNDLED));
        FontArc::try_from_slice(BUNDLED)
            .map(|font| Self { font, key })
            .map_err(|_| TextError::FontFile("the bundled font does not parse"))
    }

    /// A font from TrueType or OpenType bytes (an original or OFL font of the user's
    /// choice, `fonts.md` §11 items 3 and 5).
    ///
    /// # Errors
    /// [`TextError::FontFile`] when the bytes are not a font.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, TextError> {
        let key = fnv1a(FNV_OFFSET, &data);
        FontArc::try_from_vec(data)
            .map(|font| Self { font, key })
            .map_err(|_| TextError::FontFile("not a TrueType or OpenType font"))
    }

    /// The font's key: FNV-1a (64-bit) of its file bytes. Two fonts with the same bytes have
    /// the same key; it goes into [`FontId::Tessera`] of every face rasterised from it.
    #[must_use]
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Rasterises the codes of `ranges` (inclusive, BMP code units; surrogates are skipped)
    /// at `px` pixels (the height from the font's ascent to its descent) into a face.
    /// `U+0020` and `?` are always included. The face's [`FontId::Tessera`] names this font
    /// ([`key`](Self::key)), the code set and `px`, so faces of other fonts or other ranges
    /// at the same size never share an id.
    ///
    /// The font is treated as hostile input: a glyph whose box is larger than 4 × `px` (at
    /// most 255) or whose advance, bearings or top do not fit a glyph entry (`fonts.md`
    /// §3.2) is left out with a warning, before it is drawn, so it shows as `?` (§5).
    ///
    /// # Errors
    /// [`TextError::PixelSize`] outside [`MIN_FALLBACK_PX`]..=[`MAX_FALLBACK_PX`];
    /// [`TextError::FontFile`] when the font's vertical metrics give a scale out of range
    /// (ascent − descent under 16 font units, zero, negative or not finite) or line metrics
    /// above 255 pixels; [`TextError::AtlasFull`] when the glyphs do not fit a
    /// 4096-pixel-wide atlas or exceed the rasterisation budget.
    pub fn rasterise(&self, px: u32, ranges: &[(u16, u16)]) -> Result<FontFace, TextError> {
        if !(MIN_FALLBACK_PX..=MAX_FALLBACK_PX).contains(&px) {
            return Err(TextError::PixelSize(px));
        }
        let mut codes: BTreeSet<u16> = BTreeSet::new();
        codes.insert(SPACE);
        codes.insert(QUESTION_MARK);
        for &(lo, hi) in ranges {
            for u in lo..=hi {
                codes.insert(u);
            }
        }
        let ranges_key = codes
            .iter()
            .fold(FNV_OFFSET, |h, &u| fnv1a(h, &u.to_le_bytes()));
        let scale = PxScale::from(px as f32);
        let sf = self.font.as_scaled(scale);
        let max_box = i64::from(px) * 4;
        // Hostile vertical metrics (fonts.md §11 items 3 and 5 accept a user's font): an
        // ascent − descent of a few font units, zero or negative gives a huge, infinite or
        // NaN scale, and with it glyph bounds the rasteriser would allocate or loop over.
        let limit = px as f32 / MIN_FONT_HEIGHT_UNITS;
        let factors = [sf.h_scale_factor(), sf.v_scale_factor()];
        if factors
            .iter()
            .any(|f| !(f.is_finite() && *f > 0.0 && *f <= limit))
        {
            return Err(TextError::FontFile("vertical metrics out of range"));
        }
        let (ascent_f, descent_f) = (f64::from(sf.ascent()), f64::from(sf.descent()));
        let bound = max_box as f64;
        if !(ascent_f.is_finite() && descent_f.is_finite())
            || ascent_f.abs() > bound
            || descent_f.abs() > bound
        {
            return Err(TextError::FontFile("vertical metrics out of range"));
        }
        let ascent = ascent_f.ceil() as i64;
        let descent = (-descent_f).ceil().max(0.0) as i64;
        // Largest glyph box side; also what a glyph entry holds (fonts.md §3.2).
        let box_cap = max_box.min(255);

        let mut rasters = Vec::new();
        let mut total_pixels = 0u64;
        let mut dropped = 0usize;
        for code in codes {
            let Some(ch) = char::from_u32(u32::from(code)) else {
                continue;
            };
            let id = self.font.glyph_id(ch);
            if id.0 == 0 && code != SPACE && code != QUESTION_MARK {
                continue;
            }
            let adv_f = f64::from(sf.h_advance(id));
            if !adv_f.is_finite() || adv_f.abs() > bound {
                dropped += 1;
                continue;
            }
            let adv = adv_f.round() as i64;
            let glyph = id.with_scale_and_position(scale, point(0.0, ascent as f32));
            let mut r = Raster {
                code,
                w: 0,
                h: 0,
                a: 0,
                b: adv,
                top: 0,
                pixels: Vec::new(),
            };
            if let Some(o) = self.font.outline_glyph(glyph) {
                let bb = o.px_bounds();
                let edges = [bb.min.x, bb.min.y, bb.max.x, bb.max.y].map(f64::from);
                // Checked before drawing: the rasteriser allocates the whole box.
                if edges.iter().any(|e| !e.is_finite() || e.abs() > bound)
                    || edges[2] - edges[0] > box_cap as f64
                    || edges[3] - edges[1] > box_cap as f64
                {
                    dropped += 1;
                    continue;
                }
                let (x0, y0) = (edges[0] as i64, edges[1] as i64);
                let w = ((edges[2] - edges[0]) as i64).clamp(0, box_cap);
                let h = ((edges[3] - edges[1]) as i64).clamp(0, box_cap);
                let (wu, hu) = (w as u32, h as u32);
                total_pixels += u64::from(wu) * u64::from(hu);
                if total_pixels > MAX_RASTER_PIXELS {
                    return Err(TextError::AtlasFull(MAX_COVERAGE_DIM));
                }
                let mut pixels = vec![0u8; (wu * hu) as usize];
                o.draw(|x, y, c| {
                    if x < wu && y < hu {
                        pixels[(y * wu + x) as usize] = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                });
                r.w = wu;
                r.h = hu;
                r.a = x0;
                r.b = adv - x0 - w;
                r.top = y0;
                r.pixels = pixels;
            }
            // A and B must fit their glyph-entry fields (fonts.md §3.2); a glyph whose
            // metrics do not is left out (it shows as `?`, §5), never silently clamped.
            if !fits_i8(r.a) || !fits_i8(r.b) {
                dropped += 1;
                continue;
            }
            rasters.push(r);
        }

        // Glyphs that reach above the line box move the baseline down (T ≥ 0, §3.3).
        let shift = rasters
            .iter()
            .filter(|r| r.h > 0)
            .map(|r| -r.top)
            .max()
            .unwrap_or(0)
            .max(0);
        let baseline = ascent + shift;
        let before = rasters.len();
        rasters.retain_mut(|r| {
            if r.h > 0 {
                r.top += shift;
            }
            fits_i8(r.top)
        });
        dropped += before - rasters.len();
        let mut line_height = baseline + descent;
        for r in &rasters {
            line_height = line_height.max(r.top + i64::from(r.h));
        }
        if !(0..=255).contains(&line_height) || !(0..=255).contains(&baseline) {
            return Err(TextError::FontFile("line metrics exceed 255 pixels"));
        }
        if dropped > 0 {
            tracing::warn!(
                px,
                dropped,
                "Tessera font: glyphs whose metrics do not fit a glyph entry were left out"
            );
        }

        // Pack with the rule of §4.2, borders 0: the narrowest width whose atlas is not
        // taller than wide, else the widest that fits.
        let mut packs: Vec<_> = ATLAS_WIDTHS
            .iter()
            .filter_map(|&aw| pack(&rasters, aw))
            .collect();
        let (width, height, places) = packs
            .iter()
            .position(|(w, h, _)| h <= w)
            .or_else(|| packs.len().checked_sub(1))
            .map(|i| packs.swap_remove(i))
            .ok_or(TextError::AtlasFull(ATLAS_WIDTHS[ATLAS_WIDTHS.len() - 1]))?;

        let mut data = vec![0u8; width as usize * height as usize];
        let mut glyphs = Vec::with_capacity(rasters.len());
        for (r, &(x, y)) in rasters.iter().zip(&places) {
            for row in 0..r.h {
                let src = &r.pixels[(row * r.w) as usize..((row + 1) * r.w) as usize];
                let at = (y + row) as usize * width as usize + x as usize;
                data[at..at + r.w as usize].copy_from_slice(src);
            }
            glyphs.push(Glyph {
                width: r.w as u8,
                height: r.h as u8,
                left_bearing: clamp_i8(r.a),
                right_bearing: clamp_i8(r.b),
                top: clamp_i8(r.top), // all three checked by `fits_i8` above
                code: r.code,
                x: x as u16,
                y: y as u16,
            });
        }
        let map = CoverageMap::new(width, height, data).map_err(|_| TextError::AtlasFull(width))?;
        FontFace::from_parts(
            FontId::Tessera {
                font: self.key,
                ranges: ranges_key,
                px,
            },
            u32::try_from(line_height).unwrap_or(0),
            u32::try_from(baseline).unwrap_or(0),
            [0, 0],
            glyphs,
            Arc::new(map),
            None,
        )
    }
}
