// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal on the user's install (read-only; decision 0024 P3 exit criterion 1:
//! "Log and tracker show every quest record from a scenario without failure").
//!
//! A headless scenario selects every quest record (`select $all = quest all`, deeds
//! included) and makes them all active; every poll's events reach the client's player
//! state. Then, on the game UI built from the install's layouts (the UI host without a
//! window, painted by the CPU painter):
//!
//! - the quest log lays out every listed entry, in pages of the log's bound, and the
//!   details pane of every entry (deeds are never in the log, `quest-journal.md` §4.2);
//! - the tracker shows the tracked quests in batches up to its limit of 10 (§6.2), tracked
//!   by `QuestTracked` and untracked with the generic answers to the menu's Remove All, until
//!   every quest record has been tracked once;
//! - the quest chat lines of §7.1 are posted for the new quests.
//!
//! Counts are reported by the census categories of §9 (computed with `tessera-gamedata`'s
//! census over the same records, as its own census test does): entries laid out, details
//! panes, tracker entries, lines, and 0 failures. A second test takes local-only goldens
//! (decision 0031 tier 2) of the log and the tracker on a seeded sample. Only counts,
//! categories and paths are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test quest_journal_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod quest_common;
mod ui_common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use npc_windows_common::capture;
use quest_common::play_path;
use tessera_client::character::{CharacterData, PlayerState};
use tessera_client::npc::{NpcData, NpcRefs};
use tessera_client::quests::present::net_ref;
use tessera_client::quests::{JournalView, QuestRefs, QuestUi};
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::Family;
use tessera_gamedata::quest::{
    PlayerContext, PresentationData, QuestCategory, QuestCensus, QuestFlag, RecordRef, classify,
};
use tessera_net::domain::{CharacterEvent, NpcCommand, NpcEvent};
use tessera_net::{Body, Command, Event};
use tessera_ui::game::npc::kit::FallbackArt;
use tessera_ui::game::quest::log::{LogRow, MAX_QUESTS};
use tessera_ui::game::quest::tracker::MenuItem;
use ui_common::Rig;

/// Longest wait for the UI to settle before a picture (decision 0031).
const SETTLE_LIMIT: Duration = Duration::from_secs(120);

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

struct Install {
    dir: PathBuf,
    data: Arc<CharacterData>,
    npc: Arc<NpcData>,
    refs: Arc<QuestRefs>,
    rig: Rig,
    version: String,
    _skins: tempfile::TempDir,
}

