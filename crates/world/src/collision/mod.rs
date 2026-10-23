// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision residency: per-landblock collision sets (statics, generated objects, cells and
//! portals of landblock info) built with the bundle on the asset workers (`collision.md`
//! §9.3, §9.4, §12.1).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`CollisionSystem`] | Shared by every worker: the §9.1 property IDs, the object-collision cache (0x47 → setup → 0x66 mesh and volumes), the cell-geometry cache (0x01), limits and the content-layer options | §8, §9.1, §12.1 |
//! | [`CollisionInput`], [`CollisionSystem::build`], [`CollisionSystem::build_at`] | One landblock's set, in the order of §9.4: terrain and ceiling meshes, blocking statics (§9.3), blocking generated objects (§9.4), cells with their physics meshes and lookup (§5.3, §5.8.1), water plane and liquid boxes (`water.md` §6–§7) | §12.1 |
//! | [`PrioritySource`] | Where a build reads its job's priority, once, when the hierarchy build starts | §12.1 |
//! | [`HierarchyThreads`], [`HierarchyBuildCounts`] | Threads of one set's hierarchy build by job priority: `Prefetch` builds stay on their worker, `Visible` and higher draw helpers from the shared budget; counts per rule | §12.1 |
//! | [`BlockCollision`], [`CellPortals`], [`Portal`] | The built set, the cells of landblock info with their portal links and portal polygons in the landblock frame, and the residency charge | §5.5, §5.8 |
//!
//! Building never fails: objects and cells that cannot be resolved, placed or that exceed
//! the limits are left out and counted in the set's [`BuildReport`] (hostile input never
//! panics and never allocates beyond [`CollisionLimits`] and the per-set portal budgets
//! [`MAX_SET_PORTALS`] and [`MAX_SET_PORTAL_CORNERS`]). The caches are bounded by entry
//! count (and, for objects, by bytes). A read that fails for a reason that is not the
//! data's (a full queue, a shut-down pipeline) is **not cached**, for objects and cell
//! geometries alike, so the next landblock tries again; the set built with it is marked
//! incomplete ([`BlockCollision::complete`]) so that the streamer rebuilds it when the
//! landblock loads again.
//!
//! # Hierarchy build threads
//!
//! A set's hierarchy is built on the worker that builds the set. A large set (at least
//! [`PARALLEL_MIN_ITEMS`](tessera_physics::geometry::PARALLEL_MIN_ITEMS) items) may also
//! use helper threads, at most [`MAX_BUILD_THREADS`] a build, drawn from the one
//! process-wide budget of `tessera-physics` (§12.1). Which rule applies follows the
//! priority of the job ([`CollisionSystem::build_at`], [`HierarchyThreads::for_priority`]):
//! a job at `Prefetch` priority builds on its own worker alone (one thread), so prefetching
//! never takes helpers from a landblock that is needed now; `Visible` and `Immediate` jobs
//! use the shared budget. The tree, and so every query, does not depend on the rule.
//!
//! The rule reads the job's priority once, when the hierarchy build itself starts (§12.1),
//! after the terrain, statics, generated objects, cells and water have been added: the
//! caller of [`CollisionSystem::build_at`] passes a [`PrioritySource`] (a fixed
//! [`Priority`], a closure such as `|| ctx.current_priority()`, or the asset pool's
//! [`JobContext`] itself), which is asked exactly once, right before the hierarchy build,
//! so that a job raised from `Prefetch` to `Visible` while it waited in the queue or while
//! the earlier stages of its set ran builds under `Visible`'s rule. A change after that
//! point does not change the build's threads (the tree is the same either way). A build
//! with no job priority ([`CollisionSystem::build`]) counts as `Visible`.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::{JobContext, Priority};

use tessera_formats::collision::{CELL_GEOMETRY_PREFIX, CellGeometry};
use tessera_formats::{Did, LandblockData, LandblockId, LandblockInfo, PropertySchema};
use tessera_physics::geometry::{
    Aabb, BodyId, BuildReport, CellPlacement, CollisionLimits, CollisionSet, CollisionSetBuilder,
    DEFAULT_COLLISION_CACHE_BYTES, DEFAULT_COLLISION_CACHE_ENTRIES, FetchedRecord, GeneratedObject,
    GridMesh, MAX_BUILD_THREADS, ObjectCollision, PhysicsPids, StaticOptions,
    resolve_object_collision,
};

