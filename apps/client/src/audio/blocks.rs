// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock bookkeeping of the world audio (`sound.md` §11): which resident landblocks are
//! scanned on the workers, which scans are stale, and which static emitters are spawned or
//! despawned as the listener moves. Pure state, so that it is tested without an install;
//! [`super::WorldAudio`] spawns the jobs and runs the graph.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_assets::{CancelToken, Priority};
use tessera_scriptgraph::EntityId;
use tessera_world::LandblockKey;

use super::scripts::{BlockScan, StaticEmitter};

/// Most landblock scans queued at once. The player's own landblock is scanned even when
/// the bound is reached, so that a backlog cannot delay its music.
pub const MAX_SCANS_IN_FLIGHT: usize = 64;
/// Emitters are spawned when the listener is this much inside their reach…
pub const SPAWN_MARGIN: f32 = 30.0;
/// …and despawned when it is this much beyond it (hysteresis).
pub const DESPAWN_MARGIN: f32 = 90.0;
/// Most emitters spawned per check (nearest first), so that a fast flight into a dense
/// area spreads the graph work of their start-up scripts over several frames.
pub const MAX_SPAWNS_PER_CHECK: usize = 6;

/// A landblock scan's reply: the scan, or the block whose job ended without one
/// (cancelled, pool shut down, panicked), so that it is scanned again.
pub type ScanMsg = Result<BlockScan, (LandblockKey, usize)>;

/// One landblock the audio keeps.
#[derive(Debug)]
pub struct BlockAudio {
    /// Identity of the bundle scanned (its address).
    pub bundle: usize,
    /// The scan, once it arrived.
    pub scan: Option<Arc<BlockScan>>,
    /// Cancels the scan job while it is queued or running.
    pub token: CancelToken,
}

/// A scan to queue.
#[derive(Debug)]
pub struct ScanOrder<B> {
    /// The landblock.
    pub key: LandblockKey,
    /// Identity of its bundle.
    pub identity: usize,
    /// What the job scans (the bundle).
    pub bundle: B,
    /// The job's cancellation token (kept by the tracker).
    pub token: CancelToken,
    /// The player's block is [`Priority::Immediate`], the others [`Priority::Prefetch`].
    pub priority: Priority,
}

/// What one [`BlockTracker::plan`] asks for.
#[derive(Debug)]
pub struct SyncPlan<B> {
    /// Scans to queue.
    pub scans: Vec<ScanOrder<B>>,
    /// Landblocks whose emitters must despawn (replaced bundle, or no longer wanted).
    pub despawn: Vec<LandblockKey>,
}

/// The scanned landblocks and the scans in flight.
#[derive(Debug, Default)]
pub struct BlockTracker {
    /// Landblocks kept, scanned or being scanned.
    pub blocks: HashMap<LandblockKey, BlockAudio>,
    /// Scan jobs whose reply has not arrived.
    pub in_flight: usize,
}

impl BlockTracker {
    /// Plans the scans for the resident landblocks `(key, bundle identity, bundle)` around
    /// `here`: blocks within `radius` (Chebyshev) are scanned when new or when their bundle
    /// changed (the old scan is cancelled); blocks within `radius + 1` are kept; the others
    /// are dropped and their scans cancelled.
    pub fn plan<B>(
        &mut self,
        resident: impl IntoIterator<Item = (LandblockKey, usize, B)>,
        here: LandblockKey,
        radius: u32,
    ) -> SyncPlan<B> {
        let mut wanted: HashSet<LandblockKey> = HashSet::new();
        let mut plan = SyncPlan {
            scans: Vec::new(),
            despawn: Vec::new(),
        };
        for (key, identity, bundle) in resident {
            let Some(d) = key.chebyshev(here) else {
                continue;
            };
            if d > radius.saturating_add(1) {
                continue;
            }
            wanted.insert(key);
            let fresh = self.blocks.get(&key).is_none_or(|b| b.bundle != identity);
            if !fresh || d > radius {
                continue;
            }
            if self.in_flight >= MAX_SCANS_IN_FLIGHT && key != here {
                continue;
            }
            if let Some(old) = self.blocks.remove(&key) {
                old.token.cancel();
            }
            plan.despawn.push(key);
            let token = CancelToken::new();
            self.blocks.insert(
                key,
                BlockAudio {
                    bundle: identity,
                    scan: None,
                    token: token.clone(),
                },
            );
            self.in_flight += 1;
            plan.scans.push(ScanOrder {
                key,
                identity,
                bundle,
                token,
                priority: if key == here {
                    Priority::Immediate
                } else {
                    Priority::Prefetch
                },
            });
        }
        let gone: Vec<LandblockKey> = self
            .blocks
            .keys()
            .filter(|k| !wanted.contains(k))
            .copied()
            .collect();
        for k in gone {
            if let Some(b) = self.blocks.remove(&k) {
                b.token.cancel();
            }
            plan.despawn.push(k);
        }
        plan
    }

