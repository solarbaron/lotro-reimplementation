// SPDX-License-Identifier: GPL-3.0-or-later
//! The simulation driven synchronously, without threads or sockets: slow consumers, the
//! memory budget, interest budgets and wanderers (`test-server.md` §5.4, §8, §10).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;

use tessera_protocol::codes::AppErrorCode;
use tessera_protocol::frame::FrameDecoder;
use tessera_protocol::message::*;
use tessera_protocol::server::ServerEvent;
use tessera_protocol::types::{Appearance, Body};
use tessera_protocol::{Channel, Message, Side};
use tessera_server::accounts::{AccountStore, AccountsFile, new_account};
use tessera_server::characters::{
    CharacterFile, CharacterRecord, StoredAppearance, StoredPosition, WorldState,
};
use tessera_server::shared::{
    ConnShared, HashJob, HashOutcome, HashResult, Outbound, OutboundReceiver, SessionCmd, SimInput,
    Stats, WriterMsg,
};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

const TICK: u64 = 33_334;

struct Harness {
    sim: Sim,
    hash_rx: Receiver<HashJob>,
    writer_rx: Receiver<WriterMsg>,
    now: u64,
    conns: Vec<(Arc<ConnShared>, OutboundReceiver)>,
    accounts: Arc<AccountStore>,
    /// The outcome every hash job gets.
    outcome: HashOutcome,
    /// Every file write, in order.
    writes: Vec<(std::path::PathBuf, String)>,
}

fn accounts_file() -> AccountsFile {
    let mut file = AccountsFile::default();
    for i in 0..8 {
        let n = format!("user{i}");
        file.accounts
            .insert(n.clone(), new_account(n, "unused".into()));
    }
    file
}

impl Harness {
    fn new(config: Config, content: Content) -> Self {
        Self::with(config, content, accounts_file(), BTreeMap::new())
    }

    fn with(
        config: Config,
        content: Content,
        file: AccountsFile,
        characters: BTreeMap<String, CharacterFile>,
    ) -> Self {
        let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(100);
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(10_000);
        let accounts = Arc::new(AccountStore::in_memory(file));
        let sim = Sim::new(
            SimParts {
                config: Arc::new(config),
                content: Arc::new(content),
                data_dir: DataDir::new("/nonexistent/tessera-sim-test"),
                accounts: Arc::clone(&accounts),
                characters,
                world_state: WorldState::default(),
                hash_jobs: hash_tx,
                writer: writer_tx,
                stats: Arc::new(Stats::default()),
            },
            0,
        );
        Self {
            sim,
            hash_rx,
            writer_rx,
            now: 1_000,
            conns: Vec::new(),
            accounts,
            outcome: HashOutcome::Verified,
            writes: Vec::new(),
        }
    }

    fn pump(&mut self) {
        while let Ok(job) = self.hash_rx.try_recv() {
            self.sim.handle(
                SimInput::Hash(HashResult {
                    conn: job.conn,
                    account: job.account,
                    outcome: self.outcome,
                    register: job.register,
                }),
                self.now,
            );
        }
        while let Ok(w) = self.writer_rx.try_recv() {
            if let WriterMsg::Write(j) = w {
                self.writes.push((
                    j.path.clone(),
                    String::from_utf8_lossy(&j.contents).into_owned(),
                ));
                if let Some(token) = j.token {
                    self.sim
                        .handle(SimInput::WriteDone { token, ok: true }, self.now);
                }
            }
        }
    }

    /// The last write of `account`'s character file.
    fn last_save(&self, account: &str) -> Option<&str> {
        self.writes
            .iter()
            .rev()
            .find(|(p, _)| p.ends_with(format!("characters/{account}.toml")))
            .map(|(_, t)| t.as_str())
    }

