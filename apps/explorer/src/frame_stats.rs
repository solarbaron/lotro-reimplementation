// SPDX-License-Identifier: GPL-3.0-or-later
//! Frame-time bookkeeping for the overlay and the slow-frame log (M1 exit criterion 4:
//! no frame over 50 ms).

use std::collections::VecDeque;
use std::time::Duration;

/// A frame slower than this is logged and counted.
pub const SLOW_FRAME: Duration = Duration::from_millis(50);

/// Frames kept for the overlay graph.
pub const FRAME_HISTORY: usize = 240;

/// Frames at start-up that are counted separately (window creation, font atlas).
pub const WARMUP_FRAMES: u64 = 5;

/// Rolling frame-time statistics.
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    recent: VecDeque<f32>,
    frames: u64,
    slow: u64,
    slow_warmup: u64,
    worst_ms: f32,
}

impl FrameStats {
    /// Empty statistics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one frame's duration. Returns `true` when it was slow (over
    /// [`SLOW_FRAME`]).
    pub fn record(&mut self, frame: Duration) -> bool {
        let ms = frame.as_secs_f32() * 1000.0;
        self.frames += 1;
        if self.recent.len() == FRAME_HISTORY {
            self.recent.pop_front();
        }
        self.recent.push_back(ms);
        let slow = frame > SLOW_FRAME;
        if slow {
            if self.frames <= WARMUP_FRAMES {
                self.slow_warmup += 1;
            } else {
                self.slow += 1;
            }
        }
        if self.frames > WARMUP_FRAMES && ms > self.worst_ms {
            self.worst_ms = ms;
        }
        slow
    }

    /// Whether the frame just recorded is still in the warm-up phase.
    #[must_use]
    pub fn in_warmup(&self) -> bool {
        self.frames <= WARMUP_FRAMES
    }

    /// The last frame's duration in milliseconds.
    #[must_use]
    pub fn last_ms(&self) -> f32 {
        self.recent.back().copied().unwrap_or(0.0)
    }

    /// The longest frame in the recent window, in milliseconds.
    #[must_use]
    pub fn recent_max_ms(&self) -> f32 {
        self.recent.iter().copied().fold(0.0, f32::max)
    }

    /// The mean of the recent window, in milliseconds.
    #[must_use]
    pub fn recent_mean_ms(&self) -> f32 {
        if self.recent.is_empty() {
            0.0
        } else {
            self.recent.iter().sum::<f32>() / self.recent.len() as f32
        }
    }

    /// The longest frame after warm-up, in milliseconds.
    #[must_use]
    pub fn worst_ms(&self) -> f32 {
        self.worst_ms
    }

    /// Frames recorded.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Slow frames after warm-up.
    #[must_use]
    pub fn slow_frames(&self) -> u64 {
        self.slow
    }

    /// Slow frames during warm-up.
    #[must_use]
    pub fn slow_warmup_frames(&self) -> u64 {
        self.slow_warmup
    }

    /// The recent frame times in milliseconds, oldest first.
    pub fn recent(&self) -> impl ExactSizeIterator<Item = f32> + '_ {
        self.recent.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_slow_frames_after_warmup() {
        let mut s = FrameStats::new();
        for _ in 0..WARMUP_FRAMES {
            assert!(s.record(Duration::from_millis(80)));
        }
        assert_eq!(s.slow_warmup_frames(), WARMUP_FRAMES);
        assert_eq!(s.slow_frames(), 0);
        assert_eq!(s.worst_ms(), 0.0);
        assert!(!s.record(Duration::from_millis(10)));
        assert!(s.record(Duration::from_millis(51)));
        assert!(!s.record(SLOW_FRAME));
        assert_eq!(s.slow_frames(), 1);
        assert!((s.worst_ms() - 51.0).abs() < 0.01);
        assert!((s.last_ms() - 50.0).abs() < 0.01);
    }

    #[test]
    fn window_is_bounded() {
        let mut s = FrameStats::new();
        for i in 0..(FRAME_HISTORY * 2) {
            s.record(Duration::from_micros(i as u64));
        }
        assert_eq!(s.recent().len(), FRAME_HISTORY);
        assert!(s.recent_max_ms() > 0.0);
        assert!(s.recent_mean_ms() > 0.0);
        assert_eq!(s.frames(), (FRAME_HISTORY * 2) as u64);
    }
}
