// SPDX-License-Identifier: GPL-3.0-or-later
//! The minimap and world-map data source of the native panels: `tessera-ui`'s
//! [`MapSource`] answered from the install through `tessera-world`'s [`UiMapReader`]
//! (`game-panels.md` §8.4.2, §8.5.1).
//!
//! The reader does the record I/O and hands back the stored `UI_Map_GameMap` value; this
//! module decodes it with the UI's own decoder ([`GameMap::from_value`]), so the map rules
//! of GP §8.4.3 and §8.6.1 exist once.
//!
//! **Threading** (`docs/ARCHITECTURE.md` §21: no blocking I/O on the main thread). Built
//! with [`WorldMaps::with_pool`], a lookup that is not answered yet runs on the asset worker
//! pool and the source answers [`MapLookup::Pending`] until it finishes, as the
//! [`MapSource`] contract and the GP §8.5.1 implementation OPEN expect; the radar and the
//! world map keep asking and use the answer when it arrives. Answers (failures as "no map")
//! are cached here and in the reader, so a place costs its record reads once per cache
//! life. Built with [`WorldMaps::new`], lookups run synchronously on the caller's thread
//! (tools and tests).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tessera_assets::{AssetError, CancelToken, Priority, TaskHandle, TaskStatus, WorkerPool};
use tessera_ui::game::map::GameMap;
use tessera_ui::game::vocab::GamePids;
use tessera_ui::game::{MapLookup, MapSource};
use tessera_world::LandblockKey;
use tessera_world::ui_maps::{MapValue, UiMapReader};

/// Most decoded answers [`WorldMaps`] keeps (Tessera cap).
pub const ANSWER_CACHE: usize = 512;

/// Most lookups in flight on the worker pool at once (Tessera cap); further lookups answer
/// [`MapLookup::Pending`] and are started on a later call.
pub const MAX_IN_FLIGHT: usize = 32;

/// One lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Req {
    /// A block's landblock-properties map.
    Block(LandblockKey),
    /// An interior cell's dungeon map.
    Interior(LandblockKey, u16),
}

/// What a worker hands back: the stored map value, or why the lookup failed.
type Outcome = Result<Option<MapValue>, String>;

/// A lookup running on the pool.
struct InFlight {
    slot: Arc<Mutex<Option<Outcome>>>,
    task: TaskHandle,
}

/// A [`MapSource`] over the install.
pub struct WorldMaps {
    reader: Arc<Mutex<UiMapReader>>,
    pids: GamePids,
    pool: Option<WorkerPool>,
    answers: HashMap<Req, Option<GameMap>>,
    order: VecDeque<Req>,
    in_flight: HashMap<Req, InFlight>,
    errors: u64,
}

impl std::fmt::Debug for WorldMaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldMaps")
            .field("async", &self.pool.is_some())
            .field("answers", &self.answers.len())
            .field("in_flight", &self.in_flight.len())
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

impl WorldMaps {
    /// A synchronous source over `reader`: every lookup reads on the caller's thread (for
    /// tools and tests; the client uses [`WorldMaps::with_pool`]). `pids` are the game
    /// vocabulary's PIDs the map structs are decoded with.
    #[must_use]
    pub fn new(reader: UiMapReader, pids: GamePids) -> Self {
        Self {
            reader: Arc::new(Mutex::new(reader)),
            pids,
            pool: None,
            answers: HashMap::new(),
            order: VecDeque::new(),
            in_flight: HashMap::new(),
            errors: 0,
        }
    }

    /// A source whose lookups run on `pool` (`docs/ARCHITECTURE.md` §21): a lookup not
    /// answered yet answers [`MapLookup::Pending`] until its task finishes.
    #[must_use]
    pub fn with_pool(reader: UiMapReader, pids: GamePids, pool: WorkerPool) -> Self {
        Self {
            pool: Some(pool),
            ..Self::new(reader, pids)
        }
    }

    /// Runs `f` on the reader (blocking while a worker uses it), for direct lookups such as
    /// the world map's own queries or tests. `None` if a worker panicked while holding it.
    pub fn with_reader<R>(&self, f: impl FnOnce(&mut UiMapReader) -> R) -> Option<R> {
        self.reader.lock().ok().map(|mut r| f(&mut r))
    }

