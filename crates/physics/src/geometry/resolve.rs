// SPDX-License-Identifier: GPL-3.0-or-later
//! What an object DID collides with: its 0x47 chain's collision flags, its setup's
//! collision kind and its shape (`collision.md` §8, §9.1, §9.4).
//!
//! [`resolve_object_collision`] follows the chain 0x47 → (0x47)* → 0x1F → (0x1F)* → 0x04
//! with [`tessera_formats::resolve_setup`] (hop limit and cycle detection included), reads
//! the §9.1 properties from every 0x47 level (first value found wins), and reads the
//! setup's 0x66 collision mesh. A level's own properties are its 0x78 extra sets in list
//! order, each later set overriding the earlier ones, with its embedded set on top
//! (`scenery.md` §9.2 rule 1, BIN). Records are decoded as they are fetched and their bytes
//! dropped at once. It never touches an archive: the caller supplies a fetch function, so
//! `tessera-world` can call it on its asset workers with its own sources and caches.
//! [`ObjectCollisionCache`] memoises results per object DID, bounded by entries and by
//! bytes, and is `Send + Sync`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tessera_formats::collision::CollisionMesh;
use tessera_formats::setup::{ENTITY_PREFIX, MAX_CHAIN_HOPS};
use tessera_formats::{Did, EntityDescriptor, PropertySchema, PropertySetRecord, resolve_setup};

use super::flags::{CollisionFlags, PhysicsPids, SetupCollision};
use super::shape::ShapeTemplate;
use crate::error::PhysicsError;

/// A record as the fetch function returns it: the decompressed payload and the directory
/// `stamp` (`None` when unknown; the decoders then use their stamp-less rules).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FetchedRecord {
    /// Payload bytes.
    pub bytes: Vec<u8>,
    /// Directory stamp.
    pub stamp: Option<u16>,
}

impl AsRef<[u8]> for FetchedRecord {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// The resolved collision of one object DID.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectCollision {
    /// The object asked for.
    pub object: Did,
    /// Records visited, object first, setup last.
    pub chain: Vec<Did>,
    /// The setup.
    pub setup: Option<Did>,
    /// §9.1 properties layered along the 0x47 chain, extra sets included (no SED-level
    /// values).
    pub flags: CollisionFlags,
    /// What collision the setup has.
    pub collision: SetupCollision,
    /// The shape in the setup frame (`None` without collision, or when the 0x66 mesh could
    /// not be read and the setup has no volume: then `error` says why).
    pub template: Option<Arc<ShapeTemplate>>,
    /// Whether `template` lacks the setup's 0x66 mesh (it could not be read), so only the
    /// volumes collide.
    pub incomplete: bool,
    /// The first problem met ([`PhysicsError::Record`] for a record that could not be read
    /// or decoded, [`PhysicsError::TooLarge`] for a shape beyond the limits); later ones are
    /// logged at `debug` level.
    pub error: Option<PhysicsError>,
}

impl ObjectCollision {
    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        let error = match &self.error {
            Some(PhysicsError::Record { message, .. }) => message.capacity(),
            _ => 0,
        };
        std::mem::size_of::<Self>()
            + 4 * self.chain.capacity()
            + error
            + self.template.as_ref().map_or(0, |t| t.heap_size())
    }
}

/// Keeps the first error and logs the others.
fn note(first: &mut Option<PhysicsError>, e: PhysicsError) {
    tracing::debug!(error = %e, "object collision");
    if first.is_none() {
        *first = Some(e);
    }
}

fn record_error(did: Did, what: &'static str, message: impl ToString) -> PhysicsError {
    PhysicsError::Record {
        did: did.0,
        what,
        message: message.to_string(),
    }
}

