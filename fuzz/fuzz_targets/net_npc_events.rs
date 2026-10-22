// SPDX-License-Identifier: GPL-3.0-or-later
//! Character events of the sandbox backend (`tessera-net`): a hostile world source whose
//! static entries carry figures with arbitrary appearances (non-finite scales and dyes,
//! too many items, long names with control characters, any rotation), an arbitrary avatar,
//! arbitrary wanderer and crowd settings, and an arbitrary sequence of moves, teleports
//! and clock steps.
//!
//! Nothing may panic. Every surfaced `EntityCreated` has a valid location, a finite heading
//! within one turn of zero (|θ| ≤ 2π; in (−π, π] for a placed, tagged entity), a valid
//! appearance and a clean name; every `AppearanceChanged` a valid appearance; no identifier
//! is created twice while alive; `StaticsCleared` removes every tagged (placed) entity and
//! static object: no later event (a move, an appearance change, a despawn, a location
//! report) names one of them unless it was created again; the crowd and wanderers never
//! exceed their configured counts. The session ends with a disconnect, so the streamed
//! statics are also dropped when the session closes.
//!
//! The session contract (`tessera-net` `Event::EntityCreated`) gives a heading's units and
//! direction but no range; the sandbox reports placed entities in (−π, π] and simulated
//! ones in other one-turn ranges (wanderers in [0, 2π)), hence the two checks above.

#![no_main]

use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};
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

/// An item: DID, dyes, stowed, slot.
type RawItem = (u32, Option<f32>, Option<f32>, bool, Option<u8>);

#[derive(Debug, Clone, Arbitrary)]
struct RawAppearance {
    character: bool,
    a: u32,
    b: u32,
    scale: f32,
    items: Vec<RawItem>,
    idle: Option<u32>,
    /// Look entries (property, kind byte, payload); invalid looks included.
    look: Option<Vec<(u32, u8, u32)>>,
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
                .map(|&(item, dye, secondary_dye, stowed, slot)| Equipped {
                    item: DataId(item),
                    dye,
                    secondary_dye,
                    stowed,
                    slot,
                })
                .collect(),
            idle: self.idle,
            look: self.look.as_ref().map(|entries| {
                Box::new(tessera_net::Look {
                    entries: entries
                        .iter()
                        .take(64)
                        .map(|&(property, kind, v)| tessera_net::LookEntry {
                            property,
                            value: match kind % 6 {
                                0 => tessera_net::PropertyValue::Bool(v & 1 == 1),
                                1 => tessera_net::PropertyValue::Float(f32::from_bits(v)),
                                2 => tessera_net::PropertyValue::Instance(u64::from(v)),
                                3 => tessera_net::PropertyValue::String(
                                    "x y".repeat(usize::try_from(v % 80).unwrap_or(0)),
                                ),
                                4 => tessera_net::PropertyValue::Colour(v.to_le_bytes()),
                                _ => tessera_net::PropertyValue::Enum(v),
                            },
                        })
                        .collect(),
                })
            }),
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

/// What the events said so far.
#[derive(Default)]
struct Seen {
    /// Identifiers alive.
    live: BTreeSet<u64>,
    /// Alive identifiers with a tag (placed entities and static objects).
    tagged: BTreeSet<u64>,
    /// Identifiers removed by `StaticsCleared` and not created again.
    cleared: BTreeSet<u64>,
    /// Entities the backend simulates itself (wanderers and crowd).
    simulated: usize,
}

impl Seen {
    /// A new identifier: not alive yet; it may reuse one that was cleared.
    fn created(&mut self, id: u64, tagged: bool) {
        assert!(self.live.insert(id), "created twice");
        self.cleared.remove(&id);
        if tagged {
            self.tagged.insert(id);
        }
    }

    /// An event about an existing identifier: none removed by `StaticsCleared`.
    fn about(&self, id: u64, what: &str) {
        assert!(
            !self.cleared.contains(&id),
            "{what} for {id} after StaticsCleared removed it"
        );
    }

    fn event(&mut self, e: Event) {
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
                assert!(
                    heading.abs() <= TAU + 1e-3,
                    "heading {heading} past one turn"
                );
                assert!(appearance.is_valid());
                assert!(name.len() <= MAX_NAME_LEN);
                assert!(name.is_empty() || is_valid_name(&name) || name.trim().is_empty());
                if tag.is_some() {
                    assert!(heading > -PI - 1e-4);
                    assert!(heading <= PI + 1e-4);
                } else {
                    self.simulated += 1;
                }
                self.created(id.0, tag.is_some());
            }
            Event::EntitySpawned { id, .. } => {
                self.created(id.0, false);
                self.simulated += 1;
            }
            Event::StaticSpawned { id, .. } => self.created(id.0, true),
            Event::AppearanceChanged { id, appearance } => {
                assert!(appearance.is_valid());
                self.about(id.0, "AppearanceChanged");
            }
            Event::EntityMoved { id, .. } => self.about(id.0, "EntityMoved"),
            Event::LocationReport { id, .. } => self.about(id.0, "LocationReport"),
            Event::EntityDespawned { id } => {
                self.about(id.0, "EntityDespawned");
                self.live.remove(&id.0);
                self.tagged.remove(&id.0);
            }
            Event::StaticsCleared => {
                for id in std::mem::take(&mut self.tagged) {
                    self.live.remove(&id);
                    self.cleared.insert(id);
                }
            }
            _ => {}
        }
    }
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
    let mut seen = Seen::default();
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
                    on_ground: true,
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
            seen.event(e);
        }
    }
    // The session closes: whatever it still reports keeps the same rules.
    let _ = session.send(now, Command::Disconnect);
    now += Duration::from_millis(100);
    session.poll_into(now, &mut events);
    for e in events.drain(..) {
        seen.event(e);
    }
    assert!(seen.simulated <= wanderers + crowd);
});
