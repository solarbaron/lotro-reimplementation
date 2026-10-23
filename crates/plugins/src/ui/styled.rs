// SPDX-License-Identifier: GPL-3.0-or-later
//! Game-styled controls built from the install's layout (`lua-plugin-api.md` §14.2.1–§14.2.3):
//! which base element each class uses, the parts the layer drives (window title, close
//! button and resize bars, the check box's text and check mark, the text box's text element,
//! the scroll bar's layout widget), how the notifications of those parts reach the control
//! (through the UI's listener registry), and the once-per-class report when the layout is
//! missing and Tessera's neutral art is used instead (§14.2.1 step 5, §14.2.6).
//!
//! Element IDs, the entry key and the constants 140, 120 and 80 of the title layout are the
//! client's (§14.2.1, §14.2.3; decision 0012); every rectangle, edge mode, property and image
//! comes from the user's layout at run time.

use std::sync::atomic::{AtomicU8, Ordering};

use tessera_formats::property::PropertyValue;
use tessera_ui::vocab::{commands, tessera_msg};
use tessera_ui::{ControlEvent, ElementKey, Message, Ui, UiError};

use super::model::{Ctl, Kind, ObjData, Styled};
use super::state::State;

/// `UILAYOUT` entry key of the plugin controls' layout (`luaplugins`, §14.2.1 step 1).
pub(crate) const PLUGIN_LAYOUT_KEY: u32 = 0x1000_00C8;

/// `LotroButton` (§14.2.1 table).
pub(crate) const BUTTON: u32 = 0x1000_22A7;
/// `LotroButtonGold`.
pub(crate) const GOLD_BUTTON: u32 = 0x1000_22A8;
/// `LotroCheckBox`.
pub(crate) const CHECK_BOX: u32 = 0x1000_25AF;
/// `LotroTextBox`.
pub(crate) const TEXT_BOX: u32 = 0x1000_22A9;
/// `LotroScrollBar_Horizontal`.
pub(crate) const SCROLL_BAR_H: u32 = 0x1000_239E;
/// `LotroScrollBar_Vertical`.
pub(crate) const SCROLL_BAR_V: u32 = 0x1000_239F;
/// `LotroWindow_Container`.
pub(crate) const WINDOW: u32 = 0x1000_2294;
/// `LotroWindowGold_Container`.
pub(crate) const GOLD_WINDOW: u32 = 0x1000_2299;
/// `LotroScalerWindow_Container`: a `Window` marked as scaled (§14.2.1 table, §13.21 rule 1).
pub(crate) const SCALED_WINDOW: u32 = 0x1000_4D37;
/// `LotroScalerWindowGold_Container`: a `GoldWindow` marked as scaled (Tessera: exactly when a
/// `Window` in the same state would use its scaled variant, §14.2.1 step 2 OPEN).
pub(crate) const SCALED_GOLD_WINDOW: u32 = 0x1000_4D38;

/// `LotroCheckBox_CheckBox`: the check mark (§14.2.2).
pub(crate) const CHECK_MARK: u32 = 0x1000_25B0;
/// `LotroCheckBox_Text`: the check box's text element (§14.2.2).
pub(crate) const CHECK_TEXT: u32 = 0x1000_25B1;
/// `LotroTextBox_Text`: the text box's text element (§14.2.2).
pub(crate) const TEXT_BOX_TEXT: u32 = 0x1000_22AA;

/// `LotroWindow_TitleBarBackground` (§14.2.3).
pub(crate) const TITLE_BAR: u32 = 0x1000_2296;
/// `LotroWindow_TitleBarText`.
pub(crate) const TITLE_TEXT: u32 = 0x1000_2297;
/// `LotroWindow_CloseButton`.
pub(crate) const CLOSE_BUTTON: u32 = 0x1000_2298;
/// `LotroWindow_Dragbar`.
pub(crate) const DRAG_BAR: u32 = 0x1000_2DFB;
/// `LotroWindow_ResizeNW` … `LotroWindow_ResizeSE`: eight consecutive IDs.
pub(crate) const RESIZE_FIRST: u32 = 0x1000_2DFC;
/// Number of resize bars.
const RESIZE_BARS: u32 = 8;

/// Title layout constants of §14.2.3 (the client's, decision 0012).
const TITLE_MARGIN: i32 = 140;
const TITLE_MIN: i32 = 120;
const TITLE_BAR_EXTRA: i32 = 80;

