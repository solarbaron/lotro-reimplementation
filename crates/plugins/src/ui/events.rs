// SPDX-License-Identifier: GPL-3.0-or-later
//! From UI notifications to plugin events (`lua-plugin-api.md` §9.3, §9.7, §13.12): the
//! event of each notification, and the layer's own reactions (check boxes, quickslots,
//! list and tree selection, scroll bars, context menus, the neutral window frame).

use tessera_ui::{ControlEvent, Ui};

use super::model::{Args, Kind, ObjData, Role, Styled};
use super::state::{Pending, State, push};

fn fire(st: &mut State, target: u64, name: &'static str, args: Args) {
    push(st, Pending::Fire { target, name, args });
}

/// The scroll bar a plugin control is a part of: (bar, 0 decrement, 1 increment, 2 thumb).
/// The part keeps the link (set by `SetDecrementButton` and its siblings); it counts only
/// while the bar still names the part.
fn part_of_bar(st: &State, id: u64) -> Option<(u64, u8)> {
    let (bar, which) = match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.part_of?,
        _ => return None,
    };
    let s = match st.objs.get(&bar).map(|o| &o.data) {
        Some(ObjData::Ctl(b)) => b.scroll.as_deref()?,
        _ => return None,
    };
    let slot = match which {
        0 => s.decrement,
        1 => s.increment,
        _ => s.thumb,
    };
    (slot == Some(id)).then_some((bar, which))
}

/// The vertical scroll bar bound to list box or tree `list`, if any.
fn list_vbar(st: &State, list: u64) -> Option<u64> {
    match st.objs.get(&list).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.vbar.filter(|b| st.objs.contains_key(b)),
        _ => None,
    }
}

/// A text box's text was edited by the user (the UI's text-changed notification): the
/// layer's copy follows the element, and the caret, which `tessera-ui` does not report,
/// is placed at the end of the edited span (the common prefix and suffix of the old and
/// new text; one notification is one contiguous edit). A read-only box takes the edit
/// back instead. Returns whether the plugin sees `TextChanged`.
fn text_edited(ui: &mut Ui, st: &mut State, id: u64, key: tessera_ui::ElementKey) -> bool {
    let Some(o) = st.objs.get_mut(&id) else {
        return false;
    };
    let ap = o.apartment.clone();
    let ObjData::Ctl(c) = &mut o.data else {
        return false;
    };
    let Some(t) = c.text.as_deref_mut() else {
        return true;
    };
    if t.read_only {
        ui.set_text(key, Some(t.text.clone()));
        return false;
    }
    let new = ui.text_of(key);
    if new == t.text {
        return true;
    }
    let caret = edit_caret(&t.text, &new);
    let old_len = t.text.len();
    let new_len = new.len();
    t.text = new;
    t.sel_start = caret;
    t.sel_len = 0;
    let n = st.text_bytes.entry(ap).or_insert(0);
    *n = n.saturating_sub(old_len).saturating_add(new_len);
    true
}

/// The caret after the single edit that turned `old` into `new`, in UTF-16 units: the end
/// of the replaced span in `new`.
pub(crate) fn edit_caret(old: &str, new: &str) -> usize {
    let a: Vec<u16> = old.encode_utf16().collect();
    let b: Vec<u16> = new.encode_utf16().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let room = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();
    b.len() - suffix
}

fn bar_vertical(st: &State, bar: u64) -> bool {
    match st.objs.get(&bar).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().is_some_and(|s| s.orientation == 1),
        _ => false,
    }
}

fn set_drag(st: &mut State, bar: u64, d: Option<i32>) {
    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&bar).map(|o| &mut o.data)
        && let Some(s) = c.scroll.as_deref_mut()
    {
        s.drag = d;
    }
}

fn dragging(st: &State, bar: u64) -> bool {
    match st.objs.get(&bar).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().is_some_and(|s| s.drag.is_some()),
        _ => false,
    }
}

fn small_change(st: &State, bar: u64) -> i64 {
    match st.objs.get(&bar).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().map_or(1, |s| i64::from(s.nums.small)),
        _ => 1,
    }
}

/// A press on the thumb at `along` pixels inside it starts a drag.
fn thumb_press(st: &mut State, bar: u64, along: i32) {
    set_drag(st, bar, Some(along));
}

