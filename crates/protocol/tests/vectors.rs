// SPDX-License-Identifier: GPL-3.0-or-later
//! Every test vector of `docs/specs/tessera-protocol.md` §16: byte-exact encoding and
//! decoding, plus the negative vectors of §16.7.

use tessera_protocol::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode, TeleportRefusedCode,
};
use tessera_protocol::connect::ConnectString;
use tessera_protocol::fingerprint::{Fingerprint, ed25519_spki};
use tessera_protocol::frame::{
    FrameDecoder, StreamKind, decode_datagram, encode_datagram, frame_to_vec, frame_to_vec_minor,
};
use tessera_protocol::limits::ALPN_V1;
use tessera_protocol::message::*;
use tessera_protocol::time::TimeSample;
use tessera_protocol::types::*;
use tessera_protocol::value::{Property, Stance, Value, pid};
use tessera_protocol::{Channel, Message, ProtocolError, Side};

fn hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

fn l() -> Location {
    Location::new(1, 20_250.5, 22_500.25, 401.0)
}

fn a() -> Appearance {
    Appearance {
        body: Body::Character {
            species: 23,
            sex: 1,
        },
        scale: 1.0,
        idle: None,
        equipment: vec![
            Equipped {
                item: 0x7000_1234,
                dye: Some(0.25),
                secondary_dye: None,
                stowed: false,
                slot: None,
            },
            Equipped {
                item: 0x7000_5678,
                dye: None,
                secondary_dye: None,
                stowed: true,
                slot: None,
            },
        ],
    }
}

fn anchor() -> Anchor {
    Anchor::new(126, 140, 257)
}

/// Checks a stream frame both ways: encoding is byte-exact, decoding gives the message back,
/// and the frame length is 2 + body.
fn frame(msg: Message, sender: Side, expected_hex: &str, len: usize) {
    let expected = hex(expected_hex);
    assert_eq!(
        expected.len(),
        len,
        "{:?}: vector length",
        msg.message_type()
    );
    let encoded = frame_to_vec(&msg).unwrap();
    assert_eq!(encoded, expected, "{:?}: encoding", msg.message_type());
    let length = u32::from_le_bytes(expected[..4].try_into().unwrap()) as usize;
    assert_eq!(length + 4, expected.len());
    let mut d = FrameDecoder::new(msg.channel(), sender);
    let mut input: &[u8] = &expected;
    assert_eq!(d.push(&mut input).unwrap(), Some(msg.clone()));
    assert!(input.is_empty());
}

fn datagram(msg: Message, sender: Side, expected_hex: &str, len: usize) {
    let expected = hex(expected_hex);
    assert_eq!(expected.len(), len);
    assert_eq!(encode_datagram(&msg).unwrap(), expected);
    assert_eq!(decode_datagram(sender, &expected).unwrap(), msg);
}

#[test]
fn s16_1_fingerprint_alpn_connect_string() {
    let key: [u8; 32] = hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
        .try_into()
        .unwrap();
    let spki = ed25519_spki(&key);
    assert_eq!(
        spki.to_vec(),
        hex(
            "30 2a 30 05 06 03 2b 65 70 03 21 00 d7 5a 98 01 82 b1 0a b7 d5 4b fe d3 c9 64 07 3a
             0e e1 72 f3 da a6 23 25 af 02 1a 68 f7 07 51 1a"
        )
    );
    let fp = Fingerprint::of_spki(&spki);
    let text = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";
    assert_eq!(fp.to_string(), text);
    assert_eq!(ALPN_V1, hex("74 65 73 73 65 72 61 2f 31").as_slice());
    let cs = format!("tessera://192.0.2.10:47000?pin={text}");
    let parsed = ConnectString::parse(&cs).unwrap();
    assert_eq!(parsed.pin, Some(fp));
    assert_eq!(parsed.to_string(), cs);
}

