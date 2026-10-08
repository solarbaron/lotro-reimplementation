// SPDX-License-Identifier: GPL-3.0-or-later
//! Leak checks (`test-server.md` §13.3): periodic samples of resident memory, the server's
//! object and queue counters and the bots' own session counts; bounds the scenario implies
//! at every sample; a return to the baseline after the last bot left; and a least-squares
//! growth slope of resident memory after a warm-up.

use tessera_server::StatsSnapshot;

use crate::metrics::linear_fit;

/// Most samples kept (one hour at one sample a second); later samples thin the list.
pub const MAX_SAMPLES: usize = 3_600;
/// Most bound violations listed individually.
const MAX_VIOLATIONS: usize = 100;
/// Slack on the network-task bound (`network tasks ≤ connections + a constant`).
pub const NET_TASK_SLACK: usize = 8;
/// Smallest growth allowance, bytes (16 MiB).
pub const MIN_ALLOWANCE: u64 = 16 << 20;

/// One sample.
#[derive(Debug, Clone, PartialEq)]
pub struct LeakSample {
    /// Seconds since the run started.
    pub t_s: f64,
    /// Process resident memory, bytes (in-process: server and bots together).
    pub rss: Option<u64>,
    /// Resident memory of a separate server process on this machine (`--server-pid`).
    pub server_rss: Option<u64>,
    /// The server's counters (in-process only).
    pub server: Option<StatsSnapshot>,
    /// Bots in the world.
    pub bots_in_world: usize,
    /// Entities the bots hold, summed.
    pub bot_entities: usize,
    /// Largest number of entities one bot holds.
    pub bot_entities_max: usize,
    /// Largest number of other bots one bot holds.
    pub bot_bots_max: usize,
    /// Decoded items waiting in the bots' network threads, summed.
    pub bot_pending: usize,
    /// 1-minute load average.
    pub load: Option<f64>,
    /// Process CPU time so far, seconds.
    pub cpu_s: Option<f64>,
}

/// The bounds a scenario implies (§13.3 criterion 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Bots in the run.
    pub bots: usize,
    /// Wanderers in the content, when known (in-process: the tool's own content; remote:
    /// unknown, so a bot's non-bot entities are bounded only by its entity limit).
    pub wanderers: Option<usize>,
    /// `[limits] max_connections`.
    pub max_connections: usize,
    /// `[limits] max_handshakes`.
    pub max_handshakes: usize,
    /// Login verifications that may wait or run (`hash_workers` + 64).
    pub hash_queue: usize,
    /// Network tasks before any bot connected.
    pub base_net_tasks: usize,
    /// The bots' entity limit.
    pub max_entities: usize,
    /// The most decoded items one bot's network thread may hold
    /// (`PENDING_PAUSE + READ_CHUNK / 6 + EVENT_CHANNEL_CAPACITY`).
    pub bot_pending: usize,
}

/// Memory-trend verdict (§13.3 criterion 3).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryVerdict {
    /// Warm-up length, seconds.
    pub warmup_s: f64,
    /// Largest sample during the warm-up, bytes.
    pub warmup_max: u64,
    /// Fitted slope after the warm-up, bytes per second.
    pub slope: f64,
    /// Growth over the rest of the run by the fitted line, bytes.
    pub growth: f64,
    /// Allowed growth: the larger of 16 MiB and 5 % of the warm-up maximum.
    pub allowance: u64,
    /// Largest sample after the warm-up, bytes.
    pub max_after: u64,
    /// Samples after the warm-up.
    pub samples_after: usize,
    /// `Some(true)` pass, `Some(false)` fail, `None` not enough samples to judge.
    pub pass: Option<bool>,
}

/// The monitor.
#[derive(Debug, Default)]
pub struct LeakMonitor {
    /// Samples.
    pub samples: Vec<LeakSample>,
    /// Bound violations (the first ones).
    pub violations: Vec<String>,
    /// Number of bound violations.
    pub violation_count: u64,
    /// Samples where the server's tick counter had not advanced since the previous one
    /// (the simulation stopped).
    pub stalled: u64,
}

