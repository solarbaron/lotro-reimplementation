// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of `tessera_compat::chat_session`
//! (`docs/specs/official-messages-play.md` §6.1, §6.6, §9; vectors of §10.5). The
//! end-to-end test drives the client against an in-process synthetic server over the
//! session module's in-memory transport: no socket is opened.

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex};

use tessera_compat::chat_session::client::TransportBinder;
use tessera_compat::chat_session::events::{
    CHAT_REQUEST_OPCODES, ChatEvent, ChatRequest, is_chat_opcode,
};
use tessera_compat::chat_session::state::{ChatAction, ChatPhase, ChatSessionState};
use tessera_compat::chat_session::wire::{
    ChatExtra, ChatMessagePayload, ChatRoomDescriptor, DescriptorKind, DescriptorSet,
    MSG_TYPE_CHAT, U64ToU32Table, read_descriptor_list,
};
use tessera_compat::chat_session::{
    AUTH_TYPE_CHAT, CHAT_RECIPIENT_ID, ChatClient, ChatCodecs, ChatConfig, chat_auth_header,
    check_ticket, effective_endpoint, parse_endpoint,
};
use tessera_compat::play::PlayError;
use tessera_compat::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, ConnectChallengeHeader, DatagramHeader, InMemoryUdpTransport,
    IsaacCipher, OptionalHeaders, SessionError, UdpDatagramTransport, decode_datagram,
    encode_datagram, split_blob,
};
use tessera_compat::world::{
    EncodedPropertyTable, EncodedPropertyValue, PropertyType, WireReader, WorldError,
    encode_event_envelope,
};
use tessera_net::{ChatChannel, Command, Event, SocialCommand, SocialEvent};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

fn no_schema() -> BTreeMap<u32, PropertyType> {
    BTreeMap::new()
}

fn world_room() -> ChatRoomDescriptor {
    ChatRoomDescriptor::new(
        DescriptorKind::ServerRoom,
        ChatRoomDescriptor::server_room_id(0x020D, 2),
        "World",
    )
}

// ---------------------------------------------------------------------------
// §10.5 vectors
// ---------------------------------------------------------------------------

#[test]
fn tv5a_login_status() {
    let b = hex("00 00 00 00 00");
    let ev = ChatEvent::decode(0x165, &b).expect("decode");
    assert_eq!(ev, ChatEvent::LoginStatus { status: 0 });
    assert_eq!(ev.encode().expect("encode"), b);
}

#[test]
fn tv5b_heartbeat_pair() {
    assert_eq!(ChatRequest::Heartbeat.encode().expect("encode"), vec![0x00]);
    assert_eq!(
        ChatRequest::decode(0x17E, &[0x00]),
        Ok(ChatRequest::Heartbeat)
    );
    let resp = hex("00 2A 00 00 00 00 00 00 00");
    let ev = ChatEvent::decode(0x174, &resp).expect("decode");
    assert_eq!(
        ev,
        ChatEvent::Status {
            opcode: 0x174,
            ctx_id: 0x2A,
            status: 0
        }
    );
    assert_eq!(ev.encode().expect("encode"), resp);
}

#[test]
fn tv5c_add_room_notice() {
    let b = hex(
        "00 00 00 00 00 18 00 00 00 03 00 00 00 0D 02 00 00 00 00 02 00 \
         05 57 00 6F 00 72 00 6C 00 64 00",
    );
    assert_eq!(b.len(), 32);
    let ev = ChatEvent::decode(0x18B, &b).expect("decode");
    let ChatEvent::AddRoom { notice_flags, room } = &ev else {
        panic!("not AddRoom: {ev:?}");
    };
    assert_eq!(*notice_flags, 0);
    assert_eq!(room, &world_room());
    assert_eq!(room.server_room_types(), Some((0x020D, 2)));
    assert_eq!(room.id(), Some(0x0002_0000_0000_020D));
    assert_eq!(ev.encode().expect("encode"), b);
}

// ---------------------------------------------------------------------------
// §9.1 rules 1–3
// ---------------------------------------------------------------------------

#[test]
fn descriptor_and_opcode_validation() {
    let buf1 = hex("17 00 00 00");
    let mut r = WireReader::new(&buf1);
    assert_eq!(
        ChatRoomDescriptor::decode(&mut r),
        Err(PlayError::InvalidDescriptorTag(0x17))
    );
    for t in [0u32, 4] {
        let mut b = hex("18 00 00 00");
        b.extend(t.to_le_bytes());
        b.extend([0u8; 9]);
        let mut r = WireReader::new(&b);
        assert_eq!(
            ChatRoomDescriptor::decode(&mut r),
            Err(PlayError::InvalidDescriptorType(t))
        );
    }
    for op in [0x15B, 0x196, 0x1A1, 0x1A3, 0x30] {
        assert!(!is_chat_opcode(op));
        assert_eq!(
            ChatEvent::decode(op, &[0]),
            Err(PlayError::ChatOpcodeOutOfRange(op))
        );
    }
    for op in CHAT_REQUEST_OPCODES {
        assert_eq!(
            ChatEvent::decode(op, &[0]),
            Err(PlayError::UnexpectedChatDirection(op))
        );
    }
    // The admin response is ignored without decoding; the admin request cannot be built.
    assert_eq!(
        ChatEvent::decode(0x15F, &[0xFF]),
        Ok(ChatEvent::AdminIgnored { opcode: 0x15F })
    );
    assert_eq!(
        ChatRequest::decode(0x17D, &[0]),
        Err(PlayError::NotAChatRequest(0x17D))
    );
    // bool8 values above 1 fail.
    let mut b = ChatEvent::RecvDataFromRoom {
        recv_flags: 0,
        sender: ChatRoomDescriptor::Null,
        room: ChatRoomDescriptor::Null,
        msg_type: 0,
        data: vec![],
        is_echo: true,
    }
    .encode()
    .expect("encode");
    if let Some(last) = b.last_mut() {
        *last = 2;
    }
    assert!(ChatEvent::decode(0x179, &b).is_err());
    // Trailing bytes fail.
    assert!(ChatEvent::decode(0x165, &hex("00 00 00 00 00 00")).is_err());
}

