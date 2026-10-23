// SPDX-License-Identifier: GPL-3.0-or-later
//! Installation of the `Turbine.UI` and `Turbine.UI.Lotro` tables into an apartment, and
//! the helpers every class module uses to define its native methods
//! (`lua-plugin-api.md` §8, §13, §14, §16).

use std::rc::Rc;

use mlua::{IntoLuaMulti, Lua, MultiValue, Table, Value};

use super::enums;
use super::model::{Background, Ctl, Kind, Obj, ObjData, Styled, bind_new, int, num, truthy};
use super::state::{CLASS_KEY, Shared, install_registries, obj_id, shared};
use crate::api::{NativeApi, apartment_name, script_error};

/// The context of one native method call.
pub(crate) struct Cx<'a> {
    pub lua: &'a Lua,
    pub sh: &'a Rc<Shared>,
    pub this: Table,
    pub id: u64,
    pub ap: String,
}

/// Which records a method accepts as `self`.
pub(crate) type Accept = fn(&ObjData) -> bool;

/// Any control.
pub(crate) fn any_ctl(d: &ObjData) -> bool {
    matches!(d, ObjData::Ctl(c) if c.internal.is_none())
}

/// A control of one of `kinds`.
pub(crate) fn ctl_of(d: &ObjData, kinds: &[Kind]) -> bool {
    matches!(d, ObjData::Ctl(c) if c.internal.is_none() && kinds.contains(&c.kind))
}

/// Methods that neither read geometry nor depend on a list layout: they run without first
/// laying out the list boxes and trees marked dirty, so that filling a list in one call
/// costs one layout, not one per item (`lists::flush`). Every other method lays them out
/// first.
fn skips_layout_flush(name: &str) -> bool {
    matches!(
        name,
        "AddItem"
            | "InsertItem"
            | "RemoveItem"
            | "RemoveItemAt"
            | "ClearItems"
            | "SetItem"
            | "GetItem"
            | "GetItemCount"
            | "ContainsItem"
            | "IndexOfItem"
            | "Add"
            | "GetCount"
            | "SetSize"
            | "SetWidth"
            | "SetHeight"
            | "SetPosition"
            | "SetLeft"
            | "SetTop"
            | "SetVisible"
            | "IsVisible"
            | "SetParent"
            | "SetText"
            | "GetText"
            | "AppendText"
            | "SetBackground"
            | "SetBackColor"
            | "SetBackColorBlendMode"
            | "SetBlendMode"
            | "SetStretchMode"
            | "SetOpacity"
            | "SetFont"
            | "SetFontStyle"
            | "SetForeColor"
            | "SetOutlineColor"
            | "SetTextAlignment"
            | "SetMultiline"
            | "SetMarkupEnabled"
            | "SetSelectable"
            | "SetMouseVisible"
            | "SetZOrder"
            | "SetEnabled"
            | "IsEnabled"
            | "SetWantsUpdates"
            | "SetWantsKeyEvents"
            | "SetAllowDrop"
            | "SetChecked"
            | "Expand"
            | "Collapse"
            | "SetExpanded"
    )
}

