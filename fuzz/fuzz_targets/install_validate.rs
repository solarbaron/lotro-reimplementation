// SPDX-License-Identifier: GPL-3.0-or-later
//! The install-directory readers of `tessera-install` (`docs/specs/install-layout.md`
//! §3.4, §3.7, §6, §9): `validate` over a directory of hostile files, with
//! `read_project_version` (the `ProjectVersion` file), `detect_game_type` (launcher
//! configs through `launcherconfig::read_launcher_config`) and the archive files' header
//! prefixes and names.
//!
//! The readers take a directory, so this target writes the input's files into a private
//! temporary directory (a new one per run, removed when the run ends; never an install).
//!
//! Input layout: byte 0 is the file count (at most [`MAX_FILES`]); each file takes a name
//! byte (an index into [`NAMES`]) and a little-endian u16 length, then that many bytes.
//! Nothing may panic, and the project version read alone agrees with the one in the
//! validation report.

#![no_main]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use libfuzzer_sys::fuzz_target;
use tessera_install::{ProjectVersion, detect_game_type, read_project_version, validate};

const MAX_FILES: usize = 6;
/// Longest file written.
const MAX_LEN: usize = 16 * 1024;

/// File names a run can use: the version file (case variants), launcher configs, archive
/// files of the naming grammar (§3.1) and unrelated names.
const NAMES: [&str; 14] = [
    "ProjectVersion",
    "projectversion",
    "game.launcherconfig",
    "other.launcherconfig",
    "client_general.dat",
    "client_general_aux_1.datx",
    "client_local_English.dat",
    "client_cell_1.dat",
    "client_surface.dat",
    "client_surface_aux_1.datx",
    "client_surface_aux_2.datx",
    "client_gamelogic.dat",
    "UserPreferences.ini",
    "readme.txt",
];

/// Runs made by this process; part of each run's directory name.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// One run's private directory, removed when the run ends (a crashing input leaves its
/// files behind for inspection). It is made with `create_dir`, never `create_dir_all`, so
/// a path that already exists (a stale directory or a planted symlink) is never reused:
/// the run is skipped instead.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Option<Self> {
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tessera-fuzz-install-validate-{}-{run}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).ok()?;
        Some(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&count, mut rest)) = data.split_first() else {
        return;
    };
    let Some(scratch) = Scratch::new() else {
        return;
    };
    let dir = scratch.0.clone();
    for _ in 0..usize::from(count) % (MAX_FILES + 1) {
        let [name, lo, hi, tail @ ..] = rest else {
            break;
        };
        let len = usize::from(u16::from_le_bytes([*lo, *hi])) % (MAX_LEN + 1);
        let (bytes, next) = tail.split_at(len.min(tail.len()));
        let _ = std::fs::write(dir.join(NAMES[usize::from(*name) % NAMES.len()]), bytes);
        rest = next;
    }

    let version = read_project_version(&dir);
    if let ProjectVersion::Value(v) = version {
        assert!(v <= 9_999_999_999, "at most ten digits");
    }
    let _ = detect_game_type(&dir);
    let report = validate(&dir);
    assert_eq!(
        report.project_version, version,
        "validate reads the same project version"
    );
});