    /// Opens a connection (and greets it); returns its index.
    fn open(&mut self) -> usize {
        let i = self.conns.len();
        let addr: SocketAddr = format!("127.0.0.1:{}", 5000 + i).parse().unwrap();
        let (shared, rx) = ConnShared::new(i as u64 + 1, addr, 100_000);
        let shared = Arc::new(shared);
        self.sim
            .handle(SimInput::Opened(Arc::clone(&shared)), self.now);
        self.conns.push((shared, rx));
        self.event(
            i,
            ServerEvent::Greeted(Hello {
                minor_max: 0,
                client_caps: 0,
                max_entities: 1024,
                client_name: String::new(),
            }),
        );
        i
    }

    fn login(&mut self, i: usize, account: &str) {
        self.event(
            i,
            ServerEvent::Login {
                credentials: Credentials {
                    account: account.into(),
                    password: "x".into(),
                },
                register: false,
            },
        );
        self.tick();
    }

    fn close(&mut self, i: usize) {
        let id = self.conns[i].0.id;
        self.sim.handle(SimInput::Closed(id), self.now);
    }

    /// Chat-stream messages queued for a connection (and the commands, kept aside).
    fn drain_chat(&mut self, i: usize) -> (Vec<SessionCmd>, Vec<Message>) {
        let mut cmds = Vec::new();
        let mut chat = Vec::new();
        let mut dec = FrameDecoder::new(Channel::Chat, Side::Server);
        let (shared, rx) = &mut self.conns[i];
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

    /// Opens a connection and brings a character into the world; returns its index.
    fn player(&mut self, name: &str, max_entities: u16) -> usize {
        let i = self.conns.len();
        let addr: SocketAddr = format!("127.0.0.1:{}", 5000 + i).parse().unwrap();
        let (shared, rx) = ConnShared::new(i as u64 + 1, addr, 100_000);
        let shared = Arc::new(shared);
        self.sim
            .handle(SimInput::Opened(Arc::clone(&shared)), self.now);
        self.conns.push((shared, rx));
        self.event(
            i,
            ServerEvent::Greeted(Hello {
                minor_max: 0,
                client_caps: 0,
                max_entities,
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
        self.event(
            i,
            ServerEvent::CharacterCreate(CharacterCreate {
                name: name.into(),
                appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
            }),
        );
        self.tick();
        self.event(i, ServerEvent::EnterWorld { name: name.into() });
        self.tick();
        i
    }

    /// Drains a connection's outbound queue; returns the commands and decoded world frames.
    fn drain(&mut self, i: usize) -> (Vec<SessionCmd>, Vec<Message>) {
        let mut cmds = Vec::new();
        let mut world = Vec::new();
        let mut dec = FrameDecoder::new(Channel::World, Side::Server);
        let (shared, rx) = &mut self.conns[i];
        while let Ok(o) = rx.try_recv() {
            match o {
                Outbound::Cmd(c) => cmds.push(c),
                Outbound::World(b) => {
                    shared.world_unsent.fetch_sub(b.len(), Ordering::Relaxed);
                    let mut input: &[u8] = &b;
                    while let Ok(Some(m)) = dec.push(&mut input) {
                        world.push(m);
                    }
                }
                Outbound::Chat(b) => {
                    shared.chat_unsent.fetch_sub(b.len(), Ordering::Relaxed);
                }
                Outbound::Datagram(_) => {}
            }
        }
        (cmds, world)
    }
}

fn plain() -> Content {
    Content::parse(
        "format = 1\n[world]\nid = \"t\"\ntitle = \"t\"\nmotd = \"\"\ndefault_spawn = \"s\"\n[[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\n",
    )
    .unwrap()
}

#[test]
fn players_enter_and_see_each_other() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    let b = h.player("Bram", 1024);
    h.tick();
    let (cmds, world) = h.drain(a);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SessionCmd::AcceptLogin { .. }))
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SessionCmd::CharacterCreated(_)))
    );
    assert!(cmds.iter().any(|c| matches!(c, SessionCmd::EnterWorld(_))));
    assert!(
        world
            .iter()
            .any(|m| matches!(m, Message::EntityCreate(c) if c.name == "Bram"))
    );
    let (_, world) = h.drain(b);
    assert!(
        world
            .iter()
            .any(|m| matches!(m, Message::EntityCreate(c) if c.name == "Aldith"))
    );
}

