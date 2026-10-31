// SPDX-License-Identifier: GPL-3.0-or-later
//! Local per-install census baselines (decision 0045); see the crate documentation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::version::{check_outside_work_trees, install_version, is_version_name};

/// Base directory variable.
const ENV_DIR: &str = "TESSERA_CENSUS_DIR";
/// Fail on a difference from a spec value.
const ENV_STRICT: &str = "TESSERA_CENSUS_STRICT";
/// Rewrite this install version's baseline.
const ENV_REBASE: &str = "TESSERA_CENSUS_REBASE";

/// Largest baseline file read back (far above any census).
const MAX_BASELINE_BYTES: u64 = 16 * 1024 * 1024;
/// Longest count name.
const MAX_NAME_LEN: usize = 256;

/// One counted value. Every primitive integer converts into it, so a test passes its
/// census fields and spec literals as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Count(pub i128);

macro_rules! count_from {
    ($($t:ty),*) => {$(
        impl From<$t> for Count {
            fn from(v: $t) -> Self {
                Self(i128::from(v))
            }
        }
    )*};
}
count_from!(u8, u16, u32, u64, i8, i16, i32, i64);

impl From<usize> for Count {
    fn from(v: usize) -> Self {
        // usize is at most 64 bits on every supported target.
        Self(i128::try_from(v).unwrap_or(i128::MAX))
    }
}

impl From<isize> for Count {
    fn from(v: isize) -> Self {
        Self(i128::try_from(v).unwrap_or(i128::MAX))
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Where baselines live and the two switches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The census base directory.
    pub base: PathBuf,
    /// `TESSERA_CENSUS_STRICT=1`: a value differing from its spec value fails.
    pub strict: bool,
    /// `TESSERA_CENSUS_REBASE=1`: rewrite this install version's baseline.
    pub rebase: bool,
}

impl Options {
    /// Options with `base` and both switches off.
    #[must_use]
    pub fn at(base: impl Into<PathBuf>) -> Self {
        Self {
            base: base.into(),
            strict: false,
            rebase: false,
        }
    }

    /// The options of the environment: [`default_base`], `TESSERA_CENSUS_STRICT=1`,
    /// `TESSERA_CENSUS_REBASE=1`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            base: default_base(),
            strict: flag(ENV_STRICT),
            rebase: flag(ENV_REBASE),
        }
    }
}

fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v == "1")
}

/// `${TESSERA_CENSUS_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/census}`.
#[must_use]
pub fn default_base() -> PathBuf {
    if let Some(d) = std::env::var_os(ENV_DIR).filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    cache.join("tessera").join("census")
}

/// A census failure or a refused census.
#[derive(Debug, thiserror::Error)]
pub enum CensusError {
    /// The census or drift directory lies in a git work tree.
    #[error(
        "census directory {} is inside a git work tree ({}); refused (decision 0045)",
        dir.display(),
        repo.display()
    )]
    InsideWorkTree {
        /// The refused directory.
        dir: PathBuf,
        /// The work tree found.
        repo: PathBuf,
    },
    /// A package, test or function name that cannot be a path component.
    #[error("census {what} {value:?} is empty or holds a separator")]
    InvalidComponent {
        /// `package`, `test` or `function`.
        what: &'static str,
        /// The value given.
        value: String,
    },
    /// A version not named like an install version.
    #[error("{0:?} is not an install version (16 hex digits, optionally after `schema-`)")]
    InvalidVersion(String),
    /// The install version of the game directory could not be computed.
    #[error("the install version of {} cannot be computed", .0.display())]
    NoVersion(PathBuf),
    /// An I/O failure.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The failure.
        #[source]
        source: std::io::Error,
    },
    /// A baseline file that is not one this crate writes.
    #[error(
        "{}:{line}: {reason}; delete it or rerun with TESSERA_CENSUS_REBASE=1",
        path.display()
    )]
    BadBaseline {
        /// The baseline.
        path: PathBuf,
        /// The 1-based line (0: the whole file).
        line: usize,
        /// What is wrong.
        reason: &'static str,
    },
    /// The census did not pass.
    #[error("{0}")]
    Failed(Box<Failure>),
}

