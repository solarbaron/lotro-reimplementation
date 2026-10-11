// SPDX-License-Identifier: GPL-3.0-or-later
//! Local selection on the real `RemoteBackend` (`scenario-feed.md` §9.7, decision 0032 D1),
//! over QUIC against a scripted peer: cases R1 and R5 of §14.11 and the "no message" rule.
//! `remote_selection.rs` runs every case on the harness; this file checks that the backend
//! the client uses answers locally, before anything drained in the same poll, and sends
//! nothing for a selection.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::{Duration, Instant};

use remote_support::*;
use tessera_net::{CombatCommand, CombatEvent, Command, EntityId, Event, Session};
use tessera_protocol::message::{DestroyReason, EntityDestroy, Message, MessageType};

const PLAYER: EntityId = EntityId(42);
const WANDERER: EntityId = EntityId(43);

fn send(s: &mut Session, c: Command) {
    s.send(Instant::now(), c).unwrap();
}

fn targets(events: &[Event]) -> Vec<(EntityId, Option<EntityId>)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Combat(CombatEvent::TargetChanged { entity, target }) => {
                Some((*entity, *target))
            }
            _ => None,
        })
        .collect()
}

fn changed(target: Option<EntityId>) -> Event {
    Event::Combat(CombatEvent::TargetChanged {
        entity: PLAYER,
        target,
    })
}

/// R1 then R5: the peer creates 43, the client selects it, and the peer removes 43 before
/// the client polls again. The first event of that poll is the answer, the removal comes
/// after it, and the peer's control stream carries nothing for either selection (pings
/// are answered by the peer and not counted).
#[test]
fn r1_r5_local_answer_on_the_real_backend() {
    let (go_tx, go_rx) = tokio::sync::oneshot::channel::<()>();
    let (sent_tx, sent_rx) = std::sync::mpsc::channel::<()>();
    let peer = spawn(PeerOptions::default(), move |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let now = p.now();
        p.world(entity_create(WANDERER.0, now, 20_260.0, 0.0)).await;
        tokio::time::timeout(STEP, go_rx)
            .await
            .expect("the client never selected")
            .expect("the client side dropped");
        p.world(Message::EntityDestroy(EntityDestroy {
            id: WANDERER.0,
            reason: DestroyReason::Removed,
        }))
        .await;
        let _ = sent_tx.send(());
        // Everything on the control stream until the client says Disconnect.
        let mut other = Vec::new();
        while let Some(m) = p.recv().await {
            if m.message_type() == MessageType::Disconnect {
                break;
            }
            other.push(m);
        }
        other
    });

    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    send(
        &mut s,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    );
    pump(
        &mut s,
        |e| matches!(e, Event::EntityCreated { id, .. } if *id == WANDERER),
    );
    let earlier = backlog();
    assert!(targets(&earlier).is_empty());

    // R1, and R5's command: 43 is known when the command is given.
    send(
        &mut s,
        Command::Combat(CombatCommand::SelectTarget {
            target: Some(WANDERER),
        }),
    );
    go_tx.send(()).unwrap();
    sent_rx
        .recv_timeout(STEP)
        .expect("the peer sent no removal");
    // Give the removal time to reach the network thread's queue, so this poll drains it
    // with the answer pending (R5). If it is late the order is still checked below.
    std::thread::sleep(Duration::from_millis(300));
    let mut first = Vec::new();
    s.poll_into(Instant::now(), &mut first);
    assert_eq!(first.first(), Some(&changed(Some(WANDERER))), "{first:#?}");
    assert_eq!(targets(&first).len(), 1, "{first:#?}");
    let removed = |e: &Event| matches!(e, Event::EntityDespawned { id } if *id == WANDERER);
    let mut all = first;
    if !all.iter().any(removed) {
        all.extend(pump(&mut s, removed));
    }
    let answer = all.iter().position(|e| *e == changed(Some(WANDERER)));
    let removal = all.iter().position(removed);
    assert!(answer < removal, "{all:#?}");
    // §9.7 item 5: the removal causes no `TargetChanged` of its own.
    let later = pump_for(&mut s, Duration::from_millis(100));
    assert!(targets(&all[1..]).is_empty() && targets(&later).is_empty());

    // A selection of the player, too, stays local.
    send(
        &mut s,
        Command::Combat(CombatCommand::SelectTarget {
            target: Some(PLAYER),
        }),
    );
    let mut next = Vec::new();
    s.poll_into(Instant::now(), &mut next);
    assert_eq!(next.first(), Some(&changed(Some(PLAYER))), "{next:#?}");

    send(&mut s, Command::Disconnect);
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    let other = peer.join();
    assert!(
        other.is_empty(),
        "selection sent control messages: {other:#?}"
    );
}
