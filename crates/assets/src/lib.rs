// SPDX-License-Identifier: GPL-3.0-or-later
//! Asset pipeline over a record source such as [`tessera_dat::ArchiveSet`].
//!
//! This crate turns "I need DID X" into decoded, shareable products without ever blocking
//! the caller (`docs/ARCHITECTURE.md` §8 and §21). It has no async runtime: work runs on a
//! dedicated pool of std threads and results are handed back through a queue the UI polls
//! once per frame.
//!
//! # Pieces
//!
//! * [`WorkerPool`]: N std threads pulling jobs from one priority queue. Requests carry a
//!   [`Priority`] (`Immediate` > `Visible` > `Prefetch`, first-in first-out within a level).
//!   One pool can serve any number of stores.
//! * [`RecordSource`]: where raw record bytes come from. Implemented for
//!   [`tessera_dat::ArchiveSet`], [`tessera_dat::Archive`] and the in-memory
//!   [`MemorySource`].
//! * [`Decoder`]: turns record bytes into a product `T` and estimates its size in bytes.
//!   [`RawBytes`] keeps the bytes as they are; with the `formats` feature (default)
//!   [`decoders::SurfaceRgba`] decodes render surfaces to RGBA8.
//! * [`AssetStore`]: one decoder over one source. It de-duplicates requests (two requests
//!   for the same [`AssetKey`] share one job), caches products in an [`LruCache`] with a
//!   byte budget, remembers failures, and posts a [`Completion`] per finished job.
//! * [`Handle`]: what a request returns, at once. It reports the [`AssetState`]
//!   (`Queued → Loading → Ready | Failed | Cancelled`), offers a placeholder while loading,
//!   can change its priority and can be cancelled through its [`CancelToken`].
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use tessera_assets::{
//!     AssetState, AssetStore, MemorySource, PoolConfig, Priority, RawBytes, StoreConfig,
//!     WorkerPool,
//! };
//!
//! let pool = WorkerPool::new(PoolConfig::default());
//! let mut source = MemorySource::new();
//! source.insert(0x4100_0001, vec![1, 2, 3]);
//! let store = AssetStore::new(&pool, source, RawBytes, StoreConfig::default());
//!
//! let handle = store.request(0x4100_0001, Priority::Visible);
//! // Each frame: drain what finished, without blocking.
//! # handle.wait(std::time::Duration::from_secs(5));
//! for done in store.poll_completions(64) {
//!     println!("{:?} finished", done.key);
//! }
//! if let AssetState::Ready(bytes) = handle.state() {
//!     assert_eq!(*bytes, vec![1, 2, 3]);
//! }
//! ```
//!
//! # Cancellation and de-duplication
//!
//! Every request holds a [`CancelToken`] (fresh, or supplied with
//! [`AssetStore::request_with`] so one token can cancel a whole group). A job is abandoned
//! when *every* request attached to it has been cancelled. A queued job leaves the queue at
//! once when that happens through [`Handle::cancel`]; one abandoned only through a shared
//! token is removed when the queue is full and room is needed, or when a worker reaches
//! it. A running job checks between its stages (before reading, after reading, after
//! decoding) and decoders can poll [`JobContext::is_cancelled`] during long decodes. Cancellation is best effort: a job
//! that already finished stays cached.
//!
//! # Bounds
//!
//! Every queue and table is capped: worker threads ([`MAX_WORKERS`]), queued jobs
//! ([`PoolConfig::max_queued`]; cancelled jobs leave the queue when cancelled through their
//! handle, or are swept out when the queue is full), pending completions
//! ([`StoreConfig::max_completions`] and [`StoreConfig::max_completion_bytes`]; the oldest
//! are dropped and counted),
//! remembered failures ([`StoreConfig::max_failures`]), requests attached to one job
//! ([`StoreConfig::max_tickets_per_job`]) and cached bytes ([`StoreConfig::cache_budget`]).
//! Record sizes are bounded by the source (for archives, [`tessera_dat::ArchiveOptions`]).
//! A panicking decoder or source fails only its own job.
//!
//! # Instrumentation
//!
//! Jobs run inside a `tracing` span (`asset_job`, with the decoder name and DID); queue
//! length is traced on submit, read and decode times are logged per job at `trace`, and
//! [`AssetStore::stats`] / [`WorkerPool::stats`] give counters (hit rate, mean decode time,
//! queue length). [`AssetStore::log_stats`] emits them as one `debug` event.

#![warn(missing_docs)]

mod cache;
mod cancel;
mod decoder;
mod error;
mod pool;
mod source;
mod store;

use std::fmt;

pub use cache::LruCache;
pub use cancel::CancelToken;
pub use decoder::{Decoder, JobContext, RawBytes};
pub use error::AssetError;
pub use pool::{MAX_WORKERS, PoolConfig, PoolStats, WorkerPool};
pub use source::{MemorySource, RecordSource};
pub use store::{AssetState, AssetStore, Completion, Handle, Outcome, StoreConfig, StoreStats};

/// Ready-made decoders built on `tessera-formats` (feature `formats`).
#[cfg(feature = "formats")]
pub mod decoders {
    pub use crate::decoder::formats::SurfaceRgba;
}

/// A 32-bit data identifier, as used by the archive layer.
pub type Did = tessera_dat::Did;

/// What a request asks for: a DID plus a decoder-defined variant.
///
/// The variant distinguishes several products made from one record by the same decoder
/// (for example a texture decoded at a capped mip level). Most decoders use variant 0,
/// which is what `AssetKey::from(did)` gives.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct AssetKey {
    /// The record to read.
    pub did: Did,
    /// Decoder-defined variant; 0 by default.
    pub variant: u32,
}

