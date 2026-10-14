// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared parts of the P3 widgets: the fallback art, a small builder for code-made element
//! trees (the fallback widgets), game-code handler bookkeeping, click classification, part
//! finders and text bounds.
//!
//! **Fallback widgets** (Tessera rule): when a window's layout is not available (no
//! install, or a golden image of decision 0031 tier 1), the window is built from code-made
//! elements that carry the **same element IDs** as the install's elements
//! ([`super::super::NpcVocab`]), with states whose media and text colours come from
//! [`FallbackArt`] and Tessera's own colours. The controllers then find and fill both kinds
//! of tree the same way.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tessera_formats::did::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{DrawMode, ElementDesc, MediaItem, StateBlock};

use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::geom::IBox;
use crate::tree::{Element, ElementKey};
use crate::ui::{MAX_ELEMENTS, MAX_TREE_DEPTH, Ui};
use crate::widgets::WidgetClass;

use super::super::{GameError, find_below, subtree};

/// First element ID of the P3 fallback widgets' code-made elements (Tessera's own range,
/// below [`super::super::ICON_LAYER_ID`] and outside the creation controls' range).
pub const CODE_ID_BASE: u32 = 0x7E00_0000;

/// One past the last element ID of the range.
const CODE_ID_END: u32 = 0x7EFF_0000;

static NEXT_ID: AtomicU32 = AtomicU32::new(CODE_ID_BASE);

/// Element ID of Tessera's previous-page button inside a page control (outside the code
/// range, so that a controller finds it again by ID; Tessera rule of `npc-windows.md` §7.7).
pub const PAGE_PREV_ID: u32 = 0x7EFF_FE01;

/// Element ID of Tessera's next-page button inside a page control.
pub const PAGE_NEXT_ID: u32 = 0x7EFF_FE02;

/// Element ID of a fallback option row's label (one fixed ID for every row, so that the
/// click registrations of the rows do not grow with each page shown).
pub const OPTION_LABEL_ID: u32 = 0x7EFF_FE03;

/// A fresh element ID of the P3 code range (wraps around inside the range).
pub(crate) fn code_id() -> u32 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    if !(CODE_ID_BASE..CODE_ID_END).contains(&id) {
        NEXT_ID.store(CODE_ID_BASE + 1, Ordering::Relaxed);
        return CODE_ID_BASE;
    }
    id
}

/// Draw mode of fallback art: 4, Alphablend (the compositor's Over).
pub const ART_MODE: u32 = 4;

/// Most text characters a P3 widget puts into one element (Tessera bound: longer page
/// texts, names and lines are cut, so a hostile text cannot make layout unbounded).
pub const MAX_TEXT_CHARS: usize = 8192;

/// Tessera's colours of the fallback widgets (`0xAARRGGBB`). The values of the install's
/// own templates are used where a spec names one (`npc-windows.md` §5.2, §9.2,
/// `quest-journal.md` §6.1).
pub mod colours {
    /// Titles (`ActionPickerMiscTemplate`'s and the category title's colour, §5.3, §9.4).
    pub const TITLE: u32 = 0xFFF5_DF92;
    /// Body text (`QuestDispenserIntroText`, §5.2).
    pub const BODY: u32 = 0xFFC0_C0C0;
    /// Headers and highlighted lines (§5.3 headers, `QuestObjectives_Objective`).
    pub const YELLOW: u32 = 0xFFFF_F600;
    /// White.
    pub const WHITE: u32 = 0xFFFF_FFFF;
    /// Coin counts (`MoneyDisplay_CoinTemplate` `normal`, §9.2).
    pub const COIN: u32 = 0xFFF5_DF93;
    /// Coin counts that are too expensive (`TooExpensive`, §9.2).
    pub const RED: u32 = 0xFFFF_0000;
    /// The tracker's highlighted name (`quest-journal.md` §6.1).
    pub const ORANGE: u32 = 0xFFF3_A20A;
    /// Capped wallet totals (`ItemCapped`, §9.4).
    pub const CAPPED: u32 = 0xFFF2_0C1A;
    /// Ghosted button text (Tessera).
    pub const GHOSTED: u32 = 0xFF80_8080;
}

