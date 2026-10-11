// SPDX-License-Identifier: GPL-3.0-or-later
//! Local selection on the test-server backend (`scenario-feed.md` §9.7 revision 5, decision
//! 0032 D1): the cases R1–R10 of §14.11, overlay case O8 of §14.9 and invariant 11 of §15.
//!
//! The "test double of the server" is the scripted inbound side of [`InboundHarness`],
//! which runs the same command routing and main-thread mapper as `RemoteBackend`; the
//! messages the client sent are its [`InboundHarness::sent`] record. One harness input is
//! played per poll, so "before the next poll the double sends X" is the input played in
//! that poll (received, then mapped after the local answer).
//!
//! The streamed static the spec calls "static 900" gets its ID from the backend's local
//! range (statics are never replicated, `tessera-protocol.md` §9.1); the tests use the ID
//! its `StaticSpawned` carried.

#![cfg(feature = "remote")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::domain::Ref;
use tessera_net::remote::{HarnessInput, InboundHarness};
use tessera_net::sandbox::{Lookup, StaticEntity, WorldSource};
use tessera_net::{
    ChatChannel, CombatCommand, CombatEvent, Command, DataId, Destination, EntityId, Event,
    Landblock, Location, NetError, Placement, RegionId, Rotation, Session, SessionState,
};
use tessera_protocol::frame::frame_to_vec_minor;
use tessera_protocol::message::{
    DestroyReason, EntityCreate, EntityDestroy, EntityKind, HelloAck, LocationCause as Cause,
    LocationReport, LoginOk, Message, ServerCaps, WorldEnter,
};
use tessera_protocol::types::{
    Appearance, Body, CharacterSummary, Location as WLoc, Look, Motion, Pose, Vec3,
};

const PLAYER: EntityId = EntityId(7);
const WANDERER: EntityId = EntityId(43);
const OTHER: EntityId = EntityId(44);

fn start() -> Location {
    Location::new(RegionId(1), 20_250.5, 22_500.25, 401.0)
}

/// One static object in the player's start block, none elsewhere.
#[derive(Debug)]
struct OneStatic;

