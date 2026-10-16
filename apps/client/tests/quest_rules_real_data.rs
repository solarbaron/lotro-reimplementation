// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's client rules on the user's install (read-only; decision 0024 P3,
//! `quest-journal.md` §3, §7.2, §8.2, §8.3):
//!
//! - several objective reports read in one frame each post their own counters (§7.2);
//! - a quest marker's tooltip renders a record name by family (`scenario-feed.md` §5.2);
//! - the windowed client's glue computes the ring indicator of an NPC with
//!   `Quest_CurrentBestowRingDID` (§8.2), anchors it at the entity's position, hides it
//!   with the overhead quest icons setting off, and keeps no other DID property of NPCs
//!   once the ring property is known;
//! - the player's `Quest_MaxActiveQuests` reaches the log's count text (§4.6);
//! - with every quest record active and the log open, one objective report costs one
//!   entry's work (timings printed; asserted only with `TESSERA_PERF=1`).
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test quest_rules_real_data -- --nocapture`.

mod ui_common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::character::state::NO_INSTALL_BAGS;
use tessera_client::character::{CharacterData, PlayerState};
use tessera_client::net::World;
use tessera_client::npc::{NpcData, NpcHub, NpcRefs};
use tessera_client::quests::indicators::{ANCHOR_HEIGHT, RingKind};
use tessera_client::quests::session::FeedMarker;
use tessera_client::quests::{JournalView, QuestHub, QuestNote, QuestRefs, QuestUi};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_gamedata::Family;
use tessera_gamedata::quest::RecordRef;
use tessera_net::domain::{CharacterEvent, NpcEvent, QuestStatus, Ref, Text};
use tessera_net::{DataId, EntityId, Event, Location, PropertyValue, RegionId, Transform};
use tessera_render::glam::Vec3;
use tessera_ui::game::npc::kit::FallbackArt;
use ui_common::Rig;

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
    data: Arc<CharacterData>,
    npc: Arc<NpcData>,
    set: Arc<ArchiveSet>,
}

fn install(dir: &PathBuf) -> Install {
    let data = Arc::new(CharacterData::open(dir).expect("character data"));
    let npc = Arc::new(NpcData::new(Arc::clone(&data)).expect("NPC data"));
    let set = Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("archive set"));
    Install { data, npc, set }
}

fn entered() -> Event {
    Event::WorldEntered {
        player: EntityId(1),
        name: "Tamsin".into(),
        transform: Transform::default(),
    }
}

fn state_of(events: &[Event]) -> PlayerState {
    let mut s = PlayerState::new(NO_INSTALL_BAGS);
    let mut out = Vec::new();
    s.apply(&entered(), &mut out);
    for e in events {
        s.apply(e, &mut out);
    }
    s
}

fn record(d: Did) -> Ref {
    Ref::Record(DataId(d.0))
}

fn report(q: Did, count: u32) -> Event {
    Event::Npc(NpcEvent::QuestObjective {
        quest: record(q),
        objective: 1,
        count,
        total: 9,
        done: false,
    })
}

fn active(q: Did) -> Event {
    Event::Npc(NpcEvent::QuestState {
        quest: record(q),
        state: QuestStatus::Active,
    })
}

/// The objective lines (§7.2) the notes of `state` post.
fn objective_lines(state: &PlayerState, npc: &NpcData) -> Vec<String> {
    let view = JournalView::new(state, Some(npc));
    view.session()
        .notes_since(0)
        .filter(|(_, n)| {
            matches!(
                n,
                QuestNote::Journal {
                    report: Some(_),
                    ..
                }
            )
        })
        .flat_map(|(_, n)| view.messages(n))
        .collect()
}

#[test]
fn objective_lines_marker_labels_and_rings() {
    let Some(dir) = game_dir() else { return };
    let i = install(&dir);
    let records = i
        .data
        .store()
        .family(Family::Quest)
        .expect("quest family")
        .to_vec();
    assert!(!records.is_empty());

    // §7.2: a quest whose objective-1 line shows its counters; two reports in one batch.
    let mut found = None;
    for &q in &records {
        let one = objective_lines(&state_of(&[active(q), report(q, 1)]), &i.npc);
        let two = objective_lines(&state_of(&[active(q), report(q, 2)]), &i.npc);
        if one.len() == 1 && two.len() == 1 && one != two {
            found = Some((q, one, two));
            break;
        }
    }
    let (q, one, two) = found.expect("a quest whose objective line uses its counters");
    let both = objective_lines(&state_of(&[active(q), report(q, 1), report(q, 2)]), &i.npc);
    assert_eq!(
        both,
        [one, two].concat(),
        "each line has its own report's counters"
    );
    println!("§7.2: quest {q} posts one line per report");

    // §8.3: a marker labelled by a quest's name renders that name.
    let state = state_of(&[active(q)]);
    let view = JournalView::new(&state, Some(&i.npc));
    let m = FeedMarker {
        id: 1,
        location: Location::new(RegionId(1), 0.0, 0.0, 0.0),
        label: Text::Name(DataId(q.0)),
        icon: None,
        quest: RecordRef::Record(q),
    };
    let label = view.marker(&m).label.expect("the marker has a tooltip");
    assert!(!label.trim().is_empty());
    let lit = FeedMarker {
        label: Text::Literal("Old well".into()),
        ..m
    };
    assert_eq!(view.marker(&lit).label.as_deref(), Some("Old well"));

    // §8.2: the hub's indicator over an NPC whose ring property names a quest with no
    // feed state (an available key).
    let pid = i
        .npc
        .presenter()
        .quest()
        .data()
        .pid("Quest_CurrentBestowRingDID")
        .expect("ring property");
    let quest = records
        .iter()
        .copied()
        .find(|&d| {
            let s = state_of(&[]);
            JournalView::new(&s, Some(&i.npc))
                .presenter()
                .is_some_and(|p| p.ring_key(d, None, &[], None).is_some())
        })
        .expect("a quest with an available ring");
    let mut world = World::sandbox(Instant::now()).expect("sandbox");
    for _ in 0..500 {
        if world.player().is_some() {
            break;
        }
        world.update(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    let npc_id = EntityId(4242);
    world.apply_polled(
        Instant::now(),
        &[Event::Character(CharacterEvent::PropertyChanged {
            entity: npc_id,
            property: pid,
            value: Some(PropertyValue::Did(DataId(quest.0))),
        })],
    );
    let mut rig = Rig::new(|_| {});
    let nrefs = Arc::new(NpcRefs::load(&i.set).expect("NPC window names"));
    let mut hub = NpcHub::new(Vec::new());
    hub.set_loaded(Arc::clone(&nrefs), Some(Arc::clone(&i.npc)));
    let at = Vec3::new(1.0, 2.0, 3.0);
    let position = |id: EntityId| (id == npc_id).then_some(at);
    for icons in [true, false] {
        let mut quests = QuestHub::new(Vec::new()).with_icons(icons);
        rig.frame();
        quests.frame(&mut rig.host, &mut world, &hub, &[], None, Instant::now());
        quests.update_indicators(None, &world, &hub, &position);
        let shown = quests.indicators();
        if icons {
            assert_eq!(shown.len(), 1, "{shown:?}");
            assert_eq!(shown[0].entity, npc_id);
            assert_eq!(shown[0].kind, RingKind::Available, "{}", shown[0].key);
            assert_eq!(shown[0].anchor, Some(at + Vec3::Z * ANCHOR_HEIGHT));
        } else {
            assert!(shown.is_empty(), "the overhead quest icons setting is off");
        }
    }
    // Once the ring property is known, other DID properties of NPCs are not kept.
    let session = world.player_state().quests();
    assert_eq!(session.ring_property(), Some(pid));
    let other = pid.wrapping_add(1);
    world.apply_polled(
        Instant::now(),
        &[Event::Character(CharacterEvent::PropertyChanged {
            entity: EntityId(4243),
            property: other,
            value: Some(PropertyValue::Did(DataId(0x7000_0001))),
        })],
    );
    assert_eq!(
        world.player_state().quests().entities_with(other).count(),
        0
    );
    println!("§8.2: ring {} over entity {npc_id:?}", quest);
}

#[test]
fn the_log_follows_the_player_and_refreshes_one_entry_per_report() {
    let Some(dir) = game_dir() else { return };
    let i = install(&dir);
    let records = i
        .data
        .store()
        .family(Family::Quest)
        .expect("quest family")
        .to_vec();
    let mut events: Vec<Event> = records.iter().map(|&q| active(q)).collect();
    events.push(Event::Character(CharacterEvent::LevelChanged {
        entity: EntityId(1),
        level: 50,
        quiet: false,
    }));
    let mut state = state_of(&events);
    let refs = Arc::new(QuestRefs::fallback());
    let mut quests = QuestUi::new(refs, FallbackArt::default());
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let t = Instant::now();
    {
        let view = JournalView::new(&state, Some(&i.npc));
        let mut ui = rig.host.ui_mut();
        quests.open_log(&mut ui, &view).expect("log opens");
    }
    let open = t.elapsed();
    // Read every note (the frames' bound spreads them).
    let t = Instant::now();
    let mut frames = 0;
    loop {
        let view = JournalView::new(&state, Some(&i.npc));
        let mut ui = rig.host.ui_mut();
        let out = quests.frame(&mut ui, &view, &[]);
        frames += 1;
        if out.lines.is_empty() && frames > 1 {
            break;
        }
        assert!(frames < 10_000, "notes never drained");
    }
    println!(
        "{} quests: log opened in {open:?}; notes read in {frames} frames ({:?})",
        records.len(),
        t.elapsed()
    );
    // §4.6: the player's quest limit reaches the count text without a quest event.
    let p = i
        .npc
        .data()
        .pid("Quest_MaxActiveQuests")
        .expect("the limit property");
    {
        let mut out = Vec::new();
        state.apply(
            &Event::Character(CharacterEvent::PropertyChanged {
                entity: EntityId(1),
                property: p,
                value: Some(PropertyValue::Int(25)),
            }),
            &mut out,
        );
        let view = JournalView::new(&state, Some(&i.npc));
        let mut ui = rig.host.ui_mut();
        quests.frame(&mut ui, &view, &[]);
        assert_eq!(view.quest_limit(), Some(25));
        assert_eq!(quests.log().expect("log").limit, Some(25));
    }
    // One report, one frame, each time.
    let mut worst = Duration::ZERO;
    let mut out = Vec::new();
    for (n, &q) in records.iter().take(20).enumerate() {
        state.apply(&report(q, u32::try_from(n).unwrap_or(0) + 1), &mut out);
        let t = Instant::now();
        let view = JournalView::new(&state, Some(&i.npc));
        let mut ui = rig.host.ui_mut();
        quests.frame(&mut ui, &view, &[]);
        worst = worst.max(t.elapsed());
    }
    println!("one objective report and a frame: at most {worst:?}");
    if std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1") {
        assert!(worst < Duration::from_millis(16), "{worst:?}");
    }
}