#[test]
fn slow_consumers_are_disconnected() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    let _ = h.drain(a);
    // Above 4 MiB unsent on the world stream: disconnected at once.
    h.conns[a].0.world_unsent.store(5 << 20, Ordering::Relaxed);
    h.tick();
    let (cmds, _) = h.drain(a);
    assert!(
        cmds.contains(&SessionCmd::Disconnect(
            AppErrorCode::SlowConsumer,
            String::new()
        )),
        "{cmds:?}"
    );

    // Above 1 MiB: creates pause; still above it after 30 s: disconnected.
    let b = h.player("Bram", 1024);
    let _ = h.drain(b);
    h.conns[b].0.world_unsent.store(2 << 20, Ordering::Relaxed);
    h.tick();
    let c = h.player("Cade", 1024);
    let _ = h.drain(c);
    for _ in 0..30 {
        h.tick();
    }
    let (_, world) = h.drain(b);
    assert!(
        !world
            .iter()
            .any(|m| matches!(m, Message::EntityCreate(x) if x.name == "Cade")),
        "creates pause while the backlog is above 1 MiB"
    );
    h.conns[b].0.world_unsent.store(2 << 20, Ordering::Relaxed);
    for _ in 0..(31 * 30) {
        h.tick();
    }
    let (cmds, _) = h.drain(b);
    assert!(cmds.contains(&SessionCmd::Disconnect(
        AppErrorCode::SlowConsumer,
        String::new()
    )));
}

#[test]
fn backlog_below_resume_continues_creates() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    h.conns[a].0.world_unsent.store(2 << 20, Ordering::Relaxed);
    h.tick();
    let _ = h.player("Bram", 1024);
    h.tick();
    let (_, world) = h.drain(a);
    assert!(!world.iter().any(|m| matches!(m, Message::EntityCreate(_))));
    h.conns[a].0.world_unsent.store(0, Ordering::Relaxed);
    // The pause ends at the next flow-control check, then creates resume.
    h.tick();
    h.tick();
    let (_, world) = h.drain(a);
    assert!(
        world
            .iter()
            .any(|m| matches!(m, Message::EntityCreate(x) if x.name == "Bram"))
    );
}

#[test]
fn memory_budget_disconnects_the_largest_backlog() {
    let mut cfg = Config::default();
    cfg.limits.memory_budget_mib = 64;
    cfg.use_hash_floor();
    cfg.accounts.hash_workers = 1;
    let mut h = Harness::new(cfg, plain());
    let a = h.player("Aldith", 1024);
    let b = h.player("Bram", 1024);
    let _ = h.drain(a);
    let _ = h.drain(b);
    // 19 MiB of hashing plus 48 MiB of chat backlog on Bram: over 64 MiB.
    h.conns[b].0.chat_unsent.store(48 << 20, Ordering::Relaxed);
    h.conns[a].0.chat_unsent.store(10, Ordering::Relaxed);
    h.tick();
    let (cmds_b, _) = h.drain(b);
    let (cmds_a, _) = h.drain(a);
    assert!(cmds_b.contains(&SessionCmd::Disconnect(
        AppErrorCode::SlowConsumer,
        String::new()
    )));
    assert!(
        !cmds_a
            .iter()
            .any(|c| matches!(c, SessionCmd::Disconnect(..))),
        "{cmds_a:?}"
    );
}