use crate::coords::LandblockKey;
use crate::source::WorldSource;
use crate::water::BlockWater;

/// Default bound of the cell-geometry cache, in records (the install has 1,057 cell
/// geometries).
pub const DEFAULT_CELL_CACHE_ENTRIES: usize = 4_096;

/// Most portals kept per cell (the install's largest portal count is far below; a cell
/// record bounds its neighbour list itself, this caps what [`CellPortals`] copies).
pub const MAX_PORTALS_PER_CELL: usize = 1_024;

/// Most corners kept per portal polygon (the install's portals have 4).
pub const MAX_PORTAL_CORNERS: usize = 64;

/// Most portal links one [`BlockCollision`] keeps over all its cells; further links are
/// left out and counted in [`BlockCollision::portals_left_out`].
pub const MAX_SET_PORTALS: usize = 65_536;

/// Most portal corners one [`BlockCollision`] keeps over all its portals (about 3 MiB);
/// portals beyond it keep their links without corners and are counted in
/// [`BlockCollision::portals_left_out`].
pub const MAX_SET_PORTAL_CORNERS: usize = 262_144;

/// The object-collision cache of a [`CollisionSystem`]: resolved objects by DID, bounded
/// by entry count and approximate bytes ([`ObjectCollision::heap_size`]); when either
/// would be exceeded the cache is cleared, and a single result larger than the byte bound
/// is not cached. Unlike a get-or-resolve cache it lets the caller decline to store a
/// result (one built while a read failed for a reason that is not the data's).
#[derive(Debug)]
pub struct ObjectCache {
    inner: Mutex<ObjectCacheInner>,
    max_entries: usize,
    max_bytes: usize,
}

#[derive(Debug, Default)]
struct ObjectCacheInner {
    map: HashMap<Did, Arc<ObjectCollision>>,
    bytes: usize,
}

impl ObjectCache {
    /// A cache of at most `max_entries` objects (at least 1) and about `max_bytes` bytes.
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(ObjectCacheInner::default()),
            max_entries: max_entries.max(1),
            max_bytes,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ObjectCacheInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The cached result for `object`.
    #[must_use]
    pub fn get(&self, object: Did) -> Option<Arc<ObjectCollision>> {
        self.lock().map.get(&object).cloned()
    }

    /// Stores `r` for `object` (bounds in the type docs) and returns the cached value: an
    /// earlier entry for `object` wins, so every caller shares one allocation.
    pub fn insert(&self, object: Did, r: Arc<ObjectCollision>) -> Arc<ObjectCollision> {
        let size = r.heap_size();
        if size > self.max_bytes {
            tracing::debug!(
                object = %format_args!("{:#010x}", object.0),
                bytes = size,
                "object collision too large to cache"
            );
            return r;
        }
        let mut inner = self.lock();
        if let Some(hit) = inner.map.get(&object) {
            return Arc::clone(hit);
        }
        if inner.map.len() >= self.max_entries || inner.bytes.saturating_add(size) > self.max_bytes
        {
            inner.map.clear();
            inner.bytes = 0;
        }
        inner.bytes += size;
        inner.map.insert(object, Arc::clone(&r));
        r
    }

    /// Entries cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().map.is_empty()
    }

    /// Approximate bytes cached.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.lock().bytes
    }

    /// Drops everything.
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.map.clear();
        inner.bytes = 0;
    }
}

/// Where a collision build reads its job's priority (module docs, "Hierarchy build
/// threads"; `collision.md` §12.1). [`CollisionSystem::build_at`] asks it exactly once,
/// right before the set's hierarchy build starts, after the terrain, statics, generated
/// objects, cells and water have been added.
///
/// Implemented for a fixed [`Priority`], for closures returning one (`&dyn Fn() ->
/// Priority` included), and for the asset pool's [`JobContext`] (by value or by
/// reference), which reports [`JobContext::current_priority`].
pub trait PrioritySource {
    /// The job's priority now.
    fn current_priority(&self) -> Priority;
}

