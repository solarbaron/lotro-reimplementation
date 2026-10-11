// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol minor 2 (v1.2, named chat channels, decision 0035): the vectors of
//! `docs/specs/tessera-protocol.md` §16.9 byte for byte, its negative and non-fatal cases,
//! and the client and server state machines with channels (§10.6, §13.2, §15 invariants
//! 12–13), driven against each other in memory.

use tessera_protocol::client::{
    ChannelRequest, ClientConfig, ClientEvent, ClientSession, ClientState,
    MAX_PENDING_CHANNEL_REQUESTS,
};
use tessera_protocol::codes::{AppErrorCode, ChannelRefusedCode, ChatRejectedCode};
use tessera_protocol::frame::{FrameDecoder, StreamKind, frame_to_vec_minor};
use tessera_protocol::limits::{MINOR_CHANNELS, PROTOCOL_MINOR};
use tessera_protocol::message::*;
use tessera_protocol::server::{ServerEvent, ServerParams, ServerSession, ServerState};
use tessera_protocol::session::{ConnectionEnd, PeerDatagrams, Transmit, UsageError, Violation};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location, Look, Pose};
use tessera_protocol::{Channel, Message, ProtocolError, Side};

const S: u64 = 1_000_000;

fn hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

fn with_length(mut frame: Vec<u8>) -> Vec<u8> {
    let len = u32::try_from(frame.len() - 4).unwrap();
    frame[..4].copy_from_slice(&len.to_le_bytes());
    frame
}

fn decode_at(
    channel: Channel,
    sender: Side,
    minor: u16,
    bytes: &[u8],
) -> Result<Option<Message>, ProtocolError> {
    let mut d = FrameDecoder::with_minor(channel, sender, minor);
    let mut input = bytes;
    d.push(&mut input)
}

/// One §16.9 vector: encodes to exactly `expected_hex` at minor 2 and decodes back.
fn frame_v12(msg: Message, sender: Side, expected_hex: &str, len: usize) {
    let expected = hex(expected_hex);
    assert_eq!(expected.len(), len, "{}: vector length", msg.message_type());
    let encoded = frame_to_vec_minor(&msg, 2).unwrap();
    assert_eq!(encoded, expected, "{}: encoding", msg.message_type());
    assert_eq!(
        decode_at(msg.channel(), sender, 2, &expected),
        Ok(Some(msg.clone())),
        "{}: decoding",
        msg.message_type()
    );
}

const HELLO_ACK: &str = "19 00 00 00 02 00 02 00 0f 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68
    61 72 64 00 00";
const CHANNEL_JOIN: &str = "08 00 00 00 15 00 04 00 4c 6f 72 65";
const CHAT_SEND: &str = "0d 00 00 00 0e 00 06 04 00 4c 6f 72 65 02 00 68 69";
const CHAT_ECHO: &str = "26 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 06 01 2a 00 00 00 00 00 00 00
    06 00 41 6c 64 69 74 68 04 00 4c 6f 72 65 02 00 68 69";

fn lore_line(echo: bool, sender_entity: u64, sender: &str, text: &str) -> Message {
    Message::ChatMessage(ChatMessage {
        server_time: 7_000_000,
        channel: ChatChannel::Named {
            name: "Lore".into(),
        },
        echo,
        sender_entity,
        sender: sender.into(),
        text: text.into(),
    })
}