fn install(dir: PathBuf) -> Install {
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let npc = Arc::new(NpcData::new(Arc::clone(&data)).expect("NPC data"));
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let nrefs = NpcRefs::load(&set).expect("NPC window names");
    let refs = Arc::new(QuestRefs::from_install(&nrefs, &npc));
    assert!(refs.missing.is_empty(), "LayoutMissing: {:?}", refs.missing);
    let skins = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(skins.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    let version = goldens::install_version(&dir);
    Install {
        dir,
        data,
        npc,
        refs,
        rig,
        version,
        _skins: skins,
    }
}

/// The first key of `property`'s mapper with a display text (a synthetic player, as the
/// gamedata census test makes it).
fn first_key(data: &dyn PresentationData, property: &str) -> Option<u32> {
    let pid = data.pid(property)?;
    (1..512).find(|&k| data.enum_text(pid, k).is_some())
}

/// The synthetic local character of `tessera-gamedata`'s quest census test.
fn context(data: &dyn PresentationData) -> PlayerContext {
    PlayerContext {
        name: Some("Tamsin".into()),
        species: first_key(data, "Agent_Species"),
        sex: first_key(data, "Sex"),
        class: first_key(data, "Agent_Class"),
        level: Some(50),
        ..PlayerContext::default()
    }
}

/// Gives the player state the census character's values (`CharacterInfo`, as a server or
/// the character list reports them).
fn set_player(state: &mut PlayerState, c: &PlayerContext) {
    let mut out = Vec::new();
    state.apply(
        &Event::Character(CharacterEvent::CharacterInfo {
            name: "Tamsin".into(),
            body: match (c.species, c.sex) {
                (Some(species), Some(sex)) => Some(Body::Character { species, sex }),
                _ => None,
            },
            class: c.class,
            level: c.level.and_then(|l| u32::try_from(l).ok()),
            look: None,
        }),
        &mut out,
    );
}

fn write_scenario(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(format!("{name}.scenario"));
    std::fs::write(
        &p,
        format!(
            "# SPDX-License-Identifier: GPL-3.0-or-later\ntessera-scenario 1\ntitle \"{name}\"\n\
             tick 100ms\nrequires install\nplayer \"Tamsin\"\n{body}"
        ),
    )
    .expect("scenario written");
    p
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

#[test]
fn every_quest_record_in_the_log_and_the_tracker() {
    let Some(dir) = game_dir() else { return };
    let t0 = Instant::now();
    let mut i = install(dir);
    let data = Arc::clone(&i.npc);
    let presenter = data.presenter().quest();
    let ctx = context(data.data());

    // The census of §9 over every quest record (the categories the counts use).
    let records = i
        .data
        .store()
        .family(Family::Quest)
        .expect("quest family")
        .to_vec();
    let player = presenter.player(ctx.clone());
    let next = AtomicUsize::new(0);
    let census = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads())
            .map(|_| {
                s.spawn(|| {
                    let mut c = QuestCensus::default();
                    loop {
                        let k = next.fetch_add(64, Ordering::Relaxed);
                        if k >= records.len() {
                            break;
                        }
                        for &did in &records[k..(k + 64).min(records.len())] {
                            c.add(did, &classify(presenter, did, &player));
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
    println!(
        "{} quest records; categories {:?}; flags {:?}",
        census.records, census.categories, census.flags
    );
    let shown = census.count(QuestCategory::Shown);
    let deeds = census.count(QuestCategory::Deed);
    assert_eq!(
        census.count(QuestCategory::Unreadable),
        0,
        "{:?}",
        census.listed
    );
    assert_eq!(
        census.count(QuestCategory::NoName),
        0,
        "{:?}",
        census.listed
    );
    for f in [QuestFlag::TextFailure, QuestFlag::UnsuppliedArgument] {
        assert_eq!(census.flags.get(&f).copied().unwrap_or(0), 0, "{f:?}");
    }

    // The scenario: every quest record active.
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        tmp.path(),
        "every-quest",
        "select $all = quest all\n\nat 0s quest $all active\n",
    );
    let (outcome, _, mut state) = play_path(&path, Some(&i.dir), Some(i.data.bags()), |_, _, _| {});
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    set_player(&mut state, &ctx);
    let journal_len = state.npc().quests().journal().quests().len();
    println!(
        "scenario: {journal_len} quests reported active ({:?})",
        t0.elapsed()
    );
    assert_eq!(
        journal_len, census.records,
        "every quest record from the scenario"
    );

    // The log: every listed entry laid out, page by page, and its details pane.
    let rig = &mut i.rig;
    rig.frame();
    let mut quests = QuestUi::new(Arc::clone(&i.refs), FallbackArt::default());
    let t1 = Instant::now();
    let (entries, entry_failures) = {
        let view = JournalView::new(&state, Some(&data));
        let mut ui = rig.host.ui_mut();
        let first = quests.frame(&mut ui, &view, &[]);
        println!("quest chat lines posted: {}", first.lines.len());
        assert!(!first.lines.is_empty());
        quests
            .open_log(&mut ui, &view)
            .expect("the quest log opens");
        view.log_entries()
    };
    let listed: Vec<_> = entries.into_iter().filter(|e| !e.deed).collect();
    let mut rows = 0usize;
    let mut details = 0usize;
    for page in listed.chunks(MAX_QUESTS) {
        let view = JournalView::new(&state, Some(&data));
        let mut ui = rig.host.ui_mut();
        quests.show_entries(&mut ui, &view, page);
        let log = quests.log().expect("log");
        let n = log
            .rows
            .iter()
            .filter(|(_, r)| matches!(r, LogRow::Quest(_)))
            .count();
        assert_eq!(n, page.len(), "every entry of the page has a row");
        rows += n;
        for e in page {
            if quests.show_details_of(&mut ui, &view, e.quest) {
                details += 1;
            }
        }
    }
    println!(
        "log: {rows} entries laid out, {details} details panes ({:?})",
        t1.elapsed()
    );
    assert_eq!(rows, shown, "the log lists every Shown record");
    assert_eq!(details, shown);
    assert_eq!(entry_failures, 0, "entries whose title did not render");
    {
        let mut ui = rig.host.ui_mut();
        quests.close_log(&mut ui);
    }

    // The tracker: batches of up to 10 until every record was tracked once.
    let t2 = Instant::now();
    let refs: Vec<RecordRef> = state
        .npc()
        .quests()
        .journal()
        .quests()
        .iter()
        .map(|q| q.quest.clone())
        .collect();
    {
        let view = JournalView::new(&state, Some(&data));
        let mut ui = rig.host.ui_mut();
        quests
            .open_tracker(&mut ui, &view)
            .expect("the tracker opens");
    }
    let mut tracked_once = 0usize;
    let mut tracker_deeds = 0usize;
    let mut out = Vec::new();
    for batch in refs.chunks(tessera_gamedata::quest::TRACKER_CAPACITY) {
        for q in batch {
            state.apply(
                &Event::Npc(NpcEvent::QuestTracked {
                    quest: net_ref(q),
                    tracked: true,
                }),
                &mut out,
            );
        }
        let view = JournalView::new(&state, Some(&data));
        let mut ui = rig.host.ui_mut();
        quests.frame(&mut ui, &view, &[]);
        let t = quests.tracker().expect("tracker");
        assert_eq!(t.entries.len(), batch.len(), "the batch is shown");
        for (e, _) in &t.entries {
            assert!(!e.name.trim().is_empty(), "a tracker entry without a name");
            assert!(!e.progress.is_empty(), "a tracker entry without a line");
        }
        tracked_once += t.entries.len();
        tracker_deeds += t.entries.iter().filter(|(e, _)| e.deed).count();
        // Remove All from the menu; the feed's generic answer untracks (§6.6, §7.6).
        let first = t.entries[0].0.quest;
        quests.open_menu(first);
        let removed = quests.menu_choice(&mut ui, &view, MenuItem::RemoveAll);
        drop(view);
        drop(ui);
        assert_eq!(removed.commands.len(), batch.len());
        for c in removed.commands {
            let Command::Npc(NpcCommand::TrackQuest { quest, tracked }) = c else {
                panic!("not a TrackQuest: {c:?}");
            };
            state.apply(
                &Event::Npc(NpcEvent::QuestTracked { quest, tracked }),
                &mut out,
            );
        }
        out.clear();
    }
    let c = quests.census();
    println!(
        "tracker: {tracked_once} quests tracked once ({tracker_deeds} deeds), {} entries laid \
         out ({:?}); census {c:?}",
        c.tracker_entries,
        t2.elapsed()
    );
    assert_eq!(tracked_once, census.records, "every record tracked once");
    assert_eq!(tracker_deeds, deeds);
    assert_eq!(
        (c.entries_failed, c.details_failed, c.tracker_failed),
        (0, 0, 0),
        "failures"
    );
    assert!(quests.failures().is_empty(), "{:?}", quests.failures());
    println!(
        "criterion 1: Shown {shown} (log rows {rows}, details {details}), Deed {deeds} (tracker \
         only), NoName 0, Unreadable 0; tracker {tracked_once}; failures 0 ({:?})",
        t0.elapsed()
    );
}

/// Pictures checked against their local-only goldens.
struct Pictures {
    dir: PathBuf,
    flags: goldens::Flags,
    results: Vec<(String, &'static str)>,
}

impl Pictures {
    fn new(test: &str, version: &str) -> Self {
        let dir = goldens::golden_dir(env!("CARGO_PKG_NAME"), test, version)
            .unwrap_or_else(|e| panic!("{e}"));
        let mut flags = goldens::Flags::from_env();
        flags.sheet = flags.sheet.map(|s| s.join(test));
        Self {
            dir,
            flags,
            results: Vec::new(),
        }
    }

    fn take(&mut self, rig: &mut Rig, root: tessera_ui::ElementKey, name: &str) {
        let start = Instant::now();
        loop {
            rig.frame();
            if rig.host.settled() {
                break;
            }
            assert!(
                start.elapsed() < SETTLE_LIMIT,
                "the UI did not settle within {SETTLE_LIMIT:?}; no picture (decision 0031)"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let (w, h, data) = {
            let mut ui = rig.host.ui_mut();
            capture(&mut ui, root).unwrap_or_else(|| panic!("{name}: no surface"))
        };
        let img = goldens::Rgba {
            width: w,
            height: h,
            data,
        };
        match goldens::check_with(&self.dir, name, &img, &self.flags) {
            Ok(o) => {
                println!("{name}: {} ({w}x{h})", o.word());
                self.results.push((name.to_owned(), o.word()));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn log_and_tracker_goldens_on_a_seeded_sample() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir);
    let data = Arc::clone(&i.npc);
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = write_scenario(
        tmp.path(),
        "journal-sample",
        "select $some = quest deed=no n=8 seed=7\n\n\
         at 0s quest $some active\n\
         +0s track $some[0] on\n+0s track $some[1] on\n+0s track $some[2] on\n\
         +0s objective $some[0] 1 1/3\n+0.5s quest $some[7] completed\n",
    );
    let (outcome, _, mut state) = play_path(&path, Some(&i.dir), Some(i.data.bags()), |_, _, _| {});
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    set_player(&mut state, &context(data.data()));
    let mut pics = Pictures::new(
        "quest_journal_real_data/log_and_tracker_goldens_on_a_seeded_sample",
        &i.version,
    );
    let rig = &mut i.rig;
    rig.frame();
    let mut quests = QuestUi::new(Arc::clone(&i.refs), FallbackArt::default());
    let (log, tracker) = {
        let view = JournalView::new(&state, Some(&data));
        let mut ui = rig.host.ui_mut();
        let log = quests.open_log(&mut ui, &view).expect("log");
        let tracker = quests.open_tracker(&mut ui, &view).expect("tracker");
        quests.frame(&mut ui, &view, &[]);
        (log, tracker)
    };
    let l = quests.log().expect("log");
    println!(
        "sample: {} log rows, {} tracker entries",
        l.rows.len(),
        quests.tracker().map_or(0, |t| t.entries.len())
    );
    assert_eq!(quests.tracker().map(|t| t.entries.len()), Some(3));
    pics.take(rig, log, "01-quest-log");
    // The tracker's list paints into its own surface (the box's holds only the title).
    let _ = tracker;
    let list = quests.tracker().expect("tracker").list;
    pics.take(rig, list, "02-tracker");
    assert_eq!(pics.results.len(), 2);
}
