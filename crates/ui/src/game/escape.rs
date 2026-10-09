// SPDX-License-Identifier: GPL-3.0-or-later
//! Escape with the game UI attached (`game-panels.md` §16, Tessera rule from decision 0014,
//! Consequences "Escape with a UI attached"): the hook the client runs as game code's
//! handling of an `EscapeKey` press that the UI did not consume. It never quits.

use crate::tree::ElementKey;
use crate::ui::Ui;

/// The panels the rule knows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscapePanels {
    /// The root elements of the open on-demand panels (P6, P7, P8, P9, P10, P12, P13, P15
    /// of `ui-layout.md` §17.2). HUD panels (P1–P5, P11, P14) and plugin windows are never
    /// listed, so the rule never hides them.
    pub on_demand: Vec<ElementKey>,
    /// The main menu P6, when it has been created.
    pub main_menu: Option<ElementKey>,
}

/// What the rule did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// A release or an auto-repeat: nothing happens.
    Ignored,
    /// Rule 1: the active on-demand panel was hidden.
    HidActive(ElementKey),
    /// Rule 2: P6 was shown and made the active panel.
    ShowedMainMenu(ElementKey),
    /// Rule 2: P6 was hidden.
    HidMainMenu(ElementKey),
    /// Rule 2 with P6 not created yet: the client opens it (`ui-layout.md` §17.1) and makes
    /// it the active panel.
    OpenMainMenu,
}

/// Runs the §16 rule for a `menu` command the UI declined. Only a **press** that is not an
/// auto-repeat acts.
///
/// 1. If the active panel (`ui-behaviour.md` §5.1) is an on-demand panel, hide it.
/// 2. Otherwise, if P6 is hidden, show it and make it the active panel; if it is shown,
///    hide it.
pub fn declined_escape(
    ui: &mut Ui,
    panels: &EscapePanels,
    press: bool,
    repeat: bool,
) -> EscapeOutcome {
    if !press || repeat {
        return EscapeOutcome::Ignored;
    }
    let is_on_demand = |k: ElementKey| panels.on_demand.contains(&k) || panels.main_menu == Some(k);
    if let Some(a) = ui.active_panel().filter(|a| ui.exists(*a))
        && is_on_demand(a)
        && ui.is_visible(a)
    {
        ui.set_shown(a, false);
        return EscapeOutcome::HidActive(a);
    }
    match panels.main_menu.filter(|m| ui.exists(*m)) {
        Some(m) if ui.is_visible(m) => {
            ui.set_shown(m, false);
            EscapeOutcome::HidMainMenu(m)
        }
        Some(m) => {
            ui.set_shown(m, true);
            ui.activate(m);
            EscapeOutcome::ShowedMainMenu(m)
        }
        None => EscapeOutcome::OpenMainMenu,
    }
}
