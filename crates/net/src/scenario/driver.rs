// SPDX-License-Identifier: GPL-3.0-or-later
//! The headless driver (`scenario-feed.md` §12.2, §12.3): owns the virtual clock and the
//! session, plays the input script, keeps the mirror the input forms read, and writes the
//! canonical log.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::feed::{FinishReason, ScenarioStatus};
use super::input::{InputLine, InputPlayer};
use super::log::{EndCounters, EndReason, LogTime, LogWriter};
use super::resolve::ResolvedScenario;
use crate::sandbox::SandboxBackend;
use crate::session::{Command, Event, Session};

/// How a headless run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverOutcome {
    /// The end reason written to the log.
    pub reason: EndReason,
    /// The time of the end line.
    pub time: LogTime,
    /// The counters of the end line.
    pub counters: EndCounters,
}

impl DriverOutcome {
    /// The client's exit code for this outcome: 0 finished, 5 run-time error (§12.2).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.reason {
            EndReason::Complete | EndReason::Horizon => 0,
            EndReason::Error { .. } => 5,
        }
    }
}

/// One frame of a headless run, handed to the callback of [`HeadlessDriver::run_frames`].
#[derive(Debug)]
#[non_exhaustive]
pub struct DriverFrame<'a> {
    /// The `now` the driver passed to this poll: the run's base instant plus the virtual
    /// time (never the wall clock once the world is entered), so a walker stepping
    /// animation or physics from it stays deterministic.
    pub now: Instant,
    /// The poll's log time: `pre` before world entry, otherwise the virtual time of the
    /// tick (the time of the last event for the world-entry poll).
    pub time: LogTime,
    /// The events of this poll.
    pub events: &'a [Event],
    /// The session after this poll, for example to read the backend's diagnostics.
    pub session: &'a Session,
}

/// Runs a resolved scenario headless over a [`Session`] whose backend is a
/// [`SandboxBackend`] playing it, and writes the canonical log (`scenario-feed.md` §12.2).
///
/// The driver supplies `now` = base + virtual time and never reads the wall clock for the
/// feed; it only pauses one tick of wall-clock time between world-entry wait polls, so a
/// pending spawn lookup of the world source can answer.
#[derive(Debug)]
pub struct HeadlessDriver<W: io::Write> {
    session: Session,
    sc: Arc<ResolvedScenario>,
    log: Option<LogWriter<W>>,
    log_failed: bool,
    input: InputPlayer,
    tick: u64,
    resolve_timeout: Duration,
    max_ticks: Option<u64>,
    events: Vec<Event>,
}

impl<W: io::Write> HeadlessDriver<W> {
    /// A driver for `session` (a fresh session over a sandbox playing `scenario`), writing
    /// the log to `log` when given. The resolve timeout is the sandbox configuration's
    /// ([`SandboxBackend::resolve_timeout`]; 10 s for another backend).
    pub fn new(session: Session, scenario: Arc<ResolvedScenario>, log: Option<W>) -> Self {
        let log = log.map(|w| LogWriter::new(w, &scenario));
        let resolve_timeout = session
            .backend::<SandboxBackend>()
            .map_or(Duration::from_secs(10), SandboxBackend::resolve_timeout);
        Self {
            session,
            tick: scenario.tick_ms().max(1),
            input: InputPlayer::new(Arc::clone(&scenario)),
            sc: scenario,
            log,
            log_failed: false,
            resolve_timeout,
            max_ticks: None,
            events: Vec::new(),
        }
    }

    /// Overrides the resolve timeout the driver waits for world entry (§3.2); by default
    /// it is the sandbox configuration's.
    #[must_use]
    pub fn with_resolve_timeout(mut self, timeout: Duration) -> Self {
        self.resolve_timeout = timeout;
        self
    }

    /// Ends the run after at most `ticks` ticks of virtual time, with the end reason
    /// `Horizon` as at the real horizon. For fuzzing and tests; by default the driver runs
    /// to the horizon of §7.3 plus the draining ticks.
    #[must_use]
    pub fn with_max_ticks(mut self, ticks: u64) -> Self {
        self.max_ticks = Some(ticks);
        self
    }

