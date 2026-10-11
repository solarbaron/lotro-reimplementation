// SPDX-License-Identifier: GPL-3.0-or-later
//! The client state kept from the feed (`quest-journal.md` §3) and the views computed from
//! it: the quest log's rows (§4.2–§4.6), the log's buttons (§4.8), the tracker's height
//! budget (§6.3) and quest-marker images (§8.3).

use std::collections::HashMap;

use tessera_formats::Did;

use super::present::{LogEntry, TrackerMetrics, lower};
use super::vars::PlayerContext;
use super::{
    FeedState, MAX_JOURNAL_QUESTS, MAX_OBJECTIVE_INDEX, ObjectiveReport, RecordRef,
    TRACKER_CAPACITY, tokens,
};

/// A feed event the journal keeps (`scenario-feed.md` §9.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalEvent {
    /// `QuestState`.
    State {
        /// The quest.
        quest: RecordRef,
        /// The new state.
        state: FeedState,
    },
    /// `QuestObjective`.
    Objective {
        /// The quest.
        quest: RecordRef,
        /// The report.
        report: ObjectiveReport,
    },
    /// `QuestTracked`.
    Tracked {
        /// The quest.
        quest: RecordRef,
        /// Tracked or not.
        tracked: bool,
    },
}

/// What one event changed (for chat lines, rings and redraws).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JournalChange {
    /// For a `QuestState` that changed the state: (previous, new).
    pub state: Option<(Option<FeedState>, FeedState)>,
    /// For a `QuestObjective` that changed `count` or `done`: the objective. A first report
    /// is compared with the state without a report (count 0, not done; §5.4), so a first
    /// `0/n` report that is not done changes nothing.
    pub objective: Option<u16>,
    /// For a `QuestTracked` that changed the flag, or a `Removed` state that cleared a
    /// true flag (then `false`): the new flag.
    pub tracked: Option<bool>,
    /// The event was ignored (bound reached or an index out of range; logged).
    pub ignored: bool,
}

/// One quest the journal keeps (§3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalQuest {
    /// The quest.
    pub quest: RecordRef,
    /// The feed state (`None`: only objective or tracking reports so far).
    pub state: Option<FeedState>,
    /// Objective reports, ascending by objective index.
    pub reports: Vec<ObjectiveReport>,
    /// The tracked flag.
    pub tracked: bool,
    /// The feed reported the quest `Completed` earlier in the session (repeat note, §4.7).
    pub completed_before: bool,
}

/// The journal's client state (§3 Tessera rule): per quest, in the order first reported,
/// the feed state, the objective reports and the tracked flag; the tracker's order is the
/// order in which `QuestTracked` true first arrived (§6.5).
///
/// Bounds: at most [`MAX_JOURNAL_QUESTS`] quests and [`MAX_JOURNAL_REPORTS`] objective
/// reports over all quests; events beyond them are ignored and logged once per bound.
///
/// Reports are dropped only by `Removed` (§3): a repeatable quest that goes from
/// `Completed` or `Failed` back to `Active` keeps its earlier reports until new ones
/// arrive (OPEN in `quest-journal.md` §3).
#[derive(Debug, Clone, Default)]
pub struct QuestJournal {
    quests: Vec<JournalQuest>,
    index: HashMap<RecordRef, usize>,
    tracked_order: Vec<RecordRef>,
    /// Objective reports kept over all quests.
    reports: usize,
    /// Events ignored because a bound was reached or an index was out of range.
    ignored: u64,
}

/// Most objective reports the journal keeps over all quests (a Tessera bound: 4 MiB of
/// reports).
pub const MAX_JOURNAL_REPORTS: usize = 262_144;