    /// Lookups that failed (unreadable or undecodable records, or a failed task); each was
    /// answered "no map" and is not retried while it stays cached.
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// Lookups currently running on the pool.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    fn remember(&mut self, req: Req, m: Option<GameMap>) {
        if self.answers.insert(req, m).is_none() {
            self.order.push_back(req);
        }
        while self.answers.len() > ANSWER_CACHE {
            match self.order.pop_front() {
                Some(old) => {
                    self.answers.remove(&old);
                }
                None => break,
            }
        }
    }

    fn decode(&self, v: Option<&MapValue>) -> Option<GameMap> {
        v.and_then(|v| GameMap::from_value(v, &self.pids))
    }

    /// Turns a finished lookup into the answer, counting a failure.
    fn settle(&mut self, req: Req, outcome: Outcome) -> MapLookup {
        let m = match outcome {
            Ok(v) => self.decode(v.as_ref()),
            Err(e) => {
                self.errors += 1;
                tracing::debug!("map lookup {req:?}: {e}");
                None
            }
        };
        self.remember(req, m.clone());
        MapLookup::Ready(m)
    }

    fn run(reader: &Mutex<UiMapReader>, req: Req) -> Outcome {
        let mut r = reader
            .lock()
            .map_err(|_| "map reader poisoned".to_string())?;
        let res = match req {
            Req::Block(key) => r.landblock_map(key),
            Req::Interior(key, cell) => r.interior_map(key, cell),
        };
        res.map_err(|e| e.to_string())
    }

    fn lookup(&mut self, req: Req) -> MapLookup {
        if let Some(m) = self.answers.get(&req) {
            return MapLookup::Ready(m.clone());
        }
        let Some(pool) = self.pool.clone() else {
            let outcome = Self::run(&self.reader, req);
            return self.settle(req, outcome);
        };
        if let Some(f) = self.in_flight.get(&req) {
            let taken = f.slot.lock().ok().and_then(|mut s| s.take());
            let status = f.task.status();
            return match (taken, status) {
                (Some(outcome), _) => {
                    self.in_flight.remove(&req);
                    self.settle(req, outcome)
                }
                (None, TaskStatus::Queued | TaskStatus::Running) => MapLookup::Pending,
                // Finished without an outcome (the closure panicked, or the pool shut down).
                (None, s) => {
                    self.in_flight.remove(&req);
                    self.settle(req, Err(format!("map task ended as {s:?}")))
                }
            };
        }
        if self.in_flight.len() >= MAX_IN_FLIGHT {
            return MapLookup::Pending;
        }
        let slot: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
        let (reader, out) = (Arc::clone(&self.reader), Arc::clone(&slot));
        let spawned = pool.spawn(Priority::Visible, CancelToken::new(), move |_| {
            let outcome = Self::run(&reader, req);
            if let Ok(mut s) = out.lock() {
                *s = Some(outcome);
            }
        });
        match spawned {
            Ok(task) => {
                self.in_flight.insert(req, InFlight { slot, task });
                MapLookup::Pending
            }
            // Retried on a later call.
            Err(AssetError::QueueFull { .. }) => MapLookup::Pending,
            Err(e) => self.settle(req, Err(format!("map task not started: {e}"))),
        }
    }
}

impl MapSource for WorldMaps {
    fn landblock_map(&mut self, region: u8, x: u8, y: u8) -> MapLookup {
        let Ok(key) = LandblockKey::new(region, x, y) else {
            return MapLookup::Ready(None);
        };
        self.lookup(Req::Block(key))
    }

    fn dungeon_map(&mut self, region: u8, x: u8, y: u8, cell: u16) -> MapLookup {
        if cell == 0 || cell == 0xFFFF {
            return MapLookup::Ready(None);
        }
        let Ok(key) = LandblockKey::new(region, x, y) else {
            return MapLookup::Ready(None);
        };
        self.lookup(Req::Interior(key, cell))
    }
}

/// A [`MapSource`] with no records (no install, or tests): every lookup answers "no map".
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMaps;

