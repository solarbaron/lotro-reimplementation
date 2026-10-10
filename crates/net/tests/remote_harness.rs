// SPDX-License-Identifier: GPL-3.0-or-later
//! The test-server backend's inbound pipeline without a network ([`InboundHarness`]):
//! deterministic scripts and property tests over hostile bytes. The same driver and
//! mapper run in the real backend.

#![cfg(feature = "remote")]

use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_net::remote::{HarnessInput, InboundHarness};
use tessera_net::{Command, EntityId, Event, LocationCause, Session};
use tessera_protocol::frame::{encode_datagram, frame_to_vec};
use tessera_protocol::message::{
    HelloAck, LoginOk, Message, MessageType, MotionBatch, MotionEntry, ServerCaps, WorldEnter,
};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location, Motion, Pose, Vec3};

fn frame(m: Message) -> Vec<u8> {
    frame_to_vec(&m).unwrap()
}

fn greeting() -> Vec<HarnessInput> {
    vec![
        HarnessInput::Control(frame(Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "h".into(),
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
    ]
}

fn world_enter() -> Vec<u8> {
    frame(Message::WorldEnter(WorldEnter {
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
    }))
}

fn run(s: &mut Session, polls: usize) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..polls {
        s.poll_into(Instant::now(), &mut out);
    }
    out
}

#[test]
fn scripted_login_world_entry_and_moves() {
    let mut s = Session::new(InboundHarness::new(greeting()));
    s.send(Instant::now(), Command::Connect).unwrap();
    let events = run(&mut s, 4);
    assert!(
        matches!(&events[0], Event::Connected { characters } if characters[0].name == "Aldith")
    );
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    // Push the world's side, then let a second pass of the clock run.
    let h = s.backend::<InboundHarness>().unwrap();
    assert_eq!(h.remaining(), 0);
    let mut s2 = Session::new(InboundHarness::new(
        greeting()
            .into_iter()
            .chain([
                HarnessInput::OpenStream(0x57),
                HarnessInput::Stream {
                    world: true,
                    data: world_enter(),
                },
                HarnessInput::Advance(Duration::from_millis(200)),
            ])
            .collect::<Vec<_>>(),
    ));
    s2.send(Instant::now(), Command::Connect).unwrap();
    let _ = run(&mut s2, 3);
    s2.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let events = run(&mut s2, 4);
    assert!(matches!(
        events[0],
        Event::WorldEntered {
            player: EntityId(42),
            ..
        }
    ));
    assert!(matches!(
        events[2],
        Event::LocationReport {
            cause: LocationCause::Entered,
            ..
        }
    ));
    let h = s2.backend::<InboundHarness>().unwrap();
    let types: Vec<MessageType> = h.sent().map(Message::message_type).collect();
    assert_eq!(
        &types[..3],
        [
            MessageType::Hello,
            MessageType::Login,
            MessageType::EnterWorld
        ]
    );
    assert!(types.contains(&MessageType::MoveInput), "{types:?}");
    assert!(types.contains(&MessageType::TimeRequest), "{types:?}");
}

#[test]
fn motion_for_created_entities_only() {
    let batch = |t: u64, id: u64| {
        HarnessInput::Datagram(
            encode_datagram(&Message::MotionBatch(MotionBatch {
                server_time: t,
                entries: vec![MotionEntry {
                    id,
                    position: Vec3 {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    },
                    heading: 0.5,
                    velocity: Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    motion: Motion::STANDING,
                }],
            }))
            .unwrap(),
        )
    };
    let mut inputs = greeting();
    inputs.extend([
        HarnessInput::OpenStream(0x57),
        HarnessInput::Stream {
            world: true,
            data: world_enter(),
        },
        batch(6_000_000, 43),
    ]);
    let mut s = Session::new(InboundHarness::new(inputs));
    s.send(Instant::now(), Command::Connect).unwrap();
    run(&mut s, 3);
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let events = run(&mut s, 4);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntityMoved { .. }))
    );
}

fn arb_input() -> impl Strategy<Value = HarnessInput> {
    let bytes = proptest::collection::vec(any::<u8>(), 0..200);
    prop_oneof![
        bytes.clone().prop_map(HarnessInput::Control),
        any::<u8>().prop_map(HarnessInput::OpenStream),
        (any::<bool>(), bytes.clone())
            .prop_map(|(world, data)| HarnessInput::Stream { world, data }),
        bytes.prop_map(HarnessInput::Datagram),
        (0u64..3_000).prop_map(|ms| HarnessInput::Advance(Duration::from_millis(ms))),
        (proptest::option::of(any::<bool>()), any::<bool>())
            .prop_map(|(stream, reset)| HarnessInput::StreamEnd { stream, reset }),
        Just(HarnessInput::IdleTimeout),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Hostile bytes after a valid greeting: never a panic, at most one Disconnected,
    /// and nothing after it.
    #[test]
    fn hostile_input_never_panics(inputs in proptest::collection::vec(arb_input(), 0..24)) {
        let mut all = greeting();
        all.push(HarnessInput::OpenStream(0x57));
        all.push(HarnessInput::Stream { world: true, data: world_enter() });
        all.extend(inputs);
        let n = all.len();
        let mut s = Session::new(InboundHarness::new(all));
        s.send(Instant::now(), Command::Connect).unwrap();
        let mut events = run(&mut s, 3);
        let _ = s.send(Instant::now(), Command::EnterWorld { character: "Aldith".into() });
        events.extend(run(&mut s, n + 2));
        let ends = events.iter().filter(|e| matches!(e, Event::Disconnected { .. })).count();
        prop_assert!(ends <= 1);
        if ends == 1 {
            let last_is_end = matches!(events.last(), Some(Event::Disconnected { .. }));
            prop_assert!(last_is_end);
        }
    }
}
