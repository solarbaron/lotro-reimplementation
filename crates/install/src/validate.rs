// SPDX-License-Identifier: GPL-3.0-or-later
//! Validation of a candidate install directory.
//!
//! Spec-Ref: docs/specs/install-layout.md#4-validating-a-candidate-directory
//! Spec-Ref: docs/specs/install-layout.md#34-discovering-split-set-members
//! Spec-Ref: docs/specs/install-layout.md#37-game-type-identification
//! Spec-Ref: docs/specs/install-layout.md#7-projectversion

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::archive::{
    ArchiveFile, ArchiveKind, ArchiveStatus, MAX_AUX_INDEX, MIN_CORE_ARCHIVE_SIZE,
    parse_archive_name,
};
use crate::error::Error;
use crate::fsutil::{self, DirEntry};
use crate::launcherconfig;
use crate::records::SetMembership;

/// Largest member count a split set can plausibly have: the base plus aux
/// members up to [`MAX_AUX_INDEX`] (§3.4). A membership record claiming more
/// is treated as damaged.
pub const MAX_SPLIT_SET_COUNT: u32 = MAX_AUX_INDEX + 1;

/// Why a directory is not a usable install (§4).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationProblem {
    /// The path is not a directory.
    NotADirectory,
    /// The directory cannot be listed.
    CannotList(String),
    /// A core base archive (`general` or `gamelogic`) is missing.
    MissingCoreArchive(ArchiveKind),
    /// A core base archive exists but fails the header or size checks.
    CoreArchiveFailed {
        /// The archive kind.
        kind: ArchiveKind,
        /// The header check result.
        status: ArchiveStatus,
        /// The real file length.
        size: u64,
    },
    /// No `client_local_<Lang>.dat` exists.
    NoLanguageArchive,
    /// Language archives exist, but none passes the checks.
    NoUsableLanguageArchive,
    /// The directory holds a different game on the same engine (§3.7). Holds
    /// the identifying name that was seen.
    UnsupportedGame(String),
}

impl fmt::Display for ValidationProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory => f.write_str("not a directory"),
            Self::CannotList(e) => write!(f, "cannot list directory: {e}"),
            Self::MissingCoreArchive(kind) => write!(f, "missing client_{kind}.dat"),
            Self::CoreArchiveFailed { kind, status, size } => write!(
                f,
                "client_{kind}.dat failed checks ({status:?}, {size} bytes)"
            ),
            Self::NoLanguageArchive => f.write_str("no client_local_<Lang>.dat language archive"),
            Self::NoUsableLanguageArchive => {
                f.write_str("no language archive passes the header checks")
            }
            Self::UnsupportedGame(what) => write!(f, "unsupported game ({what})"),
        }
    }
}

