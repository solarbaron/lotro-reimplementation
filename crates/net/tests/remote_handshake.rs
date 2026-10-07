// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend: handshake, key pinning, trust on first use, login failures and
//! version mismatches (`tessera-protocol.md` §3, §4), against a scripted peer over
//! loopback UDP.

#![cfg(feature = "remote")]

mod remote_support;

use std::sync::Arc;

use remote_support::*;
use tessera_net::remote::{
    AppErrorCode, ClientFailure, FirstUse, LoginFailedCode, Password, RemoteConfig, RemoteFailure,
};
use tessera_net::{DisconnectReason, Event};
use tessera_protocol::codes::LoginFailedCode as WireLoginCode;
use tessera_protocol::message::{Disconnect, LoginFailed, Message, MessageType};
use tessera_protocol::transport::ServerIdentity;

fn disconnected(events: &[Event]) -> &DisconnectReason {
    match events.last() {
        Some(Event::Disconnected { reason }) => reason,
        other => panic!("expected Disconnected, got {other:?}"),
    }
}

#[test]
fn connects_with_pin_and_lists_characters() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith", "Bram"]).await;
        // Expect the client's polite goodbye after the test disconnects.
        p.expect(MessageType::Disconnect).await
    });
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    let Some(Event::Connected { characters }) = events.last() else {
        unreachable!()
    };
    let names: Vec<_> = characters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Aldith", "Bram"]);
    let b = backend(&s);
    assert_eq!(b.server_info().map(|i| i.name.as_str()), Some("Test Shard"));
    assert!(
        !format!("{b:?}").contains("correct horse"),
        "password in Debug"
    );
    // Statistics reach the main thread.
    let _ = pump_for(&mut s, std::time::Duration::from_millis(200));
    assert_eq!(backend(&s).stats().fingerprint, Some(peer.fingerprint));
    assert_eq!(backend(&s).stats().state, "CharacterSelect");
    s.send(std::time::Instant::now(), tessera_net::Command::Disconnect)
        .unwrap();
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert_eq!(disconnected(&events), &DisconnectReason::ClientRequested);
    match peer.join() {
        Message::Disconnect(d) => assert_eq!(d.code, AppErrorCode::NoError),
        other => panic!("{other:?}"),
    }
}

#[test]
fn wrong_pin_is_key_mismatch() {
    let other = ServerIdentity::generate().unwrap().fingerprint();
    let peer = spawn(PeerOptions::default(), |p| async move { p.handshake_error });
    let config = peer.config().pinned(other);
    let mut s = connect(config);
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    match disconnected(&events) {
        DisconnectReason::TransportFailed(t) => assert!(t.starts_with("KEY_MISMATCH"), "{t}"),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        backend(&s).failure(),
        Some(&RemoteFailure::Connect {
            failure: ClientFailure::KeyMismatch,
            presented: Some(peer.fingerprint),
        })
    );
    // The server saw the handshake fail: no password was ever sent.
    assert!(peer.join().is_some());
}

#[test]
fn unknown_key_is_refused_without_first_use() {
    let peer = spawn(PeerOptions::default(), |p| async move { p.handshake_error });
    let config = RemoteConfig::new(
        &peer.connect_string(false),
        "alice",
        Password::new("correct horse"),
    )
    .unwrap();
    let mut s = connect(config);
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    match disconnected(&events) {
        DisconnectReason::TransportFailed(t) => assert!(t.starts_with("KEY_UNKNOWN"), "{t}"),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        backend(&s).failure(),
        Some(&RemoteFailure::Connect {
            failure: ClientFailure::KeyUnknown,
            presented: Some(peer.fingerprint),
        })
    );
    assert!(peer.join().is_some());
}

