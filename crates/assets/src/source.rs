// SPDX-License-Identifier: GPL-3.0-or-later
//! Record sources: where raw (inflated) record bytes come from.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_dat::{Archive, ArchiveSet};

use crate::{AssetError, Did};

/// Something that can read a complete, inflated record by DID from any thread.
///
/// Reads take `&self` and may run concurrently on every worker. The source is responsible
/// for bounding the size of what it returns (for archives, through
/// [`tessera_dat::ArchiveOptions`]).
pub trait RecordSource: Send + Sync {
    /// Reads record `did`. A missing record is [`AssetError::NotFound`].
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError>;
}

impl RecordSource for ArchiveSet {
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError> {
        self.read(did).map_err(|e| AssetError::from((did, e)))
    }
}

impl RecordSource for Archive {
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError> {
        self.read(did).map_err(|e| AssetError::from((did, e)))
    }
}

impl<S: RecordSource + ?Sized> RecordSource for Arc<S> {
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError> {
        (**self).read_record(did)
    }
}

impl<S: RecordSource + ?Sized> RecordSource for Box<S> {
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError> {
        (**self).read_record(did)
    }
}

/// An in-memory record source, for tests and for data generated at run time.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    records: HashMap<Did, Arc<[u8]>>,
}

impl MemorySource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces record `did`.
    pub fn insert(&mut self, did: Did, bytes: impl Into<Arc<[u8]>>) {
        self.records.insert(did, bytes.into());
    }

    /// Removes record `did`, returning whether it existed.
    pub fn remove(&mut self, did: Did) -> bool {
        self.records.remove(&did).is_some()
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

    /// The DIDs present, in no particular order.
    pub fn dids(&self) -> impl Iterator<Item = Did> + '_ {
        self.records.keys().copied()
    }
}

impl<B: Into<Arc<[u8]>>> FromIterator<(Did, B)> for MemorySource {
    fn from_iter<I: IntoIterator<Item = (Did, B)>>(iter: I) -> Self {
        Self {
            records: iter.into_iter().map(|(d, b)| (d, b.into())).collect(),
        }
    }
}

impl RecordSource for MemorySource {
    fn read_record(&self, did: Did) -> Result<Vec<u8>, AssetError> {
        self.records
            .get(&did)
            .map(|b| b.to_vec())
            .ok_or(AssetError::NotFound { did })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_source_reads_and_misses() {
        let mut s: MemorySource = [(1u32, vec![1u8, 2])].into_iter().collect();
        s.insert(2, vec![3u8]);
        assert_eq!(s.len(), 2);
        assert_eq!(s.read_record(1).unwrap(), vec![1, 2]);
        assert!(matches!(
            s.read_record(9),
            Err(AssetError::NotFound { did: 9 })
        ));
        let shared: Arc<dyn RecordSource> = Arc::new(s);
        assert_eq!(shared.read_record(2).unwrap(), vec![3]);
    }
}
