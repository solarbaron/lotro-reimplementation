// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-connection inbound handling without sockets (`ConnectionCore`): control back-pressure
//! per message (`test-server.md` §10.1) and the order of `ChatRejected` answers
//! (`tessera-protocol.md` §6.4.3).

use tessera_protocol::Message;
use tessera_protocol::codes::ChatRejectedCode;
use tessera_protocol::frame::frame_to_vec;
use tessera_protocol::message::{
    ChatChannel, ChatSend, Credentials, EnterWorld, Hello, ServerCaps, WorldEnter,
};
use tessera_protocol::server::{ServerEvent, ServerParams};
use tessera_protocol::session::{PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body, Location, Pose};
use tessera_protocol::value::Stance;
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::shared::SessionCmd;

fn frame(m: &Message) -> Vec<u8> {
    frame_to_vec(m).unwrap()
}

fn sent(core: &mut ConnectionCore) -> Vec<Message> {
    let mut out = Vec::new();
    while let Some(t) = core.poll_transmit() {
        if let Transmit::Message(m) = t {
            out.push(m);
        }
    }
    out
}

/// A connection brought to InWorld.
fn in_world() -> ConnectionCore {
    let params = ServerParams {
        caps: ServerCaps(ServerCaps::CHARACTER_CREATION.0 | ServerCaps::APPEARANCE_CHANGE.0),
        ..ServerParams::default()
    };
    let mut core = ConnectionCore::new(params, 0);
    core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162));
    let ev = core.on_control_data(
        &frame(&Message::Hello(Hello {
            minor_max: 0,
            client_caps: 0,
            max_entities: 1024,
            client_name: "t".into(),
        })),
        1,
    );
    assert!(matches!(ev.as_slice(), [Inbound::Greeted(_)]), "{ev:?}");
    let ev = core.on_control_data(
        &frame(&Message::Login(Credentials {
            account: "alice".into(),
            password: "pw".into(),
        })),
        2,
    );
    assert!(matches!(
        ev.as_slice(),
        [Inbound::Event(ServerEvent::Login { .. })]
    ));
    core.apply(
        SessionCmd::AcceptLogin {
            account: "alice".into(),
            characters: Vec::new(),
            looks: Vec::new(),
        },
        3,
    );
    let ev = core.on_control_data(
        &frame(&Message::EnterWorld(EnterWorld {
            name: "Aldith".into(),
        })),
        4,
    );
    assert!(matches!(
        ev.as_slice(),
        [Inbound::Event(ServerEvent::EnterWorld { .. })]
    ));
    core.apply(
        SessionCmd::EnterWorld(Box::new(WorldEnter {
            player: 1,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: 5,
            pose: Pose {
                location: Location::new(1, 100.0, 100.0, 0.0),
                heading: 0.0,
                anchor: None,
            },
            appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
            look: tessera_protocol::types::Look::none(),
        })),
        5,
    );
    let _ = sent(&mut core);
    assert!(core.may_send_world());
    core
}

/// Finding: one read of 240 small frames used to be decoded and forwarded at once, about 4×
/// `control_queue`. Decoding now stops after the allowed number of events and keeps the rest.
#[test]
fn control_burst_is_decoded_per_message() {
    let mut core = in_world();
    let one = frame(&Message::StanceSet(Stance::Seated));
    let burst: Vec<u8> = one.iter().copied().cycle().take(one.len() * 240).collect();
    assert!(burst.len() < 16 * 1024);
    core.push_control(&burst);
    let ev = core.decode_control(10, 64);
    assert_eq!(ev.len(), 64);
    assert!(core.has_unread_control());
    let mut total = ev.len();
    while core.has_unread_control() {
        let ev = core.decode_control(10, 64);
        assert!(ev.len() <= 64 && !ev.is_empty());
        total += ev.len();
    }
    assert_eq!(total, 240);
    // A frame split across reads still decodes once complete.
    core.push_control(&one[..3]);
    assert!(core.decode_control(11, 64).is_empty());
    assert!(!core.has_unread_control());
    core.push_control(&one[3..]);
    assert_eq!(core.decode_control(12, 64).len(), 1);
}

fn chat(text: &str, channel: ChatChannel) -> Vec<u8> {
    frame(&Message::ChatSend(ChatSend {
        channel,
        text: text.into(),
    }))
}

fn rejections(msgs: &[Message]) -> Vec<ChatRejectedCode> {
    msgs.iter()
        .filter_map(|m| match m {
            Message::ChatRejected(f) => Some(f.code),
            _ => None,
        })
        .collect()
}

/// Finding: a rejection the session makes itself (here the protocol's chat rate limit) went
/// out at once, before the simulation's answer to an earlier `ChatSend`.
#[test]
fn chat_rejections_keep_the_order_of_chat_sends() {
    let mut core = in_world();
    let tell = ChatChannel::Tell {
        peer: "Nobody".into(),
    };
    // Five ChatSends within the protocol's bucket, then a sixth over it.
    let mut forwarded = 0;
    for i in 0..5 {
        let ev = core.on_control_data(&chat(&format!("line {i}"), tell.clone()), 100);
        forwarded += ev
            .iter()
            .filter(|e| matches!(e, Inbound::Event(ServerEvent::Chat(_))))
            .count();
    }
    assert_eq!(forwarded, 5);
    let ev = core.on_control_data(&chat("over the limit", ChatChannel::Say), 100);
    assert!(ev.is_empty(), "{ev:?}");
    // The session's RATE_LIMITED waits behind the five unanswered ChatSends.
    assert!(rejections(&sent(&mut core)).is_empty());
    assert_eq!(core.unanswered_chats(), 5);
    core.apply(
        SessionCmd::RejectChat(ChatRejectedCode::UnknownRecipient),
        101,
    );
    for _ in 0..3 {
        core.apply(SessionCmd::ChatDelivered, 101);
    }
    assert_eq!(
        rejections(&sent(&mut core)),
        vec![ChatRejectedCode::UnknownRecipient]
    );
    core.apply(SessionCmd::RejectChat(ChatRejectedCode::Muted), 102);
    assert_eq!(
        rejections(&sent(&mut core)),
        vec![ChatRejectedCode::Muted, ChatRejectedCode::RateLimited]
    );
    assert_eq!(core.unanswered_chats(), 0);
    // With nothing outstanding, a session-made rejection goes out at once.
    let ev = core.on_control_data(&chat("again", ChatChannel::Say), 103);
    assert!(ev.is_empty());
    assert_eq!(
        rejections(&sent(&mut core)),
        vec![ChatRejectedCode::RateLimited]
    );
}
