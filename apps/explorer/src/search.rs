// SPDX-License-Identifier: GPL-3.0-or-later
//! DID search over the archive indexes (hex and prefix forms) and the name index
//! (DID-map labels, property names). Searches run on a background thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use crate::decoders::NameIndex;
use crate::index::{ArchiveIndex, IndexEntry};
use crate::query::DidQuery;

/// Most hits one search returns. Hits are 12 bytes each ([`SearchHit`]), so a full
/// list takes at most 48 MiB (a wildcard search of a full install, about 3.8 million
/// entries, about 44 MiB).
pub const MAX_HITS: usize = 4_000_000;

/// One search hit, kept compact: the DID, archive and name are looked up through the
/// [`SearchResults`] that hold it ([`SearchResults::hit`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchHit {
    /// Entry `entry` of [`SearchResults::indexes`]`[index]`.
    Entry {
        /// Position in [`SearchResults::indexes`].
        index: u32,
        /// Entry index in that archive index.
        entry: u32,
    },
    /// A DID in no index, offered anyway (it is read through routing).
    Routed {
        /// The DID.
        did: u32,
    },
    /// Entry `name` of [`SearchResults::names`].
    Name {
        /// Entry index in the name index.
        name: u32,
    },
}

/// One hit resolved for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitView<'a> {
    /// The DID found (`None` for a name without a DID, such as most property names).
    pub did: Option<u32>,
    /// The archive it was found in (index into the set's members), if known.
    pub member: Option<usize>,
    /// Its directory entry (for size display), if known.
    pub entry: Option<&'a IndexEntry>,
    /// What matched, for name searches.
    pub text: Option<&'a str>,
    /// Where the name comes from (name searches), or a remark.
    pub detail: Option<&'a str>,
}

/// The result of one search.
#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    /// The query as typed.
    pub query: String,
    /// Hits in DID order (hex searches) or name order (name searches).
    pub hits: Vec<SearchHit>,
    /// Whether [`MAX_HITS`] cut the list.
    pub truncated: bool,
    /// A remark for the user (index still building, no name index yet, ...).
    pub note: Option<String>,
    /// The archive indexes [`SearchHit::Entry`] refers to.
    pub indexes: Vec<Arc<ArchiveIndex>>,
    /// The name index [`SearchHit::Name`] refers to.
    pub names: Option<Arc<NameIndex>>,
}

/// Remark shown for a [`SearchHit::Routed`] hit.
pub const ROUTED_DETAIL: &str = "not in an index; read through routing";

impl SearchResults {
    /// Number of hits.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// Whether there are no hits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Hit `i` resolved for display (`None` past the end or for a dangling reference).
    #[must_use]
    pub fn hit(&self, i: usize) -> Option<HitView<'_>> {
        Some(match *self.hits.get(i)? {
            SearchHit::Entry { index, entry } => {
                let idx = self.indexes.get(index as usize)?;
                let e = idx.entries.get(entry as usize)?;
                HitView {
                    did: Some(e.did),
                    member: Some(idx.member),
                    entry: Some(e),
                    text: None,
                    detail: None,
                }
            }
            SearchHit::Routed { did } => HitView {
                did: Some(did),
                member: None,
                entry: None,
                text: None,
                detail: Some(ROUTED_DETAIL),
            },
            SearchHit::Name { name } => {
                let n = self.names.as_ref()?.entries.get(name as usize)?;
                HitView {
                    did: n.did,
                    member: None,
                    entry: None,
                    text: Some(&n.text),
                    detail: Some(&n.detail),
                }
            }
        })
    }

    /// The DID of hit `i`, if it has one.
    #[must_use]
    pub fn did(&self, i: usize) -> Option<u32> {
        self.hit(i)?.did
    }

    /// The DIDs of all hits, in order (hits without a DID are skipped).
    pub fn dids(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.hits.len()).filter_map(|i| self.did(i))
    }
}

