// SPDX-License-Identifier: GPL-3.0-or-later
//! `Label`, `TextBox`, `Button`, `CheckBox` and their game-styled variants
//! (`lua-plugin-api.md` §13.4–§13.6, §14.2.2).

use std::rc::Rc;

use mlua::{Table, Value};
use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_ui::control::plugin_colour;

use super::control::{ctl_get, with_ctl, with_ui};
use super::model::{Args, Kind, ObjData, Styled, TextState, int, new_colour, truthy};
use super::reg::{Cx, class, control_ctor, ctl_of, def};
use super::state::{Pending, Shared, class_table};
use crate::api::{NativeApi, script_error};

fn texty(d: &ObjData) -> bool {
    ctl_of(
        d,
        &[Kind::Label, Kind::TextBox, Kind::Button, Kind::CheckBox],
    )
}

fn text_box(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::TextBox])
}

fn check_box(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::CheckBox])
}

/// A text argument as Lua's `tostring` would give it for strings and numbers; anything
/// else is the empty string.
pub(crate) fn text_arg(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.to_string_lossy(),
        Some(Value::Number(n)) => tessera_script::format_number_g14(*n),
        Some(Value::Integer(i)) => i.to_string(),
        _ => String::new(),
    }
}

/// Length in UTF-16 code units (§13.4: the client's units).
pub(crate) fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Split `s` at UTF-16 unit `u` (a unit inside a surrogate pair moves to the end of the
/// character).
fn byte_at(s: &str, u: usize) -> usize {
    let mut n = 0;
    for (i, ch) in s.char_indices() {
        if n >= u {
            return i;
        }
        n += ch.len_utf16();
    }
    s.len()
}

fn sub_units(s: &str, start: usize, len: usize) -> &str {
    let a = byte_at(s, start);
    let b = byte_at(s, start.saturating_add(len));
    &s[a..b.max(a)]
}

fn ts<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut TextState) -> R) -> Option<R> {
    with_ctl(sh, id, |c| c.text.as_deref_mut().map(f)).flatten()
}

/// The current text: for a text box, the element's text (user edits; the layer's copy and
/// the caret estimate follow it, see `events::text_edited`).
pub(crate) fn current_text(sh: &Shared, id: u64) -> String {
    let Some((key, kind)) = ctl_get(sh, id, |c| (c.text_key(), c.kind)) else {
        return String::new();
    };
    if kind == Kind::TextBox
        && let Some(t) = with_ui(sh, |u| u.text_of(key))
    {
        let old = ts(sh, id, |s| {
            if s.text != t {
                s.sel_start = super::events::edit_caret(&s.text, &t);
                s.sel_len = 0;
            }
            std::mem::replace(&mut s.text, t.clone()).len()
        })
        .unwrap_or(0);
        let mut st = sh.st.borrow_mut();
        if let Some(ap) = st.objs.get(&id).map(|o| o.apartment.clone()) {
            let n = st.text_bytes.entry(ap).or_insert(0);
            *n = n.saturating_sub(old).saturating_add(t.len());
        }
        return t;
    }
    ts(sh, id, |s| s.text.clone()).unwrap_or_default()
}

/// Most UTF-16 units one text keeps (the UI's own bound, `tessera_ui::text::MAX_TEXT_UNITS`);
/// longer text is cut at a character boundary (Tessera bound).
pub(crate) const MAX_TEXT_UNITS: usize = tessera_ui::text::MAX_TEXT_UNITS;

/// Most bytes of control text one apartment keeps at a time (Tessera bound, hostile
/// input: the layer's copies live outside the apartment's memory cap).
pub const MAX_TEXT_BYTES_PER_APARTMENT: usize = 64 << 20;

/// Cut `text` to [`MAX_TEXT_UNITS`] units at a character boundary.
pub(crate) fn bound_text(text: String) -> String {
    if text.len() <= MAX_TEXT_UNITS {
        return text;
    }
    let cut = byte_at(&text, MAX_TEXT_UNITS);
    text[..cut].to_owned()
}

/// Most bytes of a window caption or menu-item text (Tessera bound).
pub(crate) const MAX_TITLE_BYTES: usize = 4096;

