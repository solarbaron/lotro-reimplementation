// SPDX-License-Identifier: GPL-3.0-or-later
//! [`EntityStore`]: the client's entity table and its component sets.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::marker::PhantomData;

use crate::changes::{Applied, Changes, Deferred, DeferredOp};
use crate::column::Column;
use crate::entity::{Entities, Entity};
use crate::error::{EntityError, Rejected, Result};

/// Position, rotation and scale of an entity.
///
/// Positions are `f64` metres in the frame of the player's current region: the world
/// position of `docs/ARCHITECTURE.md` §9.1 (landblock origin plus block-local position;
/// `docs/specs/coordinates.md` §12), the frame of `tessera-net`'s positions. Region
/// coordinates reach about 40 km, where `f32` steps are about 4 mm, so the store keeps
/// `f64` and the renderer subtracts its camera anchor in `f64` before narrowing to `f32`
/// (the §9.1 render position). Z is up. The rotation is a unit quaternion stored
/// (x, y, z, w), active from object space to the region frame. The store keeps whatever
/// the caller sets; [`Transform::is_finite`] is there for callers that take transforms
/// from untrusted sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Position in metres (region frame).
    pub position: [f64; 3],
    /// Rotation quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// Per-axis scale.
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// At the origin, unrotated, unit scale.
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };

    /// Unrotated and unscaled at `position`.
    #[must_use]
    pub const fn from_position(position: [f64; 3]) -> Self {
        Self {
            position,
            ..Self::IDENTITY
        }
    }

    /// At `position`, turned `yaw` radians counter-clockwise about +Z.
    #[must_use]
    pub fn from_position_yaw(position: [f64; 3], yaw: f32) -> Self {
        let (s, c) = (yaw * 0.5).sin_cos();
        Self {
            position,
            rotation: [0.0, 0.0, s, c],
            scale: [1.0; 3],
        }
    }

    /// Whether every component is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.position.iter().all(|v| v.is_finite())
            && self
                .rotation
                .iter()
                .chain(&self.scale)
                .all(|v| v.is_finite())
    }
}

/// The ID a network backend gives an entity (`tessera-net`'s `EntityId`); unique among
/// live entities of one store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetId(pub u64);

impl fmt::Display for NetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The landblock a streamed static belongs to. Any `u32` the caller picks; the client uses
/// the landblock-data DID of `tessera-world`'s `LandblockKey`, which round-trips through
/// `LandblockKey::from_did`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LandblockTag(pub u32);

impl fmt::Display for LandblockTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

/// Configuration of an [`EntityStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreConfig {
    /// Most entity slots (live plus free); clamped to [`MAX_ENTITIES`](crate::MAX_ENTITIES). The default,
    /// 4,194,304, is far above the peak the M3 walker streams (a few hundred landblocks
    /// with their statics) and bounds what a hostile server can make the client allocate.
    pub max_entities: u32,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_entities: 1 << 22,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Member {
    tag: LandblockTag,
    /// Position in the landblock's entity list.
    pos: u32,
}

const F_SPAWNED: u8 = 1;
const F_MOVED: u8 = 2;
const F_RENDER: u8 = 4;
const F_ANIM: u8 = 8;

