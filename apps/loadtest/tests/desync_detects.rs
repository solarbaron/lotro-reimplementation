// SPDX-License-Identifier: GPL-3.0-or-later
//! The desync checks can fail: with every bot's recorded truth shifted 5 m east (the
//! `truth_bias_m` test hook), the observers' correct views disagree with the truth and the
//! run must report Position episodes and fail. Without this, a regression that silences a
//! check would pass every other test, which all expect zero episodes.

use std::time::Duration;

use tessera_loadtest::desync::Kind;
use tessera_loadtest::runner::{RunOptions, Scenario, run};

#[test]
fn a_wrong_truth_is_reported() {
    let mut o = RunOptions::new(Scenario::Walk, 2);
    o.duration = Duration::from_secs(8);
    o.area_blocks = 1;
    o.behaviour.truth_bias_m = 5.0;
    o.sample_interval = Duration::from_secs(1);
    o.warmup_min = Duration::from_secs(2);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    let d = &r.inputs.desync;
    assert!(d.count(Kind::Position) >= 2, "{:?}", d.episodes());
    let e = d
        .episodes()
        .iter()
        .find(|e| e.kind == Kind::Position)
        .expect("listed");
    assert!(e.size > 4.0 && e.size < 6.5, "{e:?}");
    assert!(!r.passed());
    // The JSON lists the episodes.
    let json = r.json();
    assert!(json.contains("\"kind\": \"position\""), "{json}");
}
