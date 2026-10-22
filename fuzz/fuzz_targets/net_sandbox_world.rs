// SPDX-License-Identifier: GPL-3.0-or-later
//! The sandbox backend with a hostile world source (`tessera-net` `sandbox::world`):
//! an arbitrary spawn-point text (`SpawnPoint::parse`), arbitrary static-entity
//! settings, a world source whose every answer (place lookups, ground heights,
//! static entities including non-finite, wrong-region and oversized lists, location
//! descriptions) is chosen by the fuzzer, and an arbitrary sequence of moves,
//! teleports (exact, ground, anchored, parsed text, named; overlapping ones),
//! location queries, clock steps and polls.
//!
//! Nothing may panic. Every surfaced static placement and location report must be
//! valid with a unit rotation, every reported anchor must own its location, no
//! identifier is spawned twice, and live statics (spawned minus despawned or
//! cleared) never exceed the configured resident budget.

#![no_main]

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::sandbox::{
    Landblock, Lookup, ResolvedPlace, SandboxBackend, SandboxConfig, SpawnPoint, StaticConfig,
    StaticEntity, WorldSource,
};
use tessera_net::{
    Anchor, Command, DataId, Destination, Event, Location, Placement, RegionId, Rotation, Session,
    Vec3,
};

const MAX_OPS: usize = 128;

#[derive(Debug, Clone, Arbitrary)]
enum Answer<T> {
    Ready(T),
    Pending,
    Unavailable(String),
}

impl<T> Answer<T> {
    fn lookup(self) -> Lookup<T> {
        match self {
            Self::Ready(v) => Lookup::Ready(v),
            Self::Pending => Lookup::Pending,
            Self::Unavailable(s) => Lookup::Unavailable(s),
        }
    }
}

#[derive(Debug, Clone, Arbitrary)]
struct RawEntity {
    object: u32,
    region_delta: u8,
    pos: [f64; 3],
    rot: [f32; 4],
    scale: f32,
    tag: u64,
}

/// Static-entity answer: either a list or a "flood" of `n` copies, which
/// ignores the limit.
#[derive(Debug, Clone, Arbitrary)]
enum Statics {
    List(Vec<RawEntity>),
    Flood(RawEntity, u16),
}

/// A resolved place: region, position, facing, anchor (block x, y, cell)
/// and whether it snaps.
type RawPlace = (u16, [f64; 3], Option<f32>, Option<(u8, u8, u16)>, bool);

#[derive(Debug, Clone, Arbitrary)]
struct Script {
    places: Vec<Answer<RawPlace>>,
    heights: Vec<Answer<f64>>,
    statics: Vec<Answer<Statics>>,
    describe: Vec<Option<String>>,
}

#[derive(Debug)]
struct Hostile {
    script: Script,
    calls: usize,
}

impl Hostile {
    fn pick<T: Clone>(list: &[T], n: usize) -> Option<T> {
        (!list.is_empty()).then(|| list[n % list.len()].clone())
    }
}

impl WorldSource for Hostile {
    fn resolve_place(&mut self, _name: &str) -> Lookup<ResolvedPlace> {
        self.calls += 1;
        match Self::pick(&self.script.places, self.calls) {
            Some(a) => a.lookup().map_ready(|(r, p, heading, anchor, snap)| {
                let location = Location::new(RegionId(r), p[0], p[1], p[2]);
                let mut place = if snap {
                    ResolvedPlace::on_ground(location)
                } else {
                    ResolvedPlace::exact(location)
                };
                if let Some(h) = heading {
                    place = place.with_heading(h);
                }
                // Possibly in another region or not owning the location.
                if let Some((x, y, cell)) = anchor
                    && let Some(block) = Landblock::new(RegionId(r), x, y)
                {
                    place = place.with_anchor(Anchor::new(block, cell));
                }
                place
            }),
            None => Lookup::Pending,
        }
    }

    fn ground_height(&mut self, _location: &Location) -> Lookup<f64> {
        self.calls += 1;
        Self::pick(&self.script.heights, self.calls).map_or(Lookup::Pending, Answer::lookup)
    }

    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        self.calls += 1;
        let Some(answer) = Self::pick(&self.script.statics, self.calls) else {
            return Lookup::Ready(());
        };
        let to_entity = |e: &RawEntity| {
            StaticEntity::new(
                DataId(e.object),
                Placement {
                    location: Location::new(
                        RegionId(block.region.0.wrapping_add(u16::from(e.region_delta % 2))),
                        e.pos[0],
                        e.pos[1],
                        e.pos[2],
                    ),
                    rotation: Rotation::new(e.rot[0], e.rot[1], e.rot[2], e.rot[3]),
                    scale: e.scale,
                },
                e.tag,
            )
        };
        match answer {
            Answer::Ready(Statics::List(list)) => {
                out.extend(list.iter().take(limit.saturating_add(4)).map(to_entity));
                Lookup::Ready(())
            }
            Answer::Ready(Statics::Flood(e, n)) => {
                out.extend(std::iter::repeat_n(to_entity(&e), usize::from(n)));
                Lookup::Ready(())
            }
            Answer::Pending => Lookup::Pending,
            Answer::Unavailable(s) => Lookup::Unavailable(s),
        }
    }

    fn describe(
        &mut self,
        _location: &Location,
        _heading: f32,
        _anchor: Option<Anchor>,
    ) -> Option<String> {
        self.calls += 1;
        Self::pick(&self.script.describe, self.calls).flatten()
    }
}

