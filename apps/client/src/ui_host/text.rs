// SPDX-License-Identifier: GPL-3.0-or-later
//! Text shaping for `tessera-ui` (`fonts.md` §7, §9.1): a text element's properties turned
//! into font slots, a character list and a line layout, on the main thread, once the fonts
//! the slots name have been decoded on the UI workers.
//!
//! - Fonts: both slots start with the locale default font, then [`FontSlots::apply`] reads
//!   the element's font properties at font index 0 (§9.1, §9.2). Without an install (no
//!   locale font) the normal slot uses Tessera's fallback font (§11 item 2), so plugin
//!   labels of synthetic runs are readable.
//! - Characters: the text in the normal slot and `UICore_Text_font_color`.
//! - Layout: margins, justification, the wrap switch, leading and truncation from the
//!   element's properties (§7).
//!
//! With `UICore_Text_parse_markup_tags` true, colour, underline and list markup and link
//! labels are interpreted through `tessera-text`'s reader (§9.1). Without it the text is
//! drawn as it is, so a link label is not recognised then (§9.1 says links are recognised
//! whatever that flag says; a recorded client gap). The locale leading factor *f*₀ (§7.1)
//! is read from the locale record at UI start.

use std::sync::Arc;

use tessera_formats::PropertyDictionary;
use tessera_formats::property::PropertyValue;
use tessera_ui::env::{PreparedText, TextRequest};
use tessera_ui::props::{array_elements, as_argb, as_bool, as_float, as_int, as_key};
use tessera_ui::text::{
    CharListBuilder, FontProps, FontSlots, FontStruct, JUSTIFY_LEFT, JUSTIFY_TOP, LayoutParams,
    Margins, SLOT_NORMAL, SLOT_TAG, SlotSource, StructMember, layout, leading_factor,
};

use super::env::EnvState;

/// Opaque white, the colour of text without a colour property.
pub const DEFAULT_TEXT_COLOUR: u32 = 0xFFFF_FFFF;
/// Opaque black, the default outline colour (`fonts.md` §8.4).
pub const DEFAULT_OUTLINE_COLOUR: u32 = 0xFF00_0000;

macro_rules! pid_table {
    ($(#[$m:meta])* $name:ident { $($field:ident = $label:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            $(#[doc = concat!("`", $label, "`.")] pub $field: u32,)*
        }

        impl Default for $name {
            fn default() -> Self {
                let mut n = 0xFFFD_0000u32;
                let mut next = || { n += 1; n };
                Self { $($field: next(),)* }
            }
        }

        impl $name {
            /// The IDs resolved through `dict`; unresolved names keep a placeholder in
            /// 0xFFFD_xxxx that no dictionary uses.
            #[must_use]
            pub fn resolve(dict: &PropertyDictionary) -> Self {
                let mut t = Self::default();
                $(if let Some(v) = dict.pid($label) { t.$field = v; })*
                t
            }

            /// Every (label, value) pair.
            #[must_use]
            pub fn entries(&self) -> Vec<(&'static str, u32)> {
                vec![$(($label, self.$field),)*]
            }
        }
    };
}

pid_table! {
    /// Property IDs of the text properties the shaper reads (property-dictionary names).
    TextPids {
        font = "UICore_Text_font",
        fonts = "UICore_Text_fonts",
        font_struct = "UICore_Text_font_struct",
        normal_by_property = "UICore_Text_set_normal_font_by_property",
        tag_font = "UICore_Text_tag_font",
        tag_fonts = "UICore_Text_tag_fonts",
        tag_font_struct = "UICore_Text_tag_font_struct",
        tag_by_property = "UICore_Text_set_tag_font_by_property",
        by_property = "UICore_Text_set_font_by_property",
        font_color = "UICore_Text_font_color",
        outline = "UICore_Text_outline",
        outline_color = "UICore_Text_font_outline_color",
        horizontal = "UICore_Text_horizontal_justification",
        vertical = "UICore_Text_vertical_justification",
        one_line = "UICore_Text_one_line",
        extra_leading = "UICore_Text_extra_leading",
        last_line_leading = "UICore_Text_last_line_leading",
        left_margin = "UICore_Text_left_margin",
        right_margin = "UICore_Text_right_margin",
        top_margin = "UICore_Text_top_margin",
        bottom_margin = "UICore_Text_bottom_margin",
        markup = "UICore_Text_parse_markup_tags",
        tag_font_color = "UICore_Text_tag_font_color",
    }
}

impl TextPids {
    /// The plugin layer's text property IDs (`tessera-plugins` `TextPids`).
    #[must_use]
    pub fn for_plugins(&self) -> tessera_plugins::ui::TextPids {
        tessera_plugins::ui::TextPids {
            font: self.font,
            font_color: self.font_color,
            outline_color: self.outline_color,
            outline: self.outline,
            horizontal: self.horizontal,
            vertical: self.vertical,
            one_line: self.one_line,
            markup: self.markup,
        }
    }
}