impl WorldSource for OneStatic {
    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        if Some(block) != Landblock::containing(&start()) {
            return Lookup::Unavailable("empty".into());
        }
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

struct Rig {
    minor: u16,
    session: Session,
    /// Every event so far, in order.
    events: Vec<Event>,
    /// The streamed static's ID.
    stat: EntityId,
    /// Client messages recorded when the setup ended.
    sent_before: usize,
}

fn frame(m: &Message, minor: u16) -> Vec<u8> {
    frame_to_vec_minor(m, minor).unwrap()
}

fn world(m: &Message, minor: u16) -> HarnessInput {
    HarnessInput::Stream {
        world: true,
        data: frame(m, minor),
    }
}

fn create(id: u64, kind: EntityKind, name: &str) -> Message {
    Message::EntityCreate(EntityCreate {
        id,
        kind,
        name: name.into(),
        class: None,
        server_time: 5_100_000,
        pose: Pose {
            location: WLoc::new(1, 20_252.0, 22_501.0, 401.0),
            heading: 0.0,
            anchor: None,
        },
        velocity: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        motion: Motion::STANDING,
        appearance: Appearance::new(Body::Character {
            species: 81,
            sex: 2,
        }),
        properties: Vec::new(),
        look: Look::none(),
    })
}

/// A `Message::EntityDestroy` for `id`.
pub fn destroy(id: u64) -> Message {
    Message::EntityDestroy(EntityDestroy {
        id,
        reason: DestroyReason::Removed,
    })
}

/// A far teleport into region 2 (the statics around the old position are released).
fn far_teleport() -> Message {
    Message::LocationReport(LocationReport {
        cause: Cause::Teleported,
        request_id: 0,
        epoch: 2,
        server_time: 6_000_000,
        pose: Pose {
            location: WLoc::new(2, 500.0, 500.0, 0.0),
            heading: 0.0,
            anchor: None,
        },
    })
}

impl Rig {
    /// In the world as player 7, with entities 43 and 44 created and the static spawned, each
    /// in an earlier poll than the case's first command; `then` is played one per poll after.
    fn new(minor: u16, then: Vec<Message>) -> Self {
        let hello = Message::HelloAck(HelloAck {
            minor,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "double".into(),
            motd: String::new(),
        });
        let login = Message::LoginOk(LoginOk {
            account: "fuzz".into(),
            characters: vec![CharacterSummary {
                name: "Aldith".into(),
                body: Body::Character { species: 1, sex: 1 },
            }],
            looks: Vec::new(),
        });
        let enter = Message::WorldEnter(WorldEnter {
            player: PLAYER.0,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: 5_000_000,
            pose: Pose {
                location: WLoc::new(1, 20_250.5, 22_500.25, 401.0),
                heading: 0.0,
                anchor: None,
            },
            appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
            look: Look::none(),
        });
        let mut inputs = vec![
            HarnessInput::Control(frame(&hello, 0)),
            HarnessInput::Control(frame(&login, minor)),
            HarnessInput::OpenStream(0x43),
            HarnessInput::OpenStream(0x57),
            world(&enter, minor),
            world(
                &create(WANDERER.0, EntityKind::NonPlayer, "Wanderer 1"),
                minor,
            ),
            world(&create(OTHER.0, EntityKind::Player, "Bram"), minor),
            HarnessInput::Advance(Duration::ZERO),
        ];
        let setup = inputs.len();
        inputs.extend(then.iter().map(|m| world(m, minor)));
        let harness = InboundHarness::with_world(inputs, OneStatic);
        let mut rig = Self {
            minor,
            session: Session::new(harness),
            events: Vec::new(),
            stat: EntityId(0),
            sent_before: 0,
        };
        rig.send(Command::Connect).unwrap();
        for _ in 0..3 {
            rig.poll();
        }
        rig.send(Command::EnterWorld {
            character: "Aldith".into(),
        })
        .unwrap();
        for _ in 3..setup {
            rig.poll();
        }
        assert_eq!(rig.session.state(), SessionState::InWorld);
        assert_eq!(rig.harness().negotiated_minor(), Some(minor));
        rig.stat = rig
            .events
            .iter()
            .find_map(|e| match e {
                Event::StaticSpawned { id, tag: 900, .. } => Some(*id),
                _ => None,
            })
            .expect("the static was streamed");
        for id in [WANDERER, OTHER] {
            assert!(
                rig.events
                    .iter()
                    .any(|e| matches!(e, Event::EntityCreated { id: i, .. } if *i == id))
            );
        }
        rig.sent_before = rig.counted_sent();
        rig
    }

    fn harness(&self) -> &InboundHarness {
        self.session.backend::<InboundHarness>().unwrap()
    }

    fn send(&mut self, c: Command) -> Result<(), NetError> {
        self.session.send(Instant::now(), c)
    }

    fn select(&mut self, target: Option<EntityId>) -> Result<(), NetError> {
        self.send(Command::Combat(CombatCommand::SelectTarget { target }))
    }

    /// One poll; returns its events (also kept in `events`).
    fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        self.session.poll_into(Instant::now(), &mut out);
        self.events.extend(out.iter().cloned());
        out
    }

    /// Client messages, moves, heartbeats and clock samples not counted (§14.11).
    fn counted_sent(&self) -> usize {
        self.harness()
            .sent()
            .filter(|m| {
                !matches!(
                    m,
                    Message::MoveInput(_)
                        | Message::TimeRequest { .. }
                        | Message::Ping { .. }
                        | Message::Pong { .. }
                )
            })
            .count()
    }

    fn assert_no_message(&self) {
        assert_eq!(
            self.counted_sent(),
            self.sent_before,
            "minor {}",
            self.minor
        );
    }
}

fn targets(events: &[Event]) -> Vec<(EntityId, Option<EntityId>)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Combat(CombatEvent::TargetChanged { entity, target }) => {
                Some((*entity, *target))
            }
            _ => None,
        })
        .collect()
}