/// The art of the fallback widgets: generated or original CC0 images the client (or a
/// test) provides by [`ImageRef`]. Every field is optional; a missing image draws nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FallbackArt {
    /// Window background.
    pub window: Option<ImageRef>,
    /// Title bar.
    pub title_bar: Option<ImageRef>,
    /// Inner panels and list backgrounds.
    pub panel: Option<ImageRef>,
    /// A list row (`normal`).
    pub row: Option<ImageRef>,
    /// A selected list row (`highlight`).
    pub row_highlight: Option<ImageRef>,
    /// A button (`normal`).
    pub button: Option<ImageRef>,
    /// A ghosted button.
    pub button_ghosted: Option<ImageRef>,
    /// A close button.
    pub close: Option<ImageRef>,
    /// A tab (`normal`).
    pub tab: Option<ImageRef>,
    /// The selected tab.
    pub tab_selected: Option<ImageRef>,
    /// A text-entry field.
    pub entry: Option<ImageRef>,
    /// An item slot's frame behind an icon.
    pub slot: Option<ImageRef>,
    /// The gold coin.
    pub coin_gold: Option<ImageRef>,
    /// The silver coin.
    pub coin_silver: Option<ImageRef>,
    /// The copper coin.
    pub coin_copper: Option<ImageRef>,
    /// The ready-to-complete mark.
    pub ready: Option<ImageRef>,
    /// The solo group-type icon.
    pub solo: Option<ImageRef>,
    /// The small-fellowship icon.
    pub small_fellowship: Option<ImageRef>,
    /// The fellowship icon.
    pub fellowship: Option<ImageRef>,
    /// The raid icon.
    pub raid: Option<ImageRef>,
    /// The repeatable mark.
    pub repeatable: Option<ImageRef>,
    /// The vendor entries' buy-mode overlay (20 × 20, shown while a buy-mode toggle is
    /// held).
    pub mode_overlay: Option<ImageRef>,
}

/// An Image media item for an image reference.
pub(crate) fn media_of(img: &ImageRef, mode: u32) -> MediaItem {
    match img {
        ImageRef::Did(d) | ImageRef::Skin { did: d, .. } => MediaItem::Image {
            did: Did(*d),
            draw_mode: DrawMode(mode),
            path: String::new(),
        },
        ImageRef::File(p) => MediaItem::Image {
            did: Did(0),
            draw_mode: DrawMode(mode),
            path: p.clone(),
        },
    }
}

/// A colour property value from `0xAARRGGBB` (stored bytes B, G, R, A).
#[must_use]
pub fn colour_value(argb: u32) -> PropertyValue {
    PropertyValue::Colour(argb.to_le_bytes())
}

/// One node of a code-made element tree.
#[derive(Debug, Clone)]
pub(crate) struct FNode {
    pub class: WidgetClass,
    pub desc: ElementDesc,
    pub text: Option<String>,
    pub shown: bool,
    pub children: Vec<FNode>,
}

impl FNode {
    /// A node of `class` with element ID `id` (0: a fresh code ID) at (`x`, `y`), `w` × `h`.
    pub fn new(class: WidgetClass, id: u32, x: i32, y: i32, w: i32, h: i32) -> Self {
        let mut desc = ElementDesc {
            element_id: if id == 0 { code_id() } else { id },
            ..ElementDesc::default()
        };
        desc.block.rect.x = x;
        desc.block.rect.y = y;
        desc.block.rect.width = w.max(0);
        desc.block.rect.height = h.max(0);
        desc.block.has_position = true;
        Self {
            class,
            desc,
            text: None,
            shown: true,
            children: Vec::new(),
        }
    }

    /// A Field.
    pub fn field(id: u32, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self::new(WidgetClass::Field, id, x, y, w, h)
    }

    /// A Text with a colour and an initial text.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        id: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        colour_pid: u32,
        colour: u32,
        text: &str,
    ) -> Self {
        let mut n =
            Self::new(WidgetClass::Text, id, x, y, w, h).prop(colour_pid, colour_value(colour));
        n.text = (!text.is_empty()).then(|| text.to_string());
        n
    }

    /// A Button with art for `normal` and `ghosted` (when given), a label colour and a text.
    #[allow(clippy::too_many_arguments)]
    pub fn button(
        id: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        art: (&Option<ImageRef>, &Option<ImageRef>),
        colour_pid: u32,
        ghosted_state: u32,
        text: &str,
    ) -> Self {
        let mut n = Self::new(WidgetClass::Button, id, x, y, w, h)
            .image(art.0)
            .prop(colour_pid, colour_value(colours::TITLE));
        n = n.state(
            ghosted_state,
            art.1.as_ref().or(art.0.as_ref()),
            vec![(colour_pid, colour_value(colours::GHOSTED))],
        );
        n.text = (!text.is_empty()).then(|| text.to_string());
        n
    }

    /// Adds `img` (when given) to the element's own media.
    pub fn image(mut self, img: &Option<ImageRef>) -> Self {
        if let Some(i) = img {
            self.desc.block.media.push(media_of(i, ART_MODE));
        }
        self
    }

    /// Sets a property of the element's own block.
    pub fn prop(mut self, pid: u32, value: PropertyValue) -> Self {
        let ps = &mut self.desc.block.properties;
        ps.entries.retain(|e| e.pid != pid);
        ps.entries.push(PropertyEntry { pid, value });
        ps.size.count = u32::try_from(ps.entries.len()).unwrap_or(u32::MAX);
        self
    }

    /// Adds a named state with an image (when given) and properties.
    pub fn state(
        mut self,
        state_id: u32,
        img: Option<&ImageRef>,
        props: Vec<(u32, PropertyValue)>,
    ) -> Self {
        if state_id == 0 || self.desc.states.iter().any(|s| s.state_id == state_id) {
            return self;
        }
        let mut s = StateBlock {
            state_id,
            ..StateBlock::default()
        };
        if let Some(i) = img {
            s.media.push(media_of(i, ART_MODE));
        }
        for (pid, value) in props {
            s.properties.entries.push(PropertyEntry { pid, value });
        }
        s.properties.size.count = u32::try_from(s.properties.entries.len()).unwrap_or(u32::MAX);
        self.desc.states.push(s);
        self.desc.states_size.count = u32::try_from(self.desc.states.len()).unwrap_or(u32::MAX);
        self
    }

    /// Sets the element type (so that type lookups find fallback elements too).
    pub fn typed(mut self, ty: u32) -> Self {
        self.desc.element_type = ty;
        self
    }

    /// Starts hidden.
    pub fn hidden(mut self) -> Self {
        self.shown = false;
        self
    }

    /// Adds a child.
    pub fn child(mut self, c: FNode) -> Self {
        self.children.push(c);
        self
    }

    /// Adds children.
    pub fn children(mut self, cs: impl IntoIterator<Item = FNode>) -> Self {
        self.children.extend(cs);
        self
    }
}

