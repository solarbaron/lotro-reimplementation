// SPDX-License-Identifier: GPL-3.0-or-later
//! A short hostile run (ROADMAP M7 exit criterion 3): random datagrams, malformed,
//! truncated and oversized frames, out-of-order handshakes and floods over real UDP, and
//! the protocol 1.2 cases in the world, while honest bots walk and chat. No panic anywhere, the honest bots see no desync and no
//! disconnection, the server keeps ticking, its counts stay bounded and return to the
//! baseline, and memory stays bounded.

use std::time::Duration;

use tessera_loadtest::runner::{HostileOptions, RunOptions, Scenario, run};

#[test]
fn hostile_clients_do_not_disturb_honest_bots() {
    let mut o = RunOptions::new(Scenario::Walk, 6);
    o.duration = Duration::from_secs(30);
    o.behaviour.chat_interval = Some(Duration::from_secs(4));
    o.hostile = Some(HostileOptions {
        clients: 6,
        pace: Duration::from_millis(100),
    });
    o.sample_interval = Duration::from_secs(1);
    // Half the run (the cap of `test-server.md` §13.3 item 3): the hostile clients keep
    // connections open for a while, so resident memory still ramps after 3 s.
    o.warmup_min = Duration::from_secs(15);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    let h = r.inputs.hostile.as_ref().expect("hostile clients ran");
    let attacks: u64 = h.attacks.values().sum();
    assert!(attacks >= 20, "{attacks} attacks");
    assert!(
        h.closed_by_server
            .get("PROTOCOL_VIOLATION")
            .copied()
            .unwrap_or(0)
            > 0,
        "the server closes offenders: {:?}",
        h.closed_by_server
    );
    // The 1.2 cases go in the world with the run's hostile characters (test-server.md
    // §13.1), and the server answers each as the spec says.
    assert!(h.v12_in_world > 0, "{h:?}");
    assert_eq!(h.v12_wrong, 0, "{:?}", h.v12_wrong_examples);
    assert_eq!(r.inputs.panics, 0, "no panic in the server or the bots");
    assert_eq!(r.totals.unexpected_disconnects, 0);
    assert_eq!(
        r.inputs.desync.total(),
        0,
        "{:?}",
        r.inputs.desync.episodes()
    );
    assert_eq!(r.inputs.leak.stalled, 0, "the simulation kept ticking");
    let (_, end) = r.inputs.stats.expect("server counters");
    assert!(end.violations > 0);
    // Accounted memory stays far below the 1 GiB budget.
    let peak = r
        .inputs
        .leak
        .samples
        .iter()
        .filter_map(|s| s.server.map(|v| v.accounted_bytes))
        .max()
        .unwrap_or(0);
    assert!(peak < 256 << 20, "accounted {peak}");
    assert!(r.passed(), "{:?}", r.failures);
}
