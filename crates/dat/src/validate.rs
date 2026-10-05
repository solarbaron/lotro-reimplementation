// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive validator: checks the invariants of spec §10 and collects statistics.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use crate::archive::Archive;
use crate::btree::ChildPtr;
use crate::bytes::u32_at;
use crate::entry::Entry;
use crate::error::Error;
use crate::extent::RecordLayout;
use crate::set::ArchiveId;
use crate::{
    CURRENT_ENGINE_VERSION, Did, HEADER_LEN, MAX_NODE_ENTRIES, MAX_TREE_DEPTH, METADATA_DID_BASE,
    MIN_NODE_ALLOC,
};

/// How bad a finding is (spec §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Readable but unexpected.
    Warning,
    /// The archive (or the affected record) is unusable.
    Error,
}

/// One validator finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Invariant number `n` of "I-n" in spec §10.
    pub invariant: u8,
    /// Severity.
    pub severity: Severity,
    /// Absolute file offset of the offending structure, if any.
    pub offset: Option<u64>,
    /// Human-readable description.
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        write!(f, "{sev} I-{}", self.invariant)?;
        if let Some(o) = self.offset {
            write!(f, " @{o:#x}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// What [`Archive::validate_with`] checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateOptions {
    /// Check that all allocations tile the data area exactly (I-20). Default `true`.
    pub check_tiling: bool,
    /// Fully inflate every compressed record (I-23). Expensive; default `false`.
    pub inflate_compressed: bool,
    /// Check the set-membership record against the file name (I-24). Default `true`.
    pub check_membership: bool,
    /// Keep at most this many issue messages per invariant; the rest are counted only.
    pub max_issues_per_invariant: usize,
    /// Most allocation ranges (nodes, record extents, fragments, free space) remembered
    /// for the tiling check, and most DIDs remembered per archive for the set-level
    /// duplicate check (I-25). Beyond it the check is abandoned with one issue, which keeps
    /// memory bounded on hostile input. Default 8,388,608 (≈ 200 MiB of ranges; the
    /// largest observed archive needs about 1.7 million).
    pub max_tracked: usize,
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            check_tiling: true,
            inflate_compressed: false,
            check_membership: true,
            max_issues_per_invariant: 20,
            max_tracked: 1 << 23,
        }
    }
}

/// Statistics gathered during validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationStats {
    /// Entries reached by the in-order walk.
    pub entries: u64,
    /// Of those, metadata entries (`did ≥ 0xFFFF0000`).
    pub metadata_entries: u64,
    /// Directory nodes reached.
    pub nodes: u64,
    /// Nodes per depth (index 0 = root).
    pub nodes_per_depth: Vec<u64>,
    /// Deepest node depth reached (root = 0).
    pub max_depth: usize,
    /// Records with flag bit 0 set.
    pub compressed: u64,
    /// Records with at least one continuation fragment.
    pub fragmented: u64,
    /// Largest fragment count seen.
    pub max_fragments: usize,
    /// Allocations using the legacy chained layout.
    pub legacy: u64,
    /// Unfragmented records whose allocation is larger than the sizing rule gives (spec §4.4).
    pub slack_records: u64,
    /// Largest stored record length.
    pub largest_stored: u32,
    /// Compressed records fully inflated (only with `inflate_compressed`).
    pub inflated: u64,
    /// Compressed records not inflated because their stored bytes overlap bytes already
    /// inflated for another record (only with `inflate_compressed`; see I-20).
    pub inflate_skipped: u64,
    /// Free extents listed in the free-space table.
    pub free_extents: u64,
    /// Σ (free extent size − 8).
    pub free_payload_bytes: u64,
    /// Number of gaps found by the tiling check.
    pub tiling_gaps: u64,
    /// Total bytes in gaps.
    pub tiling_gap_bytes: u64,
    /// Number of overlaps found by the tiling check.
    pub tiling_overlaps: u64,
}

/// Result of validating one archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// The archive's path, if it was opened from a file.
    pub path: Option<PathBuf>,
    /// Recorded issues (capped per invariant, see [`ValidateOptions`]).
    pub issues: Vec<Issue>,
    /// Number of `(warnings, errors)` per invariant number, including suppressed ones.
    pub counts: BTreeMap<u8, (u64, u64)>,
    /// Issues counted but not recorded because of the per-invariant cap.
    pub suppressed: u64,
    /// Statistics.
    pub stats: ValidationStats,
}

