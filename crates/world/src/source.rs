// SPDX-License-Identifier: GPL-3.0-or-later
//! Record access for the world: records with their directory stamps, and the bundle frame
//! that carries one landblock's records from the reading stage of an asset job to its
//! decoding stage.
//!
//! [`tessera_assets`] hands a decoder exactly the bytes its [`RecordSource`] returned for
//! one DID. A landblock bundle needs up to four records plus their directory stamps
//! (`landblock.md` §2.4, §8), so [`BundleSource`] reads them all for the landblock-data
//! DID and packs them into a **bundle frame**; [`BundleFrame::parse`] unpacks it in the
//! decoder. The frame is Tessera's own transport format, never stored on disk.
//!
//! ```text
//! frame  = magic "TWB1", u8 count (≤ 8), count × record
//! record = u32 did, u8 flag, u16 stamp, u32 length, length × u8
//! flag   = 0: payload, no stamp; 1: payload with stamp;
//!          2: the record exists but could not be read — the bytes are the UTF-8 error text
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tessera_assets::{AssetError, RecordSource};
use tessera_dat::ArchiveSet;
use tessera_formats::{Did, LandblockId, LandblockKind};

use crate::coords::LandblockKey;
use crate::error::{Result, WorldError};

/// A record's payload and its directory stamp, if the source knows one.
pub type StampedRecord = (Vec<u8>, Option<u16>);

/// A record of a landblock bundle: DID, payload and directory stamp.
pub type BundleRecord = (Did, Vec<u8>, Option<u16>);

/// A store of records addressed by DID, with their directory stamps.
///
/// Implemented for [`ArchiveSet`] (an install, read-only) and [`MemoryWorldSource`]
/// (synthetic data). Reads take `&self` and may run on any worker.
pub trait WorldSource: Send + Sync + 'static {
    /// Reads record `did`: `Ok(None)` when no archive holds it, the bytes and directory
    /// stamp (if the source knows one) otherwise.
    ///
    /// # Errors
    ///
    /// An [`AssetError`] when the record exists but cannot be read.
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError>;

    /// [`WorldSource::read_stamped`] without the stamp.
    ///
    /// # Errors
    ///
    /// As [`WorldSource::read_stamped`].
    fn read(&self, did: Did) -> Result<Option<Vec<u8>>, AssetError> {
        Ok(self.read_stamped(did)?.map(|(b, _)| b))
    }

    /// Whether record `did` exists. The default reads it; sources with a directory
    /// override this with a lookup that does not read the payload.
    ///
    /// # Errors
    ///
    /// As [`WorldSource::read_stamped`].
    fn contains(&self, did: Did) -> Result<bool, AssetError> {
        Ok(self.read_stamped(did)?.is_some())
    }
}

impl WorldSource for ArchiveSet {
    fn contains(&self, did: Did) -> Result<bool, AssetError> {
        self.locate(did.0)
            .map(|l| l.is_some())
            .map_err(|e| AssetError::from((did.0, e)))
    }

    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        let located = self
            .locate(did.0)
            .map_err(|e| AssetError::from((did.0, e)))?;
        let Some(located) = located else {
            return Ok(None);
        };
        let bytes = self
            .read_located(&located)
            .map_err(|e| AssetError::from((did.0, e)))?;
        Ok(Some((bytes, Some(located.entry.stamp))))
    }
}

impl<S: WorldSource + ?Sized> WorldSource for Arc<S> {
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        (**self).read_stamped(did)
    }

    fn read(&self, did: Did) -> Result<Option<Vec<u8>>, AssetError> {
        (**self).read(did)
    }

    fn contains(&self, did: Did) -> Result<bool, AssetError> {
        (**self).contains(did)
    }
}

/// An in-memory [`WorldSource`] for tests, tools and generated worlds.
#[derive(Debug, Clone, Default)]
pub struct MemoryWorldSource {
    records: HashMap<u32, (Arc<[u8]>, Option<u16>)>,
}

impl MemoryWorldSource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces record `did` with directory stamp `stamp`.
    pub fn insert(&mut self, did: impl Into<Did>, bytes: impl Into<Arc<[u8]>>, stamp: Option<u16>) {
        self.records.insert(did.into().0, (bytes.into(), stamp));
    }

    /// Removes record `did`; returns whether it existed.
    pub fn remove(&mut self, did: impl Into<Did>) -> bool {
        self.records.remove(&did.into().0).is_some()
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

impl WorldSource for MemoryWorldSource {
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        Ok(self.records.get(&did.0).map(|(b, s)| (b.to_vec(), *s)))
    }

    fn contains(&self, did: Did) -> Result<bool, AssetError> {
        Ok(self.records.contains_key(&did.0))
    }
}

