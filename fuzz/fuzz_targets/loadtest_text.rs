// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-loadtest`'s text readers (`apps/loadtest/src/sysinfo.rs` and `chat.rs`, with
//! `metrics.rs` for the chat ledger's histogram, included by path because this crate does
//! not depend on the load tester; `test-server.md` §13.2–§13.4): the `/proc` readers
//! (`parse_status_kib`, `parse_loadavg`, `parse_stat_cpu_ticks`, `parse_cpu_model`) on
//! arbitrary file text, and the chat ledger's tag reader (`ChatLedger::parse`,
//! `received`) on arbitrary chat lines, which arrive from the server under test.
//!
//! Byte 0 selects the mode, the rest is (lossy) UTF-8 text. Nothing may panic. A status
//! field read is a whole number of KiB; a load average is finite and not negative; a CPU
//! model is at most 128 characters; a tag the ledger wrote reads back to its speaker and
//! sequence number, and a line that reads as a tag names this run.

#![no_main]

use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "../../apps/loadtest/src/metrics.rs"]
mod metrics;

#[allow(dead_code)]
#[path = "../../apps/loadtest/src/sysinfo.rs"]
mod sysinfo;

#[allow(dead_code)]
#[path = "../../apps/loadtest/src/chat.rs"]
mod chat;

use chat::{ChatLedger, Kind};
use libfuzzer_sys::fuzz_target;

const KEYS: [&str; 4] = ["VmRSS", "VmHWM", "MemTotal", ""];

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let text = String::from_utf8_lossy(rest);
    match mode % 3 {
        0 => {
            for key in KEYS {
                if let Some(b) = sysinfo::parse_status_kib(&text, key) {
                    assert_eq!(b % 1024, 0, "{key}: {b}");
                }
            }
            if let Some(v) = sysinfo::parse_loadavg(&text) {
                assert!(v.is_finite() && v >= 0.0, "load {v}");
            }
            let _ = sysinfo::parse_stat_cpu_ticks(&text);
            if let Some(m) = sysinfo::parse_cpu_model(&text) {
                assert!(m.chars().count() <= 128);
            }
        }
        1 => {
            // Lines received from the server: a run ID from the first bytes, then every
            // line through the ledger, with a few of the run's own lines sent first.
            let run = rest
                .get(..4)
                .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            let start = Instant::now();
            let mut ledger = ChatLedger::new(run, start);
            for seq in 0..4u32 {
                ledger.sent(0, seq, Kind::Say, start, vec![0, 1], vec![2]);
            }
            for (i, line) in text.lines().take(256).enumerate() {
                if let Some((_, _)) = ledger.parse(line) {
                    // The tag is `lt` and the run ID in hex: it names this run.
                    let tag_run = line
                        .split(' ')
                        .next()
                        .and_then(|t| t.strip_prefix("lt"))
                        .and_then(|h| u32::from_str_radix(h, 16).ok());
                    assert_eq!(tag_run, Some(run), "{line:?}");
                }
                let now = start + Duration::from_millis(i as u64 * 10);
                let _ = ledger.received(i % 4, line, now);
            }
            ledger.judge_all(&|_, _, _, _| true);
            assert_eq!(ledger.open_lines(), 0);
        }
        _ => {
            // Tags the ledger writes itself read back.
            let ledger = ChatLedger::new(u32::from(mode), Instant::now());
            for (i, w) in rest.chunks(5).take(64).enumerate() {
                let seq = w
                    .get(..4)
                    .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let kind = match w.get(4).copied().unwrap_or(0) % 4 {
                    0 => Kind::Say,
                    1 => Kind::Yell,
                    2 => Kind::Tell,
                    k => Kind::Channel(k),
                };
                let line = ledger.text(i, seq, kind);
                assert_eq!(ledger.parse(&line), Some((i, seq)), "{line:?}");
            }
        }
    }
});
