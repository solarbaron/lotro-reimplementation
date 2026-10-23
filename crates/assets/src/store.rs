// SPDX-License-Identifier: GPL-3.0-or-later
//! The typed asset store: requests, de-duplication, cache, completions.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use crate::cache::LruCache;
use crate::decoder::{Decoder, JobContext};
use crate::pool::{Job, PoolShared, SchedCell, WorkerPool};
use crate::source::RecordSource;
use crate::{AssetError, AssetKey, CancelToken, Priority, lock};

// ---------------------------------------------------------------------------------------
// Public value types
// ---------------------------------------------------------------------------------------

/// Where a request stands.
pub enum AssetState<T> {
    /// Waiting in the queue.
    Queued,
    /// A worker is reading or decoding it.
    Loading,
    /// Decoded.
    Ready(Arc<T>),
    /// Could not be produced. Show a placeholder.
    Failed(AssetError),
    /// Every request for it was cancelled before it finished.
    Cancelled,
}

impl<T> AssetState<T> {
    /// Whether the state is final (`Ready`, `Failed` or `Cancelled`).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Ready(_) | Self::Failed(_) | Self::Cancelled)
    }

    /// The product, if ready.
    #[must_use]
    pub fn ready(&self) -> Option<&Arc<T>> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// A short name of the state for logs and UIs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Loading => "loading",
            Self::Ready(_) => "ready",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl<T> Clone for AssetState<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Queued => Self::Queued,
            Self::Loading => Self::Loading,
            Self::Ready(v) => Self::Ready(Arc::clone(v)),
            Self::Failed(e) => Self::Failed(e.clone()),
            Self::Cancelled => Self::Cancelled,
        }
    }
}

impl<T> fmt::Debug for AssetState<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(e) => f.debug_tuple("Failed").field(e).finish(),
            other => f.write_str(match other {
                Self::Queued => "Queued",
                Self::Loading => "Loading",
                Self::Ready(_) => "Ready(..)",
                _ => "Cancelled",
            }),
        }
    }
}

/// How a job ended.
pub enum Outcome<T> {
    /// Decoded (and cached, if it fit the budget).
    Ready(Arc<T>),
    /// Failed.
    Failed(AssetError),
    /// Abandoned because every request for it was cancelled.
    Cancelled,
}

impl<T> fmt::Debug for Outcome<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(_) => f.write_str("Ready(..)"),
            Self::Failed(e) => f.debug_tuple("Failed").field(e).finish(),
            Self::Cancelled => f.write_str("Cancelled"),
        }
    }
}

/// One finished job, as posted to the store's completion queue.
///
/// Exactly one completion is posted per job, however many requests shared it. Requests
/// that never get a job post none; their handle is already final when
/// [`AssetStore::request`] returns. That is the case for requests answered from the
/// cache (or a weak reference), from the failure memo, made with an already-cancelled
/// token, or refused at submission ([`AssetError::QueueFull`], [`AssetError::Shutdown`]).
/// A frame loop should therefore check [`Handle::is_finished`] right after requesting
/// rather than count completions against requests.
///
/// A completion holds its product, so products stay alive (and evicted ones stay
/// revivable) until the completion is polled. [`StoreConfig::max_completions`] and
/// [`StoreConfig::max_completion_bytes`] bound this; the oldest completions are dropped
/// beyond either.
pub struct Completion<T> {
    /// What was requested.
    pub key: AssetKey,
    /// How it ended.
    pub outcome: Outcome<T>,
    /// The highest priority among the requests still interested when it ended.
    pub priority: Option<Priority>,
    /// Time spent reading the record.
    pub read_time: Duration,
    /// Time spent decoding.
    pub decode_time: Duration,
}

impl<T> fmt::Debug for Completion<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Completion")
            .field("key", &self.key)
            .field("outcome", &self.outcome)
            .field("priority", &self.priority)
            .field("read_time", &self.read_time)
            .field("decode_time", &self.decode_time)
            .finish()
    }
}

/// Configuration of an [`AssetStore`].
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Byte budget of the decoded-asset cache (by [`Decoder::size_of`]). Products larger
    /// than the budget are delivered but not cached.
    pub cache_budget: usize,
    /// Maximum completions waiting to be polled; the oldest are dropped beyond it (the
    /// handles still report the result).
    pub max_completions: usize,
    /// Maximum bytes (by [`Decoder::size_of`]) of the products held by completions waiting
    /// to be polled; the oldest are dropped beyond it, except that the newest completion
    /// is always kept. A product whose size could not be computed counts as `usize::MAX`.
    pub max_completion_bytes: usize,
    /// Maximum remembered data failures; 0 disables the memo. When full it is cleared.
    pub max_failures: usize,
    /// Maximum requests tracked per job. Further requests still share the job but cannot
    /// cancel it.
    pub max_tickets_per_job: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            cache_budget: 256 << 20,
            max_completions: 65_536,
            max_completion_bytes: 256 << 20,
            max_failures: 4_096,
            max_tickets_per_job: 1_024,
        }
    }
}

