// SPDX-License-Identifier: GPL-3.0-or-later
//! The run's report (`test-server.md` §13.4): a short text summary, the samples as CSV and
//! everything as one JSON file (versions, configuration, every sample with the load
//! average, every listed episode), plus the pass/fail verdict and the "indicative" mark.

use std::fmt::Write as _;
use std::time::{Duration, SystemTime};

use tessera_server::{ShutdownReport, StatsSnapshot};

use crate::bot::{BotStats, Traffic};
use crate::chat::ChatLedger;
use crate::desync::{DesyncTracker, Kind, SAMPLE_INTERVAL};
use crate::hostile::HostileSnapshot;
use crate::leak::{LeakMonitor, MemoryVerdict};
use crate::metrics::Histogram;
use crate::runner::{RunOptions, TargetOptions};
use crate::sysinfo::Machine;

/// The return-to-baseline result (`test-server.md` §13.3 criterion 2).
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineResult {
    /// Counters that had not returned within 10 s (empty: all did).
    pub differences: Vec<String>,
    /// Throttle tables: (emptied in time, entries left), when checked.
    pub throttle: Option<(bool, usize)>,
    /// The counters at the end.
    pub after: StatsSnapshot,
}

/// The server's parameters the checks used: from the in-process configuration, or the
/// remote `HelloAck` (and assumed defaults until it arrived).
#[derive(Debug, Clone, PartialEq)]
pub struct RunParams {
    /// Server name and version.
    pub server_version: String,
    /// Interest radius *R*, blocks.
    pub interest_radius: u32,
    /// Tick rate, Hz.
    pub tick_rate: u32,
    /// `MotionBatch` rate, Hz.
    pub snapshot_rate: u32,
    /// New creates per observer per tick (remote: the protocol default 16).
    pub creates_per_tick: u32,
    /// Characters per world instance, when known.
    pub instance_capacity: Option<u32>,
    /// Say range, metres, when known.
    pub say_range: Option<f64>,
    /// Walk speed the server sends, when known in advance (in-process).
    pub walk_speed: Option<f64>,
    /// Run speed the server sends, when known in advance.
    pub run_speed: Option<f64>,
    /// Wanderers in the content, when known.
    pub wanderers: Option<usize>,
}

/// How the tool's own main loop kept up.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ToolLoad {
    /// The loop's tick, ms.
    pub tick_ms: f64,
    /// Loop iterations.
    pub iterations: u64,
    /// Iterations whose work took longer than the tick.
    pub overruns: u64,
    /// Longest iteration, ms.
    pub longest_ms: f64,
    /// Desync rounds skipped because the loop fell a whole interval behind.
    pub desync_rounds_skipped: u64,
}

/// Achieved desync sampling below this rate makes a run invalid (90 % of 10 Hz).
pub const MIN_SAMPLING_HZ: f64 = 9.0;
/// Loop overruns above this share make the timing numbers indicative.
pub const MAX_OVERRUN_SHARE: f64 = 0.05;

/// What the run hands to [`Report::build`].
#[derive(Debug)]
pub struct Inputs {
    /// Options.
    pub opts: RunOptions,
    /// Machine.
    pub machine: Machine,
    /// Wall-clock start.
    pub started_wall: SystemTime,
    /// Measured seconds.
    pub measured_s: f64,
    /// Seconds the bots needed to leave.
    pub left_s: f64,
    /// Characters created, already existing, seconds taken.
    pub provision: (usize, usize, f64),
    /// In-process server.
    pub in_process: bool,
    /// Per bot: index, cheater, counters.
    pub bots: Vec<(usize, bool, BotStats)>,
    /// Desync tracker.
    pub desync: DesyncTracker,
    /// Chat ledger (judged).
    pub ledger: ChatLedger,
    /// Leak monitor.
    pub leak: LeakMonitor,
    /// Server counters at the start and end of the measured run (in-process).
    pub stats: Option<(StatsSnapshot, StatsSnapshot)>,
    /// Process CPU seconds over the measured run.
    pub cpu: Option<f64>,
    /// Hostile counts.
    pub hostile: Option<HostileSnapshot>,
    /// Baseline result (in-process).
    pub baseline: Option<BaselineResult>,
    /// Server shutdown (in-process).
    pub shutdown: Option<ShutdownReport>,
    /// Panics during the run.
    pub panics: u64,
    /// Interpolation delay the bots used.
    pub interpolation_delay: Option<Duration>,
    /// The server's parameters.
    pub params: RunParams,
    /// The tool's own loop.
    pub tool: ToolLoad,
    /// Side of the walking area actually used, blocks.
    pub area_blocks: u32,
}

/// Bot counters summed over all bots.
#[derive(Debug, Clone, Default)]
pub struct Totals {
    /// Sessions.
    pub sessions: u64,
    /// Logins.
    pub logins: u64,
    /// World entries.
    pub world_entries: u64,
    /// Planned reconnections.
    pub reconnects: u64,
    /// Unexpected disconnections.
    pub unexpected_disconnects: u64,
    /// Logins refused for admission and retried.
    pub admission_refusals: u64,
    /// `CORRECTED` reports (cheats included).
    pub corrections: u64,
    /// Teleports done.
    pub teleports: u64,
    /// Teleports refused.
    pub teleports_refused: u64,
    /// Refusals with no teleport pending.
    pub refusals_unmatched: u64,
    /// Teleports unanswered.
    pub teleports_unanswered: u64,
    /// Far teleports beyond *R* + 1 blocks.
    pub far_teleports: u64,
    /// Far teleports that could not leave *R* + 1 blocks.
    pub far_teleports_short: u64,
    /// Chat lines sent.
    pub chat_sent: u64,
    /// Chat lines the server rejected.
    pub chat_rejected: u64,
    /// Channel joins answered.
    pub channel_joins: u64,
    /// Channel leaves answered.
    pub channel_leaves: u64,
    /// Channel requests refused.
    pub channel_refusals: u64,
    /// System lines received.
    pub system_lines: u64,
    /// Commands refused by the session.
    pub send_errors: u64,
    /// Appearance changes.
    pub appearance_changes: u64,
    /// Stance changes.
    pub stance_changes: u64,
    /// Cheating moves.
    pub cheats: u64,
    /// Cheats corrected back to the place before them.
    pub cheats_corrected: u64,
    /// Cheats corrected somewhere else.
    pub cheats_bad_correction: u64,
    /// Cheats with no correction in time.
    pub cheats_unanswered: u64,
    /// Cheats not judged (the bot left the world).
    pub cheats_abandoned: u64,
    /// Extra corrections while a cheat was open.
    pub cheat_extra_corrections: u64,
    /// Close timeouts.
    pub close_timeouts: u64,
    /// World entries the server refused.
    pub enter_refusals: u64,
    /// Transport counters.
    pub traffic: Traffic,
    /// Login latency, ms.
    pub login_ms: Histogram,
    /// World-entry latency, ms.
    pub enter_ms: Histogram,
    /// Ping round trip, ms.
    pub rtt_ms: Histogram,
}

