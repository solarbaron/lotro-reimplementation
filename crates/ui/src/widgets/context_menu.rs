// SPDX-License-Identifier: GPL-3.0-or-later
//! Context menus (`ui-widgets.md` §19): a menu built by game code from an ordered item
//! list, drawn from layout `ContextMenu`, with submenus at any depth.
//!
//! A [`MenuRequest`] (items, optional title, position and menu element) opens a
//! [`ContextMenu`] tree with [`ContextMenu::open`]. The owner hands it every UI event
//! ([`ContextMenu::handle_event`]) and calls [`ContextMenu::update`] once per frame (the
//! open delay of §19.4 step 2 needs the clock); a selection, a close item or a deactivation
//! comes back as a [`MenuEvent`], and the owner decides what it does (`ui-widgets.md`
//! §19.4 Tessera rule: the result reaches the requesting panel instead of a listener
//! message).
//!
//! What §19.3 gives, as built here:
//!
//! - **Menu element**: the request's element, else `ContextMenuWithTitleElement` with a
//!   title, else `ContextMenuElement`; it blocks clicks, is activatable, carries
//!   `UICore_ContextMenu_lvl` = its depth and lies in front of the screen's panels (a
//!   submenu in front of its parent).
//! - **Items**: kind 1 `ContextMenuBool`, 2 `ContextMenuSub`, 3 `ContextMenuEntry`; label in
//!   the text part, icon (draw mode 4) in the icon part, `UICore_ContextMenu_lvl` and
//!   `UICore_ContextMenu_index`; `normal` or `ghosted`.
//! - **Sizes** (W31): item width = text width + [`TEXT_PAD`] + (template width − text part
//!   width), height = text height + (template height − text part height); every item of a
//!   menu takes the widest item's width (at least the title's); the list is that wide and
//!   as high as its items, at most [`LIST_HEIGHT_FRACTION`] × the screen height (beyond
//!   that the list scrolls); the menu is the list plus the template's margins around it.
//! - **Scale to fit**: a menu larger than the screen is scaled down uniformly.
//! - **Placement**: a given position (top-left, or bottom-left for the upward variant), a
//!   submenu at the right edge of its item (left of it when it would leave the screen on
//!   the right), else the pointer; then moved the least distance onto the screen.
//! - **Check marks**: shown by the item's show flag, state `normal_activated` when checked,
//!   else `ghosted`.
//!
//! **Tessera rules.** Each menu of the tree holds at most one open submenu, so the open
//! tree is a chain (root, its open submenu, that one's open submenu, …). A submenu is built
//! when it is opened, so a submenu whose list holds an item of an invalid kind is not shown
//! when it is opened (§19.2 OPEN rule); the root's list is checked at [`ContextMenu::open`].
//! The request's menu element is used for every menu of the tree (the map window's
//! `ContextMenuElement_MapMenu`, `map-windows.md` §4.5). A submenu opens on hover only
//! when its item is enabled. Without the install's `ContextMenu` layout (no install, or a
//! layout or element that does not resolve) the menus are code-made elements with the
//! §19.1 boxes and Tessera's own art ([`MenuArt`]): same parts, same rules.
//!
//! **Closing on deactivation** (§19.4 step 4): every menu of the tree is an activatable
//! root panel and the root menu is activated when it opens; a press anywhere outside the
//! tree activates another panel (or none), which deactivates the active menu. On that
//! `Element_deactivated` the tree closes unless the newly active panel is one of its menus.
//! When a submenu that is the active panel closes (hover, a click on another item), its
//! parent menu is activated first, so the tree always holds the active panel while open.
//!
//! **Fonts not loaded yet** (Tessera rule). An item or title whose text cannot be measured
//! yet (the client's shaper has not loaded its font) is sized from its template's text
//! part; at a later [`ContextMenu::update`], once the text can be measured, that menu is
//! built and sized again at the same place (a submenu reopens from its item; a rebuilt root
//! menu closes its open submenu).
//!
//! # Bounds
//!
//! At most [`MAX_ITEMS`] items per menu, [`MAX_DEPTH`] levels and [`MAX_TREE_ITEMS`] items
//! in the whole request; labels are cut at [`MAX_LABEL_CHARS`] characters. Longer lists are
//! refused ([`GameError::TooLarge`]), never truncated silently.

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::env::{ImageRef, TextRequest};
use crate::events::UiEvent;
use crate::game::GameError;
use crate::game::npc::kit::{FNode, colours, instantiate, open_root};
use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::{MAX_TREE_DEPTH, Ui};
use crate::widgets::WidgetClass;

/// The fixed pixels added to an item's text width (`ui-widgets.md` §19.3 "Item size", W31).
pub const TEXT_PAD: i32 = 10;

/// The largest list height as a fraction of the screen height (§19.3 "Common width", W31).
pub const LIST_HEIGHT_FRACTION: f32 = 0.8;

/// The open delay of the fallback menus, seconds: the value `ContextMenuElement` carries in
/// the layout (`UICore_ContextMenu_open_delay`, §19.1); the install's menus read their own.
pub const FALLBACK_OPEN_DELAY: f64 = 0.5;

/// [`FALLBACK_OPEN_DELAY`] as the fallback menu element's property value.
const FALLBACK_OPEN_DELAY_F32: f32 = 0.5;

/// A click on a menu button this soon after its menu closed does not open the menu again
/// (`map-windows.md` §4.5 "a second click within a short time after it closed does not
/// reopen it"; the time is a Tessera choice, seconds of the UI clock, also used by the
/// finder's menu buttons). The press of that click deactivates the menu, which closes it,
/// and its `Button_click` follows in the same frame.
pub const REOPEN_GUARD: f64 = 0.3;

/// Most items in one menu (Tessera bound).
pub const MAX_ITEMS: usize = 1024;

/// Most menu levels of a tree (Tessera bound).
pub const MAX_DEPTH: usize = 16;

/// Most items of a whole request, at every depth (Tessera bound).
pub const MAX_TREE_ITEMS: usize = 8192;

/// Most characters of a label or title (Tessera bound).
pub const MAX_LABEL_CHARS: usize = 1024;

/// Tessera's element ID of every item element of an open menu (a placeholder no data
/// uses). The item templates' own IDs are shared with elements of other layouts, and
/// registering mouse messages for an ID makes every element with it mouse-visible
/// (`ui-behaviour.md` §4.3 item 2), so an item takes this ID once it is created; its parts
/// keep theirs and are not mouse-visible (a click on any part reaches the item).
pub const ITEM_ID: u32 = 0xFFFB_70FF;

/// Element ID of the fallback menus' background (Tessera's code range).
pub const FALLBACK_BACKGROUND_ID: u32 = 0x7EFF_7001;

