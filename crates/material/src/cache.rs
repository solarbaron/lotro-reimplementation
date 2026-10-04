// SPDX-License-Identifier: GPL-3.0-or-later
//! A small bounded cache: an entry count and a byte budget; when either would be exceeded
//! it is emptied. Every cache of the crate (records, textures, icon layers) uses it.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug)]
struct Inner<K, V> {
    map: HashMap<K, (V, usize)>,
    bytes: usize,
}

/// A thread-safe map with an entry and byte limit.
#[derive(Debug)]
pub(crate) struct BoundedCache<K, V> {
    inner: Mutex<Inner<K, V>>,
    max_entries: usize,
    max_bytes: usize,
}

impl<K: Hash + Eq, V: Clone> BoundedCache<K, V> {
    pub(crate) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                bytes: 0,
            }),
            max_entries,
            max_bytes,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner<K, V>> {
        // A panic while holding the lock cannot leave the map inconsistent beyond a
        // missing entry, so a poisoned lock is still usable.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        self.lock().map.get(key).map(|(v, _)| v.clone())
    }

    /// Inserts `value` taking `size` bytes. Values above the whole budget are not kept.
    pub(crate) fn insert(&self, key: K, value: V, size: usize) {
        if self.max_entries == 0 || size > self.max_bytes {
            return;
        }
        let mut inner = self.lock();
        // A key that is already present is replaced: its old size is released first and
        // it does not count as a new entry, so two threads inserting the same key after
        // a shared miss never flush the other entries.
        let replaced = match inner.map.remove(&key) {
            Some((_, old)) => {
                inner.bytes = inner.bytes.saturating_sub(old);
                true
            }
            None => false,
        };
        let full = !replaced && inner.map.len() >= self.max_entries;
        if full || inner.bytes.saturating_add(size) > self.max_bytes {
            inner.map.clear();
            inner.bytes = 0;
        }
        inner.map.insert(key, (value, size));
        inner.bytes = inner.bytes.saturating_add(size);
    }

    /// (entries, bytes).
    pub(crate) fn usage(&self) -> (usize, usize) {
        let inner = self.lock();
        (inner.map.len(), inner.bytes)
    }

    pub(crate) fn clear(&self) {
        let mut inner = self.lock();
        inner.map.clear();
        inner.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits() {
        let c = BoundedCache::new(2, 100);
        c.insert(1, 'a', 10);
        c.insert(2, 'b', 10);
        assert_eq!(c.usage(), (2, 20));
        c.insert(3, 'c', 10); // over the count: emptied first
        assert_eq!(c.usage(), (1, 10));
        assert_eq!(c.get(&3), Some('c'));
        c.insert(4, 'd', 101); // above the budget: not kept
        assert_eq!(c.get(&4), None);
        c.insert(3, 'e', 30); // replace
        assert_eq!(c.usage(), (1, 30));
        c.clear();
        assert_eq!(c.usage(), (0, 0));
    }

    #[test]
    fn reinserting_a_present_key_keeps_the_others() {
        // Full by entry count: re-inserting an existing key replaces it in place.
        let c = BoundedCache::new(3, 100);
        for k in 0..3 {
            c.insert(k, k, 10);
        }
        c.insert(1, 1, 10);
        assert_eq!(c.usage(), (3, 30));
        assert_eq!((c.get(&0), c.get(&2)), (Some(0), Some(2)));
        // Full by bytes: the old size is released before the budget check.
        let c = BoundedCache::new(10, 30);
        for k in 0..3 {
            c.insert(k, k, 10);
        }
        c.insert(2, 7, 10);
        assert_eq!(c.usage(), (3, 30));
        assert_eq!((c.get(&0), c.get(&2)), (Some(0), Some(7)));
        // A larger replacement that no longer fits still empties the cache.
        c.insert(2, 8, 20);
        assert_eq!(c.usage(), (1, 20));
        assert_eq!(c.get(&2), Some(8));
    }
}
