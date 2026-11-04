// SPDX-License-Identifier: GPL-3.0-or-later
//! What a pending world entry keeps until `WorldEntered` (`official-session.md` §9.3.9 "Pending
//! world order"), and the one record of what is not sent on the way there ("World send
//! inventory").
//!
//! # The store
//!
//! After the world connection's messages have decoded to exact exhaustion under their named
//! layouts, [`PendingWorld`] keeps the latest state per entity, as complete message bodies: `0x98`,
//! `0x52`, `0x88`/`0x92` by property, `0x89`, `0xCE` and `0xCC`. On `WorldEntered` the driver takes
//! one snapshot per entity, spawn before property and motion ([`PendingWorld::take_snapshots`]),
//! and the store is empty; on a failure it is discarded. Nothing here is published, and nothing
//! acts on it before `WorldEntered`.
//!
//! The bounds are per world connection: [`MAX_PENDING_ENTITIES`] entities and
//! [`MAX_PENDING_BYTES`] of kept bodies. Past either the store answers [`Stored::Overflow`], which
//! the driver turns into the `Unsupported` result. A replaced body gives its bytes back first, so
//! one entity's repeats never overflow.
//!
//! # Targets
//!
//! A target is a nonzero ID announced by a validated `0x98` or the validated `0x90`'s own ID.
//! The logical default recipient `2` alone is no target: it is what a `0xCC` with the default form
//! carries, and which entity that means is OPEN-WORLD-9. Such a message is counted and skipped; it
//! is never read as the local player.
//!
//! # What is not sent
//!
//! [`WITHHELD_BEFORE_F4`] is the one place that names the client messages the observed client sends
//! before `0xF4` and Tessera does not. The stage line of an entry that stops carries the list
//! ([`withheld_summary`]), so a live log shows what was withheld.

use std::collections::BTreeMap;

use tessera_net::Location;

/// Most entities kept per world connection (§9.3.9).
pub const MAX_PENDING_ENTITIES: usize = 256;

/// Most bytes of kept bodies per world connection (§9.3.9).
pub const MAX_PENDING_BYTES: usize = 1024 * 1024;

/// The logical default recipient (§5.6.3): no entity by itself (module docs).
pub const DEFAULT_RECIPIENT: u64 = crate::session::DEFAULT_BLOB_RECIPIENT_ID;

/// One client message of the observed road to `0xF4` that Tessera does not send, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithheldSend {
    /// The message family.
    pub family: &'static str,
    /// What holds it: the owner's decision or the question that has no answer yet.
    pub held_by: &'static str,
}

/// The client messages withheld before `0xF4` (`official-session.md` §9.3.8, §9.3.9). The first
/// three are decision 0046's fixed omissions. The others have no specified source or trigger, and
/// no omission of them is authorized: they are not sent because nothing may be sent without a
/// source, and that is a known difference for the owner (`docs/m8-remediation.md` §26.1).
pub const WITHHELD_BEFORE_F4: [WithheldSend; 8] = [
    WithheldSend {
        family: "0x13D",
        held_by: "decision 0046 item 5, OPEN-SESSION-20",
    },
    WithheldSend {
        family: "0x19E",
        held_by: "decision 0046 item 5, OPEN-SESSION-20",
    },
    WithheldSend {
        family: "first world 0x30",
        held_by: "decision 0046 item 5, OPEN-SESSION-16",
    },
    WithheldSend {
        family: "later world 0x30",
        held_by: "OPEN-SESSION-16",
    },
    WithheldSend {
        family: "0x30000035",
        held_by: "OPEN-SESSION-16",
    },
    WithheldSend {
        family: "0x24",
        held_by: "OPEN-WORLD-9",
    },
    WithheldSend {
        family: "0x19A",
        held_by: "OPEN-SESSION-16",
    },
    WithheldSend {
        family: "0xF4",
        held_by: "OPEN-WORLD-9",
    },
];

/// The families of [`WITHHELD_BEFORE_F4`], for one log field.
#[must_use]
pub fn withheld_summary() -> String {
    WITHHELD_BEFORE_F4
        .iter()
        .map(|w| w.family)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A kept message: its opcode and complete body, its one shared byte included.
pub type KeptMessage = (u32, Vec<u8>);

/// What [`PendingWorld`] did with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// Kept as the entity's latest.
    Kept,
    /// The target is no announced entity: counted and skipped.
    NoTarget,
    /// Past a bound of the store (the `Unsupported` result).
    Overflow,
}