#[test]
fn container_counts_are_bounded_before_allocation() {
    let mut b = vec![0xFF, 0xFF, 0xFF, 0x7F];
    b.extend([0u8; 8]);
    let too_many = |r: Result<(), PlayError>| {
        matches!(
            r,
            Err(PlayError::World(WorldError::CountExceedsRemaining { .. }))
        )
    };
    let mut r = WireReader::new(&b);
    assert!(too_many(read_descriptor_list(&mut r).map(|_| ())));
    let buf2 = hex("10 00 FF FF FF FF");
    let mut r = WireReader::new(&buf2);
    assert!(too_many(U64ToU32Table::decode(&mut r).map(|_| ())));
    let buf3 = hex("FF FF 02 00 00 00 00 00 00 00");
    let mut r = WireReader::new(&buf3);
    assert!(
        too_many(DescriptorSet::decode(&mut r).map(|_| ())),
        "2 descriptors need 8 bytes"
    );
    let buf4 = hex("FF FF 01 00 00 00 00 00 00 00");
    let mut r = WireReader::new(&buf4);
    let set = DescriptorSet::decode(&mut r).expect("one null descriptor");
    assert_eq!(set.entries, vec![ChatRoomDescriptor::Null]);
    assert_eq!(set.num_buckets, 0xFFFF, "kept, never used to allocate");
}

// ---------------------------------------------------------------------------
// Round trips of the whole catalogue (§6.6.5)
// ---------------------------------------------------------------------------

#[test]
fn every_request_round_trips() {
    let a = ChatRoomDescriptor::new(DescriptorKind::Entity, 5, "Me");
    let b = world_room();
    let reqs = vec![
        ChatRequest::SendDataToName {
            ctx_id: 1,
            sender: a.clone(),
            target: ChatRoomDescriptor::new(DescriptorKind::Entity, 0, "Bob"),
            msg_type: MSG_TYPE_CHAT,
            data: vec![1, 2],
            delivery_flag: true,
        },
        ChatRequest::UpdateRoomFlags {
            ctx_id: 2,
            actor: a.clone(),
            room: b.clone(),
            room_flags: 0xFFFF_0000_0000_0001,
        },
        ChatRequest::SetRoomParam {
            ctx_id: 3,
            actor: a.clone(),
            room: b.clone(),
            param_value: 9,
        },
        ChatRequest::RoomAction {
            ctx_id: 4,
            actor: a.clone(),
            room: b.clone(),
        },
        ChatRequest::SendDataToRoom {
            ctx_id: 5,
            sender: a.clone(),
            room: b.clone(),
            msg_type: MSG_TYPE_CHAT,
            data: vec![],
            delivery_flag: false,
        },
        ChatRequest::InviteToRoom {
            ctx_id: 6,
            sender: a.clone(),
            target: a.clone(),
        },
        ChatRequest::CreateRoom {
            ctx_id: 7,
            creator: a.clone(),
            room: b.clone(),
            password_or_extra: ChatRoomDescriptor::Null,
            flags_a: 1,
            flags_b: 2,
        },
        ChatRequest::DestroyRoom {
            ctx_id: 8,
            actor: a.clone(),
            room: b.clone(),
        },
        ChatRequest::QueryBuddyList {
            ctx_id: 9,
            entity: a.clone(),
        },
        ChatRequest::GetRoomList,
        ChatRequest::BatchRoomOp {
            ctx_id: 10,
            actor: a.clone(),
            room: b.clone(),
            values: vec![1, 2, 3],
        },
        ChatRequest::RoomEnter {
            ctx_id: 11,
            entity: a.clone(),
            room: b.clone(),
            flags_a: 0,
            flags_b: 0,
        },
        ChatRequest::ModifySubscription {
            ctx_id: 12,
            subscriber: a.clone(),
            target: b.clone(),
        },
        ChatRequest::Heartbeat,
        ChatRequest::QueryRoom {
            ctx_id: 13,
            room: b.clone(),
            flags_a: 4,
            flags_b: 5,
        },
        ChatRequest::QueryRoomInfo {
            ctx_id: 14,
            room: b.clone(),
        },
        ChatRequest::ChangePassword {
            ctx_id: 15,
            actor: a.clone(),
            room: b.clone(),
            flags_a: 0,
            flags_b: 1,
        },
        ChatRequest::SetRoomLeader {
            ctx_id: 16,
            room: b.clone(),
            new_leader: a.clone(),
            flags_a: 0,
            flags_b: 0,
        },
        ChatRequest::KickFromRoom {
            ctx_id: 17,
            room: b.clone(),
            kick_flags: 3,
            target: a.clone(),
        },
        ChatRequest::SendDataToEntity {
            ctx_id: 18,
            sender: a.clone(),
            target: a.clone(),
            extra: ChatRoomDescriptor::Null,
        },
        ChatRequest::RoomLeave {
            ctx_id: 19,
            entity: a.clone(),
            room: b.clone(),
        },
        ChatRequest::QueryTracker {
            ctx_id: 20,
            entity: a,
            tracked_ids: vec![1, u64::MAX],
        },
    ];
    let mut opcodes: Vec<u32> = reqs.iter().map(ChatRequest::opcode).collect();
    opcodes.push(0x17D);
    opcodes.sort_unstable();
    let mut all = CHAT_REQUEST_OPCODES.to_vec();
    all.sort_unstable();
    assert_eq!(
        opcodes, all,
        "every request but the admin one has a variant"
    );
    for req in reqs {
        let b = req.encode().expect("encode");
        assert_eq!(b.first(), Some(&0x00), "base_flags");
        assert_eq!(ChatRequest::decode(req.opcode(), &b), Ok(req));
    }
    assert_eq!(ChatRequest::GetRoomList.encode().expect("encode"), vec![0]);
}

