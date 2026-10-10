// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol 1.1 against an in-process `tessera-server` (decision 0025): character looks
//! stored and relayed, held-item slots, minor-0 observers, the hostile 1.1 vectors of
//! `tessera-protocol.md` §16.8, and the overlay run of `scenario-feed.md` §12.5 whose
//! equipment other clients see (§14.9 O9, P1 exit criterion 3).
//!
//! Needs no game install. Skips itself (reporting "passed", printing `SKIPPED`) only where
//! loopback UDP is unavailable.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::domain::{CharacterCommand, CharacterEvent};
use tessera_net::remote::{ClientConfig, Password, RemoteBackend, RemoteConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, Family, LabelContext, OverlayBackend, Scenario,
    SelectorQuery, SelectorResolver,
};
use tessera_net::{
    Appearance, Backend, Body, Command, DataId, EntityId, Equipped, Event, Look, LookEntry,
    PropertyValue, Session,
};
use tessera_server::accounts::{self, HashParams};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

const PASSWORD: &str = "correct horse battery";
const STEP: Duration = Duration::from_secs(20);
const FRAME: Duration = Duration::from_millis(10);

struct TestServer {
    handle: Option<ServerHandle>,
    dir: std::path::PathBuf,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown(Duration::ZERO);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A fresh directory under the system's temporary directory.
fn temp_dir() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tessera-loadtest-v11-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

impl TestServer {
    fn handle(&self) -> &ServerHandle {
        self.handle.as_ref().expect("server")
    }
}

fn start_server(accounts: &[&str]) -> Option<TestServer> {
    if let Err(e) = std::net::UdpSocket::bind("127.0.0.1:0") {
        eprintln!("SKIPPED: no loopback UDP ({e})");
        return None;
    }
    let dir = temp_dir();
    let data = DataDir::new(dir.join("data"));
    let mut config = Config::default();
    config.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
    config.use_hash_floor();
    config.accounts.hash_workers = 2;
    config.accounts.attempts_per_address_per_min = 1_000;
    config.log.stats_interval_s = 0;
    config.server.character_creation = true;
    config.server.appearance_change = true;
    data.create().expect("data dir");
    let list: Vec<(&str, &str)> = accounts.iter().map(|a| (*a, PASSWORD)).collect();
    accounts::add_accounts(&data, HashParams::from_config(&config.accounts), &list)
        .expect("accounts");
    std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
    let handle = Server::start(ServerOptions {
        data_dir: data,
        config,
        content: Content::default_world(),
    })
    .expect("the test server starts");
    Some(TestServer {
        handle: Some(handle),
        dir,
    })
}

fn config(server: &ServerHandle, account: &str, minor_max: u16) -> RemoteConfig {
    let mut c = RemoteConfig::new(&server.connect_string(), account, Password::new(PASSWORD))
        .expect("config");
    c.client = ClientConfig {
        minor_max,
        ..c.client
    };
    c
}

fn session(server: &ServerHandle, account: &str, minor_max: u16) -> Session {
    let mut s = Session::new(RemoteBackend::new(config(server, account, minor_max)).unwrap());
    s.send(Instant::now(), Command::Connect).unwrap();
    s
}

/// Polls every session until `done` holds for the collected events; panics after STEP.
fn pump_until(
    sessions: &mut [&mut Session],
    events: &mut [Vec<Event>],
    what: &str,
    mut done: impl FnMut(&[Vec<Event>]) -> bool,
) {
    let deadline = Instant::now() + STEP;
    let mut buf = Vec::new();
    loop {
        let now = Instant::now();
        for (s, ev) in sessions.iter_mut().zip(events.iter_mut()) {
            s.poll_into(now, &mut buf);
            ev.append(&mut buf);
        }
        if done(events) {
            return;
        }
        assert!(now < deadline, "timed out waiting for {what}: {events:#?}");
        std::thread::sleep(FRAME);
    }
}

fn look() -> Look {
    Look {
        entries: vec![
            LookEntry {
                property: 0x7FFF_0001,
                value: PropertyValue::Float(0.25),
            },
            LookEntry {
                property: 0x7FFF_0002,
                value: PropertyValue::Colour([200, 150, 100, 255]),
            },
            LookEntry {
                property: 0x7FFF_0003,
                value: PropertyValue::String("80ff".into()),
            },
        ],
    }
}

fn create(s: &mut Session, name: &str, look: Option<Look>) {
    let app = Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    })
    .with_look(look);
    s.send(
        Instant::now(),
        Command::Character(CharacterCommand::CreateCharacter {
            name: name.into(),
            appearance: Some(Box::new(app)),
            class: Some(5),
            nationality: Some(2),
        }),
    )
    .unwrap();
}

