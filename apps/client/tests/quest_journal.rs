// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal without an install (decision 0024 P3; `quest-journal.md` §3–§8,
//! `scenario-feed.md` §6.3, §6.5, §10): the committed `quest-log` scenario plays headless
//! in-process twice with byte-identical logs equal to the committed one; the journal's
//! session state follows every event (states, objective reports, tracking, markers); and
//! the log and the tracker open on the game UI as Tessera's fallback widgets, list the
//! literal quests, post the quest chat lines and turn clicks into `TrackQuest` and
//! `AbandonQuest`. No install file is opened.

mod npc_windows_common;
mod quest_common;
mod ui_common;

use std::sync::Arc;

use npc_windows_common::{button, part};
use quest_common::{committed, play_path};
use tessera_client::character::PlayerState;
use tessera_client::quests::present::LITERAL_KEY_BASE;
use tessera_client::quests::{JournalView, QuestRefs, QuestUi};
use tessera_gamedata::quest::{FeedState, RecordRef};
use tessera_net::Command;
use tessera_net::domain::{NpcCommand, Ref};
use tessera_ui::game::npc::kit::FallbackArt;
use tessera_ui::game::quest::log::LogRow;
use tessera_ui::game::quest::tracker::MenuItem;
use ui_common::Rig;

fn lit(s: &str) -> RecordRef {
    RecordRef::Literal(s.into())
}

#[test]
fn quest_log_replays_its_committed_log_in_process() {
    let (o1, a, _) = play_path(&committed(), None, None, |_, _, _| {});
    let (o2, b, _) = play_path(&committed(), None, None, |_, _, _| {});
    assert_eq!(o1.exit_code(), 0, "{o1:?}");
    assert_eq!(o2.exit_code(), 0);
    assert_eq!(a, b, "two runs differ");
    let log = std::fs::read(committed().with_extension("log")).expect("committed log");
    assert_eq!(
        String::from_utf8_lossy(&a),
        String::from_utf8_lossy(&log),
        "the log differs from the committed one"
    );
}

#[test]
fn journal_state_follows_the_feed() {
    let mut most_markers = 0;
    let (_, _, end) = play_path(&committed(), None, None, |_, _, s| {
        most_markers = most_markers.max(s.quests().markers().len());
    });
    let q = end.quests();
    let names: Vec<RecordRef> = q
        .journal()
        .quests()
        .iter()
        .map(|j| j.quest.clone())
        .collect();
    assert_eq!(
        names,
        vec![
            lit("Mill wheel"),
            lit("Flour delivery"),
            lit("Lost lamb"),
            lit("Night watch")
        ]
    );
    let state = |n: &str| q.journal().get(&lit(n)).and_then(|j| j.state);
    assert_eq!(state("Mill wheel"), Some(FeedState::Completed));
    assert_eq!(state("Flour delivery"), Some(FeedState::Active));
    assert_eq!(state("Lost lamb"), Some(FeedState::Removed));
    assert_eq!(state("Night watch"), Some(FeedState::Failed));
    let tracked: Vec<RecordRef> = q
        .journal()
        .tracked()
        .iter()
        .map(|j| j.quest.clone())
        .collect();
    assert_eq!(tracked, vec![lit("Flour delivery"), lit("Night watch")]);
    let r = q.reports(&lit("Flour delivery"));
    assert_eq!(
        (r.len(), r[0].count, r[0].total, r[0].done),
        (1, 2, 3, false)
    );
    assert!(
        q.reports(&lit("Lost lamb")).is_empty(),
        "removed drops reports"
    );
    // One quest marker at a time (the pond marker has no quest); removed at the end.
    assert_eq!(most_markers, 1);
    assert!(q.markers().is_empty());
}

/// The commands of the windows' answer to one UI event.
fn click(
    rig: &mut Rig,
    quests: &mut QuestUi,
    state: &PlayerState,
    ev: impl Fn(&tessera_ui::Ui) -> tessera_ui::UiEvent,
) -> (Vec<Command>, Vec<String>) {
    let mut ui = rig.host.ui_mut();
    let e = ev(&ui);
    let view = JournalView::new(state, None);
    let out = quests.frame(&mut ui, &view, &[e]);
    (out.commands, out.notices)
}

