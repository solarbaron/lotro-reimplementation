// SPDX-License-Identifier: GPL-3.0-or-later
//! What a pending world entry keeps until `WorldEntered` (`official-session.md` §9.3.9 "Pending
//! world order"), and the one record of what is not sent on the way there ("World send
//! inventory").
//!
//! # The store
//!
//! After the world connection's messages have decoded to exact exhaustion under their named
//! layouts, [`PendingWorld`] keeps the latest state per target key, as encoded data: `0x98`, the
//! `0x52` entries merged by property, `0x88`/`0x92` by property in arrival order, `0x89`, `0xCE` and
//! `0xCC`. On `WorldEntered` the driver takes one snapshot per resolved entity, spawn before merged
//! properties, appearance and motion ([`PendingWorld::take_snapshots`]), and the store is empty; on a
//! failure it is discarded. Nothing here is published, and nothing acts on it before `WorldEntered`.
//!
//! The bounds are per world connection: [`MAX_PENDING_ENTITIES`] target keys and
//! [`MAX_PENDING_BYTES`] of kept encoded data. Past either a new entry or its growth is counted and
//! skipped ([`Stored::KeyCap`], [`Stored::ByteCap`]), and what is kept stays: an overflow never ends
//! a healthy entry. The local identity, the first `0x90`'s encoded property table and the placement
//! live apart from these bounds, with priority (`set_player`, `keep_session_table`; the driver keeps
//! the placement). That one table is bounded by the session layer's largest message and kept once. A
//! replaced value gives its bytes back first, so one entity's repeats never overflow.
//!
//! A `0x40` deletes its key's spawn, properties and motion and leaves a record there, which refuses
//! every later `0x52`, `0x88`, `0x92`, `0x89`, `0xCE` and `0xCC` of that key, whichever field named
//! the key ([`Stored::Despawned`]); only a new validated `0x98` starts a new incarnation
//! ([`PendingWorld::despawn`]). The record takes a key slot, and at the cap a `0x40` of a new key is
//! counted and skipped.
//!
//! # Targets
//!
//! Data whose target nothing has announced is kept under its raw logical recipient within the
//! bounds, the default recipient `2` included: which entity that means is OPEN-WORLD-9, and `2`
//! never becomes the `0x90`'s ID by a numerical substitution. A key is resolved by a validated `0x98`
//! or as the validated `0x90`'s own ID. What is still unresolved at the release is discarded, not
//! guessed into a player.
//!
//! `0xCE`'s key is its RAW logical recipient, like every other message's, until its
//! controlled-target resolver is sourced (§9.3.9, §9.3.12 Question 1): its leading eight-byte word is
//! never the key, and a parsed conditional selector is kept as metadata only ([`PendingWorld::position`]).
//! A despawn has no public event at the release, and only an explicit `0x40` to a resolved target
//! deletes: a `0x40` to the default recipient is counted and skipped and deletes nothing
//! ([`DEFAULT_RECIPIENT_DESPAWN_APPLIES`]).
//!
//! Labelled until the writer answers (§9.3.9, WORLD-9): the `0x90`'s own ID counts as resolved with no
//! `0x98`, so its kept state is released at `WorldEntered`; an explicit `0x40` to an unresolved ID, or
//! to the `0x90`'s ID, is counted and skipped and deletes nothing ([`PendingWorld::despawn_applies`]);
//! a later `0x90` that does not read is counted and skipped.
//!
//! # What is not sent
//!
//! [`WITHHELD_BEFORE_F4`] is the one place that names the client messages the observed client sends
//! before `0xF4` and Tessera does not. The stage line of an entry that stops carries the list
//! ([`withheld_summary`]), so a live log shows what was withheld.

use std::collections::BTreeMap;

use tessera_net::Location;

use crate::world::PropertyTableUpdate;

/// Most target keys kept per world connection (§9.3.9).
pub const MAX_PENDING_ENTITIES: usize = 256;

/// Most bytes of kept encoded data per world connection (§9.3.9).
pub const MAX_PENDING_BYTES: usize = 1024 * 1024;

/// The logical default recipient (§5.6.3): no entity by itself (module docs).
pub const DEFAULT_RECIPIENT: u64 = crate::session::DEFAULT_BLOB_RECIPIENT_ID;

/// Whether a `0x40` to the default recipient deletes and blocks key `2`. Labelled
/// (`docs/m8-remediation.md` §28 fix 21): read literally, one such `0x40` would delete everything under
/// the key and refuse every later default-recipient message, for an entity nothing has resolved. So
/// it is counted and skipped until the writer gives its one action.
pub const DEFAULT_RECIPIENT_DESPAWN_APPLIES: bool = false;

