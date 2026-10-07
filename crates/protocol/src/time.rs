// SPDX-License-Identifier: GPL-3.0-or-later
//! Time sync and the world clock (`tessera-protocol.md` §11).
//!
//! All times are microseconds. Client times are on the client's monotonic clock (any
//! epoch); server times on the server's clock (§11.1).

use std::collections::VecDeque;

use crate::limits::{
    MAX_OUTSTANDING_TIME_REQUESTS, TIME_BURST, TIME_BURST_INTERVAL, TIME_INTERVAL,
    TIME_MAX_SLEW_PER_SECOND, TIME_REPLY_MAX_AGE, TIME_SAMPLES, TIME_STEP_THRESHOLD,
};
use crate::message::{TimeReply, WorldClock};

fn micros(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// Server time at which tick `k` begins: ⌊k · 10⁶ / `tick_rate`⌋ µs (§11.1). A zero rate is
/// treated as 1 Hz.
#[must_use]
pub fn tick_start(tick: u64, tick_rate: u8) -> u64 {
    let rate = u128::from(tick_rate.max(1));
    u64::try_from(u128::from(tick) * 1_000_000 / rate).unwrap_or(u64::MAX)
}

/// The tick in progress at `server_time`: the largest *k* with `tick_start(k)` ≤
/// `server_time` (§11.1).
#[must_use]
pub fn tick_at(server_time: u64, tick_rate: u8) -> u64 {
    let rate = u128::from(tick_rate.max(1));
    let k = u64::try_from(u128::from(server_time) * rate / 1_000_000).unwrap_or(u64::MAX);
    // Flooring the start time can make tick k + 1 begin at or before server_time.
    if k < u64::MAX && tick_start(k + 1, tick_rate) <= server_time {
        k + 1
    } else {
        k
    }
}

/// One NTP-style sample (RFC 5905 §8, §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSample {
    /// θ = ((t₁ − t₀) + (t₂ − t₃)) / 2: server clock minus client clock, µs (rounded towards
    /// negative infinity).
    pub offset: i64,
    /// δ = (t₃ − t₀) − (t₂ − t₁): round-trip delay without the server's processing time, µs.
    pub delay: i64,
    /// t₃, when the reply arrived (client clock).
    pub received: u64,
}

impl TimeSample {
    /// Computes θ and δ from the four timestamps.
    #[must_use]
    pub fn compute(t0: u64, t1: u64, t2: u64, t3: u64) -> Self {
        let (t0, t1, t2, t3) = (
            i128::from(t0),
            i128::from(t1),
            i128::from(t2),
            i128::from(t3),
        );
        let offset = ((t1 - t0) + (t2 - t3)).div_euclid(2);
        let delay = (t3 - t0) - (t2 - t1);
        let clamp = |v: i128| i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX });
        Self {
            offset: clamp(offset),
            delay: clamp(delay),
            received: u64::try_from(t3).unwrap_or(0),
        }
    }
}

/// The client's clock-offset estimator (§11.2, recommended procedure).
///
/// Record every `TimeRequest` sent with [`on_request_sent`](Self::on_request_sent) and feed
/// it every `TimeReply` with its arrival time. Only replies to a request sent in the last
/// 10 s are used, each once. It keeps the last 8 samples, takes θ of the one with the
/// smallest δ, and steps or slews the offset in use so the estimated server time never runs
/// backwards for small corrections.
#[derive(Debug, Clone, Default)]
pub struct ClockOffsetEstimator {
    samples: VecDeque<TimeSample>,
    /// Offset in use, µs (fractional while slewing).
    current: Option<f64>,
    /// Client time of the last slew step.
    last_update: Option<u64>,
    /// Requests sent so far (for the schedule).
    sent: u32,
    /// Client time of the last request.
    last_sent: Option<u64>,
    /// t₀ of the requests sent in the last 10 s and not answered yet, oldest first (at most
    /// [`MAX_OUTSTANDING_TIME_REQUESTS`]).
    outstanding: VecDeque<u64>,
}

impl ClockOffsetEstimator {
    /// A fresh estimator with no samples.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a `TimeRequest` is due at client time `now`: 4 requests 250 ms apart after
    /// `HelloAck`, then one every 5 s.
    #[must_use]
    pub fn request_due(&self, now: u64) -> bool {
        match self.last_sent {
            None => true,
            Some(last) => {
                let interval = if self.sent < TIME_BURST {
                    TIME_BURST_INTERVAL
                } else {
                    TIME_INTERVAL
                };
                now.saturating_sub(last) >= micros(interval)
            }
        }
    }

    /// Records that a `TimeRequest` with `client_time` = `now` was sent. Its reply is
    /// accepted once within 10 s.
    pub fn on_request_sent(&mut self, now: u64) {
        self.sent = self.sent.saturating_add(1);
        self.last_sent = Some(now);
        self.expire(now);
        if self.outstanding.len() >= MAX_OUTSTANDING_TIME_REQUESTS {
            self.outstanding.pop_front();
        }
        self.outstanding.push_back(now);
    }