/// One row of a comparison: a name, the value it was compared with and the observed one
/// (`None` for a name on one side only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The count's name.
    pub name: String,
    /// The baseline or spec value.
    pub expected: Option<Count>,
    /// The observed value.
    pub observed: Option<Count>,
}

/// Why a census failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// `package/test/function`.
    pub census: String,
    /// The install version.
    pub version: String,
    /// This version's baseline.
    pub baseline: PathBuf,
    /// Names that are not valid or were recorded twice (nothing was compared or written).
    pub invalid: Vec<String>,
    /// Differences from the baseline: missing, extra and differing names.
    pub baseline_mismatch: Vec<Row>,
    /// With `TESSERA_CENSUS_STRICT=1`: values differing from their spec values.
    pub spec_mismatch: Vec<Row>,
}

impl Failure {
    fn is_empty(&self) -> bool {
        self.invalid.is_empty()
            && self.baseline_mismatch.is_empty()
            && self.spec_mismatch.is_empty()
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "census {} (install {})", self.census, self.version)?;
        if !self.invalid.is_empty() {
            write!(f, ": {} invalid names:", self.invalid.len())?;
            for n in &self.invalid {
                write!(f, "\n  {n}")?;
            }
            return Ok(());
        }
        if !self.baseline_mismatch.is_empty() {
            write!(
                f,
                ": {} counts differ from the baseline {}\n{}\
                 (accept a change with TESSERA_CENSUS_REBASE=1)",
                self.baseline_mismatch.len(),
                self.baseline.display(),
                table(
                    &["name", "baseline", "observed"],
                    self.baseline_mismatch.iter().map(|r| vec![
                        r.name.clone(),
                        show(r.expected),
                        show(r.observed)
                    ]),
                )
            )?;
        }
        if !self.spec_mismatch.is_empty() {
            write!(
                f,
                "{}{} counts differ from the spec (TESSERA_CENSUS_STRICT=1)\n{}",
                if self.baseline_mismatch.is_empty() {
                    ": "
                } else {
                    "\n"
                },
                self.spec_mismatch.len(),
                table(
                    &["name", "spec", "observed"],
                    self.spec_mismatch.iter().map(|r| vec![
                        r.name.clone(),
                        show(r.expected),
                        show(r.observed)
                    ]),
                )
            )?;
        }
        Ok(())
    }
}

/// What a new baseline's drift report was compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Against {
    /// The baseline of this earlier install version.
    Previous(String),
    /// The spec values the test gave (no earlier baseline).
    Spec,
}

/// One drifted count of a new baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftRow {
    /// The count's name.
    pub name: String,
    /// The previous version's value (`None`: new name, or compared with the spec).
    pub previous: Option<Count>,
    /// The observed value (`None`: the name is gone).
    pub observed: Option<Count>,
    /// The spec value the test gave.
    pub spec: Option<Count>,
}

/// What a passing census did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every count matched this version's baseline.
    Matched {
        /// The baseline.
        baseline: PathBuf,
        /// How many counts.
        counts: usize,
    },
    /// No baseline existed: it was written.
    Created {
        /// The new baseline.
        baseline: PathBuf,
        /// How many counts.
        counts: usize,
        /// What the drift was computed against.
        against: Against,
        /// The drifted counts.
        drift: Vec<DriftRow>,
        /// The drift file, when there was drift.
        drift_file: Option<PathBuf>,
    },
    /// `TESSERA_CENSUS_REBASE=1` rewrote the baseline.
    Rebased {
        /// The baseline.
        baseline: PathBuf,
        /// How many counts.
        counts: usize,
        /// The counts that changed (expected: the replaced baseline).
        changed: Vec<Row>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: String,
    observed: Count,
    spec: Option<Count>,
}

