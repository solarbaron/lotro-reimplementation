// SPDX-License-Identifier: GPL-3.0-or-later
//! Click helpers of the game windows' rows and the front z-level of root panels.
//!
//! Tessera's code-made popup list that the P3 tracker menu, the P5 map menus and the
//! finder's menus used while `tessera-ui` had no context-menu widget is retired: those
//! menus are context menus of `ui-widgets.md` §19 now
//! ([`crate::game::context_menu`]). What stays here is shared by the windows' rows:
//!
//! - `click`: the click of a game-code message, a Button-class source's mouse click
//!   dropped in favour of its `Button_click`;
//! - `listen_row`: the click registrations of a code-made row (and its mouse-visible
//!   flag);
//! - `front_z`: the z-level that puts a new root panel in front of every other (a lower
//!   z is in front, `ui-layout.md` §9.3).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::npc::kit::{ClickKind, Listen, click_of};

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
pub(crate) fn front_z(ui: &Ui) -> i32 {
    let screen = ui.screen_root();
    ui.children(screen)
        .iter()
        .filter_map(|c| ui.z_level(*c))
        .min()
        .unwrap_or(0)
        .min(0)
        .saturating_sub(1)
}
