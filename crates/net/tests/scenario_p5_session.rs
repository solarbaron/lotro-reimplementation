// SPDX-License-Identifier: GPL-3.0-or-later
//! Session rules of the P5 travel revision (`docs/specs/scenario-feed.md` §9.5, §18.P5):
//! `TravelEnded`, `MountVitalChanged` and `InstanceQueueChanged` follow the entity rule,
//! invalid P5 values are dropped and literal refs cleaned, `LeaveQueue` needs `InWorld`, a
//! `JoinInstance` level must be 1 … 10,000, the new World forms play through the sandbox
//! with a scenario and commands sent by the client itself, and the test-server backend
//! refuses `LeaveQueue` (LEGAL.md §11.1 rule 5).
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::domain::{
    InstanceEntry, MAX_LITERAL_REF_LEN, MarkerId, MountVital, QueueState, Ref, RouteMode, Text,
    TravelEntry, TravelFlags, WindowId,
};
use tessera_net::model::{Location, RegionId};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, Scenario};
use tessera_net::{
    Backend, Command, EntityId, Event, NetError, Session, Transform, WorldCommand, WorldEvent,
};

/// A backend that replays a script, to check what `Session` lets through.
#[derive(Debug)]
struct Scripted(Vec<Vec<Event>>);

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }
    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if !self.0.is_empty() {
            out.extend(self.0.remove(0));
        }
    }
}

const W: WindowId = WindowId(3);
const P: EntityId = EntityId(1);

fn lit(s: &str) -> Ref {
    Ref::Literal(s.to_owned())
}

fn marker(note_type: Option<u32>) -> Event {
    Event::World(WorldEvent::MarkerAdded {
        marker: MarkerId(1),
        location: Location::new(RegionId(1), 1.0, 2.0, 0.0),
        label: Text::Literal("M".into()),
        icon: None,
        quest: None,
        note_type,
    })
}

fn travel(flags: u8, min_level: Option<u16>, area: Ref) -> Event {
    Event::World(WorldEvent::TravelShown {
        window: W,
        source: None,
        entries: vec![TravelEntry {
            entry: 1,
            destination: lit("Ford"),
            coin: 5,
            min_level,
            flags: TravelFlags(flags),
            route: Some(RouteMode::Slow),
            area: Some(area),
        }],
    })
}

fn instances(level: Option<u16>, featured: Ref) -> Event {
    Event::World(WorldEvent::InstanceListShown {
        window: W,
        entries: vec![InstanceEntry {
            entry: 1,
            instance: lit("Mine"),
            level,
            size: Some(3),
            tier: Some(u32::MAX),
            locked: true,
        }],
        source: Some(EntityId(9)),
        featured: Some(featured),
    })
}

fn vital(current: f32, max: f32) -> Event {
    Event::World(WorldEvent::MountVitalChanged {
        entity: P,
        vital: MountVital::Morale,
        current,
        max,
    })
}

fn queue(instance: Ref) -> Event {
    Event::World(WorldEvent::InstanceQueueChanged {
        state: QueueState::OthersConfirmEnterQueue,
        instance: Some(instance),
        seconds: Some(86_400),
    })
}

fn world(ev: &[Event]) -> Vec<&WorldEvent> {
    ev.iter()
        .filter_map(|e| match e {
            Event::World(w) => Some(w),
            _ => None,
        })
        .collect()
}

