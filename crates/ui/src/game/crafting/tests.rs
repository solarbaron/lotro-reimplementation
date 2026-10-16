// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the crafting rules (`crafting-panels.md` §4.6, §5.5–§5.7, §15).

use super::rules::*;
use super::*;

#[test]
fn reading_the_entry() {
    assert_eq!(read("", 4), 1);
    assert_eq!(read("", 0), 0);
    assert_eq!(read("007", 4), 7);
    assert_eq!(read("0", 4), 0);
    assert_eq!(read("12a", 99), 12);
    assert_eq!(read("a12", 99), 0);
    assert_eq!(read("99999999999999999999999", 1), u64::MAX);
}

#[test]
fn quantity_steps_of_15_3() {
    // M = 4.
    assert_eq!(clamp(read("9", 4), 4), 4);
    assert_eq!(clamp(read("007", 4), 4), 4);
    assert_eq!(decrease(1, 4), 1);
    assert_eq!(decrease(3, 4), 2);
    assert_eq!(increase(4, 4), 4);
    assert_eq!(increase(9, 4), 4);
    assert_eq!(increase(2, 4), 3);
    assert_eq!(decrease(0, 4), 1);
    // M = 0: the entry shows 1.
    assert_eq!(shown("", 0), 1);
    assert_eq!(shown("5", 0), 5);
    assert_eq!(make_all(1200), 999);
    assert_eq!(make_all(0), 0);
}

#[test]
fn quantity_sent() {
    assert_eq!(to_send(3, 2), Some(2));
    assert_eq!(to_send(1, 0), Some(1));
    assert_eq!(to_send(0, 5), None);
    assert_eq!(to_send(5000, 0), Some(999));
}

#[test]
fn critical_chance_of_15_5() {
    let b = [0.25, 0.35];
    assert_eq!(critical_percent(0.07, &b, 0b01), 32);
    assert_eq!(critical_percent(0.07, &b, 0b11), 67);
    assert_eq!(critical_percent(0.07, &[1.0], 0b1), 100);
    assert_eq!(critical_percent(-1.0, &[], 0), 0);
    assert_eq!(critical_percent(f32::NAN, &[f32::INFINITY], 1), 0);
    assert_eq!(bonus_percent(0.25), 25);
    assert_eq!(round_half_away(-2.5), -3);
    assert_eq!(round_half_away(2.5), 3);
}

#[test]
fn search_matching() {
    let w = search_words("  Iron  TONGS ");
    assert_eq!(w, vec!["iron".to_string(), "tongs".to_string()]);
    assert!(passes("Iron Tongs (2)", &w));
    assert!(!passes("Iron Hammer", &w));
    assert!(passes("anything", &[]));
    assert!(passes("Ärmel", &search_words("ärm")));
    // §4.6 Tessera rule: the simple mapping (U+0130 → U+0069, one character).
    assert_eq!(fold("\u{130}ron"), "iron");
    assert!(passes("\u{130}ron Ingot", &search_words("iron")));
    assert_eq!(simple_lowercase('\u{130}'), 'i');
}

#[test]
fn increase_stops_at_the_entry_cap() {
    // With M = 1,200 the panel steps with M capped at 999 (§5.6, 3 characters).
    assert_eq!(increase(999, 1200u64.min(MAX_QUANTITY)), 999);
    assert_eq!(increase(998, 1200u64.min(MAX_QUANTITY)), 999);
}

#[test]
fn meter_position() {
    let m = MeterView {
        current: 60,
        cap: 800,
        ..MeterView::default()
    };
    assert!((m.position() - 0.075).abs() < 1e-6);
    let z = MeterView {
        current: 5,
        cap: 0,
        ..MeterView::default()
    };
    assert_eq!(z.position(), 0.0);
    let o = MeterView {
        current: 900,
        cap: 800,
        ..MeterView::default()
    };
    assert_eq!(o.position(), 1.0);
}

#[test]
fn make_test_of_5_7() {
    let ing = |held, needed| IngredientView {
        held,
        needed,
        ..IngredientView::default()
    };
    let mut p = RecipePageView {
        tool: RequirementView {
            met: true,
            ..RequirementView::default()
        },
        facility: RequirementView {
            met: true,
            ..RequirementView::default()
        },
        ingredients: vec![ing(4, 2)],
        ..RecipePageView::default()
    };
    assert!(make_enabled(&p, false, 0, false));
    assert!(!make_enabled(&p, false, 0, true));
    // The makeable count does not take part (a running cooldown, §5.7).
    p.makeable = 0;
    assert!(make_enabled(&p, false, 0, false));
    p.facility.met = false;
    assert!(!make_enabled(&p, false, 0, false));
    p.facility.met = true;
    p.pack = Some(PackView {
        slot: ing(0, 1),
        ..PackView::default()
    });
    assert!(!make_enabled(&p, true, 0, false));
    p.master = Some(MasterView {
        mastered: true,
        optional: vec![OptionalView {
            slot: ing(0, 1),
            ..OptionalView::default()
        }],
        ..MasterView::default()
    });
    assert!(!make_enabled(&p, false, 1, false));
    assert!(make_enabled(&p, false, 0, false));
    assert!(!optional_enabled(&p, 0));
    assert_eq!(limit(&p, false), 0);
}
