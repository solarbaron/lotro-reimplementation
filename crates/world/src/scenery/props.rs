// SPDX-License-Identifier: GPL-3.0-or-later
//! An object's properties resolved along its 0x47 chain (`scenery.md` §9.2 rules 1–2 and
//! the Tessera Decision there): a record's own set is its 0x78 extra sets in list order
//! with the embedded set on top; a lookup tries the own set, then the parent 0x47 record,
//! recursively; the nearest level that has the property wins. No registry default is
//! applied here; each reader applies its own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::setup::ENTITY_PREFIX;
use tessera_formats::{
    Did, EntityDescriptor, PropertySchema, PropertySet, PropertySetRecord, PropertyValue,
};

use super::error::SceneryError;
use crate::source::WorldSource;

/// Most 0x47 levels followed (the object and its ancestors; the install's chains have at
/// most 2).
pub const MAX_CHAIN_LEVELS: usize = 8;
/// Most 0x78 extra sets read per record (the install has at most 2).
pub const MAX_EXTRA_SETS: usize = 16;
/// Default bound of an [`ObjectPropertyCache`].
pub const DEFAULT_PROPERTY_CACHE_ENTRIES: usize = 16_384;

/// One level of a chain: a record's property sets in lookup priority (embedded set first,
/// then the extra sets from the last named to the first).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChainLevel {
    /// The record.
    pub did: Did,
    /// Its sets, highest priority first.
    pub sets: Vec<PropertySet>,
}

impl ChainLevel {
    /// The level's value of `pid`, if any of its sets has it.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.sets.iter().find_map(|s| s.get(pid))
    }
}

/// An object's property levels, nearest first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectProperties {
    /// The levels, the object itself first.
    pub levels: Vec<ChainLevel>,
    /// Whether the chain was cut (a parent missing, broken, looping or too deep); the
    /// levels read so far are kept.
    pub incomplete: bool,
}