/// The `UILAYOUT` label of the context-menu layout (§19, `ContextMenu`).
pub const LAYOUT_LABEL: &str = "ContextMenu";

/// The kind of a menu item (§19.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// Kind 1: a check item (`ContextMenuBool`).
    Check,
    /// Kind 2: a submenu item (`ContextMenuSub`).
    Submenu,
    /// Kind 3: a plain item (`ContextMenuEntry`).
    Plain,
    /// Any other kind value: not a valid menu item.
    Other(u32),
}

impl ItemKind {
    /// The kind of a raw value (1, 2, 3; anything else is [`ItemKind::Other`]).
    #[must_use]
    pub fn from_raw(v: u32) -> Self {
        match v {
            1 => Self::Check,
            2 => Self::Submenu,
            3 => Self::Plain,
            o => Self::Other(o),
        }
    }

    /// The raw value.
    #[must_use]
    pub fn raw(self) -> u32 {
        match self {
            Self::Check => 1,
            Self::Submenu => 2,
            Self::Plain => 3,
            Self::Other(o) => o,
        }
    }

    /// Whether the kind is a valid menu item (1, 2 or 3).
    #[must_use]
    pub fn is_valid(self) -> bool {
        !matches!(self, Self::Other(_))
    }
}

/// One item of a menu (§19.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// The kind.
    pub kind: ItemKind,
    /// The label text.
    pub label: String,
    /// An optional icon.
    pub icon: Option<ImageRef>,
    /// Enabled (else shown `ghosted`; a click does nothing).
    pub enabled: bool,
    /// A check item's checked flag (its mark's state).
    pub checked: bool,
    /// A check item's flag whether its mark shows.
    pub show_check: bool,
    /// The item ID reported on selection.
    pub id: u32,
    /// The value reported on selection.
    pub value: i64,
    /// Selection reports a close (`Element_close_context_menu`) instead of a selection.
    pub reports_close: bool,
    /// A submenu item's own item list.
    pub submenu: Vec<MenuItem>,
}

impl MenuItem {
    fn new(kind: ItemKind, id: u32, label: &str) -> Self {
        Self {
            kind,
            label: label.to_string(),
            icon: None,
            enabled: true,
            checked: false,
            show_check: false,
            id,
            value: 0,
            reports_close: false,
            submenu: Vec::new(),
        }
    }

    /// A plain item (kind 3), enabled.
    #[must_use]
    pub fn plain(id: u32, label: &str) -> Self {
        Self::new(ItemKind::Plain, id, label)
    }

    /// A check item (kind 1), enabled, its mark shown and checked when `checked`.
    #[must_use]
    pub fn check(id: u32, label: &str, checked: bool) -> Self {
        let mut i = Self::new(ItemKind::Check, id, label);
        i.checked = checked;
        i.show_check = checked;
        i
    }

    /// A submenu item (kind 2) with its submenu's items, enabled.
    #[must_use]
    pub fn submenu(id: u32, label: &str, items: Vec<MenuItem>) -> Self {
        let mut i = Self::new(ItemKind::Submenu, id, label);
        i.submenu = items;
        i
    }

    /// This item with an icon.
    #[must_use]
    pub fn with_icon(mut self, icon: Option<ImageRef>) -> Self {
        self.icon = icon;
        self
    }

    /// This item enabled or disabled.
    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// This item with a value.
    #[must_use]
    pub fn with_value(mut self, v: i64) -> Self {
        self.value = v;
        self
    }

    /// This item reporting a close on selection.
    #[must_use]
    pub fn reporting_close(mut self) -> Self {
        self.reports_close = true;
        self
    }
}

/// A fixed position for a root menu (§19.3 "Placement").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPosition {
    /// The menu's top-left corner.
    TopLeft(i32, i32),
    /// The menu's bottom-left corner (the menu opens upward).
    BottomLeft(i32, i32),
}

/// What game code asks for (§19.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuRequest {
    /// The items, in order.
    pub items: Vec<MenuItem>,
    /// The title (empty: none).
    pub title: String,
    /// A fixed position (else the pointer).
    pub position: Option<MenuPosition>,
    /// An element ID that replaces the menu element.
    pub element: Option<u32>,
    /// Tessera: a selection does not close the tree (the map window's `FilterMenu`,
    /// `map-windows.md` §6.9 "the menu stays open").
    pub keep_open: bool,
}

impl MenuRequest {
    /// A request for `items` at the pointer, without a title.
    #[must_use]
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }
}

/// Declares a label table with placeholder values (Tessera placeholders that no data
/// uses, so an unresolved label never matches real data).
macro_rules! label_table {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident = ($label:literal, $value:expr)),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fm])* pub $field: u32,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $value,)* }
            }
        }

        impl $name {
            /// Every `(label, value)` pair of the table.
            #[must_use]
            pub fn entries(&self) -> Vec<(&'static str, u32)> {
                vec![$(($label, self.$field),)*]
            }

            /// Replaces each value whose label `lookup` resolves.
            pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
                $(if let Some(v) = lookup($label) { self.$field = v; })*
            }
        }
    };
}
pub(crate) use label_table;

label_table! {
    /// Element IDs of layout `ContextMenu` (`UIElementID` labels, §19.1).
    MenuIds {
        /// `ContextMenuElement`: menu without a title.
        element = ("ContextMenuElement", 0xFFFB_7001),
        /// `ContextMenuWithTitleElement`: menu with a title.
        with_title = ("ContextMenuWithTitleElement", 0xFFFB_7002),
        /// `ContextMenuElement_MapMenu`: the map window's menus.
        map_menu = ("ContextMenuElement_MapMenu", 0xFFFB_7003),
        /// `ContextMenuList`: the item list (a ListBox).
        list = ("ContextMenuList", 0xFFFB_7004),
        /// `ContextMenuTitle`: the title field.
        title = ("ContextMenuTitle", 0xFFFB_7005),
        /// `ContextMenuTitleText`: the title text.
        title_text = ("ContextMenuTitleText", 0xFFFB_7006),
        /// `ContextMenuEntry`: the plain item template.
        entry = ("ContextMenuEntry", 0xFFFB_7007),
        /// `ContextMenuBool`: the check item template.
        check_item = ("ContextMenuBool", 0xFFFB_7008),
        /// `ContextMenuSub`: the submenu item template.
        sub_item = ("ContextMenuSub", 0xFFFB_7009),
        /// `ContextMenuEntryIcon`: an item's icon part.
        icon = ("ContextMenuEntryIcon", 0xFFFB_700A),
        /// `ContextMenuText`: an item's text part.
        text = ("ContextMenuText", 0xFFFB_700B),
        /// `ContextMenuBoolFlag`: a check item's mark.
        flag = ("ContextMenuBoolFlag", 0xFFFB_700C),
        /// `ContextMenuOpen`: a submenu item's arrow.
        open_arrow = ("ContextMenuOpen", 0xFFFB_700D),
    }
}

