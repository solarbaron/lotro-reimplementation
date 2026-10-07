// SPDX-License-Identifier: GPL-3.0-or-later
//! A test-server backend without a network, for fuzzing and deterministic tests.
//!
//! [`InboundHarness`] runs the same network-side driver (the protocol's client state
//! machine, login, the movement schedule) and the same main-thread mapper as
//! [`RemoteBackend`](super::RemoteBackend), but takes the server's bytes from a script of
//! [`HarnessInput`]s instead of a QUIC connection, and runs on a virtual clock. It
//! implements [`Backend`], so it can be wrapped in a [`Session`](crate::Session) and every
//! event it produces goes through the same checks a real one would.
//!
//! ```
//! use std::time::Instant;
//! use tessera_net::{Command, Event, Session};
//! use tessera_net::remote::{HarnessInput, InboundHarness};
//!
//! // A server that answers with garbage: the session ends cleanly.
//! let mut session = Session::new(InboundHarness::new([HarnessInput::Control(vec![1, 0, 0, 0, 1])]));
//! session.send(Instant::now(), Command::Connect)?;
//! let events = session.poll(Instant::now());
//! assert!(matches!(events.last(), Some(Event::Disconnected { .. })));
//! # Ok::<(), tessera_net::NetError>(())
//! ```

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tessera_protocol::client::ClientConfig;
use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::frame::StreamKind;
use tessera_protocol::message::{Credentials, Message};
use tessera_protocol::session::{ConnectionEnd, PeerDatagrams, StreamEnd, StreamRef, Transmit};

use super::driver::{Driver, MoveSlot, TICK};
use super::mapper::Mapper;
use super::{Routed, route};
use crate::NetError;
use crate::interp::InterpConfig;
use crate::sandbox::world::{StaticConfig, WorldSource};
use crate::session::{Backend, Command, Event};

/// Most client messages [`InboundHarness::sent`] remembers (the oldest are forgotten).
pub const MAX_RECORDED_SENDS: usize = 1024;
/// Longest single [`HarnessInput::Advance`] honoured; longer ones are cut to this.
pub const MAX_ADVANCE: Duration = Duration::from_secs(120);

/// One step of the scripted server, applied at one [`Backend::poll`].
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessInput {
    /// Bytes arriving on the control stream.
    Control(Vec<u8>),
    /// The server opens a unidirectional stream whose first byte is this kind byte
    /// (0x57 world, 0x43 chat).
    OpenStream(u8),
    /// Bytes arriving on the world (`world` true) or chat stream, after its kind byte.
    Stream {
        /// World stream if true, else chat.
        world: bool,
        /// The bytes.
        data: Vec<u8>,
    },
    /// The server finishes (`reset` false) or resets a stream: `None` is the control
    /// stream, `Some(world)` a uni stream.
    StreamEnd {
        /// Which stream.
        stream: Option<bool>,
        /// RESET_STREAM instead of FIN.
        reset: bool,
    },
    /// One datagram.
    Datagram(Vec<u8>),
    /// The virtual clock moves on (at most [`MAX_ADVANCE`]), running one 30 Hz schedule tick
    /// at every whole tick boundary it crosses; a remainder carries over to the next
    /// advance, so short advances never run the schedule faster than real time.
    Advance(Duration),
    /// The server closes the connection with an application code and reason.
    Close {
        /// The code.
        code: u16,
        /// The reason phrase.
        reason: String,
    },
    /// The QUIC idle timeout fires.
    IdleTimeout,
}

/// A test-server backend without a network; see the [`remote`](super) module and the
/// harness description there.
#[derive(Debug)]
pub struct InboundHarness {
    driver: Driver,
    mapper: Mapper,
    inputs: VecDeque<HarnessInput>,
    now: u64,
    /// Virtual time of the next 30 Hz schedule tick.
    next_tick: u64,
    started: bool,
    latest_move: Option<MoveSlot>,
    sent: VecDeque<Message>,
    closed_sent: bool,
}

impl InboundHarness {
    /// A harness that logs in as `fuzz` and then plays `inputs`, one per poll.
    pub fn new(inputs: impl IntoIterator<Item = HarnessInput>) -> Self {
        Self::build(inputs, None)
    }

    /// As [`new`](Self::new), streaming statics from `source` like the real backend.
    pub fn with_world(
        inputs: impl IntoIterator<Item = HarnessInput>,
        source: impl WorldSource + 'static,
    ) -> Self {
        Self::build(inputs, Some(Box::new(source)))
    }

