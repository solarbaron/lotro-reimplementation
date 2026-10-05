// SPDX-License-Identifier: GPL-3.0-or-later
//! [`Column`]: one component type stored as a sparse set.

use std::fmt;

use crate::entity::Entity;

const ABSENT: u32 = u32::MAX;

/// Components of one type, keyed by [`Entity`]: a sparse set.
///
/// Values sit in a dense vector (iteration touches only present components, in a tight
/// loop); an index table maps an entity's slot index to its dense position. Insert,
/// lookup and remove are O(1); removal swaps the last value into the hole, so dense
/// order is not stable. The index table grows to the highest slot index inserted, which
/// is below [`MAX_ENTITIES`](crate::MAX_ENTITIES) for every [`Entity`].
///
/// [`EntityStore`](crate::EntityStore) uses one column per component set. Callers can
/// keep their own side tables as `Column<T>` and clean them with the store's despawn
/// list ([`Changes::despawned`](crate::Changes::despawned)). [`Column::insert`] refuses
/// an ID older than the slot's current value, but the column cannot tell a dead ID from
/// a live one when the slot is empty: insert values that come from events (late network
/// updates, asset completions) only after
/// [`EntityStore::is_alive`](crate::EntityStore::is_alive) says the entity is alive.
#[derive(Clone)]
pub struct Column<T> {
    sparse: Vec<u32>,
    entities: Vec<Entity>,
    values: Vec<T>,
}

impl<T> Default for Column<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Column<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Column")
            .field("len", &self.values.len())
            .field("index_table", &self.sparse.len())
            .finish()
    }
}

