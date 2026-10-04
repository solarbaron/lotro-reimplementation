// SPDX-License-Identifier: GPL-3.0-or-later
//! Directory B-tree: node layout, lookup and in-order traversal (spec §5).

use std::collections::HashSet;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::archive::Archive;
use crate::bytes::u32_at;
use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::{
    Did, ENTRY_LEN, HEADER_LEN, MAX_NODE_ENTRIES, MAX_TREE_DEPTH, MIN_NODE_ALLOC, NODE_CHILD_SLOTS,
    NODE_PAYLOAD_LEN,
};

/// Payload offset of the entry count (spec §5.1).
const COUNT_OFFSET: usize = NODE_CHILD_SLOTS * 8;
/// Payload offset of the first entry (spec §5.1).
const ENTRIES_OFFSET: usize = COUNT_OFFSET + 4;

/// A child pointer `{size, offset}` of a directory node (spec §5.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ChildPtr {
    /// Allocation size of the child node.
    pub size: u32,
    /// Absolute offset of the child node.
    pub offset: u32,
}

impl ChildPtr {
    /// Whether both fields are zero (marks a leaf when it is child 0).
    pub fn is_null(&self) -> bool {
        self.size == 0 && self.offset == 0
    }
}

/// A node exactly as stored, before structural checks. Used by the validator.
#[derive(Debug, Clone)]
pub(crate) struct RawNode {
    /// All 62 child-pointer slots, including stale ones.
    pub children: Vec<ChildPtr>,
    /// The stored entry count `n` (may be out of range in corrupt data).
    pub count: u32,
    /// The first `min(n, 61)` entries.
    pub entries: Vec<Entry>,
    /// Physical extents of the node allocation.
    pub extents: Vec<(u64, u64)>,
}

/// A structurally checked node, as used by lookup and iteration.
#[derive(Debug)]
pub(crate) struct Node {
    pub entries: Vec<Entry>,
    /// Empty for a leaf; otherwise exactly `entries.len() + 1` pointers.
    pub children: Vec<ChildPtr>,
}

impl Node {
    pub(crate) fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

fn parse_payload(p: &[u8]) -> (Vec<ChildPtr>, u32, Vec<Entry>) {
    let children = (0..NODE_CHILD_SLOTS)
        .map(|i| ChildPtr {
            size: u32_at(p, i * 8),
            offset: u32_at(p, i * 8 + 4),
        })
        .collect();
    let count = u32_at(p, COUNT_OFFSET);
    let n = (count as usize).min(MAX_NODE_ENTRIES);
    let entries = (0..n)
        .map(|i| {
            let at = ENTRIES_OFFSET + i * ENTRY_LEN;
            Entry::parse(p.get(at..at + ENTRY_LEN).unwrap_or(&[]))
        })
        .collect();
    (children, count, entries)
}

impl Archive {
    /// Reads a node allocation without structural checks beyond bounds.
    pub(crate) fn read_raw_node(&self, ptr: ChildPtr) -> Result<RawNode> {
        if ptr.size < MIN_NODE_ALLOC {
            return Err(Error::corrupt(
                u64::from(ptr.offset),
                format!("node allocation size {} < {MIN_NODE_ALLOC}", ptr.size),
            ));
        }
        let layout = self.layout_at(ptr.offset, ptr.size)?;
        let payload = self.read_payload(&layout, NODE_PAYLOAD_LEN as u64)?;
        let (children, count, entries) = parse_payload(&payload);
        Ok(RawNode {
            children,
            count,
            entries,
            extents: layout.extents(),
        })
    }

    /// Reads and structurally checks a node, using the interior-node cache.
    pub(crate) fn load_node(&self, ptr: ChildPtr) -> Result<Arc<Node>> {
        let key = (ptr.offset, ptr.size);
        if let Ok(cache) = self.node_cache.read()
            && let Some(n) = cache.get(&key)
        {
            return Ok(Arc::clone(n));
        }
        let raw = self.read_raw_node(ptr)?;
        let node = Arc::new(self.check_node(ptr, raw)?);
        let cap = self.options().node_cache_capacity;
        if !node.is_leaf()
            && cap > 0
            && let Ok(mut cache) = self.node_cache.write()
            && cache.len() < cap
        {
            cache.insert(key, Arc::clone(&node));
        }
        Ok(node)
    }

