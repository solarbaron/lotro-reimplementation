// SPDX-License-Identifier: GPL-3.0-or-later
//! Chat-type masks (`chat-window.md` §2): which tab takes a line (§4.3), the line style of a
//! mask (§5.1 step 2, `fonts.md` §9.3), the feedback sound (§5.6) and the chat type reported
//! to plugins (§9 Q18 Tessera rule).

use std::collections::BTreeMap;

use tessera_formats::property::PropertyValue;

use crate::props::{array_elements, as_argb, as_key, struct_field};

use super::super::vocab::GamePids;

/// The `Unfiltered` bit (chat type 25, bit 24).
pub const UNFILTERED: u64 = 1 << 24;

/// Most entries kept from a style, font, sound or opacity table (Tessera cap).
pub const MAX_TABLE: usize = 4096;

/// The mask bit of chat type `t` (bit *t* − 1), or `None` outside 1–64.
#[must_use]
pub fn type_bit(t: u32) -> Option<u64> {
    if (1..=64).contains(&t) {
        Some(1u64 << (t - 1))
    } else {
        None
    }
}

/// Whether a log with mask `tab` takes a line delivered with mask `line` (§4.3 step 2):
/// any common bit, bit 24 included.
#[must_use]
pub fn log_takes(tab: u64, line: u64) -> bool {
    tab & line != 0
}

/// The chat type reported to `Turbine.Chat.Received` for a line mask (§9 Q18 Tessera
/// rule): the type of the highest set bit other than bit 24, or `None` when there is none.
#[must_use]
pub fn reported_type(line: u64) -> Option<u32> {
    let t = line & !UNFILTERED;
    if t == 0 {
        None
    } else {
        Some(64 - t.leading_zeros())
    }
}

/// A line style (`fonts.md` §9.3): size step and colour `0xAARRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineStyle {
    /// Size step (0 Small … 5 XXXLarge).
    pub size_step: u32,
    /// Colour.
    pub colour: u32,
}

impl Default for LineStyle {
    /// Small, opaque white (`fonts.md` §9.3 step 1 with no entry).
    fn default() -> Self {
        Self {
            size_step: 0,
            colour: 0xFFFF_FFFF,
        }
    }
}

/// The chat-type style map and the size-step font map (`fonts.md` §9.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleMap {
    /// Chat type → style.
    pub styles: BTreeMap<u32, LineStyle>,
    /// Size step → font DID (entries with DID 0 skipped).
    pub fonts: BTreeMap<u32, u32>,
}

impl StyleMap {
    /// Reads `GOP_ChatTextTypeFontColorArray` (each entry: `GOP_ChatTextType`, a one-bit
    /// mask; `GOP_ChatTextTypeFont`; `GOP_ChatTextTypeColor`) and `UI_ChatFont_Array` (each
    /// entry `UI_ChatFont_FontEnum`, `UI_ChatFont_FontDataID`). A style entry's chat type
    /// is the highest set bit of its mask. Malformed entries are skipped.
    #[must_use]
    pub fn from_values(
        styles: Option<&PropertyValue>,
        fonts: Option<&PropertyValue>,
        p: &GamePids,
    ) -> Self {
        let mut m = Self::default();
        if let Some(v) = styles {
            for e in array_elements(v).iter().take(MAX_TABLE) {
                let mask = struct_field(&e.value, p.gop_text_type)
                    .and_then(PropertyValue::as_bits)
                    .unwrap_or(0);
                let Some(t) = reported_type(mask) else {
                    continue;
                };
                let size_step = struct_field(&e.value, p.gop_text_type_font).map_or(0, as_key);
                let colour = struct_field(&e.value, p.gop_text_type_color)
                    .and_then(as_argb)
                    .unwrap_or(0xFFFF_FFFF);
                m.styles.insert(t, LineStyle { size_step, colour });
            }
        }
        if let Some(v) = fonts {
            for e in array_elements(v).iter().take(MAX_TABLE) {
                let step = struct_field(&e.value, p.chat_font_enum).map_or(0, as_key);
                let did = match struct_field(&e.value, p.chat_font_did) {
                    Some(PropertyValue::DidRef(d)) => d.0,
                    _ => 0,
                };
                if did != 0 {
                    m.fonts.insert(step, did);
                }
            }
        }
        m
    }

