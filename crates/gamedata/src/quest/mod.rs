// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's presentation model (decision 0024 P3): quest-log entries, their
//! category, order and filter, the details pane, objective progress texts with counters,
//! tracker entries, quest chat lines, NPC rings and quest markers, and the per-record census
//! of exit criterion 1.
//!
//! Spec: `docs/specs/quest-journal.md` §3–§9 (`quests.md` for the records,
//! `npc-windows.md` §5.5 for the player variables). Widgets and panels belong to
//! `tessera-ui` and the client; this module only computes presentation from the states the
//! session reports and the install's data. It never derives a quest state, an objective
//! count, eligibility or a reward (decision 0024 §2 principle 4).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`PresentationData`], [`InstallPresentationData`] | What the models read, and its implementation over an install | — |
//! | [`QuestPresenter`] | Log entries, titles, scope bits, details pane, progress texts, tracker entries, chat lines, ring keys, difficulty states | §4–§8 |
//! | [`QuestJournal`], [`JournalEvent`], [`JournalChange`] | The client state kept from the feed | §3 |
//! | [`LogView`], [`LogMode`], [`LogFilter`] | Rows of the quest log: grouping, order, filter, shown counts | §4.2–§4.5, §4.6 shown label |
//! | [`quests_underway`], [`TaskLimiter`], [`QuestPresenter::quest_limit`], [`QuestPresenter::count_text`] | Header counts; the task limiter's category, property and default limit are supplied by the caller (not read from the install yet; OPEN in §4.6) | §4.6 |
//! | [`TrackButton`], [`AbandonButton`], [`tracker_line_budget`], [`MarkerImage`] | Controls, tracker height budget, marker images | §4.8, §6.3, §8.3 |
//! | [`PlayerContext`], [`PlayerVars`] | The player variables of `npc-windows.md` §5.5 | §5.4 |
//! | [`census`], [`QuestCategory`], [`QuestFlag`], [`QuestCensus`] | Census of every quest record | §9 |
//! | [`ids`], [`tokens`] | Argument IDs and string-table tokens the spec names | §4–§8 |
//!
//! # Tessera rules applied
//!
//! Everything marked **Tessera rule** in `quest-journal.md` is implemented as written. Two
//! representational choices of this model: an objective's progress text is returned as
//! its lines ([`ProgressText`]), with the alternative marker of §5.2 step 2.3 as a line of
//! its own, rather than as one string joined by the two join tokens (the widgets draw a
//! bullet per line); a condition that contributes no line adds no alternative marker.
//!
//! # Hostile input
//!
//! Nothing here parses bytes; records, layouts and strings are decoded by
//! `tessera-formats`, `tessera-props` and `tessera-text`. Work is bounded: the journal keeps
//! at most [`MAX_JOURNAL_QUESTS`] quests, [`MAX_OBJECTIVE_INDEX`] + 1 reports per quest and
//! [`MAX_JOURNAL_REPORTS`] reports in all;
//! a progress text holds at most [`MAX_PROGRESS_LINES`] lines; caches hold at most
//! [`MAX_CACHED`](crate::tooltip::MAX_CACHED) entries.

mod census;
mod data;
mod journal;
mod present;
mod vars;

#[cfg(test)]
pub(crate) mod tests;

pub use census::{
    MAX_LISTED, QuestCategory, QuestCensus, QuestFlag, QuestOutcome, census, classify,
};
pub use data::{
    InstallPresentationData, LayoutError, MAX_LAYOUT_WALK, PresentationData, element_property,
    find_element, layout_element, top_level_root,
};
pub use journal::{
    AbandonButton, FULL_FILTER_MASK, JournalChange, JournalEvent, JournalQuest, LogCategory,
    LogFilter, LogMode, LogRow, LogView, MAX_JOURNAL_REPORTS, MarkerImage, NORMAL_BIT,
    QUEST_NOTE_TYPE, QuestJournal, TaskLimiter, TrackButton, marker_image, quests_underway,
    tracker_line_budget,
};
pub use present::{
    DetailsMode, DetailsSection, EVEN_STATE, HeaderIcon, LevelSource, LogEntry, ProgressLine,
    ProgressSource, ProgressText, QuestHeader, QuestMessage, QuestPresenter, TrackerEntry,
    TrackerMetrics, current_objective,
};
pub use vars::{PlayerContext, PlayerVars, is_counter, is_player_variable};

