// SPDX-License-Identifier: GPL-3.0-or-later
//! What the client's log lines say of a path. On a run the official login handed over no line names
//! a directory: an install's or a settings folder's path can name the user (decision 0044 item 3,
//! as the data loader and the plugin host do). Any other run names them as it always has.
//!
//! The rule is per process, not per call, because these lines run on the walker's and the audio
//! worker's threads, far from anything that knows about the session. [`crate::runtime::Runtime`]
//! sets it when a run starts, before any of them can log.

use std::fmt::Display;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static OFFICIAL_RUN: AtomicBool = AtomicBool::new(false);

/// Sets whether this run is a session the official login handed over.
pub fn set_official_run(official: bool) {
    OFFICIAL_RUN.store(official, Ordering::SeqCst);
}

/// Whether a log line may name a directory.
#[must_use]
pub fn names_directories() -> bool {
    !OFFICIAL_RUN.load(Ordering::SeqCst)
}

/// `path` as a log line shows it: whole, or on an official run only its file name.
#[must_use]
pub fn shown(path: &Path) -> String {
    if names_directories() {
        path.display().to_string()
    } else {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// An error as a log line shows it: whole, or on an official run nothing of it, since its text can
/// hold the path it failed on.
#[must_use]
pub fn shown_error(error: &dyn Display) -> String {
    if names_directories() {
        error.to_string()
    } else {
        "(not shown)".to_owned()
    }
}

/// `paths` for a log field: whole, or on an official run how many there are.
#[must_use]
pub fn shown_list(paths: &[std::path::PathBuf]) -> String {
    if names_directories() {
        format!("{paths:?}")
    } else {
        format!("{} folders", paths.len())
    }
}
