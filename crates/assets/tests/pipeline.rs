// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the pool and the store on synthetic, in-memory records.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tessera_assets::{
    AssetError, AssetKey, AssetState, AssetStore, CancelToken, Decoder, JobContext, MemorySource,
    Outcome, PoolConfig, Priority, RawBytes, StoreConfig, WorkerPool,
};

const T: Duration = Duration::from_secs(20);

/// DIDs with special behaviour in [`TestDecoder`].
const GATE: u32 = 0xF000_0000;
const PANIC: u32 = 0xF000_0001;
const FAIL: u32 = 0xF000_0002;
const SLOW: u32 = 0xF000_0003;

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

/// Records the decode order; blocks on `GATE` until the gate opens; misbehaves on the
/// other special DIDs. The product is the record bytes.
#[derive(Clone, Default)]
struct TestDecoder {
    log: Arc<Mutex<Vec<u32>>>,
    calls: Arc<AtomicUsize>,
    gate: Arc<Gate>,
    /// When set, `size_of` panics.
    size_panics: Arc<AtomicBool>,
}

impl Decoder for TestDecoder {
    type Output = Vec<u8>;
    type Error = std::io::Error;
    const NAME: &'static str = "test";

    fn decode(
        &self,
        key: AssetKey,
        bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<Vec<u8>, std::io::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.log.lock().unwrap().push(key.did);
        match key.did {
            GATE => self.gate.wait(),
            PANIC => panic!("synthetic decoder panic"),
            FAIL => return Err(std::io::Error::other("synthetic failure")),
            SLOW => {
                let start = Instant::now();
                while start.elapsed() < T {
                    if ctx.is_cancelled() {
                        return Err(std::io::Error::other("aborted"));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            _ => {}
        }
        Ok(bytes)
    }

    fn size_of(&self, value: &Vec<u8>) -> usize {
        assert!(
            !self.size_panics.load(Ordering::SeqCst),
            "synthetic size_of panic"
        );
        value.len()
    }
}

fn source(dids: impl IntoIterator<Item = u32>, len: usize) -> MemorySource {
    let mut s: MemorySource = dids
        .into_iter()
        .map(|d| (d, vec![(d & 0xFF) as u8; len]))
        .collect();
    for d in [GATE, PANIC, FAIL, SLOW] {
        s.insert(d, vec![0u8; 1]);
    }
    s
}

fn pool(workers: usize) -> WorkerPool {
    WorkerPool::new(PoolConfig {
        workers,
        ..PoolConfig::default()
    })
}

fn wait_until(mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < T, "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn loads_and_posts_one_completion() {
    let pool = pool(2);
    let store = AssetStore::new(&pool, source(1..=3, 8), RawBytes, StoreConfig::default());
    let h = store.request(2, Priority::Visible);
    let AssetState::Ready(v) = h.wait(T) else {
        panic!("not ready: {:?}", h.state())
    };
    assert_eq!(*v, vec![2u8; 8]);
    wait_until(|| store.pending_completions() == 1);
    let c = store.poll_completions(10);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].key, AssetKey::from(2));
    assert!(matches!(c[0].outcome, Outcome::Ready(_)));
    assert_eq!(c[0].priority, Some(Priority::Visible));
    // A second request is a cache hit: final at once, no completion.
    let again = store.request(2, Priority::Visible);
    assert!(again.is_finished());
    assert!(Arc::ptr_eq(&again.get().unwrap(), &v));
    assert_eq!(store.pending_completions(), 0);
    let s = store.stats();
    assert_eq!((s.requests, s.hits, s.misses, s.completed), (2, 1, 1, 1));
    assert!((s.hit_rate() - 0.5).abs() < 1e-9);
    assert_eq!(s.cache_bytes, 8 + std::mem::size_of::<Vec<u8>>());
}

#[test]
fn duplicate_requests_share_one_job() {
    let pool = pool(4);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source([7], 4), dec.clone(), StoreConfig::default());
    // Hold the only interesting job behind the gate so all requests see it in flight.
    let gate = store.request(GATE, Priority::Immediate);
    let handles: Vec<_> = (0..10)
        .map(|_| store.request(7, Priority::Prefetch))
        .collect();
    let _ = &gate;
    wait_until(|| handles[0].is_finished());
    let first = handles[0].get().unwrap();
    for h in &handles {
        assert!(Arc::ptr_eq(&h.wait(T).ready().cloned().unwrap(), &first));
    }
    dec.gate.open();
    gate.wait(T);
    let log = dec.log.lock().unwrap().clone();
    assert_eq!(log.iter().filter(|&&d| d == 7).count(), 1, "{log:?}");
    let s = store.stats();
    assert_eq!(s.misses, 2);
    assert_eq!(s.joins + s.hits, 9);
}

