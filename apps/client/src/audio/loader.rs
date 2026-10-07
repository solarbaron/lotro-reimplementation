// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound infos (0x2A) and payloads (0x0A) for the engine (`sound.md` §4, §12.1–§12.3).
//!
//! - Sound infos are tiny and needed synchronously when a Sound node starts (the request's
//!   loop flag, positional class, distances and priority decide its gates). The start-up
//!   and landblock jobs prefetch the infos their scripts name into [`InfoCache`] on the
//!   asset workers; a miss (a driven `SoundScript_SoundID`) reads the record on the
//!   calling thread and is counted ([`LoaderStats::sync_info_reads`]).
//! - Payloads are read and decoded on the `tessera-assets` workers
//!   ([`tessera_audio::decode::open_record`]: short sounds whole, music streamed) and
//!   handed to the engine through [`tessera_audio::engine::payload_channel`]. Decoded short
//!   sounds are kept in a byte-bounded cache. A new load is refused only when more than
//!   [`tessera_audio::engine::MAX_WAITING_REQUESTS`] are in flight (`sound.md` §6.8 item 3,
//!   §12.3 step 2), the engine's own bound on its waiting requests.
//! - [`request_from`] converts a sound info and a Sound node's request into the engine's
//!   [`SoundRequest`] (decision 0019: "the client converts a sound-info record (0x2A) and a
//!   Sound node into a `SoundRequest`").

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_audio::decode::{Playable, StreamPolicy, open_record};
use tessera_audio::engine::{
    Channel, MAX_WAITING_REQUESTS, Payload, PositionalClass, Positioner, SoundRequest, SoundSource,
    payload_channel,
};
use tessera_audio::pcm::Pcm;
use tessera_dat::ArchiveSet;
use tessera_formats::{Did, Record as _, SoundInfo};
use tessera_scriptgraph::SoundStart;

/// Most sound infos cached (the install holds about 45,500).
pub const MAX_CACHED_INFOS: usize = 65_536;
/// Default byte budget of decoded short sounds.
pub const DEFAULT_PCM_BUDGET: usize = 96 << 20;
/// Largest single decoded sound kept in the cache.
pub const MAX_CACHED_PCM: usize = 16 << 20;

/// Whether a new payload load is refused with `in_flight` loads in flight: `sound.md` §6.8
/// item 3, "More than 250 requests are already waiting → refused", as the engine's gate.
#[must_use]
pub fn load_bound_reached(in_flight: usize) -> bool {
    in_flight > MAX_WAITING_REQUESTS
}

/// What the effect sink needs from a loader: sound infos and payload sources (the seam
/// that lets the sink be tested without an install).
pub trait SoundProvider {
    /// The sound info `did`; `None` when missing or malformed.
    fn info(&self, did: Did) -> Option<Arc<SoundInfo>>;

    /// The audio of `payload`; `token` cancels a queued load.
    ///
    /// # Errors
    ///
    /// When the load cannot be queued.
    fn source(&self, payload: Did, token: CancelToken) -> Result<SoundSource, String>;
}

impl SoundProvider for SoundLoader {
    fn info(&self, did: Did) -> Option<Arc<SoundInfo>> {
        Self::info(self, did)
    }

    fn source(&self, payload: Did, token: CancelToken) -> Result<SoundSource, String> {
        Self::source(self, payload, token)
    }
}

/// Locks a mutex, recovering the data of a poisoned one (a panicking worker job must not
/// take the audio down).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Reads a record: `Ok(None)` when it does not exist.
///
/// # Errors
///
/// Archive errors.
pub fn fetch(set: &ArchiveSet, did: u32) -> Result<Option<Vec<u8>>, tessera_dat::Error> {
    match set.locate(did)? {
        Some(l) => set.read_located(&l).map(Some),
        None => Ok(None),
    }
}

