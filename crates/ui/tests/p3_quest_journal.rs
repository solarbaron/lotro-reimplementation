// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal of `docs/specs/quest-journal.md` on synthetic trees: the worked
//! examples of §12 (log order, counters in the tracker budget), the log's filters, counts,
//! selection and buttons (§4), the tracker (§6) and the quest markers on the minimap and
//! the world map (§8.3). All values are invented; element IDs are the placeholders of
//! `NpcVocab::default()` and `GameVocab::default()`.

mod p3_common;
mod ui_common;

use p3_common::*;
use tessera_formats::did::Did;
use tessera_ui::game::GameVocab;
use tessera_ui::game::map::{GameMap, Georef};
use tessera_ui::game::npc::DefaultTexts;
use tessera_ui::game::npc::offer::GroupType;
use tessera_ui::game::quest::details::{
    DetailsSection, UnderwayParts, completed_sections, underway_sections,
};
use tessera_ui::game::quest::fallback;
use tessera_ui::game::quest::log::{
    LogEntry, LogMode, LogModel, LogRow, LogTemplates, QuestLogWindow, QuestStatus, SCOPE_NORMAL,
    TRACKER_CAPACITY,
};
use tessera_ui::game::quest::markers::{
    NoteImages, QuestMarker, QuestMarkers, in_frame, radar_position, within_radius,
};
use tessera_ui::game::quest::tracker::{
    MENU, MenuItem, TrackerEntry, TrackerOrder, TrackerWindow, line2_budget,
};
use tessera_ui::game::quest::{QuestAction, QuestLabel, QuestTexts, quest_label_text};
use tessera_ui::game::radar::{Radar, RadarInput};
use tessera_ui::game::worldmap::{MapNode, MapTree, WorldMapPanel};
use tessera_ui::{ImageRef, Ui};

const T: DefaultTexts = DefaultTexts;

fn entry(quest: u64, name: &str, cat: u32, title: &str, level: i32, order: u64) -> LogEntry {
    LogEntry {
        quest,
        name: name.into(),
        title: format!("[{level}] {name}"),
        category: cat,
        category_title: title.into(),
        level,
        scope: 0x20,
        can_abandon: true,
        order,
        ..LogEntry::default()
    }
}

/// §12.1: A (category 5 "Beta", 12, "Wolves"), B (5, 12, "apples"), C (7 "alpha", 3,
/// "Cart").
fn example() -> Vec<LogEntry> {
    vec![
        entry(1, "Wolves", 5, "Beta", 12, 0),
        entry(2, "apples", 5, "Beta", 12, 1),
        entry(3, "Cart", 7, "alpha", 3, 2),
    ]
}

#[test]
fn example_12_1_log_order() {
    let es = example();
    let m = LogModel::default();
    assert_eq!(
        m.rows(&es),
        vec![
            LogRow::Category {
                key: 7,
                title: "alpha".into(),
                expanded: true
            },
            LogRow::Quest(2),
            LogRow::Category {
                key: 5,
                title: "Beta".into(),
                expanded: true
            },
            LogRow::Quest(1),
            LogRow::Quest(0),
        ]
    );
    assert_eq!(m.counts(&es), (3, 3));
    // The Task filter button alone: a solo-only quest is hidden.
    let mut es2 = es.clone();
    es2[1].scope = 0x08;
    let m = LogModel {
        filter_mask: 0x08 | SCOPE_NORMAL,
        ..LogModel::default()
    };
    assert_eq!(m.counts(&es2), (1, 3));
    assert_eq!(
        m.rows(&es2),
        vec![
            LogRow::Category {
                key: 5,
                title: "Beta".into(),
                expanded: true
            },
            LogRow::Quest(1)
        ]
    );
}

