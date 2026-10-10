// SPDX-License-Identifier: GPL-3.0-or-later
//! `--scenario-log` in a windowed run (`scenario-feed.md` §12.4): the canonical log of §10
//! written by the client's [`crate::net::World`] from the commands it sends and the events
//! it receives. Its times follow the wall clock (milliseconds since the frame that
//! delivered `WorldEntered`; the end line carries the feed's virtual time), so it is not
//! normative and two runs may differ; the headless driver writes the normative log.
//!
//! The frame loop only formats lines (the same [`LogWriter`] as the headless driver, so
//! the bytes are the same); a writer thread does the I/O (`docs/ARCHITECTURE.md` §21). The
//! formatted lines go to the thread in chunks (the lines of a frame, or [`CHUNK_BYTES`] of
//! them) through a bounded channel ([`MAX_QUEUED_CHUNKS`]). The frame never waits: a chunk
//! the full channel does not take stays with the frame loop and goes with the next one.
//! What is formatted but not yet written is bounded by [`MAX_QUEUED_BYTES`]; when the
//! thread falls that far behind (a standard-output reader that stopped reading, a stalled
//! disk), the log drops the lines from there on and ends with
//! `reason=error error=LogWrite line=0`, and the run exits 5.
//!
//! The log is written until its end line: the feed finished or failed, or the session
//! ended first (`reason=error error=Disconnected`, which is also what closing the window
//! during the scenario writes). A write error stops the log, is reported once and makes the
//! run exit 5 ([`LiveLogSlot::failed`]); a log that reaches its size limit
//! ([`tessera_net::scenario::MAX_LOG_BYTES`]) ends with `error=LogOverflow`, also exit 5.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_net::scenario::{
    EndCounters, EndReason, FinishReason, LogTime, LogWriter, ResolvedScenario, ScenarioCounters,
    ScenarioStatus,
};
use tessera_net::{Command, Event};

/// The capacity of the channel to the writer thread, in chunks.
pub const MAX_QUEUED_CHUNKS: usize = 256;

/// The formatted lines are offered to the writer thread once they hold this many bytes, and
/// at the end of every frame.
pub const CHUNK_BYTES: usize = 64 * 1024;

/// The bytes formatted but not yet written (queued in the channel, being written, or held
/// by the frame loop while the channel is full) beyond which the log gives up rather than
/// grow or block the frame: 16 MiB. A chunk may pass it by one line.
pub const MAX_QUEUED_BYTES: usize = 16 * 1024 * 1024;

/// How long dropping a log waits for the writer thread to finish writing.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

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

    /// A [`LiveLog`] for `scenario` over the slot's writer (header formatted), once; `None`
    /// when it was taken already.
    #[must_use]
    pub fn take(&self, scenario: &ResolvedScenario) -> Option<LiveLog> {
        let out = self.lock().take()?;
        Some(LiveLog::new(out, scenario, Arc::clone(&self.failed)))
    }

    /// Whether writing the log failed (the run then exits 5). Final once the [`LiveLog`]
    /// is dropped.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

/// What the writer thread shares with the frame loop.
struct ThreadShared {
    /// The bytes after the last chunk (the lines left when the log closed, and the end
    /// line), written once the channel is closed and drained.
    tail: Mutex<Option<Vec<u8>>>,
    /// Bytes sent through the channel and not yet written.
    queued: AtomicUsize,
    failed: Arc<AtomicBool>,
}

/// The writer thread: writes the chunks in order, flushing whenever it has caught up; when
/// the frame loop closes the channel it writes the tail and flushes. A write error sets
/// `failed` and stops the thread (the frame loop then sees a closed channel).
fn writer_thread(mut out: Box<dyn Write + Send>, rx: Receiver<Vec<u8>>, shared: Arc<ThreadShared>) {
    let fail = |e: std::io::Error| {
        tracing::error!("writing the scenario log failed: {e}; the log stops here");
        shared.failed.store(true, Ordering::Relaxed);
    };
    let write = |out: &mut Box<dyn Write + Send>, chunk: Vec<u8>| {
        let r = out.write_all(&chunk);
        shared.queued.fetch_sub(chunk.len(), Ordering::Relaxed);
        r
    };
    while let Ok(chunk) = rx.recv() {
        if let Err(e) = write(&mut out, chunk) {
            return fail(e);
        }
        // Write what else is queued, then flush once caught up, so a reader sees the
        // lines of a frame soon after it.
        while let Ok(next) = rx.try_recv() {
            if let Err(e) = write(&mut out, next) {
                return fail(e);
            }
        }
        if let Err(e) = out.flush() {
            return fail(e);
        }
    }
    let tail = shared
        .tail
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    let r = match tail {
        Some(bytes) => out.write_all(&bytes).and_then(|()| out.flush()),
        None => out.flush(),
    };
    if let Err(e) = r {
        fail(e);
    }
}

