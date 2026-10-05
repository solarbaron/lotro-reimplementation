// SPDX-License-Identifier: GPL-3.0-or-later
//! Characters in the sandbox: the avatar's appearance, figures placed by the world
//! data (NPCs, weenie placements) served as `EntityCreated`, wanderers and the crowd.

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_net::model::{MAX_APPEARANCE_SCALE, MAX_EQUIPMENT, MAX_NAME_LEN};
use tessera_net::sandbox::{
    CrowdConfig, Landblock, Lookup, PlacedFigure, SandboxBackend, SandboxConfig, SpawnPoint,
    StaticConfig, StaticEntity, WorldSource,
};
use tessera_net::{
    Appearance, Backend, Body, Command, DataId, EntityId, EntityKind, Equipped, Event, Location,
    Placement, RegionId, Rotation, Session, SessionState, Transform, Vec3,
};

const R1: RegionId = RegionId(1);

fn human() -> Appearance {
    Appearance::new(Body::Character {
        species: 0x17,
        sex: 0x1000,
    })
}

/// A source with flat ground at 100 m and, per block, the given entries.
#[derive(Debug, Default)]
struct Figures {
    blocks: BTreeMap<Landblock, Vec<StaticEntity>>,
}

impl WorldSource for Figures {
    fn ground_height(&mut self, _location: &Location) -> Lookup<f64> {
        Lookup::Ready(100.0)
    }

    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        match self.blocks.get(&block) {
            Some(list) => {
                out.extend(list.iter().take(limit).cloned());
                Lookup::Ready(())
            }
            None => Lookup::Unavailable("empty".into()),
        }
    }
}

fn block(x: u8, y: u8) -> Landblock {
    Landblock::new(R1, x, y).unwrap()
}

fn enter(config: SandboxConfig, source: Option<Figures>) -> (Session, Vec<Event>, Instant) {
    let backend = match source {
        Some(s) => SandboxBackend::with_world(config, s),
        None => SandboxBackend::new(config),
    }
    .unwrap();
    let mut session = Session::new(backend);
    let mut now = Instant::now();
    session.send(now, Command::Connect).unwrap();
    let mut events = session.poll(now);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Wanderer".into(),
            },
        )
        .unwrap();
    for _ in 0..10 {
        events.extend(session.poll(now));
        now += Duration::from_millis(16);
    }
    assert_eq!(session.state(), SessionState::InWorld);
    (session, events, now)
}

fn created(events: &[Event]) -> Vec<&Event> {
    events
        .iter()
        .filter(|e| matches!(e, Event::EntityCreated { .. }))
        .collect()
}

#[test]
fn avatar_appearance_follows_world_entry() {
    let avatar = human().wearing(Equipped {
        dye: Some(0.06),
        ..Equipped::new(DataId(0x7900_0001))
    });
    let config = SandboxConfig {
        avatar: Some(avatar.clone()),
        wanderers: 0,
        ..SandboxConfig::default()
    };
    let (session, events, _) = enter(config, None);
    let player = session.player().unwrap();
    let entered = events
        .iter()
        .position(|e| matches!(e, Event::WorldEntered { .. }))
        .unwrap();
    let changed = events
        .iter()
        .position(|e| matches!(e, Event::AppearanceChanged { id, appearance } if *id == player && **appearance == avatar))
        .expect("the avatar's appearance is sent");
    assert!(changed > entered);
    // Without an avatar appearance nothing is sent.
    let (_, events, _) = enter(
        SandboxConfig {
            wanderers: 0,
            ..SandboxConfig::default()
        },
        None,
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::AppearanceChanged { .. }))
    );
}

