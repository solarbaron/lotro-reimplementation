// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's client rules without an install (decision 0024 P3,
//! `quest-journal.md` §3, §4.8, §5.4, §8.3): a new session forgets the previous one's
//! entries, the abandon confirmation follows the selection, the history window keeps its
//! quest until closed, the tracker menu is a popup that a click outside closes, the log's
//! tracker check box shows the tracker's state on every open, a first `0/n` report redraws
//! the tracker, the player's values redraw the log, lost notes rebuild the markers, and a
//! burst of notes is read over several frames. Synthetic events only; no install file is
//! opened.

mod npc_windows_common;
mod ui_common;

use std::sync::Arc;

use npc_windows_common::{button, left, part, right};
use tessera_client::character::PlayerState;
use tessera_client::character::state::NO_INSTALL_BAGS;
use tessera_client::quests::present::LITERAL_KEY_BASE;
use tessera_client::quests::session::MAX_NOTES;
use tessera_client::quests::ui::MAX_NOTES_PER_FRAME;
use tessera_client::quests::{JournalView, QuestFrame, QuestRefs, QuestUi};
use tessera_net::domain::{
    CharacterEvent, MarkerId, NpcCommand, NpcEvent, QuestStatus, Ref, Text, WorldEvent,
};
use tessera_net::{Command, EntityId, Event, Location, RegionId, Transform};
use tessera_ui::game::npc::kit::FallbackArt;
use tessera_ui::game::quest::log::LogRow;
use tessera_ui::{Ui, UiEvent};
use ui_common::Rig;

struct Journal {
    rig: Rig,
    quests: QuestUi,
    state: PlayerState,
}

fn lit(s: &str) -> Ref {
    Ref::Literal(s.into())
}

fn active(name: &str) -> Event {
    Event::Npc(NpcEvent::QuestState {
        quest: lit(name),
        state: QuestStatus::Active,
    })
}

fn entered() -> Event {
    Event::WorldEntered {
        player: EntityId(1),
        name: "Tamsin".into(),
        transform: Transform::default(),
    }
}

impl Journal {
    fn new() -> Self {
        let mut j = Self {
            rig: Rig::new(|_| {}),
            quests: QuestUi::new(Arc::new(QuestRefs::fallback()), FallbackArt::default()),
            state: PlayerState::new(NO_INSTALL_BAGS),
        };
        j.apply(&entered());
        j.rig.frame();
        j
    }

    fn apply(&mut self, ev: &Event) {
        let mut out = Vec::new();
        self.state.apply(ev, &mut out);
    }

    fn open_log(&mut self) {
        let mut ui = self.rig.host.ui_mut();
        let view = JournalView::new(&self.state, None);
        self.quests.open_log(&mut ui, &view).expect("log opens");
    }

    fn frame(&mut self, ev: impl Fn(&Ui, &QuestUi) -> Vec<UiEvent>) -> QuestFrame {
        let mut ui = self.rig.host.ui_mut();
        let events = ev(&ui, &self.quests);
        let view = JournalView::new(&self.state, None);
        self.quests.frame(&mut ui, &view, &events)
    }

    fn idle(&mut self) -> QuestFrame {
        self.frame(|_, _| Vec::new())
    }

    fn names(&self) -> Vec<String> {
        let log = self.quests.log().expect("log");
        log.rows
            .iter()
            .filter_map(|(_, r)| match r {
                LogRow::Quest(i) => Some(log.entries[*i].name.clone()),
                LogRow::Category { .. } => None,
            })
            .collect()
    }

    fn part(&self, id: u32) -> tessera_ui::ElementKey {
        let root = self.quests.log_root().expect("log root");
        part(&self.rig.host.ui(), root, id).expect("part")
    }

    /// A left click on the log row of the quest with `key`.
    fn row_click(ui: &Ui, q: &QuestUi, key: u64) -> UiEvent {
        let log = q.log().expect("log");
        let (k, _) = log
            .rows
            .iter()
            .find(|(_, r)| matches!(r, LogRow::Quest(i) if log.entries[*i].quest == key))
            .expect("row");
        left(ui, *k)
    }
}