fn info_of<'a>(events: &'a [Event], name: &str) -> Option<&'a CharacterEvent> {
    events.iter().rev().find_map(|e| match e {
        Event::Character(c @ CharacterEvent::CharacterInfo { name: n, .. }) if n == name => Some(c),
        _ => None,
    })
}

#[test]
fn looks_are_stored_relayed_and_kept_from_minor_0_peers() {
    let Some(server) = start_server(&["alice", "bob", "carol"]) else {
        return;
    };
    let h = server.handle();
    // Alice (minor 1) creates Aldith with a look; Carol (minor 0) creates Cora.
    let mut alice = session(h, "alice", 1);
    let mut carol = session(h, "carol", 0);
    let mut ev = vec![Vec::new(), Vec::new()];
    pump_until(&mut [&mut alice, &mut carol], &mut ev, "connected", |e| {
        e.iter()
            .all(|e| e.iter().any(|x| matches!(x, Event::Connected { .. })))
    });
    create(&mut alice, "Aldith", Some(look()));
    create(&mut carol, "Cora", Some(look()));
    pump_until(&mut [&mut alice, &mut carol], &mut ev, "created", |e| {
        info_of(&e[0], "Aldith").is_some() && info_of(&e[1], "Cora").is_some()
    });
    assert!(matches!(
        info_of(&ev[0], "Aldith"),
        Some(CharacterEvent::CharacterInfo { look: Some(l), class: None, .. }) if **l == look()
    ));
    assert!(matches!(
        info_of(&ev[1], "Cora"),
        Some(CharacterEvent::CharacterInfo { look: None, .. })
    ));
    for s in [&mut alice, &mut carol] {
        s.send(Instant::now(), Command::Disconnect).unwrap();
    }
    pump_until(&mut [&mut alice, &mut carol], &mut ev, "closed", |e| {
        e.iter()
            .all(|e| e.iter().any(|x| matches!(x, Event::Disconnected { .. })))
    });

    // A fresh login at minor 0 lists no look; one at minor 1 lists the stored look. One
    // after the other: a second login of the account would replace the first.
    let mut alice_old = session(h, "alice", 0);
    let mut ev = vec![Vec::new()];
    pump_until(&mut [&mut alice_old], &mut ev, "listed at minor 0", |e| {
        info_of(&e[0], "Aldith").is_some()
    });
    assert!(matches!(
        info_of(&ev[0], "Aldith"),
        Some(CharacterEvent::CharacterInfo { look: None, .. })
    ));
    alice_old.send(Instant::now(), Command::Disconnect).unwrap();
    pump_until(&mut [&mut alice_old], &mut ev, "closed", |e| {
        e[0].iter().any(|x| matches!(x, Event::Disconnected { .. }))
    });
    drop(alice_old);
    let mut alice = session(h, "alice", 1);
    let mut ev = vec![Vec::new()];
    pump_until(&mut [&mut alice], &mut ev, "listed at minor 1", |e| {
        info_of(&e[0], "Aldith").is_some()
    });
    assert!(matches!(
        info_of(&ev[0], "Aldith"),
        Some(CharacterEvent::CharacterInfo { look: Some(l), .. }) if **l == look()
    ));

    // Bob (minor 1) and Carol (minor 0) watch Aldith enter: Bob sees the look.
    let mut bob = session(h, "bob", 1);
    let mut carol = session(h, "carol", 0);
    let mut ev = vec![Vec::new(), Vec::new(), Vec::new()];
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "connected",
        |e| {
            e[1..]
                .iter()
                .all(|e| e.iter().any(|x| matches!(x, Event::Connected { .. })))
        },
    );
    create(&mut bob, "Bram", None);
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "Bram",
        |e| info_of(&e[1], "Bram").is_some(),
    );
    for (s, name) in [
        (&mut alice, "Aldith"),
        (&mut bob, "Bram"),
        (&mut carol, "Cora"),
    ] {
        s.send(
            Instant::now(),
            Command::EnterWorld {
                character: name.into(),
            },
        )
        .unwrap();
    }
    let created_aldith = |e: &[Event]| {
        e.iter().find_map(|x| match x {
            Event::EntityCreated {
                name, appearance, ..
            } if name == "Aldith" => Some(appearance.clone()),
            _ => None,
        })
    };
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "observers see Aldith",
        |e| created_aldith(&e[1]).is_some() && created_aldith(&e[2]).is_some(),
    );
    assert_eq!(
        created_aldith(&ev[1]).unwrap().look.as_deref(),
        Some(&look())
    );
    assert_eq!(created_aldith(&ev[2]).unwrap().look, None);
    // Alice's own entry carries the look too.
    assert!(ev[0].iter().any(|x| matches!(
        x,
        Event::AppearanceChanged { appearance, .. } if appearance.look.as_deref() == Some(&look())
    )));
}

