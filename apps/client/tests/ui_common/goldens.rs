// SPDX-License-Identifier: GPL-3.0-or-later
//! Local-only real-data goldens of decision 0031 tier 2: pictures of panels that show
//! install content, kept outside every work tree and never committed.
//!
//! - **Location:** `${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/
//!   <package>/<test binary>/<function>/<install version>/<name>.png`; the install version
//!   is the short level-1 content fingerprint (`install-layout.md` §9.4), else the short
//!   level-0 one with the prefix `schema-`.
//! - **Refusal:** the nearest existing ancestor of the directory is canonicalised and the
//!   directory is refused when it or any ancestor holds a `.git` entry.
//! - **First run:** a missing golden is written and reported `golden: blessed <path>`;
//!   `TESSERA_GOLDEN_BLESS_LOCAL=1` rewrites existing ones; `TESSERA_GOLDEN_REQUIRE=1` makes
//!   a missing golden a failure; both together are refused. `TESSERA_BLESS` is not read.
//! - **Carry-over** (owner decision 2026-10-31, decision 0031 amendment): when the golden
//!   of the current install version is missing and a sibling install-version directory of
//!   the same test holds a golden of the same name (the most recently modified such
//!   directory; directories not named like an install version are ignored), the new
//!   picture is compared with it by the caller's comparison. A match copies that golden into
//!   the current directory ([`Outcome::CarriedOver`], a pass under `TESSERA_GOLDEN_REQUIRE`).
//!   A mismatch is never blessed: the actual and diff images go into the current directory,
//!   one review entry goes to `<base>/review/<install version>/changed/`, and the check
//!   fails, with or without `TESSERA_GOLDEN_REQUIRE`, until the owner approves the picture
//!   with `TESSERA_GOLDEN_BLESS_LOCAL=1`. Without a previous golden the first-run rules
//!   apply. Carry-over works only for directories of the layout above under the base.
//! - **Subjects:** a picture whose name does not say what it shows (the parity tooltips'
//!   `item-07` is the seventh record of a sample that an update can change) is checked with
//!   [`check_with_subject`]: a previous golden counts only when the caller reads the same
//!   subject for it in that version's directory (for example from its index); otherwise the
//!   first-run rules apply and the `new` record notes the other subject.
//! - **Review records** (one file per picture, so parallel test binaries never share a
//!   file): `<base>/review/<version>/changed/<key>.tsv` (package, test, function, name, old
//!   version, old golden, actual, diff, differing pixels, subject), `.../carried/<key>.tsv`
//!   (package, test, function, name, from version, old golden, new golden, subject) and
//!   `.../new/<key>.tsv` (package, test, function, name, version, `blessed` or `missing`,
//!   golden, note on another subject); `-` for no subject or note;
//!   `<base>/review/last-version` holds the version of the latest check.
//!   A pass, a carry-over or a blessing removes the picture's `changed` entry. A
//!   [`Pending`] made with [`Pending::for_dir`] also removes, when its test ends without a
//!   panic, the records (and the actual and diff images) of pictures of that directory
//!   that this run did not take (renamed or dropped shots).
//!   `tools/golden-review.sh` prints them and builds the review page.
//! - **Comparison:** the CPU painter's tolerance (one step per channel, no neighbourhood
//!   search, no outliers). On a mismatch the actual and diff images go next to the golden.
//! - **Writes** are atomic (a temporary file in the same directory, then a rename).
//! - With `TESSERA_SHEET_DIR` set, every picture is also copied there (for viewing).
//!
//! Logs print paths and counts only.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// The picture names checked in this process, per golden directory (for
/// [`prune_unchecked`]).
static CHECKED: Mutex<BTreeMap<PathBuf, BTreeSet<String>>> = Mutex::new(BTreeMap::new());

fn note_checked(dir: &Path, name: &str) {
    if let Ok(mut m) = CHECKED.lock() {
        m.entry(dir.to_path_buf())
            .or_default()
            .insert(name.to_owned());
    }
}

/// An RGBA picture (8 bits per channel, rows top to bottom).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub data: Vec<u8>,
}

