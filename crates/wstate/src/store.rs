// SPDX-License-Identifier: GPL-3.0-or-later
//! Loading object-state records through a record source: the class library, a bounded
//! cache of decoded records, cross-record resolution of DID items, and [`GameObject`]s
//! (object state + property set).
//!
//! Spec: `docs/specs/wlib.md` §2, `docs/specs/wstate.md` §2, §8.2, §12.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use tessera_dat::ArchiveSet;
use tessera_formats::Did;
use tessera_props::{GameData, PropertySet, RecordSource};

use crate::error::{Result, WStateError};
use crate::library::{ClassLibrary, LIBRARY_DID};
use crate::state::{DecodeContext, Import, ObjectHeader, ObjectState, is_object_did};

/// Root DID-map label of the class-library map (`property-system.md` §8).
pub const WLIB_MAP_LABEL: &str = "WLIB";
/// Label of the shipped library in the WLIB map (`wlib.md` §2).
pub const WLIB_CLIENT_LABEL: &str = "Client";
/// Root DID-map label of the well-known object map (`wstate.md` §2).
pub const WEENIE_MAP_LABEL: &str = "WEENIECONTENT";

/// Default for [`CacheLimits::records`]: most decoded records kept by an
/// [`ObjectStore`]'s cache.
pub const MAX_CACHED_STATES: usize = 4096;
/// Default for [`CacheLimits::max_payload`]: records whose payload is larger than this are
/// decoded but never cached.
pub const MAX_CACHED_PAYLOAD: usize = 1 << 20;
/// Default for [`CacheLimits::bytes`]: estimated heap the cache may hold (128 MiB).
pub const DEFAULT_CACHE_BYTES: usize = 128 << 20;

/// Bounds of an [`ObjectStore`]'s cache of decoded records.
///
/// The cache evicts the least recently used records when either bound would be exceeded.
/// A record's cost is an estimate of its decoded heap: twice the payload length plus the
/// fixed size of its items (see [`ObjectStore::cached_bytes`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    /// Most records kept (0 disables the cache).
    pub records: usize,
    /// Most estimated bytes kept.
    pub bytes: usize,
    /// Records whose payload is larger than this are never cached.
    pub max_payload: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            records: MAX_CACHED_STATES,
            bytes: DEFAULT_CACHE_BYTES,
            max_payload: MAX_CACHED_PAYLOAD,
        }
    }
}

#[derive(Debug)]
struct CacheEntry {
    state: Arc<ObjectState>,
    cost: usize,
    tick: u64,
}

/// Least-recently-used cache of decoded records, bounded by count and estimated bytes.
#[derive(Debug, Default)]
struct StateCache {
    limits: CacheLimits,
    entries: HashMap<Did, CacheEntry>,
    /// Use order: tick → DID; the first entry is the least recently used.
    order: BTreeMap<u64, Did>,
    tick: u64,
    bytes: usize,
}

impl StateCache {
    fn new(limits: CacheLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    fn get(&mut self, did: Did) -> Option<Arc<ObjectState>> {
        let tick = self.next_tick();
        let e = self.entries.get_mut(&did)?;
        self.order.remove(&e.tick);
        e.tick = tick;
        self.order.insert(tick, did);
        Some(Arc::clone(&e.state))
    }

    fn insert(&mut self, did: Did, state: Arc<ObjectState>, cost: usize) {
        self.remove(did);
        if self.limits.records == 0 || cost > self.limits.bytes {
            return;
        }
        while self.entries.len() >= self.limits.records
            || self.bytes.saturating_add(cost) > self.limits.bytes
        {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(e) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(e.cost);
            }
        }
        let tick = self.next_tick();
        self.order.insert(tick, did);
        self.entries.insert(did, CacheEntry { state, cost, tick });
        self.bytes = self.bytes.saturating_add(cost);
    }

