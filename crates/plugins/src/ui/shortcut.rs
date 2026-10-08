// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.UI.Lotro.Shortcut`, `Quickslot` and the drag-and-drop info classes
//! (`lua-plugin-api.md` §13.10, §14.6; `plugin-api-usage.md` §7.1: an `Alias` shortcut
//! runs its text as a chat command through the host).

use std::rc::Rc;

use mlua::{Lua, MultiValue, Table, Value};

use super::control::{ctl_get, with_ctl};
use super::model::{Args, Kind, ObjData, Styled, bind_new, int, new_object, truthy};
use super::reg::{Cx, class, control_ctor, ctl_of, def};
use super::state::{Pending, Shared, State, class_table, obj_id};
use crate::api::{NativeApi, script_error};

/// `ShortcutType.Alias` (Appendix A.1).
pub(crate) const ALIAS: u32 = 4;

/// Most bytes of shortcut data kept (Tessera bound).
const MAX_DATA: usize = 4096;

/// A shortcut's type and data string (§14.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ShortcutData {
    pub kind: u32,
    pub data: String,
}

/// Store a type: 3 is not a valid shortcut and reads back as 0 (§14.6).
#[must_use]
pub(crate) fn norm_type(t: i32) -> u32 {
    if t == 3 { 0 } else { t as u32 }
}

fn bounded(s: String) -> String {
    if s.len() <= MAX_DATA {
        return s;
    }
    let mut end = MAX_DATA;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

fn shortcut(d: &ObjData) -> bool {
    matches!(d, ObjData::Shortcut(_))
}

fn drag_info(d: &ObjData) -> bool {
    matches!(d, ObjData::DragDrop { .. })
}

fn quickslot(d: &ObjData) -> bool {
    ctl_of(d, &[Kind::Quickslot])
}

fn sc<R>(sh: &Shared, id: u64, f: impl FnOnce(&mut ShortcutData) -> R) -> Option<R> {
    match sh.st.borrow_mut().objs.get_mut(&id).map(|o| &mut o.data) {
        Some(ObjData::Shortcut(s)) => Some(f(s)),
        _ => None,
    }
}

/// A new `Shortcut` object holding `data`.
pub(crate) fn new_shortcut(
    lua: &Lua,
    sh: &Shared,
    ap: &str,
    data: ShortcutData,
) -> mlua::Result<Table> {
    new_object(lua, sh, ap, "Lotro.Shortcut", ObjData::Shortcut(data))
}

/// The shortcut held by `v`, if it is a `Shortcut` object.
pub(crate) fn shortcut_arg(lua: &Lua, sh: &Shared, v: Option<&Value>) -> Option<ShortcutData> {
    let Some(Value::Table(t)) = v else {
        return None;
    };
    let id = obj_id(lua, t)?;
    match sh.st.borrow().objs.get(&id).map(|o| &o.data) {
        Some(ObjData::Shortcut(s)) => Some(s.clone()),
        _ => None,
    }
}

/// A user click on quickslot `id` with mouse button `button` (§14.6): activates its
/// shortcut. An `Alias` queues its text as a chat command; other types need game state.
pub(crate) fn activate(st: &mut State, id: u64, button: u32) {
    let Some(ObjData::Ctl(c)) = st.objs.get(&id).map(|o| &o.data) else {
        return;
    };
    if !c.enabled {
        return;
    }
    let Some(q) = c.quickslot.as_deref() else {
        return;
    };
    if !(button == 1 || (button == 2 && q.use_on_right_click)) {
        return;
    }
    if let Some(s) = &q.shortcut
        && s.kind == ALIAS
        && !s.data.trim().is_empty()
        && st.commands.len() < 256
    {
        let line = s.data.clone();
        st.commands.push(line);
    }
}

/// The shortcut of the drag source `id`, when it is a quickslot.
pub(crate) fn drag_shortcut(st: &State, id: Option<u64>) -> Option<ShortcutData> {
    match st.objs.get(&id?).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.quickslot.as_deref().and_then(|q| q.shortcut.clone()),
        _ => None,
    }
}

