// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the travel model (`travel-windows.md` §11 worked examples and the
//! rules of §4.4–§4.10, §5.2) over the in-memory install of the quest tests. All DIDs,
//! names and texts are invented.

use tessera_formats::{Did, PropertyValue};

use super::*;
use crate::quest::tests::{Mem, arr, literal, si};

const D1: Did = Did(0x70F0_0001);
const D2: Did = Did(0x70F0_0002);
const D3: Did = Did(0x70F0_0003);
const D4: Did = Did(0x70F0_0004);
const R1: Did = Did(0x70F0_0101);
const A1: Did = Did(0x70F0_0201);
const A2: Did = Did(0x70F0_0202);
const A3: Did = Did(0x70F0_0203);
const T1: Did = Did(0x70F0_0301);
const T2: Did = Did(0x70F0_0302);

fn install() -> Mem {
    let mut m = Mem::default();
    for (d, name, swift) in [
        (D1, "Low road", false),
        (D2, "High pass", true),
        (D3, "Ferry", false),
        (D4, "Camp", false),
    ] {
        let e = vec![
            m.e("TravelDisplayName", si(literal(name))),
            m.e("TravelDestinationIsSwiftTravel", PropertyValue::Bool(swift)),
        ];
        m.put(d, e);
    }
    let e = vec![
        m.e("TravelRoute_Destination", PropertyValue::DidRef(D2)),
        m.e("TravelRoute_ActionArray", arr(vec![])),
        m.e("Usage_MinLevel", PropertyValue::Int(30)),
    ];
    m.put(R1, e);
    for (t, name) in [(T1, "North"), (T2, "South")] {
        let e = vec![
            m.e("Area_Name", si(literal(name))),
            m.e("Area_Region", PropertyValue::DidRef(Did(0x70F0_0401))),
        ];
        m.put(t, e);
    }
    for (a, name, t) in [(A1, "Vale", T1), (A2, "Fen", T2), (A3, "Ridge", T1)] {
        let e = vec![
            m.e("Area_Name", si(literal(name))),
            m.e("Area_Territory", PropertyValue::DidRef(t)),
        ];
        m.put(a, e);
    }
    m.text(TRAVEL_TABLE, tokens::TITLE, "Routes of ${#00005343}");
    m.text(TRAVEL_TABLE, tokens::SWIFT, "swift");
    m.text(UI_TABLE, tokens::MIN_LEVEL, "Level ${#0050AA9C}");
    m.text(UI_TABLE, tokens::LEVEL_BELOW_TWO, "any");
    m
}

fn entry(n: u16, d: Did, area: Option<Did>) -> TravelEntryIn {
    TravelEntryIn {
        entry: n,
        destination: RecordRef::Record(d),
        coin: 0,
        min_level: None,
        flags: TravelFlags::default(),
        route: None,
        area: area.map(RecordRef::Record),
    }
}

#[test]
fn example_11_1_grouping_and_search() {
    let m = install();
    let tp = TravelPresenter::new(&m);
    let entries = [
        entry(1, D1, Some(A1)),
        entry(2, D2, Some(A2)),
        entry(3, D3, Some(A3)),
        entry(4, D4, None),
    ];
    let w = tp.window(Some("Groom"), &entries, 50);
    assert_eq!(w.title.as_deref(), Some("Routes of Groom"));
    let groups: Vec<(Option<&str>, Vec<usize>)> = w
        .groups
        .iter()
        .map(|g| (g.territory.as_deref(), g.rows.clone()))
        .collect();
    assert_eq!(
        groups,
        vec![
            (None, vec![3]),
            (Some("North"), vec![0, 2]),
            (Some("South"), vec![1])
        ],
        "D4 at the top level; headers in the order of their first row"
    );
    assert_eq!(w.rows[2].area.as_deref(), Some("Ridge"));
    assert_eq!(w.rows[2].territory.as_deref(), Some("North"));
    // Typing a filter only D3's area contains (case-insensitive, Tessera rule).
    let kept = w.filter("rIdGe");
    assert_eq!(kept, vec![false, false, true, false]);
    assert!(w.header_visible(1, &kept));
    assert!(!w.header_visible(2, &kept) && !w.header_visible(0, &kept));
    assert_eq!(w.filter(""), vec![true; 4]);
    // The search text: name, area, territory, then the swift label (§4.4 rule 2).
    assert_eq!(w.rows[0].search_text, "Low roadValeNorth");
    assert_eq!(w.rows[1].search_text, "High passFenSouthswift");
}