/// Resolves `object` (a 0x47 entity, a 0x1F template or a 0x04 setup) to its collision.
/// Never fails: problems end up in [`ObjectCollision::error`] (a chain that does not
/// resolve yields no shape; an unreadable 0x66 mesh yields the volumes alone and sets
/// [`ObjectCollision::incomplete`]; an unreadable property set leaves its level's values
/// out).
///
/// `fetch` returns a record's payload and stamp, `Ok(None)` when it does not exist, or an
/// error message. `schema` decodes the 0x47 property sets and the 0x78 extra sets.
pub fn resolve_object_collision<S: PropertySchema + ?Sized>(
    object: Did,
    mut fetch: impl FnMut(Did) -> Result<Option<FetchedRecord>, String>,
    schema: &S,
    pids: &PhysicsPids,
) -> ObjectCollision {
    let mut out = ObjectCollision {
        object,
        ..ObjectCollision::default()
    };
    let mut first_error: Option<PhysicsError> = None;
    let mut flags = CollisionFlags::default();
    let mut levels = 0usize;
    // Extra sets already read in this resolution.
    let mut extra_seen: HashMap<Did, CollisionFlags> = HashMap::new();
    let chain = resolve_setup(object, |did: Did| {
        let r = fetch(did)?;
        if let Some(rec) = &r
            && did.prefix() == ENTITY_PREFIX
            && levels <= MAX_CHAIN_HOPS
        {
            levels += 1;
            // Decode now and keep only the flags (records may be large).
            match EntityDescriptor::parse_expecting(&rec.bytes, did, schema) {
                Ok(d) => {
                    let mut own = CollisionFlags::from_set(&d.properties, pids);
                    // Later extra sets override earlier ones; the embedded set wins.
                    for &set in d.extra_sets.iter().rev() {
                        if let Some(f) = extra_seen.get(&set) {
                            own = own.or(*f);
                            continue;
                        }
                        let f = match fetch(set) {
                            Ok(Some(x)) => {
                                match PropertySetRecord::parse_expecting(&x.bytes, set, schema) {
                                    Ok(rec) => CollisionFlags::from_set(&rec.set, pids),
                                    Err(e) => {
                                        note(
                                            &mut first_error,
                                            record_error(set, "extra property set", e),
                                        );
                                        CollisionFlags::default()
                                    }
                                }
                            }
                            Ok(None) => {
                                note(
                                    &mut first_error,
                                    record_error(set, "extra property set", "missing"),
                                );
                                CollisionFlags::default()
                            }
                            Err(e) => {
                                note(&mut first_error, record_error(set, "extra property set", e));
                                CollisionFlags::default()
                            }
                        };
                        extra_seen.insert(set, f);
                        own = own.or(f);
                    }
                    flags = flags.or(own);
                }
                Err(e) => note(&mut first_error, record_error(did, "object properties", e)),
            }
        }
        Ok::<_, String>(r)
    });
    out.flags = flags;
    let chain = match chain {
        Ok(c) => c,
        Err(e) => {
            note(&mut first_error, record_error(object, "object chain", e));
            out.error = first_error;
            return out;
        }
    };
    out.setup = chain.path.last().copied();
    out.chain = chain.path;
    let setup = chain.setup;
    out.collision = SetupCollision::of(&setup);
    if !out.collision.has_collision() {
        out.error = first_error;
        return out;
    }
    let mut mesh_failed = false;
    let mesh = if setup.collision_mesh.is_null() {
        None
    } else {
        let did = setup.collision_mesh;
        let problem = match fetch(did) {
            Ok(Some(rec)) => match CollisionMesh::parse(&rec.bytes, rec.stamp) {
                Ok(m) => Ok(m),
                Err(e) => Err(e.to_string()),
            },
            Ok(None) => Err("missing".to_owned()),
            Err(e) => Err(e),
        };
        match problem {
            Ok(m) => Some(m),
            Err(e) => {
                mesh_failed = true;
                note(&mut first_error, record_error(did, "collision mesh", e));
                None
            }
        }
    };
    match ShapeTemplate::from_setup(&setup, mesh.as_ref()) {
        Ok(t) if !t.is_empty() => out.template = Some(Arc::new(t)),
        Ok(_) => {}
        Err(e) => note(&mut first_error, e),
    }
    out.incomplete = mesh_failed && out.template.is_some();
    out.error = first_error;
    out
}

/// Results of [`resolve_object_collision`] per object DID, shared by every worker. Bounded
/// by entry count and by approximate bytes ([`ObjectCollision::heap_size`]): when either
/// would be exceeded the cache is cleared; a single result larger than the byte bound is
/// returned without being cached.
#[derive(Debug)]
pub struct ObjectCollisionCache {
    inner: Mutex<CacheInner>,
    max_entries: usize,
    max_bytes: usize,
}

#[derive(Debug, Default)]
struct CacheInner {
    map: HashMap<Did, Arc<ObjectCollision>>,
    bytes: usize,
}

/// Default entry bound of [`ObjectCollisionCache`] (about 28,000 distinct objects are
/// placed).
pub const DEFAULT_COLLISION_CACHE_ENTRIES: usize = 65_536;

/// Default byte bound of [`ObjectCollisionCache`] (every 0x66 record of the install takes
/// well under this).
pub const DEFAULT_COLLISION_CACHE_BYTES: usize = 256 << 20;

impl Default for ObjectCollisionCache {
    fn default() -> Self {
        Self::new(DEFAULT_COLLISION_CACHE_ENTRIES)
    }
}

impl ObjectCollisionCache {
    /// A cache of at most `max_entries` objects (at least 1) and
    /// [`DEFAULT_COLLISION_CACHE_BYTES`].
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self::with_limits(max_entries, DEFAULT_COLLISION_CACHE_BYTES)
    }

    /// A cache of at most `max_entries` objects (at least 1) and about `max_bytes` bytes.
    #[must_use]
    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(CacheInner::default()),
            max_entries: max_entries.max(1),
            max_bytes,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The cached result for `object`, or `resolve(object)` stored and returned. `resolve`
    /// runs outside the lock (two workers may resolve the same object once each).
    pub fn get_or_resolve(
        &self,
        object: Did,
        resolve: impl FnOnce(Did) -> ObjectCollision,
    ) -> Arc<ObjectCollision> {
        if let Some(hit) = self.lock().map.get(&object) {
            return Arc::clone(hit);
        }
        let r = Arc::new(resolve(object));
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