/// A non-fatal validation finding (§4).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationWarning {
    /// An archive-named entry is not a regular file; ignored.
    NotRegularFile {
        /// The entry name.
        file: String,
    },
    /// An archive name repeats in a different case; ignored (§2).
    CaseDuplicate {
        /// The ignored entry name.
        file: String,
    },
    /// An archive fails the header checks.
    ArchiveStatus {
        /// The archive file name.
        file: String,
        /// The check result.
        status: ArchiveStatus,
    },
    /// An archive is 4 GiB or larger (none is in the reference install).
    FourGiBOrLarger {
        /// The archive file name.
        file: String,
    },
    /// A language archive is not offered because it fails the checks.
    UnusableLanguage {
        /// The language name.
        language: String,
        /// The archive file name.
        file: String,
    },
    /// `x64/lotroclient64.exe` is absent (not needed by this client).
    ClientBinaryMissing,
    /// The game type could not be determined (§3.7 step 4).
    GameTypeUnknown,
    /// No `cell`/`map` region archive exists.
    NoRegionArchives,
    /// The optional high-resolution archive is absent.
    HighresMissing,
    /// Region archives without their partner.
    UnpairedRegions(Vec<String>),
    /// Content-tier archive keys that are missing.
    MissingContent(Vec<String>),
    /// An aux file that is not part of a consecutive split set; ignored.
    AuxNotConsecutive {
        /// The archive file name.
        file: String,
    },
    /// The 0x14C kind word differs from the reference value (§3.2).
    KindWordDiffers {
        /// The archive file name.
        file: String,
        /// Value found.
        found: u32,
        /// Reference value.
        expected: u32,
    },
    /// Header 0x174 is not 112 (§9.6).
    Field174 {
        /// The archive file name.
        file: String,
        /// Value found.
        value: u32,
    },
    /// Header 0x178 does not match the archive family (§9.6).
    Field178 {
        /// The archive file name.
        file: String,
        /// Value found.
        value: u32,
        /// Family code from the kind word.
        family: u8,
    },
    /// A region archive's header code differs from its name.
    RegionCodeDiffers {
        /// The archive file name.
        file: String,
        /// Header value.
        found: u32,
        /// Region number from the name.
        expected: u32,
    },
    /// An aux member's kind word, code or block size differs from its base.
    AuxHeaderDiffers {
        /// The archive file name.
        file: String,
    },
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegularFile { file } => write!(f, "{file} is not a regular file"),
            Self::CaseDuplicate { file } => write!(
                f,
                "{file} duplicates another archive name in a different case; ignored"
            ),
            Self::ArchiveStatus { file, status } => write!(f, "{file}: {status:?}"),
            Self::FourGiBOrLarger { file } => write!(f, "{file} is 4 GiB or larger"),
            Self::UnusableLanguage { language, file } => write!(
                f,
                "language {language} not offered: {file} fails the archive checks"
            ),
            Self::ClientBinaryMissing => {
                f.write_str("x64/lotroclient64.exe not found (not needed by this client)")
            }
            Self::GameTypeUnknown => f.write_str("game type could not be determined"),
            Self::NoRegionArchives => f.write_str("no region (cell/map) archives found"),
            Self::HighresMissing => f.write_str("optional high-resolution archive not present"),
            Self::UnpairedRegions(r) => write!(f, "unpaired region archives: {}", r.join(", ")),
            Self::MissingContent(m) => write!(f, "content archives missing: {}", m.join(", ")),
            Self::AuxNotConsecutive { file } => {
                write!(f, "{file} is not part of a consecutive split set; ignored")
            }
            Self::KindWordDiffers {
                file,
                found,
                expected,
            } => write!(
                f,
                "{file}: kind word {found:#010x} differs from the reference value {expected:#010x}"
            ),
            Self::Field174 { file, value } => {
                write!(f, "{file}: header 0x174 = {value} (expected 112)")
            }
            Self::Field178 {
                file,
                value,
                family,
            } => write!(
                f,
                "{file}: header 0x178 = {value} does not match family {family}"
            ),
            Self::RegionCodeDiffers {
                file,
                found,
                expected,
            } => write!(f, "{file}: region code {found} != {expected}"),
            Self::AuxHeaderDiffers { file } => write!(
                f,
                "{file}: kind/code/block size differ from the base archive"
            ),
        }
    }
}

/// Which game a directory holds (§3.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameType {
    /// The game whose data files this project targets.
    Compatible,
    /// Another game on the same engine with the same archive naming. Found,
    /// but not supported. Holds the identifying name that was seen.
    Unsupported(String),
    /// Could not be decided. Accepted with a warning.
    Unknown,
}

/// The `ProjectVersion` file (§7). Metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectVersion {
    /// The file does not exist.
    Missing,
    /// The file exists but is not 1–10 ASCII digits.
    Malformed,
    /// The parsed value.
    Value(u64),
}

/// Status of a split set (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitSetStatus {
    /// All members verified through their membership records.
    Complete,
    /// The membership records name members that are not on disk.
    Incomplete {
        /// Missing member indices.
        missing: Vec<u32>,
    },
    /// Membership records were not available; every consecutive aux file
    /// found is taken as a member.
    Unverified,
}

/// A base archive and its aux members, read as one logical archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitSet {
    /// The archive kind of the base.
    pub kind: ArchiveKind,
    /// Member indices that belong to the set, ascending (0 is the base).
    pub members: Vec<u32>,
    /// Verification status.
    pub status: SplitSetStatus,
    /// Aux files that exist but are not members (stale leftovers).
    pub stale: Vec<u32>,
}

