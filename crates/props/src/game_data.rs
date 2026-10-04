// SPDX-License-Identifier: GPL-3.0-or-later
//! [`GameData`]: property access over an archive set, with lazily loaded and cached
//! registry, enum mappers and DID maps.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::enum_mapper::ENUM_BASE_CHAIN_LIMIT;
use tessera_formats::{
    DataIdEntry, DataIdMap, Did, DidKind, EnumMapper, FormatError, Record, StringInfo,
    is_property_set_did, object_property_set_did,
};

use crate::error::{PropsError, Result};
use crate::labels::{BitLabel, EnumLabel, Label, ResolvedProperty};
use crate::registry::{EnumSource, PROPERTY_DICTIONARY_DID, PropertyRegistry};
use crate::set::PropertySet;
use crate::source::RecordSource;
use crate::value::Value;

/// DID of the root DID map (`records.md` §8.2). The DIDs of its sub-maps are read at run
/// time through their labels and never hard-coded.
pub const ROOT_DID_MAP: Did = Did(0x2800_0000);

/// Root DID-map label of the map of named enum mappers (`property-system.md` §8.1).
pub const EMAPPER_LABEL: &str = "EMAPPER";

/// EMAPPER label of the DB-object-type enum mapper (`property-system.md` §9).
pub const DBO_TYPE_LABEL: &str = "DBOType";

/// Largest number of records each cache keeps (enum mappers and DID maps, hits and misses
/// alike). An install has about 735 mappers and 24 DID maps; past the cap, records are
/// still loaded but no longer cached.
pub const MAX_CACHED_RECORDS: usize = 16_384;

/// Longest label path [`GameData::lookup_path`] follows.
pub const MAX_PATH_LABELS: usize = 16;

/// Deepest level [`GameData::resolve`] descends to; containers below it are returned
/// without resolved children. Decoded sets stay far above it (observed depth 9).
pub const RESOLVE_MAX_DEPTH: usize = 64;

/// Most set bits one bitfield value gets labels for ([`Label::Bits`]); further set bits
/// are only counted (`omitted`). Bitfields in game data have a few dozen labelled bits;
/// the cap keeps a crafted wide type-22 value from turning each stored byte into eight
/// labels.
pub const MAX_LABELLED_BITS: usize = 256;

/// Most bit labels one [`GameData::resolve`] call makes over the whole tree; once spent,
/// further bitfields are returned with all their set bits `omitted`.
pub const RESOLVE_MAX_BIT_LABELS: usize = 65_536;

type Cached<T> = Result<Option<Arc<T>>>;

/// A decoded enum mapper with its texts in shared form, so that labels point into the
/// cache instead of copying a name or display text per value.
#[derive(Debug)]
struct MapperEntry {
    mapper: Arc<EnumMapper>,
    names: HashMap<u32, Arc<str>>,
    displays: HashMap<u32, Arc<StringInfo>>,
}

impl MapperEntry {
    fn new(mapper: EnumMapper) -> Self {
        // First stored entry per key, as `EnumMapper::name` and `display` answer.
        let mut names = HashMap::with_capacity(mapper.names().len());
        for (k, n) in mapper.names() {
            names
                .entry(*k)
                .or_insert_with(|| Arc::<str>::from(n.as_str()));
        }
        let mut displays = HashMap::with_capacity(mapper.displays().len());
        for (k, d) in mapper.displays() {
            displays.entry(*k).or_insert_with(|| Arc::new(d.clone()));
        }
        Self {
            mapper: Arc::new(mapper),
            names,
            displays,
        }
    }
}

/// The label of `key` from a loaded base chain: name and display each from the first
/// mapper that has one.
fn label_from_chain(chain: &[Arc<MapperEntry>], key: u32) -> EnumLabel {
    let mut label = EnumLabel {
        key,
        name: None,
        display: None,
    };
    for m in chain {
        if label.name.is_none() {
            label.name = m.names.get(&key).cloned();
        }
        if label.display.is_none() {
            label.display = m.displays.get(&key).cloned();
        }
        if label.name.is_some() && label.display.is_some() {
            break;
        }
    }
    label
}