impl ValidationReport {
    /// Total number of errors (including suppressed ones).
    pub fn errors(&self) -> u64 {
        self.counts.values().map(|c| c.1).sum()
    }

    /// Total number of warnings (including suppressed ones).
    pub fn warnings(&self) -> u64 {
        self.counts.values().map(|c| c.0).sum()
    }

    /// Whether no error was found (warnings are allowed).
    pub fn is_ok(&self) -> bool {
        self.errors() == 0
    }

    /// Whether any issue (warning or error) was found for invariant `I-n`.
    pub fn has(&self, invariant: u8) -> bool {
        self.counts.get(&invariant).is_some_and(|c| c.0 + c.1 > 0)
    }

    /// Counts one issue and records it while under `cap` per invariant. The message is
    /// built only when it is recorded, so suppressed issues cost a counter update.
    pub(crate) fn push(
        &mut self,
        cap: usize,
        invariant: u8,
        severity: Severity,
        offset: Option<u64>,
        message: impl FnOnce() -> String,
    ) {
        let c = self.counts.entry(invariant).or_default();
        match severity {
            Severity::Warning => c.0 += 1,
            Severity::Error => c.1 += 1,
        }
        if c.0 + c.1 > cap as u64 {
            self.suppressed += 1;
            return;
        }
        self.issues.push(Issue {
            invariant,
            severity,
            offset,
            message: message(),
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RangeKind {
    Node,
    Record,
    FreeTable,
    Free,
}

struct Walk<'a> {
    a: &'a Archive,
    opts: &'a ValidateOptions,
    report: ValidationReport,
    visited: HashSet<u32>,
    /// First-extent offsets of records already accounted for.
    records_seen: HashSet<u32>,
    ranges: Vec<(u64, u64, RangeKind)>,
    /// Set once `ranges` reached [`ValidateOptions::max_tracked`]; tiling is then skipped.
    ranges_overflow: bool,
    /// Set once `dids` reached [`ValidateOptions::max_tracked`].
    dids_overflow: bool,
    /// Disjoint byte ranges `start → end` of records already inflated (I-23).
    inflated: BTreeMap<u64, u64>,
    /// Set once `inflated` would exceed [`ValidateOptions::max_tracked`]; no further
    /// records are inflated.
    inflate_overflow: bool,
    last_did: Option<Did>,
    leaf_depth: Option<usize>,
    dids: Option<&'a mut Vec<Did>>,
}

impl Walk<'_> {
    fn issue(&mut self, inv: u8, sev: Severity, offset: Option<u64>, msg: String) {
        self.issue_with(inv, sev, offset, || msg);
    }

    /// Like [`Walk::issue`], but formats the message only if it is kept. Used on the
    /// per-entry paths, which hostile input can hit millions of times.
    fn issue_with(
        &mut self,
        inv: u8,
        sev: Severity,
        offset: Option<u64>,
        msg: impl FnOnce() -> String,
    ) {
        let cap = self.opts.max_issues_per_invariant;
        self.report.push(cap, inv, sev, offset, msg);
    }

    /// Remembers one allocation range for the tiling check, up to the configured cap.
    fn range(&mut self, offset: u64, len: u64, kind: RangeKind) {
        if self.ranges_overflow || !self.opts.check_tiling {
            return;
        }
        if self.ranges.len() >= self.opts.max_tracked {
            self.ranges_overflow = true;
            self.ranges = Vec::new();
            return;
        }
        self.ranges.push((offset, len, kind));
    }

    fn header(&mut self) {
        let h = self.a.header().clone();
        let len = self.a.file_len();
        if u64::from(h.file_size) != len {
            self.issue(
                3,
                Severity::Warning,
                Some(0x148),
                format!("header file size {:#x} ≠ real length {len:#x}", h.file_size),
            );
        }
        if h.engine_version != CURRENT_ENGINE_VERSION {
            self.issue(
                5,
                Severity::Warning,
                Some(0x174),
                format!("engine version {} (expected 112)", h.engine_version),
            );
        }
        if !h.layout_sizes_are_standard() {
            self.issue(
                6,
                Severity::Warning,
                Some(0x190),
                format!(
                    "layout sizes {}/{}/{} (expected 104/32/8)",
                    h.superblock_len, h.entry_size, h.pointer_size
                ),
            );
        }
        if !h.journal_is_empty() {
            self.issue(
                7,
                Severity::Warning,
                Some(0x100),
                "journal area is not the empty pattern".into(),
            );
        }
    }

    fn node(&mut self, ptr: ChildPtr, depth: usize, is_root: bool) {
        let at = u64::from(ptr.offset);
        if depth > MAX_TREE_DEPTH {
            self.issue(
                13,
                Severity::Error,
                Some(at),
                format!("tree deeper than {MAX_TREE_DEPTH}"),
            );
            return;
        }
        if !self.visited.insert(ptr.offset) {
            self.issue(13, Severity::Error, Some(at), "node reached twice".into());
            return;
        }
        let mut hdr = [0u8; 8];
        if self.a.check_range("node", at, 8).is_ok() && self.a.read_at(at, &mut hdr).is_ok() {
            let words = [u32_at(&hdr, 0), u32_at(&hdr, 4)];
            if words != [0, 0] {
                self.issue(
                    9,
                    Severity::Error,
                    Some(at),
                    format!("node allocation header {words:?} ≠ {{0, 0}}"),
                );
            }
        }
        let raw = match self.a.read_raw_node(ptr) {
            Ok(r) => r,
            Err(e) => {
                let inv = if is_root { 8 } else { 12 };
                self.issue(
                    inv,
                    Severity::Error,
                    Some(at),
                    format!("unreadable node: {e}"),
                );
                return;
            }
        };
        let s = &mut self.report.stats;
        s.nodes += 1;
        if s.nodes_per_depth.len() <= depth {
            s.nodes_per_depth.resize(depth + 1, 0);
        }
        s.nodes_per_depth[depth] += 1;
        s.max_depth = s.max_depth.max(depth);
        for &(o, l) in &raw.extents {
            self.range(o, l, RangeKind::Node);
        }
        let n = raw.count as usize;
        if n > MAX_NODE_ENTRIES {
            self.issue(
                10,
                Severity::Error,
                Some(at),
                format!("entry count {n} > 61"),
            );
            return;
        }
        let leaf = raw.children.first().is_none_or(ChildPtr::is_null);
        if n == 0 {
            // Only an empty leaf root (an archive without records) is readable; the reader
            // rejects every other empty node, interior or leaf (spec §5.4, §14 note on the
            // empty root).
            let (sev, what) = match (is_root, leaf) {
                (true, true) => (
                    Severity::Warning,
                    "empty leaf root (archive has no records)",
                ),
                (_, false) => (Severity::Error, "interior node has no entries"),
                (false, true) => (Severity::Error, "node has no entries"),
            };
            self.issue(10, sev, Some(at), what.into());
        } else if !is_root && n < 30 {
            self.issue(
                10,
                Severity::Warning,
                Some(at),
                format!("non-root node has only {n} entries"),
            );
        }
        let used: Vec<ChildPtr> = raw.children.get(..=n).unwrap_or(&[]).to_vec();
        let mut child_ok = vec![!leaf; used.len()];
        if leaf {
            if used.iter().any(|c| !c.is_null()) {
                self.issue(
                    11,
                    Severity::Error,
                    Some(at),
                    "leaf has non-zero child pointers 1..n".into(),
                );
            }
            match self.leaf_depth {
                None => self.leaf_depth = Some(depth),
                Some(d) if d != depth => self.issue(
                    14,
                    Severity::Warning,
                    Some(at),
                    format!("leaf at depth {depth}, earlier leaves at depth {d}"),
                ),
                Some(_) => {}
            }
        } else {
            for (i, c) in used.iter().enumerate() {
                if c.size == 0 || c.offset == 0 {
                    self.issue(
                        11,
                        Severity::Error,
                        Some(at),
                        format!("interior node child {i} is zero"),
                    );
                    child_ok[i] = false;
                    continue;
                }
                let end = u64::from(c.offset) + u64::from(c.size);
                if c.size < MIN_NODE_ALLOC
                    || u64::from(c.offset) < HEADER_LEN
                    || end > self.a.file_len()
                {
                    self.issue(
                        12,
                        Severity::Error,
                        Some(at),
                        format!("child {i} {{{:#x}, {:#x}}} out of range", c.size, c.offset),
                    );
                    child_ok[i] = false;
                }
            }
        }
        for i in 0..=n {
            if child_ok.get(i).copied().unwrap_or(false)
                && let Some(c) = used.get(i)
            {
                self.node(*c, depth + 1, false);
            }
            if let Some(e) = raw.entries.get(i) {
                self.entry(e);
            }
        }
    }

    fn entry(&mut self, e: &Entry) {
        let at = Some(u64::from(e.offset));
        let did = e.did;
        {
            let s = &mut self.report.stats;
            s.entries += 1;
            if e.is_metadata() {
                s.metadata_entries += 1;
            }
            if e.is_compressed() {
                s.compressed += 1;
            }
            s.largest_stored = s.largest_stored.max(e.length);
        }
        let cap = self.opts.max_tracked;
        if let Some(d) = self.dids.as_deref_mut()
            && e.did < METADATA_DID_BASE
        {
            if d.len() < cap {
                d.push(e.did);
            } else {
                self.dids_overflow = true;
            }
        }
        if let Some(last) = self.last_did
            && e.did <= last
        {
            self.issue_with(15, Severity::Error, at, || {
                format!("DID {did:#010x} after {last:#010x}")
            });
        }
        self.last_did = Some(e.did);
        if e.reserved != 0 {
            let reserved = e.reserved;
            self.issue_with(16, Severity::Warning, at, || {
                format!("DID {did:#010x}: reserved field {reserved:#x}")
            });
        }
        let flags = e.flags;
        if flags & !3 != 0 {
            self.issue_with(17, Severity::Error, at, || {
                format!("DID {did:#010x}: unknown flag bits {flags:#06x}")
            });
        } else if flags != 2 && flags != 3 {
            self.issue_with(17, Severity::Warning, at, || {
                format!("DID {did:#010x}: flags {flags}")
            });
        }
        let len = self.a.file_len();
        let start = u64::from(e.offset);
        if start < HEADER_LEN || start + u64::from(e.alloc_size) > len {
            let (offset, alloc) = (e.offset, e.alloc_size);
            self.issue_with(18, Severity::Error, at, || {
                format!("DID {did:#010x}: first extent {offset:#x}+{alloc:#x} out of range")
            });
            return;
        }
        let fresh = if self.records_seen.len() < cap {
            self.records_seen.insert(e.offset)
        } else {
            !self.records_seen.contains(&e.offset)
        };
        if !fresh {
            // Another entry already names this allocation: report it once and do not parse
            // the (possibly large) fragment table again. Re-walking it for every entry would
            // make validation cost entries × fragments on hostile input.
            self.report.stats.tiling_overlaps += 1;
            let offset = e.offset;
            self.issue_with(20, Severity::Error, at, || {
                format!("DID {did:#010x}: first extent {offset:#x} is shared with another record")
            });
            return;
        }
        let (layout, hdr) = match self.a.layout_and_header(e.offset, e.alloc_size) {
            Ok(x) => x,
            Err(err) => {
                self.issue_with(18, Severity::Error, at, || {
                    let what = if matches!(err, Error::UnsupportedLegacyLayout { .. }) {
                        "legacy chained layout (disabled)".to_string()
                    } else {
                        err.to_string()
                    };
                    format!("DID {did:#010x}: {what}")
                });
                // Account for the first extent so tiling does not report a spurious gap.
                self.range(start, u64::from(e.alloc_size), RangeKind::Record);
                return;
            }
        };
        match &layout {
            RecordLayout::Legacy { .. } => {
                self.report.stats.legacy += 1;
                self.issue_with(18, Severity::Warning, at, || {
                    format!("DID {did:#010x}: legacy chained layout {hdr:?}")
                });
            }
            RecordLayout::Modern { fragments, .. } => {
                let f = fragments.len();
                if f == 0 {
                    if u64::from(e.alloc_size) < u64::from(e.length) + 8 {
                        let (alloc, length) = (e.alloc_size, e.length);
                        self.issue_with(18, Severity::Error, at, || {
                            format!(
                                "DID {did:#010x}: allocation {alloc} too small for length {length}"
                            )
                        });
                    } else {
                        let rule = u64::from(self.a.header().min_alloc)
                            .max((u64::from(e.length) + 8).div_ceil(4) * 4);
                        if u64::from(e.alloc_size) > rule {
                            self.report.stats.slack_records += 1;
                        }
                    }
                } else {
                    let s = &mut self.report.stats;
                    s.fragmented += 1;
                    s.max_fragments = s.max_fragments.max(f);
                    // One I-19 issue per record, however many fragments are out of range:
                    // a record may list thousands of them.
                    let mut bad = 0usize;
                    let mut first_bad = None;
                    for (i, fr) in fragments.iter().enumerate() {
                        let o = u64::from(fr.offset);
                        if o < HEADER_LEN || o + u64::from(fr.length) > len {
                            bad += 1;
                            first_bad.get_or_insert(i);
                        }
                    }
                    if let Some(i) = first_bad {
                        self.issue_with(19, Severity::Error, at, || {
                            format!(
                                "DID {did:#010x}: {bad} of {f} fragments out of range (first: \
                                 fragment {i})"
                            )
                        });
                    }
                }
            }
        }
        if layout.capacity() < u64::from(e.length) {
            let (capacity, length) = (layout.capacity(), e.length);
            self.issue_with(19, Severity::Error, at, || {
                format!("DID {did:#010x}: extents hold {capacity} bytes, length is {length}")
            });
        }
        let extents = layout.extents();
        for &(o, l) in &extents {
            if o >= HEADER_LEN && o + l <= len {
                self.range(o, l, RangeKind::Record);
            }
        }
        if self.opts.inflate_compressed && e.is_compressed() {
            self.inflate(e, &extents);
        }
    }

    /// I-23: inflates one compressed record, unless its stored bytes overlap bytes that
    /// were already inflated for another record (or for itself).
    ///
    /// Without this check a hostile archive could point many small entries at one large
    /// stream and make the validator inflate it once per entry. Overlap is an I-20 error
    /// in its own right (spec §4.6); the skipped record gets an I-23 warning. Inflation
    /// work is so bounded by the compression ratio times the file length.
    fn inflate(&mut self, e: &Entry, extents: &[(u64, u64)]) {
        let at = Some(u64::from(e.offset));
        let did = e.did;
        if self.inflate_overflow {
            return;
        }
        let mut own: Vec<(u64, u64)> = extents
            .iter()
            .filter(|&&(_, l)| l > 0)
            .map(|&(o, l)| (o, o.saturating_add(l)))
            .collect();
        own.sort_unstable();
        let self_overlap = own.windows(2).any(|w| match w {
            [a, b] => b.0 < a.1,
            _ => false,
        });
        let shared = self_overlap
            || own.iter().any(|&(s, end)| {
                self.inflated
                    .range(..end)
                    .next_back()
                    .is_some_and(|(_, &prev_end)| prev_end > s)
            });
        if shared {
            self.report.stats.inflate_skipped += 1;
            self.issue_with(23, Severity::Warning, at, || {
                format!(
                    "DID {did:#010x}: not inflated, its stored bytes overlap bytes already \
                     inflated (see I-20)"
                )
            });
            return;
        }
        if self.inflated.len().saturating_add(own.len()) > self.opts.max_tracked {
            self.inflate_overflow = true;
            let max = self.opts.max_tracked;
            self.issue_with(23, Severity::Warning, None, || {
                format!("inflate check stopped: more than {max} inflated extents tracked")
            });
            return;
        }
        for (s, end) in own {
            self.inflated.insert(s, end);
        }
        match self.a.read_entry(e) {
            Ok(_) => self.report.stats.inflated += 1,
            Err(err) => self.issue_with(23, Severity::Error, at, || err.to_string()),
        }
    }

    fn free_space(&mut self) {
        let h = self.a.header().clone();
        let table = match self.a.free_table() {
            Ok(t) => t,
            Err(e) => {
                self.issue(
                    21,
                    Severity::Warning,
                    Some(u64::from(h.free_table_offset)),
                    format!("free-space table unreadable: {e}"),
                );
                return;
            }
        };
        if h.free_table_size > 0 {
            self.range(
                u64::from(h.free_table_offset),
                u64::from(h.free_table_size),
                RangeKind::FreeTable,
            );
        }
        let at = Some(u64::from(h.free_table_offset));
        if table.header != [0, 0] {
            self.issue(
                21,
                Severity::Warning,
                at,
                format!("free table header {:?}", table.header),
            );
        }
        let len = self.a.file_len();
        let mut prev: Option<u32> = None;
        let mut total: u64 = 0;
        for (i, x) in table.extents.iter().enumerate() {
            let o = u64::from(x.offset);
            let in_bounds = o >= HEADER_LEN && o + u64::from(x.size) <= len;
            if !in_bounds {
                self.issue(
                    21,
                    Severity::Warning,
                    at,
                    format!(
                        "free extent {i} {{{:#x}, {:#x}}} out of range",
                        x.size, x.offset
                    ),
                );
            } else {
                self.range(o, u64::from(x.size), RangeKind::Free);
            }
            if prev.is_some_and(|p| x.offset <= p) {
                self.issue(
                    21,
                    Severity::Warning,
                    at,
                    format!("free extent {i} not sorted by offset"),
                );
            }
            if x.size < h.min_alloc {
                self.issue(
                    21,
                    Severity::Warning,
                    at,
                    format!("free extent {i} smaller than the minimum allocation"),
                );
            }
            prev = Some(x.offset);
            total += u64::from(x.size).saturating_sub(8);
        }
        let s = &mut self.report.stats;
        s.free_extents = table.extents.len() as u64;
        s.free_payload_bytes = total;
        if total != u64::from(h.free_bytes) {
            self.issue(
                22,
                Severity::Warning,
                Some(0x19C),
                format!("free payload sum {total} ≠ header {}", h.free_bytes),
            );
        }
    }

    fn tiling(&mut self) {
        if self.ranges_overflow {
            self.issue(
                20,
                Severity::Warning,
                None,
                format!(
                    "tiling check skipped: more than {} allocation ranges",
                    self.opts.max_tracked
                ),
            );
            return;
        }
        let mut ranges = std::mem::take(&mut self.ranges);
        ranges.sort_unstable();
        let len = self.a.file_len();
        let mut cursor = HEADER_LEN;
        for (o, l, kind) in ranges {
            if o > cursor {
                self.report.stats.tiling_gaps += 1;
                self.report.stats.tiling_gap_bytes += o - cursor;
                self.issue(
                    20,
                    Severity::Warning,
                    Some(cursor),
                    format!("gap of {} bytes before {kind:?} at {o:#x}", o - cursor),
                );
            } else if o < cursor {
                self.report.stats.tiling_overlaps += 1;
                self.issue(
                    20,
                    Severity::Error,
                    Some(o),
                    format!("{kind:?} at {o:#x}+{l:#x} overlaps data ending at {cursor:#x}"),
                );
            }
            cursor = cursor.max(o + l);
        }
        if cursor < len {
            self.report.stats.tiling_gaps += 1;
            self.report.stats.tiling_gap_bytes += len - cursor;
            self.issue(
                20,
                Severity::Warning,
                Some(cursor),
                format!("{} unaccounted bytes at end of file", len - cursor),
            );
        }
    }

    fn membership(&mut self) {
        let Some(path) = self.a.path() else { return };
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(ArchiveId::from_file_name)
        else {
            return;
        };
        match self.a.membership() {
            Ok(Some(m)) => {
                if m.part_index != id.part || m.part_index >= m.part_count {
                    self.issue(
                        24,
                        Severity::Warning,
                        None,
                        format!(
                            "membership says part {} of {}, file name says part {}",
                            m.part_index, m.part_count, id.part
                        ),
                    );
                }
            }
            Ok(None) => self.issue(
                24,
                Severity::Warning,
                None,
                "set-membership record 0xFFFF0005 is missing".into(),
            ),
            Err(e) => self.issue(
                24,
                Severity::Warning,
                None,
                format!("set-membership record unreadable: {e}"),
            ),
        }
    }
}

impl Archive {
    /// Validates the archive with [`ValidateOptions::default`].
    pub fn validate(&self) -> ValidationReport {
        self.validate_with(&ValidateOptions::default())
    }

    /// Walks the whole archive and checks the invariants of spec §10.
    ///
    /// Never fails: problems become [`Issue`]s in the report. I-1, I-2, I-4, I-6 (entry
    /// size) and I-8 are already enforced by [`Archive::open`]; I-25 needs the whole set
    /// and is checked by [`crate::ArchiveSet::validate`].
    pub fn validate_with(&self, opts: &ValidateOptions) -> ValidationReport {
        self.validate_collect(opts, None)
    }

    pub(crate) fn validate_collect(
        &self,
        opts: &ValidateOptions,
        dids: Option<&mut Vec<Did>>,
    ) -> ValidationReport {
        let mut w = Walk {
            a: self,
            opts,
            report: ValidationReport {
                path: self.path().map(Into::into),
                ..ValidationReport::default()
            },
            visited: HashSet::new(),
            records_seen: HashSet::new(),
            ranges: Vec::new(),
            ranges_overflow: false,
            dids_overflow: false,
            inflated: BTreeMap::new(),
            inflate_overflow: false,
            last_did: None,
            leaf_depth: None,
            dids,
        };
        w.header();
        w.node(self.root(), 0, true);
        w.free_space();
        if opts.check_tiling {
            w.tiling();
        }
        if opts.check_membership {
            w.membership();
        }
        if w.dids_overflow {
            w.issue(
                25,
                Severity::Warning,
                None,
                format!(
                    "duplicate check incomplete: only the first {} DIDs were compared",
                    opts.max_tracked
                ),
            );
        }
        w.report
    }
}
