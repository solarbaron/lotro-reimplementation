// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for audit findings against the pool and the task API.

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetKey, AssetState, AssetStore, CancelToken, Decoder, Handle, JobContext, MemorySource,
    PoolConfig, Priority, RawBytes, StoreConfig, TaskStatus, WorkerPool,
};

const T: Duration = Duration::from_secs(20);
const MODEL: u32 = 0x0100_0001;
const TEXTURE: u32 = 0x4100_0001;

#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    cond: Condvar,
}

impl Gate {
    fn wait(&self) {
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.cond.wait(open).unwrap();
        }
    }
    fn open(&self) {
        *self.open.lock().unwrap() = true;
        self.cond.notify_all();
    }
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < T, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A product that owns a sub-request and cancels it when dropped (RAII).
struct Model {
    texture: Handle<Vec<u8>>,
    dropped: Arc<AtomicBool>,
}

impl Drop for Model {
    fn drop(&mut self) {
        self.texture.cancel();
        self.dropped.store(true, Ordering::SeqCst);
    }
}

struct ModelDecoder {
    textures: AssetStore<MemorySource, RawBytes>,
    dropped: Arc<AtomicBool>,
}

impl Decoder for ModelDecoder {
    type Output = Model;
    type Error = Infallible;
    const NAME: &'static str = "model";

    fn decode(
        &self,
        _key: AssetKey,
        _bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<Model, Infallible> {
        Ok(Model {
            texture: self.textures.request(TEXTURE, Priority::Prefetch),
            dropped: Arc::clone(&self.dropped),
        })
    }

    fn size_of(&self, _value: &Model) -> usize {
        1
    }
}

fn gate_task(
    pool: &WorkerPool,
    priority: Priority,
    gate: &Arc<Gate>,
) -> tessera_assets::TaskHandle {
    let g = Arc::clone(gate);
    pool.spawn(priority, CancelToken::new(), move |_| g.wait())
        .unwrap()
}

/// Regression: a re-prioritised request leaves a stale queue entry behind. That entry
/// used to own the finished job, so it kept the product alive outside the cache budget
/// and dropped it under the queue lock; a product whose `Drop` cancels a queued
/// sub-request then deadlocked the worker and the pool.
fn stale_entry_scenario() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let mut texs = MemorySource::new();
    texs.insert(TEXTURE, vec![1u8; 4]);
    let textures = AssetStore::new(&pool, texs, RawBytes, StoreConfig::default());
    let mut models = MemorySource::new();
    models.insert(MODEL, vec![2u8; 4]);
    let dropped = Arc::new(AtomicBool::new(false));
    let decoder = ModelDecoder {
        textures: textures.clone(),
        dropped: Arc::clone(&dropped),
    };
    let store = AssetStore::new(
        &pool,
        models,
        decoder,
        StoreConfig {
            // Products are delivered but never cached.
            cache_budget: 0,
            ..StoreConfig::default()
        },
    );

    // Keep the only worker busy while the queue is arranged.
    let gate1 = Arc::new(Gate::default());
    let busy1 = gate_task(&pool, Priority::Immediate, &gate1);
    wait_until("first gate to run", || {
        matches!(busy1.status(), TaskStatus::Running)
    });
    let h = store.request(MODEL, Priority::Prefetch);
    let gate2 = Arc::new(Gate::default());
    let busy2 = gate_task(&pool, Priority::Visible, &gate2);
    // Leaves a stale Prefetch entry for the model job behind the gate.
    h.set_priority(Priority::Immediate);

    gate1.open();
    assert!(matches!(h.wait(T), AssetState::Ready(_)));
    wait_until("second gate to run", || {
        matches!(busy2.status(), TaskStatus::Running)
    });
    // Release every reference user code holds: the product must go now.
    drop(h);
    drop(store.poll_completions(usize::MAX));
    assert!(store.cached(MODEL).is_none());
    wait_until("the product to be released", || {
        dropped.load(Ordering::SeqCst)
    });

    // The worker reaches the stale entry; the pool must stay usable.
    gate2.open();
    let last = pool
        .spawn(Priority::Prefetch, CancelToken::new(), |_| {})
        .unwrap();
    assert!(last.wait(T).is_done());
    let tex = textures.request(TEXTURE, Priority::Immediate);
    assert!(matches!(tex.wait(T), AssetState::Ready(_)));
}

#[test]
fn stale_queue_entries_do_not_own_finished_jobs() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        stale_entry_scenario();
        let _ = tx.send(());
    });
    rx.recv_timeout(T * 2)
        .expect("scenario failed or the pool deadlocked");
}

/// Regression: `TaskHandle::wait(Duration::MAX)` waits without a deadline instead of
/// panicking on `Instant` overflow.
#[test]
fn task_wait_accepts_duration_max() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        ..PoolConfig::default()
    });
    let t = pool
        .spawn(Priority::Visible, CancelToken::new(), |_| {})
        .unwrap();
    assert!(t.wait(Duration::MAX).is_done());
    // A finished task returns at once for any timeout.
    assert!(t.wait(Duration::ZERO).is_done());
}