/// The counts of one test function on one install version. Record every count with
/// [`Self::count`] or [`Self::count_vs_spec`], then call [`Self::finish`]. Dropping an
/// unfinished census panics (unless the thread is already panicking), so a forgotten
/// `finish` cannot pass silently.
#[derive(Debug)]
pub struct Census {
    package: String,
    test: String,
    function: String,
    version: String,
    options: Options,
    baseline: PathBuf,
    drift_file: PathBuf,
    entries: Vec<Entry>,
    seen: BTreeSet<String>,
    invalid: Vec<String>,
    finished: bool,
}

fn component(what: &'static str, value: &str) -> Result<String, CensusError> {
    let bad = value.is_empty()
        || value == "."
        || value == ".."
        || value
            .chars()
            .any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control());
    if bad {
        Err(CensusError::InvalidComponent {
            what,
            value: value.to_owned(),
        })
    } else {
        Ok(value.to_owned())
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && !name.starts_with('#')
        && name != "-"
        && !name.chars().any(char::is_control)
}

impl Census {
    /// Opens the census of `package`/`test`/`function` for the install at `game_dir`, with
    /// [`Options::from_env`]. `test` is the test binary's crate name (see [`crate::census!`]).
    ///
    /// # Errors
    /// No install version, invalid components, or a directory inside a git work tree.
    pub fn open(
        package: &str,
        test: &str,
        function: &str,
        game_dir: &Path,
    ) -> Result<Self, CensusError> {
        let version =
            install_version(game_dir).ok_or_else(|| CensusError::NoVersion(game_dir.into()))?;
        Self::open_with(package, test, function, &version, Options::from_env())
    }

    /// Opens a census for an install version the caller computed, with explicit options.
    ///
    /// # Errors
    /// Invalid components or version, or a directory inside a git work tree.
    pub fn open_with(
        package: &str,
        test: &str,
        function: &str,
        version: &str,
        options: Options,
    ) -> Result<Self, CensusError> {
        let package = component("package", package)?;
        let test = component("test", test)?;
        let function = component("function", function)?;
        if !is_version_name(version) {
            return Err(CensusError::InvalidVersion(version.to_owned()));
        }
        let dir = options.base.join(&package).join(&test).join(&function);
        check_outside_work_trees(&dir)?;
        let drift_dir = options.base.join("drift").join(version);
        check_outside_work_trees(&drift_dir)?;
        Ok(Self {
            baseline: dir.join(format!("{version}.tsv")),
            drift_file: drift_dir
                .join(&package)
                .join(&test)
                .join(format!("{function}.tsv")),
            package,
            test,
            function,
            version: version.to_owned(),
            options,
            entries: Vec::new(),
            seen: BTreeSet::new(),
            invalid: Vec::new(),
            finished: false,
        })
    }

    /// The install version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// This version's baseline file.
    #[must_use]
    pub fn baseline_path(&self) -> &Path {
        &self.baseline
    }

    /// Records a count the spec states no number for.
    pub fn count(&mut self, name: &str, observed: impl Into<Count>) {
        self.push(name, observed.into(), None);
    }

    /// Records a count with the number a spec table states for it (kept in the test; it is
    /// shown in drift reports and checked under `TESSERA_CENSUS_STRICT=1`).
    pub fn count_vs_spec(
        &mut self,
        name: &str,
        observed: impl Into<Count>,
        spec: impl Into<Count>,
    ) {
        self.push(name, observed.into(), Some(spec.into()));
    }

    fn push(&mut self, name: &str, observed: Count, spec: Option<Count>) {
        if !valid_name(name) {
            self.invalid.push(format!("not a valid name: {name:?}"));
            return;
        }
        if !self.seen.insert(name.to_owned()) {
            self.invalid.push(format!("recorded twice: {name}"));
            return;
        }
        self.entries.push(Entry {
            name: name.to_owned(),
            observed,
            spec,
        });
    }

    fn label(&self) -> String {
        format!("{}/{}/{}", self.package, self.test, self.function)
    }