/// The server's movement-rejection counters over the run (in-process).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerCheatCounts {
    /// `CORRECTED` reports sent.
    pub corrections: u64,
    /// Rejected moves dropped without a correction.
    pub dropped: u64,
}

/// The `cheat` scenario's verdict (`test-server.md` §13.1: every cheating move must be
/// corrected or dropped): failures and notes. `server` is the in-process server's counters
/// over the run (`None` remote, where a dropped move cannot be told from an accepted one;
/// observers showing a cheat are Cheat desyncs either way).
#[must_use]
pub fn cheat_verdict(t: &Totals, server: Option<ServerCheatCounts>) -> (Vec<String>, Vec<String>) {
    let (mut fail, mut notes) = (Vec::new(), Vec::new());
    if t.cheats == 0 {
        return (fail, notes);
    }
    if t.cheats_bad_correction > 0 {
        fail.push(format!(
            "cheat: {} cheats corrected to somewhere other than the place before them",
            t.cheats_bad_correction
        ));
    }
    match server {
        Some(s) => {
            if s.corrections + s.dropped == 0 {
                fail.push(format!(
                    "cheat: {} cheats sent but the server counted no correction or drop",
                    t.cheats
                ));
            }
            if t.cheats_unanswered > s.dropped {
                fail.push(format!(
                    "cheat: {} cheats neither corrected nor dropped (server dropped {})",
                    t.cheats_unanswered, s.dropped
                ));
            }
        }
        None if t.cheats_unanswered > 0 => notes.push(format!(
            "{} cheats were not corrected; a remote server's drops cannot be seen, so only the observers' view (Cheat desync) judged them",
            t.cheats_unanswered
        )),
        None => {}
    }
    (fail, notes)
}

/// The report.
#[derive(Debug)]
pub struct Report {
    /// What the run was given and measured.
    pub inputs: Inputs,
    /// Bot totals.
    pub totals: Totals,
    /// Bot failure descriptions (the first 32, with the bot).
    pub bot_failures: Vec<String>,
    /// Memory trend of this process (in-process: server and bots).
    pub memory: MemoryVerdict,
    /// Memory trend of the separate server process (`--server-pid`), if sampled.
    pub server_memory: Option<MemoryVerdict>,
    /// Smallest and largest 1-minute load average sampled.
    pub load: Option<(f64, f64)>,
    /// Achieved desync sampling rate, Hz.
    pub sampling_hz: Option<f64>,
    /// Why the numbers are only indicative (empty: reference conditions).
    pub indicative: Vec<String>,
    /// What the run could not check, or checked only in part.
    pub coverage: Vec<String>,
    /// Why the run failed (empty: passed).
    pub failures: Vec<String>,
}

fn mib(b: f64) -> f64 {
    b / f64::from(1u32 << 20)
}

fn memory_text(m: &MemoryVerdict) -> String {
    format!(
        "{} (warm-up {:.0} s max {:.1} MiB; after: {} samples, slope {:.3} MiB/min, growth {:.2} MiB, max {:.1} MiB, allowance {:.1} MiB)",
        match m.pass {
            Some(true) => "ok",
            Some(false) => "GROWING",
            None => "not judged (too few samples)",
        },
        m.warmup_s,
        mib(m.warmup_max as f64),
        m.samples_after,
        mib(m.slope * 60.0),
        mib(m.growth),
        mib(m.max_after as f64),
        mib(m.allowance as f64)
    )
}

fn sum_totals(bots: &[(usize, bool, BotStats)]) -> (Totals, Vec<String>) {
    let mut t = Totals::default();
    let mut failures = Vec::new();
    for (i, _, s) in bots {
        t.sessions += s.sessions;
        t.logins += s.logins;
        t.world_entries += s.world_entries;
        t.reconnects += s.reconnects;
        t.unexpected_disconnects += s.unexpected_disconnects;
        t.admission_refusals += s.admission_refusals;
        t.corrections += s.corrections;
        t.teleports += s.teleports;
        t.teleports_refused += s.teleports_refused;
        t.refusals_unmatched += s.refusals_unmatched;
        t.teleports_unanswered += s.teleports_unanswered;
        t.far_teleports += s.far_teleports;
        t.far_teleports_short += s.far_teleports_short;
        t.chat_sent += s.chat_sent;
        t.chat_rejected += s.chat_rejected;
        t.channel_joins += s.channel_joins;
        t.channel_leaves += s.channel_leaves;
        t.channel_refusals += s.channel_refusals;
        t.system_lines += s.system_lines;
        t.send_errors += s.send_errors;
        t.appearance_changes += s.appearance_changes;
        t.stance_changes += s.stance_changes;
        t.cheats += s.cheats;
        t.cheats_corrected += s.cheats_corrected;
        t.cheats_bad_correction += s.cheats_bad_correction;
        t.cheats_unanswered += s.cheats_unanswered;
        t.cheats_abandoned += s.cheats_abandoned;
        t.cheat_extra_corrections += s.cheat_extra_corrections;
        t.close_timeouts += s.close_timeouts;
        t.enter_refusals += s.enter_refusals;
        t.traffic.merge(&s.traffic);
        t.login_ms.merge(&s.login_ms);
        t.enter_ms.merge(&s.enter_ms);
        t.rtt_ms.merge(&s.rtt_ms);
        for f in &s.failures {
            if failures.len() < 32 {
                failures.push(format!("bot {i}: {f}"));
            }
        }
    }
    (t, failures)
}

