// SPDX-License-Identifier: GPL-3.0-or-later
//! Waits that give up when work stops progressing rather than after a fixed wall-clock
//! time (`walker.md` §7, implementer note on progress waits). A loaded machine makes such
//! a wait longer, not different: it ends with the work, or after a number of polls in a row
//! that showed no progress, or after a generous total number of polls.
//!
//! The caller says what progress is: a snapshot of counters that changes whenever work
//! starts, finishes or is given up. Polls are spaced by short sleeps, which a loaded
//! machine stretches too.

use std::time::{Duration, Instant};

/// Counts consecutive polls whose progress snapshot equals the previous one.
#[derive(Debug)]
pub struct StallCounter<P> {
    last: Option<P>,
    /// Polls in a row without a change, after the last [`StallCounter::observe`].
    pub stalled: u64,
}

impl<P> Default for StallCounter<P> {
    fn default() -> Self {
        Self {
            last: None,
            stalled: 0,
        }
    }
}

impl<P: PartialEq> StallCounter<P> {
    /// Records this poll's snapshot; returns the polls in a row without a change (0 for
    /// the first snapshot and for every change).
    pub fn observe(&mut self, p: P) -> u64 {
        if self.last.as_ref() == Some(&p) {
            self.stalled = self.stalled.saturating_add(1);
        } else {
            self.last = Some(p);
            self.stalled = 0;
        }
        self.stalled
    }
}

/// The limits of a wait on progress, in polls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressWait {
    /// Consecutive polls without progress before giving up.
    pub stall: u64,
    /// Polls in all before giving up.
    pub max: u64,
}

impl ProgressWait {
    /// Limits of `stall` polls without progress and `max` polls in all.
    #[must_use]
    pub const fn new(stall: u64, max: u64) -> Self {
        Self { stall, max }
    }
}

/// How long a wait may last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadWait {
    /// At most this much wall-clock time (the command line's `--settle-timeout`).
    Timeout(Duration),
    /// As long as the work progresses ([`ProgressWait`]); no wall-clock limit.
    Progress(ProgressWait),
}

/// One wait in progress: feed it a snapshot per poll and it says when to give up.
#[derive(Debug)]
pub struct Waiter<P> {
    limit: LoadWait,
    started: Instant,
    polls: u64,
    stall: StallCounter<P>,
}

impl<P: PartialEq> Waiter<P> {
    /// A wait starting now.
    #[must_use]
    pub fn new(limit: LoadWait) -> Self {
        Self {
            limit,
            started: Instant::now(),
            polls: 0,
            stall: StallCounter::default(),
        }
    }

    /// Records one poll that found the work not done yet, with its progress snapshot;
    /// returns whether the wait should give up.
    pub fn give_up(&mut self, progress: P) -> bool {
        self.polls = self.polls.saturating_add(1);
        let stalled = self.stall.observe(progress);
        match self.limit {
            LoadWait::Timeout(t) => self.started.elapsed() >= t,
            LoadWait::Progress(w) => stalled >= w.stall || self.polls >= w.max,
        }
    }

    /// Polls recorded so far.
    #[must_use]
    pub fn polls(&self) -> u64 {
        self.polls
    }

    /// Polls in a row without progress at the last poll.
    #[must_use]
    pub fn stalled(&self) -> u64 {
        self.stall.stalled
    }

    /// Wall-clock time since the wait started (for reports).
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

/// Polls `step` until it returns `None` (done; returns `true`) or the wait gives up
/// (returns `false`). `step` returns `Some(progress)` while the work is not done; between
/// polls the thread sleeps for `pause`.
pub fn wait_until<P: PartialEq>(
    limit: LoadWait,
    pause: Duration,
    mut step: impl FnMut() -> Option<P>,
) -> bool {
    let mut w = Waiter::new(limit);
    loop {
        let Some(p) = step() else {
            return true;
        };
        if w.give_up(p) {
            return false;
        }
        std::thread::sleep(pause);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_counter_resets_on_any_change() {
        let mut c = StallCounter::default();
        assert_eq!(c.observe(1), 0, "the first snapshot is progress");
        assert_eq!(c.observe(1), 1);
        assert_eq!(c.observe(1), 2);
        assert_eq!(c.observe(2), 0, "a change is progress");
        assert_eq!(c.observe(1), 0, "also a change back");
        assert_eq!(c.observe(1), 1);
    }

    #[test]
    fn progress_waits_count_polls_not_time() {
        let limit = LoadWait::Progress(ProgressWait::new(3, 100));
        // Progress every poll: only the total cap ends it.
        let mut n = 0u32;
        assert!(!wait_until(limit, Duration::ZERO, || {
            n += 1;
            Some(n)
        }));
        assert_eq!(n, 100);
        // No progress: the stall limit ends it (the first poll sets the mark).
        let mut n = 0u32;
        assert!(!wait_until(limit, Duration::ZERO, || {
            n += 1;
            Some(0)
        }));
        assert_eq!(n, 4);
        // Done before either.
        let mut n = 0u32;
        assert!(wait_until(limit, Duration::ZERO, || {
            n += 1;
            (n < 50).then_some(n)
        }));
    }

    #[test]
    fn timeouts_stay_wall_clock() {
        let mut w = Waiter::new(LoadWait::Timeout(Duration::ZERO));
        assert!(w.give_up(1), "a zero timeout gives up at once");
        let mut w = Waiter::new(LoadWait::Timeout(Duration::from_secs(3600)));
        for _ in 0..1000 {
            assert!(!w.give_up(0), "no stall limit with a timeout");
        }
        assert_eq!(w.polls(), 1000);
    }
}