/// A cache of decoded records by DID, bounded by [`MAX_CACHED_RECORDS`].
#[derive(Debug)]
struct RecordCache<T> {
    map: Mutex<HashMap<Did, Cached<T>>>,
}

impl<T> Default for RecordCache<T> {
    fn default() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }
}

impl<T> RecordCache<T> {
    fn get(&self, did: Did) -> Option<Cached<T>> {
        let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&did).cloned()
    }

    fn put(&self, did: Did, value: Cached<T>) -> Cached<T> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() < MAX_CACHED_RECORDS || map.contains_key(&did) {
            // A concurrent loader may have stored the same record; keep the first.
            return map.entry(did).or_insert(value).clone();
        }
        value
    }

    fn len(&self) -> usize {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Property access over a record source, normally the [`ArchiveSet`] of an install.
///
/// Everything is loaded lazily on first use and cached: the [`PropertyRegistry`] (DID
/// 0x34000000), enum mappers (0x23…) and DID maps (0x28…). Property sets themselves are
/// decoded on every call and not cached; callers that revisit sets keep their own cache.
///
/// Failures are cached when they follow from the stored bytes (a record that is missing
/// or does not decode), since rereading cannot change them while the archives are open;
/// a failed read ([`PropsError::Source`], such as an I/O error) is not cached, and the
/// next call reads the record again. Open a new `GameData` after the archives change.
///
/// `GameData` is `Send + Sync` when its source is, so one instance can serve worker
/// threads.
///
/// ```no_run
/// use tessera_props::GameData;
///
/// # fn main() -> Result<(), tessera_props::PropsError> {
/// let data = GameData::open("/path/to/install")?;
/// if let Some(set) = data.properties(0x7000_0001.into())? {
///     for p in data.resolve(&set)? {
///         println!("{} = {:?} {}", p.name.as_deref().unwrap_or("?"), p.value, p.label);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct GameData<S = ArchiveSet> {
    source: S,
    /// The registry, or the deterministic failure to load it.
    registry: OnceLock<Result<Arc<PropertyRegistry>>>,
    /// Serializes registry loads, so concurrent first callers parse it once.
    registry_load: Mutex<()>,
    /// Whether [`GameData::label`] has logged a registry failure yet.
    registry_warned: AtomicBool,
    mappers: RecordCache<MapperEntry>,
    maps: RecordCache<DataIdMap>,
}

impl GameData<ArchiveSet> {
    /// Opens every archive of the install in `dir` read-only.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let set = ArchiveSet::open(dir, SetOptions::default()).map_err(|e| PropsError::Open {
            path: dir.to_path_buf(),
            source: Arc::new(e),
        })?;
        Ok(Self::new(set))
    }
}

impl<S: RecordSource> GameData<S> {
    /// Wraps a record source. Nothing is read until first use.
    pub fn new(source: S) -> Self {
        Self {
            source,
            registry: OnceLock::new(),
            registry_load: Mutex::new(()),
            registry_warned: AtomicBool::new(false),
            mappers: RecordCache::default(),
            maps: RecordCache::default(),
        }
    }

    /// Uses an already loaded registry instead of reading DID 0x34000000.
    #[must_use]
    pub fn with_registry(self, registry: Arc<PropertyRegistry>) -> Self {
        let lock = OnceLock::new();
        let _ = lock.set(Ok(registry));
        Self {
            registry: lock,
            ..self
        }
    }

    /// The record source.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Consumes the facade, returning the source.
    pub fn into_source(self) -> S {
        self.source
    }

    /// Reads a record payload; `Ok(None)` if no archive holds it.
    pub fn read(&self, did: Did) -> Result<Option<Vec<u8>>> {
        self.source
            .read_record(did)
            .map_err(|e| PropsError::source(did, e))
    }

    /// The property registry, loaded on first use.
    ///
    /// A missing or undecodable dictionary is cached like a loaded one: later calls
    /// return the same error without reading the record again. A failed read
    /// ([`PropsError::Source`]) is not cached; the next call tries again. Concurrent first
    /// callers wait for one load instead of each parsing the dictionary.
    pub fn registry(&self) -> Result<&Arc<PropertyRegistry>> {
        if let Some(r) = self.registry.get() {
            return r.as_ref().map_err(Clone::clone);
        }
        let _load = self.registry_load.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = self.registry.get() {
            return r.as_ref().map_err(Clone::clone);
        }
        let loaded = self.load_registry();
        match loaded {
            Err(e @ PropsError::Source { .. }) => Err(e),
            other => {
                if let Err(e) = &other {
                    tracing::warn!(error = %e, "property registry failed to load");
                }
                self.registry
                    .get_or_init(|| other)
                    .as_ref()
                    .map_err(Clone::clone)
            }
        }
    }