    fn remove(&mut self, did: Did) -> bool {
        match self.entries.remove(&did) {
            Some(e) => {
                self.order.remove(&e.tick);
                self.bytes = self.bytes.saturating_sub(e.cost);
                true
            }
            None => false,
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    fn set_limits(&mut self, limits: CacheLimits) {
        self.limits = limits;
        while self.entries.len() > limits.records || self.bytes > limits.bytes {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(e) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(e.cost);
            }
        }
    }
}

/// Estimated heap of a decoded record: twice its payload (native bodies and strings are at
/// most a small multiple of their stored size) plus the fixed size of its items and labels.
fn estimated_cost(payload: usize, state: &ObjectState) -> usize {
    let items = state
        .bundle
        .items()
        .len()
        .saturating_mul(std::mem::size_of::<crate::state::Item>());
    let labels = state.bundle.labels().len().saturating_mul(4);
    payload
        .saturating_mul(2)
        .saturating_add(items)
        .saturating_add(labels)
        .saturating_add(std::mem::size_of::<ObjectState>())
}

/// A game object: DID, class, object state and property set (`ARCHITECTURE` §7.4).
#[derive(Debug, Clone)]
pub struct GameObject {
    /// The object's DID.
    pub did: Did,
    /// Class index of the root item.
    pub class: u32,
    /// Decoded object state.
    pub state: Arc<ObjectState>,
    /// Properties from DID + 0x09000000, if the object has a property set.
    pub properties: Option<PropertySet>,
}

/// Object-state access over a record source, normally the [`ArchiveSet`] of an install.
///
/// The class library is loaded on first use (through the WLIB DID map's "Client" label,
/// falling back to 0x56000000) and kept. Decoded records are kept in a least-recently-used
/// cache bounded by [`CacheLimits`] (record count and estimated bytes; large records are
/// never cached); [`ObjectStore::evict`] drops records a caller no longer needs, for
/// example when a landblock unloads. Property access goes through the embedded
/// [`GameData`].
///
/// `ObjectStore` is `Send + Sync` when its source is.
#[derive(Debug)]
pub struct ObjectStore<S = ArchiveSet> {
    data: GameData<S>,
    library: OnceLock<Result<Arc<ClassLibrary>>>,
    library_load: Mutex<()>,
    cache: Mutex<StateCache>,
}

impl ObjectStore<ArchiveSet> {
    /// Opens every archive of the install in `dir` read-only.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::from_game_data(GameData::open(dir)?))
    }
}

impl<S: RecordSource> ObjectStore<S> {
    /// Wraps a record source. Nothing is read until first use.
    pub fn new(source: S) -> Self {
        Self::from_game_data(GameData::new(source))
    }

    /// Wraps an existing property facade (sharing its registry and caches).
    pub fn from_game_data(data: GameData<S>) -> Self {
        Self {
            data,
            library: OnceLock::new(),
            library_load: Mutex::new(()),
            cache: Mutex::new(StateCache::new(CacheLimits::default())),
        }
    }

    /// Sets the cache bounds (evicting least recently used records if they are now over).
    #[must_use]
    pub fn with_cache_limits(self, limits: CacheLimits) -> Self {
        self.set_cache_limits(limits);
        self
    }

    /// Changes the cache bounds, evicting least recently used records as needed.
    pub fn set_cache_limits(&self, limits: CacheLimits) {
        self.lock_cache().set_limits(limits);
    }

