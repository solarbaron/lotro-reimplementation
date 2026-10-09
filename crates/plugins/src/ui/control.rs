// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.UI.Control`, `ScrollableControl` and `ControlList` (`lua-plugin-api.md`
//! §13.1–§13.2, §13.7, §13.8, §13.14–§13.19), and the helpers shared by every control
//! class.

use std::rc::Rc;

use mlua::{Lua, Table, Value};
use tessera_ui::Background as UiBackground;

use super::model::{
    Args, Background, Ctl, Kind, ListView, ObjData, Styled, build_args, int, new_colour, num,
    truthy,
};
use super::reg::{Cx, any_ctl, class, control_ctor, def, def_static};
use super::state::{Shared, anchor, obj_id, object_of, set_strong};
use crate::api::{NativeApi, apartment_name, check_image, fire_event, script_error};

/// Run `f` on the record of control `id`.
pub(crate) fn with_ctl<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut Ctl) -> R) -> Option<R> {
    match sh.st.borrow_mut().objs.get_mut(&id).map(|o| &mut o.data) {
        Some(ObjData::Ctl(c)) => Some(f(c)),
        _ => None,
    }
}

/// Read the record of control `id`.
pub(crate) fn ctl_get<R>(sh: &Shared, id: u64, f: impl FnOnce(&Ctl) -> R) -> Option<R> {
    match sh.st.borrow().objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => Some(f(c)),
        _ => None,
    }
}

/// Run `f` on the UI; `None` when the client holds it.
pub(crate) fn with_ui<R>(sh: &Shared, f: impl FnOnce(&mut tessera_ui::Ui) -> R) -> Option<R> {
    sh.ui.try_borrow_mut().ok().map(|mut u| f(&mut u))
}

/// Fire `name` on object `id` now, inside the current call (the plugin layer's own events,
/// §9.6: `EnabledChanged`, `Closing`/`Closed`, `ValueChanged`, …). Returns the argument
/// table after the handlers ran.
pub(crate) fn fire_now(lua: &Lua, sh: &Shared, id: u64, name: &str, args: &Args) -> Option<Table> {
    let obj = object_of(lua, id)?;
    let ap = apartment_name(lua).unwrap_or_default();
    let a = build_args(lua, sh, &ap, args).ok()?;
    let _ = fire_event(lua, &obj, name, a.clone());
    Some(a)
}

/// Keep object `id` alive by the rules of the module documentation: a control strongly
/// while it is a shown top-level window or a broadcast subscriber, otherwise through its
/// owner; a context menu strongly while it is open.
pub(crate) fn refresh_pin(lua: &Lua, sh: &Shared, id: u64) {
    let open_menu = {
        let st = sh.st.borrow();
        match st.objs.get(&id).map(|o| &o.data) {
            Some(ObjData::Menu(true, _)) => Some(st.menus.iter().any(|m| m.owner == id)),
            _ => None,
        }
    };
    if let Some(open) = open_menu {
        set_strong(lua, id, open);
        return;
    }
    let Some((key, kind, visible, updates, keys)) = ctl_get(sh, id, |c| {
        (c.key, c.kind, c.visible, c.wants_updates, c.wants_keys)
    }) else {
        return;
    };
    let top = with_ui(sh, |u| u.control_parent(key).is_none()).unwrap_or(true);
    let strong = (kind == Kind::Window && top && visible) || updates || keys;
    set_strong(lua, id, strong);
}

/// Set the owner (anchor) of control `id`.
pub(crate) fn set_owner(lua: &Lua, sh: &Shared, id: u64, owner: Option<u64>) {
    let old = with_ctl(sh, id, |c| std::mem::replace(&mut c.owner, owner)).flatten();
    anchor(lua, id, old, owner);
}

/// Apply the plugin's visibility flag and the layer's own hiding to the element.
pub(crate) fn apply_visibility(sh: &Shared, id: u64, quiet: bool) {
    let Some((key, shown)) = ctl_get(sh, id, |c| (c.key, c.visible && !c.layer_hidden)) else {
        return;
    };
    let changed = with_ui(sh, |u| {
        let was = u.is_visible(key);
        u.control_set_visible(key, shown);
        was != shown
    })
    .unwrap_or(false);
    if quiet && changed {
        sh.st.borrow_mut().quiet_visible.insert(key);
    }
}