#[test]
fn dungeon_area_ref_names_the_area() {
    // An `area=` ref may be a dungeon (the `area` selector family includes them): its
    // `Dungeon_Name` is the area text; with no territory the row is at the top level.
    let mut m = install();
    let dungeon = Did(0x70F0_0501);
    let e = vec![m.e("Dungeon_Name", si(literal("Deep Hall")))];
    m.put(dungeon, e);
    let tp = TravelPresenter::new(&m);
    assert_eq!(
        tp.area_texts(Some(&RecordRef::Record(dungeon))),
        (None, Some("Deep Hall".to_owned()))
    );
    let w = tp.window(None, &[entry(1, D1, Some(dungeon))], 50);
    assert_eq!(w.rows[0].area.as_deref(), Some("Deep Hall"));
    assert_eq!(w.rows[0].territory, None);
}

#[test]
fn example_11_2_free_and_level() {
    let m = install();
    let tp = TravelPresenter::new(&m);
    let mut e1 = entry(1, D1, None);
    e1.min_level = Some(1);
    let mut e2 = entry(2, D2, None);
    e2.coin = 2_500;
    e2.min_level = Some(40);
    let w = tp.window(None, &[e1, e2], 35);
    let s1 = w.selection(Some(0), &tp);
    assert!(
        s1.free && s1.cost.is_none(),
        "TI2: free text, no money display"
    );
    assert_eq!(s1.min_level, Some(LevelText::BelowTwo));
    assert_eq!(s1.min_level_text.as_deref(), Some("Level any"));
    let s2 = w.selection(Some(1), &tp);
    assert_eq!((s2.free, s2.cost), (false, Some(2_500)));
    assert_eq!(s2.min_level_text.as_deref(), Some("Level 40"));
    assert!(
        w.rows[1].marks.failed_requirements,
        "level 40 above the player's 35"
    );
    assert!(s2.go.normal, "the go button follows the flags only");
    assert_eq!(s2.go.label, tokens::GO_USABLE);
    assert_eq!(s2.route_status, RouteStatus::Discovered);
    assert!(s2.swift_label && w.rows[1].marks.swift);
    // Without a source the title's variable renders empty.
    assert_eq!(w.title.as_deref().map(str::trim_end), Some("Routes of"));
}

#[test]
fn selection_states_and_flags() {
    let m = install();
    let tp = TravelPresenter::new(&m);
    let mut undiscovered = entry(1, D1, None);
    undiscovered.flags = TravelFlags(TravelFlags::NOT_DISCOVERED | TravelFlags::NOT_USABLE);
    let mut unusable = entry(2, D3, None);
    unusable.flags = TravelFlags(TravelFlags::NOT_USABLE | TravelFlags::NOT_MET);
    let w = tp.window(None, &[undiscovered, unusable], 10);
    let none = w.selection(None, &tp);
    assert_eq!(none.route_status, RouteStatus::NoRouteSelected, "TI3");
    assert!(!none.go.normal && none.go.no_selection_label);
    assert_eq!(none.go.label, tokens::GO_NO_SELECTION);
    assert!(
        !none.free && none.cost.is_none(),
        "TI2: neither without selection"
    );
    let a = w.selection(Some(0), &tp);
    assert_eq!(a.route_status, RouteStatus::Undiscovered);
    assert_eq!((a.go.normal, a.go.label), (false, tokens::GO_UNDISCOVERED));
    assert!(w.rows[0].marks.unknown && w.rows[0].marks.ghosted);
    let b = w.selection(Some(1), &tp);
    assert_eq!((b.go.normal, b.go.label), (false, tokens::GO_NO_SELECTION));
    assert!(w.rows[1].marks.failed_requirements, "requirements not met");
    assert_eq!(RouteStatus::Undiscovered.token(), tokens::ROUTE_STATUS[2]);
    assert_eq!(
        RouteStatus::Discovered.state_label(),
        "Travel_DestinationDiscovered"
    );
}

