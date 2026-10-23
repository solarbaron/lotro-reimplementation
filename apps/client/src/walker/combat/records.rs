// SPDX-License-Identifier: GPL-3.0-or-later
//! The records the combat layer reads, answered on the asset workers (ARCHITECTURE §21: no
//! blocking I/O on the frame thread), as the beam records of the effects layer are.
//!
//! The combat state machine asks synchronously for a skill's, induction's, channel's or
//! effect's visuals ([`super::state::VisualData`]), the stance asks for the worn items'
//! categories and slots (`combat-visuals.md` §11 rule 6) and picking asks whether a body
//! record is selectable (`combat-ui.md` §3.1). [`RecordCache`] holds those answers. A cue
//! names the records it needs ([`Need`]); the layer asks [`RecordCache::ensure`] before it
//! handles the cue, which queues a worker job for the records not answered yet, and the
//! cue (and every cue behind it, so the order of §9.6 holds) waits until the job has
//! answered. Lookups on the frame thread ([`RecordCache::mark_frame_thread`]) never read:
//! a record not answered yet reads as absent and is counted
//! ([`RecordCache::frame_thread_misses`]); worker threads read through the
//! [`RecordReader`] and keep the answer.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::ThreadId;

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_formats::Did;
use tessera_gamedata::combat::{
    ChannelVisuals, EffectVisuals, InductionVisuals, ScriptKey, SkillVisuals,
};

use super::state::VisualData;

/// Most answers kept per kind; beyond it the kind is cleared (hostile-input bound, like
/// `tessera-gamedata`'s own record caches).
pub const MAX_ANSWERS: usize = 16_384;
/// Most records one worker job answers.
pub const MAX_NEEDS_PER_JOB: usize = 64;
/// Most record jobs in flight at once.
pub const MAX_RECORD_JOBS: usize = 8;

/// A record a cue needs answered before it is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Need {
    /// A skill or effect record named by a cue (`what`): its skill visuals (with the
    /// induction and channelling records it names), its effect visuals and the script of
    /// the combat state it induces.
    What(Did),
    /// A worn item's category and slots (§11 rule 6).
    Item(Did),
    /// A body record's selectability (`combat-ui.md` §3.1).
    Body(Did),
}

/// What the stance rule needs of a worn item (§11 rule 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemFacts {
    /// Its rule-6 category label; `""` for an item of another category.
    pub category: String,
    /// `Inventory_DefaultSlot` bits.
    pub default_bits: u64,
    /// `Inventory_CompatibleSlot` bits.
    pub compatible_bits: u64,
}

/// Reads the records behind the answers (blocking; only ever called off the frame thread
/// by [`RecordCache`]).
pub trait RecordReader: Send + Sync + 'static {
    /// A skill's visuals.
    fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>>;
    /// An induction record's scripts.
    fn induction(&self, did: Did) -> Option<Arc<InductionVisuals>>;
    /// A channelling record's scripts.
    fn channel(&self, did: Did) -> Option<Arc<ChannelVisuals>>;
    /// An effect's visuals.
    fn effect(&self, did: Did) -> Option<Arc<EffectVisuals>>;
    /// The script of the combat state an effect induces.
    fn induced_state_script(&self, did: Did) -> Option<ScriptKey>;
    /// A worn item's facts; `None` when its record does not read.
    fn item(&self, did: Did) -> Option<ItemFacts>;
    /// A body record's selectability; `None` when the record does not read.
    fn body_selectable(&self, did: Did) -> Option<bool>;
}

/// The answers of one kind.
type Answers<T> = Mutex<HashMap<Did, T>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn keep<T>(m: &Answers<T>, did: Did, v: T) {
    let mut m = lock(m);
    if m.len() >= MAX_ANSWERS && !m.contains_key(&did) {
        m.clear();
    }
    m.insert(did, v);
}