    /// Requests sent so far and not yet answered or expired at the last call.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding.len()
    }

    fn expire(&mut self, now: u64) {
        let max_age = micros(TIME_REPLY_MAX_AGE);
        while self
            .outstanding
            .front()
            .is_some_and(|&t0| now.saturating_sub(t0) > max_age)
        {
            self.outstanding.pop_front();
        }
    }

    /// Takes a reply received at client time `t3`. Replies whose t₀ is not a request sent in
    /// the last 10 s and not answered yet (unknown, duplicate, too old or in the future), and
    /// replies with δ < 0, are ignored (returns `None`, §11.2 step 2, §13.2). Otherwise the
    /// sample is kept and the offset in use is updated.
    pub fn on_reply(&mut self, reply: &TimeReply, t3: u64) -> Option<TimeSample> {
        self.expire(t3);
        if reply.client_time > t3 || t3 - reply.client_time > micros(TIME_REPLY_MAX_AGE) {
            return None;
        }
        let index = self
            .outstanding
            .iter()
            .position(|&t0| t0 == reply.client_time)?;
        self.outstanding.remove(index);
        let s = TimeSample::compute(
            reply.client_time,
            reply.server_receive,
            reply.server_send,
            t3,
        );
        if s.delay < 0 {
            return None;
        }
        if self.samples.len() == TIME_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
        self.update(t3);
        Some(s)
    }

    /// θ of the kept sample with the smallest δ, the newest on a tie (the target the
    /// estimate moves to).
    #[must_use]
    pub fn best_offset(&self) -> Option<i64> {
        self.samples
            .iter()
            .rev()
            .min_by_key(|s| s.delay)
            .map(|s| s.offset)
    }

    /// The smallest δ among the kept samples, µs.
    #[must_use]
    pub fn best_delay(&self) -> Option<i64> {
        self.samples.iter().map(|s| s.delay).min()
    }

    /// Advances the offset in use towards the best sample at client time `now`: a step when
    /// they differ by more than 100 ms, else a slew of at most 5 ms per second elapsed.
    pub fn update(&mut self, now: u64) {
        let Some(target) = self.best_offset() else {
            return;
        };
        let target = target as f64;
        let elapsed = self.last_update.map_or(0, |last| now.saturating_sub(last));
        self.last_update = Some(now);
        let current = match self.current {
            None => {
                self.current = Some(target);
                return;
            }
            Some(c) => c,
        };
        let diff = target - current;
        if diff.abs() > micros(TIME_STEP_THRESHOLD) as f64 {
            self.current = Some(target);
            return;
        }
        let max_step = micros(TIME_MAX_SLEW_PER_SECOND) as f64 * elapsed as f64 / 1e6;
        self.current = Some(current + diff.clamp(-max_step, max_step));
    }

    /// The offset in use (server − client), µs, once a sample exists.
    #[must_use]
    pub fn offset(&self) -> Option<i64> {
        self.current.map(|c| c.round() as i64)
    }

    /// Estimated server time at client time `client_now` (call [`update`](Self::update)
    /// first to keep slewing). `None` before the first sample, or if the result would be
    /// negative.
    #[must_use]
    pub fn server_time(&self, client_now: u64) -> Option<u64> {
        let off = self.offset()?;
        u64::try_from(i128::from(client_now) + i128::from(off)).ok()
    }
}

/// Server-side helper: builds the `TimeReply` for a request received at `t1` and answered
/// at `t2`, with the tick in progress at `t2` (§6.5.4). `t2` is raised to `t1` if earlier.
#[must_use]
pub fn time_reply(client_time: u64, t1: u64, t2: u64, tick_rate: u8) -> TimeReply {
    let t2 = t2.max(t1);
    TimeReply {
        client_time,
        server_receive: t1,
        server_send: t2,
        tick: tick_at(t2, tick_rate),
    }
}