/// What a golden check did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The golden was written (first run or `TESSERA_GOLDEN_BLESS_LOCAL=1`).
    Blessed(PathBuf),
    /// The picture matched the golden.
    Passed(PathBuf),
    /// The current version's golden was missing and the picture matched the golden of the
    /// same name of a previous install version, which was copied as the current golden.
    CarriedOver {
        /// The current version's golden, now written.
        path: PathBuf,
        /// The install version the golden came from.
        from_version: String,
    },
}

impl Outcome {
    /// `blessed`, `passed` or `carried over`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Blessed(_) => "blessed",
            Self::Passed(_) => "passed",
            Self::CarriedOver { .. } => "carried over",
        }
    }

    /// The current version's golden.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Blessed(p) | Self::Passed(p) | Self::CarriedOver { path: p, .. } => p,
        }
    }

    /// Whether the picture matched a golden (passed or carried over).
    #[must_use]
    pub fn matched(&self) -> bool {
        !matches!(self, Self::Blessed(_))
    }
}

/// Golden failures collected so that every picture of a test is checked (and changed ones
/// are listed for review) before the test fails. [`Self::finish`] panics with every
/// message; dropping a non-empty collector does the same unless the thread is already
/// panicking. Made with [`Self::for_dir`], it first runs [`prune_unchecked`] for that
/// directory (only when the thread is not panicking).
#[derive(Debug, Default)]
pub struct Pending {
    failures: Vec<String>,
    /// The golden directory and base to prune at the end.
    run: Option<(PathBuf, PathBuf)>,
}

impl Pending {
    /// A collector for the pictures of golden directory `dir` (made before the first
    /// picture of the run is checked), pruning the review records of pictures this run did
    /// not take when the test ends without a panic.
    #[must_use]
    pub fn for_dir(dir: &Path, flags: &Flags) -> Self {
        // A run starts here: only the pictures checked from now on count as taken.
        if let Ok(mut m) = CHECKED.lock() {
            m.remove(dir);
        }
        Self {
            failures: Vec::new(),
            run: Some((
                dir.to_path_buf(),
                flags.base.clone().unwrap_or_else(base_dir),
            )),
        }
    }

    fn prune(&mut self) {
        if let Some((dir, base)) = self.run.take() {
            prune_unchecked(&base, &dir);
        }
    }

    /// Records one failure message.
    pub fn push(&mut self, message: String) {
        eprintln!("{message}");
        self.failures.push(message);
    }

    /// How many failures were recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    /// Whether no failure was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }

    /// Panics with every recorded message, if any.
    pub fn finish(mut self) {
        self.prune();
        let failures = std::mem::take(&mut self.failures);
        assert!(
            failures.is_empty(),
            "{} golden failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        self.prune();
        if !self.failures.is_empty() {
            let failures = std::mem::take(&mut self.failures);
            panic!(
                "{} golden failures:\n{}",
                failures.len(),
                failures.join("\n")
            );
        }
    }
}

fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v == "1")
}

/// The base directory of tier-2 goldens.
#[must_use]
pub fn base_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TESSERA_GOLDEN_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    cache.join("tessera").join("goldens")
}

/// The short install version of `game_dir` (decision 0031: level-1, else `schema-` and
/// level-0).
#[must_use]
pub fn install_version(game_dir: &Path) -> String {
    let Ok(inst) = tessera_install::GameInstall::open(game_dir) else {
        return "unknown".into();
    };
    let entries: Vec<tessera_install::ContentEntry> = inst
        .archives
        .iter()
        .filter(|a| a.status.is_ok())
        .map(|a| content_entry(&a.path))
        .collect();
    let ok = entries
        .iter()
        .any(|e| e.iteration.is_some() || e.install_stamp.is_some());
    if ok {
        tessera_install::ContentFingerprint::compute(&entries)
            .short()
            .to_owned()
    } else {
        format!("schema-{}", inst.schema_fingerprint().short())
    }
}

fn record<T>(
    a: &tessera_dat::Archive,
    did: u32,
    parse: impl Fn(&[u8], u16) -> Result<T, tessera_install::RecordError>,
) -> Option<T> {
    let entry = a.lookup(did).ok()??;
    let bytes = a.read_entry(&entry).ok()?;
    parse(&bytes, entry.stamp).ok()
}

