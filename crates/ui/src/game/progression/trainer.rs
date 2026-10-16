// SPDX-License-Identifier: GPL-3.0-or-later
//! The skill-training window (Tessera rules; see [`super`]): one row per trainer entry
//! (`scenario-feed.md` §6.6 `trainer`, or the player's `untrained` list) with the skill's
//! icon, name, level text and price in a P3 money display (`npc-windows.md` §9.2, red
//! counts in `TooExpensive` when the price exceeds the player's coin); rows already trained
//! or whose requirements are not met are ghosted (entry flags) and cannot be selected; a
//! click selects a row, the train button (ghosted without a trainable selection) or a
//! double-click sends [`ProgressionAction::TrainSkill`]. The window changes only when the
//! client shows the answering state.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::crafting::P6Art;
use super::super::crafting::fallback::Kit;
use super::super::npc::coins::MoneyDisplay;
use super::super::npc::kit::{
    ClickKind, Listen, clear_rows, colour_value, commit_rows, find, ghost, instantiate, mark_row,
    put_text, row_of,
};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::super::popup::{click, listen_row};
use super::super::{set_icon, show, switch_if_defined};
use super::fallback::{progression_colours, skill_row};
use super::{MAX_SKILL_ROWS, ProgressionAction, ProgressionTexts, SkillTrainerView};

/// The skill-training window.
#[derive(Debug, Clone)]
pub struct SkillTrainerWindow {
    /// The root.
    pub root: ElementKey,
    /// The title.
    pub title: Option<ElementKey>,
    /// The close button.
    pub close: Option<ElementKey>,
    /// The row list.
    pub list: Option<ElementKey>,
    /// The train button.
    pub train: Option<ElementKey>,
    /// The empty-list text.
    pub empty: Option<ElementKey>,
    /// The row elements with their row indexes.
    pub rows: Vec<(ElementKey, usize)>,
    /// The selected row index.
    pub selected: Option<usize>,
    view: SkillTrainerView,
    prices: Vec<MoneyDisplay>,
    art: P6Art,
    nv: NpcVocab,
    p6: P6Vocab,
    listen: Listen,
}

impl SkillTrainerWindow {
    /// Finds the window's parts below `root` (the fallback window).
    pub fn attach(ui: &mut Ui, nv: &NpcVocab, p6: &P6Vocab, root: ElementKey, art: &P6Art) -> Self {
        let f = |id: u32| find(ui, root, id);
        let mut w = Self {
            root,
            title: f(own::TRAINER_TITLE),
            close: f(own::TRAINER_CLOSE),
            list: f(own::TRAINER_LIST),
            train: f(own::TRAINER_TRAIN),
            empty: f(own::TRAINER_EMPTY),
            rows: Vec::new(),
            selected: None,
            view: SkillTrainerView::default(),
            prices: Vec::new(),
            art: art.clone(),
            nv: nv.clone(),
            p6: p6.clone(),
            listen: Listen::default(),
        };
        w.listen.clicks(ui, w.close);
        w.listen.clicks(ui, w.train);
        w
    }

    /// The view shown.
    #[must_use]
    pub fn view(&self) -> &SkillTrainerView {
        &self.view
    }

    /// Shows a trainer view: the title, the rows, the empty text; the selection is kept
    /// on the row with the same entry while it is still trainable.
    pub fn show(&mut self, ui: &mut Ui, view: &SkillTrainerView, texts: &dyn ProgressionTexts) {
        let before = self
            .selected
            .and_then(|i| self.view.rows.get(i))
            .map(|r| r.entry);
        let same_window = view.window == self.view.window;
        if !view.title.is_empty() {
            put_text(ui, self.title, &view.title);
        }
        let mut v = view.clone();
        v.rows.truncate(MAX_SKILL_ROWS);
        self.view = v;
        self.selected = before.filter(|_| same_window).and_then(|e| {
            self.view
                .rows
                .iter()
                .position(|r| r.entry == e && !r.ghosted())
        });
        self.fill(ui, texts);
        self.refresh(ui);
    }

