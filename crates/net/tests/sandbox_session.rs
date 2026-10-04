// SPDX-License-Identifier: GPL-3.0-or-later
//! Session state machine and sandbox backend behaviour.

use std::time::{Duration, Instant};

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Backend, ChatChannel, Command, DisconnectReason, EntityId, EntityKind, Event, NetError,
    Session, SessionState, Transform, Vec3,
};

fn spawn() -> Transform {
    Transform::new(Vec3::new(100.0, -50.0, 12.5), 1.0)
}

fn config() -> SandboxConfig {
    SandboxConfig {
        characters: vec!["Alda".into(), "Bree".into()],
        spawn: spawn(),
        wanderers: 3,
        ..SandboxConfig::default()
    }
}

fn session() -> Session {
    Session::new(SandboxBackend::new(config()).unwrap())
}

fn in_world(t0: Instant) -> (Session, Vec<Event>) {
    let mut s = session();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "Bree".into(),
        },
    )
    .unwrap();
    let events = s.poll(t0);
    (s, events)
}

#[test]
fn full_session_flow() {
    let t0 = Instant::now();
    let mut s = session();
    assert_eq!(s.state(), SessionState::Idle);
    assert_eq!(s.backend_name(), "sandbox");

    s.send(t0, Command::Connect).unwrap();
    assert_eq!(s.state(), SessionState::Connecting);
    let ev: Vec<_> = s.poll(t0);
    let Event::Connected { characters } = &ev[0] else {
        panic!("expected Connected, got {ev:?}");
    };
    let names: Vec<_> = characters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Alda", "Bree"]);
    assert_eq!(s.state(), SessionState::Connected);

    s.send(
        t0,
        Command::EnterWorld {
            character: "Bree".into(),
        },
    )
    .unwrap();
    let ev: Vec<_> = s.poll(t0);
    let Event::WorldEntered {
        player,
        name,
        transform,
    } = &ev[0]
    else {
        panic!("expected WorldEntered, got {ev:?}");
    };
    assert_eq!(name, "Bree");
    assert_eq!(*transform, spawn());
    assert_eq!(s.player(), Some(*player));
    assert_eq!(s.state(), SessionState::InWorld);

    let spawned: Vec<EntityId> = ev
        .iter()
        .filter_map(|e| match e {
            Event::EntitySpawned { id, kind, .. } => {
                assert_eq!(*kind, EntityKind::NonPlayer);
                Some(*id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(spawned.len(), 3);
    assert!(!spawned.contains(player));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::ChatMessage {
            channel: ChatChannel::System,
            ..
        }
    )));

    // Wanderers move once per tick.
    let t1 = t0 + Duration::from_millis(50);
    assert_eq!(s.poll(t1).len(), 0, "no tick before the interval");
    let t2 = t0 + Duration::from_millis(500);
    let moved: Vec<_> = s
        .poll(t2)
        .into_iter()
        .filter_map(|e| match e {
            Event::EntityMoved {
                id,
                transform,
                velocity,
            } => Some((id, transform, velocity)),
            _ => None,
        })
        .collect();
    assert_eq!(moved.len(), 3);
    for (id, tr, vel) in &moved {
        assert!(spawned.contains(id));
        assert!(tr.is_finite());
        assert!((vel.length() - 1.5).abs() < 1e-3, "speed {}", vel.length());
        // Wanderers stay near the spawn point.
        assert!(tr.position.distance(spawn().position) < 4.0 * 6.0);
    }

    // Chat echoes with the character as sender.
    s.send(
        t2,
        Command::Chat {
            channel: ChatChannel::Say,
            text: "hello".into(),
        },
    )
    .unwrap();
    let ev: Vec<_> = s.poll(t2);
    assert_eq!(
        ev,
        vec![Event::ChatMessage {
            channel: ChatChannel::Say,
            sender: Some("Bree".into()),
            text: "hello".into(),
        }]
    );

    // Tells get a system notice.
    s.send(
        t2,
        Command::Chat {
            channel: ChatChannel::Tell {
                peer: "Nobody".into(),
            },
            text: "psst".into(),
        },
    )
    .unwrap();
    let ev: Vec<_> = s.poll(t2);
    assert!(matches!(
        &ev[..],
        [Event::ChatMessage { channel: ChatChannel::System, sender: None, text }] if text.contains("Nobody")
    ));

    // Plausible movement is accepted silently.
    let t3 = t2 + Duration::from_secs(1);
    let step = Transform::new(spawn().position + Vec3::new(5.0, 0.0, 0.0), 0.0);
    s.send(
        t3,
        Command::Move {
            transform: step,
            velocity: Vec3::new(5.0, 0.0, 0.0),
        },
    )
    .unwrap();
    let me = s.player();
    assert!(
        s.poll(t3)
            .into_iter()
            .all(|e| !matches!(e, Event::EntityMoved { id, .. } if Some(id) == me))
    );

    // A teleport is corrected back to the last accepted position.
    let tele = Transform::new(Vec3::new(1.0e5, 0.0, 0.0), 0.0);
    s.send(
        t3,
        Command::Move {
            transform: tele,
            velocity: Vec3::ZERO,
        },
    )
    .unwrap();
    let player = s.player().unwrap();
    let ev: Vec<_> = s.poll(t3);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::EntityMoved { id, transform, .. } if *id == player && *transform == step
    )));

    s.send(t3, Command::Disconnect).unwrap();
    assert_eq!(s.state(), SessionState::Closing);
    let ev: Vec<_> = s.poll(t3);
    assert_eq!(
        ev.last(),
        Some(&Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        })
    );
    assert_eq!(s.state(), SessionState::Closed);
    assert_eq!(s.player(), None);
    assert_eq!(s.poll(t3 + Duration::from_secs(10)).len(), 0);
    assert!(matches!(
        s.send(t3, Command::Connect),
        Err(NetError::InvalidState { .. })
    ));
}

