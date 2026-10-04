// SPDX-License-Identifier: GPL-3.0-or-later
//! The sandbox with world data: spawn points, location reports, teleports and
//! static entity streaming through a scripted [`WorldSource`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_net::sandbox::world::{GRID_EXTENT, LANDBLOCK_SIZE, MAX_LOCAL_OFFSET, MAX_SOURCE_TEXT};
use tessera_net::sandbox::{
    Landblock, Lookup, ResolvedPlace, SandboxBackend, SandboxConfig, SpawnPoint, StaticConfig,
    StaticEntity, StaticStats, WorldSource,
};
use tessera_net::{
    Anchor, Backend, ChatChannel, Command, DataId, Destination, EntityId, Event, Location,
    LocationCause, NetError, Placement, RegionId, Rotation, Session, SessionState, Transform, Vec3,
};

const R1: RegionId = RegionId(1);
const R2: RegionId = RegionId(2);

/// What the fake source saw, shared with the test.
#[derive(Debug, Default)]
struct Log {
    queries: Vec<Landblock>,
    released: Vec<Landblock>,
    limits: Vec<usize>,
    /// Anchors passed to `describe`.
    described: Vec<Option<Anchor>>,
}

/// How the fake source answers.
#[derive(Debug, Clone)]
struct Script {
    /// Places resolved on the ground.
    places: BTreeMap<String, Location>,
    /// Places resolved as given (exact, with heading or anchor).
    resolved: BTreeMap<String, ResolvedPlace>,
    /// Polls answered `Pending` before a place resolves.
    place_delay: u32,
    /// `None`: heights unsupported. `Some(h)`: flat ground at `h`.
    ground: Option<f64>,
    ground_delay: u32,
    /// Objects per block.
    per_block: usize,
    /// Polls answered `Pending` before a block's objects are ready.
    block_delay: u32,
    /// Blocks that answer `Unavailable`.
    missing: BTreeSet<Landblock>,
    /// Hostile mode: append invalid entities too, and ignore `limit`.
    hostile: bool,
    describe: Option<String>,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            places: BTreeMap::new(),
            resolved: BTreeMap::new(),
            place_delay: 0,
            ground: Some(123.5),
            ground_delay: 0,
            per_block: 3,
            block_delay: 0,
            missing: BTreeSet::new(),
            hostile: false,
            describe: None,
        }
    }
}

#[derive(Debug)]
struct FakeWorld {
    script: Script,
    log: Arc<Mutex<Log>>,
    place_asks: u32,
    ground_asks: u32,
    block_asks: BTreeMap<Landblock, u32>,
}

impl FakeWorld {
    fn new(script: Script) -> (Self, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log::default()));
        (
            Self {
                script,
                log: Arc::clone(&log),
                place_asks: 0,
                ground_asks: 0,
                block_asks: BTreeMap::new(),
            },
            log,
        )
    }
}

/// Encodes a block and an index into a tag, so tests can tell where a
/// spawned object came from.
fn tag(block: Landblock, i: usize) -> u64 {
    (u64::from(block.region.0) << 40)
        | (u64::from(block.x) << 32)
        | (u64::from(block.y) << 24)
        | i as u64
}

fn tag_block(tag: u64) -> (u16, u8, u8) {
    (
        (tag >> 40) as u16,
        ((tag >> 32) & 0xFF) as u8,
        ((tag >> 24) & 0xFF) as u8,
    )
}

impl WorldSource for FakeWorld {
    fn resolve_place(&mut self, name: &str) -> Lookup<ResolvedPlace> {
        if name.starts_with("Slow") {
            return Lookup::Pending;
        }
        self.place_asks += 1;
        if self.place_asks <= self.script.place_delay {
            return Lookup::Pending;
        }
        if let Some(p) = self.script.resolved.get(name) {
            return Lookup::Ready(*p);
        }
        match self.script.places.get(name) {
            Some(l) => Lookup::Ready(ResolvedPlace::on_ground(*l)),
            None => Lookup::Unavailable(format!("no place called {name}")),
        }
    }

    fn ground_height(&mut self, _location: &Location) -> Lookup<f64> {
        self.ground_asks += 1;
        if self.ground_asks <= self.script.ground_delay {
            return Lookup::Pending;
        }
        match self.script.ground {
            Some(h) => Lookup::Ready(h),
            None => Lookup::Unavailable("flat world".into()),
        }
    }

    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        {
            let mut log = self.log.lock().unwrap();
            log.queries.push(block);
            log.limits.push(limit);
        }
        let asks = self.block_asks.entry(block).or_insert(0);
        *asks += 1;
        if *asks <= self.script.block_delay {
            return Lookup::Pending;
        }
        if self.script.missing.contains(&block) {
            return Lookup::Unavailable("no such block".into());
        }
        let n = if self.script.hostile {
            self.script.per_block
        } else {
            self.script.per_block.min(limit)
        };
        for i in 0..n {
            out.push(StaticEntity {
                object: DataId(0x7000_0000 + i as u32),
                placement: Placement {
                    location: block.location([10.0 + i as f64, 20.0, 50.0]),
                    rotation: Rotation::new(2.0, 0.0, 0.0, 0.0),
                    scale: 1.0,
                },
                tag: tag(block, i),
            });
        }
        if self.script.hostile {
            let good = block.location([1.0, 1.0, 1.0]);
            let bad = |placement: Placement| StaticEntity {
                object: DataId(1),
                placement,
                tag: u64::MAX,
            };
            let p = Placement {
                location: good,
                rotation: Rotation::IDENTITY,
                scale: 1.0,
            };
            out.push(bad(Placement {
                location: Location::new(block.region, f64::NAN, 0.0, 0.0),
                ..p
            }));
            out.push(bad(Placement {
                location: Location::new(RegionId(999), good.x, good.y, good.z),
                ..p
            }));
            out.push(bad(Placement { scale: 0.0, ..p }));
            out.push(bad(Placement {
                scale: f32::INFINITY,
                ..p
            }));
            out.push(bad(Placement { scale: 1.0e9, ..p }));
            out.push(bad(Placement {
                rotation: Rotation::new(0.0, 0.0, 0.0, 0.0),
                ..p
            }));
            out.push(bad(Placement {
                rotation: Rotation::new(f32::NAN, 0.0, 0.0, 1.0),
                ..p
            }));
            out.push(bad(Placement {
                location: Location::new(block.region, 1.0e300, 0.0, 0.0),
                ..p
            }));
        }
        Lookup::Ready(())
    }

    fn release(&mut self, block: Landblock) {
        self.log.lock().unwrap().released.push(block);
    }

    fn describe(
        &mut self,
        location: &Location,
        _heading: f32,
        anchor: Option<Anchor>,
    ) -> Option<String> {
        self.log.lock().unwrap().described.push(anchor);
        self.script
            .describe
            .clone()
            .map(|d| format!("{d} @ {:.0},{:.0}", location.x, location.y))
    }
}

