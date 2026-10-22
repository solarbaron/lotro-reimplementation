// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Session` state machine over the sandbox backend (`tessera-net` `session`,
//! `sandbox`), driven by an arbitrary sandbox configuration and an arbitrary sequence of
//! commands (including non-finite movement, oversized or control-character text and
//! invalid names), clock steps, including stale clocks, and polls.
//!
//! Nothing may panic. Commands must be validated before they reach the backend, events
//! must respect the session order (entity events only while in the world, nothing after
//! `Disconnected`), and the pending-event queue must stay bounded.
//!
//! The sandbox tolerates invalid commands, so it is wrapped in [`Checked`], a backend that
//! asserts every command it is handed satisfies the session's validation and state rules.
//! The pending queue is bounded by what the operations inside the latency window can have
//! produced: an event emitted at time *t* is due at *t* + latency, and every poll that
//! reaches the backend removes all events that are due.

#![no_main]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::model::{MAX_CHAT_LEN, is_valid_name};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Backend, ChatChannel, Command, Destination, Event, Location, NetError, RegionId, Session,
    SessionState, Transform, Vec3,
};

const MAX_OPS: usize = 256;
/// Events one operation adds to the sandbox's queue beyond one per wanderer and crowd
/// member (world entry, location reports, corrections, system notices).
const EVENTS_PER_OP: usize = 16;

/// The sandbox behind a check of everything the session forwards to it.
#[derive(Debug)]
struct Checked {
    inner: SandboxBackend,
    connected: bool,
    entering: bool,
    /// The backend emitted `WorldEntered`, so in-world commands are allowed.
    in_world: bool,
    disconnecting: bool,
}

impl Checked {
    fn new(inner: SandboxBackend) -> Self {
        Self {
            inner,
            connected: false,
            entering: false,
            in_world: false,
            disconnecting: false,
        }
    }
}

/// The session's argument rules, restated independently of `Session::send`.
fn assert_valid(command: &Command) {
    match command {
        Command::EnterWorld { character } => {
            assert!(is_valid_name(character), "invalid name reached backend")
        }
        Command::Move {
            location,
            heading,
            velocity,
            ..
        } => assert!(
            location.is_valid() && heading.is_finite() && velocity.is_finite(),
            "invalid move reached backend: {command:?}"
        ),
        Command::Chat { channel, text } => {
            assert!(
                !text.is_empty() && text.len() <= MAX_CHAT_LEN,
                "chat length out of range reached backend"
            );
            assert!(
                !text.chars().any(char::is_control),
                "chat control characters reached backend"
            );
            match channel {
                ChatChannel::System => panic!("system-channel chat reached backend"),
                ChatChannel::Tell { peer } => {
                    assert!(is_valid_name(peer), "invalid tell peer reached backend")
                }
                _ => {}
            }
        }
        Command::Teleport { destination } => assert!(
            destination.is_valid(),
            "invalid teleport reached backend: {destination:?}"
        ),
        Command::Connect | Command::Disconnect | Command::QueryLocation => {}
        _ => panic!("unchecked command kind; extend assert_valid: {command:?}"),
    }
}

impl Backend for Checked {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        assert_valid(&command);
        assert!(!self.disconnecting, "command after Disconnect: {command:?}");
        match &command {
            Command::Connect => assert!(!self.connected, "second Connect"),
            Command::EnterWorld { .. } => {
                assert!(self.connected && !self.entering, "EnterWorld out of state");
            }
            Command::Disconnect => assert!(self.connected, "Disconnect before Connect"),
            Command::Move { .. }
            | Command::Chat { .. }
            | Command::Teleport { .. }
            | Command::QueryLocation => {
                assert!(self.in_world, "in-world command out of state: {command:?}");
            }
            _ => panic!("unchecked command kind; extend submit: {command:?}"),
        }
        let latch = match &command {
            Command::Connect => Some(&mut self.connected),
            Command::EnterWorld { .. } => Some(&mut self.entering),
            Command::Disconnect => Some(&mut self.disconnecting),
            _ => None,
        };
        // The session changes state only when the backend accepts the command; a refused
        // one (e.g. `Closed` after an unknown character) may be sent again.
        let accepted = self.inner.submit(now, command);
        if accepted.is_ok()
            && let Some(flag) = latch
        {
            *flag = true;
        }
        accepted
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        let start = out.len();
        self.inner.poll(now, out);
        if out[start..]
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
        {
            self.in_world = true;
        }
    }
}

#[derive(Debug, Arbitrary)]
struct Config {
    characters: Vec<String>,
    spawn: [f32; 4],
    wanderers: u16,
    wander_radius: f32,
    wander_speed: f32,
    tick_ms: u16,
    latency_ms: u16,
    echo_chat: bool,
    max_player_speed: f32,
}

#[derive(Debug, Arbitrary)]
enum Chan {
    Say,
    Yell,
    Emote,
    Tell(String),
    System,
}