/// `SetParent` (§13.1): attach to `parent` (a control record) or detach.
pub(crate) fn set_parent(lua: &Lua, sh: &Shared, id: u64, parent: Option<u64>) -> mlua::Result<()> {
    let key = sh
        .key_of(id)
        .ok_or_else(|| script_error("the control no longer exists"))?;
    let pkey = match parent {
        Some(p) => Some(
            sh.key_of(p)
                .ok_or_else(|| script_error("SetParent: the parent is not a control"))?,
        ),
        None => None,
    };
    if pkey == Some(key) {
        return Err(script_error(
            "SetParent: a control cannot be its own parent",
        ));
    }
    with_ui(sh, |u| u.control_set_parent(key, pkey))
        .ok_or_else(|| script_error("the user interface is busy"))?
        .map_err(|e| script_error(format!("SetParent: {e}")))?;
    // A control moved to another parent leaves the list it was an item of.
    let old_owner = ctl_get(sh, id, |c| c.owner).flatten();
    if old_owner.is_some() && old_owner != parent {
        let mut st = sh.st.borrow_mut();
        super::lists::forget_item(&mut st, id);
    }
    set_owner(lua, sh, id, parent);
    refresh_pin(lua, sh, id);
    if let Some(o) = old_owner
        && Some(o) != parent
    {
        super::lists::relayout(sh, o);
    }
    Ok(())
}

/// After a geometry change of control `id`: lay out what depends on it.
pub(crate) fn after_geometry(sh: &Shared, id: u64) {
    let Some((kind, styled, owner)) = ctl_get(sh, id, |c| (c.kind, c.styled, c.owner)) else {
        return;
    };
    match kind {
        Kind::ListBox | Kind::TreeView => super::lists::relayout(sh, id),
        Kind::ScrollBar => super::scroll::layout_parts(sh, id),
        Kind::Window if styled != Styled::Plain => super::window::layout_frame(sh, id),
        _ => {}
    }
    if let Some(o) = owner {
        let is_list =
            ctl_get(sh, o, |c| matches!(c.kind, Kind::ListBox | Kind::TreeView)).unwrap_or(false);
        if is_list {
            super::lists::relayout(sh, o);
        }
    }
}

/// Work after creating control `id` (the class's initial element state).
pub(crate) fn init(lua: &Lua, sh: &Rc<Shared>, id: u64) {
    let Some((key, kind, styled)) = ctl_get(sh, id, |c| (c.key, c.kind, c.styled)) else {
        return;
    };
    if kind == Kind::TextBox {
        with_ui(sh, |u| {
            let pid = u.vocab().pids.text_editable;
            u.set_property(
                key,
                pid,
                tessera_formats::property::PropertyValue::Bool(true),
            );
        });
    }
    if kind == Kind::Window && styled != Styled::Plain {
        super::window::build_frame(lua, sh, id);
    }
    if kind == Kind::ScrollBar && styled != Styled::Plain {
        super::scroll::build_neutral_thumb(lua, sh, id);
    }
}

/// The plugin-relative path of a checked image: the last components of the indexed file
/// (as many as the plugin path has), joined with `/`.
fn relative_image_path(requested: &str, found: &std::path::Path) -> String {
    let n = crate::paths::split_resource_path(requested).map_or(0, |c| c.len());
    let comps: Vec<String> = found
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let start = comps.len().saturating_sub(n);
    comps[start..].join("/")
}

/// `SetBackground(v)` (§13.8): checks a path at call time (decision 0015 §4.4).
pub(crate) fn set_background(
    lua: &Lua,
    sh: &Shared,
    id: u64,
    v: Option<&Value>,
) -> mlua::Result<()> {
    let key = sh
        .key_of(id)
        .ok_or_else(|| script_error("the control no longer exists"))?;
    let (stored, ui_bg) = match v {
        Some(Value::Number(_) | Value::Integer(_)) => {
            let n = num(v).unwrap_or(0.0);
            let did = if (0.0..4_294_967_296.0).contains(&n) {
                n.trunc() as u32
            } else {
                0
            };
            (Background::Did(n), UiBackground::Did(did))
        }
        Some(Value::String(s)) => {
            let path = s.to_str().map(|s| s.to_owned()).unwrap_or_default();
            if path.is_empty() {
                (Background::None, UiBackground::None)
            } else {
                let img = check_image(lua, &path).map_err(|e| script_error(e.to_string()))?;
                let rel = relative_image_path(&path, &img.path);
                (Background::Path(path), UiBackground::File(rel))
            }
        }
        Some(Value::Table(t)) => {
            let gid = obj_id(lua, t);
            let bg = gid.and_then(|g| match sh.st.borrow().objs.get(&g).map(|o| &o.data) {
                Some(ObjData::Graphic(b)) => Some(b.clone()),
                _ => None,
            });
            match (gid, bg) {
                (Some(g), Some(Background::Did(n))) => (
                    Background::Graphic(g),
                    UiBackground::Did(if (0.0..4_294_967_296.0).contains(&n) {
                        n.trunc() as u32
                    } else {
                        0
                    }),
                ),
                (Some(g), Some(Background::Path(p))) => {
                    let img = check_image(lua, &p).map_err(|e| script_error(e.to_string()))?;
                    let rel = relative_image_path(&p, &img.path);
                    (Background::Graphic(g), UiBackground::File(rel))
                }
                (Some(g), Some(_)) => (Background::Graphic(g), UiBackground::None),
                _ => (Background::None, UiBackground::None),
            }
        }
        _ => (Background::None, UiBackground::None),
    };
    with_ui(sh, |u| u.control_set_background(key, ui_bg));
    with_ctl(sh, id, |c| c.background = stored);
    Ok(())
}