struct Harness {
    session: Session,
    now: Instant,
    log: Arc<Mutex<Log>>,
}

impl Harness {
    fn new(config: SandboxConfig, script: Script) -> Self {
        let (world, log) = FakeWorld::new(script);
        let backend = SandboxBackend::with_world(config, world).unwrap();
        Self {
            session: Session::new(backend),
            now: Instant::now(),
            log,
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        self.session.poll(self.now)
    }

    fn step(&mut self, ms: u64) -> Vec<Event> {
        self.now += Duration::from_millis(ms);
        self.poll()
    }

    fn send(&mut self, command: Command) -> Result<(), NetError> {
        self.session.send(self.now, command)
    }

    /// Connect and enter; returns the events of the entering poll.
    fn enter(&mut self) -> Vec<Event> {
        self.send(Command::Connect).unwrap();
        let _ = self.poll();
        self.send(Command::EnterWorld {
            character: "Wanderer".into(),
        })
        .unwrap();
        self.poll()
    }

    /// Moves the player (accepted: the config allows fast movement).
    fn move_to(&mut self, location: Location) -> Vec<Event> {
        self.now += Duration::from_secs(1);
        self.send(Command::Move {
            location,
            heading: 0.0,
            velocity: Vec3::ZERO,
        })
        .unwrap();
        self.poll()
    }

    /// Polls `n` more frames, collecting the events.
    fn run(&mut self, n: usize) -> Vec<Event> {
        let mut all = Vec::new();
        for _ in 0..n {
            all.extend(self.step(16));
        }
        all
    }

    fn stats(&self) -> StaticStats {
        self.session
            .backend::<SandboxBackend>()
            .expect("sandbox backend")
            .static_stats()
    }
}

/// The client's view of live statics: id -> source block, kept from events.
#[derive(Debug, Default)]
struct Live(BTreeMap<EntityId, (u16, u8, u8, u64)>);

impl Live {
    fn apply(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::StaticSpawned { id, tag, .. } => {
                    let (r, x, y) = tag_block(*tag);
                    assert!(
                        self.0.insert(*id, (r, x, y, *tag)).is_none(),
                        "spawned twice"
                    );
                }
                Event::EntityDespawned { id } => {
                    self.0.remove(id);
                }
                Event::StaticsCleared => self.0.clear(),
                _ => {}
            }
        }
        let tags: BTreeSet<u64> = self.0.values().map(|v| v.3).collect();
        assert_eq!(tags.len(), self.0.len(), "an object is live twice");
    }

    fn in_block(&self, x: u8, y: u8) -> usize {
        self.0
            .values()
            .filter(|(_, bx, by, _)| (*bx, *by) == (x, y))
            .count()
    }
}

fn config(spawn: &str) -> SandboxConfig {
    SandboxConfig {
        spawn_point: Some(SpawnPoint::parse(spawn).unwrap()),
        wanderers: 0,
        max_player_speed: 10_000.0,
        statics: StaticConfig {
            radius: 1,
            ..StaticConfig::default()
        },
        ..SandboxConfig::default()
    }
}

fn reports(events: &[Event]) -> Vec<(Location, LocationCause)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::LocationReport {
                location, cause, ..
            } => Some((*location, *cause)),
            _ => None,
        })
        .collect()
}

fn statics(events: &[Event]) -> Vec<(EntityId, u64, Placement)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::StaticSpawned {
                id, tag, placement, ..
            } => Some((*id, *tag, *placement)),
            _ => None,
        })
        .collect()
}

fn despawned(events: &[Event]) -> BTreeSet<EntityId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::EntityDespawned { id } => Some(*id),
            _ => None,
        })
        .collect()
}

fn system_texts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::ChatMessage {
                channel: ChatChannel::System,
                text,
                ..
            } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn block(x: u8, y: u8) -> Landblock {
    Landblock::new(R1, x, y).unwrap()
}

#[test]
fn spawn_at_landblock_snaps_to_ground_and_reports() {
    let mut h = Harness::new(config("1,126,140,80,20"), Script::default());
    let ev = h.enter();
    let Event::WorldEntered {
        player, transform, ..
    } = &ev[0]
    else {
        panic!("expected WorldEntered first, got {ev:?}");
    };
    let expected = Location::new(R1, 20_240.0, 22_420.0, 123.5);
    assert_eq!(transform.position, expected.position());
    assert_eq!(
        ev[1],
        Event::LocationReport {
            id: *player,
            location: expected,
            heading: 0.0,
            anchor: None,
            cause: LocationCause::Entered
        }
    );
    assert_eq!(h.session.player_location(), Some((expected, 0.0)));
}

#[test]
fn exact_height_is_not_snapped() {
    let mut h = Harness::new(config("1,126,140,80,20,400.25"), Script::default());
    let ev = h.enter();
    assert_eq!(
        reports(&ev),
        [(
            Location::new(R1, 20_240.0, 22_420.0, 400.25),
            LocationCause::Entered
        )]
    );
}

#[test]
fn named_place_waits_for_the_source() {
    let place = Location::new(R2, 1_000.0, 2_000.0, 0.0);
    let script = Script {
        places: [("Somewhere".to_owned(), place)].into(),
        place_delay: 3,
        ground_delay: 2,
        ..Script::default()
    };
    let mut cfg = config("name:Somewhere");
    cfg.spawn.heading = 1.25;
    let mut h = Harness::new(cfg, script);
    // Submit asks once (pending), then polls: 2 more pending place answers,
    // then ground pending twice.
    assert!(h.enter().is_empty());
    for _ in 0..3 {
        assert!(h.step(16).is_empty());
        assert_eq!(h.session.state(), SessionState::EnteringWorld);
    }
    let ev = h.step(16);
    assert!(matches!(ev[0], Event::WorldEntered { .. }), "{ev:?}");
    let want = Location { z: 123.5, ..place };
    assert_eq!(reports(&ev), [(want, LocationCause::Entered)]);
    assert_eq!(h.session.player_location(), Some((want, 1.25)));
}

#[test]
fn unknown_place_falls_back_to_default_spawn() {
    let mut cfg = config("name:Nowhere");
    cfg.region = RegionId(5);
    cfg.spawn = Transform::new(Vec3::new(1.0, 2.0, 3.0), 0.5);
    let mut h = Harness::new(cfg, Script::default());
    let ev = h.enter();
    assert_eq!(
        reports(&ev),
        [(
            Location::new(RegionId(5), 1.0, 2.0, 3.0),
            LocationCause::Entered
        )]
    );
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.contains("no place called Nowhere") && t.contains("default"))
    );
}