#[test]
fn commands_are_validated() {
    let t0 = Instant::now();
    let mut s = session();
    let mv = Command::Move {
        transform: spawn(),
        velocity: Vec3::ZERO,
    };
    assert!(matches!(
        s.send(t0, mv.clone()),
        Err(NetError::InvalidState {
            command: "Move",
            state: SessionState::Idle
        })
    ));
    assert!(matches!(
        s.send(t0, Command::Disconnect),
        Err(NetError::InvalidState { .. })
    ));

    let (mut s, _) = in_world(t0);
    let bad = [
        Command::Chat {
            channel: ChatChannel::Say,
            text: String::new(),
        },
        Command::Chat {
            channel: ChatChannel::Say,
            text: "x".repeat(tessera_net::model::MAX_CHAT_LEN + 1),
        },
        Command::Chat {
            channel: ChatChannel::Say,
            text: "bell\u{7}".into(),
        },
        Command::Chat {
            channel: ChatChannel::System,
            text: "spoof".into(),
        },
        Command::Move {
            transform: Transform::new(Vec3::new(f32::NAN, 0.0, 0.0), 0.0),
            velocity: Vec3::ZERO,
        },
        Command::Move {
            transform: spawn(),
            velocity: Vec3::new(0.0, f32::INFINITY, 0.0),
        },
        Command::Chat {
            channel: ChatChannel::Tell {
                peer: "Ev\nil".into(),
            },
            text: "hi".into(),
        },
        Command::Chat {
            channel: ChatChannel::Tell {
                peer: "\u{0}".into(),
            },
            text: "hi".into(),
        },
    ];
    for cmd in bad {
        assert!(
            matches!(s.send(t0, cmd.clone()), Err(NetError::InvalidCommand(_))),
            "{cmd:?} accepted"
        );
    }
    assert!(matches!(
        s.send(t0, Command::Connect),
        Err(NetError::InvalidState { .. })
    ));
    s.send(t0, mv).unwrap();

    // Character names get the same checks as tell recipients.
    let mut s = session();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    for name in ["", "Br\u{0}ee", "a\nb", &"x".repeat(65)] {
        let cmd = Command::EnterWorld {
            character: name.into(),
        };
        assert!(
            matches!(s.send(t0, cmd), Err(NetError::InvalidCommand(_))),
            "{name:?} accepted"
        );
    }
    assert_eq!(s.state(), SessionState::Connected);
}

#[test]
fn unknown_character_is_refused() {
    let t0 = Instant::now();
    let mut s = session();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "Mallory".into(),
        },
    )
    .unwrap();
    let ev: Vec<_> = s.poll(t0);
    assert!(matches!(
        ev.last(),
        Some(Event::Disconnected {
            reason: DisconnectReason::ServerClosed(_)
        })
    ));
    assert_eq!(s.state(), SessionState::Closed);
}

#[test]
fn latency_delays_events() {
    let t0 = Instant::now();
    let backend = SandboxBackend::new(SandboxConfig {
        latency: Duration::from_millis(80),
        ..config()
    })
    .unwrap();
    let mut s = Session::new(backend);
    s.send(t0, Command::Connect).unwrap();
    assert_eq!(s.poll(t0 + Duration::from_millis(79)).len(), 0);
    assert_eq!(s.state(), SessionState::Connecting);
    assert_eq!(s.poll(t0 + Duration::from_millis(80)).len(), 1);
    assert_eq!(s.state(), SessionState::Connected);
}