label_table! {
    /// Property IDs the context menus read and write (property-dictionary names).
    MenuPids {
        /// `UICore_ContextMenu_open_delay` (seconds).
        open_delay = ("UICore_ContextMenu_open_delay", 0xFFFD_7001),
        /// `UICore_ContextMenu_lvl`: a menu's or item's depth.
        level = ("UICore_ContextMenu_lvl", 0xFFFD_7002),
        /// `UICore_ContextMenu_index`: an item's position in its list.
        index = ("UICore_ContextMenu_index", 0xFFFD_7003),
        /// `UICore_Text_font_color` (the fallback menus' text colour; the same placeholder
        /// as the P3 vocabulary's).
        text_font_color = ("UICore_Text_font_color", 0xFFFD_3006),
    }
}

/// Tessera's own art of the fallback menus (generated or CC0 images the client provides).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuArt {
    /// The menu's background.
    pub background: Option<ImageRef>,
    /// An item under the pointer (`normal_rollover`).
    pub rollover: Option<ImageRef>,
    /// A check item's mark.
    pub check: Option<ImageRef>,
    /// A check item's unchecked mark (`ghosted`).
    pub unchecked: Option<ImageRef>,
    /// A submenu item's arrow.
    pub arrow: Option<ImageRef>,
}

/// The install's names of the context menus, or the fallback's placeholders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuRefs {
    /// Element IDs.
    pub ids: MenuIds,
    /// Property IDs.
    pub pids: MenuPids,
    /// The `ContextMenu` layout, when it resolves (else the menus are code-made).
    pub layout: Option<Did>,
    /// The fallback menus' art.
    pub art: MenuArt,
}

impl MenuRefs {
    /// No install: placeholder IDs, code-made menus.
    #[must_use]
    pub fn fallback(art: MenuArt) -> Self {
        Self {
            art,
            ..Self::default()
        }
    }

    /// Resolves the names through `layout` (`UILAYOUT` label → DID), `element`
    /// (`UIElementID` label → ID) and `pid` (property name → PID). The layout is kept only
    /// when it is a layout DID (prefix 0x22) and both menu elements resolve.
    #[must_use]
    pub fn resolve(
        layout: &dyn Fn(&str) -> Option<Did>,
        element: &dyn Fn(&str) -> Option<u32>,
        pid: &dyn Fn(&str) -> Option<u32>,
        art: MenuArt,
    ) -> Self {
        let mut ids = MenuIds::default();
        let mut found = 0usize;
        ids.resolve_with(&mut |l| {
            let v = element(l);
            found += usize::from(v.is_some());
            v
        });
        let mut pids = MenuPids::default();
        pids.resolve_with(&mut |n| pid(n));
        let complete = found == ids.entries().len();
        let layout = layout(LAYOUT_LABEL).filter(|d| d.0 >> 24 == 0x22 && complete);
        Self {
            ids,
            pids,
            layout,
            art,
        }
    }
}

/// What a menu tree reports (§19.4; the Tessera rule hands it to the owner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEvent {
    /// An item was selected (`Element_select_context_menu`): its ID, value and position
    /// (item indices from the root), and a check item's new mark (shown or hidden).
    Selected {
        /// The item ID.
        id: u32,
        /// The item value.
        value: i64,
        /// Item indices from the root menu to the item.
        path: Vec<usize>,
        /// A check item's mark after the flip.
        checked: Option<bool>,
    },
    /// An item flagged to report a close was chosen (`Element_close_context_menu`).
    CloseItem {
        /// The item ID.
        id: u32,
        /// The item value.
        value: i64,
    },
    /// The player activated something outside the tree
    /// (`Element_close_deactivated_context_menu`); the tree is closed.
    Dismissed,
}

/// How [`ContextMenu::handle_event`] took an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuReply {
    /// Not an event of the tree.
    Ignored,
    /// An event of the tree with nothing to report (hover, a click on a disabled item, a
    /// submenu opened or closed).
    Consumed,
    /// A result; after [`MenuEvent::Dismissed`], and after a selection unless the request
    /// keeps the tree open, the tree is closed.
    Event(MenuEvent),
}

/// One item's elements.
#[derive(Debug, Clone, Copy)]
struct ItemEls {
    key: ElementKey,
    mark: Option<ElementKey>,
}

/// One open menu of the chain.
#[derive(Debug, Clone)]
struct Level {
    root: ElementKey,
    items: Vec<MenuItem>,
    els: Vec<ItemEls>,
    /// The item of the parent level this submenu belongs to.
    parent_item: Option<usize>,
    open_delay: f64,
    /// The title (a root menu's; empty for none).
    title: String,
    /// A text part (and its text) that could not be measured because its fonts were not
    /// loaded: the menu is sized again once it can be (module docs).
    unmeasured: Option<(ElementKey, String)>,
}

/// A menu's elements before sizing: the menu, its list, the items and their templates.
struct Built {
    root: ElementKey,
    list: Option<ElementKey>,
    els: Vec<ItemEls>,
    templates: Vec<ItemTemplate>,
}

/// The pointer resting on an item: (level, item, since, acted).
#[derive(Debug, Clone, Copy)]
struct Hover {
    level: usize,
    item: usize,
    since: f64,
    acted: bool,
}

/// An open menu tree (module docs).
#[derive(Debug, Clone)]
pub struct ContextMenu {
    refs: MenuRefs,
    request_element: Option<u32>,
    keep_open: bool,
    levels: Vec<Level>,
    hover: Option<Hover>,
    handlers: Vec<(u32, u32)>,
}

/// The template boxes of an item: (item size, text part box relative to the item).
#[derive(Debug, Clone, Copy)]
struct ItemTemplate {
    size: (i32, i32),
    text: Option<IBox>,
}

fn cut_label(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_LABEL_CHARS)
        .collect()
}

/// Checks the caps of a list and its submenus (items per menu, depth, items over the whole
/// tree counted in `total`). Kinds are checked per menu when it is built (§19.2 OPEN rule).
fn check_items(items: &[MenuItem], depth: usize, total: &mut usize) -> Result<(), GameError> {
    if depth >= MAX_DEPTH {
        return Err(GameError::TooLarge {
            what: "context menu depth",
            cap: MAX_DEPTH,
        });
    }
    if items.len() > MAX_ITEMS {
        return Err(GameError::TooLarge {
            what: "context menu items",
            cap: MAX_ITEMS,
        });
    }
    for it in items {
        *total += 1;
        if *total > MAX_TREE_ITEMS {
            return Err(GameError::TooLarge {
                what: "context menu tree items",
                cap: MAX_TREE_ITEMS,
            });
        }
        if !it.submenu.is_empty() {
            check_items(&it.submenu, depth + 1, total)?;
        }
    }
    Ok(())
}

