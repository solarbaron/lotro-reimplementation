// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless network check (M7): an in-process `tessera-server` on loopback and two client
//! sessions through the client's own `net::World` (the code path the walker uses, with the
//! walker's `WalkerSession` calls for chat and interpolation). Each client must see the
//! other's character, its moves interpolated (`tessera-protocol.md` §8.6), and a chat line.
//!
//! Needs no game install and no GPU. Skips itself (reporting "passed", and printing
//! `SKIPPED` for CI to find) only where loopback UDP is unavailable; any failure to start
//! the server is a test failure.

use std::time::{Duration, Instant};

use tessera_client::net::{RemoteSettings, World};
use tessera_client::walker::WalkerSession;
use tessera_net::interp::SampleKind;
use tessera_net::remote::Password;
use tessera_net::{ChatChannel, EntityId, Location, SessionState, Vec3};
use tessera_protocol::types::{Appearance, Body};
use tessera_server::accounts::{self, HashParams};
use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

const PASSWORD: &str = "correct horse battery";
/// Longest wait for one step.
const STEP: Duration = Duration::from_secs(20);
/// Client frame interval.
const FRAME: Duration = Duration::from_millis(16);
/// The server's message of the day in these tests.
const MOTD: &str = "For testing only.";

struct TestServer {
    handle: Option<ServerHandle>,
    _dir: tempfile::TempDir,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown(Duration::ZERO);
        }
    }
}

