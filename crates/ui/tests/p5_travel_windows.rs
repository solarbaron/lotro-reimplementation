// SPDX-License-Identifier: GPL-3.0-or-later
//! Rules of `docs/specs/travel-windows.md` on the fallback windows: grouping and search
//! (§4.5, §4.6, §11.1, TI1), marks, the selection display and the go button (§4.4,
//! §4.7, §4.8, §4.10, §11.2, TI2–TI4), commands (§4.9, §4.10 rule 6), the travel
//! presentation (§5.2, §11.3), the finder tree (§6.3, §11.4, TI6), menus, level and join
//! (§6.4, §6.8), the queue page and alert (§6.5, §7.2), the instance-travel window
//! (§4.11). All values are invented.

mod p5_common;

use p5_common::*;
use tessera_ui::Ui;
use tessera_ui::game::finder::{
    DefaultFinderTexts, FinderAction, FinderEntry, FinderModel, FinderTemplates, FinderTree,
    FinderView, FinderWindow, InstanceTravelWindow, QueueAlert, kind_of,
};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::travel::{
    CostShown, DefaultTravelTexts, EndTravelBox, RouteStatus, TravelAction, TravelLabel,
    TravelModel, TravelPresentation, TravelRow, TravelShow, TravelTemplates, TravelView,
    TravelWindow,
};
use tessera_ui::game::travelfallback;

const TT: DefaultTravelTexts = DefaultTravelTexts;
const FT: DefaultFinderTexts = DefaultFinderTexts;
const VALUES: [u64; 3] = [100_000, 100, 1];

fn row(entry: u16, name: &str, territory: &str, area: &str) -> TravelRow {
    TravelRow {
        entry,
        name: name.into(),
        territory: territory.into(),
        area: area.into(),
        discovered: true,
        met: true,
        usable: true,
        ..TravelRow::default()
    }
}

/// §11.1: D1 (A1 in T1), D2 (A2 in T2), D3 (A3 in T1), D4 (no area).
fn example_rows() -> Vec<TravelRow> {
    vec![
        row(1, "Dale", "Tarn", "Ashwood"),
        row(2, "Eastmarch", "Vale", "Brook"),
        row(3, "Fennick", "Tarn", "Copperhill"),
        row(4, "Gorse", "", ""),
    ]
}

#[test]
fn example_11_1_grouping_and_search() {
    let mut m = TravelModel::default();
    m.show(&TravelView {
        rows: example_rows(),
        ..TravelView::default()
    });
    let g = m.groups("Swift");
    let shape: Vec<(Option<&str>, Vec<usize>)> = g
        .iter()
        .map(|g| (g.territory.as_deref(), g.rows.clone()))
        .collect();
    assert_eq!(
        shape,
        vec![
            (None, vec![3]),
            (Some("Tarn"), vec![0, 2]),
            (Some("Vale"), vec![1])
        ]
    );
    // Only D3's area contains "copper" (case folded): D1, D2, D4 and header Vale go.
    m.filter = "COPPER".into();
    let g = m.groups("Swift");
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].territory.as_deref(), Some("Tarn"));
    assert_eq!(g[0].rows, vec![2]);
    // The swift label's text is part of a swift row's search text.
    let mut m = TravelModel::default();
    let mut r = row(9, "Hollow", "", "");
    r.swift = true;
    m.show(&TravelView {
        rows: vec![r],
        ..TravelView::default()
    });
    m.filter = "swift".into();
    assert_eq!(m.groups("Swift travel").len(), 1);
    assert!(m.groups("").is_empty());
}

