// SPDX-License-Identifier: GPL-3.0-or-later
//! `ContextMenu` and `MenuItem` (`lua-plugin-api.md` §13.10).
//!
//! An open menu is a popup window of internal rows (no Lua objects) drawn with Tessera's
//! neutral art: at most one menu is open at a time; it opens at the pointer (or a given
//! point) moved fully onto the screen; a click on a row fires the item's `Click` and
//! closes the menu, a row with sub-items opens them beside it; any press outside the open
//! popups closes them (the popups are activated, so a press elsewhere deactivates them).

use std::rc::Rc;

use mlua::{Table, Value};
use tessera_formats::property::PropertyValue;
use tessera_ui::control::plugin_colour;
use tessera_ui::{ElementKey, Ui};

use super::model::{Args, Kind, ListView, MenuState, ObjData, Role, bind_new, int, truthy};
use super::reg::{Cx, class, create_part_with, def};
use super::state::{Pending, Shared, State, anchor, obj_id, object_of, push};
use crate::api::NativeApi;

/// Height of a menu row, in pixels (Tessera's neutral art).
const ROW_HEIGHT: i32 = 20;
/// Smallest menu width, in pixels.
const MIN_WIDTH: i32 = 120;
/// Width estimate per character of a row's text, in pixels (no text measurement is
/// available to the layer).
const CHAR_WIDTH: i32 = 7;
/// Most rows shown by one popup.
const MAX_ROWS: usize = 256;
/// Deepest sub-menu level.
const MAX_LEVELS: usize = 8;
/// Z-order of popups: above every plugin window.
const POPUP_Z: i32 = 1 << 20;

/// An open menu popup.
#[derive(Debug, Clone)]
pub(crate) struct OpenMenu {
    pub apartment: String,
    /// The `ContextMenu` or parent `MenuItem` whose items it shows.
    pub owner: u64,
    pub popup: u64,
    pub popup_key: ElementKey,
    /// (row record, item record).
    pub rows: Vec<(u64, u64)>,
}

fn menu_obj(d: &ObjData) -> bool {
    matches!(d, ObjData::Menu(true, _))
}

fn item_obj(d: &ObjData) -> bool {
    matches!(d, ObjData::Menu(false, _))
}

fn any_menu(d: &ObjData) -> bool {
    matches!(d, ObjData::Menu(..))
}

fn ms<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut MenuState) -> R) -> Option<R> {
    match sh.st.borrow_mut().objs.get_mut(&id).map(|o| &mut o.data) {
        Some(ObjData::Menu(_, m)) => Some(f(m)),
        _ => None,
    }
}

fn items_list(cx: &Cx<'_>) -> mlua::Result<Table> {
    if let Some(l) = ms(cx.sh, cx.id, |m| m.list).flatten()
        && let Some(t) = object_of(cx.lua, l)
    {
        return Ok(t);
    }
    let t = super::model::new_object(
        cx.lua,
        cx.sh,
        &cx.ap,
        "MenuItemList",
        ObjData::List(ListView::MenuItems(cx.id)),
    )?;
    if let Some(l) = obj_id(cx.lua, &t) {
        ms(cx.sh, cx.id, |m| m.list = Some(l));
        anchor(cx.lua, l, None, Some(cx.id));
    }
    Ok(t)
}

/// Close every open popup. The closed menus' pins are re-evaluated at the next call that
/// has their apartment's Lua state (an open menu is pinned, `control::refresh_pin`).
pub(crate) fn close_all(ui: &mut Ui, st: &mut State) {
    let menus = std::mem::take(&mut st.menus);
    for m in menus {
        st.repin.push(m.owner);
        for (row, _) in &m.rows {
            if let Some(o) = st.objs.remove(row) {
                if let ObjData::Ctl(c) = &o.data {
                    st.by_key.remove(&c.key);
                }
                if let Some(n) = st.per_apartment.get_mut(&o.apartment) {
                    *n = n.saturating_sub(1);
                }
            }
        }
        if let Some(o) = st.objs.remove(&m.popup)
            && let Some(n) = st.per_apartment.get_mut(&o.apartment)
        {
            *n = n.saturating_sub(1);
        }
        st.by_key.remove(&m.popup_key);
        ui.destroy_control(m.popup_key);
    }
}

/// Close the popups from level `level` on.
fn close_from(ui: &mut Ui, st: &mut State, level: usize) {
    if level >= st.menus.len() {
        return;
    }
    let rest = st.menus.split_off(level);
    let keep = std::mem::replace(&mut st.menus, rest);
    close_all(ui, st);
    st.menus = keep;
}