/// Define the native method `cls[name]`: `self` must be an object whose record `accept`
/// takes; after the call the events it caused are collected and, when no delivery is
/// running, delivered (§9.7).
pub(crate) fn def<R, F>(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    cls: &Table,
    name: &'static str,
    accept: Accept,
    f: F,
) -> mlua::Result<()>
where
    R: IntoLuaMulti,
    F: Fn(&Cx<'_>, MultiValue) -> mlua::Result<R> + Send + 'static,
{
    let b = sh.id;
    let flush = !skips_layout_flush(name);
    let func = api.function(name, move |lua, mut args: MultiValue| {
        let sh = shared(b)?;
        let Some(Value::Table(this)) = args.pop_front() else {
            return Err(script_error(format!(
                "{name}: expected a user-interface object as self"
            )));
        };
        let Some(id) = obj_id(lua, &this) else {
            return Err(script_error(format!(
                "{name}: expected a user-interface object as self"
            )));
        };
        let ap = apartment_name(lua).unwrap_or_default();
        sh.reap(lua, &ap);
        let ok = sh.st.borrow().objs.get(&id).map(|o| accept(&o.data));
        match ok {
            Some(true) => {}
            Some(false) => {
                return Err(script_error(format!(
                    "{name}: not supported by this object"
                )));
            }
            None => return Err(script_error(format!("{name}: the object no longer exists"))),
        }
        if flush {
            super::lists::flush(&sh);
        }
        let cx = Cx {
            lua,
            sh: &sh,
            this,
            id,
            ap: ap.clone(),
        };
        let r = f(&cx, args).and_then(|r| r.into_lua_multi(lua));
        sh.after_call(lua, &ap);
        r
    })?;
    cls.raw_set(name, func)
}

/// Define a static function `owner[name]` that may touch the UI.
pub(crate) fn def_static<R, F>(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    owner: &Table,
    name: &'static str,
    f: F,
) -> mlua::Result<()>
where
    R: IntoLuaMulti,
    F: Fn(&Lua, &Rc<Shared>, MultiValue) -> mlua::Result<R> + Send + 'static,
{
    let b = sh.id;
    api.static_function(owner, name, move |lua, args: MultiValue| {
        let sh = shared(b)?;
        let ap = apartment_name(lua);
        if let Some(ap) = &ap {
            sh.reap(lua, ap);
        }
        let r = f(lua, &sh, args).and_then(|r| r.into_lua_multi(lua));
        if let Some(ap) = &ap {
            sh.after_call(lua, ap);
        }
        r
    })?;
    Ok(())
}

/// Define a method that returns a fixed default and ignores its arguments: a getter gives
/// `ret` whatever was set, a setter does nothing (members of tier-2 classes that need game
/// data, §17.1; listed in the compatibility table of `tests/compat_plugins.rs`).
pub(crate) fn def_stub(
    api: &NativeApi<'_>,
    cls: &Table,
    name: &'static str,
    ret: Value,
) -> mlua::Result<()> {
    let ret = match ret {
        Value::Boolean(b) => Some(Value::Boolean(b)),
        Value::Number(n) => Some(Value::Number(n)),
        _ => None,
    };
    let f = api.function(name, move |_, _args: MultiValue| {
        Ok(ret.clone().unwrap_or(Value::Nil))
    })?;
    cls.raw_set(name, f)
}

/// The instantiable class `ns[name]` with base `base`: `Constructor(self, ...)` runs
/// `ctor` and calling the class creates an instance (§8.1–§8.3).
pub(crate) fn class<F>(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    ns: &Table,
    name: &str,
    key: &str,
    base: Option<&Table>,
    ctor: F,
) -> mlua::Result<Table>
where
    F: Fn(&Lua, &Rc<Shared>, &str, &Table, &Table, MultiValue) -> mlua::Result<()> + Send + 'static,
{
    let cls = api.class(ns, name, base)?;
    api.lua()
        .set_named_registry_value(&format!("{CLASS_KEY}{key}"), cls.clone())?;
    {
        let b = sh.id;
        let cls_ref = cls.clone();
        let label = format!("{name}.Constructor");
        let f = api.function("Constructor", move |lua, mut args: MultiValue| {
            let sh = shared(b)?;
            let Some(Value::Table(this)) = args.pop_front() else {
                return Err(script_error(format!(
                    "{label}: expected an object table as first argument"
                )));
            };
            let ap = apartment_name(lua).unwrap_or_default();
            sh.reap(lua, &ap);
            let r = ctor(lua, &sh, &ap, &this, &cls_ref, args);
            sh.after_call(lua, &ap);
            r
        })?;
        cls.raw_set("Constructor", f)?;
        let inst = instantiate(api)?;
        if let Some(mt) = cls.metatable() {
            mt.raw_set("__call", inst)?;
        }
    }
    Ok(cls)
}

/// A class table `ns[name]` without a constructor (objects the layer returns, §8.1).
pub(crate) fn class_plain(
    api: &NativeApi<'_>,
    ns: &Table,
    name: &str,
    key: &str,
    base: Option<&Table>,
) -> mlua::Result<Table> {
    let cls = api.class(ns, name, base)?;
    api.lua()
        .set_named_registry_value(&format!("{CLASS_KEY}{key}"), cls.clone())?;
    Ok(cls)
}

/// The `__call` of an instantiable class: a fresh table whose `__index` is the class,
/// passed with the arguments to the class's `Constructor` (normal indexing, so a Lua
/// override counts).
fn instantiate(api: &NativeApi<'_>) -> mlua::Result<mlua::Function> {
    api.function("__call", |lua, mut args: MultiValue| {
        let Some(Value::Table(cls)) = args.pop_front() else {
            return Err(script_error("class call without a class table"));
        };
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", cls.clone())?;
        obj.set_metatable(Some(mt))?;
        if let Value::Function(ctor) = cls.get::<Value>("Constructor")? {
            args.push_front(Value::Table(obj.clone()));
            ctor.call::<()>(args)?;
        }
        Ok(obj)
    })
}

/// A control constructor of `kind` with the look `styled`.
pub(crate) fn control_ctor(
    kind: Kind,
    styled: Styled,
) -> impl Fn(&Lua, &Rc<Shared>, &str, &Table, &Table, MultiValue) -> mlua::Result<()> + Send + 'static
{
    move |lua, sh, ap, this, cls, _args| {
        create_control(lua, sh, ap, this, cls, kind, styled).map(|_| ())
    }
}

/// Create the element of a new control and bind it to `this`.
pub(crate) fn create_control(
    lua: &Lua,
    sh: &Rc<Shared>,
    ap: &str,
    this: &Table,
    cls: &Table,
    kind: Kind,
    styled: Styled,
) -> mlua::Result<u64> {
    let aid = sh.apartment_id(ap);
    let (key, based) = {
        let mut ui = sh
            .ui
            .try_borrow_mut()
            .map_err(|_| script_error("the user interface is busy"))?;
        match super::styled::base_element(kind, styled) {
            Some((class_name, base)) => {
                // A game-styled text box edits in its text element (§14.2.2): its own
                // element is a plain control, so focus and keys never land on it.
                let ui_kind = if kind == Kind::TextBox {
                    tessera_ui::ControlKind::Control
                } else {
                    kind.ui_kind()
                };
                match ui.create_based_control(ui_kind, aid, super::styled::PLUGIN_LAYOUT_KEY, base)
                {
                    Ok(k) => (k, true),
                    Err(e) => {
                        super::styled::fallback_once(class_name, &e);
                        let k = ui
                            .create_control(kind.ui_kind(), aid)
                            .map_err(|e| script_error(e.to_string()))?;
                        (k, false)
                    }
                }
            }
            None => (
                ui.create_control(kind.ui_kind(), aid)
                    .map_err(|e| script_error(e.to_string()))?,
                false,
            ),
        }
    };
    let mut ctl = Ctl::new(key, kind, styled);
    ctl.based = based;
    let id = match bind_new(lua, sh, ap, this, cls, ObjData::Ctl(ctl)) {
        Ok(id) => id,
        Err(e) => {
            if let Ok(mut ui) = sh.ui.try_borrow_mut() {
                ui.destroy_control(key);
            }
            return Err(e);
        }
    };
    super::control::init(lua, sh, id);
    Ok(id)
}

/// Create an internal part (no Lua object) of `owner` under the element `parent`.
pub(crate) fn create_part(
    sh: &Rc<Shared>,
    ap: &str,
    owner: u64,
    parent: Option<tessera_ui::ElementKey>,
    kind: Kind,
    role: super::model::Role,
) -> Option<(u64, tessera_ui::ElementKey)> {
    let mut ui = sh.ui.try_borrow_mut().ok()?;
    let mut st = sh.st.borrow_mut();
    create_part_with(&mut ui, &mut st, ap, owner, parent, kind, role)
}

/// [`create_part`] with the UI and state borrowed.
pub(crate) fn create_part_with(
    ui: &mut tessera_ui::Ui,
    st: &mut super::state::State,
    ap: &str,
    owner: u64,
    parent: Option<tessera_ui::ElementKey>,
    kind: Kind,
    role: super::model::Role,
) -> Option<(u64, tessera_ui::ElementKey)> {
    let aid = super::state::apartment_id_with(st, ap);
    let id = super::state::alloc_with(st, ap)?;
    let key = match ui.create_control(kind.ui_kind(), aid) {
        Ok(k) => k,
        Err(_) => {
            if let Some(n) = st.per_apartment.get_mut(ap) {
                *n = n.saturating_sub(1);
            }
            return None;
        }
    };
    if let Some(p) = parent
        && ui.control_set_parent(key, Some(p)).is_err()
    {
        ui.destroy_control(key);
        if let Some(n) = st.per_apartment.get_mut(ap) {
            *n = n.saturating_sub(1);
        }
        return None;
    }
    let mut ctl = Ctl::new(key, kind, Styled::Plain);
    ctl.internal = Some((role, owner));
    st.by_key.insert(key, id);
    st.objs.insert(
        id,
        Obj {
            apartment: ap.to_owned(),
            data: ObjData::Ctl(ctl),
        },
    );
    if let Some(ObjData::Ctl(c)) = st.objs.get_mut(&owner).map(|o| &mut o.data) {
        c.parts.push(id);
    }
    Some((id, key))
}

/// Longest path a `Graphic` keeps, in bytes (Tessera bound; longer paths give an empty
/// graphic).
pub(crate) const MAX_GRAPHIC_PATH_BYTES: usize = 4096;

/// Install every table of both packages.
pub(crate) fn install_all(api: &NativeApi<'_>, sh: &Rc<Shared>) -> mlua::Result<()> {
    let lua = api.lua();
    install_registries(lua)?;
    // §6.3 step 4: the apartment's controls go with it.
    let b = sh.id;
    let ap = api.apartment().to_owned();
    api.on_close(move || {
        if let Ok(sh) = shared(b) {
            sh.close_apartment(&ap);
        }
    });
    let ns_root = api.ns_root().clone();
    let ui: Table = ns_root.raw_get("UI")?;
    let styled_ns: Table = ui.raw_get("Lotro")?;
    let object = api.object_class()?;

    for (name, values) in [
        ("BlendMode", enums::BLEND_MODE),
        ("ContentAlignment", enums::CONTENT_ALIGNMENT),
        ("EdgeAttachmentType", enums::EDGE_ATTACHMENT),
        ("FontStyle", enums::FONT_STYLE),
        ("MouseButton", enums::MOUSE_BUTTON),
        ("Orientation", enums::ORIENTATION),
        ("HorizontalLayout", enums::HORIZONTAL_LAYOUT),
        ("VerticalLayout", enums::VERTICAL_LAYOUT),
    ] {
        api.enumeration(&ui, name, values)?;
    }
    for (name, values) in [
        ("Font", enums::FONT),
        ("Action", enums::ACTION),
        ("LotroUIElement", enums::UI_ELEMENT),
        ("ShortcutType", enums::SHORTCUT_TYPE),
        ("UIScalingGroupType", enums::SCALING_GROUP),
    ] {
        api.enumeration(&styled_ns, name, values)?;
    }

    // Color (§13.9).
    let colour = class(
        api,
        sh,
        &ui,
        "Color",
        "Color",
        Some(&object),
        |_: &Lua, _: &Rc<Shared>, _: &str, this: &Table, _: &Table, args: MultiValue| {
            super::colour::store(this, super::colour::from_args(&args))
        },
    )?;
    for (name, c) in super::colour::all_named() {
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", colour.clone())?;
        obj.set_metatable(Some(mt))?;
        super::colour::store(&obj, c)?;
        colour.raw_set(name, obj)?;
    }

    // Graphic (§13.8).
    class(
        api,
        sh,
        &ui,
        "Graphic",
        "Graphic",
        Some(&object),
        |lua: &Lua, sh: &Rc<Shared>, ap: &str, this: &Table, cls: &Table, args: MultiValue| {
            let bg = match args.front() {
                // A path longer than any `Plugins/` path is not copied (the copy lives
                // outside the apartment's memory cap; Tessera bound).
                Some(Value::String(s)) if s.as_bytes().len() > MAX_GRAPHIC_PATH_BYTES => {
                    Background::None
                }
                Some(Value::String(s)) => {
                    let p = s.to_str().map(|s| s.to_owned()).unwrap_or_default();
                    if p.is_empty() {
                        Background::None
                    } else {
                        Background::Path(p)
                    }
                }
                v @ Some(Value::Number(_) | Value::Integer(_)) => {
                    Background::Did(num(v).unwrap_or(0.0))
                }
                _ => Background::None,
            };
            bind_new(lua, sh, ap, this, cls, ObjData::Graphic(bg)).map(|_| ())
        },
    )?;

    super::control::install(api, sh, &ui, &object)?;
    super::text::install(api, sh, &ui, &styled_ns)?;
    super::window::install(api, sh, &ui, &styled_ns)?;
    super::scroll::install(api, sh, &ui, &styled_ns)?;
    super::lists::install(api, sh, &ui, &object)?;
    super::menu::install(api, sh, &ui, &object)?;
    super::shortcut::install(api, sh, &ui, &styled_ns, &object)?;
    install_display(api, sh, &ui)?;
    install_panel_switches(api, sh, &styled_ns)?;
    install_tier2(api, sh, &styled_ns)?;
    Ok(())
}

/// `Turbine.UI.Display` (§13.10).
fn install_display(api: &NativeApi<'_>, sh: &Rc<Shared>, ui: &Table) -> mlua::Result<()> {
    let d = api.statics(ui, "Display")?;
    fn size(sh: &Shared) -> (i32, i32) {
        sh.ui.try_borrow().map_or((0, 0), |u| u.screen_size())
    }
    fn mouse(sh: &Shared) -> (i32, i32) {
        sh.ui
            .try_borrow()
            .ok()
            .and_then(|u| u.pointer())
            .unwrap_or((0, 0))
    }
    def_static(api, sh, &d, "GetWidth", |_, sh, _| {
        Ok(f64::from(size(sh).0))
    })?;
    def_static(api, sh, &d, "GetHeight", |_, sh, _| {
        Ok(f64::from(size(sh).1))
    })?;
    def_static(api, sh, &d, "GetSize", |_, sh, _| {
        let (w, h) = size(sh);
        Ok((f64::from(w), f64::from(h)))
    })?;
    def_static(api, sh, &d, "GetMouseX", |_, sh, _| {
        Ok(f64::from(mouse(sh).0))
    })?;
    def_static(api, sh, &d, "GetMouseY", |_, sh, _| {
        Ok(f64::from(mouse(sh).1))
    })?;
    def_static(api, sh, &d, "GetMousePosition", |_, sh, _| {
        let (x, y) = mouse(sh);
        Ok((f64::from(x), f64::from(y)))
    })?;
    def_static(api, sh, &d, "GetUIGroupScale", |_, sh, args| {
        let g = int(args.front()) as u32;
        // §7.7: the `Global` group is 1.0 for plugins (Tessera's UI scale lies below the
        // UI-unit space, decision 0040); any other group reports its recorded value, 1.0
        // when it was never scaled (a group without a `UIScaleDefinition` entry has 1.0).
        let s = if g == super::window::GLOBAL_GROUP {
            1.0
        } else {
            sh.st.borrow().group_scales.get(&g).copied().unwrap_or(1.0)
        };
        Ok(f64::from(s))
    })?;
    Ok(())
}

/// Is `e` a `LotroUIElement` value (Appendix A)?
fn known_panel(e: u32) -> bool {
    enums::UI_ELEMENT.iter().any(|(_, v)| *v == e)
}

/// `Turbine.UI.Lotro.LotroUI` (§14.3): a flag store until native panels can be replaced.
/// Only `LotroUIElement` values are stored; any other value is ignored (and reported as
/// enabled), so the store stays bounded.
fn install_panel_switches(
    api: &NativeApi<'_>,
    sh: &Rc<Shared>,
    styled_ns: &Table,
) -> mlua::Result<()> {
    let t = api.statics(styled_ns, "LotroUI")?;
    def_static(api, sh, &t, "IsEnabled", |_, sh, args| {
        let e = int(args.front()) as u32;
        Ok(sh
            .st
            .borrow()
            .panel_switches
            .get(&e)
            .copied()
            .unwrap_or(true))
    })?;
    def_static(api, sh, &t, "SetEnabled", |_, sh, args| {
        let e = int(args.front()) as u32;
        let v = truthy(args.get(1));
        if known_panel(e) {
            sh.st.borrow_mut().panel_switches.insert(e, v);
        }
        Ok(())
    })?;
    def_static(api, sh, &t, "Reset", |_, sh, _| {
        sh.st.borrow_mut().panel_switches.clear();
        Ok(())
    })?;
    Ok(())
}

/// The tier-2 game-object controls (§14.5): plain controls whose game members return
/// defaults until gameplay data exists (§17.1).
fn install_tier2(api: &NativeApi<'_>, sh: &Rc<Shared>, styled_ns: &Table) -> mlua::Result<()> {
    let control = super::state::class_table(api.lua(), "Control")?;
    let base = class(
        api,
        sh,
        styled_ns,
        "BaseItemControl",
        "Lotro.BaseItemControl",
        Some(&control),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    let item = class(
        api,
        sh,
        styled_ns,
        "ItemControl",
        "Lotro.ItemControl",
        Some(&base),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    def_stub(api, &item, "GetItem", Value::Nil)?;
    def_stub(api, &item, "SetItem", Value::Nil)?;
    let info = class(
        api,
        sh,
        styled_ns,
        "ItemInfoControl",
        "Lotro.ItemInfoControl",
        Some(&base),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    def_stub(api, &info, "GetItemInfo", Value::Nil)?;
    def_stub(api, &info, "SetItemInfo", Value::Nil)?;
    def_stub(api, &info, "GetQuantity", Value::Number(0.0))?;
    def_stub(api, &info, "SetQuantity", Value::Nil)?;
    let slot = class(
        api,
        sh,
        styled_ns,
        "EquipmentSlot",
        "Lotro.EquipmentSlot",
        Some(&base),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    def_stub(api, &slot, "GetEquipmentSlot", Value::Number(0.0))?;
    def_stub(api, &slot, "SetEquipmentSlot", Value::Nil)?;
    let entity = class(
        api,
        sh,
        styled_ns,
        "EntityControl",
        "Lotro.EntityControl",
        Some(&control),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    def_stub(api, &entity, "GetEntity", Value::Nil)?;
    def_stub(api, &entity, "SetEntity", Value::Nil)?;
    def_stub(api, &entity, "IsSelectionEnabled", Value::Boolean(true))?;
    def_stub(api, &entity, "SetSelectionEnabled", Value::Nil)?;
    def_stub(api, &entity, "IsContextMenuEnabled", Value::Boolean(true))?;
    def_stub(api, &entity, "SetContextMenuEnabled", Value::Nil)?;
    let effect = class(
        api,
        sh,
        styled_ns,
        "EffectDisplay",
        "Lotro.EffectDisplay",
        Some(&control),
        control_ctor(Kind::Control, Styled::Plain),
    )?;
    def_stub(api, &effect, "GetEffect", Value::Nil)?;
    def_stub(api, &effect, "SetEffect", Value::Nil)?;
    Ok(())
}
