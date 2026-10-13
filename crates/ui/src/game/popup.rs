// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's popup list for the P5 menus: the map window's `FilterMenu` and `MapMenu`
//! (`map-windows.md` §4.5, §6.9) and the finder's size and tier menus
//! (`travel-windows.md` §6.4).
//!
//! **Tessera rule.** The original fills its context menus (`ContextMenuBool`,
//! `ContextMenuElement_MapMenu` of the `ContextMenu` layout) from game code; `tessera-ui`
//! has no context-menu widget that game code fills, so these menus open a code-made popup
//! panel below their button: one row per item, header rows (a submenu's label) not
//! selectable, check items with a check mark, an optional icon. The popup stays open until
//! its button is clicked again or the owner closes it; the owner decides what a selection
//! does.
//!
//! **Tessera rules.** A popup opens in front of every other root panel (its z-level is one
//! below the lowest z-level of the screen root's children; a lower z is in front,
//! `ui-layout.md` §9.3), so it is never hidden behind the full-screen map window (z −1,500).
//! A popup taller than the screen is laid out in columns of as many rows as fit the screen
//! height, so every row lies on the screen while the columns fit its width.

use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::GameError;
use super::mapfallback::with_z;
use super::npc::kit::{
    ClickKind, FNode, FallbackArt, Listen, click_of, colours, cut, open_root, row_of,
};
use super::npcvocab::NpcVocab;

/// Height of a popup row.
pub const ROW_H: i32 = 20;

/// Most rows a popup shows (Tessera bound).
pub const MAX_ROWS: usize = 256;

/// One item of a popup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PopupItem {
    /// The text.
    pub text: String,
    /// A check item's state (`None`: a command item).
    pub check: Option<bool>,
    /// A header (a submenu's label): drawn in the title colour, not selectable.
    pub header: bool,
    /// An icon left of the text.
    pub icon: Option<ImageRef>,
    /// Extra indentation in pixels (items under a header).
    pub indent: i32,
}

impl PopupItem {
    /// A command item.
    #[must_use]
    pub fn command(text: &str) -> Self {
        Self {
            text: text.to_string(),
            ..Self::default()
        }
    }
}

/// An open popup.
#[derive(Debug, Clone)]
pub struct Popup {
    /// The popup's root panel.
    pub root: ElementKey,
    /// The rows, in item order.
    pub rows: Vec<ElementKey>,
    /// The check marks (by row; `None` for rows without one).
    checks: Vec<Option<ElementKey>>,
    /// The items.
    pub items: Vec<PopupItem>,
    listen: Listen,
}

/// Where a popup opens relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Below the anchor's displayed box, left edges aligned (a menu button's popup).
    Below,
    /// Right of the anchor's displayed box, top edges aligned (a submenu of a popup row).
    Beside,
}

/// The click of a game-code message for the P5 windows: as the kit's `click_of`, but a
/// left click or left double-click whose source is a Button-class element (Button, Menu,
/// Scrollbar) is dropped. One physical click on a button posts both `Button_click` and
/// `Element_mouse_click` (`ui-widgets.md` §7.1, `ui-behaviour.md` §4.6); only the
/// `Button_click` is acted on (a ghosted button posts none, so it does not act).
pub(crate) fn click(ui: &Ui, ev: &UiEvent) -> Option<(ElementKey, ClickKind)> {
    let (src, kind) = click_of(ui, ev)?;
    if matches!(kind, ClickKind::Left | ClickKind::Double)
        && ui
            .class(src)
            .is_some_and(crate::widgets::WidgetClass::is_button)
    {
        return None;
    }
    Some((src, kind))
}

/// Registers the click messages on a list row made by game code and makes the row
/// mouse-visible. Rows share one element ID, so only the first registration of the ID
/// marks an element mouse-visible (`ui-behaviour.md` §4.3; the registry check runs when
/// a handler is registered and when a layout element is built, not for code-made
/// elements); without the flag a real click on a later row hits the list instead.
pub(crate) fn listen_row(ui: &mut Ui, listen: &mut Listen, k: ElementKey) {
    listen.clicks(ui, Some(k));
    ui.set_mouse_visible_flag(k, true);
}

/// The z-level that puts a new root panel in front of every child of the screen root.
fn front_z(ui: &Ui) -> i32 {
    let screen = ui.screen_root();
    ui.children(screen)
        .iter()
        .filter_map(|c| ui.z_level(*c))
        .min()
        .unwrap_or(0)
        .min(0)
        .saturating_sub(1)
}

impl Popup {
    /// Opens a popup of `width` pixels below `anchor` (its displayed box's bottom-left
    /// corner, kept on the screen) with `items` (at most [`MAX_ROWS`]).
    ///
    /// # Errors
    /// The element cap or an attachment error.
    pub fn open(
        ui: &mut Ui,
        nv: &NpcVocab,
        art: &FallbackArt,
        check_image: Option<&ImageRef>,
        anchor: ElementKey,
        width: i32,
        items: Vec<PopupItem>,
    ) -> Result<Self, GameError> {
        Self::open_at(ui, nv, art, check_image, anchor, Place::Below, width, items)
    }