/// Reads and decodes a sound info; `None` when missing or malformed (logged).
#[must_use]
pub fn read_info(set: &ArchiveSet, did: Did) -> Option<SoundInfo> {
    if did.prefix() != tessera_audio::engine::SOUND_INFO_DID_PREFIX as u8 {
        return None;
    }
    match fetch(set, did.0) {
        Ok(Some(b)) => match SoundInfo::parse_expecting(&b, did) {
            Ok(i) => Some(i),
            Err(e) => {
                tracing::debug!(%did, error = %e, "sound info does not decode");
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            tracing::debug!(%did, error = %e, "sound info unreadable");
            None
        }
    }
}

/// Decoded sound infos shared by the workers and the main thread (bounded by
/// [`MAX_CACHED_INFOS`]; `None` entries remember records that are missing or malformed).
#[derive(Debug, Default)]
pub struct InfoCache {
    map: Mutex<HashMap<Did, Option<Arc<SoundInfo>>>>,
}

impl InfoCache {
    /// The cached entry, if the DID was looked up before.
    #[must_use]
    pub fn get(&self, did: Did) -> Option<Option<Arc<SoundInfo>>> {
        lock(&self.map).get(&did).cloned()
    }

    /// The info, read from `set` when not cached (and then cached).
    #[must_use]
    pub fn load(&self, set: &ArchiveSet, did: Did) -> Option<Arc<SoundInfo>> {
        if let Some(e) = self.get(did) {
            return e;
        }
        let info = read_info(set, did).map(Arc::new);
        let mut m = lock(&self.map);
        if m.len() < MAX_CACHED_INFOS {
            m.insert(did, info.clone());
        }
        info
    }

    /// Entries cached.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.map).len()
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Default)]
struct PcmCache {
    entries: HashMap<Did, (Pcm, u64)>,
    bytes: usize,
    tick: u64,
    budget: usize,
}

fn pcm_bytes(p: &Pcm) -> usize {
    p.samples.len().saturating_mul(4)
}

impl PcmCache {
    fn get(&mut self, did: Did) -> Option<Pcm> {
        self.tick += 1;
        let t = self.tick;
        self.entries.get_mut(&did).map(|(p, used)| {
            *used = t;
            p.clone()
        })
    }

    fn insert(&mut self, did: Did, pcm: Pcm) {
        let size = pcm_bytes(&pcm);
        if size > MAX_CACHED_PCM || size > self.budget || self.entries.contains_key(&did) {
            return;
        }
        while self.bytes + size > self.budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(d, _)| *d)
            else {
                break;
            };
            if let Some((p, _)) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(pcm_bytes(&p));
            }
        }
        self.tick += 1;
        self.bytes += size;
        self.entries.insert(did, (pcm, self.tick));
    }
}

/// Loader counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoaderStats {
    /// Sound infos read on the calling (main) thread because no job had prefetched them.
    pub sync_info_reads: u64,
    /// Payload loads queued on the workers.
    pub loads: u64,
    /// Requests served from the decoded-sound cache.
    pub cache_hits: u64,
    /// Loads that failed (missing, unreadable, not audio, decode error).
    pub failures: u64,
    /// Loads refused because [`MAX_WAITING_REQUESTS`] were in flight.
    pub refused: u64,
    /// Loads in flight now.
    pub in_flight: usize,
    /// Bytes of decoded sounds cached now.
    pub cached_bytes: usize,
}

#[derive(Debug, Default)]
struct Counters {
    sync_info_reads: AtomicU64,
    loads: AtomicU64,
    cache_hits: AtomicU64,
    failures: AtomicU64,
    refused: AtomicU64,
}