#[test]
fn invalid_sandbox_config_is_rejected() {
    for cfg in [
        SandboxConfig {
            characters: vec![],
            ..config()
        },
        SandboxConfig {
            wander_radius: 0.0,
            ..config()
        },
        SandboxConfig {
            spawn: Transform::new(Vec3::new(f32::NAN, 0.0, 0.0), 0.0),
            ..config()
        },
        SandboxConfig {
            tick_interval: Duration::ZERO,
            ..config()
        },
        // Values that used to produce NaN or infinite transforms.
        SandboxConfig {
            wander_radius: 1e-40,
            ..config()
        },
        SandboxConfig {
            wander_radius: 2e38,
            ..config()
        },
        SandboxConfig {
            wander_radius: f32::NAN,
            ..config()
        },
        SandboxConfig {
            wander_speed: 1e30,
            ..config()
        },
        SandboxConfig {
            max_player_speed: f32::NAN,
            ..config()
        },
        SandboxConfig {
            spawn: Transform::new(Vec3::new(0.0, 3e38, 0.0), 0.0),
            ..config()
        },
        SandboxConfig {
            characters: vec!["Line\nbreak".into()],
            ..config()
        },
    ] {
        assert!(matches!(
            SandboxBackend::new(cfg),
            Err(NetError::InvalidConfig(_))
        ));
    }
}

/// A buggy backend that emits events out of order.
#[derive(Debug)]
struct Chaotic;

impl Backend for Chaotic {
    fn name(&self) -> &str {
        "chaotic"
    }
    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }
    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.push(Event::EntityDespawned { id: EntityId(1) });
        out.push(Event::WorldEntered {
            player: EntityId(1),
            name: "x".into(),
            transform: Transform::default(),
        });
        out.push(Event::Connected { characters: vec![] });
        out.push(Event::Disconnected {
            reason: DisconnectReason::TimedOut,
        });
        out.push(Event::Connected { characters: vec![] });
    }
}

#[test]
fn session_filters_out_of_order_events() {
    let t0 = Instant::now();
    let mut s = Session::new(Chaotic);
    s.send(t0, Command::Connect).unwrap();
    let ev: Vec<_> = s.poll(t0);
    assert_eq!(
        ev,
        vec![
            Event::Connected { characters: vec![] },
            Event::Disconnected {
                reason: DisconnectReason::TimedOut
            },
        ]
    );
    assert_eq!(s.state(), SessionState::Closed);
}

fn enter_world_events(s: &mut Session, t: Instant) {
    s.send(t, Command::Connect).unwrap();
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Bree".into(),
        },
    )
    .unwrap();
}

/// Regression: Disconnect while EnteringWorld used to surface entity events
/// (including a despawn of the never-announced player) for a world the
/// client never entered.
#[test]
fn disconnect_while_entering_world_surfaces_only_disconnected() {
    let t0 = Instant::now();
    let mut s = session();
    enter_world_events(&mut s, t0);
    s.send(t0, Command::Disconnect).unwrap();
    assert_eq!(
        s.poll(t0),
        vec![Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        }]
    );
    assert_eq!(s.state(), SessionState::Closed);
    assert_eq!(s.player(), None);
}

#[test]
fn disconnect_while_connecting_surfaces_only_disconnected() {
    let t0 = Instant::now();
    let backend = SandboxBackend::new(SandboxConfig {
        latency: Duration::from_millis(50),
        ..config()
    })
    .unwrap();
    let mut s = Session::new(backend);
    s.send(t0, Command::Connect).unwrap();
    s.send(t0, Command::Disconnect).unwrap();
    assert_eq!(
        s.poll(t0 + Duration::from_millis(50)),
        vec![Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        }]
    );
}

#[test]
fn disconnect_in_world_still_reports_player_despawn() {
    let t0 = Instant::now();
    let (mut s, _) = in_world(t0);
    let player = s.player().unwrap();
    s.send(t0, Command::Disconnect).unwrap();
    assert_eq!(
        s.poll(t0),
        vec![
            Event::EntityDespawned { id: player },
            Event::Disconnected {
                reason: DisconnectReason::ClientRequested
            }
        ]
    );
}

/// The poll result does not borrow the session, so a frame loop can react
/// to an event by sending a command.
#[test]
fn poll_into_allows_sending_while_handling_events() {
    let t0 = Instant::now();
    let mut s = session();
    s.send(t0, Command::Connect).unwrap();
    let mut events = Vec::new();
    let mut now = t0;
    for _ in 0..3 {
        s.poll_into(now, &mut events);
        for event in events.drain(..) {
            if let Event::Connected { characters } = event {
                let character = characters[1].name.clone();
                s.send(now, Command::EnterWorld { character }).unwrap();
            }
        }
        now += Duration::from_millis(16);
    }
    assert_eq!(s.state(), SessionState::InWorld);
}