impl ObjectProperties {
    /// The value of `pid` from the nearest level that has it (§9.2 rule 2).
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.levels.iter().find_map(|l| l.get(pid))
    }

    /// [`ObjectProperties::get`] of an f32 property.
    #[must_use]
    pub fn float(&self, pid: u32) -> Option<f32> {
        match self.get(pid)? {
            PropertyValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// [`ObjectProperties::get`] of a boolean property ([`value_bool`]).
    #[must_use]
    pub fn boolean(&self, pid: u32) -> Option<bool> {
        value_bool(self.get(pid)?)
    }

    /// [`ObjectProperties::get`] of an enum property's key.
    #[must_use]
    pub fn enum_key(&self, pid: u32) -> Option<u32> {
        match self.get(pid)? {
            PropertyValue::Enum(v) => Some(*v),
            _ => None,
        }
    }

    /// Reads the chain of 0x47 object `did`.
    ///
    /// # Errors
    ///
    /// When `did` itself is not a 0x47 record, is missing or does not decode. Data
    /// problems further up the chain only set [`ObjectProperties::incomplete`]; a read of
    /// any level or extra set that fails for a reason that is not the data's
    /// ([`SceneryError::is_data_error`] false) is returned as an error, so that a
    /// transient pipeline failure is never mistaken for an incomplete chain.
    pub fn load(
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
    ) -> Result<Self, SceneryError> {
        let mut out = Self::default();
        let mut next = Some(did);
        while let Some(d) = next.take() {
            if out.levels.iter().any(|l| l.did == d) {
                out.incomplete = true;
                tracing::debug!(object = %format_args!("{:#010x}", did.0), "0x47 chain loops");
                break;
            }
            if out.levels.len() >= MAX_CHAIN_LEVELS {
                out.incomplete = true;
                tracing::debug!(object = %format_args!("{:#010x}", did.0), "0x47 chain too deep");
                break;
            }
            let level = match read_level(source, schema, d) {
                Ok(l) => l,
                Err(e) if out.levels.is_empty() || !e.is_data_error() => return Err(e),
                Err(e) => {
                    tracing::debug!(object = %format_args!("{:#010x}", did.0), error = %e, "0x47 parent unusable");
                    out.incomplete = true;
                    break;
                }
            };
            let (level, parent, extra_failed) = level;
            out.incomplete |= extra_failed;
            out.levels.push(level);
            next = parent;
        }
        Ok(out)
    }
}

/// Reads one 0x47 record as a level; returns its parent (when the template is a 0x47) and
/// whether an extra set could not be read.
fn read_level(
    source: &dyn WorldSource,
    schema: &dyn PropertySchema,
    did: Did,
) -> Result<(ChainLevel, Option<Did>, bool), SceneryError> {
    if did.prefix() != ENTITY_PREFIX {
        return Err(SceneryError::Chain {
            did: did.0,
            reason: "not a 0x47 entity descriptor",
        });
    }
    let bytes = source.read(did)?.ok_or(SceneryError::MissingRecord {
        did: did.0,
        what: "entity descriptor",
    })?;
    let desc = EntityDescriptor::parse_expecting(&bytes, did, schema)
        .map_err(|e| SceneryError::format(did.0, "entity descriptor", e))?;
    let mut sets = vec![desc.properties];
    let mut extra_failed = desc.extra_sets.len() > MAX_EXTRA_SETS;
    for &extra in desc.extra_sets.iter().take(MAX_EXTRA_SETS).rev() {
        match read_set(source, schema, extra) {
            Ok(s) => sets.push(s),
            Err(e) if !e.is_data_error() => return Err(e),
            Err(e) => {
                tracing::debug!(set = %format_args!("{:#010x}", extra.0), error = %e, "extra property set unusable");
                extra_failed = true;
            }
        }
    }
    let parent = Some(desc.header.template).filter(|t| t.prefix() == ENTITY_PREFIX);
    Ok((ChainLevel { did, sets }, parent, extra_failed))
}

/// Reads a stand-alone property-set record.
///
/// # Errors
///
/// When the record is missing, unreadable or does not decode.
pub fn read_set(
    source: &dyn WorldSource,
    schema: &dyn PropertySchema,
    did: Did,
) -> Result<PropertySet, SceneryError> {
    let bytes = source.read(did)?.ok_or(SceneryError::MissingRecord {
        did: did.0,
        what: "property set",
    })?;
    PropertySetRecord::parse_expecting(&bytes, did, schema)
        .map(|r| r.set)
        .map_err(|e| SceneryError::format(did.0, "property set", e))
}

/// A boolean read from a property value: a stored boolean, or any non-float scalar whose
/// bits are non-zero. Lenient; the frill engine's per-object booleans use the stricter
/// [`value_flag`].
#[must_use]
pub fn value_bool(v: &PropertyValue) -> Option<bool> {
    match v {
        PropertyValue::Bool(b) => Some(*b),
        PropertyValue::Float(_) | PropertyValue::Timestamp(_) => None,
        other => other.as_bits().map(|b| b != 0),
    }
}

/// A frill per-object boolean (`scenery.md` §8.3 rule 8, BIN (y), S52): read only from a
/// stored boolean (true when non-zero); a value of any other type, integers, enums and
/// bitfields included, counts as absent (`None`), so the reader's default applies.
#[must_use]
pub fn value_flag(v: &PropertyValue) -> Option<bool> {
    match v {
        PropertyValue::Bool(b) => Some(*b),
        _ => None,
    }
}

/// [`ObjectProperties`] per object DID, read once and shared by every worker. Data
/// failures are cached as `None`; failures that are not the data's are returned and not
/// cached. Bounded by entry count; emptied when full.
///
/// One cache serves one source: the entries are keyed by DID only, so the cache remembers
/// the source and schema of the call that filled it and empties itself (with a debug log)
/// when it is called with a different pair.
#[derive(Debug)]
pub struct ObjectPropertyCache {
    inner: Mutex<CacheState>,
    max_entries: usize,
}

#[derive(Debug, Default)]
struct CacheState {
    /// Addresses of the source and schema the entries were read from.
    owner: Option<(usize, usize)>,
    entries: HashMap<Did, Option<Arc<ObjectProperties>>>,
}

/// The data address of a trait object (its identity; the vtable part is ignored).
fn address<T: ?Sized>(r: &T) -> usize {
    std::ptr::from_ref(r).cast::<()>() as usize
}

impl Default for ObjectPropertyCache {
    fn default() -> Self {
        Self::new(DEFAULT_PROPERTY_CACHE_ENTRIES)
    }
}

impl ObjectPropertyCache {
    /// A cache of at most `max_entries` (at least 1) objects.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            inner: Mutex::new(CacheState::default()),
            max_entries: max_entries.max(1),
        }
    }

    /// The properties of object `did` (see [`ObjectProperties::load`]): `Ok(None)` when
    /// the object itself cannot be used (missing, not a 0x47 record, broken; cached).
    ///
    /// # Errors
    ///
    /// When a read fails for a reason that is not the data's
    /// ([`SceneryError::is_data_error`] false); nothing is cached then.
    pub fn get(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        did: Did,
    ) -> Result<Option<Arc<ObjectProperties>>, SceneryError> {
        let owner = (address(source), address(schema));
        {
            let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if st.owner != Some(owner) {
                if st.owner.is_some() {
                    tracing::debug!("object property cache used with another source; emptied");
                }
                st.entries.clear();
                st.owner = Some(owner);
            }
            if let Some(hit) = st.entries.get(&did) {
                return Ok(hit.clone());
            }
        }
        let loaded = match ObjectProperties::load(source, schema, did) {
            Ok(p) => Some(Arc::new(p)),
            Err(e) if e.is_data_error() => {
                tracing::debug!(object = %format_args!("{:#010x}", did.0), error = %e, "object properties unusable");
                None
            }
            Err(e) => {
                tracing::warn!(object = %format_args!("{:#010x}", did.0), error = %e, "object properties unavailable (not cached)");
                return Err(e);
            }
        };
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if st.owner == Some(owner) {
            if st.entries.len() >= self.max_entries {
                st.entries.clear();
            }
            st.entries.insert(did, loaded.clone());
        }
        Ok(loaded)
    }

    /// Number of cached objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