#[test]
fn example_11_2_free_and_level() {
    let mut d1 = row(1, "Dale", "Tarn", "Ashwood");
    d1.min_level = 1;
    let mut d2 = row(2, "Eastmarch", "Vale", "Brook");
    d2.coin = 2500;
    d2.coins = coin_counts(2500, &VALUES);
    d2.min_level = 40;
    let mut m = TravelModel::default();
    m.show(&TravelView {
        player_level: 35,
        rows: vec![d1, d2],
        ..TravelView::default()
    });
    // TI2, TI3: nothing without a selection.
    let s = m.selection();
    assert_eq!(s.cost, CostShown::Nothing);
    assert_eq!(s.status, RouteStatus::NoRouteSelected);
    assert!(m.go_button().ghosted);
    m.selected = Some(0);
    let s = m.selection();
    assert_eq!(s.cost, CostShown::Free);
    assert_eq!(s.min_level, Some(1));
    assert!(!m.marks(0).failed);
    m.selected = Some(1);
    match m.selection().cost {
        CostShown::Coin(c) => assert_eq!((c.gold, c.silver, c.copper), (0, 25, 0)),
        other => panic!("{other:?}"),
    }
    assert!(m.marks(1).failed, "level 35 below 40");
    // The go button follows the flags only.
    assert!(!m.go_button().ghosted);
    assert_eq!(
        m.go(7),
        Some(TravelAction::Choose {
            window: 7,
            entry: 2
        })
    );
}

#[test]
fn go_button_and_marks_follow_the_flags() {
    let mut undiscovered = row(1, "A", "", "");
    undiscovered.discovered = false;
    let mut unusable = row(2, "B", "", "");
    unusable.usable = false;
    let mut unmet = row(3, "C", "", "");
    unmet.met = false;
    let mut m = TravelModel::default();
    m.show(&TravelView {
        rows: vec![undiscovered, unusable, unmet],
        ..TravelView::default()
    });
    assert!(m.marks(0).unknown && !m.marks(0).ghosted);
    assert!(m.marks(1).ghosted);
    assert!(m.marks(2).failed);
    m.selected = Some(0);
    let g = m.go_button();
    assert!(g.ghosted);
    assert_eq!(g.label, TravelLabel::GoUndiscovered);
    assert_eq!(m.selection().status, RouteStatus::Undiscovered);
    assert_eq!(m.go(1), None);
    m.selected = Some(1);
    assert_eq!(m.go_button().label, TravelLabel::GoNoSelection);
    assert_eq!(m.go(1), None);
    m.selected = Some(2);
    assert_eq!(m.go_button().label, TravelLabel::GoUsable);
    // Re-selection after a rebuild keeps a usable row's selection (§4.4 rule 7).
    let rows = m.rows.clone();
    m.show(&TravelView {
        rows,
        ..TravelView::default()
    });
    assert_eq!(m.selected, Some(2));
}

fn open_travel(ui: &mut Ui) -> TravelWindow {
    let (_, pv, nv) = vocabs();
    let a = art();
    let root = travelfallback::travel(ui, &nv, &pv, &a, &TT).unwrap();
    TravelWindow::attach(ui, &nv, &pv, root, TravelTemplates::default(), &a)
}

#[test]
fn travel_window_commands() {
    let mut ui = ui(800, 800);
    let (_, pv, _) = vocabs();
    let mut w = open_travel(&mut ui);
    w.show(
        &mut ui,
        &TravelView {
            window: 5,
            title: "Stable-master".into(),
            player_level: 50,
            rows: example_rows(),
        },
        &TT,
    );
    // Rows and headers: D4 at the top, then Tarn (D1, D3), Vale (D2).
    assert_eq!(w.rows.len(), 4);
    assert_eq!(w.headers.len(), 2);
    let nd = part(&ui, w.root, pv.ids.travel_no_destination);
    assert!(!ui.is_visible(nd));
    // No selection: go ghosted, no-route label shown; click does nothing.
    let go = w.parts.go.unwrap();
    let e = button(&ui, go);
    assert_eq!(w.handle_event(&mut ui, &e, &TT), None);
    assert!(ui.is_visible(w.parts.no_route_label.unwrap()));
    // Select D3 (its element is the third row created: D4, D1, D3).
    let (k, i) = w.rows[2];
    assert_eq!(i, 2);
    let e = left(&ui, k);
    assert_eq!(w.handle_event(&mut ui, &e, &TT), None);
    assert_eq!(w.model.selected, Some(2));
    assert!(ui.is_visible(w.parts.free.unwrap()), "cost 0: free");
    assert!(!ui.is_visible(w.parts.cost.unwrap()));
    assert!(ui.is_visible(w.parts.territory_field.unwrap()));
    assert_eq!(ui.text_of(w.parts.area.unwrap()), "Copperhill");
    let e = button(&ui, go);
    assert_eq!(
        w.handle_event(&mut ui, &e, &TT),
        Some(TravelAction::Choose {
            window: 5,
            entry: 3
        })
    );
    // Double-click on a row: select and go.
    let (k, _) = w.rows[1];
    let e = double(&ui, k);
    assert_eq!(
        w.handle_event(&mut ui, &e, &TT),
        Some(TravelAction::Choose {
            window: 5,
            entry: 1
        })
    );
    // The window does not close itself; close asks the client.
    assert!(ui.is_visible(w.root));
    let e = button(&ui, w.parts.close.unwrap());
    assert_eq!(
        w.handle_event(&mut ui, &e, &TT),
        Some(TravelAction::Close { window: 5 })
    );
    // Search through the field.
    w.set_filter(&mut ui, "brook", &TT);
    assert_eq!(w.rows.len(), 1);
    assert_eq!(w.headers.len(), 1);
    // An empty list shows the no-destination field.
    w.show(
        &mut ui,
        &TravelView {
            window: 6,
            ..TravelView::default()
        },
        &TT,
    );
    assert!(ui.is_visible(nd));
    // Paper-item and store parts are hidden (§4.8 Tessera rule).
    assert!(!ui.is_visible(part(&ui, w.root, pv.ids.travel_paper_go)));
    w.detach(&mut ui);
}