impl LeakMonitor {
    /// An empty monitor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn violation(&mut self, what: String) {
        self.violation_count += 1;
        tracing::warn!(%what, "leak bound");
        if self.violations.len() < MAX_VIOLATIONS {
            self.violations.push(what);
        }
    }

    /// Adds a sample and checks the bounds.
    pub fn push(&mut self, s: LeakSample, b: &Bounds) {
        let t = s.t_s;
        let mut problems = Vec::new();
        if let Some(v) = &s.server {
            let mut check = |ok: bool, what: String| {
                if !ok {
                    problems.push(format!("t={t:.0}s: {what}"));
                }
            };
            let wanderers = b.wanderers.unwrap_or(0);
            check(
                v.entities <= wanderers + b.bots,
                format!(
                    "entities {} > wanderers + bots {}",
                    v.entities,
                    wanderers + b.bots
                ),
            );
            check(
                v.in_world <= b.bots,
                format!("in world {} > bots {}", v.in_world, b.bots),
            );
            check(
                v.logged_in <= b.bots,
                format!("logged in {} > bots {}", v.logged_in, b.bots),
            );
            let pairs = b.bots * (b.bots + wanderers);
            check(
                v.interest_pairs <= pairs,
                format!("interest pairs {} > {pairs}", v.interest_pairs),
            );
            let conns = b.max_connections + b.max_handshakes;
            check(
                v.connections <= conns,
                format!("connections {} > {conns}", v.connections),
            );
            check(
                v.handshakes <= b.max_handshakes,
                format!("handshakes {} > {}", v.handshakes, b.max_handshakes),
            );
            let tasks = v.connections + v.handshakes + b.base_net_tasks + NET_TASK_SLACK;
            check(
                v.net_tasks <= tasks,
                format!("network tasks {} > {tasks}", v.net_tasks),
            );
            check(
                v.sim_connections <= conns,
                format!("simulation connections {} > {conns}", v.sim_connections),
            );
            check(
                v.hash_jobs <= b.hash_queue,
                format!("hash jobs {} > {}", v.hash_jobs, b.hash_queue),
            );
            check(
                v.writer_jobs <= 4_096,
                format!("writer jobs {} > 4096", v.writer_jobs),
            );
            check(
                v.character_files <= b.bots + 16,
                format!("cached character files {} > bots + 16", v.character_files),
            );
            if let Some(prev) = self.samples.iter().rev().find_map(|p| p.server)
                && v.ticks == prev.ticks
            {
                self.stalled += 1;
                problems.push(format!("t={t:.0}s: the simulation tick did not advance"));
            }
        }
        let others = b.bots.saturating_sub(1);
        if s.bot_bots_max > others {
            problems.push(format!(
                "t={t:.0}s: a bot holds {} other bots > {others}",
                s.bot_bots_max
            ));
        }
        let per_bot = b
            .wanderers
            .map_or(b.max_entities, |w| (others + w).min(b.max_entities));
        if s.bot_entities_max > per_bot {
            problems.push(format!(
                "t={t:.0}s: a bot holds {} entities > {per_bot}",
                s.bot_entities_max
            ));
        }
        if s.bot_pending > b.bots * b.bot_pending {
            problems.push(format!("t={t:.0}s: bots' pending items {}", s.bot_pending));
        }
        for p in problems {
            self.violation(p);
        }
        if self.samples.len() >= MAX_SAMPLES {
            // Keep the first sample and every other later one: the trend survives.
            let mut i = 0usize;
            self.samples.retain(|_| {
                i += 1;
                i == 1 || i.is_multiple_of(2)
            });
        }
        self.samples.push(s);
    }

    /// The memory trend after a warm-up of the larger of 10 % of the run and `min_warmup_s`,
    /// but at most half the run.
    #[must_use]
    pub fn memory_verdict(&self, run_s: f64, min_warmup_s: f64) -> MemoryVerdict {
        self.memory_verdict_of(run_s, min_warmup_s, |s| s.rss)
    }

    /// [`memory_verdict`](Self::memory_verdict) for another memory series of the samples
    /// (for example [`LeakSample::server_rss`]).
    #[must_use]
    pub fn memory_verdict_of(
        &self,
        run_s: f64,
        min_warmup_s: f64,
        series: impl Fn(&LeakSample) -> Option<u64>,
    ) -> MemoryVerdict {
        let warmup_s = (run_s * 0.1).max(min_warmup_s).min(run_s * 0.5);
        let mem: Vec<(f64, u64)> = self
            .samples
            .iter()
            .filter_map(|s| series(s).map(|r| (s.t_s, r)))
            .collect();
        let warmup_max = mem
            .iter()
            .filter(|(t, _)| *t <= warmup_s)
            .map(|(_, r)| *r)
            .max()
            .unwrap_or(0);
        let after: Vec<(f64, f64)> = mem
            .iter()
            .filter(|(t, _)| *t > warmup_s)
            .map(|(t, r)| (*t, *r as f64))
            .collect();
        let allowance = MIN_ALLOWANCE.max(warmup_max / 20);
        let max_after = after.iter().map(|(_, r)| *r as u64).max().unwrap_or(0);
        let (slope, growth) = match (linear_fit(&after), after.first(), after.last()) {
            (Some((s, _)), Some(a), Some(b)) => (s, s * (b.0 - a.0)),
            _ => (0.0, 0.0),
        };
        let pass = (after.len() >= 3 && warmup_max > 0).then(|| {
            growth < allowance as f64 && max_after <= warmup_max.saturating_add(allowance)
        });
        MemoryVerdict {
            warmup_s,
            warmup_max,
            slope,
            growth,
            allowance,
            max_after,
            samples_after: after.len(),
            pass,
        }
    }
}

