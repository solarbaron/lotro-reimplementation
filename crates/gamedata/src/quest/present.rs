// SPDX-License-Identifier: GPL-3.0-or-later
//! [`QuestPresenter`]: the per-quest presentation of `quest-journal.md` §4–§8 from a quest
//! record, the feed's reports and the player's values.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::quests::{
    ConditionElement, EventEntry, Objective, QuestArc, QuestArcDirectory, QuestLabels, QuestRecord,
    RoleAction,
};
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo, StringSource, Utf16String};
use tessera_text::{RichText, Value, Values};

use super::data::{PresentationData, element_property, info_value, layout_element, render_token};
use super::vars::{PlayerContext, PlayerVars};
use super::{FeedState, MAX_PROGRESS_LINES, ObjectiveReport, RecordRef, ids, tokens};
use crate::tooltip::{MAX_CACHED, TextRef, TextResult};

tessera_formats::property_ids! {
    /// Properties this module reads besides the quest decoder's labels.
    pub(crate) struct JournalPids {
        show_raid_in_journal = "Quest_ShowRaidInJournal",
        contains_instance_quest = "Quest_ContainsInstanceQuest",
        exp_tier = "Quest_ExpTier",
        advancement_level = "Advancement_Level",
        max_active_quests = "Quest_MaxActiveQuests",
        name = "Name",
        title_format = "UI_QuestTree_TitleLabelElementFormatString",
        total_max_objectives = "UI_QuestTracker_TotalMaxObjectives",
        size_per_objective = "UI_QuestTracker_SizePerObjective",
        rank_to_ui_state = "ConsiderationControl_RankToUIState",
        consider_rank = "ConsiderationControl_ConsiderRank",
        ui_state_id = "ConsiderationControl_UIStateID",
        quest_table_list = "ConsiderationControl_QuestTableList",
        rank_per_differential = "ConsiderationTable_RankPerDifferentialArray",
    }
}

/// `UILAYOUT` label of the quest log (§4.1).
const QUEST_LAYOUT: &str = "quest";
/// The quest tree element (§4.4).
const QUEST_TREE: &str = "QuestTree";
/// `UILAYOUT` label of the tracker templates (§6.1).
const TRACKER_LAYOUT: &str = "questtracker";
/// The tracker's quest template (§6.1).
const TRACKER_TEMPLATE: &str = "QuestTracker_QuestTemplate";
/// WEENIECONTENT label of the arc directory (`quests.md` §2.3).
const ARC_DIRECTORY: &str = "QuestArcDirectory";
/// WEENIECONTENT label of the consideration control (`game-controls.md` §7.5).
const CONSIDERATION_CONTROL: &str = "ConsiderationControl";
/// The difficulty state with no known player level (§4.4 Tessera rule).
pub const EVEN_STATE: &str = "ConEven_ColorName";
/// Most arcs and quests per arc read from the arc directory (a Tessera bound).
const MAX_ARCS: usize = 65_536;
/// Most quests listed per arc (a Tessera bound).
const MAX_ARC_QUESTS: usize = 1_024;
/// Most consideration table elements read (a Tessera bound).
const MAX_TABLE_ELEMENTS: usize = 1_024;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where a quest's level came from (§4.4; census flags of §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelSource {
    /// `Quest_HideChallengeLevel`: level 0.
    Hidden,
    /// An override property; `known` when the player's value was reported.
    Override {
        /// Whether the player's value was known.
        known: bool,
    },
    /// The stored `Quest_ChallengeLevel` (≥ 1).
    Stored,
    /// Neither: level 0.
    None,
}

/// The state of `QuestDisplay_QuestHeaderIcon` (§4.7; monster-play states never used).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderIcon {
    /// `quest_raid`.
    Raid,
    /// `quest_fellowship`.
    Fellowship,
    /// `quest_smallfellowship`.
    SmallFellowship,
    /// `quest_solo`.
    Solo,
}

impl HeaderIcon {
    /// The state label.
    #[must_use]
    pub fn state_label(self) -> &'static str {
        match self {
            Self::Raid => "quest_raid",
            Self::Fellowship => "quest_fellowship",
            Self::SmallFellowship => "quest_smallfellowship",
            Self::Solo => "quest_solo",
        }
    }
}

/// One quest's row data for the log, the details header and the tracker (§4.3, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// The quest.
    pub quest: RecordRef,
    /// `Quest_IsAccomplishment` true (never listed in the log, §4.2).
    pub deed: bool,
    /// `Quest_Category` (absent: key 0, §4.3).
    pub category: u32,
    /// The category row text: the display text, or the fallback token's text.
    pub category_text: String,
    /// The category's display text alone (the details header shows no fallback, §4.7).
    pub category_display: Option<String>,
    /// The effective level *L*.
    pub level: i32,
    /// Where *L* came from.
    pub level_source: LevelSource,
    /// The name (`Quest_Name`; the literal for a literal ref). `None` when it does not
    /// render.
    pub name: Option<RichText>,
    /// The title of §4.4 (format with level, or the name alone). `None` when it does not
    /// render.
    pub title: Option<RichText>,
    /// The lower-cased title, the name filter's key (§4.5).
    pub filter_key: String,
    /// The lower-cased name, the second sort key (§4.3).
    pub sort_name: String,
    /// The scope bits of §4.4.
    pub scope_bits: u32,
    /// The details header's icon state (§4.7).
    pub header_icon: HeaderIcon,
    /// `Quest_MaxTimesCompletable` is stored (repeat note, §4.7).
    pub repeatable_stored: bool,
    /// `Quest_CanBeCancelled` (default true).
    pub can_be_cancelled: bool,
    /// `Quest_IsInstanceQuest`.
    pub instance: bool,
    /// `Quest_IsSessionQuest`.
    pub session: bool,
    /// `Quest_IsHidden` of a deed (no new-deed line, §7.1).
    pub hidden_deed: bool,
    /// Objective indices of the record, ascending.
    pub objectives: Vec<i32>,
}

/// One line of a progress text (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressLine {
    /// A text line (it may hold line breaks of its own; each starts a bulleted line).
    Text(RichText),
    /// The alternative marker between two completion conditions (§5.2 step 2.3): the
    /// `Quest` token 0x0FBBD982 rendered with empty arguments.
    Alternative(RichText),
}

/// What a progress text was built from (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgressSource {
    /// The completed word.
    Completed,
    /// The failed word.
    Failed,
    /// `Quest_ObjectiveProgressOverride`.
    ObjectiveOverride,
    /// Event lines of the completion conditions.
    Events,
    /// No line: the objective's description (Tessera rule).
    Description,
    /// No line and no description: the objective-number line (Tessera rule).
    ObjectiveNumber,
    /// Nothing to show (a literal quest, or no objective).
    Nothing,
}

/// The progress text of a quest or one objective (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressText {
    /// The objective it describes, if any.
    pub objective: Option<i32>,
    /// The lines, in order.
    pub lines: Vec<ProgressLine>,
    /// What the lines came from.
    pub source: ProgressSource,
    /// Texts that failed to be found (`string-table.md` §9.3) and were left out.
    pub failed: usize,
    /// Texts that did not render (a value missing, or empty) and were left out.
    pub unrendered: usize,
}