#[test]
fn named_place_without_source_falls_back() {
    let backend = SandboxBackend::new(config("Somewhere")).unwrap();
    let mut s = Session::new(backend);
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Wanderer".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t);
    assert_eq!(
        reports(&ev),
        [(
            Location::new(RegionId(0), 0.0, 0.0, 0.0),
            LocationCause::Entered
        )]
    );
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.contains("no world data"))
    );
}

#[test]
fn resolution_times_out() {
    let script = Script {
        place_delay: u32::MAX,
        ..Script::default()
    };
    let mut cfg = config("Slow Town");
    cfg.resolve_timeout = Duration::from_millis(100);
    let mut h = Harness::new(cfg, script);
    assert!(h.enter().is_empty());
    assert!(h.step(50).is_empty());
    let ev = h.step(60);
    assert!(matches!(ev[0], Event::WorldEntered { .. }), "{ev:?}");
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.contains("not resolved in time"))
    );

    // Ground heights that never come keep the given height.
    let script = Script {
        ground_delay: u32::MAX,
        ..Script::default()
    };
    let mut cfg = config("1,10,10,5,5");
    cfg.resolve_timeout = Duration::from_millis(100);
    let mut h = Harness::new(cfg, script);
    assert!(h.enter().is_empty());
    let ev = h.step(100);
    assert_eq!(
        reports(&ev),
        [(
            Location::new(R1, 1605.0, 1605.0, 0.0),
            LocationCause::Entered
        )]
    );
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.contains("Ground height"))
    );
}

#[test]
fn disconnect_while_entering_is_clean() {
    let script = Script {
        place_delay: u32::MAX,
        ..Script::default()
    };
    let mut h = Harness::new(config("Slow Town"), script);
    assert!(h.enter().is_empty());
    h.send(Command::Disconnect).unwrap();
    let ev = h.step(16);
    assert!(
        matches!(ev.as_slice(), [Event::Disconnected { .. }]),
        "{ev:?}"
    );
    assert!(h.step(20_000).is_empty());
}

#[test]
fn statics_stream_nearest_first_within_budget() {
    let mut cfg = config("1,100,100");
    cfg.statics.spawns_per_poll = 4;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 3,
            ..Script::default()
        },
    );
    let mut all = Vec::new();
    let ev = h.enter();
    all.extend(statics(&ev));
    assert_eq!(all.len(), 4, "budget per poll");
    for _ in 0..10 {
        all.extend(statics(&h.step(16)));
    }
    assert_eq!(all.len(), 9 * 3);
    // The centre block's objects come first.
    for (_, tag, _) in &all[..3] {
        assert_eq!(tag_block(*tag), (1, 100, 100));
    }
    let blocks: BTreeSet<_> = all.iter().map(|(_, t, _)| tag_block(*t)).collect();
    assert_eq!(blocks.len(), 9);
    for (_, _, p) in &all {
        // Rotations are normalised by the sandbox.
        assert!((p.rotation.length() - 1.0).abs() < 1e-6);
        assert_eq!(p.location.region, R1);
    }
    // Identifiers are unique and distinct from the player.
    let ids: BTreeSet<_> = all.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(ids.len(), all.len());
    assert!(!ids.contains(&h.session.player().unwrap()));
    // Queries per poll were bounded (8 by default), so 9 blocks took two polls.
    assert_eq!(h.log.lock().unwrap().queries.len(), 9);
}

#[test]
fn statics_follow_the_player_with_hysteresis() {
    let mut h = Harness::new(config("1,100,100"), Script::default());
    let mut spawned: BTreeMap<EntityId, (u16, u8, u8)> = BTreeMap::new();
    let record = |ev: &[Event], spawned: &mut BTreeMap<EntityId, (u16, u8, u8)>| {
        for (id, tag, _) in statics(ev) {
            spawned.insert(id, tag_block(tag));
        }
        for id in despawned(ev) {
            assert!(spawned.remove(&id).is_some(), "despawn of unknown {id:?}");
        }
    };
    let ev = h.enter();
    record(&ev, &mut spawned);
    let ev = h.step(16);
    record(&ev, &mut spawned);
    assert_eq!(spawned.len(), 27);

    // One block east: three new blocks, nothing released (hysteresis).
    let ev = h.move_to(block(101, 100).location([80.0, 80.0, 0.0]));
    assert!(despawned(&ev).is_empty());
    record(&ev, &mut spawned);
    let blocks: BTreeSet<_> = spawned.values().copied().collect();
    assert_eq!(blocks.len(), 12);
    assert!(h.log.lock().unwrap().released.is_empty());

    // Two more blocks east: the west column (x = 99, 100) is now 3+ away.
    let ev = h.move_to(block(103, 100).location([80.0, 80.0, 0.0]));
    record(&ev, &mut spawned);
    let _ = h.step(16);
    let blocks: BTreeSet<_> = spawned.values().copied().collect();
    assert!(blocks.iter().all(|(_, x, _)| *x >= 101), "{blocks:?}");
    let released = h.log.lock().unwrap().released.clone();
    assert_eq!(released.len(), 6, "{released:?}");
    assert!(released.iter().all(|b| b.x <= 100));
}

#[test]
fn pending_and_missing_blocks() {
    let script = Script {
        block_delay: 2,
        missing: [block(99, 99)].into(),
        ..Script::default()
    };
    let mut cfg = config("1,100,100");
    cfg.statics.queries_per_poll = 16;
    let mut h = Harness::new(cfg, script);
    let ev = h.enter();
    assert!(statics(&ev).is_empty());
    assert!(statics(&h.step(16)).is_empty());
    let ev = h.step(16);
    assert_eq!(statics(&ev).len(), 8 * 3, "the missing block has none");
    // Answered blocks are not asked again.
    let before = h.log.lock().unwrap().queries.len();
    for _ in 0..5 {
        let _ = h.step(16);
    }
    assert_eq!(h.log.lock().unwrap().queries.len(), before);
    assert_eq!(h.session.state(), SessionState::InWorld);
}

#[test]
fn hostile_statics_are_filtered_and_capped() {
    let mut cfg = config("1,100,100");
    cfg.statics.max_per_block = 5;
    cfg.statics.max_resident = 12;
    cfg.statics.radius = 1;
    let script = Script {
        per_block: 4,
        hostile: true,
        ..Script::default()
    };
    let (world, log) = FakeWorld::new(script);
    let backend = SandboxBackend::with_world(cfg, world).unwrap();
    let mut s = Session::new(backend);
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Wanderer".into(),
        },
    )
    .unwrap();
    let mut all = statics(&s.poll(t));
    for i in 1..5 {
        all.extend(statics(&s.poll(t + Duration::from_millis(i * 16))));
    }
    // Each block returns 4 good + 8 bad = 12, cut to 5 = 4 good + 1 bad (NaN);
    // the resident cap stops at 12 good ones.
    assert_eq!(all.len(), 12);
    assert!(
        all.iter()
            .all(|(_, tag, p)| *tag != u64::MAX && p.is_valid())
    );
    // Asked for one more than taken, to see the cut; a block for which there
    // is no room is not asked at all (it used to be asked with limit 0 and
    // then never again).
    // The third block only had room for 4 (5 asked).
    let limits = log.lock().unwrap().limits.clone();
    assert_eq!(limits, [6, 6, 5]);
    let stats = s.backend::<SandboxBackend>().unwrap().static_stats();
    assert_eq!(stats.resident, 12);
    assert_eq!(
        stats.truncated,
        2 * 7,
        "12 returned, 5 taken, per full block"
    );
    assert_eq!(stats.rejected, 2);
    assert_eq!(
        stats.starved_blocks, 7,
        "the part-served block and the six unserved ones wait for room"
    );
}