/// Counters of an [`AssetStore`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StoreStats {
    /// Calls to `request` / `request_with`.
    pub requests: u64,
    /// Requests answered from the cache (including products revived from a weak
    /// reference after eviction).
    pub hits: u64,
    /// Requests that started a new job.
    pub misses: u64,
    /// Requests that joined a job already in flight (de-duplicated).
    pub joins: u64,
    /// Requests answered from the failure memo.
    pub failure_hits: u64,
    /// Requests refused at submission (queue full or shut down).
    pub rejected: u64,
    /// Jobs that produced a product.
    pub completed: u64,
    /// Jobs that failed.
    pub failed: u64,
    /// Jobs abandoned after cancellation.
    pub cancelled: u64,
    /// Cache entries evicted.
    pub evictions: u64,
    /// Products too large for the cache budget.
    pub uncacheable: u64,
    /// Completions dropped because nobody polled them.
    pub dropped_completions: u64,
    /// Products decoded.
    pub decoded: u64,
    /// Total time spent reading records.
    pub read_time: Duration,
    /// Total time spent decoding.
    pub decode_time: Duration,
    /// Entries in the cache.
    pub cache_entries: usize,
    /// Bytes charged to the cache.
    pub cache_bytes: usize,
    /// The cache budget.
    pub cache_budget: usize,
    /// Jobs queued or running.
    pub in_flight: usize,
    /// Completions waiting to be polled.
    pub pending_completions: usize,
}

impl StoreStats {
    /// Fraction of lookups answered from the cache: `hits / (hits + misses + joins +
    /// failure_hits)`, 0 when there were none.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let lookups = self.hits + self.misses + self.joins + self.failure_hits;
        if lookups == 0 {
            0.0
        } else {
            self.hits as f64 / lookups as f64
        }
    }

    /// Mean decode time per product.
    #[must_use]
    pub fn mean_decode_time(&self) -> Duration {
        match u32::try_from(self.decoded) {
            Ok(0) => Duration::ZERO,
            Ok(n) => self.decode_time / n,
            Err(_) => Duration::from_secs_f64(self.decode_time.as_secs_f64() / self.decoded as f64),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Slots and handles
// ---------------------------------------------------------------------------------------

#[derive(Debug)]
struct Ticket {
    id: u64,
    token: CancelToken,
    priority: Priority,
}

struct SlotInner<T> {
    state: AssetState<T>,
    tickets: Vec<Ticket>,
    /// Interest that cannot be cancelled (requests beyond `max_tickets_per_job`).
    pinned: Option<Priority>,
}

impl<T> SlotInner<T> {
    /// Highest priority among live (not cancelled) interest, `None` if there is none.
    fn effective(&self) -> Option<Priority> {
        self.tickets
            .iter()
            .filter(|t| !t.token.is_cancelled())
            .map(|t| t.priority)
            .chain(self.pinned)
            .max()
    }

    fn prune(&mut self) {
        self.tickets.retain(|t| !t.token.is_cancelled());
    }
}

/// The shared state behind one request (or one de-duplicated group of requests).
struct Slot<T> {
    key: AssetKey,
    inner: Mutex<SlotInner<T>>,
    cond: Condvar,
}

impl<T> Slot<T> {
    fn new(key: AssetKey, state: AssetState<T>, tickets: Vec<Ticket>) -> Self {
        Self {
            key,
            inner: Mutex::new(SlotInner {
                state,
                tickets,
                pinned: None,
            }),
            cond: Condvar::new(),
        }
    }

    /// Sets a final state unless one is already set. Returns whether it was set.
    fn settle(&self, state: AssetState<T>) -> (bool, Option<Priority>) {
        let mut inner = lock(&self.inner);
        if inner.state.is_finished() {
            return (false, None);
        }
        let prio = inner.effective();
        inner.state = state;
        inner.tickets.clear();
        inner.pinned = None;
        drop(inner);
        self.cond.notify_all();
        (true, prio)
    }
}

/// Lets a handle re-prioritise its job without knowing the store's type parameters.
trait Reschedule: Send + Sync {
    fn reschedule(&self);
}

/// The answer to a request: a cheap reference to an asset that may still be loading.
///
/// Clones share the same request (and its cancel token). Dropping a handle does not
/// cancel the request; call [`Handle::cancel`].
pub struct Handle<T> {
    slot: Arc<Slot<T>>,
    job: Option<Weak<dyn Reschedule>>,
    ticket: Option<u64>,
    token: CancelToken,
    placeholder: Option<Arc<T>>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            slot: Arc::clone(&self.slot),
            job: self.job.clone(),
            ticket: self.ticket,
            token: self.token.clone(),
            placeholder: self.placeholder.clone(),
        }
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("key", &self.slot.key)
            .field("state", &lock(&self.slot.inner).state.name())
            .field("cancelled", &self.token.is_cancelled())
            .finish()
    }
}

impl<T> Handle<T> {
    fn finished(key: AssetKey, state: AssetState<T>, token: CancelToken) -> Self {
        Self {
            slot: Arc::new(Slot::new(key, state, Vec::new())),
            job: None,
            ticket: None,
            token,
            placeholder: None,
        }
    }

