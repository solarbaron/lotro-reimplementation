// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2 exit criterion 4 (decision 0032): an in-process `tessera-server` with
//! the default world's wanderers and two clients through the client's own `net::World`.
//! Client A selects a wanderer and then B's character with `SelectTarget`; each selection
//! is answered locally by the test-server backend (`TargetChanged`, D1) and the target
//! frame shows the name, no level and the no-vitals form (D2). When B leaves, A's selection
//! of B clears locally and the combat frame sends no command (D2). The same selection works
//! in an overlay run (`--scenario` with `--server`, D1 item 7).
//!
//! The target frame is the combat widgets' fallback look on a UI host without an install
//! (the figure needs the walker and an install; the no-vitals form shows none). Needs no
//! game install and no GPU. Skips itself (reporting "passed", printing `SKIPPED`) only where
//! loopback UDP is unavailable.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::combat::CombatChange;
use tessera_client::combat::commands::select;
use tessera_client::net::{RemoteSettings, World};
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_net::remote::Password;
use tessera_net::{EntityId, SessionState};
use tessera_protocol::types::{Appearance, Body};
use tessera_server::accounts::{self, HashParams};
use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};
use tessera_ui::game::combat::{FloatySettings, LevelShown};

const PASSWORD: &str = "correct horse battery";
const STEP: Duration = Duration::from_secs(20);
const FRAME: Duration = Duration::from_millis(16);

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
    data.create().expect("data dir");
    accounts::add_accounts(
        &data,
        HashParams::from_config(&config.accounts),
        &[("alice", PASSWORD), ("bob", PASSWORD)],
    )
    .expect("accounts");
    std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
    let record = |name: &str| CharacterRecord {
        name: name.into(),
        created: "2026-10-11T00:00:00Z".into(),
        instance: 0,
        position: None,
        appearance: StoredAppearance::Valid(Appearance::new(Body::Character {
            species: 23,
            sex: 1,
        })),
        look: tessera_protocol::types::Look::none(),
    };
    for (account, name) in [("alice", "Aldith"), ("bob", "Bram")] {
        let file = CharacterFile {
            account: account.into(),
            characters: vec![record(name)],
        };
        std::fs::write(data.character_path(account), file.to_toml()).expect("character file");
    }
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

/// The entity whose name starts with `prefix` (not the player).
fn find(world: &World, prefix: &str) -> Option<EntityId> {
    world
        .entities()
        .iter()
        .find(|(id, e)| e.name.starts_with(prefix) && Some(**id) != world.player())
        .map(|(id, _)| *id)
}

/// The combat widgets on a UI host without an install, fed from a world's combat state.
struct Frames {
    host: UiHost,
}

impl Frames {
    fn new() -> Self {
        let mut host = UiHost::without_install(&UiOptions {
            screen: (1024, 768),
            hud: false,
            ..UiOptions::default()
        });
        host.attach_combat(FloatySettings::default(), 0.0);
        Self { host }
    }

    /// One combat frame; returns the changes and the commands it produced.
    fn frame(&mut self, w: &mut World) -> (Vec<CombatChange>, usize) {
        let now = w.combat_clock(Instant::now());
        let changes = w.take_combat_changes();
        let commands =
            self.host
                .combat_frame(w.combat_state(), &changes, Vec::new(), None, now, &|_| None);
        let _ = self.host.frame_without_plugins(None);
        (changes, commands.len())
    }

    /// The target frame: (name, level, has vitals, displayed).
    fn target(&self) -> Option<(String, LevelShown, bool, bool)> {
        let hud = self.host.combat_hud()?;
        let f = hud.widgets().target.as_ref()?;
        let v = f.view()?;
        let ui = self.host.ui();
        Some((
            v.name.clone(),
            v.level,
            v.has_vitals,
            ui.is_displayed(f.root()),
        ))
    }
}

/// Updates the worlds every frame until `done` holds; panics after [`STEP`].
fn run_until(
    worlds: &mut [&mut World],
    what: &str,
    mut done: impl FnMut(&mut [&mut World]) -> bool,
) {
    let deadline = Instant::now() + STEP;
    loop {
        let now = Instant::now();
        for w in worlds.iter_mut() {
            w.update(now);
        }
        if done(worlds) {
            return;
        }
        assert!(now < deadline, "timed out waiting for {what}");
        std::thread::sleep(FRAME);
    }
}