/// Shaping state (the property IDs).
#[derive(Debug, Clone, Default)]
pub struct TextShaper {
    pids: TextPids,
}

impl TextShaper {
    /// A shaper reading `pids`.
    #[must_use]
    pub fn new(pids: TextPids) -> Self {
        Self { pids }
    }

    /// The property IDs.
    #[must_use]
    pub fn pids(&self) -> TextPids {
        self.pids
    }
}

fn get(props: &[(u32, PropertyValue)], pid: u32) -> Option<&PropertyValue> {
    props.iter().find(|(p, _)| *p == pid).map(|(_, v)| v)
}

fn did_of(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::DidRef(d) => Some(d.0),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        PropertyValue::Enum(k) => Some(*k),
        _ => None,
    }
}

fn did_array(v: &PropertyValue) -> Vec<u32> {
    array_elements(v)
        .iter()
        .filter_map(|e| did_of(&e.value))
        .take(64)
        .collect()
}

fn font_struct(v: &PropertyValue) -> Option<FontStruct> {
    match v {
        PropertyValue::Struct(s) => Some(FontStruct {
            members: s
                .fields
                .iter()
                .take(64)
                .map(|f| {
                    (
                        f.pid,
                        match &f.value {
                            PropertyValue::DidRef(d) => StructMember::Did(d.0),
                            _ => StructMember::Other,
                        },
                    )
                })
                .collect(),
        }),
        _ => None,
    }
}

fn slot_source(
    props: &[(u32, PropertyValue)],
    fonts: u32,
    st: u32,
    single: u32,
    by: u32,
) -> SlotSource {
    SlotSource {
        fonts: get(props, fonts).map(did_array),
        font_struct: get(props, st).and_then(font_struct),
        font: get(props, single).and_then(did_of),
        by_property: get(props, by).map(|v| match v {
            PropertyValue::PropertyId(p) => *p,
            other => as_key(other),
        }),
    }
}

/// The font properties of an element (`fonts.md` §9.1 table).
#[must_use]
pub fn font_props(pids: &TextPids, props: &[(u32, PropertyValue)]) -> FontProps {
    FontProps {
        normal: slot_source(
            props,
            pids.fonts,
            pids.font_struct,
            pids.font,
            pids.normal_by_property,
        ),
        tag: slot_source(
            props,
            pids.tag_fonts,
            pids.tag_font_struct,
            pids.tag_font,
            pids.tag_by_property,
        ),
        set_font_by_property: get(props, pids.by_property).map(|v| match v {
            PropertyValue::PropertyId(p) => *p,
            other => as_key(other),
        }),
        font_member: pids.font,
        tag_font_member: pids.tag_font,
    }
}

/// The layout parameters of an element (`fonts.md` §7).
#[must_use]
pub fn layout_params(
    pids: &TextPids,
    props: &[(u32, PropertyValue)],
    size: (i32, i32),
    wrap_width: Option<i32>,
    locale_leading: f32,
) -> LayoutParams {
    let int = |pid| get(props, pid).map_or(0, as_int).clamp(-65_536, 65_536);
    LayoutParams {
        width: wrap_width.unwrap_or(size.0),
        height: size.1,
        margins: Margins {
            left: int(pids.left_margin),
            right: int(pids.right_margin),
            top: int(pids.top_margin),
            bottom: int(pids.bottom_margin),
        },
        one_line: get(props, pids.one_line).is_some_and(as_bool),
        horizontal: get(props, pids.horizontal).map_or(JUSTIFY_LEFT, as_key),
        vertical: get(props, pids.vertical).map_or(JUSTIFY_TOP, as_key),
        leading: leading_factor(
            locale_leading,
            get(props, pids.extra_leading).map_or(0.0, as_float),
        ),
        last_line_leading: get(props, pids.last_line_leading).is_some_and(as_bool),
        list_extra: 0,
    }
}

/// Appends `text` with its markup interpreted (`fonts.md` §9.1, `string-table.md`
/// §4.7–§4.8, through `tessera-text`'s reader): colour runs take their colour, underlined
/// runs an underline in their colour, and link labels the tag slot in the tag colour unless
/// the link brings its own colour.
fn push_markup(b: &mut CharListBuilder, text: &str, colour: u32, tag_colour: u32) {
    let rich = tessera_text::parse_markup(text);
    let mut in_list = false;
    for span in rich.spans() {
        let st = &span.style;
        let own = st.color.map(|c| u32::from_be_bytes([c.a, c.r, c.g, c.b]));
        let (slot, c) = if st.link.is_some() {
            (SLOT_TAG, own.unwrap_or(tag_colour))
        } else {
            (SLOT_NORMAL, own.unwrap_or(colour))
        };
        if st.list_item && !in_list {
            b.list_item(true);
        }
        in_list = st.list_item;
        b.slot(slot)
            .colour(c)
            .underline(st.underline.then_some(c))
            .push_str(&span.text);
    }
}