#[test]
fn hostile_statics_stats() {
    let mut cfg = config("1,0,0");
    cfg.statics.radius = 0;
    cfg.statics.max_per_block = 100;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 2,
            hostile: true,
            ..Script::default()
        },
    );
    // Through the Session, which is how a client holds the backend.
    let out = h.enter();
    let stats = h.stats();
    assert_eq!(stats.blocks, 1);
    assert_eq!(stats.resident, 2);
    assert_eq!(stats.rejected, 8);
    assert_eq!(stats.truncated, 0);
    assert_eq!(stats.starved_blocks, 0);
    assert_eq!(statics(&out).len(), 2);
    let sandbox = h.session.backend::<SandboxBackend>().unwrap();
    assert!(sandbox.world_source().is_some());
    assert_eq!(sandbox.queued_events(), 0);
    // Not a sandbox: no downcast.
    assert!(h.session.backend::<Scripted>().is_none());
}

#[test]
fn teleport_across_regions_replaces_statics() {
    let elsewhere = Location::new(R2, 5_000.0, 5_000.0, 0.0);
    let script = Script {
        places: [("Far Away".to_owned(), elsewhere)].into(),
        ..Script::default()
    };
    let mut h = Harness::new(config("1,100,100"), script);
    let mut ev = h.enter();
    ev.extend(h.step(16));
    let old: BTreeSet<_> = statics(&ev).into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(old.len(), 27);

    h.send(Command::Teleport {
        destination: Destination::Named("Far Away".into()),
    })
    .unwrap();
    let mut ev = h.poll();
    ev.extend(h.step(16));
    let want = Location {
        z: 123.5,
        ..elsewhere
    };
    assert_eq!(reports(&ev), [(want, LocationCause::Teleported)]);
    let player = h.session.player().unwrap();
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::EntityMoved { id, transform, .. } if *id == player && transform.position == want.position()
    )));
    // One clear event instead of 27 despawns.
    assert!(despawned(&ev).is_empty());
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, Event::StaticsCleared))
            .count(),
        1
    );
    let new = statics(&ev);
    assert_eq!(new.len(), 27);
    assert!(new.iter().all(|(_, tag, _)| tag_block(*tag).0 == 2));
    assert_eq!(h.session.player_location().unwrap().0, want);

    // A move keeps the new region.
    let _ = h.move_to(Location::new(R2, 5_010.0, 5_000.0, 0.0));
    assert_eq!(
        h.session.player_location().unwrap().0,
        Location::new(R2, 5_010.0, 5_000.0, 0.0)
    );

    // An exact teleport is not snapped; unknown names leave the player put.
    let exact = Location::new(R2, 6_000.5, 6_000.25, 77.0);
    h.send(Command::Teleport {
        destination: Destination::Location {
            location: exact,
            heading: 2.0,
        },
    })
    .unwrap();
    assert_eq!(reports(&h.poll()), [(exact, LocationCause::Teleported)]);
    assert_eq!(h.session.player_location(), Some((exact, 2.0)));
    h.send(Command::Teleport {
        destination: Destination::Named("Atlantis".into()),
    })
    .unwrap();
    let ev = h.poll();
    assert!(reports(&ev).is_empty());
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.starts_with("Teleport failed"))
    );
    assert_eq!(h.session.player_location(), Some((exact, 2.0)));
}

#[test]
fn query_location_reports_and_describes() {
    let script = Script {
        describe: Some("Somewhere\u{7}nice".into()),
        ..Script::default()
    };
    let mut h = Harness::new(config("1,126,140,80,20,10"), script);
    let _ = h.enter();
    h.send(Command::QueryLocation).unwrap();
    let ev = h.poll();
    let here = Location::new(R1, 20_240.0, 22_420.0, 10.0);
    assert_eq!(reports(&ev), [(here, LocationCause::Requested)]);
    assert_eq!(system_texts(&ev), ["Somewhere nice @ 20240,22420"]);

    // Long descriptions are cut; without a description a default line is sent.
    let script = Script {
        describe: Some("y".repeat(10_000)),
        ..Script::default()
    };
    let mut h = Harness::new(config("1,0,0,1,1,1"), script);
    let _ = h.enter();
    h.send(Command::QueryLocation).unwrap();
    let texts = system_texts(&h.poll());
    assert!(texts[0].len() <= MAX_SOURCE_TEXT);
    let mut h = Harness::new(config("1,0,0,1,1,1"), Script::default());
    let _ = h.enter();
    h.send(Command::QueryLocation).unwrap();
    assert_eq!(
        system_texts(&h.poll()),
        ["Region 1, x 1.00, y 1.00, z 1.00."]
    );
}

#[test]
fn disconnect_releases_every_block() {
    let mut h = Harness::new(config("1,100,100"), Script::default());
    let _ = h.enter();
    let _ = h.step(16);
    h.send(Command::Disconnect).unwrap();
    let ev = h.poll();
    assert!(matches!(ev.last(), Some(Event::Disconnected { .. })));
    // Statics are not despawned one by one: Disconnected ends everything.
    assert_eq!(despawned(&ev).len(), 1, "only the player");
    assert_eq!(h.log.lock().unwrap().released.len(), 9);
    assert_eq!(h.session.player_location(), None);
}

#[test]
fn leaving_the_grid_releases_everything() {
    let mut h = Harness::new(config("1,0,0,1,1,0"), Script::default());
    let mut ev = h.enter();
    ev.extend(h.step(16));
    // Corner block: 4 blocks of the 3x3 ring exist.
    assert_eq!(statics(&ev).len(), 4 * 3);
    let ev = h.move_to(Location::new(R1, -500.0, -500.0, 0.0));
    assert!(despawned(&ev).is_empty());
    assert!(ev.contains(&Event::StaticsCleared));
    assert!(statics(&h.step(16)).is_empty());
    assert_eq!(h.stats().resident, 0);
}