fn changed(target: Option<EntityId>) -> Event {
    Event::Combat(CombatEvent::TargetChanged {
        entity: PLAYER,
        target,
    })
}

fn r1(minor: u16) {
    let mut r = Rig::new(minor, Vec::new());
    r.select(Some(WANDERER)).unwrap();
    let next = r.poll();
    assert_eq!(next.first(), Some(&changed(Some(WANDERER))));
    assert_eq!(targets(&next).len(), 1);
    r.assert_no_message();
}

#[test]
fn r1_known_remote_entity() {
    r1(1);
}

#[test]
fn r2_none() {
    let mut r = Rig::new(1, Vec::new());
    r.select(None).unwrap();
    assert_eq!(r.poll().first(), Some(&changed(None)));
    r.assert_no_message();
}

#[test]
fn r3_the_player() {
    let mut r = Rig::new(1, Vec::new());
    r.select(Some(PLAYER)).unwrap();
    assert_eq!(r.poll().first(), Some(&changed(Some(PLAYER))));
    r.assert_no_message();
}

#[test]
fn r4_never_created() {
    let mut r = Rig::new(1, Vec::new());
    r.select(Some(EntityId(99))).unwrap();
    assert!(targets(&r.poll()).is_empty());
    assert!(targets(&r.poll()).is_empty());
    r.assert_no_message();
}

#[test]
fn r5_answer_before_the_removal_received_meanwhile() {
    let mut r = Rig::new(1, vec![destroy(WANDERER.0)]);
    r.select(Some(WANDERER)).unwrap();
    let next = r.poll();
    assert_eq!(
        next,
        vec![
            changed(Some(WANDERER)),
            Event::EntityDespawned { id: WANDERER }
        ]
    );
    for _ in 0..3 {
        assert!(targets(&r.poll()).is_empty());
    }
    r.assert_no_message();
}

#[test]
fn r6_removed_before_the_command() {
    let mut r = Rig::new(1, vec![destroy(OTHER.0)]);
    let out = r.poll();
    assert!(out.contains(&Event::EntityDespawned { id: OTHER }));
    r.select(Some(OTHER)).unwrap();
    assert!(targets(&r.poll()).is_empty());
    r.assert_no_message();
}

#[test]
fn r6_interest_reset_counts_as_a_removal() {
    let mut r = Rig::new(1, vec![Message::InterestReset]);
    let out = r.poll();
    assert!(out.contains(&Event::EntityDespawned { id: WANDERER }));
    // §9.7 item 5: the reset itself causes no `TargetChanged`.
    assert!(targets(&out).is_empty(), "{out:#?}");
    r.select(Some(WANDERER)).unwrap();
    assert!(targets(&r.poll()).is_empty());
    // The player stays selectable.
    r.select(Some(PLAYER)).unwrap();
    assert_eq!(targets(&r.poll()), vec![(PLAYER, Some(PLAYER))]);
}

#[test]
fn r7_only_the_latest_known_answer() {
    let mut r = Rig::new(1, Vec::new());
    r.select(Some(WANDERER)).unwrap();
    r.select(Some(OTHER)).unwrap();
    r.select(Some(EntityId(99))).unwrap();
    let next = r.poll();
    assert_eq!(targets(&next), vec![(PLAYER, Some(OTHER))]);
    assert_eq!(next.first(), Some(&changed(Some(OTHER))));
    assert!(targets(&r.poll()).is_empty());
    r.assert_no_message();
}

#[test]
fn r8_statics_until_cleared() {
    // A region change needs `InterestReset` first (`tessera-protocol.md` §9.5).
    let mut r = Rig::new(1, vec![Message::InterestReset, far_teleport()]);
    let stat = r.stat;
    r.select(Some(stat)).unwrap();
    let next = r.poll(); // also plays the InterestReset: the static stays
    assert_eq!(next.first(), Some(&changed(Some(stat))));
    let next = r.poll(); // the far teleport: the statics go
    assert!(next.contains(&Event::StaticsCleared), "{next:#?}");
    // §9.7 item 5: no selection was pending, and the teleport causes no `TargetChanged`.
    assert!(targets(&next).is_empty(), "{next:#?}");
    r.select(Some(stat)).unwrap();
    assert!(targets(&r.poll()).is_empty());
    r.assert_no_message();
}