/// The kept state of one entity.
#[derive(Debug, Clone, Default)]
struct PendingEntity {
    create: Option<Vec<u8>>,
    table: Option<Vec<u8>>,
    /// `0x88` and `0x92`, by property.
    single: BTreeMap<u32, KeptMessage>,
    appearance: Option<Vec<u8>>,
    position: Option<Vec<u8>>,
    sync: Option<Vec<u8>>,
    /// Whether a validated message announced the ID.
    announced: bool,
    /// The latest exact placement of this entity.
    placed: Option<(Location, f32)>,
}

impl PendingEntity {
    fn bytes(&self) -> usize {
        let opt = |b: &Option<Vec<u8>>| b.as_ref().map_or(0, Vec::len);
        opt(&self.create)
            + opt(&self.table)
            + opt(&self.appearance)
            + opt(&self.position)
            + opt(&self.sync)
            + self.single.values().map(|(_, b)| b.len()).sum::<usize>()
    }
}

/// The pending world state of one world connection (module docs).
#[derive(Debug, Clone, Default)]
pub struct PendingWorld {
    entities: BTreeMap<u64, PendingEntity>,
    /// The entities in the order they were first announced.
    order: Vec<u64>,
    bytes: usize,
    player: Option<u64>,
}

/// One entity's snapshot for its release at `WorldEntered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySnapshot {
    /// The entity.
    pub entity: u64,
    /// Its kept messages, in the order they are applied: spawn, then properties, then motion.
    pub messages: Vec<KeptMessage>,
}

impl PendingWorld {
    /// How many entities are kept.
    #[must_use]
    pub fn entities(&self) -> usize {
        self.entities.len()
    }

    /// How many bytes of bodies are kept.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// The validated `0x90`'s ID: the local player, an announced target. Answers its placement when
    /// one is already kept, and [`Stored::Overflow`] past the entity bound.
    ///
    /// # Errors
    ///
    /// [`Stored::Overflow`] past [`MAX_PENDING_ENTITIES`].
    pub fn set_player(&mut self, player: u64) -> Result<Option<(Location, f32)>, Stored> {
        self.player = Some(player);
        self.announce(player)?;
        Ok(self.entities.get(&player).and_then(|e| e.placed))
    }

    /// The local player, once known.
    #[must_use]
    pub const fn player(&self) -> Option<u64> {
        self.player
    }

    fn announce(&mut self, id: u64) -> Result<(), Stored> {
        if let Some(e) = self.entities.get_mut(&id) {
            e.announced = true;
            return Ok(());
        }
        if self.entities.len() >= MAX_PENDING_ENTITIES {
            return Err(Stored::Overflow);
        }
        self.entities.insert(
            id,
            PendingEntity {
                announced: true,
                ..PendingEntity::default()
            },
        );
        self.order.push(id);
        Ok(())
    }

    /// Whether `id` can be a target (module docs).
    #[must_use]
    pub fn announced(&self, id: u64) -> bool {
        id != 0 && id != DEFAULT_RECIPIENT && self.entities.get(&id).is_some_and(|e| e.announced)
    }

    /// Runs `edit` on the announced entity `id`, keeping the byte count and its bound.
    fn edit(&mut self, id: u64, edit: impl FnOnce(&mut PendingEntity)) -> Stored {
        if !self.announced(id) {
            return Stored::NoTarget;
        }
        let Some(entity) = self.entities.get_mut(&id) else {
            return Stored::NoTarget;
        };
        let before = entity.bytes();
        let mut next = entity.clone();
        edit(&mut next);
        let after = next.bytes();
        let total = self.bytes.saturating_sub(before).saturating_add(after);
        if total > MAX_PENDING_BYTES {
            return Stored::Overflow;
        }
        *entity = next;
        self.bytes = total;
        Stored::Kept
    }

    /// A validated `0x98` for `entity`: it announces the ID and keeps its body.
    pub fn create(&mut self, entity: u64, body: &[u8]) -> Stored {
        if entity == 0 || entity == DEFAULT_RECIPIENT {
            return Stored::NoTarget;
        }
        if self.announce(entity).is_err() {
            return Stored::Overflow;
        }
        self.edit(entity, |e| e.create = Some(body.to_vec()))
    }