#[test]
fn invalid_appearances_are_refused_in_the_config() {
    let bad_scale = Appearance {
        scale: f32::NAN,
        ..human()
    };
    let too_big = Appearance {
        scale: MAX_APPEARANCE_SCALE * 2.0,
        ..human()
    };
    let bad_dye = human().wearing(Equipped {
        dye: Some(2.0),
        ..Equipped::new(DataId(1))
    });
    let mut many = human();
    many.equipment = vec![Equipped::new(DataId(1)); MAX_EQUIPMENT + 1];
    for a in [bad_scale, too_big, bad_dye, many] {
        assert!(!a.is_valid());
        let c = SandboxConfig {
            avatar: Some(a.clone()),
            ..SandboxConfig::default()
        };
        assert!(SandboxBackend::new(c).is_err());
        let c = SandboxConfig {
            wanderer_appearances: vec![a.clone()],
            ..SandboxConfig::default()
        };
        assert!(SandboxBackend::new(c).is_err());
        let c = SandboxConfig {
            crowd: CrowdConfig {
                count: 3,
                appearances: vec![a],
                ..CrowdConfig::default()
            },
            ..SandboxConfig::default()
        };
        assert!(SandboxBackend::new(c).is_err());
    }
    // A crowd needs appearances; its numbers are bounded.
    for c in [
        CrowdConfig {
            count: 3,
            ..CrowdConfig::default()
        },
        CrowdConfig {
            count: tessera_net::sandbox::MAX_CROWD + 1,
            appearances: vec![human()],
            ..CrowdConfig::default()
        },
        CrowdConfig {
            count: 1,
            appearances: vec![human()],
            spacing: 0.0,
            ..CrowdConfig::default()
        },
        CrowdConfig {
            count: 1,
            appearances: vec![human()],
            run_speed: f32::INFINITY,
            ..CrowdConfig::default()
        },
    ] {
        let config = SandboxConfig {
            crowd: c,
            ..SandboxConfig::default()
        };
        assert!(SandboxBackend::new(config).is_err());
    }
}

#[test]
fn wanderers_become_characters_with_appearances() {
    let a = human();
    let b = Appearance::new(Body::Object(DataId(0x4700_0001)));
    let config = SandboxConfig {
        wanderers: 3,
        wanderer_appearances: vec![a.clone(), b.clone()],
        ..SandboxConfig::default()
    };
    let (_, events, _) = enter(config, None);
    let made: Vec<(&Appearance, Option<u64>)> = created(&events)
        .into_iter()
        .map(|e| match e {
            Event::EntityCreated {
                appearance, tag, ..
            } => (&**appearance, *tag),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(made.len(), 3);
    assert_eq!(made[0].0, &a);
    assert_eq!(made[1].0, &b);
    assert_eq!(made[2].0, &a);
    assert!(
        made.iter().all(|(_, t)| t.is_none()),
        "simulated, not placed"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntitySpawned { .. }))
    );
}

