// SPDX-License-Identifier: GPL-3.0-or-later
//! The archive tree: every DID of every archive, grouped by type prefix.
//!
//! Directory trees are walked on background threads ([`Indexer`]); each finished
//! archive is posted as an [`ArchiveIndex`] that the UI picks up without blocking.
//! Entries are kept compactly (12 bytes each), so the 3–4 million entries of a full
//! install take well under 100 MiB.

use std::ops::{Range, RangeInclusive};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tessera_dat::{Archive, ArchiveSet, Entry};

/// Most entries indexed per archive; a larger directory is truncated (and flagged).
/// The largest observed archive has about 850,000 entries.
pub const MAX_INDEX_ENTRIES: usize = 16 * 1024 * 1024;

/// Archives indexed concurrently.
pub const INDEX_THREADS: usize = 4;

/// One directory entry, as kept by the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexEntry {
    /// The DID.
    pub did: u32,
    /// Stored record length in bytes (before inflation).
    pub length: u32,
    /// Entry flags (bit 0: compressed).
    pub flags: u16,
}

impl From<&Entry> for IndexEntry {
    fn from(e: &Entry) -> Self {
        Self {
            did: e.did,
            length: e.length,
            flags: e.flags,
        }
    }
}

impl IndexEntry {
    /// Whether the record is stored compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & Entry::FLAG_COMPRESSED != 0
    }
}

/// A run of entries sharing one DID type prefix (high byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeGroup {
    /// The type prefix.
    pub prefix: u8,
    /// Entry index range in [`ArchiveIndex::entries`].
    pub start: usize,
    /// End of the range (exclusive).
    pub end: usize,
}

impl TypeGroup {
    /// Entry index range.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the group is empty (never true for groups built here).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }
}

/// Every entry of one archive, sorted by DID, with its type groups.
#[derive(Debug, Clone, Default)]
pub struct ArchiveIndex {
    /// Index of the archive in [`ArchiveSet::members`].
    pub member: usize,
    /// Display name of the archive (`surface_aux_1`).
    pub name: String,
    /// Entries in ascending DID order.
    pub entries: Vec<IndexEntry>,
    /// Groups by type prefix, ascending.
    pub groups: Vec<TypeGroup>,
    /// The error that stopped the walk early, if any (entries before it are kept).
    pub error: Option<String>,
    /// Whether the walk stopped at [`MAX_INDEX_ENTRIES`].
    pub truncated: bool,
}

impl ArchiveIndex {
    /// Builds an index from entries in any order (sorting and de-duplicating them).
    #[must_use]
    pub fn from_entries(member: usize, name: String, mut entries: Vec<IndexEntry>) -> Self {
        if !entries.is_sorted_by_key(|e| e.did) {
            entries.sort_by_key(|e| e.did);
        }
        entries.dedup_by_key(|e| e.did);
        let groups = group_by_prefix(&entries);
        Self {
            member,
            name,
            entries,
            groups,
            error: None,
            truncated: false,
        }
    }

    /// The entry index range of DIDs within `range`.
    #[must_use]
    pub fn range_of(&self, range: RangeInclusive<u32>) -> Range<usize> {
        let lo = self.entries.partition_point(|e| e.did < *range.start());
        let hi = self.entries.partition_point(|e| e.did <= *range.end());
        lo..hi.max(lo)
    }

    /// The entry for `did`, if this archive holds it.
    #[must_use]
    pub fn find(&self, did: u32) -> Option<&IndexEntry> {
        self.entries
            .binary_search_by_key(&did, |e| e.did)
            .ok()
            .and_then(|i| self.entries.get(i))
    }

    /// The group holding entry index `i`.
    #[must_use]
    pub fn group_of(&self, i: usize) -> Option<usize> {
        let g = self.groups.partition_point(|g| g.end <= i);
        self.groups.get(g).filter(|g| g.range().contains(&i))?;
        Some(g)
    }
}

/// Splits sorted entries into runs of equal high byte.
#[must_use]
pub fn group_by_prefix(entries: &[IndexEntry]) -> Vec<TypeGroup> {
    let mut groups: Vec<TypeGroup> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let prefix = (e.did >> 24) as u8;
        match groups.last_mut() {
            Some(g) if g.prefix == prefix => g.end = i + 1,
            _ => groups.push(TypeGroup {
                prefix,
                start: i,
                end: i + 1,
            }),
        }
    }
    groups
}

/// Walks one archive's directory into an index. `scanned` counts entries as they are
/// read; `cancel` stops the walk early (the partial index is returned).
pub fn index_archive(
    archive: &Archive,
    member: usize,
    name: String,
    scanned: &AtomicU64,
    cancel: &AtomicBool,
) -> ArchiveIndex {
    let mut entries = Vec::new();
    let mut error = None;
    let mut truncated = false;
    for (n, item) in archive.entries().enumerate() {
        if n % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            error = Some("cancelled".to_owned());
            break;
        }
        if entries.len() >= MAX_INDEX_ENTRIES {
            truncated = true;
            break;
        }
        match item {
            Ok(e) => {
                entries.push(IndexEntry::from(&e));
                scanned.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::warn!(archive = %name, error = %e, "directory walk stopped");
                error = Some(e.to_string());
                break;
            }
        }
    }
    let mut index = ArchiveIndex::from_entries(member, name, entries);
    index.error = error;
    index.truncated = truncated;
    index
}

