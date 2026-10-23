// SPDX-License-Identifier: GPL-3.0-or-later
//! The decoder trait and the built-in decoders.

use std::convert::Infallible;
use std::fmt;

use crate::{AssetKey, Priority};

/// Turns the bytes of one record into a product, on a worker thread.
///
/// A decoder must treat its input as hostile: return an error rather than panic, and bound
/// its allocations by a small multiple of the input. A panic is caught and reported as
/// [`AssetError::Panicked`](crate::AssetError::Panicked), but it still costs a stack
/// unwind and a log line.
pub trait Decoder: Send + Sync + 'static {
    /// The decoded product, shared as `Arc<Output>`.
    type Output: Send + Sync + 'static;
    /// The decoder's error type.
    type Error: std::error::Error + Send + Sync + 'static;
    /// Short name for logs and errors (for example `"surface-rgba"`).
    const NAME: &'static str;

    /// Decodes the record `key.did` (variant `key.variant`) from its complete bytes.
    ///
    /// Long decodes should poll [`JobContext::is_cancelled`] and may return any error once
    /// it is true; the job is then recorded as cancelled, not failed (or re-queued if a new
    /// request joined it in the meantime).
    fn decode(
        &self,
        key: AssetKey,
        bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<Self::Output, Self::Error>;

    /// Estimated memory held by `value`, in bytes, charged against the cache budget.
    ///
    /// Called once per product; should be cheap.
    fn size_of(&self, value: &Self::Output) -> usize;
}

/// What a running job tells its decoder: whether it was cancelled, and its priority.
///
/// Two priorities are on offer (`docs/ARCHITECTURE.md` §21 "Asset workers"):
/// [`JobContext::priority`] is the one the job held when a worker took it out of the
/// queue; [`JobContext::current_priority`] is the one it holds now, which a request may
/// have raised or lowered since. A decoder that chooses how to run a stage by priority
/// reads the latter when that stage starts (`collision.md` §12.1: the thread rule of a
/// collision hierarchy build reads the job's priority once, when the build starts).
pub struct JobContext<'a> {
    cancelled: &'a dyn Fn() -> bool,
    /// The live priority of the job's requests (`None`: none live), when the job has any.
    current: Option<&'a dyn Fn() -> Option<Priority>>,
    priority: Priority,
    observed: std::cell::Cell<bool>,
}

impl<'a> JobContext<'a> {
    /// A context for a job a worker took out of the queue at `priority`.
    pub(crate) fn new(cancelled: &'a dyn Fn() -> bool, priority: Priority) -> Self {
        Self {
            cancelled,
            current: None,
            priority,
            observed: std::cell::Cell::new(false),
        }
    }

    /// The same context, whose [`JobContext::current_priority`] asks `current`.
    pub(crate) fn with_current(mut self, current: &'a dyn Fn() -> Option<Priority>) -> Self {
        self.current = Some(current);
        self
    }

    /// A context whose job is never cancelled, for calling a decoder directly. Its
    /// priority is [`Priority::Prefetch`], the lowest ([`JobContext::detached_at`] to
    /// choose another).
    #[must_use]
    pub fn detached() -> JobContext<'static> {
        Self::detached_at(Priority::Prefetch)
    }

    /// A context whose job is never cancelled and runs at `priority`, for calling a decoder
    /// directly (tools and tests that want a decoder's behaviour at that priority).
    #[must_use]
    pub fn detached_at(priority: Priority) -> JobContext<'static> {
        JobContext {
            cancelled: &|| false,
            current: None,
            priority,
            observed: std::cell::Cell::new(false),
        }
    }

    /// Whether every request for this job has been cancelled.
    ///
    /// This takes a short lock; poll it every few milliseconds of work, not per byte.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        let c = (self.cancelled)();
        if c {
            self.observed.set(true);
        }
        c
    }

    /// The priority of the queue entry the worker took this job from: the job's priority
    /// when it started running (re-prioritisation while it waited included). Does not
    /// change while the job runs.
    #[must_use]
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// The job's priority now: for an [`AssetStore`](crate::AssetStore) job the highest
    /// priority of its requests that are not cancelled (pins included), which
    /// [`Handle::set_priority`](crate::Handle::set_priority) and new requests may have
    /// changed since the job started; [`JobContext::priority`] when no request is live, and
    /// for worker tasks and detached contexts (which have no requests).
    ///
    /// For an asset store job this takes the same short lock as
    /// [`JobContext::is_cancelled`].
    #[must_use]
    pub fn current_priority(&self) -> Priority {
        self.current.and_then(|f| f()).unwrap_or(self.priority)
    }

    /// Whether the decoder ever saw [`JobContext::is_cancelled`] return true.
    pub(crate) fn observed_cancel(&self) -> bool {
        self.observed.get()
    }
}

