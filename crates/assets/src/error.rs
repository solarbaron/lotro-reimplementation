// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type.

use std::error::Error as StdError;
use std::sync::Arc;

use crate::Did;

/// Why an asset could not be produced.
///
/// Errors are cheap to clone (sources are behind `Arc`) because one failure is reported to
/// every request that shared the job.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum AssetError {
    /// No archive holds the record.
    #[error("record {did:#010x} not found")]
    NotFound {
        /// The DID that was requested.
        did: Did,
    },
    /// The archive layer failed to read the record.
    #[error("reading record {did:#010x}: {source}")]
    Read {
        /// The DID that was requested.
        did: Did,
        /// The archive error.
        #[source]
        source: Arc<tessera_dat::Error>,
    },
    /// A custom [`RecordSource`](crate::RecordSource) failed.
    #[error("record source failed for {did:#010x}: {source}")]
    Source {
        /// The DID that was requested.
        did: Did,
        /// The source's error.
        #[source]
        source: Arc<dyn StdError + Send + Sync>,
    },
    /// The decoder rejected the record.
    #[error("decoding {did:#010x} as {decoder}: {source}")]
    Decode {
        /// The DID that was requested.
        did: Did,
        /// [`Decoder::NAME`](crate::Decoder::NAME) of the decoder.
        decoder: &'static str,
        /// The decoder's error.
        #[source]
        source: Arc<dyn StdError + Send + Sync>,
    },
    /// The source or decoder panicked. The pool keeps running.
    #[error("{stage} panicked for record {did:#010x}")]
    Panicked {
        /// The DID that was requested.
        did: Did,
        /// `"read"`, `"decode"` or `"job"`.
        stage: &'static str,
    },
    /// The worker pool's queue already holds [`PoolConfig::max_queued`](crate::PoolConfig)
    /// jobs.
    #[error("asset queue full ({capacity} jobs)")]
    QueueFull {
        /// The configured capacity.
        capacity: usize,
    },
    /// The worker pool or the store was shut down before the job ran.
    #[error("asset pipeline shut down")]
    Shutdown,
}

impl AssetError {
    /// Wraps an error from a custom record source.
    pub fn source_error(did: Did, err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Source {
            did,
            source: Arc::new(err),
        }
    }

    /// Whether the failure depends on the data (and so will recur if retried), as opposed
    /// to the pipeline's state (full queue, shutdown, panic).
    #[must_use]
    pub fn is_data_error(&self) -> bool {
        matches!(
            self,
            Self::NotFound { .. } | Self::Read { .. } | Self::Source { .. } | Self::Decode { .. }
        )
    }
}

impl From<(Did, tessera_dat::Error)> for AssetError {
    fn from((did, err): (Did, tessera_dat::Error)) -> Self {
        match err {
            tessera_dat::Error::NotFound { did } => Self::NotFound { did },
            other => Self::Read {
                did,
                source: Arc::new(other),
            },
        }
    }
}