#[test]
fn dedup_while_queued() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source([7], 4), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let handles: Vec<_> = (0..50)
        .map(|_| store.request(7, Priority::Prefetch))
        .collect();
    assert_eq!(store.stats().joins, 49);
    assert!(store.is_in_flight(7));
    dec.gate.open();
    for h in &handles {
        assert!(matches!(h.wait(T), AssetState::Ready(_)));
    }
    assert_eq!(dec.calls.load(Ordering::SeqCst), 2);
    wait_until(|| store.pending_completions() == 2);
}

#[test]
fn priority_order_is_respected() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=6, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let reqs = [
        (1, Priority::Prefetch),
        (2, Priority::Prefetch),
        (3, Priority::Visible),
        (4, Priority::Immediate),
        (5, Priority::Visible),
        (6, Priority::Prefetch),
    ];
    let handles: Vec<_> = reqs.iter().map(|&(d, p)| store.request(d, p)).collect();
    // Raise 6 above everything, lower 4 to the bottom.
    handles[5].set_priority(Priority::Immediate);
    handles[3].set_priority(Priority::Prefetch);
    assert_eq!(pool.queue_len(), 6);
    dec.gate.open();
    for h in &handles {
        h.wait(T);
    }
    let log = dec.log.lock().unwrap().clone();
    assert_eq!(log, vec![GATE, 6, 3, 5, 1, 2, 4]);
}

#[test]
fn joining_raises_priority() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let a = store.request(1, Priority::Visible);
    let b = store.request(2, Priority::Prefetch);
    let b2 = store.request(2, Priority::Immediate); // joins and bumps 2
    dec.gate.open();
    for h in [&a, &b, &b2] {
        h.wait(T);
    }
    assert_eq!(*dec.log.lock().unwrap(), vec![GATE, 2, 1]);
}

#[test]
fn cancelled_job_never_decodes() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let x = store.request(1, Priority::Visible);
    let y = store.request(2, Priority::Visible);
    x.cancel();
    dec.gate.open();
    assert!(matches!(x.wait(T), AssetState::Cancelled));
    assert!(matches!(y.wait(T), AssetState::Ready(_)));
    assert!(!dec.log.lock().unwrap().contains(&1));
    wait_until(|| store.pending_completions() == 3);
    let cs = store.poll_completions(usize::MAX);
    let c1 = cs.iter().find(|c| c.key.did == 1).unwrap();
    assert!(matches!(c1.outcome, Outcome::Cancelled));
    assert_eq!(store.stats().cancelled, 1);
    assert!(!store.is_in_flight(1));
    // A new request after cancellation starts afresh.
    assert!(matches!(
        store.request(1, Priority::Visible).wait(T),
        AssetState::Ready(_)
    ));
}

#[test]
fn shared_token_cancels_a_group_and_other_interest_survives() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=4, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let panel = CancelToken::new();
    let hs: Vec<_> = (1..=3)
        .map(|d| store.request_with(d, Priority::Visible, panel.clone()))
        .collect();
    // Someone else also wants 3.
    let other = store.request(3, Priority::Prefetch);
    panel.cancel();
    dec.gate.open();
    assert!(matches!(hs[0].wait(T), AssetState::Cancelled));
    assert!(matches!(hs[1].wait(T), AssetState::Cancelled));
    assert!(matches!(other.wait(T), AssetState::Ready(_)));
    let log = dec.log.lock().unwrap().clone();
    assert_eq!(log, vec![GATE, 3]);
    // A request with an already-cancelled token does nothing.
    let dead = store.request_with(4, Priority::Immediate, panel);
    assert!(matches!(dead.state(), AssetState::Cancelled));
    assert!(!store.is_in_flight(4));
}