#[test]
fn teleport_and_query_need_the_world() {
    let mut h = Harness::new(config("1,0,0"), Script::default());
    h.send(Command::Connect).unwrap();
    assert!(matches!(
        h.send(Command::QueryLocation),
        Err(NetError::InvalidState { .. })
    ));
    assert!(matches!(
        h.send(Command::Teleport {
            destination: Destination::Named("x".into())
        }),
        Err(NetError::InvalidState { .. })
    ));
    let _ = h.poll();
    h.send(Command::EnterWorld {
        character: "Wanderer".into(),
    })
    .unwrap();
    let _ = h.poll();
    for bad in [
        Destination::Named(String::new()),
        Destination::Named("  ".into()),
        Destination::Named("a\nb".into()),
        Destination::Named("n".repeat(300)),
        Destination::Location {
            location: Location::new(R1, f64::NAN, 0.0, 0.0),
            heading: 0.0,
        },
        Destination::Location {
            location: Location::new(R1, 1.0e8, 0.0, 0.0),
            heading: 0.0,
        },
        Destination::Location {
            location: Location::new(R1, 0.0, 0.0, 0.0),
            heading: f32::INFINITY,
        },
    ] {
        assert!(
            matches!(
                h.send(Command::Teleport {
                    destination: bad.clone()
                }),
                Err(NetError::InvalidCommand(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(Command::QueryLocation.name(), "QueryLocation");
}

#[test]
fn config_validation() {
    let bad = [
        SandboxConfig {
            spawn_point: Some(SpawnPoint {
                destination: Destination::Named(String::new()),
            }),
            ..SandboxConfig::default()
        },
        SandboxConfig {
            spawn_point: Some(SpawnPoint::at(
                Location::new(R1, f64::INFINITY, 0.0, 0.0),
                0.0,
            )),
            ..SandboxConfig::default()
        },
        SandboxConfig {
            resolve_timeout: Duration::from_secs(3600),
            ..SandboxConfig::default()
        },
        // Regression: latency was not validated; Duration::MAX silently
        // became zero latency, hours held every event without bound.
        SandboxConfig {
            latency: Duration::MAX,
            ..SandboxConfig::default()
        },
        SandboxConfig {
            latency: tessera_net::sandbox::MAX_LATENCY + Duration::from_nanos(1),
            ..SandboxConfig::default()
        },
        // An anchor that does not own its location.
        SandboxConfig {
            spawn_point: Some(SpawnPoint {
                destination: Destination::Anchored {
                    location: Location::new(R1, 0.0, 0.0, 0.0),
                    heading: 0.0,
                    anchor: Anchor::new(block(200, 200), 0),
                },
            }),
            ..SandboxConfig::default()
        },
        SandboxConfig {
            statics: StaticConfig {
                despawns_per_poll: 0,
                ..StaticConfig::default()
            },
            ..SandboxConfig::default()
        },
        SandboxConfig {
            statics: StaticConfig {
                radius: 200,
                ..StaticConfig::default()
            },
            ..SandboxConfig::default()
        },
    ];
    for cfg in bad {
        assert!(SandboxBackend::new(cfg.clone()).is_err(), "{cfg:?}");
    }
    assert!(SpawnPoint::landblock(R1, 255, 0, [0.0; 3]).is_err());
    assert!(SpawnPoint::landblock(R1, 0, 0, [5000.0, 0.0, 0.0]).is_err());
    assert!(SpawnPoint::named("ok").is_ok());
    let p = SpawnPoint::at(Location::new(R1, 1.0, 2.0, 3.0), 0.0).with_heading(1.0);
    assert!(matches!(p.destination, Destination::Location { heading, .. } if heading == 1.0));
}

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

#[test]
fn session_filters_invalid_world_events() {
    let player = EntityId(1);
    let good = Placement {
        location: Location::new(R1, 1.0, 2.0, 3.0),
        rotation: Rotation::IDENTITY,
        scale: 1.0,
    };
    let stat = |placement| Event::StaticSpawned {
        id: EntityId(9),
        object: DataId(5),
        placement,
        tag: 0,
    };
    let report = |location, heading| Event::LocationReport {
        id: player,
        location,
        heading,
        anchor: None,
        cause: LocationCause::Requested,
    };
    let script = vec![
        vec![Event::Connected { characters: vec![] }],
        vec![
            // Entity events before the world are dropped.
            stat(good),
            Event::WorldEntered {
                player,
                name: "P".into(),
                transform: Transform::default(),
            },
            stat(good),
            stat(Placement { scale: 0.0, ..good }),
            stat(Placement {
                rotation: Rotation::new(f32::NAN, 0.0, 0.0, 0.0),
                ..good
            }),
            stat(Placement {
                location: Location::new(R1, f64::NAN, 0.0, 0.0),
                ..good
            }),
            // Regression: rotations had only to be finite and scales
            // positive, so these overflowed or collapsed in the renderer.
            stat(Placement {
                rotation: Rotation::new(0.0, 1.0e20, 0.0, 0.0),
                ..good
            }),
            stat(Placement {
                rotation: Rotation::new(0.0, 0.0, 0.0, 0.0),
                ..good
            }),
            stat(Placement {
                scale: 3.0e38,
                ..good
            }),
            // Slightly off unit (stored data is unit within 0.001) passes.
            stat(Placement {
                rotation: Rotation::new(1.0005, 0.0, 0.0, 0.0),
                ..good
            }),
            // An anchor that does not own the reported location.
            Event::LocationReport {
                id: player,
                location: Location::new(R1, 0.0, 0.0, 0.0),
                heading: 0.0,
                anchor: Some(Anchor::new(block(200, 200), 3)),
                cause: LocationCause::Requested,
            },
            report(Location::new(R1, 1.0e9, 0.0, 0.0), 0.0),
            report(Location::new(R1, 0.0, 0.0, 0.0), f32::NAN),
            report(Location::new(R2, 10.0, 20.0, 30.0), 0.5),
            // A report for another entity does not move the player.
            Event::LocationReport {
                id: EntityId(7),
                location: Location::new(R1, 0.0, 0.0, 0.0),
                heading: 0.0,
                anchor: None,
                cause: LocationCause::Requested,
            },
            // A correction keeps the region.
            Event::EntityMoved {
                id: player,
                transform: Transform::new(Vec3::new(11.0, 21.0, 31.0), 0.25),
                velocity: Vec3::ZERO,
            },
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
    assert_eq!(s.player_location(), None);
    let ev = s.poll(t);
    assert_eq!(statics(&ev).len(), 2);
    assert_eq!(reports(&ev).len(), 2);
    assert_eq!(
        s.player_location(),
        Some((Location::new(R2, 11.0, 21.0, 31.0), 0.25))
    );
}

#[test]
fn model_helpers() {
    let q = Rotation::about_z(std::f32::consts::FRAC_PI_2);
    let v = q.rotate(Vec3::new(0.0, 1.0, 0.0));
    assert!((v.x + 1.0).abs() < 1e-6 && v.y.abs() < 1e-6 && v.z.abs() < 1e-6);
    assert_eq!(Rotation::default(), Rotation::IDENTITY);
    assert!(Rotation::new(0.0, 0.0, 0.0, 0.0).normalized().is_none());
    assert!(
        Rotation::new(f32::MAX, f32::MAX, 0.0, 0.0)
            .normalized()
            .is_none()
    );
    let a = Location::new(R1, 0.0, 0.0, 5.0);
    let b = Location::new(R1, 3.0, 4.0, 100.0);
    assert_eq!(a.distance_xy(&b), 5.0);
    assert!(!Location::new(R1, 0.0, 0.0, f64::NAN).is_valid());
    // Regression: the derived default had scale 0, which Session dropped.
    assert!(Placement::default().is_valid());
    assert_eq!(Placement::default().scale, 1.0);
    assert_eq!(Placement::default().rotation, Rotation::IDENTITY);
    let p = Placement::new(Location::new(R1, 1.0, 2.0, 3.0), Rotation::IDENTITY, 2.0);
    assert!(p.is_valid());
    assert!(!Placement { scale: 1.0e5, ..p }.is_valid());
    assert_eq!(
        Location::from_position(R1, Vec3::new(1.5, 2.5, 3.5)).position(),
        Vec3::new(1.5, 2.5, 3.5)
    );
}

/// Blocks the fake source was asked for, as (x, y).
fn queried(log: &Arc<Mutex<Log>>) -> BTreeSet<(u8, u8)> {
    log.lock()
        .unwrap()
        .queries
        .iter()
        .map(|b| (b.x, b.y))
        .collect()
}

/// Regression (finding: interior positions re-homed): an out-of-square
/// spawn streamed around the block its global coordinate fell in, so the
/// owning block was never served, and was snapped to that block's ground.
#[test]
fn interior_spawn_streams_around_its_owner() {
    let mut h = Harness::new(config("1,100,150,-500,80,10"), Script::default());
    let ev = h.enter();
    let owner = block(100, 150);
    let here = owner.location([-500.0, 80.0, 10.0]);
    // Exact height: not snapped to the (wrong block's) ground at 123.5.
    assert_eq!(reports(&ev), [(here, LocationCause::Entered)]);
    assert_eq!(h.session.player_anchor(), Some(Anchor::new(owner, 0)));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::LocationReport { anchor: Some(a), .. } if a.block == owner
    )));
    let _ = h.run(3);
    let asked = queried(&h.log);
    assert!(asked.contains(&(100, 150)), "{asked:?}");
    assert!(
        asked
            .iter()
            .all(|(x, y)| x.abs_diff(100) <= 1 && y.abs_diff(150) <= 1),
        "{asked:?}"
    );

    // Off the grid entirely (block 0, local -300): still served.
    let mut h = Harness::new(config("1,0,10,-300,80,10"), Script::default());
    let mut ev = h.enter();
    ev.extend(h.run(3));
    assert_eq!(statics(&ev).len(), 6 * 3, "blocks (0..=1, 9..=11)");
    assert_eq!(
        reports(&ev),
        [(
            Location::new(R1, -300.0, 1680.0, 10.0),
            LocationCause::Entered
        )]
    );

    // Moving inside the interior keeps the anchor; leaving its reach drops
    // it, and streaming then follows the coordinates.
    let _ = h.move_to(Location::new(R1, -250.0, 1680.0, 10.0));
    assert!(h.session.player_anchor().is_some());
    let _ = h.run(2);
    let edge = Location::new(R1, MAX_LOCAL_OFFSET, 1680.0, 10.0);
    let _ = h.move_to(edge);
    assert!(h.session.player_anchor().is_some(), "still within reach");
    let _ = h.run(2);
    assert!(
        !queried(&h.log).contains(&(8, 10)),
        "streams around (0, 10)"
    );
    let far = Location::new(R1, 160.0 * 10.0 + 80.0, 1680.0, 10.0);
    let _ = h.move_to(far);
    assert_eq!(h.session.player_anchor(), None);
    let _ = h.run(3);
    assert!(queried(&h.log).contains(&(10, 10)));
}

/// A `/loc` line with `cInside` resolves to an anchored, exact place with a
/// facing; the anchor reaches `describe`.
#[test]
fn named_interior_place_keeps_anchor_and_heading() {
    let owner = block(100, 150);
    let place = ResolvedPlace::exact(owner.location([900.0, -40.0, 55.0]))
        .with_heading(1.0)
        .with_anchor(Anchor::new(owner, 0x0123));
    let ground = owner.location([80.0, 80.0, 0.0]);
    let script = Script {
        resolved: [
            ("Inside".to_owned(), place),
            (
                "Bad anchor".to_owned(),
                ResolvedPlace::exact(ground).with_anchor(Anchor::new(block(1, 1), 0)),
            ),
            (
                "Faced".to_owned(),
                ResolvedPlace::on_ground(ground).with_heading(f32::NAN),
            ),
        ]
        .into(),
        ..Script::default()
    };
    let mut cfg = config("name:Inside");
    cfg.spawn.heading = 0.25;
    let mut h = Harness::new(cfg, script);
    let ev = h.enter();
    assert_eq!(reports(&ev), [(place.location, LocationCause::Entered)]);
    assert_eq!(h.session.player_location(), Some((place.location, 1.0)));
    assert_eq!(h.session.player_anchor(), place.anchor);
    let _ = h.run(2);
    assert!(queried(&h.log).contains(&(100, 150)));
    h.send(Command::QueryLocation).unwrap();
    let ev = h.poll();
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::LocationReport { anchor: Some(a), cause: LocationCause::Requested, .. }
            if a.cell == 0x0123
    )));
    assert_eq!(h.log.lock().unwrap().described, [place.anchor]);

    // A source whose anchor does not own the location is refused.
    h.send(Command::Teleport {
        destination: Destination::Named("Bad anchor".into()),
    })
    .unwrap();
    assert!(
        system_texts(&h.poll())
            .iter()
            .any(|t| t.contains("invalid location"))
    );
    // A non-finite heading from the source keeps the current one; a ground
    // place is snapped and clears the anchor.
    h.send(Command::Teleport {
        destination: Destination::Named("Faced".into()),
    })
    .unwrap();
    let ev = h.poll();
    assert_eq!(
        reports(&ev),
        [(Location { z: 123.5, ..ground }, LocationCause::Teleported)]
    );
    assert_eq!(h.session.player_location().unwrap().1, 1.0);
    assert_eq!(h.session.player_anchor(), None);
}

/// Regression: a teleport to a parsed `R,X,Y` destination could never snap
/// to the ground (the snap flag was lost), so it landed at z = 0.
#[test]
fn teleport_to_ground_destination_snaps() {
    let mut h = Harness::new(config("1,100,100"), Script::default());
    let ev = h.enter();
    assert_eq!(reports(&ev)[0].0.z, 123.5);
    let to = SpawnPoint::parse("1,120,100").unwrap();
    assert!(to.snaps_to_ground());
    h.send(Command::Teleport {
        destination: to.destination,
    })
    .unwrap();
    assert_eq!(
        reports(&h.poll()),
        [(
            Location::new(R1, 19_280.0, 16_080.0, 123.5),
            LocationCause::Teleported
        )]
    );
    // Interior destinations are exact.
    h.send(Command::Teleport {
        destination: SpawnPoint::parse("1,120,100,-40,10,7").unwrap().destination,
    })
    .unwrap();
    assert_eq!(reports(&h.poll())[0].0.z, 7.0);
}

/// Regression: with the resident budget full, newly entered blocks were
/// asked with limit 0 and stored empty forever, while blocks behind the
/// player kept the budget; `truncated` stayed 0.
#[test]
fn resident_cap_serves_the_players_block() {
    // The exact probe of the finding: radius 0, one block's worth of budget.
    let mut cfg = config("1,10,10");
    cfg.statics.radius = 0;
    cfg.statics.max_per_block = 10;
    cfg.statics.max_resident = 10;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 10,
            ..Script::default()
        },
    );
    let mut live = Live::default();
    live.apply(&h.enter());
    assert_eq!(live.in_block(10, 10), 10);
    for (x, back) in [(11, false), (12, false), (11, true)] {
        live.apply(&h.move_to(block(x, 10).location([80.0, 80.0, 0.0])));
        live.apply(&h.run(5));
        assert_eq!(live.in_block(x, 10), 10, "block ({x}, 10), back = {back}");
        assert!(live.0.len() <= 10);
    }
    assert!(h.log.lock().unwrap().limits.iter().all(|l| *l > 1));
    assert!(h.stats().evicted >= 2);
    assert_eq!(h.stats().starved_blocks, 0);

    // Radius 1 with room for five of nine blocks: after a step east the
    // player's block and the blocks ahead fill as those behind go.
    let mut cfg = config("1,100,100");
    cfg.statics.max_per_block = 10;
    cfg.statics.max_resident = 50;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 10,
            ..Script::default()
        },
    );
    let mut live = Live::default();
    live.apply(&h.enter());
    live.apply(&h.run(3));
    assert_eq!(live.0.len(), 50);
    assert_eq!(live.in_block(100, 100), 10, "nearest first");
    assert_eq!(h.stats().starved_blocks, 4);
    live.apply(&h.move_to(block(101, 100).location([80.0, 80.0, 0.0])));
    live.apply(&h.run(10));
    assert!(live.0.len() <= 50);
    assert_eq!(live.in_block(101, 100), 10, "the player's own block");
    for x in 100..=102 {
        for y in 99..=101 {
            assert!(live.in_block(x, y) > 0 || h.stats().starved_blocks > 0);
        }
    }
    // Only the ring column x = 99 was given up.
    assert!(h.log.lock().unwrap().released.iter().all(|b| b.x == 99));
}