pub(crate) use data::token_info;
pub(crate) use present::lower;

use tessera_formats::Did;

/// Most quests the journal keeps (a Tessera bound; `scenario-feed.md` caps a scenario's
/// records far below it).
pub const MAX_JOURNAL_QUESTS: usize = 65_536;
/// Highest objective index a report may name (`scenario-feed.md` §6.3: 0 … 1,024).
pub const MAX_OBJECTIVE_INDEX: u16 = 1_024;
/// Most lines of one progress text (a Tessera bound).
pub const MAX_PROGRESS_LINES: usize = 256;
/// Most tracked quests shown (`quest-journal.md` §6.2: capacity 10).
pub const TRACKER_CAPACITY: usize = 10;

/// A quest as the feed names it (`scenario-feed.md` §5.2 `Ref`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordRef {
    /// A record of the install.
    Record(Did),
    /// A literal (shown by its text, no category, level or objectives; §3).
    Literal(String),
}

impl From<crate::inventory::ItemRef> for RecordRef {
    fn from(r: crate::inventory::ItemRef) -> Self {
        match r {
            crate::inventory::ItemRef::Record(d) => Self::Record(d),
            crate::inventory::ItemRef::Literal(s) => Self::Literal(s),
        }
    }
}

impl RecordRef {
    /// The record, if this is one.
    #[must_use]
    pub fn did(&self) -> Option<Did> {
        match self {
            Self::Record(d) => Some(*d),
            Self::Literal(_) => None,
        }
    }
}

/// A quest's state as the feed reports it (`QuestStatus`, `quest-journal.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedState {
    /// Raw status underway.
    Active,
    /// Raw status completed.
    Completed,
    /// Raw status failed.
    Failed,
    /// Removed (abandoned or dropped): no objective reports, not tracked.
    Removed,
}

/// The latest `QuestObjective` for one (quest, objective index) pair (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveReport {
    /// `Quest_ObjectiveIndex`.
    pub objective: u16,
    /// Count.
    pub count: u32,
    /// Total.
    pub total: u32,
    /// Done.
    pub done: bool,
}

/// String-table argument IDs named by `quest-journal.md` and `npc-windows.md` §5.5.
pub mod ids {
    /// Event progress count (§5.4).
    pub const EVENT_COUNT: u32 = 0x053A_16A2;
    /// Event required number (§5.4).
    pub const EVENT_TOTAL: u32 = 0x0059_485C;
    /// Required − count (§5.4).
    pub const EVENT_LEFT: u32 = 0x06A3_0283;
    /// Objective summed counts (§5.4).
    pub const OBJECTIVE_COUNT: u32 = 0x000C_09A3;
    /// Objective summed required numbers (§5.4).
    pub const OBJECTIVE_TOTAL: u32 = 0x063E_25B3;
    /// World-event value of type-34 events (§5.4).
    pub const WORLD_VALUE: u32 = 0x005A_6195;
    /// NPC name of type-11 events (§5.4).
    pub const NPC_NAME: u32 = 0x0000_5343;
    /// The seven counter IDs of §5.4.
    pub const COUNTERS: [u32; 7] = [
        EVENT_COUNT,
        EVENT_TOTAL,
        EVENT_LEFT,
        OBJECTIVE_COUNT,
        OBJECTIVE_TOTAL,
        WORLD_VALUE,
        NPC_NAME,
    ];