    fn build(
        inputs: impl IntoIterator<Item = HarnessInput>,
        source: Option<Box<dyn WorldSource>>,
    ) -> Self {
        Self {
            driver: Driver::new(
                ClientConfig::default(),
                Credentials {
                    account: "fuzz".into(),
                    password: "fuzz".into(),
                },
                false,
            ),
            mapper: Mapper::new(StaticConfig::default(), InterpConfig::default(), source),
            inputs: inputs.into_iter().collect(),
            now: 1_000_000,
            next_tick: 1_000_000 + u64::try_from(TICK.as_micros()).unwrap_or(33_333),
            started: false,
            latest_move: None,
            sent: VecDeque::new(),
            closed_sent: false,
        }
    }

    /// Inputs not yet played.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.inputs.len()
    }

    /// Queues more inputs.
    pub fn push(&mut self, input: HarnessInput) {
        self.inputs.push_back(input);
    }

    /// The messages the client sent, oldest first (at most [`MAX_RECORDED_SENDS`]).
    pub fn sent(&self) -> impl Iterator<Item = &Message> {
        self.sent.iter()
    }

    /// Whether the client closed the connection (a `Close` transmit was produced).
    #[must_use]
    pub fn closed_connection(&self) -> bool {
        self.closed_sent
    }

    /// The virtual client clock, µs.
    #[must_use]
    pub fn now(&self) -> u64 {
        self.now
    }

    fn collect_transmits(&mut self) {
        while let Some(t) = self.driver.poll_transmit() {
            match t {
                Transmit::Message(m) => {
                    if self.sent.len() >= MAX_RECORDED_SENDS {
                        self.sent.pop_front();
                    }
                    self.sent.push_back(m);
                }
                Transmit::Close { .. } => {
                    self.closed_sent = true;
                    self.driver.connection_closed(ConnectionEnd::Local);
                }
                Transmit::FinishControl | Transmit::OpenStream(_) => {}
            }
        }
    }

    fn apply(&mut self, input: HarnessInput) {
        let now = self.now;
        match input {
            HarnessInput::Control(bytes) => self.driver.on_control(&bytes, now),
            HarnessInput::OpenStream(b) => {
                let _ = self.driver.on_uni_stream(b);
            }
            HarnessInput::Stream { world, data } => {
                let kind = if world {
                    StreamKind::World
                } else {
                    StreamKind::Chat
                };
                self.driver.on_stream(kind, &data, now);
            }
            HarnessInput::StreamEnd { stream, reset } => {
                let s = match stream {
                    None => StreamRef::Control,
                    Some(true) => StreamRef::Uni(StreamKind::World),
                    Some(false) => StreamRef::Uni(StreamKind::Chat),
                };
                let end = if reset {
                    StreamEnd::Reset
                } else {
                    StreamEnd::Finished
                };
                self.driver.on_stream_end(s, end);
            }
            HarnessInput::Datagram(bytes) => self.driver.on_datagram(&bytes, now),
            HarnessInput::Advance(d) => {
                let end = self
                    .now
                    .saturating_add(u64::try_from(d.min(MAX_ADVANCE).as_micros()).unwrap_or(0));
                let step = u64::try_from(TICK.as_micros()).unwrap_or(33_333);
                // Tick only on whole tick boundaries, as the real link's 30 Hz interval
                // does: an Advance that is not a multiple of the tick carries its remainder
                // into the next one instead of running an extra schedule tick.
                while self.next_tick <= end {
                    self.now = self.next_tick;
                    self.next_tick = self.next_tick.saturating_add(step);
                    self.driver.tick(self.now, self.latest_move);
                    self.collect_transmits();
                }
                self.now = end;
            }
            HarnessInput::Close { code, reason } => {
                self.driver.connection_closed(ConnectionEnd::PeerClosed {
                    code: AppErrorCode::from_u16(code),
                    reason,
                });
            }
            HarnessInput::IdleTimeout => self.driver.connection_closed(ConnectionEnd::IdleTimeout),
        }
        self.collect_transmits();
    }
}

impl Backend for InboundHarness {
    fn name(&self) -> &str {
        "harness"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        match route(command, &mut self.mapper)? {
            Routed::Connect => {
                if self.started {
                    return Err(NetError::InvalidCommand("already connected"));
                }
                self.started = true;
                self.driver.connect(self.now);
                self.driver
                    .handshake_complete(self.now, PeerDatagrams::MaxPayload(1200));
            }
            Routed::Net(cmd) => self.driver.command(cmd, self.now),
            Routed::Move(slot) => self.latest_move = Some(slot),
            Routed::Disconnect => self.driver.disconnect(self.now),
        }
        self.collect_transmits();
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if self.started
            && let Some(input) = self.inputs.pop_front()
        {
            self.apply(input);
        }
        while let Some(item) = self.driver.pending.pop_front() {
            self.mapper.map(item, out);
        }
        self.mapper.update_statics(out);
    }
}
