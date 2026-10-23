// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend: malformed and hostile server input (`tessera-protocol.md` §13).
//! Every case must end the session with one clean `Disconnected` (or, for the
//! "dropped, not fatal" cases of §13.2, keep it running), never a panic, and the client
//! must close with `PROTOCOL_VIOLATION` when the server broke a fatal rule.

#![cfg(feature = "remote")]

mod remote_support;

use std::future::Future;
use std::pin::Pin;

use remote_support::*;
use tessera_net::remote::{AppErrorCode, EnterWorldFailedCode};
use tessera_net::{Command, DisconnectReason, Event};
use tessera_protocol::codes::EnterWorldFailedCode as WireEwf;
use tessera_protocol::frame::{StreamKind, encode_datagram, frame_to_vec};
use tessera_protocol::message::{
    ChatChannel as WireChannel, ChatMessage, Disconnect, EntityUpdate, Failure, Message,
    MessageType,
};
use tessera_protocol::value::{Property, Value, pid};

type Action = Box<dyn FnOnce(&mut Peer) -> Pin<Box<dyn Future<Output = ()> + '_>> + Send>;

/// Logs in, enters the world, runs `action`, and returns how the client closed.
fn in_world_then(action: Action) -> (Vec<Event>, Option<u64>) {
    let peer = spawn(PeerOptions::default(), move |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        action(&mut p).await;
        match p.closed().await {
            quinn::ConnectionError::ApplicationClosed(c) => Some(c.error_code.into_inner()),
            _ => None,
        }
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        std::time::Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    // Nothing follows Disconnected.
    assert!(pump_for(&mut s, std::time::Duration::from_millis(50)).is_empty());
    (events, peer.join())
}

fn violation(events: &[Event]) -> String {
    match events.last() {
        Some(Event::Disconnected {
            reason: DisconnectReason::TransportFailed(t),
        }) => {
            assert!(t.contains("protocol violation"), "{t}");
            t.clone()
        }
        other => panic!("expected a violation, got {other:?}"),
    }
}

const PROTOCOL_VIOLATION: Option<u64> = Some(1);

#[test]
fn frame_length_below_two() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move { p.send_raw(&[1, 0, 0, 0, 1]).await })
    }));
    violation(&events);
    assert_eq!(code, PROTOCOL_VIOLATION);
}

#[test]
fn frame_length_above_limit() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move { p.world_raw(&[0x01, 0x00, 0x01, 0x00, 0x01, 0x01]).await })
    }));
    violation(&events);
    assert_eq!(code, PROTOCOL_VIOLATION);
}

#[test]
fn unknown_stream_kind() {
    // After login the chat stream is open; the second (and last allowed) uni stream
    // carries an unknown kind byte.
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        let _s = p.open_raw(0x58).await;
        match p.closed().await {
            quinn::ConnectionError::ApplicationClosed(c) => Some(c.error_code.into_inner()),
            _ => None,
        }
    });
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    violation(&events);
    assert_eq!(peer.join(), PROTOCOL_VIOLATION);
}

#[test]
fn entity_create_for_the_player() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move {
            let now = p.now();
            p.world(entity_create(42, now, 20_260.0, 0.0)).await;
        })
    }));
    assert!(violation(&events).contains("player"));
    assert_eq!(code, PROTOCOL_VIOLATION);
}

#[test]
fn non_finite_motion_datagram() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move {
            let now = p.now();
            p.world(entity_create(43, now, 20_260.0, 0.0)).await;
            let mut bytes = encode_datagram(&motion(now + 1, 43, 1.0, 0.0)).unwrap();
            // Entry position x at offset 11 + 8.
            bytes[19..23].copy_from_slice(&f32::NAN.to_le_bytes());
            p.datagram_raw(&bytes);
        })
    }));
    violation(&events);
    assert_eq!(code, PROTOCOL_VIOLATION);
}

#[test]
fn control_stream_finished() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move {
            // FIN outside Closing (§2.3 rule 4) via a zero-length write on a finished
            // stream is not possible; send a frame header and stop: truncated at FIN.
            p.send_raw(&[0x10, 0, 0]).await;
            p.finish_control();
        })
    }));
    violation(&events);
    assert_eq!(code, PROTOCOL_VIOLATION);
}

#[test]
fn random_bytes_on_the_world_stream() {
    for seed in 1..=4u64 {
        let (events, code) = in_world_then(Box::new(move |p| {
            Box::pin(async move {
                let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
                let mut bytes = vec![0x40, 0x00, 0x00, 0x00];
                for _ in 0..0x40 {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    bytes.push(x as u8);
                }
                p.world_raw(&bytes).await;
            })
        }));
        violation(&events);
        assert_eq!(code, PROTOCOL_VIOLATION, "seed {seed}");
    }
}

#[test]
fn server_disconnect_is_reported_with_its_code() {
    let (events, code) = in_world_then(Box::new(|p| {
        Box::pin(async move {
            p.send(Message::Disconnect(Disconnect {
                code: AppErrorCode::Kicked,
                text: "bye".into(),
            }))
            .await;
        })
    }));
    assert_eq!(
        events.last(),
        Some(&Event::Disconnected {
            reason: DisconnectReason::ServerClosed("KICKED: bye".into())
        })
    );
    // The receiver of Disconnect closes with NO_ERROR (§12.3).
    assert_eq!(code, Some(0));
}

#[test]
fn server_close_is_reported() {
    let (events, _) = in_world_then(Box::new(|p| {
        Box::pin(async move {
            p.conn().close(6u32.into(), b"maintenance\x07 window");
        })
    }));
    assert_eq!(
        events.last(),
        Some(&Event::Disconnected {
            reason: DisconnectReason::ServerClosed("SERVER_SHUTDOWN: maintenance  window".into())
        })
    );
}

