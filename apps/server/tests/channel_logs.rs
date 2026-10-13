// SPDX-License-Identifier: GPL-3.0-or-later
//! The channel events of the log (`test-server.md` §11.1, §11.2): a channel's name appears
//! with every channel event (created, joined, left, dropped) only when `log_chat` is true.
//!
//! This binary holds one test and installs a global subscriber, because `tracing` caches
//! whether a call site is enabled across threads: a scoped subscriber in a test that runs
//! next to others can miss events.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use tessera_protocol::message::*;
use tessera_protocol::server::ServerEvent;
use tessera_protocol::types::{Appearance, Body, Look};
use tessera_server::accounts::{AccountStore, AccountsFile, new_account};
use tessera_server::characters::WorldState;
use tessera_server::shared::{
    ConnShared, HashJob, HashOutcome, HashResult, OutboundReceiver, SimInput, Stats, WriterMsg,
};
use tessera_server::sim::{Sim, SimParts};
use tessera_server::{Config, Content, DataDir};

const TICK: u64 = 33_334;

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Run {
    sim: Sim,
    stats: Arc<Stats>,
    hash_rx: Receiver<HashJob>,
    writer_rx: Receiver<WriterMsg>,
    shared: Arc<ConnShared>,
    /// Kept open so the simulation's answers have somewhere to go; not read here.
    _out: OutboundReceiver,
    now: u64,
}

impl Run {
    fn new(log_chat: bool) -> Self {
        let mut config = Config::default();
        config.chat.log_chat = log_chat;
        let content = Content::parse(
            "format = 1\n[world]\nid = \"t\"\ntitle = \"t\"\nmotd = \"\"\ndefault_spawn = \"s\"\n[[spawn]]\nname = \"s\"\nregion = 1\nlocation = [1000.0, 1000.0, 0.0]\n",
        )
        .unwrap();
        let mut file = AccountsFile::default();
        file.accounts
            .insert("user0".into(), new_account("user0".into(), "unused".into()));
        let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(100);
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(10_000);
        let stats = Arc::new(Stats::default());
        let sim = Sim::new(
            SimParts {
                config: Arc::new(config),
                content: Arc::new(content),
                data_dir: DataDir::new("/nonexistent/tessera-channel-log-test"),
                accounts: Arc::new(AccountStore::in_memory(file)),
                characters: BTreeMap::new(),
                world_state: WorldState::default(),
                hash_jobs: hash_tx,
                writer: writer_tx,
                stats: Arc::clone(&stats),
            },
            0,
        );
        let addr: SocketAddr = "127.0.0.1:6000".parse().unwrap();
        let (shared, out) = ConnShared::new(1, addr, 100_000);
        let shared = Arc::new(shared);
        shared.set_minor(2);
        let mut run = Self {
            sim,
            stats,
            hash_rx,
            writer_rx,
            shared,
            _out: out,
            now: 1_000,
        };
        run.sim
            .handle(SimInput::Opened(Arc::clone(&run.shared)), run.now);
        run.event(ServerEvent::Greeted(Hello {
            minor_max: 2,
            client_caps: 0,
            max_entities: 1024,
            client_name: String::new(),
        }));
        run.event(ServerEvent::Login {
            credentials: Credentials {
                account: "user0".into(),
                password: "x".into(),
            },
            register: false,
        });
        run.tick();
        run.event(ServerEvent::CharacterCreate(CharacterCreate {
            name: "Aldith".into(),
            appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
            look: Look::none(),
        }));
        run.tick();
        run.event(ServerEvent::EnterWorld {
            name: "Aldith".into(),
        });
        run.tick();
        run
    }

    fn event(&mut self, e: ServerEvent) {
        self.shared.inbound_pending.fetch_add(1, Ordering::AcqRel);
        self.sim
            .handle(SimInput::Event(self.shared.id, e), self.now);
    }

    fn tick(&mut self) {
        self.pump();
        self.now += TICK;
        self.sim.tick(self.now);
        self.pump();
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

    fn counts(&self) -> (usize, usize) {
        let s = self.stats.snapshot();
        (s.channels, s.channel_memberships)
    }
}

/// The log lines of one join and leave of "Lorekeepers" that are channel events.
fn channel_events(buf: &Buf, log_chat: bool) -> Vec<String> {
    buf.0.lock().unwrap().clear();
    let mut run = Run::new(log_chat);
    run.event(ServerEvent::ChannelJoin {
        name: "Lorekeepers".into(),
    });
    run.tick();
    assert_eq!(run.counts(), (1, 1));
    run.event(ServerEvent::ChannelLeave {
        name: "Lorekeepers".into(),
    });
    run.tick();
    assert_eq!(run.counts(), (0, 0));
    let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    text.lines()
        .filter(|l| l.contains("channel "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn log_chat_names_the_channel_with_every_channel_event() {
    let buf = Buf::default();
    let writer = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let named = channel_events(&buf, true);
    for event in [
        "channel created",
        "channel joined",
        "channel left",
        "channel dropped",
    ] {
        let line = named
            .iter()
            .find(|l| l.contains(event))
            .unwrap_or_else(|| panic!("no {event:?} line in {named:?}"));
        assert!(line.contains("Lorekeepers"), "{line}");
    }

    // §11.2: without `log_chat` no channel name reaches the log.
    let plain = channel_events(&buf, false);
    for event in [
        "channel created",
        "channel joined",
        "channel left",
        "channel dropped",
    ] {
        assert!(plain.iter().any(|l| l.contains(event)), "{plain:?}");
    }
    assert!(
        !plain.iter().any(|l| l.contains("Lorekeepers")),
        "{plain:?}"
    );
}
