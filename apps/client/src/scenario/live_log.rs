// SPDX-License-Identifier: GPL-3.0-or-later
//! `--scenario-log` in a windowed run (`scenario-feed.md` §12.4): the canonical log of §10
//! written by the client's [`crate::net::World`] from the commands it sends and the events
//! it receives. Its times follow the wall clock (milliseconds since the frame that
//! delivered `WorldEntered`; the end line carries the feed's virtual time), so it is not
//! normative and two runs may differ; the headless driver writes the normative log.
//!
//! The log is written until its end line: the feed finished or failed, or the session
//! ended first (`reason=error error=Disconnected`, which is also what closing the window
//! during the scenario writes). A write error stops the log, is reported once and makes the
//! run exit 5 ([`LiveLogSlot::failed`]).

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_net::scenario::{
    EndCounters, EndReason, FinishReason, LogTime, LogWriter, ResolvedScenario, ScenarioCounters,
    ScenarioStatus,
};
use tessera_net::{Command, Event};

/// The output of a windowed run's log, handed from [`crate::scenario::run`] (which opens
/// it, refusing paths inside the install) to the world session the runtime creates. Cloning
/// shares the slot; the first [`LiveLogSlot::take`] gets the writer.
#[derive(Clone, Default)]
pub struct LiveLogSlot {
    out: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    failed: Arc<AtomicBool>,
}

impl std::fmt::Debug for LiveLogSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveLogSlot")
            .field("pending", &self.lock().is_some())
            .field("failed", &self.failed())
            .finish()
    }
}