#[test]
fn listing_modes_deeds_and_filters() {
    let mut es = example();
    es[0].status = QuestStatus::Completed;
    es[1].status = QuestStatus::Failed;
    es.push(LogEntry {
        deed: true,
        ..entry(4, "Deed", 1, "D", 1, 3)
    });
    es.push(LogEntry {
        status: QuestStatus::Removed,
        ..entry(5, "Gone", 1, "D", 1, 4)
    });
    let mut m = LogModel::default();
    assert_eq!(m.counts(&es), (1, 1), "only Cart is active; deeds never");
    m.mode = LogMode::Completed;
    assert_eq!(
        m.rows(&es),
        vec![
            LogRow::Category {
                key: 5,
                title: "Beta".into(),
                expanded: true
            },
            LogRow::Quest(0)
        ]
    );
    assert_eq!(LogModel::underway(&es), 1);
    // Name filter, case-insensitive substring of the title.
    let m = LogModel {
        name_filter: "CART".into(),
        ..LogModel::default()
    };
    assert_eq!(m.counts(&example()), (1, 3));
    // Ties keep the first-reported order; equal titles of two keys are two rows.
    let ties = vec![
        entry(1, "same", 1, "X", 1, 5),
        entry(2, "Same", 2, "x", 1, 1),
        entry(3, "same", 1, "X", 1, 2),
    ];
    let rows = LogModel::default().rows(&ties);
    assert_eq!(
        rows,
        vec![
            LogRow::Category {
                key: 2,
                title: "x".into(),
                expanded: true
            },
            LogRow::Quest(1),
            LogRow::Category {
                key: 1,
                title: "X".into(),
                expanded: true
            },
            LogRow::Quest(2),
            LogRow::Quest(0),
        ]
    );
}

#[test]
fn selection_and_button_states() {
    let mut es = example();
    let mut m = LogModel::default();
    m.ensure_selection(&es);
    assert_eq!(m.selected, Some(3), "the first quest row in display order");
    assert_eq!(m.track_button(&es), (false, false));
    es[2].tracked = true;
    assert_eq!(m.track_button(&es), (false, true));
    es[2].instance = true;
    assert!(m.track_button(&es).0, "instance quests ghost the button");
    es[2].instance = false;
    es[2].can_abandon = false;
    assert!(m.abandon_ghosted(&es));
    // Capacity: ten tracked, the selected one not tracked.
    let mut many: Vec<LogEntry> = (0..11)
        .map(|i| entry(i, &format!("q{i}"), 1, "c", 1, i))
        .collect();
    for e in many.iter_mut().skip(1).take(TRACKER_CAPACITY) {
        e.tracked = true;
    }
    let mut m = LogModel {
        selected: Some(0),
        ..LogModel::default()
    };
    assert_eq!(m.track_button(&many), (true, false));
    // The selected quest left the list: the first row again; nothing listed: none.
    m.selected = Some(99);
    m.ensure_selection(&many);
    assert_eq!(m.selected, Some(0));
    m.ensure_selection(&[]);
    assert_eq!(m.selected, None);
    assert_eq!(m.track_button(&[]), (true, false));
    assert!(m.abandon_ghosted(&[]));
}

#[test]
fn log_window_commands() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    let mut es = example();
    es[0].tracked = true;
    w.show(&mut ui, &es, &T);
    assert_eq!(w.filters.len(), 9);
    assert_eq!(ui.text_of(part(&ui, root, nv.ids.shown_label)), "3 / 3");
    assert_eq!(ui.text_of(part(&ui, root, nv.ids.count_text)), "3");
    assert!(
        ui.prop(
            part(&ui, root, nv.ids.share_button),
            ui.vocab().pids.button_ghosted
        )
        .bool()
    );
    // Select Wolves by clicking its row; the track button reads Untrack.
    let wolves = w
        .rows
        .iter()
        .find(|(_, r)| *r == LogRow::Quest(0))
        .unwrap()
        .0;
    let ev = left(&ui, wolves);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None);
    assert_eq!(w.selected(), Some(1));
    let track = part(&ui, root, nv.ids.track_button);
    assert_eq!(ui.text_of(track), quest_label_text(&T, QuestLabel::Untrack));
    let ev = button(&ui, track);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::TrackQuest {
            quest: 1,
            tracked: false
        })
    );
    // Abandon asks first; OK sends, cancel does not.
    let ab = part(&ui, root, nv.ids.abandon_button);
    let ev = button(&ui, ab);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::AskAbandon { quest: 1 })
    );
    assert_eq!(
        w.confirm_abandon(true),
        Some(QuestAction::AbandonQuest { quest: 1 })
    );
    assert_eq!(w.confirm_abandon(true), None);
    let ev = button(&ui, ab);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.confirm_abandon(false), None);
    let h = part(&ui, root, nv.ids.history_button);
    let ev = button(&ui, h);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::ShowHistory { quest: 1 })
    );
    // A category row collapses and expands.
    let beta = w
        .rows
        .iter()
        .find(|(_, r)| matches!(r, LogRow::Category { key: 5, .. }))
        .unwrap()
        .0;
    let ev = left(&ui, beta);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 3);
    let ex = part(&ui, root, nv.ids.expand_all);
    let ev = button(&ui, ex);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 5);
    let co = part(&ui, root, nv.ids.collapse_all);
    let ev = button(&ui, co);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 2);
    // A filter button toggles its bit; the name filter is read from its entry.
    w.expand_all(&mut ui, true, &T);
    let solo = w.filters.iter().find(|b| b.mask == 0x20).unwrap().element;
    let ev = button(&ui, solo);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.model.counts(&w.entries), (0, 3));
    assert_eq!(ui.text_of(part(&ui, root, nv.ids.shown_label)), "0 / 3");
    let ev = button(&ui, solo);
    w.handle_event(&mut ui, &ev, &T);
    let ft = part(&ui, root, nv.ids.filter_text);
    ui.set_text(ft, Some("wol".into()));
    let ev = tessera_ui::UiEvent::Cursor { cursor: None };
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.model.counts(&w.entries), (1, 3));
    // Completed mode via the box; show in log switches back.
    ui.set_text(ft, Some(String::new()));
    w.handle_event(&mut ui, &ev, &T);
    let sc = part(&ui, root, nv.ids.show_completed);
    let ev = button(&ui, sc);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.model.mode, LogMode::Completed);
    assert!(w.rows.is_empty());
    w.show_quest(&mut ui, 2, &T);
    assert_eq!((w.model.mode, w.selected()), (LogMode::Active, Some(2)));
    let close = part(&ui, root, nv.ids.quest_panel_close);
    let ev = button(&ui, close);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::CloseLog)
    );
    w.detach(&mut ui);
}