/// The class name (for the report) and base element of a game-styled control of `kind`
/// with the look `styled`, or `None` when the class is built by the layer itself.
///
/// A game-styled `ScrollBar` keeps the layer's own control element and gets the layout's
/// scroll bar as a part below it ([`build_scroll_part`]), so its numbers, visibility and
/// rebuilds stay the layer's. A window first marked as scaled is rebuilt from its scaled
/// base element ([`rebuild_scaled_window`], §14.2.1 step 3).
pub(crate) fn base_element(kind: Kind, styled: Styled) -> Option<(&'static str, u32)> {
    match (kind, styled) {
        (_, Styled::Plain) => None,
        (Kind::Window, Styled::Silver) => Some(("Window", WINDOW)),
        (Kind::Window, Styled::Gold) => Some(("GoldWindow", GOLD_WINDOW)),
        (Kind::Button, Styled::Silver) => Some(("Button", BUTTON)),
        (Kind::Button, Styled::Gold) => Some(("GoldButton", GOLD_BUTTON)),
        (Kind::CheckBox, _) => Some(("CheckBox", CHECK_BOX)),
        (Kind::TextBox, _) => Some(("TextBox", TEXT_BOX)),
        _ => None,
    }
}

/// The base element of a game-styled scroll bar of `orientation` (§14.2.1: chosen when the
/// element is built; 1 is `Orientation.Vertical`).
#[must_use]
pub(crate) fn scroll_base(orientation: u32) -> u32 {
    if orientation == 1 {
        SCROLL_BAR_V
    } else {
        SCROLL_BAR_H
    }
}

/// Classes already reported as falling back (one bit each, §14.2.1 step 5 "log once per
/// class").
static REPORTED: AtomicU8 = AtomicU8::new(0);

/// Report once per class that the layout could not build the control.
pub(crate) fn fallback_once(class_name: &'static str, error: &UiError) {
    let bit = match class_name {
        "Window" => 1,
        "GoldWindow" => 2,
        "Button" => 4,
        "GoldButton" => 8,
        "CheckBox" => 16,
        "TextBox" => 32,
        _ => 64,
    };
    if REPORTED.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
        tracing::info!(
            class = class_name,
            error = %error,
            "game-styled plugin control built with Tessera's neutral art (no layout)"
        );
    }
}

/// Work after building a game-styled control from the layout: find the parts the layer
/// drives, register the notifications the layer needs from them, hide the resize bars of a
/// new window and run the first title layout (*m* = 0).
pub(crate) fn init_based(ui: &mut Ui, st: &mut State, id: u64) {
    let Some(ObjData::Ctl(c)) = st.objs.get_mut(&id).map(|o| &mut o.data) else {
        return;
    };
    if !c.based {
        return;
    }
    let key = c.key;
    match c.kind {
        Kind::CheckBox => {
            c.text_part = ui.find_descendant(key, CHECK_TEXT);
            c.mark_part = ui.find_descendant(key, CHECK_MARK);
            if let Some(m) = c.mark_part {
                // The mark shows the state; the press goes to the check box itself.
                ui.set_mouse_visible_flag(m, false);
            }
            sync_mark(ui, c);
        }
        Kind::TextBox => {
            let text = ui.find_descendant(key, TEXT_BOX_TEXT);
            c.text_part = text;
            if let Some(t) = text {
                // The text element edits like a plugin text box: it keeps the focus on
                // Escape and Enter (`ui-behaviour.md` §5.4; the key events go to the
                // plugin, §13.13).
                let p = ui.vocab().pids;
                ui.set_property(t, p.text_lose_focus_on_escape, PropertyValue::Bool(false));
                ui.set_property(t, p.text_lose_focus_on_accept, PropertyValue::Bool(false));
            }
            if !st.text_box_handlers {
                st.text_box_handlers = true;
                for msg in text_part_messages(ui) {
                    ui.register_handler(TEXT_BOX_TEXT, msg);
                }
            }
        }
        Kind::Window => {
            let resizable = c.window.as_deref().is_some_and(|w| w.resizable);
            set_resize_bars(ui, key, resizable);
            if !st.close_handler {
                st.close_handler = true;
                let msg = ui.vocab().msgs.button_click;
                ui.register_handler(CLOSE_BUTTON, msg);
            }
            if let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) {
                let measured = title_layout(ui, c);
                note_title(st, id, measured);
            }
        }
        _ => {}
    }
}