#[test]
fn s16_2_control_stream() {
    use Side::{Client as C, Server as S};
    frame(
        Message::Hello(Hello {
            minor_max: 0,
            client_caps: 0,
            max_entities: 1024,
            client_name: "tessera-client 0.7.0".into(),
        }),
        C,
        "20 00 00 00 01 00 00 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
         6e 74 20 30 2e 37 2e 30",
        36,
    );
    frame(
        Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::REGISTRATION | ServerCaps::CHARACTER_CREATION,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Test Shard".into(),
            motd: String::new(),
        }),
        S,
        "19 00 00 00 02 00 00 00 03 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68 61 72 64 00
         00",
        29,
    );
    frame(
        Message::Login(Credentials {
            account: "alice".into(),
            password: "correct horse".into(),
        }),
        C,
        "18 00 00 00 03 00 05 00 61 6c 69 63 65 0d 00 63 6f 72 72 65 63 74 20 68 6f 72 73 65",
        28,
    );
    frame(
        Message::Register(Credentials {
            account: "bob".into(),
            password: "battery staple".into(),
        }),
        C,
        "17 00 00 00 04 00 03 00 62 6f 62 0e 00 62 61 74 74 65 72 79 20 73 74 61 70 6c 65",
        27,
    );
    let aldith = CharacterSummary {
        name: "Aldith".into(),
        body: Body::Character {
            species: 23,
            sex: 1,
        },
    };
    frame(
        Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![aldith.clone()],
            looks: Vec::new(),
        }),
        S,
        "1b 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01
         00 00 00",
        31,
    );
    frame(
        Message::LoginFailed(LoginFailed {
            code: LoginFailedCode::BadCredentials,
            retry_after: 0,
            text: String::new(),
        }),
        S,
        "08 00 00 00 06 00 01 00 00 00 00 00",
        12,
    );
    frame(
        Message::CharacterCreate(CharacterCreate {
            name: "Aldith".into(),
            appearance: Appearance::new(Body::Character {
                species: 23,
                sex: 1,
            }),
            look: tessera_protocol::types::Look::none(),
        }),
        C,
        "19 00 00 00 07 00 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00 00 80 3f 00
         00",
        29,
    );
    frame(
        Message::CharacterList(CharacterList {
            characters: vec![
                aldith,
                CharacterSummary {
                    name: "Bram".into(),
                    body: Body::Character {
                        species: 81,
                        sex: 2,
                    },
                },
            ],
            looks: Vec::new(),
        }),
        S,
        "23 00 00 00 08 00 02 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 04 00 42 72
         61 6d 01 51 00 00 00 02 00 00 00",
        39,
    );
    frame(
        Message::CharacterCreateFailed(Failure {
            code: CharacterCreateFailedCode::NameTaken,
            text: String::new(),
        }),
        S,
        "06 00 00 00 09 00 01 00 00 00",
        10,
    );
    frame(
        Message::EnterWorld(EnterWorld {
            name: "Aldith".into(),
        }),
        C,
        "0a 00 00 00 0a 00 06 00 41 6c 64 69 74 68",
        14,
    );
    frame(
        Message::EnterWorldFailed(Failure {
            code: EnterWorldFailedCode::CharacterBusy,
            text: String::new(),
        }),
        S,
        "06 00 00 00 0b 00 03 00 00 00",
        10,
    );
    frame(
        Message::TeleportRequest(TeleportRequest {
            request_id: 7,
            destination: Destination::Ground {
                location: Location::new(1, 20_000.0, 22_000.0, 0.0),
                heading: 0.0,
            },
        }),
        C,
        "25 00 00 00 0c 00 07 00 00 00 02 01 00 00 00 00 00 00 88 d3 40 00 00 00 00 00 7c d5
         40 00 00 00 00 00 00 00 00 00 00 00 00",
        41,
    );
    frame(
        Message::TeleportRequest(TeleportRequest {
            request_id: 8,
            destination: Destination::Anchored {
                location: l(),
                heading: 1.5,
                anchor: anchor(),
            },
        }),
        C,
        "29 00 00 00 0c 00 08 00 00 00 03 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5
         40 00 00 00 00 00 10 79 40 00 00 c0 3f 7e 8c 01 01",
        45,
    );
    frame(
        Message::TeleportRequest(TeleportRequest {
            request_id: 9,
            destination: Destination::Named {
                name: "origin".into(),
            },
        }),
        C,
        "0f 00 00 00 0c 00 09 00 00 00 04 06 00 6f 72 69 67 69 6e",
        19,
    );
    frame(
        Message::QueryLocation { request_id: 10 },
        C,
        "06 00 00 00 0d 00 0a 00 00 00",
        10,
    );
    frame(
        Message::ChatSend(ChatSend {
            channel: ChatChannel::Say,
            text: "hello".into(),
        }),
        C,
        "0a 00 00 00 0e 00 01 05 00 68 65 6c 6c 6f",
        14,
    );
    frame(
        Message::ChatSend(ChatSend {
            channel: ChatChannel::Tell {
                peer: "Bram".into(),
            },
            text: "hi".into(),
        }),
        C,
        "0d 00 00 00 0e 00 04 04 00 42 72 61 6d 02 00 68 69",
        17,
    );
    frame(
        Message::EmoteSet(EmoteSet {
            anim: 0x1000_0123,
            looping: false,
        }),
        C,
        "07 00 00 00 0f 00 23 01 00 10 00",
        11,
    );
    frame(
        Message::StanceSet(Stance::Ready),
        C,
        "03 00 00 00 10 00 01",
        7,
    );
    frame(
        Message::AppearanceSet(a()),
        C,
        "1f 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02 34 12 00 70 01 00 00
         80 3e 78 56 00 70 04",
        35,
    );
    for sender in [C, S] {
        frame(
            Message::Ping {
                nonce: 0x0123_4567_89ab_cdef,
            },
            sender,
            "0a 00 00 00 12 00 ef cd ab 89 67 45 23 01",
            14,
        );
        frame(
            Message::Pong {
                nonce: 0x0123_4567_89ab_cdef,
            },
            sender,
            "0a 00 00 00 13 00 ef cd ab 89 67 45 23 01",
            14,
        );
        frame(
            Message::Disconnect(Disconnect {
                code: AppErrorCode::NoError,
                text: String::new(),
            }),
            sender,
            "06 00 00 00 14 00 00 00 00 00",
            10,
        );
    }
}