#[test]
fn cancel_during_decode_is_observed() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source([], 1), dec.clone(), StoreConfig::default());
    let h = store.request(SLOW, Priority::Visible);
    wait_until(|| matches!(h.state(), AssetState::Loading) && !dec.log.lock().unwrap().is_empty());
    let start = Instant::now();
    h.cancel();
    assert!(matches!(h.wait(T), AssetState::Cancelled));
    assert!(start.elapsed() < Duration::from_secs(10));
    // Not remembered as a failure.
    assert_eq!(store.stats().failed, 0);
}

#[test]
fn eviction_is_lru_by_bytes_and_weak_refs_revive() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let cfg = StoreConfig {
        cache_budget: 100,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(1..=4, 40), dec.clone(), cfg);
    let load = |d: u32| {
        store
            .request(d, Priority::Visible)
            .wait(T)
            .ready()
            .cloned()
            .unwrap()
    };
    let a = load(1);
    drop(load(2));
    assert_eq!(store.stats().cache_bytes, 80);
    assert!(Arc::ptr_eq(&load(1), &a)); // touch 1: 2 becomes LRU
    drop(load(3)); // evicts 2
    assert!(store.contains(1) && store.contains(3) && !store.contains(2));
    // The eviction counter is updated just after the waiters are woken.
    wait_until(|| store.stats().evictions == 1);
    let s = store.stats();
    assert_eq!((s.evictions, s.cache_bytes), (1, 80));
    // 1 is still held by `a`: evicted later, it comes back without decoding.
    drop(load(4)); // evicts 1 (LRU)
    assert!(!store.contains(1));
    let calls = dec.calls.load(Ordering::SeqCst);
    assert!(Arc::ptr_eq(&load(1), &a));
    assert_eq!(dec.calls.load(Ordering::SeqCst), calls);
    // 2 was dropped by everyone (completions also hold products, so drain them first):
    // it is decoded again.
    drop(store.poll_completions(usize::MAX));
    drop(load(2));
    assert_eq!(dec.calls.load(Ordering::SeqCst), calls + 1);
    assert!(store.stats().cache_bytes <= 100);
    // Shrinking the budget evicts.
    store.set_cache_budget(40);
    assert!(store.stats().cache_bytes <= 40);
    store.clear_cache();
    assert_eq!(store.stats().cache_entries, 0);
}

#[test]
fn oversized_products_are_delivered_not_cached() {
    let pool = pool(1);
    let cfg = StoreConfig {
        cache_budget: 10,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source([1], 64), TestDecoder::default(), cfg);
    let v = store.request(1, Priority::Visible).wait(T);
    assert_eq!(v.ready().unwrap().len(), 64);
    assert!(!store.contains(1));
    assert_eq!(store.stats().uncacheable, 1);
}

#[test]
fn failures_are_values_and_remembered() {
    let pool = pool(2);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source([], 1), dec.clone(), StoreConfig::default());
    store.set_placeholder(Some(Arc::new(b"checker".to_vec())));
    let missing = store.request(0x1234, Priority::Visible);
    assert!(matches!(
        missing.wait(T),
        AssetState::Failed(AssetError::NotFound { did: 0x1234 })
    ));
    assert_eq!(&**missing.get_or_placeholder().unwrap(), b"checker");
    let bad = store.request(FAIL, Priority::Visible);
    let AssetState::Failed(e) = bad.wait(T) else {
        panic!()
    };
    assert!(matches!(
        e,
        AssetError::Decode {
            did: FAIL,
            decoder: "test",
            ..
        }
    ));
    assert!(e.to_string().contains("synthetic failure"));
    // Remembered: no second decode.
    let again = store.request(FAIL, Priority::Visible);
    assert!(matches!(again.state(), AssetState::Failed(_)));
    assert_eq!(dec.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.stats().failure_hits, 1);
    store.clear_failures();
    store.request(FAIL, Priority::Visible).wait(T);
    assert_eq!(dec.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn decoder_panic_fails_only_its_job() {
    let pool = pool(1);
    let store = AssetStore::new(
        &pool,
        source([1], 1),
        TestDecoder::default(),
        StoreConfig::default(),
    );
    let p = store.request(PANIC, Priority::Visible);
    assert!(matches!(
        p.wait(T),
        AssetState::Failed(AssetError::Panicked {
            did: PANIC,
            stage: "decode"
        })
    ));
    assert!(matches!(
        store.request(1, Priority::Visible).wait(T),
        AssetState::Ready(_)
    ));
    // Panics are not data errors, so they are retried rather than remembered.
    assert!(matches!(
        store.request(PANIC, Priority::Visible).wait(T),
        AssetState::Failed(AssetError::Panicked { .. })
    ));
    assert_eq!(store.stats().failure_hits, 0);
}

#[test]
fn full_queue_rejects() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued: 2,
        ..PoolConfig::default()
    });
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=5, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let a = store.request(1, Priority::Visible);
    let b = store.request(2, Priority::Visible);
    let c = store.request(3, Priority::Visible);
    assert!(matches!(
        c.state(),
        AssetState::Failed(AssetError::QueueFull { capacity: 2 })
    ));
    assert_eq!(store.stats().rejected, 1);
    dec.gate.open();
    assert!(a.wait(T).is_finished() && b.wait(T).is_finished());
    // Not remembered: works once there is room.
    assert!(matches!(
        store.request(3, Priority::Visible).wait(T),
        AssetState::Ready(_)
    ));
}

