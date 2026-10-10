// SPDX-License-Identifier: GPL-3.0-or-later
//! The scenario parser of `tessera-net` (`docs/specs/scenario-feed.md` §4, §11): arbitrary
//! bytes as a scenario file. Parse and static checks return `Ok` or a typed error with a
//! position from 1, never a panic, and their peak heap use stays within a fixed bound plus
//! a fixed multiple of the input size (§11.4), measured by a counting allocator. A
//! scenario that parses is resolved with the empty resolver (no install) and played for a
//! bounded number of ticks over the sandbox backend, poking the feed with a few commands;
//! no poll delivers more than eight ticks' worth of events (§7.1, §7.4). The run-time
//! paths with non-empty bindings and the input script are fuzzed by `net_scenario_run`.

#![no_main]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, Scenario};
use tessera_net::{ChatChannel, CombatCommand, Command, Event, Session};

/// Frames (one tick each) of the bounded playback.
const FRAMES: u32 = 200;
/// Most events one poll can deliver: 8 ticks of 4,096, plus the sandbox's own answers.
const MAX_EVENTS_PER_POLL: usize = 8 * 4096 + 16;
/// Peak parse heap: this fixed part (the expanded schedule of at most 200,000 entries and
/// the fixed-size tables) ...
const PEAK_FIXED: usize = 64 << 20;
/// ... plus this many bytes per input byte (the syntax tree holds copies of the strings).
const PEAK_PER_BYTE: usize = 64;

/// Counts live heap bytes and their peak since the last reset.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call forwards to the system allocator with the same arguments; the
// counters only observe sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            // During a moving realloc both blocks exist; count them both for the peak.
            let live = LIVE.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(live, Ordering::Relaxed);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fuzz_target!(|data: &[u8]| {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let parsed = Scenario::parse(data);
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(before);
    assert!(
        peak <= PEAK_FIXED + PEAK_PER_BYTE * data.len(),
        "parse peaked at {peak} bytes for {} input bytes",
        data.len()
    );
    let scenario = match parsed {
        Ok(s) => s,
        Err(e) => {
            assert!(e.line >= 1 && e.column >= 1, "position from 1: {e}");
            return;
        }
    };
    let Ok(resolved) = scenario.resolve(&mut EmptyResolver) else {
        return;
    };
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let Ok(backend) = SandboxBackend::new(config) else {
        return;
    };
    let mut session = Session::new(backend);
    let tick = Duration::from_millis(resolved.tick_ms());
    let mut now = Instant::now();
    let mut events = Vec::new();
    let _ = session.send(now, Command::Connect);
    session.poll_into(now, &mut events);
    let name = resolved
        .enter_character()
        .map(str::to_owned)
        .or_else(|| {
            events.iter().find_map(|e| match e {
                Event::Connected { characters } => characters.first().map(|c| c.name.clone()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "Wanderer".to_owned());
    let _ = session.send(now, Command::EnterWorld { character: name });
    for frame in 0..FRAMES {
        events.clear();
        session.poll_into(now, &mut events);
        assert!(
            events.len() <= MAX_EVENTS_PER_POLL,
            "{} events in one poll",
            events.len()
        );
        if frame % 7 == 3 {
            let _ = session.send(now, Command::Combat(CombatCommand::CancelCast));
            let _ = session.send(
                now,
                Command::Chat {
                    channel: ChatChannel::Say,
                    text: "hi".into(),
                },
            );
        }
        now += tick;
    }
});