/// The character the hostile 1.1 cases enter the world with.
const VECTOR_CHARACTER: &str = "Bram";

/// A connection logged in as `alice`, offering minor `minor` (in CharacterSelect).
async fn logged_in(
    endpoint: &quinn::Endpoint,
    target: &tessera_loadtest::raw::Target,
    minor: u16,
) -> tessera_loadtest::raw::ControlLink {
    use tessera_protocol::message::{Credentials, Message};
    let conn = tessera_loadtest::raw::connect(endpoint, target, STEP)
        .await
        .unwrap();
    let mut link = tessera_loadtest::raw::ControlLink::open(conn)
        .await
        .unwrap();
    link.send(&tessera_loadtest::raw::hello_minor(minor))
        .await
        .unwrap();
    link.expect(STEP, |m| matches!(m, Message::HelloAck(_)).then_some(()))
        .await
        .unwrap();
    link.send(&Message::Login(Credentials {
        account: "alice".into(),
        password: PASSWORD.into(),
    }))
    .await
    .unwrap();
    link.expect(STEP, |m| matches!(m, Message::LoginOk(_)).then_some(()))
        .await
        .unwrap();
    link
}

/// The world stream of one connection, decoded at the negotiated minor.
struct WorldStream {
    stream: quinn::RecvStream,
    decoder: tessera_protocol::frame::FrameDecoder,
    pending: Vec<u8>,
    /// The chat stream, kept open (dropping it would stop it).
    _chat: Vec<quinn::RecvStream>,
}

impl WorldStream {
    /// Waits for a world message `pick` accepts; panics after STEP.
    async fn expect(
        &mut self,
        what: &str,
        mut pick: impl FnMut(&tessera_protocol::Message) -> bool,
    ) {
        let deadline = tokio::time::Instant::now() + STEP;
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            while !self.pending.is_empty() {
                let mut input: &[u8] = &self.pending;
                let before = input.len();
                let r = self
                    .decoder
                    .push(&mut input)
                    .expect("the server's world frames decode");
                let used = before - input.len();
                self.pending.drain(..used);
                match r {
                    Some(m) if pick(&m) => return,
                    Some(_) => {}
                    None => break,
                }
            }
            match tokio::time::timeout_at(deadline, self.stream.read(&mut buf)).await {
                Ok(Ok(Some(n))) => self.pending.extend_from_slice(&buf[..n]),
                other => panic!("{what}: world stream ended or timed out: {other:?}"),
            }
        }
    }
}

/// A connection of `alice` that entered the world with [`VECTOR_CHARACTER`] at `minor`.
async fn in_world(
    endpoint: &quinn::Endpoint,
    target: &tessera_loadtest::raw::Target,
    minor: u16,
) -> (tessera_loadtest::raw::ControlLink, WorldStream) {
    use tessera_protocol::frame::{FrameDecoder, StreamKind};
    use tessera_protocol::message::{EnterWorld, Message};
    use tessera_protocol::{Channel, Side};
    let mut link = logged_in(endpoint, target, minor).await;
    assert_eq!(link.minor(), minor);
    link.send(&Message::EnterWorld(EnterWorld {
        name: VECTOR_CHARACTER.into(),
    }))
    .await
    .unwrap();
    let mut chat = Vec::new();
    let stream = loop {
        let mut s = tokio::time::timeout(STEP, link.conn.accept_uni())
            .await
            .expect("the server opens its streams")
            .expect("stream");
        let mut kind = [0u8; 1];
        s.read_exact(&mut kind).await.unwrap();
        if matches!(StreamKind::from_byte(kind[0]), Ok(StreamKind::World)) {
            break s;
        }
        chat.push(s);
    };
    let mut world = WorldStream {
        stream,
        decoder: FrameDecoder::with_minor(Channel::World, Side::Server, minor),
        pending: Vec::new(),
        _chat: chat,
    };
    world
        .expect("WorldEnter", |m| matches!(m, Message::WorldEnter(_)))
        .await;
    (link, world)
}

