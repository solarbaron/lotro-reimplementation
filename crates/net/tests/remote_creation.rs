// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend, protocol 1.1 (decision 0025): `CreateCharacter` under
//! `character-screens.md` §7.1.1, the character list with looks (`scenario-feed.md` §9.7),
//! slots and looks on world entry, and the minor-0 fall-back, against a scripted peer.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::Instant;

use remote_support::*;
use tessera_net::domain::{CharacterCommand, CharacterEvent, Text};
use tessera_net::remote::ServerCaps;
use tessera_net::{
    Appearance, Body, Command, DataId, Equipped, Event, Look, LookEntry, NetError, PropertyValue,
};
use tessera_protocol::codes::CharacterCreateFailedCode;
use tessera_protocol::message::{
    CharacterList, Failure, HelloAck, LoginOk, Message, MessageType, WorldEnter,
};
use tessera_protocol::types::{self as wt, CharacterSummary, Pose};

fn wire_look() -> wt::Look {
    wt::Look {
        entries: vec![
            wt::LookEntry {
                property: 0x1000_0A01,
                value: wt::LookValue::Float32(0.5),
            },
            wt::LookEntry {
                property: 0x1000_0A03,
                value: wt::LookValue::String("8080ff".into()),
            },
        ],
    }
}

fn model_look() -> Look {
    Look {
        entries: vec![
            LookEntry {
                property: 0x1000_0A01,
                value: PropertyValue::Float(0.5),
            },
            LookEntry {
                property: 0x1000_0A03,
                value: PropertyValue::String("8080ff".into()),
            },
        ],
    }
}

fn hello_ack_v11(caps: ServerCaps) -> Message {
    Message::HelloAck(HelloAck {
        minor: 1,
        server_caps: caps,
        tick_rate: 30,
        snapshot_rate: 15,
        interest_radius: 2,
        server_name: "Test Shard".into(),
        motd: String::new(),
    })
}

fn aldith() -> CharacterSummary {
    CharacterSummary {
        name: "Aldith".into(),
        body: wt::Body::Character {
            species: 23,
            sex: 1,
        },
    }
}

fn create(name: &str, appearance: Option<Appearance>) -> Command {
    Command::Character(CharacterCommand::CreateCharacter {
        name: name.into(),
        appearance: appearance.map(Box::new),
        class: Some(5),
        nationality: Some(2),
    })
}

fn human() -> Appearance {
    Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    })
}

