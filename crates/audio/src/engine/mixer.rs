// SPDX-License-Identifier: GPL-3.0-or-later
//! The mixer: runs on the audio thread (device backend) or inside
//! [`super::AudioEngine::render`] (offline backend).
//!
//! Real-time rules (ARCH §21): the mixer takes commands from a wait-free ring, writes
//! events to another, and never allocates, frees, locks or does I/O. Voices arrive boxed
//! (allocated on the main thread) and leave boxed through the event ring, so that they are
//! dropped on the main thread. All scratch buffers are allocated by [`Mixer::new`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use rtrb::{Consumer, Producer};

use super::reverb::{Reverb, ReverbParams};
use super::voice::Voice;

/// Largest block the mixer renders at once; larger requests are split.
pub(crate) const MAX_BLOCK: usize = 512;

/// Main thread → audio thread.
#[derive(Debug)]
pub(crate) enum Command {
    /// Put a new voice into an empty slot.
    Start { slot: u32, voice: Box<Voice> },
    /// New gains (left, right, reverb send) and state for a voice.
    Params {
        slot: u32,
        id: u64,
        gains: [f32; 3],
        paused: bool,
    },
    /// Stop a voice (at a clock time, or at once) with a ramp-out of `ramp_frames`.
    Stop {
        slot: u32,
        id: u64,
        ramp_frames: u64,
        at: Option<u64>,
    },
    /// New reverb parameters.
    Reverb(ReverbParams),
}

/// Audio thread → main thread.
#[derive(Debug)]
pub(crate) enum MixEvent {
    /// A voice ended (its `end` says why); the slot is free once the main thread has it.
    Finished { slot: u32, voice: Box<Voice> },
    /// A voice was started into an occupied slot (a controller bug); the new voice is
    /// handed back unplayed.
    Rejected { voice: Box<Voice> },
}

/// Counters shared between the mixer and the main thread.
#[derive(Debug, Default)]
pub(crate) struct SharedStats {
    pub clock: AtomicU64,
    pub underruns: AtomicU64,
    pub overloads: AtomicU64,
    pub starved_blocks: AtomicU64,
    pub callbacks: AtomicU64,
    pub event_overflows: AtomicU64,
    /// f32 bits: last and largest render time / block duration.
    pub load_last: AtomicU32,
    pub load_max: AtomicU32,
    /// Set by the device backend when its stream reported a fatal error.
    pub failed: AtomicBool,
}

impl SharedStats {
    pub fn load_last(&self) -> f32 {
        f32::from_bits(self.load_last.load(Ordering::Relaxed))
    }
    pub fn load_max(&self) -> f32 {
        f32::from_bits(self.load_max.load(Ordering::Relaxed))
    }
}

/// The audio-thread state.
#[derive(Debug)]
pub(crate) struct Mixer {
    sample_rate: u32,
    slots: Vec<Option<Box<Voice>>>,
    commands: Consumer<Command>,
    events: Producer<MixEvent>,
    l: Vec<f32>,
    r: Vec<f32>,
    send: Vec<f32>,
    reverb: Reverb,
    clock: u64,
    stats: Arc<SharedStats>,
}

impl Mixer {
    pub fn new(
        sample_rate: u32,
        slots: usize,
        commands: Consumer<Command>,
        events: Producer<MixEvent>,
        stats: Arc<SharedStats>,
    ) -> Self {
        Self {
            sample_rate,
            slots: (0..slots).map(|_| None).collect(),
            commands,
            events,
            l: vec![0.0; MAX_BLOCK],
            r: vec![0.0; MAX_BLOCK],
            send: vec![0.0; MAX_BLOCK],
            reverb: Reverb::new(sample_rate),
            clock: 0,
            stats,
        }
    }

