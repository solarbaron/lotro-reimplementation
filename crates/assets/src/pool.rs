// SPDX-License-Identifier: GPL-3.0-or-later
//! The worker pool and its priority queue.

use std::collections::{BinaryHeap, HashMap};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::cancel::cancel_epoch;
use crate::{AssetError, Priority, lock};

/// Upper bound on [`PoolConfig::workers`]; larger values are clamped to it.
pub const MAX_WORKERS: usize = 256;

/// Scheduling state of one job: its priority cell (0 = not in the queue: running,
/// finished or never queued; otherwise `priority.to_cell()` of the heap entries that are
/// valid) and the queue id it was given when it was last queued.
///
/// Only touched with the queue lock held, so plain loads and stores suffice; it is atomic
/// so that jobs stay `Sync`.
#[derive(Debug, Default)]
pub(crate) struct SchedCell {
    prio: AtomicU8,
    id: AtomicU64,
}

impl SchedCell {
    fn get(&self) -> Option<Priority> {
        Priority::from_cell(self.prio.load(Ordering::Relaxed))
    }
    fn set(&self, p: Option<Priority>) {
        self.prio
            .store(p.map_or(0, Priority::to_cell), Ordering::Relaxed);
    }
    fn id(&self) -> u64 {
        self.id.load(Ordering::Relaxed)
    }
    fn set_id(&self, id: u64) {
        self.id.store(id, Ordering::Relaxed);
    }
}

/// A unit of work the pool can run. Implemented by the stores.
pub(crate) trait Job: Send + Sync {
    /// The job's scheduling cell.
    fn sched(&self) -> &SchedCell;
    /// Runs the job on a worker thread. Must not panic (the pool catches it anyway).
    fn run(&self);
    /// Finishes the job without running it (pool shut down, or `run` panicked).
    fn abort(&self, err: AssetError);
    /// Whether nobody wants the job any more (every request cancelled, or its store is
    /// gone); `None` if that cannot be told right now without blocking.
    ///
    /// Called with the queue lock held: it must not block on another lock (use
    /// `try_lock`) and must not release the last reference to anything whose drop takes
    /// pool locks.
    fn abandoned(&self) -> Option<bool>;
    /// Settles a job the pool took out of the queue because [`Job::abandoned`] said so.
    /// Called with no lock held. If interest came back in the meantime the job must queue
    /// itself again.
    fn reap(&self);
}

/// One heap entry. It names its job by queue id and holds no reference to it, so a stale
/// entry (left behind by re-prioritisation or withdrawal) neither keeps a finished job and
/// its product alive nor ever releases one while the queue lock is held.
struct Entry {
    priority: Priority,
    seq: u64,
    id: u64,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    /// Higher priority first; within a priority, lower sequence number (older) first.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

#[derive(Default)]
struct QueueState {
    heap: BinaryHeap<Entry>,
    /// The queued jobs, by queue id: the queue's only strong references. Each has at least
    /// one valid heap entry (the heap may also hold stale entries left behind by
    /// re-prioritisation or withdrawal; they are skipped when popped).
    ///
    /// A reference taken out of this map while the lock is held must be released only
    /// after the lock is: dropping a job can drop a product, and a product's `Drop` may
    /// call back into the pool.
    queued: HashMap<u64, Arc<dyn Job>>,
    seq: u64,
    /// Last queue id handed out. Ids are never reused, so an entry left from an earlier
    /// stay in the queue never matches a job queued again.
    next_id: u64,
    shutdown: bool,
    /// [`cancel_epoch`] at the last complete sweep for abandoned jobs.
    swept_epoch: u64,
}

impl QueueState {
    /// Jobs with a valid heap entry.
    fn live(&self) -> usize {
        self.queued.len()
    }

    fn is_valid(&self, e: &Entry) -> bool {
        self.queued
            .get(&e.id)
            .is_some_and(|j| j.sched().id() == e.id && j.sched().get() == Some(e.priority))
    }

