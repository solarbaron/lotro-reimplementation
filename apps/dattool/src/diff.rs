// SPDX-License-Identifier: GPL-3.0-or-later
//! `diff`: compare the directory indexes of two installs, archives or snapshots.
//!
//! Archives are paired by label (`general`, `surface_aux_1`, ...); when both inputs are
//! single archives they are paired whatever their names. Within a pair, a DID is *added*
//! (only in B), *removed* (only in A) or *changed* (in both, with a different iteration,
//! stored size, or — when both sides carry content hashes — content hash). Write time,
//! stamp and flags are kept in the index but do not count as a change. A DID removed from
//! one archive and added to another is also reported as *moved* (per-archive housekeeping
//! records, DIDs `0xFFFFxxxx`, never are: every archive has its own).
//!
//! When a directory walk stopped early, that side's list is reliable only up to the last
//! DID it reached (walks run in ascending DID order). DIDs beyond it are counted as *not
//! compared* instead of added or removed, the walk error is printed, and `diff` exits with
//! status 2.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tessera_dat::METADATA_DID_BASE;

use crate::Out;
use crate::info::kind_name;
use crate::input::{Ctx, Input};
use crate::snapshot::{self, ArchiveIndex, HashPlan, Index, Rec, SourceKind};
use crate::util::{Align, Table, did_hex, format_time, hex_bytes};

/// Exit status of `diff --exit-code` when differences were found.
pub const EXIT_DIFFERENT: u8 = 1;
/// Exit status when the comparison is incomplete (a directory walk stopped early). Errors
/// exit with the same status.
pub const EXIT_TROUBLE: u8 = 2;

/// Options of `diff`.
#[derive(Debug, Clone)]
pub struct Opts {
    /// Older side: install directory, archive or snapshot.
    pub a: String,
    /// Newer side.
    pub b: String,
    /// Hash the payloads of live inputs and compare hashes.
    pub content: bool,
    /// Only DIDs with this high byte.
    pub prefix: Option<u8>,
    /// DIDs listed per archive and category in text output.
    pub max_list: usize,
    /// Omit DID lists (text and JSON).
    pub summary: bool,
    /// Exit with status 1 when differences were found.
    pub exit_code: bool,
}

/// Where one side of a diff came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Read live from archives.
    Live,
    /// Loaded from a snapshot file.
    Snapshot,
}

/// One side of a diff.
#[derive(Debug)]
pub struct Side {
    /// Its index.
    pub index: Index,
    /// Live or snapshot.
    pub origin: Origin,
}

/// A side before indexing: a loaded snapshot, or an opened input not yet walked.
#[derive(Debug)]
enum Opened {
    Snapshot(Index),
    Live { input: Input, source: String },
}

impl Opened {
    fn open(arg: &str, ctx: &Ctx) -> Result<Self> {
        if let Some(index) = snapshot::load_if_snapshot(Path::new(arg))? {
            return Ok(Self::Snapshot(index));
        }
        let input = Input::open(arg, ctx)?;
        let source = match &input {
            Input::Archive { archive, .. } => archive
                .path()
                .map_or_else(|| arg.to_owned(), |p| p.display().to_string()),
            Input::Set(set) => set.dir().display().to_string(),
        };
        Ok(Self::Live { input, source })
    }

    /// The label of a single-archive side.
    fn single_label(&self) -> Option<String> {
        match self {
            Self::Snapshot(i) if i.source_kind == SourceKind::Archive => {
                i.archives.first().map(|a| a.label.clone())
            }
            Self::Live {
                input: Input::Archive { label, .. },
                ..
            } => Some(label.clone()),
            _ => None,
        }
    }

    /// What to hash on this side, given the other side: nothing without `--content` or
    /// when the other side is a snapshot without hashes; only the paired archive when the
    /// other side is a single archive; only DIDs under the prefix filter.
    fn hash_plan(&self, other: &Self, arg: &str, o: &Opts) -> HashPlan {
        let Self::Live { input, .. } = self else {
            return HashPlan::default();
        };
        if !o.content {
            return HashPlan::default();
        }
        if let Self::Snapshot(i) = other
            && !i.hashed
        {
            tracing::warn!(
                "the other side is a snapshot without content hashes; not hashing {arg}"
            );
            return HashPlan::default();
        }
        let archives = match (input, other.single_label()) {
            (Input::Set(_), Some(label)) => Some(vec![label]),
            _ => None,
        };
        HashPlan {
            enabled: true,
            archives,
            prefix: o.prefix,
        }
    }

    fn into_side(self, plan: &HashPlan) -> Side {
        match self {
            Self::Snapshot(index) => Side {
                index,
                origin: Origin::Snapshot,
            },
            Self::Live { input, source } => Side {
                index: snapshot::index_input(&input, &source, plan),
                origin: Origin::Live,
            },
        }
    }
}

/// Loads both sides. Live inputs are opened and paired before anything is hashed, so
/// `--content` reads only the payloads that can actually be compared.
pub fn load_sides(o: &Opts, ctx: &Ctx) -> Result<(Side, Side)> {
    let a = Opened::open(&o.a, ctx)?;
    let b = Opened::open(&o.b, ctx)?;
    for (arg, side) in [(&o.a, &a), (&o.b, &b)] {
        if let Opened::Snapshot(i) = side
            && o.content
            && !i.hashed
        {
            tracing::warn!(
                snapshot = arg.as_str(),
                "snapshot has no content hashes; content is not compared against it"
            );
        }
    }
    let plan_a = a.hash_plan(&b, &o.a, o);
    let plan_b = b.hash_plan(&a, &o.b, o);
    Ok((a.into_side(&plan_a), b.into_side(&plan_b)))
}

