// SPDX-License-Identifier: GPL-3.0-or-later
//! Records of the layer's objects and the argument tables of their events
//! (`lua-plugin-api.md` §9.3, §13).

use mlua::{Lua, Table, Value};
use tessera_ui::ElementKey;

use super::shortcut::ShortcutData;
use super::state::{Shared, class_table, remember};
use crate::api::bind_object;

/// The control classes of the layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// `Control`, `ScrollableControl` and the game-object controls without behaviour.
    Control,
    /// `Window` (and the game-styled windows).
    Window,
    /// `Label`.
    Label,
    /// `TextBox`.
    TextBox,
    /// `Button`.
    Button,
    /// `CheckBox`.
    CheckBox,
    /// `ScrollBar`.
    ScrollBar,
    /// `ListBox`.
    ListBox,
    /// `TreeView`.
    TreeView,
    /// `TreeNode`.
    TreeNode,
    /// `Turbine.UI.Lotro.Quickslot`.
    Quickslot,
}

impl Kind {
    /// Does the class derive from `Label` (text members, §13.4)?
    pub(crate) fn has_text(self) -> bool {
        matches!(
            self,
            Self::Label | Self::TextBox | Self::Button | Self::CheckBox
        )
    }

    /// Does the class derive from `ScrollableControl`?
    pub(crate) fn scrollable(self) -> bool {
        self.has_text() || matches!(self, Self::ListBox | Self::TreeView | Self::Control)
    }

    /// The `tessera-ui` control kind behind the class.
    pub(crate) fn ui_kind(self) -> tessera_ui::ControlKind {
        use tessera_ui::ControlKind as K;
        match self {
            Self::Window => K::Window,
            Self::Label => K::Label,
            Self::TextBox => K::TextBox,
            Self::Button => K::Button,
            Self::CheckBox => K::CheckBox,
            // Lists, trees and scroll bars are laid out and driven by this layer.
            _ => K::Control,
        }
    }
}

/// Game-styled look of a control (§14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Styled {
    /// A plain `Turbine.UI` control.
    #[default]
    Plain,
    /// The standard game style.
    Silver,
    /// The gold variant.
    Gold,
}

/// Text state of a `Label`-derived control (§13.4).
#[derive(Debug, Clone)]
pub(crate) struct TextState {
    pub text: String,
    pub font: u32,
    pub font_style: u32,
    pub fore: Option<[f32; 4]>,
    pub outline: [f32; 4],
    pub alignment: u32,
    pub multiline: bool,
    pub markup: bool,
    pub selectable: bool,
    pub read_only: bool,
    /// Selection: 0-based start and length in UTF-16 units.
    pub sel_start: usize,
    pub sel_len: usize,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            text: String::new(),
            font: 0,
            font_style: 0,
            fore: None,
            outline: [1.0, 0.0, 0.0, 0.0],
            alignment: 1,
            multiline: true,
            markup: false,
            selectable: false,
            read_only: false,
            sel_start: 0,
            sel_len: 0,
        }
    }
}

/// The five numbers of a scroll bar and its two thumb fractions (§9.6, scroll-bar
/// numbers, Thumb).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScrollNums {
    pub min: u32,
    pub max: u32,
    pub value: u32,
    pub large: u32,
    pub small: u32,
    /// Thumb position fraction (value − minimum) ÷ (maximum − minimum), set only where
    /// §9.6 says (creation, a `SetValue` past the equality test).
    pub pos_frac: f32,
    /// Thumb size fraction large change ÷ (maximum − minimum), set only where §9.6 says
    /// (creation, a `SetMinimum`/`SetMaximum` that changes a bound while maximum ≠
    /// minimum).
    pub size_frac: f32,
}

impl Default for ScrollNums {
    fn default() -> Self {
        Self {
            min: 0,
            max: 100,
            value: 0,
            large: 10,
            small: 1,
            pos_frac: 0.0,
            size_frac: 0.1,
        }
    }
}

/// Scroll-bar state.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollState {
    pub nums: ScrollNums,
    pub orientation: u32,
    pub decrement: Option<u64>,
    pub increment: Option<u64>,
    pub thumb: Option<u64>,
    /// The scrollable control the bar is bound to.
    pub bound: Option<u64>,
    /// A thumb drag in progress: pointer offset inside the thumb along the axis.
    pub drag: Option<i32>,
    /// A game-styled bar's layout widget (`lua-plugin-api.md` §14.2.2): the part element
    /// built from `LotroScrollBar_Horizontal` or `_Vertical` and its run-time element ID
    /// (the key of its listener-registry entries).
    pub widget: Option<(ElementKey, u32)>,
}