#[test]
fn example_11_3_travel_presentation() {
    let swift = TravelPresentation::start(10.0, true, 4000);
    assert_eq!(swift.shown(10.0), TravelShow::LoadingScreen);
    assert_eq!(swift.meter(10.0), 0.0);
    assert_eq!(swift.meter(12.0), 0.5);
    assert_eq!(swift.shown(14.0), TravelShow::Nothing);
    let mut t = swift;
    t.end();
    assert_eq!(t.shown(13.0), TravelShow::Nothing);
    let slow = TravelPresentation::start(0.0, false, 10_000);
    assert_eq!(slow.shown(9.9), TravelShow::EndTravelBox);
    assert_eq!(slow.shown(10.0), TravelShow::Nothing);
    assert_eq!(
        TravelPresentation::start(0.0, true, 0).shown(0.0),
        TravelShow::Nothing
    );
    // The end-travel button sends Dismount.
    let mut ui = ui(400, 300);
    let (_, pv, nv) = vocabs();
    let root = travelfallback::end_travel(&mut ui, &nv, &pv, &art(), &TT).unwrap();
    let b = EndTravelBox::attach(&mut ui, &pv, root, &TT);
    assert!(!ui.is_visible(b.root));
    b.set_shown(&mut ui, true);
    let e = button(&ui, b.button.unwrap());
    assert_eq!(b.handle_event(&ui, &e), Some(TravelAction::Dismount));
    // One real click sends one Dismount.
    let acts = real(&mut ui, b.button.unwrap(), |ui, e| b.handle_event(ui, e));
    assert_eq!(acts, vec![TravelAction::Dismount]);
}

// ----- the finder (§6) -----------------------------------------------------------------

fn entry(entry: u16, name: &str, kind: u32, cat: u32) -> FinderEntry {
    FinderEntry {
        entry,
        template: 0x70F0_0000 + u32::from(entry),
        name: name.into(),
        kind,
        kind_label: format!("Kind {kind}"),
        category: cat,
        category_label: if cat == 0 {
            "Other".into()
        } else {
            format!("Cat {cat}")
        },
        ..FinderEntry::default()
    }
}

/// §11.4: P1 (Instance, cat 13), P2 (Instance, cat 0), P3 (Skirmish, cat 4), P4 and P5 in
/// group 2 with size arrays (12) and (24).
fn example_entries() -> Vec<FinderEntry> {
    let mut p4 = entry(4, "P4", 1, 4);
    p4.group = Some((2, "Group two".into()));
    p4.sizes = vec![(12, "12".into())];
    p4.tiers = vec![(1, "T1".into())];
    let mut p5 = entry(5, "P5", 1, 4);
    p5.group = Some((2, "Group two".into()));
    p5.sizes = vec![(24, "24".into())];
    p5.tiers = vec![(1, "T1".into()), (2, "T2".into())];
    p5.level_range = Some((20, 60));
    vec![
        entry(1, "P1", 2, 13),
        entry(2, "P2", 2, 0),
        entry(3, "P3", 1, 4),
        p4,
        p5,
    ]
}

