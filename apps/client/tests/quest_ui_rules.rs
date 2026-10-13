// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's client rules without an install (decision 0024 P3,
//! `quest-journal.md` §3, §4.8, §5.4, §8.3): a new session forgets the previous one's
//! entries, the abandon confirmation and the history order follow the selection, the log's
//! tracker check box shows the tracker's state on every open, a first `0/n` report redraws
//! the tracker, the player's values redraw the log, lost notes rebuild the markers, and a
//! burst of notes is read over several frames. Synthetic events only; no install file is
//! opened.

mod npc_windows_common;
mod ui_common;

use std::sync::Arc;

use npc_windows_common::{button, left, part};
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
fn history_order_survives_unrelated_changes() {
    let mut j = Journal::new();
    j.apply(&active("Alpha"));
    j.apply(&active("Bravo"));
    j.open_log();
    j.idle();
    let history = j.part(j.quests.refs().nv.ids.history_button);
    j.frame(|ui, _| vec![button(ui, history)]);
    assert_eq!(j.quests.details_shown(), Some((LITERAL_KEY_BASE, true)));
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
    assert_eq!(j.quests.details_shown(), Some((LITERAL_KEY_BASE, true)));
    // Selecting another quest ends it.
    let bravo = LITERAL_KEY_BASE + 1;
    j.frame(|ui, q| vec![Journal::row_click(ui, q, bravo)]);
    assert_eq!(j.quests.details_shown(), Some((bravo, false)));
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
    let rev = j.state.npc().quests().revision();
    j.apply(&report(5));
    assert_eq!(j.state.npc().quests().revision(), rev);
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
    assert!(j.state.npc().quests().dropped_notes() > 0);
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
