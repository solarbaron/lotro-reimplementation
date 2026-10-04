// SPDX-License-Identifier: GPL-3.0-or-later
//! The archive set of an install: discovery, naming, membership and DID routing (spec §2, §8).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::archive::{Archive, ArchiveOptions};
use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::routing::{KindTag, RouteTarget, RoutingTable};
use crate::validate::{Issue, Severity, ValidateOptions, ValidationReport};
use crate::{Did, METADATA_DID_BASE};

/// Per-archive validation output plus the DIDs it holds (for I-25).
type ArchiveResult = (ValidationReport, Vec<Did>);

/// Largest part count accepted from a set-membership record (spec §2.3, §7.2). The largest
/// observed set has 4 parts; a larger declared count is reported as implausible instead of
/// being enumerated part by part.
pub const MAX_SET_PARTS: u32 = 64;

/// Formats sorted part numbers as `"2"`, `"2-4"`, `"1, 3-5"`.
fn part_ranges(parts: &[u32]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while let Some(&lo) = parts.get(i) {
        let mut hi = lo;
        while parts.get(i + 1) == Some(&(hi + 1)) {
            hi += 1;
            i += 1;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        if lo == hi {
            out.push_str(&lo.to_string());
        } else {
            out.push_str(&format!("{lo}-{hi}"));
        }
        i += 1;
    }
    out
}

/// The kind of an archive, derived from its file name (spec §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArchiveKind {
    /// `client_general`
    General,
    /// `client_gamelogic`
    Gamelogic,
    /// `client_local_<Language>`
    Local(String),
    /// `client_mesh`
    Mesh,
    /// `client_surface`
    Surface,
    /// `client_highres`
    HighRes,
    /// `client_sound`
    Sound,
    /// `client_anim`
    Anim,
    /// `client_cell_<region>`
    Cell(u32),
    /// `client_map_<region>`
    Map(u32),
    /// Any other `client_<name>` archive.
    Other(String),
}

impl ArchiveKind {
    /// The routing tag of this kind, if it has one.
    pub fn tag(&self) -> Option<KindTag> {
        Some(match self {
            Self::General => KindTag::General,
            Self::Gamelogic => KindTag::Gamelogic,
            Self::Local(_) => KindTag::Local,
            Self::Mesh => KindTag::Mesh,
            Self::Surface => KindTag::Surface,
            Self::HighRes => KindTag::HighRes,
            Self::Sound => KindTag::Sound,
            Self::Anim => KindTag::Anim,
            Self::Cell(_) => KindTag::Cell,
            Self::Map(_) => KindTag::Map,
            Self::Other(_) => return None,
        })
    }

    /// The region number of a cell or map archive.
    pub fn region(&self) -> Option<u32> {
        match self {
            Self::Cell(r) | Self::Map(r) => Some(*r),
            _ => None,
        }
    }
}

impl fmt::Display for ArchiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(lang) => write!(f, "local_{lang}"),
            Self::Cell(r) => write!(f, "cell_{r}"),
            Self::Map(r) => write!(f, "map_{r}"),
            Self::Other(s) => f.write_str(s),
            k => f.write_str(k.tag().map_or("?", KindTag::name)),
        }
    }
}

/// Identity of one archive file: its kind and part number (0 = `.dat` base, N = `_aux_N`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArchiveId {
    /// The archive kind.
    pub kind: ArchiveKind,
    /// Part number within the kind's set.
    pub part: u32,
}

impl ArchiveId {
    /// Parses `client_<kind>.dat` or `client_<kind>_aux_<N>.datx` (spec §2.1).
    ///
    /// Returns `None` for names without the `client_` prefix or a `.dat`/`.datx`
    /// extension. Whether the file really is an archive is decided by its magic.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("client_")?;
        let stem = rest
            .strip_suffix(".datx")
            .or_else(|| rest.strip_suffix(".dat"))?;
        let (base, part) = match stem.rsplit_once("_aux_") {
            Some((b, n)) => match n.parse::<u32>() {
                Ok(p) if p > 0 && !b.is_empty() => (b, p),
                _ => (stem, 0),
            },
            None => (stem, 0),
        };
        if base.is_empty() {
            return None;
        }
        let kind = match base {
            "general" => ArchiveKind::General,
            "gamelogic" => ArchiveKind::Gamelogic,
            "mesh" => ArchiveKind::Mesh,
            "surface" => ArchiveKind::Surface,
            "highres" => ArchiveKind::HighRes,
            "sound" => ArchiveKind::Sound,
            "anim" => ArchiveKind::Anim,
            _ => {
                if let Some(lang) = base.strip_prefix("local_").filter(|l| !l.is_empty()) {
                    ArchiveKind::Local(lang.to_owned())
                } else if let Some(r) = base.strip_prefix("cell_").and_then(|r| r.parse().ok()) {
                    ArchiveKind::Cell(r)
                } else if let Some(r) = base.strip_prefix("map_").and_then(|r| r.parse().ok()) {
                    ArchiveKind::Map(r)
                } else {
                    ArchiveKind::Other(base.to_owned())
                }
            }
        };
        Some(Self { kind, part })
    }
}