impl WorldClock {
    /// The game clock *G* at server time `s` (§11.3): `game_clock` + `rate` · (s −
    /// `server_time`) / 10⁶ seconds. Never negative.
    #[must_use]
    pub fn game_clock_at(&self, s: u64) -> f64 {
        let dt = (i128::from(s) - i128::from(self.server_time)) as f64 / 1e6;
        (self.game_clock + f64::from(self.rate) * dt).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example() {
        // §16.5: t0 1,000,000, t1 5,020,000, t2 5,020,100, t3 1,040,100.
        let s = TimeSample::compute(1_000_000, 5_020_000, 5_020_100, 1_040_100);
        assert_eq!(s.offset, 4_000_000);
        assert_eq!(s.delay, 40_000);
    }

    #[test]
    fn ticks() {
        assert_eq!(tick_start(1, 30), 33_333);
        assert_eq!(tick_at(33_332, 30), 0);
        assert_eq!(tick_at(33_333, 30), 1);
        assert_eq!(tick_at(5_020_100, 30), 150);
        for t in [
            0u64, 1, 33_333, 66_666, 66_667, 999_999, 1_000_000, 12_345_678,
        ] {
            let k = tick_at(t, 30);
            assert!(tick_start(k, 30) <= t && tick_start(k + 1, 30) > t, "{t}");
        }
        assert_eq!(time_reply(1, 10, 5, 30).server_send, 10);
    }

    #[test]
    fn estimator_steps_then_slews() {
        let mut e = ClockOffsetEstimator::new();
        assert!(e.request_due(0));
        e.on_request_sent(0);
        assert!(!e.request_due(100_000));
        assert!(e.request_due(250_000));
        let r = TimeReply {
            client_time: 1_000_000,
            server_receive: 5_020_000,
            server_send: 5_020_100,
            tick: 150,
        };
        e.on_request_sent(1_000_000);
        assert!(e.on_reply(&r, 1_040_100).is_some());
        assert_eq!(e.offset(), Some(4_000_000));
        assert_eq!(e.server_time(2_000_000), Some(6_000_000));
        // A better sample 3 ms away: slewed at 5 ms/s.
        let r2 = TimeReply {
            client_time: 2_000_000,
            server_receive: 6_003_000 + 5_000,
            server_send: 6_003_000 + 5_000,
            tick: 0,
        };
        e.on_request_sent(2_000_000);
        let s = e.on_reply(&r2, 2_010_000).unwrap();
        assert_eq!(s.delay, 10_000);
        assert_eq!(s.offset, 4_003_000);
        // 0.96957 s elapsed since the first update: at most ~4.85 ms of slew, so it reaches
        // the target of +3 ms.
        assert_eq!(e.offset(), Some(4_003_000));
        // Too old, future and negative-delay replies are ignored.
        e.on_request_sent(1_000_000);
        assert!(e.on_reply(&r, 20_000_000).is_none());
        e.on_request_sent(1_000_000);
        assert!(e.on_reply(&r, 10).is_none());
        let neg = TimeReply {
            client_time: 0,
            server_receive: 100,
            server_send: 1_000,
            tick: 0,
        };
        e.on_request_sent(0);
        assert!(e.on_reply(&neg, 10).is_none());
    }

    #[test]
    fn slew_is_bounded() {
        let mut e = ClockOffsetEstimator::new();
        let at = |t0: u64, off: u64| TimeReply {
            client_time: t0,
            server_receive: t0 + off,
            server_send: t0 + off,
            tick: 0,
        };
        e.on_request_sent(0);
        e.on_reply(&at(0, 1_000_000), 0);
        assert_eq!(e.offset(), Some(1_000_000));
        // 50 ms better sample one tenth of a second later: slew 0.5 ms.
        e.on_request_sent(100_000);
        e.on_reply(&at(100_000, 1_050_000), 100_000);
        assert_eq!(e.offset(), Some(1_000_500));
        // 200 ms jump: step.
        let mut e2 = ClockOffsetEstimator::new();
        e2.on_request_sent(0);
        e2.on_reply(&at(0, 1_000_000), 0);
        e2.on_request_sent(100_000);
        e2.on_reply(&at(100_000, 1_200_000 + 1), 100_000);
        assert_eq!(e2.offset(), Some(1_200_001));
    }

    #[test]
    fn only_replies_to_sent_requests_count() {
        let reply = |t0: u64| TimeReply {
            client_time: t0,
            server_receive: 9_000_000,
            server_send: 9_000_000,
            tick: 0,
        };
        // Nothing sent: an unsolicited reply is not a sample.
        let mut e = ClockOffsetEstimator::new();
        assert!(e.on_reply(&reply(1_000), 2_000).is_none());
        assert_eq!(e.offset(), None);
        // A t0 never sent is refused; the real one is taken once, its duplicate is refused.
        e.on_request_sent(1_000_000);
        assert!(e.on_reply(&reply(1_000_001), 1_050_000).is_none());
        assert!(e.on_reply(&reply(1_000_000), 1_050_000).is_some());
        assert!(e.on_reply(&reply(1_000_000), 1_060_000).is_none());
        assert_eq!(e.outstanding(), 0);
        // Requests older than 10 s are forgotten.
        e.on_request_sent(2_000_000);
        e.on_request_sent(12_500_000);
        assert_eq!(e.outstanding(), 1);
        assert!(e.on_reply(&reply(2_000_000), 12_600_000).is_none());
        // The list is bounded.
        for i in 0..1_000u64 {
            e.on_request_sent(13_000_000 + i);
        }
        assert_eq!(e.outstanding(), MAX_OUTSTANDING_TIME_REQUESTS);
        assert!(e.on_reply(&reply(13_000_000), 13_100_000).is_none());
        assert!(e.on_reply(&reply(13_000_999), 13_100_000).is_some());
    }

    #[test]
    fn world_clock() {
        let c = WorldClock {
            server_time: 5_000_000,
            game_clock: 43_200.0,
            rate: 2.0,
        };
        assert_eq!(c.game_clock_at(6_000_000), 43_202.0);
        assert_eq!(c.game_clock_at(4_000_000), 43_198.0);
    }
}