impl fmt::Debug for JobContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobContext")
            .field("priority", &self.priority)
            .field("observed_cancel", &self.observed.get())
            .finish_non_exhaustive()
    }
}

/// The identity decoder: the product is the record bytes themselves.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawBytes;

impl Decoder for RawBytes {
    type Output = Vec<u8>;
    type Error = Infallible;
    const NAME: &'static str = "raw-bytes";

    fn decode(
        &self,
        _key: AssetKey,
        bytes: Vec<u8>,
        _ctx: &JobContext<'_>,
    ) -> Result<Vec<u8>, Infallible> {
        Ok(bytes)
    }

    fn size_of(&self, value: &Vec<u8>) -> usize {
        value.capacity() + std::mem::size_of::<Vec<u8>>()
    }
}

#[cfg(feature = "formats")]
pub(crate) mod formats {
    use tessera_formats::{FormatError, Record, RgbaImage, Surface};

    use super::{Decoder, JobContext};
    use crate::AssetKey;

    /// Decodes render surfaces (DID prefix 0x41, `docs/specs/records.md` §5) to RGBA8.
    ///
    /// The record's DID prologue must match the requested DID. The key's variant is
    /// ignored. Memory is bounded by `tessera-formats` (at most 8× the stored data for raw
    /// and DXT formats; a pixel budget for embedded JPEG).
    #[derive(Debug, Clone, Copy, Default)]
    pub struct SurfaceRgba;

    impl Decoder for SurfaceRgba {
        type Output = RgbaImage;
        type Error = FormatError;
        const NAME: &'static str = "surface-rgba";

        fn decode(
            &self,
            key: AssetKey,
            bytes: Vec<u8>,
            _ctx: &JobContext<'_>,
        ) -> Result<RgbaImage, FormatError> {
            Surface::parse_expecting(&bytes, key.did)?.to_rgba8()
        }

        fn size_of(&self, value: &RgbaImage) -> usize {
            value.pixels.capacity() + std::mem::size_of::<RgbaImage>()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_bytes_is_identity() {
        let ctx = JobContext::detached();
        let out = RawBytes
            .decode(AssetKey::from(1), vec![9, 8], &ctx)
            .unwrap();
        assert_eq!(out, vec![9, 8]);
        assert!(RawBytes.size_of(&out) >= 2);
        assert!(!ctx.is_cancelled());
        assert!(!ctx.observed_cancel());
    }

    #[test]
    fn context_records_observation() {
        let f = || true;
        let ctx = JobContext::new(&f, Priority::Visible);
        assert!(!ctx.observed_cancel());
        assert!(ctx.is_cancelled());
        assert!(ctx.observed_cancel());
    }

    #[test]
    fn context_priorities() {
        assert_eq!(JobContext::detached().priority(), Priority::Prefetch);
        assert_eq!(
            JobContext::detached().current_priority(),
            Priority::Prefetch
        );
        let at = JobContext::detached_at(Priority::Immediate);
        assert_eq!(at.priority(), Priority::Immediate);
        assert_eq!(at.current_priority(), Priority::Immediate);
        assert!(!at.is_cancelled());

        let never = || false;
        let live = std::cell::Cell::new(Some(Priority::Visible));
        let current = || live.get();
        let ctx = JobContext::new(&never, Priority::Prefetch).with_current(&current);
        assert_eq!(ctx.priority(), Priority::Prefetch);
        assert_eq!(ctx.current_priority(), Priority::Visible);
        // No live request: the pop-time priority.
        live.set(None);
        assert_eq!(ctx.current_priority(), Priority::Prefetch);
        live.set(Some(Priority::Immediate));
        assert_eq!(ctx.current_priority(), Priority::Immediate);
        assert_eq!(ctx.priority(), Priority::Prefetch);
    }

    #[cfg(feature = "formats")]
    #[test]
    fn surface_rgba_decodes_and_rejects() {
        let did = 0x4100_0010u32;
        let mut rec = Vec::new();
        for v in [did, 0, 2, 1, 20, 6] {
            rec.extend_from_slice(&v.to_le_bytes());
        }
        rec.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let ctx = JobContext::detached();
        let img = formats::SurfaceRgba
            .decode(AssetKey::from(did), rec.clone(), &ctx)
            .unwrap();
        assert_eq!((img.width, img.height), (2, 1));
        assert_eq!(img.pixels, vec![3, 2, 1, 255, 6, 5, 4, 255]);
        // Wrong DID and truncated data are errors, not panics.
        assert!(
            formats::SurfaceRgba
                .decode(AssetKey::from(did + 1), rec.clone(), &ctx)
                .is_err()
        );
        rec.truncate(rec.len() - 1);
        assert!(
            formats::SurfaceRgba
                .decode(AssetKey::from(did), rec, &ctx)
                .is_err()
        );
    }
}