/// A block served short for lack of room is asked again for the rest once
/// room frees, continuing where it left off; cuts by `max_per_block` are
/// counted.
#[test]
fn part_served_block_is_completed_later() {
    let mut cfg = config("1,100,100");
    cfg.statics.radius = 0;
    cfg.statics.max_per_block = 8;
    cfg.statics.max_resident = 8;
    cfg.statics.spawns_per_poll = 100;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 20,
            ..Script::default()
        },
    );
    let mut live = Live::default();
    live.apply(&h.enter());
    assert_eq!(live.in_block(100, 100), 8);
    // 20 objects, 8 taken: at least one known to be cut.
    assert!(h.stats().truncated >= 1);

    // Room is short: the next block gets only part, then the rest when the
    // old block is evicted.
    live.apply(&h.move_to(block(101, 100).location([80.0, 80.0, 0.0])));
    live.apply(&h.run(6));
    assert_eq!(live.in_block(101, 100), 8);
    let tags: BTreeSet<u64> = live
        .0
        .values()
        .filter(|v| (v.1, v.2) == (101, 100))
        .map(|v| v.3 & 0xFF_FFFF)
        .collect();
    assert_eq!(tags, (0..8).collect(), "first 8 objects, each once");
}

/// Regression: releasing blocks emitted every despawn in one poll.
#[test]
fn despawns_are_budgeted_and_count_against_residency() {
    let mut cfg = config("1,100,100");
    cfg.statics.despawns_per_poll = 4;
    let mut h = Harness::new(
        cfg,
        Script {
            per_block: 5,
            ..Script::default()
        },
    );
    let mut live = Live::default();
    live.apply(&h.enter());
    live.apply(&h.run(2));
    assert_eq!(live.0.len(), 45);
    // Three blocks east: the old 3x3 is all released at once -> one clear.
    let ev = h.move_to(block(104, 100).location([80.0, 80.0, 0.0]));
    assert!(ev.contains(&Event::StaticsCleared));
    live.apply(&ev);
    live.apply(&h.run(3));
    assert_eq!(live.0.len(), 45);
    // Three blocks east again: the west two columns (6 blocks, 30 objects)
    // go, 4 per poll, and stay counted until announced; column 105 stays in
    // the release ring.
    let ev = h.move_to(block(107, 100).location([80.0, 80.0, 0.0]));
    assert_eq!(despawned(&ev).len(), 4);
    assert!(!ev.contains(&Event::StaticsCleared));
    assert_eq!(h.stats().despawn_queued, 26);
    assert!(h.stats().resident >= 26);
    live.apply(&ev);
    let mut polls = 1;
    loop {
        let ev = h.step(16);
        assert!(despawned(&ev).len() <= 4);
        live.apply(&ev);
        polls += 1;
        if h.stats().despawn_queued == 0 {
            break;
        }
        assert!(polls < 20);
    }
    assert_eq!(polls, 8, "30 despawns at 4 per poll");
    live.apply(&h.run(3));
    assert_eq!(
        live.0.len(),
        60,
        "3x3 around the player plus the ring column"
    );
    assert_eq!(h.stats().resident, 60);
}

