// SPDX-License-Identifier: GPL-3.0-or-later
//! The audio-thread side of a voice: reads its source at its playback rate (pitch × source
//! rate / output rate, linear interpolation), loops, applies the gains the controller
//! sends, starts and stops at scheduled audio-clock frames, and fades out linearly
//! (`docs/specs/sound.md` §6.3–§6.5, `instruments.md` §9.2, §12).
//!
//! Nothing here allocates, locks or frees: voices are built on the main thread, moved to
//! the audio thread through the command queue and moved back through the event queue to be
//! dropped on the main thread.

use std::sync::atomic::Ordering;

use super::stream::StreamReader;
use crate::pcm::Pcm;

/// Frames over which a gain change is spread (de-zippering). A voice's first gain applies
/// at once: there is no fade-in (`sound.md` §6.5).
pub(crate) const GAIN_SMOOTH_FRAMES: u32 = 64;

/// Upper bound of the read step (source frames per output frame).
const MAX_STEP: f64 = 64.0;

/// Why a voice ended on the audio thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndReason {
    /// The source ran out (non-looping), or a stream ended.
    Ended,
    /// A stop (with or without ramp-out) completed.
    Stopped,
}

#[derive(Debug)]
enum Source {
    Pcm {
        pcm: Pcm,
        frames: usize,
        pos: f64,
    },
    Stream {
        reader: StreamReader,
        a: [f32; 2],
        b: [f32; 2],
        frac: f64,
        primed: bool,
        advance: u64,
        draining: bool,
    },
}

/// What [`Voice::next_frame`] produced.
enum Next {
    Frame(f32, f32),
    Starved,
    End,
}

/// A voice as the mixer sees it.
#[derive(Debug)]
pub(crate) struct Voice {
    pub id: u64,
    source: Source,
    channels: usize,
    step: f64,
    looping: bool,
    /// Gains: left, right, reverb send.
    cur: [f32; 3],
    target: [f32; 3],
    inc: [f32; 3],
    smooth_left: u32,
    pub paused: bool,
    started: bool,
    start_at: u64,
    stop_at: Option<(u64, u64)>,
    ramp: Option<Ramp>,
    pub end: Option<EndReason>,
    /// Starved blocks (stream ring empty while the stream is not over).
    pub starved_blocks: u32,
}

#[derive(Debug, Clone, Copy)]
struct Ramp {
    start: [f32; 3],
    total: u64,
    elapsed: u64,
}

/// Parameters of a new voice.
#[derive(Debug)]
pub(crate) struct VoiceInit {
    pub id: u64,
    pub step: f64,
    pub looping: bool,
    /// Left, right and reverb-send gain. The send gain is the voice's own gain (0 for a
    /// voice that does not feed the reverb), whatever the pan (`sound.md` §8 OPEN).
    pub gains: [f32; 3],
    pub paused: bool,
    pub start_at: u64,
}

impl Voice {
    /// A voice over a whole buffer; `None` when it has no playable frame.
    pub fn from_pcm(pcm: Pcm, init: VoiceInit) -> Option<Self> {
        let channels = usize::from(pcm.channels);
        if !(1..=2).contains(&channels) || pcm.sample_rate == 0 {
            return None;
        }
        let frames = pcm.frames();
        if frames == 0 {
            return None;
        }
        Some(Self::new(
            Source::Pcm {
                pcm,
                frames,
                pos: 0.0,
            },
            channels,
            init,
        ))
    }

    /// A voice over a stream with `channels` channels (1 or 2).
    pub fn from_stream(reader: StreamReader, channels: usize, init: VoiceInit) -> Self {
        Self::new(
            Source::Stream {
                reader,
                a: [0.0; 2],
                b: [0.0; 2],
                frac: 0.0,
                primed: false,
                advance: 0,
                draining: false,
            },
            channels.clamp(1, 2),
            init,
        )
    }

    fn new(source: Source, channels: usize, init: VoiceInit) -> Self {
        let step = if init.step.is_finite() {
            init.step.clamp(1e-6, MAX_STEP)
        } else {
            1.0
        };
        Self {
            id: init.id,
            source,
            channels,
            step,
            looping: init.looping,
            cur: init.gains,
            target: init.gains,
            inc: [0.0; 3],
            smooth_left: 0,
            paused: init.paused,
            started: false,
            start_at: init.start_at,
            stop_at: None,
            ramp: None,
            end: None,
            starved_blocks: 0,
        }
    }

