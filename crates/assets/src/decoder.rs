// SPDX-License-Identifier: GPL-3.0-or-later
//! The decoder trait and the built-in decoders.

use std::convert::Infallible;
use std::fmt;

use crate::AssetKey;

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

/// What a running job tells its decoder.
pub struct JobContext<'a> {
    cancelled: &'a dyn Fn() -> bool,
    observed: std::cell::Cell<bool>,
}

impl<'a> JobContext<'a> {
    pub(crate) fn new(cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            cancelled,
            observed: std::cell::Cell::new(false),
        }
    }

    /// A context whose job is never cancelled, for calling a decoder directly.
    #[must_use]
    pub fn detached() -> JobContext<'static> {
        JobContext {
            cancelled: &|| false,
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

    /// Whether the decoder ever saw [`JobContext::is_cancelled`] return true.
    pub(crate) fn observed_cancel(&self) -> bool {
        self.observed.get()
    }
}

impl fmt::Debug for JobContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobContext")
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
        let ctx = JobContext::new(&f);
        assert!(!ctx.observed_cancel());
        assert!(ctx.is_cancelled());
        assert!(ctx.observed_cancel());
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