#[test]
fn a_new_session_forgets_the_previous_literal_entries() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.open_log();
    j.idle();
    assert_eq!(j.names(), vec!["Alpha".to_string()]);
    // An abandon asked for in the old session must not survive it.
    let abandon = j.part(j.quests.refs().nv.ids.abandon_button);
    let out = j.frame(|ui, _| vec![button(ui, abandon)]);
    assert_eq!(out.notices, vec!["Abandon Alpha?".to_string()]);
    // The same character enters again: the journal starts over and the first literal
    // quest reuses the first literal key.
    j.apply(&entered());
    j.apply(&active("Bravo"));
    j.idle();
    assert_eq!(j.names(), vec!["Bravo".to_string()]);
    assert_eq!(
        j.quests.log().and_then(|l| l.selected()),
        Some(LITERAL_KEY_BASE)
    );
    // The first abandon click of the new session asks again (no confirmation carried over).
    let out = j.frame(|ui, _| vec![button(ui, abandon)]);
    assert!(out.commands.is_empty(), "{:?}", out.commands);
    assert_eq!(out.notices, vec!["Abandon Bravo?".to_string()]);
    // A disconnect clears it too.
    j.apply(&Event::Disconnected {
        reason: tessera_net::DisconnectReason::ClientRequested,
    });
    j.apply(&entered());
    j.apply(&active("Charlie"));
    j.idle();
    assert_eq!(j.names(), vec!["Charlie".to_string()]);
}

#[test]
fn selecting_another_quest_cancels_the_abandon_confirmation() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&active("Bravo"));
    j.open_log();
    j.idle();
    let (alpha, bravo) = (LITERAL_KEY_BASE, LITERAL_KEY_BASE + 1);
    assert_eq!(j.quests.log().and_then(|l| l.selected()), Some(alpha));
    let abandon = j.part(j.quests.refs().nv.ids.abandon_button);
    let out = j.frame(|ui, _| vec![button(ui, abandon)]);
    assert_eq!(out.notices, vec!["Abandon Alpha?".to_string()]);
    // Bravo, then Alpha again: one abandon click only asks.
    j.frame(|ui, q| vec![Journal::row_click(ui, q, bravo)]);
    assert_eq!(j.quests.log().and_then(|l| l.selected()), Some(bravo));
    j.frame(|ui, q| vec![Journal::row_click(ui, q, alpha)]);
    let out = j.frame(|ui, _| vec![button(ui, abandon)]);
    assert!(out.commands.is_empty(), "abandoned without confirmation");
    assert_eq!(out.notices, vec!["Abandon Alpha?".to_string()]);
    // The second click confirms.
    let out = j.frame(|ui, _| vec![button(ui, abandon)]);
    assert_eq!(
        out.commands,
        vec![Command::Npc(NpcCommand::AbandonQuest {
            quest: lit("Alpha")
        })]
    );
}

#[test]
fn the_reopened_log_shows_the_trackers_state() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.open_log();
    j.idle();
    let boxk = j.part(j.quests.refs().nv.ids.show_tracker);
    j.frame(|ui, _| vec![button(ui, boxk)]);
    assert!(!j.quests.log().expect("log").tracker_shown());
    assert!(j.quests.tracker().is_none());
    {
        let mut ui = j.rig.host.ui_mut();
        j.quests.close_log(&mut ui);
    }
    j.open_log();
    assert!(
        !j.quests.log().expect("log").tracker_shown(),
        "the box shows the hidden tracker"
    );
    // One click shows it again.
    let boxk = j.part(j.quests.refs().nv.ids.show_tracker);
    j.frame(|ui, _| vec![button(ui, boxk)]);
    assert!(j.quests.log().expect("log").tracker_shown());
    assert!(j.quests.tracker().is_some());
}