impl PrioritySource for Priority {
    fn current_priority(&self) -> Priority {
        *self
    }
}

impl<F: Fn() -> Priority> PrioritySource for F {
    fn current_priority(&self) -> Priority {
        self()
    }
}

/// Reads [`JobContext::current_priority`], the job's live priority, never
/// [`JobContext::priority`] (the priority the worker took the job out of the queue at): the
/// thread rule counts the priority the job holds when its hierarchy build starts
/// (`collision.md` §12.1), and an asset-store request may raise or lower it after the job
/// has started (`tests/collision_live_priority.rs` checks the two apart).
impl PrioritySource for JobContext<'_> {
    fn current_priority(&self) -> Priority {
        JobContext::current_priority(self)
    }
}

/// As for [`JobContext`]: the live priority, [`JobContext::current_priority`].
impl PrioritySource for &JobContext<'_> {
    fn current_priority(&self) -> Priority {
        JobContext::current_priority(self)
    }
}

/// How many threads may build one collision set's hierarchy (module docs, "Hierarchy build
/// threads"; `collision.md` §12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HierarchyThreads {
    /// On the building worker alone, one thread (jobs at `Prefetch` priority).
    Worker,
    /// Up to [`MAX_BUILD_THREADS`] threads, the worker included, every helper drawn from the
    /// process-wide budget of `tessera-physics` (jobs at `Visible` priority or higher).
    Shared,
}

impl HierarchyThreads {
    /// The rule for a job at `priority`: [`HierarchyThreads::Worker`] for
    /// [`Priority::Prefetch`], [`HierarchyThreads::Shared`] otherwise.
    #[must_use]
    pub fn for_priority(priority: Priority) -> Self {
        match priority {
            Priority::Prefetch => Self::Worker,
            Priority::Visible | Priority::Immediate => Self::Shared,
        }
    }

    /// The most threads a build under this rule may use, its worker included (an upper
    /// bound: a shared build gets fewer helpers, down to none, when the budget is spent).
    #[must_use]
    pub fn max_threads(self) -> usize {
        match self {
            Self::Worker => 1,
            Self::Shared => MAX_BUILD_THREADS,
        }
    }
}

/// How many hierarchy builds a [`CollisionSystem`] ran with each thread cap since it was
/// made ([`CollisionSystem::hierarchy_builds`]). The counts follow the cap handed to the
/// hierarchy build, not the rule that chose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HierarchyBuildCounts {
    /// Builds capped at one thread, the building worker alone
    /// ([`HierarchyThreads::Worker`]).
    pub worker: u64,
    /// Builds capped at more than one thread, helpers drawn from the shared budget
    /// ([`HierarchyThreads::Shared`]).
    pub shared: u64,
    /// The thread cap of the last build (its worker included), `None` before the first.
    pub last_max_threads: Option<usize>,
}

/// The shared collision state of a world: property IDs, caches, limits (module docs).
pub struct CollisionSystem {
    pids: PhysicsPids,
    limits: CollisionLimits,
    statics: StaticOptions,
    objects: ObjectCache,
    cells: Mutex<HashMap<Did, Option<Arc<CellGeometry>>>>,
    max_cells: usize,
    builds_on_worker: AtomicU64,
    builds_shared: AtomicU64,
    /// Thread cap of the last hierarchy build; 0 before the first.
    last_max_threads: AtomicUsize,
}

impl std::fmt::Debug for CollisionSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionSystem")
            .field("pids", &self.pids)
            .field("limits", &self.limits)
            .field("statics", &self.statics)
            .field("objects", &self.objects.len())
            .field("cells", &self.cell_cache_len())
            .field("hierarchy_builds", &self.hierarchy_builds())
            .finish_non_exhaustive()
    }
}