    fn load_registry(&self) -> Result<Arc<PropertyRegistry>> {
        let bytes = self
            .read(PROPERTY_DICTIONARY_DID)?
            .ok_or(PropsError::Missing {
                did: PROPERTY_DICTIONARY_DID,
                what: "property dictionary",
            })?;
        let reg = Arc::new(PropertyRegistry::parse(&bytes)?);
        tracing::debug!(definitions = reg.len(), "property registry loaded");
        Ok(reg)
    }

    /// Number of enum mappers and DID maps currently cached (hits and misses).
    pub fn cached_records(&self) -> usize {
        self.mappers.len() + self.maps.len()
    }

    /// Loads record `did` through `cache`. `build` decodes the payload and returns the
    /// DID stored in it with the cached value. Read failures are returned uncached.
    fn load_cached<T>(
        &self,
        cache: &RecordCache<T>,
        did: Did,
        build: impl FnOnce(&[u8]) -> std::result::Result<(Did, T), FormatError>,
        what: &'static str,
    ) -> Cached<T> {
        if let Some(hit) = cache.get(did) {
            return hit;
        }
        let loaded = match self.read(did) {
            Ok(None) => Ok(None),
            Ok(Some(bytes)) => match build(&bytes) {
                Ok((found, rec)) if found == did => Ok(Some(Arc::new(rec))),
                Ok((found, _)) => Err(PropsError::decode(
                    did,
                    what,
                    FormatError::DidMismatch {
                        expected: did.0,
                        found: found.0,
                    },
                )),
                Err(e) => Err(PropsError::decode(did, what, e)),
            },
            Err(e) => {
                // Possibly transient: not cached, the next call reads again.
                tracing::warn!(%did, error = %e, "{what} could not be read");
                return Err(e);
            }
        };
        if let Err(e) = &loaded {
            tracing::warn!(%did, error = %e, "{what} failed to load");
        }
        cache.put(did, loaded)
    }

    fn mapper_entry(&self, did: Did) -> Cached<MapperEntry> {
        if did.kind() != DidKind::EnumMapper {
            return Ok(None);
        }
        self.load_cached(
            &self.mappers,
            did,
            |b| EnumMapper::parse(b).map(|m| (m.did(), MapperEntry::new(m))),
            EnumMapper::NAME,
        )
    }

    /// The enum mapper `did` (prefix 0x23), loaded and cached; `Ok(None)` if absent or
    /// if `did` is not an enum-mapper DID (some EMAPPER labels name DID maps, §8.2).
    ///
    /// A record that is absent or fails to decode is remembered and reported again
    /// without rereading; a failed read is not (see [`GameData`]).
    pub fn enum_mapper(&self, did: Did) -> Result<Option<Arc<EnumMapper>>> {
        Ok(self.mapper_entry(did)?.map(|e| Arc::clone(&e.mapper)))
    }

    /// The DID map `did` (prefix 0x28), loaded and cached; `Ok(None)` if absent or if
    /// `did` is not a DID-map DID. Failures are cached as for
    /// [`GameData::enum_mapper`].
    pub fn did_map(&self, did: Did) -> Result<Option<Arc<DataIdMap>>> {
        if did.kind() != DidKind::DataIdMap {
            return Ok(None);
        }
        self.load_cached(
            &self.maps,
            did,
            |b| DataIdMap::parse(b).map(|m| (m.did(), m)),
            DataIdMap::NAME,
        )
    }

    /// The root DID map (0x28000000).
    pub fn root_did_map(&self) -> Result<Arc<DataIdMap>> {
        self.did_map(ROOT_DID_MAP)?.ok_or(PropsError::Missing {
            did: ROOT_DID_MAP,
            what: DataIdMap::NAME,
        })
    }

