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

/// Strips an ASCII case-insensitive prefix.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| s.get(prefix.len()..))
        .flatten()
}

/// Strips an ASCII case-insensitive suffix.
fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = s.len().checked_sub(suffix.len())?;
    let tail = s.get(cut..)?;
    tail.eq_ignore_ascii_case(suffix)
        .then(|| s.get(..cut))
        .flatten()
}

/// Splits `<key>_aux_<n>` at the last `_aux_` (ASCII case-insensitive).
fn split_aux(stem: &str) -> Option<(&str, &str)> {
    let pos = stem.to_ascii_lowercase().rfind("_aux_")?;
    Some((stem.get(..pos)?, stem.get(pos + 5..)?))
}

/// Decimal digits without leading zeros that fit a `u32` (install-layout §3.1).
fn parse_decimal(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return None;
    }
    s.parse().ok()
}

/// A language label: letters, digits, `_` and `-`, at most 64 bytes.
fn is_language(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Parses an archive key (`general`, `local_<Lang>`, `cell_<r>`, ...), install-layout §3.1.
fn parse_key(key: &str) -> Option<ArchiveKind> {
    if let Some(lang) = strip_prefix_ci(key, "local_") {
        return is_language(lang).then(|| ArchiveKind::Local(lang.to_owned()));
    }
    if let Some(r) = strip_prefix_ci(key, "cell_") {
        return parse_decimal(r).map(ArchiveKind::Cell);
    }
    if let Some(r) = strip_prefix_ci(key, "map_") {
        return parse_decimal(r).map(ArchiveKind::Map);
    }
    let word = key.to_ascii_lowercase();
    Some(match word.as_str() {
        "general" => ArchiveKind::General,
        "gamelogic" => ArchiveKind::Gamelogic,
        "mesh" => ArchiveKind::Mesh,
        "surface" => ArchiveKind::Surface,
        "highres" => ArchiveKind::HighRes,
        "sound" => ArchiveKind::Sound,
        "anim" => ArchiveKind::Anim,
        w if !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()) => ArchiveKind::Other(word),
        _ => return None,
    })
}

impl ArchiveId {
    /// Parses an archive file name against the grammar of install-layout §3.1:
    /// `client_<key>.dat` for a base and `client_<key>_aux_<N>.datx` for part N ≥ 1, where
    /// `<key>` is a lower-case word, `local_<Lang>`, `cell_<r>` or `map_<r>` (`<r>` and `<N>`
    /// decimal without leading zeros).
    ///
    /// Matching is ASCII case-insensitive (install-layout §2); the language label keeps
    /// the case it is written in, and an unknown word is kept in lower case as
    /// [`ArchiveKind::Other`]. Returns `None` for every other name, including aux parts
    /// named `.dat`. Whether the file really is an archive is decided by its magic.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let rest = strip_prefix_ci(name, "client_")?;
        if let Some(stem) = strip_suffix_ci(rest, ".datx") {
            let (key, n) = split_aux(stem)?;
            let part = parse_decimal(n).filter(|&p| p > 0)?;
            return Some(Self {
                kind: parse_key(key)?,
                part,
            });
        }
        let stem = strip_suffix_ci(rest, ".dat")?;
        if split_aux(stem).is_some() {
            return None;
        }
        Some(Self {
            kind: parse_key(stem)?,
            part: 0,
        })
    }

    /// The canonical file name: lower-case fixed parts, the language label as stored
    /// (install-layout §2, §3.1).
    pub fn file_name(&self) -> String {
        if self.part == 0 {
            format!("client_{}.dat", self.kind)
        } else {
            format!("client_{}_aux_{}.datx", self.kind, self.part)
        }
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
    /// The language whose `client_local_<Lang>` archives DIDs routed to the language set
    /// are read from (the `--language` value). `None` picks the default of
    /// install-layout §3.5: `English` if present, otherwise the first label in
    /// lexicographic order. Archives of other languages stay open (see
    /// [`ArchiveSet::members`]) but are never searched by [`ArchiveSet::locate`].
    pub language: Option<String>,
}

