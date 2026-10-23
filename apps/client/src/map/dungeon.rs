// SPDX-License-Identifier: GPL-3.0-or-later
//! The player's dungeon in the client half (`map-windows.md` §4.6, revision 1, MW34).
//!
//! The client half takes the player's dungeon from the first of these sources that gives
//! one:
//!
//! 1. a `Dungeon_CurrentDungeon` the session reports for the player: a player property
//!    (`PropertyChanged`, which a scenario's `stat player @Dungeon_CurrentDungeon=…` step
//!    makes, `scenario-feed.md` §5.8) holding a non-null DID ([`reported_dungeon`]);
//! 2. when the player is in an interior cell, that cell's `Dungeon_DID`, read from the cell's
//!    property set in the landblock-info record of the player's block (the minimap's indoor
//!    lookup, `game-panels.md` §8.5.1);
//! 3. none: outdoors, or the cell names no dungeon.
//!
//! **Threading** (`docs/ARCHITECTURE.md` §21): the record is read by a job on a worker pool
//! (the pattern of the minimap's lookups, `panels::maps::WorldMaps`); until its
//! answer arrives the player's dungeon stays as it was, so the shown map stays as it is
//! (§4.6 revision 1). Each job reads through the reader of the facts in use when it is
//! asked. At most one lookup is in flight; answers are kept in a bounded cache
//! ([`ANSWER_CACHE`]), failures as "no dungeon".

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tessera_assets::{
    AssetError, CancelToken, PoolConfig, Priority, TaskHandle, TaskStatus, WorkerPool,
};
use tessera_formats::Did;
use tessera_net::PropertyValue;
use tessera_ui::game::radar::RadarInput;
use tessera_world::LandblockKey;
use tessera_world::ui_maps::UiMapReader;

use crate::character::PlayerState;

/// Most (block, cell) answers kept (Tessera cap).
pub const ANSWER_CACHE: usize = 1024;

/// Jobs the pool of [`PlayerDungeon`] may hold queued (Tessera cap; one is in flight).
const MAX_QUEUED: usize = 4;

/// One lookup: the player's block and interior cell.
type Query = (LandblockKey, u16);

/// What a job hands back: the cell's dungeon, or why the lookup failed.
type Outcome = Result<Option<Did>, String>;

/// The lookup running on the pool.
struct InFlight {
    query: Query,
    slot: Arc<Mutex<Option<Outcome>>>,
    task: TaskHandle,
}

/// The `Dungeon_CurrentDungeon` the session reports for the player (source 1 of the module
/// docs): the player property `pid` (the install's `Dungeon_CurrentDungeon`, `None` without
/// a dictionary) when it holds a DID other than 0.
#[must_use]
pub fn reported_dungeon(state: &PlayerState, pid: Option<u32>) -> Option<Did> {
    match state.property(pid?)? {
        PropertyValue::Did(d) if d.0 != 0 => Some(Did(d.0)),
        _ => None,
    }
}

/// The player's dungeon, followed frame by frame from the minimap input (module docs).
#[derive(Default)]
pub struct PlayerDungeon {
    pool: Option<WorkerPool>,
    /// The lookup in flight.
    in_flight: Option<InFlight>,
    answers: HashMap<Query, Option<Did>>,
    order: VecDeque<Query>,
    /// Lookups that failed (answered "no dungeon").
    errors: u64,
    /// The player's dungeon as last known.
    current: Option<Did>,
}

impl std::fmt::Debug for PlayerDungeon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerDungeon")
            .field("current", &self.current)
            .field("asked", &self.in_flight.as_ref().map(|f| f.query))
            .field("answers", &self.answers.len())
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

impl PlayerDungeon {
    /// Lookups on `pool` (a pool of the client's own; without one, [`PlayerDungeon::update`]
    /// starts a one-worker pool at its first lookup).
    #[must_use]
    pub fn with_pool(pool: WorkerPool) -> Self {
        Self {
            pool: Some(pool),
            ..Self::default()
        }
    }

    /// The player's dungeon as last known (`None`: outdoors, none, or not known yet).
    #[must_use]
    pub fn current(&self) -> Option<Did> {
        self.current
    }

    /// Whether a lookup is in flight.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Lookups that failed so far (each answered "no dungeon").
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// Follows the player: `input` is the frame's minimap input, `reported` the session's
    /// `Dungeon_CurrentDungeon` for the player ([`reported_dungeon`]), `reader` the
    /// install's cell reader ([`super::MapFacts::cell_reader`], `None` before the facts are
    /// loaded). Returns the player's dungeon: the reported one when there is one; else
    /// outdoors none; in an interior cell the cell's `Dungeon_DID` once read, the last
    /// known value until then; with an invalid position the last known value (§4.6 step 3:
    /// change nothing).
    pub fn update(
        &mut self,
        input: &RadarInput,
        reported: Option<Did>,
        reader: Option<&Arc<Mutex<UiMapReader>>>,
    ) -> Option<Did> {
        self.drain();
        if let Some(d) = reported {
            self.current = Some(d);
            return self.current;
        }
        let valid = input.has_player
            && input.region != 0
            && input.x != 0xFF
            && input.y != 0xFF
            && input.cell != 0xFFFF;
        if !valid {
            return self.current;
        }
        if input.cell == 0 {
            self.current = None;
            return None;
        }
        let Ok(key) = LandblockKey::new(input.region, input.x, input.y) else {
            return self.current;
        };
        let q = (key, input.cell);
        if let Some(a) = self.answers.get(&q) {
            self.current = *a;
            return self.current;
        }
        if self.in_flight.is_none()
            && let Some(r) = reader
        {
            self.ask(q, r);
        }
        self.current
    }

