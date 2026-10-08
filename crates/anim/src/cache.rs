// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-clip decompression caches bounded by byte budgets.
//!
//! Two tiers, both keyed by the caller (normally the clip's DID):
//!
//! - **decoded keys**: [`ClipDecoder`] plugs clip records into a `tessera-assets`
//!   [`AssetStore`](tessera_assets::AssetStore), which decodes them on the worker pool
//!   (record → dequantised spline blocks or interleaved frames) and keeps the products in
//!   its LRU cache under [`StoreConfig::cache_budget`](tessera_assets::StoreConfig) bytes
//!   (sized by [`clip_heap_size`]); [`SkeletonDecoder`] does the same for setups;
//! - **sampled keys**: [`BakeCache`] keeps [`BakedClip`]s (every frame sampled once) under
//!   its own byte budget, so crowds interpolate stored frames instead of evaluating curves.

use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::{AssetKey, Decoder, JobContext, LruCache};
use tessera_havok::{Clip, HavokError};

use crate::clip::{BakedClip, clip_heap_size};
use crate::error::AnimError;
use crate::skeleton::Skeleton;

/// Decodes 0x05 clip records on the asset workers (`havok-data.md` §7.2).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClipDecoder;

impl Decoder for ClipDecoder {
    type Output = Clip;
    type Error = HavokError;
    const NAME: &'static str = "havok-clip";

    fn decode(
        &self,
        _key: AssetKey,
        bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        Clip::parse(&bytes)
    }

    fn size_of(&self, value: &Self::Output) -> usize {
        clip_heap_size(value)
    }
}

/// Decodes the skeleton of 0x04 setup records into a posable [`Skeleton`] on the asset
/// workers.
#[derive(Debug, Clone, Copy, Default)]
pub struct SkeletonDecoder;

impl Decoder for SkeletonDecoder {
    type Output = Skeleton;
    type Error = AnimError;
    const NAME: &'static str = "anim-skeleton";

    fn decode(
        &self,
        _key: AssetKey,
        bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        Skeleton::from_setup_record(&bytes)
    }

    fn size_of(&self, value: &Self::Output) -> usize {
        value.heap_size()
    }
}

/// Counters of a [`BakeCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BakeStats {
    /// Lookups answered from the cache.
    pub hits: u64,
    /// Clips baked.
    pub bakes: u64,
    /// Clips refused: larger than the per-clip limit (which never exceeds the current
    /// budget), or not admitted by the cache.
    pub refused: u64,
    /// Bytes held.
    pub bytes: usize,
    /// Entries held.
    pub entries: usize,
}

/// An LRU cache of [`BakedClip`]s bounded by a byte budget. Thread-safe; baking runs on the
/// calling thread without the lock held, so call [`BakeCache::get_or_bake`] from a worker
/// task, not the frame loop.
#[derive(Debug)]
pub struct BakeCache {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    lru: LruCache<u64, Arc<BakedClip>>,
    stats: BakeStats,
    /// The per-clip limit given at construction.
    max_clip_bytes: usize,
}

impl Inner {
    /// The per-clip limit in force: never above the current budget, so a clip that could
    /// not be kept is never baked.
    fn clip_limit(&self) -> usize {
        self.max_clip_bytes.min(self.lru.budget())
    }
}

impl BakeCache {
    /// A cache holding at most `budget` bytes of baked samples; a single clip larger than
    /// `max_clip_bytes` (or than the current budget, see [`BakeCache::set_budget`]) is never
    /// baked.
    #[must_use]
    pub fn new(budget: usize, max_clip_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                lru: LruCache::new(budget),
                stats: BakeStats::default(),
                max_clip_bytes,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The baked clip for `key`, if cached.
    #[must_use]
    pub fn get(&self, key: u64) -> Option<Arc<BakedClip>> {
        let mut g = self.lock();
        let r = g.lru.get(&key).cloned();
        if r.is_some() {
            g.stats.hits += 1;
        }
        r
    }

    /// The baked clip for `key`, baking `clip` if it is not cached. `None` when the clip is
    /// too large to bake under the per-clip limit or the current budget (play it through
    /// curve evaluation then); such a clip is refused before any sampling work.
    #[must_use]
    pub fn get_or_bake(&self, key: u64, clip: &Clip) -> Option<Arc<BakedClip>> {
        let limit = {
            let mut g = self.lock();
            if let Some(b) = g.lru.get(&key).cloned() {
                g.stats.hits += 1;
                return Some(b);
            }
            g.clip_limit()
        };
        let Some(baked) = BakedClip::bake(clip, limit) else {
            self.lock().stats.refused += 1;
            return None;
        };
        let baked = Arc::new(baked);
        let size = baked.heap_size();
        let mut g = self.lock();
        // Evicted entries are dropped after the lock is released (they are only Arcs).
        match g.lru.insert(key, Arc::clone(&baked), size) {
            Ok(evicted) => {
                g.stats.bakes += 1;
                drop(g);
                drop(evicted);
            }
            Err(_) => {
                // The budget shrank while baking: hand this one out, keep nothing.
                g.stats.refused += 1;
            }
        }
        Some(baked)
    }

    /// Changes the byte budget, evicting as needed. The per-clip limit follows: clips
    /// larger than the new budget are refused without being baked.
    pub fn set_budget(&self, budget: usize) {
        let evicted = self.lock().lru.set_budget(budget);
        drop(evicted);
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> BakeStats {
        let g = self.lock();
        BakeStats {
            bytes: g.lru.bytes(),
            entries: g.lru.len(),
            ..g.stats
        }
    }
}