/// `EnterWorldFailed` puts the client back in CharacterSelect (`tessera-protocol.md`
/// §6.2.11): the session surfaces `Event::EnterWorldFailed`, stays connected, and a second
/// `EnterWorld` enters the world (`WorldEnter.instance` then known).
#[test]
fn enter_world_failure_returns_to_character_selection() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.expect(MessageType::EnterWorld).await;
        p.send(Message::EnterWorldFailed(Failure {
            code: WireEwf::NoInstance,
            text: "full".into(),
        }))
        .await;
        p.enter_world().await;
        p.expect(MessageType::Disconnect).await
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    let enter = || Command::EnterWorld {
        character: "Aldith".into(),
    };
    s.send(std::time::Instant::now(), enter()).unwrap();
    let events = pump(&mut s, |e| matches!(e, Event::EnterWorldFailed { .. }));
    assert!(
        matches!(events.last(), Some(Event::EnterWorldFailed { character, reason })
            if character == "Aldith" && reason == "cannot enter the world: NO_INSTANCE: full"),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::ChatMessage { .. } | Event::Disconnected { .. })),
        "{events:?}"
    );
    assert_eq!(s.state(), tessera_net::SessionState::Connected);
    assert_eq!(
        backend(&s).enter_world_refusal(),
        Some(EnterWorldFailedCode::NoInstance)
    );
    assert_eq!(backend(&s).failure(), None);
    assert_eq!(backend(&s).instance(), None);
    s.send(std::time::Instant::now(), enter()).unwrap();
    let events = pump(&mut s, |e| matches!(e, Event::WorldClock { .. }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
    assert!(matches!(
        events.last(),
        Some(Event::WorldClock { game_clock, rate, .. }) if *game_clock == 43_200.0 && *rate == 1.0
    ));
    assert_eq!(backend(&s).instance(), Some(1));
    s.send(std::time::Instant::now(), Command::Disconnect)
        .unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(peer.join(), Message::Disconnect(_)));
}

#[test]
fn stale_and_unknown_input_is_dropped_not_fatal() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        // §13.2: updates for unknown IDs, motion for unknown IDs, and a stale batch.
        p.world(Message::EntityUpdate(EntityUpdate {
            id: 999,
            changes: vec![Property::new(pid::STANCE, Value::Enum(1))],
        }))
        .await;
        p.datagram(&motion(p.now(), 999, 1.0, 0.0));
        let now = p.now();
        p.world(entity_create(43, now, 20_260.0, 0.0)).await;
        p.datagram(&motion(now - 1, 43, 1.0, 0.0));
        // Unknown property IDs are skipped (§7.1); control characters are sanitised (§5.2).
        p.world(Message::EntityUpdate(EntityUpdate {
            id: 43,
            changes: vec![Property::new(0x8001, Value::String("x".into()))],
        }))
        .await;
        p.chat(Message::ChatMessage(ChatMessage {
            server_time: now,
            channel: WireChannel::Say,
            echo: false,
            sender_entity: 43,
            sender: "Bram".into(),
            text: "still here".into(),
        }))
        .await;
        p.expect(MessageType::Disconnect).await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        std::time::Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let events = pump(
        &mut s,
        |e| matches!(e, Event::ChatMessage { text, .. } if text == "still here"),
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Disconnected { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntityMoved { id, .. } if id.0 == 999))
    );
    let _ = pump_for(&mut s, std::time::Duration::from_millis(100));
    assert!(backend(&s).stats().protocol_dropped >= 2);
    s.send(std::time::Instant::now(), Command::Disconnect)
        .unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

#[test]
fn chat_frames_carry_whole_messages_only() {
    // A frame split across many tiny writes still decodes (§5.4 reassembly).
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        let bytes = frame_to_vec(&Message::ChatMessage(ChatMessage {
            server_time: 1,
            channel: WireChannel::System,
            echo: false,
            sender_entity: 0,
            sender: String::new(),
            text: "split".into(),
        }))
        .unwrap();
        for b in bytes {
            p.chat_raw(&[b]).await;
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        p.linger().await;
    });
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::ChatMessage { .. }));
    assert!(matches!(events.last(), Some(Event::ChatMessage { text, .. }) if text == "split"));
    drop(s);
    peer.join();
    let _ = StreamKind::Chat;
}

/// `RemoteBackend::drop_connection` ends the connection abruptly (`test-server.md` §15.3):
/// the peer sees the connection close without a `Disconnect`, and the session ends with
/// one `Disconnected`. Before it, a time sample is surfaced as `Event::TimeSynced`.
#[test]
fn drop_connection_sends_no_disconnect() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let mut seen = Vec::new();
        while let Some(m) = p.recv().await {
            seen.push(m.message_type());
        }
        let closed = p.closed().await;
        (seen, closed)
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        std::time::Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let mut events = pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    if !events.iter().any(|e| matches!(e, Event::TimeSynced { .. })) {
        events.extend(pump(&mut s, |e| matches!(e, Event::TimeSynced { .. })));
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::TimeSynced { delay_us, .. } if *delay_us >= 0))
    );
    assert_eq!(backend(&s).instance(), Some(1));
    backend(&s).drop_connection();
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Disconnected { .. }))
            .count(),
        1
    );
    let (seen, closed) = peer.join();
    assert!(!seen.contains(&MessageType::Disconnect), "{seen:?}");
    assert!(
        !matches!(closed, quinn::ConnectionError::TimedOut),
        "{closed:?}"
    );
}