impl Report {
    /// Builds the report and its verdict.
    #[must_use]
    pub fn build(mut inputs: Inputs) -> Self {
        let (totals, bot_failures) = sum_totals(&inputs.bots);
        // Chat problems are desync episodes too (§13.2 "Chat"); those beyond the ledger's
        // list are counted in one step.
        let problems = inputs.ledger.problems.clone();
        for p in problems {
            inputs.desync.record_at(
                Kind::Chat,
                p.recipient,
                Some(p.speaker),
                p.sent_s,
                p.delay_s.unwrap_or(f64::INFINITY),
                format!("{} line {}", p.what, p.seq),
            );
        }
        let c = &inputs.ledger.counts;
        let listed = inputs.ledger.problems.len() as u64;
        let all = c.late + c.lost + c.duplicated + c.out_of_order + c.unexpected;
        inputs
            .desync
            .add_unlisted(Kind::Chat, all.saturating_sub(listed));
        let memory = inputs
            .leak
            .memory_verdict(inputs.measured_s, inputs.opts.warmup_min.as_secs_f64());
        let server_memory = inputs.opts.server_pid.map(|_| {
            inputs.leak.memory_verdict_of(
                inputs.measured_s,
                inputs.opts.warmup_min.as_secs_f64(),
                |s| s.server_rss,
            )
        });
        let loads: Vec<f64> = inputs.leak.samples.iter().filter_map(|s| s.load).collect();
        let load = (!loads.is_empty()).then(|| {
            (
                loads.iter().copied().fold(f64::INFINITY, f64::min),
                loads.iter().copied().fold(0.0, f64::max),
            )
        });
        let sampling_hz = inputs.desync.sampling_rate();

        let mut indicative = Vec::new();
        if !inputs.opts.reference_machine {
            indicative.push(
                "not declared the reference machine under quiet conditions (decision 0010)".into(),
            );
        }
        if let Some((_, max)) = load {
            let half = inputs.machine.cores as f64 / 2.0;
            if max > half {
                indicative.push(format!(
                    "1-minute load average reached {max:.1} > half the {} cores",
                    inputs.machine.cores
                ));
            }
        }
        let tool = inputs.tool;
        if tool.iterations > 0 {
            let share = tool.overruns as f64 / tool.iterations as f64;
            if share > MAX_OVERRUN_SHARE {
                indicative.push(format!(
                    "the tool's loop overran its {:.0} ms tick in {:.1} % of iterations",
                    tool.tick_ms,
                    share * 100.0
                ));
            }
        }

        let mut coverage = Vec::new();
        let r = inputs.desync.config().radius;
        let teleports_far = inputs.opts.behaviour.teleport_interval.is_some()
            && inputs.opts.behaviour.far_teleport_fraction > 0.0;
        if inputs.area_blocks < r + 3 {
            coverage.push(format!(
                "the {0}×{0}-block area is smaller than R + 3 = {1} blocks: no pair can be more than R + 1 apart, so interest leave was not tested",
                inputs.area_blocks,
                r + 3
            ));
        } else if inputs.desync.pairs_left() == 0 {
            coverage.push(
                "no observer–entity pair left interest range (R + 1 blocks) after a create: the leave side was not exercised"
                    .into(),
            );
        }
        if teleports_far && totals.far_teleports_short > 0 {
            coverage.push(format!(
                "{} far teleports could not leave R + 1 blocks (the area is below 2R + 4 = {} blocks there)",
                totals.far_teleports_short,
                2 * r + 4
            ));
        }
        if inputs.desync.lagging_rounds > 0 {
            coverage.push(format!(
                "Missing not judged in {} sampling rounds while the server was behind its tick schedule",
                inputs.desync.lagging_rounds
            ));
        }
        if !inputs.in_process && inputs.params.say_range.is_none() {
            coverage.push(
                "remote: Say range unknown (--say-range), Say judged only for the speaker's echo"
                    .into(),
            );
        }
        let server_cheats = inputs.stats.map(|(a, b)| ServerCheatCounts {
            corrections: b.corrections.saturating_sub(a.corrections),
            dropped: b.rejections_dropped.saturating_sub(a.rejections_dropped),
        });
        let (cheat_fail, cheat_notes) = cheat_verdict(&totals, server_cheats);
        coverage.extend(cheat_notes);

        let mut f = Vec::new();
        let d = &inputs.desync;
        if d.total() > 0 {
            let by: Vec<String> = Kind::ALL
                .iter()
                .filter(|k| d.count(**k) > 0)
                .map(|k| format!("{} {}", k.name(), d.count(*k)))
                .collect();
            f.push(format!(
                "desync: {} episodes ({})",
                d.total(),
                by.join(", ")
            ));
        }
        if let Some(hz) = sampling_hz
            && d.rounds >= 20
            && hz < MIN_SAMPLING_HZ
        {
            f.push(format!(
                "run invalid: the tool sampled desync at {hz:.1} Hz < {MIN_SAMPLING_HZ} Hz (its loop is overloaded; fewer bots or a faster machine)"
            ));
        }
        f.extend(cheat_fail);
        if totals.world_entries == 0 {
            f.push("no bot entered the world".into());
        }
        if totals.unexpected_disconnects > 0 {
            f.push(format!(
                "{} unexpected disconnections",
                totals.unexpected_disconnects
            ));
        }
        if !bot_failures.is_empty() {
            f.push(format!("bot failures: {}", bot_failures.len()));
        }
        if totals.close_timeouts > 0 {
            f.push(format!(
                "{} sessions did not close within 10 s",
                totals.close_timeouts
            ));
        }
        if inputs.leak.violation_count > 0 {
            f.push(format!(
                "leak: {} bound violations",
                inputs.leak.violation_count
            ));
        }
        let growing = |who: &str, m: &MemoryVerdict| {
            format!(
                "leak: {who} memory grew {:.1} MiB after warm-up (allowance {:.1} MiB, max {:.1} MiB over warm-up max {:.1} MiB)",
                mib(m.growth),
                mib(m.allowance as f64),
                mib(m.max_after as f64),
                mib(m.warmup_max as f64)
            )
        };
        // §13.3 judges the server's resident memory. With a separately measured server
        // (`--server-pid`) this process holds only the bots, so its trend is reported but
        // not judged (test-server.md §15.2).
        if memory.pass == Some(false) && inputs.opts.server_pid.is_none() {
            f.push(growing("process", &memory));
        }
        if let Some(m) = &server_memory {
            match m.pass {
                Some(false) => f.push(growing("server process", m)),
                None => f.push("leak: the server process's memory could not be judged".into()),
                Some(true) => {}
            }
        }
        if let Some(b) = &inputs.baseline {
            if !b.differences.is_empty() {
                f.push(format!(
                    "leak: not back to baseline: {}",
                    b.differences.join(", ")
                ));
            }
            if let Some((false, left)) = b.throttle {
                f.push(format!("leak: {left} throttle entries left"));
            }
        }
        if inputs.panics > 0 {
            f.push(format!("{} panics", inputs.panics));
        }
        if let Some(s) = inputs.shutdown
            && s.failures > 0
        {
            f.push(format!("server shutdown: {} save failures", s.failures));
        }
        if let Some(h) = &inputs.hostile
            && h.attacks.values().sum::<u64>() == 0
        {
            f.push("hostile clients ran no attack".into());
        }
        if inputs.opts.hostile.is_some() && inputs.hostile.is_none() {
            f.push("hostile clients did not start (run too short?)".into());
        }
        if let Some(h) = &inputs.hostile
            && h.v12_wrong > 0
        {
            f.push(format!(
                "hostile 1.2 cases: {} answered otherwise than the spec says ({})",
                h.v12_wrong,
                h.v12_wrong_examples.join("; ")
            ));
        }
        Self {
            inputs,
            totals,
            bot_failures,
            memory,
            server_memory,
            load,
            sampling_hz,
            indicative,
            coverage,
            failures: f,
        }
    }