#[test]
fn log_layouts_of_one_many_and_the_maximum() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    for n in [1u64, 60, 5000] {
        let es: Vec<LogEntry> = (0..n)
            .map(|i| {
                entry(
                    i,
                    &"q".repeat((i % 300) as usize),
                    (i % 7) as u32,
                    &format!("c{}", i % 7),
                    i32::MAX - i as i32,
                    i,
                )
            })
            .collect();
        w.show(&mut ui, &es, &T);
        let quests = w
            .rows
            .iter()
            .filter(|(_, r)| matches!(r, LogRow::Quest(_)))
            .count();
        assert_eq!(
            quests as u64,
            n.min(tessera_ui::game::quest::log::MAX_QUESTS as u64)
        );
    }
    settle(&mut ui);
}

#[test]
fn details_sections_in_order() {
    let parts = UnderwayParts {
        title: "T".into(),
        arc: Some("Arc".into()),
        category: "C".into(),
        group: GroupType::Raid,
        progress: vec!["p1".into()],
        objective_text: Some(String::new()),
        description: Some(("L".into(), "D".into())),
        repeat_note: Some("R".into()),
        rewards: vec![],
    };
    let s = underway_sections(&parts);
    assert_eq!(
        s,
        vec![
            DetailsSection::Header {
                title: "T".into(),
                arc: Some("Arc".into()),
                category: "C".into(),
                group: GroupType::Raid
            },
            DetailsSection::Progress(vec!["p1".into()]),
            DetailsSection::Label("L".into()),
            DetailsSection::Text("D".into()),
            DetailsSection::Text("R".into()),
        ]
    );
    let c = completed_sections(
        s[0].clone(),
        Some(("BL".into(), "B".into())),
        None,
        &[("O1".into(), "d1".into()), ("O2".into(), "d2".into())],
        &[],
    );
    assert_eq!(c.len(), 7);
    assert_eq!(c[1], DetailsSection::Label("BL".into()));
    assert_eq!(c[6], DetailsSection::Text("d2".into()));
}

#[test]
fn details_pane_header_without_arc() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    w.show(&mut ui, &example(), &T);
    let parts = UnderwayParts {
        title: "T".into(),
        category: "C".into(),
        group: GroupType::SmallFellowship,
        ..UnderwayParts::default()
    };
    w.show_details(
        &mut ui,
        Some(&tessera_ui::game::quest::details::QuestDetails {
            sections: underway_sections(&parts),
        }),
        &T,
    );
    let d = w.details.as_ref().unwrap();
    assert_eq!(d.rows.len(), 1);
    let arc = part(&ui, d.rows[0], nv.ids.header_arc);
    let cat = part(&ui, d.rows[0], nv.ids.header_category);
    assert!(!ui.is_visible(arc));
    // The category moves up to the mean of the two tops.
    assert_eq!(ui.layout_box(cat).unwrap().t, (24 + 42) / 2);
    let icon = part(&ui, d.rows[0], nv.ids.header_icon);
    assert_eq!(ui.state(icon), Some(nv.states.quest_small_fellowship));
    w.show_details(&mut ui, None, &T);
    assert!(w.details.as_ref().unwrap().rows.is_empty());
}