/// Starts a server with accounts `alice` (characters Aldith and Bramble) and `bob`
/// (character Bram), or `None` when loopback UDP is unavailable.
fn start_server() -> Option<TestServer> {
    if let Err(e) = std::net::UdpSocket::bind("127.0.0.1:0") {
        eprintln!("SKIPPED: no loopback UDP ({e})");
        return None;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let data = DataDir::new(dir.path().join("data"));
    let mut config = Config::default();
    config.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
    config.use_hash_floor();
    config.accounts.hash_workers = 2;
    config.accounts.attempts_per_address_per_min = 1_000;
    config.log.stats_interval_s = 0;
    config.server.motd = Some(MOTD.to_owned());
    data.create().expect("data dir");
    accounts::add_accounts(
        &data,
        HashParams::from_config(&config.accounts),
        &[("alice", PASSWORD), ("bob", PASSWORD)],
    )
    .expect("accounts");
    std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
    let record = |name: &str, species| CharacterRecord {
        name: name.into(),
        created: "2026-10-07T00:00:00Z".into(),
        instance: 0,
        position: None,
        appearance: StoredAppearance::Valid(Appearance::new(Body::Character { species, sex: 1 })),
    };
    for (account, characters) in [
        ("alice", vec![record("Aldith", 23), record("Bramble", 81)]),
        ("bob", vec![record("Bram", 81)]),
    ] {
        let file = CharacterFile {
            account: account.into(),
            characters,
        };
        std::fs::write(data.character_path(account), file.to_toml()).expect("character file");
    }
    // Loopback UDP works (checked above), so any start-up error is a bug.
    let handle = Server::start(ServerOptions {
        data_dir: data,
        config,
        content: Content::default_world(),
    })
    .expect("the test server starts");
    Some(TestServer {
        handle: Some(handle),
        _dir: dir,
    })
}

fn settings(server: &ServerHandle, account: &str, character: &str) -> RemoteSettings {
    RemoteSettings {
        server: server.connect_string(),
        fingerprint: Some(server.fingerprint()),
        known_servers: None,
        accept_new_key: false,
        account: account.into(),
        password: Password::new(PASSWORD),
        character: Some(character.into()),
    }
}

/// Updates both worlds (and calls `each`) every frame until `done` holds; panics after
/// [`STEP`].
fn run_until(
    worlds: &mut [&mut World; 2],
    what: &str,
    mut each: impl FnMut(&mut [&mut World; 2], Instant),
    mut done: impl FnMut(&mut [&mut World; 2], Instant) -> bool,
) {
    let deadline = Instant::now() + STEP;
    loop {
        let now = Instant::now();
        for w in worlds.iter_mut() {
            w.update(now);
        }
        each(worlds, now);
        if done(worlds, now) {
            return;
        }
        assert!(
            now < deadline,
            "timed out waiting for {what}: states {:?} / {:?}",
            worlds[0].net_status(),
            worlds[1].net_status()
        );
        std::thread::sleep(FRAME);
    }
}

/// The entity named `name` in `world`.
fn find(world: &World, name: &str) -> Option<EntityId> {
    world
        .entities()
        .iter()
        .find(|(id, e)| e.name == name && Some(**id) != world.player())
        .map(|(id, _)| *id)
}

#[test]
fn two_clients_see_each_other_move_and_chat() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let now = Instant::now();
    let mut alice = World::remote(now, &settings(handle, "alice", "Aldith"), None, None)
        .expect("alice session");
    let mut bob =
        World::remote(now, &settings(handle, "bob", "Bram"), None, None).expect("bob session");
    alice.capture_entity_events(true);
    bob.capture_entity_events(true);
    let mut worlds = [&mut alice, &mut bob];

    // 1. Both enter the world and see each other's character.
    run_until(
        &mut worlds,
        "both in the world, each seeing the other",
        |_, _| {},
        |w, _| {
            w[0].state() == SessionState::InWorld
                && w[1].state() == SessionState::InWorld
                && w[0].player_location().is_some()
                && find(w[0], "Bram").is_some()
                && find(w[1], "Aldith").is_some()
        },
    );
    let status = worlds[0].net_status();
    assert_eq!(status.backend, "tessera-server");
    assert!(status.delay.is_some());
    let aldith_in_bob = find(worlds[1], "Aldith").expect("Aldith seen by Bob");
    let mut events = Vec::new();
    WalkerSession::drain_entity_events(worlds[1], &mut events, usize::MAX);
    assert!(
        events.iter().any(|e| matches!(
            e,
            tessera_net::Event::EntityCreated { id, .. } if *id == aldith_in_bob
        )),
        "Bob's walker gets Aldith's creation"
    );

    // 2. Alice walks east at 3 m/s; Bob's interpolated samples of her follow.
    let start = worlds[0].player_location().expect("Alice placed").location;
    let speed = 3.0_f64;
    let walk_start = Instant::now();
    let mut kinds = std::collections::HashSet::new();
    let mut xs: Vec<(Instant, f32)> = Vec::new();
    run_until(
        &mut worlds,
        "Alice's walk interpolated on Bob's side",
        |w, now| {
            let t = now
                .saturating_duration_since(walk_start)
                .as_secs_f64()
                .min(3.0);
            let moving = t < 3.0;
            let loc = Location::new(start.region, start.x + speed * t, start.y, start.z);
            let v = if moving {
                Vec3::new(speed as f32, 0.0, 0.0)
            } else {
                Vec3::ZERO
            };
            WalkerSession::send_move(w[0], now, loc, 0.0, v);
            if let Some(s) = WalkerSession::sample_entity(w[1], aldith_in_bob, now) {
                kinds.insert(s.kind);
                xs.push((now, s.position.x));
            }
            let mut sink = Vec::new();
            WalkerSession::drain_entity_events(w[1], &mut sink, usize::MAX);
        },
        |w, now| {
            now.saturating_duration_since(walk_start) > Duration::from_millis(3_600)
                && w[1]
                    .sample_entity(aldith_in_bob, now)
                    .is_some_and(|s| (f64::from(s.position.x) - (start.x + 9.0)).abs() < 1.0)
        },
    );
    assert!(
        kinds.contains(&SampleKind::Interpolated),
        "moves were interpolated: {kinds:?}"
    );
    let first = xs.first().expect("samples").1;
    let last = xs.last().expect("samples").1;
    assert!(
        f64::from(last - first) > 8.0,
        "Bob saw Alice walk about 9 m: {first} -> {last}"
    );
    // Interpolation is smooth: each step is what 3 m/s covers in the time between the two
    // samples, plus a little (the raw snapshots would jump in 1/15 s steps whatever the
    // frame time), so a slow frame on a loaded machine does not fail the test.
    for p in xs.windows(2) {
        let dt = p[1].0.saturating_duration_since(p[0].0).as_secs_f64();
        let step = f64::from((p[1].1 - p[0].1).abs());
        assert!(
            step <= speed * dt + 0.5,
            "a step of {step} m in {dt} s at {speed} m/s"
        );
    }
    assert!(
        xs.windows(2).all(|p| p[1].1 >= p[0].1 - 1e-3),
        "never moves backwards"
    );

    // 3. Bob says something; both see it (Alice as a received line, Bob as the echo).
    WalkerSession::send_chat(
        worlds[1],
        Instant::now(),
        &ChatChannel::Say,
        "hello from Bram",
    );
    let mut alice_lines = Vec::new();
    let mut bob_lines = Vec::new();
    run_until(
        &mut worlds,
        "the chat line on both sides",
        |_, _| {},
        |w, _| {
            WalkerSession::drain_chat(w[0], &mut alice_lines);
            WalkerSession::drain_chat(w[1], &mut bob_lines);
            let has = |l: &Vec<tessera_client::net::ChatLine>| {
                l.iter()
                    .any(|c| c.sender == "Bram" && c.text == "hello from Bram")
            };
            has(&alice_lines) && has(&bob_lines)
        },
    );
    // The plugin host's path (`take_chat`) sees the same line.
    assert!(
        worlds[0]
            .take_chat()
            .iter()
            .any(|c| c.text == "hello from Bram")
    );
    // The server's name and message of the day reached the overlay as system lines.
    for lines in [&alice_lines, &bob_lines] {
        assert!(
            lines
                .iter()
                .any(|c| c.sender.is_empty() && c.text == "Connected to Tessera test server."),
            "{lines:?}"
        );
        assert!(lines.iter().any(|c| c.sender.is_empty() && c.text == MOTD));
    }

    // 4. Time sync and round trip reached the HUD.
    run_until(
        &mut worlds,
        "RTT and clock offset",
        |_, _| {},
        |w, _| {
            let s = w[0].net_status();
            s.rtt.is_some() && s.clock_offset_us.is_some()
        },
    );

    alice.shutdown();
    bob.shutdown();
    assert_eq!(alice.state(), SessionState::Closed);
    assert_eq!(bob.state(), SessionState::Closed);
}

