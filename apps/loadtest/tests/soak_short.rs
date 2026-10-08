// SPDX-License-Identifier: GPL-3.0-or-later
//! A short soak (about 30 s) with 8 bots: walking and running across block borders,
//! teleports near and far (far ones leave interest range in the default 8 × 8-block area),
//! appearance and stance changes, chat on every channel and reconnections, with zero
//! desync, bounded counts and a return to the baseline. Timing is indicative.

use std::time::Duration;

use tessera_loadtest::runner::{RunOptions, Scenario, run};

#[test]
fn short_soak_has_no_desync() {
    let mut o = RunOptions::new(Scenario::Soak, 8);
    o.duration = Duration::from_secs(30);
    o.behaviour.chat_interval = Some(Duration::from_secs(4));
    o.behaviour.teleport_interval = Some(Duration::from_secs(9));
    o.behaviour.reconnect_interval = Some(Duration::from_secs(10));
    o.behaviour.reconnect_fraction = 0.25;
    o.behaviour.appearance_interval = Some(Duration::from_secs(7));
    o.sample_interval = Duration::from_secs(1);
    o.warmup_min = Duration::from_secs(3);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    let t = &r.totals;
    assert!(t.world_entries >= 8);
    assert!(t.reconnects >= 2, "reconnects {}", t.reconnects);
    assert!(t.teleports >= 8, "teleports {}", t.teleports);
    assert!(t.appearance_changes >= 8);
    assert!(t.stance_changes >= 8);
    // The leave side of interest management was exercised (regression: the old 4 × 4
    // default area never took a pair out of range).
    assert!(t.far_teleports >= 1, "far teleports {}", t.far_teleports);
    assert_eq!(t.far_teleports_short, 0);
    assert!(
        r.inputs.desync.pairs_left() > 0,
        "no pair left interest range"
    );
    assert!(
        r.coverage.iter().all(|c| !c.contains("leave")),
        "{:?}",
        r.coverage
    );
    // 10 Hz sampling (regression: the old schedule drifted to about 7.5 Hz).
    let hz = r.sampling_hz.expect("sampled");
    assert!(hz > 9.5, "{hz} Hz");
    assert!(r.inputs.ledger.counts.delivered > 0);
    assert_eq!(
        r.inputs.desync.total(),
        0,
        "{:?}",
        r.inputs.desync.episodes()
    );
    let b = r.inputs.baseline.as_ref().expect("in-process baseline");
    assert!(b.differences.is_empty(), "{:?}", b.differences);
    assert_eq!(
        r.inputs.leak.violation_count, 0,
        "{:?}",
        r.inputs.leak.violations
    );
    assert!(r.passed(), "{:?}", r.failures);
}
