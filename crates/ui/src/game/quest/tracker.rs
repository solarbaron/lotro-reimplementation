// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest tracker P11 (`quest-journal.md` §6): one entry per tracked quest (at most 10,
//! in the order their `QuestTracked` first arrived), the name line and the progress line
//! with its height budget and ellipsis, the ready icon, and the entry button with its menu.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npc::kit::{
    ClickKind, FNode, FallbackArt, Listen, clear_rows, click_of, colour_value, colours,
    commit_rows, find, fit_height, make_row, put_text, row_of,
};
use super::super::npcvocab::NpcVocab;
use super::super::show;
use super::log::TRACKER_CAPACITY;
use super::{QuestAction, QuestKey, QuestLabel, QuestTexts, quest_label_text};

/// `UI_QuestTracker_TotalMaxObjectives` when the template has none (QI5: 20 here; the
/// value is read at run time).
pub const DEFAULT_TOTAL_MAX: i32 = 20;

/// `UI_QuestTracker_SizePerObjective` when the template has none (QI5: 12 here).
pub const DEFAULT_SIZE_PER: i32 = 12;

/// Line 2's height budget with `n` tracked entries (§6.3 step 4):
/// ⌊`total_max` ÷ *n*⌋ × `size_per` pixels (240, 120, 72, … 24 for 1, 2, 3, … 10 entries
/// with 20 and 12). No entry gives the whole budget; negative values count as 0.
#[must_use]
pub fn line2_budget(n: usize, total_max: i32, size_per: i32) -> i32 {
    let n = i32::try_from(n.max(1)).unwrap_or(i32::MAX);
    (total_max.max(0) / n).saturating_mul(size_per.max(0))
}

/// One tracked quest, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackerEntry {
    /// The quest.
    pub quest: QuestKey,
    /// `Quest_Name` (line 1, no level).
    pub name: String,
    /// The progress text's lines (§5.2; the completed or failed word for those states;
    /// the deed counter of §5.3 step 5 for tracked deeds).
    pub progress: Vec<String>,
    /// The ready icon (§6.4 Tessera rule).
    pub ready: bool,
    /// A deed (no log to show it in, §6.6).
    pub deed: bool,
}

/// The tracker's display order (§6.5 Tessera rule): quests in the order in which their
/// `QuestTracked` true first arrived; at most [`TRACKER_CAPACITY`] shown, the rest kept
/// until another entry leaves (§6.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackerOrder {
    order: Vec<QuestKey>,
}

/// Most tracked quests the order keeps (Tessera bound beyond the capacity of 10).
pub const MAX_KEPT: usize = 1024;

impl TrackerOrder {
    /// A `QuestTracked` report: `true` appends a quest not yet tracked, `false` removes it
    /// (as `Removed` does).
    pub fn tracked(&mut self, quest: QuestKey, on: bool) {
        if on {
            if !self.order.contains(&quest) {
                if self.order.len() >= MAX_KEPT {
                    tracing::warn!("quest tracker: more than {MAX_KEPT} tracked quests; dropped");
                    return;
                }
                if self.order.len() >= TRACKER_CAPACITY {
                    tracing::warn!("quest tracker: an 11th tracked quest is kept but not shown");
                }
                self.order.push(quest);
            }
        } else {
            self.order.retain(|q| *q != quest);
        }
    }

    /// The quests shown, top first (at most [`TRACKER_CAPACITY`]).
    #[must_use]
    pub fn shown(&self) -> &[QuestKey] {
        &self.order[..self.order.len().min(TRACKER_CAPACITY)]
    }

    /// Every tracked quest in order.
    #[must_use]
    pub fn all(&self) -> &[QuestKey] {
        &self.order
    }
}

/// The menu items of the tracker's context menu in P3 (§6.6 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuItem {
    /// Show in log.
    ShowInLog,
    /// Remove.
    Remove,
    /// Remove all.
    RemoveAll,
}

/// The menu, in order, with its labels.
pub const MENU: [(MenuItem, QuestLabel); 3] = [
    (MenuItem::ShowInLog, QuestLabel::ShowInLog),
    (MenuItem::Remove, QuestLabel::Remove),
    (MenuItem::RemoveAll, QuestLabel::RemoveAll),
];