/// The same records for `tessera-props` / `tessera-wstate` consumers (object stores, area
/// names), so one synthetic world can drive both.
impl tessera_props::RecordSource for MemoryWorldSource {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, tessera_props::SourceError> {
        Ok(self.records.get(&did.0).map(|(b, _)| b.to_vec()))
    }
}

/// Magic of a bundle frame.
pub const FRAME_MAGIC: [u8; 4] = *b"TWB1";
/// Most records one frame carries.
pub const MAX_FRAME_RECORDS: usize = 8;
/// Bytes of a record header inside a frame.
const RECORD_HEADER: usize = 4 + 1 + 2 + 4;
/// Longest error text a failure entry carries, in bytes (longer text is cut at a
/// character boundary when encoding and rejected when parsing).
pub const MAX_FAILURE_TEXT: usize = 512;

/// One record inside a [`BundleFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramedRecord<'a> {
    /// The record's DID.
    pub did: Did,
    /// Its directory stamp, if the source knew one.
    pub stamp: Option<u16>,
    /// Its payload.
    pub bytes: &'a [u8],
}

/// An optional record of a bundle that exists but could not be read (flag 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramedFailure<'a> {
    /// The record's DID.
    pub did: Did,
    /// The read error, as text.
    pub error: &'a str,
}

/// The records of one landblock, as packed by [`BundleSource`] (module docs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleFrame<'a> {
    /// The records in stored order.
    pub records: Vec<FramedRecord<'a>>,
    /// Optional records that exist but could not be read, in stored order.
    pub failures: Vec<FramedFailure<'a>>,
}