/// Verify a split set from its membership records (§3.4 steps 2–5).
///
/// `present` lists the member indices found on disk (0 for the base, then
/// consecutive aux indices). `records` gives, for each present index, the
/// decoded 0xFFFF0005 record, or `None` if it could not be read.
///
/// Record values come from archive data and are untrusted: a base record
/// whose count is 0 or larger than [`MAX_SPLIT_SET_COUNT`] is treated as
/// damaged (set unverified), so the work and the `missing` list stay bounded.
pub fn verify_split_set(
    kind: ArchiveKind,
    present: &[u32],
    records: &[(u32, Option<SetMembership>)],
) -> SplitSet {
    let unverified = |kind: ArchiveKind| SplitSet {
        kind,
        members: present.to_vec(),
        status: SplitSetStatus::Unverified,
        stale: Vec::new(),
    };
    let lookup = |i: u32| {
        records
            .iter()
            .find(|(idx, _)| *idx == i)
            .and_then(|(_, r)| *r)
    };
    let Some(base) = lookup(0) else {
        tracing::warn!(set = %kind, "no membership record for the base; split set unverified");
        return unverified(kind);
    };
    if base.index != 0 || base.count == 0 || base.count > MAX_SPLIT_SET_COUNT {
        tracing::warn!(set = %kind, index = base.index, count = base.count, "base membership record is inconsistent");
        return unverified(kind);
    }
    let mut members = vec![0u32];
    let mut stale = Vec::new();
    for &n in present.iter().filter(|&&n| n != 0) {
        let Some(rec) = lookup(n) else {
            tracing::warn!(set = %kind, member = n, "membership record unreadable; split set unverified");
            return unverified(kind);
        };
        if n >= base.count || rec.stamp != base.stamp || rec.index != n || rec.count != base.count {
            tracing::warn!(set = %kind, member = n, "stale aux member excluded from the set");
            stale.push(n);
        } else {
            members.push(n);
        }
    }
    let missing: Vec<u32> = (0..base.count).filter(|i| !members.contains(i)).collect();
    let status = if missing.is_empty() {
        SplitSetStatus::Complete
    } else {
        SplitSetStatus::Incomplete { missing }
    };
    SplitSet {
        kind,
        members,
        status,
        stale,
    }
}

/// The structured result of validating a directory (§4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// The directory validated.
    pub root: PathBuf,
    /// True when the core archives are present and pass the header checks.
    pub core_ok: bool,
    /// True when `core_ok` and the game type is compatible or unknown.
    pub usable: bool,
    /// The game type.
    pub game_type: GameType,
    /// Every archive found, sorted by lower-case file name.
    pub archives: Vec<ArchiveFile>,
    /// Split sets of the shared-data archives.
    pub split_sets: Vec<SplitSet>,
    /// Language names from `client_local_<Lang>.dat` archives that pass the
    /// core checks (§4 checks 2–3), sorted. Broken language archives are
    /// listed in `warnings` instead.
    pub languages: Vec<String>,
    /// Content-tier archive keys that are missing.
    pub missing_content: Vec<String>,
    /// Region archives without their partner (`cell_<r>` vs `map_<r>`).
    pub unpaired_regions: Vec<String>,
    /// The `x64` directory holding the 64-bit client, if present.
    pub client_dir: Option<PathBuf>,
    /// The 64-bit client binary, if present.
    pub client_exe: Option<PathBuf>,
    /// The `ProjectVersion` file.
    pub project_version: ProjectVersion,
    /// Reasons the directory is not usable (empty when usable).
    pub problems: Vec<ValidationProblem>,
    /// Non-fatal findings.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// True when content-tier archives are missing or unpaired, or a split set
    /// is known to be incomplete.
    pub fn incomplete(&self) -> bool {
        !self.missing_content.is_empty()
            || !self.unpaired_regions.is_empty()
            || self
                .split_sets
                .iter()
                .any(|s| matches!(s.status, SplitSetStatus::Incomplete { .. }))
    }
}

