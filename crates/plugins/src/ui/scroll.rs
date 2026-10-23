// SPDX-License-Identifier: GPL-3.0-or-later
//! `ScrollBar`, `Turbine.UI.Lotro.ScrollBar` and the scroll-bar members of
//! `ScrollableControl` (`lua-plugin-api.md` §9.6 scroll-bar numbers, §13.7, §14.2.2).
//!
//! The layer drives plugin scroll bars itself: it places the replaceable parts, maps the
//! pointer to values and keeps bound bars in step with their scrollable control.

use std::rc::Rc;

use mlua::{Lua, Table, Value};
use tessera_ui::Ui;

use super::control::{ctl_arg, ctl_get, fire_now, set_parent, with_ctl, with_ui};
use super::model::{Args, Kind, ObjData, Role, ScrollNums, Styled, int, num};
use super::reg::{Cx, class, control_ctor, create_part, ctl_of, def};
use super::state::{Pending, Shared, State, class_table, object_of, push};
use crate::api::{NativeApi, script_error};

/// Smallest thumb length along the bar, in pixels (Tessera rule).
const MIN_THUMB: i32 = 8;

/// The 32-bit conversion of the five setters (§9.6): truncate toward zero to 64 bits, keep
/// the low 32 bits; outside the 64-bit range (or NaN) gives 0.
#[must_use]
pub(crate) fn conv(v: f64) -> u32 {
    // 2^63: the first value outside the signed 64-bit range.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if !v.is_finite() || v >= LIMIT || v < -LIMIT {
        return 0;
    }
    (v.trunc() as i64) as u32
}

/// The fraction `num` ÷ `den` of two unsigned differences as a 32-bit float (§9.6, Thumb);
/// 0 when `den` is 0.
fn frac(num: u32, den: u32) -> f32 {
    if den == 0 {
        0.0
    } else {
        num as f32 / den as f32
    }
}

/// `SetValue` on the numbers; counts the `ValueChanged` events it fires. A value past the
/// equality test sets the thumb position fraction (§9.6, Thumb).
pub(crate) fn set_value(n: &mut ScrollNums, v: u32, fires: &mut u32) {
    if v == n.value {
        return;
    }
    let mut v = v;
    if v > n.max {
        v = n.max;
    }
    if v < n.min {
        v = n.min;
    }
    n.value = v;
    n.pos_frac = frac(n.value.wrapping_sub(n.min), n.max.wrapping_sub(n.min));
    *fires += 1;
}

/// A bound changed: the thumb size fraction follows while maximum ≠ minimum (§9.6, Thumb).
fn bound_changed(n: &mut ScrollNums) {
    if n.max != n.min {
        n.size_frac = frac(n.large, n.max.wrapping_sub(n.min));
    }
}

/// `SetMinimum` on the numbers.
pub(crate) fn set_min(n: &mut ScrollNums, m: u32, fires: &mut u32) {
    if m == n.min {
        return;
    }
    n.min = m;
    bound_changed(n);
    if m > n.max {
        set_max(n, m, fires);
    }
    if n.value < n.min {
        let v = n.min;
        set_value(n, v, fires);
    }
}

/// `SetMaximum` on the numbers.
pub(crate) fn set_max(n: &mut ScrollNums, m: u32, fires: &mut u32) {
    if m == n.max {
        return;
    }
    n.max = m;
    bound_changed(n);
    if m < n.min {
        set_min(n, m, fires);
    }
    if n.value > n.max {
        let v = n.max;
        set_value(n, v, fires);
    }
}

/// `SetLargeChange` on the numbers.
pub(crate) fn set_large(n: &mut ScrollNums, l: u32) {
    if l == n.large {
        return;
    }
    let mut l = l;
    let range = n.max.wrapping_sub(n.min);
    if l > range {
        l = range;
    }
    if l < n.small {
        l = n.small;
    }
    n.large = l;
}

/// `SetSmallChange` on the numbers.
pub(crate) fn set_small(n: &mut ScrollNums, s: u32) {
    if s == n.small {
        return;
    }
    n.small = s.min(n.large);
}

fn bar(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::ScrollBar])
}

fn scrollable(d: &ObjData) -> bool {
    matches!(d, ObjData::Ctl(c) if c.internal.is_none() && c.kind.scrollable())
}