#[test]
fn an_unknown_server_key_is_refused_with_a_hint() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let mut s = settings(handle, "alice", "Aldith");
    // No pin, no known-servers file, no trust on first use: refused before the password.
    s.server = format!("tessera://{}", handle.local_addr());
    s.fingerprint = None;
    let mut world = World::remote(Instant::now(), &s, None, None).expect("session");
    let deadline = Instant::now() + STEP;
    let mut lines = Vec::new();
    while world.state() != SessionState::Closed {
        assert!(Instant::now() < deadline, "not refused");
        world.update(Instant::now());
        std::thread::sleep(FRAME);
    }
    world.drain_hud_chat(&mut lines);
    let fp = handle.fingerprint().to_string();
    assert!(
        lines
            .iter()
            .any(|l| l.text.contains(&fp) && l.text.contains("--server-fingerprint")),
        "{lines:?}"
    );
    assert_eq!(handle.stats().logged_in, 0);
}

/// Runs one session until `done` holds or it closes; panics after [`STEP`].
fn run_one(world: &mut World, done: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + STEP;
    while !done(world) && world.state() != SessionState::Closed {
        assert!(
            Instant::now() < deadline,
            "timed out: {:?}",
            world.net_status()
        );
        world.update(Instant::now());
        std::thread::sleep(FRAME);
    }
}

#[test]
fn the_character_is_matched_by_key_and_never_substituted() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    // Another case than stored: entered as Bramble, not as the first character (Aldith).
    let mut world = World::remote(
        Instant::now(),
        &settings(handle, "alice", "BRAMBLE"),
        None,
        None,
    )
    .expect("session");
    run_one(&mut world, |w| w.player().is_some());
    let player = world.player().expect("in the world");
    assert_eq!(world.entities()[&player].name, "Bramble");
    assert_eq!(world.state(), SessionState::InWorld);
    world.shutdown();

    // A name the account does not have: not entered, a system line names the characters.
    let mut world = World::remote(
        Instant::now(),
        &settings(handle, "alice", "Bramm"),
        None,
        None,
    )
    .expect("session");
    run_one(&mut world, |w| w.player().is_some());
    assert_eq!(world.state(), SessionState::Closed, "not entered");
    assert!(world.player().is_none());
    let mut lines = Vec::new();
    world.drain_hud_chat(&mut lines);
    assert!(
        lines.iter().any(|l| l.sender.is_empty()
            && l.text.contains("'Bramm'")
            && l.text.contains("Aldith, Bramble")),
        "{lines:?}"
    );
}

#[test]
fn chat_before_entering_the_world_is_reported_as_not_sent() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let mut s = settings(handle, "alice", "Aldith");
    s.server = format!("tessera://{}", handle.local_addr());
    s.fingerprint = None;
    // Refused (unknown key): the session closes, and chat says it was not sent.
    let mut world = World::remote(Instant::now(), &s, None, None).expect("session");
    assert!(!WalkerSession::send_chat(
        &mut world,
        Instant::now(),
        &ChatChannel::Say,
        "too early"
    ));
    run_one(&mut world, |_| false);
    assert_eq!(world.state(), SessionState::Closed);
    assert!(!WalkerSession::send_chat(
        &mut world,
        Instant::now(),
        &ChatChannel::Say,
        "too late"
    ));
}

#[test]
fn a_wrong_pin_is_refused_with_a_pin_hint() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let mut s = settings(handle, "alice", "Aldith");
    s.server = format!("tessera://{}", handle.local_addr());
    let wrong = format!("sha256:{}", "ab".repeat(32));
    s.fingerprint = Some(tessera_net::remote::Fingerprint::parse(&wrong).expect("fingerprint"));
    let mut world = World::remote(Instant::now(), &s, None, None).expect("session");
    run_one(&mut world, |_| false);
    let mut lines = Vec::new();
    world.drain_hud_chat(&mut lines);
    let fp = handle.fingerprint().to_string();
    assert!(
        lines.iter().any(|l| l.text.contains(&fp)
            && l.text.contains("pinned")
            && !l.text.contains("known-servers")),
        "{lines:?}"
    );
    assert_eq!(handle.stats().logged_in, 0);
}
