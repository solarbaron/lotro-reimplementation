// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend: the commands that replaced the `RemoteHandle` side channels
//! (`tessera-protocol.md` Appendix A implementer OPEN): `Register` sends `Register`,
//! `Emote` sends `EmoteSet`, and `SetAppearance` is refused when the server did not
//! advertise `APPEARANCE_CHANGE`, against a scripted peer.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::Instant;

use remote_support::*;
use tessera_net::remote::RemoteBackend;
use tessera_net::{Appearance, Body, Command, DisconnectReason, Event, NetError, Session};
use tessera_protocol::frame::StreamKind;
use tessera_protocol::message::{HelloAck, LoginOk, Message, MessageType, ServerCaps};

#[derive(Debug, Default)]
struct Seen {
    registered: bool,
    emote: Option<(u32, bool)>,
}

#[test]
fn register_emote_and_a_refused_appearance() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        let mut seen = Seen::default();
        p.expect(MessageType::Hello).await;
        // Registration allowed, appearance changes not.
        p.send(Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::REGISTRATION,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Test Shard".into(),
            motd: String::new(),
        }))
        .await;
        if let Message::Register(c) = p.expect(MessageType::Register).await {
            seen.registered = c.account == "alice";
        }
        p.send(Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![tessera_protocol::types::CharacterSummary {
                name: "Aldith".into(),
                body: tessera_protocol::types::Body::Character {
                    species: 23,
                    sex: 1,
                },
            }],
            looks: Vec::new(),
        }))
        .await;
        p.open(StreamKind::Chat).await;
        p.enter_world().await;
        if let Message::EmoteSet(e) = p.expect(MessageType::EmoteSet).await {
            seen.emote = Some((e.anim, e.looping));
        }
        p.linger().await;
        seen
    });

    let mut s = Session::new(RemoteBackend::new(peer.config()).unwrap());
    s.send(Instant::now(), Command::Register).unwrap();
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    let refused = s.send(
        Instant::now(),
        Command::SetAppearance {
            appearance: Box::new(Appearance::new(Body::Character {
                species: 23,
                sex: 1,
            })),
        },
    );
    assert!(
        matches!(refused, Err(NetError::InvalidCommand(_))),
        "{refused:?}"
    );
    s.send(
        Instant::now(),
        Command::Emote {
            anim: 0x1000_0456,
            looping: true,
        },
    )
    .unwrap();
    // The peer reads the emote before the session ends.
    let _ = pump_for(&mut s, std::time::Duration::from_millis(200));
    s.send(Instant::now(), Command::Disconnect).unwrap();
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(
        events.last(),
        Some(Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        })
    ));
    let seen = peer.join();
    assert!(seen.registered, "the peer saw Register");
    assert_eq!(seen.emote, Some((0x1000_0456, true)));
}
