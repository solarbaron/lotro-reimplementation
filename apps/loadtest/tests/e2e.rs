// SPDX-License-Identifier: GPL-3.0-or-later
//! End to end: two bots on an in-process server see each other move (their interpolated
//! positions match the truth) and receive each other's chat.

use std::time::Duration;

use tessera_loadtest::desync::Kind;
use tessera_loadtest::runner::{RunOptions, Scenario, run};

#[test]
fn two_bots_see_each_other_move_and_chat() {
    let mut o = RunOptions::new(Scenario::Chat, 2);
    o.duration = Duration::from_secs(15);
    // One block: the two bots are always within each other's interest range.
    o.area_blocks = 1;
    o.behaviour.walk_fraction = 1.0;
    o.behaviour.chat_interval = Some(Duration::from_secs(3));
    o.sample_interval = Duration::from_secs(1);
    o.warmup_min = Duration::from_secs(3);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    let d = &r.inputs.desync;
    assert_eq!(r.totals.world_entries, 2);
    assert_eq!(d.pairs_compared(), 2, "each bot saw the other");
    assert!(d.samples > 100, "{} comparisons", d.samples);
    assert!(r.totals.traffic.motion_batches > 0);
    for k in Kind::ALL {
        assert_eq!(d.count(k), 0, "{} desync", k.name());
    }
    let c = &r.inputs.ledger.counts;
    assert!(c.lines >= 4, "{c:?}");
    // Yell and Tell lines reach the other bot; every line comes back as an echo.
    assert!(c.delivered > c.lines, "{c:?}");
    assert_eq!(
        c.lost + c.late + c.duplicated + c.out_of_order + c.unexpected,
        0
    );
    assert_eq!(r.inputs.panics, 0);
    assert!(r.passed(), "{:?}", r.failures);
}