fn track(name: &str, tracked: bool) -> Command {
    Command::Npc(NpcCommand::TrackQuest {
        quest: Ref::Literal(name.into()),
        tracked,
    })
}

#[test]
fn log_tracker_and_lines_on_the_ui() {
    let mut rig = Rig::new(|_| {});
    let refs = Arc::new(QuestRefs::fallback());
    let ids = refs.nv.ids;
    let mut quests = QuestUi::new(refs, FallbackArt::default());
    let mut lines = Vec::new();
    let mut most_markers = 0;
    let (_, _, end) = play_path(&committed(), None, None, |i, _, s| {
        rig.frame();
        let mut ui = rig.host.ui_mut();
        let view = JournalView::new(s, None);
        if i == 0 {
            quests.open_log(&mut ui, &view).expect("log opens");
            quests.open_tracker(&mut ui, &view).expect("tracker opens");
        }
        let out = quests.frame(&mut ui, &view, &[]);
        assert!(
            out.commands.is_empty(),
            "the windows send nothing by themselves"
        );
        lines.extend(out.lines);
        most_markers = most_markers.max(quests.markers().markers().len());
    });
    assert_eq!(
        lines,
        vec![
            "New quest: Mill wheel",
            "New quest: Flour delivery",
            "New quest: Lost lamb",
            "Quest completed: Mill wheel",
            "New quest: Night watch",
            "Quest failed: Night watch",
        ]
    );
    assert_eq!(most_markers, 1);
    assert!(quests.markers().markers().is_empty());
    // The log lists the one active quest under the fallback category, selected.
    let log = quests.log().expect("log");
    let quest_rows = log
        .rows
        .iter()
        .filter(|(_, r)| matches!(r, LogRow::Quest(_)))
        .count();
    assert_eq!((log.rows.len(), quest_rows), (2, 1));
    let flour = LITERAL_KEY_BASE + 1;
    assert_eq!(log.selected(), Some(flour));
    // The tracker shows the tracked quests in arrival order, the failed one with its word.
    let t = quests.tracker().expect("tracker");
    let shown: Vec<(&str, Vec<String>)> = t
        .entries
        .iter()
        .map(|(e, _)| (e.name.as_str(), e.progress.clone()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("Flour delivery", vec!["2/3".to_string()]),
            ("Night watch", vec!["Failed".to_string()]),
        ]
    );
    let c = quests.census();
    assert!(c.log_entries > 0 && c.tracker_entries > 0 && c.details_shown > 0);
    assert_eq!((c.details_failed, c.tracker_failed), (0, 0));
    assert_eq!(c.lines_posted, 6);

    // The track button untracks the selected quest (§4.8).
    let root = quests.log_root().expect("root");
    let tb = part(&rig.host.ui(), root, ids.track_button).expect("track button");
    let (c, _) = click(&mut rig, &mut quests, &end, |ui| button(ui, tb));
    assert_eq!(c, vec![track("Flour delivery", false)]);
    // Abandon asks first, then sends AbandonQuest (§4.8 Tessera rule of this client).
    let ab = part(&rig.host.ui(), root, ids.abandon_button).expect("abandon button");
    let (c, notices) = click(&mut rig, &mut quests, &end, |ui| button(ui, ab));
    assert!(c.is_empty());
    assert_eq!(notices, vec!["Abandon Flour delivery?".to_string()]);
    let (c, _) = click(&mut rig, &mut quests, &end, |ui| button(ui, ab));
    assert_eq!(
        c,
        vec![Command::Npc(NpcCommand::AbandonQuest {
            quest: Ref::Literal("Flour delivery".into())
        })]
    );
    // The tracker menu's Remove All untracks every tracked quest in display order (§6.6).
    quests.open_menu(flour);
    let out = {
        let mut ui = rig.host.ui_mut();
        let view = JournalView::new(&end, None);
        quests.menu_choice(&mut ui, &view, MenuItem::RemoveAll)
    };
    assert_eq!(
        out.commands,
        vec![track("Flour delivery", false), track("Night watch", false)]
    );
    // Closing the log keeps the tracker.
    {
        let mut ui = rig.host.ui_mut();
        quests.close_log(&mut ui);
        assert!(!ui.exists(root));
    }
    assert!(quests.tracker().is_some());
}