#[test]
fn r9_minor_0() {
    r1(0);
}

#[test]
fn r10_states_and_other_commands() {
    // Before world entry: the session's state rule.
    let mut s = Session::new(InboundHarness::new(Vec::new()));
    s.send(Instant::now(), Command::Connect).unwrap();
    assert!(matches!(
        s.send(
            Instant::now(),
            Command::Combat(CombatCommand::SelectTarget { target: None })
        ),
        Err(NetError::InvalidState { .. })
    ));
    let mut r = Rig::new(1, Vec::new());
    for c in [
        CombatCommand::UseSkill {
            what: Ref::Literal("x".into()),
            target: Some(WANDERER),
        },
        CombatCommand::CancelCast,
        CombatCommand::SetQuickslot {
            slot: 1,
            content: None,
        },
    ] {
        assert!(matches!(
            r.send(Command::Combat(c)),
            Err(NetError::InvalidCommand(_))
        ));
    }
    assert!(targets(&r.poll()).is_empty());
    r.assert_no_message();
}

/// §15 invariant 11 over a longer mixed run: every `TargetChanged` answers a command, names
/// the player and lies between its target's creation and removal; no message is caused.
/// The removal, `InterestReset` and far teleport polls have no selection before them, so
/// any `TargetChanged` there would be one the backend emitted on its own (§9.7 item 5).
#[test]
fn invariant_11() {
    let mut r = Rig::new(
        1,
        vec![
            destroy(WANDERER.0),
            create(WANDERER.0, EntityKind::NonPlayer, "Wanderer 1"),
            Message::InterestReset,
            far_teleport(),
        ],
    );
    let stat = r.stat;
    let mut asked = 0;
    let mut rotation = [Some(WANDERER), Some(OTHER), Some(stat), None, Some(PLAYER)]
        .into_iter()
        .cycle();
    for step in 0..10 {
        // Polls 0, 2 and 3 play the destroy, the reset and the teleport.
        let picks: Vec<Option<EntityId>> = match step {
            0 | 2 | 3 | 7 => Vec::new(),
            1 => vec![Some(WANDERER), Some(stat)],
            5 => rotation.by_ref().take(3).collect(),
            _ => rotation.next().into_iter().collect(),
        };
        for &t in &picks {
            r.select(t).unwrap();
            asked += 1;
        }
        let out = r.poll();
        let n = targets(&out).len();
        if picks.is_empty() {
            assert_eq!(n, 0, "step {step}: an answer without a command: {out:#?}");
        } else {
            assert!(n <= 1, "step {step}: more than one answer: {out:#?}");
            if n == 1 {
                assert!(
                    matches!(
                        out.first(),
                        Some(Event::Combat(CombatEvent::TargetChanged { .. }))
                    ),
                    "step {step}: the answer is not first: {out:#?}"
                );
            }
        }
    }
    let mut live = std::collections::BTreeSet::from([PLAYER]);
    let mut statics = std::collections::BTreeSet::new();
    let mut answers = 0;
    for e in &r.events {
        match e {
            Event::EntityCreated { id, .. } => {
                live.insert(*id);
            }
            Event::StaticSpawned { id, .. } => {
                live.insert(*id);
                statics.insert(*id);
            }
            Event::EntityDespawned { id } => {
                live.remove(id);
            }
            Event::StaticsCleared => {
                for id in std::mem::take(&mut statics) {
                    live.remove(&id);
                }
            }
            Event::Combat(CombatEvent::TargetChanged { entity, target }) => {
                answers += 1;
                assert_eq!(*entity, PLAYER);
                if let Some(t) = target {
                    assert!(live.contains(t), "{t:?} selected while not live");
                }
            }
            _ => {}
        }
    }
    assert!(answers > 0 && answers <= asked);
    assert!(r.events.contains(&Event::StaticsCleared));
    r.assert_no_message();
}