/// Creates one code-made element from `node` (no children), attached to `parent` when
/// given.
fn make_one(ui: &mut Ui, parent: Option<ElementKey>, node: &FNode) -> Option<ElementKey> {
    if ui.element_count() >= MAX_ELEMENTS {
        return None;
    }
    if let Some(p) = parent
        && ui.depth(p) >= MAX_TREE_DEPTH
    {
        return None;
    }
    let mut e = Element::new(
        node.desc.element_id,
        node.class,
        Arc::new(node.desc.clone()),
    );
    e.effective = Ui::compute_effective(&e);
    e.accepts_double_click = true;
    let k = ui.insert_element(e);
    if let Some(p) = parent
        && ui.attach_raw(k, p).is_err()
    {
        ui.destroy(k);
        return None;
    }
    ui.post_create(k);
    if let Some(t) = &node.text {
        ui.set_text(k, Some(t.clone()));
    }
    Some(k)
}

/// Creates the tree of `node` below `parent` (pre-order, children in order, bounded by the
/// element cap and the tree depth). Returns the node's element.
pub(crate) fn instantiate(ui: &mut Ui, parent: ElementKey, node: &FNode) -> Option<ElementKey> {
    let k = make_one(ui, Some(parent), node)?;
    build_children(ui, k, node);
    Some(k)
}

fn build_children(ui: &mut Ui, k: ElementKey, node: &FNode) {
    // Iterative: (element, node) pairs whose children are still to be made.
    let mut stack: Vec<(ElementKey, &FNode)> = vec![(k, node)];
    while let Some((el, n)) = stack.pop() {
        for c in &n.children {
            if let Some(ck) = make_one(ui, Some(el), c) {
                stack.push((ck, c));
            }
        }
        if !n.shown {
            ui.set_shown(el, false);
        }
    }
}

/// Opens `node` as a root panel (attached to the screen root, screen-level, shown), as
/// `ui-layout.md` §9.1 does for a layout's panel.
///
/// # Errors
/// [`crate::UiError::TooManyElements`] past the element cap, or an attachment error.
pub(crate) fn open_root(ui: &mut Ui, node: &FNode) -> Result<ElementKey, GameError> {
    if ui.element_count() >= MAX_ELEMENTS {
        return Err(crate::UiError::TooManyElements.into());
    }
    let k = make_one(ui, None, node).ok_or(crate::UiError::TooManyElements)?;
    if let Some(e) = ui.el_mut(k) {
        e.screen_level = true;
        e.root_panel = true;
    }
    let screen = ui.screen_root();
    if let Err(err) = ui.attach_raw(k, screen) {
        ui.destroy(k);
        return Err(err.into());
    }
    build_children(ui, k, node);
    ui.set_shown(k, true);
    ui.panel_opened(k);
    Ok(k)
}

/// Game-code handler registrations made by one controller, released together.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Listen(Vec<(u32, u32)>);

impl Listen {
    /// Registers a handler for `msg` on `k`'s element ID (once per pair).
    pub fn on(&mut self, ui: &mut Ui, k: Option<ElementKey>, msg: u32) {
        let Some(id) = k.and_then(|k| ui.element_id(k)) else {
            return;
        };
        if id != 0 && !self.0.contains(&(id, msg)) {
            ui.register_handler(id, msg);
            self.0.push((id, msg));
        }
    }

    /// Registers the click messages (button click, mouse click, double click) on `k`.
    pub fn clicks(&mut self, ui: &mut Ui, k: Option<ElementKey>) {
        let m = ui.vocab().msgs;
        for msg in [m.button_click, m.mouse_click, m.mouse_dbl_click] {
            self.on(ui, k, msg);
        }
    }

    /// Removes every registration.
    pub fn release(&mut self, ui: &mut Ui) {
        for (id, msg) in self.0.drain(..) {
            ui.unregister_handler(id, msg);
        }
    }
}