    /// Whether the run passed (§13.4 exit status 0).
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    fn target_text(&self) -> String {
        match &self.inputs.opts.target {
            TargetOptions::InProcess { .. } => "in-process (127.0.0.1)".into(),
            TargetOptions::Remote { connect, .. } => connect.clone(),
        }
    }

    /// The text summary.
    #[must_use]
    pub fn text(&self) -> String {
        let i = &self.inputs;
        let o = &i.opts;
        let t = &self.totals;
        let p = &i.params;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "tessera-loadtest {}  scenario {}  bots {}  measured {:.0} s  server {} [{}]  seed {:#x}",
            env!("CARGO_PKG_VERSION"),
            o.scenario.name(),
            o.bots,
            i.measured_s,
            self.target_text(),
            p.server_version,
            o.seed
        );
        if self.indicative.is_empty() {
            let _ = writeln!(s, "timing: reference conditions");
        } else {
            let _ = writeln!(s, "timing: INDICATIVE ({})", self.indicative.join("; "));
        }
        let _ = writeln!(
            s,
            "machine: {} ({} threads), {:.1} GiB; load avg {}",
            i.machine.cpu.as_deref().unwrap_or("unknown CPU"),
            i.machine.cores,
            i.machine
                .memory
                .map_or(0.0, |m| m as f64 / f64::from(1u32 << 30)),
            self.load
                .map_or("-".into(), |(a, b)| format!("{a:.1}-{b:.1}"))
        );
        let _ = writeln!(
            s,
            "setup: characters created {}, existing {} in {:.1} s; area {}x{} blocks; interest radius {}; creates/tick {}; say range {}; interpolation delay {}",
            i.provision.0,
            i.provision.1,
            i.provision.2,
            i.area_blocks,
            i.area_blocks,
            i.desync.config().radius,
            i.desync.config().creates_per_tick,
            p.say_range.map_or("unknown".into(), |r| format!("{r} m")),
            i.interpolation_delay
                .map_or("-".into(), |d| format!("{} ms", d.as_millis()))
        );
        let _ = writeln!(
            s,
            "tool: {} loop iterations, {} over the {:.0} ms tick, longest {:.1} ms; desync sampled at {} (target {:.0} Hz), {} rounds skipped, round p99 {}",
            i.tool.iterations,
            i.tool.overruns,
            i.tool.tick_ms,
            i.tool.longest_ms,
            self.sampling_hz
                .map_or("-".into(), |h| format!("{h:.2} Hz")),
            1.0 / SAMPLE_INTERVAL.as_secs_f64(),
            i.tool.desync_rounds_skipped,
            i.desync
                .round_ms
                .quantile(0.99)
                .map_or("-".into(), |v| format!("{v:.2} ms"))
        );
        let _ = writeln!(
            s,
            "sessions: {} started, {} logins, {} world entries ({} refused), {} planned reconnects, {} admission refusals, {} unexpected disconnects, {} close timeouts, {} refused commands",
            t.sessions,
            t.logins,
            t.world_entries,
            t.enter_refusals,
            t.reconnects,
            t.admission_refusals,
            t.unexpected_disconnects,
            t.close_timeouts,
            t.send_errors
        );
        let _ = writeln!(
            s,
            "actions: teleports {} (far {}, far but short {}, refused {}, unanswered {}, late refusals {}), appearance changes {}, stance changes {}, chat lines {} (rejected {}), channel joins {} leaves {} refusals {}, system lines {}",
            t.teleports,
            t.far_teleports,
            t.far_teleports_short,
            t.teleports_refused,
            t.teleports_unanswered,
            t.refusals_unmatched,
            t.appearance_changes,
            t.stance_changes,
            t.chat_sent,
            t.chat_rejected,
            t.channel_joins,
            t.channel_leaves,
            t.channel_refusals,
            t.system_lines
        );
        if t.cheats > 0 {
            let _ = writeln!(
                s,
                "cheats: {} sent, {} corrected back, {} corrected elsewhere, {} unanswered, {} abandoned, {} extra corrections",
                t.cheats,
                t.cheats_corrected,
                t.cheats_bad_correction,
                t.cheats_unanswered,
                t.cheats_abandoned,
                t.cheat_extra_corrections
            );
        }
        let d = &i.desync;
        let by: Vec<String> = Kind::ALL
            .iter()
            .map(|k| format!("{} {}", k.name(), d.count(*k)))
            .collect();
        let _ = writeln!(s, "desync: {} episodes ({})", d.total(), by.join(", "));
        let _ = writeln!(
            s,
            "position: {} comparisons over {} observer-entity pairs, {} pairs left interest range, error m {}",
            d.samples,
            d.pairs_compared(),
            d.pairs_left(),
            d.error_m.summary(3)
        );
        for e in d.episodes().iter().take(20) {
            let _ = writeln!(
                s,
                "  {} t={:.1}s bot {} sees {:?}: {:.3} ({}){}",
                e.kind.name(),
                e.at_s,
                e.observer,
                e.entity,
                e.size,
                e.note,
                e.load.map_or(String::new(), |l| format!(" load {l:.1}"))
            );
        }
        let c = &i.ledger.counts;
        let _ = writeln!(
            s,
            "chat: {} lines ({} on channels), {} certain deliveries, {} ok, {} late, {} lost, {} duplicated, {} out of order, {} unexpected, {} optional, {} rejected; latency ms {}",
            c.lines,
            c.channel_lines,
            c.expected,
            c.delivered,
            c.late,
            c.lost,
            c.duplicated,
            c.out_of_order,
            c.unexpected,
            c.optional_received,
            c.rejected,
            i.ledger.latency_ms.summary(2)
        );
        let _ = writeln!(
            s,
            "latency ms: login {}; world entry {}; ping RTT {}",
            t.login_ms.summary(1),
            t.enter_ms.summary(1),
            t.rtt_ms.summary(2)
        );
        let secs = i.measured_s.max(1e-3);
        let per_bot = secs * o.bots as f64;
        let tr = &t.traffic;
        let _ = writeln!(
            s,
            "traffic per bot: up {:.0} B/s, down {:.0} B/s (UDP); moves {:.1}/s, motion batches {:.1}/s; lost packets {}, motion dropped {}, protocol dropped {}",
            tr.udp_sent as f64 / per_bot,
            tr.udp_received as f64 / per_bot,
            tr.moves_sent as f64 / per_bot,
            tr.motion_batches as f64 / per_bot,
            tr.lost_packets,
            tr.motion_dropped,
            tr.protocol_dropped
        );
        let _ = writeln!(
            s,
            "motion snapshots: {} received, interval mean {} max {} ms, {} empty snapshot periods",
            tr.motion_snapshots,
            tr.motion_interval_mean_ms()
                .map_or("-".into(), |v| format!("{v:.1}")),
            if tr.motion_intervals > 0 {
                format!("{:.1}", tr.motion_interval_max_us as f64 / 1e3)
            } else {
                "-".into()
            },
            tr.motion_gaps
        );
        if let Some((a, b)) = &i.stats {
            let _ = writeln!(
                s,
                "server: in {:.0} msg/s {:.0} B/s, out {:.0} msg/s {:.0} B/s; ticks {}, overruns {}, skipped {}, longest tick {} us; corrections {}, violations {}, dropped moves {}, slow consumers {}, server full {}, chat delivered {}",
                (b.messages_in - a.messages_in) as f64 / secs,
                (b.bytes_in - a.bytes_in) as f64 / secs,
                (b.messages_out - a.messages_out) as f64 / secs,
                (b.bytes_out - a.bytes_out) as f64 / secs,
                b.ticks - a.ticks,
                b.overruns - a.overruns,
                b.skipped_ticks - a.skipped_ticks,
                b.tick_max_us,
                b.corrections - a.corrections,
                b.violations - a.violations,
                b.rejections_dropped - a.rejections_dropped,
                b.slow_consumers - a.slow_consumers,
                b.server_full - a.server_full,
                b.chat_delivered - a.chat_delivered
            );
            let peak = i
                .leak
                .samples
                .iter()
                .filter_map(|x| x.server.map(|v| v.accounted_bytes))
                .max()
                .unwrap_or(0);
            let _ = writeln!(
                s,
                "server accounted memory: peak {:.1} MiB",
                mib(peak as f64)
            );
        }
        if let Some(cpu) = i.cpu {
            let _ = writeln!(
                s,
                "process CPU: {cpu:.1} s over {:.0} s ({:.0} % of one core)",
                secs,
                100.0 * cpu / secs
            );
        }
        let _ = writeln!(
            s,
            "leak: {} samples, {} bound violations, {} stalled; {} {}",
            i.leak.samples.len(),
            i.leak.violation_count,
            i.leak.stalled,
            if i.opts.server_pid.is_some() {
                "bot process memory (not judged)"
            } else {
                "process memory"
            },
            memory_text(&self.memory)
        );
        if let Some(m) = &self.server_memory {
            let _ = writeln!(s, "leak: server process memory {}", memory_text(m));
        }
        for v in i.leak.violations.iter().take(10) {
            let _ = writeln!(s, "  {v}");
        }
        if let Some(b) = &i.baseline {
            let _ = writeln!(
                s,
                "baseline: {}; throttle tables {}",
                if b.differences.is_empty() {
                    "every count returned".to_owned()
                } else {
                    b.differences.join(", ")
                },
                match b.throttle {
                    None => "not checked".to_owned(),
                    Some((true, _)) => "empty".to_owned(),
                    Some((false, n)) => format!("{n} entries left"),
                }
            );
        }
        if let Some(h) = &i.hostile {
            let attacks: Vec<String> = h.attacks.iter().map(|(k, v)| format!("{k} {v}")).collect();
            let closed: Vec<String> = h
                .closed_by_server
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            let _ = writeln!(
                s,
                "hostile: from {:?}; attacks {} ({}); udp datagrams {}; connect failures {}; closed by server ({}); left open {}; 1.2 cases in the world {}, skipped {}, wrong {}",
                h.bind,
                h.attacks.values().sum::<u64>(),
                attacks.join(", "),
                h.udp_datagrams,
                h.connect_failures,
                closed.join(", "),
                h.left_open,
                h.v12_in_world,
                h.v12_skipped,
                h.v12_wrong
            );
        }
        if let Some(sd) = i.shutdown {
            let _ = writeln!(
                s,
                "server shutdown: saved {}, failures {}",
                sd.saved, sd.failures
            );
        }
        let _ = writeln!(s, "panics: {}", i.panics);
        for b in self.bot_failures.iter().take(10) {
            let _ = writeln!(s, "  {b}");
        }
        for c in &self.coverage {
            let _ = writeln!(s, "coverage: {c}");
        }
        if self.passed() {
            let _ = writeln!(s, "verdict: PASS");
        } else {
            let _ = writeln!(s, "verdict: FAIL: {}", self.failures.join("; "));
        }
        s
    }

    /// The leak samples as CSV.
    #[must_use]
    pub fn csv(&self) -> String {
        let mut s = String::from(
            "t_s,rss_bytes,server_rss_bytes,load1,cpu_s,bots_in_world,bot_entities,bot_pending,connections,handshakes,net_tasks,sim_connections,logged_in,in_world,entities,interest_pairs,accounted_bytes,queue_bytes,inbound_queued,hash_jobs,writer_jobs,throttle_entries,character_files,ticks,overruns,tick_max_us,messages_in,messages_out,bytes_in,bytes_out,channels,channel_memberships\n",
        );
        let opt = |v: Option<String>| v.unwrap_or_default();
        for x in &self.inputs.leak.samples {
            let _ = write!(
                s,
                "{:.1},{},{},{},{},{},{},{}",
                x.t_s,
                opt(x.rss.map(|v| v.to_string())),
                opt(x.server_rss.map(|v| v.to_string())),
                opt(x.load.map(|v| format!("{v:.2}"))),
                opt(x.cpu_s.map(|v| format!("{v:.2}"))),
                x.bots_in_world,
                x.bot_entities,
                x.bot_pending
            );
            match &x.server {
                Some(v) => {
                    let _ = writeln!(
                        s,
                        ",{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                        v.connections,
                        v.handshakes,
                        v.net_tasks,
                        v.sim_connections,
                        v.logged_in,
                        v.in_world,
                        v.entities,
                        v.interest_pairs,
                        v.accounted_bytes,
                        v.queue_bytes,
                        v.inbound_queued,
                        v.hash_jobs,
                        v.writer_jobs,
                        v.throttle_entries,
                        v.character_files,
                        v.ticks,
                        v.overruns,
                        v.tick_max_us,
                        v.messages_in,
                        v.messages_out,
                        v.bytes_in,
                        v.bytes_out,
                        v.channels,
                        v.channel_memberships
                    );
                }
                None => s.push_str(",,,,,,,,,,,,,,,,,,,,,,,,\n"),
            }
        }
        s
    }

    /// Everything as one JSON document (§13.4).
    #[must_use]
    pub fn json(&self) -> String {
        let i = &self.inputs;
        let o = &i.opts;
        let t = &self.totals;
        let d = &i.desync;
        let c = &i.ledger.counts;
        let p = &i.params;
        let mut j = Json::default();
        j.begin(None, '{');
        j.str(
            Some("tool"),
            &format!("tessera-loadtest {}", env!("CARGO_PKG_VERSION")),
        );
        j.str(Some("server"), &self.target_text());
        j.str(Some("server_version"), &p.server_version);
        j.str(Some("scenario"), o.scenario.name());
        j.uint(Some("bots"), o.bots as u64);
        // As a string: JSON numbers lose integers above 2^53.
        j.str(Some("seed"), &o.seed.to_string());
        j.str(Some("seed_hex"), &format!("{:#x}", o.seed));
        j.num(
            Some("started_unix_s"),
            i.started_wall
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64()),
        );
        j.num(Some("duration_s"), o.duration.as_secs_f64());
        j.num(Some("measured_s"), i.measured_s);
        j.num(Some("left_s"), i.left_s);
        j.begin(Some("machine"), '{');
        j.str(Some("cpu"), i.machine.cpu.as_deref().unwrap_or(""));
        j.uint(Some("cores"), i.machine.cores as u64);
        j.opt_uint(Some("memory_bytes"), i.machine.memory);
        j.end('}');
        self.json_config(&mut j);
        j.bool(Some("indicative"), !self.indicative.is_empty());
        j.strs(Some("indicative_reasons"), &self.indicative);
        j.strs(Some("coverage"), &self.coverage);
        j.bool(Some("passed"), self.passed());
        j.strs(Some("failures"), &self.failures);
        j.begin(Some("tool_load"), '{');
        j.uint(Some("iterations"), i.tool.iterations);
        j.uint(Some("overruns"), i.tool.overruns);
        j.num(Some("longest_ms"), i.tool.longest_ms);
        j.uint(Some("desync_rounds"), d.rounds);
        j.uint(Some("desync_rounds_skipped"), i.tool.desync_rounds_skipped);
        j.opt(Some("desync_sampling_hz"), self.sampling_hz);
        j.opt(Some("desync_round_p99_ms"), d.round_ms.quantile(0.99));
        j.end('}');
        j.begin(Some("desync"), '{');
        j.uint(Some("total"), d.total());
        for k in Kind::ALL {
            j.uint(Some(k.name()), d.count(k));
        }
        j.uint(Some("position_comparisons"), d.samples);
        j.uint(Some("pairs_compared"), d.pairs_compared() as u64);
        j.uint(Some("pairs_left_interest"), d.pairs_left() as u64);
        j.uint(Some("lagging_rounds"), d.lagging_rounds);
        j.opt(Some("position_error_p50_m"), d.error_m.quantile(0.5));
        j.opt(Some("position_error_p99_m"), d.error_m.quantile(0.99));
        j.opt(Some("position_error_max_m"), d.error_m.max());
        j.end('}');
        j.begin(Some("episodes"), '[');
        for e in d.episodes() {
            j.begin(None, '{');
            j.str(Some("kind"), e.kind.name());
            j.uint(Some("observer"), e.observer as u64);
            j.opt_uint(Some("entity"), e.entity.map(|v| v as u64));
            j.num(Some("at_s"), e.at_s);
            j.num(Some("size"), e.size);
            j.opt(Some("load"), e.load);
            j.str(Some("note"), &e.note);
            j.end('}');
        }
        j.end(']');
        j.begin(Some("chat"), '{');
        for (k, v) in [
            ("lines", c.lines),
            ("expected", c.expected),
            ("delivered", c.delivered),
            ("late", c.late),
            ("lost", c.lost),
            ("duplicated", c.duplicated),
            ("out_of_order", c.out_of_order),
            ("unexpected", c.unexpected),
            ("optional_received", c.optional_received),
            ("rejected", c.rejected),
            ("unknown", c.unknown),
            ("channel_lines", c.channel_lines),
        ] {
            j.uint(Some(k), v);
        }
        j.opt(Some("latency_p50_ms"), i.ledger.latency_ms.quantile(0.5));
        j.opt(Some("latency_p99_ms"), i.ledger.latency_ms.quantile(0.99));
        j.end('}');
        j.begin(Some("latency_ms"), '{');
        for (k, h) in [
            ("login", &t.login_ms),
            ("enter", &t.enter_ms),
            ("rtt", &t.rtt_ms),
        ] {
            j.opt(Some(&format!("{k}_p50")), h.quantile(0.5));
            j.opt(Some(&format!("{k}_p99")), h.quantile(0.99));
            j.opt(Some(&format!("{k}_max")), h.max());
        }
        j.end('}');
        j.begin(Some("bots_total"), '{');
        for (k, v) in [
            ("sessions", t.sessions),
            ("logins", t.logins),
            ("world_entries", t.world_entries),
            ("reconnects", t.reconnects),
            ("unexpected_disconnects", t.unexpected_disconnects),
            ("admission_refusals", t.admission_refusals),
            ("close_timeouts", t.close_timeouts),
            ("enter_refusals", t.enter_refusals),
            ("send_errors", t.send_errors),
            ("corrections", t.corrections),
            ("teleports", t.teleports),
            ("far_teleports", t.far_teleports),
            ("far_teleports_short", t.far_teleports_short),
            ("teleports_refused", t.teleports_refused),
            ("refusals_unmatched", t.refusals_unmatched),
            ("teleports_unanswered", t.teleports_unanswered),
            ("appearance_changes", t.appearance_changes),
            ("stance_changes", t.stance_changes),
            ("chat_sent", t.chat_sent),
            ("chat_rejected", t.chat_rejected),
            ("channel_joins", t.channel_joins),
            ("channel_leaves", t.channel_leaves),
            ("channel_refusals", t.channel_refusals),
            ("system_lines", t.system_lines),
            ("cheats", t.cheats),
            ("cheats_corrected", t.cheats_corrected),
            ("cheats_bad_correction", t.cheats_bad_correction),
            ("cheats_unanswered", t.cheats_unanswered),
            ("cheats_abandoned", t.cheats_abandoned),
            ("cheat_extra_corrections", t.cheat_extra_corrections),
            ("udp_bytes_sent", t.traffic.udp_sent),
            ("udp_bytes_received", t.traffic.udp_received),
            ("moves_sent", t.traffic.moves_sent),
            ("motion_batches", t.traffic.motion_batches),
            ("datagrams_received", t.traffic.datagrams_received),
            ("motion_dropped", t.traffic.motion_dropped),
            ("lost_packets", t.traffic.lost_packets),
            ("protocol_dropped", t.traffic.protocol_dropped),
            ("motion_snapshots", t.traffic.motion_snapshots),
            ("motion_intervals", t.traffic.motion_intervals),
            ("motion_interval_sum_us", t.traffic.motion_interval_sum_us),
            ("motion_interval_max_us", t.traffic.motion_interval_max_us),
            ("motion_gaps", t.traffic.motion_gaps),
        ] {
            j.uint(Some(k), v);
        }
        j.strs(Some("failures"), &self.bot_failures);
        j.end('}');
        if let Some((a, b)) = &i.stats {
            j.begin(Some("server_counters"), '{');
            for (k, v) in [
                ("ticks", b.ticks - a.ticks),
                ("overruns", b.overruns - a.overruns),
                ("skipped_ticks", b.skipped_ticks - a.skipped_ticks),
                ("tick_max_us", b.tick_max_us),
                ("messages_in", b.messages_in - a.messages_in),
                ("messages_out", b.messages_out - a.messages_out),
                ("bytes_in", b.bytes_in - a.bytes_in),
                ("bytes_out", b.bytes_out - a.bytes_out),
                ("corrections", b.corrections - a.corrections),
                ("violations", b.violations - a.violations),
                (
                    "rejections_dropped",
                    b.rejections_dropped - a.rejections_dropped,
                ),
                ("slow_consumers", b.slow_consumers - a.slow_consumers),
                ("server_full", b.server_full - a.server_full),
                ("chat_delivered", b.chat_delivered - a.chat_delivered),
            ] {
                j.uint(Some(k), v);
            }
            j.end('}');
        }
        j.begin(Some("leak"), '{');
        j.uint(Some("violations"), i.leak.violation_count);
        j.strs(Some("violation_list"), &i.leak.violations);
        j.uint(Some("stalled"), i.leak.stalled);
        json_memory(&mut j, "memory", &self.memory, o.server_pid.is_none());
        if let Some(m) = &self.server_memory {
            json_memory(&mut j, "server_memory", m, true);
        }
        if let Some(b) = &i.baseline {
            j.bool(Some("baseline_ok"), b.differences.is_empty());
            j.strs(Some("baseline_differences"), &b.differences);
            match b.throttle {
                None => j.null(Some("throttle_empty")),
                Some((ok, left)) => {
                    j.bool(Some("throttle_empty"), ok);
                    j.uint(Some("throttle_left"), left as u64);
                }
            }
        }
        j.end('}');
        if let Some(h) = &i.hostile {
            j.begin(Some("hostile"), '{');
            j.uint(Some("attacks"), h.attacks.values().sum::<u64>());
            j.begin(Some("by_attack"), '{');
            for (k, v) in &h.attacks {
                j.uint(Some(k), *v);
            }
            j.end('}');
            j.begin(Some("closed_by_server"), '{');
            for (k, v) in &h.closed_by_server {
                j.uint(Some(k), *v);
            }
            j.end('}');
            j.uint(Some("udp_datagrams"), h.udp_datagrams);
            j.uint(Some("connect_failures"), h.connect_failures);
            j.uint(Some("left_open"), h.left_open);
            j.uint(Some("v12_in_world"), h.v12_in_world);
            j.uint(Some("v12_skipped"), h.v12_skipped);
            j.uint(Some("v12_wrong"), h.v12_wrong);
            j.end('}');
        }
        if let Some(sd) = i.shutdown {
            j.begin(Some("shutdown"), '{');
            j.uint(Some("saved"), sd.saved as u64);
            j.uint(Some("failures"), sd.failures as u64);
            j.end('}');
        }
        j.uint(Some("panics"), i.panics);
        j.begin(Some("samples"), '[');
        for x in &i.leak.samples {
            j.begin(None, '{');
            j.num(Some("t_s"), x.t_s);
            j.opt(Some("load1"), x.load);
            j.opt_uint(Some("rss_bytes"), x.rss);
            j.opt_uint(Some("server_rss_bytes"), x.server_rss);
            j.opt(Some("cpu_s"), x.cpu_s);
            j.uint(Some("bots_in_world"), x.bots_in_world as u64);
            j.uint(Some("bot_entities"), x.bot_entities as u64);
            j.uint(Some("bot_pending"), x.bot_pending as u64);
            if let Some(v) = &x.server {
                j.uint(Some("connections"), v.connections as u64);
                j.uint(Some("in_world"), v.in_world as u64);
                j.uint(Some("entities"), v.entities as u64);
                j.uint(Some("interest_pairs"), v.interest_pairs as u64);
                j.uint(Some("accounted_bytes"), v.accounted_bytes as u64);
                j.uint(Some("queue_bytes"), v.queue_bytes as u64);
                j.uint(Some("overruns"), v.overruns);
            }
            j.end('}');
        }
        j.end(']');
        j.end('}');
        j.out.push('\n');
        j.out
    }

    fn json_config(&self, j: &mut Json) {
        let i = &self.inputs;
        let o = &i.opts;
        let b = &o.behaviour;
        let p = &i.params;
        let secs = |d: Option<Duration>| d.map(|d| d.as_secs_f64());
        j.begin(Some("config"), '{');
        match &o.target {
            TargetOptions::InProcess { data_dir } => {
                j.str(Some("target"), "in-process");
                j.bool(Some("own_data_dir"), data_dir.is_some());
            }
            TargetOptions::Remote {
                connect, register, ..
            } => {
                j.str(Some("target"), "remote");
                j.str(Some("connect"), connect);
                j.bool(Some("register"), *register);
            }
        }
        j.uint(Some("area_blocks"), u64::from(i.area_blocks));
        j.str(Some("account_prefix"), &o.account_prefix);
        j.str(Some("name_prefix"), &o.name_prefix);
        j.num(Some("sample_interval_s"), o.sample_interval.as_secs_f64());
        j.num(Some("warmup_min_s"), o.warmup_min.as_secs_f64());
        j.num(Some("stagger_s"), o.stagger.as_secs_f64());
        j.bool(Some("check_throttle"), o.check_throttle);
        j.bool(Some("reference_machine"), o.reference_machine);
        j.opt_uint(Some("server_pid"), o.server_pid.map(u64::from));
        match o.hostile {
            Some(h) => {
                j.begin(Some("hostile"), '{');
                j.uint(Some("clients"), h.clients as u64);
                j.num(Some("pace_s"), h.pace.as_secs_f64());
                j.end('}');
            }
            None => j.null(Some("hostile")),
        }
        j.begin(Some("behaviour"), '{');
        j.num(Some("walk_fraction"), b.walk_fraction);
        j.num(Some("run_fraction"), b.run_fraction);
        j.num(Some("walk_speed"), b.walk_speed);
        j.num(Some("run_speed"), b.run_speed);
        j.opt(Some("chat_interval_s"), secs(b.chat_interval));
        j.opt(Some("teleport_interval_s"), secs(b.teleport_interval));
        j.num(Some("far_teleport_fraction"), b.far_teleport_fraction);
        j.opt(Some("reconnect_interval_s"), secs(b.reconnect_interval));
        j.num(Some("reconnect_fraction"), b.reconnect_fraction);
        j.opt(Some("appearance_interval_s"), secs(b.appearance_interval));
        j.opt(Some("cheat_interval_s"), secs(b.cheat_interval));
        j.num(Some("cheat_fraction"), b.cheat_fraction);
        j.uint(Some("channels"), u64::from(b.channels));
        j.opt(Some("channel_cycle_s"), secs(b.channel_cycle));
        j.num(Some("channel_fraction"), b.channel_fraction);
        j.num(Some("truth_bias_m"), b.truth_bias_m);
        j.end('}');
        j.begin(Some("server_params"), '{');
        j.uint(Some("interest_radius"), u64::from(i.desync.config().radius));
        j.uint(Some("tick_rate"), u64::from(p.tick_rate));
        j.uint(Some("snapshot_rate"), u64::from(p.snapshot_rate));
        j.uint(Some("creates_per_tick"), u64::from(p.creates_per_tick));
        j.opt_uint(
            Some("instance_capacity"),
            p.instance_capacity.map(u64::from),
        );
        j.opt(Some("say_range_m"), p.say_range);
        j.opt(Some("walk_speed"), p.walk_speed);
        j.opt(Some("run_speed"), p.run_speed);
        j.opt_uint(Some("wanderers"), p.wanderers.map(|w| w as u64));
        j.opt(
            Some("interpolation_delay_ms"),
            i.interpolation_delay.map(|d| d.as_secs_f64() * 1e3),
        );
        j.end('}');
        j.end('}');
    }
}

