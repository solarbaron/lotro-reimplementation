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

/// `ShortcutType.Item` (Appendix A.1).
pub(crate) const ITEM: u32 = 2;

/// `ShortcutType.Skill` (Appendix A.1).
pub(crate) const SKILL: u32 = 6;

/// Most bytes of shortcut data kept (Tessera bound).
const MAX_DATA: usize = 4096;

/// Most game actions of activated quickslots waiting for the client.
const MAX_GAME_ACTIONS: usize = 256;

/// The content of a `Skill` or `Item` shortcut as the game uses it (`lua-plugin-api.md`
/// §15.2 "Quickslot shortcuts"): what a plugin quickslot's activation uses, what a drag
/// from the game's quickslot bar carries, and what a plugin shortcut dropped on that bar
/// sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameShortcut {
    /// A skill record (`UseSkill { what: Ref::Record(did), target }`).
    Skill(u32),
    /// An item instance (`UseItem { item, target }`).
    Item(u64),
}

/// `0x` followed by exactly `n` hex digits (either case), as a number.
fn hex_field(s: &str, n: usize) -> Option<u64> {
    let digits = s.strip_prefix("0x")?;
    if digits.len() != n || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(digits, 16).ok()
}

impl GameShortcut {
    /// Parse a shortcut of type `kind` with data `data`: a `Skill` (6) whose data is `0x`
    /// and 8 hex digits; an `Item` (2) whose data is `0x`, 16 hex digits, `,0x` and 8 hex
    /// digits. Any other type or data gives `None`.
    #[must_use]
    pub fn parse(kind: u32, data: &str) -> Option<Self> {
        match kind {
            SKILL => hex_field(data, 8).map(|d| Self::Skill(d as u32)),
            ITEM => {
                let (id, companion) = data.split_once(',')?;
                hex_field(companion, 8)?;
                hex_field(id, 16).map(Self::Item)
            }
            _ => None,
        }
    }

    /// The shortcut type and data string a plugin sees (`Lotro.DragDropInfo:GetShortcut()`
    /// of a drag from the game's bar): upper-case hex digits; an item's companion value
    /// is written as 0 (§14.6 OPEN).
    #[must_use]
    pub fn to_type_and_data(self) -> (u32, String) {
        match self {
            Self::Skill(did) => (SKILL, format!("0x{did:08X}")),
            Self::Item(id) => (ITEM, format!("0x{id:016X},0x00000000")),
        }
    }
}

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
/// shortcut. An `Alias` queues its text as a chat command; a `Skill` or `Item` shortcut
/// with well-formed data queues a game action for the client (§15.2); other types and
/// data do nothing.
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
    let Some(s) = &q.shortcut else {
        return;
    };
    if s.kind == ALIAS && !s.data.trim().is_empty() && st.commands.len() < 256 {
        let line = s.data.clone();
        st.commands.push(line);
    } else if let Some(action) = GameShortcut::parse(s.kind, &s.data)
        && st.game_actions.len() < MAX_GAME_ACTIONS
    {
        st.game_actions.push(action);
    }
}

/// The shortcut of the drag source `id`, when it is a plugin quickslot; otherwise the
/// content of the game's quickslot-bar slot the drag started on, when the client set one.
pub(crate) fn drag_shortcut(st: &State, id: Option<u64>) -> Option<ShortcutData> {
    match id.and_then(|id| st.objs.get(&id)).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => c.quickslot.as_deref().and_then(|q| q.shortcut.clone()),
        Some(_) => None,
        None => st.game_drag.clone(),
    }
}

/// The game content of the current drag source's shortcut, when the source is a plugin
/// quickslot holding a well-formed `Skill` or `Item` shortcut.
pub(crate) fn drag_source_game_shortcut(st: &State) -> Option<GameShortcut> {
    match st.objs.get(&st.drag_source?).map(|o| &o.data) {
        Some(ObjData::Ctl(c)) => {
            let s = c.quickslot.as_deref()?.shortcut.as_ref()?;
            GameShortcut::parse(s.kind, &s.data)
        }
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
    // An `Item` shortcut gives the item's `Item` object while the player's containers
    // hold it, else an empty `Item` view (§15.2; `plugin-api-usage.md` §7.4.1 rule 3).
    def(api, sh, &s, "GetItem", shortcut, |cx, _| {
        let Some(s) = sc(cx.sh, cx.id, |s| s.clone()) else {
            return Ok(Value::Nil);
        };
        if s.kind != ITEM {
            return Ok(Value::Nil);
        }
        let id = match GameShortcut::parse(s.kind, &s.data) {
            Some(GameShortcut::Item(id)) => Some(id),
            _ => None,
        };
        Ok(Value::Table(crate::inventory::shortcut_item(cx.lua, id)?))
    })?;

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
    fn game_shortcut_data() {
        assert_eq!(
            GameShortcut::parse(SKILL, "0x7000aBcD"),
            Some(GameShortcut::Skill(0x7000_ABCD))
        );
        assert_eq!(
            GameShortcut::parse(ITEM, "0x00000000000000FF,0x00000000"),
            Some(GameShortcut::Item(255))
        );
        for (kind, bad) in [
            (SKILL, "0xZZ"),
            (SKILL, "0x7000ABC"),
            (SKILL, "0X7000ABCD"),
            (SKILL, "0x+000ABCD"),
            (ITEM, "0x00000000000000FF"),
            (ITEM, "0x00000000000000FF,0x0"),
            (ALIAS, "0x7000ABCD"),
        ] {
            assert_eq!(GameShortcut::parse(kind, bad), None, "{bad}");
        }
        for g in [
            GameShortcut::Skill(0x7000_00AB),
            GameShortcut::Item(u64::MAX),
        ] {
            let (kind, data) = g.to_type_and_data();
            assert_eq!(GameShortcut::parse(kind, &data), Some(g));
        }
        assert_eq!(GameShortcut::Skill(0xAB).to_type_and_data().1, "0x000000AB");
    }

    #[test]
    fn a_drag_from_the_game_bar_carries_its_content() {
        let mut st = State::default();
        assert_eq!(drag_shortcut(&st, None), None);
        st.game_drag = Some(ShortcutData {
            kind: SKILL,
            data: "0x000000AB".into(),
        });
        assert_eq!(
            drag_shortcut(&st, None).map(|s| (s.kind, s.data)),
            Some((SKILL, "0x000000AB".to_owned()))
        );
        // A source the layer does not know (a game element) is no plugin quickslot.
        assert!(drag_shortcut(&st, Some(42)).is_some());
        assert_eq!(drag_source_game_shortcut(&st), None);
    }

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