#[test]
fn a_wanderer_and_a_player_are_selected_and_shown_in_the_target_frame() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let now = Instant::now();
    let mut alice =
        World::remote(now, &settings(handle, "alice", "Aldith"), None, None).expect("alice");
    let mut bob = World::remote(now, &settings(handle, "bob", "Bram"), None, None).expect("bob");
    run_until(&mut [&mut alice, &mut bob], "both in the world", |w| {
        w[0].state() == SessionState::InWorld
            && w[1].state() == SessionState::InWorld
            && find(w[0], "Wanderer").is_some()
            && find(w[0], "Bram").is_some()
    });
    let mut frames = Frames::new();
    frames.frame(&mut alice);
    assert!(frames.target().is_none(), "nothing selected yet");

    // 1. A wanderer (kind NonPlayer, no vitals, no level).
    let wanderer = find(&alice, "Wanderer").expect("a wanderer");
    alice
        .send_command(Instant::now(), select(Some(wanderer)))
        .expect("SelectTarget accepted (decision 0032 D1)");
    run_until(&mut [&mut alice, &mut bob], "the wanderer selected", |w| {
        w[0].combat_state().selection() == Some(wanderer)
    });
    frames.frame(&mut alice);
    let (name, level, vitals, shown) = frames.target().expect("target frame");
    assert!(name.starts_with("Wanderer"), "the delivered name");
    assert_eq!(level, LevelShown::Nothing, "no level (D2)");
    assert!(!vitals, "the no-vitals form (TR12, D2)");
    assert!(shown, "the target frame shows (TR17)");

    // 2. The other client's character.
    let bram = find(&alice, "Bram").expect("Bram");
    alice
        .send_command(Instant::now(), select(Some(bram)))
        .expect("SelectTarget accepted");
    run_until(&mut [&mut alice, &mut bob], "Bram selected", |w| {
        w[0].combat_state().selection() == Some(bram)
    });
    frames.frame(&mut alice);
    let (name, _, vitals, shown) = frames.target().expect("target frame");
    assert_eq!(name, "Bram");
    assert!(!vitals && shown);

    // 3. Bob leaves: the selection clears locally, with no command (D2).
    bob.shutdown();
    let mut cleared = None;
    run_until(&mut [&mut alice], "Bram gone from Alice's view", |w| {
        let now = w[0].combat_clock(Instant::now());
        let changes = w[0].take_combat_changes();
        let commands = frames.host.combat_frame(
            w[0].combat_state(),
            &changes,
            Vec::new(),
            None,
            now,
            &|_| None,
        );
        assert!(
            commands.is_empty(),
            "no command from a display update (CUI8)"
        );
        if let Some(c) = changes
            .iter()
            .find(|c| matches!(c, CombatChange::Selection { .. }))
        {
            cleared = Some(c.clone());
        }
        w[0].combat_state().selection().is_none()
    });
    assert_eq!(
        cleared,
        Some(CombatChange::Selection {
            previous: Some(bram),
            current: None,
            local: true
        })
    );
    let _ = frames.host.frame_without_plugins(None);
    assert!(
        frames.target().is_none_or(|t| !t.3),
        "the target frame hides (TR17)"
    );
    alice.shutdown();
}

#[test]
fn an_overlay_run_selects_a_wanderer() {
    let Some(server) = start_server() else {
        return;
    };
    let handle = server.handle.as_ref().expect("server");
    let text = "tessera-scenario 1\ntitle \"Overlay selection\"\ntick 100ms\n\
                at 0s bag 30\n";
    let parsed = tessera_net::scenario::Scenario::parse(text.as_bytes()).expect("parses");
    parsed.check_overlay().expect("overlay subset");
    let mut resolver = tessera_client::scenario::resolver::ClientResolver::for_install(None);
    let sc = Arc::new(parsed.resolve(resolver.as_dyn()).expect("resolves"));
    let mut alice = World::overlay(
        Instant::now(),
        &settings(handle, "alice", "Aldith"),
        sc,
        None,
        None,
    )
    .expect("overlay session");
    run_until(&mut [&mut alice], "in the world with a wanderer", |w| {
        w[0].state() == SessionState::InWorld && find(w[0], "Wanderer").is_some()
    });
    let wanderer = find(&alice, "Wanderer").expect("a wanderer");
    alice
        .send_command(Instant::now(), select(Some(wanderer)))
        .expect("SelectTarget routed to the test-server backend (D1 item 7)");
    run_until(&mut [&mut alice], "the wanderer selected", |w| {
        w[0].combat_state().selection() == Some(wanderer)
    });
    let mut frames = Frames::new();
    frames.frame(&mut alice);
    let (name, _, vitals, shown) = frames.target().expect("target frame");
    assert!(name.starts_with("Wanderer") && !vitals && shown);
    alice.shutdown();
}
