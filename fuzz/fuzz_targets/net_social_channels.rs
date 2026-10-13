// SPDX-License-Identifier: GPL-3.0-or-later
//! Named chat channels of the test-server backend at protocol minor 2
//! (`tessera_net::remote`, `scenario-feed.md` §9.8, `tessera-protocol.md` §10.6 and
//! Appendix A), without a network. The fuzzer greets at a minor and capability set of its
//! choice, logs in, enters the world, and then mixes the client's channel commands (join,
//! leave, lines on named channels, the refused Social commands) with well-formed server
//! frames (`ChannelJoined`, `ChannelLeft`, `ChannelRefused`, `ChatMessage` on channel 6,
//! encoded at the negotiated minor or, when the fuzzer asks, at minor 2 whatever was
//! negotiated, so a minor-0/1 client receives real minor-2 frames: the hostile case of
//! `tessera-protocol.md` §13.1) and raw bytes, through the same driver, mapper and
//! `Session` checks as the real backend. Must never panic; at most one `Disconnected`, and
//! nothing after it; channel commands are sent only at minor ≥ 2 with `CHAT_CHANNELS`.

#![no_main]

use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::domain::SocialCommand;
use tessera_net::remote::{HarnessInput, InboundHarness, ServerCaps};
use tessera_net::{ChatChannel, Command, Event, Session};
use tessera_protocol::codes::ChannelRefusedCode;
use tessera_protocol::frame::frame_to_vec_minor;
use tessera_protocol::message::{
    ChannelLeft, ChannelLeftReason, ChannelRefused, ChatChannel as Wire, ChatMessage, HelloAck,
    LoginOk, Message, MessageType, WorldEnter,
};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location, Look, Pose};

const MAX_OPS: usize = 256;

#[derive(Debug, Arbitrary)]
enum Op {
    // Server frames, encoded at the negotiated minor (or at least minor 2, see
    // `Input::ahead`).
    Joined(String),
    Left {
        name: String,
        removed: bool,
    },
    Refused {
        code: u16,
        name: String,
        text: String,
    },
    Line {
        name: String,
        text: String,
        echo: bool,
    },
    // Raw bytes on the chat stream (hostile).
    RawChat(Vec<u8>),
    Advance(u16),
    // Client commands.
    Join(String),
    Leave(String),
    Say {
        name: String,
        text: String,
    },
    Fellowship(String),
    Ready(String),
    Start,
    Disband,
    Disconnect,
}

#[derive(Debug, Arbitrary)]
struct Input {
    minor: u8,
    caps: u32,
    /// Encodes the channel frames at `max(minor, 2)`: below minor 2 they are a protocol
    /// violation the client must end the session on.
    ahead: bool,
    ops: Vec<Op>,
}

fn frame(m: &Message, minor: u16) -> Vec<u8> {
    frame_to_vec_minor(m, minor).unwrap_or_default()
}

