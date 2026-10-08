// SPDX-License-Identifier: GPL-3.0-or-later
//! The server's error type and the exit statuses of `test-server.md` §2.8.

use std::path::PathBuf;

use crate::tomlx::Finding;

/// Exit statuses (`test-server.md` §2.8).
pub mod exit {
    /// Success, or a clean shutdown.
    pub const OK: u8 = 0;
    /// Runtime failure (I/O, key unreadable, save failed during shutdown).
    pub const RUNTIME: u8 = 1;
    /// Usage or configuration error.
    pub const USAGE: u8 = 2;
    /// `check-content` found violations, or `run` refused its content.
    pub const CONTENT: u8 = 3;
    /// The data directory is locked by a running server, or the install guard refused it.
    pub const LOCKED: u8 = 4;
}

/// A failure of a server operation. [`ServerError::exit_status`] maps it to §2.8.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ServerError {
    /// An I/O operation on a path failed.
    #[error("{path}: {source}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
    /// An I/O operation without a single path failed.
    #[error("{0}")]
    Runtime(String),
    /// The configuration is invalid (`CONFIG` findings).
    #[error("invalid configuration:\n{}", format_findings(.0))]
    Config(Vec<(PathBuf, Finding)>),
    /// The content file is invalid.
    #[error("content refused:\n{}", format_findings(.0))]
    Content(Vec<(PathBuf, Finding)>),
    /// A usage error (bad option or argument, refused without `--force`, ...).
    #[error("{0}")]
    Usage(String),
    /// The data directory is locked by a running server.
    #[error("the data directory is in use by a running server{}", .pid.map(|p| format!(" (PID {p})")).unwrap_or_default())]
    Locked {
        /// The PID written in the lock file, if readable.
        pid: Option<u32>,
    },
    /// The install guard refused a path (`test-server.md` §3.1).
    #[error(
        "refusing {path}: it lies in or next to a game installation (files named client_*.dat or *.datx, or a .dat/.datx file in the data directory); the server never reads game data"
    )]
    InstallGuard {
        /// The refused path.
        path: PathBuf,
    },
    /// A file in the data directory is invalid.
    #[error("{path}: {message}")]
    InvalidFile {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// The server key file has unsafe permissions or the wrong owner.
    #[error("{path}: {message}")]
    KeyPermissions {
        /// The key file.
        path: PathBuf,
        /// What is wrong and how to fix it.
        message: String,
    },
    /// The transport could not be set up.
    #[error("transport: {0}")]
    Transport(#[from] tessera_protocol::transport::TransportError),
}

impl ServerError {
    /// The process exit status for this error (`test-server.md` §2.8).
    #[must_use]
    pub fn exit_status(&self) -> u8 {
        match self {
            Self::Io { .. }
            | Self::Runtime(_)
            | Self::KeyPermissions { .. }
            | Self::Transport(_) => exit::RUNTIME,
            Self::Config(_) | Self::Usage(_) => exit::USAGE,
            Self::InvalidFile { path, .. } => {
                // An invalid accounts file stops `run` with status 2 (§3.4); an unreadable
                // key with status 1 (§3.3).
                if path.file_name().is_some_and(|n| n == "server.key") {
                    exit::RUNTIME
                } else {
                    exit::USAGE
                }
            }
            Self::Content(_) => exit::CONTENT,
            Self::Locked { .. } | Self::InstallGuard { .. } => exit::LOCKED,
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

fn format_findings(findings: &[(PathBuf, Finding)]) -> String {
    findings
        .iter()
        .map(|(p, f)| f.display(p))
        .collect::<Vec<_>>()
        .join("\n")
}