/// What kind of click a UI event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClickKind {
    /// `Button_click`.
    Button,
    /// `Element_mouse_click` with the left button.
    Left,
    /// `Element_mouse_click` with the right button.
    Right,
    /// `Element_mouse_dbl_click` with the left button (`SelectDblLeft`).
    Double,
}

/// The click of a game-code message: (source element, kind). A double-click counts only
/// with the left button (`SelectDblLeft`, the only double-click the windows name); the
/// right and middle double-clicks are ignored (their first click already arrived as a
/// click).
pub(crate) fn click_of(ui: &Ui, ev: &UiEvent) -> Option<(ElementKey, ClickKind)> {
    let UiEvent::Game { message } = ev else {
        return None;
    };
    let m = ui.vocab().msgs;
    let button = message.arg.command.as_ref().map_or(0, |c| c.mouse_button());
    let kind = if message.msg == m.button_click {
        ClickKind::Button
    } else if message.msg == m.mouse_dbl_click && button == 1 {
        ClickKind::Double
    } else if message.msg == m.mouse_click && button == 2 {
        ClickKind::Right
    } else if message.msg == m.mouse_click && button == 1 {
        ClickKind::Left
    } else {
        return None;
    };
    Some((message.source, kind))
}

/// The index of the row in `rows` that is `k` or an ancestor of `k`.
pub(crate) fn row_of(ui: &Ui, rows: &[ElementKey], k: ElementKey) -> Option<usize> {
    let mut cur = Some(k);
    let mut n = 0;
    while let Some(c) = cur {
        if let Some(i) = rows.iter().position(|r| *r == c) {
            return Some(i);
        }
        n += 1;
        if n > MAX_TREE_DEPTH {
            return None;
        }
        cur = ui.parent(c);
    }
    None
}

/// `k` when its element ID is `id`, else the first element with that ID below it.
pub(crate) fn find(ui: &Ui, k: ElementKey, id: u32) -> Option<ElementKey> {
    if id == 0 {
        return None;
    }
    if ui.element_id(k) == Some(id) {
        return Some(k);
    }
    find_below(ui, k, id)
}

/// The element of `k`'s subtree that shows a text: the element with ID `id` when found,
/// else the first Text below `k` (Tessera rule, as `creation::fill_choices_with`), else
/// `k` itself when it shows text.
pub(crate) fn text_part(ui: &Ui, k: ElementKey, id: Option<u32>) -> Option<ElementKey> {
    if let Some(e) = id.and_then(|id| find(ui, k, id)) {
        return Some(e);
    }
    subtree(ui, k)
        .into_iter()
        .skip(1)
        .find(|e| ui.class(*e) == Some(WidgetClass::Text))
        .or_else(|| ui.class(k).is_some_and(WidgetClass::is_text).then_some(k))
}

/// The element of `k`'s subtree that shows an icon: the element with ID `id` when found,
/// else the first square, non-text element below `k` that is not of `k`'s own size, a leaf
/// first (Tessera rule, as `creation::fill_choices_with`).
pub(crate) fn icon_part(ui: &Ui, k: ElementKey, id: Option<u32>) -> Option<ElementKey> {
    if let Some(e) = id.and_then(|id| find(ui, k, id)) {
        return Some(e);
    }
    let root = ui.layout_box(k).unwrap_or_default();
    let squares: Vec<ElementKey> = subtree(ui, k)
        .into_iter()
        .skip(1)
        .filter(|e| !ui.class(*e).is_some_and(WidgetClass::is_text))
        .filter(|e| {
            ui.layout_box(*e).is_some_and(|b| {
                b.w() > 0 && b.w() == b.h() && (b.w(), b.h()) != (root.w(), root.h())
            })
        })
        .collect();
    squares
        .iter()
        .copied()
        .find(|e| ui.children(*e).is_empty())
        .or_else(|| squares.first().copied())
}

/// `s` cut to [`MAX_TEXT_CHARS`] characters.
#[must_use]
pub fn cut(s: &str) -> String {
    match s.char_indices().nth(MAX_TEXT_CHARS) {
        Some((i, _)) => s[..i].to_string(),
        None => s.to_string(),
    }
}

/// Sets a text (cut to the bound) on an element when it exists.
pub(crate) fn put_text(ui: &mut Ui, k: Option<ElementKey>, text: &str) {
    if let Some(k) = k {
        ui.set_text(k, Some(cut(text)));
    }
}

/// Ghosts or unghosts a button (`UICore_Button_ghosted`).
pub(crate) fn ghost(ui: &mut Ui, k: Option<ElementKey>, ghosted: bool) {
    if let Some(k) = k {
        let p = ui.vocab().pids.button_ghosted;
        ui.set_property(k, p, PropertyValue::Bool(ghosted));
        if ui.class(k).is_some_and(WidgetClass::is_button) {
            return;
        }
        // A non-button (a fallback Field): show the state directly.
        let st = ui.vocab().states;
        ui.switch_state_if_defined(k, if ghosted { st.ghosted } else { st.normal });
    }
}

