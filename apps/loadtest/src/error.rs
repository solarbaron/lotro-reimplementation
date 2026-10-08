// SPDX-License-Identifier: GPL-3.0-or-later
//! The load tool's error type.

use std::path::PathBuf;

/// What can stop a load run before it produces a report.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadtestError {
    /// The options do not make sense (exit status 2).
    #[error("usage: {0}")]
    Usage(String),
    /// The in-process server could not start.
    #[error("in-process server: {0}")]
    Server(#[from] tessera_server::ServerError),
    /// A bot's session could not be built.
    #[error("bot session: {0}")]
    Net(#[from] tessera_net::NetError),
    /// Creating a bot's character over the protocol failed.
    #[error("provisioning {account}: {reason}")]
    Provision {
        /// The account.
        account: String,
        /// What went wrong.
        reason: String,
    },
    /// A file could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: std::io::Error,
    },
    /// The async runtime for the raw clients could not be built.
    #[error("runtime: {0}")]
    Runtime(String),
}

impl LoadtestError {
    /// An I/O error on `path`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