impl QuestJournal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&mut self, quest: &RecordRef) -> Option<usize> {
        if let Some(&i) = self.index.get(quest) {
            return Some(i);
        }
        if self.quests.len() >= MAX_JOURNAL_QUESTS {
            self.note_ignored("journal full; quest ignored");
            return None;
        }
        self.quests.push(JournalQuest {
            quest: quest.clone(),
            state: None,
            reports: Vec::new(),
            tracked: false,
            completed_before: false,
        });
        let i = self.quests.len() - 1;
        self.index.insert(quest.clone(), i);
        Some(i)
    }

    /// Counts an ignored event; warns on the first, then at debug level (rate limit).
    fn note_ignored(&mut self, what: &'static str) {
        self.ignored = self.ignored.saturating_add(1);
        if self.ignored == 1 {
            tracing::warn!("{what} (further ignored events are logged at debug level)");
        } else {
            tracing::debug!(ignored = self.ignored, "{what}");
        }
    }

    /// Events ignored so far (a bound reached or an index out of range).
    #[must_use]
    pub fn ignored_events(&self) -> u64 {
        self.ignored
    }

    /// Applies one event.
    pub fn apply(&mut self, event: &JournalEvent) -> JournalChange {
        let mut change = JournalChange::default();
        match event {
            JournalEvent::State { quest, state } => {
                let Some(i) = self.slot(quest) else {
                    change.ignored = true;
                    return change;
                };
                let q = &mut self.quests[i];
                let previous = q.state;
                q.state = Some(*state);
                match state {
                    FeedState::Completed => q.completed_before = true,
                    FeedState::Removed => {
                        self.reports = self.reports.saturating_sub(q.reports.len());
                        q.reports = Vec::new();
                        if q.tracked {
                            q.tracked = false;
                            change.tracked = Some(false);
                            self.tracked_order.retain(|t| t != quest);
                        }
                    }
                    _ => {}
                }
                if previous != Some(*state) {
                    change.state = Some((previous, *state));
                }
            }
            JournalEvent::Objective { quest, report } => {
                if report.objective > MAX_OBJECTIVE_INDEX {
                    self.note_ignored("objective index out of range");
                    change.ignored = true;
                    return change;
                }
                let Some(i) = self.slot(quest) else {
                    change.ignored = true;
                    return change;
                };
                let found = self.quests[i]
                    .reports
                    .binary_search_by_key(&report.objective, |r| r.objective);
                match found {
                    Ok(j) => {
                        let reports = &mut self.quests[i].reports;
                        let old = reports[j];
                        reports[j] = *report;
                        if old.count != report.count || old.done != report.done {
                            change.objective = Some(report.objective);
                        }
                    }
                    Err(_) if self.reports >= MAX_JOURNAL_REPORTS => {
                        self.note_ignored("journal report bound reached; report ignored");
                        change.ignored = true;
                    }
                    Err(j) => {
                        self.quests[i].reports.insert(j, *report);
                        self.reports += 1;
                        // Compared with no report: count 0, not done (§5.4).
                        if report.count != 0 || report.done {
                            change.objective = Some(report.objective);
                        }
                    }
                }
            }
            JournalEvent::Tracked { quest, tracked } => {
                let Some(i) = self.slot(quest) else {
                    change.ignored = true;
                    return change;
                };
                let q = &mut self.quests[i];
                if q.tracked != *tracked {
                    q.tracked = *tracked;
                    change.tracked = Some(*tracked);
                    if *tracked {
                        if self.tracked_order.len() >= TRACKER_CAPACITY {
                            tracing::warn!(?quest, "tracker full; entry kept but not shown");
                        }
                        self.tracked_order.push(quest.clone());
                    } else {
                        self.tracked_order.retain(|t| t != quest);
                    }
                }
            }
        }
        change
    }

    /// Every quest, in the order first reported.
    #[must_use]
    pub fn quests(&self) -> &[JournalQuest] {
        &self.quests
    }

    /// One quest.
    #[must_use]
    pub fn get(&self, quest: &RecordRef) -> Option<&JournalQuest> {
        self.index.get(quest).map(|&i| &self.quests[i])
    }

    /// The tracker's entries: tracked quests in arrival order, at most
    /// [`TRACKER_CAPACITY`] (§6.2, §6.5).
    #[must_use]
    pub fn tracked(&self) -> Vec<&JournalQuest> {
        self.tracked_order
            .iter()
            .take(TRACKER_CAPACITY)
            .filter_map(|q| self.get(q))
            .collect()
    }

    /// How many quests are tracked (shown or not).
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.tracked_order.len()
    }
}

/// Which quests the log lists (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LogMode {
    /// Feed state `Active` (the `Quest_ShowCompleted` box unchecked).
    #[default]
    Active,
    /// Feed state `Completed`.
    Completed,
}

impl LogMode {
    /// The `UI_Quest` token of the shown label's mode argument (§4.6).
    #[must_use]
    pub fn token(self) -> u32 {
        match self {
            Self::Active => tokens::MODE_ACTIVE,
            Self::Completed => tokens::MODE_COMPLETED,
        }
    }
}

