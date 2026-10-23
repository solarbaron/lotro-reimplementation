// SPDX-License-Identifier: GPL-3.0-or-later
//! Glyph atlases and glyph lookup (`fonts.md` §4–§5).
//!
//! A [`FontFace`] holds what layout and drawing need from one font: line height, baseline,
//! outline borders, the glyph table with a direct lookup table, and the foreground and
//! (optional) background atlas as shared one-byte coverage maps. Building one is a pure
//! function of the decoded record and its surfaces, so the client runs it on the asset
//! workers (§4.1 Tessera Decision); the atlases are never uploaded to the GPU.

use std::sync::Arc;

use tessera_formats::font::{Font, Glyph, MAX_GLYPHS, QUESTION_MARK};
use tessera_formats::{Did, Record, Surface, SurfaceFormat};

use super::TextError;
use crate::paint::CoverageMap;

/// Surface category of glyph atlases (`fonts.md` §4.1, FN8).
const ATLAS_CATEGORY: u32 = 13;

/// Marks a lookup-table slot with no glyph.
const NONE: u32 = u32::MAX;

/// Largest source coverage box this module copies out of an atlas (`ui-scaling.md` §6.4.4): a
/// bound so a hostile outline border on a glyph cannot drive an unbounded allocation. Far above
/// any real glyph box (a 256-pixel glyph with a 256-pixel border on every side is well under it).
const MAX_COVER_TEXELS: usize = 1 << 22;

/// Where a [`FontFace`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontId {
    /// A font record of the data (DID with prefix 0x42).
    Data(u32),
    /// Tessera's own font rasterised at one pixel size (`fonts.md` §11 item 2). The three
    /// fields together identify the face, so it can key a cache: faces of different font
    /// files or code sets at one size differ.
    Tessera {
        /// The font file's key ([`super::TesseraFont::key`]: FNV-1a 64 of its bytes).
        font: u64,
        /// FNV-1a 64 of the code set rasterised (the requested codes with U+0020 and `?`,
        /// as little-endian units in ascending order).
        ranges: u64,
        /// Pixel size the outlines were rasterised at.
        px: u32,
    },
}

/// The result of a glyph lookup (`fonts.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphLookup {
    /// The glyph to lay out and draw: the unit's own, or `?` from the same font (§5 step 2);
    /// `None` when neither exists (§5 step 3: layout uses a blank 1 × 1 glyph, drawing
    /// draws nothing and advances 0).
    pub glyph: Option<Glyph>,
    /// Whether `glyph` is the `?` substitute for a missing unit.
    pub substituted: bool,
}

/// One font ready for layout and drawing.
#[derive(Clone)]
pub struct FontFace {
    id: FontId,
    line_height: u32,
    baseline: u32,
    border_x: u32,
    border_y: u32,
    glyphs: Vec<Glyph>,
    /// Smallest code; slot *i* of `table` is code `base + i`.
    base: u16,
    /// Index into `glyphs`, or [`NONE`].
    table: Vec<u32>,
    question: Option<u32>,
    foreground: Arc<CoverageMap>,
    background: Option<Arc<CoverageMap>>,
}

impl std::fmt::Debug for FontFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontFace")
            .field("id", &self.id)
            .field("line_height", &self.line_height)
            .field("baseline", &self.baseline)
            .field("borders", &(self.border_x, self.border_y))
            .field("glyphs", &self.glyphs.len())
            .field("foreground", &self.foreground)
            .field("background", &self.background)
            .finish_non_exhaustive()
    }
}

/// Copies the coverage bytes of an A8 surface into a coverage map.
fn coverage_of(surface: &Surface<'_>) -> Result<CoverageMap, TextError> {
    let did = surface.did.0;
    if surface.format != SurfaceFormat::A8 {
        return Err(TextError::Atlas {
            did,
            reason: "not a one-byte (format 28) surface",
        });
    }
    if surface.category != ATLAS_CATEGORY {
        // FN8 holds for every atlas of the observed install; another category changes
        // nothing about how the bytes are read.
        tracing::warn!(
            did,
            category = surface.category,
            "glyph atlas category is not 13"
        );
    }
    let want = u64::from(surface.width) * u64::from(surface.height);
    if surface.data.len() as u64 != want {
        return Err(TextError::Atlas {
            did,
            reason: "data length is not width × height",
        });
    }
    CoverageMap::new(surface.width, surface.height, surface.data.to_vec()).map_err(|_| {
        TextError::Atlas {
            did,
            reason: "atlas size outside the coverage-map limits",
        }
    })
}

/// Decodes an atlas surface payload.
fn parse_surface(data: &[u8], did: Did) -> Result<Surface<'_>, TextError> {
    Surface::parse(data).map_err(|_| TextError::Atlas {
        did: did.0,
        reason: "surface does not decode",
    })
}