// ----- §6 tracker -----------------------------------------------------------------------

#[test]
fn example_12_2_budget_and_order() {
    assert_eq!(line2_budget(1, 20, 12), 240);
    assert_eq!(line2_budget(2, 20, 12), 120);
    assert_eq!(line2_budget(3, 20, 12), 72);
    assert_eq!(line2_budget(10, 20, 12), 24);
    assert_eq!(line2_budget(0, 20, 12), 240);
    assert_eq!(line2_budget(1, -5, i32::MAX), 0);
    assert_eq!(line2_budget(usize::MAX, 20, 12), 0);
    let mut o = TrackerOrder::default();
    for q in 0..12u64 {
        o.tracked(q, true);
    }
    o.tracked(3, true);
    assert_eq!(o.shown().len(), TRACKER_CAPACITY);
    assert_eq!(o.all().len(), 12, "an 11th is kept but not shown");
    o.tracked(0, false);
    assert_eq!(o.shown()[0], 1);
    assert_eq!(o.shown()[9], 10, "the 11th moves up when another leaves");
}

fn tentry(quest: u64, lines: usize, ready: bool, deed: bool) -> TrackerEntry {
    TrackerEntry {
        quest,
        name: format!("Quest {quest}"),
        progress: (0..lines).map(|i| format!("Line {i}")).collect(),
        ready,
        deed,
    }
}

#[test]
fn tracker_entries_and_menu() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (_, list) = fallback::tracker(&mut ui, &nv, &a, &T).unwrap();
    let mut t = TrackerWindow::attach(&mut ui, &nv, list.unwrap(), None, &a);
    t.show(
        &mut ui,
        &[tentry(1, 1, true, false), tentry(2, 30, true, true)],
        &T,
    );
    let p1 = t.entries[0].1;
    assert!(!p1.cut && !ui.is_visible(p1.ellipsis.unwrap()));
    assert!(ui.is_visible(p1.ready.unwrap()));
    let p2 = t.entries[1].1;
    assert!(p2.cut && ui.is_visible(p2.ellipsis.unwrap()));
    assert_eq!(p2.line2_height, 120);
    assert!(!ui.is_visible(p2.ready.unwrap()), "never for a deed");
    // Entries stack.
    let b1 = ui.layout_box(p1.entry).unwrap();
    let b2 = ui.layout_box(p2.entry).unwrap();
    assert!(b2.t > b1.t);
    let ev = button(&ui, p1.button.unwrap());
    assert_eq!(
        t.handle_event(&mut ui, &ev),
        Some(QuestAction::ShowInLog { quest: 1 })
    );
    let ev = button(&ui, p2.button.unwrap());
    assert_eq!(t.handle_event(&mut ui, &ev), None, "no deed log");
    let ev = right(&ui, p2.button.unwrap());
    assert_eq!(
        t.handle_event(&mut ui, &ev),
        Some(QuestAction::OpenTrackerMenu { quest: 2 })
    );
    assert_eq!(MENU.len(), 3);
    assert_eq!(t.menu_choice(2, MenuItem::ShowInLog), vec![]);
    assert_eq!(
        t.menu_choice(1, MenuItem::ShowInLog),
        vec![QuestAction::ShowInLog { quest: 1 }]
    );
    assert_eq!(
        t.menu_choice(2, MenuItem::Remove),
        vec![QuestAction::TrackQuest {
            quest: 2,
            tracked: false
        }]
    );
    assert_eq!(
        t.menu_choice(1, MenuItem::RemoveAll),
        vec![
            QuestAction::TrackQuest {
                quest: 1,
                tracked: false
            },
            QuestAction::TrackQuest {
                quest: 2,
                tracked: false
            },
        ]
    );
    // One, many, more than the capacity; hostile texts.
    for n in [1usize, 10, 50] {
        let es: Vec<TrackerEntry> = (0..n as u64).map(|q| tentry(q, 3, false, false)).collect();
        t.show(&mut ui, &es, &T);
        assert_eq!(t.entries.len(), n.min(TRACKER_CAPACITY));
        let budget = line2_budget(n.min(TRACKER_CAPACITY), 20, 12);
        assert!(t.entries.iter().all(|(_, p)| p.line2_height <= budget));
    }
    let mut e = tentry(9, 0, false, false);
    e.name = "n".repeat(100_000);
    e.progress = vec![String::new(), "x".repeat(100_000)];
    t.show(&mut ui, &[e], &T);
    settle(&mut ui);
    t.show(&mut ui, &[], &T);
    assert!(t.entries.is_empty());
    t.detach(&mut ui);
}