/// Waits for the server to close `conn` and returns its application code.
async fn close_code(conn: &quinn::Connection, what: &str) -> tessera_protocol::codes::AppErrorCode {
    let closed = tokio::time::timeout(STEP, conn.closed())
        .await
        .unwrap_or_else(|_| panic!("{what}: the server closes the connection"));
    match closed {
        quinn::ConnectionError::ApplicationClosed(a) => {
            let code = u16::try_from(a.error_code.into_inner()).unwrap();
            tessera_protocol::codes::AppErrorCode::from_u16(code)
        }
        other => panic!("{what}: {other:?}"),
    }
}

/// `tessera-protocol.md` §16.8 / `test-server.md` §13.1: each client-to-server vector is
/// sent in the state where its message is allowed (`CharacterCreate` in CharacterSelect,
/// `AppearanceSet` in the world, with both capabilities on), so only the v1.1 look and slot
/// rules can close the connection. Positive control: the two minor-0 vectors are valid at
/// minor 1 and, sent the same way, are answered and leave the connection open.
#[test]
fn hostile_v11_vectors_are_protocol_violations() {
    use tessera_protocol::codes::AppErrorCode;
    use tessera_protocol::message::{Message, MessageType};
    let Some(server) = start_server(&["alice"]) else {
        return;
    };
    let h = server.handle();
    let target = tessera_loadtest::raw::Target {
        addr: h.local_addrs()[0],
        server_name: "127.0.0.1".into(),
        fingerprint: h.fingerprint(),
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let endpoint = tessera_loadtest::raw::endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let provisioned = tessera_loadtest::raw::provision(
            &endpoint,
            &target,
            "alice",
            PASSWORD,
            VECTOR_CHARACTER,
            false,
            (
                tessera_loadtest::raw::bot_appearance(0),
                tessera_loadtest::raw::bot_look(0),
            ),
        )
        .await;
        assert!(provisioned.is_ok(), "{provisioned:?}");
        let vectors = tessera_loadtest::raw::negative_vectors_v11();

        // Positive control: the minor-0 vectors at minor 1, in their states.
        for v in vectors.iter().take(2) {
            let ty = u16::from_le_bytes([v.frame[4], v.frame[5]]);
            if ty == MessageType::AppearanceSet.code() {
                let (mut link, mut world) = in_world(&endpoint, &target, 1).await;
                link.send_raw(&v.frame).await.unwrap();
                world
                    .expect(v.name, |m| matches!(m, Message::EntityAppearance(_)))
                    .await;
                assert!(link.conn.close_reason().is_none(), "{}", v.name);
                link.close().await;
            } else {
                let mut link = logged_in(&endpoint, &target, 1).await;
                link.send_raw(&v.frame).await.unwrap();
                link.expect(STEP, |m| {
                    matches!(
                        m,
                        Message::CharacterList(_) | Message::CharacterCreateFailed(_)
                    )
                    .then_some(())
                })
                .await
                .unwrap_or_else(|e| panic!("{}: {e}", v.name));
                assert!(link.conn.close_reason().is_none(), "{}", v.name);
                link.close().await;
            }
        }

        // Every vector, in its message's own state, is a protocol violation.
        for v in &vectors {
            let ty = u16::from_le_bytes([v.frame[4], v.frame[5]]);
            let link = if ty == MessageType::AppearanceSet.code() {
                let (link, world) = in_world(&endpoint, &target, v.minor_max).await;
                drop(world);
                link
            } else {
                assert_eq!(ty, MessageType::CharacterCreate.code(), "{}", v.name);
                logged_in(&endpoint, &target, v.minor_max).await
            };
            let mut link = link;
            link.send_raw(&v.frame).await.unwrap();
            assert_eq!(
                close_code(&link.conn, v.name).await,
                AppErrorCode::ProtocolViolation,
                "{}",
                v.name
            );
        }
    });
}

/// `did` selectors return their listed DIDs (an install that has them).
#[derive(Debug)]
struct Dids;

impl SelectorResolver for Dids {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        if query.family != Family::Did {
            return CandidateOutcome::new(CandidateStatus::Unsupported);
        }
        out.extend(query.dids.iter().map(|&did| Candidate {
            did,
            row: 0,
            location: None,
            anchor: None,
        }));
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, _: LabelContext, _: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _: DataId, _: Option<u32>) -> Option<String> {
        None
    }
}