    fn push_entry(&mut self, id: u64, priority: Priority) {
        self.seq += 1;
        self.heap.push(Entry {
            priority,
            seq: self.seq,
            id,
        });
    }

    /// Queues a job that is not queued under a fresh id. Returns a reference the caller
    /// must release after unlocking: the job's previous queue reference if it was (against
    /// the contract) still queued.
    fn insert(&mut self, job: Arc<dyn Job>, priority: Priority) -> Option<Arc<dyn Job>> {
        let previous = if job.sched().get().is_some() {
            self.queued.remove(&job.sched().id())
        } else {
            None
        };
        self.next_id += 1;
        let id = self.next_id;
        job.sched().set_id(id);
        job.sched().set(Some(priority));
        self.queued.insert(id, job);
        self.push_entry(id, priority);
        previous
    }

    /// Takes the queued job behind `sched` out of the queue (its heap entries go stale).
    /// The caller releases the returned reference after unlocking.
    fn remove(&mut self, sched: &SchedCell) -> Option<Arc<dyn Job>> {
        sched.set(None);
        let job = self.queued.remove(&sched.id());
        self.maybe_compact();
        job
    }

    /// Drops stale entries once they outnumber live ones, so re-prioritising cannot grow
    /// the heap without bound. Entries hold no job references, so this never releases one.
    fn maybe_compact(&mut self) {
        if self.heap.len() > 2 * self.live() + 64 {
            let mut heap = std::mem::take(&mut self.heap);
            heap.retain(|e| self.is_valid(e));
            self.heap = heap;
        }
    }

    /// Takes every queued job that reports itself abandoned out of the queue (their heap
    /// entries go stale) and appends it to `out`. Skipped when no token was cancelled
    /// since the last complete sweep.
    fn sweep_abandoned(&mut self, out: &mut Vec<Arc<dyn Job>>) {
        let epoch = cancel_epoch();
        if epoch == self.swept_epoch {
            return;
        }
        let mut complete = true;
        let mut ids = Vec::new();
        for (&id, job) in &self.queued {
            match job.abandoned() {
                Some(true) => ids.push(id),
                Some(false) => {}
                None => complete = false,
            }
        }
        let removed = ids.len();
        for id in ids {
            if let Some(job) = self.queued.remove(&id) {
                job.sched().set(None);
                // Handed to the caller, which reaps it with no lock held.
                out.push(job);
            }
        }
        if complete {
            self.swept_epoch = epoch;
        }
        if removed > 0 {
            tracing::trace!(removed, "abandoned asset jobs swept from the queue");
            self.maybe_compact();
        }
    }

    /// Takes the highest-priority queued job out of the queue, discarding the stale
    /// entries above it (they hold no references, so nothing is released here).
    fn pop_valid(&mut self) -> Option<Arc<dyn Job>> {
        while let Some(e) = self.heap.pop() {
            if self.is_valid(&e)
                && let Some(job) = self.queued.remove(&e.id)
            {
                job.sched().set(None);
                return Some(job);
            }
        }
        None
    }
}

/// State shared by the pool handle, the worker threads and the stores.
pub(crate) struct PoolShared {
    queue: Mutex<QueueState>,
    cond: Condvar,
    workers: usize,
    max_queued: usize,
    busy: AtomicUsize,
    executed: AtomicU64,
}

impl PoolShared {
    /// Queues a job that is not currently queued.
    ///
    /// When the queue is full and `reaped` is given, jobs whose requests were all cancelled
    /// (or whose store is gone) are first taken out of the queue to make room; they are
    /// appended to `reaped` and the caller must call [`Job::reap`] on each once it holds no
    /// lock.
    pub(crate) fn submit(
        &self,
        job: Arc<dyn Job>,
        priority: Priority,
        reaped: Option<&mut Vec<Arc<dyn Job>>>,
    ) -> Result<(), AssetError> {
        let mut q = lock(&self.queue);
        if q.shutdown {
            return Err(AssetError::Shutdown);
        }
        if q.live() >= self.max_queued {
            if let Some(out) = reaped {
                q.sweep_abandoned(out);
            }
            if q.live() >= self.max_queued {
                return Err(AssetError::QueueFull {
                    capacity: self.max_queued,
                });
            }
        }
        let previous = q.insert(job, priority);
        let len = q.live();
        drop(q);
        drop(previous);
        self.cond.notify_one();
        tracing::trace!(queue_len = len, %priority, "asset job queued");
        Ok(())
    }