    /// Follows a path of DID-map labels from the root map (`property-system.md` §8):
    /// each label but the last must lead to another DID map. Returns the DID the last
    /// label names, or `Ok(None)` when a label is missing, names the null DID, or an
    /// intermediate target is not a DID map or is absent (a dangling entry, §8.3). An empty path, or one longer
    /// than [`MAX_PATH_LABELS`], yields `Ok(None)`.
    ///
    /// Example: `["EMAPPER", "DBOType"]` gives the DB-object-type enum mapper.
    pub fn lookup_path(&self, labels: &[&str]) -> Result<Option<Did>> {
        if labels.is_empty() || labels.len() > MAX_PATH_LABELS {
            return Ok(None);
        }
        let mut map = self.root_did_map()?;
        let mut iter = labels.iter().peekable();
        while let Some(label) = iter.next() {
            let Some(did) = map.by_label(label).filter(|d| !d.is_null()) else {
                return Ok(None);
            };
            if iter.peek().is_none() {
                return Ok(Some(did));
            }
            match self.did_map(did)? {
                Some(next) => map = next,
                None => return Ok(None),
            }
        }
        Ok(None)
    }

    /// The sub-map that the root map names `label` (for example "EMAPPER").
    pub fn did_map_by_label(&self, label: &str) -> Result<Option<Arc<DataIdMap>>> {
        match self.lookup_path(&[label])? {
            Some(did) => self.did_map(did),
            None => Ok(None),
        }
    }

    /// The enum mapper that the EMAPPER DID map names `name` (both groups, §8.3).
    pub fn enum_mapper_by_name(&self, name: &str) -> Result<Option<Arc<EnumMapper>>> {
        match self.lookup_path(&[EMAPPER_LABEL, name])? {
            Some(did) => self.enum_mapper(did),
            None => Ok(None),
        }
    }

    /// The label of `key` in enum mapper `mapper` and its base chain
    /// (`records.md` §7.1). The name and the display text are each taken from the first
    /// mapper in the chain that has one.
    ///
    /// A base mapper that is missing, fails to read or fails to decode ends the search,
    /// and what the mappers above it hold is still returned (the failure is logged);
    /// cycles and chains longer than [`ENUM_BASE_CHAIN_LIMIT`] end it too. Errors only
    /// when `mapper` itself fails to read or decode.
    pub fn enum_label(&self, mapper: Did, key: u32) -> Result<EnumLabel> {
        Ok(label_from_chain(&self.mapper_chain(mapper)?, key))
    }

    /// `mapper` and its loadable bases, top first. Errors only if `mapper` itself fails.
    fn mapper_chain(&self, mapper: Did) -> Result<Vec<Arc<MapperEntry>>> {
        let mut chain: Vec<Arc<MapperEntry>> = Vec::new();
        let mut current = mapper;
        while !current.is_null() {
            if chain.iter().any(|m| m.mapper.did == current) {
                tracing::debug!(%mapper, %current, "enum mapper base chain has a cycle");
                break;
            }
            if chain.len() >= ENUM_BASE_CHAIN_LIMIT {
                tracing::debug!(%mapper, "enum mapper base chain too long");
                break;
            }
            match self.mapper_entry(current) {
                Ok(Some(m)) => {
                    current = m.mapper.base;
                    chain.push(m);
                }
                Ok(None) => break,
                Err(e) if chain.is_empty() => return Err(e),
                Err(e) => {
                    // Already logged when it failed to load; keep the upper mappers.
                    tracing::debug!(%mapper, base = %current, error = %e,
                        "enum mapper base chain cut short");
                    break;
                }
            }
        }
        Ok(chain)
    }

    /// The entry for `key` in DID map `map`; `Ok(None)` if the map or the key is absent.
    pub fn did_map_entry(&self, map: Did, key: u32) -> Result<Option<DataIdEntry>> {
        Ok(self.did_map(map)?.and_then(|m| m.get(key).cloned()))
    }

    /// The label of a DB-object-type code (`property-system.md` §9), from the enum mapper
    /// that the EMAPPER map names "DBOType".
    pub fn dbo_type_name(&self, code: u8) -> Result<Option<Arc<str>>> {
        let Some(did) = self.lookup_path(&[EMAPPER_LABEL, DBO_TYPE_LABEL])? else {
            return Ok(None);
        };
        Ok(self.enum_label(did, u32::from(code))?.name)
    }