impl FontFace {
    /// Builds a face from a decoded font record and its atlases (`fonts.md` §4).
    ///
    /// `background` must be given when the record names a background atlas and is ignored
    /// when it does not (background DID 0: the stamped outline of §8.3 is used). Rules of
    /// §3.3 that the record breaks are logged, not fatal (§3.4 step 3); a background atlas
    /// of another size than the foreground is kept (pixels outside it read as 0).
    ///
    /// # Errors
    /// [`TextError::AtlasMismatch`] when a surface is not the one the record names;
    /// [`TextError::Atlas`] when the foreground is missing its bytes, is not A8 or is too
    /// large; the background likewise, and when the record names one but none is given;
    /// [`TextError::TooManyGlyphs`] past 65,536 entries.
    pub fn from_record(
        font: &Font,
        foreground: &Surface<'_>,
        background: Option<&Surface<'_>>,
    ) -> Result<Self, TextError> {
        if foreground.did != font.foreground {
            return Err(TextError::AtlasMismatch {
                expected: "foreground",
                want: font.foreground.0,
                got: foreground.did.0,
            });
        }
        let fg = Arc::new(coverage_of(foreground)?);
        let bg = if font.has_background() {
            let Some(b) = background else {
                return Err(TextError::Atlas {
                    did: font.background.0,
                    reason: "the record names a background atlas but none was given",
                });
            };
            if b.did != font.background {
                return Err(TextError::AtlasMismatch {
                    expected: "background",
                    want: font.background.0,
                    got: b.did.0,
                });
            }
            let map = coverage_of(b)?;
            if map.width() != fg.width() || map.height() != fg.height() {
                tracing::warn!(
                    did = font.did.0,
                    "background atlas size differs from the foreground (FN8)"
                );
            }
            Some(Arc::new(map))
        } else {
            None
        };
        for issue in font.issues() {
            tracing::debug!(
                did = font.did.0,
                issue = issue.name(),
                "font record rule broken"
            );
        }
        Self::from_parts(
            FontId::Data(font.did.0),
            font.line_height,
            font.baseline,
            [font.border_x, font.border_y],
            font.glyphs().to_vec(),
            fg,
            bg,
        )
    }

    /// Decodes a font record payload and its atlas surface payloads, then builds the face
    /// ([`FontFace::from_record`]). `background` is read only when the record names a
    /// background atlas.
    ///
    /// # Errors
    /// [`TextError::Record`] or [`TextError::Atlas`] for payloads that do not decode, and the
    /// errors of [`FontFace::from_record`].
    pub fn from_payloads(
        font: &[u8],
        foreground: &[u8],
        background: Option<&[u8]>,
    ) -> Result<Self, TextError> {
        let font = Font::parse(font).map_err(|e| TextError::Record(e.to_string()))?;
        let fg = parse_surface(foreground, font.foreground)?;
        match (font.has_background(), background) {
            (true, Some(b)) => {
                let bg = parse_surface(b, font.background)?;
                Self::from_record(&font, &fg, Some(&bg))
            }
            _ => Self::from_record(&font, &fg, None),
        }
    }

    /// Builds a face from its parts: metrics, glyph entries in code order and atlases.
    ///
    /// The lookup table is indexed by code − smallest code (§5 step 1); when codes repeat
    /// (invalid data) the first entry wins.
    ///
    /// # Errors
    /// [`TextError::TooManyGlyphs`] for more than 65,536 glyphs.
    pub fn from_parts(
        id: FontId,
        line_height: u32,
        baseline: u32,
        borders: [u32; 2],
        glyphs: Vec<Glyph>,
        foreground: Arc<CoverageMap>,
        background: Option<Arc<CoverageMap>>,
    ) -> Result<Self, TextError> {
        if glyphs.len() > MAX_GLYPHS as usize {
            return Err(TextError::TooManyGlyphs(glyphs.len()));
        }
        let (base, table) = match (
            glyphs.iter().map(|g| g.code).min(),
            glyphs.iter().map(|g| g.code).max(),
        ) {
            (Some(lo), Some(hi)) => {
                // At most 65,536 slots of 4 bytes: bounded by the 16-bit code.
                let mut t = vec![NONE; usize::from(hi - lo) + 1];
                for (i, g) in glyphs.iter().enumerate() {
                    let slot = &mut t[usize::from(g.code - lo)];
                    if *slot == NONE {
                        *slot = i as u32;
                    }
                }
                (lo, t)
            }
            _ => (0, Vec::new()),
        };
        let mut face = Self {
            id,
            line_height,
            baseline,
            border_x: borders[0],
            border_y: borders[1],
            glyphs,
            base,
            table,
            question: None,
            foreground,
            background,
        };
        face.question = face.index_of(QUESTION_MARK);
        Ok(face)
    }