fn limit_get(cx: &Cx<'_>, i: usize) -> f64 {
    let key = cx.sh.key_of(cx.id);
    key.and_then(|k| with_ui(cx.sh, |u| u.control_limits(k)[i]))
        .flatten()
        .map_or(0.0, f64::from)
}

fn limit_set(cx: &Cx<'_>, i: usize, v: i32) {
    if let Some(k) = cx.sh.key_of(cx.id) {
        with_ui(cx.sh, |u| {
            let mut l = u.control_limits(k);
            l[i] = Some(v);
            u.control_set_limits(k, l);
        });
    }
}

fn key(cx: &Cx<'_>) -> mlua::Result<tessera_ui::ElementKey> {
    cx.sh
        .key_of(cx.id)
        .ok_or_else(|| script_error("the control no longer exists"))
}

fn ui_do<R>(
    cx: &Cx<'_>,
    f: impl FnOnce(&mut tessera_ui::Ui, tessera_ui::ElementKey) -> R,
) -> mlua::Result<R> {
    let k = key(cx)?;
    with_ui(cx.sh, |u| f(u, k)).ok_or_else(|| script_error("the user interface is busy"))
}

fn set_size(cx: &Cx<'_>, w: Option<i32>, h: Option<i32>) -> mlua::Result<()> {
    ui_do(cx, |u, k| {
        let (cw, ch) = u.control_size(k);
        u.control_set_size(k, w.unwrap_or(cw), h.unwrap_or(ch));
    })?;
    after_geometry(cx.sh, cx.id);
    Ok(())
}

fn set_pos(cx: &Cx<'_>, x: Option<i32>, y: Option<i32>) -> mlua::Result<()> {
    ui_do(cx, |u, k| {
        let (cx0, cy0) = u.control_position(k);
        u.control_set_position(k, x.unwrap_or(cx0), y.unwrap_or(cy0));
    })
}

/// The `ControlList` object of control `id` (created once, kept by the control).
pub(crate) fn controls_list(lua: &Lua, sh: &Shared, ap: &str, id: u64) -> mlua::Result<Table> {
    if let Some(l) = ctl_get(sh, id, |c| c.controls_list).flatten()
        && let Some(t) = object_of(lua, l)
    {
        return Ok(t);
    }
    let t = super::model::new_object(
        lua,
        sh,
        ap,
        "ControlList",
        ObjData::List(ListView::Controls(id)),
    )?;
    if let Some(l) = obj_id(lua, &t) {
        with_ctl(sh, id, |c| c.controls_list = Some(l));
        anchor(lua, l, None, Some(id));
    }
    Ok(t)
}

/// The child controls of control `id` that are plugin objects, in sibling order.
pub(crate) fn child_ids(sh: &Shared, id: u64) -> Vec<u64> {
    let Some(k) = sh.key_of(id) else {
        return Vec::new();
    };
    let kids = with_ui(sh, |u| u.control_children(k)).unwrap_or_default();
    let st = sh.st.borrow();
    kids.iter()
        .filter_map(|k| st.by_key.get(k).copied())
        .filter(|cid| {
            matches!(st.objs.get(cid).map(|o| &o.data), Some(ObjData::Ctl(c)) if c.internal.is_none())
        })
        .collect()
}