fn nums(sh: &Shared, id: u64) -> Option<ScrollNums> {
    ctl_get(sh, id, |c| c.scroll.as_deref().map(|s| s.nums)).flatten()
}

fn bound(sh: &Shared, id: u64) -> bool {
    ctl_get(sh, id, |c| c.scroll.as_deref().and_then(|s| s.bound))
        .flatten()
        .is_some()
}

fn guard(cx: &Cx<'_>, what: &str) -> mlua::Result<()> {
    if bound(cx.sh, cx.id) {
        return Err(script_error(format!(
            "{what}: the scroll bar is bound to a scrollable control"
        )));
    }
    Ok(())
}

/// Change the numbers with `f`, relayout the parts and fire `ValueChanged` directly once
/// per value change (§9.6).
fn change(cx: &Cx<'_>, f: impl FnOnce(&mut ScrollNums, &mut u32)) {
    let mut fires = 0;
    with_ctl(cx.sh, cx.id, |c| {
        if let Some(s) = c.scroll.as_deref_mut() {
            f(&mut s.nums, &mut fires);
        }
    });
    layout_parts(cx.sh, cx.id);
    for _ in 0..fires {
        fire_now(cx.lua, cx.sh, cx.id, "ValueChanged", &Args::Empty);
    }
}

fn get_part(cx: &Cx<'_>, which: u8) -> Option<Table> {
    let p = ctl_get(cx.sh, cx.id, |c| {
        c.scroll.as_deref().and_then(|s| match which {
            0 => s.decrement,
            1 => s.increment,
            _ => s.thumb,
        })
    })
    .flatten()?;
    object_of(cx.lua, p)
}

fn set_part(cx: &Cx<'_>, which: u8, v: Option<&Value>) -> mlua::Result<()> {
    let new = match v {
        None | Some(Value::Nil) => None,
        v => Some(ctl_arg(cx.lua, cx.sh, v).ok_or_else(|| script_error("a control is required"))?),
    };
    let old = with_ctl(cx.sh, cx.id, |c| {
        c.scroll.as_deref_mut().and_then(|s| {
            let slot = match which {
                0 => &mut s.decrement,
                1 => &mut s.increment,
                _ => &mut s.thumb,
            };
            std::mem::replace(slot, new)
        })
    })
    .flatten();
    if let Some(o) = old
        && Some(o) != new
    {
        with_ctl(cx.sh, o, |c| {
            if c.part_of == Some((cx.id, which)) {
                c.part_of = None;
            }
        });
        let _ = set_parent(cx.lua, cx.sh, o, None);
    }
    if let Some(n) = new {
        set_parent(cx.lua, cx.sh, n, Some(cx.id))?;
        let bar = cx.id;
        with_ctl(cx.sh, n, |c| c.part_of = Some((bar, which)));
    }
    // A plugin thumb replaces the neutral one.
    if which == 2 {
        let neutral = neutral_thumb(&cx.sh.st.borrow(), cx.id);
        if let Some(k) = neutral {
            with_ui(cx.sh, |u| u.control_set_visible(k, new.is_none()));
        }
    }
    layout_parts(cx.sh, cx.id);
    Ok(())
}

fn neutral_thumb(st: &State, id: u64) -> Option<tessera_ui::ElementKey> {
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return None;
    };
    c.parts
        .iter()
        .find_map(|p| match st.objs.get(p).map(|o| &o.data) {
            Some(ObjData::Ctl(pc)) if pc.internal.map(|i| i.0) == Some(Role::Thumb) => Some(pc.key),
            _ => None,
        })
}

/// The neutral thumb of a game-styled scroll bar (§14.2.6).
pub(crate) fn build_neutral_thumb(lua: &Lua, sh: &Rc<Shared>, id: u64) {
    let ap = crate::api::apartment_name(lua).unwrap_or_default();
    let Some(key) = sh.key_of(id) else { return };
    if let Some((_, pk)) = create_part(sh, &ap, id, Some(key), Kind::Control, Role::Thumb) {
        with_ui(sh, |u| {
            u.control_set_back_colour(pk, Some([1.0, 0.55, 0.55, 0.6]));
            u.control_set_visible(pk, true);
        });
    }
    layout_parts(sh, id);
}