/// Install the shortcut, quickslot and drag-and-drop classes.
pub(crate) fn install(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ui: &Table,
    styled_ns: &Table,
    object: &Table,
) -> mlua::Result<()> {
    let s = class(
        api,
        sh,
        styled_ns,
        "Shortcut",
        "Lotro.Shortcut",
        Some(object),
        |lua: &Lua, sh: &Rc<Shared>, ap: &str, this: &Table, cls: &Table, args: MultiValue| {
            let data = match (args.front(), args.get(1)) {
                (Some(Value::Number(_) | Value::Integer(_)), d) => ShortcutData {
                    kind: norm_type(int(args.front())),
                    data: bounded(super::text::text_arg(d)),
                },
                (Some(Value::Table(_)), _) => ShortcutData {
                    kind: 2,
                    data: String::new(),
                },
                _ => ShortcutData::default(),
            };
            bind_new(lua, sh, ap, this, cls, ObjData::Shortcut(data)).map(|_| ())
        },
    )?;
    def(api, sh, &s, "GetType", shortcut, |cx, _| {
        Ok(f64::from(sc(cx.sh, cx.id, |s| s.kind).unwrap_or(0)))
    })?;
    def(api, sh, &s, "SetType", shortcut, |cx, args| {
        let t = norm_type(int(args.front()));
        sc(cx.sh, cx.id, |s| s.kind = t);
        Ok(())
    })?;
    def(api, sh, &s, "GetData", shortcut, |cx, _| {
        Ok(sc(cx.sh, cx.id, |s| s.data.clone()).unwrap_or_default())
    })?;
    def(api, sh, &s, "SetData", shortcut, |cx, args| {
        let d = bounded(super::text::text_arg(args.front()));
        sc(cx.sh, cx.id, |s| {
            if s.kind != 8 && s.kind != 10 {
                s.data = d;
            }
        });
        Ok(())
    })?;
    def(api, sh, &s, "GetItem", shortcut, |_, _| Ok(Value::Nil))?;

    let control = class_table(api.lua(), "Control")?;
    let q = class(
        api,
        sh,
        styled_ns,
        "Quickslot",
        "Lotro.Quickslot",
        Some(&control),
        control_ctor(Kind::Quickslot, Styled::Plain),
    )?;
    def(api, sh, &q, "GetShortcut", quickslot, |cx, _| {
        let s = ctl_get(cx.sh, cx.id, |c| {
            c.quickslot.as_deref().and_then(|q| q.shortcut.clone())
        })
        .flatten();
        match s {
            Some(s) => Ok(Value::Table(new_shortcut(cx.lua, cx.sh, &cx.ap, s)?)),
            None => Ok(Value::Nil),
        }
    })?;
    def(api, sh, &q, "SetShortcut", quickslot, |cx, args| {
        let s = shortcut_arg(cx.lua, cx.sh, args.front())
            .ok_or_else(|| script_error("SetShortcut: invalid shortcut"))?;
        set_shortcut(cx, s);
        Ok(())
    })?;
    def(api, sh, &q, "IsUseOnRightClick", quickslot, |cx, _| {
        Ok(ctl_get(cx.sh, cx.id, |c| {
            c.quickslot.as_deref().is_some_and(|q| q.use_on_right_click)
        })
        .unwrap_or(false))
    })?;
    def(api, sh, &q, "SetUseOnRightClick", quickslot, |cx, args| {
        let v = truthy(args.front());
        with_ctl(cx.sh, cx.id, |c| {
            if let Some(q) = c.quickslot.as_deref_mut() {
                q.use_on_right_click = v;
            }
        });
        Ok(())
    })?;

    let d = class(
        api,
        sh,
        ui,
        "DragDropInfo",
        "DragDropInfo",
        Some(object),
        |lua: &Lua, sh: &Rc<Shared>, ap: &str, this: &Table, cls: &Table, _args: MultiValue| {
            bind_new(
                lua,
                sh,
                ap,
                this,
                cls,
                ObjData::DragDrop {
                    successful: false,
                    shortcut: None,
                },
            )
            .map(|_| ())
        },
    )?;
    def(api, sh, &d, "IsSuccessful", drag_info, |cx, _| {
        Ok(matches!(
            cx.sh.st.borrow().objs.get(&cx.id).map(|o| &o.data),
            Some(ObjData::DragDrop {
                successful: true,
                ..
            })
        ))
    })?;
    def(api, sh, &d, "SetSuccessful", drag_info, |cx, args| {
        let v = truthy(args.front());
        if let Some(ObjData::DragDrop { successful, .. }) = cx
            .sh
            .st
            .borrow_mut()
            .objs
            .get_mut(&cx.id)
            .map(|o| &mut o.data)
        {
            *successful = v;
        }
        Ok(())
    })?;
    let ld = class(
        api,
        sh,
        styled_ns,
        "DragDropInfo",
        "Lotro.DragDropInfo",
        Some(&d),
        |lua: &Lua, sh: &Rc<Shared>, ap: &str, this: &Table, cls: &Table, _args: MultiValue| {
            bind_new(
                lua,
                sh,
                ap,
                this,
                cls,
                ObjData::DragDrop {
                    successful: false,
                    shortcut: None,
                },
            )
            .map(|_| ())
        },
    )?;
    def(api, sh, &ld, "GetShortcut", drag_info, |cx, _| {
        let s = match cx.sh.st.borrow().objs.get(&cx.id).map(|o| &o.data) {
            Some(ObjData::DragDrop { shortcut, .. }) => shortcut.clone(),
            _ => None,
        };
        match s {
            Some(s) => Ok(Value::Table(new_shortcut(cx.lua, cx.sh, &cx.ap, s)?)),
            None => Ok(Value::Nil),
        }
    })?;
    Ok(())
}

fn set_shortcut(cx: &Cx<'_>, s: ShortcutData) {
    let changed = with_ctl(cx.sh, cx.id, |c| {
        c.quickslot
            .as_deref_mut()
            .is_some_and(|q| q.shortcut.replace(s.clone()) != Some(s))
    })
    .unwrap_or(false);
    if changed {
        cx.sh.push(Pending::Fire {
            target: cx.id,
            name: "ShortcutChanged",
            args: Args::Empty,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_three_reads_back_as_zero() {
        assert_eq!(norm_type(3), 0);
        assert_eq!(norm_type(4), 4);
    }

    #[test]
    fn data_is_bounded_at_a_character_boundary() {
        let s = "é".repeat(MAX_DATA);
        let b = bounded(s);
        assert!(b.len() <= MAX_DATA);
        assert!(b.chars().all(|c| c == 'é'));
    }
}