impl<T> Column<T> {
    /// An empty column.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the column holds no value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn dense_of(&self, e: Entity) -> Option<usize> {
        let d = *self.sparse.get(e.index() as usize)?;
        if d == ABSENT {
            return None;
        }
        let d = d as usize;
        (self.entities.get(d) == Some(&e)).then_some(d)
    }

    /// Stores `value` for `e`.
    ///
    /// Returns `Ok` with the value previously stored in `e`'s slot: `e`'s own, or one
    /// left under an **older** generation of the same slot (a despawned occupant whose
    /// value was never removed), which is replaced. Returns `Err(value)`, leaving the
    /// column unchanged, when the slot holds a value of a **newer** generation: `e` is
    /// stale, and its value must not displace the live occupant's (a late event for an
    /// entity that has since been despawned and its slot reused). The caller gets the
    /// value back to release whatever it owns.
    ///
    /// Generations of one slot only grow (an exhausted slot is retired, never wrapped),
    /// so the comparison is exact for IDs of one [`Entities`](crate::Entities)
    /// allocator. The column does not know which entities are alive: an ID from another
    /// allocator, or one forged with [`Entity::from_bits`], is only compared by
    /// generation; check [`EntityStore::is_alive`](crate::EntityStore::is_alive) first
    /// when IDs come from untrusted sources.
    ///
    /// # Errors
    ///
    /// `Err(value)` when `e` is older than the slot's current value (see above).
    pub fn insert(&mut self, e: Entity, value: T) -> Result<Option<T>, T> {
        let i = e.index() as usize;
        if i >= self.sparse.len() {
            self.sparse.resize(i + 1, ABSENT);
        }
        let d = self.sparse[i];
        if d != ABSENT {
            let d = d as usize;
            // `entities` and `sparse` are kept in step; never index blindly all the same.
            let (Some(held), Some(slot)) = (self.entities.get_mut(d), self.values.get_mut(d))
            else {
                return Err(value);
            };
            if held.generation() > e.generation() {
                tracing::debug!(stale = %e, live = %held, "column insert with a stale ID refused");
                return Err(value);
            }
            *held = e;
            return Ok(Some(std::mem::replace(slot, value)));
        }
        // The dense length is bounded by the number of distinct slot indices, which is
        // below MAX_ENTITIES (2^24), so it fits in u32 and never equals ABSENT.
        self.sparse[i] = self.values.len() as u32;
        self.entities.push(e);
        self.values.push(value);
        Ok(None)
    }

    /// Removes and returns `e`'s value. `None` when absent or when the slot holds another
    /// generation's value (left untouched).
    pub fn remove(&mut self, e: Entity) -> Option<T> {
        let d = self.dense_of(e)?;
        self.sparse[e.index() as usize] = ABSENT;
        self.entities.swap_remove(d);
        let v = self.values.swap_remove(d);
        if let Some(moved) = self.entities.get(d) {
            self.sparse[moved.index() as usize] = d as u32;
        }
        Some(v)
    }

    /// Whether `e` has a value.
    #[must_use]
    pub fn contains(&self, e: Entity) -> bool {
        self.dense_of(e).is_some()
    }

    /// `e`'s value.
    #[must_use]
    pub fn get(&self, e: Entity) -> Option<&T> {
        self.dense_of(e).map(|d| &self.values[d])
    }

    /// `e`'s value, mutable.
    #[must_use]
    pub fn get_mut(&mut self, e: Entity) -> Option<&mut T> {
        self.dense_of(e).map(|d| &mut self.values[d])
    }

    /// Entities with a value, in dense order (parallel to [`Column::values`]).
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Values in dense order (parallel to [`Column::entities`]).
    #[must_use]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Values in dense order, mutable.
    #[must_use]
    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    /// (entity, value) pairs in dense order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (Entity, &T)> + '_ {
        self.entities.iter().copied().zip(self.values.iter())
    }

    /// (entity, value) pairs in dense order, values mutable.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (Entity, &mut T)> + '_ {
        self.entities.iter().copied().zip(self.values.iter_mut())
    }

    /// Removes every value, keeping allocations. The removed values are returned in dense
    /// order.
    pub fn drain(&mut self) -> impl Iterator<Item = (Entity, T)> + '_ {
        for e in &self.entities {
            self.sparse[e.index() as usize] = ABSENT;
        }
        self.entities.drain(..).zip(self.values.drain(..))
    }

    /// Removes every value, keeping allocations.
    pub fn clear(&mut self) {
        self.drain().for_each(drop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entities;

    #[test]
    fn insert_remove_swaps_and_keeps_index_consistent() {
        let mut ids = Entities::new(16);
        let es: Vec<Entity> = (0..5).map(|_| ids.alloc().unwrap()).collect();
        let mut c = Column::new();
        for (i, e) in es.iter().enumerate() {
            assert_eq!(c.insert(*e, i), Ok(None));
        }
        assert_eq!(c.remove(es[1]), Some(1));
        assert_eq!(c.remove(es[1]), None);
        for (i, e) in es.iter().enumerate() {
            if i != 1 {
                assert_eq!(c.get(*e), Some(&i));
            }
        }
        assert_eq!(c.len(), 4);
        assert_eq!(c.insert(es[0], 10), Ok(Some(0)));
        assert_eq!(c.get(es[0]), Some(&10));
    }

    #[test]
    fn stale_generation_does_not_reach_new_value() {
        let mut ids = Entities::new(4);
        let old = ids.alloc().unwrap();
        let mut c = Column::new();
        c.insert(old, "old").unwrap();
        ids.free(old);
        let new = ids.alloc().unwrap();
        assert_eq!(new.index(), old.index());
        // The new occupant's insert replaces what the old generation left behind.
        assert_eq!(c.insert(new, "new"), Ok(Some("old")));
        assert_eq!(c.get(old), None);
        assert_eq!(c.remove(old), None);
        assert_eq!(c.get(new), Some(&"new"));
    }

    #[test]
    fn stale_insert_never_displaces_newer_value() {
        // Regression: an insert under a stale ID used to replace the live occupant's
        // value and rebind the slot to the dead ID.
        let mut ids = Entities::new(4);
        let old = ids.alloc().unwrap();
        ids.free(old);
        let live = ids.alloc().unwrap();
        assert_eq!(live.index(), old.index());
        let mut c = Column::new();
        assert_eq!(c.insert(live, "live"), Ok(None));
        assert_eq!(c.insert(old, "stale"), Err("stale"), "value handed back");
        assert_eq!(c.get(live), Some(&"live"));
        assert!(c.contains(live));
        assert!(!c.contains(old));
        assert_eq!(c.get(old), None);
        assert_eq!(c.len(), 1);
        assert_eq!(c.entities(), &[live]);
        // The live occupant can still replace its own value.
        assert_eq!(c.insert(live, "again"), Ok(Some("live")));
        assert_eq!(c.remove(live), Some("again"));
        assert!(c.is_empty());
    }

    #[test]
    fn drain_resets_index() {
        let mut ids = Entities::new(4);
        let a = ids.alloc().unwrap();
        let b = ids.alloc().unwrap();
        let mut c = Column::new();
        c.insert(a, 1).unwrap();
        c.insert(b, 2).unwrap();
        let drained: Vec<_> = c.drain().collect();
        assert_eq!(drained, vec![(a, 1), (b, 2)]);
        assert!(c.is_empty() && !c.contains(a));
        c.insert(b, 3).unwrap();
        assert_eq!(c.get(b), Some(&3));
    }
}
