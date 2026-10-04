// SPDX-License-Identifier: GPL-3.0-or-later
//! Where a [`crate::MaterialLibrary`] reads records from.

use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::Did;

use crate::error::SourceFailure;

/// A read-only supply of decompressed record payloads by DID.
///
/// Implemented for [`ArchiveSet`] (an install opened read-only, with normal DID routing:
/// general first, then local, `dat-container.md`), for [`FnSource`] (any closure, for
/// synthetic data and adapters) and for references, [`Arc`]s and boxes of a source.
pub trait RecordSource: Send + Sync {
    /// The payload of record `did`, `Ok(None)` if no archive holds it, or an error if
    /// reading failed.
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure>;
}

impl RecordSource for ArchiveSet {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure> {
        match self.locate(did.0)? {
            Some(located) => Ok(Some(self.read_located(&located)?)),
            None => Ok(None),
        }
    }
}

impl<T: RecordSource + ?Sized> RecordSource for &T {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure> {
        (**self).read_record(did)
    }
}

impl<T: RecordSource + ?Sized> RecordSource for Arc<T> {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure> {
        (**self).read_record(did)
    }
}

impl<T: RecordSource + ?Sized> RecordSource for Box<T> {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure> {
        (**self).read_record(did)
    }
}

/// A closure as a record source: `FnSource(|did| Ok::<_, E>(Some(bytes)))`.
#[derive(Clone, Copy)]
pub struct FnSource<F>(pub F);

impl<F> std::fmt::Debug for FnSource<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FnSource")
    }
}

impl<F, E> RecordSource for FnSource<F>
where
    F: Fn(Did) -> Result<Option<Vec<u8>>, E> + Send + Sync,
    E: Into<SourceFailure>,
{
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceFailure> {
        (self.0)(did).map_err(Into::into)
    }
}
