// SPDX-License-Identifier: GPL-3.0-or-later
//! tessera-server's per-connection inbound handling (`ConnectionCore`: decoding, the
//! protocol state machine and the SERVER_FULL admission rule; test-server.md §10.1) fed
//! arbitrary control bytes, datagrams, stream ends, timeouts and simulation requests,
//! without sockets. It must never panic, report its end at most once, and send nothing
//! after closing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode,
};
use tessera_protocol::message::{ServerCaps, WorldEnter};
use tessera_protocol::server::ServerParams;
use tessera_protocol::session::{ConnectionEnd, PeerDatagrams, StreamEnd, StreamRef, Transmit};
use tessera_protocol::types::{Appearance, Body, Location, Pose};
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::shared::SessionCmd;

fuzz_target!(|data: &[u8]| {
    let Some((&flags, mut rest)) = data.split_first() else {
        return;
    };
    let params = ServerParams {
        caps: ServerCaps(u32::from(flags & 7)),
        ..ServerParams::default()
    };
    let mut core = ConnectionCore::new(params, 0);
    if flags & 8 != 0 {
        core.refuse();
    }
    let mut ended = 0usize;
    let mut closed = false;
    let count = |inbound: Vec<Inbound>, ended: &mut usize| {
        for i in inbound {
            if matches!(i, Inbound::Ended { .. }) {
                *ended += 1;
            }
        }
    };
    count(
        core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162)),
        &mut ended,
    );
    let mut now = 0u64;
    while let Some((&op, tail)) = rest.split_first() {
        let len = usize::from(tail.first().copied().unwrap_or(0)).min(tail.len().saturating_sub(1));
        let chunk = tail.get(1..1 + len).unwrap_or_default();
        rest = tail.get(1 + len..).unwrap_or_default();
        now += u64::from(op) * 7_919;
        let b = chunk.first().copied().unwrap_or(0);
        let inbound = match op % 13 {
            0 | 1 => core.on_control_data(chunk, now),
            2 | 3 => {
                // Buffered and decoded with a small event budget (control_queue
                // back-pressure, test-server.md §10.1).
                core.push_control(chunk);
                let max = usize::from(b % 4) + 1;
                let out = core.decode_control(now, max);
                let events = out
                    .iter()
                    .filter(|i| matches!(i, Inbound::Event(_) | Inbound::Greeted(_)))
                    .count();
                assert!(events <= max, "decode_control exceeded its budget");
                out
            }
            12 => core.decode_control(now, usize::MAX),
            4 | 5 => core.on_datagram(chunk, now),
            6 => core.handle_timeout(now),
            7 => {
                let s = match b % 3 {
                    0 => StreamRef::Control,
                    1 => StreamRef::Uni(tessera_protocol::frame::StreamKind::World),
                    _ => StreamRef::Untyped,
                };
                let e = match b / 3 % 3 {
                    0 => StreamEnd::Finished,
                    1 => StreamEnd::Reset,
                    _ => StreamEnd::Stopped,
                };
                core.on_stream_end(s, e)
            }
            8 => core.on_extra_stream(),
            9 => {
                let cmd = match b % 10 {
                    0 => SessionCmd::AcceptLogin {
                        account: "a".into(),
                        characters: Vec::new(),
                        looks: Vec::new(),
                    },
                    1 => SessionCmd::RejectLogin {
                        code: LoginFailedCode::from_u16(u16::from(b)),
                        retry_after: u16::from(b),
                    },
                    2 => SessionCmd::CharacterCreated {
                        characters: Vec::new(),
                        looks: Vec::new(),
                    },
                    3 => SessionCmd::RejectCharacter(CharacterCreateFailedCode::NameTaken),
                    4 => SessionCmd::EnterWorld(Box::new(WorldEnter {
                        player: 1,
                        name: "N".into(),
                        instance: 1,
                        epoch: 1,
                        server_time: now,
                        pose: Pose {
                            location: Location::new(1, 0.0, 0.0, 0.0),
                            heading: 0.0,
                            anchor: None,
                        },
                        appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                        look: tessera_protocol::types::Look::none(),
                    })),
                    5 => SessionCmd::RejectEnterWorld(EnterWorldFailedCode::UnknownCharacter),
                    6 => SessionCmd::RejectChat(ChatRejectedCode::RateLimited),
                    7 => SessionCmd::Disconnect(
                        AppErrorCode::Kicked,
                        String::from_utf8_lossy(chunk).into_owned(),
                    ),
                    8 => SessionCmd::ChatDelivered,
                    _ => {
                        SessionCmd::Disconnect(AppErrorCode::from_u16(u16::from(b)), String::new())
                    }
                };
                core.apply(cmd, now)
            }
            10 => core.disconnect(AppErrorCode::SlowConsumer, "", now),
            _ => core.connection_closed(ConnectionEnd::PeerClosed {
                code: AppErrorCode::from_u16(u16::from(b)),
                reason: String::from_utf8_lossy(chunk).into_owned(),
            }),
        };
        count(inbound, &mut ended);
        let mut n = 0;
        while let Some(t) = core.poll_transmit() {
            assert!(!closed, "nothing is sent after Close");
            if matches!(t, Transmit::Close { .. }) {
                closed = true;
            }
            n += 1;
            assert!(n < 10_000, "unbounded outbox");
        }
        assert!(ended <= 1, "the end is reported once");
    }
});