/// Regression: a move the client sent before it saw a teleport's events was
/// applied after the teleport and silently undid it.
#[test]
fn stale_move_does_not_undo_a_teleport() {
    let mut h = Harness::new(config("1,100,100,80,80,10"), Script::default());
    let _ = h.enter();
    let old = Location::new(R1, 16_080.0, 16_080.0, 10.0);
    let new = Location::new(R1, 16_480.0, 16_080.0, 10.0);
    h.send(Command::Teleport {
        destination: Destination::Location {
            location: new,
            heading: 0.0,
        },
    })
    .unwrap();
    // Same frame, before the poll that delivers the teleport.
    h.send(Command::Move {
        location: old,
        heading: 0.0,
        velocity: Vec3::ZERO,
    })
    .unwrap();
    let ev = h.poll();
    assert_eq!(reports(&ev), [(new, LocationCause::Teleported)]);
    h.send(Command::QueryLocation).unwrap();
    assert_eq!(reports(&h.poll()), [(new, LocationCause::Requested)]);
    // Moves after the teleport was seen count again.
    let _ = h.move_to(Location { x: 16_481.0, ..new });
    h.send(Command::QueryLocation).unwrap();
    assert_eq!(reports(&h.poll())[0].0.x, 16_481.0);
}

/// Regression: a second teleport while the first was still resolving
/// replaced it silently; the first request never got an answer.
#[test]
fn superseded_teleport_is_answered() {
    let script = Script {
        places: [("Fast".to_owned(), Location::new(R1, 100.0, 100.0, 0.0))].into(),
        ..Script::default()
    };
    let mut h = Harness::new(config("1,0,0"), script);
    let _ = h.enter();
    h.send(Command::Teleport {
        destination: Destination::Named("Slow Town".into()),
    })
    .unwrap();
    assert!(reports(&h.poll()).is_empty());
    h.send(Command::Teleport {
        destination: Destination::Named("Fast".into()),
    })
    .unwrap();
    let ev = h.poll();
    assert!(
        system_texts(&ev)
            .iter()
            .any(|t| t.contains("Teleport cancelled"))
    );
    assert_eq!(reports(&ev).len(), 1);
    let ev = h.step(20_000);
    assert!(reports(&ev).is_empty() && system_texts(&ev).is_empty());
}