fn content_entry(path: &Path) -> tessera_install::ContentEntry {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match tessera_dat::Archive::open(path) {
        Ok(a) => tessera_install::ContentEntry {
            iteration: record(
                &a,
                tessera_install::DID_ITERATION,
                tessera_install::IterationRecord::parse,
            ),
            membership: record(&a, tessera_install::DID_SET_MEMBERSHIP, |b, _| {
                tessera_install::SetMembership::parse(b)
            }),
            install_stamp: record(&a, tessera_install::DID_INSTALL_STAMP, |b, _| {
                tessera_install::InstallStamp::parse(b)
            }),
            file_name,
        },
        Err(_) => tessera_install::ContentEntry {
            file_name,
            iteration: None,
            membership: None,
            install_stamp: None,
        },
    }
}

/// Refuses `dir` when the nearest existing ancestor (canonicalised) or any ancestor of it
/// holds a `.git` entry (decision 0031 tier 2).
///
/// # Errors
/// The reason, when refused.
pub fn check_outside_work_trees(dir: &Path) -> Result<(), String> {
    let mut probe = dir.to_path_buf();
    while !probe.exists() {
        if !probe.pop() {
            break;
        }
    }
    let real = std::fs::canonicalize(&probe).map_err(|e| format!("{}: {e}", probe.display()))?;
    for a in real.ancestors() {
        if a.join(".git").exists() {
            return Err(format!(
                "golden directory {} is inside a git work tree ({}); refused (decision 0031)",
                dir.display(),
                a.display()
            ));
        }
    }
    Ok(())
}

/// The directory of one test's goldens: `<base>/<package>/<test>/<install version>`,
/// `test` being `<test binary>/<function>`.
///
/// # Errors
/// When the directory lies in a work tree.
pub fn golden_dir(package: &str, test: &str, version: &str) -> Result<PathBuf, String> {
    let dir = base_dir().join(package).join(test).join(version);
    check_outside_work_trees(&dir)?;
    Ok(dir)
}

fn encode(img: &Rgba) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, img.width, img.height);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.data).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Most bytes of a decoded golden (a 4096 × 4096 RGBA picture).
const MAX_GOLDEN_BYTES: usize = 4096 * 4096 * 4;

fn decode(path: &Path) -> Result<Rgba, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let d = png::Decoder::new(std::io::BufReader::new(f));
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let size = r
        .output_buffer_size()
        .filter(|n| *n <= MAX_GOLDEN_BYTES)
        .ok_or("picture too large")?;
    let mut buf = vec![0; size];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "{:?} {:?}, not 8-bit RGBA",
            info.color_type, info.bit_depth
        ));
    }
    buf.truncate(info.buffer_size());
    Ok(Rgba {
        width: info.width,
        height: info.height,
        data: buf,
    })
}

/// Writes `img` to `path` atomically (temporary file in the same directory, then rename).
///
/// # Errors
/// Encoding or I/O failures.
pub fn write_atomic(path: &Path, img: &Rgba) -> Result<(), String> {
    write_bytes_atomic(path, &encode(img)?)
}

/// Writes `bytes` to `path` atomically (temporary file in the same directory, named by
/// process and a per-process counter, then rename).
///
/// # Errors
/// I/O failures.
pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or("no parent directory")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes a side file of a test's golden directory (for example the parity tooltips'
/// `index.txt`): refused inside a work tree like the goldens, written atomically.
///
/// # Errors
/// A refused directory or an I/O failure.
pub fn write_side_file(dir: &Path, file_name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    check_outside_work_trees(dir)?;
    let path = dir.join(file_name);
    write_bytes_atomic(&path, bytes)?;
    Ok(path)
}