/// Which fields made a record count as changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reasons {
    /// Iteration differs.
    pub iteration: bool,
    /// Stored size differs.
    pub size: bool,
    /// Content hash differs.
    pub content: bool,
}

impl Reasons {
    fn any(self) -> bool {
        self.iteration || self.size || self.content
    }
    fn names(self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.iteration {
            v.push("iteration");
        }
        if self.size {
            v.push("size");
        }
        if self.content {
            v.push("content");
        }
        v
    }
}

/// A DID present on both sides with differing metadata or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change<'a> {
    /// The A record.
    pub a: &'a Rec,
    /// The B record.
    pub b: &'a Rec,
    /// What differs.
    pub reasons: Reasons,
}

/// Comparison of one archive pair. Records are borrowed from the two indexes.
#[derive(Debug, Clone, Default)]
pub struct ArchiveDiff<'a> {
    /// Label (B's when the names differ).
    pub label: String,
    /// A's label when it differs from B's.
    pub a_label: Option<String>,
    /// Present in A.
    pub in_a: bool,
    /// Present in B.
    pub in_b: bool,
    /// Records considered in A (after the prefix filter).
    pub a_records: u64,
    /// Records considered in B.
    pub b_records: u64,
    /// A's directory walk was complete.
    pub a_complete: bool,
    /// B's walk was complete.
    pub b_complete: bool,
    /// Why A's walk stopped, when known (live inputs).
    pub a_error: Option<&'a str>,
    /// Why B's walk stopped, when known.
    pub b_error: Option<&'a str>,
    /// Only in B.
    pub added: Vec<&'a Rec>,
    /// Only in A.
    pub removed: Vec<&'a Rec>,
    /// In both, different.
    pub changed: Vec<Change<'a>>,
    /// In both, equal.
    pub unchanged: u64,
    /// Pairs whose hashes were compared.
    pub content_compared: u64,
    /// DIDs present on one side beyond where the other side's walk stopped.
    pub not_compared: u64,
}

impl ArchiveDiff<'_> {
    /// Either side's walk stopped early.
    pub fn incomplete(&self) -> bool {
        !self.a_complete || !self.b_complete
    }
}

/// A DID that left one archive and appeared in another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    /// The DID.
    pub did: u32,
    /// Archive in A.
    pub from: String,
    /// Archive in B.
    pub to: String,
}

/// Per-DID-type tallies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypeCounts {
    /// Added DIDs.
    pub added: u64,
    /// Removed DIDs.
    pub removed: u64,
    /// Changed DIDs.
    pub changed: u64,
}

/// A complete diff.
#[derive(Debug, Clone, Default)]
pub struct Report<'a> {
    /// Archive pairs in output order.
    pub archives: Vec<ArchiveDiff<'a>>,
    /// DIDs that moved between archives.
    pub moved: Vec<Move>,
    /// Tallies by DID high byte.
    pub by_type: BTreeMap<u8, TypeCounts>,
    /// Whether content hashes were compared.
    pub content: bool,
}

impl Report<'_> {
    /// Sum of added DIDs.
    pub fn added(&self) -> u64 {
        self.archives.iter().map(|a| a.added.len() as u64).sum()
    }
    /// Sum of removed DIDs.
    pub fn removed(&self) -> u64 {
        self.archives.iter().map(|a| a.removed.len() as u64).sum()
    }
    /// Sum of changed DIDs.
    pub fn changed(&self) -> u64 {
        self.archives.iter().map(|a| a.changed.len() as u64).sum()
    }
    /// Sum of unchanged DIDs.
    pub fn unchanged(&self) -> u64 {
        self.archives.iter().map(|a| a.unchanged).sum()
    }
    /// Sum of DIDs that could not be compared.
    pub fn not_compared(&self) -> u64 {
        self.archives.iter().map(|a| a.not_compared).sum()
    }
    /// No added, removed or changed DID among those compared.
    pub fn identical(&self) -> bool {
        self.added() == 0 && self.removed() == 0 && self.changed() == 0
    }
    /// Some archive's directory walk stopped early, so the comparison is partial.
    pub fn incomplete(&self) -> bool {
        self.archives.iter().any(ArchiveDiff::incomplete)
    }
}

fn keep(prefix: Option<u8>, did: u32) -> bool {
    prefix.is_none_or(|p| (did >> 24) as u8 == p)
}

/// Whether side `x` is known to hold every record it has at or below `did`: an absent
/// archive or a complete walk covers everything, an incomplete walk only up to its last DID.
fn covers(x: Option<&ArchiveIndex>, did: u32) -> bool {
    x.is_none_or(|x| x.complete || x.recs.last().is_some_and(|l| did <= l.did))
}