/// Per-archive progress of an [`Indexer`]: entries read so far, per member.
#[derive(Debug)]
struct IndexProgress {
    scanned: Vec<AtomicU64>,
}

/// Builds the indexes of every archive of a set on background threads.
pub struct Indexer {
    rx: Receiver<ArchiveIndex>,
    progress: Arc<IndexProgress>,
    cancel: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    total: usize,
    received: usize,
}

impl std::fmt::Debug for Indexer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indexer")
            .field("total", &self.total)
            .field("received", &self.received)
            .finish_non_exhaustive()
    }
}

impl Indexer {
    /// Starts indexing every member of `set`, smallest archive first, on up to
    /// [`INDEX_THREADS`] threads. `notify` is called after each archive finishes (for
    /// example to request a repaint).
    pub fn spawn(set: Arc<ArchiveSet>, notify: impl Fn() + Send + Sync + 'static) -> Self {
        let n = set.members().len();
        let progress = Arc::new(IndexProgress {
            scanned: (0..n).map(|_| AtomicU64::new(0)).collect(),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        // Largest first: workers pop from the end, so the smallest archives come first.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(set.member(i).archive.file_len()));
        let queue = Arc::new(Mutex::new(order));
        let (tx, rx): (Sender<ArchiveIndex>, Receiver<ArchiveIndex>) = mpsc::channel();
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(notify);
        let mut threads = Vec::new();
        for t in 0..INDEX_THREADS.min(n) {
            let (set, queue, tx, progress, cancel, notify) = (
                Arc::clone(&set),
                Arc::clone(&queue),
                tx.clone(),
                Arc::clone(&progress),
                Arc::clone(&cancel),
                Arc::clone(&notify),
            );
            let spawned = std::thread::Builder::new()
                .name(format!("explorer-index-{t}"))
                .spawn(move || {
                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop();
                        let Some(i) = next else { break };
                        let member = set.member(i);
                        let started = std::time::Instant::now();
                        let index = index_archive(
                            &member.archive,
                            i,
                            member.id.to_string(),
                            &progress.scanned[i],
                            &cancel,
                        );
                        tracing::debug!(
                            archive = %index.name,
                            entries = index.entries.len(),
                            ms = started.elapsed().as_millis() as u64,
                            "archive indexed"
                        );
                        if tx.send(index).is_err() {
                            break;
                        }
                        notify();
                    }
                });
            match spawned {
                Ok(h) => threads.push(h),
                Err(e) => tracing::error!(error = %e, "could not start an index thread"),
            }
        }
        Self {
            rx,
            progress,
            cancel,
            threads,
            total: n,
            received: 0,
        }
    }

    /// Indexes finished since the last call. Never blocks.
    pub fn poll(&mut self) -> Vec<ArchiveIndex> {
        let out: Vec<ArchiveIndex> = self.rx.try_iter().collect();
        self.received += out.len();
        out
    }

    /// Whether every archive has been received.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.received >= self.total
    }

    /// Entries read so far for member `i`.
    #[must_use]
    pub fn scanned(&self, i: usize) -> u64 {
        self.progress
            .scanned
            .get(i)
            .map_or(0, |a| a.load(Ordering::Relaxed))
    }

    /// Entries read so far over all archives.
    #[must_use]
    pub fn scanned_total(&self) -> u64 {
        self.progress
            .scanned
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .sum()
    }

    /// Number of archives received / total.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (self.received, self.total)
    }
}

impl Drop for Indexer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Threads stop at their next check; they are not joined so that dropping never
        // blocks the UI thread.
        self.threads.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(did: u32) -> IndexEntry {
        IndexEntry {
            did,
            length: 1,
            flags: 0,
        }
    }

    #[test]
    fn groups_and_ranges() {
        let idx = ArchiveIndex::from_entries(
            0,
            "t".into(),
            vec![
                e(0x4100_0002),
                e(0x2500_0001),
                e(0x4100_0001),
                e(0x4100_0001),
                e(0x4200_0000),
            ],
        );
        assert_eq!(idx.entries.len(), 4);
        assert_eq!(
            idx.groups
                .iter()
                .map(|g| (g.prefix, g.len()))
                .collect::<Vec<_>>(),
            vec![(0x25, 1), (0x41, 2), (0x42, 1)]
        );
        assert_eq!(idx.range_of(0x4100_0000..=0x41FF_FFFF), 1..3);
        assert_eq!(idx.range_of(0x5000_0000..=0x5FFF_FFFF), 4..4);
        assert_eq!(idx.range_of(0..=u32::MAX), 0..4);
        assert!(idx.find(0x4100_0002).is_some());
        assert!(idx.find(0x4100_0003).is_none());
        assert_eq!(idx.group_of(2), Some(1));
        assert_eq!(idx.group_of(3), Some(2));
        assert_eq!(idx.group_of(4), None);
        assert!(!idx.groups[0].is_empty());
    }

    #[test]
    fn empty_index() {
        let idx = ArchiveIndex::from_entries(0, "e".into(), Vec::new());
        assert!(idx.groups.is_empty());
        assert_eq!(idx.range_of(0..=10), 0..0);
        assert_eq!(idx.group_of(0), None);
    }

    #[test]
    fn compressed_flag() {
        let mut x = e(1);
        assert!(!x.is_compressed());
        x.flags = Entry::FLAG_COMPRESSED;
        assert!(x.is_compressed());
    }
}