impl Default for SetOptions {
    fn default() -> Self {
        Self {
            archive: ArchiveOptions::default(),
            routing: RoutingTable::default(),
            fallback_search_all: true,
            language: None,
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

/// A problem found while opening a set (unopenable file, missing aux part, ...). Opening
/// continues; [`ArchiveSet::validate`] reports each problem as an [`Issue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetProblem {
    /// The file concerned, if any.
    pub path: Option<PathBuf>,
    /// Description.
    pub message: String,
    /// How bad it is. [`Severity::Error`] means part of the set is unusable: an archive
    /// with a valid magic that cannot be opened, a missing part or a part-count mismatch
    /// (spec §2.3, §10).
    pub severity: Severity,
    /// Invariant number `n` of "I-n" (spec §10) the problem is reported under: the
    /// violated header invariant for an archive that could not be opened, otherwise 24.
    pub invariant: u8,
}

impl SetProblem {
    fn new(path: Option<PathBuf>, severity: Severity, invariant: u8, message: String) -> Self {
        Self {
            path,
            message,
            severity,
            invariant,
        }
    }
}

/// How an [`Archive::open`] failure is reported for a member of a set.
///
/// A file without the magic is not an archive (spec §2.1): a warning. A file whose magic
/// is valid but that cannot be used (I-4, I-6, I-8) or read is an error, because every
/// DID it holds is lost (§10: "the archive is unusable").
fn open_failure(e: &Error) -> (Severity, u8) {
    match e {
        Error::TooSmall { .. } => (Severity::Warning, 1),
        Error::BadMagic { .. } => (Severity::Warning, 2),
        Error::BadHeader { invariant, .. } => (Severity::Error, *invariant),
        _ => (Severity::Error, 24),
    }
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
    /// Set-level issues: archives that could not be opened (under the violated header
    /// invariant), I-24 (membership consistency, missing parts) and I-25 (duplicate DIDs).
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
    /// The selected language label, if the set has language archives.
    language: Option<String>,
    /// Per member: whether locate may search it (false for other languages' archives).
    routable: Vec<bool>,
}

impl ArchiveSet {
    /// Opens every archive directly inside `dir` whose name follows the archive-name
    /// grammar ([`ArchiveId::from_file_name`]; ASCII case-insensitive).
    ///
    /// Files whose magic is wrong or that cannot be opened are skipped and reported in
    /// [`ArchiveSet::problems`], as are membership inconsistencies such as a missing aux
    /// part. Fails only if the directory cannot be listed or holds no usable archive.
    ///
    /// When several languages are present, one is selected ([`SetOptions::language`]);
    /// the others stay members but are not searched by [`ArchiveSet::locate`].
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

        let mut problems = Vec::new();
        let found = Self::one_file_per_archive(found, &mut problems);
        let mut members = Vec::new();
        for (id, path) in found {
            match Archive::open_with(&path, options.archive.clone()) {
                Ok(archive) => members.push(SetMember { id, path, archive }),
                Err(e) => {
                    let (severity, invariant) = open_failure(&e);
                    tracing::warn!(path = %path.display(), error = %e, "skipping archive");
                    problems.push(SetProblem::new(
                        Some(path),
                        severity,
                        invariant,
                        format!("not opened: {e}"),
                    ));
                }
            }
        }
        if members.is_empty() {
            return Err(Error::NoArchives { path: dir });
        }
        let language = Self::select_language(&members, options.language.as_deref(), &mut problems);
        let routable = members
            .iter()
            .map(|m| match (&m.id.kind, &language) {
                (ArchiveKind::Local(l), Some(sel)) => l == sel,
                _ => true,
            })
            .collect();
        let mut set = Self {
            dir,
            members,
            routing: options.routing,
            fallback: options.fallback_search_all,
            problems,
            language,
            routable,
        };
        set.check_membership();
        Ok(set)
    }

    /// Keeps one file per archive when names differ only in case (possible on a
    /// case-sensitive file system): the canonical spelling if present, otherwise the
    /// lexicographically smallest name, with a warning (install-layout §2).
    fn one_file_per_archive(
        found: Vec<(ArchiveId, PathBuf)>,
        problems: &mut Vec<SetProblem>,
    ) -> Vec<(ArchiveId, PathBuf)> {
        let name_of = |p: &Path| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned()
        };
        let mut groups: BTreeMap<String, Vec<(ArchiveId, PathBuf)>> = BTreeMap::new();
        for (id, path) in found {
            groups
                .entry(id.file_name().to_ascii_lowercase())
                .or_default()
                .push((id, path));
        }
        let mut out = Vec::new();
        for (_, mut group) in groups {
            group.sort_by_key(|(_, p)| name_of(p));
            let keep = group
                .iter()
                .position(|(id, p)| name_of(p) == id.file_name())
                .unwrap_or(0);
            if keep < group.len() {
                let chosen = group.swap_remove(keep);
                for (_, p) in &group {
                    let message = format!(
                        "ignored: {} names the same archive as {} (names differ only in case)",
                        name_of(p),
                        name_of(&chosen.1)
                    );
                    tracing::warn!(path = %p.display(), "{message}");
                    problems.push(SetProblem::new(
                        Some(p.clone()),
                        Severity::Warning,
                        24,
                        message,
                    ));
                }
                out.push(chosen);
            }
        }
        out.sort();
        out
    }