/// Merge-joins two DID-sorted archives.
pub fn diff_archive<'a>(
    a: Option<&'a ArchiveIndex>,
    b: Option<&'a ArchiveIndex>,
    compare_content: bool,
    prefix: Option<u8>,
) -> ArchiveDiff<'a> {
    let empty: &[Rec] = &[];
    let ar = a.map_or(empty, |x| &x.recs);
    let br = b.map_or(empty, |x| &x.recs);
    let label = b.or(a).map(|x| x.label.clone()).unwrap_or_default();
    let a_label = match (a, b) {
        (Some(a), Some(b)) if a.label != b.label => Some(a.label.clone()),
        _ => None,
    };
    let mut d = ArchiveDiff {
        label,
        a_label,
        in_a: a.is_some(),
        in_b: b.is_some(),
        a_complete: a.is_none_or(|x| x.complete),
        b_complete: b.is_none_or(|x| x.complete),
        a_error: a.and_then(|x| x.walk_error.as_deref()),
        b_error: b.and_then(|x| x.walk_error.as_deref()),
        ..ArchiveDiff::default()
    };
    let (mut i, mut j) = (0, 0);
    loop {
        match (ar.get(i), br.get(j)) {
            (Some(x), Some(y)) if x.did == y.did => {
                i += 1;
                j += 1;
                if !keep(prefix, x.did) {
                    continue;
                }
                d.a_records += 1;
                d.b_records += 1;
                let mut r = Reasons {
                    iteration: x.iteration != y.iteration,
                    size: x.size != y.size,
                    content: false,
                };
                if compare_content && let (Some(hx), Some(hy)) = (&x.hash, &y.hash) {
                    d.content_compared += 1;
                    r.content = hx != hy;
                }
                if r.any() {
                    d.changed.push(Change {
                        a: x,
                        b: y,
                        reasons: r,
                    });
                } else {
                    d.unchanged += 1;
                }
            }
            (Some(x), y) if y.is_none_or(|y| x.did < y.did) => {
                i += 1;
                if keep(prefix, x.did) {
                    d.a_records += 1;
                    if covers(b, x.did) {
                        d.removed.push(x);
                    } else {
                        d.not_compared += 1;
                    }
                }
            }
            (_, Some(y)) => {
                j += 1;
                if keep(prefix, y.did) {
                    d.b_records += 1;
                    if covers(a, y.did) {
                        d.added.push(y);
                    } else {
                        d.not_compared += 1;
                    }
                }
            }
            _ => break,
        }
    }
    d
}

/// Pairs the archives of two indexes and compares them. Content hashes are compared
/// whenever both indexes carry them.
pub fn diff_indexes<'a>(a: &'a Index, b: &'a Index, prefix: Option<u8>) -> Result<Report<'a>> {
    let compare_content = a.hashed && b.hashed;
    let mut pairs: Vec<(Option<&ArchiveIndex>, Option<&ArchiveIndex>)> = Vec::new();
    let a_single = a.source_kind == SourceKind::Archive;
    let b_single = b.source_kind == SourceKind::Archive;
    let find = |idx: &'_ Index, label: &str| -> Option<usize> {
        idx.archives
            .iter()
            .position(|x| x.label == label)
            .or_else(|| {
                idx.archives
                    .iter()
                    .position(|x| x.label.eq_ignore_ascii_case(label))
            })
    };
    if a_single && b_single {
        pairs.push((a.archives.first(), b.archives.first()));
    } else if a_single || b_single {
        // One side is a single archive: compare it with the same-named archive of the other.
        let (single, set, single_is_a) = if a_single {
            (a, b, true)
        } else {
            (b, a, false)
        };
        let Some(s) = single.archives.first() else {
            bail!("{} holds no archive", single.source);
        };
        let Some(k) = find(set, &s.label) else {
            let known: Vec<&str> = set.archives.iter().map(|x| x.label.as_str()).collect();
            bail!(
                "{} has no archive named {} (it has: {})",
                set.source,
                s.label,
                known.join(", ")
            );
        };
        let other = &set.archives[k];
        pairs.push(if single_is_a {
            (Some(s), Some(other))
        } else {
            (Some(other), Some(s))
        });
    } else {
        let mut used = vec![false; b.archives.len()];
        for x in &a.archives {
            match find(b, &x.label) {
                Some(k) if !used[k] => {
                    used[k] = true;
                    pairs.push((Some(x), Some(&b.archives[k])));
                }
                _ => pairs.push((Some(x), None)),
            }
        }
        for (k, y) in b.archives.iter().enumerate() {
            if !used[k] {
                pairs.push((None, Some(y)));
            }
        }
    }

    let mut report = Report {
        content: compare_content,
        ..Report::default()
    };
    for (x, y) in pairs {
        report
            .archives
            .push(diff_archive(x, y, compare_content, prefix));
    }

    report.moved = find_moves(&report.archives);

    let mut by_type: BTreeMap<u8, TypeCounts> = BTreeMap::new();
    for d in &report.archives {
        for r in &d.added {
            by_type.entry((r.did >> 24) as u8).or_default().added += 1;
        }
        for r in &d.removed {
            by_type.entry((r.did >> 24) as u8).or_default().removed += 1;
        }
        for c in &d.changed {
            by_type.entry((c.b.did >> 24) as u8).or_default().changed += 1;
        }
    }
    report.by_type = by_type;
    Ok(report)
}

