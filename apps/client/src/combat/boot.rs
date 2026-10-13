// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's combat layouts and vocabulary, resolved by label at UI start
//! (`combat-ui.md` §2, CU1; labels are looked up at run time, never hard-coded IDs), and
//! the opening of the `gameplayactivatables` roots the combat widgets attach to (E7, E11,
//! E12).

use tessera_formats::Did;
use tessera_ui::game::combat::{CombatLayouts, CombatRoots, CombatVocab};
use tessera_ui::{ElementKey, Ui};

/// `UILAYOUT` labels of the combat layouts (CU1).
pub mod labels {
    /// E1, E3, E13.
    pub const FLOATYTEXT: &str = "floatytext";
    /// E10.
    pub const EFFECTS: &str = "effects";
    /// E18.
    pub const DIALOG: &str = "Dialog";
    /// E7, E11, E12.
    pub const GAMEPLAY_ACTIVATABLES: &str = "gameplayactivatables";
}

/// What the combat widgets need from the install's UI data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CombatRefs {
    /// The combat vocabulary.
    pub vocab: CombatVocab,
    /// The layouts game code creates elements from.
    pub layouts: CombatLayouts,
    /// `gameplayactivatables`.
    pub activatables: Option<Did>,
}

impl CombatRefs {
    /// Resolves the refs: `layout` by `UILAYOUT` label, then the vocabulary's four tables
    /// in [`CombatVocab::resolve_all`] order.
    pub fn resolve(
        layout: &mut dyn FnMut(&str) -> Option<Did>,
        element_type: &mut dyn FnMut(&str) -> Option<u32>,
        ids: &mut dyn FnMut(&str) -> Option<u32>,
        states: &mut dyn FnMut(&str) -> Option<u32>,
        pids: &mut dyn FnMut(&str) -> Option<u32>,
    ) -> Self {
        let mut vocab = CombatVocab::default();
        vocab.resolve_all(element_type, ids, states, pids);
        Self {
            vocab,
            layouts: CombatLayouts {
                floatytext: layout(labels::FLOATYTEXT),
                effects: layout(labels::EFFECTS),
                dialog: layout(labels::DIALOG),
            },
            activatables: layout(labels::GAMEPLAY_ACTIVATABLES),
        }
    }

    /// Opens the `gameplayactivatables` roots of E7, E11 and E12 (created as roots,
    /// `combat-ui.md` §5.7, §7.1) and fills `roots` with them; a root that cannot be opened
    /// stays `None` (its element takes the fallback look, TR41).
    pub fn open_activatables(&self, ui: &mut Ui, roots: &mut CombatRoots) {
        let Some(layout) = self.activatables else {
            return;
        };
        let ids = self.vocab.ids;
        let mut open = |id: u32| -> Option<ElementKey> {
            match ui.open_panel(layout, id) {
                Ok(k) => {
                    ui.apply_saved(k);
                    Some(k)
                }
                Err(e) => {
                    tracing::debug!("combat panel {id:#x} not opened: {e}");
                    None
                }
            }
        };
        roots.selection_target = open(ids.selection_target);
        roots.player_progress = open(ids.progress_dragbox);
        roots.opponent_progress = open(ids.opponent_progress_dragbox);
    }
}