    /// What was requested.
    #[must_use]
    pub fn key(&self) -> AssetKey {
        self.slot.key
    }

    /// The current state. Never blocks for longer than a short lock.
    #[must_use]
    pub fn state(&self) -> AssetState<T> {
        lock(&self.slot.inner).state.clone()
    }

    /// The product, if ready.
    #[must_use]
    pub fn get(&self) -> Option<Arc<T>> {
        match &lock(&self.slot.inner).state {
            AssetState::Ready(v) => Some(Arc::clone(v)),
            _ => None,
        }
    }

    /// The product if ready, otherwise the store's placeholder (see
    /// [`AssetStore::set_placeholder`]) as it was when the request was made.
    #[must_use]
    pub fn get_or_placeholder(&self) -> Option<Arc<T>> {
        self.get().or_else(|| self.placeholder.clone())
    }

    /// Whether the state is final.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        lock(&self.slot.inner).state.is_finished()
    }

    /// The request's cancel token.
    #[must_use]
    pub fn token(&self) -> &CancelToken {
        &self.token
    }

    /// Withdraws this request (cancels its token). The job is abandoned once no live
    /// request is left; if other requests share it, it continues at their priority.
    pub fn cancel(&self) {
        self.token.cancel();
        self.reschedule();
    }

    /// Changes this request's priority. The job runs at the highest priority among its
    /// live requests. No effect once the job has started.
    pub fn set_priority(&self, priority: Priority) {
        let Some(id) = self.ticket else { return };
        {
            let mut inner = lock(&self.slot.inner);
            if let Some(t) = inner.tickets.iter_mut().find(|t| t.id == id) {
                t.priority = priority;
            }
        }
        self.reschedule();
    }

    fn reschedule(&self) {
        if let Some(job) = self.job.as_ref().and_then(Weak::upgrade) {
            job.reschedule();
        }
    }

    /// Blocks until the state is final or `timeout` passes, and returns the state.
    ///
    /// For tools, tests and loading screens; a frame loop should poll
    /// [`Handle::state`] or [`AssetStore::poll_completions`] instead.
    ///
    /// A timeout too large to represent as a deadline (for example [`Duration::MAX`])
    /// waits without one.
    pub fn wait(&self, timeout: Duration) -> AssetState<T> {
        let deadline = Instant::now().checked_add(timeout);
        let mut inner = lock(&self.slot.inner);
        while !inner.state.is_finished() {
            inner = match deadline {
                None => self
                    .slot
                    .cond
                    .wait(inner)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    self.slot
                        .cond
                        .wait_timeout(inner, deadline - now)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0
                }
            };
        }
        inner.state.clone()
    }
}

// ---------------------------------------------------------------------------------------
// Store internals
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Counters {
    requests: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    joins: AtomicU64,
    failure_hits: AtomicU64,
    rejected: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    cancelled: AtomicU64,
    evictions: AtomicU64,
    uncacheable: AtomicU64,
    dropped_completions: AtomicU64,
    decoded: AtomicU64,
    read_ns: AtomicU64,
    decode_ns: AtomicU64,
    next_ticket: AtomicU64,
}