/// Shows a selected or unselected row: state `highlight` or `normal` when the row defines
/// it.
pub(crate) fn mark_row(ui: &mut Ui, row: ElementKey, selected: bool) {
    let st = ui.vocab().states;
    ui.switch_state_if_defined(row, if selected { st.highlight } else { st.normal });
}

/// Destroys every entry of a list box (or, for an element that is not a list box, every
/// child in `rows`).
pub(crate) fn clear_rows(ui: &mut Ui, list: ElementKey, rows: &mut Vec<ElementKey>) {
    if ui.class(list).is_some_and(WidgetClass::is_list) {
        ui.list_clear(list);
    }
    let left: Vec<ElementKey> = rows.drain(..).filter(|r| ui.exists(*r)).collect();
    ui.destroy_many(&left);
}

/// Adds a created row to `list` (a list box: appended as an entry; any other element: the
/// row stays a child where it was made). Returns whether it was added.
pub(crate) fn add_row(ui: &mut Ui, list: ElementKey, row: ElementKey) -> bool {
    if ui.class(list).is_some_and(WidgetClass::is_list) {
        if ui.list_add(list, row, None).is_err() {
            ui.destroy(row);
            return false;
        }
    } else if ui.parent(row) != Some(list) && ui.attach_raw(row, list).is_err() {
        ui.destroy(row);
        return false;
    }
    if let Some(e) = ui.el_mut(row) {
        e.accepts_double_click = true;
    }
    true
}

/// Creates a row from a template of the install's layouts, unattached.
pub(crate) fn row_from_template(
    ui: &mut Ui,
    t: super::super::examine::ExamTemplate,
) -> Option<ElementKey> {
    match ui.create_element(None, Did(t.layout), t.element) {
        Ok(k) => Some(k),
        Err(e) => {
            tracing::debug!(
                layout = t.layout,
                element = t.element,
                "row template not created: {e}"
            );
            None
        }
    }
}

/// Creates a row for `list`: from `template` when given (and creatable), else from the
/// fallback node. The row is a child of `list` but not yet one of its entries: the caller
/// commits its rows with [`commit_rows`] once, so that a list of *n* rows is laid out once
/// and not *n* times.
pub(crate) fn make_row(
    ui: &mut Ui,
    list: ElementKey,
    template: Option<super::super::examine::ExamTemplate>,
    fallback: impl FnOnce() -> FNode,
) -> Option<ElementKey> {
    let k = match template.and_then(|t| row_from_template(ui, t)) {
        Some(k) => k,
        None => return instantiate(ui, list, &fallback()),
    };
    if ui.attach_raw(k, list).is_err() {
        ui.destroy(k);
        return None;
    }
    Some(k)
}

/// Makes `row` a child of `list` (a template row is created unattached); destroys it when
/// that fails. The caller commits it with [`commit_rows`].
pub(crate) fn adopt(ui: &mut Ui, list: ElementKey, row: ElementKey) -> bool {
    if ui.parent(row) == Some(list) {
        return true;
    }
    if ui.attach_raw(row, list).is_err() {
        ui.destroy(row);
        return false;
    }
    true
}

/// Makes rows created by [`make_row`] (or other children of `list`) entries of `list` in
/// order, laying the list out once (`ui.list_add_many`); for an element that is not a list
/// box they stay children. Rows that cannot be added are destroyed. Returns the rows that
/// are entries (or children) afterwards.
pub(crate) fn commit_rows(ui: &mut Ui, list: ElementKey, rows: &[ElementKey]) -> Vec<ElementKey> {
    let rows: Vec<TreeRow> = rows
        .iter()
        .map(|r| TreeRow {
            row: *r,
            parent: None,
            expanded: true,
        })
        .collect();
    commit_tree_rows(ui, list, &rows)
}

/// One row for [`commit_tree_rows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreeRow {
    /// The row.
    pub row: ElementKey,
    /// Its parent row (`None`: a top-level node).
    pub parent: Option<ElementKey>,
    /// Whether the row's node is expanded (used for rows that have children).
    pub expanded: bool,
}