impl ProgressText {
    fn new(objective: Option<i32>) -> Self {
        Self {
            objective,
            lines: Vec::new(),
            source: ProgressSource::Nothing,
            failed: 0,
            unrendered: 0,
        }
    }

    /// The plain text, one line per [`ProgressLine`].
    #[must_use]
    pub fn plain(&self) -> String {
        self.lines
            .iter()
            .map(|l| match l {
                ProgressLine::Text(t) | ProgressLine::Alternative(t) => t.plain_text(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn push(&mut self, r: TextResult) -> bool {
        match r {
            Ok(Some(t)) => {
                if self.lines.len() < MAX_PROGRESS_LINES {
                    self.lines.push(ProgressLine::Text(t));
                }
                true
            }
            Ok(None) => {
                self.unrendered += 1;
                false
            }
            Err(e) => {
                tracing::debug!(error = %e, "progress text failed");
                self.failed += 1;
                false
            }
        }
    }
}

/// One tracker entry (§6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerEntry {
    /// The quest.
    pub quest: RecordRef,
    /// Line 1: the name, no level.
    pub name: Option<RichText>,
    /// Line 2: the progress text (deed counter applied for deeds).
    pub progress: ProgressText,
    /// The ready icon (§6.4 Tessera rule).
    pub ready: bool,
    /// The quest is a deed (the menu's show-in-log item does nothing, §6.6).
    pub deed: bool,
}

/// The details header (§4.7 item 1).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestHeader {
    /// The title (§4.4).
    pub title: Option<RichText>,
    /// The arc name; `None` hides the element (the category moves up).
    pub arc_name: Option<RichText>,
    /// The category's display text (no fallback).
    pub category: Option<String>,
    /// The icon state.
    pub icon: HeaderIcon,
}

/// Which order the details pane uses (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailsMode {
    /// Underway quest (display type 4).
    Underway,
    /// Completed quest in the log (display type 4, status completed).
    Completed,
    /// The history window (display type 2): objectives up to the current one.
    History,
}

/// One section of the details pane, in display order (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub enum DetailsSection {
    /// The header.
    Header(QuestHeader),
    /// The objective block: the current objective's progress text.
    Progress(ProgressText),
    /// The current objective's `Quest_ObjectiveDescription`.
    ObjectiveText(RichText),
    /// The bestowal label and dialogue (completed and history modes).
    Bestowal {
        /// `UI_Quest` 0x08B189FC.
        label: Option<RichText>,
        /// The offer line of `npc-windows.md` §6.4 rules 2–3.
        text: RichText,
    },
    /// The background label and `Quest_Description`.
    Background {
        /// `UI_Quest` 0x099EECDC.
        label: Option<RichText>,
        /// The description.
        text: RichText,
    },
    /// One past objective (completed and history modes).
    PastObjective {
        /// The objective index.
        index: i32,
        /// `UI_Quest` 0x0B86343C with the index.
        label: Option<RichText>,
        /// Its description, when it has one.
        text: Option<RichText>,
    },
    /// The repeat note.
    RepeatNote(RichText),
    /// The reward rows of the quest's treasure record (`npc-windows.md` §6.3 steps 2–4):
    /// [`RewardRow::Header`](crate::npc::RewardRow::Header) first, then the rows; left out
    /// when the record gives no row.
    Rewards(Vec<crate::npc::RewardRow>),
}

/// A quest chat line (§7).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestMessage {
    /// Chat type key (21).
    pub chat_type: u32,
    /// The 64-bit chat type mask.
    pub mask: u64,
    /// The text.
    pub text: RichText,
}

impl QuestMessage {
    fn new(text: RichText) -> Self {
        Self {
            chat_type: tokens::CHAT_TYPE,
            mask: tokens::CHAT_MASK,
            text,
        }
    }
}

/// The tracker template's height values (§6.1, QI5), read from the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackerMetrics {
    /// `UI_QuestTracker_TotalMaxObjectives`.
    pub total_max_objectives: u32,
    /// `UI_QuestTracker_SizePerObjective`.
    pub size_per_objective: u32,
}

/// The consideration data for difficulty states (§4.4, `game-controls.md` §7.5).
#[derive(Debug, Clone, Default)]
struct Difficulty {
    /// The quest-table list: per element, the table's rank array.
    list: Vec<Arc<Vec<u32>>>,
    /// Rank key → UI state label.
    states: HashMap<u32, String>,
}

/// The current objective of a quest from its record's objective indices and the feed's
/// reports (§5.1 Tessera rule). Reports naming an index the record does not have are
/// ignored (at debug level here, since this query runs on every redraw; the warning of §5.1
/// is logged once per report by [`QuestPresenter::posts_objective_line`]).
#[must_use]
pub fn current_objective(indices: &[i32], reports: &[ObjectiveReport]) -> Option<i32> {
    let valid: Vec<&ObjectiveReport> = reports
        .iter()
        .filter(|r| {
            let ok = indices.contains(&i32::from(r.objective));
            if !ok {
                tracing::debug!(objective = r.objective, "report names an unknown objective");
            }
            ok
        })
        .collect();
    if let Some(o) = valid
        .iter()
        .filter(|r| !r.done)
        .map(|r| i32::from(r.objective))
        .max()
    {
        return Some(o);
    }
    if let Some(highest) = valid.iter().map(|r| i32::from(r.objective)).max() {
        return indices
            .iter()
            .copied()
            .filter(|&i| i > highest)
            .min()
            .or(Some(highest));
    }
    indices.iter().copied().min()
}

/// The quest presentation over a [`PresentationData`].
pub struct QuestPresenter<'d> {
    data: &'d dyn PresentationData,
    labels: QuestLabels,
    pids: JournalPids,
    quest_table: Option<Did>,
    ui_quest_table: Option<Did>,
    title_format: Option<StringInfo>,
    show_progress_default: bool,
    show_billboard_default: bool,
    can_cancel_default: bool,
    arcs: OnceLock<HashMap<Did, StringInfo>>,
    categories: Mutex<HashMap<u32, Option<String>>>,
    fallback: OnceLock<Option<String>>,
    difficulty: OnceLock<Difficulty>,
    metrics: OnceLock<Option<TrackerMetrics>>,
    npc_pids: OnceLock<crate::npc::NpcPids>,
}

impl std::fmt::Debug for QuestPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuestPresenter")
            .field("quest_table", &self.quest_table)
            .field("ui_quest_table", &self.ui_quest_table)
            .finish_non_exhaustive()
    }
}

fn bool_default(data: &dyn PresentationData, pid: Option<u32>, fallback: bool) -> bool {
    match pid.and_then(|p| data.default_value(p)) {
        Some(PropertyValue::Bool(b)) => b,
        _ => fallback,
    }
}