fn bump(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

fn nanos(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// A product and the size it was charged at (computed once, on the worker).
type Sized<T> = (Arc<T>, usize);

struct StoreState<S, D: Decoder> {
    cache: LruCache<AssetKey, Sized<D::Output>>,
    /// Evicted (or uncacheable) products that may still be alive elsewhere, with their
    /// size; revived without decoding and without calling [`Decoder::size_of`] again.
    weak: HashMap<AssetKey, (Weak<D::Output>, usize)>,
    weak_prune_at: usize,
    inflight: HashMap<AssetKey, Arc<JobImpl<S, D>>>,
    failures: HashMap<AssetKey, AssetError>,
}

struct StoreShared<S, D: Decoder> {
    source: S,
    decoder: D,
    pool: Arc<PoolShared>,
    config: StoreConfig,
    state: Mutex<StoreState<S, D>>,
    completions: Mutex<CompletionQueue<D::Output>>,
    placeholder: RwLock<Option<Arc<D::Output>>>,
    counters: Counters,
}

/// Pending completions with the byte size of each one's product.
struct CompletionQueue<T> {
    queue: VecDeque<(Completion<T>, usize)>,
    bytes: usize,
}

impl<T> CompletionQueue<T> {
    fn pop_front(&mut self) -> Option<Completion<T>> {
        let (c, size) = self.queue.pop_front()?;
        self.bytes = self.bytes.saturating_sub(size);
        Some(c)
    }
}

impl<S, D: Decoder> Drop for StoreShared<S, D> {
    /// Abandons the jobs still in flight: queued ones leave the pool queue at once (freeing
    /// their capacity) and every handle ends as [`AssetError::Shutdown`]. No job of this
    /// store can be running here, since a running job holds the store alive.
    fn drop(&mut self) {
        let st = self
            .state
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let jobs: Vec<_> = st.inflight.drain().map(|(_, j)| j).collect();
        if !jobs.is_empty() {
            tracing::debug!(
                jobs = jobs.len(),
                decoder = D::NAME,
                "asset store dropped with jobs in flight"
            );
        }
        for job in jobs {
            self.pool.withdraw(&job.sched);
            job.slot.settle(AssetState::Failed(AssetError::Shutdown));
        }
    }
}

struct JobImpl<S, D: Decoder> {
    sched: SchedCell,
    slot: Arc<Slot<D::Output>>,
    store: Weak<StoreShared<S, D>>,
    me: Weak<JobImpl<S, D>>,
}

/// A decoded product and its size; `None` if [`Decoder::size_of`] panicked.
type Product<T> = (Arc<T>, Option<usize>);

impl<S: RecordSource + 'static, D: Decoder> StoreShared<S, D> {
    /// Posts a completion whose product (if any) is `size` bytes, dropping the oldest ones
    /// beyond the count and byte bounds. Dropped products are released outside the lock.
    fn push_completion(&self, c: Completion<D::Output>, size: usize) {
        let max_count = self.config.max_completions.max(1);
        let max_bytes = self.config.max_completion_bytes;
        let mut dropped = Vec::new();
        {
            let mut q = lock(&self.completions);
            q.bytes = q.bytes.saturating_add(size);
            q.queue.push_back((c, size));
            while q.queue.len() > max_count || (q.bytes > max_bytes && q.queue.len() > 1) {
                match q.pop_front() {
                    Some(old) => dropped.push(old),
                    None => break,
                }
            }
        }
        if !dropped.is_empty() {
            self.counters
                .dropped_completions
                .fetch_add(dropped.len() as u64, Ordering::Relaxed);
        }
    }

    /// Settles a job as cancelled and posts its completion (if it was not final yet).
    fn settle_cancelled(&self, job: &JobImpl<S, D>, read_time: Duration) {
        let (settled, _) = job.slot.settle(AssetState::Cancelled);
        if settled {
            bump(&self.counters.cancelled);
            tracing::trace!(key = %job.slot.key, decoder = D::NAME, "asset job cancelled");
            self.push_completion(
                Completion {
                    key: job.slot.key,
                    outcome: Outcome::Cancelled,
                    priority: None,
                    read_time,
                    decode_time: Duration::ZERO,
                },
                0,
            );
        }
    }

    /// Takes a queued job whose requests were all cancelled out of the pool queue and
    /// settles it as cancelled at once, so it stops holding queue capacity. Does nothing if
    /// the job is not queued (a worker took it; its next checkpoint settles it) or someone
    /// still wants it.
    fn withdraw_abandoned(&self, job: &JobImpl<S, D>) {
        let mut st = lock(&self.state);
        {
            let mut inner = lock(&job.slot.inner);
            if !matches!(inner.state, AssetState::Queued) {
                return;
            }
            inner.prune();
            if inner.effective().is_some() {
                return;
            }
            // Store lock → slot lock → queue lock is the allowed order.
            if !self.pool.withdraw(&job.sched) {
                return;
            }
        }
        Self::remove_inflight(&mut st, job);
        drop(st);
        self.settle_cancelled(job, Duration::ZERO);
    }

    /// Re-checks interest. If none is left, abandons the job (removes it from the in-flight
    /// table, settles it as cancelled, posts a completion) and returns false.
    fn checkpoint(&self, job: &JobImpl<S, D>, read_time: Duration) -> bool {
        let mut st = lock(&self.state);
        {
            let mut inner = lock(&job.slot.inner);
            inner.prune();
            if inner.effective().is_some() {
                return true;
            }
        }
        Self::remove_inflight(&mut st, job);
        drop(st);
        self.settle_cancelled(job, read_time);
        false
    }

    fn remove_inflight(st: &mut StoreState<S, D>, job: &JobImpl<S, D>) {
        if st
            .inflight
            .get(&job.slot.key)
            .is_some_and(|j| std::ptr::eq(Arc::as_ptr(j), job))
        {
            st.inflight.remove(&job.slot.key);
        }
    }

    fn finish(
        &self,
        job: &JobImpl<S, D>,
        result: Result<Product<D::Output>, AssetError>,
        read_time: Duration,
        decode_time: Duration,
    ) {
        let key = job.slot.key;
        let mut evicted: Vec<(AssetKey, Sized<D::Output>)> = Vec::new();
        let mut completion_size = 0usize;
        let mut st = lock(&self.state);
        Self::remove_inflight(&mut st, job);
        let (state, outcome) = match result {
            Ok((value, size)) => {
                completion_size = size.unwrap_or(usize::MAX);
                // Drop any stale weak entry first; an uncacheable product re-adds one below.
                st.weak.remove(&key);
                let cached = match size {
                    Some(size) => match st.cache.insert(key, (Arc::clone(&value), size), size) {
                        Ok(ev) => {
                            evicted = ev;
                            true
                        }
                        Err(_) => false,
                    },
                    // `size_of` panicked: the size is unknown, so never charge it.
                    None => false,
                };
                if !cached {
                    bump(&self.counters.uncacheable);
                    if let Some(size) = size {
                        // Still revivable while someone holds it.
                        st.weak.insert(key, (Arc::downgrade(&value), size));
                    }
                }
                for (k, (v, sz)) in &evicted {
                    if *k != key {
                        st.weak.insert(*k, (Arc::downgrade(v), *sz));
                    }
                }
                if st.weak.len() > st.weak_prune_at {
                    st.weak.retain(|_, (w, _)| w.strong_count() > 0);
                    st.weak_prune_at = 2 * st.weak.len() + 1024;
                }
                bump(&self.counters.completed);
                (AssetState::Ready(Arc::clone(&value)), Outcome::Ready(value))
            }
            Err(e) => {
                bump(&self.counters.failed);
                if e.is_data_error() && self.config.max_failures > 0 {
                    if st.failures.len() >= self.config.max_failures {
                        st.failures.clear();
                    }
                    st.failures.insert(key, e.clone());
                }
                tracing::debug!(%key, decoder = D::NAME, error = %e, "asset failed");
                (AssetState::Failed(e.clone()), Outcome::Failed(e))
            }
        };
        let (settled, priority) = job.slot.settle(state);
        drop(st);
        let n_evicted = evicted.iter().filter(|(k, _)| *k != key).count();
        if n_evicted > 0 {
            self.counters
                .evictions
                .fetch_add(n_evicted as u64, Ordering::Relaxed);
            tracing::trace!(
                evicted = n_evicted,
                decoder = D::NAME,
                "asset cache eviction"
            );
        }
        // Evicted products are released here, outside the lock.
        drop(evicted);
        if settled {
            self.push_completion(
                Completion {
                    key,
                    outcome,
                    priority,
                    read_time,
                    decode_time,
                },
                completion_size,
            );
        }
    }
}

impl<S: RecordSource + 'static, D: Decoder> JobImpl<S, D> {
    fn run_inner(&self, priority: Priority) {
        let Some(store) = self.store.upgrade() else {
            self.slot.settle(AssetState::Failed(AssetError::Shutdown));
            return;
        };
        let key = self.slot.key;
        let _span = tracing::debug_span!("asset_job", decoder = D::NAME, %key).entered();
        if !store.checkpoint(self, Duration::ZERO) {
            return;
        }
        {
            let mut inner = lock(&self.slot.inner);
            if matches!(inner.state, AssetState::Queued) {
                inner.state = AssetState::Loading;
            }
        }

        let t0 = Instant::now();
        let read = catch_unwind(AssertUnwindSafe(|| store.source.read_record(key.did))).unwrap_or(
            Err(AssetError::Panicked {
                did: key.did,
                stage: "read",
            }),
        );
        let read_time = t0.elapsed();
        store
            .counters
            .read_ns
            .fetch_add(nanos(read_time), Ordering::Relaxed);
        let bytes = match read {
            Ok(b) => b,
            Err(e) => return store.finish(self, Err(e), read_time, Duration::ZERO),
        };
        if !store.checkpoint(self, read_time) {
            return;
        }

        let len = bytes.len();
        let check = || lock(&self.slot.inner).effective().is_none();
        let current = || lock(&self.slot.inner).effective();
        let ctx = JobContext::new(&check, priority).with_current(&current);
        let t1 = Instant::now();
        let decoded = catch_unwind(AssertUnwindSafe(|| store.decoder.decode(key, bytes, &ctx)));
        let decode_time = t1.elapsed();
        store
            .counters
            .decode_ns
            .fetch_add(nanos(decode_time), Ordering::Relaxed);
        tracing::trace!(
            read_us = read_time.as_micros() as u64,
            decode_us = decode_time.as_micros() as u64,
            bytes = len,
            "asset decoded"
        );
        match decoded {
            Ok(Ok(value)) => {
                bump(&store.counters.decoded);
                let size = catch_unwind(AssertUnwindSafe(|| store.decoder.size_of(&value))).ok();
                store.finish(self, Ok((Arc::new(value), size)), read_time, decode_time);
            }
            Ok(Err(e)) if ctx.observed_cancel() => {
                // The decoder gave up because of cancellation. Settle as cancelled, or,
                // if a request joined meanwhile, run again.
                if !store.checkpoint(self, read_time) {
                    return;
                }
                tracing::trace!(error = %e, "asset job re-queued after aborted decode");
                self.requeue(&store, read_time);
            }
            Ok(Err(e)) => store.finish(
                self,
                Err(AssetError::Decode {
                    did: key.did,
                    decoder: D::NAME,
                    source: Arc::new(e),
                }),
                read_time,
                decode_time,
            ),
            Err(_) => store.finish(
                self,
                Err(AssetError::Panicked {
                    did: key.did,
                    stage: "decode",
                }),
                read_time,
                decode_time,
            ),
        }
    }

    fn requeue(&self, store: &StoreShared<S, D>, read_time: Duration) {
        let prio = {
            let mut inner = lock(&self.slot.inner);
            inner.state = AssetState::Queued;
            inner.effective().unwrap_or_default()
        };
        let Some(me) = self.me.upgrade() else { return };
        // No sweep for abandoned jobs here: this may itself run while reaping one.
        if let Err(e) = store.pool.submit(me, prio, None) {
            store.finish(self, Err(e), read_time, Duration::ZERO);
        }
    }
}