/// The pixels whose largest channel difference exceeds 1, and a diff picture (red where
/// they differ, the golden dimmed elsewhere).
fn compare(want: &Rgba, got: &Rgba) -> (usize, Rgba) {
    let mut bad = 0usize;
    let mut diff = Vec::with_capacity(want.data.len());
    for (a, b) in got.data.chunks(4).zip(want.data.chunks(4)) {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > 1 {
            bad += 1;
            diff.extend_from_slice(&[255, 0, 0, 255]);
        } else {
            diff.extend_from_slice(&[b[0] / 4, b[1] / 4, b[2] / 4, 255]);
        }
    }
    (
        bad,
        Rgba {
            width: want.width,
            height: want.height,
            data: diff,
        },
    )
}

/// The environment of a check: `TESSERA_GOLDEN_REQUIRE`, `TESSERA_GOLDEN_BLESS_LOCAL`,
/// `TESSERA_SHEET_DIR`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flags {
    /// A missing golden fails.
    pub require: bool,
    /// Existing goldens are rewritten.
    pub bless_local: bool,
    /// Where pictures are also copied.
    pub sheet: Option<PathBuf>,
    /// The base directory of tier-2 goldens, for carry-over and review records; `None`:
    /// [`base_dir`].
    pub base: Option<PathBuf>,
}

impl Flags {
    /// The flags of the process environment.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            require: flag("TESSERA_GOLDEN_REQUIRE"),
            bless_local: flag("TESSERA_GOLDEN_BLESS_LOCAL"),
            sheet: std::env::var_os("TESSERA_SHEET_DIR")
                .filter(|d| !d.is_empty())
                .map(PathBuf::from),
            base: Some(base_dir()),
        }
    }
}

/// Checks `img` against `<dir>/<name>.png` with the process environment's flags
/// ([`check_with`]).
///
/// # Errors
/// As [`check_with`].
pub fn check(dir: &Path, name: &str, img: &Rgba) -> Result<Outcome, String> {
    check_with(dir, name, img, &Flags::from_env())
}

/// Checks `img` against `<dir>/<name>.png` (module docs).
///
/// # Errors
/// A refused combination of flags, a missing golden with `require`, a mismatch (with the
/// paths of the actual and diff images), or an I/O failure.
pub fn check_with(dir: &Path, name: &str, img: &Rgba, flags: &Flags) -> Result<Outcome, String> {
    check_with_compare(dir, name, img, flags, |want, got| {
        let (bad, diff) = compare(want, got);
        (bad == 0, format!("{bad} pixels differ"), diff)
    })
}

/// [`check_with`] with another comparison: `cmp(golden, actual)` gives whether the
/// picture passes, a summary for the failure message (counts only) and the diff picture.
/// Pictures of the GPU renderer use the tolerance of
/// `crates/render/tests/support/golden.rs` (decision 0031: the tolerance of tier 1 for the
/// same painter); the CPU painter's pictures use [`check_with`]. The sizes are equal when
/// `cmp` runs. The same comparison judges a carry-over from a previous install version.
///
/// # Errors
/// As [`check_with`], and a picture changed since a previous install version (module docs).
pub fn check_with_compare(
    dir: &Path,
    name: &str,
    img: &Rgba,
    flags: &Flags,
    cmp: impl FnOnce(&Rgba, &Rgba) -> (bool, String, Rgba),
) -> Result<Outcome, String> {
    check_with_subject(dir, name, img, flags, cmp, None)
}

/// What a picture shows, when its name does not say it (module docs, subjects).
pub struct Subject<'a> {
    /// The subject of this run's picture (one line, for example `item 0x70001234`).
    pub id: &'a str,
    /// The subject the picture of the same name showed in a previous install version's
    /// golden directory (given that directory), or `None` when it is not known there. A
    /// previous golden whose subject differs or is unknown is not compared.
    pub previous: &'a dyn Fn(&Path) -> Option<String>,
}