/// List-box state (§13.7).
#[derive(Debug, Clone)]
pub(crate) struct ListState {
    pub items: Vec<u64>,
    pub selected: Option<u64>,
    pub orientation: u32,
    pub max_per_line: i32,
    pub max_rows: i32,
    pub max_columns: i32,
    pub reverse: bool,
    pub flipped: bool,
    pub wrap_indent: i32,
    pub ignore_cell: bool,
    /// Content size of the last layout.
    pub content: (i32, i32),
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            orientation: 1,
            max_per_line: 0,
            max_rows: 0,
            max_columns: 0,
            reverse: false,
            flipped: false,
            wrap_indent: 0,
            ignore_cell: false,
            content: (0, 0),
        }
    }
}

/// Tree-view state.
#[derive(Debug, Clone)]
pub(crate) struct TreeState {
    pub nodes: Vec<u64>,
    pub selected: Option<u64>,
    pub indent: i32,
    pub content: (i32, i32),
    /// List objects created for `GetNodes`.
    pub nodes_list: Option<u64>,
}

impl Default for TreeState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            selected: None,
            indent: super::lists::DEFAULT_INDENT,
            content: (0, 0),
            nodes_list: None,
        }
    }
}

/// Tree-node state.
#[derive(Debug, Clone, Default)]
pub(crate) struct NodeState {
    pub children: Vec<u64>,
    pub parent: Option<u64>,
    pub view: Option<u64>,
    pub expanded: bool,
    pub filtered: bool,
    pub children_list: Option<u64>,
}

/// A move (mode 0) or bottom-right resize (mode 1) of a window by the neutral frame: the
/// mode, the pointer on the screen at the start and the window's box (x, y, w, h) then.
pub(crate) type FrameDrag = (u8, (i32, i32), (i32, i32, i32, i32));

/// Window state (§13.3, §13.21).
#[derive(Debug, Clone, Default)]
pub(crate) struct WinState {
    pub text: String,
    pub resizable: bool,
    pub marked: bool,
    pub global: bool,
    pub group: u32,
    pub own_scale: Option<f32>,
    pub origin: (f64, f64),
    pub rotation: [f32; 3],
    pub closing: bool,
    /// A window move or resize by the neutral frame in progress: (mode, pointer start
    /// on screen, box at the start). Mode 0 moves, 1 resizes from the bottom-right.
    pub frame_drag: Option<FrameDrag>,
}

/// Quickslot state (§14.6).
#[derive(Debug, Clone, Default)]
pub(crate) struct QuickslotState {
    pub shortcut: Option<ShortcutData>,
    pub use_on_right_click: bool,
}

/// What an internal part (no Lua object) does for its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// Neutral frame of a game-styled window: background.
    FrameBackground,
    /// Neutral frame: title bar (moves the window).
    FrameTitle,
    /// Neutral frame: close button.
    FrameClose,
    /// Neutral frame: resize handle (bottom-right).
    FrameResize,
    /// A neutral scroll-bar thumb.
    Thumb,
    /// A context-menu popup.
    MenuPopup,
    /// A context-menu row (index into the open menu's items).
    MenuRow(usize),
}

/// A control record.
#[derive(Debug, Clone)]
pub(crate) struct Ctl {
    pub key: ElementKey,
    pub kind: Kind,
    pub styled: Styled,
    /// For internal parts: the role and the owning record.
    pub internal: Option<(Role, u64)>,
    /// The plugin's own visibility flag.
    pub visible: bool,
    /// Hidden by the layer (a node under a collapsed parent, a filtered node).
    pub layer_hidden: bool,
    pub enabled: bool,
    pub wants_updates: bool,
    pub wants_keys: bool,
    pub background: Background,
    pub text: Option<Box<TextState>>,
    pub checked: bool,
    pub check_alignment: u32,
    pub scroll: Option<Box<ScrollState>>,
    pub hbar: Option<u64>,
    pub vbar: Option<u64>,
    pub offset: (i32, i32),
    pub list: Option<Box<ListState>>,
    pub tree: Option<Box<TreeState>>,
    pub node: Option<Box<NodeState>>,
    pub window: Option<Box<WinState>>,
    pub quickslot: Option<Box<QuickslotState>>,
    /// Internal parts owned by this control.
    pub parts: Vec<u64>,
    /// The control's `ControlList` object, once asked for.
    pub controls_list: Option<u64>,
    /// The control this one is anchored to (its parent control, list or tree).
    pub owner: Option<u64>,
    /// The scroll bar this control is a replaceable part of: (bar, 0 decrement,
    /// 1 increment, 2 thumb).
    pub part_of: Option<(u64, u8)>,
    /// Built from the install's layout (`lua-plugin-api.md` §14.2.1) rather than with
    /// Tessera's neutral art (§14.2.1 step 5).
    pub based: bool,
    /// The element the `Label` members act on when it is not the control's own element
    /// (§14.2.2: a game-styled check box's `LotroCheckBox_Text`).
    pub text_part: Option<ElementKey>,
    /// A game-styled check box's check mark (`LotroCheckBox_CheckBox`, §14.2.2), drawn
    /// by the checked state.
    pub mark_part: Option<ElementKey>,
}

