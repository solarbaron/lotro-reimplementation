// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest log window P9 (`quest-journal.md` §4): which quests are listed (§4.2),
//! categories and order (§4.3), the entry row (§4.4), filters (§4.5), header counts
//! (§4.6), selection, and the track, abandon and history buttons (§4.8).

use std::collections::BTreeSet;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::examine::ExamTemplate;
use super::super::npc::kit::{
    ClickKind, FNode, FallbackArt, FontLadder, Listen, TreeRow, click_of, colours,
    commit_tree_rows, find, ghost, make_row, mark_row, put_text, row_of, text_part,
};
use super::super::npc::wallet::fold;
use super::super::npcvocab::NpcVocab;
use super::super::{show, subtree};
use super::details::{DetailsPane, DetailsTemplates, QuestDetails};
use super::{QuestAction, QuestKey, QuestLabel, QuestTexts, quest_label_text};

/// Most quests the log lists (Tessera bound).
pub const MAX_QUESTS: usize = 1024;

/// Most quests the tracker holds (`Quest_TrackerArray` capacity, §6.2: 10).
pub const TRACKER_CAPACITY: usize = 10;

/// The `UIQuestScope` bit `Normal` (0x10): set when no other bit is, and always in the
/// filter mask (§4.5 Tessera rule).
pub const SCOPE_NORMAL: u32 = 0x10;

/// The filter mask at creation: every bit (§4.5 Tessera rule).
pub const ALL_SCOPES: u32 = 0x3FF;

/// The scope bits of the row icons (§4.4): epic, raid, fellowship, small fellowship, task,
/// instance, repeatable.
pub const SCOPE_EPIC: u32 = 0x40;
/// `Raid`.
pub const SCOPE_RAID: u32 = 0x80;
/// `Fellowship`.
pub const SCOPE_FELLOWSHIP: u32 = 0x02;
/// `SmallFellowship`.
pub const SCOPE_SMALL_FELLOWSHIP: u32 = 0x04;
/// `Task`.
pub const SCOPE_TASK: u32 = 0x08;
/// `Instance`.
pub const SCOPE_INSTANCE: u32 = 0x100;
/// `Repeatable`.
pub const SCOPE_REPEATABLE: u32 = 0x01;

/// A quest's feed state (`quest-journal.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QuestStatus {
    /// `Active` (raw status underway).
    #[default]
    Active,
    /// `Completed`.
    Completed,
    /// `Failed`.
    Failed,
    /// `Removed`.
    Removed,
}

/// The log's mode (`Quest_ShowCompleted`, §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LogMode {
    /// Quests underway (`Active`).
    #[default]
    Active,
    /// Completed quests.
    Completed,
}

/// One quest as the log shows it, rendered by the client (`quest-journal.md` §4.3–§4.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogEntry {
    /// The quest.
    pub quest: QuestKey,
    /// `Quest_Name` (or the literal).
    pub name: String,
    /// The row title of §4.4 (with or without the level).
    pub title: String,
    /// `Quest_Category` (absent: 0).
    pub category: u32,
    /// The category's display text, or the fallback token's text (§4.3).
    pub category_title: String,
    /// The effective level *L*.
    pub level: i32,
    /// The scope bits (§4.4).
    pub scope: u32,
    /// The difficulty state (`Con…_ColorName`) for `ConColorField`.
    pub difficulty: Option<u32>,
    /// The feed state.
    pub status: QuestStatus,
    /// `Quest_IsAccomplishment`: a deed, never listed (§4.2).
    pub deed: bool,
    /// The feed's tracked flag.
    pub tracked: bool,
    /// Ready to complete (§6.4).
    pub ready: bool,
    /// `Quest_CanBeCancelled` (default true).
    pub can_abandon: bool,
    /// `Quest_IsInstanceQuest`.
    pub instance: bool,
    /// The order in which the quest was first reported.
    pub order: u64,
}

/// One row of the log's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogRow {
    /// A category row.
    Category {
        /// The `Quest_Category` key.
        key: u32,
        /// The row text.
        title: String,
        /// Whether its quests are shown.
        expanded: bool,
    },
    /// A quest row (index into the entries).
    Quest(usize),
}

/// The log's state: mode, filters, collapsed categories, selection (§4.2–§4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogModel {
    /// The mode.
    pub mode: LogMode,
    /// The filter mask (`Normal` always set).
    pub filter_mask: u32,
    /// The name filter (`Quest_Filter_Text`).
    pub name_filter: String,
    /// Collapsed category keys (all expanded at creation).
    pub collapsed: BTreeSet<u32>,
    /// The selected quest.
    pub selected: Option<QuestKey>,
}