/// The counters that must return to their baseline once every bot has left (§13.3
/// criterion 2: every count except the throttle tables, which are judged apart). The
/// baseline is taken after the bots' characters were created, so the cached character
/// records are in it.
#[must_use]
pub fn baseline_differences(base: &StatsSnapshot, now: &StatsSnapshot) -> Vec<String> {
    let mut out = Vec::new();
    let mut eq = |name: &str, a: usize, b: usize| {
        if a != b {
            out.push(format!("{name} {b} (baseline {a})"));
        }
    };
    eq("connections", base.connections, now.connections);
    eq("handshakes", base.handshakes, now.handshakes);
    eq("network tasks", base.net_tasks, now.net_tasks);
    eq(
        "simulation connections",
        base.sim_connections,
        now.sim_connections,
    );
    eq("logged in", base.logged_in, now.logged_in);
    eq("in world", base.in_world, now.in_world);
    eq("entities", base.entities, now.entities);
    eq("interest pairs", base.interest_pairs, now.interest_pairs);
    eq("accounted bytes", base.accounted_bytes, now.accounted_bytes);
    eq("queued bytes", base.queue_bytes, now.queue_bytes);
    eq("inbound queued", base.inbound_queued, now.inbound_queued);
    eq("hash jobs", base.hash_jobs, now.hash_jobs);
    eq("writer jobs", base.writer_jobs, now.writer_jobs);
    eq(
        "cached character records",
        base.character_files,
        now.character_files,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Bounds {
        Bounds {
            bots: 2,
            wanderers: Some(0),
            max_connections: 64,
            max_handshakes: 32,
            hash_queue: 66,
            base_net_tasks: 0,
            max_entities: 1024,
            bot_pending: 2_000,
        }
    }

    fn sample(t: f64, rss: u64) -> LeakSample {
        LeakSample {
            t_s: t,
            rss: Some(rss),
            server_rss: None,
            server: None,
            bots_in_world: 2,
            bot_entities: 2,
            bot_entities_max: 1,
            bot_bots_max: 1,
            bot_pending: 0,
            load: None,
            cpu_s: None,
        }
    }

    #[test]
    fn flat_memory_passes_growth_fails() {
        let mut m = LeakMonitor::new();
        for i in 0..100 {
            m.push(sample(f64::from(i), 100 << 20), &bounds());
        }
        let v = m.memory_verdict(100.0, 10.0);
        assert_eq!(v.pass, Some(true), "{v:?}");
        assert_eq!(m.violation_count, 0);
        let mut g = LeakMonitor::new();
        for i in 0..100u64 {
            g.push(sample(i as f64, (100 << 20) + i * (1 << 20)), &bounds());
        }
        let v = g.memory_verdict(100.0, 10.0);
        assert_eq!(v.pass, Some(false), "{v:?}");
        assert!(v.slope > 0.9 * f64::from(1 << 20));
        assert_eq!(LeakMonitor::new().memory_verdict(10.0, 1.0).pass, None);
        assert_eq!(
            g.memory_verdict_of(100.0, 10.0, |s| s.server_rss).pass,
            None
        );
    }

    #[test]
    fn bounds_and_baseline() {
        let mut m = LeakMonitor::new();
        let mut s = sample(1.0, 1);
        let mut st = StatsSnapshot {
            entities: 5,
            ticks: 1,
            ..StatsSnapshot::default()
        };
        s.server = Some(st);
        s.bot_entities_max = 9;
        m.push(s.clone(), &bounds());
        assert_eq!(m.violation_count, 2, "{:?}", m.violations);
        st.entities = 0;
        s.server = Some(st);
        s.bot_entities_max = 0;
        m.push(s, &bounds());
        assert_eq!(m.stalled, 1);
        let base = StatsSnapshot::default();
        assert!(baseline_differences(&base, &base).is_empty());
        let after = StatsSnapshot {
            in_world: 1,
            ..base
        };
        assert_eq!(baseline_differences(&base, &after).len(), 1);
        // Regression: cached character records are part of the baseline.
        let after = StatsSnapshot {
            character_files: 3,
            ..base
        };
        assert_eq!(
            baseline_differences(&base, &after),
            vec!["cached character records 3 (baseline 0)".to_owned()]
        );
    }

    /// Regression: on a remote server (wanderers unknown) a bot that sees wanderers or
    /// other players is not a leak; only more bots than exist, or more than its limit, is.
    #[test]
    fn unknown_wanderers_bound_only_bots() {
        let remote = Bounds {
            wanderers: None,
            ..bounds()
        };
        let mut m = LeakMonitor::new();
        let mut s = sample(1.0, 1);
        s.bot_entities_max = 40;
        s.bot_bots_max = 1;
        m.push(s.clone(), &remote);
        assert_eq!(m.violation_count, 0, "{:?}", m.violations);
        s.bot_bots_max = 2;
        m.push(s.clone(), &remote);
        assert_eq!(m.violation_count, 1);
        s.bot_bots_max = 1;
        s.bot_entities_max = 2_000;
        m.push(s.clone(), &remote);
        assert_eq!(m.violation_count, 2);
        // In-process the content's wanderers are known (none): 40 entities are too many.
        let mut m = LeakMonitor::new();
        s.bot_entities_max = 40;
        m.push(s, &bounds());
        assert_eq!(m.violation_count, 1);
    }

    #[test]
    fn sample_list_is_bounded() {
        let mut m = LeakMonitor::new();
        for i in 0..(MAX_SAMPLES * 3) {
            m.push(sample(i as f64, 1), &bounds());
        }
        assert!(m.samples.len() <= MAX_SAMPLES);
        assert_eq!(m.samples[0].t_s, 0.0);
    }
}