// --- §14.9 O8: overlay routing over the test-server backend -------------------------------

#[cfg(feature = "scenario")]
#[test]
fn o8_overlay_routes_selection_to_the_local_answer() {
    use tessera_net::scenario::{EmptyResolver, OverlayBackend, Scenario};

    // A file inside the overlay subset; it can hold no trigger on `target` (§12.5.1), and the
    // overlay routes `SelectTarget` past the feed (§12.5.2).
    let scenario = Scenario::parse(b"tessera-scenario 1\ntick 100ms\n").unwrap();
    scenario.check_overlay().unwrap();
    let resolved = Arc::new(scenario.resolve(&mut EmptyResolver).unwrap());
    let hello = Message::HelloAck(HelloAck {
        minor: 1,
        server_caps: ServerCaps::NONE,
        tick_rate: 30,
        snapshot_rate: 15,
        interest_radius: 2,
        server_name: "double".into(),
        motd: String::new(),
    });
    let login = Message::LoginOk(LoginOk {
        account: "fuzz".into(),
        characters: vec![CharacterSummary {
            name: "Aldith".into(),
            body: Body::Character { species: 1, sex: 1 },
        }],
        looks: Vec::new(),
    });
    let enter = Message::WorldEnter(WorldEnter {
        player: PLAYER.0,
        name: "Aldith".into(),
        instance: 1,
        epoch: 1,
        server_time: 5_000_000,
        pose: Pose {
            location: WLoc::new(1, 20_250.5, 22_500.25, 401.0),
            heading: 0.0,
            anchor: None,
        },
        appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
        look: Look::none(),
    });
    let inputs = vec![
        HarnessInput::Control(frame(&hello, 0)),
        HarnessInput::Control(frame(&login, 1)),
        HarnessInput::OpenStream(0x43),
        HarnessInput::OpenStream(0x57),
        world(&enter, 1),
        HarnessInput::Advance(Duration::ZERO),
        HarnessInput::Advance(Duration::ZERO),
        HarnessInput::Advance(Duration::ZERO),
    ];
    let mut s = Session::new(OverlayBackend::new(InboundHarness::new(inputs), resolved));
    let now = Instant::now();
    let mut events = Vec::new();
    s.send(now, Command::Connect).unwrap();
    for _ in 0..3 {
        s.poll_into(now, &mut events);
    }
    s.send(
        now,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    for _ in 0..3 {
        s.poll_into(now, &mut events);
    }
    assert_eq!(s.state(), SessionState::InWorld);
    let server = |s: &Session| {
        s.backend::<OverlayBackend<InboundHarness>>()
            .unwrap()
            .server()
            .sent()
            .filter(|m| {
                !matches!(
                    m,
                    Message::MoveInput(_) | Message::TimeRequest { .. } | Message::Ping { .. }
                )
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    let before = server(&s).len();
    s.send(
        now,
        Command::Chat {
            channel: ChatChannel::Say,
            text: "hi".into(),
        },
    )
    .unwrap();
    s.send(
        now,
        Command::Teleport {
            destination: Destination::Named("Bree".into()),
        },
    )
    .unwrap();
    s.send(
        now,
        Command::Combat(CombatCommand::SelectTarget { target: None }),
    )
    .unwrap();
    assert!(matches!(
        s.send(
            now,
            Command::Combat(CombatCommand::UseSkill {
                what: Ref::Literal("x".into()),
                target: None,
            })
        ),
        Err(NetError::InvalidCommand(_))
    ));
    let sent = server(&s);
    let new = &sent[before..];
    assert_eq!(new.len(), 2, "{new:#?}");
    assert!(matches!(new[0], Message::ChatSend(_)));
    assert!(matches!(new[1], Message::TeleportRequest(_)));
    let mut next = Vec::new();
    s.poll_into(now, &mut next);
    assert_eq!(next.first(), Some(&changed(None)), "{next:#?}");
}