/// The client's entity table: generational IDs and the component sets the client needs.
///
/// | Component | Accessors | Query |
/// |---|---|---|
/// | [`Transform`] (every entity has one) | [`transform`](Self::transform), [`set_transform`](Self::set_transform) | [`transforms`](Self::transforms) |
/// | [`NetId`] (server-driven entities) | [`net_id`](Self::net_id), [`entity_for`](Self::entity_for) | [`networked`](Self::networked) |
/// | render instance `R` | [`render`](Self::render), [`set_render`](Self::set_render), [`remove_render`](Self::remove_render) | [`renderables`](Self::renderables), [`landblock_renderables`](Self::landblock_renderables) |
/// | animation instance `A` | [`anim`](Self::anim), [`set_anim`](Self::set_anim), [`remove_anim`](Self::remove_anim) | [`animated`](Self::animated), [`animated_mut`](Self::animated_mut) |
/// | [`LandblockTag`] (streamed statics) | [`landblock_of`](Self::landblock_of), [`spawn_static`](Self::spawn_static), [`despawn_landblock`](Self::despawn_landblock) | [`landblock_entities`](Self::landblock_entities) |
///
/// `R` and `A` are whatever handles the renderer and the animation scheduler hand out
/// (the client uses its render-instance handle and `tessera_anim::InstanceId` plus its
/// `Animator`); the store never looks inside them, so it depends on neither crate.
/// Removed `R` and `A` values come back through [`take_changes`](Self::take_changes) so
/// their owners can release them; values an operation refuses come back in its
/// [`Rejected`] error. The store never drops one.
///
/// Further per-entity data (property sets, audio emitters) lives in caller-side
/// [`Column`] tables, cleaned with [`Changes::despawned`], until a query needs it here.
/// Insert into them only after [`is_alive`](Self::is_alive) confirms the entity (see
/// [`Column::insert`]).
///
/// # Main thread only
///
/// The store is the main thread's (`docs/ARCHITECTURE.md` §20.3, §21): network events,
/// streaming and animation submission mutate it there, and workers never see it. It is
/// deliberately neither `Send` nor `Sync`, so the compiler rejects moving it to or sharing
/// it with another thread. Workers get copies (transforms, palettes) or `Arc`s of
/// immutable data instead.
///
/// ```compile_fail
/// fn needs_send<T: Send>(_: T) {}
/// needs_send(tessera_entity::EntityStore::<u32, u32>::new());
/// ```
///
/// # Iteration and removal
///
/// Queries borrow the store, so structural changes inside a loop go through a
/// [`Deferred`] list applied afterwards with [`apply`](Self::apply). Component order
/// within a query is unspecified and changes when components are removed.
///
/// # Hostile input
///
/// No operation panics. Entity IDs are generation-checked, so a stale or foreign ID fails
/// with [`EntityError::Dead`] (or returns `None`) instead of reaching another entity.
/// Network IDs are unique among live entities.
///
/// Memory: [`StoreConfig::max_entities`] bounds the slots, the component columns and the
/// indexes. It does **not** bound the pending change lists: every operation since the
/// last [`take_changes`](Self::take_changes) can add an entry (a spawn/despawn pair on a
/// reused slot adds one to `spawned` and one to `despawned` each time; each replaced or
/// removed payload adds one to its removal list), so they grow with the number of
/// operations applied, not with the cap. Call `take_changes` every frame and bound the
/// events applied per frame (the client's per-frame network event budget).
pub struct EntityStore<R, A> {
    entities: Entities,
    /// Indexed by slot index; meaningful for live slots only (every entity has one, so a
    /// join needs no index table: one access per entity).
    transforms: Vec<Transform>,
    net: Column<NetId>,
    net_index: HashMap<NetId, Entity>,
    render: Column<R>,
    anim: Column<A>,
    members: Column<Member>,
    blocks: HashMap<LandblockTag, Vec<Entity>>,
    changes: Changes<R, A>,
    /// Per slot: which change lists already hold the slot's live entity (F_* bits).
    flags: Vec<u8>,
    /// Neither `Send` nor `Sync`: main thread only.
    _main_thread: PhantomData<*const ()>,
}

impl<R, A> Default for EntityStore<R, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R, A> fmt::Debug for EntityStore<R, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntityStore")
            .field("entities", &self.entities.len())
            .field("slots", &self.entities.slot_count())
            .field("networked", &self.net.len())
            .field("render", &self.render.len())
            .field("anim", &self.anim.len())
            .field("landblocks", &self.blocks.len())
            .field("pending_changes", &self.changes)
            .finish()
    }
}