/// Where the log is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Writing,
    /// No more lines are taken; the next [`LiveLog::after_poll`] ends the log with
    /// `error=KIND`: `LogWrite` when the writer thread fell [`MAX_QUEUED_BYTES`] behind
    /// (the unwritten lines were dropped), `LogOverflow` when the log reached its size
    /// limit (the lines before the refused one are kept).
    Stopping(&'static str),
    Ended,
    Failed,
}

/// The canonical log of a windowed scenario run, with wall-clock times (see the module).
pub struct LiveLog {
    /// Formats the lines into memory; its buffer holds the lines not yet sent to the
    /// writer thread.
    writer: LogWriter<Vec<u8>>,
    /// The channel to the writer thread; `None` once closed.
    tx: Option<SyncSender<Vec<u8>>>,
    shared: Arc<ThreadShared>,
    /// The limit on unwritten bytes ([`MAX_QUEUED_BYTES`]; lower in tests).
    queue_limit: usize,
    /// Signals that the thread has finished.
    done: Option<Receiver<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
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
    /// A log for `scenario` into `out`, written by a new writer thread; formats the header
    /// and notes (§10.2 items 1–5). `failed` is set when a write fails.
    #[must_use]
    pub fn new(
        out: Box<dyn Write + Send>,
        scenario: &ResolvedScenario,
        failed: Arc<AtomicBool>,
    ) -> Self {
        Self::with_limits(
            out,
            scenario,
            failed,
            tessera_net::scenario::MAX_LOG_BYTES,
            MAX_QUEUED_BYTES,
        )
    }

