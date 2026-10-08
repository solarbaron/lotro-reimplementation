// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests: every valid message round-trips byte-exactly (§15 invariant 2), and
//! arbitrary or mutated bytes never panic any decoder or parser (§15 invariant 3).

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::strategy::Union;

use tessera_protocol::codes::*;
use tessera_protocol::connect::{ConnectString, KnownServers, ServerKey};
use tessera_protocol::fingerprint::{Fingerprint, certificate_spki};
use tessera_protocol::frame::{
    FrameDecoder, decode_datagram, decode_frame_message, encode_datagram, frame_to_vec, split_frame,
};
use tessera_protocol::message::*;
use tessera_protocol::types::*;
use tessera_protocol::value::{Property, Stance, Value, pid};
use tessera_protocol::{Channel, Message, Side};

// --- strategies ---

fn text(max: usize) -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-zA-Z0-9 .,!?éüß語-]{{0,{max}}}"))
        .unwrap()
        .prop_map(move |s| {
            let mut s = s;
            while s.len() > max {
                s.pop();
            }
            s
        })
}

fn text1(max: usize) -> impl Strategy<Value = String> {
    text(max).prop_map(|s| if s.is_empty() { "x".to_owned() } else { s })
}

fn coord() -> impl Strategy<Value = f64> {
    prop_oneof![
        -1.0e7..=1.0e7f64,
        Just(0.0),
        Just(-0.0),
        Just(1.0e7),
        Just(-1.0e7)
    ]
}

fn finite32() -> impl Strategy<Value = f32> {
    prop_oneof![
        -1.0e6f32..1.0e6,
        Just(-0.0f32),
        Just(f32::MAX),
        Just(f32::MIN_POSITIVE)
    ]
}

fn location() -> impl Strategy<Value = Location> {
    (any::<u16>(), coord(), coord(), coord()).prop_map(|(r, x, y, z)| Location::new(r, x, y, z))
}

fn anchored() -> impl Strategy<Value = (Location, Anchor)> {
    (
        any::<u16>(),
        0u8..=254,
        0u8..=254,
        -1280.0..=1280.0f64,
        -1280.0..=1280.0f64,
        coord(),
        any::<u16>(),
    )
        .prop_map(|(r, bx, by, dx, dy, z, cell)| {
            (
                Location::new(r, 160.0 * f64::from(bx) + dx, 160.0 * f64::from(by) + dy, z),
                Anchor::new(bx, by, cell),
            )
        })
}

fn pose() -> impl Strategy<Value = Pose> {
    prop_oneof![
        (location(), finite32()).prop_map(|(location, heading)| Pose {
            location,
            heading,
            anchor: None
        }),
        (anchored(), finite32()).prop_map(|((location, a), heading)| Pose {
            location,
            heading,
            anchor: Some(a)
        }),
    ]
}