/// Cut a caption to [`MAX_TITLE_BYTES`] at a character boundary.
pub(crate) fn bound_title(text: String) -> String {
    if text.len() <= MAX_TITLE_BYTES {
        return text;
    }
    let mut end = MAX_TITLE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Store and show `text`, keeping the selection inside it.
///
/// # Errors
///
/// A script error when the apartment's text would exceed
/// [`MAX_TEXT_BYTES_PER_APARTMENT`].
pub(crate) fn store_text(sh: &Shared, id: u64, text: String) -> mlua::Result<()> {
    let Some((key, kind)) = ctl_get(sh, id, |c| (c.text_key(), c.kind)) else {
        return Ok(());
    };
    let text = bound_text(text);
    // §9.6 OPEN (Tessera rule): a text box's `TextChanged` goes through the queue when
    // the text actually changes.
    let fire =
        kind == Kind::TextBox && with_ui(sh, |u| u.text_of(key)).is_some_and(|old| old != text);
    {
        let mut st = sh.st.borrow_mut();
        let Some(ap) = st.objs.get(&id).map(|o| o.apartment.clone()) else {
            return Ok(());
        };
        let old = match st.objs.get(&id).map(|o| &o.data) {
            Some(ObjData::Ctl(c)) => c.text.as_deref().map_or(0, |t| t.text.len()),
            _ => 0,
        };
        let used = st
            .text_bytes
            .get(&ap)
            .copied()
            .unwrap_or(0)
            .saturating_sub(old);
        if used.saturating_add(text.len()) > MAX_TEXT_BYTES_PER_APARTMENT {
            return Err(script_error("too much text in user-interface controls"));
        }
        st.text_bytes.insert(ap, used + text.len());
    }
    let n = units(&text);
    ts(sh, id, |s| {
        s.text = text.clone();
        s.sel_start = s.sel_start.min(n);
        s.sel_len = s.sel_len.min(n - s.sel_start);
    });
    with_ui(sh, |u| u.set_text(key, Some(text)));
    if fire {
        sh.push(Pending::Fire {
            target: id,
            name: "TextChanged",
            args: Args::Empty,
        });
    }
    Ok(())
}

fn set_prop(cx: &Cx<'_>, pid: u32, v: PropertyValue) {
    if let Some(k) = ctl_get(cx.sh, cx.id, |c| c.text_key()) {
        with_ui(cx.sh, |u| u.set_property(k, pid, v));
    }
}

/// The justification pair of a `ContentAlignment` value (§13.4 table): (horizontal,
/// vertical), and whether the stored value becomes 0.
#[must_use]
pub(crate) fn justification(a: u32) -> ((u32, u32), bool) {
    match a {
        1 => ((5, 2), false),
        2 => ((3, 2), false),
        3 => ((4, 2), false),
        4 => ((5, 3), false),
        5 => ((3, 3), false),
        6 => ((4, 3), false),
        7 => ((5, 1), false),
        8 => ((3, 1), false),
        9 => ((4, 1), false),
        0 => ((5, 2), false),
        _ => ((5, 2), true),
    }
}

fn colour_value(c: [f32; 4]) -> PropertyValue {
    PropertyValue::Colour(plugin_colour(c[0], c[1], c[2], c[3]).to_le_bytes())
}

/// Install the text classes.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    styled_ns: &Table,
) -> mlua::Result<()> {
    let lua = api.lua();
    let scrollable = class_table(lua, "ScrollableControl")?;
    let label = class(
        api,
        sh,
        ui,
        "Label",
        "Label",
        Some(&scrollable),
        control_ctor(Kind::Label, Styled::Plain),
    )?;
    let text_box_cls = class(
        api,
        sh,
        ui,
        "TextBox",
        "TextBox",
        Some(&label),
        control_ctor(Kind::TextBox, Styled::Plain),
    )?;
    let button = class(
        api,
        sh,
        ui,
        "Button",
        "Button",
        Some(&label),
        control_ctor(Kind::Button, Styled::Plain),
    )?;
    let check = class(
        api,
        sh,
        ui,
        "CheckBox",
        "CheckBox",
        Some(&label),
        control_ctor(Kind::CheckBox, Styled::Plain),
    )?;
    let lbutton = class(
        api,
        sh,
        styled_ns,
        "Button",
        "Lotro.Button",
        Some(&button),
        control_ctor(Kind::Button, Styled::Silver),
    )?;
    class(
        api,
        sh,
        styled_ns,
        "GoldButton",
        "Lotro.GoldButton",
        Some(&lbutton),
        control_ctor(Kind::Button, Styled::Gold),
    )?;
    class(
        api,
        sh,
        styled_ns,
        "CheckBox",
        "Lotro.CheckBox",
        Some(&check),
        control_ctor(Kind::CheckBox, Styled::Silver),
    )?;
    class(
        api,
        sh,
        styled_ns,
        "TextBox",
        "Lotro.TextBox",
        Some(&text_box_cls),
        control_ctor(Kind::TextBox, Styled::Silver),
    )?;

    let t = texty;
    def(api, sh, &label, "GetText", t, |cx, _| {
        Ok(current_text(cx.sh, cx.id))
    })?;
    def(api, sh, &label, "SetText", t, |cx, args| {
        store_text(cx.sh, cx.id, text_arg(args.front()))
    })?;
    def(api, sh, &label, "AppendText", t, |cx, args| {
        let mut s = current_text(cx.sh, cx.id);
        s.push_str(&text_arg(args.front()));
        store_text(cx.sh, cx.id, s)
    })?;
    def(api, sh, &label, "InsertText", t, |cx, args| {
        let s = current_text(cx.sh, cx.id);
        let ins = text_arg(args.front());
        let (start, len) = ts(cx.sh, cx.id, |t| (t.sel_start, t.sel_len)).unwrap_or((0, 0));
        let a = byte_at(&s, start);
        let b = byte_at(&s, start + len);
        let out = format!("{}{}{}", &s[..a], ins, &s[b.max(a)..]);
        let caret = start + units(&ins);
        store_text(cx.sh, cx.id, out)?;
        ts(cx.sh, cx.id, |t| {
            t.sel_start = caret.min(units(&t.text));
            t.sel_len = 0;
        });
        Ok(())
    })?;
    def(api, sh, &label, "GetTextLength", t, |cx, _| {
        Ok(units(&current_text(cx.sh, cx.id)) as f64)
    })?;
    def(api, sh, &label, "GetFont", t, |cx, _| {
        Ok(f64::from(ts(cx.sh, cx.id, |t| t.font).unwrap_or(0)))
    })?;
    def(api, sh, &label, "SetFont", t, |cx, args| {
        let f = super::model::num(args.front()).unwrap_or(0.0);
        let f = if f.is_finite() && (0.0..4_294_967_296.0).contains(&f) {
            f.trunc() as u32
        } else {
            0
        };
        // Only the DID of a font record (type 0x42) is taken; anything else is ignored.
        let same = ts(cx.sh, cx.id, |t| t.font == f).unwrap_or(true);
        if same || f >> 24 != 0x42 {
            return Ok(());
        }
        ts(cx.sh, cx.id, |t| t.font = f);
        let pid = cx.sh.st.borrow().text_pids.font;
        set_prop(cx, pid, PropertyValue::DidRef(Did(f)));
        Ok(())
    })?;
    def(api, sh, &label, "GetFontStyle", t, |cx, _| {
        Ok(f64::from(ts(cx.sh, cx.id, |t| t.font_style).unwrap_or(0)))
    })?;
    def(api, sh, &label, "SetFontStyle", t, |cx, args| {
        let n = int(args.front()) as u32;
        ts(cx.sh, cx.id, |t| t.font_style = n);
        let pid = cx.sh.st.borrow().text_pids.outline;
        set_prop(cx, pid, PropertyValue::Bool(n & 8 != 0));
        Ok(())
    })?;
    def(api, sh, &label, "GetForeColor", t, |cx, _| {
        let c = ts(cx.sh, cx.id, |t| t.fore)
            .flatten()
            .unwrap_or([1.0, 0.0, 0.0, 0.0]);
        new_colour(cx.lua, c)
    })?;
    def(api, sh, &label, "SetForeColor", t, |cx, args| {
        let cls = class_table(cx.lua, "Color")?;
        if !super::colour::is_colour(args.front(), &cls) {
            return Err(script_error("SetForeColor: a Color is required"));
        }
        let c = super::colour::read(args.front()).unwrap_or_default();
        ts(cx.sh, cx.id, |t| t.fore = Some(c));
        let pid = cx.sh.st.borrow().text_pids.font_color;
        set_prop(cx, pid, colour_value(c));
        Ok(())
    })?;
    def(api, sh, &label, "GetOutlineColor", t, |cx, _| {
        let c = ts(cx.sh, cx.id, |t| t.outline).unwrap_or([1.0, 0.0, 0.0, 0.0]);
        new_colour(cx.lua, c)
    })?;
    def(api, sh, &label, "SetOutlineColor", t, |cx, args| {
        let cls = class_table(cx.lua, "Color")?;
        if !super::colour::is_colour(args.front(), &cls) {
            return Err(script_error("SetOutlineColor: a Color is required"));
        }
        let c = super::colour::read(args.front()).unwrap_or_default();
        ts(cx.sh, cx.id, |t| t.outline = c);
        let pid = cx.sh.st.borrow().text_pids.outline_color;
        set_prop(cx, pid, colour_value(c));
        Ok(())
    })?;
    def(api, sh, &label, "GetTextAlignment", t, |cx, _| {
        Ok(f64::from(ts(cx.sh, cx.id, |t| t.alignment).unwrap_or(1)))
    })?;
    def(api, sh, &label, "SetTextAlignment", t, |cx, args| {
        let a = int(args.front()) as u32;
        if ts(cx.sh, cx.id, |t| t.alignment == a).unwrap_or(true) {
            return Ok(());
        }
        let ((h, v), zero) = justification(a);
        ts(cx.sh, cx.id, |t| t.alignment = if zero { 0 } else { a });
        let p = cx.sh.st.borrow().text_pids;
        set_prop(cx, p.horizontal, PropertyValue::Enum(h));
        set_prop(cx, p.vertical, PropertyValue::Enum(v));
        Ok(())
    })?;
    def(api, sh, &label, "IsMultiline", t, |cx, _| {
        Ok(ts(cx.sh, cx.id, |t| t.multiline).unwrap_or(true))
    })?;
    def(api, sh, &label, "SetMultiline", t, |cx, args| {
        let v = truthy(args.front());
        ts(cx.sh, cx.id, |t| t.multiline = v);
        let pid = cx.sh.st.borrow().text_pids.one_line;
        set_prop(cx, pid, PropertyValue::Bool(!v));
        Ok(())
    })?;
    def(api, sh, &label, "IsMarkupEnabled", t, |cx, _| {
        Ok(ts(cx.sh, cx.id, |t| t.markup).unwrap_or(false))
    })?;
    def(api, sh, &label, "SetMarkupEnabled", t, |cx, args| {
        let v = truthy(args.front());
        ts(cx.sh, cx.id, |t| t.markup = v);
        let pid = cx.sh.st.borrow().text_pids.markup;
        set_prop(cx, pid, PropertyValue::Bool(v));
        Ok(())
    })?;
    def(api, sh, &label, "IsSelectable", t, |cx, _| {
        Ok(ts(cx.sh, cx.id, |t| t.selectable).unwrap_or(false))
    })?;
    def(api, sh, &label, "SetSelectable", t, |cx, args| {
        let v = truthy(args.front());
        ts(cx.sh, cx.id, |t| t.selectable = v);
        if let Some(k) = ctl_get(cx.sh, cx.id, |c| c.text_key()) {
            with_ui(cx.sh, |u| {
                let pid = u.vocab().pids.text_selectable;
                u.set_property(k, pid, PropertyValue::Bool(v));
            });
        }
        Ok(())
    })?;

    // Selection (§13.4: 1-based positions in UTF-16 units).
    def(api, sh, &label, "GetSelectionStart", t, |cx, _| {
        current_text(cx.sh, cx.id);
        Ok(ts(cx.sh, cx.id, |t| t.sel_start).unwrap_or(0) as f64 + 1.0)
    })?;
    def(api, sh, &label, "GetSelectionLength", t, |cx, _| {
        current_text(cx.sh, cx.id);
        Ok(ts(cx.sh, cx.id, |t| t.sel_len).unwrap_or(0) as f64)
    })?;
    def(api, sh, &label, "GetSelectedText", t, |cx, _| {
        let s = current_text(cx.sh, cx.id);
        let (a, n) = ts(cx.sh, cx.id, |t| (t.sel_start, t.sel_len)).unwrap_or((0, 0));
        Ok(sub_units(&s, a, n).to_owned())
    })?;
    def(api, sh, &label, "SetSelectedText", t, |cx, args| {
        let s = current_text(cx.sh, cx.id);
        let ins = text_arg(args.front());
        let (start, len) = ts(cx.sh, cx.id, |t| (t.sel_start, t.sel_len)).unwrap_or((0, 0));
        let a = byte_at(&s, start);
        let b = byte_at(&s, start + len);
        let out = format!("{}{}{}", &s[..a], ins, &s[b.max(a)..]);
        let n = units(&ins);
        store_text(cx.sh, cx.id, out)?;
        ts(cx.sh, cx.id, |t| {
            t.sel_start = start.min(units(&t.text));
            t.sel_len = n.min(units(&t.text) - t.sel_start);
        });
        Ok(())
    })?;
    def(api, sh, &label, "SetSelection", t, |cx, args| {
        let start = int(args.front());
        let length = int(args.get(1));
        let n = units(&current_text(cx.sh, cx.id));
        select(cx, n, start, length);
        Ok(())
    })?;
    def(api, sh, &label, "SetSelectionStart", t, |cx, args| {
        let s = int(args.front());
        let n = units(&current_text(cx.sh, cx.id));
        ts(cx.sh, cx.id, |t| {
            if t.sel_len == 0 {
                return;
            }
            let start = (s.max(1) as usize).min(n).saturating_sub(1);
            t.sel_start = start;
            t.sel_len = t.sel_len.min(n - start);
        });
        Ok(())
    })?;
    def(api, sh, &label, "SetSelectionLength", t, |cx, args| {
        let l = int(args.front());
        let n = units(&current_text(cx.sh, cx.id));
        ts(cx.sh, cx.id, |t| {
            if l <= 0 {
                t.sel_len = 0;
            } else {
                t.sel_len = (l as usize).min(n.saturating_sub(t.sel_start));
            }
        });
        Ok(())
    })?;
    def(api, sh, &label, "SelectAll", t, |cx, _| {
        let n = units(&current_text(cx.sh, cx.id));
        ts(cx.sh, cx.id, |t| {
            t.sel_start = 0;
            t.sel_len = n;
        });
        Ok(())
    })?;
    def(api, sh, &label, "DeselectAll", t, |cx, _| {
        ts(cx.sh, cx.id, |t| t.sel_len = 0);
        Ok(())
    })?;

    // TextBox (§13.5).
    def(api, sh, &text_box_cls, "IsReadOnly", text_box, |cx, _| {
        Ok(ts(cx.sh, cx.id, |t| t.read_only).unwrap_or(false))
    })?;
    def(
        api,
        sh,
        &text_box_cls,
        "SetReadOnly",
        text_box,
        |cx, args| {
            let v = truthy(args.front());
            // The copy a read-only box restores after a user edit is the text now.
            current_text(cx.sh, cx.id);
            ts(cx.sh, cx.id, |t| t.read_only = v);
            // `tessera-ui` treats every plugin text box as editable whatever this
            // property says, so the layer also takes user edits back itself
            // (`events::text_edited`).
            if let Some(k) = ctl_get(cx.sh, cx.id, |c| c.text_key()) {
                with_ui(cx.sh, |u| {
                    let pid = u.vocab().pids.text_editable;
                    u.set_property(k, pid, PropertyValue::Bool(!v));
                });
            }
            Ok(())
        },
    )?;

    // CheckBox (§13.6).
    def(api, sh, &check, "IsChecked", check_box, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| c.checked).unwrap_or(false))
    })?;
    def(api, sh, &check, "SetChecked", check_box, |cx, args| {
        let v = truthy(args.front());
        set_checked(cx.sh, cx.id, v);
        Ok(())
    })?;
    def(api, sh, &check, "GetCheckAlignment", check_box, |cx, _| {
        Ok(f64::from(
            ctl_get(cx.sh, cx.id, |c| c.check_alignment).unwrap_or(4),
        ))
    })?;
    def(
        api,
        sh,
        &check,
        "SetCheckAlignment",
        check_box,
        |cx, args| {
            let a = int(args.front()) as u32;
            with_ctl(cx.sh, cx.id, |c| c.check_alignment = a);
            Ok(())
        },
    )?;
    let _ = lua;
    Ok(())
}