impl CollisionSystem {
    /// A system with the §9.1 property IDs `pids`, per-set `limits` and the content-layer
    /// options of the static filter (§9.3, `entity-descriptor.md` §4.5.1).
    #[must_use]
    pub fn new(pids: PhysicsPids, limits: CollisionLimits, statics: StaticOptions) -> Self {
        Self {
            pids,
            limits,
            statics,
            objects: ObjectCache::new(
                DEFAULT_COLLISION_CACHE_ENTRIES,
                DEFAULT_COLLISION_CACHE_BYTES,
            ),
            cells: Mutex::new(HashMap::new()),
            max_cells: DEFAULT_CELL_CACHE_ENTRIES,
            builds_on_worker: AtomicU64::new(0),
            builds_shared: AtomicU64::new(0),
            last_max_threads: AtomicUsize::new(0),
        }
    }

    /// How many hierarchy builds ran under each thread rule (module docs, "Hierarchy build
    /// threads"); a diagnostic and test hook.
    #[must_use]
    pub fn hierarchy_builds(&self) -> HierarchyBuildCounts {
        HierarchyBuildCounts {
            worker: self.builds_on_worker.load(Ordering::Relaxed),
            shared: self.builds_shared.load(Ordering::Relaxed),
            last_max_threads: match self.last_max_threads.load(Ordering::Relaxed) {
                0 => None,
                n => Some(n),
            },
        }
    }

    /// The property IDs.
    #[must_use]
    pub fn pids(&self) -> &PhysicsPids {
        &self.pids
    }

    /// The per-set limits.
    #[must_use]
    pub fn limits(&self) -> &CollisionLimits {
        &self.limits
    }

    /// The content-layer options of the static filter.
    #[must_use]
    pub fn static_options(&self) -> &StaticOptions {
        &self.statics
    }

    /// The object-collision cache.
    #[must_use]
    pub fn object_cache(&self) -> &ObjectCache {
        &self.objects
    }