    /// Level argument of the title format (§4.4).
    pub const TITLE_LEVEL: u32 = 0x0050_AA9C;
    /// Name argument of the title formats (§4.4).
    pub const TITLE_NAME: u32 = 0x0058_E905;
    /// Quest name argument of the state messages and the abandon confirmation (§4.8, §7.1).
    pub const QUEST_NAME: u32 = 0x0056_9A84;
    /// First and second argument of the join tokens (§5.2, §5.3).
    pub const JOIN_FIRST: u32 = 0x0896_E2F1;
    /// Second argument of the join tokens.
    pub const JOIN_SECOND: u32 = 0x0896_E2F2;
    /// Objective number argument of `UI_Quest` 0x0B86343C (§4.7, §5.2).
    pub const OBJECTIVE_NUMBER: u32 = 0x053A_16A2;
    /// Count arguments of `UI_Quest` 0x0E18AE34 (§4.6).
    pub const COUNT_FIRST: u32 = 0x08A7_6A74;
    /// Limit argument of `UI_Quest` 0x0E18AE34.
    pub const COUNT_SECOND: u32 = 0x0000_5168;
    /// Shown-label arguments (§4.6): rows shown, rows in the tree, mode text.
    pub const SHOWN_ROWS: u32 = 0x0048_4A34;
    /// Rows in the tree.
    pub const TREE_ROWS: u32 = 0x0059_485C;
    /// Mode text.
    pub const SHOWN_MODE: u32 = 0x0005_9E45;

    /// The player's name (`npc-windows.md` §5.5).
    pub const PLAYER_NAME: [u32; 4] = [0x0005_2615, 0x03EC_29B5, 0x0550_6DA2, 0x0DA4_7675];
    /// The species name.
    pub const PLAYER_SPECIES_NAME: u32 = 0x03EC_68D5;
    /// The class name.
    pub const PLAYER_CLASS_NAME: [u32; 2] = [0x0048_0683, 0x0EB8_DCB3];
    /// The species as an enum argument.
    pub const PLAYER_SPECIES_ENUM: u32 = 0x0005_6575;
    /// The sex as an enum argument.
    pub const PLAYER_SEX_ENUM: [u32; 2] = [0x0000_57A8, 0x0A3E_C778];
    /// The level.
    pub const PLAYER_LEVEL: u32 = 0x0050_AA9C;
    /// Surname, rank title and the titles of seven categories: empty in the sandbox
    /// (Tessera rule of `npc-windows.md` §5.5).
    pub const PLAYER_EMPTY: [u32; 9] = [
        0x08A7_2645,
        0x0005_662B,
        0x0479_68D1,
        0x0479_68D2,
        0x047C_E071,
        0x047C_E072,
        0x0543_E294,
        0x0546_6A34,
        0x0585_4A75,
    ];
}

/// String-table labels and tokens named by `quest-journal.md` (texts are found by table
/// label at run time; tokens are the spec's recorded values).
pub mod tokens {
    /// The `Quest` table.
    pub const QUEST_TABLE: &str = "Quest";
    /// The `UI_Quest` table.
    pub const UI_QUEST_TABLE: &str = "UI_Quest";
    /// The `PlayerActionHandler` table.
    pub const PLAYER_ACTION_TABLE: &str = "PlayerActionHandler";