    /// A validated `0x52` for `target`.
    pub fn table(&mut self, target: u64, body: &[u8]) -> Stored {
        self.edit(target, |e| e.table = Some(body.to_vec()))
    }

    /// A validated `0x88` or `0x92` of `property` for `target`.
    pub fn single(&mut self, target: u64, opcode: u32, property: u32, body: &[u8]) -> Stored {
        self.edit(target, |e| {
            e.single.insert(property, (opcode, body.to_vec()));
        })
    }

    /// A validated `0x89` for `target`.
    pub fn appearance(&mut self, target: u64, body: &[u8]) -> Stored {
        self.edit(target, |e| e.appearance = Some(body.to_vec()))
    }

    /// A validated `0xCE` for `entity`, with its exact placement when it has one.
    pub fn position(&mut self, entity: u64, body: &[u8], at: Option<(Location, f32)>) -> Stored {
        self.edit(entity, |e| {
            e.position = Some(body.to_vec());
            if at.is_some() {
                e.placed = at;
            }
        })
    }

    /// A validated `0xCC` for `target`, with its exact placement when it has one.
    pub fn sync(&mut self, target: u64, body: &[u8], at: Option<(Location, f32)>) -> Stored {
        self.edit(target, |e| {
            e.sync = Some(body.to_vec());
            if at.is_some() {
                e.placed = at;
            }
        })
    }

    /// One latest snapshot per entity, in the order they were announced, every entity's spawn
    /// before any property or motion of another (§9.3.9). The store is empty afterwards.
    pub fn take_snapshots(&mut self) -> (Vec<EntitySnapshot>, Vec<EntitySnapshot>) {
        let mut spawns = Vec::new();
        let mut states = Vec::new();
        let entities = std::mem::take(&mut self.entities);
        let order = std::mem::take(&mut self.order);
        self.bytes = 0;
        self.player = None;
        let mut entities = entities;
        for id in order {
            let Some(e) = entities.remove(&id) else {
                continue;
            };
            if let Some(body) = e.create {
                spawns.push(EntitySnapshot {
                    entity: id,
                    messages: vec![(OPCODE_CREATE, body)],
                });
            }
            let mut messages = Vec::new();
            if let Some(body) = e.table {
                messages.push((OPCODE_TABLE, body));
            }
            messages.extend(e.single.into_values());
            if let Some(body) = e.appearance {
                messages.push((OPCODE_APPEARANCE, body));
            }
            if let Some(body) = e.position {
                messages.push((OPCODE_POSITION, body));
            }
            if let Some(body) = e.sync {
                messages.push((OPCODE_SYNC, body));
            }
            if !messages.is_empty() {
                states.push(EntitySnapshot {
                    entity: id,
                    messages,
                });
            }
        }
        (spawns, states)
    }

    /// Discards everything (a failure: §9.3.9).
    pub fn discard(&mut self) {
        *self = Self::default();
    }
}

