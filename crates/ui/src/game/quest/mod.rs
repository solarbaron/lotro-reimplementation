// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal of decision 0024 P3 (`docs/specs/quest-journal.md`): the quest log
//! with its grouped list, filters, header counts and buttons (§4), the details pane (§4.7),
//! the quest tracker with its height budget (§6) and the quest markers on the minimap and
//! the world map (§8.3; `game-panels.md` §8.8, §9.3).
//!
//! As in [`super::npc`], `tessera-ui` links neither the session nor the data layer: the
//! client keeps the feed's quest state (`QuestState`, `QuestObjective`, `QuestTracked`,
//! `MarkerAdded`), renders the texts with the install's string tables (titles, progress
//! lines, counts) and hands the widgets plain view structs; every user action comes back as
//! a [`QuestAction`]. Labels come in through [`QuestTexts`].
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`log`] | which quests are listed, grouping and order, filters, counts, selection, buttons; the log window | QJ §4.1–§4.6, §4.8 |
//! | [`details`] | the details pane's sections | QJ §4.7, §4.9 |
//! | [`tracker`] | tracked entries, the height budget, the entry button and menu | QJ §6 |
//! | [`markers`] | quest markers on the radar and the world map | QJ §8.3, GP §8.8, §9.3 |
//! | [`fallback`] | fallback log and tracker windows | decision 0031 |

pub mod details;
pub mod fallback;
pub mod log;
pub mod markers;
pub mod tracker;

/// The client's key of a quest (its feed `Ref`, mapped by the client).
pub type QuestKey = u64;

/// A user action in the quest journal: a session command or a request to the client
/// (`quest-journal.md` §3, §4.8, §6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestAction {
    /// `TrackQuest { quest, tracked }` (track button, tracker menu Remove / Remove All).
    TrackQuest {
        /// The quest.
        quest: QuestKey,
        /// Tracked or not.
        tracked: bool,
    },
    /// The abandon button was clicked: the client shows the confirmation dialog
    /// (`ui-behaviour.md` dialog boxes, with the token choice of §4.8) and, on OK, calls
    /// [`log::QuestLogWindow::confirm_abandon`], which gives [`QuestAction::AbandonQuest`].
    AskAbandon {
        /// The quest.
        quest: QuestKey,
    },
    /// `AbandonQuest { quest }` (after the confirmation).
    AbandonQuest {
        /// The quest.
        quest: QuestKey,
    },
    /// Open the history window for the quest (§4.9).
    ShowHistory {
        /// The quest.
        quest: QuestKey,
    },
    /// Open the quest log at the quest (tracker button, menu "show in log", §6.6).
    ShowInLog {
        /// The quest.
        quest: QuestKey,
    },
    /// Open the tracker's context menu for the quest (right click, §6.6); the client shows
    /// the items of [`tracker::MENU`] and calls [`tracker::TrackerWindow::menu_choice`].
    OpenTrackerMenu {
        /// The quest.
        quest: QuestKey,
    },
    /// Close the quest log (`QuestPanelCloseButton`, `EscapeKey`).
    CloseLog,
    /// `Quest_ShowTrackerCheckBox` was switched (§4.8: `GOP_ShowQuestTracker` as a Tessera
    /// setting): the client saves it and shows or hides the tracker panel.
    SetTrackerShown {
        /// Whether the tracker is shown.
        shown: bool,
    },
    /// The font-size arrows chose a new font index (§4.7 Tessera rule, `fonts.md` §9.2):
    /// the client keeps it in Tessera's settings and restores it with
    /// [`log::QuestLogWindow::set_font_index`].
    FontSize {
        /// The font index (0 = the first font of the lists).
        index: u32,
    },
}

/// A fixed label of the journal windows. The install's layouts carry their own; the client
/// gives the ones game code sets ([`QuestTexts::quest_label`]); the fallback widgets show the
/// client's text or Tessera's own ([`QuestLabel::fallback`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestLabel {
    /// The log's title (`UI` token 0x048AC0CC).
    LogTitle,
    /// The track button when the quest is not tracked (`UI_Quest` 0x0C275572).
    Track,
    /// The track button when it is (`UI_Quest` 0x081F3F12).
    Untrack,
    /// The track button's tooltip when not tracked (`UI_Quest` 0x006D3754).
    TrackTooltip,
    /// The track button's tooltip when tracked (`UI_Quest` 0x01EB6284).
    UntrackTooltip,
    /// The abandon button.
    Abandon,
    /// The history button.
    History,
    /// The share button.
    Share,
    /// The show-completed box.
    ShowCompleted,
    /// The background label (`UI_Quest` 0x099EECDC).
    Background,
    /// The tracker's title (`UI` token 0x0CFB777C).
    TrackerTitle,
    /// The tracker's ellipsis (`UI` token 0x037DDBC3).
    Ellipsis,
    /// Tracker menu: show in log (`PlayerActionHandler` 0x02D768CC).
    ShowInLog,
    /// Tracker menu: remove (0x0ADC8C8C).
    Remove,
    /// Tracker menu: remove all (0x0F81D16C).
    RemoveAll,
    /// Expand all.
    ExpandAll,
    /// Collapse all.
    CollapseAll,
    /// The tracker check box (`Quest_ShowTrackerCheckBox`, §4.8; the fallback window's
    /// label).
    ShowTracker,
}

impl QuestLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::LogTitle => "Quests",
            Self::Track => "Track",
            Self::Untrack => "Untrack",
            Self::TrackTooltip => "Show this quest in the tracker",
            Self::UntrackTooltip => "Remove this quest from the tracker",
            Self::Abandon => "Abandon",
            Self::History => "History",
            Self::Share => "Share",
            Self::ShowCompleted => "Completed",
            Self::Background => "Background",
            Self::TrackerTitle => "Tracked quests",
            Self::Ellipsis => "...",
            Self::ShowInLog => "Show in log",
            Self::Remove => "Remove",
            Self::RemoveAll => "Remove all",
            Self::ExpandAll => "+",
            Self::CollapseAll => "-",
            Self::ShowTracker => "Tracker",
        }
    }
}

/// Texts the journal asks the client for (rendered from the install's string tables; the
/// defaults are Tessera's own).
pub trait QuestTexts: super::npc::NpcTexts {
    /// A fixed label; `None` keeps the layout's own text (and gives the fallback widgets
    /// [`QuestLabel::fallback`]).
    fn quest_label(&self, label: QuestLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// `Quest_CountText` (§4.6 Tessera rule): the count and the limit through `UI_Quest`
    /// 0x0E18AE34, or the count alone without a reported limit. Default "count / limit".
    fn count_text(&self, count: usize, limit: Option<i64>) -> String {
        match limit {
            Some(l) => format!("{count} / {l}"),
            None => count.to_string(),
        }
    }

    /// `Quest_Shown_Label` (§4.6): rows shown, rows in the tree and the mode. Default
    /// "shown / total".
    fn shown_text(&self, shown: usize, total: usize, mode: log::LogMode) -> String {
        let _ = mode;
        format!("{shown} / {total}")
    }

    /// The task limiter's text (§4.6), when the client has its values.
    fn task_text(&self) -> Option<String> {
        None
    }
}

/// [`QuestTexts`] with every default.
impl QuestTexts for super::npc::DefaultTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn quest_label_text(texts: &dyn QuestTexts, label: QuestLabel) -> String {
    texts
        .quest_label(label)
        .unwrap_or_else(|| label.fallback().to_string())
}
