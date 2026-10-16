// SPDX-License-Identifier: GPL-3.0-or-later
//! The trait panel (Tessera rules; see [`super`]): one tab per trait tree, the shown tree's
//! branch buttons, its cells on a grid (icon, rank text, a locked overlay while the cell
//! cannot take a rank, a prerequisite mark green when met and red when not), the points
//! text and the slotted traits.
//!
//! Interaction (Tessera rule; the client half spends nothing itself, `scenario-feed.md`
//! §6.6): a left click on an available cell below its highest rank asks for rank + 1, a
//! right click on a cell with a rank asks for rank − 1 ([`ProgressionAction::SetTraitRank`]);
//! a branch button that is not the chosen one asks for that branch
//! ([`ProgressionAction::ChooseBranch`]); a tab asks the client to show that tree. The panel
//! changes only when the client shows the answering state.
//!
//! Cells sit at (column × [`CELL_STEP`].0, row × [`CELL_STEP`].1) in the grid, which holds
//! [`GRID_CELLS`](super::fallback::GRID_CELLS) (12 × 12, every cell key of
//! `combat-data.md` §8.5 in either orientation); a cell that does not fit the grid is not
//! drawn ([`TraitPanel::dropped`]).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::crafting::P6Art;
use super::super::crafting::fallback::Kit;
use super::super::npc::kit::{
    ClickKind, Listen, clear_rows, colour_value, commit_rows, find, instantiate, mark_row,
    put_text, row_of, text_part,
};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::super::popup::{click, listen_row};
use super::super::{set_icon, show};
use super::fallback::{
    BRANCH_SIZE, CELL_SIZE, CELL_STEP, GRID_MARGIN, GRID_SIZE, TAB_SIZE, branch_button,
    progression_colours, slot_row, trait_cell, trait_tab,
};
use super::{
    MAX_BRANCHES, MAX_CELLS, MAX_SLOT_ROWS, MAX_TREES, ProgressionAction, TraitCellView,
    TraitPanelView,
};

/// The trait panel.
#[derive(Debug, Clone)]
pub struct TraitPanel {
    /// The root.
    pub root: ElementKey,
    /// The title.
    pub title: Option<ElementKey>,
    /// The close button.
    pub close: Option<ElementKey>,
    /// The tab row.
    pub tabs_parent: Option<ElementKey>,
    /// The branch row.
    pub branches_parent: Option<ElementKey>,
    /// The cell grid.
    pub grid: Option<ElementKey>,
    /// The points text.
    pub points: Option<ElementKey>,
    /// The slotted-trait list.
    pub slots_list: Option<ElementKey>,
    /// The tab elements.
    pub tabs: Vec<ElementKey>,
    /// The branch buttons with their views.
    pub branches: Vec<ElementKey>,
    /// The cell elements with their cell indexes in the view.
    pub cells: Vec<(ElementKey, usize)>,
    /// The slot rows.
    pub slots: Vec<ElementKey>,
    view: TraitPanelView,
    dropped: usize,
    art: P6Art,
    nv: NpcVocab,
    p6: P6Vocab,
    listen: Listen,
}

impl TraitPanel {
    /// Finds the panel's parts below `root` (the fallback panel).
    pub fn attach(ui: &mut Ui, nv: &NpcVocab, p6: &P6Vocab, root: ElementKey, art: &P6Art) -> Self {
        let f = |id: u32| find(ui, root, id);
        let mut p = Self {
            root,
            title: f(own::TRAIT_TITLE),
            close: f(own::TRAIT_CLOSE),
            tabs_parent: f(own::TRAIT_TABS),
            branches_parent: f(own::TRAIT_BRANCHES),
            grid: f(own::TRAIT_GRID),
            points: f(own::TRAIT_POINTS),
            slots_list: f(own::TRAIT_SLOTS),
            tabs: Vec::new(),
            branches: Vec::new(),
            cells: Vec::new(),
            slots: Vec::new(),
            view: TraitPanelView::default(),
            dropped: 0,
            art: art.clone(),
            nv: nv.clone(),
            p6: p6.clone(),
            listen: Listen::default(),
        };
        p.listen.clicks(ui, p.close);
        p
    }

