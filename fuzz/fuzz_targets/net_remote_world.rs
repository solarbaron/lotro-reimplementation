// SPDX-License-Identifier: GPL-3.0-or-later
//! The test-server backend in the world: after a valid greeting, login and `WorldEnter`,
//! arbitrary world-stream, chat-stream and datagram bytes (`tessera-protocol.md` §6.3–§6.5,
//! §13), with well-formed creations, removals, interest resets and teleports among them so
//! entities really come and go, statics streamed from a small synthetic world source, and
//! local selections in between (`scenario-feed.md` §9.7, revision 5). A selection names an
//! entity the client has seen (by its order of appearance) or a raw small ID (the player is
//! 42). Must never panic, never surface an entity event for an entity it did not create, end
//! with at most one `Disconnected`, and keep invariant 11 of `scenario-feed.md` §15: every
//! `TargetChanged` names the player and a target that is live at that point of the stream, a
//! poll carries at most one and only when a selection was accepted before it, and no
//! selection sends a message.

#![no_main]

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::remote::{HarnessInput, InboundHarness};
use tessera_net::sandbox::{Landblock, Lookup, StaticEntity, WorldSource};
use tessera_net::{
    CombatCommand, CombatEvent, Command, DataId, EntityId, Event, Placement, Rotation, Session,
};
use tessera_protocol::frame::frame_to_vec;
use tessera_protocol::message::{
    DestroyReason, EntityCreate, EntityDestroy, EntityKind, HelloAck, LocationCause,
    LocationReport, LoginOk, Message, ServerCaps, WorldEnter,
};
use tessera_protocol::types::{
    Appearance, Body, CharacterSummary, Location, Look, Motion, Pose, Vec3,
};

const MAX_OPS: usize = 256;

#[derive(Debug, Arbitrary)]
enum Op {
    World(Vec<u8>),
    Chat(Vec<u8>),
    Datagram(Vec<u8>),
    Advance(u8),
    /// A valid `EntityCreate` for this ID (0 becomes 1, the wire has no ID 0) on the world
    /// stream.
    Create(u8),
    /// A valid `EntityDestroy` for this ID (0 becomes 1) on the world stream.
    Destroy(u8),
    /// `InterestReset` on the world stream.
    Reset,
    /// A teleport report with the next epoch, far (another region) or near.
    Teleport(bool),
    /// `SelectTarget` before the next poll, or none.
    Select(Option<Pick>),
}

#[derive(Debug, Clone, Copy, Arbitrary)]
enum Pick {
    /// The n-th entity (modulo the count) of those created or spawned so far; the raw ID
    /// when none was.
    Seen(u8),
    /// A raw small ID.
    Raw(u8),
}

/// One static object in every block, so statics stream around wherever the player is.
#[derive(Debug)]
struct EveryBlock;

impl WorldSource for EveryBlock {
    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        if limit > 0 {
            out.push(StaticEntity::new(
                DataId(0x0100_0900),
                Placement::new(block.location([10.0, 10.0, 0.0]), Rotation::IDENTITY, 1.0),
                900,
            ));
        }
        Lookup::Ready(())
    }
}

fn create(id: u8) -> Message {
    Message::EntityCreate(EntityCreate {
        id: u64::from(id).max(1),
        kind: EntityKind::NonPlayer,
        name: "Wanderer".into(),
        class: None,
        server_time: 5_100_000,
        pose: Pose {
            location: Location::new(1, 20_252.0, 22_501.0, 401.0),
            heading: 0.0,
            anchor: None,
        },
        velocity: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        motion: Motion::STANDING,
        appearance: Appearance::new(Body::Character { species: 1, sex: 2 }),
        properties: Vec::new(),
        look: Look::none(),
    })
}

fn teleport(epoch: u32, far: bool) -> Message {
    let location = if far {
        Location::new(2, 500.0, 500.0, 0.0)
    } else {
        Location::new(1, 20_260.0, 22_510.0, 401.0)
    };
    Message::LocationReport(LocationReport {
        cause: LocationCause::Teleported,
        request_id: 0,
        epoch,
        server_time: 6_000_000,
        pose: Pose {
            location,
            heading: 0.0,
            anchor: None,
        },
    })
}

/// One poll; checks that its `TargetChanged` events answer a selection accepted before it.
fn poll(session: &mut Session, now: Instant, events: &mut Vec<Event>, selected: bool) {
    let before = events.len();
    session.poll_into(now, events);
    let answers = events[before..]
        .iter()
        .filter(|e| matches!(e, Event::Combat(CombatEvent::TargetChanged { .. })))
        .count();
    assert!(
        answers <= usize::from(selected),
        "unasked or repeated answer"
    );
}