/// The notifications of a text box's text element that become the control's events
/// (§13.4, §13.5, §13.12, §13.13). Enter and leave are not among them: the control's own
/// element reports those.
fn text_part_messages(ui: &Ui) -> [u32; 11] {
    let ms = ui.vocab().msgs;
    [
        ms.mouse_press,
        ms.mouse_release,
        ms.mouse_click,
        ms.mouse_dbl_click,
        ms.mouse_move,
        ms.mouse_hover,
        ms.mouse_scroll,
        ms.set_focus,
        ms.key_press,
        ms.key_release,
        tessera_msg::TEXT_CHANGED,
    ]
}

/// The notifications of a scroll bar's layout widget that drive the bar's numbers
/// (`ui-widgets.md` §6.5–§6.8).
fn scroll_part_messages(ui: &Ui) -> [u32; 5] {
    let ms = ui.vocab().msgs;
    [
        ms.sb_new_position,
        ms.sb_line_increment,
        ms.sb_line_decrement,
        ms.sb_page_increment,
        ms.sb_page_decrement,
    ]
}

/// Builds the layout scroll bar of the game-styled scroll bar `id` for its orientation as a
/// part filling the control (§14.2.1, §14.2.2), registers its notifications and shows the
/// bar's numbers on it. Returns the error when the layout cannot build it (the caller then
/// uses the neutral thumb, §14.2.1 step 5).
pub(crate) fn build_scroll_part(ui: &mut Ui, st: &mut State, id: u64) -> Result<(), UiError> {
    let Some(ObjData::Ctl(c)) = st.objs.get_mut(&id).map(|o| &mut o.data) else {
        return Err(UiError::NoSuchElement);
    };
    let key = c.key;
    let Some(s) = c.scroll.as_deref_mut() else {
        return Err(UiError::NoSuchElement);
    };
    let part = ui.create_based_part(key, PLUGIN_LAYOUT_KEY, scroll_base(s.orientation))?;
    let Some(element_id) = ui.element_id(part) else {
        return Err(UiError::NoSuchElement);
    };
    s.widget = Some((part, element_id));
    for msg in scroll_part_messages(ui) {
        ui.register_handler(element_id, msg);
    }
    c.based = true;
    sync_scroll_part(ui, c);
    Ok(())
}

/// Removes the registry entries of a scroll bar's layout widget (its element ID).
pub(crate) fn forget_scroll_part(ui: &mut Ui, element_id: u32) {
    for msg in scroll_part_messages(ui) {
        ui.unregister_handler(element_id, msg);
    }
}

/// The base-layer operation number (§13.17) of an element whose last blend mode was `n`
/// (§14.2.1 step 3 table; never set counts as 2).
#[must_use]
pub(crate) fn operation_number(n: u32) -> u32 {
    match n {
        5 => 1,
        4 => 2,
        1 => 3,
        3 => 4,
        7 => 5,
        6 => 6,
        8 => 7,
        _ => 0,
    }
}

/// The rebuild of §14.2.1 step 3 for a game-styled scroll bar whose orientation changed:
/// the layout widget is built again for the new orientation (the control's own element,
/// with its visibility, position, size, z-level and background, stays), and the old
/// element's operation number is passed to `SetBlendMode` as a blend-mode value.
pub(crate) fn rebuild_scroll_part(ui: &mut Ui, st: &mut State, id: u64) {
    let old = match st.objs.get_mut(&id).map(|o| &mut o.data) {
        Some(ObjData::Ctl(c)) => c
            .scroll
            .as_deref_mut()
            .and_then(|s| s.widget.take())
            .map(|w| (c.key, w)),
        _ => None,
    };
    let Some((key, (part, element_id))) = old else {
        return;
    };
    forget_scroll_part(ui, element_id);
    ui.destroy(part);
    if let Err(e) = build_scroll_part(ui, st, id) {
        tracing::debug!(error = %e, "game-styled scroll bar not rebuilt from the layout");
    }
    let op = operation_number(ui.control_blend_mode(key));
    // Operation numbers are 0–7, so this never raises the "above 8" error (§14.2.1 step 3).
    let _ = ui.control_set_blend_mode(key, op);
}