#[test]
fn every_event_round_trips() {
    let a = ChatRoomDescriptor::new(DescriptorKind::Entity, 5, "Me");
    let b = world_room();
    let table = U64ToU32Table {
        num_buckets: 3,
        entries: vec![(1, 2), (3, 4)],
    };
    let mut events = vec![
        ChatEvent::EntityDescriptor {
            notice_flags: 1,
            entity: a.clone(),
        },
        ChatEvent::QueryTrackerResp {
            ctx_id: 1,
            status: 0,
            tracker_id: 9,
            map_a: table.clone(),
            map_b: U64ToU32Table::default(),
            map_c: table,
            tracked_ids: vec![7],
        },
        ChatEvent::RemoveRoom {
            notice_flags: 0,
            room: b.clone(),
        },
        ChatEvent::LoginStatus { status: -5 },
        ChatEvent::RoomList {
            ctx_id: 2,
            status: 0,
            meta_a: 1,
            meta_b: 2,
            rooms: DescriptorSet {
                num_buckets: 1,
                entries: vec![b.clone()],
            },
        },
        ChatEvent::RoomInfo {
            ctx_id: 3,
            status: 0,
            info_flags: 0,
            members: vec![a.clone()],
            moderators: vec![],
        },
        ChatEvent::RoomNameResolve {
            notice_flags: 0,
            name: "x".into(),
            resolve_status: 1,
        },
        ChatEvent::RoomClosed {
            notice_flags: 0,
            room_name: "Room".into(),
        },
        ChatEvent::KickResp {
            ctx_id: 4,
            status: 0,
            kicked: vec![a.clone()],
        },
        ChatEvent::RecvDataFromRoom {
            recv_flags: 0,
            sender: a.clone(),
            room: b.clone(),
            msg_type: MSG_TYPE_CHAT,
            data: vec![9],
            is_echo: false,
        },
        ChatEvent::SendDataToRoomResp {
            ctx_id: 5,
            status: 0,
            names: vec!["a".into(), String::new()],
            descriptors: DescriptorSet::default(),
        },
        ChatEvent::RoomEnterResp {
            ctx_id: 6,
            status: 0,
            room_name: "Room".into(),
            room: b.clone(),
        },
        ChatEvent::MemberJoined {
            notice_flags: 0,
            room_name: "Room".into(),
            player_name: "Bob".into(),
        },
        ChatEvent::RecvDataFromEntity {
            recv_flags: 0,
            sender: a.clone(),
            target: a.clone(),
            extra: ChatRoomDescriptor::Null,
            msg_type: MSG_TYPE_CHAT,
            data: vec![],
        },
        ChatEvent::RoomStatusChanged {
            notice_flags: 0,
            room: b.clone(),
            new_status: 1,
            status_flags: 2,
        },
        ChatEvent::AddRoom {
            notice_flags: 0,
            room: b.clone(),
        },
        ChatEvent::BuddyListResp {
            ctx_id: 7,
            status: 0,
            buddies: vec![a.clone()],
            entity_ids: vec![1],
        },
        ChatEvent::InviteResp {
            ctx_id: 8,
            status: -1,
            invited: vec![],
            invite_flags: 3,
        },
        ChatEvent::LeaderChanged {
            notice_flags: 0,
            new_leader: "Bob".into(),
        },
        ChatEvent::MemberLeft {
            notice_flags: 0,
            room_name: "Room".into(),
            player_name: "Bob".into(),
            leave_reason: 2,
        },
        ChatEvent::SendDataToNameResp {
            ctx_id: 9,
            status: 0,
            sender: a.clone(),
            target: a,
            delivery_status: 1,
        },
        ChatEvent::CreateRoomResp {
            ctx_id: 10,
            status: 0,
            room_name: "Room".into(),
        },
        ChatEvent::EntityOffline {
            notice_flags: 0,
            entity_id: 42,
        },
    ];
    for op in [0x163, 0x164, 0x166, 0x174, 0x17B, 0x184, 0x185, 0x18F] {
        events.push(ChatEvent::Status {
            opcode: op,
            ctx_id: op,
            status: 1,
        });
    }
    for op in [0x160, 0x16C, 0x180, 0x18C] {
        events.push(ChatEvent::StatusWords {
            opcode: op,
            ctx_id: op,
            status: 0,
            status_a: 1,
            status_b: 2,
        });
    }
    // 36 server-to-client events: 35 here plus the admin response 0x15F.
    let mut ops: Vec<u32> = events.iter().map(ChatEvent::opcode).collect();
    ops.push(0x15F);
    ops.sort_unstable();
    ops.dedup();
    assert_eq!(ops.len(), 36);
    let mut all: Vec<u32> = (0x15C..=0x195).chain([0x1A2]).collect();
    all.retain(|op| !CHAT_REQUEST_OPCODES.contains(op));
    assert_eq!(ops, all);
    assert_eq!(all.len() + CHAT_REQUEST_OPCODES.len(), 59);
    for ev in events {
        let b = ev.encode().expect("encode");
        assert_eq!(ChatEvent::decode(ev.opcode(), &b), Ok(ev));
    }
}

#[test]
fn chat_message_payload() {
    let p = ChatMessagePayload::text(
        ChatRoomDescriptor::new(DescriptorKind::Entity, 1, "Bob"),
        "hello",
    );
    let b = p.to_bytes().expect("encode");
    assert_eq!(ChatMessagePayload::from_bytes(&b, &no_schema()), Ok(p));

    let p = ChatMessagePayload {
        sender: ChatRoomDescriptor::Null,
        text: String::new(),
        metadata: vec![1, 2, 3],
        extra: ChatExtra::Descriptors {
            num_buckets: 2,
            entries: vec![(7, world_room(), 9)],
        },
        msg_flags: 1,
        chat_mode_word: 0,
    };
    let b = p.to_bytes().expect("encode");
    assert_eq!(ChatMessagePayload::from_bytes(&b, &no_schema()), Ok(p));

    let mut schema = BTreeMap::new();
    schema.insert(0x0000_0007, PropertyType::Int);
    let mut props = EncodedPropertyTable::new();
    props.push(7, EncodedPropertyValue::Int(-4));
    let p = ChatMessagePayload {
        sender: ChatRoomDescriptor::Null,
        text: "t".into(),
        metadata: vec![],
        extra: ChatExtra::Properties {
            properties: props,
            table: Some((1, vec![(3, ChatRoomDescriptor::Null)])),
        },
        msg_flags: 0,
        chat_mode_word: 0,
    };
    let b = p.to_bytes().expect("encode");
    assert_eq!(ChatMessagePayload::from_bytes(&b, &schema), Ok(p));

    let mut bad = ChatMessagePayload::text(ChatRoomDescriptor::Null, "")
        .to_bytes()
        .expect("encode");
    // extra_type is after the null sender (4), empty text (1) and empty metadata (4).
    bad[9] = 3;
    assert_eq!(
        ChatMessagePayload::from_bytes(&bad, &no_schema()),
        Err(PlayError::UnknownExtraType(3))
    );
}