#[test]
fn completion_queue_is_bounded() {
    let pool = pool(2);
    let cfg = StoreConfig {
        max_completions: 4,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(1..=20, 1), RawBytes, cfg);
    let hs: Vec<_> = (1..=20)
        .map(|d| store.request(d, Priority::Visible))
        .collect();
    for h in &hs {
        h.wait(T);
    }
    // Completions are posted just after the job is counted and its waiters woken.
    wait_until(|| {
        let s = store.stats();
        s.completed == 20 && s.pending_completions as u64 + s.dropped_completions == 20
    });
    let s = store.stats();
    assert_eq!(s.pending_completions, 4);
    assert_eq!(s.dropped_completions, 16);
    assert!(store.try_next_completion().is_some());
    assert_eq!(store.poll_completions(2).len(), 2);
}

#[test]
fn dropping_the_store_and_pool_releases_waiters() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let queued: Vec<_> = (1..=3)
        .map(|d| store.request(d, Priority::Visible))
        .collect();
    drop(store);
    // Open the gate from another thread while the pool drop joins its worker.
    let g = Arc::clone(&dec.gate);
    let opener = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        g.open();
    });
    drop(pool);
    opener.join().unwrap();
    for h in &queued {
        assert!(
            matches!(h.wait(T), AssetState::Failed(AssetError::Shutdown)),
            "{h:?}"
        );
    }
    assert!(gate.wait(T).is_finished());
}

#[test]
fn variants_are_separate_assets() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let store = AssetStore::new(&pool, source([9], 1), dec.clone(), StoreConfig::default());
    store
        .request(AssetKey::new(9, 0), Priority::Visible)
        .wait(T);
    store
        .request(AssetKey::new(9, 1), Priority::Visible)
        .wait(T);
    assert_eq!(dec.calls.load(Ordering::SeqCst), 2);
    assert!(store.contains(AssetKey::new(9, 1)));
    store.invalidate(AssetKey::new(9, 1));
    assert!(!store.contains(AssetKey::new(9, 1)) && store.contains(9));
}