#[test]
fn s16_9_minor_2() {
    frame_v12(
        Message::HelloAck(HelloAck {
            minor: 2,
            server_caps: ServerCaps::REGISTRATION
                | ServerCaps::CHARACTER_CREATION
                | ServerCaps::APPEARANCE_CHANGE
                | ServerCaps::CHAT_CHANNELS,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Test Shard".into(),
            motd: String::new(),
        }),
        Side::Server,
        HELLO_ACK,
        29,
    );
    frame_v12(
        Message::ChannelJoin {
            name: "Lore".into(),
        },
        Side::Client,
        CHANNEL_JOIN,
        12,
    );
    frame_v12(
        Message::ChannelJoined {
            name: "Lore".into(),
        },
        Side::Server,
        "08 00 00 00 04 02 04 00 4c 6f 72 65",
        12,
    );
    frame_v12(
        Message::ChatSend(ChatSend {
            channel: ChatChannel::Named {
                name: "Lore".into(),
            },
            text: "hi".into(),
        }),
        Side::Client,
        CHAT_SEND,
        17,
    );
    frame_v12(
        lore_line(true, 42, "Aldith", "hi"),
        Side::Server,
        CHAT_ECHO,
        42,
    );
    frame_v12(
        lore_line(false, 43, "Bram", "hello"),
        Side::Server,
        "27 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 06 00 2b 00 00 00 00 00 00 00 04 00 42 72
         61 6d 04 00 4c 6f 72 65 05 00 68 65 6c 6c 6f",
        43,
    );
    frame_v12(
        Message::ChannelLeave {
            name: "Lore".into(),
        },
        Side::Client,
        "08 00 00 00 16 00 04 00 4c 6f 72 65",
        12,
    );
    frame_v12(
        Message::ChannelLeft(ChannelLeft {
            name: "Lore".into(),
            reason: ChannelLeftReason::Left,
        }),
        Side::Server,
        "09 00 00 00 05 02 04 00 4c 6f 72 65 00",
        13,
    );
    frame_v12(
        Message::ChannelRefused(ChannelRefused {
            code: ChannelRefusedCode::AlreadyJoined,
            name: "Lore".into(),
            text: String::new(),
        }),
        Side::Server,
        "0c 00 00 00 06 02 03 00 04 00 4c 6f 72 65 00 00",
        16,
    );
    frame_v12(
        Message::ChannelRefused(ChannelRefused {
            code: ChannelRefusedCode::InvalidName,
            name: String::new(),
            text: String::new(),
        }),
        Side::Server,
        "08 00 00 00 06 02 02 00 00 00 00 00",
        12,
    );
    frame_v12(
        Message::ChatRejected(Failure {
            code: ChatRejectedCode::NotJoined,
            text: String::new(),
        }),
        Side::Server,
        "06 00 00 00 03 02 06 00 00 00",
        10,
    );
}

/// §13.1: `ChatRejected` code 6 below minor 2 is undefined there and shown as a generic
/// failure, so the decoder keeps it as `Other(6)`; at minor 2 it is `NOT_JOINED`.
#[test]
fn chat_rejected_code_6_below_minor_2_is_generic() {
    let bytes = hex("06 00 00 00 03 02 06 00 00 00");
    let channel = Message::ChatRejected(Failure {
        code: ChatRejectedCode::NotJoined,
        text: String::new(),
    })
    .channel();
    for minor in 0..MINOR_CHANNELS {
        let Ok(Some(Message::ChatRejected(f))) = decode_at(channel, Side::Server, minor, &bytes)
        else {
            panic!("minor {minor}: ChatRejected expected");
        };
        assert!(
            matches!(f.code, ChatRejectedCode::Other(6)),
            "minor {minor}"
        );
        assert_eq!(f.code.name(), "OTHER");
        assert_eq!(f.code.to_string(), "code 6");
        // It re-encodes to the same bytes.
        let again = frame_to_vec_minor(&Message::ChatRejected(f), minor).unwrap();
        assert_eq!(again, bytes);
    }
    let Ok(Some(Message::ChatRejected(f))) =
        decode_at(channel, Side::Server, MINOR_CHANNELS, &bytes)
    else {
        panic!("ChatRejected expected");
    };
    assert_eq!(f.code.name(), "NOT_JOINED");
}