/// The `Normal` scope bit, always in the filter mask (§4.5 Tessera rule).
pub const NORMAL_BIT: u32 = 0x10;
/// The filter mask at creation: every bit (§4.5 Tessera rule).
pub const FULL_FILTER_MASK: u32 = 0x3FF;

/// The log's filter (§4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFilter {
    /// The scope mask of the pressed filter buttons (the `Normal` bit is added).
    pub mask: u32,
    /// The name filter text.
    pub name: String,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            mask: FULL_FILTER_MASK,
            name: String::new(),
        }
    }
}

impl LogFilter {
    /// Whether a row with these scope bits and filter key is shown.
    #[must_use]
    pub fn shows(&self, scope_bits: u32, filter_key: &str) -> bool {
        (scope_bits & (self.mask | NORMAL_BIT)) != 0
            && (self.name.is_empty() || filter_key.contains(&lower(&self.name)))
    }
}

/// One quest row of the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRow {
    /// Index into the entries given to [`LogView::build`].
    pub entry: usize,
    /// Whether the filter shows it.
    pub shown: bool,
}

/// One category row and its quest rows (§4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct LogCategory {
    /// The `Quest_Category` key.
    pub category: u32,
    /// The row text.
    pub text: String,
    /// The quest rows in display order.
    pub rows: Vec<LogRow>,
    /// Whether any row is shown (a category whose rows are all hidden is hidden).
    pub shown: bool,
}

/// The rows of the quest log (§4.2–§4.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LogView {
    /// Category rows in display order.
    pub categories: Vec<LogCategory>,
    /// Quest rows shown after filtering.
    pub shown_rows: usize,
    /// Quest rows in the tree.
    pub total_rows: usize,
}

impl LogView {
    /// Builds the log from entries in the order first reported, each with its feed state.
    /// Deeds and quests of other states are left out (§4.2).
    #[must_use]
    pub fn build(
        entries: &[(&LogEntry, Option<FeedState>)],
        mode: LogMode,
        filter: &LogFilter,
    ) -> Self {
        let want = match mode {
            LogMode::Active => FeedState::Active,
            LogMode::Completed => FeedState::Completed,
        };
        let mut categories: Vec<LogCategory> = Vec::new();
        let mut by_key: HashMap<u32, usize> = HashMap::new();
        let mut total = 0;
        let mut shown_rows = 0;
        for (i, (e, state)) in entries.iter().enumerate() {
            if e.deed || *state != Some(want) {
                continue;
            }
            let c = *by_key.entry(e.category).or_insert_with(|| {
                categories.push(LogCategory {
                    category: e.category,
                    text: e.category_text.clone(),
                    rows: Vec::new(),
                    shown: false,
                });
                categories.len() - 1
            });
            let shown = filter.shows(e.scope_bits, &e.filter_key);
            total += 1;
            if shown {
                shown_rows += 1;
            }
            categories[c].rows.push(LogRow { entry: i, shown });
            categories[c].shown |= shown;
        }
        for c in &mut categories {
            c.rows.sort_by(|a, b| {
                let (x, y) = (entries[a.entry].0, entries[b.entry].0);
                x.level
                    .cmp(&y.level)
                    .then_with(|| x.sort_name.cmp(&y.sort_name))
            });
        }
        categories.sort_by_cached_key(|c| lower(&c.text));
        Self {
            categories,
            shown_rows,
            total_rows: total,
        }
    }

    /// The entry indices of the quest rows in display order.
    #[must_use]
    pub fn order(&self) -> Vec<usize> {
        self.categories
            .iter()
            .flat_map(|c| c.rows.iter().map(|r| r.entry))
            .collect()
    }

    /// The first shown quest row (the selection when nothing is selected, §4.3).
    #[must_use]
    pub fn first_shown(&self) -> Option<usize> {
        self.categories
            .iter()
            .flat_map(|c| c.rows.iter())
            .find(|r| r.shown)
            .map(|r| r.entry)
    }
}

/// The number of quests underway for `Quest_CountText` (§4.6 Tessera rule): non-deed
/// quests with feed state `Active`.
#[must_use]
pub fn quests_underway(entries: &[(&LogEntry, Option<FeedState>)]) -> usize {
    entries
        .iter()
        .filter(|(e, s)| !e.deed && *s == Some(FeedState::Active))
        .count()
}