#[test]
fn crowd_stands_walks_and_runs_in_front_of_the_spawn() {
    let looks = vec![
        human(),
        Appearance::new(Body::Character {
            species: 0x51,
            sex: 0x2000,
        }),
    ];
    let heading = 0.7f32;
    let config = SandboxConfig {
        spawn: Transform::new(Vec3::new(500.0, 500.0, 20.0), heading),
        wanderers: 0,
        crowd: CrowdConfig {
            count: 50,
            appearances: looks.clone(),
            ..CrowdConfig::default()
        },
        tick_interval: Duration::from_millis(50),
        ..SandboxConfig::default()
    };
    let (mut session, events, mut now) = enter(config, None);
    let made = created(&events);
    assert_eq!(made.len(), 50);
    let forward = Vec3::new(heading.cos(), heading.sin(), 0.0);
    let mut ids = BTreeSet::new();
    for (i, e) in made.iter().enumerate() {
        let Event::EntityCreated {
            id,
            location,
            appearance,
            heading: h,
            tag,
            kind,
            ..
        } = e
        else {
            unreachable!()
        };
        assert!(ids.insert(*id));
        assert_eq!(**appearance, looks[i % 2]);
        assert_eq!(*tag, None);
        assert_eq!(*kind, EntityKind::NonPlayer);
        // In front of the spawn: the 8 m of `ahead` less a circle radius at most.
        let d = location.position() - Vec3::new(500.0, 500.0, 20.0);
        let along = d.x * forward.x + d.y * forward.y;
        assert!(along >= 8.0 - 2.5, "{i}: {along}");
        assert!(along <= 8.0 + 5.0 * 3.0 + 2.5, "{i}: {along}");
        assert_eq!(location.z, 20.0);
        if i % 3 == 0 {
            // Standing members face the spawn.
            let back = (heading + PI).rem_euclid(TAU);
            assert!((h - back).abs() < 1e-4, "{i}: {h} vs {back}");
        }
    }
    // Walkers and runners report movement at their speeds; standing members do not.
    let mut speeds: BTreeMap<EntityId, f32> = BTreeMap::new();
    for _ in 0..10 {
        now += Duration::from_millis(60);
        for e in session.poll(now) {
            if let Event::EntityMoved { id, velocity, .. } = e {
                speeds.insert(id, velocity.length());
            }
        }
    }
    let id_of = |i: usize| match made[i] {
        Event::EntityCreated { id, .. } => *id,
        _ => unreachable!(),
    };
    for i in 0..50 {
        let s = speeds.get(&id_of(i)).copied();
        match i % 3 {
            0 => assert_eq!(s, None, "{i} stands"),
            1 => assert!((s.unwrap() - 2.0).abs() < 1e-3, "{i} walks"),
            _ => assert!((s.unwrap() - 6.0).abs() < 1e-3, "{i} runs"),
        }
    }
}

#[test]
fn crowd_follows_a_teleport() {
    let config = SandboxConfig {
        wanderers: 0,
        crowd: CrowdConfig {
            count: 3,
            appearances: vec![human()],
            ..CrowdConfig::default()
        },
        ..SandboxConfig::default()
    };
    let (mut session, events, mut now) = enter(config, None);
    let standing = match created(&events)[0] {
        Event::EntityCreated { id, .. } => *id,
        _ => unreachable!(),
    };
    session
        .send(
            now,
            Command::Teleport {
                destination: tessera_net::Destination::Location {
                    location: Location::new(RegionId(0), 1000.0, 0.0, 0.0),
                    heading: 0.0,
                },
            },
        )
        .unwrap();
    let mut moved = None;
    for _ in 0..10 {
        now += Duration::from_millis(120);
        for e in session.poll(now) {
            if let Event::EntityMoved { id, transform, .. } = e
                && id == standing
            {
                moved = Some(transform.position);
            }
        }
    }
    let p = moved.expect("the standing member is moved once");
    assert!((p.x - 1008.0).abs() < 1e-3, "{p:?}");
}

/// A placed figure with a yaw of `yaw` (radians, counter-clockwise).
fn figure_at(block: Landblock, local: [f64; 3], yaw: f32, tag: u64) -> StaticEntity {
    StaticEntity::new(
        DataId(0x7000_0042),
        Placement::new(block.location(local), Rotation::about_z(yaw), 1.0),
        tag,
    )
    .with_figure(PlacedFigure {
        name: "Placed".into(),
        class: Some(DataId(0x7000_0042)),
        ..PlacedFigure::new(
            EntityKind::NonPlayer,
            Appearance {
                idle: Some(7),
                scale: 0.9,
                ..Appearance::new(Body::Object(DataId(0x4700_4E6F)))
            },
        )
    })
}