/// The scaled base element of a game-styled window of look `styled` (§14.2.1 table).
#[must_use]
pub(crate) fn scaled_base(styled: Styled) -> Option<u32> {
    match styled {
        Styled::Silver => Some(SCALED_WINDOW),
        Styled::Gold => Some(SCALED_GOLD_WINDOW),
        Styled::Plain => None,
    }
}

/// The rebuild of §14.2.1 step 3 for a game-styled window first marked as scaled (§13.21
/// rule 1): a new element is built from the window's scaled base element
/// (`LotroScalerWindow_Container` or `LotroScalerWindowGold_Container`), the old element's
/// visibility, position, size, z-level and background image (`background`, the stored
/// `SetBackground` value checked again) are kept, the old element's base-layer operation
/// number is passed to `SetBlendMode` as a blend-mode value, the title text is set again
/// (the title layout of §14.2.3) and the old element is destroyed.
///
/// Tessera choices (`lua-plugin-api.md` §14.2.6 implementer OPEN): the plugin's child
/// controls (and the layer's parts) move to the new element with their own state, so the
/// window keeps its content; the flags the layer keeps for the control (enabled, wants
/// updates, wants key events) are applied to the new element so its getters and the element
/// agree; every other element state the old element held (opacity, mouse visibility, back
/// colour, stretch mode, rotation, drop target) starts again from the new element's. When
/// the layout cannot build the scaled element (a modified install) the window keeps its
/// element (logged at debug level). Returns whether the window was rebuilt.
pub(crate) fn rebuild_scaled_window(
    ui: &mut Ui,
    st: &mut State,
    id: u64,
    background: tessera_ui::Background,
) -> bool {
    let Some(o) = st.objs.get(&id) else {
        return false;
    };
    let ObjData::Ctl(c) = &o.data else {
        return false;
    };
    if !c.based || c.kind != Kind::Window || c.internal.is_some() {
        return false;
    }
    let Some(base) = scaled_base(c.styled) else {
        return false;
    };
    let ap = o.apartment.clone();
    let old = c.key;
    let (enabled, updates, keys) = (c.enabled, c.wants_updates, c.wants_keys);
    let shown = c.visible && !c.layer_hidden;
    let aid = super::state::apartment_id_with(st, &ap);
    let new = match ui.create_based_control(
        tessera_ui::ControlKind::Window,
        aid,
        PLUGIN_LAYOUT_KEY,
        base,
    ) {
        Ok(k) => k,
        Err(e) => {
            tracing::debug!(error = %e, "scaled plugin window not rebuilt from the layout");
            return false;
        }
    };
    // What the rebuild keeps (§14.2.1 step 3).
    let (x, y) = ui.control_position(old);
    let (w, h) = ui.control_size(old);
    let z = ui.control_z_order(old);
    let op = operation_number(ui.control_blend_mode(old));
    let children = ui.control_children(old);
    // Size and position are kept, not changed (§14.2.1 step 3): the notifications the new
    // element posts for them are not plugin events (only those really posted are marked,
    // so no mark is left over to swallow a later real event).
    let (pos0, size0) = (ui.control_position(new), ui.control_size(new));
    ui.control_set_position(new, x, y);
    ui.control_set_size(new, w, h);
    if ui.control_position(new) != pos0 {
        st.quiet_position.insert(new);
    }
    if ui.control_size(new) != size0 {
        st.quiet_size.insert(new);
    }
    ui.control_set_z_order(new, z);
    ui.control_set_background(new, background);
    // Operation numbers are 0–7, so this never raises the "above 8" error (§14.2.1 step 3).
    let _ = ui.control_set_blend_mode(new, op);
    ui.control_set_enabled(new, enabled);
    if updates {
        ui.control_set_wants_updates(new, true);
    }
    if keys {
        ui.control_set_wants_keys(new, true);
    }
    for k in children {
        if ui.control_set_parent(k, Some(new)).is_err() {
            tracing::debug!("a child control could not move to the rebuilt window");
        }
    }
    let was_shown = ui.is_visible(old);
    let before = ui.is_visible(new);
    ui.control_set_visible(new, shown);
    if was_shown == shown && before != ui.is_visible(new) {
        // The visibility is kept, not changed: no `VisibleChanged` for the new element.
        // Marked only when the call posted one (the new element starts hidden, so a hidden
        // window posts none), as `apply_visibility` does.
        st.quiet_visible.insert(new);
    }
    st.by_key.remove(&old);
    st.by_key.insert(new, id);
    st.quiet_visible.remove(&old);
    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&id).map(|o| &mut o.data) {
        c.key = new;
    }
    ui.destroy_control(old);
    // The parts of the new element and the title text, set again (§13.21 rule 1).
    init_based(ui, st, id);
    true
}