#[test]
fn s16_3_world_stream() {
    let s = Side::Server;
    frame(
        Message::WorldEnter(WorldEnter {
            player: 42,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: 5_000_000,
            pose: Pose {
                location: l(),
                heading: 0.0,
                anchor: None,
            },
            appearance: a(),
            look: tessera_protocol::types::Look::none(),
        }),
        s,
        "5e 00 00 00 01 01 2a 00 00 00 00 00 00 00 06 00 41 6c 64 69 74 68 01 00 00 00 01 00
         00 00 40 4b 4c 00 00 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
         00 00 00 00 00 10 79 40 00 00 00 00 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02
         34 12 00 70 01 00 00 80 3e 78 56 00 70 04",
        98,
    );
    frame(
        Message::LocationReport(LocationReport {
            cause: LocationCause::Teleported,
            request_id: 7,
            epoch: 2,
            server_time: 6_000_000,
            pose: Pose {
                location: l(),
                heading: 1.5,
                anchor: Some(anchor()),
            },
        }),
        s,
        "36 00 00 00 02 01 01 07 00 00 00 02 00 00 00 80 8d 5b 00 00 00 00 00 01 00 00 00 00
         00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40 00 00 00 00 00 10 79 40 00 00 c0 3f 01 7e 8c
         01 01",
        58,
    );
    frame(
        Message::TeleportRefused(TeleportRefused {
            request_id: 9,
            code: TeleportRefusedCode::UnknownPlace,
            text: String::new(),
        }),
        s,
        "0a 00 00 00 03 01 09 00 00 00 01 00 00 00",
        14,
    );
    frame(
        Message::EntityCreate(EntityCreate {
            id: 43,
            kind: EntityKind::Player,
            name: "Bram".into(),
            class: None,
            server_time: 6_000_000,
            pose: Pose {
                location: Location::new(1, 20_260.0, 22_500.0, 400.5),
                heading: 3.0,
                anchor: None,
            },
            velocity: Vec3::new(2.0, 0.0, 0.0),
            motion: Motion::new(Gait::Walking, false),
            appearance: Appearance::new(Body::Character {
                species: 81,
                sex: 2,
            }),
            properties: vec![
                Property::new(pid::STANCE, Value::Enum(0)),
                Property::new(pid::RUN_SPEED, Value::Float32(6.0)),
            ],
            look: tessera_protocol::types::Look::none(),
        }),
        s,
        "65 00 00 00 04 01 2b 00 00 00 00 00 00 00 00 04 00 42 72 61 6d 00 80 8d 5b 00 00 00
         00 00 01 00 00 00 00 00 00 c9 d3 40 00 00 00 00 00 f9 d5 40 00 00 00 00 00 08 79 40
         00 00 40 40 00 00 00 00 40 00 00 00 00 00 00 00 00 01 01 51 00 00 00 02 00 00 00 00
         00 80 3f 00 00 02 00 01 00 05 00 00 00 00 04 00 04 00 00 c0 40",
        105,
    );
    frame(
        Message::EntityUpdate(EntityUpdate {
            id: 43,
            changes: vec![
                Property::new(pid::STANCE, Value::Enum(2)),
                Property::new(
                    pid::EMOTE,
                    Value::Struct(vec![
                        (1, Value::Enum(0x1000_0123)),
                        (2, Value::Time(6_500_000)),
                        (3, Value::Bool(false)),
                    ]),
                ),
            ],
        }),
        s,
        "2e 00 00 00 05 01 2b 00 00 00 00 00 00 00 02 00 01 00 05 02 00 00 00 02 00 0f 03 00
         01 00 05 23 01 00 10 02 00 0c a0 2e 63 00 00 00 00 00 03 00 01 00",
        50,
    );
    frame(
        Message::EntityUpdate(EntityUpdate {
            id: 43,
            changes: vec![Property::new(pid::EMOTE, Value::None)],
        }),
        s,
        "0f 00 00 00 05 01 2b 00 00 00 00 00 00 00 01 00 02 00 00",
        19,
    );
    frame(
        Message::EntityAppearance(EntityAppearance {
            id: 43,
            appearance: Appearance {
                body: Body::Object { did: 0x7900_0001 },
                scale: 2.5,
                idle: Some(0x1000_0001),
                equipment: vec![],
            },
            look: tessera_protocol::types::Look::none(),
        }),
        s,
        "19 00 00 00 06 01 2b 00 00 00 00 00 00 00 02 01 00 00 79 00 00 20 40 01 01 00 00 10
         00",
        29,
    );
    frame(
        Message::EntityDestroy(EntityDestroy {
            id: 43,
            reason: DestroyReason::LeftInterest,
        }),
        s,
        "0b 00 00 00 07 01 2b 00 00 00 00 00 00 00 00",
        15,
    );
    frame(Message::InterestReset, s, "02 00 00 00 08 01", 6);
    frame(
        Message::WorldClock(WorldClock {
            server_time: 5_000_000,
            game_clock: 43_200.0,
            rate: 1.0,
        }),
        s,
        "16 00 00 00 09 01 40 4b 4c 00 00 00 00 00 00 00 00 00 00 18 e5 40 00 00 80 3f",
        26,
    );
}

