// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type for the install crate.

use std::path::PathBuf;

use crate::launcherconfig::LauncherConfigError;
use crate::validate::ValidationProblem;
use crate::vdf::VdfError;

fn join_problems(problems: &[ValidationProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Errors returned by this crate.
///
/// Most discovery problems are *not* errors: an unusable candidate directory is
/// recorded in [`crate::Discovery::rejected`] instead. Errors are reserved for
/// things the caller asked for explicitly (an explicit or environment game
/// directory that is unusable, a file that cannot be read, a malformed file).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An I/O operation on `path` failed.
    #[error("I/O error on {}: {source}", path.display())]
    Io {
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// A file exceeded the size limit this crate accepts for it.
    #[error("{} is larger than the {limit}-byte limit", path.display())]
    TooLarge {
        /// The file involved.
        path: PathBuf,
        /// The limit in bytes.
        limit: u64,
    },

    /// A Steam VDF text file is malformed.
    #[error("malformed VDF in {}: {source}", path.display())]
    Vdf {
        /// The file involved.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: VdfError,
    },

    /// A launcher config file is malformed or uses a DTD.
    #[error("malformed launcher config {}: {source}", path.display())]
    LauncherConfig {
        /// The file involved.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: LauncherConfigError,
    },

    /// A directory the user named explicitly (`--game-dir` or
    /// `TESSERA_GAME_DIR`) is not a usable install.
    #[error("{} is not a usable install: {}", path.display(), join_problems(problems))]
    NotUsable {
        /// The directory involved.
        path: PathBuf,
        /// Why it is not usable (each has a human-readable `Display`).
        problems: Vec<ValidationProblem>,
    },

    /// A directory the user named explicitly holds data for a different game
    /// that shares the engine and archive naming. It is not opened.
    #[error("{} holds data for an unsupported game ({game})", path.display())]
    UnsupportedGame {
        /// The directory involved.
        path: PathBuf,
        /// The identifying name that was found (launcher-config stem, game name
        /// or client binary name).
        game: String,
    },
}

impl Error {
    /// What kind of error this is, for a log line: no path, no file content (the parse
    /// errors of a Steam or launcher file may quote it), only the I/O error's kind.
    pub(crate) fn class(&self) -> ErrorClass<'_> {
        ErrorClass(self)
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// [`Error::class`]: the error's kind without any path.
pub(crate) struct ErrorClass<'a>(&'a Error);

impl std::fmt::Display for ErrorClass<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Error::Io { source, .. } => write!(f, "I/O error ({:?})", source.kind()),
            Error::TooLarge { limit, .. } => write!(f, "larger than the {limit}-byte limit"),
            Error::Vdf { .. } => f.write_str("malformed VDF"),
            Error::LauncherConfig { .. } => f.write_str("malformed launcher config"),
            Error::NotUsable { problems, .. } => {
                write!(f, "not a usable install ({} problems)", problems.len())
            }
            Error::UnsupportedGame { .. } => f.write_str("unsupported game"),
        }
    }
}

/// Convenience alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;