/// One client message of the observed road to `0xF4` that Tessera does not send, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithheldSend {
    /// The message family.
    pub family: &'static str,
    /// What holds it: the owner's decision or the question that has no answer yet.
    pub held_by: &'static str,
}

/// The client messages withheld before `0xF4` (`official-session.md` §9.3.8, §9.3.9 "World send
/// inventory"). The first three are decision 0046 item 5's fixed omissions, the next four its item
/// 10's, which §9.3.9 cites: they stay unsent for any user past the warning and consent gate until a
/// source for their contents exists. All are labelled known differences from the observed client, the
/// server's reaction is untested, and the run ends at `UnsupportedTimeout`. `0xF4` itself has no source
/// yet (OPEN-WORLD-9).
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
        held_by: "decision 0046 item 10, OPEN-SESSION-16",
    },
    WithheldSend {
        family: "0x30000035",
        held_by: "decision 0046 item 10, OPEN-SESSION-16",
    },
    WithheldSend {
        family: "0x24",
        held_by: "decision 0046 item 10, OPEN-WORLD-9",
    },
    WithheldSend {
        family: "0x19A",
        held_by: "decision 0046 item 10, OPEN-SESSION-16",
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
    /// Kept as its key's latest.
    Kept,
    /// A `0x98` for the ID `0` or `2`, which names no entity: counted and skipped.
    NoTarget,
    /// Its key has a despawn record: counted and skipped, no resurrection.
    Despawned,
    /// A new key past [`MAX_PENDING_ENTITIES`]: counted and skipped, nothing else changes.
    KeyCap,
    /// Past [`MAX_PENDING_BYTES`]: counted and skipped, nothing else changes.
    ByteCap,
}

/// The kept state of one target key. Deliberately not `Clone`: a store update touches one field, and
/// one batch can hold many thousands of them on the connector's thread, so an update never copies
/// what it does not change.
#[derive(Debug, Default)]
struct PendingEntity {
    create: Option<Vec<u8>>,
    /// The newest encoded update per property: a `0x52` entry's encoded bytes (`opcode` `0x52`) or a
    /// complete `0x88` or `0x92` body.
    properties: BTreeMap<u32, KeptProperty>,
    appearance: Option<Vec<u8>>,
    position: Option<Vec<u8>>,
    sync: Option<Vec<u8>>,
    /// Whether a validated `0x98` announced the ID.
    announced: bool,
    /// A `0x40` deleted this key: only a new `0x98` takes the record away.
    despawned: bool,
    /// The latest exact placement of this key, kept with its motion.
    placed: Option<(Location, f32)>,
    /// The conditional selector of the latest kept `0xCE`, when its form has one: metadata only,
    /// never a key and never a guessed parent or player ID (§9.3.9, §9.3.12 Question 1).
    selector: Option<u64>,
    /// The next arrival number of a property of this key.
    next_arrival: u64,
}

impl PendingEntity {
    /// The bytes this key keeps.
    fn held(&self) -> usize {
        held(&self.create)
            + held(&self.appearance)
            + held(&self.position)
            + held(&self.sync)
            + self
                .properties
                .values()
                .map(|p| p.encoded.len())
                .sum::<usize>()
    }
}

/// The newest encoded update of one property.
#[derive(Debug)]
struct KeptProperty {
    /// `0x52` for an entry of a table; else the single update's opcode.
    opcode: u32,
    encoded: Vec<u8>,
    /// Its order among the properties of its key.
    arrival: u64,
}

/// The pending world state of one world connection (module docs).
#[derive(Debug, Default)]
pub struct PendingWorld {
    entities: BTreeMap<u64, PendingEntity>,
    /// The keys in the order they were first kept.
    order: Vec<u64>,
    bytes: usize,
    player: Option<u64>,
    /// The first `0x90`'s encoded table: required local state (module docs).
    session_table: Option<Vec<u8>>,
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
    /// How many target keys are kept.
    #[must_use]
    pub fn entities(&self) -> usize {
        self.entities.len()
    }

    /// How many bytes of encoded data are kept.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// The validated `0x90`'s ID: the local player. It is required state, held apart from the
    /// bounds, so a full store cannot lose it. Answers its placement when one is already kept.
    pub fn set_player(&mut self, player: u64) -> Option<(Location, f32)> {
        self.player = Some(player);
        self.entities.get(&player).and_then(|e| e.placed)
    }

