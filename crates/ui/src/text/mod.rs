// SPDX-License-Identifier: GPL-3.0-or-later
//! Text: glyph atlases, glyph lookup, placement, line layout and drawing (`fonts.md` §4–§9),
//! and Tessera's own fallback font path (`fonts.md` §11).
//!
//! The pieces, in the order a text element uses them:
//!
//! 1. [`FontFace`]: one font ready for layout and drawing: the metrics and glyph table of a
//!    decoded font record (`tessera-formats`, prefix 0x42) plus its glyph atlases as shared
//!    one-byte coverage maps ([`CoverageMap`]). [`FontFace::from_record`] and
//!    [`FontFace::from_payloads`] are pure functions, meant to run on the asset workers
//!    (`fonts.md` §4.1, decision 0015 §3). [`FontFace::lookup`] finds a glyph with the `?`
//!    substitution of §5.
//! 2. [`TesseraFont`]: Tessera's own font for text without a game layout (§11 item 2),
//!    rasterised on the CPU at 1 : 1 into a [`FontFace`] of the same form (§11 item 4).
//! 3. [`FontSlots`] and [`FontProps`]: the two font slots of a text element and how the
//!    `UICore_Text_*` font properties fill them (§9.1); [`FontIndexSession`]: the quest text
//!    size ladder (§9.2).
//! 4. [`CharListBuilder`] / [`TextChar`]: the character list, each character with its slot,
//!    colour, underline and list-item flag.
//! 5. [`layout`]: line metrics, breaking, alignment, list items and truncation (§6–§7),
//!    giving a [`TextLayout`].
//! 6. [`draw`]: the outline pass, then the fill pass with underlines (§8.1–§8.5), as
//!    [`TextPaint`]: glyph coverage runs ([`GlyphRun`]), underline fills ([`FillRect`]) and
//!    bullet positions, all in canvas coordinates. The byte-exact compositing of §8.4 is
//!    done by [`crate::paint`]; [`TextPaint::push_to`] appends the operations to a
//!    [`DisplayList`].
//!
//! [`mouse_to_text`] is the hit-test conversion of §6.3 item 3; [`mouse_to_text_physical`] and
//! [`TextLayout::hit`] the physical hit test of the crisp scaled path (`ui-scaling.md` §6.4.6).
//! [`layout_scaled`] / [`draw_scaled`] lay text out and draw it at a content scale with glyphs
//! chosen or derived for that size (`ui-scaling.md` §6.4); [`physical_params`] and
//! [`relayout_lines`] feed them from an element's layout-unit values.
//!
//! Everything here is pure and allocation-bounded: text longer than [`MAX_TEXT_UNITS`] code
//! units is refused, all coordinates are computed in 64-bit integers and saturated to 32
//! bits, and hostile font records or atlases give a [`TextError`], never a panic.
//!
//! [`CoverageMap`]: crate::paint::CoverageMap
//! [`GlyphRun`]: crate::paint::GlyphRun
//! [`FillRect`]: crate::paint::FillRect
//! [`DisplayList`]: crate::paint::DisplayList

mod atlas;
mod chars;
mod draw;
mod fallback;
mod layout;
mod scaled;
mod select;

pub use atlas::{FontFace, FontId, FontLadder, GlyphLookup};
pub use chars::{CharListBuilder, SLOT_NORMAL, SLOT_TAG, TextChar};
pub use draw::{BulletArt, DrawParams, TextOp, TextPaint, draw, draw_scaled};
pub use fallback::{
    DEFAULT_RANGES, MAX_FALLBACK_PX, MIN_FALLBACK_PX, TesseraFont, bundled_font_bytes,
};
pub use layout::{
    CharPlace, JUSTIFY_BOTTOM, JUSTIFY_CENTER, JUSTIFY_LEFT, JUSTIFY_RIGHT, JUSTIFY_TOP,
    LayoutParams, LineBox, Margins, ScaleCtx, TextLayout, Truncation, layout, layout_scaled,
    leading_factor, list_extra, mouse_to_text, underline_thickness,
};
pub(crate) use scaled::stretch_text_ops;
pub use scaled::{
    DEFAULT_GLYPH_CACHE_BYTES, TEXT_SCALE_RANGE, TextParamsReader, UiTextSource, ceil_div_scale,
    mouse_to_text_physical, physical_params, relayout_lines,
};
pub use select::{FontIndexSession, FontProps, FontSlots, FontStruct, SlotSource, StructMember};

/// Most UTF-16 code units one layout accepts (text plus truncation suffix), and most units a
/// [`CharListBuilder`] keeps. Tessera cap against hostile input, sized to the largest text
/// element of the data: the chat log's line template allows 128,000 characters
/// (`chat-window.md` §5.1), and 131,072 is the next power of two. A chat log is laid out one
/// line element at a time, and a plugin label is far below the cap.
///
/// Cost at the cap (indicative, release build, not measured on the reference machine of
/// decision 0010): one [`layout`] of 131,072 one-glyph lines takes about 25 ms and 30 MB of
/// transient memory, so a text that long belongs on an asset worker or a sliced paint job,
/// not in the 4 ms paint slice of decision 0015 U4.
pub const MAX_TEXT_UNITS: usize = 1 << 17;

/// Errors of the text module.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// A font record that does not decode (`fonts.md` §3.4).
    #[error("font record: {0}")]
    Record(String),
    /// A glyph-atlas surface that does not decode or is not a one-byte coverage image.
    #[error("glyph atlas {did:#010x}: {reason}")]
    Atlas {
        /// The atlas DID (0 for an atlas without one).
        did: u32,
        /// What is wrong.
        reason: &'static str,
    },
    /// The atlas DID passed does not match the one the font record names.
    #[error("atlas {got:#010x} is not the font's {expected} atlas {want:#010x}")]
    AtlasMismatch {
        /// `foreground` or `background`.
        expected: &'static str,
        /// The DID the record names.
        want: u32,
        /// The DID of the surface passed.
        got: u32,
    },
    /// More glyphs than a 16-bit code allows (`fonts.md` §3.4 step 5).
    #[error("{0} glyphs exceed the cap")]
    TooManyGlyphs(usize),
    /// Text longer than [`MAX_TEXT_UNITS`].
    #[error("text of {0} code units exceeds the cap")]
    TextTooLong(usize),
    /// A Tessera font file that does not parse.
    #[error("Tessera font: {0}")]
    FontFile(&'static str),
    /// A pixel size outside [`MIN_FALLBACK_PX`]..=[`MAX_FALLBACK_PX`].
    #[error("pixel size {0} outside the supported range")]
    PixelSize(u32),
    /// A rasterised glyph set that does not fit the largest coverage map.
    #[error("rasterised glyphs do not fit a {0}-pixel atlas")]
    AtlasFull(u32),
}

/// Saturates a 64-bit value to 32 bits.
#[inline]
pub(crate) fn sat32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