// ---------------------------------------------------------------------------
// §6.1.1 endpoint, §6.6.2 auth header, §9.2 invariant 6
// ---------------------------------------------------------------------------

#[test]
fn endpoint_fallback_and_parsing() {
    assert_eq!(effective_endpoint("", false, "198.51.100.7:9000"), None);
    assert_eq!(
        effective_endpoint("", true, "198.51.100.7:9000").as_deref(),
        Some("198.51.100.7:9000")
    );
    assert_eq!(
        effective_endpoint("198.51.100.8:9001", false, "x").as_deref(),
        Some("198.51.100.8:9001")
    );
    assert_eq!(
        parse_endpoint("198.51.100.8:9001"),
        Ok(SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 8), 9001))
    );
    for bad in [
        "chat.example:9000",
        "198.51.100.8",
        "198.51.100.8:0",
        "0.0.0.0:9",
        "",
    ] {
        assert!(parse_endpoint(bad).is_err(), "{bad}");
    }
}

#[test]
fn tickets_are_checked_as_text_not_decoded() {
    // §6.6.2: the ticket travels as its ASCII Base64 text; any printable ASCII passes.
    for ok in ["TWFu", "TWE", "TR==", "A", &"A".repeat(4095)] {
        assert_eq!(check_ticket(ok), Ok(()), "{ok}");
    }
    for bad in ["", "TW u", "T\tQ", "\u{e9}", &"A".repeat(4096)] {
        assert_eq!(check_ticket(bad), Err(PlayError::InvalidTicket), "{bad}");
    }
}

#[test]
fn chat_auth_header_modes() {
    let ticket = "AAEC";
    let h = chat_auth_header("acct", Some(ticket), 1000).expect("header");
    // §6.6.2 item 1, `official-session.md` §6.2: `netVersion` "180804", 0x40000002, 0x04.
    assert_eq!(h.net_version, "180804");
    assert_eq!(h.auth_type, AUTH_TYPE_CHAT);
    assert_eq!(h.auth_flags, AUTH_FLAG_HAS_EXTRA_BLOB);
    let c = &h.credentials;
    assert_eq!(c.account_or_ticket, "acct");
    assert_eq!(c.ticket.as_deref(), Some("AAEC"));
    let mut wire = Vec::new();
    h.encode(&mut wire).expect("encode");
    assert_eq!(&wire[..7], &[0x06, b'1', b'8', b'0', b'8', b'0', b'4']);
    // extraBlobLen 5 = the 1-byte length 4 and the 4 ASCII characters, undecoded.
    assert!(wire.ends_with(&[5, 0, 0, 0, 4, b'A', b'A', b'E', b'C']));

    let h = chat_auth_header("acct", None, 1000).expect("header");
    assert_eq!((h.auth_type, h.auth_flags), (AUTH_TYPE_CHAT, 0));
    assert!(chat_auth_header("acct", Some("!! !!"), 0).is_err());
}

// ---------------------------------------------------------------------------
// §6.6.2–§6.6.3 state machine
// ---------------------------------------------------------------------------

#[test]
fn login_gate_heartbeat_and_drop() {
    let mut s = ChatSessionState::new(0.0);
    assert_eq!(s.tick(0.5), None, "initial delay 1 s");
    assert_eq!(s.tick(1.0), Some(ChatAction::Connect));
    assert_eq!(s.phase(), ChatPhase::Connecting { since: 1.0 });
    s.on_transport_connected(2.0);
    assert!(!s.is_ready(), "State 5 alone is not ready");
    // A refused login: retry after the initial policy's 30 s.
    assert_eq!(s.on_login_status(-1, 2.5), Some(ChatAction::Disconnect));
    assert_eq!(s.phase(), ChatPhase::Waiting { until: 32.5 });
    assert_eq!(s.tick(32.4), None);
    assert_eq!(s.tick(32.5), Some(ChatAction::Connect));
    s.on_transport_connected(33.0);
    assert_eq!(s.on_login_status(0, 33.0), None);
    assert!(s.is_ready());
    assert_eq!(s.tick(48.0), None, "15 s exactly is not over the threshold");
    assert_eq!(s.tick(48.1), Some(ChatAction::SendHeartbeat));
    assert!(s.heartbeat_sent());
    assert_eq!(s.tick(60.0), None, "heartbeat sent once");
    s.on_event(61.0);
    assert!(!s.heartbeat_sent());
    assert_eq!(s.tick(76.1), Some(ChatAction::SendHeartbeat));
    assert_eq!(s.tick(121.0), None, "60 s is measured from the last event");
    assert_eq!(s.tick(121.5), Some(ChatAction::Disconnect));
    // After a successful login the reconnect policy (10 s) applies.
    assert_eq!(s.phase(), ChatPhase::Waiting { until: 131.5 });
    // A login-status wait that times out.
    assert_eq!(s.tick(131.5), Some(ChatAction::Connect));
    s.on_transport_connected(132.0);
    assert_eq!(s.tick(152.0), None);
    assert_eq!(s.tick(152.5), Some(ChatAction::Disconnect));
    // A transport handshake that never finishes times out the same way.
    assert_eq!(s.tick(162.5), Some(ChatAction::Connect));
    assert_eq!(s.tick(182.5), None);
    assert_eq!(s.tick(182.6), Some(ChatAction::Disconnect));
    assert!(matches!(s.phase(), ChatPhase::Waiting { .. }));
    s.close();
    assert_eq!(s.tick(1000.0), None);
}