/// The answers, shared by the frame thread and the worker jobs (module docs).
pub struct RecordCache<R: RecordReader> {
    reader: Arc<R>,
    skills: Answers<Option<Arc<SkillVisuals>>>,
    inductions: Answers<Option<Arc<InductionVisuals>>>,
    channels: Answers<Option<Arc<ChannelVisuals>>>,
    effects: Answers<Option<Arc<EffectVisuals>>>,
    induced: Answers<Option<ScriptKey>>,
    items: Answers<Option<ItemFacts>>,
    bodies: Answers<Option<bool>>,
    /// `What` records answered (skill, effect and induced script together).
    whats: Answers<()>,
    in_flight: Mutex<HashSet<Need>>,
    jobs: Arc<AtomicU64>,
    /// Needs found by frame-thread lookups that are not answered yet (picking), queued by
    /// the next [`RecordCache::ensure`] or [`RecordCache::flush`].
    pending: Mutex<HashSet<Need>>,
    frame_thread: OnceLock<ThreadId>,
    misses: AtomicU64,
    reads: AtomicU64,
}

impl<R: RecordReader> std::fmt::Debug for RecordCache<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordCache")
            .field("misses", &self.frame_thread_misses())
            .field("reads", &self.worker_reads())
            .finish_non_exhaustive()
    }
}

/// Counts a record job as finished, and forgets its needs as in flight, when it is
/// dropped (run or not).
struct JobGuard<R: RecordReader> {
    cache: Arc<RecordCache<R>>,
    needs: Vec<Need>,
}