/// One shown entry's elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryParts {
    /// The entry.
    pub entry: ElementKey,
    /// `QuestTracker_QuestTextLine1`.
    pub line1: Option<ElementKey>,
    /// `QuestTracker_QuestTextLine2`.
    pub line2: Option<ElementKey>,
    /// `QuestTracker_QuestTextEllipsis`.
    pub ellipsis: Option<ElementKey>,
    /// `QuestTracker_ReadyToCompleteField`.
    pub ready: Option<ElementKey>,
    /// `QuestTracker_Button`.
    pub button: Option<ElementKey>,
    /// Whether line 2 was cut (the ellipsis shown).
    pub cut: bool,
    /// Line 2's height.
    pub line2_height: i32,
}

/// Width of a fallback tracker entry (`QuestTracker_QuestTemplate`, 339).
pub const ENTRY_WIDTH: i32 = 339;

/// The quest tracker.
#[derive(Debug, Clone)]
pub struct TrackerWindow {
    /// The tracker list (`GamePlay_QuestTracker`).
    pub list: ElementKey,
    /// The entries shown, top first.
    pub entries: Vec<(TrackerEntry, EntryParts)>,
    /// `UI_QuestTracker_TotalMaxObjectives` of the template (or the default).
    pub total_max: i32,
    /// `UI_QuestTracker_SizePerObjective`.
    pub size_per: i32,
    /// Every tracked quest given to the last [`TrackerWindow::show`], in display order
    /// (shown or not, at most [`MAX_KEPT`]).
    tracked: Vec<QuestKey>,
    template: Option<ExamTemplate>,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl TrackerWindow {
    /// A tracker on the list `list` (`GamePlay_QuestTracker`, found below a root by
    /// [`TrackerWindow::find_list`]); entries are created from `template`
    /// (`QuestTracker_QuestTemplate` of layout `questtracker`) or as fallback entries.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        list: ElementKey,
        template: Option<ExamTemplate>,
        art: &FallbackArt,
    ) -> Self {
        let _ = ui;
        Self {
            list,
            entries: Vec::new(),
            total_max: DEFAULT_TOTAL_MAX,
            size_per: DEFAULT_SIZE_PER,
            tracked: Vec::new(),
            template,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        }
    }

    /// `GamePlay_QuestTracker` at or below `root`.
    #[must_use]
    pub fn find_list(ui: &Ui, nv: &NpcVocab, root: ElementKey) -> Option<ElementKey> {
        find(ui, root, nv.ids.tracker)
    }

