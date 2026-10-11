// SPDX-License-Identifier: GPL-3.0-or-later
//! Named chat channels (`test-server.md` §9.1, protocol minor 2, decision 0035): the
//! registry driven synchronously in the simulation (join, leave, refusals in the order of
//! §9.1, limits, relay to members only, leaving the world, counts), and the answer order of
//! one connection's chat and channel requests in `ConnectionCore` (`tessera-protocol.md`
//! §10.6).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;

use tessera_protocol::codes::{AppErrorCode, ChannelRefusedCode, ChatRejectedCode};
use tessera_protocol::frame::{FrameDecoder, frame_to_vec_minor};
use tessera_protocol::message::*;
use tessera_protocol::server::{ServerEvent, ServerParams};
use tessera_protocol::session::{PeerDatagrams, Transmit};
use tessera_protocol::types::{Appearance, Body, Location, Look, Pose};
use tessera_protocol::{Channel, Message, Side};
use tessera_server::accounts::{AccountStore, AccountsFile, new_account};
use tessera_server::characters::WorldState;
use tessera_server::conn::{ConnectionCore, Inbound};
use tessera_server::shared::{
    ConnShared, HashJob, HashOutcome, HashResult, Outbound, OutboundReceiver, SessionCmd, SimInput,
    Stats, WriterMsg,
};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

const TICK: u64 = 33_334;
const S: u64 = 1_000_000;

struct Harness {
    sim: Sim,
    stats: Arc<Stats>,
    hash_rx: Receiver<HashJob>,
    writer_rx: Receiver<WriterMsg>,
    now: u64,
    conns: Vec<(Arc<ConnShared>, OutboundReceiver)>,
}

fn accounts(muted: &[usize]) -> AccountsFile {
    let mut file = AccountsFile::default();
    for i in 0..8 {
        let n = format!("user{i}");
        let mut a = new_account(n.clone(), "unused".into());
        a.muted = muted.contains(&i);
        file.accounts.insert(n, a);
    }
    file
}

fn content() -> Content {
    Content::parse(
        "format = 1\n[world]\nid = \"t\"\ntitle = \"t\"\nmotd = \"\"\ndefault_spawn = \"s\"\n[[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\n",
    )
    .unwrap()
}

impl Harness {
    fn new(config: Config, muted: &[usize]) -> Self {
        let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(100);
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(10_000);
        let stats = Arc::new(Stats::default());
        let sim = Sim::new(
            SimParts {
                config: Arc::new(config),
                content: Arc::new(content()),
                data_dir: DataDir::new("/nonexistent/tessera-channel-test"),
                accounts: Arc::new(AccountStore::in_memory(accounts(muted))),
                characters: BTreeMap::new(),
                world_state: WorldState::default(),
                hash_jobs: hash_tx,
                writer: writer_tx,
                stats: Arc::clone(&stats),
            },
            0,
        );
        Self {
            sim,
            stats,
            hash_rx,
            writer_rx,
            now: 1_000,
            conns: Vec::new(),
        }
    }

    fn pump(&mut self) {
        while let Ok(job) = self.hash_rx.try_recv() {
            self.sim.handle(
                SimInput::Hash(HashResult {
                    conn: job.conn,
                    account: job.account,
                    outcome: HashOutcome::Verified,
                    register: job.register,
                }),
                self.now,
            );
        }
        while let Ok(w) = self.writer_rx.try_recv() {
            if let WriterMsg::Write(j) = w
                && let Some(token) = j.token
            {
                self.sim
                    .handle(SimInput::WriteDone { token, ok: true }, self.now);
            }
        }
    }

    fn tick(&mut self) {
        self.pump();
        self.now += TICK;
        self.sim.tick(self.now);
        self.pump();
    }

    fn event(&mut self, i: usize, e: ServerEvent) {
        let s = &self.conns[i].0;
        s.inbound_pending.fetch_add(1, Ordering::AcqRel);
        self.sim.handle(SimInput::Event(s.id, e), self.now);
    }

