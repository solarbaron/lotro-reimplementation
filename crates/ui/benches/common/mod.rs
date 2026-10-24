// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the painter benchmarks (`paint_frame`, `paint_steady`).

use std::time::{Duration, Instant};

/// CPU time of the calling thread so far (`CLOCK_THREAD_CPUTIME_ID`), so that waiting for
/// a core on a loaded machine is not counted. Falls back to wall-clock time since `start`.
#[cfg(unix)]
// The one `unsafe` call reads the thread's CPU clock; std has no safe API.
#[allow(unsafe_code)]
pub fn thread_cpu(start: Instant) -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable `timespec` owned by this frame, and the clock id is
    // a constant the C library defines; `clock_gettime` writes only into `ts`.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut ts) };
    if rc != 0 {
        return start.elapsed();
    }
    Duration::new(
        u64::try_from(ts.tv_sec).unwrap_or(0),
        u32::try_from(ts.tv_nsec).unwrap_or(0),
    )
}

/// Wall-clock time since `start`: the target has no thread CPU clock this crate reads.
#[cfg(not(unix))]
pub fn thread_cpu(start: Instant) -> Duration {
    start.elapsed()
}