// ---------------------------------------------------------------------------
// End to end over the in-memory transport
// ---------------------------------------------------------------------------

const CLIENT: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9100);
const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 9), 9200);
const INCOMING_SEED: u32 = 0xBEEF_5678;

struct FakeServer {
    isaac: IsaacCipher,
    seq: u32,
}

impl FakeServer {
    fn new() -> Self {
        Self {
            isaac: IsaacCipher::from_seed(INCOMING_SEED),
            seq: 1,
        }
    }

    fn challenge() -> Vec<u8> {
        let opt = OptionalHeaders {
            connect_challenge: Some(ConnectChallengeHeader {
                server_time: 1.0,
                assigned_client_rec_id: 0x0111,
                server_rec_id: 0x0222,
                outgoing_seed: 0xCAFE_1234,
                incoming_seed: INCOMING_SEED,
                reserved_dword_14: 0,
                client_conn_idx: 4,
                reserved_word_1a: 0,
                codec_uuid: [0u8; 16],
            }),
            ..OptionalHeaders::default()
        };
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0111,
                ..DatagramHeader::default()
            },
            &opt,
            &[],
            None,
            None,
        )
        .expect("challenge")
    }

    /// A transport-level datagram without blobs: unsequenced, acknowledging `ack` when
    /// given. It keeps the session alive without being a chat event.
    fn keepalive(&self, ack: Option<u32>) -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0111,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                cumulative_ack: ack,
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .expect("keepalive")
    }

    /// The server's own graceful `Disconnect` (`official-session.md` §8.2), answering the
    /// client's.
    fn disconnect(&self) -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0111,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                disconnect: true,
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .expect("disconnect")
    }

    fn event(&mut self, ev: &ChatEvent) -> Vec<u8> {
        let inner = ev.encode().expect("event");
        self.blob(ev.opcode(), &inner)
    }

    fn blob(&mut self, opcode: u32, inner: &[u8]) -> Vec<u8> {
        self.seq += 1;
        let frags = split_blob(
            2,
            (self.seq & 0xFFFF) as u16,
            1,
            0,
            None,
            opcode,
            &encode_event_envelope(inner),
        )
        .expect("split");
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: 0x0111,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
            Some(mask),
            None,
        )
        .expect("datagram")
    }
}

/// Whether any datagram the client sent since the last call carries `Disconnect`.
fn sent_disconnect(client: &mut ChatClient<InMemoryUdpTransport>) -> bool {
    let mut seen = false;
    while let Some((dgram, _)) = client.transport_mut().pop_outbound() {
        let pkt = decode_datagram(&dgram, false, None, false).expect("client datagram");
        seen |= pkt.opt_headers.disconnect;
    }
    seen
}

/// Every blob the client sent since the last call: `(recipient, opcode, inner payload)`.
fn sent_blobs(client: &mut ChatClient<InMemoryUdpTransport>) -> Vec<(u64, u32, Vec<u8>)> {
    let mut out = Vec::new();
    while let Some((dgram, dst)) = client.transport_mut().pop_outbound() {
        assert_eq!(dst, SERVER);
        let pkt = decode_datagram(&dgram, false, None, false).expect("client datagram");
        for f in pkt.fragments {
            let word = f.header.blob_header_word.expect("single-fragment blob");
            assert_eq!(f.payload.first(), Some(&0x00), "envelopeFlags");
            out.push((
                f.header.recipient_id,
                word,
                f.payload.get(1..).unwrap_or_default().to_vec(),
            ));
        }
    }
    out
}

