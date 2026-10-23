// SPDX-License-Identifier: GPL-3.0-or-later
//! `Window`, `Turbine.UI.Lotro.Window` and `GoldWindow` (`lua-plugin-api.md` §13.3,
//! §13.14, §13.20, §13.21, §14.2.3), with Tessera's neutral frame for the game-styled
//! windows (§14.2.1 step 5, §14.2.6: used while the layer cannot build the layout's
//! frame).

use std::rc::Rc;

use mlua::{Lua, Table, Value};
use tessera_formats::property::PropertyValue;
use tessera_ui::Ui;

use super::control::{apply_visibility, ctl_get, fire_now, refresh_pin, with_ctl, with_ui};
use super::model::{Args, Kind, ObjData, Role, Styled, WinState, int, num, truthy};
use super::reg::{Cx, class, control_ctor, create_part, ctl_of, def};
use super::state::{Pending, Shared, Sink, State, class_table, push};
use crate::api::{NativeApi, script_error};

/// The scaling group `SetScale` and a zero group use (§13.21 rule 2).
const SCALE_GROUP: u32 = 72;
/// The `Global` scaling group (Appendix A.7). Under decision 0040 (`ui-scaling.md` §7.7) its
/// value is fixed at 1.0 for plugins: Tessera's UI scale lies below the UI-unit space, so a
/// plugin never sees it except as the `Display` size.
pub(crate) const GLOBAL_GROUP: u32 = 60;

/// `UIScale_Global` as a plugin sees it: 1.0, always (decision 0040, `ui-scaling.md` §7.7).
/// The window's "global wanted" flag (§13.21 rule 3) still selects it, so a global window
/// behaves like a non-global one numerically and never scales twice.
const PLUGIN_UI_SCALE_GLOBAL: f32 = 1.0;

fn window(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::Window])
}

fn styled_window(d: &ObjData) -> bool {
    matches!(d, ObjData::Ctl(c) if c.internal.is_none() && c.kind == Kind::Window && c.styled != Styled::Plain)
}

fn ws<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut WinState) -> R) -> Option<R> {
    with_ctl(sh, id, |c| c.window.as_deref_mut().map(f)).flatten()
}

/// `Close()` inside a plugin call (§13.3): only a visible window; `Closing` with `Cancel`,
/// then hide and `Closed`, all delivered before the call returns.
pub(crate) fn close_now(lua: &Lua, sh: &Shared, id: u64) {
    let visible = ctl_get(sh, id, |c| c.visible).unwrap_or(false);
    if !visible || ws(sh, id, |w| std::mem::replace(&mut w.closing, true)).unwrap_or(true) {
        return;
    }
    let cancel = fire_now(lua, sh, id, "Closing", &Args::Closing)
        .and_then(|a| a.get::<Value>("Cancel").ok())
        .is_some_and(|v| !matches!(v, Value::Nil | Value::Boolean(false)));
    if !cancel {
        with_ctl(sh, id, |c| c.visible = false);
        apply_visibility(sh, id, false);
        refresh_pin(lua, sh, id);
        fire_now(lua, sh, id, "Closed", &Args::Empty);
    }
    ws(sh, id, |w| w.closing = false);
}

/// The queued native `Close()` of a game-styled close button, delivered through `sink`.
pub(crate) fn close(sh: &Shared, sink: &dyn Sink, ap: &str, id: u64) {
    let visible = ctl_get(sh, id, |c| c.visible).unwrap_or(false);
    if !visible || ws(sh, id, |w| std::mem::replace(&mut w.closing, true)).unwrap_or(true) {
        return;
    }
    let cancel = sink
        .fire(sh, ap, id, "Closing", &Args::Closing)
        .and_then(|a| a.get::<Value>("Cancel").ok())
        .is_some_and(|v| !matches!(v, Value::Nil | Value::Boolean(false)));
    if !cancel {
        with_ctl(sh, id, |c| c.visible = false);
        apply_visibility(sh, id, false);
        // A hidden window is no longer pinned; the pin is re-evaluated at the next call
        // that has this apartment's Lua state.
        sh.st.borrow_mut().repin.push(id);
        sink.fire(sh, ap, id, "Closed", &Args::Empty);
    }
    ws(sh, id, |w| w.closing = false);
}