impl<R, A> EntityStore<R, A> {
    /// An empty store with the default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(StoreConfig::default())
    }

    /// An empty store.
    #[must_use]
    pub fn with_config(config: StoreConfig) -> Self {
        Self {
            entities: Entities::new(config.max_entities),
            transforms: Vec::new(),
            net: Column::new(),
            net_index: HashMap::new(),
            render: Column::new(),
            anim: Column::new(),
            members: Column::new(),
            blocks: HashMap::new(),
            changes: Changes::default(),
            flags: Vec::new(),
            _main_thread: PhantomData,
        }
    }

    /// Live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether no entity is alive.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// The ID allocator (slot counts, limit, live entities in slot order).
    #[must_use]
    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    /// Whether `e` is alive.
    #[must_use]
    pub fn is_alive(&self, e: Entity) -> bool {
        self.entities.is_alive(e)
    }

    fn mark(&mut self, e: Entity, bit: u8) -> bool {
        let i = e.index() as usize;
        if i >= self.flags.len() {
            self.flags.resize(i + 1, 0);
        }
        let first = self.flags[i] & bit == 0;
        self.flags[i] |= bit;
        first
    }

    fn check(&self, e: Entity) -> Result<()> {
        if self.entities.is_alive(e) {
            Ok(())
        } else {
            Err(EntityError::Dead(e))
        }
    }

    // ----- spawning and despawning -------------------------------------------------------

    /// Spawns an entity with only a transform.
    ///
    /// # Errors
    ///
    /// [`EntityError::Full`].
    pub fn spawn(&mut self, transform: Transform) -> Result<Entity> {
        let e = self.entities.alloc().inspect_err(|err| {
            tracing::debug!(%err, "entity spawn refused");
        })?;
        let i = e.index() as usize;
        if i >= self.transforms.len() {
            self.transforms.resize(i + 1, Transform::IDENTITY);
        }
        self.transforms[i] = transform;
        if self.mark(e, F_SPAWNED) {
            self.changes.spawned.push(e);
        }
        Ok(e)
    }

    /// Spawns a server-driven entity named `net`.
    ///
    /// # Errors
    ///
    /// [`EntityError::NetIdInUse`] when a live entity already has `net`;
    /// [`EntityError::Full`].
    pub fn spawn_networked(&mut self, net: NetId, transform: Transform) -> Result<Entity> {
        if let Some(&entity) = self.net_index.get(&net) {
            return Err(EntityError::NetIdInUse { net, entity });
        }
        let e = self.spawn(transform)?;
        // `e` was just allocated, so the slot holds no newer generation.
        let _ = self.net.insert(e, net);
        self.net_index.insert(net, e);
        Ok(e)
    }

    /// Spawns a streamed static of landblock `tag`, with an optional render component.
    ///
    /// # Errors
    ///
    /// [`EntityError::Full`], with `render` handed back in the [`Rejected`] error so its
    /// owner can release it.
    pub fn spawn_static(
        &mut self,
        tag: LandblockTag,
        transform: Transform,
        render: Option<R>,
    ) -> Result<Entity, Rejected<Option<R>>> {
        let e = match self.spawn(transform) {
            Ok(e) => e,
            Err(err) => return Err(Rejected::new(err, render)),
        };
        let list = self.blocks.entry(tag).or_default();
        // A list holds distinct live entities, fewer than MAX_ENTITIES, so it fits in u32.
        let pos = list.len() as u32;
        list.push(e);
        let _ = self.members.insert(e, Member { tag, pos });
        if let Some(r) = render {
            // `e` was just allocated, so the slot holds no newer generation and `insert`
            // cannot refuse; were it to, the payload still reaches its owner.
            match self.render.insert(e, r) {
                Ok(old) => {
                    if let Some(old) = old {
                        self.changes.render_removed.push((e, old));
                    }
                    if self.mark(e, F_RENDER) {
                        self.changes.render_added.push(e);
                    }
                }
                Err(r) => self.changes.render_removed.push((e, r)),
            }
        }
        Ok(e)
    }

    /// Despawns `e` and every component it has; render and animation payloads go to the
    /// change lists. Returns false when `e` was not alive.
    pub fn despawn(&mut self, e: Entity) -> bool {
        if !self.entities.is_alive(e) {
            return false;
        }
        if let Some(m) = self.members.remove(e) {
            self.unlink_member(e, m);
        }
        self.despawn_components(e);
        true
    }

    /// Removes `e` from its landblock's list (swap-remove, fixing the moved entity's slot).
    fn unlink_member(&mut self, e: Entity, m: Member) {
        let Entry::Occupied(mut slot) = self.blocks.entry(m.tag) else {
            return;
        };
        let list = slot.get_mut();
        let pos = m.pos as usize;
        if list.get(pos) != Some(&e) {
            return; // Unreachable while the invariants hold; never index blindly.
        }
        list.swap_remove(pos);
        if let Some(&moved) = list.get(pos)
            && let Some(mm) = self.members.get_mut(moved)
        {
            mm.pos = m.pos;
        }
        if list.is_empty() {
            slot.remove();
        }
    }

    /// Everything but landblock membership.
    fn despawn_components(&mut self, e: Entity) {
        if let Some(net) = self.net.remove(e) {
            self.net_index.remove(&net);
        }
        if let Some(r) = self.render.remove(e) {
            self.changes.render_removed.push((e, r));
        }
        if let Some(a) = self.anim.remove(e) {
            self.changes.anim_removed.push((e, a));
        }
        if let Some(f) = self.flags.get_mut(e.index() as usize) {
            *f = 0;
        }
        self.entities.free(e);
        self.changes.despawned.push(e);
    }

    /// Despawns every static of landblock `tag`; returns how many.
    pub fn despawn_landblock(&mut self, tag: LandblockTag) -> usize {
        let Some(list) = self.blocks.remove(&tag) else {
            return 0;
        };
        let n = list.len();
        for e in list {
            if self.members.remove(e).is_some() {
                self.despawn_components(e);
            }
        }
        tracing::trace!(%tag, statics = n, "landblock statics despawned");
        n
    }

    /// Despawns every entity (payloads go to the change lists).
    pub fn clear(&mut self) {
        let all: Vec<Entity> = self.entities.iter().collect();
        self.blocks.clear();
        self.members.clear();
        for e in all {
            self.despawn_components(e);
        }
    }

    // ----- transforms ---------------------------------------------------------------------

    /// `e`'s transform.
    #[must_use]
    pub fn transform(&self, e: Entity) -> Option<&Transform> {
        if !self.entities.is_alive(e) {
            return None;
        }
        self.transforms.get(e.index() as usize)
    }

    /// Sets `e`'s transform and lists it in [`Changes::moved`].
    ///
    /// # Errors
    ///
    /// [`EntityError::Dead`].
    pub fn set_transform(&mut self, e: Entity, transform: Transform) -> Result<()> {
        self.check(e)?;
        let t = self
            .transforms
            .get_mut(e.index() as usize)
            .ok_or(EntityError::Dead(e))?;
        *t = transform;
        if self.mark(e, F_MOVED) {
            self.changes.moved.push(e);
        }
        Ok(())
    }

    // ----- network IDs ---------------------------------------------------------------------

    /// `e`'s network ID.
    #[must_use]
    pub fn net_id(&self, e: Entity) -> Option<NetId> {
        self.net.get(e).copied()
    }

    /// The live entity named `net`.
    #[must_use]
    pub fn entity_for(&self, net: NetId) -> Option<Entity> {
        self.net_index.get(&net).copied()
    }

    // ----- render instances ------------------------------------------------------------------

    /// `e`'s render component.
    #[must_use]
    pub fn render(&self, e: Entity) -> Option<&R> {
        self.render.get(e)
    }

    /// `e`'s render component, mutable (not change-tracked; replace it with
    /// [`set_render`](Self::set_render) when the renderer must notice).
    #[must_use]
    pub fn render_mut(&mut self, e: Entity) -> Option<&mut R> {
        self.render.get_mut(e)
    }

    /// Gives `e` a render component; a previous one goes to [`Changes::render_removed`].
    ///
    /// # Errors
    ///
    /// [`EntityError::Dead`], with `render` handed back in the [`Rejected`] error so its
    /// owner can release it (the entity may have been despawned by an earlier event of
    /// the same batch).
    pub fn set_render(&mut self, e: Entity, render: R) -> Result<(), Rejected<R>> {
        if let Err(err) = self.check(e) {
            return Err(Rejected::new(err, render));
        }
        match self.render.insert(e, render) {
            Ok(Some(old)) => self.changes.render_removed.push((e, old)),
            Ok(None) => {}
            // Unreachable for a live `e` (no slot holds a newer generation than its live
            // occupant); refuse rather than lose the value.
            Err(render) => return Err(Rejected::new(EntityError::Dead(e), render)),
        }
        if self.mark(e, F_RENDER) {
            self.changes.render_added.push(e);
        }
        Ok(())
    }

    /// Removes `e`'s render component (its payload goes to [`Changes::render_removed`]).
    /// Returns false when there was none.
    pub fn remove_render(&mut self, e: Entity) -> bool {
        match self.render.remove(e) {
            Some(r) => {
                self.changes.render_removed.push((e, r));
                true
            }
            None => false,
        }
    }

    // ----- animation instances ---------------------------------------------------------------

    /// `e`'s animation component.
    #[must_use]
    pub fn anim(&self, e: Entity) -> Option<&A> {
        self.anim.get(e)
    }

    /// `e`'s animation component, mutable.
    #[must_use]
    pub fn anim_mut(&mut self, e: Entity) -> Option<&mut A> {
        self.anim.get_mut(e)
    }

    /// Gives `e` an animation component; a previous one goes to [`Changes::anim_removed`].
    ///
    /// # Errors
    ///
    /// [`EntityError::Dead`], with `anim` handed back in the [`Rejected`] error so its
    /// owner can release it (for example remove its scheduler instance).
    pub fn set_anim(&mut self, e: Entity, anim: A) -> Result<(), Rejected<A>> {
        if let Err(err) = self.check(e) {
            return Err(Rejected::new(err, anim));
        }
        match self.anim.insert(e, anim) {
            Ok(Some(old)) => self.changes.anim_removed.push((e, old)),
            Ok(None) => {}
            // Unreachable for a live `e`; refuse rather than lose the value.
            Err(anim) => return Err(Rejected::new(EntityError::Dead(e), anim)),
        }
        if self.mark(e, F_ANIM) {
            self.changes.anim_added.push(e);
        }
        Ok(())
    }

    /// Removes `e`'s animation component (its payload goes to [`Changes::anim_removed`]).
    /// Returns false when there was none.
    pub fn remove_anim(&mut self, e: Entity) -> bool {
        match self.anim.remove(e) {
            Some(a) => {
                self.changes.anim_removed.push((e, a));
                true
            }
            None => false,
        }
    }

    // ----- landblock membership -----------------------------------------------------------------

    /// The landblock of streamed static `e`.
    #[must_use]
    pub fn landblock_of(&self, e: Entity) -> Option<LandblockTag> {
        self.members.get(e).map(|m| m.tag)
    }

    /// The live statics of landblock `tag` (empty when none).
    #[must_use]
    pub fn landblock_entities(&self, tag: LandblockTag) -> &[Entity] {
        self.blocks.get(&tag).map_or(&[], Vec::as_slice)
    }

    /// Landblocks with at least one live static.
    pub fn landblocks(&self) -> impl Iterator<Item = (LandblockTag, usize)> + '_ {
        self.blocks.iter().map(|(t, l)| (*t, l.len()))
    }

    /// Number of landblocks with at least one live static.
    #[must_use]
    pub fn landblock_count(&self) -> usize {
        self.blocks.len()
    }

    // ----- queries ---------------------------------------------------------------------------------

    /// Every entity with its transform.
    pub fn transforms(&self) -> impl Iterator<Item = (Entity, &Transform)> + '_ {
        let t = &self.transforms;
        self.entities
            .iter()
            .filter_map(move |e| t.get(e.index() as usize).map(|t| (e, t)))
    }

    /// Server-driven entities: (entity, network ID, transform).
    pub fn networked(&self) -> impl Iterator<Item = (Entity, NetId, &Transform)> + '_ {
        let t = &self.transforms;
        self.net
            .iter()
            .filter_map(move |(e, n)| t.get(e.index() as usize).map(|t| (e, *n, t)))
    }

    /// Entities with a render component: (entity, render, transform).
    pub fn renderables(&self) -> impl Iterator<Item = (Entity, &R, &Transform)> + '_ {
        let t = &self.transforms;
        self.render
            .iter()
            .filter_map(move |(e, r)| t.get(e.index() as usize).map(|t| (e, r, t)))
    }

    /// Statics of landblock `tag` that have a render component.
    pub fn landblock_renderables(
        &self,
        tag: LandblockTag,
    ) -> impl Iterator<Item = (Entity, &R, &Transform)> + '_ {
        self.landblock_entities(tag).iter().filter_map(move |&e| {
            let r = self.render.get(e)?;
            Some((e, r, self.transforms.get(e.index() as usize)?))
        })
    }

    /// Entities with an animation component: (entity, animation, transform).
    pub fn animated(&self) -> impl Iterator<Item = (Entity, &A, &Transform)> + '_ {
        let t = &self.transforms;
        self.anim
            .iter()
            .filter_map(move |(e, a)| t.get(e.index() as usize).map(|t| (e, a, t)))
    }

    /// Entities with an animation component, the component mutable (advance animators,
    /// submit sampling jobs).
    pub fn animated_mut(&mut self) -> impl Iterator<Item = (Entity, &mut A, &Transform)> + '_ {
        let t = &self.transforms;
        self.anim
            .iter_mut()
            .filter_map(move |(e, a)| t.get(e.index() as usize).map(|t| (e, a, t)))
    }

    /// Animated entities that also render: (entity, animation, render, transform), the
    /// animation mutable (palette hand-off).
    pub fn animated_renderables_mut(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &mut A, &R, &Transform)> + '_ {
        let (t, r) = (&self.transforms, &self.render);
        self.anim
            .iter_mut()
            .filter_map(move |(e, a)| Some((e, a, r.get(e)?, t.get(e.index() as usize)?)))
    }

    // ----- changes and deferred operations ---------------------------------------------------------

    /// Changes recorded since the last take (unfiltered; see [`Changes`]).
    #[must_use]
    pub fn pending_changes(&self) -> &Changes<R, A> {
        &self.changes
    }

    /// Moves the recorded changes into `out` (which is cleared first; its allocations are
    /// reused for the next batch). Call once per frame.
    pub fn take_changes(&mut self, out: &mut Changes<R, A>) {
        out.clear();
        std::mem::swap(&mut self.changes, out);
        for e in out
            .spawned
            .iter()
            .chain(&out.moved)
            .chain(&out.render_added)
            .chain(&out.anim_added)
        {
            // Every set bit belongs to an entry of this batch, so clearing all is exact.
            if let Some(f) = self.flags.get_mut(e.index() as usize) {
                *f = 0;
            }
        }
        let ids = &self.entities;
        out.spawned.retain(|&e| ids.is_alive(e));
        out.moved.retain(|&e| ids.is_alive(e));
        out.render_added.retain(|&e| self.render.contains(e));
        out.anim_added.retain(|&e| self.anim.contains(e));
    }

    /// Applies (and empties) a deferred list in order. Operations on entities that are no
    /// longer alive (or hold no such component), and landblock despawns of a landblock
    /// without live statics (already released, or never spawned), are skipped and counted
    /// as stale.
    pub fn apply(&mut self, deferred: &mut Deferred) -> Applied {
        let mut out = Applied::default();
        let mut ops = deferred.take_ops();
        for op in ops.drain(..) {
            let ok = match op {
                DeferredOp::Despawn(e) => self.despawn(e),
                DeferredOp::DespawnLandblock(t) => self.despawn_landblock(t) > 0,
                DeferredOp::SetTransform(e, t) => self.set_transform(e, t).is_ok(),
                DeferredOp::RemoveRender(e) => self.remove_render(e),
                DeferredOp::RemoveAnim(e) => self.remove_anim(e),
            };
            if ok {
                out.applied += 1;
            } else {
                out.stale += 1;
            }
        }
        deferred.restore(ops);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Store = EntityStore<u32, u64>;

    fn take(s: &mut Store) -> Changes<u32, u64> {
        let mut c = Changes::new();
        s.take_changes(&mut c);
        c
    }

    #[test]
    fn spawn_query_despawn() {
        let mut s = Store::new();
        let a = s
            .spawn_networked(NetId(7), Transform::from_position([1.0, 2.0, 3.0]))
            .unwrap();
        s.set_render(a, 11).unwrap();
        s.set_anim(a, 99).unwrap();
        assert_eq!(s.entity_for(NetId(7)), Some(a));
        assert_eq!(
            s.spawn_networked(NetId(7), Transform::IDENTITY),
            Err(EntityError::NetIdInUse {
                net: NetId(7),
                entity: a
            })
        );
        assert_eq!(s.animated().count(), 1);
        assert_eq!(s.renderables().count(), 1);
        let c = take(&mut s);
        assert_eq!(c.spawned, vec![a]);
        assert_eq!(c.render_added, vec![a]);
        assert_eq!(c.anim_added, vec![a]);
        assert!(s.despawn(a));
        assert!(!s.despawn(a));
        assert_eq!(s.entity_for(NetId(7)), None);
        let c = take(&mut s);
        assert_eq!(c.despawned, vec![a]);
        assert_eq!(c.render_removed, vec![(a, 11)]);
        assert_eq!(c.anim_removed, vec![(a, 99)]);
        assert!(take(&mut s).is_empty());
    }

    #[test]
    fn landblock_statics_come_and_go_together() {
        let mut s = Store::new();
        let (b1, b2) = (LandblockTag(1), LandblockTag(2));
        let x: Vec<Entity> = (0..5)
            .map(|i| s.spawn_static(b1, Transform::IDENTITY, Some(i)).unwrap())
            .collect();
        let y = s.spawn_static(b2, Transform::IDENTITY, None).unwrap();
        assert_eq!(s.landblock_count(), 2);
        // Despawning one static from the middle keeps the list consistent.
        assert!(s.despawn(x[1]));
        assert_eq!(s.landblock_entities(b1).len(), 4);
        for &e in s.landblock_entities(b1) {
            assert_eq!(s.landblock_of(e), Some(b1));
        }
        assert_eq!(s.landblock_renderables(b1).count(), 4);
        assert_eq!(s.despawn_landblock(b1), 4);
        assert_eq!(s.despawn_landblock(b1), 0);
        assert_eq!(s.len(), 1);
        assert!(s.is_alive(y));
        let c = take(&mut s);
        assert_eq!(c.render_removed.len(), 5);
        assert_eq!(c.despawned.len(), 5);
        assert_eq!(c.spawned, vec![y], "despawned spawns are filtered");
        assert!(c.render_added.is_empty());
    }

    #[test]
    fn stale_ids_never_reach_new_occupant() {
        let mut s = Store::new();
        let a = s.spawn(Transform::IDENTITY).unwrap();
        s.despawn(a);
        let b = s.spawn(Transform::from_position([5.0, 0.0, 0.0])).unwrap();
        assert_eq!(a.index(), b.index());
        assert_eq!(s.transform(a), None);
        assert_eq!(
            s.set_transform(a, Transform::IDENTITY),
            Err(EntityError::Dead(a))
        );
        assert_eq!(
            s.set_render(a, 1).map_err(Rejected::into_parts),
            Err((EntityError::Dead(a), 1))
        );
        assert!(!s.despawn(a));
        assert_eq!(s.transform(b).unwrap().position[0], 5.0);
        // Within one batch: `a` spawned and died, `b` reused the slot.
        let c = take(&mut s);
        assert_eq!(c.spawned, vec![b]);
        assert_eq!(c.despawned, vec![a]);
    }

    #[test]
    fn replaced_and_removed_components_report_payloads() {
        let mut s = Store::new();
        let e = s.spawn(Transform::IDENTITY).unwrap();
        s.set_render(e, 1).unwrap();
        s.set_render(e, 2).unwrap();
        assert!(s.remove_render(e));
        assert!(!s.remove_render(e));
        s.set_render(e, 3).unwrap();
        s.set_transform(e, Transform::IDENTITY).unwrap();
        s.set_transform(e, Transform::IDENTITY).unwrap();
        let c = take(&mut s);
        assert_eq!(c.render_removed, vec![(e, 1), (e, 2)]);
        assert_eq!(c.render_added, vec![e], "listed once");
        assert_eq!(c.moved, vec![e], "listed once");
        s.set_transform(e, Transform::IDENTITY).unwrap();
        assert_eq!(take(&mut s).moved, vec![e], "flags reset after take");
    }

    #[test]
    fn deferred_removal_while_iterating() {
        let mut s = Store::new();
        let tag = LandblockTag(9);
        for i in 0..10 {
            let e = s.spawn_static(tag, Transform::IDENTITY, Some(i)).unwrap();
            s.set_anim(e, u64::from(i)).unwrap();
        }
        let mut d = Deferred::new();
        for (e, a, _) in s.animated_mut() {
            *a += 100;
            if *a % 3 == 0 {
                d.despawn(e);
                d.despawn(e); // the second one is stale by the time it applies
            }
        }
        let applied = s.apply(&mut d);
        // 102, 105 and 108 are despawned; each second despawn is stale.
        assert_eq!(
            applied,
            Applied {
                applied: 3,
                stale: 3
            }
        );
        assert!(d.is_empty());
        assert_eq!(s.len(), 7);
        assert!(s.animated().all(|(_, a, _)| *a >= 100 && *a % 3 != 0));
        assert_eq!(s.landblock_entities(tag).len(), 7);
    }

    #[test]
    fn full_store_refuses_and_stays_consistent() {
        let mut s = Store::with_config(StoreConfig { max_entities: 2 });
        s.spawn_networked(NetId(1), Transform::IDENTITY).unwrap();
        s.spawn(Transform::IDENTITY).unwrap();
        assert_eq!(
            s.spawn_networked(NetId(2), Transform::IDENTITY),
            Err(EntityError::Full { max: 2 })
        );
        assert_eq!(s.entity_for(NetId(2)), None);
        assert_eq!(
            s.spawn_static(LandblockTag(1), Transform::IDENTITY, Some(1))
                .map_err(Rejected::into_parts),
            Err((EntityError::Full { max: 2 }, Some(1))),
            "the refused render payload comes back"
        );
        assert_eq!(s.landblock_count(), 0);
    }

    #[test]
    fn refused_payloads_are_handed_back() {
        // Regression: set_render / set_anim on a dead entity and spawn_static on a full
        // store used to drop the payload, leaking the resource it names.
        let mut s = Store::with_config(StoreConfig { max_entities: 1 });
        let e = s.spawn(Transform::IDENTITY).unwrap();
        assert!(s.despawn(e));
        let r = s.set_render(e, 8).unwrap_err();
        assert_eq!((r.error.clone(), r.value), (EntityError::Dead(e), 8));
        let a = s.set_anim(e, 9).unwrap_err();
        assert_eq!(a.into_parts(), (EntityError::Dead(e), 9));
        let _live = s.spawn(Transform::IDENTITY).unwrap();
        let full = s
            .spawn_static(LandblockTag(1), Transform::IDENTITY, Some(42))
            .unwrap_err();
        assert_eq!(full.into_parts(), (EntityError::Full { max: 1 }, Some(42)));
        // Nothing refused reached the change lists or the columns.
        let c = take(&mut s);
        assert!(c.render_added.is_empty() && c.render_removed.is_empty());
        assert!(c.anim_added.is_empty() && c.anim_removed.is_empty());
        assert_eq!(s.renderables().count(), 0);
        // The error displays as its reason.
        assert_eq!(
            s.set_render(e, 1).unwrap_err().to_string(),
            EntityError::Dead(e).to_string()
        );
    }

    #[test]
    fn deferred_landblock_despawn_without_statics_is_stale() {
        // Regression: a landblock despawn counted as applied even when nothing was
        // despawned (released twice, or never spawned).
        let mut s = Store::new();
        s.spawn_static(LandblockTag(1), Transform::IDENTITY, Some(1))
            .unwrap();
        let mut d = Deferred::new();
        d.despawn_landblock(LandblockTag(1));
        d.despawn_landblock(LandblockTag(1));
        d.despawn_landblock(LandblockTag(77));
        assert_eq!(
            s.apply(&mut d),
            Applied {
                applied: 1,
                stale: 2
            }
        );
        assert!(s.is_empty());
    }

    #[test]
    fn reused_slot_gets_its_own_additions_in_one_batch() {
        // The new occupant of a slot freed in the same batch is listed in render_added
        // and anim_added (despawn must reset the slot's change bits).
        let mut s = Store::with_config(StoreConfig { max_entities: 1 });
        let old = s.spawn(Transform::IDENTITY).unwrap();
        s.set_render(old, 1).unwrap();
        s.set_anim(old, 2).unwrap();
        s.set_transform(old, Transform::IDENTITY).unwrap();
        assert!(s.despawn(old));
        let new = s.spawn(Transform::IDENTITY).unwrap();
        assert_eq!(new.index(), old.index());
        s.set_render(new, 3).unwrap();
        s.set_anim(new, 4).unwrap();
        s.set_transform(new, Transform::IDENTITY).unwrap();
        let c = take(&mut s);
        assert_eq!(c.spawned, vec![new]);
        assert_eq!(c.despawned, vec![old]);
        assert_eq!(c.render_added, vec![new]);
        assert_eq!(c.anim_added, vec![new]);
        assert_eq!(c.moved, vec![new]);
        assert_eq!(c.render_removed, vec![(old, 1)]);
        assert_eq!(c.anim_removed, vec![(old, 2)]);
    }

    #[test]
    fn net_id_in_use_names_the_typed_id() {
        let mut s = Store::new();
        let a = s.spawn_networked(NetId(5), Transform::IDENTITY).unwrap();
        let err = s
            .spawn_networked(NetId(5), Transform::IDENTITY)
            .unwrap_err();
        assert_eq!(
            err,
            EntityError::NetIdInUse {
                net: NetId(5),
                entity: a
            }
        );
        assert_eq!(
            err.to_string(),
            format!("network ID 5 already names entity {a}")
        );
    }

    #[test]
    fn far_positions_keep_millimetre_precision() {
        // Region coordinates reach about 40 km; f32 would round to 4 mm steps there.
        let mut s = Store::new();
        let p = [40_800.001, 40_799.999, 12.5];
        let e = s.spawn(Transform::from_position(p)).unwrap();
        let got = s.transform(e).unwrap().position;
        assert_eq!(got, p);
        assert!((got[0] - 40_800.0 - 0.001).abs() < 1e-9);
    }

    #[test]
    fn clear_reports_everything() {
        let mut s = Store::new();
        s.spawn_static(LandblockTag(1), Transform::IDENTITY, Some(1))
            .unwrap();
        let n = s.spawn_networked(NetId(3), Transform::IDENTITY).unwrap();
        s.set_anim(n, 5).unwrap();
        take(&mut s);
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.landblock_count(), 0);
        assert_eq!(s.entity_for(NetId(3)), None);
        let c = take(&mut s);
        assert_eq!(c.despawned.len(), 2);
        assert_eq!(c.render_removed.len(), 1);
        assert_eq!(c.anim_removed, vec![(n, 5)]);
    }

    #[test]
    fn transform_helpers() {
        assert!(Transform::IDENTITY.is_finite());
        let t = Transform::from_position_yaw([0.0; 3], std::f32::consts::PI);
        assert!((t.rotation[2] - 1.0).abs() < 1e-6 && t.rotation[3].abs() < 1e-6);
        let mut bad = Transform::IDENTITY;
        bad.scale[1] = f32::NAN;
        assert!(!bad.is_finite());
    }
}