    /// Checks or writes the baseline and panics with the failure table when the census does
    /// not pass.
    ///
    /// # Panics
    /// When [`Self::try_finish`] returns an error.
    #[allow(clippy::must_use_candidate)]
    pub fn finish(self) -> Outcome {
        match self.try_finish() {
            Ok(o) => o,
            Err(e) => panic!("{e}"),
        }
    }

    /// Checks or writes the baseline (see the crate documentation).
    ///
    /// # Errors
    /// [`CensusError::Failed`] when a count differs, a name is missing, extra or invalid, or
    /// (strict) a value differs from its spec value; I/O and baseline-format errors.
    pub fn try_finish(mut self) -> Result<Outcome, CensusError> {
        self.finished = true;
        let entries = std::mem::take(&mut self.entries);
        let mut failure = Failure {
            census: self.label(),
            version: self.version.clone(),
            baseline: self.baseline.clone(),
            invalid: std::mem::take(&mut self.invalid),
            baseline_mismatch: Vec::new(),
            spec_mismatch: Vec::new(),
        };
        if !failure.invalid.is_empty() {
            return Err(CensusError::Failed(Box::new(failure)));
        }
        self.note_last_version();
        let existing = read_baseline(&self.baseline)?;
        let outcome = match existing {
            Some(old) if !self.options.rebase => {
                failure.baseline_mismatch = compare(&old, &entries);
                if failure.baseline_mismatch.is_empty() {
                    eprintln!(
                        "census: {} counts match the baseline {}",
                        entries.len(),
                        self.baseline.display()
                    );
                }
                Outcome::Matched {
                    baseline: self.baseline.clone(),
                    counts: entries.len(),
                }
            }
            Some(old) => {
                write_baseline(&self.baseline, &self.header(), &entries)?;
                let changed = compare(&old, &entries);
                eprintln!(
                    "census: rebased {} ({} counts, {} changed)",
                    self.baseline.display(),
                    entries.len(),
                    changed.len()
                );
                if !changed.is_empty() {
                    eprint!(
                        "{}",
                        table(
                            &["name", "old baseline", "observed"],
                            changed.iter().map(|r| vec![
                                r.name.clone(),
                                show(r.expected),
                                show(r.observed)
                            ]),
                        )
                    );
                }
                Outcome::Rebased {
                    baseline: self.baseline.clone(),
                    counts: entries.len(),
                    changed,
                }
            }
            None => self.create(&entries)?,
        };
        if self.options.strict {
            failure.spec_mismatch = entries
                .iter()
                .filter_map(|e| {
                    let spec = e.spec?;
                    (spec != e.observed).then(|| Row {
                        name: e.name.clone(),
                        expected: Some(spec),
                        observed: Some(e.observed),
                    })
                })
                .collect();
        }
        if failure.is_empty() {
            Ok(outcome)
        } else {
            Err(CensusError::Failed(Box::new(failure)))
        }
    }

    fn header(&self) -> String {
        format!(
            "# Tessera census baseline (decision 0045); local only, never commit\n\
             # package\ttest\tfunction\tinstall version\n\
             # {}\t{}\t{}\t{}\n\
             # name\tvalue\tspec\n",
            self.package, self.test, self.function, self.version
        )
    }