/// A press on the bar itself at (x, y).
fn bar_press(ui: &mut Ui, st: &mut State, bar: u64, x: i32, y: i32) {
    let Some((vertical, _, _, _, _, thumb, start)) = super::scroll::geometry(ui, st, bar) else {
        return;
    };
    let p = if vertical { y } else { x };
    if p >= start && p < start.saturating_add(thumb) {
        set_drag(st, bar, Some(p.saturating_sub(start)));
    } else {
        super::scroll::page_toward(ui, st, bar, p);
    }
}

fn bar_move(ui: &mut Ui, st: &mut State, bar: u64, p: i32) {
    if dragging(st, bar) {
        super::scroll::drag_to(ui, st, bar, p);
    }
}

fn thumb_pos(ui: &Ui, st: &State, bar: u64) -> i32 {
    super::scroll::geometry(ui, st, bar).map_or(0, |g| g.6)
}

/// The neutral frame of a game-styled window: move or resize by the pointer.
fn frame_drag(ui: &mut Ui, st: &mut State, win: u64, start: Option<u8>) {
    let Some(ObjData::Ctl(c)) = st.objs.get_mut(&win).map(|o| &mut o.data) else {
        return;
    };
    let key = c.key;
    let Some(w) = c.window.as_deref_mut() else {
        return;
    };
    match start {
        Some(mode) => {
            let p = ui.pointer().unwrap_or((0, 0));
            let (x, y) = ui.control_position(key);
            let (cw, ch) = ui.control_size(key);
            w.frame_drag = Some((mode, p, (x, y, cw, ch)));
        }
        None => {
            let Some((mode, p0, (x, y, cw, ch))) = w.frame_drag else {
                return;
            };
            let p = ui.pointer().unwrap_or(p0);
            let (dx, dy) = (p.0.saturating_sub(p0.0), p.1.saturating_sub(p0.1));
            if mode == 0 {
                ui.control_set_position(key, x.saturating_add(dx), y.saturating_add(dy));
            } else {
                ui.control_set_size(key, cw.saturating_add(dx), ch.saturating_add(dy));
                super::window::layout_frame_with(ui, st, win);
            }
        }
    }
}

fn end_frame_drag(st: &mut State, win: u64) {
    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&win).map(|o| &mut o.data)
        && let Some(w) = c.window.as_deref_mut()
    {
        w.frame_drag = None;
    }
}

/// Events of internal parts: the layer's reactions only.
fn part_event(ui: &mut Ui, st: &mut State, id: u64, role: Role, owner: u64, ev: ControlEvent) {
    match (role, ev) {
        (Role::FrameTitle, ControlEvent::MouseDown { button: 1, .. }) => {
            frame_drag(ui, st, owner, Some(0))
        }
        (Role::FrameResize, ControlEvent::MouseDown { button: 1, .. }) => {
            frame_drag(ui, st, owner, Some(1))
        }
        (Role::FrameTitle | Role::FrameResize, ControlEvent::MouseMove { .. }) => {
            frame_drag(ui, st, owner, None)
        }
        (Role::FrameTitle | Role::FrameResize, ControlEvent::MouseUp { .. }) => {
            end_frame_drag(st, owner)
        }
        (Role::FrameClose, ControlEvent::Click) => push(st, Pending::Close(owner)),
        (Role::Thumb, ControlEvent::MouseDown { button: 1, x, y }) => {
            let along = if bar_vertical(st, owner) { y } else { x };
            thumb_press(st, owner, along);
        }
        (Role::Thumb, ControlEvent::MouseMove { x, y }) => {
            let along = if bar_vertical(st, owner) { y } else { x };
            let p = thumb_pos(ui, st, owner).saturating_add(along);
            bar_move(ui, st, owner, p);
        }
        (Role::Thumb, ControlEvent::MouseUp { .. }) => set_drag(st, owner, None),
        (Role::Thumb, ControlEvent::MouseWheel { direction, .. }) => {
            let s = small_change(st, owner);
            super::scroll::step(ui, st, owner, -i64::from(direction) * s);
        }
        (Role::MenuRow(_), ControlEvent::MouseClick { button: 1, .. }) => {
            super::menu::row_clicked(ui, st, id)
        }
        (Role::MenuPopup, ControlEvent::Deactivated) => super::menu::deactivated(ui, st),
        _ => {}
    }
}

/// Wheel step of a list or tree without a scroll bar, in pixels (Tessera rule).
const WHEEL_STEP: i32 = 20;