#[test]
fn s16_4_chat_stream() {
    let s = Side::Server;
    frame(
        Message::ChatMessage(ChatMessage {
            server_time: 7_000_000,
            channel: ChatChannel::Say,
            echo: false,
            sender_entity: 43,
            sender: "Bram".into(),
            text: "hello".into(),
        }),
        s,
        "21 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 01 00 2b 00 00 00 00 00 00 00 04 00 42 72
         61 6d 05 00 68 65 6c 6c 6f",
        37,
    );
    frame(
        Message::ChatMessage(ChatMessage {
            server_time: 7_000_000,
            channel: ChatChannel::Tell {
                peer: "Bram".into(),
            },
            echo: true,
            sender_entity: 42,
            sender: "Aldith".into(),
            text: "hi".into(),
        }),
        s,
        "26 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 04 01 2a 00 00 00 00 00 00 00 06 00 41 6c
         64 69 74 68 04 00 42 72 61 6d 02 00 68 69",
        42,
    );
    frame(
        Message::ChatMessage(ChatMessage {
            server_time: 7_000_000,
            channel: ChatChannel::System,
            echo: false,
            sender_entity: 0,
            sender: String::new(),
            text: "Welcome".into(),
        }),
        s,
        "1f 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 05 00 00 00 00 00 00 00 00 00 00 00 07 00
         57 65 6c 63 6f 6d 65",
        35,
    );
    frame(
        Message::ServerNotice(ServerNotice {
            level: NoticeLevel::Shutdown,
            seconds: 60,
            text: "restart".into(),
        }),
        s,
        "0e 00 00 00 02 02 02 3c 00 07 00 72 65 73 74 61 72 74",
        18,
    );
    frame(
        Message::ChatRejected(Failure {
            code: ChatRejectedCode::RateLimited,
            text: String::new(),
        }),
        s,
        "06 00 00 00 03 02 01 00 00 00",
        10,
    );
}