    /// Takes one scan reply: a scan of the current bundle is kept, a stale one dropped; a
    /// lost scan of the current bundle forgets the block so that the next plan scans it
    /// again.
    pub fn receive(&mut self, msg: ScanMsg) {
        self.in_flight = self.in_flight.saturating_sub(1);
        match msg {
            Ok(scan) => {
                if let Some(b) = self.blocks.get_mut(&scan.key)
                    && b.bundle == scan.bundle
                {
                    b.scan = Some(Arc::new(scan));
                }
            }
            Err((key, identity)) => {
                if self
                    .blocks
                    .get(&key)
                    .is_some_and(|b| b.bundle == identity && b.scan.is_none())
                {
                    self.blocks.remove(&key);
                }
            }
        }
    }
}

/// Distance from `listener` to region-global `p`.
fn distance(p: [f64; 3], listener: [f32; 3]) -> f32 {
    let d = [
        p[0] as f32 - listener[0],
        p[1] as f32 - listener[1],
        p[2] as f32 - listener[2],
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// What one emitter check asks for.
#[derive(Debug, Default)]
pub struct EmitterPlan {
    /// Emitters to spawn, nearest first, at most [`MAX_SPAWNS_PER_CHECK`].
    pub spawn: Vec<(LandblockKey, StaticEmitter)>,
    /// Live emitters to despawn (beyond their reach + [`DESPAWN_MARGIN`]).
    pub despawn: Vec<EntityId>,
}

/// Plans one emitter check: a scanned emitter that is not live spawns when the listener is
/// closer than its reach + [`SPAWN_MARGIN`]; a live one despawns when the listener is
/// farther than its reach + [`DESPAWN_MARGIN`] (or its distance is not a number).
#[must_use]
pub fn plan_emitters<'a>(
    blocks: impl IntoIterator<Item = (&'a LandblockKey, &'a BlockAudio)>,
    live: &HashMap<EntityId, LandblockKey>,
    listener: [f32; 3],
) -> EmitterPlan {
    let mut spawn = Vec::new();
    let mut despawn = Vec::new();
    for (key, b) in blocks {
        let Some(scan) = &b.scan else { continue };
        for e in &scan.emitters {
            let d = distance(e.position, listener);
            let is_live = live.contains_key(&e.id);
            if !is_live && d < e.scripts.reach + SPAWN_MARGIN {
                spawn.push((d, *key, e.clone()));
            } else if is_live && (d.is_nan() || d > e.scripts.reach + DESPAWN_MARGIN) {
                despawn.push(e.id);
            }
        }
    }
    spawn.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.id.0.cmp(&b.2.id.0)));
    spawn.truncate(MAX_SPAWNS_PER_CHECK);
    despawn.sort_by_key(|id| id.0);
    EmitterPlan {
        spawn: spawn.into_iter().map(|(_, k, e)| (k, e)).collect(),
        despawn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::props::{AreaMusic, PlaceValues};
    use crate::audio::scripts::EmitterScripts;
    use tessera_formats::script_table::resolve::PropertyState;

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn scan(k: LandblockKey, identity: usize, emitters: Vec<StaticEmitter>) -> BlockScan {
        BlockScan {
            key: k,
            bundle: identity,
            area: AreaMusic::default(),
            area_did: None,
            place: PlaceValues::default(),
            emitters,
            monitors: Vec::new(),
        }
    }

    fn emitter(id: u64, x: f64, reach: f32) -> StaticEmitter {
        StaticEmitter {
            id: EntityId(id),
            position: [x, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scripts: Arc::new(EmitterScripts {
                tables: Vec::new(),
                startup_scripts: Vec::new(),
                properties: PropertyState::default(),
                reach,
            }),
        }
    }

    /// A 5 × 5 square of resident blocks around (10, 10), identity = 1000 + x * 100 + y.
    fn resident() -> Vec<(LandblockKey, usize, ())> {
        let mut v = Vec::new();
        for x in 8..=12u8 {
            for y in 8..=12u8 {
                v.push((key(x, y), 1000 + usize::from(x) * 100 + usize::from(y), ()));
            }
        }
        v
    }

    #[test]
    fn scans_within_the_radius_and_keeps_one_ring_more() {
        let mut t = BlockTracker::default();
        let here = key(10, 10);
        let p = t.plan(resident(), here, 1);
        assert_eq!(p.scans.len(), 9);
        assert_eq!(t.in_flight, 9);
        assert_eq!(t.blocks.len(), 9);
        let mine: Vec<_> = p.scans.iter().filter(|s| s.key == here).collect();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].priority, Priority::Immediate);
        assert!(
            p.scans
                .iter()
                .filter(|s| s.key != here)
                .all(|s| s.priority == Priority::Prefetch)
        );
        // Nothing new: nothing to scan.
        let p = t.plan(resident(), here, 1);
        assert!(p.scans.is_empty() && p.despawn.is_empty());
        // Another region is never wanted.
        let other = LandblockKey::new(2, 10, 10).unwrap();
        let p = t.plan(vec![(other, 1, ())], here, 1);
        assert!(p.scans.is_empty());
        assert!(
            t.blocks.is_empty(),
            "the region-1 blocks left the resident set"
        );
    }

    #[test]
    fn scans_of_blocks_that_leave_are_cancelled_and_their_slots_return() {
        let mut t = BlockTracker::default();
        let p = t.plan(resident(), key(10, 10), 1);
        let tokens: HashMap<LandblockKey, CancelToken> =
            p.scans.iter().map(|s| (s.key, s.token.clone())).collect();
        // The viewer moves two blocks east: column 9 is now beyond radius + 1.
        let p = t.plan(resident(), key(12, 10), 1);
        for y in 9..=11 {
            let k = key(9, y);
            assert!(tokens[&k].is_cancelled(), "{k:?} cancelled");
            assert!(p.despawn.contains(&k));
            assert!(!t.blocks.contains_key(&k));
        }
        assert!(!tokens[&key(11, 10)].is_cancelled(), "still wanted");
        // The cancelled jobs reply with lost scans: the count returns.
        let before = t.in_flight;
        for y in 9..=11 {
            let k = key(9, y);
            t.receive(Err((k, 1000 + 900 + usize::from(y))));
        }
        assert_eq!(t.in_flight, before - 3);
    }

    #[test]
    fn a_replaced_bundle_cancels_the_old_scan_and_drops_its_reply() {
        let mut t = BlockTracker::default();
        let here = key(10, 10);
        let p = t.plan(vec![(here, 1, ())], here, 1);
        let old = p.scans[0].token.clone();
        let p = t.plan(vec![(here, 2, ())], here, 1);
        assert!(old.is_cancelled());
        assert_eq!(p.despawn, vec![here]);
        assert_eq!(p.scans.len(), 1);
        assert_eq!(t.in_flight, 2);
        // The old scan's reply (stale) is dropped, the new one kept.
        t.receive(Ok(scan(here, 1, Vec::new())));
        assert!(t.blocks[&here].scan.is_none());
        // A lost reply of the old bundle does not forget the new block.
        t.receive(Err((here, 1)));
        assert!(t.blocks.contains_key(&here));
        t.receive(Ok(scan(here, 2, Vec::new())));
        assert!(t.blocks[&here].scan.is_some());
        assert_eq!(t.in_flight, 0);
    }

    #[test]
    fn a_lost_scan_is_retried() {
        let mut t = BlockTracker::default();
        let here = key(10, 10);
        t.plan(vec![(here, 7, ())], here, 0);
        t.receive(Err((here, 7)));
        assert_eq!(t.in_flight, 0);
        assert!(!t.blocks.contains_key(&here));
        let p = t.plan(vec![(here, 7, ())], here, 0);
        assert_eq!(p.scans.len(), 1, "scanned again");
    }

    #[test]
    fn the_players_block_bypasses_the_in_flight_bound() {
        let mut t = BlockTracker {
            in_flight: MAX_SCANS_IN_FLIGHT,
            ..BlockTracker::default()
        };
        let here = key(10, 10);
        let p = t.plan(resident(), here, 2);
        assert_eq!(p.scans.len(), 1);
        assert_eq!(p.scans[0].key, here);
        assert_eq!(t.in_flight, MAX_SCANS_IN_FLIGHT + 1);
    }

    #[test]
    fn emitters_spawn_and_despawn_with_hysteresis() {
        let here = key(10, 10);
        let mut blocks = HashMap::new();
        // Reach 50 m: spawn below 80 m, despawn beyond 140 m.
        blocks.insert(
            here,
            BlockAudio {
                bundle: 1,
                scan: Some(Arc::new(scan(here, 1, vec![emitter(5, 0.0, 50.0)]))),
                token: CancelToken::new(),
            },
        );
        let mut live = HashMap::new();
        let at = |x: f32| [x, 0.0, 0.0];
        assert!(plan_emitters(&blocks, &live, at(85.0)).spawn.is_empty());
        let p = plan_emitters(&blocks, &live, at(79.0));
        assert_eq!(p.spawn.len(), 1);
        assert_eq!(p.spawn[0].0, here);
        live.insert(EntityId(5), here);
        // Live, between the margins: nothing changes.
        let p = plan_emitters(&blocks, &live, at(120.0));
        assert!(p.spawn.is_empty() && p.despawn.is_empty());
        let p = plan_emitters(&blocks, &live, at(141.0));
        assert_eq!(p.despawn, vec![EntityId(5)]);
        let p = plan_emitters(&blocks, &live, [f32::NAN, 0.0, 0.0]);
        assert_eq!(p.despawn, vec![EntityId(5)]);
    }

    #[test]
    fn spawns_are_nearest_first_and_bounded() {
        let here = key(10, 10);
        let emitters: Vec<_> = (0..10u64)
            .map(|i| emitter(100 - i, f64::from(i as u32) * 3.0, 50.0))
            .collect();
        let mut blocks = HashMap::new();
        blocks.insert(
            here,
            BlockAudio {
                bundle: 1,
                scan: Some(Arc::new(scan(here, 1, emitters))),
                token: CancelToken::new(),
            },
        );
        let p = plan_emitters(&blocks, &HashMap::new(), [0.0; 3]);
        assert_eq!(p.spawn.len(), MAX_SPAWNS_PER_CHECK);
        let ids: Vec<u64> = p.spawn.iter().map(|(_, e)| e.id.0).collect();
        assert_eq!(ids, vec![100, 99, 98, 97, 96, 95]);
        // Unscanned blocks contribute nothing.
        blocks.get_mut(&here).unwrap().scan = None;
        assert!(
            plan_emitters(&blocks, &HashMap::new(), [0.0; 3])
                .spawn
                .is_empty()
        );
    }
}