/// Every indexed entry in `range`, across all archives, sorted by DID then archive.
/// [`SearchHit::Entry::index`] is the position in `indexes`.
#[must_use]
pub fn search_range(
    indexes: &[Arc<ArchiveIndex>],
    range: std::ops::RangeInclusive<u32>,
    max: usize,
    cancel: &AtomicBool,
) -> (Vec<SearchHit>, bool) {
    let mut hits = Vec::new();
    let mut truncated = false;
    'archives: for (pos, idx) in indexes.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(index) = u32::try_from(pos) else { break };
        for i in idx.range_of(range.clone()) {
            if hits.len() >= max {
                truncated = true;
                break 'archives;
            }
            let Ok(entry) = u32::try_from(i) else { break };
            hits.push(SearchHit::Entry { index, entry });
        }
    }
    let key = |h: &SearchHit| match *h {
        SearchHit::Entry { index, entry } => {
            let idx = &indexes[index as usize];
            (idx.entries[entry as usize].did, idx.member)
        }
        SearchHit::Routed { did } => (did, usize::MAX),
        SearchHit::Name { .. } => (u32::MAX, usize::MAX),
    };
    hits.sort_unstable_by_key(key);
    (hits, truncated)
}

/// Every name containing `needle` (already lowercased).
#[must_use]
pub fn search_names(
    names: &NameIndex,
    needle: &str,
    max: usize,
    cancel: &AtomicBool,
) -> (Vec<SearchHit>, bool) {
    let mut found: Vec<u32> = Vec::new();
    let mut truncated = false;
    for (n, e) in names.entries.iter().enumerate() {
        if n % 65536 == 0 && cancel.load(Ordering::Relaxed) {
            break;
        }
        if e.lower.contains(needle) {
            if found.len() >= max {
                truncated = true;
                break;
            }
            let Ok(n) = u32::try_from(n) else { break };
            found.push(n);
        }
    }
    // Exact matches first, then names ending with the needle, then shorter names.
    found.sort_by_key(|&n| {
        let t = &names.entries[n as usize].lower;
        (t != needle, !t.ends_with(needle), t.len(), n)
    });
    let hits = found
        .into_iter()
        .map(|name| SearchHit::Name { name })
        .collect();
    (hits, truncated)
}

/// Runs `query` against the given indexes on the calling thread.
#[must_use]
pub fn run_search(
    text: &str,
    query: &DidQuery,
    indexes: &[Arc<ArchiveIndex>],
    names: Option<Arc<NameIndex>>,
    indexing_complete: bool,
    cancel: &AtomicBool,
) -> SearchResults {
    let mut out = SearchResults {
        query: text.to_owned(),
        ..SearchResults::default()
    };
    match query {
        DidQuery::Empty => {}
        DidQuery::Exact(_) | DidQuery::Range { .. } => {
            let Some(range) = query.range() else {
                return out;
            };
            let (hits, truncated) = search_range(indexes, range, MAX_HITS, cancel);
            out.hits = hits;
            out.truncated = truncated;
            out.indexes = indexes.to_vec();
            if let DidQuery::Exact(did) = *query
                && out.hits.is_empty()
            {
                // Not (yet) in an index: still offer it, read through routing.
                out.hits.push(SearchHit::Routed { did });
            }
            if !indexing_complete {
                out.note =
                    Some("archives are still being indexed; results may be incomplete".into());
            }
        }
        DidQuery::Name(needle) => match names {
            Some(names) => {
                let (hits, truncated) = search_names(&names, needle, MAX_HITS, cancel);
                out.hits = hits;
                out.truncated = truncated;
                if out.hits.is_empty() {
                    out.note = Some("no DID-map label or property name contains this text".into());
                }
                out.names = Some(names);
            }
            None => {
                out.note = Some("the name index is still loading; try again in a moment".into());
            }
        },
    }
    out
}

/// A search running on a background thread.
#[derive(Debug)]
pub struct Search {
    rx: Receiver<SearchResults>,
    cancel: Arc<AtomicBool>,
}

