// SPDX-License-Identifier: GPL-3.0-or-later
//! tessera-server's channel registry end to end without sockets (`test-server.md` §9.1,
//! `tessera-protocol.md` §10.6): up to four connections, each a `ConnectionCore` offering
//! minor 0, 1 or 2 (chosen by the input), logged in and placed in the world as the input
//! drives them, then sending `ChannelJoin`, `ChannelLeave` and `ChatSend` lines on a small
//! set of channel names (some breaking the name rule) to a simulation with small channel
//! limits, with ticks, closes and raw bytes in between.
//!
//! It must never panic, and after every step:
//! - the channel count stays within `max_channels`, every channel has a member, and the
//!   memberships stay within the per-character and per-channel limits (§14 invariant 14);
//! - a `Named` line reaches only a connection that was told it joined that channel and has
//!   not been told it left it (§14 invariant 15), and never one below minor 2;
//! - everything the server sends decodes at the connection's negotiated minor;
//! - once every connection has closed, no channel and no membership is left.

#![no_main]

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use libfuzzer_sys::fuzz_target;
use tessera_protocol::frame::{FrameDecoder, decode_datagram, encode_datagram, frame_to_vec_minor};
use tessera_protocol::message::*;
use tessera_protocol::server::ServerEvent;
use tessera_protocol::session::{PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body};
use tessera_protocol::{Channel, Message, Side};
use tessera_server::accounts::{AccountStore, AccountsFile, new_account};
use tessera_server::characters::WorldState;
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::names::channel_key;
use tessera_server::server::server_params;
use tessera_server::shared::{
    ConnShared, HashOutcome, HashResult, Outbound, OutboundReceiver, SessionCmd, SimInput, Stats,
    WriterMsg,
};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

const PEERS: usize = 4;
const MAX_CHANNELS: u32 = 2;
const MAX_PER_CHARACTER: u32 = 2;
const MAX_MEMBERS: u32 = 3;
const NAMES: [&str; 6] = ["Lore", "LORE", "Hunt", "Tea Party", "", "bad\u{7}name"];
const ACCOUNTS: [&str; PEERS] = ["alice", "bob", "carol", "dave"];
const CHARACTERS: [&str; PEERS] = ["Aldith", "Bram", "Cade", "Dun"];

struct Peer {
    core: ConnectionCore,
    shared: Arc<ConnShared>,
    out: OutboundReceiver,
    world: FrameDecoder,
    chat: FrameDecoder,
    control: FrameDecoder,
    offer: u16,
    open: bool,
    /// Channel keys the connection was told it joined and not yet told it left.
    joined: BTreeSet<String>,
}

fn decode_all(dec: &mut FrameDecoder, bytes: &[u8]) -> Vec<Message> {
    let mut out = Vec::new();
    let mut input = bytes;
    loop {
        match dec.push(&mut input) {
            Ok(Some(m)) => out.push(m),
            Ok(None) => break,
            Err(e) => panic!("server sent an invalid frame: {e}"),
        }
    }
    out
}

fn close(sim: &mut Sim, peer: &mut Peer, now: u64) {
    if peer.open {
        peer.open = false;
        peer.joined.clear();
        sim.handle(SimInput::Closed(peer.shared.id), now);
    }
}

fn tick(
    sim: &mut Sim,
    hash_rx: &std::sync::mpsc::Receiver<tessera_server::shared::HashJob>,
    writer_rx: &std::sync::mpsc::Receiver<WriterMsg>,
    now: u64,
) {
    while let Ok(job) = hash_rx.try_recv() {
        sim.handle(
            SimInput::Hash(HashResult {
                conn: job.conn,
                account: job.account,
                outcome: HashOutcome::Verified,
                register: job.register,
            }),
            now,
        );
    }
    while let Ok(w) = writer_rx.try_recv() {
        if let WriterMsg::Write(j) = w
            && let Some(token) = j.token
        {
            sim.handle(SimInput::WriteDone { token, ok: true }, now);
        }
    }
    sim.tick(now);
}