/// `SetSelection(start, length)` (§13.4).
fn select(cx: &Cx<'_>, n: usize, start: i32, length: i32) {
    ts(cx.sh, cx.id, |t| {
        if length <= 0 {
            t.sel_len = 0;
            return;
        }
        let s = (start.max(1) as usize).min(n).saturating_sub(1);
        t.sel_start = s;
        t.sel_len = (length as usize).min(n - s);
    });
}

/// Set a check box's state; `CheckedChanged` goes through the queue when it changed
/// (§9.6 OPEN, Tessera rule).
pub(crate) fn set_checked(sh: &Shared, id: u64, v: bool) {
    let changed = with_ctl(sh, id, |c| std::mem::replace(&mut c.checked, v) != v).unwrap_or(false);
    if changed {
        if let Some(mark) = ctl_get(sh, id, |c| c.mark_part).flatten() {
            with_ui(sh, |u| super::styled::set_mark(u, Some(mark), v));
        }
        sh.push(Pending::Fire {
            target: id,
            name: "CheckedChanged",
            args: Args::Empty,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_helpers() {
        let s = "a\u{1F600}b";
        assert_eq!(units(s), 4);
        assert_eq!(sub_units(s, 1, 2), "\u{1F600}");
        assert_eq!(sub_units(s, 3, 10), "b");
        assert_eq!(sub_units(s, 10, 1), "");
    }

    #[test]
    fn alignment_table() {
        assert_eq!(justification(5), ((3, 3), false));
        assert_eq!(justification(9), ((4, 1), false));
        assert_eq!(justification(0), ((5, 2), false));
        assert_eq!(justification(10), ((5, 2), true));
        assert_eq!(justification(u32::MAX), ((5, 2), true));
    }
}