    /// Chooses the language set (install-layout §3.5): the requested label if present
    /// (exact, else ASCII case-insensitive), otherwise `English`, otherwise the first label
    /// in lexicographic order. A requested label that is absent is reported.
    fn select_language(
        members: &[SetMember],
        requested: Option<&str>,
        problems: &mut Vec<SetProblem>,
    ) -> Option<String> {
        let langs: std::collections::BTreeSet<&str> = members
            .iter()
            .filter_map(|m| match &m.id.kind {
                ArchiveKind::Local(l) => Some(l.as_str()),
                _ => None,
            })
            .collect();
        let pick = |want: &str| {
            langs
                .get(want)
                .or_else(|| langs.iter().find(|l| l.eq_ignore_ascii_case(want)))
                .map(|l| (*l).to_owned())
        };
        if let Some(req) = requested {
            if let Some(l) = pick(req) {
                return Some(l);
            }
            let message = format!(
                "requested language {req} has no client_local_{req}.dat; using the default"
            );
            tracing::warn!("{message}");
            problems.push(SetProblem::new(None, Severity::Warning, 24, message));
        }
        pick("English").or_else(|| langs.first().map(|l| (*l).to_owned()))
    }

    /// Checks the set-membership records of every kind's parts (spec §2.3, §7.2).
    ///
    /// Disagreeing identifiers, wrong part indices, an all-zero identifier on an aux part
    /// and absent or unreadable records are I-24 warnings; an all-zero identifier on the
    /// base is valid (§2.3 step 3). A missing part and a part count that differs between parts are
    /// errors for that set: spec §2.3 asks for both to be reported as errors, and the
    /// DIDs stored in a missing part are unreachable.
    fn check_membership(&mut self) {
        // kind → (set id, part count) as declared by the first part that has a record.
        let mut declared: BTreeMap<ArchiveKind, ([u8; 16], u32)> = BTreeMap::new();
        let mut problems = Vec::new();
        let warn = |path: &Path, message: String| {
            SetProblem::new(Some(path.to_owned()), Severity::Warning, 24, message)
        };
        for m in &self.members {
            match m.archive.membership() {
                Ok(Some(ms)) => {
                    if ms.part_index != m.id.part {
                        problems.push(warn(
                            &m.path,
                            format!(
                                "membership record says part {}, file name says part {}",
                                ms.part_index, m.id.part
                            ),
                        ));
                    }
                    // Spec §2.3 step 3, §7.2: an all-zero identifier is valid on the base,
                    // and the set then adopts the aux parts' identifier; on an aux part it
                    // is never valid.
                    let zero_aux = m.id.part > 0 && ms.set_id == [0; 16];
                    if zero_aux {
                        problems.push(warn(
                            &m.path,
                            "aux part has an all-zero set identifier".into(),
                        ));
                    }
                    match declared.get(&m.id.kind) {
                        None => {
                            declared.insert(m.id.kind.clone(), (ms.set_id, ms.part_count));
                        }
                        Some(&(sid, count)) => {
                            if count != ms.part_count {
                                problems.push(SetProblem::new(
                                    Some(m.path.clone()),
                                    Severity::Error,
                                    24,
                                    format!(
                                        "part count {} disagrees with the {count} declared by \
                                         other parts of {}",
                                        ms.part_count, m.id.kind
                                    ),
                                ));
                            } else if sid == [0; 16] {
                                declared.insert(m.id.kind.clone(), (ms.set_id, count));
                            } else if sid != ms.set_id && !zero_aux {
                                problems.push(warn(
                                    &m.path,
                                    format!(
                                        "membership record disagrees with other parts of {}",
                                        m.id.kind
                                    ),
                                ));
                            }
                        }
                    }
                }
                Ok(None) => problems.push(warn(
                    &m.path,
                    "no set-membership record (0xFFFF0005)".into(),
                )),
                Err(e) => problems.push(warn(
                    &m.path,
                    format!("set-membership record unreadable: {e}"),
                )),
            }
        }
        for (kind, &(_, count)) in &declared {
            // `count` comes straight from the file: never enumerate an absurd value.
            if count > MAX_SET_PARTS {
                tracing::warn!(%kind, count, "implausible set part count");
                problems.push(SetProblem::new(
                    None,
                    Severity::Error,
                    24,
                    format!("implausible part count {count} for {kind} (limit {MAX_SET_PARTS})"),
                ));
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
            problems.push(SetProblem::new(None, Severity::Error, 24, message));
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

    /// The language whose archives serve DIDs routed to the language set, if the set has
    /// any language archive ([`SetOptions::language`], install-layout §3.5).
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Whether [`ArchiveSet::locate`] may search member `i`. Archives of a language other
    /// than [`ArchiveSet::language`] are never searched: there is no cross-locale fallback
    /// (string-table §10.1).
    fn is_routable(&self, i: usize) -> bool {
        self.routable.get(i).copied().unwrap_or(false)
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
                if hit && self.is_routable(i) && !out.contains(&i) {
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
            for i in
                (0..self.members.len()).filter(|&i| self.is_routable(i) && !routed.contains(&i))
            {
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
            // Other languages' archives hold the same DIDs by design and are never read
            // through the set, so they take no part in the duplicate check.
            if self.is_routable(i) {
                all.extend(dids.into_iter().map(|d| (d, i)));
            }
            report.archives.push((m.id.clone(), r));
        }
        for p in &self.problems {
            report.issues.push(Issue {
                invariant: p.invariant,
                severity: p.severity,
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