    /// Cell geometries cached (usable or not).
    #[must_use]
    pub fn cell_cache_len(&self) -> usize {
        self.cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// The resolved collision of `object` (a 0x47 entity, 0x1F template or 0x04 setup),
    /// cached. Never fails: problems are in [`ObjectCollision::error`]. A result built
    /// while a read failed for a reason that is not the data's is returned but not cached.
    #[must_use]
    pub fn object_collision(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        object: Did,
    ) -> Arc<ObjectCollision> {
        self.object_collision_tracked(source, schema, object).0
    }

    /// [`CollisionSystem::object_collision`], also telling whether a read failed for a
    /// reason that is not the data's (the result was then not cached).
    fn object_collision_tracked(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        object: Did,
    ) -> (Arc<ObjectCollision>, bool) {
        if let Some(hit) = self.objects.get(object) {
            return (hit, false);
        }
        let transient = Cell::new(false);
        let r = Arc::new(resolve_object_collision(
            object,
            |d| fetch(source, d, &transient),
            schema,
            &self.pids,
        ));
        if transient.get() {
            tracing::debug!(object = %format_args!("{:#010x}", object.0), "object collision not readable now; not cached");
            return (r, true);
        }
        (self.objects.insert(object, r), false)
    }

    /// Puts a decoded cell geometry into the cache (tests without an encoder for 0x01
    /// records).
    #[cfg(test)]
    pub(crate) fn seed_cell_geometry(&self, g: CellGeometry) {
        self.cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(g.did, Some(Arc::new(g)));
    }

    /// The decoded cell geometry `did` (prefix 0x01), cached. `None` when it is missing,
    /// does not decode or is not a cell geometry (cached), or cannot be read now (not
    /// cached, logged).
    #[must_use]
    pub fn cell_geometry(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
    ) -> Option<Arc<CellGeometry>> {
        self.cell_geometry_tracked(source, schema, did, &Cell::new(false))
    }

    /// [`CollisionSystem::cell_geometry`]; sets `transient` when the record cannot be read
    /// now (not the data's fault).
    fn cell_geometry_tracked(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
        transient: &Cell<bool>,
    ) -> Option<Arc<CellGeometry>> {
        if did.prefix() != CELL_GEOMETRY_PREFIX {
            return None;
        }
        if let Some(hit) = self
            .cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return hit.clone();
        }
        let decoded = match source.read_stamped(did) {
            Ok(Some((bytes, stamp))) => CellGeometry::parse_expecting(&bytes, did, stamp, schema)
                .map(Arc::new)
                .map_err(|e| {
                    tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "cell geometry does not decode");
                })
                .ok(),
            Ok(None) => {
                tracing::debug!(did = %format_args!("{:#010x}", did.0), "cell geometry missing");
                None
            }
            Err(e) if !e.is_data_error() => {
                tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "cell geometry not readable now");
                transient.set(true);
                return None;
            }
            Err(e) => {
                tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "cell geometry unreadable");
                None
            }
        };
        let mut cells = self.cells.lock().unwrap_or_else(PoisonError::into_inner);
        if cells.len() >= self.max_cells && !cells.contains_key(&did) {
            cells.clear();
        }
        cells.insert(did, decoded.clone());
        decoded
    }

    /// Builds one landblock's collision (module docs; `collision.md` §9.4 order (1), (4)
    /// and §12.1) with the hierarchy rule of a job at `Visible` priority: helpers from the
    /// shared budget ([`HierarchyThreads::Shared`]). Never fails; see [`BuildReport`] for
    /// what was left out. A job that knows its priority calls [`CollisionSystem::build_at`].
    #[must_use]
    pub fn build(&self, input: &CollisionInput<'_>) -> BlockCollision {
        self.build_at(input, Priority::Visible)
    }

    /// [`CollisionSystem::build`] for a job whose priority `priority` reports. It is asked
    /// exactly once, right before the hierarchy build starts (after the terrain, statics,
    /// generated objects, cells and water have been added), so that a job raised from
    /// `Prefetch` to `Visible` while it waited or while those stages ran builds with
    /// `Visible`'s rule: a `Prefetch` build stays on the calling worker (one thread),
    /// `Visible` and `Immediate` builds may draw helpers from the shared budget
    /// ([`HierarchyThreads::for_priority`], `collision.md` §12.1). A fixed [`Priority`],
    /// a closure or a [`JobContext`] may be passed ([`PrioritySource`]). The set does not
    /// depend on the priority.
    #[must_use]
    pub fn build_at(
        &self,
        input: &CollisionInput<'_>,
        priority: impl PrioritySource,
    ) -> BlockCollision {
        self.build_with(input, &priority)
    }

    /// Builds `b`'s hierarchy on at most `cap` threads, the calling worker included, and
    /// counts it: a cap of 1 builds on the caller alone and takes no helper; a larger cap
    /// is [`MAX_BUILD_THREADS`], the cap of the budgeted build, whose helpers come from the
    /// shared budget.
    fn build_hierarchy(&self, b: CollisionSetBuilder, cap: usize) -> CollisionSet {
        self.last_max_threads.store(cap, Ordering::Relaxed);
        if cap <= 1 {
            self.builds_on_worker.fetch_add(1, Ordering::Relaxed);
            b.build_with_threads(cap)
        } else {
            self.builds_shared.fetch_add(1, Ordering::Relaxed);
            b.build()
        }
    }

    /// [`CollisionSystem::build`] with the hierarchy built under the rule of the priority
    /// `priority` reports when the hierarchy build starts.
    fn build_with(
        &self,
        input: &CollisionInput<'_>,
        priority: &dyn PrioritySource,
    ) -> BlockCollision {
        let mut b = CollisionSetBuilder::new(self.limits);
        if let Some(id) = LandblockId::from_did(input.key.data_did()) {
            b.set_landblock(id);
        }
        // (1) Terrain and ceiling meshes (§6.1, §6.4). A grid never exceeds the default
        // limits; a refusal under tiny limits is logged.
        let terrain = input
            .terrain
            .cloned()
            .unwrap_or_else(|| Arc::new(GridMesh::terrain(input.data)));
        if let Err(e) = b.add_grid(terrain) {
            tracing::warn!(landblock = %input.key, error = %e, "terrain mesh left out of the collision set");
        }
        if let Some(c) = GridMesh::ceiling(input.data)
            && let Err(e) = b.add_grid(Arc::new(c))
        {
            tracing::warn!(landblock = %input.key, error = %e, "ceiling mesh left out of the collision set");
        }
        // Whether any read failed for a reason that is not the data's.
        let transient = Cell::new(false);
        let resolve = |o: Did| {
            let (r, t) = self.object_collision_tracked(input.source, input.schema, o);
            if t {
                transient.set(true);
            }
            r
        };
        // (4) Statics (§9.3) and generated objects (§9.4).
        if let Some(info) = input.info {
            b.add_statics(info, &self.pids, &self.statics, resolve);
        }
        let generated_bodies = b.add_generated(input.generated, resolve);
        // Cells (§5.3, §5.8.1).
        let geometry =
            |d: Did| self.cell_geometry_tracked(input.source, input.schema, d, &transient);
        if let Some(info) = input.info {
            b.add_cells(info, &self.pids, geometry);
        }
        // Water (`water.md` §6–§7, decision 0013).
        if let Some(w) = input.water
            && let Err(e) = w.apply_to(&mut b)
        {
            tracing::warn!(landblock = %input.key, error = %e, "liquid boxes left out of the collision set");
        }
        // The job's priority is read once, here, as the hierarchy build starts (§12.1): a
        // raise while the stages above ran counts, a change after this point does not.
        let at = priority.current_priority();
        let threads = HierarchyThreads::for_priority(at);
        // One cap decides both the call and the count (the test hook), so the count says
        // which cap the hierarchy build was given.
        let cap = threads.max_threads();
        tracing::trace!(landblock = %input.key, priority = %at, ?threads, cap, "collision hierarchy build starts");
        let set = self.build_hierarchy(b, cap);
        // Portals of the cells the set accepted (bounded by its cell limit and the per-set
        // portal budgets).
        let (portals, portals_left_out) = match input.info {
            Some(info) => cell_portals(info, &set, self.limits.max_cells, geometry),
            None => (Vec::new(), 0),
        };
        let cell_bounds = set
            .cells()
            .cells()
            .iter()
            .filter_map(|c| c.phantom_bounds)
            .reduce(Aabb::union);
        let portal_bytes: usize = portals
            .iter()
            .map(|c| {
                std::mem::size_of::<CellPortals>()
                    + c.portals.capacity() * std::mem::size_of::<Portal>()
                    + c.portals
                        .iter()
                        .map(|p| 12 * p.corners.capacity())
                        .sum::<usize>()
            })
            .sum();
        let heap_bytes = std::mem::size_of::<BlockCollision>()
            + set.heap_size()
            + portal_bytes
            + generated_bodies.capacity() * std::mem::size_of::<Option<BodyId>>();
        let complete = !transient.get();
        if !complete {
            tracing::debug!(landblock = %input.key, "collision set incomplete: a read failed for a reason that is not the data's");
        }
        BlockCollision {
            key: input.key,
            set: Arc::new(set),
            cells: portals,
            portals_left_out,
            cell_bounds,
            generated_bodies,
            complete,
            heap_bytes,
        }
    }
}