#[test]
fn example_11_4_finder_tree() {
    assert_eq!(kind_of(0), 8);
    assert_eq!(kind_of(0x1 | 0x40), 1);
    assert_eq!(kind_of(0x10_0000), 0x10_0000);
    let t = FinderTree::build(&example_entries());
    assert_eq!(t.kinds, vec![(2, "Kind 2".into()), (1, "Kind 1".into())]);
    assert_eq!(
        t.categories,
        vec![
            (0, 13, "Cat 13".into()),
            (0, 0, "Other".into()),
            (1, 4, "Cat 4".into())
        ]
    );
    let rows: Vec<(&str, Vec<usize>, usize)> = t
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.entries.clone(), r.category))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("P1", vec![0], 0),
            ("P2", vec![1], 1),
            ("P3", vec![2], 2),
            ("Group two", vec![3, 4], 2)
        ]
    );
    // Choosing size 24 on the group row resolves P5.
    let mut m = FinderModel::default();
    m.show(&FinderView {
        window: 3,
        entries: example_entries(),
        featured: None,
        player_level: 75,
    });
    m.select(Some(3));
    assert_eq!(
        m.sizes(3),
        vec![(12, "12".to_string()), (24, "24".to_string())]
    );
    assert_eq!(m.resolved(), Some(3));
    m.size = 1;
    assert_eq!(m.resolved(), Some(4));
    assert_eq!(m.tiers().len(), 2);
    // Level: clamped into P5's range (player 75 → 60), editable.
    assert_eq!(m.level_field(), Some((60, true)));
    m.level = Some(60);
    m.tier = 1;
    assert_eq!(
        m.join(3),
        Some(FinderAction::Join {
            window: 3,
            entry: 5,
            size: Some(24),
            tier: Some(2),
            level: Some(60)
        })
    );
    m.in_instance = true;
    assert_eq!(m.join(3), None);
}

fn open_finder(ui: &mut Ui) -> FinderWindow {
    let (_, pv, nv) = vocabs();
    let a = art();
    let root = travelfallback::finder(ui, &nv, &pv, &a, &FT).unwrap();
    FinderWindow::attach(ui, &nv, &pv, root, FinderTemplates::default(), &a, &FT)
}

#[test]
fn finder_window_selection_menus_and_join() {
    let mut ui = ui(1000, 700);
    let (_, pv, _) = vocabs();
    let mut w = open_finder(&mut ui);
    let mut entries = example_entries();
    entries[0].min_level = 10;
    entries[2].locked = true;
    w.show(
        &mut ui,
        &FinderView {
            window: 9,
            entries,
            featured: Some(1),
            player_level: 30,
        },
        &FT,
    );
    assert_eq!(w.rows.len(), 4);
    assert_eq!(w.headers.len(), 5, "two kind and three category headers");
    // The board is unavailable: overlay shown, buttons ghosted.
    let ov = part(&ui, w.root, pv.ids.board_overlay);
    assert!(ui.is_visible(ov));
    // No selection: join ghosted.
    let join = w.parts.join.unwrap();
    let e = button(&ui, join);
    assert_eq!(w.handle_event(&mut ui, &e, &FT), None);
    // The featured field selects P1.
    let feat = w.parts.featured.unwrap();
    let e = left(&ui, feat);
    w.handle_event(&mut ui, &e, &FT);
    assert_eq!(w.model.selected, Some(0));
    let e = button(&ui, join);
    assert_eq!(
        w.handle_event(&mut ui, &e, &FT),
        Some(FinderAction::Join {
            window: 9,
            entry: 1,
            size: None,
            tier: None,
            level: Some(30)
        })
    );
    // The group row: size menu popup, choose 24.
    let (k, r) = w.rows[3];
    assert_eq!(r, 3);
    let e = left(&ui, k);
    w.handle_event(&mut ui, &e, &FT);
    let sm = w.parts.size_menu.unwrap();
    let e = button(&ui, sm);
    w.handle_event(&mut ui, &e, &FT);
    assert!(w.popup_root().is_some(), "size menu");
    let item = w.menu().expect("size menu").item_elements(0)[1];
    let e = left(&ui, item);
    w.handle_event(&mut ui, &e, &FT);
    assert!(w.popup_root().is_none());
    assert_eq!(w.model.resolved(), Some(4));
    let e = double(&ui, k);
    let act = w.handle_event(&mut ui, &e, &FT);
    assert!(matches!(act, Some(FinderAction::Join { entry: 4 | 5, .. })));
    // In an instance: join ghosted.
    w.set_in_instance(&mut ui, true, &FT);
    let e = button(&ui, join);
    assert_eq!(w.handle_event(&mut ui, &e, &FT), None);
    // Search by row text.
    w.set_filter(&mut ui, "p3", &FT);
    assert_eq!(w.rows.len(), 1);
    assert_eq!(w.headers.len(), 2);
    w.detach(&mut ui);
}