    /// Lowers the log's size limit to `bytes` (at most
    /// [`MAX_LOG_BYTES`](super::MAX_LOG_BYTES)); for tests.
    #[must_use]
    pub fn with_log_limit(mut self, bytes: u64) -> Self {
        self.log = self.log.map(|l| l.with_limit(bytes));
        self
    }

    /// The session (for example to read the backend's diagnostics).
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The log writer's underlying output, once the run is over.
    pub fn into_log(self) -> Option<W> {
        self.log.map(LogWriter::into_inner)
    }

    /// Runs the scenario to its end.
    pub fn run(&mut self) -> DriverOutcome {
        self.run_with(|_| {})
    }

    /// Runs the scenario to its end, handing `frame` the events of every poll (a walker
    /// renders from them).
    pub fn run_with(&mut self, mut frame: impl FnMut(&[Event])) -> DriverOutcome {
        self.run_frames(|f| frame(f.events))
    }

    /// Runs the scenario to its end, handing `frame` every poll's [`DriverFrame`]: the virtual
    /// clock's `now` and log time, the events and the session.
    pub fn run_frames(&mut self, mut frame: impl FnMut(&DriverFrame<'_>)) -> DriverOutcome {
        let base = Instant::now();
        if let Some(log) = &mut self.log
            && log.header(&self.sc).is_err()
        {
            self.log_failed = true;
        }
        // Pre-world: Connect, the `at pre` lines, EnterWorld.
        self.send(base, LogTime::Pre, Command::Connect);
        self.poll(base, LogTime::Pre, |_| LogTime::Pre, &mut frame);
        while let Some(line) = self.input.next_pre() {
            self.send_input(base, LogTime::Pre, line);
            self.poll(base, LogTime::Pre, |_| LogTime::Pre, &mut frame);
        }
        let enter = self
            .sc
            .enter_character()
            .map(str::to_owned)
            .or_else(|| self.input.characters().first().cloned());
        let Some(enter) = enter else {
            return self.finish(LogTime::Pre, error("Disconnected", 0));
        };
        self.send(base, LogTime::Pre, Command::EnterWorld { character: enter });
        let tick = Duration::from_millis(self.tick);
        let mut w: u32 = 0;
        let t0 = loop {
            let now = base + tick * w;
            let mut entered = false;
            let disconnected = self.poll(
                now,
                LogTime::Pre,
                |e| {
                    if matches!(e, Event::WorldEntered { .. }) {
                        entered = true;
                    }
                    if entered {
                        LogTime::At(0)
                    } else {
                        LogTime::Pre
                    }
                },
                &mut frame,
            );
            if entered {
                break now;
            }
            if disconnected || now > base + self.resolve_timeout + tick {
                return self.finish(LogTime::Pre, error("Disconnected", 0));
            }
            std::thread::sleep(tick);
            w = w.saturating_add(1);
        };
        if let Some(o) = self.check(0, false) {
            return o;
        }
        let horizon = super::parse::HORIZON_MS.div_ceil(self.tick) + 4_096;
        let limit = self.max_ticks.map_or(horizon, |m| m.min(horizon));
        for k in 1..=limit {
            let ms = k * self.tick;
            let now = t0 + Duration::from_millis(ms);
            let time = LogTime::At(ms);
            // Timed input, by tick then time then file order.
            while let Some(line) = self.input.next_due(k) {
                self.send_input(now, time, line);
            }
            let disconnected = self.poll(now, time, |_| time, &mut frame);
            if let Some(o) = self.check(ms, disconnected) {
                return o;
            }
        }
        if limit < horizon {
            // The lowered horizon of `with_max_ticks`.
            return self.finish(LogTime::At(limit * self.tick), EndReason::Horizon);
        }
        self.finish(LogTime::At(limit * self.tick), error("Horizon", 0))
    }

    /// Sends an input line's command, or counts it as skipped (§7.7).
    fn send_input(&mut self, now: Instant, time: LogTime, line: InputLine) {
        match line.command {
            Some(c) => {
                if !self.send(now, time, c) {
                    self.input.skip(line.line, "refused by the session");
                }
            }
            None => self.input.skip(line.line, "target does not exist"),
        }
    }

    fn counters(&self) -> EndCounters {
        let c = self
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_counters)
            .unwrap_or_default();
        EndCounters {
            skipped: c.skipped,
            input_skipped: self.input.skipped(),
            dropped: c.dropped,
            spilled: c.spilled,
        }
    }