    fn check_node(&self, ptr: ChildPtr, raw: RawNode) -> Result<Node> {
        let at = u64::from(ptr.offset);
        let n = raw.count as usize;
        if n > MAX_NODE_ENTRIES {
            return Err(Error::corrupt(at, format!("node entry count {n} > 61")));
        }
        let leaf = raw.children.first().is_none_or(ChildPtr::is_null);
        let used = raw.children.get(..=n).unwrap_or(&[]);
        if leaf {
            if used.iter().any(|c| !c.is_null()) {
                return Err(Error::corrupt(at, "leaf node has non-zero child pointers"));
            }
            return Ok(Node {
                entries: raw.entries,
                children: Vec::new(),
            });
        }
        if n == 0 {
            return Err(Error::corrupt(at, "interior node has no entries"));
        }
        for c in used {
            if c.is_null() || c.size == 0 || c.offset == 0 {
                return Err(Error::corrupt(at, "interior node has a zero child pointer"));
            }
            if c.size < MIN_NODE_ALLOC {
                return Err(Error::corrupt(
                    at,
                    format!("child node size {} < {MIN_NODE_ALLOC}", c.size),
                ));
            }
            let end = u64::from(c.offset) + u64::from(c.size);
            if u64::from(c.offset) < HEADER_LEN || end > self.file_len() {
                return Err(Error::OutOfBounds {
                    what: "child node",
                    offset: u64::from(c.offset),
                    len: u64::from(c.size),
                    file_len: self.file_len(),
                });
            }
        }
        Ok(Node {
            entries: raw.entries,
            children: used.to_vec(),
        })
    }

    /// Looks up the directory entry for `did` (spec §5.5).
    ///
    /// Returns `Ok(None)` if the DID is not in this archive. Descent is bounded by
    /// [`crate::MAX_TREE_DEPTH`].
    pub fn lookup(&self, did: Did) -> Result<Option<Entry>> {
        let mut ptr = self.root();
        for _ in 0..=MAX_TREE_DEPTH {
            let node = self.load_node(ptr)?;
            match node.entries.binary_search_by_key(&did, |e| e.did) {
                Ok(i) => return Ok(node.entries.get(i).copied()),
                Err(i) => {
                    if node.is_leaf() {
                        return Ok(None);
                    }
                    match node.children.get(i) {
                        Some(c) => ptr = *c,
                        None => return Ok(None),
                    }
                }
            }
        }
        Err(Error::TreeTooDeep {
            limit: MAX_TREE_DEPTH,
        })
    }

    /// Iterates over all entries in ascending DID order (spec §5.6).
    ///
    /// The iterator yields `Err` once and then stops if the tree is corrupt: a node
    /// reached twice, a tree deeper than [`crate::MAX_TREE_DEPTH`], an invalid node, or
    /// DIDs that are not strictly ascending.
    pub fn entries(&self) -> Entries<'_> {
        self.entries_range(..)
    }

    /// Iterates in ascending order over the entries whose DID is `≥ start`.
    ///
    /// Only the nodes on the path to `start` and those holding later entries are read.
    /// Error behaviour is that of [`Archive::entries`].
    pub fn entries_from(&self, start: Did) -> Entries<'_> {
        self.entries_range(start..)
    }

    /// Iterates in ascending order over the entries whose DID lies in `range`.
    ///
    /// The walk starts by descending to the lower bound (like [`Archive::lookup`]) and
    /// stops at the first entry past the upper bound, so selecting one DID class (for
    /// example `0x2500_0000..0x2600_0000`) does not read the rest of the tree. Error
    /// behaviour is that of [`Archive::entries`].
    pub fn entries_range(&self, range: impl RangeBounds<Did>) -> Entries<'_> {
        let start = match range.start_bound() {
            Bound::Included(&s) => Some(s),
            Bound::Excluded(&s) => s.checked_add(1),
            Bound::Unbounded => Some(0),
        };
        let end = match range.end_bound() {
            Bound::Included(&e) => Bound::Included(e),
            Bound::Excluded(&e) => Bound::Excluded(e),
            Bound::Unbounded => Bound::Unbounded,
        };
        let empty = match (start, end) {
            (None, _) => true,
            (Some(s), Bound::Included(e)) => s > e,
            (Some(s), Bound::Excluded(e)) => s >= e,
            (Some(_), Bound::Unbounded) => false,
        };
        Entries {
            archive: self,
            stack: Vec::new(),
            visited: HashSet::new(),
            last: None,
            start: start.unwrap_or(Did::MAX),
            end,
            started: false,
            done: empty,
        }
    }
}