/// [`check_with_compare`] for a picture with a [`Subject`]: a previous install version's
/// golden is used for a carry-over only when it showed the same subject.
///
/// # Errors
/// As [`check_with_compare`].
pub fn check_with_subject(
    dir: &Path,
    name: &str,
    img: &Rgba,
    flags: &Flags,
    cmp: impl FnOnce(&Rgba, &Rgba) -> (bool, String, Rgba),
    subject: Option<&Subject<'_>>,
) -> Result<Outcome, String> {
    let (require, bless) = (flags.require, flags.bless_local);
    if require && bless {
        return Err(
            "TESSERA_GOLDEN_REQUIRE and TESSERA_GOLDEN_BLESS_LOCAL are both set; refused".into(),
        );
    }
    check_outside_work_trees(dir)?;
    note_checked(dir, name);
    if let Some(sheet) = &flags.sheet {
        let p = sheet.join(format!("{name}.png"));
        if let Err(e) = write_atomic(&p, img) {
            eprintln!("sheet: {e}");
        }
    }
    let base = flags.base.clone().unwrap_or_else(base_dir);
    let place = Place::of(&base, dir);
    if let Some(p) = &place {
        p.note_version();
    }
    let path = dir.join(format!("{name}.png"));
    if bless {
        write_atomic(&path, img)?;
        eprintln!("golden: blessed {}", path.display());
        if let Some(p) = &place {
            p.clear_changed(dir, name);
            p.remove(Kind::New, name);
        }
        return Ok(Outcome::Blessed(path));
    }
    if path.exists() {
        // A golden that exists but cannot be read is a failure, never a first run: only
        // `TESSERA_GOLDEN_BLESS_LOCAL=1` replaces it (decision 0031: a golden is written on a
        // first run, when it is missing).
        let want = decode(&path).map_err(|e| {
            format!(
                "golden {} unreadable: {e}; set TESSERA_GOLDEN_BLESS_LOCAL=1 to replace it",
                path.display()
            )
        })?;
        return compare_current(dir, name, img, &want, &path, place.as_ref(), cmp);
    }
    let mut note = "-".to_owned();
    if let Some(p) = &place {
        let previous = previous_golden(dir, name, &p.version, subject);
        if let Some((from_version, old)) = previous.found {
            return carry_over(dir, name, img, p, &from_version, &old, subject, cmp);
        }
        if let (Some((version, other)), Some(s)) = (previous.other, subject) {
            eprintln!(
                "golden: {name}: the golden of install {version} shows another subject; \
                 not compared (first-run rules)"
            );
            note = format!("now {}; {version} showed {other}", s.id);
        }
    }
    if require {
        if let Some(p) = &place {
            p.record(
                Kind::New,
                name,
                &[p.version.as_str(), "missing", &show(&path), &note],
            );
        }
        return Err(format!(
            "golden {} missing and TESSERA_GOLDEN_REQUIRE=1",
            path.display()
        ));
    }
    write_atomic(&path, img)?;
    eprintln!("golden: blessed {}", path.display());
    if let Some(p) = &place {
        p.record(
            Kind::New,
            name,
            &[p.version.as_str(), "blessed", &show(&path), &note],
        );
    }
    Ok(Outcome::Blessed(path))
}

/// Compares `img` with the current version's golden `want` (at `path`).
fn compare_current(
    dir: &Path,
    name: &str,
    img: &Rgba,
    want: &Rgba,
    path: &Path,
    place: Option<&Place>,
    cmp: impl FnOnce(&Rgba, &Rgba) -> (bool, String, Rgba),
) -> Result<Outcome, String> {
    let actual = dir.join(format!("{name}.actual.png"));
    let diff_path = dir.join(format!("{name}.diff.png"));
    if (want.width, want.height) != (img.width, img.height) {
        write_atomic(&actual, img)?;
        return Err(format!(
            "golden {name}: size {}x{} != {}x{}; actual at {}",
            img.width,
            img.height,
            want.width,
            want.height,
            actual.display()
        ));
    }
    let (pass, summary, diff) = cmp(want, img);
    if !pass {
        write_atomic(&actual, img)?;
        write_atomic(&diff_path, &diff)?;
        return Err(format!(
            "golden {name}: {summary}; actual at {}, diff at {}",
            actual.display(),
            diff_path.display()
        ));
    }
    eprintln!("golden: passed {}", path.display());
    if let Some(p) = place {
        p.clear_changed(dir, name);
    }
    Ok(Outcome::Passed(path.to_path_buf()))
}

