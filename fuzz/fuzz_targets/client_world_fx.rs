// SPDX-License-Identifier: GPL-3.0-or-later
//! The world walker's M4.5 inputs (`apps/client/src/walker/fxparse.rs`, included by path):
//! the `--disable` system list, the `--camera-loop` path and the game-clock arithmetic of
//! `--day` / `--time-of-day` (`walker.md` §14, `environment.md` §11.1).
//!
//! Input layout: byte 0 selects the mode; the rest is (lossy) UTF-8 text for the parsers,
//! or raw numbers for the clock. Nothing may panic. Parsed lists name known systems once
//! each; parsed loops have 2–64 finite waypoints, a finite length and finite positions
//! along it; clock values that exist are finite.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/walker/fxparse.rs"]
mod fxparse;

use fxparse::{System, Systems, clock_seconds, parse_camera_loop, parse_systems};
use libfuzzer_sys::fuzz_target;

fn text(data: &[u8]) {
    let s = String::from_utf8_lossy(data);
    if let Ok(list) = parse_systems(&s) {
        assert!(list.len() <= System::ALL.len());
        let mut sys = Systems::all();
        for x in &list {
            sys.set(*x, false);
        }
        let _ = sys.disabled_text();
        for (i, a) in list.iter().enumerate() {
            assert!(!list[i + 1..].contains(a), "duplicate system");
        }
    }
    if let Ok(l) = parse_camera_loop(&s) {
        assert!((2..=fxparse::MAX_LOOP_POINTS).contains(&l.points.len()));
        let len = l.length();
        assert!(len.is_finite() && len >= 0.0);
        for d in [0.0, len * 0.37, len * 3.5, -len, f64::NAN, 1e300] {
            let (p, h) = l.at(d);
            assert!(p.iter().all(|v| v.is_finite()) && h.is_finite());
            assert!(l.global(p).iter().all(|v| v.is_finite()));
        }
    }
}

fn clock(data: &[u8]) {
    let mut b = [0u8; 40];
    let n = data.len().min(40);
    b[..n].copy_from_slice(&data[..n]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let f64_at = |i: usize| {
        f64::from_le_bytes([
            b[i],
            b[i + 1],
            b[i + 2],
            b[i + 3],
            b[i + 4],
            b[i + 5],
            b[i + 6],
            b[i + 7],
        ])
    };
    let day = i64::from_le_bytes([b[24], b[25], b[26], b[27], b[28], b[29], b[30], b[31]]);
    if let Some(g) = clock_seconds(
        u32_at(0),
        f64_at(4),
        f64_at(12),
        u32_at(20),
        day,
        f64_at(32),
    ) {
        assert!(g.is_finite());
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 2 {
        0 => text(rest),
        _ => clock(rest),
    }
});
