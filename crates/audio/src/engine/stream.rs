// SPDX-License-Identifier: GPL-3.0-or-later
//! Streamed sources: a feeder decodes [`PcmSource`]s into wait-free ring buffers that the
//! audio thread reads (ARCH §14 "long tracks streamed", §21). Decoding never runs on the
//! audio thread, so a decoder that allocates cannot stall the mixer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::pcm::PcmSource;

/// State shared by one stream's feeder job and its voice.
#[derive(Debug, Default)]
pub(crate) struct StreamShared {
    /// The source has no more frames (end of a non-looping source, a source that cannot
    /// rewind, or a decode fault). Set by the feeder after its last commit.
    pub ended: AtomicBool,
    /// The voice is gone; the feeder drops the job.
    pub abandoned: AtomicBool,
}

/// The feeder's half of a stream.
#[derive(Debug)]
pub(crate) struct StreamJob {
    source: Box<dyn PcmSource>,
    producer: Producer<f32>,
    shared: Arc<StreamShared>,
    looping: bool,
    channels: usize,
}

/// The voice's half of a stream.
#[derive(Debug)]
pub(crate) struct StreamReader {
    pub consumer: Consumer<f32>,
    pub shared: Arc<StreamShared>,
}

/// Creates the two halves for `source` with room for `buffer_frames` frames. `None` when
/// the source has 0 or more than 2 channels or a sample rate of 0.
pub(crate) fn stream_pair(
    source: Box<dyn PcmSource>,
    looping: bool,
    buffer_frames: usize,
) -> Option<(StreamJob, StreamReader)> {
    let channels = usize::from(source.channels());
    if !(1..=2).contains(&channels) || source.sample_rate() == 0 {
        return None;
    }
    let (producer, consumer) = RingBuffer::new(buffer_frames.max(64) * channels);
    let shared = Arc::new(StreamShared::default());
    Some((
        StreamJob {
            source,
            producer,
            shared: shared.clone(),
            looping,
            channels,
        },
        StreamReader { consumer, shared },
    ))
}

impl StreamJob {
    /// Decodes up to `max_frames` frames (bounded by the free space) into the ring. Returns
    /// false when the job is finished (ended or abandoned) and can be dropped.
    pub fn fill(&mut self, max_frames: usize) -> bool {
        if self.shared.abandoned.load(Ordering::Acquire) {
            return false;
        }
        if self.shared.ended.load(Ordering::Relaxed) {
            return false;
        }
        let ch = self.channels;
        let frames = (self.producer.slots() / ch).min(max_frames);
        if frames == 0 {
            return true;
        }
        let Ok(mut chunk) = self.producer.write_chunk(frames * ch) else {
            return true;
        };
        let (a, b) = chunk.as_mut_slices();
        let mut written = 0usize;
        let mut ended = false;
        for slice in [a, b] {
            let (n, end) = read_into(&mut *self.source, slice, ch, self.looping);
            written += n;
            if end || n < slice.len() {
                ended = end;
                break;
            }
        }
        chunk.commit(written);
        if ended {
            self.shared.ended.store(true, Ordering::Release);
            return false;
        }
        true
    }
}

/// Fills `out` (a multiple of `ch` long) from `src`, rewinding when `looping`. Returns the
/// samples written and whether the source ended.
fn read_into(src: &mut dyn PcmSource, out: &mut [f32], ch: usize, looping: bool) -> (usize, bool) {
    let mut off = 0;
    let mut rewound = false;
    while off + ch <= out.len() {
        let room = (out.len() - off) / ch;
        let got = src.read(&mut out[off..off + room * ch]).min(room);
        if got == 0 {
            // A source that yields nothing right after a rewind would spin (pcm.rs contract).
            if looping && !rewound && src.rewind() {
                rewound = true;
                continue;
            }
            return (off, true);
        }
        rewound = false;
        off += got * ch;
    }
    (off, false)
}