/// Many threads hammer one store with requests, re-prioritisations and cancellations over
/// a small key space and a tight budget. Every handle must settle, nothing may deadlock,
/// and the cache must stay within budget.
#[test]
fn stress_no_deadlock() {
    let pool = pool(4);
    let cfg = StoreConfig {
        cache_budget: 2_000,
        max_tickets_per_job: 8,
        max_completions: 256,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(0..200, 100), TestDecoder::default(), cfg);
    let threads: Vec<_> = (0..8u64)
        .map(|t| {
            let store = store.clone();
            std::thread::spawn(move || {
                let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ t;
                let mut rng = move || {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    x
                };
                let mut handles = Vec::new();
                for i in 0..3_000u64 {
                    let did = (rng() % 220) as u32; // some missing
                    let p = Priority::ALL[(rng() % 3) as usize];
                    let h = store.request(did, p);
                    match rng() % 8 {
                        0 => h.cancel(),
                        1 => h.set_priority(Priority::ALL[(rng() % 3) as usize]),
                        _ => {}
                    }
                    if i % 64 == 0 {
                        let _ = store.poll_completions(64);
                    }
                    if i % 500 == 0 {
                        store.clear_failures();
                    }
                    handles.push(h);
                }
                for h in &handles {
                    let s = h.wait(T);
                    assert!(s.is_finished(), "unsettled {h:?}");
                    if let AssetState::Ready(v) = &s {
                        assert_eq!(v.len(), 100);
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().expect("stress thread");
    }
    wait_until(|| store.stats().in_flight == 0);
    let s = store.stats();
    assert!(s.cache_bytes <= 2_000);
    assert_eq!(pool.queue_len(), 0);
    assert_eq!(s.requests, 24_000);
    assert_eq!(s.hits + s.misses + s.joins + s.failure_hits, 24_000);
    assert_eq!(s.completed + s.failed + s.cancelled, s.misses);
}

// ---------------------------------------------------------------------------------------
// Regression tests for review findings
// ---------------------------------------------------------------------------------------

fn gated(max_queued: usize) -> (WorkerPool, TestDecoder) {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued,
        ..PoolConfig::default()
    });
    (pool, TestDecoder::default())
}

/// Cancelling a queued request takes its job out of the queue at once: the handle reports
/// `Cancelled`, a completion is posted, and the queue capacity is free for new work.
#[test]
fn cancel_frees_queue_capacity_at_once() {
    let (pool, dec) = gated(2);
    let store = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let a = store.request(1, Priority::Visible);
    let b = store.request(2, Priority::Visible);
    assert_eq!(pool.queue_len(), 2);
    a.cancel();
    b.cancel();
    assert!(matches!(a.state(), AssetState::Cancelled), "{a:?}");
    assert!(matches!(b.state(), AssetState::Cancelled), "{b:?}");
    assert_eq!(pool.queue_len(), 0);
    assert!(!store.is_in_flight(1) && !store.is_in_flight(2));
    let cs = store.poll_completions(usize::MAX);
    assert_eq!(cs.len(), 2);
    assert!(cs.iter().all(|c| matches!(c.outcome, Outcome::Cancelled)));
    let c = store.request(3, Priority::Immediate);
    assert!(matches!(c.state(), AssetState::Queued), "{c:?}");
    dec.gate.open();
    assert!(matches!(c.wait(T), AssetState::Ready(_)));
    assert!(!dec.log.lock().unwrap().iter().any(|&d| d == 1 || d == 2));
    let s = store.stats();
    assert_eq!((s.cancelled, s.rejected), (2, 0));
}

/// A request cancelled while other requests still want the job keeps the job queued.
#[test]
fn cancel_with_remaining_interest_keeps_the_job() {
    let (pool, dec) = gated(4);
    let store = AssetStore::new(&pool, source(1..=1, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let a = store.request(1, Priority::Visible);
    let b = store.request(1, Priority::Prefetch);
    a.cancel();
    assert!(matches!(b.state(), AssetState::Queued));
    assert_eq!(pool.queue_len(), 1);
    dec.gate.open();
    assert!(matches!(b.wait(T), AssetState::Ready(_)));
}

/// Jobs cancelled only through a shared token (no store call) are swept out of a full
/// queue when new work needs room.
#[test]
fn token_cancelled_jobs_make_room_in_a_full_queue() {
    let (pool, dec) = gated(2);
    let store = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = store.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let panel = CancelToken::new();
    let a = store.request_with(1, Priority::Visible, panel.clone());
    let b = store.request_with(2, Priority::Visible, panel.clone());
    panel.cancel();
    let c = store.request(3, Priority::Immediate);
    assert!(matches!(c.state(), AssetState::Queued), "{c:?}");
    assert!(matches!(a.state(), AssetState::Cancelled), "{a:?}");
    assert!(matches!(b.state(), AssetState::Cancelled), "{b:?}");
    assert!(!store.is_in_flight(1) && !store.is_in_flight(2));
    assert_eq!(pool.queue_len(), 1);
    dec.gate.open();
    assert!(matches!(c.wait(T), AssetState::Ready(_)));
    assert_eq!(store.stats().cancelled, 2);
}

/// Dropping a store whose pool is still used by another store withdraws its queued jobs
/// at once: handles end as `Shutdown` and the capacity is free.
#[test]
fn dropping_a_store_frees_its_queued_jobs_at_once() {
    let (pool, dec) = gated(2);
    let a = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let b = AssetStore::new(&pool, source(1..=3, 1), dec.clone(), StoreConfig::default());
    let gate = a.request(GATE, Priority::Immediate);
    wait_until(|| matches!(gate.state(), AssetState::Loading));
    let h1 = b.request(1, Priority::Visible);
    let h2 = b.request(2, Priority::Visible);
    assert_eq!(pool.queue_len(), 2);
    drop(b);
    for h in [&h1, &h2] {
        assert!(
            matches!(h.state(), AssetState::Failed(AssetError::Shutdown)),
            "{h:?}"
        );
    }
    assert_eq!(pool.queue_len(), 0);
    let h3 = a.request(3, Priority::Immediate);
    assert!(matches!(h3.state(), AssetState::Queued), "{h3:?}");
    dec.gate.open();
    assert!(matches!(h3.wait(T), AssetState::Ready(_)));
}

/// A product too large for the cache stays revivable while someone holds it.
#[test]
fn oversized_product_is_revived_while_held() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let cfg = StoreConfig {
        cache_budget: 10,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source([1], 64), dec.clone(), cfg);
    let first = store
        .request(1, Priority::Visible)
        .wait(T)
        .ready()
        .cloned()
        .unwrap();
    let again = store.request(1, Priority::Visible);
    assert!(again.is_finished(), "{again:?}");
    assert!(Arc::ptr_eq(&again.get().unwrap(), &first));
    assert_eq!(dec.calls.load(Ordering::SeqCst), 1);
    let s = store.stats();
    assert_eq!((s.hits, s.misses, s.uncacheable), (1, 1, 1));
    assert!(!store.contains(1));
    // Once nobody holds it, it is decoded again. The worker drops its reference to
    // the finished job just after settling it, so wait for that.
    let weak = Arc::downgrade(&first);
    drop((first, again));
    // The job's completion is posted just after its waiters are woken.
    wait_until(|| store.pending_completions() == 1);
    drop(store.poll_completions(usize::MAX));
    wait_until(|| weak.strong_count() == 0);
    assert!(matches!(
        store.request(1, Priority::Visible).wait(T),
        AssetState::Ready(_)
    ));
    assert_eq!(dec.calls.load(Ordering::SeqCst), 2);
}

/// Reviving an evicted product never calls `Decoder::size_of` (user code) on the caller's
/// thread, so a panicking `size_of` cannot unwind out of `request`.
#[test]
fn revival_does_not_call_size_of() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let cfg = StoreConfig {
        cache_budget: 100,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(1..=3, 40), dec.clone(), cfg);
    let load = |d: u32| {
        store
            .request(d, Priority::Visible)
            .wait(T)
            .ready()
            .cloned()
            .unwrap()
    };
    let one = load(1);
    drop(load(2));
    drop(load(3)); // evicts 1 into the weak map
    assert!(!store.contains(1));
    dec.size_panics.store(true, Ordering::SeqCst);
    let h = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.request(1, Priority::Visible)
    }))
    .expect("request must not panic");
    assert!(Arc::ptr_eq(&h.get().unwrap(), &one));
    assert!(store.contains(1));
    assert!(store.stats().cache_bytes <= 100);
}