impl Ctl {
    pub(crate) fn new(key: ElementKey, kind: Kind, styled: Styled) -> Self {
        Self {
            key,
            kind,
            styled,
            internal: None,
            visible: false,
            layer_hidden: false,
            enabled: true,
            wants_updates: false,
            wants_keys: false,
            background: Background::None,
            text: kind.has_text().then(Box::default),
            checked: false,
            check_alignment: 4,
            scroll: (kind == Kind::ScrollBar).then(Box::default),
            hbar: None,
            vbar: None,
            offset: (0, 0),
            list: (kind == Kind::ListBox).then(Box::default),
            tree: (kind == Kind::TreeView).then(Box::default),
            node: (kind == Kind::TreeNode).then(Box::default),
            window: (kind == Kind::Window).then(Box::default),
            quickslot: (kind == Kind::Quickslot).then(Box::default),
            parts: Vec::new(),
            controls_list: None,
            owner: None,
            part_of: None,
            based: false,
            text_part: None,
            mark_part: None,
        }
    }

    /// The element the text members act on (§14.2.2).
    pub(crate) fn text_key(&self) -> ElementKey {
        self.text_part.unwrap_or(self.key)
    }
}

/// What `GetBackground()` returns: the value last given to `SetBackground`.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Background {
    #[default]
    None,
    Did(f64),
    Path(String),
    Graphic(u64),
}

/// Which list a list object views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListView {
    /// `ControlList`: the child controls of a control.
    Controls(u64),
    /// `MenuItemList`: the items of a context menu or menu item.
    MenuItems(u64),
    /// `TreeNodeList`: the top-level nodes of a tree view.
    TreeNodes(u64),
    /// `TreeNodeList`: the child nodes of a node.
    ChildNodes(u64),
}

/// A menu item or context menu (§13.10).
#[derive(Debug, Clone, Default)]
pub(crate) struct MenuState {
    pub text: String,
    pub enabled: bool,
    pub checked: bool,
    pub items: Vec<u64>,
    pub list: Option<u64>,
    /// The context menu or menu item whose item list holds this item.
    pub parent: Option<u64>,
}

/// Data of a record.
#[derive(Debug, Clone)]
pub(crate) enum ObjData {
    Ctl(Ctl),
    /// `ContextMenu` (`true`) or `MenuItem` (`false`).
    Menu(bool, MenuState),
    List(ListView),
    Shortcut(ShortcutData),
    DragDrop {
        successful: bool,
        shortcut: Option<ShortcutData>,
    },
    Graphic(Background),
}

/// A record of the layer.
#[derive(Debug, Clone)]
pub(crate) struct Obj {
    pub apartment: String,
    pub data: ObjData,
}

/// The fields of an event's argument table (§9.3).
#[derive(Debug, Clone)]
pub(crate) enum Args {
    Empty,
    Mouse {
        button: u32,
        x: i32,
        y: i32,
    },
    Pos {
        x: i32,
        y: i32,
    },
    Wheel {
        direction: i32,
        x: i32,
        y: i32,
    },
    Key {
        action: u32,
        alt: bool,
        control: bool,
        shift: bool,
    },
    Item(u64),
    DragDrop {
        x: i32,
        y: i32,
        shortcut: Option<ShortcutData>,
    },
    Closing,
    Scale {
        index: u32,
        value: f32,
    },
}

