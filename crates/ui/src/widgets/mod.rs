// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine widget classes (`ui-widgets.md`): the class an element is created with, its
//! derivation (§3.2) and the per-class state.
//!
//! A class adds to the class it derives from (§3.2), so an element keeps one optional
//! state part per class in its chain: a Scrollbar has Button and Scrollable parts, a Tree has
//! Scrollable, ListBox and Tree parts. The behaviour lives in the submodules as `impl Ui`
//! blocks.

pub(crate) mod button;
pub mod groupbox;
pub(crate) mod listbox;
pub(crate) mod menu;
pub mod meter;
pub(crate) mod panel;
pub mod scroll;

use crate::tree::ElementKey;

/// The class an element is created with (`ui-widgets.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetClass {
    /// Type 14: the base element's behaviour only.
    Field,
    /// Type 1 (§3.4).
    GroupBox,
    /// Type 2 (§3.5; Tessera: the base element).
    ColorPicker,
    /// Type 4 (§11).
    Menu,
    /// Type 5 (§6).
    Scrollbar,
    /// Type 7 (§8).
    ListBox,
    /// Type 8 (§10).
    Panel,
    /// Type 9 (`ui-layout.md` §9.7, §7).
    Button,
    /// Type 10 (§9).
    Tree,
    /// Type 11 (§12).
    Viewport,
    /// Type 12 (§3.5; Tessera: the base element, no web content).
    Browser,
    /// Type 15 (`ui-behaviour.md` §8.1).
    Dragbar,
    /// Type 16 (`fonts.md`).
    Text,
    /// Type 17 (§4).
    Meter,
    /// Type 18 (`ui-behaviour.md` §8.2).
    ResizeBar,
    /// A game type (0x1000xxxx) without a registered widget: the Tessera fallback, a plain
    /// Field that draws its images and text (`ui-layout.md` §9.2 note).
    Fallback(u32),
}

impl WidgetClass {
    /// Whether the class derives from Button (Button, Menu, Scrollbar; §3.2).
    #[must_use]
    pub fn is_button(self) -> bool {
        matches!(self, Self::Button | Self::Menu | Self::Scrollbar)
    }

    /// Whether the class derives from Scrollable (Text and its descendants, ListBox, Tree).
    #[must_use]
    pub fn is_scrollable(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Button | Self::Menu | Self::Scrollbar | Self::ListBox | Self::Tree
        )
    }

    /// Whether the class derives from Text (Text, Button, Menu, Scrollbar).
    #[must_use]
    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Button | Self::Menu | Self::Scrollbar
        )
    }

    /// Whether the class is in the list-box family (ListBox, Tree).
    #[must_use]
    pub fn is_list(self) -> bool {
        matches!(self, Self::ListBox | Self::Tree)
    }
}

/// Button part (`ui-layout.md` §9.7, `ui-widgets.md` §7.1).
#[derive(Debug, Clone, Default)]
pub(crate) struct ButtonPart {
    /// A select command went down on the button and is held (*P*).
    pub pressed_by: Option<std::sync::Arc<str>>,
    /// The pointer is over the button (*O*).
    pub over: bool,
    /// Auto-repeat running.
    pub repeating: bool,
    /// Next auto-repeat time.
    pub next: f64,
}

/// Scrollable part (`ui-widgets.md` §5).
#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollPart {
    /// Content size.
    pub content: (i32, i32),
    /// Scroll offsets.
    pub offset: (i32, i32),
    /// The linked vertical scroll bar.
    pub vbar: Option<ElementKey>,
    /// The linked horizontal scroll bar.
    pub hbar: Option<ElementKey>,
    /// Hovered elements the `ScrollableControls_Mouse` map was pushed for (§5.7).
    pub hover_pushes: Vec<ElementKey>,
}

/// Scroll-bar part (`ui-widgets.md` §6).
#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollbarPart {
    /// Disabled mark (§5.2 step 2, `UICore_Scrollbar_disabled`).
    pub dragging: bool,
    /// Press point on the bar's axis, bar coordinates.
    pub press: (i32, i32),
    /// Position at the drag start.
    pub drag_start: f32,
    /// Buttons still to adopt (§6.2), by element ID.
    pub adopt_pending: bool,
    /// Whether the hover push is active.
    pub pushed: bool,
    /// Smooth movement: (start, goal, t0, t1).
    pub anim: Option<(f32, f32, f64, f64)>,
    /// Thumb position on the axis and length (§6.6).
    pub thumb_span: (i32, i32),
}

