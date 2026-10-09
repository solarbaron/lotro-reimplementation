// SPDX-License-Identifier: GPL-3.0-or-later
//! The small game types: tutorial highlights (`game-panels.md` §5), pip displays and the
//! page changer (§7), mood and time of day (§8.7.2–§8.7.3), and the remaining types of
//! §11 with Tessera's sandbox rule.

use crate::tree::ElementKey;
use crate::ui::Ui;

use super::vocab::{GameStates, GameVocab};
use super::{subtree, switch_if_defined};

/// A `TutorialHighlighter` (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TutorialHighlighter {
    /// The element.
    pub key: ElementKey,
    /// `UIComponent_TutorialHighlighter_Type` (0 when absent).
    pub kind: u32,
    /// Whether a tutorial highlight for this type is active.
    pub active: bool,
}

impl TutorialHighlighter {
    /// Post-creation (§5 step 1): reads the type, marks itself inactive, hides itself and
    /// switches to `normal`.
    pub fn post_create(ui: &mut Ui, gv: &GameVocab, key: ElementKey) -> Self {
        let kind = ui.prop(key, gv.pids.tutorial_type).key();
        let h = Self {
            key,
            kind,
            active: false,
        };
        ui.set_shown(key, false);
        let normal = ui.vocab().states.normal;
        switch_if_defined(ui, key, normal);
        h
    }

    /// A tutorial-highlight event (§5 step 2): `Some(t)` while the game highlights type
    /// `t` (and its tutorial condition holds), `None` when the event ends. The sandbox has
    /// no tutorials, so the client never calls this with `Some` (§5 Tessera rule).
    pub fn tutorial_event(&mut self, ui: &mut Ui, highlighted: Option<u32>) {
        let on = highlighted.is_some_and(|t| t == self.kind);
        if on == self.active {
            return;
        }
        self.active = on;
        let st = ui.vocab().states;
        ui.set_shown(self.key, on);
        switch_if_defined(ui, self.key, if on { st.highlight } else { st.normal });
    }
}

/// `PipDisplay` (§7): hidden in its constructor; Tessera keeps it hidden.
pub fn pip_display_post_create(ui: &mut Ui, key: ElementKey) {
    ui.set_shown(key, false);
}

/// `ToolbarQuickslotPageChanger` (§7): hidden in its constructor; Tessera rule: shown,
/// with the page indicator showing Tessera's current page number. The indicator is taken
/// to be the first Text element below the changer (the spec names no element for it; see
/// the §7 OPEN).
pub fn page_changer_post_create(ui: &mut Ui, key: ElementKey, page: u32) {
    // The constructor's hide and Tessera's show leave it shown.
    ui.set_shown(key, true);
    let text = subtree(ui, key)
        .into_iter()
        .skip(1)
        .find(|k| ui.class(*k).is_some_and(crate::WidgetClass::is_text));
    if let Some(t) = text {
        ui.set_text(t, Some(page.to_string()));
    }
}

/// The game types that Tessera hides in the sandbox (§11: housing buttons, medal icons,
/// raid and chest locks).
pub fn hidden_type_post_create(ui: &mut Ui, key: ElementKey) {
    ui.set_shown(key, false);
}

/// `MapFogOfWar` (§8.5.3 Tessera rule): draws nothing.
pub fn fog_post_create(ui: &mut Ui, key: ElementKey) {
    ui.clear_base(key);
    ui.play_media(key, Vec::new());
}

/// The rim state index (0 = `Mood_n10`, 10 = `Mood_0`, 20 = `Mood_p10`) of a `Mood_Level`
/// value (§8.7.2): clamped to −10…10. Monotone and saturating (GP11).
#[must_use]
pub fn mood_index(m: i32) -> usize {
    // m.clamp(-10, 10) + 10 is in 0..=20.
    usize::try_from(m.clamp(-10, 10) + 10).unwrap_or(10)
}

/// The rim state of a `Mood_Level` value (§8.7.2).
#[must_use]
pub fn mood_state(states: &GameStates, m: i32) -> u32 {
    states.moods()[mood_index(m)]
}

/// The `TimeOfDayDisplay` state (§8.7.3): `Unknown_Time` without a local player or
/// without `World_IsNight`, else `Night_Time` or `Day_Time`. The two internal flags of the
/// §8.7.3 OPEN are clear (Tessera rule).
#[must_use]
pub fn time_of_day_state(states: &GameStates, has_player: bool, is_night: Option<bool>) -> u32 {
    match (has_player, is_night) {
        (true, Some(true)) => states.night_time,
        (true, Some(false)) => states.day_time,
        _ => states.unknown_time,
    }
}