#[test]
fn first_use_records_the_key_and_the_file_is_then_trusted() {
    let dir = temp_dir("tofu");
    let file = dir.join("known_servers");
    let identity = Arc::new(ServerIdentity::generate().unwrap());
    let opts = PeerOptions {
        identity: Some(identity.clone()),
        ..PeerOptions::default()
    };

    // 1. Unknown key, first use accepted: connects and records the key.
    let peer = spawn(opts.clone(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.linger().await;
    });
    let config = RemoteConfig::new(
        &peer.connect_string(false),
        "alice",
        Password::new("correct horse"),
    )
    .unwrap()
    .with_trust(trust(file.clone(), FirstUse::Accept));
    let mut s = connect(config);
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(&format!("{} {}", peer.addr, peer.fingerprint)),
        "{text}"
    );
    drop(s);
    peer.join();

    // 2. A new server on another port with the same key is a different server key:
    //    refused without first use...
    let peer = spawn(opts.clone(), |p| async move { p.handshake_error });
    let config = RemoteConfig::new(
        &peer.connect_string(false),
        "alice",
        Password::new("correct horse"),
    )
    .unwrap()
    .with_trust(trust(file.clone(), FirstUse::Refuse));
    let mut s = connect(config);
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(
        backend(&s).failure(),
        Some(RemoteFailure::Connect {
            failure: ClientFailure::KeyUnknown,
            ..
        })
    ));
    drop(s);
    peer.join();

    // 3. ...but once the file has its entry, it is trusted, and a wrong entry is a
    //    mismatch that first use cannot override.
    let peer = spawn(opts.clone(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.linger().await;
    });
    std::fs::write(&file, format!("{} {}\n", peer.addr, peer.fingerprint)).unwrap();
    let config = RemoteConfig::new(
        &peer.connect_string(false),
        "alice",
        Password::new("correct horse"),
    )
    .unwrap()
    .with_trust(trust(file.clone(), FirstUse::Refuse));
    let mut s = connect(config);
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    drop(s);
    peer.join();

    let peer = spawn(opts, |p| async move { p.handshake_error });
    let other = ServerIdentity::generate().unwrap().fingerprint();
    std::fs::write(&file, format!("{} {}\n", peer.addr, other)).unwrap();
    let config = RemoteConfig::new(
        &peer.connect_string(false),
        "alice",
        Password::new("correct horse"),
    )
    .unwrap()
    .with_trust(trust(file.clone(), FirstUse::Accept));
    let mut s = connect(config);
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(
        backend(&s).failure(),
        Some(RemoteFailure::Connect {
            failure: ClientFailure::KeyMismatch,
            ..
        })
    ));
    peer.join();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bad_password_disconnects_with_the_code() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.expect(MessageType::Hello).await;
        p.send(hello_ack()).await;
        p.expect(MessageType::Login).await;
        p.send(Message::LoginFailed(LoginFailed {
            code: WireLoginCode::BadCredentials,
            retry_after: 0,
            text: String::new(),
        }))
        .await;
        p.expect(MessageType::Disconnect).await
    });
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    match disconnected(&events) {
        DisconnectReason::ServerClosed(t) => assert!(t.contains("BAD_CREDENTIALS"), "{t}"),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        backend(&s).failure(),
        Some(&RemoteFailure::Login {
            code: LoginFailedCode::BadCredentials,
            retry_after: 0
        })
    );
    // The client said goodbye on the control stream.
    assert!(matches!(peer.join(), Message::Disconnect(_)));
}

#[test]
fn alpn_mismatch_is_no_common_version() {
    let peer = spawn(
        PeerOptions {
            alpn: Some(b"tessera/2".to_vec()),
            ..PeerOptions::default()
        },
        |p| async move { p.handshake_error },
    );
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    match disconnected(&events) {
        DisconnectReason::TransportFailed(t) => {
            assert!(t.starts_with("NO_COMMON_VERSION"), "{t}");
        }
        r => panic!("{r:?}"),
    }
    assert!(matches!(
        backend(&s).failure(),
        Some(RemoteFailure::Connect {
            failure: ClientFailure::NoCommonVersion,
            ..
        })
    ));
    assert!(peer.join().is_some());
}

#[test]
fn unsupported_minor_version_is_reported() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.expect(MessageType::Hello).await;
        p.send(Message::Disconnect(Disconnect {
            code: AppErrorCode::UnsupportedVersion,
            text: "need minor 3".into(),
        }))
        .await;
        p.closed().await
    });
    let mut s = connect(peer.config());
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    match disconnected(&events) {
        DisconnectReason::ServerClosed(t) => {
            assert_eq!(t, "UNSUPPORTED_VERSION: need minor 3");
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(
        backend(&s).failure(),
        Some(&RemoteFailure::Server {
            code: AppErrorCode::UnsupportedVersion
        })
    );
    // §12.3 step 2: the receiver of Disconnect closes with NO_ERROR.
    match peer.join() {
        quinn::ConnectionError::ApplicationClosed(c) => {
            assert_eq!(c.error_code.into_inner(), 0);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn unreachable_server_times_out() {
    // A UDP port nobody answers on: the handshake times out (10 s, §12.1).
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let fp = ServerIdentity::generate().unwrap().fingerprint();
    let config = RemoteConfig::new(
        &format!("tessera://{addr}?pin={fp}"),
        "alice",
        Password::new("x"),
    )
    .unwrap();
    let mut s = connect(config);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut events = Vec::new();
    while std::time::Instant::now() < deadline {
        s.poll_into(std::time::Instant::now(), &mut events);
        if matches!(events.last(), Some(Event::Disconnected { .. })) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(disconnected(&events), &DisconnectReason::TimedOut);
    drop(sock);
}
