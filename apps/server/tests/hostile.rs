// SPDX-License-Identifier: GPL-3.0-or-later
//! Hostile and malformed input, floods and resource bounds (`tessera-protocol.md` §13,
//! `test-server.md` §10): no panic, the offender is disconnected, others are unaffected.

mod support;

use std::time::Duration;

use support::*;
use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::message::*;
use tessera_protocol::types::{Gait, Motion, Vec3};
use tessera_server::Content;

const VIOLATION: u64 = 1;

/// A bystander in the world who must keep working.
async fn assert_alive(c: &mut RawClient, text: &str) {
    c.chat(ChatChannel::Say, text).await;
    let echo = c
        .chat_msg(|m| match m {
            Message::ChatMessage(x) if x.text == text => Some(x.echo),
            _ => None,
        })
        .await;
    assert_eq!(echo, Some(true), "the bystander must be unaffected");
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_frames_close_only_the_offender() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &["alice"]);
    let (mut bystander, _) = s.player("alice", "Aldith").await;
    let cases: Vec<Vec<u8>> = vec![
        // Frame length 1 (< 2).
        vec![1, 0, 0, 0, 1],
        // Frame length 65,537 (> 65,536), detected before any body byte.
        vec![1, 0, 1, 0],
        // Unknown message type 0x7777.
        vec![2, 0, 0, 0, 0x77, 0x77],
        // A server-to-client message (HelloAck) sent by the client.
        {
            let mut b = vec![0u8; 0];
            b.extend_from_slice(&[
                0x19, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0x1e, 0x0f, 0x02, 0x0a, 0x00,
            ]);
            b.extend_from_slice(b"Test Shard");
            b.extend_from_slice(&[0, 0]);
            b
        },
        // Hello with a trailing byte (length + 1).
        vec![
            0x21, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0x14, 0, b't', b'e', b's', b's', b'e',
            b'r', b'a', b'-', b'c', b'l', b'i', b'e', b'n', b't', b' ', b'0', b'.', b'7', b'.',
            b'0', 0,
        ],
        // Hello with max_entities below 64.
        vec![0x0c, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0x3f, 0, 0, 0],
    ];
    for (i, bytes) in cases.into_iter().enumerate() {
        let mut c = s.client().await;
        c.send_raw(&bytes).await;
        let code = c.closed_code(WAIT).await;
        assert_eq!(code, Some(VIOLATION), "case {i}");
    }
    // State violations after a valid Hello.
    let mut c = s.client().await;
    c.hello().await;
    c.send(&Message::EnterWorld(EnterWorld {
        name: "Aldith".into(),
    }))
    .await;
    assert_eq!(c.closed_code(WAIT).await, Some(VIOLATION));
    let mut c = s.client().await;
    c.hello().await;
    c.send(&Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 1024,
        client_name: String::new(),
    }))
    .await;
    assert_eq!(c.closed_code(WAIT).await, Some(VIOLATION));
    // A partial frame then FIN.
    let mut c = s.client().await;
    c.send_raw(&[0x10, 0, 0, 0, 1]).await;
    let _ = c.send.finish();
    assert_eq!(c.closed_code(WAIT).await, Some(VIOLATION));

    assert_alive(&mut bystander, "still here").await;
    assert!(s.handle().stats().violations >= 9);
}

#[tokio::test(flavor = "current_thread")]
async fn hostile_datagrams() {
    let s = TestServer::start(
        TestServer::config(),
        Content::default_world(),
        &["alice", "bob"],
    );
    let (mut bystander, _) = s.player("alice", "Aldith").await;
    // Well-formed datagrams before the state allows them are dropped, not fatal.
    let mut early = s.client().await;
    early.datagram(&Message::TimeRequest { client_time: 1 });
    tokio::time::sleep(Duration::from_millis(100)).await;
    early.hello().await;
    // A MoveInput before the world: dropped.
    early.move_to(
        tessera_protocol::types::Location::new(1, 0.0, 0.0, 0.0),
        Vec3::ZERO,
        Motion::STANDING,
    );
    early.login("bob", PASSWORD).await.expect("still usable");
    // A malformed datagram in the world is a violation.
    let (mut bad, _) = (early, ());
    bad.create("Bram", appearance(5)).await.unwrap();
    bad.enter("Bram").await.unwrap();
    // NaN x at offset 12 (§16.7).
    let mut d = vec![0x01, 0x03, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0];
    d.extend_from_slice(&f64::NAN.to_le_bytes());
    d.extend_from_slice(&[0u8; 8 + 8 + 4 + 12 + 1]);
    bad.datagram_raw(&d);
    assert_eq!(bad.closed_code(WAIT).await, Some(VIOLATION));
    assert_alive(&mut bystander, "datagrams ok").await;
}

