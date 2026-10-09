// SPDX-License-Identifier: GPL-3.0-or-later
//! GroupBox (`ui-widgets.md` §3.4): pure helpers. The behaviour (selection change,
//! completion pass, clicks) is part of the `Ui` methods of the menu module.

/// Whether a click from element ID `source` on a GroupBox whose selection is `selected`
/// goes on past the GroupBox (§3.4 "Clicks").
#[must_use]
pub fn click_continues(source: u32, selected: u32, allow_click_on_selected: bool) -> bool {
    source != selected || selected == 0 || allow_click_on_selected
}
