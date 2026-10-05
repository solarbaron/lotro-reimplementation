// SPDX-License-Identifier: GPL-3.0-or-later
//! Where string-table records come from, and the per-DID table cache.
//!
//! Tables live only in the selected language archive (`docs/specs/string-table.md` §10.1).
//! A reader builds a hash map per table on load and caches tables by DID (§9.2).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tessera_dat::Archive;
use tessera_formats::{Record, StringTable};

use crate::error::{Result, TextError};

/// DID range of string tables (prefix 0x25, `records.md` §3).
pub const STRING_TABLE_DIDS: std::ops::Range<u32> = 0x2500_0000..0x2600_0000;

/// Whether a DID has the string-table prefix 0x25.
#[must_use]
pub fn is_string_table_did(did: u32) -> bool {
    STRING_TABLE_DIDS.contains(&did)
}

/// A read-only provider of string-table record payloads.
pub trait TableSource: Send + Sync {
    /// The payload of record `did`, `Ok(None)` if the source has no such record.
    fn read_table(&self, did: u32) -> Result<Option<Vec<u8>>>;

    /// Every string-table DID the source holds, ascending.
    fn table_dids(&self) -> Result<Vec<u32>>;
}

impl TableSource for Archive {
    fn read_table(&self, did: u32) -> Result<Option<Vec<u8>>> {
        match self.lookup(did)? {
            None => Ok(None),
            Some(entry) => Ok(Some(self.read_entry(&entry)?)),
        }
    }

    fn table_dids(&self) -> Result<Vec<u32>> {
        let mut out = Vec::new();
        for entry in self.entries_range(STRING_TABLE_DIDS) {
            out.push(entry?.did);
        }
        Ok(out)
    }
}

impl<T: TableSource + ?Sized> TableSource for Arc<T> {
    fn read_table(&self, did: u32) -> Result<Option<Vec<u8>>> {
        (**self).read_table(did)
    }

    fn table_dids(&self) -> Result<Vec<u32>> {
        (**self).table_dids()
    }
}

impl<T: TableSource + ?Sized> TableSource for Box<T> {
    fn read_table(&self, did: u32) -> Result<Option<Vec<u8>>> {
        (**self).read_table(did)
    }

    fn table_dids(&self) -> Result<Vec<u32>> {
        (**self).table_dids()
    }
}

/// An in-memory source (synthetic fixtures, tools).
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    records: HashMap<u32, Vec<u8>>,
}

impl MemorySource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a record payload.
    pub fn insert(&mut self, did: u32, payload: Vec<u8>) -> &mut Self {
        self.records.insert(did, payload);
        self
    }
}

impl TableSource for MemorySource {
    fn read_table(&self, did: u32) -> Result<Option<Vec<u8>>> {
        Ok(self.records.get(&did).cloned())
    }

    fn table_dids(&self) -> Result<Vec<u32>> {
        let mut v: Vec<u32> = self
            .records
            .keys()
            .copied()
            .filter(|&d| is_string_table_did(d))
            .collect();
        v.sort_unstable();
        Ok(v)
    }
}

/// Cache limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    /// Maximum number of cached tables (including remembered misses).
    pub max_tables: usize,
    /// Maximum total payload bytes of cached tables (an approximation of memory use).
    pub max_bytes: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_tables: 16_384,
            max_bytes: 64 << 20,
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Lookups answered from the cache.
    pub hits: u64,
    /// Lookups that loaded from the source.
    pub misses: u64,
    /// Tables currently cached.
    pub tables: usize,
    /// Payload bytes currently cached.
    pub bytes: usize,
}

#[derive(Debug)]
struct Cached {
    /// The table, `Ok(None)` for a remembered absence, `Err` for a remembered read or
    /// decode failure (its message).
    table: std::result::Result<Option<Arc<StringTable>>, Arc<str>>,
    bytes: usize,
    last_used: u64,
}

#[derive(Debug, Default)]
struct CacheInner {
    map: HashMap<u32, Cached>,
    tick: u64,
    bytes: usize,
    hits: u64,
    misses: u64,
}

/// A thread-safe LRU-ish cache of decoded tables keyed by DID. Absent tables are
/// remembered too, and so are read and decode failures: a broken record is loaded once,
/// later lookups return [`TextError::CachedFailure`] until [`TableCache::clear`] (hostile
/// references to one large corrupt record must not force a full re-read each time).
///
/// A table whose payload alone exceeds [`CacheLimits::max_bytes`] is returned but not
/// cached, so loading it never flushes the other tables; a newly loaded table is never the
/// one its own insertion evicts.
#[derive(Debug, Default)]
pub struct TableCache {
    inner: Mutex<CacheInner>,
    limits: CacheLimits,
}