trait MapReady<T> {
    fn map_ready<U>(self, f: impl FnOnce(T) -> U) -> Lookup<U>;
}

impl<T> MapReady<T> for Lookup<T> {
    fn map_ready<U>(self, f: impl FnOnce(T) -> U) -> Lookup<U> {
        match self {
            Lookup::Ready(v) => Lookup::Ready(f(v)),
            Lookup::Pending => Lookup::Pending,
            Lookup::Unavailable(s) => Lookup::Unavailable(s),
        }
    }
}

#[derive(Debug, Arbitrary)]
enum Op {
    Move {
        pos: [f64; 3],
        heading: f32,
    },
    Teleport {
        region: u16,
        pos: [f64; 3],
        heading: f32,
    },
    TeleportGround {
        region: u16,
        pos: [f64; 3],
    },
    TeleportAnchored {
        region: u16,
        pos: [f64; 3],
        block: (u8, u8, u16),
    },
    /// A destination in the spawn-point text form.
    TeleportText(String),
    TeleportNamed(String),
    Query,
    Poll {
        advance_ms: u16,
    },
    Disconnect,
}

#[derive(Debug, Arbitrary)]
struct Input {
    spawn: String,
    radius: u8,
    max_per_block: u16,
    max_resident: u16,
    spawns_per_poll: u8,
    despawns_per_poll: u8,
    queries_per_poll: u8,
    timeout_ms: u16,
    latency_ms: u8,
    script: Script,
    ops: Vec<Op>,
}

fuzz_target!(|input: Input| {
    static T0: OnceLock<Instant> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    let spawn_point = SpawnPoint::parse(&input.spawn).ok();
    let statics = StaticConfig {
        radius: input.radius % 4,
        max_per_block: usize::from(input.max_per_block % 512) + 1,
        max_resident: usize::from(input.max_resident % 2048) + 1,
        spawns_per_poll: usize::from(input.spawns_per_poll).max(1),
        despawns_per_poll: usize::from(input.despawns_per_poll).max(1),
        queries_per_poll: usize::from(input.queries_per_poll % 16) + 1,
    };
    let max_resident = statics.max_resident;
    let config = SandboxConfig {
        spawn_point,
        statics,
        resolve_timeout: Duration::from_millis(u64::from(input.timeout_ms)),
        latency: Duration::from_millis(u64::from(input.latency_ms)),
        wanderers: 1,
        max_player_speed: 1.0e4,
        ..SandboxConfig::default()
    };
    let source = Hostile {
        script: input.script,
        calls: 0,
    };
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
    let mut events = Vec::new();
    for op in input.ops.into_iter().take(MAX_OPS) {
        let command = match op {
            Op::Move { pos, heading } => {
                let region = session
                    .player_location()
                    .map_or(RegionId(0), |l| l.0.region);
                Some(Command::Move {
                    location: Location::new(region, pos[0], pos[1], pos[2]),
                    heading,
                    velocity: Vec3::ZERO,
                    on_ground: true,
                })
            }
            Op::TeleportGround { region, pos } => Some(Command::Teleport {
                destination: Destination::Ground {
                    location: Location::new(RegionId(region), pos[0], pos[1], pos[2]),
                    heading: 0.0,
                },
            }),
            Op::TeleportAnchored { region, pos, block } => {
                Landblock::new(RegionId(region), block.0, block.1).map(|b| Command::Teleport {
                    destination: Destination::Anchored {
                        location: Location::new(RegionId(region), pos[0], pos[1], pos[2]),
                        heading: 0.0,
                        anchor: Anchor::new(b, block.2),
                    },
                })
            }
            Op::TeleportText(text) => SpawnPoint::parse(&text).ok().map(|p| Command::Teleport {
                destination: p.destination,
            }),
            Op::Teleport {
                region,
                pos,
                heading,
            } => Some(Command::Teleport {
                destination: Destination::Location {
                    location: Location::new(RegionId(region), pos[0], pos[1], pos[2]),
                    heading,
                },
            }),
            Op::TeleportNamed(name) => Some(Command::Teleport {
                destination: Destination::Named(name),
            }),
            Op::Query => Some(Command::QueryLocation),
            Op::Disconnect => Some(Command::Disconnect),
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
                Event::StaticSpawned { id, placement, .. } => {
                    assert!(placement.is_valid(), "invalid placement {placement:?}");
                    assert!((placement.rotation.length() - 1.0).abs() < 1e-3);
                    assert!(live.insert(id.0), "static spawned twice");
                    assert!(live.len() <= max_resident, "resident budget exceeded");
                }
                Event::EntityDespawned { id } => {
                    live.remove(&id.0);
                }
                Event::StaticsCleared => live.clear(),
                Event::LocationReport {
                    location,
                    heading,
                    anchor,
                    ..
                } => {
                    assert!(location.is_valid() && heading.is_finite());
                    assert!(anchor.is_none_or(|a| a.owns(&location)));
                }
                Event::Disconnected { .. } => live.clear(),
                _ => {}
            }
        }
    }
});
