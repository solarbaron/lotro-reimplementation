// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed-timestep accumulator (ARCH §20.3: simulation at a fixed rate,
//! interpolated for rendering).

/// Default simulation rate in ticks per second (ARCH §20.3).
pub const DEFAULT_TICK_RATE: f64 = 30.0;
/// Default most ticks run for one frame; excess time is dropped so a long
/// stall does not cause a burst of catch-up ticks.
pub const DEFAULT_MAX_TICKS_PER_FRAME: u32 = 8;

/// Splits variable frame times into whole fixed ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedTimestep {
    dt: f64,
    accumulator: f64,
    max_ticks: u32,
    dropped: f64,
}

impl Default for FixedTimestep {
    fn default() -> Self {
        Self::new(DEFAULT_TICK_RATE)
    }
}

impl FixedTimestep {
    /// A timestep running at `rate` ticks per second (clamped to
    /// `1..=1000`; non-finite means the default).
    #[must_use]
    pub fn new(rate: f64) -> Self {
        let rate = if rate.is_finite() {
            rate.clamp(1.0, 1000.0)
        } else {
            DEFAULT_TICK_RATE
        };
        Self {
            dt: 1.0 / rate,
            accumulator: 0.0,
            max_ticks: DEFAULT_MAX_TICKS_PER_FRAME,
            dropped: 0.0,
        }
    }

    /// Same timestep with another per-frame tick cap (at least 1).
    #[must_use]
    pub fn with_max_ticks(mut self, max: u32) -> Self {
        self.max_ticks = max.max(1);
        self
    }

    /// Seconds per tick.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Add a frame's elapsed seconds and return how many ticks to run now.
    /// Negative or non-finite frame times count as zero.
    pub fn advance(&mut self, frame_seconds: f64) -> u32 {
        let t = if frame_seconds.is_finite() {
            frame_seconds.max(0.0)
        } else {
            0.0
        };
        self.accumulator += t;
        // Tolerate rounding so that e.g. three frames of 1/90 s make one
        // 1/30 s tick.
        let eps = self.dt * 1e-9;
        let whole = ((self.accumulator + eps) / self.dt).floor();
        let ticks = if whole >= f64::from(self.max_ticks) {
            // `whole` may have been rounded up to the cap by `eps`, which
            // makes `keep` a hair negative; a negative remainder would
            // otherwise wrap to almost a whole tick.
            let keep = (self.accumulator - f64::from(self.max_ticks) * self.dt).max(0.0);
            // Drop the excess, keep only a fraction of a tick.
            let frac = keep.rem_euclid(self.dt);
            self.dropped += keep - frac;
            self.accumulator = frac + f64::from(self.max_ticks) * self.dt;
            self.max_ticks
        } else {
            whole as u32
        };
        self.accumulator = (self.accumulator - f64::from(ticks) * self.dt).max(0.0);
        ticks
    }

    /// Fraction of a tick accumulated but not yet simulated, in `0..1`;
    /// render with `previous.lerp(current, alpha)`.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        ((self.accumulator / self.dt).clamp(0.0, 1.0)) as f32
    }

    /// Total seconds dropped because of the per-frame cap.
    #[must_use]
    pub fn dropped_seconds(&self) -> f64 {
        self.dropped
    }

    /// Forget accumulated time (after a pause or a teleport).
    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_ticks() {
        let mut s = FixedTimestep::new(30.0);
        let mut total = 0;
        for _ in 0..90 {
            total += s.advance(1.0 / 90.0);
        }
        assert_eq!(total, 30);
        let mut s = FixedTimestep::new(30.0);
        assert_eq!(s.advance(0.05), 1);
        assert!((s.alpha() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn caps_and_hostile_values() {
        let mut s = FixedTimestep::new(30.0).with_max_ticks(4);
        assert_eq!(s.advance(10.0), 4);
        assert!(s.alpha() < 1.0);
        assert!(s.dropped_seconds() > 9.0);
        assert_eq!(s.advance(f64::NAN), 0);
        assert_eq!(s.advance(-5.0), 0);
        assert_eq!(s.advance(f64::INFINITY), 0);
        // At the cap with the rounding tolerance: no time is created.
        let mut s = FixedTimestep::new(30.0);
        let dt = s.dt();
        assert_eq!(s.advance(8.0 * dt - 1e-12), 8);
        assert!(s.dropped_seconds() >= 0.0, "{}", s.dropped_seconds());
        assert!(s.alpha() < 1e-6);
        assert_eq!(s.advance(0.0), 0, "no phantom tick");
        for k in 0..2000 {
            let frame = 8.0 * dt + (f64::from(k) - 1000.0) * 1e-13;
            s.advance(frame);
            assert!(s.dropped_seconds() >= 0.0 && s.alpha() <= 1.0);
        }
        let s = FixedTimestep::new(f64::NAN);
        assert!((s.dt() - 1.0 / 30.0).abs() < 1e-12);
        let mut s = FixedTimestep::new(0.0);
        assert_eq!(s.advance(1.0), 1);
        s.reset();
        assert_eq!(s.alpha(), 0.0);
    }
}