/// Read and parse `ProjectVersion` (§7).
pub fn read_project_version(dir: &Path) -> ProjectVersion {
    let Some(path) = fsutil::resolve_ci(dir, "ProjectVersion") else {
        return ProjectVersion::Missing;
    };
    // The whole file must be 1–10 digits plus whitespace, so anything over
    // 64 bytes is malformed, as is a non-regular file (FIFO, device,
    // directory) that merely has the name.
    if !fsutil::is_file(&path) {
        return ProjectVersion::Malformed;
    }
    let bytes = match fsutil::read_capped(&path, 64) {
        Ok(b) => b,
        Err(Error::TooLarge { .. }) => return ProjectVersion::Malformed,
        Err(e) => {
            tracing::debug!(error = %e, "cannot read ProjectVersion");
            return ProjectVersion::Missing;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let t = text.trim_matches(|c: char| c.is_ascii_whitespace());
    if (1..=10).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_digit()) {
        t.parse()
            .map_or(ProjectVersion::Malformed, ProjectVersion::Value)
    } else {
        ProjectVersion::Malformed
    }
}

/// Decide the game type of `dir` (§3.7). An unlistable directory yields
/// [`GameType::Unknown`].
pub fn detect_game_type(dir: &Path) -> GameType {
    let entries = fsutil::list_dir(dir).unwrap_or_default();
    detect_game_type_in(dir, &entries)
}

/// Map a `DataCenter.GameName` value to a game type (§3.7 step 2).
fn game_type_from_name(file: &str, name: &str) -> Option<GameType> {
    let n = name.trim();
    if n.eq_ignore_ascii_case("LOTRO") || n.eq_ignore_ascii_case("LOTROEU") {
        Some(GameType::Compatible)
    } else if n.eq_ignore_ascii_case("DDO") {
        Some(GameType::Unsupported(format!(
            "{file}: {}={n}",
            launcherconfig::KEY_GAME_NAME
        )))
    } else {
        None
    }
}

fn is_sibling_client(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.starts_with("dndclient") && l.ends_with(".exe")
}

fn detect_game_type_in(dir: &Path, entries: &[DirEntry]) -> GameType {
    // Step 1: the launcher config's file stem.
    for e in entries {
        let lower = e.name.to_ascii_lowercase();
        if let Some(stem) = lower.strip_suffix(".launcherconfig") {
            match stem {
                "lotro" => return GameType::Compatible,
                "ddo" => return GameType::Unsupported(e.name.clone()),
                _ => {}
            }
        }
    }
    // Step 2: `DataCenter.GameName` in the first launcher config (P14 order)
    // that carries one.
    for path in launcherconfig::launcher_config_candidates(entries, None) {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let cfg = match launcherconfig::read_launcher_config(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(error = %e, "launcher config unreadable; trying the next");
                continue;
            }
        };
        if let Some(name) = cfg.game_name() {
            if let Some(t) = game_type_from_name(&file, name) {
                return t;
            }
            tracing::debug!(file, name, "unrecognised game name in launcher config");
            break;
        }
    }
    // Step 3: the client binary present (install root or x64/).
    let x64_entries = fsutil::resolve_ci(dir, "x64")
        .filter(|p| fsutil::is_dir(p))
        .and_then(|p| fsutil::list_dir(&p).ok())
        .unwrap_or_default();
    let regular = |e: &&DirEntry| fsutil::is_file(&e.path);
    if fsutil::pick_ci(&x64_entries, "lotroclient64.exe")
        .filter(regular)
        .is_some()
        || fsutil::pick_ci(entries, "lotroclient.exe")
            .filter(regular)
            .is_some()
    {
        return GameType::Compatible;
    }
    if let Some(e) = entries
        .iter()
        .chain(&x64_entries)
        .find(|e| is_sibling_client(&e.name) && fsutil::is_file(&e.path))
    {
        return GameType::Unsupported(e.name.clone());
    }
    // Step 4.
    GameType::Unknown
}