#[test]
fn s16_5_datagrams() {
    datagram(
        Message::MoveInput(MoveInput {
            seq: 100,
            epoch: 2,
            location: l(),
            heading: 1.5,
            velocity: Vec3::new(0.0, 6.0, 0.0),
            motion: Motion::new(Gait::Running, false),
        }),
        Side::Client,
        "01 03 64 00 00 00 02 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
         00 00 00 00 00 10 79 40 00 00 c0 3f 00 00 00 00 00 00 c0 40 00 00 00 00 02",
        53,
    );
    datagram(
        Message::MotionBatch(MotionBatch {
            server_time: 6_066_666,
            entries: vec![MotionEntry {
                id: 43,
                position: Vec3::new(20_260.0, 22_500.0, 400.5),
                heading: 3.0,
                velocity: Vec3::new(-2.0, 0.0, 0.0),
                motion: Motion::new(Gait::Walking, false),
            }],
        }),
        Side::Server,
        "02 03 ea 91 5c 00 00 00 00 00 01 2b 00 00 00 00 00 00 00 00 48 9e 46 00 c8 af 46 00
         40 c8 43 00 00 40 40 00 00 00 c0 00 00 00 00 00 00 00 00 01",
        48,
    );
    datagram(
        Message::TimeRequest {
            client_time: 1_000_000,
        },
        Side::Client,
        "03 03 40 42 0f 00 00 00 00 00",
        10,
    );
    datagram(
        Message::TimeReply(TimeReply {
            client_time: 1_000_000,
            server_receive: 5_020_000,
            server_send: 5_020_100,
            tick: 150,
        }),
        Side::Server,
        "04 03 40 42 0f 00 00 00 00 00 60 99 4c 00 00 00 00 00 c4 99 4c 00 00 00 00 00 96 00
         00 00 00 00 00 00",
        34,
    );
    let s = TimeSample::compute(1_000_000, 5_020_000, 5_020_100, 1_040_100);
    assert_eq!((s.offset, s.delay), (4_000_000, 40_000));
}

#[test]
fn s16_6_tagged_values() {
    let cases: Vec<(Value, &str)> = vec![
        (Value::Bool(true), "01 01"),
        (Value::Int32(-2), "02 fe ff ff ff"),
        (Value::Int64(-2), "03 fe ff ff ff ff ff ff ff"),
        (Value::Float32(6.0), "04 00 00 c0 40"),
        (Value::Enum(2), "05 02 00 00 00"),
        (Value::DataId(0x7000_1234), "06 34 12 00 70"),
        (Value::String("ab".into()), "07 02 00 61 62"),
        (
            Value::Vector3(Vec3::new(1.0, 2.0, 3.0)),
            "08 00 00 80 3f 00 00 00 40 00 00 40 40",
        ),
        (Value::Bitfield32(0x8000_0001), "09 01 00 00 80"),
        (Value::Bitfield64(1 << 40), "0a 00 00 00 00 00 01 00 00"),
        (Value::EntityRef(43), "0b 2b 00 00 00 00 00 00 00"),
        (Value::Time(6_500_000), "0c a0 2e 63 00 00 00 00 00"),
        (Value::Colour([255, 128, 0, 255]), "0d ff 80 00 ff"),
        (
            Value::Array(vec![Value::Int32(1), Value::Int32(2)]),
            "0e 02 00 02 01 00 00 00 02 02 00 00 00",
        ),
        (
            Value::Struct(vec![(1, Value::Bool(false))]),
            "0f 01 00 01 00 01 00",
        ),
        (Value::None, "00"),
    ];
    for (v, h) in cases {
        let bytes = hex(h);
        let mut out = Vec::new();
        v.encode(&mut out, true).unwrap();
        assert_eq!(out, bytes, "{v:?}");
        assert_eq!(Value::decode(&bytes, true).unwrap(), (v, bytes.len()));
    }
}

fn decode_control(sender: Side, bytes: &[u8]) -> Result<Option<Message>, ProtocolError> {
    let mut d = FrameDecoder::new(Channel::Control, sender);
    let mut input = bytes;
    d.push(&mut input)
}