/// Open the items of `owner` as a popup at (x, y) (screen pixels), as level `level`.
fn open(ui: &mut Ui, st: &mut State, ap: &str, owner: u64, x: i32, y: i32, level: usize) {
    if level >= MAX_LEVELS {
        return;
    }
    close_from(ui, st, level);
    let items: Vec<(u64, MenuState)> = match st.objs.get(&owner).map(|o| &o.data) {
        Some(ObjData::Menu(_, m)) => m
            .items
            .iter()
            .take(MAX_ROWS)
            .filter_map(|i| match st.objs.get(i).map(|o| &o.data) {
                Some(ObjData::Menu(false, im)) => Some((*i, im.clone())),
                _ => None,
            })
            .collect(),
        _ => return,
    };
    if items.is_empty() {
        return;
    }
    let Some((popup, pkey)) =
        create_part_with(ui, st, ap, owner, None, Kind::Window, Role::MenuPopup)
    else {
        return;
    };
    let labels: Vec<String> = items
        .iter()
        .map(|(_, m)| {
            let mut s = String::new();
            s.push_str(if m.checked { "\u{2713} " } else { "  " });
            s.push_str(&m.text);
            if !m.items.is_empty() {
                s.push_str("  >");
            }
            s
        })
        .collect();
    let width = labels
        .iter()
        .map(|l| {
            (l.chars().count() as i32)
                .saturating_mul(CHAR_WIDTH)
                .saturating_add(16)
        })
        .max()
        .unwrap_or(0)
        .max(MIN_WIDTH);
    let height = (items.len() as i32).saturating_mul(ROW_HEIGHT);
    let (sw, sh) = ui.screen_size();
    let px = x.min(sw.saturating_sub(width)).max(0);
    let py = y.min(sh.saturating_sub(height)).max(0);
    ui.control_set_size(pkey, width, height);
    ui.control_set_position(pkey, px, py);
    ui.control_set_z_order(pkey, POPUP_Z);
    ui.control_set_back_colour(pkey, Some([0.95, 0.10, 0.10, 0.12]));
    let fore = st.text_pids.font_color;
    let mut rows = Vec::new();
    for (i, ((item, m), label)) in items.iter().zip(labels).enumerate() {
        let Some((row, rkey)) =
            create_part_with(ui, st, ap, popup, Some(pkey), Kind::Label, Role::MenuRow(i))
        else {
            continue;
        };
        ui.control_set_position(rkey, 0, (i as i32).saturating_mul(ROW_HEIGHT));
        ui.control_set_size(rkey, width, ROW_HEIGHT);
        ui.set_text(rkey, Some(label));
        let c = if m.enabled {
            [1.0, 0.92, 0.92, 0.92]
        } else {
            [1.0, 0.5, 0.5, 0.5]
        };
        ui.set_property(
            rkey,
            fore,
            PropertyValue::Colour(plugin_colour(c[0], c[1], c[2], c[3]).to_le_bytes()),
        );
        ui.control_set_visible(rkey, true);
        rows.push((row, *item));
    }
    ui.control_set_visible(pkey, true);
    st.menus.push(OpenMenu {
        apartment: ap.to_owned(),
        owner,
        popup,
        popup_key: pkey,
        rows,
    });
    ui.activate(pkey);
}

/// A click on row `row` of an open popup (from the event translation).
pub(crate) fn row_clicked(ui: &mut Ui, st: &mut State, row: u64) {
    let Some((level, item)) = st
        .menus
        .iter()
        .enumerate()
        .find_map(|(l, m)| m.rows.iter().find(|r| r.0 == row).map(|r| (l, r.1)))
    else {
        return;
    };
    let (enabled, has_sub) = match st.objs.get(&item).map(|o| &o.data) {
        Some(ObjData::Menu(false, m)) => (m.enabled, !m.items.is_empty()),
        _ => return,
    };
    if !enabled {
        return;
    }
    if has_sub {
        let ap = st.menus[level].apartment.clone();
        let rkey = match st.objs.get(&row).map(|o| &o.data) {
            Some(ObjData::Ctl(c)) => c.key,
            _ => return,
        };
        let (rx, ry) = ui.point_to_screen(rkey, 0, 0);
        let (rw, _) = ui.control_size(rkey);
        open(ui, st, &ap, item, rx.saturating_add(rw), ry, level + 1);
        return;
    }
    push(
        st,
        Pending::Fire {
            target: item,
            name: "Click",
            args: Args::Empty,
        },
    );
    close_all(ui, st);
}

/// A popup lost activation: close every popup unless the active window is one of them.
pub(crate) fn deactivated(ui: &mut Ui, st: &mut State) {
    let active = ui.active_panel();
    if st.menus.iter().any(|m| Some(m.popup_key) == active) {
        return;
    }
    close_all(ui, st);
}