/// A control argument's record number.
pub(crate) fn ctl_arg(lua: &Lua, sh: &Shared, v: Option<&Value>) -> Option<u64> {
    let Some(Value::Table(t)) = v else {
        return None;
    };
    let id = obj_id(lua, t)?;
    sh.key_of(id).map(|_| id)
}

/// Install `Control` and `ScrollableControl`.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    object: &Table,
) -> mlua::Result<()> {
    let c = class(
        api,
        sh,
        ui,
        "Control",
        "Control",
        Some(object),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    class(
        api,
        sh,
        ui,
        "ScrollableControl",
        "ScrollableControl",
        Some(&c),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    let a = any_ctl;

    // Geometry.
    def(api, sh, &c, "GetLeft", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_position(k).0))
    })?;
    def(api, sh, &c, "GetTop", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_position(k).1))
    })?;
    def(api, sh, &c, "GetPosition", a, |cx, _| {
        ui_do(cx, |u, k| {
            let (x, y) = u.control_position(k);
            (f64::from(x), f64::from(y))
        })
    })?;
    def(api, sh, &c, "SetLeft", a, |cx, args| {
        set_pos(cx, Some(int(args.front())), None)
    })?;
    def(api, sh, &c, "SetTop", a, |cx, args| {
        set_pos(cx, None, Some(int(args.front())))
    })?;
    def(api, sh, &c, "SetPosition", a, |cx, args| {
        set_pos(cx, Some(int(args.front())), Some(int(args.get(1))))
    })?;
    def(api, sh, &c, "GetWidth", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_size(k).0))
    })?;
    def(api, sh, &c, "GetHeight", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_size(k).1))
    })?;
    def(api, sh, &c, "GetSize", a, |cx, _| {
        ui_do(cx, |u, k| {
            let (w, h) = u.control_size(k);
            (f64::from(w), f64::from(h))
        })
    })?;
    def(api, sh, &c, "SetWidth", a, |cx, args| {
        set_size(cx, Some(int(args.front())), None)
    })?;
    def(api, sh, &c, "SetHeight", a, |cx, args| {
        set_size(cx, None, Some(int(args.front())))
    })?;
    def(api, sh, &c, "SetSize", a, |cx, args| {
        set_size(cx, Some(int(args.front())), Some(int(args.get(1))))
    })?;
    def(api, sh, &c, "PointToClient", a, |cx, args| {
        let (x, y) = (int(args.front()), int(args.get(1)));
        ui_do(cx, |u, k| {
            let (a, b) = u.point_to_client(k, x, y);
            (f64::from(a), f64::from(b))
        })
    })?;
    def(api, sh, &c, "PointToScreen", a, |cx, args| {
        let (x, y) = (int(args.front()), int(args.get(1)));
        ui_do(cx, |u, k| {
            let (a, b) = u.point_to_screen(k, x, y);
            (f64::from(a), f64::from(b))
        })
    })?;
    def(api, sh, &c, "GetMousePosition", a, |cx, _| {
        ui_do(cx, |u, k| {
            let (x, y) = u.control_mouse_position(k);
            (f64::from(x), f64::from(y))
        })
    })?;

    // Size limits (§13.15).
    for (i, get, set) in [
        (0usize, "GetMinimumWidth", "SetMinimumWidth"),
        (1, "GetMinimumHeight", "SetMinimumHeight"),
        (2, "GetMaximumWidth", "SetMaximumWidth"),
        (3, "GetMaximumHeight", "SetMaximumHeight"),
    ] {
        def(api, sh, &c, get, a, move |cx, _| Ok(limit_get(cx, i)))?;
        def(api, sh, &c, set, a, move |cx, args| {
            limit_set(cx, i, int(args.front()));
            Ok(())
        })?;
    }
    for (base, get, set) in [
        (0usize, "GetMinimumSize", "SetMinimumSize"),
        (2, "GetMaximumSize", "SetMaximumSize"),
    ] {
        def(api, sh, &c, get, a, move |cx, _| {
            Ok((limit_get(cx, base), limit_get(cx, base + 1)))
        })?;
        def(api, sh, &c, set, a, move |cx, args| {
            limit_set(cx, base, int(args.front()));
            limit_set(cx, base + 1, int(args.get(1)));
            Ok(())
        })?;
    }

    // Edge attachments (§13.16).
    def(api, sh, &c, "AttachEdges", a, |cx, args| {
        let e = [
            int(args.front()),
            int(args.get(1)),
            int(args.get(2)),
            int(args.get(3)),
        ];
        ui_do(cx, |u, k| u.control_attach_edges(k, e))?;
        after_geometry(cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &c, "GetEdgeAttachments", a, |cx, _| {
        ui_do(cx, |u, k| {
            let e = u.control_edges(k);
            (
                f64::from(e[0]),
                f64::from(e[1]),
                f64::from(e[2]),
                f64::from(e[3]),
            )
        })
    })?;

    // Tree.
    def(api, sh, &c, "GetParent", a, |cx, _| {
        let k = key(cx)?;
        let p = with_ui(cx.sh, |u| u.control_parent(k)).flatten();
        let pid = p.and_then(|p| cx.sh.st.borrow().by_key.get(&p).copied());
        Ok(pid.and_then(|p| object_of(cx.lua, p)))
    })?;
    def(api, sh, &c, "SetParent", a, |cx, args| {
        let parent = match args.front() {
            None | Some(Value::Nil) => None,
            v => Some(
                ctl_arg(cx.lua, cx.sh, v)
                    .ok_or_else(|| script_error("SetParent: the parent is not a control"))?,
            ),
        };
        set_parent(cx.lua, cx.sh, cx.id, parent)
    })?;
    def(api, sh, &c, "GetControls", a, |cx, _| {
        controls_list(cx.lua, cx.sh, &cx.ap, cx.id)
    })?;
    def(api, sh, &c, "GetZOrder", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_z_order(k)))
    })?;
    def(api, sh, &c, "SetZOrder", a, |cx, args| {
        let n = int(args.front());
        ui_do(cx, |u, k| u.control_set_z_order(k, n))
    })?;

    // State.
    def(api, sh, &c, "IsVisible", a, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| c.visible).unwrap_or(false))
    })?;
    def(api, sh, &c, "SetVisible", a, |cx, args| {
        let v = truthy(args.front());
        let (changed, hidden_by_layer) = with_ctl(cx.sh, cx.id, |c| {
            (std::mem::replace(&mut c.visible, v) != v, c.layer_hidden)
        })
        .unwrap_or((false, false));
        apply_visibility(cx.sh, cx.id, false);
        // §9.6: `VisibleChanged` follows the control's own flag. A node the layer hides
        // (collapsed parent, filtered) keeps its element hidden, so no notification comes;
        // the event goes through the queue here instead.
        if changed && hidden_by_layer {
            cx.sh.push(super::state::Pending::Fire {
                target: cx.id,
                name: "VisibleChanged",
                args: Args::Empty,
            });
        }
        refresh_pin(cx.lua, cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &c, "IsDisplayed", a, |cx, _| {
        ui_do(cx, |u, k| u.control_is_displayed(k))
    })?;
    def(api, sh, &c, "IsEnabled", a, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| c.enabled).unwrap_or(true))
    })?;
    def(api, sh, &c, "SetEnabled", a, |cx, args| {
        let v = truthy(args.front());
        let changed =
            with_ctl(cx.sh, cx.id, |c| std::mem::replace(&mut c.enabled, v) != v).unwrap_or(false);
        ui_do(cx, |u, k| u.control_set_enabled(k, v))?;
        if changed {
            fire_now(cx.lua, cx.sh, cx.id, "EnabledChanged", &Args::Empty);
        }
        Ok(())
    })?;
    def(api, sh, &c, "GetOpacity", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_opacity(k)))
    })?;
    def(api, sh, &c, "SetOpacity", a, |cx, args| {
        let o = num(args.front()).unwrap_or(0.0) as f32;
        ui_do(cx, |u, k| u.control_set_opacity(k, o))
    })?;
    def(api, sh, &c, "IsMouseVisible", a, |cx, _| {
        ui_do(cx, |u, k| u.mouse_visible(k))
    })?;
    def(api, sh, &c, "SetMouseVisible", a, |cx, args| {
        let v = truthy(args.front());
        ui_do(cx, |u, k| u.control_set_mouse_visible(k, v))
    })?;
    def(api, sh, &c, "GetWantsKeyEvents", a, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| c.wants_keys).unwrap_or(false))
    })?;
    def(api, sh, &c, "SetWantsKeyEvents", a, |cx, args| {
        let v = truthy(args.front());
        let changed = with_ctl(cx.sh, cx.id, |c| {
            std::mem::replace(&mut c.wants_keys, v) != v
        })
        .unwrap_or(false);
        if changed {
            let k = ui_do(cx, |u, k| {
                u.control_set_wants_keys(k, v);
                k
            })?;
            if v && let Some(log) = cx.sh.st.borrow_mut().key_log.as_mut() {
                log.push(k);
            }
        }
        refresh_pin(cx.lua, cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &c, "GetWantsUpdates", a, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| c.wants_updates).unwrap_or(false))
    })?;
    def(api, sh, &c, "SetWantsUpdates", a, |cx, args| {
        let v = truthy(args.front());
        // §9.4: the value it already has changes nothing (no re-subscription).
        let changed = with_ctl(cx.sh, cx.id, |c| {
            std::mem::replace(&mut c.wants_updates, v) != v
        })
        .unwrap_or(false);
        if changed {
            let k = ui_do(cx, |u, k| {
                u.control_set_wants_updates(k, v);
                k
            })?;
            // A broadcast running now calls it in the same tick (§9.4).
            if v && let Some(log) = cx.sh.st.borrow_mut().update_log.as_mut() {
                log.push(k);
            }
        }
        refresh_pin(cx.lua, cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &c, "GetAllowDrop", a, |cx, _| {
        let k = key(cx)?;
        let v = with_ui(cx.sh, |u| {
            u.property(k, u.vocab().pids.container)
                .is_some_and(|p| matches!(p, tessera_formats::property::PropertyValue::Bool(true)))
        });
        Ok(v.unwrap_or(false))
    })?;
    def(api, sh, &c, "SetAllowDrop", a, |cx, args| {
        let v = truthy(args.front());
        ui_do(cx, |u, k| u.control_set_allow_drop(k, v))
    })?;
    def(api, sh, &c, "Focus", a, |cx, _| {
        ui_do(cx, |u, k| u.control_focus(k))
    })?;
    def(api, sh, &c, "HasFocus", a, |cx, _| {
        ui_do(cx, |u, k| u.control_has_focus(k))
    })?;

    // Look (§13.17–§13.19).
    def(api, sh, &c, "GetBackColor", a, |cx, _| {
        let col = ui_do(cx, |u, k| u.control_back_colour(k))?;
        new_colour(cx.lua, col)
    })?;
    def(api, sh, &c, "SetBackColor", a, |cx, args| {
        let col = match args.front() {
            None | Some(Value::Nil) => None,
            v => super::colour::read(v),
        };
        ui_do(cx, |u, k| u.control_set_back_colour(k, col))
    })?;
    def(api, sh, &c, "GetBackColorBlendMode", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_back_colour_blend_mode(k)))
    })?;
    def(api, sh, &c, "SetBackColorBlendMode", a, |cx, args| {
        let n = int(args.front()) as u32;
        ui_do(cx, |u, k| u.control_set_back_colour_blend_mode(k, n))?
            .map_err(|e| script_error(e.to_string()))
    })?;
    def(api, sh, &c, "GetBlendMode", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_blend_mode(k)))
    })?;
    def(api, sh, &c, "SetBlendMode", a, |cx, args| {
        let n = int(args.front()) as u32;
        ui_do(cx, |u, k| u.control_set_blend_mode(k, n))?.map_err(|e| script_error(e.to_string()))
    })?;
    def(api, sh, &c, "GetBackground", a, |cx, _| {
        let bg = ctl_get(cx.sh, cx.id, |c| c.background.clone()).unwrap_or_default();
        Ok(match bg {
            Background::None => Value::Nil,
            Background::Did(n) => Value::Number(n),
            Background::Path(p) => Value::String(cx.lua.create_string(&p)?),
            Background::Graphic(g) => object_of(cx.lua, g).map_or(Value::Nil, Value::Table),
        })
    })?;
    def(api, sh, &c, "SetBackground", a, |cx, args| {
        set_background(cx.lua, cx.sh, cx.id, args.front())
    })?;
    def(api, sh, &c, "GetStretchMode", a, |cx, _| {
        ui_do(cx, |u, k| f64::from(u.control_stretch_mode(k)))
    })?;
    def(api, sh, &c, "SetStretchMode", a, |cx, args| {
        let n = int(args.front());
        ui_do(cx, |u, k| u.control_set_stretch_mode(k, n))
    })?;

    // Statics.
    def_static(api, sh, &c, "IsAltKeyDown", |_, sh, _| {
        Ok(sh.st.borrow().modifiers.alt)
    })?;
    def_static(api, sh, &c, "IsControlKeyDown", |_, sh, _| {
        Ok(sh.st.borrow().modifiers.control)
    })?;
    def_static(api, sh, &c, "IsShiftKeyDown", |_, sh, _| {
        Ok(sh.st.borrow().modifiers.shift)
    })?;
    Ok(())
}