/// Translate the notification `ev` of control `id` (its source).
pub(crate) fn translate(ui: &mut Ui, st: &mut State, id: u64, ev: ControlEvent) {
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return;
    };
    let (key, kind, styled, internal, owner) = (c.key, c.kind, c.styled, c.internal, c.owner);
    // The element that holds the text (a game-styled text box's text element, §14.2.2).
    let text_key = c.text_key();
    // A game-styled scroll bar's layout widget handles the pointer itself (§14.2.2).
    let widget_bar =
        kind == Kind::ScrollBar && c.scroll.as_deref().is_some_and(|s| s.widget.is_some());
    if let Some((role, owner)) = internal {
        part_event(ui, st, id, role, owner, ev);
        return;
    }
    match ev {
        ControlEvent::MouseEnter => fire(st, id, "MouseEnter", Args::Empty),
        ControlEvent::MouseLeave => fire(st, id, "MouseLeave", Args::Empty),
        ControlEvent::MouseDown { button, x, y } => {
            fire(st, id, "MouseDown", Args::Mouse { button, x, y });
            if button == 1 {
                // Tessera rule (LPA §13.1): a text box takes focus when it is clicked. The
                // press has already activated its window, so the focus moves now.
                if kind == Kind::TextBox {
                    ui.control_focus(text_key);
                }
                if kind == Kind::ScrollBar {
                    if !widget_bar {
                        bar_press(ui, st, id, x, y);
                    }
                } else if let Some((bar, 2)) = part_of_bar(st, id) {
                    let along = if bar_vertical(st, bar) { y } else { x };
                    thumb_press(st, bar, along);
                }
                if let Some((list, item)) = super::lists::owning_list(ui, st, key) {
                    let lk = match st.objs.get(&list).map(|o| &o.data) {
                        Some(ObjData::Ctl(lc)) => Some(lc.kind),
                        _ => None,
                    };
                    match (lk, item) {
                        (Some(Kind::ListBox), Some(item)) => {
                            super::lists::select_item(st, list, Some(item))
                        }
                        (Some(Kind::TreeView), Some(node)) => {
                            super::lists::select_node(ui, st, list, Some(node), true);
                        }
                        _ => {}
                    }
                }
            }
        }
        ControlEvent::MouseUp { button, x, y } => {
            fire(st, id, "MouseUp", Args::Mouse { button, x, y });
            if kind == Kind::ScrollBar {
                set_drag(st, id, None);
            } else if let Some((bar, 2)) = part_of_bar(st, id) {
                set_drag(st, bar, None);
            }
        }
        ControlEvent::MouseClick { button, x, y } => {
            fire(st, id, "MouseClick", Args::Mouse { button, x, y });
            if kind == Kind::Quickslot {
                super::shortcut::activate(st, id, button);
            }
            if button == 1
                && let Some((bar, which)) = part_of_bar(st, id)
                && which < 2
            {
                let s = small_change(st, bar);
                super::scroll::step(ui, st, bar, if which == 0 { -s } else { s });
            }
        }
        ControlEvent::MouseDoubleClick { button, x, y } => {
            fire(st, id, "MouseDoubleClick", Args::Mouse { button, x, y });
        }
        ControlEvent::MouseMove { x, y } => {
            fire(st, id, "MouseMove", Args::Pos { x, y });
            if kind == Kind::ScrollBar && !widget_bar {
                let p = if bar_vertical(st, id) { y } else { x };
                bar_move(ui, st, id, p);
            } else if let Some((bar, 2)) = part_of_bar(st, id) {
                let along = if bar_vertical(st, bar) { y } else { x };
                let p = thumb_pos(ui, st, bar).saturating_add(along);
                bar_move(ui, st, bar, p);
            }
        }
        ControlEvent::MouseHover { x, y } => fire(st, id, "MouseHover", Args::Pos { x, y }),
        ControlEvent::MouseWheel { direction, x, y } => {
            fire(st, id, "MouseWheel", Args::Wheel { direction, x, y });
            let bar = if kind == Kind::ScrollBar {
                // The layout widget steps on its own wheel messages.
                (!widget_bar).then_some(id)
            } else {
                part_of_bar(st, id).map(|b| b.0)
            };
            if let Some(bar) = bar {
                let s = small_change(st, bar);
                super::scroll::step(ui, st, bar, -i64::from(direction) * s);
            } else if let Some((list, _)) = super::lists::owning_list(ui, st, key) {
                // LPA §13.7 OPEN (Tessera rule): the bound scroll bar's small change, else
                // 20 pixels.
                match list_vbar(st, list) {
                    Some(bar) => {
                        let s = small_change(st, bar);
                        super::scroll::step(ui, st, bar, -i64::from(direction) * s);
                    }
                    None => {
                        super::lists::scroll_by(
                            ui,
                            st,
                            list,
                            -direction.saturating_mul(WHEEL_STEP),
                        );
                    }
                }
            }
        }
        ControlEvent::KeyDown { command } | ControlEvent::KeyUp { command }
            if command.id.is_none() => {}
        ControlEvent::KeyDown { command } => {
            let m = st.modifiers;
            let action = command.id.unwrap_or(0);
            fire(
                st,
                id,
                "KeyDown",
                Args::Key {
                    action,
                    alt: m.alt,
                    control: m.control,
                    shift: m.shift,
                },
            );
        }
        ControlEvent::KeyUp { command } => {
            let m = st.modifiers;
            let action = command.id.unwrap_or(0);
            fire(
                st,
                id,
                "KeyUp",
                Args::Key {
                    action,
                    alt: m.alt,
                    control: m.control,
                    shift: m.shift,
                },
            );
        }
        ControlEvent::FocusGained => fire(st, id, "FocusGained", Args::Empty),
        ControlEvent::FocusLost => fire(st, id, "FocusLost", Args::Empty),
        ControlEvent::SizeChanged => {
            if !st.quiet_size.remove(&key) {
                fire(st, id, "SizeChanged", Args::Empty);
            }
            match kind {
                Kind::ListBox | Kind::TreeView => super::lists::mark(st, id),
                Kind::ScrollBar => super::scroll::layout_parts_with(ui, st, id),
                // §14.2.3: the title layout runs after the `SizeChanged` handlers.
                Kind::Window if styled != Styled::Plain => push(st, Pending::TitleLayout(id)),
                _ => {}
            }
            if let Some(o) = owner {
                super::lists::mark(st, o);
            }
        }
        ControlEvent::PositionChanged => {
            if !st.quiet_position.remove(&key) {
                fire(st, id, "PositionChanged", Args::Empty);
            }
        }
        ControlEvent::VisibleChanged => fire(st, id, "VisibleChanged", Args::Empty),
        ControlEvent::Activated => fire(st, id, "Activated", Args::Empty),
        ControlEvent::Deactivated => fire(st, id, "Deactivated", Args::Empty),
        ControlEvent::Click => {
            if kind == Kind::CheckBox {
                let enabled =
                    matches!(st.objs.get(&id).map(|o| &o.data), Some(ObjData::Ctl(c)) if c.enabled);
                if enabled {
                    let mut changed = false;
                    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&id).map(|o| &mut o.data) {
                        c.checked = !c.checked;
                        changed = true;
                        super::styled::sync_mark(ui, c);
                    }
                    if changed {
                        fire(st, id, "CheckedChanged", Args::Empty);
                    }
                }
            } else {
                fire(st, id, "Click", Args::Empty);
            }
        }
        ControlEvent::TextChanged => {
            if kind != Kind::TextBox || text_edited(ui, st, id, text_key) {
                fire(st, id, "TextChanged", Args::Empty);
            }
        }
        ControlEvent::DragStart => fire(st, id, "DragStart", Args::Empty),
        ControlEvent::DragEnter => fire(st, id, "DragEnter", Args::Empty),
        ControlEvent::DragLeave => fire(st, id, "DragLeave", Args::Empty),
        ControlEvent::DragDrop => {
            let (x, y) = ui.control_mouse_position(key);
            let shortcut = super::shortcut::drag_shortcut(st, st.drag_source);
            fire(st, id, "DragDrop", Args::DragDrop { x, y, shortcut });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::edit_caret;

    #[test]
    fn caret_after_an_edit() {
        assert_eq!(edit_caret("", "abc"), 3);
        assert_eq!(edit_caret("abc", "Xabc"), 1);
        assert_eq!(edit_caret("ac", "abc"), 2);
        assert_eq!(edit_caret("abc", "ac"), 1);
        assert_eq!(edit_caret("aa", "aaa"), 3);
        assert_eq!(edit_caret("abc", ""), 0);
        assert_eq!(edit_caret("a\u{1F600}", "a\u{1F600}b"), 4);
    }
}
