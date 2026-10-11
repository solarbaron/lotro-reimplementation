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
//! - **Comparison:** the CPU painter's tolerance (one step per channel, no neighbourhood
//!   search, no outliers). On a mismatch the actual and diff images go next to the golden.
//! - **Writes** are atomic (a temporary file in the same directory, then a rename).
//! - With `TESSERA_SHEET_DIR` set, every picture is also copied there (for viewing).
//!
//! Logs print paths and counts only.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

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
}

impl Outcome {
    /// `blessed` or `passed`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Blessed(_) => "blessed",
            Self::Passed(_) => "passed",
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
    parse: impl Fn(&[u8]) -> Result<T, tessera_install::RecordError>,
) -> Option<T> {
    let entry = a.lookup(did).ok()??;
    let bytes = a.read_entry(&entry).ok()?;
    parse(&bytes).ok()
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
            membership: record(
                &a,
                tessera_install::DID_SET_MEMBERSHIP,
                tessera_install::SetMembership::parse,
            ),
            install_stamp: record(
                &a,
                tessera_install::DID_INSTALL_STAMP,
                tessera_install::InstallStamp::parse,
            ),
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
    let bytes = encode(img)?;
    let dir = path.parent().ok_or("no parent directory")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
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
    let (require, bless) = (flags.require, flags.bless_local);
    if require && bless {
        return Err(
            "TESSERA_GOLDEN_REQUIRE and TESSERA_GOLDEN_BLESS_LOCAL are both set; refused".into(),
        );
    }
    check_outside_work_trees(dir)?;
    if let Some(sheet) = &flags.sheet {
        let p = sheet.join(format!("{name}.png"));
        if let Err(e) = write_atomic(&p, img) {
            eprintln!("sheet: {e}");
        }
    }
    let path = dir.join(format!("{name}.png"));
    // A golden that exists but cannot be read is a failure, never a first run: only
    // `TESSERA_GOLDEN_BLESS_LOCAL=1` replaces it (decision 0031: a golden is written on a
    // first run, when it is missing).
    let existing = if bless || !path.exists() {
        None
    } else {
        match decode(&path) {
            Ok(img) => Some(img),
            Err(e) => {
                return Err(format!(
                    "golden {} unreadable: {e}; set TESSERA_GOLDEN_BLESS_LOCAL=1 to replace it",
                    path.display()
                ));
            }
        }
    };
    let Some(want) = existing else {
        if require && !bless {
            return Err(format!(
                "golden {} missing and TESSERA_GOLDEN_REQUIRE=1",
                path.display()
            ));
        }
        write_atomic(&path, img)?;
        eprintln!("golden: blessed {}", path.display());
        return Ok(Outcome::Blessed(path));
    };
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
    let (bad, diff) = compare(&want, img);
    if bad > 0 {
        write_atomic(&actual, img)?;
        write_atomic(&diff_path, &diff)?;
        return Err(format!(
            "golden {name}: {bad} pixels differ; actual at {}, diff at {}",
            actual.display(),
            diff_path.display()
        ));
    }
    eprintln!("golden: passed {}", path.display());
    Ok(Outcome::Passed(path))
}
