// SPDX-License-Identifier: GPL-3.0-or-later
//! Where [`crate::GameData`] reads records from.

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::Did;

use crate::error::SourceError;

/// A read-only supply of decompressed record payloads by DID.
///
/// Implemented for [`ArchiveSet`] (the normal case: an install opened read-only), for
/// [`MemorySource`] (synthetic data in tests and tools), and for references, [`Arc`]s and
/// boxes of any source, so one archive set can be shared with other components.
pub trait RecordSource: Send + Sync {
    /// The payload of record `did`, `Ok(None)` if no archive holds it, or an error if
    /// reading failed.
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError>;
}

impl RecordSource for ArchiveSet {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        match self.locate(did.0)? {
            Some(located) => Ok(Some(self.read_located(&located)?)),
            None => Ok(None),
        }
    }
}

impl<T: RecordSource + ?Sized> RecordSource for &T {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        (**self).read_record(did)
    }
}

impl<T: RecordSource + ?Sized> RecordSource for Arc<T> {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        (**self).read_record(did)
    }
}

impl<T: RecordSource + ?Sized> RecordSource for Box<T> {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        (**self).read_record(did)
    }
}

/// An in-memory record source: DID → payload.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    records: BTreeMap<u32, Vec<u8>>,
}

impl MemorySource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a record.
    pub fn insert(&mut self, did: impl Into<Did>, payload: Vec<u8>) {
        self.records.insert(did.into().0, payload);
    }

    /// Adds or replaces a record, builder style.
    #[must_use]
    pub fn with(mut self, did: impl Into<Did>, payload: Vec<u8>) -> Self {
        self.insert(did, payload);
        self
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the source holds no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl RecordSource for MemorySource {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        Ok(self.records.get(&did.0).cloned())
    }
}
