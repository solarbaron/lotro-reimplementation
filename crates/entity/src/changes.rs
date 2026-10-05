// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-frame change lists ([`Changes`]) and deferred operations ([`Deferred`]).

use std::fmt;

use crate::entity::Entity;
use crate::store::{LandblockTag, Transform};

/// What changed in an [`EntityStore`](crate::EntityStore) since the last
/// [`take_changes`](crate::EntityStore::take_changes): the hand-off to the renderer,
/// the animation scheduler and anything else that mirrors entities.
///
/// Take the lists once per frame and process them in this order: **removals** first
/// (`despawned`, `render_removed`, `anim_removed`: free the resources whose payloads they
/// return), then **additions** (`spawned`, `render_added`, `anim_added`), then `moved`.
/// At take time every addition and `moved` entry is filtered to entities that are still
/// alive and still hold that component, so the order above never touches a freed
/// resource.
///
/// Multiplicity:
///
/// - `spawned`, `moved`, `render_added`, `anim_added` and `despawned` list an entity at
///   most once each.
/// - `render_removed` and `anim_removed` hold **one entry per removed payload**: an entity
///   whose component was replaced twice and then removed appears three times. Release
///   every entry; never key them by entity (a map from entity to payload would leak all
///   but one).
/// - One entity can be in several lists: a character spawned, given a render component
///   and moved within one batch is in `spawned`, `render_added` and `moved`. A consumer
///   that writes the same per-entity state for an addition and for a move (an instance
///   transform) should write it once, for example by skipping `moved` entries it already
///   handled as additions.
///
/// Removal payloads are always delivered, exactly once; a payload an operation refuses
/// never enters the store and comes back in that operation's
/// [`Rejected`](crate::Rejected) error instead.
///
/// The lists grow with the operations applied between two takes, not with the store's
/// slot cap ([`EntityStore`](crate::EntityStore)'s "Hostile input"); take them every
/// frame.
#[derive(Clone, PartialEq)]
pub struct Changes<R, A> {
    /// Entities spawned (alive at take time).
    pub spawned: Vec<Entity>,
    /// Entities despawned, in despawn order. An entity spawned and despawned within one
    /// batch appears here only.
    pub despawned: Vec<Entity>,
    /// Entities whose transform was set (alive at take time).
    pub moved: Vec<Entity>,
    /// Entities that gained a render component or had it replaced.
    pub render_added: Vec<Entity>,
    /// Render components removed (explicitly, by replacement or by despawn), with their
    /// payloads so their GPU instances can be released.
    pub render_removed: Vec<(Entity, R)>,
    /// Entities that gained an animation component or had it replaced.
    pub anim_added: Vec<Entity>,
    /// Animation components removed, with their payloads (scheduler instances to remove).
    pub anim_removed: Vec<(Entity, A)>,
}

impl<R, A> Default for Changes<R, A> {
    fn default() -> Self {
        Self {
            spawned: Vec::new(),
            despawned: Vec::new(),
            moved: Vec::new(),
            render_added: Vec::new(),
            render_removed: Vec::new(),
            anim_added: Vec::new(),
            anim_removed: Vec::new(),
        }
    }
}

impl<R, A> fmt::Debug for Changes<R, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Changes")
            .field("spawned", &self.spawned.len())
            .field("despawned", &self.despawned.len())
            .field("moved", &self.moved.len())
            .field("render_added", &self.render_added.len())
            .field("render_removed", &self.render_removed.len())
            .field("anim_added", &self.anim_added.len())
            .field("anim_removed", &self.anim_removed.len())
            .finish()
    }
}

impl<R, A> Changes<R, A> {
    /// Empty lists.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether every list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spawned.is_empty()
            && self.despawned.is_empty()
            && self.moved.is_empty()
            && self.render_added.is_empty()
            && self.render_removed.is_empty()
            && self.anim_added.is_empty()
            && self.anim_removed.is_empty()
    }

    /// Empties every list, keeping allocations.
    pub fn clear(&mut self) {
        self.spawned.clear();
        self.despawned.clear();
        self.moved.clear();
        self.render_added.clear();
        self.render_removed.clear();
        self.anim_added.clear();
        self.anim_removed.clear();
    }
}