    /// The view shown.
    #[must_use]
    pub fn view(&self) -> &TraitPanelView {
        &self.view
    }

    /// Cells of the view that were not drawn (outside the grid or past [`MAX_CELLS`]).
    #[must_use]
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Shows a panel view: the title, tabs, branches, cells, points and slots.
    pub fn show(&mut self, ui: &mut Ui, view: &TraitPanelView) {
        if !view.title.is_empty() {
            put_text(ui, self.title, &view.title);
        }
        let nv = self.nv.clone();
        let p6 = self.p6.clone();
        let art = self.art.clone();
        let k = Kit::new(ui, &nv, &p6, &art);
        let st = ui.vocab().states;
        // Tabs.
        let old: Vec<ElementKey> = self.tabs.drain(..).filter(|t| ui.exists(*t)).collect();
        ui.destroy_many(&old);
        if let Some(parent) = self.tabs_parent {
            for (i, name) in view.trees.iter().take(MAX_TREES).enumerate() {
                let Some(t) = instantiate(ui, parent, &trait_tab(&k, name)) else {
                    break;
                };
                let x = i32::try_from(i).unwrap_or(0).saturating_mul(TAB_SIZE.0 + 2);
                ui.move_to(t, x, 0);
                ui.switch_state_if_defined(
                    t,
                    if i == view.current {
                        st.highlight
                    } else {
                        st.normal
                    },
                );
                listen_row(ui, &mut self.listen, t);
                self.tabs.push(t);
            }
        }
        // Branches.
        let old: Vec<ElementKey> = self.branches.drain(..).filter(|t| ui.exists(*t)).collect();
        ui.destroy_many(&old);
        if let Some(parent) = self.branches_parent {
            for (i, b) in view.branches.iter().take(MAX_BRANCHES).enumerate() {
                let Some(e) = instantiate(ui, parent, &branch_button(&k, &b.text)) else {
                    break;
                };
                let x = i32::try_from(i)
                    .unwrap_or(0)
                    .saturating_mul(BRANCH_SIZE.0 + 4);
                ui.move_to(e, x, 0);
                mark_row(ui, e, b.chosen);
                listen_row(ui, &mut self.listen, e);
                self.branches.push(e);
            }
        }
        // Cells.
        let old: Vec<ElementKey> = self
            .cells
            .drain(..)
            .map(|(e, _)| e)
            .filter(|t| ui.exists(*t))
            .collect();
        ui.destroy_many(&old);
        self.dropped = 0;
        if let Some(grid) = self.grid {
            for (i, c) in view.cells.iter().enumerate() {
                let x = i32::from(c.col)
                    .saturating_mul(CELL_STEP.0)
                    .saturating_add(GRID_MARGIN);
                let y = i32::from(c.row)
                    .saturating_mul(CELL_STEP.1)
                    .saturating_add(GRID_MARGIN);
                if i >= MAX_CELLS || x + CELL_SIZE.0 > GRID_SIZE.0 || y + CELL_SIZE.1 > GRID_SIZE.1
                {
                    self.dropped += 1;
                    continue;
                }
                let Some(e) = instantiate(ui, grid, &trait_cell(&k)) else {
                    self.dropped += 1;
                    continue;
                };
                ui.move_to(e, x, y);
                self.fill_cell(ui, e, c);
                listen_row(ui, &mut self.listen, e);
                self.cells.push((e, i));
            }
        } else {
            self.dropped = view.cells.len();
        }
        put_text(ui, self.points, &view.points_text);
        // Slots.
        if let Some(list) = self.slots_list {
            let mut old = std::mem::take(&mut self.slots);
            clear_rows(ui, list, &mut old);
            let mut made = Vec::new();
            for s in view.slots.iter().take(MAX_SLOT_ROWS) {
                let Some(r) = instantiate(ui, list, &slot_row(&k)) else {
                    break;
                };
                if let Some(ic) = find(ui, r, own::TRAIT_CELL_ICON) {
                    set_icon(ui, ic, s.icon.as_ref());
                }
                put_text(ui, text_part(ui, r, None), &s.text);
                made.push(r);
            }
            self.slots = commit_rows(ui, list, &made);
        }
        self.view = view.clone();
    }