fn json_memory(j: &mut Json, key: &str, m: &MemoryVerdict, judged: bool) {
    j.begin(Some(key), '{');
    j.bool(Some("judged"), judged);
    j.num(Some("warmup_s"), m.warmup_s);
    j.uint(Some("warmup_max_bytes"), m.warmup_max);
    j.num(Some("slope_bytes_per_s"), m.slope);
    j.num(Some("growth_bytes"), m.growth);
    j.uint(Some("allowance_bytes"), m.allowance);
    j.uint(Some("max_after_bytes"), m.max_after);
    j.uint(Some("samples_after"), m.samples_after as u64);
    match m.pass {
        Some(p) => j.bool(Some("ok"), p),
        None => j.null(Some("ok")),
    }
    j.end('}');
}

/// A small JSON writer: nested objects and arrays, two-space indentation.
#[derive(Default)]
struct Json {
    out: String,
    /// Per open container: no member written yet.
    first: Vec<bool>,
}

impl Json {
    fn indent(&mut self) {
        for _ in 0..self.first.len() {
            self.out.push_str("  ");
        }
    }

    /// Starts a member (`key` inside an object, `None` inside an array or at the top).
    fn key(&mut self, k: Option<&str>) {
        if let Some(f) = self.first.last_mut() {
            if !*f {
                self.out.push(',');
            }
            *f = false;
            self.out.push('\n');
            self.indent();
        }
        if let Some(k) = k {
            quote(&mut self.out, k);
            self.out.push_str(": ");
        }
    }