    /// [`LiveLog::new`] with the log's size limit lowered to `log_limit` bytes and the
    /// unwritten bytes to `queue_limit` (tests).
    fn with_limits(
        out: Box<dyn Write + Send>,
        scenario: &ResolvedScenario,
        failed: Arc<AtomicBool>,
        log_limit: u64,
        queue_limit: usize,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(MAX_QUEUED_CHUNKS);
        let (done_tx, done) = std::sync::mpsc::sync_channel(1);
        let shared = Arc::new(ThreadShared {
            tail: Mutex::new(None),
            queued: AtomicUsize::new(0),
            failed: Arc::clone(&failed),
        });
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("scenario-log".into())
                .spawn(move || {
                    writer_thread(out, rx, shared);
                    let _ = done_tx.send(());
                })
        };
        let (tx, thread, done, state) = match thread {
            Ok(t) => (Some(tx), Some(t), Some(done), State::Writing),
            Err(e) => {
                tracing::error!("cannot start the scenario log's writer thread: {e}");
                failed.store(true, Ordering::Relaxed);
                (None, None, None, State::Failed)
            }
        };
        let mut log = Self {
            writer: LogWriter::new(Vec::new(), scenario).with_limit(log_limit),
            tx,
            shared,
            queue_limit,
            done,
            thread,
            entered_at: None,
            state,
            failed,
        };
        if log.state == State::Writing {
            let r = log.writer.header(scenario);
            log.check(r);
        }
        log
    }

    /// Whether the end line has been written (or writing failed): nothing more is logged.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Ended | State::Failed)
    }

    /// Whether lines are still taken (not ended, failed or stopping).
    fn writing(&mut self) -> bool {
        if self.state == State::Writing && self.failed.load(Ordering::Relaxed) {
            // The writer thread failed.
            self.close(State::Failed, None);
        }
        self.state == State::Writing
    }

    /// A formatting result: the size limit or a write error.
    fn check(&mut self, r: std::io::Result<()>) {
        if r.is_err() && self.writer.overflowed() {
            // The lines before the refused one stay; the next `after_poll` ends the log
            // with `error=LogOverflow`.
            if self.state == State::Writing {
                self.state = State::Stopping("LogOverflow");
            }
            return;
        }
        if let Err(e) = r {
            // Writing into memory does not fail; kept for completeness.
            tracing::error!("formatting the scenario log failed: {e}; the log stops here");
            self.failed.store(true, Ordering::Relaxed);
            self.close(State::Failed, None);
            return;
        }
        if self.writer.get_mut().len() >= CHUNK_BYTES {
            self.ship();
        }
    }

    /// Offers the formatted lines to the writer thread without blocking. A full channel
    /// leaves them with the frame loop for the next offer, unless the unwritten bytes then
    /// pass the queue limit: the log stops (`LogWrite`) and they are dropped.
    fn ship(&mut self) {
        if self.state != State::Writing || self.writer.get_mut().is_empty() {
            return;
        }
        let Some(tx) = &self.tx else {
            return;
        };
        let held = self.writer.get_mut().len();
        let queued = self.shared.queued.load(Ordering::Relaxed);
        if queued.saturating_add(held) > self.queue_limit {
            tracing::error!(
                "the scenario log's writer is {} bytes behind; the log stops here rather than \
                 stall the frame",
                queued.saturating_add(held)
            );
            self.writer.get_mut().clear();
            self.state = State::Stopping("LogWrite");
            return;
        }
        let chunk = std::mem::take(self.writer.get_mut());
        // Counted before the send, so the thread never subtracts more than was added.
        self.shared.queued.fetch_add(held, Ordering::Relaxed);
        match tx.try_send(chunk) {
            Ok(()) => {}
            Err(TrySendError::Full(chunk)) => {
                // Kept, in order, for the next offer.
                self.shared.queued.fetch_sub(held, Ordering::Relaxed);
                *self.writer.get_mut() = chunk;
            }
            Err(TrySendError::Disconnected(_)) => {
                // The thread stopped on a write error (it set `failed`).
                self.shared.queued.fetch_sub(held, Ordering::Relaxed);
                self.failed.store(true, Ordering::Relaxed);
                self.close(State::Failed, None);
            }
        }
    }

    /// Closes the channel; the thread writes what it has, then `tail`, and stops.
    fn close(&mut self, state: State, tail: Option<Vec<u8>>) {
        *self
            .shared
            .tail
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = tail;
        self.writer.get_mut().clear();
        self.tx = None;
        self.state = state;
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
        if !self.writing() {
            return;
        }
        let r = self.writer.command(self.time(now), command);
        self.check(r);
    }

    /// Logs an event received at `now`; `WorldEntered` starts the clock (time 0).
    pub fn event(&mut self, now: Instant, event: &Event) {
        if self.entered_at.is_none() && matches!(event, Event::WorldEntered { .. }) {
            self.entered_at = Some(now);
        }
        if !self.writing() {
            return;
        }
        let r = self.writer.event(self.time(now), event);
        self.check(r);
    }

    /// After a poll at `now`: offers the frame's lines to the writer thread, and writes the
    /// end line when the feed finished or failed, the session ended (`disconnected`)
    /// before it did, or the log fell behind or reached its size limit.
    /// `input_skipped` is the input lines the client could not send (§10.4).
    pub fn after_poll(
        &mut self,
        now: Instant,
        status: Option<ScenarioStatus>,
        counters: Option<ScenarioCounters>,
        disconnected: bool,
        input_skipped: u64,
    ) {
        if self.writing() {
            self.ship();
        }
        let stopped = match self.state {
            State::Stopping(kind) => Some(kind),
            State::Writing => None,
            State::Ended | State::Failed => return,
        };
        let (time, reason) = match (stopped, status) {
            (Some(kind), _) => (self.time(now), EndReason::Error { kind, line: 0 }),
            (None, Some(ScenarioStatus::Finished { time_ms, reason })) => (
                LogTime::At(time_ms),
                match reason {
                    FinishReason::Horizon => EndReason::Horizon,
                    _ => EndReason::Complete,
                },
            ),
            (None, Some(ScenarioStatus::Failed { time_ms, error })) => (
                LogTime::At(time_ms),
                EndReason::Error {
                    kind: runtime_kind(error.kind),
                    line: error.line,
                },
            ),
            (None, _) if disconnected => (
                self.time(now),
                EndReason::Error {
                    kind: "Disconnected",
                    line: 0,
                },
            ),
            (None, _) => return,
        };
        let c = counters.unwrap_or_default();
        let counters = EndCounters {
            skipped: c.skipped,
            input_skipped,
            dropped: c.dropped,
            spilled: c.spilled,
        };
        if stopped.is_some() {
            self.failed.store(true, Ordering::Relaxed);
        }
        // The lines the channel has not taken yet, then the end line, go beside the
        // channel: the thread writes them after the chunks already queued, so the end line
        // is written even when the channel is full.
        let _ = self.writer.end(time, reason, counters);
        let tail = std::mem::take(self.writer.get_mut());
        self.close(State::Ended, Some(tail));
    }

    /// Offers the lines formatted so far to the writer thread.
    pub fn flush(&mut self) {
        if self.writing() {
            self.ship();
        }
    }
}

