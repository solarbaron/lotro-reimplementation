// SPDX-License-Identifier: GPL-3.0-or-later
//! tessera-server's whole inbound pipeline without sockets: up to four connections, each a
//! `ConnectionCore` (decoding and the protocol state machine), feeding the simulation
//! (`Sim`: logins, characters, world entry, movement validation, interest, chat, teleports,
//! flow control) with fuzzed raw bytes and fuzzed-but-well-formed messages, with ticks and
//! hash and writer results answered by the harness. It must never panic, and everything the
//! server sends must decode as valid server-to-client protocol.

#![no_main]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_protocol::frame::{FrameDecoder, decode_datagram, encode_datagram, frame_to_vec};
use tessera_protocol::message::*;
use tessera_protocol::session::{PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body, Destination, Gait, Location, Motion, Vec3};
use tessera_protocol::value::Stance;
use tessera_protocol::{Channel, Message, Side};
use tessera_server::accounts::{AccountStore, AccountsFile, new_account};
use tessera_server::characters::WorldState;
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::server::server_params;
use tessera_server::shared::{
    ConnShared, HashOutcome, HashResult, Outbound, OutboundReceiver, SimInput, Stats, WriterMsg,
};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

struct Peer {
    core: ConnectionCore,
    shared: Arc<ConnShared>,
    out: OutboundReceiver,
    world: FrameDecoder,
    chat: FrameDecoder,
    control: FrameDecoder,
    open: bool,
    seq: u32,
}