#[test]
fn the_history_window_keeps_its_quest_until_closed() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&active("Bravo"));
    j.open_log();
    j.idle();
    let history = j.part(j.quests.refs().nv.ids.history_button);
    j.frame(|ui, _| vec![button(ui, history)]);
    // §4.9: the history window opens for the selected quest; the log's pane is unchanged.
    let shown = |j: &Journal| j.quests.history().and_then(|w| w.quest());
    assert_eq!(shown(&j), Some(LITERAL_KEY_BASE));
    assert_eq!(j.quests.details_shown(), Some(LITERAL_KEY_BASE));
    let root = j.quests.history().expect("history").root;
    assert_eq!(
        j.rig.host.ui().layout_box(root).map(|b| (b.w(), b.h())),
        Some(tessera_ui::game::quest::history::HISTORY_SIZE)
    );
    // Another quest's report, a marker and an NPC property change nothing of it.
    j.apply(&Event::Npc(NpcEvent::QuestObjective {
        quest: lit("Bravo"),
        objective: 1,
        count: 1,
        total: 3,
        done: false,
    }));
    j.apply(&Event::World(WorldEvent::MarkerAdded {
        marker: MarkerId(3),
        location: Location::new(RegionId(1), 1.0, 2.0, 0.0),
        label: Text::Literal("Mill".into()),
        icon: None,
        quest: Some(lit("Bravo")),
        note_type: None,
    }));
    j.idle();
    assert_eq!(shown(&j), Some(LITERAL_KEY_BASE));
    // Selecting another quest shows it in the log; the history window keeps its quest
    // until the button is used again.
    let bravo = LITERAL_KEY_BASE + 1;
    j.frame(|ui, q| vec![Journal::row_click(ui, q, bravo)]);
    assert_eq!(j.quests.details_shown(), Some(bravo));
    assert_eq!(shown(&j), Some(LITERAL_KEY_BASE));
    j.frame(|ui, _| vec![button(ui, history)]);
    assert_eq!(shown(&j), Some(bravo));
    assert_eq!(
        j.quests.history().map(|w| w.root),
        Some(root),
        "the same window"
    );
    // Its close button closes it.
    let close = j.quests.history().expect("history").closes[0];
    j.frame(|ui, _| vec![button(ui, close)]);
    assert!(j.quests.history().is_none());
    assert!(!j.rig.host.ui().exists(root));
}

#[test]
fn the_tracker_menu_is_a_popup_closed_by_a_click_outside() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&Event::Npc(NpcEvent::QuestTracked {
        quest: lit("Alpha"),
        tracked: true,
    }));
    j.idle();
    let entry_button = |j: &Journal| {
        j.quests.tracker().expect("tracker").entries[0]
            .1
            .button
            .expect("entry button")
    };
    let b = entry_button(&j);
    let out = j.frame(|ui, _| vec![right(ui, b)]);
    assert!(out.notices.is_empty(), "no notice-line stand-in");
    let popup = j.quests.menu_popup().expect("popup");
    let labels: Vec<String> = popup.items.iter().map(|i| i.text.clone()).collect();
    assert_eq!(
        labels.len(),
        3,
        "show in log, remove, remove all: {labels:?}"
    );
    assert_eq!(j.quests.menu(), Some(LITERAL_KEY_BASE));
    // A click outside the popup closes it and does nothing else.
    let catcher = popup.catcher.expect("catcher");
    let root = popup.root;
    let out = j.frame(|ui, _| vec![left(ui, catcher)]);
    assert!(out.commands.is_empty());
    assert!(j.quests.menu_popup().is_none() && j.quests.menu().is_none());
    assert!(!j.rig.host.ui().exists(root) && !j.rig.host.ui().exists(catcher));
    // "Remove" (the second item) untracks the quest and closes the popup.
    let b = entry_button(&j);
    j.frame(|ui, _| vec![right(ui, b)]);
    let row = j.quests.menu_popup().expect("popup").rows[1];
    let out = j.frame(|ui, _| vec![left(ui, row)]);
    assert_eq!(
        out.commands,
        vec![Command::Npc(NpcCommand::TrackQuest {
            quest: lit("Alpha"),
            tracked: false,
        })]
    );
    assert!(j.quests.menu_popup().is_none());
}

#[test]
fn a_first_zero_report_and_a_new_total_redraw_the_tracker() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&Event::Npc(NpcEvent::QuestTracked {
        quest: lit("Alpha"),
        tracked: true,
    }));
    j.idle();
    let progress = |j: &Journal| {
        j.quests.tracker().expect("tracker").entries[0]
            .0
            .progress
            .clone()
    };
    assert!(progress(&j).is_empty());
    let report = |total| {
        Event::Npc(NpcEvent::QuestObjective {
            quest: lit("Alpha"),
            objective: 1,
            count: 0,
            total,
            done: false,
        })
    };
    j.apply(&report(3));
    j.idle();
    assert_eq!(progress(&j), vec!["0/3".to_string()]);
    j.apply(&report(5));
    j.idle();
    assert_eq!(progress(&j), vec!["0/5".to_string()]);
    // The same report again changes nothing (no note).
    let rev = j.state.quests().revision();
    j.apply(&report(5));
    assert_eq!(j.state.quests().revision(), rev);
}