const OPCODE_CREATE: u32 = 0x98;
const OPCODE_TABLE: u32 = 0x52;
const OPCODE_APPEARANCE: u32 = 0x89;
const OPCODE_POSITION: u32 = 0xCE;
const OPCODE_SYNC: u32 = 0xCC;

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::RegionId;

    fn at(x: f32) -> (Location, f32) {
        (
            Location {
                region: RegionId(1),
                x: f64::from(x),
                y: 2.0,
                z: 3.0,
            },
            0.5,
        )
    }

    #[test]
    fn the_default_recipient_is_no_target() {
        let mut p = PendingWorld::default();
        assert_eq!(p.table(DEFAULT_RECIPIENT, &[0]), Stored::NoTarget);
        assert_eq!(p.create(DEFAULT_RECIPIENT, &[0]), Stored::NoTarget);
        assert_eq!(p.create(0, &[0]), Stored::NoTarget);
        assert_eq!(
            p.sync(DEFAULT_RECIPIENT, &[0], Some(at(1.0))),
            Stored::NoTarget
        );
        assert_eq!(p.entities(), 0);
    }

    #[test]
    fn an_unannounced_target_is_skipped_and_an_announced_one_kept() {
        let mut p = PendingWorld::default();
        assert_eq!(p.table(7, &[0, 1]), Stored::NoTarget);
        assert_eq!(p.create(7, &[0, 2]), Stored::Kept);
        assert_eq!(p.table(7, &[0, 1]), Stored::Kept);
        assert_eq!(p.bytes(), 4);
    }

    #[test]
    fn the_player_is_a_target_and_its_kept_placement_is_answered() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(9, &[0]), Stored::Kept);
        assert_eq!(p.position(9, &[0, 1], Some(at(4.0))), Stored::Kept);
        assert_eq!(p.set_player(9), Ok(Some(at(4.0))));
        assert_eq!(p.player(), Some(9));
        let mut q = PendingWorld::default();
        assert_eq!(q.set_player(9), Ok(None));
        assert_eq!(q.sync(9, &[0], Some(at(5.0))), Stored::Kept);
    }

    #[test]
    fn the_latest_state_replaces_and_gives_its_bytes_back() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(7, &[0; 10]), Stored::Kept);
        assert_eq!(p.appearance(7, &[0; 100]), Stored::Kept);
        assert_eq!(p.bytes(), 110);
        assert_eq!(p.appearance(7, &[1; 5]), Stored::Kept);
        assert_eq!(p.bytes(), 15);
        assert_eq!(p.single(7, 0x88, 1, &[0; 3]), Stored::Kept);
        assert_eq!(p.single(7, 0x92, 1, &[0; 4]), Stored::Kept);
        assert_eq!(p.bytes(), 19);
    }

    #[test]
    fn the_bounds_overflow_and_a_replacement_still_fits() {
        let mut p = PendingWorld::default();
        for id in 0..MAX_PENDING_ENTITIES as u64 {
            assert_eq!(p.create(id + 10, &[0]), Stored::Kept);
        }
        assert_eq!(p.create(5_000, &[0]), Stored::Overflow);
        assert_eq!(p.create(10, &[0, 0]), Stored::Kept);
        assert_eq!(p.entities(), MAX_PENDING_ENTITIES);

        let mut q = PendingWorld::default();
        assert_eq!(q.create(7, &[0; 1024]), Stored::Kept);
        assert_eq!(
            q.appearance(7, &vec![0; MAX_PENDING_BYTES]),
            Stored::Overflow
        );
        assert_eq!(q.bytes(), 1024, "a refused body changes nothing");
        assert_eq!(
            q.appearance(7, &vec![0; MAX_PENDING_BYTES - 1024]),
            Stored::Kept
        );
        assert_eq!(q.appearance(7, &[0; 1]), Stored::Kept);
        assert_eq!(q.bytes(), 1025);
    }

    #[test]
    fn snapshots_release_every_spawn_first_and_empty_the_store() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(7, &[1]), Stored::Kept);
        assert_eq!(p.create(8, &[2]), Stored::Kept);
        assert_eq!(p.sync(7, &[3], None), Stored::Kept);
        assert_eq!(p.position(7, &[4], None), Stored::Kept);
        assert_eq!(p.appearance(7, &[5],), Stored::Kept);
        assert_eq!(p.single(7, 0x92, 9, &[6]), Stored::Kept);
        assert_eq!(p.table(7, &[7]), Stored::Kept);
        let (spawns, states) = p.take_snapshots();
        let ids: Vec<u64> = spawns.iter().map(|s| s.entity).collect();
        assert_eq!(ids, [7, 8]);
        assert_eq!(states.len(), 1);
        let ops: Vec<u32> = states[0].messages.iter().map(|m| m.0).collect();
        assert_eq!(ops, [0x52, 0x92, 0x89, 0xCE, 0xCC]);
        assert_eq!(p.entities(), 0);
        assert_eq!(p.bytes(), 0);
        assert_eq!(p.player(), None);
    }

    #[test]
    fn the_withheld_record_names_every_family_of_the_inventory() {
        let s = withheld_summary();
        for family in [
            "0x13D",
            "0x19E",
            "first world 0x30",
            "later world 0x30",
            "0x30000035",
            "0x24",
            "0x19A",
            "0xF4",
        ] {
            assert!(s.contains(family), "{s}");
        }
    }
}