#[tokio::test(flavor = "current_thread")]
async fn control_flood_is_disconnected() {
    let s = TestServer::start(
        TestServer::config(),
        Content::default_world(),
        &["alice", "bob"],
    );
    let (mut bystander, _) = s.player("alice", "Aldith").await;
    let mut c = s.client().await;
    c.hello().await;
    c.login("bob", PASSWORD).await.unwrap();
    for i in 0..400u64 {
        c.send(&Message::Ping { nonce: i }).await;
    }
    assert_eq!(c.disconnect_code(WAIT).await, Some(AppErrorCode::Flooding));
    assert_alive(&mut bystander, "after flood").await;
}

#[tokio::test(flavor = "current_thread")]
async fn movement_flood_stays_bounded() {
    let s = TestServer::start(
        TestServer::config(),
        Content::default_world(),
        &["alice", "bob"],
    );
    let (mut a, wa) = s.player("alice", "Aldith").await;
    let (mut b, wb) = s.player("bob", "Bram").await;
    a.created(EntityKind::Player).await.unwrap();
    // 5,000 MoveInputs as fast as possible: the server keeps only the newest per tick and
    // at most 30 per second.
    let mut l = wb.pose.location;
    for i in 0..5_000 {
        l.x = wb.pose.location.x + f64::from(i % 3) * 0.01;
        b.move_to(
            l,
            Vec3::new(0.1, 0.0, 0.0),
            Motion::new(Gait::Walking, false),
        );
        if i % 500 == 0 {
            tokio::task::yield_now().await;
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let st = s.handle().stats();
    assert!(st.queue_bytes < 1024 * 1024, "{st:?}");
    assert!(st.inbound_queued <= 64, "{st:?}");
    assert!(st.moves_accepted <= 60, "{st:?}");
    assert_alive(&mut a, "after move flood").await;
    let _ = wa;
}

#[tokio::test(flavor = "current_thread")]
async fn connection_limit_answers_server_full() {
    let mut cfg = TestServer::config();
    cfg.limits.max_connections = 2;
    let s = TestServer::start(cfg, Content::default_world(), &[]);
    let mut a = s.client().await;
    a.hello().await;
    let mut b = s.client().await;
    b.hello().await;
    let mut c = s.client().await;
    c.send(&Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 1024,
        client_name: "third".into(),
    }))
    .await;
    match c.control().await {
        Some(Message::Disconnect(d)) => assert_eq!(d.code, AppErrorCode::ServerFull),
        other => panic!("expected Disconnect(SERVER_FULL), got {other:?}"),
    }
    assert_eq!(s.handle().stats().server_full, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn counts_return_to_baseline() {
    let s = TestServer::start(
        TestServer::config(),
        Content::default_world(),
        &["alice", "bob"],
    );
    let base = s.handle().stats();
    {
        let (mut a, _) = s.player("alice", "Aldith").await;
        let (mut b, _) = s.player("bob", "Bram").await;
        a.chat(ChatChannel::Yell, "hi").await;
        b.send(&Message::Disconnect(Disconnect {
            code: AppErrorCode::NoError,
            text: String::new(),
        }))
        .await;
        a.conn.close(0u32.into(), b"");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let st = s.handle().stats();
        if st.connections == base.connections
            && st.sim_connections == base.sim_connections
            && st.logged_in == 0
            && st.in_world == 0
            && st.interest_pairs == 0
            && st.queue_bytes == 0
            && st.inbound_queued == 0
            && st.net_tasks == base.net_tasks
            && st.hash_jobs == 0
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "not back to baseline: {st:?} vs {base:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Wanderers stay once created (§5.4); every player entity is gone.
    assert_eq!(s.wait_stats(|st| st.entities, 3).await, 3);
}
