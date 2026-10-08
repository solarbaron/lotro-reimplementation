// SPDX-License-Identifier: GPL-3.0-or-later
//! Rate limiters for the limits of `tessera-protocol.md` §10.2 and §13.1.
//!
//! Times are monotonic microseconds on the caller's clock (any epoch).

use std::collections::VecDeque;
use std::time::Duration;

use crate::limits::{CHAT_BUCKET_CAPACITY, CHAT_FLOOD_REJECTIONS, CHAT_FLOOD_WINDOW, CHAT_REFILL};

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// At most `max` events in any window of length `window` (an exact sliding window; memory is
/// bounded by `max`).
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    window: u64,
    max: usize,
    times: VecDeque<u64>,
}

impl SlidingWindow {
    /// A limiter allowing `max` events per `window`.
    #[must_use]
    pub fn new(max: u32, window: Duration) -> Self {
        let max = usize::try_from(max).unwrap_or(usize::MAX);
        Self {
            window: micros(window),
            max,
            times: VecDeque::with_capacity(max.min(1024)),
        }
    }

    fn expire(&mut self, now: u64) {
        while let Some(&t) = self.times.front() {
            if now.saturating_sub(t) >= self.window {
                self.times.pop_front();
            } else {
                break;
            }
        }
    }

    /// Records an event at `now` if the limit allows it; returns whether it did.
    pub fn try_record(&mut self, now: u64) -> bool {
        self.expire(now);
        if self.times.len() >= self.max {
            return false;
        }
        self.times.push_back(now);
        true
    }

    /// Records an event at `now` regardless of the limit and returns the number of events in
    /// the window including this one. At most `max + 1` events are remembered, so the result
    /// saturates there.
    pub fn record(&mut self, now: u64) -> usize {
        self.expire(now);
        if self.times.len() > self.max {
            self.times.pop_front();
        }
        self.times.push_back(now);
        self.times.len()
    }

    /// Events in the window ending at `now`.
    pub fn count(&mut self, now: u64) -> usize {
        self.expire(now);
        self.times.len()
    }
}

/// A token bucket: `capacity` tokens, one more every `refill`.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: u32,
    refill: u64,
    tokens: u32,
    last: Option<u64>,
}

impl TokenBucket {
    /// A full bucket.
    #[must_use]
    pub fn new(capacity: u32, refill: Duration) -> Self {
        Self {
            capacity,
            refill: micros(refill).max(1),
            tokens: capacity,
            last: None,
        }
    }

    fn refill_to(&mut self, now: u64) {
        let Some(last) = self.last else {
            self.last = Some(now);
            return;
        };
        if self.tokens >= self.capacity {
            self.last = Some(now);
            return;
        }
        let gained = now.saturating_sub(last) / self.refill;
        if gained > 0 {
            let gained32 = u32::try_from(gained).unwrap_or(u32::MAX);
            self.tokens = self.tokens.saturating_add(gained32).min(self.capacity);
            self.last = Some(if self.tokens >= self.capacity {
                now
            } else {
                last + gained * self.refill
            });
        }
    }

    /// Takes one token at `now`; returns false when none is left.
    pub fn try_take(&mut self, now: u64) -> bool {
        self.refill_to(now);
        if self.tokens == 0 {
            return false;
        }
        self.tokens -= 1;
        true
    }

    /// Tokens available at `now`.
    pub fn available(&mut self, now: u64) -> u32 {
        self.refill_to(now);
        self.tokens
    }
}

/// Outcome of a `ChatSend` against the chat limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatVerdict {
    /// Deliver the line.
    Allowed,
    /// Answer `ChatRejected(RATE_LIMITED)`.
    RateLimited,
    /// 20 rejections within 60 s: `Disconnect(FLOODING)`.
    Flooding,
}

/// The per-connection chat limits of §10.2: a bucket of 5 refilled by one token every 2 s,
/// and `FLOODING` after 20 rejections within 60 s.
#[derive(Debug, Clone)]
pub struct ChatLimiter {
    bucket: TokenBucket,
    rejections: SlidingWindow,
}

impl Default for ChatLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatLimiter {
    /// The §10.2 defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bucket: TokenBucket::new(CHAT_BUCKET_CAPACITY, CHAT_REFILL),
            rejections: SlidingWindow::new(CHAT_FLOOD_REJECTIONS, CHAT_FLOOD_WINDOW),
        }
    }

    /// Checks one `ChatSend` at `now`.
    pub fn check(&mut self, now: u64) -> ChatVerdict {
        if self.bucket.try_take(now) {
            return ChatVerdict::Allowed;
        }
        self.record_rejection(now)
    }

    /// Counts a rejection for another reason (invalid text, unknown recipient, ...): every
    /// `ChatRejected` counts towards `FLOODING`.
    pub fn record_rejection(&mut self, now: u64) -> ChatVerdict {
        if self.rejections.record(now) >= usize::try_from(CHAT_FLOOD_REJECTIONS).unwrap_or(20) {
            ChatVerdict::Flooding
        } else {
            ChatVerdict::RateLimited
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1_000_000;

    #[test]
    fn sliding_window() {
        let mut w = SlidingWindow::new(4, Duration::from_secs(1));
        for i in 0..4 {
            assert!(w.try_record(i));
        }
        assert!(!w.try_record(10));
        // At S the event at 0 has left the window, at S + 1 the one at 1.
        assert!(w.try_record(S));
        assert!(!w.try_record(S));
        assert!(w.try_record(S + 1));
        assert!(!w.try_record(S + 1));
        assert_eq!(w.count(S + 10), 2);
    }

    #[test]
    fn chat_bucket() {
        let mut c = ChatLimiter::new();
        for _ in 0..5 {
            assert_eq!(c.check(0), ChatVerdict::Allowed);
        }
        assert_eq!(c.check(0), ChatVerdict::RateLimited);
        assert_eq!(c.check(2 * S), ChatVerdict::Allowed);
        assert_eq!(c.check(2 * S), ChatVerdict::RateLimited);
        // 18 more rejections reach 20 in total within 60 s.
        for i in 0..17 {
            assert_eq!(c.check(2 * S + i), ChatVerdict::RateLimited);
        }
        assert_eq!(c.check(2 * S + 100), ChatVerdict::Flooding);
        // Sustained 1 per 2 s is allowed.
        let mut c = ChatLimiter::new();
        for i in 0..100 {
            assert_eq!(c.check(i * 2 * S), ChatVerdict::Allowed);
        }
    }
}