/// A panicking `size_of` on the worker makes the product uncacheable whatever the budget
/// (it used to be charged as `usize::MAX` and overflow an "unlimited" cache).
#[test]
fn panicking_size_of_is_uncacheable_even_with_unlimited_budget() {
    let pool = pool(1);
    let dec = TestDecoder::default();
    let cfg = StoreConfig {
        cache_budget: usize::MAX,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(1..=2, 8), dec.clone(), cfg);
    assert!(matches!(
        store.request(1, Priority::Visible).wait(T),
        AssetState::Ready(_)
    ));
    dec.size_panics.store(true, Ordering::SeqCst);
    let two = store.request(2, Priority::Visible);
    assert!(matches!(two.wait(T), AssetState::Ready(_)), "{two:?}");
    assert!(store.contains(1), "nothing may be evicted for it");
    assert!(!store.contains(2));
    let s = store.stats();
    assert_eq!((s.uncacheable, s.cache_bytes), (1, 8));
}

/// Completions are bounded by the bytes of the products they hold, so a consumer that
/// never polls them cannot keep every product alive.
#[test]
fn completion_queue_is_bounded_by_bytes() {
    let pool = pool(2);
    let cfg = StoreConfig {
        cache_budget: 0,
        max_completion_bytes: 1_000,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(1..=10, 400), TestDecoder::default(), cfg);
    let mut weak = Vec::new();
    for d in 1..=10 {
        let v = store
            .request(d, Priority::Visible)
            .wait(T)
            .ready()
            .cloned()
            .unwrap();
        weak.push(Arc::downgrade(&v));
    }
    wait_until(|| store.stats().completed == 10);
    // A worker drops its reference to a finished job just after settling it, so allow
    // that moment to pass before counting.
    wait_until(|| weak.iter().filter(|w| w.strong_count() > 0).count() <= 2);
    let s = store.stats();
    assert!(s.pending_completions <= 2);
    assert_eq!(s.dropped_completions as usize + s.pending_completions, 10);
    // The newest completion is kept even when it alone exceeds the bound.
    let big = AssetStore::new(
        &pool,
        source([1], 5_000),
        TestDecoder::default(),
        StoreConfig {
            max_completion_bytes: 1_000,
            ..StoreConfig::default()
        },
    );
    big.request(1, Priority::Visible).wait(T);
    wait_until(|| big.pending_completions() == 1);
}