    /// Opens a popup as [`Popup::open`] does, placed by `place` relative to `anchor`.
    ///
    /// # Errors
    /// The element cap or an attachment error.
    #[allow(clippy::too_many_arguments)]
    pub fn open_at(
        ui: &mut Ui,
        nv: &NpcVocab,
        art: &FallbackArt,
        check_image: Option<&ImageRef>,
        anchor: ElementKey,
        place: Place,
        width: i32,
        items: Vec<PopupItem>,
    ) -> Result<Self, GameError> {
        let mut items = items;
        items.truncate(MAX_ROWS);
        let cp = nv.pids.text_font_color;
        let w = width.max(60);
        let n = i32::try_from(items.len()).unwrap_or(0);
        // Rows per column: as many as fit the screen height (at least one).
        let (_, sh) = ui.screen_size();
        let per_col = (sh.saturating_sub(4) / ROW_H).max(1);
        let cols = if n == 0 {
            1
        } else {
            (n + per_col - 1) / per_col
        };
        let rows_h = n.min(per_col);
        let mut root = with_z(
            FNode::field(0, 0, 0, w.saturating_mul(cols), rows_h * ROW_H + 4),
            front_z(ui),
        )
        .image(&art.panel);
        for (i, it) in items.iter().enumerate() {
            let i = i32::try_from(i).unwrap_or(0);
            let (col, line) = (i / per_col, i % per_col);
            let y = 2 + line * ROW_H;
            let x = 4 + it.indent.clamp(0, w / 2);
            let colour = if it.header {
                colours::TITLE
            } else {
                colours::WHITE
            };
            let mut row = FNode::field(0, col.saturating_mul(w) + 2, y, w - 4, ROW_H);
            if !it.header {
                row = row.image(&art.row);
            }
            let mut tx = x;
            if it.check.is_some() {
                let mut mark = FNode::field(0, 0, 0, 16, 16).image(&check_image.cloned());
                if it.check != Some(true) {
                    mark = mark.hidden();
                }
                row = row.child(FNode::field(0, x, 2, 16, 16).image(&art.slot).child(mark));
                tx += 20;
            }
            if let Some(icon) = &it.icon {
                row = row.child(FNode::field(0, tx, 2, 16, 16).image(&Some(icon.clone())));
                tx += 20;
            }
            row = row.child(FNode::text(
                0,
                tx,
                2,
                (w - tx - 6).max(1),
                ROW_H - 4,
                cp,
                colour,
                &cut(&it.text),
            ));
            root = root.child(row);
        }
        let k = open_root(ui, &root)?;
        let b = ui.displayed_box(anchor);
        match place {
            Place::Below => ui.move_to(k, b.l, b.t.saturating_add(b.h())),
            Place::Beside => ui.move_to(k, b.l.saturating_add(b.w()), b.t),
        }
        super::npc::keep_on_screen(ui, k);
        let rows = ui.children(k);
        let checks = rows
            .iter()
            .zip(&items)
            .map(|(r, it)| {
                it.check?;
                let frame = ui.children(*r).first().copied()?;
                ui.children(frame).first().copied()
            })
            .collect();
        let mut p = Self {
            root: k,
            rows,
            checks,
            items,
            listen: Listen::default(),
        };
        for (r, it) in p.rows.clone().into_iter().zip(p.items.clone()) {
            if !it.header {
                p.listen.clicks(ui, Some(r));
            }
        }
        Ok(p)
    }

    /// The item a UI event selects: a left click (or double-click) on a row that is not a
    /// header.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<usize> {
        let (src, kind) = click(ui, ev)?;
        if !matches!(
            kind,
            ClickKind::Left | ClickKind::Button | ClickKind::Double
        ) || !ui.exists(src)
            || !ui.is_ancestor_or_self(self.root, src)
        {
            return None;
        }
        let i = row_of(ui, &self.rows, src)?;
        (!self.items.get(i)?.header).then_some(i)
    }

    /// Sets a check item's mark.
    pub fn set_checked(&mut self, ui: &mut Ui, i: usize, on: bool) {
        if let Some(it) = self.items.get_mut(i)
            && it.check.is_some()
        {
            it.check = Some(on);
            if let Some(Some(m)) = self.checks.get(i) {
                ui.set_shown(*m, on);
            }
        }
    }

    /// Closes the popup (its root and handlers go).
    pub fn close(mut self, ui: &mut Ui) {
        self.listen.release(ui);
        if ui.exists(self.root) {
            ui.destroy(self.root);
        }
    }
}