/// Shows the numbers of a game-styled scroll bar on its layout widget: the thumb position
/// and size fractions of §9.6 (Thumb) as `UICore_Scrollbar_position` and
/// `UICore_Scrollbar_widget_size` (the size is used by proportional bars).
pub(crate) fn sync_scroll_part(ui: &mut Ui, c: &Ctl) {
    let Some(s) = c.scroll.as_deref() else { return };
    let Some((part, _)) = s.widget else { return };
    let p = ui.vocab().pids;
    let fraction = |v: f32, default: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            default
        }
    };
    let q = fraction(s.nums.pos_frac, 0.0);
    let size = fraction(s.nums.size_frac, 1.0);
    let set = |ui: &Ui, pid: u32, v: f32| {
        let cur = ui.prop(part, pid);
        !cur.is_set() || (cur.float() - v).abs() > f32::EPSILON
    };
    if set(ui, p.sb_widget_size, size) {
        ui.set_property(part, p.sb_widget_size, PropertyValue::Float(size));
    }
    if set(ui, p.sb_position, q) {
        ui.set_property(part, p.sb_position, PropertyValue::Float(q));
    }
}

/// Handles a notification that the listener registry reported (`UiEvent::Game`) when it
/// comes from a part of a game-styled control: the text element of a text box becomes the
/// box's event, a scroll bar's layout widget moves the bar's value. Returns whether it was
/// one.
pub(crate) fn part_message(ui: &mut Ui, st: &mut State, m: &Message) -> bool {
    if let Some(id) = scroll_owner(ui, st, m) {
        scroll_message(ui, st, id, m);
        return true;
    }
    if m.source_id == TEXT_BOX_TEXT
        && let Some((id, off)) = text_owner(ui, st, m.source)
    {
        if let Some(ev) = text_event(ui, m, off) {
            super::events::translate(ui, st, id, ev);
        }
        return true;
    }
    false
}

/// The game-styled scroll bar whose layout widget sent `m`.
fn scroll_owner(ui: &Ui, st: &State, m: &Message) -> Option<u64> {
    let bar = ui.parent(m.source)?;
    let id = st.by_key.get(&bar).copied()?;
    match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c))
            if c.scroll
                .as_deref()
                .and_then(|s| s.widget)
                .is_some_and(|(k, eid)| k == m.source && eid == m.source_id) =>
        {
            Some(id)
        }
        _ => None,
    }
}

/// Most levels between a text box and its text element (Tessera bound).
const MAX_PART_DEPTH: usize = 64;

/// The game-styled text box whose text element is `part`, and the part's offset inside it.
fn text_owner(ui: &Ui, st: &State, part: ElementKey) -> Option<(u64, (i32, i32))> {
    let mut off = (0i32, 0i32);
    let mut cur = part;
    for _ in 0..MAX_PART_DEPTH {
        let b = ui.layout_box(cur)?;
        off = (off.0.saturating_add(b.l), off.1.saturating_add(b.t));
        cur = ui.parent(cur)?;
        if let Some(id) = st.by_key.get(&cur).copied() {
            return match st.objs.get(&id).map(|o| &o.data) {
                Some(ObjData::Ctl(c))
                    if c.based && c.kind == Kind::TextBox && c.text_part == Some(part) =>
                {
                    Some((id, off))
                }
                _ => None,
            };
        }
    }
    None
}