/// Only the list itself (not the submenus) must hold valid kinds before a menu is shown.
fn list_is_valid(items: &[MenuItem]) -> bool {
    items.len() <= MAX_ITEMS && items.iter().all(|i| i.kind.is_valid())
}

/// The unwrapped text extent (content width and height) of `text` in element `k`, or
/// `None` while its fonts are not loaded.
fn text_extent(ui: &mut Ui, k: ElementKey, text: &str) -> Option<(i32, i32)> {
    /// A wrap width no label reaches (labels are capped).
    const UNWRAPPED: i32 = 1 << 20;
    if text.is_empty() {
        return Some((0, 0));
    }
    let props = ui.properties(k);
    let req = TextRequest {
        text,
        props: &props,
        size: (UNWRAPPED, UNWRAPPED),
        wrap_width: Some(UNWRAPPED),
    };
    ui.env.shape_text(&req).map(|p| {
        (
            p.layout.content_width.max(0),
            p.layout.content_height.max(0),
        )
    })
}

/// The box of `k` relative to `ancestor` (sum of the layout boxes in between).
fn box_in(ui: &Ui, ancestor: ElementKey, k: ElementKey) -> Option<IBox> {
    let mut b = ui.layout_box(k)?;
    let mut cur = ui.parent(k);
    let mut n = 0;
    while let Some(c) = cur {
        if c == ancestor {
            return Some(b);
        }
        let r = ui.layout_box(c)?;
        b = b.offset(r.l, r.t);
        cur = ui.parent(c);
        n += 1;
        if n > MAX_TREE_DEPTH {
            break;
        }
    }
    None
}

impl ContextMenu {
    /// Opens a menu tree (§19.2–§19.3): the root menu is built, sized, placed and
    /// activated; submenus are built when opened.
    ///
    /// # Errors
    /// [`GameError::Malformed`] (the value is the item **kind**, not a property ID, although
    /// the variant's message says "property"; `GameError` has no menu-kind variant) when
    /// the root list holds an item of an invalid kind (§19.2 OPEN rule: no menu is shown,
    /// nothing else happens),
    /// [`GameError::TooLarge`] past the caps, or an element or creation error.
    pub fn open(ui: &mut Ui, refs: &MenuRefs, req: &MenuRequest) -> Result<Self, GameError> {
        if let Some((n, i)) = req
            .items
            .iter()
            .enumerate()
            .find(|(_, i)| !i.kind.is_valid())
        {
            tracing::debug!(
                "context menu not shown: item {n} has the invalid kind {}",
                i.kind.raw()
            );
            return Err(GameError::Malformed(i.kind.raw()));
        }
        let mut total = 0usize;
        check_items(&req.items, 0, &mut total)?;
        let mut m = Self {
            refs: refs.clone(),
            request_element: req.element,
            keep_open: req.keep_open,
            levels: Vec::new(),
            hover: None,
            handlers: Vec::new(),
        };
        m.register(ui);
        let title = cut_label(&req.title);
        match m.build_level(ui, &req.items, &title, None) {
            Ok(level) => {
                let root = level.root;
                m.levels.push(level);
                m.place_root(ui, root, req.position);
                ui.request_activation(root);
                Ok(m)
            }
            Err(e) => {
                m.release(ui);
                Err(e)
            }
        }
    }

