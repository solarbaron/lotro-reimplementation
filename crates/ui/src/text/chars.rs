// SPDX-License-Identifier: GPL-3.0-or-later
//! The character list of a text element (`fonts.md` §7, §7.5, §8.5, §9.1).
//!
//! Each character is one UTF-16 code unit (§5) with the font slot it falls in, its text
//! colour, its underline (with the underline colour) and the list-item flag of §7.5. The
//! markup reader that turns tags into these attributes belongs to the element; the
//! [`CharListBuilder`] offers the two list rules of §7.5 that concern characters: a break
//! tag becomes a newline unless it is dropped at the very start or right after a newline,
//! and the list-item flag goes to the next character emitted, and only to that one.

/// Index of the normal font slot in the fonts slice of [`super::layout`] and
/// [`super::draw`] (`fonts.md` §9.1).
pub const SLOT_NORMAL: u8 = 0;
/// Index of the tag font slot (link labels only, `fonts.md` §9.1).
pub const SLOT_TAG: u8 = 1;

use super::MAX_TEXT_UNITS;

/// The newline code unit.
pub(crate) const NEWLINE: u16 = 0x000A;

/// One character of the character list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextChar {
    /// The UTF-16 code unit.
    pub unit: u16,
    /// Index into the fonts slice: [`SLOT_NORMAL`] or [`SLOT_TAG`] for an element (any
    /// index works; one past the slice means "slot without a font", §9.1 rule 4).
    pub font: u8,
    /// Text colour, `0xAARRGGBB` (`fonts.md` §8.4).
    pub colour: u32,
    /// The underline colour when the character is inside `<u>` (§8.5); `None` when not
    /// underlined. OPEN Q16: outside a coloured link, callers pass the text colour.
    pub underline: Option<u32>,
    /// The list-item flag of §7.5: the first character after an `<li>`.
    pub list_item: bool,
}

impl TextChar {
    /// A plain character in slot `font` and `colour`.
    #[must_use]
    pub const fn plain(unit: u16, font: u8, colour: u32) -> Self {
        Self {
            unit,
            font,
            colour,
            underline: None,
            list_item: false,
        }
    }

    /// Whether the character is a newline.
    #[must_use]
    pub const fn is_newline(&self) -> bool {
        self.unit == NEWLINE
    }
}

/// Builds a character list from styled pieces of text.
///
/// The list keeps at most [`MAX_TEXT_UNITS`] characters: units pushed past the cap are
/// dropped (Tessera bound, so a hostile string never allocates more than the cap) and
/// [`overflowed`](Self::overflowed) reports it. A caller that must not show a cut text
/// refuses the list then.
#[derive(Debug, Clone, Default)]
pub struct CharListBuilder {
    chars: Vec<TextChar>,
    overflowed: bool,
    pending_list_item: bool,
    /// The current style.
    font: u8,
    colour: u32,
    underline: Option<u32>,
}

impl CharListBuilder {
    /// An empty list whose style starts as the normal slot in opaque white.
    #[must_use]
    pub fn new() -> Self {
        Self {
            colour: 0xFFFF_FFFF,
            ..Self::default()
        }
    }

    /// Sets the font slot of the characters that follow.
    pub fn slot(&mut self, font: u8) -> &mut Self {
        self.font = font;
        self
    }

    /// Sets the text colour of the characters that follow.
    pub fn colour(&mut self, colour: u32) -> &mut Self {
        self.colour = colour;
        self
    }

    /// Sets (with its colour) or clears the underline of the characters that follow.
    pub fn underline(&mut self, colour: Option<u32>) -> &mut Self {
        self.underline = colour;
        self
    }

    fn push_unit(&mut self, unit: u16) -> bool {
        if self.chars.len() >= MAX_TEXT_UNITS {
            self.overflowed = true;
            return false;
        }
        self.chars.push(TextChar {
            unit,
            font: self.font,
            colour: self.colour,
            underline: self.underline,
            list_item: std::mem::take(&mut self.pending_list_item),
        });
        true
    }

    /// Appends text in the current style, as UTF-16 code units (a character outside the
    /// Basic Multilingual Plane becomes a surrogate pair, `fonts.md` §5).
    pub fn push_str(&mut self, text: &str) -> &mut Self {
        let mut buf = [0u16; 2];
        for c in text.chars() {
            let units = c.encode_utf16(&mut buf);
            // A surrogate pair is kept whole or dropped whole at the cap.
            if self.chars.len() + units.len() > MAX_TEXT_UNITS {
                self.overflowed = true;
                break;
            }
            for &u in units.iter() {
                self.push_unit(u);
            }
        }
        self
    }

    /// Appends UTF-16 code units in the current style.
    pub fn push_units(&mut self, units: &[u16]) -> &mut Self {
        for &u in units {
            if !self.push_unit(u) {
                break;
            }
        }
        self
    }

    /// A tag with the "break" flag (`<p>`, `<li>`): a newline, unless `drop_leading` (the
    /// element option of `string-table.md` §4.7) is set and the list is empty or ends in a
    /// newline (§7.5). Returns whether a newline was added (never past the cap).
    pub fn push_break(&mut self, drop_leading: bool) -> bool {
        let at_line_start = self.chars.last().is_none_or(TextChar::is_newline);
        if drop_leading && at_line_start {
            return false;
        }
        // The break itself is the character emitted, so a pending list flag would go to it;
        // `<li>` sets the flag after its break (see `list_item`).
        self.push_unit(NEWLINE)
    }

    /// `<li>`: a break (as [`push_break`](Self::push_break)), then the list-item flag for
    /// the next character emitted (§7.5 step 1).
    pub fn list_item(&mut self, drop_leading: bool) -> &mut Self {
        self.push_break(drop_leading);
        self.pending_list_item = true;
        self
    }

    /// Whether units were dropped at the [`MAX_TEXT_UNITS`] cap.
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// The characters so far.
    #[must_use]
    pub fn chars(&self) -> &[TextChar] {
        &self.chars
    }

    /// The finished list.
    #[must_use]
    pub fn finish(self) -> Vec<TextChar> {
        self.chars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_flag_goes_to_the_next_character_only() {
        let mut b = CharListBuilder::new();
        b.list_item(true).push_str("AB");
        let c = b.finish();
        assert_eq!(c.len(), 2, "the leading break is dropped");
        assert!(c[0].list_item);
        assert!(!c[1].list_item);
    }

    #[test]
    fn break_after_text_is_a_newline() {
        let mut b = CharListBuilder::new();
        b.push_str("A");
        assert!(b.push_break(true));
        assert!(!b.push_break(true), "dropped right after a newline");
        assert!(b.push_break(false));
        assert_eq!(b.chars().len(), 3);
    }

    #[test]
    fn the_list_stops_at_the_cap() {
        let mut b = CharListBuilder::new();
        b.push_units(&vec![0x41; MAX_TEXT_UNITS - 1]);
        assert!(!b.overflowed());
        b.push_str("BCD");
        assert!(b.overflowed());
        assert!(!b.push_break(false));
        b.push_units(&[0x41; 8]);
        let c = b.finish();
        assert_eq!(c.len(), MAX_TEXT_UNITS);
        assert_eq!(c[MAX_TEXT_UNITS - 1].unit, u16::from(b'B'));
    }

    #[test]
    fn surrogate_pairs_are_two_units() {
        let mut b = CharListBuilder::new();
        b.push_str("\u{1F600}");
        assert_eq!(b.chars().len(), 2);
    }
}