#[test]
fn routes_literals_and_route_modes() {
    let m = install();
    let tp = TravelPresenter::new(&m);
    let mut by_route = entry(1, R1, None);
    by_route.route = Some(RouteMode::Slow);
    let literal = TravelEntryIn {
        entry: 2,
        destination: RecordRef::Literal("Harbour".into()),
        coin: 5,
        min_level: None,
        flags: TravelFlags::default(),
        route: Some(RouteMode::Swift),
        area: Some(RecordRef::Literal("Coast".into())),
    };
    let unknown = entry(3, Did(0x70F0_0999), None);
    let w = tp.window(None, &[by_route, literal, unknown], 50);
    let r = &w.rows[0];
    assert_eq!(
        (r.source, r.destination, r.route),
        (RowSource::Route, Some(D2), Some(R1))
    );
    assert_eq!(r.name.as_deref(), Some("High pass"));
    assert_eq!(
        r.min_level, 30,
        "the route's Usage_MinLevel without a feed level"
    );
    assert!(!r.swift, "the feed's route mode wins over the record");
    let l = &w.rows[1];
    assert_eq!(l.source, RowSource::Literal);
    assert_eq!(
        (l.name.as_deref(), l.area.as_deref(), l.territory.as_deref()),
        (Some("Harbour"), Some("Coast"), None)
    );
    assert!(l.swift);
    assert_eq!(w.rows[2].source, RowSource::Unknown);
    assert!(w.rows[2].name.is_none());
    // Rows with no territory sit at the top level.
    assert_eq!(w.groups.len(), 1);
    assert!(w.groups[0].territory.is_none());
    assert!(!w.is_empty());
    assert!(tp.window(None, &[], 1).is_empty(), "rule 7");
}

#[test]
fn row_cap() {
    let m = install();
    let tp = TravelPresenter::new(&m);
    let entries: Vec<TravelEntryIn> = (0..2_000u16).map(|i| entry(i, D1, None)).collect();
    assert_eq!(tp.window(None, &entries, 1).rows.len(), MAX_ROWS);
}

#[test]
fn example_11_3_presentation() {
    assert_eq!(
        travel_presentation(Some(true), 4_000),
        TravelPresentation::LoadingScreen { duration_ms: 4_000 }
    );
    assert!((meter_value(0, 4_000) - 0.0).abs() < f32::EPSILON);
    assert!((meter_value(2_000, 4_000) - 0.5).abs() < f32::EPSILON);
    assert!((meter_value(9_000, 4_000) - 1.0).abs() < f32::EPSILON);
    assert_eq!(
        travel_presentation(Some(false), 10_000),
        TravelPresentation::EndTravelBox {
            duration_ms: 10_000
        }
    );
    assert_eq!(
        travel_presentation(None, 10_000),
        TravelPresentation::EndTravelBox {
            duration_ms: 10_000
        }
    );
    assert_eq!(
        travel_presentation(Some(true), 0),
        TravelPresentation::Nothing
    );
}

#[test]
fn flags_bits() {
    let f = TravelFlags(0b101);
    assert!(!f.discovered() && f.met() && !f.usable());
    assert!(TravelFlags::default().discovered());
}