impl Search {
    /// Starts a search; `notify` runs when it finishes.
    pub fn spawn(
        text: String,
        query: DidQuery,
        indexes: Vec<Arc<ArchiveIndex>>,
        names: Option<Arc<NameIndex>>,
        indexing_complete: bool,
        notify: impl FnOnce() + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&cancel);
        let spawned = std::thread::Builder::new()
            .name("explorer-search".into())
            .spawn(move || {
                let r = run_search(&text, &query, &indexes, names, indexing_complete, &c);
                if !c.load(Ordering::Relaxed) {
                    let _ = tx.send(r);
                    notify();
                }
            });
        if let Err(e) = spawned {
            tracing::error!(error = %e, "could not start a search thread");
        }
        Self { rx, cancel }
    }

    /// The results, once ready. Never blocks.
    pub fn poll(&self) -> Option<SearchResults> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::parse_query;

    fn idx(member: usize, dids: &[u32]) -> Arc<ArchiveIndex> {
        Arc::new(ArchiveIndex::from_entries(
            member,
            format!("a{member}"),
            dids.iter()
                .map(|&did| IndexEntry {
                    did,
                    length: 4,
                    flags: 0,
                })
                .collect(),
        ))
    }

    #[test]
    fn range_search_merges_archives() {
        let no = AtomicBool::new(false);
        let indexes = vec![
            idx(0, &[0x4100_0003, 0x2500_0001]),
            idx(1, &[0x4100_0001, 0x4100_0003]),
        ];
        let r = run_search("0x41", &parse_query("0x41"), &indexes, None, true, &no);
        let got: Vec<(Option<u32>, Option<usize>)> = (0..r.len())
            .map(|i| r.hit(i).map(|h| (h.did, h.member)).unwrap())
            .collect();
        assert_eq!(
            got,
            vec![
                (Some(0x4100_0001), Some(1)),
                (Some(0x4100_0003), Some(0)),
                (Some(0x4100_0003), Some(1))
            ]
        );
        assert!(r.note.is_none());
        let (hits, cut) = search_range(&indexes, 0..=u32::MAX, 2, &no);
        assert_eq!(hits.len(), 2);
        assert!(cut);
    }

    #[test]
    fn exact_miss_is_still_offered() {
        let no = AtomicBool::new(false);
        let r = run_search(
            "0x0A000001",
            &parse_query("0x0A000001"),
            &[],
            None,
            false,
            &no,
        );
        assert_eq!(r.len(), 1);
        let h = r.hit(0).unwrap();
        assert_eq!((h.did, h.member), (Some(0x0A00_0001), None));
        assert_eq!(h.detail, Some(ROUTED_DETAIL));
        assert!(r.note.is_some());
    }

    #[test]
    fn name_search() {
        let no = AtomicBool::new(false);
        let mut names = NameIndex::default();
        names.push("STRINGTABLE".into(), Some(0x2800_0001), "root".into());
        names.push("EMAPPER/StringType".into(), Some(0x2300_0001), "map".into());
        names.push("Item_Name".into(), None, "property".into());
        let names = Arc::new(names);
        let r = run_search(
            "string",
            &parse_query("string"),
            &[],
            Some(Arc::clone(&names)),
            true,
            &no,
        );
        assert_eq!(r.len(), 2);
        assert_eq!(r.hit(0).unwrap().text, Some("STRINGTABLE"));
        assert_eq!(r.did(0), Some(0x2800_0001));
        let r = run_search(
            "item",
            &parse_query("item"),
            &[],
            Some(Arc::clone(&names)),
            true,
            &no,
        );
        // A property name without a DID is a hit without a DID.
        assert_eq!(r.len(), 1);
        assert_eq!(r.hit(0).unwrap().text, Some("Item_Name"));
        assert_eq!(r.did(0), None);
        assert_eq!(r.dids().count(), 0);
        let r = run_search("zzz", &parse_query("zzz"), &[], Some(names), true, &no);
        assert!(r.hits.is_empty());
        assert!(r.note.is_some());
        let r = run_search("item", &parse_query("item"), &[], None, true, &no);
        assert!(r.note.is_some());
    }

    #[test]
    fn background_search_delivers() {
        let s = Search::spawn(
            "0x41".into(),
            parse_query("0x41"),
            vec![idx(0, &[0x4100_0001])],
            None,
            true,
            || {},
        );
        let start = std::time::Instant::now();
        loop {
            if let Some(r) = s.poll() {
                assert_eq!(r.dids().collect::<Vec<_>>(), vec![0x4100_0001]);
                break;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(10));
            std::thread::yield_now();
        }
    }

    #[test]
    fn hits_are_compact_and_dangling_hits_resolve_to_none() {
        assert!(std::mem::size_of::<SearchHit>() <= 12);
        let r = SearchResults {
            hits: vec![
                SearchHit::Entry { index: 3, entry: 0 },
                SearchHit::Name { name: 7 },
            ],
            ..SearchResults::default()
        };
        assert!(r.hit(0).is_none());
        assert!(r.hit(1).is_none());
        assert!(r.hit(2).is_none());
    }
}