    /// Moves a still-queued job to `priority`. Returns false if the job is no longer
    /// queued (a worker took it) or already has that priority.
    pub(crate) fn reprioritize(&self, job: &Arc<dyn Job>, priority: Priority) -> bool {
        let mut q = lock(&self.queue);
        match job.sched().get() {
            Some(current) if current != priority => {
                job.sched().set(Some(priority));
                q.push_entry(job.sched().id(), priority);
                q.maybe_compact();
                drop(q);
                // The job count is unchanged; a sleeping worker has nothing new to do.
                true
            }
            _ => false,
        }
    }

    /// Takes a still-queued job out of the queue (its heap entry goes stale). Returns
    /// false if it was not queued (a worker took it, or it was never queued).
    ///
    /// Takes the job's scheduling cell rather than the job, so it can be called from
    /// contexts that cannot name the job as `dyn Job` (a store's `Drop`).
    pub(crate) fn withdraw(&self, sched: &SchedCell) -> bool {
        let mut q = lock(&self.queue);
        if sched.get().is_none() {
            return false;
        }
        let job = q.remove(sched);
        drop(q);
        // Released with no lock of the pool held.
        drop(job);
        true
    }

    fn queued(&self) -> usize {
        lock(&self.queue).live()
    }
}

fn worker_loop(shared: Arc<PoolShared>, index: usize) {
    let _span = tracing::debug_span!("asset_worker", index).entered();
    loop {
        let job = {
            let mut q = lock(&shared.queue);
            loop {
                if q.shutdown {
                    return;
                }
                if let Some(job) = q.pop_valid() {
                    break job;
                }
                q = shared
                    .cond
                    .wait(q)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        shared.busy.fetch_add(1, Ordering::Relaxed);
        if catch_unwind(AssertUnwindSafe(|| job.run())).is_err() {
            tracing::error!("asset job panicked outside its guarded stages");
            // `abort` is a no-op if the job already finished.
            let _ = catch_unwind(AssertUnwindSafe(|| {
                job.abort(AssetError::Panicked {
                    did: 0,
                    stage: "job",
                });
            }));
        }
        shared.busy.fetch_sub(1, Ordering::Relaxed);
        shared.executed.fetch_add(1, Ordering::Relaxed);
    }
}

/// Configuration of a [`WorkerPool`].
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Worker threads; 0 picks `available cores − 2`, at least 1 (`docs/ARCHITECTURE.md`
    /// §21). Clamped to [`MAX_WORKERS`].
    pub workers: usize,
    /// Maximum jobs waiting in the queue across all stores; further requests fail with
    /// [`AssetError::QueueFull`].
    pub max_queued: usize,
    /// Prefix for worker thread names.
    pub thread_name: String,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            workers: 0,
            max_queued: 65_536,
            thread_name: "tessera-asset".to_owned(),
        }
    }
}

/// Counters of a [`WorkerPool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolStats {
    /// Worker threads.
    pub workers: usize,
    /// Jobs waiting in the queue.
    pub queued: usize,
    /// Workers currently running a job.
    pub busy: usize,
    /// Jobs run since the pool started.
    pub executed: u64,
}