/// Runs stream jobs: in a thread for the device and null backends, inline (between mixed
/// blocks) for the offline backend, so that its tests are deterministic. The first
/// `STREAM_PREFILL_FRAMES` frames of a new stream are decoded on the thread that
/// creates its voice (the main thread), so that the stream starts without a gap.
#[derive(Debug)]
pub(crate) struct Feeder {
    rx: mpsc::Receiver<StreamJob>,
    jobs: Vec<StreamJob>,
}

impl Feeder {
    pub fn new() -> (mpsc::Sender<StreamJob>, Self) {
        let (tx, rx) = mpsc::channel();
        (
            tx,
            Self {
                rx,
                jobs: Vec::new(),
            },
        )
    }

    /// Takes new jobs and tops every ring up.
    pub fn pump(&mut self) {
        while let Ok(job) = self.rx.try_recv() {
            self.jobs.push(job);
        }
        self.jobs.retain_mut(|j| j.fill(usize::MAX));
    }

    /// Whether the sending side is gone and no job is left.
    #[cfg(test)]
    pub fn finished(&self) -> bool {
        self.jobs.is_empty() && matches!(self.rx.try_recv(), Err(mpsc::TryRecvError::Disconnected))
    }

    /// Runs until `stop` is set or the engine is gone, pumping every `period`.
    pub fn run(mut self, stop: &AtomicBool, period: std::time::Duration) {
        while !stop.load(Ordering::Acquire) {
            match self.rx.recv_timeout(period) {
                Ok(job) => self.jobs.push(job),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if self.jobs.is_empty() {
                        return;
                    }
                    std::thread::sleep(period);
                }
            }
            self.pump();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcm::{Pcm, PcmCursor};

    fn cursor(samples: Vec<f32>, channels: u16) -> Box<dyn PcmSource> {
        Box::new(PcmCursor::new(Pcm {
            sample_rate: 100,
            channels,
            samples: samples.into(),
        }))
    }

    #[test]
    fn fills_and_ends() {
        let (mut job, mut r) = stream_pair(cursor(vec![1.0, 2.0, 3.0], 1), false, 64).unwrap();
        assert!(!job.fill(usize::MAX));
        assert!(r.shared.ended.load(Ordering::Acquire));
        assert_eq!(r.consumer.pop(), Ok(1.0));
        assert_eq!(r.consumer.pop(), Ok(2.0));
        assert_eq!(r.consumer.pop(), Ok(3.0));
        assert!(r.consumer.pop().is_err());
    }

    #[test]
    fn loops_across_wrap() {
        let (mut job, mut r) = stream_pair(cursor(vec![1.0, 2.0, 3.0, 4.0], 2), true, 64).unwrap();
        assert!(job.fill(5));
        let got: Vec<f32> = std::iter::from_fn(|| r.consumer.pop().ok()).collect();
        assert_eq!(got, vec![1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0]);
        for _ in 0..10 {
            assert!(job.fill(usize::MAX));
            while r.consumer.pop().is_ok() {}
        }
    }

    #[test]
    fn empty_looping_source_ends() {
        let (mut job, r) = stream_pair(cursor(vec![], 1), true, 64).unwrap();
        assert!(!job.fill(usize::MAX));
        assert!(r.shared.ended.load(Ordering::Acquire));
    }

    #[test]
    fn rejects_bad_shapes() {
        assert!(stream_pair(cursor(vec![0.0; 3], 3), false, 64).is_none());
        assert!(stream_pair(cursor(vec![], 0), false, 64).is_none());
    }

    #[test]
    fn abandoned_jobs_are_dropped() {
        let (tx, mut f) = Feeder::new();
        let (job, r) = stream_pair(cursor(vec![0.5; 1000], 1), true, 64).unwrap();
        tx.send(job).unwrap();
        f.pump();
        assert_eq!(f.jobs.len(), 1);
        r.shared.abandoned.store(true, Ordering::Release);
        f.pump();
        assert!(f.jobs.is_empty());
        drop(tx);
        assert!(f.finished());
    }
}
