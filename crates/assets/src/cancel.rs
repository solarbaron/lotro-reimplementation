// SPDX-License-Identifier: GPL-3.0-or-later
//! Cancellation tokens.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Counts tokens that went from live to cancelled, process-wide. The pool uses it to skip
/// looking for abandoned jobs when no token was cancelled since it last looked.
static CANCEL_EPOCH: AtomicU64 = AtomicU64::new(0);

/// The current value of the process-wide cancellation counter.
pub(crate) fn cancel_epoch() -> u64 {
    CANCEL_EPOCH.load(Ordering::Acquire)
}

/// A shareable cancellation flag.
///
/// Clones share the flag: cancelling any clone cancels them all. Pass one token to several
/// [`AssetStore::request_with`](crate::AssetStore::request_with) calls to withdraw a whole
/// group of requests (for example everything a closed panel asked for) at once. Once
/// cancelled, a token stays cancelled.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A fresh, not-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels the token (and every clone of it).
    pub fn cancel(&self) {
        if !self.0.swap(true, Ordering::AcqRel) {
            CANCEL_EPOCH.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Whether the token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Whether `self` and `other` are clones of the same token.
    #[must_use]
    pub fn same_as(&self, other: &CancelToken) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_state() {
        let a = CancelToken::new();
        let b = a.clone();
        let c = CancelToken::new();
        assert!(a.same_as(&b) && !a.same_as(&c));
        assert!(!b.is_cancelled());
        a.cancel();
        assert!(b.is_cancelled());
        assert!(!c.is_cancelled());
    }

    #[test]
    fn cancelling_advances_the_epoch_once() {
        let t = CancelToken::new();
        let before = cancel_epoch();
        t.cancel();
        let after = cancel_epoch();
        assert!(after > before);
        t.cancel();
        // Other tests may cancel concurrently, so only monotonicity can be checked here.
        assert!(cancel_epoch() >= after);
    }
}