#[test]
fn queue_page_and_alert() {
    let mut ui = ui(1000, 700);
    let (_, pv, _) = vocabs();
    let mut w = open_finder(&mut ui);
    let launch = w.parts.launch_page.unwrap();
    let confirm = w.parts.confirm_page.unwrap();
    assert!(ui.is_visible(launch) && !ui.is_visible(confirm));
    w.set_queue(&mut ui, 2, Some(65), 100.0, &FT);
    assert!(ui.is_visible(confirm) && !ui.is_visible(launch));
    let waiting = w.parts.waiting.unwrap();
    assert!(ui.is_visible(waiting));
    assert_eq!(ui.text_of(w.parts.waiting_timer.unwrap()), "1:05");
    w.tick(&mut ui, 110.0, &FT);
    assert_eq!(ui.text_of(w.parts.waiting_timer.unwrap()), "1:15");
    // State 1: the confirm timer counts down.
    w.set_queue(&mut ui, 1, Some(30), 0.0, &FT);
    assert!(!ui.is_visible(waiting));
    w.tick(&mut ui, 12.0, &FT);
    let ct = w.parts.confirm_timer.unwrap();
    assert!(ui.is_visible(ct));
    let leave = w.parts.leave_queue.unwrap();
    let e = button(&ui, leave);
    assert_eq!(
        w.handle_event(&mut ui, &e, &FT),
        Some(FinderAction::LeaveQueue)
    );
    w.set_queue(&mut ui, 0, None, 0.0, &FT);
    assert!(ui.is_visible(launch));
    // The alert.
    let ar = travelfallback::queue_alert(&mut ui, &pv, &art()).unwrap();
    let a = QueueAlert::attach(&mut ui, &pv, ar);
    assert!(!ui.is_visible(a.root));
    a.set_state(&mut ui, 7, &FT);
    assert!(ui.is_visible(a.root));
    let e = left(&ui, a.root);
    assert_eq!(a.handle_event(&ui, &e), Some(FinderAction::OpenFinder));
    let acts = real(&mut ui, a.root, |ui, e| a.handle_event(ui, e));
    assert_eq!(acts, vec![FinderAction::OpenFinder]);
    a.set_state(&mut ui, 0, &FT);
    assert!(!ui.is_visible(a.root));
}

#[test]
fn instance_travel_window() {
    let mut ui = ui(600, 600);
    let (_, pv, nv) = vocabs();
    let root = travelfallback::instance_travel(&mut ui, &nv, &pv, &art(), &FT).unwrap();
    let mut w = InstanceTravelWindow::attach(&mut ui, &pv, root, &FT);
    w.show(&mut ui, 4, "The Barrow", "A cold wind.");
    assert_eq!(ui.text_of(part(&ui, root, pv.ids.pe_title)), "The Barrow");
    let now = part(&ui, root, pv.ids.pe_now);
    let e = button(&ui, now);
    assert_eq!(
        w.handle_event(&ui, &e),
        Some(FinderAction::Join {
            window: 4,
            entry: 1,
            size: None,
            tier: None,
            level: None
        })
    );
    let later = part(&ui, root, pv.ids.pe_later);
    let e = button(&ui, later);
    assert_eq!(
        w.handle_event(&ui, &e),
        Some(FinderAction::Close { window: 4 })
    );
}