#[test]
fn the_players_level_redraws_the_details() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.open_log();
    j.idle();
    let before = j.quests.census().details_shown;
    j.idle();
    assert_eq!(j.quests.census().details_shown, before, "nothing changed");
    j.apply(&Event::Character(CharacterEvent::LevelChanged {
        entity: EntityId(1),
        level: 12,
        quiet: false,
    }));
    j.idle();
    assert_eq!(
        j.quests.census().details_shown,
        before + 1,
        "titles and difficulty follow the level (§4.4)"
    );
}

#[test]
fn lost_notes_rebuild_the_markers() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&Event::World(WorldEvent::MarkerAdded {
        marker: MarkerId(7),
        location: Location::new(RegionId(1), 1.0, 2.0, 0.0),
        label: Text::Literal("Well".into()),
        icon: None,
        quest: Some(lit("Alpha")),
        note_type: None,
    }));
    j.idle();
    assert!(j.quests.has_markers());
    // The removal's note is pushed out of the session's bound before the next frame.
    j.apply(&Event::World(WorldEvent::MarkerRemoved {
        marker: MarkerId(7),
    }));
    j.apply(&Event::World(WorldEvent::MarkerAdded {
        marker: MarkerId(9),
        location: Location::new(RegionId(1), 3.0, 4.0, 0.0),
        label: Text::Literal("Mill".into()),
        icon: None,
        quest: Some(lit("Alpha")),
        note_type: None,
    }));
    for i in 0..=MAX_NOTES {
        j.apply(&Event::Npc(NpcEvent::QuestTracked {
            quest: lit("Alpha"),
            tracked: i % 2 == 0,
        }));
    }
    assert!(j.state.quests().dropped_notes() > 0);
    let out = j.idle();
    assert!(out.rings_changed);
    let ids: Vec<u32> = j.quests.markers().markers().keys().copied().collect();
    assert_eq!(
        ids,
        vec![9],
        "the removed marker is gone, the added one is there"
    );
}

#[test]
fn a_burst_of_notes_is_read_over_several_frames() {
    let mut j = Journal::new();
    let n = MAX_NOTES_PER_FRAME * 3 + 7;
    for i in 0..n {
        j.apply(&active(&format!("Quest {i}")));
    }
    let mut lines = Vec::new();
    let mut frames = 0;
    loop {
        let out = j.idle();
        assert!(out.lines.len() <= MAX_NOTES_PER_FRAME);
        frames += 1;
        if out.lines.is_empty() {
            break;
        }
        lines.extend(out.lines);
    }
    assert_eq!(lines.len(), n);
    assert_eq!(lines[n - 1], format!("New quest: Quest {}", n - 1));
    assert_eq!(frames, 5);
}

/// `quest-journal.md` §4.8 Tessera rule (saved window choices): the saved tracker
/// visibility and log font index are taken without counting as a change (a start does not
/// rewrite the settings file), the open log shows them, and a click on the tracker check
/// box reports one change to save, once.
#[test]
fn saved_window_choices_apply_quietly_and_a_toggle_is_saved_once() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    let prefs = tessera_settings::WindowPrefs {
        tracker_shown: Some(false),
        quest_log_font: Some(4),
        ..tessera_settings::WindowPrefs::default()
    };
    {
        let mut ui = j.rig.host.ui_mut();
        j.quests.apply_prefs(&mut ui, &prefs);
    }
    assert!(!j.quests.tracker_shown());
    assert_eq!(j.quests.font_index(), Some(4));
    assert_eq!(
        j.quests.take_changed_prefs(),
        None,
        "applying is not a change"
    );
    j.open_log();
    j.idle();
    assert!(
        !j.quests.log().expect("log").tracker_shown(),
        "the box shows the saved hidden tracker"
    );
    assert_eq!(
        j.quests.take_changed_prefs(),
        None,
        "opening is not a change"
    );
    let boxk = j.part(j.quests.refs().nv.ids.show_tracker);
    j.frame(|ui, _| vec![button(ui, boxk)]);
    assert!(j.quests.tracker_shown());
    let saved = j.quests.take_changed_prefs().expect("the toggle is saved");
    assert_eq!(saved.tracker_shown, Some(true));
    assert_eq!(saved.quest_log_font, Some(4), "the font index rides along");
    assert_eq!(saved.dialogue_font, None, "only the journal's keys are set");
    assert_eq!(j.quests.take_changed_prefs(), None, "saved once");
    j.idle();
    assert_eq!(j.quests.take_changed_prefs(), None);
}
