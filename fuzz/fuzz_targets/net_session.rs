// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Session` state machine over the sandbox backend (`tessera-net` `session`,
//! `sandbox`), driven by an arbitrary sandbox configuration and an arbitrary sequence of
//! commands (including non-finite movement, oversized or control-character text and
//! invalid names), clock steps, including stale clocks, and polls.
//!
//! Nothing may panic. Commands must be validated before they reach the backend, events
//! must respect the session order (entity events only while in the world, nothing after
//! `Disconnected`), and the pending-event queue must stay bounded.

#![no_main]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    ChatChannel, Command, Destination, Event, Location, RegionId, Session, SessionState, Transform,
    Vec3,
};

const MAX_OPS: usize = 256;

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
    let Ok(backend) = SandboxBackend::new(config) else {
        return;
    };
    let mut session = Session::new(backend);
    let mut now = t0;
    let mut events = Vec::new();
    let mut disconnected = false;
    let mut entered = false;

    for op in ops.into_iter().take(MAX_OPS) {
        let command = match op {
            Op::Connect => Some(Command::Connect),
            Op::EnterWorld(character) => Some(Command::EnterWorld { character }),
            Op::Move {
                pos,
                region,
                heading,
                vel,
            } => Some(Command::Move {
                location: Location::new(RegionId(region % 4), pos[0], pos[1], pos[2]),
                heading,
                velocity: v3(vel),
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
                None
            }
            Op::StalePoll { back_ms } => {
                let stale = now - Duration::from_millis(u64::from(back_ms));
                session.poll_into(stale, &mut events);
                None
            }
        };
        if let Some(c) = command {
            let _ = session.send(now, c);
        }
        for e in events.drain(..) {
            assert!(!disconnected, "event after Disconnected: {e:?}");
            match e {
                Event::WorldEntered { .. } => entered = true,
                Event::EntitySpawned { .. }
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
    }
});