    fn index_of(&self, unit: u16) -> Option<u32> {
        let i = unit.checked_sub(self.base)?;
        match self.table.get(usize::from(i)) {
            Some(&NONE) | None => None,
            Some(&g) => Some(g),
        }
    }

    /// Finds the glyph for one UTF-16 code unit (`fonts.md` §5): the unit's own glyph, else
    /// `?` from the same font, else none. Never another font.
    #[must_use]
    pub fn lookup(&self, unit: u16) -> GlyphLookup {
        if let Some(i) = self.index_of(unit) {
            return GlyphLookup {
                glyph: self.glyphs.get(i as usize).copied(),
                substituted: false,
            };
        }
        GlyphLookup {
            glyph: self
                .question
                .and_then(|i| self.glyphs.get(i as usize).copied()),
            substituted: true,
        }
    }

    /// Where the face comes from.
    #[must_use]
    pub fn id(&self) -> FontId {
        self.id
    }

    /// Line height in pixels (record header offset 4).
    #[must_use]
    pub fn line_height(&self) -> u32 {
        self.line_height
    }

    /// Baseline: pixels from the top of the line box down to the baseline.
    #[must_use]
    pub fn baseline(&self) -> u32 {
        self.baseline
    }

    /// Outline borders (*bx*, *by*).
    #[must_use]
    pub fn borders(&self) -> [u32; 2] {
        [self.border_x, self.border_y]
    }

    /// Glyph entries in stored order.
    #[must_use]
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// The foreground (glyph coverage) atlas.
    #[must_use]
    pub fn foreground(&self) -> &Arc<CoverageMap> {
        &self.foreground
    }

    /// The background (outline coverage) atlas; `None` selects the stamped outline (§8.3).
    #[must_use]
    pub fn background(&self) -> Option<&Arc<CoverageMap>> {
        self.background.as_ref()
    }

    /// Approximate heap bytes held (atlases counted fully even when shared).
    #[must_use]
    pub fn memory_bytes(&self) -> usize {
        self.foreground.data().len()
            + self.background.as_ref().map_or(0, |b| b.data().len())
            + self.glyphs.len() * std::mem::size_of::<Glyph>()
            + self.table.len() * 4
    }

    /// Line height as a signed value, for the scaling decisions of `ui-scaling.md` §6.4
    /// (`target_height`, the ladder). Saturated to [`i32::MAX`].
    #[must_use]
    pub fn line_height_i32(&self) -> i32 {
        i32::try_from(self.line_height).unwrap_or(i32::MAX)
    }

    /// Copies a glyph's source coverage box out of an atlas, for derivation (`ui-scaling.md`
    /// §6.4.4). With `outline` false it is the fill box `W × H` at the glyph's `(X, Y)` in the
    /// foreground atlas; with `outline` true it is the bordered box `(W + 2·bx) × (H + 2·by)` at
    /// `(X − bx, Y − by)` in the background atlas (`fonts.md` §8.2). Coverage outside the atlas
    /// reads 0 ([`CoverageMap::at`]). Returns the bytes (row-major) and the box size.
    ///
    /// `None` when `outline` is set but the face has no background atlas, when the box is empty,
    /// or when a hostile border would make the box exceed [`MAX_COVER_TEXELS`].
    #[must_use]
    pub(crate) fn cover_box(&self, g: &Glyph, outline: bool) -> Option<(Vec<u8>, usize, usize)> {
        let (map, x0, y0, bw, bh) = if outline {
            let bg = self.background.as_ref()?;
            let bx = i64::from(self.border_x);
            let by = i64::from(self.border_y);
            (
                bg.as_ref(),
                i64::from(g.x) - bx,
                i64::from(g.y) - by,
                i64::from(g.width) + 2 * bx,
                i64::from(g.height) + 2 * by,
            )
        } else {
            (
                self.foreground.as_ref(),
                i64::from(g.x),
                i64::from(g.y),
                i64::from(g.width),
                i64::from(g.height),
            )
        };
        if bw <= 0 || bh <= 0 {
            return None;
        }
        let (w, h) = (usize::try_from(bw).ok()?, usize::try_from(bh).ok()?);
        if w.checked_mul(h)? > MAX_COVER_TEXELS {
            return None;
        }
        let mut out = vec![0u8; w * h];
        for yy in 0..h {
            let row_y = y0.saturating_add(yy as i64);
            for xx in 0..w {
                let px = x0.saturating_add(xx as i64);
                // `at` clamps to 0 outside the atlas; the coordinates stay in i32 range for any
                // real atlas, and saturation keeps a hostile position from wrapping.
                out[yy * w + xx] = map.at(
                    i32::try_from(px).unwrap_or(i32::MAX),
                    i32::try_from(row_y).unwrap_or(i32::MAX),
                );
            }
        }
        Some((out, w, h))
    }
}