impl<S: RecordSource + 'static, D: Decoder> Job for JobImpl<S, D> {
    fn sched(&self) -> &SchedCell {
        &self.sched
    }

    fn run(&self, priority: Priority) {
        self.run_inner(priority);
    }

    fn abort(&self, err: AssetError) {
        let err = match err {
            AssetError::Panicked { stage, .. } => AssetError::Panicked {
                did: self.slot.key.did,
                stage,
            },
            other => other,
        };
        match self.store.upgrade() {
            Some(store) => {
                if !lock(&self.slot.inner).state.is_finished() {
                    store.finish(self, Err(err), Duration::ZERO, Duration::ZERO);
                }
            }
            None => {
                self.slot.settle(AssetState::Failed(err));
            }
        }
    }

    fn abandoned(&self) -> Option<bool> {
        // `strong_count`, not `upgrade`: this runs under the queue lock and must never drop
        // the last reference to the store (whose `Drop` takes the queue lock).
        if self.store.strong_count() == 0 {
            return Some(true);
        }
        match self.slot.inner.try_lock() {
            Ok(inner) => Some(inner.effective().is_none()),
            Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner().effective().is_none()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }

    fn reap(&self) {
        let Some(store) = self.store.upgrade() else {
            self.slot.settle(AssetState::Failed(AssetError::Shutdown));
            return;
        };
        if store.checkpoint(self, Duration::ZERO) {
            // A request joined after the sweep took the job out of the queue.
            self.requeue(&store, Duration::ZERO);
        }
    }
}