impl Default for LogModel {
    fn default() -> Self {
        Self {
            mode: LogMode::Active,
            filter_mask: ALL_SCOPES,
            name_filter: String::new(),
            collapsed: BTreeSet::new(),
            selected: None,
        }
    }
}

impl LogModel {
    /// Whether `e` is listed in the current mode (§4.2 Tessera rule): active mode lists
    /// `Active` quests, completed mode `Completed` ones; deeds never.
    #[must_use]
    pub fn listed(&self, e: &LogEntry) -> bool {
        !e.deed
            && match self.mode {
                LogMode::Active => e.status == QuestStatus::Active,
                LogMode::Completed => e.status == QuestStatus::Completed,
            }
    }

    /// Whether a listed row is shown by the filters (§4.5): its scope bits share a bit with
    /// the mask (with `Normal` always set), and its lower-cased title contains the
    /// lower-cased name filter.
    #[must_use]
    pub fn passes(&self, e: &LogEntry) -> bool {
        self.passes_folded(e, &fold(&self.name_filter))
    }

    /// [`LogModel::passes`] with the name filter already folded.
    fn passes_folded(&self, e: &LogEntry, needle: &str) -> bool {
        let mask = self.filter_mask | SCOPE_NORMAL;
        (e.scope & mask) != 0 && (needle.is_empty() || fold(&e.title).contains(needle))
    }

    /// The listed entries in display order (§4.3): categories by their row text without
    /// case (then by first report), quests by level, then name without case, then first
    /// report. Returns (category key, category title, entry indices).
    #[must_use]
    pub fn groups(&self, entries: &[LogEntry]) -> Vec<(u32, String, Vec<usize>)> {
        // Each name and title is folded once (not once per comparison).
        let mut idx: Vec<(i32, String, u64, usize)> = entries
            .iter()
            .enumerate()
            .take(MAX_QUESTS)
            .filter(|(_, e)| self.listed(e))
            .map(|(i, e)| (e.level, fold(&e.name), e.order, i))
            .collect();
        // The index as the last key keeps equal keys in entry order (a stable sort).
        idx.sort_unstable();
        let mut groups: Vec<(u32, String, u64, Vec<usize>)> = Vec::new();
        let mut at: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for (_, _, _, i) in idx {
            let e = &entries[i];
            match at.get(&e.category) {
                Some(g) => {
                    let g = &mut groups[*g];
                    g.2 = g.2.min(e.order);
                    g.3.push(i);
                }
                None => {
                    at.insert(e.category, groups.len());
                    groups.push((e.category, e.category_title.clone(), e.order, vec![i]));
                }
            }
        }
        groups.sort_by_cached_key(|a| (fold(&a.1), a.2));
        groups.into_iter().map(|(k, t, _, v)| (k, t, v)).collect()
    }

    /// The groups of [`LogModel::groups`], each with its quests that pass the filters
    /// (computed once per rebuild).
    fn filtered(&self, entries: &[LogEntry]) -> Vec<Group> {
        let needle = fold(&self.name_filter);
        self.groups(entries)
            .into_iter()
            .map(|(key, title, all)| {
                let shown = all
                    .iter()
                    .copied()
                    .filter(|i| self.passes_folded(&entries[*i], &needle))
                    .collect();
                Group {
                    key,
                    title,
                    all,
                    shown,
                }
            })
            .collect()
    }

    /// The rows (§4.3, §4.5): each category whose quests are not all filtered out, then
    /// its shown quests unless it is collapsed.
    #[must_use]
    pub fn rows(&self, entries: &[LogEntry]) -> Vec<LogRow> {
        self.rows_of(&self.filtered(entries), false)
    }

    /// The rows of `groups`; with `all`, the quests of collapsed categories too (for a
    /// Tree, which hides them itself).
    fn rows_of(&self, groups: &[Group], all: bool) -> Vec<LogRow> {
        let mut rows = Vec::new();
        for g in groups {
            if g.shown.is_empty() {
                continue;
            }
            let expanded = !self.collapsed.contains(&g.key);
            rows.push(LogRow::Category {
                key: g.key,
                title: g.title.clone(),
                expanded,
            });
            if expanded || all {
                rows.extend(g.shown.iter().copied().map(LogRow::Quest));
            }
        }
        rows
    }