    fn drain_commands(&mut self) {
        while let Ok(cmd) = self.commands.pop() {
            match cmd {
                Command::Start { slot, voice } => match self.slots.get_mut(slot as usize) {
                    Some(s @ None) => *s = Some(voice),
                    _ => emit(&mut self.events, &self.stats, MixEvent::Rejected { voice }),
                },
                Command::Params {
                    slot,
                    id,
                    gains,
                    paused,
                } => {
                    if let Some(Some(v)) = self.slots.get_mut(slot as usize)
                        && v.id == id
                    {
                        v.set_params(gains, paused);
                    }
                }
                Command::Stop {
                    slot,
                    id,
                    ramp_frames,
                    at,
                } => {
                    if let Some(Some(v)) = self.slots.get_mut(slot as usize)
                        && v.id == id
                    {
                        v.stop(ramp_frames, at, self.clock);
                    }
                }
                Command::Reverb(p) => self.reverb.set_params(p),
            }
        }
    }

    /// Renders `out` (interleaved, `out_channels` per frame): stereo into the first two
    /// channels, silence into the others, the average for a mono device.
    pub fn process(&mut self, out: &mut [f32], out_channels: usize) {
        let started = Instant::now();
        let ch = out_channels.max(1);
        let frames = out.len() / ch;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(MAX_BLOCK);
            self.render_block(n);
            let dst = &mut out[done * ch..(done + n) * ch];
            for (i, frame) in dst.chunks_exact_mut(ch).enumerate() {
                let (l, r) = (sanitize(self.l[i]), sanitize(self.r[i]));
                if ch == 1 {
                    frame[0] = 0.5 * (l + r);
                } else {
                    frame[0] = l;
                    frame[1] = r;
                    for s in &mut frame[2..] {
                        *s = 0.0;
                    }
                }
            }
            done += n;
        }
        // Trailing samples of a partial frame.
        for s in &mut out[frames * ch..] {
            *s = 0.0;
        }
        self.stats.clock.store(self.clock, Ordering::Release);
        self.stats.callbacks.fetch_add(1, Ordering::Relaxed);
        if frames > 0 {
            let budget = frames as f64 / f64::from(self.sample_rate.max(1));
            let load = (started.elapsed().as_secs_f64() / budget) as f32;
            self.stats
                .load_last
                .store(load.to_bits(), Ordering::Relaxed);
            if load > self.stats.load_max() {
                self.stats.load_max.store(load.to_bits(), Ordering::Relaxed);
            }
            if load > 1.0 {
                self.stats.overloads.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn render_block(&mut self, n: usize) {
        self.drain_commands();
        let Self {
            slots,
            events,
            l,
            r,
            send,
            reverb,
            clock,
            stats,
            ..
        } = self;
        let (l, r, send) = (&mut l[..n], &mut r[..n], &mut send[..n]);
        l.fill(0.0);
        r.fill(0.0);
        send.fill(0.0);
        let t0 = *clock;
        let mut starved = 0u64;
        for (idx, slot) in slots.iter_mut().enumerate() {
            let Some(v) = slot.as_mut() else { continue };
            let before = v.starved_blocks;
            v.render(l, r, send, t0);
            if v.starved_blocks != before {
                starved += 1;
            }
            if v.end.is_some()
                && let Some(voice) = slot.take()
            {
                emit(
                    events,
                    stats,
                    MixEvent::Finished {
                        slot: idx as u32,
                        voice,
                    },
                );
            }
        }
        if starved > 0 {
            stats.starved_blocks.fetch_add(starved, Ordering::Relaxed);
        }
        reverb.process(send, l, r);
        *clock += n as u64;
    }
}

fn emit(events: &mut Producer<MixEvent>, stats: &SharedStats, ev: MixEvent) {
    if events.push(ev).is_err() {
        // Cannot happen with the ring sized to the slot count; the voice is then dropped
        // here (a free on the audio thread) and counted.
        stats.event_overflows.fetch_add(1, Ordering::Relaxed);
    }
}

fn sanitize(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