// ----- §8.3 markers ---------------------------------------------------------------------

#[test]
fn marker_geometry() {
    assert_eq!(radar_position((100, 100), (110, 90), (85, 85)), (95, 75));
    assert!(within_radius((0, 0), (81, 0), 81));
    assert!(!within_radius((0, 0), (58, 58), 81));
    assert!(!within_radius((0, 0), (1, 0), -5));
    // Extreme values do not overflow.
    let _ = within_radius((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), i32::MAX);
    assert!(in_frame((0, 0), (1024, 768)));
    assert!(!in_frame((1024, 0), (1024, 768)));
    assert!(!in_frame((-1, 5), (1024, 768)));
}

const RADAR_LAYOUT: u32 = 0x22FF_7000;
const RADAR: u32 = 0x1000_7001;
const MAP_LAYOUT: u32 = 0x22FF_7100;
const MAP_ROOT: u32 = 0x1000_7101;
const NODE: u32 = 0x1000_7102;
const NOTE: u32 = 0x41FF_7000;
const NOTE_HI: u32 = 0x41FF_7001;

fn images(env: &mut tessera_ui::MemoryEnv) {
    ui_common::add_image(env, NOTE, ui_common::solid(10, 10, 0xFFFF_FF00));
    ui_common::add_image(env, NOTE_HI, ui_common::solid(10, 10, 0xFFFF_FFFF));
}

fn notes() -> NoteImages {
    NoteImages {
        normal: Some(ImageRef::Did(NOTE)),
        highlight: Some(ImageRef::Did(NOTE_HI)),
    }
}

/// A radar with its parts (`game-panels.md` §8.1), radius 81.
fn radar_ui() -> (Ui, Radar, GameVocab) {
    use ui_common::*;
    let g = GameVocab::default();
    let (ids, t, vv) = (g.ids, g.types, v().types);
    let mut viewport = field(ids.minimap_viewport, 27, 0, 170, 170);
    for c in [
        typed(ids.radar_map_image, vv.list_box, 0, 0, 600, 600),
        field(ids.minimap_map_image, 0, -1, 2048, 1536),
        child(
            field(ids.fog_display, 0, 0, 2048, 1536),
            child(
                field(ids.fog_parent, 0, 0, 2048, 1536),
                field(ids.fog_field, 0, 0, 2048, 1536),
            ),
        ),
        field(ids.player_marker, 0, 3, 28, 28),
    ] {
        viewport = child(viewport, c);
    }
    let mut radar = prop(
        typed(RADAR, t.radar, 0, 0, 224, 260),
        g.pids.radar_radius,
        int(81),
    );
    for c in [
        viewport,
        typed(ids.area_label, vv.text, 27, 200, 170, 20),
        typed(ids.coordinate_label, vv.text, 12, 220, 200, 20),
    ] {
        radar = child(radar, c);
    }
    let mut env = env_with(vec![layout(RADAR_LAYOUT, 224, 260, vec![radar])]);
    images(&mut env);
    let mut ui = ui_with(env, 1280, 960);
    let root = ui.open_panel(Did(RADAR_LAYOUT), RADAR).unwrap();
    let mut r = Radar::post_create(&mut ui, &g, root, None);
    // The outdoor grid's georeference: block (100, 150), pixel (200, 400), 1.25 px per unit.
    r.outdoors = Some(true);
    r.grid_georef = Some(Georef {
        block_x: 100,
        block_y: 150,
        pixel_x: 200,
        pixel_y: 400,
        scale: 1.25,
    });
    (ui, r, g)
}

fn marker(id: u32, global: (f32, f32)) -> QuestMarker {
    QuestMarker {
        id,
        quest: u64::from(id),
        global,
        icon: None,
        label: Some(format!("Marker {id}")),
    }
}