#[test]
fn creation_with_looks_at_minor_1() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.expect(MessageType::Hello).await;
        p.send(hello_ack_v11(
            ServerCaps::CHARACTER_CREATION | ServerCaps::APPEARANCE_CHANGE,
        ))
        .await;
        p.set_minor(1);
        p.expect(MessageType::Login).await;
        p.send(Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![aldith()],
            looks: vec![wire_look()],
        }))
        .await;
        p.open(tessera_protocol::frame::StreamKind::Chat).await;
        // Cora: the look travels, class and nationality do not.
        let Message::CharacterCreate(c) = p.expect(MessageType::CharacterCreate).await else {
            unreachable!()
        };
        assert_eq!(c.name, "Cora");
        assert_eq!(c.look, wire_look());
        assert_eq!(c.appearance.equipment[0].slot, Some(17));
        p.send(Message::CharacterList(CharacterList {
            characters: vec![
                aldith(),
                CharacterSummary {
                    name: "Cora".into(),
                    body: c.appearance.body,
                },
            ],
            looks: vec![wire_look(), c.look.clone()],
        }))
        .await;
        // Dora: refused with a code; the server's text is never shown.
        p.expect(MessageType::CharacterCreate).await;
        p.send(Message::CharacterCreateFailed(Failure {
            code: CharacterCreateFailedCode::NameTaken,
            text: "server words".into(),
        }))
        .await;
        // Eve: a list without the name is SERVER_ERROR.
        p.expect(MessageType::CharacterCreate).await;
        p.send(Message::CharacterList(CharacterList {
            characters: vec![aldith()],
            looks: vec![wt::Look::none()],
        }))
        .await;
        // World entry: the look and the slots reach the player's appearance.
        p.expect(MessageType::EnterWorld).await;
        p.open(tessera_protocol::frame::StreamKind::World).await;
        let mut a = appearance();
        a.equipment.push(wt::Equipped {
            item: 0x7000_1234,
            slot: Some(18),
            ..wt::Equipped::default()
        });
        let now = p.now();
        p.world(Message::WorldEnter(WorldEnter {
            player: 42,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: now,
            pose: Pose {
                location: start_location(),
                heading: 0.0,
                anchor: None,
            },
            appearance: a,
            look: wire_look(),
        }))
        .await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    let seen = pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    assert!(matches!(seen.last(), Some(Event::Connected { characters }) if characters.len() == 1));
    // §9.7: one CharacterInfo per LoginOk entry, with the entry's body and look.
    let info = pump(&mut s, |e| matches!(e, Event::Character(_)));
    match info.last() {
        Some(Event::Character(CharacterEvent::CharacterInfo {
            name,
            body,
            class,
            level,
            look,
        })) => {
            assert_eq!(name, "Aldith");
            assert_eq!(
                *body,
                Some(Body::Character {
                    species: 23,
                    sex: 1
                })
            );
            assert_eq!((*class, *level), (None, None));
            assert_eq!(look.as_deref(), Some(&model_look()));
        }
        other => panic!("{other:?}"),
    }
    let b = backend(&s);
    assert!(b.can_create_characters());
    assert!(b.can_change_appearance());
    assert_eq!(b.negotiated_minor(), Some(1));
    assert!(
        b.server_caps()
            .is_some_and(|c| c.contains(ServerCaps::CHARACTER_CREATION))
    );

    // Without an appearance the command cannot travel.
    assert!(matches!(
        s.send(Instant::now(), create("Cora", None)),
        Err(NetError::InvalidCommand(_))
    ));
    let app = human()
        .wearing(Equipped::new(DataId(0x7000_1234)).in_slot(17))
        .with_look(Some(model_look()));
    s.send(Instant::now(), create("Cora", Some(app.clone())))
        .unwrap();
    // One outstanding creation at a time; no EnterWorld meanwhile; DeleteCharacter never.
    assert!(matches!(
        s.send(Instant::now(), create("Cora2", Some(app.clone()))),
        Err(NetError::InvalidCommand(_))
    ));
    assert!(matches!(
        s.send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldith".into()
            }
        ),
        Err(NetError::InvalidCommand(_))
    ));
    assert!(matches!(
        s.send(
            Instant::now(),
            Command::Character(CharacterCommand::DeleteCharacter {
                name: "Aldith".into()
            })
        ),
        Err(NetError::InvalidCommand(_))
    ));
    let seen = pump(&mut s, |e| matches!(e, Event::Character(_)));
    match seen.last() {
        Some(Event::Character(CharacterEvent::CharacterInfo {
            name, look, class, ..
        })) => {
            assert_eq!(name, "Cora");
            assert_eq!(*class, None, "the class never travels");
            assert_eq!(look.as_deref(), Some(&model_look()));
        }
        other => panic!("{other:?}"),
    }
    assert!(backend(&s).outstanding_creation().is_none());

    s.send(Instant::now(), create("Dora", Some(human())))
        .unwrap();
    let seen = pump(&mut s, |e| matches!(e, Event::Character(_)));
    assert_eq!(
        seen.last(),
        Some(&Event::Character(CharacterEvent::CharacterRefused {
            name: "Dora".into(),
            reason: Text::Literal("A character with this name already exists.".into()),
        }))
    );
    s.send(Instant::now(), create("Eve", Some(human())))
        .unwrap();
    let seen = pump(&mut s, |e| matches!(e, Event::Character(_)));
    assert!(matches!(
        seen.last(),
        Some(Event::Character(CharacterEvent::CharacterRefused { name, .. })) if name == "Eve"
    ));

    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let seen = pump(&mut s, |e| matches!(e, Event::AppearanceChanged { .. }));
    match seen.last() {
        Some(Event::AppearanceChanged { appearance, .. }) => {
            assert_eq!(appearance.look.as_deref(), Some(&model_look()));
            assert_eq!(appearance.equipment[0].slot, Some(18));
        }
        other => panic!("{other:?}"),
    }
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

#[test]
fn minor_0_server_gets_no_look_and_capability_gates_creation() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.expect(MessageType::Hello).await;
        // A v1.0 server with creation: the client must send the v1.0 layout (the peer
        // decodes at minor 0, where an appended look would be a violation).
        p.send(Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::CHARACTER_CREATION,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Old".into(),
            motd: String::new(),
        }))
        .await;
        p.expect(MessageType::Login).await;
        p.send(Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![aldith()],
            looks: Vec::new(),
        }))
        .await;
        p.open(tessera_protocol::frame::StreamKind::Chat).await;
        let Message::CharacterCreate(c) = p.expect(MessageType::CharacterCreate).await else {
            unreachable!()
        };
        assert!(c.look.is_empty());
        assert_eq!(c.appearance.equipment[0].slot, None);
        p.send(Message::CharacterList(CharacterList {
            characters: vec![
                aldith(),
                CharacterSummary {
                    name: "Cora".into(),
                    body: c.appearance.body,
                },
            ],
            looks: Vec::new(),
        }))
        .await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    let info = pump(&mut s, |e| matches!(e, Event::Character(_)));
    assert!(matches!(
        info.last(),
        Some(Event::Character(CharacterEvent::CharacterInfo {
            look: None,
            ..
        }))
    ));
    assert_eq!(backend(&s).negotiated_minor(), Some(0));
    let app = human()
        .wearing(Equipped::new(DataId(7)).in_slot(17))
        .with_look(Some(model_look()));
    s.send(Instant::now(), create("Cora", Some(app))).unwrap();
    let seen = pump(&mut s, |e| matches!(e, Event::Character(_)));
    assert!(matches!(
        seen.last(),
        Some(Event::Character(CharacterEvent::CharacterInfo { name, look: None, .. })) if name == "Cora"
    ));
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

#[test]
fn creation_refused_without_the_capability() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    assert!(!backend(&s).can_create_characters());
    assert!(matches!(
        s.send(Instant::now(), create("Cora", Some(human()))),
        Err(NetError::InvalidCommand(_))
    ));
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}