fn vec3() -> impl Strategy<Value = Vec3> {
    (finite32(), finite32(), finite32()).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

fn body() -> impl Strategy<Value = Body> {
    prop_oneof![
        (any::<u32>(), any::<u32>()).prop_map(|(species, sex)| Body::Character { species, sex }),
        (1u32..).prop_map(|did| Body::Object { did }),
    ]
}

fn dye() -> impl Strategy<Value = Option<f32>> {
    proptest::option::of(prop_oneof![0.0f32..=1.0, Just(0.0), Just(1.0)])
}

fn appearance() -> impl Strategy<Value = Appearance> {
    let equipped =
        (1u32.., dye(), dye(), any::<bool>()).prop_map(|(item, d, s, stowed)| Equipped {
            item,
            dye: d,
            secondary_dye: s,
            stowed,
        });
    (
        body(),
        prop_oneof![0.001f32..=100.0, Just(100.0f32), Just(f32::MIN_POSITIVE)],
        proptest::option::of(any::<u32>()),
        vec(equipped, 0..=32),
    )
        .prop_map(|(body, scale, idle, equipment)| Appearance {
            body,
            scale,
            idle,
            equipment,
        })
}

fn motion() -> impl Strategy<Value = Motion> {
    (0u8..4, any::<bool>()).prop_map(|(g, b)| Motion::from_byte(g | u8::from(b) << 3).unwrap())
}

fn leaf_of(kind: u8) -> BoxedStrategy<Value> {
    match kind {
        0 => any::<bool>().prop_map(Value::Bool).boxed(),
        1 => any::<i32>().prop_map(Value::Int32).boxed(),
        2 => any::<i64>().prop_map(Value::Int64).boxed(),
        3 => finite32().prop_map(Value::Float32).boxed(),
        4 => any::<u32>().prop_map(Value::Enum).boxed(),
        5 => any::<u32>().prop_map(Value::DataId).boxed(),
        6 => text(40).prop_map(Value::String).boxed(),
        7 => vec3().prop_map(Value::Vector3).boxed(),
        8 => any::<u32>().prop_map(Value::Bitfield32).boxed(),
        9 => any::<u64>().prop_map(Value::Bitfield64).boxed(),
        10 => any::<u64>().prop_map(Value::EntityRef).boxed(),
        11 => any::<u64>().prop_map(Value::Time).boxed(),
        _ => any::<[u8; 4]>().prop_map(Value::Colour).boxed(),
    }
}

fn leaf() -> BoxedStrategy<Value> {
    (0u8..13).prop_flat_map(leaf_of).boxed()
}

fn value() -> impl Strategy<Value = Value> {
    leaf().prop_recursive(3, 48, 6, |inner| {
        prop_oneof![
            (0u8..13).prop_flat_map(|k| vec(leaf_of(k), 0..6).prop_map(Value::Array)),
            vec(inner.clone(), 1..4).prop_map(|items| {
                // Arrays of containers: keep the first element's tag only.
                let tag = items[0].tag();
                Value::Array(items.into_iter().filter(|v| v.tag() == tag).collect())
            }),
            vec((1u16.., inner), 0..5).prop_map(|mut fields| {
                fields.sort_by_key(|f| f.0);
                fields.dedup_by_key(|f| f.0);
                Value::Struct(fields)
            }),
        ]
    })
}

fn props(allow_none: bool, min: usize) -> impl Strategy<Value = Vec<Property>> {
    let entry = (5u16.., value(), any::<bool>()).prop_map(move |(id, v, remove)| {
        Property::new(id, if allow_none && remove { Value::None } else { v })
    });
    (
        vec(entry, min..6),
        proptest::option::of(0u32..3),
        proptest::option::of(0.5f32..100.0),
    )
        .prop_map(move |(mut list, stance, run)| {
            if let Some(s) = stance {
                list.push(Property::new(pid::STANCE, Value::Enum(s)));
            }
            if let Some(r) = run {
                list.push(Property::new(pid::RUN_SPEED, Value::Float32(r)));
            }
            list.sort_by_key(|p| p.id);
            list.dedup_by_key(|p| p.id);
            list
        })
}

fn characters() -> impl Strategy<Value = Vec<CharacterSummary>> {
    vec((text1(64), body()), 0..8).prop_map(|list| {
        let mut out: Vec<CharacterSummary> = Vec::new();
        for (name, body) in list {
            if !out.iter().any(|c| c.name == name) {
                out.push(CharacterSummary { name, body });
            }
        }
        out
    })
}

fn destination() -> impl Strategy<Value = Destination> {
    prop_oneof![
        (location(), finite32())
            .prop_map(|(location, heading)| Destination::Location { location, heading }),
        (location(), finite32())
            .prop_map(|(location, heading)| Destination::Ground { location, heading }),
        (anchored(), finite32()).prop_map(|((location, anchor), heading)| {
            Destination::Anchored {
                location,
                heading,
                anchor,
            }
        }),
        text(256).prop_map(|name| Destination::Named { name }),
    ]
}

fn chat_channel(system: bool) -> impl Strategy<Value = ChatChannel> {
    let max = if system { 5u8 } else { 4 };
    (1..=max, text1(64)).prop_map(|(c, peer)| match c {
        1 => ChatChannel::Say,
        2 => ChatChannel::Yell,
        3 => ChatChannel::Emote,
        4 => ChatChannel::Tell { peer },
        _ => ChatChannel::System,
    })
}

fn messages() -> impl Strategy<Value = Message> {
    let s: Vec<BoxedStrategy<Message>> = vec![
        (any::<u16>(), any::<u32>(), 64u16.., text(64))
            .prop_map(|(minor_max, client_caps, max_entities, client_name)| {
                Message::Hello(Hello {
                    minor_max,
                    client_caps,
                    max_entities,
                    client_name,
                })
            })
            .boxed(),
        (
            any::<u16>(),
            any::<u32>(),
            1u8..=120,
            any::<u8>(),
            0u8..=8,
            text(64),
            text(1024),
        )
            .prop_map(|(minor, caps, tick, snap, r, server_name, motd)| {
                Message::HelloAck(HelloAck {
                    minor,
                    server_caps: ServerCaps(caps),
                    tick_rate: tick,
                    snapshot_rate: 1 + snap % tick,
                    interest_radius: r,
                    server_name,
                    motd,
                })
            })
            .boxed(),
        (text(32), text(256), any::<bool>())
            .prop_map(|(account, password, reg)| {
                let c = Credentials { account, password };
                if reg {
                    Message::Register(c)
                } else {
                    Message::Login(c)
                }
            })
            .boxed(),
        (text1(32), characters())
            .prop_map(|(account, characters)| {
                Message::LoginOk(LoginOk {
                    account,
                    characters,
                })
            })
            .boxed(),
        (any::<u16>(), any::<u16>(), text(256))
            .prop_map(|(c, retry_after, text)| {
                Message::LoginFailed(LoginFailed {
                    code: LoginFailedCode::from_u16(c),
                    retry_after,
                    text,
                })
            })
            .boxed(),
        (text(64), appearance())
            .prop_map(|(name, appearance)| {
                Message::CharacterCreate(CharacterCreate { name, appearance })
            })
            .boxed(),
        characters()
            .prop_map(|characters| Message::CharacterList(CharacterList { characters }))
            .boxed(),
        (any::<u16>(), text(256))
            .prop_map(|(c, text)| {
                Message::CharacterCreateFailed(Failure {
                    code: CharacterCreateFailedCode::from_u16(c),
                    text,
                })
            })
            .boxed(),
        text(64)
            .prop_map(|name| Message::EnterWorld(EnterWorld { name }))
            .boxed(),
        (any::<u16>(), text(256))
            .prop_map(|(c, text)| {
                Message::EnterWorldFailed(Failure {
                    code: EnterWorldFailedCode::from_u16(c),
                    text,
                })
            })
            .boxed(),
        (1u32.., destination())
            .prop_map(|(request_id, destination)| {
                Message::TeleportRequest(TeleportRequest {
                    request_id,
                    destination,
                })
            })
            .boxed(),
        (1u32..)
            .prop_map(|request_id| Message::QueryLocation { request_id })
            .boxed(),
        (chat_channel(false), text(1024))
            .prop_map(|(channel, text)| Message::ChatSend(ChatSend { channel, text }))
            .boxed(),
        (any::<u32>(), any::<bool>())
            .prop_map(|(anim, looping)| Message::EmoteSet(EmoteSet { anim, looping }))
            .boxed(),
        (0u8..3)
            .prop_map(|s| Message::StanceSet(Stance::from_u8(s).unwrap()))
            .boxed(),
        appearance().prop_map(Message::AppearanceSet).boxed(),
        any::<u64>()
            .prop_map(|nonce| Message::Ping { nonce })
            .boxed(),
        any::<u64>()
            .prop_map(|nonce| Message::Pong { nonce })
            .boxed(),
        (any::<u16>(), text(1024))
            .prop_map(|(c, text)| {
                Message::Disconnect(Disconnect {
                    code: AppErrorCode::from_u16(c),
                    text,
                })
            })
            .boxed(),
        (
            1u64..,
            text1(64),
            any::<u32>(),
            1u32..,
            any::<u64>(),
            pose(),
            appearance(),
        )
            .prop_map(
                |(player, name, instance, epoch, server_time, pose, appearance)| {
                    Message::WorldEnter(WorldEnter {
                        player,
                        name,
                        instance,
                        epoch,
                        server_time,
                        pose,
                        appearance,
                    })
                },
            )
            .boxed(),
        (1u8..=3, any::<u32>(), any::<u32>(), any::<u64>(), pose())
            .prop_map(|(c, request_id, epoch, server_time, pose)| {
                let cause = match c {
                    1 => LocationCause::Teleported,
                    2 => LocationCause::Requested,
                    _ => LocationCause::Corrected,
                };
                Message::LocationReport(LocationReport {
                    cause,
                    request_id: if cause == LocationCause::Corrected {
                        0
                    } else {
                        request_id
                    },
                    epoch,
                    server_time,
                    pose,
                })
            })
            .boxed(),
        (any::<u32>(), any::<u16>(), text(256))
            .prop_map(|(request_id, c, text)| {
                Message::TeleportRefused(TeleportRefused {
                    request_id,
                    code: TeleportRefusedCode::from_u16(c),
                    text,
                })
            })
            .boxed(),
        (
            (
                1u64..,
                0u8..4,
                text(64),
                proptest::option::of(any::<u32>()),
                any::<u64>(),
            ),
            (pose(), vec3(), motion(), appearance(), props(false, 0)),
        )
            .prop_map(
                |(
                    (id, k, name, class, server_time),
                    (pose, velocity, motion, appearance, properties),
                )| {
                    let kind = match k {
                        0 => EntityKind::Player,
                        1 => EntityKind::NonPlayer,
                        2 => EntityKind::Creature,
                        _ => EntityKind::Object,
                    };
                    Message::EntityCreate(EntityCreate {
                        id,
                        kind,
                        name,
                        class,
                        server_time,
                        pose,
                        velocity,
                        motion,
                        appearance,
                        properties,
                    })
                },
            )
            .boxed(),
        (any::<u64>(), props(true, 1))
            .prop_filter("non-empty", |(_, c)| !c.is_empty())
            .prop_map(|(id, changes)| Message::EntityUpdate(EntityUpdate { id, changes }))
            .boxed(),
        (any::<u64>(), appearance())
            .prop_map(|(id, appearance)| {
                Message::EntityAppearance(EntityAppearance { id, appearance })
            })
            .boxed(),
        (any::<u64>(), any::<bool>())
            .prop_map(|(id, r)| {
                Message::EntityDestroy(EntityDestroy {
                    id,
                    reason: if r {
                        DestroyReason::Removed
                    } else {
                        DestroyReason::LeftInterest
                    },
                })
            })
            .boxed(),
        Just(Message::InterestReset).boxed(),
        (any::<u64>(), 0.0..1.0e12f64, 0.0f32..=10_000.0)
            .prop_map(|(server_time, game_clock, rate)| {
                Message::WorldClock(WorldClock {
                    server_time,
                    game_clock,
                    rate,
                })
            })
            .boxed(),
        (
            any::<u64>(),
            chat_channel(true),
            any::<bool>(),
            any::<u64>(),
            text1(64),
            text1(1024),
        )
            .prop_map(|(server_time, channel, echo, entity, sender, text)| {
                let system = channel == ChatChannel::System;
                Message::ChatMessage(ChatMessage {
                    server_time,
                    channel,
                    echo,
                    sender_entity: if system { 0 } else { entity },
                    sender: if system { String::new() } else { sender },
                    text,
                })
            })
            .boxed(),
        (0u8..3, any::<u16>(), text1(1024))
            .prop_map(|(l, seconds, text)| {
                let level = match l {
                    0 => NoticeLevel::Info,
                    1 => NoticeLevel::Warning,
                    _ => NoticeLevel::Shutdown,
                };
                Message::ServerNotice(ServerNotice {
                    level,
                    seconds,
                    text,
                })
            })
            .boxed(),
        (any::<u16>(), text(256))
            .prop_map(|(c, text)| {
                Message::ChatRejected(Failure {
                    code: ChatRejectedCode::from_u16(c),
                    text,
                })
            })
            .boxed(),
        (
            any::<u32>(),
            any::<u32>(),
            location(),
            finite32(),
            vec3(),
            motion(),
        )
            .prop_map(|(seq, epoch, location, heading, velocity, motion)| {
                Message::MoveInput(MoveInput {
                    seq,
                    epoch,
                    location,
                    heading,
                    velocity,
                    motion,
                })
            })
            .boxed(),
        (
            any::<u64>(),
            vec((any::<u64>(), vec3(), finite32(), vec3(), motion()), 1..=29),
        )
            .prop_map(|(server_time, entries)| {
                let mut out: Vec<MotionEntry> = Vec::new();
                for (id, position, heading, velocity, motion) in entries {
                    if !out.iter().any(|e| e.id == id) {
                        out.push(MotionEntry {
                            id,
                            position,
                            heading,
                            velocity,
                            motion,
                        });
                    }
                }
                Message::MotionBatch(MotionBatch {
                    server_time,
                    entries: out,
                })
            })
            .boxed(),
        any::<u64>()
            .prop_map(|client_time| Message::TimeRequest { client_time })
            .boxed(),
        (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>())
            .prop_map(|(t0, a, b, tick)| {
                Message::TimeReply(TimeReply {
                    client_time: t0,
                    server_receive: a.min(b),
                    server_send: a.max(b),
                    tick,
                })
            })
            .boxed(),
    ];
    Union::new(s)
}

fn sender(m: &Message) -> Side {
    if m.message_type().direction().allows(Side::Client) {
        Side::Client
    } else {
        Side::Server
    }
}

/// Encodes and decodes on the wire, checking byte-exact re-encoding.
fn round_trip(m: &Message) -> Result<(), TestCaseError> {
    let side = sender(m);
    if m.channel() == Channel::Datagram {
        let bytes = encode_datagram(m).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let back = decode_datagram(side, &bytes).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(&back, m);
        prop_assert_eq!(encode_datagram(&back).unwrap(), bytes);
    } else {
        let bytes = frame_to_vec(m).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let (raw, used) = split_frame(&bytes).unwrap().unwrap();
        prop_assert_eq!(used, bytes.len());
        let back = decode_frame_message(m.channel(), side, raw.ty, raw.body)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(&back, m);
        prop_assert_eq!(frame_to_vec(&back).unwrap(), bytes);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2048, ..ProptestConfig::default() })]

    #[test]
    fn every_valid_message_round_trips(m in messages()) {
        round_trip(&m)?;
    }

    #[test]
    fn values_round_trip(v in value()) {
        let mut out = Vec::new();
        if v.encode(&mut out, false).is_ok() {
            let (back, used) = Value::decode(&out, false).unwrap();
            prop_assert_eq!(used, out.len());
            prop_assert_eq!(back, v);
        }
    }

    #[test]
    fn mutated_frames_never_panic(m in messages(), flips in vec((any::<usize>(), any::<u8>()), 1..6)) {
        let mut bytes = if m.channel() == Channel::Datagram {
            encode_datagram(&m).unwrap()
        } else {
            frame_to_vec(&m).unwrap()
        };
        for (i, b) in flips {
            let n = bytes.len();
            bytes[i % n] ^= b;
        }
        let _ = decode_datagram(Side::Client, &bytes);
        let _ = decode_datagram(Side::Server, &bytes);
        for ch in [Channel::Control, Channel::World, Channel::Chat] {
            for side in [Side::Client, Side::Server] {
                let mut d = FrameDecoder::new(ch, side);
                let mut input: &[u8] = &bytes;
                while !input.is_empty() {
                    if d.push(&mut input).is_err() {
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in vec(any::<u8>(), 0..2048)) {
        let _ = decode_datagram(Side::Client, &bytes);
        let _ = decode_datagram(Side::Server, &bytes);
        let _ = Value::decode(&bytes, true);
        let _ = certificate_spki(&bytes);
        for ty in MessageType::ALL {
            let _ = Message::decode_body(*ty, &bytes);
        }
        let mut d = FrameDecoder::new(Channel::World, Side::Server);
        let mut input: &[u8] = &bytes;
        while !input.is_empty() {
            if d.push(&mut input).is_err() {
                break;
            }
        }
        prop_assert!(d.buffered() <= tessera_protocol::limits::MAX_FRAME_BYTES);
    }

    #[test]
    fn arbitrary_text_never_panics(s in "\\PC{0,120}", raw in vec(any::<u8>(), 0..200)) {
        let lossy = String::from_utf8_lossy(&raw);
        for t in [s.as_str(), lossy.as_ref()] {
            let _ = Fingerprint::parse(t);
            let _ = ConnectString::parse(t);
            let _ = ServerKey::parse(t);
            let _ = KnownServers::parse(t);
            let _ = ConnectString::parse(&format!("tessera://{t}"));
        }
    }

    #[test]
    fn connect_strings_round_trip(host in "[a-z][a-z0-9-]{0,20}(\\.[a-z0-9-]{1,10}){0,3}", port in 1u16.., pin in proptest::option::of(any::<[u8; 32]>())) {
        let pin = pin.map(Fingerprint);
        let mut s = format!("tessera://{host}:{port}");
        if let Some(p) = pin {
            s.push_str(&format!("?pin={p}"));
        }
        let c = ConnectString::parse(&s).unwrap();
        prop_assert_eq!(c.to_string(), s);
        prop_assert_eq!(c.pin, pin);
    }
}

#[test]
fn limits_and_rejections() {
    use tessera_protocol::ProtocolError as E;
    // Text one byte over its class maximum is refused by the encoder and the decoder.
    let long = "x".repeat(1025);
    let m = Message::ChatSend(ChatSend {
        channel: ChatChannel::Say,
        text: long,
    });
    assert!(matches!(frame_to_vec(&m), Err(E::StringLength { .. })));
    let mut body = vec![1u8];
    body.extend_from_slice(&1025u16.to_le_bytes());
    body.extend(std::iter::repeat_n(b'x', 1025));
    assert!(matches!(
        Message::decode_body(MessageType::ChatSend, &body),
        Err(E::StringLength { len: 1025, .. })
    ));
    // Server text with control characters is sanitised, client text kept as sent.
    let mut b = Vec::new();
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(b"a\n");
    b.extend_from_slice(&0u16.to_le_bytes());
    match Message::decode_body(
        MessageType::HelloAck,
        &[&[0, 0, 0, 0, 0, 0, 30, 15, 2][..], &b].concat(),
    )
    .unwrap()
    {
        Message::HelloAck(a) => assert_eq!(a.server_name, "a "),
        other => panic!("{other:?}"),
    }
    // Hello.max_entities below 64.
    let h = Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 63,
        client_name: String::new(),
    });
    assert_eq!(frame_to_vec(&h), Err(E::OutOfRange("max_entities")));
    // 33 equipped items.
    let mut a = Appearance::new(Body::Object { did: 1 });
    a.equipment = vec![
        Equipped {
            item: 1,
            ..Equipped::default()
        };
        33
    ];
    assert!(matches!(
        frame_to_vec(&Message::AppearanceSet(a)),
        Err(E::Count { .. })
    ));
    // Scale 0 and above 100.
    for scale in [0.0, 100.5, f32::NAN] {
        let mut a = Appearance::new(Body::Object { did: 1 });
        a.scale = scale;
        assert!(frame_to_vec(&Message::AppearanceSet(a)).is_err());
    }
    // Coordinates beyond 10^7.
    let mv = Message::MoveInput(MoveInput {
        seq: 0,
        epoch: 1,
        location: Location::new(0, 1.0e7 + 1.0, 0.0, 0.0),
        heading: 0.0,
        velocity: Vec3::ZERO,
        motion: Motion::STANDING,
    });
    assert_eq!(encode_datagram(&mv), Err(E::OutOfRange("location.x")));
    // Anchor that does not own its location.
    let p = Message::LocationReport(LocationReport {
        cause: LocationCause::Requested,
        request_id: 1,
        epoch: 1,
        server_time: 0,
        pose: Pose {
            location: Location::new(1, 0.0, 0.0, 0.0),
            heading: 0.0,
            anchor: Some(Anchor::new(100, 100, 0)),
        },
    });
    assert_eq!(frame_to_vec(&p), Err(E::AnchorNotOwning));
    // Landblock index 255.
    let p = Message::LocationReport(LocationReport {
        cause: LocationCause::Requested,
        request_id: 1,
        epoch: 1,
        server_time: 0,
        pose: Pose {
            location: Location::new(1, 40_800.0, 0.0, 0.0),
            heading: 0.0,
            anchor: Some(Anchor::new(255, 0, 0)),
        },
    });
    assert!(frame_to_vec(&p).is_err());
    // 30 motion entries, and a duplicate ID.
    let e = MotionEntry {
        id: 1,
        position: Vec3::ZERO,
        heading: 0.0,
        velocity: Vec3::ZERO,
        motion: Motion::STANDING,
    };
    let many: Vec<MotionEntry> = (0..30).map(|i| MotionEntry { id: i, ..e }).collect();
    assert!(
        encode_datagram(&Message::MotionBatch(MotionBatch {
            server_time: 0,
            entries: many
        }))
        .is_err()
    );
    assert_eq!(
        encode_datagram(&Message::MotionBatch(MotionBatch {
            server_time: 0,
            entries: vec![e, e]
        })),
        Err(E::Duplicate("motion entries"))
    );
    // A full batch is 1,084 bytes, within the recommended 1,100.
    let full: Vec<MotionEntry> = (0..29).map(|i| MotionEntry { id: i, ..e }).collect();
    let bytes = encode_datagram(&Message::MotionBatch(MotionBatch {
        server_time: 0,
        entries: full,
    }))
    .unwrap();
    assert_eq!(bytes.len(), 1_084);
    // Datagrams over 1,200 bytes.
    assert_eq!(
        decode_datagram(Side::Server, &[0; 1201]),
        Err(E::DatagramTooLarge(1201))
    );
    // A PropList over 256 entries and a value over 8 KiB.
    let many: Vec<Property> = (1..=257)
        .map(|i| Property::new(0x100 + i, Value::Bool(true)))
        .collect();
    assert!(
        frame_to_vec(&Message::EntityUpdate(EntityUpdate {
            id: 1,
            changes: many
        }))
        .is_err()
    );
    let big = Value::Array(vec![Value::String("y".repeat(1024)); 8]);
    assert_eq!(
        frame_to_vec(&Message::EntityUpdate(EntityUpdate {
            id: 1,
            changes: vec![Property::new(0x9000, big)]
        })),
        Err(E::ValueTooLarge)
    );
    // EntityUpdate with no changes; EntityCreate with a removal.
    assert!(
        frame_to_vec(&Message::EntityUpdate(EntityUpdate {
            id: 1,
            changes: vec![]
        }))
        .is_err()
    );
    // Wrong kind for a defined property.
    assert_eq!(
        frame_to_vec(&Message::EntityUpdate(EntityUpdate {
            id: 1,
            changes: vec![Property::new(pid::STANCE, Value::String("x".into()))]
        })),
        Err(E::WrongPropertyKind { pid: pid::STANCE })
    );
    // A frame body as large as allowed: a PropList of String values close to 64 KiB.
    let props: Vec<Property> = (0..63)
        .map(|i| Property::new(0x100 + i, Value::String("z".repeat(1024))))
        .collect();
    let bytes = frame_to_vec(&Message::EntityUpdate(EntityUpdate {
        id: 1,
        changes: props,
    }))
    .unwrap();
    assert!(bytes.len() <= tessera_protocol::limits::MAX_FRAME_BYTES);
    let props: Vec<Property> = (0..64)
        .map(|i| Property::new(0x100 + i, Value::String("z".repeat(1024))))
        .collect();
    assert!(matches!(
        frame_to_vec(&Message::EntityUpdate(EntityUpdate {
            id: 1,
            changes: props
        })),
        Err(E::BodyTooLarge(_))
    ));
    // Unknown message type, wrong channel, wrong direction.
    assert_eq!(
        decode_frame_message(Channel::Control, Side::Client, 0x0015, &[]),
        Err(E::UnknownMessageType(0x15))
    );
    assert!(matches!(
        decode_frame_message(Channel::Chat, Side::Server, 0x0101, &[]),
        Err(E::WrongChannel { .. })
    ));
    assert!(matches!(
        decode_frame_message(Channel::Control, Side::Server, 0x0001, &[]),
        Err(E::WrongDirection { .. })
    ));
}