    /// The shown-label numbers (§4.6): quest rows shown after filtering and quest rows in
    /// the tree (category rows not counted).
    #[must_use]
    pub fn counts(&self, entries: &[LogEntry]) -> (usize, usize) {
        counts_of(&self.filtered(entries))
    }

    /// Quests underway (§4.6 Tessera rule): non-deed quests with feed state `Active`.
    #[must_use]
    pub fn underway(entries: &[LogEntry]) -> usize {
        entries
            .iter()
            .filter(|e| !e.deed && e.status == QuestStatus::Active)
            .count()
    }

    /// Keeps the selection valid (§4.3 Tessera rule): when nothing is selected or the
    /// selected quest left the list, the first shown quest row in display order.
    pub fn ensure_selection(&mut self, entries: &[LogEntry]) {
        let g = self.filtered(entries);
        self.ensure_selection_of(entries, &g);
    }

    fn ensure_selection_of(&mut self, entries: &[LogEntry], groups: &[Group]) {
        let mut shown = groups.iter().flat_map(|g| g.shown.iter().copied());
        let valid = self
            .selected
            .is_some_and(|s| shown.clone().any(|i| entries[i].quest == s));
        if !valid {
            self.selected = shown.next().map(|i| entries[i].quest);
        }
    }

    /// The selected entry.
    #[must_use]
    pub fn selected_entry<'a>(&self, entries: &'a [LogEntry]) -> Option<&'a LogEntry> {
        let s = self.selected?;
        entries.iter().find(|e| e.quest == s)
    }

    /// The track button (§4.8 Tessera column): (ghosted, tracked). Ghosted when nothing is
    /// selected, the selected quest is `Completed`, an instance quest, or not tracked while
    /// [`TRACKER_CAPACITY`] quests are tracked.
    #[must_use]
    pub fn track_button(&self, entries: &[LogEntry]) -> (bool, bool) {
        let tracked_count = entries.iter().filter(|e| e.tracked).count();
        match self.selected_entry(entries) {
            None => (true, false),
            Some(e) => (
                e.status == QuestStatus::Completed
                    || e.instance
                    || (!e.tracked && tracked_count >= TRACKER_CAPACITY),
                e.tracked,
            ),
        }
    }

    /// Whether the abandon button is ghosted (§4.8): nothing selected, the quest
    /// completed, or `Quest_CanBeCancelled` false.
    #[must_use]
    pub fn abandon_ghosted(&self, entries: &[LogEntry]) -> bool {
        self.selected_entry(entries)
            .is_none_or(|e| e.status == QuestStatus::Completed || !e.can_abandon)
    }
}

/// One category of the list with its listed quests and those that pass the filters.
#[derive(Debug, Clone)]
struct Group {
    key: u32,
    title: String,
    all: Vec<usize>,
    shown: Vec<usize>,
}

/// (quest rows shown after filtering, quest rows listed).
fn counts_of(groups: &[Group]) -> (usize, usize) {
    (
        groups.iter().map(|g| g.shown.len()).sum(),
        groups.iter().map(|g| g.all.len()).sum(),
    )
}

/// The row templates of the log (`QuestTreeCategoryTitle`, `QuestTreeQuestTitle` of layout
/// `quest`) and of the details pane. `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogTemplates {
    /// A category row.
    pub category: Option<ExamTemplate>,
    /// A quest row.
    pub quest: Option<ExamTemplate>,
    /// The details pane's templates.
    pub details: DetailsTemplates,
}

/// One filter button (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterButton {
    /// The button.
    pub element: ElementKey,
    /// `UI_QuestJournal_Filter_QuestScope`.
    pub mask: u32,
}

/// Width of a fallback log row.
pub const ROW_WIDTH: i32 = 313;