    /// Keeps the first validated `0x90`'s encoded property table (§9.3.9) until the release. A later
    /// one changes nothing: the first stays (labelled, `docs/m8-remediation.md` §28 fix 20).
    pub fn keep_session_table(&mut self, table: &[u8]) {
        if self.session_table.is_none() {
            self.session_table = Some(table.to_vec());
        }
    }

    /// How many bytes the kept `0x90` table holds, outside [`Self::bytes`].
    #[must_use]
    pub fn session_table_bytes(&self) -> usize {
        held(&self.session_table)
    }

    /// Takes the kept `0x90` table, for the release after every spawn.
    pub fn take_session_table(&mut self) -> Option<Vec<u8>> {
        self.session_table.take()
    }

    /// The local player, once known.
    #[must_use]
    pub const fn player(&self) -> Option<u64> {
        self.player
    }

    /// Whether `id` is resolved at this point: a validated `0x98` announced it, or it is the
    /// `0x90`'s own ID (labelled, module docs).
    #[must_use]
    pub fn resolved(&self, id: u64) -> bool {
        if id == 0 || id == DEFAULT_RECIPIENT {
            return false;
        }
        self.player == Some(id)
            || self
                .entities
                .get(&id)
                .is_some_and(|e| e.announced && !e.despawned)
    }

    /// Whether a `0x40` to `target` deletes (§9.3.9): only an explicit target that is resolved. The
    /// default recipient never is; labelled until the writer answers: an unresolved ID or the
    /// `0x90`'s own ID deletes nothing either (module docs).
    #[must_use]
    /// A key with a despawn record answers `true` too, so a repeat reaches the record and is counted
    /// as blocked ([`Stored::Despawned`]), deleting nothing more.
    pub fn despawn_applies(&self, target: u64) -> bool {
        target != DEFAULT_RECIPIENT
            && self.player != Some(target)
            && (self.resolved(target) || self.despawned(target))
    }

    /// The conditional selector kept with `key`'s latest `0xCE`, if any (metadata only).
    #[must_use]
    pub fn selector(&self, key: u64) -> Option<u64> {
        self.entities.get(&key).and_then(|e| e.selector)
    }

    /// Whether `key` has a despawn record.
    fn despawned(&self, key: u64) -> bool {
        self.entities.get(&key).is_some_and(|e| e.despawned)
    }

    /// A slot for `key`, new when it has none and the bound allows it.
    fn slot(&mut self, key: u64) -> Result<&mut PendingEntity, Stored> {
        if !self.entities.contains_key(&key) {
            if self.entities.len() >= MAX_PENDING_ENTITIES {
                return Err(Stored::KeyCap);
            }
            self.order.push(key);
        }
        Ok(self.entities.entry(key).or_default())
    }

    /// Takes a key slot back when nothing went into it.
    fn free_if_empty(&mut self, key: u64) {
        if self
            .entities
            .get(&key)
            .is_some_and(|e| !e.announced && !e.despawned && e.held() == 0 && e.placed.is_none())
        {
            self.entities.remove(&key);
            self.order.retain(|k| *k != key);
        }
    }

    /// Puts `new` in one field of `key` (`slot` says what it holds, `put` fills it), keeping the byte
    /// count and its bound: the old value's bytes come back first, a body that does not fit changes
    /// nothing, and the cost is that of the one field. A despawn record refuses it.
    fn store(
        &mut self,
        key: u64,
        new: &[u8],
        slot: impl FnOnce(&PendingEntity) -> usize,
        put: impl FnOnce(&mut PendingEntity, Vec<u8>),
    ) -> Stored {
        if self.despawned(key) {
            return Stored::Despawned;
        }
        let bytes = self.bytes;
        let entity = match self.slot(key) {
            Ok(entity) => entity,
            Err(stored) => return stored,
        };
        let total = bytes.saturating_sub(slot(entity)).saturating_add(new.len());
        if total > MAX_PENDING_BYTES {
            self.free_if_empty(key);
            return Stored::ByteCap;
        }
        put(entity, new.to_vec());
        self.bytes = total;
        Stored::Kept
    }

    /// A validated `0x98` for `entity`: it announces the ID, keeps its body and ends a despawn record:
    /// a new incarnation, with nothing of the old one. One that does not fit changes nothing, so a
    /// despawn record stays and later updates of the key stay refused (§9.3.9): only a `0x98` that
    /// is kept starts a new incarnation.
    pub fn create(&mut self, entity: u64, body: &[u8]) -> Stored {
        if entity == 0 || entity == DEFAULT_RECIPIENT {
            return Stored::NoTarget;
        }
        if self.despawned(entity) {
            // A record holds no bytes, and its key has its slot.
            if self.bytes.saturating_add(body.len()) > MAX_PENDING_BYTES {
                return Stored::ByteCap;
            }
            if let Some(e) = self.entities.get_mut(&entity) {
                e.despawned = false;
            }
        }
        let stored = self.store(entity, body, |e| held(&e.create), |e, b| e.create = Some(b));
        if stored == Stored::Kept
            && let Some(e) = self.entities.get_mut(&entity)
        {
            e.announced = true;
        }
        stored
    }

