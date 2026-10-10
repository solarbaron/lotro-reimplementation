// SPDX-License-Identifier: GPL-3.0-or-later
//! The statistics line (`test-server.md` §11.1, §14 invariant 9) with a synthetic clock: the
//! simulation is driven synchronously in the order of its run loop (tick, then the tick's
//! duration is recorded), without threads or sockets.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tessera_protocol::limits::DEFAULT_TICK_RATE;
use tessera_protocol::time::tick_start;
use tessera_server::accounts::{AccountStore, AccountsFile};
use tessera_server::characters::WorldState;
use tessera_server::shared::{Stats, StatsLine};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

struct Driver {
    sim: Sim,
    stats: Arc<Stats>,
    k: u64,
    _hash_rx: std::sync::mpsc::Receiver<tessera_server::shared::HashJob>,
    writer_rx: std::sync::mpsc::Receiver<tessera_server::shared::WriterMsg>,
}

impl Driver {
    fn new(stats_interval_s: u32) -> Self {
        Self::with(stats_interval_s, 1)
    }

    fn with(stats_interval_s: u32, instances: u32) -> Self {
        let mut config = Config::default();
        config.log.stats_interval_s = stats_interval_s;
        config.world.instances = instances;
        let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(16);
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(10_000);
        let stats = Arc::new(Stats::default());
        let sim = Sim::new(
            SimParts {
                config: Arc::new(config),
                content: Arc::new(Content::default_world()),
                data_dir: DataDir::new("/nonexistent/tessera-stats-test"),
                accounts: Arc::new(AccountStore::in_memory(AccountsFile::default())),
                characters: BTreeMap::new(),
                world_state: WorldState::default(),
                hash_jobs: hash_tx,
                writer: writer_tx,
                stats: Arc::clone(&stats),
            },
            0,
        );
        Self {
            sim,
            stats,
            k: 0,
            _hash_rx: hash_rx,
            writer_rx,
        }
    }

    /// Runs the next tick at its start time and records `took_us` as its duration, as the
    /// run loop does. Returns the tick's start and the line it wrote, if any.
    fn tick(&mut self, took_us: u64) -> (u64, Option<StatsLine>) {
        self.k += 1;
        let start = tick_start(self.k, DEFAULT_TICK_RATE);
        self.tick_late(start, took_us)
    }

    /// Runs the next tick, which starts at `start` (at or after its due time, §8.1), and
    /// records `took_us` as its duration.
    fn tick_late(&mut self, start: u64, took_us: u64) -> (u64, Option<StatsLine>) {
        let before = self.sim.last_stats_line().cloned();
        self.sim.tick(start);
        self.stats.record_tick(took_us);
        while self.writer_rx.try_recv().is_ok() {}
        let after = self.sim.last_stats_line().cloned();
        (start, if after != before { after } else { None })
    }
}

/// §11.1 interval boundaries and §14 invariant 9: the first interval starts with the
/// simulation, a line is written at the first tick starting at least `stats_interval_s`
/// later, the next interval starts at that tick, its longest tick covers only the ticks
/// completed in it (the writing tick counts toward the next), and the byte rates are the
/// interval's bytes × 10⁶ / its length in µs, rounded down.
#[test]
fn each_line_reports_its_own_interval() {
    let mut d = Driver::new(1);
    let mut lines: Vec<(u64, StatsLine)> = Vec::new();
    // Per interval: (longest duration completed in it, bytes in, bytes out).
    let mut expected: Vec<(u64, u64, u64)> = vec![(0, 0, 0)];
    let mut last_took = 0;
    for i in 0..200u64 {
        // A long tick every 37 ticks; a very long one on the tick that writes the first
        // line (tick 30 starts at exactly 1 s).
        let took = if i + 1 == 30 {
            90_000
        } else if i % 37 == 0 {
            20_000 + i
        } else {
            1_000 + i
        };
        // Traffic between ticks: odd byte counts so that the rates round.
        d.stats.bytes_in.fetch_add(7 * i + 3, Ordering::Relaxed);
        d.stats.bytes_out.fetch_add(11 * i + 5, Ordering::Relaxed);
        let cur = expected.last_mut().unwrap();
        cur.1 += 7 * i + 3;
        cur.2 += 11 * i + 5;
        if i % 50 == 0 {
            d.stats.overruns.fetch_add(1, Ordering::Relaxed);
        }
        let (start, line) = d.tick(took);
        if let Some(line) = line {
            // The tick that wrote it completed after it: a new interval holds it.
            assert_eq!(
                line.tick_last_us, last_took,
                "tick_last is the previous tick"
            );
            lines.push((start, line));
            expected.push((took, 0, 0));
        } else {
            let cur = expected.last_mut().unwrap();
            cur.0 = cur.0.max(took);
        }
        last_took = took;
    }
    assert!(lines.len() >= 5, "{} lines", lines.len());
    let mut interval_start = 0;
    for (n, (start, line)) in lines.iter().enumerate() {
        assert_eq!(line.at_us, *start);
        assert_eq!(line.interval_start_us, interval_start, "line {n}");
        // The first tick starting at least 1 s after the interval start.
        assert!(line.at_us >= interval_start + 1_000_000, "line {n}");
        assert!(
            tick_start(
                tessera_protocol::time::tick_at(line.at_us, DEFAULT_TICK_RATE) - 1,
                DEFAULT_TICK_RATE
            ) < interval_start + 1_000_000,
            "line {n} is late"
        );
        let (longest, bytes_in, bytes_out) = expected[n];
        assert_eq!(line.interval.tick_max_us, longest, "line {n}");
        let len = line.at_us - interval_start;
        assert_eq!(line.interval.bytes_in_per_s, bytes_in * 1_000_000 / len);
        assert_eq!(line.interval.bytes_out_per_s, bytes_out * 1_000_000 / len);
        interval_start = line.at_us;
    }
    // The 90 ms tick wrote the first line: it is in the second line, not the first.
    assert_eq!(lines[0].0, 1_000_000);
    assert!(lines[0].1.interval.tick_max_us < 90_000);
    assert_eq!(lines[1].1.interval.tick_max_us, 90_000);
    // Overruns are a total since start.
    let last = &lines.last().unwrap().1;
    assert_eq!(last.overruns, d.stats.overruns.load(Ordering::Relaxed));
    assert!(lines[0].1.overruns < last.overruns);
    // The since-start longest tick stays for the reports (§13.4).
    assert_eq!(d.stats.snapshot().tick_max_us, 90_000);
}