/// Place the parts of scroll bar `id`.
pub(crate) fn layout_parts(sh: &Shared, id: u64) {
    let Ok(mut ui) = sh.ui.try_borrow_mut() else {
        return;
    };
    let st = sh.st.borrow();
    layout_parts_with(&mut ui, &st, id);
}

/// Geometry of a bar: (vertical, length, thickness, track start, track length, thumb
/// length, thumb start).
pub(crate) fn geometry(
    ui: &Ui,
    st: &State,
    id: u64,
) -> Option<(bool, i32, i32, i32, i32, i32, i32)> {
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return None;
    };
    let s = c.scroll.as_deref()?;
    let vertical = s.orientation == 1;
    let (w, h) = ui.control_size(c.key);
    let (len, thick) = if vertical { (h, w) } else { (w, h) };
    let along = |pid: Option<u64>| -> i32 {
        pid.and_then(|p| match st.objs.get(&p).map(|o| &o.data) {
            Some(ObjData::Ctl(pc)) => {
                let (pw, ph) = ui.control_size(pc.key);
                Some(if vertical { ph } else { pw })
            }
            _ => None,
        })
        .unwrap_or(0)
        .max(0)
    };
    let dec = along(s.decrement);
    let inc = along(s.increment);
    let track_start = dec;
    let track = len.saturating_sub(dec).saturating_sub(inc).max(0);
    // The two stored fractions of §9.6 (Thumb), not the current numbers: a range change
    // that does not move the value leaves the position, `SetLargeChange` leaves the size.
    let n = s.nums;
    let f = if n.size_frac.is_finite() {
        n.size_frac.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let thumb = ((track as f32 * f) as i32).clamp(MIN_THUMB.min(track), track);
    let pos_frac = if n.pos_frac.is_finite() {
        n.pos_frac.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_start =
        track_start.saturating_add(((track.saturating_sub(thumb)).max(0) as f32 * pos_frac) as i32);
    Some((vertical, len, thick, track_start, track, thumb, thumb_start))
}

/// [`layout_parts`] with the UI and state borrowed. A game-styled bar shows its numbers
/// on its layout widget; parts the plugin set are placed by the layer in both cases.
pub(crate) fn layout_parts_with(ui: &mut Ui, st: &State, id: u64) {
    if let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) {
        super::styled::sync_scroll_part(ui, c);
    }
    let Some((vertical, len, thick, _ts, _track, thumb, thumb_start)) = geometry(ui, st, id) else {
        return;
    };
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return;
    };
    let Some(s) = c.scroll.as_deref() else { return };
    let key_of = |p: Option<u64>| match p.and_then(|p| st.objs.get(&p)).map(|o| &o.data) {
        Some(ObjData::Ctl(pc)) => Some(pc.key),
        _ => None,
    };
    if let Some(k) = key_of(s.decrement) {
        ui.control_set_position(k, 0, 0);
    }
    if let Some(k) = key_of(s.increment) {
        let (pw, ph) = ui.control_size(k);
        if vertical {
            ui.control_set_position(k, 0, len.saturating_sub(ph));
        } else {
            ui.control_set_position(k, len.saturating_sub(pw), 0);
        }
    }
    let thumb_key = key_of(s.thumb).or_else(|| neutral_thumb(st, id));
    if let Some(k) = thumb_key {
        if vertical {
            ui.control_set_position(k, 0, thumb_start);
            ui.control_set_size(k, thick, thumb);
        } else {
            ui.control_set_position(k, thumb_start, 0);
            ui.control_set_size(k, thumb, thick);
        }
    }
}

/// A value change made by the user (pointer, wheel, buttons): stored, the parts placed,
/// `ValueChanged` queued, and a bound scrollable scrolled.
pub(crate) fn user_set_value(ui: &mut Ui, st: &mut State, id: u64, v: u32) {
    let mut fires = 0;
    let mut target = None;
    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&id).map(|o| &mut o.data)
        && let Some(s) = c.scroll.as_deref_mut()
    {
        set_value(&mut s.nums, v, &mut fires);
        target = s.bound.map(|b| (b, s.orientation == 1, s.nums.value));
    }
    layout_parts_with(ui, st, id);
    if fires > 0 && target.is_none() {
        push(
            st,
            Pending::Fire {
                target: id,
                name: "ValueChanged",
                args: Args::Empty,
            },
        );
    }
    if let Some((b, vertical, value)) = target {
        let off = i32::try_from(value).unwrap_or(i32::MAX);
        if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&b).map(|o| &mut o.data) {
            if vertical {
                c.offset.1 = off;
            } else {
                c.offset.0 = off;
            }
        }
        super::lists::mark(st, b);
    }
}