/// Validate a candidate install directory (§4). Never fails: every problem is
/// recorded in the report. Reads only directory listings, the first bytes of
/// each archive and `ProjectVersion`.
pub fn validate(dir: &Path) -> ValidationReport {
    let mut report = ValidationReport {
        root: dir.to_path_buf(),
        core_ok: false,
        usable: false,
        game_type: GameType::Unknown,
        archives: Vec::new(),
        split_sets: Vec::new(),
        languages: Vec::new(),
        missing_content: Vec::new(),
        unpaired_regions: Vec::new(),
        client_dir: None,
        client_exe: None,
        project_version: ProjectVersion::Missing,
        problems: Vec::new(),
        warnings: Vec::new(),
    };
    if !fsutil::is_dir(dir) {
        report.problems.push(ValidationProblem::NotADirectory);
        return report;
    }
    let entries = match fsutil::list_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            report
                .problems
                .push(ValidationProblem::CannotList(e.to_string()));
            return report;
        }
    };

    collect_archives(&entries, &mut report);
    check_core(&mut report);
    check_content(&mut report);
    build_split_sets(&mut report);
    check_invariants(&mut report);

    report.game_type = detect_game_type_in(dir, &entries);
    if let Some(x64) = fsutil::resolve_ci(dir, "x64").filter(|p| fsutil::is_dir(p))
        && let Some(exe) =
            fsutil::resolve_ci(&x64, "lotroclient64.exe").filter(|p| fsutil::is_file(p))
    {
        report.client_exe = Some(exe);
        report.client_dir = Some(x64);
    }
    if report.client_exe.is_none() {
        report.warnings.push(ValidationWarning::ClientBinaryMissing);
    }
    report.project_version = read_project_version(dir);

    match &report.game_type {
        GameType::Compatible => {}
        GameType::Unknown => report.warnings.push(ValidationWarning::GameTypeUnknown),
        GameType::Unsupported(what) => report
            .problems
            .push(ValidationProblem::UnsupportedGame(what.clone())),
    }
    report.usable = report.core_ok && !matches!(report.game_type, GameType::Unsupported(_));
    for w in &report.warnings {
        tracing::debug!(dir = %dir.display(), warning = %w, "validation warning");
    }
    report
}

fn collect_archives(entries: &[DirEntry], report: &mut ValidationReport) {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for e in entries {
        match parse_archive_name(&e.name) {
            Some(name) => files.push((e, name)),
            None => {
                if e.name
                    .get(..7)
                    .is_some_and(|p| p.eq_ignore_ascii_case("client_"))
                {
                    tracing::debug!(name = %e.name, "ignoring non-archive client_ file");
                }
            }
        }
    }
    // Case-insensitive duplicates (§2): the exact (canonical) spelling wins,
    // otherwise the lexicographically smallest.
    files.sort_by(|(a, an), (b, bn)| {
        (a.name != an.file_name(), &a.name).cmp(&(b.name != bn.file_name(), &b.name))
    });
    for (e, name) in files {
        if !fsutil::is_file(&e.path) {
            report.warnings.push(ValidationWarning::NotRegularFile {
                file: e.name.clone(),
            });
            continue;
        }
        if !seen.insert(name.file_name().to_ascii_lowercase()) {
            report.warnings.push(ValidationWarning::CaseDuplicate {
                file: e.name.clone(),
            });
            continue;
        }
        report.archives.push(ArchiveFile::inspect(&e.path, name));
    }
    report
        .archives
        .sort_by_key(|a| a.file_name().to_ascii_lowercase());
    for a in &report.archives {
        if !a.status.is_ok() {
            report.warnings.push(ValidationWarning::ArchiveStatus {
                file: a.file_name(),
                status: a.status.clone(),
            });
        }
        if a.size >= 1u64 << 32 {
            report.warnings.push(ValidationWarning::FourGiBOrLarger {
                file: a.file_name(),
            });
        }
    }
    // Only language archives that pass the core checks are offered (§3.5,
    // §4): a broken one must never be chosen as the default language.
    let mut languages = Vec::new();
    for a in &report.archives {
        let ArchiveKind::Local(l) = &a.kind else {
            continue;
        };
        if core_archive_ok(a) {
            languages.push(l.clone());
        } else {
            report.warnings.push(ValidationWarning::UnusableLanguage {
                language: l.clone(),
                file: a.file_name(),
            });
        }
    }
    languages.sort();
    report.languages = languages;
}

fn core_archive_ok(a: &ArchiveFile) -> bool {
    a.aux_index == 0 && a.size >= MIN_CORE_ARCHIVE_SIZE && a.status.is_ok()
}