/// Judges `img` against `old`, the golden of the same name of install version
/// `from_version` (module docs, carry-over).
#[allow(clippy::too_many_arguments)]
fn carry_over(
    dir: &Path,
    name: &str,
    img: &Rgba,
    place: &Place,
    from_version: &str,
    old: &Path,
    subject: Option<&Subject<'_>>,
    cmp: impl FnOnce(&Rgba, &Rgba) -> (bool, String, Rgba),
) -> Result<Outcome, String> {
    let subject_id = subject.map_or("-", |s| s.id);
    let path = dir.join(format!("{name}.png"));
    let old_bytes = std::fs::read(old).map_err(|e| format!("{}: {e}", old.display()))?;
    let want = decode(old).map_err(|e| {
        format!(
            "previous golden {} (install {from_version}) unreadable: {e}; the picture is not \
             blessed; set TESSERA_GOLDEN_BLESS_LOCAL=1 to bless it",
            old.display()
        )
    })?;
    let actual = dir.join(format!("{name}.actual.png"));
    let diff_path = dir.join(format!("{name}.diff.png"));
    let (summary, diff_shown, bad) = if (want.width, want.height) != (img.width, img.height) {
        write_atomic(&actual, img)?;
        // A stale diff of an earlier run would not belong to this picture.
        let _ = std::fs::remove_file(&diff_path);
        let pixels = u64::from(img.width) * u64::from(img.height);
        (
            format!(
                "size {}x{} != {}x{}",
                img.width, img.height, want.width, want.height
            ),
            "-".to_owned(),
            pixels,
        )
    } else {
        let bad = strict_differences(&want, img);
        let (pass, summary, diff) = cmp(&want, img);
        if pass {
            write_bytes_atomic(&path, &old_bytes)?;
            eprintln!(
                "golden: carried over from {from_version} {} -> {}",
                old.display(),
                path.display()
            );
            place.clear_changed(dir, name);
            place.remove(Kind::New, name);
            place.record(
                Kind::Carried,
                name,
                &[from_version, &show(old), &show(&path), subject_id],
            );
            return Ok(Outcome::CarriedOver {
                path,
                from_version: from_version.to_owned(),
            });
        }
        write_atomic(&actual, img)?;
        write_atomic(&diff_path, &diff)?;
        (summary, show(&diff_path), bad)
    };
    place.remove(Kind::New, name);
    let entry = place.record(
        Kind::Changed,
        name,
        &[
            from_version,
            &show(old),
            &show(&actual),
            &diff_shown,
            &bad.to_string(),
            subject_id,
        ],
    );
    Err(format!(
        "golden {name}: changed since install {from_version}: needs the owner's review \
         ({summary}); previous golden {}, actual at {}, diff at {diff_shown}, review entry {}; \
         TESSERA_GOLDEN_BLESS_LOCAL=1 blesses it after approval",
        old.display(),
        actual.display(),
        entry.map_or_else(|| "not written".to_owned(), |p| p.display().to_string()),
    ))
}