    /// The cache bounds in use.
    pub fn cache_limits(&self) -> CacheLimits {
        self.lock_cache().limits
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, StateCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Uses an already loaded class library.
    #[must_use]
    pub fn with_library(self, library: Arc<ClassLibrary>) -> Self {
        let lock = OnceLock::new();
        let _ = lock.set(Ok(library));
        Self {
            library: lock,
            ..self
        }
    }

    /// The property facade (registry, DID maps, property sets).
    pub fn game_data(&self) -> &GameData<S> {
        &self.data
    }

    /// The DID of the class library: the WLIB map's "Client" label, else 0x56000000.
    pub fn library_did(&self) -> Did {
        match self.data.lookup_path(&[WLIB_MAP_LABEL, WLIB_CLIENT_LABEL]) {
            Ok(Some(did)) if did.prefix() == 0x56 => did,
            Ok(_) => LIBRARY_DID,
            Err(e) => {
                tracing::debug!(error = %e, "WLIB map lookup failed; using the default DID");
                LIBRARY_DID
            }
        }
    }

    /// The class library, loaded on first use. A missing or undecodable library is cached
    /// like a loaded one; a failed read is not.
    pub fn library(&self) -> Result<Arc<ClassLibrary>> {
        if let Some(r) = self.library.get() {
            return r.clone();
        }
        let _load = self.library_load.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = self.library.get() {
            return r.clone();
        }
        let did = self.library_did();
        let loaded = match self.data.read(did) {
            Err(e) => return Err(e.into()),
            Ok(None) => Err(WStateError::Missing {
                did,
                what: "class library",
            }),
            Ok(Some(bytes)) => ClassLibrary::parse(&bytes)
                .map(Arc::new)
                .map_err(|e| e.in_record(did)),
        };
        if let Err(e) = &loaded {
            tracing::warn!(error = %e, "class library failed to load");
        }
        self.library.get_or_init(|| loaded).clone()
    }

    /// Reads only the header (DID and root class) of object `did`; `Ok(None)` if absent.
    pub fn header(&self, did: Did) -> Result<Option<ObjectHeader>> {
        check_object_did(did)?;
        let Some(bytes) = self.data.read(did)? else {
            return Ok(None);
        };
        ObjectHeader::parse(&bytes)
            .map(Some)
            .map_err(|e| e.in_record(did))
    }

    /// Reads the header and the import list of object `did` (the prefetch list,
    /// `wstate.md` §3.2) without decoding its bundle; `Ok(None)` if absent. Imports with
    /// [`Import::FLAG_DID_ITEM`] name the records this one's DID items point to.
    pub fn imports(&self, did: Did) -> Result<Option<(ObjectHeader, Vec<Import>)>> {
        check_object_did(did)?;
        let Some(bytes) = self.data.read(did)? else {
            return Ok(None);
        };
        ObjectHeader::parse_with_imports(&bytes)
            .map(|(h, _, imports)| Some((h, imports)))
            .map_err(|e| e.in_record(did))
    }

    /// Decodes object `did` (cached); `Ok(None)` if absent.
    pub fn state(&self, did: Did) -> Result<Option<Arc<ObjectState>>> {
        check_object_did(did)?;
        if let Some(s) = self.lock_cache().get(did) {
            return Ok(Some(s));
        }
        let Some(bytes) = self.data.read(did)? else {
            return Ok(None);
        };
        let state = Arc::new(self.decode(did, &bytes)?);
        let mut cache = self.lock_cache();
        if bytes.len() <= cache.limits.max_payload {
            let cost = estimated_cost(bytes.len(), &state);
            cache.insert(did, Arc::clone(&state), cost);
        }
        Ok(Some(state))
    }

    /// Decodes an object-state payload that was read elsewhere, with this store's library
    /// and property registry. Nothing is cached.
    pub fn decode(&self, did: Did, bytes: &[u8]) -> Result<ObjectState> {
        let library = self.library()?;
        let registry = self.data.registry()?;
        ObjectState::parse(bytes, did, DecodeContext::new(&library, registry.as_ref()))
            .map_err(|e| e.in_record(did))
    }

    /// Number of cached records.
    pub fn cached(&self) -> usize {
        self.lock_cache().entries.len()
    }

    /// Estimated heap held by the cached records (the sum of their costs, see
    /// [`CacheLimits`]). Records still referenced elsewhere stay alive after eviction and
    /// are not counted.
    pub fn cached_bytes(&self) -> usize {
        self.lock_cache().bytes
    }

    /// Whether record `did` is cached.
    pub fn is_cached(&self, did: Did) -> bool {
        self.lock_cache().entries.contains_key(&did)
    }

    /// Drops the cached records of `dids` (for example the objects of a landblock that
    /// was unloaded). Returns how many were cached.
    pub fn evict(&self, dids: impl IntoIterator<Item = Did>) -> usize {
        let mut cache = self.lock_cache();
        dids.into_iter().filter(|&d| cache.remove(d)).count()
    }

    /// Drops every cached record.
    pub fn clear_cache(&self) {
        self.lock_cache().clear();
    }

    /// The properties of object `did` (from DID + 0x09000000); `Ok(None)` if it has none.
    pub fn properties(&self, did: Did) -> Result<Option<PropertySet>> {
        check_object_did(did)?;
        Ok(self.data.object_properties(did)?)
    }

    /// The game object `did`: state and properties. `Ok(None)` if the state record is
    /// absent.
    pub fn object(&self, did: Did) -> Result<Option<GameObject>> {
        let Some(state) = self.state(did)? else {
            return Ok(None);
        };
        let properties = self.properties(did)?;
        Ok(Some(GameObject {
            did,
            class: state.root_class,
            state,
            properties,
        }))
    }

    /// Follows a DID item to the record it names (`wstate.md` §8.2 step 3). A DID item
    /// whose record is missing is [`WStateError::Missing`].
    pub fn follow_did(&self, did: Did) -> Result<Arc<ObjectState>> {
        self.state(did)?.ok_or(WStateError::Missing {
            did,
            what: "object state",
        })
    }

    /// The object named `label` in the WEENIECONTENT DID map (for example
    /// "RegionControl"), if present.
    pub fn weenie(&self, label: &str) -> Result<Option<Did>> {
        Ok(self.data.lookup_path(&[WEENIE_MAP_LABEL, label])?)
    }

    /// The singleton record of class `class` (0x76000000 + class), if the library lists
    /// the class as a singleton and the record exists.
    pub fn singleton(&self, class: u32) -> Result<Option<Arc<ObjectState>>> {
        match self.library()?.singleton_did(class) {
            Some(did) => self.state(did),
            None => Ok(None),
        }
    }
}

fn check_object_did(did: Did) -> Result<()> {
    if is_object_did(did) {
        Ok(())
    } else {
        Err(WStateError::NotObjectDid { did })
    }
}

/// Every object-state DID (0x70000000–0x77FFFFFF) in the archive set, ascending, without
/// duplicates. Only the directory trees are walked; no record is read.
pub fn object_dids(set: &ArchiveSet) -> std::result::Result<Vec<Did>, tessera_dat::Error> {
    let mut dids = Vec::new();
    for member in set.members() {
        for entry in member.archive.entries_range(0x7000_0000..0x7800_0000) {
            dids.push(Did(entry?.did));
        }
    }
    dids.sort_unstable();
    dids.dedup();
    Ok(dids)
}