/// Regression: the wanderer phase was computed in f32 from total elapsed
/// time, so after ~100 days positions advanced in 1 s steps.
#[test]
fn wanderers_stay_smooth_after_long_uptime() {
    let t0 = Instant::now();
    let (mut s, _) = in_world(t0);
    let base = t0 + Duration::from_secs(100 * 24 * 3600);
    let mut positions = Vec::new();
    for k in 1..=20u32 {
        for e in s.poll(base + Duration::from_millis(100) * k) {
            if let Event::EntityMoved { id, transform, .. } = e
                && id == EntityId(2)
            {
                positions.push(transform.position);
            }
        }
    }
    assert_eq!(positions.len(), 20);
    for w in positions.windows(2) {
        // 1.5 m/s for 100 ms.
        let step = w[0].distance(w[1]);
        assert!((step - 0.15).abs() < 0.01, "step {step}");
    }
}

#[test]
fn extreme_valid_sandbox_configs_emit_finite_transforms() {
    let t0 = Instant::now();
    for (radius, speed, at) in [(0.01, 1e4, 1e7), (1e5, 1e4, -1e7), (1e5, 1e-3, 1e7)] {
        let backend = SandboxBackend::new(SandboxConfig {
            wander_radius: radius,
            wander_speed: speed,
            max_player_speed: 1e4,
            spawn: Transform::new(Vec3::new(at, -at, at), 1e30),
            ..config()
        })
        .unwrap();
        let mut s = Session::new(backend);
        enter_world_events(&mut s, t0);
        let mut seen = 0;
        for k in 0..50u32 {
            for e in s.poll(t0 + Duration::from_millis(137) * k) {
                match e {
                    Event::EntitySpawned { transform, .. } => {
                        assert!(transform.is_finite());
                        seen += 1;
                    }
                    Event::EntityMoved {
                        transform,
                        velocity,
                        ..
                    } => {
                        assert!(transform.is_finite() && velocity.is_finite());
                        seen += 1;
                    }
                    _ => {}
                }
            }
        }
        assert!(seen > 3);
    }
}

/// Emits one scripted batch per poll and records every `now` it is given.
#[derive(Debug, Default)]
struct Scripted {
    batches: std::collections::VecDeque<Vec<Event>>,
    nows: std::sync::Arc<std::sync::Mutex<Vec<Instant>>>,
}

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn submit(&mut self, now: Instant, _command: Command) -> Result<(), NetError> {
        self.nows.lock().unwrap().push(now);
        Ok(())
    }
    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        self.nows.lock().unwrap().push(now);
        out.extend(self.batches.pop_front().unwrap_or_default());
    }
}

#[test]
fn session_drops_non_finite_events() {
    let t0 = Instant::now();
    let bad = Transform::new(Vec3::new(f32::NAN, 0.0, 0.0), 0.0);
    let good = Transform::default();
    let spawned = |id, transform| Event::EntitySpawned {
        id: EntityId(id),
        kind: EntityKind::Creature,
        name: String::new(),
        transform,
    };
    let backend = Scripted {
        batches: [
            vec![Event::Connected { characters: vec![] }],
            vec![
                Event::WorldEntered {
                    player: EntityId(1),
                    name: "x".into(),
                    transform: bad,
                },
                Event::WorldEntered {
                    player: EntityId(1),
                    name: "x".into(),
                    transform: good,
                },
                spawned(2, bad),
                Event::EntityMoved {
                    id: EntityId(1),
                    transform: good,
                    velocity: Vec3::new(f32::INFINITY, 0.0, 0.0),
                },
                spawned(3, good),
            ],
        ]
        .into(),
        ..Scripted::default()
    };
    let mut s = Session::new(backend);
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "x".into(),
        },
    )
    .unwrap();
    assert_eq!(
        s.poll(t0),
        vec![
            Event::WorldEntered {
                player: EntityId(1),
                name: "x".into(),
                transform: good,
            },
            spawned(3, good),
        ]
    );
}

#[test]
fn backend_never_sees_time_go_backwards() {
    let t0 = Instant::now();
    let t1 = t0 + Duration::from_millis(100);
    let backend = Scripted::default();
    let nows = std::sync::Arc::clone(&backend.nows);
    let mut s = Session::new(backend);
    let _ = s.poll(t1);
    s.send(t0, Command::Connect).unwrap(); // stale frame time
    let _ = s.poll(t0);
    let _ = s.poll(t1 + Duration::from_millis(1));
    assert_eq!(
        *nows.lock().unwrap(),
        [t1, t1, t1, t1 + Duration::from_millis(1)]
    );
}