    /// New gains (left, right, reverb send) and state from the controller.
    pub fn set_params(&mut self, gains: [f32; 3], paused: bool) {
        self.paused = paused;
        if self.ramp.is_some() {
            // A fade-out runs from the gain it started with (§6.5). A voice virtualised
            // while it fades stops at once (§6.5: "when the voice is virtualised, it stops
            // at once"); otherwise the paused ramp would never advance.
            if paused {
                self.end = Some(EndReason::Stopped);
            }
            return;
        }
        if !self.started {
            self.cur = gains;
            self.target = gains;
            self.smooth_left = 0;
            return;
        }
        self.target = gains;
        let n = GAIN_SMOOTH_FRAMES as f32;
        self.inc = [0, 1, 2].map(|k| (gains[k] - self.cur[k]) / n);
        self.smooth_left = GAIN_SMOOTH_FRAMES;
    }

    /// Stops the voice: at frame `at` (or at once), fading over `ramp_frames`.
    pub fn stop(&mut self, ramp_frames: u64, at: Option<u64>, now: u64) {
        match at {
            Some(t) if t > now => self.stop_at = Some((t, ramp_frames)),
            _ => self.begin_ramp(ramp_frames),
        }
    }

    fn begin_ramp(&mut self, frames: u64) {
        if self.ramp.is_some() {
            // Already fading: a further stop keeps that fade, unless the voice is paused,
            // where the fade cannot advance and the voice ends at once.
            if self.paused {
                self.end = Some(EndReason::Stopped);
            }
            return;
        }
        if frames == 0 || self.paused || !self.started {
            self.end = Some(EndReason::Stopped);
            return;
        }
        self.ramp = Some(Ramp {
            start: self.cur,
            total: frames,
            elapsed: 0,
        });
    }

    /// Mixes into `l`, `r` (and the reverb `send`) for the block starting at clock `t0`.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32], send: &mut [f32], t0: u64) {
        let n = l.len().min(r.len()).min(send.len());
        let mut i = 0usize;
        if self.start_at > t0 {
            let skip = self.start_at - t0;
            if skip >= n as u64 {
                if let Some((at, _)) = self.stop_at
                    && at < t0 + n as u64
                {
                    // Stopped before it started.
                    self.end = Some(EndReason::Stopped);
                }
                return;
            }
            i = skip as usize;
        }
        let mut starved = false;
        while i < n {
            if let Some((at, frames)) = self.stop_at
                && t0 + i as u64 >= at
            {
                self.stop_at = None;
                self.begin_ramp(frames);
            }
            if self.end.is_some() || self.paused {
                break;
            }
            let (sl, sr) = match self.next_frame() {
                Next::Frame(a, b) => (a, b),
                Next::Starved => {
                    starved = true;
                    break;
                }
                Next::End => {
                    self.end = Some(EndReason::Ended);
                    break;
                }
            };
            self.started = true;
            let g = if let Some(ramp) = &mut self.ramp {
                if ramp.elapsed >= ramp.total {
                    self.end = Some(EndReason::Stopped);
                    break;
                }
                let m = 1.0 - ramp.elapsed as f32 / ramp.total as f32;
                ramp.elapsed += 1;
                ramp.start.map(|g| g * m)
            } else {
                if self.smooth_left > 0 {
                    self.smooth_left -= 1;
                    if self.smooth_left == 0 {
                        self.cur = self.target;
                    } else {
                        for (c, d) in self.cur.iter_mut().zip(self.inc) {
                            *c += d;
                        }
                    }
                }
                self.cur
            };
            l[i] += sl * g[0];
            r[i] += sr * g[1];
            send[i] += 0.5 * (sl + sr) * g[2];
            i += 1;
        }
        if starved {
            self.starved_blocks = self.starved_blocks.saturating_add(1);
        }
        // A fade that completed exactly at the block end.
        if let Some(ramp) = &self.ramp
            && ramp.elapsed >= ramp.total
        {
            self.end = Some(EndReason::Stopped);
        }
    }

    /// The next output frame as (left, right) source values (mono duplicated).
    fn next_frame(&mut self) -> Next {
        let ch = self.channels;
        let step = self.step;
        let looping = self.looping;
        match &mut self.source {
            Source::Pcm { pcm, frames, pos } => {
                let frames = *frames;
                if *pos >= frames as f64 {
                    if !looping {
                        return Next::End;
                    }
                    *pos %= frames as f64;
                }
                let i = (*pos as usize).min(frames - 1);
                let frac = (*pos - i as f64) as f32;
                let s = &pcm.samples;
                let j = if i + 1 < frames {
                    i + 1
                } else if looping {
                    0
                } else {
                    i
                };
                let (a, b) = if ch == 1 {
                    let (x, y) = (s[i], s[j]);
                    let v = x + (y - x) * frac;
                    (v, v)
                } else {
                    let (xl, xr, yl, yr) = (s[2 * i], s[2 * i + 1], s[2 * j], s[2 * j + 1]);
                    (xl + (yl - xl) * frac, xr + (yr - xr) * frac)
                };
                *pos += step;
                if looping && *pos >= frames as f64 {
                    *pos %= frames as f64;
                }
                Next::Frame(a, b)
            }
            Source::Stream {
                reader,
                a,
                b,
                frac,
                primed,
                advance,
                draining,
            } => {
                if !*primed {
                    let need = 2 * ch;
                    let avail = reader.consumer.slots();
                    if avail < need {
                        if !reader.shared.ended.load(Ordering::Acquire) {
                            return Next::Starved;
                        }
                        if reader.consumer.slots() < ch {
                            return Next::End;
                        }
                        *a = pop_frame(reader, ch).unwrap_or([0.0; 2]);
                        *b = *a;
                    } else {
                        *a = pop_frame(reader, ch).unwrap_or([0.0; 2]);
                        *b = pop_frame(reader, ch).unwrap_or(*a);
                    }
                    *primed = true;
                }
                while *advance > 0 {
                    match pop_frame(reader, ch) {
                        Some(f) => {
                            *a = *b;
                            *b = f;
                            *advance -= 1;
                        }
                        None => {
                            if reader.shared.ended.load(Ordering::Acquire) {
                                // Re-check after the flag (the feeder commits, then sets it).
                                if reader.consumer.slots() >= ch {
                                    continue;
                                }
                                if *draining {
                                    return Next::End;
                                }
                                // `b` is the last frame: hold it for one more output frame.
                                *a = *b;
                                *draining = true;
                                *advance -= 1;
                                continue;
                            }
                            return Next::Starved;
                        }
                    }
                }
                let t = *frac as f32;
                let out = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                *frac += step;
                let adv = frac.floor();
                *frac -= adv;
                *advance = adv as u64;
                Next::Frame(out.0, out.1)
            }
        }
    }

    /// Whether the voice is a stream (for tests and stats).
    #[cfg(test)]
    pub fn is_stream(&self) -> bool {
        matches!(self.source, Source::Stream { .. })
    }
}