/// "The `ChatSend` and the first `ChatMessage` differ from the v1.0 Tell vectors only in the
/// channel byte (6 for 4) and the name in the `peer` position."
#[test]
fn s16_9_named_lines_mirror_tells() {
    let tell = frame_to_vec_minor(
        &Message::ChatSend(ChatSend {
            channel: ChatChannel::Tell {
                peer: "Lore".into(),
            },
            text: "hi".into(),
        }),
        2,
    )
    .unwrap();
    let mut named = tell.clone();
    named[6] = 6;
    assert_eq!(named, hex(CHAT_SEND));
    let tell = frame_to_vec_minor(
        &Message::ChatMessage(ChatMessage {
            server_time: 7_000_000,
            channel: ChatChannel::Tell {
                peer: "Lore".into(),
            },
            echo: true,
            sender_entity: 42,
            sender: "Aldith".into(),
            text: "hi".into(),
        }),
        2,
    )
    .unwrap();
    let mut named = tell;
    named[14] = 6;
    assert_eq!(named, hex(CHAT_ECHO));
}

#[test]
fn s16_9_negative_vectors() {
    let c = Channel::Control;
    let ch = Channel::Chat;
    // ChannelJoin at minor 1: an unknown message type.
    assert_eq!(
        decode_at(c, Side::Client, 1, &hex(CHANNEL_JOIN)),
        Err(ProtocolError::UnknownMessageType(0x0015))
    );
    // ChatSend channel 6 at minor 1.
    assert_eq!(
        decode_at(c, Side::Client, 1, &hex(CHAT_SEND)),
        Err(ProtocolError::InvalidEnum {
            field: "channel",
            value: 6
        })
    );
    // The first ChatMessage at minor 1 (received by a client).
    assert_eq!(
        decode_at(ch, Side::Server, 1, &hex(CHAT_ECHO)),
        Err(ProtocolError::InvalidEnum {
            field: "channel",
            value: 6
        })
    );
    // ChatSend channel 7 (no fellowship chat on the wire), at every minor.
    let mut seven = hex(CHAT_SEND);
    seven[6] = 7;
    for minor in 0..=PROTOCOL_MINOR {
        assert_eq!(
            decode_at(c, Side::Client, minor, &seven),
            Err(ProtocolError::InvalidEnum {
                field: "channel",
                value: 7
            })
        );
    }
    // ChannelJoin with a 65-byte name.
    let mut long = hex("00 00 00 00 15 00 41 00");
    long.extend(std::iter::repeat_n(b'A', 65));
    assert!(matches!(
        decode_at(c, Side::Client, 2, &with_length(long)),
        Err(ProtocolError::StringLength {
            len: 65,
            max: 64,
            ..
        })
    ));
    // ChannelJoined with an empty name.
    assert!(matches!(
        decode_at(ch, Side::Server, 2, &hex("04 00 00 00 04 02 00 00")),
        Err(ProtocolError::StringLength { len: 0, min: 1, .. })
    ));
    // ChatMessage on channel 6 with an empty channel_name.
    let mut empty = hex(CHAT_ECHO);
    // Replace "04 00 4c 6f 72 65" (channel name) by "00 00".
    let at = 4 + 2 + 8 + 1 + 1 + 8 + 2 + 6;
    empty.splice(at..at + 6, [0, 0]);
    assert!(matches!(
        decode_at(ch, Side::Server, 2, &with_length(empty)),
        Err(ProtocolError::StringLength {
            field: "channel_name",
            len: 0,
            ..
        })
    ));
    // ChannelLeft with reason 2.
    assert_eq!(
        decode_at(
            ch,
            Side::Server,
            2,
            &hex("09 00 00 00 05 02 04 00 4c 6f 72 65 02")
        ),
        Err(ProtocolError::InvalidEnum {
            field: "reason",
            value: 2
        })
    );
    // ChannelJoin with one extra trailing byte.
    let mut trailing = hex(CHANNEL_JOIN);
    trailing.push(0);
    assert_eq!(
        decode_at(c, Side::Client, 2, &with_length(trailing)),
        Err(ProtocolError::TrailingBytes(1))
    );
    // Every chat-stream channel message at minor 0 and 1 is unknown, too.
    for (ty, bytes) in [
        (0x0204, "08 00 00 00 04 02 04 00 4c 6f 72 65"),
        (0x0205, "09 00 00 00 05 02 04 00 4c 6f 72 65 00"),
        (0x0206, "08 00 00 00 06 02 02 00 00 00 00 00"),
    ] {
        for minor in 0..MINOR_CHANNELS {
            assert_eq!(
                decode_at(ch, Side::Server, minor, &hex(bytes)),
                Err(ProtocolError::UnknownMessageType(ty))
            );
        }
    }
}

