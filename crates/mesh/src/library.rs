// SPDX-License-Identifier: GPL-3.0-or-later
//! [`MeshLibrary`]: mesh records of an install, decoded on the asset pool and cached.

use std::sync::Arc;
use std::time::Duration;

use tessera_assets::{
    AssetError, AssetKey, AssetState, AssetStore, Decoder, Handle, JobContext, Priority,
    RecordSource, StoreConfig, WorkerPool,
};
use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::Did;

use crate::error::{MeshError, Result};
use crate::record::{DecodeOptions, MESH_PREFIX, Mesh};

/// How a [`MeshDecoder`] receives the directory `stamp` that §10.4 needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StampInput {
    /// The bytes are the bare payload; the look-ahead fallback of §10.4 decides
    /// `build_byte` presence.
    #[default]
    None,
    /// The payload is followed by a 4-byte little-endian trailer whose low 16 bits are
    /// the directory `stamp`, as produced by [`StampedSource`].
    Trailer,
}

/// The mesh decoder for the [`tessera_assets`] pipeline.
///
/// The product is a [`Mesh`]; its cache size is [`Mesh::heap_size`]. The record must
/// start with the requested DID (MI1). The key's variant is ignored.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeshDecoder {
    /// Decoding options.
    pub options: DecodeOptions,
    /// Where the stamp comes from.
    pub stamp: StampInput,
}

/// Length of the stamp trailer of [`StampInput::Trailer`].
const TRAILER: usize = 4;

impl Decoder for MeshDecoder {
    type Output = Mesh;
    type Error = MeshError;
    const NAME: &'static str = "mesh";

    fn decode(&self, key: AssetKey, bytes: Vec<u8>, _ctx: &JobContext<'_>) -> Result<Mesh> {
        let (payload, stamp) = match self.stamp {
            StampInput::None => (&bytes[..], None),
            StampInput::Trailer => {
                let Some(split) = bytes.len().checked_sub(TRAILER) else {
                    return Err(MeshError::Truncated {
                        what: "stamp trailer",
                        offset: 0,
                        needed: TRAILER as u64,
                        available: bytes.len(),
                    });
                };
                let t = &bytes[split..];
                (&bytes[..split], Some(u16::from_le_bytes([t[0], t[1]])))
            }
        };
        Mesh::decode_expecting(payload, Did(key.did), stamp, &self.options)
    }

    fn size_of(&self, value: &Mesh) -> usize {
        value.heap_size()
    }
}

/// A record source over an [`ArchiveSet`] that appends each record's directory `stamp`
/// as a trailer, for [`MeshDecoder`] with [`StampInput::Trailer`].
#[derive(Debug, Clone)]
pub struct StampedSource {
    set: Arc<ArchiveSet>,
}

impl StampedSource {
    /// Wraps a set.
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>) -> Self {
        Self { set }
    }
}

impl RecordSource for StampedSource {
    fn read_record(&self, did: u32) -> Result<Vec<u8>, AssetError> {
        let (located, mut bytes) = self
            .set
            .read_with_entry(did)
            .map_err(|e| AssetError::from((did, e)))?;
        bytes.extend_from_slice(&u32::from(located.entry.stamp).to_le_bytes());
        Ok(bytes)
    }
}

/// Configuration of a [`MeshLibrary`].
#[derive(Debug, Clone)]
pub struct MeshLibraryConfig {
    /// Decoding options for every mesh.
    pub decode: DecodeOptions,
    /// The asset store's configuration; `cache_budget` bounds the decoded meshes kept
    /// (by [`Mesh::heap_size`]).
    pub store: StoreConfig,
}

impl Default for MeshLibraryConfig {
    fn default() -> Self {
        Self {
            decode: DecodeOptions::default(),
            store: StoreConfig {
                cache_budget: 512 << 20,
                ..StoreConfig::default()
            },
        }
    }
}

/// The mesh records of an install, decoded on a [`WorkerPool`] and cached by an
/// [`AssetStore`].
///
/// Records are read through the set's normal routing (`client_mesh.dat` first, then the
/// aux archive, `mesh.md` §2) together with their directory `stamp` (§10.4).
#[derive(Debug, Clone)]
pub struct MeshLibrary {
    set: Arc<ArchiveSet>,
    store: AssetStore<StampedSource, MeshDecoder>,
    decode: DecodeOptions,
}

