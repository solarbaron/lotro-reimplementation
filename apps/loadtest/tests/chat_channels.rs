// SPDX-License-Identifier: GPL-3.0-or-later
//! The `chat` scenario with named channels (`test-server.md` §13.1, protocol minor 2): bots
//! at minor 2 join one of the run's shared channels, speak on it, leave and re-join it
//! within the request bucket, while bots at minors 1 and 0 stay out; every channel line
//! reaches exactly the channel's members (§14 invariant 15), and the server's channel
//! counts are back at baseline after the run (§13.3).

use std::time::Duration;

use tessera_loadtest::raw::bot_minor;
use tessera_loadtest::runner::{RunOptions, Scenario, run};

#[test]
fn chat_bots_join_speak_on_and_leave_channels() {
    let bots = 10;
    let mut o = RunOptions::new(Scenario::Chat, bots);
    o.duration = Duration::from_secs(40);
    // One block: Say reaches everyone, so every line is judged.
    o.area_blocks = 1;
    o.behaviour.channels = 2;
    o.behaviour.channel_cycle = Some(Duration::from_secs(15));
    o.behaviour.channel_fraction = 0.5;
    o.sample_interval = Duration::from_secs(1);
    o.warmup_min = Duration::from_secs(5);
    let r = run(&o).expect("the run starts");
    print!("{}", r.text());
    // Bots 8 and 9 offer minors 1 and 0 and never join; the others negotiate 2.
    let joiners = (0..bots).filter(|&i| bot_minor(i) >= 2).count() as u64;
    assert_eq!(joiners, 8);
    let t = &r.totals;
    assert!(t.channel_joins >= joiners, "{} joins", t.channel_joins);
    assert!(t.channel_leaves >= 1, "members leave and re-join");
    assert_eq!(t.channel_refusals, 0, "{:?}", r.bot_failures);
    let c = &r.inputs.ledger.counts;
    assert!(c.channel_lines >= 4, "{c:?}");
    assert!(c.delivered > c.lines, "{c:?}");
    assert_eq!(
        c.lost + c.late + c.duplicated + c.out_of_order + c.unexpected,
        0,
        "{:?}",
        r.inputs.ledger.problems
    );
    let (_, end) = r.inputs.stats.expect("server counters");
    assert!(end.chat_delivered > 0);
    let base = r.inputs.baseline.as_ref().expect("baseline checked");
    assert!(base.differences.is_empty(), "{:?}", base.differences);
    assert_eq!(
        (base.after.channels, base.after.channel_memberships),
        (0, 0)
    );
    assert_eq!(r.inputs.panics, 0);
    assert!(r.passed(), "{:?}", r.failures);
}