    /// Decodes the property-set record `did` (prefix 0x78–0x7F); `Ok(None)` if absent.
    pub fn property_set(&self, did: Did) -> Result<Option<PropertySet>> {
        if !is_property_set_did(did) {
            return Err(PropsError::NotPropertySource { did });
        }
        let Some(bytes) = self.read(did)? else {
            return Ok(None);
        };
        let registry = self.registry()?;
        PropertySet::parse_record(&bytes, did, registry).map(Some)
    }

    /// Like [`GameData::property_set`], but bytes after the set are allowed: returns the
    /// set and the number of trailing bytes (see [`PropertySet::parse_record_prefix`]).
    pub fn property_set_prefix(&self, did: Did) -> Result<Option<(PropertySet, usize)>> {
        if !is_property_set_did(did) {
            return Err(PropsError::NotPropertySource { did });
        }
        let Some(bytes) = self.read(did)? else {
            return Ok(None);
        };
        let registry = self.registry()?;
        PropertySet::parse_record_prefix(&bytes, did, registry).map(Some)
    }

    /// The properties of game object `did` (prefix 0x70–0x77), found with the
    /// +0x09000000 rule (`property-system.md` §6.2). `Ok(None)` if the object has no
    /// property set (a missing target, or a 0x77 object, whose target would leave the
    /// property-set range).
    pub fn object_properties(&self, did: Did) -> Result<Option<PropertySet>> {
        if !matches!(did.prefix(), 0x70..=0x77) {
            return Err(PropsError::NotPropertySource { did });
        }
        match object_property_set_did(did) {
            Some(set) => self.property_set(set),
            None => Ok(None),
        }
    }

    /// The properties for `did`: a game object's set by the +0x09000000 rule, or a
    /// property-set record itself. Any other prefix is
    /// [`PropsError::NotPropertySource`].
    pub fn properties(&self, did: Did) -> Result<Option<PropertySet>> {
        if is_property_set_did(did) {
            self.property_set(did)
        } else {
            self.object_properties(did)
        }
    }

    /// The label of `value`, a value of property `pid` (`property-system.md` §7).
    ///
    /// Lookup failures (a mapper that does not decode, an I/O error) are logged and give
    /// an unresolved label, so one bad record never hides a whole set. Without a registry
    /// every label is [`Label::None`]; the first such failure is logged as a warning, and
    /// a missing or corrupt dictionary is not reread on later calls (see
    /// [`GameData::registry`]).
    pub fn label(&self, pid: u32, value: &Value) -> Label {
        let registry = match self.registry() {
            Ok(r) => r,
            Err(e) => {
                if !self.registry_warned.swap(true, Ordering::Relaxed) {
                    tracing::warn!(error = %e, "no property registry; values stay unlabelled");
                }
                return Label::None;
            }
        };
        let mut bit_budget = MAX_LABELLED_BITS;
        self.label_with(registry, pid, value, &mut bit_budget)
    }