#[test]
fn s16_7_negative_vectors() {
    // Frame length 1.
    assert_eq!(
        decode_control(Side::Client, &hex("01 00 00 00 01")),
        Err(ProtocolError::FrameLength(1))
    );
    // Frame length 65,537, rejected from the header alone.
    assert_eq!(
        decode_control(Side::Client, &hex("01 00 01 00")),
        Err(ProtocolError::FrameLength(65_537))
    );
    // StanceSet with body 03.
    assert!(matches!(
        decode_control(Side::Client, &hex("03 00 00 00 10 00 03")),
        Err(ProtocolError::InvalidEnum { .. })
    ));
    // Hello with one extra trailing 00 and length + 1.
    let mut hello = hex(
        "21 00 00 00 01 00 00 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
         6e 74 20 30 2e 37 2e 30",
    );
    hello.push(0);
    assert_eq!(
        decode_control(Side::Client, &hello),
        Err(ProtocolError::TrailingBytes(1))
    );
    // MotionBatch with count 0.
    assert!(matches!(
        decode_datagram(Side::Server, &hex("02 03 00 00 00 00 00 00 00 00 00")),
        Err(ProtocolError::Count { .. })
    ));
    // EntityUpdate whose PIDs go 2 then 1.
    let mut d = FrameDecoder::new(Channel::World, Side::Server);
    let bad =
        hex("13 00 00 00 05 01 2b 00 00 00 00 00 00 00 02 00 02 00 0f 00 00 01 00 05 00 00 00 00");
    let mut fixed = bad.clone();
    fixed[0] = (bad.len() - 4) as u8;
    let mut input: &[u8] = &fixed;
    assert_eq!(
        d.push(&mut input),
        Err(ProtocolError::IdOrder("properties"))
    );
    // MoveInput with x = NaN at offset 12.
    let mut mv = hex(
        "01 03 64 00 00 00 02 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
         00 00 00 00 00 10 79 40 00 00 c0 3f 00 00 00 00 00 00 c0 40 00 00 00 00 02",
    );
    mv[12..20].copy_from_slice(&hex("00 00 00 00 00 00 f8 7f"));
    assert_eq!(
        decode_datagram(Side::Client, &mv),
        Err(ProtocolError::NonFinite("location.x"))
    );
    // ChatSend with channel 5.
    assert!(matches!(
        decode_control(
            Side::Client,
            &hex("0a 00 00 00 0e 00 05 05 00 68 65 6c 6c 6f")
        ),
        Err(ProtocolError::InvalidEnum { .. })
    ));
    // Uni stream kind byte 58.
    assert_eq!(
        StreamKind::from_byte(0x58),
        Err(ProtocolError::UnknownStreamKind(0x58))
    );
}

// --- §16.8: minor 1 (v1.1) ---

/// Look K of §16.8.
fn look_k() -> Look {
    Look {
        entries: vec![
            LookEntry {
                property: 0x1000_0A01,
                value: LookValue::Float32(0.5),
            },
            LookEntry {
                property: 0x1000_0A02,
                value: LookValue::Colour([200, 150, 100, 255]),
            },
            LookEntry {
                property: 0x1000_0A03,
                value: LookValue::String("8080ff".into()),
            },
        ],
    }
}