/// Regression: moves travelled as f32 region-global positions, so the
/// precise player location lost millimetres far from the origin.
#[test]
fn moves_keep_full_precision() {
    let mut h = Harness::new(config("1,250,250,0,0,5"), Script::default());
    let _ = h.enter();
    let mut here = h.session.player_location().unwrap().0;
    assert_eq!(here.x, 40_000.0);
    // A 0.2 m/s creep at 60 fps: 3.3 mm per frame, below the f32 ulp here.
    for _ in 0..60 {
        here.x += 0.2 / 60.0;
        h.now += Duration::from_millis(16);
        h.send(Command::Move {
            location: here,
            heading: 0.0,
            velocity: Vec3::new(0.2, 0.0, 0.0),
        })
        .unwrap();
        let _ = h.poll();
    }
    let (tracked, _) = h.session.player_location().unwrap();
    assert!((tracked.x - 40_000.2).abs() < 1e-9, "{}", tracked.x);
    h.send(Command::QueryLocation).unwrap();
    assert!((reports(&h.poll())[0].0.x - 40_000.2).abs() < 1e-9);
    // An implausible move is corrected; the correction repeats the current
    // position and keeps its precision.
    h.send(Command::Move {
        location: Location { x: 1.0e6, ..here },
        heading: 0.0,
        velocity: Vec3::ZERO,
    })
    .unwrap();
    let ev = h.poll();
    let player = h.session.player().unwrap();
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::EntityMoved { id, .. } if *id == player))
    );
    assert_eq!(h.session.player_location().unwrap().0, here);
}

#[test]
fn compass_and_facing_conventions() {
    use std::f32::consts::{FRAC_PI_2, PI};
    use tessera_net::model::{compass_degrees_from_heading, heading_from_compass_degrees};
    // Heading 0 faces east = compass 90; north = compass 0 = heading π/2.
    assert!((compass_degrees_from_heading(0.0) - 90.0).abs() < 1e-4);
    assert!(compass_degrees_from_heading(FRAC_PI_2).abs() < 1e-4);
    assert!((compass_degrees_from_heading(PI) - 270.0).abs() < 1e-4);
    assert!((heading_from_compass_degrees(0.0) - FRAC_PI_2).abs() < 1e-6);
    assert!((heading_from_compass_degrees(90.0)).abs() < 1e-6);
    assert!((heading_from_compass_degrees(270.0) - PI).abs() < 1e-6);
    for d in [0.0f32, 1.3, 45.0, 180.0, 270.0, 359.9] {
        let back = compass_degrees_from_heading(heading_from_compass_degrees(d));
        assert!((back - d).abs() < 1e-3, "{d} -> {back}");
        assert!((0.0..360.0).contains(&back));
    }
    assert!(heading_from_compass_degrees(f32::NAN).is_nan());
    // A +Y-forward model drawn with `facing(θ)` points along (cos θ, sin θ).
    for theta in [0.0f32, 0.7, FRAC_PI_2, PI, -2.0] {
        let f = Rotation::facing(theta).rotate(Vec3::new(0.0, 1.0, 0.0));
        assert!((f.x - theta.cos()).abs() < 1e-5 && (f.y - theta.sin()).abs() < 1e-5);
    }
    // Spec §11.5: yaw 90° turns model +Y west, compass 270.
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let west = Rotation::new(h, 0.0, 0.0, h).rotate(Vec3::new(0.0, 1.0, 0.0));
    let theta = west.y.atan2(west.x);
    assert!((compass_degrees_from_heading(theta) - 270.0).abs() < 1e-2);
}

proptest! {
    #[test]
    fn parse_never_panics(text in "\\PC{0,80}") {
        let _ = SpawnPoint::parse(&text);
    }

    #[test]
    fn parse_landblock_roundtrip(
        r in 0u16..=u16::MAX, x in 0u8..=254, y in 0u8..=254,
        ox in 0.0f64..160.0, oy in 0.0f64..160.0,
    ) {
        let text = format!("{r},{x},{y},{ox},{oy},1");
        let p = SpawnPoint::parse(&text).unwrap();
        let Destination::Location { location, .. } = p.destination else {
            panic!("not a location");
        };
        let b = Landblock::containing(&location).unwrap();
        prop_assert_eq!((b.region, b.x, b.y), (RegionId(r), x, y));
        prop_assert!((location.x - (f64::from(x) * LANDBLOCK_SIZE + ox)).abs() < 1e-9);
    }

    #[test]
    fn containing_is_in_range(gx in -100.0f64..GRID_EXTENT + 100.0, gy in -100.0f64..GRID_EXTENT + 100.0) {
        let l = Location::new(R1, gx, gy, 0.0);
        match Landblock::containing(&l) {
            Some(b) => {
                let [ox, oy] = b.origin();
                prop_assert!(gx >= ox && gx <= ox + LANDBLOCK_SIZE);
                prop_assert!(gy >= oy && gy <= oy + LANDBLOCK_SIZE);
            }
            None => prop_assert!(!(0.0..=GRID_EXTENT).contains(&gx) || !(0.0..=GRID_EXTENT).contains(&gy)),
        }
    }
}