/// Delivers the simulation's output to every connection, checks it and tracks memberships.
fn deliver(sim: &mut Sim, peers: &mut [Peer], now: u64) {
    for peer in peers.iter_mut() {
        while let Ok(o) = peer.out.try_recv() {
            match o {
                Outbound::Cmd(c) => {
                    match &c {
                        SessionCmd::ChannelJoined(name) => {
                            assert!(peer.shared.minor() >= 2, "ChannelJoined below minor 2");
                            peer.joined.insert(channel_key(name));
                        }
                        SessionCmd::ChannelLeft(name) => {
                            assert!(peer.shared.minor() >= 2, "ChannelLeft below minor 2");
                            peer.joined.remove(&channel_key(name));
                        }
                        _ => {}
                    }
                    let ib = peer.core.apply(c, now);
                    if ib.iter().any(|i| matches!(i, Inbound::Ended { .. })) {
                        close(sim, peer, now);
                    }
                }
                Outbound::World(bytes) => {
                    peer.shared
                        .world_unsent
                        .fetch_sub(bytes.len(), Ordering::Relaxed);
                    let _ = decode_all(&mut peer.world, &bytes);
                }
                Outbound::Chat(bytes) => {
                    peer.shared
                        .chat_unsent
                        .fetch_sub(bytes.len(), Ordering::Relaxed);
                    for m in decode_all(&mut peer.chat, &bytes) {
                        if let Message::ChatMessage(ChatMessage {
                            channel: ChatChannel::Named { name },
                            ..
                        }) = &m
                        {
                            assert!(peer.shared.minor() >= 2, "Named line below minor 2");
                            // A line queued before the connection ended (for example the
                            // speaker's own echo, queued just before its session ended it
                            // for flooding) never reaches the wire: only an open
                            // connection's lines are checked against its memberships.
                            assert!(
                                !peer.open || peer.joined.contains(&channel_key(name)),
                                "Named line on {name:?} to a non-member"
                            );
                        }
                    }
                }
                Outbound::Datagram(bytes) => {
                    decode_datagram(Side::Server, &bytes).expect("server datagram decodes");
                }
            }
        }
        let minor = peer.core.minor();
        while let Some(t) = peer.core.poll_transmit() {
            if let Transmit::Message(m) = t {
                if m.channel() == Channel::Datagram {
                    decode_datagram(Side::Server, &encode_datagram(&m).expect("encodable"))
                        .expect("decodes");
                    continue;
                }
                let bytes = frame_to_vec_minor(&m, minor).expect("encodable at the minor");
                let dec = match m.channel() {
                    Channel::World => &mut peer.world,
                    Channel::Chat => &mut peer.chat,
                    _ => &mut peer.control,
                };
                let _ = decode_all(dec, &bytes);
            }
        }
    }
}

fn check_counts(stats: &Stats) {
    let s = stats.snapshot();
    let max_channels = MAX_CHANNELS as usize;
    assert!(s.channels <= max_channels, "{} channels", s.channels);
    assert!(
        s.channel_memberships >= s.channels,
        "a channel without members"
    );
    assert!(s.channel_memberships <= s.channels * MAX_MEMBERS as usize);
    assert!(s.channel_memberships <= PEERS * MAX_PER_CHARACTER as usize);
}