#[test]
fn radar_markers_inside_the_radius() {
    let (mut ui, radar, g) = radar_ui();
    let mut qm = QuestMarkers::default();
    let player = (16_000.0f32, 24_000.0f32);
    assert!(qm.add(marker(1, (16_020.0, 24_000.0)))); // 25 px east
    assert!(qm.add(marker(2, (16_200.0, 24_000.0)))); // 250 px east: outside
    assert!(qm.add(marker(3, (f32::NAN, f32::INFINITY)))); // hostile
    let input = RadarInput {
        region: 1,
        x: 100,
        y: 150,
        cell: 0,
        global: player,
        ..RadarInput::default()
    };
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    let m1 = qm.radar_element(radar.key, 1).expect("inside the radius");
    let b = ui.layout_box(m1).unwrap();
    // Centred on c + (25, 0) = (110, 85).
    assert_eq!((b.l + b.w() / 2, b.t + b.h() / 2), (110, 85));
    assert_eq!((b.w(), b.h()), (10, 10), "the image's size");
    assert_eq!(ui.parent(m1), radar.viewport());
    assert!(qm.radar_element(radar.key, 2).is_none());
    assert_eq!(
        *ui.children(radar.viewport().unwrap()).last().unwrap(),
        radar.player_marker().unwrap(),
        "the player marker stays in front"
    );
    // Moving away hides it; removing destroys it.
    let far = RadarInput {
        global: (17_000.0, 24_000.0),
        ..input.clone()
    };
    qm.update_radar(&mut ui, &g, &radar, &far, &notes());
    assert!(!ui.is_visible(m1));
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    assert!(ui.is_visible(m1));
    qm.remove(&mut ui, 1);
    assert!(!ui.exists(m1));
    // An invalid position hides every marker.
    qm.add(marker(4, player));
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    let m4 = qm.radar_element(radar.key, 4).unwrap();
    qm.update_radar(&mut ui, &g, &radar, &RadarInput::default(), &notes());
    assert!(!ui.is_visible(m4));
}

#[test]
fn marker_cap() {
    let mut qm = QuestMarkers::default();
    for i in 0..tessera_ui::game::quest::markers::MAX_MARKERS as u32 {
        assert!(qm.add(marker(i, (0.0, 0.0))));
    }
    assert!(!qm.add(marker(u32::MAX, (0.0, 0.0))));
    assert!(qm.add(marker(0, (1.0, 1.0))), "replacing an ID is allowed");
}

#[test]
fn world_map_markers_inside_the_frame() {
    use ui_common::*;
    let g = GameVocab::default();
    let ids = g.ids;
    let bg = field(ids.map_background, 0, 0, 1024, 768);
    let root = child(field(MAP_ROOT, 0, 0, 1040, 800), bg);
    let node = field(NODE, 0, 0, 1024, 768);
    let mut env = env_with(vec![layout(MAP_LAYOUT, 1040, 800, vec![root, node])]);
    images(&mut env);
    let mut ui = ui_with(env, 1280, 960);
    let rk = ui.open_panel(Did(MAP_LAYOUT), MAP_ROOT).unwrap();
    let mut tree = MapTree::default();
    tree.nodes.push(MapNode {
        parent: None,
        depth: 0,
        active_element: NODE,
        menu_name: None,
        map: GameMap {
            image: 0,
            block_x: Some(100),
            block_y: Some(150),
            pixel_x: 0,
            pixel_y: 768,
            scale: Some(0.5),
            ..GameMap::default()
        },
        pixel_size: (1024, 768),
        region: Some(1),
        areas: vec![],
        children: vec![],
    });
    let mut map = WorldMapPanel {
        root: rk,
        layout: Did(MAP_LAYOUT),
        map_window: None,
        tree: Some(tree),
        shown: None,
        dungeon: None,
        buttons: vec![],
        marker: None,
        indoor_pending: false,
    };
    map.show_node(&mut ui, &g, 0).unwrap();
    assert_eq!(map.shown_frame(), Some((1024, 768)));
    let mut qm = QuestMarkers::default();
    qm.add(marker(1, (16_000.0 + 200.0, 24_000.0 + 100.0))); // (100, 718)
    qm.add(marker(2, (16_000.0 - 10.0, 24_000.0))); // x < 0: outside
    qm.update_world_map(&mut ui, &g, &map, &notes()).unwrap();
    let m1 = qm.map_element(1).unwrap();
    let b = ui.layout_box(m1).unwrap();
    assert_eq!((b.l + b.w() / 2, b.t + b.h() / 2), (100, 718));
    assert!(qm.map_element(2).is_none());
    assert_eq!(ui.base_image(m1).map(|(i, _)| i), Some(ImageRef::Did(NOTE)));
    qm.remove(&mut ui, 1);
    assert!(qm.map_element(1).is_none());
    // No node shown: every marker hidden.
    qm.add(marker(3, (16_100.0, 24_100.0)));
    map.hide_node(&mut ui);
    qm.update_world_map(&mut ui, &g, &map, &notes()).unwrap();
    assert!(qm.map_element(3).is_none());
    qm.prune(&ui);
}

