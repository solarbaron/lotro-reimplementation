// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoded audio shared between [`crate::decode`] (producer) and [`crate::engine`]
//! (consumer).
//!
//! This is the planner's interface contract so that the decoder and the mixer can be built
//! in parallel. The `audio-decode` task owns this file and may extend it additively; the
//! `audio-engine` task consumes it.
//!
//! Samples are `f32` in [−1, 1], interleaved by channel (1 or 2 channels in the data,
//! `sound.md` §3.3–§3.4).

use std::sync::Arc;

/// A whole decoded sound: the played range only (for Ogg, `sound.md` §3.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    /// Frames per second.
    pub sample_rate: u32,
    /// Interleaved channels per frame (1 or 2).
    pub channels: u16,
    /// Interleaved samples; `samples.len()` is a multiple of `channels`.
    pub samples: Arc<[f32]>,
}

impl Pcm {
    /// Number of frames (samples per channel).
    #[must_use]
    pub fn frames(&self) -> usize {
        match self.channels {
            0 => 0,
            c => self.samples.len() / usize::from(c),
        }
    }
}

/// A streamed decoded sound (long music tracks, ARCH §14). Implementations decode
/// incrementally and never block on I/O; they own their payload bytes.
pub trait PcmSource: Send + std::fmt::Debug {
    /// Frames per second.
    fn sample_rate(&self) -> u32;
    /// Interleaved channels per frame (1 or 2).
    fn channels(&self) -> u16;
    /// Total frames of the played range, when known.
    fn len_frames(&self) -> Option<u64>;
    /// Fills `out` (a multiple of [`Self::channels`] long) with interleaved samples and
    /// returns the number of **frames** written. 0 means the end of the played range (or a
    /// decode fault, which the implementation logs).
    fn read(&mut self, out: &mut [f32]) -> usize;
    /// Restarts at the first frame of the played range (looping, `sound.md` §6.4). Returns
    /// false when the source cannot restart, which includes a played range of 0 frames
    /// (`len_frames() == Some(0)`): such a source has nothing to loop over.
    ///
    /// A looping consumer must also stop when a [`Self::read`] right after a successful
    /// rewind returns 0 (a stream that faults on its first packet), or it would spin.
    fn rewind(&mut self) -> bool;
}

impl Pcm {
    /// Duration in seconds (0 for a sample rate of 0).
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        match self.sample_rate {
            0 => 0.0,
            r => self.frames() as f64 / f64::from(r),
        }
    }
}

/// A [`PcmSource`] over a whole [`Pcm`] buffer: delivers its frames in order;
/// [`PcmSource::rewind`] restarts at frame 0 (looping over the whole buffer, `sound.md`
/// §6.4), except for an empty buffer, which cannot be rewound.
#[derive(Debug, Clone)]
pub struct PcmCursor {
    pcm: Pcm,
    /// Next frame to deliver.
    frame: usize,
}

impl PcmCursor {
    /// A cursor at the first frame of `pcm`.
    #[must_use]
    pub fn new(pcm: Pcm) -> Self {
        Self { pcm, frame: 0 }
    }

    /// The buffer.
    #[must_use]
    pub fn pcm(&self) -> &Pcm {
        &self.pcm
    }

    /// Next frame to deliver.
    #[must_use]
    pub fn position(&self) -> usize {
        self.frame
    }
}

impl PcmSource for PcmCursor {
    fn sample_rate(&self) -> u32 {
        self.pcm.sample_rate
    }

    fn channels(&self) -> u16 {
        self.pcm.channels
    }

    fn len_frames(&self) -> Option<u64> {
        Some(self.pcm.frames() as u64)
    }

    fn read(&mut self, out: &mut [f32]) -> usize {
        let ch = usize::from(self.pcm.channels);
        if ch == 0 {
            return 0;
        }
        let n = (out.len() / ch).min(self.pcm.frames().saturating_sub(self.frame));
        let from = self.frame * ch;
        out[..n * ch].copy_from_slice(&self.pcm.samples[from..from + n * ch]);
        self.frame += n;
        n
    }

    fn rewind(&mut self) -> bool {
        if self.pcm.frames() == 0 {
            return false;
        }
        self.frame = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_reads_and_rewinds() {
        let pcm = Pcm {
            sample_rate: 10,
            channels: 2,
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6].into(),
        };
        assert!((pcm.duration_secs() - 0.3).abs() < 1e-9);
        let mut c = PcmCursor::new(pcm);
        let mut out = [0.0f32; 5];
        assert_eq!(c.read(&mut out), 2);
        assert_eq!(&out[..4], &[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(c.read(&mut out), 1);
        assert_eq!(c.read(&mut out), 0);
        assert!(c.rewind());
        assert_eq!(c.read(&mut out), 2);
    }

    #[test]
    fn empty_cursor_cannot_rewind() {
        let mut c = PcmCursor::new(Pcm {
            sample_rate: 10,
            channels: 1,
            samples: Vec::new().into(),
        });
        let mut out = [0.0f32; 4];
        assert_eq!(c.read(&mut out), 0);
        assert!(!c.rewind());
    }
}