    /// `bit_budget` is how many bit labels may still be made; it is decreased by the
    /// number made.
    fn label_with(
        &self,
        registry: &PropertyRegistry,
        pid: u32,
        value: &Value,
        bit_budget: &mut usize,
    ) -> Label {
        let chain = |mapper: Did| {
            self.mapper_chain(mapper).unwrap_or_else(|e| {
                tracing::debug!(pid, %mapper, error = %e, "enum label lookup failed");
                Vec::new()
            })
        };
        match value {
            Value::Enum(key) => match registry.enum_source(pid) {
                Some(EnumSource::Mapper(mapper)) => Label::Enum {
                    mapper,
                    label: label_from_chain(&chain(mapper), *key),
                },
                Some(EnumSource::DidMap(map)) => {
                    let entry = self.did_map_entry(map, *key).unwrap_or_else(|e| {
                        tracing::debug!(pid, %map, error = %e, "DID-map lookup failed");
                        None
                    });
                    Label::DidMapKey {
                        map,
                        key: *key,
                        label: entry.as_ref().map(|e| Arc::clone(&e.label)),
                        target: entry.map(|e| e.value),
                    }
                }
                None => Label::None,
            },
            v if v.is_bitfield() => match registry.enum_source(pid) {
                Some(EnumSource::Mapper(mapper)) => {
                    let total = v.set_bit_count();
                    let room = (*bit_budget).min(MAX_LABELLED_BITS);
                    let bits: Vec<BitLabel> = if total == 0 || room == 0 {
                        Vec::new()
                    } else {
                        let chain = chain(mapper);
                        v.iter_set_bits()
                            .take(room)
                            .map(|bit| BitLabel {
                                bit,
                                label: label_from_chain(&chain, bit.saturating_add(1)),
                            })
                            .collect()
                    };
                    *bit_budget -= bits.len();
                    let omitted = total.saturating_sub(bits.len() as u64);
                    if omitted > 0 {
                        tracing::debug!(pid, omitted, "bitfield labels capped");
                    }
                    Label::Bits {
                        mapper,
                        bits,
                        omitted,
                    }
                }
                _ => Label::None,
            },
            // §7.4: PID 0 means "none", although the dictionary names it (§2.5 MP3).
            Value::PropertyId(0) => Label::Property { pid: 0, name: None },
            Value::PropertyId(p) => Label::Property {
                pid: *p,
                name: registry.shared_name(*p),
            },
            Value::DidRef(did) => {
                let dbo_type = registry.dbo_type(pid);
                let dbo_name = dbo_type.and_then(|code| {
                    self.dbo_type_name(code).unwrap_or_else(|e| {
                        tracing::debug!(code, error = %e, "DB-object-type lookup failed");
                        None
                    })
                });
                Label::DidRef {
                    did: *did,
                    dbo_type,
                    dbo_name,
                }
            }
            _ => Label::None,
        }
    }

    /// Resolves a whole set for display: every property with its name, declared type and
    /// label, struct fields and array elements included. Fails only if the registry
    /// cannot be loaded.
    ///
    /// The result borrows only `set` (see [`ResolvedProperty::into_owned`]). Its size is
    /// linear in the number of values: each bitfield gets at most
    /// [`MAX_LABELLED_BITS`] bit labels and the whole tree at most
    /// [`RESOLVE_MAX_BIT_LABELS`], and label texts are shared, not copied.
    pub fn resolve<'s>(&self, set: &'s PropertySet) -> Result<Vec<ResolvedProperty<'s>>> {
        let registry = self.registry()?;
        let mut bit_budget = RESOLVE_MAX_BIT_LABELS;
        Ok(set
            .iter()
            .map(|p| self.resolve_one(registry, p.pid, &p.value, 0, &mut bit_budget))
            .collect())
    }

    fn resolve_one<'s>(
        &self,
        registry: &PropertyRegistry,
        pid: u32,
        value: &'s Value,
        depth: usize,
        bit_budget: &mut usize,
    ) -> ResolvedProperty<'s> {
        let label = self.label_with(registry, pid, value, bit_budget);
        let children = if depth + 1 < RESOLVE_MAX_DEPTH {
            value
                .children()
                .iter()
                .map(|c| self.resolve_one(registry, c.pid, &c.value, depth + 1, bit_budget))
                .collect()
        } else {
            Vec::new()
        };
        ResolvedProperty {
            pid,
            name: registry.shared_name(pid),
            declared: registry.property_type(pid),
            value: Cow::Borrowed(value),
            label,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_cache_stops_growing_at_its_cap() {
        let cache: RecordCache<u32> = RecordCache::default();
        let n = u32::try_from(MAX_CACHED_RECORDS).unwrap();
        for i in 0..n {
            let _ = cache.put(Did(i), Ok(Some(Arc::new(i))));
        }
        assert_eq!(cache.len(), MAX_CACHED_RECORDS);
        // Past the cap: the value is handed back but not kept.
        let v = cache.put(Did(n), Ok(Some(Arc::new(7)))).unwrap().unwrap();
        assert_eq!(*v, 7);
        assert!(cache.get(Did(n)).is_none());
        assert_eq!(cache.len(), MAX_CACHED_RECORDS);
        // A key already present keeps its first value, even at the cap.
        let v = cache.put(Did(3), Ok(Some(Arc::new(99)))).unwrap().unwrap();
        assert_eq!(*v, 3);
    }
}