/// The quest log window.
#[derive(Debug, Clone)]
pub struct QuestLogWindow {
    /// The root (`QuestPanel`).
    pub root: ElementKey,
    /// The state.
    pub model: LogModel,
    /// The entries last given.
    pub entries: Vec<LogEntry>,
    /// `QuestTree`.
    pub tree: Option<ElementKey>,
    /// The rows shown.
    pub rows: Vec<(ElementKey, LogRow)>,
    /// The filter buttons shown.
    pub filters: Vec<FilterButton>,
    /// The details pane.
    pub details: Option<DetailsPane>,
    /// The limit `Quest_CountText` shows (`Quest_MaxActiveQuests` when reported).
    pub limit: Option<i64>,
    count_text: Option<ElementKey>,
    shown_label: Option<ElementKey>,
    track: Option<ElementKey>,
    abandon: Option<ElementKey>,
    history: Option<ElementKey>,
    close: Option<ElementKey>,
    expand: Option<ElementKey>,
    collapse: Option<ElementKey>,
    show_completed: Option<ElementKey>,
    filter_text: Option<ElementKey>,
    task_text: Option<ElementKey>,
    show_tracker: Option<ElementKey>,
    tracker_shown: bool,
    fonts: FontLadder,
    pending_abandon: Option<QuestKey>,
    templates: LogTemplates,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl QuestLogWindow {
    /// Finds the window's parts below `root` (§4.1) and applies the P3 rules: the store,
    /// share, fellowship and group-finder controls hidden or ghosted, the `…_MP` filter
    /// buttons (alignment other than `Good`) hidden (§4.5 Tessera rule).
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        templates: LogTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        for id in [
            ids.quest_slot_store,
            ids.tasks_reset_store,
            ids.tasks_limit_store,
            ids.fellowship_status_mode,
            ids.lff,
        ] {
            show(ui, f(ui, id), false);
        }
        for id in [ids.share_button, ids.find_fellowship_button] {
            ghost(ui, f(ui, id), true);
        }
        // Filter buttons: every element below `Quest_Filters` with a scope mask.
        let mut filters = Vec::new();
        if let Some(fl) = f(ui, ids.filters) {
            for k in subtree(ui, fl).into_iter().skip(1) {
                let mask = ui.prop(k, nv.pids.filter_scope).key();
                if mask == 0 {
                    continue;
                }
                let align = ui.prop(k, nv.pids.filter_alignment).key();
                if align != 0 && align != 1 {
                    ui.set_shown(k, false);
                    continue;
                }
                filters.push(FilterButton { element: k, mask });
            }
        }
        let details = f(ui, ids.selected_display)
            .map(|d| DetailsPane::new(ui, nv, d, templates.details, art));
        let mut w = Self {
            root,
            model: LogModel::default(),
            entries: Vec::new(),
            tree: f(ui, ids.quest_tree),
            rows: Vec::new(),
            filters,
            details,
            limit: None,
            count_text: f(ui, ids.count_text),
            shown_label: f(ui, ids.shown_label),
            track: f(ui, ids.track_button),
            abandon: f(ui, ids.abandon_button),
            history: f(ui, ids.history_button),
            close: f(ui, ids.quest_panel_close),
            expand: f(ui, ids.expand_all),
            collapse: f(ui, ids.collapse_all),
            show_completed: f(ui, ids.show_completed),
            filter_text: f(ui, ids.filter_text),
            task_text: f(ui, ids.task_count_text),
            show_tracker: f(ui, ids.show_tracker),
            tracker_shown: true,
            fonts: FontLadder::default(),
            pending_abandon: None,
            templates,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        for k in [
            w.track,
            w.abandon,
            w.history,
            w.close,
            w.expand,
            w.collapse,
            w.show_completed,
            w.show_tracker,
        ] {
            w.listen.clicks(ui, k);
        }
        for b in w.filters.clone() {
            w.listen.clicks(ui, Some(b.element));
        }
        w.update_filter_buttons(ui);
        w.set_tracker_shown(ui, true);
        // §4.7 Tessera rule: the arrows step the sample and the details pane's texts
        // through their font lists (`fonts.md` §9.2).
        let (up, down) = (f(ui, ids.quest_font_up), f(ui, ids.quest_font_down));
        w.fonts = FontLadder::new(ui, &mut w.listen, up, down);
        let sample = f(ui, ids.quest_font_example);
        w.fonts.add(ui, nv, sample);
        w
    }

    /// Shows the tracker check box's state (`Quest_ShowTrackerCheckBox`, §4.8: the
    /// tracker's visibility, a Tessera setting the client restores; shown at first).
    pub fn set_tracker_shown(&mut self, ui: &mut Ui, shown: bool) {
        self.tracker_shown = shown;
        if let Some(b) = self.show_tracker {
            let p = ui.vocab().pids.button_highlighted;
            ui.set_property(b, p, tessera_formats::property::PropertyValue::Bool(shown));
        }
    }

    /// Whether the tracker check box is on.
    #[must_use]
    pub fn tracker_shown(&self) -> bool {
        self.tracker_shown
    }

    /// Restores the font index of the font-size arrows (§4.7 Tessera rule; `fonts.md`
    /// §9.2: kept only when a text's font list is long enough). Returns whether it was
    /// taken.
    pub fn set_font_index(&mut self, ui: &mut Ui, index: u32) -> bool {
        let i = usize::try_from(index).unwrap_or(usize::MAX);
        let nv = self.nv.clone();
        let taken = self.fonts.set(ui, &nv, i);
        if taken && let Some(d) = self.details.as_mut() {
            d.refit(ui);
        }
        taken
    }

    /// The font index of the font-size arrows (0 at first).
    #[must_use]
    pub fn font_index(&self) -> u32 {
        u32::try_from(self.fonts.index()).unwrap_or(u32::MAX)
    }

    /// Shows the entries (the client's quest state, at most [`MAX_QUESTS`]) and rebuilds
    /// the list, the counts and the buttons. The client then fills the details pane for the
    /// quest [`QuestLogWindow::selected`] names with [`QuestLogWindow::show_details`].
    pub fn show(&mut self, ui: &mut Ui, entries: &[LogEntry], texts: &dyn QuestTexts) {
        self.entries = entries.iter().take(MAX_QUESTS).cloned().collect();
        self.rebuild(ui, texts, true);
    }

    /// The selected quest.
    #[must_use]
    pub fn selected(&self) -> Option<QuestKey> {
        self.model.selected
    }

    /// Fits the details pane's texts again once fonts that were loading when they were
    /// fitted have arrived (call every frame; see `DetailsPane::refit_when_ready`).
    pub fn refit_details(&mut self, ui: &mut Ui) {
        if let Some(d) = self.details.as_mut() {
            d.refit_when_ready(ui);
        }
    }

    /// Fills the details pane with the selected quest's details.
    pub fn show_details(
        &mut self,
        ui: &mut Ui,
        details: Option<&QuestDetails>,
        texts: &dyn QuestTexts,
    ) {
        if let Some(d) = self.details.as_mut() {
            d.show(ui, details, texts);
            // The pane's texts take the arrows' font index too (§4.7).
            let rows = d.rows.clone();
            for r in rows {
                let t = text_part(ui, r, None).or(Some(r));
                self.fonts.add(ui, &self.nv, t);
            }
            // The rows were fitted at their layout font; the ladder may have changed it.
            d.refit(ui);
        }
    }

    /// Sets the mode (the show-completed box) and rebuilds.
    pub fn set_mode(&mut self, ui: &mut Ui, mode: LogMode, texts: &dyn QuestTexts) {
        self.model.mode = mode;
        if let Some(b) = self.show_completed {
            let p = ui.vocab().pids.button_highlighted;
            ui.set_property(
                b,
                p,
                tessera_formats::property::PropertyValue::Bool(mode == LogMode::Completed),
            );
        }
        self.rebuild(ui, texts, true);
    }

    /// Selects a quest (as a click on its row does) and rebuilds the buttons.
    pub fn select(&mut self, ui: &mut Ui, quest: QuestKey, texts: &dyn QuestTexts) {
        if self.entries.iter().any(|e| e.quest == quest) {
            self.model.selected = Some(quest);
            self.rebuild(ui, texts, false);
        }
    }

    /// Shows the quest in the log ("show in log", §6.6 Tessera rule): switches to the mode
    /// of its feed state and selects it. So that the quest is listed and shown, the name
    /// filter is cleared, its scope bits join the filter mask and its category is
    /// expanded. A quest that no mode lists (a deed, or feed state `Failed` or `Removed`,
    /// §4.2) changes nothing.
    pub fn show_quest(&mut self, ui: &mut Ui, quest: QuestKey, texts: &dyn QuestTexts) {
        let Some(e) = self.entries.iter().find(|e| e.quest == quest).cloned() else {
            return;
        };
        let mode = match e.status {
            _ if e.deed => None,
            QuestStatus::Active => Some(LogMode::Active),
            QuestStatus::Completed => Some(LogMode::Completed),
            QuestStatus::Failed | QuestStatus::Removed => None,
        };
        let Some(mode) = mode else {
            tracing::debug!(quest, "show in log: the quest is not listed in any mode");
            return;
        };
        if !self.model.name_filter.is_empty() {
            self.model.name_filter.clear();
            if let Some(t) = self.filter_text {
                ui.set_text(t, None);
            }
        }
        if e.scope & (self.model.filter_mask | SCOPE_NORMAL) == 0 {
            self.model.filter_mask |= e.scope & ALL_SCOPES;
            self.update_filter_buttons(ui);
        }
        self.model.selected = Some(quest);
        self.model.collapsed.remove(&e.category);
        self.set_mode(ui, mode, texts);
    }

    /// Rebuilds the list, the counts and the buttons from one computation of the groups;
    /// with `ensure`, the selection is first kept valid (§4.3 Tessera rule).
    fn rebuild(&mut self, ui: &mut Ui, texts: &dyn QuestTexts, ensure: bool) {
        let groups = self.model.filtered(&self.entries);
        if ensure {
            self.model.ensure_selection_of(&self.entries, &groups);
        }
        self.fill_rows(ui, &groups);
        let (shown, total) = counts_of(&groups);
        put_text(
            ui,
            self.shown_label,
            &texts.shown_text(shown, total, self.model.mode),
        );
        put_text(
            ui,
            self.count_text,
            &texts.count_text(LogModel::underway(&self.entries), self.limit),
        );
        // §4.6: the task limiter's text when the client has its values; the layout's
        // placeholder is never shown.
        put_text(ui, self.task_text, &texts.task_text().unwrap_or_default());
        let (ghosted, tracked) = self.model.track_button(&self.entries);
        ghost(ui, self.track, ghosted);
        if let Some(t) = self.track {
            let (l, tip) = if tracked {
                (QuestLabel::Untrack, QuestLabel::UntrackTooltip)
            } else {
                (QuestLabel::Track, QuestLabel::TrackTooltip)
            };
            let el = text_part(ui, t, None).unwrap_or(t);
            ui.set_text(el, Some(quest_label_text(texts, l)));
            ui.set_tooltip_text(t, Some(quest_label_text(texts, tip)));
        }
        let ag = self.model.abandon_ghosted(&self.entries);
        ghost(ui, self.abandon, ag);
        ghost(ui, self.history, self.model.selected.is_none());
    }

    /// Whether the list is an engine Tree (its nodes nest and collapse themselves).
    fn is_tree(&self, ui: &Ui) -> bool {
        self.tree
            .is_some_and(|t| ui.class(t) == Some(WidgetClass::Tree))
    }

    fn fill_rows(&mut self, ui: &mut Ui, groups: &[Group]) {
        let Some(tree) = self.tree else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        super::super::npc::kit::clear_rows(ui, tree, &mut old);
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let v = ui.vocab().clone();
        // A Tree gets every quest row as a child node of its category row (§4.3) and
        // collapses the category itself; a list box gets only the expanded categories'
        // quest rows.
        let is_tree = self.is_tree(ui);
        let mut parents: Vec<Option<ElementKey>> = Vec::new();
        let mut category: Option<ElementKey> = None;
        for r in self.model.rows_of(groups, is_tree) {
            let k = match &r {
                LogRow::Category { title, .. } => {
                    let k = make_row(ui, tree, self.templates.category, || {
                        FNode::text(ids.category_title, 0, 0, 300, 18, cp, colours::TITLE, "")
                    });
                    if let Some(k) = k {
                        put_text(ui, text_part(ui, k, None).or(Some(k)), title);
                        self.listen.clicks(ui, Some(k));
                    }
                    category = k;
                    parents.push(None);
                    k
                }
                LogRow::Quest(i) => {
                    let e = self.entries[*i].clone();
                    let k = make_row(ui, tree, self.templates.quest, || {
                        quest_row(&v, &self.nv, &art)
                    });
                    if let Some(k) = k {
                        self.fill_quest_row(ui, k, &e);
                        let selected = self.model.selected == Some(e.quest);
                        mark_row(ui, k, selected);
                        self.listen.clicks(ui, Some(k));
                    }
                    parents.push(category);
                    k
                }
            };
            match k {
                Some(k) => self.rows.push((k, r)),
                None => {
                    parents.pop();
                    break;
                }
            }
        }
        let tree_rows: Vec<TreeRow> = self
            .rows
            .iter()
            .zip(parents)
            .map(|((k, r), parent)| TreeRow {
                row: *k,
                parent,
                expanded: !matches!(
                    r,
                    LogRow::Category {
                        expanded: false,
                        ..
                    }
                ),
            })
            .collect();
        let kept: std::collections::HashSet<ElementKey> =
            commit_tree_rows(ui, tree, &tree_rows).into_iter().collect();
        self.rows.retain(|(k, _)| kept.contains(k));
    }

    fn fill_quest_row(&self, ui: &mut Ui, k: ElementKey, e: &LogEntry) {
        let ids = self.nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, k, id);
        put_text(ui, f(ui, ids.title_text), &e.title);
        if let (Some(c), Some(s)) = (f(ui, ids.con_colour), e.difficulty) {
            ui.switch_state_if_defined(c, s);
        }
        // §4.4: an icon is shown exactly when its bit is set.
        for (id, bit) in [
            (ids.epic_image, SCOPE_EPIC),
            (ids.raid_image, SCOPE_RAID),
            (ids.fellowship_image, SCOPE_FELLOWSHIP),
            (ids.small_fellowship_image, SCOPE_SMALL_FELLOWSHIP),
            (ids.task_image, SCOPE_TASK),
            (ids.instance_image, SCOPE_INSTANCE),
            (ids.repeatable_image, SCOPE_REPEATABLE),
        ] {
            show(ui, f(ui, id), e.scope & bit != 0);
        }
        // §4.4 Tessera rules: the timer and the fellow images hidden; the ready icon from
        // §6.4; the completed icon on every row in completed mode.
        for id in [ids.time_remaining, ids.fellow_background, ids.lff] {
            show(ui, f(ui, id), false);
        }
        show(
            ui,
            f(ui, ids.ready_field),
            e.ready && self.model.mode == LogMode::Active,
        );
        show(
            ui,
            f(ui, ids.completed_field),
            self.model.mode == LogMode::Completed,
        );
        if let Some(t) = f(ui, ids.row_tracker_button) {
            let st = ui.vocab().states;
            ui.switch_state_if_defined(t, if e.tracked { st.highlight } else { st.normal });
        }
    }