/// [`frame`] at negotiated minor 1.
fn frame_v11(msg: Message, sender: Side, expected_hex: &str, len: usize) {
    let expected = hex(expected_hex);
    assert_eq!(
        expected.len(),
        len,
        "{:?}: vector length",
        msg.message_type()
    );
    let encoded = frame_to_vec_minor(&msg, 1).unwrap();
    assert_eq!(encoded, expected, "{:?}: encoding", msg.message_type());
    let length = u32::from_le_bytes(expected[..4].try_into().unwrap()) as usize;
    assert_eq!(length + 4, expected.len());
    let mut d = FrameDecoder::with_minor(msg.channel(), sender, 1);
    let mut input: &[u8] = &expected;
    assert_eq!(d.push(&mut input).unwrap(), Some(msg.clone()));
    assert!(input.is_empty());
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

const CREATE_K: &str =
    "39 00 00 00 07 00 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00 00 80 3f 00
    00 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8 96 64 ff 03 0a 00 10 07 06 00 38
    30 38 30 66 66";

const APPEARANCE_SET_SLOTS: &str =
    "21 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02 34 12 00 70 09 00 00
    80 3e 11 78 56 00 70 0c 12";

fn aldith() -> CharacterSummary {
    CharacterSummary {
        name: "Aldith".into(),
        body: Body::Character {
            species: 23,
            sex: 1,
        },
    }
}

#[test]
fn s16_8_minor_1() {
    frame_v11(
        Message::Hello(Hello {
            minor_max: 1,
            client_caps: 0,
            max_entities: 1024,
            client_name: "tessera-client 0.8.0".into(),
        }),
        Side::Client,
        "20 00 00 00 01 00 01 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
         6e 74 20 30 2e 38 2e 30",
        36,
    );
    frame_v11(
        Message::HelloAck(HelloAck {
            minor: 1,
            server_caps: ServerCaps(
                ServerCaps::REGISTRATION.0
                    | ServerCaps::CHARACTER_CREATION.0
                    | ServerCaps::APPEARANCE_CHANGE.0,
            ),
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "Test Shard".into(),
            motd: String::new(),
        }),
        Side::Server,
        "19 00 00 00 02 00 01 00 07 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68 61 72 64 00
         00",
        29,
    );
    assert_eq!(look_k().encoded_len(), 32);
    frame_v11(
        Message::CharacterCreate(CharacterCreate {
            name: "Aldith".into(),
            appearance: Appearance::new(Body::Character {
                species: 23,
                sex: 1,
            }),
            look: look_k(),
        }),
        Side::Client,
        CREATE_K,
        61,
    );
    frame_v11(
        Message::LoginOk(LoginOk {
            account: "alice".into(),
            characters: vec![aldith()],
            looks: vec![look_k()],
        }),
        Side::Server,
        "3c 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01
         00 00 00 01 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8 96 64 ff 03 0a 00 10 07
         06 00 38 30 38 30 66 66",
        64,
    );
    frame_v11(
        Message::CharacterList(CharacterList {
            characters: vec![
                aldith(),
                CharacterSummary {
                    name: "Bram".into(),
                    body: Body::Character {
                        species: 81,
                        sex: 2,
                    },
                },
            ],
            looks: vec![look_k(), Look::none()],
        }),
        Side::Server,
        "45 00 00 00 08 00 02 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 04 00 42 72
         61 6d 01 51 00 00 00 02 00 00 00 02 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8
         96 64 ff 03 0a 00 10 07 06 00 38 30 38 30 66 66 00",
        73,
    );
    let mut set = Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    });
    set.equipment = vec![
        Equipped {
            item: 0x7000_1234,
            dye: Some(0.25),
            secondary_dye: None,
            stowed: false,
            slot: Some(17),
        },
        Equipped {
            item: 0x7000_5678,
            dye: None,
            secondary_dye: None,
            stowed: true,
            slot: Some(18),
        },
    ];
    frame_v11(
        Message::AppearanceSet(set.clone()),
        Side::Client,
        APPEARANCE_SET_SLOTS,
        37,
    );
    let mut observed = Appearance::new(Body::Character {
        species: 81,
        sex: 2,
    });
    observed.equipment = vec![Equipped {
        item: 0x7000_1234,
        slot: Some(17),
        ..Equipped::default()
    }];
    frame_v11(
        Message::EntityAppearance(EntityAppearance {
            id: 43,
            appearance: observed,
            look: Look::none(),
        }),
        Side::Server,
        "20 00 00 00 06 01 2b 00 00 00 00 00 00 00 01 51 00 00 00 02 00 00 00 00 00 80 3f 00
         01 34 12 00 70 08 11 00",
        36,
    );
    // At minor 0 the same messages encode as v1.0: no slot bit, no appended look.
    let v10 = frame_to_vec_minor(&Message::AppearanceSet(set), 0).unwrap();
    assert_eq!(v10, frame_to_vec(&Message::AppearanceSet(a())).unwrap());
}

/// Replaces the look of the `CharacterCreate` vector (and fixes the frame length).
fn create_with_look(look_hex: &str) -> Vec<u8> {
    let base = hex(CREATE_K);
    // The look starts after the 61 − 32 = 29 bytes of length, type, name and appearance.
    let mut out = base[..29].to_vec();
    out.extend(hex(look_hex));
    let len = u32::try_from(out.len() - 4).unwrap();
    out[..4].copy_from_slice(&len.to_le_bytes());
    out
}

fn appearance_set_with_first_flags(flags: u8, slot: Option<u8>) -> Vec<u8> {
    // AppearanceSet with one item: 0x70001234, `flags`, optional slot byte.
    let mut out = hex("00 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 01 34 12 00 70");
    out.push(flags);
    out.extend(slot);
    let len = u32::try_from(out.len() - 4).unwrap();
    out[..4].copy_from_slice(&len.to_le_bytes());
    out
}

