// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend, protocol 1.2 (decision 0035): named chat channels at negotiated
//! minor 2 (`scenario-feed.md` §9.8, §14.13 S1–S6; `tessera-protocol.md` Appendix A),
//! against a scripted peer.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::Instant;

use remote_support::*;
use tessera_net::domain::{SocialCommand, SocialEvent};
use tessera_net::remote::ServerCaps;
use tessera_net::{ChatChannel, Command, DisconnectReason, Event, NetError, Session};
use tessera_protocol::codes::ChannelRefusedCode;
use tessera_protocol::frame::{StreamKind, frame_to_vec_minor};
use tessera_protocol::message::{
    ChannelLeft, ChannelLeftReason, ChannelRefused, ChatChannel as Wire, ChatMessage, HelloAck,
    LoginOk, Message, MessageType,
};
use tessera_protocol::types::{self as wt, CharacterSummary};

fn hello_ack(minor: u16, caps: ServerCaps) -> Message {
    Message::HelloAck(HelloAck {
        minor,
        server_caps: caps,
        tick_rate: 30,
        snapshot_rate: 15,
        interest_radius: 2,
        server_name: "Test Shard".into(),
        motd: String::new(),
    })
}

/// Hello → `HelloAck` at `minor` with `caps`, Login → `LoginOk` (Aldith), chat stream, then
/// world entry.
async fn greet_and_enter(p: &mut Peer, minor: u16, caps: ServerCaps) {
    p.expect(MessageType::Hello).await;
    p.send(hello_ack(minor, caps)).await;
    p.set_minor(minor);
    p.expect(MessageType::Login).await;
    p.send(Message::LoginOk(LoginOk {
        account: "alice".into(),
        characters: vec![CharacterSummary {
            name: "Aldith".into(),
            body: wt::Body::Character {
                species: 23,
                sex: 1,
            },
        }],
        looks: if minor >= 1 {
            vec![wt::Look::none()]
        } else {
            Vec::new()
        },
    }))
    .await;
    p.open(StreamKind::Chat).await;
    p.enter_world().await;
}

/// Echoes the client's "marker" line, so the client knows the peer has read everything
/// before it.
async fn echo_marker(p: &mut Peer) {
    let now = p.now();
    p.chat(Message::ChatMessage(ChatMessage {
        server_time: now,
        channel: Wire::Say,
        echo: true,
        sender_entity: 42,
        sender: "Aldith".into(),
        text: "marker".into(),
    }))
    .await;
}

/// Waits for the echo of [`echo_marker`].
fn wait_marker(s: &mut Session) {
    pump(
        s,
        |e| matches!(e, Event::ChatMessage { text, .. } if text == "marker"),
    );
}

fn lore(name: &str) -> Command {
    Command::Social(SocialCommand::JoinChannel { name: name.into() })
}

