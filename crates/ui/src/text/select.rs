// SPDX-License-Identifier: GPL-3.0-or-later
//! How a text element's fonts are chosen (`fonts.md` §9.1–§9.2).
//!
//! A text element has two font slots, **normal** and **tag** (link labels only). Both start
//! with the locale default font; the `UICore_Text_*` font properties then replace it.
//! [`FontProps`] holds the values the element's inheritance chain resolved to (resolution
//! itself belongs to `ui-layout.md`), and [`FontSlots::apply`] fills the slots with the
//! rules of §9.1. [`FontIndexSession`] is the quest text size ladder of §9.2.

/// A value of a font-struct member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructMember {
    /// A DID.
    Did(u32),
    /// Any value that is not a DID (a "by property" source naming it changes nothing).
    Other,
}

/// A font-struct property (`UICore_Text_font_struct` or `UICore_Text_tag_font_struct`):
/// its members by PropertyId.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontStruct {
    /// Members as (PropertyId, value), in stored order.
    pub members: Vec<(u32, StructMember)>,
}

impl FontStruct {
    /// The DID held by member `id`, if it is one.
    #[must_use]
    pub fn did(&self, id: u32) -> Option<u32> {
        self.members
            .iter()
            .find(|(k, _)| *k == id)
            .and_then(|(_, v)| match v {
                StructMember::Did(d) => Some(*d),
                StructMember::Other => None,
            })
    }
}

/// The font properties of one slot (`fonts.md` §9.1 table).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlotSource {
    /// Array of fonts: `UICore_Text_fonts` / `UICore_Text_tag_fonts`.
    pub fonts: Option<Vec<u32>>,
    /// Struct holding one font: `UICore_Text_font_struct` / `UICore_Text_tag_font_struct`.
    pub font_struct: Option<FontStruct>,
    /// Single font: `UICore_Text_font` / `UICore_Text_tag_font`.
    pub font: Option<u32>,
    /// By property: `UICore_Text_set_normal_font_by_property` /
    /// `UICore_Text_set_tag_font_by_property` (a PropertyId).
    pub by_property: Option<u32>,
}

impl SlotSource {
    /// Whether the slot has any font property of its own, as stored. [`FontSlots::apply`]
    /// does not count a by-property value of 0 (it names no member, §9.1 rule 5).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_none()
            && self.font_struct.is_none()
            && self.font.is_none()
            && self.by_property.is_none()
    }

    /// The by-property source, without the value 0 that names no member.
    fn by_property(&self) -> Option<u32> {
        self.by_property.filter(|&p| p != 0)
    }

    /// Whether the slot has a font property of its own that can change it: a by-property
    /// value of 0 "names no member and changes nothing" (§9.1 rule 5), so on its own it does
    /// not stop the tag slot from using the normal slot's properties (rule 2).
    fn has_own(&self) -> bool {
        self.fonts.is_some()
            || self.font_struct.is_some()
            || self.font.is_some()
            || self.by_property().is_some()
    }
}

/// The font properties of a text element after inheritance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontProps {
    /// The normal slot's properties.
    pub normal: SlotSource,
    /// The tag slot's properties.
    pub tag: SlotSource,
    /// `UICore_Text_set_font_by_property`: a PropertyId for both slots (rule 5).
    pub set_font_by_property: Option<u32>,
    /// PropertyId of `UICore_Text_font`, the default member of the normal font struct.
    pub font_member: u32,
    /// PropertyId of `UICore_Text_tag_font`, the default member of the tag font struct.
    pub tag_font_member: u32,
}

/// The two font slots of a text element, as font DIDs (`fonts.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontSlots {
    /// The normal slot.
    pub normal: u32,
    /// The tag slot.
    pub tag: u32,
}

impl FontSlots {
    /// Both slots loaded with the locale default font (the locale record's font DID).
    #[must_use]
    pub const fn new(locale_default: u32) -> Self {
        Self {
            normal: locale_default,
            tag: locale_default,
        }
    }

    /// Fills the slots from `props` at font index `index` (§9.2; 0 unless a panel changes
    /// it). `resolves` says whether a DID names a font record that loads.
    ///
    /// Per slot the sources apply in this order, each later one replacing the earlier
    /// (Tessera order; see the implementation OPEN in `fonts.md` §9.1): the single font,
    /// the struct (read through the by-property member when one is set, else the default
    /// member), the array entry at `index`. A DID of 0 or one that does not resolve leaves the slot unchanged
    /// (rule 4). The tag slot uses the normal slot's properties when it has none of its own
    /// (rule 2), and its by-property source reads the normal struct when it has no struct
    /// of its own (rule 5). A by-property value of 0 names no member and changes nothing
    /// (rule 5): it is ignored, also when deciding whether the tag slot has properties of
    /// its own, and it does not stop the default member from being read.
    pub fn apply(&mut self, props: &FontProps, index: usize, resolves: impl Fn(u32) -> bool) {
        let set = |slot: &mut u32, did: Option<u32>| {
            if let Some(d) = did
                && d != 0
                && resolves(d)
            {
                *slot = d;
            }
        };
        let normal = &props.normal;
        let tag_own = props.tag.has_own();
        let tag = if tag_own { &props.tag } else { normal };
        let tag_member = if tag_own {
            props.tag_font_member
        } else {
            props.font_member
        };
        for (slot, src, default_member, struct_fallback) in [
            (&mut self.normal, normal, props.font_member, None),
            (&mut self.tag, tag, tag_member, normal.font_struct.as_ref()),
        ] {
            set(slot, src.font);
            let by = src
                .by_property()
                .or(props.set_font_by_property.filter(|&p| p != 0));
            let st = src.font_struct.as_ref();
            match by {
                // Rule 5: a member of the slot's own struct (the tag slot falls back to the
                // normal struct); a missing struct or member, or a non-DID, changes nothing.
                Some(member) => set(slot, st.or(struct_fallback).and_then(|s| s.did(member))),
                None => set(slot, st.and_then(|s| s.did(default_member))),
            }
            // Rule 1: the entry at the font index; an index past the end changes nothing.
            set(slot, src.fonts.as_ref().and_then(|a| a.get(index).copied()));
        }
    }
}

/// The session font index of the quest panels' text size buttons (`fonts.md` §9.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FontIndexSession {
    index: usize,
}

impl FontIndexSession {
    /// A session starting at index 0 (not saved across sessions).
    #[must_use]
    pub const fn new() -> Self {
        Self { index: 0 }
    }

    /// The current index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Offers `requested` to elements whose font arrays have the lengths `array_lens`
    /// (step 3); it becomes the session index when at least one array is long enough
    /// (step 4). A negative request is refused. Returns whether it was taken.
    pub fn request(&mut self, requested: i64, array_lens: &[usize]) -> bool {
        let Ok(r) = usize::try_from(requested) else {
            return false;
        };
        if array_lens.iter().any(|&l| r < l) {
            self.index = r;
            true
        } else {
            false
        }
    }

    /// The increase button: request index + 1.
    pub fn increase(&mut self, array_lens: &[usize]) -> bool {
        let r = i64::try_from(self.index)
            .unwrap_or(i64::MAX)
            .saturating_add(1);
        self.request(r, array_lens)
    }

    /// The decrease button: request index − 1.
    pub fn decrease(&mut self, array_lens: &[usize]) -> bool {
        let r = i64::try_from(self.index).unwrap_or(i64::MAX) - 1;
        self.request(r, array_lens)
    }
}
