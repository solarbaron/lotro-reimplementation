// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest history window (`quest-journal.md` §4.9): layout `questhistory`, root
//! `QuestHistoryField` (widget `QuestHistory`, 448 × 475) with the `QuestDisplay` list
//! `QuestHistory_ListBox` (display type 2 `PastObjectives`, bestowal dialogue shown, rewards
//! not shown), a scroll bar, close buttons and the title.
//!
//! The list is a details pane ([`DetailsPane`]) filled with the quest's history-order
//! sections: header, bestowal label and dialogue, background, then each objective's label
//! and description from 1 to the current objective (§4.9 Tessera rule). Reward sections
//! are never shown (`UI_QuestDisplay_ShowRewards` false), whatever the client passes.
//!
//! **Tessera rule (close buttons).** §4.9 does not name the close buttons. The window's
//! parts are the list, its scroll bar, the close buttons and the title, so every element of
//! class Button below the root that lies neither inside the list nor inside a scroll bar is
//! taken as a close button; a click on one closes the window
//! ([`HistoryWindow::handle_event`]). The fallback window has one close button.
//!
//! The element IDs are looked up by label at run time ([`HistoryIds`]); the defaults are
//! Tessera placeholders that no data uses.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::npc::kit::{ClickKind, FNode, FallbackArt, Listen, click_of, colours, open_root};
use super::super::npcvocab::NpcVocab;
use super::super::{find_below, subtree};
use super::details::{DetailsPane, DetailsSection, DetailsTemplates, QuestDetails};
use super::{QuestKey, QuestLabel, QuestTexts, quest_label_text};

/// The design size of `QuestHistoryField` (§4.9).
pub const HISTORY_SIZE: (i32, i32) = (448, 475);

/// Most close buttons taken from a layout (Tessera bound).
pub const MAX_CLOSE_BUTTONS: usize = 8;

/// The element IDs of the history window, by `UIElementID` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryIds {
    /// `QuestHistoryField` (the root).
    pub root: u32,
    /// `QuestHistory_ListBox` (the `QuestDisplay` list).
    pub list: u32,
    /// The fallback window's close button (a Tessera element; no label).
    pub close: u32,
}

impl Default for HistoryIds {
    fn default() -> Self {
        Self {
            root: 0xFFFB_3E10,
            list: 0xFFFB_3E11,
            close: 0xFFFB_3E12,
        }
    }
}

impl HistoryIds {
    /// The `UIElementID` labels of [`HistoryIds::root`] and [`HistoryIds::list`].
    pub const LABELS: (&'static str, &'static str) = ("QuestHistoryField", "QuestHistory_ListBox");

    /// The IDs with each label `lookup` resolves replacing its placeholder.
    #[must_use]
    pub fn resolved(lookup: &dyn Fn(&str) -> Option<u32>) -> Self {
        let mut ids = Self::default();
        if let Some(v) = lookup(Self::LABELS.0) {
            ids.root = v;
        }
        if let Some(v) = lookup(Self::LABELS.1) {
            ids.list = v;
        }
        ids
    }
}

/// The history window on its root.
#[derive(Debug, Clone)]
pub struct HistoryWindow {
    /// The root (`QuestHistoryField`).
    pub root: ElementKey,
    /// The list (`QuestHistory_ListBox`) as a details pane.
    pub pane: DetailsPane,
    /// The close buttons.
    pub closes: Vec<ElementKey>,
    quest: Option<QuestKey>,
    listen: Listen,
}

/// Whether `k` lies inside a scroll bar below `root` (bounded walk).
fn in_scrollbar(ui: &Ui, root: ElementKey, k: ElementKey) -> bool {
    let mut cur = ui.parent(k);
    let mut n = 0;
    while let Some(c) = cur {
        if c == root || n > crate::ui::MAX_TREE_DEPTH {
            return false;
        }
        if ui.class(c) == Some(WidgetClass::Scrollbar) {
            return true;
        }
        n += 1;
        cur = ui.parent(c);
    }
    false
}

impl HistoryWindow {
    /// Finds the window's parts below `root` (module docs); `None` without the list.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        ids: HistoryIds,
        root: ElementKey,
        templates: DetailsTemplates,
        art: &FallbackArt,
    ) -> Option<Self> {
        let list = if ui.element_id(root) == Some(ids.list) {
            root
        } else {
            find_below(ui, root, ids.list)?
        };
        let closes: Vec<ElementKey> = subtree(ui, root)
            .into_iter()
            .skip(1)
            .filter(|k| {
                ui.class(*k) == Some(WidgetClass::Button)
                    && !ui.is_ancestor_or_self(list, *k)
                    && !in_scrollbar(ui, root, *k)
            })
            .take(MAX_CLOSE_BUTTONS)
            .collect();
        let pane = DetailsPane::new(ui, nv, list, templates, art);
        let mut w = Self {
            root,
            pane,
            closes,
            quest: None,
            listen: Listen::default(),
        };
        for k in w.closes.clone() {
            w.listen.clicks(ui, Some(k));
        }
        Some(w)
    }

    /// The quest shown.
    #[must_use]
    pub fn quest(&self) -> Option<QuestKey> {
        self.quest
    }

    /// Shows `quest`'s history-order sections (reward sections left out, §4.9).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        quest: QuestKey,
        details: Option<&QuestDetails>,
        texts: &dyn QuestTexts,
    ) {
        self.quest = Some(quest);
        let d = details.map(|d| QuestDetails {
            sections: d
                .sections
                .iter()
                .filter(|s| !matches!(s, DetailsSection::Rewards(_)))
                .cloned()
                .collect(),
        });
        self.pane.show(ui, d.as_ref(), texts);
    }

    /// Fits texts again once fonts that were loading have arrived (call every frame).
    pub fn refit(&mut self, ui: &mut Ui) {
        self.pane.refit_when_ready(ui);
    }

    /// Whether a UI event is a click on one of the close buttons.
    #[must_use]
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> bool {
        let Some((src, kind)) = click_of(ui, ev) else {
            return false;
        };
        matches!(kind, ClickKind::Button | ClickKind::Left)
            && self
                .closes
                .iter()
                .any(|c| ui.exists(*c) && ui.is_ancestor_or_self(*c, src))
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
        self.pane.detach(ui);
    }
}

/// The fallback history window (decision 0031 tier 1): `QuestHistoryField` at its design
/// size with the title, one close button and the list.
///
/// # Errors
/// The element cap or an attachment error.
pub fn fallback(
    ui: &mut Ui,
    nv: &NpcVocab,
    ids: HistoryIds,
    art: &FallbackArt,
    texts: &dyn QuestTexts,
) -> Result<ElementKey, GameError> {
    let v = ui.vocab().clone();
    let cp = nv.pids.text_font_color;
    let (w, h) = HISTORY_SIZE;
    let root = FNode::field(ids.root, 160, 118, w, h)
        .image(&art.window)
        .child(
            FNode::field(0, 0, 0, w, 24)
                .image(&art.title_bar)
                .child(FNode::text(
                    0,
                    8,
                    4,
                    w - 40,
                    16,
                    cp,
                    colours::TITLE,
                    &quest_label_text(texts, QuestLabel::HistoryTitle),
                )),
        )
        .child(FNode::button(
            ids.close,
            w - 24,
            4,
            16,
            16,
            (&art.close, &art.close),
            cp,
            v.states.ghosted,
            "",
        ))
        .child(
            FNode::new(WidgetClass::ListBox, ids.list, 12, 32, w - 24, h - 44).image(&art.panel),
        );
    open_root(ui, &root)
}