#[test]
fn hostile_views_do_not_panic() {
    let mut ui = ui(800, 800);
    let mut w = open_travel(&mut ui);
    let rows: Vec<TravelRow> = (0..2000u32)
        .map(|i| {
            let mut r = row(i as u16, &"x".repeat(20_000), &format!("t{}", i % 7), "");
            r.min_level = u16::MAX;
            r.coin = u64::MAX;
            r
        })
        .collect();
    w.show(
        &mut ui,
        &TravelView {
            window: 1,
            title: "\u{0}".repeat(10_000),
            player_level: u32::MAX,
            rows,
        },
        &TT,
    );
    assert_eq!(w.model.rows.len(), 1024);
    w.select(&mut ui, Some(5000), &TT);
    let mut f = open_finder(&mut ui);
    let mut entries = example_entries();
    entries[4].level_range = Some((90, 10));
    entries[4].sizes = (0..500).map(|i| (i, i.to_string())).collect();
    f.show(
        &mut ui,
        &FinderView {
            window: 2,
            entries,
            featured: Some(999),
            player_level: 0,
        },
        &FT,
    );
    f.select(&mut ui, Some(3), &FT);
    f.choose_size(&mut ui, 400, &FT);
    f.set_level(&mut ui, u16::MAX, &FT);
    f.set_queue(&mut ui, 200, Some(u32::MAX), f64::NAN, &FT);
    f.tick(&mut ui, f64::INFINITY, &FT);
    settle(&mut ui);
}

// ----- review fixes: one action per real click, selection per window, search, hostile
// model changes, locked template rows -----------------------------------------------------

/// Hands every event of a real pointer click on `k` to `f`; returns the actions.
fn real<A>(
    ui: &mut Ui,
    k: tessera_ui::ElementKey,
    mut f: impl FnMut(&mut Ui, &tessera_ui::UiEvent) -> Option<A>,
) -> Vec<A> {
    let evs = real_click(ui, k);
    evs.iter().filter_map(|e| f(ui, e)).collect()
}

#[test]
fn travel_real_clicks_act_once() {
    let mut ui = ui(800, 800);
    let mut w = open_travel(&mut ui);
    let view = TravelView {
        window: 7,
        title: "Stable-master".into(),
        player_level: 50,
        rows: example_rows(),
    };
    w.show(&mut ui, &view, &TT);
    let (k, i) = w.rows[2];
    let acts = real(&mut ui, k, |ui, e| w.handle_event(ui, e, &TT));
    assert!(acts.is_empty());
    assert_eq!(w.model.selected, Some(i));
    let go = w.parts.go.unwrap();
    let acts = real(&mut ui, go, |ui, e| w.handle_event(ui, e, &TT));
    assert_eq!(
        acts,
        vec![TravelAction::Choose {
            window: 7,
            entry: 3
        }]
    );
    let close = w.parts.close.unwrap();
    let acts = real(&mut ui, close, |ui, e| w.handle_event(ui, e, &TT));
    assert_eq!(acts, vec![TravelAction::Close { window: 7 }]);
}

#[test]
fn travel_selection_is_not_carried_to_another_window() {
    let mut ui = ui(800, 800);
    let mut w = open_travel(&mut ui);
    let view = |window| TravelView {
        window,
        title: "Stable-master".into(),
        player_level: 50,
        rows: example_rows(),
    };
    w.show(&mut ui, &view(7), &TT);
    w.select(&mut ui, Some(2), &TT);
    // A rebuild of the same window keeps it (§4.4 rule 7).
    w.show(&mut ui, &view(7), &TT);
    assert_eq!(w.model.selected, Some(2));
    // Another feed window starts without a selection.
    w.show(&mut ui, &view(8), &TT);
    assert_eq!(w.model.selected, None);
    assert_eq!(w.model.go(8), None);
    // Closing clears the window and the selection (§4.9).
    w.select(&mut ui, Some(2), &TT);
    w.close(&mut ui, &TT);
    assert_eq!(w.model.selected, None);
    assert_eq!(w.window, 0);
}

