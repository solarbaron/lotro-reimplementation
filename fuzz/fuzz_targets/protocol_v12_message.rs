// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol minor 2 (`tessera-protocol.md` v1.2, named chat channels): `Message::decode_body`
//! for every message type at negotiated minors 0–3 on arbitrary bodies, and a client in the
//! world at minor 2 fed arbitrary chat-stream bytes between its own channel requests
//! (§10.6, §13.2). Nothing may panic; accepted messages re-encode, decode to the same
//! message and re-encode to the same bytes at that minor; a type of a later minor never
//! decodes (§15 invariant 12); the client's channel bookkeeping stays bounded.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_protocol::client::{
    ClientConfig, ClientSession, ClientState, MAX_PENDING_CHANNEL_REQUESTS,
};
use tessera_protocol::frame::{StreamKind, frame_to_vec_minor};
use tessera_protocol::message::{
    ChatChannel, Credentials, HelloAck, LoginOk, ServerCaps, WorldEnter,
};
use tessera_protocol::session::{PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location, Look, Pose};
use tessera_protocol::{Message, MessageType};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    if selector & 0x80 == 0 {
        codec(selector, body);
    } else {
        client(body);
    }
});

fn codec(selector: u8, body: &[u8]) {
    let types = MessageType::ALL;
    let index = usize::from(selector & 0x3F);
    let ty = types[index % types.len()];
    // minor ∈ {1, 2, 3} (minor 0 is the protocol_message target's).
    let minor = 1 + u16::from((selector >> 6) & 1) + u16::from(index >= types.len());
    if let Ok(m) = Message::decode_body_minor(ty, body, minor) {
        assert!(ty.exists_at(minor), "{ty} decoded below its minor");
        assert_eq!(m.message_type(), ty);
        let mut out = Vec::new();
        m.encode_body_minor(&mut out, minor)
            .expect("decoded message re-encodes");
        let back = Message::decode_body_minor(ty, &out, minor).expect("re-encoded body decodes");
        assert_eq!(back, m);
        let mut again = Vec::new();
        back.encode_body_minor(&mut again, minor).unwrap();
        assert_eq!(again, out);
        // Projected below minor 2, a channel form never encodes (§15 invariant 12).
        let named = matches!(
            &m,
            Message::ChatSend(c) if matches!(c.channel, ChatChannel::Named { .. })
        ) || matches!(
            &m,
            Message::ChatMessage(c) if matches!(c.channel, ChatChannel::Named { .. })
        );
        if named || ty.min_minor() == 2 {
            assert!(m.encode_body_minor(&mut Vec::new(), 1).is_err());
        }
    }
}

/// A client session in the world at minor 2 with `CHAT_CHANNELS`.
fn in_world() -> Option<ClientSession> {
    let mut c = ClientSession::new(ClientConfig::default());
    c.connect(0).ok()?;
    c.handshake_complete(0, PeerDatagrams::MaxPayload(1_162))
        .ok()?;
    let ack = Message::HelloAck(HelloAck {
        minor: 2,
        server_caps: ServerCaps::CHAT_CHANNELS,
        tick_rate: 30,
        snapshot_rate: 15,
        interest_radius: 2,
        server_name: String::new(),
        motd: String::new(),
    });
    c.on_control_data(&frame_to_vec_minor(&ack, 0).ok()?, 0)
        .ok()?;
    c.login(
        Credentials {
            account: "a".into(),
            password: "p".into(),
        },
        0,
    )
    .ok()?;
    let ok = Message::LoginOk(LoginOk {
        account: "a".into(),
        characters: vec![CharacterSummary {
            name: "Aldith".into(),
            body: Body::Character { species: 1, sex: 1 },
        }],
        looks: Vec::new(),
    });
    c.on_control_data(&frame_to_vec_minor(&ok, 2).ok()?, 0)
        .ok()?;
    c.on_uni_stream(StreamKind::Chat.to_byte()).ok()?;
    c.enter_world("Aldith".into(), 0).ok()?;
    c.on_uni_stream(StreamKind::World.to_byte()).ok()?;
    let enter = Message::WorldEnter(WorldEnter {
        player: 1,
        name: "Aldith".into(),
        instance: 1,
        epoch: 1,
        server_time: 0,
        pose: Pose {
            location: Location::new(1, 100.0, 100.0, 0.0),
            heading: 0.0,
            anchor: None,
        },
        appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
        look: Look::none(),
    });
    c.on_stream_data(StreamKind::World, &frame_to_vec_minor(&enter, 2).ok()?, 0)
        .ok()?;
    while c.poll_transmit().is_some() {}
    (c.state() == ClientState::InWorld).then_some(c)
}

fn client(mut rest: &[u8]) {
    let Some(mut c) = in_world() else {
        panic!("the client reaches the world");
    };
    let mut requests = 0usize;
    while let Some((&op, tail)) = rest.split_first() {
        let len = usize::from(tail.first().copied().unwrap_or(0)).min(tail.len().saturating_sub(1));
        let chunk = tail.get(1..1 + len).unwrap_or_default();
        rest = tail.get(1 + len..).unwrap_or_default();
        let name = String::from_utf8_lossy(chunk.get(..chunk.len().min(16)).unwrap_or_default())
            .into_owned();
        match op % 4 {
            0 => {
                if c.join_channel(name).is_ok() {
                    requests += 1;
                }
            }
            1 => {
                if c.leave_channel(name).is_ok() {
                    requests += 1;
                }
            }
            _ => {
                let r = c.on_stream_data(StreamKind::Chat, chunk, 0);
                if r.is_err() {
                    while c.poll_transmit().is_some() {}
                    return;
                }
            }
        }
        while let Some(t) = c.poll_transmit() {
            if let Transmit::Close { .. } = t {
                return;
            }
        }
        assert!(c.pending_channel_requests().count() <= MAX_PENDING_CHANNEL_REQUESTS);
        assert!(c.joined_channels().len() <= requests);
        if c.state() != ClientState::InWorld {
            return;
        }
    }
}