const OVERLAY: &str = r#"tessera-scenario 1
title "Overlay"
tick 100ms
select $gear = did 0x70001234,0x70005678 all
item $sword $gear[0]
item $shield $gear[1]
at 0s give $sword
+0s give $shield
+0s equip $sword slot=17
+0s equip $shield slot=18
"#;

/// §14.9 O9: an overlay run equips from a scenario; an observer at minor 1 sees the items
/// with their slots, one at minor 0 sees the items without slots (P1 exit criterion 3).
#[test]
fn o9_observers_see_the_overlay_players_equipment() {
    let Some(server) = start_server(&["alice", "bob", "carol"]) else {
        return;
    };
    let h = server.handle();
    // Characters first.
    let mut ev = vec![Vec::new(), Vec::new(), Vec::new()];
    let mut a0 = session(h, "alice", 1);
    let mut b0 = session(h, "bob", 1);
    let mut c0 = session(h, "carol", 0);
    pump_until(
        &mut [&mut a0, &mut b0, &mut c0],
        &mut ev,
        "connected",
        |e| {
            e.iter()
                .all(|e| e.iter().any(|x| matches!(x, Event::Connected { .. })))
        },
    );
    create(&mut a0, "Aldith", None);
    create(&mut b0, "Bram", None);
    create(&mut c0, "Cora", None);
    pump_until(&mut [&mut a0, &mut b0, &mut c0], &mut ev, "created", |e| {
        info_of(&e[0], "Aldith").is_some()
            && info_of(&e[1], "Bram").is_some()
            && info_of(&e[2], "Cora").is_some()
    });
    drop((a0, b0, c0));

    let scenario = Scenario::parse(OVERLAY.as_bytes()).unwrap();
    scenario.check_overlay().unwrap();
    let resolved = Arc::new(scenario.resolve(&mut Dids).unwrap());
    let remote = RemoteBackend::new(config(h, "alice", 1)).unwrap();
    let mut alice = Session::new(OverlayBackend::new(remote, resolved));
    alice.send(Instant::now(), Command::Connect).unwrap();
    let mut bob = session(h, "bob", 1);
    let mut carol = session(h, "carol", 0);
    let mut ev = vec![Vec::new(), Vec::new(), Vec::new()];
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "connected",
        |e| {
            e.iter()
                .all(|e| e.iter().any(|x| matches!(x, Event::Connected { .. })))
        },
    );
    // Observers enter first, so they watch Aldith arrive and then change.
    for (s, name) in [(&mut bob, "Bram"), (&mut carol, "Cora")] {
        s.send(
            Instant::now(),
            Command::EnterWorld {
                character: name.into(),
            },
        )
        .unwrap();
    }
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "observers in the world",
        |e| {
            e[1..]
                .iter()
                .all(|e| e.iter().any(|x| matches!(x, Event::WorldEntered { .. })))
        },
    );
    alice
        .send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldith".into(),
            },
        )
        .unwrap();
    let aldith = |e: &[Event]| {
        e.iter().find_map(|x| match x {
            Event::EntityCreated { id, name, .. } if name == "Aldith" => Some(*id),
            _ => None,
        })
    };
    let changed = |e: &[Event], id: Option<EntityId>| {
        e.iter().rev().find_map(|x| match x {
            Event::AppearanceChanged { id: i, appearance } if Some(*i) == id => {
                Some(appearance.equipment.clone())
            }
            _ => None,
        })
    };
    let both = |slots: bool| {
        vec![
            Equipped {
                slot: slots.then_some(17),
                ..Equipped::new(DataId(0x7000_1234))
            },
            Equipped {
                slot: slots.then_some(18),
                ..Equipped::new(DataId(0x7000_5678))
            },
        ]
    };
    pump_until(
        &mut [&mut alice, &mut bob, &mut carol],
        &mut ev,
        "observers see the equipment",
        |e| {
            let b = changed(&e[1], aldith(&e[1]));
            let c = changed(&e[2], aldith(&e[2]));
            b.as_deref() == Some(&both(true)[..]) && c.as_deref() == Some(&both(false)[..])
        },
    );
    let overlay = alice
        .backend::<OverlayBackend<RemoteBackend>>()
        .expect("overlay");
    assert!(overlay.appearance_shared());
    assert!(overlay.appearance_sets_sent() >= 1);
    // The player's own view shows what the server holds.
    let own = changed(&ev[0], alice.player());
    assert_eq!(own.as_deref(), Some(&both(true)[..]));
    assert_eq!(overlay.name(), "tessera-server");
}
