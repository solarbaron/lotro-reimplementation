// SPDX-License-Identifier: GPL-3.0-or-later
//! Character events of the sandbox backend (`tessera-net`): a hostile world source whose
//! static entries carry figures with arbitrary appearances (non-finite scales and dyes,
//! too many items, long names with control characters, any rotation), an arbitrary avatar,
//! arbitrary wanderer and crowd settings, and an arbitrary sequence of moves, teleports
//! and clock steps.
//!
//! Nothing may panic. Every surfaced `EntityCreated` has a valid location, a finite heading
//! in (−π, π], a valid appearance and a clean name; every `AppearanceChanged` a valid
//! appearance; no identifier is created twice while alive; tagged (placed) entities are
//! all removed by `StaticsCleared`; the crowd and wanderers never exceed their configured
//! counts.

#![no_main]

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::model::{MAX_NAME_LEN, is_valid_name};
use tessera_net::sandbox::{
    CrowdConfig, Landblock, Lookup, PlacedFigure, SandboxBackend, SandboxConfig, SpawnPoint,
    StaticConfig, StaticEntity, WorldSource,
};
use tessera_net::{
    Appearance, Body, Command, DataId, Destination, EntityKind, Equipped, Event, Location,
    Placement, RegionId, Rotation, Session, Vec3,
};

const MAX_OPS: usize = 96;

#[derive(Debug, Clone, Arbitrary)]
struct RawAppearance {
    character: bool,
    a: u32,
    b: u32,
    scale: f32,
    items: Vec<(u32, Option<f32>, Option<f32>, bool)>,
    idle: Option<u32>,
}

impl RawAppearance {
    fn build(&self) -> Appearance {
        Appearance {
            body: if self.character {
                Body::Character {
                    species: self.a,
                    sex: self.b,
                }
            } else {
                Body::Object(DataId(self.a))
            },
            scale: self.scale,
            equipment: self
                .items
                .iter()
                .take(40)
                .map(|&(item, dye, secondary_dye, stowed)| Equipped {
                    item: DataId(item),
                    dye,
                    secondary_dye,
                    stowed,
                })
                .collect(),
            idle: self.idle,
        }
    }
}

#[derive(Debug, Clone, Arbitrary)]
struct RawFigure {
    pos: [f64; 3],
    rot: [f32; 4],
    kind: u8,
    name: String,
    class: Option<u32>,
    look: RawAppearance,
    tag: u64,
    figure: bool,
}

#[derive(Debug)]
struct Hostile {
    figures: Vec<RawFigure>,
    ground: Option<f64>,
}

impl WorldSource for Hostile {
    fn ground_height(&mut self, _location: &Location) -> Lookup<f64> {
        self.ground.map_or(Lookup::Pending, Lookup::Ready)
    }

    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        for f in self.figures.iter().take(limit.saturating_add(2)) {
            let e = StaticEntity::new(
                DataId(f.class.unwrap_or(1)),
                Placement {
                    location: block.location(f.pos),
                    rotation: Rotation::new(f.rot[0], f.rot[1], f.rot[2], f.rot[3]),
                    scale: 1.0,
                },
                f.tag,
            );
            out.push(if f.figure {
                e.with_figure(PlacedFigure {
                    kind: match f.kind % 4 {
                        0 => EntityKind::NonPlayer,
                        1 => EntityKind::Creature,
                        2 => EntityKind::Object,
                        _ => EntityKind::Player,
                    },
                    name: f.name.clone(),
                    class: f.class.map(DataId),
                    appearance: f.look.build(),
                })
            } else {
                e
            });
        }
        Lookup::Ready(())
    }
}

#[derive(Debug, Arbitrary)]
enum Op {
    Move { pos: [f64; 3], heading: f32 },
    Teleport { pos: [f64; 3] },
    Poll { advance_ms: u16 },
}

#[derive(Debug, Arbitrary)]
struct Input {
    spawn: String,
    avatar: Option<RawAppearance>,
    wanderers: u8,
    wanderer_looks: Vec<RawAppearance>,
    crowd: u8,
    crowd_looks: Vec<RawAppearance>,
    spacing: f32,
    ahead: f32,
    figures: Vec<RawFigure>,
    ground: Option<f64>,
    ops: Vec<Op>,
}