/// One queued operation of a [`Deferred`] list.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum DeferredOp {
    /// [`EntityStore::despawn`](crate::EntityStore::despawn).
    Despawn(Entity),
    /// [`EntityStore::despawn_landblock`](crate::EntityStore::despawn_landblock).
    DespawnLandblock(LandblockTag),
    /// [`EntityStore::set_transform`](crate::EntityStore::set_transform).
    SetTransform(Entity, Transform),
    /// [`EntityStore::remove_render`](crate::EntityStore::remove_render).
    RemoveRender(Entity),
    /// [`EntityStore::remove_anim`](crate::EntityStore::remove_anim).
    RemoveAnim(Entity),
}

/// Structural changes recorded while the store is borrowed by a query, applied
/// afterwards with [`EntityStore::apply`](crate::EntityStore::apply).
///
/// Rust's borrow rules forbid despawning while iterating a query; record the operation
/// here instead and apply the list after the loop. Operations apply in recording order;
/// one naming an entity that is no longer alive by then (despawned twice, or despawned
/// earlier in the same list), or a landblock without live statics by then, is skipped
/// and counted as stale.
///
/// ```
/// use tessera_entity::{Deferred, EntityStore, Transform};
///
/// let mut store: EntityStore<u32, ()> = EntityStore::new();
/// for i in 0..4 {
///     let e = store.spawn(Transform::from_position([f64::from(i), 0.0, 0.0])).unwrap();
///     store.set_render(e, i).unwrap();
/// }
/// let mut later = Deferred::new();
/// for (e, r, _) in store.renderables() {
///     if r % 2 == 0 {
///         later.despawn(e);
///     }
/// }
/// let applied = store.apply(&mut later);
/// assert_eq!((applied.applied, applied.stale), (2, 0));
/// assert_eq!(store.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deferred {
    ops: Vec<DeferredOp>,
}

impl Deferred {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a despawn.
    pub fn despawn(&mut self, e: Entity) {
        self.ops.push(DeferredOp::Despawn(e));
    }

    /// Queues despawning every static of a landblock.
    pub fn despawn_landblock(&mut self, tag: LandblockTag) {
        self.ops.push(DeferredOp::DespawnLandblock(tag));
    }

    /// Queues a transform update.
    pub fn set_transform(&mut self, e: Entity, t: Transform) {
        self.ops.push(DeferredOp::SetTransform(e, t));
    }

    /// Queues removing a render component.
    pub fn remove_render(&mut self, e: Entity) {
        self.ops.push(DeferredOp::RemoveRender(e));
    }

    /// Queues removing an animation component.
    pub fn remove_anim(&mut self, e: Entity) {
        self.ops.push(DeferredOp::RemoveAnim(e));
    }

    /// Queues any operation.
    pub fn push(&mut self, op: DeferredOp) {
        self.ops.push(op);
    }

    /// Queued operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// The queued operations in order.
    #[must_use]
    pub fn ops(&self) -> &[DeferredOp] {
        &self.ops
    }

    /// Drops every queued operation.
    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// Takes the queued operations out (the caller hands the emptied vector back with
    /// [`Deferred::restore`] to keep its allocation).
    pub(crate) fn take_ops(&mut self) -> Vec<DeferredOp> {
        std::mem::take(&mut self.ops)
    }

    pub(crate) fn restore(&mut self, mut ops: Vec<DeferredOp>) {
        ops.clear();
        if self.ops.is_empty() {
            self.ops = ops;
        }
    }
}

/// What [`EntityStore::apply`](crate::EntityStore::apply) did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Applied {
    /// Operations that took effect.
    pub applied: usize,
    /// Operations skipped because their entity was no longer alive (or held no such
    /// component), or, for [`DeferredOp::DespawnLandblock`], because the landblock had no
    /// live static (already released, or never spawned).
    pub stale: usize,
}