/// §11.1 (revised 2026-10-10): the server time of a line is the time at which the tick that
/// writes it starts, which may be later than its due time; the interval boundaries are those
/// start times and the byte rates divide by their difference.
#[test]
fn line_time_is_the_start_of_a_late_tick() {
    let mut d = Driver::new(1);
    let mut lines: Vec<(u64, StatsLine, u64, u64)> = Vec::new();
    let (mut bytes_in, mut bytes_out) = (0u64, 0u64);
    for i in 0..200u64 {
        d.k += 1;
        // Every tick starts late, by a varying amount up to just under one period.
        let due = tick_start(d.k, DEFAULT_TICK_RATE);
        let start = due + 1 + (i * 7_919) % 33_000;
        d.stats.bytes_in.fetch_add(13 * i + 1, Ordering::Relaxed);
        d.stats.bytes_out.fetch_add(17 * i + 2, Ordering::Relaxed);
        bytes_in += 13 * i + 1;
        bytes_out += 17 * i + 2;
        if let (start, Some(line)) = d.tick_late(start, 1_000) {
            assert_ne!(start, due);
            lines.push((start, line, bytes_in, bytes_out));
            (bytes_in, bytes_out) = (0, 0);
        }
    }
    assert!(lines.len() >= 5, "{} lines", lines.len());
    let mut interval_start = 0;
    for (n, (start, line, bytes_in, bytes_out)) in lines.iter().enumerate() {
        assert_eq!(
            line.at_us, *start,
            "line {n}: the tick's start, not its due time"
        );
        assert_eq!(line.interval_start_us, interval_start, "line {n}");
        assert!(line.at_us >= interval_start + 1_000_000, "line {n}");
        let len = line.at_us - interval_start;
        assert_eq!(line.interval.bytes_in_per_s, bytes_in * 1_000_000 / len);
        assert_eq!(line.interval.bytes_out_per_s, bytes_out * 1_000_000 / len);
        interval_start = line.at_us;
    }
}

/// §11.1 counts at the time of the line: one figure per instance, entities in all
/// instances (the default world's wanderers), no connections; resident memory where the
/// platform reports it.
#[test]
fn line_counts_are_current() {
    let mut d = Driver::with(1, 3);
    let line = (0..40)
        .find_map(|_| d.tick(500).1)
        .expect("a line within 40 ticks");
    assert_eq!(line.per_instance, vec![0, 0, 0]);
    assert_eq!((line.connections, line.logged_in, line.in_world), (0, 0, 0));
    assert_eq!(line.entities, d.stats.snapshot().entities);
    assert_eq!(line.interest_pairs, 0);
    #[cfg(target_os = "linux")]
    assert!(line.resident_bytes.is_some_and(|b| b > 0));
}

/// §11.1: with `stats_interval_s` = 0 no line is written.
#[test]
fn zero_interval_writes_no_line() {
    let mut d = Driver::new(0);
    for _ in 0..200 {
        assert!(d.tick(500).1.is_none());
    }
}