    /// Shows the entries (§6.3), top first; at most [`TRACKER_CAPACITY`]. The client
    /// passes every tracked quest in display order ([`TrackerOrder::all`]): the ones past
    /// the capacity are kept (§6.2) for Remove All but not shown.
    pub fn show(&mut self, ui: &mut Ui, entries: &[TrackerEntry], texts: &dyn QuestTexts) {
        self.tracked = entries.iter().take(MAX_KEPT).map(|e| e.quest).collect();
        let mut old: Vec<ElementKey> = self.entries.drain(..).map(|(_, p)| p.entry).collect();
        clear_rows(ui, self.list, &mut old);
        let shown: Vec<TrackerEntry> = entries.iter().take(TRACKER_CAPACITY).cloned().collect();
        let n = shown.len();
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let markup = self.nv.pids.parse_markup;
        let art = self.art.clone();
        let st = ui.vocab().states;
        let ellipsis = quest_label_text(texts, QuestLabel::Ellipsis);
        for e in shown {
            let Some(k) = make_row(ui, self.list, self.template, || {
                FNode::field(ids.tracker_template, 0, 0, ENTRY_WIDTH, 85)
                    .state(st.normal, None, vec![])
                    .state(st.highlight, None, vec![])
                    .child(
                        FNode::text(ids.tracker_line1, 75, 1, 223, 15, cp, colours::YELLOW, "")
                            .state(st.normal, None, vec![(cp, colour_value(colours::YELLOW))])
                            .state(
                                st.highlight,
                                None,
                                vec![(cp, colour_value(colours::ORANGE))],
                            ),
                    )
                    .child(
                        FNode::text(ids.tracker_line2, 73, 15, 239, 48, cp, colours::WHITE, "")
                            .prop(markup, tessera_formats::property::PropertyValue::Bool(true)),
                    )
                    .child(
                        FNode::field(ids.tracker_ready, 38, 1, 16, 16)
                            .image(&art.ready)
                            .hidden(),
                    )
                    .child(
                        FNode::text(
                            ids.tracker_ellipsis,
                            315,
                            27,
                            20,
                            12,
                            cp,
                            colours::WHITE,
                            &ellipsis,
                        )
                        .hidden(),
                    )
                    .child(FNode::button(
                        ids.tracker_button,
                        306,
                        -4,
                        30,
                        30,
                        (&art.button, &art.button_ghosted),
                        cp,
                        st.ghosted,
                        "",
                    ))
            }) else {
                break;
            };
            // The template's own values (QI5), read at run time.
            let p = self.nv.pids;
            let tm = ui.prop(k, p.tracker_total_max);
            if tm.is_set() {
                self.total_max = tm.int();
            }
            let sp = ui.prop(k, p.tracker_size_per);
            if sp.is_set() {
                self.size_per = sp.int();
            }
            let f = |ui: &Ui, id: u32| find(ui, k, id);
            // §6.3 Tessera rules: timer, points bar, lock, focus, travel, party status and
            // the tutorial highlight are hidden.
            for id in [
                ids.tracker_time_line,
                ids.tracker_points,
                ids.tracker_locked,
                ids.tracker_focus,
                ids.tracker_travel,
                ids.tracker_party,
                ids.tracker_tutorial,
            ] {
                show(ui, f(ui, id), false);
            }
            let mut parts = EntryParts {
                entry: k,
                line1: f(ui, ids.tracker_line1),
                line2: f(ui, ids.tracker_line2),
                ellipsis: f(ui, ids.tracker_ellipsis),
                ready: f(ui, ids.tracker_ready),
                button: f(ui, ids.tracker_button),
                cut: false,
                line2_height: 0,
            };
            put_text(ui, parts.line1, &e.name);
            let budget = line2_budget(n, self.total_max, self.size_per);
            if let Some(l2) = parts.line2 {
                put_text(ui, Some(l2), &e.progress.join("\n"));
                let (h, cut) = fit_height(ui, l2, 0, budget);
                parts.cut = cut;
                parts.line2_height = h;
                // A holder between the entry and line 2 (`QuestTracker_QuestObjectiveMode`)
                // follows line 2's height.
                if let Some(holder) = ui.parent(l2).filter(|p| *p != k)
                    && let (Some(hb), Some(lb)) = (ui.layout_box(holder), ui.layout_box(l2))
                {
                    ui.resize(holder, hb.w(), (lb.t + h).max(0));
                }
            }
            show(ui, parts.ellipsis, parts.cut);
            show(ui, parts.ready, e.ready && !e.deed);
            // §6.3 step 8: the entry's height is that of its visible children.
            let bottom = ui
                .children(k)
                .into_iter()
                .filter(|c| ui.is_visible(*c))
                .filter_map(|c| ui.layout_box(c).map(|b| b.b.saturating_add(1)))
                .max()
                .unwrap_or(0);
            if let Some(b) = ui.layout_box(k) {
                ui.resize(k, b.w(), bottom.max(1));
            }
            self.listen.clicks(ui, parts.button);
            self.entries.push((e, parts));
        }
        let ks: Vec<ElementKey> = self.entries.iter().map(|(_, p)| p.entry).collect();
        let kept = commit_rows(ui, self.list, &ks);
        self.entries.retain(|(_, p)| kept.contains(&p.entry));
    }

    /// The command for a UI event (§6.6): a click on an entry's button shows the quest in
    /// the log (nothing for a deed); a right click opens the context menu.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<QuestAction> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.list, src) {
            return None;
        }
        let buttons: Vec<ElementKey> = self.entries.iter().filter_map(|(_, p)| p.button).collect();
        let i = row_of(ui, &buttons, src)?;
        let (e, _) = self
            .entries
            .iter()
            .filter(|(_, p)| p.button.is_some())
            .nth(i)?;
        match kind {
            ClickKind::Right => Some(QuestAction::OpenTrackerMenu { quest: e.quest }),
            ClickKind::Button | ClickKind::Left if !e.deed => {
                Some(QuestAction::ShowInLog { quest: e.quest })
            }
            _ => None,
        }
    }

    /// A menu choice for `quest` (§6.6 Tessera rules): show in log (nothing for a deed);
    /// remove sends `TrackQuest { tracked: false }`; remove all sends it for every tracked
    /// quest given to [`TrackerWindow::show`] in display order, shown or not.
    #[must_use]
    pub fn menu_choice(&self, quest: QuestKey, item: MenuItem) -> Vec<QuestAction> {
        match item {
            MenuItem::ShowInLog => self
                .entries
                .iter()
                .find(|(e, _)| e.quest == quest && !e.deed)
                .map(|_| QuestAction::ShowInLog { quest })
                .into_iter()
                .collect(),
            MenuItem::Remove => vec![QuestAction::TrackQuest {
                quest,
                tracked: false,
            }],
            MenuItem::RemoveAll => self
                .tracked
                .iter()
                .map(|q| QuestAction::TrackQuest {
                    quest: *q,
                    tracked: false,
                })
                .collect(),
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
