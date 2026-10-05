// SPDX-License-Identifier: GPL-3.0-or-later
//! Generic prioritised, cancellable tasks on the shared [`WorkerPool`].
//!
//! The stores run "read a record, decode it" jobs. Other systems need the same workers for
//! work that is not tied to a record, for example per-frame animation sampling
//! (`docs/ARCHITECTURE.md` §20.3 step 6 and §21). [`WorkerPool::spawn`] queues such a task
//! in the same priority queue, with the same cancellation rules as asset requests:
//!
//! - a task cancelled while queued never runs; [`TaskHandle::cancel`] takes it out of the
//!   queue at once, a task cancelled only through a shared [`CancelToken`] is dropped when a
//!   worker reaches it (or when the queue is full and needs room);
//! - a running task sees the cancellation through [`JobContext::is_cancelled`];
//! - the closure is dropped exactly once in every case (run, cancelled, pool shut down,
//!   panicked), so a drop guard inside the closure is a reliable "finished or abandoned"
//!   signal.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::pool::{Job, PoolShared, SchedCell};
use crate::{AssetError, CancelToken, JobContext, Priority, WorkerPool, lock};

/// Where a task is in its life.
#[derive(Debug, Clone)]
pub enum TaskStatus {
    /// Waiting in the queue.
    Queued,
    /// A worker is running it.
    Running,
    /// The closure returned.
    Done,
    /// Cancelled before it ran (the closure was dropped without being called).
    Cancelled,
    /// The pool shut down before it ran, or the closure panicked.
    Failed(AssetError),
}

impl TaskStatus {
    /// Whether the task will not change state any more.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }

    /// Whether the closure ran to completion.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done)
    }

    /// Whether the task was cancelled before it ran.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