/// [`commit_rows`] with a tree parent per row: for a list of class Tree each row becomes a
/// node under its parent row (a top-level node when `None`, or when the parent is not a
/// node of the tree), so that the tree indents it and its parent's icon expands and
/// collapses it (`ui-widgets.md` §9.2–§9.3); each node with children then gets its
/// `expanded` state (whatever the tree's auto-expand). Parents must come before their
/// children. Other lists ignore the parents.
///
/// A Tree takes its entries one by one (`ui.list_add`, which lays the tree out per entry,
/// so committing *n* rows to a Tree costs O(*n*²); a batched tree insert belongs to the
/// list box widget).
pub(crate) fn commit_tree_rows(ui: &mut Ui, list: ElementKey, rows: &[TreeRow]) -> Vec<ElementKey> {
    for r in rows {
        if let Some(e) = ui.el_mut(r.row) {
            e.accepts_double_click = true;
        }
    }
    let live: Vec<TreeRow> = rows.iter().copied().filter(|r| ui.exists(r.row)).collect();
    if !ui.class(list).is_some_and(WidgetClass::is_list) {
        return live.into_iter().map(|r| r.row).collect();
    }
    if ui.class(list) == Some(WidgetClass::Tree) {
        let parents: std::collections::HashSet<ElementKey> =
            live.iter().filter_map(|r| r.parent).collect();
        let mut kept = Vec::with_capacity(live.len());
        let mut nodes = std::collections::HashSet::with_capacity(live.len());
        let mut states = Vec::new();
        for r in live {
            let parent = r.parent.filter(|p| nodes.contains(p));
            if ui.list_add(list, r.row, parent).is_ok() {
                kept.push(r.row);
                nodes.insert(r.row);
                if parents.contains(&r.row) {
                    states.push((r.row, r.expanded));
                }
            } else {
                ui.destroy(r.row);
            }
        }
        for (k, expanded) in states {
            ui.tree_set_expanded(list, k, expanded);
        }
        return kept;
    }
    let live: Vec<ElementKey> = live.into_iter().map(|r| r.row).collect();
    match ui.list_add_many(list, &live) {
        Ok(()) => live,
        Err((added, e)) => {
            tracing::debug!("rows not added to the list: {e}");
            ui.destroy_many(&live[added..]);
            live[..added].to_vec()
        }
    }
}

/// Measures `text` in element `k` with wrap width `wrap` through the client's shaper
/// (`fonts.md` §7; the extent includes the element's margins). Without shaped text (fonts
/// not loaded) the element's designed size stands in. The text is measured against an
/// unbounded height: measured against the box's current height, an element whose text is
/// truncated to fit (`fonts.md` §7.6) would report only the lines that fit, and a row fitted
/// to that would cut its wrapped text.
pub(crate) fn measure(ui: &mut Ui, k: ElementKey, text: &str, wrap: i32) -> (i32, i32) {
    match shaped_extent(ui, k, text, wrap) {
        Some(e) => e,
        None => {
            let b = ui.layout_box(k).unwrap_or_default();
            let d = ui.design_box(k).unwrap_or(b);
            (d.w().max(0), d.h().max(0))
        }
    }
}

/// [`measure`] without the stand-in: `None` while the shaper cannot lay the text out yet
/// (its fonts are still loading).
fn shaped_extent(ui: &mut Ui, k: ElementKey, text: &str, wrap: i32) -> Option<(i32, i32)> {
    /// Height given to the shaper when measuring (well above any fitted text's bound).
    const MEASURE_HEIGHT: i32 = 1 << 16;
    let props = ui.properties(k);
    let b = ui.layout_box(k).unwrap_or_default();
    let req = crate::env::TextRequest {
        text,
        props: &props,
        size: (b.w(), MEASURE_HEIGHT),
        wrap_width: Some(wrap.max(1)),
    };
    ui.env
        .shape_text(&req)
        .map(|p| (p.extent.0.clamp(0, wrap.max(1)), p.extent.1.max(0)))
}

/// Whether text element `k`'s text can be measured now (an empty text always can): a row
/// fitted while its fonts were loading took its designed height and is fitted again once
/// they have arrived.
pub(crate) fn text_ready(ui: &mut Ui, k: ElementKey) -> bool {
    let text = ui.text_of(k);
    if text.is_empty() {
        return true;
    }
    let w = ui.layout_box(k).map_or(1, |b| b.w());
    shaped_extent(ui, k, &text, w).is_some()
}

/// Fits the height of text element `k` to its text at its own width (at least `min`
/// pixels, at most `max`), as the "fit to text" texts of the journal and the greeting row
/// do (Tessera rule for the fallback rows: the height follows the wrapped text). Returns
/// the height set and whether the text was taller than `max`.
pub(crate) fn fit_height(ui: &mut Ui, k: ElementKey, min: i32, max: i32) -> (i32, bool) {
    let text = ui.text_of(k);
    let b = ui.layout_box(k).unwrap_or_default();
    let h = if text.is_empty() {
        0
    } else {
        measure(ui, k, &text, b.w()).1
    };
    let cut = h > max;
    let h = h.clamp(min.max(0), max.max(min).max(0));
    ui.resize(k, b.w(), h);
    (h, cut)
}

/// Switches wrapping of text element `k` off (`one_line` true) or on
/// (`UICore_Text_one_line`, `fonts.md` §7.3). With wrapping off, a text with
/// `UICore_Text_truncate_text_to_fit` is cut on its one line with the suffix (§7.6).
pub(crate) fn set_one_line(
    ui: &mut Ui,
    nv: &super::super::NpcVocab,
    k: Option<ElementKey>,
    on: bool,
) {
    if let Some(k) = k {
        ui.set_property(k, nv.pids.text_one_line, PropertyValue::Bool(on));
    }
}