    /// Whether the tree is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.levels.is_empty()
    }

    /// The root menu's element, while open.
    #[must_use]
    pub fn root(&self) -> Option<ElementKey> {
        self.levels.first().map(|l| l.root)
    }

    /// The menu elements of the open chain, root first.
    #[must_use]
    pub fn menus(&self) -> Vec<ElementKey> {
        self.levels.iter().map(|l| l.root).collect()
    }

    /// The item elements of open menu `level` (0 = the root), in item order.
    #[must_use]
    pub fn item_elements(&self, level: usize) -> Vec<ElementKey> {
        self.levels
            .get(level)
            .map(|l| l.els.iter().map(|e| e.key).collect())
            .unwrap_or_default()
    }

    /// The items of open menu `level`.
    #[must_use]
    pub fn items(&self, level: usize) -> &[MenuItem] {
        self.levels.get(level).map_or(&[], |l| l.items.as_slice())
    }

    /// The check mark element of item `item` of open menu `level`, if it has one.
    #[must_use]
    pub fn mark_element(&self, level: usize, item: usize) -> Option<ElementKey> {
        self.levels.get(level)?.els.get(item)?.mark
    }

    /// Whether `k` is an element of an open menu of the tree.
    #[must_use]
    pub fn contains(&self, ui: &Ui, k: ElementKey) -> bool {
        self.levels
            .iter()
            .any(|l| ui.is_ancestor_or_self(l.root, k))
    }

    /// Sets a check item's mark (the owner's model changed, for example the filter menu's
    /// *all* and *none*): shown and checked as `on`.
    pub fn set_check(&mut self, ui: &mut Ui, level: usize, item: usize, on: bool) {
        let Some(l) = self.levels.get_mut(level) else {
            return;
        };
        let (Some(it), Some(els)) = (l.items.get_mut(item), l.els.get(item).copied()) else {
            return;
        };
        if it.kind != ItemKind::Check {
            return;
        }
        it.checked = on;
        it.show_check = on;
        if let Some(mark) = els.mark {
            show_mark(ui, mark, it.show_check, it.checked);
        }
    }

    fn register(&mut self, ui: &mut Ui) {
        let ms = ui.vocab().msgs;
        let ids = self.refs.ids;
        for msg in [
            ms.mouse_click,
            ms.button_click,
            ms.mouse_dbl_click,
            ms.mouse_over_top,
        ] {
            self.on(ui, ITEM_ID, msg);
        }
        let mut menu_ids = vec![ids.element, ids.with_title, ids.map_menu];
        menu_ids.extend(self.request_element);
        menu_ids.sort_unstable();
        menu_ids.dedup();
        for id in menu_ids {
            self.on(ui, id, ms.deactivated);
        }
    }

    fn on(&mut self, ui: &mut Ui, id: u32, msg: u32) {
        if id != 0 && !self.handlers.contains(&(id, msg)) {
            ui.register_handler(id, msg);
            self.handlers.push((id, msg));
        }
    }

    fn release(&mut self, ui: &mut Ui) {
        for (id, msg) in self.handlers.drain(..) {
            ui.unregister_handler(id, msg);
        }
    }

    /// Closes the whole tree (every menu, its handlers).
    pub fn close(&mut self, ui: &mut Ui) {
        self.truncate(ui, 0);
        self.hover = None;
        self.release(ui);
    }

    /// Closes the menus from `level` on. When one of them is the active panel and a parent
    /// menu stays open, the parent is activated first (module docs): destroying the active
    /// panel deactivates nothing, so without it a later press outside the tree would not
    /// close it.
    fn truncate(&mut self, ui: &mut Ui, level: usize) {
        if level > 0 {
            let closing_active = ui.active_panel().is_some_and(|a| {
                self.levels
                    .get(level..)
                    .is_some_and(|ls| ls.iter().any(|l| l.root == a))
            });
            if closing_active && let Some(parent) = self.levels.get(level - 1) {
                let p = parent.root;
                ui.request_activation(p);
            }
        }
        while self.levels.len() > level {
            if let Some(l) = self.levels.pop()
                && ui.exists(l.root)
            {
                ui.destroy(l.root);
            }
        }
        if self.hover.is_some_and(|h| h.level >= level) {
            self.hover = None;
        }
    }

    /// The menu element ID for a menu with or without a title.
    fn menu_element(&self, titled: bool) -> u32 {
        let ids = self.refs.ids;
        self.request_element
            .unwrap_or(if titled { ids.with_title } else { ids.element })
    }

    /// Builds one menu (not placed): the element, the items, the sizes and scale.
    fn build_level(
        &mut self,
        ui: &mut Ui,
        items: &[MenuItem],
        title: &str,
        parent_item: Option<usize>,
    ) -> Result<Level, GameError> {
        let depth = self.levels.len();
        let titled = !title.is_empty();
        let menu_id = self.menu_element(titled);
        let built = match self.refs.layout {
            Some(layout) => self.build_install(ui, layout, menu_id, items),
            None => Err(GameError::NoLayout("ContextMenu")),
        };
        let Built {
            root,
            list,
            els,
            templates,
        } = match built {
            Ok(b) => b,
            Err(e) => {
                if self.refs.layout.is_some() {
                    tracing::debug!("context menu: install layout not used ({e}); fallback");
                }
                self.build_fallback(ui, menu_id, titled, items)?
            }
        };
        let mut unmeasured: Option<(ElementKey, String)> = None;
        let p = self.refs.pids;
        let st = ui.vocab().states;
        let pids = ui.vocab().pids;
        ui.set_property(root, pids.blocks_clicks, PropertyValue::Bool(true));
        ui.set_property(root, pids.activatable, PropertyValue::Bool(true));
        let lvl = i32::try_from(depth).unwrap_or(i32::MAX);
        ui.set_property(root, p.level, PropertyValue::Int(lvl));
        let delay = ui.prop(root, p.open_delay);
        let open_delay = if delay.is_set() {
            f64::from(delay.float()).clamp(0.0, 60.0)
        } else {
            FALLBACK_OPEN_DELAY
        };
        // Title text.
        let ids = self.refs.ids;
        let title_field = crate::game::find_below(ui, root, ids.title);
        let title_text = title_field.and_then(|t| ui.find_descendant(t, ids.title_text));
        let mut title_w = 0;
        let mut title_text_w = 0;
        if let (Some(tf), Some(tt)) = (title_field, title_text) {
            if titled {
                ui.set_text(tt, Some(title.to_string()));
                let tf_box = ui.layout_box(tf).unwrap_or_default();
                let tt_box = ui.layout_box(tt).unwrap_or_default();
                let ext = text_extent(ui, tt, title);
                if ext.is_none() && unmeasured.is_none() {
                    unmeasured = Some((tt, title.to_string()));
                }
                let (tw, _) = ext.unwrap_or((tt_box.w(), tt_box.h()));
                title_text_w = tw;
                title_w = tw.saturating_add(tf_box.w().saturating_sub(tt_box.w()));
            } else {
                ui.set_shown(tf, false);
            }
        }
        // Items: fill, measure.
        let mut sizes = Vec::with_capacity(items.len());
        for (i, (it, el)) in items.iter().zip(&els).enumerate() {
            let t = templates.get(i).copied().unwrap_or(ItemTemplate {
                size: (0, 0),
                text: None,
            });
            let k = el.key;
            let label = cut_label(&it.label);
            let text = crate::game::find_below(ui, k, ids.text);
            if let Some(tp) = text {
                ui.set_text(tp, Some(label.clone()));
            }
            if let Some(ic) = crate::game::find_below(ui, k, ids.icon) {
                match &it.icon {
                    Some(img) => crate::game::set_image_ref(ui, ic, img, 4),
                    None => crate::game::set_image(ui, ic, 0, 0),
                }
            }
            let index = i32::try_from(i).unwrap_or(i32::MAX);
            ui.set_property(k, p.level, PropertyValue::Int(lvl));
            ui.set_property(k, p.index, PropertyValue::Int(index));
            ui.switch_state(k, if it.enabled { st.normal } else { st.ghosted });
            if let Some(mark) = el.mark {
                show_mark(ui, mark, it.show_check, it.checked);
            }
            let tb = t.text.unwrap_or_default();
            let ext = text.map(|tp| (tp, text_extent(ui, tp, &label)));
            if let Some((tp, None)) = ext
                && unmeasured.is_none()
            {
                unmeasured = Some((tp, label.clone()));
            }
            let (tw, th) = ext.and_then(|e| e.1).unwrap_or((tb.w(), tb.h()));
            let w = tw
                .saturating_add(TEXT_PAD)
                .saturating_add(t.size.0.saturating_sub(tb.w()));
            let h = th.saturating_add(t.size.1.saturating_sub(tb.h()));
            sizes.push((w.max(1), h.max(1), tb, t.size));
        }
        // Common width (§19.3).
        let common = sizes
            .iter()
            .map(|s| s.0)
            .max()
            .unwrap_or(0)
            .max(title_w)
            .max(1);
        let mut total_h = 0i32;
        for ((w_h, el), it) in sizes.iter().zip(&els).zip(items) {
            let (_, h, tb, tsize) = *w_h;
            ui.set_element_box(el.key, 0, 0, common, h);
            if let Some(tp) = crate::game::find_below(ui, el.key, ids.text) {
                let tw = common.saturating_sub(tsize.0.saturating_sub(tb.w()));
                let th = h.saturating_sub(tsize.1.saturating_sub(tb.h()));
                ui.set_element_box(tp, tb.l, tb.t, tw.max(0), th.max(0));
            }
            // A mark or an arrow keeps its distance to the right edge (edges of §19.1).
            let right_part = match it.kind {
                ItemKind::Check => el.mark,
                ItemKind::Submenu => crate::game::find_below(ui, el.key, ids.open_arrow),
                _ => None,
            };
            if let Some(rp) = right_part
                && let Some(b) = ui.layout_box(rp)
            {
                let from_right = tsize.0.saturating_sub(b.l);
                ui.set_element_box(rp, common.saturating_sub(from_right), b.t, b.w(), b.h());
            }
            total_h = total_h.saturating_add(h);
        }
        // Menu, then list and title (the menu's resize lays its children out again by
        // their edges, so the list box is set after it).
        let (sw, sh) = ui.screen_size();
        let cap = crate::geom::trunc_f32(sh.max(0) as f32 * LIST_HEIGHT_FRACTION);
        let list_h = total_h.min(cap.max(1));
        let menu_box = ui.layout_box(root).unwrap_or_default();
        let list_box = list.and_then(|l| box_in(ui, root, l).map(|b| (l, b)));
        let (mw, mh) = match list_box {
            Some((_, lb)) => {
                let mr = menu_box
                    .w()
                    .saturating_sub(lb.l.saturating_add(lb.w()))
                    .max(0);
                let mb = menu_box
                    .h()
                    .saturating_sub(lb.t.saturating_add(lb.h()))
                    .max(0);
                (
                    common.saturating_add(lb.l.max(0)).saturating_add(mr),
                    list_h.saturating_add(lb.t.max(0)).saturating_add(mb),
                )
            }
            None => (common, list_h),
        };
        ui.set_element_box(root, menu_box.l, menu_box.t, mw.max(1), mh.max(1));
        if let Some((l, lb)) = list_box {
            let offset = box_in(ui, root, l)
                .zip(ui.layout_box(l))
                .map_or((0, 0), |(abs, own)| (abs.l - own.l, abs.t - own.t));
            ui.set_element_box(
                l,
                lb.l.saturating_sub(offset.0),
                lb.t.saturating_sub(offset.1),
                common,
                list_h,
            );
        }
        if let (Some(tf), true) = (title_field, titled)
            && let Some(b) = ui.layout_box(tf)
        {
            ui.set_element_box(tf, b.l, b.t, title_w.max(1), b.h());
            if let Some(tt) = title_text
                && let Some(tb) = ui.layout_box(tt)
            {
                ui.set_element_box(tt, tb.l, tb.t, title_text_w.max(1), tb.h());
            }
        }
        if let Some(bg) = crate::game::find_below(ui, root, FALLBACK_BACKGROUND_ID) {
            ui.set_element_box(bg, 0, 0, mw.max(1), mh.max(1));
        }
        // Items into the list (laid out once).
        if let Some(l) = list {
            let keys: Vec<ElementKey> = els.iter().map(|e| e.key).collect();
            if let Err((added, e)) = ui.list_add_many(l, &keys) {
                tracing::debug!("context menu: {added} items added, then {e}");
            }
        }
        // Scale to fit (§19.3).
        if mw > sw || mh > sh {
            let f = (sw.max(1) as f32 / mw.max(1) as f32).min(sh.max(1) as f32 / mh.max(1) as f32);
            if f.is_finite() && f > 0.0 && f < 1.0 {
                let (ex, ey, _) = ui.node_scale_at_or_above(root);
                let own = ex.min(ey).min(1.0);
                let s = (own * f).min(own);
                ui.set_node_scale(root, Some([s, s, 1.0]));
            }
        }
        Ok(Level {
            root,
            items: items.to_vec(),
            els,
            parent_item,
            open_delay,
            title: title.to_string(),
            unmeasured,
        })
    }

    /// The install path: the menu element as a root panel, the items from their templates.
    fn build_install(
        &mut self,
        ui: &mut Ui,
        layout: Did,
        menu_id: u32,
        items: &[MenuItem],
    ) -> Result<Built, GameError> {
        let ids = self.refs.ids;
        let root = ui.open_panel(layout, menu_id)?;
        let Some(list) = ui
            .find_descendant(root, ids.list)
            .filter(|l| ui.class(*l).is_some_and(WidgetClass::is_list))
        else {
            ui.destroy(root);
            return Err(GameError::MissingElement(ids.list));
        };
        let mut els = Vec::with_capacity(items.len());
        let mut templates = Vec::with_capacity(items.len());
        for it in items {
            let tmpl = match it.kind {
                ItemKind::Check => ids.check_item,
                ItemKind::Submenu => ids.sub_item,
                _ => ids.entry,
            };
            let k = match ui.create_element(None, layout, tmpl) {
                Ok(k) => k,
                Err(e) => {
                    for el in &els {
                        let el: &ItemEls = el;
                        ui.destroy(el.key);
                    }
                    ui.destroy(root);
                    return Err(e.into());
                }
            };
            let size = ui.layout_box(k).map_or((0, 0), |b| (b.w(), b.h()));
            let text = crate::game::find_below(ui, k, ids.text).and_then(|t| box_in(ui, k, t));
            let mark = (it.kind == ItemKind::Check)
                .then(|| crate::game::find_below(ui, k, ids.flag))
                .flatten();
            tag_item(ui, k);
            els.push(ItemEls { key: k, mark });
            templates.push(ItemTemplate { size, text });
        }
        Ok(Built {
            root,
            list: Some(list),
            els,
            templates,
        })
    }

    /// The fallback path: code-made elements with the §19.1 boxes and Tessera's art.
    fn build_fallback(
        &mut self,
        ui: &mut Ui,
        menu_id: u32,
        titled: bool,
        items: &[MenuItem],
    ) -> Result<Built, GameError> {
        let ids = self.refs.ids;
        let p = self.refs.pids;
        let art = self.refs.art.clone();
        let st = ui.vocab().states;
        let cp = p.text_font_color;
        let (list_y, list_h) = if titled { (16, 14) } else { (2, 26) };
        let mut root = FNode::field(menu_id, 0, 0, 30, 30)
            .prop(p.open_delay, PropertyValue::Float(FALLBACK_OPEN_DELAY_F32))
            .child(FNode::field(FALLBACK_BACKGROUND_ID, 0, 0, 30, 30).image(&art.background));
        if titled {
            root = root.child(FNode::field(ids.title, 0, 0, 30, 16).child(FNode::text(
                ids.title_text,
                4,
                2,
                22,
                12,
                cp,
                colours::TITLE,
                "",
            )));
        }
        root = root.child(FNode::new(
            WidgetClass::ListBox,
            ids.list,
            2,
            list_y,
            26,
            list_h,
        ));
        let root = open_root(ui, &root)?;
        let list = ui.find_descendant(root, ids.list);
        let mut els = Vec::with_capacity(items.len());
        let mut templates = Vec::with_capacity(items.len());
        for it in items {
            let (tmpl, w, h) = match it.kind {
                ItemKind::Check => (ids.check_item, 44, 18),
                ItemKind::Submenu => (ids.sub_item, 44, 18),
                _ => (ids.entry, 28, 16),
            };
            let mut node = FNode::field(tmpl, 0, 0, w, h)
                .state(st.normal, None, Vec::new())
                .state(st.normal_rollover, art.rollover.as_ref(), Vec::new())
                .state(st.ghosted, None, Vec::new())
                .child(FNode::field(ids.icon, 4, 0, 16, 16))
                .child(
                    FNode::text(ids.text, 22, 2, 6, 12, cp, colours::WHITE, "")
                        .state(st.normal, None, Vec::new())
                        .state(st.normal_rollover, None, Vec::new())
                        .state(
                            st.ghosted,
                            None,
                            vec![(cp, crate::game::npc::kit::colour_value(colours::GHOSTED))],
                        ),
                );
            match it.kind {
                ItemKind::Check => {
                    node = node.child(
                        FNode::field(ids.flag, w - 18, 0, 16, 16)
                            .image(&art.check)
                            .state(st.ghosted, art.unchecked.as_ref(), Vec::new()),
                    );
                }
                ItemKind::Submenu => {
                    node = node
                        .child(FNode::field(ids.open_arrow, w - 18, 0, 16, 16).image(&art.arrow));
                }
                _ => {}
            }
            let host = list.unwrap_or(root);
            let Some(k) = instantiate(ui, host, &node) else {
                ui.destroy(root);
                return Err(crate::UiError::TooManyElements.into());
            };
            let mark = (it.kind == ItemKind::Check)
                .then(|| crate::game::find_below(ui, k, ids.flag))
                .flatten();
            tag_item(ui, k);
            els.push(ItemEls { key: k, mark });
            templates.push(ItemTemplate {
                size: (w, h),
                text: Some(IBox::from_xywh(22, 2, 6, 12)),
            });
        }
        Ok(Built {
            root,
            list,
            els,
            templates,
        })
    }

    /// Places a root menu (§19.3 "Placement"): the given position, else the pointer, then
    /// the least move onto the screen.
    fn place_root(&self, ui: &mut Ui, root: ElementKey, pos: Option<MenuPosition>) {
        let d = ui.displayed_box(root);
        let (x, y) = match pos {
            Some(MenuPosition::TopLeft(x, y)) => (x, y),
            Some(MenuPosition::BottomLeft(x, y)) => (x, y.saturating_sub(d.h())),
            None => ui.pointer().unwrap_or((0, 0)),
        };
        move_on_screen(ui, root, x, y);
        to_front(ui, root);
    }

    /// Opens the submenu of item `item` of open menu `level` (closing deeper menus first).
    /// Returns whether it opened.
    fn open_submenu(&mut self, ui: &mut Ui, level: usize, item: usize) -> bool {
        self.truncate(ui, level + 1);
        let Some(l) = self.levels.get(level) else {
            return false;
        };
        let (Some(it), Some(el)) = (l.items.get(item).cloned(), l.els.get(item).copied()) else {
            return false;
        };
        if it.kind != ItemKind::Submenu || !list_is_valid(&it.submenu) {
            return false;
        }
        if self.levels.len() >= MAX_DEPTH {
            return false;
        }
        let anchor = ui.displayed_box(el.key);
        match self.build_level(ui, &it.submenu, "", Some(item)) {
            Ok(sub) => {
                let root = sub.root;
                self.levels.push(sub);
                let (sw, _) = ui.screen_size();
                let d = ui.displayed_box(root);
                let right = anchor.l.saturating_add(anchor.w());
                let x = if right.saturating_add(d.w()) > sw {
                    anchor.l.saturating_sub(d.w())
                } else {
                    right
                };
                move_on_screen(ui, root, x, anchor.t);
                to_front(ui, root);
                true
            }
            Err(e) => {
                tracing::debug!("context menu: submenu not opened: {e}");
                false
            }
        }
    }

    /// The (level, item) an element is or lies below.
    fn item_of(&self, ui: &Ui, k: ElementKey) -> Option<(usize, usize)> {
        for (li, l) in self.levels.iter().enumerate() {
            if !ui.is_ancestor_or_self(l.root, k) {
                continue;
            }
            let mut cur = Some(k);
            let mut n = 0;
            while let Some(c) = cur {
                if let Some(i) = l.els.iter().position(|e| e.key == c) {
                    return Some((li, i));
                }
                if c == l.root || n > MAX_TREE_DEPTH {
                    break;
                }
                n += 1;
                cur = ui.parent(c);
            }
            return None;
        }
        None
    }

    fn level_of_root(&self, k: ElementKey) -> Option<usize> {
        self.levels.iter().position(|l| l.root == k)
    }

    fn set_item_state(&self, ui: &mut Ui, level: usize, item: usize, over: bool) {
        let st = ui.vocab().states;
        let Some(l) = self.levels.get(level) else {
            return;
        };
        let (Some(it), Some(el)) = (l.items.get(item), l.els.get(item)) else {
            return;
        };
        let s = if !it.enabled {
            st.ghosted
        } else if over {
            st.normal_rollover
        } else {
            st.normal
        };
        ui.switch_state(el.key, s);
        if let Some(mark) = el.mark {
            show_mark(ui, mark, it.show_check, it.checked);
        }
    }

    /// Handles one UI event (§19.4): hover, clicks and deactivation.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> MenuReply {
        if self.levels.is_empty() {
            return MenuReply::Ignored;
        }
        let UiEvent::Game { message } = ev else {
            return MenuReply::Ignored;
        };
        let ms = ui.vocab().msgs;
        let src = message.source;
        if message.msg == ms.deactivated {
            if self.level_of_root(src).is_none() {
                return MenuReply::Ignored;
            }
            let inside = ui
                .active_panel()
                .is_some_and(|a| self.levels.iter().any(|l| l.root == a));
            if inside {
                return MenuReply::Consumed;
            }
            self.close(ui);
            return MenuReply::Event(MenuEvent::Dismissed);
        }
        if !ui.exists(src) {
            return MenuReply::Ignored;
        }
        let Some((level, item)) = self.item_of(ui, src) else {
            return if self.contains(ui, src) {
                MenuReply::Consumed
            } else {
                MenuReply::Ignored
            };
        };
        if message.msg == ms.mouse_over_top {
            let over = message.arg.value != 0;
            self.set_item_state(ui, level, item, over);
            if over {
                self.hover = Some(Hover {
                    level,
                    item,
                    since: ui.time(),
                    acted: false,
                });
            } else if self
                .hover
                .is_some_and(|h| h.level == level && h.item == item)
            {
                self.hover = None;
            }
            return MenuReply::Consumed;
        }
        let Some((_, kind)) = crate::game::popup::click(ui, ev) else {
            return MenuReply::Consumed;
        };
        use crate::game::npc::kit::ClickKind;
        if !matches!(
            kind,
            ClickKind::Left | ClickKind::Button | ClickKind::Double
        ) {
            return MenuReply::Consumed;
        }
        self.click(ui, level, item)
    }

    /// A click on item `item` of open menu `level` (§19.4 step 3).
    pub fn click(&mut self, ui: &mut Ui, level: usize, item: usize) -> MenuReply {
        let Some(it) = self
            .levels
            .get(level)
            .and_then(|l| l.items.get(item))
            .cloned()
        else {
            return MenuReply::Ignored;
        };
        if !it.enabled {
            return MenuReply::Consumed;
        }
        // An open submenu of another item is closed first.
        let open_child = self.levels.get(level + 1).and_then(|l| l.parent_item);
        if open_child.is_some_and(|c| c != item) {
            self.truncate(ui, level + 1);
        }
        if it.kind == ItemKind::Submenu {
            if open_child == Some(item) {
                self.truncate(ui, level + 1);
            } else {
                self.open_submenu(ui, level, item);
            }
            return MenuReply::Consumed;
        }
        let mut path: Vec<usize> = Vec::with_capacity(level + 1);
        for l in 1..=level {
            path.push(self.levels.get(l).and_then(|x| x.parent_item).unwrap_or(0));
        }
        path.push(item);
        let event = if it.reports_close {
            MenuEvent::CloseItem {
                id: it.id,
                value: it.value,
            }
        } else {
            let checked = if it.kind == ItemKind::Check {
                let on = !it.show_check;
                self.set_check(ui, level, item, on);
                Some(on)
            } else {
                None
            };
            MenuEvent::Selected {
                id: it.id,
                value: it.value,
                path,
                checked,
            }
        };
        if !(self.keep_open && matches!(event, MenuEvent::Selected { .. })) {
            self.close(ui);
        }
        MenuReply::Event(event)
    }

    /// The per-frame work (§19.4 step 2): when the pointer has rested on an item longer
    /// than its menu's open delay, a submenu item opens its submenu and any other item
    /// closes its menu's open submenu (once per rest).
    pub fn update(&mut self, ui: &mut Ui) {
        self.refit_pending(ui);
        let Some(h) = self.hover else {
            return;
        };
        if h.acted {
            return;
        }
        let Some(l) = self.levels.get(h.level) else {
            self.hover = None;
            return;
        };
        if ui.time() - h.since <= l.open_delay {
            return;
        }
        let Some(it) = l.items.get(h.item).cloned() else {
            self.hover = None;
            return;
        };
        if let Some(hv) = self.hover.as_mut() {
            hv.acted = true;
        }
        let open_child = self.levels.get(h.level + 1).and_then(|x| x.parent_item);
        if it.kind == ItemKind::Submenu && it.enabled {
            if open_child != Some(h.item) {
                self.open_submenu(ui, h.level, h.item);
            }
        } else if open_child.is_some() {
            self.truncate(ui, h.level + 1);
        }
    }
}