/// List-box part (`ui-widgets.md` §8).
#[derive(Debug, Clone, Default)]
pub(crate) struct ListPart {
    /// Entry list in insertion order.
    pub entries: Vec<ElementKey>,
    /// Laid-out list of the last layout.
    pub laid_out: Vec<ElementKey>,
    /// Selected entry.
    pub selected: Option<ElementKey>,
    /// Layout dirty.
    pub dirty: bool,
    /// Columns and rows of the last layout.
    pub grid: (i32, i32),
}

/// One tree node (`ui-widgets.md` §9.2).
#[derive(Debug, Clone)]
pub(crate) struct TreeNode {
    /// The node's entry.
    pub entry: ElementKey,
    /// The parent node, named by its entry (stable across node removals), `None` for
    /// top-level nodes.
    pub parent: Option<ElementKey>,
    /// Expanded.
    pub expanded: bool,
    /// Icon part.
    pub icon: Option<ElementKey>,
    /// Indent part inside the entry.
    pub indent: Option<ElementKey>,
}

/// Tree part (`ui-widgets.md` §9).
#[derive(Debug, Clone, Default)]
pub(crate) struct TreePart {
    /// Nodes.
    pub nodes: Vec<TreeNode>,
}

/// Meter part (`ui-widgets.md` §4).
#[derive(Debug, Clone, Default)]
pub(crate) struct MeterPart {
    /// Last position that sent a message (§4.6 step 2).
    pub last: f32,
    /// Frame index shown last (§4.4), signed as computed (it can be negative or past the
    /// end of the array).
    pub frame: Option<i32>,
    /// Fill part.
    pub fill: Option<ElementKey>,
    /// Fill image part.
    pub fill_image: Option<ElementKey>,
    /// Smooth movement: (start, goal, t0, t1).
    pub anim: Option<(f32, f32, f64, f64)>,
    /// Alpha-test mode.
    pub alpha_test: bool,
}

/// Panel part (`ui-widgets.md` §10).
#[derive(Debug, Clone, Default)]
pub(crate) struct PanelPart {
    /// Tab → page map, in insertion order.
    pub tab_page: Vec<(u32, u32)>,
    /// Page → tab map.
    pub page_tab: Vec<(u32, u32)>,
    /// Current (page, tab).
    pub current: Option<(u32, u32)>,
}

/// Menu part (`ui-widgets.md` §11).
#[derive(Debug, Clone, Default)]
pub(crate) struct MenuPart {
    /// The popup.
    pub popup: Option<ElementKey>,
    /// The list box in the popup.
    pub list: Option<ElementKey>,
    /// Open mark.
    pub open: bool,
}

/// All class parts of an element.
#[derive(Debug, Clone, Default)]
pub(crate) struct Widget {
    pub button: Option<ButtonPart>,
    pub scroll: Option<ScrollPart>,
    pub scrollbar: Option<ScrollbarPart>,
    pub list: Option<ListPart>,
    pub tree: Option<TreePart>,
    pub meter: Option<MeterPart>,
    pub panel: Option<PanelPart>,
    pub menu: Option<MenuPart>,
    /// GroupBox recorded selection (§3.4).
    pub group_selection: Option<u32>,
    /// Drag bar / resize bar: whether its target is moving or resizing.
    pub bar_active: bool,
}

impl Widget {
    /// The parts of a class (`ui-widgets.md` §3.2).
    pub fn for_class(class: WidgetClass) -> Self {
        let mut w = Self::default();
        if class.is_scrollable() {
            w.scroll = Some(ScrollPart::default());
        }
        if class.is_button() {
            w.button = Some(ButtonPart::default());
        }
        match class {
            WidgetClass::Scrollbar => w.scrollbar = Some(ScrollbarPart::default()),
            WidgetClass::ListBox => w.list = Some(ListPart::default()),
            WidgetClass::Tree => {
                w.list = Some(ListPart::default());
                w.tree = Some(TreePart::default());
            }
            WidgetClass::Meter => w.meter = Some(MeterPart::default()),
            WidgetClass::Panel => w.panel = Some(PanelPart::default()),
            WidgetClass::Menu => w.menu = Some(MenuPart::default()),
            WidgetClass::GroupBox => w.group_selection = Some(0),
            _ => {}
        }
        w
    }
}