/// Sizes `row` to `width` (keeping its height) and its text element `text`, when that is a
/// descendant, so that the text ends at the row's right edge; the text wraps (wrapping on)
/// and the row's height follows the wrapped text (`fit`: at least `min`, at most `max`
/// pixels for the text). A row grows or shrinks by the text's change of height, so that the
/// next row of a list starts below it.
///
/// Tessera rule (`npc-windows.md` §6.2 OPEN, `quest-journal.md` §4.7 OPEN): the texts of
/// the objective block and of the details pane are as wide as their list and as tall as
/// their wrapped text.
pub(crate) fn fit_text_row(
    ui: &mut Ui,
    nv: &super::super::NpcVocab,
    row: ElementKey,
    text: ElementKey,
    width: i32,
    (min, max): (i32, i32),
) {
    let width = width.max(1);
    if let Some(b) = ui.layout_box(row)
        && b.w() != width
    {
        ui.resize(row, width, b.h().max(1));
    }
    set_one_line(ui, nv, Some(text), false);
    if text == row {
        fit_height(ui, row, min, max);
        return;
    }
    // The text's left edge in the row (its boxes are relative to its parent).
    let mut left = ui.layout_box(text).map_or(0, |b| b.l);
    let mut cur = ui.parent(text);
    let mut depth = 0;
    while let Some(c) = cur {
        if c == row || depth > MAX_TREE_DEPTH {
            break;
        }
        left = left.saturating_add(ui.layout_box(c).map_or(0, |b| b.l));
        cur = ui.parent(c);
        depth += 1;
    }
    if let Some(tb) = ui.layout_box(text) {
        let tw = width.saturating_sub(left.max(0)).max(1);
        if tb.w() != tw {
            ui.resize(text, tw, tb.h().max(1));
        }
    }
    let before = ui.layout_box(text).map_or(0, |b| b.h());
    let (h, _) = fit_height(ui, text, min, max);
    let grow = h - before;
    if grow != 0
        && let Some(rb) = ui.layout_box(row)
    {
        ui.resize(row, rb.w(), rb.h().saturating_add(grow).max(1));
        // A text anchored to the row's top and bottom followed the row; one anchored only
        // at the top keeps the fitted height.
        if let Some(b) = ui.layout_box(text)
            && b.h() != h
        {
            ui.resize(text, b.w(), h);
        }
    }
}

/// Fits label `k`, a text element inside a control of fixed size (its parent), to its
/// wrapped text vertically, from `laid_out`, the label's box (relative to the parent) as the
/// layout placed it. The label keeps that box's left edge and width; when its text, wrapped
/// at that width, is taller than the box, its height becomes the text's height (margins
/// included), at most the parent's height, centred on the box's vertical centre and kept
/// inside the parent; when the text fits (or is empty) it gets `laid_out` back. Returns
/// `false` while the text cannot be measured yet (fonts loading): the label then keeps its
/// current box and the caller fits it again later.
///
/// Tessera rule (`travel-windows.md` §6.4 OPEN): the instance finder's join label.
pub(crate) fn fit_label_height(ui: &mut Ui, k: ElementKey, laid_out: IBox) -> bool {
    if !ui.exists(k) {
        return true;
    }
    let text = ui.text_of(k);
    let (w, h0) = (laid_out.w().max(1), laid_out.h().max(0));
    let need = if text.is_empty() {
        0
    } else {
        match shaped_extent(ui, k, &text, w) {
            Some((_, h)) => h,
            None => return false,
        }
    };
    let parent_h = ui
        .parent(k)
        .and_then(|p| ui.layout_box(p))
        .map_or(h0, |b| b.h().max(0));
    let (top, h) = if need > h0 {
        let h = need.min(parent_h.max(h0));
        // Centred on the laid-out box's vertical centre, then kept inside the parent.
        let centre = i64::from(laid_out.t) * 2 + i64::from(h0);
        let top = (centre - i64::from(h)).div_euclid(2);
        let top = top.clamp(0, i64::from(parent_h.saturating_sub(h).max(0)));
        (i32::try_from(top).unwrap_or(laid_out.t), h)
    } else {
        (laid_out.t, h0)
    };
    if let Some(b) = ui.layout_box(k)
        && (b.l, b.t, b.w(), b.h()) != (laid_out.l, top, w, h)
    {
        if (b.w(), b.h()) != (w, h) {
            ui.resize(k, w, h);
        }
        ui.move_to(k, laid_out.l, top);
    }
    true
}

/// Keeps root panel `root` on the screen (Tessera rule, `npc-windows.md` §4 OPEN): its
/// displayed box is clamped into the screen as the move step of `ui-behaviour.md` §8.1
/// step 3 clamps a dragged panel (left, then right; top, then bottom: a panel larger than
/// the screen ends with its right and bottom edges on the screen's). The size is kept.
pub fn keep_on_screen(ui: &mut Ui, root: ElementKey) {
    let Some(lb) = ui.layout_box(root) else {
        return;
    };
    let d = ui.displayed_box(root);
    let (sw, sh) = ui.screen_size();
    let x = d.l.max(0).min(sw.saturating_sub(d.w()));
    let y = d.t.max(0).min(sh.saturating_sub(d.h()));
    let (dx, dy) = (x - d.l, y - d.t);
    if (dx, dy) != (0, 0) {
        ui.move_to(root, lb.l.saturating_add(dx), lb.t.saturating_add(dy));
    }
}

