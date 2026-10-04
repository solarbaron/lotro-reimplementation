// SPDX-License-Identifier: GPL-3.0-or-later
//! A least-recently-used map with a byte budget.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::Hash;

struct Slot<V> {
    value: V,
    size: usize,
    tick: u64,
}

/// A map that keeps the sum of its entries' sizes within a byte budget by evicting the
/// least recently used entries.
///
/// Sizes are whatever the caller says they are (for decoded assets,
/// [`Decoder::size_of`](crate::Decoder::size_of)). An entry larger than the whole budget
/// is never stored. Lookups and inserts are `O(log n)`.
pub struct LruCache<K, V> {
    map: HashMap<K, Slot<V>>,
    /// Recency order: tick → key. The smallest tick is the least recently used.
    order: BTreeMap<u64, K>,
    tick: u64,
    bytes: usize,
    budget: usize,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// An empty cache holding at most `budget` bytes.
    #[must_use]
    pub fn new(budget: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: BTreeMap::new(),
            tick: 0,
            bytes: 0,
            budget,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Looks `key` up and marks it most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let tick = self.next_tick();
        let slot = self.map.get_mut(key)?;
        self.order.remove(&slot.tick);
        slot.tick = tick;
        self.order.insert(tick, key.clone());
        Some(&slot.value)
    }

    /// Looks `key` up without changing its recency.
    #[must_use]
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|s| &s.value)
    }

    /// Whether `key` is cached (recency unchanged).
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Stores `value` under `key` as the most recently used entry, evicting least recently
    /// used entries until the budget holds.
    ///
    /// Returns the evicted entries (including a replaced value under the same key). If
    /// `size` exceeds the whole budget nothing is stored and `value` comes back as
    /// `Err(value)`; an older entry under `key` is removed in that case too, so the cache
    /// never serves a stale value.
    pub fn insert(&mut self, key: K, value: V, size: usize) -> Result<Vec<(K, V)>, V> {
        let mut evicted = Vec::new();
        if let Some(old) = self.remove(&key) {
            evicted.push((key.clone(), old));
        }
        if size > self.budget {
            return Err(value);
        }
        // `bytes <= budget` always holds, so after evicting enough the sum cannot overflow;
        // `checked_add` keeps the loop honest even for sizes near `usize::MAX`.
        while self
            .bytes
            .checked_add(size)
            .is_none_or(|total| total > self.budget)
        {
            match self.pop_lru() {
                Some(e) => evicted.push(e),
                None => break,
            }
        }
        let tick = self.next_tick();
        self.order.insert(tick, key.clone());
        self.map.insert(key, Slot { value, size, tick });
        self.bytes = self.bytes.saturating_add(size);
        Ok(evicted)
    }

    /// Removes `key`, returning its value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let slot = self.map.remove(key)?;
        self.order.remove(&slot.tick);
        self.bytes = self.bytes.saturating_sub(slot.size);
        Some(slot.value)
    }

    /// Removes and returns the least recently used entry.
    pub fn pop_lru(&mut self) -> Option<(K, V)> {
        let (_, key) = self.order.pop_first()?;
        let slot = self.map.remove(&key)?;
        self.bytes = self.bytes.saturating_sub(slot.size);
        Some((key, slot.value))
    }

    /// Changes the budget, evicting least recently used entries if needed.
    pub fn set_budget(&mut self, budget: usize) -> Vec<(K, V)> {
        self.budget = budget;
        let mut evicted = Vec::new();
        while self.bytes > self.budget {
            match self.pop_lru() {
                Some(e) => evicted.push(e),
                None => break,
            }
        }
        evicted
    }

    /// Removes every entry, returning them least recently used first.
    pub fn clear(&mut self) -> Vec<(K, V)> {
        let mut out = Vec::with_capacity(self.map.len());
        while let Some(e) = self.pop_lru() {
            out.push(e);
        }
        out
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Sum of the stored entries' sizes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Keys from least to most recently used.
    pub fn keys_lru(&self) -> impl Iterator<Item = &K> {
        self.order.values()
    }
}

impl<K, V> fmt::Debug for LruCache<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LruCache")
            .field("entries", &self.map.len())
            .field("bytes", &self.bytes)
            .field("budget", &self.budget)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut c = LruCache::new(100);
        assert!(c.insert(1, "a", 40).unwrap().is_empty());
        assert!(c.insert(2, "b", 40).unwrap().is_empty());
        assert_eq!(c.get(&1), Some(&"a")); // 2 is now the LRU
        let ev = c.insert(3, "c", 40).unwrap();
        assert_eq!(ev, vec![(2, "b")]);
        assert_eq!(c.bytes(), 80);
        assert_eq!(c.keys_lru().copied().collect::<Vec<_>>(), vec![1, 3]);
        assert!(c.peek(&2).is_none());
    }

    #[test]
    fn oversized_entry_is_rejected_and_drops_stale_value() {
        let mut c = LruCache::new(10);
        c.insert(1, 1, 5).unwrap();
        assert_eq!(c.insert(1, 2, 11), Err(2));
        assert!(!c.contains(&1));
        assert_eq!(c.bytes(), 0);
        let mut zero = LruCache::new(0);
        assert_eq!(zero.insert(1, 1, 1), Err(1));
        assert!(zero.insert(2, 2, 0).is_ok());
    }

    #[test]
    fn replace_reports_old_value_and_keeps_accounting() {
        let mut c = LruCache::new(10);
        c.insert(1, "old", 4).unwrap();
        let ev = c.insert(1, "new", 6).unwrap();
        assert_eq!(ev, vec![(1, "old")]);
        assert_eq!(c.bytes(), 6);
        assert_eq!(c.len(), 1);
    }

    /// Regression: sizes near `usize::MAX` with an "unlimited" budget used to overflow the
    /// byte counter (panic in debug, corrupt accounting in release).
    #[test]
    fn huge_sizes_do_not_overflow() {
        let mut c = LruCache::new(usize::MAX);
        c.insert(1, (), 1).unwrap();
        let ev = c.insert(2, (), usize::MAX).unwrap();
        assert_eq!(ev, vec![(1, ())]);
        assert_eq!(c.bytes(), usize::MAX);
        let ev = c.insert(3, (), 7).unwrap();
        assert_eq!(ev, vec![(2, ())]);
        assert_eq!(c.bytes(), 7);
        assert_eq!(c.remove(&3), Some(()));
        assert_eq!(c.bytes(), 0);
    }

    #[test]
    fn budget_shrink_and_clear() {
        let mut c = LruCache::new(100);
        for k in 0..10 {
            c.insert(k, k, 10).unwrap();
        }
        let ev = c.set_budget(35);
        assert_eq!(
            ev.iter().map(|e| e.0).collect::<Vec<_>>(),
            (0..7).collect::<Vec<_>>()
        );
        assert_eq!(c.bytes(), 30);
        assert_eq!(c.clear().len(), 3);
        assert!(c.is_empty() && c.bytes() == 0);
        assert_eq!(c.remove(&5), None);
    }
}