struct Labels;

impl tessera_ui::game::npc::NpcTexts for Labels {}

impl QuestTexts for Labels {
    fn quest_label(&self, l: QuestLabel) -> Option<String> {
        (l == QuestLabel::Track).then(|| "Follow".to_string())
    }

    fn shown_text(&self, shown: usize, total: usize, mode: LogMode) -> String {
        format!("{shown} of {total} ({mode:?})")
    }
}

#[test]
fn texts_from_the_client() {
    assert_eq!(quest_label_text(&Labels, QuestLabel::Track), "Follow");
    assert_eq!(quest_label_text(&Labels, QuestLabel::Abandon), "Abandon");
    assert_eq!(T.count_text(3, Some(100)), "3 / 100");
    assert_eq!(T.count_text(3, None), "3");
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &Labels).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    w.show(&mut ui, &example(), &Labels);
    assert_eq!(
        ui.text_of(part(&ui, root, nv.ids.shown_label)),
        "3 of 3 (Active)"
    );
    assert_eq!(ui.text_of(part(&ui, root, nv.ids.track_button)), "Follow");
}

// ----- review regressions ---------------------------------------------------------------

#[test]
fn a_replaced_marker_is_drawn_again() {
    let (mut ui, radar, g) = radar_ui();
    let mut qm = QuestMarkers::default();
    let player = (16_000.0f32, 24_000.0f32);
    let input = RadarInput {
        region: 1,
        x: 100,
        y: 150,
        cell: 0,
        global: player,
        ..RadarInput::default()
    };
    qm.add(marker(1, (16_020.0, 24_000.0)));
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    let old = qm.radar_element(radar.key, 1).unwrap();
    // The same ID again with the same icon and label keeps the element.
    qm.add(marker(1, (16_010.0, 24_000.0)));
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    assert_eq!(qm.radar_element(radar.key, 1), Some(old));
    // A new label and icon: the element is made again.
    let mut m = marker(1, (16_020.0, 24_000.0));
    m.label = Some("Replaced".into());
    m.icon = Some(ImageRef::Did(NOTE_HI));
    qm.add(m);
    qm.update_radar(&mut ui, &g, &radar, &input, &notes());
    let new = qm.radar_element(radar.key, 1).unwrap();
    assert_ne!(new, old);
    assert!(!ui.exists(old));
    assert!(ui.is_visible(new));
}

#[test]
fn remove_all_covers_tracked_quests_past_the_capacity() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (_, list) = fallback::tracker(&mut ui, &nv, &a, &T).unwrap();
    let mut t = TrackerWindow::attach(&mut ui, &nv, list.unwrap(), None, &a);
    let es: Vec<TrackerEntry> = (1..=11).map(|q| tentry(q, 1, false, false)).collect();
    t.show(&mut ui, &es, &T);
    assert_eq!(t.entries.len(), TRACKER_CAPACITY);
    let all = t.menu_choice(1, MenuItem::RemoveAll);
    assert_eq!(all.len(), 11);
    assert_eq!(
        all[10],
        QuestAction::TrackQuest {
            quest: 11,
            tracked: false
        }
    );
}

