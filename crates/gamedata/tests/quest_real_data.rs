// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the quest journal model (`quest-journal.md` §9, decision 0024 P3 exit
//! criterion 1, data side) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Builds the
//! log entry, the tracker entry and the details pane of every quest record and sorts each
//! record into the categories of §9; checks QI4 and QI5. Prints counts and identifiers
//! only, never game text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tessera_gamedata::quest::{
    FeedState, InstallPresentationData, LogEntry, LogFilter, LogMode, LogView, PlayerContext,
    PresentationData, QuestCategory, QuestCensus, QuestFlag, QuestPresenter, RecordRef, classify,
    layout_element, tracker_line_budget,
};
use tessera_gamedata::{Family, GameDataStore, sample};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

/// The first key of `property`'s mapper with a display text (a synthetic player; no
/// label is named).
fn first_key(data: &dyn PresentationData, property: &str) -> Option<u32> {
    let pid = data.pid(property)?;
    (1..512).find(|&k| data.enum_text(pid, k).is_some())
}

/// A synthetic local character for the census.
pub fn context(data: &dyn PresentationData) -> PlayerContext {
    PlayerContext {
        name: Some("Tester".into()),
        species: first_key(data, "Agent_Species"),
        sex: first_key(data, "Sex"),
        class: first_key(data, "Agent_Class"),
        level: Some(50),
        ..PlayerContext::default()
    }
}

/// QI4: the layouts hold the elements the journal names (a representative set, found
/// anywhere in each layout).
const ELEMENTS: &[(&str, &[&str])] = &[
    (
        "quest",
        &[
            "QuestPanel",
            "QuestTree",
            "SelectedQuestDisplay",
            "Quest_Shown_Label",
            "Quest_CountText",
            "Quest_Filters",
            "QuestTrackerButton",
            "QuestAbandonButton",
            "QuestTreeCategoryTitle",
            "QuestTreeQuestTitle",
        ],
    ),
    (
        "questtracker",
        &[
            "QuestTracker_QuestTemplate",
            "QuestTracker_QuestTextLine1",
            "QuestTracker_QuestTextLine2",
            "QuestTracker_QuestTextEllipsis",
            "QuestTracker_ReadyToCompleteField",
        ],
    ),
    (
        "questdisplay",
        &[
            "QuestDisplay_QuestHeaderField",
            "QuestDisplay_QuestHeaderTitle",
            "QuestDisplay_QuestObjectiveDescription",
            "QuestDisplay_QuestDescription",
        ],
    ),
    (
        "questobjectives",
        &["QuestObjectives_Root", "QuestObjectives_Objective"],
    ),
    ("questhistory", &["QuestHistoryField"]),
    (
        "gameplayactivatables",
        &["GamePlay_QuestTrackerBox", "GamePlay_QuestTracker"],
    ),
];

#[test]
fn every_quest_record_gets_a_log_and_tracker_entry() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the quest journal real-data test");
        return;
    };
    let t0 = Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("presentation data");
    let quests = store.family(Family::Quest).expect("quest family");
    eprintln!(
        "{} quest records ({:?} to index)",
        quests.len(),
        t0.elapsed()
    );
    assert!(!quests.is_empty());

    let presenter = QuestPresenter::new(&data);
    let player = presenter.player(context(&data));
    eprintln!(
        "player: species {:?}, sex {:?}, class {:?}",
        player.context.species, player.context.sex, player.context.class
    );

    // QI4, QI5.
    let mut problems = Vec::new();
    for (layout, elements) in ELEMENTS {
        for e in *elements {
            if let Err(p) = layout_element(&data, layout, e) {
                problems.push(p);
            }
        }
    }
    eprintln!("layout problems: {problems:?}");
    assert!(problems.is_empty(), "QI4: {problems:?}");
    let metrics = presenter
        .tracker_metrics()
        .expect("QI5: tracker template values");
    eprintln!(
        "tracker metrics {metrics:?}; budgets 1/2/3/10: {} {} {} {}",
        tracker_line_budget(1, metrics),
        tracker_line_budget(2, metrics),
        tracker_line_budget(3, metrics),
        tracker_line_budget(10, metrics)
    );

    // §9: every record in exactly one category.
    let t1 = Instant::now();
    let next = AtomicUsize::new(0);
    let census = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads())
            .map(|_| {
                s.spawn(|| {
                    let mut c = QuestCensus::default();
                    loop {
                        let i = next.fetch_add(64, Ordering::Relaxed);
                        if i >= quests.len() {
                            break;
                        }
                        for &did in &quests[i..(i + 64).min(quests.len())] {
                            c.add(did, &classify(&presenter, did, &player));
                        }
                    }
                    c
                })
            })
            .collect();
        let mut all = QuestCensus::default();
        for w in workers {
            all.merge(w.join().expect("worker"));
        }
        all
    });
    eprintln!("census in {:?}", t1.elapsed());
    eprintln!("categories: {:?}", census.categories);
    eprintln!("flags: {:?}", census.flags);
    eprintln!(
        "objectives {} (without lines {})",
        census.objectives, census.objectives_without_lines
    );
    for (did, cat, detail) in census.listed.iter().take(20) {
        eprintln!("  {did} {cat:?} {detail:?}");
    }
    assert_eq!(census.records, quests.len());
    let total: usize = census.categories.values().sum();
    assert_eq!(total, quests.len(), "every record in exactly one category");
    assert!(census.count(QuestCategory::Shown) > 0);
    assert_eq!(
        census.count(QuestCategory::Unreadable),
        0,
        "{:?}",
        census.listed
    );
    // §9 "without failure" and QI1, QI2: no unshowable name, no failed text, no argument
    // outside the counters and the player variables.
    assert_eq!(
        census.count(QuestCategory::NoName),
        0,
        "QI1: {:?}",
        census.listed
    );
    for flag in [QuestFlag::TextFailure, QuestFlag::UnsuppliedArgument] {
        assert_eq!(
            census.flags.get(&flag).copied().unwrap_or(0),
            0,
            "{flag:?}: {:?}",
            census.flags
        );
    }

    // The log over a seeded sample as active quests: grouping, order, counts.
    let picked = sample(
        &quests
            .iter()
            .map(|&d| tessera_gamedata::Candidate::record(d))
            .collect::<Vec<_>>(),
        200,
        7,
    );
    let entries: Vec<LogEntry> = picked
        .iter()
        .filter_map(|c| presenter.log_entry(&RecordRef::Record(c.did), &player).ok())
        .collect();
    let with_state: Vec<(&LogEntry, Option<FeedState>)> = entries
        .iter()
        .map(|e| (e, Some(FeedState::Active)))
        .collect();
    let view = LogView::build(&with_state, LogMode::Active, &LogFilter::default());
    let shown = presenter
        .shown_text(view.shown_rows, view.total_rows, LogMode::Active)
        .expect("shown label renders");
    eprintln!(
        "sample log: {} categories, {} rows, shown label {}",
        view.categories.len(),
        view.total_rows,
        if shown.is_some() { "rendered" } else { "empty" }
    );
    assert!(shown.is_some());
    let count = presenter
        .count_text(view.total_rows, Some(25))
        .expect("count renders");
    assert!(count.is_some());
}
