// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal in the client (decision 0024 P3, `docs/specs/quest-journal.md`): the
//! quest log with its details pane, the quest tracker with objective counters, the quest
//! chat lines, the quest markers on the minimap and the world map, and the indicators over
//! NPC figures.
//!
//! The client derives no quest state (decision 0024 §2 principle 4): quest states,
//! objective counts, tracking, markers and NPC rings come from the scenario feed
//! (`scenario-feed.md` §6.3, §6.5, §9.2) or a server; the client shows them with the
//! install's layouts, texts and icons (`tessera-gamedata`'s quest model) and sends the
//! player's `TrackQuest` and `AbandonQuest` back.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`session`] | [`QuestSession`]: the reported quest state, quest markers, entity DID properties and change notes, a field of the player state | QJ §3 |
//! | [`present`] | [`JournalView`]: log entries, details, tracker entries, chat lines, rings, markers; [`JournalTexts`] | QJ §4–§8 |
//! | [`refs`] | [`QuestRefs`]: layouts, roots, templates and note images by label | QJ §4.1, §6.1, §8.3 |
//! | [`ui`] | [`QuestUi`]: the log, tracker and markers on the game UI; actions → commands | QJ §4, §6, §8.3 |
//! | [`indicators`] | the world-anchored indicators over ringed NPCs (seam with P2's floating text) | QJ §8.1–§8.2 |
//! | [`hub`] | [`QuestHub`]: the windowed client's frame work; `--open-panel quest-log`, `tracker` | — |
//!
//! # Bounds
//!
//! The session state keeps at most 65,536 quests and 262,144 objective reports
//! (`tessera-gamedata`'s journal), [`session::MAX_MARKERS`] quest markers,
//! [`session::MAX_ENTITY_DIDS`] entity DID properties and [`session::MAX_NOTES`] unread
//! notes; the log shows at most 1,024 quests and the tracker 10 (`tessera-ui`).

pub mod hub;
pub mod indicators;
pub mod present;
pub mod refs;
pub mod session;
pub mod ui;

pub use hub::{JournalPanel, QuestHub, is_log_key, parse_journal_panel};
pub use present::{JournalTexts, JournalView};
pub use refs::QuestRefs;
pub use session::{QuestNote, QuestSession};
pub use ui::{JournalCensus, QuestFrame, QuestUi};