    fn fill_cell(&self, ui: &mut Ui, e: ElementKey, c: &TraitCellView) {
        if let Some(ic) = find(ui, e, own::TRAIT_CELL_ICON) {
            set_icon(ui, ic, c.icon.as_ref());
        }
        let rank = find(ui, e, own::TRAIT_CELL_RANK);
        put_text(ui, rank, &c.rank_text);
        if let Some(r) = rank {
            let colour = if c.max_rank > 0 && c.rank >= c.max_rank {
                progression_colours::MAXED
            } else if c.rank > 0 {
                progression_colours::SPENT
            } else {
                super::super::npc::kit::colours::WHITE
            };
            ui.set_property(r, self.nv.pids.text_font_color, colour_value(colour));
        }
        let locked = !c.available && c.rank == 0;
        show(ui, find(ui, e, own::TRAIT_CELL_LOCK), locked);
        let mark = find(ui, e, own::TRAIT_CELL_PREREQ);
        match c.prerequisites {
            Some(met) => {
                show(ui, mark, true);
                if let Some(m) = mark {
                    super::super::combat::look::set_fill(
                        ui,
                        m,
                        Some(if met {
                            progression_colours::PREREQ_MET
                        } else {
                            progression_colours::PREREQ_UNMET
                        }),
                    );
                }
            }
            None => show(ui, mark, false),
        }
        let tip = if c.tooltip.is_empty() {
            c.name.clone()
        } else {
            c.tooltip.clone()
        };
        ui.set_tooltip_text(e, Some(super::super::npc::kit::cut(&tip)));
    }

    /// The action a cell click asks for (Tessera rule): left on an available cell below
    /// its highest rank → rank + 1 (with unmet prerequisites never); right on a cell with a
    /// rank → rank − 1.
    #[must_use]
    pub fn cell_action(&self, cell: &TraitCellView, right: bool) -> Option<ProgressionAction> {
        let rank = if right {
            cell.rank.checked_sub(1)?
        } else {
            if !cell.available
                || cell.prerequisites == Some(false)
                || (cell.max_rank > 0 && cell.rank >= cell.max_rank)
            {
                return None;
            }
            cell.rank.checked_add(1)?
        };
        Some(ProgressionAction::SetTraitRank {
            tree: self.view.tree,
            cell: cell.id,
            rank,
        })
    }

    /// The command for a UI event: tabs, branches, cells, close.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<ProgressionAction> {
        let (src, kind) = click(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        if matches!(kind, ClickKind::Left | ClickKind::Button) {
            if self.close.is_some_and(|c| ui.is_ancestor_or_self(c, src)) {
                return Some(ProgressionAction::CloseTraits);
            }
            if let Some(i) = row_of(ui, &self.tabs, src) {
                if i == self.view.current {
                    return None;
                }
                return Some(ProgressionAction::ShowTree { index: i });
            }
            if let Some(i) = row_of(ui, &self.branches, src) {
                let b = self.view.branches.get(i)?;
                if b.chosen {
                    return None;
                }
                return Some(ProgressionAction::ChooseBranch {
                    tree: self.view.tree,
                    branch: b.key,
                });
            }
        }
        let ks: Vec<ElementKey> = self.cells.iter().map(|(k, _)| *k).collect();
        let r = row_of(ui, &ks, src)?;
        let cell = self.view.cells.get(self.cells[r].1)?;
        match kind {
            ClickKind::Left | ClickKind::Button => self.cell_action(cell, false),
            ClickKind::Right => self.cell_action(cell, true),
            ClickKind::Double => None,
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