struct PoolHandle {
    shared: Arc<PoolShared>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Drop for PoolHandle {
    fn drop(&mut self) {
        lock(&self.shared.queue).shutdown = true;
        self.shared.cond.notify_all();
        let me = std::thread::current().id();
        for t in lock(&self.threads).drain(..) {
            if t.thread().id() != me {
                let _ = t.join();
            }
        }
        // Wake everyone still waiting on a queued job.
        let pending: Vec<Arc<dyn Job>> = {
            let mut q = lock(&self.shared.queue);
            let mut out = Vec::new();
            while let Some(job) = q.pop_valid() {
                out.push(job);
            }
            q.heap.clear();
            // Every queued job has a valid entry, so this is empty; released below anyway.
            out.extend(q.queued.drain().map(|(_, j)| j));
            out
        };
        if !pending.is_empty() {
            tracing::debug!(
                jobs = pending.len(),
                "asset pool shut down with queued jobs"
            );
        }
        for job in pending {
            let _ = catch_unwind(AssertUnwindSafe(|| job.abort(AssetError::Shutdown)));
        }
    }
}

/// A fixed set of worker threads draining one priority queue.
///
/// Cloning is cheap and shares the pool. The threads stop, and jobs still queued finish
/// as [`AssetError::Shutdown`], when the last clone and the last
/// [`AssetStore`](crate::AssetStore) using the pool are dropped. Dropping blocks until
/// running jobs finish.
#[derive(Clone)]
pub struct WorkerPool {
    handle: Arc<PoolHandle>,
}

impl WorkerPool {
    /// Starts the worker threads.
    ///
    /// If the OS refuses to spawn some threads the pool runs with the ones it got (at least
    /// one is needed; if none can be spawned every job fails with
    /// [`AssetError::Shutdown`]).
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        let wanted = if config.workers == 0 {
            std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .saturating_sub(2)
                .max(1)
        } else {
            config.workers
        }
        .min(MAX_WORKERS);
        let shared = Arc::new(PoolShared {
            queue: Mutex::new(QueueState::default()),
            cond: Condvar::new(),
            workers: wanted,
            max_queued: config.max_queued.max(1),
            busy: AtomicUsize::new(0),
            executed: AtomicU64::new(0),
        });
        let mut threads = Vec::with_capacity(wanted);
        for i in 0..wanted {
            let s = Arc::clone(&shared);
            match std::thread::Builder::new()
                .name(format!("{}-{i}", config.thread_name))
                .spawn(move || worker_loop(s, i))
            {
                Ok(t) => threads.push(t),
                Err(e) => tracing::warn!(error = %e, "could not spawn asset worker"),
            }
        }
        if threads.is_empty() {
            tracing::error!("no asset worker could be spawned");
            lock(&shared.queue).shutdown = true;
        }
        tracing::debug!(workers = threads.len(), "asset pool started");
        Self {
            handle: Arc::new(PoolHandle {
                shared,
                threads: Mutex::new(threads),
            }),
        }
    }

    pub(crate) fn shared(&self) -> &Arc<PoolShared> {
        &self.handle.shared
    }

    /// Number of worker threads.
    #[must_use]
    pub fn workers(&self) -> usize {
        lock(&self.handle.threads).len()
    }

    /// Jobs waiting in the queue.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.handle.shared.queued()
    }

    /// Snapshot of the pool's counters.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let s = &self.handle.shared;
        PoolStats {
            workers: self.workers(),
            queued: s.queued(),
            busy: s.busy.load(Ordering::Relaxed),
            executed: s.executed.load(Ordering::Relaxed),
        }
    }
}

