// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend under floods while the main thread lags: memory held on the
//! network thread stays bounded (it stops reading streams and drops movement,
//! `tessera-protocol.md` §13.3), reliable state is never lost, and the session survives.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::{Duration, Instant};

use remote_support::*;
use tessera_net::remote::{PENDING_PAUSE, READ_CHUNK};
use tessera_net::{Command, EntityId, Event};
use tessera_protocol::message::{
    ChatChannel as WireChannel, ChatMessage, DestroyReason, EntityDestroy, Message,
};

/// The most items the network thread may hold: the pause threshold plus what one read of
/// the smallest frames (6 bytes) can decode.
const PENDING_BOUND: usize = PENDING_PAUSE + READ_CHUNK / 6 + 64;

fn line(i: u32) -> Message {
    Message::ChatMessage(ChatMessage {
        server_time: u64::from(i),
        channel: WireChannel::Yell,
        echo: false,
        sender_entity: 0,
        sender: "Bram".into(),
        text: format!("line {i}"),
    })
}

fn enter(s: &mut tessera_net::Session) {
    pump(s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(s, |e| matches!(e, Event::WorldEntered { .. }));
}

#[test]
fn chat_flood_is_delivered_in_order_with_bounded_memory() {
    const N: u32 = 20_000;
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        for i in 0..N {
            p.chat(line(i)).await;
        }
        p.linger().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    // The main thread stalls while the server floods.
    std::thread::sleep(Duration::from_millis(1500));
    let mut next = 0u32;
    let deadline = Instant::now() + STEP * 3;
    let mut events = backlog();
    let mut peak = 0;
    while next < N {
        assert!(Instant::now() < deadline, "only {next} lines arrived");
        s.poll_into(Instant::now(), &mut events);
        peak = peak.max(backend(&s).stats().pending_peak);
        for e in events.drain(..) {
            match e {
                Event::ChatMessage { text, .. } => {
                    assert_eq!(text, format!("line {next}"), "out of order");
                    next += 1;
                }
                Event::Disconnected { reason } => panic!("disconnected: {reason:?}"),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(peak <= PENDING_BOUND, "pending peak {peak}");
    let stats = backend(&s).stats();
    assert!(
        stats.read_pauses > 0,
        "the network thread never paused reading"
    );
    drop(s);
    peer.join();
}

#[test]
fn motion_flood_is_shed_and_reliable_state_survives() {
    const N: u64 = 20_000;
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let t0 = p.now();
        p.world(entity_create(43, t0, 20_260.0, 1.0)).await;
        for i in 1..=N {
            p.datagram(&motion(t0 + i, 43, 20_260.0 + i as f32 * 0.001, 1.0));
            if i % 512 == 0 {
                tokio::task::yield_now().await;
            }
        }
        p.chat(line(7)).await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    enter(&mut s);
    std::thread::sleep(Duration::from_millis(1500));
    let mut moved = 0u64;
    let events = pump(&mut s, |e| {
        if matches!(
            e,
            Event::EntityMoved {
                id: EntityId(43),
                ..
            }
        ) {
            moved += 1;
        }
        matches!(e, Event::ChatMessage { text, .. } if text == "line 7")
    });
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Disconnected { .. }))
    );
    let _ = pump_for(&mut s, Duration::from_millis(200));
    let b = backend(&s);
    let stats = b.stats();
    assert!(
        stats.pending_peak <= PENDING_BOUND,
        "pending peak {}",
        stats.pending_peak
    );
    assert!(moved <= N);
    // Memory per entity is bounded too.
    assert!(
        b.interpolation().get(EntityId(43)).unwrap().len() <= tessera_net::interp::MAX_SNAPSHOTS
    );
    assert_eq!(b.entity_count(), 1);
    eprintln!(
        "motion flood: {moved} moves surfaced, {} dropped on the network thread, {} batches",
        stats.motion_dropped, stats.motion_batches
    );
    drop(s);
    peer.join();
}

#[test]
fn entity_churn_flood_keeps_the_table_exact() {
    const N: u64 = 5_000;
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        for i in 0..N {
            let now = p.now();
            p.world(entity_create(100 + i, now, 20_260.0, 0.0)).await;
            p.world(Message::EntityDestroy(EntityDestroy {
                id: 100 + i,
                reason: DestroyReason::LeftInterest,
            }))
            .await;
        }
        p.world(entity_create(99, p.now(), 20_260.0, 0.0)).await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    enter(&mut s);
    std::thread::sleep(Duration::from_millis(1000));
    let (mut created, mut despawned) = (0u64, 0u64);
    let deadline = Instant::now() + STEP * 3;
    let mut events = backlog();
    'outer: loop {
        assert!(Instant::now() < deadline);
        s.poll_into(Instant::now(), &mut events);
        for e in events.drain(..) {
            match e {
                Event::EntityCreated {
                    id: EntityId(99), ..
                } => break 'outer,
                Event::EntityCreated { .. } => created += 1,
                Event::EntityDespawned { .. } => despawned += 1,
                Event::Disconnected { reason } => panic!("{reason:?}"),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!((created, despawned), (N, N));
    let b = backend(&s);
    assert_eq!(b.entity_count(), 1);
    assert_eq!(b.interpolation().len(), 1);
    assert!(b.stats().pending_peak <= PENDING_BOUND);
    drop(s);
    peer.join();
}