fn check_frames(dec: &mut FrameDecoder, bytes: &[u8]) {
    let mut input = bytes;
    loop {
        match dec.push(&mut input) {
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(e) => panic!("server sent an invalid frame: {e}"),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut config = Config::default();
    config.world.teleport_interval_ms = 0;
    config.accounts.attempts_per_address_per_min = 10_000;
    config.world.instance_capacity = 2;
    config.world.instances = 2;
    let content = Content::default_world();
    let params = server_params(&config, &content);
    let mut file = AccountsFile::default();
    for n in ["alice", "bob", "carol", "dave"] {
        file.accounts.insert(
            n.into(),
            new_account(
                n.into(),
                "$argon2id$v=19$m=64,t=1,p=1$AAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAA".into(),
            ),
        );
    }
    let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(100);
    let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(10_000);
    let stats = Arc::new(Stats::default());
    let mut sim = Sim::new(
        SimParts {
            config: Arc::new(config),
            content: Arc::new(content),
            data_dir: DataDir::new("/nonexistent/tessera-fuzz"),
            accounts: Arc::new(AccountStore::in_memory(file)),
            characters: BTreeMap::new(),
            world_state: WorldState::default(),
            hash_jobs: hash_tx,
            writer: writer_tx,
            stats,
        },
        0,
    );
    let mut peers: Vec<Peer> = Vec::new();
    for i in 0..4u64 {
        let addr: SocketAddr = format!("127.0.0.{}:4000", i + 1).parse().unwrap();
        let (shared, out) = ConnShared::new(i + 1, addr, 4096);
        let shared = Arc::new(shared);
        let mut core = ConnectionCore::new(params.clone(), 0);
        let _ = core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162));
        sim.handle(SimInput::Opened(Arc::clone(&shared)), 0);
        peers.push(Peer {
            core,
            shared,
            out,
            world: FrameDecoder::new(Channel::World, Side::Server),
            chat: FrameDecoder::new(Channel::Chat, Side::Server),
            control: FrameDecoder::new(Channel::Control, Side::Server),
            open: true,
            seq: 0,
        });
    }
    let mut now = 1_000u64;
    let mut rest = data;
    while let Some((&op, tail)) = rest.split_first() {
        let len =
            usize::from(tail.first().copied().unwrap_or(0) % 64).min(tail.len().saturating_sub(1));
        let chunk = tail.get(1..1 + len).unwrap_or_default();
        rest = tail.get(1 + len..).unwrap_or_default();
        let b = |i: usize| chunk.get(i).copied().unwrap_or(0);
        let f = |i: usize| f64::from(i8::from_le_bytes([b(i)])) * 3.0;
        let p = usize::from(op >> 5) % peers.len();
        let text = String::from_utf8_lossy(chunk).into_owned();
        let msg = match op % 24 {
            0 => Some(Message::Hello(Hello {
                minor_max: 0,
                client_caps: 0,
                max_entities: 64 + u16::from(b(0)),
                client_name: text.clone(),
            })),
            1 => Some(Message::Login(Credentials {
                account: ["alice", "bob", "carol", "dave", "x y"][usize::from(b(0)) % 5].into(),
                password: "pw".into(),
            })),
            2 => Some(Message::CharacterCreate(CharacterCreate {
                name: ["Aldith", "Bram", "Cade", "Dun", "", "A1"][usize::from(b(0)) % 6].into(),
                appearance: Appearance::new(if b(1) & 1 == 0 {
                    Body::Character {
                        species: u32::from(b(2)),
                        sex: 1,
                    }
                } else {
                    Body::Object {
                        did: 1 + u32::from(b(2)),
                    }
                }),
                look: tessera_protocol::types::Look::none(),
            })),
            3 => Some(Message::EnterWorld(EnterWorld {
                name: ["Aldith", "Bram", "Cade", "Dun", "zz"][usize::from(b(0)) % 5].into(),
            })),
            4 => Some(Message::ChatSend(ChatSend {
                channel: match b(0) % 4 {
                    0 => ChatChannel::Say,
                    1 => ChatChannel::Yell,
                    2 => ChatChannel::Emote,
                    _ => ChatChannel::Tell {
                        peer: "bram".into(),
                    },
                },
                text: if text.is_empty() {
                    "x".into()
                } else {
                    text.clone()
                },
            })),
            5 => Some(Message::TeleportRequest(TeleportRequest {
                request_id: 1 + u32::from(b(0)),
                destination: match b(1) % 4 {
                    0 => Destination::Named {
                        name: ["start", "void", "@aldith", "@bram", "nowhere"]
                            [usize::from(b(2)) % 5]
                            .into(),
                    },
                    1 => Destination::Location {
                        location: Location::new(
                            u16::from(b(2) % 3),
                            20_240.0 + f(3) * 50.0,
                            22_420.0 + f(4) * 50.0,
                            440.0,
                        ),
                        heading: 0.0,
                    },
                    2 => Destination::Ground {
                        location: Location::new(1, 20_240.0 + f(3), 22_420.0 + f(4), 0.0),
                        heading: 1.0,
                    },
                    _ => Destination::Named { name: text.clone() },
                },
            })),
            6 => Some(Message::QueryLocation {
                request_id: 1 + u32::from(b(0)),
            }),
            7 => Some(Message::EmoteSet(EmoteSet {
                anim: u32::from(b(0)),
                looping: b(1) & 1 == 1,
            })),
            8 => Some(Message::StanceSet(
                Stance::from_u8(b(0) % 3).unwrap_or_default(),
            )),
            9 => Some(Message::AppearanceSet(Appearance::new(Body::Character {
                species: u32::from(b(0)),
                sex: 2,
            }))),
            10 => Some(Message::Ping {
                nonce: u64::from(b(0)),
            }),
            11 => Some(Message::Disconnect(Disconnect {
                code: tessera_protocol::codes::AppErrorCode::NoError,
                text: String::new(),
            })),
            _ => None,
        };
        let peer = &mut peers[p];
        let mut inbound: Vec<Inbound> = Vec::new();
        match op % 24 {
            0..=11 => {
                if let Some(m) = msg
                    && let Ok(bytes) = frame_to_vec(&m)
                {
                    inbound = peer.core.on_control_data(&bytes, now);
                }
            }
            12 | 13 => {
                peer.seq = peer.seq.wrapping_add(1 + u32::from(b(5) % 2));
                let m = Message::MoveInput(MoveInput {
                    seq: peer.seq,
                    epoch: u32::from(b(0) % 4),
                    location: Location::new(
                        u16::from(b(1) % 2),
                        20_240.0 + f(2) * 10.0,
                        22_420.0 + f(3) * 10.0,
                        440.0 + f(4),
                    ),
                    heading: f(6) as f32,
                    velocity: Vec3::new(f(2) as f32, f(3) as f32, 0.0),
                    motion: Motion::new(
                        [Gait::Standing, Gait::Walking, Gait::Running, Gait::Airborne]
                            [usize::from(b(7)) % 4],
                        false,
                    ),
                });
                if let Ok(bytes) = encode_datagram(&m) {
                    inbound = peer.core.on_datagram(&bytes, now);
                }
            }
            14 => inbound = peer.core.on_control_data(chunk, now),
            15 => inbound = peer.core.on_datagram(chunk, now),
            16 => inbound = peer.core.handle_timeout(now),
            _ => {
                now += u64::from(b(0)) * 33_333 + 33_333;
                // Answer hash and writer jobs, then tick.
                while let Ok(job) = hash_rx.try_recv() {
                    let outcome = if b(1) & 1 == 0 {
                        HashOutcome::Verified
                    } else {
                        HashOutcome::BadCredentials
                    };
                    sim.handle(
                        SimInput::Hash(HashResult {
                            conn: job.conn,
                            account: job.account,
                            outcome,
                            register: job.register,
                        }),
                        now,
                    );
                }
                while let Ok(w) = writer_rx.try_recv() {
                    if let WriterMsg::Write(j) = w
                        && let Some(token) = j.token
                    {
                        sim.handle(
                            SimInput::WriteDone {
                                token,
                                ok: b(2) != 255,
                            },
                            now,
                        );
                    }
                }
                sim.tick(now);
            }
        }
        for i in inbound {
            match i {
                Inbound::Greeted(h) => sim.handle(
                    SimInput::Event(
                        peer.shared.id,
                        tessera_protocol::server::ServerEvent::Greeted(h),
                    ),
                    now,
                ),
                Inbound::Event(e) => {
                    peer.shared
                        .inbound_pending
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    sim.handle(SimInput::Event(peer.shared.id, e), now);
                }
                Inbound::Move(m) => peer.shared.put_move(m),
                Inbound::Ended { .. } => {
                    if peer.open {
                        peer.open = false;
                        sim.handle(SimInput::Closed(peer.shared.id), now);
                    }
                }
            }
        }
        // Deliver the simulation's output to every connection and check it decodes.
        for peer in &mut peers {
            while let Ok(o) = peer.out.try_recv() {
                match o {
                    Outbound::Cmd(c) => {
                        let ib = peer.core.apply(c, now);
                        if ib.iter().any(|i| matches!(i, Inbound::Ended { .. })) && peer.open {
                            peer.open = false;
                            sim.handle(SimInput::Closed(peer.shared.id), now);
                        }
                    }
                    Outbound::World(bytes) => {
                        peer.shared
                            .world_unsent
                            .fetch_sub(bytes.len(), std::sync::atomic::Ordering::Relaxed);
                        check_frames(&mut peer.world, &bytes);
                    }
                    Outbound::Chat(bytes) => {
                        peer.shared
                            .chat_unsent
                            .fetch_sub(bytes.len(), std::sync::atomic::Ordering::Relaxed);
                        check_frames(&mut peer.chat, &bytes);
                    }
                    Outbound::Datagram(bytes) => {
                        decode_datagram(Side::Server, &bytes).expect("server datagram decodes");
                    }
                }
            }
            while let Some(t) = peer.core.poll_transmit() {
                if let Transmit::Message(m) = t {
                    let bytes = match m.channel() {
                        Channel::Datagram => {
                            decode_datagram(Side::Server, &encode_datagram(&m).expect("encodable"))
                                .expect("decodes");
                            continue;
                        }
                        _ => frame_to_vec(&m).expect("encodable"),
                    };
                    match m.channel() {
                        Channel::World => check_frames(&mut peer.world, &bytes),
                        Channel::Chat => check_frames(&mut peer.chat, &bytes),
                        _ => check_frames(&mut peer.control, &bytes),
                    }
                }
            }
        }
    }
});