/// Reads a record for [`resolve_object_collision`]; sets `transient` when the read failed
/// for a reason that is not the data's.
fn fetch(
    source: &dyn WorldSource,
    did: Did,
    transient: &Cell<bool>,
) -> Result<Option<FetchedRecord>, String> {
    source
        .read_stamped(did)
        .map(|r| r.map(|(bytes, stamp)| FetchedRecord { bytes, stamp }))
        .map_err(|e| {
            if !e.is_data_error() {
                transient.set(true);
            }
            e.to_string()
        })
}

/// Everything one landblock's collision is built from.
#[derive(Clone, Copy)]
pub struct CollisionInput<'a> {
    /// The landblock.
    pub key: LandblockKey,
    /// Its data record (terrain and ceiling grids).
    pub data: &'a LandblockData,
    /// Its terrain mesh when already built (scenery generation builds it first, §9.4
    /// order (1)); otherwise it is built from `data`.
    pub terrain: Option<&'a Arc<GridMesh>>,
    /// Its info record (statics and cells).
    pub info: Option<&'a LandblockInfo>,
    /// Its generated scenery objects, in generated-list order.
    pub generated: &'a [GeneratedObject],
    /// Its water plane and liquid boxes.
    pub water: Option<&'a BlockWater>,
    /// Records (object chains, 0x66 meshes, 0x78 sets, 0x01 cell geometries).
    pub source: &'a dyn WorldSource,
    /// Property schema of the 0x47, 0x78 and 0x01 property sets.
    pub schema: &'a dyn PropertySchema,
}