/// A font *ladder* for crisp scaling (`ui-scaling.md` §6.4.2): the sibling records of one
/// logical font that differ only in size, held as faces in ascending line-height order. The
/// scaled text path chooses a source record from the ladder for a target height
/// ([`crate::scaling::choose_glyph_source`]). A font that stands alone (no siblings, or a DID
/// the font enum does not name, §6.4.3) is a one-face ladder.
#[derive(Clone, Default)]
pub struct FontLadder {
    faces: Vec<Arc<FontFace>>,
    /// A face that serves one target height directly, whatever its own line height (Tessera's
    /// font rasterised at that height, `ui-scaling.md` §6.4.3 item 4 and its last paragraph).
    direct: Option<(i32, Arc<FontFace>)>,
}

impl std::fmt::Debug for FontLadder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontLadder")
            .field("heights", &self.heights())
            .finish()
    }
}

impl FontLadder {
    /// A ladder from a set of sibling faces; sorted by line height (ascending). Duplicates and
    /// unsorted input are accepted.
    #[must_use]
    pub fn new(mut faces: Vec<Arc<FontFace>>) -> Self {
        faces.sort_by_key(|f| f.line_height());
        Self {
            faces,
            direct: None,
        }
    }

    /// A one-face ladder (a font that stands alone, §6.4.3).
    #[must_use]
    pub fn single(face: Arc<FontFace>) -> Self {
        Self {
            faces: vec![face],
            direct: None,
        }
    }

    /// A one-face ladder whose face serves target height `target` as an exact record, used as
    /// it is with its own metrics (`ui-scaling.md` §6.4.3 item 4: Tessera's font rasterised
    /// directly at the target height, whose line height need not equal it). Any other target
    /// height is chosen from the face by the usual rules (§6.4.3 items 1–3).
    pub(crate) fn direct(target: i32, face: Arc<FontFace>) -> Self {
        Self {
            faces: vec![Arc::clone(&face)],
            direct: Some((target, face)),
        }
    }

    /// The face that serves `target` directly ([`FontLadder::direct`]), if any.
    pub(crate) fn direct_face(&self, target: i32) -> Option<&Arc<FontFace>> {
        self.direct
            .as_ref()
            .filter(|(t, _)| *t == target)
            .map(|(_, f)| f)
    }

    /// Whether the ladder holds no face.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// The line heights of the ladder's records, ascending, for
    /// [`crate::scaling::choose_glyph_source`].
    #[must_use]
    pub fn heights(&self) -> Vec<i32> {
        self.faces.iter().map(|f| f.line_height_i32()).collect()
    }

    /// The face whose line height is `line_height`, if any (the record
    /// [`crate::scaling::choose_glyph_source`] picked).
    #[must_use]
    pub fn face_at(&self, line_height: i32) -> Option<&Arc<FontFace>> {
        self.faces
            .iter()
            .find(|f| f.line_height_i32() == line_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Arc<CoverageMap> {
        Arc::new(CoverageMap::new(4, 4, vec![0; 16]).unwrap())
    }

    fn g(code: u16, w: u8) -> Glyph {
        Glyph {
            width: w,
            height: 1,
            code,
            ..Glyph::default()
        }
    }

    #[test]
    fn substitution_and_missing_question_mark() {
        let f = FontFace::from_parts(
            FontId::Data(1),
            10,
            8,
            [0, 0],
            vec![g(0x20, 1), g(0x3F, 2), g(0x41, 3)],
            map(),
            None,
        )
        .unwrap();
        assert_eq!(f.lookup(0x41).glyph.unwrap().width, 3);
        let s = f.lookup(0xE9);
        assert!(s.substituted);
        assert_eq!(s.glyph.unwrap().code, 0x3F);
        assert_eq!(f.lookup(0x10).glyph.unwrap().code, 0x3F);
        let none = FontFace::from_parts(
            FontId::Data(1),
            10,
            8,
            [0, 0],
            vec![g(0x41, 3)],
            map(),
            None,
        )
        .unwrap();
        assert_eq!(none.lookup(0x42).glyph, None);
        let empty =
            FontFace::from_parts(FontId::Data(1), 10, 8, [0, 0], vec![], map(), None).unwrap();
        assert_eq!(empty.lookup(0x41).glyph, None);
    }

    #[test]
    fn repeated_codes_take_the_first_entry() {
        let f = FontFace::from_parts(
            FontId::Data(1),
            10,
            8,
            [0, 0],
            vec![g(0x41, 3), g(0x41, 5)],
            map(),
            None,
        )
        .unwrap();
        assert_eq!(f.lookup(0x41).glyph.unwrap().width, 3);
    }
}