type Work = Box<dyn FnOnce(&JobContext<'_>) + Send + 'static>;

struct TaskJob {
    sched: SchedCell,
    token: CancelToken,
    work: Mutex<Option<Work>>,
    status: Mutex<TaskStatus>,
    cond: Condvar,
}

impl TaskJob {
    fn take_work(&self) -> Option<Work> {
        lock(&self.work).take()
    }

    /// Sets the final status unless one is already set.
    fn finish(&self, status: TaskStatus) {
        let mut s = lock(&self.status);
        if !s.is_finished() {
            *s = status;
        }
        drop(s);
        self.cond.notify_all();
    }

    /// Drops the closure without running it and records `status`.
    fn discard(&self, status: TaskStatus) {
        let work = self.take_work();
        // The closure (and whatever guard it owns) is dropped with no lock held.
        drop(work);
        self.finish(status);
    }
}

impl Job for TaskJob {
    fn sched(&self) -> &SchedCell {
        &self.sched
    }

    fn run(&self) {
        if self.token.is_cancelled() {
            self.discard(TaskStatus::Cancelled);
            return;
        }
        let Some(work) = self.take_work() else {
            // Already discarded (cancelled while being popped); status is set.
            return;
        };
        *lock(&self.status) = TaskStatus::Running;
        let token = &self.token;
        let cancelled = || token.is_cancelled();
        let ctx = JobContext::new(&cancelled);
        let result = catch_unwind(AssertUnwindSafe(move || work(&ctx)));
        match result {
            Ok(()) => self.finish_running(TaskStatus::Done),
            Err(_) => {
                tracing::error!("worker task panicked");
                self.finish_running(TaskStatus::Failed(AssetError::Panicked {
                    did: 0,
                    stage: "task",
                }));
            }
        }
    }

    fn abort(&self, err: AssetError) {
        self.discard(TaskStatus::Failed(err));
    }

    fn abandoned(&self) -> Option<bool> {
        Some(self.token.is_cancelled())
    }

    fn reap(&self) {
        self.discard(TaskStatus::Cancelled);
    }
}

impl TaskJob {
    fn finish_running(&self, status: TaskStatus) {
        *lock(&self.status) = status;
        self.cond.notify_all();
    }
}

/// What [`WorkerPool::spawn`] returns: the task's status, cancellation and priority.
///
/// Dropping the handle does not cancel the task.
pub struct TaskHandle {
    job: Arc<TaskJob>,
    shared: Arc<PoolShared>,
}

impl TaskHandle {
    /// The task's current status.
    #[must_use]
    pub fn status(&self) -> TaskStatus {
        lock(&self.job.status).clone()
    }

    /// Whether the task will not change state any more.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        lock(&self.job.status).is_finished()
    }

    /// The task's cancellation token.
    #[must_use]
    pub fn token(&self) -> &CancelToken {
        &self.job.token
    }

    /// Cancels the task. A queued task leaves the queue at once and its closure is dropped
    /// on this thread; a running task sees [`JobContext::is_cancelled`] turn true.
    pub fn cancel(&self) {
        self.job.token.cancel();
        if self.shared.withdraw(&self.job.sched) {
            self.job.discard(TaskStatus::Cancelled);
        }
    }

    /// Moves a still-queued task to `priority` (no effect once a worker took it).
    pub fn set_priority(&self, priority: Priority) {
        let job: Arc<dyn Job> = self.job.clone();
        self.shared.reprioritize(&job, priority);
    }

    /// Blocks until the task finished or `timeout` passed; returns the status then.
    /// Meant for tests and tools, never for the frame loop.
    #[must_use]
    pub fn wait(&self, timeout: Duration) -> TaskStatus {
        let deadline = Instant::now() + timeout;
        let mut s = lock(&self.job.status);
        while !s.is_finished() {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            s = self
                .job
                .cond
                .wait_timeout(s, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        s.clone()
    }
}

impl fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskHandle")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl WorkerPool {
    /// Queues `work` at `priority`, cancellable through `token` (pass a clone of one token
    /// to several tasks to cancel them as a group).
    ///
    /// The closure runs at most once on a worker thread, with a [`JobContext`] that reports
    /// the token's state. It is dropped without running if the task is cancelled first or
    /// the pool shuts down, and also when this call fails (queue full or pool shut down):
    /// in every case it is dropped exactly once. A panic in the closure is caught and fails
    /// only this task.
    ///
    /// # Errors
    ///
    /// [`AssetError::QueueFull`] when [`PoolConfig::max_queued`](crate::PoolConfig) tasks
    /// and jobs are already waiting, [`AssetError::Shutdown`] when the pool has no workers.
    pub fn spawn<F>(
        &self,
        priority: Priority,
        token: CancelToken,
        work: F,
    ) -> Result<TaskHandle, AssetError>
    where
        F: FnOnce(&JobContext<'_>) + Send + 'static,
    {
        let job = Arc::new(TaskJob {
            sched: SchedCell::default(),
            token,
            work: Mutex::new(Some(Box::new(work))),
            status: Mutex::new(TaskStatus::Queued),
            cond: Condvar::new(),
        });
        let shared = Arc::clone(self.shared());
        let mut reaped = Vec::new();
        let r = shared.submit(job.clone(), priority, Some(&mut reaped));
        for j in reaped {
            j.reap();
        }
        match r {
            Ok(()) => Ok(TaskHandle { job, shared }),
            Err(e) => {
                job.discard(TaskStatus::Failed(e.clone()));
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PoolConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    /// Counts drops, to check the closure is dropped exactly once.
    struct Guard(Arc<AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn pool(workers: usize) -> WorkerPool {
        WorkerPool::new(PoolConfig {
            workers,
            ..PoolConfig::default()
        })
    }

    #[test]
    fn runs_and_reports_done() {
        let p = pool(2);
        let (tx, rx) = mpsc::channel();
        let h = p
            .spawn(Priority::Visible, CancelToken::new(), move |ctx| {
                assert!(!ctx.is_cancelled());
                tx.send(42).unwrap();
            })
            .unwrap();
        assert!(h.wait(Duration::from_secs(10)).is_done());
        assert_eq!(rx.recv().unwrap(), 42);
    }

    #[test]
    fn cancel_while_queued_drops_without_running() {
        let p = pool(1);
        // Block the single worker.
        let (block_tx, block_rx) = mpsc::channel::<()>();
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let blocker = p
            .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                let _ = block_rx.recv_timeout(Duration::from_secs(10));
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let ran = Arc::new(AtomicUsize::new(0));
        let g = Guard(drops.clone());
        let r = ran.clone();
        let h = p
            .spawn(Priority::Visible, CancelToken::new(), move |_| {
                let _g = g;
                r.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        assert!(matches!(h.status(), TaskStatus::Queued));
        h.cancel();
        assert!(h.status().is_cancelled());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(p.queue_len(), 0);
        block_tx.send(()).unwrap();
        assert!(blocker.wait(Duration::from_secs(10)).is_done());
        assert_eq!(ran.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn token_cancel_is_seen_when_popped() {
        let p = pool(1);
        let (block_tx, block_rx) = mpsc::channel::<()>();
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let _blocker = p
            .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                let _ = block_rx.recv_timeout(Duration::from_secs(10));
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let token = CancelToken::new();
        let drops = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Guard(drops.clone());
                p.spawn(Priority::Visible, token.clone(), move |_| {
                    let _g = g;
                    panic!("must not run");
                })
                .unwrap()
            })
            .collect();
        token.cancel();
        block_tx.send(()).unwrap();
        for h in &handles {
            assert!(h.wait(Duration::from_secs(10)).is_cancelled());
        }
        assert_eq!(drops.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn panic_fails_only_the_task() {
        let p = pool(1);
        let h = p
            .spawn(Priority::Visible, CancelToken::new(), |_| panic!("boom"))
            .unwrap();
        assert!(matches!(
            h.wait(Duration::from_secs(10)),
            TaskStatus::Failed(AssetError::Panicked { stage: "task", .. })
        ));
        let h2 = p
            .spawn(Priority::Visible, CancelToken::new(), |_| {})
            .unwrap();
        assert!(h2.wait(Duration::from_secs(10)).is_done());
    }

    #[test]
    fn full_queue_drops_closure_and_errors() {
        let p = WorkerPool::new(PoolConfig {
            workers: 1,
            max_queued: 1,
            ..PoolConfig::default()
        });
        let (block_tx, block_rx) = mpsc::channel::<()>();
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let _blocker = p
            .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                let _ = block_rx.recv_timeout(Duration::from_secs(10));
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let _queued = p
            .spawn(Priority::Visible, CancelToken::new(), |_| {})
            .unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let g = Guard(drops.clone());
        let r = p.spawn(Priority::Visible, CancelToken::new(), move |_| {
            let _g = g;
        });
        assert!(matches!(r, Err(AssetError::QueueFull { capacity: 1 })));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        block_tx.send(()).unwrap();
    }

    #[test]
    fn shutdown_fails_queued_tasks_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let handle;
        {
            let p = pool(1);
            let (block_tx, block_rx) = mpsc::channel::<()>();
            let (started_tx, started_rx) = mpsc::channel::<()>();
            let _blocker = p
                .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                    started_tx.send(()).unwrap();
                    let _ = block_rx.recv_timeout(Duration::from_millis(200));
                })
                .unwrap();
            started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            let g = Guard(drops.clone());
            handle = p
                .spawn(Priority::Prefetch, CancelToken::new(), move |_| {
                    let _g = g;
                })
                .unwrap();
            drop(block_tx);
        }
        // Either the worker ran it before shutdown or the shutdown failed it; dropped once.
        let st = handle.wait(Duration::from_secs(10));
        assert!(
            matches!(
                st,
                TaskStatus::Done | TaskStatus::Failed(AssetError::Shutdown)
            ),
            "{st:?}"
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn priority_orders_tasks() {
        let p = pool(1);
        let (block_tx, block_rx) = mpsc::channel::<()>();
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let _blocker = p
            .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                let _ = block_rx.recv_timeout(Duration::from_secs(10));
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let (tx, rx) = mpsc::channel();
        let mut hs = Vec::new();
        for (i, pr) in [Priority::Prefetch, Priority::Visible, Priority::Immediate]
            .into_iter()
            .enumerate()
        {
            let tx = tx.clone();
            hs.push(
                p.spawn(pr, CancelToken::new(), move |_| tx.send(i).unwrap())
                    .unwrap(),
            );
        }
        // Raise the first one above the others.
        hs[0].set_priority(Priority::Immediate);
        block_tx.send(()).unwrap();
        for h in &hs {
            assert!(h.wait(Duration::from_secs(10)).is_done());
        }
        let order: Vec<usize> = rx.try_iter().collect();
        // Task 2 was queued at Immediate before task 0 was raised: FIFO within a level.
        assert_eq!(order, vec![2, 0, 1]);
    }
}