impl std::fmt::Debug for CollisionInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionInput")
            .field("key", &self.key)
            .field("info", &self.info.is_some())
            .field("generated", &self.generated.len())
            .field("water", &self.water.is_some())
            .finish_non_exhaustive()
    }
}

/// One opening of a cell (`collision.md` §5.5, §5.8): neighbour entry *k* of the cell
/// describes portal *k* of its geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Portal {
    /// Position *k* in the cell's neighbour list and its geometry's portal list.
    pub ordinal: u16,
    /// The neighbour cell seen through the portal.
    pub neighbour: u16,
    /// The portal index in the neighbour's geometry that is the same opening.
    pub back_portal: u16,
    /// The neighbour entry's flag byte (meaning OPEN, §5.8).
    pub flag: u8,
    /// The portal polygon in the landblock frame; empty when the cell cannot be placed, was
    /// refused by the collision set, its geometry has no portal *k*, or the per-set corner
    /// budget ([`MAX_SET_PORTAL_CORNERS`]) is spent.
    pub corners: Vec<[f32; 3]>,
}

/// A landblock-info cell with its portal links (`collision.md` §5.8).
#[derive(Debug, Clone, PartialEq)]
pub struct CellPortals {
    /// The cell number.
    pub index: u16,
    /// The 0x01 geometry it places.
    pub geometry: Did,
    /// Its placement in the landblock frame (a cell without a position offset lies at the
    /// landblock origin, `landblock.md` §4.4 L42; `None` for an unusable rotation or an
    /// offset out of range).
    pub placement: Option<CellPlacement>,
    /// Whether the geometry was available (decoded).
    pub geometry_available: bool,
    /// One entry per neighbour, in stored order (at most [`MAX_PORTALS_PER_CELL`], and
    /// within the per-set budget [`MAX_SET_PORTALS`]).
    pub portals: Vec<Portal>,
}

/// The cells of `info` with their portals; `geometry` returns a decoded 0x01 record.
///
/// At most `max_cells` cells are kept, in stored order (the set's own cell limit). Only
/// cells that `set` accepted (placed with the same number and geometry) get portal
/// polygons; the others keep their links. Over all cells at most [`MAX_SET_PORTALS`] links
/// and [`MAX_SET_PORTAL_CORNERS`] corners are kept. Returns the cells and the number of
/// portals left out or kept without their corners because of those budgets.
fn cell_portals(
    info: &LandblockInfo,
    set: &CollisionSet,
    max_cells: usize,
    mut geometry: impl FnMut(Did) -> Option<Arc<CellGeometry>>,
) -> (Vec<CellPortals>, u32) {
    let cells = info.cells();
    let mut out = Vec::with_capacity(cells.len().min(max_cells));
    let mut links = 0usize;
    let mut corners_kept = 0usize;
    let mut left_out = 0u32;
    for (n, c) in cells.iter().enumerate() {
        if n >= max_cells {
            let rest: usize = cells[n..]
                .iter()
                .map(|c| c.neighbours.len().min(MAX_PORTALS_PER_CELL))
                .sum();
            left_out = left_out.saturating_add(u32::try_from(rest).unwrap_or(u32::MAX));
            break;
        }
        let accepted = set
            .cells()
            .get(c.index)
            .is_some_and(|p| p.geometry == c.geometry);
        let g = geometry(c.geometry);
        let placement = CellPlacement::of_cell(c).ok();
        let wanted = c.neighbours.len().min(MAX_PORTALS_PER_CELL);
        let room = MAX_SET_PORTALS.saturating_sub(links);
        let kept = wanted.min(room);
        left_out = left_out.saturating_add(u32::try_from(wanted - kept).unwrap_or(u32::MAX));
        links += kept;
        let mut portals = Vec::with_capacity(kept);
        for (k, nb) in c.neighbours.iter().take(kept).enumerate() {
            let polygon = match (&g, &placement) {
                (Some(g), Some(pl)) if accepted => g.portals.get(k).map(|p| (pl, p)),
                _ => None,
            };
            let corners = match polygon {
                Some((pl, p)) => {
                    let m = p.corners.len().min(MAX_PORTAL_CORNERS);
                    if corners_kept.saturating_add(m) > MAX_SET_PORTAL_CORNERS {
                        left_out = left_out.saturating_add(1);
                        Vec::new()
                    } else {
                        corners_kept += m;
                        p.corners
                            .iter()
                            .take(m)
                            .map(|&v| to_landblock(pl, v))
                            .collect()
                    }
                }
                None => Vec::new(),
            };
            portals.push(Portal {
                ordinal: u16::try_from(k).unwrap_or(u16::MAX),
                neighbour: nb.cell,
                back_portal: nb.back_reference,
                flag: nb.flag,
                corners,
            });
        }
        out.push(CellPortals {
            index: c.index,
            geometry: c.geometry,
            placement,
            geometry_available: g.is_some(),
            portals,
        });
    }
    (out, left_out)
}

