// SPDX-License-Identifier: GPL-3.0-or-later
//! The `cheat` scenario (`test-server.md` §13.1): cheating bots send 40 m jumps; every one
//! must be corrected back to where the bot was (or dropped), and no observer may show the
//! refused position. The in-process server validates movement, so the run passes.

use std::time::Duration;

use tessera_loadtest::desync::Kind;
use tessera_loadtest::runner::{RunOptions, Scenario, run};

#[test]
fn every_cheat_is_corrected_and_never_shown() {
    let mut o = RunOptions::new(Scenario::Cheat, 6);
    o.duration = Duration::from_secs(20);
    // Everyone within range of everyone, so observers watch every cheat.
    o.area_blocks = 1;
    o.behaviour.cheat_fraction = 0.5;
    o.behaviour.cheat_interval = Some(Duration::from_secs(3));
    o.sample_interval = Duration::from_secs(1);
    o.warmup_min = Duration::from_secs(3);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    let t = &r.totals;
    assert!(t.cheats >= 3, "cheats {}", t.cheats);
    assert_eq!(t.cheats_bad_correction, 0);
    assert_eq!(t.cheats_unanswered, 0);
    assert_eq!(t.cheats_corrected + t.cheats_abandoned, t.cheats, "{t:?}");
    let (a, b) = r.inputs.stats.expect("server counters");
    assert!(b.corrections - a.corrections >= t.cheats_corrected);
    assert_eq!(r.inputs.desync.count(Kind::Cheat), 0);
    assert_eq!(r.inputs.desync.count(Kind::Correction), 0);
    assert!(r.passed(), "{:?}", r.failures);
}