/// The control event of a text element's notification, with positions relative to the
/// text box (`off` is the element's offset inside it).
fn text_event(ui: &Ui, m: &Message, off: (i32, i32)) -> Option<ControlEvent> {
    let ms = ui.vocab().msgs;
    let a = &m.arg;
    let button = a.command.as_ref().map_or(0, |c| c.mouse_button());
    let (x, y) = (a.pos.0.saturating_add(off.0), a.pos.1.saturating_add(off.1));
    Some(match m.msg {
        v if v == ms.mouse_press => ControlEvent::MouseDown { button, x, y },
        v if v == ms.mouse_release => ControlEvent::MouseUp { button, x, y },
        v if v == ms.mouse_click => ControlEvent::MouseClick { button, x, y },
        v if v == ms.mouse_dbl_click => ControlEvent::MouseDoubleClick { button, x, y },
        v if v == ms.mouse_move => ControlEvent::MouseMove { x, y },
        v if v == ms.mouse_hover => ControlEvent::MouseHover { x, y },
        v if v == ms.mouse_scroll => {
            let direction = match a.command.as_ref().map(|c| &*c.name) {
                Some(commands::SCROLL_UP) => 1,
                Some(commands::SCROLL_DOWN) => -1,
                _ => return None,
            };
            ControlEvent::MouseWheel { direction, x, y }
        }
        v if v == ms.set_focus => {
            if a.value != 0 {
                ControlEvent::FocusGained
            } else {
                ControlEvent::FocusLost
            }
        }
        v if v == ms.key_press => ControlEvent::KeyDown {
            command: a.command.clone()?,
        },
        v if v == ms.key_release => ControlEvent::KeyUp {
            command: a.command.clone()?,
        },
        tessera_msg::TEXT_CHANGED => ControlEvent::TextChanged,
        _ => return None,
    })
}

/// A notification of the layout widget of game-styled scroll bar `id`: a new position from
/// the pointer becomes the value at that fraction of the range, the arrow buttons and the
/// wheel step by the small change, the track pages by the large change (§9.6, §13.7).
fn scroll_message(ui: &mut Ui, st: &mut State, id: u64, m: &Message) {
    let Some(n) = (match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().map(|s| s.nums),
        _ => None,
    }) else {
        return;
    };
    let ms = ui.vocab().msgs;
    let small = i64::from(n.small);
    let large = i64::from(n.large);
    match m.msg {
        v if v == ms.sb_new_position => {
            let q = ui.prop(m.source, ui.vocab().pids.sb_position).float();
            let q = if q.is_finite() {
                f64::from(q).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let range = n.max.wrapping_sub(n.min);
            let along = (q * f64::from(range))
                .round()
                .clamp(0.0, f64::from(u32::MAX)) as u32;
            let v = n.min.wrapping_add(along);
            super::scroll::user_set_value(ui, st, id, v);
        }
        v if v == ms.sb_line_increment => super::scroll::step(ui, st, id, small),
        v if v == ms.sb_line_decrement => super::scroll::step(ui, st, id, -small),
        v if v == ms.sb_page_increment => super::scroll::step(ui, st, id, large),
        v if v == ms.sb_page_decrement => super::scroll::step(ui, st, id, -large),
        _ => {}
    }
}

/// Show the check mark by the checked state (§14.2.2).
pub(crate) fn sync_mark(ui: &mut Ui, c: &Ctl) {
    set_mark(ui, c.mark_part, c.checked);
}

/// Show check mark `mark` (if any) as `checked`.
pub(crate) fn set_mark(ui: &mut Ui, mark: Option<ElementKey>, checked: bool) {
    if let Some(m) = mark {
        let pid = ui.vocab().pids.button_highlighted;
        ui.set_property(m, pid, PropertyValue::Bool(checked));
    }
}

/// `UICore_Element_hide` on all eight resize bars = not `resizable` (§14.2.3 "Resizing").
pub(crate) fn set_resize_bars(ui: &mut Ui, window: ElementKey, resizable: bool) {
    let pid = ui.vocab().pids.hide;
    for i in 0..RESIZE_BARS {
        if let Some(b) = ui.find_child(window, RESIZE_FIRST + i) {
            ui.set_property(b, pid, PropertyValue::Bool(!resizable));
        }
    }
}

/// The title widths of §14.2.3 steps 1 and 3 for a window of width `w` and caption width
/// `m`: (*t*, *t* + 80), in 32-bit signed arithmetic (saturating; *t* may be negative).
#[must_use]
pub(crate) fn title_widths(w: i32, m: i32) -> (i32, i32) {
    let t = w.saturating_sub(TITLE_MARGIN).min(m.max(TITLE_MIN));
    (t, t.saturating_add(TITLE_BAR_EXTRA))
}