fn enter(s: &mut Session) {
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

/// The events after the last pump up to (and including) the one matching `pred`, without
/// moves and other world noise.
fn until(s: &mut Session, pred: impl FnMut(&Event) -> bool) -> Vec<Event> {
    pump(s, pred)
        .into_iter()
        .filter(|e| matches!(e, Event::Social(_) | Event::ChatMessage { .. }))
        .collect()
}

#[test]
fn s1_to_s4_and_s6_channels_at_minor_2() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        greet_and_enter(&mut p, 2, ServerCaps::CHAT_CHANNELS).await;
        // S1: one ChannelJoin, answered with ChannelJoined.
        let Message::ChannelJoin { name } = p.expect(MessageType::ChannelJoin).await else {
            unreachable!()
        };
        assert_eq!(name, "Lore");
        p.chat(Message::ChannelJoined {
            name: "Lore".into(),
        })
        .await;
        // S2: one ChatSend on channel 6, echoed.
        let Message::ChatSend(c) = p.expect(MessageType::ChatSend).await else {
            unreachable!()
        };
        assert_eq!(
            c.channel,
            Wire::Named {
                name: "Lore".into()
            }
        );
        assert_eq!(c.text, "hi");
        let now = p.now();
        p.chat(Message::ChatMessage(ChatMessage {
            server_time: now,
            channel: Wire::Named {
                name: "Lore".into(),
            },
            echo: true,
            sender_entity: 42,
            sender: "Aldith".into(),
            text: "hi".into(),
        }))
        .await;
        // S3: the server's spelling wins.
        let Message::ChannelJoin { name } = p.expect(MessageType::ChannelJoin).await else {
            unreachable!()
        };
        assert_eq!(name, "lore");
        p.chat(Message::ChannelJoined {
            name: "Lore".into(),
        })
        .await;
        // Leave, then S4: two joins, the second refused.
        let Message::ChannelLeave { name } = p.expect(MessageType::ChannelLeave).await else {
            unreachable!()
        };
        assert_eq!(name, "Lore");
        p.chat(Message::ChannelLeft(ChannelLeft {
            name: "Lore".into(),
            reason: ChannelLeftReason::Left,
        }))
        .await;
        p.expect(MessageType::ChannelJoin).await;
        p.expect(MessageType::ChannelJoin).await;
        p.chat(Message::ChannelJoined {
            name: "Lore".into(),
        })
        .await;
        p.chat(Message::ChannelRefused(ChannelRefused {
            code: ChannelRefusedCode::AlreadyJoined,
            name: "Lore".into(),
            text: "server words".into(),
        }))
        .await;
        // The server ends the membership on its own.
        p.chat(Message::ChannelLeft(ChannelLeft {
            name: "Lore".into(),
            reason: ChannelLeftReason::Removed,
        }))
        .await;
        // S6: nothing was sent for the refused commands; the next message is this Say.
        let Message::ChatSend(c) = p.expect(MessageType::ChatSend).await else {
            unreachable!()
        };
        assert_eq!((c.channel, c.text.as_str()), (Wire::Say, "marker"));
        echo_marker(&mut p).await;
        p.linger().await;
    });
    let mut s = connect(peer.config());
    enter(&mut s);
    let b = backend(&s);
    assert_eq!(b.negotiated_minor(), Some(2));
    assert!(b.has_chat_channels());

    // S1
    s.send(Instant::now(), lore("Lore")).unwrap();
    let seen = until(&mut s, |e| matches!(e, Event::Social(_)));
    assert_eq!(
        seen,
        vec![Event::Social(SocialEvent::ChannelJoined {
            channel: "Lore".into()
        })]
    );
    // S2
    s.send(
        Instant::now(),
        Command::Chat {
            channel: ChatChannel::Named {
                name: "Lore".into(),
            },
            text: "hi".into(),
        },
    )
    .unwrap();
    let seen = until(&mut s, |e| matches!(e, Event::ChatMessage { .. }));
    assert_eq!(
        seen,
        vec![Event::ChatMessage {
            channel: ChatChannel::Named {
                name: "Lore".into()
            },
            sender: Some("Aldith".into()),
            text: "hi".into(),
        }]
    );
    // S3
    s.send(Instant::now(), lore("lore")).unwrap();
    let seen = until(&mut s, |e| matches!(e, Event::Social(_)));
    assert_eq!(
        seen,
        vec![Event::Social(SocialEvent::ChannelJoined {
            channel: "Lore".into()
        })]
    );
    s.send(
        Instant::now(),
        Command::Social(SocialCommand::LeaveChannel {
            name: "Lore".into(),
        }),
    )
    .unwrap();
    let seen = until(&mut s, |e| matches!(e, Event::Social(_)));
    assert_eq!(
        seen,
        vec![Event::Social(SocialEvent::ChannelLeft {
            channel: "Lore".into()
        })]
    );
    // S4
    s.send(Instant::now(), lore("Lore")).unwrap();
    s.send(Instant::now(), lore("Lore")).unwrap();
    let seen = until(&mut s, |e| matches!(e, Event::ChatMessage { .. }));
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(
        seen[0],
        Event::Social(SocialEvent::ChannelJoined {
            channel: "Lore".into()
        })
    );
    match &seen[1] {
        Event::ChatMessage {
            channel: ChatChannel::System,
            sender: None,
            text,
        } => {
            assert!(text.contains("Lore"), "{text}");
            assert!(!text.contains("server words"), "{text}");
        }
        other => panic!("{other:?}"),
    }
    // REMOVED: ChannelLeft, then a System line.
    let seen = until(&mut s, |e| matches!(e, Event::ChatMessage { .. }));
    assert!(
        matches!(
            seen.as_slice(),
            [
                Event::Social(SocialEvent::ChannelLeft { channel }),
                Event::ChatMessage {
                    channel: ChatChannel::System,
                    ..
                }
            ] if channel == "Lore"
        ),
        "{seen:?}"
    );
    // S6: refused at minor 2 too, nothing sent.
    for c in [
        Command::Chat {
            channel: ChatChannel::Fellowship,
            text: "hi".into(),
        },
        Command::Social(SocialCommand::ReadyPerformance {
            tune: "Reel".into(),
        }),
        Command::Social(SocialCommand::StartPerformance),
        Command::Social(SocialCommand::DisbandFellowship),
        Command::Social(SocialCommand::InviteToFellowship {
            name: "Brin".into(),
        }),
    ] {
        assert!(
            matches!(s.send(Instant::now(), c), Err(NetError::InvalidCommand(_))),
            "refused"
        );
    }
    s.send(
        Instant::now(),
        Command::Chat {
            channel: ChatChannel::Say,
            text: "marker".into(),
        },
    )
    .unwrap();
    wait_marker(&mut s);
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

