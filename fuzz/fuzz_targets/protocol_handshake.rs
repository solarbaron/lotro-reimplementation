// SPDX-License-Identifier: GPL-3.0-or-later
//! The client and server connection state machines (tessera-protocol.md §12) fed arbitrary
//! control, world, chat and datagram bytes, with arbitrary local calls and time steps in
//! between: must never panic, and once closed they stay closed and send nothing more.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::client::{ClientConfig, ClientSession, ClientState};
use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::frame::StreamKind;
use tessera_protocol::message::{Credentials, ServerCaps};
use tessera_protocol::server::{ServerParams, ServerSession, ServerState};
use tessera_protocol::session::{PeerDatagrams, StreamEnd, StreamRef};
use tessera_protocol::types::{Appearance, Body};

fuzz_target!(|data: &[u8]| {
    let mut client = ClientSession::new(ClientConfig::default());
    let params = ServerParams {
        caps: ServerCaps(7),
        ..ServerParams::default()
    };
    let mut server = ServerSession::new(params, 0);
    let _ = client.connect(0);
    let _ = client.handshake_complete(0, PeerDatagrams::MaxPayload(1_162));
    let _ = server.handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535));
    let mut now = 0u64;
    let mut rest = data;
    while let Some((&op, tail)) = rest.split_first() {
        let len = usize::from(tail.first().copied().unwrap_or(0)).min(tail.len().saturating_sub(1));
        let chunk = tail.get(1..1 + len).unwrap_or_default();
        rest = tail.get(1 + len..).unwrap_or_default();
        now += u64::from(op) * 10_000;
        match op % 16 {
            0 => drop(client.on_control_data(chunk, now)),
            1 => drop(client.on_uni_stream(chunk.first().copied().unwrap_or(0x57))),
            2 => drop(client.on_stream_data(StreamKind::World, chunk, now)),
            3 => drop(client.on_stream_data(StreamKind::Chat, chunk, now)),
            4 => drop(client.on_datagram(chunk, now)),
            5 => drop(server.on_control_data(chunk, now)),
            6 => drop(server.on_datagram(chunk, now)),
            7 => drop(client.handle_timeout(now)),
            8 => drop(server.handle_timeout(now)),
            9 => drop(client.login(
                Credentials {
                    account: String::from_utf8_lossy(chunk).into_owned(),
                    password: "p".into(),
                },
                now,
            )),
            10 => drop(client.enter_world(String::from_utf8_lossy(chunk).into_owned(), now)),
            11 => drop(server.accept_login("a".into(), Vec::new(), now)),
            12 => drop(client.create_character(
                "n".into(),
                Appearance::new(Body::Character { species: 1, sex: 1 }),
            )),
            13 => drop(client.ping(now)),
            14 => drop(server.disconnect(AppErrorCode::Kicked, "k", now)),
            _ => {
                // Stream ends and extra streams, picked by the first chunk byte.
                let b = chunk.first().copied().unwrap_or(0);
                let stream = match b % 4 {
                    0 => StreamRef::Control,
                    1 => StreamRef::Uni(StreamKind::World),
                    2 => StreamRef::Uni(StreamKind::Chat),
                    _ => StreamRef::Untyped,
                };
                let end = match (b / 4) % 3 {
                    0 => StreamEnd::Finished,
                    1 => StreamEnd::Reset,
                    _ => StreamEnd::Stopped,
                };
                match (b / 12) % 5 {
                    0 => drop(client.on_stream_end(stream, end)),
                    1 => drop(server.on_stream_end(stream, end)),
                    2 => drop(server.on_extra_stream()),
                    3 => drop(client.disconnect(AppErrorCode::NoError, "bye", now)),
                    _ => drop(client.time_request(now)),
                }
            }
        }
        while client.poll_transmit().is_some() {}
        while server.poll_transmit().is_some() {}
    }
    if client.state() == ClientState::Closed {
        while client.poll_transmit().is_some() {}
        assert_eq!(client.on_control_data(&[0; 8], now), Ok(Vec::new()));
        assert_eq!(client.on_datagram(&[0xff; 4], now), Ok(None));
        assert_eq!(client.on_uni_stream(0), Ok(None));
        assert_eq!(
            client.on_stream_data(StreamKind::World, &[0; 8], now),
            Ok(Vec::new())
        );
        assert_eq!(
            client.on_stream_end(StreamRef::Control, StreamEnd::Reset),
            Ok(())
        );
        assert!(client.handle_timeout(u64::MAX).is_none());
        assert!(client.poll_transmit().is_none());
        assert_eq!(client.state(), ClientState::Closed);
    }
    if server.state() == ServerState::Closed {
        while server.poll_transmit().is_some() {}
        assert_eq!(server.on_control_data(&[0; 8], now), Ok(Vec::new()));
        assert_eq!(server.on_datagram(&[0xff; 4], now), Ok(None));
        assert_eq!(
            server.on_stream_end(StreamRef::Control, StreamEnd::Reset),
            Ok(())
        );
        assert_eq!(server.on_extra_stream(), Ok(()));
        assert!(server.handle_timeout(u64::MAX).is_none());
        assert!(server.poll_transmit().is_none());
        assert_eq!(server.state(), ServerState::Closed);
    }
});