/// Open context menu `cx.id` at (x, y), closing any open menu first. The open menu is
/// pinned, so a menu built in a local of a handler stays alive (with its items, which it
/// anchors) until it closes.
fn show(cx: &Cx<'_>, x: i32, y: i32) {
    {
        let Ok(mut ui) = cx.sh.ui.try_borrow_mut() else {
            return;
        };
        let mut st = cx.sh.st.borrow_mut();
        close_all(&mut ui, &mut st);
        open(&mut ui, &mut st, &cx.ap, cx.id, x, y, 0);
    }
    super::control::refresh_pin(cx.lua, cx.sh, cx.id);
}

/// Install `ContextMenu` and `MenuItem`.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    object: &Table,
) -> mlua::Result<()> {
    let cm = class(
        api,
        sh,
        ui,
        "ContextMenu",
        "ContextMenu",
        Some(object),
        |lua: &mlua::Lua,
         sh: &Rc<Shared>,
         ap: &str,
         this: &Table,
         cls: &Table,
         _args: mlua::MultiValue| {
            bind_new(
                lua,
                sh,
                ap,
                this,
                cls,
                ObjData::Menu(
                    true,
                    MenuState {
                        enabled: true,
                        ..MenuState::default()
                    },
                ),
            )
            .map(|_| ())
        },
    )?;
    let mi = class(
        api,
        sh,
        ui,
        "MenuItem",
        "MenuItem",
        Some(object),
        |lua: &mlua::Lua,
         sh: &Rc<Shared>,
         ap: &str,
         this: &Table,
         cls: &Table,
         args: mlua::MultiValue| {
            let text = super::text::bound_title(super::text::text_arg(args.front()));
            let enabled = match args.get(1) {
                None | Some(Value::Nil) => true,
                v => truthy(v),
            };
            let checked = truthy(args.get(2));
            bind_new(
                lua,
                sh,
                ap,
                this,
                cls,
                ObjData::Menu(
                    false,
                    MenuState {
                        text,
                        enabled,
                        checked,
                        ..MenuState::default()
                    },
                ),
            )
            .map(|_| ())
        },
    )?;
    def(api, sh, &cm, "GetItems", menu_obj, |cx, _| items_list(cx))?;
    def(api, sh, &cm, "ShowMenu", menu_obj, |cx, _| {
        let at = cx
            .sh
            .ui
            .try_borrow()
            .ok()
            .map(|u| u.pointer().unwrap_or((0, 0)));
        if let Some((x, y)) = at {
            show(cx, x, y);
        }
        Ok(())
    })?;
    def(api, sh, &cm, "ShowMenuAt", menu_obj, |cx, args| {
        let (x, y) = (int(args.front()), int(args.get(1)));
        show(cx, x, y);
        Ok(())
    })?;
    def(api, sh, &cm, "Close", menu_obj, |cx, _| {
        let Ok(mut ui) = cx.sh.ui.try_borrow_mut() else {
            return Ok(());
        };
        let mut st = cx.sh.st.borrow_mut();
        if st.menus.first().is_some_and(|m| m.owner == cx.id) {
            close_all(&mut ui, &mut st);
        }
        Ok(())
    })?;
    def(api, sh, &mi, "GetItems", item_obj, |cx, _| items_list(cx))?;
    def(api, sh, &mi, "GetText", item_obj, |cx, _| {
        Ok(ms(cx.sh, cx.id, |m| m.text.clone()).unwrap_or_default())
    })?;
    def(api, sh, &mi, "SetText", item_obj, |cx, args| {
        let t = super::text::bound_title(super::text::text_arg(args.front()));
        ms(cx.sh, cx.id, |m| m.text = t);
        Ok(())
    })?;
    def(api, sh, &mi, "IsEnabled", item_obj, |cx, _| {
        Ok(ms(cx.sh, cx.id, |m| m.enabled).unwrap_or(true))
    })?;
    def(api, sh, &mi, "SetEnabled", item_obj, |cx, args| {
        let v = truthy(args.front());
        ms(cx.sh, cx.id, |m| m.enabled = v);
        Ok(())
    })?;
    def(api, sh, &mi, "IsChecked", item_obj, |cx, _| {
        Ok(ms(cx.sh, cx.id, |m| m.checked).unwrap_or(false))
    })?;
    def(api, sh, &mi, "SetChecked", item_obj, |cx, args| {
        let v = truthy(args.front());
        ms(cx.sh, cx.id, |m| m.checked = v);
        Ok(())
    })?;
    let _ = any_menu;
    Ok(())
}