impl<S: RecordSource + 'static, D: Decoder> Reschedule for JobImpl<S, D> {
    fn reschedule(&self) {
        let Some(store) = self.store.upgrade() else {
            return;
        };
        let Some(me) = self.me.upgrade() else { return };
        let abandoned = {
            let inner = lock(&self.slot.inner);
            if !matches!(inner.state, AssetState::Queued) {
                return;
            }
            match inner.effective() {
                Some(p) => {
                    let me: Arc<dyn Job> = me;
                    // Slot lock → queue lock is the allowed order (workers never wait for
                    // a slot lock while holding the queue lock).
                    store.pool.reprioritize(&me, p);
                    false
                }
                None => true,
            }
        };
        if abandoned {
            store.withdraw_abandoned(self);
        }
    }
}

// ---------------------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------------------

/// One decoder over one record source, sharing a [`WorkerPool`].
///
/// Cloning is cheap and shares the store. Dropping the last clone abandons its jobs in
/// flight: queued ones leave the pool queue at once and their handles end as
/// [`AssetError::Shutdown`]. (A job a worker is running keeps the store alive until it
/// finishes; it still completes normally.)
pub struct AssetStore<S, D: Decoder> {
    shared: Arc<StoreShared<S, D>>,
    pool: WorkerPool,
}

impl<S, D: Decoder> Clone for AssetStore<S, D> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            pool: self.pool.clone(),
        }
    }
}

impl<S, D: Decoder> fmt::Debug for AssetStore<S, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetStore")
            .field("decoder", &D::NAME)
            .field("config", &self.shared.config)
            .field("pool", &self.pool)
            .finish_non_exhaustive()
    }
}