    fn begin(&mut self, k: Option<&str>, open: char) {
        self.key(k);
        self.out.push(open);
        self.first.push(true);
    }

    fn end(&mut self, close: char) {
        let empty = self.first.pop().unwrap_or(true);
        if !empty {
            self.out.push('\n');
            self.indent();
        }
        self.out.push(close);
    }

    fn str(&mut self, k: Option<&str>, v: &str) {
        self.key(k);
        quote(&mut self.out, v);
    }

    fn strs(&mut self, k: Option<&str>, v: &[String]) {
        self.key(k);
        self.out.push('[');
        for (n, x) in v.iter().enumerate() {
            if n > 0 {
                self.out.push_str(", ");
            }
            quote(&mut self.out, x);
        }
        self.out.push(']');
    }

    fn num(&mut self, k: Option<&str>, v: f64) {
        self.key(k);
        if v.is_finite() {
            let _ = write!(self.out, "{v}");
        } else {
            self.out.push_str("null");
        }
    }

    fn uint(&mut self, k: Option<&str>, v: u64) {
        self.key(k);
        let _ = write!(self.out, "{v}");
    }

    fn opt(&mut self, k: Option<&str>, v: Option<f64>) {
        match v {
            Some(v) => self.num(k, v),
            None => self.null(k),
        }
    }

