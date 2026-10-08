// SPDX-License-Identifier: GPL-3.0-or-later
//! The mixer's real-time promise (decision 0019, ARCH §21): rendering allocates and frees
//! nothing. Voices arrive boxed through the command ring and leave through the event ring,
//! so that they are dropped on the main thread.
//!
//! A counting global allocator counts allocations and frees made by the current thread while
//! a flag is set. The offline backend renders on the calling thread, so
//! `AudioEngine::render` is measured exactly. It also tops up stream rings inline (the
//! device backend does that on a feeder thread); the cursor sources used here do not
//! allocate, so they do not hide a mixer allocation.

// A counting global allocator needs `unsafe` (test only; precedent:
// crates/formats/tests/resource_limits.rs).
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use tessera_audio::engine::{
    AudioEngine, AudioTime, Channel, EngineConfig, Listener, LivePosition, PositionalClass,
    Positioner, ReverbEnvironment, ReverbPreset, SoundRequest, SoundSource,
};
use tessera_audio::pcm::{Pcm, PcmCursor};

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn counting() -> bool {
    COUNTING.try_with(Cell::get).unwrap_or(false)
}

// SAFETY: forwards to the system allocator unchanged; the counters are atomics.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if counting() {
            FREES.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations and frees made by this thread inside `f`.
fn measure(f: impl FnOnce()) -> (u64, u64) {
    let (a0, f0) = (
        ALLOCS.load(Ordering::Relaxed),
        FREES.load(Ordering::Relaxed),
    );
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    (
        ALLOCS.load(Ordering::Relaxed) - a0,
        FREES.load(Ordering::Relaxed) - f0,
    )
}

const RATE: u32 = 48_000;

fn tone(frames: usize, channels: u16) -> Pcm {
    let samples: Vec<f32> = (0..frames * usize::from(channels))
        .map(|i| (i as f32 * 0.01).sin() * 0.2)
        .collect();
    Pcm {
        sample_rate: 44_100,
        channels,
        samples: samples.into(),
    }
}

#[test]
fn the_counter_counts() {
    let (a, f) = measure(|| drop(std::hint::black_box(vec![1u8; 16])));
    assert_eq!((a, f), (1, 1));
}

#[test]
fn rendering_neither_allocates_nor_frees() {
    let mut e = AudioEngine::new(EngineConfig::offline(RATE)).unwrap();
    e.register_environment(ReverbEnvironment {
        preset: ReverbPreset::from_value(8),
        priority: 0,
        intensity: 1.0,
    });
    let live = LivePosition::new([5.0, 0.0, 0.0]);
    let mut handles = Vec::new();
    for i in 0..40 {
        let req = SoundRequest::pcm(tone(2_000 + 300 * i, 1 + (i % 2) as u16))
            .looping(i % 3 == 0)
            .pitch(0.5 + 0.05 * i as f32)
            .channel(Channel::new_lossy(i as u32 % 9))
            .positional(PositionalClass::Panned, 1.0, 60.0)
            .at(Positioner::live(live.clone()))
            .ramp_out(0.01)
            .tracked(true)
            .limit_exempt(true);
        handles.push(e.request(req).handle().unwrap());
    }
    for i in 0..4 {
        let src = Box::new(PcmCursor::new(tone(10_000, 2)));
        let req = SoundRequest::new(SoundSource::Stream(src))
            .looping(i % 2 == 0)
            .limit_exempt(true);
        handles.push(e.request(req).handle().unwrap());
    }
    e.update();
    let mut out = vec![0.0f32; 1024 * 2];
    // Warm-up: the feeder takes its new stream jobs (a Vec push, inline here only).
    e.render(&mut out).unwrap();

    let mut total = (0, 0);
    for frame in 0..60 {
        // Main-thread work between blocks: moves, volume changes, stops, scheduled starts,
        // reverb changes. Measured separately: it may allocate.
        live.set([5.0 + frame as f32, 0.0, (frame as f32 * 0.3).sin()]);
        e.set_listener(Listener {
            position: [0.0, 0.0, frame as f32 * 0.1],
            ..Listener::default()
        });
        if let Some(h) = handles.get(frame) {
            e.set_volume(*h, 0.5);
        }
        if frame % 7 == 0 {
            e.stop(handles[frame / 7]);
        }
        if frame == 20 {
            let at = AudioTime(e.clock().0 + 333);
            e.request(
                SoundRequest::pcm(tone(500, 1))
                    .start_at(at)
                    .limit_exempt(true),
            );
            e.set_reverb_enabled(false);
        }
        if frame == 40 {
            e.set_reverb_enabled(true);
        }
        e.update();
        let (a, f) = measure(|| e.render(&mut out).unwrap());
        total.0 += a;
        total.1 += f;
        let _ = e.drain_events().count();
    }
    assert!(out.iter().all(|x| x.is_finite()));
    assert_eq!(total, (0, 0), "render() allocated / freed (allocs, frees)");
    // Voices did finish during the run, so the free-on-main-thread path was exercised.
    assert!(e.stats().active_voices < 44);
}