    /// The style of a line mask (§5.1 step 2): of the bits set in `mask` (bit 24 already
    /// removed by the caller), the highest that has an entry; else Small, opaque white.
    #[must_use]
    pub fn style(&self, mask: u64) -> LineStyle {
        let mut bits = mask;
        while bits != 0 {
            let top = 63 - bits.leading_zeros();
            if let Some(s) = self.styles.get(&(top + 1)) {
                return *s;
            }
            bits &= !(1u64 << top);
        }
        LineStyle::default()
    }

    /// The font DID of a size step (`fonts.md` §9.3 step 2; 0 leaves the element's font).
    #[must_use]
    pub fn font(&self, size_step: u32) -> u32 {
        self.fonts.get(&size_step).copied().unwrap_or(0)
    }
}

/// The feedback-sound table (`ChatMarker_TextTypeFeedback`, §5.6): whole-mask matches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackSounds {
    /// Mask → sound DID.
    pub sounds: BTreeMap<u64, u32>,
}

impl FeedbackSounds {
    /// Reads `UI_Chat_ChatTextType_FeedbackSounds` (entries `UI_Chat_ChatTextType`,
    /// `UI_Chat_FeedbackSound`).
    #[must_use]
    pub fn from_value(v: &PropertyValue, p: &GamePids) -> Self {
        let mut s = Self::default();
        for e in array_elements(v).iter().take(MAX_TABLE) {
            let mask = struct_field(&e.value, p.feedback_mask).and_then(PropertyValue::as_bits);
            let did = struct_field(&e.value, p.feedback_sound).map(as_key);
            if let (Some(m), Some(d)) = (mask, did)
                && d != 0
            {
                s.sounds.insert(m, d);
            }
        }
        s
    }

    /// The sound for an appended line (§5.6): the line's mask without bit 24, looked up as
    /// one value (exact match only).
    #[must_use]
    pub fn sound(&self, line: u64) -> Option<u32> {
        self.sounds.get(&(line & !UNFILTERED)).copied()
    }
}

/// The background-opacity table (`UI_ChatOpacities`, §3.5): value → image DID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpacityTable {
    /// (min, max, image), inclusive ranges in stored order.
    pub ranges: Vec<(i32, i32, u32)>,
}

impl OpacityTable {
    /// Reads the array of `UI_ChatOpacity_Struct` (`UI_ChatOpacity_MinRange`,
    /// `UI_ChatOpacity_MaxRange`, `UI_ChatOpacity_ImageDataID`).
    #[must_use]
    pub fn from_value(v: &PropertyValue, p: &GamePids) -> Self {
        let mut t = Self::default();
        for e in array_elements(v).iter().take(MAX_TABLE) {
            let int = |pid| match struct_field(&e.value, pid) {
                Some(PropertyValue::Int(i)) => Some(*i),
                _ => None,
            };
            let (Some(lo), Some(hi)) = (int(p.opacity_min), int(p.opacity_max)) else {
                continue;
            };
            let img = struct_field(&e.value, p.opacity_image).map_or(0, as_key);
            t.ranges.push((lo, hi, img));
        }
        t
    }

    /// The image for an opacity value (§3.5 step 2). Later entries win where ranges
    /// overlap, as building the value → image map entry by entry does.
    #[must_use]
    pub fn image(&self, value: i32) -> Option<u32> {
        self.ranges
            .iter()
            .rev()
            .find(|(lo, hi, _)| *lo <= value && value <= *hi)
            .map(|(_, _, i)| *i)
            .filter(|i| *i != 0)
    }
}