#[allow(clippy::cast_possible_truncation)]
fn to_landblock(placement: &CellPlacement, v: [f32; 3]) -> [f32; 3] {
    let p = placement.to_landblock([f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]);
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

/// One landblock's collision (module docs). Immutable; queried from any thread.
#[derive(Debug, Clone)]
pub struct BlockCollision {
    /// The landblock.
    pub key: LandblockKey,
    /// The collision set in the landblock frame, keyed by its `LandblockId`
    /// ([`tessera_physics::geometry::CollisionView::push_landblock`]).
    pub set: Arc<CollisionSet>,
    /// The cells of landblock info with their portals, in stored order (at most
    /// [`CollisionLimits::max_cells`]).
    pub cells: Vec<CellPortals>,
    /// Portal links left out, or kept without their polygon, because of the per-set
    /// budgets ([`MAX_SET_PORTALS`], [`MAX_SET_PORTAL_CORNERS`]) or the cell limit.
    pub portals_left_out: u32,
    /// Bounds of every placed cell's phantom in the landblock frame (`collision.md`
    /// §5.8.1), `None` without placed cells. Cells are not confined to their landblock's
    /// square; [`crate::WorldStreamer::cell_at`] uses this box to find the landblocks
    /// whose cells can contain a point.
    pub cell_bounds: Option<Aabb>,
    /// Per generated object (same order as the bundle's generated list), the body it got.
    pub generated_bodies: Vec<Option<BodyId>>,
    /// Whether every record could be read: false when a read failed for a reason that is
    /// not the data's (a full queue, a shut-down pipeline), so objects or cells may be
    /// missing that the data has. The streamer rebuilds an incomplete set when the
    /// landblock loads again.
    pub complete: bool,
    /// Estimated heap bytes (the residency charge).
    pub heap_bytes: usize,
}

impl BlockCollision {
    /// What building did.
    #[must_use]
    pub fn report(&self) -> &BuildReport {
        self.set.report()
    }

    /// The cell containing landblock-frame `p` (`collision.md` §5.8.1), or `None`
    /// (outdoors).
    #[must_use]
    pub fn cell_at(&self, p: [f32; 3]) -> Option<&CellPortals> {
        let hit = self.set.cell_at(p)?;
        self.cell(hit.cell)
    }

    /// The cell with number `index` (the first, if several share it).
    #[must_use]
    pub fn cell(&self, index: u16) -> Option<&CellPortals> {
        self.cells.iter().find(|c| c.index == index)
    }
}

#[cfg(test)]
pub(crate) mod tests;
