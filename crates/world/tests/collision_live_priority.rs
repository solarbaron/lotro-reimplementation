// SPDX-License-Identifier: GPL-3.0-or-later
//! The hierarchy thread rule of `collision.md` §12.1 for a decoder running on a real
//! [`AssetStore`] job, on synthetic records: the rule follows the job's live priority
//! ([`JobContext::current_priority`]) when the hierarchy build starts, not the priority the
//! worker took the job out of the queue at ([`JobContext::priority`]).
//!
//! The decoder holds its job after the worker has started it, the test changes the request's
//! priority with [`Handle::set_priority`], and the decoder then passes its own `JobContext`
//! to [`CollisionSystem::build_at`]. The two priorities differ in each case, so a
//! `PrioritySource` for `JobContext` that read [`JobContext::priority`] fails here.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::mpsc;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tessera_assets::{
    AssetKey, AssetStore, Decoder, Handle, JobContext, MemorySource, PoolConfig, Priority,
    StoreConfig, WorkerPool,
};
use tessera_formats::landblock::HeightGrid;
use tessera_formats::{LandblockData, PropertyType};
use tessera_physics::geometry::{CollisionLimits, MAX_BUILD_THREADS, PhysicsPids, StaticOptions};
use tessera_world::collision::HierarchyBuildCounts;
use tessera_world::{CollisionInput, CollisionSystem, LandblockKey, MemoryWorldSource};

const WAIT: Duration = Duration::from_secs(30);
const RECORD: u32 = 0x7000_0001;

/// What the probe decoder saw: the job's start priority, its live priority as the build
/// started, and the system's build counts after the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    at_start: Priority,
    live: Priority,
    builds: HierarchyBuildCounts,
}

/// A decoder that tells the test it runs, waits for the test's go, then builds one flat
/// landblock's collision set with its own `JobContext` as the priority source.
struct Probe {
    sys: CollisionSystem,
    started: Mutex<mpsc::Sender<()>>,
    go: Mutex<mpsc::Receiver<()>>,
}

impl Decoder for Probe {
    type Output = Seen;
    type Error = Infallible;
    const NAME: &'static str = "collision-priority-probe";

    fn decode(
        &self,
        _key: AssetKey,
        _bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<Seen, Infallible> {
        let at_start = ctx.priority();
        let _ = self
            .started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(());
        let _ = self
            .go
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(WAIT);
        let key = LandblockKey::new(1, 10, 20).expect("key");
        let data = LandblockData::from_heights(key.data_did(), HeightGrid::flat(0));
        let src = MemoryWorldSource::new();
        let schema: HashMap<u32, PropertyType> = HashMap::new();
        let built = self.sys.build_at(
            &CollisionInput {
                key,
                data: &data,
                terrain: None,
                info: None,
                generated: &[],
                water: None,
                source: &src,
                schema: &schema,
            },
            ctx,
        );
        assert!(built.set.triangle_count() > 0, "terrain triangles");
        Ok(Seen {
            at_start,
            live: ctx.current_priority(),
            builds: self.sys.hierarchy_builds(),
        })
    }

    fn size_of(&self, _value: &Seen) -> usize {
        std::mem::size_of::<Seen>()
    }
}

/// Requests the record at `first`, changes the request to `then` once the decoder runs,
/// lets the decoder build, and returns what it saw.
fn run(first: Priority, then: Priority) -> Seen {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let (started_tx, started_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel();
    let mut source = MemorySource::new();
    source.insert(RECORD, vec![0u8; 4]);
    let store = AssetStore::new(
        &pool,
        source,
        Probe {
            sys: CollisionSystem::new(
                PhysicsPids::default(),
                CollisionLimits::default(),
                StaticOptions::default(),
            ),
            started: Mutex::new(started_tx),
            go: Mutex::new(go_rx),
        },
        StoreConfig::default(),
    );
    let handle: Handle<Seen> = store.request(RECORD, first);
    started_rx.recv_timeout(WAIT).expect("decoder running");
    handle.set_priority(then);
    go_tx.send(()).expect("go");
    let seen = handle.wait(WAIT).ready().map(|s| **s);
    seen.expect("decoded")
}

#[test]
fn a_job_raised_while_it_runs_builds_under_the_shared_rule() {
    let seen = run(Priority::Prefetch, Priority::Visible);
    assert_eq!(seen.at_start, Priority::Prefetch);
    assert_eq!(seen.live, Priority::Visible);
    assert_eq!(
        seen.builds,
        HierarchyBuildCounts {
            worker: 0,
            shared: 1,
            last_max_threads: Some(MAX_BUILD_THREADS),
        }
    );
}

#[test]
fn a_job_lowered_while_it_runs_builds_on_its_worker() {
    let seen = run(Priority::Visible, Priority::Prefetch);
    assert_eq!(seen.at_start, Priority::Visible);
    assert_eq!(seen.live, Priority::Prefetch);
    assert_eq!(
        seen.builds,
        HierarchyBuildCounts {
            worker: 1,
            shared: 0,
            last_max_threads: Some(1),
        }
    );
}