/// The pixels whose largest channel difference exceeds 1 (the CPU painter's tolerance),
/// whatever the caller's comparison: the review list's count.
fn strict_differences(want: &Rgba, got: &Rgba) -> u64 {
    let n = got
        .data
        .chunks(4)
        .zip(want.data.chunks(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 1))
        .count();
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Whether `s` is named like an install version: 16 lowercase hex digits, optionally after
/// `schema-` (decision 0031).
#[must_use]
pub fn is_version_name(s: &str) -> bool {
    let hex = s.strip_prefix("schema-").unwrap_or(s);
    hex.len() == tessera_install::SHORT_LEN
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What [`previous_golden`] found.
#[derive(Debug, Default)]
struct Previous {
    /// The version and path of the previous golden.
    found: Option<(String, PathBuf)>,
    /// Without one: the newest version whose golden of the name shows another subject, and
    /// that subject (`unknown` when the caller could not read it).
    other: Option<(String, String)>,
}

/// The previous golden of `<dir>/<name>.png`: among the sibling directories of `dir` named
/// like an install version other than `current` that hold `<name>.png` (and, with a
/// `subject`, showed the same subject), the most recently modified one (ties: the greater
/// name).
fn previous_golden(
    dir: &Path,
    name: &str,
    current: &str,
    subject: Option<&Subject<'_>>,
) -> Previous {
    let mut out = Previous::default();
    let Some(parent) = dir.parent() else {
        return out;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return out;
    };
    let file = format!("{name}.png");
    let mut best: Option<(std::time::SystemTime, String, PathBuf)> = None;
    let mut other: Option<(std::time::SystemTime, String, String)> = None;
    for entry in entries.flatten() {
        let Ok(version) = entry.file_name().into_string() else {
            continue;
        };
        if version == current || !is_version_name(&version) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let golden = entry.path().join(&file);
        if !golden.is_file() {
            continue;
        }
        let at = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        if let Some(s) = subject {
            let was = (s.previous)(&entry.path());
            if was.as_deref() != Some(s.id) {
                let newer = other
                    .as_ref()
                    .is_none_or(|(t, v, _)| (at, version.as_str()) > (*t, v.as_str()));
                if newer {
                    other = Some((at, version, was.unwrap_or_else(|| "unknown".to_owned())));
                }
                continue;
            }
        }
        let better = best
            .as_ref()
            .is_none_or(|(t, v, _)| (at, version.as_str()) > (*t, v.as_str()));
        if better {
            best = Some((at, version, golden));
        }
    }
    out.found = best.map(|(_, v, p)| (v, p));
    if out.found.is_none() {
        out.other = other.map(|(_, v, s)| (v, s));
    }
    out
}

/// A path for a log line or a TSV field (tabs and line breaks become spaces).
fn show(p: &Path) -> String {
    field(&p.display().to_string())
}

fn field(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// The kinds of review records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Changed since a previous install version: the review list.
    Changed,
    /// Carried over from a previous install version.
    Carried,
    /// No previous golden (blessed, or missing under `TESSERA_GOLDEN_REQUIRE`).
    New,
}

impl Kind {
    fn dir(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Carried => "carried",
            Self::New => "new",
        }
    }
}

/// Where a golden directory lies in the tier-2 layout:
/// `<base>/<package>/<test binary>/<function>/<version>` (or `<base>/<package>/<function>/
/// <version>`, the test binary then taken from this module's crate).
#[derive(Debug, Clone)]
struct Place {
    base: PathBuf,
    package: String,
    test: String,
    function: String,
    version: String,
}

impl Place {
    fn of(base: &Path, dir: &Path) -> Option<Self> {
        let rel = dir.strip_prefix(base).ok()?;
        let parts: Vec<String> = rel
            .components()
            .map(|c| match c {
                std::path::Component::Normal(n) => n.to_str().map(str::to_owned),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let (version, rest) = parts.split_last()?;
        if !is_version_name(version) || rest.first().is_some_and(|p| p == "review") {
            return None;
        }
        let (package, test, function) = match rest {
            [package, function] => (
                package.clone(),
                module_path!().split("::").next().unwrap_or("-").to_owned(),
                function.clone(),
            ),
            [package, test, function @ ..] if !function.is_empty() => {
                (package.clone(), test.clone(), function.join("/"))
            }
            _ => return None,
        };
        Some(Self {
            base: base.to_path_buf(),
            package,
            test,
            function,
            version: version.clone(),
        })
    }

    fn review_dir(&self) -> PathBuf {
        self.base.join("review")
    }

    /// The record file of picture `name` of kind `kind`.
    fn entry(&self, kind: Kind, name: &str) -> PathBuf {
        self.review_dir()
            .join(&self.version)
            .join(kind.dir())
            .join(format!(
                "{}.tsv",
                entry_key(&[&self.package, &self.test, &self.function, name])
            ))
    }

    /// Records `<base>/review/last-version` (when it differs). Failures are logged only.
    fn note_version(&self) {
        let path = self.review_dir().join("last-version");
        let line = format!("{}\n", self.version);
        if std::fs::read_to_string(&path).is_ok_and(|s| s == line) {
            return;
        }
        if let Err(e) = write_bytes_atomic(&path, line.as_bytes()) {
            eprintln!("golden review: {e}");
        }
    }

    /// Writes one record line (package, test, function, name, then `rest`). Gives its path,
    /// or `None` (logged) when it cannot be written.
    fn record(&self, kind: Kind, name: &str, rest: &[&str]) -> Option<PathBuf> {
        let mut cols = vec![
            field(&self.package),
            field(&self.test),
            field(&self.function),
            field(name),
        ];
        cols.extend(rest.iter().map(|s| field(s)));
        let path = self.entry(kind, name);
        let line = format!("{}\n", cols.join("\t"));
        match write_bytes_atomic(&path, line.as_bytes()) {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("golden review: {e}");
                None
            }
        }
    }

    fn remove(&self, kind: Kind, name: &str) {
        let path = self.entry(kind, name);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("golden review: {}: {e}", path.display()),
        }
    }

    /// Removes the picture's review entry and the actual and diff images it names in `dir`.
    fn clear_changed(&self, dir: &Path, name: &str) {
        let path = self.entry(Kind::Changed, name);
        if !path.exists() {
            return;
        }
        for f in [format!("{name}.actual.png"), format!("{name}.diff.png")] {
            let _ = std::fs::remove_file(dir.join(f));
        }
        eprintln!("golden review: cleared {}", path.display());
        self.remove(Kind::Changed, name);
    }
}

/// Removes the review records of golden directory `dir` (under `base`) for pictures not
/// checked in this process (since the last [`Pending::for_dir`] of `dir`), with the actual and diff images a `changed` record names in
/// `dir`. Nothing is removed when no picture of `dir` was checked. Failures are logged.
pub fn prune_unchecked(base: &Path, dir: &Path) {
    let Some(place) = Place::of(base, dir) else {
        return;
    };
    let checked: BTreeSet<String> = match CHECKED.lock() {
        Ok(m) => match m.get(dir) {
            Some(names) if !names.is_empty() => names.iter().map(|n| field(n)).collect(),
            _ => return,
        },
        Err(_) => return,
    };
    let own = [
        field(&place.package),
        field(&place.test),
        field(&place.function),
    ];
    for kind in [Kind::Changed, Kind::Carried, Kind::New] {
        let kdir = place.review_dir().join(&place.version).join(kind.dir());
        let Ok(entries) = std::fs::read_dir(&kdir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "tsv") {
                continue;
            }
            // A record is one short line; anything larger is not ours.
            if !entry.metadata().is_ok_and(|m| m.len() <= 64 * 1024) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let cols: Vec<&str> = text.lines().next().unwrap_or("").split('\t').collect();
            let (Some(head), Some(name)) = (cols.get(..3), cols.get(3)) else {
                continue;
            };
            let same = head.iter().zip(&own).all(|(a, b)| *a == b.as_str());
            if !same || checked.contains(*name) {
                continue;
            }
            // The images are removed only for a plain picture name inside `dir`.
            let plain = !name.is_empty() && !name.contains(['/', '\\']) && !name.starts_with('.');
            if kind == Kind::Changed && plain {
                for f in [format!("{name}.actual.png"), format!("{name}.diff.png")] {
                    let _ = std::fs::remove_file(dir.join(f));
                }
            }
            match std::fs::remove_file(&path) {
                Ok(()) => eprintln!(
                    "golden review: removed {} (picture not taken in this run)",
                    path.display()
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("golden review: {}: {e}", path.display()),
            }
        }
    }
}

/// A file-name key of a picture: the parts joined by `/`, every byte outside
/// `[A-Za-z0-9._-]` written `%XX`; a key longer than 200 bytes is cut and gets a hash.
fn entry_key(parts: &[&str]) -> String {
    let joined = parts.join("/");
    let mut key = String::with_capacity(joined.len());
    for b in joined.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-') {
            key.push(char::from(b));
        } else {
            key.push_str(&format!("%{b:02X}"));
        }
    }
    if key.len() > 200 {
        // FNV-1a 64 of the whole key keeps cut keys apart.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in key.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        key.truncate(150);
        key.push_str(&format!("-{h:016x}"));
    }
    key
}