/// Decrements the in-flight count when a load job ends, runs or not.
struct InFlight(Arc<AtomicUsize>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The loader (module docs). Cheap to share: every part is behind an `Arc`.
#[derive(Clone)]
pub struct SoundLoader {
    set: Arc<ArchiveSet>,
    pool: WorkerPool,
    infos: Arc<InfoCache>,
    pcm: Arc<Mutex<PcmCache>>,
    policy: StreamPolicy,
    in_flight: Arc<AtomicUsize>,
    counters: Arc<Counters>,
}

impl std::fmt::Debug for SoundLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundLoader")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl SoundLoader {
    /// A loader reading `set` on `pool`'s workers, caching up to `pcm_budget` bytes of
    /// decoded short sounds.
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>, pool: WorkerPool, pcm_budget: usize) -> Self {
        Self {
            set,
            pool,
            infos: Arc::new(InfoCache::default()),
            pcm: Arc::new(Mutex::new(PcmCache {
                budget: pcm_budget,
                ..PcmCache::default()
            })),
            policy: StreamPolicy::default(),
            in_flight: Arc::new(AtomicUsize::new(0)),
            counters: Arc::new(Counters::default()),
        }
    }

    /// The archive set.
    #[must_use]
    pub fn set(&self) -> &Arc<ArchiveSet> {
        &self.set
    }

    /// The shared sound-info cache (prefetch jobs fill it).
    #[must_use]
    pub fn infos(&self) -> &Arc<InfoCache> {
        &self.infos
    }

    /// The sound info `did`: from the cache, else read now (counted as a synchronous
    /// read). `None` when missing or malformed.
    #[must_use]
    pub fn info(&self, did: Did) -> Option<Arc<SoundInfo>> {
        if let Some(e) = self.infos.get(did) {
            return e;
        }
        self.counters
            .sync_info_reads
            .fetch_add(1, Ordering::Relaxed);
        self.infos.load(&self.set, did)
    }

    /// The audio of `payload`: a cached decoded sound at once, else a load queued on the
    /// workers ([`SoundSource::Loading`]). `token` cancels the load (a request stopped
    /// while loading).
    ///
    /// # Errors
    ///
    /// When more than [`MAX_WAITING_REQUESTS`] loads are in flight
    /// ([`load_bound_reached`]) or the pool refuses the job.
    pub fn source(&self, payload: Did, token: CancelToken) -> Result<SoundSource, String> {
        if let Some(p) = lock(&self.pcm).get(payload) {
            self.counters.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(SoundSource::Pcm(p));
        }
        let in_flight = self.in_flight.load(Ordering::Acquire);
        if load_bound_reached(in_flight) {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
            return Err(format!("{in_flight} loads in flight"));
        }
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        let guard = InFlight(Arc::clone(&self.in_flight));
        let (tx, rx) = payload_channel();
        let set = Arc::clone(&self.set);
        let cache = Arc::clone(&self.pcm);
        let counters = Arc::clone(&self.counters);
        let policy = self.policy;
        self.counters.loads.fetch_add(1, Ordering::Relaxed);
        let job = move |ctx: &tessera_assets::JobContext<'_>| {
            let _guard = guard;
            if ctx.is_cancelled() {
                return;
            }
            let result = load_payload(&set, payload, &policy);
            match result {
                Ok(Payload::Pcm(p)) => {
                    lock(&cache).insert(payload, p.clone());
                    tx.send(Ok(Payload::Pcm(p)));
                }
                Ok(other) => tx.send(Ok(other)),
                Err(e) => {
                    counters.failures.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!(did = %payload, error = %e, "sound payload not loaded");
                    tx.send(Err(e));
                }
            }
        };
        self.pool
            .spawn(Priority::Visible, token, job)
            .map(|_| SoundSource::Loading(rx))
            .map_err(|e| e.to_string())
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> LoaderStats {
        let c = &self.counters;
        let pcm = lock(&self.pcm);
        LoaderStats {
            sync_info_reads: c.sync_info_reads.load(Ordering::Relaxed),
            loads: c.loads.load(Ordering::Relaxed),
            cache_hits: c.cache_hits.load(Ordering::Relaxed),
            failures: c.failures.load(Ordering::Relaxed),
            refused: c.refused.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Acquire),
            cached_bytes: pcm.bytes,
        }
    }
}

/// Reads and opens one payload record (blocking; worker threads only).
///
/// # Errors
///
/// Missing or unreadable records, decode errors, and payloads that are not audio.
pub fn load_payload(set: &ArchiveSet, did: Did, policy: &StreamPolicy) -> Result<Payload, String> {
    let bytes = fetch(set, did.0)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("payload {did} missing"))?;
    match open_record(&bytes, policy).map_err(|e| e.to_string())? {
        Playable::NotAudio(n) => Err(format!("payload {did} is not audio ({n:?})")),
        p => Payload::from_playable(p).ok_or_else(|| format!("payload {did} is not audio")),
    }
}