impl AssetKey {
    /// A key with an explicit variant.
    #[must_use]
    pub const fn new(did: Did, variant: u32) -> Self {
        Self { did, variant }
    }
}

impl From<Did> for AssetKey {
    fn from(did: Did) -> Self {
        Self { did, variant: 0 }
    }
}

impl fmt::Display for AssetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.variant == 0 {
            write!(f, "{:#010x}", self.did)
        } else {
            write!(f, "{:#010x}/{}", self.did, self.variant)
        }
    }
}

/// How urgently a request is needed. Higher runs first.
///
/// `docs/ARCHITECTURE.md` §8.3 sketches finer streaming levels (near / mid / far rings);
/// those can be added between `Visible` and `Prefetch` once the world streamer exists.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub enum Priority {
    /// Speculative: may be useful soon (neighbouring items, the next page).
    #[default]
    Prefetch = 0,
    /// On screen now, a placeholder is showing.
    Visible = 1,
    /// Blocking something the user is waiting on right now.
    Immediate = 2,
}

impl Priority {
    /// All levels, lowest first.
    pub const ALL: [Priority; 3] = [Priority::Prefetch, Priority::Visible, Priority::Immediate];

    pub(crate) fn to_cell(self) -> u8 {
        self as u8 + 1
    }

    pub(crate) fn from_cell(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Prefetch),
            2 => Some(Self::Visible),
            3 => Some(Self::Immediate),
            _ => None,
        }
    }

    /// A short lowercase name for logs.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Prefetch => "prefetch",
            Self::Visible => "visible",
            Self::Immediate => "immediate",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Locks a mutex, recovering the data if another thread panicked while holding it.
///
/// Every critical section in this crate leaves its data consistent before it can panic
/// (user code never runs under a lock), so a poisoned lock is safe to reuse.
pub(crate) fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order_and_cells() {
        assert!(Priority::Immediate > Priority::Visible);
        assert!(Priority::Visible > Priority::Prefetch);
        for p in Priority::ALL {
            assert_eq!(Priority::from_cell(p.to_cell()), Some(p));
        }
        assert_eq!(Priority::from_cell(0), None);
        assert_eq!(Priority::from_cell(4), None);
    }

    #[test]
    fn key_display() {
        assert_eq!(AssetKey::from(0x4100_0001).to_string(), "0x41000001");
        assert_eq!(AssetKey::new(0x4100_0001, 3).to_string(), "0x41000001/3");
    }
}