/// Build the argument table of `args` in the Lua state of `apartment`.
pub(crate) fn build_args(
    lua: &Lua,
    sh: &Shared,
    apartment: &str,
    args: &Args,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    match args {
        Args::Empty => {}
        Args::Mouse { button, x, y } => {
            t.raw_set("Button", f64::from(*button))?;
            t.raw_set("X", f64::from(*x))?;
            t.raw_set("Y", f64::from(*y))?;
        }
        Args::Pos { x, y } => {
            t.raw_set("X", f64::from(*x))?;
            t.raw_set("Y", f64::from(*y))?;
        }
        Args::Wheel { direction, x, y } => {
            t.raw_set("Direction", f64::from(*direction))?;
            t.raw_set("X", f64::from(*x))?;
            t.raw_set("Y", f64::from(*y))?;
        }
        Args::Key {
            action,
            alt,
            control,
            shift,
        } => {
            t.raw_set("Action", f64::from(*action))?;
            t.raw_set("Alt", *alt)?;
            t.raw_set("Control", *control)?;
            t.raw_set("Shift", *shift)?;
        }
        Args::Item(id) => {
            if let Some(obj) = super::state::object_of(lua, *id) {
                t.raw_set("Item", obj)?;
            }
        }
        Args::DragDrop { x, y, shortcut } => {
            let info = new_object(
                lua,
                sh,
                apartment,
                "Lotro.DragDropInfo",
                ObjData::DragDrop {
                    successful: false,
                    shortcut: shortcut.clone(),
                },
            )?;
            t.raw_set("DragDropInfo", info)?;
            t.raw_set("X", f64::from(*x))?;
            t.raw_set("Y", f64::from(*y))?;
        }
        Args::Closing => t.raw_set("Cancel", false)?,
        Args::Scale { index, value } => {
            t.raw_set("Index", f64::from(*index))?;
            t.raw_set("Value", f64::from(*value))?;
        }
    }
    Ok(t)
}

/// Create an instance of the stored class `class` bound to a new record holding `data`.
pub(crate) fn new_object(
    lua: &Lua,
    sh: &Shared,
    apartment: &str,
    class: &str,
    data: ObjData,
) -> mlua::Result<Table> {
    let cls = class_table(lua, class)?;
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", cls.clone())?;
    obj.set_metatable(Some(mt))?;
    let id = sh.alloc(apartment)?;
    if let Err(e) =
        bind_object(lua, &obj, &cls, sh.obj_ref(id)).and_then(|()| remember(lua, id, &obj))
    {
        sh.unalloc(apartment);
        return Err(e);
    }
    sh.insert(
        id,
        Obj {
            apartment: apartment.to_owned(),
            data,
        },
    );
    Ok(obj)
}

/// A `Turbine.UI.Color` instance with the given (A, R, G, B) components.
pub(crate) fn new_colour(lua: &Lua, c: [f32; 4]) -> mlua::Result<Table> {
    let cls = class_table(lua, "Color")?;
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", cls)?;
    obj.set_metatable(Some(mt))?;
    super::colour::store(&obj, c)?;
    Ok(obj)
}

/// A number argument: Lua numbers, and strings Lua 5.1 converts to numbers.
pub(crate) fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => Some(*n),
        Value::Integer(i) => Some(*i as f64),
        Value::String(s) => {
            let s = s.to_str().ok()?;
            let t = s.trim();
            if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                u64::from_str_radix(h, 16).ok().map(|v| v as f64)
            } else {
                t.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

/// An integer argument (truncated toward zero, saturating to `i32`; not a number → 0).
pub(crate) fn int(v: Option<&Value>) -> i32 {
    match num(v) {
        Some(n) if n.is_finite() => {
            n.trunc().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        }
        _ => 0,
    }
}

/// A boolean argument with Lua truthiness.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Nil) | Some(Value::Boolean(false)))
}

/// Bind `obj` (an instance under construction) to a new record holding `data`.
pub(crate) fn bind_new(
    lua: &Lua,
    sh: &Shared,
    apartment: &str,
    obj: &Table,
    class: &Table,
    data: ObjData,
) -> mlua::Result<u64> {
    let id = sh.alloc(apartment)?;
    if let Err(e) =
        bind_object(lua, obj, class, sh.obj_ref(id)).and_then(|()| remember(lua, id, obj))
    {
        // The record is never inserted, so its slot is given back here (the dropped
        // payload's removal finds no record).
        sh.unalloc(apartment);
        return Err(e);
    }
    sh.insert(
        id,
        Obj {
            apartment: apartment.to_owned(),
            data,
        },
    );
    Ok(id)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use tessera_ui::{MemoryEnv, Ui, Vocab};

    use super::*;

    /// A constructor that fails after its record number was taken gives the number back
    /// (here the apartment's registries are missing, so binding fails).
    #[test]
    fn a_failed_bind_gives_its_slot_back() {
        let ui = Rc::new(RefCell::new(Ui::new(
            Box::new(MemoryEnv::default()),
            Vocab::default(),
            (100, 100),
        )));
        let sh = super::super::state::register(ui);
        let lua = Lua::new();
        let obj = lua.create_table().unwrap();
        let cls = lua.create_table().unwrap();
        for _ in 0..3 {
            let r = bind_new(
                &lua,
                &sh,
                "ap",
                &obj,
                &cls,
                ObjData::Graphic(Background::None),
            );
            assert!(r.is_err());
        }
        assert_eq!(sh.st.borrow().per_apartment.get("ap").copied(), Some(0));
        super::super::state::unregister(sh.id);
    }
}