/// The engine request for a Sound node's `start` of sound info `info` (`sound.md` §9.2
/// steps 5–10, §12.3 step 3): the info gives the loop flag (0x01), the positional class
/// (0x04 / 0x40), the distances and the priority; the node gives the volume, pitch,
/// channel, ramp-out and whether it is tracked. A channel value outside the nine of
/// `sound.md` §7.1 plays on the Default channel.
#[must_use]
pub fn request_from(
    info: &SoundInfo,
    start: &SoundStart,
    positioner: Positioner,
    source: SoundSource,
) -> SoundRequest {
    SoundRequest {
        sound_id: Some(info.did.0),
        source,
        volume: start.volume,
        pitch: start.pitch,
        looping: info.is_loop(),
        channel: Channel::new_lossy(start.channel),
        positional: PositionalClass::from_sound_info_flags(info.flags.bits()),
        min_distance: info.min_distance,
        max_distance: info.max_distance,
        priority: info.priority,
        positioner,
        ramp_out_secs: start.ramp_out,
        tracked: start.tracked,
        limit_exempt: false,
        bypass_filter: false,
        start_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::SoundInfoFlags;
    use tessera_scriptgraph::{EntityId, SoundPositioner};

    fn info(flags: u32) -> SoundInfo {
        SoundInfo {
            did: Did(0x2A00_0010),
            flags: SoundInfoFlags(flags),
            unused_a: -1.0,
            unused_b: -1.0,
            priority: 0.7,
            min_distance: 3.0,
            max_distance: 40.0,
            payload: Did(0x0A00_0020),
            name: String::new(),
        }
    }

    fn start(channel: u32, tracked: bool) -> SoundStart {
        SoundStart {
            entity: EntityId(1),
            sound: Did(0x2A00_0010),
            volume: 0.5,
            pitch: 1.25,
            channel,
            ramp_out: 4.0,
            tracked,
            positioner: SoundPositioner::FollowEntity {
                entity: EntityId(1),
                offset: [0.0; 3],
            },
            node_id: 9,
        }
    }

    fn pcm() -> SoundSource {
        SoundSource::Pcm(Pcm {
            sample_rate: 8000,
            channels: 1,
            samples: Arc::from(vec![0.0f32; 4]),
        })
    }

    #[test]
    fn conversion_follows_the_info_and_the_node() {
        let r = request_from(&info(0x05), &start(2, true), Positioner::listener(), pcm());
        assert_eq!(r.sound_id, Some(0x2A00_0010));
        assert!(r.looping && r.tracked);
        assert_eq!(r.positional, PositionalClass::Panned);
        assert_eq!(r.channel, Channel::MUSIC);
        assert_eq!(
            (r.min_distance, r.max_distance, r.priority),
            (3.0, 40.0, 0.7)
        );
        assert_eq!((r.volume, r.pitch, r.ramp_out_secs), (0.5, 1.25, 4.0));
        assert!(!r.limit_exempt && !r.bypass_filter && r.start_at.is_none());
        let r = request_from(&info(0x40), &start(7, false), Positioner::listener(), pcm());
        assert_eq!(r.positional, PositionalClass::Attenuated);
        assert!(!r.looping && !r.tracked);
        assert_eq!(r.channel, Channel::AMBIENT);
        let r = request_from(
            &info(0x10),
            &start(99, false),
            Positioner::listener(),
            pcm(),
        );
        assert_eq!(r.positional, PositionalClass::NonPositional);
        assert_eq!(r.channel, Channel::DEFAULT);
    }

    #[test]
    fn the_load_bound_refuses_only_more_than_250() {
        assert_eq!(MAX_WAITING_REQUESTS, 250);
        assert!(!load_bound_reached(0));
        assert!(!load_bound_reached(249));
        // Exactly 250 waiting is not "more than 250": accepted, as by the engine's gate.
        assert!(!load_bound_reached(250));
        assert!(load_bound_reached(251));
    }

    #[test]
    fn pcm_cache_is_bounded() {
        let mut c = PcmCache {
            budget: 64,
            ..PcmCache::default()
        };
        let p = |n: usize| Pcm {
            sample_rate: 8000,
            channels: 1,
            samples: Arc::from(vec![0.0f32; n]),
        };
        c.insert(Did(1), p(8));
        c.insert(Did(2), p(8));
        assert_eq!(c.bytes, 64);
        assert!(c.get(Did(1)).is_some());
        // Did 2 is the least recently used: it goes.
        c.insert(Did(3), p(8));
        assert!(c.get(Did(2)).is_none());
        assert!(c.get(Did(1)).is_some() && c.get(Did(3)).is_some());
        // Too large for the budget: not kept.
        c.insert(Did(4), p(100));
        assert!(c.get(Did(4)).is_none());
        assert!(c.bytes <= 64);
    }
}