    /// Ends the run when the feed finished or failed, or the session ended.
    fn check(&mut self, ms: u64, disconnected: bool) -> Option<DriverOutcome> {
        let status = self
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_status);
        match status {
            Some(ScenarioStatus::Finished { time_ms, reason }) => {
                let reason = match reason {
                    FinishReason::Horizon => EndReason::Horizon,
                    _ => EndReason::Complete,
                };
                let out = self.finish(LogTime::At(time_ms), reason);
                let _ = self.session.send(Instant::now(), Command::Disconnect);
                Some(out)
            }
            Some(ScenarioStatus::Failed { time_ms, error: e }) => Some(self.finish(
                LogTime::At(time_ms),
                EndReason::Error {
                    kind: runtime_kind(e.kind),
                    line: e.line,
                },
            )),
            // The log reached its size limit: stop at once (exit 5).
            _ if self.log_overflowed() => {
                Some(self.finish(LogTime::At(ms), error("LogOverflow", 0)))
            }
            _ if disconnected => Some(self.finish(LogTime::At(ms), error("Disconnected", 0))),
            None => Some(self.finish(LogTime::At(ms), error("Disconnected", 0))),
            _ => None,
        }
    }

    /// Whether the log reached its size limit ([`LogWriter::overflowed`]).
    fn log_overflowed(&self) -> bool {
        self.log.as_ref().is_some_and(LogWriter::overflowed)
    }

    fn finish(&mut self, time: LogTime, reason: EndReason) -> DriverOutcome {
        let mut reason = reason;
        if self.log_overflowed() {
            reason = error("LogOverflow", 0);
        } else if self.log_failed {
            reason = error("LogWrite", 0);
        }
        let counters = self.counters();
        if let Some(log) = &mut self.log
            && log.end(time, reason, counters).is_err()
        {
            reason = error("LogWrite", 0);
        }
        DriverOutcome {
            reason,
            time,
            counters,
        }
    }

    /// Sends a command, logging it on success.
    fn send(&mut self, now: Instant, time: LogTime, cmd: Command) -> bool {
        let logged = cmd.clone();
        if self.session.send(now, cmd).is_err() {
            return false;
        }
        if let Some(log) = &mut self.log
            && !self.log_failed
            && log.command(time, &logged).is_err()
        {
            self.log_failed = true;
        }
        true
    }

    /// Polls once, logs and mirrors the events; returns whether `Disconnected` arrived.
    fn poll(
        &mut self,
        now: Instant,
        base_time: LogTime,
        mut time: impl FnMut(&Event) -> LogTime,
        frame: &mut impl FnMut(&DriverFrame<'_>),
    ) -> bool {
        let mut events = std::mem::take(&mut self.events);
        events.clear();
        self.session.poll_into(now, &mut events);
        let mut disconnected = false;
        let mut frame_time = base_time;
        for e in &events {
            let t = time(e);
            frame_time = t;
            if let Some(log) = &mut self.log
                && !self.log_failed
                && log.event(t, e).is_err()
            {
                self.log_failed = true;
            }
            if matches!(e, Event::Disconnected { .. }) {
                disconnected = true;
            }
            self.input.observe(e);
        }
        frame(&DriverFrame {
            now,
            time: frame_time,
            events: &events,
            session: &self.session,
        });
        self.events = events;
        disconnected
    }
}

fn error(kind: &'static str, line: u32) -> EndReason {
    EndReason::Error { kind, line }
}

fn runtime_kind(k: super::error::RuntimeErrorKind) -> &'static str {
    use super::error::RuntimeErrorKind as K;
    match k {
        K::QueueOverflow => "QueueOverflow",
        K::EventOverflow => "EventOverflow",
        K::AgendaOverflow => "AgendaOverflow",
        K::ItemOverflow => "ItemOverflow",
        K::IdOverflow => "IdOverflow",
    }
}