impl MeshLibrary {
    /// A library whose decode jobs run on `pool`.
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>, pool: &WorkerPool, config: MeshLibraryConfig) -> Self {
        let decoder = MeshDecoder {
            options: config.decode,
            stamp: StampInput::Trailer,
        };
        let store = AssetStore::new(
            pool,
            StampedSource::new(Arc::clone(&set)),
            decoder,
            config.store,
        );
        Self {
            set,
            store,
            decode: config.decode,
        }
    }

    /// The archive set.
    #[must_use]
    pub fn set(&self) -> &Arc<ArchiveSet> {
        &self.set
    }

    /// The underlying asset store (statistics, completions, placeholders).
    #[must_use]
    pub fn store(&self) -> &AssetStore<StampedSource, MeshDecoder> {
        &self.store
    }

    /// Requests a mesh without blocking.
    pub fn request(&self, did: Did, priority: Priority) -> Handle<Mesh> {
        self.store.request(did.0, priority)
    }

    /// The cached mesh, if any. Never starts a job.
    #[must_use]
    pub fn cached(&self, did: Did) -> Option<Arc<Mesh>> {
        self.store.cached(did.0)
    }

    /// Requests a mesh at [`Priority::Immediate`] and blocks until it is decoded.
    ///
    /// For tools, tests and loading screens; a frame loop uses [`MeshLibrary::request`].
    ///
    /// # Errors
    ///
    /// [`MeshError::Source`] wrapping the pipeline's error; [`MeshError::category`] still
    /// gives the decode category when decoding failed.
    pub fn load(&self, did: Did) -> Result<Arc<Mesh>> {
        match self.request(did, Priority::Immediate).wait(Duration::MAX) {
            AssetState::Ready(m) => Ok(m),
            AssetState::Failed(source) => Err(MeshError::Source { did, source }),
            _ => Err(MeshError::Source {
                did,
                source: AssetError::Shutdown,
            }),
        }
    }

    /// Reads a record's payload and directory stamp (no decoding, no cache).
    ///
    /// # Errors
    ///
    /// [`MeshError::Source`] when the record cannot be read.
    pub fn read_record(&self, did: Did) -> Result<(Vec<u8>, u16)> {
        let (located, bytes) = self
            .set
            .read_with_entry(did.0)
            .map_err(|e| MeshError::Source {
                did,
                source: AssetError::from((did.0, e)),
            })?;
        Ok((bytes, located.entry.stamp))
    }

    /// Reads and decodes a record on the calling thread, bypassing the pool and cache.
    ///
    /// # Errors
    ///
    /// [`MeshError::Source`] when the record cannot be read, otherwise the decode error.
    pub fn decode_now(&self, did: Did, options: Option<&DecodeOptions>) -> Result<Mesh> {
        let (bytes, stamp) = self.read_record(did)?;
        Mesh::decode_expecting(&bytes, did, Some(stamp), options.unwrap_or(&self.decode))
    }

    /// Every mesh DID in the set's mesh archives; see [`mesh_dids`].
    ///
    /// # Errors
    ///
    /// [`MeshError::Source`] when a directory walk fails.
    pub fn mesh_dids(&self) -> Result<Vec<Did>> {
        mesh_dids(&self.set)
    }
}

/// Every mesh DID (prefix 0x06) in the mesh archives of `set` (`client_mesh.dat` and its
/// aux parts, `mesh.md` §2), sorted, without duplicates.
///
/// # Errors
///
/// [`MeshError::Source`] when a directory walk fails.
pub fn mesh_dids(set: &ArchiveSet) -> Result<Vec<Did>> {
    let lo = u32::from(MESH_PREFIX) << 24;
    let hi = lo + (1 << 24);
    let mut out = Vec::new();
    for m in set.members() {
        if m.id.kind != ArchiveKind::Mesh {
            continue;
        }
        for e in m.archive.entries_range(lo..hi) {
            let e = e.map_err(|e| MeshError::Source {
                did: Did(lo),
                source: AssetError::from((lo, e)),
            })?;
            out.push(Did(e.did));
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}