#[test]
fn entity_limit_and_creates_per_tick() {
    let mut cfg = Config::default();
    cfg.world.creates_per_tick = 2;
    let content = Content::parse(
        "format = 1\n[world]\nid = \"t\"\ntitle = \"t\"\nmotd = \"\"\ndefault_spawn = \"s\"\n[[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\n\
         [[wanderers]]\nspawn = \"s\"\ncount = 200\nradius = 5.0\nspeed = 1.0\nappearance = \"first-player\"\n",
    )
    .unwrap();
    let mut h = Harness::new(cfg, content);
    let a = h.player("Aldith", 64);
    let (_, world) = h.drain(a);
    let creates = world
        .iter()
        .filter(|m| matches!(m, Message::EntityCreate(_)))
        .count();
    assert!(creates <= 2, "{creates} creates in one tick");
    for _ in 0..100 {
        h.tick();
    }
    let (_, world) = h.drain(a);
    let creates = world
        .iter()
        .filter(|m| matches!(m, Message::EntityCreate(_)))
        .count();
    // The client's max_entities (64) bounds what is created at once.
    assert!(creates <= 64, "{creates}");
    assert!(creates >= 60, "{creates}");
}

// ---------------------------------------------------------------------- regressions

use tessera_protocol::codes::{CharacterCreateFailedCode, ChatRejectedCode, LoginFailedCode};
use tessera_protocol::types::{Location, Motion, Pose, Vec3};
use tessera_server::shared::{SimControl, StopReport};

fn mv(seq: u32, location: Location) -> MoveInput {
    MoveInput {
        seq,
        epoch: 1,
        location,
        heading: 0.0,
        velocity: Vec3::ZERO,
        motion: Motion::STANDING,
    }
}

fn player_pose(h: &mut Harness, i: usize) -> Pose {
    let (cmds, _) = h.drain(i);
    cmds.iter()
        .find_map(|c| match c {
            SessionCmd::EnterWorld(w) => Some(w.pose),
            _ => None,
        })
        .expect("entered")
}

/// Finding: a name whose NFC form exceeds the protocol's 64-byte NAME was created and saved,
/// and every later answer carrying it was unencodable.
#[test]
fn nfc_names_beyond_the_name_limit_are_refused() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.open();
    h.login(a, "user0");
    let name: String = std::iter::repeat_n('\u{FA6C}', 21).collect();
    h.event(
        a,
        ServerEvent::CharacterCreate(CharacterCreate {
            name,
            appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
        }),
    );
    h.tick();
    let (cmds, _) = h.drain(a);
    assert!(
        cmds.contains(&SessionCmd::RejectCharacter(
            CharacterCreateFailedCode::NameInvalid
        )),
        "{cmds:?}"
    );
    assert!(h.last_save("user0").is_none());
}

/// Finding: events queued before the connection closed (a final `ChatSend`, `MoveInput`)
/// were dropped with the connection.
#[test]
fn events_sent_before_closing_take_effect() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    let b = h.player("Bram", 1024);
    h.tick();
    let start = player_pose(&mut h, a).location;
    let _ = h.drain_chat(b);
    h.now += 500_000;
    h.event(
        a,
        ServerEvent::Chat(ChatSend {
            channel: ChatChannel::Say,
            text: "bye".into(),
        }),
    );
    let moved = Location::new(start.region, start.x + 1.5, start.y, start.z);
    h.conns[a].0.put_move(mv(1, moved));
    // Disconnect arrives before the next tick.
    h.close(a);
    let (_, chat) = h.drain_chat(b);
    assert!(
        chat.iter()
            .any(|m| matches!(m, Message::ChatMessage(c) if c.text == "bye")),
        "{chat:?}"
    );
    h.pump();
    let saved = h.last_save("user0").expect("saved when leaving");
    assert!(
        saved.contains(&format!("x = {:?}", moved.x)),
        "the last MoveInput is applied before the save: {saved}"
    );
}