    /// Writes the first baseline of this version and its drift report.
    fn create(&self, entries: &[Entry]) -> Result<Outcome, CensusError> {
        write_baseline(&self.baseline, &self.header(), entries)?;
        eprintln!(
            "census: no baseline for install {}; wrote {} ({} counts)",
            self.version,
            self.baseline.display(),
            entries.len()
        );
        let previous = self.baseline.parent().and_then(|dir| {
            let (version, path) = previous_baseline(dir, &self.version)?;
            match read_baseline(&path) {
                Ok(Some(old)) => Some((version, old)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("census: previous baseline ignored: {e}");
                    None
                }
            }
        });
        let (against, drift) = match previous {
            Some((version, old)) => (Against::Previous(version), drift_rows(&old, entries)),
            None => (
                Against::Spec,
                entries
                    .iter()
                    .filter_map(|e| {
                        let spec = e.spec?;
                        (spec != e.observed).then(|| DriftRow {
                            name: e.name.clone(),
                            previous: None,
                            observed: Some(e.observed),
                            spec: Some(spec),
                        })
                    })
                    .collect(),
            ),
        };
        let what = match &against {
            Against::Previous(v) => format!("the baseline of install {v}"),
            Against::Spec => "the spec values".to_owned(),
        };
        let drift_file = if drift.is_empty() {
            eprintln!("census: no drift against {what}");
            match std::fs::remove_file(&self.drift_file) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(CensusError::Io {
                        path: self.drift_file.clone(),
                        source,
                    });
                }
            }
            None
        } else {
            eprint!(
                "census: drift against {what} ({} rows):\n{}",
                drift.len(),
                table(
                    &["name", "previous", "observed", "spec"],
                    drift.iter().map(|r| {
                        vec![
                            r.name.clone(),
                            show(r.previous),
                            show(r.observed),
                            show(r.spec),
                        ]
                    }),
                )
            );
            let against_field = match &against {
                Against::Previous(v) => v.as_str(),
                Against::Spec => "spec",
            };
            let mut out = String::from(
                "# Tessera census drift (decision 0045); local only, never commit\n\
                 # package\ttest\tfunction\tname\tagainst\tprevious\tobserved\tspec\n",
            );
            for r in &drift {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    self.package,
                    self.test,
                    self.function,
                    r.name,
                    against_field,
                    show(r.previous),
                    show(r.observed),
                    show(r.spec)
                ));
            }
            write_atomic(&self.drift_file, out.as_bytes())?;
            eprintln!("census: drift rows in {}", self.drift_file.display());
            Some(self.drift_file.clone())
        };
        Ok(Outcome::Created {
            baseline: self.baseline.clone(),
            counts: entries.len(),
            against,
            drift,
            drift_file,
        })
    }

    /// `<base>/drift/last-version` (best effort: a failure only loses the tool's fallback).
    fn note_last_version(&self) {
        let path = self.options.base.join("drift").join("last-version");
        if let Err(e) = write_atomic(&path, format!("{}\n", self.version).as_bytes()) {
            eprintln!("census: last-version not written: {e}");
        }
    }
}

impl Drop for Census {
    fn drop(&mut self) {
        if !self.finished && !std::thread::panicking() {
            panic!(
                "census {} dropped without finish(); its counts were not checked",
                self.label()
            );
        }
    }
}

/// Rows where `new` differs from `old`: differing values, names of `new` missing from
/// `old` (expected `None`) and names of `old` missing from `new` (observed `None`).
fn compare(old: &[Entry], new: &[Entry]) -> Vec<Row> {
    let old_map: BTreeMap<&str, Count> =
        old.iter().map(|e| (e.name.as_str(), e.observed)).collect();
    let new_names: BTreeSet<&str> = new.iter().map(|e| e.name.as_str()).collect();
    let mut rows: Vec<Row> = new
        .iter()
        .filter_map(|e| {
            let was = old_map.get(e.name.as_str()).copied();
            (was != Some(e.observed)).then(|| Row {
                name: e.name.clone(),
                expected: was,
                observed: Some(e.observed),
            })
        })
        .collect();
    rows.extend(
        old.iter()
            .filter(|e| !new_names.contains(e.name.as_str()))
            .map(|e| Row {
                name: e.name.clone(),
                expected: Some(e.observed),
                observed: None,
            }),
    );
    rows
}

fn drift_rows(old: &[Entry], new: &[Entry]) -> Vec<DriftRow> {
    let spec: BTreeMap<&str, Option<Count>> = new
        .iter()
        .map(|e| (e.name.as_str(), e.spec))
        .chain(old.iter().map(|e| (e.name.as_str(), e.spec)))
        .rev()
        .collect();
    compare(old, new)
        .into_iter()
        .map(|r| DriftRow {
            spec: spec.get(r.name.as_str()).copied().flatten(),
            name: r.name,
            previous: r.expected,
            observed: r.observed,
        })
        .collect()
}