impl LiveLogSlot {
    /// A slot holding `out`.
    #[must_use]
    pub fn new(out: Box<dyn Write + Send>) -> Self {
        Self {
            out: Arc::new(Mutex::new(Some(out))),
            failed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn Write + Send>>> {
        self.out.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A [`LiveLog`] for `scenario` over the slot's writer (header written), once; `None`
    /// when it was taken already.
    #[must_use]
    pub fn take(&self, scenario: &ResolvedScenario) -> Option<LiveLog> {
        let out = self.lock().take()?;
        Some(LiveLog::new(out, scenario, Arc::clone(&self.failed)))
    }

    /// Whether writing the log failed (the run then exits 5).
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

/// The log's output, shared between the [`LogWriter`] and [`LiveLog::flush`].
#[derive(Clone)]
struct SharedOut(Arc<Mutex<Box<dyn Write + Send>>>);

impl SharedOut {
    fn lock(&self) -> std::sync::MutexGuard<'_, Box<dyn Write + Send>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Write for SharedOut {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.lock().flush()
    }
}

/// Where the log is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Writing,
    Ended,
    Failed,
}

/// The canonical log of a windowed scenario run, with wall-clock times (see the module).
pub struct LiveLog {
    writer: LogWriter<SharedOut>,
    /// The writer's output, kept to flush it ([`LogWriter`] owns its own handle).
    out: SharedOut,
    entered_at: Option<Instant>,
    state: State,
    failed: Arc<AtomicBool>,
}

impl std::fmt::Debug for LiveLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveLog")
            .field("entered_at", &self.entered_at)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl LiveLog {
    /// A log for `scenario` into `out`; writes the header and notes (§10.2 items 1–5).
    /// `failed` is set when a write fails.
    #[must_use]
    pub fn new(
        out: Box<dyn Write + Send>,
        scenario: &ResolvedScenario,
        failed: Arc<AtomicBool>,
    ) -> Self {
        let out = SharedOut(Arc::new(Mutex::new(out)));
        let mut log = Self {
            writer: LogWriter::new(out.clone(), scenario),
            out,
            entered_at: None,
            state: State::Writing,
            failed,
        };
        let r = log.writer.header(scenario);
        log.check(r);
        log
    }

    /// Whether the end line has been written (or writing failed): nothing more is logged.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state != State::Writing
    }

    fn check(&mut self, r: std::io::Result<()>) {
        if let Err(e) = r {
            tracing::error!("writing the scenario log failed: {e}; the log stops here");
            self.state = State::Failed;
            self.failed.store(true, Ordering::Relaxed);
        }
    }

    fn time(&self, now: Instant) -> LogTime {
        match self.entered_at {
            None => LogTime::Pre,
            Some(at) => LogTime::At(
                u64::try_from(now.saturating_duration_since(at).as_millis()).unwrap_or(u64::MAX),
            ),
        }
    }

    /// Logs a command the session accepted at `now` (`Move` and `QueryLocation` are left
    /// out, §10.2).
    pub fn command(&mut self, now: Instant, command: &Command) {
        if self.state != State::Writing {
            return;
        }
        let r = self.writer.command(self.time(now), command);
        self.check(r);
    }

    /// Logs an event received at `now`; `WorldEntered` starts the clock (time 0).
    pub fn event(&mut self, now: Instant, event: &Event) {
        if self.state != State::Writing {
            return;
        }
        if self.entered_at.is_none() && matches!(event, Event::WorldEntered { .. }) {
            self.entered_at = Some(now);
        }
        let r = self.writer.event(self.time(now), event);
        self.check(r);
    }

    /// After a poll at `now`: writes the end line when the feed finished or failed, or when
    /// the session ended (`disconnected`) before it did.
    pub fn after_poll(
        &mut self,
        now: Instant,
        status: Option<ScenarioStatus>,
        counters: Option<ScenarioCounters>,
        disconnected: bool,
    ) {
        if self.state != State::Writing {
            return;
        }
        let (time, reason) = match status {
            Some(ScenarioStatus::Finished { time_ms, reason }) => (
                LogTime::At(time_ms),
                match reason {
                    FinishReason::Horizon => EndReason::Horizon,
                    _ => EndReason::Complete,
                },
            ),
            Some(ScenarioStatus::Failed { time_ms, error }) => (
                LogTime::At(time_ms),
                EndReason::Error {
                    kind: runtime_kind(error.kind),
                    line: error.line,
                },
            ),
            _ if disconnected => (
                self.time(now),
                EndReason::Error {
                    kind: "Disconnected",
                    line: 0,
                },
            ),
            _ => return,
        };
        let c = counters.unwrap_or_default();
        let counters = EndCounters {
            skipped: c.skipped,
            // Windowed runs play no input script (§12.4 OPEN (1)).
            input_skipped: 0,
            dropped: c.dropped,
            spilled: c.spilled,
        };
        let r = self.writer.end(time, reason, counters);
        self.check(r);
        if self.state == State::Writing {
            self.state = State::Ended;
        }
    }

    /// Flushes what was written (the end line flushes by itself).
    pub fn flush(&mut self) {
        if self.state == State::Failed {
            return;
        }
        let r = self.out.lock().flush();
        self.check(r);
    }
}

impl Drop for LiveLog {
    fn drop(&mut self) {
        self.flush();
    }
}

/// The error kind names of the end line (§10.2 item 7), as the headless driver writes them.
fn runtime_kind(k: tessera_net::scenario::RuntimeErrorKind) -> &'static str {
    use tessera_net::scenario::RuntimeErrorKind as K;
    match k {
        K::QueueOverflow => "QueueOverflow",
        K::EventOverflow => "EventOverflow",
        K::AgendaOverflow => "AgendaOverflow",
        K::ItemOverflow => "ItemOverflow",
        K::IdOverflow => "IdOverflow",
        _ => "RuntimeError",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::scenario::{EmptyResolver, Scenario};
    use tessera_net::{EntityId, Transform, Vec3};

    fn scenario() -> ResolvedScenario {
        Scenario::parse(b"tessera-scenario 1\ntitle \"T\"\nplayer \"Tamsin\"\n")
            .expect("parses")
            .resolve(&mut EmptyResolver)
            .expect("resolves")
    }

    /// A writer whose bytes the test reads back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_log_has_wall_clock_times_and_one_end_line() {
        let sc = scenario();
        let out = Shared::default();
        let slot = LiveLogSlot::new(Box::new(out.clone()));
        let mut log = slot.take(&sc).expect("first take");
        assert!(slot.take(&sc).is_none(), "the writer is handed out once");
        let t0 = Instant::now();
        log.command(t0, &Command::Connect);
        log.event(
            t0 + std::time::Duration::from_millis(30),
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
        );
        log.command(
            t0 + std::time::Duration::from_millis(1_280),
            &Command::Disconnect,
        );
        log.after_poll(
            t0,
            Some(ScenarioStatus::Running { time_ms: 0 }),
            None,
            false,
        );
        assert!(!log.is_closed());
        log.after_poll(
            t0,
            Some(ScenarioStatus::Finished {
                time_ms: 1_250,
                reason: FinishReason::Complete,
            }),
            Some(ScenarioCounters {
                skipped: 2,
                dropped: 0,
                spilled: 0,
            }),
            false,
        );
        assert!(log.is_closed());
        log.event(t0, &Event::StaticsCleared);
        drop(log);
        let text = String::from_utf8(out.0.lock().expect("lock").clone()).expect("utf-8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "tessera-scenario-log 1");
        assert!(lines.contains(&"pre > Connect"), "{text}");
        assert!(
            lines.iter().any(|l| l.starts_with("0.000 WorldEntered")),
            "{text}"
        );
        assert!(
            lines.iter().any(|l| l.starts_with("1.250 > Disconnect")),
            "{text}"
        );
        assert_eq!(
            lines.last().copied(),
            Some("1.250 end reason=complete skipped=2 input-skipped=0 dropped=0 spilled=0")
        );
        assert!(!slot.failed());
    }

    #[test]
    fn a_disconnect_before_the_end_ends_the_log_with_an_error() {
        let sc = scenario();
        let out = Shared::default();
        let mut log = LiveLog::new(Box::new(out.clone()), &sc, Arc::default());
        log.after_poll(Instant::now(), None, None, true);
        assert!(log.is_closed());
        let text = String::from_utf8(out.0.lock().expect("lock").clone()).expect("utf-8");
        assert!(
            text.lines()
                .last()
                .is_some_and(|l| l.starts_with("pre end reason=error")
                    && l.ends_with("error=Disconnected line=0")),
            "{text}"
        );
    }

    #[test]
    fn a_write_error_stops_the_log_and_is_reported() {
        let sc = scenario();
        let slot = LiveLogSlot::new(Box::new(Broken));
        let mut log = slot.take(&sc).expect("take");
        assert!(log.is_closed(), "the header could not be written");
        assert!(slot.failed());
        log.command(Instant::now(), &Command::Connect);
        assert!(format!("{slot:?}").contains("failed: true"));
    }
}