    fn fill(&mut self, ui: &mut Ui, texts: &dyn ProgressionTexts) {
        let Some(list) = self.list else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        clear_rows(ui, list, &mut old);
        self.prices.clear();
        let nv = self.nv.clone();
        let p6 = self.p6.clone();
        let art = self.art.clone();
        let k = Kit::new(ui, &nv, &p6, &art);
        let cp = nv.pids.text_font_color;
        let st = ui.vocab().states;
        let mut made = Vec::new();
        let rows = self.view.rows.clone();
        for (i, r) in rows.iter().enumerate() {
            let Some(e) = instantiate(ui, list, &skill_row(&k)) else {
                break;
            };
            if let Some(ic) = find(ui, e, own::TRAINER_ROW_ICON) {
                set_icon(ui, ic, r.icon.as_ref());
            }
            let name = find(ui, e, own::TRAINER_ROW_NAME);
            put_text(ui, name, &r.name);
            if let Some(n) = name {
                switch_if_defined(ui, n, if r.ghosted() { st.ghosted } else { st.normal });
            }
            let level = find(ui, e, own::TRAINER_ROW_LEVEL);
            put_text(ui, level, &r.level_text);
            if let Some(l) = level {
                let c = if r.unmet {
                    progression_colours::UNMET
                } else {
                    super::super::npc::kit::colours::BODY
                };
                ui.set_property(l, cp, colour_value(c));
            }
            if let Some(m) =
                find(ui, e, own::TRAINER_ROW_PRICE).and_then(|p| MoneyDisplay::find(ui, &nv, p))
            {
                let mut d = MoneyDisplay::attach(ui, &nv, m, &art.base);
                let expensive = self.view.player_coin.is_some_and(|c| c < r.coin);
                d.set(ui, r.coins, expensive, texts);
                self.prices.push(d);
            }
            listen_row(ui, &mut self.listen, e);
            made.push((e, i));
        }
        let ks: Vec<ElementKey> = made.iter().map(|(e, _)| *e).collect();
        let kept = commit_rows(ui, list, &ks);
        self.rows = made.into_iter().filter(|(e, _)| kept.contains(e)).collect();
        show(ui, self.empty, self.view.rows.is_empty());
        put_text(ui, self.empty, &self.view.empty_text);
    }

    fn refresh(&self, ui: &mut Ui) {
        let st = ui.vocab().states;
        for (e, i) in &self.rows {
            let ghosted = self
                .view
                .rows
                .get(*i)
                .is_some_and(super::SkillRowView::ghosted);
            if ghosted {
                switch_if_defined(ui, *e, st.ghosted);
            } else {
                mark_row(ui, *e, self.selected == Some(*i));
            }
        }
        ghost(ui, self.train, self.selected.is_none());
    }

    /// Selects row `i` (or none); a ghosted row cannot be selected.
    pub fn select(&mut self, ui: &mut Ui, i: Option<usize>) {
        self.selected = i.filter(|i| self.view.rows.get(*i).is_some_and(|r| !r.ghosted()));
        self.refresh(ui);
    }

    fn train_action(&self) -> Option<ProgressionAction> {
        let r = self.view.rows.get(self.selected?)?;
        (!r.ghosted()).then_some(ProgressionAction::TrainSkill {
            window: self.view.window,
            entry: r.entry,
        })
    }

    /// The command for a UI event: row selection, double-click and the train button
    /// (`TrainSkill`), close.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<ProgressionAction> {
        let (src, kind) = click(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if matches!(kind, ClickKind::Button | ClickKind::Left) {
            if hit(self.close) {
                return Some(ProgressionAction::CloseTrainer {
                    window: self.view.window,
                });
            }
            if hit(self.train) {
                return self.train_action();
            }
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let r = row_of(ui, &ks, src)?;
        let i = self.rows[r].1;
        match kind {
            ClickKind::Left | ClickKind::Button => {
                self.select(ui, Some(i));
                None
            }
            ClickKind::Double => {
                self.select(ui, Some(i));
                self.train_action()
            }
            ClickKind::Right => None,
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