impl<'a> BundleFrame<'a> {
    /// Unpacks a frame. Allocation is bounded by [`MAX_FRAME_RECORDS`]; payloads are
    /// borrowed from `data`.
    ///
    /// # Errors
    ///
    /// [`WorldError::Frame`] for a bad magic, too many records, a flag byte other than 0,
    /// 1 or 2, a failure text that is not UTF-8 or longer than [`MAX_FAILURE_TEXT`], a
    /// length beyond the input, or trailing bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let err = |offset, reason| WorldError::Frame { offset, reason };
        if data.get(..4) != Some(&FRAME_MAGIC[..]) {
            return Err(err(0, "bad magic"));
        }
        let count = usize::from(*data.get(4).ok_or(err(4, "truncated count"))?);
        if count > MAX_FRAME_RECORDS {
            return Err(err(4, "too many records"));
        }
        let mut records = Vec::with_capacity(count);
        let mut failures = Vec::new();
        let mut at = 5usize;
        for _ in 0..count {
            let h = data
                .get(at..at + RECORD_HEADER)
                .ok_or(err(at, "truncated record header"))?;
            let did = Did(u32::from_le_bytes([h[0], h[1], h[2], h[3]]));
            let flag = h[4];
            if flag > 2 {
                return Err(err(at + 4, "bad stamp flag"));
            }
            let stamp = (flag == 1).then(|| u16::from_le_bytes([h[5], h[6]]));
            let len = u32::from_le_bytes([h[7], h[8], h[9], h[10]]) as usize;
            let flag_at = at + 4;
            at += RECORD_HEADER;
            let end = at.checked_add(len).ok_or(err(at, "length overflow"))?;
            let bytes = data.get(at..end).ok_or(err(at, "truncated payload"))?;
            if flag == 2 {
                if bytes.len() > MAX_FAILURE_TEXT {
                    return Err(err(at, "failure text too long"));
                }
                let error = std::str::from_utf8(bytes).map_err(|_| err(at, "failure text"))?;
                if h[5] != 0 || h[6] != 0 {
                    return Err(err(flag_at + 1, "stamp on a failure entry"));
                }
                failures.push(FramedFailure { did, error });
            } else {
                records.push(FramedRecord { did, stamp, bytes });
            }
            at = end;
        }
        if at != data.len() {
            return Err(err(at, "trailing bytes"));
        }
        Ok(Self { records, failures })
    }

    /// The record with DID `did`, if present (the first one if repeated).
    #[must_use]
    pub fn get(&self, did: Did) -> Option<&FramedRecord<'a>> {
        self.records.iter().find(|r| r.did == did)
    }

    /// The read error of record `did`, if the frame carries one.
    #[must_use]
    pub fn failure(&self, did: Did) -> Option<&'a str> {
        self.failures.iter().find(|f| f.did == did).map(|f| f.error)
    }

    /// Packs records into a frame.
    ///
    /// # Errors
    ///
    /// [`WorldError::Frame`] for more than [`MAX_FRAME_RECORDS`] records or a payload of
    /// 4 GiB or more.
    pub fn encode(records: &[FramedRecord<'_>]) -> Result<Vec<u8>> {
        Self::encode_with_failures(records, &[])
    }

    /// Packs records and failure entries (after the records) into a frame. Failure texts
    /// longer than [`MAX_FAILURE_TEXT`] bytes are cut at a character boundary.
    ///
    /// # Errors
    ///
    /// [`WorldError::Frame`] for more than [`MAX_FRAME_RECORDS`] entries in all or a
    /// payload of 4 GiB or more.
    pub fn encode_with_failures(
        records: &[FramedRecord<'_>],
        failures: &[FramedFailure<'_>],
    ) -> Result<Vec<u8>> {
        let count = records.len() + failures.len();
        if count > MAX_FRAME_RECORDS {
            return Err(WorldError::Frame {
                offset: 4,
                reason: "too many records",
            });
        }
        let total: usize = records
            .iter()
            .map(|r| RECORD_HEADER + r.bytes.len())
            .sum::<usize>()
            + failures.len() * (RECORD_HEADER + MAX_FAILURE_TEXT);
        let mut out = Vec::with_capacity(5 + total);
        out.extend_from_slice(&FRAME_MAGIC);
        // At most MAX_FRAME_RECORDS by the check above.
        #[allow(clippy::cast_possible_truncation)]
        out.push(count as u8);
        let put = |out: &mut Vec<u8>, did: Did, flag: u8, stamp: u16, bytes: &[u8]| {
            let len = u32::try_from(bytes.len()).map_err(|_| WorldError::Frame {
                offset: out.len(),
                reason: "payload too large",
            })?;
            out.extend_from_slice(&did.0.to_le_bytes());
            out.push(flag);
            out.extend_from_slice(&stamp.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(bytes);
            Ok::<(), WorldError>(())
        };
        for r in records {
            put(
                &mut out,
                r.did,
                u8::from(r.stamp.is_some()),
                r.stamp.unwrap_or(0),
                r.bytes,
            )?;
        }
        for f in failures {
            let mut end = f.error.len().min(MAX_FAILURE_TEXT);
            while !f.error.is_char_boundary(end) {
                end -= 1;
            }
            put(&mut out, f.did, 2, 0, &f.error.as_bytes()[..end])?;
        }
        Ok(out)
    }
}

/// What [`BundleSource::read_bundle_records`] read for one landblock.
#[derive(Debug, Clone, Default)]
pub struct BundleRead {
    /// The records read: the data record first, then the optional records present.
    pub records: Vec<BundleRecord>,
    /// Optional records that exist (or may exist) but could not be read, with the error.
    /// The bundle is still built from the other records.
    pub failures: Vec<(Did, AssetError)>,
}

/// The [`RecordSource`] of the bundle store: for a landblock-data DID `0x800RXXYY` it
/// reads the landblock's data, info (when the data header names it, LB-E3), block map and
/// properties records with their stamps and returns them as one [`BundleFrame`].
///
/// A missing data record is [`AssetError::NotFound`] and a data record that cannot be read
/// fails the job; the other three are optional (`landblock.md` §2.3 LB-E2): absent ones
/// are left out of the frame, and unreadable ones are carried as failure entries so the
/// decoder reports them as bundle problems and still delivers the terrain.
#[derive(Clone)]
pub struct BundleSource {
    source: Arc<dyn WorldSource>,
}

impl std::fmt::Debug for BundleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleSource").finish_non_exhaustive()
    }
}

impl BundleSource {
    /// Wraps a world source.
    #[must_use]
    pub fn new(source: Arc<dyn WorldSource>) -> Self {
        Self { source }
    }

    /// Reads the records of `key` (see the type docs) without framing them. Read errors of
    /// the optional records are returned in [`BundleRead::failures`], not as an error.
    ///
    /// # Errors
    ///
    /// [`AssetError::NotFound`] without a data record; read errors of the data record.
    pub fn read_bundle_records(&self, key: LandblockKey) -> Result<BundleRead, AssetError> {
        let data_did = key.data_did();
        let (data, stamp) = self
            .source
            .read_stamped(data_did)?
            .ok_or(AssetError::NotFound { did: data_did.0 })?;
        // Header offset 4: the info DID or 0 (`landblock.md` §3.2).
        let named_info = data
            .get(4..8)
            .map(|b| Did(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            .filter(|d| *d == key.info_did());
        let mut out = BundleRead {
            records: vec![(data_did, data, stamp)],
            failures: Vec::new(),
        };
        let mut optional = vec![key.block_map_did(), key.properties_did()];
        if let Some(info) = named_info {
            optional.insert(0, info);
        }
        for did in optional {
            match self.source.read_stamped(did) {
                Ok(Some((bytes, stamp))) => out.records.push((did, bytes, stamp)),
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(did = %format_args!("{:#010x}", did.0), error = %e, "optional landblock record unreadable");
                    out.failures.push((did, e));
                }
            }
        }
        Ok(out)
    }
}

impl RecordSource for BundleSource {
    fn read_record(&self, did: u32) -> Result<Vec<u8>, AssetError> {
        let key = match LandblockId::from_did(Did(did)) {
            Some(id) if id.kind == LandblockKind::Heightmap => {
                LandblockKey::new(id.region, id.x, id.y)
                    .map_err(|e| AssetError::source_error(did, e))?
            }
            _ => {
                return Err(AssetError::source_error(
                    did,
                    WorldError::NotABundleKey { did },
                ));
            }
        };
        let read = self.read_bundle_records(key)?;
        let framed: Vec<FramedRecord<'_>> = read
            .records
            .iter()
            .map(|(did, bytes, stamp)| FramedRecord {
                did: *did,
                stamp: *stamp,
                bytes,
            })
            .collect();
        let texts: Vec<(Did, String)> = read
            .failures
            .iter()
            .map(|(d, e)| (*d, e.to_string()))
            .collect();
        let failures: Vec<FramedFailure<'_>> = texts
            .iter()
            .map(|(did, error)| FramedFailure { did: *did, error })
            .collect();
        BundleFrame::encode_with_failures(&framed, &failures)
            .map_err(|e| AssetError::source_error(did, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let recs = [
            FramedRecord {
                did: Did(0x8001_0203),
                stamp: Some(39),
                bytes: &[1, 2, 3],
            },
            FramedRecord {
                did: Did(0x8011_0203),
                stamp: None,
                bytes: &[],
            },
        ];
        let bytes = BundleFrame::encode(&recs).unwrap();
        let f = BundleFrame::parse(&bytes).unwrap();
        assert_eq!(f.records, recs);
        assert_eq!(f.get(Did(0x8011_0203)).unwrap().stamp, None);
        assert!(f.get(Did(1)).is_none());
        // Every truncation and a trailing byte are rejected.
        for n in 0..bytes.len() {
            assert!(BundleFrame::parse(&bytes[..n]).is_err(), "{n}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(BundleFrame::parse(&extra).is_err());
        let mut bad = bytes.clone();
        bad[9] = 3; // first stamp flag
        assert!(BundleFrame::parse(&bad).is_err());
        bad[9] = 2; // a failure entry whose text is [1, 2, 3] but carries a stamp
        assert!(BundleFrame::parse(&bad).is_err());
        let mut many = bytes;
        many[4] = 9;
        assert!(BundleFrame::parse(&many).is_err());
        let nine = [recs[1]; 9];
        assert!(BundleFrame::encode(&nine).is_err());
    }

    #[test]
    fn frame_failures_round_trip() {
        let recs = [FramedRecord {
            did: Did(0x8001_0203),
            stamp: Some(39),
            bytes: &[1, 2, 3],
        }];
        let long = "é".repeat(MAX_FAILURE_TEXT);
        let fails = [
            FramedFailure {
                did: Did(0x8041_0203),
                error: "bad sector",
            },
            FramedFailure {
                did: Did(0x8011_0203),
                error: &long,
            },
        ];
        let bytes = BundleFrame::encode_with_failures(&recs, &fails).unwrap();
        let f = BundleFrame::parse(&bytes).unwrap();
        assert_eq!(f.records, recs);
        assert_eq!(f.failure(Did(0x8041_0203)), Some("bad sector"));
        assert!(f.get(Did(0x8041_0203)).is_none());
        let cut = f.failure(Did(0x8011_0203)).unwrap();
        assert!(cut.len() <= MAX_FAILURE_TEXT && cut.chars().all(|c| c == 'é'));
        for n in 0..bytes.len() {
            assert!(BundleFrame::parse(&bytes[..n]).is_err(), "{n}");
        }
        // Non-UTF-8 failure text is rejected.
        let mut bad = BundleFrame::encode_with_failures(&[], &fails[..1]).unwrap();
        let last = bad.len() - 1;
        bad[last] = 0xFF;
        assert!(BundleFrame::parse(&bad).is_err());
        let nine = [fails[0]; 9];
        assert!(BundleFrame::encode_with_failures(&[], &nine).is_err());
    }

    /// Serves `inner` but fails reads of one DID.
    struct FailOne {
        inner: MemoryWorldSource,
        bad: Did,
    }

    impl WorldSource for FailOne {
        fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
            if did == self.bad {
                return Err(AssetError::source_error(
                    did.0,
                    std::io::Error::other("bad sector"),
                ));
            }
            self.inner.read_stamped(did)
        }
    }

    #[test]
    fn optional_read_errors_do_not_fail_the_job() {
        let key = LandblockKey::new(1, 1, 1).unwrap();
        let mut m = MemoryWorldSource::new();
        m.insert(
            key.data_did(),
            key.data_did().0.to_le_bytes().to_vec(),
            Some(39),
        );
        m.insert(key.properties_did(), vec![7u8], None);
        let src = BundleSource::new(Arc::new(FailOne {
            inner: m.clone(),
            bad: key.properties_did(),
        }));
        let read = src.read_bundle_records(key).unwrap();
        assert_eq!(read.records.len(), 1);
        assert_eq!(read.failures.len(), 1);
        assert_eq!(read.failures[0].0, key.properties_did());
        let bytes = src.read_record(key.data_did().0).unwrap();
        let f = BundleFrame::parse(&bytes).unwrap();
        assert!(
            f.failure(key.properties_did())
                .unwrap()
                .contains("bad sector")
        );
        // An unreadable data record still fails the job.
        let src = BundleSource::new(Arc::new(FailOne {
            inner: m,
            bad: key.data_did(),
        }));
        assert!(matches!(
            src.read_record(key.data_did().0),
            Err(AssetError::Source { .. })
        ));
    }

    #[test]
    fn contains_does_not_need_the_payload() {
        let mut m = MemoryWorldSource::new();
        m.insert(Did(0x8001_0000), vec![1u8], None);
        assert!(WorldSource::contains(&m, Did(0x8001_0000)).unwrap());
        assert!(!WorldSource::contains(&m, Did(0x8001_0001)).unwrap());
        let arc: Arc<dyn WorldSource> = Arc::new(m.clone());
        assert!(arc.contains(Did(0x8001_0000)).unwrap());
        assert_eq!(
            tessera_props::RecordSource::read_record(&m, Did(0x8001_0000)).unwrap(),
            Some(vec![1u8])
        );
    }

    #[test]
    fn bundle_source_reads_optional_records() {
        let key = LandblockKey::new(1, 2, 3).unwrap();
        let mut m = MemoryWorldSource::new();
        let mut data = key.data_did().0.to_le_bytes().to_vec();
        data.extend_from_slice(&key.info_did().0.to_le_bytes());
        m.insert(key.data_did(), data.clone(), Some(39));
        m.insert(key.info_did(), vec![9u8], Some(31));
        m.insert(key.properties_did(), vec![7u8], None);
        let src = BundleSource::new(Arc::new(m.clone()));
        let bytes = src.read_record(key.data_did().0).unwrap();
        let f = BundleFrame::parse(&bytes).unwrap();
        let dids: Vec<u32> = f.records.iter().map(|r| r.did.0).collect();
        assert_eq!(
            dids,
            vec![key.data_did().0, key.info_did().0, key.properties_did().0]
        );
        assert_eq!(f.records[0].stamp, Some(39));

        // The info record is not read unless the header names it.
        data[4..8].copy_from_slice(&[0; 4]);
        m.insert(key.data_did(), data, Some(39));
        let src = BundleSource::new(Arc::new(m.clone()));
        let f2 = src.read_bundle_records(key).unwrap();
        assert_eq!(f2.records.len(), 2);
        assert!(f2.failures.is_empty());

        // Missing data and non-data keys are errors.
        m.remove(key.data_did());
        let src = BundleSource::new(Arc::new(m));
        assert!(matches!(
            src.read_record(key.data_did().0),
            Err(AssetError::NotFound { .. })
        ));
        assert!(src.read_record(key.block_map_did().0).is_err());
        assert!(src.read_record(0x4100_0000).is_err());
    }
}