/// Shapes one request (module docs). `None` while a needed font is still loading, or when
/// no font is available at all.
pub(crate) fn shape(st: &mut EnvState, req: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
    let pids = st.text_pids();
    let props = req.props;
    let locale = st.data().and_then(|d| d.locale_font).unwrap_or(0);
    let fp = font_props(&pids, props);
    let mut slots = FontSlots::new(locale);
    slots.apply(&fp, 0, |d| st.font_resolves(d));
    let normal = if slots.normal != 0 {
        match st.font(slots.normal) {
            Some(f) => Some(f),
            None if st.font_resolves(slots.normal) => return None,
            None => None,
        }
    } else {
        None
    };
    let normal = match normal {
        Some(f) => Some(f),
        None if locale == 0 => st.fallback_font(),
        None => None,
    };
    let tag = if slots.tag != 0 && slots.tag != slots.normal {
        st.font(slots.tag)
    } else {
        normal.clone()
    };
    let colour = get(props, pids.font_color)
        .and_then(as_argb)
        .unwrap_or(DEFAULT_TEXT_COLOUR);
    let mut b = CharListBuilder::new();
    if get(props, pids.markup).is_some_and(as_bool) {
        let tag_colour = get(props, pids.tag_font_color)
            .and_then(as_argb)
            .unwrap_or(colour);
        push_markup(&mut b, req.text, colour, tag_colour);
    } else {
        b.slot(SLOT_NORMAL).colour(colour).push_str(req.text);
    }
    if b.overflowed() {
        return None;
    }
    let chars = b.finish();
    let fonts = vec![normal, tag];
    let f0 = st.data().map_or(0.0, |d| d.locale_leading);
    let params = layout_params(&pids, props, req.size, req.wrap_width, f0);
    let lay = layout(&chars, &fonts, &params, None).ok()?;
    let truncated = lay.is_truncated();
    let extent = (lay.content_width, lay.content_height);
    Some(Arc::new(PreparedText {
        layout: lay,
        chars,
        fonts,
        outline: get(props, pids.outline).is_some_and(as_bool),
        outline_colour: get(props, pids.outline_color)
            .and_then(as_argb)
            .unwrap_or(DEFAULT_OUTLINE_COLOUR),
        truncated,
        extent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::did::Did;

    #[test]
    fn placeholders_are_distinct() {
        let p = TextPids::default();
        let mut v: Vec<u32> = p.entries().into_iter().map(|(_, v)| v).collect();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n);
    }

    #[test]
    fn font_and_layout_properties() {
        let p = TextPids::default();
        let props = vec![
            (p.font, PropertyValue::DidRef(Did(0x4200_0001))),
            (p.horizontal, PropertyValue::Enum(1)),
            (p.left_margin, PropertyValue::Int(4)),
            (p.one_line, PropertyValue::Bool(true)),
        ];
        let fp = font_props(&p, &props);
        assert_eq!(fp.normal.font, Some(0x4200_0001));
        let mut slots = FontSlots::new(0x4200_0028);
        slots.apply(&fp, 0, |_| true);
        assert_eq!(slots.normal, 0x4200_0001);
        // Rule 2: the tag slot takes the normal slot's properties.
        assert_eq!(slots.tag, 0x4200_0001);
        let lp = layout_params(&p, &props, (100, 20), None, 0.0);
        assert_eq!(lp.horizontal, 1);
        assert_eq!(lp.margins.left, 4);
        assert!(lp.one_line);
        assert_eq!(lp.vertical, JUSTIFY_TOP);
        let lp = layout_params(&p, &props, (100, 20), Some(40), 0.0);
        assert_eq!(lp.width, 40);
    }

    #[test]
    fn fallback_font_shapes_without_an_install() {
        let mut st = EnvState::new(None, 1, 1 << 20, 1 << 24);
        let p = st.text_pids();
        let props = vec![(p.font_color, PropertyValue::Colour([0, 0, 255, 255]))];
        let req = TextRequest {
            text: "Hello",
            props: &props,
            size: (200, 30),
            wrap_width: None,
        };
        let t = shape(&mut st, &req).expect("shaped with the fallback font");
        assert_eq!(t.chars.len(), 5);
        let props = vec![(p.markup, PropertyValue::Bool(true))];
        let req = TextRequest {
            text: "a<rgb=#FF0000>b</rgb><u>c</u>",
            props: &props,
            size: (200, 30),
            wrap_width: None,
        };
        let t = shape(&mut st, &req).expect("markup shaped");
        assert_eq!(t.chars.len(), 3, "tags removed");
        assert_eq!(t.chars[1].colour, 0xFFFF_0000);
        assert_eq!(t.chars[2].underline, Some(DEFAULT_TEXT_COLOUR));
        assert!(t.fonts[0].is_some());
        assert!(t.extent.0 > 0);
    }
}
