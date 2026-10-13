// SPDX-License-Identifier: GPL-3.0-or-later
//! tessera-server's per-connection handling (`ConnectionCore`) at protocol minor 2 with
//! `CHAT_CHANNELS` (`test-server.md` §9.1, `tessera-protocol.md` §10.6): a connection
//! greeted, logged in and in the world, then fed arbitrary control bytes (channel requests
//! and named lines among them) and the simulation's answers in any order, including channel
//! answers with no request outstanding. It must never panic, report its end at most once,
//! send nothing after closing, keep its held-answer queue bounded, and never send a channel
//! message on a connection below minor 2.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::Message;
use tessera_protocol::codes::{AppErrorCode, ChannelRefusedCode, ChatRejectedCode};
use tessera_protocol::frame::frame_to_vec_minor;
use tessera_protocol::message::{
    ChatChannel, Credentials, EnterWorld, Hello, ServerCaps, WorldEnter,
};
use tessera_protocol::server::ServerParams;
use tessera_protocol::session::{ConnectionEnd, PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body, Location, Look, Pose};
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::shared::SessionCmd;

fn count(ended: &mut usize, inbound: Vec<Inbound>) {
    for i in inbound {
        if matches!(i, Inbound::Ended { .. }) {
            *ended += 1;
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&flags, mut rest)) = data.split_first() else {
        return;
    };
    let minor = u16::from(flags % 3);
    let caps = if flags & 4 == 0 {
        ServerCaps::CHAT_CHANNELS
    } else {
        ServerCaps::NONE
    };
    let mut core = ConnectionCore::new(
        ServerParams {
            caps,
            ..ServerParams::default()
        },
        0,
    );
    let mut ended = 0usize;
    let mut closed = false;
    count(
        &mut ended,
        core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162)),
    );
    let frame = |m: &Message, at: u16| frame_to_vec_minor(m, at).unwrap_or_default();
    count(
        &mut ended,
        core.on_control_data(
            &frame(
                &Message::Hello(Hello {
                    minor_max: minor,
                    client_caps: 0,
                    max_entities: 1024,
                    client_name: String::new(),
                }),
                0,
            ),
            1,
        ),
    );
    count(
        &mut ended,
        core.on_control_data(
            &frame(
                &Message::Login(Credentials {
                    account: "a".into(),
                    password: "p".into(),
                }),
                minor,
            ),
            2,
        ),
    );
    count(
        &mut ended,
        core.apply(
            SessionCmd::AcceptLogin {
                account: "a".into(),
                characters: Vec::new(),
                looks: Vec::new(),
            },
            3,
        ),
    );
    count(
        &mut ended,
        core.on_control_data(
            &frame(&Message::EnterWorld(EnterWorld { name: "N".into() }), minor),
            4,
        ),
    );
    count(
        &mut ended,
        core.apply(
            SessionCmd::EnterWorld(Box::new(WorldEnter {
                player: 1,
                name: "N".into(),
                instance: 1,
                epoch: 1,
                server_time: 5,
                pose: Pose {
                    location: Location::new(1, 0.0, 0.0, 0.0),
                    heading: 0.0,
                    anchor: None,
                },
                appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                look: Look::none(),
            })),
            5,
        ),
    );
    let mut now = 10u64;
    while let Some((&op, tail)) = rest.split_first() {
        let len = usize::from(tail.first().copied().unwrap_or(0)).min(tail.len().saturating_sub(1));
        let chunk = tail.get(1..1 + len).unwrap_or_default();
        rest = tail.get(1 + len..).unwrap_or_default();
        now += u64::from(op) * 104_729;
        let b = chunk.first().copied().unwrap_or(0);
        let name = || String::from_utf8_lossy(chunk).into_owned();
        let inbound = match op % 8 {
            0..=2 => core.on_control_data(chunk, now),
            3 => {
                // A well-formed channel request with the chunk as its name.
                let m = if b & 1 == 0 {
                    Message::ChannelJoin { name: name() }
                } else {
                    Message::ChannelLeave { name: name() }
                };
                core.on_control_data(&frame(&m, 2), now)
            }
            4 | 5 => {
                let cmd = match b % 6 {
                    0 => SessionCmd::ChannelJoined(name()),
                    1 => SessionCmd::ChannelLeft(name()),
                    2 => SessionCmd::RefuseChannel {
                        code: ChannelRefusedCode::from_u16(u16::from(b % 11)),
                        name: name(),
                    },
                    3 => SessionCmd::RejectChat(ChatRejectedCode::from_u16(u16::from(b % 8))),
                    4 => SessionCmd::ChatDelivered,
                    _ => SessionCmd::RefuseChannel {
                        code: ChannelRefusedCode::RateLimited,
                        name: name(),
                    },
                };
                core.apply(cmd, now)
            }
            6 => core.handle_timeout(now),
            _ => core.connection_closed(ConnectionEnd::PeerClosed {
                code: AppErrorCode::from_u16(u16::from(b)),
                reason: String::new(),
            }),
        };
        count(&mut ended, inbound);
        let mut n = 0;
        while let Some(t) = core.poll_transmit() {
            assert!(!closed, "nothing is sent after Close");
            match &t {
                Transmit::Close { .. } => closed = true,
                Transmit::Message(m) if core.minor() < 2 => {
                    let named = matches!(
                        m,
                        Message::ChatMessage(c) if matches!(c.channel, ChatChannel::Named { .. })
                    );
                    assert!(
                        m.message_type().min_minor() < 2 && !named,
                        "{m:?} below minor 2"
                    );
                }
                _ => {}
            }
            n += 1;
            assert!(n < 10_000, "unbounded outbox");
        }
        assert!(ended <= 1, "the end is reported once");
        assert!(core.unanswered_chats() <= 100_000, "unbounded answer order");
    }
});