#[test]
fn placed_figures_are_created_at_the_data_position_and_heading() {
    let b = block(27, 143);
    let mut src = Figures::default();
    src.blocks.insert(
        b,
        vec![
            // A plain static stays a StaticSpawned.
            StaticEntity::new(
                DataId(0x4700_0001),
                Placement::new(b.location([1.0, 1.0, 1.0]), Rotation::IDENTITY, 1.0),
                1,
            ),
            figure_at(b, [100.58, 19.27, 494.75], 0.0, 2),
            figure_at(b, [48.32, 38.57, 357.86], FRAC_PI_2, 3),
        ],
    );
    let config = SandboxConfig {
        spawn_point: Some(SpawnPoint::parse("1,27,143").unwrap()),
        statics: StaticConfig {
            radius: 0,
            ..StaticConfig::default()
        },
        wanderers: 0,
        ..SandboxConfig::default()
    };
    let (mut session, mut events, mut now) = enter(config, Some(src));
    for _ in 0..5 {
        now += Duration::from_millis(16);
        events.extend(session.poll(now));
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::StaticSpawned { .. }))
            .count(),
        1
    );
    let made = created(&events);
    assert_eq!(made.len(), 2);
    for (e, (local, heading, tag)) in made.iter().zip([
        ([100.58, 19.27, 494.75], FRAC_PI_2, 2u64),
        ([48.32, 38.57, 357.86], PI, 3),
    ]) {
        let Event::EntityCreated {
            location,
            heading: h,
            class,
            appearance,
            tag: t,
            name,
            ..
        } = e
        else {
            unreachable!()
        };
        assert_eq!(*location, b.location(local));
        // Model +Y forward: yaw 0 faces north (heading π/2), a quarter turn west.
        let diff = (h - heading).rem_euclid(TAU);
        assert!(diff < 1e-5 || TAU - diff < 1e-5, "{h} vs {heading}");
        assert_eq!(*class, Some(DataId(0x7000_0042)));
        assert_eq!(appearance.idle, Some(7));
        assert_eq!(*t, Some(tag));
        assert_eq!(name, "Placed");
    }
    // Leaving the grid clears statics and placed figures with one event.
    session
        .send(
            now,
            Command::Teleport {
                destination: tessera_net::Destination::Location {
                    location: Location::new(R1, -5000.0, 10.0, 0.0),
                    heading: 0.0,
                },
            },
        )
        .unwrap();
    let mut cleared = false;
    for _ in 0..5 {
        now += Duration::from_millis(16);
        cleared |= session
            .poll(now)
            .iter()
            .any(|e| matches!(e, Event::StaticsCleared));
    }
    assert!(cleared);
}

#[test]
fn invalid_placed_figures_are_rejected_and_names_cut() {
    let b = block(5, 5);
    let mut long = figure_at(b, [1.0, 1.0, 1.0], 0.0, 1);
    if let Some(f) = long.figure.as_mut() {
        f.name = format!("{}\u{7}", "é".repeat(200));
    }
    let mut bad = figure_at(b, [2.0, 1.0, 1.0], 0.0, 2);
    if let Some(f) = bad.figure.as_mut() {
        f.appearance.scale = -1.0;
    }
    let mut src = Figures::default();
    src.blocks.insert(b, vec![long, bad]);
    let config = SandboxConfig {
        spawn_point: Some(SpawnPoint::parse("1,5,5").unwrap()),
        statics: StaticConfig {
            radius: 0,
            ..StaticConfig::default()
        },
        wanderers: 0,
        ..SandboxConfig::default()
    };
    let backend = SandboxBackend::with_world(config, src).unwrap();
    let mut session = Session::new(backend);
    let mut now = Instant::now();
    session.send(now, Command::Connect).unwrap();
    let _ = session.poll(now);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Wanderer".into(),
            },
        )
        .unwrap();
    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(session.poll(now));
        now += Duration::from_millis(16);
    }
    let made = created(&events);
    assert_eq!(made.len(), 1);
    let Event::EntityCreated { name, .. } = made[0] else {
        unreachable!()
    };
    assert!(name.len() <= MAX_NAME_LEN);
    assert!(!name.chars().any(char::is_control));
    let stats = session.backend::<SandboxBackend>().unwrap().static_stats();
    assert_eq!(stats.rejected, 1);
}

/// A backend that emits whatever it is told, to check the session's filter.
#[derive(Debug)]
struct Scripted(Vec<Vec<Event>>);

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), tessera_net::NetError> {
        Ok(())
    }
    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if !self.0.is_empty() {
            out.extend(self.0.remove(0));
        }
    }
}