impl<R: RecordReader> Drop for JobGuard<R> {
    fn drop(&mut self) {
        let mut f = lock(&self.cache.in_flight);
        for n in &self.needs {
            f.remove(n);
        }
        drop(f);
        self.cache.jobs.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<R: RecordReader> RecordCache<R> {
    /// An empty cache over `reader`.
    #[must_use]
    pub fn new(reader: Arc<R>) -> Self {
        Self {
            reader,
            skills: Mutex::new(HashMap::new()),
            inductions: Mutex::new(HashMap::new()),
            channels: Mutex::new(HashMap::new()),
            effects: Mutex::new(HashMap::new()),
            induced: Mutex::new(HashMap::new()),
            items: Mutex::new(HashMap::new()),
            bodies: Mutex::new(HashMap::new()),
            whats: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashSet::new()),
            jobs: Arc::new(AtomicU64::new(0)),
            pending: Mutex::new(HashSet::new()),
            frame_thread: OnceLock::new(),
            misses: AtomicU64::new(0),
            reads: AtomicU64::new(0),
        }
    }

    /// The reader.
    #[must_use]
    pub fn reader(&self) -> &Arc<R> {
        &self.reader
    }

    /// Names the calling thread the frame thread (once): lookups there never read.
    pub fn mark_frame_thread(&self) {
        let _ = self.frame_thread.set(std::thread::current().id());
    }

    /// Whether the calling thread is the frame thread.
    #[must_use]
    pub fn on_frame_thread(&self) -> bool {
        self.frame_thread
            .get()
            .is_some_and(|t| *t == std::thread::current().id())
    }

    /// Lookups on the frame thread that found no answer (each read as absent, no record
    /// was read there).
    #[must_use]
    pub fn frame_thread_misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Records read through the reader (all off the frame thread).
    #[must_use]
    pub fn worker_reads(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    /// Whether `need` is answered.
    #[must_use]
    pub fn answered(&self, need: Need) -> bool {
        match need {
            Need::What(d) => lock(&self.whats).contains_key(&d),
            Need::Item(d) => lock(&self.items).contains_key(&d),
            Need::Body(d) => lock(&self.bodies).contains_key(&d),
        }
    }

    /// Answers `need` by reading (blocking; a worker's job, or any thread but the frame
    /// thread).
    pub fn answer(&self, need: Need) {
        if self.on_frame_thread() {
            self.misses.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(?need, "combat record not read on the frame thread");
            return;
        }
        if self.answered(need) {
            return;
        }
        let r = &self.reader;
        match need {
            Need::What(d) => {
                self.reads.fetch_add(1, Ordering::Relaxed);
                let skill = r.skill(d);
                if let Some(s) = &skill {
                    if let Some(i) = s
                        .induction
                        .filter(|i| !lock(&self.inductions).contains_key(i))
                    {
                        keep(&self.inductions, i, r.induction(i));
                    }
                    if let Some(c) = s
                        .channelling
                        .filter(|c| !lock(&self.channels).contains_key(c))
                    {
                        keep(&self.channels, c, r.channel(c));
                    }
                }
                keep(&self.skills, d, skill);
                keep(&self.effects, d, r.effect(d));
                keep(&self.induced, d, r.induced_state_script(d));
                keep(&self.whats, d, ());
            }
            Need::Item(d) => {
                self.reads.fetch_add(1, Ordering::Relaxed);
                keep(&self.items, d, r.item(d));
            }
            Need::Body(d) => {
                self.reads.fetch_add(1, Ordering::Relaxed);
                keep(&self.bodies, d, r.body_selectable(d));
            }
        }
    }

    /// Whether every need of `needs` is answered; queues a worker job (on `pool`) for the
    /// ones that are neither answered nor in flight, and for the frame-thread misses
    /// waiting in the pending list.
    pub fn ensure(self: &Arc<Self>, pool: &WorkerPool, needs: &[Need]) -> bool {
        let missing: Vec<Need> = needs
            .iter()
            .copied()
            .filter(|n| !self.answered(*n))
            .collect();
        let mut queue = std::mem::take(&mut *lock(&self.pending));
        queue.extend(missing.iter().copied());
        self.spawn(pool, queue);
        missing.is_empty()
    }

    /// Queues the pending frame-thread misses.
    pub fn flush(self: &Arc<Self>, pool: &WorkerPool) {
        let queue = std::mem::take(&mut *lock(&self.pending));
        self.spawn(pool, queue);
    }

    fn spawn(self: &Arc<Self>, pool: &WorkerPool, queue: HashSet<Need>) {
        let mut todo: Vec<Need> = {
            let mut f = lock(&self.in_flight);
            // `insert` is true only for needs not in flight yet.
            queue.into_iter().filter(|n| f.insert(*n)).collect()
        };
        while !todo.is_empty() {
            if self.jobs.load(Ordering::Relaxed) >= MAX_RECORD_JOBS as u64 {
                // Asked for again by the next `ensure`.
                let mut f = lock(&self.in_flight);
                let mut p = lock(&self.pending);
                for n in todo.drain(..) {
                    f.remove(&n);
                    p.insert(n);
                }
                return;
            }
            let batch: Vec<Need> = todo.drain(..todo.len().min(MAX_NEEDS_PER_JOB)).collect();
            let retry = batch.clone();
            self.jobs.fetch_add(1, Ordering::Relaxed);
            let guard = JobGuard {
                cache: Arc::clone(self),
                needs: batch,
            };
            let job = move |_: &tessera_assets::JobContext<'_>| {
                for n in &guard.needs {
                    guard.cache.answer(*n);
                }
                drop(guard);
            };
            if let Err(e) = pool.spawn(Priority::Visible, CancelToken::new(), job) {
                // The refused job's guard has forgotten its batch as in flight; the batch and
                // every need not queued yet are asked for again by the next `ensure` or
                // `flush` (as at the job cap above).
                tracing::debug!("combat record job not queued: {e}");
                let mut f = lock(&self.in_flight);
                let mut p = lock(&self.pending);
                for n in retry.into_iter().chain(todo.drain(..)) {
                    f.remove(&n);
                    p.insert(n);
                }
                return;
            }
        }
    }

    /// The facts of a worn item: `None` while not answered (queued), `Some(None)` when its
    /// record does not read.
    #[must_use]
    pub fn item(&self, did: Did) -> Option<Option<ItemFacts>> {
        self.lookup(&self.items, Need::Item(did), did)
    }

    /// A body record's selectability: `None` while not answered (queued), `Some(None)`
    /// when its record does not read.
    #[must_use]
    pub fn body(&self, did: Did) -> Option<Option<bool>> {
        self.lookup(&self.bodies, Need::Body(did), did)
    }

    fn lookup<T: Clone>(&self, m: &Answers<T>, need: Need, did: Did) -> Option<T> {
        if let Some(v) = lock(m).get(&did) {
            return Some(v.clone());
        }
        if self.on_frame_thread() {
            self.misses.fetch_add(1, Ordering::Relaxed);
            let mut p = lock(&self.pending);
            if p.len() < MAX_ANSWERS {
                p.insert(need);
            }
            return None;
        }
        self.answer(need);
        lock(m).get(&did).cloned()
    }

    /// A `What` answer of one kind (module docs).
    fn what<T: Clone>(&self, m: &Answers<Option<T>>, did: Did) -> Option<T> {
        if let Some(v) = lock(m).get(&did) {
            return v.clone();
        }
        if self.on_frame_thread() {
            self.misses.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(%did, "combat record not answered yet; read as absent");
            return None;
        }
        self.answer(Need::What(did));
        lock(m).get(&did).cloned().flatten()
    }

    /// An induction or channelling answer (read with the skill that names it).
    fn chained<T: Clone>(
        &self,
        m: &Answers<Option<T>>,
        did: Did,
        read: impl FnOnce(&R) -> Option<T>,
    ) -> Option<T> {
        if let Some(v) = lock(m).get(&did) {
            return v.clone();
        }
        if self.on_frame_thread() {
            self.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        self.reads.fetch_add(1, Ordering::Relaxed);
        let v = read(&self.reader);
        keep(m, did, v.clone());
        v
    }
}

impl<R: RecordReader> VisualData for RecordCache<R> {
    fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>> {
        self.what(&self.skills, did)
    }
    fn induction(&self, did: Did) -> Option<Arc<InductionVisuals>> {
        self.chained(&self.inductions, did, |r| r.induction(did))
    }
    fn channel(&self, did: Did) -> Option<Arc<ChannelVisuals>> {
        self.chained(&self.channels, did, |r| r.channel(did))
    }
    fn effect(&self, did: Did) -> Option<Arc<EffectVisuals>> {
        self.what(&self.effects, did)
    }
    fn induced_state_script(&self, did: Did) -> Option<ScriptKey> {
        self.what(&self.induced, did)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// A reader that remembers the threads it was called on.
    #[derive(Default)]
    struct Probe {
        threads: Mutex<Vec<ThreadId>>,
        calls: AtomicU64,
        slow: AtomicBool,
    }

    impl Probe {
        fn note(&self) {
            if self.slow.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            self.calls.fetch_add(1, Ordering::Relaxed);
            lock(&self.threads).push(std::thread::current().id());
        }
    }

    impl RecordReader for Probe {
        fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>> {
            self.note();
            (did.0 == 0x7000_0001).then(|| {
                Arc::new(SkillVisuals {
                    did,
                    self_list: vec![11],
                    induction: Some(Did(0x7000_0002)),
                    ..SkillVisuals::default()
                })
            })
        }
        fn induction(&self, _did: Did) -> Option<Arc<InductionVisuals>> {
            self.note();
            Some(Arc::new(InductionVisuals {
                animation: 21,
                ..InductionVisuals::default()
            }))
        }
        fn channel(&self, _did: Did) -> Option<Arc<ChannelVisuals>> {
            self.note();
            None
        }
        fn effect(&self, _did: Did) -> Option<Arc<EffectVisuals>> {
            self.note();
            None
        }
        fn induced_state_script(&self, _did: Did) -> Option<ScriptKey> {
            self.note();
            None
        }
        fn item(&self, did: Did) -> Option<ItemFacts> {
            self.note();
            Some(ItemFacts {
                category: format!("{did}"),
                ..ItemFacts::default()
            })
        }
        fn body_selectable(&self, _did: Did) -> Option<bool> {
            self.note();
            Some(false)
        }
    }

    fn pool() -> WorkerPool {
        WorkerPool::new(tessera_assets::PoolConfig {
            workers: 2,
            ..tessera_assets::PoolConfig::default()
        })
    }

    fn wait(mut done: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        while !done() {
            assert!(start.elapsed().as_secs() < 20, "the record job finished");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// ARCHITECTURE §21: the frame thread never reads a combat record; a cue's records are
    /// answered on the workers before it is handled, and the frame thread then finds them.
    #[test]
    fn combat_records_are_never_read_on_the_frame_thread() {
        let probe = Arc::new(Probe::default());
        probe.slow.store(true, Ordering::Relaxed);
        let cache = Arc::new(RecordCache::new(Arc::clone(&probe)));
        cache.mark_frame_thread();
        let pool = pool();
        let skill = Did(0x7000_0001);
        let needs = [Need::What(skill), Need::Item(Did(0x7000_0010))];
        // Not answered yet: the cue waits; the lookups read as absent, nothing is read here.
        assert!(!cache.ensure(&pool, &needs));
        assert!(cache.skill(skill).is_none());
        assert_eq!(cache.item(Did(0x7000_0010)), None);
        wait(|| cache.ensure(&pool, &needs));
        // Answered on the workers, found on the frame thread.
        let s = cache.skill(skill).expect("skill visuals");
        assert_eq!(s.self_list, vec![11]);
        assert_eq!(
            cache.induction(Did(0x7000_0002)).map(|i| i.animation),
            Some(21),
            "the induction the skill names is answered with it"
        );
        assert_eq!(
            cache.item(Did(0x7000_0010)).flatten().map(|i| i.category),
            Some(format!("{}", Did(0x7000_0010)))
        );
        // A body asked for by picking: queued, answered by the next flush.
        assert_eq!(cache.body(Did(0x4700_0001)), None);
        cache.flush(&pool);
        wait(|| cache.answered(Need::Body(Did(0x4700_0001))));
        assert_eq!(cache.body(Did(0x4700_0001)), Some(Some(false)));
        let me = std::thread::current().id();
        let threads = lock(&probe.threads).clone();
        assert!(!threads.is_empty());
        assert!(
            threads.iter().all(|t| *t != me),
            "no record read on the frame thread"
        );
        assert!(
            cache.frame_thread_misses() >= 3,
            "the early lookups were counted"
        );
        // One read per need (the skill's induction is read with it).
        assert_eq!(cache.worker_reads(), 3);
        assert_eq!(probe.calls.load(Ordering::Relaxed), 6);
    }

    /// A pool that refuses jobs (queue full) loses no need: every need of the refused jobs,
    /// more than one job's worth, is asked for again and answered once the pool takes jobs.
    #[test]
    fn needs_refused_by_the_pool_are_asked_for_again() {
        let probe = Arc::new(Probe::default());
        let cache = Arc::new(RecordCache::new(Arc::clone(&probe)));
        cache.mark_frame_thread();
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 1,
            ..tessera_assets::PoolConfig::default()
        });
        // Block the worker, then fill the queue.
        let (release, gate) = std::sync::mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let (started_tx, started) = std::sync::mpsc::channel::<()>();
        pool.spawn(
            Priority::Visible,
            CancelToken::new(),
            move |_: &tessera_assets::JobContext<'_>| {
                let _ = started_tx.send(());
                let _ = lock(&gate).recv();
            },
        )
        .expect("blocking job");
        started
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("the worker runs the blocking job");
        pool.spawn(
            Priority::Visible,
            CancelToken::new(),
            |_: &tessera_assets::JobContext<'_>| {},
        )
        .expect("queue filler");
        let needs: Vec<Need> = (0..(MAX_NEEDS_PER_JOB as u32 + 20))
            .map(|i| Need::Body(Did(0x4700_1000 + i)))
            .collect();
        assert!(!cache.ensure(&pool, &needs), "refused: not answered");
        assert!(
            lock(&cache.in_flight).is_empty(),
            "no need is left in flight by a refused job"
        );
        assert_eq!(lock(&cache.pending).len(), needs.len());
        let _ = release.send(());
        wait(|| cache.ensure(&pool, &needs));
        assert!(needs.iter().all(|n| cache.answered(*n)));
        assert_eq!(cache.worker_reads(), needs.len() as u64);
    }

    /// Off the frame thread (tests, tools) a lookup reads at once.
    #[test]
    fn other_threads_read_through() {
        let probe = Arc::new(Probe::default());
        let cache = RecordCache::new(Arc::clone(&probe));
        assert!(cache.skill(Did(0x7000_0001)).is_some());
        assert!(cache.answered(Need::What(Did(0x7000_0001))));
        assert_eq!(cache.body(Did(5)), Some(Some(false)));
        assert_eq!(cache.frame_thread_misses(), 0);
    }
}