#[derive(Debug, Arbitrary)]
enum Op {
    Connect,
    EnterWorld(String),
    Move {
        pos: [f64; 3],
        region: u16,
        heading: f32,
        vel: [f32; 3],
        on_ground: bool,
    },
    Chat {
        chan: Chan,
        text: String,
    },
    Disconnect,
    QueryLocation,
    Teleport {
        pos: [f64; 3],
        heading: f32,
        region: u16,
    },
    TeleportNamed(String),
    Poll {
        advance_ms: u16,
    },
    /// Poll with a clock that went backwards (must be clamped).
    StalePoll {
        back_ms: u16,
    },
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

fuzz_target!(|input: (Config, Vec<Op>)| {
    static T0: OnceLock<Instant> = OnceLock::new();
    // Leave room before T0 so stale clocks never underflow `Instant`.
    let t0 = *T0.get_or_init(|| Instant::now() + Duration::from_secs(3600));
    let (cfg, ops) = input;
    let config = SandboxConfig {
        characters: cfg.characters.into_iter().take(8).collect(),
        spawn: Transform::new(v3([cfg.spawn[0], cfg.spawn[1], cfg.spawn[2]]), cfg.spawn[3]),
        // Keep per-tick work small; the cap itself is checked by `validate`.
        wanderers: usize::from(cfg.wanderers % 64),
        wander_radius: cfg.wander_radius,
        wander_speed: cfg.wander_speed,
        tick_interval: Duration::from_millis(u64::from(cfg.tick_ms)),
        latency: Duration::from_millis(u64::from(cfg.latency_ms)),
        echo_chat: cfg.echo_chat,
        max_player_speed: cfg.max_player_speed,
        ..SandboxConfig::default()
    };
    let per_op = config.wanderers + config.crowd.count + EVENTS_PER_OP;
    let latency = config.latency;
    let Ok(backend) = SandboxBackend::new(config) else {
        return;
    };
    let mut session = Session::new(Checked::new(backend));
    let mut now = t0;
    let mut events = Vec::new();
    let mut disconnected = false;
    let mut entered = false;
    // When each operation ran, and the latest poll that reached the backend.
    let mut op_times: Vec<Instant> = Vec::new();
    let mut last_poll: Option<(usize, Instant)> = None;

    for op in ops.into_iter().take(MAX_OPS) {
        let index = op_times.len();
        // `Session` does not poll the backend once closed.
        let polls_backend = session.state() != SessionState::Closed;
        let command = match op {
            Op::Connect => Some(Command::Connect),
            Op::EnterWorld(character) => Some(Command::EnterWorld { character }),
            Op::Move {
                pos,
                region,
                heading,
                vel,
                on_ground,
            } => Some(Command::Move {
                location: Location::new(RegionId(region % 4), pos[0], pos[1], pos[2]),
                heading,
                velocity: v3(vel),
                on_ground,
            }),
            Op::Chat { chan, text } => Some(Command::Chat {
                channel: match chan {
                    Chan::Say => ChatChannel::Say,
                    Chan::Yell => ChatChannel::Yell,
                    Chan::Emote => ChatChannel::Emote,
                    Chan::Tell(peer) => ChatChannel::Tell { peer },
                    Chan::System => ChatChannel::System,
                },
                text,
            }),
            Op::Disconnect => Some(Command::Disconnect),
            Op::QueryLocation => Some(Command::QueryLocation),
            Op::Teleport {
                pos,
                heading,
                region,
            } => Some(Command::Teleport {
                destination: Destination::Location {
                    location: Location::new(RegionId(region), pos[0], pos[1], pos[2]),
                    heading,
                },
            }),
            Op::TeleportNamed(name) => Some(Command::Teleport {
                destination: Destination::Named(name),
            }),
            Op::Poll { advance_ms } => {
                now += Duration::from_millis(u64::from(advance_ms));
                session.poll_into(now, &mut events);
                if polls_backend {
                    last_poll = Some((index, now));
                }
                None
            }
            Op::StalePoll { back_ms } => {
                let stale = now - Duration::from_millis(u64::from(back_ms));
                session.poll_into(stale, &mut events);
                if polls_backend {
                    // The session clamps the stale clock to `now`.
                    last_poll = Some((index, now));
                }
                None
            }
        };
        if let Some(c) = command {
            let _ = session.send(now, c);
        }
        // The time the backend stamped this operation's events with (a poll's own events
        // carry the advanced clock).
        op_times.push(now);
        for e in events.drain(..) {
            assert!(!disconnected, "event after Disconnected: {e:?}");
            match e {
                Event::WorldEntered { .. } => entered = true,
                Event::EntitySpawned { .. }
                | Event::EntityCreated { .. }
                | Event::AppearanceChanged { .. }
                | Event::StaticSpawned { .. }
                | Event::StaticsCleared
                | Event::LocationReport { .. }
                | Event::EntityMoved { .. }
                | Event::EntityDespawned { .. } => {
                    assert!(entered, "entity event before WorldEntered: {e:?}");
                }
                Event::Disconnected { .. } => disconnected = true,
                _ => {}
            }
        }
        if session.state() == SessionState::InWorld {
            assert!(session.player().is_some(), "a player exists in the world");
        }
        let _ = session.backend_name();

        // Events still queued were emitted by operations whose events are not due at the
        // last backend poll, or that ran after it.
        let checked = session
            .backend::<Checked>()
            .expect("the wrapper exposes itself");
        let live_ops = op_times
            .iter()
            .enumerate()
            .filter(|&(i, &t)| match last_poll {
                None => true,
                Some((p, at)) => i > p || t + latency > at,
            })
            .count();
        let queued = checked.inner.queued_events();
        assert!(
            queued <= live_ops * per_op,
            "pending queue {queued} exceeds {live_ops} ops x {per_op} events"
        );
    }
});
