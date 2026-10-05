// SPDX-License-Identifier: GPL-3.0-or-later
//! Generational entity IDs and their allocator.

use std::fmt;
use std::num::NonZeroU32;

use crate::error::{EntityError, Result};

/// Hard upper bound on entity slots of any store (2^24 = 16,777,216). Every [`Entity`]
/// has an index below it, so a component column's index table never exceeds
/// 64 MiB, whatever IDs a caller passes in.
pub const MAX_ENTITIES: u32 = 1 << 24;

/// A generational entity ID: a slot index plus the slot's generation when the entity was
/// spawned.
///
/// Despawning bumps the slot's generation, so an old ID never names the slot's next
/// occupant: lookups with a stale ID fail instead of reaching the wrong entity. A slot
/// whose generation would wrap is retired for good. `Option<Entity>` is 8 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity {
    index: u32,
    generation: NonZeroU32,
}

impl Entity {
    /// The slot index (below [`MAX_ENTITIES`]).
    #[must_use]
    pub fn index(self) -> u32 {
        self.index
    }

    /// The generation (at least 1).
    #[must_use]
    pub fn generation(self) -> u32 {
        self.generation.get()
    }

    /// Packs the ID as `generation << 32 | index`, for logs and foreign tables.
    #[must_use]
    pub fn to_bits(self) -> u64 {
        (u64::from(self.generation.get()) << 32) | u64::from(self.index)
    }

    /// Unpacks [`Entity::to_bits`]. `None` for generation 0 or an index at or above
    /// [`MAX_ENTITIES`]. A well-formed ID that this store never issued is simply not
    /// alive.
    #[must_use]
    pub fn from_bits(bits: u64) -> Option<Self> {
        let index = (bits & 0xFFFF_FFFF) as u32;
        let generation = NonZeroU32::new((bits >> 32) as u32)?;
        (index < MAX_ENTITIES).then_some(Self { index, generation })
    }
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    generation: NonZeroU32,
    alive: bool,
}

/// Allocates and recycles [`Entity`] IDs.
///
/// Freed slots are reused last-in first-out with their generation bumped. A slot whose
/// generation reached `u32::MAX` is retired instead of reused (after four billion reuses
/// of one slot), which keeps every ID ever issued unique.
#[derive(Debug, Clone)]
pub struct Entities {
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
    retired: usize,
    max: u32,
}

impl Entities {
    /// An allocator for at most `max` slots (clamped to [`MAX_ENTITIES`]).
    #[must_use]
    pub fn new(max: u32) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            retired: 0,
            max: max.min(MAX_ENTITIES),
        }
    }

    /// The slot limit.
    #[must_use]
    pub fn max(&self) -> u32 {
        self.max
    }

    /// Live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live
    }

    /// Whether no entity is alive.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Slots ever created (live, free and retired); one past the highest index issued.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Slots retired because their generation was exhausted.
    #[must_use]
    pub fn retired(&self) -> usize {
        self.retired
    }

    /// A new live entity.
    ///
    /// # Errors
    ///
    /// [`EntityError::Full`] when every slot below the limit is live or retired.
    pub fn alloc(&mut self) -> Result<Entity> {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.alive = true;
            self.live += 1;
            return Ok(Entity {
                index,
                generation: slot.generation,
            });
        }
        let index = u32::try_from(self.slots.len()).unwrap_or(u32::MAX);
        if index >= self.max {
            return Err(EntityError::Full { max: self.max });
        }
        self.slots.push(Slot {
            generation: NonZeroU32::MIN,
            alive: true,
        });
        self.live += 1;
        Ok(Entity {
            index,
            generation: NonZeroU32::MIN,
        })
    }

    /// Whether `e` is alive (right index and current generation).
    #[must_use]
    pub fn is_alive(&self, e: Entity) -> bool {
        self.slots
            .get(e.index as usize)
            .is_some_and(|s| s.alive && s.generation == e.generation)
    }

    /// Frees `e`'s slot. Returns false (and changes nothing) when `e` is not alive.
    pub fn free(&mut self, e: Entity) -> bool {
        if !self.is_alive(e) {
            return false;
        }
        let slot = &mut self.slots[e.index as usize];
        slot.alive = false;
        self.live -= 1;
        match slot.generation.checked_add(1) {
            Some(next) => {
                slot.generation = next;
                self.free.push(e.index);
            }
            None => self.retired += 1,
        }
        true
    }

    /// The live entity in slot `index`, if any.
    #[must_use]
    pub fn at(&self, index: u32) -> Option<Entity> {
        let s = self.slots.get(index as usize)?;
        s.alive.then_some(Entity {
            index,
            generation: s.generation,
        })
    }

    /// Live entities in slot order.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.alive.then_some(Entity {
                index: i as u32,
                generation: s.generation,
            })
        })
    }

    /// Test hook: sets the generation of a free slot so retirement can be exercised.
    #[cfg(test)]
    pub(crate) fn force_generation(&mut self, index: u32, generation: u32) {
        if let (Some(s), Some(g)) = (
            self.slots.get_mut(index as usize),
            NonZeroU32::new(generation),
        ) {
            s.generation = g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_bumps_generation() {
        let mut a = Entities::new(8);
        let e1 = a.alloc().unwrap();
        assert!(a.free(e1));
        assert!(!a.free(e1), "double free is refused");
        let e2 = a.alloc().unwrap();
        assert_eq!(e1.index(), e2.index());
        assert_eq!(e2.generation(), e1.generation() + 1);
        assert!(!a.is_alive(e1));
        assert!(a.is_alive(e2));
    }

    #[test]
    fn limit_is_enforced() {
        let mut a = Entities::new(2);
        a.alloc().unwrap();
        a.alloc().unwrap();
        assert_eq!(a.alloc(), Err(EntityError::Full { max: 2 }));
    }

    #[test]
    fn exhausted_generation_retires_slot() {
        let mut a = Entities::new(2);
        let e = a.alloc().unwrap();
        a.free(e);
        a.force_generation(e.index(), u32::MAX);
        let e = a.alloc().unwrap();
        assert_eq!(e.generation(), u32::MAX);
        assert!(a.free(e));
        assert_eq!(a.retired(), 1);
        let n = a.alloc().unwrap();
        assert_ne!(n.index(), e.index(), "retired slot is never reused");
        assert_eq!(a.alloc(), Err(EntityError::Full { max: 2 }));
    }

    #[test]
    fn bits_round_trip_and_reject_out_of_range() {
        let mut a = Entities::new(4);
        let e = a.alloc().unwrap();
        assert_eq!(Entity::from_bits(e.to_bits()), Some(e));
        assert_eq!(Entity::from_bits(0), None, "generation 0");
        assert_eq!(Entity::from_bits((1 << 32) | u64::from(MAX_ENTITIES)), None);
        let foreign = Entity::from_bits((7 << 32) | 3).unwrap();
        assert!(!a.is_alive(foreign));
    }
}