fn pop_frame(reader: &mut StreamReader, ch: usize) -> Option<[f32; 2]> {
    if reader.consumer.slots() < ch {
        return None;
    }
    let l = reader.consumer.pop().ok()?;
    let r = if ch == 2 {
        reader.consumer.pop().ok()?
    } else {
        l
    };
    Some([l, r])
}

impl Drop for StreamReader {
    fn drop(&mut self) {
        self.shared.abandoned.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::stream::stream_pair;
    use crate::pcm::PcmCursor;

    fn init(step: f64, looping: bool) -> VoiceInit {
        VoiceInit {
            id: 1,
            step,
            looping,
            gains: [1.0, 1.0, 0.0],
            paused: false,
            start_at: 0,
        }
    }

    fn mono(samples: Vec<f32>) -> Pcm {
        Pcm {
            sample_rate: 48_000,
            channels: 1,
            samples: samples.into(),
        }
    }

    fn run(v: &mut Voice, n: usize, t0: u64) -> Vec<f32> {
        let (mut l, mut r, mut s) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        v.render(&mut l, &mut r, &mut s, t0);
        l
    }

    #[test]
    fn plays_once_and_ends() {
        let mut v = Voice::from_pcm(mono(vec![0.1, 0.2, 0.3]), init(1.0, false)).unwrap();
        assert_eq!(run(&mut v, 5, 0), vec![0.1, 0.2, 0.3, 0.0, 0.0]);
        assert_eq!(v.end, Some(EndReason::Ended));
    }

    #[test]
    fn loop_wraps_exactly() {
        let mut v = Voice::from_pcm(mono(vec![0.1, 0.2, 0.3]), init(1.0, true)).unwrap();
        assert_eq!(run(&mut v, 7, 0), vec![0.1, 0.2, 0.3, 0.1, 0.2, 0.3, 0.1]);
        assert_eq!(v.end, None);
    }

    #[test]
    fn half_speed_interpolates() {
        let mut v = Voice::from_pcm(mono(vec![0.0, 1.0]), init(0.5, false)).unwrap();
        assert_eq!(run(&mut v, 5, 0), vec![0.0, 0.5, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn empty_or_bad_pcm_is_rejected() {
        assert!(Voice::from_pcm(mono(vec![]), init(1.0, true)).is_none());
        let bad = Pcm {
            sample_rate: 48_000,
            channels: 3,
            samples: vec![0.0; 3].into(),
        };
        assert!(Voice::from_pcm(bad, init(1.0, true)).is_none());
        let bad = Pcm {
            sample_rate: 0,
            channels: 1,
            samples: vec![0.0; 3].into(),
        };
        assert!(Voice::from_pcm(bad, init(1.0, true)).is_none());
    }

    #[test]
    fn stream_matches_buffer() {
        let pcm = mono((0..100).map(|i| i as f32 / 100.0).collect());
        let src = Box::new(PcmCursor::new(pcm.clone()));
        let (mut job, reader) = stream_pair(src, false, 256).unwrap();
        job.fill(usize::MAX);
        let mut v = Voice::from_stream(reader, 1, init(1.0, false));
        assert!(v.is_stream());
        let out = run(&mut v, 120, 0);
        assert_eq!(&out[..100], &pcm.samples[..]);
        assert_eq!(&out[100..], &[0.0; 20]);
        assert_eq!(v.end, Some(EndReason::Ended));
    }

    #[test]
    fn stream_starves_without_data() {
        let pcm = mono(vec![0.5; 100]);
        let (_job, reader) = stream_pair(Box::new(PcmCursor::new(pcm)), false, 256).unwrap();
        let mut v = Voice::from_stream(reader, 1, init(1.0, false));
        assert_eq!(run(&mut v, 8, 0), vec![0.0; 8]);
        assert_eq!(v.end, None);
        assert_eq!(v.starved_blocks, 1);
    }

    #[test]
    fn ramp_and_scheduled_stop() {
        let mut v = Voice::from_pcm(mono(vec![1.0; 100]), init(1.0, true)).unwrap();
        run(&mut v, 4, 0);
        v.stop(4, Some(6), 4);
        let out = run(&mut v, 8, 4);
        assert_eq!(out, vec![1.0, 1.0, 1.0, 0.75, 0.5, 0.25, 0.0, 0.0]);
        assert_eq!(v.end, Some(EndReason::Stopped));
    }

    #[test]
    fn virtualised_during_a_ramp_ends_at_once() {
        // Regression: a scheduled stop whose fade had begun, then a pause from the
        // controller (virtualisation), left the voice paused mid-ramp forever.
        let mut v = Voice::from_pcm(mono(vec![1.0; 100]), init(1.0, true)).unwrap();
        run(&mut v, 4, 0);
        v.stop(100, Some(6), 4);
        run(&mut v, 8, 4); // the fade has started at frame 6
        assert_eq!(v.end, None);
        v.set_params([1.0, 1.0, 0.0], true);
        assert_eq!(v.end, Some(EndReason::Stopped));
    }

    #[test]
    fn stop_on_a_paused_fading_voice_ends_it() {
        let mut v = Voice::from_pcm(mono(vec![1.0; 100]), init(1.0, true)).unwrap();
        run(&mut v, 4, 0);
        v.stop(100, None, 4);
        run(&mut v, 4, 4);
        // A pause that reached the voice without ending it (defence in depth).
        v.paused = true;
        v.stop(100, None, 8);
        assert_eq!(v.end, Some(EndReason::Stopped));
    }

    #[test]
    fn reverb_send_is_the_scalar_gain_whatever_the_pan() {
        let mut v = Voice::from_pcm(
            mono(vec![1.0; 8]),
            VoiceInit {
                gains: [0.0, 0.5, 0.5],
                ..init(1.0, false)
            },
        )
        .unwrap();
        let (mut l, mut r, mut s) = (vec![0.0; 4], vec![0.0; 4], vec![0.0; 4]);
        v.render(&mut l, &mut r, &mut s, 0);
        assert_eq!(s, vec![0.5; 4]);
    }

    #[test]
    fn stop_before_start_ends() {
        let mut v = Voice::from_pcm(
            mono(vec![1.0; 100]),
            VoiceInit {
                start_at: 50,
                ..init(1.0, true)
            },
        )
        .unwrap();
        v.stop(10, Some(20), 0);
        assert_eq!(run(&mut v, 32, 0), vec![0.0; 32]);
        assert_eq!(v.end, Some(EndReason::Stopped));
    }
}
