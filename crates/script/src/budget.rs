// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-call instruction budget (`plugin-runtime.md` §10.1, `lua-plugin-api.md` §3.3).

use std::sync::atomic::{AtomicU64, Ordering};

/// Message of the error raised when a call runs out of its instruction budget.
pub const BUDGET_MESSAGE: &str = "script exceeded its instruction budget";

/// Work budget of one host-to-Lua call, shared by the VM count hook, the pattern matcher
/// and any other bounded host work done on the call's behalf.
///
/// Every host call resets it ([`crate::ScriptState::enter`]); once the call has charged
/// more than its limit, every further charge fails with [`BUDGET_MESSAGE`].
#[derive(Debug)]
pub struct Budget {
    used: AtomicU64,
    limit: Option<u64>,
}

impl Budget {
    /// A budget of `limit` units per call; `None` is unlimited.
    #[must_use]
    pub fn new(limit: Option<u64>) -> Self {
        Self {
            used: AtomicU64::new(0),
            limit,
        }
    }

    /// The per-call limit.
    #[must_use]
    pub fn limit(&self) -> Option<u64> {
        self.limit
    }

    /// Units charged since the last reset.
    #[must_use]
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// Account for `n` units of work.
    ///
    /// # Errors
    ///
    /// A runtime error carrying [`BUDGET_MESSAGE`] once the call is over its budget.
    pub fn charge(&self, n: u64) -> mlua::Result<()> {
        let used = self.used.fetch_add(n, Ordering::Relaxed).saturating_add(n);
        match self.limit {
            Some(limit) if used > limit => Err(mlua::Error::runtime(BUDGET_MESSAGE)),
            _ => Ok(()),
        }
    }

    /// Has the current call spent its budget?
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.limit.is_some_and(|limit| self.used() > limit)
    }

    /// Start a new call.
    pub fn reset(&self) {
        self.used.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_and_resets() {
        let b = Budget::new(Some(10));
        assert!(b.charge(10).is_ok());
        assert!(!b.exhausted());
        assert!(b.charge(1).is_err());
        assert!(b.exhausted());
        b.reset();
        assert_eq!(b.used(), 0);
        let unlimited = Budget::new(None);
        assert!(unlimited.charge(u64::MAX).is_ok());
        assert!(unlimited.charge(u64::MAX).is_ok());
        assert!(!unlimited.exhausted());
    }
}