    fn opt_uint(&mut self, k: Option<&str>, v: Option<u64>) {
        match v {
            Some(v) => self.uint(k, v),
            None => self.null(k),
        }
    }

    fn null(&mut self, k: Option<&str>) {
        self.key(k);
        self.out.push_str("null");
    }

    fn bool(&mut self, k: Option<&str>, v: bool) {
        self.key(k);
        self.out.push_str(if v { "true" } else { "false" });
    }
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_and_nests() {
        let mut j = Json::default();
        j.begin(None, '{');
        j.str(Some("a"), "x\"y\\z\n\u{1}");
        j.num(Some("n"), f64::NAN);
        j.opt(Some("o"), None);
        j.bool(Some("b"), true);
        j.uint(Some("u"), u64::MAX);
        j.strs(Some("s"), &["1".into(), "2".into()]);
        j.begin(Some("arr"), '[');
        j.begin(None, '{');
        j.uint(Some("k"), 1);
        j.end('}');
        j.uint(None, 2);
        j.end(']');
        j.begin(Some("empty"), '{');
        j.end('}');
        j.end('}');
        assert_eq!(
            j.out,
            "{\n  \"a\": \"x\\\"y\\\\z\\n\\u0001\",\n  \"n\": null,\n  \"o\": null,\n  \"b\": true,\n  \"u\": 18446744073709551615,\n  \"s\": [\"1\", \"2\"],\n  \"arr\": [\n    {\n      \"k\": 1\n    },\n    2\n  ],\n  \"empty\": {}\n}"
        );
    }