/// Finding: delivered chat lines did not tell the session they were answered, so
/// session-made rejections of later lines could not be ordered behind them.
#[test]
fn delivered_chat_is_reported_to_the_session() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    let _ = h.drain(a);
    h.event(
        a,
        ServerEvent::Chat(ChatSend {
            channel: ChatChannel::Say,
            text: "hello".into(),
        }),
    );
    h.event(
        a,
        ServerEvent::Chat(ChatSend {
            channel: ChatChannel::Tell {
                peer: "Nobody".into(),
            },
            text: "hello".into(),
        }),
    );
    h.tick();
    let (cmds, _) = h.drain(a);
    let chat: Vec<&SessionCmd> = cmds
        .iter()
        .filter(|c| matches!(c, SessionCmd::ChatDelivered | SessionCmd::RejectChat(_)))
        .collect();
    assert_eq!(
        chat,
        vec![
            &SessionCmd::ChatDelivered,
            &SessionCmd::RejectChat(ChatRejectedCode::UnknownRecipient)
        ]
    );
}

/// Finding: failures were counted only for known accounts, so a lockout (RATE_LIMITED)
/// told known names from unknown ones.
#[test]
fn lockouts_do_not_reveal_which_accounts_exist() {
    let mut cfg = Config::default();
    cfg.accounts.lockout_after = 2;
    let mut h = Harness::new(cfg, plain());
    h.outcome = HashOutcome::BadCredentials;
    let mut answers = Vec::new();
    for account in ["user7", "ghost"] {
        for _ in 0..3 {
            let c = h.open();
            h.login(c, account);
            let (cmds, _) = h.drain(c);
            answers.extend(cmds.into_iter().filter_map(|c| match c {
                SessionCmd::RejectLogin { code, .. } => Some((account, code)),
                _ => None,
            }));
        }
    }
    assert_eq!(
        answers,
        vec![
            ("user7", LoginFailedCode::BadCredentials),
            ("user7", LoginFailedCode::BadCredentials),
            ("user7", LoginFailedCode::RateLimited),
            ("ghost", LoginFailedCode::BadCredentials),
            ("ghost", LoginFailedCode::BadCredentials),
            ("ghost", LoginFailedCode::RateLimited),
        ]
    );
}

/// Finding: an address bucket was evicted while still refilling and replaced by a full one.
#[test]
fn address_budget_survives_bucket_eviction() {
    let mut cfg = Config::default();
    cfg.accounts.lockout_base_s = 1;
    cfg.accounts.lockout_max_s = 1;
    cfg.accounts.attempts_per_address_per_min = 1;
    let mut h = Harness::new(cfg, plain());
    let a = h.open();
    h.login(a, "user0");
    let (cmds, _) = h.drain(a);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SessionCmd::AcceptLogin { .. }))
    );
    // 1.1 s later (beyond lockout_max_s, far below the 60 s refill), from the same address.
    for _ in 0..33 {
        h.tick();
    }
    let b = h.open();
    h.login(b, "user1");
    let (cmds, _) = h.drain(b);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            SessionCmd::RejectLogin {
                code: LoginFailedCode::RateLimited,
                ..
            }
        )),
        "{cmds:?}"
    );
}

/// Finding: an accepted appearance change waited for the periodic save.
#[test]
fn appearance_changes_are_saved_at_once() {
    let mut h = Harness::new(Config::default(), plain());
    let a = h.player("Aldith", 1024);
    h.pump();
    h.event(
        a,
        ServerEvent::Appearance(Appearance::new(Body::Character {
            species: 4242,
            sex: 1,
        })),
    );
    h.tick();
    let saved = h.last_save("user0").unwrap();
    assert!(saved.contains("species = 4242"), "{saved}");
}

fn two_ground_spawns() -> Content {
    Content::parse(
        "format = 1\n[world]\nid = \"t\"\ntitle = \"t\"\nmotd = \"\"\ndefault_spawn = \"s\"\n\
         [[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\nground = true\n\
         [[spawn]]\nname = \"u\"\nregion = 1\nlocation = [3000.0, 3000.0, 100.0]\nground = true\n",
    )
    .unwrap()
}

fn stored(account: &str, name: &str, instance: u32, location: Location) -> CharacterFile {
    CharacterFile {
        account: account.into(),
        characters: vec![CharacterRecord {
            name: name.into(),
            created: "2026-10-07T00:00:00Z".into(),
            instance,
            position: Some(StoredPosition {
                location,
                heading: 0.0,
                anchor: None,
            }),
            appearance: StoredAppearance::Valid(Appearance::new(Body::Character {
                species: 1,
                sex: 1,
            })),
        }],
    }
}