#[test]
fn s16_9_non_fatal_names_decode() {
    // An empty name and a name with U+0007 decode: the server refuses them with
    // INVALID_NAME, not as a violation.
    assert_eq!(
        decode_at(
            Channel::Control,
            Side::Client,
            2,
            &hex("04 00 00 00 15 00 00 00")
        ),
        Ok(Some(Message::ChannelJoin {
            name: String::new()
        }))
    );
    assert_eq!(
        decode_at(
            Channel::Control,
            Side::Client,
            2,
            &hex("07 00 00 00 15 00 03 00 61 07 62")
        ),
        Ok(Some(Message::ChannelJoin {
            name: "a\u{7}b".into()
        }))
    );
}

/// §15 invariant 12: below minor 2 none of the v1.2 forms is ever encoded.
#[test]
fn invariant_12_nothing_of_minor_2_below_it() {
    let msgs = [
        Message::ChannelJoin { name: "x".into() },
        Message::ChannelLeave { name: "x".into() },
        Message::ChannelJoined { name: "x".into() },
        Message::ChannelLeft(ChannelLeft {
            name: "x".into(),
            reason: ChannelLeftReason::Removed,
        }),
        Message::ChannelRefused(ChannelRefused {
            code: ChannelRefusedCode::ServerError,
            name: String::new(),
            text: String::new(),
        }),
        Message::ChatSend(ChatSend {
            channel: ChatChannel::Named { name: "x".into() },
            text: "t".into(),
        }),
        lore_line(false, 0, "Bram", "t"),
    ];
    for m in &msgs {
        for minor in 0..MINOR_CHANNELS {
            assert!(
                frame_to_vec_minor(m, minor).is_err(),
                "{} at minor {minor}",
                m.message_type()
            );
        }
        assert!(frame_to_vec_minor(m, 2).is_ok(), "{}", m.message_type());
    }
    assert_eq!(MessageType::ChannelJoin.min_minor(), 2);
    assert_eq!(MessageType::ChatSend.min_minor(), 0);
    assert!(!MessageType::ChannelRefused.exists_at(1));
    // Caps: bit 3 is meaningful only at minor ≥ 2.
    let caps = ServerCaps::REGISTRATION | ServerCaps::CHAT_CHANNELS;
    assert_eq!(caps.effective(1), ServerCaps::REGISTRATION);
    assert_eq!(caps.effective(2), caps);
}

// --- state machines ---

struct Pair {
    client: ClientSession,
    server: ServerSession,
    client_events: Vec<ClientEvent>,
    server_events: Vec<ServerEvent>,
    client_violation: Option<Violation>,
    server_violation: Option<Violation>,
    server_closed: Option<AppErrorCode>,
    now: u64,
}

fn params(channels: bool) -> ServerParams {
    let mut caps = ServerCaps::REGISTRATION | ServerCaps::CHARACTER_CREATION;
    if channels {
        caps = caps | ServerCaps::CHAT_CHANNELS;
    }
    ServerParams {
        caps,
        server_name: "Test Shard".into(),
        ..ServerParams::default()
    }
}