struct TaskTexts(Option<&'static str>);

impl tessera_ui::game::npc::NpcTexts for TaskTexts {}

impl QuestTexts for TaskTexts {
    fn task_text(&self) -> Option<String> {
        self.0.map(str::to_string)
    }
}

#[test]
fn task_text_tracker_box_and_font_arrows_of_the_log() {
    use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let task = part(&ui, root, nv.ids.task_count_text);
    ui.set_text(task, Some("placeholder".into()));
    // The sample carries a font list of three fonts.
    let sample = part(&ui, root, nv.ids.quest_font_example);
    let dids = [0x4100_0201u32, 0x4100_0202, 0x4100_0203];
    let arr = PropertyValue::Array(PropertyArray {
        count: 3,
        elements: dids
            .iter()
            .map(|d| PropertyEntry {
                pid: nv.pids.text_font,
                value: PropertyValue::DidRef(Did(*d)),
            })
            .collect(),
    });
    let fonts_pid = ui.vocab().pids.text_fonts;
    ui.set_property(sample, fonts_pid, arr);
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    // §4.6: the task text, or nothing; never the layout's placeholder.
    w.show(&mut ui, &example(), &TaskTexts(Some("2 / 5")));
    assert_eq!(ui.text_of(task), "2 / 5");
    w.show(&mut ui, &example(), &TaskTexts(None));
    assert_eq!(ui.text_of(task), "");
    // §4.8: the tracker check box.
    let hl = ui.vocab().pids.button_highlighted;
    let cb = part(&ui, root, nv.ids.show_tracker);
    assert!(ui.prop(cb, hl).bool() && w.tracker_shown());
    let ev = button(&ui, cb);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::SetTrackerShown { shown: false })
    );
    assert!(!ui.prop(cb, hl).bool());
    let ev = button(&ui, cb);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::SetTrackerShown { shown: true })
    );
    w.set_tracker_shown(&mut ui, false);
    assert!(!ui.prop(cb, hl).bool());
    // §4.7: the font arrows step the sample's list, clamped.
    let font = |ui: &Ui| match ui.property(sample, nv.pids.text_font) {
        Some(PropertyValue::DidRef(d)) => Some(d.0),
        _ => None,
    };
    assert_eq!(font(&ui), Some(dids[0]));
    let up = part(&ui, root, nv.ids.quest_font_up);
    let down = part(&ui, root, nv.ids.quest_font_down);
    let ev = button(&ui, up);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(QuestAction::FontSize { index: 1 })
    );
    assert_eq!(font(&ui), Some(dids[1]));
    let ev = button(&ui, down);
    w.handle_event(&mut ui, &ev, &T);
    let ev = button(&ui, down);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None, "clamped at 0");
    assert!(w.set_font_index(&mut ui, 2));
    assert_eq!((w.font_index(), font(&ui)), (2, Some(dids[2])));
}

#[test]
fn show_in_log_clears_the_filters_that_hide_the_quest() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    let mut es = example();
    es[1].scope = 0x08; // apples: a task
    let mut failed = entry(4, "Lost", 7, "alpha", 1, 3);
    failed.status = QuestStatus::Failed;
    es.push(failed);
    w.show(&mut ui, &es, &T);
    // The name filter hides apples (quest 2).
    let ft = part(&ui, root, nv.ids.filter_text);
    ui.set_text(ft, Some("wol".into()));
    w.handle_event(&mut ui, &tessera_ui::UiEvent::Cursor { cursor: None }, &T);
    assert_eq!(w.selected(), Some(1));
    w.show_quest(&mut ui, 2, &T);
    assert_eq!(w.selected(), Some(2));
    assert!(w.model.name_filter.is_empty());
    assert_eq!(ui.text_of(ft), "");
    // The scope filter hides it.
    w.toggle_filter(&mut ui, 0x08, &T);
    assert_eq!(w.selected(), Some(3), "the first shown quest (alpha: Cart)");
    w.show_quest(&mut ui, 2, &T);
    assert_eq!(w.selected(), Some(2));
    assert_ne!(w.model.filter_mask & 0x08, 0);
    // A failed quest is listed in no mode: nothing changes.
    w.show_quest(&mut ui, 4, &T);
    assert_eq!((w.model.mode, w.selected()), (LogMode::Active, Some(2)));
}

#[test]
fn log_quests_are_child_nodes_of_their_category_in_a_tree() {
    use tessera_ui::ControlKind;
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    let tree = ui.create_control(ControlKind::TreeView, 1).unwrap();
    ui.control_set_parent(tree, Some(root)).unwrap();
    ui.control_set_size(tree, 321, 384);
    ui.control_set_visible(tree, true);
    w.tree = Some(tree);
    w.show(&mut ui, &example(), &T);
    // alpha, Cart, Beta, apples, Wolves.
    assert_eq!(w.rows.len(), 5);
    let beta = w.rows[2].0;
    let (apples, wolves) = (w.rows[3].0, w.rows[4].0);
    assert!(ui.is_visible(apples) && ui.is_visible(wolves));
    let ev = left(&ui, beta);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 5, "the tree hides the rows; no rebuild");
    assert!(!ui.is_visible(apples) && !ui.is_visible(wolves));
    assert!(ui.is_visible(w.rows[1].0));
    let ev = left(&ui, beta);
    w.handle_event(&mut ui, &ev, &T);
    assert!(ui.is_visible(apples));
    w.expand_all(&mut ui, false, &T);
    assert_eq!(w.rows.len(), 5);
    assert!(
        w.rows
            .iter()
            .filter(|(_, r)| matches!(r, LogRow::Quest(_)))
            .all(|(k, _)| !ui.is_visible(*k))
    );
}