    fn update_filter_buttons(&mut self, ui: &mut Ui) {
        let p = ui.vocab().pids.button_highlighted;
        for b in &self.filters {
            let on = self.model.filter_mask & b.mask != 0;
            ui.set_property(
                b.element,
                p,
                tessera_formats::property::PropertyValue::Bool(on),
            );
        }
    }

    /// A filter button's click (§4.5): toggles its mask in the filter mask.
    pub fn toggle_filter(&mut self, ui: &mut Ui, mask: u32, texts: &dyn QuestTexts) {
        self.model.filter_mask ^= mask;
        self.model.filter_mask |= SCOPE_NORMAL;
        self.update_filter_buttons(ui);
        self.rebuild(ui, texts, true);
    }

    /// Sets the name filter (§4.5).
    pub fn set_name_filter(&mut self, ui: &mut Ui, text: &str, texts: &dyn QuestTexts) {
        let t = super::super::npc::kit::cut(text);
        if t != self.model.name_filter {
            self.model.name_filter = t;
            self.rebuild(ui, texts, true);
        }
    }

    /// Expands or collapses every category.
    pub fn expand_all(&mut self, ui: &mut Ui, expand: bool, texts: &dyn QuestTexts) {
        if expand {
            self.model.collapsed.clear();
        } else {
            self.model.collapsed = self.entries.iter().map(|e| e.category).collect();
        }
        self.rebuild(ui, texts, false);
    }