/// Most fonts a ladder keeps per element (Tessera bound; the install's ladders have 7).
pub const MAX_LADDER_FONTS: usize = 16;

/// The font-size arrows of the quest panels (`fonts.md` §9.2; Tessera rules of
/// `npc-windows.md` §5.2 and `quest-journal.md` §4.7): one font index for the panel,
/// starting at 0; increase asks for index + 1, decrease for index − 1; an element whose
/// font list (`UICore_Text_fonts`, read when it joins the ladder) is long enough takes that
/// entry for both font slots, the others keep their font; the index changes only when at
/// least one element took it (so it stays within the longest list and never goes below 0).
#[derive(Debug, Clone, Default)]
pub(crate) struct FontLadder {
    session: crate::text::FontIndexSession,
    elements: Vec<(ElementKey, Vec<u32>)>,
    /// The increase arrow.
    pub up: Option<ElementKey>,
    /// The decrease arrow.
    pub down: Option<ElementKey>,
}

impl FontLadder {
    /// A ladder with its two arrows (clicks registered on `listen`).
    pub fn new(
        ui: &mut Ui,
        listen: &mut Listen,
        up: Option<ElementKey>,
        down: Option<ElementKey>,
    ) -> Self {
        listen.clicks(ui, up);
        listen.clicks(ui, down);
        Self {
            up,
            down,
            ..Self::default()
        }
    }

    /// The current index.
    pub fn index(&self) -> usize {
        self.session.index()
    }

    /// Adds `k` with its own font list and gives it the current index (an element already
    /// in the ladder keeps the list read when it joined). Elements without a list are not
    /// added.
    pub fn add(&mut self, ui: &mut Ui, nv: &super::super::NpcVocab, k: Option<ElementKey>) {
        let Some(k) = k else { return };
        self.elements.retain(|(e, _)| ui.exists(*e));
        if let Some(i) = self.elements.iter().position(|(e, _)| *e == k) {
            self.apply_one(ui, nv, i);
            return;
        }
        let pid = ui.vocab().pids.text_fonts;
        let fonts: Vec<u32> = ui
            .prop(k, pid)
            .value()
            .map(|v| {
                crate::props::array_elements(v)
                    .iter()
                    .map(|e| crate::props::as_key(&e.value))
                    .filter(|d| *d != 0)
                    .take(MAX_LADDER_FONTS)
                    .collect()
            })
            .unwrap_or_default();
        if fonts.is_empty() {
            return;
        }
        self.elements.push((k, fonts));
        self.apply_one(ui, nv, self.elements.len() - 1);
    }

    fn lens(&self) -> Vec<usize> {
        self.elements.iter().map(|(_, f)| f.len()).collect()
    }

    /// Restores a saved index (as a request: kept only when an element can take it).
    /// Returns whether it was taken.
    pub fn set(&mut self, ui: &mut Ui, nv: &super::super::NpcVocab, index: usize) -> bool {
        let r = i64::try_from(index).unwrap_or(i64::MAX);
        let ok = self.session.request(r, &self.lens());
        if ok {
            self.apply(ui, nv);
        }
        ok
    }

    /// A click from `src`: the new index when an arrow was hit and the index changed.
    pub fn click(
        &mut self,
        ui: &mut Ui,
        nv: &super::super::NpcVocab,
        src: ElementKey,
    ) -> Option<Option<usize>> {
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let before = self.index();
        let lens = self.lens();
        if hit(self.up) {
            self.session.increase(&lens);
        } else if hit(self.down) {
            self.session.decrease(&lens);
        } else {
            return None;
        }
        if self.index() == before {
            return Some(None);
        }
        self.apply(ui, nv);
        Some(Some(self.index()))
    }

    /// Gives every element the current index.
    pub fn apply(&mut self, ui: &mut Ui, nv: &super::super::NpcVocab) {
        self.elements.retain(|(e, _)| ui.exists(*e));
        for i in 0..self.elements.len() {
            self.apply_one(ui, nv, i);
        }
    }

    fn apply_one(&self, ui: &mut Ui, nv: &super::super::NpcVocab, i: usize) {
        let Some((k, fonts)) = self.elements.get(i) else {
            return;
        };
        let Some(font) = fonts.get(self.index()).copied() else {
            return;
        };
        let d = PropertyValue::DidRef(Did(font));
        let arr = |pid: u32| {
            PropertyValue::Array(tessera_formats::property::PropertyArray {
                count: 1,
                elements: vec![PropertyEntry {
                    pid,
                    value: d.clone(),
                }],
            })
        };
        let p = nv.pids;
        let fonts_pid = ui.vocab().pids.text_fonts;
        ui.set_property(*k, p.text_font, d.clone());
        ui.set_property(*k, p.text_tag_font, d.clone());
        ui.set_property(*k, fonts_pid, arr(p.text_font));
        ui.set_property(*k, p.text_tag_fonts, arr(p.text_tag_font));
    }
}