struct Frame {
    node: Arc<Node>,
    /// Absolute offset of the node, for error reports.
    offset: u32,
    idx: usize,
    descended: bool,
}

/// In-order iterator over the entries of an [`Archive`], created by [`Archive::entries`],
/// [`Archive::entries_from`] or [`Archive::entries_range`].
pub struct Entries<'a> {
    archive: &'a Archive,
    stack: Vec<Frame>,
    visited: HashSet<u32>,
    last: Option<Did>,
    /// Lowest DID to yield; the initial descent positions the stack on it.
    start: Did,
    /// Upper bound; iteration ends at the first entry past it.
    end: Bound<Did>,
    started: bool,
    done: bool,
}

impl std::fmt::Debug for Entries<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entries")
            .field("depth", &self.stack.len())
            .field("last", &self.last)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl Entries<'_> {
    fn push(&mut self, ptr: ChildPtr) -> Result<()> {
        if self.stack.len() > MAX_TREE_DEPTH {
            return Err(Error::TreeTooDeep {
                limit: MAX_TREE_DEPTH,
            });
        }
        if !self.visited.insert(ptr.offset) {
            return Err(Error::TreeCycle {
                offset: u64::from(ptr.offset),
            });
        }
        let node = self.archive.load_node(ptr)?;
        self.stack.push(Frame {
            node,
            offset: ptr.offset,
            idx: 0,
            descended: false,
        });
        Ok(())
    }

    /// Descends from the root to the first entry `≥ self.start`, leaving every frame on
    /// the path positioned so that the normal in-order step continues from there.
    fn seek(&mut self) -> Result<()> {
        self.push(self.archive.root())?;
        if self.start == 0 {
            return Ok(());
        }
        loop {
            let Some(top) = self.stack.last_mut() else {
                return Ok(());
            };
            let pos = top
                .node
                .entries
                .binary_search_by_key(&self.start, |e| e.did);
            let i = pos.unwrap_or_else(|i| i);
            top.idx = i;
            if top.node.is_leaf() || pos.is_ok() {
                // Leaf: continue at entry i. Interior hit: child i holds only smaller
                // DIDs, so skip it and emit entry i next.
                top.descended = !top.node.is_leaf();
                return Ok(());
            }
            top.descended = true;
            let child = top.node.children.get(i).copied().unwrap_or_default();
            self.push(child)?;
        }
    }

    /// Checks ordering and the upper bound; `Ok(None)` ends the iteration.
    fn emit(&mut self, e: Entry, node_offset: u32) -> Result<Option<Entry>> {
        if let Some(last) = self.last
            && e.did <= last
        {
            return Err(Error::corrupt(
                u64::from(node_offset),
                format!(
                    "DID {:#010x} (record at {:#x}) follows {last:#010x} (not ascending)",
                    e.did, e.offset
                ),
            ));
        }
        self.last = Some(e.did);
        let past_end = match self.end {
            Bound::Included(end) => e.did > end,
            Bound::Excluded(end) => e.did >= end,
            Bound::Unbounded => false,
        };
        Ok((!past_end).then_some(e))
    }

    fn step(&mut self) -> Result<Option<Entry>> {
        if !self.started {
            self.started = true;
            self.seek()?;
        }
        loop {
            let Some(top) = self.stack.last_mut() else {
                return Ok(None);
            };
            let n = top.node.entries.len();
            let offset = top.offset;
            if top.node.is_leaf() {
                if let Some(e) = top.node.entries.get(top.idx).copied() {
                    top.idx += 1;
                    return self.emit(e, offset);
                }
                self.stack.pop();
                continue;
            }
            if !top.descended && top.idx <= n {
                top.descended = true;
                let child = top.node.children.get(top.idx).copied().unwrap_or_default();
                self.push(child)?;
                continue;
            }
            if let Some(e) = top.node.entries.get(top.idx).copied() {
                top.idx += 1;
                top.descended = false;
                return self.emit(e, offset);
            }
            self.stack.pop();
        }
    }
}

impl Iterator for Entries<'_> {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.step() {
            Ok(Some(e)) => Some(Ok(e)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

impl std::iter::FusedIterator for Entries<'_> {}