/// DIDs removed from one archive and added to another. Housekeeping DIDs exist once per
/// archive and never move. Only the smaller of the two sides (usually the added DIDs) is
/// indexed, as a sorted list, so a diff against an empty install stays cheap.
fn find_moves(archives: &[ArchiveDiff<'_>]) -> Vec<Move> {
    let side = |added: bool| -> Vec<(u32, &str)> {
        let mut v = Vec::new();
        for d in archives {
            let label = if added {
                d.label.as_str()
            } else {
                d.a_label.as_deref().unwrap_or(&d.label)
            };
            let recs = if added { &d.added } else { &d.removed };
            v.extend(
                recs.iter()
                    .filter(|r| r.did < METADATA_DID_BASE)
                    .map(|r| (r.did, label)),
            );
        }
        v
    };
    let n_added: usize = archives.iter().map(|d| d.added.len()).sum();
    let n_removed: usize = archives.iter().map(|d| d.removed.len()).sum();
    let index_added = n_added <= n_removed;
    let mut index = side(index_added);
    index.sort_unstable();
    let mut moved = Vec::new();
    for d in archives {
        let (label, recs) = if index_added {
            (d.a_label.as_deref().unwrap_or(&d.label), &d.removed)
        } else {
            (d.label.as_str(), &d.added)
        };
        for r in recs.iter().filter(|r| r.did < METADATA_DID_BASE) {
            let start = index.partition_point(|(did, _)| *did < r.did);
            for &(_, other) in index[start..].iter().take_while(|(did, _)| *did == r.did) {
                let (from, to) = if index_added {
                    (label, other)
                } else {
                    (other, label)
                };
                if from != to {
                    moved.push(Move {
                        did: r.did,
                        from: from.to_owned(),
                        to: to.to_owned(),
                    });
                }
            }
        }
    }
    moved.sort_by(|x, y| (x.did, &x.from, &x.to).cmp(&(y.did, &y.from, &y.to)));
    moved
}

// ---------------------------------------------------------------------------------------
// Output

fn side_json(s: &Side) -> Value {
    let i = &s.index;
    json!({
        "source": i.source,
        "origin": match s.origin { Origin::Live => i.source_kind.name(), Origin::Snapshot => "snapshot" },
        "source_kind": i.source_kind.name(),
        "created": if s.origin == Origin::Snapshot { json!(i.created) } else { Value::Null },
        "tool": if s.origin == Origin::Snapshot { json!(i.tool) } else { Value::Null },
        "archives": i.archives.len(),
        "records": i.records(),
        "hashes": i.hashed,
        "unreadable": i.archives.iter().map(|a| a.unreadable).sum::<u64>(),
        "incomplete": i.archives.iter().filter(|a| !a.complete).map(|a| a.label.as_str()).collect::<Vec<_>>(),
    })
}

fn rec_json(r: &Rec) -> Value {
    json!({
        "did": did_hex(r.did),
        "type": kind_name((r.did >> 24) as u8),
        "iteration": r.iteration,
        "size": r.size,
        "time": r.time,
        "stamp": r.stamp,
        "hash": r.hash.as_ref().map(|h| hex_bytes(h)),
    })
}

fn change_json(c: &Change<'_>) -> Value {
    json!({
        "did": did_hex(c.b.did),
        "type": kind_name((c.b.did >> 24) as u8),
        "reasons": c.reasons.names(),
        "a": rec_json(c.a),
        "b": rec_json(c.b),
    })
}

fn side_text(s: &Side) -> String {
    let i = &s.index;
    let mut t = format!(
        "{} ({}, {} archives, {} records{})",
        i.source,
        match s.origin {
            Origin::Live => i.source_kind.name(),
            Origin::Snapshot => "snapshot",
        },
        i.archives.len(),
        i.records(),
        if i.hashed { ", hashed" } else { "" }
    );
    if s.origin == Origin::Snapshot {
        t.push_str(&format!(
            ", taken {} UTC",
            format_time(u32::try_from(i.created).unwrap_or(u32::MAX))
        ));
    }
    t
}

fn status(d: &ArchiveDiff<'_>) -> &'static str {
    match (d.in_a, d.in_b) {
        (true, true) => "both",
        (true, false) => "only_a",
        (false, true) => "only_b",
        (false, false) => "none",
    }
}

fn change_text(c: &Change<'_>) -> String {
    let mut parts = Vec::new();
    if c.reasons.iteration {
        parts.push(format!("iteration {} -> {}", c.a.iteration, c.b.iteration));
    }
    if c.reasons.size {
        parts.push(format!("size {} -> {}", c.a.size, c.b.size));
    }
    if c.reasons.content {
        parts.push("content".to_owned());
    }
    parts.join("; ")
}

fn compared_fields(r: &Report<'_>) -> Vec<&'static str> {
    let mut v = vec!["iteration", "size"];
    if r.content {
        v.push("content");
    }
    v
}

/// Streams a JSON array of `items` as `"key":[...]` (each item on its own line), so no
/// per-record [`Value`] tree is ever held for the whole report.
fn json_list<'x, T: 'x>(
    w: &mut dyn Write,
    key: &str,
    items: impl IntoIterator<Item = &'x T>,
    f: impl Fn(&T) -> Value,
) -> Result<()> {
    write!(w, ",\n{}:[", serde_json::to_string(key)?)?;
    for (n, x) in items.into_iter().enumerate() {
        if n > 0 {
            w.write_all(b",")?;
        }
        w.write_all(b"\n")?;
        serde_json::to_writer(&mut *w, &f(x))?;
    }
    w.write_all(b"]")?;
    Ok(())
}

/// Writes `"key":value` after a comma (or first, with `first`).
fn json_field(w: &mut dyn Write, key: &str, v: &Value, first: bool) -> Result<()> {
    if !first {
        w.write_all(b",\n")?;
    }
    write!(w, "{}:", serde_json::to_string(key)?)?;
    serde_json::to_writer(&mut *w, v)?;
    Ok(())
}

fn write_json(w: &mut dyn Write, a: &Side, b: &Side, r: &Report<'_>, o: &Opts) -> Result<()> {
    w.write_all(b"{")?;
    json_field(w, "a", &side_json(a), true)?;
    json_field(w, "b", &side_json(b), false)?;
    json_field(w, "compared", &json!(compared_fields(r)), false)?;
    json_field(
        w,
        "prefix",
        &json!(o.prefix.map(|p| format!("0x{p:02X}"))),
        false,
    )?;
    json_field(w, "identical", &json!(r.identical()), false)?;
    json_field(w, "incomplete", &json!(r.incomplete()), false)?;
    json_field(
        w,
        "totals",
        &json!({
            "added": r.added(),
            "removed": r.removed(),
            "changed": r.changed(),
            "unchanged": r.unchanged(),
            "moved": r.moved.len(),
            "not_compared": r.not_compared(),
        }),
        false,
    )?;
    let by_type: Vec<Value> = r
        .by_type
        .iter()
        .map(|(p, c)| {
            json!({
                "prefix": format!("0x{p:02X}"),
                "type": kind_name(*p),
                "added": c.added,
                "removed": c.removed,
                "changed": c.changed,
            })
        })
        .collect();
    json_field(w, "by_type", &Value::Array(by_type), false)?;
    write!(w, ",\n\"archives\":[")?;
    for (n, d) in r.archives.iter().enumerate() {
        if n > 0 {
            w.write_all(b",")?;
        }
        w.write_all(b"\n{")?;
        let head = json!({
            "archive": d.label,
            "a_archive": d.a_label,
            "status": status(d),
            "a_records": d.a_records,
            "b_records": d.b_records,
            "added": d.added.len(),
            "removed": d.removed.len(),
            "changed": d.changed.len(),
            "unchanged": d.unchanged,
            "content_compared": d.content_compared,
            "not_compared": d.not_compared,
            "a_complete": d.a_complete,
            "b_complete": d.b_complete,
            "a_walk_error": d.a_error,
            "b_walk_error": d.b_error,
        });
        let mut first = true;
        if let Value::Object(m) = &head {
            for (k, v) in m {
                json_field(w, k, v, first)?;
                first = false;
            }
        }
        if !o.summary {
            json_list(w, "added_dids", d.added.iter().copied(), rec_json)?;
            json_list(w, "removed_dids", d.removed.iter().copied(), rec_json)?;
            json_list(w, "changed_dids", &d.changed, change_json)?;
        }
        w.write_all(b"}")?;
    }
    w.write_all(b"]")?;
    if !o.summary {
        json_list(
            w,
            "moved",
            &r.moved,
            |m| json!({"did": did_hex(m.did), "from": m.from, "to": m.to}),
        )?;
    }
    w.write_all(b"}\n")?;
    Ok(())
}

/// Writes the report as text or JSON. JSON is streamed record by record.
pub fn write_report(out: &mut Out, a: &Side, b: &Side, r: &Report<'_>, o: &Opts) -> Result<()> {
    if out.json {
        return write_json(&mut out.w, a, b, r, o);
    }
    let compared = compared_fields(r);
    let w = &mut out.w;
    writeln!(w, "A: {}", side_text(a))?;
    writeln!(w, "B: {}", side_text(b))?;
    write!(w, "compared: {}", compared.join(", "))?;
    if let Some(p) = o.prefix {
        write!(w, " (DIDs 0x{p:02X}xxxxxx only)")?;
    }
    writeln!(w)?;
    if o.content && !r.content {
        writeln!(
            w,
            "(!) content not compared: a snapshot side was taken without --content"
        )?;
    }
    writeln!(w)?;

    let mut t = Table::new(&[
        ("ARCHIVE", Align::Left),
        ("A", Align::Right),
        ("B", Align::Right),
        ("ADDED", Align::Right),
        ("REMOVED", Align::Right),
        ("CHANGED", Align::Right),
        ("SAME", Align::Right),
        ("NOTE", Align::Left),
    ]);
    for d in &r.archives {
        let mut note = Vec::new();
        match status(d) {
            "only_a" => note.push("only in A".to_owned()),
            "only_b" => note.push("only in B".to_owned()),
            _ => {}
        }
        if let Some(al) = &d.a_label {
            note.push(format!("A is {al}"));
        }
        if !d.a_complete {
            note.push("A incomplete".to_owned());
        }
        if !d.b_complete {
            note.push("B incomplete".to_owned());
        }
        if d.not_compared > 0 {
            note.push(format!("{} not compared", d.not_compared));
        }
        t.row(vec![
            d.label.clone(),
            if d.in_a {
                d.a_records.to_string()
            } else {
                "-".into()
            },
            if d.in_b {
                d.b_records.to_string()
            } else {
                "-".into()
            },
            d.added.len().to_string(),
            d.removed.len().to_string(),
            d.changed.len().to_string(),
            d.unchanged.to_string(),
            note.join(", "),
        ]);
    }
    t.row(vec![
        "TOTAL".into(),
        r.archives
            .iter()
            .map(|d| d.a_records)
            .sum::<u64>()
            .to_string(),
        r.archives
            .iter()
            .map(|d| d.b_records)
            .sum::<u64>()
            .to_string(),
        r.added().to_string(),
        r.removed().to_string(),
        r.changed().to_string(),
        r.unchanged().to_string(),
        if r.moved.is_empty() {
            String::new()
        } else {
            format!("{} moved between archives", r.moved.len())
        },
    ]);
    t.write(w)?;

    if r.incomplete() {
        writeln!(w)?;
        for d in r.archives.iter().filter(|d| d.incomplete()) {
            for (side, complete, err) in [
                ("A", d.a_complete, d.a_error),
                ("B", d.b_complete, d.b_error),
            ] {
                if !complete {
                    writeln!(
                        w,
                        "(!) {}: directory walk of {side} stopped early: {}",
                        d.label,
                        err.unwrap_or("(recorded as incomplete in the snapshot)")
                    )?;
                }
            }
        }
        writeln!(
            w,
            "(!) comparison incomplete: {} DIDs beyond where a walk stopped were not compared",
            r.not_compared()
        )?;
    }

    if r.identical() {
        if r.incomplete() {
            writeln!(w, "\nno differences among the records compared")?;
        } else {
            writeln!(w, "\nno differences")?;
        }
        return Ok(());
    }

    writeln!(w, "\nby DID type:")?;
    let mut t = Table::new(&[
        ("PREFIX", Align::Left),
        ("TYPE", Align::Left),
        ("ADDED", Align::Right),
        ("REMOVED", Align::Right),
        ("CHANGED", Align::Right),
    ]);
    for (p, c) in &r.by_type {
        t.row(vec![
            format!("0x{p:02X}"),
            kind_name(*p).to_owned(),
            c.added.to_string(),
            c.removed.to_string(),
            c.changed.to_string(),
        ]);
    }
    t.write(w)?;

    if o.summary || o.max_list == 0 {
        return Ok(());
    }
    let n = o.max_list;
    for d in &r.archives {
        if d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty() {
            continue;
        }
        writeln!(
            w,
            "\n== {}: +{} -{} ~{}",
            d.label,
            d.added.len(),
            d.removed.len(),
            d.changed.len()
        )?;
        let more = |w: &mut dyn Write, total: usize, what: &str| -> std::io::Result<()> {
            if total > n {
                writeln!(
                    w,
                    "  ... {} more {what} (raise --max-list or use --json)",
                    total - n
                )?;
            }
            Ok(())
        };
        for x in d.added.iter().take(n) {
            writeln!(
                w,
                "  + {}  {:<24}  size {}  iteration {}",
                did_hex(x.did),
                kind_name((x.did >> 24) as u8),
                x.size,
                x.iteration
            )?;
        }
        more(w, d.added.len(), "added")?;
        for x in d.removed.iter().take(n) {
            writeln!(
                w,
                "  - {}  {:<24}  size {}  iteration {}",
                did_hex(x.did),
                kind_name((x.did >> 24) as u8),
                x.size,
                x.iteration
            )?;
        }
        more(w, d.removed.len(), "removed")?;
        for c in d.changed.iter().take(n) {
            writeln!(
                w,
                "  ~ {}  {:<24}  {}",
                did_hex(c.b.did),
                kind_name((c.b.did >> 24) as u8),
                change_text(c)
            )?;
        }
        more(w, d.changed.len(), "changed")?;
    }
    if !r.moved.is_empty() {
        writeln!(w, "\nmoved between archives:")?;
        for m in r.moved.iter().take(n) {
            writeln!(w, "  {}  {} -> {}", did_hex(m.did), m.from, m.to)?;
        }
        if r.moved.len() > n {
            writeln!(w, "  ... {} more", r.moved.len() - n)?;
        }
    }
    Ok(())
}

/// The exit status for a finished report: [`EXIT_TROUBLE`] when it is incomplete,
/// [`EXIT_DIFFERENT`] for differences under `--exit-code`, else success.
pub fn exit_status(r: &Report<'_>, exit_code: bool) -> ExitCode {
    if r.incomplete() {
        ExitCode::from(EXIT_TROUBLE)
    } else if exit_code && !r.identical() {
        ExitCode::from(EXIT_DIFFERENT)
    } else {
        ExitCode::SUCCESS
    }
}

/// `diff <A> <B> [--content] [--prefix 0xHH] [--summary] [--max-list N] [--exit-code]`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let (a, b) = load_sides(o, ctx)?;
    let report = diff_indexes(&a.index, &b.index, o.prefix)?;
    write_report(out, &a, &b, &report, o)?;
    if report.incomplete() {
        tracing::warn!(
            not_compared = report.not_compared(),
            "comparison incomplete: a directory walk stopped early"
        );
    }
    Ok(exit_status(&report, o.exit_code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_dattool::snapfile::{HASH_LEN, Hash};

    fn rec(did: u32, iteration: u32, size: u32, h: Option<u8>) -> Rec {
        Rec {
            did,
            iteration,
            size,
            time: 1,
            stamp: 0,
            flags: 2,
            hash: h.map(|x| -> Hash { [x; HASH_LEN] }),
        }
    }

    fn arch(label: &str, recs: Vec<Rec>) -> ArchiveIndex {
        ArchiveIndex {
            label: label.into(),
            recs,
            complete: true,
            walk_error: None,
            unreadable: 0,
        }
    }

    fn idx(kind: SourceKind, hashed: bool, archives: Vec<ArchiveIndex>) -> Index {
        Index {
            source: "x".into(),
            source_kind: kind,
            created: 0,
            tool: String::new(),
            hashed,
            archives,
        }
    }

    fn dids(v: &[&Rec]) -> Vec<u32> {
        v.iter().map(|r| r.did).collect()
    }

    #[test]
    fn merge_classifies_every_did() {
        let a = arch(
            "general",
            vec![
                rec(0x0100_0001, 1, 10, Some(1)),
                rec(0x0100_0002, 1, 10, Some(1)),
                rec(0x0100_0003, 1, 10, Some(1)),
                rec(0x2500_0001, 1, 10, Some(1)),
                rec(0x2500_0002, 1, 10, Some(1)),
            ],
        );
        let b = arch(
            "general",
            vec![
                rec(0x0100_0001, 1, 10, Some(1)), // same
                rec(0x0100_0002, 2, 10, Some(1)), // iteration
                // 0x01000003 removed
                rec(0x0100_0004, 1, 10, Some(1)), // added
                rec(0x2500_0001, 1, 11, Some(1)), // size
                rec(0x2500_0002, 1, 10, Some(2)), // content only
            ],
        );
        let d = diff_archive(Some(&a), Some(&b), true, None);
        assert_eq!(dids(&d.added), [0x0100_0004]);
        assert_eq!(dids(&d.removed), [0x0100_0003]);
        let ch: Vec<(u32, Vec<&str>)> = d
            .changed
            .iter()
            .map(|c| (c.b.did, c.reasons.names()))
            .collect();
        assert_eq!(
            ch,
            [
                (0x0100_0002, vec!["iteration"]),
                (0x2500_0001, vec!["size"]),
                (0x2500_0002, vec!["content"]),
            ]
        );
        assert_eq!(d.unchanged, 1);
        assert_eq!(d.content_compared, 4);
        assert_eq!(d.not_compared, 0);
        // Without content comparison the hash-only change is invisible.
        let d = diff_archive(Some(&a), Some(&b), false, None);
        assert_eq!(d.changed.len(), 2);
        assert_eq!(d.unchanged, 2);
        // The prefix filter limits every category.
        let d = diff_archive(Some(&a), Some(&b), true, Some(0x25));
        assert_eq!((d.added.len(), d.removed.len(), d.changed.len()), (0, 0, 2));
        assert_eq!((d.a_records, d.b_records), (2, 2));
    }

    #[test]
    fn time_and_stamp_alone_are_not_changes() {
        let a = arch("g", vec![rec(1, 1, 1, None)]);
        let mut r = rec(1, 1, 1, None);
        r.time = 99;
        r.stamp = 5;
        let b = arch("g", vec![r]);
        let d = diff_archive(Some(&a), Some(&b), true, None);
        assert!(d.changed.is_empty());
        assert_eq!(d.unchanged, 1);
    }

    #[test]
    fn pairs_archives_by_label_and_detects_moves() {
        let a = idx(
            SourceKind::Dir,
            false,
            vec![
                arch("general", vec![rec(1, 1, 1, None), rec(2, 1, 1, None)]),
                arch("old", vec![rec(9, 1, 1, None)]),
                arch("surface_aux_1", vec![rec(0x4100_0001, 1, 1, None)]),
            ],
        );
        let b = idx(
            SourceKind::Dir,
            false,
            vec![
                arch("General", vec![rec(1, 1, 1, None)]),
                arch("surface_aux_1", vec![]),
                arch("surface_aux_2", vec![rec(0x4100_0001, 1, 1, None)]),
            ],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        let st: Vec<(&str, &str)> = r
            .archives
            .iter()
            .map(|d| (d.label.as_str(), status(d)))
            .collect();
        assert_eq!(
            st,
            [
                ("General", "both"),
                ("old", "only_a"),
                ("surface_aux_1", "both"),
                ("surface_aux_2", "only_b"),
            ]
        );
        assert_eq!(r.removed(), 3);
        assert_eq!(r.added(), 1);
        assert_eq!(
            r.moved,
            [Move {
                did: 0x4100_0001,
                from: "surface_aux_1".into(),
                to: "surface_aux_2".into(),
            }]
        );
        assert_eq!(
            r.by_type[&0x41],
            TypeCounts {
                added: 1,
                removed: 1,
                changed: 0
            }
        );
        assert_eq!(r.by_type[&0x00].removed, 2);
        assert!(!r.identical());
        assert!(!r.incomplete());
    }

    #[test]
    fn housekeeping_dids_never_move_and_every_source_is_kept() {
        let meta = |extra: Vec<Rec>| {
            let mut v = extra;
            v.extend([0xFFFF_0001, 0xFFFF_0003].map(|d| rec(d, 1, 1, None)));
            v
        };
        let a = idx(
            SourceKind::Dir,
            false,
            vec![
                arch("cell_15", meta(vec![rec(0x0100_0001, 1, 1, None)])),
                arch("cell_16", meta(vec![rec(0x0100_0001, 1, 1, None)])),
            ],
        );
        let b = idx(
            SourceKind::Dir,
            false,
            vec![arch("map_15", meta(vec![rec(0x0100_0001, 1, 1, None)]))],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        assert_eq!(
            r.moved,
            [
                Move {
                    did: 0x0100_0001,
                    from: "cell_15".into(),
                    to: "map_15".into(),
                },
                Move {
                    did: 0x0100_0001,
                    from: "cell_16".into(),
                    to: "map_15".into(),
                },
            ]
        );
    }

    #[test]
    fn single_archives_pair_regardless_of_name_or_by_label_against_a_set() {
        let a = idx(
            SourceKind::Archive,
            false,
            vec![arch("one", vec![rec(1, 1, 1, None)])],
        );
        let b = idx(
            SourceKind::Archive,
            false,
            vec![arch("two", vec![rec(1, 1, 1, None)])],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        assert_eq!(r.archives.len(), 1);
        assert_eq!(r.archives[0].a_label.as_deref(), Some("one"));
        assert!(r.identical());

        let set = idx(
            SourceKind::Dir,
            false,
            vec![
                arch("zero", vec![rec(5, 1, 1, None)]),
                arch("one", vec![rec(1, 2, 1, None)]),
            ],
        );
        let r = diff_indexes(&a, &set, None).unwrap();
        assert_eq!(r.archives.len(), 1);
        assert_eq!(r.changed(), 1);
        assert!(diff_indexes(&b, &set, None).is_err());
    }

    #[test]
    fn hashes_are_compared_whenever_both_sides_have_them() {
        let a = idx(
            SourceKind::Dir,
            true,
            vec![arch("g", vec![rec(1, 1, 1, Some(1))])],
        );
        // No hashes on B: nothing to compare.
        let b = idx(
            SourceKind::Dir,
            false,
            vec![arch("g", vec![rec(1, 1, 1, None)])],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        assert!(!r.content);
        assert!(r.identical());
        // An unreadable payload on one side is not a change by itself.
        let b = idx(
            SourceKind::Dir,
            true,
            vec![arch("g", vec![rec(1, 1, 1, None)])],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        assert!(r.content);
        assert!(r.identical());
        assert_eq!(r.archives[0].content_compared, 0);
        // Two hashed sides compare hashes without being asked: a content-only change shows.
        let b = idx(
            SourceKind::Dir,
            true,
            vec![arch("g", vec![rec(1, 1, 1, Some(9))])],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        assert!(r.content);
        assert_eq!(r.changed(), 1);
        assert_eq!(r.archives[0].changed[0].reasons.names(), ["content"]);
    }

    #[test]
    fn dids_past_an_incomplete_walk_are_not_compared() {
        let full = arch(
            "map_1",
            (1..=10).map(|d| rec(d, 1, 1, None)).collect::<Vec<_>>(),
        );
        // B's walk stopped after DID 4.
        let mut cut = arch("map_1", (1..=4).map(|d| rec(d, 1, 1, None)).collect());
        cut.complete = false;
        cut.walk_error = Some("child node lies outside the data area".into());
        let d = diff_archive(Some(&full), Some(&cut), false, None);
        assert!(d.removed.is_empty(), "{:?}", dids(&d.removed));
        assert_eq!(d.not_compared, 6);
        assert_eq!(d.unchanged, 4);
        assert_eq!(d.b_error, Some("child node lies outside the data area"));
        // A DID B never had, below the cut-off, is still a real removal.
        let mut cut2 = arch("map_1", [1, 2, 4].map(|d| rec(d, 1, 1, None)).to_vec());
        cut2.complete = false;
        let d = diff_archive(Some(&full), Some(&cut2), false, None);
        assert_eq!(dids(&d.removed), [3]);
        assert_eq!(d.not_compared, 6);
        // An incomplete walk that found nothing covers nothing.
        let mut none = arch("map_1", vec![]);
        none.complete = false;
        let d = diff_archive(Some(&none), Some(&full), false, None);
        assert!(d.added.is_empty());
        assert_eq!(d.not_compared, 10);

        // Both sides stopped at the same point: identical among the compared records, but
        // the report is incomplete and the exit status says so even without --exit-code.
        let mut a = cut.clone();
        a.walk_error = None;
        let ia = idx(SourceKind::Archive, false, vec![a]);
        let ib = idx(SourceKind::Archive, false, vec![cut]);
        let r = diff_indexes(&ia, &ib, None).unwrap();
        assert!(r.identical());
        assert!(r.incomplete());
        assert_eq!(exit_status(&r, false), ExitCode::from(EXIT_TROUBLE));
        let ok = idx(SourceKind::Archive, false, vec![full.clone()]);
        let r = diff_indexes(&ok, &ok, None).unwrap();
        assert_eq!(exit_status(&r, true), ExitCode::SUCCESS);
    }

    #[test]
    fn json_is_streamed_and_well_formed() {
        let a = idx(
            SourceKind::Dir,
            false,
            vec![arch("g", vec![rec(1, 1, 1, None), rec(2, 1, 1, None)])],
        );
        let b = idx(
            SourceKind::Dir,
            false,
            vec![
                arch("g", vec![rec(2, 2, 1, None), rec(3, 1, 1, None)]),
                arch("h", vec![rec(1, 1, 1, None)]),
            ],
        );
        let r = diff_indexes(&a, &b, None).unwrap();
        let side = |index: Index| Side {
            index,
            origin: Origin::Snapshot,
        };
        let o = Opts {
            a: "a".into(),
            b: "b".into(),
            content: false,
            prefix: None,
            max_list: 20,
            summary: false,
            exit_code: false,
        };
        let mut buf = Vec::new();
        write_json(&mut buf, &side(a.clone()), &side(b.clone()), &r, &o).unwrap();
        let v: Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["totals"]["added"], 2);
        assert_eq!(v["totals"]["removed"], 1);
        assert_eq!(v["totals"]["changed"], 1);
        assert_eq!(v["incomplete"], false);
        assert_eq!(v["archives"][0]["changed_dids"][0]["did"], "0x00000002");
        assert_eq!(v["moved"][0]["to"], "h");
        let o = Opts { summary: true, ..o };
        let mut buf = Vec::new();
        write_json(&mut buf, &side(a.clone()), &side(b.clone()), &r, &o).unwrap();
        let v: Value = serde_json::from_slice(&buf).unwrap();
        assert!(v["archives"][0].get("added_dids").is_none());
        assert!(v.get("moved").is_none());
    }
}