    /// A connection at minor `minor` logged in as `user<i>`; with `name`, its character
    /// created and in the world.
    fn connect(&mut self, name: Option<&str>, minor: u16) -> usize {
        let i = self.conns.len();
        let addr: SocketAddr = format!("127.0.0.1:{}", 6000 + i).parse().unwrap();
        let (shared, rx) = ConnShared::new(i as u64 + 1, addr, 100_000);
        let shared = Arc::new(shared);
        shared.set_minor(minor);
        self.sim
            .handle(SimInput::Opened(Arc::clone(&shared)), self.now);
        self.conns.push((shared, rx));
        self.event(
            i,
            ServerEvent::Greeted(Hello {
                minor_max: minor,
                client_caps: 0,
                max_entities: 1024,
                client_name: String::new(),
            }),
        );
        self.event(
            i,
            ServerEvent::Login {
                credentials: Credentials {
                    account: format!("user{i}"),
                    password: "x".into(),
                },
                register: false,
            },
        );
        self.tick();
        if let Some(name) = name {
            self.event(
                i,
                ServerEvent::CharacterCreate(CharacterCreate {
                    name: name.into(),
                    appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                    look: Look::none(),
                }),
            );
            self.tick();
            self.event(i, ServerEvent::EnterWorld { name: name.into() });
            self.tick();
        }
        let _ = self.drain(i);
        i
    }

    fn player(&mut self, name: &str) -> usize {
        self.connect(Some(name), 2)
    }

    /// The commands and decoded chat-stream messages queued for a connection.
    fn drain(&mut self, i: usize) -> (Vec<SessionCmd>, Vec<Message>) {
        let mut cmds = Vec::new();
        let mut chat = Vec::new();
        let (shared, rx) = &mut self.conns[i];
        let mut dec = FrameDecoder::with_minor(Channel::Chat, Side::Server, shared.minor());
        while let Ok(o) = rx.try_recv() {
            match o {
                Outbound::Cmd(c) => cmds.push(c),
                Outbound::Chat(b) => {
                    shared.chat_unsent.fetch_sub(b.len(), Ordering::Relaxed);
                    let mut input: &[u8] = &b;
                    while let Ok(Some(m)) = dec.push(&mut input) {
                        chat.push(m);
                    }
                }
                Outbound::World(b) => {
                    shared.world_unsent.fetch_sub(b.len(), Ordering::Relaxed);
                }
                Outbound::Datagram(_) => {}
            }
        }
        (cmds, chat)
    }

    /// The channel answers (and chat answers) queued for a connection, in order.
    fn answers(&mut self, i: usize) -> Vec<SessionCmd> {
        self.drain(i)
            .0
            .into_iter()
            .filter(|c| {
                matches!(
                    c,
                    SessionCmd::ChannelJoined(_)
                        | SessionCmd::ChannelLeft(_)
                        | SessionCmd::RefuseChannel { .. }
                        | SessionCmd::RejectChat(_)
                        | SessionCmd::ChatDelivered
                        | SessionCmd::Disconnect(..)
                )
            })
            .collect()
    }

    fn join(&mut self, i: usize, name: &str) {
        self.event(i, ServerEvent::ChannelJoin { name: name.into() });
    }

    fn leave(&mut self, i: usize, name: &str) {
        self.event(i, ServerEvent::ChannelLeave { name: name.into() });
    }

    fn say(&mut self, i: usize, channel: &str, text: &str) {
        self.event(
            i,
            ServerEvent::Chat(ChatSend {
                channel: ChatChannel::Named {
                    name: channel.into(),
                },
                text: text.into(),
            }),
        );
    }

    fn counts(&self) -> (usize, usize) {
        let s = self.stats.snapshot();
        (s.channels, s.channel_memberships)
    }
}

fn refused(code: ChannelRefusedCode, name: &str) -> SessionCmd {
    SessionCmd::RefuseChannel {
        code,
        name: name.into(),
    }
}