/// Step the value of bar `id` by `delta` units (negative: toward the minimum).
pub(crate) fn step(ui: &mut Ui, st: &mut State, id: u64, delta: i64) {
    let Some(n) = (match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().map(|s| s.nums),
        _ => None,
    }) else {
        return;
    };
    let v = (i64::from(n.value) + delta).clamp(i64::from(n.min), i64::from(n.max.max(n.min)));
    user_set_value(ui, st, id, v as u32);
}

/// A press at `p` pixels along bar `id` (not on its thumb): page toward the pointer.
pub(crate) fn page_toward(ui: &mut Ui, st: &mut State, id: u64, p: i32) {
    let Some((_, _, _, _, _, _thumb, thumb_start)) = geometry(ui, st, id) else {
        return;
    };
    let large = match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.scroll.as_deref().map_or(0, |s| s.nums.large),
        _ => 0,
    };
    let delta = if p < thumb_start {
        -i64::from(large)
    } else {
        i64::from(large)
    };
    step(ui, st, id, delta);
}

/// Drag the thumb of bar `id` so that the pointer at `p` pixels along the bar keeps the
/// grab offset.
pub(crate) fn drag_to(ui: &mut Ui, st: &mut State, id: u64, p: i32) {
    let Some((_, _, _, track_start, track, thumb, _)) = geometry(ui, st, id) else {
        return;
    };
    let (offset, n) = match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => match c.scroll.as_deref() {
            Some(s) => (s.drag, s.nums),
            None => return,
        },
        _ => return,
    };
    let Some(offset) = offset else { return };
    let free = track.saturating_sub(thumb).max(1);
    let pos = p
        .saturating_sub(offset)
        .saturating_sub(track_start)
        .clamp(0, free);
    let range = n.max.wrapping_sub(n.min);
    let v = n
        .min
        .wrapping_add(((f64::from(pos) / f64::from(free)) * f64::from(range)).round() as u32);
    user_set_value(ui, st, id, v);
}

/// Bring the bars bound to scrollable `id` in step with its content and offset.
pub(crate) fn sync_bars(ui: &mut Ui, st: &mut State, id: u64) {
    let Some((key, content, offset, vbar, hbar)) = (match st.objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => {
            let content = c
                .list
                .as_deref()
                .map(|l| l.content)
                .or_else(|| c.tree.as_deref().map(|t| t.content))
                .unwrap_or((0, 0));
            Some((c.key, content, c.offset, c.vbar, c.hbar))
        }
        _ => None,
    }) else {
        return;
    };
    let (w, h) = ui.control_size(key);
    for (bar, view, total, off) in [
        (vbar, h, content.1, offset.1),
        (hbar, w, content.0, offset.0),
    ] {
        let Some(b) = bar else { continue };
        if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&b).map(|o| &mut o.data)
            && let Some(s) = c.scroll.as_deref_mut()
        {
            let max = total.saturating_sub(view).max(0) as u32;
            s.nums.min = 0;
            s.nums.max = max;
            s.nums.large = view.max(0) as u32;
            s.nums.small = s.nums.small.min(s.nums.large).max(1);
            s.nums.value = (off.max(0) as u32).min(max);
            // A bound bar follows its control's content (no plugin setter reaches it).
            s.nums.pos_frac = frac(s.nums.value, max);
            s.nums.size_frac = if max == 0 {
                1.0
            } else {
                frac(s.nums.large, max)
            };
        }
        layout_parts_with(ui, st, b);
    }
}