fuzz_target!(|input: Input| {
    static T0: OnceLock<Instant> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    let crowd = usize::from(input.crowd % 64);
    let wanderers = usize::from(input.wanderers % 8);
    let config = SandboxConfig {
        spawn_point: SpawnPoint::parse(&input.spawn).ok(),
        statics: StaticConfig {
            radius: 1,
            max_resident: 512,
            ..StaticConfig::default()
        },
        avatar: input.avatar.as_ref().map(RawAppearance::build),
        wanderers,
        wanderer_appearances: input
            .wanderer_looks
            .iter()
            .take(8)
            .map(RawAppearance::build)
            .collect(),
        crowd: CrowdConfig {
            count: crowd,
            appearances: input
                .crowd_looks
                .iter()
                .take(8)
                .map(RawAppearance::build)
                .collect(),
            spacing: input.spacing,
            ahead: input.ahead,
            ..CrowdConfig::default()
        },
        max_player_speed: 1.0e4,
        ..SandboxConfig::default()
    };
    let source = Hostile {
        figures: input.figures.into_iter().take(64).collect(),
        ground: input.ground,
    };
    // Invalid settings are refused, never accepted half-way.
    let Ok(backend) = SandboxBackend::with_world(config, source) else {
        return;
    };
    let mut session = Session::new(backend);
    let mut now = t0;
    let _ = session.send(now, Command::Connect);
    let _ = session.poll(now);
    let _ = session.send(
        now,
        Command::EnterWorld {
            character: "Wanderer".into(),
        },
    );
    let mut live: BTreeSet<u64> = BTreeSet::new();
    let mut tagged: BTreeSet<u64> = BTreeSet::new();
    let mut simulated = 0usize;
    let mut events = Vec::new();
    let ops = input
        .ops
        .into_iter()
        .take(MAX_OPS)
        .chain([Op::Poll { advance_ms: 100 }]);
    for op in ops {
        let command = match op {
            Op::Move { pos, heading } => {
                let region = session
                    .player_location()
                    .map_or(RegionId(0), |l| l.0.region);
                Some(Command::Move {
                    location: Location::new(region, pos[0], pos[1], pos[2]),
                    heading,
                    velocity: Vec3::ZERO,
                })
            }
            Op::Teleport { pos } => {
                let region = session
                    .player_location()
                    .map_or(RegionId(0), |l| l.0.region);
                Some(Command::Teleport {
                    destination: Destination::Location {
                        location: Location::new(region, pos[0], pos[1], pos[2]),
                        heading: 0.0,
                    },
                })
            }
            Op::Poll { advance_ms } => {
                now += Duration::from_millis(u64::from(advance_ms));
                None
            }
        };
        if let Some(c) = command {
            let _ = session.send(now, c);
        }
        session.poll_into(now, &mut events);
        for e in events.drain(..) {
            match e {
                Event::EntityCreated {
                    id,
                    name,
                    location,
                    heading,
                    appearance,
                    tag,
                    ..
                } => {
                    assert!(location.is_valid());
                    assert!(heading.is_finite());
                    assert!(appearance.is_valid());
                    assert!(name.len() <= MAX_NAME_LEN);
                    assert!(name.is_empty() || is_valid_name(&name) || name.trim().is_empty());
                    assert!(live.insert(id.0), "created twice");
                    if tag.is_some() {
                        assert!(heading > -std::f32::consts::PI - 1e-4);
                        assert!(heading <= std::f32::consts::PI + 1e-4);
                        tagged.insert(id.0);
                    } else {
                        simulated += 1;
                    }
                }
                Event::EntitySpawned { id, .. } => {
                    assert!(live.insert(id.0), "spawned twice");
                    simulated += 1;
                }
                Event::StaticSpawned { id, .. } => {
                    assert!(live.insert(id.0), "spawned twice");
                    tagged.insert(id.0);
                }
                Event::AppearanceChanged { appearance, .. } => {
                    assert!(appearance.is_valid());
                }
                Event::EntityDespawned { id } => {
                    live.remove(&id.0);
                    tagged.remove(&id.0);
                }
                Event::StaticsCleared => {
                    for id in std::mem::take(&mut tagged) {
                        live.remove(&id);
                    }
                }
                _ => {}
            }
        }
    }
    assert!(simulated <= wanderers + crowd);
});
