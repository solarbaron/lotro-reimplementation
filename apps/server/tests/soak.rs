// SPDX-License-Identifier: GPL-3.0-or-later
//! A short in-process soak (`test-server.md` §13.3 in miniature): bots walk, chat and
//! reconnect against one server; afterwards every count returns to its baseline and the
//! accounted memory stays bounded. Ignored by default (it takes `TESSERA_SOAK_SECS`, default
//! 20 s); run with `cargo test -p tessera-server --release --test soak -- --ignored`. Timing
//! figures from a loaded machine are indicative only (§13.4).

mod support;

use std::time::{Duration, Instant};

use support::*;
use tessera_protocol::message::*;
use tessera_protocol::types::{Gait, Location, Motion, Vec3};
use tessera_server::Content;

#[tokio::test(flavor = "current_thread")]
#[ignore = "soak: run explicitly"]
async fn soak_walk_chat_reconnect() {
    let secs: u64 = std::env::var("TESSERA_SOAK_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let bots: usize = std::env::var("TESSERA_SOAK_BOTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let names: Vec<String> = (0..bots).map(|i| format!("bot-{i:04}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut cfg = TestServer::config();
    cfg.limits.max_connections = 1_024;
    let s = TestServer::start(cfg, Content::default_world(), &refs);
    let base = s.handle().stats();
    let mut clients = Vec::new();
    let letters = |i: usize| -> String {
        let mut n = String::from("Bot");
        let mut k = i;
        loop {
            n.push(char::from(b'a' + (k % 26) as u8));
            k /= 26;
            if k == 0 {
                break;
            }
        }
        n
    };
    for (i, a) in names.iter().enumerate() {
        clients.push(s.player(a, &letters(i)).await);
    }
    let start = Instant::now();
    let mut max_accounted = 0usize;
    let mut tick = 0u64;
    while start.elapsed() < Duration::from_secs(secs) {
        tick += 1;
        let t = start.elapsed().as_secs_f64();
        for (i, (c, w)) in clients.iter_mut().enumerate() {
            // Walk a small circle at 2 m/s around the entry point.
            let phase = t * 2.0 / 5.0 + i as f64;
            let l = Location::new(
                w.pose.location.region,
                w.pose.location.x + 5.0 * phase.cos(),
                w.pose.location.y + 5.0 * phase.sin(),
                w.pose.location.z,
            );
            c.move_to(
                l,
                Vec3::new((-2.0 * phase.sin()) as f32, (2.0 * phase.cos()) as f32, 0.0),
                Motion::new(Gait::Walking, false),
            );
            if tick % 45 == (i as u64 % 45) {
                c.chat(ChatChannel::Say, &format!("bot {i} tick {tick}"))
                    .await;
            }
            let _ = c.drain(Duration::ZERO).await;
        }
        // Reconnect one bot now and then.
        if tick.is_multiple_of(60) {
            let i = (tick / 60) as usize % clients.len();
            let (old, _) = clients.remove(i);
            old.conn.close(0u32.into(), b"");
            let mut c = s.client().await;
            c.hello().await;
            c.login(&names[i], PASSWORD).await.expect("relogin");
            let w = c.enter(&letters(i)).await.expect("re-enter");
            clients.insert(i, (c, w));
        }
        max_accounted = max_accounted.max(s.handle().stats().accounted_bytes);
        tokio::time::sleep(Duration::from_millis(66)).await;
    }
    let st = s.handle().stats();
    eprintln!(
        "soak (indicative): {bots} bots, {secs} s, ticks {}, overruns {}, max tick {} µs, corrections {}, max accounted {} bytes",
        st.ticks, st.overruns, st.tick_max_us, st.corrections, max_accounted
    );
    assert_eq!(
        st.corrections, 0,
        "bots within the limits are never corrected"
    );
    for (c, _) in clients.drain(..) {
        c.conn.close(0u32.into(), b"");
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let st = s.handle().stats();
        if st.sim_connections == base.sim_connections
            && st.logged_in == 0
            && st.in_world == 0
            && st.interest_pairs == 0
            && st.queue_bytes == 0
            && st.net_tasks == base.net_tasks
        {
            break;
        }
        assert!(Instant::now() < deadline, "not back to baseline: {st:?}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Hashing (19 MiB × 2) plus 64 KiB per connection plus small backlogs.
    assert!(max_accounted < 64 << 20, "{max_accounted}");
}