fn frame(m: Message) -> Vec<u8> {
    frame_to_vec(&m).unwrap_or_else(|e| panic!("{e:?}: {m:?}"))
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
    let setup = inputs.len();
    // (inputs played before the command, pick).
    let mut selects = Vec::new();
    let mut epoch = 1;
    let world = |m: Message| HarnessInput::Stream {
        world: true,
        data: frame(m),
    };
    for op in ops.into_iter().take(MAX_OPS) {
        inputs.push(match op {
            Op::World(data) => HarnessInput::Stream { world: true, data },
            Op::Chat(data) => HarnessInput::Stream { world: false, data },
            Op::Datagram(d) => HarnessInput::Datagram(d),
            Op::Advance(ms) => HarnessInput::Advance(Duration::from_millis(u64::from(ms) * 4)),
            Op::Create(id) => world(create(id)),
            Op::Destroy(id) => world(Message::EntityDestroy(EntityDestroy {
                id: u64::from(id).max(1),
                reason: DestroyReason::Removed,
            })),
            Op::Reset => world(Message::InterestReset),
            Op::Teleport(far) => {
                epoch += 1;
                world(teleport(epoch, far))
            }
            Op::Select(t) => {
                selects.push((inputs.len().saturating_sub(setup), t));
                continue;
            }
        });
    }
    let polls = inputs.len() + 2;
    let mut session = Session::new(InboundHarness::with_world(inputs, EveryBlock));
    let now = Instant::now();
    session.send(now, Command::Connect).expect("connect");
    let mut events = Vec::new();
    for _ in 0..3 {
        poll(&mut session, now, &mut events, false);
    }
    let _ = session.send(
        now,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    );
    // Two polls play the stream opens and WorldEnter; then one input per poll.
    for _ in 0..2 {
        poll(&mut session, now, &mut events, false);
    }
    let mut selects = selects.into_iter().peekable();
    for step in 0..polls {
        let mut selected = false;
        while let Some((_, pick)) = selects.next_if(|(i, _)| *i <= step) {
            let target = pick.map(|p| resolve(p, &events));
            // Refused outside InWorld (the stream may have ended the session).
            selected |= session
                .send(now, Command::Combat(CombatCommand::SelectTarget { target }))
                .is_ok();
        }
        poll(&mut session, now, &mut events, selected);
    }
    // Invariant 11: no message other than login, world entry, moves, clock and ping traffic.
    let harness = session.backend::<InboundHarness>().expect("harness");
    assert!(harness.sent().all(|m| matches!(
        m,
        Message::Hello(_)
            | Message::Login(_)
            | Message::EnterWorld(_)
            | Message::MoveInput(_)
            | Message::TimeRequest { .. }
            | Message::Ping { .. }
            | Message::Pong { .. }
            | Message::Disconnect(_)
    )));
    let mut live = BTreeSet::new();
    let mut statics = BTreeSet::new();
    let mut player = None;
    for e in &events {
        match e {
            Event::WorldEntered { player: p, .. } => player = Some(*p),
            Event::EntityCreated { id, tag, .. } => {
                assert!(live.insert(*id));
                if tag.is_some() {
                    statics.insert(*id);
                }
            }
            Event::StaticSpawned { id, .. } => {
                assert!(live.insert(*id));
                statics.insert(*id);
            }
            Event::EntityDespawned { id } => {
                assert!(live.remove(id));
                statics.remove(id);
            }
            Event::StaticsCleared => {
                for id in std::mem::take(&mut statics) {
                    live.remove(&id);
                }
            }
            Event::EntityMoved { id, .. } | Event::AppearanceChanged { id, .. } => {
                assert!(live.contains(id) || Some(*id) == player);
            }
            Event::Combat(CombatEvent::TargetChanged { entity, target }) => {
                assert_eq!(Some(*entity), player);
                assert!(target.is_none_or(|t| live.contains(&t) || Some(t) == player));
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

/// The entity a [`Pick`] names, given the events so far.
fn resolve(pick: Pick, events: &[Event]) -> EntityId {
    match pick {
        Pick::Raw(n) => EntityId(u64::from(n)),
        Pick::Seen(n) => {
            let seen = events.iter().filter_map(|e| match e {
                Event::EntityCreated { id, .. } | Event::StaticSpawned { id, .. } => Some(*id),
                _ => None,
            });
            let count = seen.clone().count();
            if count == 0 {
                EntityId(u64::from(n))
            } else {
                seen.clone()
                    .nth(usize::from(n) % count)
                    .unwrap_or(EntityId(u64::from(n)))
            }
        }
    }
}