fuzz_target!(|data: &[u8]| {
    let Some((&offers, mut rest)) = data.split_first() else {
        return;
    };
    let mut config = Config::default();
    config.accounts.attempts_per_address_per_min = 10_000;
    config.chat.max_channels = MAX_CHANNELS;
    config.chat.max_channels_per_character = MAX_PER_CHARACTER;
    config.chat.max_channel_members = MAX_MEMBERS;
    let content = Content::default_world();
    let params = server_params(&config, &content);
    let mut file = AccountsFile::default();
    for n in ACCOUNTS {
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
            stats: Arc::clone(&stats),
        },
        0,
    );
    let mut peers: Vec<Peer> = Vec::new();
    for i in 0..PEERS {
        let addr: SocketAddr = format!("127.0.0.{}:4000", i + 1).parse().unwrap();
        let (shared, out) = ConnShared::new(i as u64 + 1, addr, 4096);
        let shared = Arc::new(shared);
        let mut core = ConnectionCore::new(params.clone(), 0);
        let _ = core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162));
        sim.handle(SimInput::Opened(Arc::clone(&shared)), 0);
        // Two bits per peer: 0 and 3 offer minor 2, 1 offers minor 1, 2 offers minor 0.
        let offer = match (offers >> (2 * i)) & 3 {
            1 => 1,
            2 => 0,
            _ => 2,
        };
        peers.push(Peer {
            core,
            shared,
            out,
            world: FrameDecoder::new(Channel::World, Side::Server),
            chat: FrameDecoder::new(Channel::Chat, Side::Server),
            control: FrameDecoder::new(Channel::Control, Side::Server),
            offer,
            open: true,
            joined: BTreeSet::new(),
        });
    }
    let mut now = 1_000u64;
    let mut steps = 0usize;
    while let Some((&op, tail)) = rest.split_first() {
        steps += 1;
        if steps > 4_096 {
            break;
        }
        let arg = tail.first().copied().unwrap_or(0);
        rest = tail.get(1..).unwrap_or_default();
        let p = usize::from(op >> 5) % PEERS;
        let name = NAMES[usize::from(arg) % NAMES.len()].to_owned();
        let peer = &mut peers[p];
        let minor = peer.core.minor();
        let msg = match op % 16 {
            0 => Some(Message::Hello(Hello {
                minor_max: peer.offer,
                client_caps: 0,
                max_entities: 1024,
                client_name: String::new(),
            })),
            1 => Some(Message::Login(Credentials {
                account: ACCOUNTS[p].into(),
                password: "pw".into(),
            })),
            2 => Some(Message::CharacterCreate(CharacterCreate {
                name: CHARACTERS[p].into(),
                appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                look: tessera_protocol::types::Look::none(),
            })),
            3 => Some(Message::EnterWorld(EnterWorld {
                name: CHARACTERS[p].into(),
            })),
            4 | 5 => Some(Message::ChannelJoin { name }),
            6 => Some(Message::ChannelLeave { name }),
            7 | 8 => Some(Message::ChatSend(ChatSend {
                channel: ChatChannel::Named { name },
                text: "hello".into(),
            })),
            9 => Some(Message::ChatSend(ChatSend {
                channel: ChatChannel::Say,
                text: "hi".into(),
            })),
            10 => Some(Message::Disconnect(Disconnect {
                code: tessera_protocol::codes::AppErrorCode::NoError,
                text: String::new(),
            })),
            _ => None,
        };
        let mut inbound: Vec<Inbound> = Vec::new();
        match op % 16 {
            0..=10 => {
                if let Some(m) = msg {
                    // Channel messages are framed at minor 2 even on a lower-minor
                    // connection, where they must be violations.
                    let at = if matches!(op % 16, 4..=8) { 2 } else { minor };
                    if let Ok(bytes) = frame_to_vec_minor(&m, at) {
                        inbound = peer.core.on_control_data(&bytes, now);
                    }
                }
            }
            11 => {
                close(&mut sim, peer, now);
            }
            12 => {
                let chunk: Vec<u8> = rest.iter().take(usize::from(arg % 16)).copied().collect();
                rest = rest.get(chunk.len()..).unwrap_or_default();
                inbound = peer.core.on_control_data(&chunk, now);
            }
            _ => {
                now += u64::from(arg % 8) * 33_333 + 33_333;
                tick(&mut sim, &hash_rx, &writer_rx, now);
            }
        }
        for i in inbound {
            match i {
                Inbound::Greeted(h) => {
                    let m = peer.core.minor();
                    peer.shared.set_minor(m);
                    peer.chat.set_minor(m);
                    peer.world.set_minor(m);
                    peer.control.set_minor(m);
                    sim.handle(
                        SimInput::Event(peer.shared.id, ServerEvent::Greeted(h)),
                        now,
                    );
                }
                Inbound::Event(e) => {
                    peer.shared.inbound_pending.fetch_add(1, Ordering::AcqRel);
                    sim.handle(SimInput::Event(peer.shared.id, e), now);
                }
                Inbound::Move(m) => peer.shared.put_move(m),
                Inbound::Ended { .. } => close(&mut sim, peer, now),
            }
        }
        deliver(&mut sim, &mut peers, now);
        check_counts(&stats);
    }
    // Everyone leaves: no channel and no membership remains (§14 invariant 14).
    for peer in &mut peers {
        close(&mut sim, peer, now);
    }
    now += 33_334;
    tick(&mut sim, &hash_rx, &writer_rx, now);
    deliver(&mut sim, &mut peers, now);
    let s = stats.snapshot();
    assert_eq!((s.channels, s.channel_memberships), (0, 0));
});