/// Install the scroll-bar classes and the `ScrollableControl` members.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    styled_ns: &Table,
) -> mlua::Result<()> {
    let lua = api.lua();
    let control = class_table(lua, "Control")?;
    let sc = class_table(lua, "ScrollableControl")?;
    let sb = class(
        api,
        sh,
        ui,
        "ScrollBar",
        "ScrollBar",
        Some(&control),
        control_ctor(Kind::ScrollBar, Styled::Plain),
    )?;
    class(
        api,
        sh,
        styled_ns,
        "ScrollBar",
        "Lotro.ScrollBar",
        Some(&sb),
        control_ctor(Kind::ScrollBar, Styled::Silver),
    )?;

    let b = bar;
    def(api, sh, &sb, "GetMinimum", b, |cx, _| {
        guard(cx, "GetMinimum")?;
        Ok(f64::from(nums(cx.sh, cx.id).unwrap_or_default().min))
    })?;
    def(api, sh, &sb, "GetMaximum", b, |cx, _| {
        guard(cx, "GetMaximum")?;
        Ok(f64::from(nums(cx.sh, cx.id).unwrap_or_default().max))
    })?;
    def(api, sh, &sb, "GetValue", b, |cx, _| {
        guard(cx, "GetValue")?;
        Ok(f64::from(nums(cx.sh, cx.id).unwrap_or_default().value))
    })?;
    def(api, sh, &sb, "GetLargeChange", b, |cx, _| {
        guard(cx, "GetLargeChange")?;
        Ok(f64::from(nums(cx.sh, cx.id).unwrap_or_default().large))
    })?;
    def(api, sh, &sb, "GetSmallChange", b, |cx, _| {
        Ok(f64::from(nums(cx.sh, cx.id).unwrap_or_default().small))
    })?;
    def(api, sh, &sb, "SetMinimum", b, |cx, args| {
        guard(cx, "SetMinimum")?;
        let m = conv(num(args.front()).unwrap_or(0.0));
        change(cx, |n, f| set_min(n, m, f));
        Ok(())
    })?;
    def(api, sh, &sb, "SetMaximum", b, |cx, args| {
        guard(cx, "SetMaximum")?;
        let m = conv(num(args.front()).unwrap_or(0.0));
        change(cx, |n, f| set_max(n, m, f));
        Ok(())
    })?;
    def(api, sh, &sb, "SetValue", b, |cx, args| {
        guard(cx, "SetValue")?;
        let v = conv(num(args.front()).unwrap_or(0.0));
        change(cx, |n, f| set_value(n, v, f));
        Ok(())
    })?;
    def(api, sh, &sb, "SetLargeChange", b, |cx, args| {
        guard(cx, "SetLargeChange")?;
        let v = conv(num(args.front()).unwrap_or(0.0));
        change(cx, |n, _| set_large(n, v));
        Ok(())
    })?;
    def(api, sh, &sb, "SetSmallChange", b, |cx, args| {
        guard(cx, "SetSmallChange")?;
        let v = conv(num(args.front()).unwrap_or(0.0));
        change(cx, |n, _| set_small(n, v));
        Ok(())
    })?;
    def(api, sh, &sb, "GetOrientation", b, |cx, _| {
        Ok(f64::from(
            ctl_get(cx.sh, cx.id, |c| {
                c.scroll.as_deref().map_or(0, |s| s.orientation)
            })
            .unwrap_or(0),
        ))
    })?;
    def(api, sh, &sb, "SetOrientation", b, |cx, args| {
        let o = int(args.front()) as u32;
        let changed = with_ctl(cx.sh, cx.id, |c| {
            c.scroll
                .as_deref_mut()
                .is_some_and(|s| std::mem::replace(&mut s.orientation, o) != o)
        })
        .unwrap_or(false);
        if changed {
            // §14.2.1 step 3: a game-styled bar is rebuilt for the new orientation.
            let widget = ctl_get(cx.sh, cx.id, |c| {
                c.scroll.as_deref().is_some_and(|s| s.widget.is_some())
            })
            .unwrap_or(false);
            if widget {
                if let Ok(mut ui) = cx.sh.ui.try_borrow_mut() {
                    let mut st = cx.sh.st.borrow_mut();
                    super::styled::rebuild_scroll_part(&mut ui, &mut st, cx.id);
                }
                let lost = ctl_get(cx.sh, cx.id, |c| {
                    c.scroll.as_deref().is_some_and(|s| s.widget.is_none())
                })
                .unwrap_or(false);
                if lost {
                    // The layout could not build the new orientation: neutral art (step 5).
                    build_neutral_thumb(cx.lua, cx.sh, cx.id);
                }
            }
        }
        layout_parts(cx.sh, cx.id);
        if changed {
            cx.sh.push(Pending::Fire {
                target: cx.id,
                name: "OrientationChanged",
                args: Args::Empty,
            });
        }
        Ok(())
    })?;
    for (which, get, set) in [
        (0u8, "GetDecrementButton", "SetDecrementButton"),
        (1, "GetIncrementButton", "SetIncrementButton"),
        (2, "GetThumbButton", "SetThumbButton"),
    ] {
        def(api, sh, &sb, get, b, move |cx, _| Ok(get_part(cx, which)))?;
        def(api, sh, &sb, set, b, move |cx, args| {
            set_part(cx, which, args.front())
        })?;
    }

    // ScrollableControl (§13.7).
    for (vertical, get, set) in [
        (false, "GetHorizontalScrollBar", "SetHorizontalScrollBar"),
        (true, "GetVerticalScrollBar", "SetVerticalScrollBar"),
    ] {
        def(api, sh, &sc, get, scrollable, move |cx, _| {
            let b = ctl_get(cx.sh, cx.id, |c| if vertical { c.vbar } else { c.hbar }).flatten();
            Ok(b.and_then(|b| object_of(cx.lua, b)))
        })?;
        def(api, sh, &sc, set, scrollable, move |cx, args| {
            let new = match args.front() {
                None | Some(Value::Nil) => None,
                v => {
                    let id = ctl_arg(cx.lua, cx.sh, v)
                        .filter(|b| cx.sh.kind_of(*b) == Some(Kind::ScrollBar))
                        .ok_or_else(|| script_error("a ScrollBar is required"))?;
                    Some(id)
                }
            };
            let old = with_ctl(cx.sh, cx.id, |c| {
                let slot = if vertical { &mut c.vbar } else { &mut c.hbar };
                std::mem::replace(slot, new)
            })
            .flatten();
            if let Some(o) = old {
                with_ctl(cx.sh, o, |c| {
                    if let Some(s) = c.scroll.as_deref_mut() {
                        s.bound = None;
                    }
                });
            }
            if let Some(n) = new {
                let this = cx.id;
                with_ctl(cx.sh, n, |c| {
                    if let Some(s) = c.scroll.as_deref_mut() {
                        s.bound = Some(this);
                    }
                });
                super::state::anchor(cx.lua, n, None, Some(this));
            }
            if let Ok(mut ui) = cx.sh.ui.try_borrow_mut() {
                let mut st = cx.sh.st.borrow_mut();
                sync_bars(&mut ui, &mut st, cx.id);
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example_of_the_spec() {
        let mut n = ScrollNums::default();
        let mut f = 0;
        set_min(&mut n, conv(-10.0), &mut f);
        assert_eq!(
            (n.min, n.max, n.value, f),
            (4_294_967_286, 4_294_967_286, 4_294_967_286, 1)
        );
        set_max(&mut n, conv(10.0), &mut f);
        assert_eq!((n.min, n.max, n.value, f), (10, 10, 10, 2));

        let mut n = ScrollNums::default();
        let mut f = 0;
        set_min(&mut n, 5, &mut f);
        set_max(&mut n, 10, &mut f);
        set_value(&mut n, conv(20.0), &mut f);
        assert_eq!(n.value, 10);
        n.value = 7;
        set_value(&mut n, conv(-1.0), &mut f);
        assert_eq!(n.value, 10);
        set_value(&mut n, conv(2.0), &mut f);
        assert_eq!(n.value, 5);
    }

    #[test]
    fn conversion() {
        assert_eq!(conv(-5.0), 4_294_967_291);
        assert_eq!(conv(4_294_967_291.0), 4_294_967_291);
        assert_eq!(conv(f64::NAN), 0);
        assert_eq!(conv(1e30), 0);
        assert_eq!(conv(2.9), 2);
    }

    #[test]
    fn change_rules() {
        let mut n = ScrollNums::default();
        set_large(&mut n, 500);
        assert_eq!(n.large, 100);
        set_small(&mut n, 1000);
        assert_eq!(n.small, 100);
        set_large(&mut n, 50);
        assert_eq!(
            n.large, 100,
            "below the small change it becomes the small change"
        );
    }
}