#[test]
fn finder_real_clicks_act_once() {
    let mut ui = ui(1000, 700);
    let mut w = open_finder(&mut ui);
    w.show(
        &mut ui,
        &FinderView {
            window: 9,
            entries: example_entries(),
            featured: None,
            player_level: 30,
        },
        &FT,
    );
    let (k, _) = w.rows[3];
    real(&mut ui, k, |ui, e| w.handle_event(ui, e, &FT));
    assert_eq!(w.model.selected, Some(3));
    // The size menu stays open after one click, and closes after a second.
    let sm = w.parts.size_menu.unwrap();
    real(&mut ui, sm, |ui, e| w.handle_event(ui, e, &FT));
    assert!(w.popup_root().is_some(), "size menu open");
    real(&mut ui, sm, |ui, e| w.handle_event(ui, e, &FT));
    assert!(w.popup_root().is_none());
    let join = w.parts.join.unwrap();
    let acts = real(&mut ui, join, |ui, e| w.handle_event(ui, e, &FT));
    assert_eq!(acts.len(), 1, "{acts:?}");
    assert!(matches!(acts[0], FinderAction::Join { window: 9, .. }));
    w.set_queue(&mut ui, 2, Some(65), 100.0, &FT);
    let leave = w.parts.leave_queue.unwrap();
    let acts = real(&mut ui, leave, |ui, e| w.handle_event(ui, e, &FT));
    assert_eq!(acts, vec![FinderAction::LeaveQueue]);
}

#[test]
fn finder_search_covers_header_texts() {
    let mut m = FinderModel::default();
    m.show(&FinderView {
        window: 1,
        entries: example_entries(),
        featured: None,
        player_level: 30,
    });
    let shown = |m: &FinderModel| {
        (0..m.tree.rows.len())
            .filter(|r| m.matches(*r))
            .map(|r| m.tree.rows[r].text.clone())
            .collect::<Vec<_>>()
    };
    m.filter = "kind 1".into();
    assert_eq!(shown(&m), vec!["P3", "Group two"], "kind header text");
    m.filter = "CAT 13".into();
    assert_eq!(shown(&m), vec!["P1"], "category header text, case folded");
    m.filter = "p2".into();
    assert_eq!(shown(&m), vec!["P2"], "row text");
}

#[test]
fn finder_model_replaced_directly_does_not_panic() {
    let mut ui = ui(1000, 700);
    let mut w = open_finder(&mut ui);
    let mut entries = example_entries();
    entries.extend((10..20).map(|i| entry(i, &format!("E{i}"), 3, 1)));
    w.show(
        &mut ui,
        &FinderView {
            window: 9,
            entries,
            featured: None,
            player_level: 30,
        },
        &FT,
    );
    w.select(&mut ui, Some(10), &FT);
    // The public model is replaced behind the window's back.
    w.model.show(&FinderView {
        window: 9,
        entries: vec![entry(1, "P1", 2, 13)],
        featured: Some(1),
        player_level: 30,
    });
    w.set_in_instance(&mut ui, true, &FT);
    // Entries shortened after the tree was built.
    w.model.select(Some(0));
    w.model.entries.clear();
    assert_eq!(w.model.join(9), None);
    assert!(w.model.tiers().is_empty());
    assert_eq!(w.model.level_field(), None);
    w.model.select(Some(0));
    w.set_in_instance(&mut ui, false, &FT);
}