/// The group value of §13.21 rule 2.
fn group_value(sh: &Shared, group: u32) -> f32 {
    sh.st
        .borrow()
        .group_scales
        .get(&group)
        .copied()
        .unwrap_or(1.0)
}

/// Apply a marked window's effective scale (§13.21 rules 4–5).
///
/// The effective scale is the group's value `b` (§13.21 rule 2): the window's own scale when
/// one was set, otherwise the group's option value. Under decision 0040 (`ui-scaling.md`
/// §7.7) `UIScale_Global` is 1.0 for plugins, so the global factor is never applied: the
/// node scale is `b` whether or not the window is registered for global scaling, and the
/// window never scales twice. The factor is applied as a uniform, crisp node scale
/// (`window_set_scale` with no global multiply).
fn apply_scale(cx: &Cx<'_>) -> mlua::Result<()> {
    let Some((key, w)) = ctl_get(cx.sh, cx.id, |c| (c.key, c.window.as_deref().cloned())) else {
        return Ok(());
    };
    let Some(w) = w else { return Ok(()) };
    let b = w.own_scale.unwrap_or_else(|| group_value(cx.sh, w.group));
    let r = with_ui(cx.sh, |u| u.window_set_scale(key, b, false));
    match r {
        Some(Err(e)) => Err(script_error(e.to_string())),
        _ => Ok(()),
    }
}

/// Mark a parentless window (§13.21 "marked"); `false` for a window with a parent.
fn mark(cx: &Cx<'_>) -> bool {
    let Some((key, styled)) = ctl_get(cx.sh, cx.id, |c| (c.key, c.styled)) else {
        return false;
    };
    if with_ui(cx.sh, |u| u.control_parent(key).is_some()).unwrap_or(true) {
        return false;
    }
    ws(cx.sh, cx.id, |w| {
        if !w.marked {
            w.marked = true;
            w.group = SCALE_GROUP;
            if styled != Styled::Plain {
                w.global = true;
            }
        }
    });
    true
}

/// A scaling group's value changed: record it, re-apply it to the marked windows that
/// follow the group (those without an own scale, §13.21 rule 2), and fire `UIScaleChanged`
/// on every marked window of the group (§13.21 rule 8, the optional per-window scale of
/// `ui-scaling.md` §7.4).
///
/// The `Global` group (60) is ignored: under decision 0040 (`ui-scaling.md` §7.7)
/// `UIScale_Global` is fixed at 1.0 for plugins, so a change of Tessera's UI scale fires no
/// `UIScaleChanged` and `Display.GetUIGroupScale(Global)` stays 1.0. A non-finite or
/// non-positive value is rejected (it would not be a valid node scale).
pub(crate) fn scale_changed(sh: &Shared, group: u32, value: f32) {
    if group == GLOBAL_GROUP || !value.is_finite() || value <= 0.0 {
        return;
    }
    // Which marked windows follow this group (rule 2: an own scale overrides the group's
    // value), and which of them should be re-scaled to the new value.
    let mut fire: Vec<u64> = Vec::new();
    let mut reapply: Vec<(u64, tessera_ui::ElementKey)> = Vec::new();
    {
        let mut st = sh.st.borrow_mut();
        st.group_scales.insert(group, value);
        for (id, o) in &st.objs {
            if let ObjData::Ctl(c) = &o.data
                && let Some(w) = c.window.as_deref()
                && w.marked
                && w.group == group
            {
                fire.push(*id);
                if w.own_scale.is_none() {
                    reapply.push((*id, c.key));
                }
            }
        }
    }
    // Re-apply the new node scale to the windows that take the group's value (§13.21 rule 5;
    // the global factor is 1.0 for plugins, §7.7).
    for (_, key) in &reapply {
        with_ui(sh, |u| {
            let _ = u.window_set_scale(*key, value, false);
        });
    }
    let mut st = sh.st.borrow_mut();
    for id in fire {
        push(
            &mut st,
            Pending::Fire {
                target: id,
                name: "UIScaleChanged",
                args: Args::Scale {
                    index: group,
                    value,
                },
            },
        );
    }
}