fuzz_target!(|input: Input| {
    // Minors 0–3: below, at and above the one that adds channels.
    let minor = u16::from(input.minor % 4);
    let caps = ServerCaps(input.caps);
    let mut inputs = vec![HarnessInput::Control(frame(
        &Message::HelloAck(HelloAck {
            minor,
            server_caps: caps,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Fuzz".into(),
            motd: String::new(),
        }),
        0,
    ))];
    inputs.push(HarnessInput::Control(frame(
        &Message::LoginOk(LoginOk {
            account: "fuzz".into(),
            characters: vec![CharacterSummary {
                name: "Aldith".into(),
                body: Body::Character {
                    species: 23,
                    sex: 1,
                },
            }],
            looks: if minor >= 1 {
                vec![Look::none()]
            } else {
                Vec::new()
            },
        }),
        minor,
    )));
    inputs.push(HarnessInput::OpenStream(0x43));
    inputs.push(HarnessInput::OpenStream(0x57));
    inputs.push(HarnessInput::Stream {
        world: true,
        data: frame(
            &Message::WorldEnter(WorldEnter {
                player: 42,
                name: "Aldith".into(),
                instance: 1,
                epoch: 1,
                server_time: 1_000_000,
                pose: Pose {
                    location: Location::new(1, 100.0, 100.0, 10.0),
                    heading: 0.0,
                    anchor: None,
                },
                appearance: Appearance::new(Body::Character {
                    species: 23,
                    sex: 1,
                }),
                look: Look::none(),
            }),
            minor,
        ),
    });
    let channel_minor = if input.ahead { minor.max(2) } else { minor };
    let mut commands = Vec::new();
    for (i, op) in input.ops.into_iter().take(MAX_OPS).enumerate() {
        let chat = |m: Message| HarnessInput::Stream {
            world: false,
            data: frame(&m, channel_minor),
        };
        let at = i + inputs.len();
        let input = match op {
            Op::Joined(name) => chat(Message::ChannelJoined { name }),
            Op::Left { name, removed } => chat(Message::ChannelLeft(ChannelLeft {
                name,
                reason: if removed {
                    ChannelLeftReason::Removed
                } else {
                    ChannelLeftReason::Left
                },
            })),
            Op::Refused { code, name, text } => chat(Message::ChannelRefused(ChannelRefused {
                code: ChannelRefusedCode::from_u16(code),
                name,
                text,
            })),
            Op::Line { name, text, echo } => chat(Message::ChatMessage(ChatMessage {
                server_time: 1_000_000,
                channel: Wire::Named { name },
                echo,
                sender_entity: 42,
                sender: "Brin".into(),
                text,
            })),
            Op::RawChat(data) => HarnessInput::Stream { world: false, data },
            Op::Advance(ms) => HarnessInput::Advance(Duration::from_millis(u64::from(ms))),
            Op::Join(name) => {
                commands.push((at, Command::Social(SocialCommand::JoinChannel { name })));
                continue;
            }
            Op::Leave(name) => {
                commands.push((at, Command::Social(SocialCommand::LeaveChannel { name })));
                continue;
            }
            Op::Say { name, text } => {
                commands.push((
                    at,
                    Command::Chat {
                        channel: ChatChannel::Named { name },
                        text,
                    },
                ));
                continue;
            }
            Op::Fellowship(text) => {
                commands.push((
                    at,
                    Command::Chat {
                        channel: ChatChannel::Fellowship,
                        text,
                    },
                ));
                continue;
            }
            Op::Ready(tune) => {
                commands.push((
                    at,
                    Command::Social(SocialCommand::ReadyPerformance { tune }),
                ));
                continue;
            }
            Op::Start => {
                commands.push((at, Command::Social(SocialCommand::StartPerformance)));
                continue;
            }
            Op::Disband => {
                commands.push((at, Command::Social(SocialCommand::DisbandFellowship)));
                continue;
            }
            Op::Disconnect => {
                commands.push((at, Command::Disconnect));
                continue;
            }
        };
        inputs.push(input);
    }
    let polls = inputs.len() + 2;
    let mut session = Session::new(InboundHarness::new(inputs));
    let now = Instant::now();
    let _ = session.send(now, Command::Connect);
    let mut events: Vec<Event> = Vec::new();
    let mut cmds = commands.into_iter().peekable();
    let mut entered = false;
    for step in 0..polls + MAX_OPS {
        if !entered && step == 3 {
            let _ = session.send(
                now,
                Command::EnterWorld {
                    character: "Aldith".into(),
                },
            );
            entered = true;
        }
        while let Some((_, c)) = cmds.next_if(|(i, _)| *i <= step) {
            // Session and the backend refuse what the state, the minor or the
            // capabilities do not allow; that is fine here.
            let _ = session.send(now, c);
        }
        session.poll_into(now, &mut events);
        if step >= polls && cmds.peek().is_none() {
            break;
        }
    }
    let ends = events
        .iter()
        .filter(|e| matches!(e, Event::Disconnected { .. }))
        .count();
    assert!(ends <= 1);
    if ends == 1 {
        assert!(matches!(events.last(), Some(Event::Disconnected { .. })));
    }
    // Invariant 13 of scenario-feed.md §15: channel messages leave the client only at
    // minor ≥ 2 with CHAT_CHANNELS; no Social command sends anything else.
    let harness = session.backend::<InboundHarness>().expect("harness");
    let channels = minor >= 2 && caps.contains(ServerCaps::CHAT_CHANNELS);
    for m in harness.sent() {
        match m.message_type() {
            MessageType::ChannelJoin | MessageType::ChannelLeave => assert!(channels),
            MessageType::ChatSend => {
                if let Message::ChatSend(c) = m {
                    assert!(channels || !matches!(c.channel, Wire::Named { .. }));
                }
            }
            _ => {}
        }
    }
});