impl TableCache {
    /// A cache with the given limits.
    #[must_use]
    pub fn new(limits: CacheLimits) -> Self {
        Self {
            inner: Mutex::default(),
            limits,
        }
    }

    /// The limits in effect.
    #[must_use]
    pub fn limits(&self) -> CacheLimits {
        self.limits
    }

    fn lock(&self) -> MutexGuard<'_, CacheInner> {
        // A panic while holding the lock cannot leave the map inconsistent in a way that
        // matters for a cache, so a poisoned lock is simply reused.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The cached table for `did`, loading and decoding it from `source` on a miss.
    pub fn get_or_load(
        &self,
        did: u32,
        source: &dyn TableSource,
    ) -> Result<Option<Arc<StringTable>>> {
        self.get_or_load_counted(did, source).0
    }

    /// The cached entry for `did` without loading anything: `None` if it is not cached.
    /// A found entry counts as a hit.
    pub(crate) fn get_cached(&self, did: u32) -> Option<Result<Option<Arc<StringTable>>>> {
        let mut g = self.lock();
        g.tick += 1;
        let tick = g.tick;
        let c = g.map.get_mut(&did)?;
        c.last_used = tick;
        let t = c.table.clone();
        g.hits += 1;
        Some(t.map_err(|reason| TextError::CachedFailure {
            did,
            reason: reason.to_string(),
        }))
    }

    /// [`TableCache::get_or_load`], also returning the number of payload bytes read from
    /// `source` (0 when the cache answered).
    pub(crate) fn get_or_load_counted(
        &self,
        did: u32,
        source: &dyn TableSource,
    ) -> (Result<Option<Arc<StringTable>>>, usize) {
        if let Some(t) = self.get_cached(did) {
            return (t, 0);
        }
        self.lock().misses += 1;
        // Load outside the lock so other threads are not blocked on I/O.
        let loaded = source.read_table(did).and_then(|payload| match payload {
            None => Ok((None, 0)),
            Some(payload) => {
                let t = StringTable::parse_expecting(&payload, did)
                    .map_err(|source| TextError::Decode { did, source })?;
                Ok((Some(Arc::new(t)), payload.len()))
            }
        });
        let (table, bytes, result) = match loaded {
            Ok((t, bytes)) => (Ok(t.clone()), bytes, Ok(t)),
            Err(e) => {
                tracing::debug!(did = format_args!("{did:#010x}"), error = %e, "string table failed to load; failure cached");
                (Err(Arc::from(e.to_string())), 0, Err(e))
            }
        };
        if bytes > self.limits.max_bytes || self.limits.max_tables == 0 {
            // Caching it would evict every other table and then this one too.
            tracing::debug!(
                did = format_args!("{did:#010x}"),
                bytes,
                limit = self.limits.max_bytes,
                "string table larger than the cache; not cached"
            );
            return (result, bytes);
        }
        let mut g = self.lock();
        g.tick += 1;
        let tick = g.tick;
        if let Some(old) = g.map.insert(
            did,
            Cached {
                table,
                bytes,
                last_used: tick,
            },
        ) {
            g.bytes -= old.bytes;
        }
        g.bytes += bytes;
        self.evict(&mut g, did);
        (result, bytes)
    }

    /// Evicts least recently used tables, never `keep` (the table just inserted), until the
    /// cache is at or below half of each limit, so eviction cost is amortised.
    fn evict(&self, g: &mut CacheInner, keep: u32) {
        if g.map.len() <= self.limits.max_tables && g.bytes <= self.limits.max_bytes {
            return;
        }
        let mut order: Vec<(u64, u32)> = g
            .map
            .iter()
            .filter(|&(&d, _)| d != keep)
            .map(|(&d, c)| (c.last_used, d))
            .collect();
        order.sort_unstable();
        let target_tables = self.limits.max_tables / 2;
        let target_bytes = self.limits.max_bytes / 2;
        for (_, did) in order {
            if g.map.len() <= target_tables && g.bytes <= target_bytes {
                break;
            }
            if let Some(c) = g.map.remove(&did) {
                g.bytes -= c.bytes;
            }
        }
    }

    /// Drops every cached table.
    pub fn clear(&self) {
        let mut g = self.lock();
        g.map.clear();
        g.bytes = 0;
    }

    /// Current statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let g = self.lock();
        CacheStats {
            hits: g.hits,
            misses: g.misses,
            tables: g.map.len(),
            bytes: g.bytes,
        }
    }
}