impl fmt::Display for ArchiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.part == 0 {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "{}_aux_{}", self.kind, self.part)
        }
    }
}

/// Options for [`ArchiveSet::open`].
#[derive(Debug, Clone)]
pub struct SetOptions {
    /// Options applied to every archive.
    pub archive: ArchiveOptions,
    /// DID → archive routing table.
    pub routing: RoutingTable,
    /// Search every archive when the routed candidates do not hold a DID. Default `true`.
    pub fallback_search_all: bool,
}

impl Default for SetOptions {
    fn default() -> Self {
        Self {
            archive: ArchiveOptions::default(),
            routing: RoutingTable::default(),
            fallback_search_all: true,
        }
    }
}

/// One archive of a set.
#[derive(Debug)]
pub struct SetMember {
    /// Identity derived from the file name.
    pub id: ArchiveId,
    /// Full path of the file.
    pub path: PathBuf,
    /// The opened archive.
    pub archive: Archive,
}

/// A non-fatal problem found while opening a set (unopenable file, missing aux part, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetProblem {
    /// The file concerned, if any.
    pub path: Option<PathBuf>,
    /// Description.
    pub message: String,
}

/// Where [`ArchiveSet::locate`] found a DID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Located {
    /// Index into [`ArchiveSet::members`].
    pub member: usize,
    /// The directory entry.
    pub entry: Entry,
    /// `true` if found in a routed candidate, `false` if found by the fallback search.
    pub routed: bool,
}

/// Validation result for a whole set.
#[derive(Debug, Clone, Default)]
pub struct SetValidationReport {
    /// Per-archive reports, in member order.
    pub archives: Vec<(ArchiveId, ValidationReport)>,
    /// Set-level issues: I-24 (membership consistency) and I-25 (duplicate DIDs).
    pub issues: Vec<Issue>,
    /// Number of distinct DIDs (< 0xFFFF0000) found in more than one archive. A DID
    /// repeated inside one archive only is an I-15 error of that archive, not counted here.
    pub duplicate_dids: u64,
}

impl SetValidationReport {
    /// Whether no archive and no set-level check reported an error.
    pub fn is_ok(&self) -> bool {
        self.archives.iter().all(|(_, r)| r.is_ok())
            && self.issues.iter().all(|i| i.severity != Severity::Error)
    }
}

/// All archives of one install directory, with DID routing (spec §2, §8).
#[derive(Debug)]
pub struct ArchiveSet {
    dir: PathBuf,
    members: Vec<SetMember>,
    routing: RoutingTable,
    fallback: bool,
    problems: Vec<SetProblem>,
}