/// The most recently modified sibling baseline of `dir` named like an install version other
/// than `current` (ties: the greater name).
fn previous_baseline(dir: &Path, current: &str) -> Option<(String, PathBuf)> {
    let mut best: Option<(std::time::SystemTime, String, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let Ok(file) = entry.file_name().into_string() else {
            continue;
        };
        let Some(version) = file.strip_suffix(".tsv") else {
            continue;
        };
        if version == current || !is_version_name(version) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let at = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        let better = best
            .as_ref()
            .is_none_or(|(t, v, _)| (at, version) > (*t, v.as_str()));
        if better {
            best = Some((at, version.to_owned(), entry.path()));
        }
    }
    best.map(|(_, v, p)| (v, p))
}

fn parse_count(s: &str) -> Option<Count> {
    s.parse::<i128>().ok().map(Count)
}

/// Reads a baseline this crate wrote (`None` when there is none).
fn read_baseline(path: &Path) -> Result<Option<Vec<Entry>>, CensusError> {
    let io = |source| CensusError::Io {
        path: path.to_path_buf(),
        source,
    };
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let bad = |line, reason| CensusError::BadBaseline {
        path: path.to_path_buf(),
        line,
        reason,
    };
    if meta.len() > MAX_BASELINE_BYTES {
        return Err(bad(0, "larger than any census baseline"));
    }
    let text = std::fs::read_to_string(path).map_err(io)?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(name), Some(value), Some(spec), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(bad(n, "not three tab-separated fields"));
        };
        if !valid_name(name) {
            return Err(bad(n, "not a valid name"));
        }
        let observed = parse_count(value).ok_or_else(|| bad(n, "value is not an integer"))?;
        let spec = match spec {
            "-" => None,
            s => Some(parse_count(s).ok_or_else(|| bad(n, "spec is not an integer"))?),
        };
        if !seen.insert(name.to_owned()) {
            return Err(bad(n, "name recorded twice"));
        }
        out.push(Entry {
            name: name.to_owned(),
            observed,
            spec,
        });
    }
    Ok(Some(out))
}

fn write_baseline(path: &Path, header: &str, entries: &[Entry]) -> Result<(), CensusError> {
    let mut out = String::from(header);
    for e in entries {
        out.push_str(&format!("{}\t{}\t{}\n", e.name, e.observed, show(e.spec)));
    }
    write_atomic(path, out.as_bytes())
}

/// Writes `bytes` to `path` atomically: a temporary file in the same directory (named by
/// process and a per-process counter), then a rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CensusError> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or_else(|| CensusError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("no parent directory"),
    })?;
    std::fs::create_dir_all(dir).map_err(|source| CensusError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let tmp = dir.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(source) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(CensusError::Io { path: tmp, source });
    }
    std::fs::rename(&tmp, path).map_err(|source| {
        let _ = std::fs::remove_file(&tmp);
        CensusError::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn show(c: Option<Count>) -> String {
    c.map_or_else(|| "-".to_owned(), |c| c.to_string())
}

/// A plain text table (columns padded, two spaces apart), one line per row, each ending in a
/// line break.
fn table(headers: &[&str], rows: impl Iterator<Item = Vec<String>>) -> String {
    let rows: Vec<Vec<String>> = std::iter::once(headers.iter().map(|h| (*h).to_owned()).collect())
        .chain(rows)
        .collect();
    let mut widths = vec![0usize; headers.len()];
    for r in &rows {
        for (w, c) in widths.iter_mut().zip(r) {
            *w = (*w).max(c.chars().count());
        }
    }
    let mut out = String::new();
    for r in &rows {
        let line: Vec<String> = r
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        out.push_str("  ");
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}
