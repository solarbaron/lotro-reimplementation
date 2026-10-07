// SPDX-License-Identifier: GPL-3.0-or-later
//! A small pool of worker threads for the walker's CPU work that is not a record decode
//! (terrain blocks and meshes): bounded submission that never blocks the caller, results
//! polled without blocking, panics contained per job.

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

struct Queue<J> {
    jobs: VecDeque<J>,
    closed: bool,
}

struct Shared<J> {
    queue: Mutex<Queue<J>>,
    ready: Condvar,
}

/// Worker threads running `F(job) -> output` (module docs).
pub struct JobRunner<J, O> {
    shared: Arc<Shared<J>>,
    results: Receiver<O>,
    threads: Vec<JoinHandle<()>>,
    capacity: usize,
    panics: Arc<Mutex<u64>>,
}

impl<J, O> std::fmt::Debug for JobRunner<J, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRunner")
            .field("threads", &self.threads.len())
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl<J: Send + 'static, O: Send + 'static> JobRunner<J, O> {
    /// `threads` workers (at least 1) named `name-N`, at most `capacity` queued jobs.
    ///
    /// # Errors
    ///
    /// When a thread cannot be spawned.
    pub fn new<F>(name: &str, threads: usize, capacity: usize, work: F) -> std::io::Result<Self>
    where
        F: Fn(J) -> O + Send + Sync + 'static,
    {
        Self::with_spawner(name, threads, capacity, work, |builder, f| builder.spawn(f))
    }

    /// [`JobRunner::new`] with the thread spawn supplied (tests inject failures). On a
    /// spawn error the workers already started are stopped and joined before returning.
    fn with_spawner<F, S>(
        name: &str,
        threads: usize,
        capacity: usize,
        work: F,
        mut spawn: S,
    ) -> std::io::Result<Self>
    where
        F: Fn(J) -> O + Send + Sync + 'static,
        S: FnMut(
            std::thread::Builder,
            Box<dyn FnOnce() + Send + 'static>,
        ) -> std::io::Result<JoinHandle<()>>,
    {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                jobs: VecDeque::new(),
                closed: false,
            }),
            ready: Condvar::new(),
        });
        let (tx, results) = mpsc::channel::<O>();
        let work = Arc::new(work);
        let panics = Arc::new(Mutex::new(0u64));
        // Built first, so a failed spawn below drops it and `Drop` stops and joins the
        // workers already running.
        let mut runner = Self {
            shared,
            results,
            threads: Vec::new(),
            capacity: capacity.max(1),
            panics,
        };
        for i in 0..threads.max(1) {
            let shared = Arc::clone(&runner.shared);
            let tx: Sender<O> = tx.clone();
            let work = Arc::clone(&work);
            let panics = Arc::clone(&runner.panics);
            let builder = std::thread::Builder::new().name(format!("{name}-{i}"));
            let handle = spawn(
                builder,
                Box::new(move || worker(&shared, &tx, &*work, &panics)),
            )?;
            runner.threads.push(handle);
        }
        Ok(runner)
    }

    /// Queues `job`; gives it back when the queue is full or closed. Never blocks for
    /// longer than the queue lock.
    ///
    /// # Errors
    ///
    /// The job, when it was not queued.
    pub fn submit(&self, job: J) -> Result<(), J> {
        let mut q = self
            .shared
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if q.closed || q.jobs.len() >= self.capacity {
            return Err(job);
        }
        q.jobs.push_back(job);
        drop(q);
        self.shared.ready.notify_one();
        Ok(())
    }

    /// Reorders the queued jobs by `rank`, lowest first (ties keep their order), and drops
    /// the jobs it ranks `None`. Returns how many were dropped.
    pub fn reorder(&self, mut rank: impl FnMut(&J) -> Option<f32>) -> usize {
        let mut q = self
            .shared
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if q.jobs.is_empty() {
            return 0;
        }
        let before = q.jobs.len();
        let mut ranked: Vec<(f32, J)> = q
            .jobs
            .drain(..)
            .filter_map(|j| rank(&j).map(|r| (r, j)))
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        q.jobs.extend(ranked.into_iter().map(|(_, j)| j));
        before - q.jobs.len()
    }

    /// The next finished result, without blocking.
    pub fn try_next(&self) -> Option<O> {
        self.results.try_recv().ok()
    }

    /// Jobs that panicked so far.
    #[must_use]
    pub fn panics(&self) -> u64 {
        *self.panics.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Drops queued jobs, stops the workers after their current job and joins them.
    pub fn shutdown(&mut self) {
        {
            let mut q = self
                .shared
                .queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            q.closed = true;
            q.jobs.clear();
        }
        self.shared.ready.notify_all();
        for h in self.threads.drain(..) {
            if h.join().is_err() {
                tracing::error!("walker job thread panicked outside a job");
            }
        }
    }
}