/// S5: at minor 1 (and at minor 2 without `CHAT_CHANNELS`) the three channel commands are
/// refused and nothing is sent.
#[test]
fn s5_channels_refused_below_minor_2_or_without_the_capability() {
    for (minor, caps) in [(1, ServerCaps::CHAT_CHANNELS), (2, ServerCaps::NONE)] {
        let peer = spawn(PeerOptions::default(), move |mut p| async move {
            greet_and_enter(&mut p, minor, caps).await;
            let Message::ChatSend(c) = p.expect(MessageType::ChatSend).await else {
                unreachable!()
            };
            assert_eq!((c.channel, c.text.as_str()), (Wire::Say, "marker"));
            echo_marker(&mut p).await;
            p.linger().await;
        });
        let mut s = connect(peer.config());
        enter(&mut s);
        assert!(!backend(&s).has_chat_channels());
        for c in [
            lore("Lore"),
            Command::Social(SocialCommand::LeaveChannel {
                name: "Lore".into(),
            }),
            Command::Chat {
                channel: ChatChannel::Named {
                    name: "Lore".into(),
                },
                text: "hi".into(),
            },
        ] {
            assert!(
                matches!(s.send(Instant::now(), c), Err(NetError::InvalidCommand(_))),
                "minor {minor}"
            );
        }
        s.send(
            Instant::now(),
            Command::Chat {
                channel: ChatChannel::Say,
                text: "marker".into(),
            },
        )
        .unwrap();
        wait_marker(&mut s);
        s.send(Instant::now(), Command::Disconnect).unwrap();
        pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
        peer.join();
    }
}

/// Minor-2 server messages at negotiated minor 1 (`tessera-protocol.md` §13.1, decision
/// 0035 limits): an unsolicited `ChannelJoined`, and a channel-6 `ChatMessage`, end the
/// session with a protocol violation and surface no Social event or chat line.
#[test]
fn minor_2_server_frames_at_minor_1_are_a_protocol_violation() {
    let frames = [
        frame_to_vec_minor(
            &Message::ChannelJoined {
                name: "Lore".into(),
            },
            2,
        )
        .unwrap(),
        frame_to_vec_minor(
            &Message::ChatMessage(ChatMessage {
                server_time: 1,
                channel: Wire::Named {
                    name: "Lore".into(),
                },
                echo: false,
                sender_entity: 42,
                sender: "Brin".into(),
                text: "hi".into(),
            }),
            2,
        )
        .unwrap(),
    ];
    for bytes in frames {
        let peer = spawn(PeerOptions::default(), move |mut p| async move {
            greet_and_enter(&mut p, 1, ServerCaps::CHAT_CHANNELS).await;
            // After the client's marker, so the client is in the world.
            p.expect(MessageType::ChatSend).await;
            p.chat_raw(&bytes).await;
            p.linger().await;
        });
        let mut s = connect(peer.config());
        enter(&mut s);
        s.send(
            Instant::now(),
            Command::Chat {
                channel: ChatChannel::Say,
                text: "marker".into(),
            },
        )
        .unwrap();
        let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Social(_) | Event::ChatMessage { .. })),
            "{events:?}"
        );
        match events.last() {
            Some(Event::Disconnected {
                reason: DisconnectReason::TransportFailed(t),
            }) => assert!(t.contains("protocol violation"), "{t}"),
            other => panic!("expected a violation, got {other:?}"),
        }
        assert!(pump_for(&mut s, std::time::Duration::from_millis(50)).is_empty());
        peer.join();
    }
}