fn check_core(report: &mut ValidationReport) {
    let mut ok = true;
    for kind in [ArchiveKind::General, ArchiveKind::Gamelogic] {
        match report
            .archives
            .iter()
            .find(|a| a.kind == kind && a.aux_index == 0)
        {
            None => {
                ok = false;
                report
                    .problems
                    .push(ValidationProblem::MissingCoreArchive(kind));
            }
            Some(a) if !core_archive_ok(a) => {
                ok = false;
                report.problems.push(ValidationProblem::CoreArchiveFailed {
                    kind,
                    status: a.status.clone(),
                    size: a.size,
                });
            }
            Some(_) => {}
        }
    }
    let locals: Vec<&ArchiveFile> = report
        .archives
        .iter()
        .filter(|a| matches!(a.kind, ArchiveKind::Local(_)))
        .collect();
    if locals.is_empty() {
        ok = false;
        report.problems.push(ValidationProblem::NoLanguageArchive);
    } else if !locals.iter().any(|a| core_archive_ok(a)) {
        ok = false;
        report
            .problems
            .push(ValidationProblem::NoUsableLanguageArchive);
    }
    report.core_ok = ok;
}

fn check_content(report: &mut ValidationReport) {
    let has = |k: &ArchiveKind| {
        report
            .archives
            .iter()
            .any(|a| &a.kind == k && a.aux_index == 0)
    };
    let missing: Vec<String> = [
        ArchiveKind::Mesh,
        ArchiveKind::Surface,
        ArchiveKind::Sound,
        ArchiveKind::Anim,
    ]
    .into_iter()
    .filter(|k| !has(k))
    .map(|k| k.key())
    .collect();
    let mut unpaired = Vec::new();
    for a in &report.archives {
        let partner = match a.kind {
            ArchiveKind::Cell(r) => ArchiveKind::Map(r),
            ArchiveKind::Map(r) => ArchiveKind::Cell(r),
            _ => continue,
        };
        if !has(&partner) {
            unpaired.push(a.kind.key());
        }
    }
    report.missing_content = missing;
    report.unpaired_regions = unpaired;
    let any_region = report
        .archives
        .iter()
        .any(|a| matches!(a.kind, ArchiveKind::Cell(_) | ArchiveKind::Map(_)));
    if !any_region {
        report.warnings.push(ValidationWarning::NoRegionArchives);
    }
    if !has(&ArchiveKind::Highres) {
        report.warnings.push(ValidationWarning::HighresMissing);
    }
    if !report.unpaired_regions.is_empty() {
        report.warnings.push(ValidationWarning::UnpairedRegions(
            report.unpaired_regions.clone(),
        ));
    }
    if !report.missing_content.is_empty() {
        report.warnings.push(ValidationWarning::MissingContent(
            report.missing_content.clone(),
        ));
    }
}

fn build_split_sets(report: &mut ValidationReport) {
    let bases: Vec<ArchiveKind> = report
        .archives
        .iter()
        .filter(|a| a.aux_index == 0 && a.kind.may_have_aux())
        .map(|a| a.kind.clone())
        .collect();
    let mut sets = Vec::new();
    for kind in bases {
        let mut members = vec![0u32];
        for n in 1..=MAX_AUX_INDEX {
            if report
                .archives
                .iter()
                .any(|a| a.kind == kind && a.aux_index == n)
            {
                members.push(n);
            } else {
                break;
            }
        }
        sets.push(SplitSet {
            kind,
            members,
            status: SplitSetStatus::Unverified,
            stale: Vec::new(),
        });
    }
    // Aux files that were not reached by the consecutive probe.
    for a in report.archives.iter().filter(|a| a.aux_index > 0) {
        let reached = sets
            .iter()
            .any(|s| s.kind == a.kind && s.members.contains(&a.aux_index));
        if !reached {
            report.warnings.push(ValidationWarning::AuxNotConsecutive {
                file: a.file_name(),
            });
        }
    }
    report.split_sets = sets;
}