    /// A validated `0x52` for `target`, as its entries' encoded bytes by property: merged by
    /// property key, each newest update kept, never a whole table replaced (§9.3.9). All of it fits
    /// or nothing of it changes, so a table is never half-merged. A later table takes the place of
    /// an older single of the properties it carries.
    pub fn table(&mut self, target: u64, entries: &[(u32, &[u8])]) -> Stored {
        if self.despawned(target) {
            return Stored::Despawned;
        }
        let bytes = self.bytes;
        let entity = match self.slot(target) {
            Ok(entity) => entity,
            Err(stored) => return stored,
        };
        let total = merged_size(entity, entries, bytes);
        if total > MAX_PENDING_BYTES {
            self.free_if_empty(target);
            return Stored::ByteCap;
        }
        for (property, encoded) in entries {
            let arrival = entity.next_arrival;
            entity.next_arrival = arrival.saturating_add(1);
            entity.properties.insert(
                *property,
                KeptProperty {
                    opcode: OPCODE_TABLE,
                    encoded: encoded.to_vec(),
                    arrival,
                },
            );
        }
        self.bytes = total;
        self.free_if_empty(target);
        Stored::Kept
    }

    /// A validated `0x40` for `target` (§9.3.9): a record stays, and the key's spawn, properties and
    /// motion are gone, so nothing of it is released and nothing but a new `0x98` brings it back. The
    /// local identity stays. Answers what happened: at the key cap, a new key is counted and skipped.
    pub fn despawn(&mut self, target: u64) -> Stored {
        if self.despawned(target) {
            return Stored::Despawned;
        }
        let entity = match self.slot(target) {
            Ok(entity) => entity,
            Err(stored) => return stored,
        };
        let kept = entity.held();
        *entity = PendingEntity {
            despawned: true,
            ..PendingEntity::default()
        };
        self.bytes = self.bytes.saturating_sub(kept);
        Stored::Kept
    }

    /// A validated `0x88` or `0x92` of `property` for `target`.
    pub fn single(&mut self, target: u64, opcode: u32, property: u32, body: &[u8]) -> Stored {
        self.store(
            target,
            body,
            |e| e.properties.get(&property).map_or(0, |p| p.encoded.len()),
            |e, b| {
                let arrival = e.next_arrival;
                e.next_arrival = arrival.saturating_add(1);
                e.properties.insert(
                    property,
                    KeptProperty {
                        opcode,
                        encoded: b,
                        arrival,
                    },
                );
            },
        )
    }

    /// A validated `0x89` for `target`.
    pub fn appearance(&mut self, target: u64, body: &[u8]) -> Stored {
        self.store(
            target,
            body,
            |e| held(&e.appearance),
            |e, b| e.appearance = Some(b),
        )
    }

    /// A validated `0xCE` under its RAW logical recipient `key`, with its exact placement when it has
    /// one and its parsed conditional `selector` as metadata (§9.3.9: never the leading eight-byte
    /// word, never a guessed parent or player ID).
    pub fn position(
        &mut self,
        key: u64,
        body: &[u8],
        at: Option<(Location, f32)>,
        selector: Option<u64>,
    ) -> Stored {
        self.store(
            key,
            body,
            |e| held(&e.position),
            |e, b| {
                e.position = Some(b);
                e.selector = selector;
                if at.is_some() {
                    e.placed = at;
                }
            },
        )
    }

    /// A validated `0xCC` for `target`, with its exact placement when it has one.
    pub fn sync(&mut self, target: u64, body: &[u8], at: Option<(Location, f32)>) -> Stored {
        self.store(
            target,
            body,
            |e| held(&e.sync),
            |e, b| {
                e.sync = Some(b);
                if at.is_some() {
                    e.placed = at;
                }
            },
        )
    }