impl ContextMenu {
    /// Builds and sizes again the first menu measured without its fonts, once its text can
    /// be measured (module docs, "Fonts not loaded yet").
    fn refit_pending(&mut self, ui: &mut Ui) {
        let Some(li) = self.levels.iter().position(|l| l.unmeasured.is_some()) else {
            return;
        };
        let ready = self
            .levels
            .get(li)
            .and_then(|l| l.unmeasured.clone())
            .is_some_and(|(k, t)| !ui.exists(k) || text_extent(ui, k, &t).is_some());
        if !ready {
            return;
        }
        if li == 0 {
            self.refit_root(ui);
        } else if let Some(item) = self.levels.get(li).and_then(|l| l.parent_item)
            && !self.open_submenu(ui, li - 1, item)
        {
            tracing::debug!("context menu: submenu not rebuilt after its fonts arrived");
        }
    }

    /// Builds the root menu again at its place (its open submenu closes).
    fn refit_root(&mut self, ui: &mut Ui) {
        let Some(l) = self.levels.first() else {
            return;
        };
        let (items, title, old_root) = (l.items.clone(), l.title.clone(), l.root);
        let was_active = ui
            .active_panel()
            .is_some_and(|a| self.levels.iter().any(|x| x.root == a));
        let place = ui.displayed_box(old_root);
        let old = std::mem::take(&mut self.levels);
        match self.build_level(ui, &items, &title, None) {
            Ok(level) => {
                for l in old.into_iter().rev() {
                    if ui.exists(l.root) {
                        ui.destroy(l.root);
                    }
                }
                self.hover = None;
                let root = level.root;
                self.levels.push(level);
                move_on_screen(ui, root, place.l, place.t);
                to_front(ui, root);
                if was_active {
                    ui.request_activation(root);
                }
            }
            Err(e) => {
                tracing::debug!("context menu: not rebuilt after its fonts arrived: {e}");
                self.levels = old;
                if let Some(l) = self.levels.first_mut() {
                    l.unmeasured = None;
                }
            }
        }
    }
}