    fn cheats(sent: u64, corrected: u64, bad: u64, unanswered: u64) -> Totals {
        Totals {
            cheats: sent,
            cheats_corrected: corrected,
            cheats_bad_correction: bad,
            cheats_unanswered: unanswered,
            ..Totals::default()
        }
    }

    /// Regression: the cheat scenario could not fail. A server without movement
    /// validation (no correction, no drop, no violation) now fails it.
    #[test]
    fn cheat_verdict_needs_a_correction_or_a_drop() {
        let none = ServerCheatCounts::default();
        let (f, _) = cheat_verdict(&cheats(11, 0, 0, 11), Some(none));
        assert_eq!(f.len(), 2, "{f:?}");
        // Every cheat corrected back: pass.
        let good = ServerCheatCounts {
            corrections: 11,
            dropped: 0,
        };
        assert!(
            cheat_verdict(&cheats(11, 11, 0, 0), Some(good))
                .0
                .is_empty()
        );
        // Dropped instead of corrected: pass.
        let dropped = ServerCheatCounts {
            corrections: 0,
            dropped: 11,
        };
        assert!(
            cheat_verdict(&cheats(11, 0, 0, 11), Some(dropped))
                .0
                .is_empty()
        );
        // Corrected to the wrong place: fail.
        assert_eq!(cheat_verdict(&cheats(3, 2, 1, 0), Some(good)).0.len(), 1);
        // Remote: unanswered cheats are a coverage note, not a failure.
        let (f, n) = cheat_verdict(&cheats(4, 0, 0, 4), None);
        assert!(f.is_empty());
        assert_eq!(n.len(), 1);
        assert!(cheat_verdict(&Totals::default(), Some(none)).0.is_empty());
    }
}