impl fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerPool")
            .field("workers", &self.handle.shared.workers)
            .field("max_queued", &self.handle.shared.max_queued)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct Probe {
        sched: SchedCell,
        name: u32,
        out: Mutex<mpsc::Sender<u32>>,
    }

    impl Job for Probe {
        fn sched(&self) -> &SchedCell {
            &self.sched
        }
        fn run(&self) {
            let _ = lock(&self.out).send(self.name);
        }
        fn abort(&self, _err: AssetError) {
            let _ = lock(&self.out).send(1000 + self.name);
        }
        fn abandoned(&self) -> Option<bool> {
            Some(self.name >= 100)
        }
        fn reap(&self) {
            let _ = lock(&self.out).send(2000 + self.name);
        }
    }

    fn bare(max_queued: usize) -> PoolShared {
        PoolShared {
            queue: Mutex::new(QueueState::default()),
            cond: Condvar::new(),
            workers: 0,
            max_queued,
            busy: AtomicUsize::new(0),
            executed: AtomicU64::new(0),
        }
    }

    fn probe(name: u32, tx: &mpsc::Sender<u32>) -> Arc<dyn Job> {
        Arc::new(Probe {
            sched: SchedCell::default(),
            name,
            out: Mutex::new(tx.clone()),
        })
    }

    #[test]
    fn queue_pops_by_priority_then_fifo_and_skips_stale() {
        let (tx, _rx) = mpsc::channel();
        let mut q = QueueState::default();
        let jobs: Vec<_> = (0..4).map(|i| probe(i, &tx)).collect();
        for (j, p) in jobs.iter().zip([
            Priority::Prefetch,
            Priority::Visible,
            Priority::Prefetch,
            Priority::Immediate,
        ]) {
            assert!(q.insert(Arc::clone(j), p).is_none());
        }
        // Re-prioritise job 2 (Prefetch → Immediate): its old entry becomes stale.
        jobs[2].sched().set(Some(Priority::Immediate));
        q.push_entry(jobs[2].sched().id(), Priority::Immediate);
        let order: Vec<usize> = std::iter::from_fn(|| q.pop_valid())
            .map(|j| jobs.iter().position(|x| Arc::ptr_eq(x, &j)).unwrap())
            .collect();
        assert_eq!(order, vec![3, 2, 1, 0]);
        assert_eq!(q.live(), 0);
    }

    #[test]
    fn pool_runs_and_aborts_on_shutdown() {
        let (tx, rx) = mpsc::channel();
        let pool = WorkerPool::new(PoolConfig {
            workers: 2,
            ..PoolConfig::default()
        });
        assert_eq!(pool.workers(), 2);
        pool.shared()
            .submit(probe(7, &tx), Priority::Visible, None)
            .unwrap();
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(10)), Ok(7));
        drop(pool);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn queue_capacity_is_enforced() {
        let (tx, _rx) = mpsc::channel();
        let shared = bare(2);
        shared
            .submit(probe(1, &tx), Priority::Prefetch, None)
            .unwrap();
        shared
            .submit(probe(2, &tx), Priority::Prefetch, None)
            .unwrap();
        assert!(matches!(
            shared.submit(probe(3, &tx), Priority::Immediate, None),
            Err(AssetError::QueueFull { capacity: 2 })
        ));
        lock(&shared.queue).shutdown = true;
        assert!(matches!(
            shared.submit(probe(4, &tx), Priority::Immediate, None),
            Err(AssetError::Shutdown)
        ));
    }

    #[test]
    fn compaction_bounds_stale_entries() {
        let (tx, _rx) = mpsc::channel();
        let shared = bare(10);
        let j = probe(1, &tx);
        shared
            .submit(Arc::clone(&j), Priority::Prefetch, None)
            .unwrap();
        for i in 0..1000 {
            let p = if i % 2 == 0 {
                Priority::Immediate
            } else {
                Priority::Prefetch
            };
            assert!(shared.reprioritize(&j, p));
        }
        assert!(!shared.reprioritize(&j, Priority::Prefetch));
        let q = lock(&shared.queue);
        assert!(q.heap.len() <= 2 * q.live() + 65, "heap {}", q.heap.len());
        assert_eq!(q.live(), 1);
    }

    /// Regression: withdrawing a queued job frees its queue slot at once.
    #[test]
    fn withdraw_frees_capacity() {
        let (tx, _rx) = mpsc::channel();
        let shared = bare(2);
        let a = probe(1, &tx);
        shared
            .submit(Arc::clone(&a), Priority::Prefetch, None)
            .unwrap();
        shared
            .submit(probe(2, &tx), Priority::Prefetch, None)
            .unwrap();
        assert!(shared.withdraw(a.sched()));
        assert!(!shared.withdraw(a.sched()), "already withdrawn");
        assert_eq!(shared.queued(), 1);
        shared
            .submit(probe(3, &tx), Priority::Immediate, None)
            .unwrap();
        let mut q = lock(&shared.queue);
        assert_eq!(std::iter::from_fn(|| q.pop_valid()).count(), 2);
        assert_eq!(q.live(), 0);
    }

    /// Regression: stale heap entries hold no reference to their job, so a job that was
    /// re-prioritised, withdrawn or run is owned only by its other holders afterwards.
    #[test]
    fn stale_entries_hold_no_references() {
        let (tx, _rx) = mpsc::channel();
        let shared = bare(10);
        let a = probe(1, &tx);
        let b = probe(2, &tx);
        shared
            .submit(Arc::clone(&a), Priority::Prefetch, None)
            .unwrap();
        shared
            .submit(Arc::clone(&b), Priority::Prefetch, None)
            .unwrap();
        assert!(shared.reprioritize(&a, Priority::Immediate));
        assert!(shared.reprioritize(&b, Priority::Visible));
        assert_eq!(Arc::strong_count(&a), 2);
        assert!(shared.withdraw(b.sched()));
        assert_eq!(Arc::strong_count(&b), 1);
        let popped = lock(&shared.queue).pop_valid().unwrap();
        assert!(Arc::ptr_eq(&popped, &a));
        drop(popped);
        assert_eq!(Arc::strong_count(&a), 1);
        let mut q = lock(&shared.queue);
        assert!(q.heap.len() >= 2, "stale entries are still queued");
        assert!(q.pop_valid().is_none());
        assert_eq!(q.live(), 0);
        drop(q);
        // Queued again, the job gets a fresh id: old entries never match it.
        shared
            .submit(Arc::clone(&a), Priority::Prefetch, None)
            .unwrap();
        assert_eq!(shared.queued(), 1);
        let mut q = lock(&shared.queue);
        assert!(q.pop_valid().is_some());
        assert!(q.pop_valid().is_none());
    }

    /// Regression: a full queue of abandoned jobs (cancelled through a token only) is swept
    /// when a new job needs room, and the swept jobs are handed back for reaping.
    #[test]
    fn full_queue_sweeps_abandoned_jobs() {
        let (tx, rx) = mpsc::channel();
        let shared = bare(2);
        shared
            .submit(probe(100, &tx), Priority::Prefetch, None)
            .unwrap();
        shared
            .submit(probe(1, &tx), Priority::Prefetch, None)
            .unwrap();
        // Without a reap list nothing is swept.
        assert!(matches!(
            shared.submit(probe(2, &tx), Priority::Immediate, None),
            Err(AssetError::QueueFull { capacity: 2 })
        ));
        // Make sure the epoch differs from the last sweep.
        crate::CancelToken::new().cancel();
        let mut reaped = Vec::new();
        shared
            .submit(probe(3, &tx), Priority::Immediate, Some(&mut reaped))
            .unwrap();
        assert_eq!(reaped.len(), 1);
        for j in reaped {
            j.reap();
        }
        assert_eq!(rx.try_recv(), Ok(2100));
        assert_eq!(shared.queued(), 2);
        // Nothing abandoned is left: the next one is refused.
        let mut reaped = Vec::new();
        crate::CancelToken::new().cancel();
        assert!(
            shared
                .submit(probe(4, &tx), Priority::Immediate, Some(&mut reaped))
                .is_err()
        );
        assert!(reaped.is_empty());
    }

    #[test]
    fn worker_count_is_clamped() {
        let pool = WorkerPool::new(PoolConfig {
            workers: usize::MAX,
            ..PoolConfig::default()
        });
        assert!(pool.workers() <= MAX_WORKERS);
        assert!(pool.workers() >= 1);
    }
}