impl Drop for LiveLog {
    fn drop(&mut self) {
        if matches!(self.state, State::Stopping(_)) {
            // No `after_poll` came to end it: end it now.
            self.after_poll(Instant::now(), None, None, true, 0);
        } else if self.writing() {
            // Not ended: the lines formatted so far go out, without an end line.
            let tail = std::mem::take(self.writer.get_mut());
            self.close(State::Ended, Some(tail));
        }
        self.tx = None;
        let Some(done) = self.done.take() else {
            return;
        };
        match done.recv_timeout(DRAIN_TIMEOUT) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                if let Some(t) = self.thread.take()
                    && t.join().is_err()
                {
                    self.failed.store(true, Ordering::Relaxed);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // The output does not take the bytes (a reader that stopped reading): leave
                // the thread, the log is incomplete.
                tracing::error!(
                    "the scenario log was not written within {DRAIN_TIMEOUT:?}; it is incomplete"
                );
                self.failed.store(true, Ordering::Relaxed);
            }
        }
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
            0,
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
            0,
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
        log.after_poll(Instant::now(), None, None, true, 3);
        assert!(log.is_closed());
        drop(log);
        let text = String::from_utf8(out.0.lock().expect("lock").clone()).expect("utf-8");
        assert!(
            text.lines()
                .last()
                .is_some_and(|l| l.starts_with("pre end reason=error")
                    && l.contains(" input-skipped=3 ")
                    && l.ends_with("error=Disconnected line=0")),
            "{text}"
        );
    }

    #[test]
    fn a_write_error_stops_the_log_and_is_reported() {
        let sc = scenario();
        let slot = LiveLogSlot::new(Box::new(Broken));
        let mut log = slot.take(&sc).expect("take");
        // The header reaches the writer thread with the first frame and fails there.
        log.after_poll(
            Instant::now(),
            Some(ScenarioStatus::Running { time_ms: 0 }),
            None,
            false,
            0,
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !slot.failed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(slot.failed());
        log.command(Instant::now(), &Command::Connect);
        assert!(log.is_closed(), "the log stops after a write error");
        drop(log);
        assert!(format!("{slot:?}").contains("failed: true"));
    }

    /// Commands and events of a short run, at wall-clock times.
    fn feed(log: &mut LiveLog, t0: Instant, frames: u64) {
        log.command(t0, &Command::Connect);
        log.event(
            t0,
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
        );
        for k in 1..=frames {
            let now = t0 + Duration::from_millis(k * 50);
            log.event(
                now,
                &Event::ChatMessage {
                    channel: tessera_net::ChatChannel::System,
                    sender: None,
                    text: format!("line {k}"),
                },
            );
            log.after_poll(
                now,
                Some(ScenarioStatus::Running { time_ms: k * 50 }),
                None,
                false,
                0,
            );
        }
    }

    #[test]
    fn the_thread_writes_the_bytes_the_log_writer_formats() {
        let sc = scenario();
        let out = Shared::default();
        let slot = LiveLogSlot::new(Box::new(out.clone()));
        let mut log = slot.take(&sc).expect("take");
        let t0 = Instant::now();
        feed(&mut log, t0, 2_000);
        log.after_poll(
            t0 + Duration::from_millis(100_050),
            Some(ScenarioStatus::Finished {
                time_ms: 100_000,
                reason: FinishReason::Complete,
            }),
            None,
            false,
            0,
        );
        drop(log);
        // The same lines through a plain `LogWriter`, as the headless driver writes them.
        let mut direct = LogWriter::new(Vec::new(), &sc);
        direct.header(&sc).expect("header");
        direct
            .command(LogTime::Pre, &Command::Connect)
            .expect("command");
        direct
            .event(
                LogTime::At(0),
                &Event::WorldEntered {
                    player: EntityId(1),
                    name: "Tamsin".into(),
                    transform: Transform::new(Vec3::ZERO, 0.0),
                },
            )
            .expect("event");
        for k in 1..=2_000u64 {
            direct
                .event(
                    LogTime::At(k * 50),
                    &Event::ChatMessage {
                        channel: tessera_net::ChatChannel::System,
                        sender: None,
                        text: format!("line {k}"),
                    },
                )
                .expect("event");
        }
        direct
            .end(
                LogTime::At(100_000),
                EndReason::Complete,
                EndCounters::default(),
            )
            .expect("end");
        let written = out.0.lock().expect("lock").clone();
        assert_eq!(
            String::from_utf8(written).expect("utf-8"),
            String::from_utf8(direct.into_inner()).expect("utf-8")
        );
        assert!(!slot.failed());
    }

    /// A writer that blocks until its gate opens (a reader that stopped reading).
    #[derive(Clone)]
    struct Gated {
        open: Arc<(Mutex<bool>, std::sync::Condvar)>,
        out: Shared,
    }

    impl Write for Gated {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let (lock, cv) = &*self.open;
            let mut open = lock.lock().expect("lock");
            while !*open {
                open = cv.wait(open).expect("wait");
            }
            self.out.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn gate() -> Gated {
        Gated {
            open: Arc::new((Mutex::new(false), std::sync::Condvar::new())),
            out: Shared::default(),
        }
    }

    fn open(gated: &Gated) {
        let (lock, cv) = &*gated.open;
        *lock.lock().expect("lock") = true;
        cv.notify_all();
    }

    #[test]
    fn a_stalled_output_never_blocks_the_frame() {
        let sc = scenario();
        let gated = gate();
        let failed = Arc::new(AtomicBool::new(false));
        let mut log = LiveLog::with_limits(
            Box::new(gated.clone()),
            &sc,
            Arc::clone(&failed),
            tessera_net::scenario::MAX_LOG_BYTES,
            4_096,
        );
        let t0 = Instant::now();
        // The thread blocks on the first chunk; once 4,096 bytes are unwritten the log
        // gives up instead of blocking the frame.
        let frames = 400;
        feed(&mut log, t0, frames);
        assert!(log.is_closed(), "the log ended when the writer fell behind");
        assert!(failed.load(Ordering::Relaxed), "the run exits 5");
        open(&gated);
        drop(log);
        let text = String::from_utf8(gated.out.0.lock().expect("lock").clone()).expect("utf-8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.first().copied(), Some("tessera-scenario-log 1"));
        let last = lines.last().copied().unwrap_or_default();
        assert!(
            last.contains(" end reason=error ") && last.ends_with("error=LogWrite line=0"),
            "{last}"
        );
        // Every line before the end line is a whole line of the run, in order.
        let chats: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|l| l.contains(" ChatMessage "))
            .collect();
        assert!(!chats.is_empty() && chats.len() < usize::try_from(frames).expect("fits"));
        for (i, l) in chats.iter().enumerate() {
            assert!(l.ends_with(&format!("text=\"line {}\"", i + 1)), "{l}");
        }
    }

    /// A full channel keeps the frame's lines for the next offer: an output that stalls
    /// for more frames than the channel holds, but within the byte limit, loses nothing.
    #[test]
    fn a_full_channel_keeps_the_lines_for_later() {
        let sc = scenario();
        let gated = gate();
        let failed = Arc::new(AtomicBool::new(false));
        let mut log = LiveLog::new(Box::new(gated.clone()), &sc, Arc::clone(&failed));
        let t0 = Instant::now();
        let frames = u64::try_from(MAX_QUEUED_CHUNKS).expect("fits") * 2;
        feed(&mut log, t0, frames);
        assert!(!log.is_closed(), "still writing");
        open(&gated);
        let end = t0 + Duration::from_millis((frames + 1) * 50);
        log.after_poll(
            end,
            Some(ScenarioStatus::Finished {
                time_ms: frames * 50,
                reason: FinishReason::Complete,
            }),
            None,
            false,
            0,
        );
        drop(log);
        assert!(!failed.load(Ordering::Relaxed));
        let text = String::from_utf8(gated.out.0.lock().expect("lock").clone()).expect("utf-8");
        let chats: Vec<&str> = text
            .lines()
            .filter(|l| l.contains(" ChatMessage "))
            .collect();
        assert_eq!(chats.len(), usize::try_from(frames).expect("fits"));
        for (i, l) in chats.iter().enumerate() {
            assert!(l.ends_with(&format!("text=\"line {}\"", i + 1)), "{l}");
        }
        assert!(
            text.lines().last().is_some_and(|l| l
                .ends_with(" end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0")),
            "{text}"
        );
    }

    #[test]
    fn a_log_at_its_size_limit_ends_with_log_overflow() {
        let sc = scenario();
        let out = Shared::default();
        let failed = Arc::new(AtomicBool::new(false));
        let mut log = LiveLog::with_limits(
            Box::new(out.clone()),
            &sc,
            Arc::clone(&failed),
            2_000,
            MAX_QUEUED_BYTES,
        );
        feed(&mut log, Instant::now(), 200);
        assert!(log.is_closed());
        drop(log);
        assert!(failed.load(Ordering::Relaxed), "the run exits 5");
        let text = String::from_utf8(out.0.lock().expect("lock").clone()).expect("utf-8");
        let (body, last) = text
            .trim_end_matches('\n')
            .rsplit_once('\n')
            .expect("lines");
        assert!(body.len() < 2_000, "{}", body.len());
        assert!(last.ends_with("error=LogOverflow line=0"), "{last}");
    }
}