impl MapSource for NoMaps {
    fn landblock_map(&mut self, _region: u8, _x: u8, _y: u8) -> MapLookup {
        MapLookup::Ready(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tessera_assets::PoolConfig;
    use tessera_formats::property::PropertyType;
    use tessera_world::MemoryWorldSource;
    use tessera_world::testkit::{self, TestValue};
    use tessera_world::ui_maps::UiMapPids;

    /// Synthetic PIDs: the reader's map PID and the UI's struct fields are the game
    /// vocabulary's placeholders.
    fn setup(src: MemoryWorldSource) -> (UiMapReader, GamePids) {
        let gp = GamePids::default();
        let mut schema = testkit::schema();
        schema.insert(gp.map_game_map, PropertyType::from_code(11));
        schema.insert(gp.map_image, PropertyType::from_code(20));
        schema.insert(gp.map_block_x, PropertyType::from_code(15));
        let reader = UiMapReader::new(
            Arc::new(src),
            Arc::new(schema),
            UiMapPids {
                game_map: Some(gp.map_game_map),
                dungeon_did: None,
                dungeon_map_data: None,
            },
            8,
        );
        (reader, gp)
    }

    fn source_with_map(key: LandblockKey) -> MemoryWorldSource {
        let gp = GamePids::default();
        let mut src = MemoryWorldSource::new();
        src.insert(
            key.properties_did(),
            testkit::landblock_properties(
                key,
                &[(
                    gp.map_game_map,
                    TestValue::Struct(vec![
                        (gp.map_image, TestValue::Did(0x4100_0001)),
                        (gp.map_block_x, TestValue::Int(4)),
                    ]),
                )],
            ),
            Some(39),
        );
        src
    }

    #[test]
    fn synchronous_lookups_decode_with_the_ui_decoder() {
        let key = LandblockKey::new(1, 3, 3).unwrap();
        let (reader, gp) = setup(source_with_map(key));
        let mut m = WorldMaps::new(reader, gp);
        let MapLookup::Ready(Some(g)) = m.landblock_map(1, 3, 3) else {
            panic!("a map");
        };
        assert_eq!(g.image, 0x4100_0001);
        assert_eq!(g.block_x, Some(4));
        assert_eq!(m.landblock_map(0, 1, 1), MapLookup::Ready(None));
        assert_eq!(m.landblock_map(1, 255, 1), MapLookup::Ready(None));
        assert_eq!(m.landblock_map(1, 4, 4), MapLookup::Ready(None));
        assert_eq!(m.dungeon_map(1, 3, 3, 0x100), MapLookup::Ready(None));
        assert_eq!(m.dungeon_map(1, 3, 3, 0), MapLookup::Ready(None));
        assert_eq!(m.errors(), 0);
        assert_eq!(m.with_reader(|r| r.reads()), Some(2));
        assert_eq!(NoMaps.landblock_map(1, 1, 1), MapLookup::Ready(None));
    }

    /// Regression (review): failed lookups were never cached and re-read every update.
    #[test]
    fn failures_are_answered_from_the_cache() {
        let key = LandblockKey::new(1, 3, 4).unwrap();
        let mut src = MemoryWorldSource::new();
        src.insert(key.properties_did(), vec![0xFF; 64], None);
        src.insert(key.info_did(), vec![0xFF; 64], Some(39));
        let gp = GamePids::default();
        let mut schema = testkit::schema();
        schema.insert(gp.map_game_map, PropertyType::from_code(11));
        let reader = UiMapReader::new(
            Arc::new(src),
            Arc::new(schema),
            UiMapPids {
                game_map: Some(gp.map_game_map),
                dungeon_did: Some(0x1000_0204),
                dungeon_map_data: None,
            },
            8,
        );
        let mut m = WorldMaps::new(reader, gp);
        for _ in 0..100 {
            assert_eq!(m.landblock_map(1, 3, 4), MapLookup::Ready(None));
            assert_eq!(m.dungeon_map(1, 3, 4, 0x100), MapLookup::Ready(None));
        }
        assert_eq!(m.errors(), 2);
        assert_eq!(m.with_reader(|r| r.reads()), Some(2));
    }

    #[test]
    fn pool_lookups_answer_pending_until_done() {
        let key = LandblockKey::new(1, 3, 3).unwrap();
        let (reader, gp) = setup(source_with_map(key));
        let pool = WorkerPool::new(PoolConfig {
            workers: 1,
            ..PoolConfig::default()
        });
        let mut m = WorldMaps::with_pool(reader, gp, pool);
        assert_eq!(m.landblock_map(1, 3, 3), MapLookup::Pending);
        assert_eq!(m.in_flight(), 1);
        let started = Instant::now();
        let answer = loop {
            match m.landblock_map(1, 3, 3) {
                MapLookup::Pending => {
                    assert!(started.elapsed() < Duration::from_secs(10), "task finishes");
                    std::thread::yield_now();
                }
                MapLookup::Ready(a) => break a,
            }
        };
        assert_eq!(answer.map(|g| g.image), Some(0x4100_0001));
        assert_eq!(m.in_flight(), 0);
        // Cached: ready at once, no new task.
        assert!(matches!(
            m.landblock_map(1, 3, 3),
            MapLookup::Ready(Some(_))
        ));
        assert_eq!(m.in_flight(), 0);
        assert_eq!(m.with_reader(|r| r.reads()), Some(1));
        // Many distinct lookups: bounded in flight.
        for x in 10..80u8 {
            let _ = m.landblock_map(1, x, 0);
        }
        assert!(m.in_flight() <= MAX_IN_FLIGHT);
        assert_eq!(m.errors(), 0);
    }
}