    /// Records a looked-up answer directly (tests, a synchronous caller).
    pub fn insert_answer(&mut self, key: LandblockKey, cell: u16, dungeon: Option<Did>) {
        self.remember((key, cell), dungeon);
    }

    fn remember(&mut self, q: Query, a: Option<Did>) {
        if self.answers.insert(q, a).is_none() {
            self.order.push_back(q);
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

    fn settle(&mut self, q: Query, outcome: Outcome) {
        let a = match outcome {
            Ok(a) => a,
            Err(e) => {
                self.errors += 1;
                tracing::debug!(key = ?q.0, cell = q.1, "cell dungeon unreadable: {e}");
                None
            }
        };
        self.remember(q, a);
    }

    /// Takes the finished lookup's answer.
    fn drain(&mut self) {
        let Some(f) = &self.in_flight else {
            return;
        };
        let taken = f.slot.lock().ok().and_then(|mut s| s.take());
        let status = f.task.status();
        let q = f.query;
        match (taken, status) {
            (Some(outcome), _) => {
                self.in_flight = None;
                self.settle(q, outcome);
            }
            (None, TaskStatus::Queued | TaskStatus::Running) => {}
            // Finished without an outcome (the job panicked, or the pool shut down).
            (None, s) => {
                self.in_flight = None;
                self.settle(q, Err(format!("lookup ended as {s:?}")));
            }
        }
    }

    fn ask(&mut self, q: Query, reader: &Arc<Mutex<UiMapReader>>) {
        let pool = self
            .pool
            .get_or_insert_with(|| {
                WorkerPool::new(PoolConfig {
                    workers: 1,
                    max_queued: MAX_QUEUED,
                    thread_name: "tessera-map-dungeon".to_owned(),
                })
            })
            .clone();
        let slot: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
        let (reader, out) = (Arc::clone(reader), Arc::clone(&slot));
        let spawned = pool.spawn(Priority::Visible, CancelToken::new(), move |_| {
            let outcome = match reader.lock() {
                Ok(mut r) => r.cell_dungeon(q.0, q.1).map_err(|e| e.to_string()),
                Err(_) => Err("map reader poisoned".to_owned()),
            };
            if let Ok(mut s) = out.lock() {
                *s = Some(outcome);
            }
        });
        match spawned {
            Ok(task) => {
                self.in_flight = Some(InFlight {
                    query: q,
                    slot,
                    task,
                });
            }
            // Asked again on a later frame.
            Err(AssetError::QueueFull { .. }) => {}
            Err(e) => self.settle(q, Err(format!("lookup not started: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(cell: u16) -> RadarInput {
        RadarInput {
            region: 1,
            x: 10,
            y: 20,
            cell,
            has_player: true,
            ..RadarInput::default()
        }
    }

    #[test]
    fn outdoors_has_no_dungeon_and_interiors_wait_for_their_answer() {
        let mut d = PlayerDungeon::default();
        let key = LandblockKey::new(1, 10, 20).expect("key");
        assert_eq!(d.update(&input(0), None, None), None);
        // An interior cell without an answer (and without a reader): unchanged.
        assert_eq!(d.update(&input(0x0105), None, None), None);
        d.insert_answer(key, 0x0105, Some(Did(0x7000_1234)));
        assert_eq!(d.update(&input(0x0105), None, None), Some(Did(0x7000_1234)));
        // Another cell not looked up yet: the last known dungeon stays.
        assert_eq!(d.update(&input(0x0106), None, None), Some(Did(0x7000_1234)));
        // An invalid position changes nothing.
        assert_eq!(d.update(&input(0xFFFF), None, None), Some(Did(0x7000_1234)));
        let none = RadarInput {
            x: 0xFF,
            ..input(0)
        };
        assert_eq!(d.update(&none, None, None), Some(Did(0x7000_1234)));
        // Back outdoors.
        assert_eq!(d.update(&input(0), None, None), None);
        // A cell that names no dungeon.
        d.insert_answer(key, 0x0107, None);
        assert_eq!(d.update(&input(0x0107), None, None), None);
        assert!(!d.is_pending());
    }

    #[test]
    fn a_reported_dungeon_comes_first() {
        // Source 1 before the cell's `Dungeon_DID` (source 2), outdoors too.
        let mut d = PlayerDungeon::default();
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let reported = Some(Did(0x7000_0077));
        assert_eq!(d.update(&input(0), reported, None), reported);
        d.insert_answer(key, 0x0105, Some(Did(0x7000_1234)));
        assert_eq!(d.update(&input(0x0105), reported, None), reported);
        // The report ends: the cell decides again.
        assert_eq!(d.update(&input(0x0105), None, None), Some(Did(0x7000_1234)));
        assert_eq!(d.update(&input(0), None, None), None);
    }

    #[test]
    fn the_answer_cache_is_bounded() {
        let mut d = PlayerDungeon::default();
        let key = LandblockKey::new(1, 10, 20).expect("key");
        for c in 0..(ANSWER_CACHE as u16 + 10) {
            d.insert_answer(key, 0x0100 + c, None);
        }
        assert_eq!(d.answers.len(), ANSWER_CACHE);
        assert_eq!(d.order.len(), ANSWER_CACHE);
    }
}