/// A request with an already-cancelled token still gets the placeholder; `wait` accepts
/// `Duration::MAX`.
#[test]
fn precancelled_request_has_placeholder_and_wait_max_does_not_panic() {
    let pool = pool(1);
    let store = AssetStore::new(&pool, source([1], 1), RawBytes, StoreConfig::default());
    store.set_placeholder(Some(Arc::new(b"checker".to_vec())));
    let token = CancelToken::new();
    token.cancel();
    let h = store.request_with(1, Priority::Visible, token);
    assert!(matches!(h.wait(Duration::MAX), AssetState::Cancelled));
    assert_eq!(&**h.get_or_placeholder().unwrap(), b"checker");
    let live = store.request(1, Priority::Visible);
    assert!(matches!(live.wait(Duration::MAX), AssetState::Ready(_)));
}

/// Like `stress_no_deadlock`, but with a queue so small that requests are refused and
/// abandoned jobs are swept and withdrawn concurrently with workers popping them.
#[test]
fn stress_tiny_queue_with_group_cancels() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 3,
        max_queued: 8,
        ..PoolConfig::default()
    });
    let cfg = StoreConfig {
        cache_budget: 1_000,
        max_completions: 64,
        ..StoreConfig::default()
    };
    let store = AssetStore::new(&pool, source(0..100, 50), TestDecoder::default(), cfg);
    let threads: Vec<_> = (0..6u64)
        .map(|t| {
            let store = store.clone();
            std::thread::spawn(move || {
                let mut x = 0xD1B5_4A32_D192_ED03u64 ^ (t + 1);
                let mut rng = move || {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    x
                };
                let mut handles = Vec::new();
                let mut group = CancelToken::new();
                for i in 0..2_000u64 {
                    let did = (rng() % 110) as u32;
                    let p = Priority::ALL[(rng() % 3) as usize];
                    let h = store.request_with(did, p, group.clone());
                    match rng() % 6 {
                        0 => h.cancel(),
                        1 => {
                            group.cancel();
                            group = CancelToken::new();
                        }
                        _ => {}
                    }
                    if i % 32 == 0 {
                        let _ = store.poll_completions(64);
                    }
                    handles.push(h);
                }
                group.cancel();
                // Make room so abandoned leftovers are swept or run.
                for h in &handles {
                    let s = h.wait(T);
                    assert!(s.is_finished(), "unsettled {h:?}");
                }
            })
        })
        .collect();
    for t in threads {
        t.join().expect("stress thread");
    }
    wait_until(|| store.stats().in_flight == 0 && pool.queue_len() == 0);
    let s = store.stats();
    assert!(s.cache_bytes <= 1_000);
    assert_eq!(s.requests, 12_000);
    assert_eq!(s.completed + s.failed + s.cancelled, s.misses - s.rejected);
}