/// The title layout of §14.2.3: the caption is set on the title text, measured (*m*), and
/// the title text, title-bar background and drag bar are placed from the window's width;
/// their tops and heights are kept. Returns `false` when a caption could not be measured
/// (its fonts are not loaded yet: laid out with *m* = 0 for now, to be laid out again).
pub(crate) fn title_layout(ui: &mut Ui, c: &Ctl) -> bool {
    let key = c.key;
    let caption = c
        .window
        .as_deref()
        .map(|w| w.text.clone())
        .unwrap_or_default();
    let (w, _) = ui.control_size(key);
    let text = ui.find_child(key, TITLE_TEXT);
    let mut measured = true;
    let m = match text {
        Some(t) => {
            // Step 2: the title text's four text margins are 0 (the layout's own margins do
            // not apply), set before the caption is measured. Properties by dictionary name
            // (`fonts.md` §7, `ui-layout.md` §5.9; resolved at run time like every UI PID).
            let p = ui.vocab().pids;
            for pid in [
                p.text_left_margin,
                p.text_right_margin,
                p.text_top_margin,
                p.text_bottom_margin,
            ] {
                ui.set_property(t, pid, PropertyValue::Int(0));
            }
            let empty = caption.is_empty();
            ui.set_text(t, Some(caption));
            let width = ui.measure_text_width(t);
            measured = empty || width.is_some();
            width.unwrap_or(0)
        }
        None => 0,
    };
    let (t, bar) = title_widths(w, m);
    if let Some(k) = text {
        place_x(ui, k, w.saturating_sub(t) / 2, t);
    }
    for id in [TITLE_BAR, DRAG_BAR] {
        if let Some(k) = ui.find_child(key, id) {
            place_x(ui, k, w.saturating_sub(bar) / 2, bar);
        }
    }
    measured
}

/// Records whether the title layout of window `id` measured its caption: a window whose
/// caption could not be measured is laid out again by [`super::window::retry_titles`]. A
/// new layout (new caption or size) starts its retries afresh.
pub(crate) fn note_title(st: &State, id: u64, measured: bool) {
    let mut pending = st.title_pending.borrow_mut();
    if measured {
        pending.remove(&id);
    } else {
        pending.insert(id, 0);
    }
}

/// Moves part `k` to `left` with width `width`, keeping its top and height.
fn place_x(ui: &mut Ui, k: ElementKey, left: i32, width: i32) {
    let Some(b) = ui.layout_box(k) else { return };
    ui.set_element_box(k, left, b.t, width, b.h());
}

/// The game-styled window whose close button sent `message`, when it is a
/// `Button_click` of a close button below a window built from the layout (§14.2.3
/// "Close button").
pub(crate) fn closed_window(ui: &Ui, st: &State, message: &Message) -> Option<u64> {
    if message.source_id != CLOSE_BUTTON || message.msg != ui.vocab().msgs.button_click {
        return None;
    }
    let window = ui.parent(message.source)?;
    let id = st.by_key.get(&window).copied()?;
    match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) if c.based && c.kind == Kind::Window && c.internal.is_none() => {
            Some(id)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{operation_number, title_widths};

    /// The table of §14.2.1 step 3 and its chains: `SetBlendMode(8)` and three rebuilds
    /// give 7, 5, 1; 6 is a fixed point; every other chain ends at 0 within seven rebuilds.
    #[test]
    fn operation_numbers() {
        let ops: Vec<u32> = (0..=8).map(operation_number).collect();
        assert_eq!(ops, [0, 3, 0, 4, 2, 1, 6, 5, 7]);
        let mut n = 8;
        let chain: Vec<u32> = (0..3)
            .map(|_| {
                n = operation_number(n);
                n
            })
            .collect();
        assert_eq!(chain, [7, 5, 1]);
        assert_eq!(operation_number(6), 6);
        for start in [0, 1, 2, 3, 4, 5, 7, 8] {
            let mut n = start;
            for _ in 0..7 {
                n = operation_number(n);
            }
            assert_eq!(n, 0, "chain from {start}");
        }
    }

    /// The worked example of `lua-plugin-api.md` §14.2.3.
    #[test]
    fn worked_example() {
        assert_eq!(title_widths(400, 96), (120, 200));
        assert_eq!(title_widths(200, 96), (60, 140));
        assert_eq!(title_widths(50, 0), (-90, -10));
        assert_eq!(title_widths(i32::MIN, i32::MAX), (i32::MIN, i32::MIN + 80));
    }
}