#[test]
fn s16_8_negative_vectors() {
    let c = Channel::Control;
    // The AppearanceSet above at minor 0: flag bit 3.
    assert_eq!(
        decode_at(c, Side::Client, 0, &hex(APPEARANCE_SET_SLOTS)),
        Err(ProtocolError::ReservedBits("equipped.flags"))
    );
    // The CharacterCreate above at minor 0: 32 bytes left.
    assert_eq!(
        decode_at(c, Side::Client, 0, &hex(CREATE_K)),
        Err(ProtocolError::TrailingBytes(32))
    );
    // Slot 00 and 41 (65).
    for slot in [0x00, 0x41] {
        assert_eq!(
            decode_at(
                c,
                Side::Client,
                1,
                &appearance_set_with_first_flags(0x08, Some(slot))
            ),
            Err(ProtocolError::OutOfRange("equipped.slot"))
        );
    }
    // Slots 1 and 64 are fine.
    for slot in [1, 64] {
        assert!(
            decode_at(
                c,
                Side::Client,
                1,
                &appearance_set_with_first_flags(0x08, Some(slot))
            )
            .is_ok()
        );
    }
    // Flag bit 4.
    assert_eq!(
        decode_at(
            c,
            Side::Client,
            1,
            &appearance_set_with_first_flags(0x10, None)
        ),
        Err(ProtocolError::ReservedBits("equipped.flags"))
    );
    // Property IDs not ascending; property ID 0.
    assert_eq!(
        decode_at(
            c,
            Side::Client,
            1,
            &create_with_look("02 02 0a 00 10 01 01 01 0a 00 10 01 00")
        ),
        Err(ProtocolError::IdOrder("look"))
    );
    assert_eq!(
        decode_at(
            c,
            Side::Client,
            1,
            &create_with_look("01 00 00 00 00 01 01")
        ),
        Err(ProtocolError::IdOrder("look"))
    );
    // Array tag; space in a string.
    assert_eq!(
        decode_at(
            c,
            Side::Client,
            1,
            &create_with_look("01 01 0a 00 10 0e 00 00")
        ),
        Err(ProtocolError::LookTag(0x0E))
    );
    assert_eq!(
        decode_at(
            c,
            Side::Client,
            1,
            &create_with_look("01 01 0a 00 10 07 03 00 61 20 62")
        ),
        Err(ProtocolError::LookString)
    );
    // 49 Bool entries.
    let mut l49 = String::from("31");
    for i in 1..=49u32 {
        for b in i.to_le_bytes() {
            l49.push_str(&format!(" {b:02x}"));
        }
        l49.push_str(" 01 01");
    }
    assert!(matches!(
        decode_at(c, Side::Client, 1, &create_with_look(&l49)),
        Err(ProtocolError::Count {
            field: "look",
            count: 49,
            ..
        })
    ));
    // Six String entries of 128 bytes: 811 bytes.
    let mut big = String::from("06");
    for i in 1..=6u32 {
        for b in i.to_le_bytes() {
            big.push_str(&format!(" {b:02x}"));
        }
        big.push_str(" 07 80 00");
        for _ in 0..128 {
            big.push_str(" 41");
        }
    }
    assert_eq!(
        decode_at(c, Side::Client, 1, &create_with_look(&big)),
        Err(ProtocolError::LookTooLarge(811))
    );
    // LoginOk with 1 character and looks count 0.
    let mut ok = hex(
        "00 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00",
    );
    let len = u32::try_from(ok.len() - 4).unwrap();
    ok[..4].copy_from_slice(&len.to_le_bytes());
    assert!(matches!(
        decode_at(c, Side::Server, 1, &ok),
        Err(ProtocolError::Count { field: "looks", .. })
    ));
    // The encoder refuses the same values.
    let mut bad = look_k();
    bad.entries.swap(0, 1);
    assert_eq!(bad.check(), Err(ProtocolError::IdOrder("look")));
    let mut bad = look_k();
    bad.entries[2].value = LookValue::String("a b".into());
    assert_eq!(bad.check(), Err(ProtocolError::LookString));
    let mut bad = look_k();
    bad.entries[0].value = LookValue::Float32(f32::NAN);
    assert_eq!(bad.check(), Err(ProtocolError::NonFinite("look.value")));
    let mut slot0 = Appearance::new(Body::Character { species: 1, sex: 1 });
    slot0.equipment.push(Equipped {
        item: 1,
        slot: Some(0),
        ..Equipped::default()
    });
    assert!(frame_to_vec_minor(&Message::AppearanceSet(slot0.clone()), 1).is_err());
    // At minor 0 the slot is simply not sent.
    assert!(frame_to_vec_minor(&Message::AppearanceSet(slot0), 0).is_ok());
}