impl<J, O> Drop for JobRunner<J, O> {
    fn drop(&mut self) {
        {
            let mut q = self
                .shared
                .queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            q.closed = true;
            q.jobs.clear();
        }
        self.shared.ready.notify_all();
        for h in self.threads.drain(..) {
            let _ = h.join();
        }
    }
}

fn worker<J, O>(
    shared: &Shared<J>,
    tx: &Sender<O>,
    work: &(dyn Fn(J) -> O + Send + Sync),
    panics: &Mutex<u64>,
) {
    loop {
        let job = {
            let mut q = shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if q.closed {
                    return;
                }
                if let Some(j) = q.jobs.pop_front() {
                    break j;
                }
                q = shared.ready.wait(q).unwrap_or_else(PoisonError::into_inner);
            }
        };
        match std::panic::catch_unwind(AssertUnwindSafe(|| work(job))) {
            Ok(out) => {
                if tx.send(out).is_err() {
                    return;
                }
            }
            Err(_) => {
                *panics.lock().unwrap_or_else(PoisonError::into_inner) += 1;
                tracing::error!("walker job panicked; result dropped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn reorder_sorts_and_drops_queued_jobs() {
        // One worker held on a gate, so the rest stay queued.
        let gate = Arc::new(Mutex::new(()));
        let held = gate.lock().unwrap();
        let g = Arc::clone(&gate);
        let mut r = JobRunner::new("test", 1, 16, move |x: u32| {
            drop(g.lock().unwrap_or_else(PoisonError::into_inner));
            x
        })
        .unwrap();
        r.submit(0).unwrap();
        let t = Instant::now();
        while r.shared.queue.lock().unwrap().jobs.len() == 1
            && t.elapsed() < Duration::from_secs(10)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        for x in [5u32, 3, 9, 1, 7] {
            r.submit(x).unwrap();
        }
        let dropped = r.reorder(|&x| (x != 9).then_some(x as f32));
        assert_eq!(dropped, 1);
        drop(held);
        let mut got = Vec::new();
        let t = Instant::now();
        while got.len() < 5 && t.elapsed() < Duration::from_secs(10) {
            match r.try_next() {
                Some(x) => got.push(x),
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        assert_eq!(got, vec![0, 1, 3, 5, 7]);
        r.shutdown();
    }

    #[test]
    fn runs_jobs_bounds_the_queue_and_contains_panics() {
        let mut r = JobRunner::new("test", 2, 4, |x: u32| {
            assert!(x != 13, "unlucky");
            x * 2
        })
        .unwrap();
        let mut sent = 0;
        let mut rejected = 0;
        for i in 0..100u32 {
            match r.submit(i + 100) {
                Ok(()) => sent += 1,
                Err(_) => rejected += 1,
            }
        }
        assert!(sent >= 4 && sent + rejected == 100);
        let _ = r.submit(13);
        let mut got = 0;
        let t = Instant::now();
        while got < sent && t.elapsed() < Duration::from_secs(10) {
            if r.try_next().is_some() {
                got += 1;
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        assert_eq!(got, sent);
        assert!(r.submit(13).is_ok());
        let t = Instant::now();
        while r.panics() == 0 && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(r.panics() >= 1);
        r.shutdown();
        assert!(r.submit(1).is_err());
    }

    #[test]
    fn failed_spawn_stops_the_started_workers() {
        let token = Arc::new(());
        let held = Arc::clone(&token);
        let mut n = 0;
        let r = JobRunner::with_spawner(
            "test",
            3,
            4,
            move |x: u32| {
                let _ = &held;
                x
            },
            |builder, f| {
                n += 1;
                if n == 2 {
                    Err(std::io::Error::other("no more threads"))
                } else {
                    builder.spawn(f)
                }
            },
        );
        assert!(r.is_err());
        // The worker started first was joined: nothing holds the work closure any more.
        assert_eq!(Arc::strong_count(&token), 1);
    }
}