/// Shows or hides a check mark and sets its state (§19.3 "Check marks"): `normal_activated`
/// when checked (the template defines no such state, so the mark shows its own block), else
/// `ghosted`.
fn show_mark(ui: &mut Ui, mark: ElementKey, shown: bool, checked: bool) {
    let st = ui.vocab().states;
    ui.set_shown(mark, shown);
    ui.switch_state(
        mark,
        if checked {
            st.normal_activated
        } else {
            st.ghosted
        },
    );
}

/// Moves root `k` so that its displayed top-left corner is at (`x`, `y`), then the least
/// distance that puts it wholly on the screen (a menu larger than the screen ends at the
/// top-left corner).
fn move_on_screen(ui: &mut Ui, k: ElementKey, x: i32, y: i32) {
    let (sw, sh) = ui.screen_size();
    let d = ui.displayed_box(k);
    let x = x.min(sw.saturating_sub(d.w())).max(0);
    let y = y.min(sh.saturating_sub(d.h())).max(0);
    let Some(lb) = ui.layout_box(k) else {
        return;
    };
    let (dx, dy) = (x.saturating_sub(d.l), y.saturating_sub(d.t));
    ui.move_to(k, lb.l.saturating_add(dx), lb.t.saturating_add(dy));
}

/// Puts root `k` in front of every other root panel: a z-level below the lowest of the
/// screen root's children (a lower z is in front, `ui-layout.md` §9.3), last in the list.
fn to_front(ui: &mut Ui, k: ElementKey) {
    let z = crate::game::popup::front_z(ui);
    ui.set_z(k, z);
    crate::game::move_to_front(ui, k);
}

/// Gives item `k` Tessera's item ID ([`ITEM_ID`]) and makes it, and only it, mouse-visible.
fn tag_item(ui: &mut Ui, k: ElementKey) {
    for c in crate::game::subtree(ui, k).into_iter().skip(1) {
        ui.set_mouse_visible_flag(c, false);
    }
    if let Some(e) = ui.el_mut(k) {
        e.element_id = ITEM_ID;
    }
    ui.set_mouse_visible_flag(k, true);
}