fn lines(chat: &[Message]) -> Vec<(String, String, bool, String)> {
    chat.iter()
        .filter_map(|m| match m {
            Message::ChatMessage(ChatMessage {
                channel: ChatChannel::Named { name },
                echo,
                sender,
                text,
                ..
            }) => Some((name.clone(), sender.clone(), *echo, text.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn join_relay_and_leave() {
    let mut h = Harness::new(Config::default(), &[]);
    let a = h.player("Aldith");
    let b = h.player("Bram");
    let c = h.player("Cade");
    // The first join creates the channel with its spelling; a later join matches by key.
    h.join(a, "Lore Keepers");
    h.join(b, "LORE keepers");
    h.tick();
    assert_eq!(
        h.answers(a),
        [SessionCmd::ChannelJoined("Lore Keepers".into())]
    );
    assert_eq!(
        h.answers(b),
        [SessionCmd::ChannelJoined("Lore Keepers".into())]
    );
    assert_eq!(h.counts(), (1, 2));
    // A line reaches the members only, the speaker with ECHO, under the channel's spelling.
    h.say(b, "lore keepers", "hello");
    h.tick();
    let (cmds_b, chat_b) = h.drain(b);
    let (_, chat_a) = h.drain(a);
    let (_, chat_c) = h.drain(c);
    assert!(cmds_b.contains(&SessionCmd::ChatDelivered), "{cmds_b:?}");
    let want = |echo| {
        vec![(
            "Lore Keepers".to_owned(),
            "Bram".to_owned(),
            echo,
            "hello".to_owned(),
        )]
    };
    assert_eq!(lines(&chat_a), want(false));
    assert_eq!(lines(&chat_b), want(true));
    assert!(lines(&chat_c).is_empty());
    // A non-member's line is refused with NOT_JOINED, also for a name breaking the rule.
    h.say(c, "Lore Keepers", "let me in");
    h.say(c, "bad\u{7}name", "x");
    h.tick();
    assert_eq!(
        h.answers(c),
        [
            SessionCmd::RejectChat(ChatRejectedCode::NotJoined),
            SessionCmd::RejectChat(ChatRejectedCode::NotJoined)
        ]
    );
    let (_, chat_a) = h.drain(a);
    assert!(lines(&chat_a).is_empty());
    // Leave: the answer names the channel's spelling; the last leave drops the channel.
    h.leave(a, "lore keepers");
    h.tick();
    assert_eq!(
        h.answers(a),
        [SessionCmd::ChannelLeft("Lore Keepers".into())]
    );
    assert_eq!(h.counts(), (1, 1));
    h.say(b, "Lore Keepers", "alone");
    h.tick();
    let (_, chat_a) = h.drain(a);
    assert!(lines(&chat_a).is_empty());
    let (_, chat_b) = h.drain(b);
    assert_eq!(lines(&chat_b).len(), 1);
    h.leave(b, "Lore Keepers");
    h.tick();
    assert_eq!(
        h.answers(b),
        [SessionCmd::ChannelLeft("Lore Keepers".into())]
    );
    assert_eq!(h.counts(), (0, 0));
    // A new join of the key creates a new channel with the new spelling.
    h.join(c, "lore KEEPERS");
    h.tick();
    assert_eq!(
        h.answers(c),
        [SessionCmd::ChannelJoined("lore KEEPERS".into())]
    );
}

#[test]
fn refusals_in_the_order_of_9_1() {
    let mut cfg = Config::default();
    cfg.chat.max_channels = 2;
    cfg.chat.max_channels_per_character = 2;
    cfg.chat.max_channel_members = 2;
    let mut h = Harness::new(cfg, &[]);
    let a = h.player("Aldith");
    let b = h.player("Bram");
    let c = h.player("Cade");
    // Invalid names, then a join, the same channel again, a leave of another channel.
    h.join(a, "");
    h.join(a, "Lo\u{7}re");
    h.join(a, "fellowship");
    h.join(a, "Lore");
    h.tick();
    assert_eq!(
        h.answers(a),
        [
            refused(ChannelRefusedCode::InvalidName, ""),
            refused(ChannelRefusedCode::InvalidName, "Lo\u{7}re"),
            refused(ChannelRefusedCode::InvalidName, "fellowship"),
            SessionCmd::ChannelJoined("Lore".into()),
        ]
    );
    // The bucket (5, one per 6 s) is spent: wait for it to refill.
    h.now += 30 * S;
    h.join(a, "lore");
    h.leave(a, "Elsewhere");
    h.leave(a, "!!");
    h.tick();
    assert_eq!(
        h.answers(a),
        [
            refused(ChannelRefusedCode::AlreadyJoined, "lore"),
            refused(ChannelRefusedCode::NotJoined, "Elsewhere"),
            refused(ChannelRefusedCode::InvalidName, "!!"),
        ]
    );
    // Per character, server-wide, members.
    h.join(a, "Second");
    h.join(a, "Third");
    h.join(b, "Third");
    h.join(b, "Lore");
    h.tick();
    // (Connections are served round-robin within a tick: Cade's join comes after.)
    h.join(c, "Lore");
    h.tick();
    assert_eq!(
        h.answers(a),
        [
            SessionCmd::ChannelJoined("Second".into()),
            refused(ChannelRefusedCode::TooManyChannels, "Third"),
        ]
    );
    assert_eq!(
        h.answers(b),
        [
            refused(ChannelRefusedCode::ServerLimit, "Third"),
            SessionCmd::ChannelJoined("Lore".into()),
        ]
    );
    assert_eq!(
        h.answers(c),
        [refused(ChannelRefusedCode::ChannelFull, "Lore")]
    );
    assert_eq!(h.counts(), (2, 3));
    // A connection that is not placed in the world.
    let d = h.connect(None, 2);
    h.join(d, "Lore");
    h.tick();
    assert_eq!(
        h.answers(d),
        [refused(ChannelRefusedCode::NotInWorld, "Lore")]
    );
}

#[test]
fn a_burst_of_joins_is_rate_limited_until_flooding() {
    let mut h = Harness::new(Config::default(), &[]);
    let a = h.player("Aldith");
    for i in 0..30 {
        h.join(a, &format!("Burst{i}"));
    }
    h.tick();
    let answers = h.answers(a);
    let joined = answers
        .iter()
        .filter(|c| matches!(c, SessionCmd::ChannelJoined(_)))
        .count();
    let limited = answers
        .iter()
        .filter(|c| {
            matches!(
                c,
                SessionCmd::RefuseChannel {
                    code: ChannelRefusedCode::RateLimited,
                    ..
                }
            )
        })
        .count();
    assert_eq!(joined, 5, "{answers:?}");
    // The 20th RATE_LIMITED refusal ends the session (flood_rejections 20 in 60 s).
    assert_eq!(limited, 20, "{answers:?}");
    assert_eq!(
        answers.last(),
        Some(&SessionCmd::Disconnect(
            AppErrorCode::Flooding,
            String::new()
        ))
    );
    // Leaving the world (the disconnect) ended every membership.
    assert_eq!(h.counts(), (0, 0));
}

#[test]
fn leaving_the_world_ends_memberships_silently() {
    let mut h = Harness::new(Config::default(), &[0]);
    let a = h.player("Aldith");
    let b = h.player("Bram");
    h.join(a, "Lore");
    h.join(b, "Lore");
    h.join(a, "Solo");
    h.tick();
    let _ = (h.answers(a), h.answers(b));
    assert_eq!(h.counts(), (2, 3));
    // The muted account may join, but its lines are refused.
    h.say(a, "Lore", "hi");
    h.tick();
    assert_eq!(
        h.answers(a),
        [SessionCmd::RejectChat(ChatRejectedCode::Muted)]
    );
    // Aldith's connection closes: Bram is told nothing, "Solo" is dropped.
    let id = h.conns[a].0.id;
    h.sim.handle(SimInput::Closed(id), h.now);
    h.tick();
    let (cmds, chat) = h.drain(b);
    assert!(lines(&chat).is_empty());
    assert!(
        !cmds.iter().any(|c| matches!(c, SessionCmd::ChannelLeft(_))),
        "{cmds:?}"
    );
    assert_eq!(h.counts(), (1, 1));
    h.say(b, "Lore", "anyone?");
    h.tick();
    let (_, chat) = h.drain(b);
    assert_eq!(lines(&chat).len(), 1);
    let id = h.conns[b].0.id;
    h.sim.handle(SimInput::Closed(id), h.now);
    h.tick();
    // §14 invariant 14: after every client has left the world there is no channel.
    assert_eq!(h.counts(), (0, 0));
}

/// The echo flag and sender entity of each Named line (`tessera-protocol.md` §6.4.1, §10.6).
fn sender_entities(chat: &[Message]) -> Vec<(bool, u64)> {
    chat.iter()
        .filter_map(|m| match m {
            Message::ChatMessage(ChatMessage {
                channel: ChatChannel::Named { .. },
                echo,
                sender_entity,
                ..
            }) => Some((*echo, *sender_entity)),
            _ => None,
        })
        .collect()
}

#[test]
fn requests_queued_before_a_close_keep_their_order() {
    // §10.6 "Order": a connection's ChatSend, ChannelJoin and ChannelLeave are handled in
    // arrival order, also when the connection closes before the next tick.
    let mut h = Harness::new(Config::default(), &[]);
    let a = h.player("Aldith");
    let b = h.player("Bram");
    let c = h.player("Cade");
    h.join(a, "Lore");
    h.join(b, "Lore");
    h.tick();
    let _ = (h.answers(a), h.answers(b));
    assert_eq!(h.counts(), (1, 2));
    // Leave, then a line, then the close, all before one tick: the line is not relayed.
    h.leave(a, "Lore");
    h.say(a, "Lore", "after my leave");
    let id = h.conns[a].0.id;
    h.sim.handle(SimInput::Closed(id), h.now);
    h.tick();
    let (_, chat_b) = h.drain(b);
    assert!(lines(&chat_b).is_empty(), "{:?}", lines(&chat_b));
    assert_eq!(h.counts(), (1, 1));
    // Join, then a line, then the close: the line reaches the members.
    h.join(c, "Lore");
    h.say(c, "Lore", "hello and goodbye");
    let id = h.conns[c].0.id;
    h.sim.handle(SimInput::Closed(id), h.now);
    h.tick();
    let (_, chat_b) = h.drain(b);
    assert_eq!(
        lines(&chat_b),
        [(
            "Lore".to_owned(),
            "Cade".to_owned(),
            false,
            "hello and goodbye".to_owned()
        )]
    );
    // The closed connection's membership ended with it (§14 invariant 14).
    assert_eq!(h.counts(), (1, 1));
}

#[test]
fn sender_entity_is_zero_where_the_speaker_is_not_created() {
    // Two instances of two places: Aldith and Cade share instance 1, Bram is in instance 2.
    let mut cfg = Config::default();
    cfg.world.instances = 2;
    cfg.world.instance_capacity = 2;
    let mut h = Harness::new(cfg, &[]);
    let a = h.player("Aldith");
    let c = h.player("Cade");
    let b = h.player("Bram");
    for i in [a, b, c] {
        h.join(i, "Lore");
    }
    h.tick();
    let _ = (h.answers(a), h.answers(b), h.answers(c));
    h.say(a, "Lore", "across instances");
    h.tick();
    let (_, chat_a) = h.drain(a);
    let (_, chat_b) = h.drain(b);
    let (_, chat_c) = h.drain(c);
    let echo = sender_entities(&chat_a);
    assert_eq!(echo.len(), 1, "{echo:?}");
    let (is_echo, eid) = echo[0];
    assert!(is_echo);
    assert_ne!(eid, 0, "the speaker's echo carries its own entity ID");
    // Cade has Aldith created (same instance, same place): the speaker's ID.
    assert_eq!(sender_entities(&chat_c), [(false, eid)]);
    // Bram, in the other instance, does not: 0.
    assert_eq!(sender_entities(&chat_b), [(false, 0)]);
}

#[test]
fn channels_key_controls_the_capability() {
    let mut cfg = Config::default();
    let params = tessera_server::server::server_params(&cfg, &content());
    assert!(params.caps.contains(ServerCaps::CHAT_CHANNELS));
    cfg.chat.channels = false;
    let params = tessera_server::server::server_params(&cfg, &content());
    assert!(!params.caps.contains(ServerCaps::CHAT_CHANNELS));
}

// --- ConnectionCore: answer order (tessera-protocol.md §10.6) ---

fn frame(m: &Message) -> Vec<u8> {
    frame_to_vec_minor(m, 2).unwrap()
}

fn core_in_world(minor: u16) -> ConnectionCore {
    let params = ServerParams {
        caps: ServerCaps::CHAT_CHANNELS,
        ..ServerParams::default()
    };
    let mut core = ConnectionCore::new(params, 0);
    core.handshake_complete(0, PeerDatagrams::MaxPayload(1_162));
    let ev = core.on_control_data(
        &frame_to_vec_minor(
            &Message::Hello(Hello {
                minor_max: minor,
                client_caps: 0,
                max_entities: 1024,
                client_name: "t".into(),
            }),
            0,
        )
        .unwrap(),
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
    assert_eq!(ev.len(), 1);
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
    assert_eq!(ev.len(), 1);
    core.apply(
        SessionCmd::EnterWorld(Box::new(WorldEnter {
            player: 7,
            name: "Aldith".into(),
            instance: 1,
            epoch: 1,
            server_time: 5,
            pose: Pose {
                location: Location::new(1, 1000.0, 1000.0, 0.0),
                heading: 0.0,
                anchor: None,
            },
            appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
            look: Look::none(),
        })),
        5,
    );
    while core.poll_transmit().is_some() {}
    core
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

#[test]
fn channel_answers_keep_the_order_of_requests() {
    let mut core = core_in_world(2);
    assert_eq!(core.minor(), 2);
    let mut bytes = frame(&Message::ChannelJoin {
        name: "Lore".into(),
    });
    // An invalid text: the session rejects it itself, behind the forwarded join.
    bytes.extend(frame(&Message::ChatSend(ChatSend {
        channel: ChatChannel::Say,
        text: String::new(),
    })));
    bytes.extend(frame(&Message::ChannelLeave {
        name: "Other".into(),
    }));
    let ev = core.on_control_data(&bytes, 10);
    assert!(matches!(
        ev.as_slice(),
        [
            Inbound::Event(ServerEvent::ChannelJoin { .. }),
            Inbound::Event(ServerEvent::ChannelLeave { .. })
        ]
    ));
    assert!(sent(&mut core).is_empty(), "the rejection waits");
    assert_eq!(core.unanswered_chats(), 2);
    core.apply(SessionCmd::ChannelJoined("Lore".into()), 11);
    let out = sent(&mut core);
    assert!(matches!(
        out.as_slice(),
        [
            Message::ChannelJoined { name },
            Message::ChatRejected(Failure { code: ChatRejectedCode::InvalidText, .. })
        ] if name == "Lore"
    ));
    core.apply(
        SessionCmd::RefuseChannel {
            code: ChannelRefusedCode::NotJoined,
            name: "Oth\u{7}er".into(),
        },
        12,
    );
    let out = sent(&mut core);
    assert!(matches!(
        out.as_slice(),
        [Message::ChannelRefused(ChannelRefused { code: ChannelRefusedCode::NotJoined, name, .. })]
            if name == "Oth er"
    ));
    assert_eq!(core.unanswered_chats(), 0);
}

#[test]
fn channel_messages_below_minor_2_are_violations() {
    let mut core = core_in_world(1);
    assert_eq!(core.minor(), 1);
    // The ChannelJoin frame is unknown at minor 1.
    let ev = core.on_control_data(
        &frame(&Message::ChannelJoin {
            name: "Lore".into(),
        }),
        10,
    );
    assert!(
        matches!(
            ev.as_slice(),
            [Inbound::Ended {
                code: AppErrorCode::ProtocolViolation,
                violation: true,
                ..
            }]
        ),
        "{ev:?}"
    );
    // A simulation answer for minor 2 is dropped on a minor-1 connection.
    let mut core = core_in_world(1);
    core.apply(SessionCmd::ChannelJoined("Lore".into()), 11);
    assert!(sent(&mut core).is_empty());
}