#[test]
fn chat_client_end_to_end() {
    let config = ChatConfig::new(SERVER, "acct", Some("AAEC".into()), 1000);
    assert!(!format!("{config:?}").contains("acct"), "account redacted");
    let mut client = ChatClient::with_transport(
        InMemoryUdpTransport::new(CLIENT),
        config,
        ChatCodecs::default(),
        0.0,
    )
    .expect("client");
    client.set_self_descriptor(5, "Me");
    let schema = no_schema();

    // The first attempt waits for the initial delay.
    client.poll(0.5, &schema).expect("poll");
    assert!(client.transport_mut().pop_outbound().is_none());
    client.poll(1.0, &schema).expect("poll");
    let (auth_dgram, dst) = client.transport_mut().pop_outbound().expect("SessionAuth");
    assert_eq!(dst, SERVER);
    let auth = decode_datagram(&auth_dgram, false, None, true)
        .expect("decode")
        .opt_headers
        .session_auth
        .expect("SessionAuth");
    assert_eq!(auth.auth_type, 0x4000_0002);
    assert_eq!(auth.auth_flags, 0x04);
    // `official-session.md` §6.2: `netVersion` "180804" on the chat connection, and the
    // ticket's text undecoded.
    assert_eq!(auth.net_version, "180804");
    assert_eq!(
        &auth_dgram[22..29],
        &[0x06, b'1', b'8', b'0', b'8', b'0', b'4']
    );
    assert_eq!(auth.credentials.ticket.as_deref(), Some("AAEC"));

    // Handshake, then the login status.
    let mut server = FakeServer::new();
    client
        .transport_mut()
        .push_inbound(FakeServer::challenge(), SERVER);
    client.poll(1.1, &schema).expect("poll");
    // §6.4: the chat connection's `ConnectAcceptHeader` carries maxRecvRate 0x00040000.
    let (accept_dgram, _) = client.transport_mut().pop_outbound().expect("accept");
    let accept = decode_datagram(&accept_dgram, false, None, true)
        .expect("decode")
        .opt_headers
        .connect_accept
        .expect("ConnectAcceptHeader");
    assert_eq!(accept.max_recv_rate, 0x0004_0000);
    let _ = sent_blobs(&mut client);
    assert!(!client.is_ready());
    assert_eq!(
        client.command(
            &Command::Social(SocialCommand::JoinChannel { name: "x".into() }),
            1.1
        ),
        Err(PlayError::ChatNotReady)
    );
    let login = server.event(&ChatEvent::LoginStatus { status: 0 });
    client.transport_mut().push_inbound(login, SERVER);
    client.poll(1.2, &schema).expect("poll");
    assert!(client.is_ready(), "0x165 with status 0 opens the session");

    // A server-provisioned room.
    let add = server.event(&ChatEvent::AddRoom {
        notice_flags: 0,
        room: world_room(),
    });
    client.transport_mut().push_inbound(add, SERVER);
    let polled = client.poll(1.3, &schema).expect("poll");
    assert_eq!(
        polled.events,
        vec![Event::Social(SocialEvent::ChannelJoined {
            channel: "World".into()
        })]
    );
    assert_eq!(client.rooms(), &[world_room()]);

    // Sending to it: 0x16A to the chat recipient ID.
    client
        .command(
            &Command::Chat {
                channel: ChatChannel::Named {
                    name: "World".into(),
                },
                text: "hello".into(),
            },
            1.4,
        )
        .expect("send");
    client.poll(1.4, &schema).expect("poll");
    let blobs = sent_blobs(&mut client);
    let (recipient, opcode, inner) = blobs
        .iter()
        .find(|(_, op, _)| *op == 0x16A)
        .expect("0x16A sent");
    assert_eq!(*recipient, CHAT_RECIPIENT_ID);
    let ChatRequest::SendDataToRoom {
        room,
        msg_type,
        data,
        sender,
        ..
    } = ChatRequest::decode(*opcode, inner).expect("request")
    else {
        panic!("not a room message");
    };
    assert_eq!(room, world_room());
    assert_eq!(msg_type, MSG_TYPE_CHAT);
    assert_eq!(sender.name(), Some("Me"));
    let payload = ChatMessagePayload::from_bytes(&data, &schema).expect("payload");
    assert_eq!(payload.text, "hello");

    // A message from the room.
    let data = ChatMessagePayload::text(
        ChatRoomDescriptor::new(DescriptorKind::Entity, 9, "Bob"),
        "hi all",
    )
    .to_bytes()
    .expect("payload");
    let recv = server.event(&ChatEvent::RecvDataFromRoom {
        recv_flags: 0,
        sender: ChatRoomDescriptor::new(DescriptorKind::Entity, 9, "Bob"),
        room: world_room(),
        msg_type: MSG_TYPE_CHAT,
        data,
        is_echo: false,
    });
    client.transport_mut().push_inbound(recv, SERVER);
    let polled = client.poll(2.0, &schema).expect("poll");
    assert_eq!(
        polled.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Named {
                name: "World".into()
            },
            sender: Some("Bob".into()),
            text: "hi all".into(),
        }]
    );

    // Joining a user channel: 0x177, answered by 0x17C.
    client
        .command(
            &Command::Social(SocialCommand::JoinChannel {
                name: "Hobby".into(),
            }),
            2.1,
        )
        .expect("join");
    client.poll(2.1, &schema).expect("poll");
    let blobs = sent_blobs(&mut client);
    let (_, _, inner) = blobs
        .iter()
        .find(|(_, op, _)| *op == 0x177)
        .expect("0x177 sent");
    let ChatRequest::RoomEnter { ctx_id, .. } = ChatRequest::decode(0x177, inner).expect("req")
    else {
        panic!("not RoomEnter");
    };
    let resp = server.event(&ChatEvent::RoomEnterResp {
        ctx_id,
        status: 0,
        room_name: "Hobby".into(),
        room: ChatRoomDescriptor::new(DescriptorKind::UserRoom, 77, "Hobby"),
    });
    client.transport_mut().push_inbound(resp, SERVER);
    let polled = client.poll(2.2, &schema).expect("poll");
    assert_eq!(
        polled.events,
        vec![Event::Social(SocialEvent::ChannelJoined {
            channel: "Hobby".into()
        })]
    );

    // The admin response is ignored and counted.
    let admin = server.blob(0x15F, &[0x00, 0x00]);
    client.transport_mut().push_inbound(admin, SERVER);
    let polled = client.poll(2.3, &schema).expect("poll");
    assert!(polled.events.is_empty());
    assert_eq!(client.counters().admin_ignored, 1);

    // A room message for a nameless room with ID 0 is not guessed by ID, even when a
    // room kept with ID 0 is known.
    let created = server.event(&ChatEvent::CreateRoomResp {
        ctx_id: 999,
        status: 0,
        room_name: "Zero".into(),
    });
    client.transport_mut().push_inbound(created, SERVER);
    let polled = client.poll(2.35, &schema).expect("poll");
    assert_eq!(
        polled.events,
        vec![Event::Social(SocialEvent::ChannelJoined {
            channel: "Zero".into()
        })]
    );
    let nameless = ChatMessagePayload::text(
        ChatRoomDescriptor::new(DescriptorKind::Entity, 9, "Bob"),
        "lost",
    )
    .to_bytes()
    .expect("payload");
    let recv = server.event(&ChatEvent::RecvDataFromRoom {
        recv_flags: 0,
        sender: ChatRoomDescriptor::new(DescriptorKind::Entity, 9, "Bob"),
        room: ChatRoomDescriptor::new(DescriptorKind::UserRoom, 0, ""),
        msg_type: MSG_TYPE_CHAT,
        data: nameless,
        is_echo: false,
    });
    client.transport_mut().push_inbound(recv, SERVER);
    let polled = client.poll(2.4, &schema).expect("poll");
    assert!(polled.events.is_empty());
    assert_eq!(client.counters().unmapped_messages, 1);

    // A blob that fails to decode still counts as received (§6.6.3 rule 1).
    let junk = server.blob(0x179, &[0x00]);
    client.transport_mut().push_inbound(junk, SERVER);
    client.poll(10.0, &schema).expect("poll");
    assert_eq!(client.counters().decode_errors, 1);
    let _ = sent_blobs(&mut client);
    client.poll(17.4, &schema).expect("poll");
    assert_eq!(client.counters().heartbeats, 0, "idle since 10.0 only");

    // 15 s without events: a heartbeat (0x17E, payload 0x00).
    client.poll(25.1, &schema).expect("poll");
    let blobs = sent_blobs(&mut client);
    assert!(
        blobs
            .iter()
            .any(|(r, op, inner)| *r == CHAT_RECIPIENT_ID && *op == 0x17E && inner == &[0x00])
    );
    assert_eq!(client.counters().heartbeats, 1);

    // 60 s without chat events while the heartbeat is outstanding. The transport is kept
    // alive (and the heartbeat acknowledged) by blob-free datagrams, so only the chat-level
    // drop rule can fire: the session is still connected and gets a graceful Disconnect.
    for t in [40.0, 65.0] {
        client
            .transport_mut()
            .push_inbound(server.keepalive(Some(10_000)), SERVER);
        client.poll(t, &schema).expect("poll");
        assert!(
            client.is_ready(),
            "transport alive, no chat event: still ready"
        );
    }
    let _ = sent_blobs(&mut client);
    client.poll(70.2, &schema).expect("poll");
    assert!(!client.is_ready());
    assert_eq!(client.counters().disconnects, 1);
    assert!(
        sent_disconnect(&mut client),
        "Disconnect flushed before the drop"
    );
    let ChatPhase::Waiting { until } = client.phase() else {
        panic!("expected a scheduled reconnect, got {:?}", client.phase());
    };
    assert!((until - 80.2).abs() < 1e-9, "reconnect policy: 10 s");
    client.poll(until, &schema).expect("poll");
    assert_eq!(client.counters().connects, 2);
}