impl<'d> QuestPresenter<'d> {
    /// Resolves labels, tables and the title format once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        let labels = QuestLabels::resolve(&names);
        let pids = JournalPids::resolve(&names);
        let title_format = layout_element(data, QUEST_LAYOUT, QUEST_TREE)
            .ok()
            .and_then(|(layout, id)| {
                let e = super::data::find_element(&layout, id)?;
                match element_property(e, pids.title_format.id()?)? {
                    PropertyValue::StringInfo(i) => Some((**i).clone()),
                    _ => None,
                }
            });
        if title_format.is_none() {
            tracing::debug!("no title format on the quest tree; using the Quest table token");
        }
        Self {
            show_progress_default: bool_default(data, labels.event.show_progress_text.id(), true),
            show_billboard_default: bool_default(data, labels.event.show_billboard_text.id(), true),
            can_cancel_default: bool_default(data, labels.quest.can_be_cancelled.id(), true),
            quest_table: data.table(tokens::QUEST_TABLE),
            ui_quest_table: data.table(tokens::UI_QUEST_TABLE),
            data,
            labels,
            pids,
            title_format,
            arcs: OnceLock::new(),
            categories: Mutex::new(HashMap::new()),
            fallback: OnceLock::new(),
            difficulty: OnceLock::new(),
            metrics: OnceLock::new(),
            npc_pids: OnceLock::new(),
        }
    }

    /// The properties the NPC-window models read, resolved once.
    pub(crate) fn npc_pids(&self) -> &crate::npc::NpcPids {
        self.npc_pids
            .get_or_init(|| crate::npc::NpcPids::resolve(&|l: &str| self.data.pid(l)))
    }

    /// The data source.
    #[must_use]
    pub fn data(&self) -> &'d dyn PresentationData {
        self.data
    }

    /// The quest decoder's labels.
    #[must_use]
    pub fn labels(&self) -> &QuestLabels {
        &self.labels
    }

    /// The player variables for `context` (`npc-windows.md` §5.5).
    #[must_use]
    pub fn player(&self, context: PlayerContext) -> PlayerVars {
        PlayerVars::new(self.data, context)
    }

    /// A quest record's property set; `Err` names why it cannot be read.
    pub fn record(&self, did: Did) -> Result<Arc<PropertySet>, String> {
        let set = self
            .data
            .record(did)?
            .ok_or_else(|| format!("record {did} absent"))?;
        QuestRecord::new(&self.labels, &set).map_err(|e| e.to_string())?;
        Ok(set)
    }

    fn quest<'s>(&'s self, set: &'s PropertySet) -> QuestRecord<'s> {
        QuestRecord::new_unchecked(&self.labels, set)
    }

    /// Renders a `Quest` table token.
    pub fn quest_token(&self, token: u32, values: &Values) -> TextResult {
        render_token(self.data, self.quest_table, token, values)
    }

    /// Renders a `UI_Quest` table token.
    pub fn ui_token(&self, token: u32, values: &Values) -> TextResult {
        render_token(self.data, self.ui_quest_table, token, values)
    }

    fn info(&self, info: &StringInfo, values: &Values) -> TextResult {
        self.data.render(TextRef::Info(info), values)
    }

    // -----------------------------------------------------------------------------------------
    // §4.3–§4.4 Log entries
    // -----------------------------------------------------------------------------------------

    /// The category row text of `key` (§4.3): its display text, or the fallback token's
    /// text. The flag is true for the fallback.
    #[must_use]
    pub fn category_text(&self, key: u32) -> (String, bool) {
        match self.category_display(key) {
            Some(t) => (t, false),
            None => (
                self.fallback
                    .get_or_init(|| {
                        self.quest_token(tokens::CATEGORY_FALLBACK, &Values::new())
                            .ok()
                            .flatten()
                            .map(|t| t.plain_text())
                    })
                    .clone()
                    .unwrap_or_default(),
                true,
            ),
        }
    }

    /// The display text of category `key`, when it has one that resolves.
    #[must_use]
    pub fn category_display(&self, key: u32) -> Option<String> {
        if let Some(t) = lock(&self.categories).get(&key) {
            return t.clone();
        }
        let text = self
            .labels
            .quest
            .category
            .id()
            .and_then(|p| self.data.enum_text(p, key));
        let mut cache = lock(&self.categories);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(key, text.clone());
        text
    }

    /// The scope bits of a quest record (§4.4; bit 0x200 never set in P3).
    #[must_use]
    pub fn scope_bits(&self, set: &PropertySet) -> u32 {
        let q = &self.labels.quest;
        let scope = self.quest(set).scope();
        let raid = set.boolean(self.pids.show_raid_in_journal) == Some(true);
        let fellowship = set.boolean(q.is_fellowship_recommended) == Some(true);
        let small = !fellowship && set.boolean(q.is_small_fellowship_recommended) == Some(true);
        let mut bits = 0u32;
        if scope == Some(3) {
            bits |= 0x40;
        }
        if raid {
            bits |= 0x80;
        }
        if fellowship {
            bits |= 0x02;
        }
        if small {
            bits |= 0x04;
        }
        if !fellowship && !small && !raid {
            bits |= 0x20;
        }
        if scope == Some(1) {
            bits |= 0x08;
        }
        if set.boolean(q.is_instance_quest) == Some(true)
            || set.boolean(self.pids.contains_instance_quest) == Some(true)
        {
            bits |= 0x100;
        }
        if let Some(max) = set.int(q.max_times_completable)
            && max != 1
            && scope != Some(4)
        {
            bits |= 0x01;
        }
        if bits == 0 {
            bits = 0x10;
        }
        bits
    }

    fn header_icon(&self, set: &PropertySet) -> HeaderIcon {
        let q = &self.labels.quest;
        if set.boolean(self.pids.show_raid_in_journal) == Some(true) {
            HeaderIcon::Raid
        } else if set.boolean(q.is_fellowship_recommended) == Some(true) {
            HeaderIcon::Fellowship
        } else if set.boolean(q.is_small_fellowship_recommended) == Some(true) {
            HeaderIcon::SmallFellowship
        } else {
            HeaderIcon::Solo
        }
    }

    /// The effective level and its source (§4.4 Tessera rule over `quests.md` §4.3).
    #[must_use]
    pub fn level(&self, set: &PropertySet, player: &PlayerContext) -> (i32, LevelSource) {
        let q = self.quest(set);
        if q.hides_challenge_level() {
            return (0, LevelSource::Hidden);
        }
        let adv = self.pids.advancement_level.id();
        if let Some(pid) = q.challenge_level_override().filter(|&p| p != 0) {
            if let Some(v) = player.int_property(pid, adv) {
                let v = i32::try_from(v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
                    .unwrap_or_default();
                return (v, LevelSource::Override { known: true });
            }
            let stored = q.challenge_level().unwrap_or(0);
            return (stored, LevelSource::Override { known: false });
        }
        match q.challenge_level() {
            Some(l) if l >= 1 => (l, LevelSource::Stored),
            Some(l) => (l, LevelSource::None),
            None => (0, LevelSource::None),
        }
    }

    /// A feed literal as an argument value: rendered like a literal StringInfo.
    fn literal_value(&self, s: &str) -> Value {
        let info = StringInfo {
            source: StringSource::Literal(Utf16String(s.encode_utf16().collect())),
            args: None,
        };
        info_value(self.data, &info, &Values::new()).unwrap_or_else(|_| Value::Text(s.to_owned()))
    }

    /// The name value for title and message arguments.
    fn name_value(&self, set: &PropertySet, player: &PlayerVars) -> Option<Value> {
        let info = self.quest(set).name()?;
        info_value(self.data, info, &player.values).ok()
    }

    /// The rendered `Quest_Name`.
    pub fn name(&self, set: &PropertySet, player: &PlayerVars) -> TextResult {
        match self.quest(set).name() {
            Some(info) => self.info(info, &player.values),
            None => Ok(None),
        }
    }

    /// The title of §4.4 for level `level`.
    pub fn title(&self, set: &PropertySet, level: i32, player: &PlayerVars) -> TextResult {
        let Some(name) = self.name_value(set, player) else {
            return Ok(None);
        };
        let mut v = Values::new();
        v.insert(ids::TITLE_NAME, name);
        if level >= 1 {
            v.insert(ids::TITLE_LEVEL, Value::Integer(i64::from(level)));
            let v = player.with(&v);
            match &self.title_format {
                Some(f) => self.info(f, &v),
                None => self.quest_token(tokens::TITLE_WITH_LEVEL, &v),
            }
        } else {
            self.quest_token(tokens::TITLE_WITHOUT_LEVEL, &player.with(&v))
        }
    }

    /// The log entry of a quest (§4.3, §4.4). `Err` when the record cannot be read.
    pub fn log_entry(&self, quest: &RecordRef, player: &PlayerVars) -> Result<LogEntry, String> {
        let did = match quest {
            RecordRef::Literal(s) => return Ok(self.literal_entry(s)),
            RecordRef::Record(d) => *d,
        };
        let set = self.record(did)?;
        Ok(self.entry_of(quest.clone(), &set, player))
    }

    fn literal_entry(&self, s: &str) -> LogEntry {
        let (category_text, _) = self.category_text(0);
        let text = RichText::plain(s);
        let lower = lower(s);
        LogEntry {
            quest: RecordRef::Literal(s.to_owned()),
            deed: false,
            category: 0,
            category_text,
            category_display: None,
            level: 0,
            level_source: LevelSource::None,
            name: Some(text.clone()),
            title: Some(text),
            filter_key: lower.clone(),
            sort_name: lower,
            scope_bits: 0x20,
            header_icon: HeaderIcon::Solo,
            repeatable_stored: false,
            can_be_cancelled: true,
            instance: false,
            session: false,
            hidden_deed: false,
            objectives: Vec::new(),
        }
    }

    /// The log entry of a decoded quest record.
    #[must_use]
    pub fn entry_of(&self, quest: RecordRef, set: &PropertySet, player: &PlayerVars) -> LogEntry {
        let q = self.quest(set);
        let qp = &self.labels.quest;
        let category = q.category().unwrap_or(0);
        let (category_text, _) = self.category_text(category);
        let (level, level_source) = self.level(set, &player.context);
        let name = self.name(set, player).ok().flatten();
        let title = self.title(set, level, player).ok().flatten();
        let mut objectives: Vec<i32> = q.objectives().iter().filter_map(Objective::index).collect();
        objectives.sort_unstable();
        objectives.dedup();
        LogEntry {
            quest,
            deed: q.is_deed(),
            category,
            category_display: self.category_display(category),
            category_text,
            level,
            level_source,
            filter_key: title
                .as_ref()
                .map(|t| lower(&t.plain_text()))
                .unwrap_or_default(),
            sort_name: name
                .as_ref()
                .map(|t| lower(&t.plain_text()))
                .unwrap_or_default(),
            name,
            title,
            scope_bits: self.scope_bits(set),
            header_icon: self.header_icon(set),
            repeatable_stored: set.has(qp.max_times_completable),
            can_be_cancelled: set
                .boolean(qp.can_be_cancelled)
                .unwrap_or(self.can_cancel_default),
            instance: set.boolean(qp.is_instance_quest) == Some(true),
            session: set.boolean(qp.is_session_quest) == Some(true),
            hidden_deed: q.deed().and_then(|d| d.is_hidden) == Some(true),
            objectives,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §4.4 Difficulty state
    // -----------------------------------------------------------------------------------------

    fn difficulty(&self) -> &Difficulty {
        self.difficulty.get_or_init(|| {
            let read = || -> Option<Difficulty> {
                let did = self.data.weenie(CONSIDERATION_CONTROL)?;
                let set = self.data.record(did).ok()??;
                let mut tables: HashMap<Did, Arc<Vec<u32>>> = HashMap::new();
                let mut list = Vec::new();
                for t in set
                    .dids(self.pids.quest_table_list)
                    .take(MAX_TABLE_ELEMENTS)
                {
                    let table = tables
                        .entry(t)
                        .or_insert_with(|| {
                            let ranks = self
                                .data
                                .record(t)
                                .ok()
                                .flatten()
                                .map(|s| {
                                    s.elements(self.pids.rank_per_differential)
                                        .iter()
                                        .take(MAX_TABLE_ELEMENTS)
                                        .filter_map(|e| match e.value {
                                            PropertyValue::Enum(k) => Some(k),
                                            _ => None,
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Arc::new(ranks)
                        })
                        .clone();
                    list.push(table);
                }
                let mut states = HashMap::new();
                for s in set
                    .structs(self.pids.rank_to_ui_state)
                    .take(MAX_TABLE_ELEMENTS)
                {
                    let (Some(rank), Some(state)) = (
                        s.enum_key(self.pids.consider_rank),
                        s.get_pid(self.pids.ui_state_id).and_then(|v| match v {
                            PropertyValue::Enum(k) => Some(*k),
                            PropertyValue::Int(i) => u32::try_from(*i).ok(),
                            _ => None,
                        }),
                    ) else {
                        continue;
                    };
                    if let Some(label) = self.data.ui_label(UiEnum::StateId, state) {
                        states.insert(rank, label);
                    }
                }
                Some(Difficulty { list, states })
            };
            read().unwrap_or_default()
        })
    }

    /// The difficulty state label of a quest of level `level` for the player's level (§4.4
    /// Tessera rule): list element by the player's level, table element *d* + 15 for
    /// *d* = *L* − player level (0 when *L* < 1), clamped to the table;
    /// [`EVEN_STATE`] without a player level or data.
    #[must_use]
    pub fn difficulty_state(&self, level: i32, player_level: Option<i32>) -> String {
        let Some(p) = player_level else {
            return EVEN_STATE.to_owned();
        };
        let d = self.difficulty();
        if d.list.is_empty() {
            return EVEN_STATE.to_owned();
        }
        let li = usize::try_from(p.saturating_sub(1).max(0))
            .unwrap_or(0)
            .min(d.list.len() - 1);
        let table = &d.list[li];
        if table.is_empty() {
            return EVEN_STATE.to_owned();
        }
        let diff = if level < 1 {
            0
        } else {
            i64::from(level) - i64::from(p)
        };
        let idx = usize::try_from((diff + 15).clamp(0, 30))
            .unwrap_or(15)
            .min(table.len() - 1);
        d.states
            .get(&table[idx])
            .cloned()
            .unwrap_or_else(|| EVEN_STATE.to_owned())
    }

    // -----------------------------------------------------------------------------------------
    // §5 Progress texts
    // -----------------------------------------------------------------------------------------

    fn event_number(e: &EventEntry<'_>) -> i64 {
        i64::from(e.number().unwrap_or(1).max(0))
    }

    fn event_counters(&self, report: Option<&ObjectiveReport>, number: i64) -> Values {
        let count = report.map_or(0, |r| i64::from(r.count));
        let total = report.map_or(number, |r| i64::from(r.total));
        let mut v = Values::new();
        v.insert(ids::EVENT_COUNT, Value::Integer(count));
        v.insert(ids::EVENT_TOTAL, Value::Integer(total));
        v.insert(ids::EVENT_LEFT, Value::Integer((total - count).max(0)));
        v
    }

    fn objective_counters(&self, o: &Objective<'_>, report: Option<&ObjectiveReport>) -> Values {
        let count = report.map_or(0, |r| i64::from(r.count));
        let total = report.map_or_else(
            || {
                o.completion
                    .iter()
                    .flat_map(|c| c.entries())
                    .map(Self::event_number)
                    .sum()
            },
            |r| i64::from(r.total),
        );
        let mut v = Values::new();
        v.insert(ids::OBJECTIVE_COUNT, Value::Integer(count));
        v.insert(ids::OBJECTIVE_TOTAL, Value::Integer(total));
        v
    }

    /// The NPC name of a type-11 event as a value.
    fn npc_name(&self, e: &EventEntry<'_>) -> Option<Value> {
        let did = e.npc_talk()?;
        let set = self.data.record(did).ok()??;
        let info = set.string_info(self.pids.name)?;
        info_value(self.data, info, &Values::new()).ok()
    }

    /// The event counters of `e` plus the values its subclass adds (§5.3 step 2.4, §5.4):
    /// the NPC name of a type-11 event and the world value of a type-34 event. Event lines
    /// and billboard overrides use the same set (§5.4).
    fn event_values(&self, e: &EventEntry<'_>, report: Option<&ObjectiveReport>) -> Values {
        let mut v = self.event_counters(report, Self::event_number(e));
        match e.event_type() {
            Some(11) => {
                if let Some(n) = self.npc_name(e) {
                    v.insert(ids::NPC_NAME, n);
                }
            }
            Some(34) => {
                v.insert(
                    ids::WORLD_VALUE,
                    Value::Integer(report.map_or(0, |r| i64::from(r.count))),
                );
            }
            _ => {}
        }
        v
    }

    /// The line of one event (§5.3 with the Tessera rules): `None` when it adds no line.
    fn event_line(
        &self,
        e: &EventEntry<'_>,
        report: Option<&ObjectiveReport>,
        deed_counter: bool,
        player: &PlayerVars,
    ) -> Option<TextResult> {
        let show = e
            .raw()
            .boolean(self.labels.event.show_progress_text)
            .unwrap_or(self.show_progress_default);
        if !show {
            return None;
        }
        let info = e.progress_override()?;
        let number = Self::event_number(e);
        let v = player.with(&self.event_values(e, report));
        if deed_counter && number > 1 {
            let args = self.data.text_args(TextRef::Info(info)).unwrap_or_default();
            let both = args.contains(&ids::EVENT_COUNT) && args.contains(&ids::EVENT_TOTAL);
            if !both {
                return Some(self.deed_suffix(info, &v));
            }
        }
        Some(self.info(info, &v))
    }

    fn deed_suffix(&self, info: &StringInfo, v: &Values) -> TextResult {
        let mut w = Values::new();
        w.insert(ids::JOIN_FIRST, info_value(self.data, info, v)?);
        for id in [ids::EVENT_COUNT, ids::EVENT_TOTAL] {
            if let Some(x) = v.get(id) {
                w.insert(id, x.clone());
            }
        }
        self.quest_token(tokens::DEED_COUNTER, &w)
    }

    fn alternative(&self) -> Option<RichText> {
        let mut v = Values::new();
        v.insert(ids::JOIN_FIRST, "");
        v.insert(ids::JOIN_SECOND, "");
        let t = self
            .quest_token(tokens::ALTERNATIVE_JOIN, &v)
            .ok()
            .flatten()?;
        let plain = t.plain_text();
        let trimmed = plain.trim();
        (!trimmed.is_empty()).then(|| RichText::plain(trimmed))
    }

    /// The progress text of objective `index` of a quest (§5.2 step 2, Tessera rules),
    /// regardless of the quest's state. `deed_counter` asks for §5.3 step 5 (applied only
    /// to deeds).
    #[must_use]
    pub fn objective_progress(
        &self,
        set: &PropertySet,
        index: i32,
        reports: &[ObjectiveReport],
        deed_counter: bool,
        player: &PlayerVars,
    ) -> ProgressText {
        let q = self.quest(set);
        let objectives = q.objectives();
        match objectives.iter().find(|o| o.index() == Some(index)) {
            Some(o) => self.progress_of(&q, o, reports, deed_counter, player),
            None => ProgressText::new(Some(index)),
        }
    }

    /// [`Self::objective_progress`] for an objective already decoded.
    pub(crate) fn progress_of(
        &self,
        q: &QuestRecord<'_>,
        o: &Objective<'_>,
        reports: &[ObjectiveReport],
        deed_counter: bool,
        player: &PlayerVars,
    ) -> ProgressText {
        let index = o.index().unwrap_or_default();
        let mut out = ProgressText::new(Some(index));
        let report = reports.iter().find(|r| i32::from(r.objective) == index);
        let deed_counter = deed_counter && q.is_deed();
        if let Some(info) = o.progress_override() {
            let v = player.with(&self.objective_counters(o, report));
            if out.push(self.info(info, &v)) {
                out.source = ProgressSource::ObjectiveOverride;
                return out;
            }
        } else {
            let mut groups = 0usize;
            let mut marker: Option<Option<RichText>> = None;
            for c in &o.completion {
                if out.lines.len() >= MAX_PROGRESS_LINES {
                    // The line budget is spent: later conditions are not rendered.
                    break;
                }
                let mut lines = Vec::new();
                for el in &c.elements {
                    let results: Vec<TextResult> = match el {
                        ConditionElement::Entry(e) => self
                            .event_line(e, report, deed_counter, player)
                            .into_iter()
                            .collect(),
                        ConditionElement::Compound(cmp) => match cmp.progress_override {
                            Some(info) => {
                                let number: i64 = cmp.members().map(Self::event_number).sum();
                                let v = player.with(&self.event_counters(report, number.max(1)));
                                vec![self.info(info, &v)]
                            }
                            None => cmp
                                .members()
                                .filter_map(|e| self.event_line(e, report, deed_counter, player))
                                .collect(),
                        },
                    };
                    for r in results {
                        match r {
                            Ok(Some(t)) => lines.push(t),
                            Ok(None) => out.unrendered += 1,
                            Err(e) => {
                                tracing::debug!(error = %e, "event line failed");
                                out.failed += 1;
                            }
                        }
                    }
                }
                if lines.is_empty() {
                    continue;
                }
                if groups > 0
                    && let Some(m) = marker.get_or_insert_with(|| self.alternative())
                {
                    // A marker only when a line can follow it within the budget.
                    if out.lines.len() + 1 >= MAX_PROGRESS_LINES {
                        break;
                    }
                    out.lines.push(ProgressLine::Alternative(m.clone()));
                }
                groups += 1;
                for t in lines {
                    if out.lines.len() >= MAX_PROGRESS_LINES {
                        break;
                    }
                    out.lines.push(ProgressLine::Text(t));
                }
            }
            if groups > 0 {
                out.source = ProgressSource::Events;
                return out;
            }
        }
        // Tessera rule: no line → the description, else the objective-number line.
        if let Some(info) = o.description()
            && out.push(self.info(info, &player.values))
        {
            out.source = ProgressSource::Description;
            return out;
        }
        let mut v = Values::new();
        v.insert(ids::OBJECTIVE_NUMBER, Value::Integer(i64::from(index)));
        if out.push(self.ui_token(tokens::OBJECTIVE_LABEL, &v)) {
            out.source = ProgressSource::ObjectiveNumber;
        }
        out
    }

    /// The progress text of a quest (§5.2): the completed or failed word, or the current
    /// objective's text (§5.1).
    #[must_use]
    pub fn progress(
        &self,
        set: &PropertySet,
        state: Option<FeedState>,
        reports: &[ObjectiveReport],
        deed_counter: bool,
        player: &PlayerVars,
    ) -> ProgressText {
        if let Some(out) = self.state_word(state) {
            return out;
        }
        let indices = self.objective_indices(set);
        match current_objective(&indices, reports) {
            Some(i) => self.objective_progress(set, i, reports, deed_counter, player),
            None => ProgressText::new(None),
        }
    }

    /// The completed or failed word of §5.2 step 1 for those states (no objective needed,
    /// so literal quests use it too).
    fn state_word(&self, state: Option<FeedState>) -> Option<ProgressText> {
        let (token, source) = match state {
            Some(FeedState::Completed) => (tokens::COMPLETED_WORD, ProgressSource::Completed),
            Some(FeedState::Failed) => (tokens::FAILED_WORD, ProgressSource::Failed),
            _ => return None,
        };
        let mut out = ProgressText::new(None);
        if out.push(self.quest_token(token, &Values::new())) {
            out.source = source;
        }
        Some(out)
    }

    /// The record's objective indices, ascending.
    #[must_use]
    pub fn objective_indices(&self, set: &PropertySet) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .quest(set)
            .objectives()
            .iter()
            .filter_map(Objective::index)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The rendered `Quest_ObjectiveDescription` of objective `index`.
    pub fn objective_description(
        &self,
        set: &PropertySet,
        index: i32,
        player: &PlayerVars,
    ) -> TextResult {
        let q = self.quest(set);
        let objectives = q.objectives();
        match objectives.iter().find(|o| o.index() == Some(index)) {
            Some(o) => self.description_of(o, player),
            None => Ok(None),
        }
    }

    /// [`Self::objective_description`] for an objective already decoded.
    pub(crate) fn description_of(&self, o: &Objective<'_>, player: &PlayerVars) -> TextResult {
        match o.description() {
            Some(info) => self.info(info, &player.values),
            None => Ok(None),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §6 Tracker
    // -----------------------------------------------------------------------------------------

    /// Whether the ready icon shows (§6.4 Tessera rule).
    #[must_use]
    pub fn ready(
        &self,
        set: &PropertySet,
        state: Option<FeedState>,
        reports: &[ObjectiveReport],
    ) -> bool {
        if state != Some(FeedState::Active) || self.quest(set).is_deed() {
            return false;
        }
        let indices = self.objective_indices(set);
        indices.len() >= 2 && current_objective(&indices, reports) == indices.last().copied()
    }

    /// The tracker entry of a quest (§6.3).
    pub fn tracker_entry(
        &self,
        quest: &RecordRef,
        state: Option<FeedState>,
        reports: &[ObjectiveReport],
        player: &PlayerVars,
    ) -> Result<TrackerEntry, String> {
        let did = match quest {
            RecordRef::Literal(s) => {
                // §3, §6.4: the literal as the name; line 2 is the completed or failed word
                // for those states, else empty (no objectives).
                return Ok(TrackerEntry {
                    quest: quest.clone(),
                    name: Some(RichText::plain(s)),
                    progress: self
                        .state_word(state)
                        .unwrap_or_else(|| ProgressText::new(None)),
                    ready: false,
                    deed: false,
                });
            }
            RecordRef::Record(d) => *d,
        };
        let set = self.record(did)?;
        Ok(TrackerEntry {
            quest: quest.clone(),
            name: self.name(&set, player).ok().flatten(),
            progress: self.progress(&set, state, reports, true, player),
            ready: self.ready(&set, state, reports),
            deed: self.quest(&set).is_deed(),
        })
    }

    /// The tracker template's height values (§6.1, QI5), when the layout has them.
    #[must_use]
    pub fn tracker_metrics(&self) -> Option<TrackerMetrics> {
        *self.metrics.get_or_init(|| {
            let (layout, id) = layout_element(self.data, TRACKER_LAYOUT, TRACKER_TEMPLATE).ok()?;
            let e = super::data::find_element(&layout, id)?;
            let int = |pid: Option<u32>| -> Option<u32> {
                match element_property(e, pid?)? {
                    PropertyValue::Int(i) => u32::try_from(*i).ok(),
                    PropertyValue::Float(f) if f.is_finite() && *f >= 0.0 => {
                        // Tessera: a float value is truncated.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        Some(*f as u32)
                    }
                    _ => None,
                }
            };
            Some(TrackerMetrics {
                total_max_objectives: int(self.pids.total_max_objectives.id())?,
                size_per_objective: int(self.pids.size_per_objective.id())?,
            })
        })
    }

    // -----------------------------------------------------------------------------------------
    // §4.7 Details pane
    // -----------------------------------------------------------------------------------------

    /// The arc name of a quest (§4.7 Tessera rule).
    pub fn arc_name(&self, quest: Did, player: &PlayerVars) -> TextResult {
        let arcs = self.arcs.get_or_init(|| {
            let mut out = HashMap::new();
            let Some(dir) = self.data.weenie(ARC_DIRECTORY) else {
                return out;
            };
            let Ok(Some(set)) = self.data.record(dir) else {
                return out;
            };
            for arc in QuestArcDirectory::new(&self.labels, &set)
                .arcs()
                .into_iter()
                .take(MAX_ARCS)
            {
                let Ok(Some(a)) = self.data.record(arc) else {
                    continue;
                };
                let view = QuestArc::new(&self.labels, &a);
                let Some(name) = view.name() else {
                    continue;
                };
                for q in view.quests().into_iter().take(MAX_ARC_QUESTS) {
                    out.entry(q).or_insert_with(|| name.clone());
                }
            }
            out
        });
        match arcs.get(&quest) {
            Some(info) => self.info(info, &player.values),
            None => Ok(None),
        }
    }

    /// The details pane of a quest (§4.7), sections in display order. `completed_before`
    /// selects the repeat note (Tessera rule: the feed reported the quest completed earlier
    /// in the session).
    pub fn details(
        &self,
        quest: &RecordRef,
        mode: DetailsMode,
        reports: &[ObjectiveReport],
        completed_before: bool,
        player: &PlayerVars,
    ) -> Result<Vec<DetailsSection>, String> {
        let did = match quest {
            RecordRef::Literal(s) => {
                return Ok(vec![DetailsSection::Header(QuestHeader {
                    title: Some(RichText::plain(s)),
                    arc_name: None,
                    category: None,
                    icon: HeaderIcon::Solo,
                })]);
            }
            RecordRef::Record(d) => *d,
        };
        let set = self.record(did)?;
        let entry = self.entry_of(quest.clone(), &set, player);
        let mut out = vec![DetailsSection::Header(QuestHeader {
            title: entry.title.clone(),
            arc_name: self.arc_name(did, player).ok().flatten(),
            category: entry.category_display.clone(),
            icon: entry.header_icon,
        })];
        let background = || -> Option<DetailsSection> {
            let info = self.quest(&set).description()?;
            let text = self.info(info, &player.values).ok().flatten()?;
            Some(DetailsSection::Background {
                label: self
                    .ui_token(tokens::BACKGROUND_LABEL, &Values::new())
                    .ok()
                    .flatten(),
                text,
            })
        };
        let state = match mode {
            DetailsMode::Completed => Some(FeedState::Completed),
            _ => Some(FeedState::Active),
        };
        match mode {
            DetailsMode::Underway => {
                out.push(DetailsSection::Progress(
                    self.progress(&set, state, reports, false, player),
                ));
                if let Some(i) = current_objective(&entry.objectives, reports)
                    && let Ok(Some(t)) = self.objective_description(&set, i, player)
                {
                    out.push(DetailsSection::ObjectiveText(t));
                }
                out.extend(background());
                if entry.repeatable_stored {
                    let token = if completed_before {
                        tokens::REPEAT_DONE_BEFORE
                    } else {
                        tokens::REPEAT_NEVER_DONE
                    };
                    if let Ok(Some(t)) = self.ui_token(token, &Values::new()) {
                        out.push(DetailsSection::RepeatNote(t));
                    }
                }
            }
            DetailsMode::Completed | DetailsMode::History => {
                let (line, _) = crate::npc::offer_line_of(self, &set, None, player);
                if let Some(text) = line {
                    out.push(DetailsSection::Bestowal {
                        label: self
                            .ui_token(tokens::BESTOW_LABEL, &Values::new())
                            .ok()
                            .flatten(),
                        text,
                    });
                }
                out.extend(background());
                let last = if mode == DetailsMode::Completed {
                    entry.objectives.last().copied()
                } else {
                    current_objective(&entry.objectives, reports)
                };
                if let Some(last) = last {
                    // Decoded once: the first objective of each index, as
                    // `objective_description` finds it.
                    let q = self.quest(&set);
                    let objectives = q.objectives();
                    let mut by_index: HashMap<i32, &Objective<'_>> = HashMap::new();
                    for o in &objectives {
                        if let Some(i) = o.index() {
                            by_index.entry(i).or_insert(o);
                        }
                    }
                    for &i in entry.objectives.iter().filter(|&&i| i <= last) {
                        let mut v = Values::new();
                        v.insert(ids::OBJECTIVE_NUMBER, Value::Integer(i64::from(i)));
                        out.push(DetailsSection::PastObjective {
                            index: i,
                            label: self.ui_token(tokens::OBJECTIVE_LABEL, &v).ok().flatten(),
                            text: by_index
                                .get(&i)
                                .and_then(|o| self.description_of(o, player).ok().flatten()),
                        });
                    }
                }
            }
        }
        if mode != DetailsMode::History {
            // Filled like the offer window's reward block (§4.7 item 6, `npc-windows.md`
            // §6.3 step 4): the header on top when any row was added; no section otherwise.
            let mut rows = crate::npc::treasure_rows_of(self, &set, player);
            if !rows.is_empty() {
                rows.insert(0, crate::npc::RewardRow::Header);
                out.push(DetailsSection::Rewards(rows));
            }
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------------------------
    // §4.6 Header counts
    // -----------------------------------------------------------------------------------------

    /// The `Quest_CountText` / `Quest_Task_Count_Text` text (§4.6 Tessera rule): `UI_Quest`
    /// 0x0E18AE34 with the count and the limit, or the count alone as a decimal integer
    /// without a limit.
    pub fn count_text(&self, count: usize, limit: Option<i64>) -> TextResult {
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let Some(limit) = limit else {
            return Ok(Some(RichText::plain(&count.to_string())));
        };
        let mut v = Values::new();
        v.insert(ids::COUNT_FIRST, Value::Integer(count));
        v.insert(ids::COUNT_SECOND, Value::Integer(limit));
        self.ui_token(tokens::COUNT_PAIR, &v)
    }

    /// The player's quest limit for `Quest_CountText` (§4.6 Tessera rule): the reported
    /// `Quest_MaxActiveQuests`, `None` while it was never reported.
    #[must_use]
    pub fn quest_limit(&self, player: &PlayerContext) -> Option<i64> {
        player.int_property(self.pids.max_active_quests.id()?, None)
    }

    /// The `Quest_Shown_Label` text (§4.6): rows shown, rows in the tree and the mode text.
    pub fn shown_text(&self, shown: usize, total: usize, mode: super::LogMode) -> TextResult {
        let mut v = Values::new();
        v.insert(
            ids::SHOWN_ROWS,
            Value::Integer(i64::try_from(shown).unwrap_or(i64::MAX)),
        );
        v.insert(
            ids::TREE_ROWS,
            Value::Integer(i64::try_from(total).unwrap_or(i64::MAX)),
        );
        let table = self
            .ui_quest_table
            .ok_or_else(|| "no UI_Quest table".to_owned())?;
        let mode_text = info_value(
            self.data,
            &super::data::token_info(table, mode.token()),
            &Values::new(),
        )?;
        v.insert(ids::SHOWN_MODE, mode_text);
        self.ui_token(tokens::SHOWN_LABEL, &v)
    }

    // -----------------------------------------------------------------------------------------
    // §4.8 Abandon confirmation
    // -----------------------------------------------------------------------------------------

    /// The abandon confirmation text (§4.8; "inside an instance" is false in P3). A literal
    /// quest (§3) takes the default token with its literal as the quest name.
    pub fn abandon_confirmation(&self, quest: &RecordRef, player: &PlayerVars) -> TextResult {
        let did = match quest {
            RecordRef::Literal(s) => {
                let mut v = Values::new();
                v.insert(ids::QUEST_NAME, self.literal_value(s));
                return self.quest_token(tokens::ABANDON_DEFAULT, &player.with(&v));
            }
            RecordRef::Record(d) => *d,
        };
        let set = self.record(did)?;
        let qp = &self.labels.quest;
        let token = if set.boolean(qp.is_instance_quest) == Some(true) {
            tokens::ABANDON_INSTANCE
        } else if set.boolean(qp.is_session_quest) == Some(true) {
            tokens::ABANDON_SESSION
        } else {
            tokens::ABANDON_DEFAULT
        };
        let mut v = Values::new();
        if let Some(n) = self.name_value(&set, player) {
            v.insert(ids::QUEST_NAME, n);
        }
        self.quest_token(token, &player.with(&v))
    }

    // -----------------------------------------------------------------------------------------
    // §7 Messages
    // -----------------------------------------------------------------------------------------

    /// The state line for a change from `previous` to `state` (§7.1 Tessera rule). A
    /// literal quest (§3) is never a deed and gives its literal as the quest name.
    pub fn state_message(
        &self,
        quest: &RecordRef,
        previous: Option<FeedState>,
        state: FeedState,
        player: &PlayerVars,
    ) -> Result<Option<QuestMessage>, String> {
        if previous == Some(state) {
            return Ok(None);
        }
        let did = match quest {
            RecordRef::Literal(s) => {
                let token = match state {
                    FeedState::Active => tokens::MSG_NEW_QUEST,
                    FeedState::Completed => tokens::MSG_COMPLETED,
                    FeedState::Failed => tokens::MSG_FAILED,
                    FeedState::Removed => return Ok(None),
                };
                let mut v = Values::new();
                v.insert(ids::QUEST_NAME, self.literal_value(s));
                return Ok(self
                    .quest_token(token, &player.with(&v))?
                    .map(QuestMessage::new));
            }
            RecordRef::Record(d) => *d,
        };
        let set = self.record(did)?;
        let q = self.quest(&set);
        let token = match state {
            FeedState::Active if q.is_deed() => {
                if q.deed().and_then(|d| d.is_hidden) == Some(true) {
                    return Ok(None);
                }
                tokens::MSG_NEW_DEED
            }
            FeedState::Active => tokens::MSG_NEW_QUEST,
            FeedState::Completed => tokens::MSG_COMPLETED,
            FeedState::Failed => tokens::MSG_FAILED,
            FeedState::Removed => return Ok(None),
        };
        let mut v = Values::new();
        if let Some(n) = self.name_value(&set, player) {
            v.insert(ids::QUEST_NAME, n);
        }
        Ok(self
            .quest_token(token, &player.with(&v))?
            .map(QuestMessage::new))
    }

    /// The objective update line for a report of objective `objective` (§7.2 Tessera
    /// rule): the objective's billboard override with objective counters, else the first
    /// completion-condition event with a billboard override and a true show flag, with
    /// event counters. `reports` are the quest's reports after the update; the caller posts
    /// the line only when the report changed `count` or `done` and names the current or an
    /// earlier objective ([`QuestPresenter::posts_objective_line`]).
    pub fn objective_message(
        &self,
        quest: Did,
        objective: i32,
        reports: &[ObjectiveReport],
        player: &PlayerVars,
    ) -> Result<Option<QuestMessage>, String> {
        let set = self.record(quest)?;
        let q = self.quest(&set);
        let objectives = q.objectives();
        let Some(o) = objectives.iter().find(|o| o.index() == Some(objective)) else {
            return Ok(None);
        };
        let report = reports.iter().find(|r| i32::from(r.objective) == objective);
        if let Some(info) = o.billboard_override() {
            let v = player.with(&self.objective_counters(o, report));
            return Ok(self.info(info, &v)?.map(QuestMessage::new));
        }
        for e in o.completion.iter().flat_map(|c| c.entries()) {
            let show = e
                .raw()
                .boolean(self.labels.event.show_billboard_text)
                .unwrap_or(self.show_billboard_default);
            if let (true, Some(info)) = (show, e.billboard_progress_override()) {
                let v = player.with(&self.event_values(e, report));
                return Ok(self.info(info, &v)?.map(QuestMessage::new));
            }
        }
        Ok(None)
    }

    /// Whether a report of `objective` posts a line (§7.2): it names the current or an
    /// earlier objective of the reports after the update. The client calls it once per
    /// changed report ([`JournalChange::objective`](super::JournalChange::objective)); a
    /// report naming an index the record does not have is ignored with the warning of §5.1
    /// and posts nothing.
    #[must_use]
    pub fn posts_objective_line(
        &self,
        set: &PropertySet,
        objective: i32,
        reports: &[ObjectiveReport],
    ) -> bool {
        let indices = self.objective_indices(set);
        if indices.binary_search(&objective).is_err() {
            tracing::warn!(objective, "report names an unknown objective; ignored");
            return false;
        }
        current_objective(&indices, reports).is_some_and(|c| objective <= c)
    }

    // -----------------------------------------------------------------------------------------
    // §8.2 Rings
    // -----------------------------------------------------------------------------------------

    /// The ring script key (an FX-mapper label) for an NPC whose
    /// `Quest_CurrentBestowRingDID` names `quest` (§8.2 Tessera rule). `npc_body` is the
    /// entity's body record. `None`: no ring (not a quest record, or completed, failed or
    /// removed).
    #[must_use]
    pub fn ring_key(
        &self,
        quest: Did,
        state: Option<FeedState>,
        reports: &[ObjectiveReport],
        npc_body: Option<Did>,
    ) -> Option<&'static str> {
        let set = self.record(quest).ok()?;
        let q = self.quest(&set);
        let scope = q.scope();
        let hard = set
            .get_pid(self.pids.exp_tier)
            .and_then(|v| match v {
                PropertyValue::Enum(k) => Some(i64::from(*k)),
                PropertyValue::Int(i) => Some(i64::from(*i)),
                _ => None,
            })
            .is_some_and(|t| t > 4);
        let advance = match state {
            None => false,
            Some(FeedState::Active) => {
                let current = current_objective(&self.objective_indices(&set), reports);
                let talks = npc_body.is_some_and(|body| {
                    q.roles().iter().any(|r| {
                        r.action() == Some(RoleAction::Talk)
                            && r.npc() == Some(body)
                            && r.objective_index().is_some()
                            && r.objective_index() == current
                    })
                });
                if !talks {
                    return Some("Quest_InProgress");
                }
                true
            }
            Some(_) => return None,
        };
        Some(ring_label(advance, scope, hard))
    }
}

/// The ring key of an available (`advance` false) or advance ring by scope and tier
/// (§8.1 variant rules E, H and the scopes; never MP).
fn ring_label(advance: bool, scope: Option<u32>, hard: bool) -> &'static str {
    match (advance, scope) {
        (_, Some(2)) => "Quest_Crafting",
        (_, Some(5)) => "Quest_Item_Advancement",
        (false, Some(9)) => "Quest_Available_Allegiance",
        (true, Some(9)) => "Quest_Advance_Allegiance",
        (false, Some(10)) => "Quest_Available_Mission",
        (true, Some(10)) => "Quest_Advance_Mission",
        (false, Some(3)) if hard => "Quest_Available_Epic_Hard",
        (false, Some(3)) => "Quest_Available_Epic",
        (true, Some(3)) if hard => "Quest_Advance_Epic_Hard",
        (true, Some(3)) => "Quest_Advance_Epic",
        (false, _) if hard => "Quest_Available_Hard",
        (false, _) => "Quest_Available",
        (true, _) if hard => "Quest_Advance_Hard",
        (true, _) => "Quest_Advance",
    }
}

/// Lower-cases with the simple Unicode mapping of each character (§4.3 Tessera rule).
#[must_use]
pub(crate) fn lower(s: &str) -> String {
    s.chars()
        .map(|c| {
            // U+0130 is the one character whose full lower-case mapping has more than one
            // character; its simple mapping (UnicodeData field 13) is U+0069.
            if c == '\u{130}' {
                return 'i';
            }
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}