/// One task limiter of `Quest_Task_Count_Text` (§4.6 Tessera rule).
///
/// The caller supplies the three values: the category key of the layout's
/// `UI_QuestJournal_QuestCategoryLimiter_Array` entry, and the limiter's
/// `QuestCategoryLimiterControl_PropertyName` and `…_DefaultLimit` for that category
/// (`quests.md` §9.8). This crate does not read them yet: the limiter's values sit in the
/// root object of the class library rather than in a property set, and the spec does not
/// name the fields of the layout array's entries (OPEN in `quest-journal.md` §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskLimiter {
    /// The limited `Quest_Category` key.
    pub category: u32,
    /// The player property that holds the player's limit for the category.
    pub property: Option<u32>,
    /// The limit used while the player's property was never reported.
    pub default_limit: Option<i64>,
}

impl TaskLimiter {
    /// The task count: non-deed `Active` quests of the limiter's category.
    #[must_use]
    pub fn count(&self, entries: &[(&LogEntry, Option<FeedState>)]) -> usize {
        entries
            .iter()
            .filter(|(e, s)| {
                !e.deed && *s == Some(FeedState::Active) && e.category == self.category
            })
            .count()
    }

    /// The task limit: the player's reported value of the limiter's property, else the
    /// default limit.
    #[must_use]
    pub fn limit(&self, player: &PlayerContext) -> Option<i64> {
        self.property
            .and_then(|p| player.int_property(p, None))
            .or(self.default_limit)
    }
}

/// The track button (§4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackButton {
    /// `UI_Quest` token of the label.
    pub label: u32,
    /// `UI_Quest` token of the tooltip.
    pub tooltip: u32,
    /// Ghosted.
    pub ghosted: bool,
    /// The `tracked` value a click sends in `TrackQuest`.
    pub sends: bool,
}

impl TrackButton {
    /// The button for the selected quest (entry, feed state, tracked flag) with
    /// `tracked_count` quests tracked.
    #[must_use]
    pub fn of(
        selected: Option<(&LogEntry, Option<FeedState>, bool)>,
        tracked_count: usize,
    ) -> Self {
        let tracked = selected.is_some_and(|s| s.2);
        let (label, tooltip) = if tracked {
            tokens::UNTRACK
        } else {
            tokens::TRACK
        };
        let ghosted = match selected {
            None => true,
            Some((e, state, tracked)) => {
                state == Some(FeedState::Completed)
                    || e.instance
                    || (!tracked && tracked_count >= TRACKER_CAPACITY)
            }
        };
        Self {
            label,
            tooltip,
            ghosted,
            sends: !tracked,
        }
    }
}

/// The abandon button (§4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbandonButton {
    /// Ghosted.
    pub ghosted: bool,
}

impl AbandonButton {
    /// The button for the selected quest.
    #[must_use]
    pub fn of(selected: Option<(&LogEntry, Option<FeedState>)>) -> Self {
        Self {
            ghosted: match selected {
                None => true,
                Some((e, state)) => state == Some(FeedState::Completed) || !e.can_be_cancelled,
            },
        }
    }
}

/// The line-2 height limit in pixels with `entries` tracked entries (§6.3 step 4):
/// ⌊total ÷ *n*⌋ × size per objective (one entry when *n* is 0).
#[must_use]
pub fn tracker_line_budget(entries: usize, metrics: TrackerMetrics) -> u32 {
    let n = u32::try_from(entries.max(1)).unwrap_or(u32::MAX);
    (metrics.total_max_objectives / n).saturating_mul(metrics.size_per_objective)
}

/// The image of a quest marker (§8.3 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerImage {
    /// The marker's own `icon`.
    Given(Did),
    /// The `Quest` note type's normal image of the view's icon table (highlight image on
    /// mouse-over).
    QuestNote,
}

/// The note-type label of quest markers in the radar and map icon tables (§8.3).
pub const QUEST_NOTE_TYPE: &str = "Quest";

/// The image of a `MarkerAdded` with `quest` present.
#[must_use]
pub fn marker_image(icon: Option<Did>) -> MarkerImage {
    match icon {
        Some(d) if !d.is_null() => MarkerImage::Given(d),
        _ => MarkerImage::QuestNote,
    }
}
