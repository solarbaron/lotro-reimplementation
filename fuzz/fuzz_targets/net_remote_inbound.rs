// SPDX-License-Identifier: GPL-3.0-or-later
//! The test-server backend's inbound handling (`tessera_net::remote`), without a network:
//! arbitrary control, world and chat stream bytes, stream opens and ends, datagrams,
//! clock steps and client commands, fed through the same driver and mapper as the real
//! backend and through `Session`'s checks (`tessera-protocol.md` §13). Must never panic;
//! at most one `Disconnected`, and nothing after it.

#![no_main]

use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::remote::{HarnessInput, InboundHarness};
use tessera_net::{ChatChannel, Command, Event, Location, RegionId, Session, Vec3};

const MAX_OPS: usize = 256;

#[derive(Debug, Arbitrary)]
enum Op {
    Control(Vec<u8>),
    Open(u8),
    Stream {
        world: bool,
        data: Vec<u8>,
    },
    End {
        stream: Option<bool>,
        reset: bool,
    },
    Datagram(Vec<u8>),
    Advance(u16),
    Close {
        code: u16,
        reason: String,
    },
    Idle,
    EnterWorld(String),
    Move {
        x: f64,
        y: f64,
        z: f64,
        heading: f32,
        vx: f32,
        vy: f32,
    },
    Chat(String),
    Query,
    Disconnect,
}

fuzz_target!(|ops: Vec<Op>| {
    let mut inputs = Vec::new();
    let mut commands = Vec::new();
    for (i, op) in ops.into_iter().take(MAX_OPS).enumerate() {
        let input = match op {
            Op::Control(b) => HarnessInput::Control(b),
            Op::Open(k) => HarnessInput::OpenStream(k),
            Op::Stream { world, data } => HarnessInput::Stream { world, data },
            Op::End { stream, reset } => HarnessInput::StreamEnd { stream, reset },
            Op::Datagram(b) => HarnessInput::Datagram(b),
            Op::Advance(ms) => HarnessInput::Advance(Duration::from_millis(u64::from(ms))),
            Op::Close { code, reason } => HarnessInput::Close { code, reason },
            Op::Idle => HarnessInput::IdleTimeout,
            Op::EnterWorld(name) => {
                commands.push((i, Command::EnterWorld { character: name }));
                continue;
            }
            Op::Move {
                x,
                y,
                z,
                heading,
                vx,
                vy,
            } => {
                commands.push((
                    i,
                    Command::Move {
                        location: Location::new(RegionId(1), x, y, z),
                        heading,
                        velocity: Vec3::new(vx, vy, 0.0),
                    },
                ));
                continue;
            }
            Op::Chat(text) => {
                commands.push((
                    i,
                    Command::Chat {
                        channel: ChatChannel::Say,
                        text,
                    },
                ));
                continue;
            }
            Op::Query => {
                commands.push((i, Command::QueryLocation));
                continue;
            }
            Op::Disconnect => {
                commands.push((i, Command::Disconnect));
                continue;
            }
        };
        inputs.push(input);
    }
    let polls = inputs.len() + 2;
    let mut session = Session::new(InboundHarness::new(inputs));
    let now = Instant::now();
    let _ = session.send(now, Command::Connect);
    let mut events: Vec<Event> = Vec::new();
    let mut cmds = commands.into_iter().peekable();
    for step in 0..polls + MAX_OPS {
        while let Some((_, c)) = cmds.next_if(|(i, _)| *i <= step) {
            // Session refuses what the state does not allow; that is fine here.
            let _ = session.send(now, c);
        }
        session.poll_into(now, &mut events);
        if step >= polls && cmds.peek().is_none() {
            break;
        }
    }
    let ends = events
        .iter()
        .filter(|e| matches!(e, Event::Disconnected { .. }))
        .count();
    assert!(ends <= 1);
    if ends == 1 {
        assert!(matches!(events.last(), Some(Event::Disconnected { .. })));
    }
});