/// Build the neutral frame parts of a game-styled window.
pub(crate) fn build_frame(lua: &Lua, sh: &Rc<Shared>, id: u64) {
    let ap = crate::api::apartment_name(lua).unwrap_or_default();
    let Some(key) = sh.key_of(id) else { return };
    let parts = [
        (
            Role::FrameBackground,
            Kind::Control,
            -100,
            [0.85, 0.07, 0.07, 0.09],
            false,
        ),
        (
            Role::FrameTitle,
            Kind::Label,
            -90,
            [1.0, 0.18, 0.18, 0.22],
            true,
        ),
        (
            Role::FrameResize,
            Kind::Control,
            -10,
            [1.0, 0.30, 0.30, 0.34],
            true,
        ),
        (
            Role::FrameClose,
            Kind::Button,
            -5,
            [1.0, 0.45, 0.14, 0.14],
            true,
        ),
    ];
    for (role, kind, z, colour, mouse) in parts {
        let Some((_, pk)) = create_part(sh, &ap, id, Some(key), kind, role) else {
            continue;
        };
        with_ui(sh, |u| {
            u.control_set_z_order(pk, z);
            u.control_set_back_colour(pk, Some(colour));
            u.control_set_mouse_visible(pk, mouse);
            u.control_set_visible(pk, role != Role::FrameResize);
        });
    }
    layout_frame(sh, id);
}

/// Lay out the neutral frame of window `id` (§14.2.3 rectangles, caption width 0).
pub(crate) fn layout_frame(sh: &Shared, id: u64) {
    let Ok(mut ui) = sh.ui.try_borrow_mut() else {
        return;
    };
    let st = sh.st.borrow();
    layout_frame_with(&mut ui, &st, id);
}

/// Retries of one window's title layout while its caption cannot be measured (about 30 s
/// at 60 frames a second; Tessera bound, `lua-plugin-api.md` §14.2.6 implementer OPEN).
const MAX_TITLE_RETRIES: u32 = 1_800;

/// Lays out again the titles of game-styled windows whose caption could not be measured
/// when it was set (its fonts were still loading, §14.2.3 *m*), once a frame, until the
/// measurement succeeds, the window is gone or [`MAX_TITLE_RETRIES`] is reached.
pub(crate) fn retry_titles(sh: &Shared) {
    let pending: Vec<(u64, u32)> = {
        let st = sh.st.borrow();
        let mut p = st.title_pending.borrow_mut();
        if p.is_empty() {
            return;
        }
        std::mem::take(&mut *p).into_iter().collect()
    };
    let Ok(mut ui) = sh.ui.try_borrow_mut() else {
        sh.st.borrow().title_pending.borrow_mut().extend(pending);
        return;
    };
    let st = sh.st.borrow();
    for (id, tries) in pending {
        if tries >= MAX_TITLE_RETRIES {
            tracing::debug!(
                window = id,
                "plugin window caption never measured; title kept"
            );
            continue;
        }
        layout_frame_with(&mut ui, &st, id);
        if let Some(n) = st.title_pending.borrow_mut().get_mut(&id) {
            *n = tries.saturating_add(1);
        }
    }
}

/// [`layout_frame`] with the UI and state already borrowed.
pub(crate) fn layout_frame_with(ui: &mut Ui, st: &State, id: u64) {
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return;
    };
    if c.based {
        let measured = super::styled::title_layout(ui, c);
        super::styled::note_title(st, id, measured);
        return;
    }
    let (w, h) = ui.control_size(c.key);
    let t = (w.saturating_sub(140)).min(120);
    let resizable = c.window.as_deref().is_some_and(|w| w.resizable);
    let title = c
        .window
        .as_deref()
        .map(|w| w.text.clone())
        .unwrap_or_default();
    for pid in &c.parts {
        let Some(ObjData::Ctl(p)) = st.objs.get(pid).map(|o| &o.data) else {
            continue;
        };
        let Some((role, _)) = p.internal else {
            continue;
        };
        let (x, y, pw, ph) = match role {
            Role::FrameBackground => (0, 16, w, h.saturating_sub(18)),
            Role::FrameTitle => {
                let tw = t.saturating_add(80);
                ((w.saturating_sub(tw)) / 2, -8, tw, 41)
            }
            Role::FrameClose => (w.saturating_sub(21), 21, 16, 16),
            Role::FrameResize => (w.saturating_sub(20), h.saturating_sub(20), 20, 20),
            _ => continue,
        };
        ui.control_set_position(p.key, x, y);
        ui.control_set_size(p.key, pw, ph);
        match role {
            Role::FrameTitle => ui.set_text(p.key, Some(title.clone())),
            Role::FrameResize => ui.control_set_visible(p.key, resizable),
            _ => {}
        }
    }
}