/// Finding: the settling grace applied only to the default spawn with instance 0, not to a
/// character `character move --spawn` had put on another ground spawn.
#[test]
fn settling_grace_applies_at_every_ground_spawn() {
    let at_spawn = Location::new(1, 3000.0, 3000.0, 100.0);
    let elsewhere = Location::new(1, 3010.0, 3000.0, 100.0);
    let mut chars = BTreeMap::new();
    chars.insert("user0".to_owned(), stored("user0", "Bram", 1, at_spawn));
    chars.insert("user1".to_owned(), stored("user1", "Cade", 1, elsewhere));
    let mut h = Harness::with(
        Config::default(),
        two_ground_spawns(),
        accounts_file(),
        chars,
    );
    let mut corrected = Vec::new();
    for (i, (account, name, loc)) in [("user0", "Bram", at_spawn), ("user1", "Cade", elsewhere)]
        .into_iter()
        .enumerate()
    {
        let c = h.open();
        assert_eq!(c, i);
        h.login(c, account);
        h.event(c, ServerEvent::EnterWorld { name: name.into() });
        h.tick();
        let _ = h.drain(c);
        h.now += 100_000;
        h.conns[c]
            .0
            .put_move(mv(1, Location::new(1, loc.x, loc.y, loc.z - 150.0)));
        h.tick();
        let (_, world) = h.drain(c);
        corrected.push(
            world
                .iter()
                .any(|m| matches!(m, Message::LocationReport(_))),
        );
    }
    assert_eq!(corrected, vec![false, true]);
}

/// Finding: shutdown saves used `try_send` and were skipped when the writer queue was full,
/// yet counted as saved.
#[test]
fn shutdown_saves_wait_for_a_full_writer_queue() {
    let mut chars = BTreeMap::new();
    for i in 0..4 {
        let account = format!("user{i}");
        let name = ["Aldith", "Bram", "Cade", "Dora"][i];
        chars.insert(
            account.clone(),
            stored(&account, name, 1, Location::new(1, 1000.0, 1000.0, 0.0)),
        );
    }
    let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel::<HashJob>(100);
    // A writer queue of one message, drained slowly by another thread.
    let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel::<WriterMsg>(1);
    let mut sim = Sim::new(
        SimParts {
            config: Arc::new(Config::default()),
            content: Arc::new(plain()),
            data_dir: DataDir::new("/nonexistent/tessera-sim-test"),
            accounts: Arc::new(AccountStore::in_memory(accounts_file())),
            characters: chars,
            world_state: WorldState::default(),
            hash_jobs: hash_tx,
            writer: writer_tx,
            stats: Arc::new(Stats::default()),
        },
        0,
    );
    let mut conns = Vec::new();
    let mut now = 1_000;
    for i in 0..4usize {
        let addr: SocketAddr = format!("127.0.0.1:{}", 6000 + i).parse().unwrap();
        let (shared, rx) = ConnShared::new(i as u64 + 1, addr, 100_000);
        let shared = Arc::new(shared);
        sim.handle(SimInput::Opened(Arc::clone(&shared)), now);
        sim.handle(
            SimInput::Event(
                shared.id,
                ServerEvent::Login {
                    credentials: Credentials {
                        account: format!("user{i}"),
                        password: "x".into(),
                    },
                    register: false,
                },
            ),
            now,
        );
        now += TICK;
        sim.tick(now);
        while let Ok(job) = hash_rx.try_recv() {
            sim.handle(
                SimInput::Hash(HashResult {
                    conn: job.conn,
                    account: job.account,
                    outcome: HashOutcome::Verified,
                    register: false,
                }),
                now,
            );
        }
        let name = ["Aldith", "Bram", "Cade", "Dora"][i];
        sim.handle(
            SimInput::Event(shared.id, ServerEvent::EnterWorld { name: name.into() }),
            now,
        );
        now += TICK;
        sim.tick(now);
        conns.push((shared, rx));
    }
    // Fill the queue so every save below must wait.
    while let Ok(m) = writer_rx.try_recv() {
        drop(m);
    }
    let writer = std::thread::spawn(move || {
        let mut saved = Vec::new();
        while let Ok(m) = writer_rx.recv() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            match m {
                WriterMsg::Write(j) => saved.push(j.path),
                WriterMsg::Barrier(reply) => {
                    let _ = reply.send(0);
                    return saved;
                }
                WriterMsg::Stop => return saved,
            }
        }
        saved
    });
    let (tx, rx) = std::sync::mpsc::channel::<StopReport>();
    sim.handle(SimInput::Control(SimControl::Stop(tx)), now);
    let report = rx.recv().unwrap();
    let saved = writer.join().unwrap();
    assert_eq!(report.saved, 4, "{report:?}");
    assert_eq!(report.failures, 0);
    for i in 0..4 {
        assert!(
            saved
                .iter()
                .any(|p| p.ends_with(format!("characters/user{i}.toml"))),
            "{saved:?}"
        );
    }
    assert!(saved.iter().any(|p| p.ends_with("world-state.toml")));
}