impl ArchiveSet {
    /// Opens every `client_*.dat` / `client_*.datx` archive directly inside `dir`.
    ///
    /// Files whose magic is wrong or that cannot be opened are skipped and reported in
    /// [`ArchiveSet::problems`], as are membership inconsistencies such as a missing aux
    /// part. Fails only if the directory cannot be listed or holds no usable archive.
    pub fn open(dir: impl AsRef<Path>, options: SetOptions) -> Result<Self> {
        let dir = dir.as_ref().to_owned();
        let rd = std::fs::read_dir(&dir).map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        let mut found: Vec<(ArchiveId, PathBuf)> = Vec::new();
        for ent in rd {
            let ent = ent.map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            let name = ent.file_name();
            let Some(id) = name.to_str().and_then(ArchiveId::from_file_name) else {
                continue;
            };
            let path = ent.path();
            if path.is_file() {
                found.push((id, path));
            }
        }
        found.sort();

        let mut members = Vec::new();
        let mut problems = Vec::new();
        for (id, path) in found {
            match Archive::open_with(&path, options.archive.clone()) {
                Ok(archive) => members.push(SetMember { id, path, archive }),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping archive");
                    problems.push(SetProblem {
                        path: Some(path),
                        message: format!("not opened: {e}"),
                    });
                }
            }
        }
        if members.is_empty() {
            return Err(Error::NoArchives { path: dir });
        }
        let mut set = Self {
            dir,
            members,
            routing: options.routing,
            fallback: options.fallback_search_all,
            problems,
        };
        set.check_membership();
        Ok(set)
    }

    fn check_membership(&mut self) {
        // kind → (set id, part count) as declared by the first part that has a record.
        let mut declared: BTreeMap<ArchiveKind, ([u8; 16], u32)> = BTreeMap::new();
        let mut problems = Vec::new();
        for m in &self.members {
            match m.archive.membership() {
                Ok(Some(ms)) => {
                    if ms.part_index != m.id.part {
                        problems.push(SetProblem {
                            path: Some(m.path.clone()),
                            message: format!(
                                "membership record says part {}, file name says part {}",
                                ms.part_index, m.id.part
                            ),
                        });
                    }
                    match declared.get(&m.id.kind) {
                        None => {
                            declared.insert(m.id.kind.clone(), (ms.set_id, ms.part_count));
                        }
                        Some((sid, count)) => {
                            if *sid != ms.set_id || *count != ms.part_count {
                                problems.push(SetProblem {
                                    path: Some(m.path.clone()),
                                    message: format!(
                                        "membership record disagrees with other parts of {}",
                                        m.id.kind
                                    ),
                                });
                            }
                        }
                    }
                }
                Ok(None) => problems.push(SetProblem {
                    path: Some(m.path.clone()),
                    message: "no set-membership record (0xFFFF0005)".into(),
                }),
                Err(e) => problems.push(SetProblem {
                    path: Some(m.path.clone()),
                    message: format!("set-membership record unreadable: {e}"),
                }),
            }
        }
        for (kind, &(_, count)) in &declared {
            // `count` comes straight from the file: never enumerate an absurd value.
            if count > MAX_SET_PARTS {
                tracing::warn!(%kind, count, "implausible set part count");
                problems.push(SetProblem {
                    path: None,
                    message: format!(
                        "implausible part count {count} for {kind} (limit {MAX_SET_PARTS})"
                    ),
                });
                continue;
            }
            let missing: Vec<u32> = (0..count)
                .filter(|&part| {
                    !self
                        .members
                        .iter()
                        .any(|m| &m.id.kind == kind && m.id.part == part)
                })
                .collect();
            let message = match missing.as_slice() {
                [] => continue,
                &[part] => {
                    let name = ArchiveId {
                        kind: kind.clone(),
                        part,
                    };
                    format!("missing part {part} of {count} for {kind} ({name})")
                }
                many => format!("missing parts {} of {count} for {kind}", part_ranges(many)),
            };
            tracing::warn!(%kind, "{message}");
            problems.push(SetProblem {
                path: None,
                message,
            });
        }
        self.problems.extend(problems);
    }

    /// The directory the set was opened from.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// All opened archives, sorted by kind and part.
    pub fn members(&self) -> &[SetMember] {
        &self.members
    }

    /// The member at `index` (as returned in [`Located::member`]).
    ///
    /// Prefer [`ArchiveSet::get`], [`ArchiveSet::archive_of`] or
    /// [`ArchiveSet::read_located`], which do not panic.
    ///
    /// # Panics
    /// Panics if `index` is out of range, like slice indexing.
    pub fn member(&self, index: usize) -> &SetMember {
        &self.members[index]
    }

    /// The member at `index`, or `None` if it is out of range.
    pub fn get(&self, index: usize) -> Option<&SetMember> {
        self.members.get(index)
    }

    /// The member a [`Located`] refers to, or `None` if it came from another set.
    pub fn archive_of(&self, located: &Located) -> Option<&SetMember> {
        self.members.get(located.member)
    }

    /// Reads (and inflates) the record a [`Located`] refers to.
    ///
    /// Fails with [`Error::NoSuchMember`] if `located` does not belong to this set.
    pub fn read_located(&self, located: &Located) -> Result<Vec<u8>> {
        self.archive_of(located)
            .ok_or(Error::NoSuchMember {
                index: located.member,
            })?
            .archive
            .read_entry(&located.entry)
    }

    /// Locates and reads `did`, returning where it was found together with the bytes.
    pub fn read_with_entry(&self, did: Did) -> Result<(Located, Vec<u8>)> {
        let l = self.locate(did)?.ok_or(Error::NotFound { did })?;
        let bytes = self.read_located(&l)?;
        Ok((l, bytes))
    }

    /// Finds the member with the given kind and part.
    pub fn find(&self, kind: &ArchiveKind, part: u32) -> Option<&SetMember> {
        self.members
            .iter()
            .find(|m| &m.id.kind == kind && m.id.part == part)
    }

    /// Non-fatal problems found while opening.
    pub fn problems(&self) -> &[SetProblem] {
        &self.problems
    }

    /// The routing table in use.
    pub fn routing(&self) -> &RoutingTable {
        &self.routing
    }

    /// Indices of the members the routing table names for `did`, in search order.
    pub fn candidates(&self, did: Did) -> Vec<usize> {
        let region = RoutingTable::region_of(did);
        let mut out: Vec<usize> = Vec::new();
        for t in self.routing.targets(did) {
            for (i, m) in self.members.iter().enumerate() {
                let hit = match t {
                    RouteTarget::Kind(tag) => m.id.kind.tag() == Some(*tag),
                    RouteTarget::Region(tag) => {
                        m.id.kind.tag() == Some(*tag) && m.id.kind.region() == Some(region)
                    }
                };
                if hit && !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        out
    }

    /// Finds the archive holding `did`: routed candidates first, then (if enabled) every
    /// other archive.
    ///
    /// The first copy found wins; duplicates across archives are reported by
    /// [`ArchiveSet::validate`] (I-25). A lookup error in one archive is logged and the
    /// search continues; it is returned only if no archive has the DID.
    pub fn locate(&self, did: Did) -> Result<Option<Located>> {
        let routed = self.candidates(did);
        let mut first_err = None;
        let mut try_member = |i: usize, routed: bool| -> Option<Located> {
            match self.members.get(i)?.archive.lookup(did) {
                Ok(Some(entry)) => Some(Located {
                    member: i,
                    entry,
                    routed,
                }),
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(did, archive = %self.members.get(i).map(|m| m.id.to_string()).unwrap_or_default(), error = %e, "lookup failed");
                    first_err.get_or_insert(e);
                    None
                }
            }
        };
        for &i in &routed {
            if let Some(l) = try_member(i, true) {
                return Ok(Some(l));
            }
        }
        if self.fallback {
            for i in (0..self.members.len()).filter(|i| !routed.contains(i)) {
                if let Some(l) = try_member(i, false) {
                    if !routed.is_empty() && did < METADATA_DID_BASE {
                        tracing::debug!(did, "DID found outside its routed archives");
                    }
                    return Ok(Some(l));
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(None),
        }
    }

    /// Reads (and inflates) the record `did` from whichever archive holds it.
    pub fn read(&self, did: Did) -> Result<Vec<u8>> {
        self.read_with_entry(did).map(|(_, bytes)| bytes)
    }

    /// Validates every archive (in parallel) plus the set-level invariants I-24 and I-25.
    pub fn validate(&self, opts: &ValidateOptions) -> SetValidationReport {
        let n = self.members.len();
        let threads = std::thread::available_parallelism()
            .map_or(1, |p| p.get())
            .min(n)
            .max(1);
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<Option<ArchiveResult>>> = Mutex::new((0..n).map(|_| None).collect());
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(m) = self.members.get(i) else { break };
                        let mut dids = Vec::new();
                        let r = m.archive.validate_collect(opts, Some(&mut dids));
                        if let Ok(mut g) = results.lock()
                            && let Some(slot) = g.get_mut(i)
                        {
                            *slot = Some((r, dids));
                        }
                    }
                });
            }
        });
        let results = results.into_inner().unwrap_or_default();

        let mut report = SetValidationReport::default();
        let mut all: Vec<(Did, usize)> = Vec::new();
        for (i, (m, r)) in self.members.iter().zip(results).enumerate() {
            let (r, dids) = r.unwrap_or_default();
            all.extend(dids.into_iter().map(|d| (d, i)));
            report.archives.push((m.id.clone(), r));
        }
        for p in &self.problems {
            report.issues.push(Issue {
                invariant: 24,
                severity: Severity::Warning,
                offset: None,
                message: match &p.path {
                    Some(path) => format!("{}: {}", path.display(), p.message),
                    None => p.message.clone(),
                },
            });
        }
        // Sorting by (DID, member) and dropping equal pairs leaves one row per archive
        // that holds a DID; in-archive repeats are I-15 errors of that archive.
        all.sort_unstable();
        all.dedup();
        let mut shown = 0usize;
        for group in all.chunk_by(|a, b| a.0 == b.0) {
            let [(did, _), _, ..] = group else { continue };
            report.duplicate_dids += 1;
            if shown < opts.max_issues_per_invariant {
                shown += 1;
                let names: Vec<String> = group
                    .iter()
                    .filter_map(|&(_, i)| self.members.get(i).map(|m| m.id.to_string()))
                    .collect();
                report.issues.push(Issue {
                    invariant: 25,
                    severity: Severity::Warning,
                    offset: None,
                    message: format!("DID {did:#010x} is in {}", names.join(", ")),
                });
            }
        }
        report
    }
}