    /// The abandon confirmation's answer: OK gives `AbandonQuest` for the quest the button
    /// asked about; cancel sends nothing (§4.8).
    pub fn confirm_abandon(&mut self, ok: bool) -> Option<QuestAction> {
        let q = self.pending_abandon.take()?;
        ok.then_some(QuestAction::AbandonQuest { quest: q })
    }

    /// The command for a UI event (§4.8): the track button sends `TrackQuest` with the
    /// opposite flag, the abandon button asks for confirmation, the history button opens
    /// the history; rows select, category rows collapse or expand; filters, the
    /// show-completed box and expand/collapse act locally. The name filter's text is read
    /// on every event.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn QuestTexts,
    ) -> Option<QuestAction> {
        if let Some(t) = self.filter_text {
            let typed = ui.text_of(t);
            if typed != self.model.name_filter {
                self.set_name_filter(ui, &typed, texts);
            }
        }
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        if !matches!(kind, ClickKind::Button | ClickKind::Left) {
            return None;
        }
        if let Some(changed) = self.fonts.click(ui, &self.nv, src) {
            if changed.is_some()
                && let Some(d) = self.details.as_mut()
            {
                d.refit(ui);
            }
            return changed.map(|i| QuestAction::FontSize {
                index: u32::try_from(i).unwrap_or(u32::MAX),
            });
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if hit(self.close) {
            return Some(QuestAction::CloseLog);
        }
        if hit(self.show_tracker) {
            let shown = !self.tracker_shown;
            self.set_tracker_shown(ui, shown);
            return Some(QuestAction::SetTrackerShown { shown });
        }
        if hit(self.track) {
            let (ghosted, tracked) = self.model.track_button(&self.entries);
            if ghosted {
                return None;
            }
            return Some(QuestAction::TrackQuest {
                quest: self.model.selected?,
                tracked: !tracked,
            });
        }
        if hit(self.abandon) {
            if self.model.abandon_ghosted(&self.entries) {
                return None;
            }
            let q = self.model.selected?;
            self.pending_abandon = Some(q);
            return Some(QuestAction::AskAbandon { quest: q });
        }
        if hit(self.history) {
            return Some(QuestAction::ShowHistory {
                quest: self.model.selected?,
            });
        }
        if hit(self.expand) {
            self.expand_all(ui, true, texts);
            return None;
        }
        if hit(self.collapse) {
            self.expand_all(ui, false, texts);
            return None;
        }
        if hit(self.show_completed) {
            let m = match self.model.mode {
                LogMode::Active => LogMode::Completed,
                LogMode::Completed => LogMode::Active,
            };
            self.set_mode(ui, m, texts);
            return None;
        }
        if let Some(b) = self
            .filters
            .iter()
            .find(|b| ui.is_ancestor_or_self(b.element, src))
            .copied()
        {
            self.toggle_filter(ui, b.mask, texts);
            return None;
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let i = row_of(ui, &ks, src)?;
        match self.rows[i].1.clone() {
            LogRow::Category { key, .. } => {
                let expanded = self.model.collapsed.contains(&key);
                if expanded {
                    self.model.collapsed.remove(&key);
                } else {
                    self.model.collapsed.insert(key);
                }
                match self.tree.filter(|_| self.is_tree(ui)) {
                    // A Tree hides and shows the category's rows itself (the press may
                    // already have toggled it; setting the state is idempotent).
                    Some(tree) => {
                        let row = self.rows[i].0;
                        if let LogRow::Category { expanded: e, .. } = &mut self.rows[i].1 {
                            *e = expanded;
                        }
                        ui.tree_set_expanded(tree, row, expanded);
                    }
                    None => self.rebuild(ui, texts, false),
                }
            }
            LogRow::Quest(j) => {
                let q = self.entries.get(j)?.quest;
                self.select(ui, q, texts);
            }
        }
        None
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
        if let Some(d) = self.details.as_mut() {
            d.detach(ui);
        }
    }
}

/// A fallback quest row (`QuestTreeQuestTitle`, 313 × 36): the difficulty field, the
/// title, the scope icons, the ready and completed marks, indented by 8 (the tree
/// indent).
pub(crate) fn quest_row(v: &crate::vocab::Vocab, nv: &NpcVocab, art: &FallbackArt) -> FNode {
    let ids = nv.ids;
    let cp = nv.pids.text_font_color;
    let st = v.states;
    let icon = |id: u32, x: i32, img: &Option<crate::env::ImageRef>| {
        FNode::field(id, x, 22, 10, 10).image(img)
    };
    FNode::new(WidgetClass::Field, nv.ids.quest_title, 0, 0, ROW_WIDTH, 36)
        .image(&art.row)
        .state(st.normal, art.row.as_ref(), vec![])
        .state(st.highlight, art.row_highlight.as_ref(), vec![])
        .child(FNode::field(ids.con_colour, 8, 2, 32, 32).image(&art.slot))
        .child(FNode::text(
            ids.title_text,
            46,
            3,
            238,
            18,
            cp,
            colours::WHITE,
            "",
        ))
        .child(icon(ids.epic_image, 46, &art.ready))
        .child(icon(ids.raid_image, 58, &art.raid))
        .child(icon(ids.fellowship_image, 70, &art.fellowship))
        .child(icon(ids.small_fellowship_image, 82, &art.small_fellowship))
        .child(icon(ids.task_image, 94, &art.solo))
        .child(icon(ids.instance_image, 106, &art.panel))
        .child(icon(ids.repeatable_image, 118, &art.repeatable))
        .child(
            FNode::field(ids.ready_field, 290, 10, 16, 16)
                .image(&art.ready)
                .hidden(),
        )
        .child(
            FNode::field(ids.completed_field, 290, 10, 16, 16)
                .image(&art.ready)
                .hidden(),
        )
}