// ---------------------------------------------------------------------------
// Reconnects bind a fresh socket; the old session flushes its Disconnect on its own
// ---------------------------------------------------------------------------

/// Sockets of the test network: per socket an inbound queue, and one log of sent
/// datagrams with the sending socket's index.
#[derive(Debug, Default)]
struct Net {
    inbound: Vec<VecDeque<(Vec<u8>, SocketAddrV4)>>,
    sent: Vec<(usize, Vec<u8>)>,
    fail_binds: bool,
    /// Reads on this socket fail.
    fail_reads: Option<usize>,
}

#[derive(Debug)]
struct NetSocket {
    index: usize,
    net: Arc<Mutex<Net>>,
}

impl UdpDatagramTransport for NetSocket {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        let port = u16::try_from(9100 + self.index).unwrap_or(9100);
        Ok(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    fn send_to(&mut self, buf: &[u8], _target: SocketAddrV4) -> Result<usize, SessionError> {
        let mut net = self.net.lock().expect("net");
        net.sent.push((self.index, buf.to_vec()));
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        let mut net = self.net.lock().expect("net");
        if net.fail_reads == Some(self.index) {
            return Err(SessionError::Io("read refused".into()));
        }
        let Some((data, src)) = net
            .inbound
            .get_mut(self.index)
            .and_then(VecDeque::pop_front)
        else {
            return Ok(None);
        };
        buf[..data.len()].copy_from_slice(&data);
        Ok(Some((data.len(), src)))
    }
}

fn net_binder(net: &Arc<Mutex<Net>>) -> TransportBinder<NetSocket> {
    let net = Arc::clone(net);
    Box::new(move || {
        let mut n = net.lock().expect("net");
        if n.fail_binds {
            return Err(PlayError::InvalidEndpoint("bind refused".into()));
        }
        n.inbound.push(VecDeque::new());
        Ok(NetSocket {
            index: n.inbound.len() - 1,
            net: Arc::clone(&net),
        })
    })
}

fn push(net: &Arc<Mutex<Net>>, socket: usize, dgram: Vec<u8>) {
    net.lock().expect("net").inbound[socket].push_back((dgram, SERVER));
}

/// The sent datagrams since the last call, decoded: `(socket, packet)`.
fn sent(net: &Arc<Mutex<Net>>) -> Vec<(usize, tessera_compat::session::DecodedPacket)> {
    let mut n = net.lock().expect("net");
    std::mem::take(&mut n.sent)
        .into_iter()
        .map(|(i, d)| {
            (
                i,
                decode_datagram(&d, false, None, false).expect("datagram"),
            )
        })
        .collect()
}

#[test]
fn reconnect_binds_a_fresh_socket_and_flushes_the_old_disconnect() {
    let net = Arc::new(Mutex::new(Net::default()));
    let config = ChatConfig::new(SERVER, "acct", None, 0);
    let mut client = ChatClient::with_binder(net_binder(&net), config, ChatCodecs::default(), 0.0)
        .expect("client");
    let schema = no_schema();
    let mut server = FakeServer::new();

    client.poll(1.0, &schema).expect("poll");
    assert!(
        sent(&net).iter().all(|(i, _)| *i == 0),
        "socket 0 for attempt 1"
    );
    push(&net, 0, FakeServer::challenge());
    client.poll(1.1, &schema).expect("poll");
    let login = server.event(&ChatEvent::LoginStatus { status: 0 });
    push(&net, 0, login);
    client.poll(1.2, &schema).expect("poll");
    assert!(client.is_ready());

    // The heartbeat at 16.3 stays unacknowledged; blob-free datagrams keep the transport
    // alive without acknowledging it.
    client.poll(16.3, &schema).expect("poll");
    assert_eq!(client.counters().heartbeats, 1);
    for t in [30.0, 55.0] {
        push(&net, 0, server.keepalive(None));
        client.poll(t, &schema).expect("poll");
    }
    let _ = sent(&net);
    // The chat-level drop: the old connection still has a reliable packet to flush, so
    // it keeps socket 0 while a fresh socket is bound for the next attempt.
    client.poll(61.3, &schema).expect("poll");
    assert!(!client.is_ready());
    assert_eq!(client.counters().disconnects, 1);
    assert_eq!(client.draining_len(), 1);
    // Once the server acknowledges, the old session sends its Disconnect on socket 0.
    push(&net, 0, server.keepalive(Some(10_000)));
    client.poll(62.0, &schema).expect("poll");
    let out = sent(&net);
    assert!(
        out.iter().any(|(i, p)| *i == 0 && p.opt_headers.disconnect),
        "Disconnect on the old socket"
    );
    assert!(out.iter().all(|(i, _)| *i == 0));
    assert_eq!(client.draining_len(), 1, "waiting for the server's answer");
    // The server acknowledges the Disconnect: the old connection is let go at once, long
    // before the drain limit.
    push(&net, 0, server.disconnect());
    client.poll(62.5, &schema).expect("poll");
    assert_eq!(client.draining_len(), 0);
    assert_eq!(client.counters().transport_errors, 0);

    // The next attempt authenticates on socket 1; stale traffic on socket 0 never reaches
    // it.
    let ChatPhase::Waiting { until } = client.phase() else {
        panic!("expected a scheduled reconnect");
    };
    push(&net, 0, server.keepalive(None));
    client.poll(until, &schema).expect("poll");
    let out = sent(&net);
    assert!(
        out.iter()
            .any(|(i, p)| *i == 1 && p.opt_headers.session_auth.is_some()),
        "SessionAuth on the fresh socket"
    );
    assert!(
        !out.iter()
            .any(|(i, p)| *i == 0 && p.opt_headers.session_auth.is_some())
    );
    assert_eq!(client.counters().connects, 2);
    // The server gives the new connection the recipient ID of the old one: the handshake
    // still completes and the new session opens.
    let mut server = FakeServer::new();
    push(&net, 1, FakeServer::challenge());
    client.poll(until + 0.1, &schema).expect("poll");
    let login = server.event(&ChatEvent::LoginStatus { status: 0 });
    push(&net, 1, login);
    client.poll(until + 0.2, &schema).expect("poll");
    assert!(client.is_ready(), "the second session opens");
    let _ = sent(&net);
    // The old connection is let go once the session layer is done with it.
    client.poll(until + 46.0, &schema).expect("poll");
    assert_eq!(client.draining_len(), 0);
}

/// Drives a client over the test network to a ready session on socket 0, then to the
/// chat-level drop at 61.3 s with a reliable heartbeat still unacknowledged.
fn ready_then_dropped(
    net: &Arc<Mutex<Net>>,
    fail_rebind: bool,
) -> (ChatClient<NetSocket>, FakeServer) {
    let config = ChatConfig::new(SERVER, "acct", None, 0);
    let mut client = ChatClient::with_binder(net_binder(net), config, ChatCodecs::default(), 0.0)
        .expect("client");
    let schema = no_schema();
    let mut server = FakeServer::new();
    client.poll(1.0, &schema).expect("poll");
    push(net, 0, FakeServer::challenge());
    client.poll(1.1, &schema).expect("poll");
    let login = server.event(&ChatEvent::LoginStatus { status: 0 });
    push(net, 0, login);
    client.poll(1.2, &schema).expect("poll");
    assert!(client.is_ready());
    client.poll(16.3, &schema).expect("poll");
    for t in [30.0, 55.0] {
        push(net, 0, server.keepalive(None));
        client.poll(t, &schema).expect("poll");
    }
    net.lock().expect("net").fail_binds = fail_rebind;
    client.poll(61.3, &schema).expect("poll");
    assert!(!client.is_ready());
    assert_eq!(client.counters().disconnects, 1);
    (client, server)
}

#[test]
fn failed_reads_of_an_old_connection_are_counted() {
    let net = Arc::new(Mutex::new(Net::default()));
    let (mut client, _server) = ready_then_dropped(&net, false);
    assert_eq!(client.draining_len(), 1);
    let before = client.counters();
    net.lock().expect("net").fail_reads = Some(0);
    client.poll(61.5, &no_schema()).expect("poll");
    assert_eq!(client.counters().recv_errors, before.recv_errors + 1);
    assert_eq!(
        client.draining_len(),
        1,
        "a failed read does not end the drain"
    );
    // A datagram the old session refuses is counted as a transport error.
    net.lock().expect("net").fail_reads = None;
    push(&net, 0, vec![0xFF; 3]);
    client.poll(61.6, &no_schema()).expect("poll");
    assert_eq!(
        client.counters().transport_errors,
        before.transport_errors + 1
    );
}

#[test]
fn a_failed_rebind_on_retire_is_not_a_connect_failure() {
    let net = Arc::new(Mutex::new(Net::default()));
    let (client, _server) = ready_then_dropped(&net, true);
    let c = client.counters();
    assert_eq!(c.rebind_failures, 1);
    assert_eq!(c.connect_failures, 0);
    assert_eq!(c.connects, 1);
    assert_eq!(
        client.draining_len(),
        0,
        "without a fresh socket the old connection is let go after one flush"
    );
}

#[test]
fn a_failed_connection_attempt_schedules_a_retry() {
    let net = Arc::new(Mutex::new(Net::default()));
    let config = ChatConfig::new(SERVER, "acct", None, 0);
    let mut client = ChatClient::with_binder(net_binder(&net), config, ChatCodecs::default(), 0.0)
        .expect("client");
    let schema = no_schema();
    client.poll(1.0, &schema).expect("poll");
    assert!(matches!(client.phase(), ChatPhase::Connecting { .. }));
    // No server answers: the handshake times out and a retry is scheduled.
    client.poll(21.5, &schema).expect("poll");
    let ChatPhase::Waiting { until } = client.phase() else {
        panic!("expected a retry, got {:?}", client.phase());
    };
    // The next attempt cannot bind a socket: counted, and a retry is scheduled again
    // instead of hanging in Connecting.
    net.lock().expect("net").fail_binds = true;
    client.poll(until, &schema).expect("poll");
    assert_eq!(client.counters().connect_failures, 1);
    assert!(
        matches!(client.phase(), ChatPhase::Waiting { until: next } if next > until),
        "{:?}",
        client.phase()
    );
    // Binding works again: the following attempt connects.
    net.lock().expect("net").fail_binds = false;
    let ChatPhase::Waiting { until } = client.phase() else {
        unreachable!()
    };
    client.poll(until, &schema).expect("poll");
    assert!(matches!(client.phase(), ChatPhase::Connecting { .. }));
    assert_eq!(client.counters().connects, 2);
}

#[test]
fn set_ticket_validates() {
    let mut config = ChatConfig::new(SERVER, "acct", Some("AAEC".into()), 0);
    assert_eq!(
        config.set_ticket(Some("not base64!".into())),
        Err(PlayError::InvalidTicket)
    );
    assert_eq!(config.set_ticket(Some("AAEF".into())), Ok(()));
    assert_eq!(config.set_ticket(None), Ok(()));
}