#[test]
fn session_drops_invalid_created_entities() {
    let make = |location: Location, heading: f32, appearance: Appearance| Event::EntityCreated {
        id: EntityId(5),
        kind: EntityKind::NonPlayer,
        name: "x\ny".into(),
        class: None,
        location,
        heading,
        appearance: Box::new(appearance),
        tag: None,
    };
    let good = Location::new(R1, 1.0, 2.0, 3.0);
    let script = vec![
        vec![tessera_net::Event::Connected {
            characters: vec![tessera_net::CharacterSummary { name: "P".into() }],
        }],
        vec![
            make(good, 0.0, human()), // before the world: dropped
            Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::default(),
            },
            make(good, 0.0, human()),
            make(Location::new(R1, f64::NAN, 0.0, 0.0), 0.0, human()),
            make(good, f32::INFINITY, human()),
            make(
                good,
                0.0,
                Appearance {
                    scale: 0.0,
                    ..human()
                },
            ),
            Event::AppearanceChanged {
                id: EntityId(1),
                appearance: Box::new(human()),
            },
            Event::AppearanceChanged {
                id: EntityId(1),
                appearance: Box::new(Appearance {
                    scale: f32::NAN,
                    ..human()
                }),
            },
        ],
    ];
    let mut s = Session::new(Scripted(script));
    let now = Instant::now();
    s.send(now, Command::Connect).unwrap();
    let _ = s.poll(now);
    s.send(
        now,
        Command::EnterWorld {
            character: "P".into(),
        },
    )
    .unwrap();
    let out = s.poll(now);
    let made = created(&out);
    assert_eq!(made.len(), 1);
    assert!(
        matches!(made[0], Event::EntityCreated { name, .. } if name == "x y"),
        "names are cleaned"
    );
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, Event::AppearanceChanged { .. }))
            .count(),
        1
    );
}

#[test]
fn rotation_heading_inverts_facing() {
    for h in [-3.0f32, -1.0, 0.0, 0.5, 1.0, 2.0, 3.1] {
        let got = Rotation::facing(h).heading().unwrap();
        let d = (got - h).rem_euclid(TAU);
        assert!(d < 1e-5 || TAU - d < 1e-5, "{h} -> {got}");
    }
    assert_eq!(Rotation::new(0.0, 0.0, 0.0, 0.0).heading(), None);
    assert_eq!(Rotation::new(f32::NAN, 0.0, 0.0, 1.0).heading(), None);
    // Pitched straight up (forward = +Z): the heading of +X turned by π/2.
    let up = Rotation::new(
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
    );
    let h = up.heading().unwrap();
    assert!((h - FRAC_PI_2).abs() < 1e-5, "{h}");
}

proptest! {
    #[test]
    fn heading_is_finite_and_in_range(w in -2.0f32..2.0, x in -2.0f32..2.0, y in -2.0f32..2.0, z in -2.0f32..2.0) {
        if let Some(h) = Rotation::new(w, x, y, z).heading() {
            prop_assert!(h.is_finite());
            prop_assert!(h > -PI - 1e-5 && h <= PI + 1e-5);
        }
    }

    #[test]
    fn crowd_grid_is_bounded(count in 0usize..200, spacing in 0.5f32..10.0, ahead in 0.0f32..50.0) {
        let config = SandboxConfig {
            wanderers: 0,
            crowd: CrowdConfig { count, appearances: vec![human()], spacing, ahead, ..CrowdConfig::default() },
            ..SandboxConfig::default()
        };
        let (_, events, _) = enter(config, None);
        let made = created(&events);
        prop_assert_eq!(made.len(), count);
        for e in made {
            if let Event::EntityCreated { location, heading, .. } = e {
                prop_assert!(location.is_valid() && heading.is_finite());
                let r = (location.x * location.x + location.y * location.y).sqrt();
                prop_assert!(r <= f64::from(ahead) + 30.0 * f64::from(spacing));
            }
        }
    }
}
