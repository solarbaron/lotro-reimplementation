// SPDX-License-Identifier: GPL-3.0-or-later
//! A bot's true trajectory (`test-server.md` §13.2): the positions it simulated (and sent),
//! stamped with server time, so an observer's interpolated view at its render time can be
//! compared with where the bot really was at that moment.

use std::collections::VecDeque;

/// How long a trajectory keeps its samples, µs (render time lags real time by the
/// interpolation delay, about 0.15–0.2 s; this leaves ample room).
pub const KEEP_US: u64 = 5_000_000;
/// Most samples kept (30 Hz × 5 s plus a margin); older ones are dropped first.
pub const MAX_SAMPLES: usize = 512;

/// One sample of the truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruthSample {
    /// Server time, µs.
    pub t_us: u64,
    /// Region.
    pub region: u16,
    /// Region-global position, metres.
    pub x: f64,
    /// Region-global position, metres.
    pub y: f64,
    /// Height, metres.
    pub z: f64,
    /// Facing, radians.
    pub heading: f32,
    /// Horizontal speed, m/s.
    pub speed: f64,
    /// The position jumped here (teleport, correction, world entry): never interpolate
    /// across this sample's start.
    pub jump: bool,
    /// Server time since which the bot has been at rest at this position, if it is.
    pub rest_since_us: Option<u64>,
}

/// The truth at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruthAt {
    /// Region.
    pub region: u16,
    /// Position, metres.
    pub x: f64,
    /// Position, metres.
    pub y: f64,
    /// Facing, radians (the sample's, never blended).
    pub heading: f32,
    /// Horizontal speed, m/s.
    pub speed: f64,
    /// Server time since which the bot has been at rest, if it is.
    pub rest_since_us: Option<u64>,
}

/// A bounded, time-ordered list of truth samples.
#[derive(Debug, Clone, Default)]
pub struct Trajectory {
    samples: VecDeque<TruthSample>,
}

impl Trajectory {
    /// An empty trajectory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets everything (the bot left the world).
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Number of samples kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether no sample is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Appends a sample. Samples must come in server-time order; one that does not (the
    /// clock estimate stepped back) replaces the samples it overlaps. `rest_since_us` is
    /// derived here: the bot is at rest when its speed is zero and it has not moved since
    /// the previous sample.
    pub fn record(&mut self, mut s: TruthSample) {
        while self.samples.back().is_some_and(|b| b.t_us > s.t_us) {
            self.samples.pop_back();
        }
        let resting = s.speed == 0.0 && !s.jump;
        s.rest_since_us = match self.samples.back() {
            Some(b) if resting && b.region == s.region && b.x == s.x && b.y == s.y => {
                Some(b.rest_since_us.unwrap_or(s.t_us))
            }
            _ if resting => Some(s.t_us),
            _ => None,
        };
        if s.jump {
            s.rest_since_us = None;
        }
        self.samples.push_back(s);
        while self.samples.len() > MAX_SAMPLES
            || self
                .samples
                .front()
                .is_some_and(|f| s.t_us.saturating_sub(f.t_us) > KEEP_US)
        {
            self.samples.pop_front();
        }
    }

    /// The truth at server time `t_us`: linear between the samples around it (a step at a
    /// jump), the newest sample after the end. `None` before the first sample.
    #[must_use]
    pub fn at(&self, t_us: u64) -> Option<TruthAt> {
        let idx = self.samples.partition_point(|s| s.t_us <= t_us);
        let a = self.samples.get(idx.checked_sub(1)?)?;
        let out = |s: &TruthSample, x: f64, y: f64| TruthAt {
            region: s.region,
            x,
            y,
            heading: s.heading,
            speed: s.speed,
            rest_since_us: s.rest_since_us,
        };
        let Some(b) = self.samples.get(idx) else {
            return Some(out(a, a.x, a.y));
        };
        if b.jump || b.region != a.region || b.t_us <= a.t_us {
            return Some(out(a, a.x, a.y));
        }
        let f = (t_us - a.t_us) as f64 / (b.t_us - a.t_us) as f64;
        Some(out(a, a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f))
    }

    /// The newest sample.
    #[must_use]
    pub fn newest(&self) -> Option<&TruthSample> {
        self.samples.back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: u64, x: f64, speed: f64, jump: bool) -> TruthSample {
        TruthSample {
            t_us: t,
            region: 0,
            x,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            speed,
            jump,
            rest_since_us: None,
        }
    }

    #[test]
    fn interpolates_and_steps() {
        let mut tr = Trajectory::new();
        assert!(tr.at(0).is_none());
        tr.record(s(1_000_000, 0.0, 2.0, true));
        tr.record(s(1_100_000, 0.2, 2.0, false));
        tr.record(s(1_200_000, 50.0, 0.0, true));
        assert!(tr.at(999_999).is_none());
        let m = tr.at(1_050_000).unwrap();
        assert!((m.x - 0.1).abs() < 1e-9);
        // A jump is a step, not a blend.
        assert_eq!(tr.at(1_150_000).unwrap().x, 0.2);
        assert_eq!(tr.at(1_200_000).unwrap().x, 50.0);
        assert_eq!(tr.at(9_000_000).unwrap().x, 50.0);
    }

    #[test]
    fn rest_since() {
        let mut tr = Trajectory::new();
        tr.record(s(0, 1.0, 1.0, false));
        tr.record(s(100, 2.0, 0.0, false));
        tr.record(s(200, 2.0, 0.0, false));
        tr.record(s(300, 2.0, 0.0, false));
        assert_eq!(tr.at(300).unwrap().rest_since_us, Some(100));
        tr.record(s(400, 3.0, 0.0, false));
        assert_eq!(tr.at(400).unwrap().rest_since_us, Some(400));
        tr.record(s(500, 4.0, 1.0, false));
        assert_eq!(tr.at(500).unwrap().rest_since_us, None);
    }

    #[test]
    fn bounded_and_reordered() {
        let mut tr = Trajectory::new();
        for i in 0..10_000u64 {
            tr.record(s(i * 1_000, i as f64, 1.0, false));
        }
        assert!(tr.len() <= MAX_SAMPLES);
        tr.record(s(5, 0.0, 1.0, false));
        assert_eq!(tr.len(), 1);
        tr.clear();
        assert!(tr.is_empty());
    }
}