/// Spec behaviours without a test before: `muted` (§9), kicks after an accounts reload
/// (§3.7), the idle timeout (§8.1) and the shutdown notice (§12.1).
#[test]
fn muted_kicked_idle_and_notices() {
    let mut file = accounts_file();
    file.accounts.get_mut("user0").unwrap().muted = true;
    let mut cfg = Config::default();
    cfg.world.idle_timeout_min = 1;
    let mut h = Harness::with(cfg, plain(), file, BTreeMap::new());
    let a = h.player("Aldith", 1024);
    let b = h.player("Bram", 1024);
    let c = h.player("Cade", 1024);
    let _ = (h.drain(a), h.drain(b), h.drain(c));
    h.event(
        a,
        ServerEvent::Chat(ChatSend {
            channel: ChatChannel::Say,
            text: "hi".into(),
        }),
    );
    h.tick();
    let (cmds, _) = h.drain(a);
    assert!(
        cmds.contains(&SessionCmd::RejectChat(ChatRejectedCode::Muted)),
        "{cmds:?}"
    );
    // Shutdown notice, also for a grace of 0 s.
    h.sim.handle(
        SimInput::Control(SimControl::ShutdownNotice { seconds: 0 }),
        h.now,
    );
    let (_, chat) = h.drain_chat(b);
    assert!(
        chat.iter().any(|m| matches!(
            m,
            Message::ServerNotice(n) if n.level == NoticeLevel::Shutdown && n.seconds == 0
        )),
        "{chat:?}"
    );
    // Disabled in the accounts file: kicked at the next tick.
    h.accounts
        .modify(|f| {
            f.accounts.get_mut("user1").unwrap().disabled = true;
            Ok(())
        })
        .unwrap();
    h.tick();
    let (cmds, _) = h.drain(b);
    assert!(
        cmds.contains(&SessionCmd::Disconnect(AppErrorCode::Kicked, String::new())),
        "{cmds:?}"
    );
    // Idle for a minute: disconnected with IDLE. Cade keeps moving.
    for step in 1..=4u32 {
        h.now += 15_000_000;
        let loc = Location::new(1, 1000.0 + f64::from(step), 1000.0, 0.0);
        h.conns[c].0.put_move(mv(step, loc));
        h.tick();
    }
    let (cmds_a, _) = h.drain(a);
    let (cmds_c, _) = h.drain(c);
    assert!(
        cmds_a.contains(&SessionCmd::Disconnect(AppErrorCode::Idle, String::new())),
        "{cmds_a:?}"
    );
    assert!(
        !cmds_c
            .iter()
            .any(|x| matches!(x, SessionCmd::Disconnect(..))),
        "{cmds_c:?}"
    );
}