impl<S: RecordSource + 'static, D: Decoder> AssetStore<S, D> {
    /// Creates a store whose jobs run on `pool`.
    pub fn new(pool: &WorkerPool, source: S, decoder: D, config: StoreConfig) -> Self {
        let shared = Arc::new(StoreShared {
            source,
            decoder,
            pool: Arc::clone(pool.shared()),
            state: Mutex::new(StoreState {
                cache: LruCache::new(config.cache_budget),
                weak: HashMap::new(),
                weak_prune_at: 1024,
                inflight: HashMap::new(),
                failures: HashMap::new(),
            }),
            completions: Mutex::new(CompletionQueue {
                queue: VecDeque::new(),
                bytes: 0,
            }),
            placeholder: RwLock::new(None),
            counters: Counters::default(),
            config,
        });
        Self {
            shared,
            pool: pool.clone(),
        }
    }

    /// Requests `key` with a fresh cancel token. Never blocks on I/O or decoding.
    pub fn request(&self, key: impl Into<AssetKey>, priority: Priority) -> Handle<D::Output> {
        self.request_with(key, priority, CancelToken::new())
    }

    /// Requests `key`, attaching `token`: cancelling it withdraws this request (and every
    /// other request made with a clone of it).
    ///
    /// The returned handle is already final if the product was cached, if the key failed
    /// recently (failure memo), if `token` was already cancelled, or if the job could not
    /// be queued ([`AssetError::QueueFull`], [`AssetError::Shutdown`]); none of these posts
    /// a [`Completion`]. Otherwise the request joins the job already in flight for `key`,
    /// raising its priority if needed, or starts a new one.
    ///
    /// When the pool queue is full, queued jobs whose requests were all cancelled (also
    /// through a shared token alone) are first taken out of the queue to make room; they
    /// end as cancelled.
    pub fn request_with(
        &self,
        key: impl Into<AssetKey>,
        priority: Priority,
        token: CancelToken,
    ) -> Handle<D::Output> {
        let key = key.into();
        let c = &self.shared.counters;
        bump(&c.requests);
        let placeholder = self
            .shared
            .placeholder
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if token.is_cancelled() {
            let mut h = Handle::finished(key, AssetState::Cancelled, token);
            h.placeholder = placeholder;
            return h;
        }
        let ready = |v: Arc<D::Output>, token: CancelToken| {
            let mut h = Handle::finished(key, AssetState::Ready(v), token);
            h.placeholder = placeholder.clone();
            h
        };

        let mut st = lock(&self.shared.state);
        if let Some((v, _)) = st.cache.get(&key) {
            bump(&c.hits);
            return ready(Arc::clone(v), token);
        }
        let revived = st
            .weak
            .get(&key)
            .and_then(|(w, size)| Some((w.upgrade()?, *size)));
        if let Some((v, size)) = revived {
            // The size was recorded when the product was first charged, so no user code
            // runs here under the lock.
            st.weak.remove(&key);
            let evicted = match st.cache.insert(key, (Arc::clone(&v), size), size) {
                Ok(ev) => ev,
                Err(_) => {
                    st.weak.insert(key, (Arc::downgrade(&v), size));
                    Vec::new()
                }
            };
            for (k, (e, sz)) in &evicted {
                st.weak.insert(*k, (Arc::downgrade(e), *sz));
            }
            drop(st);
            c.evictions
                .fetch_add(evicted.len() as u64, Ordering::Relaxed);
            drop(evicted);
            bump(&c.hits);
            return ready(v, token);
        }
        if let Some(e) = st.failures.get(&key) {
            bump(&c.failure_hits);
            let mut h = Handle::finished(key, AssetState::Failed(e.clone()), token);
            h.placeholder = placeholder;
            return h;
        }

        let id = c.next_ticket.fetch_add(1, Ordering::Relaxed);
        let ticket = Ticket {
            id,
            token: token.clone(),
            priority,
        };
        if let Some(job) = st.inflight.get(&key).cloned() {
            bump(&c.joins);
            // Attach the ticket while still holding the store lock, so the job cannot be
            // abandoned for lack of interest in between.
            let tracked = {
                let mut inner = lock(&job.slot.inner);
                if inner.tickets.len() >= self.shared.config.max_tickets_per_job {
                    inner.prune();
                }
                if inner.tickets.len() < self.shared.config.max_tickets_per_job {
                    inner.tickets.push(ticket);
                    true
                } else {
                    inner.pinned = inner.pinned.max(Some(priority));
                    false
                }
            };
            drop(st);
            job.reschedule();
            let weak: Weak<dyn Reschedule> = {
                let strong: Arc<dyn Reschedule> = job.clone();
                Arc::downgrade(&strong)
            };
            return Handle {
                slot: Arc::clone(&job.slot),
                job: Some(weak),
                ticket: tracked.then_some(id),
                token,
                placeholder,
            };
        }

        bump(&c.misses);
        let slot = Arc::new(Slot::new(key, AssetState::Queued, vec![ticket]));
        let job = Arc::new_cyclic(|me| JobImpl {
            sched: SchedCell::default(),
            slot: Arc::clone(&slot),
            store: Arc::downgrade(&self.shared),
            me: me.clone(),
        });
        let as_job: Arc<dyn Job> = job.clone();
        let mut reaped = Vec::new();
        let submitted = self.shared.pool.submit(as_job, priority, Some(&mut reaped));
        if submitted.is_ok() {
            st.inflight.insert(key, Arc::clone(&job));
        }
        drop(st);
        // Settle the jobs swept to make room (of any store sharing the pool), with no lock
        // held.
        for j in reaped {
            j.reap();
        }
        if let Err(e) = submitted {
            bump(&c.rejected);
            tracing::debug!(%key, error = %e, "asset request rejected");
            slot.settle(AssetState::Failed(e));
            return Handle {
                slot,
                job: None,
                ticket: None,
                token,
                placeholder,
            };
        }
        let weak: Weak<dyn Reschedule> = {
            let strong: Arc<dyn Reschedule> = job;
            Arc::downgrade(&strong)
        };
        Handle {
            slot,
            job: Some(weak),
            ticket: Some(id),
            token,
            placeholder,
        }
    }

    /// The cached product for `key`, if any, marking it recently used. Never starts a job.
    #[must_use]
    pub fn cached(&self, key: impl Into<AssetKey>) -> Option<Arc<D::Output>> {
        lock(&self.shared.state)
            .cache
            .get(&key.into())
            .map(|(v, _)| Arc::clone(v))
    }

    /// Whether `key` is cached.
    #[must_use]
    pub fn contains(&self, key: impl Into<AssetKey>) -> bool {
        lock(&self.shared.state).cache.contains(&key.into())
    }

    /// Whether a job for `key` is queued or running.
    #[must_use]
    pub fn is_in_flight(&self, key: impl Into<AssetKey>) -> bool {
        lock(&self.shared.state).inflight.contains_key(&key.into())
    }

    /// Takes up to `max` finished jobs from the completion queue, oldest first. Never
    /// blocks on work; call it once per frame.
    pub fn poll_completions(&self, max: usize) -> Vec<Completion<D::Output>> {
        let mut q = lock(&self.shared.completions);
        let n = max.min(q.queue.len());
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            match q.pop_front() {
                Some(c) => out.push(c),
                None => break,
            }
        }
        out
    }

    /// Takes the oldest finished job, if any.
    pub fn try_next_completion(&self) -> Option<Completion<D::Output>> {
        lock(&self.shared.completions).pop_front()
    }

    /// Completions waiting to be polled.
    #[must_use]
    pub fn pending_completions(&self) -> usize {
        lock(&self.shared.completions).queue.len()
    }

    /// Sets the placeholder handed out by [`Handle::get_or_placeholder`] for requests made
    /// from now on (for example a checkerboard texture).
    pub fn set_placeholder(&self, placeholder: Option<Arc<D::Output>>) {
        *self
            .shared
            .placeholder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = placeholder;
    }

    /// The current placeholder.
    #[must_use]
    pub fn placeholder(&self) -> Option<Arc<D::Output>> {
        self.shared
            .placeholder
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Forgets everything known about `key`: cached product, weak reference and failure.
    /// A job in flight is not affected.
    pub fn invalidate(&self, key: impl Into<AssetKey>) {
        let key = key.into();
        let mut st = lock(&self.shared.state);
        let old = st.cache.remove(&key);
        st.weak.remove(&key);
        st.failures.remove(&key);
        drop(st);
        drop(old);
    }

    /// Empties the cache, the weak references and the failure memo (for example after the
    /// archives were reopened).
    pub fn clear_cache(&self) {
        let mut st = lock(&self.shared.state);
        let old = st.cache.clear();
        st.weak.clear();
        st.failures.clear();
        drop(st);
        drop(old);
    }

    /// Forgets remembered failures so the next request retries.
    pub fn clear_failures(&self) {
        lock(&self.shared.state).failures.clear();
    }

    /// Changes the cache budget, evicting least recently used products if needed.
    pub fn set_cache_budget(&self, bytes: usize) {
        let mut st = lock(&self.shared.state);
        let evicted = st.cache.set_budget(bytes);
        for (k, (v, sz)) in &evicted {
            st.weak.insert(*k, (Arc::downgrade(v), *sz));
        }
        drop(st);
        self.shared
            .counters
            .evictions
            .fetch_add(evicted.len() as u64, Ordering::Relaxed);
    }

    /// The record source.
    #[must_use]
    pub fn source(&self) -> &S {
        &self.shared.source
    }

    /// The decoder.
    #[must_use]
    pub fn decoder(&self) -> &D {
        &self.shared.decoder
    }

    /// The worker pool.
    #[must_use]
    pub fn pool(&self) -> &WorkerPool {
        &self.pool
    }

    /// Snapshot of the store's counters.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let c = &self.shared.counters;
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let (cache_entries, cache_bytes, cache_budget, in_flight) = {
            let st = lock(&self.shared.state);
            (
                st.cache.len(),
                st.cache.bytes(),
                st.cache.budget(),
                st.inflight.len(),
            )
        };
        StoreStats {
            requests: g(&c.requests),
            hits: g(&c.hits),
            misses: g(&c.misses),
            joins: g(&c.joins),
            failure_hits: g(&c.failure_hits),
            rejected: g(&c.rejected),
            completed: g(&c.completed),
            failed: g(&c.failed),
            cancelled: g(&c.cancelled),
            evictions: g(&c.evictions),
            uncacheable: g(&c.uncacheable),
            dropped_completions: g(&c.dropped_completions),
            decoded: g(&c.decoded),
            read_time: Duration::from_nanos(g(&c.read_ns)),
            decode_time: Duration::from_nanos(g(&c.decode_ns)),
            cache_entries,
            cache_bytes,
            cache_budget,
            in_flight,
            pending_completions: self.pending_completions(),
        }
    }

    /// Emits the store and pool counters as one `debug` event.
    pub fn log_stats(&self) {
        let s = self.stats();
        let p = self.pool.stats();
        tracing::debug!(
            decoder = D::NAME,
            queue_len = p.queued,
            busy = p.busy,
            in_flight = s.in_flight,
            requests = s.requests,
            hit_rate = s.hit_rate(),
            joins = s.joins,
            completed = s.completed,
            failed = s.failed,
            cancelled = s.cancelled,
            evictions = s.evictions,
            cache_bytes = s.cache_bytes,
            cache_entries = s.cache_entries,
            mean_decode_us = s.mean_decode_time().as_micros() as u64,
            "asset store stats"
        );
    }
}