    /// One latest snapshot per resolved entity, in the order their keys were first kept, every
    /// entity's spawn before any property or motion of another (§9.3.9). Unresolved keys and despawn
    /// records are discarded with no event. The store is empty afterwards.
    pub fn take_snapshots(&mut self) -> (Vec<EntitySnapshot>, Vec<EntitySnapshot>) {
        let resolved: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| self.resolved(*id))
            .collect();
        let mut spawns = Vec::new();
        let mut states = Vec::new();
        let mut entities = std::mem::take(&mut self.entities);
        self.order.clear();
        self.bytes = 0;
        self.player = None;
        self.session_table = None;
        for id in resolved {
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
            let mut properties: Vec<KeptProperty> = e.properties.into_values().collect();
            properties.sort_by_key(|p| p.arrival);
            let table: Vec<&[u8]> = properties
                .iter()
                .filter(|p| p.opcode == OPCODE_TABLE)
                .map(|p| p.encoded.as_slice())
                .collect();
            // Its entries read to exact exhaustion when they were kept, so the one body holds them.
            if !table.is_empty()
                && let Ok(body) = PropertyTableUpdate::body_of_entries(&table)
            {
                messages.push((OPCODE_TABLE, body));
            }
            messages.extend(
                properties
                    .into_iter()
                    .filter(|p| p.opcode != OPCODE_TABLE)
                    .map(|p| (p.opcode, p.encoded)),
            );
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

/// The store's byte count once `entries` are merged into `entity`: each property counts once, its
/// old bytes coming back first.
fn merged_size(entity: &PendingEntity, entries: &[(u32, &[u8])], bytes: usize) -> usize {
    let mut seen: BTreeMap<u32, usize> = BTreeMap::new();
    for (property, encoded) in entries {
        seen.insert(*property, encoded.len());
    }
    seen.iter().fold(bytes, |total, (property, len)| {
        let old = entity
            .properties
            .get(property)
            .map_or(0, |p| p.encoded.len());
        total.saturating_sub(old).saturating_add(*len)
    })
}

/// The bytes one kept body holds.
fn held(body: &Option<Vec<u8>>) -> usize {
    body.as_ref().map_or(0, Vec::len)
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

    /// §9.3.9: logical 2 never names an entity by itself, and a `0x98` cannot make it one. Its data is
    /// kept under its raw key and discarded at the release.
    #[test]
    fn the_default_recipient_is_kept_raw_and_never_released() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(DEFAULT_RECIPIENT, &[0]), Stored::NoTarget);
        assert_eq!(p.create(0, &[0]), Stored::NoTarget);
        assert_eq!(p.table(DEFAULT_RECIPIENT, &[(1, &[9])]), Stored::Kept);
        assert_eq!(p.sync(DEFAULT_RECIPIENT, &[0], Some(at(1.0))), Stored::Kept);
        assert_eq!(p.entities(), 1);
        assert_eq!(p.set_player(7), None, "2 is not the player's ID");
        assert!(!p.resolved(DEFAULT_RECIPIENT));
        let (spawns, states) = p.take_snapshots();
        assert!(spawns.is_empty() && states.is_empty());
        assert_eq!(p.bytes(), 0);
    }

    #[test]
    fn an_unannounced_target_is_kept_and_resolved_by_a_later_creation() {
        let mut p = PendingWorld::default();
        assert_eq!(p.appearance(7, &[0, 1]), Stored::Kept);
        assert!(!p.resolved(7));
        assert_eq!(p.appearance(8, &[0, 3]), Stored::Kept);
        assert_eq!(p.create(7, &[0, 2]), Stored::Kept);
        assert!(p.resolved(7));
        assert_eq!(p.bytes(), 6);
        let (spawns, states) = p.take_snapshots();
        assert_eq!(spawns.iter().map(|s| s.entity).collect::<Vec<_>>(), [7]);
        assert_eq!(states.iter().map(|s| s.entity).collect::<Vec<_>>(), [7]);
    }