#[test]
fn session_applies_the_p5_event_rules() {
    let long = "x".repeat(100);
    let script = vec![
        vec![Event::Connected { characters: vec![] }],
        vec![
            // Before world entry the entity rule drops them.
            Event::World(WorldEvent::TravelEnded),
            vital(1.0, 2.0),
            queue(lit("Q")),
            Event::WorldEntered {
                player: P,
                name: "P".into(),
                transform: Transform::default(),
            },
            // Valid.
            Event::World(WorldEvent::TravelEnded),
            vital(1.0, 2.0),
            queue(lit("Q")),
            marker(Some(1)),
            marker(Some(64)),
            marker(None),
            travel(0b111, Some(10_000), lit("Vale")),
            instances(Some(10_000), lit("Mine")),
            // Cleaned: literal refs cut to 64 bytes, control characters replaced.
            travel(0, None, lit(&long)),
            instances(None, lit(&long)),
            queue(lit("a\u{7}b")),
            // Invalid: dropped.
            marker(Some(0)),
            marker(Some(65)),
            travel(0b1000, None, lit("Vale")),
            travel(0, Some(10_001), lit("Vale")),
            instances(Some(10_001), lit("Mine")),
            vital(f32::NAN, 2.0),
            vital(1.0, f32::INFINITY),
        ],
    ];
    let mut s = Session::new(Scripted(script));
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    assert_eq!(s.poll(t).len(), 1);
    s.send(
        t,
        Command::EnterWorld {
            character: "P".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t);
    let w = world(&ev);
    assert_eq!(
        w.iter().map(|e| e.name()).collect::<Vec<_>>(),
        [
            "TravelEnded",
            "MountVitalChanged",
            "InstanceQueueChanged",
            "MarkerAdded",
            "MarkerAdded",
            "MarkerAdded",
            "TravelShown",
            "InstanceListShown",
            "TravelShown",
            "InstanceListShown",
            "InstanceQueueChanged",
        ],
        "{w:#?}"
    );
    let WorldEvent::TravelShown { entries, .. } = w[8] else {
        panic!("{:?}", w[8]);
    };
    assert!(matches!(&entries[0].area, Some(Ref::Literal(a)) if a.len() == MAX_LITERAL_REF_LEN));
    let WorldEvent::InstanceListShown { featured, .. } = w[9] else {
        panic!("{:?}", w[9]);
    };
    assert!(matches!(featured, Some(Ref::Literal(a)) if a.len() == MAX_LITERAL_REF_LEN));
    let WorldEvent::InstanceQueueChanged { instance, .. } = w[10] else {
        panic!("{:?}", w[10]);
    };
    assert_eq!(instance, &Some(lit("a b")));
}

fn join(level: Option<u16>) -> Command {
    Command::World(WorldCommand::JoinInstance {
        window: W,
        entry: 1,
        size: Some(2),
        tier: None,
        level,
    })
}

#[test]
fn leave_queue_needs_the_world_and_join_levels_are_checked() {
    let leave = Command::World(WorldCommand::LeaveQueue);
    assert_eq!(leave.name(), "LeaveQueue");
    let mut s = Session::new(SandboxBackend::new(SandboxConfig::default()).expect("sandbox"));
    let t = Instant::now();
    assert!(matches!(
        s.send(t, leave.clone()),
        Err(NetError::InvalidState { .. })
    ));
    s.send(t, Command::Connect).unwrap();
    let character = s
        .poll(t)
        .iter()
        .find_map(|e| match e {
            Event::Connected { characters } => characters.first().map(|c| c.name.clone()),
            _ => None,
        })
        .expect("a character");
    assert!(matches!(
        s.send(t, leave.clone()),
        Err(NetError::InvalidState { .. })
    ));
    s.send(t, Command::EnterWorld { character }).unwrap();
    let _ = s.poll(t);
    for bad in [Some(0), Some(10_001), Some(u16::MAX)] {
        assert!(
            matches!(s.send(t, join(bad)), Err(NetError::InvalidCommand(_))),
            "{bad:?}"
        );
    }
    // The sandbox without a scenario accepts and ignores them (§9.5).
    s.send(t, join(None)).unwrap();
    s.send(t, join(Some(1))).unwrap();
    s.send(t, join(Some(10_000))).unwrap();
    s.send(t, leave).unwrap();
}

/// Polls until `f` matches an event (at most 200 frames of 50 ms), collecting the World
/// events seen on the way.
fn poll_until(
    s: &mut Session,
    now: &mut Instant,
    seen: &mut Vec<WorldEvent>,
    mut f: impl FnMut(&Event) -> bool,
) -> bool {
    for _ in 0..200 {
        let ev = s.poll(*now);
        let mut hit = false;
        for e in &ev {
            if let Event::World(w) = e {
                seen.push(w.clone());
            }
            hit |= f(e);
        }
        if hit {
            return true;
        }
        *now += Duration::from_millis(50);
    }
    false
}

/// The new World forms through the sandbox: the client sends `JoinInstance` with its
/// options and `LeaveQueue` itself (a windowed run), and the triggers script the queue.
#[test]
fn p5_world_forms_play_through_the_sandbox() {
    let text = r#"tessera-scenario 1
tick 50ms
player "Testa"
select $r = map-region all
at 0s instances "Old mine","Deep vault" featured="Deep vault" level=20 size=3 locked=2
+0s markers $m $r named
+0s marker $camp 1,2,0 "Camp" note-type=9
on join-instance entry=1
+0s instance-queue confirm "Old mine" seconds=30
+0s mount player "Grey steed" kind=basic
+0s mount-vital player power 10/20
end
on leave-queue
+0s instance-queue none
+0s travel "Ford" 1s route=slow
+0.1s travel-end
end
"#;
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    let mut resolved = scenario.resolve(&mut EmptyResolver).expect("resolve");
    resolved.set_plays_input(false);
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(resolved),
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut now = Instant::now();
    s.send(now, Command::Connect).unwrap();
    let _ = s.poll(now);
    s.send(
        now,
        Command::EnterWorld {
            character: "Testa".into(),
        },
    )
    .unwrap();
    let mut seen = Vec::new();
    let mut window = None;
    assert!(poll_until(&mut s, &mut now, &mut seen, |e| match e {
        Event::World(WorldEvent::InstanceListShown { window: w, .. }) => {
            window = Some(*w);
            true
        }
        _ => false,
    }));
    let window = window.expect("instances window");
    // An entry beyond the list fires nothing; entry 1 fires the trigger.
    s.send(
        now,
        Command::World(WorldCommand::JoinInstance {
            window,
            entry: 1,
            size: Some(3),
            tier: Some(1),
            level: Some(20),
        }),
    )
    .unwrap();
    assert!(poll_until(&mut s, &mut now, &mut seen, |e| matches!(
        e,
        Event::World(WorldEvent::MountVitalChanged { .. })
    )));
    s.send(now, Command::World(WorldCommand::LeaveQueue))
        .unwrap();
    assert!(poll_until(&mut s, &mut now, &mut seen, |e| matches!(
        e,
        Event::World(WorldEvent::TravelEnded)
    )));
    let names: Vec<&str> = seen.iter().map(WorldEvent::name).collect();
    assert_eq!(
        names,
        [
            "InstanceListShown",
            "MarkerAdded",
            "InstanceQueueChanged",
            "MountChanged",
            "MountVitalChanged",
            "InstanceQueueChanged",
            "TravelStarted",
            "TravelEnded",
        ],
        "{seen:#?}"
    );
    assert!(matches!(
        &seen[0],
        WorldEvent::InstanceListShown { entries, source: None, featured: Some(Ref::Literal(f)), .. }
            if f == "Deep vault"
                && entries.len() == 2
                && entries[0].level == Some(20)
                && entries[1].size == Some(3)
                && !entries[0].locked
                && entries[1].locked
                && entries[1].tier.is_none()
    ));
    assert!(matches!(
        &seen[1],
        WorldEvent::MarkerAdded {
            marker: MarkerId(1),
            note_type: Some(9),
            ..
        }
    ));
    assert!(matches!(
        &seen[2],
        WorldEvent::InstanceQueueChanged {
            state: QueueState::Confirm,
            seconds: Some(30),
            ..
        }
    ));
    assert!(matches!(
        &seen[3],
        WorldEvent::MountChanged {
            combat: Some(false),
            mount: Some(_),
            ..
        }
    ));
    assert!(matches!(
        &seen[4],
        WorldEvent::MountVitalChanged { vital: MountVital::Power, current, max, .. }
            if *current == 10.0 && *max == 20.0
    ));
    assert!(matches!(
        &seen[5],
        WorldEvent::InstanceQueueChanged {
            state: QueueState::None,
            instance: None,
            seconds: None
        }
    ));
    assert!(matches!(
        &seen[6],
        WorldEvent::TravelStarted {
            route: Some(RouteMode::Slow),
            duration_ms: 1000,
            ..
        }
    ));
}

/// The test-server backend refuses `LeaveQueue` and `JoinInstance` like every other
/// gameplay command, and never produces the P5 events.
#[cfg(feature = "remote")]
mod remote_support;

#[cfg(feature = "remote")]
#[test]
fn the_test_server_backend_refuses_leave_queue() {
    use remote_support::{PeerOptions, connect, pump, spawn};
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let _ = p.closed().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    assert!(matches!(
        s.send(Instant::now(), Command::World(WorldCommand::LeaveQueue)),
        Err(NetError::InvalidCommand(_))
    ));
    assert!(matches!(
        s.send(Instant::now(), join(Some(5))),
        Err(NetError::InvalidCommand(_))
    ));
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

#[test]
fn travel_flags_combine_as_typed_marks() {
    let f = TravelFlags::UNMET | TravelFlags::UNUSABLE;
    assert_eq!(f, TravelFlags(6));
    assert!(f.is_valid() && !f.is_empty());
    assert!(f.contains(TravelFlags::UNMET) && f.contains(TravelFlags::UNUSABLE));
    assert!(!f.contains(TravelFlags::UNDISCOVERED));
    assert!(!f.contains(TravelFlags::UNDISCOVERED | TravelFlags::UNMET));
    let mut g = TravelFlags::NONE;
    assert!(g.is_empty());
    g |= TravelFlags::UNDISCOVERED;
    assert_eq!(g.0, 1);
    assert!(!TravelFlags(8).is_valid());
}