/// Header invariants that can be checked without the archive directory
/// (§9.6 items 2–4 and the observed kind words of §3.2). Warnings only.
fn check_invariants(report: &mut ValidationReport) {
    let mut warnings = Vec::new();
    for a in &report.archives {
        let Some(h) = &a.header else { continue };
        let name = a.file_name();
        if let Some(expected) = a.kind.observed_kind_word()
            && h.kind_word != expected
        {
            warnings.push(ValidationWarning::KindWordDiffers {
                file: name.clone(),
                found: h.kind_word,
                expected,
            });
        }
        if h.field_174 != 112 {
            warnings.push(ValidationWarning::Field174 {
                file: name.clone(),
                value: h.field_174,
            });
        }
        let region = h.family() == 2;
        if region != (h.field_178 == 0) {
            warnings.push(ValidationWarning::Field178 {
                file: name.clone(),
                value: h.field_178,
                family: h.family(),
            });
        }
        if let ArchiveKind::Cell(r) | ArchiveKind::Map(r) = a.kind
            && h.sub_code != r
        {
            warnings.push(ValidationWarning::RegionCodeDiffers {
                file: name.clone(),
                found: h.sub_code,
                expected: r,
            });
        }
        if a.aux_index > 0 {
            let base = report
                .archives
                .iter()
                .find(|b| b.kind == a.kind && b.aux_index == 0)
                .and_then(|b| b.header.as_ref());
            if let Some(b) = base
                && (b.kind_word, b.sub_code, b.block_size)
                    != (h.kind_word, h.sub_code, h.block_size)
            {
                warnings.push(ValidationWarning::AuxHeaderDiffers { file: name.clone() });
            }
        }
    }
    report.warnings.extend(warnings);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(stamp: u8, index: u32, count: u32) -> Option<SetMembership> {
        Some(SetMembership {
            stamp: [stamp; 16],
            index,
            count,
        })
    }

    #[test]
    fn split_set_complete_incomplete_stale() {
        let s = verify_split_set(
            ArchiveKind::Surface,
            &[0, 1, 2],
            &[(0, m(1, 0, 3)), (1, m(1, 1, 3)), (2, m(1, 2, 3))],
        );
        assert_eq!(s.status, SplitSetStatus::Complete);
        assert_eq!(s.members, vec![0, 1, 2]);

        let s = verify_split_set(
            ArchiveKind::Highres,
            &[0, 1],
            &[(0, m(1, 0, 4)), (1, m(1, 1, 4))],
        );
        assert_eq!(
            s.status,
            SplitSetStatus::Incomplete {
                missing: vec![2, 3]
            }
        );

        let s = verify_split_set(
            ArchiveKind::Mesh,
            &[0, 1, 2],
            &[(0, m(1, 0, 2)), (1, m(1, 1, 2)), (2, m(9, 2, 3))],
        );
        assert_eq!(s.status, SplitSetStatus::Complete);
        assert_eq!(s.stale, vec![2]);

        let s = verify_split_set(ArchiveKind::Mesh, &[0, 1], &[(0, None), (1, m(1, 1, 2))]);
        assert_eq!(s.status, SplitSetStatus::Unverified);
        assert_eq!(s.members, vec![0, 1]);
    }

    #[test]
    fn project_version() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Missing);
        std::fs::write(t.path().join("ProjectVersion"), b"120").unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Value(120));
        std::fs::write(t.path().join("ProjectVersion"), b" 7\r\n").unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Value(7));
        std::fs::write(t.path().join("ProjectVersion"), b"12a").unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Malformed);
        std::fs::write(t.path().join("ProjectVersion"), b"12345678901").unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Malformed);
        // Garbage after byte 64 must not be hidden by a short read.
        let mut long = b"123".to_vec();
        long.extend([b' '; 61]);
        long.extend(b"garbage");
        std::fs::write(t.path().join("ProjectVersion"), &long).unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Malformed);
        // Exactly 64 bytes of digits and whitespace is still fine.
        let mut ok = b"120".to_vec();
        ok.extend([b' '; 61]);
        std::fs::write(t.path().join("ProjectVersion"), &ok).unwrap();
        assert_eq!(read_project_version(t.path()), ProjectVersion::Value(120));
    }

    /// Hostile membership counts must not drive allocation or work.
    #[test]
    fn split_set_rejects_implausible_counts() {
        for count in [MAX_SPLIT_SET_COUNT + 1, 50_000_000, u32::MAX] {
            let s = verify_split_set(
                ArchiveKind::Mesh,
                &[0, 1],
                &[(0, m(1, 0, count)), (1, m(1, 1, count))],
            );
            assert_eq!(s.status, SplitSetStatus::Unverified, "count {count}");
            assert_eq!(s.members, vec![0, 1]);
        }
        // The largest plausible count still verifies normally.
        let s = verify_split_set(
            ArchiveKind::Mesh,
            &[0, 1],
            &[
                (0, m(1, 0, MAX_SPLIT_SET_COUNT)),
                (1, m(1, 1, MAX_SPLIT_SET_COUNT)),
            ],
        );
        let SplitSetStatus::Incomplete { missing } = s.status else {
            panic!("expected incomplete, got {:?}", s.status);
        };
        assert_eq!(missing.len(), MAX_SPLIT_SET_COUNT as usize - 2);
        // A record parsed from hostile bytes goes through the same path.
        let mut b = vec![7u8; 16];
        b.extend(0u32.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        let rec = SetMembership::parse(&b).unwrap();
        let s = verify_split_set(ArchiveKind::Mesh, &[0], &[(0, Some(rec))]);
        assert_eq!(s.status, SplitSetStatus::Unverified);
    }

    #[cfg(unix)]
    #[test]
    fn project_version_fifo_does_not_hang() {
        let t = tempfile::tempdir().unwrap();
        if !crate::fsutil::tests::make_fifo(&t.path().join("ProjectVersion")) {
            return;
        }
        let dir = t.path().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(read_project_version(&dir));
        });
        let v = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("read_project_version blocked on a FIFO");
        assert_eq!(v, ProjectVersion::Malformed);
    }

    #[test]
    fn game_type_from_launcher_config_and_x64_binaries() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path();
        assert_eq!(detect_game_type(d), GameType::Unknown);

        // Step 3 looks in x64/ for the sibling game's client too.
        std::fs::create_dir_all(d.join("x64")).unwrap();
        std::fs::write(d.join("x64/dndclient64.exe"), b"MZ").unwrap();
        assert!(matches!(detect_game_type(d), GameType::Unsupported(n) if n == "dndclient64.exe"));

        // Step 2 wins over step 3: P14 legacy config name, compatible value.
        let cfg = |v: &str| {
            format!(
                "<configuration><appSettings><add key=\"DataCenter.GameName\" value=\"{v}\"/></appSettings></configuration>"
            )
        };
        std::fs::write(d.join("TurbineLauncher.exe.config"), cfg("LOTROEU")).unwrap();
        assert_eq!(detect_game_type(d), GameType::Compatible);
        std::fs::write(d.join("TurbineLauncher.exe.config"), cfg("DDO")).unwrap();
        assert!(matches!(detect_game_type(d), GameType::Unsupported(_)));
        // An arbitrary *.launcherconfig comes before TurbineLauncher.exe.config.
        std::fs::write(d.join("custom.launcherconfig"), cfg("LOTRO")).unwrap();
        assert_eq!(detect_game_type(d), GameType::Compatible);
        // A config with a DTD is ignored, falling through to the next one.
        std::fs::write(
            d.join("custom.launcherconfig"),
            "<!DOCTYPE x><configuration/>",
        )
        .unwrap();
        assert!(matches!(detect_game_type(d), GameType::Unsupported(_)));
        // Step 1 still wins over everything.
        std::fs::write(d.join("lotro.launcherconfig"), "").unwrap();
        assert_eq!(detect_game_type(d), GameType::Compatible);
    }

    proptest::proptest! {
        /// Arbitrary (hostile) membership records: bounded output, no panic.
        #[test]
        fn split_set_bounded_for_any_records(
            recs in proptest::collection::vec(
                (0u8..3, proptest::num::u32::ANY, proptest::num::u32::ANY, proptest::bool::ANY),
                1..=(MAX_SPLIT_SET_COUNT as usize),
            )
        ) {
            let present: Vec<u32> = (0..recs.len() as u32).collect();
            let records: Vec<(u32, Option<SetMembership>)> = recs
                .iter()
                .enumerate()
                .map(|(i, &(st, idx, cnt, some))| {
                    (i as u32, some.then_some(SetMembership { stamp: [st; 16], index: idx, count: cnt }))
                })
                .collect();
            let s = verify_split_set(ArchiveKind::Sound, &present, &records);
            let bound = MAX_SPLIT_SET_COUNT as usize;
            proptest::prop_assert!(s.members.len() <= bound);
            if let SplitSetStatus::Incomplete { missing } = &s.status {
                proptest::prop_assert!(missing.len() <= bound);
            }
        }
    }
}