    #[test]
    fn the_player_is_required_state_and_its_kept_placement_is_answered() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(9, &[0]), Stored::Kept);
        assert_eq!(p.position(9, &[0, 1], Some(at(4.0)), None), Stored::Kept);
        assert_eq!(p.set_player(9), Some(at(4.0)));
        assert_eq!(p.player(), Some(9));
        let mut q = PendingWorld::default();
        assert_eq!(q.set_player(9), None);
        assert_eq!(q.sync(9, &[0], Some(at(5.0))), Stored::Kept);
        assert!(q.resolved(9), "the 0x90's own ID");
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

    /// §9.3.9: past a cap a new entry or growth is counted and skipped and what is kept stays.
    #[test]
    fn the_caps_skip_and_keep_what_is_there() {
        let mut p = PendingWorld::default();
        for id in 0..MAX_PENDING_ENTITIES as u64 {
            assert_eq!(p.create(id + 10, &[0]), Stored::Kept);
        }
        assert_eq!(p.create(5_000, &[0]), Stored::KeyCap);
        assert_eq!(p.single(5_000, 0x88, 1, &[0]), Stored::KeyCap);
        assert_eq!(p.despawn(5_000), Stored::KeyCap);
        assert_eq!(
            p.create(10, &[0, 0]),
            Stored::Kept,
            "a kept key still updates"
        );
        assert_eq!(p.entities(), MAX_PENDING_ENTITIES);
        assert_eq!(
            p.set_player(5_001),
            None,
            "the local identity is not bounded"
        );
        assert_eq!(p.player(), Some(5_001));
        assert!(p.resolved(5_001));

        let mut q = PendingWorld::default();
        assert_eq!(q.create(7, &[0; 1024]), Stored::Kept);
        assert_eq!(
            q.appearance(7, &vec![0; MAX_PENDING_BYTES]),
            Stored::ByteCap
        );
        assert_eq!(q.bytes(), 1024, "a refused body changes nothing");
        assert_eq!(
            q.appearance(9, &vec![0; MAX_PENDING_BYTES]),
            Stored::ByteCap
        );
        assert_eq!(q.entities(), 1, "a refused new key takes no slot");
        assert_eq!(
            q.appearance(7, &vec![0; MAX_PENDING_BYTES - 1024]),
            Stored::Kept
        );
        assert_eq!(q.appearance(7, &[0; 1]), Stored::Kept);
        assert_eq!(q.bytes(), 1025);
    }

    /// One entity near the bound and a batch's worth of small updates: each costs its own slot,
    /// and the count stays exact. (`PendingEntity` is not `Clone`, so no update can copy the rest.)
    #[test]
    fn a_heavy_entity_keeps_its_count_exact_through_many_small_updates() {
        let mut p = PendingWorld::default();
        let heavy = MAX_PENDING_BYTES - 200_000;
        assert_eq!(p.create(7, &vec![0; heavy]), Stored::Kept);
        for i in 0..200_000u32 {
            assert_eq!(p.single(7, 0x88, i % 50_000, &[1, 2]), Stored::Kept);
        }
        assert_eq!(p.bytes(), heavy + 100_000);
        // Replacing a property with a longer value gives the old bytes back first.
        assert_eq!(p.single(7, 0x88, 0, &[1, 2, 3]), Stored::Kept);
        assert_eq!(p.bytes(), heavy + 100_001);
        let room = MAX_PENDING_BYTES - p.bytes();
        assert_eq!(p.single(7, 0x88, 0, &vec![0; room + 4]), Stored::ByteCap);
        assert_eq!(p.bytes(), heavy + 100_001, "a refused body changes nothing");
        assert_eq!(p.single(7, 0x88, 0, &vec![0; room + 3]), Stored::Kept);
        assert_eq!(p.bytes(), MAX_PENDING_BYTES);
        // The kept placement stays with a refused body.
        assert_eq!(p.position(7, &[], Some(at(1.0)), None), Stored::Kept);
        assert_eq!(
            p.position(7, &[0; 16], Some(at(2.0)), None),
            Stored::ByteCap
        );
        assert_eq!(p.set_player(7), Some(at(1.0)));
    }

    #[test]
    fn snapshots_release_every_spawn_first_and_empty_the_store() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(7, &[1]), Stored::Kept);
        assert_eq!(p.create(8, &[2]), Stored::Kept);
        assert_eq!(p.sync(7, &[3], None), Stored::Kept);
        assert_eq!(p.position(7, &[4], None, None), Stored::Kept);
        assert_eq!(p.appearance(7, &[5],), Stored::Kept);
        assert_eq!(p.single(7, 0x92, 9, &[6]), Stored::Kept);
        assert_eq!(p.table(7, &[(1, &[7])]), Stored::Kept);
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

    /// §9.3.9: `0x52` tables merge by property, never one replacing another whole; the newest update
    /// per property stays, and `0x88`/`0x92` overlay in arrival order.
    #[test]
    fn tables_and_singles_merge_by_property() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(7, &[0]), Stored::Kept);
        assert_eq!(p.table(7, &[(1, &[0xA1]), (2, &[0xA2])]), Stored::Kept);
        assert_eq!(p.bytes(), 3);
        assert_eq!(p.single(7, 0x88, 3, &[1, 1]), Stored::Kept);
        assert_eq!(p.single(7, 0x92, 4, &[2, 2, 2]), Stored::Kept);
        assert_eq!(p.bytes(), 8);
        // A second table with one property keeps the first table's other, and takes the single's
        // place for property 3; property 4's single stays over the table.
        assert_eq!(
            p.table(7, &[(2, &[0xB2, 0xB2]), (3, &[0xB3])]),
            Stored::Kept
        );
        assert_eq!(p.bytes(), 1 + 1 + 2 + 1 + 3);
        assert_eq!(p.single(7, 0x88, 1, &[4]), Stored::Kept);
        let (_, states) = p.take_snapshots();
        let messages = &states[0].messages;
        assert_eq!(
            messages[0],
            (0x52, vec![0, 2, 0xB2, 0xB2, 0xB3]),
            "properties 2 and 3 in one table body"
        );
        assert_eq!(messages[1], (0x92, vec![2, 2, 2]));
        assert_eq!(messages[2], (0x88, vec![4]));
        assert_eq!(messages.len(), 3);
    }

    /// A table that would pass the byte cap changes nothing, and one that replaces its own bytes fits.
    #[test]
    fn a_table_is_merged_whole_or_not_at_all() {
        let mut p = PendingWorld::default();
        let big = vec![0; MAX_PENDING_BYTES];
        assert_eq!(p.table(7, &[(1, &big)]), Stored::Kept);
        assert_eq!(p.table(7, &[(1, &big)]), Stored::Kept, "replaces its own");
        assert_eq!(p.table(7, &[(1, &[1]), (2, &big)]), Stored::ByteCap);
        assert_eq!(p.bytes(), MAX_PENDING_BYTES);
        assert_eq!(p.table(8, &[]), Stored::Kept);
        assert_eq!(p.entities(), 1, "an empty table takes no key");
    }

    /// §9.3.9: `0x40` takes the key's data and leaves a record: later updates do not bring it back,
    /// whichever field names the key; only a new creation does, with nothing of the old incarnation.
    #[test]
    fn a_despawned_key_stays_gone_until_it_is_created_again() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(7, &[0; 4]), Stored::Kept);
        assert_eq!(p.create(8, &[0; 2]), Stored::Kept);
        assert_eq!(p.appearance(7, &[0; 8]), Stored::Kept);
        assert_eq!(p.single(7, 0x88, 1, &[0; 16]), Stored::Kept);
        assert_eq!(p.despawn(7), Stored::Kept);
        assert_eq!(p.despawn(7), Stored::Despawned, "a repeat changes nothing");
        assert_eq!(p.bytes(), 2);
        assert!(!p.resolved(7));
        for stored in [
            p.table(7, &[(1, &[0])]),
            p.single(7, 0x88, 1, &[0]),
            p.appearance(7, &[0]),
            p.position(7, &[0], Some(at(1.0)), None),
            p.sync(7, &[0], Some(at(1.0))),
        ] {
            assert_eq!(stored, Stored::Despawned);
        }
        let (spawns, states) = p.take_snapshots();
        assert_eq!(spawns.iter().map(|s| s.entity).collect::<Vec<_>>(), [8]);
        assert!(states.is_empty());

        // A despawn of a key with no data yet blocks too.
        let mut r = PendingWorld::default();
        assert_eq!(r.despawn(9), Stored::Kept);
        assert_eq!(r.appearance(9, &[1]), Stored::Despawned);

        let mut q = PendingWorld::default();
        assert_eq!(q.create(7, &[0]), Stored::Kept);
        assert_eq!(q.appearance(7, &[0; 3]), Stored::Kept);
        assert_eq!(q.despawn(7), Stored::Kept);
        assert_eq!(q.create(7, &[1]), Stored::Kept);
        assert_eq!(q.bytes(), 1, "nothing of the old incarnation");
        assert_eq!(q.appearance(7, &[2]), Stored::Kept);
        let (spawns, states) = q.take_snapshots();
        assert_eq!(spawns.len(), 1);
        assert_eq!(states.len(), 1);
    }

    /// §9.3.9: a `0x98` refused at the byte cap is no new incarnation: the record stays, later updates
    /// stay refused, and nothing of them reaches the `0x98` that fits.
    #[test]
    fn a_creation_refused_at_the_byte_cap_keeps_the_despawn_record() {
        let mut p = PendingWorld::default();
        assert_eq!(p.create(1, &vec![0; MAX_PENDING_BYTES - 16]), Stored::Kept);
        assert_eq!(p.create(7, &[0; 8]), Stored::Kept);
        assert_eq!(p.despawn(7), Stored::Kept);
        let (entities, bytes) = (p.entities(), p.bytes());
        assert_eq!(p.create(7, &[0; 64]), Stored::ByteCap);
        assert_eq!((p.entities(), p.bytes()), (entities, bytes), "no change");
        assert_eq!(p.appearance(7, &[0; 2]), Stored::Despawned);
        assert_eq!(
            p.position(7, &[0; 2], Some(at(1.0)), None),
            Stored::Despawned
        );
        assert_eq!(p.single(7, 0x88, 1, &[0; 2]), Stored::Despawned);
        assert_eq!(p.create(7, &[0; 4]), Stored::Kept, "one that fits");
        assert_eq!(p.bytes(), bytes + 4, "nothing of the refused updates");
        let (spawns, states) = p.take_snapshots();
        assert_eq!(spawns.iter().map(|s| s.entity).collect::<Vec<_>>(), [1, 7]);
        assert!(states.is_empty(), "{states:?}");
    }

    /// §9.3.9: the first `0x90`'s table is required state: it stays outside both bounds, a repeat does
    /// not replace it, and a failure discards it.
    #[test]
    fn the_session_table_is_kept_once_and_outside_the_bounds() {
        let mut p = PendingWorld::default();
        for id in 0..MAX_PENDING_ENTITIES as u64 {
            assert_eq!(p.create(id + 10, &[0]), Stored::Kept);
        }
        assert_eq!(p.create(5_000, &vec![0; MAX_PENDING_BYTES]), Stored::KeyCap);
        let bytes = p.bytes();
        let big = vec![0xA5; MAX_PENDING_BYTES];
        p.keep_session_table(&big);
        assert_eq!(p.bytes(), bytes, "not in the store's count");
        assert_eq!(p.session_table_bytes(), MAX_PENDING_BYTES);
        p.keep_session_table(&[1]);
        assert_eq!(p.take_session_table().as_deref(), Some(big.as_slice()));
        assert_eq!(p.take_session_table(), None);
        p.keep_session_table(&[1]);
        p.discard();
        assert_eq!(p.session_table_bytes(), 0);
        p.keep_session_table(&[1]);
        let _ = p.take_snapshots();
        assert_eq!(p.session_table_bytes(), 0, "the release empties the store");
    }

    /// A despawn keeps the local identity, which is held apart.
    #[test]
    fn a_despawn_leaves_the_local_identity() {
        let mut p = PendingWorld::default();
        p.set_player(7);
        assert_eq!(p.create(7, &[0]), Stored::Kept);
        assert_eq!(p.despawn(7), Stored::Kept);
        assert_eq!(p.player(), Some(7));
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
        // Decision 0046 item 10 holds the later messages, with item 5 for the first three.
        for send in &WITHHELD_BEFORE_F4[3..7] {
            assert!(send.held_by.contains("decision 0046 item 10"), "{send:?}");
        }
        for send in &WITHHELD_BEFORE_F4[..3] {
            assert!(send.held_by.contains("decision 0046 item 5"), "{send:?}");
        }
    }

    /// §9.3.9: only an explicit 0x40 to a resolved target deletes. Labelled until the writer answers:
    /// an unresolved ID and the `0x90`'s own ID delete nothing; a despawn record answers so a repeat
    /// is counted as blocked.
    #[test]
    fn only_an_explicit_resolved_target_despawns() {
        let mut p = PendingWorld::default();
        p.set_player(7);
        assert!(!p.despawn_applies(DEFAULT_RECIPIENT));
        assert!(!p.despawn_applies(7), "the 0x90's ID");
        assert_eq!(p.appearance(9, &[0, 1]), Stored::Kept);
        assert!(!p.despawn_applies(9), "unresolved");
        assert_eq!(p.create(9, &[0, 2]), Stored::Kept);
        assert!(p.despawn_applies(9));
        assert_eq!(p.despawn(9), Stored::Kept);
        assert!(p.despawn_applies(9), "the record answers");
        assert_eq!(p.despawn(9), Stored::Despawned);
    }

    /// §9.3.9, §9.3.12 Question 1: a `0xCE` is kept under its raw key with its selector as metadata
    /// only; the selector names no key.
    #[test]
    fn a_position_keeps_its_raw_key_and_selector_metadata() {
        let mut p = PendingWorld::default();
        p.set_player(7);
        assert_eq!(
            p.position(DEFAULT_RECIPIENT, &[0, 1], Some(at(1.0)), Some(7)),
            Stored::Kept
        );
        assert_eq!(p.selector(DEFAULT_RECIPIENT), Some(7));
        assert_eq!(p.entities(), 1, "no key for the selector");
        assert_eq!(
            p.set_player(7),
            None,
            "the selector does not place the player"
        );
        let (spawns, states) = p.take_snapshots();
        assert!(
            spawns.is_empty() && states.is_empty(),
            "raw key 2 is discarded"
        );
    }
}
