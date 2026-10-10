// SPDX-License-Identifier: GPL-3.0-or-later
//! The test-server backend in the world: after a valid greeting, login and `WorldEnter`,
//! arbitrary world-stream, chat-stream and datagram bytes (`tessera-protocol.md` §6.3–§6.5,
//! §13). Must never panic, never surface an entity event for an entity it did not create,
//! and end with at most one `Disconnected`.

#![no_main]

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::remote::{HarnessInput, InboundHarness};
use tessera_net::{Command, Event, Session};
use tessera_protocol::frame::frame_to_vec;
use tessera_protocol::message::{HelloAck, LoginOk, Message, ServerCaps, WorldEnter};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location, Pose};

const MAX_OPS: usize = 256;

#[derive(Debug, Arbitrary)]
enum Op {
    World(Vec<u8>),
    Chat(Vec<u8>),
    Datagram(Vec<u8>),
    Advance(u8),
}

fn frame(m: Message) -> Vec<u8> {
    frame_to_vec(&m).expect("valid message")
}

fuzz_target!(|ops: Vec<Op>| {
    let mut inputs = vec![
        HarnessInput::Control(frame(Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "fuzz".into(),
            motd: String::new(),
        }))),
        HarnessInput::Control(frame(Message::LoginOk(LoginOk {
            account: "fuzz".into(),
            characters: vec![CharacterSummary {
                name: "Aldith".into(),
                body: Body::Character { species: 1, sex: 1 },
            }],
            looks: Vec::new(),
        }))),
        HarnessInput::OpenStream(0x43),
        HarnessInput::OpenStream(0x57),
        HarnessInput::Stream {
            world: true,
            data: frame(Message::WorldEnter(WorldEnter {
                player: 42,
                name: "Aldith".into(),
                instance: 1,
                epoch: 1,
                server_time: 5_000_000,
                pose: Pose {
                    location: Location::new(1, 20_250.5, 22_500.25, 401.0),
                    heading: 0.0,
                    anchor: None,
                },
                appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                look: tessera_protocol::types::Look::none(),
            })),
        },
    ];
    for op in ops.into_iter().take(MAX_OPS) {
        inputs.push(match op {
            Op::World(data) => HarnessInput::Stream { world: true, data },
            Op::Chat(data) => HarnessInput::Stream { world: false, data },
            Op::Datagram(d) => HarnessInput::Datagram(d),
            Op::Advance(ms) => HarnessInput::Advance(Duration::from_millis(u64::from(ms) * 4)),
        });
    }
    let polls = inputs.len() + 2;
    let mut session = Session::new(InboundHarness::new(inputs));
    let now = Instant::now();
    session.send(now, Command::Connect).expect("connect");
    let mut events = Vec::new();
    for _ in 0..3 {
        session.poll_into(now, &mut events);
    }
    let _ = session.send(
        now,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    );
    for _ in 0..polls {
        session.poll_into(now, &mut events);
    }
    let mut live = BTreeSet::new();
    let mut player = None;
    for e in &events {
        match e {
            Event::WorldEntered { player: p, .. } => player = Some(*p),
            Event::EntityCreated { id, .. } => assert!(live.insert(*id)),
            Event::EntityDespawned { id } => assert!(live.remove(id)),
            Event::EntityMoved { id, .. } | Event::AppearanceChanged { id, .. } => {
                assert!(live.contains(id) || Some(*id) == player);
            }
            _ => {}
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
});