    // `Quest` table.
    /// Category fallback (§4.3).
    pub const CATEGORY_FALLBACK: u32 = 0x0F59_FEF6;
    /// Title with level, used when the layout's format string cannot be read (§4.4).
    pub const TITLE_WITH_LEVEL: u32 = 0x0108_CAB5;
    /// Title without level (§4.4).
    pub const TITLE_WITHOUT_LEVEL: u32 = 0x0B35_5705;
    /// Completed word (§5.2).
    pub const COMPLETED_WORD: u32 = 0x0FEF_4204;
    /// Failed word (§5.2).
    pub const FAILED_WORD: u32 = 0x0828_5964;
    /// Line join (§5.2).
    pub const LINE_JOIN: u32 = 0x0A31_93D5;
    /// Alternative join (§5.2 step 2.3).
    pub const ALTERNATIVE_JOIN: u32 = 0x0FBB_D982;
    /// Deed counter suffix (§5.3 step 5).
    pub const DEED_COUNTER: u32 = 0x0F6E_F2D4;
    /// New quest (§7.1).
    pub const MSG_NEW_QUEST: u32 = 0x0413_1F74;
    /// New deed (§7.1).
    pub const MSG_NEW_DEED: u32 = 0x0475_5C44;
    /// Completed (§7.1).
    pub const MSG_COMPLETED: u32 = 0x0FF5_E454;
    /// Failed (§7.1).
    pub const MSG_FAILED: u32 = 0x028F_5AC4;
    /// Abandon confirmation: instance quest (§4.8).
    pub const ABANDON_INSTANCE: u32 = 0x092E_ECC4;
    /// Abandon confirmation: session quest.
    pub const ABANDON_SESSION: u32 = 0x0522_E51E;
    /// Abandon confirmation: inside an instance (never chosen in P3).
    pub const ABANDON_IN_INSTANCE: u32 = 0x0EF5_3D54;
    /// Abandon confirmation: other quests.
    pub const ABANDON_DEFAULT: u32 = 0x0A91_2DDE;

    // `UI_Quest` table.
    /// Objective label with its number (§4.7, §5.2).
    pub const OBJECTIVE_LABEL: u32 = 0x0B86_343C;
    /// Two integers with a separator (§4.6).
    pub const COUNT_PAIR: u32 = 0x0E18_AE34;
    /// Shown-count label (§4.6).
    pub const SHOWN_LABEL: u32 = 0x0843_160C;
    /// Active-mode text (§4.6).
    pub const MODE_ACTIVE: u32 = 0x0A6E_23FC;
    /// Completed-mode text (§4.6).
    pub const MODE_COMPLETED: u32 = 0x04E8_8B4C;
    /// Track button label and tooltip, quest not tracked (§4.8).
    pub const TRACK: (u32, u32) = (0x0C27_5572, 0x006D_3754);
    /// Track button label and tooltip, quest tracked (§4.8).
    pub const UNTRACK: (u32, u32) = (0x081F_3F12, 0x01EB_6284);
    /// Background label (§4.7).
    pub const BACKGROUND_LABEL: u32 = 0x099E_ECDC;
    /// Repeat note: completed before (§4.7).
    pub const REPEAT_DONE_BEFORE: u32 = 0x05DD_1B74;
    /// Repeat note: never completed (§4.7).
    pub const REPEAT_NEVER_DONE: u32 = 0x00B2_2074;
    /// Bestowal label (§4.7).
    pub const BESTOW_LABEL: u32 = 0x08B1_89FC;
    /// Tooltip of the ready icon (§4.4).
    pub const READY_TOOLTIP: u32 = 0x07F8_730E;
    /// Tooltip of the completed icon (§4.4).
    pub const COMPLETED_TOOLTIP: u32 = 0x07B2_36EE;

    // `PlayerActionHandler` table (tracker menu, §6.6; Tessera offers three items).
    /// Show in log.
    pub const MENU_SHOW_IN_LOG: u32 = 0x02D7_68CC;
    /// Show in deed log.
    pub const MENU_SHOW_IN_DEED_LOG: u32 = 0x0D4E_DD34;
    /// Remove.
    pub const MENU_REMOVE: u32 = 0x0ADC_8C8C;
    /// Remove all.
    pub const MENU_REMOVE_ALL: u32 = 0x0F81_D16C;

    /// Chat type of quest lines (`ChatTextType` key 21, §7.1).
    pub const CHAT_TYPE: u32 = 21;
    /// The 64-bit chat type mask of quest lines (§7.1).
    pub const CHAT_MASK: u64 = 0x10_0000;
}