/// Install the window classes.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    styled_ns: &Table,
) -> mlua::Result<()> {
    let control = class_table(api.lua(), "Control")?;
    let win = class(
        api,
        sh,
        ui,
        "Window",
        "Window",
        Some(&control),
        control_ctor(Kind::Window, Styled::Plain),
    )?;
    let lwin = class(
        api,
        sh,
        styled_ns,
        "Window",
        "Lotro.Window",
        Some(&win),
        control_ctor(Kind::Window, Styled::Silver),
    )?;
    class(
        api,
        sh,
        styled_ns,
        "GoldWindow",
        "Lotro.GoldWindow",
        Some(&lwin),
        control_ctor(Kind::Window, Styled::Gold),
    )?;

    let w = window;
    def(api, sh, &win, "GetText", w, |cx, _| {
        Ok(ws(cx.sh, cx.id, |w| w.text.clone()).unwrap_or_default())
    })?;
    def(api, sh, &win, "SetText", w, |cx, args| {
        let t = super::text::bound_title(super::text::text_arg(args.front()));
        ws(cx.sh, cx.id, |w| w.text = t);
        layout_frame(cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &win, "Activate", w, |cx, _| {
        if let Some(k) = cx.sh.key_of(cx.id) {
            with_ui(cx.sh, |u| u.activate(k));
        }
        Ok(())
    })?;
    def(api, sh, &win, "Close", w, |cx, _| {
        close_now(cx.lua, cx.sh, cx.id);
        Ok(())
    })?;
    def(api, sh, &win, "GetRotation", w, |cx, _| {
        let r = ctl_get(cx.sh, cx.id, |c| c.key)
            .and_then(|k| with_ui(cx.sh, |u| u.window_rotation(k)))
            .unwrap_or([0.0; 3]);
        let t = cx.lua.create_table()?;
        t.raw_set("x", f64::from(r[0]))?;
        t.raw_set("y", f64::from(r[1]))?;
        t.raw_set("z", f64::from(r[2]))?;
        Ok(t)
    })?;
    def(api, sh, &win, "SetRotation", w, |cx, args| {
        let Some(Value::Table(t)) = args.front() else {
            return Err(script_error(
                "SetRotation: a table with x, y and z is required",
            ));
        };
        let f = |k: &str| num(t.get::<Value>(k).ok().as_ref()).unwrap_or(0.0) as f32;
        let angles = [f("x"), f("y"), f("z")];
        if let Some(k) = cx.sh.key_of(cx.id) {
            with_ui(cx.sh, |u| u.window_set_rotation(k, angles));
        }
        ws(cx.sh, cx.id, |w| w.rotation = angles);
        Ok(())
    })?;
    def(api, sh, &win, "RegisterForGlobalScaling", w, |cx, _| {
        if mark(cx) {
            ws(cx.sh, cx.id, |w| w.global = true);
            apply_scale(cx)?;
        }
        Ok(())
    })?;
    def(api, sh, &win, "UnregisterForGlobalScaling", w, |cx, _| {
        let marked = ws(cx.sh, cx.id, |w| w.marked).unwrap_or(false);
        if marked {
            ws(cx.sh, cx.id, |w| w.global = false);
            apply_scale(cx)?;
        }
        Ok(())
    })?;
    def(api, sh, &win, "SetUIScalingGroup", w, |cx, args| {
        let g = int(args.front()) as u32;
        let plain = ctl_get(cx.sh, cx.id, |c| c.styled == Styled::Plain).unwrap_or(true);
        if mark(cx) {
            let g = if g == 0 || (g == GLOBAL_GROUP && plain) {
                SCALE_GROUP
            } else {
                g
            };
            // §13.21 rule 2: the group's value stays the window's own scale when one was
            // set; only `SetScale` changes the group back.
            ws(cx.sh, cx.id, |w| w.group = g);
            apply_scale(cx)?;
        }
        Ok(())
    })?;
    def(api, sh, &win, "GetUIScalingGroup", w, |cx, _| {
        Ok(f64::from(
            ws(cx.sh, cx.id, |w| if w.marked { w.group } else { 0 }).unwrap_or(0),
        ))
    })?;
    def(api, sh, &win, "SetScale", w, |cx, args| {
        let f = num(args.front()).unwrap_or(0.0) as f32;
        let has_parent = cx
            .sh
            .key_of(cx.id)
            .and_then(|k| with_ui(cx.sh, |u| u.control_parent(k).is_some()))
            .unwrap_or(true);
        if has_parent {
            return Ok(());
        }
        if f.is_nan() || f <= 0.0 || !f.is_finite() {
            return Err(script_error("SetScale: the scale must be greater than 0"));
        }
        mark(cx);
        ws(cx.sh, cx.id, |w| {
            w.own_scale = Some(f);
            w.group = SCALE_GROUP;
        });
        apply_scale(cx)
    })?;
    def(api, sh, &win, "GetScale", w, |cx, _| {
        let Some(w) = ctl_get(cx.sh, cx.id, |c| c.window.as_deref().cloned()).flatten() else {
            return Ok(1.0);
        };
        if !w.marked {
            return Ok(1.0);
        }
        // §13.21 rule 6: the effective scale *s*, the global factor included. With `b` the
        // group's value (rule 2), a global window has *s* = b × `UIScale_Global`, which is
        // 1.0 for plugins (decision 0040, `ui-scaling.md` §7.7), so a global window does not
        // scale twice.
        let b = w.own_scale.unwrap_or_else(|| group_value(cx.sh, w.group));
        let g = if w.global {
            PLUGIN_UI_SCALE_GLOBAL
        } else {
            1.0
        };
        Ok(f64::from(b * g))
    })?;
    def(api, sh, &win, "SetScalingOriginPoint", w, |cx, args| {
        let (x, y) = (
            num(args.front()).unwrap_or(0.0),
            num(args.get(1)).unwrap_or(0.0),
        );
        let marked = ws(cx.sh, cx.id, |w| {
            if w.marked {
                w.origin = (x, y);
            }
            w.marked
        })
        .unwrap_or(false);
        // §13.21 rule 7: the scaler's origin properties (unscaled window pixels), read by
        // the panel-scale rule (`ui-behaviour.md` §9.2 step 4).
        if marked && let Some(k) = cx.sh.key_of(cx.id) {
            let to_int = |v: f64| {
                if v.is_finite() {
                    v.trunc().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
                } else {
                    0
                }
            };
            let (ox, oy) = (to_int(x), to_int(y));
            with_ui(cx.sh, |u| {
                let p = u.vocab().pids;
                u.set_property(k, p.scaler_origin_x, PropertyValue::Int(ox));
                u.set_property(k, p.scaler_origin_y, PropertyValue::Int(oy));
            });
        }
        Ok(())
    })?;
    def(api, sh, &win, "GetScalingOriginPoint", w, |cx, _| {
        Ok(ws(cx.sh, cx.id, |w| w.origin).unwrap_or((0.0, 0.0)))
    })?;

    def(api, sh, &lwin, "IsResizable", styled_window, |cx, _| {
        Ok(ws(cx.sh, cx.id, |w| w.resizable).unwrap_or(false))
    })?;
    def(api, sh, &lwin, "SetResizable", styled_window, |cx, args| {
        let v = truthy(args.front());
        // §14.2.3: nothing happens when the value is the stored one.
        let changed = ws(cx.sh, cx.id, |w| {
            std::mem::replace(&mut w.resizable, v) != v
        })
        .unwrap_or(false);
        let based = ctl_get(cx.sh, cx.id, |c| c.based.then_some(c.key)).flatten();
        match based {
            Some(key) if changed => {
                with_ui(cx.sh, |u| super::styled::set_resize_bars(u, key, v));
            }
            Some(_) => {}
            None => layout_frame(cx.sh, cx.id),
        }
        Ok(())
    })?;
    Ok(())
}