#[test]
fn locked_template_rows_keep_their_own_text_colour() {
    use p5_common::ui_common::{child, field, layout, typed};
    const ROWS: u32 = 0x22FF_6000;
    const ROW: u32 = 0x1000_7000;
    let (_, pv, nv) = vocabs();
    let text_type = tessera_ui::Vocab::default().types.text;
    let row = child(
        field(ROW, 0, 0, 400, 24),
        typed(pv.ids.encounter_text, text_type, 4, 2, 380, 20),
    );
    let mut ui = ui_layouts(1000, 700, vec![layout(ROWS, 400, 24, vec![row])]);
    let a = art();
    let root = travelfallback::finder(&mut ui, &nv, &pv, &a, &FT).unwrap();
    let templates = FinderTemplates {
        row: Some(tessera_ui::game::examine::ExamTemplate {
            layout: ROWS,
            element: ROW,
        }),
        ..FinderTemplates::default()
    };
    let mut w = FinderWindow::attach(&mut ui, &nv, &pv, root, templates, &a, &FT);
    let mut entries = example_entries();
    entries[2].locked = true;
    w.show(
        &mut ui,
        &FinderView {
            window: 9,
            entries,
            featured: None,
            player_level: 30,
        },
        &FT,
    );
    let colour = |ui: &Ui, r: usize| {
        let k = w.rows.iter().find(|(_, x)| *x == r).unwrap().0;
        let t = part(ui, k, pv.ids.encounter_text);
        ui.prop(t, nv.pids.text_font_color).value().cloned()
    };
    assert_eq!(w.model.tree.rows[2].text, "P3");
    assert!(w.model.tree.rows[2].locked);
    assert_eq!(colour(&ui, 2), colour(&ui, 0), "no red on a template row");
    // The fallback row draws a locked row's text red.
    let mut f = open_finder(&mut ui);
    let mut entries = example_entries();
    entries[2].locked = true;
    f.show(
        &mut ui,
        &FinderView {
            window: 9,
            entries,
            featured: None,
            player_level: 30,
        },
        &FT,
    );
    let fcolour = |ui: &Ui, r: usize| {
        let k = f.rows.iter().find(|(_, x)| *x == r).unwrap().0;
        let t = part(ui, k, pv.ids.encounter_text);
        ui.prop(t, nv.pids.text_font_color).value().cloned()
    };
    assert_ne!(fcolour(&ui, 2), fcolour(&ui, 0));
}

/// `travel-windows.md` §6.4 OPEN (Tessera rule): a join label whose text wraps grows to its
/// wrapped text inside its button, centred on its laid-out box; text taller than the button
/// is capped at the button's height; a text that fits gets the laid-out box back.
#[test]
fn join_label_grows_to_its_wrapped_text() {
    use tessera_ui::game::finder::{FinderLabel, FinderTexts};
    struct JoinText(&'static str);
    impl tessera_ui::game::npc::NpcTexts for JoinText {}
    impl FinderTexts for JoinText {
        fn finder_label(&self, l: FinderLabel) -> Option<String> {
            (l == FinderLabel::JoinNone).then(|| self.0.to_string())
        }
    }
    let mut ui = ui(1000, 700);
    let mut w = open_finder(&mut ui);
    let label = w.parts.join_label.unwrap();
    let bh = ui.layout_box(w.parts.join.unwrap()).unwrap().h();
    let laid = ui.layout_box(label).unwrap();
    let view = FinderView {
        window: 9,
        entries: example_entries(),
        featured: None,
        player_level: 30,
    };
    // Two lines.
    w.show(
        &mut ui,
        &view,
        &JoinText("pick one entry from the list of instances on the left first"),
    );
    let b = ui.layout_box(label).unwrap();
    assert_eq!((b.l, b.w()), (laid.l, laid.w()), "the width is kept");
    assert!(b.h() > laid.h() && b.h() <= bh, "{b:?} in {bh}");
    assert!(b.t >= 0 && b.t + b.h() <= bh, "inside the button: {b:?}");
    let centre = |b: tessera_ui::IBox| b.t * 2 + b.h();
    assert!(
        (centre(b) - centre(laid)).abs() <= 1,
        "centred: {b:?} {laid:?}"
    );
    // Taller than the button: capped.
    w.show(
        &mut ui,
        &view,
        &JoinText(concat!(
            "a label text so long that it needs many lines in a button this small, ",
            "longer than any label of the install would be, and then some more words"
        )),
    );
    let b = ui.layout_box(label).unwrap();
    assert_eq!((b.t, b.h()), (0, bh), "capped at the button's height");
    // Short: the laid-out box again.
    w.show(&mut ui, &view, &FT);
    assert_eq!(ui.layout_box(label).unwrap(), laid);
    // The frame tick keeps it.
    w.tick(&mut ui, 1.0, &FT);
    assert_eq!(ui.layout_box(label).unwrap(), laid);
    w.detach(&mut ui);
}
