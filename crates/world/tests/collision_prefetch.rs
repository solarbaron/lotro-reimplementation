// SPDX-License-Identifier: GPL-3.0-or-later
//! The hierarchy thread rule of `collision.md` §12.1 for a model caller on the asset
//! workers, on synthetic records: a job at `Prefetch` priority builds its set with one
//! thread; a job raised to `Visible` while it waits in the queue builds under the shared
//! rule.
//!
//! These tests model a caller of [`CollisionSystem::build_at`]; they do not check the pool.
//! The asset pool gives a running job no way to read its own priority, so each job here
//! reads it from a cell the test keeps in step with [`TaskHandle::set_priority`]. Only the
//! run order comes from the pool. The live bundle decoder cannot do this yet and builds
//! under `Visible`'s rule ([`CollisionSystem::build`]).

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tessera_assets::{CancelToken, PoolConfig, Priority, TaskHandle, WorkerPool};
use tessera_formats::landblock::HeightGrid;
use tessera_formats::{LandblockData, PropertyType};
use tessera_physics::geometry::{CollisionLimits, MAX_BUILD_THREADS, PhysicsPids, StaticOptions};
use tessera_world::collision::{HierarchyBuildCounts, HierarchyThreads};
use tessera_world::{CollisionInput, CollisionSystem, LandblockKey, MemoryWorldSource};

const WAIT: Duration = Duration::from_secs(30);

fn system() -> Arc<CollisionSystem> {
    Arc::new(CollisionSystem::new(
        PhysicsPids::default(),
        CollisionLimits::default(),
        StaticOptions::default(),
    ))
}

/// One flat landblock's set, built at `priority`.
fn build(sys: &CollisionSystem, priority: Priority) {
    let key = LandblockKey::new(1, 10, 20).expect("key");
    let data = LandblockData::from_heights(key.data_did(), HeightGrid::flat(0));
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, PropertyType> = HashMap::new();
    let built = sys.build_at(
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
        priority,
    );
    assert!(built.complete);
    assert!(built.set.triangle_count() > 0, "terrain triangles");
}

/// A model collision job: it reads its priority when a worker starts it (`now`, which the
/// test keeps in step with the queue's priority by hand), builds with it and reports that
/// priority with the thread cap its build was given (the pool has one worker, so no other
/// build runs in between).
fn spawn_build(
    pool: &WorkerPool,
    sys: &Arc<CollisionSystem>,
    now: &Arc<Mutex<Priority>>,
    done: &mpsc::Sender<(Priority, Option<usize>)>,
) -> TaskHandle {
    let priority = *now.lock().unwrap_or_else(PoisonError::into_inner);
    let (sys, now, done) = (Arc::clone(sys), Arc::clone(now), done.clone());
    pool.spawn(priority, CancelToken::new(), move |_| {
        let at_start = *now.lock().unwrap_or_else(PoisonError::into_inner);
        build(&sys, at_start);
        let _ = done.send((at_start, sys.hierarchy_builds().last_max_threads));
    })
    .expect("spawn")
}

/// Holds the pool's only worker until the returned sender is dropped or sent to.
fn block_worker(pool: &WorkerPool) -> (mpsc::Sender<()>, TaskHandle) {
    let (release, gate) = mpsc::channel::<()>();
    let (started, running) = mpsc::channel::<()>();
    let h = pool
        .spawn(Priority::Immediate, CancelToken::new(), move |_| {
            let _ = started.send(());
            let _ = gate.recv_timeout(WAIT);
        })
        .expect("spawn gate");
    running.recv_timeout(WAIT).expect("gate running");
    (release, h)
}

#[test]
fn prefetch_jobs_build_on_their_worker() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let sys = system();
    let (done_tx, done_rx) = mpsc::channel();
    let now = Arc::new(Mutex::new(Priority::Prefetch));
    let h = spawn_build(&pool, &sys, &now, &done_tx);
    assert_eq!(
        done_rx.recv_timeout(WAIT),
        Ok((Priority::Prefetch, Some(1)))
    );
    assert!(h.wait(WAIT).is_done());
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 1,
            shared: 0,
            last_max_threads: Some(1),
        }
    );
    assert_eq!(
        HierarchyThreads::for_priority(Priority::Prefetch).max_threads(),
        1
    );
}

#[test]
fn a_job_raised_while_it_waits_builds_under_the_shared_rule() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let sys = system();
    let (done_tx, done_rx) = mpsc::channel();
    let (release, gate) = block_worker(&pool);

    // Two Prefetch jobs wait behind the gate; one is raised to Visible before it starts.
    let raised_now = Arc::new(Mutex::new(Priority::Prefetch));
    let raised = spawn_build(&pool, &sys, &raised_now, &done_tx);
    let left_now = Arc::new(Mutex::new(Priority::Prefetch));
    let left = spawn_build(&pool, &sys, &left_now, &done_tx);
    *raised_now.lock().unwrap_or_else(PoisonError::into_inner) = Priority::Visible;
    raised.set_priority(Priority::Visible);
    assert_eq!(sys.hierarchy_builds(), HierarchyBuildCounts::default());

    drop(release);
    assert!(gate.wait(WAIT).is_done());
    // The raised job runs first (higher priority), with the shared budget's cap.
    assert_eq!(
        done_rx.recv_timeout(WAIT),
        Ok((Priority::Visible, Some(MAX_BUILD_THREADS)))
    );
    // The job left at Prefetch runs last and is given one thread.
    assert_eq!(
        done_rx.recv_timeout(WAIT),
        Ok((Priority::Prefetch, Some(1)))
    );
    assert!(raised.wait(WAIT).is_done());
    assert!(left.wait(WAIT).is_done());
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 1,
            shared: 1,
            last_max_threads: Some(1),
        }
    );
}