impl Pair {
    /// Greeted, logged in and in the world at the minor `minor_max` negotiates.
    fn in_world(minor_max: u16, channels: bool) -> Self {
        let mut p = Self {
            client: ClientSession::new(ClientConfig {
                minor_max,
                ..ClientConfig::default()
            }),
            server: ServerSession::new(params(channels), 0),
            client_events: Vec::new(),
            server_events: Vec::new(),
            client_violation: None,
            server_violation: None,
            server_closed: None,
            now: 0,
        };
        p.client.connect(0).unwrap();
        p.server
            .handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535))
            .unwrap();
        p.client
            .handshake_complete(0, PeerDatagrams::MaxPayload(1_162))
            .unwrap();
        p.pump();
        p.client
            .login(
                Credentials {
                    account: "alice".into(),
                    password: "pw".into(),
                },
                0,
            )
            .unwrap();
        p.pump();
        p.server
            .accept_login(
                "alice".into(),
                vec![CharacterSummary {
                    name: "Aldith".into(),
                    body: Body::Character {
                        species: 23,
                        sex: 1,
                    },
                }],
                0,
            )
            .unwrap();
        p.pump();
        p.client.enter_world("Aldith".into(), 0).unwrap();
        p.pump();
        p.server
            .enter_world(WorldEnter {
                player: 42,
                name: "Aldith".into(),
                instance: 1,
                epoch: 1,
                server_time: 5 * S,
                pose: Pose {
                    location: Location::new(1, 20_250.5, 22_500.25, 401.0),
                    heading: 0.0,
                    anchor: None,
                },
                appearance: Appearance::new(Body::Character {
                    species: 23,
                    sex: 1,
                }),
                look: Look::none(),
            })
            .unwrap();
        p.pump();
        assert_eq!(p.client.state(), ClientState::InWorld);
        p.client_events.clear();
        p.server_events.clear();
        p
    }

    fn pump(&mut self) {
        loop {
            let mut moved = false;
            while let Some(t) = self.client.poll_transmit() {
                moved = true;
                if let Transmit::Message(m) = t {
                    let b = frame_to_vec_minor(&m, self.client.minor()).unwrap();
                    match self.server.on_control_data(&b, self.now) {
                        Ok(e) => self.server_events.extend(e),
                        Err(v) => self.server_violation = Some(v),
                    }
                }
            }
            while let Some(t) = self.server.poll_transmit() {
                moved = true;
                match t {
                    Transmit::Message(m) => {
                        let b = frame_to_vec_minor(&m, self.server.minor()).unwrap();
                        let r = match m.channel() {
                            Channel::Control => self.client.on_control_data(&b, self.now),
                            Channel::World => {
                                self.client.on_stream_data(StreamKind::World, &b, self.now)
                            }
                            Channel::Chat => {
                                self.client.on_stream_data(StreamKind::Chat, &b, self.now)
                            }
                            Channel::Datagram => Ok(Vec::new()),
                        };
                        match r {
                            Ok(e) => self.client_events.extend(e),
                            Err(v) => self.client_violation = Some(v),
                        }
                    }
                    Transmit::OpenStream(kind) => {
                        self.client.on_uni_stream(kind.to_byte()).unwrap();
                    }
                    Transmit::Close { code, .. } => self.server_closed = Some(code),
                    Transmit::FinishControl => {}
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// Feeds raw client bytes to the server.
    fn raw(&mut self, bytes: &[u8]) {
        match self.server.on_control_data(bytes, self.now) {
            Ok(e) => self.server_events.extend(e),
            Err(v) => self.server_violation = Some(v),
        }
    }

    /// Feeds a server message to the client's chat stream, bypassing the server session.
    fn feed_client(&mut self, m: &Message) {
        let b = frame_to_vec_minor(m, self.client.minor()).unwrap();
        match self.client.on_stream_data(StreamKind::Chat, &b, self.now) {
            Ok(e) => self.client_events.extend(e),
            Err(v) => self.client_violation = Some(v),
        }
    }
}

#[test]
fn hello_ack_carries_bit_3_only_at_minor_2() {
    for (minor_max, want) in [(0u16, false), (1, false), (2, true)] {
        let p = Pair::in_world(minor_max, true);
        assert_eq!(p.client.minor(), minor_max);
        let sent = p.client.hello_ack().unwrap().server_caps;
        assert_eq!(
            sent.contains(ServerCaps::CHAT_CHANNELS),
            want,
            "{minor_max}"
        );
        assert_eq!(p.server.channels_allowed(), want);
    }
    let p = Pair::in_world(2, false);
    assert!(!p.client.server_caps().contains(ServerCaps::CHAT_CHANNELS));
}

#[test]
fn join_speak_leave() {
    let mut p = Pair::in_world(2, true);
    p.client.join_channel("lore".into()).unwrap();
    p.pump();
    assert_eq!(
        std::mem::take(&mut p.server_events),
        [ServerEvent::ChannelJoin {
            name: "lore".into()
        }]
    );
    // The server keeps the spelling of the join that created the channel.
    p.server.channel_joined("Lore".into()).unwrap();
    p.pump();
    assert_eq!(
        std::mem::take(&mut p.client_events),
        [ClientEvent::ChannelJoined {
            name: "Lore".into(),
            request: ChannelRequest::Join("lore".into())
        }]
    );
    assert_eq!(p.client.joined_channels(), ["Lore".to_owned()]);
    // A line on the channel.
    p.client
        .chat(
            ChatChannel::Named {
                name: "lore".into(),
            },
            "hi".into(),
        )
        .unwrap();
    p.pump();
    assert!(matches!(
        &p.server_events[..],
        [ServerEvent::Chat(ChatSend { channel: ChatChannel::Named { name }, .. })] if name == "lore"
    ));
    p.server
        .send(lore_line(true, 42, "Aldith", "hi"), p.now)
        .unwrap();
    p.pump();
    assert!(matches!(&p.client_events[..], [ClientEvent::Chat(m)] if m.echo));
    p.client_events.clear();
    p.server_events.clear();
    // A line of a channel the client does not hold is dropped and counted (§13.2).
    let before = p.client.dropped();
    let mut other = lore_line(false, 0, "Bram", "x");
    if let Message::ChatMessage(m) = &mut other {
        m.channel = ChatChannel::Named {
            name: "lore".into(),
        };
    }
    p.feed_client(&other);
    assert!(p.client_events.is_empty());
    assert_eq!(p.client.dropped(), before + 1);
    // Leave.
    p.client.leave_channel("Lore".into()).unwrap();
    p.pump();
    assert_eq!(
        std::mem::take(&mut p.server_events),
        [ServerEvent::ChannelLeave {
            name: "Lore".into()
        }]
    );
    p.server
        .channel_left("Lore".into(), ChannelLeftReason::Left)
        .unwrap();
    p.pump();
    assert!(matches!(
        &p.client_events[..],
        [ClientEvent::ChannelLeft { left, request: Some(ChannelRequest::Leave(n)) }]
            if left.name == "Lore" && n == "Lore"
    ));
    assert!(p.client.joined_channels().is_empty());
    // A stale line after the leave is dropped.
    p.client_events.clear();
    p.feed_client(&lore_line(false, 43, "Bram", "late"));
    assert!(p.client_events.is_empty());
    // A ChannelLeft(REMOVED) for a channel not held is dropped, not fatal.
    p.feed_client(&Message::ChannelLeft(ChannelLeft {
        name: "Lore".into(),
        reason: ChannelLeftReason::Removed,
    }));
    assert!(p.client_events.is_empty());
    assert!(p.client_violation.is_none());
    assert!(p.server_violation.is_none());
}

#[test]
fn refusals_pair_with_requests_in_order() {
    let mut p = Pair::in_world(2, true);
    p.client.join_channel(String::new()).unwrap();
    p.client.join_channel("a\u{7}b".into()).unwrap();
    p.client.leave_channel("Nowhere".into()).unwrap();
    p.pump();
    assert_eq!(p.server_events.len(), 3);
    assert_eq!(p.client.pending_channel_requests().count(), 3);
    for (code, name) in [
        (ChannelRefusedCode::InvalidName, ""),
        (ChannelRefusedCode::InvalidName, "a\u{7}b"),
        (ChannelRefusedCode::NotJoined, "Nowhere"),
    ] {
        assert_eq!(p.server.refuse_channel(code, name, "", p.now), Ok(None));
    }
    p.pump();
    let names: Vec<_> = p
        .client_events
        .iter()
        .map(|e| match e {
            ClientEvent::ChannelRefused { refused, request } => {
                (refused.code, refused.name.clone(), request.clone())
            }
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        names,
        [
            (
                ChannelRefusedCode::InvalidName,
                String::new(),
                ChannelRequest::Join(String::new())
            ),
            // The bell is sanitised to a space in the refusal.
            (
                ChannelRefusedCode::InvalidName,
                "a b".into(),
                ChannelRequest::Join("a\u{7}b".into())
            ),
            (
                ChannelRefusedCode::NotJoined,
                "Nowhere".into(),
                ChannelRequest::Leave("Nowhere".into())
            ),
        ]
    );
    assert_eq!(p.client.pending_channel_requests().count(), 0);
}

#[test]
fn rate_limited_refusals_count_towards_flooding() {
    let mut p = Pair::in_world(2, true);
    for i in 0..19 {
        p.client.join_channel(format!("c{i}")).unwrap();
        p.pump();
        assert_eq!(
            p.server
                .refuse_channel(ChannelRefusedCode::RateLimited, "c", "", p.now),
            Ok(None),
            "{i}"
        );
    }
    p.client.join_channel("c20".into()).unwrap();
    p.pump();
    let r = p
        .server
        .refuse_channel(ChannelRefusedCode::RateLimited, "c", "", p.now);
    assert!(matches!(
        r,
        Ok(Some(ServerEvent::Closed {
            code: AppErrorCode::Flooding,
            client: false
        }))
    ));
    assert_eq!(p.server.state(), ServerState::Closing);
    // Other refusal codes do not count.
    let mut p = Pair::in_world(2, true);
    for _ in 0..40 {
        p.client.join_channel("Lore".into()).unwrap();
        p.pump();
        assert_eq!(
            p.server
                .refuse_channel(ChannelRefusedCode::AlreadyJoined, "Lore", "", p.now),
            Ok(None)
        );
        p.pump();
        p.now += S / 2;
    }
    assert_eq!(p.server.state(), ServerState::InWorld);
}

#[test]
fn channel_requests_without_the_capability_are_violations() {
    // Minor 2 but CHAT_CHANNELS not advertised.
    let mut p = Pair::in_world(2, false);
    assert_eq!(
        p.client.join_channel("Lore".into()),
        Err(UsageError::CapabilityMissing)
    );
    assert_eq!(
        p.client.chat(
            ChatChannel::Named {
                name: "Lore".into()
            },
            "hi".into()
        ),
        Err(UsageError::CapabilityMissing)
    );
    p.raw(&hex(CHANNEL_JOIN));
    assert_eq!(
        p.server_violation,
        Some(Violation::CapabilityMissing(MessageType::ChannelJoin))
    );
    assert_eq!(p.server.state(), ServerState::Closed);
    let mut p = Pair::in_world(2, false);
    p.raw(&hex(CHAT_SEND));
    assert_eq!(
        p.server_violation,
        Some(Violation::CapabilityMissing(MessageType::ChatSend))
    );
    // Minor 1: an unknown message type and an undefined channel value.
    let mut p = Pair::in_world(1, true);
    assert_eq!(
        p.client.join_channel("Lore".into()),
        Err(UsageError::CapabilityMissing)
    );
    p.raw(&hex(CHANNEL_JOIN));
    assert_eq!(
        p.server_violation,
        Some(Violation::Decode(ProtocolError::UnknownMessageType(0x0015)))
    );
    let mut p = Pair::in_world(1, true);
    p.raw(&hex(CHAT_SEND));
    assert!(matches!(
        p.server_violation,
        Some(Violation::Decode(ProtocolError::InvalidEnum {
            value: 6,
            ..
        }))
    ));
    // The server cannot answer with a channel message at minor 1, nor send code 6.
    let mut p = Pair::in_world(1, true);
    assert_eq!(
        p.server.channel_joined("Lore".into()),
        Err(UsageError::CapabilityMissing)
    );
    assert!(
        p.server
            .reject_chat(ChatRejectedCode::NotJoined, "", 0)
            .is_err()
    );
    assert!(p.server.send(lore_line(false, 0, "Bram", "x"), 0).is_err());
}

#[test]
fn client_rejects_unsolicited_answers() {
    // ChannelJoined without a pending join.
    let mut p = Pair::in_world(2, true);
    p.feed_client(&Message::ChannelJoined {
        name: "Lore".into(),
    });
    assert!(matches!(
        p.client_violation,
        Some(Violation::UnexpectedMessage {
            ty: MessageType::ChannelJoined,
            ..
        })
    ));
    // ChannelRefused without a pending request.
    let mut p = Pair::in_world(2, true);
    p.feed_client(&Message::ChannelRefused(ChannelRefused {
        code: ChannelRefusedCode::ServerError,
        name: String::new(),
        text: String::new(),
    }));
    assert!(p.client_violation.is_some());
    // ChannelLeft(LEFT) answering a join.
    let mut p = Pair::in_world(2, true);
    p.client.join_channel("Lore".into()).unwrap();
    p.pump();
    p.feed_client(&Message::ChannelLeft(ChannelLeft {
        name: "Lore".into(),
        reason: ChannelLeftReason::Left,
    }));
    assert!(p.client_violation.is_some());
    // ChannelJoined answering a leave.
    let mut p = Pair::in_world(2, true);
    p.client.leave_channel("Lore".into()).unwrap();
    p.pump();
    p.feed_client(&Message::ChannelJoined {
        name: "Lore".into(),
    });
    assert!(p.client_violation.is_some());
    // A HelloAck with bit 3 at minor 1 is not fatal; the client just never uses channels.
    let caps = ServerCaps::CHAT_CHANNELS;
    assert!(!caps.effective(1).contains(ServerCaps::CHAT_CHANNELS));
}

#[test]
fn removed_membership_and_pending_bound() {
    let mut p = Pair::in_world(2, true);
    p.client.join_channel("Lore".into()).unwrap();
    p.pump();
    p.server.channel_joined("Lore".into()).unwrap();
    p.pump();
    p.client_events.clear();
    p.server
        .channel_left("Lore".into(), ChannelLeftReason::Removed)
        .unwrap();
    p.pump();
    assert!(matches!(
        &p.client_events[..],
        [ClientEvent::ChannelLeft { left, request: None }]
            if left.reason == ChannelLeftReason::Removed
    ));
    assert!(!p.client.holds_channel("Lore"));
    // The client's own bound on unanswered requests.
    let mut p = Pair::in_world(2, true);
    for i in 0..MAX_PENDING_CHANNEL_REQUESTS {
        p.client.join_channel(format!("c{i}")).unwrap();
    }
    assert_eq!(
        p.client.join_channel("one more".into()),
        Err(UsageError::Outstanding)
    );
    // Channel requests outside the world are refused locally and fatal on the wire.
    let mut c = ClientSession::new(ClientConfig::default());
    assert!(c.join_channel("Lore".into()).is_err());
    let mut s = ServerSession::new(params(true), 0);
    s.handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535))
        .unwrap();
    let mut hello = frame_to_vec_minor(
        &Message::Hello(Hello {
            minor_max: 2,
            client_caps: 0,
            max_entities: 1024,
            client_name: String::new(),
        }),
        0,
    )
    .unwrap();
    hello.extend(hex(CHANNEL_JOIN));
    assert!(matches!(
        s.on_control_data(&hello, 0),
        Err(Violation::UnexpectedMessage {
            ty: MessageType::ChannelJoin,
            ..
        })
    ));
    let _ = s.connection_closed(ConnectionEnd::Local);
}
